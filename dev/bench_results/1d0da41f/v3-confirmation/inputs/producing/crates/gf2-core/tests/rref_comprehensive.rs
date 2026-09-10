//! Comprehensive property-based tests for RREF implementation.
//!
//! This test suite validates the mathematical properties of reduced row echelon form
//! for both left-to-right and right-to-left pivoting strategies.

use gf2_core::alg::rref::rref;
use gf2_core::matrix::BitMatrix;
use proptest::prelude::*;

/// Generate a random BitMatrix with given dimensions and density
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

/// Check if a matrix is in reduced row echelon form (RREF).
///
/// A matrix is in RREF if:
/// 1. All zero rows are at the bottom
/// 2. The leading 1 (pivot) of each non-zero row is to the right of the leading 1 of the row above
/// 3. Each pivot column contains exactly one 1 (the pivot itself)
fn is_rref(m: &BitMatrix, pivot_cols: &[usize]) -> bool {
    let rows = m.rows();
    let cols = m.cols();

    if pivot_cols.is_empty() {
        // All rows should be zero
        for r in 0..rows {
            for c in 0..cols {
                if m.get(r, c) {
                    return false;
                }
            }
        }
        return true;
    }

    // Check pivot columns are sorted and in range
    for i in 1..pivot_cols.len() {
        if pivot_cols[i] <= pivot_cols[i - 1] {
            return false; // Not strictly increasing
        }
    }

    let rank = pivot_cols.len();

    // Check first 'rank' rows have pivots in the right columns
    for (i, &pivot_col) in pivot_cols.iter().enumerate() {
        if pivot_col >= cols {
            return false;
        }

        // Row i should have a 1 at pivot_col
        if !m.get(i, pivot_col) {
            return false;
        }

        // All other rows should have 0 at pivot_col (reduced form)
        for r in 0..rows {
            if r != i && m.get(r, pivot_col) {
                return false;
            }
        }
    }

    // Check remaining rows (after rank) are all zero
    for r in rank..rows {
        for c in 0..cols {
            if m.get(r, c) {
                return false;
            }
        }
    }

    true
}

/// Check if two matrices span the same row space.
///
/// Two matrices have the same row space if every row of A can be expressed
/// as a linear combination of rows of B, and vice versa.
/// For RREF, this means they should have the same reduced form.
fn same_row_space(a: &BitMatrix, b: &BitMatrix) -> bool {
    if a.rows() != b.rows() || a.cols() != b.cols() {
        return false;
    }

    let rref_a = rref(a, false);
    let rref_b = rref(b, false);

    rref_a.reduced == rref_b.reduced
}

/// Compute H × G^T where H is m×n and G is k×n (so G^T is n×k).
/// Returns an m×k matrix.
fn matmul_with_transpose(h: &BitMatrix, g: &BitMatrix) -> BitMatrix {
    assert_eq!(
        h.cols(),
        g.cols(),
        "Matrices must have same number of columns"
    );

    let m = h.rows();
    let k = g.rows();
    let n = h.cols();

    let mut result = BitMatrix::zeros(m, k);

    for i in 0..m {
        for j in 0..k {
            let mut dot = false;
            for l in 0..n {
                dot ^= h.get(i, l) && g.get(j, l);
            }
            result.set(i, j, dot);
        }
    }

    result
}

// ============================================================================
// Property Tests for RREF Mathematical Properties
// ============================================================================

