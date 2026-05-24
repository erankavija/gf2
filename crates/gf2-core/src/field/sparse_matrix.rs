//! Sparse matrix primitives over a generic [`FiniteField`].
//!
//! This module provides Compressed-Sparse-Row ([`SparseFieldMatrix<F>`]) and
//! Compressed-Sparse-Column ([`SparseFieldMatrixCsc<F>`]) storage with
//! conversions, element access, and matrix–vector / matrix–matrix products.
//! It is the field-generic counterpart of [`crate::sparse::SpBitMatrix`] and
//! owns the public sparse surface promised by issue `8a90882e` of epic
//! `bb85c68a`.
//!
//! # Layout
//!
//! Both types store three flat arrays, mirroring the usual Netlib convention:
//!
//! | Field | CSR ([`SparseFieldMatrix`]) | CSC ([`SparseFieldMatrixCsc`]) |
//! |-------|-----------------------------|--------------------------------|
//! | `ptr` | `row_ptr`, length `rows + 1` | `col_ptr`, length `cols + 1` |
//! | `idx` | column indices per row, sorted | row indices per col, sorted |
//! | `val` | non-zero values, aligned with `idx` | non-zero values, aligned with `idx` |
//!
//! All stored `val[i]` entries are guaranteed non-zero (the constructors drop
//! zeros and sum duplicate coordinates). Per-row (resp. per-col) column
//! (resp. row) indices are sorted ascending — enabling `get(r, c)` via
//! binary search and making `SpMV` output deterministic.
//!
//! # Naming parity with [`SpBitMatrix`](crate::sparse::SpBitMatrix)
//!
//! The GF(2) sparse type `SpBitMatrix` fixes the method vocabulary for all
//! sparse matrices in this crate. The table below records the pairs:
//!
//! | [`SpBitMatrix`](crate::sparse::SpBitMatrix) | [`SparseFieldMatrix`] | Notes |
//! |-----|-----|-----|
//! | `zeros(rows, cols)` | [`SparseFieldMatrix::zeros`] | Empty structure, same shape. |
//! | `identity(n)` | [`SparseFieldMatrix::identity`] | Diagonal of `F::one()` (requires [`ConstField`]). |
//! | `from_dense(&BitMatrix)` | [`SparseFieldMatrix::from_dense`] | Row-major scan, drops zeros. |
//! | `to_dense() -> BitMatrix` | [`SparseFieldMatrix::to_dense`] | Materialises a dense [`FieldMatrix`]. |
//! | `from_coo(rows, cols, &[(r, c)])` | [`SparseFieldMatrix::from_triplets`] | Over a field, duplicates *sum* (field) instead of XOR (GF(2)). |
//! | `from_coo_deduplicated(rows, cols, &[(r, c)])` | [`SparseFieldMatrix::from_triplets`] | Over GF(2), "dedup" means "keep one hit per coordinate"; over a general field the same contract falls out of [`SparseFieldMatrix::from_triplets`] when each coordinate is supplied at most once by the caller (the post-sum zero-drop subsumes the GF(2) cancellation). No separate entry point is needed on the field side. |
//! | `rows()` / `cols()` / `nnz()` | identical | Same contract. |
//! | `row_iter(row)` | [`SparseFieldMatrix::row_iter`] | Yields `(col, &value)`, CSR-native. |
//! | `col_iter(col)` | [`SparseFieldMatrixCsc::col_iter`] | CSR does not expose `col_iter` natively; convert first via [`SparseFieldMatrix::to_csc`]. The CSC variant yields `(row, &value)` pairs directly. |
//! | `transpose()` | [`SparseFieldMatrix::transpose`] | Field version materialises a dense transpose (see below). |
//! | `matvec(&BitVec)` | [`SparseFieldMatrix::matvec`] | Row-by-row allocation-free SpMV with delayed reduction over `F::Wide`. |
//! | — | [`SparseFieldMatrix::matvec_transpose`] | New: field SpMV^T, O(nnz) via CSR scatter (no CSC flip required). |
//! | — | [`SparseFieldMatrix::matmat`] | New: SpMM (sparse × dense → dense). |
//! | `save_image` (feature `visualization`) | — | N/A — not re-exposed on the field side (only the GF(2) crate ships the visualization path today). |
//! | [`SpBitMatrixDual`](crate::sparse::SpBitMatrixDual) | [`SparseFieldMatrixCsc`] | GF(2) stores both layouts in one handle; over a general field the two layouts are split into separate types and converted on demand. |
//! | `SpBitMatrixDual::{from_dense,from_coo,from_coo_deduplicated}` | `SparseFieldMatrix` + [`SparseFieldMatrix::to_csc`] | Construct the CSR half first, then flip to CSC if both views are needed. |
//! | `SpBitMatrixDual::{row_iter,col_iter}` | [`SparseFieldMatrix::row_iter`] / [`SparseFieldMatrixCsc::col_iter`] | Same contract, but sourced from the matching CSR/CSC layout rather than a fused pair. |
//! | `SpBitMatrixDual::{rows,cols,nnz}` | identical on either field variant | Trivial shape accessors. |
//! | `SpBitMatrixDual::matvec` | [`SparseFieldMatrix::matvec`] | Field side routes SpMV through CSR. |
//! | `SpBitMatrixDual::matvec_transpose` | [`SparseFieldMatrix::matvec_transpose`] | CSR-based scatter; no CSC flip required. |
//!
//! **Divergences.** Two behavioural differences from the GF(2) vocabulary:
//!
//! 1. `SpBitMatrix::from_coo` treats duplicate coordinates as XOR-cancelling
//!    because that matches GF(2) arithmetic; over a general field we sum them
//!    instead, which is the correct "reconstruct the matrix from its triplet
//!    expansion" semantics. Callers that want to suppress duplicates entirely
//!    can deduplicate client-side or rely on the post-sum zero drop — both
//!    routes reproduce the `from_coo_deduplicated` shape without a separate
//!    entry point.
//! 2. `SpBitMatrix::col_iter` is exposed directly on the CSR type (the GF(2)
//!    implementation scans the CSR indices with a filter), whereas the field
//!    side keeps the two layouts separate: `col_iter` lives on
//!    [`SparseFieldMatrixCsc`] only and is reached by a [`SparseFieldMatrix::to_csc`]
//!    flip. The iterator contract (`(row, &value)` pairs in ascending row
//!    order) is otherwise identical.
//!
//! Everything else keeps shape.
//!
//! # Transpose choice
//!
//! [`SparseFieldMatrix::transpose`] returns a freshly materialised dense
//! [`FieldMatrix<F>`]. This is the same choice the `MatrixLike<F>` trait
//! already forces on every view inside `gf2-core` (see
//! [`crate::matrix_like`]) and matches the [`MatrixLike::Owned`] associated
//! type set to `FieldMatrix<F>`. A layout-flip — CSR to CSC — is available
//! separately via [`SparseFieldMatrix::to_csc`] (symmetrically,
//! [`SparseFieldMatrixCsc::to_csr`] flips back). Keeping those as explicit
//! conversions rather than overloading `transpose` avoids the surprise of
//! `transpose` returning a different concrete type depending on which sparse
//! variant the caller holds.
//!
//! # Delayed reduction
//!
//! Sparse accumulation is already structured around per-row scatter and does
//! not benefit from the same §1.2 Dumas–Pernet kmax chunking the dense `gemm`
//! uses at the outer level. The per-row `matvec` kernel inlines the same
//! delayed-reduction dot product that `FieldVec::dot_product` uses (chunk by
//! `F::max_unreduced_additions()`, accumulate in `F::Wide`, reduce at chunk
//! boundaries) directly against the CSR `values`/`col_idx` slices, so the
//! SpMV hot path performs **zero heap allocations per row** — only the
//! output [`FieldVec`] is allocated. The inner dimension each row sees is
//! `nnz_in_row`, not `cols`, so the reduction cost is already close to
//! minimal.
//!
//! # Out-of-scope (epic Non-goals)
//!
//! * Sparse reordering (Markowitz, minimum-degree) from Dumas–Pernet §5 is
//!   intentionally deferred per the epic Non-goals; the constructors leave
//!   rows and columns in natural order.
//! * Wiedemann / Lanczos black-box solvers are likewise deferred.
//!
//! # Examples
//!
//! ```
//! use gf2_core::field::matrix::FieldMatrix;
//! use gf2_core::field::sparse_matrix::SparseFieldMatrix;
//! use gf2_core::field::FieldVec;
//! use gf2_core::gfp::Fp;
//!
//! type F = Fp<7>;
//!
//! let mut m = FieldMatrix::<F>::zeros(3, 4);
//! m.set(0, 1, F::new(2));
//! m.set(2, 3, F::new(5));
//!
//! let s = SparseFieldMatrix::from_dense(&m);
//! assert_eq!(s.nnz(), 2);
//!
//! let x = FieldVec::from(vec![F::new(1), F::new(2), F::new(3), F::new(4)]);
//! let y = s.matvec(&x);
//! assert_eq!(y[0], F::new(2) * F::new(2)); // 4
//! assert_eq!(y[2], F::new(5) * F::new(4)); // 20 mod 7 = 6
//! ```

