//! AVX2 batch multiply-reduce kernels for Mersenne primes.
//!
//! This module provides SIMD batch multiplication over small Mersenne prime
//! fields — specifically `M31 = 2^31 - 1`. The kernel operates on packed
//! `u32` lanes (8 lanes per 256-bit vector) and amortises the modular
//! reduction across all lanes, beating scalar Montgomery for bulk
//! multiplication workloads.
//!
//! # Algorithm (M31)
//!
//! For `a, b ∈ [0, 2^31 - 1)`, the product `a·b` fits in 62 bits. Writing
//! `p = a·b = hi · 2^31 + lo` with `lo < 2^31`, the identity `2^31 ≡ 1
//! (mod 2^31 - 1)` gives
//!
//! ```text
//! a·b ≡ lo + hi  (mod 2^31 - 1),
//! ```
//!
//! with `lo + hi ≤ 2·(2^31 - 2) < 2^32` — one conditional subtract
//! canonicalises to `[0, 2^31 - 1)`.
//!
//! AVX2 lacks a native `u32 × u32 → u32` widening multiply. We use
//! `_mm256_mul_epu32`, which multiplies the even-indexed 32-bit lanes of
//! two 256-bit vectors and produces four 64-bit products. We apply the
//! reduction to that vector of four products, then repeat for the odd
//! lanes (shifted into even position via `_mm256_srli_epi64`).  Finally
//! we blend the two result vectors back into eight 32-bit lanes.
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure AVX2 is
//! available at runtime. The safe, dispatched entry points live in
//! `gf2-core` via the `MersenneFns` table returned by [`detect`].

#![allow(clippy::missing_safety_doc)]

use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// Packed M31 reduction kernel
// ---------------------------------------------------------------------------

/// Reduces four packed 64-bit products modulo `M31 = 2^31 - 1`, returning
/// four canonical `u32` values in the low 32 bits of each 64-bit lane.
///
/// Each input lane `p` must satisfy `p < 2^62` (which holds for any product
/// of two canonical M31 values).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn reduce_m31_64(p: __m256i) -> __m256i {
    // 2^31 - 1 in the low 32 bits of every 64-bit lane.
    let p31 = _mm256_set1_epi64x((1i64 << 31) - 1);
    // First fold: p = hi·2^31 + lo ⇒ r = lo + hi (mod M31), r < 2^32.
    let lo = _mm256_and_si256(p, p31);
    let hi = _mm256_srli_epi64(p, 31);
    let sum = _mm256_add_epi64(lo, hi);

    // sum may be up to (2^31 - 1) + (2^31 - 1) = 2^32 - 2 — still a single
    // u32's worth. One more fold handles the overflow bit.
    let lo2 = _mm256_and_si256(sum, p31);
    let hi2 = _mm256_srli_epi64(sum, 31);
    let r = _mm256_add_epi64(lo2, hi2);

    // r ∈ [0, 2·(2^31 - 1)). Conditional subtract to canonicalise.
    // Use branchless masked subtract: if r >= M31, subtract M31.
    let p31_vec = _mm256_set1_epi64x((1i64 << 31) - 1);
    // ge = (r >= M31); we compute this as !(r < M31). AVX2 lacks unsigned
    // compare, but all values fit in 33 bits ≪ 2^63 so signed 64-bit cmp
    // is correct.
    let lt = _mm256_cmpgt_epi64(p31_vec, r); // lt mask where M31 > r (i.e. r < M31)
                                             // mask_sub is -1 where we must subtract, 0 otherwise.
    let ones = _mm256_set1_epi64x(-1);
    let ge_mask = _mm256_xor_si256(lt, ones);
    let to_sub = _mm256_and_si256(ge_mask, p31_vec);
    _mm256_sub_epi64(r, to_sub)
}

/// Multiplies two 256-bit vectors of eight packed `u32` M31 canonical
/// values lane-wise and returns eight canonical results packed as `u32`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical (`< 2^31 - 1`).
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn mersenne31_batch_mul8(a: __m256i, b: __m256i) -> __m256i {
    // Even-lane products: mul_epu32 takes the low 32 bits of each 64-bit
    // lane — lanes {0, 2, 4, 6} of the 8×u32 view — and produces four
    // 64-bit products.
    let prod_even = _mm256_mul_epu32(a, b);
    let red_even = reduce_m31_64(prod_even);

    // Odd-lane products: shift each 64-bit lane right by 32 bits so the
    // odd lanes move into the low half, then multiply.
    let a_odd = _mm256_srli_epi64(a, 32);
    let b_odd = _mm256_srli_epi64(b, 32);
    let prod_odd = _mm256_mul_epu32(a_odd, b_odd);
    let red_odd = reduce_m31_64(prod_odd);

    // Blend: red_even has results in low-32 of each 64-bit lane (lanes
    // 0, 2, 4, 6). red_odd has results in low-32 of each 64-bit lane
    // (should populate lanes 1, 3, 5, 7). Shift red_odd left 32 bits and
    // OR with red_even.
    let odd_shifted = _mm256_slli_epi64(red_odd, 32);
    _mm256_or_si256(red_even, odd_shifted)
}

