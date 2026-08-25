//! Most-reliable-basis reprocessing shared by every OSD semantic adapter.
//!
//! The engine owns the parts of ordered-statistics decoding that do not depend
//! on what a candidate means: the reliability-ordered independent basis, the
//! bounded order-`m` reprocessing loop, the soft ranking, and the work
//! metadata.  A semantic adapter supplies the initial basis representation, the
//! candidate reconstruction, and the validity rule.

use std::fmt;
use std::sync::atomic::AtomicBool;

use gf2_core::alg::rref::{
    ordered_column_elimination, OrderedEliminationError, OrderedEliminationResult,
};
use gf2_core::{BitMatrix, BitVec};

use crate::llr::ReliabilityPermutation;

use super::patterns::{PatternSegment, PatternSegmentation};
use super::{
    OsdConfig, OsdTermination, PatternControl, PatternEnumerationError, PatternEnumerator,
};

#[cfg(test)]
mod segmentation_tests {
    use std::sync::atomic::AtomicBool;

    use gf2_core::{BitMatrix, BitVec};

    use super::{
        reprocess, reprocess_segmented, ColumnPreference, MostReliableBasis, OsdConfig,
        OsdSemantics, OsdTermination, ReprocessedColumns,
    };

    struct RowSpace;

    impl OsdSemantics for RowSpace {
        fn reprocessed_columns(&self) -> ReprocessedColumns {
            ReprocessedColumns::Basis
        }

        fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
            let elimination = basis.elimination();
            let mut message = BitVec::zeros(elimination.reduced.rows());
            for (row, &column) in elimination.selected_cols.iter().enumerate() {
                message.set(row, basis.reference().get(column));
            }
            elimination.reduced.matvec_transpose(&message)
        }

        fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
            (0..basis.elimination().rank)
                .map(|row| basis.elimination().reduced.row_as_bitvec(row))
                .collect()
        }

        fn accepts(&self, _candidate: &BitVec) -> bool {
            true
        }
    }

    fn basis() -> MostReliableBasis {
        let mut matrix = BitMatrix::zeros(1, 4);
        for column in [0, 1, 2, 3] {
            matrix.set(0, column, true);
        }
        MostReliableBasis::build(
            &matrix,
            &[4.0, 3.0, 2.0, 1.0],
            &BitVec::ones(4),
            None,
            ColumnPreference::MostReliableFirst,
        )
        .unwrap()
    }

    #[test]
    fn segmentation_without_discard_is_baseline_equivalent() {
        let basis = basis();
        let config = OsdConfig::new(1);
        let baseline = reprocess(&basis, &RowSpace, config).unwrap();
        let segmented = reprocess_segmented(&basis, &RowSpace, config).unwrap();

        assert_eq!(segmented.outcome(), baseline);
        assert_eq!(segmented.work().segments(), 2);
        assert_eq!(segmented.work().eliminations(), 1);
        assert_eq!(segmented.work().generated_patterns(), 2);
        assert_eq!(segmented.work().tested_candidates(), 2);
        assert_eq!(segmented.work().discarded_patterns(), 0);
    }

    #[test]
    fn policy_exposes_the_generated_pattern_metric_at_each_boundary() {
        let basis = basis();
        let segmented = reprocess_segmented(
            &basis,
            &RowSpace,
            OsdConfig::new(2).with_candidate_cap(Some(2)),
        )
        .unwrap();

        let policy = segmented.work().policy();
        assert_eq!(
            policy.metric(),
            super::OsdComplexityMetric::GeneratedPatterns
        );
        assert_eq!(policy.segment_metric(0), Some(1));
        assert_eq!(policy.segment_metric(1), Some(1));
        assert_eq!(policy.segment_metric(2), None);
        assert_eq!(
            segmented.work().termination(),
            super::OsdTermination::CandidateCap
        );
    }

    #[test]
    fn segmented_engine_reports_exact_cap_and_cancellation() {
        let basis = basis();
        let capped = reprocess_segmented(
            &basis,
            &RowSpace,
            OsdConfig::new(2).with_candidate_cap(Some(1)),
        )
        .unwrap();
        assert_eq!(capped.work().segments(), 1);
        assert_eq!(capped.work().generated_patterns(), 1);
        assert_eq!(capped.work().tested_candidates(), 1);
        assert_eq!(capped.work().discarded_patterns(), 0);
        assert_eq!(
            capped.work().termination(),
            super::OsdTermination::CandidateCap
        );

        let cancelled = AtomicBool::new(true);
        let cancelled_outcome = super::reprocess_segmented_with_cancellation(
            &basis,
            &RowSpace,
            OsdConfig::new(2),
            &cancelled,
        )
        .unwrap();
        assert_eq!(
            cancelled_outcome.work().termination(),
            super::OsdTermination::Cancelled
        );
    }

    #[test]
    fn discard_threshold_equality_retains_the_segment() {
        let basis = basis();
        let baseline = reprocess(&basis, &RowSpace, OsdConfig::new(1)).unwrap();
        let thresholded = basis
            .reprocess_segmented_with_discard_threshold(&RowSpace, OsdConfig::new(1), Some(1))
            .unwrap();

        assert_eq!(thresholded.outcome(), baseline);
        assert_eq!(thresholded.work().discarded_patterns(), 0);
        assert_eq!(thresholded.work().tested_candidates(), 2);
        assert_eq!(thresholded.work().eliminations(), 1);
        assert_eq!(thresholded.work().termination(), OsdTermination::Exhaustive);
        assert_eq!(thresholded.work().policy().discard_threshold(), Some(1));
        assert!(thresholded
            .work()
            .segment_work()
            .iter()
            .all(|segment| segment.discarded_patterns() == 0));
    }

    #[test]
    fn discard_threshold_that_is_not_triggered_is_baseline_equivalent() {
        let basis = basis();
        let baseline = reprocess(&basis, &RowSpace, OsdConfig::new(2)).unwrap();
        let thresholded = basis
            .reprocess_segmented_with_discard_threshold(
                &RowSpace,
                OsdConfig::new(2),
                Some(usize::MAX),
            )
            .unwrap();

        assert_eq!(thresholded.outcome(), baseline);
        assert_eq!(thresholded.work().discarded_patterns(), 0);
        assert_eq!(thresholded.work().tested_candidates(), 2);
        assert_eq!(thresholded.work().eliminations(), 1);
        assert_eq!(thresholded.work().termination(), OsdTermination::Exhaustive);
        assert!(thresholded
            .work()
            .segment_work()
            .iter()
            .all(|segment| segment.tested_candidates() == segment.generated_patterns()));
    }

    #[test]
    fn discard_threshold_can_discard_every_pattern() {
        let basis = basis();
        let thresholded = basis
            .reprocess_segmented_with_discard_threshold(&RowSpace, OsdConfig::new(2), Some(0))
            .unwrap();

        assert!(thresholded.best().is_none());
        assert_eq!(thresholded.work().discarded_patterns(), 2);
        assert_eq!(thresholded.work().tested_candidates(), 0);
        assert_eq!(thresholded.work().eliminations(), 1);
        assert_eq!(thresholded.work().termination(), OsdTermination::Exhaustive);
        assert!(thresholded
            .work()
            .segment_work()
            .iter()
            .filter(|segment| !segment.segment().is_empty())
            .all(|segment| segment.discarded_patterns() == segment.generated_patterns()));
    }

    #[test]
    fn discard_threshold_respects_the_candidate_cap() {
        let basis = basis();
        let thresholded = basis
            .reprocess_segmented_with_discard_threshold(
                &RowSpace,
                OsdConfig::new(2).with_candidate_cap(Some(1)),
                Some(0),
            )
            .unwrap();

        assert!(thresholded.best().is_none());
        assert_eq!(thresholded.work().generated_patterns(), 1);
        assert_eq!(thresholded.work().discarded_patterns(), 1);
        assert_eq!(thresholded.work().tested_candidates(), 0);
        assert_eq!(thresholded.work().eliminations(), 1);
        assert_eq!(
            thresholded.work().termination(),
            OsdTermination::CandidateCap
        );
        assert_eq!(thresholded.work().segment_work()[0].discarded_patterns(), 1);
        assert_eq!(thresholded.work().segment_work()[0].tested_candidates(), 0);
        assert_eq!(thresholded.work().segment_work()[1].discarded_patterns(), 0);
    }

    #[test]
    fn discard_threshold_cancellation_terminates_before_testing() {
        let basis = basis();
        let cancellation = AtomicBool::new(true);
        let thresholded = basis
            .reprocess_segmented_with_discard_threshold_and_cancellation(
                &RowSpace,
                OsdConfig::new(2),
                Some(0),
                &cancellation,
            )
            .unwrap();

        assert!(thresholded.best().is_none());
        assert_eq!(thresholded.work().generated_patterns(), 0);
        assert_eq!(thresholded.work().discarded_patterns(), 0);
        assert_eq!(thresholded.work().tested_candidates(), 0);
        assert_eq!(thresholded.work().eliminations(), 1);
        assert_eq!(thresholded.work().termination(), OsdTermination::Cancelled);
    }
}

