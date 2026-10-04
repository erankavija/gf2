//! Log-likelihood ratio type and box-plus operations for soft-decision
//! decoding, with `LLR = ln(P(b=0|r) / P(b=1|r))`.

#![allow(dead_code)]

/// Log-likelihood ratio as `f32`: positive favours bit 0, negative bit 1,
/// and the magnitude is the confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Llr(f32);

/// Deterministic index orders for a slice of reliability magnitudes.
///
/// Both orders contain every input index exactly once. [`Self::ascending`]
/// orders smaller magnitudes first, while [`Self::descending`] orders larger
/// magnitudes first. Equal numeric magnitudes, including `-0.0` and `0.0`,
/// are ordered by their original index in both directions.
///
/// # Examples
///
/// ```
/// use gf2_coding::llr::ReliabilityPermutation;
///
/// let order = ReliabilityPermutation::from_magnitudes(&[2.0, 1.0, 2.0]);
/// assert_eq!(order.ascending(), &[1, 0, 2]);
/// assert_eq!(order.descending(), &[0, 2, 1]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReliabilityPermutation {
    ascending: Vec<usize>,
    descending: Vec<usize>,
}

impl ReliabilityPermutation {
    /// Builds both index orders; infinite values take their natural `f32`
    /// order.
    ///
    /// # Panics
    ///
    /// Panics if any magnitude is NaN.
    ///
    /// # Complexity
    ///
    /// O(n log n) time and O(n) additional space for `n` magnitudes.
    pub fn from_magnitudes(magnitudes: &[f32]) -> Self {
        assert!(
            !magnitudes.iter().any(|magnitude| magnitude.is_nan()),
            "reliability magnitude cannot be NaN"
        );

        let mut ascending: Vec<usize> = (0..magnitudes.len()).collect();
        ascending.sort_by(|&left, &right| {
            magnitudes[left]
                .partial_cmp(&magnitudes[right])
                .expect("NaN magnitudes were rejected above")
                .then(left.cmp(&right))
        });

        let mut descending: Vec<usize> = (0..magnitudes.len()).collect();
        descending.sort_by(|&left, &right| {
            magnitudes[right]
                .partial_cmp(&magnitudes[left])
                .expect("NaN magnitudes were rejected above")
                .then(left.cmp(&right))
        });

        Self {
            ascending,
            descending,
        }
    }

    /// Indices from least reliable to most reliable.
    pub fn ascending(&self) -> &[usize] {
        &self.ascending
    }

    /// Indices from most reliable to least reliable.
    pub fn descending(&self) -> &[usize] {
        &self.descending
    }
}

impl Llr {
    /// Creates a new LLR from a raw f32 value.
    pub fn new(value: f32) -> Self {
        Llr(value)
    }

    /// Returns the raw LLR value.
    pub fn value(self) -> f32 {
        self.0
    }

    /// Makes a hard decision: returns `false` (bit 0) if LLR >= 0, `true` (bit 1) if LLR < 0.
    pub fn hard_decision(self) -> bool {
        self.0 < 0.0
    }

    /// Returns the magnitude (absolute value) of the LLR, representing confidence.
    pub fn magnitude(self) -> f32 {
        self.0.abs()
    }

    /// Reliability index orders of `llrs` by magnitude, under the contract of
    /// [`ReliabilityPermutation::from_magnitudes`].
    ///
    /// # Panics
    ///
    /// Panics if any LLR has a NaN magnitude.
    ///
    /// # Complexity
    ///
    /// O(n log n) time and O(n) additional space for `n` LLRs.
    pub fn reliability_permutation(llrs: &[Llr]) -> ReliabilityPermutation {
        let magnitudes: Vec<f32> = llrs.iter().map(|llr| llr.magnitude()).collect();
        ReliabilityPermutation::from_magnitudes(&magnitudes)
    }

    /// Creates an LLR representing infinite confidence in bit 0.
    pub fn infinity() -> Self {
        Llr(f32::INFINITY)
    }

    /// Creates an LLR representing infinite confidence in bit 1.
    pub fn neg_infinity() -> Self {
        Llr(f32::NEG_INFINITY)
    }

    /// Creates an LLR representing complete uncertainty (equal probability).
    pub fn zero() -> Self {
        Llr(0.0)
    }

    /// Clamp on the tanh product before `atanh`. In `f32`, `tanh` saturates
    /// to exactly `1.0` and `atanh(1.0) = Inf`, which yields `Inf - Inf = NaN`
    /// at a variable-node update.
    const MAX_TANH_PRODUCT: f32 = 1.0 - f32::EPSILON;

