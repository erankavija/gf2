//! AVX2 batch entry points for the F_7 4-bit-packed encoding.
//!
//! One `u64` holds 16 F_7 elements in 4-bit slots, mapped through the scalar
//! LUTs of [`crate::bipedal::packed7`]. All slices of one call share a length
//! `n` with `n % 4 == 0`; `n = 0` is a no-op.

use crate::bipedal::packed7::{binary7_op_word, neg7_word, ADD7_LUT, MUL7_LUT, SUB7_LUT};

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Apply a binary F_7 LUT op to 4 u64 words packed in one AVX2 register.
///
/// # Safety
///
/// AVX2 must be available at runtime (caller's precondition via inlining into
/// a `#[target_feature(enable = "avx2")]` function).
#[inline(always)]
unsafe fn binary7_avx2_lane(a: __m256i, b: __m256i, lut: &[u8; 65536]) -> __m256i {
    let a0 = _mm256_extract_epi64(a, 0) as u64;
    let a1 = _mm256_extract_epi64(a, 1) as u64;
    let a2 = _mm256_extract_epi64(a, 2) as u64;
    let a3 = _mm256_extract_epi64(a, 3) as u64;
    let b0 = _mm256_extract_epi64(b, 0) as u64;
    let b1 = _mm256_extract_epi64(b, 1) as u64;
    let b2 = _mm256_extract_epi64(b, 2) as u64;
    let b3 = _mm256_extract_epi64(b, 3) as u64;
    let r0 = binary7_op_word(a0, b0, lut) as i64;
    let r1 = binary7_op_word(a1, b1, lut) as i64;
    let r2 = binary7_op_word(a2, b2, lut) as i64;
    let r3 = binary7_op_word(a3, b3, lut) as i64;
    _mm256_set_epi64x(r3, r2, r1, r0)
}

/// Apply the F_7 neg op to 4 u64 words packed in one AVX2 register.
///
/// # Safety
///
/// AVX2 must be available at runtime (caller's precondition via inlining into
/// a `#[target_feature(enable = "avx2")]` function).
#[inline(always)]
unsafe fn neg7_avx2_lane(a: __m256i) -> __m256i {
    let a0 = _mm256_extract_epi64(a, 0) as u64;
    let a1 = _mm256_extract_epi64(a, 1) as u64;
    let a2 = _mm256_extract_epi64(a, 2) as u64;
    let a3 = _mm256_extract_epi64(a, 3) as u64;
    let r0 = neg7_word(a0) as i64;
    let r1 = neg7_word(a1) as i64;
    let r2 = neg7_word(a2) as i64;
    let r3 = neg7_word(a3) as i64;
    _mm256_set_epi64x(r3, r2, r1, r0)
}

#[inline(always)]
unsafe fn load256(src: &[u64], offset: usize) -> __m256i {
    // SAFETY: caller ensures offset + 4 <= src.len() and AVX2 available.
    _mm256_loadu_si256(src.as_ptr().add(offset) as *const __m256i)
}

#[inline(always)]
unsafe fn store256(dst: &mut [u64], offset: usize, v: __m256i) {
    // SAFETY: caller ensures offset + 4 <= dst.len() and AVX2 available.
    _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v)
}

/// Apply F_7 add over packed word streams via AVX2.
///
/// Each AVX2 lane covers 4 u64 words (= 64 F_7 elements). All three slices
/// (`a`, `b`, `out`) must have the same length `n` where `n % 4 == 0`. Empty
/// input is allowed (no-op).
///
/// # Safety
///
/// AVX2 must be available at runtime. All three slices share the same length
/// divisible by 4. Behaviour is undefined otherwise.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_add7_batch(a: &[u64], b: &[u64], out: &mut [u64]) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(a.len() % 4, 0);
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    let n = a.len();
    let mut i = 0usize;
    while i < n {
        let va = load256(a, i);
        let vb = load256(b, i);
        let vr = binary7_avx2_lane(va, vb, &ADD7_LUT);
        store256(out, i, vr);
        i += 4;
    }
}

/// Apply F_7 sub over packed word streams via AVX2.
///
/// See [`run_add7_batch`] for the slice-shape contract.
///
/// # Safety
///
/// AVX2 must be available at runtime. All three slices share the same length
/// divisible by 4.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_sub7_batch(a: &[u64], b: &[u64], out: &mut [u64]) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(a.len() % 4, 0);
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    let n = a.len();
    let mut i = 0usize;
    while i < n {
        let va = load256(a, i);
        let vb = load256(b, i);
        let vr = binary7_avx2_lane(va, vb, &SUB7_LUT);
        store256(out, i, vr);
        i += 4;
    }
}

/// Apply F_7 mul over packed word streams via AVX2.
///
/// See [`run_add7_batch`] for the slice-shape contract.
///
/// # Safety
///
/// AVX2 must be available at runtime. All three slices share the same length
/// divisible by 4.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_mul7_batch(a: &[u64], b: &[u64], out: &mut [u64]) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(a.len() % 4, 0);
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    let n = a.len();
    let mut i = 0usize;
    while i < n {
        let va = load256(a, i);
        let vb = load256(b, i);
        let vr = binary7_avx2_lane(va, vb, &MUL7_LUT);
        store256(out, i, vr);
        i += 4;
    }
}

/// Apply F_7 neg over packed word streams via AVX2.
///
/// # Safety
///
/// AVX2 must be available at runtime. Both slices share the same length
/// divisible by 4.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_neg7_batch(a: &[u64], out: &mut [u64]) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(a.len() % 4, 0);
    debug_assert_eq!(a.len(), out.len());
    let n = a.len();
    let mut i = 0usize;
    while i < n {
        let va = load256(a, i);
        let vr = neg7_avx2_lane(va);
        store256(out, i, vr);
        i += 4;
    }
}
