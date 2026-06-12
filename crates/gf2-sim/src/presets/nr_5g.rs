//! 5G NR LDPC BICM preset: a typestate fluent builder over the graph API.
//!
//! The 5G NR sibling of the DVB-T2 preset ([`dvb_t2`](crate::presets::dvb_t2),
//! design doc §9): a **thin wrapper** over the graph
//! [`Chain`](crate::graph::Chain) that composes the 5G NR stage wrappers from
//! [`stages::nr_5g`](crate::stages::nr_5g) — rate-matched LDPC encode, the
//! TS 38.212 §5.4.2.2 bit interleaver, Gray-QAM map, AWGN channel, soft demap,
//! §5.4.2.2 LLR deinterleave, and rate-matched BP decode — into a seven-stage
//! linear pipeline. None of the LDPC / rate-matching / interleaver / QAM math
//! is implemented here. 5G NR has **no outer code** at the LDPC layer (unlike
//! DVB-T2's BCH+LDPC concatenation), so the chain is a single inner code.
//!
//! # Typestate stage ordering
//!
//! The required methods must be called in order —
//! [`base_graph`](Builder::base_graph) → [`lifting_size`](Builder::lifting_size)
//! → [`rate`](Builder::rate) → [`decoder`](Builder::decoder) →
//! [`demap`](Builder::demap) → [`channel`](Builder::channel) — and each
//! consumes `self`, returning a `Builder` in the next state. Calling them out
//! of order (e.g. [`lifting_size`](Builder::lifting_size) before
//! [`base_graph`](Builder::base_graph)) is a **compile-time** error because the
//! method only exists on the predecessor state. The optional
//! [`lifting_set`](Builder::lifting_set) refinement is available (only) in the
//! same state as `lifting_size`; the non-state-advancing setters
//! ([`parallelism`](Builder::parallelism), [`seed`](Builder::seed),
//! [`checkpoint_dir`](Builder::checkpoint_dir)) are available on [`Ready`].
//! Only a [`Builder<Ready>`] exposes [`build`](Builder::build).
//!
//! # Parameter scope (3GPP TS 38.212)
//!
//! * **Base graph**: BG1 (46x68, K_b = 22) or BG2 (42x52, K_b = 10).
//! * **Lifting size** `Z`: any of the 51 values of Table 5.3.2-1; the optional
//!   [`lifting_set`](Builder::lifting_set) index is cross-checked against `Z`
//!   at build time.
//! * **Rate** (per the §7.2.2 operating region): BG1 x {1/3, 1/2, 2/3, 5/6};
//!   BG2 x {1/3, 1/2, 2/3} (BG2 is capped at R <= 0.67, so BG2 + 5/6 is
//!   rejected).
//! * **Modulation**: QPSK / 16-QAM / 64-QAM / 256-QAM (`Q_m` ∈ {2, 4, 6, 8}).
//!
//! The message length is the **largest payload realising exactly the requested
//! `Z`** ([`max_payload_for_lifting`]): `22 * Z` for BG1 and the largest
//! self-consistent §5.2.2 band payload for BG2. The codeword length `E`
//! (= `target_n`) is derived from the rate in the floor form of the TS 38.212
//! §5.4.2.1 bit-selection formula (`E_r = N_L·Q_m·⌊G/(N_L·Q_m·C')⌋`), which
//! makes `E` a multiple of `Q_m` **by construction** — the §5.4.2.2
//! interleaver's `E mod Q_m == 0` precondition is guaranteed upstream in the
//! standard, not rejected: `E = ⌊k·den/(num·Q_m)⌋·Q_m` for rate `num/den`.
//! This equals the exact `k·den/num` whenever that ratio is already a
//! `Q_m`-multiple integer (e.g. every rate at BG1 `Z = 384` under QPSK); in
//! the fractional cases the rate is nominal (the realized `k/E` is slightly
//! above the requested rate).
//!
//! # Driving the built pipeline
//!
//! Drive the built pipeline with the generic per-stage executor
//! [`TopologyExecutor::run`](crate::TopologyExecutor::run) (see the
//! [`Pipeline::nr_5g`](crate::Pipeline::nr_5g) doctest). Sweep-level
//! [`Pipeline::run`](crate::Pipeline::run) integration (the scheduler's
//! SNR-sweep engine, currently DVB-T2-specific on its CPU arm) is the GPU
//! tuning / benchmark task's scope (`23d3525f`); this preset attaches no run
//! plan.

use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use gf2_coding::ldpc::nr_5g::{lifting_set_index, max_payload_for_lifting};
use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
use gf2_coding::modem::DemapMethod;

use crate::channels::Awgn;
use crate::error::BuildError;
use crate::graph::Chain;
use crate::pipeline::Pipeline;
use crate::stage::erase;
use crate::stages::nr_5g::{
    Nr5gBitInterleave, Nr5gDecode, Nr5gEncode, Nr5gLlrDeinterleave, NrGrayQamDemap, NrGrayQamMap,
};
use crate::PipelineConfig;

/// A 5G NR LDPC base graph (3GPP TS 38.212 §5.3.2).
///
/// BG1 is the high-rate / large-block graph (46x68, K_b = 22); BG2 is the
/// low-rate / small-block graph (42x52, K_b = 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseGraph {
    /// Base graph 1: 46x68, K_b = 22.
    Bg1,
    /// Base graph 2: 42x52, K_b = 10.
    Bg2,
}

impl BaseGraph {
    /// The TS 38.212 base-graph number (`1` or `2`), as consumed by the
    /// `gf2-coding` `nr_5g` constructors.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::presets::nr_5g::BaseGraph;
    ///
    /// assert_eq!(BaseGraph::Bg1.number(), 1);
    /// assert_eq!(BaseGraph::Bg2.number(), 2);
    /// ```
    #[inline]
    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            BaseGraph::Bg1 => 1,
            BaseGraph::Bg2 => 2,
        }
    }
}

/// A 5G NR LDPC code rate selector for the preset.
///
/// The in-scope rates per base graph follow the TS 38.212 §7.2.2 operating
/// region: BG1 x {1/3, 1/2, 2/3, 5/6}; BG2 x {1/3, 1/2, 2/3} (§7.2.2 caps BG2
/// at R <= 0.67, so BG2 + [`R5_6`](Nr5gRate::R5_6) is rejected at
/// [`build`](Builder::build)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nr5gRate {
    /// Rate 1/3 (the BG1 mother-code rate).
    R1_3,
    /// Rate 1/2.
    R1_2,
    /// Rate 2/3.
    R2_3,
    /// Rate 5/6 (BG1 only).
    R5_6,
}

impl Nr5gRate {
    /// The `(numerator, denominator)` of this rate.
    fn num_den(self) -> (usize, usize) {
        match self {
            Nr5gRate::R1_3 => (1, 3),
            Nr5gRate::R1_2 => (1, 2),
            Nr5gRate::R2_3 => (2, 3),
            Nr5gRate::R5_6 => (5, 6),
        }
    }

