#![cfg(feature = "visualization")]

mod matrix_image;

use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SpBitMatrix;
use gf2_core::test_scratch::scratch;
use image::ImageReader;
use matrix_image::assert_image_matches;

#[test]
fn test_sparse_and_dense_produce_identical_images() {
    let mut m = BitMatrix::zeros(5, 7);
    for (r, c) in [(0, 0), (0, 6), (2, 3), (4, 1), (4, 5)] {
        m.set(r, c, true);
    }
    let s = SpBitMatrix::from_dense(&m);

    let dir = scratch("image-equivalence");
    let dense_path = dir.join("dense.png");
    let sparse_path = dir.join("sparse.png");
    m.save_image(&dense_path).unwrap();
    s.save_image(&sparse_path).unwrap();

    assert_image_matches(&dense_path, &m);
    assert_image_matches(&sparse_path, &m);

    let dense = ImageReader::open(&dense_path).unwrap().decode().unwrap();
    let sparse = ImageReader::open(&sparse_path).unwrap().decode().unwrap();
    assert_eq!(dense.as_bytes(), sparse.as_bytes());
}
