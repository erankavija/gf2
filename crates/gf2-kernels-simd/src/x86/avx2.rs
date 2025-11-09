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
    for j in (nvec * 4)..len {
        let x = buf.get_unchecked(j);
        *buf.get_unchecked_mut(j) = !*x;
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_popcnt(buf: &[u64]) -> u64 {
    // Minimal: scalar popcount for correctness. SIMD popcnt can be added later.
    buf.iter().map(|w| w.count_ones() as u64).sum()
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
