//! DVB-T2 BICM preset: a typestate fluent builder over the graph API.
//!
//! Owned by `81d05bab` (design doc §9, "Typestate builder (presets)"). This
//! module is a **thin wrapper** over the already-landed graph
//! [`Chain`]: it reuses the canonical BICM stage order from
//! [`dvb_t2_bicm_stages`], inserts the AWGN
//! [`Awgn`] channel between the forward and inverse
//! halves, connects the seven stages consecutively, and calls
//! [`Chain::build`]. None of the BCH / LDPC / QAM /
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
use crate::error::BuildError;
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
    /// Returns [`BuildError::InvalidModcod`] when the `(rate, modulation)` pair
    /// is outside that set — e.g. a DVB-T2 rate such as `Rate3_5` that this
    /// preset does not wire, or the QPSK modulation. The error carries
    /// human-readable strings of the **actual** offending rate and modulation
    /// that were requested (e.g. `rate = "5/6"`, `modulation = "QPSK"`), so it
    /// reports exactly what was rejected rather than a lossy fold onto the
    /// in-scope set.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::presets::dvb_t2::Modcod;
    /// use gf2_sim::error::BuildError;
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    ///
    /// let ok = Modcod::Normal { rate: CodeRate::Rate2_3, modulation: DvbT2Modulation::Qam64 };
    /// assert!(ok.validate().is_ok());
    ///
    /// // The error reports the TRUE offending rate, not an in-scope substitute.
    /// let bad = Modcod::Normal { rate: CodeRate::Rate5_6, modulation: DvbT2Modulation::Qam16 };
    /// match bad.validate() {
    ///     Err(BuildError::InvalidModcod { rate, .. }) => assert_eq!(rate, "5/6"),
    ///     other => panic!("expected InvalidModcod, got {other:?}"),
    /// }
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
                rate: rate_label(rate).to_string(),
                modulation: modulation_label(modulation).to_string(),
            })
        }
    }
}

/// Human-readable label for a DVB-T2 [`CodeRate`] (e.g. `"1/2"`, `"5/6"`).
///
/// Renders the rate as its `numerator/denominator` ratio so an out-of-scope
/// rejected rate is reported losslessly in [`BuildError::InvalidModcod`].
fn rate_label(rate: CodeRate) -> &'static str {
    match rate {
        CodeRate::Rate1_2 => "1/2",
        CodeRate::Rate3_5 => "3/5",
        CodeRate::Rate2_3 => "2/3",
        CodeRate::Rate3_4 => "3/4",
        CodeRate::Rate4_5 => "4/5",
        CodeRate::Rate5_6 => "5/6",
    }
}

