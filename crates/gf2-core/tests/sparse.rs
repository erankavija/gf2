use gf2_core::sparse::SparseMatrix;
use gf2_core::{matrix::BitMatrix, BitVec};

#[test]
fn test_sparse_zeros_and_identity() {
    let s0 = SparseMatrix::zeros(3, 4);
    assert_eq!(s0.rows(), 3);
    assert_eq!(s0.cols(), 4);
    assert_eq!(s0.nnz(), 0);

    let si = SparseMatrix::identity(4);
    assert_eq!(si.rows(), 4);
    assert_eq!(si.cols(), 4);
    assert_eq!(si.nnz(), 4);

    let d = si.to_dense();
    for i in 0..4 {
        assert!(d.get(i, i));
    }
}

#[test]
fn test_sparse_from_coo_dedup_xor() {
    // Two entries at same (r,c) cancel under XOR
    let coo = vec![(0, 1), (0, 1), (1, 2)];
    let s = SparseMatrix::from_coo(3, 4, &coo);
    assert_eq!(s.nnz(), 1);
    let d = s.to_dense();
    assert!(!d.get(0, 1));
    assert!(d.get(1, 2));
}

#[test]
fn test_sparse_from_dense_roundtrip() {
    let mut m = BitMatrix::zeros(2, 3);
    m.set(0, 0, true);
    m.set(0, 2, true);
    m.set(1, 1, true);

    let s = SparseMatrix::from_dense(&m);
    assert_eq!(s.nnz(), 3);
    let d = s.to_dense();
    assert_eq!(d, m);
}

#[test]
fn test_sparse_matvec() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 1, true);
    m.set(0, 3, true);
    m.set(1, 0, true);
    m.set(1, 4, true);
    m.set(2, 2, true);
    let s = SparseMatrix::from_dense(&m);

    let mut x = BitVec::with_capacity(5);
    for b in [false, true, true, false, true] {
        x.push_bit(b);
    }

    let y = s.matvec(&x);
    assert_eq!(y.len(), 3);
    // Row 0: x1 XOR x3 = 1 XOR 0 = 1
    assert!(y.get(0));
    // Row 1: x0 XOR x4 = 0 XOR 1 = 1
    assert!(y.get(1));
    // Row 2: x2 = 1
    assert!(y.get(2));
}
