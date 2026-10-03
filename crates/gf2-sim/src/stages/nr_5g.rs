//! 5G NR LDPC BICM-chain [`Stage`] wrappers (`@/citation/ThreeGpp2017`).
//!
//! The stages wrap the `gf2-coding` 5G NR types ([`Nr5gRateMatchedCode`]
//! encode, [`Nr5gRateMatchedDecoder`] decode, and the Section 5.4.2.2
//! [`interleaver`](gf2_coding::ldpc::nr_5g::interleaver)). Every stage is
//! `ExecutionClass::CpuOnly` with `CpuFallback = Self` and `Scratch = ()`,
//! except [`Nr5gDecode`], whose [`Nr5gDecodeScratch`] exposes the per-frame BP
//! iteration counts.

use std::sync::Arc;

use gf2_coding::ldpc::nr_5g::interleaver::{deinterleave_llrs, interleave_bits};
use gf2_coding::ldpc::nr_5g::{Nr5gRateMatchedCode, Nr5gRateMatchedDecoder};
use gf2_coding::ldpc::DecoderAlgorithm;
use gf2_coding::modem::DemapMethod;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::BitVec;

use crate::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
use crate::error::StageError;
use crate::stage::{ExecutionClass, Stage};
use crate::stages::{GrayQamDemapCore, GrayQamMapCore, DEFAULT_DEMAP_NOISE_VAR};


/// 5G NR LDPC encode stage: `target_k` message bits → `target_n` codeword bits.
///
/// Wraps [`Nr5gRateMatchedCode::encode`]. Each input frame must be exactly
/// `k()` bits; each output frame is `n()` bits.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use gf2_sim::stages::nr_5g::Nr5gEncode;
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::QuasiCyclicLdpc;
/// use gf2_core::BitVec;
///
/// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
/// let stage = Nr5gEncode::new(code.clone());
/// let msg = BitVec::zeros(121);
/// let out = stage.process(&BitPackedBatch::new(vec![msg]), &mut ()).unwrap();
/// assert_eq!(out.frames[0].len(), 256);
/// ```
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


/// 5G NR §5.4.2.2 bit-interleave stage: rate-matched bits → interleaved bits.
///
/// Wraps [`interleave_bits`] at modulation order `q_m`. Each frame length must
/// be a multiple of `q_m`.
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::nr_5g::Nr5gBitInterleave;
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_core::BitVec;
///
/// let stage = Nr5gBitInterleave::new(2);
/// let frame = BitVec::zeros(6);
/// let out = stage.process(&BitPackedBatch::new(vec![frame]), &mut ()).unwrap();
/// assert_eq!(out.frames[0].len(), 6);
/// ```
pub struct Nr5gBitInterleave {
    q_m: usize,
}

