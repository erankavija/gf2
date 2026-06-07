//! Within-SNR parallel determinism regression (issue `3fcb7025`, design doc
//! §3 / §11).
//!
//! Asserts the [hard] success criterion: a parallel run on `{1, 2, 4, 8, 24}`
//! workers produces **byte-identical** `fer` / `frames` / `errors` /
//! `mean_iters` versus single-thread for the same seed, across at least three
//! `(rate, modulation)` configurations.
//!
//! This is the slow-tier regression: each config runs a Normal-frame
//! (n = 64800) DVB-T2 BICM decode for `FRAMES` frames per worker count, so it is
//! `#[ignore]`d. The three configs are split into three tests
//! (`determinism_r1_2_16qam_sumproduct`, `determinism_r2_3_16qam_nms`,
//! `determinism_r1_2_64qam_minsum`) so each stays under the slow tier's
//! 120 s/test cap. Run them all explicitly with:
//!
//! ```bash
//! cargo nextest run -p gf2-sim --release --profile slow \
//!     --run-ignored ignored-only -E 'test(determinism_r)'
//! ```
//!
//! A fast-tier smoke guard for the seek/aggregation logic ({1,2} workers,
//! synthetic closure) lives in `parallel/mod.rs`'s unit tests so CI still guards
//! the core logic without the full LDPC decode cost.

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::{run_snr_point, WorkerCounters};

/// Worker counts the byte-identity must hold across (the issue's exact set).
const WORKER_COUNTS: [usize; 5] = [1, 2, 4, 8, 24];

/// Frames per worker-count run. Chosen to straddle the waterfall (some frames
/// decode, some fail) at the chosen SNRs so the test exercises non-trivial
/// `errors` / `mean_iters`, not just the all-converge or all-fail degenerate
/// cases. Kept modest so the slow tier stays well under its 120 s/test budget.
const FRAMES: usize = 12;

/// Fixed base seed for the run.
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
    for (i, &c) in results.iter().enumerate() {
        let w = WORKER_COUNTS[i];
        assert_eq!(
            c.frames, baseline.frames,
            "{label}: frames differ at {w} workers ({} vs {})",
            c.frames, baseline.frames
        );
        assert_eq!(
            c.errors, baseline.errors,
            "{label}: errors differ at {w} workers ({} vs {})",
            c.errors, baseline.errors
        );
        // fer and mean_iters are derived integer ratios — byte-identical iff the
        // integer counters are. Assert the bit patterns to be strict.
        assert_eq!(
            c.fer().to_bits(),
            baseline.fer().to_bits(),
            "{label}: fer bit pattern differs at {w} workers ({} vs {})",
            c.fer(),
            baseline.fer()
        );
        assert_eq!(
            c.mean_iters().to_bits(),
            baseline.mean_iters().to_bits(),
            "{label}: mean_iters bit pattern differs at {w} workers ({} vs {})",
            c.mean_iters(),
            baseline.mean_iters()
        );
        assert_eq!(
            c.total_iterations, baseline.total_iterations,
            "{label}: total_iterations differ at {w} workers"
        );
    }
}

/// Asserts byte-identity across all worker counts for one `(rate, modulation)`
/// configuration at a waterfall Es/N0 (frames straddle decode success/failure).
///
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

// The three configs are split into separate tests so each stays under the slow
// tier's 120 s/test cap; together they satisfy the [hard] criterion of
// byte-identity across {1,2,4,8,24} workers over >= 3 (rate, modulation)
// configs. Run them all with:
//
//   cargo nextest run -p gf2-sim --release --profile slow \
//       --run-ignored ignored-only -E 'test(determinism_r)'

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
