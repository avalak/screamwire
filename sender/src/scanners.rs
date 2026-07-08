use std::convert::TryInto;

/// Compare to zero
#[inline(always)]
pub fn scan_generic_silence_fold(packet: &[u8], _threshold: u32) -> bool {
    packet.iter().fold(0u8, |acc, &b| acc | b) != 0
}

/// 16‑bit
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
#[inline(always)]
pub fn scan_24bit_any(packet: &[u8], threshold: u32) -> bool {
    packet.chunks_exact(3).any(|ch| {
        let raw = i32::from_le_bytes([0, ch[0], ch[1], ch[2]]);
        let sample = raw >> 8;
        sample.unsigned_abs() > threshold
    })
}

/// 8‑bit; normally not used
#[inline(always)]
pub fn scan_8bit_any(packet: &[u8], threshold: u32) -> bool {
    let t = threshold as u8;
    packet.iter().any(|&b| (b as i8).unsigned_abs() > t)
}

#[allow(dead_code)]
#[inline(always)]
pub fn scan_disabled(_packet: &[u8], _threshold: u32) -> bool {
    true
}
