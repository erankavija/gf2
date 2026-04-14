//! AWGN (Additive White Gaussian Noise) channel noise sampler.
//!
//! # Overview
//!
//! This module provides [`AwgnChannel`], a thin wrapper around a Gaussian
//! RNG configured for a particular per-component noise variance `sigma^2`.
//! It is the noise source used by the modem framework's AWGN link adapter
//! (see [`crate::modem::awgn_link`]) and by the legacy BPSK simulation
//! channel [`crate::simulation::BpskAwgnChannel`].
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
//! # What does NOT live here
//!
//! Modulation, demapping, and LLR computation are the exclusive
//! responsibility of the modem framework at
//! [`crate::modem`]. Shannon-capacity / Shannon-limit information-theory
//! utilities moved to [`crate::info_theory`].

use rand::Rng;
use rand_distr::{Distribution, Normal};

/// AWGN channel noise sampler.
///
/// Samples per-component real-valued noise `n ~ N(0, sigma^2)`. Used by
/// the modem framework's AWGN link and the legacy BPSK simulation channel
/// as the underlying Gaussian source.
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

    /// Creates a new AWGN channel from Eb/N0 in dB and code rate, using
    /// the BPSK convention (one bit per real symbol, unit symbol energy).
    ///
    /// For BPSK at code rate `R`, `sigma^2 = 1 / (2 R 10^{Eb/N0_dB / 10})`.
    /// For modulation schemes with more than one bit per symbol use
    /// [`crate::modem::awgn_link::unit_energy_sigma_sq_from_eb_n0_db`]
    /// directly — that helper accepts `m = bits_per_symbol` and is the
    /// single source of truth for Eb/N0 -> noise conversion across the
    /// modem framework.
    ///
    /// # Arguments
    ///
    /// * `eb_n0_db` - Energy per bit to noise power spectral density ratio in dB
    /// * `rate` - Code rate (k/n), where k is message length and n is codeword length
    ///
    /// # Panics
    ///
    /// Panics if `rate` is not in `(0, 1]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// // Uncoded BPSK (rate = 1.0) at 3 dB
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

    /// Transmits a single real symbol through the channel, adding Gaussian noise.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// let channel = AwgnChannel::from_variance(0.5);
    /// let mut rng = rand::thread_rng();
    /// let received = channel.transmit(1.0, &mut rng);
    /// assert!(received.is_finite());
    /// ```
    pub fn transmit<R: Rng>(&self, symbol: f64, rng: &mut R) -> f64 {
        symbol + self.noise_dist.sample(rng)
    }

    /// Transmits multiple real symbols through the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    ///
    /// let channel = AwgnChannel::from_variance(0.5);
    /// let mut rng = rand::thread_rng();
    /// let symbols = vec![1.0, -1.0, 1.0];
    /// let received = channel.transmit_symbols(&symbols, &mut rng);
    /// assert_eq!(received.len(), 3);
    /// ```
    pub fn transmit_symbols<R: Rng>(&self, symbols: &[f64], rng: &mut R) -> Vec<f64> {
        symbols.iter().map(|&s| self.transmit(s, rng)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let received = channel.transmit(1.0, &mut rng);
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

    #[cfg(test)]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn awgn_variance_correct(eb_n0_db in 0.0f64..20.0f64, rate in 0.1f64..1.0f64) {
                let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, rate);
                let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
                let expected = 1.0 / (2.0 * rate * eb_n0_linear);
                prop_assert!((channel.variance() - expected).abs() < 1e-10);
            }
        }
    }
}
