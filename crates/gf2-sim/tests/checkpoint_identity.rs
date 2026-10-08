//! Checkpoint identity of a preset-built [`Pipeline`]: it separates link
//! configurations, a resume under another identity is rejected, and a resumed
//! sweep equals an uninterrupted one.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::error::{BuildError, FatalError};
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::snr_checkpoint::{
    checkpoint_path, clear_interrupt, pipeline_config_hash, request_interrupt, SweepError,
};
use gf2_sim::{Pipeline, Scheduler};

/// Serializes the tests that run a sweep: the interrupt flag is process-wide.
static SWEEP_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn sweep_lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = SWEEP_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_interrupt();
    guard
}

#[derive(Clone, Copy)]
struct Link {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    decoder: DecoderConfig,
    demap: DemapMethod,
}

const ES_N0_DB: f64 = 6.0;
const MAX_FRAMES: u64 = 4;
const HEARTBEAT: u64 = 2;

impl Link {
    fn base() -> Self {
        Self {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            decoder: DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            demap: DemapMethod::MaxLog,
        }
    }

    fn pipeline(self, checkpoint_dir: Option<PathBuf>) -> Pipeline {
        let mut pipeline = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: self.rate,
                modulation: self.modulation,
            })
            .decoder(self.decoder)
            .demap(self.demap)
            .channel(Channel::awgn(ES_N0_DB as f32))
            .parallelism(NonZeroUsize::new(2).unwrap())
            .seed(0x3305_7520)
            .checkpoint_dir(checkpoint_dir)
            .build()
            .expect("pipeline builds");
        let cfg = pipeline.config_mut();
        cfg.esn0_db_points = vec![ES_N0_DB];
        cfg.max_frames = MAX_FRAMES;
        cfg.heartbeat_every_frames = HEARTBEAT;
        pipeline
    }

    fn identity(self) -> String {
        pipeline_config_hash(&self.pipeline(None))
    }
}

/// One variant of [`Link::base`] per link field, each differing in that field
/// only.
fn single_field_variants() -> [(&'static str, Link); 4] {
    let base = Link::base();
    [
        (
            "rate",
            Link {
                rate: CodeRate::Rate2_3,
                ..base
            },
        ),
        (
            "modulation",
            Link {
                modulation: DvbT2Modulation::Qam64,
                ..base
            },
        ),
        (
            "decoder",
            Link {
                decoder: DecoderConfig::new(DecoderAlgorithm::MinSum, true),
                ..base
            },
        ),
        (
            "demapper",
            Link {
                demap: DemapMethod::ExactLogMap,
                ..base
            },
        ),
    ]
}

fn variant(field: &str) -> Link {
    single_field_variants()
        .into_iter()
        .find(|(name, _)| *name == field)
        .expect("a variant exists per link field")
        .1
}

#[test]
fn identity_is_stable_for_one_configuration() {
    assert_eq!(Link::base().identity(), Link::base().identity());
}

#[test]
fn identity_separates_code_rates() {
    assert_ne!(Link::base().identity(), variant("rate").identity());
}

#[test]
fn identity_separates_modulations() {
    assert_ne!(Link::base().identity(), variant("modulation").identity());
}

#[test]
fn identity_separates_decoders() {
    let base = Link::base();
    assert_ne!(base.identity(), variant("decoder").identity());

    let with_decoder = |decoder| Link { decoder, ..base }.identity();
    assert_ne!(
        with_decoder(DecoderConfig::new(
            DecoderAlgorithm::NormalizedMinSum(0.75),
            true
        )),
        with_decoder(DecoderConfig::new(
            DecoderAlgorithm::NormalizedMinSum(0.8),
            true
        )),
        "the normalization factor is part of the identity"
    );
    assert_ne!(
        with_decoder(DecoderConfig::new(
            DecoderAlgorithm::OffsetMinSum(0.5),
            true
        )),
        with_decoder(DecoderConfig::new(
            DecoderAlgorithm::NormalizedMinSum(0.5),
            true
        )),
        "the algorithm is part of the identity at equal parameter"
    );
    assert_ne!(
        with_decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true)),
        with_decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, false)),
        "early termination is part of the identity"
    );
}

#[test]
fn identity_separates_demappers() {
    assert_ne!(Link::base().identity(), variant("demapper").identity());
}

