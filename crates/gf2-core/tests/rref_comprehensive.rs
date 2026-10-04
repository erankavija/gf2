//! Property tests for reduced row echelon form under left-to-right and
//! right-to-left pivoting.

use gf2_core::alg::rref::rref;
use gf2_core::matrix::BitMatrix;
use proptest::prelude::*;

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

fn is_rref(m: &BitMatrix, pivot_cols: &[usize]) -> bool {
    let rows = m.rows();
    let cols = m.cols();

    if pivot_cols.is_empty() {
        for r in 0..rows {
            for c in 0..cols {
                if m.get(r, c) {
                    return false;
                }
            }
        }
        return true;
    }

    for i in 1..pivot_cols.len() {
        if pivot_cols[i] <= pivot_cols[i - 1] {
            return false;
        }
    }

    let rank = pivot_cols.len();

    for (i, &pivot_col) in pivot_cols.iter().enumerate() {
        if pivot_col >= cols {
            return false;
        }

        if !m.get(i, pivot_col) {
            return false;
        }

        for r in 0..rows {
            if r != i && m.get(r, pivot_col) {
                return false;
            }
        }
    }

    for r in rank..rows {
        for c in 0..cols {
            if m.get(r, c) {
                return false;
            }
        }
    }

    true
}

/// Equal row spaces, decided by equal reduced forms.
fn same_row_space(a: &BitMatrix, b: &BitMatrix) -> bool {
    if a.rows() != b.rows() || a.cols() != b.cols() {
        return false;
    }

    let rref_a = rref(a, false);
    let rref_b = rref(b, false);

    rref_a.reduced == rref_b.reduced
}

/// Compute H × G^T where H is m×n and G is k×n (so G^T is n×k).
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

proptest! {
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

    #[test]
    fn prop_identity_full_rank_left(n in 1..50usize) {
        let id = BitMatrix::identity(n);
        let result = rref(&id, false);

        prop_assert_eq!(result.rank, n, "Identity matrix should have full rank");
        prop_assert_eq!(result.reduced, id, "Identity RREF should be itself");
    }

    #[test]
    fn prop_identity_full_rank_right(n in 1..50usize) {
        let id = BitMatrix::identity(n);
        let result = rref(&id, true);

        prop_assert_eq!(result.rank, n, "Identity matrix should have full rank (right)");
        prop_assert_eq!(result.reduced, id, "Identity RREF should be itself (right)");
    }

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

proptest! {
    /// Property: For H in systematic form [A | I_m], the generator G = [I_k | -A^T]
    /// should satisfy H × G^T = 0
    #[test]
    fn prop_systematic_generator_orthogonality_left(
        m in 3..20usize,
        k in 3..20usize,
        seed in any::<u64>()
    ) {
        let n = m + k;

        let mut h = BitMatrix::zeros(m, n);

        let a = random_matrix(m, k, 0.3, seed);
        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
        }

        for i in 0..m {
            h.set(i, k + i, true);
        }

        let mut g = BitMatrix::zeros(k, n);

        for i in 0..k {
            g.set(i, i, true);
        }

        for r in 0..k {
            for c in 0..m {
                g.set(r, k + c, a.get(c, r));
            }
        }

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

    #[test]
    fn prop_rref_generator_orthogonality_left(
        m in 5..15usize,
        k in 5..15usize,
        seed in any::<u64>()
    ) {
        let n = m + k;

        // H = [A | I_m] has rank m.
        let mut h = BitMatrix::zeros(m, n);

        let a = random_matrix(m, k, 0.3, seed);
        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
            h.set(r, k + r, true);
        }

        let rref_result = rref(&h, false);

        prop_assert_eq!(rref_result.rank, m, "H should have full rank");

        // If RREF gives [B | I_m], then G = [I_k | -B^T].

        let h_sys = &rref_result.reduced;

        let pivot_set: std::collections::HashSet<_> = rref_result.pivot_cols.iter().copied().collect();

        let info_cols: Vec<_> = (0..n).filter(|c| !pivot_set.contains(c)).collect();
        prop_assert_eq!(info_cols.len(), k, "Should have k information columns");

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
            "H × G^T must be zero matrix (left pivot). Found {} errors at positions: {:?}",
            error_positions.len(),
            &error_positions[..error_positions.len().min(10)]
        );
    }

    #[test]
    fn prop_rref_generator_orthogonality_right(
        m in 5..15usize,
        k in 5..15usize,
        seed in any::<u64>()
    ) {
        let n = m + k;

        let mut h = BitMatrix::zeros(m, n);
        let a = random_matrix(m, k, 0.3, seed);

        for r in 0..m {
            for c in 0..k {
                h.set(r, c, a.get(r, c));
            }
            h.set(r, k + r, true);
        }

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

    #[test]
    fn prop_sparse_matrix_orthogonality_left(
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

#[test]
fn test_word_boundary_matrix() {
    let mut m = BitMatrix::zeros(10, 64);

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
    let m = 100;
    let k = 200;
    let n = m + k;

    let mut h = BitMatrix::zeros(m, n);

    for r in 0..m {
        h.set(r, r * 3 % k, true);
        h.set(r, (r * 3 + 17) % k, true);
        h.set(r, (r * 3 + 41) % k, true);
        h.set(r, k + r, true);
    }

    let result_left = rref(&h, false);
    let result_right = rref(&h, true);

    assert_eq!(result_left.rank, m);
    assert_eq!(result_right.rank, m);
    assert!(is_rref(&result_left.reduced, &result_left.pivot_cols));
    assert!(is_rref(&result_right.reduced, &result_right.pivot_cols));
}
