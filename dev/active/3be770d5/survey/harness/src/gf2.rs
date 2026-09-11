//! Per-worker state of the gf2 arm.

use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder};
use gf2_coding::llr::Llr;
use std::hint::black_box;

/// The decoder settings a case declares.
#[must_use]
pub fn config(normalization_factor: f32, syndrome_stopping: bool) -> DecoderConfig {
    DecoderConfig::new(
        DecoderAlgorithm::NormalizedMinSum(normalization_factor),
        syndrome_stopping,
    )
}

/// One worker's reused decoder and buffers.
pub struct Gf2Worker {
    decoder: LdpcDecoder,
    packed: Vec<Llr>,
    n: usize,
    k: usize,
    cap: usize,
    /// Information-window decisions of the last batch, `k` bytes per frame.
    pub decisions: Vec<u8>,
    /// Iterations of each frame of the last batch.
    pub iterations: Vec<u32>,
}

impl Gf2Worker {
    /// Builds a worker decoder for a clone of `code`.
    #[must_use]
    pub fn new(code: &LdpcCode, config: DecoderConfig, batch: usize, cap: usize) -> Self {
        let (n, k) = (code.n(), code.k());
        Self {
            decoder: LdpcDecoder::with_config(code.clone(), config),
            packed: vec![Llr::zero(); n],
            n,
            k,
            cap,
            decisions: vec![0; batch * k],
            iterations: vec![0; batch],
        }
    }

    /// Decodes every frame of `llrs`, `n` values per frame: converts the
    /// recorded f32 values into the decoder's `Llr`, decodes, and extracts the
    /// information-window hard decisions.
    ///
    /// # Panics
    ///
    /// Panics when `llrs` holds another frame count than the worker's batch.
    pub fn decode_batch(&mut self, llrs: &[f32]) {
        assert_eq!(llrs.len(), self.iterations.len() * self.n, "batch size");
        for (frame, recorded) in llrs.chunks_exact(self.n).enumerate() {
            for (packed, value) in self.packed.iter_mut().zip(recorded) {
                *packed = Llr::new(*value);
            }
            let result = self
                .decoder
                .decode_to_codeword(black_box(&self.packed), self.cap);
            let decided = &mut self.decisions[frame * self.k..(frame + 1) * self.k];
            for (position, bit) in decided.iter_mut().enumerate() {
                *bit = u8::from(result.decoded_bits.get(position));
            }
            self.iterations[frame] = result.iterations as u32;
        }
        black_box(&self.decisions);
    }
}
