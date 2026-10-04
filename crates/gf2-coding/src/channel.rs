//! AWGN noise sampler: [`AwgnChannel`] draws per-component real noise
//! `n ~ N(0, sigma^2)`. Modulation, demapping and LLR computation belong to
//! [`crate::modem`].

use rand::Rng;
use rand_distr::{Distribution, Normal};

/// Sampler of per-component real noise `n ~ N(0, sigma^2)`.
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
    pub fn from_variance(sigma_squared: f64) -> Self {
        assert!(sigma_squared > 0.0, "Noise variance must be positive");
        let noise_dist =
            Normal::new(0.0, sigma_squared.sqrt()).expect("Failed to create normal distribution");
        AwgnChannel {
            sigma_squared,
            noise_dist,
        }
    }

    /// Creates the channel for Eb/N0 in dB at code rate `rate` under the BPSK
    /// convention (one bit per real symbol, unit symbol energy):
    /// `sigma^2 = 1 / (2 R 10^{Eb/N0_dB / 10})`. For more than one bit per symbol
    /// use [`crate::modem::awgn_link::unit_energy_sigma_sq_from_eb_n0_db`].
    ///
    ///
    /// # Panics
    ///
    /// Panics if `rate` is not in `(0, 1]`.
    pub fn from_eb_n0_db(eb_n0_db: f64, rate: f64) -> Self {
        let sigma_squared =
            crate::modem::awgn_link::unit_energy_sigma_sq_from_eb_n0_db(1, rate, eb_n0_db);
        Self::from_variance(sigma_squared)
    }

    /// Returns the noise variance `sigma^2`.
    pub fn variance(&self) -> f64 {
        self.sigma_squared
    }

    /// Transmits a single real symbol through the channel, adding Gaussian noise.
    pub fn transmit<R: Rng>(&self, symbol: f64, rng: &mut R) -> f64 {
        symbol + self.noise_dist.sample(rng)
    }

    /// Transmits multiple real symbols through the channel.
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
