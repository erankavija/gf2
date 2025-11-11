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
/// use gf2_core::matrix::BitMatrix;
///
/// // Create a 3x4 zero matrix
/// let mut m = BitMatrix::zeros(3, 4);
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
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let m = BitMatrix::zeros(10, 20);
    /// assert_eq!(m.rows(), 10);
    /// assert_eq!(m.cols(), 20);
    /// ```
    pub fn zeros(rows: usize, cols: usize) -> Self {
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
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let id = BitMatrix::identity(3);
    /// assert_eq!(id.get(0, 0), true);
    /// assert_eq!(id.get(1, 1), true);
    /// assert_eq!(id.get(0, 1), false);
    /// ```
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, true);
        }
        m
    }

    /// Creates a `BitMatrix` with random bits using the provided RNG.
    ///
    /// Each bit has probability 0.5 of being set. For custom probabilities,
    /// use [`BitMatrix::random_with_probability`].
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of rows
    /// * `cols` - Number of columns
    /// * `rng` - A mutable reference to a random number generator
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::matrix::BitMatrix;
    /// use rand::rngs::StdRng;
    /// use rand::SeedableRng;
    ///
    /// let mut rng = StdRng::seed_from_u64(42);
    /// let m = BitMatrix::random(10, 20, &mut rng);
    /// assert_eq!(m.rows(), 10);
    /// assert_eq!(m.cols(), 20);
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows × stride_words) where stride_words = ⌈cols / 64⌉.
    #[cfg(feature = "rand")]
    pub fn random<R: rand::Rng>(rows: usize, cols: usize, rng: &mut R) -> Self {
        let mut m = Self::zeros(rows, cols);
        if !m.data.is_empty() {
            rng.fill(&mut m.data[..]);
            m.mask_padding_bits();
        }
        m
    }

    /// Creates a `BitMatrix` with random bits using a seeded RNG.
    ///
    /// This provides deterministic random generation - the same seed
    /// will always produce the same matrix.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of rows
    /// * `cols` - Number of columns
    /// * `seed` - Seed value for the random number generator
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let m1 = BitMatrix::random_seeded(10, 20, 0x1234);
    /// let m2 = BitMatrix::random_seeded(10, 20, 0x1234);
    /// assert_eq!(m1, m2); // Same seed produces same matrix
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows × stride_words) where stride_words = ⌈cols / 64⌉.
    #[cfg(feature = "rand")]
    pub fn random_seeded(rows: usize, cols: usize, seed: u64) -> Self {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng = StdRng::seed_from_u64(seed);
        Self::random(rows, cols, &mut rng)
    }

    /// Creates a `BitMatrix` with random bits where each bit is set with probability `p`.
    ///
    /// For `p = 0.5`, prefer [`BitMatrix::random`] which is optimized for the uniform case.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of rows
    /// * `cols` - Number of columns
    /// * `p` - Probability in [0.0, 1.0] that each bit is set to 1
    /// * `rng` - A mutable reference to a random number generator
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in the range [0.0, 1.0].
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::matrix::BitMatrix;
    /// use rand::rngs::StdRng;
    /// use rand::SeedableRng;
    ///
    /// let mut rng = StdRng::seed_from_u64(42);
    /// // Create a sparse matrix (~10% ones)
    /// let m = BitMatrix::random_with_probability(100, 100, 0.1, &mut rng);
    /// assert_eq!(m.rows(), 100);
    /// assert_eq!(m.cols(), 100);
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows × cols). Note that this is slower than [`BitMatrix::random`]
    /// for the default p=0.5 case.
    #[cfg(feature = "rand")]
    pub fn random_with_probability<R: rand::Rng>(
        rows: usize,
        cols: usize,
        p: f64,
        rng: &mut R,
    ) -> Self {
        assert!(
            (0.0..=1.0).contains(&p),
            "Probability must be in range [0.0, 1.0], got {}",
            p
        );

        let mut m = Self::zeros(rows, cols);

        // Fast paths for extreme probabilities
        if p == 0.0 {
            return m;
        }
        if p == 1.0 {
            for word in &mut m.data {
                *word = u64::MAX;
            }
            m.mask_padding_bits();
            return m;
        }

        // For p=0.5, use optimized word-level generation
        if (p - 0.5).abs() < 1e-10 {
            return Self::random(rows, cols, rng);
        }

        // General case: generate bits individually
        for r in 0..rows {
            for c in 0..cols {
                if rng.gen_bool(p) {
                    m.set(r, c, true);
                }
            }
        }
        m
    }

    /// Fills this `BitMatrix` with random bits using the provided RNG.
    ///
    /// The dimensions of the matrix remain unchanged.
    ///
    /// # Arguments
    ///
    /// * `rng` - A mutable reference to a random number generator
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::matrix::BitMatrix;
    /// use rand::rngs::StdRng;
    /// use rand::SeedableRng;
    ///
    /// let mut m = BitMatrix::zeros(10, 10);
    /// let mut rng = StdRng::seed_from_u64(42);
    /// m.fill_random(&mut rng);
    /// // m now contains random bits
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows × stride_words).
    #[cfg(feature = "rand")]
    pub fn fill_random<R: rand::Rng>(&mut self, rng: &mut R) {
        if !self.data.is_empty() {
            rng.fill(&mut self.data[..]);
            self.mask_padding_bits();
        }
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
    /// use gf2_core::matrix::BitMatrix;
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
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(3, 3);
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
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(2, 128);
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
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(2, 128);
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
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(3, 3);
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
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(2, 3);
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
        let mut result = Self::zeros(self.cols, self.rows);

        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.get(r, c) {
                    result.set(c, r, true);
                }
            }
        }

        result
    }

    /// Masks padding bits in each row to zero.
    ///
    /// This maintains the invariant that bits beyond `cols` in each row
    /// are always zero. Called internally after bulk operations.
    fn mask_padding_bits(&mut self) {
        if self.cols == 0 || self.stride_words == 0 {
            return;
        }

        let used_bits_in_last_word = self.cols % 64;
        if used_bits_in_last_word == 0 {
            return; // No padding bits
        }

        let mask = (1u64 << used_bits_in_last_word) - 1;
        let last_word_idx = self.stride_words - 1;

        for row in 0..self.rows {
            let offset = row * self.stride_words + last_word_idx;
            self.data[offset] &= mask;
        }
    }
}

