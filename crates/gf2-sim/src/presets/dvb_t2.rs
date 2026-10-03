//! DVB-T2 BICM preset: a typestate fluent builder over the graph API.
//!
//! [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2) returns a
//! `Builder<NeedsModcod>`. The required setters are called in order,
//! [`modcod`](Builder::modcod) → [`decoder`](Builder::decoder) →
//! [`demap`](Builder::demap) → [`channel`](Builder::channel); each exists only
//! on its predecessor state. A [`Builder<Ready>`] exposes the optional setters
//! and [`build`](Builder::build), which takes the stage order from
//! [`dvb_t2_bicm_stages`], inserts the [`Awgn`] channel between the forward and
//! inverse halves, and calls [`Chain::build`].
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
/// The supported MODCODs are `rate ∈ {1/2, 2/3, 3/4}` crossed with
/// `modulation ∈ {16-QAM, 64-QAM}`. [`Modcod::validate`] rejects any other
/// combination.
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
    fn parts(self) -> (CodeRate, DvbT2Modulation) {
        match self {
            Modcod::Normal { rate, modulation } => (rate, modulation),
        }
    }

    /// Validates that this MODCOD is one of the supported combinations.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::InvalidModcod`], carrying the requested rate and
    /// modulation as strings (e.g. `rate = "5/6"`, `modulation = "QPSK"`), when
    /// the pair is outside the supported set.
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

fn modulation_label(modulation: DvbT2Modulation) -> &'static str {
    match modulation {
        DvbT2Modulation::Qpsk => "QPSK",
        DvbT2Modulation::Qam16 => "16-QAM",
        DvbT2Modulation::Qam64 => "64-QAM",
    }
}

/// The channel [`build`](Builder::build) inserts between the forward
/// (transmit) and inverse (receive) halves of the BICM chain.
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
    /// soft demapper must assume to be physically consistent with this channel.
    ///
    /// Computed in `f64` and rounded once, `N0 = (2 * sigma_sq) as f32` with
    /// `sigma_sq = 1 / (2 * 10^(Es/N0 / 10))`, matching
    /// [`DvbT2BicmFrameSim::noise_var`](crate::frame_sim::DvbT2BicmFrameSim::noise_var)
    /// bit for bit. Squaring the `f32` sigma the [`Awgn`] stage injects rounds
    /// twice and can differ by an ULP.
    ///
    /// May be non-finite or zero if `es_n0_db` is non-finite or so large that
    /// `sigma^2` underflows; [`Channel::validate`] rejects those cases.
    fn demap_noise_var(self) -> f32 {
        match self {
            Channel::Awgn { es_n0_db } => crate::channels::es_n0_db_to_n0(es_n0_db),
        }
    }

    /// Validates this channel's parameters, returning the demapper `N0` it
    /// implies on success.
    ///
    /// The rejected inputs are those that panic
    /// [`GrayQamDemap::with_noise_var`](crate::stages::GrayQamDemap::with_noise_var).
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

/// Typestate marker: only [`Builder::modcod`] is available.
#[derive(Debug)]
pub enum NeedsModcod {}

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

/// A typestate fluent builder for the DVB-T2 BICM pipeline.
///
/// `State` is one of [`NeedsModcod`], [`NeedsDecoder`], [`NeedsDemap`],
/// [`NeedsChannel`], [`Ready`]. Construct one via
/// [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2); see the [module docs](self)
/// for the call sequence.
pub struct Builder<State> {
    // The `cfg_` prefix keeps field names distinct from the setter names, so
    // an out-of-order call reports "no method named `decoder`" instead of
    // "private field, not a method".
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
    /// Defaults: `parallelism = 1`, `seed = 0`, no checkpoint directory, GPU
    /// offload disabled.
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
    /// [`build`](Builder::build) validates it via [`Modcod::validate`].
    pub fn modcod(self, modcod: Modcod) -> Builder<NeedsDecoder> {
        self.with_state(|b| b.cfg_modcod = Some(modcod))
    }
}

impl Builder<NeedsDecoder> {
    /// Sets the LDPC belief-propagation decoder configuration, advancing to
    /// [`NeedsDemap`].
    pub fn decoder(self, decoder: DecoderConfig) -> Builder<NeedsDemap> {
        self.with_state(|b| b.cfg_decoder = Some(decoder))
    }
}

