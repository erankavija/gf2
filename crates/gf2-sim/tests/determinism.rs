//! CPU determinism property suite over the **typestate preset production path**
//! (issue `48a0db6c`, design doc §3 / §11; closes Phase A story `bcf7776d`).
//!
//! This complements — and does not duplicate —
//! [`parallel_determinism.rs`](./parallel_determinism.rs) (issue `3fcb7025`),
//! which exercises the *direct* [`DvbT2BicmFrameSim`] dispatch via
//! [`run_snr_point`](gf2_sim::parallel::run_snr_point). Here every config is
//! constructed through the **production typestate builder**
//! [`Pipeline::dvb_t2`](gf2_sim::Pipeline::dvb_t2) (issue `81d05bab`) — see
//! [`seeded_runner_factory`] — so the property test rides the same builder a
//! real campaign uses (validating the seven-stage BICM chain assembly and
//! threading `seed` / `parallelism` / `checkpoint_dir` into the
//! [`PipelineConfig`](gf2_sim::PipelineConfig)), not a stripped helper. The
//! byte-identity column set and the BER exclusion are pinned by the shared SSOT
//! helper [`common::assert_four_columns_byte_identical`].
//!
//! # What it asserts
//!
//! For each of three named DVB-T2 MODCODs — **r1/2 16-QAM**, **r2/3 64-QAM**,
//! **r3/4 16-QAM** — at a fixed seed and a fixed Es/N0 (above the smoke-knee so
//! the 200-frame batch fits the slow-tier time budget; see the tiering note):
//!
//! 1. **Across-worker byte-identity.** A 200-frame batch is run at each worker
//!    count in `{1, 2, 4, 8, 24}`; the four columns `fer` / `frames` / `errors`
//!    / `mean_iters` are asserted byte-identical to the 1-worker reference (BER
//!    is recorded but never asserted — issue `152388f4`, design-doc §11). The
//!    worker-count sweep is split into a NARROW group `{1, 2}` and a WIDE group
//!    `{1, 4, 8, 24}` — two tests per config — so each stays under the slow
//!    tier's 120 s/test cap (the heavy 1-worker leg alone is ~60-70 s).
//! 2. **Heartbeat-resume parity.** An interrupted run (SIGINT-equivalent stop at
//!    frame 100) resumed under the full 200-frame budget reproduces the same
//!    final `(fer, frames, errors, mean_iters)` tuple as an uninterrupted
//!    200-frame run.
//!
//! # Test tiering (project hard rule)
//!
//! A 200-frame Normal-frame (n = 64800) DVB-T2 encode+decode is **slow** (a
//! single 1-worker batch is ~60-70 s): every test here is
//! `#[ignore = "sim: ..."]` slow-tier and is excluded from the 5 s fast-tier CI
//! gate. The across-worker sweep is split into two tests per config (NARROW
//! `{1,2}` / WIDE `{1,4,8,24}`) so no single test exceeds the slow tier's
//! 120 s/test cap. Run them all explicitly with:
//!
//! ```bash
//! cargo nextest run -p gf2-sim --release --profile slow \
//!     --run-ignored ignored-only -E 'binary(determinism)'
//! ```
//!
//! The fast-tier seek/aggregation smoke guard lives in `parallel/mod.rs`'s unit
//! tests; this file adds no fast-tier test.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::checkpoint::{
    clear_interrupt, config_hash, request_interrupt, run_snr_point_checkpointed, CheckpointReader,
    CheckpointWriter,
};
use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, PipelineConfig};

mod common;
use common::assert_four_columns_byte_identical;

/// The byte-identity must hold across worker counts `{1, 2, 4, 8, 24}` (the
/// issue's exact set). The 1-worker count is the baseline; every other count is
/// asserted equal to it. The set is covered by two groups —
/// [`WORKER_GROUP_NARROW`] `{1, 2}` and [`WORKER_GROUP_WIDE`] `{1, 4, 8, 24}` —
/// run as separate slow-tier tests per config so each stays under the 120 s/test
/// cap (the full sweep in one test would exceed it; see the module-level tiering
/// note).
///
/// The lighter group: the 1-worker baseline plus 2 workers. A 200-frame
/// 1-worker Normal-frame decode is the single most expensive leg (~60-70 s under
/// load), which is why the worker counts are split rather than swept in one
/// test.
const WORKER_GROUP_NARROW: [usize; 2] = [1, 2];

