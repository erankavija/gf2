//! Tests for matrix multiplication over GF(2).

use gf2::alg::m4rm::multiply;
use gf2::matrix::BitMatrix;

/// Naive reference matrix multiplication for testing.
/// Computes C = A × B over GF(2) using dot products.
fn naive_multiply(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    assert_eq!(
        a.cols(),
        b.rows(),
        "incompatible dimensions for multiplication"
    );

    let m = a.rows();
    let n = b.cols();
    let k = a.cols();

    let mut c = BitMatrix::new_zero(m, n);

    for i in 0..m {
        for j in 0..n {
            // Compute dot product of row i of A with column j of B
            let mut sum = false;
            for p in 0..k {
                sum ^= a.get(i, p) & b.get(p, j);
            }
            c.set(i, j, sum);
        }
    }

    c
}

#[test]
fn test_multiply_small_square() {
    // 2x2 matrices
    let mut a = BitMatrix::new_zero(2, 2);
    a.set(0, 0, true);
    a.set(0, 1, true);
    a.set(1, 1, true);

    let mut b = BitMatrix::new_zero(2, 2);
    b.set(0, 0, true);
    b.set(1, 0, true);
    b.set(1, 1, true);

    let c = multiply(&a, &b);
    let expected = naive_multiply(&a, &b);

    assert_eq!(c.rows(), 2);
    assert_eq!(c.cols(), 2);

    for r in 0..2 {
        for col in 0..2 {
            assert_eq!(
                c.get(r, col),
                expected.get(r, col),
                "mismatch at ({}, {})",
                r,
                col
            );
        }
    }
}

#[test]
fn test_multiply_identity_left() {
    // I × A = A
    let mut a = BitMatrix::new_zero(3, 4);
    a.set(0, 1, true);
    a.set(1, 2, true);
    a.set(2, 3, true);

    let i = BitMatrix::identity(3);
    let c = multiply(&i, &a);

    for r in 0..3 {
        for col in 0..4 {
            assert_eq!(c.get(r, col), a.get(r, col), "I×A != A at ({}, {})", r, col);
        }
    }
}

#[test]
fn test_multiply_identity_right() {
    // A × I = A
    let mut a = BitMatrix::new_zero(3, 4);
    a.set(0, 1, true);
    a.set(1, 2, true);
    a.set(2, 3, true);

    let i = BitMatrix::identity(4);
    let c = multiply(&a, &i);

    for r in 0..3 {
        for col in 0..4 {
            assert_eq!(c.get(r, col), a.get(r, col), "A×I != A at ({}, {})", r, col);
        }
    }
}

#[test]
fn test_multiply_zero() {
    let a = BitMatrix::new_zero(3, 4);
    let b = BitMatrix::new_zero(4, 5);

    let c = multiply(&a, &b);

    assert_eq!(c.rows(), 3);
    assert_eq!(c.cols(), 5);

    // Result should be all zeros
    for r in 0..3 {
        for col in 0..5 {
            assert!(!c.get(r, col));
        }
    }
}

#[test]
fn test_multiply_rectangular() {
    // 2x3 × 3x2 = 2x2
    let mut a = BitMatrix::new_zero(2, 3);
    a.set(0, 0, true);
    a.set(0, 1, true);
    a.set(1, 1, true);
    a.set(1, 2, true);

    let mut b = BitMatrix::new_zero(3, 2);
    b.set(0, 0, true);
    b.set(1, 1, true);
    b.set(2, 0, true);

    let c = multiply(&a, &b);
    let expected = naive_multiply(&a, &b);

    assert_eq!(c.rows(), 2);
    assert_eq!(c.cols(), 2);

    for r in 0..2 {
        for col in 0..2 {
            assert_eq!(c.get(r, col), expected.get(r, col));
        }
    }
}