#[test]
fn identity_ignores_output_locations() {
    let mut relocated = Link::base().pipeline(Some("/elsewhere/checkpoints".into()));
    relocated.config_mut().tracing_log_path = Some("/elsewhere/tracing.jsonl".into());
    relocated.config_mut().diagnostic_dump_dir = Some("/elsewhere/dumps".into());
    assert_eq!(pipeline_config_hash(&relocated), Link::base().identity());
}

fn checkpoint_bytes(dir: &Path) -> Vec<u8> {
    std::fs::read(checkpoint_path(dir, 0)).expect("point-0 checkpoint exists")
}

#[test]
fn resume_under_a_different_identity_reuses_no_checkpoint() {
    let _guard = sweep_lock();
    let dir = gf2_core::test_scratch::scratch("gf2sim-ck-identity-mismatch");

    let mut written = Link::base().pipeline(Some(dir.path().to_path_buf()));
    written.config_mut().max_frames = 1;
    written.config_mut().heartbeat_every_frames = 0;
    let written_identity = pipeline_config_hash(&written);
    let sweep = Scheduler::from_pipeline(&written)
        .run_sweep_checkpointed(&written, false, &|_, _| {})
        .expect("the checkpoint-writing sweep completes");
    assert_eq!(sweep.results.per_point[0].frames, 1);
    let on_disk = checkpoint_bytes(dir.path());

    for (field, link) in single_field_variants() {
        let mut resumed = link.pipeline(Some(dir.path().to_path_buf()));
        resumed.config_mut().max_frames = 1;
        resumed.config_mut().heartbeat_every_frames = 0;
        let frames_simulated = std::sync::atomic::AtomicU64::new(0);
        let outcome =
            Scheduler::from_pipeline(&resumed).run_sweep_checkpointed(&resumed, true, &|_, _| {
                frames_simulated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            });
        match outcome {
            Err(SweepError::Load(FatalError::BuildError(BuildError::ConfigHashMismatch {
                loaded,
                expected,
            }))) => {
                assert_eq!(loaded, written_identity, "{field}: loaded identity");
                assert_eq!(
                    expected,
                    pipeline_config_hash(&resumed),
                    "{field}: live identity"
                );
            }
            Err(other) => panic!("{field}: expected ConfigHashMismatch, got {other}"),
            Ok(sweep) => panic!(
                "{field}: the resume reported another configuration's checkpoint: {:?}",
                sweep.results.per_point
            ),
        }
        assert_eq!(
            frames_simulated.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "{field}: the rejected resume simulates nothing"
        );
        assert_eq!(
            checkpoint_bytes(dir.path()),
            on_disk,
            "{field}: the rejected resume leaves the checkpoint untouched"
        );
    }
}

#[test]
fn resumed_sweep_equals_uninterrupted_sweep() {
    let _guard = sweep_lock();

    let reference_dir = gf2_core::test_scratch::scratch("gf2sim-ck-identity-reference");
    let reference = Link::base().pipeline(Some(reference_dir.path().to_path_buf()));
    let uninterrupted = Scheduler::from_pipeline(&reference)
        .run_sweep_checkpointed(&reference, false, &|_, _| {})
        .expect("the uninterrupted sweep completes");
    assert!(!uninterrupted.interrupted);

    let dir = gf2_core::test_scratch::scratch("gf2sim-ck-identity-resume");
    let pipeline = Link::base().pipeline(Some(dir.path().to_path_buf()));
    let scheduler = Scheduler::from_pipeline(&pipeline);
    let first = scheduler
        .run_sweep_checkpointed(&pipeline, false, &|_, g| {
            if g == 0 {
                request_interrupt();
            }
        })
        .expect("the interrupted sweep flushes a checkpoint");
    clear_interrupt();
    assert!(first.interrupted);
    assert_eq!(
        first.results.per_point[0].frames, HEARTBEAT,
        "the interrupt lands at the first heartbeat boundary"
    );

    let resumed = scheduler
        .run_sweep_checkpointed(&pipeline, true, &|_, _| {})
        .expect("the resumed sweep completes");
    assert!(!resumed.interrupted);
    assert_eq!(resumed.results.per_point[0].frames, MAX_FRAMES);
    assert_eq!(resumed.results, uninterrupted.results);
}
