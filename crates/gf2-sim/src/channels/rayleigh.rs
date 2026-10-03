//! Rayleigh flat-fading channel stage.

use rand_chacha::ChaCha20Rng;

use crate::batch::SymbolBatch;
use crate::channels::awgn::ChannelScratch;
use crate::channels::{draw_cn01, draw_standard_normal};
use crate::error::StageError;
use crate::parallel::{WorkerCtx, FRAME_STRIDE};
use crate::stage::{ExecutionClass, Stage};

/// Rayleigh flat-fading channel stage.
///
/// Each symbol `x` becomes `r = h * x + n` with an independent
/// `h ~ CN(0, 1)` (`E[|h|^2] = 1`) and complex AWGN `n` of per-axis variance
/// `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))`.
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
    /// Constructs a Rayleigh fading channel stage; `bits_per_symbol` does not
    /// enter the channel model.
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

    /// Applies Rayleigh fading and AWGN to `batch` in-place, drawing from `rng`
    /// at its current position.
    ///
    /// Per symbol, `h` is drawn first and then `n`, consuming 16 ChaCha20
    /// 32-bit words (8 for `h`, 8 for `n`).
    ///
    /// # Panics
    ///
    /// In debug builds, if the call draws more than `FRAME_STRIDE - 256`
    /// ChaCha20 words.
    pub fn apply(&self, batch: &mut SymbolBatch, rng: &mut ChaCha20Rng) {
        let pos_before = rng.get_word_pos();
        for (i_frame, q_frame) in batch.i.iter_mut().zip(batch.q.iter_mut()) {
            for (xi, xq) in i_frame.iter_mut().zip(q_frame.iter_mut()) {
                let (h_r, h_i) = draw_cn01(rng);

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
            "Rayleigh draw {noise_words_drawn} words exceeded FRAME_STRIDE - 256 = {}",
            FRAME_STRIDE - 256
        );
    }
}

impl Stage<SymbolBatch, SymbolBatch> for Rayleigh {
    type Scratch = ChannelScratch;
    type CpuFallback = Self;

    /// Applies Rayleigh fading and AWGN to a copy of `input`, drawing from
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
