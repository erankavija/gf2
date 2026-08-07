//! Sample-size planning and interval estimation for the zero-fraction campaign.

/// Samples needed for a target standard error on a proportion.
///
/// `N = ceil(p (1 - p) / SE^2)`, the normal-approximation sample size for a
/// binomial proportion. `p` is the planning estimate: the campaign uses
/// `p = 1/q` (the conjectured value under [GGK2025]) and reports the
/// conservative `p = 1/2` alongside it, since `p(1-p)` is maximised there and
/// no proportion needs more samples than that column states.
#[must_use]
pub fn required_n(p: f64, se: f64) -> u64 {
    (p * (1.0 - p) / (se * se)).ceil() as u64
}

/// Standard error of a proportion estimate at `p` after `n` samples.
#[must_use]
pub fn se_at(p: f64, n: f64) -> f64 {
    (p * (1.0 - p) / n).sqrt()
}

/// Wilson score interval for a binomial proportion.
///
/// Preferred over the Wald interval because the campaign's `p ≈ 1/q` with very
/// large `N` still produces counts near an interval boundary in the small-`N`
/// pilot cells, where Wald undercovers. `z` is the standard normal quantile
/// (1.959964 for 95 %).
#[must_use]
pub fn wilson_interval(successes: u64, trials: u64, z: f64) -> (f64, f64) {
    if trials == 0 {
        return (f64::NAN, f64::NAN);
    }
    let n = trials as f64;
    let p = successes as f64 / n;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denom;
    let half = z * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).sqrt() / denom;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// The 95 % standard normal quantile.
pub const Z_95: f64 = 1.959_964;

/// Fraction of the wall-clock budget reserved for work that is not permanent
/// evaluation: checkpoint writes, dataset compaction, restart after a failed
/// shard, and the residual throttling a sustained run does not already show.
///
/// Applied as a haircut on the budget rather than on the measured rate, so the
/// measured rates in the CSV stay untouched.
pub const OPERATIONAL_RESERVE: f64 = 0.15;

/// Productive compute seconds available inside a `budget_hours` window.
#[must_use]
pub fn effective_budget_s(budget_hours: f64) -> f64 {
    budget_hours * 3600.0 * (1.0 - OPERATIONAL_RESERVE)
}

/// One `(q, n, SE target)` row of the attainable envelope.
#[derive(Clone, Debug)]
pub struct EnvelopeRow {
    pub q: u64,
    pub n: usize,
    pub target_se: f64,
    /// Backend with the highest measured composite rate for this `(q, n)`.
    pub best_backend: String,
    pub best_rate: f64,
    pub required_n_planning: u64,
    pub required_n_conservative: u64,
    pub hours_planning: f64,
    pub hours_conservative: f64,
    pub feasible: bool,
    /// Samples the best path actually delivers inside the effective budget.
    pub attainable_n: u64,
    /// Standard error those samples buy at `p = 1/q`.
    pub attainable_se: f64,
    /// Trials behind the published estimate at this `(q, n)`, if one exists.
    /// Only $q = 3$ has published numerics; see [`crate::prior`].
    pub prior_trials: Option<u64>,
    /// Standard error the published estimate achieved at this `(q, n)`.
    pub prior_se: Option<f64>,
    /// Half-width of a 95 % Wilson interval implied by the published counts.
    pub prior_ci_half_width: Option<f64>,
    /// `prior_se / attainable_se`: above 1 means this budget resolves the cell
    /// more finely than the published estimate does.
    pub precision_ratio: Option<f64>,
    /// Classification of this cell against the published precision.
    pub prior_comparison: crate::prior::PriorComparison,
}

/// CSV header for [`EnvelopeRow::to_csv_row`].
pub const ENVELOPE_CSV_HEADER: &str = "q,n,target_se,best_backend,best_composite_matrices_per_s,\
required_n_p_1_over_q,required_n_p_half,hours_p_1_over_q,hours_p_half,feasible_12h,\
attainable_n_12h,attainable_se_12h,scheinerman2024_trials,scheinerman2024_se,\
scheinerman2024_wilson_half_width,precision_ratio_vs_scheinerman2024,precision_comparison";

