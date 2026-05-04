//! Sparse matrix primitives for GF(2) with CSR/CSC representations.
//!
//! This module provides memory-efficient sparse matrix support for low-density
//! matrices (< 5% density) over GF(2).
//!
//! # Storage Formats
//!
//! - **CSR (Compressed Sparse Row)**: Row-major format optimized for row iteration
//!   and matrix-vector multiply. All nonzero values are implicitly 1 in GF(2).
//! - **Dual (CSR+CSC)**: Stores both row and column formats for efficient bidirectional
//!   access patterns (e.g., alternating row/column sweeps in iterative algorithms).
//!
//! # Examples
//!
//! ```
//! use gf2_core::sparse::SpBitMatrix;
//! use gf2_core::BitVec;
//!
//! // Build from COO (coordinate) format
//! let coo = vec![(0, 1), (0, 3), (1, 2)];
//! let s = SpBitMatrix::from_coo(2, 4, &coo);
//! assert_eq!(s.nnz(), 3);
//!
//! // Matrix-vector multiply: x = [0, 1, 0, 1]
//! let mut x = BitVec::new();
//! x.push_bit(false);
//! x.push_bit(true);
//! x.push_bit(false);
//! x.push_bit(true);
//!
//! let y = s.matvec(&x);
//! // Row 0: x[1] ^ x[3] = 1 ^ 1 = 0
//! // Row 1: x[2] = 0
//! assert_eq!(y.get(0), false);
//! assert_eq!(y.get(1), false);
//! ```

use crate::{matrix::BitMatrix, BitVec};
use gf2_kernels_simd::prefetch_read_l1;
use std::fmt;

const DEFAULT_BLOCK_ROWS: usize = 32;
const DEFAULT_PREFETCH_DISTANCE: usize = 0;

/// A row-major sparse matrix in Compressed Sparse Row (CSR) format over GF(2).
///
/// Optimized for low-density matrices (< 5% nonzeros).
/// All nonzero entries are implicitly 1; the values array is omitted for GF(2).
///
/// # Storage Layout
///
/// - `indptr`: Array of length `rows + 1`. Row r spans `indices[indptr[r]..indptr[r+1]]`.
/// - `indices`: Packed array of column indices for nonzero entries (sorted per row).
/// - Duplicate coordinates XOR (even count cancels) in COO construction.
///
/// # Examples
///
/// ```
/// use gf2_core::sparse::SpBitMatrix;
///
/// let s = SpBitMatrix::identity(3);
/// assert_eq!(s.rows(), 3);
/// assert_eq!(s.cols(), 3);
/// assert_eq!(s.nnz(), 3);
///
/// // Iterate over nonzero columns in row 1
/// let cols: Vec<_> = s.row_iter(1).collect();
/// assert_eq!(cols, vec![1]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpBitMatrix {
    rows: usize,
    cols: usize,
    indptr: Vec<usize>,
    indices: Vec<usize>,
}

/// Descriptive alias for [`SpBitMatrix`].
///
/// All inherent methods, including [`SpBitMatrix::reorder_rcm`], are available
/// through this alias as `SparseBitMatrix::reorder_rcm`.
pub type SparseBitMatrix = SpBitMatrix;

/// Row and column permutation produced by [`SpBitMatrix::reorder_rcm`].
///
/// The reordered matrix stores rows and columns in Reverse Cuthill-McKee
/// order. This type uses a destination-to-source convention: `old_*_by_new[i]`
/// is the original index now stored at reordered index `i`. For an original
/// input vector `x`, call [`apply_cols`](Self::apply_cols) before multiplying
/// by the reordered matrix, then call [`unapply_rows`](Self::unapply_rows) on
/// the result if the caller needs original row order.
///
/// # Examples
///
/// ```
/// use gf2_core::sparse::SpBitMatrix;
/// use gf2_core::BitVec;
///
/// let a = SpBitMatrix::from_coo(3, 4, &[(0, 3), (1, 0), (2, 1), (2, 3)]);
/// let (reordered, permutation) = a.reorder_rcm();
/// let x = BitVec::ones(4);
///
/// let y = a.matvec(&x);
/// let y_rcm = reordered.matvec(&permutation.apply_cols(&x));
/// assert_eq!(permutation.unapply_rows(&y_rcm), y);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowPermutation {
    old_rows_by_new: Vec<usize>,
    new_rows_by_old: Vec<usize>,
    old_cols_by_new: Vec<usize>,
    new_cols_by_old: Vec<usize>,
}

impl RowPermutation {
    fn from_old_orders(old_rows_by_new: Vec<usize>, old_cols_by_new: Vec<usize>) -> Self {
        let new_rows_by_old = invert_permutation(&old_rows_by_new);
        let new_cols_by_old = invert_permutation(&old_cols_by_new);
        Self {
            old_rows_by_new,
            new_rows_by_old,
            old_cols_by_new,
            new_cols_by_old,
        }
    }

    /// Number of matrix rows covered by this permutation.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn rows_len(&self) -> usize {
        self.old_rows_by_new.len()
    }

    /// Number of matrix columns covered by this permutation.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn cols_len(&self) -> usize {
        self.old_cols_by_new.len()
    }

    /// Returns the original row index stored at `new_row` in the reordered matrix.
    ///
    /// This is the destination-to-source row mapping used by
    /// [`apply_rows`](Self::apply_rows).
    ///
    /// # Panics
    ///
    /// Panics if `new_row >= self.rows_len()`.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn old_row_for_new(&self, new_row: usize) -> usize {
        self.old_rows_by_new[new_row]
    }

    /// Returns the reordered row index for an original row.
    ///
    /// This is the inverse of [`old_row_for_new`](Self::old_row_for_new).
    ///
    /// # Panics
    ///
    /// Panics if `old_row >= self.rows_len()`.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn new_row_for_old(&self, old_row: usize) -> usize {
        self.new_rows_by_old[old_row]
    }

    /// Returns the original column index stored at `new_col` in the reordered matrix.
    ///
    /// This is the destination-to-source column mapping used by
    /// [`apply_cols`](Self::apply_cols).
    ///
    /// # Panics
    ///
    /// Panics if `new_col >= self.cols_len()`.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn old_col_for_new(&self, new_col: usize) -> usize {
        self.old_cols_by_new[new_col]
    }

    /// Returns the reordered column index for an original column.
    ///
    /// This is the inverse of [`old_col_for_new`](Self::old_col_for_new).
    ///
    /// # Panics
    ///
    /// Panics if `old_col >= self.cols_len()`.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn new_col_for_old(&self, old_col: usize) -> usize {
        self.new_cols_by_old[old_col]
    }

    /// Applies the row permutation to a vector in original row order.
    ///
    /// The returned vector is in reordered row order: output bit `new_row`
    /// equals input bit `old_row_for_new(new_row)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(3, 3, &[(0, 2), (1, 1), (2, 0)]);
    /// let (_reordered, perm) = a.reorder_rcm();
    /// let rows = BitVec::ones(3);
    /// assert_eq!(perm.unapply_rows(&perm.apply_rows(&rows)), rows);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.rows_len()`.
    ///
    /// # Complexity
    ///
    /// O(rows) time and O(rows) output storage.
    pub fn apply_rows(&self, bits: &BitVec) -> BitVec {
        apply_bitvec_permutation(bits, &self.old_rows_by_new, "row")
    }

    /// Restores a row vector from reordered row order to original row order.
    ///
    /// The returned vector is in original row order; use this on a reordered
    /// matrix-vector product when callers require the same order as
    /// [`SpBitMatrix::matvec`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(2, 3, &[(0, 0), (1, 2)]);
    /// let (reordered, perm) = a.reorder_rcm();
    /// let x = BitVec::ones(3);
    /// let y_rcm = reordered.matvec(&perm.apply_cols(&x));
    /// assert_eq!(perm.unapply_rows(&y_rcm), a.matvec(&x));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.rows_len()`.
    ///
    /// # Complexity
    ///
    /// O(rows) time and O(rows) output storage.
    pub fn unapply_rows(&self, bits: &BitVec) -> BitVec {
        unapply_bitvec_permutation(bits, &self.old_rows_by_new, "row")
    }

    /// Applies the column permutation to a vector in original column order.
    ///
    /// The returned vector is in reordered column order: output bit `new_col`
    /// equals input bit `old_col_for_new(new_col)`.
    ///
    /// Use this on the input vector before calling `matvec` on the matrix
    /// returned by [`SpBitMatrix::reorder_rcm`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(2, 4, &[(0, 3), (1, 0)]);
    /// let (_reordered, perm) = a.reorder_rcm();
    /// let cols = BitVec::ones(4);
    /// assert_eq!(perm.unapply_cols(&perm.apply_cols(&cols)), cols);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.cols_len()`.
    ///
    /// # Complexity
    ///
    /// O(cols) time and O(cols) output storage.
    pub fn apply_cols(&self, bits: &BitVec) -> BitVec {
        apply_bitvec_permutation(bits, &self.old_cols_by_new, "column")
    }

    /// Restores a column vector from reordered column order to original column order.
    ///
    /// This is the inverse of [`apply_cols`](Self::apply_cols).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(2, 4, &[(0, 3), (1, 0)]);
    /// let (_reordered, perm) = a.reorder_rcm();
    /// let cols = BitVec::ones(4);
    /// assert_eq!(perm.apply_cols(&perm.unapply_cols(&cols)), cols);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.cols_len()`.
    ///
    /// # Complexity
    ///
    /// O(cols) time and O(cols) output storage.
    pub fn unapply_cols(&self, bits: &BitVec) -> BitVec {
        unapply_bitvec_permutation(bits, &self.old_cols_by_new, "column")
    }
}

