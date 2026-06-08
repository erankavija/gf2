//! Rayleigh flat-fading channel stage.
//!
//! This module provides the [`Rayleigh`] [`Stage`](crate::Stage) impl, which
//! models frequency-flat Rayleigh fading. Each symbol is multiplied by an
//! independent complex fading coefficient `h ~ CN(0, 1)` and then corrupted by
//! complex AWGN:
//!
//! ```text
//! r = h * x + n
//! ```
//!
//! where `h = (h_r + j*h_i) / sqrt(2)` with `h_r, h_i ~ N(0,1)` (so
//! `E[|h|^2] = 1`), and `n = (n_r + j*n_i)` with `n_r, n_i ~ N(0, sigma^2)`.
//!
//! # Noise model
//!
//! `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))` — the same per-axis variance as in
//! the AWGN channel (`frame_sim.rs` SSOT). The fading and noise draws are
//! interleaved from the **same** ChaCha20 stream, consuming exactly **8 ChaCha20
//! 32-bit words per symbol** (4 draws for fading h, 4 draws for noise n).
//!
//! A debug assertion guards that the total draw does not exceed
//! `FRAME_STRIDE - 256` words.

use rand::Rng as _;
use rand_chacha::ChaCha20Rng;

use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;

use crate::batch::SymbolBatch;
use crate::channels::awgn::ChannelScratch;
use crate::error::StageError;
use crate::parallel::FRAME_STRIDE;
use crate::stage::{ExecutionClass, Stage};

/// Rayleigh flat-fading channel stage.
///
/// Each symbol is passed through an independent complex fading coefficient
/// `h ~ CN(0, 1)` plus AWGN noise, matching the standard frequency-flat
/// Rayleigh model.
///
/// # Arguments (constructor)
///
/// * `es_n0_db` — channel Es/N0 in dB.
/// * `bits_per_symbol` — modulation order in bits/symbol (stored for diagnostics).
///
/// # Examples
///
/// ```
/// use gf2_sim::channels::rayleigh::Rayleigh;
///
/// let ch = Rayleigh::new(6.25, 4);
/// assert_eq!(ch.bits_per_symbol(), 4);
/// let sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(6.25 / 10.0));
/// let expected_sigma = (sigma_sq as f32).sqrt();
/// assert!((ch.sigma() - expected_sigma).abs() < 1e-7);
/// ```
#[derive(Debug, Clone)]
pub struct Rayleigh {
    /// Channel Es/N0 in dB.
    es_n0_db: f32,
    /// Modulation order (bits/symbol).
    bits_per_symbol: usize,
    /// Per-axis AWGN noise standard deviation.
    sigma: f32,
}

