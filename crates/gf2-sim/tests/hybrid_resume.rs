//! Hybrid CPU+GPU checkpoint/resume parity suite (Phase C task `571c11c4`,
//! design doc §4 "Drain commit contract" + 2026-06-10 amendment).
//!
//! All tests are `#[cfg(feature = "hip")]` and GPU-gated (skip without a
//! device, like the other `gpu_*` / hybrid suites):
//!
//! * `hybrid_drain_resume_smoke` — **fast tier** (NOT ignored; precedent: the
//!   C.1 `hybrid_two_run_byte_identical` fast smoke): a 2-point hybrid sweep
//!   interrupted after the first SNR point's drain+flush, resumed via
//!   [`Pipeline::run_checkpointed`]`(true)`, and asserted 4-column
//!   byte-identical against the uninterrupted hybrid `Pipeline::run()`
//!   reference. Sized (8 workers × 16 frames × 2 points) to fit the 5 s
//!   fast-tier cap on the gfx1030 host.
//! * `hybrid_resume_parity_*` — **slow tier** (`#[ignore = "sim: ..."]`), one
//!   per named DVB-T2 config (criterion 2): a SIGINT lands during a 10-SNR
//!   hybrid sweep while GPU batches are active at a named frame; the in-flight
//!   batches complete and the streams drain before the v2 checkpoint flushes
//!   (criterion 1); the resumed sweep is byte-identical in
//!   `fer / frames / errors / mean_iters` to an uninterrupted HYBRID-executor
//!   reference run at the same seed and worker count. `mean_iters` IS asserted
//!   — resume-vs-uninterrupted is a same-path comparison, so the §11
//!   CPU-vs-GPU exclusion does not apply. BER is recorded, never asserted
//!   (issue `152388f4`, §11 "Always-excluded").
//!
//! # Interrupt determinism
//!
//! The interrupt is the **programmatic SIGINT equivalent**
//! ([`request_interrupt`], the `48a0db6c` precedent), landed deterministically
//! via the [`Scheduler::run_sweep_checkpointed`] frame observer at a named
//! `(snr_idx, global_frame)`. Two interrupt shapes are covered:
//!
//! * **prep-time trip** (`g` inside the first batch's CPU prep): every worker
//!   still launches and completes its current batch — the §4 contract that
//!   in-flight work commits before the flush — and the stop lands at a
//!   deterministic batch boundary (asserted partial frame count);
//! * **overlap-time trip** (`g` inside the *next* batch's prep, which runs on
//!   the helper thread **while the current batch decodes on the GPU**): the
//!   genuinely-in-flight SIGINT. Whether a racing worker squeezes in one more
//!   batch before observing the flag is scheduler-dependent, so that case
//!   asserts the resume parity and bounds — not an exact stop frame.
//!
//! # Same-host scope
//!
//! Criterion 2 is asserted on this host (single gfx1030 in CI). The v2
//! checkpoint JSON is host-independent, but cross-host resume is documented,
//! not asserted (no second host in CI; see `executor::drain` module docs).

#![cfg(feature = "hip")]

use std::num::NonZeroUsize;
use std::path::PathBuf;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::checkpoint::{
    clear_interrupt, config_hash, request_interrupt, CheckpointReader, SweepError,
};
use gf2_sim::error::{RecoverableError, StageError};
use gf2_sim::executor::SnrPointResult;
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, Scheduler};

mod common;
use common::assert_four_columns_byte_identical;

/// Fixed base seed for every run in this suite.
const SEED: u64 = 0x571C_11C4;

/// Serializes tests that touch the process-wide checkpoint interrupt flag
/// (`request_interrupt` / `clear_interrupt`), mirroring `determinism.rs`'s
/// `RESUME_PARITY_GUARD`: bare `cargo test` runs this binary's tests
/// multi-threaded in one process, so an unguarded trip could stop a concurrent
/// test's reference run early.
static RESUME_PARITY_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// True if a usable HIP device is present.
fn gpu_present() -> bool {
    gf2_kernels_hip::host::device_mem_info().is_ok()
}

