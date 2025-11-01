//! Tests for BitMatrix core functionality.

use gf2::matrix::BitMatrix;

#[test]
fn test_new_zero_basic() {
    let m = BitMatrix::new_zero(3, 4);
    assert_eq!(m.rows(), 3);
    assert_eq!(m.cols(), 4);

    // All bits should be zero
    for r in 0..3 {
        for c in 0..4 {
            assert!(!m.get(r, c), "bit at ({}, {}) should be false", r, c);
        }
    }
}

#[test]
fn test_new_zero_empty() {
    let m = BitMatrix::new_zero(0, 0);
    assert_eq!(m.rows(), 0);
    assert_eq!(m.cols(), 0);
}

#[test]
fn test_new_zero_single_row() {
    let m = BitMatrix::new_zero(1, 10);
    assert_eq!(m.rows(), 1);
    assert_eq!(m.cols(), 10);
}

#[test]
fn test_new_zero_single_col() {
    let m = BitMatrix::new_zero(10, 1);
    assert_eq!(m.rows(), 10);
    assert_eq!(m.cols(), 1);
}

#[test]
fn test_stride_words() {
    // 64 cols should be 1 word
    let m1 = BitMatrix::new_zero(1, 64);
    assert_eq!(m1.stride_words(), 1);

    // 65 cols should be 2 words
    let m2 = BitMatrix::new_zero(1, 65);
    assert_eq!(m2.stride_words(), 2);

    // 1 col should be 1 word
    let m3 = BitMatrix::new_zero(1, 1);
    assert_eq!(m3.stride_words(), 1);
}

#[test]
fn test_get_set_basic() {
    let mut m = BitMatrix::new_zero(3, 4);

    // Set some bits
    m.set(0, 0, true);
    m.set(1, 2, true);
    m.set(2, 3, true);

    // Check they're set
    assert!(m.get(0, 0));
    assert!(m.get(1, 2));
    assert!(m.get(2, 3));

    // Check others are still false
    assert!(!m.get(0, 1));
    assert!(!m.get(0, 2));
    assert!(!m.get(1, 0));
}

#[test]
fn test_get_set_large() {
    let mut m = BitMatrix::new_zero(10, 128);

    // Set bits across multiple words
    m.set(5, 63, true); // Last bit of first word
    m.set(5, 64, true); // First bit of second word
    m.set(5, 127, true); // Last bit of second word

    assert!(m.get(5, 63));
    assert!(m.get(5, 64));
    assert!(m.get(5, 127));
    assert!(!m.get(5, 62));
    assert!(!m.get(5, 65));
}

#[test]
fn test_identity_square() {
    let m = BitMatrix::identity(4);
    assert_eq!(m.rows(), 4);
    assert_eq!(m.cols(), 4);

    // Check diagonal is 1
    for i in 0..4 {
        assert!(m.get(i, i), "diagonal ({}, {}) should be true", i, i);
    }

    // Check off-diagonal is 0
    for r in 0..4 {
        for c in 0..4 {
            if r != c {
                assert!(!m.get(r, c), "off-diagonal ({}, {}) should be false", r, c);
            }
        }
    }
}

#[test]
fn test_identity_1x1() {
    let m = BitMatrix::identity(1);
    assert_eq!(m.rows(), 1);
    assert_eq!(m.cols(), 1);
    assert!(m.get(0, 0));
}

#[test]
fn test_swap_rows() {
    let mut m = BitMatrix::new_zero(3, 4);

    // Set up row 0: [1, 0, 1, 0]
    m.set(0, 0, true);
    m.set(0, 2, true);

    // Set up row 1: [0, 1, 0, 1]
    m.set(1, 1, true);
    m.set(1, 3, true);

    // Swap rows 0 and 1
    m.swap_rows(0, 1);

    // Check row 0 now has what was row 1
    assert!(!m.get(0, 0));
    assert!(m.get(0, 1));
    assert!(!m.get(0, 2));
    assert!(m.get(0, 3));

    // Check row 1 now has what was row 0
    assert!(m.get(1, 0));
    assert!(!m.get(1, 1));
    assert!(m.get(1, 2));
    assert!(!m.get(1, 3));
}

