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

    strassen_rec(Square::full(a), Square::full(b), 0, cfg)
}

#[derive(Clone, Copy)]
struct Square<'a> {
    matrix: &'a BitMatrix,
    row0: usize,
    col0: usize,
    size: usize,
}

impl<'a> Square<'a> {
    fn full(matrix: &'a BitMatrix) -> Self {
        debug_assert_eq!(matrix.rows(), matrix.cols());
        Self {
            matrix,
            row0: 0,
            col0: 0,
            size: matrix.rows(),
        }
    }

    fn sub(self, row: usize, col: usize, size: usize) -> Self {
        debug_assert!(row + size <= self.size);
        debug_assert!(col + size <= self.size);
        Self {
            matrix: self.matrix,
            row0: self.row0 + row,
            col0: self.col0 + col,
            size,
        }
    }

    fn is_full(self) -> bool {
        self.row0 == 0
            && self.col0 == 0
            && self.size == self.matrix.rows()
            && self.size == self.matrix.cols()
    }

    fn is_word_aligned(self) -> bool {
        self.col0.is_multiple_of(64) && self.size.is_multiple_of(64)
    }

    fn word_row(self, row: usize) -> &'a [u64] {
        debug_assert!(row < self.size);
        debug_assert!(self.is_word_aligned());
        let word0 = self.col0 / 64;
        let words = self.size / 64;
        &self.matrix.row_words(self.row0 + row)[word0..word0 + words]
    }

    fn get(self, row: usize, col: usize) -> bool {
        debug_assert!(row < self.size);
        debug_assert!(col < self.size);
        self.matrix.get(self.row0 + row, self.col0 + col)
    }

    fn to_matrix(self) -> BitMatrix {
        let mut out = BitMatrix::zeros(self.size, self.size);

        if self.size == 0 {
            return out;
        }

        if self.is_word_aligned() {
            for row in 0..self.size {
                out.row_words_mut(row).copy_from_slice(self.word_row(row));
            }
        } else {
            for row in 0..self.size {
                for col in 0..self.size {
                    out.set(row, col, self.get(row, col));
                }
            }
        }

        out
    }
}

fn strassen_rec(a: Square<'_>, b: Square<'_>, depth: usize, cfg: StrassenConfig) -> BitMatrix {
    let n = a.size;
    debug_assert_eq!(b.size, n);
    if !should_recurse(n, depth, cfg) {
        return leaf_multiply(a, b);
    }

    let h = n / 2;

    let a11 = a.sub(0, 0, h);
    let a12 = a.sub(0, h, h);
    let a21 = a.sub(h, 0, h);
    let a22 = a.sub(h, h, h);
    let b11 = b.sub(0, 0, h);
    let b12 = b.sub(0, h, h);
    let b21 = b.sub(h, 0, h);
    let b22 = b.sub(h, h, h);

    let mut out = BitMatrix::zeros(n, n);

    {
        let a11_xor_a22 = xor_squares(a11, a22);
        let b11_xor_b22 = xor_squares(b11, b22);
        let m1 = strassen_rec(
            Square::full(&a11_xor_a22),
            Square::full(&b11_xor_b22),
            depth + 1,
            cfg,
        );
        xor_into_quadrant(&mut out, &m1, 0, 0);
        xor_into_quadrant(&mut out, &m1, h, h);
    }

    {
        let a21_xor_a22 = xor_squares(a21, a22);
        let m2 = strassen_rec(Square::full(&a21_xor_a22), b11, depth + 1, cfg);
        xor_into_quadrant(&mut out, &m2, h, 0);
        xor_into_quadrant(&mut out, &m2, h, h);
    }

    {
        let b12_xor_b22 = xor_squares(b12, b22);
        let m3 = strassen_rec(a11, Square::full(&b12_xor_b22), depth + 1, cfg);
        xor_into_quadrant(&mut out, &m3, 0, h);
        xor_into_quadrant(&mut out, &m3, h, h);
    }

    {
        let b21_xor_b11 = xor_squares(b21, b11);
        let m4 = strassen_rec(a22, Square::full(&b21_xor_b11), depth + 1, cfg);
        xor_into_quadrant(&mut out, &m4, 0, 0);
        xor_into_quadrant(&mut out, &m4, h, 0);
    }

    {
        let a11_xor_a12 = xor_squares(a11, a12);
        let m5 = strassen_rec(Square::full(&a11_xor_a12), b22, depth + 1, cfg);
        xor_into_quadrant(&mut out, &m5, 0, 0);
        xor_into_quadrant(&mut out, &m5, 0, h);
    }

    {
        let a21_xor_a11 = xor_squares(a21, a11);
        let b11_xor_b12 = xor_squares(b11, b12);
        let m6 = strassen_rec(
            Square::full(&a21_xor_a11),
            Square::full(&b11_xor_b12),
            depth + 1,
            cfg,
        );
        xor_into_quadrant(&mut out, &m6, h, h);
    }

    {
        let a12_xor_a22 = xor_squares(a12, a22);
        let b21_xor_b22 = xor_squares(b21, b22);
        let m7 = strassen_rec(
            Square::full(&a12_xor_a22),
            Square::full(&b21_xor_b22),
            depth + 1,
            cfg,
        );
        xor_into_quadrant(&mut out, &m7, 0, 0);
    }

    out
}