fn apply_bitvec_permutation(bits: &BitVec, old_by_new: &[usize], axis: &str) -> BitVec {
    assert_eq!(
        bits.len(),
        old_by_new.len(),
        "input BitVec length must equal {axis} permutation length"
    );
    let mut out = BitVec::with_capacity(old_by_new.len());
    for &old in old_by_new {
        out.push_bit(bits.get(old));
    }
    out
}

fn unapply_bitvec_permutation(bits: &BitVec, old_by_new: &[usize], axis: &str) -> BitVec {
    assert_eq!(
        bits.len(),
        old_by_new.len(),
        "input BitVec length must equal {axis} permutation length"
    );
    let mut out = BitVec::zeros(old_by_new.len());
    for (new, &old) in old_by_new.iter().enumerate() {
        if bits.get(new) {
            out.set(old, true);
        }
    }
    out
}

fn invert_permutation(old_by_new: &[usize]) -> Vec<usize> {
    let mut new_by_old = vec![usize::MAX; old_by_new.len()];
    for (new, &old) in old_by_new.iter().enumerate() {
        assert!(old < old_by_new.len(), "permutation index out of bounds");
        assert_eq!(
            new_by_old[old],
            usize::MAX,
            "duplicate index in permutation"
        );
        new_by_old[old] = new;
    }
    debug_assert!(new_by_old.iter().all(|&new| new != usize::MAX));
    new_by_old
}

fn rcm_bipartite_orders(csr: &SpBitMatrix) -> (Vec<usize>, Vec<usize>) {
    let nodes = csr.rows + csr.cols;
    if nodes == 0 {
        return (Vec::new(), Vec::new());
    }

    let mut row_degrees = Vec::with_capacity(csr.rows);
    for row in 0..csr.rows {
        row_degrees.push(csr.indptr[row + 1] - csr.indptr[row]);
    }

    let mut col_degrees = vec![0usize; csr.cols];
    for &col in &csr.indices {
        col_degrees[col] += 1;
    }

    let mut col_rows = vec![Vec::<usize>::new(); csr.cols];
    for row in 0..csr.rows {
        for &col in &csr.indices[csr.indptr[row]..csr.indptr[row + 1]] {
            col_rows[col].push(row);
        }
    }

    let degree = |node: usize| -> usize {
        if node < csr.rows {
            row_degrees[node]
        } else {
            col_degrees[node - csr.rows]
        }
    };

    let mut node_order: Vec<usize> = (0..nodes).collect();
    node_order.sort_by_key(|&node| (degree(node), node));

    let mut visited = vec![false; nodes];
    let mut cm_order = Vec::with_capacity(nodes);
    let mut queue = Vec::new();
    let mut neighbors = Vec::new();

    for &start in &node_order {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        queue.clear();
        queue.push(start);
        let mut head = 0;

        while head < queue.len() {
            let node = queue[head];
            head += 1;
            cm_order.push(node);

            neighbors.clear();
            if node < csr.rows {
                for &col in &csr.indices[csr.indptr[node]..csr.indptr[node + 1]] {
                    let next = csr.rows + col;
                    if !visited[next] {
                        neighbors.push(next);
                    }
                }
            } else {
                let col = node - csr.rows;
                for &row in &col_rows[col] {
                    if !visited[row] {
                        neighbors.push(row);
                    }
                }
            }

            neighbors.sort_by_key(|&next| (degree(next), next));
            for &next in &neighbors {
                visited[next] = true;
                queue.push(next);
            }
        }
    }

    cm_order.reverse();
    let mut old_rows_by_new = Vec::with_capacity(csr.rows);
    let mut old_cols_by_new = Vec::with_capacity(csr.cols);
    for node in cm_order {
        if node < csr.rows {
            old_rows_by_new.push(node);
        } else {
            old_cols_by_new.push(node - csr.rows);
        }
    }

    debug_assert_eq!(old_rows_by_new.len(), csr.rows);
    debug_assert_eq!(old_cols_by_new.len(), csr.cols);
    (old_rows_by_new, old_cols_by_new)
}

/// A row-blocked CSR representation for repeated sparse GF(2) matvecs.
///
/// `SpBitMatrix` keeps the classic scalar CSR path unchanged. Convert explicitly
/// with [`block_csr_from_csr`] or [`SpBitMatrix::to_block_csr`] when an LDPC-style
/// workload repeatedly multiplies the same sparse matrix by dense bit vectors.
///
/// # Storage layout
///
/// Rows are partitioned into fixed-size blocks. Each block stores row offsets
/// relative to the block's first nonzero and keeps the column stream contiguous
/// within each block. Matvec therefore keeps the public GF(2) API clean while
/// the hot loop avoids per-edge bounds checks and can issue best-effort L1
/// software prefetches for future input-vector words.
///
/// # Complexity
///
/// Construction is O(rows + nnz). Matvec is O(rows + nnz) and preserves the same
/// little-endian bit numbering and tail masking invariants as [`BitVec`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpBitMatrixBlockCsr {
    rows: usize,
    cols: usize,
    block_rows: usize,
    block_ptr: Vec<usize>,
    block_nnz_ptr: Vec<usize>,
    row_offsets: Vec<usize>,
    indices: Vec<usize>,
}