    /// Human-readable label (e.g. `"5/6"`) for error reporting.
    fn label(self) -> &'static str {
        match self {
            Nr5gRate::R1_3 => "1/3",
            Nr5gRate::R1_2 => "1/2",
            Nr5gRate::R2_3 => "2/3",
            Nr5gRate::R5_6 => "5/6",
        }
    }
}

/// A 5G NR data-channel modulation (TS 38.214): `Q_m` bits per QAM symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NrModulation {
    /// QPSK (`Q_m` = 2).
    Qpsk,
    /// 16-QAM (`Q_m` = 4).
    Qam16,
    /// 64-QAM (`Q_m` = 6).
    Qam64,
    /// 256-QAM (`Q_m` = 8).
    Qam256,
}

impl NrModulation {
    /// The modulation order `Q_m` (bits per QAM symbol).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::presets::nr_5g::NrModulation;
    ///
    /// assert_eq!(NrModulation::Qpsk.bits_per_symbol(), 2);
    /// assert_eq!(NrModulation::Qam256.bits_per_symbol(), 8);
    /// ```
    #[inline]
    #[must_use]
    pub fn bits_per_symbol(self) -> usize {
        match self {
            NrModulation::Qpsk => 2,
            NrModulation::Qam16 => 4,
            NrModulation::Qam64 => 6,
            NrModulation::Qam256 => 8,
        }
    }

    /// Human-readable label (e.g. `"16-QAM"`) for error reporting.
    fn label(self) -> &'static str {
        match self {
            NrModulation::Qpsk => "QPSK",
            NrModulation::Qam16 => "16-QAM",
            NrModulation::Qam64 => "64-QAM",
            NrModulation::Qam256 => "256-QAM",
        }
    }
}

/// The BP decoder configuration for the 5G NR preset.
///
/// Bundles the check-node update algorithm and the per-frame iteration cap the
/// [`Nr5gDecode`] stage runs with. Both are validated at
/// [`build`](Builder::build) (a typed [`BuildError::InvalidNr5gParams`] instead
/// of a downstream panic).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nr5gDecoderConfig {
    /// The check-node update algorithm.
    pub algorithm: DecoderAlgorithm,
    /// The BP iteration cap per frame.
    pub max_iterations: usize,
}

impl Nr5gDecoderConfig {
    /// Creates a decoder configuration.
    ///
    /// # Arguments
    ///
    /// * `algorithm` — the check-node update algorithm. Validated at
    ///   [`build`](Builder::build): `NormalizedMinSum(alpha)` requires a finite
    ///   `alpha` in `(0.0, 1.0]`, `OffsetMinSum(beta)` a finite `beta >= 0.0`.
    /// * `max_iterations` — the BP iteration cap; must be `>= 1` (validated at
    ///   build).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::presets::nr_5g::Nr5gDecoderConfig;
    /// use gf2_coding::ldpc::DecoderAlgorithm;
    ///
    /// let cfg = Nr5gDecoderConfig::new(DecoderAlgorithm::SumProduct, 25);
    /// assert_eq!(cfg.max_iterations, 25);
    /// ```
    #[must_use]
    pub fn new(algorithm: DecoderAlgorithm, max_iterations: usize) -> Self {
        Self {
            algorithm,
            max_iterations,
        }
    }

    /// The standard 5G NR normalized min-sum configuration (`alpha` = 0.75).
    ///
    /// # Arguments
    ///
    /// * `max_iterations` — the BP iteration cap; must be `>= 1` (validated at
    ///   build).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::presets::nr_5g::Nr5gDecoderConfig;
    /// use gf2_coding::ldpc::DecoderAlgorithm;
    ///
    /// let cfg = Nr5gDecoderConfig::normalized_min_sum(25);
    /// assert_eq!(cfg.algorithm, DecoderAlgorithm::NormalizedMinSum(0.75));
    /// ```
    #[must_use]
    pub fn normalized_min_sum(max_iterations: usize) -> Self {
        Self::new(DecoderAlgorithm::NormalizedMinSum(0.75), max_iterations)
    }

    /// Validates the configuration the way the downstream constructors would
    /// otherwise panic on, returning a typed error instead.
    fn validate(self) -> Result<(), BuildError> {
        if self.max_iterations == 0 {
            return Err(BuildError::InvalidNr5gParams {
                reason: "decoder max_iterations must be >= 1, got 0".to_string(),
            });
        }
        match self.algorithm {
            DecoderAlgorithm::NormalizedMinSum(alpha)
                if !(alpha.is_finite() && alpha > 0.0 && alpha <= 1.0) =>
            {
                Err(BuildError::InvalidNr5gParams {
                    reason: format!(
                        "NormalizedMinSum alpha must be finite and in (0.0, 1.0], got {alpha}"
                    ),
                })
            }
            DecoderAlgorithm::OffsetMinSum(beta) if !(beta.is_finite() && beta >= 0.0) => {
                Err(BuildError::InvalidNr5gParams {
                    reason: format!("OffsetMinSum beta must be finite and >= 0.0, got {beta}"),
                })
            }
            _ => Ok(()),
        }
    }
}

/// A channel selector for the 5G NR preset.
///
/// Constructs the channel [`Stage`](crate::Stage) inserted between the forward
/// (transmit) and inverse (receive) halves of the chain at
/// [`build`](Builder::build) time. The thin selector mirrors the DVB-T2
/// preset's [`Channel`](crate::presets::dvb_t2::Channel); both delegate the
/// Es/N0 → noise-variance arithmetic to the SSOT helpers in
/// [`channels`](crate::channels), so no conversion math is duplicated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Channel {
    /// An AWGN channel at the given Es/N0 (dB).
    Awgn {
        /// Channel Es/N0 in dB.
        es_n0_db: f32,
    },
}

impl Channel {
    /// An AWGN channel at the given Es/N0 (dB).
    ///
    /// # Arguments
    ///
    /// * `es_n0_db` — channel Es/N0 in dB.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::presets::nr_5g::Channel;
    ///
    /// let ch = Channel::awgn(4.5);
    /// assert_eq!(ch, Channel::Awgn { es_n0_db: 4.5 });
    /// ```
    #[must_use]
    pub fn awgn(es_n0_db: f32) -> Self {
        Channel::Awgn { es_n0_db }
    }

    /// Materialises the channel into an erased [`Awgn`] stage for
    /// `bits_per_symbol`.
    fn into_stage(self, bits_per_symbol: usize) -> Box<dyn crate::stage::AnyStage> {
        match self {
            Channel::Awgn { es_n0_db } => erase(Awgn::new(es_n0_db, bits_per_symbol)),
        }
    }

