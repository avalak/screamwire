// Scream protocol constants
pub const DEFAULT_BITS: u32 = 16;
pub const DEFAULT_RATE: u32 = 48000;
pub const DEFAULT_CHANNELS: u32 = 2;

#[derive(Debug, Clone, Copy)]
pub struct AudioParams {
    pub rate: u32,
    pub bits: u32,
    pub channels: u32,
}

impl AudioParams {
    /// bytes per audio frame (bits/8 * channels)
    pub fn frame_bytes(&self) -> usize {
        (self.bits as usize / 8) * self.channels as usize
    }
}