impl Nr5gBitInterleave {
    /// Builds a §5.4.2.2 bit-interleave stage for `q_m` bits per QAM symbol.
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

/// 5G NR §5.4.2.2 LLR-deinterleave stage: interleaved LLRs → rate-matched LLRs.
///
/// Wraps [`deinterleave_llrs`],
/// the receive-path inverse of [`Nr5gBitInterleave`] operating in the LLR
/// domain. Each frame length must be a multiple of `q_m`.
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::nr_5g::Nr5gLlrDeinterleave;
/// use gf2_sim::batch::LlrBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::Llr;
///
/// let stage = Nr5gLlrDeinterleave::new(2);
/// let frame = vec![Llr::new(1.0); 6];
/// let out = stage.process(&LlrBatch::new(vec![frame]), &mut ()).unwrap();
/// assert_eq!(out.frames[0].len(), 6);
/// ```
pub struct Nr5gLlrDeinterleave {
    q_m: usize,
}

impl Nr5gLlrDeinterleave {
    /// Builds a §5.4.2.2 LLR-deinterleave stage for `q_m` bits per QAM symbol.
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
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::nr_5g::NrGrayQamMap;
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_core::BitVec;
///
/// let stage = NrGrayQamMap::new(4); // 16-QAM
/// let frame = BitVec::zeros(8); // 2 symbols of 4 bits
/// let out = stage.process(&BitPackedBatch::new(vec![frame]), &mut ()).unwrap();
/// assert_eq!(out.i[0].len(), 2);
/// ```
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
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::nr_5g::{NrGrayQamMap, NrGrayQamDemap};
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::modem::DemapMethod;
/// use gf2_core::BitVec;
///
/// let map = NrGrayQamMap::new(4);
/// let demap = NrGrayQamDemap::new(4, DemapMethod::ExactLogMap);
/// let syms = map.process(&BitPackedBatch::new(vec![BitVec::zeros(8)]), &mut ()).unwrap();
/// let llrs = demap.process(&syms, &mut ()).unwrap();
/// assert_eq!(llrs.frames[0].len(), 8);
/// ```
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


/// Per-stage scratch for [`Nr5gDecode`]: the per-frame BP iteration counts of
/// the most recent `process` call, in input-frame order.
///
/// [`Nr5gDecode::process`] clears [`iterations`](Self::iterations) and pushes
/// one entry per input frame: the BP depth reported by
/// [`Nr5gRateMatchedDecoder::decode_iterative`].
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::nr_5g::Nr5gDecodeScratch;
///
/// let scratch = Nr5gDecodeScratch::default();
/// assert!(scratch.iterations.is_empty());
/// ```
#[derive(Debug, Clone, Default)]
pub struct Nr5gDecodeScratch {
    /// Per-frame BP iteration counts of the most recent [`Nr5gDecode::process`]
    /// call, in input-frame order.
    pub iterations: Vec<u64>,
}

/// 5G NR LDPC decode stage: rate-matched-order soft LLRs → recovered message
/// bits.
///
/// Wraps [`Nr5gRateMatchedDecoder::decode_iterative`] (belief propagation on the
/// full mother code with rate-matching LLR mapping). Each input frame is `n()`
/// LLRs; each output frame is `k()` recovered message bits. The per-frame BP
/// iteration counts are recorded into the [`Nr5gDecodeScratch`].
///
/// A frame whose BP does not converge yields the decoder's message estimate
/// and no stage error.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use gf2_sim::stages::nr_5g::{Nr5gDecode, Nr5gDecodeScratch};
/// use gf2_sim::batch::LlrBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::QuasiCyclicLdpc;
/// use gf2_coding::Llr;
///
/// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
/// let stage = Nr5gDecode::new(code.clone(), 20);
/// // All-zero codeword: strongly positive LLRs decode to the zero message.
/// let llrs = vec![Llr::new(10.0); 256];
/// let mut scratch = Nr5gDecodeScratch::default();
/// let out = stage.process(&LlrBatch::new(vec![llrs]), &mut scratch).unwrap();
/// assert_eq!(out.frames[0].len(), 121);
/// assert_eq!(scratch.iterations.len(), 1);
/// ```
pub struct Nr5gDecode {
    code: Arc<Nr5gRateMatchedCode>,
    algorithm: DecoderAlgorithm,
    max_iterations: usize,
}

impl Nr5gDecode {
    /// Builds a 5G NR decode stage over a shared rate-matched code, decoding
    /// with normalized min-sum (`alpha` = 0.75) at the given iteration cap.
    pub fn new(code: Arc<Nr5gRateMatchedCode>, max_iterations: usize) -> Self {
        Self::with_algorithm(
            code,
            DecoderAlgorithm::NormalizedMinSum(0.75),
            max_iterations,
        )
    }

    /// Builds a 5G NR decode stage with an explicit BP algorithm.
    pub fn with_algorithm(
        code: Arc<Nr5gRateMatchedCode>,
        algorithm: DecoderAlgorithm,
        max_iterations: usize,
    ) -> Self {
        Self {
            code,
            algorithm,
            max_iterations,
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
    type Scratch = Nr5gDecodeScratch;
    type CpuFallback = Self;

    fn process(
        &self,
        input: &LlrBatch,
        scratch: &mut Nr5gDecodeScratch,
    ) -> Result<HardDecisionBatch, StageError> {
        scratch.iterations.clear();
        // The decoder owns mutable BP state, and `process` takes `&self`, so
        // each frame builds a decoder from a clone of the shared code.
        let mut frames: Vec<BitVec> = Vec::with_capacity(input.frames.len());
        for llrs in &input.frames {
            let mut decoder =
                Nr5gRateMatchedDecoder::with_algorithm((*self.code).clone(), self.algorithm);
            let result = decoder.decode_iterative(llrs, self.max_iterations);
            scratch.iterations.push(result.iterations as u64);
            frames.push(result.decoded_bits);
        }
        Ok(HardDecisionBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::ldpc::QuasiCyclicLdpc;
    use gf2_coding::Llr;

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
        let mut scratch = Nr5gDecodeScratch::default();
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
