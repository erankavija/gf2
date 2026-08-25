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

/// One deterministic Hamming-weight segment in the OSD pattern stream.
///
/// The theoretical range is the complete lexicographic group for `weight`.
/// The generated range is that group clipped by the configured candidate cap.
/// Consequently, a capped policy can retain descriptors for empty tail
/// segments without changing the pattern source's order or candidate identity.
///
/// # Panics
///
/// Constructing or copying a descriptor does not panic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatternSegment {
    weight: usize,
    theoretical_start: usize,
    theoretical_end: usize,
    generated_start: usize,
    generated_end: usize,
}

impl PatternSegment {
    /// Returns the Hamming weight of patterns in this segment.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn weight(&self) -> usize {
        self.weight
    }

    /// Returns the complete uncapped stream range for this segment.
    ///
    /// The range is half-open and uses the same zero-based generation indices
    /// as [`PatternEnumerator::generated`].
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn theoretical_range(&self) -> std::ops::Range<usize> {
        self.theoretical_start..self.theoretical_end
    }

    /// Returns the stream range available under the configured candidate cap.
    ///
    /// The range is half-open.  It is empty when the cap ends before this
    /// segment begins, which preserves a descriptor for every possible weight
    /// while making the cap boundary explicit.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn generated_range(&self) -> std::ops::Range<usize> {
        self.generated_start..self.generated_end
    }

    /// Returns the number of uncapped patterns in this segment.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn theoretical_pattern_count(&self) -> usize {
        self.theoretical_end - self.theoretical_start
    }

    /// Returns the number of patterns generated in this segment after the cap.
    ///
    /// This is the complexity metric exposed to a discard-threshold consumer;
    /// it is a planned count, not a count of candidates accepted by an adapter.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn generated_pattern_count(&self) -> usize {
        self.generated_end - self.generated_start
    }

    /// Reports whether the configured cap leaves this segment empty.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn is_empty(&self) -> bool {
        self.generated_start == self.generated_end
    }
}

/// Deterministic segmentation policy for one bounded OSD pattern source.
///
/// There is one descriptor for each weight from zero through
/// `min(order, dimension)`.  The descriptors are ordered by increasing weight,
/// and each descriptor's generated range is a prefix of its theoretical range.
/// This is the public policy representation a threshold evaluator can inspect
/// before deciding whether to retain or discard a complete weight segment.
///
/// # Panics
///
/// Constructing or copying a policy does not panic for a valid checked bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSegmentation {
    dimension: usize,
    max_weight: usize,
    theoretical_candidates: usize,
    candidate_cap: Option<usize>,
    segments: Vec<PatternSegment>,
}

impl PatternSegmentation {
    /// Builds the deterministic weight segmentation for `config`.
    ///
    /// The checked uncapped candidate bound is computed even when a cap is
    /// present.  A cap clips only generated ranges; it never changes the
    /// theoretical ranges or the order-m search definition.
    ///
    /// # Errors
    ///
    /// Returns [`PatternEnumerationError::CandidateBoundOverflow`] when the
    /// uncapped order-m bound cannot be represented by `usize`.
    ///
    /// # Panics
    ///
    /// This constructor never panics for any `dimension` or [`OsdConfig`].
    pub fn new(dimension: usize, config: OsdConfig) -> Result<Self, PatternEnumerationError> {
        let theoretical_candidates = checked_candidate_bound(dimension, config.order)?;
        let max_weight = config.order.min(dimension);
        let mut segments = Vec::with_capacity(max_weight + 1);
        let mut theoretical_start = 0;
        let mut coefficient = 1usize;

        for weight in 0..=max_weight {
            if weight != 0 {
                coefficient = next_binomial_coefficient(dimension, weight, coefficient).ok_or(
                    PatternEnumerationError::CandidateBoundOverflow {
                        dimension,
                        order: config.order,
                    },
                )?;
            }
            let theoretical_end = theoretical_start + coefficient;
            let generated_start = config
                .candidate_cap
                .map_or(theoretical_start, |cap| theoretical_start.min(cap));
            let generated_end = config
                .candidate_cap
                .map_or(theoretical_end, |cap| theoretical_end.min(cap));
            segments.push(PatternSegment {
                weight,
                theoretical_start,
                theoretical_end,
                generated_start,
                generated_end,
            });
            theoretical_start = theoretical_end;
        }

        Ok(Self {
            dimension,
            max_weight,
            theoretical_candidates,
            candidate_cap: config.candidate_cap,
            segments,
        })
    }

