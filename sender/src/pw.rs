use crate::dispatch_vad;
use crate::event_bridge::StreamEventBridge;
use crate::fade::{Fade, FadeDirection};
use crate::rt_debug;

use crate::vad::{DynamicVad, VadConfig};
use log::{error, info};
use pipewire::{
    context::ContextRc,
    main_loop::MainLoopRc,
    properties::PropertiesBox,
    spa,
    stream::{StreamFlags, StreamRc},
    types::ObjectType,
};
use ringbuf::traits::Producer;
use screamwire_common::pw::make_format_data;
use screamwire_common::types::AudioParams;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
struct StreamStateFlags {
    reset: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
}

impl StreamStateFlags {
    fn new() -> Self {
        Self {
            reset: Arc::new(AtomicBool::new(false)),
            idle: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline(always)]
    fn request_reset(&self) {
        self.idle.store(false, Ordering::Release);
        self.reset.store(true, Ordering::Release);
    }

    #[inline(always)]
    fn request_idle(&self) {
        self.reset.store(false, Ordering::Release);
        self.idle.store(true, Ordering::Release);
    }

    #[inline(always)]
    fn take_reset(&self) -> bool {
        self.reset.swap(false, Ordering::Acquire)
    }

    #[inline(always)]
    fn take_idle(&self) -> bool {
        self.idle.swap(false, Ordering::Acquire)
    }
}

/// Build stream properties, flags and a human-readable description.
/// Properties are grouped hierarchically by operational priority and impact.
#[inline]
fn make_stream_config(
    format: AudioParams,
    sink_name: Option<&str>,
) -> (PropertiesBox, StreamFlags, String) {
    let mut props = PropertiesBox::new();

    // Common properties
    props.insert(*pipewire::keys::NODE_ALWAYS_PROCESS, "false");
    props.insert(*pipewire::keys::APP_NAME, "ScreamWire");
    props.insert(*pipewire::keys::APP_ID, "io.github.avalak.screamwire");
    props.insert(*pipewire::keys::MEDIA_SOFTWARE, "ScreamWire");
    props.insert(*pipewire::keys::NODE_DESCRIPTION, "ScreamWire Sender");
    props.insert(*pipewire::keys::MEDIA_TYPE, "Audio");
    props.insert(*pipewire::keys::MEDIA_ROLE, "Production");
    props.insert(
        *pipewire::keys::NODE_LATENCY,
        format!("{}/{}", 256, format.rate),
    ); // TODO: replace magic number with config

    if let Some(name) = sink_name {
        // Capture from an existing sink
        props.insert(*pipewire::keys::MEDIA_CATEGORY, "Manager");
        props.insert(*pipewire::keys::STREAM_CAPTURE_SINK, "true");
        props.insert(*pipewire::keys::TARGET_OBJECT, name); // feature `v0_3_44` required
        props.insert(*pipewire::keys::CLIENT_NAME, "ScreamWire");
        props.insert(*pipewire::keys::MEDIA_NAME, "Capture audio");
        props.insert(*pipewire::keys::APP_ICON_NAME, "audio-speakers");

        (
            props,
            StreamFlags::RT_PROCESS | StreamFlags::MAP_BUFFERS | StreamFlags::AUTOCONNECT,
            format!("capture from '{}'", name),
        )
    } else {
        // Create a virtual sink
        props.insert(*pipewire::keys::MEDIA_CATEGORY, "Playback");
        props.insert(*pipewire::keys::NODE_NAME, "ScreamWire");
        props.insert(*pipewire::keys::MEDIA_CLASS, "Audio/Sink");
        props.insert(*pipewire::keys::NODE_VIRTUAL, "true");

        (
            props,
            StreamFlags::RT_PROCESS | StreamFlags::MAP_BUFFERS | StreamFlags::AUTOCONNECT,
            "virtual sink 'ScreamWire'".to_string(),
        )
    }
}

/// Return a list of all `node.name` values for PipeWire nodes with
/// `media.class = "Audio/Sink"`.
pub fn get_sink_names(mainloop: MainLoopRc, context: ContextRc) -> Vec<String> {
    let core = context.connect_rc(None).expect("Failed to connect to core");
    let registry = core.get_registry().expect("Failed to get registry");

    let sinks = Rc::new(RefCell::new(Vec::new()));

    {
        let sinks_clone = Rc::clone(&sinks);

        let _registry_listener = registry
            .add_listener_local()
            .global(move |global| {
                if global.type_ == ObjectType::Node
                    && let Some(props) = global.props
                    && props.get("media.class") == Some("Audio/Sink")
                {
                    let name = props.get("node.name").unwrap_or("Unknown").to_string();
                    sinks_clone.borrow_mut().push(name);
                }
            })
            .register();

        let sync_seq = core.sync(0).expect("Failed to sync core");
        let mainloop_clone = mainloop.clone();

        let _core_listener = core
            .add_listener_local()
            .done(move |id, seq| {
                if id == pipewire::core::PW_ID_CORE && seq == sync_seq {
                    mainloop_clone.quit();
                }
            })
            .register();

        mainloop.run();
    }

    Rc::try_unwrap(sinks)
        .expect("Rc still has multiple owners")
        .into_inner()
}

/// Universal audio stream runner.
///
/// * `target_sink = Some(name)` -> capture from the monitor of an existing sink.
/// * `target_sink = None`       -> create a virtual "ScreamWire" output device.
///
/// The `fade_ms` parameter controls the duration of the initial fade‑in
#[allow(clippy::too_many_arguments)]
pub fn run_audio_stream(
    mainloop: MainLoopRc,
    context: ContextRc,
    mut producer: impl Producer<Item = u8> + Send + 'static,
    format: AudioParams,
    target_sink: Option<String>,
    vad_config: VadConfig,
    fade_ms: u32,
    event_bridge: StreamEventBridge,
) -> Result<(), Box<dyn std::error::Error>> {
    let core = context.connect_rc(None)?;

    // SPA format pod
    let pod_data = make_format_data(format);
    let pod = spa::pod::Pod::from_bytes(&pod_data).unwrap();
    let mut params = [pod];

    // Configure properties and flags based on mode
    let (props, flags, log_desc) = make_stream_config(format, target_sink.as_deref());
    let log_desc_for_closure = log_desc.clone();

    // Monomorphize VAD
    let mut vad = DynamicVad::from_config(vad_config);

    // Fade-in processor (real‑time safe, no allocations)
    let fade_in = Rc::new(RefCell::new(Fade::new(
        fade_ms,
        format.rate,
        FadeDirection::In,
    )));
    let fade_in_clone = fade_in.clone();

    // Stream logic
    let state_flags = StreamStateFlags::new();
    let process_flags = state_flags.clone();

    // Track previous VAD state to reset fade on reactivation
    let mut vad_was_active = false;

    // Stream
    let stream = match StreamRc::new(core.clone(), "screamwire-stream", props) {
        Ok(s) => s,
        Err(e) => {
            info!("Failed to create stream: {}", e);
            return Err(e.into());
        }
    };
    let listener = stream
        .add_local_listener::<()>()
        .process(move |s, _| {
            if process_flags.take_idle() {
                dispatch_vad!(&mut vad, |v| v.force_idle());
                vad_was_active = false;
                event_bridge.notify_flush();
            }
            if process_flags.take_reset() {
                dispatch_vad!(&mut vad, |v| v.reset());
                // Restart fade‑in when the stream (re)starts
                fade_in_clone.borrow_mut().reset();
                vad_was_active = false;
            }

            let mut should_notify = false;
            while let Some(mut buf) = s.dequeue_buffer() {
                let Some(data) = buf.datas_mut().first_mut() else {
                    continue;
                };
                let off = data.chunk().offset() as usize;
                let sz = data.chunk().size() as usize;
                let Some(bytes) = data.data() else { continue };
                // Buffer from PipeWire. Should be safe
                let raw_audio = &mut bytes[off..off + sz];

                // Monomorphized call
                let is_active = dispatch_vad!(&mut vad, |v| v.process(raw_audio));
                if is_active {
                    // Reset fade when transitioning from inactive to active
                    if !vad_was_active {
                        fade_in_clone.borrow_mut().reset();
                    }
                    // Apply fade‑in if still in progress
                    fade_in_clone
                        .borrow_mut()
                        .apply(raw_audio, format.bits, format.channels);
                    producer.push_slice(raw_audio);
                    should_notify = true;
                }
                vad_was_active = is_active;
            }
            if should_notify {
                event_bridge.notify_data_ready();
            }
        })
        .state_changed(move |_stream, _user_data, _old, new| {
            rt_debug!(
                "Stream state: {:?} -> {:?} ({})",
                _old,
                new,
                log_desc_for_closure
            );
            match new {
                pipewire::stream::StreamState::Streaming => {
                    state_flags.request_reset();
                }
                pipewire::stream::StreamState::Paused
                | pipewire::stream::StreamState::Unconnected => {
                    state_flags.request_idle();
                }
                pipewire::stream::StreamState::Error(_) => {
                    error!("Stream error");
                }
                _ => {}
            }
        })
        .register()?;

    stream.connect(spa::utils::Direction::Input, None, flags, &mut params[..])?;

    info!("Initialized and connected audio stream: {}", log_desc);

    mainloop.run();

    drop(listener);

    Ok(())
}
