//! Shared DVB-T2 codec + modem [`Stage`] wrappers.
//!
//! This neutral module supplies the DVB-T2 BICM-chain [`Stage`] adapters that
//! the parallel-dispatch (`3fcb7025`), graph-API (`c09d3e95`), and preset
//! (`81d05bab`) waves all reuse, plus a single [`dvb_t2_bicm_stages`] wiring
//! factory so those consumers never re-derive the stage order. Per design-doc
//! §9, the wrappers wrap the **existing validated** `gf2-coding` codec / modem
//! types ([`DvbT2Concat`], [`GrayQamMapper`], [`FastGrayQamDemapper`],
//! [`DvbT2BitInterleaver`]); none of the BCH / LDPC / QAM / interleaver math is
//! reimplemented here.
//!
//! # Stage inventory
//!
//! | Stage | Wraps | Direction |
//! |-------|-------|-----------|
//! | [`DvbT2Encode`] | [`DvbT2Concat::encode`] | [`BitPackedBatch`] → [`BitPackedBatch`] |
//! | [`BitInterleave`] | [`DvbT2BitInterleaver::interleave`] | [`BitPackedBatch`] → [`BitPackedBatch`] |
//! | [`GrayQamMap`] | [`GrayQamMapper::map_bits`] | [`BitPackedBatch`] → [`SymbolBatch`] |
//! | [`GrayQamDemap`] | [`FastGrayQamDemapper::demap_llrs`] | [`SymbolBatch`] → [`LlrBatch`] |
//! | [`BitDeinterleave`] | [`DvbT2BitInterleaver::deinterleave_llrs`] | [`LlrBatch`] → [`LlrBatch`] |
//! | [`DvbT2Decode`] | [`DvbT2Concat::decode_soft_counted`] | [`LlrBatch`] → [`HardDecisionBatch`] |
//! | [`DvbT2BchTail`] | [`DvbT2Concat::decode_bch_from_ldpc_codeword`] | [`HardDecisionBatch`] → [`HardDecisionBatch`] |
//!
//! Every stage is pure-CPU: `CpuFallback = Self`,
//! `execution_class() == ExecutionClass::CpuOnly` (design-doc §1, §8), and
//! `Scratch = ()` — except [`DvbT2Decode`], whose scratch is [`DecodeScratch`]
//! so the per-frame LDPC BP iteration counts are observable by the stage-driven
//! executor (`de160fc5`; the `mean_iters` byte-identity column needs them).
//! [`DvbT2BchTail`] is the outer-decode tail the GPU-offload preset pairs with
//! the `GpuOnly` LDPC decode stage; the all-CPU chain uses the combined
//! [`DvbT2Decode`] instead.
//!
//! # BICM order
//!
//! The forward chain follows the canonical DVB-T2 BICM order also realised by
//! `gf2_coding::dvb_t2_bicm_harness`: encode → bit-interleave → QAM-map →
//! (channel) → QAM-demap → bit-deinterleave → decode. The inverse (receive)
//! half is demap + deinterleave + decode.

pub mod nr_5g;

use std::sync::Arc;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::DecoderConfig;
use gf2_coding::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GrayQamMapper,
    ModemSpec,
};
use gf2_coding::{CodeRate, Llr};
use gf2_core::BitVec;

use crate::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
use crate::error::StageError;
use crate::stage::{erase, AnyStage, ExecutionClass, Stage};

/// Default per-symbol total noise variance (`N0 = 2 sigma^2`) used by
/// [`GrayQamDemap`] when none is supplied.
///
/// Small but strictly positive so the noiseless forward+inverse roundtrip
/// produces high-confidence, correct-sign LLRs without the `exp`/`ln` log-MAP
/// reduction underflowing. Callers that simulate a channel pass the true `N0`
/// via [`GrayQamDemap::with_noise_var`].
pub const DEFAULT_DEMAP_NOISE_VAR: f32 = 0.1;

/// Maps a [`DvbT2Modulation`] to its bits-per-QAM-symbol (`m`).
///
/// 16-QAM → 4, 64-QAM → 6, QPSK → 2. This is the modem `bits_per_symbol`, used
/// to size the constellation order (`1 << m`) the [`GrayQamMapper`] /
/// [`FastGrayQamDemapper`] presets are built from.
fn bits_per_symbol(modulation: DvbT2Modulation) -> usize {
    modulation.bits_per_cell()
}

// ===========================================================================
// Shared Gray-QAM map / demap kernels
// ===========================================================================

