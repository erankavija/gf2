//! AVX2 batch multiply-reduce kernels for the Fermat prime
//! `P = 65537 = 2^16 + 1`.
//!
//! For `a, b ∈ [0, P)`, the product `a·b ≤ 2³²` splits as
//! `hi · 2¹⁶ + lo` with `lo < 2¹⁶`, `hi ≤ 2¹⁶`, and `2¹⁶ ≡ -1 (mod P)` gives
//! `a·b ≡ lo - hi`; adding `P` moves it into `[1, 2P - 1]` and one conditional
//! subtract canonicalises. `_mm256_mullo_epi32` would wrap on
//! `65536 × 65536 = 2³²`, so the kernels use `_mm256_mul_epu32` on the even
//! and then the odd u32 lanes and recombine the two reduced vectors.
//!
//! All public functions are `unsafe`: callers must ensure AVX2 is available
//! at runtime. `crate::fp65537::detect` returns the safe dispatched table.

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
    let mask16 = _mm256_set1_epi64x(0xFFFF);
    let p_vec = _mm256_set1_epi64x(65537);

    let lo = _mm256_and_si256(p, mask16);
    let hi = _mm256_srli_epi64(p, 16);

    // r_shifted = lo + P - hi  (always non-negative since P > hi ≤ 2^16).
    // Range: [1, 2P - 1] — a single conditional subtract canonicalises.
    let lo_plus_p = _mm256_add_epi64(lo, p_vec);
    let r_shifted = _mm256_sub_epi64(lo_plus_p, hi);

    // r_shifted < 2·P < 2^17 ≪ 2^63, so the signed compare is exact.
    let r_minus_p = _mm256_sub_epi64(r_shifted, p_vec);
    let lt = _mm256_cmpgt_epi64(p_vec, r_shifted);
    let ones = _mm256_set1_epi64x(-1);
    let ge_mask = _mm256_xor_si256(lt, ones);
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

    // Odd results move to the high 32 bits of each 64-bit lane.
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
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime. All input values
/// must be canonical (strictly less than `65537`); behaviour is undefined
/// otherwise.
///
/// # Panics
///
/// Panics if the slice lengths differ.
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
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical.
///
/// # Panics
///
/// Panics if slice lengths differ.
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
/// Each 8-lane chunk reads the four input slices once and writes the two
/// output slices once; all intermediates live in AVX2 registers.
///
/// `beta` is the non-residue β, canonical (`< 65537`).
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical
/// (`< 65537`).
///
/// # Panics
///
/// Panics if any input/output slice length differs from `a0.len()`.
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

        let v0 = fp65537_batch_mul8(a0v, b0v);
        let v1 = fp65537_batch_mul8(a1v, b1v);

        let sum_a = {
            let s = _mm256_add_epi32(a0v, a1v);
            _mm256_min_epu32(s, _mm256_sub_epi32(s, p_vec))
        };
        let sum_b = {
            let s = _mm256_add_epi32(b0v, b1v);
            _mm256_min_epu32(s, _mm256_sub_epi32(s, p_vec))
        };

        let cross = fp65537_batch_mul8(sum_a, sum_b);

        let beta_v1 = if beta == 0 {
            _mm256_setzero_si256()
        } else if beta == 1 {
            v1
        } else {
            fp65537_batch_mul8(v1, beta_vec)
        };

        let out_c0_v = {
            let s = _mm256_add_epi32(v0, beta_v1);
            _mm256_min_epu32(s, _mm256_sub_epi32(s, p_vec))
        };

        let tmp = {
            let a_plus_p = _mm256_add_epi32(cross, p_vec);
            let diff = _mm256_sub_epi32(a_plus_p, v0);
            _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec))
        };
        let out_c1_v = {
            let a_plus_p = _mm256_add_epi32(tmp, p_vec);
            let diff = _mm256_sub_epi32(a_plus_p, v1);
            _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec))
        };

        _mm256_storeu_si256(c0_ptr.add(i), out_c0_v);
        _mm256_storeu_si256(c1_ptr.add(i), out_c1_v);
    }

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

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp65537_add8(a: __m256i, b: __m256i, p_vec: __m256i) -> __m256i {
    let s = _mm256_add_epi32(a, b);
    _mm256_min_epu32(s, _mm256_sub_epi32(s, p_vec))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp65537_sub8(a: __m256i, b: __m256i, p_vec: __m256i) -> __m256i {
    let a_plus_p = _mm256_add_epi32(a, p_vec);
    let diff = _mm256_sub_epi32(a_plus_p, b);
    _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp65537_mul_beta8(x: __m256i, beta: u32, beta_vec: __m256i) -> __m256i {
    if beta == 0 {
        _mm256_setzero_si256()
    } else if beta == 1 {
        x
    } else {
        fp65537_batch_mul8(x, beta_vec)
    }
}

/// Fused batch Karatsuba-3 combine for `GF(p³)` element-wise multiplication
/// over `Fp<65537>`.
///
/// For every `i`, computes the same six-product formula as
/// `CubicExt::mul`, with inputs and outputs in SoA coefficient-lane order.
/// All vector-loop intermediates stay in AVX2 registers.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical
/// (`< 65537`).
///
/// # Panics
///
/// Panics if any input/output slice length differs from `a0.len()`.
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
pub unsafe fn fp65537_batch_cubic_karatsuba(
    a0: &[u32],
    a1: &[u32],
    a2: &[u32],
    b0: &[u32],
    b1: &[u32],
    b2: &[u32],
    beta: u32,
    out_c0: &mut [u32],
    out_c1: &mut [u32],
    out_c2: &mut [u32],
) {
    let n = a0.len();
    assert_eq!(a1.len(), n, "fp65537_batch_cubic_karatsuba: a1 length");
    assert_eq!(a2.len(), n, "fp65537_batch_cubic_karatsuba: a2 length");
    assert_eq!(b0.len(), n, "fp65537_batch_cubic_karatsuba: b0 length");
    assert_eq!(b1.len(), n, "fp65537_batch_cubic_karatsuba: b1 length");
    assert_eq!(b2.len(), n, "fp65537_batch_cubic_karatsuba: b2 length");
    assert_eq!(
        out_c0.len(),
        n,
        "fp65537_batch_cubic_karatsuba: out_c0 length"
    );
    assert_eq!(
        out_c1.len(),
        n,
        "fp65537_batch_cubic_karatsuba: out_c1 length"
    );
    assert_eq!(
        out_c2.len(),
        n,
        "fp65537_batch_cubic_karatsuba: out_c2 length"
    );

    let nvec = n / 8;

    let a0_ptr = a0.as_ptr() as *const __m256i;
    let a1_ptr = a1.as_ptr() as *const __m256i;
    let a2_ptr = a2.as_ptr() as *const __m256i;
    let b0_ptr = b0.as_ptr() as *const __m256i;
    let b1_ptr = b1.as_ptr() as *const __m256i;
    let b2_ptr = b2.as_ptr() as *const __m256i;
    let c0_ptr = out_c0.as_mut_ptr() as *mut __m256i;
    let c1_ptr = out_c1.as_mut_ptr() as *mut __m256i;
    let c2_ptr = out_c2.as_mut_ptr() as *mut __m256i;

    let p_vec = _mm256_set1_epi32(65537i32);
    let beta_vec = _mm256_set1_epi32(beta as i32);

    for i in 0..nvec {
        let a0v = _mm256_loadu_si256(a0_ptr.add(i));
        let a1v = _mm256_loadu_si256(a1_ptr.add(i));
        let a2v = _mm256_loadu_si256(a2_ptr.add(i));
        let b0v = _mm256_loadu_si256(b0_ptr.add(i));
        let b1v = _mm256_loadu_si256(b1_ptr.add(i));
        let b2v = _mm256_loadu_si256(b2_ptr.add(i));

        let v0 = fp65537_batch_mul8(a0v, b0v);
        let v1 = fp65537_batch_mul8(a1v, b1v);
        let v2 = fp65537_batch_mul8(a2v, b2v);

        let x_cross =
            fp65537_batch_mul8(fp65537_add8(a1v, a2v, p_vec), fp65537_add8(b1v, b2v, p_vec));
        let x = fp65537_sub8(fp65537_sub8(x_cross, v1, p_vec), v2, p_vec);

        let y_cross =
            fp65537_batch_mul8(fp65537_add8(a0v, a1v, p_vec), fp65537_add8(b0v, b1v, p_vec));
        let y = fp65537_sub8(fp65537_sub8(y_cross, v0, p_vec), v1, p_vec);

        let z_cross =
            fp65537_batch_mul8(fp65537_add8(a0v, a2v, p_vec), fp65537_add8(b0v, b2v, p_vec));
        let z = fp65537_sub8(
            fp65537_add8(fp65537_sub8(z_cross, v0, p_vec), v1, p_vec),
            v2,
            p_vec,
        );

        let c0 = fp65537_add8(v0, fp65537_mul_beta8(x, beta, beta_vec), p_vec);
        let c1 = fp65537_add8(y, fp65537_mul_beta8(v2, beta, beta_vec), p_vec);

        _mm256_storeu_si256(c0_ptr.add(i), c0);
        _mm256_storeu_si256(c1_ptr.add(i), c1);
        _mm256_storeu_si256(c2_ptr.add(i), z);
    }

    let tail_start = nvec * 8;
    for i in tail_start..n {
        let a0i = *a0.get_unchecked(i) as u64;
        let a1i = *a1.get_unchecked(i) as u64;
        let a2i = *a2.get_unchecked(i) as u64;
        let b0i = *b0.get_unchecked(i) as u64;
        let b1i = *b1.get_unchecked(i) as u64;
        let b2i = *b2.get_unchecked(i) as u64;
        let beta_u = beta as u64;

        let v0 = scalar_tail_mul(a0i, b0i);
        let v1 = scalar_tail_mul(a1i, b1i);
        let v2 = scalar_tail_mul(a2i, b2i);
        let x = scalar_tail_sub(
            scalar_tail_sub(
                scalar_tail_mul(scalar_tail_add(a1i, a2i), scalar_tail_add(b1i, b2i)),
                v1,
            ),
            v2,
        );
        let y = scalar_tail_sub(
            scalar_tail_sub(
                scalar_tail_mul(scalar_tail_add(a0i, a1i), scalar_tail_add(b0i, b1i)),
                v0,
            ),
            v1,
        );
        let z = scalar_tail_sub(
            scalar_tail_add(
                scalar_tail_sub(
                    scalar_tail_mul(scalar_tail_add(a0i, a2i), scalar_tail_add(b0i, b2i)),
                    v0,
                ),
                v1,
            ),
            v2,
        );
        let beta_x = if beta == 0 {
            0
        } else if beta == 1 {
            x
        } else {
            scalar_tail_mul(x, beta_u)
        };
        let beta_v2 = if beta == 0 {
            0
        } else if beta == 1 {
            v2
        } else {
            scalar_tail_mul(v2, beta_u)
        };

        *out_c0.get_unchecked_mut(i) = scalar_tail_add(v0, beta_x) as u32;
        *out_c1.get_unchecked_mut(i) = scalar_tail_add(y, beta_v2) as u32;
        *out_c2.get_unchecked_mut(i) = z as u32;
    }
}

/// Batch lane-wise subtraction for `Fp<65537>`.
///
/// Computes `out[i] = (a[i] - b[i]) mod 65537` with the result in
/// canonical form `[0, 65537)`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and inputs are canonical.
///
/// # Panics
///
/// Panics if slice lengths differ.
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
    fn batch_cubic_karatsuba_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &n in &[0usize, 1, 7, 8, 9, 16, 17, 100, 1000] {
            let a0: Vec<u32> = (0..n as u32).map(|i| (i * 17) % 65537).collect();
            let a1: Vec<u32> = (0..n as u32).map(|i| (i * 23 + 5) % 65537).collect();
            let a2: Vec<u32> = (0..n as u32).map(|i| (i * 37 + 13) % 65537).collect();
            let b0: Vec<u32> = (0..n as u32).map(|i| (i * 29 + 7) % 65537).collect();
            let b1: Vec<u32> = (0..n as u32).map(|i| (i * 31 + 11) % 65537).collect();
            let b2: Vec<u32> = (0..n as u32).map(|i| (i * 41 + 19) % 65537).collect();
            let beta = 3u32;

            let mut out_c0 = vec![0u32; n];
            let mut out_c1 = vec![0u32; n];
            let mut out_c2 = vec![0u32; n];
            unsafe {
                fp65537_batch_cubic_karatsuba(
                    &a0,
                    &a1,
                    &a2,
                    &b0,
                    &b1,
                    &b2,
                    beta,
                    &mut out_c0,
                    &mut out_c1,
                    &mut out_c2,
                );
            }

            for i in 0..n {
                let v0 = scalar_mul(a0[i], b0[i]);
                let v1 = scalar_mul(a1[i], b1[i]);
                let v2 = scalar_mul(a2[i], b2[i]);
                let x = scalar_sub(
                    scalar_sub(
                        scalar_mul(scalar_add(a1[i], a2[i]), scalar_add(b1[i], b2[i])),
                        v1,
                    ),
                    v2,
                );
                let y = scalar_sub(
                    scalar_sub(
                        scalar_mul(scalar_add(a0[i], a1[i]), scalar_add(b0[i], b1[i])),
                        v0,
                    ),
                    v1,
                );
                let z = scalar_sub(
                    scalar_add(
                        scalar_sub(
                            scalar_mul(scalar_add(a0[i], a2[i]), scalar_add(b0[i], b2[i])),
                            v0,
                        ),
                        v1,
                    ),
                    v2,
                );
                let expected_c0 = scalar_add(v0, scalar_mul(beta, x));
                let expected_c1 = scalar_add(y, scalar_mul(beta, v2));
                assert_eq!(out_c0[i], expected_c0, "c0 mismatch n={n} i={i}");
                assert_eq!(out_c1[i], expected_c1, "c1 mismatch n={n} i={i}");
                assert_eq!(out_c2[i], z, "c2 mismatch n={n} i={i}");
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