    /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`) the
    /// soft demapper must assume to be physically consistent with this channel
    /// (the SSOT once-rounded `f64` derivation,
    /// [`es_n0_db_to_n0`](crate::channels::es_n0_db_to_n0)).
    fn demap_noise_var(self) -> f32 {
        match self {
            Channel::Awgn { es_n0_db } => crate::channels::es_n0_db_to_n0(es_n0_db),
        }
    }

    /// Validates this channel's parameters, returning the demapper `N0` it
    /// implies on success.
    ///
    /// Rejects a non-finite Es/N0 (`NaN`/`±inf`) and any parameter whose
    /// derived demapper noise variance is not finite and strictly positive —
    /// exactly the inputs that would otherwise panic
    /// [`NrGrayQamDemap::with_noise_var`].
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::InvalidChannel`] (with a human-readable reason)
    /// when the Es/N0 is non-finite or the derived `N0` is non-finite or
    /// `<= 0`.
    fn validate(self) -> Result<f32, BuildError> {
        match self {
            Channel::Awgn { es_n0_db } => {
                if !es_n0_db.is_finite() {
                    return Err(BuildError::InvalidChannel {
                        reason: format!("AWGN Es/N0 must be a finite number of dB, got {es_n0_db}"),
                    });
                }
                let n0 = self.demap_noise_var();
                if !n0.is_finite() || n0 <= 0.0 {
                    return Err(BuildError::InvalidChannel {
                        reason: format!(
                            "AWGN Es/N0 = {es_n0_db} dB yields a demapper noise variance \
                             N0 = {n0} that is not finite and strictly positive \
                             (Es/N0 too large, underflowing N0 to zero)"
                        ),
                    });
                }
                Ok(n0)
            }
        }
    }
}

// ===========================================================================
// Typestate marker types
// ===========================================================================

/// Typestate marker: the base graph has not been selected yet.
///
/// The initial state of a fresh [`Builder`]; only [`Builder::base_graph`] is
/// available.
#[derive(Debug)]
pub enum NeedsBaseGraph {}

/// Typestate marker: the base graph is set; the lifting size is next.
///
/// [`Builder::lifting_size`] advances the state; the optional
/// [`Builder::lifting_set`] refinement is available here too.
#[derive(Debug)]
pub enum NeedsLifting {}

/// Typestate marker: the lifting size is set; the code rate is next.
///
/// Only [`Builder::rate`] is available.
#[derive(Debug)]
pub enum NeedsRate {}

/// Typestate marker: the rate is set; the decoder configuration is next.
///
/// Only [`Builder::decoder`] is available.
#[derive(Debug)]
pub enum NeedsDecoder {}

/// Typestate marker: the decoder is set; the modulation + demap method is next.
///
/// Only [`Builder::demap`] is available.
#[derive(Debug)]
pub enum NeedsDemap {}

/// Typestate marker: the demap is set; the channel is next.
///
/// Only [`Builder::channel`] is available.
#[derive(Debug)]
pub enum NeedsChannel {}

/// Typestate marker: every required stage is specified; the builder is ready.
///
/// The optional setters ([`Builder::parallelism`], [`Builder::seed`],
/// [`Builder::checkpoint_dir`]) and [`Builder::build`] are available.
#[derive(Debug)]
pub enum Ready {}

// ===========================================================================
// Builder
// ===========================================================================

/// A typestate fluent builder for the 5G NR LDPC BICM pipeline.
///
/// The generic `State` parameter (one of [`NeedsBaseGraph`], [`NeedsLifting`],
/// [`NeedsRate`], [`NeedsDecoder`], [`NeedsDemap`], [`NeedsChannel`],
/// [`Ready`]) tracks how far the chain has been specified, so the compiler
/// enforces the call order. Construct one via
/// [`Pipeline::nr_5g`](crate::Pipeline::nr_5g); see the [module docs](self)
/// for the full call sequence.
///
/// The required fields are stored as `Option`s populated in typestate order;
/// each is guaranteed `Some` by the time [`Ready`] is reached, so
/// [`build`](Builder::build) unwraps them with an internal invariant message.
pub struct Builder<State> {
    // The storage fields are deliberately NOT named after the public methods
    // (same rationale as the DVB-T2 preset): a private field sharing a name
    // with a method muddies the out-of-order compile error. The `cfg_` prefix
    // keeps the compile-fail diagnostic a clean "no method named `lifting_size`
    // ... for Builder<NeedsBaseGraph>".
    cfg_base_graph: Option<BaseGraph>,
    cfg_lifting_set: Option<usize>,
    cfg_lifting_size: Option<usize>,
    cfg_rate: Option<Nr5gRate>,
    cfg_decoder: Option<Nr5gDecoderConfig>,
    cfg_modulation: Option<NrModulation>,
    cfg_demap: Option<DemapMethod>,
    cfg_channel: Option<Channel>,
    cfg_parallelism: NonZeroUsize,
    cfg_seed: u64,
    cfg_checkpoint_dir: Option<PathBuf>,
    _state: PhantomData<fn() -> State>,
}

impl Builder<NeedsBaseGraph> {
    /// Creates a fresh builder in the [`NeedsBaseGraph`] state.
    ///
    /// The default optional settings are: `parallelism = 1`, `seed = 0`, no
    /// lifting-set refinement, and no checkpoint directory. Prefer the
    /// [`Pipeline::nr_5g`](crate::Pipeline::nr_5g) entry point.
    pub(crate) fn new() -> Self {
        Self {
            cfg_base_graph: None,
            cfg_lifting_set: None,
            cfg_lifting_size: None,
            cfg_rate: None,
            cfg_decoder: None,
            cfg_modulation: None,
            cfg_demap: None,
            cfg_channel: None,
            cfg_parallelism: NonZeroUsize::new(1).expect("1 is non-zero"),
            cfg_seed: 0,
            cfg_checkpoint_dir: None,
            _state: PhantomData,
        }
    }

    /// Selects the base graph, advancing to [`NeedsLifting`].
    ///
    /// # Arguments
    ///
    /// * `base_graph` — BG1 or BG2.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::nr_5g::BaseGraph;
    ///
    /// let _b = Pipeline::nr_5g().base_graph(BaseGraph::Bg1);
    /// ```
    pub fn base_graph(self, base_graph: BaseGraph) -> Builder<NeedsLifting> {
        self.with_state(|b| b.cfg_base_graph = Some(base_graph))
    }
}

impl Builder<NeedsLifting> {
    /// Records the expected lifting-set index `i_LS` (0..=7, TS 38.212
    /// Table 5.3.2-1). Optional and non-state-advancing.
    ///
    /// At [`build`](Builder::build) the recorded index is cross-checked against
    /// the chosen [`lifting_size`](Builder::lifting_size): a mismatch (or an
    /// index outside 0..=7) yields [`BuildError::InvalidNr5gParams`]. When this
    /// method is not called, the set index is derived from `Z` via
    /// [`lifting_set_index`].
    ///
    /// # Arguments
    ///
    /// * `i_ls` — the expected lifting-set index per Table 5.3.2-1.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::nr_5g::BaseGraph;
    /// use gf2_coding::ldpc::nr_5g::lifting_set_index;
    ///
    /// // Z = 104 belongs to the a = 13 set; derive the index, don't hardcode.
    /// let i_ls = lifting_set_index(104).unwrap();
    /// let _b = Pipeline::nr_5g()
    ///     .base_graph(BaseGraph::Bg2)
    ///     .lifting_set(i_ls)
    ///     .lifting_size(104);
    /// ```
    #[must_use]
    pub fn lifting_set(mut self, i_ls: usize) -> Self {
        self.cfg_lifting_set = Some(i_ls);
        self
    }

