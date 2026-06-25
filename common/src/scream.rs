//! Scream protocol constants, header builder, and channel map.

use super::types::AudioParams;

/// Default multicast group address (IPv4).
pub const DEFAULT_MULTICAST_IP: &str = "239.255.77.77";

/// Default UDP port used by the Scream protocol.
pub const DEFAULT_SCREAM_PORT: u16 = 4010;

// Scream protocol related
pub const HEADER_SIZE: usize = 5;
pub const AUDIO_PAYLOAD_SIZE: usize = 1152;
pub const PACKET_SIZE: usize = HEADER_SIZE + AUDIO_PAYLOAD_SIZE;

/// Build the default target address string from the IP and port.
pub fn default_target_addr() -> String {
    format!("{}:{}", DEFAULT_MULTICAST_IP, DEFAULT_SCREAM_PORT)
}

/// Return the Windows speaker mask for the given number of channels.
/// NOTE: Should work for common setups
pub fn channel_map(channels: u32) -> u16 {
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