/// Which end of the canonical reliability order leads the column preference.
///
/// The preference decides which columns the elimination pivots on, and
/// therefore which columns end up in the independent basis and which end up
/// free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnPreference {
    /// Pivot on the largest reliability magnitudes first, so the basis is the
    /// most reliable independent column set.
    MostReliableFirst,
    /// Pivot on the smallest reliability magnitudes first, so the basis is the
    /// least reliable independent column set and the free columns are the most
    /// reliable ones.
    LeastReliableFirst,
}

/// The column set an OSD run reprocesses.
///
/// Both sets are listed in the basis's column-preference order, so pattern
/// index zero always names the most preferred reprocessed column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReprocessedColumns {
    /// The independent basis columns selected by the elimination.
    Basis,
    /// The columns the elimination did not select.
    Free,
}

/// A most-reliable basis: a canonical reliability order, the ordered
/// elimination it drove, and the transformed right-hand side that goes with
/// it.
///
/// The elimination pivots on the columns in reliability-preference order, so
/// [`OrderedEliminationResult::selected_cols`] is the greedy independent
/// column set under that order and [`Self::free_cols`] holds the rest.  Rank
/// deficiency is an ordinary outcome: the basis is then smaller than the row
/// count and the free set correspondingly larger.
///
/// Building one basis costs a single ordered elimination; reprocessing reuses
/// it for every candidate.
#[derive(Clone, Debug)]
pub struct MostReliableBasis {
    elimination: OrderedEliminationResult,
    preference: ColumnPreference,
    preference_order: Vec<usize>,
    free_cols: Vec<usize>,
    transformed_rhs: Option<BitVec>,
    consistent: bool,
    reference: BitVec,
    magnitudes: Vec<f32>,
}

impl MostReliableBasis {
    /// Builds the basis for one reprocessing run.
    ///
    /// `magnitudes` and `reference` carry one entry per matrix column.  The
    /// canonical [`ReliabilityPermutation`] orders the columns, `preference`
    /// picks the direction, and the ordered elimination selects the greedy
    /// independent set under that order.  A `right_hand_side` is moved into the
    /// reduced coordinates by the elimination's row transform, and the
    /// resulting system is checked for consistency.
    ///
    /// `reference` is the word candidates are scored against: the soft metric
    /// of a candidate is the sum of the magnitudes of the coordinates where it
    /// disagrees with `reference`.
    ///
    /// # Arguments
    ///
    /// * `matrix` — the linear system whose columns are ordered
    /// * `magnitudes` — reliability magnitude of each column
    /// * `reference` — the word the soft metric measures candidates against
    /// * `right_hand_side` — one entry per matrix row, or `None` for a
    ///   homogeneous system
    /// * `preference` — which end of the reliability order leads
    ///
    /// # Errors
    ///
    /// Returns [`OsdEngineError::MagnitudeLength`] or
    /// [`OsdEngineError::ReferenceLength`] when those inputs do not carry one
    /// entry per matrix column, and [`OsdEngineError::Elimination`] when
    /// `right_hand_side` does not carry one entry per matrix row.
    ///
    /// # Panics
    ///
    /// Panics if any entry of `magnitudes` is NaN.  NaN has no numeric
    /// ordering, so it is not a valid reliability magnitude for the canonical
    /// permutation.
    ///
    /// # Complexity
    ///
    /// O(n log n) for `n` columns to order them, plus the ordered
    /// elimination's own cost.
    ///
    /// # Examples
    ///
    /// See [`reprocess`] for an end-to-end walkthrough.
    pub fn build(
        matrix: &BitMatrix,
        magnitudes: &[f32],
        reference: &BitVec,
        right_hand_side: Option<&BitVec>,
        preference: ColumnPreference,
    ) -> Result<Self, OsdEngineError> {
        let cols = matrix.cols();
        if magnitudes.len() != cols {
            return Err(OsdEngineError::MagnitudeLength {
                expected: cols,
                actual: magnitudes.len(),
            });
        }
        if reference.len() != cols {
            return Err(OsdEngineError::ReferenceLength {
                expected: cols,
                actual: reference.len(),
            });
        }

        let permutation = ReliabilityPermutation::from_magnitudes(magnitudes);
        let preference_order = match preference {
            ColumnPreference::MostReliableFirst => permutation.descending(),
            ColumnPreference::LeastReliableFirst => permutation.ascending(),
        }
        .to_vec();

        let elimination = ordered_column_elimination(matrix, &preference_order)?;
        let transformed_rhs = right_hand_side
            .map(|rhs| elimination.apply_transform(rhs))
            .transpose()?;
        let consistent = match &transformed_rhs {
            Some(rhs) => elimination.is_consistent(rhs)?,
            None => true,
        };

        let mut selected = vec![false; cols];
        for &col in &elimination.selected_cols {
            selected[col] = true;
        }
        let free_cols: Vec<usize> = preference_order
            .iter()
            .copied()
            .filter(|&col| !selected[col])
            .collect();

        Ok(Self {
            elimination,
            preference,
            preference_order,
            free_cols,
            transformed_rhs,
            consistent,
            reference: reference.clone(),
            magnitudes: magnitudes.to_vec(),
        })
    }

