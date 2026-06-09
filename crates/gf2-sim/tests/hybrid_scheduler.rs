//! Hybrid CPU+GPU scheduler integration tests (Phase C `75c22fa8`).
//!
//! Two `#[cfg(feature = "hip")]`, GPU-gated, slow-tier suites:
//!
//! * `hybrid_gpu_cpu_overlap_exceeds_50pct` — runs the hybrid scheduler over a
//!   small frame count and asserts the recorded [`OverlapTimeline`] shows GPU
//!   stream activity overlapping CPU stage activity for > 50% of GPU-active
//!   wall-time (criterion 1 / deliverable 4).
//! * `hybrid_two_run_byte_identical` — runs the SAME hybrid path twice at a
//!   fixed seed and asserts byte-identical `fer` / `frames` / `errors` /
//!   `mean_iters` (criterion 3, the same-path determinism guarantee; since this
//!   is the same device path twice, `mean_iters` IS deterministic run-to-run).
//!
//! Both skip when no GPU is present (`device_mem_info().is_err()`) and carry
//! `#[ignore]` per the test-tier rules (each is a heavy n=64800 decode sweep).

#![cfg(feature = "hip")]

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{BatchHandle, Pipeline, Scheduler};

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
    // 8 workers, 128 frames at the r1/2 16-QAM waterfall — enough batches per
    // worker that the double-buffer steady state dominates startup.
    let pipeline = hybrid_pipeline(8, 128, 6.0);
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
    assert_eq!(results.per_point[0].frames, 128);

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

#[test]
#[ignore = "sim: hybrid two-run byte-identity (GPU-gated, n=64800 decode sweep)"]
fn hybrid_two_run_byte_identical() {
    if !gpu_present() {
        eprintln!("skipping hybrid_two_run_byte_identical: no usable GPU");
        return;
    }
    let run = || {
        let pipeline = hybrid_pipeline(4, 64, 6.0);
        pipeline.run().expect("hybrid run").per_point[0]
    };
    let a = run();
    let b = run();

    // The four contractual columns must be byte-identical run-to-run (same
    // device path twice; mean_iters IS deterministic here per §11).
    assert_eq!(a.frames, b.frames, "frames");
    assert_eq!(a.errors, b.errors, "errors (frame errors)");
    assert_eq!(a.fer.to_bits(), b.fer.to_bits(), "fer bit-pattern");
    assert_eq!(
        a.mean_iters.to_bits(),
        b.mean_iters.to_bits(),
        "mean_iters bit-pattern (same device path twice)"
    );
}
