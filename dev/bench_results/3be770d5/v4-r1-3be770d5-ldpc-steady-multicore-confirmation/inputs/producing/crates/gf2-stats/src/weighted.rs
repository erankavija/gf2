//! Exact statistics for weights represented as powers of an integer base.
//!
//! This module deliberately contains no scientific-event semantics. A caller
//! supplies exponent histograms for weights `base^-exponent`; reduction keeps
//! sums, run-mean variance, ESS, and interval containment exact. Rendering
//! uses an arbitrary-precision symbolic scale, so tiny absolute weights never
//! pass through binary floating point.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Add, Div, Mul};

use num_bigint::BigUint;

/// Internal exact nonnegative ratio in canonical reduced form.
#[derive(Clone, Debug, Eq)]
struct Ratio {
    numerator: BigUint,
    denominator: BigUint,
}

impl Ratio {
    /// Constructs and reduces an exact ratio.
    ///
    /// # Errors
    ///
    /// Returns [`WeightedError::ZeroDenominator`] for a zero denominator.
    fn new(
        numerator: impl Into<BigUint>,
        denominator: impl Into<BigUint>,
    ) -> Result<Self, WeightedError> {
        let numerator = numerator.into();
        let denominator = denominator.into();
        if denominator == BigUint::from(0_u8) {
            return Err(WeightedError::ZeroDenominator);
        }
        Ok(Self::reduced(numerator, denominator))
    }

    /// Returns zero.
    #[must_use]
    fn zero() -> Self {
        Self::from_integer(0_u8)
    }

    /// Returns one.
    #[must_use]
    fn one() -> Self {
        Self::from_integer(1_u8)
    }

    /// Constructs an exact integer ratio.
    #[must_use]
    fn from_integer(value: impl Into<BigUint>) -> Self {
        Self {
            numerator: value.into(),
            denominator: BigUint::from(1_u8),
        }
    }

    /// Returns the reduced numerator.
    #[must_use]
    fn decimal_pair(&self) -> (String, String) {
        (self.numerator.to_string(), self.denominator.to_string())
    }

    fn reduced(numerator: BigUint, denominator: BigUint) -> Self {
        if numerator == BigUint::from(0_u8) {
            return Self {
                numerator,
                denominator: BigUint::from(1_u8),
            };
        }
        let divisor = gcd(numerator.clone(), denominator.clone());
        Self {
            numerator: numerator / &divisor,
            denominator: denominator / divisor,
        }
    }

    fn checked_sub(&self, other: &Self) -> Option<Self> {
        if self < other {
            return None;
        }
        Some(Self::reduced(
            &self.numerator * &other.denominator - &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        ))
    }

    fn abs_diff(&self, other: &Self) -> Self {
        if self >= other {
            self.checked_sub(other)
                .expect("ordering proves subtraction")
        } else {
            other
                .checked_sub(self)
                .expect("ordering proves subtraction")
        }
    }
}

impl PartialEq for Ratio {
    fn eq(&self, other: &Self) -> bool {
        self.numerator == other.numerator && self.denominator == other.denominator
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.numerator * &other.denominator).cmp(&(&other.numerator * &self.denominator))
    }
}

impl Add for &Ratio {
    type Output = Ratio;
    fn add(self, other: Self) -> Self::Output {
        Ratio::reduced(
            &self.numerator * &other.denominator + &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
    }
}

impl Mul for &Ratio {
    type Output = Ratio;
    fn mul(self, other: Self) -> Self::Output {
        Ratio::reduced(
            &self.numerator * &other.numerator,
            &self.denominator * &other.denominator,
        )
    }
}

impl Div for &Ratio {
    type Output = Ratio;
    fn div(self, other: Self) -> Self::Output {
        assert!(
            other.numerator != BigUint::from(0_u8),
            "division by an exact zero ratio"
        );
        Ratio::reduced(
            &self.numerator * &other.denominator,
            &self.denominator * &other.numerator,
        )
    }
}

/// A validation or reduction failure for exponent-weighted statistics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WeightedError {
    /// A rational denominator was zero.
    ZeroDenominator,
    /// A weight base smaller than two was requested.
    InvalidBase(u32),
    /// An exponent exceeded the declared closed range.
    ExponentOutOfRange { exponent: u32, maximum: u32 },
    /// A histogram count overflowed `u64`.
    CountOverflow,
    /// No run was supplied, or one supplied run was empty.
    EmptyRun,
    /// Run histograms did not share one base and exponent bound.
    IncompatibleHistograms,
    /// Independent-run variance needs at least two runs.
    TooFewRuns,
    /// An exact interval needs at least one independent run.
    ZeroIndependentRuns,
    /// Decimal rendering requested fewer than two significant digits.
    InvalidPrecision(usize),
}

