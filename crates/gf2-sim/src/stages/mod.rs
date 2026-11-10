//! DVB-T2 codec and modem [`Stage`] wrappers over the `gf2-coding` types
//! [`DvbT2Concat`], [`DvbT2BitInterleaver`] and [`DvbT2CellMapper`];
//! [`dvb_t2_bicm_stages`] wires them in BICM order: encode → bit-interleave →
//! cell demultiplex and map → (channel) → cell demap and multiplex →
//! bit-deinterleave → decode. Every stage is `ExecutionClass::CpuOnly` with
//! `CpuFallback = Self` and `Scratch = ()`, except [`DvbT2Decode`], which
//! decodes through [`BatchDecode`] and whose [`DecodeScratch`] exposes the
//! per-frame LDPC BP iteration counts.

pub mod batch_decode;
pub mod nr_5g;

use std::sync::Arc;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::{DvbT2Concat, DvbT2ConcatBatchDecoder};
use gf2_coding::ldpc::dvb_t2::{DvbT2CellMapper, FrameSize};
use gf2_coding::ldpc::DecoderConfig;
use gf2_coding::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GrayQamMapper,
    ModemSpec,
};
use gf2_coding::tuning::{self, DecodeSelectors};
use gf2_coding::{CodeRate, Llr};
use gf2_core::BitVec;

use crate::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
use crate::error::StageError;
use crate::stage::{erase, AnyStage, ExecutionClass, Stage};
use batch_decode::{BatchDecode, BatchDecodeScratch};

/// Default per-symbol total noise variance (`N0 = 2 sigma^2`) used by
/// [`DvbT2CellDemap`] when none is supplied.
///
/// Callers that simulate a channel pass the true `N0` via
/// [`DvbT2CellDemap::with_noise_var`].
pub const DEFAULT_DEMAP_NOISE_VAR: f32 = 0.1;

/// Gray-QAM map kernel of [`NrGrayQamMap`](nr_5g::NrGrayQamMap): [`BitPackedBatch`] →
/// [`SymbolBatch`] via [`GrayQamMapper::map_bits`].
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

/// Gray-QAM soft-demap kernel shared by
/// [`NrGrayQamDemap`](nr_5g::NrGrayQamDemap) and the hip-gated
/// `gpu::demap::CpuGrayQamDemapper`: [`SymbolBatch`] → [`LlrBatch`] via
/// [`FastGrayQamDemapper::demap_llrs`] under AWGN with a constant per-symbol
/// noise variance (`N0 = 2 sigma^2`).
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
    /// `stage_name` prefixes the panic message.
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
    #[cfg(feature = "hip")]
    #[inline]
    pub(crate) fn method(&self) -> DemapMethod {
        self.method
    }

    /// The bits-per-symbol (`m`) this core demaps.
    #[cfg(feature = "hip")]
    #[inline]
    pub(crate) fn bits_per_symbol(&self) -> usize {
        self.bits_per_symbol
    }

    /// The underlying [`FastGrayQamDemapper`].
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

/// FEC-encode stage: BBFRAME info bits → FECFRAME coded bits.
///
/// Wraps [`DvbT2Concat::encode`] (BCH outer + LDPC inner). Each frame in the
/// input [`BitPackedBatch`] must be exactly `k_bch` bits; each output frame is
/// `n_ldpc` bits.
pub struct DvbT2Encode {
    codec: Arc<DvbT2Concat>,
}

impl DvbT2Encode {
    /// Builds an encode stage over a shared [`DvbT2Concat`] codec.
    pub fn new(codec: Arc<DvbT2Concat>) -> Self {
        Self { codec }
    }