    /// Saturates the LLR to the range `[-max, max]`.
    ///
    /// # Panics
    ///
    /// Panics if `max` is negative or NaN.
    pub fn saturate(self, max: f32) -> Self {
        Llr(self.0.clamp(-max, max))
    }

    /// Box-plus: LLR of the XOR of two independent bits, with the tanh
    /// product clamped to `±(1 - f32::EPSILON)`.
    ///
    /// ```text
    /// LLR(a XOR b) = 2 * atanh(tanh(L_a/2) * tanh(L_b/2))
    /// ```
    pub fn boxplus(self, other: Llr) -> Llr {
        let a = self.0 / 2.0;
        let b = other.0 / 2.0;
        let product = (a.tanh() * b.tanh()).clamp(-Self::MAX_TANH_PRODUCT, Self::MAX_TANH_PRODUCT);
        Llr(2.0 * product.atanh())
    }

    /// Min-sum approximation of box-plus.
    ///
    /// ```text
    /// sign(L_a) * sign(L_b) * min(|L_a|, |L_b|)
    /// ```
    pub fn boxplus_minsum(self, other: Llr) -> Llr {
        let sign = if (self.0 >= 0.0) == (other.0 >= 0.0) {
            1.0
        } else {
            -1.0
        };
        Llr(sign * self.0.abs().min(other.0.abs()))
    }

    /// LLR of the XOR of several independent bits, with the tanh product
    /// clamped as in [`Llr::boxplus`]:
    ///
    /// $$
    /// \text{LLR}(b_1 \oplus b_2 \oplus \cdots \oplus b_n) = 2 \cdot \text{atanh}\left(\prod_{i=1}^{n} \tanh\left(\frac{L_i}{2}\right)\right)
    /// $$
    ///
    /// where $L_i$ is the LLR of bit $b_i$.
    ///
    /// # Panics
    ///
    /// Panics if `llrs` is empty.
    pub fn boxplus_n(llrs: &[Llr]) -> Llr {
        assert!(!llrs.is_empty(), "Cannot compute boxplus_n of empty slice");

        let product: f32 = llrs.iter().map(|llr| (llr.0 / 2.0).tanh()).product();
        let clamped = product.clamp(-Self::MAX_TANH_PRODUCT, Self::MAX_TANH_PRODUCT);
        Llr(2.0 * clamped.atanh())
    }

    /// Multi-operand min-sum approximation of box-plus:
    ///
    /// $$
    /// \text{LLR}(b_1 \oplus \cdots \oplus b_n) \approx \left(\prod_{i=1}^{n} \text{sign}(L_i)\right) \cdot \min_{i=1}^{n} |L_i|
    /// $$
    ///
    /// # Panics
    ///
    /// Panics if `llrs` is empty.
    pub fn boxplus_minsum_n(llrs: &[Llr]) -> Llr {
        assert!(
            !llrs.is_empty(),
            "Cannot compute boxplus_minsum_n of empty slice"
        );

        #[cfg(feature = "simd")]
        {
            use once_cell::sync::Lazy;
            static SIMD_FNS: Lazy<Option<gf2_kernels_simd::llr::LlrFns>> =
                Lazy::new(gf2_kernels_simd::llr::detect);

            if let Some(ref fns) = *SIMD_FNS {
                let f32_vals: Vec<f32> = llrs.iter().map(|l| l.0).collect();
                let result = (fns.minsum_fn)(&f32_vals);
                return Llr(result);
            }
        }

        Self::boxplus_minsum_n_scalar(llrs)
    }

    fn boxplus_minsum_n_scalar(llrs: &[Llr]) -> Llr {
        let sign_product: f32 = llrs
            .iter()
            .map(|llr| if llr.0 >= 0.0 { 1.0 } else { -1.0 })
            .product();

        let min_magnitude = llrs
            .iter()
            .map(|llr| llr.0.abs())
            .fold(f32::INFINITY, f32::min);

        Llr(sign_product * min_magnitude)
    }

    /// Saturate a batch of LLRs to the range `[-max, max]`.
    ///
    /// # Panics
    ///
    /// Panics if `llrs` is non-empty and `max` is negative or NaN.
    pub fn saturate_batch(llrs: &[Llr], max: f32) -> Vec<Llr> {
        llrs.iter().map(|llr| llr.saturate(max)).collect()
    }

