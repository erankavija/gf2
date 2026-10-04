//! Sparse GF(2) matrices: CSR ([`SpBitMatrix`]), row-blocked CSR
//! ([`SpBitMatrixBlockCsr`]) and dual CSR+CSC ([`SpBitMatrixDual`]). Nonzero
//! values are implicitly 1, so only index arrays are stored.

use crate::{matrix::BitMatrix, BitVec};
use gf2_kernels_simd::prefetch_read_l1;
use std::fmt;

const DEFAULT_BLOCK_ROWS: usize = 32;
const DEFAULT_PREFETCH_DISTANCE: usize = 0;

/// A row-major sparse matrix in Compressed Sparse Row (CSR) format over GF(2).
///
/// All nonzero entries are implicitly 1; the values array is omitted for GF(2).
///
/// # Storage Layout
///
/// - `indptr`: Array of length `rows + 1`. Row r spans `indices[indptr[r]..indptr[r+1]]`.
/// - `indices`: Packed array of column indices for nonzero entries (sorted per row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpBitMatrix {
    rows: usize,
    cols: usize,
    indptr: Vec<usize>,
    indices: Vec<usize>,
}

/// Descriptive alias for [`SpBitMatrix`].
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
    #[inline]
    pub fn rows_len(&self) -> usize {
        self.old_rows_by_new.len()
    }

    /// Number of matrix columns covered by this permutation.
    #[inline]
    pub fn cols_len(&self) -> usize {
        self.old_cols_by_new.len()
    }

    /// Returns the original row index stored at `new_row` in the reordered matrix.
    ///
    /// # Panics
    ///
    /// Panics if `new_row >= self.rows_len()`.
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
    #[inline]
    pub fn new_row_for_old(&self, old_row: usize) -> usize {
        self.new_rows_by_old[old_row]
    }

    /// Returns the original column index stored at `new_col` in the reordered matrix.
    ///
    /// # Panics
    ///
    /// Panics if `new_col >= self.cols_len()`.
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
    #[inline]
    pub fn new_col_for_old(&self, old_col: usize) -> usize {
        self.new_cols_by_old[old_col]
    }

    /// Applies the row permutation to a vector in original row order.
    ///
    /// The returned vector is in reordered row order: output bit `new_row`
    /// equals input bit `old_row_for_new(new_row)`.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.rows_len()`.
    pub fn apply_rows(&self, bits: &BitVec) -> BitVec {
        apply_bitvec_permutation(bits, &self.old_rows_by_new, "row")
    }

    /// Restores a row vector from reordered row order to original row order.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.rows_len()`.
    pub fn unapply_rows(&self, bits: &BitVec) -> BitVec {
        unapply_bitvec_permutation(bits, &self.old_rows_by_new, "row")
    }

    /// Applies the column permutation to a vector in original column order.
    ///
    /// The returned vector is in reordered column order: output bit `new_col`
    /// equals input bit `old_col_for_new(new_col)`.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.cols_len()`.
    pub fn apply_cols(&self, bits: &BitVec) -> BitVec {
        apply_bitvec_permutation(bits, &self.old_cols_by_new, "column")
    }

    /// Restores a column vector from reordered column order to original column order.
    ///
    /// This is the inverse of [`apply_cols`](Self::apply_cols).
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != self.cols_len()`.
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

/// A row-blocked CSR representation for repeated sparse GF(2) matvecs, built
/// with [`block_csr_from_csr`] or [`SpBitMatrix::to_block_csr`].
///
/// # Storage layout
///
/// Rows are partitioned into fixed-size blocks. Each block stores row offsets
/// relative to the block's first nonzero and keeps the column stream contiguous
/// within each block.
///
/// # Complexity
///
/// Construction and matvec are O(rows + nnz).
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

/// Converts a CSR sparse matrix into the block-CSR layout.
///
/// # Panics
///
/// Panics if `block_rows == 0`.
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
/// # Panics
///
/// Panics if `cols == 0` while `rows` and `row_weight` are nonzero.
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
#[doc(hidden)]
pub fn deterministic_sparse_bitvec_fixture(len: usize) -> BitVec {
    let mut x = BitVec::with_capacity(len);
    for i in 0..len {
        x.push_bit(((i.wrapping_mul(0x9E37_79B1) ^ (i >> 3)) & 7) < 3);
    }
    x
}

impl SpBitMatrixBlockCsr {
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

    /// Number of nonzeros.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Row-block height of this layout.
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
    /// O(rows + nnz). The default schedule issues no software prefetch; see
    /// [`matvec_with_prefetch_distance`](Self::matvec_with_prefetch_distance).
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
    /// # Complexity
    ///
    /// O(rows + nnz) time and O(rows) output storage.
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
    /// [`from_coo_deduplicated`](Self::from_coo_deduplicated) keeps one entry per
    /// coordinate instead.
    ///
    /// # Panics
    ///
    /// Panics if an entry lies outside `rows × cols`.
    pub fn from_coo(rows: usize, cols: usize, entries: &[(usize, usize)]) -> Self {
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
    /// Duplicate entries at the same (row, col) position are ignored.
    ///
    /// For GF(2) XOR semantics where duplicates cancel, use [`from_coo`](Self::from_coo).
    ///
    /// # Panics
    ///
    /// Panics if an entry lies outside `rows × cols`.
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

    /// Computes the reduced row echelon form (RREF) over GF(2) by sparse
    /// column elimination with Markowitz-degree pivot selection.
    ///
    /// Pivot rows come first, in pivot-column order, followed by zero rows.
    ///
    /// # Algorithm
    ///
    /// Each step takes the smallest column `pc` that leads an unused row and,
    /// among the rows led by `pc`, pivots on the one with the fewest nonzeros.
    /// At a fixed pivot column that row minimises the Markowitz product
    /// `(row_nnz(r) - 1) * (col_nnz(c) - 1)`, because `col_nnz(pc)` is the
    /// same for every candidate. The column is then eliminated from every
    /// other row by a symmetric-difference merge of sorted column lists, in
    /// `O(|target| + |source|)`.
    ///
    /// # Complexity
    ///
    /// `O(r·m·w)` for rank `r` and row weights bounded by `w` throughout the
    /// elimination.
    pub fn rref(&self) -> Self {
        let m = self.rows;
        let n = self.cols;

        if m == 0 || n == 0 {
            return Self {
                rows: m,
                cols: n,
                indptr: vec![0; m + 1],
                indices: Vec::new(),
            };
        }

        // CSR rows are already sorted and unique.
        let mut rows: Vec<Vec<usize>> = (0..m)
            .map(|r| {
                let s = self.indptr[r];
                let e = self.indptr[r + 1];
                self.indices[s..e].to_vec()
            })
            .collect();

        // `row_nnz[i] = rows[i].len()`, maintained after each row XOR.
        let mut row_nnz: Vec<usize> = rows.iter().map(|r| r.len()).collect();

        // Symmetric difference of two sorted, strictly ascending column lists:
        // `target ← target XOR source`.
        fn xor_into(target: &mut Vec<usize>, source: &[usize]) {
            let mut merged: Vec<usize> = Vec::with_capacity(target.len() + source.len());
            let mut ti = 0usize;
            let mut si = 0usize;
            while ti < target.len() && si < source.len() {
                let tc = target[ti];
                let sc = source[si];
                match tc.cmp(&sc) {
                    std::cmp::Ordering::Less => {
                        merged.push(tc);
                        ti += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        merged.push(sc);
                        si += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        // tc == sc: cancels in GF(2)
                        ti += 1;
                        si += 1;
                    }
                }
            }
            while ti < target.len() {
                merged.push(target[ti]);
                ti += 1;
            }
            while si < source.len() {
                merged.push(source[si]);
                si += 1;
            }
            *target = merged;
        }

        let mut row_used = vec![false; m];
        // Pivots in pick order, as `(original_row, pivot_col)`.
        let mut pivot_order: Vec<(usize, usize)> = Vec::new();
        for _ in 0..m.min(n) {
            // The pivot column set of an RREF is unique, so each step takes
            // the smallest column that still leads an unused row.
            let mut pc: usize = usize::MAX;
            for i in 0..m {
                if row_used[i] {
                    continue;
                }
                if rows[i].is_empty() {
                    continue;
                }
                let c = rows[i][0];
                if c < pc {
                    pc = c;
                    if pc == 0 {
                        break;
                    }
                }
            }
            if pc == usize::MAX {
                break;
            }
            let mut pi: Option<usize> = None;
            let mut best_rn: usize = usize::MAX;
            for i in 0..m {
                if row_used[i] {
                    continue;
                }
                if rows[i].first().copied() != Some(pc) {
                    continue;
                }
                let rn = row_nnz[i];
                if rn < best_rn {
                    best_rn = rn;
                    pi = Some(i);
                    if rn == 1 {
                        break;
                    }
                }
            }
            let pi = match pi {
                Some(p) => p,
                None => break,
            };

            row_used[pi] = true;
            pivot_order.push((pi, pc));

            // Snapshot the pivot row so `xor_into` can borrow `rows[k]`
            // mutably.
            let pivot_snapshot: Vec<usize> = rows[pi].clone();
            for k in 0..m {
                if k == pi {
                    continue;
                }
                if rows[k].binary_search(&pc).is_err() {
                    continue;
                }
                xor_into(&mut rows[k], &pivot_snapshot);
                row_nnz[k] = rows[k].len();
            }
        }
        pivot_order.sort_by_key(|&(_orig, pc)| pc);

        let mut ordered: Vec<Vec<usize>> = Vec::with_capacity(m);
        for &(orig, _) in &pivot_order {
            ordered.push(std::mem::take(&mut rows[orig]));
        }
        while ordered.len() < m {
            ordered.push(Vec::new());
        }
        let mut indptr = Vec::with_capacity(m + 1);
        let mut indices: Vec<usize> = Vec::new();
        indptr.push(0);
        for row in ordered {
            indices.extend_from_slice(&row);
            indptr.push(indices.len());
        }

        Self {
            rows: m,
            cols: n,
            indptr,
            indices,
        }
    }

    /// Returns the transpose of this CSR matrix as CSR of the transposed shape.
    /// This is O(nnz + rows + cols) and stable by column order.
    pub fn transpose(&self) -> Self {
        let rows_t = self.cols;
        let cols_t = self.rows;
        let nnz = self.indices.len();
        let mut counts = vec![0usize; rows_t];
        for r in 0..self.rows {
            for c in self.row_iter(r) {
                counts[c] += 1;
            }
        }
        let mut indptr = Vec::with_capacity(rows_t + 1);
        indptr.push(0);
        for i in 0..rows_t {
            indptr.push(indptr[i] + counts[i]);
        }
        let mut indices = vec![0usize; nnz];
        let mut next = indptr.clone();
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
    /// Builds a transient transpose: O(nnz + rows + cols) per call.
    ///
    /// # Panics
    ///
    /// Panics if `col >= self.cols()`.
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

    /// Number of nonzeros.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Converts this CSR matrix to the block-CSR matvec layout.
    ///
    /// # Panics
    ///
    /// Panics if `block_rows == 0`.
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
    /// one node per column, and an edge per nonzero entry; both rows and
    /// columns of the returned matrix are in that order.
    ///
    /// For an original input `x`, compute with the reordered matrix as:
    /// `perm.unapply_rows(&reordered.matvec(&perm.apply_cols(&x)))`.
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
    /// Panics if `x.len() != self.cols()`.
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
    /// The output is canonical CSR: column indices within each row are sorted
    /// ascending and free of duplicates. It equals
    /// `SpBitMatrix::from_dense(&(self.to_dense() * other.to_dense()))`
    /// (`proptest_matmul_matches_dense`).
    ///
    /// # Algorithm
    ///
    /// Row-by-row CSR multiply with a dense word-packed XOR accumulator: for
    /// each row `i` of `A` and each nonzero column `k`, every nonzero column
    /// `j` of `B`'s row `k` is toggled in the accumulator; the touched words
    /// are then scanned for the output row and cleared.
    ///
    /// # Panics
    ///
    /// Panics if `self.cols() != other.rows()`.
    ///
    /// # Complexity
    ///
    /// O(Σ_i Σ_{k ∈ A_row_i} nnz(B_row_k)) toggles plus a sort and scan of the
    /// touched accumulator words per output row. Memory is O(cols(B) / 64) for
    /// the accumulator plus O(nnz(C)) for the output.
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

        let mut acc = vec![0u64; n_words];
        // Touched words, so each row clears only what it used.
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

            // Sorted touched words make the emitted column indices ascending.
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

    /// Sparse × dense matrix multiplication `C = A · B` over GF(2), with a
    /// dense `self.rows() × b.cols()` result equal to `self.to_dense() * b`
    /// (`proptest_matmat_matches_dense`).
    ///
    /// # Algorithm
    ///
    /// For each non-zero column `k` of row `i` of `A`, the `k`-th row of `B`
    /// is XORed into output row `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.cols() != b.rows()`.
    ///
    /// # Complexity
    ///
    /// O(nnz(A) · ⌈cols(B) / 64⌉) word-XOR operations.
    pub fn matmat(&self, b: &BitMatrix) -> BitMatrix {
        assert_eq!(
            self.cols,
            b.rows(),
            "matmat inner dimensions must match: lhs.cols={} rhs.rows={}",
            self.cols,
            b.rows(),
        );

        let mut out = BitMatrix::zeros(self.rows, b.cols());
        if self.rows == 0 || b.cols() == 0 || self.cols == 0 {
            return out;
        }

        let xor = crate::kernels::ops::resolve_xor_inplace(out.stride_words());

        for i in 0..self.rows {
            let start = self.indptr[i];
            let end = self.indptr[i + 1];
            if start == end {
                continue;
            }
            let out_row = out.row_words_mut(i);
            for &k in &self.indices[start..end] {
                xor(out_row, b.row_words(k));
            }
        }
        out
    }
}

/// Dual representation storing the same matrix in both CSR and CSC formats,
/// so row and column iteration are O(nnz_in_row) and O(nnz_in_col) without a
/// transposition, at twice the index storage of a single CSR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpBitMatrixDual {
    csr: SpBitMatrix,
    csc: SpBitMatrix,
}

impl SpBitMatrixDual {
    /// Creates a dual representation from a dense BitMatrix.
    pub fn from_dense(m: &BitMatrix) -> Self {
        let csr = SpBitMatrix::from_dense(m);
        let csc = csr.transpose();
        Self { csr, csc }
    }

    /// Creates a dual representation from COO coordinates with XOR semantics.
    ///
    /// Duplicates cancel (even count → 0, odd count → 1).
    /// For deduplication semantics, use [`from_coo_deduplicated`](Self::from_coo_deduplicated).
    ///
    /// # Panics
    ///
    /// Panics if an entry lies outside `rows × cols`.
    pub fn from_coo(rows: usize, cols: usize, entries: &[(usize, usize)]) -> Self {
        let csr = SpBitMatrix::from_coo(rows, cols, entries);
        let csc = csr.transpose();
        Self { csr, csc }
    }

    /// Creates a dual representation from COO coordinates with deduplication.
    ///
    /// Duplicate entries are ignored.
    ///
    /// # Panics
    ///
    /// Panics if an entry lies outside `rows × cols`.
    pub fn from_coo_deduplicated(rows: usize, cols: usize, entries: &[(usize, usize)]) -> Self {
        let csr = SpBitMatrix::from_coo_deduplicated(rows, cols, entries);
        let csc = csr.transpose();
        Self { csr, csc }
    }

    /// Returns an iterator over set column indices in the given row.
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

    /// Number of rows.
    #[inline]
    pub fn rows(&self) -> usize {
        self.csr.rows()
    }

    /// Number of columns.
    #[inline]
    pub fn cols(&self) -> usize {
        self.csr.cols()
    }

    /// Number of nonzeros.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.csr.nnz()
    }

    /// Matrix-vector product y = A · x over GF(2).
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.cols()`.
    #[inline]
    pub fn matvec(&self, x: &BitVec) -> BitVec {
        self.csr.matvec(x)
    }

    /// Reduced row echelon form (RREF) of `self` over GF(2), computed by
    /// [`SpBitMatrix::rref`]; the CSC half is rebuilt by transposition.
    pub fn rref(&self) -> Self {
        let csr = self.csr.rref();
        let csc = csr.transpose();
        Self { csr, csc }
    }

    /// Transpose-vector product y = A^T · x over GF(2).
    ///
    /// Panics if `x.len() != self.rows()`.
    pub fn matvec_transpose(&self, x: &BitVec) -> BitVec {
        assert_eq!(
            x.len(),
            self.csr.rows(),
            "input BitVec length must equal rows for transpose"
        );
        let mut y = BitVec::with_capacity(self.csr.cols());
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.csr, f)
    }
}

