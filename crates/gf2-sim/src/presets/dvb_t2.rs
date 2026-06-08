//! DVB-T2 BICM preset: a typestate fluent builder over the graph API.
//!
//! Owned by `81d05bab` (design doc §9, "Typestate builder (presets)"). This
//! module is a **thin wrapper** over the already-landed graph
//! [`Chain`](crate::graph::Chain): it reuses the canonical BICM stage order from
//! [`dvb_t2_bicm_stages`](crate::stages::dvb_t2_bicm_stages), inserts the AWGN
//! [`Awgn`](crate::channels::Awgn) channel between the forward and inverse
//! halves, connects the seven stages consecutively, and calls
//! [`Chain::build`](crate::graph::Chain::build). None of the BCH / LDPC / QAM /
//! interleaver math, the channel noise model, or the chain-wiring logic is
//! re-implemented here.
//!
//! # Typestate stage ordering
//!
//! The builder is generic over a zero-sized marker type tracking how far the
//! chain has been specified. The required methods must be called in order —
//! [`modcod`](Builder::modcod) → [`decoder`](Builder::decoder) →
//! [`demap`](Builder::demap) → [`channel`](Builder::channel) — and each consumes
//! `self`, returning a `Builder` in the next state. Calling them out of order
//! (e.g. [`decoder`](Builder::decoder) before [`modcod`](Builder::modcod)) is a
//! **compile-time** error because the method only exists on the predecessor
//! state. Only a [`Builder<Ready>`] exposes [`build`](Builder::build).
//!
//! The non-state-advancing setters
//! ([`parallelism`](Builder::parallelism), [`seed`](Builder::seed),
//! [`checkpoint_dir`](Builder::checkpoint_dir)) are available on the
//! [`Ready`] state and may be chained in any order before
//! [`build`](Builder::build).
//!
//! # Entry point
//!
//! [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2) returns a fresh
//! `Builder<NeedsModcod>`. (The design doc spells the entry `Pipeline::builder()`;
//! `dvb_t2()` is the chosen, more explicit name for the DVB-T2 preset.)
//!
//! # Examples
//!
//! Build the full DVB-T2 BICM pipeline for the Normal-frame rate-1/2 16-QAM
//! MODCOD, then drive a noiseless BBFRAME through it:
//!
//! ```
//! use std::num::NonZeroUsize;
//! use gf2_sim::Pipeline;
//! use gf2_sim::presets::dvb_t2::{Channel, Modcod};
//! use gf2_coding::CodeRate;
//! use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
//! use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
//! use gf2_coding::modem::DemapMethod;
//!
//! let pipeline = Pipeline::dvb_t2()
//!     .modcod(Modcod::Normal {
//!         rate: CodeRate::Rate1_2,
//!         modulation: DvbT2Modulation::Qam16,
//!     })
//!     .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
//!     .demap(DemapMethod::ExactLogMap)
//!     .channel(Channel::awgn(6.0))
//!     .parallelism(NonZeroUsize::new(4).unwrap())
//!     .seed(0xC0DE_F00D)
//!     .build()
//!     .expect("the six in-scope MODCODs all build");
//!
//! // Forward (3) + channel (1) + inverse (3) = seven stages.
//! assert_eq!(pipeline.stage_count(), 7);
//! assert_eq!(pipeline.config().seed, 0xC0DE_F00D);
//! ```

use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::DecoderConfig;
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use crate::channels::Awgn;
use crate::error::{BuildError, Modulation, NrRate};
use crate::graph::Chain;
use crate::pipeline::Pipeline;
use crate::stage::erase;
use crate::stages::dvb_t2_bicm_stages;
use crate::PipelineConfig;

/// A DVB-T2 MODCOD: a `(code-rate, modulation)` pair on the Normal FECFRAME.
///
/// The six in-scope MODCODs are `rate ∈ {1/2, 2/3, 3/4}` crossed with
/// `modulation ∈ {16-QAM, 64-QAM}` (design doc §9). [`Modcod::validate`] rejects
/// any other combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modcod {
    /// A Normal-frame (n = 64800) MODCOD.
    Normal {
        /// The LDPC code rate.
        rate: CodeRate,
        /// The QAM modulation order.
        modulation: DvbT2Modulation,
    },
}