/// One hybrid-resume config: a `(rate, modulation)` MODCOD plus decoder /
/// demap, worker count, frame budget, heartbeat cadence, the 10-point Es/N0
/// ladder start, and the `(snr_idx, global_frame)` the interrupt lands at.
#[derive(Clone, Copy)]
struct ResumeConfig {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    algo: DecoderAlgorithm,
    demap: DemapMethod,
    workers: usize,
    max_frames: u64,
    heartbeat: u64,
    /// First Es/N0 (dB) of the 10-point ladder (0.2 dB steps), spanning the
    /// MODCOD's waterfall so the sweep mixes errored and clean frames.
    esn0_start: f64,
    snr_points: usize,
    /// The `(snr_idx, global_frame)` whose frame-observer event trips the
    /// programmatic SIGINT.
    interrupt_at: (usize, usize),
}

impl ResumeConfig {
    fn label(&self) -> String {
        format!("{:?}/{:?}", self.rate, self.modulation)
    }

    fn decoder(&self) -> DecoderConfig {
        DecoderConfig::new(self.algo, true)
    }

    fn esn0_points(&self) -> Vec<f64> {
        (0..self.snr_points)
            .map(|i| self.esn0_start + 0.2 * i as f64)
            .collect()
    }

    /// Builds the GPU-enabled production pipeline for this config with the
    /// given checkpoint dir threaded onto its [`PipelineConfig`].
    fn build_pipeline(&self, checkpoint_dir: Option<PathBuf>) -> Pipeline {
        let mut pipeline = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: self.rate,
                modulation: self.modulation,
            })
            .decoder(self.decoder())
            .demap(self.demap)
            .channel(Channel::awgn(self.esn0_start as f32))
            .parallelism(NonZeroUsize::new(self.workers).expect("non-zero workers"))
            .seed(SEED)
            .checkpoint_dir(checkpoint_dir)
            .with_gpu(true)
            .build()
            .expect("in-scope MODCOD builds through the production preset");
        let cfg = pipeline.config_mut();
        cfg.esn0_db_points = self.esn0_points();
        cfg.max_frames = self.max_frames;
        cfg.heartbeat_every_frames = self.heartbeat;
        cfg.target_errors = 0; // full frame budget at every point (byte-identity)
        pipeline
    }
}

/// The three named DVB-T2 configs (criterion 2), with the determinism-suite
/// algorithm/demap variety. Worker count 2 and 34 frames give each worker a
/// `[BATCH_FRAMES, tail]` partition (17 frames = batches of 16 + 1), so the
/// interrupted point stops at a real batch boundary with work remaining.
const CONFIGS: [ResumeConfig; 3] = [
    // Heartbeat 32 ⇒ rounds [0,32) and [32,34): the trip at (1, g=4) lands in
    // point 1's FIRST batch prep, so the stop is the deterministic round-1
    // heartbeat flush (frames_completed = 32).
    ResumeConfig {
        rate: CodeRate::Rate1_2,
        modulation: DvbT2Modulation::Qam16,
        algo: DecoderAlgorithm::SumProduct,
        demap: DemapMethod::ExactLogMap,
        workers: 2,
        max_frames: 34,
        heartbeat: 32,
        esn0_start: 5.8,
        snr_points: 10,
        interrupt_at: (1, 4),
    },
    // Heartbeat 0 ⇒ one round to the end; the trip at (1, g=33) fires from the
    // double-buffer helper thread while batch 0 decodes ON THE GPU — the
    // genuinely-in-flight SIGINT (overlap-time trip; see module docs).
    ResumeConfig {
        rate: CodeRate::Rate2_3,
        modulation: DvbT2Modulation::Qam64,
        algo: DecoderAlgorithm::NormalizedMinSum(0.75),
        demap: DemapMethod::ExactLogMap,
        workers: 2,
        max_frames: 34,
        heartbeat: 0,
        esn0_start: 13.2,
        snr_points: 10,
        interrupt_at: (1, 33),
    },
    ResumeConfig {
        rate: CodeRate::Rate3_4,
        modulation: DvbT2Modulation::Qam16,
        algo: DecoderAlgorithm::MinSum,
        demap: DemapMethod::MaxLog,
        workers: 2,
        max_frames: 34,
        heartbeat: 32,
        esn0_start: 9.8,
        snr_points: 10,
        interrupt_at: (1, 4),
    },
];

