//! Flooding updates over QC lifted positions, stored at `edge * z + position`.
//! The min-sum rules and edge order are those of [`crate::ldpc_lanes`].

#![forbid(unsafe_code)]

use crate::ldpc_lanes::LaneMinSumRule;
use std::ops::Range;

type CheckBlockFn = fn(LaneMinSumRule, &[f32], &mut [f32], usize, usize);
type RotatedDifferenceFn = fn(&[f32], usize, &[f32], &mut [f32]);
type AddRotatedFn = fn(&mut [f32], Range<usize>, &[f32], usize);

/// Safe QC kernels, with a portable implementation on every target.
#[derive(Clone, Copy)]
pub struct QcKernels {
    pub(crate) check_block_fn: CheckBlockFn,
    pub(crate) rotated_difference_fn: RotatedDifferenceFn,
    pub(crate) add_rotated_fn: AddRotatedFn,
}

impl QcKernels {
    /// Scalar kernels; the check kernel preserves each check's edge order.
    #[must_use]
    pub const fn portable() -> Self {
        Self {
            check_block_fn: |rule, inputs, outputs, degree, z| {
                check_block_scalar(rule, inputs, outputs, degree, z, 0);
            },
            rotated_difference_fn: rotated_difference_runs,
            add_rotated_fn: add_rotated_runs,
        }
    }

    /// Writes one base row's messages, with z lifted positions per edge.
    ///
    /// Input and output index `edge * z + i` belongs to lifted check i.
    /// Degree zero writes nothing; degree one writes positive zeros.
    /// Takes O(degree * z) time without allocation. Parameter domains are
    /// those of [`LaneMinSumRule`].
    ///
    /// # Panics
    ///
    /// Panics before writing unless both lengths equal `degree * z`, or
    /// when that product overflows `usize`.
    pub fn check_block(
        &self,
        rule: LaneMinSumRule,
        inputs: &[f32],
        outputs: &mut [f32],
        degree: usize,
        z: usize,
    ) {
        let length = degree.checked_mul(z).expect("QC message count overflow");
        assert_eq!(inputs.len(), length, "QC input length");
        assert_eq!(outputs.len(), length, "QC output length");
        (self.check_block_fn)(rule, inputs, outputs, degree, z);
    }

    /// Writes `out[i] = block[(i + shift) mod z] - subtrahend[i]` for every
    /// lane `i`, with `z = block.len()`: the variable-to-check messages of a
    /// circulant from the beliefs of its column block and its check-to-variable
    /// messages. Each lane is one `f32` subtraction.
    ///
    /// # Panics
    ///
    /// Panics before writing unless `subtrahend` and `out` hold `z` values
    /// and `shift < z`.
    pub fn rotated_difference(
        &self,
        block: &[f32],
        shift: usize,
        subtrahend: &[f32],
        out: &mut [f32],
    ) {
        let z = block.len();
        assert!(shift < z, "QC shift {shift} outside lifting size {z}");
        assert_eq!(subtrahend.len(), z, "QC subtrahend length");
        assert_eq!(out.len(), z, "QC difference length");
        (self.rotated_difference_fn)(block, shift, subtrahend, out);
    }

    /// Adds `messages[(j - shift) mod z]` to `block[j]` for each lane `j` of
    /// `lanes`, with `z = block.len()`: one circulant's check-to-variable
    /// messages into the beliefs of its column block. Each lane is one `f32`
    /// addition.
    ///
    /// # Panics
    ///
    /// Panics before writing unless `messages` holds `z` values, `shift < z`
    /// and `lanes` lies within `0..z`.
    pub fn add_rotated(
        &self,
        block: &mut [f32],
        lanes: Range<usize>,
        messages: &[f32],
        shift: usize,
    ) {
        let z = block.len();
        assert!(shift < z, "QC shift {shift} outside lifting size {z}");
        assert_eq!(messages.len(), z, "QC message length");
        assert!(
            lanes.start <= lanes.end && lanes.end <= z,
            "QC lanes {lanes:?} outside lifting size {z}"
        );
        (self.add_rotated_fn)(block, lanes, messages, shift);
    }
}

/// AVX2 kernels when supported by the host; `None` on other targets or hosts.
#[must_use]
pub fn detect() -> Option<QcKernels> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return crate::x86::ldpc_qc::detect();
    }
    #[allow(unreachable_code)]
    None
}

/// [`QcKernels::rotated_difference`] as two contiguous runs.
fn rotated_difference_runs(block: &[f32], shift: usize, subtrahend: &[f32], out: &mut [f32]) {
    let split = block.len() - shift;
    let (head_out, tail_out) = out.split_at_mut(split);
    let (head_sub, tail_sub) = subtrahend.split_at(split);
    for ((slot, &value), &sub) in head_out.iter_mut().zip(&block[shift..]).zip(head_sub) {
        *slot = value - sub;
    }
    for ((slot, &value), &sub) in tail_out.iter_mut().zip(&block[..shift]).zip(tail_sub) {
        *slot = value - sub;
    }
}

/// [`QcKernels::add_rotated`] as at most two contiguous runs: lanes below
/// `shift` read messages that wrap.
fn add_rotated_runs(block: &mut [f32], lanes: Range<usize>, messages: &[f32], shift: usize) {
    let z = block.len();
    let (start, end) = (lanes.start, lanes.end);
    let split = shift.clamp(start, end);
    if start < split {
        for (slot, &value) in block[start..split]
            .iter_mut()
            .zip(&messages[start + z - shift..split + z - shift])
        {
            *slot += value;
        }
    }
    if split < end {
        for (slot, &value) in block[split..end]
            .iter_mut()
            .zip(&messages[split - shift..end - shift])
        {
            *slot += value;
        }
    }
}

pub(crate) fn check_block_scalar(
    rule: LaneMinSumRule,
    inputs: &[f32],
    outputs: &mut [f32],
    degree: usize,
    z: usize,
    start: usize,
) {
    if degree == 0 || z == 0 {
        return;
    }
    if degree == 1 {
        outputs[..z].fill(0.0);
        return;
    }
    for lane in start..z {
        let mut min1 = f32::INFINITY;
        let mut min2 = f32::INFINITY;
        let mut arg1 = usize::MAX;
        let mut sign_product = 1.0f32;
        for edge in 0..degree {
            let value = inputs[edge * z + lane];
            sign_product = if value >= 0.0 {
                sign_product
            } else {
                -sign_product
            };
            let magnitude = value.abs();
            if magnitude < min1 {
                min2 = min1;
                min1 = magnitude;
                arg1 = edge;
            } else if magnitude < min2 {
                min2 = magnitude;
            }
        }
        for edge in 0..degree {
            let value = inputs[edge * z + lane];
            let sign = if value >= 0.0 {
                sign_product
            } else {
                -sign_product
            };
            let magnitude = if edge == arg1 { min2 } else { min1 };
            outputs[edge * z + lane] = rule.apply(sign, magnitude);
        }
    }
}
