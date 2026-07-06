use clap::Parser;
use log::{debug, error, info, warn};
use ringbuf::{
    HeapRb,
    traits::{Producer, Split},
};
use screamwire_common::scream::{
    AUDIO_PAYLOAD_SIZE, HEADER_SIZE, PACKET_SIZE, default_target_addr, parse_header,
};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

mod cli;
mod pw;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = cli::Cli::parse();

    // Initialize logger
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    debug!("Verbose mode enabled");

    let listen_addr = SocketAddrV4::from_str("0.0.0.0:4010")?;
    let multicast_addr = SocketAddrV4::from_str(default_target_addr().as_str())?;

    info!("ScreamWire receiver starting...");
    info!("Listening on multicast {}", multicast_addr);

    let socket = UdpSocket::bind(listen_addr).expect("Failed to bind UDP socket");
    socket
        .join_multicast_v4(multicast_addr.ip(), &Ipv4Addr::new(0, 0, 0, 0))
        .unwrap();

    debug!("Buffer size: {}", cli.buffer_size);
    // One-time PipeWire initialization
    pipewire::init();

    // Clone the socket once for the worker thread
    let socket_worker = socket.try_clone().expect("Failed to clone socket");

    thread::spawn(move || {
        loop {
            info!("Waiting for initial audio packet...");

            // Switch to blocking mode while waiting for the very first packet
            if let Err(e) = socket_worker.set_read_timeout(None) {
                error!("Failed to set socket blocking: {}", e);
            }

            // Wait for first packet (format detection)
            let (initial_format, initial_payload) = 'first: loop {
                let mut buf = [0u8; PACKET_SIZE];
                match socket_worker.recv_from(&mut buf) {
                    Ok((n, addr)) if n == PACKET_SIZE => {
                        let header: [u8; HEADER_SIZE] = buf[..HEADER_SIZE].try_into().unwrap();
                        if let Some(format) = parse_header(&header) {
                            info!("Initial format from {}: {:?}", addr, format);
                            let mut payload = [0u8; AUDIO_PAYLOAD_SIZE];
                            payload.copy_from_slice(
                                &buf[HEADER_SIZE..HEADER_SIZE + AUDIO_PAYLOAD_SIZE],
                            );
                            break 'first (format, payload);
                        } else {
                            warn!("Invalid header from {}, waiting...", addr);
                        }
                    }
                    Ok((n, addr)) => warn!("Short packet ({} bytes) from {}, waiting...", n, addr),
                    Err(e) => error!("UDP recv error: {}, waiting...", e),
                }
            };

            // Ring buffer
            let buffer_size = PACKET_SIZE * cli.buffer_size as usize;
            let rb = HeapRb::<u8>::new(buffer_size);
            let (mut producer, consumer) = rb.split();
            let _ = producer.push_slice(&initial_payload);

            // Channel to forward the PipeWire eventfd sender to the network thread
            let (tx_bridge, rx_bridge) = mpsc::channel::<pipewire::channel::Sender<()>>();

            let stop_flag = std::sync::Arc::new(AtomicBool::new(false));
            let stop_flag_sender = stop_flag.clone();

            // Hand the producer to the network thread. It will be safely dropped when the session ends.
            let mut network_producer = Some(producer);
            let socket_clone = socket_worker.try_clone().expect("Failed to clone socket");

            let receiver_thread = thread::spawn(move || {
                let mut buf = [0u8; PACKET_SIZE];
                let mut last_packet = Instant::now();
                let watchdog_timeout = Duration::from_secs(5);

                let mut local_producer = network_producer.take().unwrap();

                socket_clone
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .expect("Failed to set read timeout");

                loop {
                    if stop_flag_sender.load(Ordering::Relaxed) {
                        break;
                    }
                    match socket_clone.recv_from(&mut buf) {
                        Ok((n, _addr)) if n == PACKET_SIZE => {
                            last_packet = Instant::now();
                            let _ = local_producer
                                .push_slice(&buf[HEADER_SIZE..HEADER_SIZE + AUDIO_PAYLOAD_SIZE]);
                        }
                        Ok((n, _addr)) => warn!("Short packet ({} bytes)", n),
                        Err(ref e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            if last_packet.elapsed() >= watchdog_timeout {
                                info!("Watchdog timeout – stopping stream");
                                stop_flag_sender.store(true, Ordering::Relaxed);

                                // Tear down PipeWire (eventfd channel)
                                if let Ok(pw_sender) = rx_bridge.try_recv() {
                                    let _ = pw_sender.send(());
                                }
                                break;
                            }
                        }
                        Err(e) => {
                            error!("UDP recv error: {}", e);
                            break;
                        }
                    }
                }
                info!("Receiver thread finished");
            });

            info!("Starting playback stream...");

            // Start playback. The consumer is moved and dies with the stream.
            if let Err(e) = pw::run_playback_stream(consumer, initial_format, tx_bridge) {
                error!("Playback stream error: {}", e);
            }

            // Ensure thread finished
            let _ = receiver_thread.join();

            info!("Stream ended, waiting for next source...");

            // Short pause for finishing
            thread::sleep(Duration::from_millis(100));
        }
    });

    std::thread::park();
    Ok(())
}