    /// Applies [`Llr::hard_decision`] to each LLR.
    pub fn hard_decision_batch(llrs: &[Llr]) -> Vec<bool> {
        llrs.iter().map(|llr| llr.hard_decision()).collect()
    }

    /// Normalized min-sum: the min-sum result scaled by $\alpha$.
    ///
    /// $$
    /// \text{LLR} \approx \alpha \cdot \left(\prod_{i=1}^{n} \text{sign}(L_i)\right) \cdot \min_{i=1}^{n} |L_i|
    /// $$
    ///
    /// # Panics
    ///
    /// Panics if `llrs` is empty.
    pub fn boxplus_normalized_minsum_n(llrs: &[Llr], alpha: f32) -> Llr {
        let minsum = Self::boxplus_minsum_n(llrs);
        Llr(alpha * minsum.0)
    }

    /// Offset min-sum: the min-sum magnitude reduced by $\beta$ and floored
    /// at zero.
    ///
    /// $$
    /// \text{LLR} \approx \left(\prod_{i=1}^{n} \text{sign}(L_i)\right) \cdot \max\left(0, \min_{i=1}^{n} |L_i| - \beta\right)
    /// $$
    ///
    /// # Panics
    ///
    /// Panics if `llrs` is empty.
    pub fn boxplus_offset_minsum_n(llrs: &[Llr], beta: f32) -> Llr {
        assert!(
            !llrs.is_empty(),
            "Cannot compute boxplus_offset_minsum_n of empty slice"
        );

        let sign_product: f32 = llrs
            .iter()
            .map(|llr| if llr.0 >= 0.0 { 1.0 } else { -1.0 })
            .product();

        let min_magnitude = llrs
            .iter()
            .map(|llr| llr.0.abs())
            .fold(f32::INFINITY, f32::min);
        let offset_magnitude = (min_magnitude - beta).max(0.0);

        Llr(sign_product * offset_magnitude)
    }

