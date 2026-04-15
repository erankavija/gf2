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
fn test_analysis_capture_preserves_msb_first_bit_position_mapping_qam16() {
    // Correctness lock for bit-channel separation on Gray-QAM. We feed
    // `PerBitLlrStats::accumulate` a symbol-major, MSB-first stream
    // with a pattern whose per-position fingerprint is **asymmetric**
    // across bit positions — so any column transpose, stride shift, or
    // MSB↔LSB swap in the accumulator would scramble the counts.
    //
    // The asymmetry is encoded in two ways:
    //   1. Per-position bit1 fraction is `p_k = (k + 1) / (m + 1)` —
    //      strictly monotone in `k`, so positions {0, 1, 2, 3} have
    //      distinct expected bit1 counts of N/5, 2N/5, 3N/5, 4N/5.
    //      Transposing any two positions breaks the monotone sequence.
    //   2. LLR magnitudes scale with position: `|L_k| = (k + 1)`. That
    //      means bit0.mean()/bit1.mean() at position k = ±(k + 1), so a
    //      column swap would also scramble the means.
    use gf2_coding::llr::Llr;

    let bits_per_symbol = 4u8;
    let m = bits_per_symbol as usize;
    // Choose a multiple of 5 so every p_k = (k+1)/5 gives an integer count.
    let num_symbols: usize = 5 * 200;
    let mut stats = PerBitLlrStats::new(bits_per_symbol);

    let mut llrs: Vec<Llr> = Vec::with_capacity(num_symbols * m);
    let mut truth: Vec<bool> = Vec::with_capacity(num_symbols * m);
    for s in 0..num_symbols {
        for k in 0..m {
            // Position k is bit 1 iff (s mod 5) < (k + 1). Over
            // num_symbols = 5*N this gives exactly (k+1)*N ones at
            // position k, and (5 - (k+1))*N = (4-k)*N zeros.
            let bit = (s % 5) < (k + 1);
            truth.push(bit);
            let mag = (k + 1) as f32;
            llrs.push(Llr::new(if bit { -mag } else { mag }));
        }
    }

    stats.accumulate(&llrs, &truth);
    let report = stats.report();
    assert_eq!(report.len(), m);

    let samples_per_position = num_symbols as u64;
    for (k, r) in report.iter().enumerate() {
        let expected_ones = ((k + 1) as u64) * (num_symbols as u64 / 5);
        let expected_zeros = samples_per_position - expected_ones;

        assert_eq!(
            r.bit1.count(),
            expected_ones,
            "position {k}: bit1 count = {}, expected {expected_ones} — \
             a column transpose or stride shift would break this",
            r.bit1.count()
        );
        assert_eq!(
            r.bit0.count(),
            expected_zeros,
            "position {k}: bit0 count = {}, expected {expected_zeros}",
            r.bit0.count()
        );

        // All bit0 samples at position k are +(k+1); bit1 samples are -(k+1).
        let expected_mag = (k + 1) as f64;
        if r.bit0.count() > 0 {
            assert!(
                (r.bit0.mean() - expected_mag).abs() < 1e-9,
                "position {k}: bit0 mean = {}, expected {expected_mag} — \
                 LLR-magnitude check would flag a column swap",
                r.bit0.mean()
            );
        }
        if r.bit1.count() > 0 {
            assert!(
                (r.bit1.mean() + expected_mag).abs() < 1e-9,
                "position {k}: bit1 mean = {}, expected -{expected_mag}",
                r.bit1.mean()
            );
        }
    }

    // Final asymmetry check: the per-position bit1-fractions must be
    // strictly monotone in k. A transposition would shuffle this
    // sequence and trip the assert.
    let fractions: Vec<f64> = report
        .iter()
        .map(|r| r.bit1.count() as f64 / samples_per_position as f64)
        .collect();
    for w in fractions.windows(2) {
        assert!(
            w[1] > w[0],
            "per-position bit1 fraction must be strictly increasing, got {fractions:?}"
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
fn test_analysis_capture_multi_snr_aggregation_matches_sum_of_single_point_sweeps() {
    // The documented behaviour is that the same AnalysisCapture is
    // reused across every SNR point in the sweep and reports an
    // aggregate. Lock that precisely:
    //   1. Run a two-point sweep (Eb/N0 = 3 dB, 7 dB) into one capture.
    //   2. Run the same config as two separate single-point sweeps
    //      (3 dB, 7 dB) each into its own capture.
    //   3. The aggregate count from (1) must exactly equal
    //      single_point_3dB + single_point_7dB, because the runner
    //      consumes the same per-point frame count driven by the same
    //      seed stream.
    let channel = BpskAwgnChannel;
    let make_config = |points: Vec<f64>| SimulationConfig {
        eb_n0_range_db: points,
        min_errors: usize::MAX,
        max_frames: 4_000,
        max_decoder_iterations: 0,
        rng_seed: Some(0xAAAA_5555),
        output_path: None,
    };
    const RNG_SEED: u64 = 0xAAAA_5555;

    // (1) Two-point sweep into one capture.
    let mut swept_stats = PerBitLlrStats::new(1);
    {
        let mut cap = AnalysisCapture::new(&mut swept_stats);
        let mut rng = StdRng::seed_from_u64(RNG_SEED);
        let _ = SimulationRunner::run_uncoded_ber_with_analysis(
            &channel,
            &make_config(vec![3.0, 7.0]),
            Some(&mut cap),
            &mut rng,
        );
    }
    let swept_report = swept_stats.report();
    let swept_total = swept_report[0].bit0.count() + swept_report[0].bit1.count();

    // (2a) Single-point sweep at 3 dB. Crucial: seed the runner's RNG
    //      from the same config seed as the two-point run — that is
    //      what the two-point run does for its first SNR point.
    let mut stats_3db = PerBitLlrStats::new(1);
    {
        let mut cap = AnalysisCapture::new(&mut stats_3db);
        let mut rng = StdRng::seed_from_u64(RNG_SEED);
        let _ = SimulationRunner::run_uncoded_ber_with_analysis(
            &channel,
            &make_config(vec![3.0]),
            Some(&mut cap),
            &mut rng,
        );
    }
    let total_3db = {
        let r = stats_3db.report();
        r[0].bit0.count() + r[0].bit1.count()
    };

    // (2b) Single-point sweep at 7 dB, run **after** the 3 dB run on
    //      the same RNG stream. The two-point runner's 7 dB batches
    //      draw from the same post-3dB stream state.
    let mut stats_7db = PerBitLlrStats::new(1);
    {
        let mut cap = AnalysisCapture::new(&mut stats_7db);
        // Re-seed and advance: the two-point runner uses a single
        // StdRng across both SNR points. We emulate that by re-seeding
        // here and running a 3 dB warmup to advance the stream.
        let mut rng = StdRng::seed_from_u64(RNG_SEED);
        // Warmup: same shape the two-point runner did at 3 dB.
        let mut warmup_stats = PerBitLlrStats::new(1);
        {
            let mut warmup = AnalysisCapture::new(&mut warmup_stats);
            let _ = SimulationRunner::run_uncoded_ber_with_analysis(
                &channel,
                &make_config(vec![3.0]),
                Some(&mut warmup),
                &mut rng,
            );
        }
        // Now run the 7 dB point on the advanced stream.
        let _ = SimulationRunner::run_uncoded_ber_with_analysis(
            &channel,
            &make_config(vec![7.0]),
            Some(&mut cap),
            &mut rng,
        );
    }
    let total_7db = {
        let r = stats_7db.report();
        r[0].bit0.count() + r[0].bit1.count()
    };

    assert_eq!(
        swept_total,
        total_3db + total_7db,
        "two-point sweep aggregate ({swept_total}) must equal the sum of matching \
         single-point sweeps ({total_3db} at 3 dB + {total_7db} at 7 dB = {})",
        total_3db + total_7db
    );
}
