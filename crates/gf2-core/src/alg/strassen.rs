//! Strassen-family multiplication for square GF(2) matrices.
//!
//! The implementation is intentionally private to the high-level matrix
//! multiplier. Recursive leaves call the existing M4RM path, preserving all
//! scalar/SIMD fallback behaviour below the selected crossover.

use crate::matrix::BitMatrix;

#[derive(Clone, Copy, Debug)]
pub(crate) struct StrassenConfig {
    pub(crate) leaf_n: usize,
    pub(crate) max_depth: usize,
}

pub(crate) fn multiply_square_with_config(
    a: &BitMatrix,
    b: &BitMatrix,
    cfg: StrassenConfig,
) -> BitMatrix {
    let n = a.rows();
    assert_eq!(
        a.cols(),
        n,
        "Strassen multiplication requires a square left matrix, got {}×{}",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        b.rows(),
        n,
        "incompatible dimensions: A is {}×{} but B is {}×{}",
        a.rows(),
        a.cols(),
        b.rows(),
        b.cols()
    );
    assert_eq!(
        b.cols(),
        n,
        "Strassen multiplication requires a square right matrix, got {}×{}",
        b.rows(),
        b.cols()
    );

    strassen_rec(a, b, 0, cfg)
}

fn strassen_rec(a: &BitMatrix, b: &BitMatrix, depth: usize, cfg: StrassenConfig) -> BitMatrix {
    let n = a.rows();
    if !should_recurse(n, depth, cfg) {
        return crate::alg::m4rm::multiply(a, b);
    }

    let h = n / 2;

    let a11 = copy_quadrant(a, 0, 0, h);
    let a12 = copy_quadrant(a, 0, h, h);
    let a21 = copy_quadrant(a, h, 0, h);
    let a22 = copy_quadrant(a, h, h, h);

    let b11 = copy_quadrant(b, 0, 0, h);
    let b12 = copy_quadrant(b, 0, h, h);
    let b21 = copy_quadrant(b, h, 0, h);
    let b22 = copy_quadrant(b, h, h, h);

    let a11_xor_a22 = xor_matrices(&a11, &a22);
    let b11_xor_b22 = xor_matrices(&b11, &b22);
    let m1 = strassen_rec(&a11_xor_a22, &b11_xor_b22, depth + 1, cfg);

    let a21_xor_a22 = xor_matrices(&a21, &a22);
    let m2 = strassen_rec(&a21_xor_a22, &b11, depth + 1, cfg);

    let b12_xor_b22 = xor_matrices(&b12, &b22);
    let m3 = strassen_rec(&a11, &b12_xor_b22, depth + 1, cfg);

    let b21_xor_b11 = xor_matrices(&b21, &b11);
    let m4 = strassen_rec(&a22, &b21_xor_b11, depth + 1, cfg);

    let a11_xor_a12 = xor_matrices(&a11, &a12);
    let m5 = strassen_rec(&a11_xor_a12, &b22, depth + 1, cfg);

    let a21_xor_a11 = xor_matrices(&a21, &a11);
    let b11_xor_b12 = xor_matrices(&b11, &b12);
    let m6 = strassen_rec(&a21_xor_a11, &b11_xor_b12, depth + 1, cfg);

    let a12_xor_a22 = xor_matrices(&a12, &a22);
    let b21_xor_b22 = xor_matrices(&b21, &b22);
    let m7 = strassen_rec(&a12_xor_a22, &b21_xor_b22, depth + 1, cfg);

    let c11 = xor_many([&m1, &m4, &m5, &m7]);
    let c12 = xor_matrices(&m3, &m5);
    let c21 = xor_matrices(&m2, &m4);
    let c22 = xor_many([&m1, &m2, &m3, &m6]);

    assemble_quadrants(&c11, &c12, &c21, &c22)
}

fn should_recurse(n: usize, depth: usize, cfg: StrassenConfig) -> bool {
    n > cfg.leaf_n && depth < cfg.max_depth && n > 1 && n.is_multiple_of(2)
}

