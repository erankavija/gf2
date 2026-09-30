//! Example: Uncoded transmission over AWGN channel with soft-decision decoding.
//!
//! This example demonstrates:
//! - BPSK modulation of random bits
//! - AWGN channel simulation at various Eb/N0 values
//! - Soft-decision (LLR) and hard-decision decoding
//! - Bit error rate (BER) computation
//! - BI-AWGN capacity at each point and the Shannon limits of coded rates
//!
//! This serves as a baseline for comparing coded vs. uncoded transmission.

use gf2_coding::info_theory::{bi_awgn_capacity, ebn0_to_esn0, shannon_limit};
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
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    };

    println!(
        "Simulating {} errors minimum per Eb/N0 point",
        config.min_errors
    );
    println!("Maximum {} frames per point\n", config.max_frames);

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    let code_rate = 1.0; // uncoded: Es/N0 = Eb/N0

    println!("┌──────────┬─────────────┬────────────────┐");
    println!("│ Eb/N0 dB │     BER     │ Capacity (bit) │");
    println!("├──────────┼─────────────┼────────────────┤");

    for result in &results {
        let capacity = bi_awgn_capacity(ebn0_to_esn0(result.eb_n0_db, 1, code_rate));

        println!(
            "│   {:5.1}  │  {:9.6}  │     {:6.4}     │",
            result.eb_n0_db, result.ber, capacity
        );
    }

    println!("└──────────┴─────────────┴────────────────┘\n");

    // Export to CSV
    let csv = SimulationRunner::results_to_csv(&results, true);
    println!("CSV Output (copy to file for plotting):");
    println!("{}", csv);

    println!("\nNotes:");
    println!("- Capacity: BI-AWGN capacity at Es/N0 = Eb/N0 (rate 1)");
    println!("- Rate 1 has no finite Shannon limit: capacity < 1 bit at every SNR");
    for rate in [1.0 / 3.0, 0.5, 0.75, 0.9] {
        println!(
            "- Shannon limit for rate {:.3} is {:.2} dB Eb/N0",
            rate,
            shannon_limit(rate)
        );
    }
    println!("- Coded systems trade rate for operation closer to these limits");
}
