//! Contract tests for the shared OSD most-reliable-basis reprocessing engine.
//!
//! The engine is exercised through two abstract semantic adapters that stand
//! in for the downstream generator-codeword and syndrome-error shapes without
//! borrowing their semantics: a basis-reprocessing adapter whose candidates
//! are the row space of the eliminated matrix, and a free-column adapter whose
//! candidates solve the transformed system.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use gf2_coding::osd::{
    checked_candidate_bound, reprocess, reprocess_with_cancellation, ColumnPreference,
    MostReliableBasis, OsdConfig, OsdEngineError, OsdSemantics, OsdTermination, ReprocessedColumns,
};
use gf2_core::alg::rref::OrderedEliminationError;
use gf2_core::{BitMatrix, BitVec};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Fixtures and independent reference implementations
// ---------------------------------------------------------------------------

fn matrix_from_rows(rows: &[&str]) -> BitMatrix {
    let cols = rows[0].len();
    let mut matrix = BitMatrix::zeros(rows.len(), cols);
    for (row, bits) in rows.iter().enumerate() {
        assert_eq!(bits.len(), cols, "every fixture row has the same width");
        for (col, bit) in bits.chars().enumerate() {
            matrix.set(row, col, bit == '1');
        }
    }
    matrix
}

fn word(bits: &str) -> BitVec {
    let mut vector = BitVec::zeros(bits.len());
    for (index, bit) in bits.chars().enumerate() {
        vector.set(index, bit == '1');
    }
    vector
}

/// Sums the magnitudes of the coordinates where `candidate` and `reference`
/// disagree, without consulting the engine.
fn naive_metric(candidate: &BitVec, reference: &BitVec, magnitudes: &[f32]) -> f64 {
    let mut total = 0.0;
    for (column, &magnitude) in magnitudes.iter().enumerate() {
        if candidate.get(column) != reference.get(column) {
            total += f64::from(magnitude);
        }
    }
    total
}

/// Computes `message * matrix` with a naive double loop.
fn naive_row_combination(matrix: &BitMatrix, message: &[bool]) -> BitVec {
    let mut product = BitVec::zeros(matrix.cols());
    for col in 0..matrix.cols() {
        let mut bit = false;
        for (row, &selected) in message.iter().enumerate() {
            bit ^= selected && matrix.get(row, col);
        }
        product.set(col, bit);
    }
    product
}

fn bits_of(mask: u32, len: usize) -> Vec<bool> {
    (0..len).map(|index| (mask >> index) & 1 == 1).collect()
}

/// Exhaustive bounded search over the row space of `matrix`: the candidate set
/// is every row-space word within Hamming distance `order` of `reference` on
/// the basis columns.
fn exhaustive_row_space_best(
    matrix: &BitMatrix,
    basis_cols: &[usize],
    reference: &BitVec,
    magnitudes: &[f32],
    order: usize,
) -> Option<(f64, BitVec)> {
    let rows = matrix.rows();
    let mut best: Option<(f64, BitVec)> = None;

    for mask in 0..(1u32 << rows) {
        let candidate = naive_row_combination(matrix, &bits_of(mask, rows));
        let distance = basis_cols
            .iter()
            .filter(|&&col| candidate.get(col) != reference.get(col))
            .count();
        if distance > order {
            continue;
        }

        let metric = naive_metric(&candidate, reference, magnitudes);
        if best.as_ref().is_none_or(|(current, _)| metric < *current) {
            best = Some((metric, candidate));
        }
    }

    best
}

/// Exhaustive bounded search over the solutions of `matrix * x = rhs` whose
/// free-column weight is at most `order`.
fn exhaustive_solution_best(
    matrix: &BitMatrix,
    rhs: &BitVec,
    free_cols: &[usize],
    reference: &BitVec,
    magnitudes: &[f32],
    order: usize,
) -> Option<(f64, BitVec)> {
    let cols = matrix.cols();
    let mut best: Option<(f64, BitVec)> = None;

    for mask in 0..(1u32 << cols) {
        let mut candidate = BitVec::zeros(cols);
        for (col, bit) in bits_of(mask, cols).into_iter().enumerate() {
            candidate.set(col, bit);
        }
        if matrix.matvec(&candidate) != *rhs {
            continue;
        }
        let weight = free_cols
            .iter()
            .filter(|&&col| candidate.get(col))
            .count();
        if weight > order {
            continue;
        }

        let metric = naive_metric(&candidate, reference, magnitudes);
        if best.as_ref().is_none_or(|(current, _)| metric < *current) {
            best = Some((metric, candidate));
        }
    }

    best
}

