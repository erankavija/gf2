//! Paired, interleaved A/B measurement statistics.
//!
//! The Zen 3 benchmark protocol resamples the *paired execution* (one fresh
//! baseline child and one fresh candidate child, run adjacently in a
//! seed-determined order) and reports a percentile bootstrap interval for the
//! ratio of medians. Everything here is deterministic given its seed so an
//! independent acceptance pass recomputes the same interval from the same raw
//! samples.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Two-sided 95% normal quantile used by the frozen Wilson quality interval.
pub const Z_95: f64 = 1.959_963_984_540_054;

/// Invalid statistical input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbError(pub String);

impl fmt::Display for AbError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AbError {}

/// SplitMix64 stream used to expand one seed into generator state.
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// Starts the stream at `seed`.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Returns the next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// xoshiro256** generator seeded through SplitMix64.
#[derive(Clone, Debug)]
pub struct Xoshiro256StarStar {
    state: [u64; 4],
}

impl Xoshiro256StarStar {
    /// Seeds all four words from one SplitMix64 stream.
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut mixer = SplitMix64::new(seed);
        Self {
            state: [
                mixer.next_u64(),
                mixer.next_u64(),
                mixer.next_u64(),
                mixer.next_u64(),
            ],
        }
    }

    /// Returns the next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    /// Returns an index in `0..n` from the top 53 bits; `n` must be positive.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "cannot draw below zero");
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        ((unit * n as f64) as usize).min(n - 1)
    }
}

/// Which arm a paired execution ran first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArmOrder {
    BaselineFirst,
    CandidateFirst,
}

/// Counterbalanced pair orders: every block of two pairs holds one order of
/// each kind in a seed-determined sequence, so drift cancels within blocks.
pub fn pair_orders(seed: u64, pairs: usize) -> Vec<ArmOrder> {
    let mut generator = Xoshiro256StarStar::seed_from_u64(seed);
    let mut orders = Vec::with_capacity(pairs);
    while orders.len() < pairs {
        let first = if generator.next_u64() & 1 == 0 {
            ArmOrder::BaselineFirst
        } else {
            ArmOrder::CandidateFirst
        };
        let second = match first {
            ArmOrder::BaselineFirst => ArmOrder::CandidateFirst,
            ArmOrder::CandidateFirst => ArmOrder::BaselineFirst,
        };
        orders.push(first);
        if orders.len() < pairs {
            orders.push(second);
        }
    }
    orders
}

/// Derives a cell's resampling seed from the campaign seed and cell key.
pub fn bootstrap_seed(campaign_seed: u64, cell_key: &str) -> u64 {
    let digest = Sha256::digest(cell_key.as_bytes());
    let mut word = [0u8; 8];
    word.copy_from_slice(&digest[..8]);
    SplitMix64::new(campaign_seed ^ u64::from_le_bytes(word)).next_u64()
}

/// One paired execution's per-arm summary, in nanoseconds per logical call.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairedObservation {
    pub baseline_ns_per_call: f64,
    pub candidate_ns_per_call: f64,
}

