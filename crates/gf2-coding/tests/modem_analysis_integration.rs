//! Integration tests for the opt-in per-bit analysis capture of
//! `SimulationRunner`, over [`BpskAwgnChannel`] (`batch_alignment = 1`) and
//! [`ModemChannelAdapter`] with Gray 16-QAM (`batch_alignment = 4`).

use gf2_coding::modem::analysis::PerBitLlrStats;
use gf2_coding::modem::{
    AnalysisCapture, DemapMethod, GrayQamMapper, ModemChannelAdapter, ModemSpec,
    ReferenceSoftDemapper,
};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationRunner};
use rand::rngs::StdRng;
use rand::SeedableRng;

/// The frame budget gives thousands of samples per bit position within the
/// fast-tier time limit.
fn bpsk_config() -> SimulationConfig {
    SimulationConfig {
        eb_n0_range_db: vec![6.0],
        min_errors: usize::MAX,
        max_frames: 8_000,
        max_decoder_iterations: 0,
        rng_seed: Some(0x80F2_18CA),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    }
}

/// `max_frames` is a multiple of the runner's 960-bit batch, so every batch
/// is full and each bit position receives the same number of samples.
fn qam16_config() -> SimulationConfig {
    SimulationConfig {
        eb_n0_range_db: vec![9.0],
        min_errors: usize::MAX,
        max_frames: 7_680,
        max_decoder_iterations: 0,
        rng_seed: Some(0x80F2_18CA_u64.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    }
}

#[test]
fn test_analysis_capture_integrates_with_uncoded_runner_bpsk() {
    let channel = BpskAwgnChannel;
    let config = bpsk_config();

    let mut stats = PerBitLlrStats::new(1);
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let results = {
        let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::ExactLogMap);
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
        let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::ExactLogMap);
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
    // The stream has a distinct fingerprint per bit position, so a column
    // transpose, stride shift or MSB/LSB swap in the accumulator changes the
    // counts: position k has bit-1 fraction (k + 1) / (m + 1) and |LLR| = k + 1.
    use gf2_coding::llr::Llr;

    let bits_per_symbol = 4u8;
    let m = bits_per_symbol as usize;
    // A multiple of 5, so every (k + 1) / 5 gives an integer count.
    let num_symbols: usize = 5 * 200;
    let mut stats = PerBitLlrStats::new(bits_per_symbol);

    let mut llrs: Vec<Llr> = Vec::with_capacity(num_symbols * m);
    let mut truth: Vec<bool> = Vec::with_capacity(num_symbols * m);
    for s in 0..num_symbols {
        for k in 0..m {
            // Exactly (k + 1) * num_symbols / 5 ones at position k.
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
#[should_panic(expected = "AnalysisCapture was tagged with")]
fn test_analysis_capture_mismatched_demap_method_panics() {
    // `BpskAwgnChannel` produces ExactLogMap LLRs; the capture is tagged MaxLog.
    let channel = BpskAwgnChannel;
    let mut stats = PerBitLlrStats::new(1);
    let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::MaxLog);
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
fn test_analysis_report_carries_demap_method_provenance() {
    let channel = BpskAwgnChannel;
    let config = bpsk_config();
    let mut stats = PerBitLlrStats::new(1);
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    {
        let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::ExactLogMap);
        let _ = SimulationRunner::run_uncoded_ber_with_analysis(
            &channel,
            &config,
            Some(&mut capture),
            &mut rng,
        );
    }
    let report = stats.report();
    assert_eq!(report.len(), 1);
    assert_eq!(
        report[0].demap_method,
        Some(DemapMethod::ExactLogMap),
        "report must carry the demap method that produced the LLRs"
    );
}

#[test]
#[should_panic(expected = "merge: demap_method mismatch")]
fn test_analysis_stats_merge_rejects_mismatched_demap_methods() {
    use gf2_coding::llr::Llr;
    let mut a = PerBitLlrStats::new(1);
    let mut b = PerBitLlrStats::new(1);
    a.set_demap_method_once(DemapMethod::ExactLogMap);
    b.set_demap_method_once(DemapMethod::MaxLog);
    // Accumulate something in each so merge is a real operation.
    a.accumulate(&[Llr::new(1.0)], &[false]);
    b.accumulate(&[Llr::new(1.0)], &[false]);
    a.merge(b);
}

#[test]
fn test_analysis_capture_retains_demap_method_tag() {
    let mut stats_a = PerBitLlrStats::new(1);
    let cap_a = AnalysisCapture::with_method(&mut stats_a, DemapMethod::ExactLogMap);
    assert_eq!(cap_a.demap_method(), DemapMethod::ExactLogMap);
    let mut stats_b = PerBitLlrStats::new(4);
    let cap_b = AnalysisCapture::with_method(&mut stats_b, DemapMethod::MaxLog);
    assert_eq!(cap_b.demap_method(), DemapMethod::MaxLog);
    let mut stats_c = PerBitLlrStats::new(2);
    let cap_c = AnalysisCapture::new(&mut stats_c);
    assert_eq!(cap_c.demap_method(), DemapMethod::MaxLog);
}

#[test]
#[should_panic(expected = "AnalysisCapture bits_per_symbol")]
fn test_analysis_capture_mismatched_bits_per_symbol_panics() {
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
    let channel = BpskAwgnChannel;
    let make_config = |points: Vec<f64>| SimulationConfig {
        eb_n0_range_db: points,
        min_errors: usize::MAX,
        max_frames: 4_000,
        max_decoder_iterations: 0,
        rng_seed: Some(0xAAAA_5555),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    };
    const RNG_SEED: u64 = 0xAAAA_5555;

    let mut swept_stats = PerBitLlrStats::new(1);
    {
        let mut cap = AnalysisCapture::with_method(&mut swept_stats, DemapMethod::ExactLogMap);
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

    let mut stats_3db = PerBitLlrStats::new(1);
    {
        let mut cap = AnalysisCapture::with_method(&mut stats_3db, DemapMethod::ExactLogMap);
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

    let mut stats_7db = PerBitLlrStats::new(1);
    {
        let mut cap = AnalysisCapture::with_method(&mut stats_7db, DemapMethod::ExactLogMap);
        // The two-point runner uses one StdRng across both SNR points: a 3 dB
        // warmup on a re-seeded stream reproduces its state at the 7 dB point.
        let mut rng = StdRng::seed_from_u64(RNG_SEED);
        let mut warmup_stats = PerBitLlrStats::new(1);
        {
            let mut warmup =
                AnalysisCapture::with_method(&mut warmup_stats, DemapMethod::ExactLogMap);
            let _ = SimulationRunner::run_uncoded_ber_with_analysis(
                &channel,
                &make_config(vec![3.0]),
                Some(&mut warmup),
                &mut rng,
            );
        }
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