impl fmt::Display for WeightedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => formatter.write_str("an exact denominator must be positive"),
            Self::InvalidBase(base) => write!(formatter, "weight base {base} must be at least two"),
            Self::ExponentOutOfRange { exponent, maximum } => write!(
                formatter,
                "weight exponent {exponent} exceeds maximum {maximum}"
            ),
            Self::CountOverflow => formatter.write_str("histogram sample count exceeds u64"),
            Self::EmptyRun => formatter.write_str("every weighted run must contain a sample"),
            Self::IncompatibleHistograms => {
                formatter.write_str("weighted runs must share one base and exponent bound")
            }
            Self::TooFewRuns => formatter.write_str("run variance needs at least two runs"),
            Self::ZeroIndependentRuns => {
                formatter.write_str("an exact interval needs an independent run")
            }
            Self::InvalidPrecision(digits) => write!(
                formatter,
                "outward rendering needs at least two digits, received {digits}"
            ),
        }
    }
}

impl std::error::Error for WeightedError {}

/// Exact counts of observed weights `base^-exponent`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExponentHistogram {
    base: u32,
    maximum_exponent: u32,
    counts: BTreeMap<u32, u64>,
    sample_count: u64,
}

impl ExponentHistogram {
    /// Creates an empty closed-range histogram.
    pub fn new(base: u32, maximum_exponent: u32) -> Result<Self, WeightedError> {
        if base < 2 {
            return Err(WeightedError::InvalidBase(base));
        }
        Ok(Self {
            base,
            maximum_exponent,
            counts: BTreeMap::new(),
            sample_count: 0,
        })
    }

    /// Records one exponent.
    pub fn record(&mut self, exponent: u32) -> Result<(), WeightedError> {
        self.record_many(exponent, 1)
    }

    /// Records `count` identical exponents; zero count is an identity operation.
    pub fn record_many(&mut self, exponent: u32, count: u64) -> Result<(), WeightedError> {
        if exponent > self.maximum_exponent {
            return Err(WeightedError::ExponentOutOfRange {
                exponent,
                maximum: self.maximum_exponent,
            });
        }
        self.sample_count = self
            .sample_count
            .checked_add(count)
            .ok_or(WeightedError::CountOverflow)?;
        let entry = self.counts.entry(exponent).or_default();
        *entry = entry
            .checked_add(count)
            .ok_or(WeightedError::CountOverflow)?;
        if *entry == 0 {
            self.counts.remove(&exponent);
        }
        Ok(())
    }

    /// Returns the integer weight base.
    #[must_use]
    pub const fn base(&self) -> u32 {
        self.base
    }

    /// Returns the inclusive exponent bound.
    #[must_use]
    pub const fn maximum_exponent(&self) -> u32 {
        self.maximum_exponent
    }

    /// Returns the number of recorded weights.
    #[must_use]
    pub const fn sample_count(&self) -> u64 {
        self.sample_count
    }

