#![cfg(feature = "visualization")]

use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SpBitMatrix;
use image::ImageReader;
use std::fs;

#[test]
fn test_sparse_and_dense_produce_identical_images() {
    let mut m = BitMatrix::zeros(5, 7);
    m.set(0, 0, true);
    m.set(0, 6, true);
    m.set(2, 3, true);
    m.set(4, 1, true);
    m.set(4, 5, true);

    let s = SpBitMatrix::from_dense(&m);

    let dense_path = "test_dense_image.png";
    let sparse_path = "test_sparse_image.png";

    m.save_image(dense_path).unwrap();
    s.save_image(sparse_path).unwrap();

    // Verify both images exist
    assert!(std::path::Path::new(dense_path).exists());
    assert!(std::path::Path::new(sparse_path).exists());

    // Load both images
    let img_dense = ImageReader::open(dense_path).unwrap().decode().unwrap();
    let img_sparse = ImageReader::open(sparse_path).unwrap().decode().unwrap();

    // Verify dimensions match
    assert_eq!(img_dense.width(), img_sparse.width());
    assert_eq!(img_dense.height(), img_sparse.height());
    assert_eq!(img_dense.width(), 7);
    assert_eq!(img_dense.height(), 5);

    // Verify pixel data is identical
    let dense_bytes = img_dense.as_bytes();
    let sparse_bytes = img_sparse.as_bytes();
    assert_eq!(
        dense_bytes, sparse_bytes,
        "Dense and sparse images should be pixel-identical"
    );

    fs::remove_file(dense_path).ok();
    fs::remove_file(sparse_path).ok();
}