/// Reconstructs the SSOT [`WorkerCounters`] from a [`SnrPointResult`] so the
/// shared four-column comparison helper (`tests/common`) stays the single
/// source of truth for the byte-identity column set and the BER exclusion.
fn to_counters(p: &SnrPointResult) -> WorkerCounters {
    WorkerCounters {
        frames: p.frames,
        errors: p.errors,
        total_iterations: p.total_iterations,
        total_bits: p.total_bits,
        total_bit_errors: p.total_bit_errors,
    }
}

/// Logs (never asserts) a point's BER — the §11 always-excluded column.
fn record_ber(p: &SnrPointResult, label: &str) {
    let ber = if p.total_bits == 0 {
        0.0
    } else {
        p.total_bit_errors as f64 / p.total_bits as f64
    };
    eprintln!(
        "{label}: Es/N0 {:.1} dB frames {} errors {} mean_iters {:.4} \
         BER {ber:e} (BER recorded, NOT asserted)",
        p.es_n0_db, p.frames, p.errors, p.mean_iters
    );
}

/// Runs the full interrupted → resumed → reference scenario for one config and
/// asserts the criterion-2 four-column byte-identity per SNR point.
fn assert_hybrid_resume_parity(cfg: ResumeConfig) {
    let _guard = RESUME_PARITY_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_interrupt();

    let label = cfg.label();
    let dir = tempdir();

    // Interrupted leg: the programmatic SIGINT lands at the named
    // (snr_idx, global_frame) while GPU batches are active.
    let pipeline = cfg.build_pipeline(Some(dir.clone()));
    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "{label}: hybrid resume parity requires an active GPU stream pool"
    );
    let (trip_snr, trip_frame) = cfg.interrupt_at;
    let interrupted = scheduler
        .run_sweep_checkpointed(&pipeline, false, &|snr_idx, g| {
            if snr_idx == trip_snr && g == trip_frame {
                request_interrupt();
            }
        })
        .expect("interrupted hybrid sweep");
    assert!(
        interrupted.interrupted,
        "{label}: the SIGINT must stop the sweep early"
    );
    let reached = interrupted.results.per_point.len();
    assert!(
        reached < cfg.snr_points,
        "{label}: the sweep must stop mid-sweep, reached {reached} points"
    );
    // Point 0 completed before the trip; its SNR-boundary checkpoint is final.
    assert_eq!(
        interrupted.results.per_point[0].frames, cfg.max_frames,
        "{label}: point 0 completed before the interrupt"
    );

    // The flushed checkpoint of the interrupted point is resumable and its
    // worker_states are batch-aligned strided-partition prefixes.
    let hash = config_hash(pipeline.config());
    let reader = CheckpointReader::new(&dir, hash);
    let ck = reader
        .load(trip_snr)
        .expect("interrupted checkpoint loads")
        .expect("interrupted checkpoint was flushed");
    let ws_sum: u64 = ck.worker_states.iter().map(|w| w.frames_in_worker).sum();
    assert_eq!(
        ws_sum, ck.frames_completed,
        "{label}: hybrid worker_states must sum to frames_completed"
    );
    if cfg.heartbeat != 0 {
        // Prep-time trip (see module docs): the stop is the deterministic
        // round-1 heartbeat flush, a genuinely partial point.
        assert!(!ck.completed, "{label}: interrupted point is resumable");
        assert_eq!(
            ck.frames_completed, 32,
            "{label}: deterministic batch-boundary stop at 32 of 34 frames"
        );
        for ws in &ck.worker_states {
            assert_eq!(
                ws.frames_in_worker, 16,
                "{label}: worker {} stopped on a whole batch",
                ws.worker_idx
            );
        }
    } else {
        // Overlap-time trip: a racing worker may squeeze in its tail batch
        // before observing the flag, so bound the stop instead.
        assert!(
            (32..=cfg.max_frames).contains(&ck.frames_completed),
            "{label}: stop lands at a batch boundary in [32, {}], got {}",
            cfg.max_frames,
            ck.frames_completed
        );
    }

    // Resume leg: same dir, same config (same seed + worker count).
    clear_interrupt();
    let resumed = pipeline
        .run_checkpointed(true)
        .expect("resumed hybrid sweep");
    assert!(
        !resumed.interrupted,
        "{label}: the resumed sweep runs to completion"
    );
    assert_eq!(
        resumed.results.per_point.len(),
        cfg.snr_points,
        "{label}: the resumed sweep covers all SNR points"
    );

    // Reference: the uninterrupted HYBRID-executor run (criterion 2 names the
    // hybrid reference path — `Pipeline::run` is the C.1 scheduler).
    let reference = pipeline.run().expect("uninterrupted hybrid reference run");
    assert_eq!(reference.per_point.len(), cfg.snr_points);

    // Criterion 2: fer / frames / errors / mean_iters byte-identical per SNR
    // point (mean_iters INCLUDED: same-path comparison; §11's CPU-vs-GPU
    // exclusion does not apply). BER recorded, never asserted.
    for (idx, (res, refp)) in resumed
        .results
        .per_point
        .iter()
        .zip(reference.per_point.iter())
        .enumerate()
    {
        assert_eq!(
            res.frames, cfg.max_frames,
            "{label} point {idx}: full frame budget"
        );
        assert_four_columns_byte_identical(
            &to_counters(res),
            &to_counters(refp),
            &format!("{label} resume-parity point {idx}"),
        );
        record_ber(res, &format!("{label} point {idx}"));
    }

    // Non-vacuity: the ladder spans the waterfall, so the parity must cover
    // both errored and clean frames somewhere in the sweep.
    let total_errors: u64 = reference.per_point.iter().map(|p| p.errors).sum();
    let total_frames: u64 = reference.per_point.iter().map(|p| p.frames).sum();
    assert!(
        total_errors > 0 && total_errors < total_frames,
        "{label}: expected a mixed errored/clean sweep across the waterfall \
         ladder, got {total_errors}/{total_frames}"
    );
}

