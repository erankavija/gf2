//! BitMatrix - A row-major, bit-packed boolean matrix for GF(2) operations.

use crate::tuning;
use std::fmt;
use std::ops::Mul;
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicUsize, Ordering};

/// Conservative default definition for `bit_matrix.matvec_simd_min_words`.
///
/// The matvec selector uses this value in ordinary builds; the
/// `gf2_tuning_baked` build configuration substitutes the baked counterpart
/// at the selection site. The conservative profile names this constant as its
/// default definition.
pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;

#[cfg(all(feature = "simd", gf2_tuning_baked))]
const MATVEC_SIMD_MIN_WORDS_SELECTED: usize = crate::tuning::baked::MATVEC_SIMD_MIN_WORDS;

#[cfg(all(feature = "simd", not(gf2_tuning_baked)))]
const MATVEC_SIMD_MIN_WORDS_SELECTED: usize = MATVEC_SIMD_MIN_WORDS;

/// Conservative default definition for `bit_matrix.transpose_macro_tile_blocks`.
///
/// The active profile's `transpose_macro_tile_blocks` value replaces this
/// default at the transpose dispatch entry.
pub(crate) const MACRO_TILE_BLOCKS: usize = 8;

/// The selected arm of the [`BitMatrix::matvec`] dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatvecRoute {
    /// Use the scalar row-parity implementation.
    Scalar,
    /// Attempt the SIMD row-parity implementation when it is available.
    Simd,
}

/// Reports the word-count arm of [`BitMatrix::matvec`].
///
/// The comparison uses the baked `bit_matrix.matvec_simd_min_words` value.
/// Runtime profile installation does not move this boundary; the dispatcher
/// still falls back to the scalar implementation when SIMD is unavailable.
#[must_use]
pub fn matvec_route(stride_words: usize) -> MatvecRoute {
    #[cfg(feature = "simd")]
    {
        if stride_words >= MATVEC_SIMD_MIN_WORDS_SELECTED {
            return MatvecRoute::Simd;
        }
    }

    #[cfg(not(feature = "simd"))]
    let _ = stride_words;
    MatvecRoute::Scalar
}

/// The selected outer-loop strategy of [`BitMatrix::transpose`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransposeRoute {
    /// Use the direct two-level block loop.
    Simple,
    /// Use the cache-tiled outer loop with the resolved macro-tile extent.
    MacroTiled {
        /// Number of 64×64 blocks in each outer-loop edge.
        macro_tile_blocks: usize,
    },
}

/// Test-only observation of the route completed by [`BitMatrix::transpose`].
///
/// This records executed work after the transpose and its tail masking finish;
/// it does not select a route. It is intended for one public operation in a
/// fresh child process. Concurrent transposes have no attribution guarantee.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransposeEffectiveObservation {
    /// No transpose has completed since the observation was reset.
    None,
    /// The direct two-level block loop completed.
    Simple,
    /// The macro-tiled loop completed with this consumed tile extent.
    MacroTiled {
        /// Number of 64×64 blocks consumed along each macro-tile edge.
        macro_tile_blocks: usize,
    },
}

#[cfg(any(test, feature = "test-support"))]
static TRANSPOSE_EFFECTIVE_KIND: AtomicUsize = AtomicUsize::new(0);
#[cfg(any(test, feature = "test-support"))]
static TRANSPOSE_EFFECTIVE_MACRO_TILE_BLOCKS: AtomicUsize = AtomicUsize::new(0);

#[cfg(any(test, feature = "test-support"))]
fn record_transpose_effective_observation(route: TransposeRoute) {
    match route {
        TransposeRoute::Simple => TRANSPOSE_EFFECTIVE_KIND.store(1, Ordering::Release),
        TransposeRoute::MacroTiled { macro_tile_blocks } => {
            TRANSPOSE_EFFECTIVE_MACRO_TILE_BLOCKS.store(macro_tile_blocks, Ordering::Relaxed);
            TRANSPOSE_EFFECTIVE_KIND.store(2, Ordering::Release);
        }
    }
}

