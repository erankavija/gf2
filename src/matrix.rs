//! BitMatrix - A row-major, bit-packed boolean matrix for GF(2) operations.
//!
//! This module provides a memory-efficient matrix type where each element is a single bit,
//! stored in a row-major layout with bits packed into u64 words.

use std::fmt;
use std::ops::Mul;

/// A row-major, bit-packed boolean matrix.
///
/// # Storage Layout
///
/// - Bits are stored row-major in a `Vec<u64>`.
/// - Each row occupies `stride_words` full u64 words (padded to word boundary).
/// - Within each word, bits are stored in little-endian order (bit 0 = LSB).
/// - Bit at position `(r, c)` is stored at:
///   - Word index: `r * stride_words + (c / 64)`
///   - Bit offset: `c % 64`
///
/// # Examples
///
/// ```
/// use gf2::matrix::BitMatrix;
///
/// // Create a 3x4 zero matrix
/// let mut m = BitMatrix::new_zero(3, 4);
/// m.set(0, 0, true);
/// m.set(1, 2, true);
/// assert_eq!(m.get(0, 0), true);
/// assert_eq!(m.get(1, 2), true);
/// assert_eq!(m.get(0, 1), false);
///
/// // Create a 4x4 identity matrix
/// let id = BitMatrix::identity(4);
/// assert_eq!(id.get(0, 0), true);
/// assert_eq!(id.get(0, 1), false);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitMatrix {
    data: Vec<u64>,
    rows: usize,
    cols: usize,
    stride_words: usize,
}

impl BitMatrix {
    /// Creates a new zero-initialized matrix with the given dimensions.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of rows
    /// * `cols` - Number of columns
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let m = BitMatrix::new_zero(10, 20);
    /// assert_eq!(m.rows(), 10);
    /// assert_eq!(m.cols(), 20);
    /// ```
    pub fn new_zero(rows: usize, cols: usize) -> Self {
        let stride_words = if cols == 0 { 0 } else { cols.div_ceil(64) };
        let total_words = rows * stride_words;
        Self {
            data: vec![0u64; total_words],
            rows,
            cols,
            stride_words,
        }
    }

    /// Creates an n×n identity matrix.
    ///
    /// # Arguments
    ///
    /// * `n` - Size of the square identity matrix
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let id = BitMatrix::identity(3);
    /// assert_eq!(id.get(0, 0), true);
    /// assert_eq!(id.get(1, 1), true);
    /// assert_eq!(id.get(0, 1), false);
    /// ```
    pub fn identity(n: usize) -> Self {
        let mut m = Self::new_zero(n, n);
        for i in 0..n {
            m.set(i, i, true);
        }
        m
    }

    /// Returns the number of rows.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Returns the number of columns.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Returns the number of u64 words per row (stride).
    #[inline]
    pub fn stride_words(&self) -> usize {
        self.stride_words
    }

    /// Gets the bit value at position (row, col).
    ///
    /// # Panics
    ///
    /// Panics if row >= rows or col >= cols.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let m = BitMatrix::identity(3);
    /// assert_eq!(m.get(0, 0), true);
    /// assert_eq!(m.get(0, 1), false);
    /// ```
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> bool {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        assert!(
            col < self.cols,
            "col index {} out of bounds (cols={})",
            col,
            self.cols
        );

        let word_idx = row * self.stride_words + (col / 64);
        let bit_offset = col % 64;
        (self.data[word_idx] & (1u64 << bit_offset)) != 0
    }

    /// Sets the bit value at position (row, col).
    ///
    /// # Panics
    ///
    /// Panics if row >= rows or col >= cols.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::new_zero(3, 3);
    /// m.set(1, 2, true);
    /// assert_eq!(m.get(1, 2), true);
    /// ```
    #[inline]
    pub fn set(&mut self, row: usize, col: usize, val: bool) {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        assert!(
            col < self.cols,
            "col index {} out of bounds (cols={})",
            col,
            self.cols
        );

        let word_idx = row * self.stride_words + (col / 64);
        let bit_offset = col % 64;
        let mask = 1u64 << bit_offset;

        if val {
            self.data[word_idx] |= mask;
        } else {
            self.data[word_idx] &= !mask;
        }
    }

