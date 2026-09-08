#![cfg(feature = "visualization")]

use gf2_core::matrix::BitMatrix;
use image::ImageReader;
use std::fs;

#[test]
fn test_save_identity_matrix() {
    let m = BitMatrix::identity(8);
    let path = "test_output_identity.png";

    m.save_image(path).unwrap();
    assert!(std::path::Path::new(path).exists());

    // Verify image dimensions
    let img = ImageReader::open(path).unwrap().decode().unwrap();
    assert_eq!(img.width(), 8);
    assert_eq!(img.height(), 8);

    fs::remove_file(path).ok();
}

#[test]
fn test_save_zeros_matrix() {
    let m = BitMatrix::zeros(4, 6);
    let path = "test_output_zeros.png";

    m.save_image(path).unwrap();

    let img = ImageReader::open(path).unwrap().decode().unwrap();
    assert_eq!(img.width(), 6);
    assert_eq!(img.height(), 4);

    fs::remove_file(path).ok();
}

#[test]
fn test_save_single_bit() {
    let mut m = BitMatrix::zeros(3, 3);
    m.set(1, 1, true);
    let path = "test_output_single.png";

    m.save_image(path).unwrap();
    assert!(std::path::Path::new(path).exists());

    fs::remove_file(path).ok();
}

#[cfg(feature = "rand")]
#[test]
fn test_save_random_matrix() {
    use rand::thread_rng;
    let m = BitMatrix::random(20, 30, &mut thread_rng());
    let path = "test_output_random.png";

    m.save_image(path).unwrap();

    let img = ImageReader::open(path).unwrap().decode().unwrap();
    assert_eq!(img.width(), 30);
    assert_eq!(img.height(), 20);

    fs::remove_file(path).ok();
}
