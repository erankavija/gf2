//! Rician fading channel model and random interleaver.
//!
//! # Overview
//!
//! This module implements:
//! - Block Rician fading channel model for wireless simulations
//! - Random bit interleaver for coded QPSK transmissions
//!
//! # Rician Fading Model
//!
//! The Rician fading channel gain for each coherence block is:
//! ```text
//! H_Ri = sqrt(K/(K+1)) + sqrt(1/(K+1)) · H_Ra
//! ```
//! where `H_Ra ~ CN(0, 2·σ²)` with `σ² = 0.5`, so `E[|H_Ri|²] = 1`.
//!
//! The Rician K-factor controls the ratio of the deterministic component
//! (LOS path) to the random scattered component power.
//!
//! # Block Structure
//!
//! Each frame contains `N = 2·t·N_c` bits, structured as `t` coherence blocks
//! of `N_c` QPSK symbols each (`2·N_c` bits per block).
//!
//! # Supported Configurations (from published paper)
//!
//! | Figure | K  | N_c | t  |
//! |--------|----|-----|----|
//! | Fig 8  | 5  | 128 | 4  |
//! | Fig 9  | 8  | 256 | 2  |
//! | Fig 10 | 6  | 256 | 8  |

use crate::modulation::Complex;
use rand::Rng;
use rand_distr::{Distribution, Normal};

/// Rician fading configuration for one of the three paper configurations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RicianConfig {
    /// Rician K-factor (ratio of LOS to scatter power).
    pub k_factor: f64,
    /// Coherence block size in QPSK symbols.
    pub coherence_block: usize,
    /// Number of coherence blocks per frame (taps).
    pub taps: usize,
}

impl RicianConfig {
    /// Configuration from Fig 8 of the GRAND paper: K=5, N_c=128, t=4.
    ///
    /// Frame length: N = 2·4·128 = 1024 bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::RicianConfig;
    ///
    /// let cfg = RicianConfig::fig8();
    /// assert_eq!(cfg.k_factor, 5.0);
    /// assert_eq!(cfg.frame_bits(), 1024);
    /// ```
    pub fn fig8() -> Self {
        RicianConfig {
            k_factor: 5.0,
            coherence_block: 128,
            taps: 4,
        }
    }

    /// Configuration from Fig 9 of the GRAND paper: K=8, N_c=256, t=2.
    ///
    /// Frame length: N = 2·2·256 = 1024 bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::RicianConfig;
    ///
    /// let cfg = RicianConfig::fig9();
    /// assert_eq!(cfg.k_factor, 8.0);
    /// assert_eq!(cfg.frame_bits(), 1024);
    /// ```
    pub fn fig9() -> Self {
        RicianConfig {
            k_factor: 8.0,
            coherence_block: 256,
            taps: 2,
        }
    }

    /// Configuration from Fig 10 of the GRAND paper: K=6, N_c=256, t=8.
    ///
    /// Frame length: N = 2·8·256 = 4096 bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::RicianConfig;
    ///
    /// let cfg = RicianConfig::fig10();
    /// assert_eq!(cfg.k_factor, 6.0);
    /// assert_eq!(cfg.frame_bits(), 4096);
    /// ```
    pub fn fig10() -> Self {
        RicianConfig {
            k_factor: 6.0,
            coherence_block: 256,
            taps: 8,
        }
    }

    /// Returns the total number of bits per frame: `N = 2·t·N_c`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::RicianConfig;
    ///
    /// let cfg = RicianConfig::fig8();
    /// assert_eq!(cfg.frame_bits(), 1024);
    /// ```
    pub fn frame_bits(&self) -> usize {
        2 * self.taps * self.coherence_block
    }

    /// Returns the number of QPSK symbols per frame.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::RicianConfig;
    ///
    /// let cfg = RicianConfig::fig8();
    /// assert_eq!(cfg.frame_symbols(), 512);
    /// ```
    pub fn frame_symbols(&self) -> usize {
        self.taps * self.coherence_block
    }
}