proptest! {
    /// Property: RREF result should be in valid reduced row echelon form
    #[test]
    fn prop_rref_is_valid_rref_left(
        rows in 1..50usize,
        cols in 1..50usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.3, seed);
        let result = rref(&m, false);

        prop_assert!(
            is_rref(&result.reduced, &result.pivot_cols),
            "Result is not in valid RREF form (left pivoting)"
        );
    }

    /// Property: RREF result should be in valid reduced row echelon form (right pivoting)
    #[test]
    fn prop_rref_is_valid_rref_right(
        rows in 1..50usize,
        cols in 1..50usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.3, seed);
        let result = rref(&m, true);

        prop_assert!(
            is_rref(&result.reduced, &result.pivot_cols),
            "Result is not in valid RREF form (right pivoting)"
        );
    }

    /// Property: Rank must be at most min(rows, cols)
    #[test]
    fn prop_rref_rank_bounded_left(
        rows in 1..50usize,
        cols in 1..50usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.5, seed);
        let result = rref(&m, false);

        prop_assert!(result.rank <= rows.min(cols));
        prop_assert_eq!(result.pivot_cols.len(), result.rank);
    }

    /// Property: Rank must be at most min(rows, cols) (right pivoting)
    #[test]
    fn prop_rref_rank_bounded_right(
        rows in 1..50usize,
        cols in 1..50usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.5, seed);
        let result = rref(&m, true);

        prop_assert!(result.rank <= rows.min(cols));
        prop_assert_eq!(result.pivot_cols.len(), result.rank);
    }

    /// Property: RREF is idempotent - RREF(RREF(M)) = RREF(M)
    #[test]
    fn prop_rref_idempotent_left(
        rows in 1..30usize,
        cols in 1..30usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.4, seed);

        let result1 = rref(&m, false);
        let result2 = rref(&result1.reduced, false);

        prop_assert_eq!(result1.reduced, result2.reduced, "RREF is not idempotent (left)");
        prop_assert_eq!(result1.rank, result2.rank);
    }

    /// Property: RREF is idempotent (right pivoting)
    #[test]
    fn prop_rref_idempotent_right(
        rows in 1..30usize,
        cols in 1..30usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.4, seed);

        let result1 = rref(&m, true);
        let result2 = rref(&result1.reduced, true);

        prop_assert_eq!(result1.reduced, result2.reduced, "RREF is not idempotent (right)");
        prop_assert_eq!(result1.rank, result2.rank);
    }

    /// Property: RREF preserves row space (same span)
    #[test]
    fn prop_rref_preserves_row_space_left(
        rows in 2..30usize,
        cols in 2..30usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.4, seed);
        let result = rref(&m, false);

        prop_assert!(
            same_row_space(&m, &result.reduced),
            "RREF does not preserve row space (left)"
        );
    }

    /// Property: RREF preserves row space (right pivoting)
    #[test]
    fn prop_rref_preserves_row_space_right(
        rows in 2..30usize,
        cols in 2..30usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.4, seed);
        let result = rref(&m, true);

        prop_assert!(
            same_row_space(&m, &result.reduced),
            "RREF does not preserve row space (right)"
        );
    }

    /// Property: Identity matrix should have full rank
    #[test]
    fn prop_identity_full_rank_left(n in 1..50usize) {
        let id = BitMatrix::identity(n);
        let result = rref(&id, false);

        prop_assert_eq!(result.rank, n, "Identity matrix should have full rank");
        prop_assert_eq!(result.reduced, id, "Identity RREF should be itself");
    }

    /// Property: Identity matrix should have full rank (right pivoting)
    #[test]
    fn prop_identity_full_rank_right(n in 1..50usize) {
        let id = BitMatrix::identity(n);
        let result = rref(&id, true);

        prop_assert_eq!(result.rank, n, "Identity matrix should have full rank (right)");
        prop_assert_eq!(result.reduced, id, "Identity RREF should be itself (right)");
    }

    /// Property: Zero matrix should have rank 0
    #[test]
    fn prop_zero_matrix_rank_zero_left(
        rows in 1..50usize,
        cols in 1..50usize
    ) {
        let zero = BitMatrix::zeros(rows, cols);
        let result = rref(&zero, false);

        prop_assert_eq!(result.rank, 0, "Zero matrix should have rank 0");
        prop_assert_eq!(result.reduced, zero, "Zero matrix RREF should be itself");
    }

    /// Property: Zero matrix should have rank 0 (right pivoting)
    #[test]
    fn prop_zero_matrix_rank_zero_right(
        rows in 1..50usize,
        cols in 1..50usize
    ) {
        let zero = BitMatrix::zeros(rows, cols);
        let result = rref(&zero, true);

        prop_assert_eq!(result.rank, 0, "Zero matrix should have rank 0 (right)");
        prop_assert_eq!(result.reduced, zero, "Zero matrix RREF should be itself (right)");
    }

    /// Property: Both pivoting strategies should produce same rank
    #[test]
    fn prop_pivot_strategies_same_rank(
        rows in 1..40usize,
        cols in 1..40usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.4, seed);

        let left_result = rref(&m, false);
        let right_result = rref(&m, true);

        prop_assert_eq!(
            left_result.rank,
            right_result.rank,
            "Left and right pivoting should produce same rank"
        );
    }

    /// Property: Both pivoting strategies preserve row space
    #[test]
    fn prop_pivot_strategies_same_row_space(
        rows in 2..30usize,
        cols in 2..30usize,
        seed in any::<u64>()
    ) {
        let m = random_matrix(rows, cols, 0.4, seed);

        let left_result = rref(&m, false);
        let right_result = rref(&m, true);

        prop_assert!(
            same_row_space(&left_result.reduced, &right_result.reduced),
            "Both pivoting strategies should preserve same row space"
        );
    }
}

