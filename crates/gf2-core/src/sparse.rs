//! Sparse matrix primitives for GF(2) with CSR/COO representations.
//!
//! This module provides memory-efficient sparse matrix support for low-density
//! matrices (< 1% density) over GF(2), optimized for LDPC code workloads.
//!
//! # Storage Formats
//!
//! - **CSR (Compressed Sparse Row)**: Row-major format optimized for row iteration
//!   and matrix-vector multiply. All nonzero values are implicitly 1 in GF(2).
//!
//! # Examples
//!
//! ```
//! use gf2_core::sparse::SparseMatrix;
//! use gf2_core::BitVec;
//!
//! // Build from COO (coordinate) format
//! let coo = vec![(0, 1), (0, 3), (1, 2)];
//! let s = SparseMatrix::from_coo(2, 4, &coo);
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

/// A row-major sparse matrix in Compressed Sparse Row (CSR) format over GF(2).
///
/// Optimized for low-density matrices (< 1% nonzeros) typical in LDPC codes.
/// All nonzero entries are implicitly 1; the values array is omitted for GF(2).
///
/// # Storage Layout
///
/// - `indptr`: Array of length `rows + 1`. Row r spans `indices[indptr[r]..indptr[r+1]]`.
/// - `indices`: Packed array of column indices for nonzero entries.
/// - Duplicate coordinates XOR (even count cancels) in COO construction.
///
/// # Examples
///
/// ```
/// use gf2_core::sparse::SparseMatrix;
///
/// let s = SparseMatrix::identity(3);
/// assert_eq!(s.rows(), 3);
/// assert_eq!(s.cols(), 3);
/// assert_eq!(s.nnz(), 3);
///
/// // Iterate over nonzero columns in row 1
/// let cols: Vec<_> = s.row_iter(1).collect();
/// assert_eq!(cols, vec![1]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseMatrix {
    rows: usize,
    cols: usize,
    indptr: Vec<usize>,
    indices: Vec<usize>,
}

impl SparseMatrix {
    /// Creates an all-zero sparse matrix with given shape.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::sparse::SparseMatrix;
    ///
    /// let s = SparseMatrix::zeros(10, 20);
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
    /// use gf2_core::sparse::SparseMatrix;
    ///
    /// let coo = vec![(0, 2), (0, 5), (1, 3)];
    /// let s = SparseMatrix::from_coo(2, 6, &coo);
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
}
