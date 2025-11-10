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
    }
}