    /// Returns an immutable slice of the u64 words for the given row.
    ///
    /// # Panics
    ///
    /// Panics if row >= rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::new_zero(2, 128);
    /// m.set(0, 64, true);
    /// let words = m.row_words(0);
    /// assert_eq!(words.len(), 2);
    /// assert_eq!(words[1] & 1, 1);
    /// ```
    #[inline]
    pub fn row_words(&self, row: usize) -> &[u64] {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        let start = row * self.stride_words;
        &self.data[start..start + self.stride_words]
    }

    /// Returns a mutable slice of the u64 words for the given row.
    ///
    /// # Panics
    ///
    /// Panics if row >= rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::new_zero(2, 128);
    /// {
    ///     let words = m.row_words_mut(0);
    ///     words[0] = 0xFF;
    /// }
    /// assert_eq!(m.get(0, 0), true);
    /// assert_eq!(m.get(0, 7), true);
    /// ```
    #[inline]
    pub fn row_words_mut(&mut self, row: usize) -> &mut [u64] {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        let start = row * self.stride_words;
        &mut self.data[start..start + self.stride_words]
    }

    /// Swaps two rows in the matrix.
    ///
    /// # Panics
    ///
    /// Panics if r1 >= rows or r2 >= rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::new_zero(3, 3);
    /// m.set(0, 0, true);
    /// m.set(1, 1, true);
    /// m.swap_rows(0, 1);
    /// assert_eq!(m.get(0, 0), false);
    /// assert_eq!(m.get(0, 1), true);
    /// assert_eq!(m.get(1, 0), true);
    /// ```
    pub fn swap_rows(&mut self, r1: usize, r2: usize) {
        assert!(
            r1 < self.rows,
            "row index {} out of bounds (rows={})",
            r1,
            self.rows
        );
        assert!(
            r2 < self.rows,
            "row index {} out of bounds (rows={})",
            r2,
            self.rows
        );

        if r1 == r2 {
            return;
        }

        let start1 = r1 * self.stride_words;
        let start2 = r2 * self.stride_words;

        // Swap words in the two rows
        for i in 0..self.stride_words {
            self.data.swap(start1 + i, start2 + i);
        }
    }

    /// Returns the transpose of this matrix.
    ///
    /// The transpose of an m×n matrix is an n×m matrix where element (i,j)
    /// of the transpose equals element (j,i) of the original.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::new_zero(2, 3);
    /// m.set(0, 1, true);
    /// m.set(1, 2, true);
    ///
    /// let mt = m.transpose();
    /// assert_eq!(mt.rows(), 3);
    /// assert_eq!(mt.cols(), 2);
    /// assert_eq!(mt.get(1, 0), true);
    /// assert_eq!(mt.get(2, 1), true);
    /// ```
    pub fn transpose(&self) -> Self {
        let mut result = Self::new_zero(self.cols, self.rows);

        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.get(r, c) {
                    result.set(c, r, true);
                }
            }
        }

        result
    }
}

impl fmt::Display for BitMatrix {
    /// Formats the BitMatrix in nalgebra-like style.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::new_zero(3, 4);
    /// m.set(0, 0, true);
    /// m.set(0, 3, true);
    /// m.set(1, 1, true);
    /// m.set(2, 2, true);
    /// println!("{}", m);
    /// // Displays:
    /// //   ┌       ┐
    /// //   │ 1 0 0 1 │
    /// //   │ 0 1 0 0 │
    /// //   │ 0 0 1 0 │
    /// //   └       ┘
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rows == 0 || self.cols == 0 {
            return write!(f, "[ ]");
        }

        // Border width: each column takes 2 chars (digit + space), plus 1 for final space
        let border_width = self.cols * 2 + 1;

        // Top border
        writeln!(f, "  ┌{}┐", " ".repeat(border_width))?;

        // Matrix rows
        for r in 0..self.rows {
            write!(f, "  │ ")?;
            for c in 0..self.cols {
                if self.get(r, c) {
                    write!(f, "1")?;
                } else {
                    write!(f, "0")?;
                }
                if c < self.cols - 1 {
                    write!(f, " ")?;
                }
            }
            writeln!(f, " │")?;
        }

        // Bottom border
        write!(f, "  └{}┘", " ".repeat(border_width))
    }
}

// Implement Mul trait for matrix multiplication using the M4RM algorithm
// This enables the infix `*` operator for matrix multiplication

impl Mul<BitMatrix> for BitMatrix {
    type Output = BitMatrix;

