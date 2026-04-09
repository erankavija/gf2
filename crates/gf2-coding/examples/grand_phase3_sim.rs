//! Phase 3 AWGN simulation runner: Fig 7 GLDPC vs LDPC comparison.
//!
//! Reproduces the BLER curves from Figure 7 of the GLDPC-SOGRAND paper,
//! comparing a (1024, 646) QC-GLDPC code against a rate-matched 5G NR LDPC
//! code at matching parameters.
//!
//! # Usage
//!
//! ```bash
//! # Quick mode (default): few frames per SNR point for sanity checking
//! cargo run -p gf2-coding --example grand_phase3_sim --release
//!
//! # Full mode: production run with many frames for publishable curves
//! cargo run -p gf2-coding --example grand_phase3_sim --release -- --full
//! ```

use gf2_coding::gldpc::{GldpcDecoder, QcGldpcCode};
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::simulation::{
    BpskAwgnChannel, SimulationConfig, SimulationResults, SimulationRunner,
};
use std::path::PathBuf;

fn main() {
    let full_mode = std::env::args().any(|a| a == "--full");
    let moderate_mode = std::env::args().any(|a| a == "--moderate");

    println!("=== Phase 3: Fig 7 GLDPC vs LDPC AWGN Simulation ===");
    let mode_name = if full_mode {
        "FULL"
    } else if moderate_mode {
        "MODERATE"
    } else {
        "QUICK"
    };
    println!("Mode: {mode_name}");
    println!();

    // Eb/N0 sweep: 0 to 4 dB in 0.5 dB steps
    let eb_n0_range: Vec<f64> = (0..=8).map(|i| i as f64 * 0.5).collect();

    let channel = BpskAwgnChannel;

    // ---------------------------------------------------------------
    // GLDPC: (1024, 646) QC-GLDPC with eBCH(32,26) component
    // ---------------------------------------------------------------
    println!("Constructing QC-GLDPC (1024, 646) code...");
    let gldpc_code = QcGldpcCode::lentmaier_1024();
    assert_eq!(gldpc_code.code_n(), 1024);
    assert_eq!(gldpc_code.code_k(), 646);
    let gldpc_rate = gldpc_code.code_k() as f64 / gldpc_code.code_n() as f64;
    println!(
        "  n={}, k={}, rate={:.4}",
        gldpc_code.code_n(),
        gldpc_code.code_k(),
        gldpc_rate
    );

    let gldpc_config = SimulationConfig {
        eb_n0_range_db: eb_n0_range.clone(),
        min_errors: if full_mode {
            200
        } else if moderate_mode {
            100
        } else {
            10
        },
        max_frames: if full_mode {
            1_000_000
        } else if moderate_mode {
            500_000
        } else {
            500
        },
        max_decoder_iterations: 50,
        rng_seed: Some(42),
        output_path: Some(PathBuf::from("dev/simulation_results/fig7_gldpc_bler.csv")),
    };

    println!(
        "Running GLDPC simulation ({} SNR points)...",
        eb_n0_range.len()
    );
    let gldpc_code_for_factory = gldpc_code.clone();
    let gldpc_results = SimulationRunner::run_coded_iterative_parallel(
        &gldpc_code,
        || GldpcDecoder::new(gldpc_code_for_factory.clone()),
        &channel,
        &gldpc_config,
    );
    println!("GLDPC simulation complete.");
    println!();

    // ---------------------------------------------------------------
    // LDPC: 5G NR rate-matched (1024, 646), BG1 (rate ~0.63)
    // ---------------------------------------------------------------
    println!("Constructing 5G NR LDPC rate-matched (1024, 646) code...");
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(1, 1024, 646);
    println!(
        "  n={}, k={}, rate={:.4}",
        rm_code.n(),
        rm_code.k(),
        rm_code.k() as f64 / rm_code.n() as f64
    );

    let ldpc_config = SimulationConfig {
        eb_n0_range_db: eb_n0_range.clone(),
        min_errors: if full_mode {
            200
        } else if moderate_mode {
            100
        } else {
            10
        },
        max_frames: if full_mode {
            1_000_000
        } else if moderate_mode {
            500_000
        } else {
            500
        },
        max_decoder_iterations: 50,
        rng_seed: Some(42),
        output_path: Some(PathBuf::from("dev/simulation_results/fig7_ldpc_bler.csv")),
    };

    println!(
        "Running LDPC simulation ({} SNR points)...",
        eb_n0_range.len()
    );
    let rm_code_for_factory = rm_code.clone();
    let ldpc_results = SimulationRunner::run_coded_iterative_parallel(
        &rm_code,
        || Nr5gRateMatchedDecoder::new(rm_code_for_factory.clone()),
        &channel,
        &ldpc_config,
    );
    println!("LDPC simulation complete.");
    println!();

    // ---------------------------------------------------------------
    // Results table
    // ---------------------------------------------------------------
    print_results_table(&gldpc_results, &ldpc_results);

    // ---------------------------------------------------------------
    // Compare against reference data
    // ---------------------------------------------------------------
    compare_with_reference(&gldpc_results, &ldpc_results);

    // Save JSON alongside CSV (CSV is saved via output_path in SimulationConfig)
    let gldpc_json = gldpc_results.to_json();
    std::fs::write("dev/simulation_results/fig7_gldpc_bler.json", &gldpc_json)
        .expect("Failed to write GLDPC JSON");
    let ldpc_json = ldpc_results.to_json();
    std::fs::write("dev/simulation_results/fig7_ldpc_bler.json", &ldpc_json)
        .expect("Failed to write LDPC JSON");

    // Save durable comparison report
    let report = build_comparison_report(&gldpc_results, &ldpc_results);
    println!("{report}");
    let report_path = "dev/simulation_results/phase3_comparison_report.txt";
    std::fs::write(report_path, &report).expect("Failed to write comparison report");
    println!("Comparison report saved to {report_path}");

    println!();
    println!("Results saved to:");
    println!("  dev/simulation_results/fig7_gldpc_bler.{{csv,json}}");
    println!("  dev/simulation_results/fig7_ldpc_bler.{{csv,json}}");
}

