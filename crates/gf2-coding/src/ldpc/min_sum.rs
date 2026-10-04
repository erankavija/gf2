//! Shared minimum, second-minimum and sign reduction for check-node updates.

use crate::llr::Llr;

/// A check-node update rule of the min-sum family.
///
/// The three variants share the reduction and differ only in how the excluded
/// minimum magnitude is scaled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinSumRule {
    /// Plain min-sum: the excluded minimum, signed.
    Plain,
    /// Normalized min-sum: the signed excluded minimum scaled by `alpha`.
    Normalized(f32),
    /// Offset min-sum: the excluded minimum reduced by `beta`, floored at zero,
    /// then signed.
    Offset(f32),
}

impl MinSumRule {
    /// Applies the rule to one output's sign and excluded minimum magnitude.
    #[inline]
    #[must_use]
    fn apply(self, sign: f32, magnitude: f32) -> Llr {
        match self {
            Self::Plain => Llr::new(sign * magnitude),
            Self::Normalized(alpha) => Llr::new(alpha * (sign * magnitude)),
            Self::Offset(beta) => Llr::new(sign * (magnitude - beta).max(0.0)),
        }
    }
}

/// Writes every outgoing message of one check node from one shared reduction.
///
/// `inputs[i]` is the incoming message on the check's `i`-th edge and
/// `outputs[i]` receives the outgoing message on that same edge, computed from
/// every input except `inputs[i]`. The two slices may not overlap; a caller
/// that keeps one message array per direction passes the check's run of each.
///
/// A check of degree one has no other input for its single output, and takes
/// the zero LLR that the degree-zero reduction of an iterative decoder means.
/// A check of degree zero writes nothing.
///
/// Each input is read exactly twice, so the work is linear in the check degree.
///
/// # Panics
///
/// Panics if `inputs` and `outputs` have different lengths.
///
/// # Numerical contract
///
/// The reduction reproduces, bit for bit, the scalar reference for the same
/// excluded input set: [`Llr::boxplus_minsum_n`] as it is implemented
/// without the `simd` cargo feature, and the corresponding scalar
/// [`Llr::boxplus_normalized_minsum_n`] and [`Llr::boxplus_offset_minsum_n`].
/// Two rules of that reference decide the cases a reduction can disagree on:
///
/// - **Sign** is taken by comparison against zero, so a negative-zero input
///   counts as positive and a NaN input counts as negative.
/// - **Magnitude** is the [`f32::min`] fold of the input magnitudes from
///   `f32::INFINITY`, which skips a NaN input; an all-NaN input set therefore
///   reduces to an infinite magnitude.
///
/// This function is scalar and performs no runtime dispatch. The AVX2 kernel
/// that `Llr::boxplus_minsum_n` reaches under `simd` takes the IEEE sign bit
/// in its vector lanes and so disagrees with this contract.
///
/// Both factors of a leave-one-out result are exact. A sign is `±1.0`, so
/// removing one input's sign from the shared product is a multiplication by
/// that same `±1.0`. The magnitude excluding input `i` is the smallest input
/// magnitude when `i` is not at the smallest position and the second smallest
/// when it is, which is the minimum over the other inputs even when the two
/// smallest magnitudes are equal.
///
/// [`Llr::boxplus_minsum_n`]: crate::llr::Llr::boxplus_minsum_n
/// [`Llr::boxplus_normalized_minsum_n`]: crate::llr::Llr::boxplus_normalized_minsum_n
/// [`Llr::boxplus_offset_minsum_n`]: crate::llr::Llr::boxplus_offset_minsum_n
#[inline]
pub fn min_sum_check_row(rule: MinSumRule, inputs: &[Llr], outputs: &mut [Llr]) {
    assert_eq!(
        inputs.len(),
        outputs.len(),
        "a check's outputs and inputs sit on the same edges"
    );
    match inputs.len() {
        0 => return,
        1 => {
            outputs[0] = Llr::zero();
            return;
        }
        _ => {}
    }

    // Pass one: the two smallest magnitudes, the position of the smallest, and
    // the product of every input's sign.
    let mut min1 = f32::INFINITY;
    let mut min2 = f32::INFINITY;
    let mut arg1 = usize::MAX;
    let mut sign_product = 1.0f32;
    for (index, input) in inputs.iter().enumerate() {
        let value = input.value();
        sign_product = if value >= 0.0 {
            sign_product
        } else {
            -sign_product
        };
        let magnitude = value.abs();
        if magnitude < min1 {
            min2 = min1;
            min1 = magnitude;
            arg1 = index;
        } else if magnitude < min2 {
            min2 = magnitude;
        }
    }

    // Pass two: each output takes the minimum that excludes its own input, and
    // the shared sign product with its own sign removed.
    for (index, (input, output)) in inputs.iter().zip(outputs.iter_mut()).enumerate() {
        let sign = if input.value() >= 0.0 {
            sign_product
        } else {
            -sign_product
        };
        let magnitude = if index == arg1 { min2 } else { min1 };
        *output = rule.apply(sign, magnitude);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scalar reference for one output: the leave-one-out reduction the
    /// per-edge path performs, built from the crate's scalar `Llr` operations.
    fn reference(rule: MinSumRule, inputs: &[Llr], excluded: usize) -> Llr {
        let others: Vec<Llr> = inputs
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != excluded)
            .map(|(_, llr)| *llr)
            .collect();
        if others.is_empty() {
            return Llr::zero();
        }
        let sign_product: f32 = others
            .iter()
            .map(|llr| if llr.value() >= 0.0 { 1.0 } else { -1.0 })
            .product();
        let min_magnitude = others
            .iter()
            .map(|llr| llr.value().abs())
            .fold(f32::INFINITY, f32::min);
        rule.apply(sign_product, min_magnitude)
    }

    fn assert_matches_reference(rule: MinSumRule, values: &[f32]) {
        let inputs: Vec<Llr> = values.iter().copied().map(Llr::new).collect();
        let mut outputs = vec![Llr::zero(); inputs.len()];
        min_sum_check_row(rule, &inputs, &mut outputs);
        for (index, got) in outputs.iter().copied().enumerate() {
            let want = reference(rule, &inputs, index);
            assert_eq!(
                got.value().to_bits(),
                want.value().to_bits(),
                "rule {rule:?} output {index} of {values:?}: got {got:?}, want {want:?}"
            );
        }
    }

    const RULES: [MinSumRule; 3] = [
        MinSumRule::Plain,
        MinSumRule::Normalized(0.75),
        MinSumRule::Offset(0.5),
    ];

    #[test]
    fn matches_the_reference_on_ordinary_inputs() {
        for rule in RULES {
            assert_matches_reference(rule, &[3.0, -2.0, 4.0]);
            assert_matches_reference(rule, &[-1.5, -2.5]);
            assert_matches_reference(rule, &[0.25, 8.0, -0.5, 7.0, 6.0, -9.0, 1.0, 2.0, 3.0]);
        }
    }

    #[test]
    fn matches_the_reference_on_ties() {
        for rule in RULES {
            assert_matches_reference(rule, &[2.0, 2.0, 5.0]);
            assert_matches_reference(rule, &[-2.0, 2.0, 5.0]);
            assert_matches_reference(rule, &[3.0, 3.0, 3.0, 3.0]);
        }
    }

    #[test]
    fn matches_the_reference_on_signed_zero() {
        for rule in RULES {
            assert_matches_reference(rule, &[-0.0, 1.0, 2.0]);
            assert_matches_reference(rule, &[0.0, -0.0, 2.0]);
            assert_matches_reference(rule, &[-0.0, -0.0, -3.0]);
            // Nine inputs: the length at which the AVX2 kernel's vector lanes
            // would engage, and where the reduction still follows the scalar rule.
            assert_matches_reference(rule, &[-0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        }
    }

    #[test]
    fn matches_the_reference_on_extrema_and_non_finite() {
        for rule in RULES {
            assert_matches_reference(rule, &[f32::MAX, f32::MIN, 1.0]);
            assert_matches_reference(rule, &[f32::MIN_POSITIVE, -f32::MIN_POSITIVE, 1.0]);
            assert_matches_reference(rule, &[f32::INFINITY, 2.0, -1.0]);
            assert_matches_reference(rule, &[f32::NEG_INFINITY, f32::INFINITY]);
            assert_matches_reference(rule, &[f32::NAN, 2.0, -1.0]);
            assert_matches_reference(rule, &[f32::NAN, f32::NAN]);
            assert_matches_reference(rule, &[f32::NAN, f32::NAN, 4.0, -5.0]);
        }
    }

    #[test]
    fn degree_one_and_zero() {
        for rule in RULES {
            let mut outputs = [Llr::new(7.0)];
            min_sum_check_row(rule, &[Llr::new(-3.0)], &mut outputs);
            assert_eq!(outputs[0].value().to_bits(), 0.0f32.to_bits());

            min_sum_check_row(rule, &[], &mut []);
        }
    }

    #[test]
    #[should_panic(expected = "a check's outputs and inputs sit on the same edges")]
    fn mismatched_lengths_panic() {
        let mut outputs = [Llr::zero(); 2];
        min_sum_check_row(MinSumRule::Plain, &[Llr::new(1.0)], &mut outputs);
    }
}