/// Converts a classic CSR sparse matrix into the opt-in block-CSR layout.
///
/// The existing [`SpBitMatrix::matvec`] path is intentionally not changed by
/// this transformer; callers choose the blocked representation explicitly.
///
/// # Panics
///
/// Panics if `block_rows == 0`.
///
/// # Examples
///
/// ```
/// use gf2_core::sparse::{block_csr_from_csr, SpBitMatrix};
/// use gf2_core::BitVec;
///
/// let a = SpBitMatrix::from_coo(2, 65, &[(0, 0), (0, 64), (1, 63)]);
/// let blocked = block_csr_from_csr(&a, 2);
/// let x = BitVec::ones(65);
/// assert_eq!(blocked.matvec(&x), a.matvec(&x));
/// ```
///
/// # Complexity
///
/// O(rows + nnz) time and O(rows + nnz) additional memory.
pub fn block_csr_from_csr(csr: &SpBitMatrix, block_rows: usize) -> SpBitMatrixBlockCsr {
    assert!(block_rows > 0, "block_rows must be non-zero");

    let num_blocks = csr.rows.div_ceil(block_rows);
    let mut block_ptr = Vec::with_capacity(num_blocks + 1);
    let mut block_nnz_ptr = Vec::with_capacity(num_blocks + 1);
    let mut row_offsets = Vec::with_capacity(csr.rows + num_blocks);
    let mut indices = Vec::with_capacity(csr.nnz());

    for block in 0..num_blocks {
        let row_start = block * block_rows;
        let row_end = (row_start + block_rows).min(csr.rows);
        block_ptr.push(row_offsets.len());
        block_nnz_ptr.push(indices.len());
        row_offsets.push(0);

        for row in row_start..row_end {
            let start = csr.indptr[row];
            let end = csr.indptr[row + 1];
            indices.extend_from_slice(&csr.indices[start..end]);
            row_offsets.push(indices.len() - block_nnz_ptr[block]);
        }
    }

    block_ptr.push(row_offsets.len());
    block_nnz_ptr.push(indices.len());

    SpBitMatrixBlockCsr {
        rows: csr.rows,
        cols: csr.cols,
        block_rows,
        block_ptr,
        block_nnz_ptr,
        row_offsets,
        indices,
    }
}

/// Deterministic LDPC-like sparse fixture shared by sparse benches and examples.
///
/// This is hidden from generated API docs because it exists only to keep
/// performance evidence harnesses on one deterministic input pattern.
#[doc(hidden)]
pub fn deterministic_ldpc_like_fixture(rows: usize, cols: usize, row_weight: usize) -> SpBitMatrix {
    let mut entries = Vec::with_capacity(rows * row_weight);
    for r in 0..rows {
        let base = r.wrapping_mul(1_315_423_911usize) ^ rows.rotate_left(7);
        for k in 0..row_weight {
            let stride = 2 * k + 1;
            let col = base
                .wrapping_add(k.wrapping_mul(97_531))
                .wrapping_add(r.wrapping_mul(stride))
                % cols;
            entries.push((r, col));
        }
    }
    SpBitMatrix::from_coo_deduplicated(rows, cols, &entries)
}

/// Deterministic input bit-vector fixture shared by sparse benches and examples.
///
/// This is hidden from generated API docs because it exists only to keep
/// performance evidence harnesses on one deterministic input pattern.
#[doc(hidden)]
pub fn deterministic_sparse_bitvec_fixture(len: usize) -> BitVec {
    let mut x = BitVec::with_capacity(len);
    for i in 0..len {
        x.push_bit(((i.wrapping_mul(0x9E37_79B1) ^ (i >> 3)) & 7) < 3);
    }
    x
}

impl SpBitMatrixBlockCsr {
    /// Returns number of rows.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Returns number of cols.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Returns number of nonzeros.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Returns the row-block height used by this layout.
    #[inline]
    pub fn block_rows(&self) -> usize {
        self.block_rows
    }

    /// Matrix-vector product y = A · x over GF(2) using the default block-CSR
    /// schedule.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.cols()`.
    ///
    /// # Complexity
    ///
    /// O(rows + nnz). The default schedule uses block-local row metadata and
    /// direct word-level gathers without software prefetch. Call
    /// [`matvec_with_prefetch_distance`](Self::matvec_with_prefetch_distance)
    /// with a nonzero distance to additionally issue best-effort L1 prefetch
    /// hints on targets supported by `gf2-kernels-simd`.
    #[inline]
    pub fn matvec(&self, x: &BitVec) -> BitVec {
        self.matvec_with_prefetch_distance(x, DEFAULT_PREFETCH_DISTANCE)
    }

    /// Matrix-vector product y = A · x with an explicit prefetch lookahead.
    ///
    /// `prefetch_distance` is counted in nonzero entries within a row. A distance
    /// of zero disables software prefetch while retaining the block-CSR layout.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.cols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(2, 66, &[(0, 1), (0, 65), (1, 64)]);
    /// let blocked = a.to_block_csr(2);
    /// let x = BitVec::ones(66);
    ///
    /// assert_eq!(blocked.matvec_with_prefetch_distance(&x, 0), a.matvec(&x));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows + nnz) time and O(rows) output storage. A nonzero
    /// `prefetch_distance` additionally issues best-effort L1 prefetch hints
    /// while preserving identical GF(2) results.
    pub fn matvec_with_prefetch_distance(&self, x: &BitVec, prefetch_distance: usize) -> BitVec {
        assert_eq!(x.len(), self.cols, "input BitVec length must equal cols");

        let x_words = x.words();
        let mut y = BitVec::with_capacity(self.rows);
        let num_blocks = self.block_nnz_ptr.len() - 1;

        for block in 0..num_blocks {
            let row_base = block * self.block_rows;
            let rows_in_block = (self.rows - row_base).min(self.block_rows);
            let offset_start = self.block_ptr[block];
            let nnz_base = self.block_nnz_ptr[block];
            let offsets = &self.row_offsets[offset_start..offset_start + rows_in_block + 1];

            for local_row in 0..rows_in_block {
                let start = nnz_base + offsets[local_row];
                let end = nnz_base + offsets[local_row + 1];
                let mut acc = 0u64;

                for edge in start..end {
                    let future = edge + prefetch_distance;
                    if prefetch_distance != 0 && future < end {
                        let future_word = self.indices[future] >> 6;
                        let ptr = x_words.as_ptr().wrapping_add(future_word).cast::<u8>();
                        prefetch_read_l1(ptr);
                    }

                    let col = self.indices[edge];
                    acc ^= ((x_words[col >> 6] & (1u64 << (col & 63))) != 0) as u64;
                }

                y.push_bit((acc & 1) != 0);
            }
        }

        y
    }
}