fn should_recurse(n: usize, depth: usize, cfg: StrassenConfig) -> bool {
    n > cfg.leaf_n && depth < cfg.max_depth && n > 1 && n.is_multiple_of(2)
}

fn leaf_multiply(a: Square<'_>, b: Square<'_>) -> BitMatrix {
    match (a.is_full(), b.is_full()) {
        (true, true) => crate::alg::m4rm::multiply(a.matrix, b.matrix),
        (true, false) => {
            let b_owned = b.to_matrix();
            crate::alg::m4rm::multiply(a.matrix, &b_owned)
        }
        (false, true) => {
            let a_owned = a.to_matrix();
            crate::alg::m4rm::multiply(&a_owned, b.matrix)
        }
        (false, false) => {
            let a_owned = a.to_matrix();
            let b_owned = b.to_matrix();
            crate::alg::m4rm::multiply(&a_owned, &b_owned)
        }
    }
}

fn xor_squares(a: Square<'_>, b: Square<'_>) -> BitMatrix {
    debug_assert_eq!(a.size, b.size);
    let mut out = BitMatrix::zeros(a.size, a.size);

    if a.size == 0 {
        return out;
    }

    if a.is_word_aligned() && b.is_word_aligned() {
        let xor = crate::kernels::ops::resolve_xor_inplace(out.stride_words());
        for row in 0..a.size {
            let out_row = out.row_words_mut(row);
            out_row.copy_from_slice(a.word_row(row));
            xor(out_row, b.word_row(row));
        }
    } else {
        for row in 0..a.size {
            for col in 0..a.size {
                out.set(row, col, a.get(row, col) ^ b.get(row, col));
            }
        }
    }

    out
}

fn xor_into_quadrant(dst: &mut BitMatrix, src: &BitMatrix, row0: usize, col0: usize) {
    let size = src.rows();
    debug_assert_eq!(src.cols(), size);

    if size == 0 {
        return;
    }

    if col0.is_multiple_of(64) && size.is_multiple_of(64) {
        let dst_word0 = col0 / 64;
        let words = size / 64;
        let xor = crate::kernels::ops::resolve_xor_inplace(words);
        for row in 0..size {
            let dst_row = dst.row_words_mut(row0 + row);
            xor(
                &mut dst_row[dst_word0..dst_word0 + words],
                src.row_words(row),
            );
        }
    } else {
        for row in 0..size {
            for col in 0..size {
                let bit = dst.get(row0 + row, col0 + col) ^ src.get(row, col);
                dst.set(row0 + row, col0 + col, bit);
            }
        }
    }
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