// ---------------------------------------------------------------------------
// Abstract semantic adapters
// ---------------------------------------------------------------------------

/// Basis-reprocessing shape: candidates are row-space words and the patterns
/// perturb the independent basis columns.
struct RowSpaceSemantics;

impl OsdSemantics for RowSpaceSemantics {
    fn reprocessed_columns(&self) -> ReprocessedColumns {
        ReprocessedColumns::Basis
    }

    fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
        let elimination = basis.elimination();
        let mut message = BitVec::zeros(elimination.reduced.rows());
        for (row, &col) in elimination.selected_cols.iter().enumerate() {
            message.set(row, basis.reference().get(col));
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

/// Free-column shape: candidates solve the transformed system and the patterns
/// perturb the free columns.
struct SolutionSemantics {
    matrix: BitMatrix,
    rhs: BitVec,
}

impl OsdSemantics for SolutionSemantics {
    fn reprocessed_columns(&self) -> ReprocessedColumns {
        ReprocessedColumns::Free
    }

    fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
        let elimination = basis.elimination();
        let transformed = basis
            .transformed_rhs()
            .expect("the free-column shape supplies a right-hand side");
        let mut solution = BitVec::zeros(elimination.reduced.cols());
        for (row, &col) in elimination.selected_cols.iter().enumerate() {
            solution.set(col, transformed.get(row));
        }
        solution
    }

    fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
        let elimination = basis.elimination();
        basis
            .free_cols()
            .iter()
            .map(|&free| {
                let mut delta = BitVec::zeros(elimination.reduced.cols());
                delta.set(free, true);
                for (row, &col) in elimination.selected_cols.iter().enumerate() {
                    if elimination.reduced.get(row, free) {
                        delta.set(col, true);
                    }
                }
                delta
            })
            .collect()
    }

    fn accepts(&self, candidate: &BitVec) -> bool {
        self.matrix.matvec(candidate) == self.rhs
    }
}

/// Sets a cancellation flag once the wrapped adapter has judged `limit`
/// candidates.
struct CancelAfter<'a> {
    limit: usize,
    seen: Cell<usize>,
    cancellation: &'a AtomicBool,
}

impl OsdSemantics for CancelAfter<'_> {
    fn reprocessed_columns(&self) -> ReprocessedColumns {
        RowSpaceSemantics.reprocessed_columns()
    }

    fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
        RowSpaceSemantics.base_candidate(basis)
    }

    fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
        RowSpaceSemantics.position_deltas(basis)
    }

    fn accepts(&self, candidate: &BitVec) -> bool {
        self.seen.set(self.seen.get() + 1);
        if self.seen.get() >= self.limit {
            self.cancellation.store(true, Ordering::Relaxed);
        }
        RowSpaceSemantics.accepts(candidate)
    }
}

/// Rejects every candidate, so no pattern reaches the ranking.
struct RejectAllSemantics;

impl OsdSemantics for RejectAllSemantics {
    fn reprocessed_columns(&self) -> ReprocessedColumns {
        ReprocessedColumns::Basis
    }

    fn base_candidate(&self, basis: &MostReliableBasis) -> BitVec {
        RowSpaceSemantics.base_candidate(basis)
    }

    fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
        RowSpaceSemantics.position_deltas(basis)
    }

    fn accepts(&self, _candidate: &BitVec) -> bool {
        false
    }
}

/// Returns adapter vectors of a caller-chosen shape, to exercise the engine's
/// adapter-conformance checks.
struct MalformedSemantics {
    base_len: usize,
    delta_count: Option<usize>,
    delta_len: usize,
}

impl OsdSemantics for MalformedSemantics {
    fn reprocessed_columns(&self) -> ReprocessedColumns {
        ReprocessedColumns::Basis
    }

    fn base_candidate(&self, _basis: &MostReliableBasis) -> BitVec {
        BitVec::zeros(self.base_len)
    }

