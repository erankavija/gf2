//! High-level GF(2) matrix multiplication dispatch.
//!
//! This layer keeps the existing M4RM implementation as the leaf multiplier
//! and selects a Strassen-family recursion only for square matrices above the
//! measured crossover.

use crate::matrix::BitMatrix;

const STRASSEN_AUTO_MIN_N: usize = 16_384;
const STRASSEN_AUTO_LEAF_N: usize = 2048;
const STRASSEN_AUTO_MAX_DEPTH: usize = 1;

pub(crate) fn multiply(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    if should_use_strassen(a, b) {
        crate::alg::strassen::multiply_square_with_config(
            a,
            b,
            crate::alg::strassen::StrassenConfig {
                leaf_n: STRASSEN_AUTO_LEAF_N,
                max_depth: STRASSEN_AUTO_MAX_DEPTH,
            },
        )
    } else {
        crate::alg::m4rm::multiply(a, b)
    }
}

fn should_use_strassen(a: &BitMatrix, b: &BitMatrix) -> bool {
    let n = a.rows();

    a.cols() == n
        && b.rows() == n
        && b.cols() == n
        && n >= STRASSEN_AUTO_MIN_N
        && n.is_power_of_two()
        && (n / 2).is_multiple_of(64)
}

#[cfg(test)]
mod tests {
    use super::should_use_strassen;
    use crate::matrix::BitMatrix;

    #[test]
    fn auto_strassen_predicate_is_square_power_of_two_crossover() {
        let square_2048 = BitMatrix::zeros(2048, 2048);
        assert!(!should_use_strassen(&square_2048, &square_2048));

        let square_4096 = BitMatrix::zeros(4096, 4096);
        assert!(!should_use_strassen(&square_4096, &square_4096));

        let square_8192 = BitMatrix::zeros(8192, 8192);
        assert!(!should_use_strassen(&square_8192, &square_8192));

        let square_16384 = BitMatrix::zeros(16_384, 16_384);
        assert!(should_use_strassen(&square_16384, &square_16384));
    }

    #[test]
    fn auto_strassen_predicate_rejects_rectangular_and_non_power_of_two() {
        let lhs_rect = BitMatrix::zeros(4096, 4095);
        let rhs_rect = BitMatrix::zeros(4095, 4096);
        assert!(!should_use_strassen(&lhs_rect, &rhs_rect));

        let non_power = BitMatrix::zeros(4097, 4097);
        assert!(!should_use_strassen(&non_power, &non_power));
    }
}
