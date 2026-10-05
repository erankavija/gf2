//! AVX2 forms of the inter-frame lane kernel, at the repository MSRV.
//!
//! One lane is one frame, so each function is the canonical scalar statement
//! of `min_sum_check_row` or of the variable update applied to eight frames at
//! once. The tests assert bit identity against that scalar statement per lane,
//! including on signed zeros, NaN, infinities and ties.
//!
//! # Safety
//!
//! Every `unsafe` function requires the `avx2` target feature at the call
//! site. Loads and stores are register-width `loadu`/`storeu` on eight-element
//! arrays, so feature detection is the only obligation.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Pass one of the check update: folds one edge's magnitudes into the running
/// smallest and second-smallest magnitudes and the position of the smallest.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn fold_two_minima_and_position_f32(
    min1: &mut [f32; 8],
    min2: &mut [f32; 8],
    position: &mut [i32; 8],
    input: &[f32; 8],
    index: i32,
) {
    unsafe {
        let magnitude = _mm256_andnot_ps(_mm256_set1_ps(-0.0), _mm256_loadu_ps(input.as_ptr()));
        let m1 = _mm256_loadu_ps(min1.as_ptr());
        let m2 = _mm256_loadu_ps(min2.as_ptr());
        let below1 = _mm256_cmp_ps::<_CMP_LT_OQ>(magnitude, m1);
        let below2 = _mm256_cmp_ps::<_CMP_LT_OQ>(magnitude, m2);
        let next2 = _mm256_blendv_ps(_mm256_blendv_ps(m2, magnitude, below2), m1, below1);
        let next1 = _mm256_blendv_ps(m1, magnitude, below1);
        let held = _mm256_loadu_si256(position.as_ptr().cast());
        let next_position =
            _mm256_blendv_epi8(held, _mm256_set1_epi32(index), _mm256_castps_si256(below1));
        _mm256_storeu_ps(min1.as_mut_ptr(), next1);
        _mm256_storeu_ps(min2.as_mut_ptr(), next2);
        _mm256_storeu_si256(position.as_mut_ptr().cast(), next_position);
    }
}

/// Pass one of the check update: flips the running sign of every lane whose
/// input is not `>= 0.0`, the canonical comparison rule.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn flip_sign_by_comparison_f32(sign: &mut [f32; 8], input: &[f32; 8]) {
    unsafe {
        let value = _mm256_loadu_ps(input.as_ptr());
        let negative = _mm256_cmp_ps::<_CMP_NGE_UQ>(value, _mm256_setzero_ps());
        let flip = _mm256_and_ps(negative, _mm256_set1_ps(-0.0));
        _mm256_storeu_ps(sign.as_mut_ptr(), _mm256_xor_ps(_mm256_loadu_ps(sign.as_ptr()), flip));
    }
}

/// Pass two of the check update: the signed excluded minimum of one edge,
/// before the rule's scaling.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn excluded_minimum_f32(
    min1: &[f32; 8],
    min2: &[f32; 8],
    position: &[i32; 8],
    index: i32,
) -> __m256 {
    unsafe {
        let at_minimum = _mm256_cmpeq_epi32(
            _mm256_loadu_si256(position.as_ptr().cast()),
            _mm256_set1_epi32(index),
        );
        _mm256_blendv_ps(
            _mm256_loadu_ps(min1.as_ptr()),
            _mm256_loadu_ps(min2.as_ptr()),
            _mm256_castsi256_ps(at_minimum),
        )
    }
}

/// The sign an output carries: the shared sign with the input's own removed.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn output_sign_f32(sign: &[f32; 8], input: &[f32; 8]) -> __m256 {
    unsafe {
        let negative =
            _mm256_cmp_ps::<_CMP_NGE_UQ>(_mm256_loadu_ps(input.as_ptr()), _mm256_setzero_ps());
        let flip = _mm256_and_ps(negative, _mm256_set1_ps(-0.0));
        _mm256_and_ps(
            _mm256_xor_ps(_mm256_loadu_ps(sign.as_ptr()), flip),
            _mm256_set1_ps(-0.0),
        )
    }
}

