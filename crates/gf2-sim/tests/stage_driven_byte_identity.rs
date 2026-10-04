//! The stage-driven DVB-T2 chain (`TopologyExecutor::run_dvb_t2_snr_point`)
//! matches `run_snr_point` over the `DvbT2BicmFrameSim` frame kernel: `fer` /
//! `frames` / `errors` / `mean_iters` are byte-identical on the CPU-only chain,
//! and `fer` / `frames` / `errors` with the GPU LDPC stage, where `mean_iters`
//! is logged only.

mod common;

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

#[cfg(feature = "hip")]
use common::assert_three_columns_byte_identical_log_mean_iters;

const SEED: u64 = 0xDE16_0FC5;

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

/// `es_n0_db` must be `f32`-representable so the preset and the frame kernel
/// derive bit-identical sigma / N0.
fn build_pipeline_seeded(es_n0_db: f32, workers: usize, gpu: bool, seed: u64) -> Pipeline {
    Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0_db))
        .parallelism(NonZeroUsize::new(workers).unwrap())
        .seed(seed)
        .with_gpu(gpu)
        .build()
        .expect("in-scope MODCOD builds")
}

fn build_pipeline(es_n0_db: f32, workers: usize, gpu: bool) -> Pipeline {
    build_pipeline_seeded(es_n0_db, workers, gpu, SEED)
}

fn ssot_counters_seeded(es_n0_db: f64, frames: usize, workers: usize, seed: u64) -> WorkerCounters {
    let template = DvbT2BicmFrameSim::new(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        es_n0_db,
        decoder_config(),
        DemapMethod::ExactLogMap,
    );
    run_snr_point(
        seed,
        0,
        frames,
        NonZeroUsize::new(workers).unwrap(),
        || template.clone(),
        |g, ctx, sim| sim.simulate_frame(g, ctx),
    )
}

fn ssot_counters(es_n0_db: f64, frames: usize, workers: usize) -> WorkerCounters {
    ssot_counters_seeded(es_n0_db, frames, workers, SEED)
}

#[test]
fn test_stage_driven_cpu_smoke_matches_ssot_4_columns() {
    let es_n0 = 9.0_f32;
    let frames = 2usize;
    let pipeline = build_pipeline(es_n0, 2, false);
    let scheduler = Scheduler::from_pipeline(&pipeline);

    let staged = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, frames)
        .expect("stage-driven sweep runs");
    let ssot = ssot_counters(f64::from(es_n0), frames, 2);

    assert_eq!(staged.frames, frames as u64);
    common::assert_four_columns_byte_identical(&staged, &ssot, "stage-driven CPU smoke @9dB");
    assert_eq!(staged.errors, 0, "9 dB is above the r1/2 16-QAM waterfall");
}

#[test]
#[ignore = "sim: 32-frame waterfall sweep, staged arm decodes serialise on the shared codec"]
fn test_stage_driven_cpu_waterfall_matches_ssot_4_columns() {
    let es_n0 = 6.0_f32;
    let frames = 32usize;
    let pipeline = build_pipeline(es_n0, 4, false);
    let scheduler = Scheduler::from_pipeline(&pipeline);

    let staged = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, frames)
        .expect("stage-driven sweep runs");
    let ssot = ssot_counters(f64::from(es_n0), frames, 4);

    // Without a mixed verdict the `errors`/`fer` columns compare 0 == 0.
    assert!(
        staged.errors > 0 && staged.errors < staged.frames,
        "expected a mixed decode-success/failure sweep at the waterfall, got \
         {}/{} errored frames",
        staged.errors,
        staged.frames
    );
    common::assert_four_columns_byte_identical(&staged, &ssot, "stage-driven CPU waterfall @6dB");
    eprintln!(
        "stage-driven CPU waterfall: frames={} errors={} fer={:.6} mean_iters={:.6} \
         (byte-identical to SSOT)",
        staged.frames,
        staged.errors,
        staged.fer(),
        staged.mean_iters()
    );
}

#[cfg(feature = "hip")]
mod gpu {
    use super::*;

    fn gpu_present() -> bool {
        gf2_kernels_hip::host::device_mem_info().is_ok()
    }

    #[test]
    fn test_stage_driven_gpu_smoke_matches_ssot_3_columns() {
        if !gpu_present() {
            eprintln!("skipping test_stage_driven_gpu_smoke_matches_ssot_3_columns: no usable GPU");
            return;
        }
        // At 6.0 dB this seed's first 4 global frames decode to a mixed verdict.
        const SMOKE_SEED: u64 = 0xDE16_0FC5;
        let es_n0 = 6.0_f32;
        let frames = 4usize;
        let pipeline = build_pipeline_seeded(es_n0, 4, true, SMOKE_SEED);
        assert_eq!(
            pipeline.stage_count(),
            8,
            "the GPU chain replaces the combined decode with GpuLdpcBp + BCH tail"
        );
        let scheduler = Scheduler::from_pipeline(&pipeline);
        assert!(
            scheduler.gpu_active(),
            "GPU host must build an active stream pool"
        );

        let staged = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, frames)
            .expect("stage-driven GPU smoke runs");
        let ssot = ssot_counters_seeded(f64::from(es_n0), frames, 4, SMOKE_SEED);

        assert!(
            staged.errors > 0 && staged.errors < staged.frames,
            "expected a mixed decode-success/failure smoke at the waterfall, got \
             {}/{} errored frames (re-pin SMOKE_SEED if the chain changes)",
            staged.errors,
            staged.frames
        );

        assert_three_columns_byte_identical_log_mean_iters(
            &staged,
            &ssot,
            "stage-driven GPU smoke @6dB staged(GPU)-vs-ssot(CPU)",
        );
    }

    #[test]
    #[ignore = "sim: GPU-gated 32-frame waterfall sweep (per-frame GPU LDPC decodes)"]
    fn test_stage_driven_gpu_chain_matches_ssot_3_columns() {
        if !gpu_present() {
            eprintln!("skipping test_stage_driven_gpu_chain_matches_ssot_3_columns: no usable GPU");
            return;
        }
        let es_n0 = 6.0_f32;
        let frames = 32usize;
        let pipeline = build_pipeline(es_n0, 4, true);
        assert_eq!(
            pipeline.stage_count(),
            8,
            "the GPU chain replaces the combined decode with GpuLdpcBp + BCH tail"
        );
        let scheduler = Scheduler::from_pipeline(&pipeline);
        assert!(
            scheduler.gpu_active(),
            "GPU host must build an active stream pool"
        );

        let staged = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, frames)
            .expect("stage-driven GPU sweep runs");
        let ssot = ssot_counters(f64::from(es_n0), frames, 4);

        assert!(
            staged.errors > 0 && staged.errors < staged.frames,
            "expected a mixed decode-success/failure sweep at the waterfall, got \
             {}/{} errored frames",
            staged.errors,
            staged.frames
        );

        assert_three_columns_byte_identical_log_mean_iters(
            &staged,
            &ssot,
            "stage-driven GPU chain @6dB staged(GPU)-vs-ssot(CPU)",
        );
    }
}