    /// Returns the ordered elimination that selected this basis.
    ///
    /// Its `reduced`, `selected_cols`, `rank`, and `transform` fields are the
    /// canonical description of the eliminated system.
    pub fn elimination(&self) -> &OrderedEliminationResult {
        &self.elimination
    }

    /// Returns which end of the reliability order led the column preference.
    pub const fn preference(&self) -> ColumnPreference {
        self.preference
    }

    /// Returns the complete column preference handed to the elimination.
    ///
    /// This is the canonical reliability permutation in the direction
    /// [`Self::preference`] names, so equal magnitudes keep ascending
    /// original-index order.
    pub fn preference_order(&self) -> &[usize] {
        &self.preference_order
    }

    /// Returns the columns the elimination did not select, in preference
    /// order.
    pub fn free_cols(&self) -> &[usize] {
        &self.free_cols
    }

    /// Returns the columns of the requested reprocessed set, in preference
    /// order.
    pub fn reprocessed_cols(&self, columns: ReprocessedColumns) -> &[usize] {
        match columns {
            ReprocessedColumns::Basis => &self.elimination.selected_cols,
            ReprocessedColumns::Free => &self.free_cols,
        }
    }

    /// Returns the right-hand side in the reduced coordinates, or `None` for a
    /// homogeneous system.
    pub fn transformed_rhs(&self) -> Option<&BitVec> {
        self.transformed_rhs.as_ref()
    }

    /// Reports whether the transformed system has a solution.
    ///
    /// A homogeneous system is always consistent.  Otherwise the transformed
    /// right-hand side must be zero on the rows the elimination reduced to
    /// zero; a set entry there demands `0 = 1`.
    pub const fn is_consistent(&self) -> bool {
        self.consistent
    }

    /// Returns the word candidates are scored against.
    pub fn reference(&self) -> &BitVec {
        &self.reference
    }

    /// Returns the reliability magnitude of each column.
    pub fn magnitudes(&self) -> &[f32] {
        &self.magnitudes
    }

    /// Reprocesses this basis with deterministic weight-segment accounting.
    ///
    /// This inherent entry point keeps the additive segmented API reachable
    /// through the existing public [`MostReliableBasis`] export while the
    /// baseline [`reprocess`] function retains its original result type.
    ///
    /// # Errors
    ///
    /// Returns [`OsdEngineError::Patterns`] when the checked uncapped
    /// order-m candidate bound cannot be represented by `usize`, or when the
    /// semantics returns a base candidate or delta vector with a width or
    /// count that does not match the basis.
    ///
    /// # Panics
    ///
    /// This method adds no panics beyond those possible while executing the
    /// caller-provided [`OsdSemantics`] implementation.
    pub fn reprocess_segmented<S>(
        &self,
        semantics: &S,
        config: OsdConfig,
    ) -> Result<OsdSegmentedOutcome, OsdEngineError>
    where
        S: OsdSemantics + ?Sized,
    {
        reprocess_segmented(self, semantics, config)
    }

    /// Reprocesses this basis with an optional implementation-policy discard
    /// threshold.
    ///
    /// The threshold is compared with the policy's generated-pattern metric
    /// once, at each deterministic weight-segment boundary.  A segment is
    /// discarded only when `metric > discard_threshold`; equality retains the
    /// segment.  `None` disables discarding and preserves exhaustive order-`m`
    /// results.  A thresholded result is a policy-bounded approximation, not
    /// an exhaustive order-`m` search.
    ///
    /// # Errors
    ///
    /// Returns [`OsdEngineError::Patterns`] when the checked uncapped
    /// order-m candidate bound cannot be represented by `usize`, or when the
    /// semantics returns a base candidate or delta vector with a width or
    /// count that does not match the basis.
    ///
    /// # Panics
    ///
    /// This method adds no panics beyond those possible while executing the
    /// caller-provided [`OsdSemantics`] implementation.
    ///
    /// # Complexity
    ///
    /// The pattern source still accounts for each generated pattern, while
    /// reconstruction, validation, and ranking run only for retained
    /// patterns: O(generated + retained × (weight + columns) / 64) word
    /// operations after the shared basis setup.
    pub fn reprocess_segmented_with_discard_threshold<S>(
        &self,
        semantics: &S,
        config: OsdConfig,
        discard_threshold: Option<usize>,
    ) -> Result<OsdSegmentedOutcome, OsdEngineError>
    where
        S: OsdSemantics + ?Sized,
    {
        reprocess_segmented_with_discard_threshold(self, semantics, config, discard_threshold, None)
    }

    /// Reprocesses this basis with segment accounting and caller cancellation.
    ///
    /// # Errors
    ///
    /// Returns [`OsdEngineError::Patterns`] when the checked uncapped
    /// order-m candidate bound cannot be represented by `usize`, or when the
    /// semantics returns a base candidate or delta vector with a width or
    /// count that does not match the basis.
    ///
    /// # Panics
    ///
    /// This method adds no panics beyond those possible while executing the
    /// caller-provided [`OsdSemantics`] implementation.
    pub fn reprocess_segmented_with_cancellation<S>(
        &self,
        semantics: &S,
        config: OsdConfig,
        cancellation: &AtomicBool,
    ) -> Result<OsdSegmentedOutcome, OsdEngineError>
    where
        S: OsdSemantics + ?Sized,
    {
        reprocess_segmented_with_cancellation(self, semantics, config, cancellation)
    }