    /// Selects the lifting size `Z`, advancing to [`NeedsRate`].
    ///
    /// `Z` must be one of the 51 valid lifting sizes of TS 38.212
    /// Table 5.3.2-1; validated at [`build`](Builder::build).
    ///
    /// # Arguments
    ///
    /// * `z` — the lifting (expansion) size.
    pub fn lifting_size(self, z: usize) -> Builder<NeedsRate> {
        self.with_state(|b| b.cfg_lifting_size = Some(z))
    }
}

impl Builder<NeedsRate> {
    /// Selects the code rate, advancing to [`NeedsDecoder`].
    ///
    /// The rate must be in the chosen base graph's operating region (BG2 caps
    /// at 2/3); validated at [`build`](Builder::build).
    ///
    /// # Arguments
    ///
    /// * `rate` — the nominal code rate.
    pub fn rate(self, rate: Nr5gRate) -> Builder<NeedsDecoder> {
        self.with_state(|b| b.cfg_rate = Some(rate))
    }
}

impl Builder<NeedsDecoder> {
    /// Sets the LDPC BP decoder configuration, advancing to [`NeedsDemap`].
    ///
    /// # Arguments
    ///
    /// * `decoder` — algorithm + iteration cap; validated at
    ///   [`build`](Builder::build).
    pub fn decoder(self, decoder: Nr5gDecoderConfig) -> Builder<NeedsDemap> {
        self.with_state(|b| b.cfg_decoder = Some(decoder))
    }
}

impl Builder<NeedsDemap> {
    /// Sets the modulation and soft-demap method, advancing to
    /// [`NeedsChannel`].
    ///
    /// The modulation order `Q_m` parameterises the §5.4.2.2 interleaver, the
    /// Gray-QAM mapper, and the soft demapper; the codeword length must be a
    /// multiple of `Q_m` (validated at [`build`](Builder::build)).
    ///
    /// # Arguments
    ///
    /// * `modulation` — QPSK / 16-QAM / 64-QAM / 256-QAM.
    /// * `method` — exact log-MAP or max-log demapping.
    pub fn demap(self, modulation: NrModulation, method: DemapMethod) -> Builder<NeedsChannel> {
        self.with_state(|b| {
            b.cfg_modulation = Some(modulation);
            b.cfg_demap = Some(method);
        })
    }
}

impl Builder<NeedsChannel> {
    /// Sets the channel, advancing to [`Ready`].
    ///
    /// # Arguments
    ///
    /// * `channel` — the channel inserted between the forward and inverse
    ///   halves.
    pub fn channel(self, channel: Channel) -> Builder<Ready> {
        self.with_state(|b| b.cfg_channel = Some(channel))
    }
}

impl Builder<Ready> {
    /// Sets the number of parallel workers carried on the built pipeline's
    /// [`PipelineConfig`]. Non-state-advancing.
    ///
    /// # Arguments
    ///
    /// * `parallelism` — the worker count.
    #[must_use]
    pub fn parallelism(mut self, parallelism: NonZeroUsize) -> Self {
        self.cfg_parallelism = parallelism;
        self
    }

