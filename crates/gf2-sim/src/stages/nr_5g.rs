//! 5G NR LDPC BICM-chain [`Stage`] wrappers (3GPP TS 38.212).
//!
//! These adapters wrap the **existing validated** `gf2-coding` 5G NR surface
//! ([`Nr5gRateMatchedCode`] encode, [`Nr5gRateMatchedDecoder`] decode, and the
//! [`interleaver`](gf2_coding::ldpc::nr_5g::interleaver) clause-5.4.2.2 bit
//! interleaver) into the pipeline [`Stage`] form, analogous to the DVB-T2
//! wrappers in [`super`]. None of the LDPC, rate-matching, QAM, or interleaver
//! math is reimplemented here. 5G NR has **no outer code** at the LDPC layer, so
//! the chain is a single inner code (unlike the DVB-T2 BCH+LDPC concatenation).
//!
//! # Stage inventory
//!
//! | Stage | Wraps | Direction |
//! |-------|-------|-----------|
//! | [`Nr5gEncode`] | [`Nr5gRateMatchedCode::encode`] | [`BitPackedBatch`] → [`BitPackedBatch`] |
//! | [`Nr5gBitInterleave`] | [`interleave_bits`](gf2_coding::ldpc::nr_5g::interleaver::interleave_bits) (§5.4.2.2) | [`BitPackedBatch`] → [`BitPackedBatch`] |
//! | [`NrGrayQamMap`] | [`GrayQamMapper::map_bits`] | [`BitPackedBatch`] → [`SymbolBatch`] |
//! | [`NrGrayQamDemap`] | [`FastGrayQamDemapper::demap_llrs`] | [`SymbolBatch`] → [`LlrBatch`] |
//! | [`Nr5gLlrDeinterleave`] | [`deinterleave_llrs`](gf2_coding::ldpc::nr_5g::interleaver::deinterleave_llrs) (§5.4.2.2 inverse) | [`LlrBatch`] → [`LlrBatch`] |
//! | [`Nr5gDecode`] | [`Nr5gRateMatchedDecoder::decode_iterative`] | [`LlrBatch`] → [`HardDecisionBatch`] |
//!
//! The bit interleaver and its LLR-domain inverse are the **5G-NR-specific**
//! §5.4.2.2 interleaver (the DVB-T2 column-row interleaver is a different,
//! DVB-T2-only mapping and is deliberately NOT reused — a hybrid chain would be
//! standards-wrong). Every stage is pure-CPU (`CpuFallback = Self`,
//! `execution_class() == ExecutionClass::CpuOnly`); only [`Nr5gDecode`] carries
//! a non-`()` [`Nr5gDecodeScratch`] so the per-frame BP iteration counts are
//! observable.

use std::sync::Arc;

use gf2_coding::ldpc::nr_5g::interleaver::{deinterleave_llrs, interleave_bits};
use gf2_coding::ldpc::nr_5g::{Nr5gRateMatchedCode, Nr5gRateMatchedDecoder};
use gf2_coding::ldpc::DecoderAlgorithm;
use gf2_coding::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GrayQamMapper,
    ModemSpec,
};
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_coding::Llr;
use gf2_core::BitVec;

use crate::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
use crate::error::StageError;
use crate::stage::{ExecutionClass, Stage};
use crate::stages::DEFAULT_DEMAP_NOISE_VAR;

/// Builds the QAM constellation order `2^q_m` for a modulation order `q_m`.
fn qam_order(q_m: usize) -> usize {
    1usize << q_m
}

// ===========================================================================
// Nr5gEncode
// ===========================================================================

