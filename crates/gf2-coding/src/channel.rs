//! AWGN (Additive White Gaussian Noise) channel modeling for BER/FER simulations.
//!
//! # Overview
//!
//! This module provides tools for simulating transmission over an AWGN channel:
//! - BPSK modulation: maps bits to symbols (0 → +1, 1 → -1)
//! - AWGN noise generation using Box-Muller transform
//! - Channel simulation with configurable Eb/N0 (energy per bit to noise ratio)
//! - Conversion from received symbols back to LLRs for soft-decision decoding
//!
//! # AWGN Channel Model
//!
//! The AWGN channel adds Gaussian noise to transmitted symbols:
//! ```text
//! r = s + n, where n ~ N(0, sigma^2)
//! ```
//!
//! The noise variance `sigma^2` relates to `Eb/N0` (in dB) by:
//! ```text
//! sigma^2 = 1 / (2 * R * 10^(Eb/N0_dB / 10))
//! ```
//! where `R` is the code rate.
//!
//! # LLR Computation
//!
//! For BPSK over AWGN, the optimal LLR for received symbol `r` is:
//! ```text
//! LLR = (2 * r) / sigma^2
//! ```

use crate::llr::Llr;
use rand::Rng;
use rand_distr::{Distribution, Normal};

/// BPSK (Binary Phase Shift Keying) modulator.
///
/// Maps bits to symbols: `false` (0) → +1.0, `true` (1) → -1.0
pub struct BpskModulator;

impl BpskModulator {
    /// Modulates a single bit to a BPSK symbol.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::BpskModulator;
    ///
    /// assert_eq!(BpskModulator::modulate(false), 1.0);
    /// assert_eq!(BpskModulator::modulate(true), -1.0);
    /// ```
    pub fn modulate(bit: bool) -> f64 {
        if bit {
            -1.0
        } else {
            1.0
        }
    }

    /// Modulates a slice of bits to BPSK symbols.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::BpskModulator;
    ///
    /// let bits = vec![false, true, false, true];
    /// let symbols = BpskModulator::modulate_bits(&bits);
    /// assert_eq!(symbols, vec![1.0, -1.0, 1.0, -1.0]);
    /// ```
    pub fn modulate_bits(bits: &[bool]) -> Vec<f64> {
        bits.iter().map(|&b| Self::modulate(b)).collect()
    }

    /// Hard demodulates a symbol back to a bit.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::BpskModulator;
    ///
    /// assert_eq!(BpskModulator::demodulate_hard(0.5), false);
    /// assert_eq!(BpskModulator::demodulate_hard(-0.5), true);
    /// assert_eq!(BpskModulator::demodulate_hard(0.0), false); // tie goes to 0
    /// ```
    pub fn demodulate_hard(symbol: f64) -> bool {
        symbol < 0.0
    }

    /// Converts a received symbol to an LLR given the noise variance.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::BpskModulator;
    ///
    /// let sigma_sq = 0.5;
    /// let llr = BpskModulator::to_llr(1.0, sigma_sq);
    /// assert!(llr.value() > 0.0); // Positive symbol suggests bit 0
    /// ```
    pub fn to_llr(received: f64, sigma_squared: f64) -> Llr {
        Llr::new(2.0 * received / sigma_squared)
    }
}

/// AWGN channel simulator.
///
/// Simulates transmission over an Additive White Gaussian Noise channel
/// with configurable signal-to-noise ratio.
pub struct AwgnChannel {
    sigma_squared: f64,
    noise_dist: Normal<f64>,
}

impl AwgnChannel {
    /// Creates a new AWGN channel from noise variance.
    ///
    /// # Panics
    ///
    /// Panics if `sigma_squared <= 0.0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// let channel = AwgnChannel::from_variance(0.5);
    /// ```
    pub fn from_variance(sigma_squared: f64) -> Self {
        assert!(sigma_squared > 0.0, "Noise variance must be positive");
        let noise_dist =
            Normal::new(0.0, sigma_squared.sqrt()).expect("Failed to create normal distribution");
        AwgnChannel {
            sigma_squared,
            noise_dist,
        }
    }

