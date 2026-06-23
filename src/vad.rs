#[allow(unused_imports)]
use log::{debug, info};

#[derive(Debug, Clone)]
pub struct VadConfig {
    pub threshold: u16,
    pub silence_packets: u32,
    pub active_sleep_ms: u64,
    pub idle_sleep_ms: u64,
    pub frame_bytes: usize, // bytes per audio frame (bits/8 * channels)
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
}

impl Vad {
    pub fn new(config: VadConfig) -> Self {
        let enabled = config.threshold > 0 && config.silence_packets > 0;

        // Calculate packet and silence duration for logging
        let packet_duration_ms = (crate::scream::AUDIO_PAYLOAD_SIZE as f64
            / config.frame_bytes as f64
            / crate::scream::RATE as f64)
            * 1000.0;
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
        }
    }

    /// Analyse a raw audio packet (1152 bytes, 16‑bit LE interleaved).
    ///
    /// Returns `(should_send, sleep_ms)`.
    /// - `should_send`: true if the packet should be transmitted.
    /// - `sleep_ms`: recommended sleep duration for the next idle wait.
    pub fn process(&mut self, packet: &[u8]) -> (bool, u64) {
        if !self.enabled {
            return (true, self.sleep_ms);
        }

        // Early exit: stop scanning as soon as a loud sample is found
        let has_signal = packet.chunks_exact(2).any(|ch| {
            let sample = i16::from_le_bytes([ch[0], ch[1]]);
            sample.unsigned_abs() > self.threshold
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