#[test]
fn test_swap_rows_same_row() {
    let mut m = BitMatrix::new_zero(3, 4);
    m.set(1, 1, true);
    m.set(1, 2, true);

    // Swap row with itself
    m.swap_rows(1, 1);

    // Should be unchanged
    assert!(m.get(1, 1));
    assert!(m.get(1, 2));
}

#[test]
fn test_row_words() {
    let mut m = BitMatrix::new_zero(2, 128);

    // Set some bits in row 0
    m.set(0, 0, true);
    m.set(0, 63, true);
    m.set(0, 64, true);

    let words = m.row_words(0);
    assert_eq!(words.len(), 2); // 128 bits = 2 words
    assert_eq!(words[0] & 1, 1); // bit 0 set
    assert_eq!(words[0] & (1u64 << 63), 1u64 << 63); // bit 63 set
    assert_eq!(words[1] & 1, 1); // bit 64 (first bit of second word) set
}

#[test]
fn test_row_words_mut() {
    let mut m = BitMatrix::new_zero(2, 128);

    // Modify row directly via row_words_mut
    {
        let words = m.row_words_mut(0);
        words[0] = 0xFFFFFFFFFFFFFFFFu64;
        words[1] = 0x00000000000000FFu64;
    }

    // Check bits are set correctly
    assert!(m.get(0, 0));
    assert!(m.get(0, 63));
    assert!(m.get(0, 64));
    assert!(m.get(0, 71));
    assert!(!m.get(0, 72));
}

#[test]
fn test_transpose_square() {
    let mut m = BitMatrix::new_zero(3, 3);

    // Set up a non-symmetric matrix
    m.set(0, 1, true);
    m.set(0, 2, true);
    m.set(1, 0, true);
    m.set(2, 1, true);

    let mt = m.transpose();

    assert_eq!(mt.rows(), 3);
    assert_eq!(mt.cols(), 3);

    // Check transpose property: mt[i,j] = m[j,i]
    for r in 0..3 {
        for c in 0..3 {
            assert_eq!(
                mt.get(r, c),
                m.get(c, r),
                "transpose mismatch at ({}, {})",
                r,
                c
            );
        }
    }
}

#[test]
fn test_transpose_rectangular() {
    let mut m = BitMatrix::new_zero(2, 3);

    // Set up pattern
    m.set(0, 0, true);
    m.set(0, 2, true);
    m.set(1, 1, true);

    let mt = m.transpose();

    assert_eq!(mt.rows(), 3);
    assert_eq!(mt.cols(), 2);

    // Check transpose
    assert!(mt.get(0, 0));
    assert!(mt.get(2, 0));
    assert!(mt.get(1, 1));
    assert!(!mt.get(0, 1));
    assert!(!mt.get(1, 0));
}

#[test]
fn test_transpose_identity() {
    let m = BitMatrix::identity(5);
    let mt = m.transpose();

    // Identity matrix should equal its transpose
    for r in 0..5 {
        for c in 0..5 {
            assert_eq!(m.get(r, c), mt.get(r, c));
        }
    }
}

#[test]
#[should_panic]
fn test_get_out_of_bounds_row() {
    let m = BitMatrix::new_zero(3, 4);
    let _ = m.get(3, 0); // row 3 doesn't exist
}

#[test]
#[should_panic]
fn test_get_out_of_bounds_col() {
    let m = BitMatrix::new_zero(3, 4);
    let _ = m.get(0, 4); // col 4 doesn't exist
}

#[test]
#[should_panic]
fn test_set_out_of_bounds_row() {
    let mut m = BitMatrix::new_zero(3, 4);
    m.set(3, 0, true);
}

#[test]
#[should_panic]
fn test_set_out_of_bounds_col() {
    let mut m = BitMatrix::new_zero(3, 4);
    m.set(0, 4, true);
}
