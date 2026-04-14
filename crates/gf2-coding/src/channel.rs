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
//!
//! # Framework-backed compatibility surface
//!
//! [`BpskModulator`] is a thin compatibility shim over the shared modem
//! framework (see [`crate::modem`]). All bit-to-symbol mapping, demapping,
//! and LLR conversion delegate to a cached [`crate::modem::ReferenceMapper`]
//! and [`crate::modem::ReferenceSoftDemapper`] built from
//! [`crate::modem::ModemSpec::bpsk_with_scalar`]. There is no hand-rolled
//! `±1` BPSK math in this module.

use crate::llr::Llr;
use crate::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ReferenceMapper,
    ReferenceSoftDemapper,
};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::sync::OnceLock;

/// Lazily-initialised, process-wide BPSK reference mapper over `f64`.
///
/// The BPSK preset is constant, so the mapper can be shared across all
/// callers of [`BpskModulator`]. Construction is cheap, but caching keeps
/// per-call cost down to a single pointer dereference.
fn bpsk_mapper() -> &'static ReferenceMapper<f64> {
    static MAPPER: OnceLock<ReferenceMapper<f64>> = OnceLock::new();
    MAPPER.get_or_init(|| ReferenceMapper::new(ModemSpec::<f64>::bpsk_with_scalar()))
}

/// Lazily-initialised, process-wide BPSK reference soft demapper over `f64`.
///
/// Shared across all callers of [`BpskModulator::to_llr`] for the same
/// reason as [`bpsk_mapper`]. The demapper computes the exact log-MAP LLR
/// `LLR = 4 y / N0`, which for BPSK equals `2 y / sigma^2` — the legacy
/// closed form preserved by this compatibility surface.
fn bpsk_demapper() -> &'static ReferenceSoftDemapper<f64> {
    static DEMAP: OnceLock<ReferenceSoftDemapper<f64>> = OnceLock::new();
    DEMAP.get_or_init(|| ReferenceSoftDemapper::new(ModemSpec::<f64>::bpsk_with_scalar()))
}

/// BPSK (Binary Phase Shift Keying) modulator.
///
/// Maps bits to symbols: `false` (0) → +1.0, `true` (1) → -1.0
///
/// All methods on this type delegate to the shared modem framework
/// ([`crate::modem`]); no hand-rolled BPSK arithmetic lives in this
/// module. The framework spec is [`ModemSpec::bpsk_with_scalar::<f64>`].
pub struct BpskModulator;