    /// Returns the information-set dimension used to construct the policy.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Returns the largest pattern weight represented by this policy.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn max_weight(&self) -> usize {
        self.max_weight
    }

    /// Returns the checked uncapped number of patterns in the search.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn theoretical_candidates(&self) -> usize {
        self.theoretical_candidates
    }

    /// Returns the candidate cap applied to generated ranges.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn candidate_cap(&self) -> Option<usize> {
        self.candidate_cap
    }

    /// Returns all weight descriptors, including empty capped tail segments.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub fn segments(&self) -> &[PatternSegment] {
        &self.segments
    }

    /// Returns the descriptor at `index`, if it exists.
    ///
    /// # Panics
    ///
    /// This accessor never panics for any index.
    pub fn segment(&self, index: usize) -> Option<&PatternSegment> {
        self.segments.get(index)
    }

    /// Returns whether no pattern is available under the configured cap.
    ///
    /// The policy can still contain nonempty theoretical ranges when this is
    /// true, for example for a zero candidate cap.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub fn is_empty(&self) -> bool {
        self.segments.iter().all(PatternSegment::is_empty)
    }

    fn segment_index_for_generated(&self, generation: usize) -> usize {
        self.segments
            .iter()
            .position(|segment| {
                segment.generated_start <= generation && generation < segment.generated_end
            })
            .expect("every generated pattern belongs to one nonempty segment")
    }
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
    /// Stop the run and report cancellation.  Cancellation takes precedence
    /// over a candidate cap observed at the same boundary.
    Cancel,
}

/// Why an OSD run stopped.
///
/// A pattern run reports the three reasons a pattern source can observe.
/// [`Self::InconsistentTransform`] belongs to the reprocessing engine, which
/// rejects a system with no solution before generating anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OsdTermination {
    /// Every subset through the configured order was generated.
    Exhaustive,
    /// The configured candidate cap was reached without cancellation.
    CandidateCap,
    /// The caller requested cancellation.
    Cancelled,
    /// The transformed right-hand side lies outside the reachable row space,
    /// so the system has no solution and no candidate was generated.
    InconsistentTransform,
}

/// Counters for one deterministic pattern segment.
///
/// The descriptor identifies the segment and the counters describe the
/// generated prefix actually visited by a segmented source.  `tested` counts
/// callbacks, matching [`PatternEnumerationReport::tested`].
///
/// # Panics
///
/// Constructing or copying a report does not panic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatternSegmentReport {
    segment: PatternSegment,
    generated: usize,
    tested: usize,
}

impl PatternSegmentReport {
    /// Returns the descriptor for this report.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn segment(&self) -> PatternSegment {
        self.segment
    }

    /// Returns the number of patterns generated in this segment.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn generated(&self) -> usize {
        self.generated
    }

    /// Returns the number of generated patterns passed to the visitor.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn tested(&self) -> usize {
        self.tested
    }
}

/// Counters and termination metadata for a segmented pattern run.
///
/// # Panics
///
/// Constructing or copying a report does not panic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSegmentEnumerationReport {
    theoretical_candidates: usize,
    generated: usize,
    tested: usize,
    segments: usize,
    termination: OsdTermination,
    segment_reports: Vec<PatternSegmentReport>,
}