impl SpBitMatrix {
    /// Creates an all-zero sparse matrix with given shape.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// let s = SpBitMatrix::zeros(10, 20);
    /// assert_eq!(s.rows(), 10);
    /// assert_eq!(s.cols(), 20);
    /// assert_eq!(s.nnz(), 0);
    /// ```
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            indptr: vec![0; rows + 1],
            indices: Vec::new(),
        }
    }

    /// Returns an iterator over set column indices in the given row.
    ///
    /// # Panics
    ///
    /// Panics if `row >= rows`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// let coo = vec![(0, 2), (0, 5), (1, 3)];
    /// let s = SpBitMatrix::from_coo(2, 6, &coo);
    ///
    /// let r0: Vec<_> = s.row_iter(0).collect();
    /// assert_eq!(r0, vec![2, 5]);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(nnz_in_row) where nnz_in_row is the number of nonzeros in the row.
    pub fn row_iter(&self, row: usize) -> impl ExactSizeIterator<Item = usize> + '_ {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        let start = self.indptr[row];
        let end = self.indptr[row + 1];
        self.indices[start..end].iter().copied()
    }

    /// Creates an n×n identity matrix.
    pub fn identity(n: usize) -> Self {
        let mut indptr = Vec::with_capacity(n + 1);
        indptr.push(0);
        for r in 1..=n {
            indptr.push(r);
        }
        let indices = (0..n).collect();
        Self {
            rows: n,
            cols: n,
            indptr,
            indices,
        }
    }

    /// Builds a CSR matrix from COO coordinates. Duplicates toggle (XOR) semantics.
    ///
    /// In GF(2), duplicate entries at the same (row, col) position cancel each other:
    /// - Even number of duplicates → bit is 0 (cleared)
    /// - Odd number of duplicates → bit is 1 (set)
    ///
    /// For LDPC matrices where duplicates are construction artifacts, use
    /// [`from_coo_deduplicated`](Self::from_coo_deduplicated) instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// // Two duplicates cancel via XOR
    /// let edges = vec![(0, 1), (0, 1), (1, 2)];
    /// let m = SpBitMatrix::from_coo(2, 3, &edges);
    /// let d = m.to_dense();
    /// assert_eq!(d.get(0, 1), false); // Canceled
    /// assert_eq!(m.nnz(), 1);
    /// ```
    pub fn from_coo(rows: usize, cols: usize, entries: &[(usize, usize)]) -> Self {
        // Collect columns per row
        let mut per_row: Vec<Vec<usize>> = vec![Vec::new(); rows];
        for &(r, c) in entries {
            assert!(r < rows, "row index {} out of bounds (rows={})", r, rows);
            assert!(c < cols, "col index {} out of bounds (cols={})", c, cols);
            per_row[r].push(c);
        }

        // For each row: sort, XOR-dedup, and append
        let mut indptr = Vec::with_capacity(rows + 1);
        let mut indices = Vec::new();
        indptr.push(0);
        for row in per_row.iter_mut() {
            if !row.is_empty() {
                row.sort_unstable();
                let mut i = 0;
                while i < row.len() {
                    let c = row[i];
                    let mut count = 1;
                    while i + count < row.len() && row[i + count] == c {
                        count += 1;
                    }
                    if count % 2 == 1 {
                        indices.push(c);
                    }
                    i += count;
                }
            }
            indptr.push(indices.len());
        }

        Self {
            rows,
            cols,
            indptr,
            indices,
        }
    }

    /// Builds a CSR matrix from COO coordinates with deduplication.
    ///
    /// Duplicate entries at the same (row, col) position are ignored (first occurrence wins).
    /// This is appropriate for LDPC parity-check matrices where duplicates are typically
    /// construction artifacts from combining information bit connections with parity structure.
    ///
    /// For GF(2) XOR semantics where duplicates cancel, use [`from_coo`](Self::from_coo).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// // Duplicates are ignored (dedup, not XOR)
    /// let edges = vec![(0, 0), (0, 1), (0, 1), (1, 2)];
    /// let m = SpBitMatrix::from_coo_deduplicated(2, 3, &edges);
    /// let d = m.to_dense();
    /// assert_eq!(d.get(0, 0), true);
    /// assert_eq!(d.get(0, 1), true); // NOT false
    /// assert_eq!(d.get(1, 2), true);
    /// assert_eq!(m.nnz(), 3);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(nnz log(nnz/rows)) where nnz is the total number of input entries.
    pub fn from_coo_deduplicated(rows: usize, cols: usize, entries: &[(usize, usize)]) -> Self {
        let mut per_row: Vec<Vec<usize>> = vec![Vec::new(); rows];
        for &(r, c) in entries {
            assert!(r < rows, "row index {} out of bounds (rows={})", r, rows);
            assert!(c < cols, "col index {} out of bounds (cols={})", c, cols);
            per_row[r].push(c);
        }

        let mut indptr = Vec::with_capacity(rows + 1);
        let mut indices = Vec::new();
        indptr.push(0);

        for row in per_row.iter_mut() {
            if !row.is_empty() {
                row.sort_unstable();
                row.dedup();
                indices.extend_from_slice(row);
            }
            indptr.push(indices.len());
        }

        Self {
            rows,
            cols,
            indptr,
            indices,
        }
    }

    /// Constructs a CSR matrix by scanning a dense BitMatrix.
    pub fn from_dense(m: &BitMatrix) -> Self {
        let rows = m.rows();
        let cols = m.cols();
        let mut indptr = Vec::with_capacity(rows + 1);
        let mut indices = Vec::new();
        indptr.push(0);
        for r in 0..rows {
            for c in 0..cols {
                if m.get(r, c) {
                    indices.push(c);
                }
            }
            indptr.push(indices.len());
        }
        Self {
            rows,
            cols,
            indptr,
            indices,
        }
    }

    /// Converts this sparse matrix to a dense bit-packed BitMatrix.
    pub fn to_dense(&self) -> BitMatrix {
        let mut m = BitMatrix::zeros(self.rows, self.cols);
        for r in 0..self.rows {
            for c in self.row_iter(r) {
                m.set(r, c, true);
            }
        }
        m
    }

    /// Returns the transpose of this CSR matrix as CSR of the transposed shape.
    /// This is O(nnz + rows + cols) and stable by column order.
    pub fn transpose(&self) -> Self {
        let rows_t = self.cols;
        let cols_t = self.rows;
        let nnz = self.indices.len();
        // Count nnz per column (which become rows in transpose)
        let mut counts = vec![0usize; rows_t];
        for r in 0..self.rows {
            for c in self.row_iter(r) {
                counts[c] += 1;
            }
        }
        // Exclusive prefix-sum to build indptr_t
        let mut indptr = Vec::with_capacity(rows_t + 1);
        indptr.push(0);
        for i in 0..rows_t {
            indptr.push(indptr[i] + counts[i]);
        }
        let mut indices = vec![0usize; nnz];
        // Working offsets initialized to row starts
        let mut next = indptr.clone();
        // Scatter
        for r in 0..self.rows {
            for c in self.row_iter(r) {
                let pos = next[c];
                indices[pos] = r;
                next[c] += 1;
            }
        }
        Self {
            rows: rows_t,
            cols: cols_t,
            indptr,
            indices,
        }
    }

    /// Returns an iterator over row indices that have a 1 in the given column.
    /// Simpler baseline using a transient transpose.
    pub fn col_iter(&self, col: usize) -> impl IntoIterator<Item = usize> {
        assert!(
            col < self.cols,
            "col index {} out of bounds (cols={})",
            col,
            self.cols
        );
        let st = self.transpose();
        let v: Vec<_> = st.row_iter(col).collect();
        v
    }

    /// Returns number of rows.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Returns number of cols.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Returns number of nonzeros (after XOR-dedup).
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Converts this CSR matrix to the opt-in block-CSR matvec layout.
    ///
    /// Existing callers of [`matvec`](Self::matvec) continue to use the classic
    /// scalar CSR path. This method is for workloads that repeatedly multiply
    /// the same sparse matrix and can amortize the O(rows + nnz) transformation.
    ///
    /// # Panics
    ///
    /// Panics if `block_rows == 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(2, 66, &[(0, 1), (0, 65), (1, 64)]);
    /// let blocked = a.to_block_csr(2);
    /// let x = BitVec::ones(66);
    /// assert_eq!(blocked.matvec(&x), a.matvec(&x));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows + nnz) time and memory.
    #[inline]
    pub fn to_block_csr(&self, block_rows: usize) -> SpBitMatrixBlockCsr {
        block_csr_from_csr(self, block_rows)
    }

    /// Converts this CSR matrix to the default block-CSR matvec layout.
    ///
    /// Uses a 32-row block, which keeps per-block row metadata compact while
    /// preserving row order for bit-exact parity with CSR.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(2, 66, &[(0, 1), (0, 65), (1, 64)]);
    /// let blocked = a.to_default_block_csr();
    /// let x = BitVec::ones(66);
    /// assert_eq!(blocked.matvec(&x), a.matvec(&x));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows + nnz) time and memory.
    #[inline]
    pub fn to_default_block_csr(&self) -> SpBitMatrixBlockCsr {
        self.to_block_csr(DEFAULT_BLOCK_ROWS)
    }

    /// Returns a Reverse Cuthill-McKee row/column reordered copy of this matrix.
    ///
    /// The ordering is computed on the bipartite graph with one node per row,
    /// one node per column, and edges for nonzero matrix entries. The matrix
    /// returned by this method stores both rows and columns in reverse
    /// Cuthill-McKee (RCM) order, which tends to reduce sparse-matrix bandwidth
    /// and improve cache reuse for repeated LDPC-style matvecs. The default CSR
    /// layout and [`matvec`](Self::matvec) behavior are unchanged; this is an
    /// explicit one-shot preprocessing step for workloads that can amortize the
    /// O(rows + cols + nnz) reorder cost over many multiplies.
    ///
    /// For an original input `x`, compute with the reordered matrix as:
    /// `perm.unapply_rows(&reordered.matvec(&perm.apply_cols(&x)))`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    /// use gf2_core::BitVec;
    ///
    /// let a = SpBitMatrix::from_coo(3, 4, &[(0, 0), (0, 3), (1, 1), (2, 3)]);
    /// let (reordered, perm) = a.reorder_rcm();
    /// let x = BitVec::ones(4);
    /// let y = a.matvec(&x);
    /// let y_rcm = reordered.matvec(&perm.apply_cols(&x));
    /// assert_eq!(perm.unapply_rows(&y_rcm), y);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows + cols + nnz + sum(d(v) log d(v))) time for RCM neighbor ordering,
    /// where d(v) is graph degree, and O(rows + cols + nnz) additional memory.
    pub fn reorder_rcm(&self) -> (Self, RowPermutation) {
        let (old_rows_by_new, old_cols_by_new) = rcm_bipartite_orders(self);
        let permutation = RowPermutation::from_old_orders(old_rows_by_new, old_cols_by_new);

        let mut indptr = Vec::with_capacity(self.rows + 1);
        let mut indices = Vec::with_capacity(self.nnz());
        indptr.push(0);

        for &old_row in &permutation.old_rows_by_new {
            let start = self.indptr[old_row];
            let end = self.indptr[old_row + 1];
            let row_start = indices.len();
            for &old_col in &self.indices[start..end] {
                indices.push(permutation.new_cols_by_old[old_col]);
            }
            indices[row_start..].sort_unstable();
            indptr.push(indices.len());
        }

        (
            Self {
                rows: self.rows,
                cols: self.cols,
                indptr,
                indices,
            },
            permutation,
        )
    }

    /// Matrix-vector product y = A · x over GF(2).
    /// x length must equal cols, y length equals rows.
    pub fn matvec(&self, x: &BitVec) -> BitVec {
        assert_eq!(x.len(), self.cols, "input BitVec length must equal cols");
        let mut y = BitVec::with_capacity(self.rows);
        for r in 0..self.rows {
            let mut acc = false;
            let start = self.indptr[r];
            let end = self.indptr[r + 1];
            for &c in &self.indices[start..end] {
                acc ^= x.get(c);
            }
            y.push_bit(acc);
        }
        y
    }

    /// Sparse × sparse matrix multiplication `C = A · B` over GF(2).
    ///
    /// Both operands and the result are CSR matrices. Inner dimensions must
    /// agree (`self.cols() == other.rows()`), and the output has shape
    /// `self.rows() × other.cols()`. GF(2) semantics apply: contributions from
    /// distinct `k` indices accumulate by XOR, so an even number of touches at
    /// the same output coordinate cancels.
    ///
    /// # Arguments
    ///
    /// * `other` — right-hand-side sparse matrix `B`. Must satisfy
    ///   `self.cols() == other.rows()`; otherwise the call panics (see
    ///   *Panics* below).
    ///
    /// # Algorithm
    ///
    /// Row-by-row CSR multiply with a dense word-packed XOR accumulator:
    /// for each row `i` of `A`, for each nonzero column `k`, toggle every
    /// nonzero column `j` of `B`'s row `k` in the accumulator. After all
    /// contributions for output row `i` have been XOR-folded, the accumulator
    /// is scanned via `trailing_zeros` to extract the canonical sorted column
    /// indices, then the touched accumulator words are cleared in place for
    /// reuse on the next row.
    ///
    /// The output is canonical CSR: column indices within each row are sorted
    /// ascending and free of duplicates. This guarantees criterion #2 of the
    /// API contract — the result is bit-equal to
    /// `SpBitMatrix::from_dense(&(self.to_dense() * other.to_dense()))`.
    ///
    /// # Panics
    ///
    /// Panics if `self.cols() != other.rows()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// // A = [[1 0 1] [0 1 0]] over GF(2)
    /// let a = SpBitMatrix::from_coo(2, 3, &[(0, 0), (0, 2), (1, 1)]);
    /// // B = I_3
    /// let b = SpBitMatrix::identity(3);
    /// let c = a.matmul(&b);
    /// assert_eq!(c, a);
    ///
    /// // Multiplying by an identity on the left also reproduces A.
    /// let lhs = SpBitMatrix::identity(2);
    /// assert_eq!(lhs.matmul(&a), a);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(nnz(A · B-rows) + rows(A) · cols(B) / 64) where the first term is the
    /// total flop work counted as `Σ_i Σ_{k ∈ A_row_i} nnz(B_row_k)` and the
    /// second term covers the per-row accumulator scan and clear. Memory is
    /// O(cols(B) / 64) for the accumulator plus O(nnz(C)) for the output.
    pub fn matmul(&self, other: &Self) -> Self {
        assert_eq!(
            self.cols, other.rows,
            "matmul inner dimensions must match: lhs.cols={} rhs.rows={}",
            self.cols, other.rows,
        );

        let out_rows = self.rows;
        let out_cols = other.cols;
        let n_words = out_cols.div_ceil(64);

        let mut indptr = Vec::with_capacity(out_rows + 1);
        indptr.push(0);
        let mut indices: Vec<usize> = Vec::new();

        // Dense word-packed XOR accumulator, reused across output rows.
        let mut acc = vec![0u64; n_words];
        // Track which words were touched so we can clear lazily without
        // sweeping the whole accumulator every row.
        let mut touched: Vec<usize> = Vec::new();
        let mut touched_seen = vec![false; n_words];

        for i in 0..out_rows {
            let a_start = self.indptr[i];
            let a_end = self.indptr[i + 1];
            for &k in &self.indices[a_start..a_end] {
                let b_start = other.indptr[k];
                let b_end = other.indptr[k + 1];
                for &j in &other.indices[b_start..b_end] {
                    let w = j >> 6;
                    let bit = 1u64 << (j & 63);
                    acc[w] ^= bit;
                    if !touched_seen[w] {
                        touched_seen[w] = true;
                        touched.push(w);
                    }
                }
            }

            // Emit canonical, ascending column indices for output row i.
            // Sort touched words so each row's emitted slice is monotone.
            touched.sort_unstable();
            for &w in &touched {
                let mut word = acc[w];
                let base = w << 6;
                while word != 0 {
                    let b = word.trailing_zeros() as usize;
                    indices.push(base + b);
                    word &= word - 1;
                }
                acc[w] = 0;
                touched_seen[w] = false;
            }
            touched.clear();

            indptr.push(indices.len());
        }

        Self {
            rows: out_rows,
            cols: out_cols,
            indptr,
            indices,
        }
    }
}