/// Block Rician fading channel simulator.
///
/// Generates complex channel gains following the Rician fading model.
/// Within each coherence block, the channel gain is constant (block fading).
/// Across blocks, gains are i.i.d.
///
/// # Channel Model
///
/// ```text
/// H_Ri = sqrt(K/(K+1)) + sqrt(1/(K+1)) · (X + jY)
/// ```
/// where `X, Y ~ N(0, σ² = 0.5)` i.i.d., ensuring `E[|H_Ri|²] = 1`.
pub struct RicianChannel {
    config: RicianConfig,
    /// sqrt(K / (K+1)) — LOS amplitude
    los_amplitude: f64,
    /// sqrt(1 / (K+1)) — scatter amplitude scaling
    scatter_scale: f64,
    /// Normal distribution for real and imaginary scatter components
    scatter_dist: Normal<f64>,
}

impl RicianChannel {
    /// Creates a new Rician fading channel with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Rician channel configuration (K-factor, block size, taps)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{RicianChannel, RicianConfig};
    ///
    /// let channel = RicianChannel::new(RicianConfig::fig8());
    /// ```
    pub fn new(config: RicianConfig) -> Self {
        let k = config.k_factor;
        let los_amplitude = (k / (k + 1.0)).sqrt();
        let scatter_scale = (1.0 / (k + 1.0)).sqrt();
        // sigma^2 = 0.5, so sigma = 1/sqrt(2)
        let scatter_dist =
            Normal::new(0.0, (0.5_f64).sqrt()).expect("Failed to create normal distribution");
        RicianChannel {
            config,
            los_amplitude,
            scatter_scale,
            scatter_dist,
        }
    }

    /// Returns the channel configuration.
    pub fn config(&self) -> &RicianConfig {
        &self.config
    }

    /// Generates a single Rician fading channel coefficient.
    ///
    /// Returns `H_Ri = sqrt(K/(K+1)) + sqrt(1/(K+1))·(X + jY)` where
    /// `X, Y ~ N(0, 0.5)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{RicianChannel, RicianConfig};
    ///
    /// let channel = RicianChannel::new(RicianConfig::fig8());
    /// let mut rng = rand::thread_rng();
    /// let h = channel.sample_coefficient(&mut rng);
    /// // Coefficient is finite
    /// assert!(h.re.is_finite() && h.im.is_finite());
    /// ```
    pub fn sample_coefficient<R: Rng>(&self, rng: &mut R) -> Complex {
        let x = self.scatter_dist.sample(rng);
        let y = self.scatter_dist.sample(rng);
        Complex::new(
            self.los_amplitude + self.scatter_scale * x,
            self.scatter_scale * y,
        )
    }

    /// Generates channel coefficients for one complete frame.
    ///
    /// Returns one coefficient per coherence block, repeated for all symbols
    /// within that block. The output has `frame_symbols()` entries, where each
    /// group of `coherence_block` consecutive entries shares the same coefficient.
    ///
    /// # Arguments
    ///
    /// * `rng` - Random number generator
    ///
    /// # Returns
    ///
    /// A `Vec<Complex>` of length `frame_symbols()`.
    ///
    /// # Complexity
    ///
    /// O(t·N_c) where t is the number of taps and N_c is the coherence block size.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{RicianChannel, RicianConfig};
    ///
    /// let channel = RicianChannel::new(RicianConfig::fig8());
    /// let mut rng = rand::thread_rng();
    /// let gains = channel.generate_frame_gains(&mut rng);
    /// assert_eq!(gains.len(), channel.config().frame_symbols());
    /// ```
    pub fn generate_frame_gains<R: Rng>(&self, rng: &mut R) -> Vec<Complex> {
        let mut gains = Vec::with_capacity(self.config.frame_symbols());
        for _ in 0..self.config.taps {
            let h = self.sample_coefficient(rng);
            for _ in 0..self.config.coherence_block {
                gains.push(h);
            }
        }
        gains
    }

