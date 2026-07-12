#[cfg(target_pointer_width = "64")]
type Word = u64;
#[cfg(target_pointer_width = "64")]
const WORD_BYTES: usize = 8;
#[cfg(target_pointer_width = "32")]
type Word = u32;
#[cfg(target_pointer_width = "32")]
const WORD_BYTES: usize = 4;

/// Compare to zero
/// Stride scan
#[inline(always)]
pub fn any_strided_nonzero_1024(packet: &[u8], _threshold: u32) -> bool {
    let len = packet.len();
    let mut i = 0;

    // Full S16_LE, S24_LE, S32_LE support
    // Step: 4x256 == 1024
    while i + 1024 <= len {
        unsafe {
            let ptr = packet.as_ptr().add(i);
            let p0 = ptr as *const Word;
            let p1 = ptr.add(256) as *const Word;
            let p2 = ptr.add(512) as *const Word;
            let p3 = ptr.add(768) as *const Word;

            if (p0.read_unaligned()
                | p1.read_unaligned()
                | p2.read_unaligned()
                | p3.read_unaligned())
                != 0
            {
                return true;
            }
        }
        i += 1024;
    }

    // Tail
    while i + WORD_BYTES <= len {
        unsafe {
            let p = packet.as_ptr().add(i) as *const Word;
            if p.read_unaligned() != 0 {
                return true;
            }
        }
        i += 64;
    }

    false
}

// SIMD
#[allow(dead_code)]
#[inline(always)]
pub fn scan_generic_silence_fold(packet: &[u8], _threshold: u32) -> bool {
    packet.iter().fold(0u8, |acc, &b| acc | b) != 0
}

#[allow(dead_code)]
#[inline(always)]
pub fn scan_generic_silence_simd(packet: &[u8], _threshold: u32) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        packet.iter().fold(0u8, |acc, &b| acc | b) != 0
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        packet.iter().fold(0u8, |acc, &b| acc | b) != 0
    }
}
