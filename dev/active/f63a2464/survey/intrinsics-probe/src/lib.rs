//! AVX2 intrinsic forms the candidate families need, at the repository MSRV.
//!
//! Each function is one operation a candidate's inner loop performs on a
//! full AVX2 register, paired with the scalar reference that defines what it
//! must compute. The tests run both over exhaustive or boundary-covering
//! inputs, so a form that compiles but computes something else fails here
//! rather than in a prototype. The assembly of these functions is the
//! feasibility artefact; an instruction list alone is not such evidence.
//!
//! # Safety
//!
//! Every `unsafe` function requires the `avx2` target feature at the call
//! site. Each is `#[target_feature(enable = "avx2")]`, so the caller carries
//! the obligation to have detected the feature at run time. The loads and
//! stores are register-width `loadu`/`storeu` on slices the caller has already
//! bounds-checked to 32 or 16 elements, so no alignment or aliasing
//! obligation is added beyond feature detection.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Family Q, `i8`: saturating add, the variable-node accumulation step.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn saturating_add_i8(a: &[i8; 32], b: &[i8; 32], out: &mut [i8; 32]) {
    unsafe {
        let va = _mm256_loadu_si256(a.as_ptr().cast());
        let vb = _mm256_loadu_si256(b.as_ptr().cast());
        _mm256_storeu_si256(out.as_mut_ptr().cast(), _mm256_adds_epi8(va, vb));
    }
}

/// Family Q, `i8`: saturating subtract, the leave-one-out step of the
/// variable-node update.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn saturating_sub_i8(a: &[i8; 32], b: &[i8; 32], out: &mut [i8; 32]) {
    unsafe {
        let va = _mm256_loadu_si256(a.as_ptr().cast());
        let vb = _mm256_loadu_si256(b.as_ptr().cast());
        _mm256_storeu_si256(out.as_mut_ptr().cast(), _mm256_subs_epi8(va, vb));
    }
}

/// Family Q, `i8`: magnitude, minimum and second minimum folded in one step.
///
/// `min1` and `min2` carry the running two smallest magnitudes; the returned
/// pair is the fold with `x`'s magnitude included. The magnitude of the
/// symmetric alphabet is exact because `-128` is excluded from it.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn fold_two_minima_i8(
    min1: &[i8; 32],
    min2: &[i8; 32],
    x: &[i8; 32],
    out1: &mut [i8; 32],
    out2: &mut [i8; 32],
) {
    unsafe {
        let m1 = _mm256_loadu_si256(min1.as_ptr().cast());
        let m2 = _mm256_loadu_si256(min2.as_ptr().cast());
        let mag = _mm256_abs_epi8(_mm256_loadu_si256(x.as_ptr().cast()));
        let new1 = _mm256_min_epi8(m1, mag);
        let new2 = _mm256_min_epi8(m2, _mm256_max_epi8(m1, mag));
        _mm256_storeu_si256(out1.as_mut_ptr().cast(), new1);
        _mm256_storeu_si256(out2.as_mut_ptr().cast(), new2);
    }
}

/// Family Q, `i8`: sign extraction as an all-ones mask, and its application.
///
/// `mask` is all ones in a lane whose input is negative, which is the form the
/// running sign product accumulates by exclusive or. `apply` negates a
/// magnitude under such a mask without the zero-lane special case of
/// `_mm256_sign_epi8`.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn sign_mask_and_apply_i8(
    x: &[i8; 32],
    magnitude: &[i8; 32],
    mask_out: &mut [i8; 32],
    signed_out: &mut [i8; 32],
) {
    unsafe {
        let vx = _mm256_loadu_si256(x.as_ptr().cast());
        let mask = _mm256_cmpgt_epi8(_mm256_setzero_si256(), vx);
        let mag = _mm256_loadu_si256(magnitude.as_ptr().cast());
        // Two's-complement negation under a mask: (m ^ mask) - mask.
        let signed = _mm256_sub_epi8(_mm256_xor_si256(mag, mask), mask);
        _mm256_storeu_si256(mask_out.as_mut_ptr().cast(), mask);
        _mm256_storeu_si256(signed_out.as_mut_ptr().cast(), signed);
    }
}