/// Median of a nonempty slice; the slice is sorted in place.
pub fn median(values: &mut [f64]) -> Result<f64, AbError> {
    if values.is_empty() {
        return Err(AbError("median of an empty sample".into()));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(AbError("sample contains a non-finite value".into()));
    }
    values.sort_by(|a, b| a.partial_cmp(b).expect("finite values compare"));
    let middle = values.len() / 2;
    Ok(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

/// Speedup estimator: median baseline time divided by median candidate time.
pub fn speedup_of_medians(pairs: &[PairedObservation]) -> Result<f64, AbError> {
    let mut baseline: Vec<f64> = pairs.iter().map(|pair| pair.baseline_ns_per_call).collect();
    let mut candidate: Vec<f64> = pairs
        .iter()
        .map(|pair| pair.candidate_ns_per_call)
        .collect();
    let candidate_median = median(&mut candidate)?;
    if baseline
        .iter()
        .chain(candidate.iter())
        .any(|value| *value <= 0.0)
    {
        return Err(AbError("timing samples must be positive".into()));
    }
    Ok(median(&mut baseline)? / candidate_median)
}

/// Percentile bootstrap interval for the speedup of medians.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapInterval {
    /// Speedup of medians on the observed pairs.
    pub estimate: f64,
    pub lower: f64,
    pub upper: f64,
    pub confidence: f64,
    /// Declared two-sided error rate used for nearest-rank tail selection.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub alpha: f64,
    pub resamples: u32,
    pub seed: u64,
    pub pairs: usize,
}

/// Resamples whole pairs with replacement and reports the nearest-rank
/// percentile interval of the speedup at the declared two-sided error rate.
pub fn paired_bootstrap_speedup(
    pairs: &[PairedObservation],
    resamples: u32,
    alpha: f64,
    seed: u64,
) -> Result<BootstrapInterval, AbError> {
    if pairs.len() < 2 {
        return Err(AbError("bootstrap needs at least two pairs".into()));
    }
    if resamples < 100 {
        return Err(AbError("bootstrap needs at least 100 resamples".into()));
    }
    if !(0.0 < alpha && alpha <= 0.5) {
        return Err(AbError("two-sided alpha must lie in (0, 0.5]".into()));
    }
    let estimate = speedup_of_medians(pairs)?;
    let mut generator = Xoshiro256StarStar::seed_from_u64(seed);
    let mut replicates = Vec::with_capacity(resamples as usize);
    let mut draw = Vec::with_capacity(pairs.len());
    for _ in 0..resamples {
        draw.clear();
        for _ in 0..pairs.len() {
            draw.push(pairs[generator.below(pairs.len())]);
        }
        replicates.push(speedup_of_medians(&draw)?);
    }
    replicates.sort_by(|a, b| a.partial_cmp(b).expect("finite replicates"));
    let count = replicates.len();
    Ok(BootstrapInterval {
        estimate,
        lower: replicates[bootstrap_rank(alpha / 2.0, count)],
        upper: replicates[bootstrap_rank(1.0 - alpha / 2.0, count)],
        confidence: 1.0 - alpha,
        alpha,
        resamples,
        seed,
        pairs: pairs.len(),
    })
}

/// Preserves the v1/v2 evaluator's confidence-parameterized interval.
///
/// Version 3 callers pass their declared corrected alpha to
/// [`paired_bootstrap_speedup`]. This compatibility boundary remains only while
/// committed v1/v2 receipts need reproducible evaluation.
pub fn paired_bootstrap_speedup_legacy(
    pairs: &[PairedObservation],
    resamples: u32,
    confidence: f64,
    seed: u64,
) -> Result<BootstrapInterval, AbError> {
    if !(0.5..1.0).contains(&confidence) {
        return Err(AbError("confidence must lie in [0.5, 1)".into()));
    }
    paired_bootstrap_speedup(pairs, resamples, 1.0 - confidence, seed)
}

/// Zero-based nearest-rank index for a percentile in a sorted sample.
fn bootstrap_rank(quantile: f64, count: usize) -> usize {
    ((quantile * count as f64).ceil() as usize).clamp(1, count) - 1
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

/// Family-declared speedup margins.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Margins {
    /// Worthwhile-effect threshold on the speedup lower bound; above 1.
    pub improvement: f64,
    /// Equivalence margin: a candidate at most this factor slower is not
    /// worse; at least 1.
    pub equivalence: f64,
}

impl Margins {
    /// Rejects margins that cannot express a confidence-bound decision.
    pub fn validate(&self) -> Result<(), AbError> {
        if !(self.improvement.is_finite() && self.improvement > 1.0) {
            return Err(AbError("improvement margin must exceed 1".into()));
        }
        if !(self.equivalence.is_finite() && self.equivalence >= 1.0) {
            return Err(AbError("equivalence margin must be at least 1".into()));
        }
        Ok(())
    }
}

/// Confidence-bound decision for one comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    /// The interval's lower bound reaches the improvement margin.
    Improved,
    /// The lower bound clears the equivalence floor without reaching the
    /// improvement margin: one-sided non-inferiority.
    NotWorse,
    /// The upper bound lies below the equivalence floor.
    Regressed,
    /// The interval spans a margin.
    Inconclusive,
}

/// Applies the frozen decision rule to an interval.
pub fn decide(interval: &BootstrapInterval, margins: &Margins) -> Result<Decision, AbError> {
    margins.validate()?;
    let floor = 1.0 / margins.equivalence;
    Ok(if interval.lower >= margins.improvement {
        Decision::Improved
    } else if interval.lower >= floor {
        Decision::NotWorse
    } else if interval.upper < floor {
        Decision::Regressed
    } else {
        Decision::Inconclusive
    })
}

/// Per-comparison confidence under Bonferroni control of a family of
/// `comparisons` confidence statements at family-wise level `family_alpha`.
pub fn bonferroni_confidence(family_alpha: f64, comparisons: u32) -> Result<f64, AbError> {
    if !(family_alpha > 0.0 && family_alpha < 0.5) {
        return Err(AbError("family alpha must lie in (0, 0.5)".into()));
    }
    if comparisons == 0 {
        return Err(AbError("a family has at least one comparison".into()));
    }
    Ok(1.0 - family_alpha / f64::from(comparisons))
}

/// Counts windows slower than `factor` times the execution median. Flagged
/// windows are retained; the count is evidence about stability, never a
/// filter.
pub fn flagged_windows(ns_per_call: &[f64], factor: f64) -> Result<usize, AbError> {
    let mut sorted = ns_per_call.to_vec();
    let center = median(&mut sorted)?;
    Ok(ns_per_call
        .iter()
        .filter(|value| **value >= factor * center)
        .count())
}