/// The wider group: the 1-worker baseline plus the three cheap high-worker-count
/// runs `{4, 8, 24}` (each ~5-20 s for 200 frames). Paired with
/// [`WORKER_GROUP_NARROW`] so every count in `{1, 2, 4, 8, 24}` is asserted
/// against the 1-worker baseline.
const WORKER_GROUP_WIDE: [usize; 4] = [1, 4, 8, 24];

/// Frames per worker-count run (the issue's pinned 200-frame batch). Each run
/// is a full Normal-frame (n = 64800) DVB-T2 BICM encode → AWGN → decode batch;
/// at the chosen Es/N0 the frames converge so the per-run wall time stays within
/// the slow-tier budget while `mean_iters` remains a non-trivial, real,
/// byte-identical-across-workers quantity.
const FRAMES: usize = 200;

/// Frame index at which the "SIGINT" interruption lands in the resume-parity
/// case — mid-run within the 200-frame budget, per the issue.
const INTERRUPT_AT_FRAME: usize = 100;

/// Fixed base seed for every run in this suite.
const SEED: u64 = 0xC0DE_F00D;

/// One DVB-T2 determinism config: a `(rate, modulation)` MODCOD plus the
/// decoder / demap / Es/N0 the byte-identity is asserted at.
#[derive(Clone, Copy)]
struct DetConfig {
    /// SNR-point index — selects a disjoint `SNR_STRIDE` RNG region per config
    /// so their streams never overlap (design doc §3).
    snr_idx: usize,
    rate: CodeRate,
    modulation: DvbT2Modulation,
    /// Es/N0 (dB) for the run — set just **above** the MODCOD's smoke-knee (QEF
    /// threshold) so the frames converge (a real, non-sentinel `mean_iters`)
    /// while the 200-frame batch stays within the slow-tier time budget. (Right
    /// at the knee, frames that hit the 50-iteration BP ceiling push the
    /// 1-worker leg past the 120 s/test cap; the byte-identity contract this
    /// suite proves is independent of the operating point.)
    es_n0_db: f64,
    algo: DecoderAlgorithm,
    demap: DemapMethod,
}

impl DetConfig {
    /// A human-readable label for assertion messages.
    fn label(&self) -> String {
        format!("{:?}/{:?}@{}dB", self.rate, self.modulation, self.es_n0_db)
    }

    /// The decoder configuration (hard-decision verification on).
    fn decoder(&self) -> DecoderConfig {
        DecoderConfig::new(self.algo, true)
    }
}

/// The three named DVB-T2 MODCODs from the issue, each at a fixed Es/N0 a few dB
/// above the ETSI TS 102 831 QEF anchor (so the frames converge and the
/// 200-frame batch fits the slow-tier budget — see [`DetConfig::es_n0_db`]) and
/// a distinct `snr_idx` (disjoint `SNR_STRIDE` RNG region, design doc §3).
///
/// QEF anchors (TS 102 831 Table 44): r1/2 16-QAM = 6.0 dB, r2/3 64-QAM =
/// 13.5 dB, r3/4 16-QAM = 10.0 dB. The Es/N0 used here is set above each anchor;
/// the byte-identity contract is operating-point-independent.
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