impl Modcod {
    /// Returns the `(rate, modulation)` of this MODCOD.
    fn parts(self) -> (CodeRate, DvbT2Modulation) {
        match self {
            Modcod::Normal { rate, modulation } => (rate, modulation),
        }
    }

    /// Validates that this MODCOD is one of the six in-scope DVB-T2 combinations.
    ///
    /// The in-scope set is `rate ∈ {Rate1_2, Rate2_3, Rate3_4}` crossed with
    /// `modulation ∈ {Qam16, Qam64}` (design doc §9).
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::InvalidModcod`] (carrying the offending rate and
    /// modulation) when the `(rate, modulation)` pair is outside that set — e.g.
    /// a DVB-T2 rate such as `Rate3_5` that this preset does not wire, or the
    /// QPSK modulation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::presets::dvb_t2::Modcod;
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    ///
    /// let ok = Modcod::Normal { rate: CodeRate::Rate2_3, modulation: DvbT2Modulation::Qam64 };
    /// assert!(ok.validate().is_ok());
    ///
    /// let bad = Modcod::Normal { rate: CodeRate::Rate3_5, modulation: DvbT2Modulation::Qam16 };
    /// assert!(bad.validate().is_err());
    /// ```
    pub fn validate(self) -> Result<(), BuildError> {
        let (rate, modulation) = self.parts();
        let rate_ok = matches!(
            rate,
            CodeRate::Rate1_2 | CodeRate::Rate2_3 | CodeRate::Rate3_4
        );
        let modulation_ok = matches!(modulation, DvbT2Modulation::Qam16 | DvbT2Modulation::Qam64);
        if rate_ok && modulation_ok {
            Ok(())
        } else {
            Err(BuildError::InvalidModcod {
                rate: map_rate(rate),
                modulation: map_modulation(modulation),
            })
        }
    }
}

/// Maps a `gf2-coding` [`CodeRate`] onto the [`NrRate`] selector carried by
/// [`BuildError::InvalidModcod`].
///
/// The three in-scope rates map onto the matching [`NrRate`] variant; any other
/// rate (which only ever reaches here on the invalid-MODCOD error path) is
/// reported as the closest in-scope rate `R1_2` purely so the error carries a
/// concrete value — the rate string is informational, and the modulation /
/// rate pair as a whole is what was rejected.
fn map_rate(rate: CodeRate) -> NrRate {
    match rate {
        CodeRate::Rate1_2 => NrRate::R1_2,
        CodeRate::Rate2_3 => NrRate::R2_3,
        CodeRate::Rate3_4 => NrRate::R3_4,
        _ => NrRate::R1_2,
    }
}

/// Maps a `gf2-coding` [`DvbT2Modulation`] onto the [`Modulation`] selector
/// carried by [`BuildError::InvalidModcod`].
///
/// 16-QAM and 64-QAM map onto the matching variant; QPSK (the only other
/// modulation, reachable solely on the invalid-MODCOD error path) is reported as
/// `Qam16` so the error carries a concrete value.
fn map_modulation(modulation: DvbT2Modulation) -> Modulation {
    match modulation {
        DvbT2Modulation::Qam16 => Modulation::Qam16,
        DvbT2Modulation::Qam64 => Modulation::Qam64,
        DvbT2Modulation::Qpsk => Modulation::Qam16,
    }
}