/// Plain min-sum output of one edge.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn write_plain_f32(
    out: &mut [f32; 8],
    min1: &[f32; 8],
    min2: &[f32; 8],
    position: &[i32; 8],
    sign: &[f32; 8],
    input: &[f32; 8],
    index: i32,
) {
    unsafe {
        let magnitude = excluded_minimum_f32(min1, min2, position, index);
        let signed = _mm256_xor_ps(magnitude, output_sign_f32(sign, input));
        _mm256_storeu_ps(out.as_mut_ptr(), signed);
    }
}

/// Normalized min-sum output of one edge: `alpha * (sign * magnitude)`.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
pub unsafe fn write_normalized_f32(
    out: &mut [f32; 8],
    min1: &[f32; 8],
    min2: &[f32; 8],
    position: &[i32; 8],
    sign: &[f32; 8],
    input: &[f32; 8],
    index: i32,
    alpha: f32,
) {
    unsafe {
        let magnitude = excluded_minimum_f32(min1, min2, position, index);
        let signed = _mm256_xor_ps(magnitude, output_sign_f32(sign, input));
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_mul_ps(_mm256_set1_ps(alpha), signed));
    }
}

/// Offset min-sum output of one edge: `sign * max(magnitude - beta, 0.0)`.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
pub unsafe fn write_offset_f32(
    out: &mut [f32; 8],
    min1: &[f32; 8],
    min2: &[f32; 8],
    position: &[i32; 8],
    sign: &[f32; 8],
    input: &[f32; 8],
    index: i32,
    beta: f32,
) {
    unsafe {
        let magnitude = excluded_minimum_f32(min1, min2, position, index);
        let reduced = _mm256_max_ps(
            _mm256_sub_ps(magnitude, _mm256_set1_ps(beta)),
            _mm256_setzero_ps(),
        );
        _mm256_storeu_ps(
            out.as_mut_ptr(),
            _mm256_xor_ps(reduced, output_sign_f32(sign, input)),
        );
    }
}

/// Variable update: one sequential accumulation step of the belief.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn accumulate_belief_f32(belief: &mut [f32; 8], incoming: &[f32; 8]) {
    unsafe {
        let sum = _mm256_add_ps(
            _mm256_loadu_ps(belief.as_ptr()),
            _mm256_loadu_ps(incoming.as_ptr()),
        );
        _mm256_storeu_ps(belief.as_mut_ptr(), sum);
    }
}

/// Variable update: the outgoing message of one edge, written only in the
/// lanes `active` selects so a terminated lane keeps its terminal value.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn write_extrinsic_masked_f32(
    out: &mut [f32; 8],
    belief: &[f32; 8],
    incoming: &[f32; 8],
    active: &[i32; 8],
) {
    unsafe {
        let extrinsic = _mm256_sub_ps(
            _mm256_loadu_ps(belief.as_ptr()),
            _mm256_loadu_ps(incoming.as_ptr()),
        );
        let mask = _mm256_castsi256_ps(_mm256_loadu_si256(active.as_ptr().cast()));
        let kept = _mm256_blendv_ps(_mm256_loadu_ps(out.as_ptr()), extrinsic, mask);
        _mm256_storeu_ps(out.as_mut_ptr(), kept);
    }
}

/// Hard decisions of eight frames at one variable, one bit per lane, by the
/// canonical strict comparison.
///
/// # Safety
///
/// The caller detected `avx2` at run time.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn hard_decision_bits_f32(belief: &[f32; 8]) -> u8 {
    unsafe {
        let negative =
            _mm256_cmp_ps::<_CMP_LT_OQ>(_mm256_loadu_ps(belief.as_ptr()), _mm256_setzero_ps());
        _mm256_movemask_ps(negative) as u8
    }
}

