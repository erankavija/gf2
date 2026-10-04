//! AVX2 batch kernels for medium primes `Fp<P>` with `P < 2^16`.
//!
//! Residues occupy one 16-bit lane; the kernels accept any odd prime
//! `P ∈ (251, 65535]`. Each kernel's doc states whether it takes canonical
//! residues or Montgomery raw storage. All public functions are `unsafe`:
//! callers must ensure AVX2 is available at runtime;
//! `crate::fp_medium::detect` returns the safe dispatched table.

#![allow(clippy::missing_safety_doc)]

use core::arch::x86_64::*;

/// Lane-wise modular multiplication for 16 u16 values per 256-bit vector.
///
/// Inputs are 16 canonical u16 values per vector (`a, b < P`); output is
/// 16 canonical u16 values. Internally widens to 32-bit, multiplies, and
/// Barrett-reduces via [`super::fp_small::barrett_reduce_lane32`].
///
/// `m32` carries `μ = ⌊2³² / P⌋` broadcast as 8 u32 lanes.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_batch_mul16(a: __m256i, b: __m256i, p32: __m256i, m32: __m256i) -> __m256i {
    // Zero-extend u16 → u32; the unpacks work per 128-bit lane.
    let zero = _mm256_setzero_si256();
    let a_lo = _mm256_unpacklo_epi16(a, zero);
    let a_hi = _mm256_unpackhi_epi16(a, zero);
    let b_lo = _mm256_unpacklo_epi16(b, zero);
    let b_hi = _mm256_unpackhi_epi16(b, zero);

    // 32-bit lane multiply: (P-1)² < 2^32 so mullo is exact.
    let prod_lo = _mm256_mullo_epi32(a_lo, b_lo);
    let prod_hi = _mm256_mullo_epi32(a_hi, b_hi);

    let red_lo = super::fp_small::barrett_reduce_lane32(prod_lo, m32, p32);
    let red_hi = super::fp_small::barrett_reduce_lane32(prod_hi, m32, p32);

    // Reduced values are in `[0, P)`, so `packus_epi32` never saturates. It
    // interleaves per 128-bit lane the same way the unpacks above do, so one
    // packus restores the original lane order.
    _mm256_packus_epi32(red_lo, red_hi)
}

/// Lane-wise modular addition for 16 u16 lanes.
///
/// Sum `s = a + b ≤ 2P - 2 < 2^17`, so the add runs in 32-bit lanes with a
/// branchless conditional subtract of `P`.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_add16(a: __m256i, b: __m256i, p: __m256i) -> __m256i {
    let zero = _mm256_setzero_si256();
    let a_lo = _mm256_unpacklo_epi16(a, zero);
    let a_hi = _mm256_unpackhi_epi16(a, zero);
    let b_lo = _mm256_unpacklo_epi16(b, zero);
    let b_hi = _mm256_unpackhi_epi16(b, zero);

    let s_lo = _mm256_add_epi32(a_lo, b_lo);
    let s_hi = _mm256_add_epi32(a_hi, b_hi);

    let r_lo = _mm256_min_epu32(s_lo, _mm256_sub_epi32(s_lo, p));
    let r_hi = _mm256_min_epu32(s_hi, _mm256_sub_epi32(s_hi, p));

    _mm256_packus_epi32(r_lo, r_hi)
}

/// Lane-wise modular subtraction for 16 u16 lanes.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_sub16(a: __m256i, b: __m256i, p: __m256i) -> __m256i {
    let zero = _mm256_setzero_si256();
    let a_lo = _mm256_unpacklo_epi16(a, zero);
    let a_hi = _mm256_unpackhi_epi16(a, zero);
    let b_lo = _mm256_unpacklo_epi16(b, zero);
    let b_hi = _mm256_unpackhi_epi16(b, zero);

    // Compute (a + P) - b. Since `a, b < P`, `a + P < 2P < 2^17` so the
    // 32-bit add is exact, and `(a + P) - b ∈ [1, 2P - 1]`. Then a single
    // branchless cond-sub of P canonicalises.
    let ap_lo = _mm256_add_epi32(a_lo, p);
    let ap_hi = _mm256_add_epi32(a_hi, p);
    let d_lo = _mm256_sub_epi32(ap_lo, b_lo);
    let d_hi = _mm256_sub_epi32(ap_hi, b_hi);

    let r_lo = _mm256_min_epu32(d_lo, _mm256_sub_epi32(d_lo, p));
    let r_hi = _mm256_min_epu32(d_hi, _mm256_sub_epi32(d_hi, p));

    _mm256_packus_epi32(r_lo, r_hi)
}

/// Batch lane-wise multiplication for `Fp<P>` with `P < 2^16`.
///
/// Computes `out[i] = (a[i] * b[i]) mod P` for all `i`, using 16-lane
/// AVX2 vectorisation with Barrett reduction.
///
/// `a` and `b` hold canonical residues in `[0, P)`; `p` is in `(1, 2^16)`
/// and `barrett_m` is `floor(2^32 / p)`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime, all input values are
/// `< p`, and `barrett_m == floor(2^32 / p)`. Behaviour is undefined
/// otherwise. Inputs in Montgomery raw storage are *not* an unsoundness
/// hazard but produce a wrong-domain result.
///
/// # Panics
///
/// Panics if slice lengths differ.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_mul(a: &[u16], b: &[u16], p: u16, barrett_m: u32, out: &mut [u16]) {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_mul: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_medium_batch_mul: output length");

    let n = a.len();
    let nvec = n / 16;

    let p32 = _mm256_set1_epi32(p as i32);
    let m32 = _mm256_set1_epi32(barrett_m as i32);

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp_medium_batch_mul16(av, bv, p32, m32);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    let tail_start = nvec * 16;
    for i in tail_start..n {
        let prod = (*a.get_unchecked(i) as u32) * (*b.get_unchecked(i) as u32);
        *out.get_unchecked_mut(i) = (prod % p as u32) as u16;
    }
}

