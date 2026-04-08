//! Phase 1 AWGN simulation runners for GRAND paper Figs 1 and 3.
//!
//! Runs product code (turbo-SOGRAND) vs 5G NR LDPC simulations at matched
//! code parameters:
//!
//! - **Fig 3**: (256, 121) eBCH product code vs 5G NR LDPC (BG2)
//! - **Fig 1**: (1024, 441) dRM product code vs 5G NR LDPC (BG2)
//!
//! # Usage
//!
//! ```bash
//! # Quick mode (few frames, fast sanity check)
//! cargo run -p gf2-coding --example grand_phase1_sims --release
//!
//! # Moderate mode (≥100 frame errors per SNR point)
//! cargo run -p gf2-coding --example grand_phase1_sims --release -- --moderate
//!
//! # Full production mode (≥200 frame errors, publishable statistics)
//! cargo run -p gf2-coding --example grand_phase1_sims --release -- --full
//! ```

use gf2_coding::bch::extended::ExtendedBchCode;
use gf2_coding::drm::DrmCode;
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};
use gf2_coding::simulation::{
    BpskAwgnChannel, SimulationConfig, SimulationResults, SimulationRunner,
};
use std::path::PathBuf;

/// Eb/N0 sweep for Fig 3: 0–4 dB in 0.5 dB steps.
fn fig3_snr_points() -> Vec<f64> {
    (0..=8).map(|i| i as f64 * 0.5).collect()
}

/// Eb/N0 sweep for Fig 1: 0–2.5 dB in 0.5 dB steps (per paper).
fn fig1_snr_points() -> Vec<f64> {
    (0..=5).map(|i| i as f64 * 0.5).collect()
}

/// Save results to CSV and JSON, creating parent directories as needed.
fn save_results(results: &SimulationResults, path: &str) {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create output directory");
    }
    results.write_to(&path);
    eprintln!("  Saved CSV: {}", path.display());
    let json_path = path.with_extension("json");
    let json = results.to_json();
    std::fs::write(&json_path, json).expect("Failed to write JSON output");
    eprintln!("  Saved JSON: {}", json_path.display());
}

// =========================================================================
// Fig 3: (256, 121) eBCH product vs 5G NR LDPC
// =========================================================================

fn run_fig3_product(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 3: (256,121) eBCH Product Code (turbo-SOGRAND) ===");
    let component = ExtendedBchCode::ebch_16_11();
    let product = ProductCode::new(component.clone());
    assert_eq!(product.n(), 256);
    assert_eq!(product.k(), 121);

    let turbo_config = TurboDecoderConfig {
        max_iterations: 20,
        alpha: 0.5,
        list_size: 4,
        max_queries: 1_000_000,
        list_bler_threshold: None,
    };
    let turbo = TurboDecoder::new(component, turbo_config);
    let channel = BpskAwgnChannel;

    SimulationRunner::run_with_decoder(&product, |llrs| turbo.decode(llrs).into(), &channel, config)
}

fn run_fig3_ldpc_nms(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 3: (256,121) 5G NR LDPC normalized min-sum α=0.75 ===");
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    let encoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
    let channel = BpskAwgnChannel;
    SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, config)
}

fn run_fig3_ldpc_sp(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 3: (256,121) 5G NR LDPC sum-product BP ===");
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    let encoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::with_algorithm(rm_code, DecoderAlgorithm::SumProduct);
    let channel = BpskAwgnChannel;
    SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, config)
}

// =========================================================================
// Fig 1: (1024, 441) dRM product vs 5G NR LDPC
// =========================================================================

fn run_fig1_product(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 1: (1024,441) dRM Product Code (turbo-SOGRAND) ===");
    let component = DrmCode::drm_32_21();
    let product = ProductCode::new(component.clone());
    assert_eq!(product.n(), 1024);
    assert_eq!(product.k(), 441);

    let turbo_config = TurboDecoderConfig {
        max_iterations: 20,
        alpha: 0.5,
        list_size: 4,
        max_queries: 1_000_000,
        list_bler_threshold: None,
    };
    let turbo = TurboDecoder::new(component, turbo_config);
    let channel = BpskAwgnChannel;

    SimulationRunner::run_with_decoder(&product, |llrs| turbo.decode(llrs).into(), &channel, config)
}

fn run_fig1_ldpc_nms(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 1: (1024,441) 5G NR LDPC normalized min-sum α=0.75 ===");
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
    let encoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
    let channel = BpskAwgnChannel;
    SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, config)
}

fn run_fig1_ldpc_sp(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 1: (1024,441) 5G NR LDPC sum-product BP ===");
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
    let encoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::with_algorithm(rm_code, DecoderAlgorithm::SumProduct);
    let channel = BpskAwgnChannel;
    SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, config)
}

