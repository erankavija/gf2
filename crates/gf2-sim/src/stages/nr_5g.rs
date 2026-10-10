//! 5G NR LDPC BICM-chain [`Stage`] wrappers (`@/citation/ThreeGpp2017`) over
//! the `gf2-coding` 5G NR types ([`Nr5gRateMatchedCode`] encode,
//! [`Nr5gRateMatchedBatchDecoder`] decode, and the `@/citation/ThreeGpp2020`
//! clause 5.4.2.2 [`interleaver`](gf2_coding::ldpc::nr_5g::interleaver)). Every
//! stage is `ExecutionClass::CpuOnly` with `CpuFallback = Self` and
//! `Scratch = ()`, except [`Nr5gDecode`], whose [`BatchDecodeScratch`] exposes
//! the per-frame BP iteration counts.

use std::sync::Arc;

use gf2_coding::ldpc::nr_5g::interleaver::{deinterleave_llrs, interleave_bits};
use gf2_coding::ldpc::nr_5g::{Nr5gRateMatchedBatchDecoder, Nr5gRateMatchedCode};
use gf2_coding::ldpc::DecoderAlgorithm;
use gf2_coding::modem::DemapMethod;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::tuning::{self, DecodeSelectors};

use crate::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
use crate::error::StageError;
use crate::stage::{ExecutionClass, Stage};
use crate::stages::batch_decode::{BatchDecode, BatchDecodeScratch};
use crate::stages::{GrayQamDemapCore, GrayQamMapCore, DEFAULT_DEMAP_NOISE_VAR};

/// 5G NR LDPC encode stage: `target_k` message bits → `target_n` codeword bits.
///
/// Wraps [`Nr5gRateMatchedCode::encode`]. Each input frame must be exactly
/// `k()` bits; each output frame is `n()` bits.
pub struct Nr5gEncode {
    code: Arc<Nr5gRateMatchedCode>,
}

impl Nr5gEncode {
    /// Builds a 5G NR encode stage over a shared rate-matched code.
    pub fn new(code: Arc<Nr5gRateMatchedCode>) -> Self {
        Self { code }
    }

    /// The message length `k` (= `target_k`) this stage encodes.
    #[inline]
    #[must_use]
    pub fn k(&self) -> usize {
        self.code.k()
    }

    /// The codeword length `n` (= `target_n`, the rate-matched length `E`)
    /// this stage emits per frame.
    #[inline]
    #[must_use]
    pub fn n(&self) -> usize {
        self.code.n()
    }
}

