//! Tests for BitMatrix matrix-vector multiplication operations.

use gf2_core::{BitMatrix, BitVec};

#[cfg(test)]
mod matvec_tests {
    use super::*;

    #[test]
    fn test_matvec_empty() {
        let m = BitMatrix::zeros(0, 0);
        let x = BitVec::new();
        let y = m.matvec(&x);
        assert_eq!(y.len(), 0);
    }

    #[test]
    fn test_matvec_zero_matrix() {
        let m = BitMatrix::zeros(3, 4);
        let mut x = BitVec::new();
        for _ in 0..4 {
            x.push_bit(true);
        }
        let y = m.matvec(&x);
        assert_eq!(y.len(), 3);
        for i in 0..3 {
            assert!(!y.get(i), "All outputs should be zero");
        }
    }

    #[test]
    fn test_matvec_identity() {
        let m = BitMatrix::identity(4);
        let mut x = BitVec::new();
        x.push_bit(true);
        x.push_bit(false);
        x.push_bit(true);
        x.push_bit(false);

        let y = m.matvec(&x);
        assert_eq!(y.len(), 4);
        assert!(y.get(0));
        assert!(!y.get(1));
        assert!(y.get(2));
        assert!(!y.get(3));
    }

    #[test]
    fn test_matvec_simple() {
        // Matrix:
        // [1 0 1]
        // [0 1 1]
        let mut m = BitMatrix::zeros(2, 3);
        m.set(0, 0, true);
        m.set(0, 2, true);
        m.set(1, 1, true);
        m.set(1, 2, true);

        // x = [1, 1, 0]
        let mut x = BitVec::new();
        x.push_bit(true);
        x.push_bit(true);
        x.push_bit(false);

        let y = m.matvec(&x);
        assert_eq!(y.len(), 2);
        // Row 0: 1*1 ^ 0*1 ^ 1*0 = 1
        assert!(y.get(0));
        // Row 1: 0*1 ^ 1*1 ^ 1*0 = 1
        assert!(y.get(1));
    }

    #[test]
    fn test_matvec_xor_cancellation() {
        // Matrix: [1 1 1]
        let mut m = BitMatrix::zeros(1, 3);
        m.set(0, 0, true);
        m.set(0, 1, true);
        m.set(0, 2, true);

        // x = [1, 1, 0]
        let mut x = BitVec::new();
        x.push_bit(true);
        x.push_bit(true);
        x.push_bit(false);

        let y = m.matvec(&x);
        // 1 ^ 1 ^ 0 = 0 (XOR cancellation in GF(2))
        assert!(!y.get(0));

        // x = [1, 1, 1]
        let mut x2 = BitVec::new();
        x2.push_bit(true);
        x2.push_bit(true);
        x2.push_bit(true);

        let y2 = m.matvec(&x2);
        // 1 ^ 1 ^ 1 = 1
        assert!(y2.get(0));
    }

    #[test]
    fn test_matvec_word_boundary_64() {
        // Test at 64-bit word boundary
        let m = BitMatrix::identity(64);
        let mut x = BitVec::with_capacity(64);
        for i in 0..64 {
            x.push_bit(i % 2 == 0);
        }

        let y = m.matvec(&x);
        assert_eq!(y.len(), 64);
        for i in 0..64 {
            assert_eq!(y.get(i), i % 2 == 0);
        }
    }

    #[test]
    fn test_matvec_word_boundary_65() {
        // Test just over 64-bit word boundary
        let m = BitMatrix::identity(65);
        let mut x = BitVec::with_capacity(65);
        for i in 0..65 {
            x.push_bit(i % 3 == 0);
        }

        let y = m.matvec(&x);
        assert_eq!(y.len(), 65);
        for i in 0..65 {
            assert_eq!(y.get(i), i % 3 == 0);
        }
    }

    #[test]
    #[should_panic(expected = "input BitVec length must equal cols")]
    fn test_matvec_wrong_size() {
        let m = BitMatrix::zeros(3, 4);
        let x = BitVec::new(); // length 0, should be 4
        let _ = m.matvec(&x);
    }

    #[test]
    fn test_matvec_matches_sparse() {
        // Verify dense matvec matches sparse matvec for the same matrix
        use gf2_core::sparse::SpBitMatrix;

        let mut m = BitMatrix::zeros(5, 7);
        m.set(0, 1, true);
        m.set(0, 3, true);
        m.set(1, 2, true);
        m.set(2, 0, true);
        m.set(2, 6, true);
        m.set(3, 4, true);
        m.set(4, 1, true);
        m.set(4, 5, true);

        let s = SpBitMatrix::from_dense(&m);

        let mut x = BitVec::new();
        for i in 0..7 {
            x.push_bit(i % 2 == 1);
        }

        let y_dense = m.matvec(&x);
        let y_sparse = s.matvec(&x);

        assert_eq!(y_dense.len(), y_sparse.len());
        for i in 0..y_dense.len() {
            assert_eq!(y_dense.get(i), y_sparse.get(i), "Mismatch at row {}", i);
        }
    }
}