#[cfg(feature = "visualization")]
impl SpBitMatrix {
    /// Saves the sparse matrix as a PNG image, one pixel per bit: unset bits
    /// black, set bits white.
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
    /// Saves the sparse matrix as a PNG image, one pixel per bit: unset bits
    /// black, set bits white.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or PNG encoding fails.
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

    fn dense_matmul_reference(a: &SpBitMatrix, b: &SpBitMatrix) -> SpBitMatrix {
        let prod = a.to_dense() * b.to_dense();
        SpBitMatrix::from_dense(&prod)
    }

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
            assert_eq!(c, SpBitMatrix::from_dense(&c.to_dense()));
        }
    }

    #[test]
    fn matmul_random_seeded_cases() {
        let cases: &[(usize, usize, usize, u64)] = &[
            (16, 24, 20, 0xA5A5_5A5A_C3C3_3C3C),
            (37, 41, 53, 0xDEAD_BEEF_CAFE_BABE),
            (65, 66, 67, 0x1234_5678_9ABC_DEF0),
        ];
        for &(ar, ak, bc, seed) in cases {
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

    fn matmat_dense_reference(a: &SpBitMatrix, b: &BitMatrix) -> BitMatrix {
        a.to_dense() * b.clone()
    }

    /// LCG with the multiplier of `@/citation/Knuth1997`.
    fn matmat_sparse_from_seed(m: usize, n: usize, density: f64, seed: u64) -> SpBitMatrix {
        let mut entries: Vec<(usize, usize)> = Vec::new();
        let mut st = seed;
        let threshold = if density >= 1.0 {
            u64::MAX
        } else {
            (density * (u64::MAX as f64 + 1.0)) as u64
        };
        for r in 0..m {
            for c in 0..n {
                st = st
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let mix = st ^ (st >> 31);
                if mix < threshold {
                    entries.push((r, c));
                }
            }
        }
        SpBitMatrix::from_coo(m, n, &entries)
    }

    fn matmat_dense_from_seed(m: usize, n: usize, seed: u64) -> BitMatrix {
        let mut out = BitMatrix::zeros(m, n);
        let mut st = seed;
        for r in 0..m {
            for c in 0..n {
                st = st
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                if (st >> 33) & 1 == 1 {
                    out.set(r, c, true);
                }
            }
        }
        out
    }

    #[test]
    fn test_matmat_empty() {
        let a = SpBitMatrix::zeros(0, 0);
        let b = BitMatrix::zeros(0, 0);
        let c = a.matmat(&b);
        assert_eq!(c.rows(), 0);
        assert_eq!(c.cols(), 0);
        assert_eq!(c, matmat_dense_reference(&a, &b));
        let a2 = SpBitMatrix::zeros(0, 5);
        let b2 = BitMatrix::zeros(5, 7);
        let c2 = a2.matmat(&b2);
        assert_eq!(c2.rows(), 0);
        assert_eq!(c2.cols(), 7);
        assert_eq!(c2, matmat_dense_reference(&a2, &b2));
        let a3 = SpBitMatrix::identity(3);
        let b3 = BitMatrix::zeros(3, 0);
        let c3 = a3.matmat(&b3);
        assert_eq!(c3.rows(), 3);
        assert_eq!(c3.cols(), 0);
        assert_eq!(c3, matmat_dense_reference(&a3, &b3));
    }

    #[test]
    fn test_matmat_single_bit() {
        for &k in &[1usize, 64, 65] {
            let a = SpBitMatrix::from_coo(1, 1, &[(0, 0)]);
            let b = matmat_dense_from_seed(1, k, 0xA5A5_5A5A_C3C3_3C3C ^ k as u64);
            let c = a.matmat(&b);
            assert_eq!(c.rows(), 1);
            assert_eq!(c.cols(), k);
            assert_eq!(c, matmat_dense_reference(&a, &b));
            let a_zero = SpBitMatrix::zeros(1, 1);
            let c_zero = a_zero.matmat(&b);
            assert_eq!(c_zero.rows(), 1);
            assert_eq!(c_zero.cols(), k);
            assert_eq!(c_zero, matmat_dense_reference(&a_zero, &b));
        }
    }

    fn run_word_boundary_case(n: usize, seed: u64) {
        let density = if n == 0 {
            0.0
        } else {
            8.0 / (n as f64).max(1.0)
        };
        let a = matmat_sparse_from_seed(n.max(1), n.max(1), density.min(1.0), seed);
        let b = matmat_dense_from_seed(n.max(1), n, seed ^ 0xDEAD_BEEF);
        let c = a.matmat(&b);
        assert_eq!(c.rows(), a.rows());
        assert_eq!(c.cols(), b.cols());
        assert_eq!(c, matmat_dense_reference(&a, &b));
    }

    #[test]
    fn test_matmat_word_boundary_below() {
        run_word_boundary_case(63, 0x1111_2222_3333_4444);
    }

    #[test]
    fn test_matmat_exact_word() {
        run_word_boundary_case(64, 0x5555_6666_7777_8888);
    }

    #[test]
    fn test_matmat_word_boundary_above() {
        run_word_boundary_case(65, 0x9999_AAAA_BBBB_CCCC);
    }

    #[test]
    fn test_matmat_two_words_below() {
        run_word_boundary_case(1023, 0x0102_0304_0506_0708);
    }

    #[test]
    fn test_matmat_two_words_exact() {
        run_word_boundary_case(1024, 0x1020_3040_5060_7080);
    }

    #[test]
    fn test_matmat_two_words_above() {
        run_word_boundary_case(1025, 0xFEDC_BA98_7654_3210);
    }

    #[test]
    fn test_matmat_xor_cancellation() {
        let a = SpBitMatrix::from_coo(1, 3, &[(0, 0), (0, 1)]);
        let mut b = BitMatrix::zeros(3, 2);
        b.set(0, 0, true);
        b.set(1, 0, true);
        b.set(2, 1, true);
        let c = a.matmat(&b);
        assert_eq!(c.rows(), 1);
        assert_eq!(c.cols(), 2);
        assert!(!c.get(0, 0));
        assert!(!c.get(0, 1));
        assert_eq!(c, matmat_dense_reference(&a, &b));
    }

    #[test]
    fn test_matmat_identity_left() {
        for &n in &[1usize, 63, 64, 65] {
            let i = SpBitMatrix::identity(n);
            let b = matmat_dense_from_seed(n, n + 7, 0xDEAD_BEEF_DEAD_BEEF ^ n as u64);
            let c = i.matmat(&b);
            assert_eq!(c, b);
        }
    }

    fn dense_rref_reference(m: &SpBitMatrix) -> SpBitMatrix {
        let r = crate::alg::rref::rref(&m.to_dense(), false);
        SpBitMatrix::from_dense(&r.reduced)
    }

    #[test]
    fn test_rref_empty_shapes() {
        for (r, c) in [(0usize, 0usize), (0, 5), (5, 0)] {
            let m = SpBitMatrix::zeros(r, c);
            let out = m.rref();
            assert_eq!(out.rows(), r);
            assert_eq!(out.cols(), c);
            assert_eq!(out.nnz(), 0);
            assert_csr_canonical(&out);
        }
    }

    #[test]
    #[should_panic(expected = "matmat inner dimensions must match")]
    fn test_matmat_dimension_mismatch_panics() {
        let a = SpBitMatrix::zeros(2, 3);
        let b = BitMatrix::zeros(4, 5);
        let _ = a.matmat(&b);
    }

    #[test]
    fn test_rref_single_row_single_col_one() {
        let m = SpBitMatrix::from_coo(1, 1, &[(0, 0)]);
        let out = m.rref();
        assert_eq!(out.nnz(), 1);
        assert_eq!(out.to_dense(), m.to_dense());
        assert_csr_canonical(&out);
    }

    #[test]
    fn test_rref_single_row_single_col_zero() {
        let m = SpBitMatrix::zeros(1, 1);
        let out = m.rref();
        assert_eq!(out.nnz(), 0);
        assert_csr_canonical(&out);
    }

    #[test]
    fn test_rref_identity_is_idempotent() {
        for n in [1usize, 63, 64, 65] {
            let id = SpBitMatrix::identity(n);
            let out = id.rref();
            assert_eq!(out.nnz(), n);
            assert_eq!(out, id);
            assert_csr_canonical(&out);
        }
    }

    #[test]
    fn test_rref_singular_matrix_drops_dependent_row() {
        let entries = [(0, 0), (0, 2), (1, 1), (1, 2), (2, 0), (2, 1)];
        let m = SpBitMatrix::from_coo(3, 3, &entries);
        let out = m.rref();
        assert_csr_canonical(&out);
        let nnz_per_row: Vec<usize> = (0..out.rows())
            .map(|r| out.indptr[r + 1] - out.indptr[r])
            .collect();
        assert!(nnz_per_row[2] == 0, "third row should be zero post-RREF");
        assert_eq!(out, dense_rref_reference(&m));
    }

    #[test]
    fn test_rref_word_boundary_n64() {
        let mut entries: Vec<(usize, usize)> = (0..64).map(|i| (i, i)).collect();
        entries.push((0, 32));
        entries.push((32, 63));
        let m = SpBitMatrix::from_coo(64, 64, &entries);
        let out = m.rref();
        assert_csr_canonical(&out);
        assert_eq!(out, dense_rref_reference(&m));
    }

    #[test]
    fn test_rref_word_boundary_n65() {
        let mut entries: Vec<(usize, usize)> = (0..65).map(|i| (i, i)).collect();
        entries.push((0, 64));
        entries.push((64, 0));
        let m = SpBitMatrix::from_coo(65, 65, &entries);
        let out = m.rref();
        assert_csr_canonical(&out);
        assert_eq!(out, dense_rref_reference(&m));
    }

    #[test]
    fn test_rref_random_seeded_n1024_matches_dense() {
        let n = 1024usize;
        let density = 10.0 / n as f64;
        let seed = crate::bench_seed::derive_seed(0xDEAD_BEEF, "spelim-test", 0, 0, 1);
        let m = crate::bench_seed::bitmatrix_sparse_from_seed(n, n, density, seed);
        let out = m.rref();
        assert_csr_canonical(&out);
        assert_eq!(out, dense_rref_reference(&m));
    }

    #[test]
    fn test_rref_dual_matches_csr_path() {
        let entries = [(0, 0), (0, 2), (1, 1), (1, 2), (2, 0), (2, 1)];
        let csr = SpBitMatrix::from_coo(3, 3, &entries);
        let dual = SpBitMatrixDual::from_coo(3, 3, &entries);
        let out_csr = csr.rref();
        let out_dual = dual.rref();
        assert_eq!(out_csr.to_dense(), out_dual.to_dense());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn proptest_matmat_matches_dense(
            ar in 1usize..=256,
            ak in 1usize..=256,
            bc in 1usize..=256,
            a_raw in proptest::collection::vec((0usize..256, 0usize..256), 0..256),
            b_seed in any::<u64>(),
        ) {
            let a_entries: Vec<_> = a_raw
                .into_iter()
                .map(|(r, c)| (r % ar, c % ak))
                .collect();

            let a = SpBitMatrix::from_coo(ar, ak, &a_entries);
            let b = matmat_dense_from_seed(ak, bc, b_seed);

            let c = a.matmat(&b);
            prop_assert_eq!(c.rows(), ar);
            prop_assert_eq!(c.cols(), bc);
            prop_assert_eq!(c, matmat_dense_reference(&a, &b));
        }

        #[test]
        fn proptest_rref_matches_dense_reference(
            rows in 0usize..16,
            cols in 0usize..16,
            raw_entries in proptest::collection::vec((0usize..16, 0usize..16), 0..80),
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
            let m = SpBitMatrix::from_coo(rows, cols, &entries);
            let out = m.rref();
            assert_csr_canonical(&out);
            prop_assert_eq!(out, dense_rref_reference(&m));
        }

        /// RREF is unique, so equality with the dense reference holds whatever
        /// the internal pivot order.
        #[test]
        fn proptest_rref_markowitz_byte_equality(
            rows in 0usize..=65,
            cols in 0usize..=65,
            entry_count in 0usize..=200,
            seed in any::<u64>(),
        ) {
            // splitmix64, for reproducibility under proptest shrinking.
            let mut st = seed;
            fn next(st: &mut u64) -> u64 {
                *st = st.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = *st;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            }
            let mut entries: Vec<(usize, usize)> = Vec::new();
            if rows > 0 && cols > 0 {
                for _ in 0..entry_count {
                    let r = (next(&mut st) as usize) % rows;
                    let c = (next(&mut st) as usize) % cols;
                    entries.push((r, c));
                }
            }
            let m = SpBitMatrix::from_coo(rows, cols, &entries);
            let out = m.rref();
            assert_csr_canonical(&out);
            prop_assert_eq!(out.clone(), dense_rref_reference(&m));
            let out2 = out.rref();
            prop_assert_eq!(out2, out);
        }
    }
}