    /// The BBFRAME information-bit count `k_bch` this stage encodes.
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

/// Bit-interleave stage: FECFRAME coded bits → interleaved coded bits.
///
/// Wraps [`DvbT2BitInterleaver::interleave`]. Each input/output frame is
/// `frame_bits()` (= `n_ldpc`) bits.
pub struct BitInterleave {
    interleaver: Arc<DvbT2BitInterleaver>,
}

impl BitInterleave {
    /// Builds a bit-interleave stage over a shared interleaver.
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
pub struct BitDeinterleave {
    interleaver: Arc<DvbT2BitInterleaver>,
}

impl BitDeinterleave {
    /// Builds a bit-deinterleave stage over a shared interleaver.
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

/// Cell map stage: bit-interleaved FECFRAME → cells.
///
/// Wraps [`DvbT2CellMapper::map`]: the bit-to-cell-word demultiplexer and
/// cell-word mapping of `@/citation/Etsi2015` clauses 6.2.1 and 6.2.2. Each
/// input frame is `N_ldpc` bits; each output frame holds
/// [`DvbT2CellMapper::num_cells`] cells as parallel I/Q `f32` lanes.
pub struct DvbT2CellMap {
    mapper: Arc<DvbT2CellMapper>,
}

impl DvbT2CellMap {
    /// Builds a cell map stage over a shared [`DvbT2CellMapper`].
    pub fn new(mapper: Arc<DvbT2CellMapper>) -> Self {
        Self { mapper }
    }
}

impl Stage<BitPackedBatch, SymbolBatch> for DvbT2CellMap {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(
        &self,
        input: &BitPackedBatch,
        _scratch: &mut (),
    ) -> Result<SymbolBatch, StageError> {
        let cells = self.mapper.num_cells();
        let (mut i_lanes, mut q_lanes) = (Vec::new(), Vec::new());
        for frame in &input.frames {
            let (mut out_i, mut out_q) = (vec![0.0_f32; cells], vec![0.0_f32; cells]);
            self.mapper.map(frame, &mut out_i, &mut out_q);
            i_lanes.push(out_i);
            q_lanes.push(out_q);
        }
        Ok(SymbolBatch::new(i_lanes, q_lanes))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// Cell soft-demap stage: cells → LLRs in bit-interleaver output order.
///
/// Wraps [`DvbT2CellMapper::demap_llrs`] under AWGN with a constant
/// per-cell total noise variance (`N0`), the receive-path inverse of
/// [`DvbT2CellMap`]. Each input frame of [`DvbT2CellMapper::num_cells`]
/// cells produces `N_ldpc` LLRs. `N0` defaults to
/// [`DEFAULT_DEMAP_NOISE_VAR`]; set the true channel `N0` via
/// [`DvbT2CellDemap::with_noise_var`].
pub struct DvbT2CellDemap {
    mapper: Arc<DvbT2CellMapper>,
    method: DemapMethod,
    noise_var: f32,
}

impl DvbT2CellDemap {
    /// Builds a cell demap stage with [`DEFAULT_DEMAP_NOISE_VAR`].
    pub fn new(mapper: Arc<DvbT2CellMapper>, method: DemapMethod) -> Self {
        Self::with_noise_var(mapper, method, DEFAULT_DEMAP_NOISE_VAR)
    }

    /// Builds a cell demap stage with the per-cell total complex AWGN noise
    /// variance `noise_var` (`N0 = 2 sigma^2`).
    ///
    /// # Panics
    ///
    /// Panics if `noise_var` is not strictly positive and finite.
    pub fn with_noise_var(
        mapper: Arc<DvbT2CellMapper>,
        method: DemapMethod,
        noise_var: f32,
    ) -> Self {
        assert!(
            noise_var.is_finite() && noise_var > 0.0,
            "DvbT2CellDemap: noise_var must be finite and > 0, got {noise_var}"
        );
        Self {
            mapper,
            method,
            noise_var,
        }
    }

    /// The per-cell total complex AWGN noise variance (`N0 = 2 sigma^2`) this
    /// demapper assumes when computing LLRs.
    #[inline]
    #[must_use]
    pub fn noise_var(&self) -> f32 {
        self.noise_var
    }
}

impl Stage<SymbolBatch, LlrBatch> for DvbT2CellDemap {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &SymbolBatch, _scratch: &mut ()) -> Result<LlrBatch, StageError> {
        let noise_var = vec![self.noise_var; self.mapper.num_cells()];
        let n = self.mapper.num_cells() * self.mapper.bits_per_cell();
        let frames = input
            .i
            .iter()
            .zip(&input.q)
            .map(|(rx_i, rx_q)| {
                let mut out = vec![Llr::zero(); n];
                self.mapper.demap_llrs(
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

/// Scratch of [`DvbT2Decode`]: the [`BatchDecodeScratch`] of the
/// [`BatchDecode`] stage it decodes through, whose
/// [`iterations`](BatchDecodeScratch::iterations) are the per-frame LDPC BP
/// iteration counts of the most recent `process` call.
pub type DecodeScratch = BatchDecodeScratch;

type ConcatDecoderFactory = Box<dyn Fn() -> DvbT2ConcatBatchDecoder + Send + Sync>;

/// FEC-decode stage: FECFRAME-order soft LLRs → recovered BBFRAME bits.
///
/// Decodes a batch through [`BatchDecode`] over a [`DvbT2ConcatBatchDecoder`]
/// of the codec (`@/citation/Etsi2015` §6: LDPC belief propagation, then BCH
/// hard decision). Each input frame is `n_ldpc` LLRs; each output frame is
/// `k_bch` recovered BBFRAME bits. A frame's BBFRAME and its BP iteration
/// count in the [`DecodeScratch`] equal those of
/// [`DvbT2Concat::decode_soft_counted`] on the same LLRs.
///
/// A frame whose LDPC belief propagation does not converge yields the
/// `bbframe` payload of the `Err(LdpcDecodeFailed { bbframe, .. })` returned by
/// [`DvbT2Concat::decode_soft_counted`], and no stage error.
///
/// # Panics
///
/// `process` panics as [`BatchDecode`] does, and on the first call with a
/// scratch as [`DvbT2ConcatBatchDecoder::with_selectors`] does.
pub struct DvbT2Decode {
    codec: Arc<DvbT2Concat>,
    selectors: DecodeSelectors,
    batch: BatchDecode<ConcatDecoderFactory>,
}

impl DvbT2Decode {
    /// Builds a decode stage over a shared [`DvbT2Concat`] codec. The decode
    /// selectors are those of the process-wide coding tuning section, read
    /// once here.
    ///
    /// # Panics
    ///
    /// As [`tuning::active`].
    pub fn new(codec: Arc<DvbT2Concat>) -> Self {
        let selectors = tuning::active().section.decode().clone();
        Self::with_selectors(codec, selectors)
    }

    /// Builds a decode stage whose decoders take the given decode selectors,
    /// as [`DvbT2ConcatBatchDecoder::with_selectors`] does.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    pub fn with_selectors(codec: Arc<DvbT2Concat>, selectors: DecodeSelectors) -> Self {
        Self::build(codec, selectors, false)
    }

    /// Holds the lane route of every decoder on the portable backend, as
    /// [`DvbT2ConcatBatchDecoder::with_portable_lanes`] does.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    #[must_use]
    pub fn with_portable_lanes(self) -> Self {
        Self::build(self.codec, self.selectors, true)
    }

    fn build(codec: Arc<DvbT2Concat>, selectors: DecodeSelectors, portable: bool) -> Self {
        let (shared, chosen) = (codec.clone(), selectors.clone());
        let factory: ConcatDecoderFactory = Box::new(move || {
            let decoder = DvbT2ConcatBatchDecoder::with_selectors(&shared, chosen.clone());
            if portable {
                decoder.with_portable_lanes()
            } else {
                decoder
            }
        });
        Self {
            codec,
            selectors,
            batch: BatchDecode::new(factory),
        }
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
        self.batch.process(input, scratch)
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// BCH outer-decode tail stage: LDPC hard-decision FECFRAME codewords →
/// recovered BBFRAME bits.
///
/// Wraps [`DvbT2Concat::decode_bch_from_ldpc_codeword`], so a pipeline whose
/// inner LDPC decode runs in a separate stage (`gpu::ldpc_bp::GpuLdpcBp` or
/// its `CpuLdpcBp` fallback) finishes the concatenated decode on the CPU. Each
/// input frame is the full `n_ldpc`-bit hard-decision codeword; each output
/// frame is the `k_bch`-bit BBFRAME.
pub struct DvbT2BchTail {
    codec: Arc<DvbT2Concat>,
}

impl DvbT2BchTail {
    /// Builds a BCH outer-decode tail stage over a shared [`DvbT2Concat`] codec.
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

/// The ordered DVB-T2 BICM stage wiring returned by [`dvb_t2_bicm_stages`].
///
/// * `forward` — `[DvbT2Encode, BitInterleave, DvbT2CellMap]`
///   (`BitPackedBatch` → `BitPackedBatch` → `BitPackedBatch` → `SymbolBatch`).
/// * `inverse` — `[DvbT2CellDemap, BitDeinterleave, DvbT2Decode]`
///   (`SymbolBatch` → `LlrBatch` → `LlrBatch` → `HardDecisionBatch`).
///
/// A channel stage (`SymbolBatch` → `SymbolBatch`) slots between `forward` and
/// `inverse`.
pub struct DvbT2BicmStages {
    /// Forward (transmit) chain in execution order.
    pub forward: Vec<Box<dyn AnyStage>>,
    /// Inverse (receive) chain in execution order.
    pub inverse: Vec<Box<dyn AnyStage>>,
    /// Shared concatenated BCH+LDPC codec.
    pub codec: Arc<DvbT2Concat>,
    /// Shared DVB-T2 bit interleaver.
    pub interleaver: Arc<DvbT2BitInterleaver>,
    /// Shared DVB-T2 cell mapper of the map and demap stages.
    pub cell_mapper: Arc<DvbT2CellMapper>,
}

/// Builds the ordered DVB-T2 BICM forward + inverse stage chains for a MODCOD.
///
/// The forward chain produces the cells of `@/citation/Etsi2015` clause 6.2
/// before constellation rotation: BCH and LDPC encoding, bit interleaving,
/// the bit-to-cell-word demultiplexer and cell-word mapping normalized to
/// unit average energy. The 256-QAM chain reproduces the cells of an
/// external reference stream (`crates/gf2-sim/tests/dvb_t2_chain_cells_vv001.rs`);
/// the other orders follow the same clause 6.2 tables without an external
/// stream check.
///
/// The codec is constructed for [`FrameSize::Normal`] (n=64800) with the
/// `decoder` configuration applied. `demap_noise_var` is the per-cell total
/// complex AWGN noise variance (`N0 = 2 sigma^2`) the soft demapper assumes;
/// a chain with no channel passes [`DEFAULT_DEMAP_NOISE_VAR`].
///
/// # Panics
///
/// Panics if the `(FrameSize::Normal, rate)` pair cannot construct a codec, if
/// `rate` / `modulation` is unsupported by the bit interleaver, or if
/// `demap_noise_var` is not finite and strictly positive (per
/// [`DvbT2CellDemap::with_noise_var`]).
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
    let cell_mapper = Arc::new(DvbT2CellMapper::new(modcod));

    let forward: Vec<Box<dyn AnyStage>> = vec![
        erase(DvbT2Encode::new(codec.clone())),
        erase(BitInterleave::new(interleaver.clone())),
        erase(DvbT2CellMap::new(cell_mapper.clone())),
    ];
    let inverse: Vec<Box<dyn AnyStage>> = vec![
        erase(DvbT2CellDemap::with_noise_var(
            cell_mapper.clone(),
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
        cell_mapper,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::AwgnLlrSource;
    use gf2_coding::ldpc::dvb_t2::concat::ConcatError;
    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    use gf2_coding::traits::BatchSoftDecoder;

    /// Noise deviations of consecutive frames, from clean to undecodable, so
    /// that a wave holds frames of each kind.
    const SIGMAS: [f64; 6] = [0.3, 0.6, 1.3, 0.55, 1.0, 0.66];

    /// The short-frame rate-1/2 codec (`@/citation/Etsi2015`) with a low
    /// iteration cap.
    fn short_codec() -> Arc<DvbT2Concat> {
        let mut codec = DvbT2Concat::new(FrameSize::Short, CodeRate::Rate1_2).unwrap();
        codec.set_max_ldpc_iterations(12);
        Arc::new(codec)
    }

    /// `count` FECFRAMEs of random BBFRAMEs through BPSK and AWGN, frame `f`
    /// at the deviation [`SIGMAS`] gives its index.
    fn noisy_frames(codec: &DvbT2Concat, count: usize) -> Vec<Vec<Llr>> {
        let mut source = AwgnLlrSource::new(0x391f_5419);
        (0..count)
            .map(|f| {
                let fecframe = codec.encode(&BitVec::random_seeded(codec.k_bch(), f as u64));
                source.frame_for_codeword(&fecframe, SIGMAS[f % SIGMAS.len()])
            })
            .collect()
    }

    /// BBFRAME, LDPC iteration count and convergence of
    /// [`DvbT2Concat::decode_soft_counted`], from either arm of its result.
    fn per_frame(codec: &DvbT2Concat, llrs: &[Llr]) -> (BitVec, u64, bool) {
        match codec.decode_soft_counted(llrs) {
            Ok((bbframe, iterations)) => (bbframe, iterations as u64, true),
            Err(ConcatError::LdpcDecodeFailed {
                bbframe,
                iterations,
            }) => (bbframe, iterations as u64, false),
            Err(error) => panic!("{error}"),
        }
    }

    #[test]
    fn decode_stage_matches_per_frame_decoding_at_each_batch_size() {
        let codec = short_codec();
        let lanes = DvbT2ConcatBatchDecoder::new(&codec).batch_granularity();
        let frames = noisy_frames(&codec, lanes + 3);
        let expected: Vec<_> = frames.iter().map(|llrs| per_frame(&codec, llrs)).collect();

        let stage = DvbT2Decode::new(codec.clone());
        let mut scratch = DecodeScratch::default();
        for count in [0, 1, lanes - 3, lanes, lanes + 3] {
            let out = stage
                .process(&LlrBatch::new(frames[..count].to_vec()), &mut scratch)
                .unwrap();

            assert_eq!(out.frames.len(), count);
            assert_eq!(scratch.iterations.len(), count);
            for (f, (bbframe, iterations, _)) in expected[..count].iter().enumerate() {
                assert_eq!(&out.frames[f], bbframe, "{count} frames, frame {f}");
                assert_eq!(
                    scratch.iterations[f], *iterations,
                    "{count} frames, frame {f}"
                );
            }
            if count >= lanes - 3 {
                let converged = expected[..count].iter().filter(|e| e.2).count();
                assert!(converged > 0 && converged < count, "{count} frames");
            }
        }
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
        assert_eq!(s.forward[0].output_type(), bitpacked);
        assert_eq!(s.forward[1].output_type(), bitpacked);
        assert_eq!(s.forward[2].input_type(), bitpacked);
        assert_eq!(s.forward[2].output_type(), symbol);

        assert_eq!(s.inverse[0].input_type(), symbol);
        assert_eq!(s.inverse[0].output_type(), llr);
        assert_eq!(s.inverse[1].output_type(), llr);
        assert_eq!(s.inverse[2].input_type(), llr);
        assert_eq!(s.inverse[2].output_type(), hard);
    }

    #[test]
    #[should_panic(expected = "noise_var must be finite and > 0")]
    fn test_demap_rejects_nonpositive_noise_var() {
        let mapper = Arc::new(DvbT2CellMapper::new(DvbT2Modcod::new(
            FrameSize::Normal,
            CodeRate::Rate1_2,
            DvbT2Modulation::Qam16,
        )));
        let _ = DvbT2CellDemap::with_noise_var(mapper, DemapMethod::ExactLogMap, 0.0);
    }
}
