use gf2_stats::binomial::{
    bonferroni_level, lower_tail_test, permanent_zero_floor_test, two_sided_test,
};

const TOLERANCE: f64 = 2e-11;

/// Independently evaluates a binomial point mass from the defining recurrence.
///
/// The small cases used here keep every intermediate probability representable,
/// so this deliberately does not share the production log-domain code.
fn binomial_probability(successes: u64, trials: u64, probability: f64) -> f64 {
    let mut term = (1.0 - probability).powi(trials as i32);
    for k in 0..successes {
        term *= (trials - k) as f64 / (k + 1) as f64;
        term *= probability / (1.0 - probability);
    }
    term
}

fn binomial_lower_tail(successes: u64, trials: u64, probability: f64) -> f64 {
    (0..=successes)
        .map(|k| binomial_probability(k, trials, probability))
        .sum()
}

/// The probability-ordering definition of a two-sided exact binomial test.
fn probability_ordering_p_value(successes: u64, trials: u64, probability: f64) -> f64 {
    let observed = binomial_probability(successes, trials, probability);
    (0..=trials)
        .map(|k| binomial_probability(k, trials, probability))
        .filter(|mass| *mass <= observed)
        .sum()
}

fn assert_log_p_value_matches(actual_log_p_value: f64, expected_p_value: f64) {
    assert!(
        (actual_log_p_value.exp() - expected_p_value).abs() <= TOLERANCE,
        "log p-value {actual_log_p_value} differs from reference probability {expected_p_value}"
    );
}

#[test]
fn floor_test_matches_an_independent_lower_tail_sum() {
    for (successes, trials, field_order) in [
        (0, 20, 3),
        (4, 20, 3),
        (9, 37, 5),
        (21, 80, 7),
        (51, 200, 3),
    ] {
        let probability = 1.0 / field_order as f64;
        let expected = binomial_lower_tail(successes, trials, probability);
        let actual = permanent_zero_floor_test(successes, trials, field_order);
        assert_log_p_value_matches(actual.log_p_value(), expected);
    }
}

#[test]
fn lower_tail_test_uses_the_supplied_composite_null_boundary() {
    let null_probability = 0.3;
    let actual = lower_tail_test(7, 31, null_probability);
    let expected = binomial_lower_tail(7, 31, null_probability);
    assert_log_p_value_matches(actual.log_p_value(), expected);
}

#[test]
fn two_sided_test_matches_an_independent_probability_ordering_sum() {
    for (successes, trials, probability) in [
        (1, 3, 0.01),
        (3, 17, 0.3),
        (11, 31, 0.27),
        (22, 75, 0.4),
        (54, 200, 0.31),
        (2, 3, 0.99),
    ] {
        let expected = probability_ordering_p_value(successes, trials, probability);
        let actual = two_sided_test(successes, trials, probability);
        assert_log_p_value_matches(actual.log_p_value(), expected);
    }
}

#[test]
fn two_sided_test_includes_equal_probability_symmetric_outcomes() {
    let actual = two_sided_test(3, 10, 0.5);
    let expected = 2.0 * binomial_lower_tail(3, 10, 0.5);
    assert_log_p_value_matches(actual.log_p_value(), expected);
}

#[test]
fn exact_test_boundaries_are_explicit() {
    assert_eq!(lower_tail_test(0, 0, 0.3).log_p_value(), 0.0);
    assert_eq!(lower_tail_test(0, 12, 0.0).log_p_value(), 0.0);
    assert_eq!(lower_tail_test(12, 12, 1.0).log_p_value(), 0.0);
    assert!(lower_tail_test(0, 12, 1.0).log_p_value().is_sign_negative());
    assert_eq!(two_sided_test(0, 0, 0.7).log_p_value(), 0.0);
    assert_eq!(two_sided_test(0, 12, 0.0).log_p_value(), 0.0);
    assert_eq!(two_sided_test(12, 12, 1.0).log_p_value(), 0.0);

    let impossible_under_zero = two_sided_test(1, 12, 0.0);
    assert!(impossible_under_zero.log_p_value().is_infinite());
    assert!(impossible_under_zero.log_p_value().is_sign_negative());

    let impossible_under_one = two_sided_test(11, 12, 1.0);
    assert!(impossible_under_one.log_p_value().is_infinite());
    assert!(impossible_under_one.log_p_value().is_sign_negative());
}

#[test]
fn threshold_comparison_includes_the_level() {
    let result = lower_tail_test(0, 1, 0.5);
    assert!(result.rejects_at(0.5));
    assert!(!result.rejects_at(0.499_999_999_999));
}

#[test]
fn bonferroni_level_splits_a_global_error_budget() {
    let per_test = bonferroni_level(0.05, 63);
    assert!((per_test - 0.05 / 63.0).abs() <= f64::EPSILON);

    let two_family_allocation = bonferroni_level(0.05, 63 * 2);
    assert!((two_family_allocation - 0.05 / 126.0).abs() <= f64::EPSILON);
}

#[test]
fn campaign_scale_floor_decisions_keep_a_finite_log_p_value() {
    const TRIALS: u64 = 20_000_000;
    let level = bonferroni_level(0.05, 63);

    let below_threshold = permanent_zero_floor_test(6_000_000, TRIALS, 3);
    assert!(below_threshold.log_p_value().is_finite());
    assert!(below_threshold.log_p_value() < f64::MIN_POSITIVE.ln());
    assert_eq!(below_threshold.log_p_value().exp(), 0.0);
    assert!(below_threshold.rejects_at(level));

    let above_threshold = permanent_zero_floor_test(6_662_000, TRIALS, 3);
    assert!(above_threshold.log_p_value().is_finite());
    assert!(!above_threshold.rejects_at(level));
}
