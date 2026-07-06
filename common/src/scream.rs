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

/// Static lookup table for the Windows speaker masks (Scream spec).
/// Array indices strictly map to the channel count (from 0 to 8).
const LAZY_CHANNEL_MAPS: [u16; 9] = [
    0x0000, // 0 channels (invalid fallback)
    0x0001, // 1 ch: Front Left
    0x0003, // 2 ch: Front Left | Front Right (Stereo)
    0x0007, // 3 ch: Front Left | Front Right | Front Center
    0x0033, // 4 ch: Quadraphonic
    0x003F, // 5 ch: 5.0 Surround
    0x060F, // 6 ch: 5.1 Surround
    0x06FF, // 7 ch: 7.0 Layout
    0x00FF, // 8 ch: 7.1 Surround
];

/// Returns the Windows speaker mask based on channel count without runtime branching.
#[inline]
pub fn channel_map(channels: u32) -> u16 {
    if channels <= 8 {
        LAZY_CHANNEL_MAPS[channels as usize]
    } else {
        (1u16 << channels).wrapping_sub(1)
    }
}

/// Builds a 5-byte Scream packet header using a fast branchless lookup table.
pub fn make_header(format: AudioParams) -> [u8; HEADER_SIZE] {
    let sample_rate_code = match format.rate {
        // 48000 Hz base family (bit 7 clear, lower 7 bits hold the multiplier)
        48000 => 1,
        96000 => 2,
        192000 => 4,
        384000 => 8,
        768000 => 16,

        // 44100 Hz base family (bit 7 set: 0x80 | multiplier)
        44100 => 0x80 | 1,
        88200 => 0x80 | 2,
        176400 => 0x80 | 4,
        352800 => 0x80 | 8,

        _ => {
            if format.rate.is_multiple_of(44100) {
                0x80 | ((format.rate / 44100) as u8).min(127)
            } else {
                ((format.rate / 48000) as u8).min(127)
            }
        }
    };

    let map = channel_map(format.channels);

    [
        sample_rate_code,
        format.bits as u8,
        format.channels as u8,
        map as u8,
        (map >> 8) as u8,
    ]
}

/// Parse a 5‑byte Scream header into AudioParams | None.
///
/// Returns `None` if the header is invalid or describes an unsupported format.
pub fn parse_header(header: &[u8; HEADER_SIZE]) -> Option<AudioParams> {
    let sample_rate_code = header[0];
    let bits = header[1] as u32;
    let channels = header[2] as u32;

    // Bits per sample must be 16, 24 or 32
    if bits != 16 && bits != 24 && bits != 32 {
        return None;
    }
    // Channels must be in 1..8
    if !(1..=8).contains(&channels) {
        return None;
    }

    // Decode sample rate
    let (base, multiplier) = if sample_rate_code & 0x80 != 0 {
        (44100, (sample_rate_code & 0x7F) as u32)
    } else {
        (48000, (sample_rate_code & 0x7F) as u32)
    };

    // Multiplier must never be zero (spec requirement)
    if multiplier == 0 {
        return None;
    }

    let rate = base * multiplier;

    // Basic sanity check: rate should be reasonable (< 768 kHz)
    if rate > 768_000 {
        return None;
    }

    Some(AudioParams {
        rate,
        bits,
        channels,
    })
}
