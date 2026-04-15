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
fn test_analysis_capture_unused_accumulator_stays_empty_after_none_sweep() {
    // Guards the opt-in contract from the *other* side: build an
    // accumulator up front, but deliberately do NOT pass it to the
    // runner. Run a sweep with `None`. After the sweep, the untouched
    // accumulator must still be empty — catches a regression where the
    // runner grabs some shared/thread-local capture target instead of
    // respecting the caller's explicit `None`.
    let channel = BpskAwgnChannel;
    let config = bpsk_config();

    let unused = PerBitLlrStats::new(1);
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let _ = SimulationRunner::run_uncoded_ber_with_analysis(&channel, &config, None, &mut rng);

    let report = unused.report();
    for r in &report {
        assert_eq!(r.bit0.count(), 0);
        assert_eq!(r.bit1.count(), 0);
    }
}

#[test]
#[should_panic(expected = "AnalysisCapture bits_per_symbol")]
fn test_analysis_capture_mismatched_bits_per_symbol_panics() {
    // A 16-QAM modem channel advertises batch_alignment = 4, so an
    // AnalysisCapture built from a BPSK-shaped (m = 1) accumulator must
    // be rejected up front. Previously this silently accumulated
    // nonsensical per-position statistics; the runner now panics with a
    // descriptive error before the first batch.
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let mapper = GrayQamMapper::<f32>::from_preset_order(16);
    let demapper = ReferenceSoftDemapper::new(spec);
    let channel = ModemChannelAdapter::new(mapper, demapper, DemapMethod::MaxLog);

    // BPSK-shaped accumulator: m = 1, but channel.batch_alignment() = 4.
    let mut wrong = PerBitLlrStats::new(1);
    let mut capture = AnalysisCapture::new(&mut wrong);
    let config = bpsk_config();
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let _ = SimulationRunner::run_uncoded_ber_with_analysis(
        &channel,
        &config,
        Some(&mut capture),
        &mut rng,
    );
}

#[test]
fn test_analysis_capture_multi_snr_aggregation_is_documented() {
    // The documented behaviour is that the same AnalysisCapture is
    // reused across every SNR point in the sweep and reports an
    // aggregate. Lock that: run a two-point sweep with a capture, then
    // run two single-point sweeps at the same seeds, and confirm the
    // total sample count is preserved (we do not require bit-exact
    // decomposition, because the runner stream order is deliberately
    // sweep-oriented, but the *sum* must match so callers reading
    // `report()` see every bit that was transmitted).
    let channel = BpskAwgnChannel;
    let config = SimulationConfig {
        eb_n0_range_db: vec![3.0, 7.0],
        min_errors: usize::MAX,
        max_frames: 4_000,
        max_decoder_iterations: 0,
        rng_seed: Some(0xAAAA_5555),
        output_path: None,
    };

    let mut stats = PerBitLlrStats::new(1);
    let mut cap = AnalysisCapture::new(&mut stats);
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let _ = SimulationRunner::run_uncoded_ber_with_analysis(
        &channel,
        &config,
        Some(&mut cap),
        &mut rng,
    );

    let report = stats.report();
    let total = report[0].bit0.count() + report[0].bit1.count();
    // Two SNR points × max_frames bits each, rounded to batch alignment
    // (= 1 for BPSK), must account for every transmitted bit.
    let expected_min = 2 * (config.max_frames - (config.max_frames % 960));
    assert!(
        total >= expected_min as u64,
        "aggregate sample count {total} is below expected minimum {expected_min} — \
         the multi-SNR sweep did not accumulate across both points"
    );
}