    /// Transmits QPSK symbols through the Rician fading channel with AWGN.
    ///
    /// Applies the block fading channel: `y_k = h_block · x_k + n_k` where
    /// `n_k ~ CN(0, σ²)` (i.e., real and imaginary noise each ~ N(0, σ²/2)).
    ///
    /// # Arguments
    ///
    /// * `symbols` - Transmitted QPSK symbols
    /// * `channel_gains` - Per-symbol channel gains (same length as `symbols`)
    /// * `sigma_squared` - Total noise variance σ² per complex dimension
    /// * `rng` - Random number generator
    ///
    /// # Panics
    ///
    /// Panics if `symbols` and `channel_gains` have different lengths.
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of symbols.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{RicianChannel, RicianConfig};
    /// use gf2_coding::modulation::{Complex, QpskModulator};
    ///
    /// let channel = RicianChannel::new(RicianConfig::fig8());
    /// let mut rng = rand::thread_rng();
    /// let qpsk = QpskModulator::new(1.0);
    /// let bits = vec![false; 8];
    /// let symbols = qpsk.modulate_bits(&bits);
    /// let gains = vec![Complex::new(1.0, 0.0); symbols.len()];
    /// let received = channel.transmit(&symbols, &gains, 0.1, &mut rng);
    /// assert_eq!(received.len(), symbols.len());
    /// ```
    pub fn transmit<R: Rng>(
        &self,
        symbols: &[Complex],
        channel_gains: &[Complex],
        sigma_squared: f64,
        rng: &mut R,
    ) -> Vec<Complex> {
        assert_eq!(
            symbols.len(),
            channel_gains.len(),
            "symbols and channel_gains must have equal length"
        );
        // Each complex noise sample has variance sigma^2/2 per component
        let noise_std = (sigma_squared / 2.0).sqrt();
        let noise_dist = Normal::new(0.0, noise_std).expect("Failed to create noise distribution");

        symbols
            .iter()
            .zip(channel_gains.iter())
            .map(|(&x, &h)| {
                let hx = h * x;
                let n_re = noise_dist.sample(rng);
                let n_im = noise_dist.sample(rng);
                Complex::new(hx.re + n_re, hx.im + n_im)
            })
            .collect()
    }
}

/// Random bit interleaver/de-interleaver.
///
/// Permutes coded bits before QPSK symbol mapping to spread burst errors.
/// The permutation is a uniformly random shuffle seeded for reproducibility.
///
/// # Usage
///
/// ```
/// use gf2_coding::fading::BitInterleaver;
///
/// let interleaver = BitInterleaver::new(1024, 42);
/// let bits = vec![false, true, false, false, true, false, true, true];
/// // Only works for len == 1024 in this example, use small size for demo
/// let interleaver = BitInterleaver::new(8, 42);
/// let interleaved = interleaver.interleave(&bits);
/// let deinterleaved = interleaver.deinterleave(&interleaved);
/// assert_eq!(deinterleaved, bits);
/// ```
pub struct BitInterleaver {
    /// Forward permutation: `perm[i]` is where bit `i` goes.
    perm: Vec<usize>,
    /// Inverse permutation: `inv_perm[j]` is where bit `j` came from.
    inv_perm: Vec<usize>,
}

impl BitInterleaver {
    /// Creates a new random bit interleaver for the given block length and seed.
    ///
    /// The permutation is deterministic given `length` and `seed`, ensuring
    /// the transmitter and receiver use the same permutation.
    ///
    /// # Arguments
    ///
    /// * `length` - Number of bits to interleave (must be > 0)
    /// * `seed` - Seed for the random permutation generator
    ///
    /// # Panics
    ///
    /// Panics if `length == 0`.
    ///
    /// # Complexity
    ///
    /// O(n) construction time and memory.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::BitInterleaver;
    ///
    /// let interleaver = BitInterleaver::new(64, 12345);
    /// ```
    pub fn new(length: usize, seed: u64) -> Self {
        assert!(length > 0, "Interleaver length must be positive");
        let perm = generate_permutation(length, seed);
        let mut inv_perm = vec![0usize; length];
        for (i, &p) in perm.iter().enumerate() {
            inv_perm[p] = i;
        }
        BitInterleaver { perm, inv_perm }
    }

