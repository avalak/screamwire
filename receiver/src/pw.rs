use log::{debug, info};
use pipewire::{
    context::ContextRc,
    main_loop::MainLoopRc,
    properties::properties,
    spa,
    stream::{StreamFlags, StreamRc},
};
use ringbuf::traits::Consumer;
use screamwire_common::pw::{make_buffers_data, make_format_data};
use screamwire_common::types::AudioParams;
use std::sync::mpsc;

pub fn run_playback_stream(
    mut consumer: impl Consumer<Item = u8> + Send + 'static,
    format: AudioParams,
    tx_bridge: mpsc::Sender<pipewire::channel::Sender<()>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mainloop = MainLoopRc::new(None)?;
    let context = ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;

    let props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Playback",
        *pipewire::keys::MEDIA_ROLE => "Music",
        *pipewire::keys::MEDIA_CLASS => "Stream/Output/Audio",
        *pipewire::keys::NODE_NAME => "ScreamWireReceiver",
        *pipewire::keys::NODE_DESCRIPTION => "ScreamWire Receiver",
        *pipewire::keys::APP_NAME => "ScreamWire",
        *pipewire::keys::APP_ID => "io.github.avalak.screamwire",
        *pipewire::keys::MEDIA_SOFTWARE => "ScreamWire",
        *pipewire::keys::NODE_AUTOCONNECT => "true",
        *pipewire::keys::TARGET_OBJECT => "default.audio.sink",
        *pipewire::keys::NODE_LATENCY => format!("{}/{}", 288, format.rate),
    };

    let stream = StreamRc::new(core, "screamwire-receiver", props)?;
    let frame_size = format.frame_bytes();

    let _listener = stream
        .add_local_listener::<()>()
        .process(move |s, _| {
            if let Some(mut buf) = s.dequeue_buffer() {
                let datas = buf.datas_mut();
                if let Some(data) = datas.first_mut()
                    && let Some(bytes) = data.data()
                {
                    let max_size = bytes.len();
                    let dst = &mut bytes[0..max_size];
                    let available = consumer.occupied_len();

                    let safe_available = (available / frame_size) * frame_size;

                    if safe_available >= max_size {
                        consumer.pop_slice(dst);
                    } else {
                        if safe_available > 0 {
                            consumer.pop_slice(&mut dst[..safe_available]);
                        }
                        dst[safe_available..max_size].fill(0);
                    }

                    let chunk = data.chunk_mut();
                    *chunk.offset_mut() = 0;
                    *chunk.size_mut() = max_size as u32;
                    *chunk.stride_mut() = 0;
                }
            }
        })
        .state_changed(move |_stream, _user_data, old, new| {
            debug!("Stream state changed from {:?} to {:?}", old, new);
            if new == pipewire::stream::StreamState::Streaming {
                info!("Playback stream started and streaming");
            }
        })
        .register()?;

    let pod_data = make_format_data(format);
    let pod = pipewire::spa::pod::Pod::from_bytes(&pod_data).unwrap();

    let buffers_data = make_buffers_data();
    let buffers_param = pipewire::spa::pod::Pod::from_bytes(&buffers_data).unwrap();

    let mut params_refs: [&pipewire::spa::pod::Pod; 2] = [pod, buffers_param];

    stream.connect(
        spa::utils::Direction::Output,
        None,
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
        &mut params_refs[..],
    )?;

    // Create a PipeWire eventfd channel and attach it to the mainloop.
    let (pw_tx, pw_rx) = pipewire::channel::channel::<()>();
    let mainloop_clone = mainloop.clone();
    let _receiver = pw_rx.attach(mainloop.loop_(), move |_| {
        mainloop_clone.quit();
    });

    // Hand the sender to the network thread via the provided bridge
    let _ = tx_bridge.send(pw_tx);

    info!("Initialized receiver - waiting for audio packets...");
    mainloop.run(); // Deep sleep

    info!("Playback stream stopped");
    Ok(())
}