impl PatternSegmentEnumerationReport {
    /// Returns the checked uncapped candidate bound.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn theoretical_candidates(&self) -> usize {
        self.theoretical_candidates
    }

    /// Returns the number of patterns emitted by the source.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn generated(&self) -> usize {
        self.generated
    }

    /// Returns the number of generated patterns passed to the visitor.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn tested(&self) -> usize {
        self.tested
    }

    /// Returns the number of nonempty segments visited before termination.
    ///
    /// Empty capped tail segments are present in [`Self::segment_reports`]
    /// but do not contribute to this counter.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn segments(&self) -> usize {
        self.segments
    }

    /// Returns the reason the segmented source stopped.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn termination(&self) -> OsdTermination {
        self.termination
    }

    /// Returns one report for every policy descriptor, including empty tails.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub fn segment_reports(&self) -> &[PatternSegmentReport] {
        &self.segment_reports
    }
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
    segmentation: PatternSegmentation,
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
        let segmentation = PatternSegmentation::new(dimension, config)?;
        let theoretical_candidates = segmentation.theoretical_candidates();
        let max_weight = segmentation.max_weight();
        let candidate_cap = config.candidate_cap;
        let cap_zero = candidate_cap == Some(0);

        Ok(Self {
            dimension,
            max_weight,
            theoretical_candidates,
            segmentation,
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

    /// Returns the deterministic weight segmentation used by this source.
    ///
    /// The policy is fixed at construction and remains unchanged as the
    /// iterator advances.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub fn segmentation(&self) -> &PatternSegmentation {
        &self.segmentation
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
    ///
    /// Cancellation takes precedence over a candidate cap that was reached by
    /// the most recently generated pattern but has not yet been observed by
    /// the caller.
    pub fn cancel(&mut self) {
        self.cancellation_requested = true;
        if self.termination == Some(OsdTermination::CandidateCap) {
            self.finish(OsdTermination::Cancelled);
        }
    }

    /// Visits generated patterns until the source is exhausted, capped, or
    /// cancelled by the visitor.
    ///
    /// The visitor is called exactly once for every pattern generated during
    /// this invocation.  Each callback is counted as one tested candidate.  A
    /// [`PatternControl::Cancel`]
    /// response stops immediately after that candidate and reports
    /// [`OsdTermination::Cancelled`].  If that candidate also reaches the
    /// configured cap, cancellation still takes precedence over
    /// [`OsdTermination::CandidateCap`].
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

    /// Visits the existing deterministic pattern stream with weight-segment
    /// accounting.
    ///
    /// The callback receives the descriptor for the generated pattern's
    /// weight segment and the pattern itself.  Callback order is identical to
    /// [`Self::run`], so a run with no active discard decision yields the same
    /// patterns and candidate order as the baseline source.  Empty capped
    /// tail descriptors receive zero counters in the returned report.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::osd::{OsdConfig, PatternControl, PatternEnumerator};
    ///
    /// let mut source = PatternEnumerator::new(3, OsdConfig::new(1)).unwrap();
    /// let report = source.run_segmented(|segment, pattern| {
    ///     assert_eq!(segment.weight(), pattern.len());
    ///     PatternControl::Continue
    /// });
    /// assert_eq!(report.segments(), 2);
    /// assert_eq!(report.generated(), 4);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the source's internal generation counter does not map a
    /// generated pattern to one of its policy segments.  A source constructed
    /// by [`Self::new`] maintains this invariant.
    pub fn run_segmented<F>(&mut self, mut visitor: F) -> PatternSegmentEnumerationReport
    where
        F: FnMut(&PatternSegment, &[usize]) -> PatternControl,
    {
        self.run_segmented_with_flag(None, &mut visitor)
    }

    /// Like [`Self::run_segmented`], but observes a caller-owned cancellation
    /// flag before every generated pattern.
    ///
    /// # Panics
    ///
    /// Panics under the same internal-invariant condition as
    /// [`Self::run_segmented`].
    pub fn run_segmented_with_cancellation<F>(
        &mut self,
        cancellation: &AtomicBool,
        mut visitor: F,
    ) -> PatternSegmentEnumerationReport
    where
        F: FnMut(&PatternSegment, &[usize]) -> PatternControl,
    {
        self.run_segmented_with_flag(Some(cancellation), &mut visitor)
    }

    fn run_segmented_with_flag<F>(
        &mut self,
        cancellation: Option<&AtomicBool>,
        visitor: &mut F,
    ) -> PatternSegmentEnumerationReport
    where
        F: FnMut(&PatternSegment, &[usize]) -> PatternControl,
    {
        let mut segment_reports: Vec<PatternSegmentReport> = self
            .segmentation
            .segments()
            .iter()
            .copied()
            .map(|segment| PatternSegmentReport {
                segment,
                generated: 0,
                tested: 0,
            })
            .collect();

        loop {
            if self.cancellation_requested
                || cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                self.finish(OsdTermination::Cancelled);
                break;
            }
            if self.termination.is_some() {
                break;
            }

            let Some(pattern) = self.next() else {
                break;
            };
            let generation = self.generated - 1;
            let segment_index = self.segmentation.segment_index_for_generated(generation);
            segment_reports[segment_index].generated += 1;
            self.tested += 1;
            segment_reports[segment_index].tested += 1;
            if visitor(&segment_reports[segment_index].segment, &pattern) == PatternControl::Cancel
            {
                self.finish(OsdTermination::Cancelled);
            }
        }

        PatternSegmentEnumerationReport {
            theoretical_candidates: self.theoretical_candidates,
            generated: self.generated,
            tested: self.tested,
            segments: segment_reports
                .iter()
                .filter(|report| report.generated != 0)
                .count(),
            termination: self
                .termination
                .expect("pattern runs always finish before reporting"),
            segment_reports,
        }
    }

    fn run_with_cancellation<F>(
        &mut self,
        mut visitor: Option<&mut F>,
        cancellation: Option<&AtomicBool>,
    ) -> PatternEnumerationReport
    where
        F: FnMut(&[usize]) -> PatternControl,
    {
        loop {
            if self.cancellation_requested
                || cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                self.finish(OsdTermination::Cancelled);
                break;
            }
            if self.termination.is_some() {
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
        if self.termination.is_none()
            || (termination == OsdTermination::Cancelled
                && self.termination == Some(OsdTermination::CandidateCap))
        {
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
        if self.cancellation_requested {
            self.finish(OsdTermination::Cancelled);
            return None;
        }
        if self.termination.is_some() {
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
        PatternEnumerationError, PatternEnumerator, PatternSegmentation,
    };

    #[test]
    fn segmentation_boundaries_follow_weight_transitions_and_keep_empty_tail() {
        let segmentation =
            PatternSegmentation::new(4, OsdConfig::new(2).with_candidate_cap(Some(5))).unwrap();

        let segments = segmentation.segments();
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].weight(), 0);
        assert_eq!(segments[0].theoretical_range(), 0..1);
        assert_eq!(segments[0].generated_range(), 0..1);
        assert_eq!(segments[1].weight(), 1);
        assert_eq!(segments[1].theoretical_range(), 1..5);
        assert_eq!(segments[1].generated_range(), 1..5);
        assert_eq!(segments[2].weight(), 2);
        assert_eq!(segments[2].theoretical_range(), 5..11);
        assert_eq!(segments[2].generated_range(), 5..5);
        assert_eq!(segments[2].generated_pattern_count(), 0);
    }

    #[test]
    fn segmented_report_counts_only_nonempty_segments() {
        let mut enumerator =
            PatternEnumerator::new(4, OsdConfig::new(2).with_candidate_cap(Some(5))).unwrap();
        let report = enumerator.run_segmented(|_, _| PatternControl::Continue);

        assert_eq!(report.segments(), 2);
        assert_eq!(report.generated(), 5);
        assert_eq!(report.tested(), 5);
        assert_eq!(report.termination(), OsdTermination::CandidateCap);
        assert_eq!(report.segment_reports()[0].generated(), 1);
        assert_eq!(report.segment_reports()[1].generated(), 4);
        assert_eq!(report.segment_reports()[2].generated(), 0);
    }

    #[test]
    fn zero_cap_exposes_empty_segments_without_invoking_the_visitor() {
        let segmentation =
            PatternSegmentation::new(3, OsdConfig::new(2).with_candidate_cap(Some(0))).unwrap();
        assert!(segmentation.is_empty());
        assert!(segmentation.segments().iter().all(|segment| {
            segment.generated_pattern_count() == 0
                && segment.generated_range().start == segment.generated_range().end
        }));

        let mut enumerator =
            PatternEnumerator::new(3, OsdConfig::new(2).with_candidate_cap(Some(0))).unwrap();
        let report = enumerator.run_segmented(|_, _| {
            panic!("an empty capped segmentation must not invoke the visitor")
        });
        assert_eq!(report.segments(), 0);
        assert_eq!(report.generated(), 0);
        assert_eq!(report.tested(), 0);
        assert_eq!(report.termination(), OsdTermination::CandidateCap);
    }

    #[test]
    fn exact_cap_at_weight_boundary_reports_cap_after_the_complete_segment() {
        let mut enumerator =
            PatternEnumerator::new(4, OsdConfig::new(2).with_candidate_cap(Some(5))).unwrap();
        let mut patterns = Vec::new();
        let report = enumerator.run_segmented(|_, pattern| {
            patterns.push(pattern.to_vec());
            PatternControl::Continue
        });

        assert_eq!(patterns, vec![vec![], vec![0], vec![1], vec![2], vec![3]]);
        assert_eq!(report.segments(), 2);
        assert_eq!(report.termination(), OsdTermination::CandidateCap);
        assert_eq!(report.segment_reports()[2].generated(), 0);
    }

    #[test]
    fn cancellation_mid_segment_preserves_the_observed_prefix() {
        let mut enumerator = PatternEnumerator::new(5, OsdConfig::new(2)).unwrap();
        let mut calls = 0;
        let report = enumerator.run_segmented(|segment, pattern| {
            calls += 1;
            assert_eq!(segment.weight(), pattern.len());
            if calls == 3 {
                PatternControl::Cancel
            } else {
                PatternControl::Continue
            }
        });

        assert_eq!(calls, 3);
        assert_eq!(report.segments(), 2);
        assert_eq!(report.generated(), 3);
        assert_eq!(report.tested(), 3);
        assert_eq!(report.segment_reports()[0].generated(), 1);
        assert_eq!(report.segment_reports()[1].generated(), 2);
        assert_eq!(report.termination(), OsdTermination::Cancelled);
    }

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
    fn cancellation_overrides_cap_on_the_final_candidate() {
        let config = OsdConfig::new(1).with_candidate_cap(Some(1));
        let report = enumerate_patterns(10, config, |_| PatternControl::Cancel).unwrap();

        assert_eq!(report.generated(), 1);
        assert_eq!(report.tested(), 1);
        assert_eq!(report.termination(), OsdTermination::Cancelled);
    }

    #[test]
    fn atomic_cancellation_overrides_cap_after_the_final_candidate() {
        let config = OsdConfig::new(1).with_candidate_cap(Some(1));
        let cancelled = AtomicBool::new(false);
        let report = {
            let mut enumerator = PatternEnumerator::new(10, config).unwrap();
            enumerator.enumerate_with_cancellation(&cancelled, |_| {
                cancelled.store(true, Ordering::Relaxed);
                PatternControl::Continue
            })
        };

        assert_eq!(report.generated(), 1);
        assert_eq!(report.tested(), 1);
        assert_eq!(report.termination(), OsdTermination::Cancelled);
    }

    #[test]
    fn direct_cancellation_overrides_a_reached_cap() {
        let config = OsdConfig::new(1).with_candidate_cap(Some(1));
        let mut enumerator = PatternEnumerator::new(10, config).unwrap();

        assert_eq!(enumerator.next(), Some(Vec::new()));
        enumerator.cancel();

        assert_eq!(enumerator.termination(), Some(OsdTermination::Cancelled));
        assert_eq!(enumerator.next(), None);
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
