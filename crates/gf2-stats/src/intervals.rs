//! Confidence intervals for binomial counts and for bounded sample means.

use crate::numerics::log_gamma;

// The beta-CDF implementation uses a self-contained continued-fraction
// evaluation. This crate needs only the regularized incomplete beta function
// for count intervals, so a general-purpose statistical dependency would widen
// its API and MSRV surface without adding a caller-visible capability.

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
/// zero or equal to `trials`, unlike the Wald interval. For rare-event cells
/// with observed or expected counts in the tens, prefer
/// [`clopper_pearson_interval`]: its conservative coverage does not rely on
/// the normal approximation.
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

    let lower = (centre - half_width).max(0.0);
    let upper = (centre + half_width).min(1.0);
    (
        if successes == 0 { 0.0 } else { lower },
        if successes == trials { 1.0 } else { upper },
    )
}

/// Computes an equal-tailed Clopper-Pearson interval for a binomial count.
///
/// `successes` is the observed event count, `trials` is the total count, and
/// `level` is the requested two-sided confidence level, strictly between zero
/// and one. The returned tuple is `(lower, upper)`, with both bounds in
/// `[0, 1]`. The endpoints are the numerical inverses of the defining
/// binomial tails (equivalently, beta quantiles), so this is the usual
/// *exact* Clopper-Pearson construction rather than a normal approximation.
///
/// Use this interval when observed or expected event counts are in the tens,
/// especially for rare-event cells: its conservative coverage is preferable
/// to an approximation in that regime. [`wilson_interval`] is typically
/// narrower and is a useful normal-score approximation when counts are large.
/// Both entry points accept counts, so callers can use either an online
/// accumulator or published aggregate data without retaining samples.
///
/// # Panics
///
/// Panics when `successes` exceeds `trials`, or when `level` is not finite or
/// is outside the open interval `(0, 1)`. A zero `trials` count has no
/// estimand and returns `(NaN, NaN)`.
///
/// # Complexity
///
/// Uses two fixed 80-step bisections. Each beta-CDF evaluation performs at
/// most 200 continued-fraction iterations, with $O(1)$ auxiliary space. It
/// therefore does materially more numerical work than [`wilson_interval`].
///
/// # Examples
///
/// ```
/// use gf2_stats::intervals::clopper_pearson_interval;
///
/// let (lower, upper) = clopper_pearson_interval(12, 100, 0.95);
/// assert!(lower < 0.12 && 0.12 < upper);
/// ```
#[must_use]
pub fn clopper_pearson_interval(successes: u64, trials: u64, level: f64) -> (f64, f64) {
    assert!(successes <= trials, "successes cannot exceed trials");
    assert!(
        level.is_finite() && 0.0 < level && level < 1.0,
        "level must be finite and strictly between zero and one"
    );

    if trials == 0 {
        return (f64::NAN, f64::NAN);
    }

    let tail_probability = (1.0 - level) / 2.0;
    let n = trials as f64;
    let lower = if successes == 0 {
        0.0
    } else {
        inverse_regularized_beta(
            tail_probability,
            successes as f64,
            n - successes as f64 + 1.0,
        )
    };
    let upper = if successes == trials {
        1.0
    } else {
        inverse_regularized_beta(
            1.0 - tail_probability,
            successes as f64 + 1.0,
            n - successes as f64,
        )
    };

    (lower, upper)
}

