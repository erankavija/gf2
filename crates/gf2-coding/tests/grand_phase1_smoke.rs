//! Smoke tests for Phase 1 AWGN simulation pipeline.
//!
//! Verifies that both product code and LDPC code construction, encoding,
//! channel transmission, and decoding pipelines produce valid results
//! without running full Monte Carlo simulations.

use gf2_coding::bch::extended::ExtendedBchCode;
use gf2_coding::drm::DrmCode;
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationRunner};
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;

// =========================================================================
// Fig 3: (256, 121) eBCH product code
// =========================================================================

#[test]
fn test_fig3_ebch_product_code_construction() {
    let component = ExtendedBchCode::ebch_16_11();
    let product = ProductCode::new(component);
    assert_eq!(product.n(), 256);
    assert_eq!(product.k(), 121);
}

#[test]
fn test_fig3_ebch_product_encode_decode() {
    let component = ExtendedBchCode::ebch_16_11();
    let product = ProductCode::new(component.clone());

    let turbo_config = TurboDecoderConfig {
        max_iterations: 5,
        alpha: 0.5,
        list_size: 2,
        max_queries: 100_000,
        list_bler_threshold: None,
    };
    let turbo = TurboDecoder::new(component, turbo_config);

    let channel = BpskAwgnChannel;
    let mut rng = StdRng::seed_from_u64(12345);

    let k = product.k();
    let n = product.n();
    let rate = k as f64 / n as f64;

    // Run 3 frames at a high SNR (should decode correctly)
    let eb_n0_db = 4.0;
    for _ in 0..3 {
        let message = BitVec::random(k, &mut rng);
        let codeword = product.encode(&message);
        assert_eq!(codeword.len(), n);

        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);
        assert_eq!(llrs.len(), n);

        let result = turbo.decode(&llrs);
        assert_eq!(result.decoded_bits.len(), k);
        assert!(result.iterations > 0);
        // BER should be finite (not NaN)
        let bit_errors = count_bit_errors(&message, &result.decoded_bits);
        let ber = bit_errors as f64 / k as f64;
        assert!(ber.is_finite(), "BER must be finite, got {ber}");
    }
}

// =========================================================================
// Fig 3: (256, 121) 5G NR LDPC
// =========================================================================

#[test]
fn test_fig3_nr5g_ldpc_construction() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    assert_eq!(rm_code.n(), 256);
    assert_eq!(rm_code.k(), 121);
    assert_eq!(rm_code.params().base_graph, 2);
    assert_eq!(rm_code.params().lifting_factor, 13);
}

#[test]
fn test_fig3_nr5g_ldpc_encode_decode() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code.clone());

    let channel = BpskAwgnChannel;
    let mut rng = StdRng::seed_from_u64(54321);

    let k = rm_code.k();
    let n = rm_code.n();
    let rate = k as f64 / n as f64;

    let eb_n0_db = 4.0;
    for _ in 0..3 {
        let message = BitVec::random(k, &mut rng);
        let codeword = rm_code.encode(&message);
        assert_eq!(codeword.len(), n);

        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);
        assert_eq!(llrs.len(), n);

        decoder.reset();
        let result = decoder.decode_iterative(&llrs, 50);
        assert_eq!(result.decoded_bits.len(), k);

        let bit_errors = count_bit_errors(&message, &result.decoded_bits);
        let ber = bit_errors as f64 / k as f64;
        assert!(ber.is_finite(), "BER must be finite, got {ber}");
    }
}

#[test]
fn test_fig3_ldpc_simulation_runner() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    let encoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
    let channel = BpskAwgnChannel;

    let config = SimulationConfig {
        eb_n0_range_db: vec![3.0],
        min_errors: 1,
        max_frames: 5,
        max_decoder_iterations: 50,
        rng_seed: Some(99),
        output_path: None,
    };

    let results = SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);
    assert_eq!(results.points.len(), 1);
    let point = &results.points[0];
    assert!((point.eb_n0_db - 3.0).abs() < 1e-10);
    assert!(point.ber.is_finite());
    assert!(point.bler.is_finite());
    assert!(point.num_frames > 0);
    assert!(point.num_frames <= 5);
}

