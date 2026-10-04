//! Hybrid CPU+GPU checkpoint/resume parity: a sweep interrupted while GPU
//! batches are active drains, flushes a checkpoint, and resumes with `fer`,
//! `frames`, `errors` and `mean_iters` byte-identical to an uninterrupted
//! hybrid run. The interrupt is `request_interrupt`, raised from the
//! `Scheduler::run_sweep_checkpointed` frame observer at a fixed
//! `(snr_idx, global_frame)`. Every test skips when no HIP device is present.

#![cfg(feature = "hip")]

use std::num::NonZeroUsize;
use std::path::PathBuf;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::error::{RecoverableError, StageError};
use gf2_sim::executor::SnrPointResult;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::snr_checkpoint::{
    clear_interrupt, config_hash, request_interrupt, CheckpointReader, SweepError,
};
use gf2_sim::{Pipeline, Scheduler};

mod common;
use common::assert_four_columns_byte_identical;
use common::snr_point_to_counters as to_counters;

const SEED: u64 = 0x571C_11C4;

/// Serializes tests that touch the process-wide checkpoint interrupt flag.
static RESUME_PARITY_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn gpu_present() -> bool {
    gf2_kernels_hip::host::device_mem_info().is_ok()
}

#[derive(Clone, Copy)]
struct ResumeConfig {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    algo: DecoderAlgorithm,
    demap: DemapMethod,
    workers: usize,
    max_frames: u64,
    heartbeat: u64,
    /// First Es/N0 (dB) of the 0.2 dB ladder, chosen so the sweep spans the
    /// MODCOD's waterfall.
    esn0_start: f64,
    snr_points: usize,
    /// The `(snr_idx, global_frame)` at which the frame observer interrupts.
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

/// Two workers and 34 frames give each worker a 17-frame partition, one
/// `BATCH_FRAMES` batch plus a 1-frame tail, so the interrupted point stops at
/// a batch boundary with work remaining.
const CONFIGS: [ResumeConfig; 3] = [
    // Heartbeat 32 gives rounds [0,32) and [32,34); the trip at (1, 4) lands
    // in point 1's first batch prep, so the stop is the round-1 flush at 32.
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
    // Heartbeat 0 gives one round; the trip at (1, 33) fires from the helper
    // thread prepping the tail while batch 0 decodes on the GPU.
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

fn assert_hybrid_resume_parity(cfg: ResumeConfig) {
    let _guard = RESUME_PARITY_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_interrupt();

    let label = cfg.label();
    let dir = tempdir();

    let pipeline = cfg.build_pipeline(Some(dir.path().to_path_buf()));
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
    assert_eq!(
        interrupted.results.per_point[0].frames, cfg.max_frames,
        "{label}: point 0 completed before the interrupt"
    );

    let hash = config_hash(pipeline.config());
    let reader = CheckpointReader::new(dir.path(), hash);
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
        // A racing worker may finish its tail batch before observing the flag.
        assert!(
            (32..=cfg.max_frames).contains(&ck.frames_completed),
            "{label}: stop lands at a batch boundary in [32, {}], got {}",
            cfg.max_frames,
            ck.frames_completed
        );
    }

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

    let reference = pipeline.run().expect("uninterrupted hybrid reference run");
    assert_eq!(reference.per_point.len(), cfg.snr_points);

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

    let total_errors: u64 = reference.per_point.iter().map(|p| p.errors).sum();
    let total_frames: u64 = reference.per_point.iter().map(|p| p.frames).sum();
    assert!(
        total_errors > 0 && total_errors < total_frames,
        "{label}: expected a mixed errored/clean sweep across the waterfall \
         ladder, got {total_errors}/{total_frames}"
    );
}

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
        // In-flight batches commit before the flush, so point 0 completes and
        // the sweep stops at point 1's entry.
        interrupt_at: (0, 9),
    };
    let dir = tempdir();
    let pipeline = smoke.build_pipeline(Some(dir.to_path_buf()));
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

    let hash = config_hash(pipeline.config());
    let ck = CheckpointReader::new(dir.path(), hash)
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

    clear_interrupt();
    let resumed = pipeline.run_checkpointed(true).expect("resumed sweep");
    assert!(!resumed.interrupted);
    assert_eq!(resumed.results.per_point.len(), 2);

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

/// Worker 0 owns frames {0,2,…,32} (17), worker 1 owns {1,3,…,31} (16).
/// Heartbeat 1 rounds up to one 32-frame round (one batch per worker), so the
/// trip at frame 4 commits `done=[16,16]` and resume runs only frame 32.
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
    let pipeline = restore_cfg.build_pipeline(Some(dir.to_path_buf()));
    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "partial restore test requires an active GPU stream pool"
    );

    let (trip_snr, trip_frame) = restore_cfg.interrupt_at;
    let interrupted = scheduler
        .run_sweep_checkpointed(&pipeline, false, &|snr_idx, g| {
            if snr_idx == trip_snr && g == trip_frame {
                request_interrupt();
            }
        })
        .expect("interrupted hybrid sweep");
    assert!(interrupted.interrupted, "SIGINT must stop the sweep");

    let hash = config_hash(pipeline.config());
    let ck = CheckpointReader::new(dir.path(), hash)
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
    let seen = frames_seen.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        seen, 1,
        "restore branch must process exactly the 1 remaining frame; saw {seen}"
    );

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

/// A recoverable GPU fault in a checkpointed sweep propagates instead of
/// taking the CPU fallback, and the last committed checkpoint stays
/// resumable. `inject_gpu_oom_modulus` is excluded from `config_hash`, so the
/// faulting resume loads the same checkpoint; frame 0 is a multiple of every
/// modulus, so the checkpoint is built without injection and the first
/// injected batch starts at frame 32.
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

    let pipeline = cfg.build_pipeline(Some(dir.to_path_buf()));
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
    let ck = CheckpointReader::new(dir.path(), hash.clone())
        .load(0)
        .expect("step-1 checkpoint loads")
        .expect("step-1 checkpoint was flushed");
    assert!(!ck.completed, "the round-1 checkpoint is resumable");
    assert_eq!(
        ck.frames_completed, 32,
        "deterministic round-1 stop at 32 of 34 frames"
    );

    clear_interrupt();
    let mut faulting = cfg.build_pipeline(Some(dir.to_path_buf()));
    faulting.config_mut().inject_gpu_oom_modulus = Some(32);
    let faulting_sched = Scheduler::from_pipeline(&faulting);
    let err = faulting_sched
        .run_sweep_checkpointed(&faulting, true, &|_, _| {})
        .expect_err("the injected recoverable OOM must abort the checkpointed sweep");
    match err {
        SweepError::Stage(StageError::Recoverable(RecoverableError::OutOfMemory { .. })) => {}
        other => panic!(
            "expected SweepError::Stage(Recoverable(OutOfMemory)) from the propagated \
             injected fault, got {other:?}"
        ),
    }

    let ck_after = CheckpointReader::new(dir.path(), hash.clone())
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

fn tempdir() -> gf2_core::test_scratch::Scratch {
    common::tempdir("hybres")
}
