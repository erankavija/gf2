//! QPSK modulation and demodulation for fading channel simulations.
//!
//! # Overview
//!
//! This module provides QPSK (Quadrature Phase Shift Keying) modulation with
//! Gray labeling and LLR computation suitable for use with fading channels.
//!
//! # QPSK Constellation
//!
//! The QPSK constellation uses Gray labeling with four points:
//! ```text
//! X = { +Δ+Δj, -Δ+Δj, -Δ-Δj, +Δ-Δj }
//! ```
//! Mapping (Gray-coded): bits `[b1, b2]` →
//! - `[0, 0]` → `+Δ+Δj`
//! - `[0, 1]` → `+Δ-Δj`
//! - `[1, 0]` → `-Δ+Δj`
//! - `[1, 1]` → `-Δ-Δj`
//!
//! # LLR Computation
//!
//! For received signal `y` and channel estimate `h_hat`, the per-bit LLRs are:
//! ```text
//! L_1 = 2·Δ·Re(y·conj(h_hat)) / σ²
//! L_2 = 2·Δ·Im(y·conj(h_hat)) / σ²
//! ```

use std::ops::{Add, Mul};

use crate::llr::Llr;

/// Complex number type for QPSK computations.
///
/// Represents a complex number `re + im·j`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl Complex {
    /// Creates a new complex number.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::Complex;
    ///
    /// let c = Complex::new(1.0, -2.0);
    /// assert_eq!(c.re, 1.0);
    /// assert_eq!(c.im, -2.0);
    /// ```
    pub fn new(re: f64, im: f64) -> Self {
        Complex { re, im }
    }

    /// Returns the complex conjugate.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::Complex;
    ///
    /// let c = Complex::new(1.0, -2.0);
    /// let conj = c.conj();
    /// assert_eq!(conj.re, 1.0);
    /// assert_eq!(conj.im, 2.0);
    /// ```
    pub fn conj(self) -> Self {
        Complex {
            re: self.re,
            im: -self.im,
        }
    }

    /// Returns the squared absolute value |z|².
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::Complex;
    ///
    /// let c = Complex::new(3.0, 4.0);
    /// assert!((c.norm_sq() - 25.0).abs() < 1e-12);
    /// ```
    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Returns the absolute value |z|.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::Complex;
    ///
    /// let c = Complex::new(3.0, 4.0);
    /// assert!((c.norm() - 5.0).abs() < 1e-12);
    /// ```
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Scales the complex number by a real scalar.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::Complex;
    ///
    /// let c = Complex::new(1.0, -2.0).scale(3.0);
    /// assert_eq!(c.re, 3.0);
    /// assert_eq!(c.im, -6.0);
    /// ```
    pub fn scale(self, s: f64) -> Self {
        Complex {
            re: self.re * s,
            im: self.im * s,
        }
    }
}

impl Mul for Complex {
    type Output = Complex;

