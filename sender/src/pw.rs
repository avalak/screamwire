use crate::dispatch_vad;
use crate::event_bridge::StreamEventBridge;
use crate::rt_debug;

use crate::vad::{DynamicVad, Vad, VadConfig, VadDisabled};
use log::info;
use pipewire::{
    context::ContextRc,
    init,
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

/// Build stream properties, flags and a human-readable description.
/// Properties are grouped hierarchically by operational priority and impact.
#[inline]
fn stream_config(sink_name: Option<&str>) -> (PropertiesBox, StreamFlags, String) {
    let mut props = PropertiesBox::new();

    // Common properties
    props.insert(*pipewire::keys::NODE_ALWAYS_PROCESS, "false");
    props.insert(*pipewire::keys::APP_NAME, "ScreamWire");
    props.insert(*pipewire::keys::APP_ID, "io.github.avalak.screamwire");
    props.insert(*pipewire::keys::MEDIA_SOFTWARE, "ScreamWire");
    props.insert(*pipewire::keys::NODE_DESCRIPTION, "ScreamWire Sender");
    props.insert(*pipewire::keys::MEDIA_TYPE, "Audio");
    props.insert(*pipewire::keys::MEDIA_ROLE, "Production");

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
pub fn get_sink_names() -> Vec<String> {
    init();

    let mainloop = MainLoopRc::new(None).expect("Failed to create main loop");
    let context = ContextRc::new(&mainloop, None).expect("Failed to create context");
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
pub fn run_audio_stream(
    mut producer: impl Producer<Item = u8> + Send + 'static,
    format: AudioParams,
    target_sink: Option<String>,
    vad_config: VadConfig,
    event_bridge: StreamEventBridge,
) -> Result<(), Box<dyn std::error::Error>> {
    init();

    let mainloop = MainLoopRc::new(None)?;
    let context = ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;

    // SPA format pod
    let pod_data = make_format_data(format);
    let pod = spa::pod::Pod::from_bytes(&pod_data).unwrap();
    let mut params = [pod];

    // Configure properties and flags based on mode
    let (props, flags, log_desc) = stream_config(target_sink.as_deref());

    // Monomorphize VAD
    let mut vad = if vad_config.threshold == 0 || vad_config.max_silence_bytes == 0 {
        DynamicVad::Disabled(VadDisabled::new(vad_config))
    } else {
        match format.bits {
            8 => DynamicVad::Bits8(Vad::new(vad_config)),
            16 => DynamicVad::Bits16(Vad::new(vad_config)),
            24 => DynamicVad::Bits24(Vad::new(vad_config)),
            _ => DynamicVad::Bits32(Vad::new(vad_config)),
        }
    };

    let needs_reset = Arc::new(AtomicBool::new(false));
    let needs_force_idle = Arc::new(AtomicBool::new(false));

    let process_reset = Arc::clone(&needs_reset);
    let state_reset = needs_reset;

    let process_force_idle = Arc::clone(&needs_force_idle);
    let state_force_idle = needs_force_idle;

    let _log_desc_for_closure = log_desc.clone();

    let process_bridge = event_bridge.clone();
    let _state_bridge = event_bridge;

    let stream = StreamRc::new(core.clone(), "screamwire-stream", props)?;
    let listener = stream
        .add_local_listener::<()>()
        .process(move |s, _| {
            if process_reset.swap(false, Ordering::Acquire) {
                dispatch_vad!(&mut vad, |v| v.reset());
            }
            if process_force_idle.swap(false, Ordering::Acquire) {
                dispatch_vad!(&mut vad, |v| v.force_idle());
                process_bridge.notify_flush();
            }

            if let Some(mut buf) = s.dequeue_buffer() {
                let datas = buf.datas_mut();
                if let Some(data) = datas.first_mut() {
                    let chunk = data.chunk();
                    let off = chunk.offset() as usize;
                    let sz = chunk.size() as usize;
                    if let Some(bytes) = data.data() {
                        let raw_audio = &bytes[off..off + sz];

                        // Monomorphized call
                        let is_active = dispatch_vad!(&mut vad, |v| v.process(raw_audio));

                        if is_active {
                            let _ = producer.push_slice(raw_audio);
                            process_bridge.notify_data_ready();
                        }
                    }
                }
            }
        })
        .state_changed(move |_stream, _user_data, _old, new| {
            rt_debug!(
                "Stream state: {:?} -> {:?} ({})",
                _old,
                new,
                _log_desc_for_closure
            );
            match new {
                pipewire::stream::StreamState::Streaming => {
                    rt_debug!(
                        "Stream started ({}) -> Requesting VAD Reset",
                        _log_desc_for_closure
                    );
                    state_reset.store(true, Ordering::Release);
                }
                pipewire::stream::StreamState::Paused
                | pipewire::stream::StreamState::Unconnected => {
                    rt_debug!(
                        "Stream stopped/paused ({}) -> Requesting VAD Idle",
                        _log_desc_for_closure
                    );
                    state_force_idle.store(true, Ordering::Release);
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