    /// Returns the block length this interleaver was built for.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::BitInterleaver;
    ///
    /// let interleaver = BitInterleaver::new(128, 0);
    /// assert_eq!(interleaver.len(), 128);
    /// ```
    pub fn len(&self) -> usize {
        self.perm.len()
    }

    /// Returns `true` if the interleaver length is zero (never true in practice).
    pub fn is_empty(&self) -> bool {
        self.perm.is_empty()
    }

    /// Interleaves a slice of bits according to the permutation.
    ///
    /// Output bit at position `perm[i]` is input bit `i`.
    ///
    /// # Arguments
    ///
    /// * `bits` - Input bit slice of length `self.len()`
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::BitInterleaver;
    ///
    /// let interleaver = BitInterleaver::new(4, 7);
    /// let bits = vec![true, false, true, false];
    /// let out = interleaver.interleave(&bits);
    /// assert_eq!(out.len(), 4);
    /// ```
    pub fn interleave(&self, bits: &[bool]) -> Vec<bool> {
        assert_eq!(
            bits.len(),
            self.perm.len(),
            "Input length {} does not match interleaver length {}",
            bits.len(),
            self.perm.len()
        );
        let mut out = vec![false; bits.len()];
        for (i, &bit) in bits.iter().enumerate() {
            out[self.perm[i]] = bit;
        }
        out
    }

    /// De-interleaves a slice of bits, reversing the interleaving permutation.
    ///
    /// Output bit at position `i` is input bit `perm[i]` (inverse permutation).
    ///
    /// # Arguments
    ///
    /// * `bits` - Interleaved bit slice of length `self.len()`
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::BitInterleaver;
    ///
    /// let interleaver = BitInterleaver::new(4, 7);
    /// let bits = vec![true, false, true, false];
    /// let interleaved = interleaver.interleave(&bits);
    /// let recovered = interleaver.deinterleave(&interleaved);
    /// assert_eq!(recovered, bits);
    /// ```
    pub fn deinterleave(&self, bits: &[bool]) -> Vec<bool> {
        assert_eq!(
            bits.len(),
            self.inv_perm.len(),
            "Input length {} does not match interleaver length {}",
            bits.len(),
            self.inv_perm.len()
        );
        let mut out = vec![false; bits.len()];
        for (j, &bit) in bits.iter().enumerate() {
            out[self.inv_perm[j]] = bit;
        }
        out
    }

    /// Interleaves LLR values using the same permutation as bits.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Input LLR slice of length `self.len()`
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::BitInterleaver;
    /// use gf2_coding::llr::Llr;
    ///
    /// let interleaver = BitInterleaver::new(4, 7);
    /// let llrs = vec![Llr::new(1.0), Llr::new(-2.0), Llr::new(3.0), Llr::new(-0.5)];
    /// let out = interleaver.interleave_llrs(&llrs);
    /// assert_eq!(out.len(), 4);
    /// ```
    pub fn interleave_llrs(&self, llrs: &[crate::llr::Llr]) -> Vec<crate::llr::Llr> {
        assert_eq!(
            llrs.len(),
            self.perm.len(),
            "Input length {} does not match interleaver length {}",
            llrs.len(),
            self.perm.len()
        );
        let mut out = vec![crate::llr::Llr::new(0.0); llrs.len()];
        for (i, &llr) in llrs.iter().enumerate() {
            out[self.perm[i]] = llr;
        }
        out
    }

