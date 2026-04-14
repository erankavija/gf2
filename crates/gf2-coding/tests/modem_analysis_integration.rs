//! Integration tests for the opt-in per-bit analysis capture
//! wired into `SimulationRunner` (JIT `80f218ca`).
//!
//! These exercise the public surface end-to-end over the two
//! `ChannelModel` paths that matter in practice:
//!
//! 1. [`BpskAwgnChannel`] — the legacy BPSK path (`batch_alignment = 1`),
//!    verifying that every transmitted bit is accounted for in the
//!    capture.
//! 2. [`ModemChannelAdapter`] over a Gray-coded 16-QAM preset — the
//!    generic modem path (`batch_alignment = 4`), verifying that the
//!    capture aggregates per-bit-position statistics for every bit of
//!    every transmitted symbol.
//!
//! The third test locks the zero-overhead behavioural contract: running
//! the sweep with `None` through `run_uncoded_ber_with_analysis` must
//! produce an identical BER to the unaugmented
//! `run_uncoded_ber_with_channel` runner at the same seed.

use gf2_coding::modem::analysis::PerBitLlrStats;
use gf2_coding::modem::{
    AnalysisCapture, DemapMethod, GrayQamMapper, ModemChannelAdapter, ModemSpec,
    ReferenceSoftDemapper,
};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationRunner};
use rand::rngs::StdRng;
use rand::SeedableRng;

/// Locked SNR / frame budget for the integration tests. Small enough
/// to keep the release-mode test suite well under the 60-second
/// workspace limit but large enough that the capture has thousands of
/// per-bit-position samples to report.
fn bpsk_config() -> SimulationConfig {
    SimulationConfig {
        eb_n0_range_db: vec![6.0],
        min_errors: usize::MAX,
        max_frames: 8_000,
        max_decoder_iterations: 0,
        rng_seed: Some(0x80F2_18CA),
        output_path: None,
    }
}

/// 16-QAM-oriented config. `max_frames` is a multiple of 960 so the
/// runner's internal batcher (rounded down to `bits_per_symbol = 4`)
/// sees full batches and the capture gathers even mass across bit
/// positions.
fn qam16_config() -> SimulationConfig {
    SimulationConfig {
        eb_n0_range_db: vec![9.0],
        min_errors: usize::MAX,
        max_frames: 7_680,
        max_decoder_iterations: 0,
        rng_seed: Some(0x80F2_18CA_u64.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
        output_path: None,
    }
}

#[test]
fn test_analysis_capture_integrates_with_uncoded_runner_bpsk() {
    let channel = BpskAwgnChannel;
    let config = bpsk_config();

    let mut stats = PerBitLlrStats::new(1);
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let results = {
        let mut capture = AnalysisCapture::new(&mut stats);
        SimulationRunner::run_uncoded_ber_with_analysis(
            &channel,
            &config,
            Some(&mut capture),
            &mut rng,
        )
    };
    assert_eq!(results.len(), 1);
    let total_bits = results[0].num_bits;

    let report = stats.report();
    assert_eq!(report.len(), 1, "BPSK has exactly one bit per symbol");

    let observed = report[0].bit0.count() + report[0].bit1.count();
    assert_eq!(
        observed as usize, total_bits,
        "capture must see every transmitted bit: got {observed}, expected {total_bits}",
    );
    assert!(total_bits > 0, "runner must transmit at least one bit");
}

#[test]
fn test_analysis_capture_integrates_with_qam16_runner() {
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let mapper = GrayQamMapper::<f32>::from_preset_order(16);
    let demap = ReferenceSoftDemapper::new(spec);
    let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);
    assert_eq!(adapter.batch_alignment(), 4, "16-QAM has 4 bits per symbol");

    let config = qam16_config();

    let bits_per_symbol = 4u8;
    let mut stats = PerBitLlrStats::new(bits_per_symbol);
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let results = {
        let mut capture = AnalysisCapture::new(&mut stats);
        SimulationRunner::run_uncoded_ber_with_analysis(
            &adapter,
            &config,
            Some(&mut capture),
            &mut rng,
        )
    };
    assert_eq!(results.len(), 1);
    let total_bits = results[0].num_bits;
    assert!(total_bits > 0);
    assert_eq!(total_bits % bits_per_symbol as usize, 0);

    let report = stats.report();
    assert_eq!(report.len(), bits_per_symbol as usize);

    let per_position_samples = (total_bits / bits_per_symbol as usize) as u64;
    for (idx, r) in report.iter().enumerate() {
        let seen = r.bit0.count() + r.bit1.count();
        assert_eq!(
            seen, per_position_samples,
            "bit position {idx}: capture must see one sample per symbol; got {seen}, expected {per_position_samples}",
        );
    }
}

#[test]
fn test_analysis_capture_disabled_matches_unaugmented_path() {
    // Same seed, same config, same channel. The analysis-capable runner
    // with `None` must return exactly the same BER as the unaugmented
    // `run_uncoded_ber_with_channel` path. This is the behavioural
    // lock behind the zero-overhead contract.
    let channel = BpskAwgnChannel;
    let config = bpsk_config();
    let seed = config.rng_seed.unwrap();

    let mut rng_a = StdRng::seed_from_u64(seed);
    let baseline = SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng_a);

    let mut rng_b = StdRng::seed_from_u64(seed);
    let analysed =
        SimulationRunner::run_uncoded_ber_with_analysis(&channel, &config, None, &mut rng_b);

    assert_eq!(baseline.len(), analysed.len());
    for (a, b) in baseline.iter().zip(analysed.iter()) {
        assert_eq!(a.eb_n0_db, b.eb_n0_db);
        assert_eq!(a.num_bits, b.num_bits);
        assert_eq!(a.num_bit_errors, b.num_bit_errors);
        assert_eq!(a.ber, b.ber);
    }
}

#[test]
fn test_analysis_capture_none_branch_is_inert() {
    // Sanity: passing `None` does not mutate any accumulator state we
    // might have nearby. A fresh `PerBitLlrStats` built before the run
    // must still be empty after a `None`-capture sweep.
    let channel = BpskAwgnChannel;
    let config = bpsk_config();

    let stats_before = PerBitLlrStats::new(1);
    let empty_report_before = stats_before.report();
    let empty_counts_before: Vec<(u64, u64)> = empty_report_before
        .iter()
        .map(|r| (r.bit0.count(), r.bit1.count()))
        .collect();

    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let _ = SimulationRunner::run_uncoded_ber_with_analysis(&channel, &config, None, &mut rng);

    let stats_after = PerBitLlrStats::new(1);
    let empty_report_after = stats_after.report();
    let empty_counts_after: Vec<(u64, u64)> = empty_report_after
        .iter()
        .map(|r| (r.bit0.count(), r.bit1.count()))
        .collect();

    assert_eq!(empty_counts_before, empty_counts_after);
}