    /// Reprocesses this basis with an optional discard threshold and caller
    /// cancellation.
    ///
    /// Threshold decisions use the implementation-produced generated-pattern
    /// metric at deterministic weight boundaries.  A segment is discarded
    /// only when `metric > discard_threshold`; equality retains it.  `None`
    /// disables discarding.  When a threshold is active, the returned best
    /// candidate is a policy-bounded approximation rather than the result of
    /// exhaustive order-`m` search.
    ///
    /// # Errors
    ///
    /// Returns [`OsdEngineError::Patterns`] when the checked uncapped
    /// order-m candidate bound cannot be represented by `usize`, or when the
    /// semantics returns a base candidate or delta vector with a width or
    /// count that does not match the basis.
    ///
    /// # Panics
    ///
    /// This method adds no panics beyond those possible while executing the
    /// caller-provided [`OsdSemantics`] implementation.
    ///
    /// # Complexity
    ///
    /// The pattern source accounts for each generated pattern, while
    /// reconstruction, validation, and ranking run only for retained
    /// patterns: O(generated + retained × (weight + columns) / 64) word
    /// operations after the shared basis setup.
    pub fn reprocess_segmented_with_discard_threshold_and_cancellation<S>(
        &self,
        semantics: &S,
        config: OsdConfig,
        discard_threshold: Option<usize>,
        cancellation: &AtomicBool,
    ) -> Result<OsdSegmentedOutcome, OsdEngineError>
    where
        S: OsdSemantics + ?Sized,
    {
        reprocess_segmented_with_discard_threshold(
            self,
            semantics,
            config,
            discard_threshold,
            Some(cancellation),
        )
    }
}

/// The semantics of one OSD candidate space.
///
/// The engine reconstructs candidates as an affine map over GF(2): the empty
/// pattern reconstructs [`Self::base_candidate`], and a pattern adds the
/// [`Self::position_deltas`] of the reprocessed columns it names.  A candidate
/// space carved out of a linear system is affine in exactly this sense —
/// re-encoding from an information set and back-substituting a free assignment
/// both are — so the engine needs no other reconstruction hook.
///
/// Every vector an adapter returns carries one entry per matrix column.
///
/// # Examples
///
/// See [`reprocess`] for an adapter implemented end to end.
pub trait OsdSemantics {
    /// Returns which column set the order-`m` patterns perturb.
    fn reprocessed_columns(&self) -> ReprocessedColumns;

    /// Returns the candidate the empty pattern reconstructs.
    fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec;

    /// Returns one candidate delta per reprocessed column, in the order of
    /// `basis.reprocessed_cols(self.reprocessed_columns())`.
    ///
    /// The delta at index `i` is added to a candidate exactly when a pattern
    /// names `i`.
    fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec>;

    /// Reports whether a reconstructed candidate is valid for this semantics.
    ///
    /// Every candidate reaching this rule counts as a tested candidate.  A
    /// rejected one never enters the ranking.
    fn accepts(&self, candidate: &BitVec) -> bool;
}

/// One reprocessed candidate and its rank in the soft order.
#[derive(Clone, Debug, PartialEq)]
pub struct OsdCandidate {
    word: BitVec,
    pattern: Vec<usize>,
    metric: f64,
    generation: usize,
}

impl OsdCandidate {
    /// Returns the reconstructed candidate word.
    pub fn word(&self) -> &BitVec {
        &self.word
    }

    /// Consumes the candidate and returns its word.
    pub fn into_word(self) -> BitVec {
        self.word
    }

    /// Returns the test pattern that reconstructed this candidate.
    ///
    /// Its entries index the reprocessed column list, not the matrix columns;
    /// `basis.reprocessed_cols(..)` maps them to columns.
    pub fn pattern(&self) -> &[usize] {
        &self.pattern
    }

    /// Returns the soft metric: the sum of the reliability magnitudes of the
    /// coordinates where this candidate disagrees with the basis reference
    /// word.
    pub const fn metric(&self) -> f64 {
        self.metric
    }

    /// Returns the zero-based position of this candidate in the generation
    /// order.
    pub const fn generation(&self) -> usize {
        self.generation
    }
}

/// Counters and termination metadata for one reprocessing run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OsdWork {
    rank: usize,
    theoretical_candidates: usize,
    generated_patterns: usize,
    tested_candidates: usize,
    eliminations: usize,
    termination: OsdTermination,
}

impl OsdWork {
    /// Returns the rank of the eliminated matrix.
    pub const fn rank(&self) -> usize {
        self.rank
    }

    /// Returns the checked uncapped candidate bound for the reprocessed
    /// dimension, `sum(binomial(dimension, weight))` through the configured
    /// order.
    pub const fn theoretical_candidates(&self) -> usize {
        self.theoretical_candidates
    }

    /// Returns the number of test patterns the run generated.
    pub const fn generated_patterns(&self) -> usize {
        self.generated_patterns
    }

    /// Returns the number of reconstructed candidates the semantics evaluated.
    ///
    /// Every candidate handed to [`OsdSemantics::accepts`] counts, whichever
    /// way that rule answered; a candidate the run stopped before evaluating
    /// does not.  The accepted subset is what the ranking sees, so a run whose
    /// semantics rejected everything reports tested candidates and no best
    /// candidate.
    pub const fn tested_candidates(&self) -> usize {
        self.tested_candidates
    }

    /// Returns the number of ordered eliminations behind this outcome.
    ///
    /// The baseline engine reprocesses one most-reliable basis, so this is
    /// always one.
    pub const fn eliminations(&self) -> usize {
        self.eliminations
    }

    /// Returns the reason the run stopped.
    pub const fn termination(&self) -> OsdTermination {
        self.termination
    }
}

/// The best candidate a reprocessing run found, with its work metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct OsdOutcome {
    best: Option<OsdCandidate>,
    work: OsdWork,
}

impl OsdOutcome {
    /// Returns the lowest-metric accepted candidate, or `None` when the run
    /// accepted none.
    pub fn best(&self) -> Option<&OsdCandidate> {
        self.best.as_ref()
    }

    /// Consumes the outcome and returns its best candidate.
    pub fn into_best(self) -> Option<OsdCandidate> {
        self.best
    }

    /// Returns the counters and termination reason of the run.
    pub const fn work(&self) -> &OsdWork {
        &self.work
    }
}

/// The metric a discard-threshold consumer compares at a segment boundary.
///
/// The segmentation producer currently exposes generated-pattern count.  It
/// is deliberately a named enum rather than an implicit field so a future
/// threshold policy cannot silently switch to tested-candidate count, which
/// has different semantics when a segment is discarded.
///
/// # Panics
///
/// Selecting a metric does not panic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OsdComplexityMetric {
    /// Compare the cap-bounded number of patterns in the segment.
    GeneratedPatterns,
}

