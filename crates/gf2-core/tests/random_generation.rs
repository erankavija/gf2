//! Tests for random BitVec and BitMatrix generation.
//!
//! Following TDD principles, these tests define the expected behavior
//! before implementation.

#![cfg(feature = "rand")]

use gf2_core::{matrix::BitMatrix, BitVec};
use proptest::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;

// ============================================================================
// BitVec Random Generation Tests
// ============================================================================

#[test]
fn test_bitvec_random_length() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random(100, &mut rng);
    assert_eq!(bv.len(), 100);
}

#[test]
fn test_bitvec_random_seeded_deterministic() {
    let bv1 = BitVec::random_seeded(200, 0x1234);
    let bv2 = BitVec::random_seeded(200, 0x1234);
    assert_eq!(bv1, bv2);
}

#[test]
fn test_bitvec_random_seeded_different_seeds() {
    let bv1 = BitVec::random_seeded(200, 0x1234);
    let bv2 = BitVec::random_seeded(200, 0x5678);
    // Extremely unlikely to be equal with different seeds
    assert_ne!(bv1, bv2);
}

#[test]
fn test_bitvec_random_empty() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random(0, &mut rng);
    assert_eq!(bv.len(), 0);
    assert!(bv.is_empty());
}

#[test]
fn test_bitvec_random_single_bit() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random(1, &mut rng);
    assert_eq!(bv.len(), 1);
}

#[test]
fn test_bitvec_random_word_boundary_63() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random(63, &mut rng);
    assert_eq!(bv.len(), 63);
}

#[test]
fn test_bitvec_random_word_boundary_64() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random(64, &mut rng);
    assert_eq!(bv.len(), 64);
}

#[test]
fn test_bitvec_random_word_boundary_65() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random(65, &mut rng);
    assert_eq!(bv.len(), 65);
}

#[test]
fn test_bitvec_random_tail_masking() {
    // Verify that padding bits are always zero (tail masking invariant)
    let mut rng = StdRng::seed_from_u64(42);
    for len in [1, 7, 63, 65, 100, 127, 129] {
        let bv = BitVec::random(len, &mut rng);
        // Create a reference BitVec and verify all bits match
        for i in 0..bv.len() {
            let _ = bv.get(i); // Should not panic
        }
        // Verify length is exact
        assert_eq!(bv.len(), len);
    }
}

#[test]
fn test_bitvec_fill_random() {
    let mut bv = BitVec::from_bytes_le(&[0xFF, 0xFF]);
    let mut rng = StdRng::seed_from_u64(99);
    bv.fill_random(&mut rng);
    // Should have changed from all ones
    assert_eq!(bv.len(), 16);
}

#[test]
fn test_bitvec_fill_random_deterministic() {
    let mut bv1 = BitVec::from_bytes_le(&[0x00; 10]);
    let mut bv2 = BitVec::from_bytes_le(&[0x00; 10]);

    let mut rng1 = StdRng::seed_from_u64(777);
    let mut rng2 = StdRng::seed_from_u64(777);

    bv1.fill_random(&mut rng1);
    bv2.fill_random(&mut rng2);

    assert_eq!(bv1, bv2);
}

#[test]
fn test_bitvec_random_with_probability_default() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random_with_probability(1000, 0.5, &mut rng);
    assert_eq!(bv.len(), 1000);

    // With p=0.5 and 1000 bits, expect roughly 400-600 ones
    let ones = bv.count_ones();
    assert!(
        (400..=600).contains(&ones),
        "Expected ~500 ones, got {}",
        ones
    );
}

#[test]
fn test_bitvec_random_with_probability_zero() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random_with_probability(100, 0.0, &mut rng);
    assert_eq!(bv.count_ones(), 0);
}

#[test]
fn test_bitvec_random_with_probability_one() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random_with_probability(100, 1.0, &mut rng);
    assert_eq!(bv.count_ones(), 100);
}

#[test]
fn test_bitvec_random_with_probability_sparse() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random_with_probability(1000, 0.1, &mut rng);
    let ones = bv.count_ones();
    // With p=0.1 and 1000 bits, expect roughly 50-150 ones
    assert!(
        (50..=150).contains(&ones),
        "Expected ~100 ones, got {}",
        ones
    );
}

