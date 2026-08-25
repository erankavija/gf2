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

use super::{
    OsdConfig, OsdTermination, PatternControl, PatternEnumerationError, PatternEnumerator,
};

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
}

/// The semantics of one OSD candidate space.
///
/// The engine reconstructs candidates as an affine map over GF(2): the empty
/// pattern reconstructs [`Self::base_candidate`], and a pattern adds the
/// [`Self::position_deltas`] of the reprocessed columns it names.  Both
/// downstream shapes are affine in exactly this sense — a generator-codeword
/// adapter adds reduced generator rows to a re-encoded word, and a
/// syndrome-error adapter adds free-column solution deltas to the pivot
/// back-substitution — so the engine needs no other reconstruction hook.
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
    /// Rejected candidates are counted as generated patterns but never enter
    /// the ranking.
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

    /// Returns the number of reconstructed candidates the semantics accepted,
    /// which is the number that entered the ranking.
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

fn reprocess_inner<S>(
    basis: &MostReliableBasis,
    semantics: &S,
    config: OsdConfig,
    cancellation: Option<&AtomicBool>,
) -> Result<OsdOutcome, OsdEngineError>
where
    S: OsdSemantics + ?Sized,
{
    let dimension = basis
        .reprocessed_cols(semantics.reprocessed_columns())
        .len();
    let mut enumerator = PatternEnumerator::new(dimension, config)?;
    let rank = basis.elimination.rank;

    if !basis.consistent {
        return Ok(OsdOutcome {
            best: None,
            work: OsdWork {
                rank,
                theoretical_candidates: enumerator.theoretical_candidates(),
                generated_patterns: 0,
                tested_candidates: 0,
                eliminations: 1,
                termination: OsdTermination::InconsistentTransform,
            },
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

    let visit = |pattern: &[usize]| {
        let mut candidate = base.clone();
        for &position in pattern {
            candidate.bit_xor_into(&deltas[position]);
        }

        let current = generation;
        generation += 1;

        if semantics.accepts(&candidate) {
            tested_candidates += 1;
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

    let report = match cancellation {
        Some(flag) => enumerator.enumerate_with_cancellation(flag, visit),
        None => enumerator.run(visit),
    };

    Ok(OsdOutcome {
        best,
        work: OsdWork {
            rank,
            theoretical_candidates: report.theoretical_candidates(),
            generated_patterns: report.generated(),
            tested_candidates,
            eliminations: 1,
            termination: report.termination(),
        },
    })
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