/// The canonical scalar statements, one lane at a time.
pub mod reference {
    /// Pass one of `min_sum_check_row` for one input.
    pub fn fold(min1: &mut f32, min2: &mut f32, position: &mut i32, sign: &mut f32, value: f32, index: i32) {
        *sign = if value >= 0.0 { *sign } else { -*sign };
        let magnitude = value.abs();
        if magnitude < *min1 {
            *min2 = *min1;
            *min1 = magnitude;
            *position = index;
        } else if magnitude < *min2 {
            *min2 = magnitude;
        }
    }

    /// Pass two of `min_sum_check_row`: the sign and excluded minimum.
    pub fn excluded(min1: f32, min2: f32, position: i32, sign: f32, value: f32, index: i32) -> (f32, f32) {
        let sign = if value >= 0.0 { sign } else { -sign };
        (sign, if index == position { min2 } else { min1 })
    }

    pub fn plain(sign: f32, magnitude: f32) -> f32 {
        sign * magnitude
    }

    pub fn normalized(alpha: f32, sign: f32, magnitude: f32) -> f32 {
        alpha * (sign * magnitude)
    }

    pub fn offset(beta: f32, sign: f32, magnitude: f32) -> f32 {
        sign * (magnitude - beta).max(0.0)
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    /// Inputs that part a sign-bit rule from the comparison rule, a NaN-
    /// propagating fold from a NaN-skipping one, and a first-wins tie from a
    /// last-wins one.
    const SPECIAL: [f32; 12] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        2.5,
        -2.5,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
    ];

