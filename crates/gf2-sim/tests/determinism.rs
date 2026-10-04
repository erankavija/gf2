//! Determinism of the DVB-T2 frame kernel at a fixed seed: `frames`, `errors`,
//! `fer` and `mean_iters` are byte-identical across worker counts
//! `{1, 2, 4, 8, 24}`, and a run interrupted at a heartbeat and resumed matches
//! the uninterrupted run. Each configuration also builds through
//! [`Pipeline::dvb_t2`](gf2_sim::Pipeline::dvb_t2).

use std::num::NonZeroUsize;
use std::path::PathBuf;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::snr_checkpoint::{
    clear_interrupt, config_hash, request_interrupt, run_snr_point_checkpointed, CheckpointReader,
    CheckpointWriter,
};
use gf2_sim::{Pipeline, PipelineConfig};

mod common;
use common::assert_four_columns_byte_identical;

/// Worker counts are split into two groups, each led by the 1-worker baseline,
/// so that each test fits the slow-tier budget.
const WORKER_GROUP_NARROW: [usize; 2] = [1, 2];

const WORKER_GROUP_WIDE: [usize; 4] = [1, 4, 8, 24];

const FRAMES: usize = 200;

const INTERRUPT_AT_FRAME: usize = 100;

const SEED: u64 = 0xC0DE_F00D;

/// Serializes the resume-parity tests, which set and clear the process-wide
/// checkpoint interrupt flag and may share one process.
static RESUME_PARITY_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Copy)]
struct DetConfig {
    /// Selects a disjoint RNG region per config.
    snr_idx: usize,
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    algo: DecoderAlgorithm,
    demap: DemapMethod,
}

impl DetConfig {
    fn label(&self) -> String {
        format!("{:?}/{:?}@{}dB", self.rate, self.modulation, self.es_n0_db)
    }

    fn decoder(&self) -> DecoderConfig {
        DecoderConfig::new(self.algo, true)
    }
}

/// Each Es/N0 lies above the `@/citation/Etsi2012` Table 44 QEF C/N of its
/// MODCOD, so the frames converge.
const CONFIGS: [DetConfig; 3] = [
    DetConfig {
        snr_idx: 0,
        rate: CodeRate::Rate1_2,
        modulation: DvbT2Modulation::Qam16,
        es_n0_db: 9.0,
        algo: DecoderAlgorithm::SumProduct,
        demap: DemapMethod::ExactLogMap,
    },
    DetConfig {
        snr_idx: 1,
        rate: CodeRate::Rate2_3,
        modulation: DvbT2Modulation::Qam64,
        es_n0_db: 17.0,
        algo: DecoderAlgorithm::NormalizedMinSum(0.75),
        demap: DemapMethod::ExactLogMap,
    },
    DetConfig {
        snr_idx: 2,
        rate: CodeRate::Rate3_4,
        modulation: DvbT2Modulation::Qam16,
        es_n0_db: 13.0,
        algo: DecoderAlgorithm::MinSum,
        demap: DemapMethod::MaxLog,
    },
];

/// Builds the preset pipeline for `cfg` and the frame kernel with the same parameters.
fn seeded_runner_factory(
    cfg: DetConfig,
    parallelism: NonZeroUsize,
    checkpoint_dir: Option<PathBuf>,
) -> (Pipeline, DvbT2BicmFrameSim) {
    let pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: cfg.rate,
            modulation: cfg.modulation,
        })
        .decoder(cfg.decoder())
        .demap(cfg.demap)
        .channel(Channel::awgn(cfg.es_n0_db as f32))
        .parallelism(parallelism)
        .seed(SEED)
        .checkpoint_dir(checkpoint_dir)
        .build()
        .expect("the three named MODCODs are in-scope and build through the production preset");

    assert_eq!(
        pipeline.stage_count(),
        7,
        "production DVB-T2 chain is 7 stages"
    );
    assert_eq!(
        pipeline.config().seed,
        SEED,
        "seed threaded onto the pipeline config"
    );

    let frame_sim = DvbT2BicmFrameSim::new(
        cfg.rate,
        cfg.modulation,
        cfg.es_n0_db,
        cfg.decoder(),
        cfg.demap,
    );
    (pipeline, frame_sim)
}

