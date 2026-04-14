//! Example: Uncoded transmission over AWGN channel with soft-decision decoding.
//!
//! This example demonstrates:
//! - BPSK modulation of random bits
//! - AWGN channel simulation at various Eb/N0 values
//! - Soft-decision (LLR) and hard-decision decoding
//! - Bit error rate (BER) computation
//! - Comparison with Shannon limit
//!
//! This serves as a baseline for comparing coded vs. uncoded transmission.

use gf2_coding::info_theory::{shannon_capacity, shannon_limit};
use gf2_coding::simulation::{SimulationConfig, SimulationRunner};

fn main() {
    println!("=== Uncoded BPSK Transmission over AWGN ===\n");

    let config = SimulationConfig {
        eb_n0_range_db: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
        min_errors: 500,
        max_frames: 1_000_000,
        max_decoder_iterations: 1,
        rng_seed: None,
        output_path: None,
    };

    println!(
        "Simulating {} errors minimum per Eb/N0 point",
        config.min_errors
    );
    println!("Maximum {} frames per point\n", config.max_frames);

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    let code_rate = 1.0; // uncoded

    println!("┌──────────┬─────────────┬────────────────┬────────────┐");
    println!("│ Eb/N0 dB │     BER     │  Shannon Limit │  Gap (dB)  │");
    println!("├──────────┼─────────────┼────────────────┼────────────┤");

    for result in &results {
        let capacity = shannon_capacity(result.eb_n0_db);
        let limit = shannon_limit(code_rate);
        let gap = result.eb_n0_db - limit;

        println!(
            "│   {:5.1}  │  {:9.6}  │     {:6.4}     │   {:6.2}   │",
            result.eb_n0_db, result.ber, capacity, gap
        );
    }

    println!("└──────────┴─────────────┴────────────────┴────────────┘\n");

    // Export to CSV
    let csv = SimulationRunner::results_to_csv(&results, true);
    println!("CSV Output (copy to file for plotting):");
    println!("{}", csv);

    println!("\nNotes:");
    println!(
        "- Shannon limit for rate {} is {:.2} dB",
        code_rate,
        shannon_limit(code_rate)
    );
    println!("- At Shannon limit, capacity = rate (reliable communication theoretically possible)");
    println!("- Gap shows how far uncoded transmission is from the Shannon limit");
    println!("- Coded systems can operate closer to the Shannon limit");
}