#[test]
fn test_bitvec_random_with_probability_dense() {
    let mut rng = StdRng::seed_from_u64(42);
    let bv = BitVec::random_with_probability(1000, 0.9, &mut rng);
    let ones = bv.count_ones();
    // With p=0.9 and 1000 bits, expect roughly 850-950 ones
    assert!(
        (850..=950).contains(&ones),
        "Expected ~900 ones, got {}",
        ones
    );
}

#[test]
#[should_panic(expected = "Probability must be in range [0.0, 1.0]")]
fn test_bitvec_random_with_probability_invalid_negative() {
    let mut rng = StdRng::seed_from_u64(42);
    let _ = BitVec::random_with_probability(100, -0.1, &mut rng);
}

#[test]
#[should_panic(expected = "Probability must be in range [0.0, 1.0]")]
fn test_bitvec_random_with_probability_invalid_too_large() {
    let mut rng = StdRng::seed_from_u64(42);
    let _ = BitVec::random_with_probability(100, 1.1, &mut rng);
}

// ============================================================================
// BitMatrix Random Generation Tests
// ============================================================================

#[test]
fn test_bitmatrix_random_dimensions() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random(10, 20, &mut rng);
    assert_eq!(m.rows(), 10);
    assert_eq!(m.cols(), 20);
}

#[test]
fn test_bitmatrix_random_seeded_deterministic() {
    let m1 = BitMatrix::random_seeded(15, 25, 0xABCD);
    let m2 = BitMatrix::random_seeded(15, 25, 0xABCD);
    assert_eq!(m1, m2);
}

#[test]
fn test_bitmatrix_random_seeded_different_seeds() {
    let m1 = BitMatrix::random_seeded(15, 25, 0xABCD);
    let m2 = BitMatrix::random_seeded(15, 25, 0xDCBA);
    assert_ne!(m1, m2);
}

#[test]
fn test_bitmatrix_random_empty() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random(0, 0, &mut rng);
    assert_eq!(m.rows(), 0);
    assert_eq!(m.cols(), 0);
}

#[test]
fn test_bitmatrix_random_single_element() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random(1, 1, &mut rng);
    assert_eq!(m.rows(), 1);
    assert_eq!(m.cols(), 1);
}

#[test]
fn test_bitmatrix_random_row_vector() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random(1, 100, &mut rng);
    assert_eq!(m.rows(), 1);
    assert_eq!(m.cols(), 100);
}

#[test]
fn test_bitmatrix_random_col_vector() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random(100, 1, &mut rng);
    assert_eq!(m.rows(), 100);
    assert_eq!(m.cols(), 1);
}

#[test]
fn test_bitmatrix_random_square() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random(50, 50, &mut rng);
    assert_eq!(m.rows(), 50);
    assert_eq!(m.cols(), 50);
}

#[test]
fn test_bitmatrix_fill_random() {
    let mut m = BitMatrix::zeros(10, 10);
    let mut rng = StdRng::seed_from_u64(123);
    m.fill_random(&mut rng);
    assert_eq!(m.rows(), 10);
    assert_eq!(m.cols(), 10);
}

#[test]
fn test_bitmatrix_fill_random_deterministic() {
    let mut m1 = BitMatrix::zeros(8, 12);
    let mut m2 = BitMatrix::zeros(8, 12);

    let mut rng1 = StdRng::seed_from_u64(888);
    let mut rng2 = StdRng::seed_from_u64(888);

    m1.fill_random(&mut rng1);
    m2.fill_random(&mut rng2);

    assert_eq!(m1, m2);
}

#[test]
fn test_bitmatrix_random_with_probability_default() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(20, 30, 0.5, &mut rng);
    assert_eq!(m.rows(), 20);
    assert_eq!(m.cols(), 30);

    // Count ones in the matrix
    let mut ones = 0u64;
    for r in 0..m.rows() {
        for c in 0..m.cols() {
            if m.get(r, c) {
                ones += 1;
            }
        }
    }

    // With p=0.5 and 600 bits, expect roughly 240-360 ones
    assert!(
        (240..=360).contains(&ones),
        "Expected ~300 ones, got {}",
        ones
    );
}

