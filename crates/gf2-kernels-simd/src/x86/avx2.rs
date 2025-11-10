#![allow(clippy::many_single_char_names)]
use crate::LogicalFns;
use core::arch::x86_64::*;

#[inline(always)]
unsafe fn loadu(ptr: *const u8) -> __m256i {
    _mm256_loadu_si256(ptr as *const __m256i)
}

#[inline(always)]
unsafe fn storeu(ptr: *mut u8, v: __m256i) {
    _mm256_storeu_si256(ptr as *mut __m256i, v)
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_xor_into(dst: &mut [u64], src: &[u64]) {
    let len = dst.len().min(src.len());
    let nvec = len / 4; // 4 u64 per 256-bit vector
    let dst_ptr = dst.as_mut_ptr() as *mut u8;
    let src_ptr = src.as_ptr() as *const u8;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(dst_ptr.offset(off));
        let b = loadu(src_ptr.offset(off));
        let r = _mm256_xor_si256(a, b);
        storeu(dst_ptr.offset(off), r);
        i += 1;
    }
    for j in (nvec * 4)..len {
        *dst.get_unchecked_mut(j) ^= *src.get_unchecked(j);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_and_into(dst: &mut [u64], src: &[u64]) {
    let len = dst.len().min(src.len());
    let nvec = len / 4;
    let dst_ptr = dst.as_mut_ptr() as *mut u8;
    let src_ptr = src.as_ptr() as *const u8;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(dst_ptr.offset(off));
        let b = loadu(src_ptr.offset(off));
        let r = _mm256_and_si256(a, b);
        storeu(dst_ptr.offset(off), r);
        i += 1;
    }
    for j in (nvec * 4)..len {
        *dst.get_unchecked_mut(j) &= *src.get_unchecked(j);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_or_into(dst: &mut [u64], src: &[u64]) {
    let len = dst.len().min(src.len());
    let nvec = len / 4;
    let dst_ptr = dst.as_mut_ptr() as *mut u8;
    let src_ptr = src.as_ptr() as *const u8;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(dst_ptr.offset(off));
        let b = loadu(src_ptr.offset(off));
        let r = _mm256_or_si256(a, b);
        storeu(dst_ptr.offset(off), r);
        i += 1;
    }
    for j in (nvec * 4)..len {
        *dst.get_unchecked_mut(j) |= *src.get_unchecked(j);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_not_into(buf: &mut [u64]) {
    let len = buf.len();
    let nvec = len / 4;
    let ptr = buf.as_mut_ptr() as *mut u8;
    let ones = _mm256_set1_epi64x(-1);
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(ptr.offset(off));
        let r = _mm256_xor_si256(a, ones);
        storeu(ptr.offset(off), r);
        i += 1;
    }
    // Tail: avoid aliasing &/&mut; use raw pointer arithmetic
    let p = buf.as_mut_ptr();
    for j in (nvec * 4)..len {
        let v = *p.add(j);
        *p.add(j) = !v;
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_popcnt(buf: &[u64]) -> u64 {
    if buf.is_empty() { return 0; }
    // Byte-wise popcount via nibble LUT + vpshufb, then widen-sum with vpsadbw.
    let ptr = buf.as_ptr() as *const u8;
    let nbytes = buf.len() * 8;

    let lut = _mm256_setr_epi8(
        0,1,1,2,1,2,2,3,1,2,2,3,2,3,3,4,
        0,1,1,2,1,2,2,3,1,2,2,3,2,3,3,4,
    );
    let mask0f = _mm256_set1_epi8(0x0f);
    let mut acc = _mm256_setzero_si256();

    let nvec = nbytes / 32;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let v = loadu(ptr.offset(off));
        let lo = _mm256_and_si256(v, mask0f);
        let hi = _mm256_and_si256(_mm256_srli_epi16(v, 4), mask0f);
        let pc_lo = _mm256_shuffle_epi8(lut, lo);
        let pc_hi = _mm256_shuffle_epi8(lut, hi);
        let pc = _mm256_add_epi8(pc_lo, pc_hi);
        // Sum bytes to 64-bit lanes
        acc = _mm256_add_epi64(acc, _mm256_sad_epu8(pc, _mm256_setzero_si256()));
        i += 1;
    }

    // Horizontal add acc's four 64-bit lanes
    let acc_lo = _mm256_castsi256_si128(acc);
    let acc_hi = _mm256_extracti128_si256(acc, 1);
    let acc128 = _mm_add_epi64(acc_lo, acc_hi);
    let mut total = _mm_cvtsi128_si64(acc128) as u64;
    // Avoid _mm_extract_epi64 (SSE4.1); instead shift right by 8 bytes and read low 64
    let acc128_hi = _mm_srli_si128(acc128, 8);
    total += _mm_cvtsi128_si64(acc128_hi) as u64;

    // Tail bytes
    let rem = nbytes & 31;
    if rem != 0 {
        let tail_ptr = ptr.add(nvec * 32);
        for k in 0..rem {
            total += (*tail_ptr.add(k)).count_ones() as u64;
        }
    }

    total
}

pub(crate) fn fns() -> LogicalFns {
    // Provide safe wrappers that call into the unsafe AVX2 fns.
    fn and_fn(dst: &mut [u64], src: &[u64]) {
        if dst.is_empty() { return; }
        unsafe { avx2_and_into(dst, src) }
    }
    fn or_fn(dst: &mut [u64], src: &[u64]) {
        if dst.is_empty() { return; }
        unsafe { avx2_or_into(dst, src) }
    }
    fn xor_fn(dst: &mut [u64], src: &[u64]) {
        if dst.is_empty() { return; }
        unsafe { avx2_xor_into(dst, src) }
    }
    fn not_fn(dst: &mut [u64]) {
        if dst.is_empty() { return; }
        unsafe { avx2_not_into(dst) }
    }
    fn popcnt_fn(src: &[u64]) -> u64 {
        unsafe { avx2_popcnt(src) }
    }
    LogicalFns { and_fn, or_fn, xor_fn, not_fn, popcnt_fn }
}
