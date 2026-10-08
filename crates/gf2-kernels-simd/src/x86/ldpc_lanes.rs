//! AVX2 backend of the lane kernels of [`crate::ldpc_lanes`]: one edge of
//! eight frames is one aligned 256-bit register, and every lane follows the
//! portable bundle bit for bit. The canonical sign is the comparison
//! `>= 0.0` and the two-minimum fold a strict comparison, each kept as a
//! compare and a blend. [`detect`] publishes the safe entries.

#![deny(unsafe_op_in_unsafe_fn)]

use core::arch::x86_64::*;
use std::arch::is_x86_feature_detected;

use crate::ldpc_lanes::{
    assert_check_step_lengths, assert_variable_step_lengths, F32x8, LaneMask, LaneMinSumRule,
    LdpcLaneFns,
};

/// The AVX2 bundle, or `None` on a host without AVX2.
pub(crate) fn detect() -> Option<LdpcLaneFns> {
    is_x86_feature_detected!("avx2").then_some(LdpcLaneFns {
        check_step_fn: lane_check_step_avx2,
        variable_step_fn: lane_variable_step_avx2,
    })
}

/// AVX2 [`crate::ldpc_lanes::LaneCheckStepFn`].
fn lane_check_step_avx2(
    rule: LaneMinSumRule,
    check_offsets: &[u32],
    var_to_check: &[F32x8],
    check_to_var: &mut [F32x8],
) {
    assert_check_step_lengths(
        "lane_check_step_avx2",
        check_offsets,
        var_to_check.len(),
        check_to_var.len(),
    );
    // SAFETY: `detect` publishes this entry only on a host with AVX2, and the
    // assertion leaves nondecreasing offsets that end at the length of both
    // message arrays.
    unsafe { lane_check_step_avx2_unchecked(rule, check_offsets, var_to_check, check_to_var) }
}

/// AVX2 [`crate::ldpc_lanes::LaneVariableStepFn`].
#[allow(clippy::too_many_arguments)]
fn lane_variable_step_avx2(
    active: LaneMask,
    var_offsets: &[u32],
    var_edge_to_check_edge: &[u32],
    channel: &[F32x8],
    check_to_var: &[F32x8],
    beliefs: &mut [F32x8],
    var_to_check: &mut [F32x8],
    hard: &mut [u8],
) {
    assert_variable_step_lengths(
        "lane_variable_step_avx2",
        var_offsets,
        var_edge_to_check_edge.len(),
        channel.len(),
        check_to_var.len(),
        beliefs.len(),
        var_to_check.len(),
        hard.len(),
    );
    // SAFETY: `detect` publishes this entry only on a host with AVX2, and the
    // assertion leaves nondecreasing offsets that end at the length of the
    // slot map and two message arrays of one length.
    let slots_in_range = unsafe {
        lane_variable_step_avx2_unchecked(
            active,
            var_offsets,
            var_edge_to_check_edge,
            channel,
            check_to_var,
            beliefs,
            var_to_check,
            hard,
        )
    };
    assert!(
        slots_in_range,
        "lane_variable_step_avx2: a slot names an edge at or beyond the edge count"
    );
}

#[inline]
#[target_feature(enable = "avx2")]
fn load(lanes: &F32x8) -> __m256 {
    // SAFETY: an `F32x8` is 32 readable bytes at 32-byte alignment.
    unsafe { _mm256_load_ps(lanes.0.as_ptr()) }
}

#[inline]
#[target_feature(enable = "avx2")]
fn store(lanes: &mut F32x8, value: __m256) {
    // SAFETY: an `F32x8` is 32 writable bytes at 32-byte alignment.
    unsafe { _mm256_store_ps(lanes.0.as_mut_ptr(), value) }
}

/// The sign bit in each lane whose value is not `>= 0.0`.
#[inline]
#[target_feature(enable = "avx2")]
fn negative_sign(value: __m256) -> __m256 {
    _mm256_and_ps(
        _mm256_cmp_ps::<_CMP_NGE_UQ>(value, _mm256_setzero_ps()),
        _mm256_set1_ps(-0.0),
    )
}

/// The check step without its argument check.
///
/// # Safety
///
/// The host has AVX2. `check_offsets` never decreases, and its last entry is
/// at most the length of each message array.
#[target_feature(enable = "avx2")]
unsafe fn lane_check_step_avx2_unchecked(
    rule: LaneMinSumRule,
    check_offsets: &[u32],
    var_to_check: &[F32x8],
    check_to_var: &mut [F32x8],
) {
    for bounds in check_offsets.windows(2) {
        let range = bounds[0] as usize..bounds[1] as usize;
        // SAFETY: the offsets never decrease and stay within both arrays.
        let (inputs, outputs) = unsafe {
            (
                var_to_check.get_unchecked(range.clone()),
                check_to_var.get_unchecked_mut(range),
            )
        };
        match rule {
            LaneMinSumRule::Plain => {
                check_row(inputs, outputs, |magnitude, sign| {
                    _mm256_xor_ps(magnitude, sign)
                });
            }
            LaneMinSumRule::Normalized(alpha) => {
                let alpha = _mm256_set1_ps(alpha);
                check_row(inputs, outputs, |magnitude, sign| {
                    _mm256_mul_ps(alpha, _mm256_xor_ps(magnitude, sign))
                });
            }
            LaneMinSumRule::Offset(beta) => {
                let beta = _mm256_set1_ps(beta);
                // `_mm256_max_ps` returns its second operand on a NaN first
                // operand, as `f32::max(NaN, 0.0)` returns zero.
                check_row(inputs, outputs, |magnitude, sign| {
                    let reduced =
                        _mm256_max_ps(_mm256_sub_ps(magnitude, beta), _mm256_setzero_ps());
                    _mm256_xor_ps(reduced, sign)
                });
            }
        }
    }
}

