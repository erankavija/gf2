//! Conformance tests for ordered-column elimination over GF(2).
//!
//! The suite pins the four observable contracts of
//! [`gf2_core::alg::rref::ordered_column_elimination`]: preference-ordered basis
//! selection with a reusable row transform, right-hand-side equivalence under
//! that transform, deterministic behaviour on rank-deficient, empty, and
//! rectangular inputs, and rejection of malformed preferences and dimensions.
//! It also ties the primitive back to the existing RREF semantics under the
//! natural and reversed column orders.

use gf2_core::alg::rref::{ordered_column_elimination, rref, OrderedEliminationError};
use gf2_core::matrix::BitMatrix;
use gf2_core::BitVec;
use proptest::prelude::*;

/// Word-boundary column counts required by the repository bit-packing policy.
const BOUNDARY_SIZES: [usize; 5] = [0, 1, 63, 64, 65];

fn natural_preference(cols: usize) -> Vec<usize> {
    (0..cols).collect()
}

fn reversed_preference(cols: usize) -> Vec<usize> {
    (0..cols).rev().collect()
}

fn random_matrix(rows: usize, cols: usize, density: f64, seed: u64) -> BitMatrix {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = BitMatrix::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen_bool(density) {
                m.set(r, c, true);
            }
        }
    }
    m
}

fn random_vector(len: usize, seed: u64) -> BitVec {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(seed);
    let mut v = BitVec::zeros(len);
    for i in 0..len {
        v.set(i, rng.gen_bool(0.5));
    }
    v
}

/// Rank-deficient matrix whose last `rows / 2` rows repeat earlier rows.
fn duplicated_rows(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let independent = rows.div_ceil(2);
    let base = random_matrix(independent, cols, 0.5, seed);
    let mut m = BitMatrix::zeros(rows, cols);
    for r in 0..rows {
        let src = r % independent;
        for c in 0..cols {
            if base.get(src, c) {
                m.set(r, c, true);
            }
        }
    }
    m
}

/// Canonical row-space fingerprint: the left-to-right RREF is unique per row
/// space, so equal fingerprints mean equal row spaces.
fn row_space(m: &BitMatrix) -> BitMatrix {
    rref(m, false).reduced
}

// ---------------------------------------------------------------------------
// REQ-01: ordered basis selection and applied row transform
// ---------------------------------------------------------------------------

#[test]
fn selects_greedy_independent_columns_in_preference_order() {
    // Columns 0 and 2 are equal, column 3 is the sum of columns 0 and 1.
    //     [1 0 1 1]
    //     [0 1 0 1]
    //     [1 1 1 0]
    let mut a = BitMatrix::zeros(3, 4);
    for (r, c) in [(0, 0), (0, 2), (0, 3), (1, 1), (1, 3), (2, 0), (2, 1), (2, 2)] {
        a.set(r, c, true);
    }

    let result = ordered_column_elimination(&a, &[3, 2, 1, 0]).expect("valid preference");

    assert_eq!(result.rank, 2);
    assert_eq!(
        result.selected_cols,
        vec![3, 2],
        "preference order decides which independent columns enter the basis"
    );

    // The selected columns carry the identity of the reduced matrix.
    for (row, &col) in result.selected_cols.iter().enumerate() {
        for other in 0..result.reduced.rows() {
            assert_eq!(
                result.reduced.get(other, col),
                other == row,
                "selected column {col} must be the unit column of row {row}"
            );
        }
    }
}

#[test]
fn transform_reproduces_the_reduced_matrix() {
    for cols in BOUNDARY_SIZES {
        for rows in [1usize, 5, 64, 65] {
            let a = random_matrix(rows, cols, 0.5, 0xA11CE ^ (rows as u64) ^ (cols as u64) << 8);
            let preference = reversed_preference(cols);
            let result = ordered_column_elimination(&a, &preference).expect("valid preference");

            assert_eq!(result.transform.rows(), rows);
            assert_eq!(result.transform.cols(), rows);
            assert_eq!(
                &result.transform * &a,
                result.reduced,
                "reduced matrix must equal U·A for rows={rows} cols={cols}"
            );
            assert_eq!(
                rref(&result.transform, false).rank,
                rows,
                "the row transform is invertible for rows={rows} cols={cols}"
            );
        }
    }
}