    /// De-interleaves LLR values, reversing the interleaving permutation.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Interleaved LLR slice of length `self.len()`
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != self.len()`.
    ///
    /// # Complexity
    ///
    /// O(n).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::BitInterleaver;
    /// use gf2_coding::llr::Llr;
    ///
    /// let interleaver = BitInterleaver::new(4, 7);
    /// let llrs = vec![Llr::new(1.0), Llr::new(-2.0), Llr::new(3.0), Llr::new(-0.5)];
    /// let interleaved = interleaver.interleave_llrs(&llrs);
    /// let recovered = interleaver.deinterleave_llrs(&interleaved);
    /// for (a, b) in llrs.iter().zip(recovered.iter()) {
    ///     assert!((a.value() - b.value()).abs() < 1e-6);
    /// }
    /// ```
    pub fn deinterleave_llrs(&self, llrs: &[crate::llr::Llr]) -> Vec<crate::llr::Llr> {
        assert_eq!(
            llrs.len(),
            self.inv_perm.len(),
            "Input length {} does not match interleaver length {}",
            llrs.len(),
            self.inv_perm.len()
        );
        let mut out = vec![crate::llr::Llr::new(0.0); llrs.len()];
        for (j, &llr) in llrs.iter().enumerate() {
            out[self.inv_perm[j]] = llr;
        }
        out
    }
}