    /// Iterates over nonzero bins in increasing exponent order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (u32, u64)> + '_ {
        self.counts
            .iter()
            .map(|(&exponent, &count)| (exponent, count))
    }

    /// Returns the smallest observed exponent, if any.
    #[must_use]
    pub fn minimum_exponent(&self) -> Option<u32> {
        self.counts.first_key_value().map(|(&exponent, _)| exponent)
    }

    /// Returns the largest observed exponent, if any.
    #[must_use]
    pub fn maximum_observed_exponent(&self) -> Option<u32> {
        self.counts.last_key_value().map(|(&exponent, _)| exponent)
    }

    fn exact_sum(&self, exponent_multiplier: u32) -> Ratio {
        let maximum = self
            .maximum_observed_exponent()
            .expect("nonempty histogram required before reduction")
            * exponent_multiplier;
        let base = BigUint::from(self.base);
        let denominator = base.pow(maximum);
        let numerator = self
            .counts
            .iter()
            .fold(BigUint::from(0_u8), |sum, (&exponent, &count)| {
                sum + BigUint::from(count) * base.pow(maximum - exponent * exponent_multiplier)
            });
        Ratio::reduced(numerator, denominator)
    }
}

/// Exact reduction of independent exponent-histogram runs.
#[derive(Clone, Debug)]
pub struct WeightedRuns {
    histograms: Vec<ExponentHistogram>,
    pooled_histogram: ExponentHistogram,
    run_means: Vec<Ratio>,
    weight_sum: Ratio,
    squared_weight_sum: Ratio,
    mean: Ratio,
    run_variance: Option<Ratio>,
    mean_variance: Option<Ratio>,
    final_weight_ess: Ratio,
    per_run_ess: Vec<Ratio>,
}

impl WeightedRuns {
    /// Reduces independent runs without floating-point weight conversion.
    ///
    /// # Errors
    ///
    /// Rejects no runs, empty runs, incompatible histogram domains, or count
    /// overflow. One run supports sums and ESS but not variance or intervals.
    ///
    /// # Complexity
    ///
    /// Linear in the total number of nonempty histogram bins.
    pub fn from_histograms(histograms: Vec<ExponentHistogram>) -> Result<Self, WeightedError> {
        let first = histograms.first().ok_or(WeightedError::EmptyRun)?;
        if histograms
            .iter()
            .any(|histogram| histogram.sample_count == 0)
        {
            return Err(WeightedError::EmptyRun);
        }
        if histograms.iter().any(|histogram| {
            histogram.base != first.base || histogram.maximum_exponent != first.maximum_exponent
        }) {
            return Err(WeightedError::IncompatibleHistograms);
        }
        let mut pooled = ExponentHistogram::new(first.base, first.maximum_exponent)?;
        for histogram in &histograms {
            for (exponent, count) in histogram.iter() {
                pooled.record_many(exponent, count)?;
            }
        }
        let weight_sum = pooled.exact_sum(1);
        let squared_weight_sum = pooled.exact_sum(2);
        let mean = &weight_sum / &Ratio::from_integer(pooled.sample_count);
        let run_means: Vec<_> = histograms
            .iter()
            .map(|histogram| &histogram.exact_sum(1) / &Ratio::from_integer(histogram.sample_count))
            .collect();
        let per_run_ess = histograms
            .iter()
            .map(|histogram| {
                let sum = histogram.exact_sum(1);
                &(&sum * &sum) / &histogram.exact_sum(2)
            })
            .collect();
        let final_weight_ess = &(&weight_sum * &weight_sum) / &squared_weight_sum;
        let (run_variance, mean_variance) = if run_means.len() >= 2 {
            let run_grand_mean = &run_means.iter().fold(Ratio::zero(), |sum, run| &sum + run)
                / &Ratio::from_integer(run_means.len());
            let squared_deviations = run_means.iter().fold(Ratio::zero(), |sum, run| {
                let difference = run.abs_diff(&run_grand_mean);
                &sum + &(&difference * &difference)
            });
            let variance = &squared_deviations / &Ratio::from_integer(run_means.len() - 1);
            let variance_of_mean = &variance / &Ratio::from_integer(run_means.len());
            (Some(variance), Some(variance_of_mean))
        } else {
            (None, None)
        };
        Ok(Self {
            histograms,
            pooled_histogram: pooled,
            run_means,
            weight_sum,
            squared_weight_sum,
            mean,
            run_variance,
            mean_variance,
            final_weight_ess,
            per_run_ess,
        })
    }

