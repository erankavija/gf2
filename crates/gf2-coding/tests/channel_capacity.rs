//! Integration tests for Shannon channel capacity calculations.
//!
//! Validates that the AWGN channel capacity computations match theoretical
//! predictions and satisfy fundamental information-theoretic constraints.

use gf2_coding::channel::AwgnChannel;

/// Known Shannon capacity values for BPSK over AWGN at specific Eb/N0 points.
///
/// Reference values computed using numerical integration of the BPSK capacity formula.
/// Format: (Eb/N0 in dB, expected capacity in bits/symbol)
const REFERENCE_CAPACITY_VALUES: &[(f64, f64)] = &[
    // Very low SNR
    (-2.0, 0.348879),
    (0.0, 0.485944),
    // Moderate SNR
    (3.0, 0.720661),
    (6.0, 0.911880),
    (9.0, 0.990164),
    // High SNR
    (12.0, 0.999854),
    (15.0, 1.000000),
];

#[test]
fn test_capacity_at_reference_points() {
    for &(eb_n0_db, expected) in REFERENCE_CAPACITY_VALUES {
        let capacity = AwgnChannel::shannon_capacity(eb_n0_db);
        let error = (capacity - expected).abs();

        // Allow 0.1% relative error for numerical stability
        let tolerance = expected * 0.001;
        assert!(
            error < tolerance,
            "Capacity mismatch at Eb/N0={} dB: expected {}, got {} (error: {})",
            eb_n0_db,
            expected,
            capacity,
            error
        );
    }
}

#[test]
fn test_capacity_monotonic_with_snr() {
    let eb_n0_values = vec![-2.0, 0.0, 2.0, 4.0, 6.0, 8.0, 10.0];

    let mut prev_capacity = 0.0;
    for &eb_n0_db in &eb_n0_values {
        let capacity = AwgnChannel::shannon_capacity(eb_n0_db);
        assert!(
            capacity > prev_capacity,
            "Capacity should increase with Eb/N0: at {} dB got {}, previous was {}",
            eb_n0_db,
            capacity,
            prev_capacity
        );
        prev_capacity = capacity;
    }
}

#[test]
fn test_capacity_bounds() {
    // Capacity must be in [0, 1] for all valid SNR values
    for eb_n0_db in [-5.0, -2.0, 0.0, 3.0, 6.0, 9.0, 15.0, 20.0] {
        let capacity = AwgnChannel::shannon_capacity(eb_n0_db);
        assert!(
            (0.0..=1.0).contains(&capacity),
            "Capacity out of bounds at Eb/N0={} dB: {}",
            eb_n0_db,
            capacity
        );
    }
}

#[test]
fn test_capacity_approaches_zero_at_low_snr() {
    let capacity = AwgnChannel::shannon_capacity(-10.0);
    assert!(
        capacity < 0.2,
        "Capacity should be small at very low SNR, got {}",
        capacity
    );
}

#[test]
fn test_capacity_approaches_one_at_high_snr() {
    let capacity = AwgnChannel::shannon_capacity(20.0);
    assert!(capacity > 0.99, "Capacity should approach 1.0 at high SNR");
}

#[test]
fn test_shannon_limit_for_rate_half() {
    let rate = 0.5;
    let eb_n0_min = AwgnChannel::shannon_limit(rate);

    // Theoretical Shannon limit for rate 1/2 is approximately 0.19 dB
    assert!(
        eb_n0_min > -0.5 && eb_n0_min < 0.5,
        "Shannon limit for rate 1/2 should be near 0.19 dB, got {}",
        eb_n0_min
    );
}

#[test]
fn test_shannon_limit_for_various_rates() {
    // Test that Shannon limit increases with rate
    let rates = vec![0.25, 0.5, 0.75, 0.9];
    let mut prev_limit = f64::NEG_INFINITY;

    for &rate in &rates {
        let limit = AwgnChannel::shannon_limit(rate);
        assert!(
            limit > prev_limit,
            "Shannon limit should increase with rate: at rate {} got {} dB, previous was {}",
            rate,
            limit,
            prev_limit
        );
        prev_limit = limit;
    }
}

#[test]
fn test_shannon_limit_consistency() {
    // Verify that capacity at Shannon limit equals the target rate
    for &rate in &[0.25, 0.5, 0.75, 0.9] {
        let eb_n0_limit = AwgnChannel::shannon_limit(rate);
        let capacity = AwgnChannel::shannon_capacity(eb_n0_limit);

        let error = (capacity - rate).abs();
        assert!(
            error < 0.002,
            "Capacity at Shannon limit should equal rate: rate={}, limit={} dB, capacity={}, error={}",
            rate, eb_n0_limit, capacity, error
        );
    }
}

#[test]
fn test_shannon_limit_for_rate_one() {
    let rate = 1.0;
    let eb_n0_min = AwgnChannel::shannon_limit(rate);

    // For rate 1.0, Shannon limit approaches infinity (need infinite SNR)
    // But for BPSK, practical limit is very high
    assert!(
        eb_n0_min > 5.0,
        "Shannon limit for rate 1.0 should be high, got {} dB",
        eb_n0_min
    );
}

#[test]
fn test_capacity_with_different_rates() {
    // This test no longer makes sense since capacity doesn't take rate
    // Shannon capacity is a property of the channel alone
    // We test that capacity increases with SNR instead
    let capacity_low = AwgnChannel::shannon_capacity(0.0);
    let capacity_high = AwgnChannel::shannon_capacity(6.0);

    assert!(capacity_high > capacity_low);
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn capacity_always_in_unit_interval(
            eb_n0_db in -5.0..20.0
        ) {
            let capacity = AwgnChannel::shannon_capacity(eb_n0_db);
            prop_assert!((0.0..=1.0).contains(&capacity));
        }

        #[test]
        fn capacity_increases_with_snr(
            eb_n0_low in -5.0..10.0,
            delta in 0.1..5.0
        ) {
            let eb_n0_high = eb_n0_low + delta;
            let cap_low = AwgnChannel::shannon_capacity(eb_n0_low);
            let cap_high = AwgnChannel::shannon_capacity(eb_n0_high);
            prop_assert!(cap_high > cap_low);
        }

        #[test]
        fn shannon_limit_achieves_rate(
            rate in 0.2..0.95
        ) {
            let limit = AwgnChannel::shannon_limit(rate);
            let capacity = AwgnChannel::shannon_capacity(limit);
            let error = (capacity - rate).abs();
            prop_assert!(error < 0.002, "Error {} exceeds tolerance for rate {}", error, rate);
        }

        #[test]
        fn shannon_limit_increases_with_rate(
            rate_low in 0.2f64..0.75f64,
            delta in 0.05f64..0.15f64
        ) {
            let rate_high = (rate_low + delta).min(0.95);
            // Only test if rates are sufficiently different
            if rate_high - rate_low < 0.03 {
                return Ok(());
            }
            let limit_low = AwgnChannel::shannon_limit(rate_low);
            let limit_high = AwgnChannel::shannon_limit(rate_high);
            prop_assert!(limit_high > limit_low - 0.01,
                "Shannon limit should increase: rate {} -> {} gave {} -> {} dB",
                rate_low, rate_high, limit_low, limit_high);
        }
    }
}
