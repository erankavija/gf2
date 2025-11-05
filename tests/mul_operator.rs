//! Integration tests for the matrix multiplication operator (*).

use gf2::matrix::BitMatrix;

#[test]
fn test_mul_operator_basic_square() {
    // Create two simple 2x2 matrices
    let mut a = BitMatrix::zeros(2, 2);
    a.set(0, 0, true);
    a.set(1, 1, true);

    let mut b = BitMatrix::zeros(2, 2);
    b.set(0, 1, true);
    b.set(1, 0, true);

    // Use the * operator
    let c = &a * &b;

    // Expected result:
    // [1 0] * [0 1] = [0 1]
    // [0 1]   [1 0]   [1 0]
    assert!(c.get(0, 1));
    assert!(c.get(1, 0));
    assert!(!c.get(0, 0));
    assert!(!c.get(1, 1));
}

#[test]
fn test_mul_operator_identity_left() {
    // I * A = A
    let mut a = BitMatrix::zeros(3, 4);
    a.set(0, 1, true);
    a.set(1, 2, true);
    a.set(2, 3, true);

    let i = BitMatrix::identity(3);
    let c = &i * &a;

    for r in 0..3 {
        for col in 0..4 {
            assert_eq!(c.get(r, col), a.get(r, col));
        }
    }
}

#[test]
fn test_mul_operator_identity_right() {
    // A * I = A
    let mut a = BitMatrix::zeros(3, 4);
    a.set(0, 1, true);
    a.set(1, 2, true);
    a.set(2, 3, true);

    let i = BitMatrix::identity(4);
    let c = &a * &i;

    for r in 0..3 {
        for col in 0..4 {
            assert_eq!(c.get(r, col), a.get(r, col));
        }
    }
}

#[test]
fn test_mul_operator_owned_values() {
    // Test that owned values can be used
    let a = BitMatrix::identity(3);
    let b = BitMatrix::identity(3);

    // This consumes both a and b
    let c = a * b;

    assert_eq!(c, BitMatrix::identity(3));
}

#[test]
fn test_mul_operator_mixed_refs_owned_lhs() {
    // Test a * &b
    let a = BitMatrix::identity(2);
    let b = BitMatrix::identity(2);

    let c = a * &b;

    assert_eq!(c, BitMatrix::identity(2));
}

#[test]
fn test_mul_operator_mixed_refs_owned_rhs() {
    // Test &a * b
    let a = BitMatrix::identity(2);
    let b = BitMatrix::identity(2);

    let c = &a * b;

    assert_eq!(c, BitMatrix::identity(2));
}

#[test]
fn test_mul_operator_rectangular_matrix() {
    // Test multiplication of non-square matrices
    // 2x3 * 3x2 = 2x2
    let mut a = BitMatrix::zeros(2, 3);
    a.set(0, 0, true);
    a.set(0, 1, true);
    a.set(1, 1, true);
    a.set(1, 2, true);

    let mut b = BitMatrix::zeros(3, 2);
    b.set(0, 0, true);
    b.set(1, 1, true);
    b.set(2, 0, true);

    let c = &a * &b;

    assert_eq!(c.rows(), 2);
    assert_eq!(c.cols(), 2);

    // Manual verification of matrix multiplication over GF(2)
    // A = [1 1 0]    B = [1 0]
    //     [0 1 1]        [0 1]
    //                    [1 0]
    // C = [(1&1)^(1&0)^(0&1), (1&0)^(1&1)^(0&0)]
    //     [(0&1)^(1&0)^(1&1), (0&0)^(1&1)^(1&0)]
    // C = [1, 1]
    //     [1, 1]
    assert!(c.get(0, 0));
    assert!(c.get(0, 1));
    assert!(c.get(1, 0));
    assert!(c.get(1, 1));
}

#[test]
fn test_mul_operator_chain() {
    // Test chaining: (A * B) * C
    let a = BitMatrix::identity(2);
    let b = BitMatrix::identity(2);
    let c = BitMatrix::identity(2);

    let result = (&a * &b) * &c;

    assert_eq!(result, BitMatrix::identity(2));
}

#[test]
fn test_mul_operator_matrix_vector_simulation() {
    // Simulate matrix-vector multiplication using a column vector (nx1 matrix)
    // This is similar to what's done in the Hamming code example
    let mut a = BitMatrix::zeros(3, 3);
    a.set(0, 0, true);
    a.set(0, 1, true);
    a.set(1, 1, true);
    a.set(2, 2, true);

    // Column vector [1, 1, 0]^T represented as 3x1 matrix
    let mut v = BitMatrix::zeros(3, 1);
    v.set(0, 0, true);
    v.set(1, 0, true);

    // A * v using the * operator
    let result = &a * &v;

    assert_eq!(result.rows(), 3);
    assert_eq!(result.cols(), 1);

    // Expected: [1^1, 1, 0]^T = [0, 1, 0]^T in GF(2)
    assert!(!result.get(0, 0)); // 1 XOR 1 = 0
    assert!(result.get(1, 0)); // 1
    assert!(!result.get(2, 0)); // 0
}

#[test]
fn test_mul_operator_zero_matrix() {
    // Test multiplication with zero matrices
    let a = BitMatrix::zeros(3, 4);
    let b = BitMatrix::zeros(4, 5);

    let c = &a * &b;

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
fn test_mul_operator_large_matrices() {
    // Test with larger matrices (crossing word boundaries)
    let mut a = BitMatrix::zeros(10, 65);
    let mut b = BitMatrix::zeros(65, 10);

    // Set some bits
    a.set(0, 0, true);
    a.set(0, 64, true);
    b.set(0, 0, true);
    b.set(64, 9, true);

    let c = &a * &b;

    assert_eq!(c.rows(), 10);
    assert_eq!(c.cols(), 10);

    // Expected: c[0,0] = 1, c[0,9] = 1
    assert!(c.get(0, 0));
    assert!(c.get(0, 9));
}
