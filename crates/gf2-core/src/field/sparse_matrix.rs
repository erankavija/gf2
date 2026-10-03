//! Sparse matrices over a generic [`FiniteField`]: Compressed-Sparse-Row
//! ([`SparseFieldMatrix<F>`]) and Compressed-Sparse-Column
//! ([`SparseFieldMatrixCsc<F>`]) storage with conversions, element access,
//! matrix–vector and matrix–matrix products, and reduced row-echelon form.
//! The field-generic counterpart of [`crate::sparse::SpBitMatrix`];
//! duplicate triplets sum where the GF(2) type cancels them. Rows and
//! columns stay in natural order.

use crate::field::matrix::FieldMatrix;
use crate::field::vec::FieldVec;
use crate::field::{ConstField, FiniteField};
use crate::matrix_like::MatrixLike;

/// Row-major sparse matrix over a [`FiniteField`] in Compressed-Sparse-Row
/// (CSR) form.
///
/// All stored values are non-zero (constructors canonicalise) and per-row
/// column indices are sorted ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseFieldMatrix<F: FiniteField> {
    rows: usize,
    cols: usize,
    /// Length `rows + 1`. Row `r` owns indices `row_ptr[r]..row_ptr[r + 1]`.
    row_ptr: Vec<usize>,
    /// Column indices of non-zero entries. Sorted ascending within each row.
    col_idx: Vec<usize>,
    /// Non-zero values, aligned with [`col_idx`]; guaranteed non-zero.
    values: Vec<F>,
}

/// Column-major sparse matrix over a [`FiniteField`] in
/// Compressed-Sparse-Column (CSC) form.
///
/// All stored values are non-zero and per-column row indices are sorted
/// ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseFieldMatrixCsc<F: FiniteField> {
    rows: usize,
    cols: usize,
    /// Length `cols + 1`. Column `c` owns indices `col_ptr[c]..col_ptr[c + 1]`.
    col_ptr: Vec<usize>,
    /// Row indices of non-zero entries. Sorted ascending within each column.
    row_idx: Vec<usize>,
    /// Non-zero values aligned with [`row_idx`]; guaranteed non-zero.
    values: Vec<F>,
}

/// Returns a field zero from the first element of `from_values`, else from
/// [`FiniteField::zero_hint`]. Panics when neither exists.
fn zero_witness<F: FiniteField>(from_values: &[F]) -> F {
    if let Some(v) = from_values.first() {
        v.zero_like()
    } else if let Some(z) = F::zero_hint() {
        z
    } else {
        panic!(
            "SparseFieldMatrix: no zero witness available; \
             use F: ConstField or build the matrix with at least one non-zero entry"
        );
    }
}

/// [`zero_witness`] over two candidate slices.
fn zero_witness_pair<F: FiniteField>(a: &[F], b: &[F]) -> F {
    if let Some(v) = a.first() {
        v.zero_like()
    } else if let Some(v) = b.first() {
        v.zero_like()
    } else if let Some(z) = F::zero_hint() {
        z
    } else {
        panic!(
            "SparseFieldMatrix: no zero witness available; \
             use F: ConstField or provide at least one non-zero operand"
        );
    }
}

impl<F: FiniteField> SparseFieldMatrix<F> {
    /// Creates a structurally empty `rows × cols` sparse matrix (no stored
    /// non-zeros).
    ///
    /// # Complexity
    ///
    /// O(rows) — only the `row_ptr` array is allocated.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            row_ptr: vec![0; rows + 1],
            col_idx: Vec::new(),
            values: Vec::new(),
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

    /// Returns `(rows, cols)`.
    #[inline]
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Returns the number of stored non-zero entries.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Returns the value stored at `(row, col)`, or `F::zero()` if the entry
    /// is structurally absent.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()` or `col >= self.cols()`. Also panics
    /// if the stored `F` is a runtime-context field with no zero witness and
    /// the queried cell is structurally zero — callers on such fields should
    /// keep at least one non-zero in the matrix or use [`ConstField`].
    ///
    /// # Complexity
    ///
    /// `O(log k)` where `k` is the number of non-zeros in the target row,
    /// via binary search on the sorted column-index slice.
    pub fn get(&self, row: usize, col: usize) -> F {
        assert!(
            row < self.rows,
            "SparseFieldMatrix::get: row {row} out of bounds (rows={})",
            self.rows
        );
        assert!(
            col < self.cols,
            "SparseFieldMatrix::get: col {col} out of bounds (cols={})",
            self.cols
        );
        let start = self.row_ptr[row];
        let end = self.row_ptr[row + 1];
        let slice = &self.col_idx[start..end];
        match slice.binary_search(&col) {
            Ok(off) => self.values[start + off].clone(),
            Err(_) => zero_witness(&self.values),
        }
    }

