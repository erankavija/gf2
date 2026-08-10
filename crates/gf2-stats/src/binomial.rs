//! Exact binomial hypothesis tests for count data.
//!
//! These tests evaluate binomial tails rather than normal approximations. The
//! normal critical values used when planning a campaign's sample size are not
//! acceptance decisions: use the exact tests in this module to make campaign
//! decisions. Results retain the natural logarithm of the p-value, so a valid
//! decision remains available when converting the p-value itself to `f64`
//! would underflow.

use crate::numerics::log_gamma;

const MAX_SUPPORTED_TRIALS: u64 = (1 << 53) - 1;

/// The result of an exact binomial hypothesis test.
///
/// The result intentionally exposes the natural logarithm of the p-value,
/// rather than a direct probability. A p-value below the smallest positive
/// `f64` is still distinguished from zero by its finite logarithm. A p-value
/// that is mathematically zero, which can occur after observing an impossible
/// outcome under a degenerate null probability, is represented by negative
/// infinity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExactBinomialTest {
    log_p_value: f64,
}

impl ExactBinomialTest {
    /// Returns the natural logarithm of the exact p-value.
    ///
    /// The value is zero when the p-value is one and negative infinity when
    /// the p-value is zero. Otherwise it remains finite even where
    /// `log_p_value.exp()` would underflow.
    #[must_use]
    pub const fn log_p_value(self) -> f64 {
        self.log_p_value
    }

    /// Returns whether this test rejects at `level`.
    ///
    /// The comparison is closed: a p-value equal to `level` rejects. This is
    /// the conventional p-value decision rule and matters for discrete exact
    /// tests, whose attainable p-values need not equal a requested level.
    ///
    /// # Panics
    ///
    /// Panics when `level` is not finite or is outside `(0, 1]`.
    #[must_use]
    pub fn rejects_at(self, level: f64) -> bool {
        assert!(
            level.is_finite() && 0.0 < level && level <= 1.0,
            "level must be finite and in (0, 1]"
        );
        self.log_p_value <= level.ln()
    }
}

/// Tests the composite null $H_0: p \ge p_0$ against $H_1: p < p_0$.
///
/// `successes` is the observed count and `trials` is the fixed number of
/// independent Bernoulli trials. The p-value is
/// $\Pr_{p_0}[X \le \text{successes}]$. This lower tail is deliberately the
/// floor-test direction: as $p$ increases over the composite null, the chance
/// of an observation this small cannot increase, so evaluating the boundary
/// $p = p_0$ is the valid worst case for the whole null.
///
/// `null_probability` may be zero or one, which represent degenerate binomial
/// nulls. With zero trials the sole observable count is zero and the returned
/// p-value is one.
///
/// # Panics
///
/// Panics when `successes` exceeds `trials`, when `trials` is too large for
/// the exact-integer `f64` arithmetic used by the numerical kernel, or when
/// `null_probability` is not finite or outside `[0, 1]`.
///
/// # Complexity
///
/// Computes one binomial tail by recurrence in $O(N)$ worst-case time and
/// $O(1)$ auxiliary space, where $N$ is `trials`. The final threshold decision
/// through [`ExactBinomialTest::rejects_at`] is $O(1)$.
#[must_use]
pub fn lower_tail_test(successes: u64, trials: u64, null_probability: f64) -> ExactBinomialTest {
    validate_binomial_input(successes, trials, null_probability);
    ExactBinomialTest {
        log_p_value: lower_tail_log_probability(successes, trials, null_probability),
    }
}

/// Tests the permanent zero-probability floor for a field of order `field_order`.
///
/// This is [`lower_tail_test`] specialized to $H_0: p \ge 1/q$, where
/// `field_order` is $q$. It therefore evaluates the composite null at exactly
/// the least favorable permitted probability $p_0 = 1/q$, and rejects only for
/// a count that is too small. It does not test a deviation above the floor.
///
/// # Panics
///
/// Panics when `field_order` is smaller than two, or for any invalid count
/// input documented by [`lower_tail_test`].
///
/// # Complexity
///
/// Has the same $O(N)$ worst-case time and $O(1)$ auxiliary-space complexity
/// as [`lower_tail_test`].
#[must_use]
pub fn permanent_zero_floor_test(
    successes: u64,
    trials: u64,
    field_order: u64,
) -> ExactBinomialTest {
    assert!(field_order >= 2, "field order must be at least two");
    lower_tail_test(successes, trials, 1.0 / field_order as f64)
}

