#[allow(unused_imports)]
use log::{debug, error, info};
use ringbuf::{
    HeapCons,
    traits::{Consumer, Observer},
};
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

use crate::vad::{Vad, VadConfig};

// Scream protocol constants
pub const BITS: u32 = 16;
pub const RATE: u32 = 48000;
pub const CHANNELS: u32 = 2;

pub const HEADER_SIZE: usize = 5;
pub const AUDIO_PAYLOAD_SIZE: usize = 1152;
pub const PACKET_SIZE: usize = HEADER_SIZE + AUDIO_PAYLOAD_SIZE;

#[derive(Debug, Clone, Copy)]
pub struct AudioParams {
    pub rate: u32,
    pub bits: u32,
    pub channels: u32,
}

impl AudioParams {
    /// bytes per audio frame (bits/8 * channels)
    pub fn frame_bytes(&self) -> usize {
        (self.bits as usize / 8) * self.channels as usize
    }
}

/// Return the Windows speaker mask for the given number of channels.
/// NOTE: Should work for common setups
fn channel_map(channels: u32) -> u16 {
    match channels {
        1 => 0x0001,                 // FL
        2 => 0x0003,                 // FL | FR
        3 => 0x0007,                 // FL | FR | FC
        4 => 0x0033,                 // FL | FR | BL | BR (quad)
        5 => 0x003F,                 // FL | FR | FC | BL | BR (5.0)
        6 => 0x060F,                 // FL | FR | FC | LFE | BL | BR (5.1)
        7 => 0x06FF,                 // 7.0 (non‑standard extension)
        8 => 0x00FF,                 // 7.1 (first 8 bits)
        _ => (1u16 << channels) - 1, // fallback for other counts
    }
}

/// Build a Scream packet header (5 bytes) from audio format parameters.
///
/// Docs: [Packet format](https://github.com/duncanthrax/scream/blob/master/tools/wireshark/README.md#packet-format).
pub fn make_header(format: AudioParams) -> [u8; HEADER_SIZE] {
    let (base, multiplier) = if format.rate.is_multiple_of(44100) {
        (44100, format.rate / 44100)
    } else {
        (48000, format.rate / 48000)
    };
    assert!(
        multiplier > 0 && multiplier <= 127,
        "Unsupported sample rate: {}",
        format.rate
    );

    let sample_rate_code = if base == 44100 {
        0x80 | (multiplier as u8)
    } else {
        multiplier as u8
    };

    let sample_size = format.bits as u8;
    assert!(
        (1..=8).contains(&format.channels),
        "Unsupported channel count: {}",
        format.channels
    );

    let map = channel_map(format.channels);

    [
        sample_rate_code,
        sample_size,
        format.channels as u8,
        map as u8,
        (map >> 8) as u8,
    ]
}

/// Network sender with Voice Activity Detection (VAD)
pub fn send_loop(
    mut consumer: HeapCons<u8>,
    target: std::net::SocketAddr,
    bind_addr: std::net::SocketAddr,
    format: AudioParams,
    vad_config: VadConfig,
) {
    let mut vad = Vad::new(vad_config, format.frame_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_header() {
        let format = AudioParams {
            rate: 48000,
            bits: 16,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x01, 0x10, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_44100_stereo() {
        let format = AudioParams {
            rate: 44100,
            bits: 16,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x81, 0x10, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_96000_stereo() {
        let format = AudioParams {
            rate: 96000,
            bits: 16,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x02, 0x10, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_88200_stereo() {
        let format = AudioParams {
            rate: 88200,
            bits: 16,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x82, 0x10, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_192000_stereo() {
        let format = AudioParams {
            rate: 192000,
            bits: 16,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x04, 0x10, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_176400_stereo() {
        let format = AudioParams {
            rate: 176400,
            bits: 16,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x84, 0x10, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_48000_mono() {
        let format = AudioParams {
            rate: 48000,
            bits: 16,
            channels: 1,
        };
        let header = make_header(format);
        assert_eq!(header, [0x01, 0x10, 0x01, 0x01, 0x00]);
    }

    #[test]
    fn test_44100_24bit() {
        let format = AudioParams {
            rate: 44100,
            bits: 24,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x81, 0x18, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_48000_32bit() {
        let format = AudioParams {
            rate: 48000,
            bits: 32,
            channels: 2,
        };
        let header = make_header(format);
        assert_eq!(header, [0x01, 0x20, 0x02, 0x03, 0x00]);
    }

    #[test]
    fn test_channel_map_1ch() {
        assert_eq!(channel_map(1), 0x0001);
    }

    #[test]
    fn test_channel_map_2ch() {
        assert_eq!(channel_map(2), 0x0003);
    }

    #[test]
    fn test_channel_map_6ch() {
        assert_eq!(channel_map(6), 0x060F);
    }

    #[test]
    fn test_channel_map_8ch() {
        assert_eq!(channel_map(8), 0x00FF);
    }

    #[test]
    fn test_channel_map_4ch_quad() {
        assert_eq!(channel_map(4), 0x0033);
    }

    #[test]
    fn test_channel_map_custom() {
        assert_eq!(channel_map(3), 0x0007);
    }

    #[test]
    fn test_channel_map_6ch_in_header() {
        let format = AudioParams {
            rate: 48000,
            bits: 16,
            channels: 6,
        };
        let header = make_header(format);
        assert_eq!(header, [0x01, 0x10, 0x06, 0x0F, 0x06]);
    }

    #[test]
    fn test_frame_bytes() {
        let format = AudioParams {
            rate: 48000,
            bits: 16,
            channels: 2,
        };
        assert_eq!(format.frame_bytes(), 4);
        let format = AudioParams {
            rate: 48000,
            bits: 24,
            channels: 2,
        };
        assert_eq!(format.frame_bytes(), 6);
        let format = AudioParams {
            rate: 48000,
            bits: 32,
            channels: 1,
        };
        assert_eq!(format.frame_bytes(), 4);
    }
}
