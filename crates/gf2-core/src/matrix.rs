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

    /// Gets a word from the matrix at the specified row and word index.
    ///
    /// # Panics
    ///
    /// Panics if row >= rows or word_idx >= stride_words.
    #[inline]
    pub(crate) fn get_word(&self, row: usize, word_idx: usize) -> u64 {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        assert!(
            word_idx < self.stride_words,
            "word_idx {} out of bounds (stride_words={})",
            word_idx,
            self.stride_words
        );
        self.data[row * self.stride_words + word_idx]
    }

    /// Sets a word in the matrix at the specified row and word index.
    ///
    /// # Panics
    ///
    /// Panics if row >= rows or word_idx >= stride_words.
    #[inline]
    pub(crate) fn set_word(&mut self, row: usize, word_idx: usize, word: u64) {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        assert!(
            word_idx < self.stride_words,
            "word_idx {} out of bounds (stride_words={})",
            word_idx,
            self.stride_words
        );
        self.data[row * self.stride_words + word_idx] = word;
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

    /// Extracts a row as a BitVec.
    ///
    /// Creates a new BitVec containing all column values from the specified row.
    ///
    /// # Arguments
    ///
    /// * `row` - Row index (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(3, 4);
    /// m.set(1, 0, true);
    /// m.set(1, 2, true);
    ///
    /// let row = m.row_as_bitvec(1);
    /// assert_eq!(row.len(), 4);
    /// assert!(row.get(0));
    /// assert!(!row.get(1));
    /// assert!(row.get(2));
    /// assert!(!row.get(3));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(cols) - iterates through all columns in the row
    pub fn row_as_bitvec(&self, row: usize) -> crate::BitVec {
        assert!(
            row < self.rows,
            "Row index {} out of bounds (rows: {})",
            row,
            self.rows
        );

        let mut bits = crate::BitVec::new();
        for col in 0..self.cols {
            bits.push_bit(self.get(row, col));
        }
        bits
    }

    /// Extracts a column as a BitVec.
    ///
    /// Creates a new BitVec containing all row values from the specified column.
    ///
    /// # Arguments
    ///
    /// * `col` - Column index (0-based)
    ///
    /// # Panics
    ///
    /// Panics if `col >= self.cols()`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(4, 3);
    /// m.set(0, 1, true);
    /// m.set(2, 1, true);
    ///
    /// let col = m.col_as_bitvec(1);
    /// assert_eq!(col.len(), 4);
    /// assert!(col.get(0));
    /// assert!(!col.get(1));
    /// assert!(col.get(2));
    /// assert!(!col.get(3));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows) - iterates through all rows in the column
    pub fn col_as_bitvec(&self, col: usize) -> crate::BitVec {
        assert!(
            col < self.cols,
            "Column index {} out of bounds (cols: {})",
            col,
            self.cols
        );

        let mut bits = crate::BitVec::new();
        for row in 0..self.rows {
            bits.push_bit(self.get(row, col));
        }
        bits
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

    /// XOR row `src` into row `dst` (word-level operation).
    ///
    /// Performs: `dst_row ^= src_row` over GF(2).
    ///
    /// # Arguments
    ///
    /// * `dst` - Destination row index (will be modified)
    /// * `src` - Source row index (will be XOR'd into dst)
    ///
    /// # Panics
    ///
    /// Panics if `dst` or `src` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let mut m = BitMatrix::zeros(2, 3);
    /// m.set(0, 0, true);
    /// m.set(1, 1, true);
    ///
    /// m.row_xor(1, 0);  // row1 ^= row0
    /// assert!(m.get(1, 0));  // Now row1 has bit 0 set
    /// assert!(m.get(1, 1));  // And still has bit 1 set
    /// ```
    pub fn row_xor(&mut self, dst: usize, src: usize) {
        assert!(
            dst < self.rows,
            "dst row index {} out of bounds (rows={})",
            dst,
            self.rows
        );
        assert!(
            src < self.rows,
            "src row index {} out of bounds (rows={})",
            src,
            self.rows
        );

        if dst == src {
            // XOR'ing a row with itself yields all zeros - just clear the row
            let start = dst * self.stride_words;
            for i in 0..self.stride_words {
                self.data[start + i] = 0;
            }
            return;
        }

        let start_dst = dst * self.stride_words;
        let start_src = src * self.stride_words;

        // XOR words from src into dst
        for i in 0..self.stride_words {
            self.data[start_dst + i] ^= self.data[start_src + i];
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

    /// Converts this dense matrix to a CSR SpBitMatrix.
    ///
    /// This scans all bits and records set columns per row. Suitable for low-density matrices.
    ///
    /// # Examples
    /// ```
    /// use gf2_core::matrix::BitMatrix;
    /// let mut m = BitMatrix::zeros(2, 3);
    /// m.set(0, 1, true);
    /// let s = m.to_sparse();
    /// assert_eq!(s.rows(), 2);
    /// assert_eq!(s.cols(), 3);
    /// assert_eq!(s.nnz(), 1);
    /// ```
    pub fn to_sparse(&self) -> crate::sparse::SpBitMatrix {
        crate::sparse::SpBitMatrix::from_dense(self)
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

#[cfg(feature = "visualization")]
impl BitMatrix {
    /// Saves the matrix as a PNG image.
    ///
    /// Each bit is represented as a single pixel:
    /// - Unset bits (0) → black (0, 0, 0)
    /// - Set bits (1) → white (255, 255, 255)
    ///
    /// # Arguments
    ///
    /// * `path` - Output file path (e.g., "matrix.png")
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_core::matrix::BitMatrix;
    ///
    /// let m = BitMatrix::identity(100);
    /// m.save_image("identity.png").unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be created
    /// - PNG encoding fails
    ///
    /// # Note
    ///
    /// To modify colors, edit the hard-coded `ZERO_COLOR` and `ONE_COLOR` constants
    /// in the implementation.
    pub fn save_image(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use image::{ImageBuffer, Rgb};

        const ZERO_COLOR: [u8; 3] = [0, 0, 0]; // black
        const ONE_COLOR: [u8; 3] = [255, 255, 255]; // white

        let mut img = ImageBuffer::new(self.cols as u32, self.rows as u32);

        for row in 0..self.rows {
            for col in 0..self.cols {
                let bit = self.get(row, col);
                let color = if bit { ONE_COLOR } else { ZERO_COLOR };
                img.put_pixel(col as u32, row as u32, Rgb(color));
            }
        }

        img.save(path)?;
        Ok(())
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

    // Row/column extraction tests

    #[test]
    fn test_row_as_bitvec_identity() {
        let m = BitMatrix::identity(4);

        let row0 = m.row_as_bitvec(0);
        assert_eq!(row0.len(), 4);
        assert!(row0.get(0));
        assert!(!row0.get(1));
        assert!(!row0.get(2));
        assert!(!row0.get(3));

        let row2 = m.row_as_bitvec(2);
        assert_eq!(row2.len(), 4);
        assert!(!row2.get(0));
        assert!(!row2.get(1));
        assert!(row2.get(2));
        assert!(!row2.get(3));
    }

    #[test]
    fn test_row_as_bitvec_zeros() {
        let m = BitMatrix::zeros(3, 5);

        let row = m.row_as_bitvec(1);
        assert_eq!(row.len(), 5);
        for i in 0..5 {
            assert!(!row.get(i), "Bit {} should be false", i);
        }
    }

    #[test]
    fn test_row_as_bitvec_custom_pattern() {
        let mut m = BitMatrix::zeros(3, 5);
        m.set(1, 0, true);
        m.set(1, 2, true);
        m.set(1, 4, true);

        let row = m.row_as_bitvec(1);
        assert_eq!(row.len(), 5);
        assert!(row.get(0));
        assert!(!row.get(1));
        assert!(row.get(2));
        assert!(!row.get(3));
        assert!(row.get(4));
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_row_as_bitvec_out_of_bounds() {
        let m = BitMatrix::zeros(3, 4);
        let _ = m.row_as_bitvec(3);
    }

    #[test]
    fn test_col_as_bitvec_identity() {
        let m = BitMatrix::identity(4);

        let col0 = m.col_as_bitvec(0);
        assert_eq!(col0.len(), 4);
        assert!(col0.get(0));
        assert!(!col0.get(1));
        assert!(!col0.get(2));
        assert!(!col0.get(3));

        let col2 = m.col_as_bitvec(2);
        assert_eq!(col2.len(), 4);
        assert!(!col2.get(0));
        assert!(!col2.get(1));
        assert!(col2.get(2));
        assert!(!col2.get(3));
    }

    #[test]
    fn test_col_as_bitvec_zeros() {
        let m = BitMatrix::zeros(5, 3);

        let col = m.col_as_bitvec(1);
        assert_eq!(col.len(), 5);
        for i in 0..5 {
            assert!(!col.get(i), "Bit {} should be false", i);
        }
    }

    #[test]
    fn test_col_as_bitvec_custom_pattern() {
        let mut m = BitMatrix::zeros(5, 3);
        m.set(0, 1, true);
        m.set(2, 1, true);
        m.set(4, 1, true);

        let col = m.col_as_bitvec(1);
        assert_eq!(col.len(), 5);
        assert!(col.get(0));
        assert!(!col.get(1));
        assert!(col.get(2));
        assert!(!col.get(3));
        assert!(col.get(4));
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_col_as_bitvec_out_of_bounds() {
        let m = BitMatrix::zeros(3, 4);
        let _ = m.col_as_bitvec(4);
    }

    #[test]
    fn test_row_col_extraction_consistency() {
        let mut m = BitMatrix::zeros(4, 4);
        m.set(0, 1, true);
        m.set(1, 0, true);
        m.set(2, 3, true);
        m.set(3, 2, true);

        // Extract all rows and verify against original
        for r in 0..4 {
            let row = m.row_as_bitvec(r);
            for c in 0..4 {
                assert_eq!(
                    row.get(c),
                    m.get(r, c),
                    "Row extraction mismatch at ({}, {})",
                    r,
                    c
                );
            }
        }

        // Extract all columns and verify against original
        for c in 0..4 {
            let col = m.col_as_bitvec(c);
            for r in 0..4 {
                assert_eq!(
                    col.get(r),
                    m.get(r, c),
                    "Column extraction mismatch at ({}, {})",
                    r,
                    c
                );
            }
        }
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_row_extraction_preserves_values(
            rows in 1..20usize,
            cols in 1..20usize,
            seed in any::<u64>()
        ) {
            let m = BitMatrix::random_seeded(rows, cols, seed);

            for r in 0..rows {
                let row_vec = m.row_as_bitvec(r);
                assert_eq!(row_vec.len(), cols);

                for c in 0..cols {
                    assert_eq!(row_vec.get(c), m.get(r, c),
                        "Mismatch at ({}, {})", r, c);
                }
            }
        }

        #[test]
        fn prop_col_extraction_preserves_values(
            rows in 1..20usize,
            cols in 1..20usize,
            seed in any::<u64>()
        ) {
            let m = BitMatrix::random_seeded(rows, cols, seed);

            for c in 0..cols {
                let col_vec = m.col_as_bitvec(c);
                assert_eq!(col_vec.len(), rows);

                for r in 0..rows {
                    assert_eq!(col_vec.get(r), m.get(r, c),
                        "Mismatch at ({}, {})", r, c);
                }
            }
        }
    }
}