/// Computes an equal-tailed exact interval for an inverse-binomial event rate.
///
/// Inverse (negative-binomial) sampling runs a Bernoulli process until the
/// `events`-th event and reports the `trials` count reached at that moment, so
/// `trials` is a stopping time and the final trial is an event by construction.
/// `level` is the requested two-sided confidence level, strictly between zero
/// and one. The returned tuple is `(lower, upper)`, with both bounds in
/// `[0, 1]`.
///
/// The endpoints invert the exact sampling distribution through the identity
/// `P_p[trials <= n] = P_p[Binomial(n, p) >= events]`, which turns each tail
/// equation into a regularized incomplete beta quantile. Writing `g` for the
/// per-tail probability `(1 - level) / 2`, the lower endpoint solves
/// `P_p[trials <= n] = g` and the upper endpoint solves `P_p[trials >= n] = g`.
/// The lower endpoint therefore coincides with the [`clopper_pearson_interval`]
/// lower endpoint, while the upper endpoint is strictly tighter: the design
/// fixes the event count, so the final trial carries no uncertainty.
///
/// Use this interval when sampling stops on an event count, for example a
/// simulation cell that runs until it observes a preregistered number of
/// failures. Applying [`clopper_pearson_interval`] to such a sample states a
/// coverage its fixed-trial design does not have.
///
/// # Panics
///
/// Panics when `events` exceeds `trials`, or when `level` is not finite or is
/// outside the open interval `(0, 1)`. A zero `trials` count has no estimand
/// and returns `(NaN, NaN)`. A zero `events` count leaves the design without an
/// observed stopping event; the returned interval is then the conservative
/// fixed-trial interval `[0, U]` for zero observed events.
///
/// # Complexity
///
/// Matches [`clopper_pearson_interval`]: two fixed 80-step bisections over beta
/// CDF evaluations, with $O(1)$ auxiliary space.
///
/// # Examples
///
/// ```
/// use gf2_stats::intervals::{clopper_pearson_interval, negative_binomial_interval};
///
/// let (lower, upper) = negative_binomial_interval(12, 100, 0.95);
/// assert!(lower < 0.12 && 0.12 < upper);
///
/// let fixed_trials = clopper_pearson_interval(12, 100, 0.95);
/// assert_eq!(lower, fixed_trials.0);
/// assert!(upper < fixed_trials.1);
/// ```
#[must_use]
pub fn negative_binomial_interval(events: u64, trials: u64, level: f64) -> (f64, f64) {
    assert!(events <= trials, "events cannot exceed trials");
    assert!(
        level.is_finite() && 0.0 < level && level < 1.0,
        "level must be finite and strictly between zero and one"
    );

    if trials == 0 {
        return (f64::NAN, f64::NAN);
    }

    let tail_probability = (1.0 - level) / 2.0;
    let n = trials as f64;
    let k = events as f64;
    let lower = if events == 0 {
        0.0
    } else {
        inverse_regularized_beta(tail_probability, k, n - k + 1.0)
    };
    let upper = if events == trials {
        1.0
    } else {
        inverse_regularized_beta(1.0 - tail_probability, k.max(1.0), n - k)
    };

    (lower, upper)
}

