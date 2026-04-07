//! Phase 1 AWGN simulation runners for GRAND paper Figs 1 and 3.
//!
//! Runs product code (turbo-decoded) vs 5G NR LDPC simulations at matched
//! code parameters:
//!
//! - **Fig 3**: (256, 121) eBCH product code vs 5G NR LDPC (BG2)
//! - **Fig 1**: (1024, 441) dRM product code vs 5G NR LDPC (BG2)
//!
//! # Usage
//!
//! Quick mode (few frames, fast):
//! ```bash
//! cargo run -p gf2-coding --example grand_phase1_sims --release
//! ```
//!
//! Full production mode (many frames, slow):
//! ```bash
//! cargo run -p gf2-coding --example grand_phase1_sims --release -- --full
//! ```

use gf2_coding::bch::extended::ExtendedBchCode;
use gf2_coding::drm::DrmCode;
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};
use gf2_coding::simulation::{
    BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationResult, SimulationResults,
    SimulationRunner,
};
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::path::PathBuf;

/// Eb/N0 sweep for Fig 3: 0–4 dB in 0.5 dB steps.
fn fig3_snr_points() -> Vec<f64> {
    (0..=8).map(|i| i as f64 * 0.5).collect()
}

/// Eb/N0 sweep for Fig 1: 0–2.5 dB in 0.5 dB steps (per paper).
fn fig1_snr_points() -> Vec<f64> {
    (0..=5).map(|i| i as f64 * 0.5).collect()
}

/// Run a product code turbo decoder simulation (custom loop since TurboDecoder
/// does not implement the IterativeSoftDecoder trait).
fn run_product_sim<C: gf2_coding::product::ProductComponent + Clone>(
    product: &ProductCode<C>,
    turbo: &TurboDecoder<C>,
    config: &SimulationConfig,
) -> SimulationResults {
    let k = product.k();
    let n = product.n();
    let rate = k as f64 / n as f64;
    let channel = BpskAwgnChannel;

    let seed = config.rng_seed.unwrap_or_else(|| rand::thread_rng().gen());
    let mut rng = StdRng::seed_from_u64(seed);
    let mut points = Vec::with_capacity(config.eb_n0_range_db.len());

    for &eb_n0_db in &config.eb_n0_range_db {
        let mut total_bit_errors: usize = 0;
        let mut total_bits: usize = 0;
        let mut total_frame_errors: usize = 0;
        let mut total_frames: usize = 0;
        let mut total_iterations: usize = 0;
        let mut total_queries: usize = 0;

        while total_frame_errors < config.min_errors && total_frames < config.max_frames {
            let message = BitVec::random(k, &mut rng);
            let codeword = product.encode(&message);
            let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);

            let result = turbo.decode(&llrs);

            let bit_errors = count_bit_errors(&message, &result.decoded_bits);
            total_bit_errors += bit_errors;
            total_bits += k;
            total_frames += 1;
            if bit_errors > 0 {
                total_frame_errors += 1;
            }
            total_iterations += result.iterations;
            total_queries += result.total_queries;

            if total_frames % 100 == 0 {
                eprintln!(
                    "  [{:.1} dB] frames={}, errors={}/{}",
                    eb_n0_db, total_frames, total_frame_errors, config.min_errors
                );
            }
        }

        let ber = if total_bits > 0 {
            total_bit_errors as f64 / total_bits as f64
        } else {
            0.0
        };
        let bler = if total_frames > 0 {
            total_frame_errors as f64 / total_frames as f64
        } else {
            0.0
        };
        let avg_iterations = if total_frames > 0 {
            Some(total_iterations as f64 / total_frames as f64)
        } else {
            None
        };
        let avg_queries_per_bit = if total_bits > 0 {
            Some(total_queries as f64 / total_bits as f64)
        } else {
            None
        };

        eprintln!(
            "  [{:.1} dB] DONE: frames={}, errors={}, BER={:.2e}, BLER={:.2e}",
            eb_n0_db, total_frames, total_frame_errors, ber, bler
        );

        points.push(SimulationResult {
            eb_n0_db,
            ber,
            bler,
            avg_iterations,
            avg_queries_per_bit,
            num_bits: total_bits,
            num_bit_errors: total_bit_errors,
            num_frames: total_frames,
            num_frame_errors: total_frame_errors,
        });
    }

    SimulationResults { points }
}

