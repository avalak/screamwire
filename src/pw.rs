use pipewire::{
    context::ContextRc,
    main_loop::MainLoopRc,
    properties::properties,
    spa,
    stream::{StreamFlags, StreamRc},
};
use ringbuf::traits::Producer;

/// Virtual sink that feeds audio into the given ring buffer producer.
/// The function blocks until the main loop exits.
pub fn run_virtual_sink(
    mut producer: impl Producer<Item = u8> + Send + 'static,
    rate: u32,
    channels: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    pipewire::init();

    let mainloop = MainLoopRc::new(None)?;
    let context = ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;

    let sink_props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Playback",
        *pipewire::keys::MEDIA_ROLE => "Music",
        *pipewire::keys::NODE_NAME => "ScreamWire",
        *pipewire::keys::NODE_DESCRIPTION => "ScreamWire Sender",
        *pipewire::keys::MEDIA_CLASS => "Audio/Sink",
        *pipewire::keys::NODE_VIRTUAL => "true",
        // Application identity
        *pipewire::keys::APP_NAME => "ScreamWire",
        *pipewire::keys::APP_ID => "io.github.avalak.screamwire",
        *pipewire::keys::MEDIA_SOFTWARE => "ScreamWire",
    };

    let sink_stream = StreamRc::new(core, "screamwire-sink", sink_props)?;
    let sink_pod_data = make_format_data(rate, channels);
    let sink_pod = pipewire::spa::pod::Pod::from_bytes(&sink_pod_data).unwrap();
    let mut sink_params = [sink_pod];

    let _sink_listener = sink_stream
        .add_local_listener()
        .process(move |stream, _user_data: &mut ()| {
            if let Some(mut pw_buffer) = stream.dequeue_buffer() {
                let datas = pw_buffer.datas_mut();
                if let Some(data) = datas.first_mut() {
                    let chunk = data.chunk();
                    let offset = chunk.offset() as usize;
                    let size = chunk.size() as usize;
                    if let Some(buf) = data.data() {
                        let audio_bytes = &buf[offset..offset + size];
                        let _ = producer.push_slice(audio_bytes);
                    }
                }
            }
        })
        .state_changed(move |_stream, _user_data, _old, new| {
            if new == pipewire::stream::StreamState::Streaming {
                println!("ScreamWire sink is now streaming");
            }
        })
        .register()?;

    sink_stream.connect(
        spa::utils::Direction::Input,
        None,
        StreamFlags::MAP_BUFFERS,
        &mut sink_params[..],
    )?;

    println!("Initialized. Send audio to 'ScreamWire Sender' device.");
    mainloop.run();

    Ok(())
}

/// Build a SPA format pod for the given sample rate and channel count.
fn make_format_data(rate: u32, channels: u32) -> Vec<u8> {
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
                value: spa::pod::Value::Id(spa::utils::Id(spa::sys::SPA_AUDIO_FORMAT_S16_LE)),
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
