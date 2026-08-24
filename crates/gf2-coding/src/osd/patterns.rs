use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

/// Baseline OSD search policy.
///
/// The search visits every subset of the information set whose Hamming weight
/// is at most [`Self::order`].  A candidate cap limits the number of patterns
/// visited, but the theoretical uncapped bound is still checked when an
/// enumerator is constructed.  This keeps a bounded run from silently
/// accepting a configuration whose exhaustive baseline cannot be represented
/// by the work metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OsdConfig {
    /// Maximum Hamming weight of an error pattern.
    pub order: usize,
    /// Optional maximum number of generated candidates.
    pub candidate_cap: Option<usize>,
}

impl OsdConfig {
    /// Creates an exhaustive order-`order` configuration.
    pub const fn new(order: usize) -> Self {
        Self {
            order,
            candidate_cap: None,
        }
    }

    /// Sets the optional maximum number of generated candidates.
    pub const fn with_candidate_cap(mut self, candidate_cap: Option<usize>) -> Self {
        self.candidate_cap = candidate_cap;
        self
    }

    /// Returns the checked uncapped candidate bound for an information-set
    /// dimension of `dimension`.
    ///
    /// The bound is `sum(binomial(dimension, weight))` for weights from zero
    /// through `min(order, dimension)`.  A configured candidate cap does not
    /// change this calculation.
    pub fn candidate_bound(&self, dimension: usize) -> Result<usize, PatternEnumerationError> {
        checked_candidate_bound(dimension, self.order)
    }
}

/// Computes the checked exhaustive order-`order` OSD candidate bound.
///
/// The result is the number of subsets of `0..dimension` having weights from
/// zero through `min(order, dimension)`.  The calculation uses exact
/// cancellation before each multiplication, so it accepts every bound that
/// fits in `usize` without allocating a table of binomial coefficients.
pub fn checked_candidate_bound(
    dimension: usize,
    order: usize,
) -> Result<usize, PatternEnumerationError> {
    let limit = order.min(dimension);
    let mut coefficient = 1usize;
    let mut total = 1usize;

    for weight in 1..=limit {
        coefficient = next_binomial_coefficient(dimension, weight, coefficient)
            .ok_or(PatternEnumerationError::CandidateBoundOverflow { dimension, order })?;
        total = total
            .checked_add(coefficient)
            .ok_or(PatternEnumerationError::CandidateBoundOverflow { dimension, order })?;
    }

    Ok(total)
}

/// Computes `binomial(dimension, weight)` from the preceding coefficient.
///
/// The denominator is cancelled against both factors before multiplying.  In
/// this recurrence the remaining denominator must be one because each
/// binomial coefficient is integral; retaining the check makes the arithmetic
/// failure explicit if the recurrence is changed in the future.
fn next_binomial_coefficient(dimension: usize, weight: usize, previous: usize) -> Option<usize> {
    let mut numerator = dimension - weight + 1;
    let mut denominator = weight;

    let common = gcd(numerator, denominator);
    numerator /= common;
    denominator /= common;

    let common = gcd(previous, denominator);
    let previous = previous / common;
    denominator /= common;

    if denominator != 1 {
        return None;
    }

    previous.checked_mul(numerator)
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// A failure to construct an OSD pattern source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternEnumerationError {
    /// The exhaustive order bound cannot be represented by `usize`.
    CandidateBoundOverflow {
        /// Information-set dimension used in the bound.
        dimension: usize,
        /// Requested OSD order.
        order: usize,
    },
}

impl fmt::Display for PatternEnumerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CandidateBoundOverflow { dimension, order } => write!(
                formatter,
                "OSD candidate bound overflows usize for dimension {dimension} and order {order}"
            ),
        }
    }
}

impl std::error::Error for PatternEnumerationError {}

/// The action a pattern visitor takes after testing one generated pattern.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternControl {
    /// Continue generating patterns.
    Continue,
    /// Stop the run and report cancellation.
    Cancel,
}

/// Why an OSD pattern run stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OsdTermination {
    /// Every subset through the configured order was generated.
    Exhaustive,
    /// The configured candidate cap was reached.
    CandidateCap,
    /// The caller requested cancellation.
    Cancelled,
}

/// Counters and termination metadata for one pattern run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatternEnumerationReport {
    theoretical_candidates: usize,
    generated: usize,
    tested: usize,
    termination: OsdTermination,
}

impl PatternEnumerationReport {
    /// Returns the checked uncapped candidate bound.
    pub const fn theoretical_candidates(&self) -> usize {
        self.theoretical_candidates
    }

    /// Returns the number of patterns handed to the visitor.
    pub const fn generated(&self) -> usize {
        self.generated
    }