/// Batch lane-wise addition for `Fp<P>` with `P < 2^16`.
///
/// Lanes are in `[0, P)`, canonical or Montgomery raw storage; the result
/// is in the input domain. `p` is in `(1, 2^16)`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime and all input values
/// are `< p`. Behaviour is undefined otherwise.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_add(a: &[u16], b: &[u16], p: u16, out: &mut [u16]) {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_add: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_medium_batch_add: output length");

    let n = a.len();
    let nvec = n / 16;

    let p_vec = _mm256_set1_epi32(p as i32);

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp_medium_add16(av, bv, p_vec);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    let tail_start = nvec * 16;
    for i in tail_start..n {
        let s = *a.get_unchecked(i) as u32 + *b.get_unchecked(i) as u32;
        *out.get_unchecked_mut(i) = (if s >= p as u32 { s - p as u32 } else { s }) as u16;
    }
}

/// Batch lane-wise subtraction for `Fp<P>` with `P < 2^16`.
///
/// Lanes are in `[0, P)`, canonical or Montgomery raw storage; the result
/// is in the input domain. `p` is in `(1, 2^16)`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime and all input values
/// are `< p`. Behaviour is undefined otherwise.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_sub(a: &[u16], b: &[u16], p: u16, out: &mut [u16]) {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_sub: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_medium_batch_sub: output length");

    let n = a.len();
    let nvec = n / 16;

    let p_vec = _mm256_set1_epi32(p as i32);

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp_medium_sub16(av, bv, p_vec);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    let tail_start = nvec * 16;
    for i in tail_start..n {
        let ai = *a.get_unchecked(i) as u32;
        let bi = *b.get_unchecked(i) as u32;
        let d = ai + p as u32 - bi;
        *out.get_unchecked_mut(i) = (if d >= p as u32 { d - p as u32 } else { d }) as u16;
    }
}

/// Batch dot product for `Fp<P>` with `P < 2^16`, returning the
/// reduced sum `(Σ a[i] * b[i]) mod p` in the same domain interpretation
/// as the inputs.
///
/// # Domain semantics
///
/// On canonical lanes the result is the canonical dot product. On
/// Montgomery raw storage `aR mod p` it is `(R² · Σ aᵢbᵢ) mod p`, and one
/// Montgomery REDC by the caller recovers `R · Σ aᵢbᵢ mod p`.
///
/// # Algorithm
///
/// * `p ≤ 32767`: `_mm256_madd_epi16` fuses two adjacent u16 × u16
///   products into one i32 lane; lanes are below `2^15`, so the signed
///   interpretation is exact. The per-pair bound is `2 · (p-1)² < 2^31`, so
///   a u32 lane absorbs `floor(2^32 / (2 · (p-1)²))` pair-MACs before it
///   drains to u64.
/// * `p > 32767`: `_mm256_mullo_epi16` + `_mm256_mulhi_epu16` reconstruct
///   the full u32 product, widened to u64 every iteration.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and all input values are `< p`.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_dot(a: &[u16], b: &[u16], p: u16) -> u32 {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_dot: length mismatch");

    if p <= 32_767 {
        fp_medium_batch_dot_madd(a, b, p)
    } else {
        fp_medium_batch_dot_mulhi(a, b, p)
    }
}

/// Fast path for `p ≤ 32767`: signed `_mm256_madd_epi16` with per-prime
/// panel-size accumulation in u32 lanes before draining to u64.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_batch_dot_madd(a: &[u16], b: &[u16], p: u16) -> u32 {
    debug_assert!(p > 0 && p <= 32_767);

    let n = a.len();
    let nvec = n / 16;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;

    // Each madd_epi16 lane holds `a[2i]*b[2i] + a[2i+1]*b[2i+1] ≤
    // 2 * (p-1)²`, so a u32 lane absorbs `floor(2^32 / (2 * (p-1)²))`
    // chunks before the panel drains into the u64 accumulator.
    let pair_bound = 2u64 * (p as u64 - 1) * (p as u64 - 1);
    let panel_chunks: usize = match (1u64 << 32).checked_div(pair_bound) {
        Some(raw) => raw.max(1) as usize,
        None => usize::MAX,
    };

    let zero = _mm256_setzero_si256();
    let mut acc_u64 = _mm256_setzero_si256(); // 4 × u64 accumulators

    let mut chunk = 0usize;
    while chunk < nvec {
        let panel_end = (chunk + panel_chunks).min(nvec);
        let mut acc_u32 = _mm256_setzero_si256(); // 8 × u32 lane sums

        while chunk < panel_end {
            let av = _mm256_loadu_si256(a_ptr.add(chunk));
            let bv = _mm256_loadu_si256(b_ptr.add(chunk));
            let m = _mm256_madd_epi16(av, bv);
            acc_u32 = _mm256_add_epi32(acc_u32, m);
            chunk += 1;
        }

        // Drain u32 panel sum into the u64 accumulator. Widen 8 × u32
        // → 8 × u64 split across two vectors.
        let lo = _mm256_unpacklo_epi32(acc_u32, zero);
        let hi = _mm256_unpackhi_epi32(acc_u32, zero);
        acc_u64 = _mm256_add_epi64(acc_u64, lo);
        acc_u64 = _mm256_add_epi64(acc_u64, hi);
    }

    let mut tmp = [0u64; 4];
    _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc_u64);
    let mut total: u64 = tmp[0]
        .wrapping_add(tmp[1])
        .wrapping_add(tmp[2])
        .wrapping_add(tmp[3]);

    let tail_start = nvec * 16;
    for i in tail_start..n {
        total = total.wrapping_add((*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64));
    }

    (total % p as u64) as u32
}

