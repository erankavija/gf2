#![cfg(feature = "visualization")]

use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};
use image::ImageReader;
use std::fs;

#[test]
fn test_sparse_save_identity_matrix() {
    let s = SpBitMatrix::identity(8);
    let path = "test_sparse_output_identity.png";

    s.save_image(path).unwrap();
    assert!(std::path::Path::new(path).exists());

    // Verify image dimensions
    let img = ImageReader::open(path).unwrap().decode().unwrap();
    assert_eq!(img.width(), 8);
    assert_eq!(img.height(), 8);

    fs::remove_file(path).ok();
}

#[test]
fn test_sparse_save_zeros_matrix() {
    let s = SpBitMatrix::zeros(4, 6);
    let path = "test_sparse_output_zeros.png";

    s.save_image(path).unwrap();

    let img = ImageReader::open(path).unwrap().decode().unwrap();
    assert_eq!(img.width(), 6);
    assert_eq!(img.height(), 4);

    fs::remove_file(path).ok();
}

#[test]
fn test_sparse_save_single_bit() {
    let coo = vec![(1, 1)];
    let s = SpBitMatrix::from_coo(3, 3, &coo);
    let path = "test_sparse_output_single.png";

    s.save_image(path).unwrap();
    assert!(std::path::Path::new(path).exists());

    fs::remove_file(path).ok();
}

#[test]
fn test_sparse_dual_save_image() {
    let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    let sd = SpBitMatrixDual::from_coo(3, 4, &coo);
    let path = "test_sparse_dual_output.png";

    sd.save_image(path).unwrap();

    let img = ImageReader::open(path).unwrap().decode().unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 3);

    fs::remove_file(path).ok();
}

#[cfg(feature = "rand")]
#[test]
fn test_sparse_save_from_dense_random() {
    use gf2_core::matrix::BitMatrix;
    use rand::thread_rng;

    let m = BitMatrix::random(10, 15, &mut thread_rng());
    let s = SpBitMatrix::from_dense(&m);
    let path = "test_sparse_output_random.png";

    s.save_image(path).unwrap();

    let img = ImageReader::open(path).unwrap().decode().unwrap();
    assert_eq!(img.width(), 15);
    assert_eq!(img.height(), 10);

    fs::remove_file(path).ok();
}