// ---------------------------------------------------------------------------
// Public batch entry point
// ---------------------------------------------------------------------------

/// Batch lane-wise multiplication for `Fp<2^31 - 1>`.
///
/// Computes `out[i] = a[i] * b[i] mod (2^31 - 1)` for all `i`.
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical M31 values (`< 2^31 - 1`).
///   Must have the same length.
/// * `out` — output slice (same length as `a` and `b`).
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime. All input values
/// must be canonical (strictly less than `2^31 - 1`); behaviour is
/// undefined otherwise.
///
/// # Panics
///
/// Panics if the slice lengths differ.
#[target_feature(enable = "avx2")]
pub unsafe fn mersenne31_batch_mul(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len(), "mersenne31_batch_mul: length mismatch");
    assert_eq!(a.len(), out.len(), "mersenne31_batch_mul: output length");

    let n = a.len();
    let nvec = n / 8;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = mersenne31_batch_mul8(av, bv);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    // Scalar tail for `n % 8` remaining elements.
    let tail_start = nvec * 8;
    for i in tail_start..n {
        let prod = (*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64);
        // One fold suffices because prod < 2^62.
        let lo = (prod as u32) & ((1u32 << 31) - 1);
        let hi = (prod >> 31) as u32;
        let s = lo.wrapping_add(hi);
        let r = (s & ((1u32 << 31) - 1)).wrapping_add(s >> 31);
        let p31 = (1u32 << 31) - 1;
        *out.get_unchecked_mut(i) = if r >= p31 { r - p31 } else { r };
    }
}

/// Batch lane-wise multiply-and-accumulate for `Fp<2^31 - 1>`.
///
/// Computes `acc[i] += a[i] * b[i] mod (2^31 - 1)`. Useful for dot-product
/// style reductions where a running sum is maintained. Each accumulator
/// lane is reduced after every update.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and all inputs are canonical
/// (`< 2^31 - 1`).
///
/// # Panics
///
/// Panics if slice lengths differ.
#[target_feature(enable = "avx2")]
pub unsafe fn mersenne31_batch_mul_add(a: &[u32], b: &[u32], acc: &mut [u32]) {
    assert_eq!(
        a.len(),
        b.len(),
        "mersenne31_batch_mul_add: length mismatch"
    );
    assert_eq!(a.len(), acc.len(), "mersenne31_batch_mul_add: acc length");

    let n = a.len();
    let nvec = n / 8;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let c_ptr = acc.as_mut_ptr() as *mut __m256i;

    let p31_vec = _mm256_set1_epi32(((1u32 << 31) - 1) as i32);

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let cv = _mm256_loadu_si256(c_ptr.add(i));
        let rv = mersenne31_batch_mul8(av, bv);
        // Add in 32-bit lanes (each < 2^31), then canonicalise. The sum
        // is at most 2·(2^31 - 2) < 2^32, so a single conditional subtract
        // suffices. Implement the conditional as `min_u32(sum, sum - M31)`:
        // when `sum >= M31`, `sum - M31 < sum` (valid unsigned); when
        // `sum < M31`, `sum - M31` wraps to a very large u32 so `min`
        // keeps the original `sum`. Both branches are branchless and use
        // only AVX2 intrinsics.
        let sum = _mm256_add_epi32(rv, cv);
        let minned = _mm256_min_epu32(sum, _mm256_sub_epi32(sum, p31_vec));
        _mm256_storeu_si256(c_ptr.add(i), minned);
    }

    let tail_start = nvec * 8;
    let p31 = (1u32 << 31) - 1;
    for i in tail_start..n {
        let prod = (*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64);
        let lo = (prod as u32) & p31;
        let hi = (prod >> 31) as u32;
        let s = lo.wrapping_add(hi);
        let r = (s & p31).wrapping_add(s >> 31);
        let mul = if r >= p31 { r - p31 } else { r };
        let sum = mul.wrapping_add(*acc.get_unchecked(i));
        *acc.get_unchecked_mut(i) = if sum >= p31 { sum - p31 } else { sum };
    }
}