/// Implementation-produced complexity policy for segmented OSD search.
///
/// The policy has one weight-ordered descriptor per possible pattern weight.
/// Its [`Self::segment_metric`] values are the cap-bounded generated-pattern
/// counts a threshold evaluator compares before deciding whether to retain a
/// segment.  An optional discard threshold uses the rule
/// `segment_metric > discard_threshold`; equality retains the segment.  With
/// no threshold, the policy is descriptive and segmented execution remains
/// exhaustive.  With a threshold, results are policy-bounded approximations,
/// not exhaustive order-`m` search results.
///
/// # Panics
///
/// Constructing or copying a policy does not panic for a valid checked bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OsdComplexityPolicy {
    segmentation: PatternSegmentation,
    metric: OsdComplexityMetric,
    discard_threshold: Option<usize>,
}

impl OsdComplexityPolicy {
    /// Builds the implementation-level policy for one reprocessed dimension.
    ///
    /// # Errors
    ///
    /// Returns [`OsdEngineError::Patterns`] when the checked uncapped
    /// order-m candidate bound cannot be represented by `usize`.
    ///
    /// # Panics
    ///
    /// This constructor never panics for any dimension or [`OsdConfig`].
    pub fn new(dimension: usize, config: OsdConfig) -> Result<Self, OsdEngineError> {
        Ok(Self {
            segmentation: PatternSegmentation::new(dimension, config)?,
            metric: OsdComplexityMetric::GeneratedPatterns,
            discard_threshold: None,
        })
    }

    /// Returns a copy of this policy with an optional segment discard
    /// threshold.
    ///
    /// The threshold compares only [`Self::segment_metric`] at a deterministic
    /// segment boundary.  A segment is discarded when the metric is strictly
    /// greater than the threshold; equality retains it.  `None` disables
    /// discarding.
    ///
    /// # Panics
    ///
    /// This builder never panics.
    pub const fn with_discard_threshold(mut self, threshold: Option<usize>) -> Self {
        self.discard_threshold = threshold;
        self
    }

    /// Returns the configured segment discard threshold, if any.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn discard_threshold(&self) -> Option<usize> {
        self.discard_threshold
    }

    /// Returns the metric used at every segment boundary.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn metric(&self) -> OsdComplexityMetric {
        self.metric
    }

    /// Returns all deterministic weight descriptors, including empty capped
    /// tail segments.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub fn segments(&self) -> &[PatternSegment] {
        self.segmentation.segments()
    }

    /// Returns the descriptor at `index`, if present.
    ///
    /// # Panics
    ///
    /// This accessor never panics for any index.
    pub fn segment(&self, index: usize) -> Option<&PatternSegment> {
        self.segmentation.segment(index)
    }

    /// Returns the threshold-comparison value for `index`, if present.
    ///
    /// The value is the generated-pattern count in the descriptor's
    /// cap-bounded range.  It is available before any candidate is tested.
    ///
    /// # Panics
    ///
    /// This accessor never panics for any index.
    pub fn segment_metric(&self, index: usize) -> Option<usize> {
        self.segment(index)
            .map(|segment| self.metric_value(segment))
    }

    /// Returns the checked uncapped candidate bound.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn theoretical_candidates(&self) -> usize {
        self.segmentation.theoretical_candidates()
    }

    /// Returns the candidate cap represented by the policy.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn candidate_cap(&self) -> Option<usize> {
        self.segmentation.candidate_cap()
    }

    fn metric_value(&self, segment: &PatternSegment) -> usize {
        match self.metric {
            OsdComplexityMetric::GeneratedPatterns => segment.generated_pattern_count(),
        }
    }

    fn discards(&self, segment: &PatternSegment) -> bool {
        self.discard_threshold
            .is_some_and(|threshold| self.metric_value(segment) > threshold)
    }
}

/// Per-segment counters emitted by a segmented OSD run.
///
/// # Panics
///
/// Constructing or copying segment work does not panic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OsdSegmentWork {
    segment: PatternSegment,
    generated_patterns: usize,
    tested_candidates: usize,
    discarded_patterns: usize,
}

impl OsdSegmentWork {
    /// Returns the deterministic descriptor for this segment.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn segment(&self) -> PatternSegment {
        self.segment
    }

    /// Returns the generated-pattern count observed in this segment.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn generated_patterns(&self) -> usize {
        self.generated_patterns
    }

    /// Returns the number of adapter-evaluated candidates in this segment.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn tested_candidates(&self) -> usize {
        self.tested_candidates
    }

    /// Returns the number of generated patterns discarded by the active
    /// segment policy before candidate reconstruction.
    ///
    /// A discarded pattern is never counted by [`Self::tested_candidates`].
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn discarded_patterns(&self) -> usize {
        self.discarded_patterns
    }
}

/// Work metadata for a segmented OSD run.
///
/// Aggregate counters retain the baseline meanings: generated patterns are
/// enumerator output and tested candidates are adapter evaluations.  The
/// segment reports add deterministic weight-local counters without changing
/// the baseline [`OsdWork`] representation.  A discarded pattern contributes
/// to generated and discarded counts, but never to tested candidates.
/// Thresholded outcomes are policy-bounded approximations rather than
/// exhaustive order-`m` searches.
///
/// # Panics
///
/// Constructing or copying work metadata does not panic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OsdSegmentedWork {
    rank: usize,
    policy: OsdComplexityPolicy,
    segments: usize,
    segment_work: Vec<OsdSegmentWork>,
    theoretical_candidates: usize,
    generated_patterns: usize,
    tested_candidates: usize,
    discarded_patterns: usize,
    eliminations: usize,
    termination: OsdTermination,
}

impl OsdSegmentedWork {
    /// Returns the rank of the eliminated matrix.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn rank(&self) -> usize {
        self.rank
    }

    /// Returns the implementation-produced threshold policy.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn policy(&self) -> &OsdComplexityPolicy {
        &self.policy
    }

    /// Returns the number of nonempty segments visited by the run.
    ///
    /// Empty capped tail descriptors remain available through
    /// [`Self::segment_work`] but do not contribute to this counter.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn segments(&self) -> usize {
        self.segments
    }

    /// Returns one counter record for every policy segment, including empty
    /// capped tails.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub fn segment_work(&self) -> &[OsdSegmentWork] {
        &self.segment_work
    }

    /// Returns the checked uncapped candidate bound.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn theoretical_candidates(&self) -> usize {
        self.theoretical_candidates
    }

    /// Returns the number of generated patterns.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn generated_patterns(&self) -> usize {
        self.generated_patterns
    }

    /// Returns the number of adapter-evaluated candidates.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn tested_candidates(&self) -> usize {
        self.tested_candidates
    }

    /// Returns the number of generated patterns discarded before candidate
    /// reconstruction.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn discarded_patterns(&self) -> usize {
        self.discarded_patterns
    }

    /// Returns the number of ordered eliminations behind the outcome.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn eliminations(&self) -> usize {
        self.eliminations
    }

    /// Returns the reason the segmented run stopped.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn termination(&self) -> OsdTermination {
        self.termination
    }

    fn baseline(&self) -> OsdWork {
        OsdWork {
            rank: self.rank,
            theoretical_candidates: self.theoretical_candidates,
            generated_patterns: self.generated_patterns,
            tested_candidates: self.tested_candidates,
            eliminations: self.eliminations,
            termination: self.termination,
        }
    }
}

