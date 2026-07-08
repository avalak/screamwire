use crate::event_bridge::StreamEventBridge;
#[allow(unused_imports)]
use log::{debug, error, info};
use ringbuf::traits::{Consumer, Observer};
use screamwire_common::scream::{AUDIO_PAYLOAD_SIZE, HEADER_SIZE, PACKET_SIZE, make_header};
use screamwire_common::types::AudioParams;
use std::net::UdpSocket;

/// Network sender with Voice Activity Detection (VAD)
pub fn send_loop(
    mut consumer: ringbuf::HeapCons<u8>,
    target: std::net::SocketAddr,
    bind_addr: std::net::SocketAddr,
    format: AudioParams,
    event_bridge: StreamEventBridge,
) {
    let socket = UdpSocket::bind(bind_addr).expect("Failed to bind UDP socket");

    info!("Multicast target: {}, sender bind: {}", target, bind_addr);

    let mut packet = [0u8; PACKET_SIZE];
    let header = make_header(format);
    packet[..HEADER_SIZE].copy_from_slice(&header);

    loop {
        event_bridge.wait_for_data();
        debug!("event: Data chunk ready");

        if event_bridge.swap_flush_requested() {
            consumer.clear();
            debug!("Send loop: Flush requested. Ringbuffer cleared.");
            continue;
        }

        while consumer.occupied_len() >= AUDIO_PAYLOAD_SIZE {
            let bytes_read =
                consumer.pop_slice(&mut packet[HEADER_SIZE..HEADER_SIZE + AUDIO_PAYLOAD_SIZE]);

            if bytes_read != AUDIO_PAYLOAD_SIZE {
                error!(
                    "Ringbuffer error: Expected to read {} bytes, but got {}",
                    AUDIO_PAYLOAD_SIZE, bytes_read
                );
                continue;
            }

            if let Err(e) = socket.send_to(&packet, target) {
                error!("UDP send error: {}", e);
            }
        }
    }
}
