//! `BitMatrix` core operations.

use gf2_core::matrix::BitMatrix;

#[test]
fn test_zeros_basic() {
    let m = BitMatrix::zeros(3, 4);
    assert_eq!(m.rows(), 3);
    assert_eq!(m.cols(), 4);

    for r in 0..3 {
        for c in 0..4 {
            assert!(!m.get(r, c), "bit at ({}, {}) should be false", r, c);
        }
    }
}

#[test]
fn test_zeros_empty() {
    let m = BitMatrix::zeros(0, 0);
    assert_eq!(m.rows(), 0);
    assert_eq!(m.cols(), 0);
}

#[test]
fn test_zeros_single_row() {
    let m = BitMatrix::zeros(1, 10);
    assert_eq!(m.rows(), 1);
    assert_eq!(m.cols(), 10);
}

#[test]
fn test_zeros_single_col() {
    let m = BitMatrix::zeros(10, 1);
    assert_eq!(m.rows(), 10);
    assert_eq!(m.cols(), 1);
}

#[test]
fn test_stride_words() {
    let m1 = BitMatrix::zeros(1, 64);
    assert_eq!(m1.stride_words(), 1);

    let m2 = BitMatrix::zeros(1, 65);
    assert_eq!(m2.stride_words(), 2);

    let m3 = BitMatrix::zeros(1, 1);
    assert_eq!(m3.stride_words(), 1);
}

#[test]
fn test_get_set_basic() {
    let mut m = BitMatrix::zeros(3, 4);

    m.set(0, 0, true);
    m.set(1, 2, true);
    m.set(2, 3, true);

    assert!(m.get(0, 0));
    assert!(m.get(1, 2));
    assert!(m.get(2, 3));

    assert!(!m.get(0, 1));
    assert!(!m.get(0, 2));
    assert!(!m.get(1, 0));
}

#[test]
fn test_get_set_large() {
    let mut m = BitMatrix::zeros(10, 128);

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

    for i in 0..4 {
        assert!(m.get(i, i), "diagonal ({}, {}) should be true", i, i);
    }

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
    let mut m = BitMatrix::zeros(3, 4);

    m.set(0, 0, true);
    m.set(0, 2, true);

    m.set(1, 1, true);
    m.set(1, 3, true);

    m.swap_rows(0, 1);

    assert!(!m.get(0, 0));
    assert!(m.get(0, 1));
    assert!(!m.get(0, 2));
    assert!(m.get(0, 3));

    assert!(m.get(1, 0));
    assert!(!m.get(1, 1));
    assert!(m.get(1, 2));
    assert!(!m.get(1, 3));
}

#[test]
fn test_swap_rows_same_row() {
    let mut m = BitMatrix::zeros(3, 4);
    m.set(1, 1, true);
    m.set(1, 2, true);

    m.swap_rows(1, 1);

    assert!(m.get(1, 1));
    assert!(m.get(1, 2));
}

#[test]
fn test_row_words() {
    let mut m = BitMatrix::zeros(2, 128);

    m.set(0, 0, true);
    m.set(0, 63, true);
    m.set(0, 64, true);

    let words = m.row_words(0);
    assert_eq!(words.len(), 2); // 128 bits = 2 words
    assert_eq!(words[0] & 1, 1);
    assert_eq!(words[0] & (1u64 << 63), 1u64 << 63);
    assert_eq!(words[1] & 1, 1); // bit 64 (first bit of second word) set
}

#[test]
fn test_row_words_mut() {
    let mut m = BitMatrix::zeros(2, 128);

    {
        let words = m.row_words_mut(0);
        words[0] = 0xFFFFFFFFFFFFFFFFu64;
        words[1] = 0x00000000000000FFu64;
    }

    assert!(m.get(0, 0));
    assert!(m.get(0, 63));
    assert!(m.get(0, 64));
    assert!(m.get(0, 71));
    assert!(!m.get(0, 72));
}

#[test]
fn test_transpose_square() {
    let mut m = BitMatrix::zeros(3, 3);

    m.set(0, 1, true);
    m.set(0, 2, true);
    m.set(1, 0, true);
    m.set(2, 1, true);

    let mt = m.transpose();

    assert_eq!(mt.rows(), 3);
    assert_eq!(mt.cols(), 3);

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
    let mut m = BitMatrix::zeros(2, 3);

    m.set(0, 0, true);
    m.set(0, 2, true);
    m.set(1, 1, true);

    let mt = m.transpose();

    assert_eq!(mt.rows(), 3);
    assert_eq!(mt.cols(), 2);

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

    for r in 0..5 {
        for c in 0..5 {
            assert_eq!(m.get(r, c), mt.get(r, c));
        }
    }
}

#[test]
fn test_transpose_word_boundaries() {
    for &n in &[0usize, 1, 63, 64, 65, 127, 128, 129] {
        let m = BitMatrix::random_seeded(n, n, 0xB17B_10C0 ^ n as u64);
        let mt = m.transpose();

        assert_eq!(mt.rows(), n);
        assert_eq!(mt.cols(), n);
        assert_eq!(mt.transpose(), m, "transpose(transpose(M)) != M for n={n}");

        for r in 0..n {
            for c in 0..n {
                assert_eq!(
                    mt.get(c, r),
                    m.get(r, c),
                    "transpose mismatch at n={n}, row={r}, col={c}"
                );
            }
        }
    }
}

#[test]
#[should_panic]
fn test_get_out_of_bounds_row() {
    let m = BitMatrix::zeros(3, 4);
    let _ = m.get(3, 0); // row 3 doesn't exist
}

#[test]
#[should_panic]
fn test_get_out_of_bounds_col() {
    let m = BitMatrix::zeros(3, 4);
    let _ = m.get(0, 4); // col 4 doesn't exist
}

#[test]
#[should_panic]
fn test_set_out_of_bounds_row() {
    let mut m = BitMatrix::zeros(3, 4);
    m.set(3, 0, true);
}

#[test]
#[should_panic]
fn test_set_out_of_bounds_col() {
    let mut m = BitMatrix::zeros(3, 4);
    m.set(0, 4, true);
}