impl Stage<BitPackedBatch, BitPackedBatch> for Nr5gEncode {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(
        &self,
        input: &BitPackedBatch,
        _scratch: &mut (),
    ) -> Result<BitPackedBatch, StageError> {
        let frames = input
            .frames
            .iter()
            .map(|msg| self.code.encode(msg))
            .collect();
        Ok(BitPackedBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// 5G NR bit-interleave stage: rate-matched bits → interleaved bits.
///
/// Wraps [`interleave_bits`] at modulation order `q_m`. Each frame length must
/// be a multiple of `q_m`.
pub struct Nr5gBitInterleave {
    q_m: usize,
}

impl Nr5gBitInterleave {
    /// Builds a bit-interleave stage for `q_m` bits per QAM symbol.
    pub fn new(q_m: usize) -> Self {
        Self { q_m }
    }
}

impl Stage<BitPackedBatch, BitPackedBatch> for Nr5gBitInterleave {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(
        &self,
        input: &BitPackedBatch,
        _scratch: &mut (),
    ) -> Result<BitPackedBatch, StageError> {
        let frames = input
            .frames
            .iter()
            .map(|frame| interleave_bits(frame, self.q_m))
            .collect();
        Ok(BitPackedBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// 5G NR LLR-deinterleave stage: interleaved LLRs → rate-matched LLRs.
///
/// Wraps [`deinterleave_llrs`],
/// the receive-path inverse of [`Nr5gBitInterleave`] operating in the LLR
/// domain. Each frame length must be a multiple of `q_m`.
pub struct Nr5gLlrDeinterleave {
    q_m: usize,
}

impl Nr5gLlrDeinterleave {
    /// Builds an LLR-deinterleave stage for `q_m` bits per QAM symbol.
    pub fn new(q_m: usize) -> Self {
        Self { q_m }
    }
}

impl Stage<LlrBatch, LlrBatch> for Nr5gLlrDeinterleave {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &LlrBatch, _scratch: &mut ()) -> Result<LlrBatch, StageError> {
        let frames = input
            .frames
            .iter()
            .map(|llrs| deinterleave_llrs(llrs, self.q_m))
            .collect();
        Ok(LlrBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// Gray-QAM map stage for 5G NR: interleaved coded bits → IQ symbols.
///
/// Wraps [`GrayQamMapper`](gf2_coding::modem::GrayQamMapper)`::map_bits` at
/// constellation order `2^q_m`, `q_m ∈ {2, 4, 6, 8}` (QPSK / 16-QAM / 64-QAM /
/// 256-QAM). Each input frame's bit count must be a multiple of `q_m`.
pub struct NrGrayQamMap {
    core: GrayQamMapCore,
}

impl NrGrayQamMap {
    /// Builds a Gray-QAM map stage for `q_m` bits per QAM symbol.
    pub fn new(q_m: usize) -> Self {
        Self {
            core: GrayQamMapCore::new(q_m),
        }
    }
}

impl Stage<BitPackedBatch, SymbolBatch> for NrGrayQamMap {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(
        &self,
        input: &BitPackedBatch,
        _scratch: &mut (),
    ) -> Result<SymbolBatch, StageError> {
        Ok(self.core.map_batch(input))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// Gray-QAM soft-demap stage for 5G NR: IQ symbols → soft LLRs.
///
/// Wraps [`FastGrayQamDemapper`](gf2_coding::modem::FastGrayQamDemapper)`::demap_llrs`
/// under AWGN-shaped log-MAP at constellation order `2^q_m`. Each input frame of
/// `s` symbols produces `s * q_m` LLRs. The per-symbol total noise variance
/// (`N0`) defaults to [`DEFAULT_DEMAP_NOISE_VAR`]; set the
/// true channel `N0` via [`NrGrayQamDemap::with_noise_var`].
pub struct NrGrayQamDemap {
    core: GrayQamDemapCore,
}

impl NrGrayQamDemap {
    /// Builds an NR Gray-QAM demap stage with [`DEFAULT_DEMAP_NOISE_VAR`].
    pub fn new(q_m: usize, method: DemapMethod) -> Self {
        Self::with_noise_var(q_m, method, DEFAULT_DEMAP_NOISE_VAR)
    }

    /// Builds an NR Gray-QAM demap stage with the per-symbol total complex
    /// AWGN noise variance `noise_var` (`N0 = 2 sigma^2`).
    ///
    /// # Panics
    ///
    /// Panics if `noise_var` is not strictly positive and finite.
    pub fn with_noise_var(q_m: usize, method: DemapMethod, noise_var: f32) -> Self {
        Self {
            core: GrayQamDemapCore::new(q_m, method, noise_var, "NrGrayQamDemap"),
        }
    }

    /// The per-symbol noise variance (`N0 = 2 sigma^2`) this demapper assumes.
    #[inline]
    #[must_use]
    pub fn noise_var(&self) -> f32 {
        self.core.noise_var()
    }
}

impl Stage<SymbolBatch, LlrBatch> for NrGrayQamDemap {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &SymbolBatch, _scratch: &mut ()) -> Result<LlrBatch, StageError> {
        Ok(self.core.demap_batch(input))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// 5G NR LDPC decode stage: rate-matched-order soft LLRs → recovered message
/// bits.
///
/// Decodes each batch through a [`BatchDecode`] stage over one
/// [`Nr5gRateMatchedBatchDecoder`] per scratch (belief propagation on the full
/// mother code with rate-matching LLR mapping). Each input frame is `n()`
/// LLRs; each output frame is `k()` recovered message bits. The per-frame BP
/// iteration counts are recorded into the [`BatchDecodeScratch`].
///
/// A frame whose BP does not converge yields the decoder's message estimate
/// and no stage error.
///
/// # Panics
///
/// `process` panics as [`BatchDecode`] does, and its first call on a scratch
/// as [`Nr5gRateMatchedBatchDecoder::with_selectors`] does.
pub struct Nr5gDecode {
    code: Arc<Nr5gRateMatchedCode>,
    algorithm: DecoderAlgorithm,
    max_iterations: usize,
    selectors: DecodeSelectors,
    batch: BatchDecode<DecoderFactory>,
}

type DecoderFactory = Box<dyn Fn() -> Nr5gRateMatchedBatchDecoder + Send + Sync>;

impl Nr5gDecode {
    /// Builds a 5G NR decode stage over a shared rate-matched code, decoding
    /// with normalized min-sum (`alpha` = 0.75) at the given iteration cap. A
    /// cap of zero behaves as in [`Nr5gRateMatchedBatchDecoder`].
    ///
    /// # Panics
    ///
    /// As [`with_algorithm`](Self::with_algorithm).
    pub fn new(code: Arc<Nr5gRateMatchedCode>, max_iterations: usize) -> Self {
        Self::with_algorithm(
            code,
            DecoderAlgorithm::NormalizedMinSum(0.75),
            max_iterations,
        )
    }

    /// Builds a 5G NR decode stage with an explicit BP algorithm; the cap of
    /// zero behaves as in [`new`](Self::new). The decode selectors are those
    /// of the process-wide coding tuning section, read once here.
    ///
    /// # Panics
    ///
    /// As [`tuning::active`].
    pub fn with_algorithm(
        code: Arc<Nr5gRateMatchedCode>,
        algorithm: DecoderAlgorithm,
        max_iterations: usize,
    ) -> Self {
        let selectors = tuning::active().section.decode().clone();
        Self::with_selectors(code, algorithm, max_iterations, selectors)
    }

    /// Builds a 5G NR decode stage whose decoders take the given decode
    /// selectors, as [`Nr5gRateMatchedBatchDecoder::with_selectors`] does.
    pub fn with_selectors(
        code: Arc<Nr5gRateMatchedCode>,
        algorithm: DecoderAlgorithm,
        max_iterations: usize,
        selectors: DecodeSelectors,
    ) -> Self {
        Self::build(code, algorithm, max_iterations, selectors, false)
    }

    /// Holds the lane route of every decoder on the portable backend, as
    /// [`Nr5gRateMatchedBatchDecoder::with_portable_lanes`] does.
    #[must_use]
    pub fn with_portable_lanes(self) -> Self {
        Self::build(
            self.code,
            self.algorithm,
            self.max_iterations,
            self.selectors,
            true,
        )
    }

    fn build(
        code: Arc<Nr5gRateMatchedCode>,
        algorithm: DecoderAlgorithm,
        max_iterations: usize,
        selectors: DecodeSelectors,
        portable: bool,
    ) -> Self {
        let (shared, chosen) = (code.clone(), selectors.clone());
        let factory: DecoderFactory = Box::new(move || {
            let decoder = Nr5gRateMatchedBatchDecoder::with_selectors(
                (*shared).clone(),
                algorithm,
                max_iterations,
                chosen.clone(),
            );
            if portable {
                decoder.with_portable_lanes()
            } else {
                decoder
            }
        });
        Self {
            code,
            algorithm,
            max_iterations,
            selectors,
            batch: BatchDecode::new(factory),
        }
    }

    /// The recovered message length `k` (= `target_k`).
    #[inline]
    #[must_use]
    pub fn k(&self) -> usize {
        self.code.k()
    }
}

impl Stage<LlrBatch, HardDecisionBatch> for Nr5gDecode {
    type Scratch = BatchDecodeScratch;
    type CpuFallback = Self;

    fn process(
        &self,
        input: &LlrBatch,
        scratch: &mut BatchDecodeScratch,
    ) -> Result<HardDecisionBatch, StageError> {
        self.batch.process(input, scratch)
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    use gf2_coding::ldpc::QuasiCyclicLdpc;
    use gf2_coding::traits::IterativeSoftDecoder;
    use gf2_coding::Llr;
    use gf2_core::BitVec;

    /// Noise amplitudes of consecutive frames, from clean to undecodable, so
    /// a wave holds frames that converge early, late and not at all.
    const NOISE_LADDER: [f32; 6] = [0.8, 1.7, 4.0, 1.5, 1.9, 1.3];

    /// `count` noisy transmitted words of `code`, frame `f` at its entry of
    /// [`NOISE_LADDER`].
    fn noisy_frames(code: &Nr5gRateMatchedCode, count: usize) -> Vec<Vec<Llr>> {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut uniform = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f32 / (1u64 << 53) as f32
        };
        (0..count)
            .map(|f| {
                let sent = code.encode(&BitVec::random_seeded(code.k(), f as u64));
                let amplitude = NOISE_LADDER[f % NOISE_LADDER.len()];
                (0..sent.len())
                    .map(|i| {
                        let symbol = if sent.get(i) { -1.0 } else { 1.0 };
                        let noise = amplitude * (uniform() + uniform() - 1.0);
                        Llr::new(2.0 * (symbol + noise))
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn decode_matches_per_frame_decode_iterative_at_every_batch_size() {
        const WAVE: usize = 8;
        let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
        let cap = 12;
        for algorithm in [
            DecoderAlgorithm::NormalizedMinSum(0.75),
            DecoderAlgorithm::SumProduct,
        ] {
            let stage = Nr5gDecode::with_algorithm(code.clone(), algorithm, cap);
            let mut scratch = BatchDecodeScratch::default();
            let mut single = Nr5gRateMatchedDecoder::with_algorithm((*code).clone(), algorithm);
            for count in [0, 1, WAVE - 3, WAVE, 2 * WAVE + 3] {
                let frames = noisy_frames(&code, count);
                let out = stage
                    .process(&LlrBatch::new(frames.clone()), &mut scratch)
                    .unwrap();

                assert_eq!(out.frames.len(), count);
                assert_eq!(scratch.iterations.len(), count);
                let mut depths = std::collections::BTreeSet::new();
                for (f, llrs) in frames.iter().enumerate() {
                    let expected = single.decode_iterative(llrs, cap);
                    let label = format!("{algorithm:?}, {count} frames, frame {f}");
                    assert_eq!(out.frames[f], expected.decoded_bits, "{label}");
                    assert_eq!(scratch.iterations[f], expected.iterations as u64, "{label}");
                    depths.insert((expected.iterations, expected.converged));
                }
                if count >= WAVE {
                    assert!(depths.len() > 2, "{algorithm:?}: {depths:?}");
                    assert!(depths.iter().any(|(_, converged)| !converged));
                }
            }
        }
    }

    #[test]
    fn test_encode_decode_roundtrip_zero_message() {
        let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
        let enc = Nr5gEncode::new(code.clone());
        let dec = Nr5gDecode::new(code.clone(), 20);
        let msg = BitVec::zeros(121);
        let coded = enc
            .process(&BitPackedBatch::new(vec![msg.clone()]), &mut ())
            .unwrap();
        let llrs: Vec<Llr> = (0..coded.frames[0].len())
            .map(|i| {
                if coded.frames[0].get(i) {
                    Llr::new(-12.0)
                } else {
                    Llr::new(12.0)
                }
            })
            .collect();
        let mut scratch = BatchDecodeScratch::default();
        let out = dec
            .process(&LlrBatch::new(vec![llrs]), &mut scratch)
            .unwrap();
        assert_eq!(out.frames[0], msg, "zero message round-trips");
        assert_eq!(scratch.iterations.len(), 1);
    }

    #[test]
    fn test_interleave_deinterleave_llr_identity() {
        let q_m = 4;
        let inter = Nr5gBitInterleave::new(q_m);
        let deinter = Nr5gLlrDeinterleave::new(q_m);
        let mut frame = BitVec::zeros(q_m * 5);
        for i in (0..frame.len()).step_by(2) {
            frame.set(i, true);
        }
        let interleaved = inter
            .process(&BitPackedBatch::new(vec![frame.clone()]), &mut ())
            .unwrap();
        let llrs: Vec<Llr> = (0..interleaved.frames[0].len())
            .map(|i| {
                if interleaved.frames[0].get(i) {
                    Llr::new(-1.0)
                } else {
                    Llr::new(1.0)
                }
            })
            .collect();
        let recovered = deinter
            .process(&LlrBatch::new(vec![llrs]), &mut ())
            .unwrap();
        for i in 0..frame.len() {
            let bit = recovered.frames[0][i].value() < 0.0;
            assert_eq!(bit, frame.get(i), "position {i} must round-trip");
        }
    }
}
