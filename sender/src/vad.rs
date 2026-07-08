use crate::scanners::*;
#[allow(unused)]
use log::{debug, info};
use screamwire_common::scream::AUDIO_PAYLOAD_SIZE;
use screamwire_common::types::AudioParams;

#[derive(Debug, Clone)]
pub struct VadConfig {
    pub threshold: u16,
    pub silence_packets: u32,
}

/// Voice Activity Detector (Silence Detector)
/// Handle raw data and PipeWire steam events
/// This is experimental version for PipeWire thread
pub struct Vad {
    enabled: bool,
    threshold: u32,
    max_silence_bytes: usize,
    silent_bytes_count: usize,

    active: bool,
    format: AudioParams,
}

impl Vad {
    pub fn new(config: VadConfig, format: AudioParams) -> Self {
        let enabled = config.threshold > 0 && config.silence_packets > 0;

        // Convert packets to bytes
        // TODO: replace packet with seconds
        let max_silence_bytes = config.silence_packets as usize * AUDIO_PAYLOAD_SIZE;

        info!(
            "VAD initialized for PipeWire thread: {} (threshold={}, max_silence_bytes={})",
            if enabled { "enabled" } else { "disabled" },
            config.threshold,
            max_silence_bytes
        );

        Vad {
            enabled,
            threshold: config.threshold as u32,
            max_silence_bytes,
            silent_bytes_count: 0,
            active: true,
            format,
        }
    }

    pub fn reset(&mut self) {
        self.active = true;
        self.silent_bytes_count = 0;
    }

    pub fn force_idle(&mut self) {
        self.active = false;
        self.silent_bytes_count = 0;
    }

    #[inline(always)]
    pub fn process(&mut self, packet: &[u8]) -> bool {
        if !self.enabled {
            return true;
        }

        let has_signal = match (self.active, self.format.bits) {
            (true, 16) => scan_16bit_any(packet, self.threshold),
            (true, 24) => scan_24bit_any(packet, self.threshold),
            (true, 32) => scan_32bit_any(packet, self.threshold),
            (true, 8) => scan_8bit_any(packet, self.threshold),

            // on idle
            (false, _) => scan_generic_silence_fold(packet, self.threshold),

            _ => false,
        };

        // update state
        if self.active {
            if !has_signal {
                self.silent_bytes_count += packet.len();
                if self.silent_bytes_count >= self.max_silence_bytes {
                    self.active = false;
                    self.silent_bytes_count = 0;
                    debug!("inactive");
                }
            } else {
                self.silent_bytes_count = 0;
            }
        } else if has_signal {
            self.active = true;
            self.silent_bytes_count = 0;
            debug!("active");
        }

        self.active
    }
}
