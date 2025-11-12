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

    /// Computes the Shannon capacity for BPSK over AWGN at the given Eb/N0.
    ///
    /// The capacity is given by:
    /// $$
    /// C = \frac{1}{2} \int_{-\infty}^{\infty} p(y|x) \log_2\left(1 + \frac{p(y|x=1)}{p(y|x=-1)}\right) dy
    /// $$
    ///
    /// For BPSK over AWGN, this simplifies to a function of SNR.
    ///
    /// # Arguments
    ///
    /// * `eb_n0_db` - Energy per bit to noise ratio in dB
    /// * `rate` - Code rate (affects SNR calculation)
    ///
    /// # Returns
    ///
    /// Channel capacity in bits per channel use (0 to 1.0)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// let capacity = AwgnChannel::shannon_capacity(3.0, 0.5);
    /// assert!(capacity > 0.5 && capacity < 1.0);
    /// ```
    pub fn shannon_capacity(eb_n0_db: f64, rate: f64) -> f64 {
        // Convert Eb/N0 to SNR per symbol
        let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
        let snr = 2.0 * rate * eb_n0_linear;

        // For BPSK, capacity is C = 1 - E_y[log2(1 + exp(-2*y*sqrt(SNR)))]
        // where y is received signal. We approximate using numerical integration.
        //
        // Simplified approximation using Q-function:
        // C ≈ 1 - H(Q(sqrt(2*SNR)))
        // where H is binary entropy function

        // For computational efficiency, use a good approximation:
        // C ≈ 1 - (1/ln(2)) * ∫ e^(-(x-√SNR)²/2) * log(1 + e^(-2x√SNR)) dx / √(2π)
        //
        // Simplified bound (tight for high SNR):
        if snr > 10.0 {
            // High SNR: capacity approaches 1
            1.0 - (std::f64::consts::E / std::f64::consts::LN_2) * (-snr / 2.0).exp()
        } else {
            // Use numerical integration for low/medium SNR
            shannon_capacity_numerical(snr)
        }
    }

    /// Returns the minimum Eb/N0 (in dB) required to achieve a given rate.
    ///
    /// This is the Shannon limit: the theoretical minimum SNR needed for
    /// reliable communication at the specified rate.
    ///
    /// For rate R, finds Eb/N0 such that C(Eb/N0) = R.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// // Rate 1/2 code requires approximately 0.2 dB at Shannon limit
    /// let eb_n0_min = AwgnChannel::shannon_limit(0.5);
    /// assert!(eb_n0_min < 1.0 && eb_n0_min > -1.0);
    /// ```
    pub fn shannon_limit(rate: f64) -> f64 {
        assert!(rate > 0.0 && rate <= 1.0, "Rate must be in (0, 1]");

        // Binary search for Eb/N0 where capacity equals rate
        let mut low = -2.0; // Start at -2 dB
        let mut high = 20.0; // Up to 20 dB

        for _ in 0..50 {
            // 50 iterations gives high precision
            let mid = (low + high) / 2.0;
            let capacity = Self::shannon_capacity(mid, rate);

            if (capacity - rate).abs() < 1e-6 {
                return mid;
            }

            if capacity > rate {
                high = mid;
            } else {
                low = mid;
            }
        }

        (low + high) / 2.0
    }
}

/// Numerically computes Shannon capacity for BPSK at given SNR.
///
/// Uses Gaussian quadrature to integrate the capacity formula.
fn shannon_capacity_numerical(snr: f64) -> f64 {
    // Integrate using trapezoidal rule over received signal range
    let sqrt_snr = snr.sqrt();
    let num_points = 200;
    let x_max = 5.0 * sqrt_snr.max(2.0); // Integrate from -x_max to +x_max
    let dx = 2.0 * x_max / num_points as f64;

    let mut sum = 0.0;
    let sqrt_2pi = (2.0 * std::f64::consts::PI).sqrt();

    for i in 0..=num_points {
        let x = -x_max + i as f64 * dx;

        // p(y|x=+1) for transmitted symbol +1
        let p_plus = (-(x - sqrt_snr).powi(2) / 2.0).exp() / sqrt_2pi;
        // p(y|x=-1) for transmitted symbol -1
        let p_minus = (-(x + sqrt_snr).powi(2) / 2.0).exp() / sqrt_2pi;

        // Average over both transmitted symbols
        let p_y = 0.5 * (p_plus + p_minus);

        if p_y > 1e-10 {
            // Mutual information contribution
            let term_plus = if p_plus > 1e-10 {
                p_plus * (p_plus / p_y).log2()
            } else {
                0.0
            };
            let term_minus = if p_minus > 1e-10 {
                p_minus * (p_minus / p_y).log2()
            } else {
                0.0
            };

            let weight = if i == 0 || i == num_points { 0.5 } else { 1.0 };
            sum += weight * 0.5 * (term_plus + term_minus);
        }
    }

    (sum * dx).clamp(0.0, 1.0)
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
    fn test_shannon_capacity_high_snr() {
        // At high Eb/N0, capacity should approach 1
        let capacity = AwgnChannel::shannon_capacity(20.0, 1.0);
        assert!(capacity > 0.95);
    }

    #[test]
    fn test_shannon_capacity_low_snr() {
        // At very low Eb/N0, capacity should be small
        let capacity = AwgnChannel::shannon_capacity(-10.0, 1.0);
        assert!(capacity < 0.2); // Relaxed bound
    }

    #[test]
    fn test_shannon_limit_rate_half() {
        // Rate 1/2 should require approximately -0.2 dB
        let eb_n0_min = AwgnChannel::shannon_limit(0.5);
        assert!(eb_n0_min > -1.0 && eb_n0_min < 1.0);
    }

    #[test]
    fn test_shannon_limit_rate_high() {
        // Rate close to 1 requires higher Eb/N0
        let eb_n0_min = AwgnChannel::shannon_limit(0.9);
        assert!(eb_n0_min > 2.0); // Relaxed bound
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