    /// Sets the base RNG seed carried on the built pipeline's
    /// [`PipelineConfig`]. Non-state-advancing.
    ///
    /// # Arguments
    ///
    /// * `seed` — the base ChaCha20 seed (design doc §3).
    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.cfg_seed = seed;
        self
    }

    /// Sets the optional checkpoint directory carried on the built pipeline's
    /// [`PipelineConfig`]. Non-state-advancing.
    ///
    /// # Arguments
    ///
    /// * `checkpoint_dir` — the per-SNR checkpoint directory, or `None`.
    #[must_use]
    pub fn checkpoint_dir(mut self, checkpoint_dir: Option<PathBuf>) -> Self {
        self.cfg_checkpoint_dir = checkpoint_dir;
        self
    }

    /// Validates the parameters and compiles the 5G NR chain into a
    /// [`Pipeline`].
    ///
    /// Assembles the seven-stage chain — [`Nr5gEncode`] →
    /// [`Nr5gBitInterleave`] → [`NrGrayQamMap`] → [`Awgn`] →
    /// [`NrGrayQamDemap`] → [`Nr5gLlrDeinterleave`] → [`Nr5gDecode`] — into a
    /// [`Chain`](crate::graph::Chain), connects the stages consecutively, and
    /// returns [`Chain::build`](crate::graph::Chain::build)'s [`Pipeline`]
    /// carrying the configured `seed`, `parallelism`, and `checkpoint_dir`.
    ///
    /// The soft demapper's assumed noise variance is derived from the **same**
    /// channel (`N0 = 2 sigma^2` via the SSOT Es/N0 conversion), so the LLR
    /// scaling is physically consistent with the injected noise.
    ///
    /// The code dimensions are `target_k = `[`max_payload_for_lifting`]`(BG, Z)`
    /// and `target_n = ⌊target_k * den / (num * Q_m)⌋ * Q_m` for rate
    /// `num/den` (the §5.4.2.1 floor form, a `Q_m` multiple by construction;
    /// see the [module docs](self)); the built code is verified to realise
    /// **exactly** the requested `Z`.
    ///
    /// # Errors
    ///
    /// * [`BuildError::InvalidNr5gParams`] if `Z` is not a valid TS 38.212
    ///   Table 5.3.2-1 lifting size; if a [`lifting_set`](Builder::lifting_set)
    ///   index is inconsistent with `Z` (or outside 0..=7); if the rate is
    ///   outside the base graph's operating region (BG2 + 5/6, §7.2.2); if the
    ///   decoder configuration is invalid (zero iteration cap, out-of-range
    ///   min-sum scale); if the constructed code does not realise the
    ///   requested `Z`; or — defensively, unreachable from this builder's own
    ///   §5.4.2.1-shaped `E` derivation — if `E` is not a multiple of `Q_m`
    ///   or does not exceed `k`.
    /// * [`BuildError::InvalidChannel`] if a channel parameter is invalid — a
    ///   non-finite (`NaN`/`±inf`) AWGN Es/N0, or an Es/N0 so large that the
    ///   derived demapper noise variance underflows to a non-positive value.
    ///
    /// `build()` validates every input it receives and returns one of the
    /// above typed errors on bad input; it **never panics** on any public
    /// input combination (the typestate guarantees the required setters were
    /// called, and the chain wiring — a fixed seven-stage linear DAG with
    /// type-compatible consecutive edges — is well-formed by construction).
    ///
    /// # Complexity
    ///
    /// Dominated by the one-off rate-matched mother-code construction
    /// ([`QuasiCyclicLdpc::nr_5g_rate_matched`]: RREF on the `N_b * Z`-column
    /// parity-check matrix — for BG1 at `Z = 384` that is roughly a second);
    /// the graph assembly itself is O(1) in the number of stages.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::nr_5g::{
    ///     BaseGraph, Channel, Nr5gDecoderConfig, Nr5gRate, NrModulation,
    /// };
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// // A small BG2 code (Z = 52, k = 416, n = 1248) builds in milliseconds.
    /// let pipeline = Pipeline::nr_5g()
    ///     .base_graph(BaseGraph::Bg2)
    ///     .lifting_size(52)
    ///     .rate(Nr5gRate::R1_3)
    ///     .decoder(Nr5gDecoderConfig::normalized_min_sum(25))
    ///     .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
    ///     .channel(Channel::awgn(3.0))
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(pipeline.stage_count(), 7);
    /// ```
    pub fn build(self) -> Result<Pipeline, BuildError> {
        // Panic-on-bad-input audit (this method must NEVER panic on any public
        // input combination — it returns `Ok` or a typed `BuildError`):
        //   .base_graph(BaseGraph)   -> closed enum, no invalid value.
        //   .lifting_set(usize) /
        //   .lifting_size(usize)     -> validated below (valid Z + i_LS
        //                               consistency) -> InvalidNr5gParams;
        //                               guards the asserts in
        //                               max_payload_for_lifting and
        //                               nr_5g_rate_matched.
        //   .rate(Nr5gRate)          -> per-BG region validated below.
        //   .decoder(Nr5gDecoderConfig) -> validated below (guards the
        //                               DecoderConfig::new assert inside the
        //                               decode stage's per-frame decoder).
        //   .demap(NrModulation, DemapMethod) -> closed enums; the E % Q_m
        //                               divisibility is validated below (guards
        //                               the interleaver's assert).
        //   .channel(Channel)        -> validated below -> InvalidChannel
        //                               (guards NrGrayQamDemap::with_noise_var).
        //   .parallelism/.seed/.checkpoint_dir -> any value valid; copied
        //                               verbatim into the config.
        let base_graph = self.cfg_base_graph.expect("base graph set before Ready");
        let z = self
            .cfg_lifting_size
            .expect("lifting size set before Ready");
        let rate = self.cfg_rate.expect("rate set before Ready");
        let decoder = self.cfg_decoder.expect("decoder set before Ready");
        let modulation = self.cfg_modulation.expect("modulation set before Ready");
        let demap = self.cfg_demap.expect("demap method set before Ready");
        let channel = self.cfg_channel.expect("channel set before Ready");

        // (1) Z must be a Table 5.3.2-1 lifting size; derive its set index.
        let actual_i_ls = lifting_set_index(u16::try_from(z).unwrap_or(0)).ok_or_else(|| {
            BuildError::InvalidNr5gParams {
                reason: format!(
                    "Z = {z} is not a valid 5G NR lifting size \
                     (TS 38.212 Table 5.3.2-1)"
                ),
            }
        })?;

        // (2) (i_LS, Z) consistency per Table 5.3.2-1.
        if let Some(requested_i_ls) = self.cfg_lifting_set {
            if requested_i_ls != actual_i_ls {
                return Err(BuildError::InvalidNr5gParams {
                    reason: format!(
                        "lifting set i_LS = {requested_i_ls} is inconsistent with \
                         Z = {z}, which belongs to set i_LS = {actual_i_ls} \
                         (TS 38.212 Table 5.3.2-1)"
                    ),
                });
            }
        }

        // (3) Rate must be inside the base graph's operating region:
        //     BG1 x {1/3, 1/2, 2/3, 5/6}; BG2 x {1/3, 1/2, 2/3} (TS 38.212
        //     §7.2.2 caps BG2 at R <= 0.67).
        if base_graph == BaseGraph::Bg2 && rate == Nr5gRate::R5_6 {
            return Err(BuildError::InvalidNr5gParams {
                reason: format!(
                    "rate {} is outside BG2's operating region \
                     (TS 38.212 §7.2.2 caps BG2 at R <= 0.67); \
                     use BG1 for rate 5/6",
                    rate.label()
                ),
            });
        }

        // (4) Decoder configuration (guards the DecoderConfig::new panic).
        decoder.validate()?;

        // (5) Code dimensions. `target_k` is the largest payload realising
        //     exactly the requested Z (max_payload_for_lifting resolves the
        //     §5.2.2 K_b' fixed point). `target_n` (the rate-matched length E)
        //     is the spec-shaped floor form of TS 38.212 §5.4.2.1 — the
        //     bit-selection formula E_r = N_L * Q_m * floor(G / (N_L*Q_m*C'))
        //     makes E a multiple of Q_m BY CONSTRUCTION in the standard, so the
        //     §5.4.2.2 interleaver's rectangularity is guaranteed upstream,
        //     never rejected. Here: E = floor(k*den / (num*Q_m)) * Q_m, which
        //     equals the exact k*den/num whenever that ratio is already a
        //     Q_m-multiple integer. The rate is therefore nominal: the realized
        //     rate k/E is >= the requested num/den, with equality in the exact
        //     case (the floor only ever shrinks E, so it can never exceed the
        //     mother code's transmission budget and break the exact-Z
        //     realization).
        let target_k = max_payload_for_lifting(base_graph.number(), z);
        let (num, den) = rate.num_den();
        let q_m = modulation.bits_per_symbol();
        let target_n = (target_k * den) / (num * q_m) * q_m;

        // (6) §5.4.2.2 interleaver precondition: E = target_n must be a
        //     multiple of the modulation order Q_m. The derivation above
        //     guarantees this by construction (per §5.4.2.1), so this gate is
        //     defense-in-depth: build() is the authoritative validation gate
        //     (the same philosophy as Chain::build re-checking edges connect()
        //     already validated), and a future change to the E derivation must
        //     not be able to hand the interleaver a non-rectangular length.
        if !target_n.is_multiple_of(q_m) {
            return Err(BuildError::InvalidNr5gParams {
                reason: format!(
                    "codeword length E = {target_n} (BG{} Z = {z} rate {}) is not \
                     a multiple of the {} modulation order Q_m = {q_m}; the \
                     TS 38.212 §5.4.2.2 interleaver requires E mod Q_m == 0",
                    base_graph.number(),
                    rate.label(),
                    modulation.label()
                ),
            });
        }
        // Defense for the same reason: every in-scope rate is < 1 and the
        // smallest in-scope payload (BG2, Z = 2, K_b' = 6 -> k = 12) still
        // floors to E > k for every Q_m, but the rate-matched constructor
        // asserts target_n > target_k, so gate it typed-ly here.
        if target_n <= target_k {
            return Err(BuildError::InvalidNr5gParams {
                reason: format!(
                    "codeword length E = {target_n} does not exceed the message \
                     length k = {target_k} (BG{} Z = {z} rate {} {})",
                    base_graph.number(),
                    rate.label(),
                    modulation.label()
                ),
            });
        }

        // (7) Channel (guards NrGrayQamDemap::with_noise_var); the demapper N0
        //     is derived from the SAME channel for physical consistency.
        let demap_noise_var = channel.validate()?;

        // (8) Construct the rate-matched code. The inputs are pre-validated:
        //     base_graph ∈ {1, 2}, target_n > target_k (every in-scope rate is
        //     < 1), and a lifting size exists (max_payload_for_lifting's
        //     fixed-point payload selects exactly z for every rate >= 1/3), so
        //     none of nr_5g_rate_matched's asserts can fire. The realized Z is
        //     still verified — a mismatch is a typed error, not a panic.
        let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(
            base_graph.number(),
            target_n,
            target_k,
        ));
        let realized_z = code.params().lifting_factor;
        if realized_z != z {
            return Err(BuildError::InvalidNr5gParams {
                reason: format!(
                    "the (BG{}, Z = {z}, rate {}) tuple is not realisable at the \
                     requested lifting size: the TS 38.212 Z-selection lands on \
                     Z = {realized_z} for (n = {target_n}, k = {target_k})",
                    base_graph.number(),
                    rate.label()
                ),
            });
        }

        // Chain wiring: encode → §5.4.2.2 interleave → QAM map → AWGN →
        // QAM demap → §5.4.2.2 LLR deinterleave → decode. All stages are
        // CPU-only, so no fallback registration is needed.
        let mut chain = Chain::new();
        let ids = [
            chain.add(erase(Nr5gEncode::new(code.clone()))),
            chain.add(erase(Nr5gBitInterleave::new(q_m))),
            chain.add(erase(NrGrayQamMap::new(q_m))),
            chain.add(channel.into_stage(q_m)),
            chain.add(erase(NrGrayQamDemap::with_noise_var(
                q_m,
                demap,
                demap_noise_var,
            ))),
            chain.add(erase(Nr5gLlrDeinterleave::new(q_m))),
            chain.add(erase(Nr5gDecode::with_algorithm(
                code,
                decoder.algorithm,
                decoder.max_iterations,
            ))),
        ];
        for pair in ids.windows(2) {
            chain
                .connect(pair[0], pair[1])
                .expect("consecutive 5G NR chain stages are type-compatible");
        }

        let config = PipelineConfig {
            seed: self.cfg_seed,
            esn0_db_points: Vec::new(),
            target_errors: 0,
            max_frames: 0,
            heartbeat_every_frames: 0,
            checkpoint_dir: self.cfg_checkpoint_dir,
            tracing_log_path: None,
            parallelism: self.cfg_parallelism,
            gpu_enabled: false,
            strict_gpu: false,
            diagnostic_dump_dir: None,
            inject_gpu_oom_modulus: None,
        };

        // No run plan is attached: `Pipeline::run`'s sweep engine is DVB-T2-
        // specific on its CPU arm; drive this pipeline with `TopologyExecutor`
        // (see the module docs). Sweep integration is `23d3525f`'s scope.
        chain.with_config(config).build()
    }
}

