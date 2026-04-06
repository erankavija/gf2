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
    /// # Arguments
    ///
    /// * `re` - Real part of the complex number
    /// * `im` - Imaginary part of the complex number
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
    ///
    /// # Complexity
    ///
    /// O(1).
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
    ///
    /// # Complexity
    ///
    /// O(1).
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
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Scales the complex number by a real scalar.
    ///
    /// # Arguments
    ///
    /// * `s` - Real scalar to multiply by
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
    ///
    /// # Complexity
    ///
    /// O(1).
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
    ///
    /// # Complexity
    ///
    /// O(1).
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
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of bits.
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
    ///
    /// # Complexity
    ///
    /// O(1).
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
    /// # Arguments
    ///
    /// * `y` - Received complex symbol
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
    ///
    /// # Complexity
    ///
    /// O(1).
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

    // ---- QPSK BER vs theory in uncoded AWGN ----

    #[test]
    fn test_qpsk_uncoded_ber_matches_theory() {
        // Theoretical QPSK BER over AWGN: BER = erfc(sqrt(Eb/N0)) / 2
        // For QPSK with Gray labeling, each bit sees an effective BPSK channel,
        // so the BER formula is the same as BPSK: Q(sqrt(2*Eb/N0)) = erfc(sqrt(Eb/N0))/2.
        //
        // We test at several Eb/N0 points with enough samples for statistical reliability.
        use rand::Rng;
        use rand::SeedableRng;
        use rand_distr::{Distribution, Normal};

        let mut rng = rand::rngs::StdRng::seed_from_u64(0xCAFE);

        // QPSK with unit energy per symbol: Es = 2*delta^2 = 1, so delta = 1/sqrt(2)
        // For QPSK, Eb = Es / 2 (2 bits per symbol), so Eb = 0.5
        // sigma^2 = N0/2 = Eb/(2*Eb/N0_linear) ... we use the relation directly.
        //
        // More precisely: for Eb/N0 = gamma, noise variance per complex dim is
        // sigma^2 = Es / (2 * log2(M) * gamma) = 1 / (2*2*gamma) for M=4
        // But since Es = 2*delta^2 and Eb = Es/log2(M) = Es/2:
        //   sigma^2 = Eb / gamma = 0.5 / gamma  (per complex dimension, real+imag)
        //
        // Actually for QPSK: sigma^2_per_dim = N0/2. And Eb/N0 = gamma means N0 = Eb/gamma.
        // With delta = 1/sqrt(2), Es = 2*delta^2 = 1, Eb = Es/2 = 0.5.
        // N0 = Eb/gamma = 0.5/gamma, sigma^2_per_dim = N0/2 = 0.25/gamma.
        // Total complex noise variance = N0 = 0.5/gamma.

        let delta = 1.0_f64 / 2.0_f64.sqrt();
        let qpsk = QpskModulator::new(delta);

        // Test points: (Eb/N0 in dB, theoretical BER)
        let test_points: Vec<(f64, f64)> = vec![
            (0.0, theoretical_qpsk_ber(0.0)),
            (2.0, theoretical_qpsk_ber(2.0)),
            (4.0, theoretical_qpsk_ber(4.0)),
            (6.0, theoretical_qpsk_ber(6.0)),
            (8.0, theoretical_qpsk_ber(8.0)),
        ];

        for (eb_n0_db, expected_ber) in &test_points {
            let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);

            // Noise variance: per real/imag component
            // sigma_component^2 = N0/2 = Eb/(2*gamma) = 0.5/(2*gamma) = 0.25/gamma
            let sigma_component = (0.25 / eb_n0_linear).sqrt();
            let noise_dist = Normal::new(0.0, sigma_component).unwrap();

            // Number of bits to transmit: enough for statistical reliability
            // At high SNR, BER is small, so we need more bits
            let num_bits = if *eb_n0_db >= 6.0 { 2_000_000 } else { 500_000 };
            // Must be even for QPSK
            let num_bits = num_bits + (num_bits % 2);

            let mut total_errors = 0usize;
            let mut total_bits = 0usize;

            // Process in chunks
            let chunk_size = 1000; // bits per chunk (must be even)
            let mut bits_remaining = num_bits;

            while bits_remaining > 0 {
                let this_chunk = chunk_size.min(bits_remaining);
                let bits: Vec<bool> = (0..this_chunk).map(|_| rng.gen()).collect();
                let symbols = qpsk.modulate_bits(&bits);

                // Transmit through AWGN (h=1, no fading)
                let received: Vec<Complex> = symbols
                    .iter()
                    .map(|&s| {
                        Complex::new(
                            s.re + noise_dist.sample(&mut rng),
                            s.im + noise_dist.sample(&mut rng),
                        )
                    })
                    .collect();

                // Hard demodulate
                for (i, &y) in received.iter().enumerate() {
                    let (d1, d2) = qpsk.demodulate_hard(y);
                    if d1 != bits[2 * i] {
                        total_errors += 1;
                    }
                    if d2 != bits[2 * i + 1] {
                        total_errors += 1;
                    }
                }

                total_bits += this_chunk;
                bits_remaining -= this_chunk;
            }

            let measured_ber = total_errors as f64 / total_bits as f64;
            let tolerance = 0.15; // 15% relative tolerance

            // For very low BER, use absolute tolerance instead
            if *expected_ber < 1e-5 {
                assert!(
                    measured_ber < 1e-4,
                    "At Eb/N0={eb_n0_db} dB: measured BER {measured_ber:.6} too high \
                     (expected ~{expected_ber:.6})"
                );
            } else {
                let ratio = measured_ber / expected_ber;
                assert!(
                    (1.0 - tolerance..=1.0 + tolerance).contains(&ratio),
                    "At Eb/N0={eb_n0_db} dB: measured BER {measured_ber:.6} vs \
                     theoretical {expected_ber:.6} (ratio {ratio:.3}, tolerance {tolerance})"
                );
            }
        }
    }

    /// Theoretical QPSK BER: BER = erfc(sqrt(Eb/N0)) / 2
    fn theoretical_qpsk_ber(eb_n0_db: f64) -> f64 {
        let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
        erfc(eb_n0_linear.sqrt()) / 2.0
    }

    /// Complementary error function approximation (Abramowitz & Stegun 7.1.26).
    fn erfc(x: f64) -> f64 {
        // For negative x: erfc(-x) = 2 - erfc(x)
        if x < 0.0 {
            return 2.0 - erfc(-x);
        }
        let t = 1.0 / (1.0 + 0.3275911 * x);
        let poly = t
            * (0.254829592
                + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
        poly * (-x * x).exp()
    }

    // ---- Simulation surface integration: QPSK + fading channel decode loop ----

    #[test]
    fn test_qpsk_fading_channel_decode_loop() {
        // Demonstrates that QPSK modulation + Rician fading channel can feed
        // into a decode loop, verifying the integration surface between the
        // modulation/fading modules and the simulation harness.
        use crate::fading::{BitInterleaver, RicianChannel, RicianConfig};
        use rand::Rng;
        use rand::SeedableRng;

        let mut rng = rand::rngs::StdRng::seed_from_u64(0xBEEF);
        let cfg = RicianConfig::fig8(); // K=5, N_c=128, t=4 → 1024 bits
        let channel = RicianChannel::new(cfg);
        let delta = 1.0_f64 / 2.0_f64.sqrt();
        let qpsk = QpskModulator::new(delta);
        let interleaver = BitInterleaver::new(cfg.frame_bits(), 42);

        let eb_n0_db = 10.0;
        let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
        // sigma^2 = Eb / gamma = 0.5 / gamma (for unit-energy QPSK)
        let sigma_sq = 0.5 / eb_n0_linear;

        // Generate random data bits
        let tx_bits: Vec<bool> = (0..cfg.frame_bits()).map(|_| rng.gen()).collect();

        // Interleave
        let interleaved = interleaver.interleave(&tx_bits);

        // QPSK modulate
        let symbols = qpsk.modulate_bits(&interleaved);

        // Pass through Rician fading channel
        let gains = channel.generate_frame_gains(&mut rng);
        let received = channel.transmit(&symbols, &gains, sigma_sq, &mut rng);

        // Compute LLRs with channel estimates (perfect CSI)
        let llrs = qpsk.symbols_to_llrs(&received, &gains, sigma_sq);

        // De-interleave LLRs
        let deinterleaved_llrs = interleaver.deinterleave_llrs(&llrs);

        // Hard decisions from LLRs (simulate a trivial "decoder")
        let decoded_bits: Vec<bool> = deinterleaved_llrs.iter().map(|l| l.value() < 0.0).collect();

        // At 10 dB with Rician fading (K=5), most bits should be correct
        let errors: usize = tx_bits
            .iter()
            .zip(decoded_bits.iter())
            .filter(|(&a, &b)| a != b)
            .count();
        let ber = errors as f64 / cfg.frame_bits() as f64;
        assert!(
            ber < 0.05,
            "BER {ber:.4} too high for 10 dB Rician K=5 channel"
        );
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
