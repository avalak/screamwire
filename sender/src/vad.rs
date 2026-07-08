#[allow(unused_imports)]
use log::{debug, error, info};
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

    /// Analyse a raw audio data
    /// true if data goes to ring buffer
    pub fn process(&mut self, packet: &[u8]) -> bool {
        if !self.enabled {
            return true;
        }

        //let mask = packet.iter().fold(0u8, |acc, &b| acc | b);
        //mask != 0
        // }
        // /*
        let has_signal = match self.format.bits {
            16 => self.scan_16bit(packet),
            24 => self.scan_24bit(packet),
            32 => self.scan_32bit(packet),
            8 => self.scan_8bit(packet),
            _ => false,
        };

        if self.active {
            if !has_signal {
                self.silent_bytes_count += packet.len();
                if self.silent_bytes_count >= self.max_silence_bytes {
                    self.active = false;
                    self.silent_bytes_count = 0;
                    debug!("VAD: Silence threshold reached. Going IDLE.");
                }
            } else {
                self.silent_bytes_count = 0;
            }
        } else if has_signal {
            self.active = true;
            self.silent_bytes_count = 0;
            debug!("VAD: Signal detected. Going ACTIVE.");
        }

        self.active
    }
    /* */

    #[allow(dead_code)]
    #[inline(always)]
    fn scan_generic_silence(&self, packet: &[u8]) -> bool {
        packet.iter().any(|&b| b != 0)
    }

    // Bit‑depth‑specific scanners

    /// 16‑bit: uses `align_to` to let the compiler auto‑vectorise.
    #[inline(always)]
    fn scan_16bit(&self, packet: &[u8]) -> bool {
        let (prefix, samples, suffix) = unsafe { packet.align_to::<i16>() };

        if samples
            .iter()
            .any(|&s| (s as i32).unsigned_abs() > self.threshold)
        {
            return true;
        }

        if !prefix.is_empty()
            && prefix.chunks_exact(2).any(|ch| {
                let s = i16::from_le_bytes(ch.try_into().unwrap());
                (s as i32).unsigned_abs() > self.threshold
            })
        {
            return true;
        }

        if !suffix.is_empty()
            && suffix.chunks_exact(2).any(|ch| {
                let s = i16::from_le_bytes(ch.try_into().unwrap());
                (s as i32).unsigned_abs() > self.threshold
            })
        {
            return true;
        }

        false
    }

    /// 32‑bit: same as 16-bit
    fn scan_32bit(&self, packet: &[u8]) -> bool {
        let (prefix, samples, suffix) = unsafe { packet.align_to::<i32>() };

        if samples.iter().any(|&s| s.unsigned_abs() > self.threshold) {
            return true;
        }

        if !prefix.is_empty()
            && prefix.chunks_exact(4).any(|ch| {
                let s = i32::from_le_bytes(ch.try_into().unwrap());
                s.unsigned_abs() > self.threshold
            })
        {
            return true;
        }

        if !suffix.is_empty()
            && suffix.chunks_exact(4).any(|ch| {
                let s = i32::from_le_bytes(ch.try_into().unwrap());
                s.unsigned_abs() > self.threshold
            })
        {
            return true;
        }

        false
    }

    /// 24‑bit
    fn scan_24bit(&self, packet: &[u8]) -> bool {
        packet.chunks_exact(3).any(|ch| {
            let raw = i32::from_le_bytes([0, ch[0], ch[1], ch[2]]);
            let sample = raw >> 8;
            sample.unsigned_abs() > self.threshold
        })
    }

    /// 8‑bit; normally not used
    fn scan_8bit(&self, packet: &[u8]) -> bool {
        let t = self.threshold as u8;
        packet.iter().any(|&b| (b as i8).unsigned_abs() > t)
    }
}