/// A channel selector for the DVB-T2 preset.
///
/// Constructs the channel [`Stage`](crate::Stage) inserted between the forward
/// (transmit) and inverse (receive) halves of the BICM chain at
/// [`build`](Builder::build) time.
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
    /// use gf2_sim::presets::dvb_t2::Channel;
    ///
    /// let ch = Channel::awgn(6.25);
    /// assert_eq!(ch, Channel::Awgn { es_n0_db: 6.25 });
    /// ```
    #[must_use]
    pub fn awgn(es_n0_db: f32) -> Self {
        Channel::Awgn { es_n0_db }
    }

    /// Materialises the channel into an erased [`Awgn`] stage for `bits_per_symbol`.
    fn into_stage(self, bits_per_symbol: usize) -> Box<dyn crate::stage::AnyStage> {
        match self {
            Channel::Awgn { es_n0_db } => erase(Awgn::new(es_n0_db, bits_per_symbol)),
        }
    }
}

// ===========================================================================
// Typestate marker types
// ===========================================================================

/// Typestate marker: the MODCOD has not been selected yet.
///
/// The initial state of a fresh [`Builder`]; only [`Builder::modcod`] is
/// available.
#[derive(Debug)]
pub enum NeedsModcod {}

/// Typestate marker: the MODCOD is set; the decoder is next.
///
/// Only [`Builder::decoder`] is available.
#[derive(Debug)]
pub enum NeedsDecoder {}

/// Typestate marker: the decoder is set; the demap method is next.
///
/// Only [`Builder::demap`] is available.
#[derive(Debug)]
pub enum NeedsDemap {}

/// Typestate marker: the demap method is set; the channel is next.
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

/// A typestate fluent builder for the DVB-T2 BICM pipeline.
///
/// The generic `State` parameter (one of [`NeedsModcod`], [`NeedsDecoder`],
/// [`NeedsDemap`], [`NeedsChannel`], [`Ready`]) tracks how far the chain has
/// been specified, so the compiler enforces the stage order. Construct one via
/// [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2); see the [module docs](self)
/// for the full call sequence.
///
/// The required fields are stored as `Option`s populated in typestate order;
/// each is guaranteed `Some` by the time [`Ready`] is reached, so
/// [`build`](Builder::build) unwraps them with an internal invariant message.
pub struct Builder<State> {
    // The storage fields are deliberately NOT named `modcod` / `decoder` /
    // `demap` / `channel` / `seed` / `parallelism` / `checkpoint_dir`: a private
    // field that shares a name with a public method makes the out-of-order
    // compile error ("private field, not a method") less clear about the
    // typestate. Prefixing with `cfg_` keeps the compile-fail diagnostic a clean
    // "no method named `decoder` ... for Builder<NeedsModcod>".
    cfg_modcod: Option<Modcod>,
    cfg_decoder: Option<DecoderConfig>,
    cfg_demap: Option<DemapMethod>,
    cfg_channel: Option<Channel>,
    cfg_parallelism: NonZeroUsize,
    cfg_seed: u64,
    cfg_checkpoint_dir: Option<PathBuf>,
    _state: PhantomData<fn() -> State>,
}

impl Builder<NeedsModcod> {
    /// Creates a fresh builder in the [`NeedsModcod`] state.
    ///
    /// The default optional settings are: `parallelism = 1`, `seed = 0`, and no
    /// checkpoint directory. Prefer the [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2)
    /// entry point.
    pub(crate) fn new() -> Self {
        Self {
            cfg_modcod: None,
            cfg_decoder: None,
            cfg_demap: None,
            cfg_channel: None,
            cfg_parallelism: NonZeroUsize::new(1).expect("1 is non-zero"),
            cfg_seed: 0,
            cfg_checkpoint_dir: None,
            _state: PhantomData,
        }
    }

    /// Selects the DVB-T2 MODCOD, advancing to [`NeedsDecoder`].
    ///
    /// # Arguments
    ///
    /// * `modcod` — the `(rate, modulation)` MODCOD; validated at
    ///   [`build`](Builder::build) time via [`Modcod::validate`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::dvb_t2::Modcod;
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    ///
    /// let _b = Pipeline::dvb_t2().modcod(Modcod::Normal {
    ///     rate: CodeRate::Rate1_2,
    ///     modulation: DvbT2Modulation::Qam16,
    /// });
    /// ```
    pub fn modcod(self, modcod: Modcod) -> Builder<NeedsDecoder> {
        self.with_state(|b| b.cfg_modcod = Some(modcod))
    }
}

