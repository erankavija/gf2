//! Confidence intervals for binomial counts.

/// The standard-normal critical value for a two-sided 95% interval.
pub const Z_95: f64 = 1.959_964;

/// Computes a Wilson score interval for a binomial count.
///
/// `successes` is the observed event count and `trials` is the total count;
/// the entry point therefore works with a running accumulator or with counts
/// read from a published dataset. `z` is the caller-supplied non-negative
/// standard-normal critical value (for example, [`Z_95`]). The returned tuple
/// is `(lower, upper)`, with both bounds clipped to the probability domain
/// `[0, 1]`.
///
/// The Wilson interval is the set of proportions that are not rejected by the
/// two-sided normal score test. It remains well behaved when `successes` is
/// zero or equal to `trials`, unlike the Wald interval.
///
/// # Panics
///
/// Panics when `successes` exceeds `trials`, or when `z` is negative or not
/// finite. A zero `trials` count has no estimand and returns `(NaN, NaN)`.
///
/// # Examples
///
/// ```
/// use gf2_stats::intervals::{wilson_interval, Z_95};
///
/// let (lower, upper) = wilson_interval(3_333, 10_000, Z_95);
/// assert!(lower < 0.3333 && 0.3333 < upper);
/// ```
#[must_use]
pub fn wilson_interval(successes: u64, trials: u64, z: f64) -> (f64, f64) {
    assert!(successes <= trials, "successes cannot exceed trials");
    assert!(
        z.is_finite() && z >= 0.0,
        "z must be finite and non-negative"
    );

    if trials == 0 {
        return (f64::NAN, f64::NAN);
    }

    let n = trials as f64;
    let p = successes as f64 / n;
    let z_squared = z * z;
    let denominator = 1.0 + z_squared / n;
    let centre = (p + z_squared / (2.0 * n)) / denominator;
    let half_width = z * ((p * (1.0 - p) / n) + z_squared / (4.0 * n * n)).sqrt() / denominator;

    (
        (centre - half_width).max(0.0),
        (centre + half_width).min(1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::{wilson_interval, Z_95};

    /// Solves the Wilson score inequality directly for its two roots. This is
    /// deliberately independent of the centre/half-width implementation.
    fn score_roots(successes: u64, trials: u64, z: f64) -> (f64, f64) {
        let n = trials as f64;
        let p = successes as f64 / n;
        let z_squared = z * z;
        let a = n + z_squared;
        let b = -(2.0 * n * p + z_squared);
        let c = n * p * p;
        let discriminant = b * b - 4.0 * a * c;
        let root = discriminant.sqrt();
        ((-b - root) / (2.0 * a), (-b + root) / (2.0 * a))
    }

    fn assert_matches_score_definition(successes: u64, trials: u64, z: f64) {
        let actual = wilson_interval(successes, trials, z);
        let expected = score_roots(successes, trials, z);
        assert!((actual.0 - expected.0).abs() <= 1e-12);
        assert!((actual.1 - expected.1).abs() <= 1e-12);
    }

    #[test]
    fn agrees_with_independent_score_roots_for_interior_count() {
        assert_matches_score_definition(3_333, 10_000, Z_95);
    }

    #[test]
    fn handles_zero_and_all_successes() {
        assert_matches_score_definition(0, 50, Z_95);
        assert_matches_score_definition(50, 50, Z_95);
    }

    #[test]
    fn accepts_a_caller_supplied_critical_value() {
        let narrow = wilson_interval(3, 10, 1.0);
        let wide = wilson_interval(3, 10, Z_95);
        assert!(narrow.1 - narrow.0 < wide.1 - wide.0);
    }

    #[test]
    fn remains_well_formed_at_campaign_maximum() {
        let (lower, upper) = wilson_interval(2_857_143, 20_000_000, Z_95);
        assert!(lower.is_finite() && upper.is_finite());
        assert!(0.0 <= lower && lower <= upper && upper <= 1.0);
    }
}
