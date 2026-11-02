//! AVX2 QC check updates; SIMD lanes are distinct lifted checks.
#![deny(unsafe_op_in_unsafe_fn)]

use crate::ldpc_lanes::LaneMinSumRule;
use crate::ldpc_qc::{check_block_scalar, QcCheckFn};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

pub(crate) fn detect() -> Option<QcCheckFn> {
    std::arch::is_x86_feature_detected!("avx2").then_some(QcCheckFn {
        check_block_fn: check_block_avx2,
    })
}

fn check_block_avx2(
    rule: LaneMinSumRule,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
) {
    // SAFETY: detect publishes this entry only on AVX2 hosts. QcCheckFn's
    // public method checks both lengths and product overflow before calling.
    unsafe { check_block_avx2_unchecked(rule, inputs, outputs, degree, z) }
}

/// # Safety
/// The host must support AVX2 and both slices must contain degree * z values.
#[target_feature(enable = "avx2")]
unsafe fn check_block_avx2_unchecked(
    rule: LaneMinSumRule,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
) {
    if degree <= 1 || degree > i32::MAX as usize {
        check_block_scalar(rule, inputs, outputs, degree, z, 0);
        return;
    }
    let groups = z / 8;
    // SAFETY: slices have degree * z elements; each access is eight f32s
    // at edge * z + lane, with edge < degree and lane + 8 <= z. Unaligned
    // accesses are permitted and the mutable output cannot alias the input.
    unsafe {
        let sign_bit = _mm256_castsi256_ps(_mm256_set1_epi32(-0x8000_0000i32));
        let zero = _mm256_setzero_ps();
        for group in 0..groups {
            let lane = group * 8;
            let mut min1 = _mm256_set1_ps(f32::INFINITY);
            let mut min2 = _mm256_set1_ps(f32::INFINITY);
            let mut arg1 = _mm256_set1_epi32(-1);
            let mut sign_product = _mm256_set1_ps(1.0);

            for edge in 0..degree {
                let value = _mm256_loadu_ps(inputs.as_ptr().add(edge * z + lane));
                let negative = _mm256_cmp_ps(value, zero, _CMP_NGE_UQ);
                sign_product = _mm256_blendv_ps(
                    sign_product,
                    _mm256_xor_ps(sign_product, sign_bit),
                    negative,
                );
                let magnitude = _mm256_andnot_ps(sign_bit, value);
                let below1 = _mm256_cmp_ps(magnitude, min1, _CMP_LT_OS);
                let below2 = _mm256_cmp_ps(magnitude, min2, _CMP_LT_OS);
                let promoted = _mm256_blendv_ps(min2, magnitude, below2);
                min2 = _mm256_blendv_ps(promoted, min1, below1);
                min1 = _mm256_blendv_ps(min1, magnitude, below1);
                arg1 = _mm256_blendv_epi8(
                    arg1,
                    _mm256_set1_epi32(edge as i32),
                    _mm256_castps_si256(below1),
                );
            }

            for edge in 0..degree {
                let value = _mm256_loadu_ps(inputs.as_ptr().add(edge * z + lane));
                let negative = _mm256_cmp_ps(value, zero, _CMP_NGE_UQ);
                let sign = _mm256_blendv_ps(
                    sign_product,
                    _mm256_xor_ps(sign_product, sign_bit),
                    negative,
                );
                let at_min =
                    _mm256_castsi256_ps(_mm256_cmpeq_epi32(arg1, _mm256_set1_epi32(edge as i32)));
                let magnitude = _mm256_blendv_ps(min1, min2, at_min);
                let out = match rule {
                    LaneMinSumRule::Plain => _mm256_mul_ps(sign, magnitude),
                    LaneMinSumRule::Normalized(alpha) => {
                        _mm256_mul_ps(_mm256_set1_ps(alpha), _mm256_mul_ps(sign, magnitude))
                    }
                    LaneMinSumRule::Offset(beta) => _mm256_mul_ps(
                        sign,
                        _mm256_max_ps(_mm256_sub_ps(magnitude, _mm256_set1_ps(beta)), zero),
                    ),
                };
                _mm256_storeu_ps(outputs.as_mut_ptr().add(edge * z + lane), out);
            }
        }
    }
    check_block_scalar(rule, inputs, outputs, degree, z, groups * 8);
}