/// Best candidate and complexity metadata from segmented OSD search.
///
/// When [`OsdSegmentedWork::policy`] has a discard threshold, the candidate is
/// a policy-bounded approximation rather than the result of exhaustive
/// order-`m` search.  With no threshold, the segmented result preserves the
/// exhaustive baseline candidate order and ranking.
///
/// # Panics
///
/// Constructing or copying an outcome does not panic.
#[derive(Clone, Debug, PartialEq)]
pub struct OsdSegmentedOutcome {
    best: Option<OsdCandidate>,
    work: OsdSegmentedWork,
}

impl OsdSegmentedOutcome {
    /// Returns the lowest-metric accepted candidate, if one was tested.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub fn best(&self) -> Option<&OsdCandidate> {
        self.best.as_ref()
    }

    /// Returns the segmented work metadata.
    ///
    /// # Panics
    ///
    /// This accessor never panics.
    pub const fn work(&self) -> &OsdSegmentedWork {
        &self.work
    }

    /// Projects the segmented result onto the unchanged baseline outcome.
    ///
    /// With no active discard threshold, this projection is equal to the
    /// result of [`reprocess`] because segmentation only observes the existing
    /// pattern stream.
    ///
    /// # Panics
    ///
    /// This method never panics.
    pub fn outcome(&self) -> OsdOutcome {
        OsdOutcome {
            best: self.best.clone(),
            work: self.work.baseline(),
        }
    }

    /// Consumes the segmented result and returns its baseline projection.
    ///
    /// # Panics
    ///
    /// This method never panics.
    pub fn into_outcome(self) -> OsdOutcome {
        OsdOutcome {
            best: self.best,
            work: self.work.baseline(),
        }
    }
}

/// A failure to build or reprocess a most-reliable basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OsdEngineError {
    /// The magnitude slice does not carry one entry per matrix column.
    MagnitudeLength {
        /// Number of columns in the matrix.
        expected: usize,
        /// Number of supplied magnitudes.
        actual: usize,
    },

    /// The reference word does not carry one entry per matrix column.
    ReferenceLength {
        /// Number of columns in the matrix.
        expected: usize,
        /// Length of the supplied reference word.
        actual: usize,
    },

    /// The semantics returned a base candidate of the wrong width.
    BaseCandidateLength {
        /// Number of columns in the matrix.
        expected: usize,
        /// Length of the returned base candidate.
        actual: usize,
    },

    /// The semantics returned a number of deltas that does not match the
    /// reprocessed dimension.
    DeltaCount {
        /// Number of reprocessed columns.
        expected: usize,
        /// Number of returned deltas.
        actual: usize,
    },

    /// The semantics returned a delta of the wrong width.
    DeltaLength {
        /// Index of the offending delta within the reprocessed column list.
        position: usize,
        /// Number of columns in the matrix.
        expected: usize,
        /// Length of the returned delta.
        actual: usize,
    },

    /// The ordered elimination or its right-hand-side transform rejected the
    /// input.
    Elimination(OrderedEliminationError),

    /// The pattern source rejected the configured order.
    Patterns(PatternEnumerationError),
}

impl fmt::Display for OsdEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MagnitudeLength { expected, actual } => write!(
                formatter,
                "reliability magnitudes have {actual} entries, expected one per column ({expected})"
            ),
            Self::ReferenceLength { expected, actual } => write!(
                formatter,
                "reference word has {actual} entries, expected one per column ({expected})"
            ),
            Self::BaseCandidateLength { expected, actual } => write!(
                formatter,
                "base candidate has {actual} entries, expected one per column ({expected})"
            ),
            Self::DeltaCount { expected, actual } => write!(
                formatter,
                "semantics returned {actual} candidate deltas, expected one per \
                 reprocessed column ({expected})"
            ),
            Self::DeltaLength {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "candidate delta {position} has {actual} entries, expected one per \
                 column ({expected})"
            ),
            Self::Elimination(error) => write!(formatter, "ordered elimination failed: {error}"),
            Self::Patterns(error) => write!(formatter, "pattern enumeration failed: {error}"),
        }
    }
}

impl std::error::Error for OsdEngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Elimination(error) => Some(error),
            Self::Patterns(error) => Some(error),
            _ => None,
        }
    }
}

impl From<OrderedEliminationError> for OsdEngineError {
    fn from(error: OrderedEliminationError) -> Self {
        Self::Elimination(error)
    }
}

impl From<PatternEnumerationError> for OsdEngineError {
    fn from(error: PatternEnumerationError) -> Self {
        Self::Patterns(error)
    }
}