/// Computes a Maurer-Pontil empirical-Bernstein interval for a bounded mean.
///
/// The observations are independent and identically distributed in `[0, 1]`.
/// `mean` is their sample mean, `sample_variance` their unbiased sample
/// variance (the `count - 1` denominator), `count` their number, and `level`
/// the requested two-sided confidence level. The returned tuple is
/// `(lower, upper)`, clipped to `[0, 1]`.
///
/// Maurer and Pontil (*Empirical Bernstein Bounds and Sample Variance
/// Penalization*, 2009) bound the one-sided deviation of the sample mean
/// `M` from the true mean by
/// `sqrt(2 V ln(2 / d) / n) + 7 ln(2 / d) / (3 (n - 1))`
/// with probability at least `1 - d`, for sample variance `V` and `n >= 2`.
/// This entry point spends `d = (1 - level) / 2` on each side, so the two-sided
/// statement holds at `level`. The bound is variance-adaptive: unlike Hoeffding
/// its leading term shrinks with the observed spread rather than with the range
/// of the observation domain, which matters when bounded observations
/// concentrate far from their extremes.
///
/// # Panics
///
/// Panics when `mean` is not finite or lies outside `[0, 1]`, when
/// `sample_variance` is not finite or is negative, or when `level` is not
/// finite or is outside the open interval `(0, 1)`. Fewer than two
/// observations admit no variance estimate; the returned interval is then the
/// trivial `(0.0, 1.0)`.
///
/// # Examples
///
/// ```
/// use gf2_stats::intervals::empirical_bernstein_interval;
///
/// let (lower, upper) = empirical_bernstein_interval(0.2, 0.001, 100, 0.95);
/// assert!(lower < 0.2 && 0.2 < upper);
///
/// // Concentrated observations give a tighter interval than dispersed ones.
/// let (dispersed_lower, _) = empirical_bernstein_interval(0.2, 0.05, 100, 0.95);
/// assert!(dispersed_lower < lower);
/// ```
#[must_use]
pub fn empirical_bernstein_interval(
    mean: f64,
    sample_variance: f64,
    count: u64,
    level: f64,
) -> (f64, f64) {
    assert!(
        mean.is_finite() && (0.0..=1.0).contains(&mean),
        "mean must be finite and inside the observation domain"
    );
    assert!(
        sample_variance.is_finite() && sample_variance >= 0.0,
        "sample variance must be finite and non-negative"
    );
    assert!(
        level.is_finite() && 0.0 < level && level < 1.0,
        "level must be finite and strictly between zero and one"
    );

    if count < 2 {
        return (0.0, 1.0);
    }

    let n = count as f64;
    let log_term = (4.0 / (1.0 - level)).ln();
    let radius = (2.0 * sample_variance * log_term / n).sqrt() + 7.0 * log_term / (3.0 * (n - 1.0));
    ((mean - radius).max(0.0), (mean + radius).min(1.0))
}

/// Inverts a regularized incomplete beta CDF by bisection.  The fixed 80
/// iterations reduce the initial unit interval below `2^-80`, well below f64
/// precision, while retaining a bracketed result even at very large counts.
fn inverse_regularized_beta(probability: f64, a: f64, b: f64) -> f64 {
    let mut lower = 0.0;
    let mut upper = 1.0;
    for _ in 0..80 {
        let midpoint = (lower + upper) / 2.0;
        if regularized_beta(midpoint, a, b) < probability {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
    }
    (lower + upper) / 2.0
}

/// Evaluates the regularized incomplete beta CDF for positive `a` and `b`.
fn regularized_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    let front =
        (log_gamma(a + b) - log_gamma(a) - log_gamma(b) + a * x.ln() + b * (-x).ln_1p()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        (front * beta_continued_fraction(a, b, x) / a).clamp(0.0, 1.0)
    } else {
        (1.0 - front * beta_continued_fraction(b, a, 1.0 - x) / b).clamp(0.0, 1.0)
    }
}

/// Evaluates the continued fraction used by [`regularized_beta`].
fn beta_continued_fraction(a: f64, b: f64, x: f64) -> f64 {
    const MAX_ITERATIONS: u32 = 200;
    const EPSILON: f64 = 3.0e-14;
    const MINIMUM: f64 = 1.0e-300;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < MINIMUM {
        d = MINIMUM;
    }
    d = 1.0 / d;
    let mut fraction = d;

    for iteration in 1..=MAX_ITERATIONS {
        let m = iteration as f64;
        let m2 = 2.0 * m;
        let mut coefficient = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + coefficient * d;
        if d.abs() < MINIMUM {
            d = MINIMUM;
        }
        c = 1.0 + coefficient / c;
        if c.abs() < MINIMUM {
            c = MINIMUM;
        }
        d = 1.0 / d;
        fraction *= d * c;

        coefficient = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + coefficient * d;
        if d.abs() < MINIMUM {
            d = MINIMUM;
        }
        c = 1.0 + coefficient / c;
        if c.abs() < MINIMUM {
            c = MINIMUM;
        }
        d = 1.0 / d;
        let delta = d * c;
        fraction *= delta;
        if (delta - 1.0).abs() <= EPSILON {
            break;
        }
    }

    fraction
}

