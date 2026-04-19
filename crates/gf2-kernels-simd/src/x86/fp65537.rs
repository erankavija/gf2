//! AVX2 batch multiply-reduce kernels for the Fermat prime `Fp<65537>`.
//!
//! Targets the prime `P = 65537 = 2^16 + 1` — the fifth (and likely last)
//! known Fermat prime. This prime admits an exceptionally tight SIMD
//! reduction sequence on 8-lane 256-bit AVX2 because the modular
//! reduction collapses to a single subtract once the product is split
//! into high and low 16-bit halves.
//!
//! # Algorithm
//!
//! For `a, b ∈ [0, P) = [0, 65536]`, the product `a·b` fits in 33 bits
//! (`65536² = 2³²`). Writing
//!
//! ```text
//! a·b = hi · 2¹⁶ + lo   with   lo < 2¹⁶,   hi ≤ 2¹⁶
//! ```
//!
//! the identity `2¹⁶ ≡ -1 (mod 65537)` gives
//!
//! ```text
//! a·b ≡ lo - hi  (mod 65537).
//! ```
//!
//! Since `lo - hi ∈ [-65536, 65535]`, we add `P` to shift the sum into the
//! non-negative range, producing a value in `[1, 2P - 1]`, then apply
//! exactly one branchless conditional subtract of `P` to canonicalise.
//!
//! # Lane layout choice
//!
//! AVX2 lacks a native `u32 × u32 → u32` widening multiply. For inputs up
//! to `2¹⁶`, an `_mm256_mullo_epi32` product would wrap on the single
//! `65536 × 65536 = 2³²` boundary case, destroying correctness. We
//! therefore follow the Mersenne31 pattern: use `_mm256_mul_epu32`, which
//! multiplies the even-indexed u32 lanes of two 256-bit vectors to produce
//! four packed 64-bit products, apply the reduction per pair of products,
//! repeat for odd lanes, then blend the two 4-lane vectors back into a
//! single 8-lane u32 result. This yields **8 canonical u32 outputs per
//! 256-bit vector loop iteration**.
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure AVX2 is
//! available at runtime. Safe, dispatched entry points live in the parent
//! `fp65537.rs` module via the `Fp65537Fns` table returned by
//! `crate::fp65537::detect`.

#![allow(clippy::missing_safety_doc)]

use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// Packed reduction helper
// ---------------------------------------------------------------------------

/// Reduces four packed 64-bit products of canonical `Fp<65537>` values
/// modulo `65537`, returning four canonical `u32` results in the low 32
/// bits of each 64-bit lane.
///
/// Each input lane `p` must satisfy `p ≤ 2^32` — which holds for any
/// product of two canonical values (`max = 65536² = 2^32`).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn reduce_fp65537_64(p: __m256i) -> __m256i {
    // Mask for the low 16 bits of every 64-bit lane, and `P = 65537` in
    // the low 32 bits of every 64-bit lane.
    let mask16 = _mm256_set1_epi64x(0xFFFF);
    let p_vec = _mm256_set1_epi64x(65537);

    // Split each 64-bit lane into (hi, lo) halves at the 16-bit boundary.
    // lo = product & 0xFFFF, hi = product >> 16.
    let lo = _mm256_and_si256(p, mask16);
    let hi = _mm256_srli_epi64(p, 16);

    // r_shifted = lo + P - hi  (always non-negative since P > hi ≤ 2^16).
    // Range: [1, 2P - 1] — a single conditional subtract canonicalises.
    let lo_plus_p = _mm256_add_epi64(lo, p_vec);
    let r_shifted = _mm256_sub_epi64(lo_plus_p, hi);

    // Conditional subtract of P when r_shifted >= P.
    // Branchless via signed cmp: because r_shifted < 2·P < 2^17 ≪ 2^63,
    // signed and unsigned comparisons agree. Compute r_minus_p = r - P,
    // select r_minus_p if r >= P else r.
    let r_minus_p = _mm256_sub_epi64(r_shifted, p_vec);
    // mask = (r_shifted >= P) = NOT (P > r_shifted)
    let lt = _mm256_cmpgt_epi64(p_vec, r_shifted);
    let ones = _mm256_set1_epi64x(-1);
    let ge_mask = _mm256_xor_si256(lt, ones);
    // Select: (r_minus_p & ge_mask) | (r_shifted & !ge_mask)
    let take_minus = _mm256_and_si256(r_minus_p, ge_mask);
    let take_orig = _mm256_andnot_si256(ge_mask, r_shifted);
    _mm256_or_si256(take_minus, take_orig)
}