/// Batch dot product for `Fp<2^31 - 1>`.
///
/// Computes `sum(a[i] * b[i]) mod (2^31 - 1)`, returning the canonical
/// sum as a single `u32`. Uses 64-bit lane accumulators and reduces once
/// at the end, amortising the fold across the entire vector.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical
/// (`< 2^31 - 1`).
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
#[target_feature(enable = "avx2")]
pub unsafe fn mersenne31_batch_dot(a: &[u32], b: &[u32]) -> u32 {
    assert_eq!(a.len(), b.len(), "mersenne31_batch_dot: length mismatch");
    let n = a.len();
    let nvec = n / 8;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;

    // We accumulate 8 reduced u32 results into 4 pairs of 64-bit lanes.
    // Each reduced value is < 2^31, so summing up to 2^33 of them in each
    // 64-bit accumulator is safe. Since nvec ≤ len/8 ≤ 2^60 in practice,
    // we just reduce each step to be safe.
    let mut acc_lo = _mm256_setzero_si256();
    let mut acc_hi = _mm256_setzero_si256();

    let mask32 = _mm256_set1_epi64x(0xFFFF_FFFFi64);
    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = mersenne31_batch_mul8(av, bv);
        // rv has 8 canonical u32 values. Split into even/odd halves as u64
        // and accumulate.
        let even64 = _mm256_and_si256(rv, mask32);
        let odd64 = _mm256_srli_epi64(rv, 32);
        acc_lo = _mm256_add_epi64(acc_lo, even64);
        acc_hi = _mm256_add_epi64(acc_hi, odd64);
    }

    // Sum of acc_lo + acc_hi across all 4 lanes, then reduce.
    let combined = _mm256_add_epi64(acc_lo, acc_hi);
    // Horizontal sum of 4×u64 lanes into a single u64. acc is at most
    // 2^32 per lane × 2^32 iterations ≤ 2^64 worst case, but in practice
    // n ≤ 2^30 so we are safe well below u64 overflow.
    let lo128 = _mm256_castsi256_si128(combined);
    let hi128 = _mm256_extracti128_si256(combined, 1);
    let sum128 = _mm_add_epi64(lo128, hi128);

    // Extract two 64-bit halves.
    let s0 = _mm_cvtsi128_si64(sum128) as u64;
    let sum128_hi = _mm_srli_si128(sum128, 8);
    let s1 = _mm_cvtsi128_si64(sum128_hi) as u64;

    let mut total: u64 = s0.wrapping_add(s1);

    // Reduce total modulo M31 via iterated fold.
    let p31: u64 = (1u64 << 31) - 1;
    while total >= (1u64 << 31) {
        total = (total & p31) + (total >> 31);
    }
    if total >= p31 {
        total -= p31;
    }

    // Tail: add scalar products.
    let tail_start = nvec * 8;
    for i in tail_start..n {
        let prod = (*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64);
        let lo = prod & p31;
        let hi = prod >> 31;
        let s = lo + hi;
        let r = (s & p31) + (s >> 31);
        let mul = if r >= p31 { r - p31 } else { r };
        total += mul;
        if total >= p31 {
            total -= p31;
        }
    }

    total as u32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const P31: u32 = (1u32 << 31) - 1;

    fn scalar_m31_mul(a: u32, b: u32) -> u32 {
        ((a as u64 * b as u64) % P31 as u64) as u32
    }

    #[test]
    fn batch_mul_exact_multiple_of_8() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u32> = (0..16u32).map(|i| i * 12345 % P31).collect();
        let b: Vec<u32> = (0..16u32).map(|i| i * 67890 % P31).collect();
        let mut out = vec![0u32; 16];
        unsafe { mersenne31_batch_mul(&a, &b, &mut out) };
        for i in 0..16 {
            assert_eq!(out[i], scalar_m31_mul(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_mul_with_tail() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u32> = (0..13u32).map(|i| i * 12345 % P31).collect();
        let b: Vec<u32> = (0..13u32).map(|i| i * 67890 % P31).collect();
        let mut out = vec![0u32; 13];
        unsafe { mersenne31_batch_mul(&a, &b, &mut out) };
        for i in 0..13 {
            assert_eq!(out[i], scalar_m31_mul(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_mul_boundary_values() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a = vec![0u32, 1, P31 - 1, P31 / 2, 1, P31 - 1, 0, P31 / 3];
        let b = vec![P31 - 1, 0, P31 - 1, 2, P31 / 2, 1, 0, 3];
        let mut out = vec![0u32; 8];
        unsafe { mersenne31_batch_mul(&a, &b, &mut out) };
        for i in 0..8 {
            assert_eq!(out[i], scalar_m31_mul(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_dot_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0, 1, 7, 8, 9, 16, 17, 100, 1024] {
            let a: Vec<u32> = (0..len as u32)
                .map(|i| (i.wrapping_mul(17)) % P31)
                .collect();
            let b: Vec<u32> = (0..len as u32)
                .map(|i| (i.wrapping_mul(23) + 5) % P31)
                .collect();
            let got = unsafe { mersenne31_batch_dot(&a, &b) };
            let mut expected: u64 = 0;
            for i in 0..len {
                expected = (expected + (a[i] as u64 * b[i] as u64) % P31 as u64) % P31 as u64;
            }
            assert_eq!(got as u64, expected, "len={len}");
        }
    }

    #[test]
    fn batch_mul_add_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u32> = (0..16u32).map(|i| (i * 17) % P31).collect();
        let b: Vec<u32> = (0..16u32).map(|i| (i * 29 + 3) % P31).collect();
        let initial_acc: Vec<u32> = (0..16u32).map(|i| (i * 31) % P31).collect();
        let mut acc = initial_acc.clone();
        unsafe { mersenne31_batch_mul_add(&a, &b, &mut acc) };
        for i in 0..16 {
            let m = scalar_m31_mul(a[i], b[i]);
            let expected = (m + initial_acc[i]) % P31;
            assert_eq!(acc[i], expected, "i={i}");
        }
    }
}