/// Shared Gray-QAM **map** kernel: the single per-frame symbol loop turning a
/// [`BitPackedBatch`] into a [`SymbolBatch`] via [`GrayQamMapper::map_bits`].
///
/// The DVB-T2 [`GrayQamMap`] stage and the 5G NR
/// [`NrGrayQamMap`](nr_5g::NrGrayQamMap) stage are thin standard-specific
/// constructors over this core, so the frame loop exists exactly once.
pub(crate) struct GrayQamMapCore {
    mapper: GrayQamMapper<f32>,
    bits_per_symbol: usize,
}

impl GrayQamMapCore {
    /// Builds the map core for a Gray square-QAM constellation of
    /// `2^bits_per_symbol` points.
    pub(crate) fn new(bits_per_symbol: usize) -> Self {
        Self {
            mapper: GrayQamMapper::from_preset_order(1usize << bits_per_symbol),
            bits_per_symbol,
        }
    }

    /// Maps every frame's bits to I/Q symbol lanes (the shared frame loop).
    ///
    /// Each input frame's bit count must be a multiple of `bits_per_symbol`;
    /// each output frame has `bits / bits_per_symbol` symbols.
    pub(crate) fn map_batch(&self, input: &BitPackedBatch) -> SymbolBatch {
        let mut i_lanes = Vec::with_capacity(input.frames.len());
        let mut q_lanes = Vec::with_capacity(input.frames.len());
        for frame in &input.frames {
            let num_symbols = frame.len() / self.bits_per_symbol;
            let bits: Vec<bool> = (0..frame.len()).map(|b| frame.get(b)).collect();
            let mut out_i = vec![0.0_f32; num_symbols];
            let mut out_q = vec![0.0_f32; num_symbols];
            self.mapper.map_bits(&bits, &mut out_i, &mut out_q);
            i_lanes.push(out_i);
            q_lanes.push(out_q);
        }
        SymbolBatch::new(i_lanes, q_lanes)
    }
}

/// Shared Gray-QAM **soft-demap** kernel: the single per-frame loop turning a
/// [`SymbolBatch`] into an [`LlrBatch`] via [`FastGrayQamDemapper::demap_llrs`]
/// under AWGN with a constant per-symbol noise variance (`N0 = 2 sigma^2`).
///
/// The DVB-T2 [`GrayQamDemap`] stage, the 5G NR
/// [`NrGrayQamDemap`](nr_5g::NrGrayQamDemap) stage, and the hip-gated
/// `gpu::demap::CpuGrayQamDemapper` GPU fallback are thin standard-specific
/// constructors over this core, so the frame loop exists exactly once.
pub(crate) struct GrayQamDemapCore {
    demapper: FastGrayQamDemapper<f32>,
    method: DemapMethod,
    bits_per_symbol: usize,
    noise_var: f32,
}

impl GrayQamDemapCore {
    /// Builds the demap core for a Gray square-QAM constellation of
    /// `2^bits_per_symbol` points.
    ///
    /// `stage_name` labels the panic diagnostic so each wrapping stage keeps
    /// its historical message prefix.
    ///
    /// # Panics
    ///
    /// Panics if `noise_var` is not strictly positive and finite.
    pub(crate) fn new(
        bits_per_symbol: usize,
        method: DemapMethod,
        noise_var: f32,
        stage_name: &'static str,
    ) -> Self {
        assert!(
            noise_var.is_finite() && noise_var > 0.0,
            "{stage_name}: noise_var must be finite and > 0, got {noise_var}"
        );
        let spec = ModemSpec::<f32>::gray_square_qam(1usize << bits_per_symbol);
        Self {
            demapper: FastGrayQamDemapper::new(spec),
            method,
            bits_per_symbol,
            noise_var,
        }
    }