/// 5G NR LDPC encode stage: `target_k` message bits → `target_n` codeword bits.
///
/// Wraps [`Nr5gRateMatchedCode::encode`] (the 3GPP TS 38.212 rate-matched LDPC
/// encoder). Each input frame must be exactly `k()` bits; each output frame is
/// `n()` bits. There is no outer code.
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
    ///
    /// # Arguments
    ///
    /// * `code` — the 3GPP-conformant rate-matched 5G NR LDPC code.
    pub fn new(code: Arc<Nr5gRateMatchedCode>) -> Self {
        Self { code }
    }

    /// The message length `k` (= `target_k`) this stage encodes.
    ///
    /// Exposed so a stage-driven executor can mint a random input of the
    /// correct width.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use gf2_sim::stages::nr_5g::Nr5gEncode;
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
    /// let stage = Nr5gEncode::new(code.clone());
    /// assert_eq!(stage.k(), 121);
    /// ```
    #[inline]
    #[must_use]
    pub fn k(&self) -> usize {
        self.code.k()
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

// ===========================================================================
// Nr5gBitInterleave / Nr5gLlrDeinterleave
// ===========================================================================

/// 5G NR §5.4.2.2 bit-interleave stage: rate-matched bits → interleaved bits.
///
/// Wraps [`interleave_bits`](gf2_coding::ldpc::nr_5g::interleaver::interleave_bits),
/// the TS 38.212 clause 5.4.2.2 block interleaver parameterised by the
/// modulation order `q_m`. Each frame length must be a multiple of `q_m`.
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
    /// Builds a §5.4.2.2 bit-interleave stage for modulation order `q_m`.
    ///
    /// # Arguments
    ///
    /// * `q_m` — bits per QAM symbol (`2`/`4`/`6`/`8`).
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
/// Wraps [`deinterleave_llrs`](gf2_coding::ldpc::nr_5g::interleaver::deinterleave_llrs),
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
    /// Builds a §5.4.2.2 LLR-deinterleave stage for modulation order `q_m`.
    ///
    /// # Arguments
    ///
    /// * `q_m` — bits per QAM symbol (`2`/`4`/`6`/`8`).
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

// ===========================================================================
// NrGrayQamMap
// ===========================================================================

/// Gray-QAM map stage for 5G NR: interleaved coded bits → IQ symbols.
///
/// Wraps [`GrayQamMapper::map_bits`] at constellation order `2^q_m`. Unlike the
/// DVB-T2 [`GrayQamMap`](crate::stages::GrayQamMap) (which keys off
/// `DvbT2Modulation` and tops out at 64-QAM), this stage is parameterised by the
/// raw NR modulation order `q_m ∈ {2, 4, 6, 8}` (QPSK / 16-QAM / 64-QAM /
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
    mapper: GrayQamMapper<f32>,
    q_m: usize,
}