/// Multiplies two 256-bit vectors of eight packed canonical `Fp<65537>`
/// values lane-wise and returns eight canonical results packed as `u32`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical
/// (`< 65537`).
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn fp65537_batch_mul8(a: __m256i, b: __m256i) -> __m256i {
    // Even-lane products: mul_epu32 multiplies the low 32 bits of each
    // 64-bit lane — i.e. u32 lanes {0, 2, 4, 6} — and yields four 64-bit
    // products.
    let prod_even = _mm256_mul_epu32(a, b);
    let red_even = reduce_fp65537_64(prod_even);

    // Odd-lane products: shift each 64-bit lane right by 32 bits so the
    // odd u32 lanes move into the low half, then multiply.
    let a_odd = _mm256_srli_epi64(a, 32);
    let b_odd = _mm256_srli_epi64(b, 32);
    let prod_odd = _mm256_mul_epu32(a_odd, b_odd);
    let red_odd = reduce_fp65537_64(prod_odd);

    // Blend: red_even has its 32-bit results in lanes {0, 2, 4, 6};
    // red_odd has its results in lanes {0, 2, 4, 6} of its own vector
    // (because mul_epu32 writes into the low half of each 64-bit lane).
    // Shift red_odd left 32 bits to populate lanes {1, 3, 5, 7} and OR
    // with red_even.
    let odd_shifted = _mm256_slli_epi64(red_odd, 32);
    _mm256_or_si256(red_even, odd_shifted)
}

// ---------------------------------------------------------------------------
// Public batch entry points
// ---------------------------------------------------------------------------

/// Batch lane-wise multiplication for `Fp<65537>`.
///
/// Computes `out[i] = a[i] * b[i] mod 65537` for all `i`.
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical `Fp<65537>` values (`< 65537`).
///   Must have the same length.
/// * `out` — output slice (same length as `a` and `b`).
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime. All input values
/// must be canonical (strictly less than `65537`); behaviour is undefined
/// otherwise.
///
/// # Panics
///
/// Panics if the slice lengths differ.
///
/// # Complexity
///
/// O(n) with a vectorisation factor of 8 u32 lanes per 256-bit AVX2 vector.
#[target_feature(enable = "avx2")]
pub unsafe fn fp65537_batch_mul(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len(), "fp65537_batch_mul: length mismatch");
    assert_eq!(a.len(), out.len(), "fp65537_batch_mul: output length");

    let n = a.len();
    let nvec = n / 8;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp65537_batch_mul8(av, bv);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    // Scalar tail for the remaining `n % 8` elements.
    let tail_start = nvec * 8;
    for i in tail_start..n {
        let prod = (*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64);
        let lo = prod & 0xFFFF;
        let hi = prod >> 16;
        let r = lo + 65537 - hi;
        let r = if r >= 65537 { r - 65537 } else { r };
        *out.get_unchecked_mut(i) = r as u32;
    }
}

/// Batch lane-wise addition for `Fp<65537>`.
///
/// Computes `out[i] = (a[i] + b[i]) mod 65537`.
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical `Fp<65537>` values.
/// * `out` — output slice.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical.
///
/// # Panics
///
/// Panics if slice lengths differ.
///
/// # Complexity
///
/// O(n) with 8-lane vectorisation.
#[target_feature(enable = "avx2")]
pub unsafe fn fp65537_batch_add(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len(), "fp65537_batch_add: length mismatch");
    assert_eq!(a.len(), out.len(), "fp65537_batch_add: output length");

    let n = a.len();
    let nvec = n / 8;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    let p_vec = _mm256_set1_epi32(65537i32);

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        // sum ≤ 2·(P - 1) = 131072 < 2^18, fits easily in u32.
        let sum = _mm256_add_epi32(av, bv);
        // Branchless conditional subtract of P: use min_epu32(sum, sum - P).
        // When sum >= P, (sum - P) < sum (unsigned), so min selects sum - P.
        // When sum < P,  (sum - P) wraps to a very large u32, so min keeps sum.
        let minned = _mm256_min_epu32(sum, _mm256_sub_epi32(sum, p_vec));
        _mm256_storeu_si256(o_ptr.add(i), minned);
    }

    let tail_start = nvec * 8;
    for i in tail_start..n {
        let s = *a.get_unchecked(i) + *b.get_unchecked(i);
        *out.get_unchecked_mut(i) = if s >= 65537 { s - 65537 } else { s };
    }
}