impl<State> Builder<State> {
    /// Re-tags the builder into the `Next` typestate after applying `mutate`.
    ///
    /// Moving the fields field-by-field (rather than transmuting) keeps the
    /// state transition zero-cost while preserving the `#![deny(unsafe_code)]`
    /// guarantee.
    fn with_state<Next>(mut self, mutate: impl FnOnce(&mut Self)) -> Builder<Next> {
        mutate(&mut self);
        Builder {
            cfg_base_graph: self.cfg_base_graph,
            cfg_lifting_set: self.cfg_lifting_set,
            cfg_lifting_size: self.cfg_lifting_size,
            cfg_rate: self.cfg_rate,
            cfg_decoder: self.cfg_decoder,
            cfg_modulation: self.cfg_modulation,
            cfg_demap: self.cfg_demap,
            cfg_channel: self.cfg_channel,
            cfg_parallelism: self.cfg_parallelism,
            cfg_seed: self.cfg_seed,
            cfg_checkpoint_dir: self.cfg_checkpoint_dir,
            _state: PhantomData,
        }
    }
}

impl Pipeline {
    /// Starts a 5G NR LDPC preset builder in the [`NeedsBaseGraph`] state.
    ///
    /// This is the entry point for the typestate fluent builder; the required
    /// stages must then be specified in order
    /// ([`base_graph`](Builder::base_graph) →
    /// [`lifting_size`](Builder::lifting_size) → [`rate`](Builder::rate) →
    /// [`decoder`](Builder::decoder) → [`demap`](Builder::demap) →
    /// [`channel`](Builder::channel)), after which the optional setters and
    /// [`build`](Builder::build) become available. See the
    /// [module docs](crate::presets::nr_5g).
    ///
    /// # Examples
    ///
    /// The full BG1 / `Z` = 384 / rate-1/2 chain, driven end-to-end through the
    /// generic per-stage executor — one QPSK frame at 6 dB Es/N0 decodes back
    /// to the transmitted message:
    ///
    /// ```
    /// use std::num::NonZeroUsize;
    /// use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch};
    /// use gf2_sim::presets::nr_5g::{
    ///     BaseGraph, Channel, Nr5gDecoderConfig, Nr5gRate, NrModulation,
    /// };
    /// use gf2_sim::stage::TypedBatch;
    /// use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};
    /// use gf2_coding::ldpc::nr_5g::lifting_set_index;
    /// use gf2_coding::modem::DemapMethod;
    /// use gf2_core::BitVec;
    ///
    /// // Z = 384 belongs to lifting set i_LS = 1 (the a = 3 set of TS 38.212
    /// // Table 5.3.2-1: 384 = 3 * 2^7) — derive the index, don't hardcode it.
    /// let i_ls = lifting_set_index(384).expect("384 is a valid lifting size");
    /// assert_eq!(i_ls, 1);
    ///
    /// let pipeline = Pipeline::nr_5g()
    ///     .base_graph(BaseGraph::Bg1)
    ///     .lifting_set(i_ls)
    ///     .lifting_size(384)
    ///     .rate(Nr5gRate::R1_2)
    ///     .decoder(Nr5gDecoderConfig::normalized_min_sum(25))
    ///     .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
    ///     .channel(Channel::awgn(6.0))
    ///     .seed(0x5697_4242)
    ///     .build()
    ///     .expect("BG1 / Z = 384 / rate 1/2 / QPSK builds");
    /// assert_eq!(pipeline.stage_count(), 7);
    ///
    /// // BG1 full payload at Z = 384: k = 22 * 384 = 8448 message bits.
    /// let k = 22 * 384;
    /// let mut msg = BitVec::with_capacity(k);
    /// for i in 0..k {
    ///     msg.push_bit(i % 5 < 2);
    /// }
    ///
    /// // Drive one frame end-to-end (encode → … → decode) through the
    /// // generic per-stage executor.
    /// let sched = Scheduler::new(NonZeroUsize::new(2).unwrap(), false, 42);
    /// let out = TopologyExecutor::run(
    ///     &pipeline,
    ///     &sched,
    ///     Box::new(BitPackedBatch::new(vec![msg.clone()])),
    /// )
    /// .expect("the 5G NR chain runs to completion")
    /// .into_single()
    /// .expect("a linear chain has exactly one sink");
    /// let decoded = out
    ///     .as_any()
    ///     .downcast_ref::<HardDecisionBatch>()
    ///     .expect("the chain ends in recovered message bits");
    /// assert_eq!(decoded.frames[0], msg, "the chain recovers the message");
    /// ```
    #[must_use]
    pub fn nr_5g() -> Builder<NeedsBaseGraph> {
        Builder::<NeedsBaseGraph>::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The standard decoder configuration used across the tests.
    fn nms25() -> Nr5gDecoderConfig {
        Nr5gDecoderConfig::normalized_min_sum(25)
    }

    /// A valid small builder (BG2, Z = 52, rate 1/3, QPSK) whose single
    /// varied input is supplied by the caller.
    fn small_builder() -> Builder<Ready> {
        Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(3.0))
    }