/// Resets the test-only completed-transpose observation.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_transpose_effective_observation() {
    TRANSPOSE_EFFECTIVE_MACRO_TILE_BLOCKS.store(0, Ordering::Relaxed);
    TRANSPOSE_EFFECTIVE_KIND.store(0, Ordering::Release);
}

/// Reads the route completed by the most recent observed transpose.
///
/// Call this after exactly one public [`BitMatrix::transpose`] following
/// [`reset_transpose_effective_observation`].
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn transpose_effective_observation() -> TransposeEffectiveObservation {
    match TRANSPOSE_EFFECTIVE_KIND.load(Ordering::Acquire) {
        0 => TransposeEffectiveObservation::None,
        1 => TransposeEffectiveObservation::Simple,
        2 => TransposeEffectiveObservation::MacroTiled {
            macro_tile_blocks: TRANSPOSE_EFFECTIVE_MACRO_TILE_BLOCKS.load(Ordering::Relaxed),
        },
        value => panic!("invalid effective transpose observation {value}"),
    }
}

impl TransposeRoute {
    /// Returns the macro-tile extent consumed by the cache-tiled route.
    ///
    /// The simple route has no macro-tile extent and returns `None`.
    #[must_use]
    pub fn macro_tile_blocks(self) -> Option<usize> {
        match self {
            Self::Simple => None,
            Self::MacroTiled { macro_tile_blocks } => Some(macro_tile_blocks),
        }
    }
}

/// Reports the block-count arm of [`BitMatrix::transpose`].
///
/// The two selector values are read once from the active
/// `bit_matrix.transpose_simple_max_blocks` and
/// `bit_matrix.transpose_macro_tile_blocks` profile fields. The transpose
/// dispatcher calls this reporter and consumes the returned macro-tile
/// extent.
#[must_use]
pub fn transpose_route(n_row_blocks: usize, n_col_blocks: usize) -> TransposeRoute {
    let tuning = tuning::active();
    let selectors = tuning.bit_matrix();
    transpose_route_resolved(
        selectors.transpose_simple_max_blocks(),
        selectors.transpose_macro_tile_blocks(),
        n_row_blocks,
        n_col_blocks,
    )
}

/// The 64×64 block kernel and lane [`BitMatrix::transpose`] resolves: under
/// the `simd` cargo feature the lane `gf2_kernels_simd::transpose::detect`
/// publishes, otherwise the portable kernel.
fn resolved_transpose_block() -> (
    gf2_kernels_simd::transpose::Transpose64x64Fn,
    gf2_kernels_simd::transpose::TransposeLane,
) {
    use gf2_kernels_simd::transpose::{transpose_64x64_scalar, TransposeLane};

    #[cfg(feature = "simd")]
    if let Some(fns) = crate::simd::maybe_transpose() {
        return (fns.transpose_64x64, fns.lane);
    }
    (transpose_64x64_scalar, TransposeLane::Scalar)
}

/// Reports the 64×64 block lane [`BitMatrix::transpose`] runs on this host
/// in this build.
#[must_use]
pub fn transpose_block_lane() -> gf2_kernels_simd::transpose::TransposeLane {
    resolved_transpose_block().1
}

