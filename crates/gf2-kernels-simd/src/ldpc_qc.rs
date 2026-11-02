//! Check updates over QC lifted positions, stored at `edge * z + position`.
//! The min-sum rules and edge order are those of [`crate::ldpc_lanes`].

#![forbid(unsafe_code)]

use crate::ldpc_lanes::LaneMinSumRule;

type CheckBlockFn = fn(LaneMinSumRule, &[f32], &mut [f32], usize, usize);

/// A safe QC check kernel, with a portable implementation on every target.
#[derive(Clone, Copy)]
pub struct QcCheckFn {
    pub(crate) check_block_fn: CheckBlockFn,
}

impl QcCheckFn {
    /// Scalar kernel preserving each check's edge order.
    #[must_use]
    pub const fn portable() -> Self {
        Self {
            check_block_fn: |rule, inputs, outputs, degree, z| {
                check_block_scalar(rule, inputs, outputs, degree, z, 0);
            },
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
}

/// AVX2 kernel when supported by the host; `None` on other targets or hosts.
#[must_use]
pub fn detect() -> Option<QcCheckFn> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return crate::x86::ldpc_qc::detect();
    }
    #[allow(unreachable_code)]
    None
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
