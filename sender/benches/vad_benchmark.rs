use criterion::{Criterion, criterion_group, criterion_main};
use screamwire::vad::{Vad, VadConfig};
use screamwire_common::scream::AUDIO_PAYLOAD_SIZE;
use screamwire_common::types::AudioParams;
use std::hint::black_box;

/// Generate a realistic interleaved audio packet.
///
/// If `is_silent` is true the packet is filled with zeros.
/// Otherwise a peak is placed in the first channel every 100th frame.
fn generate_packet(bits: u32, channels: u32, is_silent: bool, peak_amplitude: u16) -> Vec<u8> {
    let sample_bytes = (bits / 8) as usize;
    let frame_bytes = sample_bytes * channels as usize;
    let num_frames = AUDIO_PAYLOAD_SIZE / frame_bytes;
    let mut data = vec![0u8; AUDIO_PAYLOAD_SIZE];

    if is_silent {
        return data;
    }

    for frame in 0..num_frames {
        let frame_offset = frame * frame_bytes;
        for ch in 0..channels {
            let offset = frame_offset + ch as usize * sample_bytes;
            // Only the first channel gets a peak every 100th frame.
            let sample_val = if ch == 0 && frame % 100 == 0 {
                // Clamp 8-bit peak to avoid overflow.
                if bits == 8 {
                    (peak_amplitude as i32).min(i8::MAX as i32)
                } else {
                    peak_amplitude as i32
                }
            } else {
                0
            };
            match bits {
                8 => {
                    data[offset] = sample_val as u8;
                }
                16 => {
                    let s = sample_val as i16;
                    data[offset..offset + 2].copy_from_slice(&s.to_le_bytes());
                }
                24 => {
                    let bytes = (sample_val as u32 & 0x00FF_FFFF).to_le_bytes();
                    data[offset..offset + 3].copy_from_slice(&bytes[..3]);
                }
                32 => {
                    let bytes = sample_val.to_le_bytes();
                    data[offset..offset + 4].copy_from_slice(&bytes);
                }
                _ => panic!("unsupported bits"),
            }
        }
    }
    data
}

fn bench_vad_variants(c: &mut Criterion, bits: u32) {
    let format = AudioParams {
        rate: 48000,
        bits,
        channels: 2,
    };
    let config = VadConfig {
        threshold: 100,
        silence_packets: 167,
    };

    let active_data = generate_packet(bits, 2, false, 200);
    let silent_data = generate_packet(bits, 2, true, 0);

    // Active signal - state is mutated across iterations.
    c.bench_function(&format!("vad_{}bit_active_signal", bits), |b| {
        let mut vad = Vad::new(config.clone(), format);
        b.iter_with_setup(
            || active_data.clone(),
            |data| black_box(&mut vad).process(black_box(&data)),
        )
    });

    // Pure silence - a fresh VAD is created for each iteration so the
    // benchmark covers scanning, silence counting and state transitions.
    c.bench_function(&format!("vad_{}bit_pure_silence", bits), |b| {
        b.iter_with_setup(
            || {
                let vad = Vad::new(config.clone(), format);
                (vad, silent_data.clone())
            },
            |(mut vad, data)| black_box(&mut vad).process(black_box(&data)),
        )
    });
}

fn benchmarks(c: &mut Criterion) {
    for bits in [8, 16, 24, 32] {
        bench_vad_variants(c, bits);
    }
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