/// Creates a simulation config for the given SNR points and output file.
fn make_config(
    snr_points: Vec<f64>,
    min_errors: usize,
    max_frames: usize,
    output_csv: &str,
) -> SimulationConfig {
    SimulationConfig {
        eb_n0_range_db: snr_points,
        min_errors,
        max_frames,
        max_decoder_iterations: 50,
        rng_seed: Some(42),
        output_path: Some(PathBuf::from(output_csv)),
    }
}

fn main() {
    let full_mode = std::env::args().any(|a| a == "--full");
    let moderate_mode = std::env::args().any(|a| a == "--moderate");

    let (min_errors, max_frames, label) = if full_mode {
        (200, 1_000_000_000, "FULL")
    } else if moderate_mode {
        (100, 500_000, "MODERATE")
    } else {
        (10, 1_000, "QUICK")
    };

    eprintln!("Phase 1 AWGN Simulations ({label} mode)");
    eprintln!("  min_errors={min_errors}, max_frames={max_frames}");
    eprintln!();

    std::fs::create_dir_all("dev/simulation_results").ok();

    // --- Fig 3: (256,121) ---
    let fig3_product = run_fig3_product(&make_config(
        fig3_snr_points(),
        min_errors,
        max_frames,
        "dev/simulation_results/fig3_ebch_product_256_121.csv",
    ));
    save_results(
        &fig3_product,
        "dev/simulation_results/fig3_ebch_product_256_121.csv",
    );

    let fig3_ldpc_nms = run_fig3_ldpc_nms(&make_config(
        fig3_snr_points(),
        min_errors,
        max_frames,
        "dev/simulation_results/fig3_ldpc_nms_256_121.csv",
    ));
    save_results(
        &fig3_ldpc_nms,
        "dev/simulation_results/fig3_ldpc_nms_256_121.csv",
    );

    let fig3_ldpc_sp = run_fig3_ldpc_sp(&make_config(
        fig3_snr_points(),
        min_errors,
        max_frames,
        "dev/simulation_results/fig3_ldpc_sp_256_121.csv",
    ));
    save_results(
        &fig3_ldpc_sp,
        "dev/simulation_results/fig3_ldpc_sp_256_121.csv",
    );

    // --- Fig 1: (1024,441) ---
    let fig1_product = run_fig1_product(&make_config(
        fig1_snr_points(),
        min_errors,
        max_frames,
        "dev/simulation_results/fig1_drm_product_1024_441.csv",
    ));
    save_results(
        &fig1_product,
        "dev/simulation_results/fig1_drm_product_1024_441.csv",
    );

    let fig1_ldpc_nms = run_fig1_ldpc_nms(&make_config(
        fig1_snr_points(),
        min_errors,
        max_frames,
        "dev/simulation_results/fig1_ldpc_nms_1024_441.csv",
    ));
    save_results(
        &fig1_ldpc_nms,
        "dev/simulation_results/fig1_ldpc_nms_1024_441.csv",
    );

    let fig1_ldpc_sp = run_fig1_ldpc_sp(&make_config(
        fig1_snr_points(),
        min_errors,
        max_frames,
        "dev/simulation_results/fig1_ldpc_sp_1024_441.csv",
    ));
    save_results(
        &fig1_ldpc_sp,
        "dev/simulation_results/fig1_ldpc_sp_1024_441.csv",
    );

    // --- Comparison report ---
    let report = build_comparison_report(
        &fig3_product,
        &fig3_ldpc_nms,
        &fig3_ldpc_sp,
        &fig1_product,
        &fig1_ldpc_nms,
        &fig1_ldpc_sp,
    );
    println!("{report}");

    let report_path = "dev/simulation_results/phase1_comparison_report.txt";
    std::fs::write(report_path, &report).expect("Failed to write comparison report");
    eprintln!("Comparison report saved to {report_path}");
}

