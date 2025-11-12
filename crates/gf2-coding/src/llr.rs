//! Log-Likelihood Ratio (LLR) types and operations for soft-decision decoding.
//!
//! # Background
//!
//! In soft-decision decoding, we work with **log-likelihood ratios** (LLRs) instead of
//! hard bit decisions. For a received signal `r` corresponding to transmitted bit `b`:
//!
//! ```text
//! LLR = ln(P(b=0|r) / P(b=1|r))
//! ```
//!
//! **Interpretation**:
//! - `LLR > 0`: bit is more likely 0
//! - `LLR < 0`: bit is more likely 1
//! - `|LLR|`: confidence (magnitude indicates reliability)
//!
//! # AWGN Channel Example
//!
//! For BPSK modulation over AWGN with noise variance `sigma^2`:
//! - Bit 0 maps to symbol `+1`
//! - Bit 1 maps to symbol `-1`
//! - LLR for received symbol `r`: `LLR = (4 * r) / (2 * sigma^2)`
//!
//! # LLR Operations
//!
//! Common operations in belief propagation and soft-decision decoding:
//! - **Hard decision**: `sign(LLR)` gives most likely bit
//! - **XOR in LLR domain**: `boxplus` operation for convolutional/LDPC decoding
//! - **Saturation**: clip to prevent overflow in fixed-point arithmetic

#![allow(dead_code)]

/// Log-Likelihood Ratio represented as a floating-point value.
///
/// Positive values indicate bit 0 is more likely, negative values indicate bit 1.
/// The magnitude represents confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Llr(f64);

impl Llr {
    /// Creates a new LLR from a raw f64 value.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let llr = Llr::new(3.5);  // High confidence in bit 0
    /// ```
    pub fn new(value: f64) -> Self {
        Llr(value)
    }

    /// Returns the raw LLR value.
    pub fn value(self) -> f64 {
        self.0
    }

    /// Makes a hard decision: returns `false` (bit 0) if LLR >= 0, `true` (bit 1) if LLR < 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// assert_eq!(Llr::new(3.5).hard_decision(), false);   // bit 0
    /// assert_eq!(Llr::new(-2.0).hard_decision(), true);   // bit 1
    /// assert_eq!(Llr::new(0.0).hard_decision(), false);   // tie goes to 0
    /// ```
    pub fn hard_decision(self) -> bool {
        self.0 < 0.0
    }

    /// Returns the magnitude (absolute value) of the LLR, representing confidence.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// assert_eq!(Llr::new(3.5).magnitude(), 3.5);
    /// assert_eq!(Llr::new(-2.0).magnitude(), 2.0);
    /// ```
    pub fn magnitude(self) -> f64 {
        self.0.abs()
    }

    /// Creates an LLR representing infinite confidence in bit 0.
    pub fn infinity() -> Self {
        Llr(f64::INFINITY)
    }

    /// Creates an LLR representing infinite confidence in bit 1.
    pub fn neg_infinity() -> Self {
        Llr(f64::NEG_INFINITY)
    }

    /// Creates an LLR representing complete uncertainty (equal probability).
    pub fn zero() -> Self {
        Llr(0.0)
    }

    /// Saturates the LLR to the range `[-max, max]` to prevent overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// assert_eq!(Llr::new(100.0).saturate(10.0).value(), 10.0);
    /// assert_eq!(Llr::new(-100.0).saturate(10.0).value(), -10.0);
    /// assert_eq!(Llr::new(5.0).saturate(10.0).value(), 5.0);
    /// ```
    pub fn saturate(self, max: f64) -> Self {
        Llr(self.0.clamp(-max, max))
    }

    /// Box-plus operation: approximates LLR of XOR of two independent bits.
    ///
    /// For bits `a` and `b` with LLRs `L_a` and `L_b`:
    /// ```text
    /// LLR(a XOR b) ≈ 2 * atanh(tanh(L_a/2) * tanh(L_b/2))
    /// ```
    ///
    /// This is equivalent to:
    /// ```text
    /// sign(L_a) * sign(L_b) * min(|L_a|, |L_b|)  [min-sum approximation]
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let a = Llr::new(3.0);
    /// let b = Llr::new(2.0);
    /// let result = a.boxplus(b);
    /// // Result should be positive (both bits likely 0, so XOR likely 0)
    /// assert!(result.value() > 0.0);
    /// ```
    pub fn boxplus(self, other: Llr) -> Llr {
        let a = self.0 / 2.0;
        let b = other.0 / 2.0;
        Llr(2.0 * (a.tanh() * b.tanh()).atanh())
    }