    /// Returns the number of generated patterns whose visitor was invoked.
    pub const fn tested(&self) -> usize {
        self.tested
    }

    /// Returns the reason the run stopped.
    pub const fn termination(&self) -> OsdTermination {
        self.termination
    }
}

/// Streaming source for OSD Hamming-weight patterns.
///
/// Patterns are emitted in increasing weight order.  Within one weight they
/// are emitted in lexicographic order of their ascending zero-based indices;
/// for example, dimension four and order two yield `[]`, `[0]`, `[1]`,
/// `[2]`, `[3]`, `[0, 1]`, `[0, 2]`, and so on.  The source stores only the
/// current combination and never allocates the complete candidate set.
///
/// Construct with [`PatternEnumerator::new`].  Use the [`Iterator`] surface
/// when the caller owns testing and stopping, or [`PatternEnumerator::run`]
/// when generated and tested counters plus cancellation metadata should be
/// returned together.
pub struct PatternEnumerator {
    dimension: usize,
    max_weight: usize,
    theoretical_candidates: usize,
    candidate_cap: Option<usize>,
    next_pattern: Option<Vec<usize>>,
    generated: usize,
    tested: usize,
    termination: Option<OsdTermination>,
    cancellation_requested: bool,
}

impl PatternEnumerator {
    /// Creates a streaming source for an information set of `dimension` bits.
    ///
    /// Construction performs the complete checked uncapped bound calculation,
    /// even when `config` has a candidate cap.  It returns an error instead of
    /// constructing a source with unrepresentable counters.
    pub fn new(dimension: usize, config: OsdConfig) -> Result<Self, PatternEnumerationError> {
        let theoretical_candidates = config.candidate_bound(dimension)?;
        let max_weight = config.order.min(dimension);
        let candidate_cap = config.candidate_cap;
        let cap_zero = candidate_cap == Some(0);

        Ok(Self {
            dimension,
            max_weight,
            theoretical_candidates,
            candidate_cap,
            next_pattern: (!cap_zero).then(Vec::new),
            generated: 0,
            tested: 0,
            termination: cap_zero.then_some(OsdTermination::CandidateCap),
            cancellation_requested: false,
        })
    }

    /// Returns the information-set dimension.
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Returns the checked uncapped candidate bound.
    pub const fn theoretical_candidates(&self) -> usize {
        self.theoretical_candidates
    }

    /// Returns the number of patterns emitted by this source.
    pub const fn generated(&self) -> usize {
        self.generated
    }

    /// Returns the number of patterns passed to a visitor by
    /// [`Self::run`].  Direct [`Iterator`] use does not increment this
    /// counter because the iterator does not test candidates on the caller's
    /// behalf.
    pub const fn tested(&self) -> usize {
        self.tested
    }

    /// Returns the termination reason once the source has stopped.
    pub const fn termination(&self) -> Option<OsdTermination> {
        self.termination
    }

    /// Requests cancellation before the next pattern is generated.
    pub fn cancel(&mut self) {
        self.cancellation_requested = true;
    }

    /// Visits generated patterns until the source is exhausted, capped, or
    /// cancelled by the visitor.
    ///
    /// The visitor is called exactly once for every generated pattern.  Each
    /// callback is counted as one tested candidate.  A [`PatternControl::Cancel`]
    /// response stops immediately after that candidate and reports
    /// [`OsdTermination::Cancelled`].
    pub fn run<F>(&mut self, mut visitor: F) -> PatternEnumerationReport
    where
        F: FnMut(&[usize]) -> PatternControl,
    {
        self.run_with_cancellation(Some(&mut visitor), None)
    }

    /// Like [`Self::run`], but observes a caller-owned cancellation flag
    /// before each candidate.
    pub fn enumerate_with_cancellation<F>(
        &mut self,
        cancellation: &AtomicBool,
        mut visitor: F,
    ) -> PatternEnumerationReport
    where
        F: FnMut(&[usize]) -> PatternControl,
    {
        self.run_with_cancellation(Some(&mut visitor), Some(cancellation))
    }

    fn run_with_cancellation<F>(
        &mut self,
        mut visitor: Option<&mut F>,
        cancellation: Option<&AtomicBool>,
    ) -> PatternEnumerationReport
    where
        F: FnMut(&[usize]) -> PatternControl,
    {
        while self.termination.is_none() {
            if self.cancellation_requested
                || cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                self.finish(OsdTermination::Cancelled);
                break;
            }

            let Some(pattern) = self.next() else {
                break;
            };

            self.tested += 1;
            if let Some(visitor) = visitor.as_mut() {
                if visitor(&pattern) == PatternControl::Cancel {
                    self.finish(OsdTermination::Cancelled);
                }
            }
        }

