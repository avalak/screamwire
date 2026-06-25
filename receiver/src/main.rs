use clap::Parser;
use log::{debug, error, info, warn};
use ringbuf::{
    HeapRb,
    traits::{Producer, Split},
};
use screamwire_common::scream::{
    AUDIO_PAYLOAD_SIZE, HEADER_SIZE, PACKET_SIZE, default_target_addr, make_header,
};
use screamwire_common::types::{AudioParams, DEFAULT_BITS, DEFAULT_CHANNELS, DEFAULT_RATE};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::thread;
use std::{collections::HashSet, str::FromStr};

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

    // Ring buffer
    let buffer_size = PACKET_SIZE * cli.buffer_size as usize;
    let rb = HeapRb::<u8>::new(buffer_size);
    let (mut producer, consumer) = rb.split();

    // TODO: fix hardcoded values ASAP
    let format = AudioParams {
        rate: DEFAULT_RATE,
        bits: DEFAULT_BITS,
        channels: DEFAULT_CHANNELS,
    };
    let expected_header = make_header(format);

    let mut known_senders = HashSet::new();

    let _receiver_thread = thread::spawn(move || {
        let mut buf = [0u8; PACKET_SIZE];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, addr)) if n == PACKET_SIZE => {
                    if known_senders.insert(addr) {
                        info!("New sender detected: {}", addr);
                    }
                    if buf[..HEADER_SIZE] == expected_header {
                        producer.push_slice(&buf[HEADER_SIZE..HEADER_SIZE + AUDIO_PAYLOAD_SIZE]);
                        //debug!("Valid packet from {} pushed to buffer", addr);
                    } else {
                        warn!("Invalid Scream header from {}", addr);
                    }
                }
                Ok((n, addr)) => warn!("Short packet ({} bytes) from {}", n, addr),
                Err(e) => error!("UDP recv error: {}", e),
            }
        }
    });

    pw::run_playback_stream(consumer, format)?;

    Ok(())
}