    /// Returns the independent-run count.
    #[must_use]
    pub fn run_count(&self) -> usize {
        self.histograms.len()
    }
    /// Returns the pooled sample count.
    #[must_use]
    pub const fn sample_count(&self) -> u64 {
        self.pooled_histogram.sample_count
    }
    /// Returns the exact pooled weight sum as reduced decimal integers.
    #[must_use]
    pub fn weight_sum_decimal(&self) -> (String, String) {
        self.weight_sum.decimal_pair()
    }
    /// Returns the exact pooled squared-weight sum as reduced decimal integers.
    #[must_use]
    pub fn squared_weight_sum_decimal(&self) -> (String, String) {
        self.squared_weight_sum.decimal_pair()
    }
    /// Returns the exact pooled mean weight as reduced decimal integers.
    #[must_use]
    pub fn mean_decimal(&self) -> (String, String) {
        self.mean.decimal_pair()
    }
    /// Returns exact run means as reduced decimal integer pairs in input order.
    #[must_use]
    pub fn run_means_decimal(&self) -> Vec<(String, String)> {
        self.run_means.iter().map(Ratio::decimal_pair).collect()
    }
    /// Returns sample variance of run means as reduced decimal integers.
    #[must_use]
    pub fn independent_run_variance_decimal(&self) -> Option<(String, String)> {
        self.run_variance.as_ref().map(Ratio::decimal_pair)
    }
    /// Returns variance of the grand mean as reduced decimal integers.
    #[must_use]
    pub fn mean_variance_decimal(&self) -> Option<(String, String)> {
        self.mean_variance.as_ref().map(Ratio::decimal_pair)
    }
    /// Returns final-weight ESS as reduced decimal integers.
    #[must_use]
    pub fn final_weight_ess_decimal(&self) -> (String, String) {
        self.final_weight_ess.decimal_pair()
    }
    /// Returns per-run final-weight ESS as reduced decimal pairs in input order.
    #[must_use]
    pub fn per_run_ess_decimal(&self) -> Vec<(String, String)> {
        self.per_run_ess.iter().map(Ratio::decimal_pair).collect()
    }
    /// Returns pooled ESS divided by sample count as reduced decimal integers.
    #[must_use]
    pub fn ess_fraction_decimal(&self) -> (String, String) {
        (&self.final_weight_ess / &Ratio::from_integer(self.sample_count())).decimal_pair()
    }
    /// Returns the largest normalized single-weight share as reduced decimal integers.
    #[must_use]
    pub fn largest_weight_share_decimal(&self) -> (String, String) {
        let exponent = self
            .pooled_histogram
            .minimum_exponent()
            .expect("runs are nonempty");
        (&power_inverse(self.pooled_histogram.base, exponent) / &self.weight_sum).decimal_pair()
    }
    /// Returns the largest run-mean share as reduced decimal integers.
    #[must_use]
    pub fn largest_run_mean_share_decimal(&self) -> (String, String) {
        let largest = self.run_means.iter().max().expect("runs are nonempty");
        let total = self
            .run_means
            .iter()
            .fold(Ratio::zero(), |sum, run| &sum + run);
        (largest / &total).decimal_pair()
    }
    /// Compares pooled ESS/sample count with an exact nonnegative threshold.
    pub fn ess_fraction_at_least(
        &self,
        threshold_numerator: impl Into<BigUint>,
        threshold_denominator: impl Into<BigUint>,
    ) -> Result<bool, WeightedError> {
        let threshold = Ratio::new(threshold_numerator, threshold_denominator)?;
        Ok(&self.final_weight_ess / &Ratio::from_integer(self.sample_count()) >= threshold)
    }

