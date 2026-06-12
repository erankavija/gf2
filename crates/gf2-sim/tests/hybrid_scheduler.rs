//! Hybrid CPU+GPU scheduler integration tests (Phase C `75c22fa8`).
//!
//! Two `#[cfg(feature = "hip")]`, GPU-gated suites:
//!
//! * `hybrid_gpu_cpu_overlap_exceeds_50pct` — slow tier (`#[ignore]`): runs the
//!   hybrid scheduler over a frame count large enough for the double-buffer
//!   steady state and asserts the recorded [`OverlapTimeline`] shows GPU
//!   stream activity overlapping CPU stage activity for > 50% of GPU-active
//!   wall-time (criterion 1 / deliverable 4).
//! * `hybrid_two_run_byte_identical` — fast tier (NOT ignored: criterion 3
//!   names the literal command `cargo test -p gf2-sim --features hip`, so this
//!   test must run under it): runs the SAME hybrid path twice at a fixed seed
//!   and asserts byte-identical `fer` / `frames` / `errors` / `mean_iters`
//!   (the same-path determinism guarantee; since this is the same device path
//!   twice, `mean_iters` IS deterministic run-to-run). Sized (8 workers x 32
//!   frames per run) to fit the 5 s fast-tier cap on the gfx1030 host.
//!
//! Both skip when no GPU is present (`device_mem_info().is_err()`).

#![cfg(feature = "hip")]

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{BatchHandle, Pipeline, Scheduler};

mod common;
use common::{assert_four_columns_byte_identical, snr_point_to_counters};

/// True if a usable HIP device is present.
fn gpu_present() -> bool {
    gf2_kernels_hip::host::device_mem_info().is_ok()
}

/// Builds a GPU-enabled DVB-T2 r1/2 16-QAM pipeline at a waterfall Es/N0, sized
/// for `max_frames` frames across `workers` workers.
fn hybrid_pipeline(workers: usize, max_frames: u64, es_n0_db: f32) -> Pipeline {
    let mut p = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        // MaxLog so the GPU demap path is available (the GPU LDPC decode is the
        // overlapped heavy stage regardless).
        .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
        .demap(DemapMethod::MaxLog)
        .channel(Channel::awgn(es_n0_db))
        .parallelism(NonZeroUsize::new(workers).unwrap())
        .seed(0x75C2_2FA8)
        .with_gpu(true)
        .build()
        .expect("in-scope MODCOD builds");
    p.config_mut().esn0_db_points = vec![es_n0_db as f64];
    p.config_mut().max_frames = max_frames;
    p
}

#[test]
#[ignore = "sim: hybrid CPU+GPU overlap smoke (GPU-gated, n=64800 decode sweep)"]
fn hybrid_gpu_cpu_overlap_exceeds_50pct() {
    if !gpu_present() {
        eprintln!("skipping hybrid_gpu_cpu_overlap_exceeds_50pct: no usable GPU");
        return;
    }
    // 8 workers, 384 frames at the r1/2 16-QAM waterfall: 48 frames per worker
    // = 3 batches of BATCH_FRAMES=16, so each worker genuinely double-buffers
    // (prep of batch N+1 overlapping the stream-ordered GPU decode of batch N).
    // With REAL per-worker stream semantics this intra-worker overlap is what
    // the criterion measures — a single-batch-per-worker config would leave no
    // batch N+1 to prep during the decode and trivially under-report.
    let pipeline = hybrid_pipeline(8, 384, 6.0);
    let scheduler = Scheduler::from_pipeline(&pipeline);
    assert!(
        scheduler.gpu_active(),
        "hybrid scheduler must have an active GPU stream pool on a GPU host"
    );
    let handle = BatchHandle::new(0, 0);
    let (results, timeline) = scheduler
        .run_instrumented(&pipeline, handle)
        .expect("hybrid run");

    assert_eq!(results.per_point.len(), 1);
    assert_eq!(results.per_point[0].frames, 384);

    // GPU and CPU intervals must both have been recorded.
    let has_gpu = timeline
        .intervals
        .iter()
        .any(|iv| iv.kind == gf2_sim::ActivityKind::GpuDecode);
    let has_cpu = timeline
        .intervals
        .iter()
        .any(|iv| iv.kind == gf2_sim::ActivityKind::CpuPrep);
    assert!(
        has_gpu && has_cpu,
        "both GPU and CPU activity must be recorded"
    );

    let overlap = timeline.gpu_overlap_fraction();
    eprintln!(
        "hybrid GPU/CPU overlap = {:.1}% over {} intervals",
        overlap * 100.0,
        timeline.intervals.len()
    );
    assert!(
        overlap > 0.5,
        "GPU activity must overlap CPU activity > 50% of GPU-active wall-time \
         (no serial-only gaps); got {:.1}%",
        overlap * 100.0
    );
}

/// Criterion 3: `cargo test -p gf2-sim --features hip` run twice produces
/// byte-identical `fer` / `frames` / `errors` / `mean_iters` at a fixed seed.
/// NOT `#[ignore]`d — the criterion names that literal command, so this test
/// must execute under it. GPU-gated (skips without a device); sized to fit the
/// 5 s fast-tier per-test cap on the gfx1030 host (8 workers, 32 frames per
/// run at the r1/2 16-QAM waterfall — non-vacuous: a mixed
/// decode-success/failure verdict is asserted below).
#[test]
fn hybrid_two_run_byte_identical() {
    if !gpu_present() {
        eprintln!("skipping hybrid_two_run_byte_identical: no usable GPU");
        return;
    }
    // One build, two runs: `Pipeline::run` takes `&self` and reseeds from the
    // config, so each run IS the same hybrid device path end-to-end.
    let pipeline = hybrid_pipeline(8, 32, 6.0);
    let run = || pipeline.run().expect("hybrid run").per_point[0];
    let a = run();
    let b = run();

    // Non-vacuous at the waterfall: some — not all — frames must error, so the
    // determinism assertion exercises a genuine mixed-verdict sweep.
    assert!(
        a.errors > 0 && a.errors < a.frames,
        "expected a mixed decode-success/failure sweep at the waterfall, got \
         {}/{} errored frames",
        a.errors,
        a.frames
    );

    // The four contractual columns must be byte-identical run-to-run (same
    // device path twice; mean_iters IS deterministic here per §11), via the
    // shared SSOT comparator over the adapted counters.
    assert_four_columns_byte_identical(
        &snr_point_to_counters(&b),
        &snr_point_to_counters(&a),
        "hybrid run-to-run (same device path twice)",
    );
}