impl EnvelopeRow {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        let opt = |v: Option<f64>| v.map_or_else(|| "none".to_string(), |x| format!("{x:.3e}"));
        let prior = self
            .prior_trials
            .map_or_else(|| "none".to_string(), |t| t.to_string());
        format!(
            "{},{},{:.0e},{},{:.4},{},{},{:.4},{:.4},{},{},{:.3e},{},{},{},{},{}",
            self.q,
            self.n,
            self.target_se,
            self.best_backend,
            self.best_rate,
            self.required_n_planning,
            self.required_n_conservative,
            self.hours_planning,
            self.hours_conservative,
            self.feasible,
            self.attainable_n,
            self.attainable_se,
            prior,
            opt(self.prior_se),
            opt(self.prior_ci_half_width),
            self.precision_ratio
                .map_or_else(|| "none".to_string(), |r| format!("{r:.3}")),
            self.prior_comparison.name(),
        )
    }
}

/// Derive one envelope row from a measured composite rate.
#[must_use]
pub fn envelope_row(
    q: u64,
    n: usize,
    target_se: f64,
    best_backend: String,
    best_rate: f64,
    budget_hours: f64,
) -> EnvelopeRow {
    let p = 1.0 / q as f64;
    let required_planning = required_n(p, target_se);
    let required_conservative = required_n(0.5, target_se);
    let budget_s = effective_budget_s(budget_hours);
    let hours = |n_req: u64| n_req as f64 / best_rate / 3600.0;
    let attainable_n = (best_rate * budget_s) as u64;
    let attainable_se = if attainable_n == 0 {
        f64::NAN
    } else {
        se_at(p, attainable_n as f64)
    };
    let prior_trials = crate::prior::prior_trials(q, n);
    let prior_se = crate::prior::prior_se(q, n);
    EnvelopeRow {
        q,
        n,
        target_se,
        best_backend,
        best_rate,
        required_n_planning: required_planning,
        required_n_conservative: required_conservative,
        hours_planning: hours(required_planning),
        hours_conservative: hours(required_conservative),
        feasible: required_planning as f64 / best_rate <= budget_s,
        attainable_n,
        attainable_se,
        prior_trials,
        prior_se,
        prior_ci_half_width: crate::prior::prior_wilson_half_width(q, n),
        precision_ratio: prior_se.map(|se| se / attainable_se),
        prior_comparison: crate::prior::compare_precision(q, n, attainable_se),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_n_matches_hand_computed_values() {
        // p = 1/3, SE = 1e-3: 0.2222.../1e-6 = 222223 (rounded up).
        assert_eq!(required_n(1.0 / 3.0, 1e-3), 222_223);
        // Conservative p = 1/2, SE = 1e-4: 0.25/1e-8 = 25_000_000.
        assert_eq!(required_n(0.5, 1e-4), 25_000_000);
    }

    #[test]
    fn required_n_is_maximised_at_one_half() {
        let se = 1e-3;
        for q in [3.0, 5.0, 7.0] {
            assert!(required_n(1.0 / q, se) < required_n(0.5, se));
        }
    }

    #[test]
    fn wilson_interval_brackets_the_point_estimate() {
        let (lo, hi) = wilson_interval(3_333, 10_000, Z_95);
        assert!(lo < 0.3333 && 0.3333 < hi, "interval [{lo}, {hi}]");
        // Width shrinks as 1/sqrt(N).
        let (lo2, hi2) = wilson_interval(333_333, 1_000_000, Z_95);
        assert!(hi2 - lo2 < (hi - lo) / 9.0);
    }

    #[test]
    fn wilson_interval_stays_inside_the_unit_interval() {
        let (lo, hi) = wilson_interval(0, 50, Z_95);
        assert!(lo >= 0.0 && hi <= 1.0 && hi > 0.0);
    }

    #[test]
    fn effective_budget_applies_the_reserve() {
        assert!((effective_budget_s(12.0) - 12.0 * 3600.0 * 0.85).abs() < 1e-9);
    }
}