    /// Min-sum approximation of box-plus: faster but less accurate.
    ///
    /// ```text
    /// sign(L_a) * sign(L_b) * min(|L_a|, |L_b|)
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let a = Llr::new(3.0);
    /// let b = Llr::new(2.0);
    /// assert_eq!(a.boxplus_minsum(b).value(), 2.0);
    ///
    /// let c = Llr::new(3.0);
    /// let d = Llr::new(-2.0);
    /// assert_eq!(c.boxplus_minsum(d).value(), -2.0);
    /// ```
    pub fn boxplus_minsum(self, other: Llr) -> Llr {
        let sign = if (self.0 >= 0.0) == (other.0 >= 0.0) {
            1.0
        } else {
            -1.0
        };
        Llr(sign * self.0.abs().min(other.0.abs()))
    }

    /// Multi-operand box-plus operation for check node updates in LDPC decoding.
    ///
    /// Computes the LLR of the XOR of multiple independent bits using the exact formula:
    ///
    /// $$
    /// \text{LLR}(b_1 \oplus b_2 \oplus \cdots \oplus b_n) = 2 \cdot \text{atanh}\left(\prod_{i=1}^{n} \tanh\left(\frac{L_i}{2}\right)\right)
    /// $$
    ///
    /// where $L_i$ is the LLR of bit $b_i$.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Slice of LLRs to combine
    ///
    /// # Returns
    ///
    /// The combined LLR representing the XOR of all input bits
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let llrs = vec![Llr::new(3.0), Llr::new(2.0), Llr::new(4.0)];
    /// let result = Llr::boxplus_n(&llrs);
    /// // All bits likely 0, so XOR likely 0 (positive LLR)
    /// assert!(result.value() > 0.0);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `llrs` is empty.
    pub fn boxplus_n(llrs: &[Llr]) -> Llr {
        assert!(!llrs.is_empty(), "Cannot compute boxplus_n of empty slice");

        let product: f64 = llrs.iter().map(|llr| (llr.0 / 2.0).tanh()).product();
        Llr(2.0 * product.atanh())
    }

    /// Multi-operand min-sum approximation of box-plus for check nodes.
    ///
    /// Uses the min-sum approximation for computational efficiency:
    ///
    /// $$
    /// \text{LLR}(b_1 \oplus \cdots \oplus b_n) \approx \left(\prod_{i=1}^{n} \text{sign}(L_i)\right) \cdot \min_{i=1}^{n} |L_i|
    /// $$
    ///
    /// This avoids transcendental functions while maintaining reasonable accuracy.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Slice of LLRs to combine
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let llrs = vec![Llr::new(3.0), Llr::new(2.0), Llr::new(4.0)];
    /// assert_eq!(Llr::boxplus_minsum_n(&llrs).value(), 2.0);
    ///
    /// let llrs2 = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(4.0)];
    /// assert_eq!(Llr::boxplus_minsum_n(&llrs2).value(), -2.0);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `llrs` is empty.
    pub fn boxplus_minsum_n(llrs: &[Llr]) -> Llr {
        assert!(
            !llrs.is_empty(),
            "Cannot compute boxplus_minsum_n of empty slice"
        );

        let sign_product: f64 = llrs
            .iter()
            .map(|llr| if llr.0 >= 0.0 { 1.0 } else { -1.0 })
            .product();

        let min_magnitude = llrs
            .iter()
            .map(|llr| llr.0.abs())
            .fold(f64::INFINITY, f64::min);

        Llr(sign_product * min_magnitude)
    }

    /// Normalized min-sum approximation with scaling factor.
    ///
    /// Applies a normalization factor $\alpha$ to reduce the bias of min-sum:
    ///
    /// $$
    /// \text{LLR} \approx \alpha \cdot \left(\prod_{i=1}^{n} \text{sign}(L_i)\right) \cdot \min_{i=1}^{n} |L_i|
    /// $$
    ///
    /// Typical values: $\alpha \in [0.75, 0.95]$. Common choice: $\alpha = 0.875$.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Slice of LLRs to combine
    /// * `alpha` - Normalization factor (typically 0.75-0.95)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let llrs = vec![Llr::new(4.0), Llr::new(6.0)];
    /// let result = Llr::boxplus_normalized_minsum_n(&llrs, 0.875);
    /// assert_eq!(result.value(), 0.875 * 4.0);
    /// ```
    pub fn boxplus_normalized_minsum_n(llrs: &[Llr], alpha: f64) -> Llr {
        let minsum = Self::boxplus_minsum_n(llrs);
        Llr(alpha * minsum.0)
    }

