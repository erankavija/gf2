use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};
use gf2_core::BitVec;

#[test]
fn test_dual_from_dense_roundtrip() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 1, true);
    m.set(0, 3, true);
    m.set(1, 2, true);
    m.set(2, 0, true);
    m.set(2, 4, true);

    let dual = SpBitMatrixDual::from_dense(&m);
    assert_eq!(dual.rows(), 3);
    assert_eq!(dual.cols(), 5);
    assert_eq!(dual.nnz(), 5);

    let d = dual.to_dense();
    assert_eq!(d, m);
}

#[test]
fn test_dual_row_iter_matches_csr() {
    let coo = vec![(0, 2), (0, 4), (1, 1), (2, 0), (2, 3)];
    let dual = SpBitMatrixDual::from_coo(3, 5, &coo);
    let single = SpBitMatrix::from_coo(3, 5, &coo);

    for r in 0..3 {
        let dual_cols: Vec<_> = dual.row_iter(r).collect();
        let single_cols: Vec<_> = single.row_iter(r).collect();
        assert_eq!(dual_cols, single_cols);
    }
}

#[test]
fn test_dual_col_iter_no_transpose() {
    let mut m = BitMatrix::zeros(4, 3);
    m.set(0, 1, true);
    m.set(2, 1, true);
    m.set(3, 2, true);

    let dual = SpBitMatrixDual::from_dense(&m);

    // Column 1 has rows [0, 2]
    let col1_rows: Vec<_> = dual.col_iter(1).collect();
    assert_eq!(col1_rows, vec![0, 2]);

    // Column 2 has rows [3]
    let col2_rows: Vec<_> = dual.col_iter(2).collect();
    assert_eq!(col2_rows, vec![3]);

    // Column 0 is empty
    let col0_rows: Vec<_> = dual.col_iter(0).collect();
    assert_eq!(col0_rows, Vec::<usize>::new());
}

#[test]
fn test_dual_matvec_matches_single() {
    let coo = vec![(0, 1), (0, 3), (1, 0), (1, 4), (2, 2)];
    let dual = SpBitMatrixDual::from_coo(3, 5, &coo);
    let single = SpBitMatrix::from_coo(3, 5, &coo);

    let mut x = BitVec::new();
    for b in [false, true, true, false, true] {
        x.push_bit(b);
    }

    let y_dual = dual.matvec(&x);
    let y_single = single.matvec(&x);
    assert_eq!(y_dual, y_single);
}

#[test]
fn test_dual_matvec_transpose() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 1, true);
    m.set(0, 3, true);
    m.set(1, 0, true);
    m.set(1, 4, true);
    m.set(2, 2, true);

    let dual = SpBitMatrixDual::from_dense(&m);

    let mut x = BitVec::new();
    for _ in 0..3 {
        x.push_bit(true);
    }

    let y = dual.matvec_transpose(&x);
    assert_eq!(y.len(), 5);

    // Check against manual transpose
    let mt = m.transpose();
    let single_t = SpBitMatrix::from_dense(&mt);
    let y_expected = single_t.matvec(&x);
    assert_eq!(y, y_expected);
}

#[test]
fn test_dual_bidirectional_sweep() {
    // Simulate alternating row/column access pattern
    let mut m = BitMatrix::zeros(4, 6);
    m.set(0, 1, true);
    m.set(1, 2, true);
    m.set(2, 3, true);
    m.set(3, 4, true);

    let dual = SpBitMatrixDual::from_dense(&m);

    // Row sweep
    let mut row_sum = 0;
    for r in 0..dual.rows() {
        row_sum += dual.row_iter(r).count();
    }
    assert_eq!(row_sum, 4);

    // Column sweep (no transpose!)
    let mut col_sum = 0;
    for c in 0..dual.cols() {
        col_sum += dual.col_iter(c).count();
    }
    assert_eq!(col_sum, 4);
}
