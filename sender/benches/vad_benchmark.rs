use criterion::{Criterion, criterion_group, criterion_main};
use screamwire::vad::{FullSimd, ScanStrategy, Stride1024, Vad, VadConfig};

use std::hint::black_box;

use screamwire_common::test_utils::{BUFFER_SIZE, generate_buffer};

fn bench_vad_variants<D: ScanStrategy>(c: &mut Criterion) {
    let bits = 16;
    let config = VadConfig {
        enabled: true,
        mode: String::from(""),
        threshold: 100,
        max_silence_bytes: 384_000,
    };

    let active_data = generate_buffer(BUFFER_SIZE, bits, 2, false, 200);
    let silent_data = generate_buffer(BUFFER_SIZE, bits, 2, true, 0);

    // Active signal - state is mutated across iterations.
    let active_id = format!("vad_{}_{}bit_active_signal", D::NAME, bits);
    c.bench_function(&active_id, |b| {
        let mut vad = Vad::<D>::new(config.clone());
        b.iter(|| {
            black_box(&mut vad).process(black_box(&active_data));
        })
    });

    // Pure silence - a fresh VAD is created for each iteration so the
    // benchmark covers scanning, silence counting and state transitions.
    let silence_id = format!("vad_{}_{}bit_pure_silence", D::NAME, bits);
    c.bench_function(&silence_id, |b| {
        b.iter(|| {
            let mut vad = Vad::<D>::new(config.clone());
            black_box(&mut vad).process(black_box(&silent_data));
        })
    });
}

fn benchmarks(c: &mut Criterion) {
    bench_vad_variants::<Stride1024>(c);
    bench_vad_variants::<FullSimd>(c);
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