    /// Iterates over the non-zero entries of `row` as `(col, &value)` pairs,
    /// sorted by `col`.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()`.
    ///
    /// # Complexity
    ///
    /// O(nnz_in_row).
    pub fn row_iter(&self, row: usize) -> impl ExactSizeIterator<Item = (usize, &F)> + '_ {
        assert!(
            row < self.rows,
            "SparseFieldMatrix::row_iter: row {row} out of bounds (rows={})",
            self.rows
        );
        let start = self.row_ptr[row];
        let end = self.row_ptr[row + 1];
        self.col_idx[start..end]
            .iter()
            .copied()
            .zip(self.values[start..end].iter())
    }

    /// Returns `(row_ptr, col_idx, values)` as borrowed slices; `row_ptr`
    /// has length `rows + 1`.
    #[inline]
    pub fn as_raw_parts(&self) -> (&[usize], &[usize], &[F]) {
        (&self.row_ptr, &self.col_idx, &self.values)
    }

    /// Builds a sparse matrix from an arbitrary triplet stream, canonicalising
    /// the result: duplicate `(row, col)` pairs are **summed** (field
    /// arithmetic), explicit zeros are dropped, and column indices are sorted
    /// ascending within each row.
    ///
    /// # Panics
    ///
    /// Panics on out-of-bounds indices.
    ///
    /// # Complexity
    ///
    /// O(nnz log(nnz/rows)) from the per-row sort, plus O(nnz) for the
    /// duplicate-merge and zero-drop pass.
    pub fn from_triplets<I>(rows: usize, cols: usize, triplets: I) -> Self
    where
        I: IntoIterator<Item = (usize, usize, F)>,
    {
        let mut per_row: Vec<Vec<(usize, F)>> = (0..rows).map(|_| Vec::new()).collect();
        for (r, c, v) in triplets {
            assert!(
                r < rows,
                "SparseFieldMatrix::from_triplets: row {r} out of bounds (rows={rows})"
            );
            assert!(
                c < cols,
                "SparseFieldMatrix::from_triplets: col {c} out of bounds (cols={cols})"
            );
            per_row[r].push((c, v));
        }

        let mut row_ptr = Vec::with_capacity(rows + 1);
        let mut col_idx: Vec<usize> = Vec::new();
        let mut values: Vec<F> = Vec::new();
        row_ptr.push(0);

        for bucket in per_row.iter_mut() {
            bucket.sort_by_key(|&(c, _)| c);
            let mut i = 0;
            while i < bucket.len() {
                let c = bucket[i].0;
                let mut acc = bucket[i].1.clone();
                let mut j = i + 1;
                while j < bucket.len() && bucket[j].0 == c {
                    acc += &bucket[j].1;
                    j += 1;
                }
                if !acc.is_zero() {
                    col_idx.push(c);
                    values.push(acc);
                }
                i = j;
            }
            row_ptr.push(values.len());
        }

        Self {
            rows,
            cols,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Builds a sparse matrix by scanning a dense [`FieldMatrix<F>`] and
    /// recording non-zero cells in row-major order.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) scalar comparisons; allocates exactly `nnz`
    /// `(col_idx, value)` pairs.
    pub fn from_dense(m: &FieldMatrix<F>) -> Self {
        let rows = m.rows();
        let cols = m.cols();
        let mut row_ptr = Vec::with_capacity(rows + 1);
        let mut col_idx: Vec<usize> = Vec::new();
        let mut values: Vec<F> = Vec::new();
        row_ptr.push(0);
        for r in 0..rows {
            for c in 0..cols {
                let v = m.get(r, c);
                if !v.is_zero() {
                    col_idx.push(c);
                    values.push(v);
                }
            }
            row_ptr.push(values.len());
        }
        Self {
            rows,
            cols,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Materialises a dense [`FieldMatrix<F>`].
    ///
    /// # Panics
    ///
    /// Panics on a non-empty shape with no stored entries when `F` provides
    /// no [`FiniteField::zero_hint`] witness.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) — the dense back-buffer is zero-filled and then the
    /// `nnz` stored entries are scattered into place.
    pub fn to_dense(&self) -> FieldMatrix<F> {
        if self.rows == 0 || self.cols == 0 {
            return FieldMatrix::<F>::from_raw_parts(self.rows, self.cols, FieldVec::new());
        }
        let zero = zero_witness(&self.values);
        let mut out = FieldMatrix::<F>::from_raw_parts(
            self.rows,
            self.cols,
            FieldVec::zeros_from(self.rows * self.cols, &zero),
        );
        for r in 0..self.rows {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            for k in start..end {
                out.set(r, self.col_idx[k], self.values[k].clone());
            }
        }
        out
    }

    /// Converts this CSR matrix to a [`SparseFieldMatrixCsc`] of the same
    /// shape.
    ///
    /// # Complexity
    ///
    /// O(nnz + rows + cols).
    pub fn to_csc(&self) -> SparseFieldMatrixCsc<F> {
        let nnz = self.values.len();
        let mut counts = vec![0usize; self.cols];
        for &c in &self.col_idx {
            counts[c] += 1;
        }
        let mut col_ptr = Vec::with_capacity(self.cols + 1);
        col_ptr.push(0);
        for i in 0..self.cols {
            col_ptr.push(col_ptr[i] + counts[i]);
        }
        // Working write-heads, initialised to the start of each column run.
        let mut next = col_ptr.clone();
        // Initial entries cloned from an existing value keep the storage
        // type-correct without requiring `F: ConstField`.
        let mut row_idx = vec![0usize; nnz];
        let mut values: Vec<F> = if nnz == 0 {
            Vec::new()
        } else {
            (0..nnz).map(|_| self.values[0].clone()).collect()
        };

        // Row-major scatter. CSR was built with rows in natural order, so
        // the emitted row indices per column are already ascending.
        for r in 0..self.rows {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            for k in start..end {
                let c = self.col_idx[k];
                let pos = next[c];
                row_idx[pos] = r;
                values[pos] = self.values[k].clone();
                next[c] += 1;
            }
        }

        SparseFieldMatrixCsc {
            rows: self.rows,
            cols: self.cols,
            col_ptr,
            row_idx,
            values,
        }
    }

    /// Creates the `n × n` identity matrix.
    pub fn identity(n: usize) -> Self
    where
        F: ConstField,
    {
        let row_ptr: Vec<usize> = (0..=n).collect();
        let col_idx: Vec<usize> = (0..n).collect();
        let values: Vec<F> = (0..n).map(|_| F::one()).collect();
        Self {
            rows: n,
            cols: n,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Computes `y = A · x` using CSR row iteration.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.cols()`. Also panics on a `rows > 0`,
    /// `cols == 0` matrix if `F::zero_hint()` returns `None`.
    ///
    /// # Complexity
    ///
    /// O(nnz) multiply-adds.
    pub fn matvec(&self, x: &FieldVec<F>) -> FieldVec<F> {
        assert_eq!(
            x.len(),
            self.cols,
            "SparseFieldMatrix::matvec: x.len() ({}) != cols ({})",
            x.len(),
            self.cols
        );
        if self.rows == 0 {
            return FieldVec::new();
        }
        let zero: F = zero_witness_pair(self.values.as_slice(), x.as_slice());
        let mut y: FieldVec<F> = FieldVec::zeros_from(self.rows, &zero);
        let xs = x.as_slice();
        // Per-row delayed-reduction dot product over the CSR slices, with
        // no gather buffer: products accumulate in `F::Wide` and are reduced
        // once per chunk of at most `max_unreduced_additions()` terms.
        let kmax = F::max_unreduced_additions();
        for r in 0..self.rows {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            if start == end {
                continue;
            }
            let values_row = &self.values[start..end];
            let cols_row = &self.col_idx[start..end];
            let n = values_row.len();

            let dot: F = if kmax == usize::MAX {
                // No overflow is possible (e.g. GF(2^m), `Wide = Self`).
                let mut acc = values_row[0].mul_product_sum_wide(&xs[cols_row[0]]);
                for i in 1..n {
                    acc += values_row[i].mul_product_sum_wide(&xs[cols_row[i]]);
                }
                F::reduce_product_sum_wide(&acc)
            } else if kmax == 0 {
                // Degenerate: reduce after every multiply.
                let mut acc = values_row[0].clone() * xs[cols_row[0]].clone();
                for i in 1..n {
                    acc += &(values_row[i].clone() * xs[cols_row[i]].clone());
                }
                acc
            } else {
                let mut result = zero.zero_like();
                let mut offset = 0usize;
                while offset < n {
                    let chunk_size = (n - offset).min(kmax);
                    let mut acc = values_row[offset].mul_product_sum_wide(&xs[cols_row[offset]]);
                    for i in 1..chunk_size {
                        acc +=
                            values_row[offset + i].mul_product_sum_wide(&xs[cols_row[offset + i]]);
                    }
                    result += &F::reduce_product_sum_wide(&acc);
                    offset += chunk_size;
                }
                result
            };
            y.set(r, dot);
        }
        y
    }

    /// Computes `y = Aᵀ · x`, with `x.len() == self.rows()` and
    /// `y.len() == self.cols()`.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.rows()`. Also panics on
    /// `(rows == 0, cols > 0)` runtime-field shape if `F::zero_hint()` is
    /// `None`.
    ///
    /// # Complexity
    ///
    /// O(nnz) multiply-adds.
    pub fn matvec_transpose(&self, x: &FieldVec<F>) -> FieldVec<F> {
        assert_eq!(
            x.len(),
            self.rows,
            "SparseFieldMatrix::matvec_transpose: x.len() ({}) != rows ({})",
            x.len(),
            self.rows
        );
        if self.cols == 0 {
            return FieldVec::new();
        }
        let zero: F = zero_witness_pair(self.values.as_slice(), x.as_slice());
        let mut y: FieldVec<F> = FieldVec::zeros_from(self.cols, &zero);
        for (r, xr) in x.as_slice().iter().enumerate().take(self.rows) {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            if start == end {
                continue;
            }
            for k in start..end {
                let c = self.col_idx[k];
                let contrib = self.values[k].clone() * xr;
                let updated = y[c].clone() + contrib;
                y.set(c, updated);
            }
        }
        y
    }

    /// Computes `C = A · B` where `A` is this sparse matrix and `B` is a
    /// dense [`FieldMatrix`]. The result `C` is dense and has shape
    /// `self.rows × B.cols`.
    ///
    /// # Panics
    ///
    /// Panics if `self.cols() != b.rows()`. Also panics if `self.cols() == 0`,
    /// both result dimensions are non-zero and `F::zero_hint()` returns
    /// `None`.
    ///
    /// # Complexity
    ///
    /// O(nnz · B.cols) multiply-adds — one pass per stored non-zero of `A`
    /// and per column of `B`.
    pub fn matmat(&self, b: &FieldMatrix<F>) -> FieldMatrix<F> {
        assert_eq!(
            self.cols,
            b.rows(),
            "SparseFieldMatrix::matmat: A.cols ({}) != B.rows ({})",
            self.cols,
            b.rows()
        );
        let out_cols = b.cols();
        if self.rows == 0 || out_cols == 0 {
            return FieldMatrix::<F>::from_raw_parts(self.rows, out_cols, FieldVec::new());
        }
        let zero = if let Some(v) = self.values.first() {
            v.zero_like()
        } else if b.rows() > 0 && b.cols() > 0 {
            b.get(0, 0).zero_like()
        } else if let Some(z) = F::zero_hint() {
            z
        } else {
            panic!(
                "SparseFieldMatrix::matmat: no zero witness; use F: ConstField \
                 or supply at least one non-zero operand"
            );
        };
        {
            let mut out_buf: Vec<F> = vec![zero.clone(); self.rows * out_cols];
            if F::try_simd_spmm(
                &self.row_ptr,
                &self.col_idx,
                &self.values,
                b.as_data_slice(),
                b.rows(),
                out_cols,
                &mut out_buf,
            ) {
                return FieldMatrix::<F>::from_raw_parts(
                    self.rows,
                    out_cols,
                    FieldVec::from(out_buf),
                );
            }
        }

        // Per-row accumulator in `F::Wide`: each output cell is reduced once
        // per row, or once per chunk of `kmax` non-zeros when the row has
        // more.
        let kmax = F::max_unreduced_additions();
        let zero_wide = zero.to_wide();
        let mut wide_row: Vec<F::Wide> = vec![zero_wide.clone(); out_cols];
        let mut out_data: Vec<F> = Vec::with_capacity(self.rows * out_cols);

        for r in 0..self.rows {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            let nnz_r = end - start;

            if nnz_r == 0 {
                for _ in 0..out_cols {
                    out_data.push(zero.clone());
                }
                continue;
            }

            if kmax == usize::MAX || nnz_r <= kmax {
                for w in wide_row.iter_mut() {
                    *w = zero_wide.clone();
                }
                for k_off in start..end {
                    let k = self.col_idx[k_off];
                    let a_rk = &self.values[k_off];
                    let b_row_k = b.row(k);
                    for (w, bkj) in wide_row.iter_mut().zip(b_row_k.iter()) {
                        *w += a_rk.mul_product_sum_wide(bkj);
                    }
                }
                for w in wide_row.iter() {
                    out_data.push(F::reduce_product_sum_wide(w));
                }
            } else {
                let mut row_out: Vec<F> = vec![zero.clone(); out_cols];
                let mut offset = start;
                while offset < end {
                    let chunk_end = (offset + kmax).min(end);
                    for w in wide_row.iter_mut() {
                        *w = zero_wide.clone();
                    }
                    for k_off in offset..chunk_end {
                        let k = self.col_idx[k_off];
                        let a_rk = &self.values[k_off];
                        let b_row_k = b.row(k);
                        for (w, bkj) in wide_row.iter_mut().zip(b_row_k.iter()) {
                            *w += a_rk.mul_product_sum_wide(bkj);
                        }
                    }
                    for (out_cell, w) in row_out.iter_mut().zip(wide_row.iter()) {
                        *out_cell = out_cell.clone() + F::reduce_product_sum_wide(w);
                    }
                    offset = chunk_end;
                }
                out_data.extend(row_out);
            }
        }

        FieldMatrix::<F>::from_raw_parts(self.rows, out_cols, FieldVec::from(out_data))
    }

    /// Returns the dense transpose of this matrix as an owned
    /// [`FieldMatrix<F>`], the `Owned` type the [`MatrixLike<F>`] contract
    /// fixes for both sparse layouts. [`SparseFieldMatrix::to_csc`] flips
    /// the layout instead.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) because the output is dense.
    pub fn transpose(&self) -> FieldMatrix<F> {
        self.to_dense().transpose()
    }

    /// Computes `C = A · B` as a sparse-times-sparse product, returning
    /// canonical CSR output (column indices sorted ascending within each
    /// row, no stored zeros, no duplicate `(row, col)` keys).
    ///
    /// # Panics
    ///
    /// Panics if `self.cols() != other.rows()`.
    ///
    /// # Complexity
    ///
    /// `O(Σ_i Σ_{k ∈ row_i(A)} nnz(row_k(B)))` field operations, plus a
    /// sort of each output row's column indices and one `O(B.cols)` scatter
    /// buffer.
    pub fn matmul(&self, other: &Self) -> Self {
        assert_eq!(
            self.cols, other.rows,
            "SparseFieldMatrix::matmul: A.cols ({}) != B.rows ({})",
            self.cols, other.rows
        );
        let out_rows = self.rows;
        let out_cols = other.cols;

        let mut row_ptr = Vec::with_capacity(out_rows + 1);
        let mut col_idx: Vec<usize> = Vec::new();
        let mut values: Vec<F> = Vec::new();
        row_ptr.push(0);

        if out_rows == 0 || out_cols == 0 {
            row_ptr.resize(out_rows + 1, 0);
            return Self {
                rows: out_rows,
                cols: out_cols,
                row_ptr,
                col_idx,
                values,
            };
        }

        // Scatter buffer (`Option<F>`-style) and a touched-column list.
        // `marker[c]` records the row-index for which `accum[c]` holds a
        // partial sum, avoiding clears between rows: any stale value is
        // ignored unless `marker[c] == r + 1` (using `r + 1` so the
        // sentinel `0` never collides with row 0).
        let mut accum: Vec<Option<F>> = (0..out_cols).map(|_| None).collect();
        let mut marker: Vec<usize> = vec![0usize; out_cols];
        let mut touched: Vec<usize> = Vec::new();

        for r in 0..out_rows {
            let r_tag = r + 1;
            let a_start = self.row_ptr[r];
            let a_end = self.row_ptr[r + 1];
            touched.clear();

            for ka in a_start..a_end {
                let k = self.col_idx[ka];
                let a_rk = &self.values[ka];

                let b_start = other.row_ptr[k];
                let b_end = other.row_ptr[k + 1];
                for kb in b_start..b_end {
                    let c = other.col_idx[kb];
                    let prod = a_rk.clone() * other.values[kb].clone();
                    if marker[c] == r_tag {
                        // Existing partial sum; fold via `Option::take` to
                        // sidestep `+=` requiring a pre-existing zero on
                        // runtime-context fields.
                        let prev = accum[c].take().expect(
                            "SparseFieldMatrix::matmul: marker set without accumulator value",
                        );
                        accum[c] = Some(prev + prod);
                    } else {
                        marker[c] = r_tag;
                        accum[c] = Some(prod);
                        touched.push(c);
                    }
                }
            }

            touched.sort_unstable();
            for &c in &touched {
                if let Some(v) = accum[c].take() {
                    if !v.is_zero() {
                        col_idx.push(c);
                        values.push(v);
                    }
                }
            }
            row_ptr.push(values.len());
        }

        Self {
            rows: out_rows,
            cols: out_cols,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Computes the reduced row-echelon form via sparse Gauss–Jordan
    /// elimination, returning a new sparse matrix in canonical CSR form
    /// (column indices sorted ascending within each row, no stored zeros).
    ///
    /// The pivot column set is the leftmost linearly-independent subset
    /// of `self`'s columns, every pivot entry is `F::one()`, every
    /// non-pivot entry in a pivot column is `F::zero()`, and rows are
    /// ordered by pivot column.
    ///
    /// # Algorithm
    ///
    /// Sparse Gauss–Jordan. Pivot columns are taken in ascending order, and
    /// within a pivot column `pc` the unused row with minimum `row_nnz` is
    /// the pivot. Every candidate row has `pc` as its leading column, so
    /// `col_nnz[pc]` is the same for all of them and the Markowitz product
    /// `(row_nnz - 1) * (col_nnz - 1)` reduces to minimising `row_nnz`, the
    /// pivot priority of `GaussDomain::NoReordering` in
    /// `@/citation/LinBox2025`.
    ///
    /// # Panics
    ///
    /// Does not panic on any valid input. Zero-dimension shapes
    /// (`0 × n` or `m × 0`) return an empty canonical CSR immediately.
    ///
    /// # Complexity
    ///
    /// Worst-case `O(rows · cols · min(rows, cols))` field operations.
    pub fn rref(&self) -> Self {
        let m = self.rows;
        let n = self.cols;

        if m == 0 || n == 0 {
            return Self {
                rows: m,
                cols: n,
                row_ptr: vec![0; m + 1],
                col_idx: Vec::new(),
                values: Vec::new(),
            };
        }

        // Working copy: each row as a sorted `Vec<(usize, F)>` of non-zeros.
        let mut rows: Vec<Vec<(usize, F)>> = (0..m)
            .map(|r| {
                let s = self.row_ptr[r];
                let e = self.row_ptr[r + 1];
                self.col_idx[s..e]
                    .iter()
                    .copied()
                    .zip(self.values[s..e].iter().cloned())
                    .collect()
            })
            .collect();

        fn find_at<G: FiniteField>(row: &[(usize, G)], col: usize) -> Result<usize, usize> {
            row.binary_search_by_key(&col, |&(c, _)| c)
        }

        // `factor` must be non-zero, so no entry becomes zero.
        fn scale_row<G: FiniteField>(row: &mut [(usize, G)], factor: &G) {
            for (_, v) in row.iter_mut() {
                let new_v = v.clone() * factor.clone();
                *v = new_v;
            }
        }

        // target ← target − factor · source as a merge of two sorted
        // `(col, val)` lists; zeros are dropped.
        fn axpy<G: FiniteField>(target: &mut Vec<(usize, G)>, source: &[(usize, G)], factor: &G) {
            let mut merged: Vec<(usize, G)> = Vec::with_capacity(target.len() + source.len());
            let mut ti = 0usize;
            let mut si = 0usize;
            while ti < target.len() && si < source.len() {
                let tc = target[ti].0;
                let sc = source[si].0;
                if tc < sc {
                    merged.push(target[ti].clone());
                    ti += 1;
                } else if tc > sc {
                    let neg = -(factor.clone() * source[si].1.clone());
                    if !neg.is_zero() {
                        merged.push((sc, neg));
                    }
                    si += 1;
                } else {
                    let v = target[ti].1.clone() - factor.clone() * source[si].1.clone();
                    if !v.is_zero() {
                        merged.push((tc, v));
                    }
                    ti += 1;
                    si += 1;
                }
            }
            while ti < target.len() {
                merged.push(target[ti].clone());
                ti += 1;
            }
            while si < source.len() {
                let neg = -(factor.clone() * source[si].1.clone());
                if !neg.is_zero() {
                    merged.push((source[si].0, neg));
                }
                si += 1;
            }
            *target = merged;
        }

        let mut row_nnz: Vec<usize> = rows.iter().map(|r| r.len()).collect();

        let mut row_used = vec![false; m];
        // `(original_row, pivot_col)` in pick order.
        let mut pivot_order: Vec<(usize, usize)> = Vec::new();

        for _ in 0..m.min(n) {
            // The smallest column that leads some unused row, then the
            // unused row with minimum `row_nnz` among those it leads.
            let mut pc: usize = usize::MAX;
            for i in 0..m {
                if row_used[i] {
                    continue;
                }
                if rows[i].is_empty() {
                    continue;
                }
                let c = rows[i][0].0;
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
                if rows[i].first().map(|(c, _)| *c) != Some(pc) {
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

            let pos = find_at(&rows[pi], pc).expect("pivot was just verified to exist");
            let pivot_val = rows[pi][pos].1.clone();
            if !pivot_val.is_one() {
                let inv = pivot_val
                    .inv()
                    .expect("SparseFieldMatrix::rref: non-zero pivot must invert in a field");
                scale_row(&mut rows[pi], &inv);
            }
            row_used[pi] = true;
            pivot_order.push((pi, pc));

            // The snapshot avoids borrowing `rows[pi]` while the other rows
            // are mutated.
            let pivot_snapshot: Vec<(usize, F)> = rows[pi].clone();
            for k in 0..m {
                if k == pi {
                    continue;
                }
                let factor = match find_at(&rows[k], pc) {
                    Ok(p) => rows[k][p].1.clone(),
                    Err(_) => continue,
                };
                if factor.is_zero() {
                    continue;
                }
                axpy(&mut rows[k], &pivot_snapshot, &factor);
                row_nnz[k] = rows[k].len();
            }
        }

        // Canonical RREF row order: ascending pivot column.
        pivot_order.sort_by_key(|&(_orig, pc)| pc);

        let mut ordered: Vec<Vec<(usize, F)>> = Vec::with_capacity(m);
        for &(orig, _) in &pivot_order {
            ordered.push(std::mem::take(&mut rows[orig]));
        }
        // Pad with empty rows for rows that never became pivots.
        while ordered.len() < m {
            ordered.push(Vec::new());
        }

        let mut row_ptr = Vec::with_capacity(m + 1);
        let mut col_idx: Vec<usize> = Vec::new();
        let mut values: Vec<F> = Vec::new();
        row_ptr.push(0);
        for row in ordered {
            for (c, v) in row {
                col_idx.push(c);
                values.push(v);
            }
            row_ptr.push(values.len());
        }

        Self {
            rows: m,
            cols: n,
            row_ptr,
            col_idx,
            values,
        }
    }
}

impl<F: FiniteField> SparseFieldMatrixCsc<F> {
    /// Creates a structurally empty CSC matrix.
    ///
    /// # Complexity
    ///
    /// O(cols).
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            col_ptr: vec![0; cols + 1],
            row_idx: Vec::new(),
            values: Vec::new(),
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

    /// Returns `(rows, cols)`.
    #[inline]
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Number of stored non-zero entries.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Returns the value stored at `(row, col)`, or `F::zero()` if absent.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()` or `col >= self.cols()`. Also panics
    /// on pure runtime-context fields with no zero witness when the queried
    /// cell is structurally zero — use [`ConstField`] if that matters.
    ///
    /// # Complexity
    ///
    /// O(log k) where k is the per-column non-zero count.
    pub fn get(&self, row: usize, col: usize) -> F {
        assert!(
            row < self.rows,
            "SparseFieldMatrixCsc::get: row {row} out of bounds (rows={})",
            self.rows
        );
        assert!(
            col < self.cols,
            "SparseFieldMatrixCsc::get: col {col} out of bounds (cols={})",
            self.cols
        );
        let start = self.col_ptr[col];
        let end = self.col_ptr[col + 1];
        let slice = &self.row_idx[start..end];
        match slice.binary_search(&row) {
            Ok(off) => self.values[start + off].clone(),
            Err(_) => zero_witness(&self.values),
        }
    }

    /// Iterates over the non-zero entries of column `col` as `(row, &value)`
    /// pairs in ascending row order.
    ///
    /// # Panics
    ///
    /// Panics if `col >= self.cols()`.
    ///
    /// # Complexity
    ///
    /// O(nnz_in_col).
    pub fn col_iter(&self, col: usize) -> impl ExactSizeIterator<Item = (usize, &F)> + '_ {
        assert!(
            col < self.cols,
            "SparseFieldMatrixCsc::col_iter: col {col} out of bounds (cols={})",
            self.cols
        );
        let start = self.col_ptr[col];
        let end = self.col_ptr[col + 1];
        self.row_idx[start..end]
            .iter()
            .copied()
            .zip(self.values[start..end].iter())
    }

    /// Converts to CSR with the same shape.
    ///
    /// # Complexity
    ///
    /// O(nnz + rows + cols).
    pub fn to_csr(&self) -> SparseFieldMatrix<F> {
        let nnz = self.values.len();
        let mut counts = vec![0usize; self.rows];
        for &r in &self.row_idx {
            counts[r] += 1;
        }
        let mut row_ptr = Vec::with_capacity(self.rows + 1);
        row_ptr.push(0);
        for i in 0..self.rows {
            row_ptr.push(row_ptr[i] + counts[i]);
        }
        let mut next = row_ptr.clone();
        let mut col_idx = vec![0usize; nnz];
        let mut values: Vec<F> = if nnz == 0 {
            Vec::new()
        } else {
            (0..nnz).map(|_| self.values[0].clone()).collect()
        };
        // Column-major scatter: columns are already in ascending order, so
        // within each row the emitted column indices come out sorted.
        for c in 0..self.cols {
            let start = self.col_ptr[c];
            let end = self.col_ptr[c + 1];
            for k in start..end {
                let r = self.row_idx[k];
                let pos = next[r];
                col_idx[pos] = c;
                values[pos] = self.values[k].clone();
                next[r] += 1;
            }
        }
        SparseFieldMatrix {
            rows: self.rows,
            cols: self.cols,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Materialises the dense counterpart.
    ///
    /// # Complexity
    ///
    /// O(rows · cols).
    pub fn to_dense(&self) -> FieldMatrix<F> {
        self.to_csr().to_dense()
    }

    /// Returns `(col_ptr, row_idx, values)`.
    #[inline]
    pub fn as_raw_parts(&self) -> (&[usize], &[usize], &[F]) {
        (&self.col_ptr, &self.row_idx, &self.values)
    }
}

impl<F: FiniteField> MatrixLike<F> for SparseFieldMatrix<F> {
    type Owned = FieldMatrix<F>;

    #[inline]
    fn rows(&self) -> usize {
        SparseFieldMatrix::rows(self)
    }

    #[inline]
    fn cols(&self) -> usize {
        SparseFieldMatrix::cols(self)
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> F {
        SparseFieldMatrix::get(self, row, col)
    }

    #[inline]
    fn transpose(&self) -> Self::Owned {
        SparseFieldMatrix::transpose(self)
    }
}

impl<F: FiniteField> MatrixLike<F> for SparseFieldMatrixCsc<F> {
    type Owned = FieldMatrix<F>;

    #[inline]
    fn rows(&self) -> usize {
        SparseFieldMatrixCsc::rows(self)
    }

    #[inline]
    fn cols(&self) -> usize {
        SparseFieldMatrixCsc::cols(self)
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> F {
        SparseFieldMatrixCsc::get(self, row, col)
    }

    #[inline]
    fn transpose(&self) -> Self::Owned {
        self.to_dense().transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::FieldMatrix;
    use crate::field::FieldVec;
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;

    type F7 = Fp<7>;
    type F65521 = Fp<65521>;
    const M31: u64 = (1u64 << 31) - 1;

    // GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
    struct Gf2m8AesCfg;
    impl Gf2mWideConfig<1> for Gf2m8AesCfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "SparseTestsGf2m8AesCfg";
    }
    type G8 = Gf2mWide<1, Gf2m8AesCfg>;

    use crate::field::test_random_matrix::{
        dense_random_fp_sparse, direct_rref_oracle_fp as direct_rref_reference_fp_shared,
    };

    fn dense_random_fp<const P: u64>(
        rows: usize,
        cols: usize,
        density: f64,
        seed: u64,
    ) -> FieldMatrix<Fp<P>> {
        dense_random_fp_sparse::<P>(rows, cols, density, seed)
    }

    fn dense_random_g8(rows: usize, cols: usize, density: f64, seed: u64) -> FieldMatrix<G8> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<G8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                if rng.gen::<f64>() < density {
                    let w = (rng.gen::<u64>() & 0xFF).max(1);
                    m.set(r, c, G8::new([w]));
                }
            }
        }
        m
    }

    fn random_fieldvec_fp<const P: u64>(n: usize, seed: u64) -> FieldVec<Fp<P>> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        (0..n).map(|_| Fp::<P>::new(rng.gen::<u64>() % P)).collect()
    }

    fn random_fieldvec_g8(n: usize, seed: u64) -> FieldVec<G8> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        (0..n).map(|_| G8::new([rng.gen::<u64>() & 0xFF])).collect()
    }

    #[test]
    fn test_roundtrip_from_dense_to_dense_fp7() {
        let m = dense_random_fp::<7>(5, 7, 0.4, 0xAAA);
        let s = SparseFieldMatrix::from_dense(&m);
        assert_eq!(s.to_dense(), m);
    }

    #[test]
    fn test_roundtrip_from_dense_to_dense_fp65521() {
        let m = dense_random_fp::<65521>(4, 9, 0.3, 0xBBB);
        let s = SparseFieldMatrix::from_dense(&m);
        assert_eq!(s.to_dense(), m);
    }

    #[test]
    fn test_roundtrip_from_dense_to_dense_m31() {
        let m = dense_random_fp::<M31>(8, 8, 0.2, 0xCCC);
        let s = SparseFieldMatrix::from_dense(&m);
        assert_eq!(s.to_dense(), m);
    }

    #[test]
    fn test_roundtrip_from_dense_to_dense_g8() {
        let m = dense_random_g8(6, 10, 0.3, 0xDDD);
        let s = SparseFieldMatrix::from_dense(&m);
        assert_eq!(s.to_dense(), m);
    }

    #[test]
    fn test_csr_csc_roundtrip_fp7() {
        let m = dense_random_fp::<7>(6, 5, 0.5, 0xE1);
        let csr = SparseFieldMatrix::from_dense(&m);
        let csc = csr.to_csc();
        let csr_back = csc.to_csr();
        assert_eq!(csr, csr_back);
        assert_eq!(csc.to_dense(), m);
    }

    #[test]
    fn test_csr_csc_roundtrip_g8() {
        let m = dense_random_g8(7, 4, 0.4, 0xE2);
        let csr = SparseFieldMatrix::from_dense(&m);
        let csc = csr.to_csc();
        assert_eq!(csc.to_dense(), m);
        assert_eq!(csc.to_csr(), csr);
    }

    #[test]
    fn test_matvec_matches_dense_fp7() {
        let m = dense_random_fp::<7>(8, 11, 0.3, 0x11);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_fp::<7>(11, 0x22);
        assert_eq!(s.matvec(&x), m.matvec(&x));
    }

    #[test]
    fn test_matvec_matches_dense_fp65521() {
        let m = dense_random_fp::<65521>(6, 7, 0.25, 0x33);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_fp::<65521>(7, 0x44);
        assert_eq!(s.matvec(&x), m.matvec(&x));
    }

    #[test]
    fn test_matvec_matches_dense_m31() {
        let m = dense_random_fp::<M31>(9, 13, 0.2, 0x55);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_fp::<M31>(13, 0x66);
        assert_eq!(s.matvec(&x), m.matvec(&x));
    }

    #[test]
    fn test_matvec_matches_dense_g8() {
        let m = dense_random_g8(5, 12, 0.35, 0x77);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_g8(12, 0x88);
        assert_eq!(s.matvec(&x), m.matvec(&x));
    }

    #[test]
    fn test_matvec_transpose_matches_dense_fp7() {
        let m = dense_random_fp::<7>(6, 9, 0.4, 0x99);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_fp::<7>(6, 0xAA);
        assert_eq!(s.matvec_transpose(&x), m.matvec_transpose(&x));
    }

    #[test]
    fn test_matvec_transpose_matches_dense_fp65521() {
        let m = dense_random_fp::<65521>(5, 8, 0.3, 0xBB);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_fp::<65521>(5, 0xCC);
        assert_eq!(s.matvec_transpose(&x), m.matvec_transpose(&x));
    }

    #[test]
    fn test_matvec_transpose_matches_dense_m31() {
        let m = dense_random_fp::<M31>(7, 10, 0.25, 0xDD);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_fp::<M31>(7, 0xEE);
        assert_eq!(s.matvec_transpose(&x), m.matvec_transpose(&x));
    }

    #[test]
    fn test_matvec_transpose_matches_dense_g8() {
        let m = dense_random_g8(4, 11, 0.35, 0xFF);
        let s = SparseFieldMatrix::from_dense(&m);
        let x = random_fieldvec_g8(4, 0x101);
        assert_eq!(s.matvec_transpose(&x), m.matvec_transpose(&x));
    }

    #[test]
    fn test_matmat_matches_dense_fp7() {
        let a = dense_random_fp::<7>(5, 8, 0.3, 0x201);
        let b = dense_random_fp::<7>(8, 4, 0.5, 0x202);
        let s = SparseFieldMatrix::from_dense(&a);
        let got = s.matmat(&b);
        let expected: FieldMatrix<F7> = (&a * &b).into();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_matmat_matches_dense_fp65521() {
        let a = dense_random_fp::<65521>(4, 6, 0.25, 0x203);
        let b = dense_random_fp::<65521>(6, 3, 0.6, 0x204);
        let s = SparseFieldMatrix::from_dense(&a);
        let expected: FieldMatrix<Fp<65521>> = (&a * &b).into();
        assert_eq!(s.matmat(&b), expected);
    }

    #[test]
    fn test_matmat_matches_dense_m31() {
        let a = dense_random_fp::<M31>(6, 7, 0.2, 0x205);
        let b = dense_random_fp::<M31>(7, 5, 0.4, 0x206);
        let s = SparseFieldMatrix::from_dense(&a);
        let expected: FieldMatrix<Fp<M31>> = (&a * &b).into();
        assert_eq!(s.matmat(&b), expected);
    }

    #[test]
    fn test_matmat_matches_dense_g8() {
        let a = dense_random_g8(4, 6, 0.3, 0x207);
        let b = dense_random_g8(6, 5, 0.4, 0x208);
        let s = SparseFieldMatrix::from_dense(&a);
        let expected: FieldMatrix<_> = (&a * &b).into();
        assert_eq!(s.matmat(&b), expected);
    }

    #[test]
    fn test_from_triplets_sums_duplicates() {
        let s = SparseFieldMatrix::<F7>::from_triplets(
            2,
            2,
            [
                (0usize, 0usize, F7::new(2)),
                (0, 0, F7::new(3)),
                (1, 1, F7::new(1)),
            ],
        );
        assert_eq!(s.get(0, 0), F7::new(5));
        assert_eq!(s.get(1, 1), F7::new(1));
        assert_eq!(s.nnz(), 2);
    }

    #[test]
    fn test_from_triplets_drops_zero_sum() {
        // 3 + 4 ≡ 0 (mod 7) — the (0, 0) cell must disappear.
        let s = SparseFieldMatrix::<F7>::from_triplets(
            2,
            2,
            [(0usize, 0usize, F7::new(3)), (0, 0, F7::new(4))],
        );
        assert_eq!(s.nnz(), 0);
    }

    #[test]
    fn test_from_triplets_drops_explicit_zeros() {
        let s = SparseFieldMatrix::<F7>::from_triplets(
            2,
            2,
            [(0usize, 0usize, F7::new(0)), (1, 1, F7::new(2))],
        );
        assert_eq!(s.nnz(), 1);
    }

    #[test]
    fn test_from_triplets_sorts_within_row() {
        let s = SparseFieldMatrix::<F7>::from_triplets(
            1,
            4,
            [
                (0usize, 3usize, F7::new(1)),
                (0, 0, F7::new(2)),
                (0, 2, F7::new(3)),
            ],
        );
        let (_rp, ci, _vs) = s.as_raw_parts();
        assert_eq!(ci, &[0, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_from_triplets_oob_row_panics() {
        let _ = SparseFieldMatrix::<F7>::from_triplets(2, 2, [(5usize, 0usize, F7::new(1))]);
    }

    #[test]
    fn test_identity_matvec_fp7() {
        let id = SparseFieldMatrix::<F7>::identity(5);
        let x = random_fieldvec_fp::<7>(5, 0x301);
        assert_eq!(id.matvec(&x), x);
    }

    #[test]
    fn test_empty_0x0() {
        let s: SparseFieldMatrix<F7> = SparseFieldMatrix::zeros(0, 0);
        assert_eq!(s.shape(), (0, 0));
        assert_eq!(s.nnz(), 0);
        let d = s.to_dense();
        assert_eq!(d.shape(), (0, 0));
    }

    #[test]
    fn test_all_zero_mxn() {
        let s: SparseFieldMatrix<F7> = SparseFieldMatrix::zeros(4, 7);
        assert_eq!(s.shape(), (4, 7));
        assert_eq!(s.nnz(), 0);
        let d = s.to_dense();
        assert_eq!(d.shape(), (4, 7));
        for r in 0..4 {
            for c in 0..7 {
                assert_eq!(d.get(r, c), F7::new(0));
            }
        }
        let x = random_fieldvec_fp::<7>(7, 0x401);
        let y = s.matvec(&x);
        assert_eq!(y.len(), 4);
        for i in 0..4 {
            assert!(y[i].is_zero());
        }
    }

    #[test]
    fn test_single_nonzero() {
        let s = SparseFieldMatrix::<F7>::from_triplets(3, 3, [(1usize, 2usize, F7::new(4))]);
        assert_eq!(s.nnz(), 1);
        assert_eq!(s.get(1, 2), F7::new(4));
        assert_eq!(s.get(0, 0), F7::new(0));
        let m = s.to_dense();
        assert_eq!(m.get(1, 2), F7::new(4));
        let s2 = SparseFieldMatrix::from_dense(&m);
        assert_eq!(s2, s);
    }

    #[test]
    fn test_diagonal_only() {
        let mut m = FieldMatrix::<F7>::zeros(5, 5);
        for i in 0..5 {
            m.set(i, i, F7::new((i as u64 + 1) % 7));
        }
        let s = SparseFieldMatrix::from_dense(&m);
        assert_eq!(s.nnz(), m.diag().iter().filter(|v| !v.is_zero()).count());
        assert_eq!(s.to_dense(), m);
    }

    #[test]
    fn test_fully_dense_stored_as_sparse() {
        let mut m = FieldMatrix::<F7>::zeros(3, 4);
        for r in 0..3 {
            for c in 0..4 {
                m.set(r, c, F7::new(((r * 7 + c + 1) as u64) % 7 + 1));
            }
        }
        let s = SparseFieldMatrix::from_dense(&m);
        assert_eq!(s.nnz(), 3 * 4);
        assert_eq!(s.to_dense(), m);
        let x = random_fieldvec_fp::<7>(4, 0x501);
        assert_eq!(s.matvec(&x), m.matvec(&x));
    }

    #[test]
    fn test_very_wide_matrix() {
        let n = 10_000usize;
        let s = SparseFieldMatrix::<F7>::from_triplets(
            1,
            n,
            [
                (0usize, 0usize, F7::new(1)),
                (0, n / 2, F7::new(2)),
                (0, n - 1, F7::new(3)),
            ],
        );
        assert_eq!(s.shape(), (1, n));
        let m = s.to_dense();
        assert_eq!(m.shape(), (1, n));
        let x = random_fieldvec_fp::<7>(n, 0x601);
        assert_eq!(s.matvec(&x), m.matvec(&x));
    }

    #[test]
    fn test_very_tall_matrix() {
        let rows = 1000usize;
        let mut triplets = Vec::new();
        for r in 0..rows {
            triplets.push((r, r % 5, F7::new(((r as u64) % 6) + 1)));
        }
        let s = SparseFieldMatrix::<F7>::from_triplets(rows, 5, triplets);
        assert_eq!(s.shape(), (rows, 5));
        let m = s.to_dense();
        let x = random_fieldvec_fp::<7>(5, 0x701);
        assert_eq!(s.matvec(&x), m.matvec(&x));
    }

    #[test]
    fn test_transpose_matches_dense_transpose() {
        let m = dense_random_fp::<7>(4, 6, 0.4, 0x801);
        let s = SparseFieldMatrix::from_dense(&m);
        let t = s.transpose();
        assert_eq!(t, m.transpose());
    }

    #[test]
    fn test_matrixlike_transpose_csc_matches_dense() {
        let m = dense_random_fp::<7>(4, 5, 0.4, 0x802);
        let csc = SparseFieldMatrix::from_dense(&m).to_csc();
        let t = <SparseFieldMatrixCsc<F7> as MatrixLike<F7>>::transpose(&csc);
        assert_eq!(t, m.transpose());
    }

    #[test]
    fn test_matrixlike_csr_basic() {
        let m = dense_random_fp::<7>(3, 4, 0.4, 0x901);
        let s = SparseFieldMatrix::from_dense(&m);
        assert_eq!(<SparseFieldMatrix<F7> as MatrixLike<F7>>::rows(&s), 3);
        assert_eq!(<SparseFieldMatrix<F7> as MatrixLike<F7>>::cols(&s), 4);
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(
                    <SparseFieldMatrix<F7> as MatrixLike<F7>>::get(&s, r, c),
                    m.get(r, c)
                );
            }
        }
    }

    #[test]
    fn test_matrixlike_csc_basic() {
        let m = dense_random_fp::<65521>(3, 4, 0.4, 0x902);
        let s = SparseFieldMatrix::from_dense(&m).to_csc();
        assert_eq!(
            <SparseFieldMatrixCsc<F65521> as MatrixLike<F65521>>::rows(&s),
            3
        );
        assert_eq!(
            <SparseFieldMatrixCsc<F65521> as MatrixLike<F65521>>::cols(&s),
            4
        );
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(
                    <SparseFieldMatrixCsc<F65521> as MatrixLike<F65521>>::get(&s, r, c),
                    m.get(r, c)
                );
            }
        }
    }

    #[test]
    fn test_field_matrix_to_sparse_returns_csr() {
        let m = dense_random_fp::<7>(4, 5, 0.3, 0xA01);
        let s = m.to_sparse();
        assert_eq!(s.shape(), m.shape());
        assert_eq!(s.to_dense(), m);
    }

    fn sparse_from_dense_g8(m: &FieldMatrix<G8>) -> SparseFieldMatrix<G8> {
        SparseFieldMatrix::from_dense(m)
    }

    #[test]
    fn test_matmul_identity_left_fp7() {
        let id = SparseFieldMatrix::<F7>::identity(4);
        let m = dense_random_fp::<7>(4, 5, 0.4, 0x1001);
        let a = SparseFieldMatrix::from_dense(&m);
        assert_eq!(id.matmul(&a).to_dense(), m);
    }

    #[test]
    fn test_matmul_identity_right_fp7() {
        let m = dense_random_fp::<7>(5, 4, 0.4, 0x1002);
        let a = SparseFieldMatrix::from_dense(&m);
        let id = SparseFieldMatrix::<F7>::identity(4);
        assert_eq!(a.matmul(&id).to_dense(), m);
    }

    #[test]
    fn test_matmul_matches_dense_fp7() {
        let a_dense = dense_random_fp::<7>(5, 7, 0.3, 0x1100);
        let b_dense = dense_random_fp::<7>(7, 4, 0.4, 0x1101);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let b = SparseFieldMatrix::from_dense(&b_dense);
        let expected: FieldMatrix<F7> = (&a_dense * &b_dense).into();
        assert_eq!(a.matmul(&b).to_dense(), expected);
    }

    #[test]
    fn test_matmul_matches_dense_fp65521() {
        let a_dense = dense_random_fp::<65521>(6, 5, 0.25, 0x1110);
        let b_dense = dense_random_fp::<65521>(5, 7, 0.35, 0x1111);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let b = SparseFieldMatrix::from_dense(&b_dense);
        let expected: FieldMatrix<F65521> = (&a_dense * &b_dense).into();
        assert_eq!(a.matmul(&b).to_dense(), expected);
    }

    #[test]
    fn test_matmul_matches_dense_m31() {
        let a_dense = dense_random_fp::<M31>(7, 6, 0.2, 0x1120);
        let b_dense = dense_random_fp::<M31>(6, 8, 0.3, 0x1121);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let b = SparseFieldMatrix::from_dense(&b_dense);
        let expected: FieldMatrix<Fp<M31>> = (&a_dense * &b_dense).into();
        assert_eq!(a.matmul(&b).to_dense(), expected);
    }

    #[test]
    fn test_matmul_matches_dense_g8() {
        let a_dense = dense_random_g8(4, 6, 0.3, 0x1130);
        let b_dense = dense_random_g8(6, 5, 0.4, 0x1131);
        let a = sparse_from_dense_g8(&a_dense);
        let b = sparse_from_dense_g8(&b_dense);
        let expected: FieldMatrix<G8> = (&a_dense * &b_dense).into();
        assert_eq!(a.matmul(&b).to_dense(), expected);
    }

    #[test]
    fn test_matmul_empty_inner() {
        let a = SparseFieldMatrix::<F7>::zeros(3, 0);
        let b = SparseFieldMatrix::<F7>::zeros(0, 4);
        let c = a.matmul(&b);
        assert_eq!(c.shape(), (3, 4));
        assert_eq!(c.nnz(), 0);
    }

    #[test]
    fn test_matmul_empty_rows() {
        let a = SparseFieldMatrix::<F7>::zeros(0, 3);
        let b_dense = dense_random_fp::<7>(3, 5, 0.4, 0x1201);
        let b = SparseFieldMatrix::from_dense(&b_dense);
        let c = a.matmul(&b);
        assert_eq!(c.shape(), (0, 5));
        assert_eq!(c.nnz(), 0);
    }

    #[test]
    fn test_matmul_empty_cols() {
        let a_dense = dense_random_fp::<7>(4, 3, 0.4, 0x1202);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let b = SparseFieldMatrix::<F7>::zeros(3, 0);
        let c = a.matmul(&b);
        assert_eq!(c.shape(), (4, 0));
        assert_eq!(c.nnz(), 0);
    }

    #[test]
    fn test_matmul_zero_drops_in_fp7() {
        // a = [[1, 1]], b = [[3], [4]]: c = [[3 + 4]] ≡ [[0]] (mod 7).
        let a = SparseFieldMatrix::<F7>::from_triplets(
            1,
            2,
            [(0usize, 0usize, F7::new(1)), (0, 1, F7::new(1))],
        );
        let b = SparseFieldMatrix::<F7>::from_triplets(
            2,
            1,
            [(0usize, 0usize, F7::new(3)), (1, 0, F7::new(4))],
        );
        let c = a.matmul(&b);
        assert_eq!(c.shape(), (1, 1));
        assert_eq!(c.nnz(), 0);
        assert_eq!(c.get(0, 0), F7::new(0));
    }

    #[test]
    fn test_matmul_canonical_csr_invariants() {
        let a_dense = dense_random_fp::<7>(6, 5, 0.4, 0x1301);
        let b_dense = dense_random_fp::<7>(5, 8, 0.5, 0x1302);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let b = SparseFieldMatrix::from_dense(&b_dense);
        let c = a.matmul(&b);
        let (rp, ci, vs) = c.as_raw_parts();
        assert_eq!(rp.len(), c.rows() + 1);
        for r in 0..c.rows() {
            let s = rp[r];
            let e = rp[r + 1];
            for w in s..e.saturating_sub(1) {
                assert!(ci[w] < ci[w + 1], "col_idx not strictly ascending");
            }
            for v in vs.iter().take(e).skip(s) {
                assert!(!v.is_zero(), "stored value must be non-zero");
            }
        }
    }

    #[test]
    #[should_panic(expected = "A.cols")]
    fn test_matmul_dim_mismatch_panics() {
        let a = SparseFieldMatrix::<F7>::zeros(2, 3);
        let b = SparseFieldMatrix::<F7>::zeros(4, 2);
        let _ = a.matmul(&b);
    }

    #[test]
    fn test_rref_identity_g8() {
        let id = SparseFieldMatrix::<G8>::identity(5);
        let r = id.rref();
        assert_eq!(r, id);
    }

    #[test]
    fn test_rref_matches_dense_g8() {
        let a_dense = dense_random_g8(5, 7, 0.4, 0x2001);
        let a = sparse_from_dense_g8(&a_dense);
        let got = a.rref();
        let (_x, expected) = a_dense.rref();
        assert_eq!(got.to_dense(), expected);
    }

    #[test]
    fn test_rref_matches_dense_g8_square() {
        let a_dense = dense_random_g8(6, 6, 0.35, 0x2002);
        let a = sparse_from_dense_g8(&a_dense);
        let got = a.rref();
        let (_x, expected) = a_dense.rref();
        assert_eq!(got.to_dense(), expected);
    }

    #[test]
    fn test_rref_matches_dense_g8_tall() {
        let a_dense = dense_random_g8(8, 4, 0.3, 0x2003);
        let a = sparse_from_dense_g8(&a_dense);
        let got = a.rref();
        let (_x, expected) = a_dense.rref();
        assert_eq!(got.to_dense(), expected);
    }

    #[test]
    fn test_rref_matches_dense_fp7() {
        let a_dense = dense_random_fp::<7>(5, 7, 0.4, 0x2010);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let got = a.rref();
        let (_x, expected) = a_dense.rref();
        assert_eq!(got.to_dense(), expected);
    }

    #[test]
    fn test_rref_matches_dense_fp65521() {
        let a_dense = dense_random_fp::<65521>(4, 6, 0.35, 0x2011);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let got = a.rref();
        let (_x, expected) = a_dense.rref();
        assert_eq!(got.to_dense(), expected);
    }

    struct Gf2m16TestCfg;
    impl Gf2mWideConfig<1> for Gf2m16TestCfg {
        const M: usize = 16;
        const MODULUS: [u64; 1] = [0x002D];
        const NAME: &'static str = "SparseTestsGf2m16Cfg";
    }
    type G16 = Gf2mWide<1, Gf2m16TestCfg>;

    fn dense_random_g16(rows: usize, cols: usize, density: f64, seed: u64) -> FieldMatrix<G16> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<G16>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                if rng.gen::<f64>() < density {
                    let w = (rng.gen::<u64>() & 0xFFFF).max(1);
                    m.set(r, c, G16::new([w]));
                }
            }
        }
        m
    }

    #[test]
    fn test_rref_matches_dense_g16() {
        let a_dense = dense_random_g16(4, 6, 0.35, 0x2020);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let got = a.rref();
        let (_x, expected) = a_dense.rref();
        assert_eq!(got.to_dense(), expected);
    }

    #[test]
    fn test_rref_empty_matrix() {
        let a: SparseFieldMatrix<F7> = SparseFieldMatrix::zeros(0, 0);
        let r = a.rref();
        assert_eq!(r.shape(), (0, 0));
        assert_eq!(r.nnz(), 0);
    }

    #[test]
    fn test_rref_zero_rows() {
        let a: SparseFieldMatrix<F7> = SparseFieldMatrix::zeros(4, 3);
        let r = a.rref();
        assert_eq!(r.shape(), (4, 3));
        assert_eq!(r.nnz(), 0);
    }

    #[test]
    fn test_rref_canonical_csr_invariants() {
        let a_dense = dense_random_fp::<7>(6, 6, 0.4, 0x2101);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let (rp, ci, vs) = r.as_raw_parts();
        assert_eq!(rp.len(), r.rows() + 1);
        for row in 0..r.rows() {
            let s = rp[row];
            let e = rp[row + 1];
            for w in s..e.saturating_sub(1) {
                assert!(ci[w] < ci[w + 1], "RREF col_idx not strictly ascending");
            }
            for v in vs.iter().take(e).skip(s) {
                assert!(!v.is_zero(), "RREF stored value must be non-zero");
            }
        }
    }

    #[test]
    fn test_rref_idempotent_g8() {
        let a_dense = dense_random_g8(5, 5, 0.4, 0x2200);
        let a = sparse_from_dense_g8(&a_dense);
        let r1 = a.rref();
        let r2 = r1.rref();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_rref_idempotent_fp7() {
        let a_dense = dense_random_fp::<7>(5, 6, 0.45, 0x2201);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r1 = a.rref();
        let r2 = r1.rref();
        assert_eq!(r1, r2);
    }

    fn direct_rref_reference_fp<const P: u64>(a: &FieldMatrix<Fp<P>>) -> FieldMatrix<Fp<P>> {
        direct_rref_reference_fp_shared::<P>(a)
    }

    /// Column-by-column Gauss–Jordan oracle over GF(2^8).
    #[cfg(test)]
    fn direct_rref_reference_g8(a: &FieldMatrix<G8>) -> FieldMatrix<G8> {
        let (m, n) = a.shape();
        let mut e = a.clone();
        let zero = G8::new([0]);
        let one = G8::new([1]);
        let mut next_pivot_row = 0usize;
        for col in 0..n {
            if next_pivot_row >= m {
                break;
            }
            let mut pivot_row: Option<usize> = None;
            for i in next_pivot_row..m {
                if e.get(i, col) != zero {
                    pivot_row = Some(i);
                    break;
                }
            }
            let Some(p) = pivot_row else {
                continue;
            };
            if p != next_pivot_row {
                for c in 0..n {
                    let tmp = e.get(next_pivot_row, c);
                    e.set(next_pivot_row, c, e.get(p, c));
                    e.set(p, c, tmp);
                }
            }
            let piv = e.get(next_pivot_row, col);
            if piv != one {
                let inv = piv.inv().unwrap();
                for c in 0..n {
                    let v = e.get(next_pivot_row, c) * inv;
                    e.set(next_pivot_row, c, v);
                }
            }
            for k in 0..m {
                if k == next_pivot_row {
                    continue;
                }
                let factor = e.get(k, col);
                if factor == zero {
                    continue;
                }
                for c in 0..n {
                    let v = e.get(k, c) - factor * e.get(next_pivot_row, c);
                    e.set(k, c, v);
                }
            }
            next_pivot_row += 1;
        }
        e
    }

    #[test]
    fn test_rref_markowitz_1x1_single_entry_fp7() {
        let a = SparseFieldMatrix::<F7>::from_triplets(1, 1, [(0usize, 0usize, F7::new(3))]);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a.to_dense());
        assert_eq!(r.to_dense(), expected);
    }

    #[test]
    fn test_rref_markowitz_tall_deficient_fp7() {
        let a_dense = dense_random_fp::<7>(8, 4, 0.3, 0x5CE1_BAE0);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    #[test]
    fn test_rref_markowitz_wide_fp7() {
        let a_dense = dense_random_fp::<7>(4, 12, 0.25, 0x5CE1_BAE1);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    /// Density `1/64`: one entry per row on average.
    #[test]
    fn test_rref_markowitz_word_boundary_n64_fp7() {
        let a_dense = dense_random_fp::<7>(64, 64, 1.0 / 64.0, 0x5CE1_BAE2);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    #[test]
    fn test_rref_markowitz_word_boundary_n65_fp65521() {
        let a_dense = dense_random_fp::<65521>(65, 65, 0.05, 0x5CE1_BAE3);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    #[test]
    fn test_rref_markowitz_sweep_fp7() {
        for seed in 0u64..32 {
            for &(rows, cols) in &[
                (0usize, 0usize),
                (1, 1),
                (3, 5),
                (5, 3),
                (8, 8),
                (15, 17),
                (24, 24),
            ] {
                for &density in &[0.0_f64, 0.05, 0.25, 0.5, 0.9] {
                    let a_dense = dense_random_fp::<7>(rows, cols, density, seed ^ 0xF1AB_CAFE);
                    let a = SparseFieldMatrix::from_dense(&a_dense);
                    let got = a.rref();
                    let expected = direct_rref_reference_fp(&a_dense);
                    assert_eq!(
                        got.to_dense(),
                        expected,
                        "Markowitz RREF != direct reference @ seed={seed} rows={rows} cols={cols} density={density}"
                    );
                    let got2 = got.rref();
                    assert_eq!(got, got2);
                }
            }
        }
    }

    #[test]
    fn test_rref_markowitz_sweep_fp65521() {
        for seed in 0u64..16 {
            for &(rows, cols) in &[(4usize, 4usize), (8, 8), (16, 16), (8, 20)] {
                for &density in &[0.05_f64, 0.3, 0.7] {
                    let a_dense = dense_random_fp::<65521>(rows, cols, density, seed ^ 0xCAFE_F1AB);
                    let a = SparseFieldMatrix::from_dense(&a_dense);
                    let got = a.rref();
                    let expected = direct_rref_reference_fp(&a_dense);
                    assert_eq!(
                        got.to_dense(),
                        expected,
                        "Markowitz RREF != direct reference @ seed={seed} rows={rows} cols={cols} density={density}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_rref_markowitz_sweep_g8() {
        for seed in 0u64..16 {
            for &(rows, cols) in &[(4usize, 4usize), (8, 8), (12, 16)] {
                for &density in &[0.1_f64, 0.4, 0.8] {
                    let a_dense = dense_random_g8(rows, cols, density, seed ^ 0xBEEF_5CE1);
                    let a = sparse_from_dense_g8(&a_dense);
                    let got = a.rref();
                    let expected = direct_rref_reference_g8(&a_dense);
                    assert_eq!(
                        got.to_dense(),
                        expected,
                        "Markowitz RREF != direct reference @ seed={seed} rows={rows} cols={cols} density={density}"
                    );
                }
            }
        }
    }
}
