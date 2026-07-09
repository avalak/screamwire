use screamwire::vad::{AudioDepth, Depth16, Depth24, Depth32, Vad, VadConfig};
use screamwire_common::scream::AUDIO_PAYLOAD_SIZE;

/// Silence packet
fn silent_packet(_bits: u32) -> Vec<u8> {
    vec![0u8; AUDIO_PAYLOAD_SIZE]
}

/// Loud packet
fn loud_packet(bits: u32, channels: u32, peak: u16) -> Vec<u8> {
    let sample_bytes = (bits / 8) as usize;
    let _frame_bytes = sample_bytes * channels as usize;
    let mut data = vec![0u8; AUDIO_PAYLOAD_SIZE];

    // Put the peak in the very first sample (leftmost channel, frame 0)
    match bits {
        16 => {
            let s = peak as i16;
            data[..2].copy_from_slice(&s.to_le_bytes());
        }
        24 => {
            let raw = (peak as u32) & 0x00FF_FFFF;
            let bytes = raw.to_le_bytes();
            data[..3].copy_from_slice(&bytes[..3]);
        }
        32 => {
            let s = peak as i32;
            let bytes = s.to_le_bytes();
            data[..4].copy_from_slice(&bytes);
        }
        _ => panic!("unsupported bits"),
    }
    data
}

/// Make VAD with provided config
fn make_vad<D: AudioDepth>(threshold: u16, silence_packets: u32) -> Vad<D> {
    let config = VadConfig {
        threshold,
        silence_packets,
    };
    Vad::<D>::new(config)
}

#[test]
fn test_silence_detected_16bit() {
    let mut vad = make_vad::<Depth16>(1, 2);
    let pkt = silent_packet(16);

    // First silent packet – still active
    let send = vad.process(&pkt);
    assert!(send, "Still active after one silent packet");

    // Second silent packet – should trigger pause
    let send = vad.process(&pkt);
    assert!(!send, "Should pause after two silent packets");
}

#[test]
fn test_signal_detected_16bit() {
    let mut vad = make_vad::<Depth16>(1, 2);
    let loud = loud_packet(16, 2, 100);

    // Should detect signal immediately
    let send = vad.process(&loud);
    assert!(send, "Signal detected, should send");
}

#[test]
fn test_signal_detected_24bit() {
    let mut vad = make_vad::<Depth24>(1, 2);
    let loud = loud_packet(24, 2, 100);

    let send = vad.process(&loud);
    assert!(send, "24‑bit signal detected");
}

#[test]
fn test_signal_detected_32bit() {
    let mut vad = make_vad::<Depth32>(1, 2);
    let loud = loud_packet(32, 2, 100);

    let send = vad.process(&loud);
    assert!(send, "32‑bit signal detected");
}

#[test]
fn test_resume_after_silence() {
    let mut vad = make_vad::<Depth16>(10, 2);
    let silent = silent_packet(16);
    let loud = loud_packet(16, 2, 200);

    // Feed two silent packets to pause
    vad.process(&silent);
    let send = vad.process(&silent);
    assert!(!send, "Paused after two silent packets");

    // Feed a loud packet to resume
    let send = vad.process(&loud);
    assert!(send, "Resumed after loud packet");
}

#[test]
fn test_counter_reset_on_signal() {
    let mut vad = make_vad::<Depth16>(10, 5);
    let silent = silent_packet(16);
    let loud = loud_packet(16, 2, 200);

    // One silent, then a loud – counter should reset
    vad.process(&silent);
    vad.process(&loud);

    // Now three more silents should not yet trigger pause
    for _ in 0..3 {
        let send = vad.process(&silent);
        assert!(send, "Still active, silence count reset");
    }
}

#[test]
fn test_vad_disabled_with_zero_threshold() {
    let mut vad = make_vad::<Depth16>(0, 2); // threshold 0 disables VAD
    let silent = silent_packet(16);

    // Should always send, no matter how many silent packets
    for _ in 0..10 {
        let send = vad.process(&silent);
        assert!(send, "VAD disabled, always send");
    }
}
