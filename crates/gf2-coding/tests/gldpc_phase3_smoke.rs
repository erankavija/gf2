//! Phase 3 smoke tests: GLDPC and NR LDPC pipeline verification.

use gf2_coding::gldpc::{GldpcDecoder, QcGldpcCode};
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};

#[test]
fn test_gldpc_1024_construction() {
    let code = QcGldpcCode::lentmaier_1024();
    assert_eq!(code.code_n(), 1024);
    assert_eq!(code.code_k(), 646);
}

#[test]
fn test_gldpc_1024_encode_decode_pipeline() {
    let code = QcGldpcCode::lentmaier_1024();
    let mut decoder = GldpcDecoder::new(code.clone());
    let channel = BpskAwgnChannel;

    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range_db = vec![6.0];
    config.max_frames = 3;
    config.min_errors = 1;

    let results = SimulationRunner::run_coded_iterative(&code, &mut decoder, &channel, &config);
    assert_eq!(results.points.len(), 1);
    assert!(results.points[0].num_frames > 0);
}

#[test]
fn test_nr5g_ldpc_1024_646_pipeline() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 646);
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code.clone());
    let channel = BpskAwgnChannel;

    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range_db = vec![6.0];
    config.max_frames = 3;
    config.min_errors = 1;

    let results = SimulationRunner::run_coded_iterative(&rm_code, &mut decoder, &channel, &config);
    assert_eq!(results.points.len(), 1);
    assert!(results.points[0].num_frames > 0);
}

#[test]
fn test_gldpc_and_ldpc_matching_parameters() {
    let gldpc = QcGldpcCode::lentmaier_1024();
    let ldpc = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 646);

    assert_eq!(gldpc.code_n(), 1024);
    assert_eq!(ldpc.n(), 1024);
    assert_eq!(gldpc.code_k(), ldpc.k());
}