/// Fused batch Karatsuba combine for `GF(p²)` element-wise multiplication
/// over `Fp<65537>`.
///
/// For every `i` computes
///
/// ```text
/// v0_i    = a0[i] * b0[i]
/// v1_i    = a1[i] * b1[i]
/// cross_i = (a0[i] + a1[i]) * (b0[i] + b1[i])
/// out_c0[i] = v0_i + β · v1_i
/// out_c1[i] = cross_i - v0_i - v1_i
/// ```
///
/// The fused form reads each of the four input slices once per 8-lane
/// chunk and writes each of the two output slices once, eliminating the
/// nine intermediate heap buffers used by a pass-per-op composition. All
/// intermediates live in AVX2 registers.
///
/// # Arguments
///
/// * `a0`, `a1` — left-batch coefficient slices (same length).
/// * `b0`, `b1` — right-batch coefficient slices (same length as `a0`).
/// * `beta` — non-residue β, canonical (< 65537).
/// * `out_c0`, `out_c1` — output coefficient slices (same length).
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical
/// (`< 65537`).
///
/// # Panics
///
/// Panics if any input/output slice length differs from `a0.len()`.
///
/// # Complexity
///
/// `O(n)` with 8-lane vectorisation. Three multiplies, two adds, two
/// subtracts, one broadcast multiply per 8-lane chunk.
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
pub unsafe fn fp65537_batch_karatsuba(
    a0: &[u32],
    a1: &[u32],
    b0: &[u32],
    b1: &[u32],
    beta: u32,
    out_c0: &mut [u32],
    out_c1: &mut [u32],
) {
    let n = a0.len();
    assert_eq!(a1.len(), n, "fp65537_batch_karatsuba: a1 length");
    assert_eq!(b0.len(), n, "fp65537_batch_karatsuba: b0 length");
    assert_eq!(b1.len(), n, "fp65537_batch_karatsuba: b1 length");
    assert_eq!(out_c0.len(), n, "fp65537_batch_karatsuba: out_c0 length");
    assert_eq!(out_c1.len(), n, "fp65537_batch_karatsuba: out_c1 length");

    let nvec = n / 8;

    let a0_ptr = a0.as_ptr() as *const __m256i;
    let a1_ptr = a1.as_ptr() as *const __m256i;
    let b0_ptr = b0.as_ptr() as *const __m256i;
    let b1_ptr = b1.as_ptr() as *const __m256i;
    let c0_ptr = out_c0.as_mut_ptr() as *mut __m256i;
    let c1_ptr = out_c1.as_mut_ptr() as *mut __m256i;

    let p_vec = _mm256_set1_epi32(65537i32);
    let beta_vec = _mm256_set1_epi32(beta as i32);

    for i in 0..nvec {
        let a0v = _mm256_loadu_si256(a0_ptr.add(i));
        let a1v = _mm256_loadu_si256(a1_ptr.add(i));
        let b0v = _mm256_loadu_si256(b0_ptr.add(i));
        let b1v = _mm256_loadu_si256(b1_ptr.add(i));

        // v0 = a0 * b0
        let v0 = fp65537_batch_mul8(a0v, b0v);
        // v1 = a1 * b1
        let v1 = fp65537_batch_mul8(a1v, b1v);

        // sum_a = a0 + a1  (canonical)
        let sum_a = {
            let s = _mm256_add_epi32(a0v, a1v);
            _mm256_min_epu32(s, _mm256_sub_epi32(s, p_vec))
        };
        // sum_b = b0 + b1  (canonical)
        let sum_b = {
            let s = _mm256_add_epi32(b0v, b1v);
            _mm256_min_epu32(s, _mm256_sub_epi32(s, p_vec))
        };

        // cross = sum_a * sum_b
        let cross = fp65537_batch_mul8(sum_a, sum_b);

        // beta_v1 = β * v1
        let beta_v1 = if beta == 0 {
            _mm256_setzero_si256()
        } else if beta == 1 {
            v1
        } else {
            fp65537_batch_mul8(v1, beta_vec)
        };

        // out_c0 = v0 + beta_v1  (canonical)
        let out_c0_v = {
            let s = _mm256_add_epi32(v0, beta_v1);
            _mm256_min_epu32(s, _mm256_sub_epi32(s, p_vec))
        };

        // tmp = cross - v0  (canonical) =  cross + P - v0, then cond-sub
        let tmp = {
            let a_plus_p = _mm256_add_epi32(cross, p_vec);
            let diff = _mm256_sub_epi32(a_plus_p, v0);
            _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec))
        };
        // out_c1 = tmp - v1 (canonical)
        let out_c1_v = {
            let a_plus_p = _mm256_add_epi32(tmp, p_vec);
            let diff = _mm256_sub_epi32(a_plus_p, v1);
            _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec))
        };

        _mm256_storeu_si256(c0_ptr.add(i), out_c0_v);
        _mm256_storeu_si256(c1_ptr.add(i), out_c1_v);
    }

    // Scalar tail for `n % 8` remaining elements.
    let tail_start = nvec * 8;
    for i in tail_start..n {
        let a0i = *a0.get_unchecked(i) as u64;
        let a1i = *a1.get_unchecked(i) as u64;
        let b0i = *b0.get_unchecked(i) as u64;
        let b1i = *b1.get_unchecked(i) as u64;
        let beta_u = beta as u64;

        let v0 = scalar_tail_mul(a0i, b0i);
        let v1 = scalar_tail_mul(a1i, b1i);
        let sum_a = scalar_tail_add(a0i, a1i);
        let sum_b = scalar_tail_add(b0i, b1i);
        let cross = scalar_tail_mul(sum_a, sum_b);
        let beta_v1 = if beta == 0 {
            0
        } else if beta == 1 {
            v1
        } else {
            scalar_tail_mul(v1, beta_u)
        };
        let out_c0_i = scalar_tail_add(v0, beta_v1);
        let tmp = scalar_tail_sub(cross, v0);
        let out_c1_i = scalar_tail_sub(tmp, v1);

        *out_c0.get_unchecked_mut(i) = out_c0_i as u32;
        *out_c1.get_unchecked_mut(i) = out_c1_i as u32;
    }
}