#[test]
fn test_multiply_vs_naive_4x4() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(42);

    let mut a = BitMatrix::new_zero(4, 4);
    let mut b = BitMatrix::new_zero(4, 4);

    // Random fill
    for i in 0..4 {
        for j in 0..4 {
            a.set(i, j, rng.gen_bool(0.5));
            b.set(i, j, rng.gen_bool(0.5));
        }
    }

    let c = multiply(&a, &b);
    let expected = naive_multiply(&a, &b);

    for r in 0..4 {
        for col in 0..4 {
            assert_eq!(c.get(r, col), expected.get(r, col));
        }
    }
}

#[test]
fn test_multiply_vs_naive_8x8() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(123);

    let mut a = BitMatrix::new_zero(8, 8);
    let mut b = BitMatrix::new_zero(8, 8);

    for i in 0..8 {
        for j in 0..8 {
            a.set(i, j, rng.gen_bool(0.5));
            b.set(i, j, rng.gen_bool(0.5));
        }
    }

    let c = multiply(&a, &b);
    let expected = naive_multiply(&a, &b);

    for r in 0..8 {
        for col in 0..8 {
            assert_eq!(c.get(r, col), expected.get(r, col));
        }
    }
}

#[test]
fn test_multiply_vs_naive_rectangular_various() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let test_cases = [(5, 7, 3), (10, 15, 8), (16, 16, 16), (20, 10, 15)];

    for (idx, (m, k, n)) in test_cases.iter().enumerate() {
        let mut rng = StdRng::seed_from_u64(1000 + idx as u64);

        let mut a = BitMatrix::new_zero(*m, *k);
        let mut b = BitMatrix::new_zero(*k, *n);

        for i in 0..*m {
            for j in 0..*k {
                a.set(i, j, rng.gen_bool(0.5));
            }
        }

        for i in 0..*k {
            for j in 0..*n {
                b.set(i, j, rng.gen_bool(0.5));
            }
        }

        let c = multiply(&a, &b);
        let expected = naive_multiply(&a, &b);

        assert_eq!(c.rows(), *m);
        assert_eq!(c.cols(), *n);

        for r in 0..*m {
            for col in 0..*n {
                assert_eq!(
                    c.get(r, col),
                    expected.get(r, col),
                    "mismatch at ({}, {}) for test case {}x{}x{}",
                    r,
                    col,
                    m,
                    k,
                    n
                );
            }
        }
    }
}

#[test]
fn test_multiply_large_64x64() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(9999);

    let mut a = BitMatrix::new_zero(64, 64);
    let mut b = BitMatrix::new_zero(64, 64);

    for i in 0..64 {
        for j in 0..64 {
            a.set(i, j, rng.gen_bool(0.5));
            b.set(i, j, rng.gen_bool(0.5));
        }
    }

    let c = multiply(&a, &b);
    let expected = naive_multiply(&a, &b);

    for r in 0..64 {
        for col in 0..64 {
            assert_eq!(c.get(r, col), expected.get(r, col));
        }
    }
}

#[test]
fn test_multiply_with_word_boundaries() {
    // Test with dimensions that span word boundaries
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(2024);

    // Dimensions crossing 64-bit word boundaries
    let mut a = BitMatrix::new_zero(10, 65);
    let mut b = BitMatrix::new_zero(65, 10);

    for i in 0..10 {
        for j in 0..65 {
            a.set(i, j, rng.gen_bool(0.5));
        }
    }

    for i in 0..65 {
        for j in 0..10 {
            b.set(i, j, rng.gen_bool(0.5));
        }
    }

    let c = multiply(&a, &b);
    let expected = naive_multiply(&a, &b);

    for r in 0..10 {
        for col in 0..10 {
            assert_eq!(c.get(r, col), expected.get(r, col));
        }
    }
}

#[test]
#[should_panic(expected = "incompatible")]
fn test_multiply_incompatible_dimensions() {
    let a = BitMatrix::new_zero(3, 4);
    let b = BitMatrix::new_zero(5, 6); // 4 != 5

    let _ = multiply(&a, &b);
}