fn print_results_table(gldpc: &SimulationResults, ldpc: &SimulationResults) {
    println!("=== BLER Comparison: GLDPC vs LDPC ===");
    println!();
    println!(
        "{:>10} | {:>12} {:>10} {:>8} | {:>12} {:>10} {:>8}",
        "Eb/N0 (dB)", "GLDPC BLER", "Avg Iter", "Frames", "LDPC BLER", "Avg Iter", "Frames"
    );
    println!("{}", "-".repeat(88));

    for (g, l) in gldpc.points.iter().zip(ldpc.points.iter()) {
        let g_iter = g
            .avg_iterations
            .map_or("-".to_string(), |v| format!("{:.1}", v));
        let l_iter = l
            .avg_iterations
            .map_or("-".to_string(), |v| format!("{:.1}", v));
        println!(
            "{:>10.2} | {:>12.6e} {:>10} {:>8} | {:>12.6e} {:>10} {:>8}",
            g.eb_n0_db, g.bler, g_iter, g.num_frames, l.bler, l_iter, l.num_frames
        );
    }
    println!();
}

fn compare_with_reference(gldpc: &SimulationResults, ldpc: &SimulationResults) {
    let ref_path = "dev/reference_data/fig_gldpc_sogrand.csv";
    let ref_data = match std::fs::read_to_string(ref_path) {
        Ok(data) => data,
        Err(e) => {
            println!(
                "WARNING: Could not load reference data from {}: {}",
                ref_path, e
            );
            return;
        }
    };

    // Parse reference LDPC BP BLER values
    let mut ref_ldpc_bler: Vec<(f64, f64)> = Vec::new();
    let mut ref_gldpc_bler: Vec<(f64, f64)> = Vec::new();

    for line in ref_data.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 5 {
            continue;
        }
        let metric = fields[1];
        if metric != "BLER_or_BER" {
            continue;
        }
        let eb_n0: f64 = match fields[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let value: f64 = match fields[3].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let decoder = fields[4];

        if decoder == "LDPC_BP" {
            ref_ldpc_bler.push((eb_n0, value));
        } else if decoder == "eBCH_GLDPC" {
            ref_gldpc_bler.push((eb_n0, value));
        }
    }

    println!("=== Comparison with Reference Data (Fig 7) ===");
    println!();

    if !ref_ldpc_bler.is_empty() {
        println!("LDPC BP reference points:");
        for &(snr, ref_bler) in &ref_ldpc_bler {
            if let Some(sim_point) = ldpc.points.iter().find(|p| (p.eb_n0_db - snr).abs() < 0.01) {
                let ratio = if ref_bler > 0.0 {
                    sim_point.bler / ref_bler
                } else {
                    f64::NAN
                };
                println!(
                    "  Eb/N0={:.2} dB: ref={:.6e}, sim={:.6e}, ratio={:.3}",
                    snr, ref_bler, sim_point.bler, ratio
                );
            }
        }
        println!();
    }

    if !ref_gldpc_bler.is_empty() {
        println!("eBCH GLDPC reference points:");
        for &(snr, ref_bler) in &ref_gldpc_bler {
            if let Some(sim_point) = gldpc
                .points
                .iter()
                .find(|p| (p.eb_n0_db - snr).abs() < 0.01)
            {
                let ratio = if ref_bler > 0.0 {
                    sim_point.bler / ref_bler
                } else {
                    f64::NAN
                };
                println!(
                    "  Eb/N0={:.2} dB: ref={:.6e}, sim={:.6e}, ratio={:.3}",
                    snr, ref_bler, sim_point.bler, ratio
                );
            }
        }
        println!();
    }
}