// ===========================================================================
// Fast-tier smoke (NOT ignored; GPU-gated). Covers drain + resume on a small
// frame count so the green gate exercises the path (de160fc5 precedent).
// ===========================================================================

/// Drain + resume smoke: a 2-point hybrid sweep (8 workers × 16 frames per
/// point, r1/2 16-QAM at the waterfall) interrupted after point 0's
/// drain+flush, resumed, and asserted 4-column byte-identical to the
/// uninterrupted hybrid `Pipeline::run()` reference. Every flush in the run —
/// including point 0's SNR-boundary flush — goes through
/// `Scheduler::drain_for_checkpoint` (per-stream sync + in-flight tally), so
/// the fast tier exercises the §4 drain commit path end-to-end.
#[test]
fn hybrid_drain_resume_smoke() {
    if !gpu_present() {
        eprintln!("skipping hybrid_drain_resume_smoke: no usable GPU");
        return;
    }
    let _guard = RESUME_PARITY_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_interrupt();

    let smoke = ResumeConfig {
        rate: CodeRate::Rate1_2,
        modulation: DvbT2Modulation::Qam16,
        algo: DecoderAlgorithm::SumProduct,
        demap: DemapMethod::MaxLog,
        workers: 8,
        max_frames: 16,
        heartbeat: 0,
        esn0_start: 6.0,
        snr_points: 2,
        // Trip during point 0's batch prep: point 0 still completes (every
        // in-flight batch commits before the flush, §4), and the sweep stops
        // at point 1's entry with point 0's final checkpoint on disk.
        interrupt_at: (0, 9),
    };
    let dir = tempdir();
    let pipeline = smoke.build_pipeline(Some(dir.clone()));
    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "smoke requires an active GPU stream pool"
    );

    let interrupted = scheduler
        .run_sweep_checkpointed(&pipeline, false, &|snr_idx, g| {
            if (snr_idx, g) == smoke.interrupt_at {
                request_interrupt();
            }
        })
        .expect("interrupted hybrid sweep");
    assert!(interrupted.interrupted, "the SIGINT must stop the sweep");

    // Point 0's checkpoint: complete, with the hybrid strided-partition
    // worker_states latched after the drain (8 workers x 2 frames each).
    let hash = config_hash(pipeline.config());
    let ck = CheckpointReader::new(&dir, hash)
        .load(0)
        .expect("point-0 checkpoint loads")
        .expect("point-0 checkpoint exists");
    assert!(ck.completed, "point 0 completed before the interrupt");
    assert_eq!(ck.frames_completed, 16);
    assert_eq!(ck.worker_states.len(), smoke.workers);
    for ws in &ck.worker_states {
        assert_eq!(
            ws.frames_in_worker, 2,
            "worker {}: strided partition of 16 frames over 8 workers",
            ws.worker_idx
        );
    }

    // Resume folds point 0 from its checkpoint and runs point 1 fresh.
    clear_interrupt();
    let resumed = pipeline.run_checkpointed(true).expect("resumed sweep");
    assert!(!resumed.interrupted);
    assert_eq!(resumed.results.per_point.len(), 2);

    // 4-column byte-identity vs the uninterrupted hybrid reference.
    let reference = pipeline.run().expect("hybrid reference run");
    for (idx, (res, refp)) in resumed
        .results
        .per_point
        .iter()
        .zip(reference.per_point.iter())
        .enumerate()
    {
        assert_eq!(res.frames, 16, "point {idx}: full frame budget");
        assert_four_columns_byte_identical(
            &to_counters(res),
            &to_counters(refp),
            &format!("smoke resume-parity point {idx}"),
        );
        record_ber(res, &format!("smoke point {idx}"));
    }
}