// =========================================================================
// Fig 1: (1024, 441) dRM product code
// =========================================================================

#[test]
fn test_fig1_drm_product_code_construction() {
    let component = DrmCode::drm_32_21();
    let product = ProductCode::new(component);
    assert_eq!(product.n(), 1024);
    assert_eq!(product.k(), 441);
}

#[test]
fn test_fig1_drm_product_encode_decode() {
    let component = DrmCode::drm_32_21();
    let product = ProductCode::new(component.clone());

    let turbo_config = TurboDecoderConfig {
        max_iterations: 3,
        alpha: 0.5,
        list_size: 2,
        max_queries: 100_000,
        list_bler_threshold: None,
    };
    let turbo = TurboDecoder::new(component, turbo_config);

    let channel = BpskAwgnChannel;
    let mut rng = StdRng::seed_from_u64(67890);

    let k = product.k();
    let n = product.n();
    let rate = k as f64 / n as f64;

    // Run 3 frames at high SNR
    let eb_n0_db = 4.0;
    for _ in 0..3 {
        let message = BitVec::random(k, &mut rng);
        let codeword = product.encode(&message);
        assert_eq!(codeword.len(), n);

        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);
        assert_eq!(llrs.len(), n);

        let result = turbo.decode(&llrs);
        assert_eq!(result.decoded_bits.len(), k);
        assert!(result.iterations > 0);

        let bit_errors = count_bit_errors(&message, &result.decoded_bits);
        let ber = bit_errors as f64 / k as f64;
        assert!(ber.is_finite(), "BER must be finite, got {ber}");
    }
}

// =========================================================================
// Fig 1: (1024, 441) 5G NR LDPC
// =========================================================================

#[test]
fn test_fig1_nr5g_ldpc_construction() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
    assert_eq!(rm_code.n(), 1024);
    assert_eq!(rm_code.k(), 441);
    assert_eq!(rm_code.params().base_graph, 2);
}

#[test]
fn test_fig1_nr5g_ldpc_encode_decode() {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code.clone());

    let channel = BpskAwgnChannel;
    let mut rng = StdRng::seed_from_u64(11111);

    let k = rm_code.k();
    let n = rm_code.n();
    let rate = k as f64 / n as f64;

    let eb_n0_db = 3.0;
    for _ in 0..3 {
        let message = BitVec::random(k, &mut rng);
        let codeword = rm_code.encode(&message);
        assert_eq!(codeword.len(), n);

        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);
        assert_eq!(llrs.len(), n);

        decoder.reset();
        let result = decoder.decode_iterative(&llrs, 50);
        assert_eq!(result.decoded_bits.len(), k);

        let bit_errors = count_bit_errors(&message, &result.decoded_bits);
        let ber = bit_errors as f64 / k as f64;
        assert!(ber.is_finite(), "BER must be finite, got {ber}");
    }
}

// =========================================================================
// CSV output format test
// =========================================================================

#[test]
fn test_csv_output_format() {
    use gf2_coding::simulation::SimulationResults;

    let results = SimulationResults {
        points: vec![gf2_coding::simulation::SimulationResult {
            eb_n0_db: 2.5,
            ber: 0.01,
            bler: 0.05,
            avg_iterations: Some(8.3),
            avg_queries_per_bit: Some(12.5),
            num_bits: 1000,
            num_bit_errors: 10,
            num_frames: 50,
            num_frame_errors: 3,
        }],
    };

    let csv = results.to_csv(true);
    assert!(csv.contains("eb_n0_db"));
    assert!(csv.contains("2.5"));
    assert!(csv.contains("0.01"));
}

use gf2_coding::simulation::count_bit_errors;