    /// Creates a new AWGN channel from Eb/N0 in dB and code rate.
    ///
    /// # Arguments
    ///
    /// * `eb_n0_db` - Energy per bit to noise power spectral density ratio in dB
    /// * `rate` - Code rate (k/n), where k is message length and n is codeword length
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// // Uncoded transmission (rate = 1.0) at 3 dB
    /// let channel = AwgnChannel::from_eb_n0_db(3.0, 1.0);
    /// ```
    pub fn from_eb_n0_db(eb_n0_db: f64, rate: f64) -> Self {
        assert!(rate > 0.0 && rate <= 1.0, "Code rate must be in (0, 1]");
        let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
        let sigma_squared = 1.0 / (2.0 * rate * eb_n0_linear);
        Self::from_variance(sigma_squared)
    }

    /// Returns the noise variance `sigma^2`.
    pub fn variance(&self) -> f64 {
        self.sigma_squared
    }

    /// Transmits a symbol through the channel, adding Gaussian noise.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// let mut channel = AwgnChannel::from_variance(0.5);
    /// let mut rng = rand::thread_rng();
    ///
    /// let transmitted = 1.0;
    /// let received = channel.transmit(transmitted, &mut rng);
    /// // Received symbol should be close to transmitted but with noise
    /// ```
    pub fn transmit<R: Rng>(&self, symbol: f64, rng: &mut R) -> f64 {
        symbol + self.noise_dist.sample(rng)
    }

    /// Transmits multiple symbols through the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// let mut channel = AwgnChannel::from_variance(0.5);
    /// let mut rng = rand::thread_rng();
    ///
    /// let symbols = vec![1.0, -1.0, 1.0];
    /// let received = channel.transmit_symbols(&symbols, &mut rng);
    /// assert_eq!(received.len(), 3);
    /// ```
    pub fn transmit_symbols<R: Rng>(&self, symbols: &[f64], rng: &mut R) -> Vec<f64> {
        symbols.iter().map(|&s| self.transmit(s, rng)).collect()
    }

