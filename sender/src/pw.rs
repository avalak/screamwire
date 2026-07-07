use crate::event_bridge::StreamEventBridge;
use crate::vad::{Vad, VadConfig};
#[allow(unused_imports)]
use log::{debug, info};
use pipewire::{
    context::ContextRc,
    init,
    main_loop::MainLoopRc,
    properties::properties,
    spa,
    stream::{StreamFlags, StreamRc},
    types::ObjectType,
};
use ringbuf::traits::Producer;
use screamwire_common::pw::make_format_data;
use screamwire_common::types::AudioParams;
use std::cell::RefCell;
use std::rc::Rc;

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
    let pod = pipewire::spa::pod::Pod::from_bytes(&pod_data).unwrap();
    let mut params = [pod];

    // Configure properties and flags based on mode
    let (props, flags, log_desc) = if let Some(ref sink_name) = target_sink {
        info!("Capture mode: using monitor of sink '{}'", sink_name);
        (
            properties! {
                *pipewire::keys::CLIENT_NAME => "ScreamWire",
                *pipewire::keys::MEDIA_NAME => "Capture audio",
                *pipewire::keys::MEDIA_TYPE => "Audio",
                *pipewire::keys::MEDIA_CATEGORY => "Manager", //"Capture",
                *pipewire::keys::MEDIA_ROLE => "Production",
                *pipewire::keys::STREAM_CAPTURE_SINK => "true",
                *pipewire::keys::TARGET_OBJECT => sink_name.as_str(),
                *pipewire::keys::NODE_DESCRIPTION => "ScreamWire Sender",
                *pipewire::keys::APP_ICON_NAME => "audio-speakers",
                *pipewire::keys::APP_NAME => "ScreamWire",
                *pipewire::keys::APP_ID => "io.github.avalak.screamwire",
                *pipewire::keys::MEDIA_SOFTWARE => "ScreamWire",
            },
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            format!("capture from '{}'", sink_name),
        )
    } else {
        info!("Virtual mode: creating 'ScreamWire' output device");
        (
            properties! {
                *pipewire::keys::MEDIA_TYPE => "Audio",
                *pipewire::keys::MEDIA_CATEGORY => "Playback",
                *pipewire::keys::MEDIA_ROLE => "Production",
                *pipewire::keys::NODE_NAME => "ScreamWire",
                *pipewire::keys::NODE_DESCRIPTION => "ScreamWire Remote Output",
                *pipewire::keys::MEDIA_CLASS => "Audio/Sink",
                *pipewire::keys::NODE_VIRTUAL => "true",

                *pipewire::keys::APP_NAME => "ScreamWire",
                *pipewire::keys::APP_ID => "io.github.avalak.screamwire",
                *pipewire::keys::MEDIA_SOFTWARE => "ScreamWire",
            },
            StreamFlags::MAP_BUFFERS,
            "virtual sink 'ScreamWire'".to_string(),
        )
    };

    // VAD
    let mut vad = Vad::new(vad_config, format);

    let stream = StreamRc::new(core.clone(), "screamwire-stream", props)?;
    let log_desc_for_closure = log_desc.clone();

    let event_bridge_clone = event_bridge.clone();
    let _listener = stream
        .add_local_listener::<()>()
        .process(move |s, _| {
            if let Some(mut buf) = s.dequeue_buffer() {
                let datas = buf.datas_mut();
                if let Some(data) = datas.first_mut() {
                    let chunk = data.chunk();
                    let off = chunk.offset() as usize;
                    let sz = chunk.size() as usize;
                    if let Some(bytes) = data.data() {
                        let raw_audio = &bytes[off..off + sz];

                        if vad.process(raw_audio) {
                            let _ = producer.push_slice(raw_audio);
                            event_bridge_clone.notify_data_ready();
                        }
                        info!("got data");
                    }
                }
            }
        })
        .state_changed(move |_stream, _user_data, old, new| {
            debug!(
                "Stream state changed from {:?} to {:?} ({})",
                old, new, log_desc_for_closure
            );
            if new == pipewire::stream::StreamState::Streaming {
                info!("Stream started ({})", log_desc_for_closure);
                // TODO: handle stream change event
            }
        })
        .register()?;

    stream.connect(spa::utils::Direction::Input, None, flags, &mut params[..])?;

    info!("Initialized: {}", log_desc);
    mainloop.run();

    Ok(())
}