/// Generates a random permutation of `[0, length)` using a seeded LCG.
///
/// Uses a simple Fisher-Yates shuffle with a linear congruential generator
/// for a deterministic, seed-reproducible permutation without external deps.
fn generate_permutation(length: usize, seed: u64) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..length).collect();
    // LCG parameters from Knuth TAOCP Vol 2, suitable for 64-bit
    let mut state = seed.wrapping_add(1);
    for i in (1..length).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (state >> 33) as usize % (i + 1);
        perm.swap(i, j);
    }
    perm
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- RicianConfig ----

    #[test]
    fn test_rician_config_fig8_params() {
        let cfg = RicianConfig::fig8();
        assert_eq!(cfg.k_factor, 5.0);
        assert_eq!(cfg.coherence_block, 128);
        assert_eq!(cfg.taps, 4);
    }

    #[test]
    fn test_rician_config_fig8_frame_bits() {
        let cfg = RicianConfig::fig8();
        assert_eq!(cfg.frame_bits(), 1024);
    }

    #[test]
    fn test_rician_config_fig8_frame_symbols() {
        let cfg = RicianConfig::fig8();
        assert_eq!(cfg.frame_symbols(), 512);
    }

    #[test]
    fn test_rician_config_fig9_params() {
        let cfg = RicianConfig::fig9();
        assert_eq!(cfg.k_factor, 8.0);
        assert_eq!(cfg.coherence_block, 256);
        assert_eq!(cfg.taps, 2);
    }

    #[test]
    fn test_rician_config_fig9_frame_bits() {
        let cfg = RicianConfig::fig9();
        assert_eq!(cfg.frame_bits(), 1024);
    }

    #[test]
    fn test_rician_config_fig10_params() {
        let cfg = RicianConfig::fig10();
        assert_eq!(cfg.k_factor, 6.0);
        assert_eq!(cfg.coherence_block, 256);
        assert_eq!(cfg.taps, 8);
    }

    #[test]
    fn test_rician_config_fig10_frame_bits() {
        let cfg = RicianConfig::fig10();
        assert_eq!(cfg.frame_bits(), 4096);
    }

    // ---- RicianChannel ----

    #[test]
    fn test_rician_channel_coefficient_is_finite() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let h = channel.sample_coefficient(&mut rng);
            assert!(h.re.is_finite());
            assert!(h.im.is_finite());
        }
    }

    #[test]
    fn test_rician_channel_mean_power_near_one() {
        // E[|H_Ri|^2] = 1 by construction; verify empirically
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let n = 100_000;
        let mean_power: f64 = (0..n)
            .map(|_| channel.sample_coefficient(&mut rng).norm_sq())
            .sum::<f64>()
            / n as f64;
        assert!(
            (mean_power - 1.0).abs() < 0.05,
            "Expected E[|H|^2] ≈ 1, got {mean_power:.4}"
        );
    }

    #[test]
    fn test_rician_channel_mean_real_part() {
        // E[Re(H_Ri)] = sqrt(K/(K+1))
        let cfg = RicianConfig::fig8();
        let channel = RicianChannel::new(cfg);
        let expected_mean_re = (cfg.k_factor / (cfg.k_factor + 1.0)).sqrt();
        let mut rng = rand::thread_rng();
        let n = 100_000;
        let mean_re: f64 = (0..n)
            .map(|_| channel.sample_coefficient(&mut rng).re)
            .sum::<f64>()
            / n as f64;
        assert!(
            (mean_re - expected_mean_re).abs() < 0.05,
            "Expected E[Re(H)] ≈ {expected_mean_re:.4}, got {mean_re:.4}"
        );
    }

    #[test]
    fn test_rician_channel_mean_imag_near_zero() {
        // E[Im(H_Ri)] = 0 (scatter imaginary part has zero mean)
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let n = 100_000;
        let mean_im: f64 = (0..n)
            .map(|_| channel.sample_coefficient(&mut rng).im)
            .sum::<f64>()
            / n as f64;
        assert!(
            mean_im.abs() < 0.05,
            "Expected E[Im(H)] ≈ 0, got {mean_im:.4}"
        );
    }

    #[test]
    fn test_rician_channel_generate_frame_gains_length() {
        let cfg = RicianConfig::fig8();
        let channel = RicianChannel::new(cfg);
        let mut rng = rand::thread_rng();
        let gains = channel.generate_frame_gains(&mut rng);
        assert_eq!(gains.len(), cfg.frame_symbols());
    }

    #[test]
    fn test_rician_channel_block_fading_constant_within_block() {
        // Within each coherence block, all gains must be identical
        let cfg = RicianConfig::fig8();
        let channel = RicianChannel::new(cfg);
        let mut rng = rand::thread_rng();
        let gains = channel.generate_frame_gains(&mut rng);
        for tap in 0..cfg.taps {
            let start = tap * cfg.coherence_block;
            let block = &gains[start..start + cfg.coherence_block];
            let first = block[0];
            for g in block {
                assert_eq!(g.re, first.re);
                assert_eq!(g.im, first.im);
            }
        }
    }

    #[test]
    fn test_rician_channel_transmit_length() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let symbols: Vec<Complex> = (0..8).map(|_| Complex::new(1.0, 0.0)).collect();
        let gains = vec![Complex::new(1.0, 0.0); 8];
        let received = channel.transmit(&symbols, &gains, 0.1, &mut rng);
        assert_eq!(received.len(), 8);
    }

    #[test]
    fn test_rician_channel_transmit_is_finite() {
        let channel = RicianChannel::new(RicianConfig::fig8());
        let mut rng = rand::thread_rng();
        let symbols: Vec<Complex> = (0..16).map(|_| Complex::new(1.0, -1.0)).collect();
        let gains = vec![Complex::new(0.8, 0.3); 16];
        let received = channel.transmit(&symbols, &gains, 0.5, &mut rng);
        for r in &received {
            assert!(r.re.is_finite() && r.im.is_finite());
        }
    }

    #[test]
    fn test_rician_channel_all_configs() {
        // Smoke test: all three paper configurations can be constructed and sampled
        let mut rng = rand::thread_rng();
        for cfg in [
            RicianConfig::fig8(),
            RicianConfig::fig9(),
            RicianConfig::fig10(),
        ] {
            let channel = RicianChannel::new(cfg);
            let gains = channel.generate_frame_gains(&mut rng);
            assert_eq!(gains.len(), cfg.frame_symbols());
        }
    }

    // ---- BitInterleaver ----

    #[test]
    fn test_interleaver_len() {
        let il = BitInterleaver::new(128, 42);
        assert_eq!(il.len(), 128);
    }

    #[test]
    fn test_interleaver_is_permutation() {
        // The permutation must be a bijection: each index appears exactly once
        let n = 64;
        let il = BitInterleaver::new(n, 99);
        let mut seen = vec![false; n];
        for &p in &il.perm {
            assert!(!seen[p], "Duplicate index {p} in permutation");
            seen[p] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn test_interleaver_inverse_is_valid() {
        // inv_perm[perm[i]] == i for all i
        let n = 64;
        let il = BitInterleaver::new(n, 7);
        for i in 0..n {
            assert_eq!(il.inv_perm[il.perm[i]], i);
        }
    }

    #[test]
    fn test_interleave_deinterleave_roundtrip() {
        let n = 64;
        let il = BitInterleaver::new(n, 12345);
        let bits: Vec<bool> = (0..n).map(|i| i % 3 == 0).collect();
        let interleaved = il.interleave(&bits);
        let recovered = il.deinterleave(&interleaved);
        assert_eq!(recovered, bits);
    }

    #[test]
    fn test_interleave_changes_order() {
        // With a non-trivial seed and n > 1, interleaving should produce a different order
        let n = 64;
        let il = BitInterleaver::new(n, 42);
        let bits: Vec<bool> = (0..n).map(|i| i % 2 == 0).collect();
        let interleaved = il.interleave(&bits);
        // Very unlikely to be identical for n=64 with a non-trivial permutation
        assert_ne!(interleaved, bits, "Interleaving should change the order");
    }

    #[test]
    fn test_interleave_llrs_roundtrip() {
        let n = 64;
        let il = BitInterleaver::new(n, 55);
        let llrs: Vec<crate::llr::Llr> = (0..n)
            .map(|i| crate::llr::Llr::new(i as f32 - 32.0))
            .collect();
        let interleaved = il.interleave_llrs(&llrs);
        let recovered = il.deinterleave_llrs(&interleaved);
        for (a, b) in llrs.iter().zip(recovered.iter()) {
            assert!((a.value() - b.value()).abs() < 1e-6);
        }
    }

    #[test]
    fn test_interleaver_deterministic_same_seed() {
        let a = BitInterleaver::new(32, 999);
        let b = BitInterleaver::new(32, 999);
        assert_eq!(a.perm, b.perm);
    }

    #[test]
    fn test_interleaver_different_seeds_differ() {
        let a = BitInterleaver::new(32, 1);
        let b = BitInterleaver::new(32, 2);
        assert_ne!(a.perm, b.perm);
    }

    #[test]
    #[should_panic(expected = "Input length")]
    fn test_interleave_wrong_length_panics() {
        let il = BitInterleaver::new(16, 0);
        let bits = vec![false; 8]; // Wrong length
        il.interleave(&bits);
    }

    #[test]
    #[should_panic(expected = "must be positive")]
    fn test_interleaver_zero_length_panics() {
        let _ = BitInterleaver::new(0, 0);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn interleave_deinterleave_roundtrip_prop(
            n in 1usize..128usize,
            seed: u64,
            bits in prop::collection::vec(any::<bool>(), 0..128)
        ) {
            // Only test when bits.len() == n
            let bits: Vec<bool> = bits.into_iter().take(n).chain(std::iter::repeat(false)).take(n).collect();
            let il = BitInterleaver::new(n, seed);
            let interleaved = il.interleave(&bits);
            let recovered = il.deinterleave(&interleaved);
            prop_assert_eq!(recovered, bits);
        }

        #[test]
        fn rician_channel_power_near_one(k_factor in 0.1f64..20.0f64) {
            let cfg = RicianConfig { k_factor, coherence_block: 1, taps: 1 };
            let channel = RicianChannel::new(cfg);
            let mut rng = rand::thread_rng();
            let n = 10_000;
            let mean_power: f64 = (0..n)
                .map(|_| channel.sample_coefficient(&mut rng).norm_sq())
                .sum::<f64>() / n as f64;
            // Allow 3% tolerance for statistical variation
            prop_assert!((mean_power - 1.0).abs() < 0.1,
                "E[|H|^2] = {mean_power:.4}, expected ~1.0");
        }
    }
}