    /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`).
    #[inline]
    pub(crate) fn noise_var(&self) -> f32 {
        self.noise_var
    }

    /// The demap method (exact log-MAP or max-log).
    ///
    /// Consumed only by the hip-gated `gpu::demap::CpuGrayQamDemapper`
    /// wrapper, hence the feature gate (dead otherwise).
    #[cfg(feature = "hip")]
    #[inline]
    pub(crate) fn method(&self) -> DemapMethod {
        self.method
    }

    /// The bits-per-symbol (`m`) this core demaps.
    ///
    /// Consumed only by the hip-gated `gpu::demap::CpuGrayQamDemapper`
    /// wrapper, hence the feature gate (dead otherwise).
    #[cfg(feature = "hip")]
    #[inline]
    pub(crate) fn bits_per_symbol(&self) -> usize {
        self.bits_per_symbol
    }

    /// The underlying [`FastGrayQamDemapper`] this core delegates to.
    ///
    /// Consumed only by the hip-gated `gpu::demap::CpuGrayQamDemapper`
    /// wrapper (the GPU stage shares its Gray-PAM level table), hence the
    /// feature gate (dead otherwise).
    #[cfg(feature = "hip")]
    #[inline]
    pub(crate) fn demapper(&self) -> &FastGrayQamDemapper<f32> {
        &self.demapper
    }

    /// Demaps one frame's I/Q symbols into `rx_i.len() * m` LLRs.
    pub(crate) fn demap_frame(&self, rx_i: &[f32], rx_q: &[f32]) -> Vec<Llr> {
        let num_symbols = rx_i.len();
        let noise_var = vec![self.noise_var; num_symbols];
        let mut out = vec![Llr::zero(); num_symbols * self.bits_per_symbol];
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
    }

    /// Demaps every frame in the batch (the shared frame loop).
    pub(crate) fn demap_batch(&self, input: &SymbolBatch) -> LlrBatch {
        let frames = input
            .i
            .iter()
            .zip(input.q.iter())
            .map(|(rx_i, rx_q)| self.demap_frame(rx_i, rx_q))
            .collect();
        LlrBatch::new(frames)
    }
}

// ===========================================================================
// DvbT2Encode
// ===========================================================================

/// FEC-encode stage: BBFRAME info bits → FECFRAME coded bits.
///
/// Wraps [`DvbT2Concat::encode`] (BCH outer + LDPC inner). Each frame in the
/// input [`BitPackedBatch`] must be exactly `k_bch` bits; each output frame is
/// `n_ldpc` bits.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use gf2_sim::stages::DvbT2Encode;
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
/// use gf2_coding::ldpc::dvb_t2::FrameSize;
/// use gf2_coding::CodeRate;
/// use gf2_core::BitVec;
///
/// let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());
/// let stage = DvbT2Encode::new(codec.clone());
/// let bbframe = BitVec::zeros(codec.k_bch());
/// let out = stage.process(&BitPackedBatch::new(vec![bbframe]), &mut ()).unwrap();
/// assert_eq!(out.frames[0].len(), codec.n_ldpc());
/// ```
pub struct DvbT2Encode {
    codec: Arc<DvbT2Concat>,
}

impl DvbT2Encode {
    /// Builds an encode stage over a shared [`DvbT2Concat`] codec.
    ///
    /// # Arguments
    ///
    /// * `codec` — the concatenated BCH+LDPC codec to encode with.
    pub fn new(codec: Arc<DvbT2Concat>) -> Self {
        Self { codec }
    }

