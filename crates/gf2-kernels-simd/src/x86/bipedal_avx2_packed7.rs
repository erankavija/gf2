//! AVX2 batch entry points for the F_7 4-bit-packed encoding.
//!
//! These are the actual SIMD-emitting functions for the F_7 kernel (R2
//! Candidate A). They operate on single `&[u64]` word streams — one word holds
//! 16 F_7 elements at 4-bit-aligned slots — and step by 4 u64 words (one AVX2
//! lane = 4 × u64 = 64 F_7 elements) per iteration.
//!
//! The 64 KiB compile-time LUTs from [`crate::bipedal::packed7`] are used
//! scalar-per-u64 inside each AVX2 lane (extract 4 u64s via `_mm256_extract_epi64`,
//! look them up, reassemble via `_mm256_set_epi64x`). This is the "per-lane
//! scalar LUT fallback inside a SIMD wrapper" option noted in the issue: it
//! reduces loop overhead and enables the framework's batch calling convention
//! while using the proven LUT for correctness.
//!
//! An AVX2-native approach using `_mm256_shuffle_epi8` (vpshufb) for 4-bit
//! lookups within a 16-byte nibble table would be faster but requires a
//! different LUT layout (16 × 16 nibble table vs 64 KiB byte-pair table) and
//! is deferred. AVX-512 gather paths (`_mm512_i32gather_epi32`) are also
//! deferred.
//!
//! Each `run_*7_batch` function is `#[target_feature(enable = "avx2")]`;
//! private helpers inherit the target feature from their callers via inlining
//! (`#[inline(always)]` only, no `#[target_feature]` — Rust 1.95 does not
//! allow combining both per issue #145574).
//!
//! ## Slice contract
//!
//! All three slices for binary ops (two inputs, one output) must have the same
//! length `n` where `n % 4 == 0`. Unary neg uses two slices. Empty slices
//! (`n = 0`) are allowed (no-op).

use crate::bipedal::packed7::{binary7_op_word, neg7_word, ADD7_LUT, MUL7_LUT, SUB7_LUT};

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// Per-lane LUT application helpers
// ---------------------------------------------------------------------------

/// Apply a binary F_7 LUT op to 4 u64 words packed in one AVX2 register.
///
/// Extracts 4 u64 values from `a` and `b`, applies `lut` per word, and
/// reassembles into an AVX2 register.
///
/// # Safety
///
/// AVX2 must be available at runtime (caller's precondition via inlining into
/// a `#[target_feature(enable = "avx2")]` function).
#[inline(always)]
unsafe fn binary7_avx2_lane(a: __m256i, b: __m256i, lut: &[u8; 65536]) -> __m256i {
    // Extract 4 u64 elements from each register.
    let a0 = _mm256_extract_epi64(a, 0) as u64;
    let a1 = _mm256_extract_epi64(a, 1) as u64;
    let a2 = _mm256_extract_epi64(a, 2) as u64;
    let a3 = _mm256_extract_epi64(a, 3) as u64;
    let b0 = _mm256_extract_epi64(b, 0) as u64;
    let b1 = _mm256_extract_epi64(b, 1) as u64;
    let b2 = _mm256_extract_epi64(b, 2) as u64;
    let b3 = _mm256_extract_epi64(b, 3) as u64;
    // Apply the scalar LUT word-by-word.
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

// ---------------------------------------------------------------------------
// Load / store helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public batch entry points
// ---------------------------------------------------------------------------

/// Apply F_7 add over packed word streams via AVX2.
///
/// Each AVX2 lane covers 4 u64 words (= 64 F_7 elements). All three slices
/// (`a`, `b`, `out`) must have the same length `n` where `n % 4 == 0`. Empty
/// input is allowed (no-op).
///
/// # Arguments
///
/// * `a` — first operand packed word slice (16 F_7 elements per word).
/// * `b` — second operand packed word slice.
/// * `out` — output packed word slice.
///
/// # Safety
///
/// AVX2 must be available at runtime. All three slices share the same length
/// divisible by 4. Behaviour is undefined otherwise.
///
/// # Complexity
///
/// `O(n / 4)` AVX2 lanes processed; each lane applies 4 scalar LUT ops.
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
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
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
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
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
/// All two slices (`a`, `out`) must have the same length `n` where `n % 4 == 0`.
///
/// # Safety
///
/// AVX2 must be available at runtime. Both slices share the same length
/// divisible by 4.
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
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

// AVX-512 gather paths (deferred). A `#[cfg(target_feature = "avx512f")]`
// block could use `_mm512_i32gather_epi32` with a scatter-gather index built
// from the nibble pairs, or better, use `vpshufb` with a reformatted 16-byte
// nibble LUT per 4-bit operation. Deferred per issue 1f769232 aspirational
// criterion note.
