//! AVX2 Montgomery batch kernels for generic `Fp<P>` storage words.
//!
//! AVX2 has no packed `u64 × u64 → u128` instruction, so the multiplier uses a
//! 32×32 limb decomposition in four lanes. The Montgomery REDC step then
//! reuses the same multiplier for `m·P`, keeping the whole vector loop in YMM
//! registers and falling back to scalar code only for the tail.

#![allow(clippy::missing_safety_doc)]

use core::arch::x86_64::*;

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn unsigned_lt_epi64(a: __m256i, b: __m256i) -> __m256i {
    let sign = _mm256_set1_epi64x(i64::MIN);
    _mm256_cmpgt_epi64(_mm256_xor_si256(b, sign), _mm256_xor_si256(a, sign))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn select_epi64(mask: __m256i, when_true: __m256i, when_false: __m256i) -> __m256i {
    _mm256_or_si256(
        _mm256_and_si256(mask, when_true),
        _mm256_andnot_si256(mask, when_false),
    )
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn add_mod_u64x4(a: __m256i, b: __m256i, p: __m256i) -> __m256i {
    let sum = _mm256_add_epi64(a, b);
    let diff = _mm256_sub_epi64(sum, p);
    let overflow = unsigned_lt_epi64(sum, a);
    let ge_p = _mm256_xor_si256(unsigned_lt_epi64(sum, p), _mm256_set1_epi64x(-1));
    select_epi64(_mm256_or_si256(overflow, ge_p), diff, sum)
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn sub_mod_u64x4(a: __m256i, b: __m256i, p: __m256i) -> __m256i {
    let diff = _mm256_sub_epi64(a, b);
    let borrow = unsigned_lt_epi64(a, b);
    _mm256_add_epi64(diff, _mm256_and_si256(borrow, p))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn mul_lo_u64x4(a: __m256i, b: __m256i) -> __m256i {
    let mask32 = _mm256_set1_epi64x(0xFFFF_FFFF);
    let a0 = _mm256_and_si256(a, mask32);
    let b0 = _mm256_and_si256(b, mask32);
    let a1 = _mm256_srli_epi64(a, 32);
    let b1 = _mm256_srli_epi64(b, 32);

    let p0 = _mm256_mul_epu32(a0, b0);
    let p1 = _mm256_mul_epu32(a0, b1);
    let p2 = _mm256_mul_epu32(a1, b0);
    _mm256_add_epi64(p0, _mm256_slli_epi64(_mm256_add_epi64(p1, p2), 32))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn mul_wide_u64x4(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
    let mask32 = _mm256_set1_epi64x(0xFFFF_FFFF);
    let a0 = _mm256_and_si256(a, mask32);
    let b0 = _mm256_and_si256(b, mask32);
    let a1 = _mm256_srli_epi64(a, 32);
    let b1 = _mm256_srli_epi64(b, 32);

    let ll = _mm256_mul_epu32(a0, b0);
    let lh = _mm256_mul_epu32(a0, b1);
    let hl = _mm256_mul_epu32(a1, b0);
    let hh = _mm256_mul_epu32(a1, b1);

    let ll_hi = _mm256_srli_epi64(ll, 32);
    let lh_lo = _mm256_and_si256(lh, mask32);
    let hl_lo = _mm256_and_si256(hl, mask32);
    let mid = _mm256_add_epi64(_mm256_add_epi64(ll_hi, lh_lo), hl_lo);

    let lo = _mm256_or_si256(_mm256_slli_epi64(mid, 32), _mm256_and_si256(ll, mask32));
    let hi = _mm256_add_epi64(
        _mm256_add_epi64(hh, _mm256_srli_epi64(lh, 32)),
        _mm256_add_epi64(_mm256_srli_epi64(hl, 32), _mm256_srli_epi64(mid, 32)),
    );

    (lo, hi)
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn mul_hi_u64x4(a: __m256i, b: __m256i) -> __m256i {
    let mask32 = _mm256_set1_epi64x(0xFFFF_FFFF);
    let a0 = _mm256_and_si256(a, mask32);
    let b0 = _mm256_and_si256(b, mask32);
    let a1 = _mm256_srli_epi64(a, 32);
    let b1 = _mm256_srli_epi64(b, 32);

    let ll = _mm256_mul_epu32(a0, b0);
    let lh = _mm256_mul_epu32(a0, b1);
    let hl = _mm256_mul_epu32(a1, b0);
    let hh = _mm256_mul_epu32(a1, b1);

    let mid = _mm256_add_epi64(
        _mm256_add_epi64(_mm256_srli_epi64(ll, 32), _mm256_and_si256(lh, mask32)),
        _mm256_and_si256(hl, mask32),
    );

    _mm256_add_epi64(
        _mm256_add_epi64(hh, _mm256_srli_epi64(lh, 32)),
        _mm256_add_epi64(_mm256_srli_epi64(hl, 32), _mm256_srli_epi64(mid, 32)),
    )
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn montgomery_redc_u64x4(
    t_lo: __m256i,
    t_hi: __m256i,
    p: __m256i,
    p_inv: __m256i,
) -> __m256i {
    let m = mul_lo_u64x4(t_lo, p_inv);
    let mp_hi = mul_hi_u64x4(m, p);

    // Montgomery construction makes t_lo + (m * p)_lo either 0 (no carry) or
    // 2^64 (carry 1), so the carry into the high half is exactly t_lo != 0.
    let zero = _mm256_setzero_si256();
    let carry = _mm256_andnot_si256(_mm256_cmpeq_epi64(t_lo, zero), _mm256_set1_epi64x(1));
    let u = _mm256_add_epi64(_mm256_add_epi64(t_hi, mp_hi), carry);
    let u_minus_p = _mm256_sub_epi64(u, p);
    select_epi64(unsigned_lt_epi64(u, p), u, u_minus_p)
}

#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn fp_montgomery_mul4(a: __m256i, b: __m256i, p: __m256i, p_inv: __m256i) -> __m256i {
    let (t_lo, t_hi) = mul_wide_u64x4(a, b);
    montgomery_redc_u64x4(t_lo, t_hi, p, p_inv)
}

#[target_feature(enable = "avx2")]
pub unsafe fn fp_montgomery_batch_mul(
    a: &[u64],
    b: &[u64],
    modulus: u64,
    p_inv: u64,
    out: &mut [u64],
) {
    assert_eq!(a.len(), b.len(), "fp_montgomery_batch_mul: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_montgomery_batch_mul: output length");
    assert!(
        modulus <= (1u64 << 63),
        "fp_montgomery_batch_mul: modulus too large"
    );

    let n = a.len();
    let nvec = n / 4;
    let p = _mm256_set1_epi64x(modulus as i64);
    let inv = _mm256_set1_epi64x(p_inv as i64);
    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp_montgomery_mul4(av, bv, p, inv);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    for i in (nvec * 4)..n {
        let t = *a.get_unchecked(i) as u128 * *b.get_unchecked(i) as u128;
        let m = (t as u64).wrapping_mul(p_inv);
        let u = ((t + m as u128 * modulus as u128) >> 64) as u64;
        *out.get_unchecked_mut(i) = if u >= modulus { u - modulus } else { u };
    }
}

#[target_feature(enable = "avx2")]
pub unsafe fn fp_montgomery_batch_add(a: &[u64], b: &[u64], modulus: u64, out: &mut [u64]) {
    assert_eq!(a.len(), b.len(), "fp_montgomery_batch_add: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_montgomery_batch_add: output length");

    let n = a.len();
    let nvec = n / 4;
    let p = _mm256_set1_epi64x(modulus as i64);
    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;
    for i in 0..nvec {
        let rv = add_mod_u64x4(
            _mm256_loadu_si256(a_ptr.add(i)),
            _mm256_loadu_si256(b_ptr.add(i)),
            p,
        );
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }
    for i in (nvec * 4)..n {
        let sum = (*a.get_unchecked(i)).wrapping_add(*b.get_unchecked(i));
        let overflow = sum < *a.get_unchecked(i);
        *out.get_unchecked_mut(i) = if overflow || sum >= modulus {
            sum.wrapping_sub(modulus)
        } else {
            sum
        };
    }
}

#[target_feature(enable = "avx2")]
pub unsafe fn fp_montgomery_batch_sub(a: &[u64], b: &[u64], modulus: u64, out: &mut [u64]) {
    assert_eq!(a.len(), b.len(), "fp_montgomery_batch_sub: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_montgomery_batch_sub: output length");

    let n = a.len();
    let nvec = n / 4;
    let p = _mm256_set1_epi64x(modulus as i64);
    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;
    for i in 0..nvec {
        let rv = sub_mod_u64x4(
            _mm256_loadu_si256(a_ptr.add(i)),
            _mm256_loadu_si256(b_ptr.add(i)),
            p,
        );
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }
    for i in (nvec * 4)..n {
        let (diff, borrow) = (*a.get_unchecked(i)).overflowing_sub(*b.get_unchecked(i));
        *out.get_unchecked_mut(i) = if borrow {
            diff.wrapping_add(modulus)
        } else {
            diff
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: u64 = 9_223_372_036_854_775_783; // 2^63 - 25
    const P_INV: u64 = {
        let mut inv: u64 = 1;
        let mut i = 0;
        while i < 6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(P.wrapping_mul(inv)));
            i += 1;
        }
        inv.wrapping_neg()
    };

    fn redc(t: u128) -> u64 {
        let m = (t as u64).wrapping_mul(P_INV);
        let u = ((t + m as u128 * P as u128) >> 64) as u64;
        if u >= P {
            u - P
        } else {
            u
        }
    }

    #[test]
    fn batch_mul_matches_scalar_word_boundaries() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257] {
            let a: Vec<u64> = (0..len as u64)
                .map(|i| (i.wrapping_mul(123_456_789_123) + 7) % P)
                .collect();
            let b: Vec<u64> = (0..len as u64)
                .map(|i| (i.wrapping_mul(987_654_321_987) + 11) % P)
                .collect();
            let mut out = vec![0u64; len];
            unsafe { fp_montgomery_batch_mul(&a, &b, P, P_INV, &mut out) };
            for i in 0..len {
                assert_eq!(
                    out[i],
                    redc(a[i] as u128 * b[i] as u128),
                    "len={len}, i={i}"
                );
            }
        }
    }
}