fn build_comparison_report(
    fig3_product: &SimulationResults,
    fig3_ldpc_nms: &SimulationResults,
    fig3_ldpc_sp: &SimulationResults,
    fig1_product: &SimulationResults,
    fig1_ldpc_nms: &SimulationResults,
    fig1_ldpc_sp: &SimulationResults,
) -> String {
    let mut report = String::new();
    report.push_str("Phase 1 AWGN Simulation Results — Comparison Report\n");
    report.push_str("===================================================\n\n");

    // Fig 3 table
    report.push_str("Fig 3: (256,121) eBCH Product Code vs 5G NR LDPC\n");
    report.push_str(&format!(
        "{:>8} | {:>14} | {:>14} | {:>14}\n",
        "Eb/N0", "Product BLER", "LDPC NMS BLER", "LDPC SP BLER"
    ));
    report.push_str(&format!("{}\n", "-".repeat(60)));
    for (i, p) in fig3_product.points.iter().enumerate() {
        let nms = fig3_ldpc_nms.points.get(i);
        let sp = fig3_ldpc_sp.points.get(i);
        report.push_str(&format!(
            "{:>8.1} | {:>14.4e} | {:>14.4e} | {:>14.4e}\n",
            p.eb_n0_db,
            p.bler,
            nms.map_or(f64::NAN, |x| x.bler),
            sp.map_or(f64::NAN, |x| x.bler),
        ));
    }

    // Product vs LDPC crossover
    report.push_str("\nProduct code outperforms LDPC (NMS) at:\n");
    for (i, p) in fig3_product.points.iter().enumerate() {
        if let Some(nms) = fig3_ldpc_nms.points.get(i) {
            if p.bler < nms.bler && p.bler > 0.0 {
                report.push_str(&format!(
                    "  Eb/N0={:.1} dB: Product BLER={:.4e} < LDPC NMS BLER={:.4e}\n",
                    p.eb_n0_db, p.bler, nms.bler
                ));
            }
        }
    }

    report.push('\n');

    // Fig 1 table
    report.push_str("Fig 1: (1024,441) dRM Product Code vs 5G NR LDPC\n");
    report.push_str(&format!(
        "{:>8} | {:>14} | {:>14} | {:>14}\n",
        "Eb/N0", "Product BLER", "LDPC NMS BLER", "LDPC SP BLER"
    ));
    report.push_str(&format!("{}\n", "-".repeat(60)));
    for (i, p) in fig1_product.points.iter().enumerate() {
        let nms = fig1_ldpc_nms.points.get(i);
        let sp = fig1_ldpc_sp.points.get(i);
        report.push_str(&format!(
            "{:>8.1} | {:>14.4e} | {:>14.4e} | {:>14.4e}\n",
            p.eb_n0_db,
            p.bler,
            nms.map_or(f64::NAN, |x| x.bler),
            sp.map_or(f64::NAN, |x| x.bler),
        ));
    }

    // Reference comparison
    report.push_str("\nReference Data Comparison (paper's LDPC BP curve)\n");
    report.push_str(&format!("{}\n", "-".repeat(60)));
    append_reference_comparison(
        &mut report,
        "dev/reference_data/fig_prod_ebch_16x11.csv",
        fig3_ldpc_sp,
        "Fig 3 LDPC SP vs paper LDPC_BP",
        "LDPC_BP",
    );
    append_reference_comparison(
        &mut report,
        "dev/reference_data/fig_prod_ebch_16x11.csv",
        fig3_product,
        "Fig 3 Product vs paper eBCH_prod_SOGRAND",
        "eBCH_prod_SOGRAND",
    );
    append_reference_comparison(
        &mut report,
        "dev/reference_data/fig_prod_drm_32x21.csv",
        fig1_ldpc_sp,
        "Fig 1 LDPC SP vs paper LDPC_BP",
        "LDPC_BP",
    );

    report
}

fn append_reference_comparison(
    report: &mut String,
    ref_path: &str,
    results: &SimulationResults,
    label: &str,
    decoder_filter: &str,
) {
    let Ok(content) = std::fs::read_to_string(ref_path) else {
        report.push_str(&format!("  {label}: reference file not found\n"));
        return;
    };
    report.push_str(&format!("  {label}:\n"));
    report.push_str(&format!(
        "    {:>8} | {:>12} | {:>12} | {:>10}\n",
        "Eb/N0", "Ours", "Paper", "Ratio(dB)"
    ));
    for point in &results.points {
        let mut best_ref: Option<f64> = None;
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() >= 5 {
                if let (Ok(snr), Ok(val)) = (fields[2].parse::<f64>(), fields[3].parse::<f64>()) {
                    if (snr - point.eb_n0_db).abs() < 0.26
                        && fields[4] == decoder_filter
                        && fields[1].contains("BLER")
                    {
                        best_ref = Some(val);
                    }
                }
            }
        }
        if let Some(ref_val) = best_ref {
            let delta_db = if point.bler > 0.0 && ref_val > 0.0 {
                10.0 * (point.bler / ref_val).log10()
            } else {
                f64::NAN
            };
            report.push_str(&format!(
                "    {:>8.1} | {:>12.4e} | {:>12.4e} | {:>8.2} dB\n",
                point.eb_n0_db, point.bler, ref_val, delta_db
            ));
        }
    }
}
