//! Correctness coverage for the Strassen-family BitMatrix multiply layer.

use gf2_core::alg::m4rm::multiply as m4rm_multiply;
use gf2_core::matrix::BitMatrix;
use proptest::prelude::*;
use proptest::sample::select;

fn patterned_square(n: usize, salt: usize) -> BitMatrix {
    let mut matrix = BitMatrix::zeros(n, n);
    for row in 0..n {
        for col in 0..n {
            matrix.set(row, col, ((row * 17 + col * 31 + salt * 13) & 7) <= 2);
        }
    }
    matrix
}

fn matrix_from_bits(n: usize, bits: &[bool]) -> BitMatrix {
    let mut matrix = BitMatrix::zeros(n, n);
    for (idx, &bit) in bits.iter().enumerate() {
        if bit {
            matrix.set(idx / n, idx % n, true);
        }
    }
    matrix
}

fn assert_tail_masked(matrix: &BitMatrix) {
    let used = matrix.cols() % 64;
    if used == 0 || matrix.cols() == 0 {
        return;
    }

    let mask = (1u64 << used) - 1;
    for row in 0..matrix.rows() {
        let last = matrix.row_words(row).last().copied().unwrap_or(0);
        assert_eq!(last & !mask, 0, "row {row} has unmasked tail bits");
    }
}

#[test]
fn auto_matmul_matches_m4rm_on_required_edge_sizes() {
    for n in [0usize, 1, 63, 64, 65, 127, 128, 129, 256, 512] {
        let lhs = patterned_square(n, 1);
        let rhs = patterned_square(n, 2);
        let actual = &lhs * &rhs;
        let expected = m4rm_multiply(&lhs, &rhs);

        assert_eq!(actual, expected, "auto multiply diverged at n={n}");
        assert_tail_masked(&actual);
    }
}

#[test]
fn rectangular_shapes_preserve_m4rm_fallback_semantics() {
    let lhs = {
        let mut matrix = BitMatrix::zeros(129, 257);
        for row in 0..matrix.rows() {
            for col in 0..matrix.cols() {
                matrix.set(row, col, ((row * 5 + col * 7) & 15) <= 4);
            }
        }
        matrix
    };
    let rhs = {
        let mut matrix = BitMatrix::zeros(257, 65);
        for row in 0..matrix.rows() {
            for col in 0..matrix.cols() {
                matrix.set(row, col, ((row * 11 + col * 3) & 15) <= 6);
            }
        }
        matrix
    };

    assert_eq!(&lhs * &rhs, m4rm_multiply(&lhs, &rhs));
}

#[test]
fn forced_strassen_matches_m4rm_at_word_boundaries() {
    for n in [64usize, 128, 256] {
        let lhs = patterned_square(n, 3);
        let rhs = patterned_square(n, 4);
        let actual = lhs.strassen_mul_for_test(&rhs, 32, 3);
        let expected = m4rm_multiply(&lhs, &rhs);

        assert_eq!(actual, expected, "forced Strassen diverged at n={n}");
        assert_tail_masked(&actual);
    }
}

#[test]
fn forced_strassen_release_validation_512_and_1024() {
    if cfg!(debug_assertions) {
        return;
    }

    for n in [512usize, 1024] {
        let lhs = patterned_square(n, 5);
        let rhs = patterned_square(n, 6);
        let actual = lhs.strassen_mul_for_test(&rhs, 128, 2);
        let expected = m4rm_multiply(&lhs, &rhs);

        assert_eq!(actual, expected, "forced Strassen diverged at n={n}");
        assert_tail_masked(&actual);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        .. ProptestConfig::default()
    })]

    #[test]
    fn forced_strassen_matches_m4rm_for_small_power_of_two_squares(
        n in select(vec![2usize, 4, 8, 16]),
        lhs_bits in proptest::collection::vec(any::<bool>(), 256),
        rhs_bits in proptest::collection::vec(any::<bool>(), 256),
    ) {
        let len = n * n;
        let lhs = matrix_from_bits(n, &lhs_bits[..len]);
        let rhs = matrix_from_bits(n, &rhs_bits[..len]);

        let actual = lhs.strassen_mul_for_test(&rhs, 1, 8);
        let expected = m4rm_multiply(&lhs, &rhs);

        prop_assert_eq!(actual, expected);
    }
}
