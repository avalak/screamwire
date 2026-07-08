use criterion::{Criterion, criterion_group, criterion_main};
use screamwire::vad::{Vad, VadConfig};

use screamwire_common::types::AudioParams;
use std::hint::black_box;

mod helpers;
use helpers::generate_packet;

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

    // "Deep sleep"
    c.bench_function(&format!("vad_{}bit_pure_silence_sleeping", bits), |b| {
        b.iter_with_setup(
            || {
                let mut vad = Vad::new(config.clone(), format);
                for _ in 0..=config.silence_packets {
                    vad.process(&silent_data);
                }

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