    /// Multiplies two complex numbers.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::Complex;
    ///
    /// let a = Complex::new(1.0, 2.0);
    /// let b = Complex::new(3.0, 4.0);
    /// let c = a * b;
    /// assert!((c.re - (-5.0)).abs() < 1e-12);
    /// assert!((c.im - 10.0).abs() < 1e-12);
    /// ```
    fn mul(self, other: Complex) -> Complex {
        Complex {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
}

impl Add for Complex {
    type Output = Complex;

    /// Adds two complex numbers.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::Complex;
    ///
    /// let a = Complex::new(1.0, 2.0);
    /// let b = Complex::new(3.0, -1.0);
    /// let c = a + b;
    /// assert_eq!(c.re, 4.0);
    /// assert_eq!(c.im, 1.0);
    /// ```
    fn add(self, other: Complex) -> Complex {
        Complex {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
}

/// QPSK modulator with Gray labeling.
///
/// Maps pairs of bits to complex QPSK symbols. The constellation amplitude
/// parameter `delta` (Δ) controls the energy per symbol:
/// `Es = 2·Δ²·E[|H|²]`.
///
/// # Gray mapping
///
/// | bits [b1, b2] | symbol |
/// |---------------|--------|
/// | [0, 0]        | +Δ+Δj  |
/// | [0, 1]        | +Δ-Δj  |
/// | [1, 0]        | -Δ+Δj  |
/// | [1, 1]        | -Δ-Δj  |
pub struct QpskModulator {
    delta: f64,
}

impl QpskModulator {
    /// Creates a new QPSK modulator with given constellation amplitude Δ.
    ///
    /// The per-symbol energy (in AWGN, no fading) is `Es = 2·Δ²`.
    ///
    /// # Arguments
    ///
    /// * `delta` - Constellation amplitude (must be positive)
    ///
    /// # Panics
    ///
    /// Panics if `delta <= 0.0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::QpskModulator;
    ///
    /// let qpsk = QpskModulator::new(1.0 / 2.0_f64.sqrt());
    /// ```
    pub fn new(delta: f64) -> Self {
        assert!(
            delta > 0.0,
            "Constellation amplitude delta must be positive"
        );
        QpskModulator { delta }
    }

    /// Returns the constellation amplitude Δ.
    pub fn delta(&self) -> f64 {
        self.delta
    }

    /// Modulates a pair of bits to a QPSK symbol.
    ///
    /// # Arguments
    ///
    /// * `b1` - First bit (controls real axis sign)
    /// * `b2` - Second bit (controls imaginary axis sign)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::QpskModulator;
    ///
    /// let qpsk = QpskModulator::new(1.0);
    /// let s = qpsk.modulate(false, false);
    /// assert!((s.re - 1.0).abs() < 1e-12);
    /// assert!((s.im - 1.0).abs() < 1e-12);
    /// ```
    pub fn modulate(&self, b1: bool, b2: bool) -> Complex {
        let re = if b1 { -self.delta } else { self.delta };
        let im = if b2 { -self.delta } else { self.delta };
        Complex::new(re, im)
    }

    /// Modulates a slice of bits (must have even length) to QPSK symbols.
    ///
    /// Consecutive pairs `[b_{2k}, b_{2k+1}]` are mapped to one symbol each.
    ///
    /// # Arguments
    ///
    /// * `bits` - Slice of bits with even length
    ///
    /// # Panics
    ///
    /// Panics if `bits.len()` is odd.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::QpskModulator;
    ///
    /// let qpsk = QpskModulator::new(1.0);
    /// let bits = vec![false, false, true, true];
    /// let symbols = qpsk.modulate_bits(&bits);
    /// assert_eq!(symbols.len(), 2);
    /// assert!((symbols[0].re - 1.0).abs() < 1e-12);
    /// assert!((symbols[1].re + 1.0).abs() < 1e-12);
    /// ```
    pub fn modulate_bits(&self, bits: &[bool]) -> Vec<Complex> {
        assert_eq!(
            bits.len() % 2,
            0,
            "QPSK requires even number of bits, got {}",
            bits.len()
        );
        bits.chunks(2)
            .map(|chunk| self.modulate(chunk[0], chunk[1]))
            .collect()
    }

    /// Computes soft LLRs for a received symbol given channel estimate and noise variance.
    ///
    /// For received signal `y = h·x + n` with channel estimate `h_hat`, the optimal
    /// soft LLRs (assuming perfect channel estimation) are:
    /// ```text
    /// L_1 = 2·Δ·Re(y·conj(h_hat)) / σ²
    /// L_2 = 2·Δ·Im(y·conj(h_hat)) / σ²
    /// ```
    ///
    /// # Arguments
    ///
    /// * `y` - Received complex symbol
    /// * `h_hat` - Complex channel estimate
    /// * `sigma_squared` - Noise variance σ²
    ///
    /// # Returns
    ///
    /// `(llr_b1, llr_b2)` — LLRs for the two bits in the symbol
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::{Complex, QpskModulator};
    ///
    /// let qpsk = QpskModulator::new(1.0);
    /// // Perfect channel (h=1), noiseless: y = modulate(0,0) = 1+1j
    /// let y = Complex::new(1.0, 1.0);
    /// let h_hat = Complex::new(1.0, 0.0); // unity channel
    /// let (l1, l2) = qpsk.soft_llrs(y, h_hat, 0.5);
    /// assert!(l1.value() > 0.0); // b1=0 → positive LLR
    /// assert!(l2.value() > 0.0); // b2=0 → positive LLR
    /// ```
    pub fn soft_llrs(&self, y: Complex, h_hat: Complex, sigma_squared: f64) -> (Llr, Llr) {
        let z = y * h_hat.conj();
        let scale = 2.0 * self.delta / sigma_squared;
        let l1 = Llr::new((scale * z.re) as f32);
        let l2 = Llr::new((scale * z.im) as f32);
        (l1, l2)
    }

    /// Computes LLRs for all received symbols, returning a flat LLR vector.
    ///
    /// The returned LLRs are ordered as `[L_1(0), L_2(0), L_1(1), L_2(1), ...]`,
    /// matching the order in which bits were modulated.
    ///
    /// # Arguments
    ///
    /// * `received` - Slice of received complex symbols
    /// * `channel_estimates` - Per-symbol complex channel estimates (same length as `received`)
    /// * `sigma_squared` - Noise variance σ²
    ///
    /// # Panics
    ///
    /// Panics if `received` and `channel_estimates` have different lengths.
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of symbols.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::{Complex, QpskModulator};
    ///
    /// let qpsk = QpskModulator::new(1.0);
    /// let bits = vec![false, false, true, false];
    /// let symbols = qpsk.modulate_bits(&bits);
    /// let h = vec![Complex::new(1.0, 0.0); symbols.len()];
    /// let llrs = qpsk.symbols_to_llrs(&symbols, &h, 0.5);
    /// assert_eq!(llrs.len(), 4); // 2 symbols × 2 bits each
    /// ```
    pub fn symbols_to_llrs(
        &self,
        received: &[Complex],
        channel_estimates: &[Complex],
        sigma_squared: f64,
    ) -> Vec<Llr> {
        assert_eq!(
            received.len(),
            channel_estimates.len(),
            "received and channel_estimates must have equal length"
        );
        let mut llrs = Vec::with_capacity(received.len() * 2);
        for (&y, &h) in received.iter().zip(channel_estimates.iter()) {
            let (l1, l2) = self.soft_llrs(y, h, sigma_squared);
            llrs.push(l1);
            llrs.push(l2);
        }
        llrs
    }

    /// Hard-demodulates a received symbol pair to bits (no channel compensation).
    ///
    /// Signs of real and imaginary parts determine the hard decisions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modulation::{Complex, QpskModulator};
    ///
    /// let qpsk = QpskModulator::new(1.0);
    /// let (b1, b2) = qpsk.demodulate_hard(Complex::new(0.7, -0.3));
    /// assert_eq!(b1, false); // re > 0 → b1 = 0
    /// assert_eq!(b2, true);  // im < 0 → b2 = 1
    /// ```
    pub fn demodulate_hard(&self, y: Complex) -> (bool, bool) {
        let b1 = y.re < 0.0;
        let b2 = y.im < 0.0;
        (b1, b2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Complex arithmetic ----

    #[test]
    fn test_complex_new() {
        let c = Complex::new(3.0, -4.0);
        assert_eq!(c.re, 3.0);
        assert_eq!(c.im, -4.0);
    }

    #[test]
    fn test_complex_conj() {
        let c = Complex::new(3.0, -4.0).conj();
        assert_eq!(c.re, 3.0);
        assert_eq!(c.im, 4.0);
    }

    #[test]
    fn test_complex_norm_sq() {
        let c = Complex::new(3.0, 4.0);
        assert!((c.norm_sq() - 25.0).abs() < 1e-12);
    }

    #[test]
    fn test_complex_norm() {
        let c = Complex::new(3.0, 4.0);
        assert!((c.norm() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_complex_mul() {
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, 4.0);
        let c = a * b;
        // (1+2j)(3+4j) = 3+4j+6j+8j² = 3+10j-8 = -5+10j
        assert!((c.re + 5.0).abs() < 1e-12);
        assert!((c.im - 10.0).abs() < 1e-12);
    }

    #[test]
    fn test_complex_scale() {
        let c = Complex::new(1.0, -2.0).scale(3.0);
        assert!((c.re - 3.0).abs() < 1e-12);
        assert!((c.im + 6.0).abs() < 1e-12);
    }

    #[test]
    fn test_complex_add() {
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, -1.0);
        let c = a + b;
        assert_eq!(c.re, 4.0);
        assert_eq!(c.im, 1.0);
    }

    // ---- QPSK modulator ----

    #[test]
    fn test_qpsk_new_positive_delta() {
        let qpsk = QpskModulator::new(0.5);
        assert!((qpsk.delta() - 0.5).abs() < 1e-12);
    }

    #[test]
    #[should_panic(expected = "delta must be positive")]
    fn test_qpsk_new_zero_delta_panics() {
        let _ = QpskModulator::new(0.0);
    }

    #[test]
    fn test_qpsk_modulate_00() {
        let qpsk = QpskModulator::new(1.0);
        let s = qpsk.modulate(false, false);
        assert!((s.re - 1.0).abs() < 1e-12);
        assert!((s.im - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_qpsk_modulate_01() {
        let qpsk = QpskModulator::new(1.0);
        let s = qpsk.modulate(false, true);
        assert!((s.re - 1.0).abs() < 1e-12);
        assert!((s.im + 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_qpsk_modulate_10() {
        let qpsk = QpskModulator::new(1.0);
        let s = qpsk.modulate(true, false);
        assert!((s.re + 1.0).abs() < 1e-12);
        assert!((s.im - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_qpsk_modulate_11() {
        let qpsk = QpskModulator::new(1.0);
        let s = qpsk.modulate(true, true);
        assert!((s.re + 1.0).abs() < 1e-12);
        assert!((s.im + 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_qpsk_modulate_bits_length() {
        let qpsk = QpskModulator::new(1.0);
        let bits = vec![false, true, true, false, false, false];
        let symbols = qpsk.modulate_bits(&bits);
        assert_eq!(symbols.len(), 3);
    }

    #[test]
    #[should_panic(expected = "even number of bits")]
    fn test_qpsk_modulate_bits_odd_panics() {
        let qpsk = QpskModulator::new(1.0);
        qpsk.modulate_bits(&[false, false, false]);
    }

    #[test]
    fn test_qpsk_symbol_energy() {
        // Each constellation point should have energy delta^2 + delta^2 = 2*delta^2
        let delta = 0.7;
        let qpsk = QpskModulator::new(delta);
        for &b1 in &[false, true] {
            for &b2 in &[false, true] {
                let s = qpsk.modulate(b1, b2);
                let energy = s.norm_sq();
                assert!((energy - 2.0 * delta * delta).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn test_qpsk_soft_llrs_correct_sign_noiseless() {
        // Perfect channel h=1+0j, noiseless reception
        let delta = 1.0;
        let qpsk = QpskModulator::new(delta);
        let h = Complex::new(1.0, 0.0);
        let sigma_sq = 0.5;

        for &b1 in &[false, true] {
            for &b2 in &[false, true] {
                let s = qpsk.modulate(b1, b2);
                let (l1, l2) = qpsk.soft_llrs(s, h, sigma_sq);
                if b1 {
                    assert!(l1.value() < 0.0, "bit1=1 should give negative LLR");
                } else {
                    assert!(l1.value() > 0.0, "bit1=0 should give positive LLR");
                }
                if b2 {
                    assert!(l2.value() < 0.0, "bit2=1 should give negative LLR");
                } else {
                    assert!(l2.value() > 0.0, "bit2=0 should give positive LLR");
                }
            }
        }
    }

    #[test]
    fn test_qpsk_soft_llrs_formula() {
        // Verify exact formula: L = 2*delta*Re/Im(y*conj(h)) / sigma^2
        let delta = 2.0;
        let qpsk = QpskModulator::new(delta);
        let y = Complex::new(1.5, -0.5);
        let h = Complex::new(0.8, 0.3);
        let sigma_sq = 1.0;

        let (l1, l2) = qpsk.soft_llrs(y, h, sigma_sq);
        // y * conj(h) = (1.5 - 0.5j)(0.8 - 0.3j) = 1.5*0.8 - 1.5*0.3j - 0.5*0.8*j + 0.5*0.3*j^2*(-1)
        // = 1.2 - 0.45j - 0.4j - 0.15 = 1.05 - 0.85j
        let z = y.mul(h.conj());
        let expected_l1 = (2.0 * delta * z.re / sigma_sq) as f32;
        let expected_l2 = (2.0 * delta * z.im / sigma_sq) as f32;
        assert!((l1.value() - expected_l1).abs() < 1e-5);
        assert!((l2.value() - expected_l2).abs() < 1e-5);
    }

    #[test]
    fn test_qpsk_symbols_to_llrs_length() {
        let qpsk = QpskModulator::new(1.0);
        let bits = vec![false, false, true, true, false, true];
        let symbols = qpsk.modulate_bits(&bits);
        let h = vec![Complex::new(1.0, 0.0); symbols.len()];
        let llrs = qpsk.symbols_to_llrs(&symbols, &h, 0.5);
        assert_eq!(llrs.len(), bits.len());
    }

    #[test]
    fn test_qpsk_hard_demodulate_all_quadrants() {
        let qpsk = QpskModulator::new(1.0);
        assert_eq!(qpsk.demodulate_hard(Complex::new(0.9, 0.8)), (false, false));
        assert_eq!(qpsk.demodulate_hard(Complex::new(0.9, -0.8)), (false, true));
        assert_eq!(qpsk.demodulate_hard(Complex::new(-0.9, 0.8)), (true, false));
        assert_eq!(qpsk.demodulate_hard(Complex::new(-0.9, -0.8)), (true, true));
    }

    #[test]
    fn test_qpsk_roundtrip_noiseless() {
        // Hard-demodulate modulated symbols should give back original bits
        let qpsk = QpskModulator::new(1.5);
        for &b1 in &[false, true] {
            for &b2 in &[false, true] {
                let s = qpsk.modulate(b1, b2);
                let (d1, d2) = qpsk.demodulate_hard(s);
                assert_eq!((d1, d2), (b1, b2), "Roundtrip failed for ({b1}, {b2})");
            }
        }
    }

    #[test]
    fn test_qpsk_gray_coding_adjacent_symbols_differ_one_bit() {
        // Adjacent constellation points in real axis differ by b1 only, imaginary by b2 only
        let qpsk = QpskModulator::new(1.0);
        // (false,false) and (true,false) differ only in b1
        let s00 = qpsk.modulate(false, false);
        let s10 = qpsk.modulate(true, false);
        assert!((s00.im - s10.im).abs() < 1e-12);
        assert!((s00.re - s10.re).abs() > 1e-6);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn qpsk_symbol_always_has_correct_energy(delta in 0.01f64..10.0f64, b1: bool, b2: bool) {
            let qpsk = QpskModulator::new(delta);
            let s = qpsk.modulate(b1, b2);
            let energy = s.norm_sq();
            prop_assert!((energy - 2.0 * delta * delta).abs() < 1e-10);
        }

        #[test]
        fn qpsk_roundtrip_hard_noiseless(delta in 0.01f64..10.0f64, b1: bool, b2: bool) {
            let qpsk = QpskModulator::new(delta);
            let s = qpsk.modulate(b1, b2);
            let (d1, d2) = qpsk.demodulate_hard(s);
            prop_assert_eq!((d1, d2), (b1, b2));
        }

        #[test]
        fn qpsk_llr_sign_noiseless_unit_channel(delta in 0.01f64..10.0f64, b1: bool, b2: bool) {
            let qpsk = QpskModulator::new(delta);
            let s = qpsk.modulate(b1, b2);
            let h = Complex::new(1.0, 0.0);
            let (l1, l2) = qpsk.soft_llrs(s, h, 1.0);
            if b1 {
                prop_assert!(l1.value() < 0.0);
            } else {
                prop_assert!(l1.value() > 0.0);
            }
            if b2 {
                prop_assert!(l2.value() < 0.0);
            } else {
                prop_assert!(l2.value() > 0.0);
            }
        }

        #[test]
        fn complex_mul_conj_gives_norm_sq(re in -10.0f64..10.0f64, im in -10.0f64..10.0f64) {
            let c = Complex::new(re, im);
            let product = c * c.conj();
            prop_assert!((product.re - c.norm_sq()).abs() < 1e-10);
            prop_assert!(product.im.abs() < 1e-10);
        }
    }
}