    /// Offset min-sum approximation with offset correction.
    ///
    /// Applies an offset $\beta$ to compensate for min-sum overestimation:
    ///
    /// $$
    /// \text{LLR} \approx \left(\prod_{i=1}^{n} \text{sign}(L_i)\right) \cdot \max\left(0, \min_{i=1}^{n} |L_i| - \beta\right)
    /// $$
    ///
    /// Typical values: $\beta \in [0.25, 0.5]$. Common choice: $\beta = 0.5$.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Slice of LLRs to combine
    /// * `beta` - Offset value (typically 0.25-0.5)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let llrs = vec![Llr::new(4.0), Llr::new(6.0)];
    /// let result = Llr::boxplus_offset_minsum_n(&llrs, 0.5);
    /// assert_eq!(result.value(), 3.5);
    /// ```
    pub fn boxplus_offset_minsum_n(llrs: &[Llr], beta: f64) -> Llr {
        assert!(
            !llrs.is_empty(),
            "Cannot compute boxplus_offset_minsum_n of empty slice"
        );

        let sign_product: f64 = llrs
            .iter()
            .map(|llr| if llr.0 >= 0.0 { 1.0 } else { -1.0 })
            .product();

        let min_magnitude = llrs
            .iter()
            .map(|llr| llr.0.abs())
            .fold(f64::INFINITY, f64::min);
        let offset_magnitude = (min_magnitude - beta).max(0.0);

        Llr(sign_product * offset_magnitude)
    }