#[test]
fn selected_columns_match_the_rank_and_are_preference_ordered() {
    let preference = [5, 0, 7, 2, 4, 1, 6, 3];
    for seed in 0..8u64 {
        let a = random_matrix(6, 8, 0.4, seed);
        let result = ordered_column_elimination(&a, &preference).expect("valid preference");

        assert_eq!(result.selected_cols.len(), result.rank);
        assert_eq!(result.rank, rref(&a, false).rank, "rank is order-independent");

        let positions: Vec<usize> = result
            .selected_cols
            .iter()
            .map(|col| preference.iter().position(|p| p == col).expect("in range"))
            .collect();
        assert!(
            positions.windows(2).all(|w| w[0] < w[1]),
            "selected columns follow the preference order, got {positions:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// REQ-02: right-hand-side transform equivalence
// ---------------------------------------------------------------------------

#[test]
fn transformed_system_has_the_same_solutions() {
    for cols in [1usize, 7, 63, 64, 65] {
        for rows in [1usize, 6, 64] {
            let a = random_matrix(rows, cols, 0.5, 0xB0B ^ (rows as u64) << 16 ^ cols as u64);
            let result = ordered_column_elimination(&a, &reversed_preference(cols))
                .expect("valid preference");

            for seed in 0..4u64 {
                let x = random_vector(cols, seed ^ 0xF00D);
                let b = a.matvec(&x);
                let transformed = result.apply_transform(&b).expect("conforming right-hand side");

                assert_eq!(
                    result.reduced.matvec(&x),
                    transformed,
                    "x solving Ax=b must solve (UA)x=Ub for rows={rows} cols={cols}"
                );
                assert!(
                    result.is_consistent(&transformed).expect("conforming"),
                    "a right-hand side in the column space is consistent"
                );
            }
        }
    }
}

#[test]
fn apply_transform_agrees_with_the_returned_matrix() {
    let a = random_matrix(9, 12, 0.5, 4242);
    let result = ordered_column_elimination(&a, &natural_preference(12)).expect("valid preference");
    let b = random_vector(9, 99);

    assert_eq!(result.apply_transform(&b).expect("conforming"), result.transform.matvec(&b));
}

// ---------------------------------------------------------------------------
// REQ-03: rank-deficient, empty, rectangular, and inconsistent inputs
// ---------------------------------------------------------------------------

#[test]
fn rank_deficient_input_leaves_zero_rows_below_the_rank() {
    for cols in [1usize, 63, 64, 65] {
        let a = duplicated_rows(8, cols, 0xDEF ^ cols as u64);
        let result =
            ordered_column_elimination(&a, &reversed_preference(cols)).expect("valid preference");

        assert!(result.rank < 8, "input is rank deficient for cols={cols}");
        for row in result.rank..a.rows() {
            for col in 0..cols {
                assert!(
                    !result.reduced.get(row, col),
                    "row {row} below the rank must be zero for cols={cols}"
                );
            }
        }
        assert_eq!(&result.transform * &a, result.reduced);
    }
}

#[test]
fn empty_dimensions_are_deterministic() {
    let empty = BitMatrix::zeros(0, 0);
    let result = ordered_column_elimination(&empty, &[]).expect("empty preference");
    assert_eq!(result.rank, 0);
    assert!(result.selected_cols.is_empty());
    assert_eq!(result.reduced.rows(), 0);
    assert_eq!(result.reduced.cols(), 0);
    assert_eq!(result.transform.rows(), 0);
    assert_eq!(result.transform.cols(), 0);

    let no_rows = BitMatrix::zeros(0, 5);
    let result = ordered_column_elimination(&no_rows, &natural_preference(5)).expect("valid");
    assert_eq!(result.rank, 0);
    assert!(result.selected_cols.is_empty());
    assert_eq!(result.reduced.rows(), 0);
    assert_eq!(result.reduced.cols(), 5);
    assert_eq!(result.transform.rows(), 0);

    let no_cols = BitMatrix::zeros(4, 0);
    let result = ordered_column_elimination(&no_cols, &[]).expect("empty preference");
    assert_eq!(result.rank, 0);
    assert!(result.selected_cols.is_empty());
    assert_eq!(result.reduced.cols(), 0);
    assert_eq!(result.transform, BitMatrix::identity(4));
    assert!(result
        .is_consistent(&BitVec::zeros(4))
        .expect("conforming right-hand side"));
    assert!(!result
        .is_consistent(&{
            let mut b = BitVec::zeros(4);
            b.set(2, true);
            b
        })
        .expect("conforming right-hand side"));
}

#[test]
fn rectangular_inputs_reduce_deterministically() {
    for (rows, cols) in [(3usize, 11usize), (11, 3), (1, 65), (65, 1)] {
        let a = random_matrix(rows, cols, 0.5, (rows * 1000 + cols) as u64);
        let preference = reversed_preference(cols);
        let first = ordered_column_elimination(&a, &preference).expect("valid preference");
        let second = ordered_column_elimination(&a, &preference).expect("valid preference");

        assert_eq!(first, second, "repeated calls agree for {rows}×{cols}");
        assert_eq!(first.rank, rref(&a, false).rank);
        assert!(first.rank <= rows.min(cols));
        assert_eq!(&first.transform * &a, first.reduced);
    }
}

#[test]
fn right_hand_side_outside_the_row_space_is_inconsistent() {
    // Rows 0 and 1 are equal, so any b with b0 != b1 is unreachable.
    //     [1 0]
    //     [1 0]
    //     [0 1]
    let mut a = BitMatrix::zeros(3, 2);
    a.set(0, 0, true);
    a.set(1, 0, true);
    a.set(2, 1, true);

    let result = ordered_column_elimination(&a, &[0, 1]).expect("valid preference");
    assert_eq!(result.rank, 2);

    let mut unreachable = BitVec::zeros(3);
    unreachable.set(1, true);
    let transformed = result.apply_transform(&unreachable).expect("conforming");
    assert!(
        !result.is_consistent(&transformed).expect("conforming"),
        "a right-hand side outside the reachable row space is inconsistent"
    );

    let mut x = BitVec::zeros(2);
    x.set(0, true);
    x.set(1, true);
    let reachable = a.matvec(&x);
    let transformed = result.apply_transform(&reachable).expect("conforming");
    assert!(result.is_consistent(&transformed).expect("conforming"));
    assert_eq!(result.reduced.matvec(&x), transformed);
}

#[test]
fn every_zero_row_pairs_with_a_consistency_condition() {
    let a = duplicated_rows(7, 9, 271828);
    let result = ordered_column_elimination(&a, &reversed_preference(9)).expect("valid preference");
    assert!(result.rank < 7);

    let mut transformed = BitVec::zeros(7);
    assert!(result.is_consistent(&transformed).expect("conforming"));
    transformed.set(result.rank, true);
    assert!(
        !result.is_consistent(&transformed).expect("conforming"),
        "a nonzero entry aligned with a zero reduced row is inconsistent"
    );
}

// ---------------------------------------------------------------------------
// REQ-04: malformed preferences and right-hand-side dimensions
// ---------------------------------------------------------------------------

#[test]
fn preference_length_mismatch_is_rejected() {
    let a = BitMatrix::identity(3);
    assert_eq!(
        ordered_column_elimination(&a, &[0, 1]),
        Err(OrderedEliminationError::PreferenceLength {
            expected: 3,
            actual: 2
        })
    );
    assert_eq!(
        ordered_column_elimination(&a, &[0, 1, 2, 0]),
        Err(OrderedEliminationError::PreferenceLength {
            expected: 3,
            actual: 4
        })
    );
}

#[test]
fn preference_column_out_of_range_is_rejected() {
    let a = BitMatrix::identity(3);
    assert_eq!(
        ordered_column_elimination(&a, &[0, 3, 1]),
        Err(OrderedEliminationError::PreferenceColumnOutOfRange {
            position: 1,
            column: 3,
            cols: 3
        })
    );
}

#[test]
fn preference_repeated_column_is_rejected() {
    let a = BitMatrix::identity(3);
    assert_eq!(
        ordered_column_elimination(&a, &[2, 0, 2]),
        Err(OrderedEliminationError::PreferenceColumnRepeated {
            position: 2,
            column: 2,
            first_position: 0
        })
    );
}

#[test]
fn right_hand_side_length_mismatch_is_rejected() {
    let a = random_matrix(4, 6, 0.5, 7);
    let result = ordered_column_elimination(&a, &natural_preference(6)).expect("valid preference");
    let expected = Err(OrderedEliminationError::RightHandSideLength {
        expected: 4,
        actual: 6,
    });

    assert_eq!(result.apply_transform(&BitVec::zeros(6)), expected);
    assert_eq!(result.is_consistent(&BitVec::zeros(6)), expected);
}

#[test]
fn errors_describe_themselves() {
    let rendered = format!(
        "{}",
        OrderedEliminationError::PreferenceColumnRepeated {
            position: 2,
            column: 2,
            first_position: 0
        }
    );
    assert!(rendered.contains('2'), "message names the repeated column");

    let error: &dyn std::error::Error = &OrderedEliminationError::RightHandSideLength {
        expected: 4,
        actual: 6,
    };
    assert!(!error.to_string().is_empty());
}

// ---------------------------------------------------------------------------
// REQ-05: agreement with the existing RREF semantics
// ---------------------------------------------------------------------------

#[test]
fn natural_order_matches_left_to_right_rref() {
    for cols in BOUNDARY_SIZES {
        for rows in [0usize, 1, 5, 64, 65] {
            let a = random_matrix(rows, cols, 0.5, 0x5EED ^ (rows as u64) << 32 ^ cols as u64);
            let ordered = ordered_column_elimination(&a, &natural_preference(cols))
                .expect("valid preference");
            let baseline = rref(&a, false);

            assert_eq!(ordered.rank, baseline.rank, "rank for {rows}×{cols}");
            assert_eq!(
                ordered.selected_cols, baseline.pivot_cols,
                "pivot columns for {rows}×{cols}"
            );
            assert_eq!(
                ordered.reduced, baseline.reduced,
                "reduced matrix for {rows}×{cols}"
            );
        }
    }
}

#[test]
fn reversed_order_matches_right_to_left_rref() {
    for cols in BOUNDARY_SIZES {
        for rows in [0usize, 1, 5, 64, 65] {
            let a = random_matrix(rows, cols, 0.5, 0xC0FFEE ^ (rows as u64) << 32 ^ cols as u64);
            let ordered = ordered_column_elimination(&a, &reversed_preference(cols))
                .expect("valid preference");
            let baseline = rref(&a, true);

            assert_eq!(ordered.rank, baseline.rank, "rank for {rows}×{cols}");

            let mut selected = ordered.selected_cols.clone();
            selected.sort_unstable();
            assert_eq!(
                selected, baseline.pivot_cols,
                "right-to-left pivot columns for {rows}×{cols}"
            );
            assert_eq!(
                row_space(&ordered.reduced),
                row_space(&baseline.reduced),
                "row space for {rows}×{cols}"
            );
        }
    }
}

#[test]
fn row_space_is_preserved_under_every_preference() {
    let a = random_matrix(6, 9, 0.5, 31337);
    let baseline = row_space(&a);
    let preferences: [&[usize]; 3] = [
        &[0, 1, 2, 3, 4, 5, 6, 7, 8],
        &[8, 7, 6, 5, 4, 3, 2, 1, 0],
        &[4, 0, 8, 2, 6, 1, 7, 3, 5],
    ];

    for preference in preferences {
        let result = ordered_column_elimination(&a, preference).expect("valid preference");
        assert_eq!(row_space(&result.reduced), baseline);
        assert_eq!(result.rank, rref(&a, false).rank);
    }
}

proptest! {
    #[test]
    fn prop_transform_and_solutions_are_consistent(
        rows in 1..12usize,
        cols in 1..12usize,
        seed in any::<u64>(),
        rotate in 0..12usize,
    ) {
        let a = random_matrix(rows, cols, 0.5, seed);
        let mut preference = natural_preference(cols);
        preference.rotate_left(rotate % cols);

        let result = ordered_column_elimination(&a, &preference).expect("valid preference");

        prop_assert_eq!(result.rank, result.selected_cols.len());
        prop_assert!(result.rank <= rows.min(cols));
        prop_assert_eq!(&result.transform * &a, result.reduced.clone());
        prop_assert_eq!(rref(&result.transform, false).rank, rows);
        prop_assert_eq!(row_space(&result.reduced), row_space(&a));

        let x = random_vector(cols, seed ^ 0x1234_5678);
        let b = a.matvec(&x);
        let transformed = result.apply_transform(&b).expect("conforming");
        prop_assert_eq!(result.reduced.matvec(&x), transformed.clone());
        prop_assert!(result.is_consistent(&transformed).expect("conforming"));
    }
}
