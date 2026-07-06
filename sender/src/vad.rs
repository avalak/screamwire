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
    threshold: u32,
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
            threshold: config.threshold as u32,
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

        let has_signal = match self.format.bits {
            16 => self.scan_16bit(packet),
            24 => self.scan_24bit(packet),
            32 => self.scan_32bit(packet),
            8 => self.scan_8bit(packet), // normally not used
            _ => false,
        };

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
        } else if has_signal {
            self.active = true;
            self.silent_count = 0;
            self.sleep_ms = self.active_sleep_ms; // switch back to active sleep
            debug!("VAD: audio resumed, restarting TX");
        }

        (self.active, self.sleep_ms)
    }

    // Bit‑depth‑specific scanners

    /// 16‑bit: uses `align_to` to let the compiler auto‑vectorise.
    fn scan_16bit(&self, packet: &[u8]) -> bool {
        let (prefix, samples, suffix) = unsafe { packet.align_to::<i16>() };

        if !prefix.is_empty() || !suffix.is_empty() {
            return packet.chunks_exact(2).any(|ch| {
                let s = i16::from_le_bytes([ch[0], ch[1]]);
                (s as i32).unsigned_abs() > self.threshold
            });
        }

        samples
            .iter()
            .any(|&s| (s as i32).unsigned_abs() > self.threshold)
    }

    /// 32‑bit: same as 16-bit
    fn scan_32bit(&self, packet: &[u8]) -> bool {
        let (prefix, samples, suffix) = unsafe { packet.align_to::<i32>() };

        if !prefix.is_empty() || !suffix.is_empty() {
            return packet.chunks_exact(4).any(|ch| {
                let s = i32::from_le_bytes([ch[0], ch[1], ch[2], ch[3]]);
                s.unsigned_abs() > self.threshold
            });
        }

        samples.iter().any(|&s| s.unsigned_abs() > self.threshold)
    }

    /// 24‑bit
    fn scan_24bit(&self, packet: &[u8]) -> bool {
        packet.chunks_exact(3).any(|ch| {
            let raw = u32::from_le_bytes([ch[0], ch[1], ch[2], 0]);

            let sample = if (raw & 0x0080_0000) != 0 {
                (raw | 0xFF00_0000) as i32
            } else {
                raw as i32
            };

            sample.unsigned_abs() > self.threshold
        })
    }

    /// 8‑bit; normally not used
    fn scan_8bit(&self, packet: &[u8]) -> bool {
        packet
            .iter()
            .any(|&b| (b as i8 as i32).unsigned_abs() > self.threshold)
    }
}
