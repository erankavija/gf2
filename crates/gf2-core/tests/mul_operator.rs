//! `BitMatrix` multiplication through the `*` operator.

use gf2_core::matrix::BitMatrix;

#[test]
fn test_mul_operator_basic_square() {
    let mut a = BitMatrix::zeros(2, 2);
    a.set(0, 0, true);
    a.set(1, 1, true);

    let mut b = BitMatrix::zeros(2, 2);
    b.set(0, 1, true);
    b.set(1, 0, true);

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
    let a = BitMatrix::identity(3);
    let b = BitMatrix::identity(3);

    let c = a * b;

    assert_eq!(c, BitMatrix::identity(3));
}

#[test]
fn test_mul_operator_mixed_refs_owned_lhs() {
    let a = BitMatrix::identity(2);
    let b = BitMatrix::identity(2);

    let c = a * &b;

    assert_eq!(c, BitMatrix::identity(2));
}

#[test]
fn test_mul_operator_mixed_refs_owned_rhs() {
    let a = BitMatrix::identity(2);
    let b = BitMatrix::identity(2);

    let c = &a * b;

    assert_eq!(c, BitMatrix::identity(2));
}

#[test]
fn test_mul_operator_rectangular_matrix() {
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
    let a = BitMatrix::identity(2);
    let b = BitMatrix::identity(2);
    let c = BitMatrix::identity(2);

    let result = (&a * &b) * &c;

    assert_eq!(result, BitMatrix::identity(2));
}

#[test]
fn test_mul_operator_matrix_vector_simulation() {
    let mut a = BitMatrix::zeros(3, 3);
    a.set(0, 0, true);
    a.set(0, 1, true);
    a.set(1, 1, true);
    a.set(2, 2, true);

    let mut v = BitMatrix::zeros(3, 1);
    v.set(0, 0, true);
    v.set(1, 0, true);

    let result = &a * &v;

    assert_eq!(result.rows(), 3);
    assert_eq!(result.cols(), 1);

    // Expected: [1^1, 1, 0]^T = [0, 1, 0]^T in GF(2)
    assert!(!result.get(0, 0));
    assert!(result.get(1, 0));
    assert!(!result.get(2, 0));
}

#[test]
fn test_mul_operator_zero_matrix() {
    let a = BitMatrix::zeros(3, 4);
    let b = BitMatrix::zeros(4, 5);

    let c = &a * &b;

    assert_eq!(c.rows(), 3);
    assert_eq!(c.cols(), 5);

    for r in 0..3 {
        for col in 0..5 {
            assert!(!c.get(r, col));
        }
    }
}

#[test]
fn test_mul_operator_large_matrices() {
    let mut a = BitMatrix::zeros(10, 65);
    let mut b = BitMatrix::zeros(65, 10);

    a.set(0, 0, true);
    a.set(0, 64, true);
    b.set(0, 0, true);
    b.set(64, 9, true);

    let c = &a * &b;

    assert_eq!(c.rows(), 10);
    assert_eq!(c.cols(), 10);

    assert!(c.get(0, 0));
    assert!(c.get(0, 9));
}
