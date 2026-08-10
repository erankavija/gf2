use gf2_stats::intervals::{wilson_interval, Z_95};

#[test]
fn count_api_matches_a_hand_computed_zero_count() {
    let (lower, upper) = wilson_interval(0, 50, Z_95);
    assert_eq!(lower, 0.0);
    assert!((upper - 0.07134760017861414).abs() <= 1e-12);
}

#[test]
fn count_api_matches_a_hand_computed_all_success_count() {
    let (lower, upper) = wilson_interval(50, 50, Z_95);
    assert!((lower - 0.9286523998213857).abs() <= 1e-12);
    assert_eq!(upper, 1.0);
}

#[test]
fn count_api_does_not_require_sample_storage() {
    let successes = 2_857_143;
    let trials = 20_000_000;
    let (lower, upper) = wilson_interval(successes, trials, Z_95);
    assert!(lower <= successes as f64 / trials as f64);
    assert!(successes as f64 / trials as f64 <= upper);
}