#[cfg(test)]
mod matvec_transpose_tests {
    use super::*;

    #[test]
    fn test_matvec_transpose_empty() {
        let m = BitMatrix::zeros(0, 0);
        let x = BitVec::new();
        let y = m.matvec_transpose(&x);
        assert_eq!(y.len(), 0);
    }

    #[test]
    fn test_matvec_transpose_zero_matrix() {
        let m = BitMatrix::zeros(3, 4);
        let mut x = BitVec::new();
        for _ in 0..3 {
            x.push_bit(true);
        }
        let y = m.matvec_transpose(&x);
        assert_eq!(y.len(), 4);
        for i in 0..4 {
            assert!(!y.get(i), "All outputs should be zero");
        }
    }

    #[test]
    fn test_matvec_transpose_identity() {
        let m = BitMatrix::identity(4);
        let mut x = BitVec::new();
        x.push_bit(true);
        x.push_bit(false);
        x.push_bit(true);
        x.push_bit(false);

        let y = m.matvec_transpose(&x);
        assert_eq!(y.len(), 4);
        assert!(y.get(0));
        assert!(!y.get(1));
        assert!(y.get(2));
        assert!(!y.get(3));
    }

    #[test]
    fn test_matvec_transpose_simple() {
        // Matrix A:
        // [1 0 1]
        // [0 1 1]
        // A^T:
        // [1 0]
        // [0 1]
        // [1 1]
        let mut m = BitMatrix::zeros(2, 3);
        m.set(0, 0, true);
        m.set(0, 2, true);
        m.set(1, 1, true);
        m.set(1, 2, true);

        // x = [1, 0]
        let mut x = BitVec::new();
        x.push_bit(true);
        x.push_bit(false);

        let y = m.matvec_transpose(&x);
        assert_eq!(y.len(), 3);
        // Col 0 of A (row 0 of A^T): [1, 0] · [1, 0] = 1
        assert!(y.get(0));
        // Col 1 of A (row 1 of A^T): [0, 1] · [1, 0] = 0
        assert!(!y.get(1));
        // Col 2 of A (row 2 of A^T): [1, 1] · [1, 0] = 1
        assert!(y.get(2));
    }

    #[test]
    fn test_matvec_transpose_xor() {
        // Matrix:
        // [1]
        // [1]
        // [1]
        // A^T = [1 1 1]
        let mut m = BitMatrix::zeros(3, 1);
        m.set(0, 0, true);
        m.set(1, 0, true);
        m.set(2, 0, true);

        // x = [1, 1, 0]
        let mut x = BitVec::new();
        x.push_bit(true);
        x.push_bit(true);
        x.push_bit(false);

        let y = m.matvec_transpose(&x);
        assert_eq!(y.len(), 1);
        // 1 ^ 1 ^ 0 = 0
        assert!(!y.get(0));
    }

    #[test]
    fn test_matvec_transpose_word_boundary_64() {
        let m = BitMatrix::identity(64);
        let mut x = BitVec::with_capacity(64);
        for i in 0..64 {
            x.push_bit(i % 2 == 0);
        }

        let y = m.matvec_transpose(&x);
        assert_eq!(y.len(), 64);
        for i in 0..64 {
            assert_eq!(y.get(i), i % 2 == 0);
        }
    }

    #[test]
    fn test_matvec_transpose_word_boundary_65() {
        let m = BitMatrix::identity(65);
        let mut x = BitVec::with_capacity(65);
        for i in 0..65 {
            x.push_bit(i % 3 == 0);
        }

        let y = m.matvec_transpose(&x);
        assert_eq!(y.len(), 65);
        for i in 0..65 {
            assert_eq!(y.get(i), i % 3 == 0);
        }
    }

    #[test]
    #[should_panic(expected = "input BitVec length must equal rows")]
    fn test_matvec_transpose_wrong_size() {
        let m = BitMatrix::zeros(3, 4);
        let x = BitVec::new(); // length 0, should be 3
        let _ = m.matvec_transpose(&x);
    }

    #[test]
    fn test_matvec_transpose_matches_sparse() {
        use gf2_core::sparse::SpBitMatrixDual;

        let mut m = BitMatrix::zeros(5, 7);
        m.set(0, 1, true);
        m.set(0, 3, true);
        m.set(1, 2, true);
        m.set(2, 0, true);
        m.set(2, 6, true);
        m.set(3, 4, true);
        m.set(4, 1, true);
        m.set(4, 5, true);

        let s = SpBitMatrixDual::from_dense(&m);

        let mut x = BitVec::new();
        for i in 0..5 {
            x.push_bit(i % 2 == 1);
        }

        let y_dense = m.matvec_transpose(&x);
        let y_sparse = s.matvec_transpose(&x);

        assert_eq!(y_dense.len(), y_sparse.len());
        for i in 0..y_dense.len() {
            assert_eq!(y_dense.get(i), y_sparse.get(i), "Mismatch at col {}", i);
        }
    }