// ===========================================================================
// Fast-tier restore-branch leg (NOT ignored; GPU-gated). Exercises the
// `frames_in_worker` restore + worker-state validation path that the
// `hybrid_drain_resume_smoke` test does NOT reach (it takes the
// `ck.completed` early-return at point 0). This test writes a real partial
// v2 checkpoint via the production machinery (interrupted at a genuine
// batch boundary with `done=[16,16]` of 64 frames), then resumes and
// asserts that (a) the restore engaged (the resumed run processed exactly
// 32 remaining frames), (b) 4-column byte-identity holds vs the
// uninterrupted hybrid reference, and (c) the batch-alignment validation
// accepts the checkpoint.
// ===========================================================================

/// Restore-branch leg: a 1-point hybrid sweep (2 workers × 33 frames,
/// r1/2 16-QAM at the waterfall, heartbeat=1) interrupted at global frame
/// 4 (prep-time trip, stops at the deterministic round-1 boundary with
/// `done=[16,16]`, 32 frames committed). Resume processes the remaining
/// 1 frame (worker 0's partition tail) and is asserted 4-column
/// byte-identical to the uninterrupted hybrid reference.
///
/// Config sizing: 2 workers × 33 frames. Worker 0 owns {0,2,…,32} = 17
/// frames; worker 1 owns {1,3,…,31} = 16 frames. Heartbeat = 1 ⟹
/// `round_frames = ceil(1 / (16×2)).max(1) × 16 × 2 = 32`. Round
/// boundaries are at 32. Trip at (snr=0, g=4) lands in round-1's first
/// batch prep: the round-1 flush commits done=[16,16] = 32 frames, and the
/// remaining 1 frame (worker 0 at index j=16, global g=32) runs on resume.
///
/// This is the HIGH-1(a) test: the fast tier exercises the
/// `frames_in_worker` restore code path, the batch-alignment validation,
/// and the remaining-frames assertion. Sized to stay well inside the 5 s
/// fast-tier cap (~33 × 3 = ~99 GPU LDPC decodes).
#[test]
fn hybrid_partial_point_restore() {
    if !gpu_present() {
        eprintln!("skipping hybrid_partial_point_restore: no usable GPU");
        return;
    }
    let _guard = RESUME_PARITY_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_interrupt();

    // 2 workers × 33 frames, heartbeat=1 ⟹ round_frames=32.
    // Trip at (snr=0, g=4) stops at the round-1 boundary: done=[16,16]=32.
    let restore_cfg = ResumeConfig {
        rate: CodeRate::Rate1_2,
        modulation: DvbT2Modulation::Qam16,
        algo: DecoderAlgorithm::SumProduct,
        demap: DemapMethod::MaxLog,
        workers: 2,
        max_frames: 33,
        heartbeat: 1,
        esn0_start: 6.0,
        snr_points: 1,
        interrupt_at: (0, 4),
    };
    let dir = tempdir();
    let pipeline = restore_cfg.build_pipeline(Some(dir.clone()));
    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "partial restore test requires an active GPU stream pool"
    );

    // Interrupted leg: trip at (snr=0, g=4) stops at the round-1 boundary.
    let (trip_snr, trip_frame) = restore_cfg.interrupt_at;
    let interrupted = scheduler
        .run_sweep_checkpointed(&pipeline, false, &|snr_idx, g| {
            if snr_idx == trip_snr && g == trip_frame {
                request_interrupt();
            }
        })
        .expect("interrupted hybrid sweep");
    assert!(interrupted.interrupted, "SIGINT must stop the sweep");

    // Verify the partial checkpoint: not complete, done=[16,16], sum=32.
    let hash = config_hash(pipeline.config());
    let ck = CheckpointReader::new(&dir, hash)
        .load(0)
        .expect("checkpoint loads")
        .expect("checkpoint was flushed");
    assert!(
        !ck.completed,
        "partial-point checkpoint must not be complete"
    );
    assert_eq!(
        ck.frames_completed, 32,
        "deterministic round-1 heartbeat flush stopped at 32 of 33 frames"
    );
    assert_eq!(ck.worker_states.len(), 2, "2 workers");
    for ws in &ck.worker_states {
        assert_eq!(
            ws.frames_in_worker, 16,
            "worker {} stopped on a whole BATCH_FRAMES batch",
            ws.worker_idx
        );
    }

    // Resume: processes only the remaining 1 frame (RESTORE branch).
    clear_interrupt();
    let frames_seen = std::sync::atomic::AtomicU64::new(0);
    let resumed_sweep = scheduler
        .run_sweep_checkpointed(&pipeline, true, &|_snr, _g| {
            frames_seen.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // Relaxed store: value read after the closure returns at the barrier.
        })
        .expect("resumed hybrid sweep");
    assert!(
        !resumed_sweep.interrupted,
        "resumed sweep runs to completion"
    );
    assert_eq!(resumed_sweep.results.per_point.len(), 1, "one SNR point");
    let resumed = &resumed_sweep.results.per_point[0];
    assert_eq!(
        resumed.frames, 33,
        "resumed run must accumulate the full 33 frames"
    );
    // The observer fires only for the 1 remaining frame in the resumed run
    // (the first 32 are loaded from the checkpoint, not re-processed).
    let seen = frames_seen.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        seen, 1,
        "restore branch must process exactly the 1 remaining frame; saw {seen}"
    );

    // 4-column byte-identity vs the uninterrupted hybrid reference.
    let reference = pipeline.run().expect("uninterrupted hybrid reference run");
    assert_eq!(reference.per_point.len(), 1);
    let refp = &reference.per_point[0];
    assert_four_columns_byte_identical(
        &to_counters(resumed),
        &to_counters(refp),
        "partial-restore resume-parity point 0",
    );
    record_ber(resumed, "partial-restore point 0");
}

