//! AVX2 QC kernels; SIMD lanes are distinct lifted checks or variables.
//!
//! The check kernel is exact against [`check_block_scalar`] without tracking
//! the argmin: a strict-comparison minimum `min1` reached by two edges forces
//! `min2 == min1`, so selecting `min2` on every edge whose magnitude equals
//! `min1` writes the scalar kernel's values, and a NaN magnitude equals
//! nothing and enters neither minimum.
#![deny(unsafe_op_in_unsafe_fn)]

use crate::ldpc_lanes::LaneMinSumRule;
use crate::ldpc_qc::{check_block_scalar, QcKernels};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;
use std::ops::Range;

pub(crate) fn detect() -> Option<QcKernels> {
    std::arch::is_x86_feature_detected!("avx2").then_some(QcKernels {
        check_block_fn: check_block_avx2,
        rotated_difference_fn: rotated_difference_avx2,
        add_rotated_fn: add_rotated_avx2,
    })
}

fn check_block_avx2(
    rule: LaneMinSumRule,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
) {
    // SAFETY: detect publishes this entry only on AVX2 hosts. QcKernels'
    // public method checks both lengths and product overflow before calling.
    unsafe { check_block_avx2_unchecked(rule, inputs, outputs, degree, z) }
}

fn rotated_difference_avx2(block: &[f32], shift: usize, subtrahend: &[f32], out: &mut [f32]) {
    let split = block.len() - shift;
    let (head_out, tail_out) = out.split_at_mut(split);
    let (head_sub, tail_sub) = subtrahend.split_at(split);
    // SAFETY: detect publishes this entry only on AVX2 hosts; QcKernels'
    // public method has checked that every slice holds z values and
    // shift < z, so each pair of runs has equal lengths.
    unsafe {
        difference_avx2(&block[shift..], head_sub, head_out);
        difference_avx2(&block[..shift], tail_sub, tail_out);
    }
}

fn add_rotated_avx2(block: &mut [f32], lanes: Range<usize>, messages: &[f32], shift: usize) {
    let z = block.len();
    let (start, end) = (lanes.start, lanes.end);
    let split = shift.clamp(start, end);
    // SAFETY: detect publishes this entry only on AVX2 hosts; QcKernels'
    // public method has checked lanes within 0..z, shift < z and z messages,
    // so each run and its messages have equal lengths within their slices.
    unsafe {
        if start < split {
            add_avx2(
                &mut block[start..split],
                &messages[start + z - shift..split + z - shift],
            );
        }
        if split < end {
            add_avx2(
                &mut block[split..end],
                &messages[split - shift..end - shift],
            );
        }
    }
}

/// `out[i] = minuend[i] - subtrahend[i]`.
///
/// # Safety
/// The host must support AVX2 and the three slices must have one length.
#[target_feature(enable = "avx2")]
unsafe fn difference_avx2(minuend: &[f32], subtrahend: &[f32], out: &mut [f32]) {
    let full = out.len() / 8 * 8;
    // SAFETY: every access is eight f32s below `full`, within all three
    // slices of the caller's common length; `out` aliases neither input.
    unsafe {
        for i in (0..full).step_by(8) {
            let value = _mm256_sub_ps(
                _mm256_loadu_ps(minuend.as_ptr().add(i)),
                _mm256_loadu_ps(subtrahend.as_ptr().add(i)),
            );
            _mm256_storeu_ps(out.as_mut_ptr().add(i), value);
        }
    }
    for ((slot, &a), &b) in out[full..]
        .iter_mut()
        .zip(&minuend[full..])
        .zip(&subtrahend[full..])
    {
        *slot = a - b;
    }
}

