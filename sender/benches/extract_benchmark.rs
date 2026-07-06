use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const AUDIO_PAYLOAD_SIZE: usize = 1152;
const HEADER_SIZE: usize = 4;
const PACKET_SIZE: usize = HEADER_SIZE + AUDIO_PAYLOAD_SIZE;

/// Ring buffer that can be either contiguous or split.
struct RingBuffer {
    slice1: Vec<u8>,
    slice2: Vec<u8>,
}

impl RingBuffer {
    fn contiguous() -> Self {
        Self {
            slice1: vec![0xAAu8; AUDIO_PAYLOAD_SIZE],
            slice2: vec![],
        }
    }

    fn split() -> Self {
        Self {
            slice1: vec![0xAAu8; 500],
            slice2: vec![0xAAu8; 652],
        }
    }

    fn as_slices(&self) -> (&[u8], &[u8]) {
        (&self.slice1, &self.slice2)
    }
}

/// A lightweight VAD stub whose behaviour is controlled by `has_signal`.
#[inline(never)]
fn mock_vad_process(_payload: &[u8], has_signal: bool) -> (bool, u64) {
    if has_signal { (true, 1) } else { (false, 10) }
}

// ---------------------------------------------------------------------------
// Variant 1 – Original: always copy to a local buffer before VAD & send
// ---------------------------------------------------------------------------
fn run_original(
    ring: &RingBuffer,
    packet: &mut [u8; PACKET_SIZE],
    has_signal: bool,
) -> (bool, u64) {
    let mut local_payload = [0u8; AUDIO_PAYLOAD_SIZE];
    let (slice1, slice2) = ring.as_slices();

    if slice1.len() >= AUDIO_PAYLOAD_SIZE {
        local_payload.copy_from_slice(&slice1[..AUDIO_PAYLOAD_SIZE]);
    } else {
        let first = slice1.len();
        local_payload[..first].copy_from_slice(slice1);
        local_payload[first..].copy_from_slice(&slice2[..AUDIO_PAYLOAD_SIZE - first]);
    }

    let (should_send, sleep_ms) = mock_vad_process(&local_payload, has_signal);

    if should_send {
        packet[HEADER_SIZE..].copy_from_slice(&local_payload);
    }

    (should_send, sleep_ms)
}

// ---------------------------------------------------------------------------
// Variant 2 – Hybrid: zero‑copy when contiguous, copy only on split
// ---------------------------------------------------------------------------
fn run_hybrid(ring: &RingBuffer, packet: &mut [u8; PACKET_SIZE], has_signal: bool) -> (bool, u64) {
    let (slice1, slice2) = ring.as_slices();

    if slice1.len() >= AUDIO_PAYLOAD_SIZE {
        let payload_slice = &slice1[..AUDIO_PAYLOAD_SIZE];
        let (send, sleep) = mock_vad_process(payload_slice, has_signal);
        if send {
            packet[HEADER_SIZE..].copy_from_slice(payload_slice);
        }
        (send, sleep)
    } else {
        let first = slice1.len();
        packet[HEADER_SIZE..HEADER_SIZE + first].copy_from_slice(slice1);
        packet[HEADER_SIZE + first..HEADER_SIZE + AUDIO_PAYLOAD_SIZE]
            .copy_from_slice(&slice2[..AUDIO_PAYLOAD_SIZE - first]);

        mock_vad_process(
            &packet[HEADER_SIZE..HEADER_SIZE + AUDIO_PAYLOAD_SIZE],
            has_signal,
        )
    }
}

// ---------------------------------------------------------------------------
// Benchmark
// ---------------------------------------------------------------------------
fn bench_extract(c: &mut Criterion) {
    let mut packet = [0u8; PACKET_SIZE];

    for has_signal in [true, false] {
        for buffer_type in ["Contiguous", "Split"] {
            let ring = if buffer_type == "Contiguous" {
                RingBuffer::contiguous()
            } else {
                RingBuffer::split()
            };

            let group_name = format!(
                "VAD_{}_Buffer_{}",
                if has_signal { "Voice" } else { "Silence" },
                buffer_type
            );

            let mut group = c.benchmark_group(group_name);

            group.bench_function("Original", |b| {
                b.iter(|| {
                    let _ = run_original(
                        black_box(&ring),
                        black_box(&mut packet),
                        black_box(has_signal),
                    );
                    black_box(&packet);
                })
            });

            group.bench_function("Hybrid", |b| {
                b.iter(|| {
                    let _ = run_hybrid(
                        black_box(&ring),
                        black_box(&mut packet),
                        black_box(has_signal),
                    );
                    black_box(&packet);
                })
            });

            group.finish();
        }
    }
}

criterion_group!(benches, bench_extract);
criterion_main!(benches);
