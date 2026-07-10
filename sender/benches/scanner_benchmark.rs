use criterion::{Criterion, criterion_group, criterion_main};
use screamwire::scanners::{
    any_strided_nonzero_1024, scan_8bit_any, scan_16bit_any, scan_24bit_any, scan_32bit_any,
    scan_generic_silence_fold, scan_generic_silence_simd,
};
use screamwire_common::test_utils::{BUFFER_SIZE, generate_buffer};
use std::hint::black_box;

fn bench_scanners(c: &mut Criterion) {
    let threshold = 100;

    // Pre‑generate all test vectors.
    let data_8bit_signal = generate_buffer(BUFFER_SIZE, 8, 2, false, 200);
    let data_8bit_silence = generate_buffer(BUFFER_SIZE, 8, 2, true, 0);
    let data_16bit_signal = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    let data_16bit_silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    let data_24bit_signal = generate_buffer(BUFFER_SIZE, 24, 2, false, 200);
    let data_24bit_silence = generate_buffer(BUFFER_SIZE, 24, 2, true, 0);
    let data_32bit_signal = generate_buffer(BUFFER_SIZE, 32, 2, false, 200);
    let data_32bit_silence = generate_buffer(BUFFER_SIZE, 32, 2, true, 0);

    // Universal silence detector – works on any bit depth because
    // digital silence is always a stream of zero bytes.
    c.bench_function("scanner_generic_fold_active_signal", |b| {
        b.iter(|| scan_generic_silence_fold(black_box(&data_16bit_signal), threshold))
    });

    c.bench_function("scanner_generic_fold_pure_silence", |b| {
        b.iter(|| scan_generic_silence_fold(black_box(&data_16bit_silence), threshold))
    });

    c.bench_function("scanner_generic_simd_active_signal", |b| {
        b.iter(|| scan_generic_silence_simd(black_box(&data_16bit_signal), threshold))
    });

    c.bench_function("scanner_generic_simd_pure_silence", |b| {
        b.iter(|| scan_generic_silence_simd(black_box(&data_16bit_silence), threshold))
    });

    c.bench_function("scanner_any_strided_nonzero_1024_active_signal", |b| {
        b.iter(|| any_strided_nonzero_1024(black_box(&data_16bit_signal), threshold))
    });

    c.bench_function("scanner_any_strided_nonzero_1024_pure_silence", |b| {
        b.iter(|| any_strided_nonzero_1024(black_box(&data_16bit_silence), threshold))
    });

    // Specialized scan functions

    // 16‑bit
    c.bench_function("scanner_16bit_active_signal", |b| {
        b.iter(|| scan_16bit_any(black_box(&data_16bit_signal), threshold))
    });
    c.bench_function("scanner_16bit_pure_silence", |b| {
        b.iter(|| scan_16bit_any(black_box(&data_16bit_silence), threshold))
    });

    // 24‑bit
    c.bench_function("scanner_24bit_active_signal", |b| {
        b.iter(|| scan_24bit_any(black_box(&data_24bit_signal), threshold))
    });
    c.bench_function("scanner_24bit_pure_silence", |b| {
        b.iter(|| scan_24bit_any(black_box(&data_24bit_silence), threshold))
    });

    // 32‑bit
    c.bench_function("scanner_32bit_active_signal", |b| {
        b.iter(|| scan_32bit_any(black_box(&data_32bit_signal), threshold))
    });
    c.bench_function("scanner_32bit_pure_silence", |b| {
        b.iter(|| scan_32bit_any(black_box(&data_32bit_silence), threshold))
    });

    // Normally not used
    // 8‑bit
    c.bench_function("scanner_8bit_active_signal", |b| {
        b.iter(|| scan_8bit_any(black_box(&data_8bit_signal), threshold))
    });
    c.bench_function("scanner_8bit_pure_silence", |b| {
        b.iter(|| scan_8bit_any(black_box(&data_8bit_silence), threshold))
    });
}

criterion_group!(benches, bench_scanners);
criterion_main!(benches);