/// Fallback path for `p > 32767`: full u32 products via mullo + mulhi,
/// widened to u64 per chunk. No panel batching possible (a single u32
/// product can approach 2^32).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_batch_dot_mulhi(a: &[u16], b: &[u16], p: u16) -> u32 {
    let n = a.len();
    let nvec = n / 16;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;

    let mut acc_lo = _mm256_setzero_si256();
    let mut acc_hi = _mm256_setzero_si256();
    let zero = _mm256_setzero_si256();

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));

        // `mullo_epi16` is signed, but the low 16 bits of a signed product
        // equal those of the unsigned product; `mulhi_epu16` returns the
        // unsigned high half. `unpack{lo,hi}_epi16` re-interleaves them into
        // eight u32 products per 256-bit half.
        let prod_lo16 = _mm256_mullo_epi16(av, bv);
        let prod_hi16 = _mm256_mulhi_epu16(av, bv);
        let prod_full_lo = _mm256_unpacklo_epi16(prod_lo16, prod_hi16);
        let prod_full_hi = _mm256_unpackhi_epi16(prod_lo16, prod_hi16);

        let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
        let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
        let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
        let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);

        acc_lo = _mm256_add_epi64(acc_lo, _mm256_add_epi64(p_lo_l, p_hi_l));
        acc_hi = _mm256_add_epi64(acc_hi, _mm256_add_epi64(p_lo_h, p_hi_h));
    }

    // Horizontal sum of `acc_lo + acc_hi`. Each holds four u64 lanes; max
    // per lane is `(n/16) * (P-1)² ≈ n/16 * 2^32 ≈ 2^60` for `n = 2^32`.
    let acc = _mm256_add_epi64(acc_lo, acc_hi);

    let mut tmp = [0u64; 4];
    _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc);
    let mut total: u64 = tmp[0]
        .wrapping_add(tmp[1])
        .wrapping_add(tmp[2])
        .wrapping_add(tmp[3]);

    let tail_start = nvec * 16;
    for i in tail_start..n {
        total = total.wrapping_add((*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64));
    }

    // `total` stays within u64 for n < 2^64 / (P-1)² ≈ 4.3e9.
    (total % p as u64) as u32
}

/// Sparse-times-dense row kernel for medium-prime `Fp<P>` with
/// `P ∈ (251, 65535]`.
///
/// Writes `out[j] = (∑_h a_vals[h] * b[a_cols[h] * b_stride + j]) mod p`
/// for `j ∈ [0, n)`. The sparse left row is given as `(a_vals, a_cols)`
/// with canonical u16 lanes; `b` is a row-major dense u16 matrix with
/// row stride `b_stride`. `out` is the dense output row of length `n`.
///
/// Per block of 16 output lanes the kernel sweeps every non-zero of the
/// sparse row, forms full u32 products via `mullo_epi16 + mulhi_epu16`, and
/// accumulates them in u64 lanes that are reduced modulo `p` after the
/// sweep. The u64 lanes hold `2^64 / (p-1)² ≈ 4.3 × 10^9` products at
/// `p = 65521` and are not reduced mid-sweep.
///
/// # Safety
///
/// Caller must ensure AVX2 is available, `p` is an odd prime in
/// `(251, 65535]`, every input lane is canonical (`< p`), and:
/// - `a_vals.len() == a_cols.len()`,
/// - every `a_cols[h] * b_stride + n <= b.len()`,
/// - `out.len() == n`.
///
/// # Panics
///
/// Panics if `a_vals.len() != a_cols.len()` or `out.len() != n`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_spmm_row(
    a_vals: &[u16],
    a_cols: &[usize],
    b: &[u16],
    b_stride: usize,
    n: usize,
    p: u16,
    out: &mut [u16],
) {
    assert_eq!(
        a_vals.len(),
        a_cols.len(),
        "fp_medium_spmm_row: a_vals/a_cols length mismatch"
    );
    assert_eq!(out.len(), n, "fp_medium_spmm_row: out.len() != n");

    let nnz = a_vals.len();
    let p_u64 = p as u64;
    let zero = _mm256_setzero_si256();

    let mut j = 0;
    while j + 16 <= n {
        // 16 u32 products → widened to 16 u64 lanes split into 4 ymm.
        let mut acc0 = _mm256_setzero_si256(); // u64 lanes 0..3 (B-cols j..j+3)
        let mut acc1 = _mm256_setzero_si256(); // u64 lanes 4..7 (B-cols j+4..j+7)
        let mut acc2 = _mm256_setzero_si256(); // u64 lanes 8..11
        let mut acc3 = _mm256_setzero_si256(); // u64 lanes 12..15
        for h in 0..nnz {
            let a_h = *a_vals.get_unchecked(h);
            let col = *a_cols.get_unchecked(h);
            let b_row_ptr = b.as_ptr().add(col * b_stride + j) as *const __m256i;
            let bv = _mm256_loadu_si256(b_row_ptr);
            let av = _mm256_set1_epi16(a_h as i16);
            let prod_lo16 = _mm256_mullo_epi16(av, bv);
            let prod_hi16 = _mm256_mulhi_epu16(av, bv);
            let prod_full_lo = _mm256_unpacklo_epi16(prod_lo16, prod_hi16);
            let prod_full_hi = _mm256_unpackhi_epi16(prod_lo16, prod_hi16);
            // unpack{lo,hi}_epi32 zero-extend per 128-bit half:
            //   p_lo_l = B-cols {j+0, j+1 | j+8,  j+9}
            //   p_lo_h = B-cols {j+2, j+3 | j+10, j+11}
            //   p_hi_l = B-cols {j+4, j+5 | j+12, j+13}
            //   p_hi_h = B-cols {j+6, j+7 | j+14, j+15}
            let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
            let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
            let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
            let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);
            // The halves are regrouped so that accumulator `acc<i>` covers
            // the 4 contiguous output cells `j+4i .. j+4i+3`.
            let lo01 = _mm256_castsi256_si128(p_lo_l);
            let lo23 = _mm256_castsi256_si128(p_lo_h);
            let lo45 = _mm256_castsi256_si128(p_hi_l);
            let lo67 = _mm256_castsi256_si128(p_hi_h);
            let hi89 = _mm256_extracti128_si256::<1>(p_lo_l);
            let hi1011 = _mm256_extracti128_si256::<1>(p_lo_h);
            let hi1213 = _mm256_extracti128_si256::<1>(p_hi_l);
            let hi1415 = _mm256_extracti128_si256::<1>(p_hi_h);
            let v0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo01), lo23);
            let v1 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo45), lo67);
            let v2 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(hi89), hi1011);
            let v3 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(hi1213), hi1415);
            acc0 = _mm256_add_epi64(acc0, v0);
            acc1 = _mm256_add_epi64(acc1, v1);
            acc2 = _mm256_add_epi64(acc2, v2);
            acc3 = _mm256_add_epi64(acc3, v3);
        }
        let mut tmp = [0u64; 4];
        for (acc, base) in [acc0, acc1, acc2, acc3]
            .iter()
            .enumerate()
            .map(|(i, v)| (v, j + 4 * i))
        {
            _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, *acc);
            for (k, &t) in tmp.iter().enumerate() {
                *out.get_unchecked_mut(base + k) = (t % p_u64) as u16;
            }
        }
        j += 16;
    }
    while j < n {
        let mut total: u64 = 0;
        for h in 0..nnz {
            let a_h = *a_vals.get_unchecked(h) as u64;
            let col = *a_cols.get_unchecked(h);
            let b_kj = *b.get_unchecked(col * b_stride + j) as u64;
            total = total.wrapping_add(a_h * b_kj);
        }
        *out.get_unchecked_mut(j) = (total % p_u64) as u16;
        j += 1;
    }
}

