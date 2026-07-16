use screamwire::dispatch_vad;
use screamwire::vad::{DynamicVad, FullSimd, Stride1024, Vad, VadConfig, VadDisabled};

use screamwire_common::test_utils::generate_buffer;

const BUFFER_SIZE: usize = 4096;

/// Create a default `VadConfig` with a given silence limit in bytes.
fn make_vad_config(max_silence_bytes: usize) -> VadConfig {
    VadConfig {
        enabled: true,
        mode: String::from(""),
        threshold: 1, // unused
        max_silence_bytes,
    }
}

// ---------------------------------------------------------------------------
// VadDisabled tests
// ---------------------------------------------------------------------------

#[test]
fn disabled_vad_passes_always_when_active() {
    let mut vad = VadDisabled::new(make_vad_config(0));
    let buf = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    assert!(vad.process(&buf));
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    assert!(vad.process(&loud));
}

#[test]
fn disabled_vad_reacts_to_stream_state() {
    let mut vad = VadDisabled::new(make_vad_config(0));
    let buf = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    vad.force_idle();
    assert!(!vad.process(&buf));
    vad.reset();
    assert!(vad.process(&buf));
}

// ---------------------------------------------------------------------------
// Full SIMD (bit‑perfect) tests
// ---------------------------------------------------------------------------

#[test]
fn fullsimd_detects_signal() {
    let mut vad = Vad::<FullSimd>::new(make_vad_config(8192));
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    assert!(vad.process(&loud));
}

#[test]
fn fullsimd_goes_idle_on_silence() {
    // With max_silence_bytes = 8000, two 4096‑byte silent buffers exceed the limit.
    let mut vad = Vad::<FullSimd>::new(make_vad_config(8000));
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    assert!(vad.process(&silence)); // 4096 bytes – still active
    assert!(!vad.process(&silence)); // 8192 bytes – idle triggered
}

#[test]
fn fullsimd_resumes_on_signal() {
    let mut vad = Vad::<FullSimd>::new(make_vad_config(8000));
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    // Drive into idle state
    while vad.process(&silence) {}
    assert!(!vad.process(&silence));
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    assert!(vad.process(&loud));
}

// ---------------------------------------------------------------------------
// Quick strided (probabilistic) tests
// ---------------------------------------------------------------------------

#[test]
fn quick_detects_signal() {
    let mut vad = Vad::<Stride1024>::new(make_vad_config(8192));
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    assert!(vad.process(&loud));
}

#[test]
fn quick_goes_idle_on_silence() {
    let mut vad = Vad::<Stride1024>::new(make_vad_config(8000));
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    assert!(vad.process(&silence));
    assert!(!vad.process(&silence));
}

#[test]
fn quick_resumes_on_signal() {
    let mut vad = Vad::<Stride1024>::new(make_vad_config(8000));
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    while vad.process(&silence) {}
    assert!(!vad.process(&silence));
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    assert!(vad.process(&loud));
}

// ---------------------------------------------------------------------------
// State machine edge‑case tests (using FullSimd as representative)
// ---------------------------------------------------------------------------

#[test]
fn reset_clears_silence_counter() {
    let mut vad = Vad::<FullSimd>::new(make_vad_config(8000));
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    vad.process(&silence); // 4096
    vad.process(&silence); // 8192 – would trigger idle, but we reset before that
    vad.reset();
    assert!(vad.process(&silence)); // counter restarted, still active
    assert!(!vad.process(&silence)); // now exceeded after reset
}

#[test]
fn force_idle_mutes_output() {
    let mut vad = Vad::<FullSimd>::new(make_vad_config(99999));
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    assert!(vad.process(&loud));
    vad.force_idle();
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    assert!(!vad.process(&silence));
}

#[test]
fn signal_resets_silence_counter() {
    let mut vad = Vad::<FullSimd>::new(make_vad_config(10000));
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    // Accumulate silence below the limit
    vad.process(&silence); // 4096
    vad.process(&silence); // 8192 (< 10000)
    // Inject a signal – counter must reset
    vad.process(&loud);
    // Now three more silence buffers should trigger idle at 3*4096 = 12288 > 10000
    assert!(vad.process(&silence)); // 4096
    assert!(vad.process(&silence)); // 8192
    assert!(!vad.process(&silence)); // 12288 → idle
}

#[test]
fn disabled_vad_ignores_packets() {
    let mut vad = Vad::<FullSimd>::new(VadConfig {
        enabled: false,
        mode: String::from(""),
        threshold: 0,
        max_silence_bytes: 0,
    });
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);
    assert!(vad.process(&silence));
}

// ---------------------------------------------------------------------------
// DynamicVad dispatch test
// ---------------------------------------------------------------------------

#[test]
fn dynamic_vad_dispatch_works() {
    let config = make_vad_config(8000);
    let mut dyn_vad = DynamicVad::FullSimd(Vad::<FullSimd>::new(config));
    let loud = generate_buffer(BUFFER_SIZE, 16, 2, false, 200);
    let silence = generate_buffer(BUFFER_SIZE, 16, 2, true, 0);

    // Use the dispatch_vad! macro
    assert!(dispatch_vad!(&mut dyn_vad, |v| v.process(&loud)));
    dispatch_vad!(&mut dyn_vad, |v| v.force_idle());
    assert!(!dispatch_vad!(&mut dyn_vad, |v| v.process(&silence)));
}