    fn position_deltas(&self, basis: &MostReliableBasis) -> Vec<BitVec> {
        let count = self.delta_count.unwrap_or(basis.elimination().rank);
        (0..count).map(|_| BitVec::zeros(self.delta_len)).collect()
    }

    fn accepts(&self, _candidate: &BitVec) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// REQ-01: most-reliable independent basis
// ---------------------------------------------------------------------------

#[test]
fn basis_is_the_greedy_independent_set_of_the_most_reliable_columns() {
    let matrix = matrix_from_rows(&["1001", "0101", "0011"]);
    let magnitudes = [1.0, 2.0, 3.0, 4.0];
    let reference = word("0000");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();

    assert_eq!(basis.preference_order(), &[3, 2, 1, 0]);
    assert_eq!(basis.elimination().rank, 3);
    assert_eq!(basis.reprocessed_cols(ReprocessedColumns::Basis), &[3, 2, 1]);
    assert_eq!(basis.free_cols(), &[0]);
    assert_eq!(basis.reprocessed_cols(ReprocessedColumns::Free), &[0]);
    assert_eq!(basis.preference(), ColumnPreference::MostReliableFirst);
    assert!(basis.is_consistent());
    assert!(basis.transformed_rhs().is_none());
}

#[test]
fn least_reliable_preference_selects_the_opposite_basis() {
    let matrix = matrix_from_rows(&["1001", "0101", "0011"]);
    let magnitudes = [1.0, 2.0, 3.0, 4.0];
    let reference = word("0000");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::LeastReliableFirst,
    )
    .unwrap();

    assert_eq!(basis.preference_order(), &[0, 1, 2, 3]);
    assert_eq!(basis.reprocessed_cols(ReprocessedColumns::Basis), &[0, 1, 2]);
    assert_eq!(basis.free_cols(), &[3]);
}

#[test]
fn free_columns_follow_the_preference_order_not_the_column_order() {
    // Column 0 is the most reliable and column 3 the least, so the preference
    // order is [0, 1, 2, 3] reversed only by magnitude.
    let matrix = matrix_from_rows(&["1000", "0100"]);
    let magnitudes = [1.0, 4.0, 2.0, 3.0];
    let reference = word("0000");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();

    assert_eq!(basis.preference_order(), &[1, 3, 2, 0]);
    assert_eq!(basis.reprocessed_cols(ReprocessedColumns::Basis), &[1, 0]);
    assert_eq!(basis.free_cols(), &[3, 2]);
}

#[test]
fn equal_magnitudes_keep_original_index_ties_in_the_basis() {
    let matrix = matrix_from_rows(&["1100", "0110"]);
    let magnitudes = [2.0, 2.0, 2.0, 2.0];
    let reference = word("0000");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();

    assert_eq!(basis.preference_order(), &[0, 1, 2, 3]);
    assert_eq!(basis.reprocessed_cols(ReprocessedColumns::Basis), &[0, 1]);
}

#[test]
#[should_panic(expected = "NaN")]
fn nan_magnitudes_panic_through_the_canonical_permutation() {
    let matrix = matrix_from_rows(&["10", "01"]);
    let reference = word("00");
    let _ = MostReliableBasis::build(
        &matrix,
        &[1.0, f32::NAN],
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    );
}

// ---------------------------------------------------------------------------
// REQ-05: exhaustive-search equivalence for both adapter shapes
// ---------------------------------------------------------------------------

fn matrix_strategy(rows: usize, cols: usize) -> impl Strategy<Value = BitMatrix> {
    proptest::collection::vec(any::<bool>(), rows * cols).prop_map(move |bits| {
        let mut matrix = BitMatrix::zeros(rows, cols);
        for (index, bit) in bits.into_iter().enumerate() {
            matrix.set(index / cols, index % cols, bit);
        }
        matrix
    })
}

fn word_strategy(cols: usize) -> impl Strategy<Value = BitVec> {
    proptest::collection::vec(any::<bool>(), cols).prop_map(move |bits| {
        let mut vector = BitVec::zeros(cols);
        for (index, bit) in bits.into_iter().enumerate() {
            vector.set(index, bit);
        }
        vector
    })
}

