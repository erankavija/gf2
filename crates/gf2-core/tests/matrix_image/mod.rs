//! Decodes a saved matrix image and compares it with the matrix it came from.

#![allow(dead_code)]

use gf2_core::matrix::BitMatrix;
use image::ImageReader;
use std::path::Path;

/// Decodes the PNG at `path` and asserts it has the matrix's dimensions and
/// that each pixel is white where the bit is set and black elsewhere.
pub fn assert_image_matches(path: &Path, expected: &BitMatrix) {
    let img = ImageReader::open(path).unwrap().decode().unwrap().to_rgb8();
    assert_eq!(img.width() as usize, expected.cols());
    assert_eq!(img.height() as usize, expected.rows());
    for r in 0..expected.rows() {
        for c in 0..expected.cols() {
            let want = if expected.get(r, c) { [255; 3] } else { [0; 3] };
            assert_eq!(
                img.get_pixel(c as u32, r as u32).0,
                want,
                "pixel ({r}, {c})"
            );
        }
    }
}