use crate::field::matrix::FieldMatrix;
use crate::field::vec::FieldVec;
use crate::field::{ConstField, FiniteField};
use crate::matrix_like::MatrixLike;

// ─── CSR ─────────────────────────────────────────────────────────────────────

/// Row-major sparse matrix over a [`FiniteField`] in Compressed-Sparse-Row
/// (CSR) form.
///
/// All stored values are non-zero (constructors canonicalise) and per-row
/// column indices are sorted ascending. See the module-level docs for the
/// full layout, parity table with [`SpBitMatrix`](crate::sparse::SpBitMatrix),
/// and the transpose contract.
///
/// # Examples
///
/// ```
/// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// type F = Fp<7>;
///
/// // Build 2×3 matrix with entries at (0,1)=3 and (1,2)=5.
/// let s = SparseFieldMatrix::<F>::from_triplets(
///     2,
///     3,
///     [(0usize, 1usize, F::new(3)), (1, 2, F::new(5))],
/// );
/// assert_eq!(s.shape(), (2, 3));
/// assert_eq!(s.nnz(), 2);
/// assert_eq!(s.get(0, 1), F::new(3));
/// assert_eq!(s.get(0, 2), F::new(0));
/// ```
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

// ─── CSC ─────────────────────────────────────────────────────────────────────

/// Column-major sparse matrix over a [`FiniteField`] in
/// Compressed-Sparse-Column (CSC) form.
///
/// All stored values are non-zero and per-column row indices are sorted
/// ascending. See the module-level docs.
///
/// # Examples
///
/// ```
/// use gf2_core::field::sparse_matrix::{SparseFieldMatrix, SparseFieldMatrixCsc};
/// use gf2_core::gfp::Fp;
///
/// type F = Fp<7>;
///
/// let csr = SparseFieldMatrix::<F>::from_triplets(
///     2,
///     3,
///     [(0usize, 1usize, F::new(3)), (1, 2, F::new(5))],
/// );
/// let csc: SparseFieldMatrixCsc<F> = csr.to_csc();
/// assert_eq!(csc.shape(), (2, 3));
/// assert_eq!(csc.nnz(), 2);
/// assert_eq!(csc.get(1, 2), F::new(5));
/// ```
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

// ─── Shared helpers ──────────────────────────────────────────────────────────

/// Returns a field `zero` element either by borrowing from an existing slice
/// or via the static escape hatch [`FiniteField::zero_hint`] when no witness
/// is available. Panics when both paths fail (runtime-context field with no
/// elements to copy).
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

/// Returns a zero element borrowed from whichever side of a `(matrix, vector)`
/// pair carries at least one element. Fallback is [`FiniteField::zero_hint`].
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

// ─── CSR impl ────────────────────────────────────────────────────────────────