#[inline]
fn scalar_tail_mul(a: u64, b: u64) -> u64 {
    let prod = a * b;
    let lo = prod & 0xFFFF;
    let hi = prod >> 16;
    let r = lo + 65537 - hi;
    if r >= 65537 {
        r - 65537
    } else {
        r
    }
}

#[inline]
fn scalar_tail_add(a: u64, b: u64) -> u64 {
    let s = a + b;
    if s >= 65537 {
        s - 65537
    } else {
        s
    }
}

#[inline]
fn scalar_tail_sub(a: u64, b: u64) -> u64 {
    let d = a + 65537 - b;
    if d >= 65537 {
        d - 65537
    } else {
        d
    }
}

/// Batch lane-wise subtraction for `Fp<65537>`.
///
/// Computes `out[i] = (a[i] - b[i]) mod 65537` with the result in
/// canonical form `[0, 65537)`.
///
/// # Arguments
///
/// * `a`, `b` — input slices of canonical `Fp<65537>` values.
/// * `out` — output slice.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical.
///
/// # Panics
///
/// Panics if slice lengths differ.
///
/// # Complexity
///
/// O(n) with 8-lane vectorisation.
#[target_feature(enable = "avx2")]
pub unsafe fn fp65537_batch_sub(a: &[u32], b: &[u32], out: &mut [u32]) {
    assert_eq!(a.len(), b.len(), "fp65537_batch_sub: length mismatch");
    assert_eq!(a.len(), out.len(), "fp65537_batch_sub: output length");

    let n = a.len();
    let nvec = n / 8;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    let p_vec = _mm256_set1_epi32(65537i32);

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        // Compute (a + P) - b. Since a, b ≤ P - 1 = 65536, a + P ≤ 131073
        // and (a + P) - b ∈ [1, 131073]. A single conditional subtract of
        // P canonicalises to [0, P).
        let a_plus_p = _mm256_add_epi32(av, p_vec);
        let diff = _mm256_sub_epi32(a_plus_p, bv);
        let minned = _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec));
        _mm256_storeu_si256(o_ptr.add(i), minned);
    }

    let tail_start = nvec * 8;
    for i in tail_start..n {
        let ai = *a.get_unchecked(i);
        let bi = *b.get_unchecked(i);
        let d = ai + 65537 - bi;
        *out.get_unchecked_mut(i) = if d >= 65537 { d - 65537 } else { d };
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Test-local reference implementations. Signatures differ from the
    // module-level u64 helpers; the name shadowing is deliberate so each
    // test expression reads naturally.
    fn scalar_mul(a: u32, b: u32) -> u32 {
        ((a as u64 * b as u64) % 65537) as u32
    }

    fn scalar_add(a: u32, b: u32) -> u32 {
        (a + b) % 65537
    }

    fn scalar_sub(a: u32, b: u32) -> u32 {
        (a + 65537 - b) % 65537
    }

    #[test]
    fn batch_mul_exact_multiple_of_8() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u32> = (0..16u32).map(|i| (i * 12345) % 65537).collect();
        let b: Vec<u32> = (0..16u32).map(|i| (i * 67890) % 65537).collect();
        let mut out = vec![0u32; 16];
        unsafe { fp65537_batch_mul(&a, &b, &mut out) };
        for i in 0..16 {
            assert_eq!(out[i], scalar_mul(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_mul_with_tail() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u32> = (0..13u32).map(|i| (i * 12345) % 65537).collect();
        let b: Vec<u32> = (0..13u32).map(|i| (i * 67890) % 65537).collect();
        let mut out = vec![0u32; 13];
        unsafe { fp65537_batch_mul(&a, &b, &mut out) };
        for i in 0..13 {
            assert_eq!(out[i], scalar_mul(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_mul_boundary_values() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // 65536 = P - 1, 0, 1, P/2, and intermediate values. Covers the
        // key 65536² = 2³² overflow edge case.
        let a = vec![0u32, 1, 65536, 32768, 1, 65536, 0, 65535];
        let b = vec![65536, 0, 65536, 2, 32768, 1, 0, 3];
        let mut out = vec![0u32; 8];
        unsafe { fp65537_batch_mul(&a, &b, &mut out) };
        for i in 0..8 {
            assert_eq!(out[i], scalar_mul(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_mul_saturation_lane() {
        // 65536 × 65536 = 2^32 -- the one value that causes u32 mullo to wrap
        // to 0. Verifies the mul_epu32 + reduction path handles it correctly.
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a = vec![65536u32; 16];
        let b = vec![65536u32; 16];
        let mut out = vec![0u32; 16];
        unsafe { fp65537_batch_mul(&a, &b, &mut out) };
        // 65536 ≡ -1 (mod 65537), so 65536 * 65536 ≡ 1.
        for (i, &v) in out.iter().enumerate() {
            assert_eq!(v, 1, "i={i}");
        }
    }

    #[test]
    fn batch_add_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u32> = (0..17u32).map(|i| (i * 4093) % 65537).collect();
        let b: Vec<u32> = (0..17u32).map(|i| (i * 9973) % 65537).collect();
        let mut out = vec![0u32; 17];
        unsafe { fp65537_batch_add(&a, &b, &mut out) };
        for i in 0..17 {
            assert_eq!(out[i], scalar_add(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_add_boundary_values() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a = vec![0u32, 1, 65536, 65536, 32768, 65535, 65536, 1];
        let b = vec![0u32, 65536, 1, 65536, 32769, 2, 0, 65536];
        let mut out = vec![0u32; 8];
        unsafe { fp65537_batch_add(&a, &b, &mut out) };
        for i in 0..8 {
            assert_eq!(out[i], scalar_add(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_sub_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u32> = (0..17u32).map(|i| (i * 4093) % 65537).collect();
        let b: Vec<u32> = (0..17u32).map(|i| (i * 9973) % 65537).collect();
        let mut out = vec![0u32; 17];
        unsafe { fp65537_batch_sub(&a, &b, &mut out) };
        for i in 0..17 {
            assert_eq!(out[i], scalar_sub(a[i], b[i]), "i={i}");
        }
    }

    #[test]
    fn batch_karatsuba_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &n in &[0usize, 1, 7, 8, 9, 16, 17, 100, 1000] {
            let a0: Vec<u32> = (0..n as u32).map(|i| (i * 17) % 65537).collect();
            let a1: Vec<u32> = (0..n as u32).map(|i| (i * 23 + 5) % 65537).collect();
            let b0: Vec<u32> = (0..n as u32).map(|i| (i * 29 + 7) % 65537).collect();
            let b1: Vec<u32> = (0..n as u32).map(|i| (i * 31 + 11) % 65537).collect();
            let beta = 3u32;

            let mut out_c0 = vec![0u32; n];
            let mut out_c1 = vec![0u32; n];
            unsafe {
                fp65537_batch_karatsuba(&a0, &a1, &b0, &b1, beta, &mut out_c0, &mut out_c1);
            }

            for i in 0..n {
                let v0 = scalar_mul(a0[i], b0[i]);
                let v1 = scalar_mul(a1[i], b1[i]);
                let sum_a = scalar_add(a0[i], a1[i]);
                let sum_b = scalar_add(b0[i], b1[i]);
                let cross = scalar_mul(sum_a, sum_b);
                let beta_v1 = scalar_mul(v1, beta);
                let expected_c0 = scalar_add(v0, beta_v1);
                let tmp = scalar_sub(cross, v0);
                let expected_c1 = scalar_sub(tmp, v1);
                assert_eq!(out_c0[i], expected_c0, "c0 mismatch n={n} i={i}");
                assert_eq!(out_c1[i], expected_c1, "c1 mismatch n={n} i={i}");
            }
        }
    }

    #[test]
    fn batch_karatsuba_beta_0_and_1() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &beta in &[0u32, 1] {
            let n = 24;
            let a0: Vec<u32> = (0..n as u32).map(|i| (i * 17) % 65537).collect();
            let a1: Vec<u32> = (0..n as u32).map(|i| (i * 23 + 5) % 65537).collect();
            let b0: Vec<u32> = (0..n as u32).map(|i| (i * 29 + 7) % 65537).collect();
            let b1: Vec<u32> = (0..n as u32).map(|i| (i * 31 + 11) % 65537).collect();

            let mut out_c0 = vec![0u32; n];
            let mut out_c1 = vec![0u32; n];
            unsafe {
                fp65537_batch_karatsuba(&a0, &a1, &b0, &b1, beta, &mut out_c0, &mut out_c1);
            }

            for i in 0..n {
                let v0 = scalar_mul(a0[i], b0[i]);
                let v1 = scalar_mul(a1[i], b1[i]);
                let sum_a = scalar_add(a0[i], a1[i]);
                let sum_b = scalar_add(b0[i], b1[i]);
                let cross = scalar_mul(sum_a, sum_b);
                let beta_v1 = scalar_mul(v1, beta);
                let expected_c0 = scalar_add(v0, beta_v1);
                let tmp = scalar_sub(cross, v0);
                let expected_c1 = scalar_sub(tmp, v1);
                assert_eq!(out_c0[i], expected_c0);
                assert_eq!(out_c1[i], expected_c1);
            }
        }
    }

    #[test]
    fn batch_sub_boundary_values() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a = vec![0u32, 65536, 0, 1, 32768, 65535, 65536, 65536];
        let b = vec![0u32, 65536, 65536, 0, 32769, 65535, 1, 65536];
        let mut out = vec![0u32; 8];
        unsafe { fp65537_batch_sub(&a, &b, &mut out) };
        for i in 0..8 {
            assert_eq!(out[i], scalar_sub(a[i], b[i]), "i={i}");
        }
    }
}