/// `sums[i] += addend[i]`.
///
/// # Safety
/// The host must support AVX2 and both slices must have one length.
#[target_feature(enable = "avx2")]
unsafe fn add_avx2(sums: &mut [f32], addend: &[f32]) {
    let full = sums.len() / 8 * 8;
    // SAFETY: every access is eight f32s below `full`, within both slices of
    // the caller's common length; `sums` does not alias `addend`.
    unsafe {
        for i in (0..full).step_by(8) {
            let at = sums.as_mut_ptr().add(i);
            _mm256_storeu_ps(
                at,
                _mm256_add_ps(_mm256_loadu_ps(at), _mm256_loadu_ps(addend.as_ptr().add(i))),
            );
        }
    }
    for (slot, &value) in sums[full..].iter_mut().zip(&addend[full..]) {
        *slot += value;
    }
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
    if degree <= 1 || z == 0 {
        check_block_scalar(rule, inputs, outputs, degree, z, 0);
        return;
    }
    let full = z / 8;
    let tail = z % 8;
    // SAFETY: slices have degree * z elements. A full group accesses eight
    // f32s at edge * z + lane with edge < degree and lane + 8 <= z; the tail
    // group accesses only its `tail` unmasked lanes, lane + tail == z. Unaligned
    // accesses are permitted and the mutable output cannot alias the input.
    unsafe {
        for group in 0..full {
            check_group(rule, inputs, outputs, degree, z, group * 8, None);
        }
        if tail > 0 {
            let lanes = _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7);
            let mask = _mm256_cmpgt_epi32(_mm256_set1_epi32(tail as i32), lanes);
            check_group(rule, inputs, outputs, degree, z, full * 8, Some(mask));
        }
    }
}

/// Eight lifted checks from `lane`, only the lanes of `mask` when given.
///
/// # Safety
/// The host must support AVX2; every unmasked lane `lane + k` of every edge
/// lies within both slices.
#[target_feature(enable = "avx2")]
unsafe fn check_group(
    rule: LaneMinSumRule,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
    lane: usize,
    mask: Option<__m256i>,
) {
    // SAFETY: the caller bounds every unmasked access.
    unsafe {
        let load = |edge: usize| {
            let at = inputs.as_ptr().add(edge * z + lane);
            match mask {
                None => _mm256_loadu_ps(at),
                Some(mask) => _mm256_maskload_ps(at, mask),
            }
        };
        let sign_bit = _mm256_castsi256_ps(_mm256_set1_epi32(-0x8000_0000i32));
        let zero = _mm256_setzero_ps();
        let mut min1 = _mm256_set1_ps(f32::INFINITY);
        let mut min2 = min1;
        let mut signs = zero;
        for edge in 0..degree {
            let value = load(edge);
            // `>= 0.0` is false for a NaN, which counts as negative.
            let negative = _mm256_cmp_ps(value, zero, _CMP_NGE_UQ);
            signs = _mm256_xor_ps(signs, _mm256_and_ps(negative, sign_bit));
            let magnitude = _mm256_andnot_ps(sign_bit, value);
            // max_ps returns its second operand for a NaN, so a NaN magnitude
            // leaves min2 unchanged; min_ps(m, min1) is `m < min1 ? m : min1`.
            min2 = _mm256_min_ps(_mm256_max_ps(min1, magnitude), min2);
            min1 = _mm256_min_ps(magnitude, min1);
        }
        let product = _mm256_or_ps(_mm256_set1_ps(1.0), signs);
        for edge in 0..degree {
            let value = load(edge);
            let negative = _mm256_cmp_ps(value, zero, _CMP_NGE_UQ);
            let sign = _mm256_xor_ps(product, _mm256_and_ps(negative, sign_bit));
            let at_min = _mm256_cmp_ps(_mm256_andnot_ps(sign_bit, value), min1, _CMP_EQ_OQ);
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
            let at = outputs.as_mut_ptr().add(edge * z + lane);
            match mask {
                None => _mm256_storeu_ps(at, out),
                Some(mask) => _mm256_maskstore_ps(at, mask, out),
            }
        }
    }
}
