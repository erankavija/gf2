//! 5G NR LDPC BICM preset: a typestate fluent builder over the graph API.
//!
//! [`Pipeline::nr_5g`](crate::Pipeline::nr_5g) returns a
//! `Builder<NeedsBaseGraph>`. The required setters are called in order,
//! [`base_graph`](Builder::base_graph) → [`lifting_size`](Builder::lifting_size)
//! → [`rate`](Builder::rate) → [`decoder`](Builder::decoder) →
//! [`demap`](Builder::demap) → [`channel`](Builder::channel); each exists only
//! on its predecessor state, and [`lifting_set`](Builder::lifting_set) only
//! alongside `lifting_size`. A [`Builder<Ready>`] exposes the optional setters
//! and [`build`](Builder::build), which composes the
//! [`stages::nr_5g`](crate::stages::nr_5g) stages around an [`Awgn`] channel
//! into a seven-stage [`Chain`]. The built pipeline carries no run plan; drive
//! it with [`TopologyExecutor::run`](crate::TopologyExecutor::run).
//!
//! # Supported parameters (`@/citation/ThreeGpp2017`)
//!
//! * **Base graph**: BG1 (46x68, K_b = 22) or BG2 (42x52, K_b = 10).
//! * **Lifting size** `Z`: any value of Table 5.3.2-1; the optional
//!   [`lifting_set`](Builder::lifting_set) index is cross-checked against `Z`
//!   at build time.
//! * **Rate**: BG1 x {1/3, 1/2, 2/3, 5/6}; BG2 x {1/3, 1/2, 2/3}.
//! * **Modulation**: QPSK / 16-QAM / 64-QAM / 256-QAM (`Q_m` ∈ {2, 4, 6, 8}).
//!
//! The message length is the largest payload realising exactly the requested
//! `Z` ([`max_payload_for_lifting`]). The codeword length is
//! `E = ⌊k·den/(num·Q_m)⌋·Q_m` for rate `num/den`, the floor form of the
//! §5.4.2.1 bit-selection formula, so `E` is a multiple of `Q_m` as the
//! §5.4.2.2 interleaver requires. When `k·den/num` is not a `Q_m`-multiple
//! integer the realized rate `k/E` exceeds the requested one.

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

/// A 5G NR LDPC base graph (`@/citation/ThreeGpp2017` Section 5.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseGraph {
    /// Base graph 1: 46x68, K_b = 22.
    Bg1,
    /// Base graph 2: 42x52, K_b = 10.
    Bg2,
}

impl BaseGraph {
    /// The base-graph number (`1` or `2`) the `gf2-coding` `nr_5g`
    /// constructors take.
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
/// [`build`](Builder::build) rejects BG2 with [`R5_6`](Nr5gRate::R5_6).
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
    fn num_den(self) -> (usize, usize) {
        match self {
            Nr5gRate::R1_3 => (1, 3),
            Nr5gRate::R1_2 => (1, 2),
            Nr5gRate::R2_3 => (2, 3),
            Nr5gRate::R5_6 => (5, 6),
        }
    }

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
/// [`build`](Builder::build) validates both fields and returns
/// [`BuildError::InvalidNr5gParams`] for an invalid one.
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
    /// [`build`](Builder::build) requires `max_iterations >= 1`, a finite
    /// `alpha` in `(0.0, 1.0]` for `NormalizedMinSum(alpha)`, and a finite
    /// `beta >= 0.0` for `OffsetMinSum(beta)`.
    #[must_use]
    pub fn new(algorithm: DecoderAlgorithm, max_iterations: usize) -> Self {
        Self {
            algorithm,
            max_iterations,
        }
    }

    /// Normalized min-sum with `alpha` = 0.75.
    #[must_use]
    pub fn normalized_min_sum(max_iterations: usize) -> Self {
        Self::new(DecoderAlgorithm::NormalizedMinSum(0.75), max_iterations)
    }

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

/// The channel [`build`](Builder::build) inserts between the forward
/// (transmit) and inverse (receive) halves of the chain.
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
    #[must_use]
    pub fn awgn(es_n0_db: f32) -> Self {
        Channel::Awgn { es_n0_db }
    }

    fn into_stage(self, bits_per_symbol: usize) -> Box<dyn crate::stage::AnyStage> {
        match self {
            Channel::Awgn { es_n0_db } => erase(Awgn::new(es_n0_db, bits_per_symbol)),
        }
    }