/// Dual representation storing both CSR and CSC formats for efficient bidirectional access.
///
/// This representation stores the same sparse matrix in both row-major (CSR) and
/// column-major (CSC) formats, enabling O(nnz_in_row/col) access for both row and
/// column iteration patterns without transposition overhead.
///
/// # Use Cases
///
/// - Algorithms requiring alternating row and column sweeps
/// - Iterative methods with bidirectional access patterns
/// - Applications where both A×x and A^T×x are frequently computed
///
/// # Memory Trade-off
///
/// Uses 2× memory of single CSR representation, but still typically < dense BitMatrix
/// at densities below 3-5%.
///
/// # Examples
///
/// ```
/// use gf2_core::sparse::SpBitMatrixDual;
/// use gf2_core::matrix::BitMatrix;
///
/// let mut m = BitMatrix::zeros(3, 4);
/// m.set(0, 1, true);
/// m.set(1, 2, true);
/// m.set(2, 0, true);
///
/// let dual = SpBitMatrixDual::from_dense(&m);
///
/// // Fast row iteration (no transpose)
/// let row_cols: Vec<_> = dual.row_iter(0).collect();
/// assert_eq!(row_cols, vec![1]);
///
/// // Fast column iteration (no transpose)
/// let col_rows: Vec<_> = dual.col_iter(1).collect();
/// assert_eq!(col_rows, vec![0]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpBitMatrixDual {
    csr: SpBitMatrix,
    csc: SpBitMatrix,
}

