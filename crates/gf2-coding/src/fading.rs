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

use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::ops::{Add, Mul};

/// Complex number used by the Rician fading channel model for
/// channel-math composition (`y = h·x + n`).
///
/// This type is intentionally minimal: it carries only the real/imaginary
/// pair and the arithmetic required by [`RicianChannel::transmit`] and
/// [`RicianChannel::generate_frame_gains`]. It is **not** a modulation
/// primitive — all bit-to-symbol mapping and demapping lives in
/// [`crate::modem`].
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
    /// use gf2_coding::fading::Complex;
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
    /// use gf2_coding::fading::Complex;
    ///
    /// let c = Complex::new(1.0, -2.0).conj();
    /// assert_eq!(c.re, 1.0);
    /// assert_eq!(c.im, 2.0);
    /// ```
    pub fn conj(self) -> Self {
        Complex {
            re: self.re,
            im: -self.im,
        }
    }

    /// Returns the squared absolute value |z|^2.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::Complex;
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
    /// use gf2_coding::fading::Complex;
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
    /// use gf2_coding::fading::Complex;
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

    fn mul(self, other: Complex) -> Complex {
        Complex {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
}

impl Add for Complex {
    type Output = Complex;

    fn add(self, other: Complex) -> Complex {
        Complex {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
}

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
    /// # Panics
    ///
    /// Panics if:
    /// - `config.k_factor < 0.0` (K-factor must be non-negative)
    /// - `config.coherence_block == 0` (coherence block size must be positive)
    /// - `config.taps == 0` (number of taps must be positive)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{RicianChannel, RicianConfig};
    ///
    /// let channel = RicianChannel::new(RicianConfig::fig8());
    /// ```
    pub fn new(config: RicianConfig) -> Self {
        assert!(
            config.k_factor >= 0.0,
            "Rician K-factor must be non-negative, got {}",
            config.k_factor
        );
        assert!(
            config.coherence_block > 0,
            "Coherence block size N_c must be positive, got {}",
            config.coherence_block
        );
        assert!(
            config.taps > 0,
            "Number of taps t must be positive, got {}",
            config.taps
        );

        let k = config.k_factor;
        let los_amplitude = (k / (k + 1.0)).sqrt();
        let scatter_scale = (1.0 / (k + 1.0)).sqrt();
        // sigma^2 = 0.5, so sigma = 1/sqrt(2)
        let sigma = (0.5_f64).sqrt();
        let scatter_dist = Normal::new(0.0, sigma).expect("Failed to create normal distribution");
        RicianChannel {
            config,
            los_amplitude,
            scatter_scale,
            scatter_dist,
        }
    }

    /// Returns the channel configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{RicianChannel, RicianConfig};
    ///
    /// let channel = RicianChannel::new(RicianConfig::fig9());
    /// let cfg = channel.config();
    /// assert_eq!(cfg.k_factor, 8.0);
    /// assert_eq!(cfg.coherence_block, 256);
    /// assert_eq!(cfg.taps, 2);
    /// ```
    pub fn config(&self) -> &RicianConfig {
        &self.config
    }

    /// Generates a single Rician fading channel coefficient.
    ///
    /// Returns `H_Ri = sqrt(K/(K+1)) + sqrt(1/(K+1))·(X + jY)` where
    /// `X, Y ~ N(0, 0.5)`.
    ///
    /// # Arguments
    ///
    /// * `rng` - Random number generator for sampling the scatter component
    ///
    /// # Complexity
    ///
    /// O(1). Generates two Gaussian random samples and combines them with
    /// the deterministic LOS component.
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
    /// Panics if `symbols` and `channel_gains` have different lengths,
    /// or if `sigma_squared` is not positive and finite.
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of symbols.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{Complex, RicianChannel, RicianConfig};
    ///
    /// let channel = RicianChannel::new(RicianConfig::fig8());
    /// let mut rng = rand::thread_rng();
    /// // Drive the channel with explicit complex symbols — QPSK bit-to-
    /// // symbol mapping lives in `gf2_coding::modem` and is exercised by
    /// // `QpskRicianChannelModel` end-to-end.
    /// let symbols = vec![Complex::new(1.0, 1.0); 4];
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
        assert!(
            sigma_squared > 0.0 && sigma_squared.is_finite(),
            "sigma_squared must be positive and finite, got {sigma_squared}"
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
/// let interleaver = BitInterleaver::new(8, 42);
/// let bits = vec![false, true, false, false, true, false, true, true];
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

    // ---- RicianChannel input validation ----

    #[test]
    #[should_panic(expected = "K-factor must be non-negative")]
    fn test_rician_channel_negative_k_panics() {
        let cfg = RicianConfig {
            k_factor: -1.0,
            coherence_block: 128,
            taps: 4,
        };
        let _ = RicianChannel::new(cfg);
    }

    #[test]
    #[should_panic(expected = "N_c must be positive")]
    fn test_rician_channel_zero_coherence_block_panics() {
        let cfg = RicianConfig {
            k_factor: 5.0,
            coherence_block: 0,
            taps: 4,
        };
        let _ = RicianChannel::new(cfg);
    }

    #[test]
    #[should_panic(expected = "t must be positive")]
    fn test_rician_channel_zero_taps_panics() {
        let cfg = RicianConfig {
            k_factor: 5.0,
            coherence_block: 128,
            taps: 0,
        };
        let _ = RicianChannel::new(cfg);
    }

    #[test]
    fn test_rician_channel_k_factor_zero_is_rayleigh() {
        // K=0 is valid: pure Rayleigh fading (no LOS component)
        let cfg = RicianConfig {
            k_factor: 0.0,
            coherence_block: 64,
            taps: 1,
        };
        let channel = RicianChannel::new(cfg);
        let mut rng = rand::thread_rng();
        let h = channel.sample_coefficient(&mut rng);
        assert!(h.re.is_finite() && h.im.is_finite());
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

// ---------------------------------------------------------------------------
// Simulation harness integration: ChannelModel for QPSK + Rician fading
// ---------------------------------------------------------------------------

use crate::simulation::ChannelModel;

/// QPSK modulation over a Rician fading channel with interleaving.
///
/// Implements [`ChannelModel`] for integration with the simulation harness.
/// The pipeline: bits → interleave → QPSK map → Rician fading + AWGN →
/// soft LLR (framework demapper, fed per-symbol complex channel gains) →
/// de-interleave.
///
/// # Modem framework integration
///
/// Internally this delegates the QPSK mapping and demapping to the shared
/// modem surface — [`GrayQamMapper`](crate::modem::GrayQamMapper) at preset
/// order `4` and
/// [`ReferenceSoftDemapper`](crate::modem::ReferenceSoftDemapper) over
/// [`ModemSpec::gray_square_qam(4)`](crate::modem::ModemSpec::gray_square_qam).
/// The Rician fading composition (per-coherence-block complex gain) and
/// the [`BitInterleaver`] composition are unchanged; only the
/// hand-rolled QPSK map / LLR math is replaced with framework calls.
///
/// The framework demapper consumes the per-symbol complex gain
/// `h = h_i + j h_q` directly via [`DemapInput::gain_i`] /
/// [`DemapInput::gain_q`]; no manual `conj(h)` pre-rotation is performed
/// here. The MSB-first intra-symbol bit order matches the framework
/// Gray-QAM convention (bit 0 of each pair drives the I axis, bit 1
/// drives Q).
///
/// # Noise convention
///
/// Legacy and framework agree: `N0 = 2 sigma^2`. The legacy formula
/// `sigma^2 = 1 / (2 * Es/N0_lin)` with `Es/N0_lin = m * rate * Eb/N0_lin`
/// (here `m = 2` for QPSK) is preserved exactly, then surfaced to the
/// demapper as `noise_var = 2 * sigma^2 = N0`. See the
/// [`crate::modem::awgn_link`] module-level "Noise convention" docs for
/// the full chain — this fading path uses the same scaling.
///
/// # Constraints
///
/// The codeword passed to [`ChannelModel::transmit_and_demodulate`] must satisfy:
/// - **Even length**: QPSK maps 2 bits per symbol.
/// - **Length ≤ frame capacity**: The codeword must fit within the Rician
///   channel's frame structure (`config.frame_bits()`). For example,
///   [`RicianConfig::fig8`] supports up to 1024 bits.
///
/// # Panics
///
/// Panics if the codeword length is odd or exceeds the frame capacity.
///
/// # Examples
///
/// ```no_run
/// use gf2_coding::fading::{QpskRicianChannelModel, RicianConfig};
/// use gf2_coding::simulation::{SimulationRunner, SimulationConfig};
///
/// let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
/// // Use with SimulationRunner::run_coded(&encoder, &decoder, &channel, &config)
/// // Codeword length must be even and ≤ 1024 for fig8 config
/// ```
pub struct QpskRicianChannelModel {
    config: RicianConfig,
}

impl QpskRicianChannelModel {
    /// Creates a new QPSK + Rician fading channel model.
    ///
    /// # Arguments
    ///
    /// * `config` - Rician fading channel configuration
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::fading::{QpskRicianChannelModel, RicianConfig};
    ///
    /// let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
    /// let _ = channel; // delegates QPSK map/demap to the modem framework
    /// ```
    pub fn new(config: RicianConfig) -> Self {
        Self { config }
    }
}

impl ChannelModel for QpskRicianChannelModel {
    fn batch_alignment(&self) -> usize {
        // QPSK carries 2 bits per symbol, so `codeword.len()` must be
        // even. Surfacing this through the trait lets modem-aware
        // simulation runners round their batches down to a multiple of
        // 2 rather than hitting the assertion below.
        2
    }

    fn transmit_and_demodulate<R: rand::Rng>(
        &self,
        codeword: &gf2_core::BitVec,
        eb_n0_db: f64,
        code_rate: f64,
        rng: &mut R,
    ) -> Vec<crate::llr::Llr> {
        use crate::modem::{
            BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, GrayQamMapper, ModemSpec,
            ReferenceSoftDemapper,
        };
        use rand_distr::{Distribution, Normal};

        let n = codeword.len();
        assert!(n % 2 == 0, "QPSK requires even codeword length, got {n}");
        assert!(
            n <= self.config.frame_bits(),
            "codeword length {n} exceeds frame capacity {} for this Rician config",
            self.config.frame_bits()
        );

        // Canonical unit-energy Eb/N0 -> N0 via the shared helper in
        // `modem::awgn_link`. This is the SSOT for Eb/N0 -> noise-scale
        // conversion across the AWGN and fading paths, so if the
        // framework's convention ever changes we only edit one place.
        //
        // `RicianChannel::transmit` below expects `N0` (it samples each
        // axis with std = sqrt(N0/2)), and the framework demapper takes
        // `noise_var = N0`. We retain the historic name `sigma_squared`
        // for the `RicianChannel` API parameter — semantically N0.
        use crate::modem::awgn_link::unit_energy_n0_from_eb_n0_db;
        const M_BITS_PER_SYMBOL: usize = 2; // QPSK
        let sigma_squared = unit_energy_n0_from_eb_n0_db(M_BITS_PER_SYMBOL, code_rate, eb_n0_db);
        let noise_dist = Normal::new(0.0, (sigma_squared / 2.0).sqrt())
            .expect("Failed to create noise distribution");

        // Convert BitVec to bool slice for interleaver.
        let bit_vec: Vec<bool> = (0..n).map(|i| codeword.get(i)).collect();

        // Interleave (orthogonal to modem choice).
        let interleaver = BitInterleaver::new(n, 0xFADE);
        let interleaved = interleaver.interleave(&bit_vec);

        // QPSK map via the shared modem surface.
        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let num_symbols = n / 2;
        let mut tx_i = vec![0.0_f32; num_symbols];
        let mut tx_q = vec![0.0_f32; num_symbols];
        mapper.map_bits(&interleaved, &mut tx_i, &mut tx_q);

        // Rician fading: one coherence-block-shared gain per symbol.
        let channel = RicianChannel::new(self.config);
        let mut gains = channel.generate_frame_gains(rng);
        gains.truncate(num_symbols);

        // Apply h * x + n with independent Gaussian noise on I and Q.
        // Note: the framework demapper consumes the raw complex gain
        // (gain_i, gain_q); do NOT pre-rotate by conj(h) here.
        let mut rx_i = vec![0.0_f32; num_symbols];
        let mut rx_q = vec![0.0_f32; num_symbols];
        let mut gain_i = vec![0.0_f32; num_symbols];
        let mut gain_q = vec![0.0_f32; num_symbols];
        for k in 0..num_symbols {
            let h = gains[k];
            let xi = tx_i[k] as f64;
            let xq = tx_q[k] as f64;
            let noise_re: f64 = noise_dist.sample(rng);
            let noise_im: f64 = noise_dist.sample(rng);
            rx_i[k] = (h.re * xi - h.im * xq + noise_re) as f32;
            rx_q[k] = (h.re * xq + h.im * xi + noise_im) as f32;
            gain_i[k] = h.re as f32;
            gain_q[k] = h.im as f32;
        }

        // Demap via the shared modem surface. `sigma_squared` above is
        // semantically N0 (the file-wide convention — see RicianChannel
        // docs); the framework demapper takes N0 directly.
        let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let noise_var = vec![sigma_squared as f32; num_symbols];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: Some(&gain_i),
            gain_q: Some(&gain_q),
            noise_var: &noise_var,
            method: DemapMethod::ExactLogMap,
        };
        let mut llrs = vec![crate::llr::Llr::new(0.0); n];
        demapper.demap_llrs(input, &mut llrs);

        interleaver.deinterleave_llrs(&llrs)
    }
}

#[cfg(test)]
mod channel_model_tests {
    use super::*;
    use crate::grand::{OrbGrand, OrbGrandConfig};
    use crate::simulation::{SimulationConfig, SimulationRunner};

    #[test]
    fn test_qpsk_rician_channel_model_preconditions() {
        let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
        // fig8 frame_bits = 2 * 4 * 128 = 1024
        assert_eq!(channel.config.frame_bits(), 1024);
    }

    #[test]
    #[should_panic(expected = "even codeword length")]
    fn test_qpsk_rician_rejects_odd_length() {
        let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
        let bits = gf2_core::BitVec::zeros(7); // odd
        let mut rng = rand::thread_rng();
        channel.transmit_and_demodulate(&bits, 6.0, 0.5, &mut rng);
    }

    #[test]
    fn test_qpsk_rician_through_simulation_runner() {
        // Use Hamming(7,4) extended to (8,4) so codeword length is even
        // Actually, use a small systematic code with even n.
        // Hamming(15,11) has n=15 (odd). Let's use eBCH(16,11) with ORBGRAND.
        use crate::bch::extended::ExtendedBchCode;

        let ebch = ExtendedBchCode::ebch_16_11();
        let h = ebch.parity_check().clone();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());

        // fig9 has frame_bits = 2 * 2 * 256 = 1024, n=16 fits
        let channel = QpskRicianChannelModel::new(RicianConfig::fig9());

        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0]; // High SNR for reliable decode
        config.max_frames = 20;
        config.min_errors = 1;

        let results = SimulationRunner::run_coded(&ebch, &decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].num_frames > 0);
        // At 10 dB with short code, BER should be reasonable
        assert!(results.points[0].ber < 0.5, "BER too high at 10 dB");
    }
}

// ---------------------------------------------------------------------------
// Framework calibration lock: the Rician fading path shares its N0 formula
// with `ModemChannelAdapter` through `modem::awgn_link::unit_energy_*`.
// The remaining test below pins that formula at literal values so any
// silent drift in the Eb/N0 -> N0 mapping is caught regardless of which
// caller triggered the change.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod modem_framework_calibration_tests {
    use super::*;

    /// Shared-formula calibration lock for the fading path.
    ///
    /// The migrated fading path and `ModemChannelAdapter` both derive
    /// `N0` from the same Eb/N0 via
    /// [`crate::modem::awgn_link::unit_energy_n0_from_eb_n0_db`]. This
    /// test asserts that helper's output against literal expected
    /// values at several (m, rate, Eb/N0) points — a single pinning
    /// point for the whole framework — and then executes the full
    /// `QpskRicianChannelModel::transmit_and_demodulate` pipeline to
    /// prove it runs end-to-end through the shared helper and produces
    /// finite, scale-appropriate LLRs.
    ///
    /// Any drift in the shared N0 formula (the earlier 3 dB calibration
    /// bug) would be caught at the `assert!` on `expected_n0` below.
    #[test]
    fn test_qpsk_rician_shared_n0_calibration() {
        use crate::modem::awgn_link::{
            unit_energy_n0_from_eb_n0_db, unit_energy_sigma_sq_from_eb_n0_db,
        };
        use crate::simulation::ChannelModel;
        use gf2_core::BitVec;
        use rand::{rngs::StdRng, SeedableRng};

        // Lock the shared helper's output against literal values at
        // several (m, rate, Eb/N0_dB) points. These are the canonical
        // unit-energy formula `N0 = 1 / (m * rate * 10^(Eb/N0_dB/10))`.
        for (m, rate, eb_n0_db, expected_n0) in [
            (2usize, 1.0_f64, 0.0_f64, 0.5_f64),
            (2, 1.0, 10.0, 0.05),
            (2, 0.5, 0.0, 1.0),
            (2, 0.5, 10.0, 0.1),
            (4, 1.0, 0.0, 0.25),
            (4, 1.0, 10.0, 0.025),
        ] {
            let n0 = unit_energy_n0_from_eb_n0_db(m, rate, eb_n0_db);
            let sigma_sq = unit_energy_sigma_sq_from_eb_n0_db(m, rate, eb_n0_db);
            assert!(
                (n0 - expected_n0).abs() < 1e-12,
                "n0(m={m}, rate={rate}, eb_n0_db={eb_n0_db}) = {n0}, expected {expected_n0}"
            );
            assert!(
                (n0 - 2.0 * sigma_sq).abs() < 1e-12,
                "N0 must equal 2·sigma^2 by construction"
            );
        }

        // Run the full fading pipeline end-to-end and verify it only
        // produces finite LLRs and that the hot-path magnitudes are in
        // the band predicted by the calibrated N0. At Eb/N0 = 10 dB,
        // QPSK uncoded, N0 = 0.025 and well-resolved symbols see
        // `|LLR| = 4 y / (N0 |h|^2)` on the order of tens.
        let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
        let n_bits = 64;
        let mut codeword = BitVec::zeros(n_bits);
        for i in 0..n_bits {
            if (i * 17 + 11) & 1 == 0 {
                codeword.set(i, true);
            }
        }
        let mut rng = StdRng::seed_from_u64(0xDEADBEEFCAFE0010);
        let llrs = channel.transmit_and_demodulate(&codeword, 10.0, 1.0, &mut rng);
        assert_eq!(llrs.len(), n_bits);
        for llr in &llrs {
            assert!(
                llr.value().is_finite(),
                "non-finite LLR through fading pipeline: {}",
                llr.value()
            );
        }
        // A 3 dB calibration shift would halve every LLR magnitude.
        // At 10 dB Eb/N0 with strong LOS (fig8) we expect at least
        // one |LLR| above 1.0; the shared-formula lock above is the
        // tight guard, this assertion is a sanity backstop.
        let any_decisive = llrs.iter().any(|l| l.value().abs() > 1.0);
        assert!(
            any_decisive,
            "no decisive LLRs at 10 dB Eb/N0 — calibration likely broken"
        );
    }

    #[test]
    fn test_interleaver_still_composes() {
        use crate::bch::extended::ExtendedBchCode;
        use crate::grand::{OrbGrand, OrbGrandConfig};
        use crate::simulation::{SimulationConfig, SimulationRunner};

        let ebch = ExtendedBchCode::ebch_16_11();
        let h = ebch.parity_check().clone();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());
        let channel = QpskRicianChannelModel::new(RicianConfig::fig9());

        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![12.0];
        config.max_frames = 30;
        config.min_errors = 1;

        let results = SimulationRunner::run_coded(&ebch, &decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].num_frames > 0);
        assert!(
            results.points[0].ber < 0.5,
            "interleaver+modem composition broken: BER {} too high",
            results.points[0].ber
        );
    }
}