/// Distinct powers of two in a random order: every subset has a distinct exact
/// sum, so the bounded search has a unique minimum and the comparison against
/// the reference implementation is exact.
fn magnitudes_strategy(cols: usize) -> impl Strategy<Value = Vec<f32>> {
    Just(
        (0..cols)
            .map(|index| (1u32 << index) as f32)
            .collect::<Vec<f32>>(),
    )
    .prop_shuffle()
}

fn basis_case() -> impl Strategy<Value = (BitMatrix, Vec<f32>, BitVec, usize)> {
    (1usize..=3, 3usize..=6).prop_flat_map(|(rows, cols)| {
        (
            matrix_strategy(rows, cols),
            magnitudes_strategy(cols),
            word_strategy(cols),
            0usize..=2,
        )
    })
}

fn solution_case() -> impl Strategy<Value = (BitMatrix, BitVec, Vec<f32>, usize)> {
    (1usize..=3, 3usize..=6).prop_flat_map(|(rows, cols)| {
        (
            matrix_strategy(rows, cols),
            word_strategy(cols),
            magnitudes_strategy(cols),
            0usize..=2,
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn basis_shape_matches_exhaustive_row_space_search(
        (matrix, magnitudes, reference, order) in basis_case()
    ) {
        let basis = MostReliableBasis::build(
            &matrix,
            &magnitudes,
            &reference,
            None,
            ColumnPreference::MostReliableFirst,
        )
        .unwrap();
        let outcome = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(order)).unwrap();

        let (expected_metric, expected_word) = exhaustive_row_space_best(
            &matrix,
            basis.reprocessed_cols(ReprocessedColumns::Basis),
            &reference,
            &magnitudes,
            order,
        )
        .unwrap();

        let best = outcome.best().unwrap();
        prop_assert_eq!(best.metric(), expected_metric);
        prop_assert_eq!(best.word(), &expected_word);

        let work = outcome.work();
        prop_assert_eq!(work.rank(), basis.elimination().rank);
        prop_assert_eq!(work.eliminations(), 1);
        prop_assert_eq!(work.termination(), OsdTermination::Exhaustive);
        prop_assert_eq!(work.tested_candidates(), work.generated_patterns());
        prop_assert_eq!(
            work.theoretical_candidates(),
            checked_candidate_bound(basis.elimination().rank, order).unwrap()
        );
        prop_assert_eq!(work.generated_patterns(), work.theoretical_candidates());
    }

    #[test]
    fn free_shape_matches_exhaustive_solution_search(
        (matrix, error, magnitudes, order) in solution_case()
    ) {
        let rhs = matrix.matvec(&error);
        let reference = BitVec::zeros(matrix.cols());
        let basis = MostReliableBasis::build(
            &matrix,
            &magnitudes,
            &reference,
            Some(&rhs),
            ColumnPreference::LeastReliableFirst,
        )
        .unwrap();
        prop_assert!(basis.is_consistent());

        let semantics = SolutionSemantics {
            matrix: matrix.clone(),
            rhs: rhs.clone(),
        };
        let outcome = reprocess(&basis, &semantics, OsdConfig::new(order)).unwrap();

        let (expected_metric, expected_word) = exhaustive_solution_best(
            &matrix,
            &rhs,
            basis.free_cols(),
            &reference,
            &magnitudes,
            order,
        )
        .unwrap();

        let best = outcome.best().unwrap();
        prop_assert_eq!(best.metric(), expected_metric);
        prop_assert_eq!(best.word(), &expected_word);

        let work = outcome.work();
        prop_assert_eq!(work.termination(), OsdTermination::Exhaustive);
        prop_assert_eq!(work.tested_candidates(), work.generated_patterns());
        prop_assert_eq!(
            work.theoretical_candidates(),
            checked_candidate_bound(basis.free_cols().len(), order).unwrap()
        );
    }
}

// ---------------------------------------------------------------------------
// REQ-02: deterministic ranking and tie behavior
// ---------------------------------------------------------------------------

#[test]
fn a_strictly_better_later_candidate_replaces_the_order_zero_candidate() {
    // The single generator row flips every coordinate; the basis coordinate
    // costs 2.0 while the three remaining disagreements cost 3.0 together.
    let matrix = matrix_from_rows(&["1111"]);
    let magnitudes = [2.0, 1.0, 1.0, 1.0];
    let reference = word("0111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    let outcome = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(1)).unwrap();

    let best = outcome.best().unwrap();
    assert_eq!(best.word(), &word("1111"));
    assert_eq!(best.metric(), 2.0);
    assert_eq!(best.pattern(), &[0]);
    assert_eq!(best.generation(), 1);
}

#[test]
fn an_equal_metric_later_candidate_keeps_the_earlier_one() {
    let matrix = matrix_from_rows(&["1111"]);
    let magnitudes = [3.0, 1.0, 1.0, 1.0];
    let reference = word("0111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    let outcome = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(1)).unwrap();

    let best = outcome.best().unwrap();
    assert_eq!(best.metric(), 3.0);
    assert_eq!(best.word(), &word("0000"));
    assert_eq!(best.pattern(), &[] as &[usize]);
    assert_eq!(best.generation(), 0);
}

#[test]
fn a_three_way_metric_tie_resolves_to_the_first_generated_candidate() {
    let matrix = matrix_from_rows(&["1010", "0101"]);
    let magnitudes = [1.0, 1.0, 1.0, 1.0];
    let reference = word("1100");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    let outcome = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(1)).unwrap();

    let best = outcome.best().unwrap();
    assert_eq!(best.metric(), 2.0);
    assert_eq!(best.word(), &word("1111"));
    assert_eq!(best.generation(), 0);
    assert_eq!(outcome.work().generated_patterns(), 3);
}

#[test]
fn order_zero_tests_only_the_base_candidate() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let magnitudes = [3.0, 1.0, 4.0, 2.0];
    let reference = word("1111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    let outcome = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(0)).unwrap();

    let work = outcome.work();
    assert_eq!(work.theoretical_candidates(), 1);
    assert_eq!(work.generated_patterns(), 1);
    assert_eq!(work.tested_candidates(), 1);
    assert_eq!(work.termination(), OsdTermination::Exhaustive);
    assert_eq!(outcome.best().unwrap().pattern(), &[] as &[usize]);
}

// ---------------------------------------------------------------------------
// REQ-03 and REQ-04: work metadata, rank deficiency, inconsistency, caps,
// cancellation
// ---------------------------------------------------------------------------

#[test]
fn rank_deficiency_shrinks_the_search_space_without_failing() {
    // The third row is the sum of the first two.
    let matrix = matrix_from_rows(&["1010", "0110", "1100"]);
    let magnitudes = [8.0, 4.0, 2.0, 1.0];
    let reference = word("1111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    assert_eq!(basis.elimination().rank, 2);
    assert_eq!(basis.reprocessed_cols(ReprocessedColumns::Basis), &[0, 1]);
    assert_eq!(basis.free_cols(), &[2, 3]);

    let outcome = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(2)).unwrap();
    let work = outcome.work();
    assert_eq!(work.rank(), 2);
    assert_eq!(work.theoretical_candidates(), 4);
    assert_eq!(work.generated_patterns(), 4);
    assert_eq!(work.termination(), OsdTermination::Exhaustive);

    let (expected_metric, expected_word) = exhaustive_row_space_best(
        &matrix,
        basis.reprocessed_cols(ReprocessedColumns::Basis),
        &reference,
        &magnitudes,
        2,
    )
    .unwrap();
    assert_eq!(outcome.best().unwrap().metric(), expected_metric);
    assert_eq!(outcome.best().unwrap().word(), &expected_word);
}

#[test]
fn an_inconsistent_transformed_right_hand_side_terminates_without_a_candidate() {
    let matrix = matrix_from_rows(&["1010", "0110", "1100"]);
    let magnitudes = [8.0, 4.0, 2.0, 1.0];
    let reference = word("0000");
    let rhs = word("100");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        Some(&rhs),
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    assert!(!basis.is_consistent());
    assert!(basis.transformed_rhs().unwrap().get(2));

    let semantics = SolutionSemantics {
        matrix: matrix.clone(),
        rhs: rhs.clone(),
    };
    let outcome = reprocess(&basis, &semantics, OsdConfig::new(2)).unwrap();

    assert!(outcome.best().is_none());
    let work = outcome.work();
    assert_eq!(work.rank(), 2);
    assert_eq!(work.generated_patterns(), 0);
    assert_eq!(work.tested_candidates(), 0);
    assert_eq!(work.eliminations(), 1);
    assert_eq!(work.termination(), OsdTermination::InconsistentTransform);
    assert_eq!(
        work.theoretical_candidates(),
        checked_candidate_bound(basis.free_cols().len(), 2).unwrap()
    );
}

#[test]
fn a_consistent_rank_deficient_system_remains_searchable() {
    let matrix = matrix_from_rows(&["1010", "0110", "1100"]);
    let magnitudes = [8.0, 4.0, 2.0, 1.0];
    let reference = BitVec::zeros(4);
    let error = word("0011");
    let rhs = matrix.matvec(&error);

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        Some(&rhs),
        ColumnPreference::LeastReliableFirst,
    )
    .unwrap();
    assert!(basis.is_consistent());
    assert_eq!(basis.elimination().rank, 2);

    let semantics = SolutionSemantics {
        matrix: matrix.clone(),
        rhs: rhs.clone(),
    };
    let outcome = reprocess(&basis, &semantics, OsdConfig::new(2)).unwrap();

    let (expected_metric, expected_word) =
        exhaustive_solution_best(&matrix, &rhs, basis.free_cols(), &reference, &magnitudes, 2)
            .unwrap();
    assert_eq!(outcome.best().unwrap().metric(), expected_metric);
    assert_eq!(outcome.best().unwrap().word(), &expected_word);
    assert_eq!(outcome.work().termination(), OsdTermination::Exhaustive);
}

#[test]
fn a_candidate_cap_stops_the_run_and_ranks_only_the_generated_prefix() {
    let matrix = matrix_from_rows(&["1111"]);
    let magnitudes = [2.0, 1.0, 1.0, 1.0];
    let reference = word("0111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    let config = OsdConfig::new(1).with_candidate_cap(Some(1));
    let outcome = reprocess(&basis, &RowSpaceSemantics, config).unwrap();

    let work = outcome.work();
    assert_eq!(work.theoretical_candidates(), 2);
    assert_eq!(work.generated_patterns(), 1);
    assert_eq!(work.tested_candidates(), 1);
    assert_eq!(work.termination(), OsdTermination::CandidateCap);

    // The order-one flip that wins the uncapped run was never generated.
    let best = outcome.best().unwrap();
    assert_eq!(best.word(), &word("0000"));
    assert_eq!(best.metric(), 3.0);
}

#[test]
fn a_zero_candidate_cap_reports_no_candidate() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let magnitudes = [3.0, 1.0, 4.0, 2.0];
    let reference = word("1111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    let config = OsdConfig::new(1).with_candidate_cap(Some(0));
    let outcome = reprocess(&basis, &RowSpaceSemantics, config).unwrap();

    assert!(outcome.best().is_none());
    assert_eq!(outcome.work().generated_patterns(), 0);
    assert_eq!(outcome.work().termination(), OsdTermination::CandidateCap);
}

#[test]
fn cancellation_during_reprocessing_stops_after_the_observed_candidate() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let magnitudes = [3.0, 1.0, 4.0, 2.0];
    let reference = word("1111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();

    let cancellation = AtomicBool::new(false);
    let semantics = CancelAfter {
        limit: 2,
        seen: Cell::new(0),
        cancellation: &cancellation,
    };
    let outcome = reprocess_with_cancellation(
        &basis,
        &semantics,
        OsdConfig::new(1),
        &cancellation,
    )
    .unwrap();

    let work = outcome.work();
    assert_eq!(work.generated_patterns(), 2);
    assert_eq!(work.tested_candidates(), 2);
    assert_eq!(work.termination(), OsdTermination::Cancelled);
    assert!(outcome.best().is_some());
}

#[test]
fn a_pre_set_cancellation_flag_stops_before_the_first_candidate() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let magnitudes = [3.0, 1.0, 4.0, 2.0];
    let reference = word("1111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();

    let cancellation = AtomicBool::new(true);
    let outcome = reprocess_with_cancellation(
        &basis,
        &RowSpaceSemantics,
        OsdConfig::new(2),
        &cancellation,
    )
    .unwrap();

    assert!(outcome.best().is_none());
    assert_eq!(outcome.work().generated_patterns(), 0);
    assert_eq!(outcome.work().tested_candidates(), 0);
    assert_eq!(outcome.work().termination(), OsdTermination::Cancelled);
}

#[test]
fn rejected_candidates_are_generated_but_not_tested() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let magnitudes = [3.0, 1.0, 4.0, 2.0];
    let reference = word("1111");

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    let outcome = reprocess(&basis, &RejectAllSemantics, OsdConfig::new(1)).unwrap();

    assert!(outcome.best().is_none());
    assert_eq!(outcome.work().generated_patterns(), 3);
    assert_eq!(outcome.work().tested_candidates(), 0);
    assert_eq!(outcome.work().termination(), OsdTermination::Exhaustive);
}

#[test]
fn an_unrepresentable_candidate_bound_is_reported_before_reprocessing() {
    let matrix = BitMatrix::identity(64);
    let magnitudes: Vec<f32> = (0..64).map(|index| index as f32).collect();
    let reference = BitVec::zeros(64);

    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();
    assert_eq!(basis.elimination().rank, 64);

    let error = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(64)).unwrap_err();
    assert!(matches!(error, OsdEngineError::Patterns(_)));
}

// ---------------------------------------------------------------------------
// Dimension and adapter conformance
// ---------------------------------------------------------------------------

#[test]
fn magnitude_and_reference_lengths_must_match_the_column_count() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let reference = word("1111");

    let error = MostReliableBasis::build(
        &matrix,
        &[1.0, 2.0],
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap_err();
    assert_eq!(
        error,
        OsdEngineError::MagnitudeLength {
            expected: 4,
            actual: 2
        }
    );

    let error = MostReliableBasis::build(
        &matrix,
        &[1.0, 2.0, 3.0, 4.0],
        &word("11"),
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap_err();
    assert_eq!(
        error,
        OsdEngineError::ReferenceLength {
            expected: 4,
            actual: 2
        }
    );
}

#[test]
fn a_right_hand_side_must_carry_one_entry_per_row() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let reference = word("1111");

    let error = MostReliableBasis::build(
        &matrix,
        &[1.0, 2.0, 3.0, 4.0],
        &reference,
        Some(&word("111")),
        ColumnPreference::MostReliableFirst,
    )
    .unwrap_err();
    assert_eq!(
        error,
        OsdEngineError::Elimination(OrderedEliminationError::RightHandSideLength {
            expected: 2,
            actual: 3
        })
    );
}

#[test]
fn adapter_vectors_must_match_the_basis_shape() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let magnitudes = [3.0, 1.0, 4.0, 2.0];
    let reference = word("1111");
    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();

    let error = reprocess(
        &basis,
        &MalformedSemantics {
            base_len: 3,
            delta_count: None,
            delta_len: 4,
        },
        OsdConfig::new(1),
    )
    .unwrap_err();
    assert_eq!(
        error,
        OsdEngineError::BaseCandidateLength {
            expected: 4,
            actual: 3
        }
    );

    let error = reprocess(
        &basis,
        &MalformedSemantics {
            base_len: 4,
            delta_count: Some(1),
            delta_len: 4,
        },
        OsdConfig::new(1),
    )
    .unwrap_err();
    assert_eq!(
        error,
        OsdEngineError::DeltaCount {
            expected: 2,
            actual: 1
        }
    );

    let error = reprocess(
        &basis,
        &MalformedSemantics {
            base_len: 4,
            delta_count: None,
            delta_len: 5,
        },
        OsdConfig::new(1),
    )
    .unwrap_err();
    assert_eq!(
        error,
        OsdEngineError::DeltaLength {
            position: 0,
            expected: 4,
            actual: 5
        }
    );
}

#[test]
fn repeated_runs_over_one_basis_are_identical() {
    let matrix = matrix_from_rows(&["1011", "0110"]);
    let magnitudes = [3.0, 1.0, 4.0, 2.0];
    let reference = word("1101");
    let basis = MostReliableBasis::build(
        &matrix,
        &magnitudes,
        &reference,
        None,
        ColumnPreference::MostReliableFirst,
    )
    .unwrap();

    let first = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(2)).unwrap();
    let second = reprocess(&basis, &RowSpaceSemantics, OsdConfig::new(2)).unwrap();

    assert_eq!(first.work(), second.work());
    assert_eq!(
        first.best().map(|candidate| candidate.word()),
        second.best().map(|candidate| candidate.word())
    );
}