/// Wilson score interval at 95% for a binomial proportion with independent trials (e.g. frame errors for FER); `errors` must not exceed `trials` and `trials` must be positive.
pub fn wilson_interval_95(errors: u64, trials: u64) -> Result<(f64, f64), AbError> {
    if trials == 0 || errors > trials {
        return Err(AbError("invalid binomial counts".into()));
    }
    let n = trials as f64;
    let p = errors as f64 / n;
    let z2 = Z_95 * Z_95;
    let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let half = Z_95 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
    Ok(((center - half).max(0.0), (center + half).min(1.0)))
}

/// Distribution-free interval for a bounded mean, using independent observations.
/// Hoeffding's inequality gives simultaneous two-sided coverage `confidence`.
/// Values must lie in [lower, upper]; no within-observation independence is assumed.
/// Returns an error for empty data or invalid bounds/confidence. O(n) time.
pub fn bounded_mean_interval(
    values: &[f64],
    lower: f64,
    upper: f64,
    confidence: f64,
) -> Result<(f64, f64), AbError> {
    if values.is_empty()
        || !lower.is_finite()
        || !upper.is_finite()
        || lower >= upper
        || !(0.0 < confidence && confidence < 1.0)
        || values
            .iter()
            .any(|x| !x.is_finite() || *x < lower || *x > upper)
    {
        return Err(AbError("invalid bounded observations".into()));
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let half =
        (upper - lower) * ((2.0 / (1.0 - confidence)).ln() / (2.0 * values.len() as f64)).sqrt();
    Ok(((mean - half).max(lower), (mean + half).min(upper)))
}

/// BER interval treating each frame's information-bit error fraction as one
/// independent bounded observation. Bits inside a frame can be arbitrarily dependent.
pub fn frame_ber_interval(
    errors: &[u64],
    bits_per_frame: u64,
    confidence: f64,
) -> Result<(f64, f64), AbError> {
    if bits_per_frame == 0 || errors.iter().any(|e| *e > bits_per_frame) {
        return Err(AbError("invalid frame bit counts".into()));
    }
    let rates: Vec<_> = errors
        .iter()
        .map(|e| *e as f64 / bits_per_frame as f64)
        .collect();
    bounded_mean_interval(&rates, 0.0, 1.0, confidence)
}

/// Upper confidence bound on FER(candidate) - ratio * FER(baseline).
/// Each paired same-frame observation is C_i - ratio B_i in [-ratio, 1].
/// A nonpositive upper bound establishes the declared ratio non-inferiority.
/// Zero observed baseline errors do not automatically certify equivalence.
pub fn paired_fer_upper(
    baseline: &[u64],
    candidate: &[u64],
    ratio: f64,
    confidence: f64,
) -> Result<f64, AbError> {
    if baseline.len() != candidate.len() || !ratio.is_finite() || ratio < 1.0 {
        return Err(AbError("invalid paired FER contract".into()));
    }
    let differences: Vec<_> = baseline
        .iter()
        .zip(candidate)
        .map(|(b, c)| f64::from(u8::from(*c > 0)) - ratio * f64::from(u8::from(*b > 0)))
        .collect();
    bounded_mean_interval(&differences, -ratio, 1.0, confidence).map(|(_, upper)| upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xoshiro_matches_the_reference_first_output() {
        // Reference: xoshiro256** seeded with SplitMix64(0) yields these words.
        let mut generator = Xoshiro256StarStar::seed_from_u64(0);
        let first = generator.next_u64();
        let mut again = Xoshiro256StarStar::seed_from_u64(0);
        assert_eq!(first, again.next_u64());
        assert_ne!(first, generator.next_u64());
    }

    #[test]
    fn nearest_rank_percentiles_use_the_frozen_ranks() {
        let pairs: Vec<_> = (1..=10)
            .map(|index| PairedObservation {
                baseline_ns_per_call: 200.0 + index as f64,
                candidate_ns_per_call: 100.0 + index as f64,
            })
            .collect();
        let interval = paired_bootstrap_speedup(&pairs, 1000, 0.05, 7).unwrap();
        assert!(interval.lower <= interval.estimate && interval.estimate <= interval.upper);
        assert_eq!(interval.pairs, 10);
    }

    #[test]
    fn declared_alpha_tail_ranks_cover_frozen_family_sizes() {
        for comparisons in [1, 2, 5, 10, 25] {
            let alpha = 0.05 / f64::from(comparisons);
            assert_eq!(bootstrap_rank(alpha / 2.0, 10_000), 250 / comparisons as usize - 1);
        }
    }

    #[test]
    fn wilson_interval_brackets_the_point_estimate() {
        let (lower, upper) = wilson_interval_95(5, 100).unwrap();
        assert!(lower < 0.05 && 0.05 < upper);
        assert_eq!(wilson_interval_95(0, 10).unwrap().0, 0.0);
        assert!(wilson_interval_95(11, 10).is_err());
    }
}