impl Builder<NeedsDecoder> {
    /// Sets the LDPC belief-propagation decoder configuration, advancing to
    /// [`NeedsDemap`].
    ///
    /// # Arguments
    ///
    /// * `decoder` — the decoder configuration applied to the shared codec.
    pub fn decoder(self, decoder: DecoderConfig) -> Builder<NeedsDemap> {
        self.with_state(|b| b.cfg_decoder = Some(decoder))
    }
}

impl Builder<NeedsDemap> {
    /// Sets the soft-demap method, advancing to [`NeedsChannel`].
    ///
    /// # Arguments
    ///
    /// * `demap` — exact log-MAP or max-log demapping.
    pub fn demap(self, demap: DemapMethod) -> Builder<NeedsChannel> {
        self.with_state(|b| b.cfg_demap = Some(demap))
    }
}

impl Builder<NeedsChannel> {
    /// Sets the channel, advancing to [`Ready`].
    ///
    /// # Arguments
    ///
    /// * `channel` — the channel inserted between the forward and inverse halves.
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

    /// Validates the MODCOD and compiles the BICM chain into a [`Pipeline`].
    ///
    /// Assembles the seven-stage DVB-T2 BICM chain — the three forward stages
    /// and three inverse stages from
    /// [`dvb_t2_bicm_stages`](crate::stages::dvb_t2_bicm_stages) with the channel
    /// stage spliced between them — into a [`Chain`](crate::graph::Chain),
    /// connects them consecutively, and returns
    /// [`Chain::build`](crate::graph::Chain::build)'s [`Pipeline`]. The built
    /// pipeline carries a [`PipelineConfig`] holding the configured `seed`,
    /// `parallelism`, and `checkpoint_dir`.
    ///
    /// # Errors
    ///
    /// * [`BuildError::InvalidModcod`] if the `(rate, modulation)` pair is not
    ///   one of the six in-scope DVB-T2 MODCODs (see [`Modcod::validate`]).
    ///
    /// The chain wiring itself (a fixed seven-stage linear DAG with
    /// type-compatible consecutive edges) is well-formed by construction, so
    /// `build()` never surfaces a topology [`BuildError`] for this preset.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::dvb_t2::{Channel, Modcod};
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// let pipeline = Pipeline::dvb_t2()
    ///     .modcod(Modcod::Normal {
    ///         rate: CodeRate::Rate3_4,
    ///         modulation: DvbT2Modulation::Qam64,
    ///     })
    ///     .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
    ///     .demap(DemapMethod::MaxLog)
    ///     .channel(Channel::awgn(10.0))
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(pipeline.stage_count(), 7);
    /// ```
    pub fn build(self) -> Result<Pipeline, BuildError> {
        let modcod = self.cfg_modcod.expect("modcod set before Ready");
        modcod.validate()?;
        let (rate, modulation) = modcod.parts();
        let decoder = self.cfg_decoder.expect("decoder set before Ready");
        let demap = self.cfg_demap.expect("demap set before Ready");
        let channel = self.cfg_channel.expect("channel set before Ready");

        // SSOT BICM stage order: forward = [encode, interleave, map],
        // inverse = [demap, deinterleave, decode]. The channel slots between.
        let stages = dvb_t2_bicm_stages(rate, modulation, decoder, demap);
        let channel_stage = channel.into_stage(modulation.bits_per_cell());

        let mut chain = Chain::new();
        let mut ids = Vec::with_capacity(7);
        for stage in stages.forward {
            ids.push(chain.add(stage));
        }
        ids.push(chain.add(channel_stage));
        for stage in stages.inverse {
            ids.push(chain.add(stage));
        }

        for pair in ids.windows(2) {
            chain
                .connect(pair[0], pair[1])
                .expect("consecutive DVB-T2 BICM stages are type-compatible");
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
            strict_gpu: false,
        };

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
            cfg_modcod: self.cfg_modcod,
            cfg_decoder: self.cfg_decoder,
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
    /// Starts a DVB-T2 BICM preset builder in the [`NeedsModcod`] state.
    ///
    /// This is the entry point for the typestate fluent builder; the required
    /// stages must then be specified in order
    /// ([`modcod`](Builder::modcod) → [`decoder`](Builder::decoder) →
    /// [`demap`](Builder::demap) → [`channel`](Builder::channel)), after which
    /// the optional setters and [`build`](Builder::build) become available. See
    /// the [module docs](crate::presets::dvb_t2).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::dvb_t2::{Channel, Modcod};
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// let pipeline = Pipeline::dvb_t2()
    ///     .modcod(Modcod::Normal {
    ///         rate: CodeRate::Rate1_2,
    ///         modulation: DvbT2Modulation::Qam16,
    ///     })
    ///     .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
    ///     .demap(DemapMethod::ExactLogMap)
    ///     .channel(Channel::awgn(6.0))
    ///     .build()
    ///     .unwrap();
    /// assert_eq!(pipeline.stage_count(), 7);
    /// ```
    #[must_use]
    pub fn dvb_t2() -> Builder<NeedsModcod> {
        Builder::<NeedsModcod>::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::ldpc::DecoderAlgorithm;

    fn sp() -> DecoderConfig {
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
    }

    #[test]
    fn test_validate_accepts_six_in_scope_modcods() {
        for rate in [CodeRate::Rate1_2, CodeRate::Rate2_3, CodeRate::Rate3_4] {
            for modulation in [DvbT2Modulation::Qam16, DvbT2Modulation::Qam64] {
                assert!(
                    Modcod::Normal { rate, modulation }.validate().is_ok(),
                    "{rate:?}/{modulation:?} must be in scope"
                );
            }
        }
    }

    #[test]
    fn test_validate_rejects_out_of_scope_rate() {
        let bad = Modcod::Normal {
            rate: CodeRate::Rate3_5,
            modulation: DvbT2Modulation::Qam16,
        };
        assert!(matches!(
            bad.validate(),
            Err(BuildError::InvalidModcod { .. })
        ));
    }

    #[test]
    fn test_validate_rejects_qpsk() {
        let bad = Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qpsk,
        };
        assert!(matches!(
            bad.validate(),
            Err(BuildError::InvalidModcod { .. })
        ));
    }

    #[test]
    fn test_build_produces_seven_stage_pipeline() {
        let pipeline = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(sp())
            .demap(DemapMethod::ExactLogMap)
            .channel(Channel::awgn(6.0))
            .build()
            .expect("in-scope MODCOD builds");
        assert_eq!(pipeline.stage_count(), 7);
        assert_eq!(pipeline.edges().len(), 6);
    }

    #[test]
    fn test_build_threads_optional_config() {
        let dir = PathBuf::from("checkpoints");
        let pipeline = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate2_3,
                modulation: DvbT2Modulation::Qam64,
            })
            .decoder(sp())
            .demap(DemapMethod::MaxLog)
            .channel(Channel::awgn(8.0))
            .parallelism(NonZeroUsize::new(8).unwrap())
            .seed(0xABCD)
            .checkpoint_dir(Some(dir.clone()))
            .build()
            .expect("in-scope MODCOD builds");
        assert_eq!(pipeline.config().seed, 0xABCD);
        assert_eq!(pipeline.config().parallelism.get(), 8);
        assert_eq!(
            pipeline.config().checkpoint_dir.as_deref(),
            Some(dir.as_path())
        );
    }

    #[test]
    fn test_build_rejects_invalid_modcod_at_build_time() {
        let result = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate4_5,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(sp())
            .demap(DemapMethod::ExactLogMap)
            .channel(Channel::awgn(6.0))
            .build();
        assert!(matches!(result, Err(BuildError::InvalidModcod { .. })));
    }
}
