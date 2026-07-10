use crate::rt_debug;
use crate::scanners::*;
#[allow(unused)]
use log::{debug, info};

// Configuration

#[derive(Debug, Clone)]
pub struct VadConfig {
    pub enabled: bool,
    pub threshold: u16,
    /// Maximum silence duration in bytes.
    /// rate * (bits/8) * channels * silence_seconds
    pub max_silence_bytes: usize,
}

/// Voice Activity Detector (Silence Detector)
/// Monomorphized VAD with compile-time audio format dispatch.
pub struct BitPerfectVad {
    enabled: bool,
    #[allow(dead_code)]
    threshold: u32,
    max_silence_bytes: usize,

    // Mutable state
    active: bool,
    silent_bytes_count: usize,
}

impl BitPerfectVad {
    pub fn new(config: VadConfig) -> Self {
        let enabled = config.max_silence_bytes > 0;
        info!(
            "VAD: bitperfect (SIMD, max_silence_bytes={})",
            config.max_silence_bytes
        );
        Self {
            enabled,
            threshold: config.threshold as u32,
            max_silence_bytes: config.max_silence_bytes,
            active: true,
            silent_bytes_count: 0,
        }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.active = true;
        self.silent_bytes_count = 0;
    }

    #[inline(always)]
    pub fn force_idle(&mut self) {
        self.active = false;
        self.silent_bytes_count = 0;
    }

    #[inline(always)]
    pub fn process(&mut self, packet: &[u8]) -> bool {
        if !self.enabled {
            return true;
        }

        let has_signal = scan_generic_silence_simd(packet, 0);

        if self.active {
            if !has_signal {
                self.silent_bytes_count += packet.len();
                if self.silent_bytes_count >= self.max_silence_bytes {
                    self.active = false;
                    self.silent_bytes_count = 0;
                }
            } else {
                self.silent_bytes_count = 0;
            }
        } else if has_signal {
            self.active = true;
            self.silent_bytes_count = 0;
        }

        self.active
    }
}

/// Disabled VAD: stream-only control, passes everything
pub struct VadDisabled {
    active: bool,
}

impl VadDisabled {
    pub fn new(_config: VadConfig) -> Self {
        info!("VAD: disabled");
        Self { active: true }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.active = true;
    }

    #[inline(always)]
    pub fn force_idle(&mut self) {
        self.active = false;
    }

    #[inline(always)]
    pub fn process(&mut self, _packet: &[u8]) -> bool {
        self.active
    }
}

pub struct DirtyVad {
    enabled: bool,
    max_silence_bytes: usize,
    active: bool,
    silent_bytes_count: usize,
}

impl DirtyVad {
    pub fn new(config: VadConfig) -> Self {
        let enabled = config.threshold > 0 && config.max_silence_bytes > 0;
        log::info!(
            "DirtyVAD: {} (max_silence_bytes={})",
            if enabled { "enabled" } else { "disabled" },
            config.max_silence_bytes
        );
        Self {
            enabled,
            max_silence_bytes: config.max_silence_bytes,
            active: true,
            silent_bytes_count: 0,
        }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.active = true;
        self.silent_bytes_count = 0;
    }

    #[inline(always)]
    pub fn force_idle(&mut self) {
        self.active = false;
        self.silent_bytes_count = 0;
    }

    /// Process a chunk of audio bytes.
    ///
    /// Returns `true` if the chunk should be transmitted.
    /// Never branches on bit‑depth; threshold is ignored.
    #[inline(always)]
    pub fn process(&mut self, packet: &[u8]) -> bool {
        if !self.enabled {
            return true;
        }

        let has_signal = any_strided_nonzero(packet, 0);

        if self.active {
            if !has_signal {
                rt_debug!("+++ silence");
                self.silent_bytes_count += packet.len();
                if self.silent_bytes_count >= self.max_silence_bytes {
                    self.active = false;
                    self.silent_bytes_count = 0;
                }
            } else {
                self.silent_bytes_count = 0;
            }
        } else if has_signal {
            self.active = true;
            self.silent_bytes_count = 0;
        }

        self.active
    }
}

/// Monomorphized VAD dispatch
pub enum DynamicVad {
    Disabled(VadDisabled),
    Dirty(DirtyVad),
    BitPerfect(BitPerfectVad),
}

#[macro_export]
macro_rules! dispatch_vad {
    ($vad:expr, |$v:ident| $body:expr) => {
        match $vad {
            $crate::vad::DynamicVad::Disabled($v) => $body,
            $crate::vad::DynamicVad::Dirty($v) => $body,
            $crate::vad::DynamicVad::BitPerfect($v) => $body,
        }
    };
}
