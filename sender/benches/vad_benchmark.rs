use criterion::{Criterion, criterion_group, criterion_main};
use screamwire::vad::{Vad, VadConfig};
use screamwire_common::scream::AUDIO_PAYLOAD_SIZE;
use screamwire_common::types::AudioParams;
use std::hint::black_box;

fn generate_packet(bits: u32, channels: u32, peak_amplitude: u16) -> Vec<u8> {
    let sample_bytes = (bits / 8) as usize;
    let frame_bytes = sample_bytes * channels as usize;
    let num_frames = AUDIO_PAYLOAD_SIZE / frame_bytes;
    let mut data = vec![0u8; AUDIO_PAYLOAD_SIZE];

    for frame in 0..num_frames {
        let frame_offset = frame * frame_bytes;
        for ch in 0..channels {
            let offset = frame_offset + ch as usize * sample_bytes;
            let sample_val = if ch == 0 && frame % 100 == 0 {
                peak_amplitude as i32
            } else {
                0
            };
            match bits {
                8 => {
                    let s = sample_val as i8;
                    data[offset] = s as u8;
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

fn bench_vad(c: &mut Criterion, bits: u32) {
    let format = AudioParams {
        rate: 48000,
        bits,
        channels: 2,
    };
    let config = VadConfig {
        threshold: 100,
        silence_packets: 167,
        active_sleep_ms: 4,
        idle_sleep_ms: 30,
    };
    let data = generate_packet(bits, 2, 200);
    let mut vad = Vad::new(config, format);

    c.bench_function(&format!("vad_{}bit", bits), |b| {
        b.iter(|| vad.process(black_box(&data)))
    });
}

fn benchmarks(c: &mut Criterion) {
    bench_vad(c, 8);
    bench_vad(c, 16);
    bench_vad(c, 24);
    bench_vad(c, 32);
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