/// MR register tile rows for the medium-prime panel kernel. The
/// widen-to-u64 path holds 4 u64 acc ymm per row; any MR > 2 exceeds the
/// 16-register file once the B-load, MR broadcasts and product temps are
/// counted.
const FP_MEDIUM_PANEL_MR: usize = 2;

/// NR register tile cols (one ymm of u16 lanes = 16 cells).
const FP_MEDIUM_PANEL_NR: usize = 16;

/// Outer-N panel grouping (NC blocking of `@/citation/VanZee2015`) for the
/// medium-prime panel kernel: the number of B panels that fit a 16 MB L3
/// budget, sharing one A-pack across every panel in the group.
#[inline]
fn fp_medium_nc_panels_outer(n_panels: usize, k: usize) -> usize {
    const L3_BUDGET_BYTES: usize = 16 * 1024 * 1024;
    let panel_bytes = k
        .saturating_mul(FP_MEDIUM_PANEL_NR)
        .saturating_mul(core::mem::size_of::<u16>());
    if panel_bytes == 0 {
        return n_panels.max(1);
    }
    let blocked = L3_BUDGET_BYTES / panel_bytes;
    blocked.max(1).min(n_panels.max(1))
}

/// Whole-GEMM panel kernel for medium-prime `Fp<P>` with `P in
/// (251, 65535]`.
///
/// Computes `c[i*n + j] = (sum_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) in [0, m) x [0, n)`. Inputs `a` and `bt` carry
/// canonical u16 residues (value `< p`); `bt` is the row-major
/// transpose of B (length `n * k`, row `j` holds column `j` of B).
///
/// # Algorithm
///
/// 1. Pack B into N-major panels of width `NR = 16` (one ymm of
///    u16 lanes), each `k x NR` row-major. For each panel and each
///    k-step the kernel issues one ymm load of B.
/// 2. Process A in MR-row blocks. For each block, sweep every panel,
///    holding MR × 4 u64-lane accumulator vectors live across the full k
///    axis; each step widens the `mullo_epi16` / `mulhi_epu16` products
///    straight into them.
/// 3. After the k sweep, reduce each u64 acc lane mod p and pack the
///    16-cell row back to canonical u16.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime, `p in (251, 2^16)`
/// is an odd prime, and all input lanes are canonical (`< p`).
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_gemm_panel(
    a: &[u16],
    bt: &[u16],
    m: usize,
    k: usize,
    n: usize,
    p: u16,
    c: &mut [u16],
) {
    assert_eq!(a.len(), m * k, "fp_medium_gemm_panel: a shape");
    assert_eq!(bt.len(), n * k, "fp_medium_gemm_panel: bt shape");
    assert_eq!(c.len(), m * n, "fp_medium_gemm_panel: c shape");

    if m == 0 || k == 0 || n == 0 {
        return;
    }

    // Pack B^T into NR-major panels. Panel `pj` covers columns
    // `pj*NR..min((pj+1)*NR, n)` of the original B; each row of the
    // panel is a contiguous ymm-aligned u16 slice of NR cells.
    let n_panels = n.div_ceil(FP_MEDIUM_PANEL_NR);
    let panel_stride = k * FP_MEDIUM_PANEL_NR;
    let mut b_packed: Vec<u16> = vec![0u16; n_panels * panel_stride];
    for panel_idx in 0..n_panels {
        let j_blk = panel_idx * FP_MEDIUM_PANEL_NR;
        let j_end = (j_blk + FP_MEDIUM_PANEL_NR).min(n);
        let n_eff = j_end - j_blk;
        let panel_off = panel_idx * panel_stride;
        for t in 0..k {
            let dst_row_off = panel_off + t * FP_MEDIUM_PANEL_NR;
            for j_off in 0..n_eff {
                b_packed[dst_row_off + j_off] = bt[(j_blk + j_off) * k + t];
            }
            // Slack columns (j_off >= n_eff) stay zero from the
            // initial vec![0u16; _]; the inner kernel still produces
            // a product for them but the output write below skips
            // the slack lanes.
        }
    }

    // NC outer cache-blocking: for each outer-N panel group, sweep every
    // M-row block before moving on.
    let nc_panels = fp_medium_nc_panels_outer(n_panels, k);

    // A-pack scratch, MR-interleaved:
    //   a_pack[t * MR + r] = a[(i_blk + r) * k + t]
    // filled once per i_blk and read unit-stride by every panel in the group.
    let mut a_pack: Vec<u16> = vec![0u16; FP_MEDIUM_PANEL_MR * k];

    let m_full = m - (m % FP_MEDIUM_PANEL_MR);
    let mut n_outer = 0usize;
    while n_outer < n_panels {
        let n_outer_end = (n_outer + nc_panels).min(n_panels);

        let mut i_blk = 0usize;
        while i_blk < m_full {
            fp_medium_pack_a_block::<{ FP_MEDIUM_PANEL_MR }>(a, i_blk, k, &mut a_pack);
            for panel_idx in n_outer..n_outer_end {
                let j_blk = panel_idx * FP_MEDIUM_PANEL_NR;
                let j_end = (j_blk + FP_MEDIUM_PANEL_NR).min(n);
                let n_eff = j_end - j_blk;
                let panel_off = panel_idx * panel_stride;
                fp_medium_panel_run::<{ FP_MEDIUM_PANEL_MR }>(
                    &a_pack, &b_packed, i_blk, k, n, panel_off, j_blk, n_eff, p, c,
                );
            }
            i_blk += FP_MEDIUM_PANEL_MR;
        }
        // Trailing rows: split into MR ∈ {3, 2, 1} cases. The dispatch
        // is monomorphised on M_EFF so the unused row's MAC tower
        // collapses at codegen.
        if i_blk < m {
            let m_eff = m - i_blk;
            macro_rules! run_trailing {
                ($me:literal) => {{
                    fp_medium_pack_a_block::<$me>(a, i_blk, k, &mut a_pack);
                    for panel_idx in n_outer..n_outer_end {
                        let j_blk = panel_idx * FP_MEDIUM_PANEL_NR;
                        let j_end = (j_blk + FP_MEDIUM_PANEL_NR).min(n);
                        let n_eff = j_end - j_blk;
                        let panel_off = panel_idx * panel_stride;
                        fp_medium_panel_run::<$me>(
                            &a_pack, &b_packed, i_blk, k, n, panel_off, j_blk, n_eff, p, c,
                        );
                    }
                }};
            }
            // With FP_MEDIUM_PANEL_MR = 2 the only trailing case is
            // m_eff == 1; M_EFF in {2, 3} monomorphise but never execute.
            match m_eff {
                1 => run_trailing!(1),
                2 => run_trailing!(2),
                3 => run_trailing!(3),
                _ => unreachable!("m_eff in 1..MR (with MR<=4) cannot exceed 3"),
            }
        }
        n_outer = n_outer_end;
    }
}