fn copy_quadrant(src: &BitMatrix, row0: usize, col0: usize, size: usize) -> BitMatrix {
    let mut out = BitMatrix::zeros(size, size);

    if size == 0 {
        return out;
    }

    if col0.is_multiple_of(64) && size.is_multiple_of(64) {
        let src_word0 = col0 / 64;
        let words = size / 64;
        for row in 0..size {
            let src_row = src.row_words(row0 + row);
            out.row_words_mut(row)
                .copy_from_slice(&src_row[src_word0..src_word0 + words]);
        }
    } else {
        for row in 0..size {
            for col in 0..size {
                out.set(row, col, src.get(row0 + row, col0 + col));
            }
        }
    }

    out
}

fn copy_into_quadrant(dst: &mut BitMatrix, src: &BitMatrix, row0: usize, col0: usize) {
    let size = src.rows();

    if size == 0 {
        return;
    }

    if col0.is_multiple_of(64) && size.is_multiple_of(64) {
        let dst_word0 = col0 / 64;
        let words = size / 64;
        for row in 0..size {
            let dst_row = dst.row_words_mut(row0 + row);
            dst_row[dst_word0..dst_word0 + words].copy_from_slice(src.row_words(row));
        }
    } else {
        for row in 0..size {
            for col in 0..size {
                dst.set(row0 + row, col0 + col, src.get(row, col));
            }
        }
    }
}

fn xor_matrices(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    assert_eq!(a.rows(), b.rows());
    assert_eq!(a.cols(), b.cols());

    let mut out = BitMatrix::zeros(a.rows(), a.cols());
    let xor = crate::kernels::ops::resolve_xor_inplace(out.stride_words());
    for row in 0..a.rows() {
        out.row_words_mut(row).copy_from_slice(a.row_words(row));
        xor(out.row_words_mut(row), b.row_words(row));
    }
    out
}

fn xor_into(dst: &mut BitMatrix, src: &BitMatrix) {
    assert_eq!(dst.rows(), src.rows());
    assert_eq!(dst.cols(), src.cols());

    let xor = crate::kernels::ops::resolve_xor_inplace(dst.stride_words());
    for row in 0..dst.rows() {
        xor(dst.row_words_mut(row), src.row_words(row));
    }
}

fn xor_many<const N: usize>(matrices: [&BitMatrix; N]) -> BitMatrix {
    assert!(N > 0);
    let mut out = BitMatrix::zeros(matrices[0].rows(), matrices[0].cols());
    for row in 0..out.rows() {
        out.row_words_mut(row)
            .copy_from_slice(matrices[0].row_words(row));
    }
    for matrix in matrices.into_iter().skip(1) {
        xor_into(&mut out, matrix);
    }
    out
}

fn assemble_quadrants(
    c11: &BitMatrix,
    c12: &BitMatrix,
    c21: &BitMatrix,
    c22: &BitMatrix,
) -> BitMatrix {
    let h = c11.rows();
    let mut out = BitMatrix::zeros(2 * h, 2 * h);
    copy_into_quadrant(&mut out, c11, 0, 0);
    copy_into_quadrant(&mut out, c12, 0, h);
    copy_into_quadrant(&mut out, c21, h, 0);
    copy_into_quadrant(&mut out, c22, h, h);
    out
}

#[cfg(test)]
mod tests {
    use super::{multiply_square_with_config, StrassenConfig};
    use crate::alg::m4rm;
    use crate::matrix::BitMatrix;

    fn patterned_square(n: usize, salt: usize) -> BitMatrix {
        let mut matrix = BitMatrix::zeros(n, n);
        for row in 0..n {
            for col in 0..n {
                matrix.set(row, col, ((row * 17 + col * 31 + salt) & 7) <= 2);
            }
        }
        matrix
    }

    #[test]
    fn forced_strassen_handles_small_recursive_square() {
        let lhs = patterned_square(8, 1);
        let rhs = patterned_square(8, 2);
        let cfg = StrassenConfig {
            leaf_n: 1,
            max_depth: 8,
        };

        assert_eq!(
            multiply_square_with_config(&lhs, &rhs, cfg),
            m4rm::multiply(&lhs, &rhs)
        );
    }

    #[test]
    fn forced_strassen_handles_word_aligned_square() {
        let lhs = patterned_square(128, 3);
        let rhs = patterned_square(128, 4);
        let cfg = StrassenConfig {
            leaf_n: 64,
            max_depth: 2,
        };

        assert_eq!(
            multiply_square_with_config(&lhs, &rhs, cfg),
            m4rm::multiply(&lhs, &rhs)
        );
    }
}