// ===========================================================================
// Fast-tier failure-semantics leg (NOT ignored; GPU-gated). Proves the
// explicit OPTION (a) decision of epic task bb11c2e6: a recoverable GPU fault
// during a CHECKPOINTED sweep aborts RESUMABLY — the drain loop propagates the
// fault (rather than substituting the CPU fallback like the uncheckpointed
// scheduler), the last committed heartbeat checkpoint survives, and a
// subsequent resume completes byte-identically vs an uninterrupted reference.
// ===========================================================================

/// **bb11c2e6 failure-semantics leg (fast tier, GPU-gated, NOT ignored).**
///
/// The checkpointed sweep's recoverable-GPU-fault behaviour is OPTION (a) —
/// abort resumably, not deterministic-fallback-in-sweep. This drives it as a
/// run:
///
/// 1. **Commit a checkpoint.** A 2-worker × 34-frame, heartbeat-32 sweep is
///    interrupted in round 1's first-batch prep (trip at `g=4`). The round-1
///    heartbeat flush commits `done=[16,16]` = 32 frames — the "last committed
///    round" requirement 2 names. No injection here (clean baseline).
/// 2. **Fault on resume.** The point is resumed with
///    `inject_gpu_oom_modulus = Some(32)` — excluded from
///    [`config_hash`](gf2_sim::checkpoint::config_hash), so the same checkpoint
///    loads. Round 2 = `[32, 34)`; worker 0's round-2 batch starts at global
///    frame 32 (`32 % 32 == 0`), so the drain hook injects a recoverable OOM
///    and **propagates** it. The sweep returns
///    `Err(SweepError::Stage(StageError::Recoverable(OutOfMemory)))`
///    (requirement 1), and because the fault aborts the round *before* its
///    drain/flush, the 32-frame checkpoint from step 1 is still on disk
///    (requirement 2, re-asserted).
/// 3. **Resume to completion, byte-identically.** Injection removed, the point
///    is resumed once more; it folds the 32-frame checkpoint, runs round 2
///    clean (the 2-frame `[32,34)` tail), and is asserted 4-column
///    byte-identical to the uninterrupted hybrid `Pipeline::run()` reference
///    (requirement 3).
///
/// `g=0` is a multiple of every modulus, so a modulus-keyed injection always
/// faults the very first batch — hence the baseline checkpoint must be built
/// WITHOUT injection (step 1) and injection enabled only on the resume
/// (step 2), where round 2's first batch (`g=32`) is the earliest injected one.
///
/// Timing: ~32 (step 1) + 1 (faulted) + 2 (step-3 tail) + 34 (reference) GPU
/// LDPC decodes, well inside the 5 s fast-tier cap. Skips with no GPU.
#[test]
fn hybrid_checkpointed_recoverable_fault_aborts_resumably() {
    if !gpu_present() {
        eprintln!("skipping hybrid_checkpointed_recoverable_fault_aborts_resumably: no usable GPU");
        return;
    }
    let _guard = RESUME_PARITY_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_interrupt();

    // 2 workers × 34 frames, heartbeat=32 ⟹ round_frames=32: rounds [0,32),
    // [32,34). Worker 0's round-2 batch first global frame is 32.
    let cfg = ResumeConfig {
        rate: CodeRate::Rate1_2,
        modulation: DvbT2Modulation::Qam16,
        algo: DecoderAlgorithm::SumProduct,
        demap: DemapMethod::MaxLog,
        workers: 2,
        max_frames: 34,
        heartbeat: 32,
        esn0_start: 6.0,
        snr_points: 1,
        interrupt_at: (0, 4), // prep-time trip → deterministic round-1 stop at 32
    };
    let dir = tempdir();

    // Step 1: clean interrupted run commits a 32-frame round-1 checkpoint.
    let pipeline = cfg.build_pipeline(Some(dir.clone()));
    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "failure-semantics leg requires an active GPU stream pool"
    );
    let (trip_snr, trip_frame) = cfg.interrupt_at;
    let interrupted = scheduler
        .run_sweep_checkpointed(&pipeline, false, &|snr_idx, g| {
            if snr_idx == trip_snr && g == trip_frame {
                request_interrupt();
            }
        })
        .expect("step 1 interrupted sweep");
    assert!(interrupted.interrupted, "the SIGINT must stop the sweep");

    let hash = config_hash(pipeline.config());
    let ck = CheckpointReader::new(&dir, hash.clone())
        .load(0)
        .expect("step-1 checkpoint loads")
        .expect("step-1 checkpoint was flushed");
    assert!(!ck.completed, "the round-1 checkpoint is resumable");
    assert_eq!(
        ck.frames_completed, 32,
        "deterministic round-1 stop at 32 of 34 frames"
    );

    // Step 2: resume WITH OOM injection (modulus 32, not in config_hash). The
    // round-2 batch at g=32 injects a recoverable OOM, which the drain loop
    // PROPAGATES — the sweep aborts (requirement 1) without committing a new
    // checkpoint, so the 32-frame checkpoint survives (requirement 2).
    clear_interrupt();
    let mut faulting = cfg.build_pipeline(Some(dir.clone()));
    faulting.config_mut().inject_gpu_oom_modulus = Some(32);
    let faulting_sched = Scheduler::from_pipeline(&faulting);
    let err = faulting_sched
        .run_sweep_checkpointed(&faulting, true, &|_, _| {})
        .expect_err("the injected recoverable OOM must abort the checkpointed sweep");
    match err {
        SweepError::Stage(StageError::Recoverable(RecoverableError::OutOfMemory { .. })) => {
            // Requirement 1: the recoverable fault is PROPAGATED, not
            // substituted by the CPU fallback (the OPTION (a) decision).
        }
        other => panic!(
            "expected SweepError::Stage(Recoverable(OutOfMemory)) from the propagated \
             injected fault, got {other:?}"
        ),
    }

    // Requirement 2 (re-assert): the last committed checkpoint is intact and
    // still resumable — the faulted round committed nothing.
    let ck_after = CheckpointReader::new(&dir, hash.clone())
        .load(0)
        .expect("checkpoint still loads after the faulted resume")
        .expect("the committed checkpoint survives the abort");
    assert!(
        !ck_after.completed,
        "the surviving checkpoint is still resumable"
    );
    assert_eq!(
        ck_after.frames_completed, 32,
        "the faulted round committed nothing; the 32-frame checkpoint is intact"
    );

    // Step 3: resume WITHOUT injection → completes byte-identically vs the
    // uninterrupted hybrid reference (requirement 3).
    clear_interrupt();
    let resumed = pipeline
        .run_checkpointed(true)
        .expect("step-3 resumed sweep");
    assert!(!resumed.interrupted, "the resumed sweep runs to completion");
    assert_eq!(resumed.results.per_point.len(), 1, "one SNR point");
    let resumed_pt = &resumed.results.per_point[0];
    assert_eq!(
        resumed_pt.frames, 34,
        "the resumed run accumulates the full frame budget"
    );

    let reference = pipeline.run().expect("uninterrupted hybrid reference run");
    assert_eq!(reference.per_point.len(), 1);
    assert_four_columns_byte_identical(
        &to_counters(resumed_pt),
        &to_counters(&reference.per_point[0]),
        "bb11c2e6 fault-then-resume parity point 0",
    );
    record_ber(resumed_pt, "bb11c2e6 fault-then-resume point 0");
}