impl NrGrayQamMap {
    /// Builds a Gray-QAM map stage for NR modulation order `q_m`.
    ///
    /// # Arguments
    ///
    /// * `q_m` — bits per QAM symbol (`2`/`4`/`6`/`8`).
    pub fn new(q_m: usize) -> Self {
        Self {
            mapper: GrayQamMapper::from_preset_order(qam_order(q_m)),
            q_m,
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
        let mut i_lanes = Vec::with_capacity(input.frames.len());
        let mut q_lanes = Vec::with_capacity(input.frames.len());
        for frame in &input.frames {
            let num_symbols = frame.len() / self.q_m;
            let bits: Vec<bool> = (0..frame.len()).map(|b| frame.get(b)).collect();
            let mut out_i = vec![0.0_f32; num_symbols];
            let mut out_q = vec![0.0_f32; num_symbols];
            self.mapper.map_bits(&bits, &mut out_i, &mut out_q);
            i_lanes.push(out_i);
            q_lanes.push(out_q);
        }
        Ok(SymbolBatch::new(i_lanes, q_lanes))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

// ===========================================================================
// NrGrayQamDemap
// ===========================================================================

/// Gray-QAM soft-demap stage for 5G NR: IQ symbols → soft LLRs.
///
/// Wraps [`FastGrayQamDemapper::demap_llrs`] under AWGN-shaped log-MAP at
/// constellation order `2^q_m`. Each input frame of `s` symbols produces
/// `s * q_m` LLRs. The per-symbol total noise variance (`N0`) defaults to
/// [`DEFAULT_DEMAP_NOISE_VAR`](crate::stages::DEFAULT_DEMAP_NOISE_VAR); set the
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
    demapper: FastGrayQamDemapper<f32>,
    method: DemapMethod,
    q_m: usize,
    noise_var: f32,
}

impl NrGrayQamDemap {
    /// Builds an NR Gray-QAM demap stage with the default demap noise variance.
    ///
    /// # Arguments
    ///
    /// * `q_m` — bits per QAM symbol (`2`/`4`/`6`/`8`).
    /// * `method` — exact log-MAP or max-log demapping.
    pub fn new(q_m: usize, method: DemapMethod) -> Self {
        Self::with_noise_var(q_m, method, DEFAULT_DEMAP_NOISE_VAR)
    }

    /// Builds an NR Gray-QAM demap stage with an explicit per-symbol noise
    /// variance.
    ///
    /// # Arguments
    ///
    /// * `q_m` — bits per QAM symbol (`2`/`4`/`6`/`8`).
    /// * `method` — exact log-MAP or max-log demapping.
    /// * `noise_var` — per-symbol total complex AWGN noise variance
    ///   (`N0 = 2 sigma^2`); must be strictly positive and finite.
    ///
    /// # Panics
    ///
    /// Panics if `noise_var` is not strictly positive and finite.
    pub fn with_noise_var(q_m: usize, method: DemapMethod, noise_var: f32) -> Self {
        assert!(
            noise_var.is_finite() && noise_var > 0.0,
            "NrGrayQamDemap: noise_var must be finite and > 0, got {noise_var}"
        );
        let spec = ModemSpec::<f32>::gray_square_qam(qam_order(q_m));
        Self {
            demapper: FastGrayQamDemapper::new(spec),
            method,
            q_m,
            noise_var,
        }
    }

    /// The per-symbol noise variance (`N0 = 2 sigma^2`) this demapper assumes.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::stages::nr_5g::NrGrayQamDemap;
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// let d = NrGrayQamDemap::with_noise_var(4, DemapMethod::ExactLogMap, 0.25);
    /// assert_eq!(d.noise_var(), 0.25);
    /// ```
    #[inline]
    #[must_use]
    pub fn noise_var(&self) -> f32 {
        self.noise_var
    }
}

impl Stage<SymbolBatch, LlrBatch> for NrGrayQamDemap {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &SymbolBatch, _scratch: &mut ()) -> Result<LlrBatch, StageError> {
        let frames = input
            .i
            .iter()
            .zip(input.q.iter())
            .map(|(rx_i, rx_q)| {
                let num_symbols = rx_i.len();
                let noise_var = vec![self.noise_var; num_symbols];
                let mut out = vec![Llr::zero(); num_symbols * self.q_m];
                self.demapper.demap_llrs(
                    DemapInput {
                        rx_i,
                        rx_q,
                        gain_i: None,
                        gain_q: None,
                        noise_var: &noise_var,
                        method: self.method,
                    },
                    &mut out,
                );
                out
            })
            .collect();
        Ok(LlrBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

// ===========================================================================
// Nr5gDecode
// ===========================================================================

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
/// iteration counts are recorded into the [`Nr5gDecodeScratch`]. 5G NR has no
/// outer code, so the decoder output is the final message estimate.
///
/// A frame whose BP does not converge is still passed through using its
/// best-effort message estimate; frame-error accounting compares the recovered
/// bits against the transmitted message.
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
    /// with the default normalized-min-sum BP at the given iteration cap.
    ///
    /// # Arguments
    ///
    /// * `code` — the 3GPP-conformant rate-matched 5G NR LDPC code.
    /// * `max_iterations` — the BP iteration cap per frame.
    pub fn new(code: Arc<Nr5gRateMatchedCode>, max_iterations: usize) -> Self {
        // The default `Nr5gRateMatchedDecoder` uses normalized min-sum (α=0.75),
        // the standard 5G NR BP approximation; mirror that here.
        Self::with_algorithm(
            code,
            DecoderAlgorithm::NormalizedMinSum(0.75),
            max_iterations,
        )
    }

    /// Builds a 5G NR decode stage with an explicit BP algorithm.
    ///
    /// # Arguments
    ///
    /// * `code` — the rate-matched 5G NR LDPC code.
    /// * `algorithm` — the check-node update algorithm.
    /// * `max_iterations` — the BP iteration cap per frame.
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
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use gf2_sim::stages::nr_5g::Nr5gDecode;
    /// use gf2_coding::ldpc::QuasiCyclicLdpc;
    ///
    /// let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
    /// let stage = Nr5gDecode::new(code, 20);
    /// assert_eq!(stage.k(), 121);
    /// ```
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
        // The decoder owns mutable BP state; construct one per call from the
        // shared code (cloning the code is the same pattern `decode_soft` uses).
        // Reusing a single decoder across the batch would require &mut self.
        let mut frames: Vec<BitVec> = Vec::with_capacity(input.frames.len());
        for llrs in &input.frames {
            let mut decoder = Nr5gRateMatchedDecoder::with_algorithm(
                (*self.code).clone(),
                self.algorithm,
            );
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

    #[test]
    fn test_qam_order() {
        assert_eq!(qam_order(2), 4);
        assert_eq!(qam_order(4), 16);
        assert_eq!(qam_order(6), 64);
        assert_eq!(qam_order(8), 256);
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
        // Map coded bits to confident LLRs (no channel): +inf-ish for 0, -inf for 1.
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
        // Forward bit interleave then LLR deinterleave is identity on positions.
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
        // Convert interleaved bits to signed LLRs, deinterleave, check signs.
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

    use gf2_coding::ldpc::QuasiCyclicLdpc;
}
