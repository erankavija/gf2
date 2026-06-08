//! Rician flat-fading channel stage.
//!
//! This module provides the [`Rician`] [`Stage`](crate::Stage) impl, which
//! models frequency-flat Rician fading. The K-factor parametrises the ratio of
//! the direct-path (line-of-sight) power to the scattered-path power. The
//! fading coefficient is:
//!
//! ```text
//! h = sqrt(K/(K+1)) + sqrt(1/(K+1)) * CN(0,1)
//! ```
//!
//! so `E[|h|^2] = 1` for all K ≥ 0. When K = 0, the model reduces to Rayleigh.
//! For K → ∞, the channel becomes AWGN (fixed h = 1).
//!
//! # Signal model
//!
//! ```text
//! r = h * x + n,    n ~ CN(0, 2*sigma^2)
//! ```
//!
//! # Noise model
//!
//! `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))` — same formula as AWGN/Rayleigh
//! (SSOT in `frame_sim.rs`). Fading and noise draws are interleaved from the
//! **same** ChaCha20 stream, consuming **8 ChaCha20 32-bit words per symbol**.
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

/// Rician flat-fading channel stage.
///
/// The Rician K-factor determines the ratio of line-of-sight (deterministic)
/// power to scatter (random) power. The channel applies:
///
/// ```text
/// h = los_mag + scatter * CN(0,1)
/// r = h * x + n
/// ```
///
/// where `los_mag = sqrt(K/(K+1))` and `scatter = sqrt(1/(K+1))`, so
/// `E[|h|^2] = 1` for all K.
///
/// # Arguments (constructor)
///
/// * `es_n0_db` — channel Es/N0 in dB.
/// * `bits_per_symbol` — modulation order in bits/symbol (stored for diagnostics).
/// * `k_factor` — Rician K-factor (≥ 0). K=0 gives Rayleigh; large K approaches AWGN.
///
/// # Examples
///
/// ```
/// use gf2_sim::channels::rician::Rician;
///
/// let ch = Rician::new(6.25, 4, 3.0);
/// assert_eq!(ch.bits_per_symbol(), 4);
/// assert!((ch.k_factor() - 3.0).abs() < 1e-7);
/// let sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(6.25 / 10.0));
/// let expected_sigma = (sigma_sq as f32).sqrt();
/// assert!((ch.sigma() - expected_sigma).abs() < 1e-7);
/// ```
#[derive(Debug, Clone)]
pub struct Rician {
    /// Channel Es/N0 in dB.
    es_n0_db: f32,
    /// Modulation order (bits/symbol).
    bits_per_symbol: usize,
    /// Rician K-factor.
    k_factor: f32,
    /// Per-axis AWGN noise standard deviation.
    sigma: f32,
    /// Line-of-sight component magnitude: `sqrt(K/(K+1))`.
    los_mag: f32,
    /// Scatter component scale: `sqrt(1/(K+1))` — applied to the CN(0,1) draw.
    scatter: f32,
}

