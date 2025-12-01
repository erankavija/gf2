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
use std::fmt;

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
