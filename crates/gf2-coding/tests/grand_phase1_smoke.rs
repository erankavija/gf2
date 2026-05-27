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
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

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
        alpha_final: None,
        extrinsic_clamp: None,
        no_early_termination: false,
        pyndiah_extrinsic: false,
        use_bcjr: false,
        #[cfg(feature = "hip")]
        use_gpu_bcjr: false,
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
    assert_eq!(rm_code.params().lifting_factor, 22);
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
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
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
#[ignore = "slow: TurboDecoder on dRM(32,21) product code (1024,441)"]
fn test_fig1_drm_product_encode_decode() {
    let component = DrmCode::drm_32_21();
    let product = ProductCode::new(component.clone());

    let turbo_config = TurboDecoderConfig {
        max_iterations: 3,
        alpha: 0.5,
        list_size: 2,
        max_queries: 100_000,
        list_bler_threshold: None,
        alpha_final: None,
        extrinsic_clamp: None,
        no_early_termination: false,
        pyndiah_extrinsic: false,
        use_bcjr: false,
        #[cfg(feature = "hip")]
        use_gpu_bcjr: false,
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

#[derive(Debug)]
struct Phase1CsvRow {
    bler: f64,
    frame_errors: u64,
    queries_per_bit: f64,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn read_repo_text(relative: &str) -> String {
    let path = repo_path(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn normalize_snr(snr: f64) -> String {
    format!("{snr:.2}")
}

fn parse_phase1_csv(relative: &str) -> BTreeMap<String, Phase1CsvRow> {
    let text = read_repo_text(relative);
    text.lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let columns: Vec<_> = line.split(',').collect();
            assert_eq!(
                columns.len(),
                9,
                "unexpected CSV shape in {relative}: {line}"
            );
            let snr = columns[0]
                .parse::<f64>()
                .unwrap_or_else(|err| panic!("invalid SNR in {relative}: {err}"));
            (
                normalize_snr(snr),
                Phase1CsvRow {
                    bler: columns[2]
                        .parse::<f64>()
                        .unwrap_or_else(|err| panic!("invalid BLER in {relative}: {err}")),
                    frame_errors: columns[6].parse::<u64>().unwrap_or_else(|err| {
                        panic!("invalid frame error count in {relative}: {err}")
                    }),
                    queries_per_bit: columns[8]
                        .parse::<f64>()
                        .unwrap_or_else(|err| panic!("invalid queries/bit in {relative}: {err}")),
                },
            )
        })
        .collect()
}

fn parse_report_table(report: &str, heading: &str) -> BTreeMap<String, Vec<String>> {
    let mut in_section = false;
    let mut rows = BTreeMap::new();

    for line in report.lines() {
        if line == heading {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if !in_section || !line.trim_start().starts_with('|') {
            continue;
        }

        let columns: Vec<String> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|column| column.trim().to_string())
            .collect();
        if columns.is_empty()
            || columns[0] == "Eb/N0 (dB)"
            || columns
                .iter()
                .all(|column| column.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        rows.insert(columns[0].clone(), columns);
    }

    rows
}

fn parse_report_value(cell: &str) -> Option<f64> {
    if cell == "—" {
        None
    } else if let Some(value) = cell.strip_suffix('K') {
        Some(
            value
                .parse::<f64>()
                .unwrap_or_else(|err| panic!("invalid K-suffixed value {cell}: {err}"))
                * 1000.0,
        )
    } else {
        Some(
            cell.parse::<f64>()
                .unwrap_or_else(|err| panic!("invalid numeric report value {cell}: {err}")),
        )
    }
}

fn assert_approx_eq(actual: f64, expected: f64, tolerance: f64) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tolerance,
        "expected {expected} within {tolerance}, got {actual} (diff {diff})"
    );
}

#[test]
fn test_phase1_canonical_campaign_surface() {
    assert!(repo_path("dev/campaigns/phase1_fig1.toml").is_file());
    assert!(repo_path("dev/campaigns/phase1_fig3.toml").is_file());
    assert!(!repo_path("dev/campaigns/phase1_fig1_highstat.toml").exists());
    assert!(!repo_path("dev/campaigns/phase1_fig3_sogrand.toml").exists());
}

#[test]
fn test_phase1_canonical_result_surface() {
    assert!(repo_path("dev/simulation_results/fig1_drm_product.csv").is_file());
    assert!(repo_path("dev/simulation_results/fig1_drm_product.json").is_file());
    assert!(repo_path("dev/simulation_results/fig3_ebch_product.csv").is_file());
    assert!(repo_path("dev/simulation_results/fig3_ebch_product.json").is_file());
    assert!(repo_path("dev/simulation_results/phase1_final/fig3_sogrand_stdout.log").is_file());

    assert!(!repo_path("dev/simulation_results/phase1_final/fig1_drm_product.csv").exists());
    assert!(!repo_path("dev/simulation_results/phase1_final/fig1_drm_product.json").exists());
    assert!(!repo_path("dev/simulation_results/phase1_final/fig3_ebch_sogrand.csv").exists());
    assert!(!repo_path("dev/simulation_results/phase1_final/fig3_ebch_sogrand.json").exists());
}

#[test]
fn test_phase1_report_matches_figure1_artifacts() {
    let report = read_repo_text("dev/simulation_results/phase1_comparison_report.md");
    let product = parse_phase1_csv("dev/simulation_results/fig1_drm_product.csv");
    let sp = parse_phase1_csv("dev/simulation_results/fig1_ldpc_sp.csv");
    let nms = parse_phase1_csv("dev/simulation_results/fig1_ldpc_nms.csv");
    let table = parse_report_table(&report, "## Figure 1: dRM(32,21)^2 product code vs LDPC");

    for snr in ["0.50", "0.75", "1.00", "1.25", "1.50", "1.75", "2.00"] {
        let row = table
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 1 row {snr}"));
        let product_row = product
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 1 product CSV row {snr}"));
        assert_approx_eq(parse_report_value(&row[1]).unwrap(), product_row.bler, 5e-4);
        assert_eq!(row[2], product_row.frame_errors.to_string());
    }

    for snr in ["0.50", "1.00", "1.50", "2.00"] {
        let row = table
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 1 row {snr}"));
        let sp_row = sp
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 1 LDPC SP row {snr}"));
        let nms_row = nms
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 1 LDPC NMS row {snr}"));
        assert_approx_eq(parse_report_value(&row[3]).unwrap(), sp_row.bler, 5e-4);
        assert_approx_eq(parse_report_value(&row[4]).unwrap(), nms_row.bler, 5e-4);
    }

    for snr in ["0.75", "1.25", "1.75"] {
        let row = table
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 1 row {snr}"));
        assert_eq!(row[3], "—");
        assert_eq!(row[4], "—");
    }
}

#[test]
fn test_phase1_report_matches_figure3_artifacts() {
    let report = read_repo_text("dev/simulation_results/phase1_comparison_report.md");
    let product = parse_phase1_csv("dev/simulation_results/fig3_ebch_product.csv");
    let sp = parse_phase1_csv("dev/simulation_results/fig3_ldpc_sp.csv");
    let nms = parse_phase1_csv("dev/simulation_results/fig3_ldpc_nms.csv");
    let table = parse_report_table(&report, "## Figure 3: eBCH(16,11)^2 product code vs LDPC");

    for snr in ["0.50", "1.00", "1.50", "2.00", "2.50", "3.00", "3.50"] {
        let row = table
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 3 row {snr}"));
        let product_row = product
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 3 product CSV row {snr}"));
        let sp_row = sp
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 3 LDPC SP row {snr}"));
        let nms_row = nms
            .get(snr)
            .unwrap_or_else(|| panic!("missing Figure 3 LDPC NMS row {snr}"));

        assert_approx_eq(parse_report_value(&row[1]).unwrap(), product_row.bler, 5e-4);
        assert_approx_eq(
            parse_report_value(&row[2]).unwrap(),
            product_row.queries_per_bit,
            50.0,
        );
        assert_approx_eq(parse_report_value(&row[3]).unwrap(), sp_row.bler, 5e-4);
        assert_approx_eq(parse_report_value(&row[4]).unwrap(), nms_row.bler, 5e-4);
    }

    assert!(report.contains("`dev/campaigns/phase1_fig1.toml`, `dev/campaigns/phase1_fig3.toml`"));
    assert!(report.contains("Quick/alignment helper"));
    assert!(report.contains("`100/max_frames`"));
    assert!(!report.contains("exceeds 1/max_frames"));
    assert!(!report.contains("between 1.5 and 1.75 dB"));
    assert!(report.contains("between 1.5 and\n2.0 dB"));
    assert!(!report.contains("identical extrinsic to SOGRAND"));
    assert!(!report.contains("phase1_final/fig1_drm_product"));
}
