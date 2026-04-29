//! BitMatrix - A row-major, bit-packed boolean matrix for GF(2) operations.
//!
//! This module provides a memory-efficient matrix type where each element is a single bit,
//! stored in a row-major layout with bits packed into u64 words.

use std::fmt;
use std::ops::Mul;

// Route the PPC A1 design sizes (512 and 1024 columns, i.e. 8 and 16 words)
// through the AVX2-dispatched AND+popcount kernel when available. Benchmarked
// vs ppc-v0-2026-04-27: 512 cols 2.356x, 1024 cols 2.949x, geomean 2.636x.
#[cfg(feature = "simd")]
const MATVEC_SIMD_MIN_WORDS: usize = 8;

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
    /// Multiplies two matrices with the direct row-XOR accumulator.
    ///
    /// This is the non-M4RM fallback used when the Four Russians table would
    /// degenerate to single-row entries. It computes each output row as the XOR
    /// of rows of `rhs` selected by set bits in the corresponding row of `self`.
    /// For wide output rows, the row accumulation uses the same hoisted
    /// `LogicalFns::xor_fn` dispatch as the M4RM hot path.
    ///
    /// # Complexity
    ///
    /// O(nnz(`self`) × `rhs.cols().div_ceil(64)`) word operations. Dense inputs
    /// should use the M4RM path; this fallback is for narrow/degenerate panels
    /// where table precomputation does not buy reuse.
    #[inline(never)]
    pub(crate) fn mul_row_xor_dispatch(&self, rhs: &BitMatrix) -> BitMatrix {
        assert_eq!(
            self.cols,
            rhs.rows(),
            "incompatible dimensions: A is {}×{} but B is {}×{}",
            self.rows,
            self.cols,
            rhs.rows(),
            rhs.cols()
        );

        let mut out = BitMatrix::zeros(self.rows, rhs.cols());

        if self.rows == 0 || self.cols == 0 || rhs.cols() == 0 {
            return out;
        }

        let xor = crate::kernels::ops::resolve_xor_inplace(out.stride_words);

        for row in 0..self.rows {
            let lhs_row = self.row_words(row);
            let out_row = out.row_words_mut(row);

            for (word_idx, &word) in lhs_row.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let bit = bits.trailing_zeros() as usize;
                    let rhs_row = (word_idx << 6) + bit;
                    if rhs_row < self.cols {
                        xor(out_row, rhs.row_words(rhs_row));
                    }
                    bits &= bits - 1;
                }
            }
        }

        out
    }

    /// Test-support hook for exercising the non-M4RM row-XOR multiplier.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn mul_row_xor_for_test(&self, rhs: &BitMatrix) -> BitMatrix {
        self.mul_row_xor_dispatch(rhs)
    }

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

    /// Creates a matrix with all bits set to 1.
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
    /// let m = BitMatrix::ones(3, 4);
    /// assert_eq!(m.rows(), 3);
    /// assert_eq!(m.cols(), 4);
    /// assert!(m.get(0, 0));
    /// assert!(m.get(2, 3));
    /// ```
    pub fn ones(rows: usize, cols: usize) -> Self {
        let stride_words = if cols == 0 { 0 } else { cols.div_ceil(64) };
        let total_words = rows * stride_words;

        if total_words == 0 {
            return Self {
                data: vec![],
                rows,
                cols,
                stride_words,
            };
        }

        let mut data = vec![!0u64; total_words];

        // Mask padding bits in last word of each row
        if !cols.is_multiple_of(64) {
            let used_bits = cols % 64;
            let mask = (1u64 << used_bits) - 1;
            for row in 0..rows {
                let last_word_idx = row * stride_words + stride_words - 1;
                data[last_word_idx] &= mask;
            }
        }

        Self {
            data,
            rows,
            cols,
            stride_words,
        }
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

    #[inline]
    pub(crate) fn row_words_block_mut(&mut self, row_start: usize, row_count: usize) -> &mut [u64] {
        assert!(
            row_start <= self.rows && row_count <= self.rows - row_start,
            "row block {}..{} out of bounds (rows={})",
            row_start,
            row_start + row_count,
            self.rows
        );
        let start = row_start * self.stride_words;
        &mut self.data[start..start + row_count * self.stride_words]
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

    /// Returns all columns as u32 bitmasks.
    ///
    /// For each column j, bit i of the returned u32 is set iff `self.get(i, j)`
    /// is true. Useful for trellis-based decoders (BCJR) where each column of
    /// the parity-check matrix defines a state transition.
    ///
    /// # Panics
    ///
    /// Panics if the matrix has more than 32 rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::bitmatrix;
    ///
    /// let m = bitmatrix![
    ///     1, 0, 1;
    ///     0, 1, 1
    /// ];
    /// let masks = m.cols_as_u32_masks();
    /// assert_eq!(masks, vec![0b01, 0b10, 0b11]);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows * cols).
    pub fn cols_as_u32_masks(&self) -> Vec<u32> {
        assert!(
            self.rows <= 32,
            "Matrix has {} rows; column bitmasks require <= 32",
            self.rows
        );
        (0..self.cols)
            .map(|j| {
                let mut mask = 0u32;
                for i in 0..self.rows {
                    if self.get(i, j) {
                        mask |= 1 << i;
                    }
                }
                mask
            })
            .collect()
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

        // Use kernel xor_inplace which automatically dispatches to SIMD when available
        use crate::kernels::ops::xor_inplace;

        // Use split_at_mut to get non-overlapping slices for borrow checker
        if start_dst < start_src {
            let (left, right) = self.data.split_at_mut(start_src);
            xor_inplace(
                &mut left[start_dst..start_dst + self.stride_words],
                &right[..self.stride_words],
            );
        } else {
            let (left, right) = self.data.split_at_mut(start_dst);
            xor_inplace(
                &mut right[..self.stride_words],
                &left[start_src..start_src + self.stride_words],
            );
        }
    }

    /// Find the first row >= start_row that has a 1 in the given column.
    ///
    /// Uses word-level access for better performance than repeated get() calls.
    ///
    /// # Arguments
    ///
    /// * `col` - Column index to search
    /// * `start_row` - First row to check (inclusive)
    ///
    /// # Returns
    ///
    /// Row index if found, None if no such row exists.
    pub fn find_pivot_row(&self, col: usize, start_row: usize) -> Option<usize> {
        if col >= self.cols || start_row >= self.rows {
            return None;
        }

        let word_idx = col / 64;
        let bit_mask = 1u64 << (col % 64);

        for r in start_row..self.rows {
            let row_start = r * self.stride_words;
            if self.data[row_start + word_idx] & bit_mask != 0 {
                return Some(r);
            }
        }

        None
    }

    /// Check if a specific bit is set using word-level access (no bounds checking).
    ///
    /// This is faster than get() for inner loops where bounds are already known.
    ///
    /// # Safety
    ///
    /// This is a safe function but panics in debug mode if indices are out of bounds.
    /// In release mode, it performs no bounds checking for performance.
    #[inline]
    pub fn get_unchecked(&self, row: usize, col: usize) -> bool {
        debug_assert!(row < self.rows, "row {} out of bounds", row);
        debug_assert!(col < self.cols, "col {} out of bounds", col);

        let word_idx = row * self.stride_words + (col / 64);
        let bit_mask = 1u64 << (col % 64);

        self.data[word_idx] & bit_mask != 0
    }

    /// Returns the transpose of this matrix.
    ///
    /// The transpose of an m×n matrix is an n×m matrix where element (i,j)
    /// of the transpose equals element (j,i) of the original.
    ///
    /// # Implementation
    ///
    /// Uses a 64×64 bit-block transpose primitive driven from
    /// [`gf2_kernels_simd::transpose`]: an O(N log N) Hacker's Delight
    /// recursive bit-twiddle (V4) on the scalar fallback, and the measured
    /// AVX2 YMM bit-twiddle lane on x86_64 hosts that report AVX2 at
    /// runtime. A separate AVX2 PSHUFB byte-tile lane is kept in the SIMD
    /// crate for B1 artefact inspection, but production dispatch uses the
    /// faster measured bit-twiddle lane.
    ///
    /// The outer driver tiles the matrix into 64×64 bit-blocks, calls
    /// the kernel once per block, and writes the transposed block at
    /// the swapped tile coordinate in the output. Compared with the
    /// naive bit-by-bit double loop this drops the per-block cost from
    /// O(64²) gets/sets to O(64 log 64) word ops, a ~50–100× win for
    /// dense matrices on the order of 1024 cols.
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
    ///
    /// # Complexity
    ///
    /// O((rows × cols / 64²) · 64 · log₂ 64) word operations =
    /// O(rows · cols / 64) — linear in the bit count up to a small
    /// constant.
    pub fn transpose(&self) -> Self {
        if self.rows == 0 || self.cols == 0 {
            return Self::zeros(self.cols, self.rows);
        }

        // Resolve the dispatched 64×64 transpose kernel once. When the
        // `simd` feature is off, fall back to the always-available
        // scalar primitive directly.
        #[cfg(feature = "simd")]
        let transpose_64x64: fn(&[u64; 64], &mut [u64; 64]) = match crate::simd::maybe_transpose() {
            Some(fns) => fns.transpose_64x64,
            None => gf2_kernels_simd::transpose::transpose_64x64_scalar,
        };
        #[cfg(not(feature = "simd"))]
        let transpose_64x64: fn(&[u64; 64], &mut [u64; 64]) =
            gf2_kernels_simd::transpose::transpose_64x64_scalar;

        self.transpose_blocked(transpose_64x64)
    }

    /// Tiled transpose driver: walks 64×64 bit-blocks and dispatches each
    /// to `transpose_64x64`.
    ///
    /// Beyond the [`Self::TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS`] block
    /// count, the driver imposes an L1-friendly outer macro-tile so
    /// the (input row-band) × (output column-band) working set fits
    /// in L1d. Below the threshold it uses the simple 2-level block
    /// loop. The macro-tile size is chosen so that the input
    /// row-strip + output column-strip fits in ~64 KiB on Zen 3
    /// (32 KiB L1d × 2 ways shared between read + write).
    ///
    /// Factored out of [`Self::transpose`] so each PPC-spiral step
    /// can instrument the outer loop (V4 — no tiling, V3 — same,
    /// V7 — cache-tiled outer loop) without duplicating the per-block
    /// bit-packing logic.
    fn transpose_blocked(&self, transpose_64x64: fn(&[u64; 64], &mut [u64; 64])) -> Self {
        let mut out = Self::zeros(self.cols, self.rows);
        let in_stride = self.stride_words;
        let out_stride = out.stride_words;

        let n_row_blocks = self.rows.div_ceil(64);
        let n_col_blocks = self.cols.div_ceil(64);

        // V7: pick a macro-tile once either dimension spans enough
        // 64×64 blocks that the simple 2-level loop starts losing
        // cache locality. The threshold below is empirical for the B1
        // recovery measurements; small and medium matrices stay on the
        // simpler path.
        const MACRO_TILE_BLOCKS: usize = 8;

        if n_row_blocks <= Self::TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS
            && n_col_blocks <= Self::TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS
        {
            Self::transpose_inner_loop(
                &self.data,
                &mut out.data,
                in_stride,
                out_stride,
                self.rows,
                self.cols,
                0,
                n_row_blocks,
                0,
                n_col_blocks,
                transpose_64x64,
            );
        } else {
            // Macro-tiled outer loop: process MACRO_TILE_BLOCKS ×
            // MACRO_TILE_BLOCKS bit-blocks per macro-tile so the
            // per-tile input/output footprint stays L1-resident.
            let mut br_macro = 0usize;
            while br_macro < n_row_blocks {
                let br_end = (br_macro + MACRO_TILE_BLOCKS).min(n_row_blocks);
                let mut bc_macro = 0usize;
                while bc_macro < n_col_blocks {
                    let bc_end = (bc_macro + MACRO_TILE_BLOCKS).min(n_col_blocks);
                    Self::transpose_inner_loop(
                        &self.data,
                        &mut out.data,
                        in_stride,
                        out_stride,
                        self.rows,
                        self.cols,
                        br_macro,
                        br_end,
                        bc_macro,
                        bc_end,
                        transpose_64x64,
                    );
                    bc_macro = bc_end;
                }
                br_macro = br_end;
            }
        }

        // Mask the output's padding bits — the kernel may have written
        // bits beyond row count `self.rows` into the high bits of the
        // last `u64` of each output row; those must be zero per the
        // tail-mask invariant.
        out.mask_padding_bits();
        out
    }

    /// Threshold (in 64×64 bit-blocks) below which the transpose
    /// driver uses the simple 2-level block loop and above which it
    /// engages the V7 macro-tile outer loop.
    ///
    /// Tuned from the recovered B1 benchmark sweep: matrices up to
    /// 16 blocks (= 1024 rows/cols) in both dimensions use the simple
    /// loop, while larger matrices use the macro-tiled driver.
    const TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS: usize = 16;

    /// Inner loop over a (br, bc) range of 64×64 bit-blocks.
    ///
    /// Allocates a single tile pair on the stack per invocation;
    /// shared across the macro-tile and direct-loop paths in
    /// [`Self::transpose_blocked`].
    #[allow(clippy::too_many_arguments)]
    fn transpose_inner_loop(
        in_data: &[u64],
        out_data: &mut [u64],
        in_stride: usize,
        out_stride: usize,
        rows: usize,
        cols: usize,
        br_start: usize,
        br_end: usize,
        bc_start: usize,
        bc_end: usize,
        transpose_64x64: fn(&[u64; 64], &mut [u64; 64]),
    ) {
        let mut tile_in = [0u64; 64];
        let mut tile_out = [0u64; 64];

        for br in br_start..br_end {
            let row_start = br * 64;
            let row_end = (row_start + 64).min(rows);
            let block_rows = row_end - row_start;

            for bc in bc_start..bc_end {
                let col_start = bc * 64;
                let col_end = (col_start + 64).min(cols);
                let block_cols = col_end - col_start;

                // Load the input tile: bit (i, j) of the block lives in
                // bit `j` of `data[(row_start + i) * in_stride + bc]`.
                // Rows beyond the matrix end pad to zero.
                for (i, slot) in tile_in.iter_mut().enumerate().take(block_rows) {
                    *slot = in_data[(row_start + i) * in_stride + bc];
                }
                for slot in tile_in.iter_mut().take(64).skip(block_rows) {
                    *slot = 0;
                }

                transpose_64x64(&tile_in, &mut tile_out);

                // Write the transposed tile: bit (j, i) of the
                // transposed block lives in bit `i` of `tile_out[j]`,
                // which maps to row `(col_start + j)` and word `br` of
                // the output. Rows beyond `block_cols` aren't written
                // because they don't exist in the transposed matrix.
                for (j, &word) in tile_out.iter().enumerate().take(block_cols) {
                    out_data[(col_start + j) * out_stride + br] = word;
                }
            }
        }
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

    /// Compute matrix-vector product: y = A × x over GF(2).
    ///
    /// For an m×n matrix A, computes the product with vector x (n bits).
    /// Returns vector y of length m.
    ///
    /// # Arguments
    ///
    /// * `x` - Input bit vector of length n (must equal self.cols())
    ///
    /// # Returns
    ///
    /// Output bit vector of length m (equals self.rows())
    ///
    /// # Panics
    ///
    /// Panics if x.len() != self.cols()
    ///
    /// # Performance
    ///
    /// Uses word-level operations (64-bit) for efficiency. Each row is processed
    /// by XORing masked words from the input vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitMatrix, BitVec};
    ///
    /// let mut a = BitMatrix::zeros(2, 3);
    /// a.set(0, 0, true);  // Row 0: [1 0 1]
    /// a.set(0, 2, true);
    /// a.set(1, 1, true);  // Row 1: [0 1 1]
    /// a.set(1, 2, true);
    ///
    /// let mut x = BitVec::new();
    /// x.push_bit(true);   // [1, 1, 1]
    /// x.push_bit(true);
    /// x.push_bit(true);
    ///
    /// let y = a.matvec(&x);
    /// assert_eq!(y.len(), 2);
    /// // Row 0: 1^0^1 = 0
    /// assert_eq!(y.get(0), false);
    /// // Row 1: 0^1^1 = 0
    /// assert_eq!(y.get(1), false);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows × cols) in the worst case, but optimized with word-level operations.
    pub fn matvec(&self, x: &crate::BitVec) -> crate::BitVec {
        assert_eq!(x.len(), self.cols, "input BitVec length must equal cols");

        #[cfg(feature = "simd")]
        if self.stride_words >= MATVEC_SIMD_MIN_WORDS {
            if let Some(fns) = crate::simd::maybe_simd() {
                return self.matvec_simd(x, fns);
            }
        }

        self.matvec_scalar(x)
    }

    #[inline]
    fn matvec_scalar(&self, x: &crate::BitVec) -> crate::BitVec {
        let mut y = crate::BitVec::with_capacity(self.rows);

        for r in 0..self.rows {
            let row_offset = r * self.stride_words;
            let row = &self.data[row_offset..row_offset + self.stride_words];
            let bit = Self::row_dot_parity_scalar(row, x.words());
            y.push_bit(bit);
        }

        y
    }

    #[inline]
    fn row_dot_parity_scalar(row: &[u64], x_words: &[u64]) -> bool {
        let mut acc0 = 0u64;
        let mut acc1 = 0u64;
        let mut acc2 = 0u64;
        let mut acc3 = 0u64;

        let mut chunks = row.chunks_exact(4);
        let mut x_chunks = x_words.chunks_exact(4);
        for (r, x) in chunks.by_ref().zip(x_chunks.by_ref()) {
            acc0 ^= r[0] & x[0];
            acc1 ^= r[1] & x[1];
            acc2 ^= r[2] & x[2];
            acc3 ^= r[3] & x[3];
        }

        let mut acc = acc0 ^ acc1 ^ acc2 ^ acc3;
        for (&r, &x) in chunks.remainder().iter().zip(x_chunks.remainder()) {
            acc ^= r & x;
        }

        acc.count_ones() & 1 == 1
    }

    #[cfg(feature = "simd")]
    #[inline(never)]
    fn matvec_simd(&self, x: &crate::BitVec, fns: &gf2_kernels_simd::LogicalFns) -> crate::BitVec {
        let x_words = x.words();
        debug_assert_eq!(x_words.len(), self.stride_words);

        let mut y = crate::BitVec::with_capacity(self.rows);

        for row in self.data.chunks_exact(self.stride_words).take(self.rows) {
            y.push_bit((fns.and_popcnt_fn)(row, x_words) & 1 == 1);
        }

        y
    }

    /// Compute matrix-vector product with transpose: y = A^T × x over GF(2).
    ///
    /// For an m×n matrix A, computes the product of A^T (n×m) with vector x (m bits).
    /// Returns vector y of length n.
    ///
    /// # Arguments
    ///
    /// * `x` - Input bit vector of length m (must equal self.rows())
    ///
    /// # Returns
    ///
    /// Output bit vector of length n (equals self.cols())
    ///
    /// # Panics
    ///
    /// Panics if x.len() != self.rows()
    ///
    /// # Performance
    ///
    /// Processes 64 columns at a time using word-level operations. This optimization
    /// provides 10-15× speedup over bit-by-bit column iteration by exploiting the
    /// row-major memory layout and processing entire words at once.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitMatrix, BitVec};
    ///
    /// let mut a = BitMatrix::zeros(2, 3);
    /// a.set(0, 0, true);  // Row 0: [1 0 1]
    /// a.set(0, 2, true);
    /// a.set(1, 1, true);  // Row 1: [0 1 1]
    /// a.set(1, 2, true);
    ///
    /// let mut x = BitVec::new();
    /// x.push_bit(true);   // [1, 0]
    /// x.push_bit(false);
    ///
    /// let y = a.matvec_transpose(&x);
    /// assert_eq!(y.len(), 3);
    /// // Col 0: [1, 0] dot [1, 0] = 1
    /// assert_eq!(y.get(0), true);
    /// // Col 1: [0, 1] dot [1, 0] = 0
    /// assert_eq!(y.get(1), false);
    /// // Col 2: [1, 1] dot [1, 0] = 1
    /// assert_eq!(y.get(2), true);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows × stride_words) - processes columns in blocks of 64.
    pub fn matvec_transpose(&self, x: &crate::BitVec) -> crate::BitVec {
        assert_eq!(x.len(), self.rows, "input BitVec length must equal rows");

        let mut y = crate::BitVec::with_capacity(self.cols);

        // Process 64 columns at a time (one word)
        for word_idx in 0..self.stride_words {
            let col_start = word_idx * 64;
            let col_end = (col_start + 64).min(self.cols);

            // Accumulate XOR of all rows where x[r] = 1
            let mut block_result = 0u64;

            for r in 0..self.rows {
                if !x.get(r) {
                    continue; // Skip rows where x[r] = 0
                }

                let row_offset = r * self.stride_words;
                let word = self.data[row_offset + word_idx];
                block_result ^= word;
            }

            // Unpack block_result into individual column bits
            let num_cols_in_block = col_end - col_start;
            for bit_idx in 0..num_cols_in_block {
                let bit = (block_result & (1u64 << bit_idx)) != 0;
                y.push_bit(bit);
            }
        }

        y
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

impl crate::matrix_like::MatrixLike<bool> for BitMatrix {
    type Owned = BitMatrix;

    #[inline]
    fn rows(&self) -> usize {
        BitMatrix::rows(self)
    }

    #[inline]
    fn cols(&self) -> usize {
        BitMatrix::cols(self)
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> bool {
        BitMatrix::get(self, row, col)
    }

    #[inline]
    fn transpose(&self) -> Self {
        BitMatrix::transpose(self)
    }
}

impl crate::matrix_like::MatrixLikeMut<bool> for BitMatrix {
    #[inline]
    fn set(&mut self, row: usize, col: usize, v: bool) {
        BitMatrix::set(self, row, col, v);
    }

    #[inline]
    fn swap_rows(&mut self, r1: usize, r2: usize) {
        BitMatrix::swap_rows(self, r1, r2);
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
    fn test_ones() {
        let m = BitMatrix::ones(3, 4);
        assert_eq!(m.rows(), 3);
        assert_eq!(m.cols(), 4);
        // All bits should be set
        for r in 0..3 {
            for c in 0..4 {
                assert!(m.get(r, c), "Bit at ({}, {}) should be 1", r, c);
            }
        }
    }

    #[test]
    fn test_ones_edge_cases() {
        // Single element
        let m = BitMatrix::ones(1, 1);
        assert!(m.get(0, 0));

        // Non-word-aligned columns
        let m = BitMatrix::ones(2, 65);
        assert!(m.get(1, 64));

        // Empty matrix
        let m = BitMatrix::ones(0, 0);
        assert_eq!(m.rows(), 0);
        assert_eq!(m.cols(), 0);
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

    fn random_bitvec(len: usize, seed: u64) -> crate::BitVec {
        crate::BitVec::random_seeded(len, seed)
    }

    proptest! {
        #[test]
        fn prop_transpose_double_is_identity(
            rows in 0..130usize,
            cols in 0..130usize,
            seed in any::<u64>()
        ) {
            let m = BitMatrix::random_seeded(rows, cols, seed);
            let transposed_twice = m.transpose().transpose();
            prop_assert_eq!(transposed_twice, m);
        }

        #[test]
        fn prop_transpose_matvec_semantics(
            rows in 0..130usize,
            cols in 0..130usize,
            matrix_seed in any::<u64>(),
            vector_seed in any::<u64>()
        ) {
            let m = BitMatrix::random_seeded(rows, cols, matrix_seed);
            let v = random_bitvec(cols, vector_seed);

            let direct = m.matvec(&v);
            let via_transpose = m.transpose().matvec_transpose(&v);

            prop_assert_eq!(via_transpose, direct);
        }

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