    /// Builds a scaled Student interval with an exact critical-value pair.
    pub fn student_interval(
        &self,
        critical_numerator: impl Into<BigUint>,
        critical_denominator: impl Into<BigUint>,
    ) -> Result<ScaledStudentInterval, WeightedError> {
        Ok(ScaledStudentInterval {
            center: self.mean.clone(),
            variance: self
                .mean_variance
                .clone()
                .ok_or(WeightedError::TooFewRuns)?,
            critical: Ratio::new(critical_numerator, critical_denominator)?,
            scale_base: self.pooled_histogram.base,
            scale_exponent: self
                .pooled_histogram
                .minimum_exponent()
                .expect("runs are nonempty"),
        })
    }
}

/// Exact center/variance/critical-value representation of a Student interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScaledStudentInterval {
    center: Ratio,
    variance: Ratio,
    critical: Ratio,
    scale_base: u32,
    scale_exponent: u32,
}

impl ScaledStudentInterval {
    /// Reconstructs an interval from exact center and independent-run variance pairs.
    ///
    /// This is the reusable validation path for a serialized summary whose
    /// histogram reduction has already been checked. `scale_exponent` is the
    /// minimum observed weight exponent and controls only underflow-safe
    /// rendering, never the exact containment result.
    #[allow(clippy::too_many_arguments)]
    pub fn from_exact_independent_runs(
        center_numerator: impl Into<BigUint>,
        center_denominator: impl Into<BigUint>,
        run_variance_numerator: impl Into<BigUint>,
        run_variance_denominator: impl Into<BigUint>,
        critical_numerator: impl Into<BigUint>,
        critical_denominator: impl Into<BigUint>,
        independent_runs: usize,
        scale_base: u32,
        scale_exponent: u32,
    ) -> Result<Self, WeightedError> {
        if independent_runs == 0 {
            return Err(WeightedError::ZeroIndependentRuns);
        }
        if scale_base < 2 {
            return Err(WeightedError::InvalidBase(scale_base));
        }
        let run_variance = Ratio::new(run_variance_numerator, run_variance_denominator)?;
        Ok(Self {
            center: Ratio::new(center_numerator, center_denominator)?,
            variance: &run_variance / &Ratio::from_integer(independent_runs),
            critical: Ratio::new(critical_numerator, critical_denominator)?,
            scale_base,
            scale_exponent,
        })
    }

    /// Tests clipped interval containment by exact squared comparison.
    pub fn contains_exact(
        &self,
        numerator: impl Into<BigUint>,
        denominator: impl Into<BigUint>,
    ) -> Result<bool, WeightedError> {
        let value = Ratio::new(numerator, denominator)?;
        if value > Ratio::one() {
            return Ok(false);
        }
        let difference = value.abs_diff(&self.center);
        Ok(&difference * &difference <= &(&self.critical * &self.critical) * &self.variance)
    }

    /// Renders clipped endpoints outward with at least the requested digits.
    pub fn render_outward(
        &self,
        significant_digits: usize,
    ) -> Result<RenderedInterval, WeightedError> {
        if significant_digits < 2 {
            return Err(WeightedError::InvalidPrecision(significant_digits));
        }
        let scale = Ratio::from_integer(BigUint::from(self.scale_base).pow(self.scale_exponent));
        let scaled_center = &self.center * &scale;
        let scaled_variance = &self.variance * &(&scale * &scale);
        let radius = &self.critical * &sqrt_upper(&scaled_variance, significant_digits + 8);
        let scaled_lower = scaled_center
            .checked_sub(&radius)
            .unwrap_or_else(Ratio::zero);
        let scaled_upper = std::cmp::min(&scaled_center + &radius, scale.clone());
        Ok(RenderedInterval {
            lower: render_ratio(
                &(&scaled_lower / &scale),
                significant_digits,
                Rounding::Down,
            ),
            upper: render_ratio(&(&scaled_upper / &scale), significant_digits, Rounding::Up),
        })
    }
}