/// Performs a two-sided exact binomial test against `null_probability`.
///
/// This uses the conventional probability-ordering definition. For an observed
/// count $x$, the p-value is
///
/// $$
/// \sum_{k:\;\Pr[X=k] \le \Pr[X=x]} \Pr[X=k].
/// $$
///
/// It is not a doubled one-sided tail. Outcomes whose mass equals the observed
/// mass are included. The numerical comparison is a literal `<=` on the
/// logarithmic masses computed from the caller-supplied `f64`; no arbitrary
/// tolerance changes the rejection region. For `null_probability == 0.5`, the
/// implementation canonicalizes the symmetric combinatorial term so mirrored
/// outcomes have bit-identical logarithmic masses and their required ties are
/// included.
///
/// `null_probability` may be zero or one. A possible outcome under a
/// degenerate null has p-value one; an impossible outcome has p-value zero.
/// With zero trials, the only possible observed count is zero and has p-value
/// one.
///
/// # Panics
///
/// Panics when `successes` exceeds `trials`, when `trials` is too large for
/// the exact-integer `f64` arithmetic used by the numerical kernel, or when
/// `null_probability` is not finite or outside `[0, 1]`.
///
/// # Complexity
///
/// Finds the opposite-tail cutoff in $O(\log N)$ comparisons, then sums the
/// two tails in $O(N)$ worst-case time with $O(1)$ auxiliary space.
#[must_use]
pub fn two_sided_test(successes: u64, trials: u64, null_probability: f64) -> ExactBinomialTest {
    validate_binomial_input(successes, trials, null_probability);

    if trials == 0 {
        return ExactBinomialTest { log_p_value: 0.0 };
    }
    if null_probability == 0.0 {
        return ExactBinomialTest {
            log_p_value: if successes == 0 {
                0.0
            } else {
                f64::NEG_INFINITY
            },
        };
    }
    if null_probability == 1.0 {
        return ExactBinomialTest {
            log_p_value: if successes == trials {
                0.0
            } else {
                f64::NEG_INFINITY
            },
        };
    }

    let (lower_mode, upper_mode) = mode_bounds(trials, null_probability);
    let observed_log_mass = log_binomial_probability(successes, trials, null_probability);
    let lower_cutoff =
        largest_left_mass_at_most(lower_mode, trials, null_probability, observed_log_mass);
    let upper_cutoff =
        smallest_right_mass_at_most(upper_mode, trials, null_probability, observed_log_mass);

    match (lower_cutoff, upper_cutoff) {
        (Some(lower_cutoff), Some(upper_cutoff)) if lower_cutoff >= upper_cutoff => {
            ExactBinomialTest { log_p_value: 0.0 }
        }
        (Some(lower_cutoff), Some(upper_cutoff)) => ExactBinomialTest {
            log_p_value: log_add_exp(
                log_lower_tail(lower_cutoff, trials, null_probability),
                log_upper_tail(upper_cutoff, trials, null_probability),
            )
            .min(0.0),
        },
        (Some(lower_cutoff), None) => ExactBinomialTest {
            log_p_value: log_lower_tail(lower_cutoff, trials, null_probability),
        },
        (None, Some(upper_cutoff)) => ExactBinomialTest {
            log_p_value: log_upper_tail(upper_cutoff, trials, null_probability),
        },
        (None, None) => unreachable!("the observed outcome always belongs to one probability tail"),
    }
}

/// Returns a Bonferroni per-test level for a fixed family of tests.
///
/// Divide the caller's complete family-wise error budget by every test across
/// every family it intends to control. For example, allocating one global
/// budget across 63 permanent cells and 63 determinant cells uses
/// `bonferroni_level(budget, 126)`, not one independent full budget per family.
///
/// # Panics
///
/// Panics when `familywise_error` is not finite or outside `(0, 1]`, or when
/// `test_count` is zero.
///
/// # Complexity
///
/// Runs in $O(1)$ time and uses $O(1)$ space.
#[must_use]
pub fn bonferroni_level(familywise_error: f64, test_count: u64) -> f64 {
    assert!(
        familywise_error.is_finite() && 0.0 < familywise_error && familywise_error <= 1.0,
        "familywise error must be finite and in (0, 1]"
    );
    assert!(test_count > 0, "test count must be non-zero");
    familywise_error / test_count as f64
}

fn validate_binomial_input(successes: u64, trials: u64, null_probability: f64) {
    assert!(successes <= trials, "successes cannot exceed trials");
    assert!(
        trials <= MAX_SUPPORTED_TRIALS,
        "trials must be smaller than 2^53"
    );
    assert!(
        null_probability.is_finite() && (0.0..=1.0).contains(&null_probability),
        "null probability must be finite and in [0, 1]"
    );
}