    /// Converts received symbols to LLRs for soft-decision decoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// let channel = AwgnChannel::from_variance(0.5);
    /// let received = vec![0.8, -0.9, 0.1];
    /// let llrs = channel.to_llrs(&received);
    /// assert_eq!(llrs.len(), 3);
    /// ```
    pub fn to_llrs(&self, received: &[f64]) -> Vec<Llr> {
        received
            .iter()
            .map(|&r| BpskModulator::to_llr(r, self.sigma_squared))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpsk_modulate_zero() {
        assert_eq!(BpskModulator::modulate(false), 1.0);
    }

    #[test]
    fn test_bpsk_modulate_one() {
        assert_eq!(BpskModulator::modulate(true), -1.0);
    }

    #[test]
    fn test_bpsk_modulate_bits() {
        let bits = vec![false, true, false, true];
        let symbols = BpskModulator::modulate_bits(&bits);
        assert_eq!(symbols, vec![1.0, -1.0, 1.0, -1.0]);
    }

    #[test]
    fn test_bpsk_demodulate_hard_positive() {
        assert!(!BpskModulator::demodulate_hard(0.5));
    }

    #[test]
    fn test_bpsk_demodulate_hard_negative() {
        assert!(BpskModulator::demodulate_hard(-0.5));
    }

    #[test]
    fn test_bpsk_demodulate_hard_zero() {
        assert!(!BpskModulator::demodulate_hard(0.0));
    }

    #[test]
    fn test_bpsk_to_llr_positive_symbol() {
        let llr = BpskModulator::to_llr(1.0, 0.5);
        assert!(llr.value() > 0.0);
    }

    #[test]
    fn test_bpsk_to_llr_negative_symbol() {
        let llr = BpskModulator::to_llr(-1.0, 0.5);
        assert!(llr.value() < 0.0);
    }

    #[test]
    fn test_awgn_from_variance() {
        let channel = AwgnChannel::from_variance(0.5);
        assert_eq!(channel.variance(), 0.5);
    }

    #[test]
    #[should_panic(expected = "Noise variance must be positive")]
    fn test_awgn_from_variance_negative() {
        AwgnChannel::from_variance(-0.5);
    }

    #[test]
    fn test_awgn_from_eb_n0_db() {
        let channel = AwgnChannel::from_eb_n0_db(3.0, 1.0);
        // For uncoded (rate=1), sigma^2 = 1/(2*10^(Eb/N0_dB/10))
        let expected = 1.0 / (2.0 * 10.0_f64.powf(3.0 / 10.0));
        assert!((channel.variance() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_awgn_transmit_adds_noise() {
        let channel = AwgnChannel::from_variance(0.5);
        let mut rng = rand::thread_rng();

        let symbol = 1.0;
        let received = channel.transmit(symbol, &mut rng);

        // Received should be different from transmitted (with very high probability)
        // But we can't assert inequality due to randomness, so just check it's reasonable
        assert!(received.is_finite());
    }

    #[test]
    fn test_awgn_transmit_symbols() {
        let channel = AwgnChannel::from_variance(0.5);
        let mut rng = rand::thread_rng();

        let symbols = vec![1.0, -1.0, 1.0];
        let received = channel.transmit_symbols(&symbols, &mut rng);

        assert_eq!(received.len(), 3);
        assert!(received.iter().all(|&r| r.is_finite()));
    }

    #[test]
    fn test_awgn_to_llrs() {
        let channel = AwgnChannel::from_variance(0.5);
        let received = vec![0.8, -0.9, 0.1];
        let llrs = channel.to_llrs(&received);

        assert_eq!(llrs.len(), 3);
        assert!(llrs[0].value() > 0.0); // Positive symbol
        assert!(llrs[1].value() < 0.0); // Negative symbol
        assert!(llrs[2].value() > 0.0); // Small positive
    }

    #[test]
    fn test_roundtrip_no_noise() {
        let bits = vec![false, true, false, true, false];
        let symbols = BpskModulator::modulate_bits(&bits);
        let decoded: Vec<bool> = symbols
            .iter()
            .map(|&s| BpskModulator::demodulate_hard(s))
            .collect();
        assert_eq!(decoded, bits);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn bpsk_modulate_always_unit_magnitude(bit: bool) {
            let symbol = BpskModulator::modulate(bit);
            assert!((symbol.abs() - 1.0).abs() < 1e-10);
        }

        #[test]
        fn bpsk_roundtrip_no_noise(bits in prop::collection::vec(any::<bool>(), 0..100)) {
            let symbols = BpskModulator::modulate_bits(&bits);
            let decoded: Vec<bool> = symbols
                .iter()
                .map(|&s| BpskModulator::demodulate_hard(s))
                .collect();
            assert_eq!(decoded, bits);
        }

        #[test]
        fn awgn_variance_correct(eb_n0_db in 0.0..20.0, rate in 0.1..1.0) {
            let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, rate);
            let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
            let expected = 1.0 / (2.0 * rate * eb_n0_linear);
            prop_assert!((channel.variance() - expected).abs() < 1e-10);
        }

        #[test]
        fn llr_sign_matches_symbol_sign(received in -10.0..10.0, sigma_sq in 0.1..10.0) {
            let llr = BpskModulator::to_llr(received, sigma_sq);
            if received > 0.0 {
                prop_assert!(llr.value() > 0.0);
            } else if received < 0.0 {
                prop_assert!(llr.value() < 0.0);
            }
        }

        #[test]
        fn llr_magnitude_increases_with_signal(sigma_sq in 0.1..10.0) {
            let llr_weak = BpskModulator::to_llr(0.5, sigma_sq);
            let llr_strong = BpskModulator::to_llr(1.0, sigma_sq);
            prop_assert!(llr_strong.magnitude() > llr_weak.magnitude());
        }
    }
}