// ============================================================================
// Property Tests for Generator Matrix from Parity Check Matrix
// ============================================================================

proptest! {
    /// Property: For H in systematic form [A | I_m], the generator G = [I_k | -A^T]
    /// should satisfy H × G^T = 0
    #[test]
    fn prop_systematic_generator_orthogonality_left(
        m in 3..20usize,  // parity check rows
        k in 3..20usize,  // information bits
        seed in any::<u64>()
    ) {
        let n = m + k; // codeword length

        // Build H in systematic form: H = [A | I_m] where A is m×k
        let mut h = BitMatrix::zeros(m, n);

        // Fill A part with random values
        let a = random_matrix(m, k, 0.3, seed);
        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
        }

        // Set I_m part (identity in rightmost m columns)
        for i in 0..m {
            h.set(i, k + i, true);
        }

        // Build generator G = [I_k | -A^T] = [I_k | A^T] in GF(2)
        let mut g = BitMatrix::zeros(k, n);

        // Set I_k part
        for i in 0..k {
            g.set(i, i, true);
        }

        // Set A^T part (parity bits)
        for r in 0..k {
            for c in 0..m {
                g.set(r, k + c, a.get(c, r)); // Transpose
            }
        }

        // Verify H × G^T = 0
        let product = matmul_with_transpose(&h, &g);

        for r in 0..m {
            for c in 0..k {
                prop_assert!(
                    !product.get(r, c),
                    "H × G^T must be zero matrix, but found 1 at ({}, {})", r, c
                );
            }
        }
    }

    /// Property: For arbitrary H, computing systematic form via RREF
    /// should produce G such that H × G^T = 0
    #[test]
    fn prop_rref_generator_orthogonality_left(
        m in 5..15usize,
        k in 5..15usize,
        seed in any::<u64>()
    ) {
        let n = m + k;

        // Create a random full-rank parity check matrix
        // We'll create it by building H = [A | I_m] to ensure it has rank m
        let mut h = BitMatrix::zeros(m, n);

        let a = random_matrix(m, k, 0.3, seed);
        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
            // Set identity in last m columns
            h.set(r, k + r, true);
        }

        // Use RREF to convert H to systematic form
        let rref_result = rref(&h, false);

        // H should have full rank m
        prop_assert_eq!(rref_result.rank, m, "H should have full rank");

        // Build generator from RREF result
        // If RREF gives us [B | I_m], then G = [I_k | -B^T]
        // We need to extract B (the non-identity part)

        let h_sys = &rref_result.reduced;

        // Find which columns are pivot columns (should be rightmost m)
        let pivot_set: std::collections::HashSet<_> = rref_result.pivot_cols.iter().copied().collect();

        // Non-pivot columns form the information set
        let info_cols: Vec<_> = (0..n).filter(|c| !pivot_set.contains(c)).collect();
        prop_assert_eq!(info_cols.len(), k, "Should have k information columns");

        // Build generator matrix G (k × n)
        let mut g = BitMatrix::zeros(k, n);

        // Set identity part in info_cols positions
        for (i, &col) in info_cols.iter().enumerate() {
            g.set(i, col, true);
        }

        // Set parity part from systematic H
        for (g_row, &info_col) in info_cols.iter().enumerate() {
            for (h_row, &pivot_col) in rref_result.pivot_cols.iter().enumerate() {
                // Parity bit at g_row, pivot_col = h_sys[h_row, info_col]
                g.set(g_row, pivot_col, h_sys.get(h_row, info_col));
            }
        }

        // Verify H_sys × G^T = 0
        let product = matmul_with_transpose(h_sys, &g);

        let mut error_positions = Vec::new();
        for r in 0..m {
            for c in 0..k {
                if product.get(r, c) {
                    error_positions.push((r, c));
                }
            }
        }

        prop_assert!(
            error_positions.is_empty(),
            "H × G^T must be zero matrix (left pivot). Found {} errors at positions: {:?}",
            error_positions.len(),
            &error_positions[..error_positions.len().min(10)]
        );
    }

    /// Property: Same orthogonality test with right pivoting
    #[test]
    fn prop_rref_generator_orthogonality_right(
        m in 5..15usize,
        k in 5..15usize,
        seed in any::<u64>()
    ) {
        let n = m + k;

        // Create H = [A | I_m]
        let mut h = BitMatrix::zeros(m, n);
        let a = random_matrix(m, k, 0.3, seed);

        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
            h.set(r, k + r, true);
        }

        // Use RREF with right pivoting
        let rref_result = rref(&h, true);

        prop_assert_eq!(rref_result.rank, m, "H should have full rank (right)");

        let h_sys = &rref_result.reduced;
        let pivot_set: std::collections::HashSet<_> = rref_result.pivot_cols.iter().copied().collect();
        let info_cols: Vec<_> = (0..n).filter(|c| !pivot_set.contains(c)).collect();

        prop_assert_eq!(info_cols.len(), k, "Should have k information columns (right)");

        let mut g = BitMatrix::zeros(k, n);

        for (i, &col) in info_cols.iter().enumerate() {
            g.set(i, col, true);
        }

        for (g_row, &info_col) in info_cols.iter().enumerate() {
            for (h_row, &pivot_col) in rref_result.pivot_cols.iter().enumerate() {
                g.set(g_row, pivot_col, h_sys.get(h_row, info_col));
            }
        }

        let product = matmul_with_transpose(h_sys, &g);

        let mut error_positions = Vec::new();
        for r in 0..m {
            for c in 0..k {
                if product.get(r, c) {
                    error_positions.push((r, c));
                }
            }
        }

        prop_assert!(
            error_positions.is_empty(),
            "H × G^T must be zero matrix (right pivot). Found {} errors at positions: {:?}",
            error_positions.len(),
            &error_positions[..error_positions.len().min(10)]
        );
    }

    /// Property: Test with sparse matrices (mimicking LDPC structure)
    #[test]
    fn prop_sparse_matrix_orthogonality_left(
        m in 10..25usize,
        k in 10..25usize,
        seed in any::<u64>()
    ) {
        let n = m + k;

        // Create very sparse H (like LDPC: ~1-5% density)
        let mut h = BitMatrix::zeros(m, n);
        let a = random_matrix(m, k, 0.05, seed); // 5% density

        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
            h.set(r, k + r, true);
        }

        let rref_result = rref(&h, false);

        // Only test if full rank (sparse matrices might not be full rank)
        if rref_result.rank == m {
            let h_sys = &rref_result.reduced;
            let pivot_set: std::collections::HashSet<_> = rref_result.pivot_cols.iter().copied().collect();
            let info_cols: Vec<_> = (0..n).filter(|c| !pivot_set.contains(c)).collect();

            if info_cols.len() == k {
                let mut g = BitMatrix::zeros(k, n);

                for (i, &col) in info_cols.iter().enumerate() {
                    g.set(i, col, true);
                }

                for (g_row, &info_col) in info_cols.iter().enumerate() {
                    for (h_row, &pivot_col) in rref_result.pivot_cols.iter().enumerate() {
                        g.set(g_row, pivot_col, h_sys.get(h_row, info_col));
                    }
                }

                let product = matmul_with_transpose(h_sys, &g);

                let mut error_count = 0;
                for r in 0..m {
                    for c in 0..k {
                        if product.get(r, c) {
                            error_count += 1;
                        }
                    }
                }

                prop_assert_eq!(
                    error_count, 0,
                    "H × G^T must be zero for sparse matrix (left pivot)"
                );
            }
        }
    }

    /// Property: Test with sparse matrices and right pivoting
    #[test]
    fn prop_sparse_matrix_orthogonality_right(
        m in 10..25usize,
        k in 10..25usize,
        seed in any::<u64>()
    ) {
        let n = m + k;

        let mut h = BitMatrix::zeros(m, n);
        let a = random_matrix(m, k, 0.05, seed);

        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
            h.set(r, k + r, true);
        }

        let rref_result = rref(&h, true);

        if rref_result.rank == m {
            let h_sys = &rref_result.reduced;
            let pivot_set: std::collections::HashSet<_> = rref_result.pivot_cols.iter().copied().collect();
            let info_cols: Vec<_> = (0..n).filter(|c| !pivot_set.contains(c)).collect();

            if info_cols.len() == k {
                let mut g = BitMatrix::zeros(k, n);

                for (i, &col) in info_cols.iter().enumerate() {
                    g.set(i, col, true);
                }

                for (g_row, &info_col) in info_cols.iter().enumerate() {
                    for (h_row, &pivot_col) in rref_result.pivot_cols.iter().enumerate() {
                        g.set(g_row, pivot_col, h_sys.get(h_row, info_col));
                    }
                }

                let product = matmul_with_transpose(h_sys, &g);

                let mut error_count = 0;
                for r in 0..m {
                    for c in 0..k {
                        if product.get(r, c) {
                            error_count += 1;
                        }
                    }
                }

                prop_assert_eq!(
                    error_count, 0,
                    "H × G^T must be zero for sparse matrix (right pivot)"
                );
            }
        }
    }
}