    /// Checks if the LLR value is finite (not NaN or infinity).
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }

    /// [`Llr::boxplus`] that returns [`Llr::zero`] when the result is not
    /// finite.
    pub fn safe_boxplus(self, other: Llr) -> Llr {
        let result = self.boxplus(other);
        if result.is_finite() {
            result
        } else {
            Llr::zero()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_llr() {
        let llr = Llr::new(3.5);
        assert_eq!(llr.value(), 3.5);
    }

    #[test]
    fn test_hard_decision_positive() {
        let llr = Llr::new(3.5);
        assert!(!llr.hard_decision());
    }

    #[test]
    fn test_hard_decision_negative() {
        let llr = Llr::new(-2.0);
        assert!(llr.hard_decision());
    }

    #[test]
    fn test_hard_decision_zero() {
        let llr = Llr::new(0.0);
        assert!(!llr.hard_decision());
    }

    #[test]
    fn test_magnitude() {
        assert_eq!(Llr::new(3.5).magnitude(), 3.5);
        assert_eq!(Llr::new(-2.0).magnitude(), 2.0);
        assert_eq!(Llr::new(0.0).magnitude(), 0.0);
    }

    #[test]
    fn test_infinity() {
        let llr = Llr::infinity();
        assert!(llr.value().is_infinite() && llr.value() > 0.0);
        assert!(!llr.hard_decision());
    }

    #[test]
    fn test_neg_infinity() {
        let llr = Llr::neg_infinity();
        assert!(llr.value().is_infinite() && llr.value() < 0.0);
        assert!(llr.hard_decision());
    }

    #[test]
    fn test_zero() {
        let llr = Llr::zero();
        assert_eq!(llr.value(), 0.0);
    }

    #[test]
    fn test_saturate_positive_overflow() {
        let llr = Llr::new(100.0).saturate(10.0);
        assert_eq!(llr.value(), 10.0);
    }

    #[test]
    fn test_saturate_negative_overflow() {
        let llr = Llr::new(-100.0).saturate(10.0);
        assert_eq!(llr.value(), -10.0);
    }

    #[test]
    fn test_saturate_within_range() {
        let llr = Llr::new(5.0).saturate(10.0);
        assert_eq!(llr.value(), 5.0);
    }

    #[test]
    fn test_boxplus_both_positive() {
        let a = Llr::new(3.0);
        let b = Llr::new(2.0);
        let result = a.boxplus(b);
        assert!(result.value() > 0.0);
        assert!(result.value() < 3.0);
    }

    #[test]
    fn test_boxplus_opposite_signs() {
        let a = Llr::new(3.0);
        let b = Llr::new(-2.0);
        let result = a.boxplus(b);
        assert!(result.value() < 0.0);
    }

    #[test]
    fn test_boxplus_both_negative() {
        let a = Llr::new(-3.0);
        let b = Llr::new(-2.0);
        let result = a.boxplus(b);
        assert!(result.value() > 0.0);
    }

    #[test]
    fn test_boxplus_with_zero() {
        let a = Llr::new(3.0);
        let b = Llr::zero();
        let result = a.boxplus(b);
        assert!(result.value().abs() < 0.1);
    }

    #[test]
    fn test_boxplus_minsum_both_positive() {
        let a = Llr::new(3.0);
        let b = Llr::new(2.0);
        assert_eq!(a.boxplus_minsum(b).value(), 2.0);
    }

    #[test]
    fn test_boxplus_minsum_opposite_signs() {
        let a = Llr::new(3.0);
        let b = Llr::new(-2.0);
        assert_eq!(a.boxplus_minsum(b).value(), -2.0);
    }

    #[test]
    fn test_boxplus_minsum_both_negative() {
        let a = Llr::new(-3.0);
        let b = Llr::new(-2.0);
        assert_eq!(a.boxplus_minsum(b).value(), 2.0);
    }

    #[test]
    fn test_boxplus_minsum_symmetric() {
        let a = Llr::new(3.0);
        let b = Llr::new(2.0);
        assert_eq!(a.boxplus_minsum(b), b.boxplus_minsum(a));
    }

    #[test]
    fn test_boxplus_n_all_positive() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0), Llr::new(4.0)];
        let result = Llr::boxplus_n(&llrs);
        assert!(result.value() > 0.0);
        assert!(result.value() < 2.0);
    }

    #[test]
    fn test_boxplus_n_mixed_signs_odd() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0), Llr::new(-4.0)];
        let result = Llr::boxplus_n(&llrs);
        assert!(result.value() < 0.0);
    }

    #[test]
    fn test_boxplus_n_mixed_signs_even() {
        let llrs = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(-4.0), Llr::new(5.0)];
        let result = Llr::boxplus_n(&llrs);
        assert!(result.value() > 0.0);
    }

    #[test]
    fn test_boxplus_n_single_element() {
        let llrs = vec![Llr::new(3.5)];
        let result = Llr::boxplus_n(&llrs);
        assert!((result.value() - 3.5).abs() < 0.1);
    }

    #[test]
    fn test_boxplus_n_two_elements_matches_binary() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0)];
        let result_n = Llr::boxplus_n(&llrs);
        let result_binary = Llr::new(3.0).boxplus(Llr::new(2.0));
        assert!((result_n.value() - result_binary.value()).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "Cannot compute boxplus_n of empty slice")]
    fn test_boxplus_n_empty_panics() {
        let llrs: Vec<Llr> = vec![];
        Llr::boxplus_n(&llrs);
    }

    #[test]
    fn test_boxplus_minsum_n_all_positive() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0), Llr::new(4.0)];
        let result = Llr::boxplus_minsum_n(&llrs);
        assert_eq!(result.value(), 2.0);
    }

    #[test]
    fn test_boxplus_minsum_n_one_negative() {
        let llrs = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(4.0)];
        let result = Llr::boxplus_minsum_n(&llrs);
        assert_eq!(result.value(), -2.0);
    }

    #[test]
    fn test_boxplus_minsum_n_two_negatives() {
        let llrs = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(-4.0)];
        let result = Llr::boxplus_minsum_n(&llrs);
        assert_eq!(result.value(), 2.0);
    }

    #[test]
    fn test_boxplus_minsum_n_single_element() {
        let llrs = vec![Llr::new(3.5)];
        let result = Llr::boxplus_minsum_n(&llrs);
        assert_eq!(result.value(), 3.5);
    }

    #[test]
    fn test_boxplus_minsum_n_matches_binary() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0)];
        let result_n = Llr::boxplus_minsum_n(&llrs);
        let result_binary = Llr::new(3.0).boxplus_minsum(Llr::new(2.0));
        assert_eq!(result_n.value(), result_binary.value());
    }

    #[test]
    #[should_panic(expected = "Cannot compute boxplus_minsum_n of empty slice")]
    fn test_boxplus_minsum_n_empty_panics() {
        let llrs: Vec<Llr> = vec![];
        Llr::boxplus_minsum_n(&llrs);
    }

    #[test]
    fn test_boxplus_normalized_minsum_n() {
        let llrs = vec![Llr::new(4.0), Llr::new(6.0)];
        let result = Llr::boxplus_normalized_minsum_n(&llrs, 0.875);
        assert_eq!(result.value(), 0.875 * 4.0);
    }

    #[test]
    fn test_boxplus_normalized_minsum_n_scales_correctly() {
        let llrs = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(5.0)];
        let alpha = 0.8;
        let result = Llr::boxplus_normalized_minsum_n(&llrs, alpha);
        let expected = -0.8 * 2.0;
        assert_eq!(result.value(), expected);
    }

    #[test]
    fn test_boxplus_offset_minsum_n() {
        let llrs = vec![Llr::new(4.0), Llr::new(6.0)];
        let result = Llr::boxplus_offset_minsum_n(&llrs, 0.5);
        assert_eq!(result.value(), 3.5);
    }

    #[test]
    fn test_boxplus_offset_minsum_n_clamps_to_zero() {
        let llrs = vec![Llr::new(0.3), Llr::new(6.0)];
        let result = Llr::boxplus_offset_minsum_n(&llrs, 0.5);
        assert_eq!(result.value(), 0.0);
    }

    #[test]
    fn test_boxplus_offset_minsum_n_preserves_sign() {
        let llrs = vec![Llr::new(-4.0), Llr::new(6.0)];
        let result = Llr::boxplus_offset_minsum_n(&llrs, 0.5);
        assert_eq!(result.value(), -3.5);
    }

    #[test]
    #[should_panic(expected = "Cannot compute boxplus_offset_minsum_n of empty slice")]
    fn test_boxplus_offset_minsum_n_empty_panics() {
        let llrs: Vec<Llr> = vec![];
        Llr::boxplus_offset_minsum_n(&llrs, 0.5);
    }

    #[test]
    fn test_is_finite_normal_value() {
        assert!(Llr::new(3.5).is_finite());
        assert!(Llr::new(-2.0).is_finite());
        assert!(Llr::new(0.0).is_finite());
    }

    #[test]
    fn test_is_finite_infinity() {
        assert!(!Llr::infinity().is_finite());
        assert!(!Llr::neg_infinity().is_finite());
    }

    #[test]
    fn test_is_finite_nan() {
        let nan_llr = Llr::new(f32::NAN);
        assert!(!nan_llr.is_finite());
    }

    #[test]
    fn test_safe_boxplus_normal_case() {
        let a = Llr::new(3.0);
        let b = Llr::new(2.0);
        let result = a.safe_boxplus(b);
        assert!(result.is_finite());
        assert!((result.value() - a.boxplus(b).value()).abs() < 1e-6);
    }

    #[test]
    fn test_safe_boxplus_extreme_values() {
        let a = Llr::new(1000.0);
        let b = Llr::new(1000.0);
        let result = a.safe_boxplus(b);
        assert!(result.is_finite());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn hard_decision_consistent_with_sign(value in -100.0f32..100.0f32) {
            let llr = Llr::new(value);
            assert_eq!(llr.hard_decision(), value < 0.0);
        }

        #[test]
        fn magnitude_always_non_negative(value in -100.0f32..100.0f32) {
            let llr = Llr::new(value);
            assert!(llr.magnitude() >= 0.0);
        }

        #[test]
        fn saturate_stays_within_bounds(value in -1000.0f32..1000.0f32, max in 1.0f32..100.0f32) {
            let saturated = Llr::new(value).saturate(max);
            assert!(saturated.value().abs() <= max);
        }

        #[test]
        fn boxplus_minsum_symmetric(a in -10.0f32..10.0f32, b in -10.0f32..10.0f32) {
            let llr_a = Llr::new(a);
            let llr_b = Llr::new(b);
            assert_eq!(
                llr_a.boxplus_minsum(llr_b).value(),
                llr_b.boxplus_minsum(llr_a).value()
            );
        }

        #[test]
        fn boxplus_symmetric(a in -10.0f32..10.0f32, b in -10.0f32..10.0f32) {
            let llr_a = Llr::new(a);
            let llr_b = Llr::new(b);
            let ab = llr_a.boxplus(llr_b).value();
            let ba = llr_b.boxplus(llr_a).value();
            prop_assert!((ab - ba).abs() < 1e-6);
        }

        #[test]
        fn boxplus_magnitude_bounded(a in -10.0f32..10.0f32, b in -10.0f32..10.0f32) {
            let result = Llr::new(a).boxplus(Llr::new(b));
            assert!(result.magnitude() <= a.abs().max(b.abs()) + 1e-6);
        }

        #[test]
        fn boxplus_n_commutative(
            values in prop::collection::vec(-10.0f32..10.0f32, 2..6)
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let mut shuffled = llrs.clone();
            shuffled.reverse();

            let result1 = Llr::boxplus_n(&llrs);
            let result2 = Llr::boxplus_n(&shuffled);

            // f32 precision near saturation (|LLR| ~10) causes atanh amplification
            // of ~1/(2*exp(-10)) ≈ 11000x, so per-ULP error reaches ~1e-3.
            prop_assert!((result1.value() - result2.value()).abs() < 1e-2);
        }

        #[test]
        fn boxplus_minsum_n_commutative(
            values in prop::collection::vec(-10.0f32..10.0f32, 2..6)
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let mut shuffled = llrs.clone();
            shuffled.reverse();

            let result1 = Llr::boxplus_minsum_n(&llrs);
            let result2 = Llr::boxplus_minsum_n(&shuffled);

            prop_assert_eq!(result1.value(), result2.value());
        }

        #[test]
        fn boxplus_minsum_n_magnitude_is_minimum(
            values in prop::collection::vec(-10.0f32..10.0f32, 1..6)
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let result = Llr::boxplus_minsum_n(&llrs);
            let min_magnitude = values.iter().map(|v| v.abs()).fold(f32::INFINITY, f32::min);

            let diff = (result.magnitude() - min_magnitude).abs();
            prop_assert!(diff < 1e-6, "magnitude {} differs from expected {} by {}",
                result.magnitude(), min_magnitude, diff);
        }

        #[test]
        fn boxplus_normalized_minsum_scales_result(
            values in prop::collection::vec(-10.0f32..10.0f32, 2..5),
            alpha in 0.5f32..1.0f32
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let minsum = Llr::boxplus_minsum_n(&llrs);
            let normalized = Llr::boxplus_normalized_minsum_n(&llrs, alpha);

            prop_assert!((normalized.value() - alpha * minsum.value()).abs() < 1e-6);
        }

        #[test]
        fn boxplus_offset_minsum_reduces_magnitude(
            values in prop::collection::vec(-10.0f32..10.0f32, 2..5),
            beta in 0.1f32..1.0f32
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let minsum = Llr::boxplus_minsum_n(&llrs);
            let offset = Llr::boxplus_offset_minsum_n(&llrs, beta);

            prop_assert!(offset.magnitude() <= minsum.magnitude());
        }

        #[test]
        fn boxplus_n_matches_binary_for_two_elements(
            a in -10.0f32..10.0f32,
            b in -10.0f32..10.0f32
        ) {
            let llrs = vec![Llr::new(a), Llr::new(b)];
            let result_n = Llr::boxplus_n(&llrs);
            let result_binary = Llr::new(a).boxplus(Llr::new(b));

            prop_assert!((result_n.value() - result_binary.value()).abs() < 1e-6);
        }

        #[test]
        fn boxplus_minsum_approximates_boxplus(
            values in prop::collection::vec(1.0f32..10.0f32, 2..5)
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let exact = Llr::boxplus_n(&llrs);
            let approx = Llr::boxplus_minsum_n(&llrs);

            prop_assert_eq!(exact.value() >= 0.0, approx.value() >= 0.0);

            // Min-sum overestimates more for uniform small inputs (e.g., four
            // values of 1.0: exact boxplus ≈ 0.09, min-sum = 1.0, ratio ≈ 11).
            // Bound of 15x accommodates worst-case uniform-small inputs.
            let ratio = (approx.magnitude() / exact.magnitude()).max(exact.magnitude() / approx.magnitude());
            prop_assert!(ratio < 15.0, "Approximation ratio {} exceeds acceptable bounds", ratio);
        }

        #[test]
        fn safe_boxplus_always_finite(a in -100.0f32..100.0f32, b in -100.0f32..100.0f32) {
            let result = Llr::new(a).safe_boxplus(Llr::new(b));
            prop_assert!(result.is_finite());
        }

        #[test]
        fn is_finite_correct_for_normal_values(value: f32) {
            prop_assert!(Llr::new(value).is_finite());
        }
    }
}
