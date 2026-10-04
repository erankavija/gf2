//! Integration tests for uncoded BPSK transmission over AWGN: BER bounds and
//! CSV export.

use gf2_coding::info_theory::bi_awgn_capacity;
use gf2_coding::simulation::{SimulationConfig, SimulationRunner};

#[test]
#[ignore = "sim: uncoded BPSK BER baseline, 100 000 frames"]
fn test_uncoded_ber_at_high_snr() {
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range_db = vec![10.0];
    config.min_errors = 10;
    config.max_frames = 100_000;

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
#[ignore = "sim: BER monotonicity, 3 SNR points x 100 000 frames"]
fn test_uncoded_ber_decreases_monotonically() {
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range_db = vec![0.0, 3.0, 6.0];
    config.min_errors = 100;
    config.max_frames = 100_000;

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
#[ignore = "sim: BER precision, 500 000 frames, 200 min errors"]
fn test_uncoded_ber_reasonable_values() {
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range_db = vec![3.0, 6.0];
    config.min_errors = 200;
    config.max_frames = 500_000;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    assert!(
        results[0].ber > 0.001 && results[0].ber < 0.03,
        "BER at 3 dB should be around 0.004, got {}",
        results[0].ber
    );

    assert!(
        results[1].ber < 0.005,
        "BER at 6 dB should be small, got {}",
        results[1].ber
    );
}

#[test]
#[ignore = "sim: BER vs Shannon, 100 000 frames"]
fn test_ber_far_from_shannon_limit() {
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range_db = vec![3.0];
    config.min_errors = 100;
    config.max_frames = 100_000;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    // Uncoded (rate 1): Es/N0 = Eb/N0 = 3 dB, where capacity is ~0.91 bit,
    // below the transmitted rate 1.0
    let capacity = bi_awgn_capacity(3.0);
    assert!(
        capacity > 0.9 && capacity < 0.92,
        "Capacity at Es/N0 = 3 dB should be ~0.91, got {}",
        capacity
    );

    assert!(
        results[0].ber > 0.001,
        "BER should be significant at 3 dB for rate 1.0"
    );
}

#[test]
fn test_csv_export_format() {
    let mut config = SimulationConfig::quick_test();
    config.eb_n0_range_db = vec![3.0];
    config.min_errors = 50;

    let mut rng = rand::thread_rng();
    let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

    let csv = SimulationRunner::results_to_csv(&results, true);

    assert!(csv.contains("eb_n0_db"));
    assert!(csv.contains("ber"));
    assert!(csv.contains("num_bits"));
    assert!(csv.contains("num_bit_errors"));

    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2, "Should have header + 1 data row");
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        #[ignore = "sim: proptest Monte Carlo, 20+ SNR points x 10 000 frames each"]
        fn ber_bounded_by_half(eb_n0_db in -5.0..20.0) {
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![eb_n0_db];
            config.min_errors = 10;
            config.max_frames = 10_000;

            let mut rng = rand::thread_rng();
            let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

            prop_assert!(results[0].ber <= 0.5,
                "BER {} exceeds 0.5 at Eb/N0 = {} dB", results[0].ber, eb_n0_db);
            prop_assert!(results[0].ber >= 0.0,
                "BER {} is negative at Eb/N0 = {} dB", results[0].ber, eb_n0_db);
        }
    }
}