#[cfg(test)]
mod tests {
    use super::{
        clopper_pearson_interval, empirical_bernstein_interval, negative_binomial_interval,
        regularized_beta, wilson_interval, Z_95,
    };

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

    /// The endpoints are defined by the two tail equations of the stopping
    /// distribution, rewritten through the binomial-tail identity. Evaluating
    /// those tails at the returned endpoints is independent of the bisection
    /// that produced them.
    #[test]
    fn negative_binomial_endpoints_solve_their_defining_tail_equations() {
        let (events, trials, level) = (12_u64, 100_u64, 0.95);
        let (lower, upper) = negative_binomial_interval(events, trials, level);
        let tail = (1.0 - level) / 2.0;
        let (k, n) = (events as f64, trials as f64);

        // P_lower[trials <= n] = P_lower[Binomial(n, p) >= events] = tail.
        assert!((regularized_beta(lower, k, n - k + 1.0) - tail).abs() <= 1e-9);
        // P_upper[trials >= n] = 1 - P_upper[Binomial(n - 1, p) >= events] = tail.
        assert!((1.0 - regularized_beta(upper, k, n - k) - tail).abs() <= 1e-9);
    }

    #[test]
    fn negative_binomial_shares_the_fixed_trial_lower_endpoint_and_tightens_the_upper() {
        for (events, trials) in [(3_u64, 40_u64), (100, 140_000), (7, 9)] {
            let inverse = negative_binomial_interval(events, trials, 0.95);
            let fixed = clopper_pearson_interval(events, trials, 0.95);
            assert_eq!(inverse.0, fixed.0);
            assert!(inverse.1 < fixed.1);
            assert!(inverse.0 <= events as f64 / trials as f64);
            assert!(inverse.1 >= events as f64 / trials as f64);
        }
    }

    #[test]
    fn negative_binomial_handles_degenerate_event_counts() {
        let (lower, upper) = negative_binomial_interval(0, 500, 0.95);
        assert_eq!(lower, 0.0);
        assert_eq!(upper, clopper_pearson_interval(0, 500, 0.95).1);

        assert_eq!(negative_binomial_interval(9, 9, 0.95).1, 1.0);
        assert!(negative_binomial_interval(0, 0, 0.95).0.is_nan());
    }

    #[test]
    fn empirical_bernstein_narrows_with_variance_and_with_observation_count() {
        let concentrated = empirical_bernstein_interval(0.2, 0.000_9, 100, 0.95);
        let dispersed = empirical_bernstein_interval(0.2, 0.02, 100, 0.95);
        let plentiful = empirical_bernstein_interval(0.2, 0.000_9, 10_000, 0.95);

        assert!(concentrated.1 - concentrated.0 < dispersed.1 - dispersed.0);
        assert!(plentiful.1 - plentiful.0 < concentrated.1 - concentrated.0);
        assert!(concentrated.0 < 0.2 && 0.2 < concentrated.1);
    }

    #[test]
    fn empirical_bernstein_is_symmetric_about_the_sample_mean_before_clipping() {
        let (lower, upper) = empirical_bernstein_interval(0.5, 0.01, 400, 0.95);
        assert!(((0.5 - lower) - (upper - 0.5)).abs() <= 1e-12);
        assert!(lower > 0.0 && upper < 1.0);
    }

    #[test]
    fn empirical_bernstein_clips_to_the_observation_domain() {
        assert_eq!(empirical_bernstein_interval(0.01, 0.2, 5, 0.95).0, 0.0);
        assert_eq!(empirical_bernstein_interval(0.99, 0.2, 5, 0.95).1, 1.0);
        assert_eq!(empirical_bernstein_interval(0.3, 0.0, 1, 0.95), (0.0, 1.0));
    }
}