/// Reports the transpose arm against already-resolved selector values.
fn transpose_route_resolved(
    transpose_simple_max_blocks: usize,
    transpose_macro_tile_blocks: usize,
    n_row_blocks: usize,
    n_col_blocks: usize,
) -> TransposeRoute {
    if n_row_blocks <= transpose_simple_max_blocks && n_col_blocks <= transpose_simple_max_blocks {
        TransposeRoute::Simple
    } else {
        TransposeRoute::MacroTiled {
            macro_tile_blocks: transpose_macro_tile_blocks,
        }
    }
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitMatrix {
    data: Vec<u64>,
    rows: usize,
    cols: usize,
    stride_words: usize,
}

impl BitMatrix {
    /// Multiplies two matrices with the direct row-XOR accumulator: each output
    /// row is the XOR of the rows of `rhs` selected by the set bits of the
    /// corresponding row of `self`.
    ///
    /// # Complexity
    ///
    /// O(nnz(`self`) × `rhs.cols().div_ceil(64)`) word operations.
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
    ///
    /// # Panics
    ///
    /// Panics if `self.cols() != rhs.rows()`.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn mul_row_xor_for_test(&self, rhs: &BitMatrix) -> BitMatrix {
        self.mul_row_xor_dispatch(rhs)
    }

    /// Test-support hook for exercising the Strassen-family multiplier.
    ///
    /// # Panics
    ///
    /// Panics unless `self` and `rhs` are square with the same dimension.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn strassen_mul_for_test(
        &self,
        rhs: &BitMatrix,
        leaf_n: usize,
        max_depth: usize,
    ) -> BitMatrix {
        crate::alg::strassen::multiply_square_with_config(
            self,
            rhs,
            crate::alg::strassen::StrassenConfig { leaf_n, max_depth },
        )
    }

    /// Creates a new zero-initialized matrix with the given dimensions.
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
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, true);
        }
        m
    }

    /// Creates a matrix with all bits set to 1.
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
    #[cfg(feature = "rand")]
    pub fn random<R: rand::Rng>(rows: usize, cols: usize, rng: &mut R) -> Self {
        let mut m = Self::zeros(rows, cols);
        if !m.data.is_empty() {
            rng.fill(&mut m.data[..]);
            m.mask_padding_bits();
        }
        m
    }

    /// Creates a `BitMatrix` with random bits from an RNG seeded with `seed`;
    /// equal seeds give equal matrices
    /// (`test_bitmatrix_random_seeded_deterministic`).
    #[cfg(feature = "rand")]
    pub fn random_seeded(rows: usize, cols: usize, seed: u64) -> Self {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng = StdRng::seed_from_u64(seed);
        Self::random(rows, cols, &mut rng)
    }

    /// Creates a `BitMatrix` with random bits where each bit is set with probability `p`.
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in the range [0.0, 1.0].
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
        if (p - 0.5).abs() < 1e-10 {
            return Self::random(rows, cols, rng);
        }
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
    #[cfg(feature = "rand")]
    pub fn fill_random<R: rand::Rng>(&mut self, rng: &mut R) {
        if !self.data.is_empty() {
            rng.fill(&mut self.data[..]);
            self.mask_padding_bits();
        }
    }

    /// Number of rows.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of `u64` words per row.
    #[inline]
    pub fn stride_words(&self) -> usize {
        self.stride_words
    }

    /// Gets the bit value at position (row, col).
    ///
    /// # Panics
    ///
    /// Panics if row >= rows or col >= cols.
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
    #[cfg(feature = "io")]
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
    #[cfg(feature = "io")]
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

    /// Returns a contiguous immutable slice spanning `row_count` rows from
    /// `row_start`, in row-major (`stride_words`-strided) layout.
    #[cfg(feature = "simd")]
    #[inline]
    pub(crate) fn row_words_block(&self, row_start: usize, row_count: usize) -> &[u64] {
        assert!(
            row_start <= self.rows && row_count <= self.rows - row_start,
            "row block {}..{} out of bounds (rows={})",
            row_start,
            row_start + row_count,
            self.rows
        );
        let start = row_start * self.stride_words;
        &self.data[start..start + row_count * self.stride_words]
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
    /// # Panics
    ///
    /// Panics if `row >= self.rows()`
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
    /// # Panics
    ///
    /// Panics if `col >= self.cols()`
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
    /// is true.
    ///
    /// # Panics
    ///
    /// Panics if the matrix has more than 32 rows.
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

        for i in 0..self.stride_words {
            self.data.swap(start1 + i, start2 + i);
        }
    }

    /// XOR row `src` into row `dst` (word-level operation).
    ///
    /// # Panics
    ///
    /// Panics if `dst` or `src` is out of bounds.
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

        use crate::kernels::ops::xor_inplace;
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

    /// XORs row `src` into row `dst`, starting at `start_word`.
    ///
    /// Skipping words before the active pivot is valid once earlier pivot
    /// columns are cleared. A no-op when `dst == src`.
    pub(crate) fn row_xor_from(&mut self, dst: usize, src: usize, start_word: usize) {
        debug_assert!(dst < self.rows, "dst row {} out of bounds", dst);
        debug_assert!(src < self.rows, "src row {} out of bounds", src);
        debug_assert!(
            start_word <= self.stride_words,
            "start_word {} out of bounds for stride {}",
            start_word,
            self.stride_words
        );

        if dst == src || start_word == self.stride_words {
            return;
        }

        let start_dst = dst * self.stride_words;
        let start_src = src * self.stride_words;
        let len = self.stride_words - start_word;

        if start_dst < start_src {
            let (left, right) = self.data.split_at_mut(start_src);
            for (d, &s) in left[start_dst + start_word..start_dst + start_word + len]
                .iter_mut()
                .zip(&right[start_word..start_word + len])
            {
                *d ^= s;
            }
        } else {
            let (left, right) = self.data.split_at_mut(start_dst);
            for (d, &s) in right[start_word..start_word + len]
                .iter_mut()
                .zip(&left[start_src + start_word..start_src + start_word + len])
            {
                *d ^= s;
            }
        }
    }

    /// XORs a precomputed row slice into row `dst`, starting at `start_word`.
    ///
    /// Used by Gray-table elimination, where `src` is a row combination stored
    /// outside the matrix.
    pub(crate) fn row_xor_slice_from(&mut self, dst: usize, start_word: usize, src: &[u64]) {
        debug_assert!(dst < self.rows, "dst row {} out of bounds", dst);
        debug_assert!(
            start_word <= self.stride_words,
            "start_word {} out of bounds for stride {}",
            start_word,
            self.stride_words
        );
        debug_assert_eq!(
            src.len(),
            self.stride_words - start_word,
            "row_xor_slice_from source length must match suffix length"
        );

        if src.is_empty() {
            return;
        }

        let start_dst = dst * self.stride_words + start_word;
        for (d, &s) in self.data[start_dst..start_dst + src.len()]
            .iter_mut()
            .zip(src)
        {
            *d ^= s;
        }
    }

    /// Find the first row >= start_row that has a 1 in the given column.
    ///
    /// Returns `None` if no such row exists or an index is out of range.
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

    /// Returns the bit at `(row, col)` without the per-axis range checks of
    /// [`Self::get`].
    ///
    /// Out-of-range indices panic in debug builds; in release builds they read
    /// another position of the backing store or panic on its bounds.
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
    /// # Implementation
    ///
    /// The driver tiles the matrix into 64×64 bit-blocks and calls a block
    /// kernel of [`gf2_kernels_simd::transpose`] once per block. Under the
    /// `simd` cargo feature the kernel is the lane
    /// `gf2_kernels_simd::transpose::detect` publishes; without it the scalar
    /// lane is called directly.
    /// [`transpose_with_block_kernel`](Self::transpose_with_block_kernel) runs
    /// the same driver at a caller-chosen lane.
    ///
    /// # Complexity
    ///
    /// O(rows · cols / 64) word operations.
    pub fn transpose(&self) -> Self {
        self.transpose_with_block_kernel(Self::resolved_block_kernel())
    }

    /// The 64×64 block kernel [`Self::transpose`] resolves, which is the
    /// kernel of the lane [`transpose_block_lane`] reports.
    fn resolved_block_kernel() -> gf2_kernels_simd::transpose::Transpose64x64Fn {
        resolved_transpose_block().0
    }

    /// [`Self::transpose`] driven by one caller-chosen 64×64 block kernel.
    ///
    /// Tiling, output allocation, zero padding of a partial input tile and
    /// the output tail mask are those of [`Self::transpose`]; only the block
    /// primitive differs. `gf2_kernels_simd::transpose::lane` names a kernel.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::matrix::BitMatrix;
    /// use gf2_kernels_simd::transpose::{lane, TransposeLane};
    ///
    /// let mut m = BitMatrix::zeros(2, 3);
    /// m.set(0, 1, true);
    /// let scalar = lane(TransposeLane::Scalar).expect("always available");
    /// assert_eq!(m.transpose_with_block_kernel(scalar), m.transpose());
    /// ```
    pub fn transpose_with_block_kernel(
        &self,
        transpose_64x64: gf2_kernels_simd::transpose::Transpose64x64Fn,
    ) -> Self {
        if self.rows == 0 || self.cols == 0 {
            return Self::zeros(self.cols, self.rows);
        }
        self.transpose_blocked(transpose_64x64)
    }

    /// Tiled transpose driver: walks 64×64 bit-blocks and dispatches each
    /// to `transpose_64x64`, through the outer loop [`transpose_route`]
    /// selects.
    fn transpose_blocked(&self, transpose_64x64: fn(&[u64; 64], &mut [u64; 64])) -> Self {
        let mut out = Self::zeros(self.cols, self.rows);
        let in_stride = self.stride_words;
        let out_stride = out.stride_words;

        let n_row_blocks = self.rows.div_ceil(64);
        let n_col_blocks = self.cols.div_ceil(64);

        let route = transpose_route(n_row_blocks, n_col_blocks);
        match route {
            TransposeRoute::Simple => {
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
            }
            TransposeRoute::MacroTiled { macro_tile_blocks } => {
                let mut br_macro = 0usize;
                while br_macro < n_row_blocks {
                    let br_end = (br_macro + macro_tile_blocks).min(n_row_blocks);
                    let mut bc_macro = 0usize;
                    while bc_macro < n_col_blocks {
                        let bc_end = (bc_macro + macro_tile_blocks).min(n_col_blocks);
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
        }
        out.mask_padding_bits();
        #[cfg(any(test, feature = "test-support"))]
        record_transpose_effective_observation(route);
        out
    }

    /// Conservative default definition for
    /// `bit_matrix.transpose_simple_max_blocks` (in 64×64 bit-blocks): at or
    /// below the active value in both dimensions the transpose driver uses the
    /// simple two-level block loop, above it the macro-tiled outer loop.
    pub(crate) const TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS: usize = 16;

    /// Inner loop over a (br, bc) range of 64×64 bit-blocks.
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

                // Bit (j, i) of the transposed block is bit `i` of
                // `tile_out[j]`: row `col_start + j`, word `br` of the output.
                for (j, &word) in tile_out.iter().enumerate().take(block_cols) {
                    out_data[(col_start + j) * out_stride + br] = word;
                }
            }
        }
    }

    /// Converts this dense matrix to a CSR SpBitMatrix.
    ///
    /// This scans all bits and records set columns per row.
    pub fn to_sparse(&self) -> crate::sparse::SpBitMatrix {
        crate::sparse::SpBitMatrix::from_dense(self)
    }

    /// Masks padding bits in each row to zero.
    fn mask_padding_bits(&mut self) {
        if self.cols == 0 || self.stride_words == 0 {
            return;
        }

        let used_bits_in_last_word = self.cols % 64;
        if used_bits_in_last_word == 0 {
            return;
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
    /// For an m×n matrix A and an n-bit `x`, returns the m-bit vector y.
    ///
    /// # Panics
    ///
    /// Panics if x.len() != self.cols()
    pub fn matvec(&self, x: &crate::BitVec) -> crate::BitVec {
        assert_eq!(x.len(), self.cols, "input BitVec length must equal cols");

        match matvec_route(self.stride_words) {
            MatvecRoute::Simd => self.matvec_simd_lane(x),
            MatvecRoute::Scalar => self.matvec_scalar(x),
        }
    }

    /// [`Self::matvec`] on a caller-chosen lane.
    ///
    /// [`MatvecRoute::Simd`] runs the scalar lane in a build without the
    /// `simd` cargo feature and on a host whose kernel bundle is not
    /// detected, as [`Self::matvec`] does, and for a matrix without columns;
    /// both lanes return the same bits.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.cols()`.
    pub fn matvec_with_route(&self, x: &crate::BitVec, route: MatvecRoute) -> crate::BitVec {
        // Without `simd` both routes are the scalar lane `matvec` runs;
        // delegating keeps that lane's one inlined copy.
        #[cfg(not(feature = "simd"))]
        {
            let _ = route;
            self.matvec(x)
        }
        #[cfg(feature = "simd")]
        {
            assert_eq!(x.len(), self.cols, "input BitVec length must equal cols");

            match route {
                // The kernel lane walks rows by stride, which is zero here.
                MatvecRoute::Simd if self.stride_words > 0 => self.matvec_simd_lane(x),
                _ => self.matvec_scalar(x),
            }
        }
    }

    /// The SIMD lane of both entries: the fused kernel when the bundle is
    /// detected, the scalar lane otherwise.
    #[inline(always)]
    fn matvec_simd_lane(&self, x: &crate::BitVec) -> crate::BitVec {
        #[cfg(feature = "simd")]
        if let Some(fns) = crate::simd::maybe_simd() {
            return self.matvec_simd(x, fns);
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

    /// Row parities through the detected bundle's fused AND-population-count
    /// kernel, which counts each row's intersection with `x` without an
    /// intermediate buffer or a second pass.
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
    /// For an m×n matrix A and an m-bit `x`, returns the n-bit vector y.
    ///
    /// # Panics
    ///
    /// Panics if x.len() != self.rows()
    ///
    /// # Complexity
    ///
    /// O(rows × stride_words) word operations.
    pub fn matvec_transpose(&self, x: &crate::BitVec) -> crate::BitVec {
        assert_eq!(x.len(), self.rows, "input BitVec length must equal rows");

        let mut y = crate::BitVec::with_capacity(self.cols);

        for word_idx in 0..self.stride_words {
            let col_start = word_idx * 64;
            let col_end = (col_start + 64).min(self.cols);
            let mut block_result = 0u64;

            for r in 0..self.rows {
                if !x.get(r) {
                    continue;
                }

                let row_offset = r * self.stride_words;
                let word = self.data[row_offset + word_idx];
                block_result ^= word;
            }
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rows == 0 || self.cols == 0 {
            return write!(f, "[ ]");
        }

        // Border width: each column takes 2 chars (digit + space), plus 1 for final space
        let border_width = self.cols * 2 + 1;

        writeln!(f, "  ┌{}┐", " ".repeat(border_width))?;
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

    fn mul(self, rhs: BitMatrix) -> BitMatrix {
        crate::alg::matmul::multiply(&self, &rhs)
    }
}

impl Mul<&BitMatrix> for BitMatrix {
    type Output = BitMatrix;

    fn mul(self, rhs: &BitMatrix) -> BitMatrix {
        crate::alg::matmul::multiply(&self, rhs)
    }
}

impl Mul<BitMatrix> for &BitMatrix {
    type Output = BitMatrix;

    fn mul(self, rhs: BitMatrix) -> BitMatrix {
        crate::alg::matmul::multiply(self, &rhs)
    }
}

impl Mul<&BitMatrix> for &BitMatrix {
    type Output = BitMatrix;

    fn mul(self, rhs: &BitMatrix) -> BitMatrix {
        crate::alg::matmul::multiply(self, rhs)
    }
}

#[cfg(feature = "visualization")]
impl BitMatrix {
    /// Saves the matrix as a PNG image, one pixel per bit: unset bits black,
    /// set bits white.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or PNG encoding fails.
    pub fn save_image(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use image::{ImageBuffer, Rgb};

        const ZERO_COLOR: [u8; 3] = [0, 0, 0];
        const ONE_COLOR: [u8; 3] = [255, 255, 255];

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
        for r in 0..3 {
            for c in 0..4 {
                assert!(m.get(r, c), "Bit at ({}, {}) should be 1", r, c);
            }
        }
    }

    #[test]
    fn test_ones_edge_cases() {
        let m = BitMatrix::ones(1, 1);
        assert!(m.get(0, 0));

        // Non-word-aligned columns
        let m = BitMatrix::ones(2, 65);
        assert!(m.get(1, 64));
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
        let a = BitMatrix::identity(3);
        let b = BitMatrix::identity(3);
        let c = a * b;

        assert_eq!(c, BitMatrix::identity(3));
    }

    #[test]
    fn test_mul_operator_mixed_refs() {
        let a = BitMatrix::identity(2);
        let b = BitMatrix::identity(2);
        let c1 = a.clone() * &b;
        assert_eq!(c1, BitMatrix::identity(2));
        let c2 = &a * b.clone();
        assert_eq!(c2, BitMatrix::identity(2));
        let c3 = &a * &b;
        assert_eq!(c3, BitMatrix::identity(2));
    }

    #[test]
    fn test_mul_operator_rectangular() {
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
        assert!(c.get(0, 0));
        assert!(c.get(0, 1));
        assert!(c.get(1, 0));
        assert!(c.get(1, 1));
    }

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