/// Reprocesses order-`m` test patterns over a most-reliable basis and returns
/// the best candidate the semantics accepted.
///
/// Patterns come from the bounded [`PatternEnumerator`], so they arrive in
/// increasing Hamming weight and, within one weight, in lexicographic order of
/// their reprocessed-column indices.  Each pattern reconstructs a candidate,
/// the semantics accepts or rejects it, and an accepted candidate is ranked by
/// its soft metric: the sum of the reliability magnitudes of the coordinates
/// where it disagrees with the basis reference word, accumulated in ascending
/// column order.  A candidate replaces the standing best only when its metric
/// is strictly smaller, so equal metrics keep the earlier candidate in
/// generation order.  Because the reprocessed columns are listed in the
/// canonical preference order, which breaks equal magnitudes by original
/// index, the whole ranking is a deterministic function of the inputs.
///
/// The work metadata keeps those stages apart:
/// [`OsdWork::generated_patterns`] counts what the enumerator emitted,
/// [`OsdWork::tested_candidates`] counts what the semantics evaluated, and the
/// returned candidate is the ranking over the accepted subset.
///
/// An inconsistent transformed right-hand side has no solution, so the run
/// generates nothing and reports [`OsdTermination::InconsistentTransform`].
///
/// # Errors
///
/// Returns [`OsdEngineError::Patterns`] when the uncapped candidate bound for
/// the reprocessed dimension is not representable, and
/// [`OsdEngineError::BaseCandidateLength`], [`OsdEngineError::DeltaCount`], or
/// [`OsdEngineError::DeltaLength`] when the semantics returns vectors that do
/// not match the basis.
///
/// # Complexity
///
/// O(candidates × (weight + columns) / 64) word operations, plus one
/// reconstruction setup of O(dimension × columns / 64).  The bound is checked
/// before any candidate is reconstructed.
///
/// # Examples
///
/// A minimal basis-reprocessing adapter: candidates are the row space of the
/// eliminated matrix, and the patterns perturb its independent basis columns.
///
/// ```
/// use gf2_coding::osd::{
///     reprocess, ColumnPreference, MostReliableBasis, OsdConfig, OsdSemantics,
///     OsdTermination, ReprocessedColumns,
/// };
/// use gf2_core::{BitMatrix, BitVec};
///
/// struct RowSpace;
///
/// impl OsdSemantics for RowSpace {
///     fn reprocessed_columns(&self) -> ReprocessedColumns {
///         ReprocessedColumns::Basis
///     }
///
///     fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
///         // Re-encode the reference word from the basis coordinates.
///         let elimination = basis.elimination();
///         let mut message = BitVec::zeros(elimination.reduced.rows());
///         for (row, &col) in elimination.selected_cols.iter().enumerate() {
///             message.set(row, basis.reference().get(col));
///         }
///         elimination.reduced.matvec_transpose(&message)
///     }
///
///     fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
///         // Flipping basis coordinate `row` adds that reduced row.
///         (0..basis.elimination().rank)
///             .map(|row| basis.elimination().reduced.row_as_bitvec(row))
///             .collect()
///     }
///
///     fn accepts(&self, _candidate: &BitVec) -> bool {
///         true
///     }
/// }
///
/// // Rows of a 2 x 4 generator: [1 0 1 1] and [0 1 1 0].
/// let mut generator = BitMatrix::zeros(2, 4);
/// for (row, col) in [(0, 0), (0, 2), (0, 3), (1, 1), (1, 2)] {
///     generator.set(row, col, true);
/// }
///
/// // Column 2 is the most reliable coordinate and column 1 the least.
/// let magnitudes = [3.0, 1.0, 4.0, 2.0];
/// let received = BitVec::ones(4);
///
/// let basis = MostReliableBasis::build(
///     &generator,
///     &magnitudes,
///     &received,
///     None,
///     ColumnPreference::MostReliableFirst,
/// )
/// .unwrap();
/// assert_eq!(basis.reprocessed_cols(ReprocessedColumns::Basis), &[2, 0]);
///
/// let outcome = reprocess(&basis, &RowSpace, OsdConfig::new(1)).unwrap();
/// let best = outcome.best().unwrap();
///
/// // The order-zero candidate already agrees with the received word except on
/// // the least reliable coordinate, and no order-one flip is cheaper.
/// assert_eq!(best.pattern(), &[] as &[usize]);
/// assert_eq!(best.metric(), 1.0);
///
/// let work = outcome.work();
/// assert_eq!(work.rank(), 2);
/// assert_eq!(work.generated_patterns(), 3);
/// assert_eq!(work.termination(), OsdTermination::Exhaustive);
/// ```
pub fn reprocess<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
) -> Result<OsdOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    reprocess_inner(basis, semantics, config, None)
}

/// Reprocesses like [`reprocess`], observing a caller-owned cancellation flag
/// before each candidate.
///
/// A run that observes the flag set stops without reconstructing another
/// candidate and reports [`OsdTermination::Cancelled`], which takes precedence
/// over a candidate cap reached by the same candidate.  The best candidate
/// found before the stop is returned.
///
/// # Errors
///
/// Identical to [`reprocess`].
///
/// # Examples
///
/// See [`reprocess`]; this entry point differs only by the flag.
pub fn reprocess_with_cancellation<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
    cancellation: &AtomicBool,
) -> Result<OsdOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    reprocess_inner(basis, semantics, config, Some(cancellation))
}

/// Reprocesses the baseline pattern stream with deterministic weight-segment
/// accounting.
///
/// Segmentation observes the same generated patterns, reconstructs candidates
/// through the same adapter, and ranks them in the same order as
/// [`reprocess`].  The returned policy exposes the cap-bounded generated
/// pattern count at every weight boundary for a later discard-threshold
/// consumer; this function does not discard any segment.
///
/// # Errors
///
/// Returns the same errors as [`reprocess`].
///
/// # Panics
///
/// This function adds no panics beyond those possible while executing the
/// caller-provided [`OsdSemantics`] implementation.
pub fn reprocess_segmented<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
) -> Result<OsdSegmentedOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    reprocess_segmented_inner(basis, semantics, config, None, None)
}

/// Reprocesses with deterministic segment accounting and observes a caller-
/// owned cancellation flag before every candidate.
///
/// Cancellation stops before reconstructing the next candidate and takes
/// precedence over a candidate cap reached by the preceding candidate.
///
/// # Errors
///
/// Returns the same errors as [`reprocess`].
///
/// # Panics
///
/// This function adds no panics beyond those possible while executing the
/// caller-provided [`OsdSemantics`] implementation.
pub fn reprocess_segmented_with_cancellation<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
    cancellation: &AtomicBool,
) -> Result<OsdSegmentedOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    reprocess_segmented_inner(basis, semantics, config, Some(cancellation), None)
}

struct ReprocessRun {
    best: Option<OsdCandidate>,
    work: OsdWork,
    policy: Option<OsdComplexityPolicy>,
    segment_work: Vec<OsdSegmentWork>,
    segments: usize,
    discarded_patterns: usize,
}

fn reprocess_inner<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
    cancellation: Option<&AtomicBool>,
) -> Result<OsdOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    let run = execute_reprocess(basis, semantics, config, cancellation, false, None)?;
    Ok(OsdOutcome {
        best: run.best,
        work: run.work,
    })
}

fn reprocess_segmented_inner<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
    cancellation: Option<&AtomicBool>,
    discard_threshold: Option<usize>,
) -> Result<OsdSegmentedOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    let run = execute_reprocess(
        basis,
        semantics,
        config,
        cancellation,
        true,
        discard_threshold,
    )?;
    Ok(OsdSegmentedOutcome {
        best: run.best,
        work: OsdSegmentedWork {
            rank: run.work.rank,
            policy: run
                .policy
                .expect("segmented execution always creates a complexity policy"),
            segments: run.segments,
            segment_work: run.segment_work,
            theoretical_candidates: run.work.theoretical_candidates,
            generated_patterns: run.work.generated_patterns,
            tested_candidates: run.work.tested_candidates,
            discarded_patterns: run.discarded_patterns,
            eliminations: run.work.eliminations,
            termination: run.work.termination,
        },
    })
}

fn reprocess_segmented_with_discard_threshold<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
    discard_threshold: Option<usize>,
    cancellation: Option<&AtomicBool>,
) -> Result<OsdSegmentedOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    reprocess_segmented_inner(basis, semantics, config, cancellation, discard_threshold)
}

