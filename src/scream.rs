use ringbuf::{
    HeapCons,
    traits::{Consumer, Observer},
};
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

// Scream protocol constants
pub const RATE: u32 = 48000;
pub const CHANNELS: u32 = 2;

pub const HEADER_SIZE: usize = 5;
pub const AUDIO_PAYLOAD_SIZE: usize = 1152;
pub const PACKET_SIZE: usize = HEADER_SIZE + AUDIO_PAYLOAD_SIZE;

pub const HEADER: [u8; HEADER_SIZE] = [0x01, 0x10, 0x02, 0x03, 0x00];

/// Network sender thread – reads audio from the ring buffer and sends multicast UDP packets.
pub fn send_loop(
    mut consumer: HeapCons<u8>,
    target: std::net::SocketAddr,
    bind_addr: std::net::SocketAddr,
) {
    let socket = UdpSocket::bind(bind_addr).expect("Failed to bind UDP socket");
    let mut packet = [0u8; PACKET_SIZE];
    packet[..HEADER_SIZE].copy_from_slice(&HEADER);

    loop {
        if consumer.occupied_len() >= AUDIO_PAYLOAD_SIZE {
            let (slice1, slice2) = consumer.as_slices();
            if slice1.len() >= AUDIO_PAYLOAD_SIZE {
                packet[HEADER_SIZE..].copy_from_slice(&slice1[..AUDIO_PAYLOAD_SIZE]);
            } else {
                let first = slice1.len();
                packet[HEADER_SIZE..HEADER_SIZE + first].copy_from_slice(slice1);
                let remaining = AUDIO_PAYLOAD_SIZE - first;
                packet[HEADER_SIZE + first..].copy_from_slice(&slice2[..remaining]);
            }

            if let Err(e) = socket.send_to(&packet, target) {
                eprintln!("UDP send error: {}", e);
            }

            consumer.skip(AUDIO_PAYLOAD_SIZE);
        } else {
            // Polling interval
            thread::sleep(Duration::from_millis(1));
        }
    }
}
