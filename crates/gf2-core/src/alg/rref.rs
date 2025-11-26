//! Reduced Row Echelon Form (RREF) computation over GF(2).
//!
//! This module implements row reduction (Gaussian elimination) to compute
//! the reduced row echelon form of matrices over the binary field GF(2).

use crate::matrix::BitMatrix;

/// Result of reduced row echelon form computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RrefResult {
    /// Matrix in reduced row echelon form
    pub reduced: BitMatrix,

    /// Indices of pivot columns (in order found during reduction)
    pub pivot_cols: Vec<usize>,

    /// Row permutation applied: reduced_row[i] = input_row[row_perm[i]]
    pub row_perm: Vec<usize>,

    /// Rank of the matrix (number of linearly independent rows)
    pub rank: usize,
}

/// Compute the reduced row echelon form (RREF) of a matrix over GF(2).
///
/// Performs row reduction with column pivoting to transform the input matrix
/// into reduced row echelon form. This is the standard form produced by
/// Gaussian elimination.
///
/// # Arguments
///
/// * `matrix` - Input matrix to reduce
/// * `pivot_from_right` - If true, search for pivots from right to left;
///   if false, search left to right
///
/// # Returns
///
/// Result containing:
/// - The reduced matrix in RREF
/// - Pivot column indices (in order found)
/// - Row permutation applied
/// - Matrix rank
///
/// # Algorithm
///
/// Uses word-level XOR operations for ~64× speedup over bit-level operations.
/// Complexity: O(m² × n / 64) for dense matrices.
///
/// # Examples
///
/// ```
/// use gf2_core::matrix::BitMatrix;
/// use gf2_core::alg::rref::rref;
///
/// // Simple 2×3 matrix: [1 0 1]
/// //                    [0 1 1]
/// let mut m = BitMatrix::zeros(2, 3);
/// m.set(0, 0, true);
/// m.set(0, 2, true);
/// m.set(1, 1, true);
/// m.set(1, 2, true);
///
/// let result = rref(&m, false);
/// assert_eq!(result.rank, 2);
/// assert_eq!(result.pivot_cols, vec![0, 1]);
/// ```
pub fn rref(matrix: &BitMatrix, pivot_from_right: bool) -> RrefResult {
    let m = matrix.rows();
    let n = matrix.cols();

    // Handle empty matrix
    if m == 0 || n == 0 {
        return RrefResult {
            reduced: matrix.clone(),
            pivot_cols: Vec::new(),
            row_perm: Vec::new(),
            rank: 0,
        };
    }

    // Create working copy
    let mut work = matrix.clone();

    // Track row permutation
    let mut row_perm: Vec<usize> = (0..m).collect();

    // Track pivot columns
    let mut pivot_cols = Vec::new();

    let mut current_row = 0;

    // Forward elimination: find pivots and eliminate
    // Process columns in order based on pivot_from_right flag
    let mut col_iter = 0..n;
    while current_row < m {
        let col = if pivot_from_right {
            match col_iter.next_back() {
                Some(c) => c,
                None => break,
            }
        } else {
            match col_iter.next() {
                Some(c) => c,
                None => break,
            }
        };

        // Rest of loop body
        {
            // Find pivot row using optimized word-level search
            if let Some(pivot_row) = work.find_pivot_row(col, current_row) {
                // Swap pivot row to current position
                if pivot_row != current_row {
                    work.swap_rows(current_row, pivot_row);
                    row_perm.swap(current_row, pivot_row);
                }

                // Record pivot column
                pivot_cols.push(col);

                // Eliminate this column from all OTHER rows (reduced form)
                // Use unchecked access for inner loop performance
                for r in 0..m {
                    if r != current_row && work.get_unchecked(r, col) {
                        // XOR current_row into row r using built-in method
                        work.row_xor(r, current_row);
                    }
                }

                current_row += 1;
            }
        } // End of inner scope
    }

    let rank = current_row;

    // Sort pivot_cols to match natural column order (for consistency)
    if pivot_from_right {
        pivot_cols.reverse();
    }

    RrefResult {
        reduced: work,
        pivot_cols,
        row_perm,
        rank,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_rref_empty_matrix() {
        let m = BitMatrix::zeros(0, 0);
        let result = rref(&m, false);

        assert_eq!(result.rank, 0);
        assert_eq!(result.pivot_cols.len(), 0);
        assert_eq!(result.reduced.rows(), 0);
        assert_eq!(result.reduced.cols(), 0);
    }

    #[test]
    fn test_rref_single_element_zero() {
        let m = BitMatrix::zeros(1, 1);
        let result = rref(&m, false);

        assert_eq!(result.rank, 0);
        assert!(result.pivot_cols.is_empty());
    }

    #[test]
    fn test_rref_single_element_one() {
        let mut m = BitMatrix::zeros(1, 1);
        m.set(0, 0, true);
        let result = rref(&m, false);

        assert_eq!(result.rank, 1);
        assert_eq!(result.pivot_cols, vec![0]);
        assert!(result.reduced.get(0, 0));
    }

    #[test]
    fn test_rref_identity_2x2() {
        let m = BitMatrix::identity(2);
        let result = rref(&m, false);

        assert_eq!(result.rank, 2);
        assert_eq!(result.pivot_cols, vec![0, 1]);

        // Result should still be identity
        assert!(result.reduced.get(0, 0));
        assert!(!result.reduced.get(0, 1));
        assert!(!result.reduced.get(1, 0));
        assert!(result.reduced.get(1, 1));
    }

    #[test]
    fn test_rref_simple_2x3() {
        // Matrix: [1 0 1]
        //         [0 1 1]
        // Already in RREF
        let mut m = BitMatrix::zeros(2, 3);
        m.set(0, 0, true);
        m.set(0, 2, true);
        m.set(1, 1, true);
        m.set(1, 2, true);

        let result = rref(&m, false);

        assert_eq!(result.rank, 2);
        assert_eq!(result.pivot_cols, vec![0, 1]);
    }

    #[test]
    fn test_rref_needs_elimination() {
        // Matrix: [1 1 0]
        //         [1 0 1]
        // RREF should be: [1 0 1]
        //                 [0 1 1]
        let mut m = BitMatrix::zeros(2, 3);
        m.set(0, 0, true);
        m.set(0, 1, true);
        m.set(1, 0, true);
        m.set(1, 2, true);

        let result = rref(&m, false);

        assert_eq!(result.rank, 2);
        assert_eq!(result.pivot_cols, vec![0, 1]);

        // Check RREF form
        assert!(result.reduced.get(0, 0));
        assert!(!result.reduced.get(0, 1));
        assert!(result.reduced.get(0, 2));
        assert!(!result.reduced.get(1, 0));
        assert!(result.reduced.get(1, 1));
        assert!(result.reduced.get(1, 2));
    }

    #[test]
    fn test_rref_rank_deficient() {
        // Matrix: [1 0 1]
        //         [1 0 1]  (duplicate row)
        // RREF: [1 0 1]
        //       [0 0 0]
        let mut m = BitMatrix::zeros(2, 3);
        m.set(0, 0, true);
        m.set(0, 2, true);
        m.set(1, 0, true);
        m.set(1, 2, true);

        let result = rref(&m, false);

        assert_eq!(result.rank, 1);
        assert_eq!(result.pivot_cols, vec![0]);

        // First row should be [1 0 1]
        assert!(result.reduced.get(0, 0));
        assert!(!result.reduced.get(0, 1));
        assert!(result.reduced.get(0, 2));

        // Second row should be all zeros
        assert!(!result.reduced.get(1, 0));
        assert!(!result.reduced.get(1, 1));
        assert!(!result.reduced.get(1, 2));
    }

    #[test]
    fn test_rref_all_zeros() {
        let m = BitMatrix::zeros(3, 4);
        let result = rref(&m, false);

        assert_eq!(result.rank, 0);
        assert!(result.pivot_cols.is_empty());

        // Should remain all zeros
        for r in 0..3 {
            for c in 0..4 {
                assert!(!result.reduced.get(r, c));
            }
        }
    }

    #[test]
    fn test_rref_pivot_from_right() {
        // Matrix: [1 1 0]
        //         [0 1 1]
        // When pivoting from right, should prefer rightmost pivots
        let mut m = BitMatrix::zeros(2, 3);
        m.set(0, 0, true);
        m.set(0, 1, true);
        m.set(1, 1, true);
        m.set(1, 2, true);

        let result = rref(&m, true);

        assert_eq!(result.rank, 2);
        // With right-to-left pivoting, should select columns 2, 1 (in that search order)
        // But pivot_cols should still be ordered by when found
    }

    // Property-based tests
    proptest! {
        #[test]
        fn prop_rref_rank_bounded(rows in 1..20usize, cols in 1..20usize, seed in any::<u64>()) {
            use rand::rngs::StdRng;
            use rand::{Rng, SeedableRng};

            let mut rng = StdRng::seed_from_u64(seed);
            let mut m = BitMatrix::zeros(rows, cols);

            for r in 0..rows {
                for c in 0..cols {
                    if rng.gen_bool(0.5) {
                        m.set(r, c, true);
                    }
                }
            }

            let result = rref(&m, false);

            // Rank must be at most min(rows, cols)
            prop_assert!(result.rank <= rows.min(cols));

            // Number of pivot columns must equal rank
            prop_assert_eq!(result.pivot_cols.len(), result.rank);
        }

        #[test]
        fn prop_rref_idempotent(rows in 1..10usize, cols in 1..10usize, seed in any::<u64>()) {
            use rand::rngs::StdRng;
            use rand::{Rng, SeedableRng};

            let mut rng = StdRng::seed_from_u64(seed);
            let mut m = BitMatrix::zeros(rows, cols);

            for r in 0..rows {
                for c in 0..cols {
                    if rng.gen_bool(0.5) {
                        m.set(r, c, true);
                    }
                }
            }

            let result1 = rref(&m, false);
            let result2 = rref(&result1.reduced, false);

            // RREF of RREF should be the same (idempotent)
            prop_assert_eq!(result1.reduced, result2.reduced);
            prop_assert_eq!(result1.rank, result2.rank);
        }
    }
}
