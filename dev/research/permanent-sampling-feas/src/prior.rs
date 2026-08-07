//! Published prior numerics for $\Pr[\mathrm{per}(A) = 0]$ over $\mathbb{F}_3$.
//!
//! Transcribed verbatim from [Scheinerman2024] (arXiv:2407.20205v2) §4,
//! "Distribution of permanent of random matrix over F_3". The campaign's
//! $q = 3$ arm is therefore a *reproduction and extension* target, not virgin
//! ground: any claim of novelty at $q = 3$ has to be stated against these
//! numbers.
//!
//! Notation follows the paper: $z(n) = |\{A \in \mathbb{F}_3^{n \times n} :
//! \mathrm{perm}(A) = 0\}|$.
//!
//! The paper's Conjecture 4.1 is the same statement the campaign tests at
//! $q = 3$: "As $n \to \infty$ the distribution of $\mathrm{perm}(A)$ for
//! $A \in \mathbb{F}_3^{n \times n}$ approaches the uniform distribution.
//! Equivalently, $\lim_{n \to \infty} z(n) / 3^{n^2} = 1/3$."
//!
//! The paper also states the resolution limit its own data reaches: "For
//! $n \le 13$ the proportions are statistically distinguishable from $1/3$,
//! but for larger $n$ they are not." Its simulations "took about one full day
//! on 128 processors".

/// Table 3 — exact counts, computed by full enumeration, for $n \le 5$.
///
/// `(n, z(n), published z(n)/3^(n^2))`.
pub const TABLE_3_EXACT: &[(usize, u128, f64)] = &[
    (1, 1, 0.3333),
    (2, 33, 0.4074),
    (3, 8_163, 0.4147),
    (4, 17_116_353, 0.3976),
    (5, 317_193_401_763, 0.3744),
];

/// Table 4 — Monte Carlo results for $6 \le n \le 30$.
///
/// `(n, zero count, log10(number of trials))`. The paper reports the trial
/// count only as a power of ten, so the exact trial count is
/// $10^{\text{log10\_trials}}$.
pub const TABLE_4_MONTE_CARLO: &[(usize, u64, u32)] = &[
    (6, 35_456_365_448, 11),
    (7, 34_209_345_718, 11),
    (8, 33_623_043_873, 11),
    (9, 33_417_515_901, 11),
    (10, 33_358_878_343, 11),
    (11, 3_334_206_857, 10),
    (12, 3_333_537_904, 10),
    (13, 3_333_483_177, 10),
    (14, 3_333_394_825, 10),
    (15, 333_332_350, 9),
    (16, 333_308_622, 9),
    (17, 333_314_098, 9),
    (18, 33_331_991, 8),
    (19, 33_338_438, 8),
    (20, 33_338_902, 8),
    (21, 3_332_782, 7),
    (22, 3_333_672, 7),
    (23, 3_336_968, 7),
    (24, 3_333_518, 7),
    (25, 3_332_961, 7),
    (26, 3_335_524, 7),
    (27, 3_332_955, 7),
    (28, 333_743, 6),
    (29, 334_097, 6),
    (30, 333_080, 6),
];

/// Largest `n` for which [Scheinerman2024] reports the measured proportion to
/// be statistically distinguishable from `1/3`.
pub const DISTINGUISHABLE_THROUGH_N: usize = 13;

/// Processor-hours behind Table 4: "about one full day on 128 processors".
pub const PRIOR_PROCESSOR_HOURS: f64 = 128.0 * 24.0;

/// Trials behind the published $q = 3$ estimate at `n`, if any.
#[must_use]
pub fn prior_trials(q: u64, n: usize) -> Option<u64> {
    if q != 3 {
        return None;
    }
    if let Some((_, _, log10)) = TABLE_4_MONTE_CARLO.iter().find(|(m, _, _)| *m == n) {
        return Some(10u64.pow(*log10));
    }
    // The exact rows are enumerations, not sampling; report the full space so a
    // comparison against a sampled N is still meaningful.
    TABLE_3_EXACT
        .iter()
        .find(|(m, _, _)| *m == n)
        .map(|(m, _, _)| 3u64.saturating_pow((m * m) as u32))
}