    /// The BBFRAME information-bit count `k_bch` this stage encodes.
    ///
    /// Exposed so the stage-driven executor (`de160fc5`) can mint the
    /// per-frame random BBFRAME input of the correct width after downcasting
    /// the chain's source stage via [`AnyStage::stage_as_any`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use gf2_sim::stages::DvbT2Encode;
    /// use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    ///
    /// let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());
    /// let stage = DvbT2Encode::new(codec.clone());
    /// assert_eq!(stage.k_bch(), codec.k_bch());
    /// ```
    #[inline]
    #[must_use]
    pub fn k_bch(&self) -> usize {
        self.codec.k_bch()
    }
}

impl Stage<BitPackedBatch, BitPackedBatch> for DvbT2Encode {
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
            .map(|bbframe| self.codec.encode(bbframe))
            .collect();
        Ok(BitPackedBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

// ===========================================================================
// BitInterleave / BitDeinterleave
// ===========================================================================

/// Bit-interleave stage: FECFRAME coded bits → interleaved coded bits.
///
/// Wraps [`DvbT2BitInterleaver::interleave`]. Each input/output frame is
/// `frame_bits()` (= `n_ldpc`) bits.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use gf2_sim::stages::BitInterleave;
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation};
/// use gf2_coding::ldpc::dvb_t2::FrameSize;
/// use gf2_coding::CodeRate;
/// use gf2_core::BitVec;
///
/// let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
/// let il = Arc::new(DvbT2BitInterleaver::new(modcod));
/// let stage = BitInterleave::new(il.clone());
/// let frame = BitVec::zeros(il.frame_bits());
/// let out = stage.process(&BitPackedBatch::new(vec![frame]), &mut ()).unwrap();
/// assert_eq!(out.frames[0].len(), il.frame_bits());
/// ```
pub struct BitInterleave {
    interleaver: Arc<DvbT2BitInterleaver>,
}

impl BitInterleave {
    /// Builds a bit-interleave stage over a shared interleaver.
    ///
    /// # Arguments
    ///
    /// * `interleaver` — the DVB-T2 bit interleaver for this MODCOD.
    pub fn new(interleaver: Arc<DvbT2BitInterleaver>) -> Self {
        Self { interleaver }
    }
}

impl Stage<BitPackedBatch, BitPackedBatch> for BitInterleave {
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
            .map(|frame| self.interleaver.interleave(frame))
            .collect();
        Ok(BitPackedBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// Bit-deinterleave stage: interleaved LLRs → FECFRAME-order LLRs.
///
/// Wraps [`DvbT2BitInterleaver::deinterleave_llrs`], the receive-path inverse
/// of [`BitInterleave`]. Each input/output frame is `frame_bits()` LLRs.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use gf2_sim::stages::BitDeinterleave;
/// use gf2_sim::batch::LlrBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation};
/// use gf2_coding::ldpc::dvb_t2::FrameSize;
/// use gf2_coding::{CodeRate, Llr};
///
/// let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
/// let il = Arc::new(DvbT2BitInterleaver::new(modcod));
/// let stage = BitDeinterleave::new(il.clone());
/// let frame = vec![Llr::new(1.0); il.frame_bits()];
/// let out = stage.process(&LlrBatch::new(vec![frame]), &mut ()).unwrap();
/// assert_eq!(out.frames[0].len(), il.frame_bits());
/// ```
pub struct BitDeinterleave {
    interleaver: Arc<DvbT2BitInterleaver>,
}

impl BitDeinterleave {
    /// Builds a bit-deinterleave stage over a shared interleaver.
    ///
    /// # Arguments
    ///
    /// * `interleaver` — the DVB-T2 bit interleaver for this MODCOD.
    pub fn new(interleaver: Arc<DvbT2BitInterleaver>) -> Self {
        Self { interleaver }
    }
}

impl Stage<LlrBatch, LlrBatch> for BitDeinterleave {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &LlrBatch, _scratch: &mut ()) -> Result<LlrBatch, StageError> {
        let frames = input
            .frames
            .iter()
            .map(|llrs| self.interleaver.deinterleave_llrs(llrs))
            .collect();
        Ok(LlrBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

// ===========================================================================
// GrayQamMap
// ===========================================================================

/// Gray-QAM map stage: interleaved coded bits → IQ symbols.
///
/// Wraps [`GrayQamMapper::map_bits`]. Each input frame's bit count must be a
/// multiple of `m = log2(order)`; each output frame has `bits / m` symbols
/// stored as parallel I/Q `f32` lanes.
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::GrayQamMap;
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
/// use gf2_core::BitVec;
///
/// let stage = GrayQamMap::new(DvbT2Modulation::Qam16);
/// let frame = BitVec::zeros(8); // 2 symbols of 4 bits
/// let out = stage.process(&BitPackedBatch::new(vec![frame]), &mut ()).unwrap();
/// assert_eq!(out.i[0].len(), 2);
/// ```
pub struct GrayQamMap {
    core: GrayQamMapCore,
}

impl GrayQamMap {
    /// Builds a Gray-QAM map stage for a DVB-T2 modulation.
    ///
    /// # Arguments
    ///
    /// * `modulation` — DVB-T2 modulation order (QPSK / 16-QAM / 64-QAM).
    pub fn new(modulation: DvbT2Modulation) -> Self {
        Self {
            core: GrayQamMapCore::new(bits_per_symbol(modulation)),
        }
    }
}

impl Stage<BitPackedBatch, SymbolBatch> for GrayQamMap {
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

// ===========================================================================
// GrayQamDemap
// ===========================================================================

/// Gray-QAM soft-demap stage: IQ symbols → soft LLRs.
///
/// Wraps [`FastGrayQamDemapper::demap_llrs`] under AWGN-shaped log-MAP. Each
/// input frame of `s` symbols produces `s * m` LLRs (`m = log2(order)`). The
/// per-symbol total noise variance (`N0`) defaults to
/// [`DEFAULT_DEMAP_NOISE_VAR`]; set the true channel `N0` via
/// [`GrayQamDemap::with_noise_var`].
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::{GrayQamMap, GrayQamDemap};
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
/// use gf2_coding::modem::DemapMethod;
/// use gf2_core::BitVec;
///
/// let map = GrayQamMap::new(DvbT2Modulation::Qam16);
/// let demap = GrayQamDemap::new(DvbT2Modulation::Qam16, DemapMethod::ExactLogMap);
/// let syms = map.process(&BitPackedBatch::new(vec![BitVec::zeros(8)]), &mut ()).unwrap();
/// let llrs = demap.process(&syms, &mut ()).unwrap();
/// assert_eq!(llrs.frames[0].len(), 8);
/// ```
pub struct GrayQamDemap {
    core: GrayQamDemapCore,
}

impl GrayQamDemap {
    /// Builds a Gray-QAM demap stage with the default demap noise variance.
    ///
    /// # Arguments
    ///
    /// * `modulation` — DVB-T2 modulation order.
    /// * `method` — exact log-MAP or max-log demapping.
    pub fn new(modulation: DvbT2Modulation, method: DemapMethod) -> Self {
        Self::with_noise_var(modulation, method, DEFAULT_DEMAP_NOISE_VAR)
    }

    /// Builds a Gray-QAM demap stage with an explicit per-symbol noise variance.
    ///
    /// # Arguments
    ///
    /// * `modulation` — DVB-T2 modulation order.
    /// * `method` — exact log-MAP or max-log demapping.
    /// * `noise_var` — per-symbol total complex AWGN noise variance
    ///   (`N0 = 2 sigma^2`); must be strictly positive.
    ///
    /// # Panics
    ///
    /// Panics if `noise_var` is not strictly positive and finite.
    pub fn with_noise_var(
        modulation: DvbT2Modulation,
        method: DemapMethod,
        noise_var: f32,
    ) -> Self {
        Self {
            core: GrayQamDemapCore::new(
                bits_per_symbol(modulation),
                method,
                noise_var,
                "GrayQamDemap",
            ),
        }
    }

    /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`) this
    /// demapper assumes when computing LLRs.
    ///
    /// Exposed so consumers (e.g. the DVB-T2 preset's regression test) can
    /// verify the demapper's `N0` was wired to the channel's true `N0`.
    #[inline]
    #[must_use]
    pub fn noise_var(&self) -> f32 {
        self.core.noise_var()
    }
}

impl Stage<SymbolBatch, LlrBatch> for GrayQamDemap {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &SymbolBatch, _scratch: &mut ()) -> Result<LlrBatch, StageError> {
        Ok(self.core.demap_batch(input))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

// ===========================================================================
// DvbT2Decode
// ===========================================================================

/// Per-stage scratch for [`DvbT2Decode`]: the per-frame LDPC BP iteration
/// counts of the most recent `process` call.
///
/// [`DvbT2Decode::process`] clears [`iterations`](Self::iterations) and pushes
/// one entry per input frame (in frame order): the genuine BP depth reported by
/// [`DvbT2Concat::decode_soft_counted`] on both the converged and
/// non-converged arms. The stage-driven executor (`de160fc5`) reads the counts
/// back after each frame so the aggregated `mean_iters` column is byte-identical
/// to the SSOT frame kernel's (design doc §11) — the erased
/// [`process_any`](crate::stage::AnyStage::process_any) signature cannot carry
/// them, and scratch is the sanctioned per-stage side channel.
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::DecodeScratch;
///
/// let scratch = DecodeScratch::default();
/// assert!(scratch.iterations.is_empty());
/// ```
#[derive(Debug, Clone, Default)]
pub struct DecodeScratch {
    /// Per-frame LDPC BP iteration counts of the most recent
    /// [`DvbT2Decode::process`] call, in input-frame order.
    pub iterations: Vec<u64>,
}

/// FEC-decode stage: FECFRAME-order soft LLRs → recovered BBFRAME bits.
///
/// Wraps [`DvbT2Concat::decode_soft_counted`] (LDPC belief-propagation + BCH
/// hard-decision). Each input frame is `n_ldpc` LLRs; each output frame is
/// `k_bch` recovered BBFRAME bits. The per-frame BP iteration counts are
/// recorded into the [`DecodeScratch`] (see its docs).
///
/// A frame whose LDPC belief propagation does not converge is still passed
/// through using its best-effort (BCH-corrected) BBFRAME estimate, matching the
/// `Err(LdpcDecodeFailed { bbframe, .. })` payload of
/// [`DvbT2Concat::decode_soft_counted`]; the simulation's frame-error
/// accounting compares the recovered bits against the transmitted BBFRAME
/// rather than relying on a hard decode error here.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use gf2_sim::stages::{DecodeScratch, DvbT2Decode};
/// use gf2_sim::batch::{HardDecisionBatch, LlrBatch};
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
/// use gf2_coding::ldpc::dvb_t2::FrameSize;
/// use gf2_coding::{CodeRate, Llr};
///
/// let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());
/// let stage = DvbT2Decode::new(codec.clone());
/// let llrs = vec![Llr::new(10.0); codec.n_ldpc()];
/// let mut scratch = DecodeScratch::default();
/// let out: HardDecisionBatch = stage.process(&LlrBatch::new(vec![llrs]), &mut scratch).unwrap();
/// assert_eq!(out.frames[0].len(), codec.k_bch());
/// assert_eq!(scratch.iterations.len(), 1, "one BP count per frame");
/// ```
pub struct DvbT2Decode {
    codec: Arc<DvbT2Concat>,
}

impl DvbT2Decode {
    /// Builds a decode stage over a shared [`DvbT2Concat`] codec.
    ///
    /// # Arguments
    ///
    /// * `codec` — the concatenated BCH+LDPC codec to decode with.
    pub fn new(codec: Arc<DvbT2Concat>) -> Self {
        Self { codec }
    }
}

impl Stage<LlrBatch, HardDecisionBatch> for DvbT2Decode {
    type Scratch = DecodeScratch;
    type CpuFallback = Self;

    fn process(
        &self,
        input: &LlrBatch,
        scratch: &mut DecodeScratch,
    ) -> Result<HardDecisionBatch, StageError> {
        scratch.iterations.clear();
        let mut frames: Vec<BitVec> = Vec::with_capacity(input.frames.len());
        for llrs in &input.frames {
            // `decode_soft_counted` is the SSOT decode call the frame kernel
            // (`frame_sim`) makes, so the recorded iteration counts (and the
            // best-effort BBFRAME on the non-converged arm) are identical to
            // the SSOT path's.
            let (bbframe, iterations) = match self.codec.decode_soft_counted(llrs) {
                Ok((bbframe, iterations)) => (bbframe, iterations as u64),
                // Non-convergence is not a stage error: keep the best-effort
                // BBFRAME estimate so frame-error accounting can compare it
                // against the transmitted bits (see stage doc).
                Err(gf2_coding::ldpc::dvb_t2::concat::ConcatError::LdpcDecodeFailed {
                    bbframe,
                    iterations,
                }) => (bbframe, iterations as u64),
                // `decode_soft_counted` only ever returns `LdpcDecodeFailed`;
                // any other variant (currently unreachable) is surfaced as a
                // transient error so the executor can decide policy.
                Err(other) => {
                    return Err(StageError::Recoverable(
                        crate::error::RecoverableError::Transient(Box::new(other)),
                    ))
                }
            };
            scratch.iterations.push(iterations);
            frames.push(bbframe);
        }
        Ok(HardDecisionBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

// ===========================================================================
// DvbT2BchTail
// ===========================================================================

/// BCH outer-decode tail stage: LDPC hard-decision FECFRAME codewords →
/// recovered BBFRAME bits.
///
/// Wraps [`DvbT2Concat::decode_bch_from_ldpc_codeword`] — the factored-out
/// outer-decode tail of [`DvbT2Concat::decode_soft`] — so a pipeline whose
/// inner LDPC decode runs elsewhere (the `ExecutionClass::GpuOnly`
/// `gpu::ldpc_bp::GpuLdpcBp` stage under `feature = "hip"`, or its registered
/// `CpuLdpcBp` fallback) can finish the concatenated decode on the CPU. Each
/// input frame is the full `n_ldpc`-bit hard-decision codeword; each output
/// frame is the `k_bch`-bit BBFRAME. The DVB-T2 preset places this stage after
/// the GPU LDPC decode when GPU offload is enabled; the all-CPU chain keeps
/// the combined [`DvbT2Decode`] instead.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use gf2_sim::stages::DvbT2BchTail;
/// use gf2_sim::batch::HardDecisionBatch;
/// use gf2_sim::Stage;
/// use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
/// use gf2_coding::ldpc::dvb_t2::FrameSize;
/// use gf2_coding::CodeRate;
/// use gf2_core::BitVec;
///
/// let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());
/// let stage = DvbT2BchTail::new(codec.clone());
/// // The all-zeros FECFRAME is a valid codeword: it BCH-decodes to zeros.
/// let codeword = BitVec::zeros(codec.n_ldpc());
/// let out = stage.process(&HardDecisionBatch::new(vec![codeword]), &mut ()).unwrap();
/// assert_eq!(out.frames[0].len(), codec.k_bch());
/// ```
pub struct DvbT2BchTail {
    codec: Arc<DvbT2Concat>,
}

impl DvbT2BchTail {
    /// Builds a BCH outer-decode tail stage over a shared [`DvbT2Concat`] codec.
    ///
    /// # Arguments
    ///
    /// * `codec` — the concatenated BCH+LDPC codec whose BCH outer decode to run.
    pub fn new(codec: Arc<DvbT2Concat>) -> Self {
        Self { codec }
    }
}

impl Stage<HardDecisionBatch, HardDecisionBatch> for DvbT2BchTail {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(
        &self,
        input: &HardDecisionBatch,
        _scratch: &mut (),
    ) -> Result<HardDecisionBatch, StageError> {
        let frames: Vec<BitVec> = input
            .frames
            .iter()
            .map(|codeword| self.codec.decode_bch_from_ldpc_codeword(codeword))
            .collect();
        Ok(HardDecisionBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

// ===========================================================================
// Wiring factory
// ===========================================================================

/// The ordered DVB-T2 BICM stage wiring shared by every consumer wave.
///
/// Returned by [`dvb_t2_bicm_stages`]. The two `Vec<Box<dyn AnyStage>>` halves
/// are the forward (transmit) and inverse (receive) stage chains in execution
/// order, plus the shared codec / interleaver handles so a consumer can read
/// frame dimensions (`k_bch`, `n_ldpc`, `frame_bits`) without rebuilding them.
///
/// * `forward` — `[DvbT2Encode, BitInterleave, GrayQamMap]`
///   (`BitPackedBatch` → `BitPackedBatch` → `BitPackedBatch` → `SymbolBatch`).
/// * `inverse` — `[GrayQamDemap, BitDeinterleave, DvbT2Decode]`
///   (`SymbolBatch` → `LlrBatch` → `LlrBatch` → `HardDecisionBatch`).
///
/// A channel stage (owned by `db9836e4`) slots between `forward` and `inverse`
/// (`SymbolBatch` → `SymbolBatch`); this factory deliberately emits no channel
/// so noiseless composition is possible.
pub struct DvbT2BicmStages {
    /// Forward (transmit) chain in execution order.
    pub forward: Vec<Box<dyn AnyStage>>,
    /// Inverse (receive) chain in execution order.
    pub inverse: Vec<Box<dyn AnyStage>>,
    /// Shared concatenated BCH+LDPC codec.
    pub codec: Arc<DvbT2Concat>,
    /// Shared DVB-T2 bit interleaver.
    pub interleaver: Arc<DvbT2BitInterleaver>,
}

/// Builds the ordered DVB-T2 BICM forward + inverse stage chains for a MODCOD.
///
/// This is the single wiring source the graph-API (`c09d3e95`), preset
/// (`81d05bab`), and parallel-dispatch (`3fcb7025`) waves reuse, so the stage
/// order is defined in exactly one place. The codec is constructed for
/// [`FrameSize::Normal`] (n=64800) — the in-scope DVB-T2 FECFRAME — and the
/// supplied `decoder` configuration is applied to it.
///
/// # Arguments
///
/// * `rate` — DVB-T2 LDPC code rate (1/2, 2/3, or 3/4 are the in-scope rates).
/// * `modulation` — DVB-T2 modulation order (16-QAM or 64-QAM in scope).
/// * `decoder` — LDPC belief-propagation decoder configuration applied to the
///   shared codec.
/// * `demap` — soft-demap method ([`DemapMethod::ExactLogMap`] or
///   [`DemapMethod::MaxLog`]).
/// * `demap_noise_var` — the per-symbol total complex AWGN noise variance
///   (`N0 = 2 sigma^2`) the soft demapper assumes. For a physically consistent
///   chain this **must** equal the channel's true `N0`; the preset derives it
///   from the channel's Es/N0 via the crate-private `es_n0_db_to_sigma` helper.
///   Noiseless
///   callers (those that connect [`GrayQamMap`] straight to [`GrayQamDemap`]
///   with no channel) pass [`DEFAULT_DEMAP_NOISE_VAR`].
///
/// # Returns
///
/// A [`DvbT2BicmStages`] holding the forward and inverse erased-stage chains
/// plus the shared codec / interleaver handles.
///
/// # Panics
///
/// Panics if the `(FrameSize::Normal, rate)` pair cannot construct a codec
/// (every in-scope DVB-T2 rate constructs successfully), if `rate` /
/// `modulation` is out of the bit-interleaver's supported scope, or if
/// `demap_noise_var` is not finite and strictly positive (per
/// [`GrayQamDemap::with_noise_var`]).
///
/// # Examples
///
/// ```
/// use gf2_sim::stages::{dvb_t2_bicm_stages, DEFAULT_DEMAP_NOISE_VAR};
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
/// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
/// use gf2_coding::modem::DemapMethod;
/// use gf2_coding::CodeRate;
///
/// let stages = dvb_t2_bicm_stages(
///     CodeRate::Rate1_2,
///     DvbT2Modulation::Qam16,
///     DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
///     DemapMethod::ExactLogMap,
///     DEFAULT_DEMAP_NOISE_VAR,
/// );
/// assert_eq!(stages.forward.len(), 3);
/// assert_eq!(stages.inverse.len(), 3);
/// assert_eq!(stages.codec.n_ldpc(), 64800);
/// ```
pub fn dvb_t2_bicm_stages(
    rate: CodeRate,
    modulation: DvbT2Modulation,
    decoder: DecoderConfig,
    demap: DemapMethod,
    demap_noise_var: f32,
) -> DvbT2BicmStages {
    let mut concat = DvbT2Concat::new(FrameSize::Normal, rate)
        .expect("DVB-T2 Normal-frame codec construction must succeed for in-scope rates");
    concat.set_decoder_config(decoder);
    let codec = Arc::new(concat);

    let modcod = DvbT2Modcod::new(FrameSize::Normal, rate, modulation);
    let interleaver = Arc::new(DvbT2BitInterleaver::new(modcod));

    let forward: Vec<Box<dyn AnyStage>> = vec![
        erase(DvbT2Encode::new(codec.clone())),
        erase(BitInterleave::new(interleaver.clone())),
        erase(GrayQamMap::new(modulation)),
    ];
    let inverse: Vec<Box<dyn AnyStage>> = vec![
        erase(GrayQamDemap::with_noise_var(
            modulation,
            demap,
            demap_noise_var,
        )),
        erase(BitDeinterleave::new(interleaver.clone())),
        erase(DvbT2Decode::new(codec.clone())),
    ];

    DvbT2BicmStages {
        forward,
        inverse,
        codec,
        interleaver,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};

    #[test]
    fn test_bits_per_symbol() {
        assert_eq!(bits_per_symbol(DvbT2Modulation::Qpsk), 2);
        assert_eq!(bits_per_symbol(DvbT2Modulation::Qam16), 4);
        assert_eq!(bits_per_symbol(DvbT2Modulation::Qam64), 6);
    }

    #[test]
    fn test_factory_emits_three_plus_three_stages() {
        let s = dvb_t2_bicm_stages(
            CodeRate::Rate1_2,
            DvbT2Modulation::Qam16,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            DemapMethod::ExactLogMap,
            DEFAULT_DEMAP_NOISE_VAR,
        );
        assert_eq!(s.forward.len(), 3);
        assert_eq!(s.inverse.len(), 3);
        assert_eq!(s.codec.n_ldpc(), 64800);
        assert_eq!(s.interleaver.frame_bits(), 64800);
    }

    #[test]
    fn test_factory_forward_chain_type_threading() {
        // Forward chain types must thread: BitPacked -> BitPacked -> BitPacked -> Symbol.
        let s = dvb_t2_bicm_stages(
            CodeRate::Rate1_2,
            DvbT2Modulation::Qam16,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            DemapMethod::ExactLogMap,
            DEFAULT_DEMAP_NOISE_VAR,
        );
        use crate::batch::HardDecisionBatch;
        use std::any::TypeId;
        let bitpacked = TypeId::of::<BitPackedBatch>();
        let symbol = TypeId::of::<SymbolBatch>();
        let llr = TypeId::of::<LlrBatch>();
        let hard = TypeId::of::<HardDecisionBatch>();

        assert_eq!(s.forward[0].input_type(), bitpacked);
        assert_eq!(s.forward[0].output_type(), bitpacked); // encode
        assert_eq!(s.forward[1].output_type(), bitpacked); // interleave
        assert_eq!(s.forward[2].input_type(), bitpacked);
        assert_eq!(s.forward[2].output_type(), symbol); // map

        // Inverse chain: Symbol -> Llr -> Llr -> HardDecision.
        assert_eq!(s.inverse[0].input_type(), symbol);
        assert_eq!(s.inverse[0].output_type(), llr); // demap
        assert_eq!(s.inverse[1].output_type(), llr); // deinterleave
        assert_eq!(s.inverse[2].input_type(), llr);
        assert_eq!(s.inverse[2].output_type(), hard); // decode
    }

    #[test]
    #[should_panic(expected = "noise_var must be finite and > 0")]
    fn test_demap_rejects_nonpositive_noise_var() {
        let _ = GrayQamDemap::with_noise_var(DvbT2Modulation::Qam16, DemapMethod::ExactLogMap, 0.0);
    }
}