/// One check of equally many `inputs` and `outputs`. `finish` maps the
/// excluded minimum, which is never NaN and never negative, and the sign bit
/// of the output to the output.
#[inline]
#[target_feature(enable = "avx2")]
fn check_row(inputs: &[F32x8], outputs: &mut [F32x8], finish: impl Fn(__m256, __m256) -> __m256) {
    if inputs.len() < 2 {
        if let Some(output) = outputs.first_mut() {
            *output = F32x8::ZERO;
        }
        return;
    }

    let sign_bit = _mm256_set1_ps(-0.0);
    let mut min1 = _mm256_set1_ps(f32::INFINITY);
    let mut min2 = min1;
    let mut sign_product = _mm256_setzero_ps();
    for input in inputs {
        let value = load(input);
        sign_product = _mm256_xor_ps(sign_product, negative_sign(value));
        let magnitude = _mm256_andnot_ps(sign_bit, value);
        // Ordered comparisons are false on a NaN magnitude, which so enters
        // neither minimum.
        let below1 = _mm256_cmp_ps::<_CMP_LT_OQ>(magnitude, min1);
        let below2 = _mm256_cmp_ps::<_CMP_LT_OQ>(magnitude, min2);
        min2 = _mm256_blendv_ps(_mm256_blendv_ps(min2, magnitude, below2), min1, below1);
        min1 = _mm256_blendv_ps(min1, magnitude, below1);
    }

    for (input, output) in inputs.iter().zip(outputs) {
        let value = load(input);
        let sign = _mm256_xor_ps(sign_product, negative_sign(value));
        // An input at the smallest magnitude takes the second smallest. Past
        // the first such input the two are equal, so no position is kept.
        let at_minimum = _mm256_cmp_ps::<_CMP_EQ_OQ>(_mm256_andnot_ps(sign_bit, value), min1);
        let magnitude = _mm256_blendv_ps(min1, min2, at_minimum);
        store(output, finish(magnitude, sign));
    }
}

/// The variable step without its length checks. Returns `false`, with
/// earlier variables written, at a slot that names an edge at or beyond the
/// edge count.
///
/// # Safety
///
/// The host has AVX2. `var_offsets` never decreases, and its last entry is at
/// most the length of `var_edge_to_check_edge`. `check_to_var` and
/// `var_to_check` have one length.
#[allow(clippy::too_many_arguments)]
#[target_feature(enable = "avx2")]
unsafe fn lane_variable_step_avx2_unchecked(
    active: LaneMask,
    var_offsets: &[u32],
    var_edge_to_check_edge: &[u32],
    channel: &[F32x8],
    check_to_var: &[F32x8],
    beliefs: &mut [F32x8],
    var_to_check: &mut [F32x8],
    hard: &mut [u8],
) -> bool {
    let active = active.bits();
    let lane_bit = _mm256_setr_epi32(1, 2, 4, 8, 16, 32, 64, 128);
    let selected = _mm256_and_si256(_mm256_set1_epi32(i32::from(active)), lane_bit);
    let mask = _mm256_castsi256_ps(_mm256_cmpeq_epi32(selected, lane_bit));

    let variables = channel.iter().zip(beliefs).zip(hard);
    for (bounds, ((channel, posterior), hard)) in var_offsets.windows(2).zip(variables) {
        // SAFETY: the offsets never decrease and stay within the slot map.
        let slots =
            unsafe { var_edge_to_check_edge.get_unchecked(bounds[0] as usize..bounds[1] as usize) };

        let mut belief = load(channel);
        for &edge in slots {
            let Some(incoming) = check_to_var.get(edge as usize) else {
                return false;
            };
            belief = _mm256_add_ps(belief, load(incoming));
        }

        let negative = _mm256_cmp_ps::<_CMP_LT_OQ>(belief, _mm256_setzero_ps());
        let decided = _mm256_movemask_ps(negative) as u8;
        *hard = *hard & !active | decided & active;
        store(posterior, _mm256_blendv_ps(load(posterior), belief, mask));

        for &edge in slots {
            // SAFETY: the loop above found `edge` inside `check_to_var`,
            // whose length `var_to_check` shares.
            let (incoming, outgoing) = unsafe {
                (
                    check_to_var.get_unchecked(edge as usize),
                    var_to_check.get_unchecked_mut(edge as usize),
                )
            };
            let extrinsic = _mm256_sub_ps(belief, load(incoming));
            store(outgoing, _mm256_blendv_ps(load(outgoing), extrinsic, mask));
        }
    }
    true
}
