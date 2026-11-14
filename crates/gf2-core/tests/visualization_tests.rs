#![cfg(feature = "visualization")]

mod matrix_image;

use gf2_core::matrix::BitMatrix;
use gf2_core::test_scratch::scratch;
use matrix_image::assert_image_matches;

fn assert_roundtrip(name: &str, m: &BitMatrix) {
    let dir = scratch("visualization");
    let path = dir.join(format!("{name}.png"));
    m.save_image(&path).unwrap();
    assert_image_matches(&path, m);
}

#[test]
fn test_save_identity_matrix() {
    assert_roundtrip("identity", &BitMatrix::identity(8));
}

#[test]
fn test_save_zeros_matrix() {
    assert_roundtrip("zeros", &BitMatrix::zeros(4, 6));
}

#[test]
fn test_save_single_bit() {
    let mut m = BitMatrix::zeros(3, 3);
    m.set(1, 1, true);
    assert_roundtrip("single", &m);
}

#[test]
fn test_save_word_boundary_widths() {
    for cols in [1, 63, 64, 65] {
        let mut m = BitMatrix::zeros(2, cols);
        m.set(0, cols - 1, true);
        m.set(1, 0, true);
        assert_roundtrip(&format!("w{cols}"), &m);
    }
}

#[cfg(feature = "rand")]
#[test]
fn test_save_random_matrix() {
    use rand::thread_rng;

    assert_roundtrip("random", &BitMatrix::random(20, 30, &mut thread_rng()));
}