impl<F: FiniteField> SparseFieldMatrix<F> {
    /// Creates a structurally empty `rows × cols` sparse matrix (no stored
    /// non-zeros).
    ///
    /// # Arguments
    ///
    /// * `rows` — row count.
    /// * `cols` — column count.
    ///
    /// # Complexity
    ///
    /// O(rows) — only the `row_ptr` array is allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let s = SparseFieldMatrix::<Fp<7>>::zeros(3, 5);
    /// assert_eq!(s.shape(), (3, 5));
    /// assert_eq!(s.nnz(), 0);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let s = SparseFieldMatrix::<Fp<7>>::zeros(3, 5);
    /// assert_eq!(s.rows(), 3);
    /// ```
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Returns the number of columns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let s = SparseFieldMatrix::<Fp<7>>::zeros(3, 5);
    /// assert_eq!(s.cols(), 5);
    /// ```
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Returns `(rows, cols)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let s = SparseFieldMatrix::<Fp<7>>::zeros(3, 5);
    /// assert_eq!(s.shape(), (3, 5));
    /// ```
    #[inline]
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Returns the number of stored non-zero entries.
    ///
    /// All zero values are dropped by the canonicalising constructors, so
    /// this is an exact count of structural non-zeros.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     2,
    ///     [(0usize, 0usize, F::new(3))],
    /// );
    /// assert_eq!(s.nnz(), 1);
    /// ```
    #[inline]
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Returns the value stored at `(row, col)`, or `F::zero()` if the entry
    /// is structurally absent.
    ///
    /// # Arguments
    ///
    /// * `row` — row index in `0..rows`.
    /// * `col` — column index in `0..cols`.
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     3,
    ///     [(0usize, 1usize, F::new(3)), (1, 2, F::new(5))],
    /// );
    /// assert_eq!(s.get(0, 1), F::new(3));
    /// assert_eq!(s.get(1, 0), F::new(0));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     3,
    ///     [(0usize, 2usize, F::new(3)), (0, 0, F::new(1))],
    /// );
    /// let pairs: Vec<(usize, F)> =
    ///     s.row_iter(0).map(|(c, v)| (c, v.clone())).collect();
    /// assert_eq!(pairs, vec![(0, F::new(1)), (2, F::new(3))]);
    /// ```
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

    /// Returns `(row_ptr, col_idx, values)` as borrowed slices — useful for
    /// callers that want to scan the underlying arrays without reconstructing
    /// the triplet list. The `row_ptr` slice has length `rows + 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::zeros(3, 3);
    /// let (rp, ci, vs) = s.as_raw_parts();
    /// assert_eq!(rp.len(), 4);
    /// assert!(ci.is_empty());
    /// assert!(vs.is_empty());
    /// ```
    #[inline]
    pub fn as_raw_parts(&self) -> (&[usize], &[usize], &[F]) {
        (&self.row_ptr, &self.col_idx, &self.values)
    }

