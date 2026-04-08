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
    let mut gldpc_decoder = GldpcDecoder::new(gldpc_code.clone());
    let gldpc_results = SimulationRunner::run_coded_iterative(
        &gldpc_code,
        &mut gldpc_decoder,
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
    let rm_code_for_decoder = rm_code.clone();
    let mut ldpc_decoder = Nr5gRateMatchedDecoder::new(rm_code_for_decoder);
    let ldpc_results =
        SimulationRunner::run_coded_iterative(&rm_code, &mut ldpc_decoder, &channel, &ldpc_config);
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

    // Save JSON alongside CSV
    let gldpc_json = gldpc_results.to_json();
    std::fs::write("dev/simulation_results/fig7_gldpc_bler.json", &gldpc_json)
        .expect("Failed to write GLDPC JSON");
    let ldpc_json = ldpc_results.to_json();
    std::fs::write("dev/simulation_results/fig7_ldpc_bler.json", &ldpc_json)
        .expect("Failed to write LDPC JSON");

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