    /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`) the
    /// soft demapper assumes for this channel.
    fn demap_noise_var(self) -> f32 {
        match self {
            Channel::Awgn { es_n0_db } => crate::channels::es_n0_db_to_n0(es_n0_db),
        }
    }

    /// Validates this channel's parameters, returning the demapper `N0` it
    /// implies on success.
    ///
    /// The rejected inputs are those that panic
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

/// Typestate marker: only [`Builder::base_graph`] is available.
#[derive(Debug)]
pub enum NeedsBaseGraph {}

/// Typestate marker: [`Builder::lifting_size`] and the optional
/// [`Builder::lifting_set`] are available.
#[derive(Debug)]
pub enum NeedsLifting {}

/// Typestate marker: only [`Builder::rate`] is available.
#[derive(Debug)]
pub enum NeedsRate {}

/// Typestate marker: only [`Builder::decoder`] is available.
#[derive(Debug)]
pub enum NeedsDecoder {}

/// Typestate marker: only [`Builder::demap`] is available.
#[derive(Debug)]
pub enum NeedsDemap {}

/// Typestate marker: only [`Builder::channel`] is available.
#[derive(Debug)]
pub enum NeedsChannel {}

/// Typestate marker: the optional setters and [`Builder::build`] are
/// available.
#[derive(Debug)]
pub enum Ready {}

/// A typestate fluent builder for the 5G NR LDPC BICM pipeline.
///
/// `State` is one of [`NeedsBaseGraph`], [`NeedsLifting`], [`NeedsRate`],
/// [`NeedsDecoder`], [`NeedsDemap`], [`NeedsChannel`], [`Ready`]. Construct
/// one via [`Pipeline::nr_5g`](crate::Pipeline::nr_5g); see the
/// [module docs](self) for the call sequence.
pub struct Builder<State> {
    // The `cfg_` prefix keeps field names distinct from the setter names, so
    // an out-of-order call reports "no method named `lifting_size`" instead of
    // "private field, not a method".
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
    /// Defaults: `parallelism = 1`, `seed = 0`, no lifting-set index, no
    /// checkpoint directory.
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
    /// Records the expected lifting-set index `i_LS` (0..=7,
    /// `@/citation/ThreeGpp2017` Table 5.3.2-1).
    ///
    /// [`build`](Builder::build) returns [`BuildError::InvalidNr5gParams`] when
    /// the index differs from the [`lifting_set_index`] of the chosen
    /// [`lifting_size`](Builder::lifting_size).
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
    /// [`build`](Builder::build) requires a Table 5.3.2-1 lifting size.
    pub fn lifting_size(self, z: usize) -> Builder<NeedsRate> {
        self.with_state(|b| b.cfg_lifting_size = Some(z))
    }
}

impl Builder<NeedsRate> {
    /// Selects the nominal code rate, advancing to [`NeedsDecoder`].
    /// [`build`](Builder::build) rejects BG2 with rate 5/6.
    pub fn rate(self, rate: Nr5gRate) -> Builder<NeedsDecoder> {
        self.with_state(|b| b.cfg_rate = Some(rate))
    }
}

impl Builder<NeedsDecoder> {
    /// Sets the LDPC BP decoder configuration, advancing to [`NeedsDemap`].
    pub fn decoder(self, decoder: Nr5gDecoderConfig) -> Builder<NeedsDemap> {
        self.with_state(|b| b.cfg_decoder = Some(decoder))
    }
}

impl Builder<NeedsDemap> {
    /// Sets the modulation and soft-demap method, advancing to
    /// [`NeedsChannel`].
    ///
    /// The modulation order `Q_m` parameterises the §5.4.2.2 interleaver, the
    /// Gray-QAM mapper, and the soft demapper.
    pub fn demap(self, modulation: NrModulation, method: DemapMethod) -> Builder<NeedsChannel> {
        self.with_state(|b| {
            b.cfg_modulation = Some(modulation);
            b.cfg_demap = Some(method);
        })
    }
}

impl Builder<NeedsChannel> {
    /// Sets the channel, advancing to [`Ready`].
    pub fn channel(self, channel: Channel) -> Builder<Ready> {
        self.with_state(|b| b.cfg_channel = Some(channel))
    }
}

impl Builder<Ready> {
    /// Sets the number of parallel workers carried on the built pipeline's
    /// [`PipelineConfig`].
    #[must_use]
    pub fn parallelism(mut self, parallelism: NonZeroUsize) -> Self {
        self.cfg_parallelism = parallelism;
        self
    }

    /// Sets the base ChaCha20 seed carried on the built pipeline's
    /// [`PipelineConfig`].
    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.cfg_seed = seed;
        self
    }

    /// Sets the optional per-SNR checkpoint directory carried on the built
    /// pipeline's [`PipelineConfig`].
    #[must_use]
    pub fn checkpoint_dir(mut self, checkpoint_dir: Option<PathBuf>) -> Self {
        self.cfg_checkpoint_dir = checkpoint_dir;
        self
    }

    /// Validates the parameters and compiles the 5G NR chain into a
    /// [`Pipeline`].
    ///
    /// The chain is [`Nr5gEncode`] → [`Nr5gBitInterleave`] → [`NrGrayQamMap`]
    /// → [`Awgn`] → [`NrGrayQamDemap`] → [`Nr5gLlrDeinterleave`] →
    /// [`Nr5gDecode`]. The demapper's noise variance is derived from the
    /// channel's Es/N0. The code dimensions are
    /// `target_k = `[`max_payload_for_lifting`]`(BG, Z)` and
    /// `target_n = ⌊target_k * den / (num * Q_m)⌋ * Q_m` for rate `num/den`
    /// (see the [module docs](self)).
    ///
    /// # Errors
    ///
    /// * [`BuildError::InvalidNr5gParams`] if `Z` is not a Table 5.3.2-1
    ///   lifting size; if a [`lifting_set`](Builder::lifting_set) index is
    ///   inconsistent with `Z`; if the rate is 5/6 on BG2; if the decoder
    ///   configuration is invalid (zero iteration cap, out-of-range min-sum
    ///   scale); if the constructed code does not realise the requested `Z`;
    ///   or if `E` is not a multiple of `Q_m` or does not exceed `k`.
    /// * [`BuildError::InvalidChannel`] if the AWGN Es/N0 is non-finite, or so
    ///   large that the derived demapper noise variance underflows to a
    ///   non-positive value.
    ///
    /// # Complexity
    ///
    /// Dominated by the rate-matched mother-code construction
    /// ([`QuasiCyclicLdpc::nr_5g_rate_matched`]: RREF on the `N_b * Z`-column
    /// parity-check matrix).
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
        // The required fields are `Some` by typestate.
        let base_graph = self.cfg_base_graph.expect("base graph set before Ready");
        let z = self
            .cfg_lifting_size
            .expect("lifting size set before Ready");
        let rate = self.cfg_rate.expect("rate set before Ready");
        let decoder = self.cfg_decoder.expect("decoder set before Ready");
        let modulation = self.cfg_modulation.expect("modulation set before Ready");
        let demap = self.cfg_demap.expect("demap method set before Ready");
        let channel = self.cfg_channel.expect("channel set before Ready");

        // An out-of-`u16` Z maps to 0, which is not a lifting size.
        let actual_i_ls = lifting_set_index(u16::try_from(z).unwrap_or(0)).ok_or_else(|| {
            BuildError::InvalidNr5gParams {
                reason: format!(
                    "Z = {z} is not a valid 5G NR lifting size \
                     (TS 38.212 Table 5.3.2-1)"
                ),
            }
        })?;

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

        // Guards the `DecoderConfig::new` panic in the decode stage.
        decoder.validate()?;

        // E = floor(k*den / (num*Q_m)) * Q_m: the floor only shrinks E, so the
        // realized rate k/E is >= num/den and E stays within the mother code's
        // transmission budget.
        let target_k = max_payload_for_lifting(base_graph.number(), z);
        let (num, den) = rate.num_den();
        let q_m = modulation.bits_per_symbol();
        let target_n = (target_k * den) / (num * q_m) * q_m;

        // Holds by the derivation above; checked because the §5.4.2.2
        // interleaver asserts it.
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
        // The rate-matched constructor asserts target_n > target_k.
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

        let demap_noise_var = channel.validate()?;
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

        chain.with_config(config).build()
    }
}

impl<State> Builder<State> {
    /// Re-tags the builder into the `Next` typestate after applying `mutate`.
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
    /// See the [module docs](crate::presets::nr_5g) for the call sequence.
    ///
    /// # Examples
    ///
    /// The BG1 / `Z` = 384 / rate-1/2 chain driven through
    /// [`TopologyExecutor::run`](crate::TopologyExecutor::run): one QPSK frame
    /// at 6 dB Es/N0 decodes back to the transmitted message.
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

    fn nms25() -> Nr5gDecoderConfig {
        Nr5gDecoderConfig::normalized_min_sum(25)
    }

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

    /// In this BG2 band the full payload would select Z = 72 instead of 52.
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

    #[test]
    fn test_nr5g_modulation_bits_per_symbol_all_variants() {
        assert_eq!(NrModulation::Qpsk.bits_per_symbol(), 2);
        assert_eq!(NrModulation::Qam16.bits_per_symbol(), 4);
        assert_eq!(NrModulation::Qam64.bits_per_symbol(), 6);
        assert_eq!(NrModulation::Qam256.bits_per_symbol(), 8);
    }

    #[test]
    fn test_channel_awgn_and_demap_noise_var() {
        let ch = Channel::awgn(5.0);
        assert_eq!(ch, Channel::Awgn { es_n0_db: 5.0 });
        let n0 = ch.demap_noise_var();
        assert!(
            n0.is_finite() && n0 > 0.0,
            "N0 must be finite positive: {n0}"
        );
    }

    #[test]
    fn test_build_rejects_offset_min_sum_nan() {
        let result = Pipeline::nr_5g()
            .base_graph(BaseGraph::Bg2)
            .lifting_size(52)
            .rate(Nr5gRate::R1_3)
            .decoder(Nr5gDecoderConfig::new(
                DecoderAlgorithm::OffsetMinSum(f32::NAN),
                25,
            ))
            .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
            .channel(Channel::awgn(3.0))
            .build();
        assert!(
            matches!(result, Err(BuildError::InvalidNr5gParams { .. })),
            "OffsetMinSum(NaN) must produce InvalidNr5gParams"
        );
    }
}
