#![cfg(feature = "visualization")]

mod matrix_image;

use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};
use gf2_core::test_scratch::scratch;
use matrix_image::assert_image_matches;

fn assert_roundtrip(name: &str, s: &SpBitMatrix) {
    let dir = scratch("sparse-visualization");
    let path = dir.join(format!("{name}.png"));
    s.save_image(&path).unwrap();
    assert_image_matches(&path, &s.to_dense());
}

#[test]
fn test_sparse_save_identity_matrix() {
    assert_roundtrip("identity", &SpBitMatrix::identity(8));
}

#[test]
fn test_sparse_save_zeros_matrix() {
    assert_roundtrip("zeros", &SpBitMatrix::zeros(4, 6));
}

#[test]
fn test_sparse_save_single_bit() {
    assert_roundtrip("single", &SpBitMatrix::from_coo(3, 3, &[(1, 1)]));
}

#[test]
fn test_sparse_dual_save_image() {
    let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    let sd = SpBitMatrixDual::from_coo(3, 4, &coo);
    let dir = scratch("sparse-dual-visualization");
    let path = dir.join("dual.png");
    sd.save_image(&path).unwrap();

    let mut expected = BitMatrix::zeros(3, 4);
    for &(r, c) in &coo {
        expected.set(r, c, true);
    }
    assert_image_matches(&path, &expected);
}

#[cfg(feature = "rand")]
#[test]
fn test_sparse_save_from_dense_random() {
    use rand::thread_rng;

    let m = BitMatrix::random(10, 15, &mut thread_rng());
    let dir = scratch("sparse-visualization");
    let path = dir.join("random.png");
    SpBitMatrix::from_dense(&m).save_image(&path).unwrap();
    assert_image_matches(&path, &m);
}