/// Family Q, `i8`: clip to the symmetric alphabet `[-limit, limit]`.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn clip_symmetric_i8(x: &[i8; 32], limit: i8, out: &mut [i8; 32]) {
    unsafe {
        let vx = _mm256_loadu_si256(x.as_ptr().cast());
        let hi = _mm256_set1_epi8(limit);
        let lo = _mm256_set1_epi8(-limit);
        let clipped = _mm256_max_epi8(lo, _mm256_min_epi8(hi, vx));
        _mm256_storeu_si256(out.as_mut_ptr().cast(), clipped);
    }
}

/// Family Q, `i16`: saturating add.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn saturating_add_i16(a: &[i16; 16], b: &[i16; 16], out: &mut [i16; 16]) {
    unsafe {
        let va = _mm256_loadu_si256(a.as_ptr().cast());
        let vb = _mm256_loadu_si256(b.as_ptr().cast());
        _mm256_storeu_si256(out.as_mut_ptr().cast(), _mm256_adds_epi16(va, vb));
    }
}

/// Family Q, `i16`: saturating subtract.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn saturating_sub_i16(a: &[i16; 16], b: &[i16; 16], out: &mut [i16; 16]) {
    unsafe {
        let va = _mm256_loadu_si256(a.as_ptr().cast());
        let vb = _mm256_loadu_si256(b.as_ptr().cast());
        _mm256_storeu_si256(out.as_mut_ptr().cast(), _mm256_subs_epi16(va, vb));
    }
}

/// Family Q, `i16`: magnitude and two-minimum fold.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn fold_two_minima_i16(
    min1: &[i16; 16],
    min2: &[i16; 16],
    x: &[i16; 16],
    out1: &mut [i16; 16],
    out2: &mut [i16; 16],
) {
    unsafe {
        let m1 = _mm256_loadu_si256(min1.as_ptr().cast());
        let m2 = _mm256_loadu_si256(min2.as_ptr().cast());
        let mag = _mm256_abs_epi16(_mm256_loadu_si256(x.as_ptr().cast()));
        let new1 = _mm256_min_epi16(m1, mag);
        let new2 = _mm256_min_epi16(m2, _mm256_max_epi16(m1, mag));
        _mm256_storeu_si256(out1.as_mut_ptr().cast(), new1);
        _mm256_storeu_si256(out2.as_mut_ptr().cast(), new2);
    }
}

/// Family QC: the f32 lanes of one lifted block, magnitude and sign.
///
/// The lanes are distinct checks of the lifted code, so this is the same
/// per-lane arithmetic the scalar reduction performs, with no reassociation.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn magnitude_and_sign_f32(
    x: &[f32; 8],
    magnitude_out: &mut [f32; 8],
    sign_bits_out: &mut [f32; 8],
) {
    unsafe {
        let vx = _mm256_loadu_ps(x.as_ptr());
        let abs_mask = _mm256_castsi256_ps(_mm256_set1_epi32(0x7fff_ffff));
        let sign_mask = _mm256_castsi256_ps(_mm256_set1_epi32(-0x8000_0000i32));
        _mm256_storeu_ps(magnitude_out.as_mut_ptr(), _mm256_and_ps(vx, abs_mask));
        _mm256_storeu_ps(sign_bits_out.as_mut_ptr(), _mm256_and_ps(vx, sign_mask));
    }
}

/// Family QC: the comparison-based sign rule of the canonical contract.
///
/// The float contract takes a sign by comparison against zero, so a
/// negative-zero input counts as positive. `_mm256_cmp_ps` with `_CMP_LT_OS`
/// reproduces that rule; the IEEE sign bit does not.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn negative_by_comparison_f32(x: &[f32; 8], mask_out: &mut [f32; 8]) {
    unsafe {
        let vx = _mm256_loadu_ps(x.as_ptr());
        let zero = _mm256_setzero_ps();
        _mm256_storeu_ps(mask_out.as_mut_ptr(), _mm256_cmp_ps(vx, zero, _CMP_LT_OS));
    }
}