/// Pack `M_EFF` rows of `a` starting at row `i_blk` into the MR-
/// interleaved scratch `a_pack`, layout `a_pack[t * MR + r] =
/// a[(i_blk + r) * k + t]`. Caller guarantees `a_pack.len() >=
/// FP_MEDIUM_PANEL_MR * k` and `M_EFF <= FP_MEDIUM_PANEL_MR`.
///
/// Monomorphised on `M_EFF` so the row-fold loop collapses to a
/// straight stride-2 write for the steady-state `M_EFF == MR ==
/// 2` case and to a single unit-stride write for `M_EFF == 1`.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_pack_a_block<const M_EFF: usize>(
    a: &[u16],
    i_blk: usize,
    k: usize,
    a_pack: &mut [u16],
) {
    // For each t, write MR contiguous u16 lanes into a_pack. Slack
    // rows (r >= M_EFF) stay at the previous block's residue but are
    // never read by the inner kernel (the kernel guards every row
    // body with `if M_EFF >= ...`), so leaving them stale is sound.
    debug_assert!(M_EFF <= FP_MEDIUM_PANEL_MR);
    debug_assert!(a_pack.len() >= FP_MEDIUM_PANEL_MR * k);
    debug_assert!(a.len() >= (i_blk + M_EFF) * k);
    for t in 0..k {
        let dst = a_pack.as_mut_ptr().add(t * FP_MEDIUM_PANEL_MR);
        if M_EFF >= 1 {
            *dst = *a.get_unchecked(i_blk * k + t);
        }
        if M_EFF >= 2 {
            *dst.add(1) = *a.get_unchecked((i_blk + 1) * k + t);
        }
        if M_EFF >= 3 {
            *dst.add(2) = *a.get_unchecked((i_blk + 2) * k + t);
        }
        if M_EFF >= 4 {
            *dst.add(3) = *a.get_unchecked((i_blk + 3) * k + t);
        }
    }
}