fn build_comparison_report(gldpc: &SimulationResults, ldpc: &SimulationResults) -> String {
    let mut report = String::new();
    report.push_str("Phase 3 AWGN Simulation Results — Comparison Report\n");
    report.push_str("===================================================\n\n");

    report.push_str("Fig 7: (1024, 646) QC-GLDPC vs 5G NR LDPC (NMS α=0.75)\n\n");
    report.push_str(&format!(
        "{:>8} | {:>14} {:>10} {:>8} | {:>14} {:>10} {:>8}\n",
        "Eb/N0", "GLDPC BLER", "Avg Iter", "Frames", "LDPC BLER", "Avg Iter", "Frames"
    ));
    report.push_str(&format!("{}\n", "-".repeat(88)));

    for (g, l) in gldpc.points.iter().zip(ldpc.points.iter()) {
        let g_iter = g
            .avg_iterations
            .map_or("-".to_string(), |v| format!("{:.1}", v));
        let l_iter = l
            .avg_iterations
            .map_or("-".to_string(), |v| format!("{:.1}", v));
        report.push_str(&format!(
            "{:>8.2} | {:>14.6e} {:>10} {:>8} | {:>14.6e} {:>10} {:>8}\n",
            g.eb_n0_db, g.bler, g_iter, g.num_frames, l.bler, l_iter, l.num_frames
        ));
    }

    // Reference comparison
    let ref_path = "dev/reference_data/fig_gldpc_sogrand.csv";
    if let Ok(ref_data) = std::fs::read_to_string(ref_path) {
        report.push_str("\nReference Data Comparison (paper's Fig 7 curves)\n");
        report.push_str(&format!("{}\n", "-".repeat(60)));

        for decoder_label in ["LDPC_BP", "eBCH_GLDPC"] {
            let sim_data = if decoder_label == "LDPC_BP" {
                ldpc
            } else {
                gldpc
            };
            report.push_str(&format!("  {} (sim vs paper):\n", decoder_label));
            for line in ref_data.lines().skip(1) {
                let fields: Vec<&str> = line.split(',').collect();
                if fields.len() >= 5 && fields[1] == "BLER_or_BER" && fields[4] == decoder_label {
                    if let (Ok(snr), Ok(ref_val)) =
                        (fields[2].parse::<f64>(), fields[3].parse::<f64>())
                    {
                        if let Some(sim_pt) = sim_data
                            .points
                            .iter()
                            .find(|p| (p.eb_n0_db - snr).abs() < 0.01)
                        {
                            let ratio_db = if sim_pt.bler > 0.0 && ref_val > 0.0 {
                                10.0 * (sim_pt.bler / ref_val).log10()
                            } else {
                                f64::NAN
                            };
                            report.push_str(&format!(
                                "    Eb/N0={:.1}: sim={:.4e}, ref={:.4e}, ratio={:.1} dB\n",
                                snr, sim_pt.bler, ref_val, ratio_db
                            ));
                        }
                    }
                }
            }
        }
    }

    report
}
