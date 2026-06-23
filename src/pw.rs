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

    // Shared vector wrapped in Rc<RefCell<…>> because it is written from
    // the registry callback and read after the loop.
    let sinks = Rc::new(RefCell::new(Vec::new()));
    let sinks_clone = sinks.clone();

    // Listen for global objects – collect every Audio/Sink that appears.
    let registry_listener = registry
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

    // Request synchronisation and keep the returned sequence number.
    let sync_seq = core.sync(0).expect("Failed to sync core");

    // When the server has processed our sync request it will emit a `done`
    // event with the same sequence number -> we can quit the loop.
    let mainloop_clone = mainloop.clone();
    let core_listener = core
        .add_listener_local()
        .done(move |id, seq| {
            if id == pipewire::core::PW_ID_CORE && seq == sync_seq {
                mainloop_clone.quit();
            }
        })
        .register();

    mainloop.run();

    // Keep the listeners alive until here.
    drop(registry_listener);
    drop(core_listener);

    // Extract the vector – the Rc and RefCell are no longer needed.
    Rc::into_inner(sinks)
        .expect("There are remaining Rc references")
        .into_inner()
}

/// Universal audio stream runner.
///
/// * `target_sink = Some(name)` -> capture from the monitor of an existing sink.
/// * `target_sink = None`       -> create a virtual "ScreamWire" output device.
pub fn run_audio_stream(
    mut producer: impl Producer<Item = u8> + Send + 'static,
    rate: u32,
    bits: u32,
    channels: u32,
    target_sink: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    init();

    let mainloop = MainLoopRc::new(None)?;
    let context = ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;

    // SPA format pod
    let pod_data = make_format_data(rate, bits, channels);
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

    let stream = StreamRc::new(core.clone(), "screamwire-stream", props)?;
    let log_desc_for_closure = log_desc.clone();
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
                        let _ = producer.push_slice(&bytes[off..off + sz]);
                    }
                }
            }
        })
        .state_changed(move |_stream, _user_data, _old, new| {
            if new == pipewire::stream::StreamState::Streaming {
                debug!("Stream started ({})", log_desc_for_closure);
            }
        })
        .register()?;

    stream.connect(spa::utils::Direction::Input, None, flags, &mut params[..])?;

    info!("Initialized: {}", log_desc);
    mainloop.run();

    Ok(())
}

/// Build a SPA format pod for the given sample rate, bit depth and channel count.
fn make_format_data(rate: u32, bits: u32, channels: u32) -> Vec<u8> {
    let audio_format = match bits {
        16 => spa::sys::SPA_AUDIO_FORMAT_S16_LE,
        24 => spa::sys::SPA_AUDIO_FORMAT_S24_LE,
        32 => spa::sys::SPA_AUDIO_FORMAT_S32_LE,
        _ => panic!("Unsupported bit depth: {}", bits),
    };

    let obj = spa::pod::Object {
        type_: spa::sys::SPA_TYPE_OBJECT_Format,
        id: spa::sys::SPA_PARAM_EnumFormat,
        properties: vec![
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_mediaType,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Id(spa::utils::Id(spa::sys::SPA_MEDIA_TYPE_audio)),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_mediaSubtype,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Id(spa::utils::Id(spa::sys::SPA_MEDIA_SUBTYPE_raw)),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_AUDIO_format,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Id(spa::utils::Id(audio_format)),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_AUDIO_rate,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Int(rate as i32),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_AUDIO_channels,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Int(channels as i32),
            },
        ],
    };

    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner()
}