/// Outward-rounded decimal interval endpoints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedInterval {
    /// Lower endpoint rounded downward.
    pub lower: String,
    /// Upper endpoint rounded upward.
    pub upper: String,
}

fn power_inverse(base: u32, exponent: u32) -> Ratio {
    Ratio::reduced(BigUint::from(1_u8), BigUint::from(base).pow(exponent))
}

fn gcd(mut left: BigUint, mut right: BigUint) -> BigUint {
    while right != BigUint::from(0_u8) {
        let remainder = &left % &right;
        left = right;
        right = remainder;
    }
    left
}

fn sqrt_upper(value: &Ratio, decimal_places: usize) -> Ratio {
    if value.numerator == BigUint::from(0_u8) {
        return Ratio::zero();
    }
    let scale = BigUint::from(10_u8).pow(decimal_places as u32);
    let scaled_square = &value.numerator * &scale * &scale;
    let quotient = &scaled_square / &value.denominator;
    let root = integer_sqrt(&quotient);
    let exact = &root * &root * &value.denominator == scaled_square;
    Ratio::reduced(root + u8::from(!exact), scale)
}

fn integer_sqrt(value: &BigUint) -> BigUint {
    if *value < BigUint::from(2_u8) {
        return value.clone();
    }
    let mut estimate = BigUint::from(1_u8) << (value.bits().div_ceil(2) as usize);
    loop {
        let next = (&estimate + value / &estimate) >> 1;
        if next >= estimate {
            return estimate;
        }
        estimate = next;
    }
}

#[derive(Clone, Copy)]
enum Rounding {
    Down,
    Up,
}

fn render_ratio(value: &Ratio, significant_digits: usize, rounding: Rounding) -> String {
    if value.numerator == BigUint::from(0_u8) {
        return "0e+0".to_owned();
    }
    let exponent = decimal_exponent(value);
    let places = significant_digits as i64 - 1 - exponent;
    let (quotient, remainder) = if places >= 0 {
        let scaled = &value.numerator * BigUint::from(10_u8).pow(places as u32);
        (&scaled / &value.denominator, &scaled % &value.denominator)
    } else {
        let divisor = &value.denominator * BigUint::from(10_u8).pow((-places) as u32);
        (&value.numerator / &divisor, &value.numerator % &divisor)
    };
    let mut rounded = quotient;
    if matches!(rounding, Rounding::Up) && remainder != BigUint::from(0_u8) {
        rounded += 1_u8;
    }
    let mut digits = rounded.to_string();
    let mut rendered_exponent = exponent;
    if digits.len() > significant_digits {
        rendered_exponent += 1;
        digits.truncate(significant_digits);
    }
    while digits.len() < significant_digits {
        digits.insert(0, '0');
    }
    let sign = if rendered_exponent >= 0 { "+" } else { "" };
    format!(
        "{}.{}e{sign}{rendered_exponent}",
        &digits[..1],
        &digits[1..]
    )
}

fn decimal_exponent(value: &Ratio) -> i64 {
    if value.numerator >= value.denominator {
        let mut exponent = 0_i64;
        let mut threshold = value.denominator.clone();
        while value.numerator >= &threshold * 10_u8 {
            threshold *= 10_u8;
            exponent += 1;
        }
        exponent
    } else {
        let mut exponent = -1_i64;
        let mut numerator = &value.numerator * 10_u8;
        while numerator < value.denominator {
            numerator *= 10_u8;
            exponent -= 1;
        }
        exponent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_arithmetic_is_reduced() {
        let half = Ratio::new(2_u8, 4_u8).unwrap();
        assert_eq!(half.decimal_pair(), ("1".to_owned(), "2".to_owned()));
        assert_eq!(&half + &half, Ratio::one());
    }

    #[test]
    fn integer_square_root_bounds() {
        for value in 0_u32..1000 {
            let root = integer_sqrt(&BigUint::from(value));
            assert!(&root * &root <= BigUint::from(value));
            assert!((&root + 1_u8) * (&root + 1_u8) > BigUint::from(value));
        }
    }
}