fn execute_reprocess<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
    cancellation: Option<&AtomicBool>,
    segmented: bool,
    discard_threshold: Option<usize>,
) -> Result<ReprocessRun, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    let dimension = basis
        .reprocessed_cols(semantics.reprocessed_columns())
        .len();
    let mut enumerator = PatternEnumerator::new(dimension, config)?;
    let rank = basis.elimination.rank;
    let policy = segmented.then(|| {
        OsdComplexityPolicy::from_segmentation(enumerator.segmentation().clone())
            .with_discard_threshold(discard_threshold)
    });

    if !basis.consistent {
        let segment_work = policy.as_ref().map_or_else(Vec::new, empty_segment_work);
        return Ok(ReprocessRun {
            best: None,
            work: OsdWork {
                rank,
                theoretical_candidates: enumerator.theoretical_candidates(),
                generated_patterns: 0,
                tested_candidates: 0,
                eliminations: 1,
                termination: OsdTermination::InconsistentTransform,
            },
            policy,
            segment_work,
            segments: 0,
            discarded_patterns: 0,
        });
    }

    let width = basis.reference.len();
    let base = semantics.base_candidate(basis);
    if base.len() != width {
        return Err(OsdEngineError::BaseCandidateLength {
            expected: width,
            actual: base.len(),
        });
    }
    let deltas = semantics.position_deltas(basis);
    if deltas.len() != dimension {
        return Err(OsdEngineError::DeltaCount {
            expected: dimension,
            actual: deltas.len(),
        });
    }
    for (position, delta) in deltas.iter().enumerate() {
        if delta.len() != width {
            return Err(OsdEngineError::DeltaLength {
                position,
                expected: width,
                actual: delta.len(),
            });
        }
    }

    let mut best: Option<OsdCandidate> = None;
    let mut tested_candidates = 0;
    let mut generation = 0;
    let mut discarded_patterns = 0;
    let mut segment_tested_candidates = policy
        .as_ref()
        .map(|policy| vec![0usize; policy.segments().len()]);
    let mut segment_discarded_patterns = policy
        .as_ref()
        .map(|policy| vec![0usize; policy.segments().len()]);
    let mut active_segment_weight = None;
    let mut active_segment_index = None;
    let mut discard_active_segment = false;

    let mut visit = |segment: Option<&PatternSegment>, pattern: &[usize]| {
        let current = generation;
        generation += 1;

        if let Some(segment) = segment {
            let segment_index = if active_segment_weight == Some(segment.weight()) {
                active_segment_index.expect("an active segment always has an index")
            } else {
                let index = policy
                    .as_ref()
                    .and_then(|policy| {
                        policy
                            .segments()
                            .iter()
                            .position(|candidate| candidate.weight() == segment.weight())
                    })
                    .expect("a generated segment belongs to the complexity policy");
                active_segment_weight = Some(segment.weight());
                active_segment_index = Some(index);
                discard_active_segment = policy
                    .as_ref()
                    .is_some_and(|policy| policy.discards(segment));
                index
            };

            if discard_active_segment {
                discarded_patterns += 1;
                segment_discarded_patterns
                    .as_mut()
                    .expect("segmented execution has segment counters")[segment_index] += 1;
                return PatternControl::Continue;
            }

            segment_tested_candidates
                .as_mut()
                .expect("segmented execution has segment counters")[segment_index] += 1;
        }

        let mut candidate = base.clone();
        for &position in pattern {
            candidate.bit_xor_into(&deltas[position]);
        }

        tested_candidates += 1;

        if semantics.accepts(&candidate) {
            let metric = soft_metric(&candidate, &basis.reference, &basis.magnitudes);
            if best
                .as_ref()
                .is_none_or(|standing| metric < standing.metric)
            {
                best = Some(OsdCandidate {
                    word: candidate,
                    pattern: pattern.to_vec(),
                    metric,
                    generation: current,
                });
            }
        }

        PatternControl::Continue
    };

    let (generated_patterns, segment_reports, segments, termination) = if segmented {
        let report = match cancellation {
            Some(flag) => enumerator.run_segmented_with_cancellation(flag, |segment, pattern| {
                visit(Some(segment), pattern)
            }),
            None => enumerator.run_segmented(|segment, pattern| visit(Some(segment), pattern)),
        };
        let segments = report.segments();
        let termination = report.termination();
        (report.generated(), Some(report), segments, termination)
    } else {
        let report = match cancellation {
            Some(flag) => {
                enumerator.enumerate_with_cancellation(flag, |pattern| visit(None, pattern))
            }
            None => enumerator.run(|pattern| visit(None, pattern)),
        };
        (report.generated(), None, 0, report.termination())
    };

    let segment_work = segment_reports.as_ref().map_or_else(Vec::new, |report| {
        report
            .segment_reports()
            .iter()
            .enumerate()
            .map(|segment| OsdSegmentWork {
                segment: segment.1.segment(),
                generated_patterns: segment.1.generated(),
                tested_candidates: segment_tested_candidates
                    .as_ref()
                    .expect("segmented execution has segment counters")[segment.0],
                discarded_patterns: segment_discarded_patterns
                    .as_ref()
                    .expect("segmented execution has segment counters")[segment.0],
            })
            .collect()
    });

    Ok(ReprocessRun {
        best,
        work: OsdWork {
            rank,
            theoretical_candidates: enumerator.theoretical_candidates(),
            generated_patterns,
            tested_candidates,
            eliminations: 1,
            termination,
        },
        policy,
        segment_work,
        segments,
        discarded_patterns,
    })
}

impl OsdComplexityPolicy {
    fn from_segmentation(segmentation: PatternSegmentation) -> Self {
        Self {
            segmentation,
            metric: OsdComplexityMetric::GeneratedPatterns,
            discard_threshold: None,
        }
    }
}

fn empty_segment_work(policy: &OsdComplexityPolicy) -> Vec<OsdSegmentWork> {
    policy
        .segments()
        .iter()
        .copied()
        .map(|segment| OsdSegmentWork {
            segment,
            generated_patterns: 0,
            tested_candidates: 0,
            discarded_patterns: 0,
        })
        .collect()
}

/// Sums the reliability magnitudes of the coordinates where `candidate` and
/// `reference` disagree.
///
/// The accumulation runs in ascending column order and widens each `f32`
/// magnitude to `f64`, so the same inputs always produce the same sum.
fn soft_metric(candidate: &BitVec, reference: &BitVec, magnitudes: &[f32]) -> f64 {
    let mut total = 0.0;
    for (column, &magnitude) in magnitudes.iter().enumerate() {
        if candidate.get(column) != reference.get(column) {
            total += f64::from(magnitude);
        }
    }
    total
}