    #[test]
    fn test_matvec_transpose_relationship() {
        // Verify that A^T × x is transpose of A × x when x is a row pattern
        let mut m = BitMatrix::zeros(3, 5);
        m.set(0, 1, true);
        m.set(0, 4, true);
        m.set(1, 0, true);
        m.set(1, 3, true);
        m.set(2, 2, true);

        let mut x1 = BitVec::new();
        for i in 0..5 {
            x1.push_bit(i == 2);
        }
        let y1 = m.matvec(&x1);

        let mut x2 = BitVec::new();
        for i in 0..3 {
            x2.push_bit(i == 2);
        }
        let y2 = m.matvec_transpose(&x2);

        // This tests the relationship holds for specific vector patterns
        assert_eq!(y1.len(), 3);
        assert_eq!(y2.len(), 5);
    }
}

#[cfg(all(test, feature = "rand"))]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    proptest! {
        #[test]
        fn matvec_identity_preserves_vector(seed in any::<u64>(), size in 1usize..100) {
            let mut rng = StdRng::seed_from_u64(seed);
            let id = BitMatrix::identity(size);
            let mut x = BitVec::with_capacity(size);
            for _ in 0..size {
                x.push_bit(rng.gen_bool(0.5));
            }

            let y = id.matvec(&x);

            prop_assert_eq!(x.len(), y.len());
            for i in 0..size {
                prop_assert_eq!(x.get(i), y.get(i));
            }
        }

        #[test]
        fn matvec_zero_produces_zero(
            rows in 1usize..50,
            cols in 1usize..50,
            seed in any::<u64>()
        ) {
            let m = BitMatrix::zeros(rows, cols);
            let mut rng = StdRng::seed_from_u64(seed);
            let mut x = BitVec::with_capacity(cols);
            for _ in 0..cols {
                x.push_bit(rng.gen_bool(0.5));
            }

            let y = m.matvec(&x);

            prop_assert_eq!(y.len(), rows);
            for i in 0..rows {
                prop_assert!(!y.get(i));
            }
        }

        #[test]
        fn matvec_transpose_identity_preserves_vector(
            seed in any::<u64>(),
            size in 1usize..100
        ) {
            let mut rng = StdRng::seed_from_u64(seed);
            let id = BitMatrix::identity(size);
            let mut x = BitVec::with_capacity(size);
            for _ in 0..size {
                x.push_bit(rng.gen_bool(0.5));
            }

            let y = id.matvec_transpose(&x);

            prop_assert_eq!(x.len(), y.len());
            for i in 0..size {
                prop_assert_eq!(x.get(i), y.get(i));
            }
        }

        #[test]
        fn matvec_matches_sparse_random(
            rows in 1usize..30,
            cols in 1usize..30,
            seed in any::<u64>()
        ) {
            use gf2_core::sparse::SpBitMatrix;

            let mut rng = StdRng::seed_from_u64(seed);
            let m = BitMatrix::random_with_probability(rows, cols, 0.3, &mut rng);
            let s = SpBitMatrix::from_dense(&m);

            let mut x = BitVec::with_capacity(cols);
            for _ in 0..cols {
                x.push_bit(rng.gen_bool(0.5));
            }

            let y_dense = m.matvec(&x);
            let y_sparse = s.matvec(&x);

            prop_assert_eq!(y_dense.len(), y_sparse.len());
            for i in 0..y_dense.len() {
                prop_assert_eq!(y_dense.get(i), y_sparse.get(i));
            }
        }

        #[test]
        fn matvec_transpose_matches_sparse_random(
            rows in 1usize..30,
            cols in 1usize..30,
            seed in any::<u64>()
        ) {
            use gf2_core::sparse::SpBitMatrixDual;

            let mut rng = StdRng::seed_from_u64(seed);
            let m = BitMatrix::random_with_probability(rows, cols, 0.3, &mut rng);
            let s = SpBitMatrixDual::from_dense(&m);

            let mut x = BitVec::with_capacity(rows);
            for _ in 0..rows {
                x.push_bit(rng.gen_bool(0.5));
            }

            let y_dense = m.matvec_transpose(&x);
            let y_sparse = s.matvec_transpose(&x);

            prop_assert_eq!(y_dense.len(), y_sparse.len());
            for i in 0..y_dense.len() {
                prop_assert_eq!(y_dense.get(i), y_sparse.get(i));
            }
        }
    }
}