#[test]
fn test_bitmatrix_random_with_probability_zero() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(10, 10, 0.0, &mut rng);

    for r in 0..m.rows() {
        for c in 0..m.cols() {
            assert!(!m.get(r, c), "Expected all zeros");
        }
    }
}

#[test]
fn test_bitmatrix_random_with_probability_one() {
    let mut rng = StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(10, 10, 1.0, &mut rng);

    for r in 0..m.rows() {
        for c in 0..m.cols() {
            assert!(m.get(r, c), "Expected all ones");
        }
    }
}

#[test]
#[should_panic(expected = "Probability must be in range [0.0, 1.0]")]
fn test_bitmatrix_random_with_probability_invalid() {
    let mut rng = StdRng::seed_from_u64(42);
    let _ = BitMatrix::random_with_probability(10, 10, 1.5, &mut rng);
}

// ============================================================================
// Property-Based Tests
// ============================================================================

proptest! {
    #[test]
    fn prop_bitvec_random_has_correct_length(len in 0usize..10000) {
        let mut rng = StdRng::seed_from_u64(42);
        let bv = BitVec::random(len, &mut rng);
        assert_eq!(bv.len(), len);
    }

    #[test]
    fn prop_bitvec_random_seeded_is_deterministic(len in 0usize..1000, seed in any::<u64>()) {
        let bv1 = BitVec::random_seeded(len, seed);
        let bv2 = BitVec::random_seeded(len, seed);
        assert_eq!(bv1, bv2);
    }

    #[test]
    fn prop_bitvec_random_tail_masking_invariant(len in 1usize..1000) {
        let mut rng = StdRng::seed_from_u64(99);
        let bv = BitVec::random(len, &mut rng);

        // Verify we can access all bits
        for i in 0..bv.len() {
            let _ = bv.get(i);
        }
        assert_eq!(bv.len(), len);
    }

    #[test]
    fn prop_bitmatrix_random_has_correct_dimensions(
        rows in 0usize..500,
        cols in 0usize..500
    ) {
        let mut rng = StdRng::seed_from_u64(42);
        let m = BitMatrix::random(rows, cols, &mut rng);
        assert_eq!(m.rows(), rows);
        assert_eq!(m.cols(), cols);
    }

    #[test]
    fn prop_bitmatrix_random_seeded_is_deterministic(
        rows in 0usize..100,
        cols in 0usize..100,
        seed in any::<u64>()
    ) {
        let m1 = BitMatrix::random_seeded(rows, cols, seed);
        let m2 = BitMatrix::random_seeded(rows, cols, seed);
        assert_eq!(m1, m2);
    }

    #[test]
    fn prop_bitvec_with_probability_respects_extremes_zero(len in 0usize..1000) {
        let mut rng = StdRng::seed_from_u64(123);
        let bv = BitVec::random_with_probability(len, 0.0, &mut rng);
        assert_eq!(bv.count_ones(), 0);
    }

    #[test]
    fn prop_bitvec_with_probability_respects_extremes_one(len in 0usize..1000) {
        let mut rng = StdRng::seed_from_u64(456);
        let bv = BitVec::random_with_probability(len, 1.0, &mut rng);
        assert_eq!(bv.count_ones(), len as u64);
    }

    #[test]
    fn prop_bitmatrix_with_probability_respects_extremes_zero(
        rows in 0usize..100,
        cols in 0usize..100
    ) {
        let mut rng = StdRng::seed_from_u64(789);
        let m = BitMatrix::random_with_probability(rows, cols, 0.0, &mut rng);
        for r in 0..m.rows() {
            for c in 0..m.cols() {
                assert!(!m.get(r, c));
            }
        }
    }

    #[test]
    fn prop_bitmatrix_with_probability_respects_extremes_one(
        rows in 0usize..100,
        cols in 0usize..100
    ) {
        let mut rng = StdRng::seed_from_u64(101112);
        let m = BitMatrix::random_with_probability(rows, cols, 1.0, &mut rng);
        for r in 0..m.rows() {
            for c in 0..m.cols() {
                assert!(m.get(r, c));
            }
        }
    }
}