fn lower_tail_log_probability(successes: u64, trials: u64, probability: f64) -> f64 {
    if trials == 0 || probability == 0.0 || successes == trials {
        return 0.0;
    }
    if probability == 1.0 {
        return f64::NEG_INFINITY;
    }

    let (lower_mode, _) = mode_bounds(trials, probability);
    if successes < lower_mode {
        return log_lower_tail(successes, trials, probability);
    }
    if successes > lower_mode {
        return log_one_minus_exp(log_upper_tail(successes + 1, trials, probability));
    }

    if successes <= trials - successes {
        log_lower_tail(successes, trials, probability)
    } else {
        log_one_minus_exp(log_upper_tail(successes + 1, trials, probability))
    }
}

fn mode_bounds(trials: u64, probability: f64) -> (u64, u64) {
    let scaled = (trials as f64 + 1.0) * probability;
    let upper_mode = (scaled.floor() as u64).min(trials);
    let lower_mode = if upper_mode > 0 && scaled == upper_mode as f64 {
        upper_mode - 1
    } else {
        upper_mode
    };
    (lower_mode, upper_mode)
}

fn largest_left_mass_at_most(
    lower_mode: u64,
    trials: u64,
    probability: f64,
    observed_log_mass: f64,
) -> Option<u64> {
    if log_binomial_probability(0, trials, probability) > observed_log_mass {
        return None;
    }
    let mut lower = 0;
    let mut upper = lower_mode;
    while lower < upper {
        let midpoint = lower + (upper - lower).div_ceil(2);
        if log_binomial_probability(midpoint, trials, probability) <= observed_log_mass {
            lower = midpoint;
        } else {
            upper = midpoint - 1;
        }
    }
    Some(lower)
}

fn smallest_right_mass_at_most(
    upper_mode: u64,
    trials: u64,
    probability: f64,
    observed_log_mass: f64,
) -> Option<u64> {
    if log_binomial_probability(trials, trials, probability) > observed_log_mass {
        return None;
    }
    let mut lower = upper_mode;
    let mut upper = trials;
    while lower < upper {
        let midpoint = lower + (upper - lower) / 2;
        if log_binomial_probability(midpoint, trials, probability) <= observed_log_mass {
            upper = midpoint;
        } else {
            lower = midpoint + 1;
        }
    }
    Some(lower)
}

fn log_lower_tail(successes: u64, trials: u64, probability: f64) -> f64 {
    let mut relative_term = 1.0;
    let mut relative_sum = 1.0;
    let ratio_scale = (1.0 - probability) / probability;
    let mut current = successes;
    while current > 0 {
        relative_term *= current as f64 / (trials - current + 1) as f64 * ratio_scale;
        relative_sum += relative_term;
        current -= 1;
    }
    (log_binomial_probability(successes, trials, probability) + relative_sum.ln()).min(0.0)
}

fn log_upper_tail(successes: u64, trials: u64, probability: f64) -> f64 {
    let mut relative_term = 1.0;
    let mut relative_sum = 1.0;
    let ratio_scale = probability / (1.0 - probability);
    let mut current = successes;
    while current < trials {
        relative_term *= (trials - current) as f64 / (current + 1) as f64 * ratio_scale;
        relative_sum += relative_term;
        current += 1;
    }
    (log_binomial_probability(successes, trials, probability) + relative_sum.ln()).min(0.0)
}

fn log_binomial_probability(successes: u64, trials: u64, probability: f64) -> f64 {
    let trials_as_f64 = trials as f64;
    if probability == 0.5 {
        let lesser = successes.min(trials - successes) as f64;
        let greater = trials as f64 - lesser;
        return log_gamma(trials_as_f64 + 1.0) - log_gamma(lesser + 1.0) - log_gamma(greater + 1.0)
            + trials_as_f64 * probability.ln();
    }

    log_gamma(trials_as_f64 + 1.0)
        - log_gamma(successes as f64 + 1.0)
        - log_gamma((trials - successes) as f64 + 1.0)
        + successes as f64 * probability.ln()
        + (trials - successes) as f64 * (-probability).ln_1p()
}

fn log_add_exp(left: f64, right: f64) -> f64 {
    if left == f64::NEG_INFINITY {
        return right;
    }
    if right == f64::NEG_INFINITY {
        return left;
    }
    if left >= right {
        left + (right - left).exp().ln_1p()
    } else {
        right + (left - right).exp().ln_1p()
    }
}

fn log_one_minus_exp(log_probability: f64) -> f64 {
    debug_assert!(log_probability <= 0.0);
    if log_probability == f64::NEG_INFINITY {
        return 0.0;
    }
    if log_probability <= -core::f64::consts::LN_2 {
        (-log_probability.exp()).ln_1p()
    } else {
        (-log_probability.exp_m1()).ln()
    }
}
