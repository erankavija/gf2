//! Smoke test for Phase 3 GLDPC vs LDPC simulation components.
//!
//! Verifies that both GLDPC and LDPC codes can be constructed, encoded, and
//! decoded at a high-SNR point with valid results. Runs under 5 seconds.

use gf2_coding::gldpc::{GldpcDecoder, QcGldpcCode};
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};
use gf2_coding::traits::BlockEncoder;

/// Smoke test: construct GLDPC (1024, 646), run 2 frames at high SNR.
#[test]
fn smoke_gldpc_1024_high_snr() {
    let code = QcGldpcCode::lentmaier_1024();
    assert_eq!(code.code_n(), 1024);
    assert_eq!(code.code_k(), 646);

    let mut decoder = GldpcDecoder::new(code.clone());
    let channel = BpskAwgnChannel;

    let config = SimulationConfig {
        eb_n0_range_db: vec![4.0],
        min_errors: 1_000_000,
        max_frames: 2,
        max_decoder_iterations: 50,
        rng_seed: Some(123),
        output_path: None,
    };

    let results = SimulationRunner::run_coded_iterative(&code, &mut decoder, &channel, &config);
    assert_eq!(results.points.len(), 1);

    let point = &results.points[0];
    assert_eq!(point.num_frames, 2);
    assert!(point.ber >= 0.0);
    assert!(point.bler >= 0.0 && point.bler <= 1.0);
}

/// Smoke test: construct 5G NR LDPC rate-matched (1024, 646), run 3 frames at high SNR.
#[test]
fn smoke_nr5g_ldpc_1024_high_snr() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 646);
    assert_eq!(rm_code.n(), 1024);
    assert_eq!(rm_code.k(), 646);

    let rm_code_for_decoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code_for_decoder);
    let channel = BpskAwgnChannel;

    let config = SimulationConfig {
        eb_n0_range_db: vec![4.0],
        min_errors: 1_000_000,
        max_frames: 3,
        max_decoder_iterations: 50,
        rng_seed: Some(456),
        output_path: None,
    };

    let results = SimulationRunner::run_coded_iterative(&rm_code, &mut decoder, &channel, &config);
    assert_eq!(results.points.len(), 1);

    let point = &results.points[0];
    assert_eq!(point.num_frames, 3);
    assert!(point.ber >= 0.0);
    assert!(point.bler >= 0.0 && point.bler <= 1.0);
}

/// Verify both codes have matching parameters for fair comparison.
#[test]
fn smoke_matching_parameters() {
    let gldpc = QcGldpcCode::lentmaier_1024();
    let ldpc = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 646);

    // Same (n, k) for fair comparison
    assert_eq!(gldpc.code_n(), ldpc.n());
    assert_eq!(gldpc.code_k(), ldpc.k());

    // Verify BlockEncoder trait reports same dimensions
    assert_eq!(gldpc.n(), ldpc.n());
    assert_eq!(gldpc.k(), ldpc.k());

    // Check rate is reasonable (~0.63)
    let rate = gldpc.code_k() as f64 / gldpc.code_n() as f64;
    assert!(
        rate > 0.6 && rate < 0.7,
        "Rate {rate} not in expected range"
    );
}