impl Rician {
    /// Constructs a Rician fading channel stage.
    ///
    /// # Arguments
    ///
    /// * `es_n0_db` — channel Es/N0 in dB.
    /// * `bits_per_symbol` — modulation order in bits/symbol.
    /// * `k_factor` — Rician K-factor (≥ 0).
    ///
    /// # Panics
    ///
    /// Panics if `k_factor < 0.0` or is NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::channels::rician::Rician;
    ///
    /// let ch = Rician::new(10.0, 4, 2.0);
    /// assert_eq!(ch.bits_per_symbol(), 4);
    /// assert!((ch.k_factor() - 2.0).abs() < 1e-6);
    /// ```
    #[must_use]
    pub fn new(es_n0_db: f32, bits_per_symbol: usize, k_factor: f32) -> Self {
        assert!(
            k_factor >= 0.0 && k_factor.is_finite(),
            "k_factor must be non-negative and finite, got {k_factor}"
        );
        let es_n0_lin = 10.0_f64.powf(es_n0_db as f64 / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        let sigma = (sigma_sq as f32).sqrt();
        let los_mag = (k_factor / (k_factor + 1.0)).sqrt();
        let scatter = (1.0_f32 / (k_factor + 1.0)).sqrt();
        Self {
            es_n0_db,
            bits_per_symbol,
            k_factor,
            sigma,
            los_mag,
            scatter,
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

    /// The Rician K-factor.
    #[inline]
    #[must_use]
    pub fn k_factor(&self) -> f32 {
        self.k_factor
    }

    /// Per-axis AWGN noise standard deviation `sigma = sqrt(1 / (2 * 10^(Es/N0_dB/10)))`.
    #[inline]
    #[must_use]
    pub fn sigma(&self) -> f32 {
        self.sigma
    }

    /// Applies Rician fading and AWGN to `batch` in-place, drawing from `rng`.
    ///
    /// For each symbol, draws `v ~ CN(0, 1)` and `n ~ CN(0, 2*sigma^2)` from
    /// `rng`, then computes `h = los_mag + scatter * v` and `r = h * x + n`.
    /// The fading and noise draws are interleaved from the same stream,
    /// consuming **8 ChaCha20 32-bit words per symbol** (4 for `v`, 4 for `n`).
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
                // Draw scatter component v ~ CN(0, 1):
                //   v = (v_r + j*v_i), v_r, v_i ~ N(0, 0.5)
                // Box-Muller gives N(0,1); divide by sqrt(2) for variance 1/2
                // so E[|v|^2] = 1 (same as Rayleigh fading component).
                let u1v: f64 = rng.random();
                let u2v: f64 = rng.random();
                let v_r = box_muller_cos(u1v, u2v) * std::f32::consts::FRAC_1_SQRT_2;
                let u3v: f64 = rng.random();
                let u4v: f64 = rng.random();
                let v_i = box_muller_cos(u3v, u4v) * std::f32::consts::FRAC_1_SQRT_2;

                // Rician fading coefficient:
                //   h = los_mag + scatter * v   (h_r = los_mag + scatter*v_r,
                //                                h_i = scatter*v_i)
                // E[|h|^2] = los_mag^2 + scatter^2 * E[|v|^2]
                //           = K/(K+1)  + 1/(K+1) * 1  = 1  ✓
                let h_r = self.los_mag + self.scatter * v_r;
                let h_i = self.scatter * v_i;

                // Apply fading: r_noiseless = h * x
                let x_i = *xi;
                let x_q = *xq;
                let r_i = h_r * x_i - h_i * x_q;
                let r_q = h_r * x_q + h_i * x_i;

                // Draw AWGN noise n ~ CN(0, 2*sigma^2)
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
            "Rician draw {noise_words_drawn} words exceeded FRAME_STRIDE - 256 = {}",
            FRAME_STRIDE - 256
        );
    }
}

impl Stage<SymbolBatch, SymbolBatch> for Rician {
    type Scratch = ChannelScratch;
    type CpuFallback = Self;

    /// Applies Rician fading and AWGN to a copy of `input`, drawing from
    /// `scratch.rng`.
    ///
    /// # Errors
    ///
    /// This stage is infallible in release builds; the debug assert on the
    /// word budget panics only in debug builds if the draw exceeds the budget.
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
    fn test_rician_sigma_formula() {
        let ch = Rician::new(0.0, 4, 1.0);
        let expected = (1.0_f32 / 2.0).sqrt();
        assert!(
            (ch.sigma() - expected).abs() < 1e-6,
            "sigma mismatch at 0 dB Es/N0"
        );
    }

    #[test]
    fn test_rician_k0_is_rayleigh() {
        // K=0 → los_mag=0, scatter=1 → pure Rayleigh.
        let ch = Rician::new(10.0, 4, 0.0);
        assert!((ch.los_mag).abs() < 1e-7, "los_mag must be 0 when K=0");
        assert!(
            (ch.scatter - 1.0).abs() < 1e-6,
            "scatter must be 1 when K=0"
        );
    }

    #[test]
    fn test_rician_apply_dimensions_preserved() {
        let ch = Rician::new(10.0, 4, 2.0);
        let input = make_batch(2, 50);
        let mut batch = input.clone();
        let mut rng = ChaCha20Rng::seed_from_u64(99);
        ch.apply(&mut batch, &mut rng);
        assert_eq!(batch.batch_size(), 2);
        assert_eq!(batch.i[0].len(), 50);
    }

    #[test]
    fn test_rician_stage_process() {
        let ch = Rician::new(10.0, 4, 3.0);
        let input = make_batch(1, 100);
        let mut scratch = ChannelScratch::default();
        let out = ch.process(&input, &mut scratch).unwrap();
        assert_eq!(out.batch_size(), 1);
        assert_eq!(out.i[0].len(), 100);
    }

    #[test]
    #[should_panic(expected = "k_factor must be non-negative")]
    fn test_rician_negative_k_panics() {
        let _ = Rician::new(10.0, 4, -1.0);
    }
}