    /// Matrix multiplication: `A * B`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let a = BitMatrix::identity(3);
    /// let b = BitMatrix::identity(3);
    /// let c = a * b;
    /// assert_eq!(c, BitMatrix::identity(3));
    /// ```
    fn mul(self, rhs: BitMatrix) -> BitMatrix {
        crate::alg::m4rm::multiply(&self, &rhs)
    }
}

impl Mul<&BitMatrix> for BitMatrix {
    type Output = BitMatrix;

    /// Matrix multiplication: `A * &B`
    fn mul(self, rhs: &BitMatrix) -> BitMatrix {
        crate::alg::m4rm::multiply(&self, rhs)
    }
}

impl Mul<BitMatrix> for &BitMatrix {
    type Output = BitMatrix;

    /// Matrix multiplication: `&A * B`
    fn mul(self, rhs: BitMatrix) -> BitMatrix {
        crate::alg::m4rm::multiply(self, &rhs)
    }
}

impl Mul<&BitMatrix> for &BitMatrix {
    type Output = BitMatrix;

    /// Matrix multiplication: `&A * &B`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2::matrix::BitMatrix;
    ///
    /// let a = BitMatrix::identity(3);
    /// let b = BitMatrix::identity(3);
    /// let c = &a * &b;
    /// assert_eq!(c, BitMatrix::identity(3));
    /// ```
    fn mul(self, rhs: &BitMatrix) -> BitMatrix {
        crate::alg::m4rm::multiply(self, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_zero() {
        let m = BitMatrix::new_zero(5, 10);
        assert_eq!(m.rows(), 5);
        assert_eq!(m.cols(), 10);
        assert_eq!(m.stride_words(), 1);
    }

    #[test]
    fn test_identity() {
        let m = BitMatrix::identity(3);
        assert!(m.get(0, 0));
        assert!(m.get(1, 1));
        assert!(m.get(2, 2));
        assert!(!m.get(0, 1));
        assert!(!m.get(1, 0));
    }

    #[test]
    fn test_get_set() {
        let mut m = BitMatrix::new_zero(2, 3);
        m.set(0, 1, true);
        assert!(m.get(0, 1));
        assert!(!m.get(0, 0));

        m.set(0, 1, false);
        assert!(!m.get(0, 1));
    }

    #[test]
    fn test_mul_operator_identity() {
        // Test A * I = A
        let mut a = BitMatrix::new_zero(3, 4);
        a.set(0, 1, true);
        a.set(1, 2, true);
        a.set(2, 3, true);

        let i = BitMatrix::identity(4);
        let c = &a * &i;

        assert_eq!(c.rows(), 3);
        assert_eq!(c.cols(), 4);
        for r in 0..3 {
            for col in 0..4 {
                assert_eq!(c.get(r, col), a.get(r, col));
            }
        }
    }

    #[test]
    fn test_mul_operator_owned() {
        // Test owned values: A * B
        let a = BitMatrix::identity(3);
        let b = BitMatrix::identity(3);
        let c = a * b;

        assert_eq!(c, BitMatrix::identity(3));
    }

    #[test]
    fn test_mul_operator_mixed_refs() {
        // Test mixed references
        let a = BitMatrix::identity(2);
        let b = BitMatrix::identity(2);

        // A * &B
        let c1 = a.clone() * &b;
        assert_eq!(c1, BitMatrix::identity(2));

        // &A * B
        let c2 = &a * b.clone();
        assert_eq!(c2, BitMatrix::identity(2));

        // &A * &B
        let c3 = &a * &b;
        assert_eq!(c3, BitMatrix::identity(2));
    }

    #[test]
    fn test_mul_operator_rectangular() {
        // Test 2x3 * 3x2 = 2x2
        let mut a = BitMatrix::new_zero(2, 3);
        a.set(0, 0, true);
        a.set(0, 1, true);
        a.set(1, 1, true);
        a.set(1, 2, true);

        let mut b = BitMatrix::new_zero(3, 2);
        b.set(0, 0, true);
        b.set(1, 1, true);
        b.set(2, 0, true);

        let c = &a * &b;

        assert_eq!(c.rows(), 2);
        assert_eq!(c.cols(), 2);

        // Verify against expected result
        assert!(c.get(0, 0));
        assert!(c.get(0, 1));
        assert!(c.get(1, 0));
        assert!(c.get(1, 1));
    }
}