        self.report()
    }

    fn finish(&mut self, termination: OsdTermination) {
        if self.termination.is_none() {
            self.termination = Some(termination);
            self.next_pattern = None;
        }
    }

    fn report(&self) -> PatternEnumerationReport {
        PatternEnumerationReport {
            theoretical_candidates: self.theoretical_candidates,
            generated: self.generated,
            tested: self.tested,
            termination: self
                .termination
                .expect("pattern runs always finish before reporting"),
        }
    }
}

impl Iterator for PatternEnumerator {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.termination.is_some() {
            return None;
        }
        if self.cancellation_requested {
            self.finish(OsdTermination::Cancelled);
            return None;
        }
        if self
            .candidate_cap
            .is_some_and(|candidate_cap| self.generated >= candidate_cap)
        {
            self.finish(OsdTermination::CandidateCap);
            return None;
        }

        let Some(pattern) = self.next_pattern.take() else {
            self.finish(OsdTermination::Exhaustive);
            return None;
        };
        self.next_pattern = next_pattern(&pattern, self.dimension, self.max_weight);
        self.generated += 1;

        if self
            .candidate_cap
            .is_some_and(|candidate_cap| self.generated >= candidate_cap)
        {
            self.finish(OsdTermination::CandidateCap);
        }

        Some(pattern)
    }
}

fn next_pattern(current: &[usize], dimension: usize, max_weight: usize) -> Option<Vec<usize>> {
    if current.is_empty() {
        return (max_weight >= 1).then(|| vec![0]);
    }

    let weight = current.len();
    for pivot in (0..weight).rev() {
        let largest = dimension - (weight - pivot);
        if current[pivot] < largest {
            let mut successor = current.to_vec();
            successor[pivot] += 1;
            for index in pivot + 1..weight {
                successor[index] = successor[index - 1] + 1;
            }
            return Some(successor);
        }
    }

    (weight < max_weight).then(|| (0..weight + 1).collect())
}