/// Builds the **production** DVB-T2 pipeline for `cfg` via the typestate preset
/// builder, returning it alongside the matching single-frame kernel.
///
/// This is the issue's `seeded_runner_factory`: it drives the real
/// [`Pipeline::dvb_t2`](gf2_sim::Pipeline::dvb_t2) typestate path
/// (`modcod → decoder → demap → channel`, then the optional `parallelism` /
/// `seed` / `checkpoint_dir` setters and `build()`), so the property test
/// exercises the production composition — the validated seven-stage BICM chain
/// and the `PipelineConfig` threading — rather than a stripped-down helper. The
/// returned [`DvbT2BicmFrameSim`] is the per-frame compute the within-SNR
/// dispatch drives (the Phase A frame kernel; the end-to-end `Pipeline` executor
/// that walks the built stage graph is Phase C, `42eac5cc`), constructed from
/// the **same** `(rate, modulation, es_n0_db, decoder, demap)` the pipeline was
/// built from so the two stay in lock-step.
///
/// # Arguments
///
/// * `cfg` — the determinism config (MODCOD + decoder/demap/Es/N0).
/// * `parallelism` — the worker count carried on the built pipeline's config.
/// * `checkpoint_dir` — the optional checkpoint directory threaded onto the
///   pipeline's config (used by the resume-parity case; `None` otherwise).
///
/// # Returns
///
/// `(pipeline, frame_sim)`: the production-built [`Pipeline`] (validated, with
/// the seed/parallelism/checkpoint_dir threaded into its
/// [`PipelineConfig`](gf2_sim::PipelineConfig)) and the matching frame kernel.
///
/// # Panics
///
/// Panics if `Pipeline::dvb_t2().….build()` returns an error — every config here
/// is one of the six in-scope DVB-T2 MODCODs with a finite Es/N0, so the build
/// always succeeds; a panic would signal a regression in the production builder.
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

    // Sanity: the production builder assembled the full seven-stage BICM chain
    // and threaded the seed onto the config — i.e. we really did ride the
    // production path, not a shortcut.
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

/// Runs a [`FRAMES`]-frame batch for `cfg` at `workers` parallelism, building
/// the **production** pipeline (validating the chain + threading the config) and
/// the matching frame kernel via [`seeded_runner_factory`], and returns the
/// aggregate [`WorkerCounters`].
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

/// Logs (but does **not** assert) the BER for one run, documenting the §11
/// always-excluded column.
///
/// BER (`total_bit_errors / total_bits`) is excluded from byte-identity (issue
/// `152388f4`; design-doc §11 "Always-excluded": a non-associative f32
/// horizontal reduction whose value depends on summation order), so it is
/// recorded for diagnostics only and never compared across worker counts.
fn record_ber(c: &WorkerCounters, label: &str, workers: usize) {
    let ber = if c.total_bits == 0 {
        0.0
    } else {
        c.total_bit_errors as f64 / c.total_bits as f64
    };
    eprintln!("{label} @ {workers} workers: BER = {ber:e} (recorded, NOT asserted)");
}

/// Asserts byte-identity of the four §11 columns across the given `workers`
/// (which must start with the 1-worker baseline) for one config, recording the
/// always-excluded BER for each.
///
/// The first element of `workers` is the byte-identity baseline (it must be
/// `1`); every subsequent count is asserted byte-identical to it via the shared
/// SSOT helper [`assert_four_columns_byte_identical`]. The worker counts are
/// split across two tests per config ([`WORKER_GROUP_NARROW`] /
/// [`WORKER_GROUP_WIDE`]) so each slow-tier test stays under the 120 s/test cap;
/// together the two groups cover all of `{1, 2, 4, 8, 24}`.
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