    fn lcg(state: &mut u64) -> u64 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *state >> 33
    }

    /// One check row per lane: `degree` inputs for each of eight frames.
    fn rows(degree: usize, seed: u64) -> Vec<[f32; 8]> {
        let mut state = seed;
        (0..degree)
            .map(|_| {
                let mut edge = [0.0f32; 8];
                for lane in &mut edge {
                    let draw = lcg(&mut state);
                    *lane = if draw % 3 == 0 {
                        SPECIAL[(draw / 3) as usize % SPECIAL.len()]
                    } else {
                        ((draw % 2001) as f32 - 1000.0) / 64.0
                    };
                }
                // A negative NaN, which the IEEE sign bit and the comparison
                // rule both count negative, beside the positive one above.
                if draw_bit(&mut state) {
                    edge[0] = -f32::NAN;
                }
                edge
            })
            .collect()
    }

    fn draw_bit(state: &mut u64) -> bool {
        lcg(state) % 7 == 0
    }

    fn avx2() -> bool {
        let found = std::arch::is_x86_feature_detected!("avx2");
        assert!(found, "this probe records AVX2 forms and needs an AVX2 host");
        found
    }

    #[test]
    fn check_row_lanes_match_the_canonical_scalar_row_bit_for_bit() {
        if !avx2() {
            return;
        }
        for degree in [2usize, 3, 6, 7, 10, 19] {
            for seed in 0..200u64 {
                let inputs = rows(degree, seed * 31 + degree as u64);
                let mut min1 = [f32::INFINITY; 8];
                let mut min2 = [f32::INFINITY; 8];
                let mut position = [-1i32; 8];
                let mut sign = [1.0f32; 8];
                for (index, edge) in inputs.iter().enumerate() {
                    unsafe {
                        fold_two_minima_and_position_f32(&mut min1, &mut min2, &mut position, edge, index as i32);
                        flip_sign_by_comparison_f32(&mut sign, edge);
                    }
                }
                for lane in 0..8 {
                    let (mut r1, mut r2, mut rp, mut rs) = (f32::INFINITY, f32::INFINITY, -1i32, 1.0f32);
                    for (index, edge) in inputs.iter().enumerate() {
                        reference::fold(&mut r1, &mut r2, &mut rp, &mut rs, edge[lane], index as i32);
                    }
                    assert_eq!(min1[lane].to_bits(), r1.to_bits(), "min1, degree {degree} seed {seed} lane {lane}");
                    assert_eq!(min2[lane].to_bits(), r2.to_bits(), "min2, degree {degree} seed {seed} lane {lane}");
                    assert_eq!(position[lane], rp, "position, degree {degree} seed {seed} lane {lane}");
                    assert_eq!(sign[lane].to_bits(), rs.to_bits(), "sign, degree {degree} seed {seed} lane {lane}");
                }
                for (index, edge) in inputs.iter().enumerate() {
                    let (mut plain, mut normalized, mut offset) = ([0.0f32; 8], [0.0f32; 8], [0.0f32; 8]);
                    unsafe {
                        write_plain_f32(&mut plain, &min1, &min2, &position, &sign, edge, index as i32);
                        write_normalized_f32(&mut normalized, &min1, &min2, &position, &sign, edge, index as i32, 0.75);
                        write_offset_f32(&mut offset, &min1, &min2, &position, &sign, edge, index as i32, 0.5);
                    }
                    for lane in 0..8 {
                        let (s, m) = reference::excluded(min1[lane], min2[lane], position[lane], sign[lane], edge[lane], index as i32);
                        assert_eq!(plain[lane].to_bits(), reference::plain(s, m).to_bits(), "plain, degree {degree} seed {seed} edge {index} lane {lane}");
                        assert_eq!(normalized[lane].to_bits(), reference::normalized(0.75, s, m).to_bits(), "normalized, degree {degree} seed {seed} edge {index} lane {lane}");
                        assert_eq!(offset[lane].to_bits(), reference::offset(0.5, s, m).to_bits(), "offset, degree {degree} seed {seed} edge {index} lane {lane}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_tie_keeps_the_first_position_and_negative_zero_counts_positive() {
        if !avx2() {
            return;
        }
        let edges = [[2.0f32; 8], [-0.0; 8], [2.0; 8], [0.0; 8]];
        let mut min1 = [f32::INFINITY; 8];
        let mut min2 = [f32::INFINITY; 8];
        let mut position = [-1i32; 8];
        let mut sign = [1.0f32; 8];
        for (index, edge) in edges.iter().enumerate() {
            unsafe {
                fold_two_minima_and_position_f32(&mut min1, &mut min2, &mut position, edge, index as i32);
                flip_sign_by_comparison_f32(&mut sign, edge);
            }
        }
        assert_eq!(position, [1; 8]);
        assert_eq!(min1.map(f32::to_bits), [0.0f32.to_bits(); 8]);
        assert_eq!(min2.map(f32::to_bits), [0.0f32.to_bits(); 8]);
        assert_eq!(sign.map(f32::to_bits), [1.0f32.to_bits(); 8]);
    }

    #[test]
    fn belief_steps_match_scalar_addition_and_a_masked_lane_is_not_written() {
        if !avx2() {
            return;
        }
        for seed in 0..500u64 {
            let pair = rows(2, seed + 9000);
            let mut belief = pair[0];
            unsafe { accumulate_belief_f32(&mut belief, &pair[1]) };
            let active = [-1i32, 0, -1, 0, -1, -1, 0, 0];
            let held = [7.25f32; 8];
            let mut out = held;
            unsafe { write_extrinsic_masked_f32(&mut out, &belief, &pair[1], &active) };
            let bits = unsafe { hard_decision_bits_f32(&belief) };
            for lane in 0..8 {
                let sum = pair[0][lane] + pair[1][lane];
                assert_eq!(belief[lane].to_bits(), sum.to_bits(), "belief, seed {seed} lane {lane}");
                let want = if active[lane] != 0 { sum - pair[1][lane] } else { held[lane] };
                assert_eq!(out[lane].to_bits(), want.to_bits(), "extrinsic, seed {seed} lane {lane}");
                assert_eq!(bits >> lane & 1 == 1, sum < 0.0, "decision, seed {seed} lane {lane}");
            }
        }
    }

    #[test]
    fn a_negative_zero_belief_decides_bit_zero() {
        if !avx2() {
            return;
        }
        let belief = [-0.0f32, 0.0, f32::NAN, -f32::NAN, -1.0, 1.0, f32::NEG_INFINITY, f32::INFINITY];
        assert_eq!(unsafe { hard_decision_bits_f32(&belief) }, 0b0101_0000);
    }
}
