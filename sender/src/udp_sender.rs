use crate::vad::{Vad, VadConfig};
#[allow(unused_imports)]
use log::{debug, error, info};
use ringbuf::{
    HeapCons,
    traits::{Consumer, Observer},
};
use screamwire_common::scream::{AUDIO_PAYLOAD_SIZE, HEADER_SIZE, PACKET_SIZE, make_header};
use screamwire_common::types::AudioParams;
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

/// Network sender with Voice Activity Detection (VAD)
pub fn send_loop(
    mut consumer: HeapCons<u8>,
    target: std::net::SocketAddr,
    bind_addr: std::net::SocketAddr,
    format: AudioParams,
    vad_config: VadConfig,
) {
    let mut vad = Vad::new(vad_config, format);
    let socket = UdpSocket::bind(bind_addr).expect("Failed to bind UDP socket");

    info!("Multicast target: {}, sender bind: {}", target, bind_addr);

    let mut packet = [0u8; PACKET_SIZE];
    let header = make_header(format);
    debug!("Header: {:02X?}", header);
    packet[..HEADER_SIZE].copy_from_slice(&header);

    let mut local_payload = [0u8; AUDIO_PAYLOAD_SIZE];
    let mut should_send;
    let mut sleep_ms = 1u64;

    loop {
        if consumer.occupied_len() >= AUDIO_PAYLOAD_SIZE {
            // Non‑destructive peek into the ring buffer
            let (slice1, slice2) = consumer.as_slices();
            if slice1.len() >= AUDIO_PAYLOAD_SIZE {
                local_payload.copy_from_slice(&slice1[..AUDIO_PAYLOAD_SIZE]);
            } else {
                let first = slice1.len();
                local_payload[..first].copy_from_slice(slice1);
                local_payload[first..].copy_from_slice(&slice2[..AUDIO_PAYLOAD_SIZE - first]);
            }

            // VAD
            (should_send, sleep_ms) = vad.process(&local_payload);

            if should_send {
                packet[HEADER_SIZE..].copy_from_slice(&local_payload);
                if let Err(e) = socket.send_to(&packet, target) {
                    error!("UDP send error: {}", e);
                }
            }

            // Drain consumed data
            consumer.skip(AUDIO_PAYLOAD_SIZE);
        } else {
            thread::sleep(Duration::from_millis(sleep_ms));
        }
    }
}
