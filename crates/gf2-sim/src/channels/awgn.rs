//! AWGN channel stage: additive white Gaussian noise on IQ symbol batches.

use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

use crate::batch::SymbolBatch;
use crate::channels::draw_standard_normal;
use crate::error::StageError;
use crate::parallel::{WorkerCtx, FRAME_STRIDE};
use crate::stage::{ExecutionClass, Stage};

/// Per-stage scratch of the channel stages: a [`ChaCha20Rng`].
///
/// `Default` seeds the RNG with 0.
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

/// AWGN channel stage: adds circularly-symmetric complex Gaussian noise to
/// every symbol in a [`SymbolBatch`].
///
/// The noise variance is `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))` per axis.
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
    /// Constructs an AWGN channel stage; `bits_per_symbol` does not enter the
    /// noise variance.
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

    /// Seeks `ctx`'s RNG to frame `frame_idx_in_worker` and applies AWGN noise.
    pub fn apply_for_frame(
        &self,
        batch: &mut SymbolBatch,
        ctx: &mut WorkerCtx,
        frame_idx_in_worker: usize,
    ) {
        ctx.reseek_to_frame(frame_idx_in_worker);
        self.apply(batch, ctx.rng_mut());
    }

    /// Applies AWGN noise to `batch` in-place, drawing from `rng` at its
    /// current position.
    ///
    /// Within each frame every I-axis sample is drawn first, then every Q-axis
    /// sample, the draw order of
    /// [`BicmAwgnChannel::transmit_and_demodulate_with_noise`](gf2_coding::dvb_t2_bicm_harness::BicmAwgnChannel::transmit_and_demodulate_with_noise).
    /// Each sample consumes 4 ChaCha20 32-bit words.
    ///
    /// # Panics
    ///
    /// In debug builds, if the call draws more than `FRAME_STRIDE - 256`
    /// ChaCha20 words.
    pub fn apply(&self, batch: &mut SymbolBatch, rng: &mut ChaCha20Rng) {
        let pos_before = rng.get_word_pos();
        for (i_frame, q_frame) in batch.i.iter_mut().zip(batch.q.iter_mut()) {
            for xi in i_frame.iter_mut() {
                *xi += draw_standard_normal(rng) * self.sigma;
            }
            for xq in q_frame.iter_mut() {
                *xq += draw_standard_normal(rng) * self.sigma;
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

    /// Adds AWGN noise to a copy of `input`, drawing from `scratch.rng`; never
    /// returns `Err`.
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
        let ch = Awgn::new(10.0, 4);
        let input = make_batch(1, 1000);
        let mut batch = input.clone();
        let mut rng = ChaCha20Rng::seed_from_u64(42);
        ch.apply(&mut batch, &mut rng);
        let mean_i: f32 = batch.i[0].iter().sum::<f32>() / 1000.0;
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
        assert_eq!(out.batch_size(), 2);
        assert_eq!(out.i[0].len(), 100);
    }
}
