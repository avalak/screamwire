use clap::Parser;
use log::{debug, error, info, warn};
use ringbuf::{
    HeapRb,
    traits::{Producer, Split},
};
use screamwire_common::scream::{
    AUDIO_PAYLOAD_SIZE, HEADER_SIZE, PACKET_SIZE, default_target_addr, parse_header,
};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::str::FromStr;
use std::thread;

mod cli;
mod pw;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // Wait for first packet (format detection)
    let (initial_format, initial_payload) = loop {
        let mut buf = [0u8; PACKET_SIZE];
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) if n == PACKET_SIZE => {
                let header: [u8; HEADER_SIZE] = buf[..HEADER_SIZE].try_into().unwrap();
                if let Some(format) = parse_header(&header) {
                    info!("Initial format from {}: {:?}", addr, format);
                    let payload: [u8; AUDIO_PAYLOAD_SIZE] = buf
                        [HEADER_SIZE..HEADER_SIZE + AUDIO_PAYLOAD_SIZE]
                        .try_into()
                        .unwrap();
                    break (format, payload);
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
    producer.push_slice(&initial_payload);

    // Cached headers for each sender address
    let mut headers: HashMap<std::net::SocketAddr, [u8; HEADER_SIZE]> = HashMap::new();
    let mut current_format = initial_format;

    // Reuse socket
    let socket_clone = socket.try_clone().unwrap();
    let _receiver_thread = thread::spawn(move || {
        let mut buf = [0u8; PACKET_SIZE];
        loop {
            match socket_clone.recv_from(&mut buf) {
                Ok((n, addr)) if n == PACKET_SIZE => {
                    let header: [u8; HEADER_SIZE] = buf[..HEADER_SIZE].try_into().unwrap();
                    let changed = match headers.get(&addr) {
                        Some(old) => *old != header,
                        None => true,
                    };
                    if changed {
                        info!("New or changed header from {}: {:02X?}", addr, header);
                        headers.insert(addr, header);
                        if let Some(new_format) = parse_header(&header) {
                            if new_format != current_format {
                                debug!(
                                    "{}:{} changed format to {:?}",
                                    addr.ip(),
                                    addr.port(),
                                    new_format
                                );
                                info!("Audio format changed, restarting stream...");
                                // TODO: stop current stream and start a new one
                                current_format = new_format;
                            }
                        } else {
                            warn!("Failed to parse header, keeping current format");
                        }
                    }
                    producer.push_slice(&buf[HEADER_SIZE..HEADER_SIZE + AUDIO_PAYLOAD_SIZE]);
                }
                Ok((n, addr)) => warn!("Short packet ({} bytes) from {}", n, addr),
                Err(e) => error!("UDP recv error: {}", e),
            }
        }
    });

    pw::run_playback_stream(consumer, current_format)?;

    Ok(())
}
