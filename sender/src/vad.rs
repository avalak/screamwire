#[allow(unused_imports)]
use log::{debug, error, info};
use screamwire_common::scream::AUDIO_PAYLOAD_SIZE;
use screamwire_common::types::AudioParams;

#[derive(Debug, Clone)]
pub struct VadConfig {
    pub threshold: u16,
    pub silence_packets: u32,
    pub active_sleep_ms: u64,
    pub idle_sleep_ms: u64,
}

/// Voice Activity Detector with integrated sleep policy.
///
/// When `threshold == 0` or `silence_packets == 0`, VAD is disabled:
/// every packet is sent and the active sleep duration is used.
pub struct Vad {
    enabled: bool,
    threshold: u16,
    silence_packets: u32,

    active: bool,
    silent_count: u32,

    // Sleep durations (milliseconds)
    active_sleep_ms: u64,
    idle_sleep_ms: u64,
    /// Current sleep duration, updated only on state transitions.
    sleep_ms: u64,
    format: AudioParams,
}

impl Vad {
    pub fn new(config: VadConfig, format: AudioParams) -> Self {
        let enabled = config.threshold > 0 && config.silence_packets > 0;

        let frame_bytes = format.frame_bytes();
        // Calculate packet and silence duration for logging
        let packet_duration_ms =
            (AUDIO_PAYLOAD_SIZE as f64 / frame_bytes as f64 / format.rate as f64) * 1000.0;
        let silence_duration_ms = packet_duration_ms * config.silence_packets as f64;

        info!(
            "VAD initialised: {} (threshold={}, silence_packets={}, packet={:.1} ms, silence≈{:.0} ms)",
            if enabled { "enabled" } else { "disabled" },
            config.threshold,
            config.silence_packets,
            packet_duration_ms,
            silence_duration_ms
        );

        Vad {
            enabled,
            threshold: config.threshold,
            silence_packets: config.silence_packets,
            active: true,
            silent_count: 0,
            active_sleep_ms: config.active_sleep_ms,
            idle_sleep_ms: config.idle_sleep_ms,
            sleep_ms: config.active_sleep_ms,
            format,
        }
    }

    /// Analyse a raw audio packet (1152 bytes, 16/24/32‑bit LE interleaved).
    ///
    /// Returns `(should_send, sleep_ms)`.
    /// - `should_send`: true if the packet should be transmitted.
    /// - `sleep_ms`: recommended sleep duration for the next idle wait.
    pub fn process(&mut self, packet: &[u8]) -> (bool, u64) {
        if !self.enabled {
            return (true, self.sleep_ms);
        }

        // Early exit: stop scanning as soon as a loud sample is found
        let sample_bytes = (self.format.bits / 8) as usize;
        let has_signal = packet.chunks_exact(sample_bytes).any(|ch| {
            let sample = sign_extend(ch, self.format.bits);
            sample.unsigned_abs() > self.threshold as u32
        });

        if self.active {
            if !has_signal {
                self.silent_count += 1;
                if self.silent_count >= self.silence_packets {
                    self.active = false;
                    self.silent_count = 0;
                    self.sleep_ms = self.idle_sleep_ms; // switch to idle sleep
                    debug!("VAD: silence detected, pausing TX");
                }
            } else {
                self.silent_count = 0;
            }
        } else {
            if has_signal {
                self.active = true;
                self.silent_count = 0;
                self.sleep_ms = self.active_sleep_ms; // switch back to active sleep
                debug!("VAD: audio resumed, restarting TX");
            }
        }

        (self.active, self.sleep_ms)
    }
}

/// Extend sign to i32 for any bit depth ≤ 32.
fn sign_extend(bytes: &[u8], bits: u32) -> i32 {
    let shift = 32 - bits;
    let raw = match bytes.len() {
        1 => i8::from_le_bytes([bytes[0]]) as i32,
        2 => i16::from_le_bytes([bytes[0], bytes[1]]) as i32,
        3 => {
            let sign_byte = if bytes[2] & 0x80 != 0 { 0xFFu32 } else { 0 };
            (u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]) | (sign_byte << 24)) as i32
        }
        4 => i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        _ => 0,
    };
    (raw << shift) >> shift // arithmetic shift extends sign
}