impl Builder<NeedsDemap> {
    /// Sets the soft-demap method, advancing to [`NeedsChannel`].
    pub fn demap(self, demap: DemapMethod) -> Builder<NeedsChannel> {
        self.with_state(|b| b.cfg_demap = Some(demap))
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

    /// Enables or disables GPU offload of the LDPC decode, and records
    /// `gpu_enabled` on the [`PipelineConfig`].
    ///
    /// When `true` under the `hip` feature, [`build`](Builder::build) replaces
    /// the combined CPU decode stage with the
    /// [`ExecutionClass::GpuOnly`](crate::ExecutionClass) LDPC BP decode stage
    /// (`gpu::ldpc_bp::GpuLdpcBp`, its `CpuLdpcBp` fallback registered on the
    /// pipeline) followed by the CPU BCH outer-decode tail
    /// ([`DvbT2BchTail`](crate::stages::DvbT2BchTail)): an eight-stage chain.
    /// Without `hip` the chain stays all-CPU (seven stages).
    #[must_use]
    pub fn with_gpu(mut self, enabled: bool) -> Self {
        self.cfg_gpu_enabled = enabled;
        self
    }

    /// Validates the MODCOD and compiles the BICM chain into a [`Pipeline`].
    ///
    /// The chain is the three forward and three inverse stages of
    /// [`dvb_t2_bicm_stages`] with the channel stage between them (seven
    /// stages), or the eight-stage GPU chain of
    /// [`with_gpu(true)`](Builder::with_gpu). The demapper's noise variance is
    /// derived from the channel's Es/N0.
    ///
    /// # Errors
    ///
    /// * [`BuildError::InvalidModcod`] if the `(rate, modulation)` pair is
    ///   unsupported (see [`Modcod::validate`]).
    /// * [`BuildError::InvalidChannel`] if the AWGN Es/N0 is non-finite, or so
    ///   large that the derived demapper noise variance underflows to a
    ///   non-positive value.
    ///
    /// # Complexity
    ///
    /// Dominated by constructing the
    /// [`DvbT2Concat`](gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat) codec
    /// and the LDPC encoder cache inside [`dvb_t2_bicm_stages`].
    pub fn build(self) -> Result<Pipeline, BuildError> {
        // The required fields are `Some` by typestate.
        let modcod = self.cfg_modcod.expect("modcod set before Ready");
        // Validated first so `dvb_t2_bicm_stages`'s codec `expect` is never
        // reached on an unsupported rate.
        modcod.validate()?;
        let (rate, modulation) = modcod.parts();
        let decoder = self.cfg_decoder.expect("decoder set before Ready");
        let demap = self.cfg_demap.expect("demap set before Ready");
        let channel = self.cfg_channel.expect("channel set before Ready");

        let demap_noise_var = channel.validate()?;

        // forward = [encode, interleave, map], inverse = [demap, deinterleave,
        // decode]. The validated `demap_noise_var`, `rate` and `modulation`
        // keep the factory's asserts unreachable.
        let stages = dvb_t2_bicm_stages(rate, modulation, decoder, demap, demap_noise_var);
        let channel_stage = channel.into_stage(modulation.bits_per_cell());

        #[cfg(feature = "hip")]
        let gpu_decode = self.cfg_gpu_enabled;
        #[cfg(not(feature = "hip"))]
        let gpu_decode = false;

        let mut inverse = stages.inverse;
        if gpu_decode {
            // The GPU LDPC decode + BCH tail below replace the combined CPU
            // `DvbT2Decode`, the last inverse stage.
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
    /// See the [module docs](crate::presets::dvb_t2) for the call sequence.
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
        let bad = Modcod::Normal {
            rate: CodeRate::Rate5_6,
            modulation: DvbT2Modulation::Qam16,
        };
        match bad.validate() {
            Err(BuildError::InvalidModcod { rate, modulation }) => {
                assert_eq!(rate, "5/6", "must report the true offending rate");
                assert_eq!(modulation, "16-QAM");
            }
            other => panic!("expected InvalidModcod, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_rejects_qpsk_reports_true_modulation() {
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
        // At 1000 dB, N0 = 2*sigma^2 = 10^-100 underflows to 0.0 in f32.
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
        let pipeline = build_with_channel(Channel::awgn(6.0)).expect("normal Es/N0 builds");
        assert_eq!(pipeline.stage_count(), 7);
    }

    #[test]
    fn test_channel_validate_returns_n0_for_valid_es_n0() {
        let n0 = Channel::awgn(6.0).validate().expect("valid channel");
        let es_n0_lin = 10.0_f64.powf(6.0 / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        assert_eq!(
            n0.to_bits(),
            ((2.0 * sigma_sq) as f32).to_bits(),
            "demapper N0 must be the single-rounded f64 derivation"
        );
        // Close to the doubly-rounded 2*sigma^2 form.
        let sigma = crate::channels::es_n0_db_to_sigma(6.0);
        assert!((n0 - 2.0 * sigma * sigma).abs() < 1e-6);
    }

    /// Construction touches no device, so this runs on any host.
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