/// Published $\Pr[\mathrm{per} = 0]$ at `n` for $q = 3$, if any.
///
/// Derived from the counts rather than the paper's rounded fraction column,
/// except for the exact rows where the count is exact.
#[must_use]
pub fn prior_zero_fraction(q: u64, n: usize) -> Option<f64> {
    if q != 3 {
        return None;
    }
    if let Some((_, zeros, log10)) = TABLE_4_MONTE_CARLO.iter().find(|(m, _, _)| *m == n) {
        return Some(*zeros as f64 / 10f64.powi(*log10 as i32));
    }
    TABLE_3_EXACT
        .iter()
        .find(|(m, _, _)| *m == n)
        .map(|(_, _, frac)| *frac)
}

/// Standard error of the published $q = 3$ estimate at `n`, if any.
///
/// $\mathrm{SE} = \sqrt{\hat p (1 - \hat p) / N}$ using the paper's own
/// $\hat p$ and trial count. This is the quantity a new campaign has to beat:
/// matching the *number* of trials is not the goal, resolving the deviation is,
/// and two campaigns are comparable only through the precision they achieve.
///
/// Returns `None` for the exactly-enumerated rows ($n \le 5$), which have no
/// sampling error at all — a campaign cannot improve on an exact count.
#[must_use]
pub fn prior_se(q: u64, n: usize) -> Option<f64> {
    if q != 3 {
        return None;
    }
    TABLE_4_MONTE_CARLO
        .iter()
        .find(|(m, _, _)| *m == n)
        .map(|(_, zeros, log10)| {
            let trials = 10f64.powi(*log10 as i32);
            let p = *zeros as f64 / trials;
            (p * (1.0 - p) / trials).sqrt()
        })
}

/// Half-width of a 95 % Wilson interval around the published estimate at `n`.
///
/// Reported alongside [`prior_se`] because the paper itself publishes point
/// estimates without intervals; supplying the interval its own data implies is
/// part of what a reanalysis contributes.
#[must_use]
pub fn prior_wilson_half_width(q: u64, n: usize) -> Option<f64> {
    if q != 3 {
        return None;
    }
    TABLE_4_MONTE_CARLO
        .iter()
        .find(|(m, _, _)| *m == n)
        .map(|(_, zeros, log10)| {
            let trials = 10u64.pow(*log10);
            let (lo, hi) = crate::stats::wilson_interval(*zeros, trials, crate::stats::Z_95);
            (hi - lo) / 2.0
        })
}

/// How a campaign cell's attainable precision compares with the published one.
///
/// Deliberately not a verdict on novelty: a cell that merely matches the prior
/// precision still contributes an independent reproduction, which is why
/// `MatchesPrior` is a distinct outcome rather than a failure.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PriorComparison {
    /// No published estimate for this `(q, n)`.
    NoPrior,
    /// The published value is an exact enumeration; sampling cannot improve it.
    PriorExact,
    ExceedsPrior,
    MatchesPrior,
    BelowPrior,
}

impl PriorComparison {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            PriorComparison::NoPrior => "no_prior",
            PriorComparison::PriorExact => "prior_exact",
            PriorComparison::ExceedsPrior => "exceeds_prior_precision",
            PriorComparison::MatchesPrior => "matches_prior_precision",
            PriorComparison::BelowPrior => "below_prior_precision",
        }
    }
}