/// Counts bit errors between original and decoded.
fn count_bit_errors(original: &BitVec, decoded: &BitVec) -> usize {
    if original.len() == decoded.len() {
        let mut diff = original.clone();
        diff.bit_xor_into(decoded);
        diff.count_ones()
    } else {
        let len = original.len().min(decoded.len());
        let mut errors = 0;
        for i in 0..len {
            if original.get(i) != decoded.get(i) {
                errors += 1;
            }
        }
        errors + original.len().abs_diff(decoded.len())
    }
}

/// Save results to CSV, creating parent directories as needed.
fn save_results(results: &SimulationResults, path: &str) {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create output directory");
    }
    results.write_to(&path);
    eprintln!("  Saved: {}", path.display());
}

// =========================================================================
// Fig 3: (256, 121) eBCH product vs 5G NR LDPC
// =========================================================================

fn run_fig3_product(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 3: (256,121) eBCH Product Code ===");
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

    run_product_sim(&product, &turbo, config)
}

fn run_fig3_ldpc(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 3: (256,121) 5G NR LDPC (BG2) ===");
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    assert_eq!(rm_code.n(), 256);
    assert_eq!(rm_code.k(), 121);

    let encoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
    let channel = BpskAwgnChannel;

    SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, config)
}

// =========================================================================
// Fig 1: (1024, 441) dRM product vs 5G NR LDPC
// =========================================================================

fn run_fig1_product(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 1: (1024,441) dRM Product Code ===");
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

    run_product_sim(&product, &turbo, config)
}

fn run_fig1_ldpc(config: &SimulationConfig) -> SimulationResults {
    eprintln!("=== Fig 1: (1024,441) 5G NR LDPC (BG2) ===");
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 1024, 441);
    assert_eq!(rm_code.n(), 1024);
    assert_eq!(rm_code.k(), 441);

    let encoder = rm_code.clone();
    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
    let channel = BpskAwgnChannel;

    SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, config)
}

fn main() {
    let full_mode = std::env::args().any(|a| a == "--full");

    let (min_errors, max_frames, label) = if full_mode {
        // 1B frames ensures >=100 errors even at BLER ~1e-7.
        // The loop exits as soon as min_errors is reached, so this cap
        // only matters for the lowest-BLER points.
        (100, 1_000_000_000, "FULL")
    } else {
        (10, 1_000, "QUICK")
    };

    eprintln!("Phase 1 AWGN Simulations ({label} mode)");
    eprintln!("  min_errors={min_errors}, max_frames={max_frames}");
    eprintln!();

    // Fig 3 config: 0–4 dB
    let fig3_config = SimulationConfig {
        eb_n0_range_db: fig3_snr_points(),
        min_errors,
        max_frames,
        max_decoder_iterations: 50,
        rng_seed: Some(42),
        output_path: None,
    };

    // Fig 1 config: 0–2.5 dB (per paper)
    let fig1_config = SimulationConfig {
        eb_n0_range_db: fig1_snr_points(),
        min_errors,
        max_frames,
        max_decoder_iterations: 50,
        rng_seed: Some(42),
        output_path: None,
    };

    std::fs::create_dir_all("dev/simulation_results").ok();

    // --- Fig 3: (256,121) ---
    let fig3_product = run_fig3_product(&fig3_config);
    save_results(
        &fig3_product,
        "dev/simulation_results/fig3_ebch_product_256_121.csv",
    );

    let fig3_ldpc = run_fig3_ldpc(&fig3_config);
    save_results(
        &fig3_ldpc,
        "dev/simulation_results/fig3_ldpc_nr5g_256_121.csv",
    );

    // --- Fig 1: (1024,441) ---
    let fig1_product = run_fig1_product(&fig1_config);
    save_results(
        &fig1_product,
        "dev/simulation_results/fig1_drm_product_1024_441.csv",
    );

    let fig1_ldpc = run_fig1_ldpc(&fig1_config);
    save_results(
        &fig1_ldpc,
        "dev/simulation_results/fig1_ldpc_nr5g_1024_441.csv",
    );

    // --- Summary ---
    println!();
    println!("=== Fig 3: (256,121) eBCH Product vs 5G NR LDPC ===");
    print_comparison(&fig3_product, &fig3_ldpc, "Product", "LDPC");

    println!();
    println!("=== Fig 1: (1024,441) dRM Product vs 5G NR LDPC ===");
    print_comparison(&fig1_product, &fig1_ldpc, "Product", "LDPC");

    // --- Reference data comparison ---
    println!();
    println!("=== Reference Data Comparison ===");
    println!("  (Comparing our simulation results against paper's reference curves.)");
    println!("  (Acceptance: LDPC baseline within ~0.1 dB of paper.)");
    println!();

    // Fig 3: compare both LDPC and product code curves
    compare_with_reference(
        "dev/reference_data/fig_prod_ebch_16x11.csv",
        &fig3_ldpc,
        "Fig 3 LDPC (ours vs paper LDPC_BP)",
        "LDPC",
    );
    compare_with_reference(
        "dev/reference_data/fig_prod_ebch_16x11.csv",
        &fig3_product,
        "Fig 3 Product (ours vs paper SOGRAND turbo)",
        "SOGRAND",
    );

    // Fig 1: compare LDPC and product code curves
    compare_with_reference(
        "dev/reference_data/fig_prod_drm_32x21.csv",
        &fig1_ldpc,
        "Fig 1 LDPC (ours vs paper LDPC_BP)",
        "LDPC",
    );
    compare_with_reference(
        "dev/reference_data/fig_prod_drm_32x21.csv",
        &fig1_product,
        "Fig 1 Product (ours vs paper SOGRAND turbo)",
        "SOGRAND",
    );
}