fn run_worker_count(cfg: DetConfig, workers: usize) -> WorkerCounters {
    let p = NonZeroUsize::new(workers).expect("worker count is non-zero");
    let (_pipeline, frame_sim) = seeded_runner_factory(cfg, p, None);
    run_snr_point(
        SEED,
        cfg.snr_idx,
        FRAMES,
        p,
        || frame_sim.clone(),
        |g, ctx, s| s.simulate_frame(g, ctx),
    )
}

fn record_ber(c: &WorkerCounters, label: &str, workers: usize) {
    let ber = if c.total_bits == 0 {
        0.0
    } else {
        c.total_bit_errors as f64 / c.total_bits as f64
    };
    eprintln!("{label} @ {workers} workers: BER = {ber:e} (recorded, NOT asserted)");
}

fn assert_workers_byte_identical(cfg: DetConfig, workers: &[usize]) {
    assert_eq!(
        workers[0], 1,
        "the first worker count must be the 1-worker baseline"
    );
    let label = cfg.label();

    let baseline = run_worker_count(cfg, 1);
    assert_eq!(
        baseline.frames, FRAMES as u64,
        "{label}: baseline frame budget"
    );
    record_ber(&baseline, &label, 1);

    for &w in &workers[1..] {
        let c = run_worker_count(cfg, w);
        assert_eq!(
            c.frames, FRAMES as u64,
            "{label} @ {w} workers: frame budget"
        );
        assert_four_columns_byte_identical(&c, &baseline, &format!("{label} @ {w} workers"));
        record_ber(&c, &label, w);
    }
}

fn run_uninterrupted(cfg: DetConfig, parallelism: NonZeroUsize) -> WorkerCounters {
    let dir = tempdir();
    let (_pipeline, frame_sim) =
        seeded_runner_factory(cfg, parallelism, Some(dir.path().to_path_buf()));
    let config = checkpoint_config(parallelism, FRAMES, dir.path(), cfg.es_n0_db);
    let hash = config_hash(&config);
    let writer = CheckpointWriter::new(dir.path()).expect("create checkpoint dir");
    clear_interrupt();
    let run = run_snr_point_checkpointed(
        &config,
        cfg.snr_idx,
        cfg.es_n0_db,
        &writer,
        &hash,
        None, // fresh
        || frame_sim.clone(),
        |g, ctx, s| s.simulate_frame(g, ctx),
        |_, _| {},
    )
    .expect("uninterrupted checkpointed run");
    assert!(run.completed, "uninterrupted run must complete");
    run.counters
}

fn assert_resume_parity(cfg: DetConfig) {
    let _guard = RESUME_PARITY_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_interrupt();

    let label = cfg.label();
    // Two workers exercise multi-worker striding on resume.
    let parallelism = NonZeroUsize::new(2).expect("2 is non-zero");

    let reference = run_uninterrupted(cfg, parallelism);
    assert_eq!(
        reference.frames, FRAMES as u64,
        "{label}: reference frame budget"
    );

    let dir = tempdir();
    let (_pipeline, frame_sim) =
        seeded_runner_factory(cfg, parallelism, Some(dir.path().to_path_buf()));
    let writer = CheckpointWriter::new(dir.path()).expect("create checkpoint dir");
    let config = checkpoint_config(parallelism, FRAMES, dir.path(), cfg.es_n0_db);
    let hash = config_hash(&config);

    clear_interrupt();
    let interrupted = run_snr_point_checkpointed(
        &config,
        cfg.snr_idx,
        cfg.es_n0_db,
        &writer,
        &hash,
        None, // fresh
        || frame_sim.clone(),
        |g, ctx, s| s.simulate_frame(g, ctx),
        // The resumable checkpoint is on disk when this callback runs.
        |_snr, frames_completed| {
            assert_eq!(
                frames_completed, INTERRUPT_AT_FRAME as u64,
                "{label}: heartbeat flush fires at the interrupt boundary"
            );
            request_interrupt();
        },
    )
    .expect("interrupted checkpointed run");
    assert!(
        interrupted.interrupted,
        "{label}: run observed the SIGINT and stopped early"
    );
    assert!(
        !interrupted.completed,
        "{label}: interrupted run did not complete the point"
    );
    assert_eq!(
        interrupted.counters.frames, INTERRUPT_AT_FRAME as u64,
        "{label}: interrupted at the frame-100 boundary",
    );

    // Clear first, or the resume's first chunk-boundary check stops again.
    clear_interrupt();
    let reader = CheckpointReader::new(dir.path(), hash.clone());
    let loaded = reader
        .load(cfg.snr_idx)
        .expect("load interrupted checkpoint")
        .expect("interrupted checkpoint was flushed");
    assert!(
        !loaded.completed,
        "{label}: the flushed checkpoint is resumable (completed = false)"
    );
    assert_eq!(
        loaded.frames_completed, INTERRUPT_AT_FRAME as u64,
        "{label}: checkpoint recorded the frame-100 resume point",
    );
    let resumed = run_snr_point_checkpointed(
        &config,
        cfg.snr_idx,
        cfg.es_n0_db,
        &writer,
        &hash,
        Some(loaded),
        || frame_sim.clone(),
        |g, ctx, s| s.simulate_frame(g, ctx),
        |_, _| {},
    )
    .expect("resumed checkpointed run");
    assert!(
        resumed.completed,
        "{label}: resumed run completes the point"
    );

    assert_four_columns_byte_identical(
        &resumed.counters,
        &reference,
        &format!("{label} resume-parity"),
    );
    assert_eq!(
        resumed.counters.frames, FRAMES as u64,
        "{label}: resumed run reaches the full frame budget",
    );
}