/// One panel-tile inner kernel. Computes a `M_EFF x NR` output tile
/// (at rows `i_blk..i_blk+M_EFF`, cols `j_blk..j_blk+n_eff`).
///
/// Monomorphised on `M_EFF in {1, 2, 3, 4}` so the compiler can prune
/// the dead rows' MAC ops for the trailing-row cases.
///
/// `a_pack` is the MR-interleaved buffer prepared by
/// `fp_medium_pack_a_block`: `a_pack[t * MR + r]` holds the A row-`r`
/// value at column `t` for the current i_blk strip. This converts
/// the MR row reads per k-step into a single contiguous load.
#[inline]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
unsafe fn fp_medium_panel_run<const M_EFF: usize>(
    a_pack: &[u16],
    b_packed: &[u16],
    i_blk: usize,
    k: usize,
    n: usize,
    panel_off: usize,
    j_blk: usize,
    n_eff: usize,
    p: u16,
    c: &mut [u16],
) {
    // Accumulators: each row needs 4 ymm of u64 lanes (4 u64 lanes
    // per ymm) to cover 16 output cells.
    let mut acc0_0 = _mm256_setzero_si256();
    let mut acc0_1 = _mm256_setzero_si256();
    let mut acc0_2 = _mm256_setzero_si256();
    let mut acc0_3 = _mm256_setzero_si256();
    let mut acc1_0 = _mm256_setzero_si256();
    let mut acc1_1 = _mm256_setzero_si256();
    let mut acc1_2 = _mm256_setzero_si256();
    let mut acc1_3 = _mm256_setzero_si256();
    let mut acc2_0 = _mm256_setzero_si256();
    let mut acc2_1 = _mm256_setzero_si256();
    let mut acc2_2 = _mm256_setzero_si256();
    let mut acc2_3 = _mm256_setzero_si256();
    let mut acc3_0 = _mm256_setzero_si256();
    let mut acc3_1 = _mm256_setzero_si256();
    let mut acc3_2 = _mm256_setzero_si256();
    let mut acc3_3 = _mm256_setzero_si256();
    let zero = _mm256_setzero_si256();

    let a_pack_ptr = a_pack.as_ptr();
    let b_panel_ptr = b_packed.as_ptr().add(panel_off);

    for t in 0..k {
        let bv = _mm256_loadu_si256(b_panel_ptr.add(t * FP_MEDIUM_PANEL_NR) as *const __m256i);

        let a_pack_t_base = a_pack_ptr.add(t * FP_MEDIUM_PANEL_MR);

        if M_EFF >= 1 {
            let a0_val = *a_pack_t_base;
            let av0 = _mm256_set1_epi16(a0_val as i16);
            let prod_lo = _mm256_mullo_epi16(av0, bv);
            let prod_hi = _mm256_mulhi_epu16(av0, bv);
            let prod_full_lo = _mm256_unpacklo_epi16(prod_lo, prod_hi);
            let prod_full_hi = _mm256_unpackhi_epi16(prod_lo, prod_hi);
            let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
            let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
            let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
            let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);
            acc0_0 = _mm256_add_epi64(acc0_0, p_lo_l);
            acc0_1 = _mm256_add_epi64(acc0_1, p_lo_h);
            acc0_2 = _mm256_add_epi64(acc0_2, p_hi_l);
            acc0_3 = _mm256_add_epi64(acc0_3, p_hi_h);
        }
        if M_EFF >= 2 {
            let a1_val = *a_pack_t_base.add(1);
            let av1 = _mm256_set1_epi16(a1_val as i16);
            let prod_lo = _mm256_mullo_epi16(av1, bv);
            let prod_hi = _mm256_mulhi_epu16(av1, bv);
            let prod_full_lo = _mm256_unpacklo_epi16(prod_lo, prod_hi);
            let prod_full_hi = _mm256_unpackhi_epi16(prod_lo, prod_hi);
            let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
            let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
            let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
            let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);
            acc1_0 = _mm256_add_epi64(acc1_0, p_lo_l);
            acc1_1 = _mm256_add_epi64(acc1_1, p_lo_h);
            acc1_2 = _mm256_add_epi64(acc1_2, p_hi_l);
            acc1_3 = _mm256_add_epi64(acc1_3, p_hi_h);
        }
        if M_EFF >= 3 {
            let a2_val = *a_pack_t_base.add(2);
            let av2 = _mm256_set1_epi16(a2_val as i16);
            let prod_lo = _mm256_mullo_epi16(av2, bv);
            let prod_hi = _mm256_mulhi_epu16(av2, bv);
            let prod_full_lo = _mm256_unpacklo_epi16(prod_lo, prod_hi);
            let prod_full_hi = _mm256_unpackhi_epi16(prod_lo, prod_hi);
            let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
            let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
            let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
            let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);
            acc2_0 = _mm256_add_epi64(acc2_0, p_lo_l);
            acc2_1 = _mm256_add_epi64(acc2_1, p_lo_h);
            acc2_2 = _mm256_add_epi64(acc2_2, p_hi_l);
            acc2_3 = _mm256_add_epi64(acc2_3, p_hi_h);
        }
        if M_EFF >= 4 {
            let a3_val = *a_pack_t_base.add(3);
            let av3 = _mm256_set1_epi16(a3_val as i16);
            let prod_lo = _mm256_mullo_epi16(av3, bv);
            let prod_hi = _mm256_mulhi_epu16(av3, bv);
            let prod_full_lo = _mm256_unpacklo_epi16(prod_lo, prod_hi);
            let prod_full_hi = _mm256_unpackhi_epi16(prod_lo, prod_hi);
            let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
            let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
            let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
            let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);
            acc3_0 = _mm256_add_epi64(acc3_0, p_lo_l);
            acc3_1 = _mm256_add_epi64(acc3_1, p_lo_h);
            acc3_2 = _mm256_add_epi64(acc3_2, p_hi_l);
            acc3_3 = _mm256_add_epi64(acc3_3, p_hi_h);
        }
    }

    // Reduce per-lane mod p and write the M_EFF rows of the tile to `c`.
    // The unpack_epi16 / unpack_epi32 chain works per 128-bit half, so the
    // u64 lanes hold output cells
    //   acc*_0 (p_lo_l) {0, 1, 8, 9}
    //   acc*_1 (p_lo_h) {2, 3, 10, 11}
    //   acc*_2 (p_hi_l) {4, 5, 12, 13}
    //   acc*_3 (p_hi_h) {6, 7, 14, 15}
    let p_u64 = p as u64;
    let mut row_buf = [0u64; 16];
    if M_EFF >= 1 {
        let mut tmp = [0u64; 4];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc0_0);
        row_buf[0] = tmp[0];
        row_buf[1] = tmp[1];
        row_buf[8] = tmp[2];
        row_buf[9] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc0_1);
        row_buf[2] = tmp[0];
        row_buf[3] = tmp[1];
        row_buf[10] = tmp[2];
        row_buf[11] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc0_2);
        row_buf[4] = tmp[0];
        row_buf[5] = tmp[1];
        row_buf[12] = tmp[2];
        row_buf[13] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc0_3);
        row_buf[6] = tmp[0];
        row_buf[7] = tmp[1];
        row_buf[14] = tmp[2];
        row_buf[15] = tmp[3];
        let c_row0_base = i_blk * n + j_blk;
        for (j_off, slot) in row_buf.iter().take(n_eff).enumerate() {
            *c.get_unchecked_mut(c_row0_base + j_off) = (*slot % p_u64) as u16;
        }
    }
    if M_EFF >= 2 {
        let mut tmp = [0u64; 4];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc1_0);
        row_buf[0] = tmp[0];
        row_buf[1] = tmp[1];
        row_buf[8] = tmp[2];
        row_buf[9] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc1_1);
        row_buf[2] = tmp[0];
        row_buf[3] = tmp[1];
        row_buf[10] = tmp[2];
        row_buf[11] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc1_2);
        row_buf[4] = tmp[0];
        row_buf[5] = tmp[1];
        row_buf[12] = tmp[2];
        row_buf[13] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc1_3);
        row_buf[6] = tmp[0];
        row_buf[7] = tmp[1];
        row_buf[14] = tmp[2];
        row_buf[15] = tmp[3];
        let c_row1_base = (i_blk + 1) * n + j_blk;
        for (j_off, slot) in row_buf.iter().take(n_eff).enumerate() {
            *c.get_unchecked_mut(c_row1_base + j_off) = (*slot % p_u64) as u16;
        }
    }
    if M_EFF >= 3 {
        let mut tmp = [0u64; 4];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc2_0);
        row_buf[0] = tmp[0];
        row_buf[1] = tmp[1];
        row_buf[8] = tmp[2];
        row_buf[9] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc2_1);
        row_buf[2] = tmp[0];
        row_buf[3] = tmp[1];
        row_buf[10] = tmp[2];
        row_buf[11] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc2_2);
        row_buf[4] = tmp[0];
        row_buf[5] = tmp[1];
        row_buf[12] = tmp[2];
        row_buf[13] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc2_3);
        row_buf[6] = tmp[0];
        row_buf[7] = tmp[1];
        row_buf[14] = tmp[2];
        row_buf[15] = tmp[3];
        let c_row2_base = (i_blk + 2) * n + j_blk;
        for (j_off, slot) in row_buf.iter().take(n_eff).enumerate() {
            *c.get_unchecked_mut(c_row2_base + j_off) = (*slot % p_u64) as u16;
        }
    }
    if M_EFF >= 4 {
        let mut tmp = [0u64; 4];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc3_0);
        row_buf[0] = tmp[0];
        row_buf[1] = tmp[1];
        row_buf[8] = tmp[2];
        row_buf[9] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc3_1);
        row_buf[2] = tmp[0];
        row_buf[3] = tmp[1];
        row_buf[10] = tmp[2];
        row_buf[11] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc3_2);
        row_buf[4] = tmp[0];
        row_buf[5] = tmp[1];
        row_buf[12] = tmp[2];
        row_buf[13] = tmp[3];
        _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc3_3);
        row_buf[6] = tmp[0];
        row_buf[7] = tmp[1];
        row_buf[14] = tmp[2];
        row_buf[15] = tmp[3];
        let c_row3_base = (i_blk + 3) * n + j_blk;
        for (j_off, slot) in row_buf.iter().take(n_eff).enumerate() {
            *c.get_unchecked_mut(c_row3_base + j_off) = (*slot % p_u64) as u16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::fp_medium::barrett_m32;

    const P_65521: u16 = 65521;
    const M_65521: u32 = barrett_m32(P_65521);

    fn scalar_mul(a: u16, b: u16, p: u16) -> u16 {
        ((a as u32 * b as u32) % p as u32) as u16
    }

    fn scalar_add(a: u16, b: u16, p: u16) -> u16 {
        let s = a as u32 + b as u32;
        (if s >= p as u32 { s - p as u32 } else { s }) as u16
    }

    fn scalar_sub(a: u16, b: u16, p: u16) -> u16 {
        let d = a as u32 + p as u32 - b as u32;
        (if d >= p as u32 { d - p as u32 } else { d }) as u16
    }

    #[test]
    fn batch_mul_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u16> = (0..50u16).map(|i| (i * 137) % P_65521).collect();
        let b: Vec<u16> = (0..50u16).map(|i| (i * 211 + 17) % P_65521).collect();
        let mut out = vec![0u16; 50];
        unsafe { fp_medium_batch_mul(&a, &b, P_65521, M_65521, &mut out) };
        for i in 0..50 {
            assert_eq!(out[i], scalar_mul(a[i], b[i], P_65521), "i={i}");
        }
    }

    #[test]
    fn batch_mul_boundary_values_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // 65520 = P - 1 stresses the (P-1)² ≈ 2^32 overflow boundary.
        let a = vec![
            0u16, 1, 65520, 32760, 1, 65520, 0, 65519, 65520, 32761, 1, 100, 0, 65520, 32760, 1,
        ];
        let b = vec![
            65520u16, 0, 65520, 2, 32760, 1, 0, 3, 65519, 32761, 65520, 100, 0, 65520, 2, 32760,
        ];
        let mut out = vec![0u16; 16];
        unsafe { fp_medium_batch_mul(&a, &b, P_65521, M_65521, &mut out) };
        for i in 0..16 {
            assert_eq!(out[i], scalar_mul(a[i], b[i], P_65521), "i={i}");
        }
    }

    #[test]
    fn batch_mul_with_tail_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 7, 15, 16, 17, 31, 32, 33, 100, 1024, 4097] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 137) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 211 + 7) % P_65521 as u32) as u16)
                .collect();
            let mut out = vec![0u16; len];
            unsafe { fp_medium_batch_mul(&a, &b, P_65521, M_65521, &mut out) };
            for i in 0..len {
                assert_eq!(out[i], scalar_mul(a[i], b[i], P_65521), "len={len} i={i}");
            }
        }
    }

    #[test]
    fn batch_add_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 15, 16, 17, 100] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 4093) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 9973) % P_65521 as u32) as u16)
                .collect();
            let mut out = vec![0u16; len];
            unsafe { fp_medium_batch_add(&a, &b, P_65521, &mut out) };
            for i in 0..len {
                assert_eq!(out[i], scalar_add(a[i], b[i], P_65521), "len={len} i={i}");
            }
        }
    }

    #[test]
    fn batch_sub_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 15, 16, 17, 100] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 4093) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 9973) % P_65521 as u32) as u16)
                .collect();
            let mut out = vec![0u16; len];
            unsafe { fp_medium_batch_sub(&a, &b, P_65521, &mut out) };
            for i in 0..len {
                assert_eq!(out[i], scalar_sub(a[i], b[i], P_65521), "len={len} i={i}");
            }
        }
    }

    #[test]
    fn batch_dot_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 15, 16, 17, 100, 256, 1024] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 17) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 23 + 5) % P_65521 as u32) as u16)
                .collect();
            let got = unsafe { fp_medium_batch_dot(&a, &b, P_65521) };
            let mut expected: u64 = 0;
            for i in 0..len {
                expected += (a[i] as u64) * (b[i] as u64);
            }
            assert_eq!(got as u64, expected % P_65521 as u64, "len={len}");
        }
    }

    #[test]
    fn batch_dot_boundary_values_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // Worst-case lane saturation: every product is (P-1)² ≈ 2^32.
        let a = vec![65520u16; 1024];
        let b = vec![65520u16; 1024];
        let got = unsafe { fp_medium_batch_dot(&a, &b, P_65521) };
        let expected = (1024u64 * 65520u64 * 65520u64) % P_65521 as u64;
        assert_eq!(got as u64, expected);
    }

    #[test]
    fn smaller_medium_primes_match_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &p in &[257u16, 509, 1009, 8191, 32749] {
            let m = barrett_m32(p);
            let a: Vec<u16> = (0..200)
                .map(|i| ((i as u32 * 17) % p as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..200)
                .map(|i| ((i as u32 * 23 + 5) % p as u32) as u16)
                .collect();
            let mut out = vec![0u16; 200];
            unsafe { fp_medium_batch_mul(&a, &b, p, m, &mut out) };
            for i in 0..200 {
                assert_eq!(out[i], scalar_mul(a[i], b[i], p), "p={p} i={i}");
            }
            let got = unsafe { fp_medium_batch_dot(&a, &b, p) };
            let mut expected: u64 = 0;
            for i in 0..200 {
                expected += (a[i] as u64) * (b[i] as u64);
            }
            assert_eq!(got as u64, expected % p as u64, "dot p={p}");
        }
    }

    #[test]
    fn spmm_row_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &p in &[257u16, 65521, 8191, 1009] {
            let cases = [
                (1usize, 16usize, 8usize),
                (5, 16, 32),
                (7, 16, 33),
                (10, 16, 64),
                (1, 16, 17),
                (20, 16, 100),
                (3, 8, 16),
                (10, 32, 128),
                (15, 16, 1024),
            ];
            for &(nnz, b_rows, n) in &cases {
                let a_vals: Vec<u16> = (0..nnz as u32)
                    .map(|h| ((h * 13 + 1) % p as u32) as u16)
                    .collect();
                let a_cols: Vec<usize> = (0..nnz).map(|h| (h * 7) % b_rows).collect();
                let b: Vec<u16> = (0..(b_rows * n) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u16)
                    .collect();
                let mut out = vec![0u16; n];
                unsafe { fp_medium_spmm_row(&a_vals, &a_cols, &b, n, n, p, &mut out) };
                for (j, &val) in out.iter().enumerate() {
                    let mut expected: u64 = 0;
                    for h in 0..nnz {
                        let col = a_cols[h];
                        expected = expected.wrapping_add(a_vals[h] as u64 * b[col * n + j] as u64);
                    }
                    expected %= p as u64;
                    assert_eq!(
                        val as u64, expected,
                        "p={p} nnz={nnz} b_rows={b_rows} n={n} j={j}"
                    );
                }
            }
        }
    }

    #[test]
    fn spmm_row_empty_nnz() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let p = 65521u16;
        let n = 64;
        let a_vals: Vec<u16> = vec![];
        let a_cols: Vec<usize> = vec![];
        let b: Vec<u16> = (0..n).map(|i| ((i as u32 * 7) % p as u32) as u16).collect();
        let mut out = vec![5u16; n];
        unsafe { fp_medium_spmm_row(&a_vals, &a_cols, &b, n, n, p, &mut out) };
        for (j, &val) in out.iter().enumerate().take(n) {
            assert_eq!(val, 0, "j={j}");
        }
    }

    fn scalar_gemm_u16(a: &[u16], bt: &[u16], m: usize, k: usize, n: usize, p: u16) -> Vec<u16> {
        let mut out = vec![0u16; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut acc: u64 = 0;
                for t in 0..k {
                    acc += a[i * k + t] as u64 * bt[j * k + t] as u64;
                }
                out[i * n + j] = (acc % p as u64) as u16;
            }
        }
        out
    }

    #[test]
    fn gemm_panel_matches_scalar_at_boundary_shapes() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let cases = [
            (1usize, 1usize, 1usize),
            (1, 16, 16),
            (1, 16, 17),
            (2, 16, 16),
            (3, 16, 16),
            (4, 16, 16),
            (2, 1, 16),
            (2, 33, 16),
            (2, 16, 32),
            (5, 4, 17),
            (7, 32, 13),
            (8, 64, 24),
            (15, 32, 33),
            (16, 64, 64),
            (32, 64, 32),
        ];
        for &p in &[257u16, 1009, 8191, 32749, 65521] {
            for &(m, k, n) in &cases {
                let a: Vec<u16> = (0..(m * k))
                    .map(|i| ((i as u32 * 17 + 3) % p as u32) as u16)
                    .collect();
                let bt: Vec<u16> = (0..(n * k))
                    .map(|i| ((i as u32 * 23 + 7) % p as u32) as u16)
                    .collect();
                let mut got = vec![0u16; m * n];
                unsafe { fp_medium_gemm_panel(&a, &bt, m, k, n, p, &mut got) };
                let expected = scalar_gemm_u16(&a, &bt, m, k, n, p);
                assert_eq!(got, expected, "p={p} m={m} k={k} n={n}");
            }
        }
    }

    #[test]
    fn gemm_panel_boundary_values() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // Worst-case lane saturation: every product is (P-1)² near 2^32.
        let p = 65521u16;
        let m = 4;
        let k = 1024;
        let n = 16;
        let a: Vec<u16> = vec![65520u16; m * k];
        let bt: Vec<u16> = vec![65520u16; n * k];
        let mut got = vec![0u16; m * n];
        unsafe { fp_medium_gemm_panel(&a, &bt, m, k, n, p, &mut got) };
        let cell = ((k as u64) * 65520u64 * 65520u64) % p as u64;
        for &v in &got {
            assert_eq!(v as u64, cell);
        }
    }

    #[test]
    fn gemm_panel_zero_outer_dims() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // n=0 and m=0 are no-ops; the kernel must still execute the
        // early-return path without scribbling on the empty c slice.
        let p = 65521u16;
        let mut out0 = vec![];
        unsafe { fp_medium_gemm_panel(&[], &[], 0, 0, 0, p, &mut out0) };
        let mut out_m = vec![];
        unsafe { fp_medium_gemm_panel(&[], &[42u16; 4], 0, 1, 4, p, &mut out_m) };
        let mut out_n = vec![];
        unsafe { fp_medium_gemm_panel(&[42u16; 4], &[], 4, 1, 0, p, &mut out_n) };
        let mut out_k = vec![0u16; 4 * 4];
        unsafe { fp_medium_gemm_panel(&[], &[], 4, 0, 4, p, &mut out_k) };
        // k=0 leaves c unchanged at its caller-provided initial value.
        for &v in &out_k {
            assert_eq!(v, 0);
        }
    }
}
