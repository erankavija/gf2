use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SparseMatrix;
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
        // Constrain to small sizes for test speed; low density typical for LDPC
        // Use deterministic RNG for reproducibility
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let m = BitMatrix::random_with_probability(rows, cols, p, &mut rng);

        let s = SparseMatrix::from_dense(&m);
        let d = s.to_dense();
        prop_assert_eq!(d, m);
    }
}
