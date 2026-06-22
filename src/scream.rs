#[allow(unused_imports)]
use log::{error, info};
use ringbuf::{
    HeapCons,
    traits::{Consumer, Observer},
};
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

use crate::vad::Vad;

// Scream protocol constants
pub const RATE: u32 = 48000;
pub const CHANNELS: u32 = 2;
pub const FRAME_BYTES: usize = 2 * CHANNELS as usize; // 4

pub const HEADER_SIZE: usize = 5;
pub const AUDIO_PAYLOAD_SIZE: usize = 1152;
pub const PACKET_SIZE: usize = HEADER_SIZE + AUDIO_PAYLOAD_SIZE;

pub const HEADER: [u8; HEADER_SIZE] = [0x01, 0x10, 0x02, 0x03, 0x00];

/// Network sender with Voice Activity Detection (VAD)
pub fn send_loop(
    mut consumer: HeapCons<u8>,
    target: std::net::SocketAddr,
    bind_addr: std::net::SocketAddr,
    vad_threshold: u16,
    silence_packets: u32,
    active_sleep_ms: u64,
    idle_sleep_ms: u64,
) {
    let mut vad = Vad::new(
        vad_threshold,
        silence_packets,
        active_sleep_ms,
        idle_sleep_ms,
    );
    let socket = UdpSocket::bind(bind_addr).expect("Failed to bind UDP socket");

    info!("Multicast target: {}, sender bind: {}", target, bind_addr);

    let mut packet = [0u8; PACKET_SIZE];
    packet[..HEADER_SIZE].copy_from_slice(&HEADER);

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
