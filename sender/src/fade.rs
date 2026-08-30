//! Linear fade for real-time audio using Q32.32 fixed-point arithmetic.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FadeDirection {
    In,
    Out,
}

pub struct Fade {
    total_samples: u32,
    current_sample: u32,
    /// Q32.32 fixed-point accumulator (0.0 ..= 1.0).
    accumulator: u64,
    gain_step_32_32: u64,
    direction: FadeDirection,
}

impl Fade {
    pub fn new(duration_ms: u32, sample_rate: u32, direction: FadeDirection) -> Self {
        let total_samples = (duration_ms as u64 * sample_rate as u64 / 1000) as u32;

        // Rounding for fade-in avoids a final step below 1.0; floor for fade-out
        // guarantees the accumulator never underflows before the last sample.
        let gain_step_32_32 = if total_samples > 0 {
            match direction {
                FadeDirection::In => {
                    ((1u64 << 32) + (total_samples as u64 / 2)) / total_samples as u64
                }
                FadeDirection::Out => (1u64 << 32) / total_samples as u64,
            }
        } else {
            0
        };

        let accumulator = match direction {
            FadeDirection::In => 0,
            FadeDirection::Out => 1u64 << 32,
        };

        Self {
            total_samples,
            current_sample: 0,
            accumulator,
            gain_step_32_32,
            direction,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.current_sample = 0;
        self.accumulator = match self.direction {
            FadeDirection::In => 0,
            FadeDirection::Out => 1u64 << 32,
        };
    }

    /// Apply the fade to a buffer of interleaved samples. `bits` must be 16, 24, or 32.
    pub fn apply(&mut self, data: &mut [u8], bits: u32, channels: u32) {
        let sample_bytes = (bits / 8) as usize;
        let frame_bytes = sample_bytes * channels as usize;

        if frame_bytes == 0 || data.len() < frame_bytes {
            return;
        }

        let num_frames = data.len() / frame_bytes;

        for frame in 0..num_frames {
            if self.current_sample < self.total_samples {
                let offset = frame * frame_bytes;

                let gain = (self.accumulator >> 16).min(65536) as i64;

                match self.direction {
                    FadeDirection::In => {
                        self.accumulator = self.accumulator.saturating_add(self.gain_step_32_32);
                    }
                    FadeDirection::Out => {
                        self.accumulator = self.accumulator.saturating_sub(self.gain_step_32_32);
                    }
                }
                self.current_sample += 1;

                for ch in 0..channels as usize {
                    let sample_offset = offset + ch * sample_bytes;
                    match bits {
                        16 => {
                            let sample =
                                i16::from_le_bytes([data[sample_offset], data[sample_offset + 1]]);
                            let scaled = ((sample as i64) * gain) >> 16;
                            data[sample_offset..sample_offset + 2]
                                .copy_from_slice(&(scaled as i16).to_le_bytes());
                        }
                        24 => {
                            let mut sample = (data[sample_offset] as i32)
                                | ((data[sample_offset + 1] as i32) << 8)
                                | ((data[sample_offset + 2] as i32) << 16);
                            if sample & 0x0080_0000 != 0 {
                                sample |= !0x00FF_FFFF;
                            }
                            let scaled = ((sample as i64) * gain) >> 16;
                            let bytes = (scaled as i32).to_le_bytes();
                            data[sample_offset..sample_offset + 3].copy_from_slice(&bytes[..3]);
                        }
                        32 => {
                            let sample = i32::from_le_bytes([
                                data[sample_offset],
                                data[sample_offset + 1],
                                data[sample_offset + 2],
                                data[sample_offset + 3],
                            ]);
                            let scaled = ((sample as i64) * gain) >> 16;
                            data[sample_offset..sample_offset + 4]
                                .copy_from_slice(&(scaled as i32).to_le_bytes());
                        }
                        _ => unsafe { std::hint::unreachable_unchecked() },
                    }
                }
            } else {
                match self.direction {
                    // Fade-in complete: remaining frames are already at full volume.
                    FadeDirection::In => break,
                    // Fade-out complete: silence the rest of the buffer at once.
                    FadeDirection::Out => {
                        let offset = frame * frame_bytes;
                        data[offset..].fill(0);
                        break;
                    }
                }
            }
        }
    }
}