impl BpskModulator {
    /// Modulates a single bit to a BPSK symbol.
    ///
    /// Delegates to the framework [`ReferenceMapper`] over
    /// [`ModemSpec::bpsk_with_scalar::<f64>`]. The framework's BPSK preset
    /// stores label `0` at `+1` and label `1` at `-1` on the I axis with
    /// `Q = 0`, so the returned scalar matches the legacy mapping.
    ///
    /// # Arguments
    ///
    /// * `bit` - Information bit; `false` is bit 0, `true` is bit 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::BpskModulator;
    ///
    /// assert_eq!(BpskModulator::modulate(false), 1.0);
    /// assert_eq!(BpskModulator::modulate(true), -1.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn modulate(bit: bool) -> f64 {
        let mut i = [0.0_f64; 1];
        let mut q = [0.0_f64; 1];
        bpsk_mapper().map_bits(&[bit], &mut i, &mut q);
        i[0]
    }

    /// Modulates a slice of bits to BPSK symbols.
    ///
    /// Delegates to the framework [`ReferenceMapper`] in a single batched
    /// call so the per-symbol cost matches a direct framework user.
    ///
    /// # Arguments
    ///
    /// * `bits` - Information bits, MSB-first (irrelevant for BPSK since
    ///   `bits_per_symbol = 1`, but the layout matches the framework
    ///   contract for consistency).
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
    ///
    /// # Complexity
    ///
    /// O(n) in `bits.len()`. Allocates two scratch vectors of length `n`.
    pub fn modulate_bits(bits: &[bool]) -> Vec<f64> {
        let n = bits.len();
        let mut out_i = vec![0.0_f64; n];
        let mut out_q = vec![0.0_f64; n];
        bpsk_mapper().map_bits(bits, &mut out_i, &mut out_q);
        out_i
    }

    /// Hard demodulates a symbol back to a bit.
    ///
    /// Implemented as `symbol < 0.0`; this matches taking the sign of the
    /// LLR produced by the framework demapper for the same input under
    /// any positive noise variance, since the BPSK closed form
    /// `LLR = 2 y / sigma^2` is sign-preserving in `y`.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Received real-valued sample on the I axis.
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
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn demodulate_hard(symbol: f64) -> bool {
        symbol < 0.0
    }

    /// Converts a received symbol to an LLR given the noise variance.
    ///
    /// Delegates to the framework [`ReferenceSoftDemapper`] using the
    /// shared noise convention `N0 = 2 * sigma_squared` (see the
    /// [`crate::modem`] module-level "Noise convention" docs). For BPSK
    /// this returns the closed-form value `2 * received / sigma_squared`
    /// converted to `f32`.
    ///
    /// # Arguments
    ///
    /// * `received` - Received I-axis sample.
    /// * `sigma_squared` - Per-component AWGN variance `sigma^2` (the
    ///   same quantity returned by [`AwgnChannel::variance`]).
    ///
    /// # Panics
    ///
    /// Panics if `sigma_squared <= 0.0` (the framework demapper rejects a
    /// non-positive `noise_var`).
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
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn to_llr(received: f64, sigma_squared: f64) -> Llr {
        let rx_i = [received];
        let rx_q = [0.0_f64];
        let n0 = [2.0 * sigma_squared];
        let mut out = [Llr::new(0.0); 1];
        let input = DemapInput::<f64> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &n0,
            method: DemapMethod::ExactLogMap,
        };
        bpsk_demapper().demap_llrs(input, &mut out);
        out[0]
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

    /// Converts received BPSK symbols to LLRs for soft-decision decoding.
    ///
    /// Each entry is computed by [`BpskModulator::to_llr`], which routes
    /// through the shared modem framework's
    /// [`crate::modem::ReferenceSoftDemapper`] over
    /// [`crate::modem::ModemSpec::bpsk_with_scalar`]. No bespoke BPSK
    /// LLR formula lives in this method.
    ///
    /// # Arguments
    ///
    /// * `received` - Received I-axis samples, one per transmitted BPSK
    ///   symbol.
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
    ///
    /// # Complexity
    ///
    /// O(n) in `received.len()`.
    pub fn to_llrs(&self, received: &[f64]) -> Vec<Llr> {
        received
            .iter()
            .map(|&r| BpskModulator::to_llr(r, self.sigma_squared))
            .collect()
    }

    /// Computes the Shannon capacity for BPSK over AWGN at the given Eb/N0.
    ///
    /// For BPSK modulation, the channel capacity in bits per channel use is:
    /// $$
    /// C = 1 - \int_{-\infty}^{\infty} p(y) \log_2\left(\frac{1}{p(y|+1) + p(y|-1)}\right) dy
    /// $$
    ///
    /// where $p(y|x)$ is the Gaussian conditional density.
    ///
    /// # Arguments
    ///
    /// * `eb_n0_db` - Energy per bit to noise ratio in dB (= Es/N0 for BPSK)
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
    /// let capacity = AwgnChannel::shannon_capacity(3.0);
    /// assert!(capacity > 0.7 && capacity < 0.8);
    /// ```
    pub fn shannon_capacity(eb_n0_db: f64) -> f64 {
        // Convert Eb/N0 to linear scale
        // For BPSK: Es/N0 = Eb/N0 (one bit per symbol)
        let snr = 10.0_f64.powf(eb_n0_db / 10.0);

        // Use numerical integration for all cases for consistency
        // The high SNR approximation was causing non-monotonic behavior
        shannon_capacity_numerical(snr)
    }

    /// Returns the minimum Eb/N0 (in dB) required to achieve a given rate.
    ///
    /// This is the Shannon limit: the theoretical minimum SNR needed for
    /// reliable communication at the specified rate over a BPSK AWGN channel.
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
        let mut low = -10.0; // Start at -10 dB for very low rates
        let mut high = 25.0; // Up to 25 dB for rates near 1

        for _ in 0..60 {
            // 60 iterations for better precision
            let mid = (low + high) / 2.0;
            let capacity = Self::shannon_capacity(mid);

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

/// Numerically computes Shannon capacity for BPSK at given SNR (Eb/N0).
///
/// For BPSK modulation over AWGN, the capacity is:
/// C = 1 - integral_{-inf}^{inf} f(y) log2(1 + exp(-2*sqrt(SNR)*y)) dy
/// where f(y) is N(sqrt(SNR), 1) distribution and SNR = Eb/N0
fn shannon_capacity_numerical(eb_n0_linear: f64) -> f64 {
    let sqrt_snr = eb_n0_linear.sqrt();

    // Use numerical integration over the received signal
    // For BPSK with transmitted symbol +/-sqrt(SNR), noise variance = 1
    let num_points = 1000;
    let y_max = sqrt_snr + 6.0; // Cover mean ± 6 standard deviations
    let dy = 2.0 * y_max / num_points as f64;

    let sqrt_2pi = (2.0 * std::f64::consts::PI).sqrt();
    let mut integral = 0.0;

    for i in 0..=num_points {
        let y = -y_max + i as f64 * dy;

        // PDF of received signal when sending +sqrt(SNR): N(sqrt(SNR), 1)
        let f_y = (-(y - sqrt_snr).powi(2) / 2.0).exp() / sqrt_2pi;

        // log2(1 + exp(-2*sqrt(SNR)*y))
        let arg = -2.0 * sqrt_snr * y;
        let log_term = if arg > 20.0 {
            // For large positive arg, 1 + exp(arg) ≈ exp(arg)
            arg / std::f64::consts::LN_2
        } else if arg < -20.0 {
            // For large negative arg, 1 + exp(arg) ≈ 1
            0.0
        } else {
            (1.0 + arg.exp()).log2()
        };

        let weight = if i == 0 || i == num_points { 0.5 } else { 1.0 };
        integral += weight * f_y * log_term;
    }

    let capacity = 1.0 - integral * dy;
    capacity.clamp(0.0, 1.0)
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
        let capacity = AwgnChannel::shannon_capacity(20.0);
        assert!(capacity > 0.95);
    }

    #[test]
    fn test_shannon_capacity_low_snr() {
        // At very low Eb/N0, capacity should be small
        let capacity = AwgnChannel::shannon_capacity(-10.0);
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

    /// Compatibility regression: the legacy [`BpskModulator::modulate_bits`]
    /// surface must produce bit-for-bit identical symbols to the framework
    /// [`ReferenceMapper`] over [`ModemSpec::bpsk_with_scalar`]. Because
    /// BPSK is integer-clean (`±1` exactly) this is an exact equality
    /// check, not a tolerance check.
    #[test]
    fn test_bpsk_modulator_matches_framework() {
        use crate::modem::{BatchMapper, ModemSpec, ReferenceMapper};
        let bits: Vec<bool> = (0..32).map(|i| (i * 5 + 3) & 1 == 1).collect();
        let legacy = BpskModulator::modulate_bits(&bits);

        let mapper = ReferenceMapper::new(ModemSpec::<f64>::bpsk_with_scalar());
        let mut fi = vec![0.0_f64; bits.len()];
        let mut fq = vec![0.0_f64; bits.len()];
        mapper.map_bits(&bits, &mut fi, &mut fq);

        assert_eq!(legacy, fi);
        for q in fq {
            assert_eq!(q, 0.0);
        }
    }

    /// Compatibility regression: the legacy [`BpskAwgnChannel`] surface
    /// must produce identical received samples and LLRs to a freshly
    /// composed [`crate::modem::ModemAwgnChannel`] over
    /// [`ModemSpec::bpsk_with_scalar`] **when the same Gaussian noise
    /// samples are applied to the I axis**. Because the framework
    /// [`crate::modem::ModemAwgnChannel`] additionally draws Q-axis noise
    /// (which BPSK ignores in its LLR), we replicate the legacy 1-D
    /// pipeline by applying [`AwgnChannel::transmit_symbols`] manually
    /// then handing the same received vector to a framework demap call.
    /// This isolates the comparison to the modulation/demapping math.
    #[test]
    fn test_bpsk_awgn_channel_matches_modem_awgn_channel() {
        use crate::modem::{
            BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ReferenceMapper,
            ReferenceSoftDemapper,
        };
        use rand::SeedableRng;

        let bits: Vec<bool> = (0..16).map(|i| (i & 3) >= 2).collect();
        let channel = AwgnChannel::from_variance(0.4);

        // Legacy path.
        let mut rng_legacy = rand::rngs::StdRng::seed_from_u64(0xDEAD_BEEF);
        let legacy_symbols = BpskModulator::modulate_bits(&bits);
        let legacy_rx = channel.transmit_symbols(&legacy_symbols, &mut rng_legacy);
        let legacy_llrs = channel.to_llrs(&legacy_rx);

        // Framework path: same RNG seed, same I-axis noise sequence.
        let mut rng_framework = rand::rngs::StdRng::seed_from_u64(0xDEAD_BEEF);
        let mapper = ReferenceMapper::new(ModemSpec::<f64>::bpsk_with_scalar());
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f64>::bpsk_with_scalar());
        let mut fi = vec![0.0_f64; bits.len()];
        let mut fq = vec![0.0_f64; bits.len()];
        mapper.map_bits(&bits, &mut fi, &mut fq);
        // Apply the same noise sequence to I; Q stays at zero so it is
        // semantically equivalent to the legacy 1-D AWGN application.
        let framework_rx = channel.transmit_symbols(&fi, &mut rng_framework);
        let n0 = vec![2.0 * channel.variance(); bits.len()];
        let mut framework_llrs = vec![Llr::new(0.0); bits.len()];
        let input = DemapInput::<f64> {
            rx_i: &framework_rx,
            rx_q: &fq,
            gain_i: None,
            gain_q: None,
            noise_var: &n0,
            method: DemapMethod::ExactLogMap,
        };
        demap.demap_llrs(input, &mut framework_llrs);

        // Received samples are bit-for-bit identical (same RNG path).
        assert_eq!(legacy_rx, framework_rx);
        // LLRs match within f32 round-off (both compute 2 r / sigma^2 in
        // f64 and cast to f32 at the end).
        for (a, b) in legacy_llrs.iter().zip(framework_llrs.iter()) {
            let diff = (a.value() - b.value()).abs();
            assert!(
                diff < 1e-4 * a.value().abs().max(1.0),
                "legacy {} vs framework {}",
                a.value(),
                b.value()
            );
        }
    }

    /// Regression guard for the BPSK LLR sign convention: positive LLR
    /// must mean "bit 0 is more likely" and negative LLR must mean
    /// "bit 1 is more likely", at every SNR. Bit 0 maps to `+1` in the
    /// shared modem framework, so a positive received sample must
    /// produce a positive LLR.
    #[test]
    fn test_bpsk_awgn_channel_llr_sign_convention() {
        for sigma_sq in [0.05_f64, 0.5, 2.0, 10.0] {
            // Strongly positive sample: bit 0 most likely → positive LLR.
            let llr_pos = BpskModulator::to_llr(1.0, sigma_sq);
            assert!(
                llr_pos.value() > 0.0,
                "sigma^2={sigma_sq}: positive sample must give positive LLR, got {}",
                llr_pos.value()
            );
            // Strongly negative sample: bit 1 most likely → negative LLR.
            let llr_neg = BpskModulator::to_llr(-1.0, sigma_sq);
            assert!(
                llr_neg.value() < 0.0,
                "sigma^2={sigma_sq}: negative sample must give negative LLR, got {}",
                llr_neg.value()
            );
        }
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
        fn awgn_variance_correct(eb_n0_db in 0.0f64..20.0f64, rate in 0.1f64..1.0f64) {
            let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, rate);
            let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
            let expected = 1.0 / (2.0 * rate * eb_n0_linear);
            prop_assert!((channel.variance() - expected).abs() < 1e-10);
        }

        #[test]
        fn llr_sign_matches_symbol_sign(received in -10.0f64..10.0f64, sigma_sq in 0.1f64..10.0f64) {
            let llr = BpskModulator::to_llr(received, sigma_sq);
            if received > 0.0 {
                prop_assert!(llr.value() > 0.0);
            } else if received < 0.0 {
                prop_assert!(llr.value() < 0.0);
            }
        }

        #[test]
        fn llr_magnitude_increases_with_signal(sigma_sq in 0.1f64..10.0f64) {
            let llr_weak = BpskModulator::to_llr(0.5, sigma_sq);
            let llr_strong = BpskModulator::to_llr(1.0, sigma_sq);
            prop_assert!(llr_strong.magnitude() > llr_weak.magnitude());
        }

        /// Parity property: for any random bit vector and any AWGN
        /// variance, [`BpskAwgnChannel`]-style I-axis transmit followed
        /// by [`AwgnChannel::to_llrs`] (legacy surface) and an equivalent
        /// framework [`crate::modem::ReferenceMapper`] +
        /// [`crate::modem::ReferenceSoftDemapper`] composition produce
        /// identical received samples and identical LLRs (within f32
        /// round-off), regardless of seed.
        #[test]
        fn bpsk_awgn_channel_matches_modem_awgn_channel_random(
            bits in prop::collection::vec(any::<bool>(), 0..64),
            sigma_sq in 0.05f64..3.0f64,
            seed in any::<u64>(),
        ) {
            use crate::modem::{
                BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec,
                ReferenceMapper, ReferenceSoftDemapper,
            };
            use rand::SeedableRng;

            let channel = AwgnChannel::from_variance(sigma_sq);

            let mut rng_legacy = rand::rngs::StdRng::seed_from_u64(seed);
            let legacy_symbols = BpskModulator::modulate_bits(&bits);
            let legacy_rx = channel.transmit_symbols(&legacy_symbols, &mut rng_legacy);
            let legacy_llrs = channel.to_llrs(&legacy_rx);

            let mut rng_framework = rand::rngs::StdRng::seed_from_u64(seed);
            let mapper = ReferenceMapper::new(ModemSpec::<f64>::bpsk_with_scalar());
            let demap = ReferenceSoftDemapper::new(ModemSpec::<f64>::bpsk_with_scalar());
            let mut fi = vec![0.0_f64; bits.len()];
            let mut fq = vec![0.0_f64; bits.len()];
            mapper.map_bits(&bits, &mut fi, &mut fq);
            let framework_rx = channel.transmit_symbols(&fi, &mut rng_framework);
            let n0 = vec![2.0 * channel.variance(); bits.len()];
            let mut framework_llrs = vec![Llr::new(0.0); bits.len()];
            let input = DemapInput::<f64> {
                rx_i: &framework_rx,
                rx_q: &fq,
                gain_i: None,
                gain_q: None,
                noise_var: &n0,
                method: DemapMethod::ExactLogMap,
            };
            demap.demap_llrs(input, &mut framework_llrs);

            prop_assert_eq!(&legacy_rx, &framework_rx);
            for (a, b) in legacy_llrs.iter().zip(framework_llrs.iter()) {
                let tol = 1e-4_f32 * a.value().abs().max(1.0);
                prop_assert!(
                    (a.value() - b.value()).abs() <= tol,
                    "legacy {} vs framework {} (tol {})",
                    a.value(),
                    b.value(),
                    tol
                );
            }
        }
    }
}