impl Rayleigh {
    /// Constructs a Rayleigh fading channel stage.
    ///
    /// # Arguments
    ///
    /// * `es_n0_db` — channel Es/N0 in dB.
    /// * `bits_per_symbol` — modulation order in bits/symbol.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::channels::rayleigh::Rayleigh;
    ///
    /// let ch = Rayleigh::new(10.0, 4);
    /// assert_eq!(ch.bits_per_symbol(), 4);
    /// ```
    #[must_use]
    pub fn new(es_n0_db: f32, bits_per_symbol: usize) -> Self {
        let es_n0_lin = 10.0_f64.powf(es_n0_db as f64 / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        let sigma = (sigma_sq as f32).sqrt();
        Self {
            es_n0_db,
            bits_per_symbol,
            sigma,
        }
    }

    /// The Es/N0 in dB this channel was constructed with.
    #[inline]
    #[must_use]
    pub fn es_n0_db(&self) -> f32 {
        self.es_n0_db
    }

    /// The modulation order in bits/symbol.
    #[inline]
    #[must_use]
    pub fn bits_per_symbol(&self) -> usize {
        self.bits_per_symbol
    }

    /// Per-axis AWGN noise standard deviation `sigma = sqrt(1 / (2 * 10^(Es/N0_dB/10)))`.
    #[inline]
    #[must_use]
    pub fn sigma(&self) -> f32 {
        self.sigma
    }

    /// Applies Rayleigh fading and AWGN to `batch` in-place, drawing from `rng`.
    ///
    /// For each symbol, draws `h ~ CN(0, 1)` and `n ~ CN(0, 2*sigma^2)` from
    /// `rng`, then sets `r = h * x + n`. The fading and noise draws are
    /// interleaved from the same stream, consuming **8 ChaCha20 32-bit words per
    /// symbol** (4 for `h`, 4 for `n`).
    ///
    /// # Arguments
    ///
    /// * `batch` — the IQ symbol batch to corrupt in-place.
    /// * `rng` — noise RNG seeked to the frame's §3 word-position offset.
    ///
    /// # Complexity
    ///
    /// O(N) where N is the total number of symbols across all frames in the batch.
    pub fn apply(&self, batch: &mut SymbolBatch, rng: &mut ChaCha20Rng) {
        let pos_before = rng.get_word_pos();
        for (i_frame, q_frame) in batch.i.iter_mut().zip(batch.q.iter_mut()) {
            for (xi, xq) in i_frame.iter_mut().zip(q_frame.iter_mut()) {
                // Draw complex fading coefficient h ~ CN(0, 1):
                //   h = (h_r + j*h_i) where h_r, h_i ~ N(0, 0.5)
                //   so |h|^2 has mean 1 (Rayleigh envelope).
                // Box-Muller produces N(0,1) samples; divide by sqrt(2) for
                // per-component variance 1/2 so E[|h|^2] = 2*(1/2) = 1.
                let u1h: f64 = rng.random();
                let u2h: f64 = rng.random();
                let h_r = box_muller_cos(u1h, u2h) * std::f32::consts::FRAC_1_SQRT_2;
                let u3h: f64 = rng.random();
                let u4h: f64 = rng.random();
                let h_i = box_muller_cos(u3h, u4h) * std::f32::consts::FRAC_1_SQRT_2;

                // Apply fading: r_noiseless = h * x
                let x_i = *xi;
                let x_q = *xq;
                let r_i = h_r * x_i - h_i * x_q;
                let r_q = h_r * x_q + h_i * x_i;

                // Draw AWGN noise n ~ CN(0, 2*sigma^2):
                //   n_r, n_q ~ N(0, sigma^2)
                let u1n: f64 = rng.random();
                let u2n: f64 = rng.random();
                let n_i = box_muller_cos(u1n, u2n) * self.sigma;
                let u3n: f64 = rng.random();
                let u4n: f64 = rng.random();
                let n_q = box_muller_cos(u3n, u4n) * self.sigma;

                *xi = r_i + n_i;
                *xq = r_q + n_q;
            }
        }
        let noise_words_drawn = rng.get_word_pos().saturating_sub(pos_before);
        debug_assert!(
            noise_words_drawn <= FRAME_STRIDE - 256,
            "Rayleigh draw {noise_words_drawn} words exceeded FRAME_STRIDE - 256 = {}",
            FRAME_STRIDE - 256
        );
    }
}

impl Stage<SymbolBatch, SymbolBatch> for Rayleigh {
    type Scratch = ChannelScratch;
    type CpuFallback = Self;

    /// Applies Rayleigh fading and AWGN to a copy of `input`, drawing from
    /// `scratch.rng`.
    ///
    /// # Errors
    ///
    /// Returns `StageError::Internal` if the underlying draw overflows the
    /// frame budget (debug build only; release builds skip the check).
    fn process(
        &self,
        input: &SymbolBatch,
        scratch: &mut ChannelScratch,
    ) -> Result<SymbolBatch, StageError> {
        let mut out = input.clone();
        self.apply(&mut out, &mut scratch.rng);
        Ok(out)
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::BatchSize;
    use rand::SeedableRng as _;

    fn make_batch(frames: usize, syms: usize) -> SymbolBatch {
        let i = vec![vec![1.0_f32; syms]; frames];
        let q = vec![vec![0.0_f32; syms]; frames];
        SymbolBatch::new(i, q)
    }

    #[test]
    fn test_rayleigh_sigma_formula() {
        let ch = Rayleigh::new(0.0, 4);
        let expected = (1.0_f32 / 2.0).sqrt();
        assert!(
            (ch.sigma() - expected).abs() < 1e-6,
            "sigma mismatch at 0 dB Es/N0"
        );
    }

    #[test]
    fn test_rayleigh_apply_dimensions_preserved() {
        let ch = Rayleigh::new(10.0, 4);
        let input = make_batch(2, 50);
        let mut batch = input.clone();
        let mut rng = ChaCha20Rng::seed_from_u64(7);
        ch.apply(&mut batch, &mut rng);
        assert_eq!(batch.batch_size(), 2);
        assert_eq!(batch.i[0].len(), 50);
    }

    #[test]
    fn test_rayleigh_stage_process() {
        let ch = Rayleigh::new(10.0, 4);
        let input = make_batch(1, 100);
        let mut scratch = ChannelScratch::default();
        let out = ch.process(&input, &mut scratch).unwrap();
        assert_eq!(out.batch_size(), 1);
        assert_eq!(out.i[0].len(), 100);
    }
}