/// Family QC: a cyclic rotation of eight f32 lanes by a run-time amount.
///
/// A QC rotation of a lifted block whose size exceeds the register width is a
/// pair of offset loads; this is the residual within-register case, and
/// `_mm256_permutevar8x32_ps` performs it as one cross-lane shuffle from an
/// index vector the caller derives from the rotation amount.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn rotate_lanes_f32(x: &[f32; 8], indices: &[i32; 8], out: &mut [f32; 8]) {
    unsafe {
        let vx = _mm256_loadu_ps(x.as_ptr());
        let idx = _mm256_loadu_si256(indices.as_ptr().cast());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_permutevar8x32_ps(vx, idx));
    }
}

/// Family Q: a cyclic rotation of thirty-two `i8` lanes within 128-bit halves.
///
/// AVX2 has no cross-lane byte shuffle, so a byte rotation is a within-lane
/// `_mm256_alignr_epi8` over the register and its `_mm256_permute2x128_si256`
/// half-swap. This function is that composition for a compile-time amount,
/// which is what a lifted block of `i8` messages needs.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn rotate_bytes_i8<const K: i32>(x: &[i8; 32], out: &mut [i8; 32]) {
    unsafe {
        let vx = _mm256_loadu_si256(x.as_ptr().cast());
        let swapped = _mm256_permute2x128_si256(vx, vx, 0x01);
        _mm256_storeu_si256(
            out.as_mut_ptr().cast(),
            _mm256_alignr_epi8(swapped, vx, K),
        );
    }
}

/// Family Q: the byte rotation a lifted block of `i8` messages needs, at the
/// two amounts the probe records assembly for.
///
/// One instantiation is a within-lane amount and the other the half-register
/// amount; both are the same composition, and a generic function emits no code
/// until it is instantiated, so the feasibility record needs these.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn rotate_bytes_i8_by_three(x: &[i8; 32], out: &mut [i8; 32]) {
    unsafe { rotate_bytes_i8::<3>(x, out) }
}

/// Family Q: the half-register byte rotation.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn rotate_bytes_i8_by_sixteen(x: &[i8; 32], out: &mut [i8; 32]) {
    unsafe { rotate_bytes_i8::<16>(x, out) }
}

/// Scalar references the intrinsic forms are checked against.
pub mod reference {
    /// Saturating add over the `i8` alphabet.
    #[must_use]
    pub fn saturating_add_i8(a: i8, b: i8) -> i8 {
        a.saturating_add(b)
    }

    /// Saturating subtract over the `i8` alphabet.
    #[must_use]
    pub fn saturating_sub_i8(a: i8, b: i8) -> i8 {
        a.saturating_sub(b)
    }

    /// Magnitude over the symmetric alphabet, where `-128` does not occur.
    ///
    /// # Panics
    ///
    /// Panics on `i8::MIN`, which the symmetric alphabet excludes.
    #[must_use]
    pub fn magnitude_i8(x: i8) -> i8 {
        assert_ne!(x, i8::MIN, "the symmetric alphabet excludes i8::MIN");
        x.abs()
    }

