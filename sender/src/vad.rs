use crate::rt_debug;
use crate::scanners::*;
#[allow(unused)]
use log::{debug, info};
use std::marker::PhantomData;

// Configuration
#[derive(Debug, Clone)]
pub struct VadConfig {
    pub enabled: bool,
    pub mode: String,
    pub threshold: u16,
    /// Maximum silence duration in bytes.
    /// rate * (bits/8) * channels * silence_seconds
    pub max_silence_bytes: usize,
}

pub trait ScanStrategy: Send + 'static {
    const NAME: &'static str;
    fn scan(packet: &[u8]) -> bool;
}

pub struct Stride1024;
impl ScanStrategy for Stride1024 {
    const NAME: &'static str = "quick-1024";
    #[inline(always)]
    fn scan(packet: &[u8]) -> bool {
        any_strided_nonzero_1024(packet, 0)
    }
}

pub struct FullSimd;
impl ScanStrategy for FullSimd {
    const NAME: &'static str = "full-simd";
    #[inline(always)]
    fn scan(packet: &[u8]) -> bool {
        scan_generic_silence_simd(packet, 0)
    }
}

/// Voice Activity Detector (Silence Detector)
/// Monomorphized VAD with compile-time audio format dispatch.
pub struct Vad<S: ScanStrategy> {
    enabled: bool,
    #[allow(dead_code)]
    threshold: u32,
    max_silence_bytes: usize,

    // Mutable state
    active: bool,
    silent_bytes_count: usize,
    _marker: PhantomData<S>,
}

impl<S: ScanStrategy> Vad<S> {
    pub fn new(config: VadConfig) -> Self {
        let enabled = config.max_silence_bytes > 0;
        info!(
            "VAD: {} strategy={}, max_silence_bytes={}",
            if enabled { "enabled" } else { "disabled" },
            S::NAME,
            config.max_silence_bytes
        );
        Self {
            enabled,
            threshold: config.threshold as u32,
            max_silence_bytes: config.max_silence_bytes,
            active: true,
            silent_bytes_count: 0,
            _marker: PhantomData,
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

        let has_signal = S::scan(packet);

        if self.active {
            if !has_signal {
                self.silent_bytes_count += packet.len();
                if self.silent_bytes_count >= self.max_silence_bytes {
                    self.active = false;
                    self.silent_bytes_count = 0;
                    rt_debug!("VAD: inactive");
                }
            } else {
                self.silent_bytes_count = 0;
            }
        } else if has_signal {
            self.active = true;
            self.silent_bytes_count = 0;
            rt_debug!("VAD: active");
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

/// Monomorphized VAD dispatch
pub enum DynamicVad {
    Disabled(VadDisabled),
    Quick1024(Vad<Stride1024>),
    FullSimd(Vad<FullSimd>),
}

impl DynamicVad {
    pub fn from_config(config: VadConfig) -> Self {
        if !config.enabled || config.max_silence_bytes == 0 {
            return DynamicVad::Disabled(VadDisabled::new(config));
        }
        let mode = config.mode.trim().to_lowercase();
        match mode.as_str() {
            Stride1024::NAME => DynamicVad::Quick1024(Vad::new(config)),
            FullSimd::NAME => DynamicVad::FullSimd(Vad::new(config)),
            _ => DynamicVad::Disabled(VadDisabled::new(config)),
        }
    }
}

#[macro_export]
macro_rules! dispatch_vad {
    ($vad:expr, |$v:ident| $body:expr) => {
        match $vad {
            $crate::vad::DynamicVad::Disabled($v) => $body,
            $crate::vad::DynamicVad::Quick1024($v) => $body,
            $crate::vad::DynamicVad::FullSimd($v) => $body,
        }
    };
}
