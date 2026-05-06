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

    /// Row permutation applied: reduced_row\[i\] = input_row\[row_perm\[i\]\]
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
/// Uses an M4RI-style blocked schedule for left-to-right pivoting: pivot rows
/// are collected in small blocks, a Gray table of row combinations is built,
/// and each non-pivot row clears the whole block with at most one suffix XOR.
/// This keeps the public API unchanged while reducing dense RREF row traffic by
/// roughly the block width. Right-to-left pivoting uses the compatible
/// unblocked path.
///
/// Complexity remains O(m² × n / 64) word operations for dense matrices, with a
/// lower constant factor from Gray-table batching and suffix-only row updates.
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
    rref_with_block_size(matrix, pivot_from_right, default_block_size(matrix.cols()))
}

/// Test-support hook for benchmarking the same RREF implementation with a
/// fixed M4RI block size. `block_size = 1` is the scalar baseline: it uses the
/// same pivoting and suffix-XOR kernels as production, but disables Gray-table
/// row-combination batching.
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub fn rref_with_block_size_for_test(
    matrix: &BitMatrix,
    pivot_from_right: bool,
    block_size: usize,
) -> RrefResult {
    rref_with_block_size(matrix, pivot_from_right, block_size)
}

fn default_block_size(cols: usize) -> usize {
    match cols {
        0..=64 => 4,
        65..=512 => 4,
        _ => 8,
    }
}

fn rref_with_block_size(
    matrix: &BitMatrix,
    pivot_from_right: bool,
    block_size: usize,
) -> RrefResult {
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

    if pivot_from_right {
        return rref_unblocked_right_to_left(matrix);
    }

    let block_size = block_size.clamp(1, 10);

    // Create working copy
    let mut work = matrix.clone();

    // Track row permutation
    let mut row_perm: Vec<usize> = (0..m).collect();

    // Track pivot columns
    let mut pivot_cols = Vec::new();

    let xor = crate::kernels::ops::resolve_xor_inplace(work.stride_words());
    let mut current_row = 0;
    let mut col = 0;

    while current_row < m && col < n {
        let block_row_start = current_row;
        let first_block_word = col / 64;
        let mut block_pivots = Vec::with_capacity(block_size);

        while current_row < m && col < n && block_pivots.len() < block_size {
            if let Some(pivot_row) =
                find_block_pivot(&mut work, block_row_start, current_row, col, &block_pivots)
            {
                if pivot_row != current_row {
                    work.swap_rows(current_row, pivot_row);
                    row_perm.swap(current_row, pivot_row);
                }

                for (offset, &pivot_col) in block_pivots.iter().enumerate() {
                    let prev_row = block_row_start + offset;
                    debug_assert!(
                        !work.get_unchecked(current_row, pivot_col),
                        "block pivot reduction left an earlier pivot set"
                    );
                    if work.get_unchecked(prev_row, col) {
                        work.row_xor_from(prev_row, current_row, pivot_col / 64);
                    }
                }

                block_pivots.push(col);
                pivot_cols.push(col);
                current_row += 1;
            } else if current_row <= m.saturating_mul(3) / 4
                && !tail_has_nonzero_from(&work, current_row, col + 1)
            {
                col = n;
                break;
            }
            col += 1;
        }

        if !block_pivots.is_empty() {
            eliminate_block(
                &mut work,
                m,
                block_row_start,
                &block_pivots,
                first_block_word,
                xor,
            );
        }
    }

    let rank = current_row;

    RrefResult {
        reduced: work,
        pivot_cols,
        row_perm,
        rank,
    }
}

fn tail_has_nonzero_from(work: &BitMatrix, start_row: usize, start_col: usize) -> bool {
    if start_row >= work.rows() || start_col >= work.cols() {
        return false;
    }

    let start_word = start_col / 64;
    let start_mask = !0u64 << (start_col & 63);
    for row in start_row..work.rows() {
        let words = work.row_words(row);
        if words[start_word] & start_mask != 0 {
            return true;
        }
        if words[start_word + 1..].iter().any(|&word| word != 0) {
            return true;
        }
    }
    false
}

