use gf2_core::matrix::BitMatrix;

#[test]
fn test_bitmatrix_to_sparse_roundtrip() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 1, true);
    m.set(1, 3, true);
    m.set(2, 4, true);

    let s = m.to_sparse();
    assert_eq!(s.nnz(), 3);
    let d = s.to_dense();
    assert_eq!(d, m);
}