/// Human-readable label for a [`DvbT2Modulation`] (e.g. `"16-QAM"`, `"QPSK"`).
///
/// Reports the modulation losslessly in [`BuildError::InvalidModcod`].
fn modulation_label(modulation: DvbT2Modulation) -> &'static str {
    match modulation {
        DvbT2Modulation::Qpsk => "QPSK",
        DvbT2Modulation::Qam16 => "16-QAM",
        DvbT2Modulation::Qam64 => "64-QAM",
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

    /// The per-symbol total complex AWGN noise variance (`N0 = 2 sigma^2`) the
    /// soft demapper must assume to be physically consistent with this channel.
    ///
    /// Computed in `f64` and rounded once — `N0 = (2 * sigma_sq) as f32` with
    /// `sigma_sq = 1 / (2 * 10^(Es/N0 / 10))` — which is **bit-identical** to
    /// the SSOT frame kernel's derivation
    /// ([`DvbT2BicmFrameSim::noise_var`](crate::frame_sim::DvbT2BicmFrameSim::noise_var))
    /// for any `f32`-representable Es/N0. The stage-driven executor's
    /// chain-vs-SSOT byte-identity (`de160fc5`, design doc §11) requires this:
    /// the earlier `2.0 * sigma * sigma` form (squaring the already-rounded
    /// `f32` sigma the [`Awgn`] stage injects) rounds twice and can differ from
    /// the frame kernel's `N0` by an ULP, which perturbs every demapped LLR.
    /// The injected noise is unchanged — `sigma` itself is the same SSOT
    /// [`es_n0_db_to_sigma`](crate::channels::es_n0_db_to_sigma) value either
    /// way.
    ///
    /// May be non-finite or zero if `es_n0_db` is non-finite or so large that
    /// `sigma^2` underflows; [`Channel::validate`] rejects those cases before
    /// this value is fed to the demapper.
    fn demap_noise_var(self) -> f32 {
        match self {
            Channel::Awgn { es_n0_db } => crate::channels::es_n0_db_to_n0(es_n0_db),
        }
    }

    /// Validates this channel's parameters, returning the demapper `N0` it
    /// implies on success.
    ///
    /// Rejects a non-finite Es/N0 (`NaN`/`±inf`) and any parameter whose derived
    /// demapper noise variance (`N0 = 2 sigma^2`) is not finite and strictly
    /// positive — i.e. exactly the inputs that would otherwise panic
    /// [`GrayQamDemap::with_noise_var`](crate::stages::GrayQamDemap::with_noise_var)
    /// (and the [`Awgn`] stage assumes a finite `sigma`). On success the returned
    /// `f32` is the validated demapper `N0`, ready to pass to the factory.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::InvalidChannel`] (with a human-readable reason) when
    /// the Es/N0 is non-finite or the derived `N0` is non-finite or `<= 0`.
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
    cfg_gpu_enabled: bool,
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
            cfg_gpu_enabled: false,
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

    /// Enables (or disables) GPU offload of the heavy device-bound stages.
    /// Non-state-advancing.
    ///
    /// When `true` (with the `hip` feature compiled in), [`build`](Builder::build)
    /// replaces the combined CPU decode stage with the
    /// [`ExecutionClass::GpuOnly`](crate::ExecutionClass) LDPC BP decode stage
    /// (`gpu::ldpc_bp::GpuLdpcBp`, its `CpuLdpcBp` fallback registered on the
    /// pipeline for §8 OOM substitution) followed by the CPU BCH outer-decode
    /// tail ([`DvbT2BchTail`](crate::stages::DvbT2BchTail)) — an eight-stage
    /// chain — and records `gpu_enabled` on the [`PipelineConfig`].
    /// [`Pipeline::run`](crate::Pipeline::run) then routes by execution class
    /// and drives the hybrid CPU+GPU scheduler (Phase C `75c22fa8`): each rayon
    /// worker prepares the next batch on the CPU while its owned HIP stream
    /// decodes the current batch on the device. When `false` (the default),
    /// every stage runs on the CPU via the within-SNR frame-parallel path.
    /// The OOM CPU-fallback substitution policy is active: the executor's
    /// [`dispatch_with_fallback`](crate::executor::failure::dispatch_with_fallback)
    /// (issue `42eac5cc`) intercepts every GPU stage error — an OOM with
    /// `strict_gpu` unset triggers a `tracing::warn!` and substitutes the
    /// registered `CpuLdpcBp` fallback on the same input.
    ///
    /// # Feature gating
    ///
    /// GPU offload requires the `hip` Cargo feature. Built **without** `hip`,
    /// setting `with_gpu(true)` is accepted but degrades gracefully: the chain
    /// stays all-CPU (seven stages), the run emits a one-shot `tracing::warn!`,
    /// and the CPU path executes (there is no device backend to dispatch to).
    /// The flag is still recorded on the config so a hip-enabled build of the
    /// same pipeline honours it.
    ///
    /// # Arguments
    ///
    /// * `enabled` — whether to offload the GPU-bound stages to the HIP device.
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
    ///     .demap(DemapMethod::MaxLog)
    ///     .channel(Channel::awgn(6.0))
    ///     .with_gpu(true)
    ///     .build()
    ///     .unwrap();
    /// assert!(pipeline.config().gpu_enabled);
    /// ```
    #[must_use]
    pub fn with_gpu(mut self, enabled: bool) -> Self {
        self.cfg_gpu_enabled = enabled;
        self
    }

    /// Validates the MODCOD and compiles the BICM chain into a [`Pipeline`].
    ///
    /// Assembles the seven-stage DVB-T2 BICM chain — the three forward stages
    /// and three inverse stages from
    /// [`dvb_t2_bicm_stages`] with the channel
    /// stage spliced between them — into a [`Chain`],
    /// connects them consecutively, and returns
    /// [`Chain::build`](crate::graph::Chain::build)'s [`Pipeline`]. With
    /// [`with_gpu(true)`](Builder::with_gpu) under the `hip` feature the chain
    /// is instead eight stages: the combined CPU decode is replaced by the
    /// `GpuOnly` LDPC BP decode stage (CPU fallback registered) plus the BCH
    /// outer-decode tail. The built pipeline carries a [`PipelineConfig`]
    /// holding the configured `seed`, `parallelism`, and `checkpoint_dir`.
    ///
    /// The soft demapper's assumed noise variance is derived from the **same**
    /// channel (`N0 = 2 sigma^2` via the SSOT Es/N0→sigma conversion), so the
    /// demapper's `N0` equals the channel's true injected `N0` — the LLR scaling
    /// is physically consistent with the channel rather than a fixed placeholder.
    ///
    /// # Errors
    ///
    /// * [`BuildError::InvalidModcod`] if the `(rate, modulation)` pair is not
    ///   one of the six in-scope DVB-T2 MODCODs (see [`Modcod::validate`]).
    /// * [`BuildError::InvalidChannel`] if a channel parameter is invalid — a
    ///   non-finite (`NaN`/`±inf`) AWGN Es/N0, or an Es/N0 so large that the
    ///   derived demapper noise variance underflows to a non-positive value.
    ///
    /// `build()` validates every input it receives and returns one of the above
    /// typed errors on bad input; it **never panics** on any public input
    /// combination (the typestate guarantees the required setters were called,
    /// and the chain wiring — a fixed seven- or eight-stage linear DAG with
    /// type-compatible consecutive edges, plus at most one registered fallback —
    /// is well-formed by construction, so no topology [`BuildError`] arises for
    /// this preset).
    ///
    /// # Complexity
    ///
    /// O(1) in the number of stages (a fixed seven- or eight-node chain,
    /// topologically sorted in constant time). The wall-clock cost is dominated by the one-off
    /// construction of the [`DvbT2Concat`](gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat)
    /// codec and the LDPC encoder cache inside
    /// [`dvb_t2_bicm_stages`], not by the
    /// graph assembly itself.
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
        // Panic-on-bad-input audit (this method must NEVER panic on any public
        // input combination — it returns `Ok` or a typed `BuildError`):
        //   .modcod(Modcod)      -> validated below -> InvalidModcod.
        //   .channel(Channel)    -> validated below -> InvalidChannel (guards the
        //                           only assert reachable from input, namely
        //                           GrayQamDemap::with_noise_var).
        //   .decoder(DecoderConfig) -> DecoderConfig::new validates its own alpha/
        //                           beta at construction (before reaching the
        //                           builder); set_decoder_config just stores it.
        //   .demap(DemapMethod)  -> plain enum, no invalid value.
        //   .parallelism(NonZeroUsize) -> the type forbids 0; copied verbatim.
        //   .seed(u64) / .checkpoint_dir(Option<PathBuf>) -> any value valid;
        //                           copied verbatim into the config.
        //
        // The required fields are all `Some` by typestate (each was set on the
        // way to `Ready`); the `expect`s document that invariant and are
        // unreachable from any public call sequence.
        let modcod = self.cfg_modcod.expect("modcod set before Ready");
        // Validate the MODCOD up front (rejects out-of-scope rate/modulation
        // before any stage is built) so `dvb_t2_bicm_stages`'s in-crate codec
        // `expect` is never reached on a bad rate.
        modcod.validate()?;
        let (rate, modulation) = modcod.parts();
        let decoder = self.cfg_decoder.expect("decoder set before Ready");
        let demap = self.cfg_demap.expect("demap set before Ready");
        let channel = self.cfg_channel.expect("channel set before Ready");

        // Validate the channel and derive the demapper's N0 from the SAME
        // channel, so (a) invalid Es/N0 (NaN/inf/underflow) yields a typed
        // `BuildError::InvalidChannel` rather than panicking
        // `GrayQamDemap::with_noise_var`, and (b) the demapper assumes exactly
        // the noise the channel injects (physically consistent LLRs). See
        // `Channel::validate` / `Channel::demap_noise_var`.
        let demap_noise_var = channel.validate()?;

        // SSOT BICM stage order: forward = [encode, interleave, map],
        // inverse = [demap, deinterleave, decode]. The channel slots between.
        // `demap_noise_var` is validated finite & > 0 above, so
        // `with_noise_var` inside the factory cannot panic; `rate`/`modulation`
        // are in-scope by `Modcod::validate`, so the factory's codec/interleaver
        // construction cannot panic either.
        let stages = dvb_t2_bicm_stages(rate, modulation, decoder, demap, demap_noise_var);
        let channel_stage = channel.into_stage(modulation.bits_per_cell());

        // GPU offload placement (Phase C `75c22fa8`, deliverable 3): with
        // `with_gpu(true)` and the `hip` feature compiled in, the combined CPU
        // decode stage is replaced by the `ExecutionClass::GpuOnly` LDPC BP
        // decode stage — its `CpuLdpcBp` fallback registered on the chain for
        // §8 OOM substitution — followed by the CPU BCH outer-decode tail
        // ([`DvbT2BchTail`](crate::stages::DvbT2BchTail)). The scheduler then
        // DISCOVERS the GPU stage from the pipeline's stage list by execution
        // class instead of reconstructing it. Without `hip` the flag degrades
        // gracefully: the chain stays all-CPU and the scheduler warns at run
        // time (the flag is still recorded on the config).
        #[cfg(feature = "hip")]
        let gpu_decode = self.cfg_gpu_enabled;
        #[cfg(not(feature = "hip"))]
        let gpu_decode = false;

        let mut inverse = stages.inverse;
        if gpu_decode {
            // Drop the combined CPU `DvbT2Decode` (the last inverse stage);
            // the GPU LDPC decode + BCH tail below replace it.
            inverse.pop();
        }

        let mut chain = Chain::new();
        let mut ids = Vec::with_capacity(8);
        for stage in stages.forward {
            ids.push(chain.add(stage));
        }
        ids.push(chain.add(channel_stage));
        for stage in inverse {
            ids.push(chain.add(stage));
        }
        #[cfg(feature = "hip")]
        if gpu_decode {
            // The GPU stage runs the same iteration cap as the codec's own
            // soft decode, so its hard decisions match the CPU chain's.
            let max_iters = stages.codec.max_ldpc_iterations();
            let gpu_id = chain.add(erase(crate::gpu::ldpc_bp::GpuLdpcBp::new(
                stages.codec.ldpc_code(),
                decoder,
                max_iters,
            )));
            let fb_id = chain.add(erase(crate::gpu::ldpc_bp::CpuLdpcBp::new(
                stages.codec.ldpc_code(),
                decoder,
                max_iters,
            )));
            chain.register_fallback(gpu_id, fb_id);
            ids.push(gpu_id);
            ids.push(chain.add(erase(crate::stages::DvbT2BchTail::new(
                stages.codec.clone(),
            ))));
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
            gpu_enabled: self.cfg_gpu_enabled,
            strict_gpu: false,
            diagnostic_dump_dir: None,
            inject_gpu_oom_modulus: None,
        };

        let mut pipeline = chain.with_config(config).build()?;
        // Attach the run plan so `Pipeline::run` can rebuild the validated
        // DVB-T2 BICM frame kernel per SNR point (the scheduler engine).
        pipeline.set_run_plan(crate::executor::RunPlan::Dvbt2 {
            rate,
            modulation,
            decoder,
            demap,
        });
        Ok(pipeline)
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
            cfg_gpu_enabled: self.cfg_gpu_enabled,
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
    fn test_validate_rejects_out_of_scope_rate_reports_true_rate() {
        // Rate5_6 is out of scope; the error must report the ACTUAL rate, not a
        // lossy fold onto an in-scope value.
        let bad = Modcod::Normal {
            rate: CodeRate::Rate5_6,
            modulation: DvbT2Modulation::Qam16,
        };
        match bad.validate() {
            Err(BuildError::InvalidModcod { rate, modulation }) => {
                assert_eq!(rate, "5/6", "must report the true offending rate");
                // The modulation was in-scope; it is still reported faithfully.
                assert_eq!(modulation, "16-QAM");
            }
            other => panic!("expected InvalidModcod, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_rejects_qpsk_reports_true_modulation() {
        // QPSK is out of scope; the error must report "QPSK", not "16-QAM".
        let bad = Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qpsk,
        };
        match bad.validate() {
            Err(BuildError::InvalidModcod { rate, modulation }) => {
                assert_eq!(rate, "1/2");
                assert_eq!(
                    modulation, "QPSK",
                    "must report the true offending modulation"
                );
            }
            other => panic!("expected InvalidModcod, got {other:?}"),
        }
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

    /// Builds a valid `Ready` builder differing only in the channel, so the
    /// channel-validation tests vary exactly one input.
    fn build_with_channel(channel: Channel) -> Result<Pipeline, BuildError> {
        Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(sp())
            .demap(DemapMethod::ExactLogMap)
            .channel(channel)
            .build()
    }

    #[test]
    fn test_build_rejects_nan_es_n0_without_panicking() {
        // A NaN Es/N0 must yield a typed error, not panic GrayQamDemap.
        let result = build_with_channel(Channel::awgn(f32::NAN));
        assert!(matches!(result, Err(BuildError::InvalidChannel { .. })));
    }

    #[test]
    fn test_build_rejects_infinite_es_n0_without_panicking() {
        let pos = build_with_channel(Channel::awgn(f32::INFINITY));
        assert!(matches!(pos, Err(BuildError::InvalidChannel { .. })));
        let neg = build_with_channel(Channel::awgn(f32::NEG_INFINITY));
        assert!(matches!(neg, Err(BuildError::InvalidChannel { .. })));
    }

    #[test]
    fn test_build_rejects_underflowing_es_n0_without_panicking() {
        // A very large Es/N0 drives sigma (hence N0 = 2*sigma^2) to underflow to
        // exactly 0.0 in f32; 1000 dB does so (sigma = 1/sqrt(2*10^100) -> 0).
        // build() must reject the non-positive N0 with a typed error rather than
        // panic GrayQamDemap::with_noise_var.
        assert_eq!(
            Channel::awgn(1000.0).demap_noise_var(),
            0.0,
            "1000 dB must underflow N0 to 0 (test premise)"
        );
        let result = build_with_channel(Channel::awgn(1000.0));
        assert!(matches!(result, Err(BuildError::InvalidChannel { .. })));
    }

    #[test]
    fn test_build_accepts_normal_es_n0() {
        // A normal finite Es/N0 still builds the seven-stage pipeline.
        let pipeline = build_with_channel(Channel::awgn(6.0)).expect("normal Es/N0 builds");
        assert_eq!(pipeline.stage_count(), 7);
    }

    #[test]
    fn test_channel_validate_returns_n0_for_valid_es_n0() {
        // validate() returns the demapper N0 = 2*sigma^2 for a valid channel,
        // derived in f64 and rounded once — bit-identical to the SSOT frame
        // kernel's `noise_var` derivation (see `Channel::demap_noise_var`).
        let n0 = Channel::awgn(6.0).validate().expect("valid channel");
        let es_n0_lin = 10.0_f64.powf(6.0 / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        assert_eq!(
            n0.to_bits(),
            ((2.0 * sigma_sq) as f32).to_bits(),
            "demapper N0 must be the single-rounded f64 derivation"
        );
        // And it stays within an ULP of the doubly-rounded 2*sigma^2 form (the
        // physical-consistency sanity check).
        let sigma = crate::channels::es_n0_db_to_sigma(6.0);
        assert!((n0 - 2.0 * sigma * sigma).abs() < 1e-6);
    }

    /// `with_gpu(true)` under `hip` must place the `GpuOnly` LDPC decode stage
    /// into the pipeline's stage list (discoverable by execution class and
    /// downcastable to the concrete `GpuLdpcBp`), register its CPU fallback,
    /// and append the BCH outer-decode tail — eight stages total (75c22fa8
    /// deliverable 3 wiring). Construction touches no device, so this runs on
    /// any host.
    #[cfg(feature = "hip")]
    #[test]
    fn test_with_gpu_places_discoverable_gpu_stage() {
        use crate::stage::ExecutionClass;

        let pipeline = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(sp())
            .demap(DemapMethod::MaxLog)
            .channel(Channel::awgn(6.0))
            .with_gpu(true)
            .build()
            .expect("in-scope MODCOD builds with GPU offload");

        assert_eq!(pipeline.stage_count(), 8, "GPU chain: 7 stages + BCH tail");
        assert_eq!(pipeline.edges().len(), 7);
        assert_eq!(
            pipeline.fallback_count(),
            1,
            "the GPU LDPC stage's CpuLdpcBp fallback must be registered"
        );

        let gpu_stages: Vec<_> = pipeline
            .stages()
            .iter()
            .filter(|s| s.execution_class() == ExecutionClass::GpuOnly)
            .collect();
        assert_eq!(gpu_stages.len(), 1, "exactly one GpuOnly stage");
        let concrete = gpu_stages[0]
            .stage_as_any()
            .expect("erased stage exposes its concrete stage")
            .downcast_ref::<crate::gpu::ldpc_bp::GpuLdpcBp>();
        assert!(
            concrete.is_some(),
            "the GpuOnly stage must downcast to gpu::ldpc_bp::GpuLdpcBp"
        );
        assert_eq!(
            concrete.unwrap().max_iterations(),
            50,
            "the GPU stage must run the codec's own BP iteration cap"
        );
    }

    /// Without `with_gpu(true)` the chain stays all-CPU even on a hip build:
    /// seven stages, no `GpuOnly` stage, no registered fallback.
    #[cfg(feature = "hip")]
    #[test]
    fn test_without_gpu_chain_stays_all_cpu() {
        use crate::stage::ExecutionClass;

        let pipeline = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(sp())
            .demap(DemapMethod::MaxLog)
            .channel(Channel::awgn(6.0))
            .with_gpu(false)
            .build()
            .expect("in-scope MODCOD builds");

        assert_eq!(pipeline.stage_count(), 7);
        assert_eq!(pipeline.fallback_count(), 0);
        assert!(
            pipeline
                .stages()
                .iter()
                .all(|s| s.execution_class() == ExecutionClass::CpuOnly),
            "every stage in the CPU chain is CpuOnly"
        );
    }
}