fn checkpoint_config(
    parallelism: NonZeroUsize,
    max_frames: usize,
    dir: &std::path::Path,
    es_n0_db: f64,
) -> PipelineConfig {
    PipelineConfig {
        seed: SEED,
        esn0_db_points: vec![es_n0_db],
        target_errors: 0, // run every frame; no early stop (byte-identity intact)
        max_frames: max_frames as u64,
        heartbeat_every_frames: INTERRUPT_AT_FRAME as u64,
        checkpoint_dir: Some(dir.to_path_buf()),
        tracing_log_path: None,
        parallelism,
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
    }
}

#[test]
#[ignore = "sim: preset-path determinism workers {1,2} — r1/2 16-QAM SumProduct/ExactLogMap"]
fn determinism_preset_r1_2_16qam_workers_1_2() {
    assert_workers_byte_identical(CONFIGS[0], &WORKER_GROUP_NARROW);
}

#[test]
#[ignore = "sim: preset-path determinism workers {1,4,8,24} — r1/2 16-QAM SumProduct/ExactLogMap"]
fn determinism_preset_r1_2_16qam_workers_1_4_8_24() {
    assert_workers_byte_identical(CONFIGS[0], &WORKER_GROUP_WIDE);
}

#[test]
#[ignore = "sim: preset-path determinism workers {1,2} — r2/3 64-QAM NMS(0.75)/ExactLogMap"]
fn determinism_preset_r2_3_64qam_workers_1_2() {
    assert_workers_byte_identical(CONFIGS[1], &WORKER_GROUP_NARROW);
}

#[test]
#[ignore = "sim: preset-path determinism workers {1,4,8,24} — r2/3 64-QAM NMS(0.75)/ExactLogMap"]
fn determinism_preset_r2_3_64qam_workers_1_4_8_24() {
    assert_workers_byte_identical(CONFIGS[1], &WORKER_GROUP_WIDE);
}

#[test]
#[ignore = "sim: preset-path determinism workers {1,2} — r3/4 16-QAM MinSum/MaxLog"]
fn determinism_preset_r3_4_16qam_workers_1_2() {
    assert_workers_byte_identical(CONFIGS[2], &WORKER_GROUP_NARROW);
}

#[test]
#[ignore = "sim: preset-path determinism workers {1,4,8,24} — r3/4 16-QAM MinSum/MaxLog"]
fn determinism_preset_r3_4_16qam_workers_1_4_8_24() {
    assert_workers_byte_identical(CONFIGS[2], &WORKER_GROUP_WIDE);
}

#[test]
#[ignore = "sim: preset-path heartbeat-resume parity — r1/2 16-QAM"]
fn determinism_resume_parity_r1_2_16qam() {
    assert_resume_parity(CONFIGS[0]);
}

#[test]
#[ignore = "sim: preset-path heartbeat-resume parity — r2/3 64-QAM"]
fn determinism_resume_parity_r2_3_64qam() {
    assert_resume_parity(CONFIGS[1]);
}

#[test]
#[ignore = "sim: preset-path heartbeat-resume parity — r3/4 16-QAM"]
fn determinism_resume_parity_r3_4_16qam() {
    assert_resume_parity(CONFIGS[2]);
}

fn tempdir() -> gf2_core::test_scratch::Scratch {
    gf2_core::test_scratch::scratch("gf2sim-det")
}
