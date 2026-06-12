//! Stage-driven DVB-T2 chain vs the SSOT path: byte-identity (issue
//! `de160fc5`, the moved `75c22fa8` [hard] criterion; design doc §11).
//!
//! The stage-driven path is [`TopologyExecutor::run_dvb_t2_snr_point`]: every
//! stage of the preset-built DVB-T2 BICM chain executes via its type-erased
//! `AnyStage` object, routed by `execution_class()`. The SSOT path is the
//! pinned [`run_snr_point`] dispatch over the
//! [`DvbT2BicmFrameSim`] frame kernel (the `3fcb7025` composite that
//! `Pipeline::run` drives).
//!
//! # Contracts asserted
//!
//! * **CPU-only chain: 4 columns** — `fer` / `frames` / `errors` /
//!   `mean_iters` byte-identical (the §11 CPU contract), asserted via the
//!   shared [`common::assert_four_columns_byte_identical`] comparator (BER
//!   deliberately excluded there, issue `152388f4`).
//! * **GPU LDPC stage in the chain: 3 columns** (§11 CPU-vs-GPU relaxed
//!   contract) — `fer` / `frames` / `errors` byte-identical; `mean_iters` is
//!   **logged, never asserted** (RDNA2 transcendental ULP drift can shift BP
//!   early-termination by ±1).
//!
//! # Why the stage path CAN be byte-identical (the determinism trap)
//!
//! The SSOT kernel draws all per-frame randomness — random BBFRAME, then AWGN
//! noise (all I-axis samples, then all Q-axis samples) — from ONE ChaCha20
//! stream reseeked per global frame. The stage-driven sweep reproduces that
//! order exactly: same `random_bitvec` message draw at the frame offset, the
//! channel stage's scratch RNG positioned at the post-message offset, and the
//! `Awgn` stage's planar draw order (aligned to the
//! `transmit_and_demodulate_with_noise` SSOT contract by this task). The
//! demapper N0 and channel sigma derivations are bit-identical between the
//! preset and the frame kernel for `f32`-representable Es/N0 values.
//!
//! # Tiers
//!
//! * Fast: a 2-frame above-threshold CPU smoke (both arms cheap), plus —
//!   under `hip`, GPU-presence-gated, NOT ignored — a 4-frame **waterfall**
//!   GPU smoke asserting the non-vacuous 3-column contract on every green
//!   `--profile ci` run (so the gate genuinely exercises the stage-driven
//!   GPU byte-identity criterion rather than deferring it to the ignored
//!   slow leg).
//! * Slow (`#[ignore = "sim: …"]`): a 32-frame **waterfall** sweep (6.0 dB
//!   r1/2 16-QAM) asserting a non-vacuous mixed verdict — the regime §11 is
//!   about — for the CPU 4-column and (GPU-gated, `hip`) the GPU 3-column
//!   contract. The stage chain's shared codec serialises decodes on its
//!   internal lock, so the staged arm runs near single-thread speed; 32
//!   frames keeps each test well under the 120 s slow-tier cap.
//!
//! [`TopologyExecutor::run_dvb_t2_snr_point`]: gf2_sim::TopologyExecutor::run_dvb_t2_snr_point
//! [`run_snr_point`]: gf2_sim::parallel::run_snr_point
//! [`DvbT2BicmFrameSim`]: gf2_sim::frame_sim::DvbT2BicmFrameSim

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

/// Builds the preset DVB-T2 r1/2 16-QAM chain at `es_n0_db` (which must be
/// `f32`-representable so the preset and the frame kernel derive bit-identical
/// sigma / N0) at an explicit `seed`.
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

/// [`build_pipeline_seeded`] at the suite's shared [`SEED`].
fn build_pipeline(es_n0_db: f32, workers: usize, gpu: bool) -> Pipeline {
    build_pipeline_seeded(es_n0_db, workers, gpu, SEED)
}

/// The SSOT arm: `run_snr_point` over the `DvbT2BicmFrameSim` kernel (the
/// byte-identity baseline every other path is pinned to), at an explicit
/// `seed`.
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

