//! CPU-path `Pipeline::run` integration tests (Phase C `75c22fa8`).
//!
//! These exercise the runnable [`Pipeline::run`] surface on the CPU-only path
//! (no `hip` feature needed) and pin it to the `3fcb7025` SSOT
//! [`run_snr_point`] dispatch: the scheduler's CPU path is a thin wrapper over
//! [`run_snr_point`], so a `Pipeline::run` over the preset MUST produce the same
//! `fer` / `frames` / `errors` / `mean_iters` as a direct
//! [`run_snr_point`] over the same [`DvbT2BicmFrameSim`] kernel. This guards the
//! "scheduler's CPU path preserves global-frame-indexed seeking" requirement
//! without breaking the existing byte-identity contract.

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::run_snr_point;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::Pipeline;

mod common;
use common::{assert_four_columns_byte_identical, snr_point_to_counters};

const SEED: u64 = 0x75C2_2FA8;
const ES_N0: f64 = 9.0; // well above the r1/2 16-QAM waterfall

fn build(workers: usize, frames: u64) -> Pipeline {
    let mut p = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(ES_N0 as f32))
        .parallelism(NonZeroUsize::new(workers).unwrap())
        .seed(SEED)
        .build()
        .expect("in-scope MODCOD builds");
    p.config_mut().esn0_db_points = vec![ES_N0];
    p.config_mut().max_frames = frames;
    p
}

#[test]
fn pipeline_run_cpu_single_frame_decodes() {
    // One frame above threshold: frames == 1, no error. Fast tier (< 5 s).
    let p = build(1, 1);
    let r = p.run().expect("cpu run");
    assert_eq!(r.per_point.len(), 1);
    assert_eq!(r.per_point[0].frames, 1);
    assert_eq!(
        r.per_point[0].errors, 0,
        "frame above threshold must decode"
    );
}

#[test]
fn pipeline_run_cpu_matches_run_snr_point_ssot() {
    // The CPU `Pipeline::run` must produce the SAME aggregate as a direct
    // `run_snr_point` over the same frame kernel (the 3fcb7025 SSOT). 4 frames
    // keeps the fast tier under 5 s (4 × ~0.6 s decode at high SNR, 4 workers).
    let frames = 4u64;
    let p = build(4, frames);
    let via_pipeline = p.run().expect("cpu run").per_point[0];

    let template = DvbT2BicmFrameSim::new(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        ES_N0,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
        DemapMethod::ExactLogMap,
    );
    let direct = run_snr_point(
        SEED,
        0,
        frames as usize,
        NonZeroUsize::new(4).unwrap(),
        || template.clone(),
        |g, ctx, sim| sim.simulate_frame(g, ctx),
    );

    // All four contractual columns must match the run_snr_point SSOT
    // bit-for-bit, via the shared SSOT comparator over the adapted point.
    assert_four_columns_byte_identical(
        &snr_point_to_counters(&via_pipeline),
        &direct,
        "Pipeline::run vs run_snr_point SSOT",
    );
}
