//! AVX2 QC kernels; SIMD lanes are distinct lifted checks or variables.
//!
//! The check kernel is exact against [`check_block_scalar`] without tracking
//! the argmin: a strict-comparison minimum `min1` reached by two edges forces
//! `min2 == min1`, so selecting `min2` on every edge whose magnitude equals
//! `min1` writes the scalar kernel's values, and a NaN magnitude equals
//! nothing and enters neither minimum.
#![deny(unsafe_op_in_unsafe_fn)]

use crate::ldpc_lanes::LaneMinSumRule;
use crate::ldpc_qc::{check_block_scalar, QcKernels, QcTerm};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;
use std::ops::Range;

pub(crate) fn detect() -> Option<QcKernels> {
    std::arch::is_x86_feature_detected!("avx2").then_some(QcKernels {
        check_block_fn: check_block_avx2,
        rotated_difference_fn: rotated_difference_avx2,
        accumulate_rotated_fn: accumulate_rotated_avx2,
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

fn accumulate_rotated_avx2(
    beliefs: &mut [f32],
    lanes: Range<usize>,
    channel: &[f32],
    messages: &[f32],
    terms: &[QcTerm],
) {
    // SAFETY: detect publishes this entry only on AVX2 hosts; QcKernels'
    // public method has checked z channel values, lanes within 0..z, and
    // shift < z and offset + z <= messages.len() for every term.
    unsafe { accumulate_rotated_avx2_unchecked(beliefs, lanes, channel, messages, terms) }
}

/// Eight lanes from `lane` of one term's rotated messages.
///
/// # Safety
/// The host must support AVX2, `lane + 8 <= z`, `term.shift < z` and
/// `term.offset + z <= messages.len()`.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn rotated_lanes(messages: &[f32], z: usize, term: QcTerm, lane: usize) -> __m256 {
    let first = if lane >= term.shift {
        lane - term.shift
    } else {
        lane + z - term.shift
    };
    // SAFETY: the messages of the term are offset..offset + z within the
    // slice; a contiguous load stays below first + 8 <= z, and a wrapping
    // chunk reads first + k - z < z for first + k >= z, since z >= 8.
    unsafe {
        let base = messages.as_ptr().add(term.offset);
        if first + 8 <= z {
            _mm256_loadu_ps(base.add(first))
        } else {
            let mut gathered = [0f32; 8];
            for (k, slot) in gathered.iter_mut().enumerate() {
                let index = first + k;
                *slot = *base.add(if index >= z { index - z } else { index });
            }
            _mm256_loadu_ps(gathered.as_ptr())
        }
    }
}

/// The first `count` lanes from `lane` of one term's rotated messages, the
/// lanes of `mask`; the other lanes are zero.
///
/// # Safety
/// The host must support AVX2, `0 < count < 8`, `lane + count <= z`,
/// `mask` holds exactly the first `count` lanes, `term.shift < z` and
/// `term.offset + z <= messages.len()`.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn rotated_tail(
    messages: &[f32],
    z: usize,
    term: QcTerm,
    lane: usize,
    count: usize,
    mask: __m256i,
) -> __m256 {
    let first = if lane >= term.shift {
        lane - term.shift
    } else {
        lane + z - term.shift
    };
    // SAFETY: the term's messages are offset..offset + z within the slice.
    // A masked load reads only its first count lanes, below first + count
    // <= z; a wrapping tail reads first + k - z < z for first + k >= z,
    // since first < z and k < count <= z.
    unsafe {
        let base = messages.as_ptr().add(term.offset);
        if first + count <= z {
            _mm256_maskload_ps(base.add(first), mask)
        } else {
            let mut gathered = [0f32; 8];
            for (k, slot) in gathered[..count].iter_mut().enumerate() {
                let index = first + k;
                *slot = *base.add(if index >= z { index - z } else { index });
            }
            _mm256_loadu_ps(gathered.as_ptr())
        }
    }
}

/// # Safety
/// The host must support AVX2 and the arguments must satisfy the checks of
/// [`QcKernels::accumulate_rotated`].
#[target_feature(enable = "avx2")]
unsafe fn accumulate_rotated_avx2_unchecked(
    beliefs: &mut [f32],
    lanes: Range<usize>,
    channel: &[f32],
    messages: &[f32],
    terms: &[QcTerm],
) {
    let z = beliefs.len();
    let (mut lane, end) = (lanes.start, lanes.end);
    // SAFETY: every chunk ends at lane + 8 <= end <= z, within beliefs and
    // channel, so z >= 8 inside both loops, as rotated_lanes requires; the
    // tail's masked channel load reads count lanes from lane, below end.
    unsafe {
        // Two independent chunks overlap their chains of additions.
        while lane + 16 <= end {
            let mut low = _mm256_loadu_ps(channel.as_ptr().add(lane));
            let mut high = _mm256_loadu_ps(channel.as_ptr().add(lane + 8));
            for &term in terms {
                low = _mm256_add_ps(low, rotated_lanes(messages, z, term, lane));
                high = _mm256_add_ps(high, rotated_lanes(messages, z, term, lane + 8));
            }
            _mm256_storeu_ps(beliefs.as_mut_ptr().add(lane), low);
            _mm256_storeu_ps(beliefs.as_mut_ptr().add(lane + 8), high);
            lane += 16;
        }
        if lane + 8 <= end {
            let mut sum = _mm256_loadu_ps(channel.as_ptr().add(lane));
            for &term in terms {
                sum = _mm256_add_ps(sum, rotated_lanes(messages, z, term, lane));
            }
            _mm256_storeu_ps(beliefs.as_mut_ptr().add(lane), sum);
            lane += 8;
        }
        // The last 1..7 lanes, loaded under a mask and written through a
        // stack array: a masked store is slow on Zen 3.
        let count = end - lane;
        if count > 0 {
            let mask = _mm256_cmpgt_epi32(
                _mm256_set1_epi32(count as i32),
                _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7),
            );
            let mut sum = _mm256_maskload_ps(channel.as_ptr().add(lane), mask);
            for &term in terms {
                sum = _mm256_add_ps(sum, rotated_tail(messages, z, term, lane, count, mask));
            }
            let mut out = [0f32; 8];
            _mm256_storeu_ps(out.as_mut_ptr(), sum);
            beliefs[lane..end].copy_from_slice(&out[..count]);
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

/// Rule kinds of [`check_groups`], so each rule's group loop is compiled
/// without a per-edge rule branch.
const PLAIN: u8 = 0;
const NORMALIZED: u8 = 1;
const OFFSET: u8 = 2;

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
    // SAFETY: the caller's contract is check_groups' contract.
    unsafe {
        match rule {
            LaneMinSumRule::Plain => check_groups::<PLAIN>(0.0, inputs, outputs, degree, z),
            LaneMinSumRule::Normalized(alpha) => {
                check_groups::<NORMALIZED>(alpha, inputs, outputs, degree, z);
            }
            LaneMinSumRule::Offset(beta) => {
                check_groups::<OFFSET>(beta, inputs, outputs, degree, z);
            }
        }
    }
}

/// Every group of eight lifted checks under rule `RULE` with parameter
/// `parameter`.
///
/// # Safety
/// The host must support AVX2, `degree >= 2`, and both slices must contain
/// degree * z values.
#[target_feature(enable = "avx2")]
unsafe fn check_groups<const RULE: u8>(
    parameter: f32,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
) {
    let full = z / 8;
    let tail = z % 8;
    // SAFETY: slices have degree * z elements. A full group accesses eight
    // f32s at edge * z + lane with edge < degree and lane + 8 <= z; the tail
    // group accesses only its `tail` unmasked lanes, lane + tail == z. Unaligned
    // accesses are permitted and the mutable output cannot alias the input.
    unsafe {
        let parameter = _mm256_set1_ps(parameter);
        for group in 0..full {
            check_group::<RULE>(parameter, inputs, outputs, degree, z, group * 8, None);
        }
        if tail > 0 {
            let lanes = _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7);
            let mask = _mm256_cmpgt_epi32(_mm256_set1_epi32(tail as i32), lanes);
            check_group::<RULE>(parameter, inputs, outputs, degree, z, full * 8, Some(mask));
        }
    }
}

/// Eight lifted checks from `lane`, only the lanes of `mask` when given.
///
/// The minima are never NaN, so the rule's product with a sign of ±1 is the
/// sign-bit XOR applied here, and the offset floor, never NaN either, is
/// taken once per minimum instead of once per edge.
///
/// # Safety
/// The host must support AVX2; every unmasked lane `lane + k` of every edge
/// lies within both slices.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn check_group<const RULE: u8>(
    parameter: __m256,
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
        let (low, high) = if RULE == OFFSET {
            (
                _mm256_max_ps(_mm256_sub_ps(min1, parameter), zero),
                _mm256_max_ps(_mm256_sub_ps(min2, parameter), zero),
            )
        } else {
            (min1, min2)
        };
        for edge in 0..degree {
            let value = load(edge);
            let negative = _mm256_cmp_ps(value, zero, _CMP_NGE_UQ);
            let sign = _mm256_xor_ps(signs, _mm256_and_ps(negative, sign_bit));
            let at_min = _mm256_cmp_ps(_mm256_andnot_ps(sign_bit, value), min1, _CMP_EQ_OQ);
            let signed = _mm256_xor_ps(sign, _mm256_blendv_ps(low, high, at_min));
            let out = if RULE == NORMALIZED {
                _mm256_mul_ps(parameter, signed)
            } else {
                signed
            };
            let at = outputs.as_mut_ptr().add(edge * z + lane);
            match mask {
                None => _mm256_storeu_ps(at, out),
                Some(mask) => _mm256_maskstore_ps(at, mask, out),
            }
        }
    }
}