/// [`ssot_counters_seeded`] at the suite's shared [`SEED`].
fn ssot_counters(es_n0_db: f64, frames: usize, workers: usize) -> WorkerCounters {
    ssot_counters_seeded(es_n0_db, frames, workers, SEED)
}

/// Fast-tier smoke: 2 frames above threshold (9 dB). Every column — including
/// the integer-exact totals — must match the SSOT bit-for-bit.
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
    // Above threshold both arms decode cleanly (sanity, not the contract).
    assert_eq!(staged.errors, 0, "9 dB is above the r1/2 16-QAM waterfall");
}

/// Slow tier: the §11 regime. 32 frames at the r1/2 16-QAM waterfall
/// (6.0 dB) produce a non-vacuous mixed verdict; the 4 columns must be
/// byte-identical to the SSOT path.
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

    // Non-vacuous: the waterfall yields a genuine mixed verdict, so the
    // assertion exercises the frame-verdict boundary §11 names.
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

/// GPU arm (slow tier, GPU-gated): the stage-driven chain with the
/// `ExecutionClass::GpuOnly` LDPC BP stage must match the SSOT CPU path on
/// the three §11 relaxed-contract columns; `mean_iters` is logged only.
#[cfg(feature = "hip")]
mod gpu {
    use super::*;

    fn gpu_present() -> bool {
        gf2_kernels_hip::host::device_mem_info().is_ok()
    }

    /// The fast-tier stage-driven GPU smoke (round-1 finding 2): a NOT-ignored,
    /// GPU-presence-gated miniature of the 32-frame waterfall leg below, so the
    /// green `cargo nextest --profile ci` gate genuinely RUNS the stage-driven
    /// GPU byte-identity criterion on the gfx1030 host instead of skipping it.
    ///
    /// 4 frames at the same 6.0 dB r1/2 16-QAM waterfall, at a PINNED seed
    /// chosen so the frame-verdict mix is non-vacuous (`0 < errors < frames`,
    /// asserted). The three §11 relaxed-contract columns
    /// (`fer`/`frames`/`errors`) must be byte-identical stage-driven-vs-SSOT;
    /// `mean_iters` is logged, never asserted (§11 CPU-vs-GPU exclusion).
    ///
    /// Timing: measured ~3 s on the gfx1030 host under the ci profile
    /// (pipeline build + 4 staged GPU frames + 4 SSOT CPU frames across 4
    /// workers), inside the 5 s fast-tier cap. Skips cleanly with no GPU.
    #[test]
    fn test_stage_driven_gpu_smoke_matches_ssot_3_columns() {
        if !gpu_present() {
            eprintln!("skipping test_stage_driven_gpu_smoke_matches_ssot_3_columns: no usable GPU");
            return;
        }
        // Pinned smoke seed: at 6.0 dB this seed's first 4 global frames decode
        // to a mixed verdict (some errored, some clean) — verified empirically
        // and asserted non-vacuous below.
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

        // Non-vacuous mixed verdict at the waterfall (the §11 regime).
        assert!(
            staged.errors > 0 && staged.errors < staged.frames,
            "expected a mixed decode-success/failure smoke at the waterfall, got \
             {}/{} errored frames (re-pin SMOKE_SEED if the chain changes)",
            staged.errors,
            staged.frames
        );

        // The three §11 CPU-vs-GPU columns, byte-identical, via the shared
        // SSOT comparator (mean_iters logged there, never asserted).
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

        // Non-vacuous mixed verdict at the waterfall (the §11 regime).
        assert!(
            staged.errors > 0 && staged.errors < staged.frames,
            "expected a mixed decode-success/failure sweep at the waterfall, got \
             {}/{} errored frames",
            staged.errors,
            staged.frames
        );

        // The three §11 CPU-vs-GPU columns, byte-identical, via the shared
        // SSOT comparator (mean_iters logged there, never asserted).
        assert_three_columns_byte_identical_log_mean_iters(
            &staged,
            &ssot,
            "stage-driven GPU chain @6dB staged(GPU)-vs-ssot(CPU)",
        );
    }
}
