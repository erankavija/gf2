//! A parallel [`run_snr_point`] run produces byte-identical `fer` / `frames` /
//! `errors` / `mean_iters` to the single-worker run for the same seed, on each
//! worker count in [`WORKER_COUNTS`] and for three DVB-T2 `(rate, modulation)`
//! configurations.

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};

mod common;
use common::assert_four_columns_byte_identical;

const WORKER_COUNTS: [usize; 5] = [1, 2, 4, 8, 24];

const FRAMES: usize = 12;

const SEED: u64 = 0xC0DE_F00D;

fn run_all_worker_counts(
    sim: &DvbT2BicmFrameSim,
    seed: u64,
    snr_idx: usize,
) -> Vec<WorkerCounters> {
    WORKER_COUNTS
        .iter()
        .map(|&w| {
            let p = NonZeroUsize::new(w).expect("worker count is non-zero");
            run_snr_point(
                seed,
                snr_idx,
                FRAMES,
                p,
                || sim.clone(),
                |g, ctx, s| s.simulate_frame(g, ctx),
            )
        })
        .collect()
}

fn assert_byte_identical(results: &[WorkerCounters], label: &str) {
    let baseline = results[0]; // 1-worker reference.
    for (i, c) in results.iter().enumerate() {
        let w = WORKER_COUNTS[i];
        assert_four_columns_byte_identical(c, &baseline, &format!("{label} @ {w} workers"));
    }
}

/// `snr_idx` only selects the [`SNR_STRIDE`](gf2_sim::parallel::SNR_STRIDE)
/// region; distinct values per config keep their RNG streams disjoint.
fn assert_config_byte_identical(
    snr_idx: usize,
    rate: CodeRate,
    modulation: DvbT2Modulation,
    es_n0_db: f64,
    algo: DecoderAlgorithm,
    demap: DemapMethod,
) {
    let sim = DvbT2BicmFrameSim::new(
        rate,
        modulation,
        es_n0_db,
        DecoderConfig::new(algo, true),
        demap,
    );
    let label = format!("{rate:?}/{modulation:?}@{es_n0_db}dB");
    let results = run_all_worker_counts(&sim, SEED, snr_idx);
    assert_eq!(results[0].frames, FRAMES as u64, "{label}: frame budget");
    assert_byte_identical(&results, &label);
}

#[test]
#[ignore = "sim: determinism across workers — r1/2 16-QAM SumProduct/ExactLogMap"]
fn determinism_r1_2_16qam_sumproduct() {
    assert_config_byte_identical(
        0,
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        6.25,
        DecoderAlgorithm::SumProduct,
        DemapMethod::ExactLogMap,
    );
}

#[test]
#[ignore = "sim: determinism across workers — r2/3 16-QAM NMS(0.75)/ExactLogMap"]
fn determinism_r2_3_16qam_nms() {
    assert_config_byte_identical(
        1,
        CodeRate::Rate2_3,
        DvbT2Modulation::Qam16,
        8.9,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DemapMethod::ExactLogMap,
    );
}

#[test]
#[ignore = "sim: determinism across workers — r1/2 64-QAM MinSum/MaxLog"]
fn determinism_r1_2_64qam_minsum() {
    assert_config_byte_identical(
        2,
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam64,
        9.9,
        DecoderAlgorithm::MinSum,
        DemapMethod::MaxLog,
    );
}