/// Runs `cfg` uninterrupted for the full [`FRAMES`] budget via the checkpointed
/// runner, returning the final counters. This is the parity reference for
/// [`assert_resume_parity`].
fn run_uninterrupted(cfg: DetConfig, parallelism: NonZeroUsize) -> WorkerCounters {
    let dir = tempdir();
    let (_pipeline, frame_sim) = seeded_runner_factory(cfg, parallelism, Some(dir.clone()));
    let config = checkpoint_config(parallelism, FRAMES, &dir, cfg.es_n0_db);
    let hash = config_hash(&config);
    let writer = CheckpointWriter::new(&dir).expect("create checkpoint dir");
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

/// Asserts heartbeat-resume parity for one config: a run interrupted by a
/// SIGINT at frame [`INTERRUPT_AT_FRAME`], then resumed, reproduces the same
/// final four-column tuple as the uninterrupted run.
///
/// The interruption is the **real SIGINT-flush path**, not a reduced-`max_frames`
/// chunking. A single [`FRAMES`]-frame config (one stable [`config_hash`]) drives
/// the whole scenario:
///
/// 1. The run starts fresh under the full 200-frame budget with a heartbeat at
///    frame 100. When the frame-100 heartbeat flushes its resumable
///    (`completed = false`) checkpoint, the `on_heartbeat_flush` callback trips
///    [`request_interrupt`] (the programmatic equivalent of a SIGINT). The next
///    chunk boundary observes [`is_interrupted`] and stops with
///    `interrupted = true`, `completed = false` — exactly the
///    SIGINT-at-frame-100 state.
/// 2. The interrupted checkpoint is loaded under the **same** config hash and
///    resumed under the **same** full config (no hand-mutation of `completed`).
///
/// Byte-identity holds because every frame's outcome is a pure function of its
/// global index (design doc §3), so the `0..100` + `100..200` aggregate equals
/// the single `0..200` run. Because the config (and thus `max_frames`, which
/// feeds `config_hash`) is identical across the interrupt and the resume, the
/// flushed checkpoint loads under the live hash — the contract a real SIGINT
/// resume must satisfy.
fn assert_resume_parity(cfg: DetConfig) {
    let label = cfg.label();
    // Two workers exercises the multi-worker chunked striding on resume; the
    // contract is worker-count-independent.
    let parallelism = NonZeroUsize::new(2).expect("2 is non-zero");

    // Reference: uninterrupted full run.
    let reference = run_uninterrupted(cfg, parallelism);
    assert_eq!(
        reference.frames, FRAMES as u64,
        "{label}: reference frame budget"
    );

    // Interrupted run: ONE full 200-frame config (stable config_hash), heartbeat
    // at frame 100. The frame-100 flush trips the SIGINT flag, so the next chunk
    // boundary stops with a resumable (completed = false) checkpoint on disk.
    let dir = tempdir();
    let (_pipeline, frame_sim) = seeded_runner_factory(cfg, parallelism, Some(dir.clone()));
    let writer = CheckpointWriter::new(&dir).expect("create checkpoint dir");
    let config = checkpoint_config(parallelism, FRAMES, &dir, cfg.es_n0_db);
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
        // SIGINT lands at the frame-100 heartbeat: the resumable checkpoint has
        // just hit disk, so requesting the interrupt now makes the next chunk
        // boundary stop with that checkpoint as the resume point.
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

    // Resume leg: same config + same hash. Clear the SIGINT first (else the
    // resume's first chunk-boundary check would trip it again), load the flushed
    // resumable checkpoint, and continue WITHOUT touching `completed`.
    clear_interrupt();
    let reader = CheckpointReader::new(&dir, hash.clone());
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

    // Parity: the resumed final four-column tuple equals the uninterrupted run's.
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

/// Builds a [`PipelineConfig`] for the checkpointed runner with `max_frames`
/// frames, one heartbeat chunk at frame [`INTERRUPT_AT_FRAME`] (so the
/// interrupted leg flushes exactly at the mid-run boundary), and no early stop
/// on errors (`target_errors = 0`).
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
        strict_gpu: false,
    }
}

// ===========================================================================
// Across-worker byte-identity. Each config's worker-count sweep is split into a
// NARROW group {1,2} and a WIDE group {1,4,8,24} — two #[ignore]d slow-tier
// tests per config — because a single 200-frame Normal-frame decode at 1 worker
// is ~60-70 s, so the full {1,2,4,8,24} sweep in one test exceeds the slow
// tier's 120 s/test cap. The two groups share the 1-worker baseline and between
// them cover all of {1,2,4,8,24} versus that baseline, satisfying the [hard]
// byte-identity criterion for the three named configs.
// ===========================================================================

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

// ===========================================================================
// Heartbeat-resume parity — one #[ignore]d slow-tier test per config.
// ===========================================================================

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

/// Minimal unique tempdir helper (no `tempfile` dev-dependency), mirroring the
/// checkpoint module's own test helper.
fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    let unique = format!(
        "gf2sim-det-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    p.push(unique);
    std::fs::create_dir_all(&p).expect("create unique tempdir");
    p
}
static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