impl SpBitMatrixDual {
    /// Creates a dual representation from a dense BitMatrix.
    ///
    /// Constructs both CSR and CSC formats in one pass.
    pub fn from_dense(m: &BitMatrix) -> Self {
        let csr = SpBitMatrix::from_dense(m);
        let csc = csr.transpose();
        Self { csr, csc }
    }

    /// Creates a dual representation from COO coordinates with XOR semantics.
    ///
    /// Duplicates cancel (even count → 0, odd count → 1).
    /// For deduplication semantics, use [`from_coo_deduplicated`](Self::from_coo_deduplicated).
    pub fn from_coo(rows: usize, cols: usize, entries: &[(usize, usize)]) -> Self {
        let csr = SpBitMatrix::from_coo(rows, cols, entries);
        let csc = csr.transpose();
        Self { csr, csc }
    }

    /// Creates a dual representation from COO coordinates with deduplication.
    ///
    /// Duplicate entries are ignored (first occurrence wins). This is appropriate for
    /// LDPC matrices where duplicates are construction artifacts.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrixDual;
    ///
    /// let edges = vec![(0, 1), (0, 1), (1, 2)];
    /// let dual = SpBitMatrixDual::from_coo_deduplicated(2, 3, &edges);
    /// let d = dual.to_dense();
    /// assert_eq!(d.get(0, 1), true); // NOT false (dedup, not XOR)
    /// assert_eq!(dual.nnz(), 2);
    /// ```
    pub fn from_coo_deduplicated(rows: usize, cols: usize, entries: &[(usize, usize)]) -> Self {
        let csr = SpBitMatrix::from_coo_deduplicated(rows, cols, entries);
        let csc = csr.transpose();
        Self { csr, csc }
    }

    /// Returns an iterator over set column indices in the given row.
    ///
    /// This uses the CSR representation for O(nnz_in_row) performance.
    ///
    /// # Panics
    ///
    /// Panics if `row >= rows`.
    #[inline]
    pub fn row_iter(&self, row: usize) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.csr.row_iter(row)
    }

    /// Returns an iterator over set row indices in the given column.
    ///
    /// This uses the CSC representation for O(nnz_in_col) performance
    /// without transposition overhead.
    ///
    /// # Panics
    ///
    /// Panics if `col >= cols`.
    #[inline]
    pub fn col_iter(&self, col: usize) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.csc.row_iter(col) // CSC's rows are original columns
    }

    /// Converts to dense BitMatrix.
    pub fn to_dense(&self) -> BitMatrix {
        self.csr.to_dense()
    }

    /// Returns number of rows.
    #[inline]
    pub fn rows(&self) -> usize {
        self.csr.rows()
    }

    /// Returns number of columns.
    #[inline]
    pub fn cols(&self) -> usize {
        self.csr.cols()
    }

    /// Returns number of nonzeros.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.csr.nnz()
    }

    /// Matrix-vector product y = A · x over GF(2).
    #[inline]
    pub fn matvec(&self, x: &BitVec) -> BitVec {
        self.csr.matvec(x)
    }

    /// Transpose-vector product y = A^T · x over GF(2).
    ///
    /// Uses the CSC representation to compute the transpose-vector product
    /// efficiently without materializing the transpose.
    pub fn matvec_transpose(&self, x: &BitVec) -> BitVec {
        assert_eq!(
            x.len(),
            self.csr.rows(),
            "input BitVec length must equal rows for transpose"
        );
        let mut y = BitVec::with_capacity(self.csr.cols());
        // CSC's row iteration is the transpose's column iteration
        for c in 0..self.csr.cols() {
            let mut acc = false;
            for r in self.col_iter(c) {
                acc ^= x.get(r);
            }
            y.push_bit(acc);
        }
        y
    }

    /// Internal constructor from CSR and CSC data (for deserialization)
    #[cfg(feature = "io")]
    pub(crate) fn from_csr_csc(
        rows: usize,
        cols: usize,
        row_offsets: Vec<usize>,
        row_indices: Vec<usize>,
        col_offsets: Vec<usize>,
        col_indices: Vec<usize>,
    ) -> Self {
        let csr = SpBitMatrix {
            rows,
            cols,
            indptr: row_offsets,
            indices: row_indices,
        };
        let csc = SpBitMatrix {
            rows: cols,
            cols: rows,
            indptr: col_offsets,
            indices: col_indices,
        };
        Self { csr, csc }
    }

    /// Access row offsets (for serialization)
    #[cfg(feature = "io")]
    pub(crate) fn row_offsets(&self) -> &[usize] {
        &self.csr.indptr
    }

    /// Access row indices (for serialization)
    #[cfg(feature = "io")]
    pub(crate) fn row_indices(&self) -> &[usize] {
        &self.csr.indices
    }

    /// Access col offsets (for serialization)
    #[cfg(feature = "io")]
    pub(crate) fn col_offsets(&self) -> &[usize] {
        &self.csc.indptr
    }

    /// Access col indices (for serialization)
    #[cfg(feature = "io")]
    pub(crate) fn col_indices(&self) -> &[usize] {
        &self.csc.indices
    }
}

impl fmt::Display for SpBitMatrix {
    /// Formats the SpBitMatrix in nalgebra-like style.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    /// let s = SpBitMatrix::from_coo(3, 4, &coo);
    /// println!("{}", s);
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

        let border_width = self.cols * 2 + 1;

        writeln!(f, "  ┌{}┐", " ".repeat(border_width))?;