/// Load reference CSV and compare BLER at matching Eb/N0 points.
///
/// `decoder_filter` selects which decoder's curve to compare against
/// (e.g., "LDPC" matches LDPC_BP/LDPC_normMinSum, "SOGRAND" matches
/// SOGRAND turbo curves, "dRM" matches dRM product curves).
fn compare_with_reference(
    ref_path: &str,
    results: &SimulationResults,
    label: &str,
    decoder_filter: &str,
) {
    let Ok(content) = std::fs::read_to_string(ref_path) else {
        eprintln!("  {label}: reference file {ref_path} not found, skipping comparison");
        return;
    };
    println!("  {label}:");
    println!(
        "    {:>8} | {:>12} | {:>12} | {:>10}",
        "Eb/N0", "Ours(BLER)", "Ref(BLER)", "Ratio(dB)"
    );
    for point in &results.points {
        // Find closest reference BLER at this Eb/N0
        // Reference CSV format: figure,metric,eb_n0_db,value,decoder,...
        let mut best_ref: Option<f64> = None;
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() >= 5 {
                if let (Ok(snr), Ok(val)) = (fields[2].parse::<f64>(), fields[3].parse::<f64>()) {
                    if (snr - point.eb_n0_db).abs() < 0.26
                        && fields[4].contains(decoder_filter)
                        && (fields[1].contains("BLER") || fields[1].contains("BLER_or_BER"))
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
            println!(
                "    {:>8.1} | {:>12.4e} | {:>12.4e} | {:>8.2} dB",
                point.eb_n0_db, point.bler, ref_val, delta_db
            );
        }
    }
}

fn print_comparison(a: &SimulationResults, b: &SimulationResults, label_a: &str, label_b: &str) {
    println!(
        "{:>8} | {:>12} {:>12} | {:>12} {:>12}",
        "Eb/N0", "BER(A)", "BLER(A)", "BER(B)", "BLER(B)"
    );
    println!(
        "{:>8} | {:>12} {:>12} | {:>12} {:>12}",
        "(dB)", label_a, label_a, label_b, label_b
    );
    println!("{}", "-".repeat(68));
    for (pa, pb) in a.points.iter().zip(b.points.iter()) {
        println!(
            "{:8.1} | {:12.4e} {:12.4e} | {:12.4e} {:12.4e}",
            pa.eb_n0_db, pa.ber, pa.bler, pb.ber, pb.bler
        );
    }
}
