//! Rician flat-fading channel stage.

use rand_chacha::ChaCha20Rng;

use crate::batch::SymbolBatch;
use crate::channels::awgn::ChannelScratch;
use crate::channels::{draw_cn01, draw_standard_normal};
use crate::error::StageError;
use crate::parallel::{WorkerCtx, FRAME_STRIDE};
use crate::stage::{ExecutionClass, Stage};

/// Rician flat-fading channel stage.
///
/// The K-factor is the ratio of line-of-sight power to scattered power. Each
/// symbol `x` becomes
///
/// ```text
/// h = los_mag + scatter * CN(0,1)
/// r = h * x + n
/// ```
///
/// where `los_mag = sqrt(K/(K+1))` and `scatter = sqrt(1/(K+1))`, so
/// `E[|h|^2] = 1` for all K, and `n` is complex AWGN of per-axis variance
/// `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))`. `K = 0` is Rayleigh fading.
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
    /// Constructs a Rician fading channel stage; `bits_per_symbol` does not
    /// enter the channel model.
    ///
    /// # Panics
    ///
    /// Panics if `k_factor` is negative or not finite.
    #[must_use]
    pub fn new(es_n0_db: f32, bits_per_symbol: usize, k_factor: f32) -> Self {
        assert!(
            k_factor >= 0.0 && k_factor.is_finite(),
            "k_factor must be non-negative and finite, got {k_factor}"
        );
        let sigma = crate::channels::es_n0_db_to_sigma(es_n0_db);
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

    /// Seeks `ctx`'s RNG to frame `frame_idx_in_worker` and applies the channel.
    pub fn apply_for_frame(
        &self,
        batch: &mut SymbolBatch,
        ctx: &mut WorkerCtx,
        frame_idx_in_worker: usize,
    ) {
        ctx.reseek_to_frame(frame_idx_in_worker);
        self.apply(batch, ctx.rng_mut());
    }

    /// Applies Rician fading and AWGN to `batch` in-place, drawing from `rng`
    /// at its current position.
    ///
    /// Per symbol, the `CN(0, 1)` scatter draw comes first and then `n`,
    /// consuming 16 ChaCha20 32-bit words (8 each).
    ///
    /// # Panics
    ///
    /// In debug builds, if the call draws more than `FRAME_STRIDE - 256`
    /// ChaCha20 words.
    pub fn apply(&self, batch: &mut SymbolBatch, rng: &mut ChaCha20Rng) {
        let pos_before = rng.get_word_pos();
        for (i_frame, q_frame) in batch.i.iter_mut().zip(batch.q.iter_mut()) {
            for (xi, xq) in i_frame.iter_mut().zip(q_frame.iter_mut()) {
                let (v_r, v_i) = draw_cn01(rng);

                let h_r = self.los_mag + self.scatter * v_r;
                let h_i = self.scatter * v_i;

                let x_i = *xi;
                let x_q = *xq;
                let r_i = h_r * x_i - h_i * x_q;
                let r_q = h_r * x_q + h_i * x_i;

                let n_i = draw_standard_normal(rng) * self.sigma;
                let n_q = draw_standard_normal(rng) * self.sigma;

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
    /// `scratch.rng`; never returns `Err`.
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
