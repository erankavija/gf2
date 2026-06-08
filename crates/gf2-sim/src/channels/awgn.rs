//! AWGN channel stage — additive white Gaussian noise on IQ symbol batches.
//!
//! This module provides the [`Awgn`] [`Stage`](crate::Stage) impl, which adds
//! independent circularly-symmetric complex Gaussian noise to every I/Q symbol
//! in a [`SymbolBatch`](crate::SymbolBatch). Noise samples are drawn from the
//! per-stage [`ChannelScratch`] RNG, which the Phase C executor seeks per frame
//! via the §3 word-position scheme (design doc §3).
//!
//! # Noise model
//!
//! For each symbol the channel adds:
//! - I-axis: `n_I ~ N(0, sigma^2)`,  `r_I = x_I + n_I`
//! - Q-axis: `n_Q ~ N(0, sigma^2)`,  `r_Q = x_Q + n_Q`
//!
//! where `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))` (unit-energy symbol assumption,
//! per-axis — same formula as in `frame_sim.rs` and the SSOT at
//! `gf2_coding::dvb_t2_bicm_harness`).
//!
//! Each Gaussian sample consumes **4 ChaCha20 32-bit words** (two `f64` uniform
//! draws fed to [`box_muller_cos`](gf2_coding::dvb_t2_bicm_harness::box_muller_cos)).
//! A debug assertion guards that the total draw for the batch does not exceed
//! `FRAME_STRIDE - 256` words.

use rand::Rng as _;
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;

use crate::batch::SymbolBatch;
use crate::error::StageError;
use crate::parallel::FRAME_STRIDE;
use crate::stage::{ExecutionClass, Stage};

/// Per-stage scratch for the AWGN channel: holds an independent [`ChaCha20Rng`].
///
/// The Phase C executor seeds this RNG per frame via
/// [`ChaCha20Rng::set_word_pos`], aligning it to the §3 per-frame offset.
/// For the unit tests and [`Stage::process`] the RNG is used from wherever
/// it was left (or from position 0 on `Default::default()`).
///
/// # Examples
///
/// ```
/// use gf2_sim::channels::awgn::ChannelScratch;
///
/// let scratch = ChannelScratch::default();
/// // Scratch is default-constructible and Send + Sync.
/// let _: &dyn Send = &scratch;
/// ```
pub struct ChannelScratch {
    /// The worker's noise RNG.
    pub rng: ChaCha20Rng,
}

impl Default for ChannelScratch {
    fn default() -> Self {
        Self {
            rng: ChaCha20Rng::seed_from_u64(0),
        }
    }
}

// ChaCha20Rng is Send + Sync, so ChannelScratch is Send + Sync automatically.

/// AWGN channel stage: adds circularly-symmetric complex Gaussian noise to
/// every symbol in a [`SymbolBatch`].
///
/// The noise variance is `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))` per axis, where
/// `Es/N0_dB` is supplied at construction. Each sample draws two `f64` uniforms
/// from the scratch RNG and passes them to [`box_muller_cos`], consuming exactly
/// 4 ChaCha20 32-bit words per noise sample.
///
/// # Arguments (constructor)
///
/// * `es_n0_db` — channel Es/N0 in dB (e.g. `6.25`).
/// * `bits_per_symbol` — modulation order in bits/symbol (e.g. `4` for 16-QAM).
///   Stored for future diagnostics; not used in the variance formula.
///
/// # Examples
///
/// ```
/// use gf2_sim::channels::awgn::Awgn;
///
/// let ch = Awgn::new(6.25, 4);
/// assert_eq!(ch.bits_per_symbol(), 4);
/// let sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(6.25 / 10.0));
/// let expected_sigma = (sigma_sq as f32).sqrt();
/// assert!((ch.sigma() - expected_sigma).abs() < 1e-7);
/// ```
#[derive(Debug, Clone)]
pub struct Awgn {
    /// Channel Es/N0 in dB.
    es_n0_db: f32,
    /// Modulation order (bits/symbol).
    bits_per_symbol: usize,
    /// Per-axis noise standard deviation: `sqrt(1 / (2 * 10^(Es/N0_dB/10)))`.
    sigma: f32,
}