impl fmt::Display for BitMatrix {
    /// Formats the BitMatrix in nalgebra-like style.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(3, 4);
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

impl Mul<BitMatrix> for BitMatrix {
    type Output = BitMatrix;

    /// Matrix multiplication: `A * B`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::matrix::BitMatrix;
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

    fn mul(self, rhs: &BitMatrix) -> BitMatrix {
        crate::alg::m4rm::multiply(&self, rhs)
    }
}

impl Mul<BitMatrix> for &BitMatrix {
    type Output = BitMatrix;

    fn mul(self, rhs: BitMatrix) -> BitMatrix {
        crate::alg::m4rm::multiply(self, &rhs)
    }
}

impl Mul<&BitMatrix> for &BitMatrix {
    type Output = BitMatrix;

    fn mul(self, rhs: &BitMatrix) -> BitMatrix {
        crate::alg::m4rm::multiply(self, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeros() {
        let m = BitMatrix::zeros(5, 10);
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
        let mut m = BitMatrix::zeros(2, 3);
        m.set(0, 1, true);
        assert!(m.get(0, 1));
        assert!(!m.get(0, 0));

        m.set(0, 1, false);
        assert!(!m.get(0, 1));
    }

    #[test]
    fn test_mul_operator_identity() {
        // Test A * I = A
        let mut a = BitMatrix::zeros(3, 4);
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
        let mut a = BitMatrix::zeros(2, 3);
        a.set(0, 0, true);
        a.set(0, 1, true);
        a.set(1, 1, true);
        a.set(1, 2, true);

        let mut b = BitMatrix::zeros(3, 2);
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
