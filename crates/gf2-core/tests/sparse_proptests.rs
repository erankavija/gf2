use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SparseMatrix, SparseMatrixDual};
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

        let s = SparseMatrix::from_dense(&m);
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

        let dual = SparseMatrixDual::from_dense(&m);
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

        let dual = SparseMatrixDual::from_dense(&m);
        let single = SparseMatrix::from_dense(&m);

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

        let dual = SparseMatrixDual::from_dense(&m);
        let y_dual = dual.matvec_transpose(&x);

        // Compare with dense transpose
        let mt = m.transpose();
        let st = SparseMatrix::from_dense(&mt);
        let y_expected = st.matvec(&x);

        prop_assert_eq!(y_dual, y_expected);
    }
}
