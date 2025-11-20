use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};
use gf2_core::BitVec;
use proptest::prelude::*;
use rand::SeedableRng;

proptest! {
    #[test]
    fn sparse_dense_roundtrip_equivalence(
        rows in 0usize..8,
        cols in 0usize..8,
        p in 0.0f64..0.15,
        seed in any::<u64>(),
    ) {
        // Constrain to small sizes for test speed; low density typical for sparse matrices
        // Use deterministic RNG for reproducibility
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let m = BitMatrix::random_with_probability(rows, cols, p, &mut rng);

        let s = SpBitMatrix::from_dense(&m);
        let d = s.to_dense();
        prop_assert_eq!(d, m);
    }

    #[test]
    fn dual_roundtrip_equivalence(
        rows in 0usize..8,
        cols in 0usize..8,
        p in 0.0f64..0.15,
        seed in any::<u64>(),
    ) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let m = BitMatrix::random_with_probability(rows, cols, p, &mut rng);

        let dual = SpBitMatrixDual::from_dense(&m);
        let d = dual.to_dense();
        prop_assert_eq!(d, m);
    }

    #[test]
    fn dual_col_iter_matches_transpose(
        rows in 1usize..8,
        cols in 1usize..8,
        p in 0.0f64..0.2,
        seed in any::<u64>(),
    ) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let m = BitMatrix::random_with_probability(rows, cols, p, &mut rng);

        let dual = SpBitMatrixDual::from_dense(&m);
        let single = SpBitMatrix::from_dense(&m);

        // For each column, dual.col_iter should match single.col_iter (via transpose)
        for c in 0..cols {
            let dual_rows: Vec<_> = dual.col_iter(c).collect();
            let single_rows: Vec<_> = single.col_iter(c).into_iter().collect();
            prop_assert_eq!(dual_rows, single_rows, "Column {} mismatch", c);
        }
    }

    #[test]
    fn dual_matvec_transpose_matches_dense(
        rows in 1usize..8,
        cols in 1usize..8,
        p in 0.0f64..0.2,
        seed in any::<u64>(),
    ) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let m = BitMatrix::random_with_probability(rows, cols, p, &mut rng);
        let x = BitVec::random(rows, &mut rng);

        let dual = SpBitMatrixDual::from_dense(&m);
        let y_dual = dual.matvec_transpose(&x);

        // Compare with dense transpose
        let mt = m.transpose();
        let st = SpBitMatrix::from_dense(&mt);
        let y_expected = st.matvec(&x);

        prop_assert_eq!(y_dual, y_expected);
    }

    #[test]
    fn from_coo_deduplicated_unique_count(
        rows in 1usize..8,
        cols in 1usize..8,
        num_edges in 0usize..30,
        seed in any::<u64>(),
    ) {
        use std::collections::HashSet;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        use rand::Rng;

        // Generate random edges (may contain duplicates)
        let mut edges = Vec::new();
        for _ in 0..num_edges {
            let r = rng.gen_range(0..rows);
            let c = rng.gen_range(0..cols);
            edges.push((r, c));
        }

        // Count unique edges
        let unique_edges: HashSet<_> = edges.iter().copied().collect();

        let matrix = SpBitMatrix::from_coo_deduplicated(rows, cols, &edges);

        // nnz should equal number of unique edges
        prop_assert_eq!(matrix.nnz(), unique_edges.len());
    }

    #[test]
    fn from_coo_deduplicated_idempotent(
        rows in 1usize..8,
        cols in 1usize..8,
        num_edges in 1usize..20,
        seed in any::<u64>(),
    ) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        use rand::Rng;

        let mut edges = Vec::new();
        for _ in 0..num_edges {
            let r = rng.gen_range(0..rows);
            let c = rng.gen_range(0..cols);
            edges.push((r, c));
        }

        // Build once
        let m1 = SpBitMatrix::from_coo_deduplicated(rows, cols, &edges);

        // Build twice with duplicated input
        let mut doubled_edges = edges.clone();
        doubled_edges.extend_from_slice(&edges);
        let m2 = SpBitMatrix::from_coo_deduplicated(rows, cols, &doubled_edges);

        // Should produce identical matrices
        prop_assert_eq!(m1.to_dense(), m2.to_dense());
    }

    #[test]
    fn dual_from_coo_deduplicated_consistent(
        rows in 1usize..8,
        cols in 1usize..8,
        num_edges in 0usize..30,
        seed in any::<u64>(),
    ) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        use rand::Rng;

        let mut edges = Vec::new();
        for _ in 0..num_edges {
            let r = rng.gen_range(0..rows);
            let c = rng.gen_range(0..cols);
            edges.push((r, c));
        }

        let dual = SpBitMatrixDual::from_coo_deduplicated(rows, cols, &edges);
        let single = SpBitMatrix::from_coo_deduplicated(rows, cols, &edges);

        // CSR view should match single CSR
        prop_assert_eq!(dual.to_dense(), single.to_dense());

        // Row and column access should be consistent
        for r in 0..rows {
            let dual_row: Vec<_> = dual.row_iter(r).collect();
            let single_row: Vec<_> = single.row_iter(r).collect();
            prop_assert_eq!(dual_row, single_row);
        }
    }
}