impl Awgn {
    /// Constructs an AWGN channel stage.
    ///
    /// # Arguments
    ///
    /// * `es_n0_db` — channel Es/N0 in dB.
    /// * `bits_per_symbol` — modulation order in bits/symbol.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::channels::awgn::Awgn;
    ///
    /// let ch = Awgn::new(10.0, 4);
    /// assert_eq!(ch.bits_per_symbol(), 4);
    /// ```
    #[must_use]
    pub fn new(es_n0_db: f32, bits_per_symbol: usize) -> Self {
        let sigma = crate::channels::es_n0_db_to_sigma(es_n0_db);
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

    /// Per-axis noise standard deviation `sigma = sqrt(1 / (2 * 10^(Es/N0_dB/10)))`.
    #[inline]
    #[must_use]
    pub fn sigma(&self) -> f32 {
        self.sigma
    }

    /// Applies AWGN noise to `batch` in-place, drawing noise from `rng`.
    ///
    /// Each symbol has its I and Q components independently corrupted by
    /// `N(0, sigma^2)` noise. Each Gaussian sample consumes exactly 4 ChaCha20
    /// 32-bit words (two `f64` uniform draws via Box-Muller).
    ///
    /// # Arguments
    ///
    /// * `batch` — the IQ symbol batch to corrupt in-place.
    /// * `rng` — noise RNG; the caller must have seeked it to the frame's §3
    ///   word-position offset before calling this method.
    ///
    /// # Complexity
    ///
    /// O(N) where N is the total number of symbols across all frames in the batch.
    pub fn apply(&self, batch: &mut SymbolBatch, rng: &mut ChaCha20Rng) {
        let pos_before = rng.get_word_pos();
        for (i_frame, q_frame) in batch.i.iter_mut().zip(batch.q.iter_mut()) {
            for (xi, xq) in i_frame.iter_mut().zip(q_frame.iter_mut()) {
                let u1: f64 = rng.random();
                let u2: f64 = rng.random();
                let n_i = box_muller_cos(u1, u2) * self.sigma;
                let u3: f64 = rng.random();
                let u4: f64 = rng.random();
                let n_q = box_muller_cos(u3, u4) * self.sigma;
                *xi += n_i;
                *xq += n_q;
            }
        }
        let noise_words_drawn = rng.get_word_pos().saturating_sub(pos_before);
        debug_assert!(
            noise_words_drawn <= FRAME_STRIDE - 256,
            "AWGN noise draw {noise_words_drawn} words exceeded FRAME_STRIDE - 256 = {}",
            FRAME_STRIDE - 256
        );
    }
}

impl Stage<SymbolBatch, SymbolBatch> for Awgn {
    type Scratch = ChannelScratch;
    type CpuFallback = Self;

    /// Adds AWGN noise to a copy of `input`, drawing from `scratch.rng`.
    ///
    /// # Errors
    ///
    /// This stage is infallible in release builds; the debug-budget assertion
    /// panics only in debug builds if the draw exceeds `FRAME_STRIDE - 256`.
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
    use rand::SeedableRng;

    fn make_batch(frames: usize, syms: usize) -> SymbolBatch {
        let i = vec![vec![1.0_f32; syms]; frames];
        let q = vec![vec![0.0_f32; syms]; frames];
        SymbolBatch::new(i, q)
    }

    #[test]
    fn test_awgn_new_sigma_formula() {
        // sigma = sqrt(1 / (2 * 10^(es_n0_db / 10)))
        let ch = Awgn::new(0.0, 4);
        let expected = (1.0_f32 / 2.0).sqrt();
        assert!(
            (ch.sigma() - expected).abs() < 1e-6,
            "sigma mismatch at 0 dB"
        );
    }

    #[test]
    fn test_awgn_apply_preserves_mean() {
        // Over many samples the mean of noise is ~0; original symbols are 1+0j.
        let ch = Awgn::new(10.0, 4);
        let input = make_batch(1, 1000);
        let mut batch = input.clone();
        let mut rng = ChaCha20Rng::seed_from_u64(42);
        ch.apply(&mut batch, &mut rng);
        let mean_i: f32 = batch.i[0].iter().sum::<f32>() / 1000.0;
        // Mean should be close to 1.0 (original) ± a few sigma / sqrt(N).
        assert!(
            (mean_i - 1.0).abs() < 0.1,
            "mean I component too far from 1.0: {mean_i}"
        );
    }

    #[test]
    fn test_awgn_stage_process_clones_input() {
        let ch = Awgn::new(10.0, 4);
        let input = make_batch(2, 100);
        let mut scratch = ChannelScratch::default();
        let out = ch.process(&input, &mut scratch).unwrap();
        // Output has same dimensions.
        assert_eq!(out.batch_size(), 2);
        assert_eq!(out.i[0].len(), 100);
    }
}