// ===========================================================================
// Slow-tier criterion-2 parity, one #[ignore]d test per named config
// (10-SNR hybrid sweep x interrupted + resumed + reference legs).
// ===========================================================================

#[test]
#[ignore = "sim: hybrid SIGINT-resume parity, 10-SNR GPU sweep — r1/2 16-QAM SumProduct/ExactLogMap"]
fn hybrid_resume_parity_r1_2_16qam() {
    if !gpu_present() {
        eprintln!("skipping hybrid_resume_parity_r1_2_16qam: no usable GPU");
        return;
    }
    assert_hybrid_resume_parity(CONFIGS[0]);
}

#[test]
#[ignore = "sim: hybrid SIGINT-resume parity, 10-SNR GPU sweep — r2/3 64-QAM NMS(0.75)/ExactLogMap"]
fn hybrid_resume_parity_r2_3_64qam() {
    if !gpu_present() {
        eprintln!("skipping hybrid_resume_parity_r2_3_64qam: no usable GPU");
        return;
    }
    assert_hybrid_resume_parity(CONFIGS[1]);
}

#[test]
#[ignore = "sim: hybrid SIGINT-resume parity, 10-SNR GPU sweep — r3/4 16-QAM MinSum/MaxLog"]
fn hybrid_resume_parity_r3_4_16qam() {
    if !gpu_present() {
        eprintln!("skipping hybrid_resume_parity_r3_4_16qam: no usable GPU");
        return;
    }
    assert_hybrid_resume_parity(CONFIGS[2]);
}

/// Creates a unique tempdir for this test file via the shared `common::tempdir`
/// helper (L8: eliminates the duplicated helper from this file).
///
/// The `determinism.rs` copy is NOT pointed here: its prefix (`gf2sim-det-`)
/// and `COUNTER` are local to that binary; a mechanical swap would silently
/// rename every logged path in that suite. Leave it in place per the L8 rule.
fn tempdir() -> PathBuf {
    common::tempdir("hybres")
}