// ============================================================================
// Specific Edge Case Tests
// ============================================================================

#[test]
fn test_word_boundary_matrix() {
    // Test matrix with exactly 64 columns (word boundary)
    let mut m = BitMatrix::zeros(10, 64);

    // Create a pattern
    for i in 0..10 {
        m.set(i, i * 6 % 64, true);
        m.set(i, (i * 6 + 1) % 64, true);
    }

    let result_left = rref(&m, false);
    let result_right = rref(&m, true);

    assert!(is_rref(&result_left.reduced, &result_left.pivot_cols));
    assert!(is_rref(&result_right.reduced, &result_right.pivot_cols));
    assert_eq!(result_left.rank, result_right.rank);
}

#[test]
fn test_just_over_word_boundary() {
    // Test with 65 columns (just over word boundary)
    let mut m = BitMatrix::zeros(10, 65);

    for i in 0..10 {
        m.set(i, i * 6 % 65, true);
        m.set(i, (i * 6 + 2) % 65, true);
    }

    let result_left = rref(&m, false);
    let result_right = rref(&m, true);

    assert!(is_rref(&result_left.reduced, &result_left.pivot_cols));
    assert!(is_rref(&result_right.reduced, &result_right.pivot_cols));
    assert_eq!(result_left.rank, result_right.rank);
}

#[test]
fn test_large_sparse_matrix() {
    // Mimic LDPC-style large sparse matrix
    let m = 100;
    let k = 200;
    let n = m + k;

    let mut h = BitMatrix::zeros(m, n);

    // Add sparse entries (about 3 ones per row, typical for LDPC)
    for r in 0..m {
        h.set(r, r * 3 % k, true);
        h.set(r, (r * 3 + 17) % k, true);
        h.set(r, (r * 3 + 41) % k, true);
        // Identity part
        h.set(r, k + r, true);
    }

    let result_left = rref(&h, false);
    let result_right = rref(&h, true);

    assert_eq!(result_left.rank, m);
    assert_eq!(result_right.rank, m);
    assert!(is_rref(&result_left.reduced, &result_left.pivot_cols));
    assert!(is_rref(&result_right.reduced, &result_right.pivot_cols));
}
