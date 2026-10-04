//! Hybrid CPU+GPU scheduler tests: GPU/CPU activity overlap and run-to-run
//! determinism of the hybrid path. Both skip when no HIP device is present.

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

fn gpu_present() -> bool {
    gf2_kernels_hip::host::device_mem_info().is_ok()
}

fn hybrid_pipeline(workers: usize, max_frames: u64, es_n0_db: f32) -> Pipeline {
    let mut p = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        // The demap stage runs on the GPU only under MaxLog.
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
    // 48 frames per worker = 3 batches of BATCH_FRAMES = 16, so each worker
    // preps batch N+1 while the GPU decodes batch N; a single batch per worker
    // leaves no overlap to measure.
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
fn hybrid_two_run_byte_identical() {
    if !gpu_present() {
        eprintln!("skipping hybrid_two_run_byte_identical: no usable GPU");
        return;
    }
    let pipeline = hybrid_pipeline(8, 32, 6.0);
    let run = || pipeline.run().expect("hybrid run").per_point[0];
    let a = run();
    let b = run();

    assert!(
        a.errors > 0 && a.errors < a.frames,
        "expected a mixed decode-success/failure sweep at the waterfall, got \
         {}/{} errored frames",
        a.errors,
        a.frames
    );

    assert_four_columns_byte_identical(
        &snr_point_to_counters(&b),
        &snr_point_to_counters(&a),
        "hybrid run-to-run (same device path twice)",
    );
}