fn find_block_pivot(
    work: &mut BitMatrix,
    block_row_start: usize,
    start_row: usize,
    col: usize,
    block_pivots: &[usize],
) -> Option<usize> {
    for row in start_row..work.rows() {
        for (offset, &pivot_col) in block_pivots.iter().enumerate() {
            if work.get_unchecked(row, pivot_col) {
                work.row_xor_from(row, block_row_start + offset, pivot_col / 64);
            }
        }
        if work.get_unchecked(row, col) {
            return Some(row);
        }
    }
    None
}

fn eliminate_block(
    work: &mut BitMatrix,
    rows: usize,
    block_row_start: usize,
    block_pivots: &[usize],
    first_word: usize,
    xor: crate::kernels::ops::XorInplaceFn,
) {
    let block_rows = block_pivots.len();
    let suffix_words = work.stride_words() - first_word;
    if suffix_words == 0 {
        return;
    }

    let table_rows = 1usize << block_rows;
    let mut table = vec![0u64; table_rows * suffix_words];
    for idx in 1..table_rows {
        let bit = idx.trailing_zeros() as usize;
        let prev = idx & !(1usize << bit);

        let (before, after) = table.split_at_mut(idx * suffix_words);
        let prev_slice = &before[prev * suffix_words..prev * suffix_words + suffix_words];
        let dst = &mut after[..suffix_words];
        dst.copy_from_slice(prev_slice);
        xor(
            dst,
            &work.row_words(block_row_start + bit)[first_word..first_word + suffix_words],
        );
    }

    for row in 0..rows {
        if (block_row_start..block_row_start + block_rows).contains(&row) {
            continue;
        }
        let table_idx = block_table_index(work, row, block_pivots);
        if table_idx != 0 {
            let src_start = table_idx * suffix_words;
            work.row_xor_slice_from(row, first_word, &table[src_start..src_start + suffix_words]);
        }
    }
}

fn block_table_index(work: &BitMatrix, row: usize, block_pivots: &[usize]) -> usize {
    if let (Some(&first), Some(&last)) = (block_pivots.first(), block_pivots.last()) {
        let width = block_pivots.len();
        if last + 1 == first + width && first / 64 == last / 64 {
            let mask = (1usize << width) - 1;
            return ((work.row_words(row)[first / 64] >> (first & 63)) as usize) & mask;
        }
    }

    let mut table_idx = 0usize;
    for (bit, &pivot_col) in block_pivots.iter().enumerate() {
        if work.get_unchecked(row, pivot_col) {
            table_idx |= 1usize << bit;
        }
    }
    table_idx
}