        for r in 0..self.rows {
            write!(f, "  │ ")?;
            let row_cols: Vec<usize> = self.row_iter(r).collect();
            for c in 0..self.cols {
                if row_cols.contains(&c) {
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

impl fmt::Display for SpBitMatrixBlockCsr {
    /// Formats the `SpBitMatrixBlockCsr` in nalgebra-like style.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    /// let s = SpBitMatrix::from_coo(3, 4, &coo).to_block_csr(2);
    /// println!("{}", s);
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rows == 0 || self.cols == 0 {
            return write!(f, "[ ]");
        }

        let border_width = self.cols * 2 + 1;
        writeln!(f, "  ┌{}┐", " ".repeat(border_width))?;

        for r in 0..self.rows {
            let block = r / self.block_rows;
            let local_row = r - block * self.block_rows;
            let offset_start = self.block_ptr[block];
            let nnz_base = self.block_nnz_ptr[block];
            let start = nnz_base + self.row_offsets[offset_start + local_row];
            let end = nnz_base + self.row_offsets[offset_start + local_row + 1];
            let row_cols = &self.indices[start..end];

            write!(f, "  │ ")?;
            for c in 0..self.cols {
                if row_cols.contains(&c) {
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

impl fmt::Display for SpBitMatrixDual {
    /// Formats the SpBitMatrixDual in nalgebra-like style.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SpBitMatrixDual;
    ///
    /// let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    /// let s = SpBitMatrixDual::from_coo(3, 4, &coo);
    /// println!("{}", s);
    /// // Displays:
    /// //   ┌       ┐
    /// //   │ 1 0 0 1 │
    /// //   │ 0 1 0 0 │
    /// //   │ 0 0 1 0 │
    /// //   └       ┘
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.csr, f)
    }
}

#[cfg(feature = "visualization")]
impl SpBitMatrix {
    /// Saves the sparse matrix as a PNG image.
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
    /// use gf2_core::sparse::SpBitMatrix;
    ///
    /// let s = SpBitMatrix::identity(100);
    /// s.save_image("identity.png").unwrap();
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
            let row_cols: Vec<usize> = self.row_iter(row).collect();
            for col in 0..self.cols {
                let bit = row_cols.contains(&col);
                let color = if bit { ONE_COLOR } else { ZERO_COLOR };
                img.put_pixel(col as u32, row as u32, Rgb(color));
            }
        }

        img.save(path)?;
        Ok(())
    }
}

#[cfg(feature = "visualization")]
impl SpBitMatrixDual {
    /// Saves the sparse matrix as a PNG image.
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
    /// use gf2_core::sparse::SpBitMatrixDual;
    ///
    /// let coo = vec![(0, 1), (1, 2)];
    /// let sd = SpBitMatrixDual::from_coo(3, 3, &coo);
    /// sd.save_image("sparse_dual.png").unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be created
    /// - PNG encoding fails
    pub fn save_image(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.csr.save_image(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn bitvec_from_pattern(len: usize, salt: usize) -> BitVec {
        let mut bits = BitVec::with_capacity(len);
        for i in 0..len {
            bits.push_bit(((i.wrapping_mul(17) ^ salt.wrapping_mul(31) ^ (i >> 2)) & 3) != 0);
        }
        bits
    }

    fn assert_rcm_matvec_roundtrip(matrix: &SpBitMatrix, x: &BitVec) {
        let (reordered, permutation) = matrix.reorder_rcm();
        assert_eq!(permutation.rows_len(), matrix.rows());
        assert_eq!(permutation.cols_len(), matrix.cols());

        let y = matrix.matvec(x);
        let x_rcm = permutation.apply_cols(x);
        let y_rcm = reordered.matvec(&x_rcm);
        assert_eq!(permutation.unapply_rows(&y_rcm), y);
    }

    #[test]
    fn rcm_matvec_matches_original_for_edge_shapes() {
        let cases = [
            SpBitMatrix::zeros(0, 0),
            SpBitMatrix::zeros(1, 0),
            SpBitMatrix::zeros(0, 1),
            SpBitMatrix::from_coo(1, 1, &[(0, 0)]),
            SpBitMatrix::from_coo(2, 65, &[(0, 0), (0, 64), (1, 63)]),
            SpBitMatrix::from_coo(65, 66, &[(0, 65), (1, 0), (63, 64), (64, 1), (64, 65)]),
        ];

        for matrix in cases {
            let x = bitvec_from_pattern(matrix.cols(), matrix.rows());
            assert_rcm_matvec_roundtrip(&matrix, &x);
        }
    }

    #[test]
    fn rcm_permutation_roundtrip_identity_for_boundaries() {
        let matrix = SpBitMatrix::from_coo(
            65,
            67,
            &[
                (0, 66),
                (1, 0),
                (2, 65),
                (31, 32),
                (63, 64),
                (64, 1),
                (64, 66),
            ],
        );
        let (_reordered, permutation) = matrix.reorder_rcm();

        let rows = bitvec_from_pattern(permutation.rows_len(), 11);
        let cols = bitvec_from_pattern(permutation.cols_len(), 29);

        assert_eq!(
            permutation.unapply_rows(&permutation.apply_rows(&rows)),
            rows
        );
        assert_eq!(
            permutation.apply_rows(&permutation.unapply_rows(&rows)),
            rows
        );
        assert_eq!(
            permutation.unapply_cols(&permutation.apply_cols(&cols)),
            cols
        );
        assert_eq!(
            permutation.apply_cols(&permutation.unapply_cols(&cols)),
            cols
        );
    }

    #[test]
    fn rcm_maps_indices_bijectively() {
        let matrix = deterministic_ldpc_like_fixture(128, 257, 6);
        let (_reordered, permutation) = matrix.reorder_rcm();

        let mut rows_seen = vec![false; permutation.rows_len()];
        for new_row in 0..permutation.rows_len() {
            let old_row = permutation.old_row_for_new(new_row);
            assert_eq!(permutation.new_row_for_old(old_row), new_row);
            rows_seen[old_row] = true;
        }
        assert!(rows_seen.into_iter().all(|seen| seen));

        let mut cols_seen = vec![false; permutation.cols_len()];
        for new_col in 0..permutation.cols_len() {
            let old_col = permutation.old_col_for_new(new_col);
            assert_eq!(permutation.new_col_for_old(old_col), new_col);
            cols_seen[old_col] = true;
        }
        assert!(cols_seen.into_iter().all(|seen| seen));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        #[test]
        fn proptest_rcm_matvec_matches_original(
            rows in 0usize..24,
            cols in 0usize..24,
            raw_entries in proptest::collection::vec((0usize..24, 0usize..24), 0..160),
            salt in any::<usize>(),
        ) {
            let entries: Vec<_> = raw_entries
                .into_iter()
                .filter_map(|(r, c)| {
                    if rows == 0 || cols == 0 {
                        None
                    } else {
                        Some((r % rows, c % cols))
                    }
                })
                .collect();
            let matrix = SpBitMatrix::from_coo(rows, cols, &entries);
            let x = bitvec_from_pattern(cols, salt);

            let y = matrix.matvec(&x);
            let (reordered, permutation) = matrix.reorder_rcm();
            let y_rcm = reordered.matvec(&permutation.apply_cols(&x));

            prop_assert_eq!(permutation.unapply_rows(&y_rcm), y);
            prop_assert_eq!(permutation.unapply_cols(&permutation.apply_cols(&x)), x);

            let row_bits = bitvec_from_pattern(rows, salt.rotate_left(7));
            prop_assert_eq!(permutation.unapply_rows(&permutation.apply_rows(&row_bits)), row_bits);
        }
    }

    /// Reference oracle: reduce sparse-sparse matmul to dense matmul on
    /// `BitMatrix`, then back to CSR. This is the reference for criterion #2
    /// of the matmul contract.
    fn dense_matmul_reference(a: &SpBitMatrix, b: &SpBitMatrix) -> SpBitMatrix {
        let prod = a.to_dense() * b.to_dense();
        SpBitMatrix::from_dense(&prod)
    }

    /// Verifies that the canonical CSR invariants hold on a freshly multiplied
    /// matrix: indptr is monotone, indices within each row are strictly
    /// ascending, every column index lies in range, and length agrees with
    /// the final indptr value.
    fn assert_csr_canonical(c: &SpBitMatrix) {
        assert_eq!(c.indptr.len(), c.rows + 1, "indptr length must be rows + 1");
        assert_eq!(*c.indptr.first().unwrap(), 0, "first indptr must be 0");
        assert_eq!(
            *c.indptr.last().unwrap(),
            c.indices.len(),
            "last indptr must equal indices length"
        );
        for r in 0..c.rows {
            let s = c.indptr[r];
            let e = c.indptr[r + 1];
            assert!(s <= e, "indptr must be non-decreasing");
            for w in s..e {
                assert!(
                    c.indices[w] < c.cols,
                    "column index out of range: {} >= {}",
                    c.indices[w],
                    c.cols
                );
                if w > s {
                    assert!(
                        c.indices[w - 1] < c.indices[w],
                        "row indices must be strictly ascending and dedup'd"
                    );
                }
            }
        }
    }

    #[test]
    fn matmul_empty_lhs() {
        let a = SpBitMatrix::zeros(0, 5);
        let b = SpBitMatrix::zeros(5, 7);
        let c = a.matmul(&b);
        assert_eq!(c.rows(), 0);
        assert_eq!(c.cols(), 7);
        assert_eq!(c.nnz(), 0);
        assert_csr_canonical(&c);
    }

    #[test]
    fn matmul_empty_rhs_cols() {
        let a = SpBitMatrix::identity(3);
        let b = SpBitMatrix::zeros(3, 0);
        let c = a.matmul(&b);
        assert_eq!(c.rows(), 3);
        assert_eq!(c.cols(), 0);
        assert_eq!(c.nnz(), 0);
        assert_csr_canonical(&c);
    }

    #[test]
    fn matmul_zero_inner_dim() {
        let a = SpBitMatrix::zeros(2, 0);
        let b = SpBitMatrix::zeros(0, 3);
        let c = a.matmul(&b);
        assert_eq!(c.rows(), 2);
        assert_eq!(c.cols(), 3);
        assert_eq!(c.nnz(), 0);
        assert_csr_canonical(&c);
    }

    #[test]
    fn matmul_identity_right_returns_lhs() {
        let a = SpBitMatrix::from_coo(3, 4, &[(0, 0), (0, 3), (1, 2), (2, 1), (2, 3)]);
        let i = SpBitMatrix::identity(4);
        let c = a.matmul(&i);
        assert_eq!(c, a);
        assert_csr_canonical(&c);
    }

    #[test]
    fn matmul_identity_left_returns_rhs() {
        let b = SpBitMatrix::from_coo(4, 3, &[(0, 1), (1, 0), (2, 2), (3, 0), (3, 2)]);
        let i = SpBitMatrix::identity(4);
        let c = i.matmul(&b);
        assert_eq!(c, b);
        assert_csr_canonical(&c);
    }

    #[test]
    fn matmul_xor_cancellation_at_output() {
        // A · A^T where A has exactly two nonzeros in row 0 sharing the same
        // pivot column with A^T's row 0 → contributions cancel under GF(2).
        // A = [[1,1,0]] (1×3). A^T = [[1],[1],[0]] (3×1).
        // (A · A^T)[0,0] = 1·1 + 1·1 + 0·0 = 0 (XOR).
        let a = SpBitMatrix::from_coo(1, 3, &[(0, 0), (0, 1)]);
        let at = a.transpose();
        let c = a.matmul(&at);
        assert_eq!(c.rows(), 1);
        assert_eq!(c.cols(), 1);
        assert_eq!(c.nnz(), 0, "GF(2) self-inner-product of even weight is 0");
        assert_csr_canonical(&c);
    }

    #[test]
    fn matmul_word_boundary_widths() {
        // Cover output column counts spanning u64 word boundaries.
        for &(ar, ak, bc) in &[
            (2usize, 3usize, 63usize),
            (2, 3, 64),
            (2, 3, 65),
            (5, 7, 127),
            (5, 7, 128),
            (5, 7, 129),
        ] {
            let a_entries: Vec<(usize, usize)> =
                (0..ar).flat_map(|r| (0..ak).map(move |k| (r, k))).collect();
            let a = SpBitMatrix::from_coo(ar, ak, &a_entries);
            let b_entries: Vec<(usize, usize)> = (0..ak)
                .map(|k| (k, k.wrapping_mul(0x9E37_79B1) % bc))
                .collect();
            let b = SpBitMatrix::from_coo(ak, bc, &b_entries);

            let c = a.matmul(&b);
            assert_csr_canonical(&c);
            assert_eq!(c, dense_matmul_reference(&a, &b));
            // Sanity: serializing through to_dense round-trips identically.
            assert_eq!(c, SpBitMatrix::from_dense(&c.to_dense()));
        }
    }

    #[test]
    fn matmul_random_seeded_cases() {
        // Three deterministic, low-density inputs of moderate size.
        let cases: &[(usize, usize, usize, u64)] = &[
            (16, 24, 20, 0xA5A5_5A5A_C3C3_3C3C),
            (37, 41, 53, 0xDEAD_BEEF_CAFE_BABE),
            (65, 66, 67, 0x1234_5678_9ABC_DEF0),
        ];
        for &(ar, ak, bc, seed) in cases {
            // Generate sparse A and B with a fixed pseudo-random pattern.
            let mut a_entries = Vec::new();
            let mut x = seed;
            for r in 0..ar {
                for k in 0..ak {
                    x = x
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    if (x >> 60) & 0xF == 0 {
                        a_entries.push((r, k));
                    }
                }
            }
            let a = SpBitMatrix::from_coo(ar, ak, &a_entries);

            let mut b_entries = Vec::new();
            let mut y = !seed;
            for k in 0..ak {
                for c in 0..bc {
                    y = y
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    if (y >> 60) & 0xF == 0 {
                        b_entries.push((k, c));
                    }
                }
            }
            let b = SpBitMatrix::from_coo(ak, bc, &b_entries);

            let c = a.matmul(&b);
            assert_csr_canonical(&c);
            assert_eq!(c, dense_matmul_reference(&a, &b));
        }
    }

    #[test]
    #[should_panic(expected = "matmul inner dimensions must match")]
    fn matmul_dimension_mismatch_panics() {
        let a = SpBitMatrix::zeros(2, 3);
        let b = SpBitMatrix::zeros(4, 5);
        let _ = a.matmul(&b);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        #[test]
        fn proptest_matmul_matches_dense(
            ar in 0usize..14,
            ak in 0usize..14,
            bc in 0usize..14,
            a_raw in proptest::collection::vec((0usize..14, 0usize..14), 0..60),
            b_raw in proptest::collection::vec((0usize..14, 0usize..14), 0..60),
        ) {
            let a_entries: Vec<_> = a_raw
                .into_iter()
                .filter_map(|(r, c)| {
                    if ar == 0 || ak == 0 {
                        None
                    } else {
                        Some((r % ar, c % ak))
                    }
                })
                .collect();
            let b_entries: Vec<_> = b_raw
                .into_iter()
                .filter_map(|(r, c)| {
                    if ak == 0 || bc == 0 {
                        None
                    } else {
                        Some((r % ak, c % bc))
                    }
                })
                .collect();

            let a = SpBitMatrix::from_coo(ar, ak, &a_entries);
            let b = SpBitMatrix::from_coo(ak, bc, &b_entries);

            let c = a.matmul(&b);
            assert_csr_canonical(&c);
            prop_assert_eq!(c, dense_matmul_reference(&a, &b));
        }
    }
}
