use gf2_stats::intervals::{clopper_pearson_interval, wilson_interval, Z_95};

const LEVEL_95: f64 = 0.95;
const ENDPOINT_TOLERANCE: f64 = 3e-12;

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

/// Computes a binomial probability by its finite recurrence.  This is kept in
/// the contract test rather than sharing the estimator's beta-CDF routine, so
/// that it independently validates the defining binomial-tail equations.
fn binomial_probability(successes: u64, trials: u64, probability: f64) -> f64 {
    let mut term = (1.0 - probability).powf(trials as f64);
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

fn binomial_upper_tail(successes: u64, trials: u64, probability: f64) -> f64 {
    (successes..=trials)
        .map(|k| binomial_probability(k, trials, probability))
        .sum()
}

/// Independently inverts the finite binomial tails that define the equal-tailed
/// Clopper-Pearson interval.  The small trial counts below avoid underflow in
/// this direct probability recurrence.
fn binomial_tail_interval(successes: u64, trials: u64, level: f64) -> (f64, f64) {
    let tail_probability = (1.0 - level) / 2.0;

    let lower = if successes == 0 {
        0.0
    } else {
        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..100 {
            let midpoint = (low + high) / 2.0;
            if binomial_upper_tail(successes, trials, midpoint) < tail_probability {
                low = midpoint;
            } else {
                high = midpoint;
            }
        }
        (low + high) / 2.0
    };

    let upper = if successes == trials {
        1.0
    } else {
        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..100 {
            let midpoint = (low + high) / 2.0;
            if binomial_lower_tail(successes, trials, midpoint) > tail_probability {
                low = midpoint;
            } else {
                high = midpoint;
            }
        }
        (low + high) / 2.0
    };

    (lower, upper)
}

#[test]
fn clopper_pearson_matches_independent_binomial_tail_inversion() {
    for (successes, trials) in [(0, 50), (17, 50), (50, 50)] {
        let actual = clopper_pearson_interval(successes, trials, LEVEL_95);
        let expected = binomial_tail_interval(successes, trials, LEVEL_95);
        assert!(
            (actual.0 - expected.0).abs() <= ENDPOINT_TOLERANCE,
            "lower endpoint differs for {successes}/{trials}: {actual:?} vs {expected:?}"
        );
        assert!(
            (actual.1 - expected.1).abs() <= ENDPOINT_TOLERANCE,
            "upper endpoint differs for {successes}/{trials}: {actual:?} vs {expected:?}"
        );
    }
}

#[test]
fn clopper_pearson_is_well_formed_at_campaign_maximum() {
    let (lower, upper) = clopper_pearson_interval(2_857_143, 20_000_000, LEVEL_95);
    assert!(lower.is_finite() && upper.is_finite());
    assert!(0.0 <= lower && lower <= upper && upper <= 1.0);
}

#[derive(Clone, Copy)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
}

#[test]
fn clopper_pearson_empirically_covers_small_probability_regime() {
    // A fixed seed makes this a repeatable simulation.  At N = 20 and p =
    // 0.01, the 95% Clopper-Pearson interval has ample conservative coverage;
    // 20,000 repeated binomial draws retain that margin without a flaky test.
    const TRIALS: u64 = 20;
    const PROBABILITY: f64 = 0.01;
    const REPLICATES: u64 = 20_000;

    let intervals: Vec<_> = (0..=TRIALS)
        .map(|successes| clopper_pearson_interval(successes, TRIALS, LEVEL_95))
        .collect();
    let mut rng = SplitMix64(0xD1B6_A182_95C0_0001);
    let mut covered = 0;
    for _ in 0..REPLICATES {
        let successes = (0..TRIALS).filter(|_| rng.unit_f64() < PROBABILITY).count() as u64;
        let (lower, upper) = intervals[successes as usize];
        covered += u64::from(lower <= PROBABILITY && PROBABILITY <= upper);
    }

    let empirical_coverage = covered as f64 / REPLICATES as f64;
    assert!(
        empirical_coverage >= LEVEL_95,
        "coverage {empirical_coverage} is below nominal {LEVEL_95}"
    );
}
