use crate::event_bridge::StreamEventBridge;
//use crate::vad::VadConfig;
#[allow(unused_imports)]
use log::{debug, error, info};
use ringbuf::{
    //    HeapCons,
    traits::{Consumer, Observer},
};
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
        debug!("Awaken");

        while consumer.occupied_len() >= AUDIO_PAYLOAD_SIZE {
            let (slice1, slice2) = consumer.as_slices();

            if slice1.len() >= AUDIO_PAYLOAD_SIZE {
                packet[HEADER_SIZE..].copy_from_slice(&slice1[..AUDIO_PAYLOAD_SIZE]);
            } else {
                let first = slice1.len();
                packet[HEADER_SIZE..HEADER_SIZE + first].copy_from_slice(slice1);
                packet[HEADER_SIZE + first..HEADER_SIZE + AUDIO_PAYLOAD_SIZE]
                    .copy_from_slice(&slice2[..AUDIO_PAYLOAD_SIZE - first]);
            };

            if let Err(e) = socket.send_to(&packet, target) {
                error!("UDP send error: {}", e);
            }

            consumer.skip(AUDIO_PAYLOAD_SIZE);
        }
    }
}