fn rref_unblocked_right_to_left(matrix: &BitMatrix) -> RrefResult {
    let m = matrix.rows();
    let n = matrix.cols();

    let mut work = matrix.clone();
    let mut row_perm: Vec<usize> = (0..m).collect();
    let mut pivot_cols = Vec::new();
    let mut current_row = 0;
    for col in (0..n).rev() {
        if current_row == m {
            break;
        }
        if let Some(pivot_row) = work.find_pivot_row(col, current_row) {
            if pivot_row != current_row {
                work.swap_rows(current_row, pivot_row);
                row_perm.swap(current_row, pivot_row);
            }

            pivot_cols.push(col);

            for r in 0..m {
                if r != current_row && work.get_unchecked(r, col) {
                    work.row_xor(r, current_row);
                }
            }

            current_row += 1;
        }
    }

    let rank = current_row;

    // When pivoting right-to-left, we need to reorder rows to maintain RREF invariant:
    // pivot_cols must be in ascending order with pivot_cols[i] being the pivot for row i
    if rank > 0 {
        // Create mapping: (row_index, pivot_col) and sort by pivot_col
        let mut col_to_row: Vec<(usize, usize)> = pivot_cols.iter().copied().enumerate().collect();
        col_to_row.sort_unstable_by_key(|(_, col)| *col);

        // Check if reordering is actually needed
        let needs_reorder = col_to_row.iter().enumerate().any(|(i, &(row, _))| i != row);

        if needs_reorder {
            let new_row_order: Vec<usize> = col_to_row.iter().map(|(row, _)| *row).collect();
            let sorted_pivot_cols: Vec<usize> = col_to_row.iter().map(|(_, col)| *col).collect();

            // Build new matrix with reordered rows using word-level operations
            let mut new_work = BitMatrix::zeros(m, n);

            // Copy reordered pivot rows word-by-word for performance
            for (new_row, &old_row) in new_row_order.iter().enumerate() {
                let old_words = work.row_words(old_row);
                let new_words = new_work.row_words_mut(new_row);
                new_words.copy_from_slice(old_words);
            }

            // Zero rows are already zero in new_work (no need to copy)

            work = new_work;

            // Update row_perm to reflect reordering
            let old_row_perm = row_perm.clone();
            for (new_row, &old_row) in new_row_order.iter().enumerate() {
                row_perm[new_row] = old_row_perm[old_row];
            }

            pivot_cols = sorted_pivot_cols;
        } else {
            // No reordering needed, just sort pivot_cols
            pivot_cols.sort_unstable();
        }
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
    fn test_blocked_rref_boundary_full_rank_and_deficient() {
        for n in [0usize, 1, 63, 64, 65, 128, 129] {
            let full = boundary_full_rank(n);
            let full_prod = rref(&full, false);
            let full_baseline = rref_with_block_size_for_test(&full, false, 1);
            assert_eq!(full_prod, full_baseline, "full-rank boundary n={n}");
            assert_eq!(full_prod.rank, n, "full-rank boundary rank n={n}");

            let deficient = boundary_rank_deficient(n);
            let deficient_prod = rref(&deficient, false);
            let deficient_baseline = rref_with_block_size_for_test(&deficient, false, 1);
            assert_eq!(
                deficient_prod, deficient_baseline,
                "rank-deficient boundary n={n}"
            );
            assert_eq!(
                deficient_prod.rank,
                n.div_ceil(2),
                "rank-deficient boundary rank n={n}"
            );
        }
    }

    fn boundary_full_rank(n: usize) -> BitMatrix {
        let mut matrix = BitMatrix::identity(n);
        for r in 0..n {
            let mut c = r + 1;
            while c < n {
                if ((r * 17 + c * 31 + n) & 3) == 0 {
                    matrix.set(r, c, true);
                }
                c += 1;
            }
        }
        matrix
    }

    fn boundary_rank_deficient(n: usize) -> BitMatrix {
        let rank = n.div_ceil(2);
        let mut matrix = BitMatrix::zeros(n, n);
        for r in 0..rank {
            matrix.set(r, r, true);
            for c in rank..n {
                if ((r * 13 + c * 7 + n) & 1) == 0 {
                    matrix.set(r, c, true);
                }
            }
        }
        for r in rank..n {
            for c in 0..n {
                if matrix.get(r - rank, c) {
                    matrix.set(r, c, true);
                }
            }
        }
        matrix
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

    #[test]
    fn test_rref_pivot_from_right_preserves_lower_columns_across_words() {
        let mut m = BitMatrix::zeros(2, 65);
        m.set(0, 0, true);
        m.set(0, 64, true);
        m.set(1, 1, true);
        m.set(1, 64, true);

        let result = rref(&m, true);

        assert_eq!(result.rank, 2);
        assert_eq!(result.pivot_cols, vec![1, 64]);
        assert!(
            result.reduced.get(0, 0),
            "right-to-left row elimination must update lower columns in earlier words"
        );
        assert!(result.reduced.get(0, 1));
        assert!(result.reduced.get(1, 0));
        assert!(result.reduced.get(1, 64));
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