    #[test]
    fn test_build_produces_seven_stage_pipeline() {
        let pipeline = small_builder().build().expect("in-scope tuple builds");
        assert_eq!(pipeline.stage_count(), 7);
        assert_eq!(pipeline.edges().len(), 6);
    }

    #[test]
    fn test_build_threads_optional_config() {
        let dir = PathBuf::from("checkpoints");
        let pipeline = small_builder()
            .parallelism(NonZeroUsize::new(8).unwrap())
            .seed(0xABCD)
            .checkpoint_dir(Some(dir.clone()))
            .build()
            .expect("in-scope tuple builds");
        assert_eq!(pipeline.config().seed, 0xABCD);
        assert_eq!(pipeline.config().parallelism.get(), 8);
        assert_eq!(
            pipeline.config().checkpoint_dir.as_deref(),
            Some(dir.as_path())
        );
    }

    #[test]
    fn test_build_rejects_invalid_lifting_size() {
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg1)
            .lifting_size(100) // not in Table 5.3.2-1
            .rate(Nr5gRate::R1_2)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(6.0))
            .build();
        match result {
            Err(BuildError::InvalidNr5gParams { reason }) => {
                assert!(reason.contains("100"), "reason must name the bad Z");
            }
            Err(other) => panic!("expected InvalidNr5gParams, got {other:?}"),
            Ok(_) => panic!("expected InvalidNr5gParams, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_rejects_oversized_lifting_size_without_panicking() {
        // Larger than any u16 lifting size; the u16 narrowing must not panic.
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg1)
            .lifting_size(1 << 20)
            .rate(Nr5gRate::R1_2)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(6.0))
            .build();
        assert!(matches!(result, Err(BuildError::InvalidNr5gParams { .. })));
    }

    #[test]
    fn test_build_rejects_inconsistent_lifting_set() {
        // Z = 384 belongs to i_LS = 1; requesting set 0 is a mismatch.
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg1)
            .lifting_set(0)
            .lifting_size(384)
            .rate(Nr5gRate::R1_2)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(6.0))
            .build();
        match result {
            Err(BuildError::InvalidNr5gParams { reason }) => {
                assert!(
                    reason.contains("i_LS = 0") && reason.contains("i_LS = 1"),
                    "reason must name both the requested and the actual set: {reason}"
                );
            }
            Err(other) => panic!("expected InvalidNr5gParams, got {other:?}"),
            Ok(_) => panic!("expected InvalidNr5gParams, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_accepts_consistent_lifting_set() {
        let pipeline = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_set(lifting_set_index(52).unwrap()) // i_LS = 6 (a = 13)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(3.0))
            .build()
            .expect("consistent (i_LS, Z) builds");
        assert_eq!(pipeline.stage_count(), 7);
    }

    #[test]
    fn test_build_rejects_bg2_rate_5_6() {
        // TS 38.212 §7.2.2 caps BG2 at R <= 0.67 (acf9b11a amendment).
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(104)
            .rate(Nr5gRate::R5_6)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(6.0))
            .build();
        match result {
            Err(BuildError::InvalidNr5gParams { reason }) => {
                assert!(
                    reason.contains("5/6") && reason.contains("BG2"),
                    "reason must name the rejected rate and BG: {reason}"
                );
            }
            Err(other) => panic!("expected InvalidNr5gParams, got {other:?}"),
            Ok(_) => panic!("expected InvalidNr5gParams, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_accepts_bg1_rate_5_6_when_divisible() {
        // BG1 Z = 320 (a 5-divisible Z): k = 7040, n = 6k/5 = 8448 exactly;
        // 8448 % 4 == 0 so 16-QAM works too.
        let pipeline = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg1)
            .lifting_size(320)
            .rate(Nr5gRate::R5_6)
            .decoder(nms25())
            .demap(NrModulation::Qam16, DemapMethod::MaxLog)
            .channel(Channel::awgn(12.0))
            .build()
            .expect("BG1 x 5/6 is in the operating region");
        assert_eq!(pipeline.stage_count(), 7);
    }

    /// The §5.4.2.1 floor-form E derivation: when the exact `k * den / num`
    /// is not a Q_m-multiple integer, E floors to the next Q_m multiple, so
    /// the §5.4.2.2 interleaver's `E mod Q_m == 0` precondition holds by
    /// construction (TS 38.212 makes E a multiple of Q_m upstream — the
    /// E_r = N_L*Q_m*floor(...) bit-selection formula — rather than rejecting).
    #[test]
    fn test_e_floors_to_qm_multiple() {
        // BG1 Z = 384 rate 5/6 16-QAM: exact n would be 8448 * 6/5 = 10137.6;
        // floor to a multiple of Q_m = 4 gives E = 10136.
        let pipeline = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg1)
            .lifting_size(384)
            .rate(Nr5gRate::R5_6)
            .decoder(nms25())
            .demap(NrModulation::Qam16, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(12.0))
            .build()
            .expect("the floor-form E derivation makes every in-scope tuple buildable");
        let encode = pipeline.stages()[0]
            .stage_as_any()
            .expect("erased stage exposes its concrete stage")
            .downcast_ref::<Nr5gEncode>()
            .expect("first stage is the NR encode");
        assert_eq!(encode.k(), 8448, "k = 22 * 384");
        assert_eq!(encode.n(), 10136, "E = floor(8448 * 6 / (5 * 4)) * 4");
        assert_eq!(encode.n() % 4, 0, "E is a Q_m multiple by construction");
    }

    /// In the exact case the floor form changes nothing: BG1 Z = 384 r1/2
    /// QPSK has E = 2k = 16896 exactly (the doctest configuration).
    #[test]
    fn test_e_exact_when_divisible() {
        let pipeline = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg1)
            .lifting_size(16)
            .rate(Nr5gRate::R1_2)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(6.0))
            .build()
            .expect("exact-ratio tuple builds");
        let encode = pipeline.stages()[0]
            .stage_as_any()
            .expect("erased stage exposes its concrete stage")
            .downcast_ref::<Nr5gEncode>()
            .expect("first stage is the NR encode");
        assert_eq!(encode.k(), 352, "k = 22 * 16");
        assert_eq!(encode.n(), 704, "E = 2k exactly");
    }

    #[test]
    fn test_build_rejects_zero_decoder_iterations() {
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(Nr5gDecoderConfig::new(DecoderAlgorithm::SumProduct, 0))
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(3.0))
            .build();
        assert!(matches!(result, Err(BuildError::InvalidNr5gParams { .. })));
    }

    #[test]
    fn test_build_rejects_invalid_min_sum_scale_without_panicking() {
        // alpha = 0.0 would panic DecoderConfig::new inside the decode stage's
        // per-frame decoder; build() must reject it with a typed error first.
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(Nr5gDecoderConfig::new(
                DecoderAlgorithm::NormalizedMinSum(0.0),
                25,
            ))
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(3.0))
            .build();
        assert!(matches!(result, Err(BuildError::InvalidNr5gParams { .. })));

        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(Nr5gDecoderConfig::new(
                DecoderAlgorithm::OffsetMinSum(-0.5),
                25,
            ))
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(3.0))
            .build();
        assert!(matches!(result, Err(BuildError::InvalidNr5gParams { .. })));
    }

    #[test]
    fn test_build_rejects_nan_es_n0_without_panicking() {
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(f32::NAN))
            .build();
        assert!(matches!(result, Err(BuildError::InvalidChannel { .. })));
    }

    #[test]
    fn test_build_rejects_underflowing_es_n0_without_panicking() {
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(nms25())
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(1000.0))
            .build();
        assert!(matches!(result, Err(BuildError::InvalidChannel { .. })));
    }

    /// Every in-scope (BG, rate) pair builds at a representative lifting size
    /// of each BG2 K_b' band (small Z keeps this fast-tier); the §5.4.2.1
    /// floor-form E derivation makes every tuple buildable for every
    /// modulation order.
    #[test]
    fn test_build_all_in_scope_bg_rate_pairs() {
        let bg1_rates = [
            Nr5gRate::R1_3,
            Nr5gRate::R1_2,
            Nr5gRate::R2_3,
            Nr5gRate::R5_6,
        ];
        let bg2_rates = [Nr5gRate::R1_3, Nr5gRate::R1_2, Nr5gRate::R2_3];
        // BG1 5/6 needs a 5-divisible Z for an exact (and even) target_n;
        // Z = 20 gives k = 440, n = 528.
        for rate in bg1_rates {
            let pipeline = Pipeline::nr_5g()
                .base_graph(BaseGraph::Bg1)
                .lifting_size(20)
                .rate(rate)
                .decoder(nms25())
                .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
                .channel(Channel::awgn(6.0))
                .build()
                .unwrap_or_else(|e| panic!("BG1 x {} must build: {e:?}", rate.label()));
            assert_eq!(pipeline.stage_count(), 7);
        }
        // BG2 at one Z per K_b' band: 15 (kb=6), 52 (kb=8), 64 (kb=9),
        // 72 (kb=10) — crossed with every modulation order, including the
        // odd-exact-n corner (Z = 15 rate 2/3: exact n = 135, floored per
        // modulation).
        let modulations = [
            NrModulation::Qpsk,
            NrModulation::Qam16,
            NrModulation::Qam64,
            NrModulation::Qam256,
        ];
        for z in [15usize, 52, 64, 72] {
            for rate in bg2_rates {
                for modulation in modulations {
                    let pipeline = Pipeline::nr_5g()
                        .base_graph(BaseGraph::Bg2)
                        .lifting_size(z)
                        .rate(rate)
                        .decoder(nms25())
                        .demap(modulation, DemapMethod::ExactLogMap)
                        .channel(Channel::awgn(6.0))
                        .build()
                        .unwrap_or_else(|e| {
                            panic!(
                                "BG2 Z={z} x {} x {:?} must build: {e:?}",
                                rate.label(),
                                modulation
                            )
                        });
                    assert_eq!(pipeline.stage_count(), 7);
                }
            }
        }
    }

    /// A noisy end-to-end roundtrip through the generic per-stage executor at
    /// a comfortable Es/N0: the chain the doctest runs at Z = 384, exercised
    /// here at a fast small-Z configuration (BG1 Z = 16, rate 1/2, 16-QAM).
    #[test]
    fn test_chain_roundtrip_via_topology_executor() {
        use crate::batch::{BitPackedBatch, HardDecisionBatch};
        use crate::{Scheduler, TopologyExecutor};
        use gf2_core::BitVec;

        let pipeline = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg1)
            .lifting_size(16)
            .rate(Nr5gRate::R1_2)
            .decoder(nms25())
            .demap(NrModulation::Qam16, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(12.0))
            .seed(7)
            .build()
            .expect("BG1 Z=16 r1/2 16-QAM builds");
        // k = 22 * 16 = 352, n = 704, 704 % 4 == 0.

        let k = 22 * 16;
        let mut msg = BitVec::with_capacity(k);
        for i in 0..k {
            msg.push_bit(i % 3 == 1);
        }

        let sched = Scheduler::new(NonZeroUsize::new(2).unwrap(), false, 7);
        let out = TopologyExecutor::run(
            &pipeline,
            &sched,
            Box::new(BitPackedBatch::new(vec![msg.clone()])),
        )
        .expect("chain runs")
        .into_single()
        .expect("linear chain has one sink");
        let decoded = out
            .as_any()
            .downcast_ref::<HardDecisionBatch>()
            .expect("chain ends in HardDecisionBatch");
        assert_eq!(
            decoded.frames[0], msg,
            "high-SNR 5G NR roundtrip recovers the message"
        );
    }

    /// The preset realises exactly the requested Z even in the BG2 band where
    /// the naive full payload would mis-select (Z = 52 -> 72): the built
    /// chain's encode stage must carry the Z = 52 payload k = 8 * 52 = 416.
    #[test]
    fn test_bg2_small_z_realizes_requested_z() {
        let pipeline = small_builder().build().expect("BG2 Z=52 builds");
        let encode = pipeline.stages()[0]
            .stage_as_any()
            .expect("erased stage exposes its concrete stage")
            .downcast_ref::<Nr5gEncode>()
            .expect("first stage is the NR encode");
        assert_eq!(encode.k(), 416, "k = 8 * 52 (the kb=8 band payload)");
    }
}
