//! Integration tests for uncoded transmission baseline performance.
//!
//! These tests establish reference BER values for uncoded BPSK transmission
//! over AWGN channels, serving as a baseline for comparing coded systems.

use gf2_coding::channel::AwgnChannel;
use gf2_coding::simulation::{SimulationConfig, SimulationRunner};

#[test]
fn test_uncoded_ber_at_high_snr() {
    // At high SNR (10 dB), BER should be very low
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range = vec![10.0];
    config.min_errors = 10;
    config.max_trials = 100_000;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    assert_eq!(results.len(), 1);
    assert!(
        results[0].ber < 0.001,
        "BER at 10 dB should be < 0.1%, got {}",
        results[0].ber
    );
}

#[test]
fn test_uncoded_ber_decreases_monotonically() {
    // BER should decrease as Eb/N0 increases
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range = vec![0.0, 3.0, 6.0];
    config.min_errors = 100;
    config.max_trials = 100_000;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    assert_eq!(results.len(), 3);
    assert!(
        results[1].ber < results[0].ber,
        "BER should decrease: {} > {}",
        results[0].ber,
        results[1].ber
    );
    assert!(
        results[2].ber < results[1].ber,
        "BER should decrease: {} > {}",
        results[1].ber,
        results[2].ber
    );
}

#[test]
fn test_uncoded_ber_reasonable_values() {
    // Check that BER values are in expected ranges for uncoded BPSK
    // These are approximate bounds based on Q-function
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range = vec![3.0, 6.0];
    config.min_errors = 200;
    config.max_trials = 500_000;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    // At 3 dB: BER ~ 0.004 (Q(sqrt(6)))
    // Allow wider tolerance due to Monte Carlo variance
    assert!(
        results[0].ber > 0.001 && results[0].ber < 0.03,
        "BER at 3 dB should be around 0.004, got {}",
        results[0].ber
    );

    // At 6 dB: BER ~ 0.000023 (Q(sqrt(12)))
    // With only 200 errors minimum, variance is high
    assert!(
        results[1].ber < 0.005,
        "BER at 6 dB should be small, got {}",
        results[1].ber
    );
}

#[test]
fn test_ber_far_from_shannon_limit() {
    // Uncoded transmission operates far from Shannon limit
    // At Shannon limit for rate 1.0, we'd need infinite SNR
    // At practical SNRs, there's a significant gap

    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range = vec![3.0];
    config.min_errors = 100;
    config.max_trials = 100_000;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    // At 3 dB, capacity is ~0.72, far below rate 1.0
    let capacity = AwgnChannel::shannon_capacity(3.0);
    assert!(capacity < 1.0, "Capacity at 3 dB should be < 1.0");
    assert!(
        capacity > 0.7 && capacity < 0.8,
        "Capacity at 3 dB should be ~0.72, got {}",
        capacity
    );

    // BER is non-zero, showing we're operating above the Shannon limit
    assert!(
        results[0].ber > 0.001,
        "BER should be significant at 3 dB for rate 1.0"
    );
}

#[test]
fn test_csv_export_format() {
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range = vec![3.0];
    config.min_errors = 50;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    let csv = SimulationRunner::results_to_csv(&results, true);

    // Check CSV has header and data
    assert!(csv.contains("eb_n0_db"));
    assert!(csv.contains("ber"));
    assert!(csv.contains("num_bits"));
    assert!(csv.contains("num_errors"));

    // Check it has the data row
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2, "Should have header + 1 data row");
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn ber_bounded_by_half(eb_n0_db in -5.0..20.0) {
            // BER for BPSK should always be <= 0.5 (worst case is random guessing)
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range = vec![eb_n0_db];
            config.min_errors = 10;
            config.max_trials = 10_000;

            let mut rng = rand::thread_rng();
            let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

            prop_assert!(results[0].ber <= 0.5,
                "BER {} exceeds 0.5 at Eb/N0 = {} dB", results[0].ber, eb_n0_db);
            prop_assert!(results[0].ber >= 0.0,
                "BER {} is negative at Eb/N0 = {} dB", results[0].ber, eb_n0_db);
        }
    }
}