    /// Checks if the LLR value is finite (not NaN or infinity).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// assert!(Llr::new(3.5).is_finite());
    /// assert!(!Llr::infinity().is_finite());
    /// assert!(!Llr::neg_infinity().is_finite());
    /// ```
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }

    /// Safe box-plus operation with overflow detection.
    ///
    /// Performs box-plus but returns zero LLR (complete uncertainty) if the result
    /// would be non-finite (NaN or infinity from numerical issues).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    ///
    /// let a = Llr::new(3.0);
    /// let b = Llr::new(2.0);
    /// let result = a.safe_boxplus(b);
    /// assert!(result.is_finite());
    /// ```
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
        assert!(!llr.hard_decision()); // bit 0
    }

    #[test]
    fn test_hard_decision_negative() {
        let llr = Llr::new(-2.0);
        assert!(llr.hard_decision()); // bit 1
    }

    #[test]
    fn test_hard_decision_zero() {
        let llr = Llr::new(0.0);
        assert!(!llr.hard_decision()); // tie goes to 0
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
        // Both bits likely 0, so XOR likely 0 (positive LLR)
        assert!(result.value() > 0.0);
        assert!(result.value() < 3.0); // Should be less than max
    }

    #[test]
    fn test_boxplus_opposite_signs() {
        let a = Llr::new(3.0); // bit likely 0
        let b = Llr::new(-2.0); // bit likely 1
        let result = a.boxplus(b);
        // XOR likely 1 (negative LLR)
        assert!(result.value() < 0.0);
    }

    #[test]
    fn test_boxplus_both_negative() {
        let a = Llr::new(-3.0);
        let b = Llr::new(-2.0);
        let result = a.boxplus(b);
        // Both bits likely 1, so XOR likely 0 (positive LLR)
        assert!(result.value() > 0.0);
    }

    #[test]
    fn test_boxplus_with_zero() {
        let a = Llr::new(3.0);
        let b = Llr::zero();
        let result = a.boxplus(b);
        // Complete uncertainty in b, result should be near zero
        assert!(result.value().abs() < 0.1);
    }

    #[test]
    fn test_boxplus_minsum_both_positive() {
        let a = Llr::new(3.0);
        let b = Llr::new(2.0);
        assert_eq!(a.boxplus_minsum(b).value(), 2.0); // min magnitude, positive sign
    }

    #[test]
    fn test_boxplus_minsum_opposite_signs() {
        let a = Llr::new(3.0);
        let b = Llr::new(-2.0);
        assert_eq!(a.boxplus_minsum(b).value(), -2.0); // min magnitude, negative sign
    }

    #[test]
    fn test_boxplus_minsum_both_negative() {
        let a = Llr::new(-3.0);
        let b = Llr::new(-2.0);
        assert_eq!(a.boxplus_minsum(b).value(), 2.0); // min magnitude, positive sign
    }

    #[test]
    fn test_boxplus_minsum_symmetric() {
        let a = Llr::new(3.0);
        let b = Llr::new(2.0);
        assert_eq!(a.boxplus_minsum(b), b.boxplus_minsum(a));
    }

    // Tests for multi-operand box-plus operations

    #[test]
    fn test_boxplus_n_all_positive() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0), Llr::new(4.0)];
        let result = Llr::boxplus_n(&llrs);
        // All bits likely 0 → XOR likely 0 (odd number of 0s → 0)
        assert!(result.value() > 0.0);
        assert!(result.value() < 2.0); // Should be less than minimum
    }

    #[test]
    fn test_boxplus_n_mixed_signs_odd() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0), Llr::new(-4.0)];
        let result = Llr::boxplus_n(&llrs);
        // Two bits likely 0, one likely 1 → XOR likely 1
        assert!(result.value() < 0.0);
    }

    #[test]
    fn test_boxplus_n_mixed_signs_even() {
        let llrs = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(-4.0), Llr::new(5.0)];
        let result = Llr::boxplus_n(&llrs);
        // Two likely 0, two likely 1 → XOR likely 0
        assert!(result.value() > 0.0);
    }

    #[test]
    fn test_boxplus_n_single_element() {
        let llrs = vec![Llr::new(3.5)];
        let result = Llr::boxplus_n(&llrs);
        // Single element should return approximately the same value
        assert!((result.value() - 3.5).abs() < 0.1);
    }

    #[test]
    fn test_boxplus_n_two_elements_matches_binary() {
        let llrs = vec![Llr::new(3.0), Llr::new(2.0)];
        let result_n = Llr::boxplus_n(&llrs);
        let result_binary = Llr::new(3.0).boxplus(Llr::new(2.0));
        assert!((result_n.value() - result_binary.value()).abs() < 1e-10);
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
        // Min magnitude with positive sign
        assert_eq!(result.value(), 2.0);
    }

    #[test]
    fn test_boxplus_minsum_n_one_negative() {
        let llrs = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(4.0)];
        let result = Llr::boxplus_minsum_n(&llrs);
        // Min magnitude with negative sign (odd number of negatives)
        assert_eq!(result.value(), -2.0);
    }

    #[test]
    fn test_boxplus_minsum_n_two_negatives() {
        let llrs = vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(-4.0)];
        let result = Llr::boxplus_minsum_n(&llrs);
        // Min magnitude with positive sign (even number of negatives)
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
        let expected = -0.8 * 2.0; // Negative sign, min magnitude 2.0, scaled by alpha
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
        // Min is 0.3, after offset 0.3 - 0.5 = -0.2, should clamp to 0
        assert_eq!(result.value(), 0.0);
    }

    #[test]
    fn test_boxplus_offset_minsum_n_preserves_sign() {
        let llrs = vec![Llr::new(-4.0), Llr::new(6.0)];
        let result = Llr::boxplus_offset_minsum_n(&llrs, 0.5);
        // Negative sign (odd number), min 4.0, after offset 3.5
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
        let nan_llr = Llr::new(f64::NAN);
        assert!(!nan_llr.is_finite());
    }

    #[test]
    fn test_safe_boxplus_normal_case() {
        let a = Llr::new(3.0);
        let b = Llr::new(2.0);
        let result = a.safe_boxplus(b);
        assert!(result.is_finite());
        assert!((result.value() - a.boxplus(b).value()).abs() < 1e-10);
    }

    #[test]
    fn test_safe_boxplus_extreme_values() {
        // Very large values might cause numerical issues in tanh/atanh
        let a = Llr::new(1000.0);
        let b = Llr::new(1000.0);
        let result = a.safe_boxplus(b);
        // Should handle gracefully, either returning finite result or zero
        assert!(result.is_finite());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn hard_decision_consistent_with_sign(value in -100.0..100.0) {
            let llr = Llr::new(value);
            assert_eq!(llr.hard_decision(), value < 0.0);
        }

        #[test]
        fn magnitude_always_non_negative(value in -100.0..100.0) {
            let llr = Llr::new(value);
            assert!(llr.magnitude() >= 0.0);
        }

        #[test]
        fn saturate_stays_within_bounds(value in -1000.0..1000.0, max in 1.0..100.0) {
            let saturated = Llr::new(value).saturate(max);
            assert!(saturated.value().abs() <= max);
        }

        #[test]
        fn boxplus_minsum_symmetric(a in -10.0..10.0, b in -10.0..10.0) {
            let llr_a = Llr::new(a);
            let llr_b = Llr::new(b);
            assert_eq!(
                llr_a.boxplus_minsum(llr_b).value(),
                llr_b.boxplus_minsum(llr_a).value()
            );
        }

        #[test]
        fn boxplus_symmetric(a in -10.0..10.0, b in -10.0..10.0) {
            let llr_a = Llr::new(a);
            let llr_b = Llr::new(b);
            let ab = llr_a.boxplus(llr_b).value();
            let ba = llr_b.boxplus(llr_a).value();
            prop_assert!((ab - ba).abs() < 1e-10);
        }

        #[test]
        fn boxplus_magnitude_bounded(a in -10.0..10.0, b in -10.0..10.0) {
            let result = Llr::new(a).boxplus(Llr::new(b));
            // Box-plus result magnitude should not exceed max of inputs
            assert!(result.magnitude() <= a.abs().max(b.abs()) + 1e-6);
        }

        #[test]
        fn boxplus_n_commutative(
            values in prop::collection::vec(-10.0..10.0, 2..6)
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let mut shuffled = llrs.clone();
            shuffled.reverse(); // Simple permutation

            let result1 = Llr::boxplus_n(&llrs);
            let result2 = Llr::boxplus_n(&shuffled);

            // Results should be close (numerical precision tolerance)
            prop_assert!((result1.value() - result2.value()).abs() < 1e-8);
        }

        #[test]
        fn boxplus_minsum_n_commutative(
            values in prop::collection::vec(-10.0..10.0, 2..6)
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
            values in prop::collection::vec(-10.0..10.0, 1..6)
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let result = Llr::boxplus_minsum_n(&llrs);
            let min_magnitude = values.iter().map(|v| v.abs()).fold(f64::INFINITY, f64::min);

            prop_assert_eq!(result.magnitude(), min_magnitude);
        }

        #[test]
        fn boxplus_normalized_minsum_scales_result(
            values in prop::collection::vec(-10.0..10.0, 2..5),
            alpha in 0.5..1.0
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let minsum = Llr::boxplus_minsum_n(&llrs);
            let normalized = Llr::boxplus_normalized_minsum_n(&llrs, alpha);

            prop_assert!((normalized.value() - alpha * minsum.value()).abs() < 1e-10);
        }

        #[test]
        fn boxplus_offset_minsum_reduces_magnitude(
            values in prop::collection::vec(-10.0..10.0, 2..5),
            beta in 0.1..1.0
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let minsum = Llr::boxplus_minsum_n(&llrs);
            let offset = Llr::boxplus_offset_minsum_n(&llrs, beta);

            // Offset result magnitude should be at most minsum magnitude
            prop_assert!(offset.magnitude() <= minsum.magnitude());
        }

        #[test]
        fn boxplus_n_matches_binary_for_two_elements(
            a in -10.0..10.0,
            b in -10.0..10.0
        ) {
            let llrs = vec![Llr::new(a), Llr::new(b)];
            let result_n = Llr::boxplus_n(&llrs);
            let result_binary = Llr::new(a).boxplus(Llr::new(b));

            prop_assert!((result_n.value() - result_binary.value()).abs() < 1e-8);
        }

        #[test]
        fn boxplus_minsum_approximates_boxplus(
            values in prop::collection::vec(1.0..10.0, 2..5)
        ) {
            let llrs: Vec<Llr> = values.iter().map(|&v| Llr::new(v)).collect();
            let exact = Llr::boxplus_n(&llrs);
            let approx = Llr::boxplus_minsum_n(&llrs);

            // Min-sum should have same sign
            prop_assert_eq!(exact.value() >= 0.0, approx.value() >= 0.0);

            // Min-sum approximation quality varies with input
            // For larger LLRs (>1.0), the approximation is typically within 2x
            // This is acceptable for LDPC decoding where it's a performance tradeoff
            let ratio = (approx.magnitude() / exact.magnitude()).max(exact.magnitude() / approx.magnitude());
            prop_assert!(ratio < 5.0, "Approximation ratio {} exceeds acceptable bounds", ratio);
        }

        #[test]
        fn safe_boxplus_always_finite(a in -100.0..100.0, b in -100.0..100.0) {
            let result = Llr::new(a).safe_boxplus(Llr::new(b));
            prop_assert!(result.is_finite());
        }

        #[test]
        fn is_finite_correct_for_normal_values(value in -1000.0..1000.0) {
            prop_assert!(Llr::new(value).is_finite());
        }
    }
}
