use std::convert::TryInto;

/// Compare to zero
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

#[cfg(target_pointer_width = "64")]
type Word = u64;
#[cfg(target_pointer_width = "64")]
const WORD_BYTES: usize = 8;
#[cfg(target_pointer_width = "32")]
type Word = u32;
#[cfg(target_pointer_width = "32")]
const WORD_BYTES: usize = 4;

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

/// 16‑bit
#[allow(dead_code)]
#[inline(always)]
pub fn scan_16bit_any(packet: &[u8], threshold: u32) -> bool {
    let (prefix, samples, suffix) = unsafe { packet.align_to::<i16>() };

    if samples
        .iter()
        .any(|&s| (s as i32).unsigned_abs() > threshold)
    {
        return true;
    }
    if !prefix.is_empty()
        && prefix.chunks_exact(2).any(|ch| {
            let s = i16::from_le_bytes(ch.try_into().unwrap());
            (s as i32).unsigned_abs() > threshold
        })
    {
        return true;
    }
    if !suffix.is_empty()
        && suffix.chunks_exact(2).any(|ch| {
            let s = i16::from_le_bytes(ch.try_into().unwrap());
            (s as i32).unsigned_abs() > threshold
        })
    {
        return true;
    }
    false
}

/// 32‑bit: same as 16-bit
#[allow(dead_code)]
#[inline(always)]
pub fn scan_32bit_any(packet: &[u8], threshold: u32) -> bool {
    let (prefix, samples, suffix) = unsafe { packet.align_to::<i32>() };

    if samples.iter().any(|&s| s.unsigned_abs() > threshold) {
        return true;
    }
    if !prefix.is_empty()
        && prefix.chunks_exact(4).any(|ch| {
            let s = i32::from_le_bytes(ch.try_into().unwrap());
            s.unsigned_abs() > threshold
        })
    {
        return true;
    }
    if !suffix.is_empty()
        && suffix.chunks_exact(4).any(|ch| {
            let s = i32::from_le_bytes(ch.try_into().unwrap());
            s.unsigned_abs() > threshold
        })
    {
        return true;
    }
    false
}

/// 24-bit
#[allow(dead_code)]
#[inline(always)]
pub fn scan_24bit_any(packet: &[u8], threshold: u32) -> bool {
    packet.chunks_exact(3).any(|ch| {
        let raw = i32::from_le_bytes([0, ch[0], ch[1], ch[2]]);
        let sample = raw >> 8;
        sample.unsigned_abs() > threshold
    })
}

/// 8‑bit; normally not used
#[allow(dead_code)]
#[inline(always)]
pub fn scan_8bit_any(packet: &[u8], threshold: u32) -> bool {
    let t = threshold as u8;
    packet.iter().any(|&b| (b as i8).unsigned_abs() > t)
}