/// Compare an attainable standard error against the published one at `(q, n)`.
///
/// "Matches" is a +/-10 % band on the standard error, which is well inside the
/// run-to-run variation of the throughput that produced it.
#[must_use]
pub fn compare_precision(q: u64, n: usize, attainable_se: f64) -> PriorComparison {
    if q != 3 {
        return PriorComparison::NoPrior;
    }
    if TABLE_3_EXACT.iter().any(|(m, _, _)| *m == n) {
        return PriorComparison::PriorExact;
    }
    match prior_se(q, n) {
        None => PriorComparison::NoPrior,
        Some(se) if !attainable_se.is_finite() => {
            let _ = se;
            PriorComparison::BelowPrior
        }
        Some(se) if attainable_se < 0.9 * se => PriorComparison::ExceedsPrior,
        Some(se) if attainable_se <= 1.1 * se => PriorComparison::MatchesPrior,
        Some(_) => PriorComparison::BelowPrior,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The n=2 count is small enough to derive independently, which checks the
    /// transcription rather than trusting it. Over F_3,
    /// `per([[a,b],[c,d]]) = ad + bc`; counting solutions of `ad = -bc` gives
    /// `N(0)^2 + N(1)N(2) + N(2)N(1)` where `N(v)` counts ordered pairs with
    /// product `v`: `N(0) = 5`, `N(1) = N(2) = 2`. So `25 + 4 + 4 = 33`.
    #[test]
    fn table_3_n2_count_is_independently_derivable() {
        let mut zeros = 0u32;
        for a in 0..3u32 {
            for b in 0..3u32 {
                for c in 0..3u32 {
                    for d in 0..3u32 {
                        if (a * d + b * c) % 3 == 0 {
                            zeros += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(zeros, 33, "brute force must reproduce Table 3's z(2)");
        assert_eq!(TABLE_3_EXACT[1].1, u128::from(zeros));
        assert!((zeros as f64 / 81.0 - 0.4074).abs() < 5e-5);
    }

    /// Table 3's n=3 count is also small enough to enumerate: 3^9 = 19683.
    #[test]
    fn table_3_n3_count_is_independently_derivable() {
        let mut zeros = 0u32;
        for m in 0..19_683u32 {
            let mut d = [0u32; 9];
            let mut x = m;
            for slot in &mut d {
                *slot = x % 3;
                x /= 3;
            }
            // per = sum over the 6 permutations of S_3.
            let per = d[0] * d[4] * d[8]
                + d[0] * d[5] * d[7]
                + d[1] * d[3] * d[8]
                + d[1] * d[5] * d[6]
                + d[2] * d[3] * d[7]
                + d[2] * d[4] * d[6];
            if per % 3 == 0 {
                zeros += 1;
            }
        }
        assert_eq!(zeros, 8_163, "brute force must reproduce Table 3's z(3)");
        assert_eq!(TABLE_3_EXACT[2].1, u128::from(zeros));
    }

    #[test]
    fn table_4_fractions_are_near_one_third() {
        for &(n, zeros, log10) in TABLE_4_MONTE_CARLO {
            let p = zeros as f64 / 10f64.powi(log10 as i32);
            assert!(
                (0.32..0.42).contains(&p),
                "n={n} published fraction {p} is outside the plausible band"
            );
        }
    }

    /// The published standard errors must be the `sqrt(p(1-p)/N)` of the
    /// paper's own numbers, and must grow by a decade in variance each time the
    /// trial count drops by one.
    #[test]
    fn prior_se_tracks_the_published_trial_counts() {
        let se12 = prior_se(3, 12).expect("n=12 has a published estimate");
        let se16 = prior_se(3, 16).expect("n=16 has a published estimate");
        // 10^10 -> 10^9 trials is a sqrt(10) widening of the standard error.
        assert!(
            ((se16 / se12) - 10f64.sqrt()).abs() < 0.05,
            "se16/se12 = {} should be about sqrt(10)",
            se16 / se12
        );
        assert!((se12 - 4.714e-6).abs() < 1e-8, "se at n=12 was {se12}");
        assert!(prior_se(5, 12).is_none() && prior_se(7, 12).is_none());
        // The exactly enumerated rows carry no sampling error.
        assert!(prior_se(3, 4).is_none());
    }

    #[test]
    fn compare_precision_distinguishes_the_four_outcomes() {
        let se20 = prior_se(3, 20).expect("n=20 has a published estimate");
        assert_eq!(
            compare_precision(3, 20, se20 * 0.5),
            PriorComparison::ExceedsPrior
        );
        assert_eq!(
            compare_precision(3, 20, se20),
            PriorComparison::MatchesPrior
        );
        assert_eq!(
            compare_precision(3, 20, se20 * 2.0),
            PriorComparison::BelowPrior
        );
        assert_eq!(compare_precision(5, 20, 1e-9), PriorComparison::NoPrior);
        assert_eq!(compare_precision(3, 4, 1e-9), PriorComparison::PriorExact);
    }

    #[test]
    fn prior_trials_reports_the_grid_sizes() {
        assert_eq!(prior_trials(3, 12), Some(10_000_000_000));
        assert_eq!(prior_trials(3, 16), Some(1_000_000_000));
        assert_eq!(prior_trials(3, 20), Some(100_000_000));
        assert_eq!(prior_trials(3, 24), Some(10_000_000));
        assert_eq!(prior_trials(3, 28), Some(1_000_000));
        assert_eq!(prior_trials(5, 20), None);
        assert_eq!(prior_trials(7, 20), None);
    }
}