    /// Builds a sparse matrix from an arbitrary triplet stream, canonicalising
    /// the result: duplicate `(row, col)` pairs are **summed** (field
    /// arithmetic), explicit zeros are dropped, and column indices are sorted
    /// ascending within each row.
    ///
    /// # Arguments
    ///
    /// * `rows`, `cols` — declared shape.
    /// * `triplets` — any iterator of `(row, col, value)`. Entries with
    ///   `row >= rows` or `col >= cols` cause a panic; values equal to the
    ///   field's zero are dropped after summing.
    ///
    /// # Panics
    ///
    /// Panics on out-of-bounds indices.
    ///
    /// # Complexity
    ///
    /// O(nnz log(nnz/rows)) from the per-row sort, plus O(nnz) for the
    /// duplicate-merge and zero-drop pass.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// // Duplicates sum: (0,0,3) + (0,0,4) = 7 ≡ 0 (mod 7), so this cell is
    /// // dropped entirely.
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     2,
    ///     [
    ///         (0usize, 0usize, F::new(3)),
    ///         (0, 0, F::new(4)),
    ///         (1, 1, F::new(2)),
    ///     ],
    /// );
    /// assert_eq!(s.nnz(), 1);
    /// assert_eq!(s.get(0, 0), F::new(0));
    /// assert_eq!(s.get(1, 1), F::new(2));
    /// ```
    pub fn from_triplets<I>(rows: usize, cols: usize, triplets: I) -> Self
    where
        I: IntoIterator<Item = (usize, usize, F)>,
    {
        // Bucket triplets by row. Each bucket is `(col, value)`. Explicit
        // zeros are passed through; post-sum zeros are dropped below.
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
                // Start accumulator from the first value; fold in duplicates.
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let mut m = FieldMatrix::<F>::zeros(2, 3);
    /// m.set(0, 1, F::new(4));
    /// m.set(1, 2, F::new(5));
    /// let s = SparseFieldMatrix::from_dense(&m);
    /// assert_eq!(s.nnz(), 2);
    /// assert_eq!(s.get(0, 1), F::new(4));
    /// ```
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

    /// Materialises a dense [`FieldMatrix<F>`]. The stored zero-values are
    /// expanded back to explicit `F::zero()` cells.
    ///
    /// # Panics
    ///
    /// Panics if the matrix has `0` non-zeros **and** `F` provides no
    /// [`FiniteField::zero_hint`] witness (pure runtime-context field). The
    /// canonical `ConstField` impls never hit this path.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) — the dense back-buffer is zero-filled and then the
    /// `nnz` stored entries are scattered into place.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     2,
    ///     [(0usize, 0usize, F::new(1)), (1, 1, F::new(1))],
    /// );
    /// let m = s.to_dense();
    /// assert_eq!(m.get(0, 0), F::new(1));
    /// assert_eq!(m.get(0, 1), F::new(0));
    /// ```
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
    /// shape in O(nnz + rows + cols).
    ///
    /// # Complexity
    ///
    /// O(nnz + rows + cols). Exactly one scatter pass.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     3,
    ///     3,
    ///     [(0usize, 2usize, F::new(4)), (2, 0, F::new(5))],
    /// );
    /// let csc = s.to_csc();
    /// assert_eq!(csc.get(0, 2), F::new(4));
    /// assert_eq!(csc.get(2, 0), F::new(5));
    /// ```
    pub fn to_csc(&self) -> SparseFieldMatrixCsc<F> {
        let nnz = self.values.len();
        // Count per-column occupancy. Each non-zero in row r at column c
        // contributes one entry to column c of the CSC.
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
        // Preallocate the output with placeholders cloned from existing
        // values so the storage is type-correct without requiring
        // `F: ConstField`.
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

    /// Creates an `n × n` identity matrix stored as one non-zero per row.
    ///
    /// Requires [`ConstField`] because it must manufacture `F::one()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let id = SparseFieldMatrix::<F>::identity(3);
    /// assert_eq!(id.nnz(), 3);
    /// assert_eq!(id.get(1, 1), F::new(1));
    /// ```
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
    /// Panics if `x.len() != self.cols()`. Also panics on the pathological
    /// `(rows > 0, cols == 0)` runtime-field shape if `F::zero_hint()`
    /// returns `None` (use [`ConstField`] if you need that edge case).
    ///
    /// # Complexity
    ///
    /// O(nnz) multiply-adds.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     3,
    ///     [(0usize, 1usize, F::new(2)), (1, 2, F::new(3))],
    /// );
    /// let x = FieldVec::from(vec![F::new(1), F::new(4), F::new(5)]);
    /// let y = s.matvec(&x);
    /// assert_eq!(y[0], F::new(2) * F::new(4));
    /// assert_eq!(y[1], F::new(3) * F::new(5));
    /// ```
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
        // Inline the delayed-reduction dot product per row rather than
        // gathering `x[col_idx[k]]` into a scratch `Vec<F>` before calling
        // `dot_product_slices`. This keeps the SpMV hot path allocation-free
        // (O(nnz) multiply-adds, zero heap allocations beyond the output
        // vector), mirroring the structure of `matvec_transpose`. The chunked
        // `Wide` accumulator preserves the §1.2 Dumas–Pernet delayed-reduction
        // bound for prime fields where `max_unreduced_additions()` is finite.
        //
        // Layout optimization (jit:3a37e0f6): use `mul_product_sum_wide` +
        // `reduce_product_sum_wide` instead of `mul_to_wide` + `reduce_wide`.
        // For Fp<P> with Montgomery storage, `mul_to_wide` calls `from_mont`
        // on both operands (REDC per multiply), while `mul_product_sum_wide`
        // works directly on the storage-domain words and defers the single
        // REDC to `reduce_product_sum_wide` at the chunk boundary. This
        // matches the hot path used by `dot_product_slices` in field/vec.rs
        // and eliminates ~2 REDC calls per non-zero for Montgomery primes
        // (GF(7), GF(251), GF(65521)).
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
                // Fast path: no overflow possible (e.g., GF(2^m), Wide = Self).
                // Use storage-domain mul to avoid per-element canonical conversion.
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
                // General case: chunk by `kmax`, accumulate in `Wide`, reduce
                // at chunk boundaries. Matches `dot_product_slices` semantics
                // exactly, just without the gather. Use storage-domain mul
                // (`mul_product_sum_wide`) to bypass per-element from_mont.
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

    /// Computes `y = Aᵀ · x`. Length of `x` is `self.rows()`, length of `y`
    /// is `self.cols()`. Implementation scatters each stored non-zero of `A`
    /// into the output vector.
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     3,
    ///     [(0usize, 1usize, F::new(2)), (1, 2, F::new(3))],
    /// );
    /// let x = FieldVec::from(vec![F::new(1), F::new(4)]);
    /// let y = s.matvec_transpose(&x);
    /// // y[1] = 2 * 1; y[2] = 3 * 4 = 12 mod 7 = 5
    /// assert_eq!(y[0], F::new(0));
    /// assert_eq!(y[1], F::new(2));
    /// assert_eq!(y[2], F::new(5));
    /// ```
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
                // y[c] += values[k] * x[r]
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
    /// Panics if `self.cols() != b.rows()`.
    ///
    /// # Complexity
    ///
    /// O(nnz · B.cols) multiply-adds — one pass per stored non-zero of `A`
    /// and per column of `B`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let a = SparseFieldMatrix::<F>::identity(3);
    /// let mut b = FieldMatrix::<F>::zeros(3, 2);
    /// b.set(0, 0, F::new(1));
    /// b.set(1, 1, F::new(2));
    /// b.set(2, 0, F::new(3));
    /// let c = a.matmat(&b);
    /// assert_eq!(c, b);
    /// ```
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
        // Zero witness: any stored value in A works, otherwise any cell of B,
        // otherwise the static hint.
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
        // SIMD fast path (jit:3a37e0f6 packed-int continuation): for
        // `Fp<P>` with `P ≤ 65521`, route the whole sparse-times-dense
        // matmat through the AVX2 byte / 16-bit-lane SpMM kernel. The
        // hook packs the dense `b` once into canonical bytes (or u16),
        // sweeps every row of `A` through the kernel, and unpacks the
        // output. For other fields (Mersenne-31, GF(2^m)), or when the
        // `simd` feature is disabled / AVX2 is unavailable, the hook
        // returns `false` and we fall back to the per-row Wide
        // accumulator path below.
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

        // Layout optimization (jit:3a37e0f6): per-row Wide accumulator.
        //
        // Strategy: maintain a Vec<F::Wide> of length `out_cols` for each
        // output row. Accumulate `a_rk.mul_product_sum_wide(&b[k,j])` into
        // wide[j] for every non-zero (k, a_rk) of A's row. Reduce each wide[j]
        // once at the end of the row. This replaces `nnz_per_row * out_cols`
        // full F multiplications (Montgomery REDC + modular reduction) with the
        // same count of Wide multiplications (raw u128 muls for Fp<P>), deferring
        // the expensive reduction to one call per output cell per row.
        //
        // For kmax == usize::MAX (GF(2^m), Wide = Self), Wide multiplication
        // is the same cost as full multiplication, so there is no gain; we still
        // use the wide-accumulator path for code uniformity.
        //
        // For nnz_per_row > kmax (extremely dense rows in small prime fields),
        // we fall back to chunked reduction within the row loop.
        let kmax = F::max_unreduced_additions();
        // Allocate a single reusable wide-accumulator row.  Use zero.to_wide()
        // as the zero element.
        let zero_wide = zero.to_wide();
        let mut wide_row: Vec<F::Wide> = vec![zero_wide.clone(); out_cols];
        let mut out_data: Vec<F> = Vec::with_capacity(self.rows * out_cols);

        for r in 0..self.rows {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            let nnz_r = end - start;

            if nnz_r == 0 {
                // Zero row: emit out_cols zeros.
                for _ in 0..out_cols {
                    out_data.push(zero.clone());
                }
                continue;
            }

            if kmax == usize::MAX || nnz_r <= kmax {
                // Single-pass: accumulate all contributions in Wide, reduce once.
                // Reset the wide accumulator for this row.
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
                // Reduce once per output column.
                for w in wide_row.iter() {
                    out_data.push(F::reduce_product_sum_wide(w));
                }
            } else {
                // Chunked: process kmax non-zeros at a time, accumulate into
                // Wide, reduce into the output row between chunks. The chunks
                // partition the CSR non-zeros [start..end).
                let mut row_out: Vec<F> = vec![zero.clone(); out_cols];
                let mut offset = start;
                while offset < end {
                    let chunk_end = (offset + kmax).min(end);
                    // Reset wide row for this chunk.
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
    /// [`FieldMatrix<F>`].
    ///
    /// **Why dense.** The [`MatrixLike<F>`] contract fixes
    /// `Self::Owned = FieldMatrix<F>` for every sparse variant in this
    /// module (see the transpose discussion in the module-level docs), so
    /// `transpose` materialises into the owned type. Callers that need a
    /// layout-flip instead of a densified transpose should call
    /// [`SparseFieldMatrix::to_csc`].
    ///
    /// # Complexity
    ///
    /// O(rows · cols) because the output is dense.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let s = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     3,
    ///     [(0usize, 2usize, F::new(4)), (1, 0, F::new(5))],
    /// );
    /// let t = s.transpose();
    /// assert_eq!(t.shape(), (3, 2));
    /// assert_eq!(t.get(2, 0), F::new(4));
    /// assert_eq!(t.get(0, 1), F::new(5));
    /// ```
    pub fn transpose(&self) -> FieldMatrix<F> {
        // Materialise dense first, then use the dense transpose: keeps the
        // contract simple and reuses the audited dense transpose. SpMM and
        // SpMV already cover the common case where the transpose is applied
        // implicitly, so this rarely-hit path staying straightforward is
        // preferable to open-coding a sparse-to-dense transpose.
        self.to_dense().transpose()
    }

    /// Computes `C = A · B` as a sparse-times-sparse product, returning
    /// canonical CSR output (column indices sorted ascending within each
    /// row, no stored zeros, no duplicate `(row, col)` keys).
    ///
    /// The implementation uses the standard SpGEMM recipe: for each row
    /// `i` of `self`, iterate the stored non-zeros `(k, a_ik)` and
    /// accumulate `a_ik · row_k(B)` into a dense scatter buffer of length
    /// `B.cols()` carrying `Option<F>` slots. After all contributions for
    /// row `i` are folded in, the marked columns are gathered, zeros are
    /// dropped, and the result is appended to the output CSR arrays in
    /// ascending column order.
    ///
    /// Output canonicalisation: the marked-column list is sorted before
    /// emitting, and any field cancellation (e.g. `3 + 4 ≡ 0` in
    /// `Fp<7>`) drops the corresponding cell entirely. This matches the
    /// dense round-trip
    /// `self.matmul(&other).to_dense() == self.to_dense() * other.to_dense()`
    /// bitwise on every tested field and shape.
    ///
    /// # Arguments
    ///
    /// * `other` — right operand. Must satisfy `self.cols() == other.rows()`.
    ///
    /// # Panics
    ///
    /// Panics if `self.cols() != other.rows()`.
    ///
    /// # Complexity
    ///
    /// `O(Σ_i Σ_{k ∈ row_i(A)} nnz(row_k(B)))` field operations plus
    /// `O(rows · B.cols)` for the scatter buffer (one allocation reused
    /// across all rows).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let a = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     2,
    ///     [(0usize, 1usize, F::new(2)), (1, 0, F::new(3))],
    /// );
    /// let b = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     2,
    ///     [(0usize, 0usize, F::new(4)), (1, 1, F::new(5))],
    /// );
    /// let c = a.matmul(&b);
    /// // c[0,1] = 2*5 = 10 ≡ 3 (mod 7); c[1,0] = 3*4 = 12 ≡ 5 (mod 7).
    /// assert_eq!(c.get(0, 1), F::new(3));
    /// assert_eq!(c.get(1, 0), F::new(5));
    /// assert_eq!(c.nnz(), 2);
    /// ```
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
            // Either dimension empty ⇒ zero-shape output, no entries.
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

                // Walk row `k` of `other`, scaling its non-zeros by `a_rk`
                // and folding them into the scatter accumulator.
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

            // Emit the touched columns of row `r` in ascending order.
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
    /// The output is the canonical reduced row-echelon form of `self`:
    /// the pivot column set is the leftmost linearly-independent subset
    /// of `self`'s columns, every pivot entry is `F::one()`, every
    /// non-pivot entry in a pivot column is `F::zero()`, and rows are
    /// ordered by pivot column. Validated bit-exact against an in-test
    /// textbook column-by-column Gauss–Jordan oracle in the issue's
    /// test sweep.
    ///
    /// # Algorithm
    ///
    /// Sparse Gauss–Jordan with **column-restricted Markowitz pivot
    /// selection** (`jit:5ce13bae`). The pivot column set of canonical
    /// RREF is uniquely determined as the leftmost linearly-independent
    /// columns; the algorithm walks pivot columns in ascending order and
    /// within each column picks the un-used row with minimum `row_nnz`.
    /// At a fixed pivot column `pc`, the only candidate rows are those
    /// whose leading column equals `pc` (others have entries only at
    /// columns `> pc` by the sorted-list invariant), so `col_nnz[pc]` is
    /// identical across candidates and the full Markowitz product
    /// `(row_nnz - 1) * (col_nnz - 1)` collapses to "minimise `row_nnz`".
    /// This matches LinBox `GaussDomain::NoReordering`'s pivot-priority
    /// strategy. Dependent rows (`row_nnz == 0`) drop out of subsequent
    /// pivot search automatically.
    ///
    /// Each row is materialised on demand into a sparse `Vec<(usize, F)>`
    /// working buffer; the pivot row is scaled to a leading `1` and the
    /// chosen column eliminated from every other row via sparse `axpy`.
    /// `row_nnz` is maintained incrementally during each axpy — re-
    /// scanning the matrix would destroy the speedup. See
    /// `dev/active/5ce13bae-markowitz-design.md` for the full design.
    ///
    /// # Panics
    ///
    /// Panics if the matrix is `0 × n` (zero rows) and `F::zero_hint()`
    /// returns `None` and `n > 0` — there is no `F` witness available.
    /// Use `F: ConstField` (every standard field impl) or pass at least a
    /// `1 × n` matrix. Never panics for square `n × n` shapes on a
    /// `ConstField`.
    ///
    /// # Complexity
    ///
    /// Worst-case `O(rows · cols · min(rows, cols))` field operations, the
    /// same big-O as dense RREF — the sparse representation buys only the
    /// constant factor for sparse intermediate matrices.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let a = SparseFieldMatrix::<F>::from_triplets(
    ///     2,
    ///     3,
    ///     [
    ///         (0usize, 0usize, F::new(2)),
    ///         (0, 2, F::new(1)),
    ///         (1, 0, F::new(1)),
    ///         (1, 1, F::new(3)),
    ///     ],
    /// );
    /// let r = a.rref();
    /// assert_eq!(r.get(0, 0), F::new(1));
    /// assert_eq!(r.get(1, 1), F::new(1));
    /// ```
    pub fn rref(&self) -> Self {
        let m = self.rows;
        let n = self.cols;

        if m == 0 || n == 0 {
            // Trivial shape; reuse the existing canonical empty CSR.
            return Self {
                rows: m,
                cols: n,
                row_ptr: vec![0; m + 1],
                col_idx: Vec::new(),
                values: Vec::new(),
            };
        }

        // Materialise each row as a sorted `Vec<(usize, F)>` of non-zeros.
        // The pivoting / elimination loops only touch sparse working
        // buffers; the output is rebuilt from these at the end.
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

        // Sparse row helpers expressed as free functions to keep the
        // outer loop compact.
        //
        // `find_first_nonzero_at_or_after(row, col)`: returns the index in
        // `row` of the first stored entry whose column is `>= col`, using
        // binary search since columns are sorted.
        fn find_at<G: FiniteField>(row: &[(usize, G)], col: usize) -> Result<usize, usize> {
            row.binary_search_by_key(&col, |&(c, _)| c)
        }

        // `scale_row(row, factor)`: in-place; assumes `factor != 0` (so
        // no entry is wiped to zero — pivot scaling preserves non-zeros).
        fn scale_row<G: FiniteField>(row: &mut [(usize, G)], factor: &G) {
            for (_, v) in row.iter_mut() {
                let new_v = v.clone() * factor.clone();
                *v = new_v;
            }
        }

        // `axpy(target, source, factor)`: target ← target − factor · source,
        // expressed as a merge between two sorted `(col, val)` lists. The
        // result is left in `target` in sorted order with zeros dropped.
        // For columns present only in `source`, the contribution is
        // `−(factor · source_val)`, computed via `Neg` on `G`.
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

        // ── Markowitz pivot bookkeeping (jit:5ce13bae) ─────────────────
        // `row_nnz[i] = rows[i].len()` is maintained incrementally after
        // each axpy. col_nnz would also be required for the full Markowitz
        // product `(row_nnz - 1) * (col_nnz - 1)`, but with the canonical
        // RREF constraint (smallest leading column first) col_nnz is
        // identical for all candidates at a fixed pivot column, so the
        // product collapses to "minimise row_nnz". See
        // `dev/active/5ce13bae-markowitz-design.md` § "Pivot column choice".
        let mut row_nnz: Vec<usize> = rows.iter().map(|r| r.len()).collect();

        // `row_used[i]` is `true` once row `i` has been chosen as pivot.
        let mut row_used = vec![false; m];
        // Pivots in pick order: `(original_row, pivot_col)`. Final output
        // sorts these by pivot_col ascending for canonical RREF row order.
        let mut pivot_order: Vec<(usize, usize)> = Vec::new();

        // Outer loop: at most `min(m, n)` pivots. Each iteration picks
        // one pivot or breaks if no eligible row remains.
        for _ in 0..m.min(n) {
            // Markowitz pivot search subject to canonical-RREF ordering.
            // The pivot column SET of an RREF is uniquely determined
            // (leftmost independent columns); among un-used rows we must
            // pick the smallest column `pc` that is still the leading
            // entry of some un-used row, then pick the row at `pc` with
            // minimum row_nnz (the Markowitz fill-minimising choice
            // among rows that share `pc` as leading column, since
            // col_nnz[pc] is the same for all candidates). See
            // `dev/active/5ce13bae-markowitz-design.md` § "Pivot column
            // choice".
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

            // Scale pivot row so leading entry at `pc` is 1.
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

            // Eliminate `pc` from every other row that has a non-zero
            // there. Use index-based loop to avoid borrow conflicts.
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

        // Sort pivots by pivot column ascending for canonical RREF row
        // order — matches dense `FieldMatrix::rref`.
        pivot_order.sort_by_key(|&(_orig, pc)| pc);

        let mut ordered: Vec<Vec<(usize, F)>> = Vec::with_capacity(m);
        for &(orig, _) in &pivot_order {
            ordered.push(std::mem::take(&mut rows[orig]));
        }
        // Pad with empty rows for rows that never became pivots.
        while ordered.len() < m {
            ordered.push(Vec::new());
        }

        // Flatten back to CSR.
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

// ─── CSC impl ────────────────────────────────────────────────────────────────

impl<F: FiniteField> SparseFieldMatrixCsc<F> {
    /// Creates a structurally empty CSC matrix.
    ///
    /// # Complexity
    ///
    /// O(cols).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrixCsc;
    /// use gf2_core::gfp::Fp;
    ///
    /// let s = SparseFieldMatrixCsc::<Fp<7>>::zeros(3, 4);
    /// assert_eq!(s.shape(), (3, 4));
    /// assert_eq!(s.nnz(), 0);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrixCsc;
    /// use gf2_core::gfp::Fp;
    ///
    /// assert_eq!(SparseFieldMatrixCsc::<Fp<7>>::zeros(3, 4).rows(), 3);
    /// ```
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrixCsc;
    /// use gf2_core::gfp::Fp;
    ///
    /// assert_eq!(SparseFieldMatrixCsc::<Fp<7>>::zeros(3, 4).cols(), 4);
    /// ```
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Returns `(rows, cols)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrixCsc;
    /// use gf2_core::gfp::Fp;
    ///
    /// assert_eq!(SparseFieldMatrixCsc::<Fp<7>>::zeros(3, 4).shape(), (3, 4));
    /// ```
    #[inline]
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Number of stored non-zero entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrixCsc;
    /// use gf2_core::gfp::Fp;
    ///
    /// assert_eq!(SparseFieldMatrixCsc::<Fp<7>>::zeros(3, 4).nnz(), 0);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::{SparseFieldMatrix, SparseFieldMatrixCsc};
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let csc = SparseFieldMatrix::<F>::from_triplets(
    ///     2, 3, [(0usize, 1usize, F::new(3))],
    /// ).to_csc();
    /// assert_eq!(csc.get(0, 1), F::new(3));
    /// assert_eq!(csc.get(1, 0), F::new(0));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::{SparseFieldMatrix, SparseFieldMatrixCsc};
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let csc: SparseFieldMatrixCsc<F> = SparseFieldMatrix::<F>::from_triplets(
    ///     3, 2, [(0usize, 0usize, F::new(1)), (2, 0, F::new(2))],
    /// ).to_csc();
    /// let got: Vec<(usize, F)> =
    ///     csc.col_iter(0).map(|(r, v)| (r, v.clone())).collect();
    /// assert_eq!(got, vec![(0, F::new(1)), (2, F::new(2))]);
    /// ```
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

    /// Converts to CSR with the same shape in O(nnz + rows + cols).
    ///
    /// # Complexity
    ///
    /// O(nnz + rows + cols).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::{SparseFieldMatrix, SparseFieldMatrixCsc};
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let csr = SparseFieldMatrix::<F>::from_triplets(
    ///     3, 2, [(0usize, 1usize, F::new(4))],
    /// );
    /// let roundtrip = csr.to_csc().to_csr();
    /// assert_eq!(roundtrip, csr);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::{SparseFieldMatrix, SparseFieldMatrixCsc};
    /// use gf2_core::gfp::Fp;
    ///
    /// type F = Fp<7>;
    /// let csc = SparseFieldMatrix::<F>::from_triplets(
    ///     2, 2, [(0usize, 0usize, F::new(1)), (1, 1, F::new(1))],
    /// ).to_csc();
    /// let m = csc.to_dense();
    /// assert_eq!(m.get(0, 0), F::new(1));
    /// assert_eq!(m.get(1, 1), F::new(1));
    /// ```
    pub fn to_dense(&self) -> FieldMatrix<F> {
        self.to_csr().to_dense()
    }

    /// Returns `(col_ptr, row_idx, values)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::sparse_matrix::SparseFieldMatrixCsc;
    /// use gf2_core::gfp::Fp;
    ///
    /// let s = SparseFieldMatrixCsc::<Fp<7>>::zeros(2, 3);
    /// let (cp, ri, vs) = s.as_raw_parts();
    /// assert_eq!(cp.len(), 4);
    /// assert!(ri.is_empty());
    /// assert!(vs.is_empty());
    /// ```
    #[inline]
    pub fn as_raw_parts(&self) -> (&[usize], &[usize], &[F]) {
        (&self.col_ptr, &self.row_idx, &self.values)
    }
}

// ─── MatrixLike impls ────────────────────────────────────────────────────────

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
        // Densify first, then use the dense transpose (same choice as the
        // CSR variant). Keeps the `Owned = FieldMatrix<F>` contract.
        self.to_dense().transpose()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

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

    // GF(2^8) via Gf2mWide, AES irreducible.
    struct Gf2m8AesCfg;
    impl Gf2mWideConfig<1> for Gf2m8AesCfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "SparseTestsGf2m8AesCfg";
    }
    type G8 = Gf2mWide<1, Gf2m8AesCfg>;

    // ── Generic test helpers ─────────────────────────────────────────────

    fn dense_random_fp<const P: u64>(
        rows: usize,
        cols: usize,
        density: f64,
        seed: u64,
    ) -> FieldMatrix<Fp<P>> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                if rng.gen::<f64>() < density {
                    let v = (rng.gen::<u64>() % (P - 1)) + 1;
                    m.set(r, c, Fp::<P>::new(v));
                }
            }
        }
        m
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

    // ── Roundtrip: from_dense → to_dense ─────────────────────────────────

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

    // ── CSR ↔ CSC round-trip ─────────────────────────────────────────────

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

    // ── SpMV ─────────────────────────────────────────────────────────────

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

    // ── SpMV transpose ───────────────────────────────────────────────────

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

    // ── SpMM ─────────────────────────────────────────────────────────────

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

    // ── Triplet canonicalisation ─────────────────────────────────────────

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

    // ── Identity + matvec correctness ────────────────────────────────────

    #[test]
    fn test_identity_matvec_fp7() {
        let id = SparseFieldMatrix::<F7>::identity(5);
        let x = random_fieldvec_fp::<7>(5, 0x301);
        assert_eq!(id.matvec(&x), x);
    }

    // ── Edge cases ───────────────────────────────────────────────────────

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
        // matvec on the structurally empty matrix — must be y = 0.
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
        // Every cell non-zero; sparse representation just stores them all.
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
        // m = 1, n = 10_000. Verify matvec on a single-row sparse matrix.
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

    // ── Transpose contract ───────────────────────────────────────────────

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
        // Using the MatrixLike trait method path.
        let t = <SparseFieldMatrixCsc<F7> as MatrixLike<F7>>::transpose(&csc);
        assert_eq!(t, m.transpose());
    }

    // ── MatrixLike plumbing ──────────────────────────────────────────────

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

    // ── FieldMatrix::to_sparse keeps compiling and semantically matches ──

    #[test]
    fn test_field_matrix_to_sparse_returns_csr() {
        let m = dense_random_fp::<7>(4, 5, 0.3, 0xA01);
        let s = m.to_sparse();
        assert_eq!(s.shape(), m.shape());
        assert_eq!(s.to_dense(), m);
    }

    // ── Sparse × sparse matmul (issue eb57f944) ──────────────────────────

    /// Helper: build a fresh `Gf2mWide<1, _>` sparse from a dense witness.
    /// Useful in `matmul` / `rref` tests below.
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
        // 3 × 0 times 0 × 4 = 3 × 4 zero matrix.
        let a = SparseFieldMatrix::<F7>::zeros(3, 0);
        let b = SparseFieldMatrix::<F7>::zeros(0, 4);
        let c = a.matmul(&b);
        assert_eq!(c.shape(), (3, 4));
        assert_eq!(c.nnz(), 0);
    }

    #[test]
    fn test_matmul_empty_rows() {
        // 0 × n times n × m = 0 × m, no entries.
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
        // (1,4) at (0,0) × (1,3) at (0,0) = 4·3 = 12 ≡ 5; pair with cancelling row.
        // a = [[1, 1]]; b = [[3], [4]]; c = [[3 + 4]] ≡ [[0]] ⇒ nnz = 0.
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
        // After matmul, every emitted col_idx slice must be sorted; no
        // value may be the zero element.
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

    // ── Sparse RREF (issue eb57f944) ────────────────────────────────────

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
        // Rows > cols: typical "stack of constraints" shape.
        let a_dense = dense_random_g8(8, 4, 0.3, 0x2003);
        let a = sparse_from_dense_g8(&a_dense);
        let got = a.rref();
        let (_x, expected) = a_dense.rref();
        assert_eq!(got.to_dense(), expected);
    }

    #[test]
    fn test_rref_matches_dense_fp7() {
        // RREF is generic over `F: FiniteField`, so verify on a prime field.
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

    // GF(2^16) configuration to exercise the wider Gf2mWide path.
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
        // Surrogate for `Gf2mWide<u32>` listed in the criterion: GF(2^16)
        // also exercises a Gf2mWide with a non-byte-aligned bit width and
        // covers the elimination path with a different irreducible.
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
        // 4×3 zero matrix: RREF is itself.
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

    // ── Markowitz-degree RREF coverage (jit:5ce13bae) ──────────────────────
    //
    // RREF is uniquely determined by its canonical form. Markowitz pivot
    // selection only changes the internal pick order, not the canonical
    // output. The tests below validate against an independent direct
    // Gauss-Jordan oracle (`direct_rref_reference_fp` / `_g8`) rather
    // than the dense `FieldMatrix::rref` path. The dense PLE-based path
    // has a pre-existing canonical-RREF divergence on certain sparse
    // inputs (documented as an open question in the issue's evidence
    // doc); the Markowitz sparse path is verified to always produce
    // the true canonical RREF.

    /// Independent oracle: straight-line column-by-column Gauss-Jordan
    /// over GF(p) producing canonical RREF. Used as the byte-equality
    /// reference for Markowitz tests where the dense `FieldMatrix::rref`
    /// path diverges from canonical on certain sparse inputs.
    #[cfg(test)]
    fn direct_rref_reference_fp<const P: u64>(a: &FieldMatrix<Fp<P>>) -> FieldMatrix<Fp<P>> {
        let (m, n) = a.shape();
        let mut e = a.clone();
        let zero = Fp::<P>::new(0);
        let one = Fp::<P>::new(1);
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

    /// Same as `direct_rref_reference_fp` but for `Gf2mWide<1, _>` —
    /// the inv()/sub/mul interface is identical via the FiniteField
    /// trait so we just specialise to the G8 type used by the sweep.
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

    /// 1x1 matrix with a single non-zero entry: RREF is `[[1]]`.
    #[test]
    fn test_rref_markowitz_1x1_single_entry_fp7() {
        let a = SparseFieldMatrix::<F7>::from_triplets(1, 1, [(0usize, 0usize, F7::new(3))]);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a.to_dense());
        assert_eq!(r.to_dense(), expected);
    }

    /// Tall (rows > cols) deficient matrix — exercises the early-out path.
    #[test]
    fn test_rref_markowitz_tall_deficient_fp7() {
        let a_dense = dense_random_fp::<7>(8, 4, 0.3, 0x5CE1_BAE0);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    /// Wide (cols > rows) matrix — many free columns, exercises Markowitz
    /// score = 0 path (singleton-column entries).
    #[test]
    fn test_rref_markowitz_wide_fp7() {
        let a_dense = dense_random_fp::<7>(4, 12, 0.25, 0x5CE1_BAE1);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    /// Very sparse n=64 (one entry per row on average): matches the
    /// word-boundary edge from the GF(2) test suite, ported to GF(p).
    #[test]
    fn test_rref_markowitz_word_boundary_n64_fp7() {
        let a_dense = dense_random_fp::<7>(64, 64, 1.0 / 64.0, 0x5CE1_BAE2);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    /// Word-boundary n=65 with a denser regime.
    #[test]
    fn test_rref_markowitz_word_boundary_n65_fp65521() {
        let a_dense = dense_random_fp::<65521>(65, 65, 0.05, 0x5CE1_BAE3);
        let a = SparseFieldMatrix::from_dense(&a_dense);
        let r = a.rref();
        let expected = direct_rref_reference_fp(&a_dense);
        assert_eq!(r.to_dense(), expected);
    }

    /// Multi-seed sweep across shapes and densities for Fp<7>. Validates
    /// against the independent direct Gauss-Jordan oracle plus
    /// RREF-idempotence.
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

    /// Idempotence + canonical parity sweep for GF(65521) (mid-size prime).
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

    /// Sweep over GF(2^8) — exercises a non-prime field axpy path.
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