/// Runs an OSD pattern source without exposing its iterator state.
pub fn enumerate_patterns<F>(
    dimension: usize,
    config: OsdConfig,
    visitor: F,
) -> Result<PatternEnumerationReport, PatternEnumerationError>
where
    F: FnMut(&[usize]) -> PatternControl,
{
    let mut enumerator = PatternEnumerator::new(dimension, config)?;
    Ok(enumerator.run(visitor))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use proptest::prelude::*;

    use super::{
        checked_candidate_bound, enumerate_patterns, OsdConfig, OsdTermination, PatternControl,
        PatternEnumerationError, PatternEnumerator,
    };

    #[test]
    fn enumerates_empty_and_singleton_patterns_in_order() {
        let mut enumerator = PatternEnumerator::new(3, OsdConfig::new(1)).unwrap();
        let patterns: Vec<_> = enumerator.by_ref().collect();

        assert_eq!(patterns, vec![vec![], vec![0], vec![1], vec![2]]);
        assert_eq!(enumerator.generated(), 4);
        assert_eq!(enumerator.termination(), Some(OsdTermination::Exhaustive));
    }

    #[test]
    fn order_zero_contains_only_the_empty_pattern() {
        let mut enumerator = PatternEnumerator::new(5, OsdConfig::new(0)).unwrap();
        let report = enumerator.run(|pattern| {
            assert!(pattern.is_empty());
            PatternControl::Continue
        });

        assert_eq!(report.theoretical_candidates(), 1);
        assert_eq!(report.generated(), 1);
        assert_eq!(report.tested(), 1);
        assert_eq!(report.termination(), OsdTermination::Exhaustive);
    }

    #[test]
    fn enumerates_each_weight_lexicographically() {
        let mut patterns = Vec::new();
        let report = enumerate_patterns(4, OsdConfig::new(2), |pattern| {
            patterns.push(pattern.to_vec());
            PatternControl::Continue
        })
        .unwrap();

        assert_eq!(
            patterns,
            vec![
                vec![],
                vec![0],
                vec![1],
                vec![2],
                vec![3],
                vec![0, 1],
                vec![0, 2],
                vec![0, 3],
                vec![1, 2],
                vec![1, 3],
                vec![2, 3],
            ]
        );
        assert_eq!(report.generated(), 11);
        assert_eq!(report.termination(), OsdTermination::Exhaustive);
    }

    #[test]
    fn order_above_dimension_is_clamped_to_the_dimension() {
        let report =
            enumerate_patterns(3, OsdConfig::new(usize::MAX), |_| PatternControl::Continue)
                .unwrap();

        assert_eq!(report.theoretical_candidates(), 8);
        assert_eq!(report.generated(), 8);
        assert_eq!(report.tested(), 8);
        assert_eq!(report.termination(), OsdTermination::Exhaustive);
    }

    #[test]
    fn zero_dimension_has_one_empty_pattern() {
        let report = enumerate_patterns(0, OsdConfig::new(7), |pattern| {
            assert!(pattern.is_empty());
            PatternControl::Continue
        })
        .unwrap();

        assert_eq!(report.theoretical_candidates(), 1);
        assert_eq!(report.generated(), 1);
        assert_eq!(report.tested(), 1);
        assert_eq!(report.termination(), OsdTermination::Exhaustive);
    }

    #[test]
    fn exact_candidate_cap_is_reported_without_allocating_the_rest() {
        let config = OsdConfig::new(3).with_candidate_cap(Some(4));
        let report = enumerate_patterns(8, config, |_| PatternControl::Continue).unwrap();

        assert_eq!(report.theoretical_candidates(), 93);
        assert_eq!(report.generated(), 4);
        assert_eq!(report.tested(), 4);
        assert_eq!(report.termination(), OsdTermination::CandidateCap);
    }

    #[test]
    fn cap_equal_to_the_exhaustive_bound_still_reports_cap_termination() {
        let config = OsdConfig::new(2).with_candidate_cap(Some(11));
        let report = enumerate_patterns(4, config, |_| PatternControl::Continue).unwrap();

        assert_eq!(report.theoretical_candidates(), 11);
        assert_eq!(report.generated(), 11);
        assert_eq!(report.tested(), 11);
        assert_eq!(report.termination(), OsdTermination::CandidateCap);
    }

    #[test]
    fn zero_candidate_cap_generates_nothing() {
        let report = enumerate_patterns(8, OsdConfig::new(2).with_candidate_cap(Some(0)), |_| {
            panic!("a zero cap must not invoke the visitor")
        })
        .unwrap();

        assert_eq!(report.generated(), 0);
        assert_eq!(report.tested(), 0);
        assert_eq!(report.termination(), OsdTermination::CandidateCap);
    }

    #[test]
    fn visitor_cancellation_reports_only_tested_patterns() {
        let mut calls = 0;
        let report = enumerate_patterns(10, OsdConfig::new(4), |_| {
            calls += 1;
            PatternControl::Cancel
        })
        .unwrap();

        assert_eq!(calls, 1);
        assert_eq!(report.generated(), 1);
        assert_eq!(report.tested(), 1);
        assert_eq!(report.termination(), OsdTermination::Cancelled);
    }

    #[test]
    fn atomic_cancellation_can_stop_before_the_first_callback() {
        let cancelled = AtomicBool::new(true);
        let mut enumerator = PatternEnumerator::new(10, OsdConfig::new(4)).unwrap();
        let report = enumerator.enumerate_with_cancellation(&cancelled, |_| {
            panic!("the cancelled source must not invoke the visitor")
        });

        assert_eq!(report.generated(), 0);
        assert_eq!(report.tested(), 0);
        assert_eq!(report.termination(), OsdTermination::Cancelled);
        assert!(cancelled.load(Ordering::Relaxed));
    }

    #[test]
    fn checked_bound_rejects_unrepresentable_growth() {
        let error = checked_candidate_bound(usize::MAX, 2).unwrap_err();

        assert_eq!(
            error,
            PatternEnumerationError::CandidateBoundOverflow {
                dimension: usize::MAX,
                order: 2,
            }
        );
    }

    #[test]
    fn checked_bound_accepts_the_largest_order_one_ball() {
        assert_eq!(
            checked_candidate_bound(usize::MAX - 1, 1).unwrap(),
            usize::MAX
        );
    }

    proptest! {
        #[test]
        fn every_small_hamming_ball_is_unique_and_complete(
            dimension in 0usize..8,
            order in 0usize..10,
        ) {
            let mut patterns = Vec::new();
            let report = enumerate_patterns(dimension, OsdConfig::new(order), |pattern| {
                patterns.push(pattern.to_vec());
                PatternControl::Continue
            }).unwrap();

            let limit = order.min(dimension);
            let expected = checked_candidate_bound(dimension, order).unwrap();
            prop_assert_eq!(report.theoretical_candidates(), expected);
            prop_assert_eq!(patterns.len(), expected);
            prop_assert_eq!(report.generated(), expected);
            prop_assert_eq!(report.tested(), expected);

            let mut sorted = patterns.clone();
            sorted.sort();
            sorted.dedup();
            prop_assert_eq!(sorted.len(), expected);
            let valid_patterns = patterns.iter().all(|pattern| {
                pattern.len() <= limit
                    && pattern.windows(2).all(|window| window[0] < window[1])
                    && pattern.iter().all(|&index| index < dimension)
            });
            prop_assert!(valid_patterns);
        }
    }
}
