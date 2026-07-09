use crate::rt_debug;
use crate::scanners::*;
#[allow(unused)]
use log::{debug, info};
use screamwire_common::scream::AUDIO_PAYLOAD_SIZE;
use std::marker::PhantomData;

/// Compile-time audio depth dispatch
pub trait AudioDepth: Send + 'static {
    const BITS: u8;
    fn scan_active(packet: &[u8], threshold: u32) -> bool;
}

pub struct Depth8;
impl AudioDepth for Depth8 {
    const BITS: u8 = 8;
    #[inline(always)]
    fn scan_active(packet: &[u8], threshold: u32) -> bool {
        scan_8bit_any(packet, threshold)
    }
}

pub struct Depth16;
impl AudioDepth for Depth16 {
    const BITS: u8 = 16;
    #[inline(always)]
    fn scan_active(packet: &[u8], threshold: u32) -> bool {
        scan_16bit_any(packet, threshold)
    }
}

pub struct Depth24;
impl AudioDepth for Depth24 {
    const BITS: u8 = 24;
    #[inline(always)]
    fn scan_active(packet: &[u8], threshold: u32) -> bool {
        scan_24bit_any(packet, threshold)
    }
}

pub struct Depth32;
impl AudioDepth for Depth32 {
    const BITS: u8 = 32;
    #[inline(always)]
    fn scan_active(packet: &[u8], threshold: u32) -> bool {
        scan_32bit_any(packet, threshold)
    }
}

// Configuration

#[derive(Debug, Clone)]
pub struct VadConfig {
    pub threshold: u16,
    pub silence_packets: u32,
}

/// Voice Activity Detector (Silence Detector)
/// Monomorphized VAD with compile-time audio format dispatch.
pub struct Vad<D: AudioDepth> {
    enabled: bool,
    threshold: u32,
    max_silence_bytes: usize,

    // Mutable state
    active: bool,
    silent_bytes_count: usize,

    _marker: PhantomData<D>,
}

impl<D: AudioDepth> Vad<D> {
    pub fn new(config: VadConfig) -> Self {
        let enabled = config.threshold > 0 && config.silence_packets > 0;
        let max_silence_bytes = config.silence_packets as usize * AUDIO_PAYLOAD_SIZE;

        info!(
            "VAD initialized for PipeWire thread: {} (bits={}, threshold={}, max_silence_bytes={})",
            if enabled { "enabled" } else { "disabled" },
            D::BITS,
            config.threshold,
            max_silence_bytes
        );

        Self {
            enabled,
            threshold: config.threshold as u32,
            max_silence_bytes,
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

        // Compile-time dispatch
        let has_signal = if self.active {
            D::scan_active(packet, self.threshold)
        } else {
            scan_generic_silence_fold(packet, self.threshold)
        };

        // State machine
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
    Bits8(Vad<Depth8>),
    Bits16(Vad<Depth16>),
    Bits24(Vad<Depth24>),
    Bits32(Vad<Depth32>),
    Disabled(VadDisabled),
}

#[macro_export]
macro_rules! dispatch_vad {
    ($vad:expr, |$v:ident| $body:expr) => {
        match $vad {
            $crate::vad::DynamicVad::Bits8($v) => $body,
            $crate::vad::DynamicVad::Bits16($v) => $body,
            $crate::vad::DynamicVad::Bits24($v) => $body,
            $crate::vad::DynamicVad::Bits32($v) => $body,
            $crate::vad::DynamicVad::Disabled($v) => $body,
        }
    };
}
