use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SpBitMatrix;

#[test]
fn test_row_iter_basic() {
    let coo = vec![(0, 2), (0, 4), (1, 1), (2, 0), (2, 3)];
    let s = SpBitMatrix::from_coo(3, 5, &coo);
    let r0: Vec<_> = s.row_iter(0).collect();
    assert_eq!(r0, vec![2, 4]);
    let r1: Vec<_> = s.row_iter(1).collect();
    assert_eq!(r1, vec![1]);
    let r2: Vec<_> = s.row_iter(2).collect();
    assert_eq!(r2, vec![0, 3]);
}

#[test]
fn test_transpose_roundtrip_dense() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 2, true);
    m.set(0, 4, true);
    m.set(1, 1, true);
    m.set(2, 0, true);
    m.set(2, 3, true);

    let s = SpBitMatrix::from_dense(&m);
    let st = s.transpose();
    let dtt = st.transpose().to_dense();
    assert_eq!(dtt, m);
}

#[test]
fn test_column_access_via_transpose() {
    // Column 3 has (rows 0,2) set.
    let mut m = BitMatrix::zeros(3, 6);
    m.set(0, 3, true);
    m.set(2, 3, true);
    let s = SpBitMatrix::from_dense(&m);
    // Column 3 via col_iter
    let col3_rows: Vec<_> = s.col_iter(3).into_iter().collect();
    assert_eq!(col3_rows, vec![0, 2]);
}