    /// The two smallest of three magnitudes, folded in the running order.
    #[must_use]
    pub fn fold_two_minima(min1: i32, min2: i32, magnitude: i32) -> (i32, i32) {
        (min1.min(magnitude), min2.min(min1.max(magnitude)))
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    fn avx2() -> bool {
        is_x86_feature_detected!("avx2")
    }

    /// Every `i8` pair, so saturation is checked at both limits exhaustively.
    #[test]
    fn saturating_add_i8_matches_the_scalar_reference() {
        if !avx2() {
            return;
        }
        for a in i8::MIN..=i8::MAX {
            let lhs = [a; 32];
            let mut rhs = [0i8; 32];
            let mut got = [0i8; 32];
            for base in (i8::MIN as i32..=i8::MAX as i32).step_by(32) {
                for (offset, slot) in rhs.iter_mut().enumerate() {
                    *slot = (base + offset as i32).clamp(-128, 127) as i8;
                }
                unsafe { saturating_add_i8(&lhs, &rhs, &mut got) };
                for lane in 0..32 {
                    assert_eq!(got[lane], reference::saturating_add_i8(a, rhs[lane]));
                }
            }
        }
    }

    #[test]
    fn saturating_sub_i8_matches_the_scalar_reference() {
        if !avx2() {
            return;
        }
        for a in i8::MIN..=i8::MAX {
            let lhs = [a; 32];
            let mut rhs = [0i8; 32];
            let mut got = [0i8; 32];
            for base in (i8::MIN as i32..=i8::MAX as i32).step_by(32) {
                for (offset, slot) in rhs.iter_mut().enumerate() {
                    *slot = (base + offset as i32).clamp(-128, 127) as i8;
                }
                unsafe { saturating_sub_i8(&lhs, &rhs, &mut got) };
                for lane in 0..32 {
                    assert_eq!(got[lane], reference::saturating_sub_i8(a, rhs[lane]));
                }
            }
        }
    }

    #[test]
    fn saturating_add_and_sub_i16_match_the_scalar_reference() {
        if !avx2() {
            return;
        }
        let cases: [i16; 16] = [
            0, 1, -1, 127, -127, 128, -128, 255, -256, 4096, -4096, i16::MAX, i16::MIN,
            i16::MAX - 1, i16::MIN + 1, 32000,
        ];
        for &a in &cases {
            let lhs = [a; 16];
            let mut sum = [0i16; 16];
            let mut difference = [0i16; 16];
            unsafe { saturating_add_i16(&lhs, &cases, &mut sum) };
            unsafe { saturating_sub_i16(&lhs, &cases, &mut difference) };
            for lane in 0..16 {
                assert_eq!(sum[lane], a.saturating_add(cases[lane]));
                assert_eq!(difference[lane], a.saturating_sub(cases[lane]));
            }
        }
    }

    /// `_mm256_abs_epi8` is exact on the symmetric alphabet and wrong on
    /// `i8::MIN`, which is why the alphabet excludes that value.
    #[test]
    fn magnitude_i8_is_exact_on_the_symmetric_alphabet_and_wraps_at_the_minimum() {
        if !avx2() {
            return;
        }
        let mut x = [0i8; 32];
        let mut min1 = [i8::MAX; 32];
        let mut min2 = [i8::MAX; 32];
        let mut out1 = [0i8; 32];
        let mut out2 = [0i8; 32];
        for value in -127i8..=127 {
            x.fill(value);
            unsafe { fold_two_minima_i8(&min1, &min2, &x, &mut out1, &mut out2) };
            let want = reference::fold_two_minima(
                i32::from(min1[0]),
                i32::from(min2[0]),
                i32::from(reference::magnitude_i8(value)),
            );
            assert_eq!((i32::from(out1[0]), i32::from(out2[0])), want, "value {value}");
            min1 = out1;
            min2 = out2;
        }

        x.fill(i8::MIN);
        min1.fill(i8::MAX);
        min2.fill(i8::MAX);
        unsafe { fold_two_minima_i8(&min1, &min2, &x, &mut out1, &mut out2) };
        assert_eq!(out1[0], i8::MIN, "abs of i8::MIN stays negative");
    }

    #[test]
    fn fold_two_minima_i16_matches_the_scalar_reference() {
        if !avx2() {
            return;
        }
        let values: [i16; 16] = [7, -3, 0, 9, -9, 1, 1, 400, -400, 32, 8, 8, 5, -5, 12, 2];
        let mut min1 = [i16::MAX; 16];
        let mut min2 = [i16::MAX; 16];
        let mut out1 = [0i16; 16];
        let mut out2 = [0i16; 16];
        let mut want = (i32::from(i16::MAX), i32::from(i16::MAX));
        for &value in &values {
            let x = [value; 16];
            unsafe { fold_two_minima_i16(&min1, &min2, &x, &mut out1, &mut out2) };
            want = reference::fold_two_minima(want.0, want.1, i32::from(value.abs()));
            assert_eq!((i32::from(out1[0]), i32::from(out2[0])), want, "value {value}");
            min1 = out1;
            min2 = out2;
        }
    }

    #[test]
    fn sign_mask_and_apply_i8_negate_under_the_mask() {
        if !avx2() {
            return;
        }
        let mut x = [0i8; 32];
        let magnitude = [5i8; 32];
        let mut mask = [0i8; 32];
        let mut signed = [0i8; 32];
        for value in -127i8..=127 {
            x.fill(value);
            unsafe { sign_mask_and_apply_i8(&x, &magnitude, &mut mask, &mut signed) };
            let negative = value < 0;
            assert_eq!(mask[0] != 0, negative, "value {value}");
            assert_eq!(signed[0], if negative { -5 } else { 5 }, "value {value}");
        }
    }

    #[test]
    fn clip_symmetric_i8_bounds_both_sides() {
        if !avx2() {
            return;
        }
        let mut x = [0i8; 32];
        let mut out = [0i8; 32];
        for value in i8::MIN..=i8::MAX {
            x.fill(value);
            unsafe { clip_symmetric_i8(&x, 31, &mut out) };
            assert_eq!(i32::from(out[0]), i32::from(value).clamp(-31, 31), "value {value}");
        }
    }

    /// The float contract's sign rule is a comparison, so negative zero counts
    /// as positive; the IEEE sign bit disagrees on exactly that input.
    #[test]
    fn the_comparison_sign_rule_and_the_sign_bit_part_on_negative_zero() {
        if !avx2() {
            return;
        }
        let x = [-0.0f32, 0.0, 1.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -2.0];
        let mut magnitude = [0.0f32; 8];
        let mut sign_bits = [0.0f32; 8];
        let mut comparison = [0.0f32; 8];
        unsafe { magnitude_and_sign_f32(&x, &mut magnitude, &mut sign_bits) };
        unsafe { negative_by_comparison_f32(&x, &mut comparison) };

        for lane in 0..8 {
            assert_eq!(magnitude[lane].to_bits(), x[lane].abs().to_bits(), "lane {lane}");
            let by_comparison = comparison[lane].to_bits() != 0;
            assert_eq!(by_comparison, x[lane] < 0.0, "lane {lane}");
        }
        assert_ne!(sign_bits[0].to_bits(), 0, "the sign bit of -0.0 is set");
        assert!(!(x[0] < 0.0), "the comparison rule counts -0.0 as positive");
    }

    #[test]
    fn rotate_lanes_f32_is_a_cyclic_rotation() {
        if !avx2() {
            return;
        }
        let x = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        for amount in 0..8usize {
            let mut indices = [0i32; 8];
            for (lane, slot) in indices.iter_mut().enumerate() {
                *slot = ((lane + amount) % 8) as i32;
            }
            let mut out = [0.0f32; 8];
            unsafe { rotate_lanes_f32(&x, &indices, &mut out) };
            for lane in 0..8 {
                assert_eq!(out[lane], x[(lane + amount) % 8], "amount {amount} lane {lane}");
            }
        }
    }

    #[test]
    fn rotate_bytes_i8_is_a_cyclic_rotation_of_thirty_two_lanes() {
        if !avx2() {
            return;
        }
        let mut x = [0i8; 32];
        for (lane, slot) in x.iter_mut().enumerate() {
            *slot = lane as i8;
        }
        let mut out = [0i8; 32];
        unsafe { rotate_bytes_i8_by_three(&x, &mut out) };
        for lane in 0..32 {
            assert_eq!(out[lane], x[(lane + 3) % 32], "lane {lane}");
        }
        unsafe { rotate_bytes_i8_by_sixteen(&x, &mut out) };
        for lane in 0..32 {
            assert_eq!(out[lane], x[(lane + 16) % 32], "lane {lane}");
        }
    }
}
