//! Dense row-major matrix [`FieldMatrix<F>`] over an arbitrary
//! [`FiniteField`], its zero-copy views [`MatView`], [`MatViewMut`] and
//! [`ColView`], and the classical `gemm` kernels.

use std::fmt;
use std::ops::{Bound, Index, RangeBounds};
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use crate::field::{ConstField, FieldVec, FiniteField};
use crate::matrix_like::{MatrixLike, MatrixLikeMut};

pub use crate::field::ple::Permutation;

/// Basis of column vectors for the `cyclic_decomposition` reduce loop, held
/// in a field-specific packed form. A field without one returns `None` from
/// `try_make_basis_reducer`.
pub trait BasisReducer<F: FiniteField>: Send {
    /// Appends `col`, which has a non-zero entry, as basis column `self.len()`.
    fn push_col(&mut self, col: &[F]);

    /// [`push_col`](Self::push_col) for a column whose pivot row is known.
    /// The caller guarantees `col[pivot_row]` is non-zero; an implementation
    /// may cache its inverse.
    fn push_col_with_pivot_row(&mut self, col: &[F], pivot_row: usize) {
        let _ = pivot_row;
        self.push_col(col);
    }

    /// Computes `(residual, coeffs)` such that
    /// `v = Σ coeffs[j] · basis[j] + residual`, with `residual` having
    /// zeros at every pivot row. `pivot_row_of_col` must align with
    /// the columns appended via [`Self::push_col`] (one entry per
    /// column, in append order).
    fn reduce(&self, v: &[F], pivot_row_of_col: &[usize]) -> (Vec<F>, Vec<F>);

    /// Columns appended so far: the length [`reduce`](Self::reduce) expects of
    /// `pivot_row_of_col`.
    fn len(&self) -> usize;

    /// Returns `true` when no columns have been appended yet.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A matrix packed once for the repeated matvec products of
/// `cyclic_decomposition` and `wiedemann_minpoly_attempt`. A field without
/// one returns `None` from `try_prepack_matvec`.
pub trait PackedMatvec<F: FiniteField>: Send {
    /// Computes `out = A · x` using the pre-packed matrix. The caller
    /// guarantees `x.len() == k` and `out.len() == m` where `m, k` were
    /// the shape passed to `try_prepack_matvec`.
    fn matvec(&self, x: &[F], out: &mut [F]);
}

/// Packed arithmetic for the chain polynomials of `cyclic_decomposition`,
/// whose Krylov step `d` computes
///
/// ```text
/// next_poly = x · chain_polys[d-1]
///           − Σ_{j=0}^{d-1} α_j · chain_polys[j]
/// ```
///
/// A field without an implementation returns `None` from
/// `try_make_chain_poly_arith`.
pub trait ChainPolyArith<F: FiniteField>: Send {
    /// Appends the constant polynomial `1` as the first chain entry.
    fn push_one(&mut self);

    /// Computes `x · chain_polys[last]` and stores the result into `buf`.
    /// `buf` is an opaque scratch buffer managed by this trait; callers
    /// pass the same value back to [`sub_scaled_into`](Self::sub_scaled_into) and then to
    /// [`push_buf`](Self::push_buf) or [`finish_buf`](Self::finish_buf).
    fn shift_x_last_into(&self, buf: &mut Vec<u8>);

    /// Subtracts `alpha · chain_polys[j]` from `buf`, the accumulator started
    /// by [`shift_x_last_into`](Self::shift_x_last_into).
    fn sub_scaled_into(&mut self, buf: &mut Vec<u8>, alpha: &F, j: usize);

    /// Appends the polynomial stored in `buf` as the next chain entry.
    fn push_buf(&mut self, buf: &[u8]);

    /// Converts `buf` to a `FieldPoly<F>`.
    fn finish_buf(&self, buf: &[u8], zero: &F) -> crate::field::poly::FieldPoly<F>;

    /// Allocates a scratch buffer for polynomials of degree up to `max_deg`.
    fn alloc_buf(&self, max_deg: usize) -> Vec<u8>;

    /// Returns the number of chain polynomials stored so far.
    fn len(&self) -> usize;

    /// Returns `true` when no polynomials have been stored yet.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
thread_local! {
    static FIELDMATRIX_NEW_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Test-only: count of owned [`FieldMatrix`] constructions (`new`,
/// `transpose`, view `to_owned`) on this thread since the last
/// [`reset_fieldmatrix_new_count`].
#[cfg(test)]
pub(crate) fn fieldmatrix_new_count() -> u64 {
    FIELDMATRIX_NEW_COUNT.with(|c| c.get())
}

#[cfg(test)]
pub(crate) fn reset_fieldmatrix_new_count() {
    FIELDMATRIX_NEW_COUNT.with(|c| c.set(0));
}

/// Lazy-transpose proxy: wraps a matrix reference and reports `rows` and
/// `cols` swapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transposed<M>(pub M);

impl<F: FiniteField> Transposed<&FieldMatrix<F>> {
    /// Rows of the logically transposed matrix (i.e. columns of the backing
    /// matrix).
    pub fn rows(&self) -> usize {
        self.0.cols()
    }

    /// Columns of the logically transposed matrix (i.e. rows of the backing
    /// matrix).
    pub fn cols(&self) -> usize {
        self.0.rows()
    }
}

/// Row-major dense matrix over a [`FiniteField`].
///
/// # Storage
///
/// Entries are stored row-major in a single [`FieldVec<F>`] of length
/// `rows * cols`. Element `(r, c)` lives at linear index `r * cols + c`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMatrix<F: FiniteField> {
    rows: usize,
    cols: usize,
    data: FieldVec<F>,
}

impl<F: FiniteField> FieldMatrix<F> {
    /// Creates an `rows × cols` matrix with every entry equal to `fill`.
    pub fn new(rows: usize, cols: usize, fill: F) -> Self {
        #[cfg(test)]
        FIELDMATRIX_NEW_COUNT.with(|c| c.set(c.get() + 1));
        let data: FieldVec<F> = (0..rows * cols).map(|_| fill.clone()).collect();
        Self { rows, cols, data }
    }

    /// Builds a matrix from a pre-sized payload. The caller guarantees
    /// `data.len() == rows * cols`, or an empty `data` when either dimension
    /// is zero; debug builds assert it.
    #[doc(hidden)]
    pub(crate) fn from_raw_parts(rows: usize, cols: usize, data: FieldVec<F>) -> Self {
        debug_assert!(
            (rows == 0 || cols == 0) && data.is_empty() || data.len() == rows * cols,
            "FieldMatrix::from_raw_parts: inconsistent data length {} for shape ({}, {})",
            data.len(),
            rows,
            cols
        );
        Self { rows, cols, data }
    }

    /// The backing storage, row-major over `rows * cols` cells.
    #[doc(hidden)]
    pub(crate) fn as_data_slice(&self) -> &[F] {
        self.data.as_slice()
    }

    #[doc(hidden)]
    pub(crate) fn as_data_mut_slice(&mut self) -> &mut [F] {
        self.data.as_mut_slice()
    }

    /// Constructs a matrix from a `Vec` of row vectors, one [`FieldVec<F>`] per row.
    ///
    /// # Panics
    ///
    /// Panics if `rows` is empty or if the rows have unequal lengths.
    pub fn from_rows(rows: Vec<FieldVec<F>>) -> Self {
        assert!(
            !rows.is_empty(),
            "FieldMatrix::from_rows: need at least one row"
        );
        let cols = rows[0].len();
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(
                r.len(),
                cols,
                "FieldMatrix::from_rows: row {} has length {} but expected {}",
                i,
                r.len(),
                cols
            );
        }
        let nrows = rows.len();
        let mut data = FieldVec::with_capacity(nrows * cols);
        for r in rows {
            for e in r.into_iter() {
                data.push(e);
            }
        }
        Self {
            rows: nrows,
            cols,
            data,
        }
    }
}

impl<F: ConstField> FieldMatrix<F> {
    /// Equivalent to [`FieldMatrix::zeros`]; named for Armadillo parity.
    pub fn with_capacity(rows: usize, cols: usize) -> Self {
        Self::zeros(rows, cols)
    }

    /// Returns a `rows × cols` zero matrix.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: FieldVec::zeros(rows * cols),
        }
    }

    /// Returns a `rows × cols` matrix filled with the multiplicative identity.
    pub fn ones(rows: usize, cols: usize) -> Self {
        Self::new(rows, cols, F::one())
    }

    /// Returns the `n × n` identity matrix.
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, F::one());
        }
        m
    }
}

/// Lets `rng.gen::<Fp<P>>()` draw field elements, which
/// [`FieldMatrix::random`] requires.
#[cfg(feature = "rand")]
impl<const P: u64> rand::distributions::Distribution<crate::gfp::Fp<P>>
    for rand::distributions::Standard
{
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> crate::gfp::Fp<P> {
        crate::gfp::Fp::<P>::new(rng.gen::<u64>())
    }
}

#[cfg(feature = "rand")]
impl<F: ConstField> FieldMatrix<F>
where
    rand::distributions::Standard: rand::distributions::Distribution<F>,
{
    /// Returns a `rows × cols` matrix populated from `rng` via
    /// [`rand::distributions::Standard`] (uniform over the storage type).
    pub fn random<R: rand::Rng + ?Sized>(rows: usize, cols: usize, rng: &mut R) -> Self {
        let data: FieldVec<F> = (0..rows * cols).map(|_| rng.gen::<F>()).collect();
        Self { rows, cols, data }
    }

    /// Returns a `rows × cols` matrix drawn from a `StdRng` seeded with
    /// `seed`.
    pub fn random_seeded(rows: usize, cols: usize, seed: u64) -> Self {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        Self::random(rows, cols, &mut rng)
    }
}

impl<F: FiniteField> FieldMatrix<F> {
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

    /// Returns `true` if the matrix is square.
    #[inline]
    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    /// Returns `true` if either dimension is zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows == 0 || self.cols == 0
    }

    /// Returns the value at `(row, col)`.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()` or `col >= self.cols()`.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> F
    where
        F: Clone,
    {
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
        self.data.get(row * self.cols + col).clone()
    }

    /// Writes `val` at `(row, col)`.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()` or `col >= self.cols()`.
    #[inline]
    pub fn set(&mut self, row: usize, col: usize, val: F) {
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
        self.data.set(row * self.cols + col, val);
    }

    /// Unchecked element access. Skips bounds checks in release; `debug_assert`s
    /// them in debug builds.
    ///
    /// # Safety
    ///
    /// This method is safe but silently returns wrong values (or panics on a
    /// bogus index into the backing `FieldVec`) if the indices are out of
    /// bounds. Callers are expected to have verified `row < self.rows()` and
    /// `col < self.cols()` via an outer loop invariant.
    #[inline]
    pub fn get_unchecked(&self, row: usize, col: usize) -> F
    where
        F: Clone,
    {
        debug_assert!(row < self.rows);
        debug_assert!(col < self.cols);
        self.data.get(row * self.cols + col).clone()
    }

    /// Returns the row as a contiguous slice of field elements.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.rows()`.
    #[inline]
    pub fn row(&self, i: usize) -> &[F] {
        assert!(
            i < self.rows,
            "row index {} out of bounds (rows={})",
            i,
            self.rows
        );
        let start = i * self.cols;
        &self.data.as_slice()[start..start + self.cols]
    }

    /// Mutable view of row `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.rows()`.
    #[inline]
    pub fn row_mut(&mut self, i: usize) -> &mut [F] {
        assert!(
            i < self.rows,
            "row index {} out of bounds (rows={})",
            i,
            self.rows
        );
        let start = i * self.cols;
        let end = start + self.cols;
        &mut self.data.as_mut_slice()[start..end]
    }

    /// Returns a non-owning strided view over column `j`.
    ///
    /// # Panics
    ///
    /// Panics if `j >= self.cols()`.
    #[inline]
    pub fn col(&self, j: usize) -> ColView<'_, F> {
        assert!(
            j < self.cols,
            "col index {} out of bounds (cols={})",
            j,
            self.cols
        );
        ColView {
            data: self.data.as_slice(),
            start: j,
            stride: self.cols,
            len: self.rows,
        }
    }

    /// Iterator yielding references to each element of column `j`.
    ///
    /// # Panics
    ///
    /// Panics if `j >= self.cols()`.
    pub fn col_iter(&self, j: usize) -> impl Iterator<Item = &F> {
        assert!(
            j < self.cols,
            "col index {} out of bounds (cols={})",
            j,
            self.cols
        );
        let slice = self.data.as_slice();
        let cols = self.cols;
        (0..self.rows).map(move |r| &slice[r * cols + j])
    }

    /// Returns an immutable submatrix view over the rectangle
    /// `(rows, cols)`.
    ///
    /// Ranges follow standard Rust semantics: `a..b` (half-open), `a..=b`
    /// (inclusive), `..`, `..b`, `a..`. End bounds may equal the matrix
    /// dimension.
    ///
    /// # Panics
    ///
    /// Panics if the range exceeds the parent dimensions.
    pub fn submat(
        &self,
        rows: impl RangeBounds<usize>,
        cols: impl RangeBounds<usize>,
    ) -> MatView<'_, F> {
        let (r0, r1) = resolve_range(rows, self.rows);
        let (c0, c1) = resolve_range(cols, self.cols);
        MatView {
            data: self.data.as_slice(),
            parent_cols: self.cols,
            row_offset: r0,
            col_offset: c0,
            rows: r1 - r0,
            cols: c1 - c0,
        }
    }

    /// Returns a mutable submatrix view.
    ///
    /// See [`FieldMatrix::submat`] for range semantics.
    ///
    /// # Panics
    ///
    /// Panics if the range exceeds the parent dimensions.
    pub fn submat_mut(
        &mut self,
        rows: impl RangeBounds<usize>,
        cols: impl RangeBounds<usize>,
    ) -> MatViewMut<'_, F> {
        let (r0, r1) = resolve_range(rows, self.rows);
        let (c0, c1) = resolve_range(cols, self.cols);
        let parent_cols = self.cols;
        MatViewMut {
            data: self.data.as_mut_slice(),
            parent_cols,
            row_offset: r0,
            col_offset: c0,
            rows: r1 - r0,
            cols: c1 - c0,
        }
    }

    /// Convenience: submatrix selecting a contiguous row range, all columns.
    ///
    /// # Panics
    ///
    /// Panics if the range exceeds the row count.
    pub fn row_range(&self, rows: impl RangeBounds<usize>) -> MatView<'_, F> {
        self.submat(rows, ..)
    }

    /// Convenience: submatrix selecting all rows and a contiguous column range.
    ///
    /// # Panics
    ///
    /// Panics if the range exceeds the column count.
    pub fn col_range(&self, cols: impl RangeBounds<usize>) -> MatView<'_, F> {
        self.submat(.., cols)
    }
}

impl<F: FiniteField> FieldMatrix<F> {
    /// Swaps rows `r1` and `r2`. A no-op when `r1 == r2`.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds.
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
        let cols = self.cols;
        let data = self.data.as_mut_slice();
        let (lo, hi) = if r1 < r2 { (r1, r2) } else { (r2, r1) };
        let (left, right) = data.split_at_mut(hi * cols);
        let a = &mut left[lo * cols..lo * cols + cols];
        let b = &mut right[..cols];
        a.swap_with_slice(b);
    }

    /// Scales every entry of `row` by `factor`.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()`.
    pub fn scale_row(&mut self, row: usize, factor: F) {
        assert!(
            row < self.rows,
            "row index {} out of bounds (rows={})",
            row,
            self.rows
        );
        for e in self.row_mut(row) {
            *e = e.clone() * factor.clone();
        }
    }

    /// Fused multiply-add on rows: `row[dst] += factor * row[src]`.
    ///
    /// # Panics
    ///
    /// Panics if either row index is out of bounds.
    pub fn axpy_row(&mut self, dst: usize, src: usize, factor: F) {
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
        if self.cols == 0 {
            return;
        }
        if dst == src {
            // `row[dst] += factor · row[dst]` ⇔ `row[dst] := (1 + factor) · row[dst]`.
            let one = self.data.get(0).one_like();
            let scale = one + factor;
            for e in self.row_mut(dst) {
                *e = e.clone() * scale.clone();
            }
            return;
        }
        let cols = self.cols;
        let data = self.data.as_mut_slice();
        let (lo, hi) = if dst < src { (dst, src) } else { (src, dst) };
        let (left, right) = data.split_at_mut(hi * cols);
        let (lo_slice, hi_slice) = (&mut left[lo * cols..lo * cols + cols], &mut right[..cols]);
        let (dst_slice, src_slice) = if dst < src {
            (lo_slice, &*hi_slice)
        } else {
            (hi_slice, &*lo_slice)
        };
        for (d, s) in dst_slice.iter_mut().zip(src_slice.iter()) {
            *d = d.clone() + factor.clone() * s.clone();
        }
    }

    /// Returns the first row `>= start_row` with a non-zero entry in `col`,
    /// or `None`, also when `col` or `start_row` is out of range.
    pub fn find_pivot_row(&self, col: usize, start_row: usize) -> Option<usize> {
        if col >= self.cols || start_row >= self.rows {
            return None;
        }
        let zero = self.data.get(0).zero_like();
        (start_row..self.rows).find(|&r| self.data.get(r * self.cols + col) != &zero)
    }
}

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns an owned transpose.
    pub fn transpose(&self) -> Self {
        #[cfg(test)]
        FIELDMATRIX_NEW_COUNT.with(|c| c.set(c.get() + 1));
        if self.is_empty() {
            return Self {
                rows: self.cols,
                cols: self.rows,
                data: FieldVec::new(),
            };
        }
        let mut data = FieldVec::with_capacity(self.rows * self.cols);
        for c in 0..self.cols {
            for r in 0..self.rows {
                data.push(self.data.get(r * self.cols + c).clone());
            }
        }
        Self {
            rows: self.cols,
            cols: self.rows,
            data,
        }
    }

    /// Converts this dense matrix into a [`SparseFieldMatrix<F>`](crate::field::sparse_matrix::SparseFieldMatrix),
    /// keeping only the non-zero entries. The returned matrix is in CSR
    /// layout with column indices sorted ascending within each row.
    pub fn to_sparse(&self) -> crate::field::sparse_matrix::SparseFieldMatrix<F> {
        crate::field::sparse_matrix::SparseFieldMatrix::from_dense(self)
    }

    /// Returns a lazy transpose proxy borrowing `self`.
    pub fn t(&self) -> Transposed<&Self> {
        Transposed(self)
    }

    /// Returns the diagonal, of length `min(rows, cols)`.
    pub fn diag(&self) -> FieldVec<F> {
        let n = self.rows.min(self.cols);
        (0..n)
            .map(|i| self.data.get(i * self.cols + i).clone())
            .collect()
    }

    /// Sum of the diagonal entries.
    ///
    /// # Panics
    ///
    /// Panics if the matrix is empty.
    pub fn trace(&self) -> F {
        assert!(!self.is_empty(), "FieldMatrix::trace: matrix is empty");
        let n = self.rows.min(self.cols);
        let mut acc = self.data.get(0).clone();
        for i in 1..n {
            acc += self.data.get(i * self.cols + i);
        }
        acc
    }

    /// Returns `true` if `self == self.transpose()`.
    pub fn is_symmetric(&self) -> bool {
        if self.rows != self.cols {
            return false;
        }
        for i in 0..self.rows {
            for j in (i + 1)..self.cols {
                if self.data.get(i * self.cols + j) != self.data.get(j * self.cols + i) {
                    return false;
                }
            }
        }
        true
    }

    /// Computes `y = A · x`.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.cols()`, or if `self.rows() > 0`,
    /// `self.cols() == 0` and `F::zero_hint()` is `None`.
    pub fn matvec(&self, x: &FieldVec<F>) -> FieldVec<F> {
        assert_eq!(
            x.len(),
            self.cols,
            "FieldMatrix::matvec: x.len() ({}) != cols ({})",
            x.len(),
            self.cols
        );
        if self.rows == 0 {
            return FieldVec::new();
        }
        // A zero without `F: ConstField`: from `x[0]` when `cols > 0`, else
        // from `F::zero_hint()`.
        let zero: F = if self.cols > 0 {
            x.as_slice()[0].zero_like()
        } else if let Some(z) = F::zero_hint() {
            z
        } else {
            panic!(
                "FieldMatrix::matvec: producing length-{} zero vector from \
                 ({}×0) matrix requires a zero witness; use F: ConstField \
                 or ensure the matrix has at least one column",
                self.rows, self.rows
            );
        };
        let mut y: FieldVec<F> = FieldVec::zeros_from(self.rows, &zero);
        if self.cols > 0
            && F::try_simd_matvec(
                self.data.as_slice(),
                x.as_slice(),
                self.rows,
                self.cols,
                y.as_mut_slice(),
            )
        {
            return y;
        }
        // The delayed-reduction dot-product kernel serves large-prime fields
        // and GF(2^m) through one code path.
        for r in 0..self.rows {
            let row = &self.data.as_slice()[r * self.cols..(r + 1) * self.cols];
            y.set(
                r,
                crate::field::vec::dot_product_slices(row, x.as_slice(), &zero),
            );
        }
        y
    }

    /// Computes `y = Aᵀ · x`.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.rows()`, or if `self.rows() == 0`,
    /// `self.cols() > 0` and `F::zero_hint()` is `None`.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) multiply-adds, plus one transposed copy of `self`.
    pub fn matvec_transpose(&self, x: &FieldVec<F>) -> FieldVec<F> {
        assert_eq!(
            x.len(),
            self.rows,
            "FieldMatrix::matvec_transpose: x.len() ({}) != rows ({})",
            x.len(),
            self.rows
        );
        if self.cols == 0 {
            return FieldVec::new();
        }
        // The zero witness follows the rule of `matvec`.
        let zero: F = if self.rows > 0 {
            x.as_slice()[0].zero_like()
        } else if let Some(z) = F::zero_hint() {
            z
        } else {
            panic!(
                "FieldMatrix::matvec_transpose: producing length-{} zero \
                 vector from (0×{}) matrix requires a zero witness; use \
                 F: ConstField or ensure the matrix has at least one row",
                self.cols, self.cols
            );
        };
        let mut y: FieldVec<F> = FieldVec::zeros_from(self.cols, &zero);
        // Dot products over the transposed copy keep both operands contiguous
        // for the delayed-reduction kernel.
        let self_t = self.transpose();
        for j in 0..self.cols {
            let row = &self_t.data.as_slice()[j * self_t.cols..(j + 1) * self_t.cols];
            y.set(
                j,
                crate::field::vec::dot_product_slices(row, x.as_slice(), &zero),
            );
        }
        y
    }
}

impl<F: FiniteField> MatrixLike<F> for FieldMatrix<F> {
    type Owned = FieldMatrix<F>;

    #[inline]
    fn rows(&self) -> usize {
        FieldMatrix::rows(self)
    }

    #[inline]
    fn cols(&self) -> usize {
        FieldMatrix::cols(self)
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> F {
        FieldMatrix::get(self, row, col)
    }

    #[inline]
    fn transpose(&self) -> Self {
        FieldMatrix::transpose(self)
    }
}

impl<F: FiniteField> MatrixLikeMut<F> for FieldMatrix<F> {
    #[inline]
    fn set(&mut self, row: usize, col: usize, v: F) {
        FieldMatrix::set(self, row, col, v);
    }

    #[inline]
    fn swap_rows(&mut self, r1: usize, r2: usize) {
        FieldMatrix::swap_rows(self, r1, r2);
    }
}

/// Zero-copy immutable submatrix view.
///
/// A `MatView` borrows a rectangular window of a parent [`FieldMatrix`].
/// Rows are contiguous in memory; stepping between rows uses the parent's
/// full row stride (`parent_cols`) so views over column ranges remain
/// aligned to the parent's row-major layout.
#[derive(Debug)]
pub struct MatView<'a, F> {
    data: &'a [F],
    parent_cols: usize,
    row_offset: usize,
    col_offset: usize,
    rows: usize,
    cols: usize,
}

impl<'a, F: FiniteField> MatView<'a, F> {
    /// Number of rows in the view.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns in the view.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Element at `(r, c)`, relative to the view's origin.
    ///
    /// # Panics
    ///
    /// Panics if `r >= self.rows()` or `c >= self.cols()`.
    pub fn get(&self, r: usize, c: usize) -> F {
        assert!(r < self.rows && c < self.cols, "MatView::get out of bounds");
        self.data[(self.row_offset + r) * self.parent_cols + self.col_offset + c].clone()
    }

    #[inline]
    pub(crate) fn row_slice(&self, r: usize) -> &[F] {
        assert!(
            r < self.rows,
            "MatView::row_slice index {} out of bounds (rows={})",
            r,
            self.rows
        );
        let start = (self.row_offset + r) * self.parent_cols + self.col_offset;
        &self.data[start..start + self.cols]
    }

    /// Materialises this view into a freshly allocated [`FieldMatrix<F>`].
    pub fn to_owned(&self) -> FieldMatrix<F> {
        #[cfg(test)]
        FIELDMATRIX_NEW_COUNT.with(|c| c.set(c.get() + 1));
        if self.rows == 0 || self.cols == 0 {
            return FieldMatrix {
                rows: self.rows,
                cols: self.cols,
                data: FieldVec::new(),
            };
        }
        let mut data = FieldVec::with_capacity(self.rows * self.cols);
        for r in 0..self.rows {
            for c in 0..self.cols {
                let idx = (self.row_offset + r) * self.parent_cols + self.col_offset + c;
                data.push(self.data[idx].clone());
            }
        }
        FieldMatrix {
            rows: self.rows,
            cols: self.cols,
            data,
        }
    }

    /// Returns a sub-view restricted to the rectangle `(rows, cols)`.
    ///
    /// # Panics
    ///
    /// Panics if either range exceeds the view's dimensions.
    pub fn submat(
        &self,
        rows: impl RangeBounds<usize>,
        cols: impl RangeBounds<usize>,
    ) -> MatView<'_, F> {
        let (r0, r1) = resolve_range(rows, self.rows);
        let (c0, c1) = resolve_range(cols, self.cols);
        MatView {
            data: self.data,
            parent_cols: self.parent_cols,
            row_offset: self.row_offset + r0,
            col_offset: self.col_offset + c0,
            rows: r1 - r0,
            cols: c1 - c0,
        }
    }
}

impl<F: FiniteField> MatrixLike<F> for MatView<'_, F> {
    type Owned = FieldMatrix<F>;

    #[inline]
    fn rows(&self) -> usize {
        self.rows
    }

    #[inline]
    fn cols(&self) -> usize {
        self.cols
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> F {
        MatView::get(self, row, col)
    }

    fn transpose(&self) -> FieldMatrix<F> {
        // A MatView borrows a row-major slice; the transpose cannot be
        // expressed as another borrowed view without physically moving the
        // data. Materialise an owned FieldMatrix and return its transpose.
        self.to_owned().transpose()
    }
}

/// Zero-copy mutable submatrix view.
///
/// A `MatViewMut` borrows a rectangular window of a parent [`FieldMatrix`]
/// with exclusive write access. Like [`MatView`], rows are contiguous in
/// memory and stepping between rows uses the parent's row stride.
#[derive(Debug)]
pub struct MatViewMut<'a, F> {
    data: &'a mut [F],
    parent_cols: usize,
    row_offset: usize,
    col_offset: usize,
    rows: usize,
    cols: usize,
}

impl<'a, F: FiniteField> MatViewMut<'a, F> {
    /// Row count.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Column count.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Element at `(r, c)`, relative to the view's origin.
    ///
    /// # Panics
    ///
    /// Panics if `r >= self.rows()` or `c >= self.cols()`.
    pub fn get(&self, r: usize, c: usize) -> F {
        assert!(
            r < self.rows && c < self.cols,
            "MatViewMut::get out of bounds"
        );
        self.data[(self.row_offset + r) * self.parent_cols + self.col_offset + c].clone()
    }

    /// Writes `v` at `(r, c)`.
    ///
    /// # Panics
    ///
    /// Panics if `r >= self.rows()` or `c >= self.cols()`.
    pub fn set(&mut self, r: usize, c: usize, v: F) {
        assert!(
            r < self.rows && c < self.cols,
            "MatViewMut::set out of bounds"
        );
        let idx = (self.row_offset + r) * self.parent_cols + self.col_offset + c;
        self.data[idx] = v;
    }

    /// Fills every cell of the view with `value`.
    pub fn fill(&mut self, value: F) {
        for r in 0..self.rows {
            for c in 0..self.cols {
                let idx = (self.row_offset + r) * self.parent_cols + self.col_offset + c;
                self.data[idx] = value.clone();
            }
        }
    }

    /// Copies every entry of `src` into this view.
    ///
    /// # Panics
    ///
    /// Panics if `src.shape() != self.shape()`.
    pub fn assign(&mut self, src: &FieldMatrix<F>) {
        assert_eq!(src.rows(), self.rows, "assign: row count mismatch");
        assert_eq!(src.cols(), self.cols, "assign: col count mismatch");
        for r in 0..self.rows {
            for c in 0..self.cols {
                let idx = (self.row_offset + r) * self.parent_cols + self.col_offset + c;
                self.data[idx] = src.get(r, c);
            }
        }
    }

    /// Swaps rows `r1` and `r2` within the view.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of range for the view.
    pub fn swap_rows(&mut self, r1: usize, r2: usize) {
        assert!(r1 < self.rows && r2 < self.rows, "swap_rows out of bounds");
        if r1 == r2 {
            return;
        }
        for c in 0..self.cols {
            let i1 = (self.row_offset + r1) * self.parent_cols + self.col_offset + c;
            let i2 = (self.row_offset + r2) * self.parent_cols + self.col_offset + c;
            self.data.swap(i1, i2);
        }
    }

    /// Materialises this mutable view into a freshly allocated
    /// [`FieldMatrix<F>`].
    pub fn to_owned(&self) -> FieldMatrix<F> {
        #[cfg(test)]
        FIELDMATRIX_NEW_COUNT.with(|c| c.set(c.get() + 1));
        if self.rows == 0 || self.cols == 0 {
            return FieldMatrix {
                rows: self.rows,
                cols: self.cols,
                data: FieldVec::new(),
            };
        }
        let mut data = FieldVec::with_capacity(self.rows * self.cols);
        for r in 0..self.rows {
            for c in 0..self.cols {
                let idx = (self.row_offset + r) * self.parent_cols + self.col_offset + c;
                data.push(self.data[idx].clone());
            }
        }
        FieldMatrix {
            rows: self.rows,
            cols: self.cols,
            data,
        }
    }

    /// Returns a mutable sub-view restricted to the rectangle `(rows, cols)`.
    ///
    /// # Panics
    ///
    /// Panics if either range exceeds the view's dimensions.
    pub fn submat_mut(
        &mut self,
        rows: impl RangeBounds<usize>,
        cols: impl RangeBounds<usize>,
    ) -> MatViewMut<'_, F> {
        let (r0, r1) = resolve_range(rows, self.rows);
        let (c0, c1) = resolve_range(cols, self.cols);
        MatViewMut {
            data: self.data,
            parent_cols: self.parent_cols,
            row_offset: self.row_offset + r0,
            col_offset: self.col_offset + c0,
            rows: r1 - r0,
            cols: c1 - c0,
        }
    }

    /// Returns an immutable sub-view restricted to `(rows, cols)`.
    ///
    /// # Panics
    ///
    /// Panics if either range exceeds the view's dimensions.
    pub fn submat(
        &self,
        rows: impl RangeBounds<usize>,
        cols: impl RangeBounds<usize>,
    ) -> MatView<'_, F> {
        let (r0, r1) = resolve_range(rows, self.rows);
        let (c0, c1) = resolve_range(cols, self.cols);
        MatView {
            data: self.data,
            parent_cols: self.parent_cols,
            row_offset: self.row_offset + r0,
            col_offset: self.col_offset + c0,
            rows: r1 - r0,
            cols: c1 - c0,
        }
    }

    /// Reborrows this mutable view as an immutable view over the same
    /// rectangle.
    pub fn as_view(&self) -> MatView<'_, F> {
        MatView {
            data: self.data,
            parent_cols: self.parent_cols,
            row_offset: self.row_offset,
            col_offset: self.col_offset,
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// Splits the view at row `mid`, consuming `self`, into disjoint mutable
    /// views `(top, bot)` over rows `0..mid` and `mid..rows`.
    ///
    /// # Panics
    ///
    /// Panics if `mid > self.rows()`.
    pub fn split_rows_mut(self, mid: usize) -> (MatViewMut<'a, F>, MatViewMut<'a, F>) {
        assert!(
            mid <= self.rows,
            "split_rows_mut: mid ({}) > rows ({})",
            mid,
            self.rows
        );
        let split_index = (self.row_offset + mid) * self.parent_cols;
        let (top_data, bot_data) = self.data.split_at_mut(split_index);
        let top = MatViewMut {
            data: top_data,
            parent_cols: self.parent_cols,
            row_offset: self.row_offset,
            col_offset: self.col_offset,
            rows: mid,
            cols: self.cols,
        };
        let bot = MatViewMut {
            data: bot_data,
            parent_cols: self.parent_cols,
            row_offset: 0,
            col_offset: self.col_offset,
            rows: self.rows - mid,
            cols: self.cols,
        };
        (top, bot)
    }

    /// Reborrows this mutable view with a shorter lifetime, so a callee can
    /// take a `MatViewMut` without consuming the outer borrow.
    pub fn reborrow(&mut self) -> MatViewMut<'_, F> {
        MatViewMut {
            data: self.data,
            parent_cols: self.parent_cols,
            row_offset: self.row_offset,
            col_offset: self.col_offset,
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// The parent backing slice with this view's
    /// `(parent_cols, row_offset, col_offset, rows, cols)`.
    #[doc(hidden)]
    #[inline]
    pub(crate) fn raw_parts_mut(&mut self) -> (&mut [F], usize, usize, usize, usize, usize) {
        (
            self.data,
            self.parent_cols,
            self.row_offset,
            self.col_offset,
            self.rows,
            self.cols,
        )
    }
}

impl<F: FiniteField> MatrixLike<F> for MatViewMut<'_, F> {
    type Owned = FieldMatrix<F>;

    #[inline]
    fn rows(&self) -> usize {
        self.rows
    }

    #[inline]
    fn cols(&self) -> usize {
        self.cols
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> F {
        MatViewMut::get(self, row, col)
    }

    fn transpose(&self) -> FieldMatrix<F> {
        // A `MatViewMut` borrows a row-major slice; the transpose cannot be
        // expressed as another borrowed view without physically moving the
        // data. Materialise an owned `FieldMatrix` and return its transpose.
        self.to_owned().transpose()
    }
}

impl<F: FiniteField> MatrixLikeMut<F> for MatViewMut<'_, F> {
    #[inline]
    fn set(&mut self, row: usize, col: usize, v: F) {
        MatViewMut::set(self, row, col, v);
    }

    #[inline]
    fn swap_rows(&mut self, r1: usize, r2: usize) {
        MatViewMut::swap_rows(self, r1, r2);
    }
}

/// Zero-copy strided view of a single column.
///
/// `ColView` is returned by [`FieldMatrix::col`] and borrows the parent's
/// backing slice with a stride of `parent.cols()`.
#[derive(Debug, Clone, Copy)]
pub struct ColView<'a, F> {
    data: &'a [F],
    start: usize,
    stride: usize,
    len: usize,
}

impl<'a, F: FiniteField> ColView<'a, F> {
    /// Number of elements in the column.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the column is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Element at position `i` along the column.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    pub fn get(&self, i: usize) -> F {
        assert!(i < self.len, "ColView::get index {} out of bounds", i);
        self.data[self.start + i * self.stride].clone()
    }

    /// Iterator over references to each column element.
    pub fn iter(&self) -> impl Iterator<Item = &'a F> {
        let data = self.data;
        let start = self.start;
        let stride = self.stride;
        (0..self.len).map(move |i| &data[start + i * stride])
    }
}

impl<F: FiniteField> Index<(usize, usize)> for FieldMatrix<F> {
    type Output = F;

    /// Read access `m[(r, c)]`. Panics on out-of-range indices.
    fn index(&self, (r, c): (usize, usize)) -> &F {
        assert!(
            r < self.rows && c < self.cols,
            "FieldMatrix index out of bounds"
        );
        self.data.get(r * self.cols + c)
    }
}

impl<F: FiniteField + fmt::Display> fmt::Display for FieldMatrix<F> {
    /// Formats the matrix with Unicode brackets matching
    /// [`BitMatrix::Display`](crate::matrix::BitMatrix).
    ///
    /// Each column is right-padded to the width of the widest element in
    /// that column so that entries line up vertically.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "[ ]");
        }
        let rendered: Vec<Vec<String>> = (0..self.rows)
            .map(|r| {
                (0..self.cols)
                    .map(|c| format!("{}", self.data.get(r * self.cols + c)))
                    .collect()
            })
            .collect();
        let col_widths: Vec<usize> = (0..self.cols)
            .map(|c| rendered.iter().map(|row| row[c].len()).max().unwrap_or(0))
            .collect();
        let border_width: usize =
            col_widths.iter().sum::<usize>() + (self.cols).max(1) /* interleaving spaces */ + 1;
        writeln!(f, "  ┌{}┐", " ".repeat(border_width))?;
        for row in &rendered {
            write!(f, "  │ ")?;
            for (c, cell) in row.iter().enumerate() {
                write!(f, "{:>w$}", cell, w = col_widths[c])?;
                if c < self.cols - 1 {
                    write!(f, " ")?;
                }
            }
            writeln!(f, " │")?;
        }
        write!(f, "  └{}┘", " ".repeat(border_width))
    }
}

// The `Add`, `Sub`, `Neg` and `Mul` operator overloads live in
// `crate::field::expr` and return proxy types, so `&a * &b + &c` fuses into
// one kernel call at evaluation.

/// Row-tile height for the blocked classical GEMM loops.
///
/// This is the compile-time selection site for the tuning-profile field
/// `gemm.row_tile`. The default build retains the conservative value; a
/// `gf2_tuning_baked` build uses the committed baked profile value.
pub(crate) const GEMM_ROW_TILE_DEFAULT: usize = 32;

#[cfg(not(gf2_tuning_baked))]
pub(crate) const GEMM_ROW_TILE: usize = GEMM_ROW_TILE_DEFAULT;

#[cfg(gf2_tuning_baked)]
pub(crate) const GEMM_ROW_TILE: usize = crate::tuning::baked::GEMM_ROW_TILE;

/// Column-tile width for the blocked classical GEMM loops.
///
/// This is the compile-time selection site for the tuning-profile field
/// `gemm.col_tile`; see [`GEMM_ROW_TILE`] for the selection mechanism.
pub(crate) const GEMM_COL_TILE_DEFAULT: usize = 64;

#[cfg(not(gf2_tuning_baked))]
pub(crate) const GEMM_COL_TILE: usize = GEMM_COL_TILE_DEFAULT;

#[cfg(gf2_tuning_baked)]
pub(crate) const GEMM_COL_TILE: usize = crate::tuning::baked::GEMM_COL_TILE;

/// Returns the unconditional conservative GEMM tile pair.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub const fn conservative_gemm_tiles() -> (usize, usize) {
    (GEMM_ROW_TILE_DEFAULT, GEMM_COL_TILE_DEFAULT)
}

/// Returns the cfg-selected GEMM tile pair used by ordinary production APIs.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub const fn selected_gemm_tiles() -> (usize, usize) {
    (GEMM_ROW_TILE, GEMM_COL_TILE)
}

/// Conservative default for the tuning-profile field
/// `gemm.axpy_fast_path_min_volume`: the smallest work volume `m · k · n` at
/// which [`gemm_axpy_into_view`] takes the whole-GEMM fast path.
pub(crate) const GEMM_AXPY_FAST_PATH_THRESHOLD: usize = 16 * 16 * 16;

/// The selected volume arm of the crate-private `gemm_axpy_into_view`
/// dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmAxpyRoute {
    /// Use the per-cell dot-product traversal.
    PerCell,
    /// Attempt the whole-GEMM field kernel before falling back per cell.
    WholeGemm,
}

/// Reports the volume arm of the crate-private `gemm_axpy_into_view`
/// dispatcher for an `m × k` by `k × n` product.
///
/// The comparison uses the active tuning-profile field
/// `gemm.axpy_fast_path_min_volume`. Field capability remains a separate
/// dispatcher condition: a field without a whole-GEMM kernel follows the
/// per-cell implementation even when this reporter returns
/// [`GemmAxpyRoute::WholeGemm`].
#[must_use]
pub fn gemm_axpy_route(m: usize, k: usize, n: usize) -> GemmAxpyRoute {
    gemm_axpy_route_resolved(
        crate::tuning::active().gemm().axpy_fast_path_min_volume(),
        m,
        k,
        n,
    )
}

fn gemm_axpy_route_resolved(
    axpy_fast_path_min_volume: usize,
    m: usize,
    k: usize,
    n: usize,
) -> GemmAxpyRoute {
    if m * k * n >= axpy_fast_path_min_volume {
        GemmAxpyRoute::WholeGemm
    } else {
        GemmAxpyRoute::PerCell
    }
}

/// A production blocked loop that consumes the GEMM tiles.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum GemmTileSite {
    /// The owning [`gemm`] kernel.
    MatrixGemm,
    /// The view-writing `gemm_into_view` kernel.
    MatrixGemmIntoView,
    /// The fused `gemm_axpy_into_view` kernel.
    MatrixGemmAxpyIntoView,
    /// The implicit-diagonal `gemm_axpy_into_view_diag` kernel.
    MatrixGemmAxpyIntoViewDiag,
    /// The expression evaluator's concrete GEMM-plus-beta kernel.
    ExprGemmWithBeta,
    /// The expression evaluator's concrete transposed-left GEMM kernel.
    ExprGemmTransA,
    /// The expression evaluator's concrete transposed-left fused kernel.
    ExprGemmTransAWithBeta,
}

#[cfg(not(any(test, feature = "test-support")))]
#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum GemmTileSite {
    MatrixGemm,
    MatrixGemmIntoView,
    MatrixGemmAxpyIntoView,
    MatrixGemmAxpyIntoViewDiag,
    ExprGemmWithBeta,
    ExprGemmTransA,
    ExprGemmTransAWithBeta,
}

#[cfg(any(test, feature = "test-support"))]
impl GemmTileSite {
    /// Every production blocked-loop selection site, in stable display order.
    pub const ALL: [Self; 7] = [
        Self::MatrixGemm,
        Self::MatrixGemmIntoView,
        Self::MatrixGemmAxpyIntoView,
        Self::MatrixGemmAxpyIntoViewDiag,
        Self::ExprGemmWithBeta,
        Self::ExprGemmTransA,
        Self::ExprGemmTransAWithBeta,
    ];
}

/// Compile-time policy for test-support execution observations.
///
/// The computational kernels are generic over this zero-sized policy so a
/// calibration timing adapter can instantiate the production body without
/// candidate-dependent atomic writes. Ordinary entry points use
/// [`RecordObservations`]; in builds without test support its methods compile
/// to no-ops.
pub(crate) trait ObservationPolicy {
    #[inline(always)]
    fn gemm_tiles(_site: GemmTileSite, _row_tile: usize, _col_tile: usize) {}

    #[inline(always)]
    fn gemm_axpy_route(_route: GemmAxpyRoute) {}

    #[inline(always)]
    fn triangular_route(_base_case_max_dim: usize, _route: usize) {}

    #[inline(always)]
    fn trsm_panel_rows(_rows: usize) {}

    #[inline(always)]
    fn ple_panel_cols(_cols: usize) {}

    #[inline(always)]
    fn ple_base_route(_scalar_base_max_cols: usize, _route: usize) {}
}

/// Observation policy used by ordinary public entry points.
pub(crate) struct RecordObservations;

impl ObservationPolicy for RecordObservations {
    #[inline(always)]
    fn gemm_tiles(site: GemmTileSite, row_tile: usize, col_tile: usize) {
        #[cfg(any(test, feature = "test-support"))]
        record_gemm_tiles(site, row_tile, col_tile);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = (site, row_tile, col_tile);
    }

    #[inline(always)]
    fn gemm_axpy_route(route: GemmAxpyRoute) {
        #[cfg(any(test, feature = "test-support"))]
        record_gemm_axpy_dispatch_route(route);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = route;
    }

    #[inline(always)]
    fn triangular_route(base_case_max_dim: usize, route: usize) {
        #[cfg(any(test, feature = "test-support"))]
        crate::field::triangular::record_triangular_route(base_case_max_dim, route);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = (base_case_max_dim, route);
    }

    #[inline(always)]
    fn trsm_panel_rows(rows: usize) {
        #[cfg(any(test, feature = "test-support"))]
        crate::field::triangular::record_trsm_panel_rows(rows);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = rows;
    }

    #[inline(always)]
    fn ple_panel_cols(cols: usize) {
        #[cfg(any(test, feature = "test-support"))]
        crate::field::ple::record_ple_panel_cols(cols);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = cols;
    }

    #[inline(always)]
    fn ple_base_route(scalar_base_max_cols: usize, route: usize) {
        #[cfg(any(test, feature = "test-support"))]
        crate::field::ple::record_ple_base_route(scalar_base_max_cols, route);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = (scalar_base_max_cols, route);
    }
}

/// Observation-free specialization used by calibration timing adapters.
#[cfg(any(test, feature = "test-support"))]
pub(crate) struct QuietObservations;

#[cfg(any(test, feature = "test-support"))]
impl ObservationPolicy for QuietObservations {}

/// One admissible compile-time GEMM tile pair for calibration.
///
/// Each value resolves to a monomorphized production body; installing a
/// runtime tuning profile does not select one of these baked extents.
/// Resolver methods are constant-time and do not panic. Returned functions
/// inherit the shape, field, and complexity contracts of their named GEMM
/// operations.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmTilePair {
    /// 16 rows by 32 columns.
    R16C32,
    /// 16 rows by 64 columns.
    R16C64,
    /// 16 rows by 128 columns.
    R16C128,
    /// 32 rows by 32 columns.
    R32C32,
    /// 32 rows by 64 columns.
    R32C64,
    /// 32 rows by 128 columns.
    R32C128,
    /// 64 rows by 32 columns.
    R64C32,
    /// 64 rows by 64 columns.
    R64C64,
    /// 64 rows by 128 columns.
    R64C128,
}

#[cfg(any(test, feature = "test-support"))]
impl GemmTilePair {
    /// All admissible pairs in protocol order.
    pub const ALL: [Self; 9] = [
        Self::R16C32,
        Self::R16C64,
        Self::R16C128,
        Self::R32C32,
        Self::R32C64,
        Self::R32C128,
        Self::R64C32,
        Self::R64C64,
        Self::R64C128,
    ];

    /// Returns the `(row_tile, col_tile)` pair.
    #[must_use]
    pub const fn extents(self) -> (usize, usize) {
        match self {
            Self::R16C32 => (16, 32),
            Self::R16C64 => (16, 64),
            Self::R16C128 => (16, 128),
            Self::R32C32 => (32, 32),
            Self::R32C64 => (32, 64),
            Self::R32C128 => (32, 128),
            Self::R64C32 => (64, 32),
            Self::R64C64 => (64, 64),
            Self::R64C128 => (64, 128),
        }
    }
}

/// A tile pair observed at one production blocked-loop site.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmTileObservation {
    /// Production loop that consumed the tile pair.
    pub site: GemmTileSite,
    /// Row-tile extent consumed by that loop.
    pub row_tile: usize,
    /// Column-tile extent consumed by that loop.
    pub col_tile: usize,
}

#[cfg(any(test, feature = "test-support"))]
static OBSERVED_GEMM_ROW_TILES: [AtomicUsize; 7] = [const { AtomicUsize::new(0) }; 7];

#[cfg(any(test, feature = "test-support"))]
static OBSERVED_GEMM_COL_TILES: [AtomicUsize; 7] = [const { AtomicUsize::new(0) }; 7];

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn record_gemm_tiles(site: GemmTileSite, row_tile: usize, col_tile: usize) {
    OBSERVED_GEMM_ROW_TILES[site as usize].store(row_tile, Ordering::Relaxed);
    OBSERVED_GEMM_COL_TILES[site as usize].store(col_tile, Ordering::Relaxed);
}

/// Clears all test-support observations of production GEMM tile reads.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_gemm_tile_observations() {
    for site in GemmTileSite::ALL {
        OBSERVED_GEMM_ROW_TILES[site as usize].store(0, Ordering::Relaxed);
        OBSERVED_GEMM_COL_TILES[site as usize].store(0, Ordering::Relaxed);
    }
}

/// Returns the tile pairs observed at production blocked-loop sites.
///
/// Only sites reached since [`reset_gemm_tile_observations`] are returned.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn gemm_tile_observations() -> Vec<GemmTileObservation> {
    GemmTileSite::ALL
        .into_iter()
        .filter_map(|site| {
            let row_tile = OBSERVED_GEMM_ROW_TILES[site as usize].load(Ordering::Relaxed);
            let col_tile = OBSERVED_GEMM_COL_TILES[site as usize].load(Ordering::Relaxed);
            (row_tile != 0 && col_tile != 0).then_some(GemmTileObservation {
                site,
                row_tile,
                col_tile,
            })
        })
        .collect()
}

#[cfg(any(test, feature = "test-support"))]
static LAST_GEMM_AXPY_DISPATCH_ROUTE: AtomicU8 = AtomicU8::new(0);

#[cfg(any(test, feature = "test-support"))]
fn record_gemm_axpy_dispatch_route(route: GemmAxpyRoute) {
    let value = match route {
        GemmAxpyRoute::PerCell => 1,
        GemmAxpyRoute::WholeGemm => 2,
    };
    LAST_GEMM_AXPY_DISPATCH_ROUTE.store(value, Ordering::Relaxed);
}

/// Clears the test-support observation of the last effective GEMM AXPY route.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_gemm_axpy_dispatch_route() {
    LAST_GEMM_AXPY_DISPATCH_ROUTE.store(0, Ordering::Relaxed);
}

/// Returns the route that completed the last crate-private
/// `gemm_axpy_into_view` dispatch.
///
/// [`GemmAxpyRoute::WholeGemm`] is reported only after the whole-GEMM kernel
/// returns successfully. A declined whole-GEMM attempt reports
/// [`GemmAxpyRoute::PerCell`] after its per-cell fallback completes.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_gemm_axpy_dispatch_route() -> Option<GemmAxpyRoute> {
    match LAST_GEMM_AXPY_DISPATCH_ROUTE.load(Ordering::Relaxed) {
        1 => Some(GemmAxpyRoute::PerCell),
        2 => Some(GemmAxpyRoute::WholeGemm),
        _ => None,
    }
}

/// Classical blocked product `A · B` with delayed reduction
/// (`@/citation/DumasPernet2012` §1.2): `B` is transposed once and each
/// output cell is a row·row dot product whose accumulation is chunked by
/// [`FiniteField::max_unreduced_additions`].
///
/// Field hooks may replace the whole product or the per-cell dot product;
/// the fallback is `dot_product_slices`.
///
/// # Panics
///
/// Panics if `a.cols() != b.rows()`. Also panics when the output is
/// non-empty, both factors are storage-empty (inner dimension 0) and
/// `F::zero_hint()` is `None`.
///
/// # Complexity
///
/// `O(m · k · n)` field multiplications and additions, plus one transposed
/// copy of `B`.
pub fn gemm<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    gemm_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, RecordObservations>(a, b)
}

fn gemm_tiled<
    F: FiniteField,
    const ROW_TILE: usize,
    const COL_TILE: usize,
    O: ObservationPolicy,
>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
) -> FieldMatrix<F> {
    assert_eq!(
        a.cols, b.rows,
        "FieldMatrix::mul: inner dimensions must match ({} vs {})",
        a.cols, b.rows
    );

    // Degenerate outer dimensions: output is empty in storage. This matches
    // `FieldMatrix::new(rows, 0, _)` and `FieldMatrix::new(0, cols, _)`, both
    // of which carry an empty `FieldVec`.
    if a.rows == 0 || b.cols == 0 {
        return FieldMatrix {
            rows: a.rows,
            cols: b.cols,
            data: FieldVec::new(),
        };
    }

    // The non-empty output needs a zero element: from whichever factor is
    // non-empty, else from `F::zero_hint()`.
    let zero: F = if !a.data.as_slice().is_empty() {
        a.data.as_slice()[0].zero_like()
    } else if !b.data.as_slice().is_empty() {
        b.data.as_slice()[0].zero_like()
    } else if let Some(z) = F::zero_hint() {
        z
    } else {
        panic!(
            "gemm: producing an m×n zero matrix from (m×0) * (0×n) is \
             ambiguous for runtime-context fields; use F: ConstField or \
             ensure at least one factor is non-empty"
        );
    };

    let mut out = FieldMatrix {
        rows: a.rows,
        cols: b.cols,
        data: FieldVec::zeros_from(a.rows * b.cols, &zero),
    };
    if a.cols == 0 {
        // No inner accumulation; the already-zero `out` is the result.
        return out;
    }

    // `dot_product_slices` chunks by `kmax`, so the `Wide` accumulator does
    // not overflow for any inner dimension.
    let kmax = F::max_unreduced_additions();
    debug_assert!(
        kmax == usize::MAX || a.cols <= kmax || kmax > 0,
        "gemm: delayed-reduction kmax invariant violated \
         (a.cols = {}, kmax = {})",
        a.cols,
        kmax
    );

    // Transpose B once so the inner dot product walks contiguous memory in
    // both operands. `b_t` is `b.cols × b.rows` row-major, so `b_t` row `j`
    // is exactly column `j` of `b`.
    let b_t = b.transpose();

    // Whole-product fast path: a field that packs both operands once and
    // runs its own inner kernel bypasses the per-cell loop below, its packing
    // amortising across the `O(m·k·n)` inner work. The default declines.
    if F::try_simd_gemm_classical(
        a.data.as_slice(),
        b_t.data.as_slice(),
        a.rows,
        a.cols,
        b.cols,
        out.data.as_mut_slice(),
    ) {
        return out;
    }

    let mut scratch_a = Vec::<u64>::new();
    let mut scratch_b = Vec::<u64>::new();
    let mut scratch_products = Vec::<u64>::new();

    // Both operands are packed to `u16` once per call.
    // `try_pack_fp_medium_u16` returns `None` for every field without the
    // packed kernel, which leaves the buffers unallocated.
    let inner = a.cols;
    let mut a_pack_buf: Vec<u16> = Vec::new();
    let mut b_pack_buf: Vec<u16> = Vec::new();
    let medium_pack_ok = F::try_pack_fp_medium_u16(a.data.as_slice(), &mut a_pack_buf).is_some()
        && F::try_pack_fp_medium_u16(b_t.data.as_slice(), &mut b_pack_buf).is_some();

    for i_blk in (0..a.rows).step_by(ROW_TILE) {
        let i_end = (i_blk + ROW_TILE).min(a.rows);
        for j_blk in (0..b.cols).step_by(COL_TILE) {
            let j_end = (j_blk + COL_TILE).min(b.cols);
            for i in i_blk..i_end {
                let a_row = &a.data.as_slice()[i * a.cols..(i + 1) * a.cols];
                let out_row = &mut out.data.as_mut_slice()[i * out.cols..(i + 1) * out.cols];
                for (j, out_cell) in out_row.iter_mut().enumerate().take(j_end).skip(j_blk) {
                    let b_col = &b_t.data.as_slice()[j * b_t.cols..(j + 1) * b_t.cols];
                    debug_assert_eq!(a_row.len(), b_col.len());
                    if let Some(value) = F::try_gf2m_u64_batch_dot_product(
                        a_row,
                        b_col,
                        &zero,
                        &mut scratch_a,
                        &mut scratch_b,
                        &mut scratch_products,
                    ) {
                        *out_cell = value;
                    } else if medium_pack_ok {
                        let a_packed = &a_pack_buf[i * inner..(i + 1) * inner];
                        let b_packed = &b_pack_buf[j * inner..(j + 1) * inner];
                        if let Some(value) = F::try_fp_simd_dot_packed_u16(a_packed, b_packed) {
                            *out_cell = value;
                            continue;
                        }
                        *out_cell = crate::field::vec::dot_product_slices(a_row, b_col, &zero);
                    } else {
                        *out_cell = crate::field::vec::dot_product_slices(a_row, b_col, &zero);
                    }
                }
            }
        }
    }
    O::gemm_tiles(GemmTileSite::MatrixGemm, ROW_TILE, COL_TILE);
    out
}

/// Function-pointer type for a monomorphized GEMM tile candidate.
#[cfg(any(test, feature = "test-support"))]
pub type GemmCandidateFn<F> = fn(&FieldMatrix<F>, &FieldMatrix<F>) -> FieldMatrix<F>;

#[cfg(any(test, feature = "test-support"))]
impl GemmTilePair {
    /// Resolves this pair to the shared production GEMM body.
    ///
    /// Resolve once before a timing loop; the returned function contains no
    /// runtime candidate branch.
    pub fn gemm_fn<F: FiniteField>(self) -> GemmCandidateFn<F> {
        match self {
            Self::R16C32 => gemm_tiled::<F, 16, 32, RecordObservations>,
            Self::R16C64 => gemm_tiled::<F, 16, 64, RecordObservations>,
            Self::R16C128 => gemm_tiled::<F, 16, 128, RecordObservations>,
            Self::R32C32 => gemm_tiled::<F, 32, 32, RecordObservations>,
            Self::R32C64 => gemm_tiled::<F, 32, 64, RecordObservations>,
            Self::R32C128 => gemm_tiled::<F, 32, 128, RecordObservations>,
            Self::R64C32 => gemm_tiled::<F, 64, 32, RecordObservations>,
            Self::R64C64 => gemm_tiled::<F, 64, 64, RecordObservations>,
            Self::R64C128 => gemm_tiled::<F, 64, 128, RecordObservations>,
        }
    }
}

/// Writes `A · B` into `out` by the blocked traversal of [`gemm`]. Its one
/// allocation is the transposed copy of `B`.
///
/// # Panics
///
/// Panics if `a.cols() != b.rows()`, `out.rows() != a.rows()`, or
/// `out.cols() != b.cols()`.
pub(crate) fn gemm_into_view<F, A, B>(a: &A, b: &B, out: MatViewMut<'_, F>)
where
    F: FiniteField,
    A: MatrixLike<F> + ?Sized,
    B: MatrixLike<F> + ?Sized,
{
    gemm_into_view_tiled::<F, A, B, GEMM_ROW_TILE, GEMM_COL_TILE, RecordObservations>(a, b, out);
}

pub(crate) fn gemm_into_view_tiled<
    F,
    A,
    B,
    const ROW_TILE: usize,
    const COL_TILE: usize,
    O: ObservationPolicy,
>(
    a: &A,
    b: &B,
    mut out: MatViewMut<'_, F>,
) where
    F: FiniteField,
    A: MatrixLike<F> + ?Sized,
    B: MatrixLike<F> + ?Sized,
{
    let (m, k) = a.shape();
    let (kb, n) = b.shape();
    assert_eq!(
        k, kb,
        "gemm_into_view: inner dimensions must match ({} vs {})",
        k, kb
    );
    assert_eq!(
        (m, n),
        (out.rows(), out.cols()),
        "gemm_into_view: output shape mismatch (expected {}×{}, got {}×{})",
        m,
        n,
        out.rows(),
        out.cols()
    );
    if m == 0 || n == 0 {
        return;
    }
    if k == 0 {
        // Empty inner dim: A·B is the zero matrix. Source a zero from
        // `out` itself (which is non-empty here).
        let zero = out.get(0, 0).zero_like();
        for i in 0..m {
            for j in 0..n {
                out.set(i, j, zero.clone());
            }
        }
        return;
    }
    let zero: F = a.get(0, 0).zero_like();
    let b_t = b.transpose();
    for i_blk in (0..m).step_by(ROW_TILE) {
        let i_end = (i_blk + ROW_TILE).min(m);
        for j_blk in (0..n).step_by(COL_TILE) {
            let j_end = (j_blk + COL_TILE).min(n);
            for i in i_blk..i_end {
                for j in j_blk..j_end {
                    let mut acc = zero.clone();
                    for t in 0..k {
                        acc += a.get(i, t) * b_t.get(j, t);
                    }
                    out.set(i, j, acc);
                }
            }
        }
    }
    O::gemm_tiles(GemmTileSite::MatrixGemmIntoView, ROW_TILE, COL_TILE);
}

/// Function-pointer type for the view-writing GEMM candidate adapter.
#[cfg(any(test, feature = "test-support"))]
pub type GemmIntoViewCandidateFn<F> = fn(&FieldMatrix<F>, &FieldMatrix<F>, &mut FieldMatrix<F>);

#[cfg(any(test, feature = "test-support"))]
fn gemm_into_view_candidate<F: FiniteField, const ROW_TILE: usize, const COL_TILE: usize>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    gemm_into_view_tiled::<F, _, _, ROW_TILE, COL_TILE, RecordObservations>(
        a,
        b,
        out.submat_mut(.., ..),
    );
}

#[cfg(any(test, feature = "test-support"))]
impl GemmTilePair {
    /// Resolves this pair to the shared view-writing GEMM body.
    pub fn gemm_into_view_fn<F: FiniteField>(self) -> GemmIntoViewCandidateFn<F> {
        match self {
            Self::R16C32 => gemm_into_view_candidate::<F, 16, 32>,
            Self::R16C64 => gemm_into_view_candidate::<F, 16, 64>,
            Self::R16C128 => gemm_into_view_candidate::<F, 16, 128>,
            Self::R32C32 => gemm_into_view_candidate::<F, 32, 32>,
            Self::R32C64 => gemm_into_view_candidate::<F, 32, 64>,
            Self::R32C128 => gemm_into_view_candidate::<F, 32, 128>,
            Self::R64C32 => gemm_into_view_candidate::<F, 64, 32>,
            Self::R64C64 => gemm_into_view_candidate::<F, 64, 64>,
            Self::R64C128 => gemm_into_view_candidate::<F, 64, 128>,
        }
    }
}

/// Fused kernel `out ← α · A · B + β · out`.
///
/// Each cell reads `out[i, j]` before writing it, so `out` serves as its own
/// `C` operand: `α = −1, β = 1` is the `trsm` update and `α = 1, β = 1` the
/// `trmm` update.
///
/// A field with `has_simd_gemm_classical()` takes its whole-GEMM kernel when
/// [`gemm_axpy_route`] reports [`GemmAxpyRoute::WholeGemm`]; a field whose
/// `try_pack_fp_medium_u16` accepts the operands takes packed per-cell dot
/// products at every volume; every other case uses `dot_product_slices`.
///
/// # Panics
///
/// Panics if `a.cols() != b.rows()` or
/// `out.shape() != (a.rows(), b.cols())`.
///
/// # Complexity
///
/// `O(m · k · n)` field multiplications, plus one transposed copy of `B`.
pub(crate) fn gemm_axpy_into_view<F>(
    alpha: F,
    a: &MatView<'_, F>,
    b: &MatView<'_, F>,
    beta: F,
    out: MatViewMut<'_, F>,
) where
    F: FiniteField,
{
    gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, RecordObservations>(
        alpha, a, b, beta, out,
    );
}

pub(crate) fn gemm_axpy_into_view_tiled<
    F,
    const ROW_TILE: usize,
    const COL_TILE: usize,
    O: ObservationPolicy,
>(
    alpha: F,
    a: &MatView<'_, F>,
    b: &MatView<'_, F>,
    beta: F,
    mut out: MatViewMut<'_, F>,
) where
    F: FiniteField,
{
    let (m, k) = (a.rows(), a.cols());
    let (kb, n) = (b.rows(), b.cols());
    assert_eq!(
        k, kb,
        "gemm_axpy_into_view: inner dimensions must match ({} vs {})",
        k, kb
    );
    assert_eq!(
        (m, n),
        (out.rows(), out.cols()),
        "gemm_axpy_into_view: output shape mismatch (expected {}×{}, got {}×{})",
        m,
        n,
        out.rows(),
        out.cols()
    );
    let route = gemm_axpy_route(m, k, n);
    if m == 0 || n == 0 {
        O::gemm_axpy_route(GemmAxpyRoute::PerCell);
        return;
    }
    if k == 0 {
        // Empty inner dim: A·B is the zero matrix, so out ← β · out.
        for i in 0..m {
            for j in 0..n {
                let v = beta.clone() * out.get(i, j);
                out.set(i, j, v);
            }
        }
        O::gemm_axpy_route(GemmAxpyRoute::PerCell);
        return;
    }
    let zero: F = a.get(0, 0).zero_like();
    // The owned transpose gives every kernel below a contiguous `B^T`.
    let b_t = b.transpose();

    // `has_simd_gemm_classical` is a non-allocating probe, so `a_flat` and
    // `scratch` are allocated only when the kernel can run.
    if F::has_simd_gemm_classical() && route == GemmAxpyRoute::WholeGemm {
        // `A` may be a strided sub-view; the kernel takes it contiguous.
        let mut a_flat: Vec<F> = Vec::with_capacity(m * k);
        for i in 0..m {
            a_flat.extend_from_slice(a.row_slice(i));
        }
        let mut scratch: Vec<F> = vec![zero.clone(); m * n];
        if F::try_simd_gemm_classical(&a_flat, b_t.data.as_slice(), m, k, n, &mut scratch) {
            for i in 0..m {
                let row_start = i * n;
                for j in 0..n {
                    let prod = scratch[row_start + j].clone();
                    let c_old = out.get(i, j);
                    out.set(i, j, alpha.clone() * prod + beta.clone() * c_old);
                }
            }
            O::gemm_axpy_route(GemmAxpyRoute::WholeGemm);
            return;
        }
        // The kernel declined; the per-cell loop below takes over.
    }

    // The packed per-cell path has no volume gate: `dot_product_slices`
    // fills fresh `u16` scratch vectors on every call, which packing once
    // per call avoids at any size.
    let mut a_contig_for_medium: Vec<F> = Vec::new();
    let mut a_pack_buf: Vec<u16> = Vec::new();
    let mut b_pack_buf: Vec<u16> = Vec::new();
    let medium_pack_ok = {
        // `B^T` is already contiguous, so packing it doubles as the
        // eligibility probe; `A` is copied and packed only when it accepts.
        if F::try_pack_fp_medium_u16(b_t.data.as_slice(), &mut b_pack_buf).is_some() {
            a_contig_for_medium.reserve(m * k);
            for i in 0..m {
                a_contig_for_medium.extend_from_slice(a.row_slice(i));
            }
            F::try_pack_fp_medium_u16(a_contig_for_medium.as_slice(), &mut a_pack_buf).is_some()
        } else {
            false
        }
    };

    for i_blk in (0..m).step_by(ROW_TILE) {
        let i_end = (i_blk + ROW_TILE).min(m);
        for j_blk in (0..n).step_by(COL_TILE) {
            let j_end = (j_blk + COL_TILE).min(n);
            for i in i_blk..i_end {
                let a_row = a.row_slice(i);
                debug_assert_eq!(a_row.len(), k);
                for j in j_blk..j_end {
                    let b_col = b_t.row(j);
                    debug_assert_eq!(b_col.len(), k);
                    let prod = if medium_pack_ok {
                        let a_packed = &a_pack_buf[i * k..(i + 1) * k];
                        let b_packed = &b_pack_buf[j * k..(j + 1) * k];
                        match F::try_fp_simd_dot_packed_u16(a_packed, b_packed) {
                            Some(v) => v,
                            None => crate::field::vec::dot_product_slices(a_row, b_col, &zero),
                        }
                    } else {
                        crate::field::vec::dot_product_slices(a_row, b_col, &zero)
                    };
                    let c_old = out.get(i, j);
                    out.set(i, j, alpha.clone() * prod + beta.clone() * c_old);
                }
            }
        }
    }
    O::gemm_tiles(GemmTileSite::MatrixGemmAxpyIntoView, ROW_TILE, COL_TILE);
    O::gemm_axpy_route(GemmAxpyRoute::PerCell);
}

/// Runs the production GEMM AXPY dispatcher as `out ← a · b` for
/// test-support route observation.
///
/// # Panics
///
/// Panics when either operand is empty, when their inner dimensions differ,
/// or when `out` does not have the product shape.
#[cfg(any(test, feature = "test-support"))]
pub fn run_gemm_axpy_dispatch_for_test<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    assert!(a.rows() > 0 && a.cols() > 0 && b.cols() > 0);
    let one = a.get(0, 0).one_like();
    let zero = one.zero_like();
    let a_view = a.submat(.., ..);
    let b_view = b.submat(.., ..);
    let out_view = out.submat_mut(.., ..);
    gemm_axpy_into_view(one, &a_view, &b_view, zero, out_view);
}

/// Function-pointer type for a monomorphized fused GEMM candidate.
#[cfg(any(test, feature = "test-support"))]
pub type GemmAxpyCandidateFn<F> = fn(F, &FieldMatrix<F>, &FieldMatrix<F>, F, &mut FieldMatrix<F>);

#[cfg(any(test, feature = "test-support"))]
fn gemm_axpy_candidate<F: FiniteField, const ROW_TILE: usize, const COL_TILE: usize>(
    alpha: F,
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    out: &mut FieldMatrix<F>,
) {
    gemm_axpy_into_view_tiled::<F, ROW_TILE, COL_TILE, RecordObservations>(
        alpha,
        &a.submat(.., ..),
        &b.submat(.., ..),
        beta,
        out.submat_mut(.., ..),
    );
}

#[cfg(any(test, feature = "test-support"))]
impl GemmTilePair {
    /// Resolves this pair to the shared fused GEMM body.
    pub fn gemm_axpy_fn<F: FiniteField>(self) -> GemmAxpyCandidateFn<F> {
        match self {
            Self::R16C32 => gemm_axpy_candidate::<F, 16, 32>,
            Self::R16C64 => gemm_axpy_candidate::<F, 16, 64>,
            Self::R16C128 => gemm_axpy_candidate::<F, 16, 128>,
            Self::R32C32 => gemm_axpy_candidate::<F, 32, 32>,
            Self::R32C64 => gemm_axpy_candidate::<F, 32, 64>,
            Self::R32C128 => gemm_axpy_candidate::<F, 32, 128>,
            Self::R64C32 => gemm_axpy_candidate::<F, 64, 32>,
            Self::R64C64 => gemm_axpy_candidate::<F, 64, 64>,
            Self::R64C128 => gemm_axpy_candidate::<F, 64, 128>,
        }
    }
}

/// Diagonal-handling flag for [`gemm_axpy_into_view_diag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitDiag {
    /// Operand storage holds the actual diagonal values.
    Stored,
    /// The diagonal is logically all-ones; the kernel never reads the
    /// `[i, i]` storage cell.
    Implicit,
}

/// Read-time wrapper that returns `F::one()` for `get(i, i)` on any
/// [`MatrixLike`] operand and forwards every other read.
pub(crate) struct UnitDiagView<'a, F, M: ?Sized> {
    inner: &'a M,
    one: F,
}

impl<'a, F: FiniteField, M: MatrixLike<F> + ?Sized> UnitDiagView<'a, F, M> {
    fn new(inner: &'a M, one: F) -> Self {
        Self { inner, one }
    }
}

impl<F: FiniteField, M: MatrixLike<F> + ?Sized> MatrixLike<F> for UnitDiagView<'_, F, M> {
    type Owned = M::Owned;

    #[inline]
    fn rows(&self) -> usize {
        self.inner.rows()
    }

    #[inline]
    fn cols(&self) -> usize {
        self.inner.cols()
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> F {
        if row == col {
            self.one.clone()
        } else {
            self.inner.get(row, col)
        }
    }

    fn transpose(&self) -> Self::Owned {
        // The transpose of the underlying storage: the implicit diagonal is
        // not synthesised here.
        self.inner.transpose()
    }
}

/// [`gemm_axpy_into_view`] for operands with implicit unit diagonals: under
/// `UnitDiag::Implicit` a diagonal read of that operand yields `F::one()`
/// and its storage cell is not read.
///
/// # Complexity
///
/// `O(m · k · n)` field operations, evaluated cell-wise through
/// `MatrixLike::get` with no transposed copy of `b`.
pub(crate) fn gemm_axpy_into_view_diag<F, A, B>(
    diag_a: UnitDiag,
    alpha: F,
    a: &A,
    diag_b: UnitDiag,
    b: &B,
    beta: F,
    out: MatViewMut<'_, F>,
) where
    F: FiniteField,
    A: MatrixLike<F> + ?Sized,
    B: MatrixLike<F> + ?Sized,
{
    gemm_axpy_into_view_diag_tiled::<F, A, B, GEMM_ROW_TILE, GEMM_COL_TILE, RecordObservations>(
        diag_a, alpha, a, diag_b, b, beta, out,
    );
}

pub(crate) fn gemm_axpy_into_view_diag_tiled<
    F,
    A,
    B,
    const ROW_TILE: usize,
    const COL_TILE: usize,
    O: ObservationPolicy,
>(
    diag_a: UnitDiag,
    alpha: F,
    a: &A,
    diag_b: UnitDiag,
    b: &B,
    beta: F,
    mut out: MatViewMut<'_, F>,
) where
    F: FiniteField,
    A: MatrixLike<F> + ?Sized,
    B: MatrixLike<F> + ?Sized,
{
    let (m, k) = (a.rows(), a.cols());
    let (kb, n) = (b.rows(), b.cols());
    assert_eq!(
        k, kb,
        "gemm_axpy_into_view_diag: inner dimensions must match ({} vs {})",
        k, kb
    );
    assert_eq!(
        (m, n),
        (out.rows(), out.cols()),
        "gemm_axpy_into_view_diag: output shape mismatch (expected {}×{}, got {}×{})",
        m,
        n,
        out.rows(),
        out.cols()
    );
    if m == 0 || n == 0 {
        return;
    }
    if k == 0 {
        // Empty inner dim: A·B is the zero matrix, so out ← β · out.
        for i in 0..m {
            for j in 0..n {
                let v = beta.clone() * out.get(i, j);
                out.set(i, j, v);
            }
        }
        return;
    }
    let zero: F = out.get(0, 0).zero_like();
    let one: F = zero.one_like();
    // A unit-diagonal operand has no contiguous slice for
    // `dot_product_slices`, so the dot product is evaluated eagerly.
    for i_blk in (0..m).step_by(ROW_TILE) {
        let i_end = (i_blk + ROW_TILE).min(m);
        for j_blk in (0..n).step_by(COL_TILE) {
            let j_end = (j_blk + COL_TILE).min(n);
            for i in i_blk..i_end {
                for j in j_blk..j_end {
                    let mut acc = zero.clone();
                    for kk in 0..k {
                        let a_val = if diag_a == UnitDiag::Implicit && i == kk {
                            one.clone()
                        } else {
                            a.get(i, kk)
                        };
                        let b_val = if diag_b == UnitDiag::Implicit && kk == j {
                            one.clone()
                        } else {
                            b.get(kk, j)
                        };
                        acc += a_val * b_val;
                    }
                    let c_old = out.get(i, j);
                    out.set(i, j, alpha.clone() * acc + beta.clone() * c_old);
                }
            }
        }
    }
    O::gemm_tiles(GemmTileSite::MatrixGemmAxpyIntoViewDiag, ROW_TILE, COL_TILE);
}

/// Function-pointer type for the implicit-diagonal GEMM candidate adapter.
#[cfg(any(test, feature = "test-support"))]
pub type GemmDiagCandidateFn<F> = fn(F, &FieldMatrix<F>, &FieldMatrix<F>, F, &mut FieldMatrix<F>);

#[cfg(any(test, feature = "test-support"))]
fn gemm_diag_candidate<F: FiniteField, const ROW_TILE: usize, const COL_TILE: usize>(
    alpha: F,
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    out: &mut FieldMatrix<F>,
) {
    gemm_axpy_into_view_diag_tiled::<F, _, _, ROW_TILE, COL_TILE, RecordObservations>(
        UnitDiag::Implicit,
        alpha,
        a,
        UnitDiag::Implicit,
        b,
        beta,
        out.submat_mut(.., ..),
    );
}

#[cfg(any(test, feature = "test-support"))]
impl GemmTilePair {
    /// Resolves this pair to the shared implicit-diagonal fused GEMM body.
    pub fn gemm_diag_fn<F: FiniteField>(self) -> GemmDiagCandidateFn<F> {
        match self {
            Self::R16C32 => gemm_diag_candidate::<F, 16, 32>,
            Self::R16C64 => gemm_diag_candidate::<F, 16, 64>,
            Self::R16C128 => gemm_diag_candidate::<F, 16, 128>,
            Self::R32C32 => gemm_diag_candidate::<F, 32, 32>,
            Self::R32C64 => gemm_diag_candidate::<F, 32, 64>,
            Self::R32C128 => gemm_diag_candidate::<F, 32, 128>,
            Self::R64C32 => gemm_diag_candidate::<F, 64, 32>,
            Self::R64C64 => gemm_diag_candidate::<F, 64, 64>,
            Self::R64C128 => gemm_diag_candidate::<F, 64, 128>,
        }
    }
}

#[allow(dead_code)]
pub(crate) fn unit_diag_view<F: FiniteField, M: MatrixLike<F> + ?Sized>(
    inner: &M,
    one: F,
) -> UnitDiagView<'_, F, M> {
    UnitDiagView::new(inner, one)
}

fn resolve_range(bounds: impl RangeBounds<usize>, upper: usize) -> (usize, usize) {
    let start = match bounds.start_bound() {
        Bound::Included(&s) => s,
        Bound::Excluded(&s) => s + 1,
        Bound::Unbounded => 0,
    };
    let end = match bounds.end_bound() {
        Bound::Included(&e) => e + 1,
        Bound::Excluded(&e) => e,
        Bound::Unbounded => upper,
    };
    assert!(
        start <= end,
        "range start ({}) must be <= end ({})",
        start,
        end
    );
    assert!(
        end <= upper,
        "range end ({}) exceeds upper bound ({})",
        end,
        upper
    );
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::{Gf2mElement, Gf2mField};
    use crate::gfp::Fp;
    use proptest::prelude::*;

    type F = Fp<7>;
    fn f(v: u64) -> F {
        Fp::<7>::new(v)
    }

    // GF(2^4) with primitive polynomial x^4 + x + 1 = 0b10011.
    fn gf16() -> Gf2mField {
        Gf2mField::new(4, 0b10011)
    }

    fn gf16_mat(field: &Gf2mField, values: &[&[u64]]) -> FieldMatrix<Gf2mElement> {
        let rows: Vec<FieldVec<Gf2mElement>> = values
            .iter()
            .map(|row| FieldVec::from(row.iter().map(|v| field.element(*v)).collect::<Vec<_>>()))
            .collect();
        FieldMatrix::from_rows(rows)
    }

    #[test]
    fn test_zeros_and_identity_construct_correctly() {
        let z = FieldMatrix::<F>::zeros(2, 3);
        assert_eq!(z.shape(), (2, 3));
        assert_eq!(z.get(1, 2), f(0));
        let id = FieldMatrix::<F>::identity(3);
        assert_eq!(id.get(2, 2), f(1));
        assert_eq!(id.get(0, 2), f(0));
    }

    #[test]
    fn test_with_capacity_honours_requested_shape() {
        let m = FieldMatrix::<F>::with_capacity(4, 5);
        assert_eq!(m.shape(), (4, 5));
        assert_eq!(m.rows(), 4);
        assert_eq!(m.cols(), 5);
        for r in 0..4 {
            for c in 0..5 {
                assert_eq!(m.get(r, c), f(0));
            }
        }
        let mut m = FieldMatrix::<F>::with_capacity(4, 5);
        m.set(3, 4, f(2));
        assert_eq!(m.get(3, 4), f(2));
    }

    #[test]
    fn test_set_and_get_round_trip() {
        let mut m = FieldMatrix::<F>::zeros(2, 2);
        m.set(0, 1, f(3));
        assert_eq!(m.get(0, 1), f(3));
        assert_eq!(m[(0, 1)], f(3));
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_get_out_of_bounds_panics() {
        let m = FieldMatrix::<F>::zeros(2, 2);
        let _ = m.get(3, 0);
    }

    #[test]
    fn test_row_and_col_views_expose_expected_elements() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(0, 1, f(2));
        m.set(1, 1, f(5));
        m.set(2, 1, f(4));
        let c = m.col(1);
        assert_eq!(c.len(), 3);
        assert_eq!(c.get(0), f(2));
        assert_eq!(c.get(2), f(4));
        let vs: Vec<_> = m.col_iter(1).collect();
        assert_eq!(vs.len(), 3);
        let r = m.row(1);
        assert_eq!(r.len(), 3);
        assert_eq!(r[1], f(5));
    }

    #[test]
    fn test_submat_view_bounds_are_tight() {
        let mut m = FieldMatrix::<F>::zeros(4, 4);
        m.set(1, 1, f(2));
        m.set(2, 2, f(3));
        let v = m.submat(1..=2, 1..=2);
        assert_eq!(v.rows(), 2);
        assert_eq!(v.cols(), 2);
        assert_eq!(v.get(0, 0), f(2));
        assert_eq!(v.get(1, 1), f(3));
    }

    #[test]
    fn test_submat_mut_fill_writes_only_window() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.submat_mut(0..2, 1..3).fill(f(5));
        assert_eq!(m.get(0, 0), f(0));
        assert_eq!(m.get(0, 1), f(5));
        assert_eq!(m.get(1, 2), f(5));
        assert_eq!(m.get(2, 2), f(0));
    }

    #[test]
    fn test_swap_rows_and_scale_row_row_ops() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(0, 0, f(1));
        m.set(1, 0, f(2));
        m.swap_rows(0, 1);
        assert_eq!(m.get(0, 0), f(2));
        assert_eq!(m.get(1, 0), f(1));
        m.scale_row(0, f(3));
        assert_eq!(m.get(0, 0), f(6));
    }

    #[test]
    fn test_axpy_row_applies_fma_to_target_row() {
        let mut m = FieldMatrix::<F>::zeros(2, 2);
        m.set(0, 0, f(1));
        m.set(1, 0, f(3));
        m.axpy_row(1, 0, f(2));
        assert_eq!(m.get(1, 0), f(3) + f(2) * f(1));
    }

    #[test]
    fn test_find_pivot_row_returns_first_nonzero() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(1, 1, f(2));
        assert_eq!(m.find_pivot_row(1, 0), Some(1));
        assert_eq!(m.find_pivot_row(2, 0), None);
    }

    #[test]
    fn test_transpose_and_diag_shapes_match() {
        let mut m = FieldMatrix::<F>::zeros(2, 3);
        m.set(0, 2, f(3));
        let t = m.transpose();
        assert_eq!(t.shape(), (3, 2));
        assert_eq!(t.get(2, 0), f(3));
        let id = FieldMatrix::<F>::identity(3);
        assert_eq!(id.diag().len(), 3);
        assert_eq!(id.trace(), f(3));
        assert!(id.is_symmetric());
    }

    #[test]
    fn test_matvec_identity_and_general_over_fp() {
        let id = FieldMatrix::<F>::identity(3);
        let x = FieldVec::from(vec![f(1), f(2), f(3)]);
        let y = id.matvec(&x);
        assert_eq!(y[0], f(1));
        assert_eq!(y[2], f(3));

        let mut m = FieldMatrix::<F>::zeros(2, 3);
        m.set(0, 0, f(1));
        m.set(0, 1, f(2));
        m.set(1, 2, f(3));
        let x = FieldVec::from(vec![f(1), f(1), f(2)]);
        let y = m.matvec(&x);
        assert_eq!(y[0], f(1) + f(2));
        assert_eq!(y[1], f(6));
        let yt = m.matvec_transpose(&FieldVec::from(vec![f(1), f(1)]));
        assert_eq!(yt[2], f(3));
    }

    #[test]
    fn test_matvec_over_gf2m_runtime_field() {
        let field = gf16();
        // 2x3 matrix
        //   [ α^1  α^2  0 ]
        //   [ 0    α^3  α^5 ]
        let m = gf16_mat(&field, &[&[2, 4, 0], &[0, 8, 6]]);
        let x = FieldVec::from(vec![field.element(3), field.element(5), field.element(7)]);
        let y = m.matvec(&x);
        // y[0] = 2·3 + 4·5 + 0·7, y[1] = 0·3 + 8·5 + 6·7, using GF(16) mul.
        let expected0 = field.element(2) * field.element(3) + field.element(4) * field.element(5);
        let expected1 = field.element(8) * field.element(5) + field.element(6) * field.element(7);
        assert_eq!(y[0], expected0);
        assert_eq!(y[1], expected1);
    }

    #[test]
    fn test_add_sub_neg_element_wise() {
        let mut a = FieldMatrix::<F>::zeros(2, 2);
        let mut b = FieldMatrix::<F>::zeros(2, 2);
        a.set(0, 0, f(5));
        b.set(0, 0, f(3));
        let sum: FieldMatrix<F> = (&a + &b).into();
        let diff: FieldMatrix<F> = &a - &b;
        let neg: FieldMatrix<F> = (-&a).into();
        assert_eq!(sum.get(0, 0), f(1));
        assert_eq!(diff.get(0, 0), f(2));
        assert_eq!(neg.get(0, 0), f(7 - 5));
    }

    #[test]
    fn test_mul_identity_returns_identity() {
        let a = FieldMatrix::<F>::identity(3);
        let b = FieldMatrix::<F>::identity(3);
        let c: FieldMatrix<F> = (&a * &b).into();
        assert_eq!(c, FieldMatrix::<F>::identity(3));
    }

    #[test]
    fn test_mul_rectangular_dimensions_match() {
        let mut a = FieldMatrix::<F>::zeros(2, 3);
        a.set(0, 0, f(1));
        a.set(0, 1, f(2));
        a.set(1, 2, f(3));
        let mut b = FieldMatrix::<F>::zeros(3, 2);
        b.set(0, 0, f(1));
        b.set(1, 1, f(4));
        b.set(2, 0, f(5));
        let c: FieldMatrix<F> = (&a * &b).into();
        assert_eq!(c.shape(), (2, 2));
        assert_eq!(c.get(0, 0), f(1));
        assert_eq!(c.get(0, 1), f(2) * f(4));
        assert_eq!(c.get(1, 0), f(3) * f(5));
    }

    #[test]
    fn test_mul_over_gf2m_runtime_field() {
        let field = gf16();
        let a = gf16_mat(&field, &[&[2, 4, 0], &[0, 8, 6]]);
        let b = gf16_mat(&field, &[&[1, 3], &[5, 0], &[0, 7]]);
        let c = crate::field::matrix::gemm(&a, &b);
        assert_eq!(c.shape(), (2, 2));
        // c[0][0] = 2·1 + 4·5 + 0·0.
        let expected_00 = field.element(2) * field.element(1) + field.element(4) * field.element(5);
        assert_eq!(c.get(0, 0), expected_00);
        // c[1][1] = 0·3 + 8·0 + 6·7.
        let expected_11 = field.element(6) * field.element(7);
        assert_eq!(c.get(1, 1), expected_11);
    }

    #[test]
    fn test_scalar_mul_both_sides_agree() {
        let mut a = FieldMatrix::<F>::zeros(2, 2);
        a.set(0, 1, f(3));
        let r1: FieldMatrix<F> = (&a * f(2)).into();
        let r2: FieldMatrix<F> = (f(2) * &a).into();
        assert_eq!(r1, r2);
        assert_eq!(r1.get(0, 1), f(6));
    }

    #[test]
    fn test_left_scalar_mul_fp_matches_right() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        a.set(0, 0, Fp::<7>::new(1));
        a.set(0, 1, Fp::<7>::new(3));
        a.set(1, 0, Fp::<7>::new(5));
        let k = Fp::<7>::new(4);
        let right_ref: FieldMatrix<Fp<7>> = (&a * k).into();
        let left_ref: FieldMatrix<Fp<7>> = (k * &a).into();
        assert_eq!(right_ref, left_ref);
        let right_owned: FieldMatrix<Fp<7>> = (a.clone() * k).into();
        let left_owned: FieldMatrix<Fp<7>> = (k * a.clone()).into();
        assert_eq!(right_owned, left_owned);
        assert_eq!(right_owned, right_ref);
    }

    #[test]
    fn test_left_scalar_mul_goldilocks_matches_right() {
        use crate::gfp::specialized::GoldilocksFp;
        let mut a = FieldMatrix::<GoldilocksFp>::zeros(2, 2);
        a.set(0, 0, GoldilocksFp::new(7));
        a.set(0, 1, GoldilocksFp::new(11));
        a.set(1, 1, GoldilocksFp::new(13));
        let k = GoldilocksFp::new(5);
        let right_ref: FieldMatrix<GoldilocksFp> = (&a * k).into();
        let left_ref: FieldMatrix<GoldilocksFp> = (k * &a).into();
        assert_eq!(right_ref, left_ref);
        let right_owned: FieldMatrix<GoldilocksFp> = (a.clone() * k).into();
        let left_owned: FieldMatrix<GoldilocksFp> = (k * a.clone()).into();
        assert_eq!(right_owned, left_owned);
        assert_eq!(right_owned, right_ref);
    }

    // GF(7²) and GF(7³) with simple non-residues.
    struct MatScalarQ7Cfg;
    impl crate::gfpn::ExtConfig for MatScalarQ7Cfg {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    struct MatScalarC7Cfg;
    impl crate::gfpn::ExtConfig for MatScalarC7Cfg {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    #[test]
    fn test_left_scalar_mul_quadratic_ext_matches_right() {
        use crate::gfpn::QuadraticExt;
        type Q = QuadraticExt<MatScalarQ7Cfg>;
        let a00 = Q::new(Fp::<7>::new(2), Fp::<7>::new(1));
        let a01 = Q::new(Fp::<7>::new(5), Fp::<7>::new(3));
        let mut a = FieldMatrix::<Q>::zeros(2, 2);
        a.set(0, 0, a00);
        a.set(0, 1, a01);
        let k = Q::new(Fp::<7>::new(4), Fp::<7>::new(6));
        let right_ref: FieldMatrix<Q> = (&a * k).into();
        let left_ref: FieldMatrix<Q> = (k * &a).into();
        assert_eq!(right_ref, left_ref);
        let right_owned: FieldMatrix<Q> = (a.clone() * k).into();
        let left_owned: FieldMatrix<Q> = (k * a.clone()).into();
        assert_eq!(right_owned, left_owned);
        assert_eq!(right_owned, right_ref);
    }

    #[test]
    fn test_left_scalar_mul_cubic_ext_matches_right() {
        use crate::gfpn::CubicExt;
        type C = CubicExt<MatScalarC7Cfg>;
        let a00 = C::new(Fp::<7>::new(2), Fp::<7>::new(1), Fp::<7>::new(0));
        let a01 = C::new(Fp::<7>::new(5), Fp::<7>::new(3), Fp::<7>::new(4));
        let mut a = FieldMatrix::<C>::zeros(2, 2);
        a.set(0, 0, a00);
        a.set(0, 1, a01);
        let k = C::new(Fp::<7>::new(4), Fp::<7>::new(6), Fp::<7>::new(2));
        let right_ref: FieldMatrix<C> = (&a * k).into();
        let left_ref: FieldMatrix<C> = (k * &a).into();
        assert_eq!(right_ref, left_ref);
        let right_owned: FieldMatrix<C> = (a.clone() * k).into();
        let left_owned: FieldMatrix<C> = (k * a.clone()).into();
        assert_eq!(right_owned, left_owned);
        assert_eq!(right_owned, right_ref);
    }

    // GF(2^4) with irreducible x^4 + x + 1: `MODULUS` stores the low m bits,
    // the leading term being implicit, hence `0b0011`.
    struct MatScalarGf2m4Cfg;
    impl crate::gf2m::Gf2mWideConfig<1> for MatScalarGf2m4Cfg {
        const M: usize = 4;
        const MODULUS: [u64; 1] = [0b0011];
        const NAME: &'static str = "MatScalarGf2m4Cfg";
    }

    #[test]
    fn test_left_scalar_mul_gf2m_wide_matches_right() {
        use crate::gf2m::Gf2mWide;
        type W = Gf2mWide<1, MatScalarGf2m4Cfg>;
        let a00 = W::new([0b0110]); // α^2 + α
        let a01 = W::new([0b1001]); // α^3 + 1
        let mut a = FieldMatrix::<W>::zeros(2, 2);
        a.set(0, 0, a00);
        a.set(0, 1, a01);
        let k = W::new([0b0011]); // α + 1
        let right_ref: FieldMatrix<W> = (&a * k).into();
        let left_ref: FieldMatrix<W> = (k * &a).into();
        assert_eq!(right_ref, left_ref);
        let right_owned: FieldMatrix<W> = (a.clone() * k).into();
        let left_owned: FieldMatrix<W> = (k * a.clone()).into();
        assert_eq!(right_owned, left_owned);
        assert_eq!(right_owned, right_ref);
    }

    #[test]
    fn test_right_scalar_mul_gf2m_element_generic() {
        use crate::matrix_like::MatrixLike;
        let field = gf16();
        let values: &[&[u64]] = &[&[1, 2, 3], &[4, 5, 6], &[7, 8, 9]];
        let m = gf16_mat(&field, values);
        let k = field.element(11); // arbitrary non-zero scalar

        let right_ref = &m * k.clone();
        let right_owned = m.clone() * k.clone();

        for (r, row) in values.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let expected = field.element(*v) * k.clone();
                assert_eq!(
                    <_ as MatrixLike<Gf2mElement>>::get(&right_ref, r, c),
                    expected
                );
                assert_eq!(
                    <_ as MatrixLike<Gf2mElement>>::get(&right_owned, r, c),
                    expected
                );
            }
        }

        assert_eq!(<_ as MatrixLike<Gf2mElement>>::shape(&right_ref), (3, 3));
        assert_eq!(<_ as MatrixLike<Gf2mElement>>::shape(&right_owned), (3, 3));
    }

    #[test]
    fn test_display_contains_corner_borders() {
        let m = FieldMatrix::<F>::identity(2);
        let s = format!("{}", m);
        assert!(s.contains('┌'));
        assert!(s.contains('└'));
    }

    #[test]
    fn test_matrixlike_trait_on_field_matrix_forwards_to_inherent() {
        let mut m = FieldMatrix::<F>::zeros(2, 2);
        <FieldMatrix<F> as MatrixLikeMut<F>>::set(&mut m, 1, 0, f(4));
        assert_eq!(<FieldMatrix<F> as MatrixLike<F>>::get(&m, 1, 0), f(4));
        assert_eq!(<FieldMatrix<F> as MatrixLike<F>>::shape(&m), (2, 2));
    }

    #[test]
    fn test_matrixlike_trait_on_matview_honours_window() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(1, 1, f(2));
        let v = m.submat(1..3, 1..3);
        assert_eq!(<MatView<F> as MatrixLike<F>>::rows(&v), 2);
        assert_eq!(<MatView<F> as MatrixLike<F>>::get(&v, 0, 0), f(2));
    }

    #[test]
    fn test_matview_transpose_materialises_owned() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(0, 1, f(2));
        m.set(1, 2, f(5));
        let v = m.submat(0..2, 0..3);
        let t: FieldMatrix<F> = <MatView<F> as MatrixLike<F>>::transpose(&v);
        assert_eq!(t.shape(), (3, 2));
        assert_eq!(t.get(1, 0), f(2));
        assert_eq!(t.get(2, 1), f(5));
    }

    #[test]
    fn test_matview_mut_transpose_materialises_owned() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(0, 1, f(2));
        let v = m.submat_mut(0..2, 0..2);
        let t: FieldMatrix<F> = <MatViewMut<F> as MatrixLike<F>>::transpose(&v);
        assert_eq!(t.shape(), (2, 2));
        assert_eq!(t.get(1, 0), f(2));
    }

    #[test]
    fn test_from_rows_roundtrip_preserves_entries() {
        let r0 = FieldVec::from(vec![f(1), f(2)]);
        let r1 = FieldVec::from(vec![f(3), f(4)]);
        let m = FieldMatrix::from_rows(vec![r0, r1]);
        assert_eq!(m.shape(), (2, 2));
        assert_eq!(m.get(1, 1), f(4));
    }

    #[test]
    fn test_to_sparse_emits_only_non_zero_entries() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(0, 0, f(1));
        m.set(1, 2, f(5));
        m.set(2, 1, f(3));
        let s = m.to_sparse();
        assert_eq!(s.shape(), (3, 3));
        assert_eq!(s.nnz(), 3);
        // CSR stores row-major; row_ptr marks row boundaries.
        let (row_ptr, col_idx, values) = s.as_raw_parts();
        assert_eq!(row_ptr, &[0, 1, 2, 3]);
        assert_eq!(col_idx, &[0, 2, 1]);
        assert_eq!(values, &[f(1), f(5), f(3)]);
    }

    #[test]
    fn test_to_sparse_empty_matrix_is_empty_sparse() {
        let m = FieldMatrix::<F>::zeros(0, 4);
        let s = m.to_sparse();
        assert_eq!(s.shape(), (0, 4));
        assert_eq!(s.nnz(), 0);
    }

    // Dimensions stay ≤ 6 to keep the `n³` proptest cases inside the
    // fast-tier budget.

    fn random_fp7_matrix(rows: usize, cols: usize, seed: u64) -> FieldMatrix<F> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<F>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, f(rng.gen::<u64>() % 7));
            }
        }
        m
    }

    fn random_gf2m_matrix_with_mask(
        field: &Gf2mField,
        rows: usize,
        cols: usize,
        seed: u64,
        mask: u64,
    ) -> FieldMatrix<Gf2mElement> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut data = FieldVec::with_capacity(rows * cols);
        for _ in 0..(rows * cols) {
            data.push(field.element(rng.gen::<u64>() & mask));
        }
        // Construct row-by-row via from_rows to avoid touching the
        // ConstField-only `zeros` constructor.
        let mut rows_vec: Vec<FieldVec<Gf2mElement>> = Vec::with_capacity(rows);
        let mut iter = data.into_iter();
        for _ in 0..rows {
            let mut row = FieldVec::with_capacity(cols);
            for _ in 0..cols {
                row.push(iter.next().unwrap());
            }
            rows_vec.push(row);
        }
        FieldMatrix::from_rows(rows_vec)
    }

    fn random_gf16_matrix(
        field: &Gf2mField,
        rows: usize,
        cols: usize,
        seed: u64,
    ) -> FieldMatrix<Gf2mElement> {
        random_gf2m_matrix_with_mask(field, rows, cols, seed, 0xF)
    }

    #[test]
    fn test_gemm_m_times_zero_times_zero_times_n_returns_zero_matrix() {
        let a = FieldMatrix::<F>::zeros(3, 0);
        let b = FieldMatrix::<F>::zeros(0, 2);
        let out: FieldMatrix<F> = (&a * &b).into();
        assert_eq!(out.rows(), 3);
        assert_eq!(out.cols(), 2);
        for r in 0..3 {
            for c in 0..2 {
                assert_eq!(out.get(r, c), f(0), "({}, {}) not zero", r, c);
            }
        }
    }

    #[test]
    fn test_gemm_empty_outer_dim_returns_empty_storage() {
        let field = gf16();
        let a_empty_rows = FieldMatrix::<Gf2mElement>::new(0, 3, field.element(0));
        let b = gf16_mat(&field, &[&[1, 2], &[3, 4], &[5, 6]]);
        let out1 = crate::field::matrix::gemm(&a_empty_rows, &b);
        assert_eq!(out1.rows(), 0);
        assert_eq!(out1.cols(), 2);

        let a = gf16_mat(&field, &[&[1, 2, 3], &[4, 5, 6]]);
        let b_empty_cols = FieldMatrix::<Gf2mElement>::new(3, 0, field.element(0));
        let out2 = crate::field::matrix::gemm(&a, &b_empty_cols);
        assert_eq!(out2.rows(), 2);
        assert_eq!(out2.cols(), 0);
    }

    #[test]
    fn test_gemm_panics_for_zero_inner_without_const_zero() {
        let field = gf16();
        let a = FieldMatrix::<Gf2mElement>::new(3, 0, field.element(0));
        let b = FieldMatrix::<Gf2mElement>::new(0, 2, field.element(0));
        let result = std::panic::catch_unwind(|| crate::field::matrix::gemm(&a, &b));
        assert!(result.is_err(), "expected gemm to panic");
        let payload = result.err().unwrap();
        let msg = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&'static str>()
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        assert!(
            msg.contains("ambiguous for runtime-context fields"),
            "unexpected panic message: {:?}",
            msg
        );
    }

    #[test]
    fn test_matvec_zero_cols_returns_zero_vector() {
        let a = FieldMatrix::<F>::zeros(3, 0);
        let x = FieldVec::<F>::new();
        let y = a.matvec(&x);
        assert_eq!(y.len(), 3);
        for i in 0..3 {
            assert_eq!(y[i], f(0));
        }
    }

    #[test]
    fn test_matvec_transpose_zero_rows_returns_zero_vector() {
        let a = FieldMatrix::<F>::zeros(0, 3);
        let x = FieldVec::<F>::new();
        let y = a.matvec_transpose(&x);
        assert_eq!(y.len(), 3);
        for i in 0..3 {
            assert_eq!(y[i], f(0));
        }
    }

    #[test]
    fn test_matvec_panics_for_non_const_zero_cols() {
        let field = gf16();
        let a = FieldMatrix::<Gf2mElement>::new(3, 0, field.element(0));
        let x = FieldVec::<Gf2mElement>::new();
        let result = std::panic::catch_unwind(|| a.matvec(&x));
        assert!(result.is_err(), "expected matvec to panic");
        let payload = result.err().unwrap();
        let msg = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&'static str>()
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        assert!(
            msg.contains("requires a zero witness"),
            "unexpected panic message: {:?}",
            msg
        );
    }

    #[test]
    fn test_matvec_transpose_panics_for_non_const_zero_rows() {
        let field = gf16();
        let a = FieldMatrix::<Gf2mElement>::new(0, 3, field.element(0));
        let x = FieldVec::<Gf2mElement>::new(); // length 0 == self.rows
        let result = std::panic::catch_unwind(|| a.matvec_transpose(&x));
        assert!(result.is_err(), "expected matvec_transpose to panic");
        let payload = result.err().unwrap();
        let msg = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&'static str>()
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        assert!(
            msg.contains("requires a zero witness"),
            "unexpected panic message: {:?}",
            msg
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn test_addition_is_commutative_over_fp7(
            rows in 1usize..=6,
            cols in 1usize..=6,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let a = random_fp7_matrix(rows, cols, seed_a);
            let b = random_fp7_matrix(rows, cols, seed_b);
            let ab: FieldMatrix<F> = (&a + &b).into();
            let ba: FieldMatrix<F> = (&b + &a).into();
            prop_assert_eq!(ab, ba);
        }

        #[test]
        fn test_addition_is_associative_over_fp7(
            rows in 1usize..=6,
            cols in 1usize..=6,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            seed_c in any::<u64>(),
        ) {
            let a = random_fp7_matrix(rows, cols, seed_a);
            let b = random_fp7_matrix(rows, cols, seed_b);
            let c = random_fp7_matrix(rows, cols, seed_c);
            let t1: FieldMatrix<F> = (&a + &b).into();
            let lhs: FieldMatrix<F> = (&t1 + &c).into();
            let t2: FieldMatrix<F> = (&b + &c).into();
            let rhs: FieldMatrix<F> = (&a + &t2).into();
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn test_mul_distributes_over_add_on_square_fp7(
            n in 1usize..=5,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            seed_c in any::<u64>(),
        ) {
            let a = random_fp7_matrix(n, n, seed_a);
            let b = random_fp7_matrix(n, n, seed_b);
            let c = random_fp7_matrix(n, n, seed_c);
            let bc: FieldMatrix<F> = (&b + &c).into();
            let lhs: FieldMatrix<F> = (&a * &bc).into();
            let ab: FieldMatrix<F> = (&a * &b).into();
            let ac: FieldMatrix<F> = (&a * &c).into();
            let rhs: FieldMatrix<F> = (&ab + &ac).into();
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn test_transpose_is_involution_over_fp7(
            rows in 1usize..=6,
            cols in 1usize..=6,
            seed in any::<u64>(),
        ) {
            let a = random_fp7_matrix(rows, cols, seed);
            prop_assert_eq!(a.transpose().transpose(), a);
        }

        #[test]
        fn test_identity_is_mul_identity_over_fp7(
            n in 1usize..=5,
            seed in any::<u64>(),
        ) {
            let a = random_fp7_matrix(n, n, seed);
            let id = FieldMatrix::<F>::identity(n);
            let aid: FieldMatrix<F> = (&a * &id).into();
            let ida: FieldMatrix<F> = (&id * &a).into();
            prop_assert_eq!(aid, a.clone());
            prop_assert_eq!(ida, a);
        }

        #[test]
        fn test_matvec_matches_matrix_times_column_over_fp7(
            rows in 1usize..=5,
            cols in 1usize..=5,
            seed_a in any::<u64>(),
            seed_x in any::<u64>(),
        ) {
            let a = random_fp7_matrix(rows, cols, seed_a);
            let x_mat = random_fp7_matrix(cols, 1, seed_x);
            let x_vec: FieldVec<F> =
                (0..cols).map(|i| x_mat.get(i, 0)).collect();
            let ax: FieldMatrix<F> = (&a * &x_mat).into();
            let y = a.matvec(&x_vec);
            for i in 0..rows {
                prop_assert_eq!(ax.get(i, 0), y[i]);
            }
        }

        #[test]
        fn test_addition_is_commutative_over_gf16(
            rows in 1usize..=6,
            cols in 1usize..=6,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let field = gf16();
            let a = random_gf16_matrix(&field, rows, cols, seed_a);
            let b = random_gf16_matrix(&field, rows, cols, seed_b);
            let ab = &a + &b;
            let ba = &b + &a;
            for r in 0..rows {
                for c in 0..cols {
                    prop_assert_eq!(
                        <_ as MatrixLike<Gf2mElement>>::get(&ab, r, c),
                        <_ as MatrixLike<Gf2mElement>>::get(&ba, r, c)
                    );
                }
            }
        }

        #[test]
        fn test_addition_is_associative_over_gf16(
            rows in 1usize..=6,
            cols in 1usize..=6,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            seed_c in any::<u64>(),
        ) {
            let field = gf16();
            let a = random_gf16_matrix(&field, rows, cols, seed_a);
            let b = random_gf16_matrix(&field, rows, cols, seed_b);
            let c = random_gf16_matrix(&field, rows, cols, seed_c);
            let ab = &a + &b;
            let bc = &b + &c;
            for r in 0..rows {
                for col in 0..cols {
                    let lhs = <_ as MatrixLike<Gf2mElement>>::get(&ab, r, col)
                        + <_ as MatrixLike<Gf2mElement>>::get(&c, r, col);
                    let rhs = <_ as MatrixLike<Gf2mElement>>::get(&a, r, col)
                        + <_ as MatrixLike<Gf2mElement>>::get(&bc, r, col);
                    prop_assert_eq!(lhs, rhs);
                }
            }
        }

        #[test]
        fn test_mul_distributes_over_add_on_square_gf16(
            n in 1usize..=5,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            seed_c in any::<u64>(),
        ) {
            let field = gf16();
            let a = random_gf16_matrix(&field, n, n, seed_a);
            let b = random_gf16_matrix(&field, n, n, seed_b);
            let c = random_gf16_matrix(&field, n, n, seed_c);
            let bc_proxy = &b + &c;
            let bc = gf16_mat_from_proxy(n, n, &bc_proxy);
            let a_bc = crate::field::matrix::gemm(&a, &bc);
            let ab = crate::field::matrix::gemm(&a, &b);
            let ac = crate::field::matrix::gemm(&a, &c);
            let ab_ac_proxy = &ab + &ac;
            for r in 0..n {
                for col in 0..n {
                    prop_assert_eq!(
                        a_bc.get(r, col),
                        <_ as MatrixLike<Gf2mElement>>::get(&ab_ac_proxy, r, col)
                    );
                }
            }
        }

        #[test]
        fn test_transpose_is_involution_over_gf16(
            rows in 1usize..=5,
            cols in 1usize..=5,
            seed in any::<u64>(),
        ) {
            let field = gf16();
            let a = random_gf16_matrix(&field, rows, cols, seed);
            prop_assert_eq!(a.transpose().transpose(), a);
        }
    }

    // The `From<Expr>` bridge is `ConstField`-only; this materialises a
    // proxy over `Gf2mElement`.
    fn gf16_mat_from_proxy<M: MatrixLike<Gf2mElement>>(
        rows: usize,
        cols: usize,
        m: &M,
    ) -> FieldMatrix<Gf2mElement> {
        let mut data = FieldVec::with_capacity(rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                data.push(m.get(r, c));
            }
        }
        FieldMatrix::from_raw_parts(rows, cols, data)
    }

    use crate::field::FiniteField;

    // GF(2^8) with the AES reduction polynomial x^8 + x^4 + x^3 + x + 1
    // (`@/citation/Nist2001`); `MODULUS` stores the low byte `0x1B`.
    struct MatGf2m8AesCfg;
    impl crate::gf2m::Gf2mWideConfig<1> for MatGf2m8AesCfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "MatGf2m8AesCfg";
    }
    type Gf2m8 = crate::gf2m::Gf2mWide<1, MatGf2m8AesCfg>;

    /// Naive triple-loop gemm reference: every multiply is reduced
    /// immediately, bypassing the `Wide` accumulator.
    fn naive_gemm<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
        assert_eq!(a.cols, b.rows);
        let m = a.rows;
        let n = b.cols;
        if m == 0 || n == 0 {
            return FieldMatrix {
                rows: m,
                cols: n,
                data: FieldVec::new(),
            };
        }
        let zero = if !a.data.as_slice().is_empty() {
            a.data.as_slice()[0].zero_like()
        } else if !b.data.as_slice().is_empty() {
            b.data.as_slice()[0].zero_like()
        } else {
            F::zero_hint().expect("naive_gemm: no zero witness")
        };
        let mut out = FieldMatrix {
            rows: m,
            cols: n,
            data: FieldVec::zeros_from(m * n, &zero),
        };
        for i in 0..m {
            for j in 0..n {
                let mut acc = zero.clone();
                for k in 0..a.cols {
                    acc += a.get(i, k) * b.get(k, j);
                }
                out.set(i, j, acc);
            }
        }
        out
    }

    fn random_fp_matrix<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        if rows == 0 || cols == 0 {
            return FieldMatrix::<Fp<P>>::zeros(rows, cols);
        }
        let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
            }
        }
        m
    }

    fn random_gf2m8_matrix(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        if rows == 0 || cols == 0 {
            return FieldMatrix::<Gf2m8>::zeros(rows, cols);
        }
        let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
            }
        }
        m
    }

    fn random_gf2m_wide1_matrix<F>(rows: usize, cols: usize, seed: u64, mask: u64) -> FieldMatrix<F>
    where
        F: crate::field::ConstField + FromGf2mU64,
    {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<F>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, F::from_gf2m_u64(rng.gen::<u64>() & mask));
            }
        }
        m
    }

    trait FromGf2mU64 {
        fn from_gf2m_u64(value: u64) -> Self;
    }

    impl FromGf2mU64 for Gf2m8 {
        fn from_gf2m_u64(value: u64) -> Self {
            Gf2m8::from_u64(value)
        }
    }

    #[test]
    fn test_gemm_matches_naive_fp7_small() {
        for (m, k, n) in [(1, 1, 1), (2, 3, 4), (5, 5, 5), (7, 13, 3)] {
            let a = random_fp_matrix::<7>(m, k, 0xA1 ^ (m * k * n) as u64);
            let b = random_fp_matrix::<7>(k, n, 0xB2 ^ (m * k * n) as u64);
            let got: FieldMatrix<Fp<7>> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_matches_naive_fp65521() {
        for (m, k, n) in [(1, 1, 1), (3, 5, 2), (7, 11, 5)] {
            let a = random_fp_matrix::<65521>(m, k, 0xCAFEu64 ^ (m * k) as u64);
            let b = random_fp_matrix::<65521>(k, n, 0xBEEFu64 ^ (k * n) as u64);
            let got: FieldMatrix<Fp<65521>> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_matches_naive_fp_mersenne31() {
        const M31: u64 = 2_147_483_647;
        for (m, k, n) in [(1, 1, 1), (4, 6, 3), (5, 17, 5)] {
            let a = random_fp_matrix::<M31>(m, k, 0xD00Du64 ^ (m * k) as u64);
            let b = random_fp_matrix::<M31>(k, n, 0xE11Eu64 ^ (k * n) as u64);
            let got: FieldMatrix<Fp<M31>> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_matches_naive_gf2_8_const() {
        for (m, k, n) in [(1, 1, 1), (3, 5, 2), (7, 11, 5)] {
            let a = random_gf2m8_matrix(m, k, 0xF00Du64 ^ (m * k) as u64);
            let b = random_gf2m8_matrix(k, n, 0x1234u64 ^ (k * n) as u64);
            let got: FieldMatrix<Gf2m8> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_matches_naive_gf2_16_const() {
        struct MatGf2m16Cfg;
        impl crate::gf2m::Gf2mWideConfig<1> for MatGf2m16Cfg {
            const M: usize = 16;
            // x^16 + x^12 + x^3 + x + 1 → low 16 bits of 0x11009 (= 0x1009
            // after stripping the implicit leading 1).
            const MODULUS: [u64; 1] = [0x1009];
            const NAME: &'static str = "MatGf2m16Cfg";
        }
        type Gf2m16 = crate::gf2m::Gf2mWide<1, MatGf2m16Cfg>;
        use rand::{Rng, SeedableRng};
        let mk_mat = |rows: usize, cols: usize, seed: u64| {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let mut m = FieldMatrix::<Gf2m16>::zeros(rows, cols);
            for r in 0..rows {
                for c in 0..cols {
                    m.set(r, c, Gf2m16::new([rng.gen::<u64>() & 0xFFFF]));
                }
            }
            m
        };
        for (m, k, n) in [(1, 1, 1), (3, 5, 2), (7, 11, 5)] {
            let a = mk_mat(m, k, 0xAAu64 ^ (m * k) as u64);
            let b = mk_mat(k, n, 0xBBu64 ^ (k * n) as u64);
            let got: FieldMatrix<Gf2m16> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gf2m_batch_gemm_matches_scalar_for_supported_degrees_and_boundaries() {
        struct MatGf2m16Cfg;
        impl crate::gf2m::Gf2mWideConfig<1> for MatGf2m16Cfg {
            const M: usize = 16;
            const MODULUS: [u64; 1] = [0x100B];
            const NAME: &'static str = "MatGf2m16Cfg";
        }
        type Gf2m16 = crate::gf2m::Gf2mWide<1, MatGf2m16Cfg>;
        impl FromGf2mU64 for Gf2m16 {
            fn from_gf2m_u64(value: u64) -> Self {
                Gf2m16::from_u64(value)
            }
        }

        struct MatGf2m32Cfg;
        impl crate::gf2m::Gf2mWideConfig<1> for MatGf2m32Cfg {
            const M: usize = 32;
            const MODULUS: [u64; 1] = [0x0040_0007];
            const NAME: &'static str = "MatGf2m32Cfg";
        }
        type Gf2m32 = crate::gf2m::Gf2mWide<1, MatGf2m32Cfg>;
        impl FromGf2mU64 for Gf2m32 {
            fn from_gf2m_u64(value: u64) -> Self {
                Gf2m32::from_u64(value)
            }
        }

        fn check<F>(label: &str, mask: u64)
        where
            F: crate::field::ConstField + FromGf2mU64,
        {
            for (m, k, n) in [
                (GEMM_ROW_TILE - 1, 8, GEMM_COL_TILE - 1),
                (GEMM_ROW_TILE, 8, GEMM_COL_TILE),
                (GEMM_ROW_TILE + 1, 8, GEMM_COL_TILE + 1),
                (17, 32, 65),
                (65, 32, 17),
            ] {
                let seed = 0x577B_9E7Fu64 ^ ((m as u64) << 32) ^ ((k as u64) << 16) ^ n as u64;
                let a = random_gf2m_wide1_matrix::<F>(m, k, seed, mask);
                let b = random_gf2m_wide1_matrix::<F>(k, n, seed.rotate_left(17), mask);
                let got = gemm(&a, &b);
                assert_eq!(got, naive_gemm(&a, &b), "{label}: {m}x{k}x{n}");
            }
        }

        check::<Gf2m8>("GF(2^8)", 0xFF);
        check::<Gf2m16>("GF(2^16)", 0xFFFF);
        check::<Gf2m32>("GF(2^32)", 0xFFFF_FFFF);
    }

    #[test]
    fn test_runtime_gf2m_batch_gemm_matches_scalar_for_rectangular_shapes() {
        for (field, mask) in [
            (crate::gf2m::Gf2mField::gf256(), 0xFF),
            (crate::gf2m::Gf2mField::gf65536(), 0xFFFF),
        ] {
            for (m, k, n) in [(9, 8, 7), (7, 32, 9), (GEMM_ROW_TILE + 1, 8, 11)] {
                let a =
                    random_gf2m_matrix_with_mask(&field, m, k, 0xABCDu64 ^ (m * k) as u64, mask);
                let b =
                    random_gf2m_matrix_with_mask(&field, k, n, 0xDCBAu64 ^ (k * n) as u64, mask);
                let got = gemm(&a, &b);
                assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
            }
        }
    }

    #[test]
    fn test_gf2m_batch_gemm_covers_64c88ae4_rectangular_shapes() {
        struct RectGf2m16Cfg;
        impl crate::gf2m::Gf2mWideConfig<1> for RectGf2m16Cfg {
            const M: usize = 16;
            const MODULUS: [u64; 1] = [0x100B];
            const NAME: &'static str = "RectGf2m16Cfg";
        }
        type RectGf2m16 = crate::gf2m::Gf2mWide<1, RectGf2m16Cfg>;
        impl FromGf2mU64 for RectGf2m16 {
            fn from_gf2m_u64(value: u64) -> Self {
                RectGf2m16::from_u64(value)
            }
        }

        fn check<F>(label: &str, mask: u64)
        where
            F: crate::field::ConstField + FromGf2mU64,
        {
            for (rows, cols) in [(1024usize, 8usize), (1024, 32)] {
                let mut left = FieldMatrix::<F>::zeros(rows, rows);
                let mut right = FieldMatrix::<F>::zeros(rows, cols);
                let mut expected = FieldMatrix::<F>::zeros(rows, cols);

                for i in 0..rows {
                    let diagonal = F::from_gf2m_u64((((i as u64) * 13 + 1) & mask).max(1));
                    left.set(i, i, diagonal);
                    for j in 0..cols {
                        let value = F::from_gf2m_u64(
                            (((i as u64) * 17) ^ ((j as u64) * 29) ^ 0x577B_9E7F) & mask,
                        );
                        right.set(i, j, value);
                        expected.set(i, j, diagonal * value);
                    }
                }

                let got = gemm(&left, &right);
                assert_eq!(got, expected, "{label}: 1024x1024x{cols}");
            }
        }

        check::<Gf2m8>("GF(2^8)", 0xFF);
        check::<RectGf2m16>("GF(2^16)", 0xFFFF);
    }

    #[test]
    fn test_gemm_matches_naive_gf2m_element_runtime() {
        let field = gf16();
        for (m, k, n) in [(1, 1, 1), (2, 3, 4), (5, 5, 5)] {
            let a = random_gf16_matrix(&field, m, k, 0x42u64 ^ (m * k) as u64);
            let b = random_gf16_matrix(&field, k, n, 0x43u64 ^ (k * n) as u64);
            let got = crate::field::matrix::gemm(&a, &b);
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_block_boundary_crossing_fp7() {
        // Dims straddle the `GEMM_ROW_TILE` and `GEMM_COL_TILE` boundaries.
        let cases = [
            (GEMM_ROW_TILE - 1, 8, GEMM_COL_TILE - 1),
            (GEMM_ROW_TILE, 8, GEMM_COL_TILE),
            (GEMM_ROW_TILE + 1, 8, GEMM_COL_TILE + 1),
            (2 * GEMM_ROW_TILE, 3, 2 * GEMM_COL_TILE),
            (35, 7, 70),
        ];
        for (m, k, n) in cases {
            let a = random_fp_matrix::<7>(m, k, 0x77u64 ^ (m * n) as u64);
            let b = random_fp_matrix::<7>(k, n, 0x88u64 ^ (k * n) as u64);
            let got: FieldMatrix<Fp<7>> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_rectangular_extremes_fp7() {
        let m = 2;
        let k = 1001;
        let n = 2;
        let a = random_fp_matrix::<7>(m, k, 0x5A);
        let b = random_fp_matrix::<7>(k, n, 0xA5);
        let got: FieldMatrix<Fp<7>> = (&a * &b).into();
        assert_eq!(got, naive_gemm(&a, &b));
    }

    #[test]
    fn test_gemm_kmax_boundary_reduction_chunking() {
        // For this prime near 2^63, `kmax` is small enough for the inner
        // dimension to cross it. With all-ones operands, `out[0,0]` equals
        // the inner dimension only if no term is dropped.
        const P: u64 = 9_223_372_036_854_775_783;
        type Fpx = Fp<P>;
        let kmax = <Fpx as FiniteField>::max_unreduced_additions();
        assert!(kmax >= 1, "sanity: kmax must permit at least one product");
        assert!(
            kmax < 100,
            "sanity: this field should have a small kmax for the chunking path"
        );
        // Just above 2·kmax: three chunks, the last of size 1, covering the
        // `remaining.min(kmax)` clamp.
        let k_inner = 2 * kmax + 1;
        let mut a = FieldMatrix::<Fpx>::zeros(1, k_inner);
        let mut b = FieldMatrix::<Fpx>::zeros(k_inner, 1);
        for i in 0..k_inner {
            a.set(0, i, Fpx::new(1));
            b.set(i, 0, Fpx::new(1));
        }
        let out: FieldMatrix<Fpx> = (&a * &b).into();
        let expected = Fpx::new(k_inner as u64 % P);
        assert_eq!(out.get(0, 0), expected);

        // Exactly kmax products: the single-chunk path.
        let k_inner = kmax;
        let mut a = FieldMatrix::<Fpx>::zeros(1, k_inner);
        let mut b = FieldMatrix::<Fpx>::zeros(k_inner, 1);
        for i in 0..k_inner {
            a.set(0, i, Fpx::new(1));
            b.set(i, 0, Fpx::new(1));
        }
        let out: FieldMatrix<Fpx> = (&a * &b).into();
        assert_eq!(out.get(0, 0), Fpx::new(k_inner as u64 % P));
    }

    #[test]
    fn test_gemm_all_four_owned_ref_combos_agree() {
        let a = random_fp_matrix::<7>(3, 3, 0xABCD);
        let b = random_fp_matrix::<7>(3, 3, 0xDCBA);
        let r1: FieldMatrix<Fp<7>> = (&a * &b).into();
        let r2 = a.clone() * &b;
        let r3 = &a * b.clone();
        let r4 = a.clone() * b.clone();
        assert_eq!(r1, r2);
        assert_eq!(r1, r3);
        assert_eq!(r1, r4);
    }

    #[test]
    fn test_sub_all_four_owned_ref_combos_agree() {
        let a = random_fp_matrix::<7>(3, 4, 0x11);
        let b = random_fp_matrix::<7>(3, 4, 0x22);
        let r1: FieldMatrix<Fp<7>> = &a - &b;
        let r2 = a.clone() - &b;
        let r3 = &a - b.clone();
        let r4 = a.clone() - b.clone();
        assert_eq!(r1, r2);
        assert_eq!(r1, r3);
        assert_eq!(r1, r4);
    }

    #[test]
    fn test_add_all_four_owned_ref_combos_agree() {
        let a = random_fp_matrix::<7>(3, 4, 0x33);
        let b = random_fp_matrix::<7>(3, 4, 0x44);
        let r1: FieldMatrix<Fp<7>> = (&a + &b).into();
        let r2 = a.clone() + &b;
        let r3 = &a + b.clone();
        let r4 = a.clone() + b.clone();
        assert_eq!(r1, r2);
        assert_eq!(r1, r3);
        assert_eq!(r1, r4);
    }

    #[test]
    fn test_neg_owned_and_ref_agree() {
        let a = random_fp_matrix::<7>(3, 4, 0x55);
        let r_owned: FieldMatrix<Fp<7>> = (-a.clone()).into();
        let r_ref: FieldMatrix<Fp<7>> = (-&a).into();
        assert_eq!(r_owned, r_ref);
        let twice_neg: FieldMatrix<Fp<7>> = {
            let n1: FieldMatrix<Fp<7>> = (-&a).into();
            (-&n1).into()
        };
        assert_eq!(twice_neg, a);
    }

    #[test]
    fn test_indexing_matches_get() {
        let a = random_fp_matrix::<7>(3, 4, 0x66);
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(a[(r, c)], a.get(r, c));
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        #[test]
        fn prop_gemm_matches_naive_fp65521(
            m in 1usize..=6,
            k in 1usize..=10,
            n in 1usize..=6,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let a = random_fp_matrix::<65521>(m, k, seed_a);
            let b = random_fp_matrix::<65521>(k, n, seed_b);
            let got: FieldMatrix<Fp<65521>> = (&a * &b).into();
            prop_assert_eq!(got, naive_gemm(&a, &b));
        }

        #[test]
        fn prop_mul_is_associative_fp7(
            m in 1usize..=4,
            k in 1usize..=4,
            n in 1usize..=4,
            p in 1usize..=4,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            seed_c in any::<u64>(),
        ) {
            let a = random_fp_matrix::<7>(m, k, seed_a);
            let b = random_fp_matrix::<7>(k, n, seed_b);
            let c = random_fp_matrix::<7>(n, p, seed_c);
            let ab: FieldMatrix<Fp<7>> = (&a * &b).into();
            let bc: FieldMatrix<Fp<7>> = (&b * &c).into();
            let lhs: FieldMatrix<Fp<7>> = (&ab * &c).into();
            let rhs: FieldMatrix<Fp<7>> = (&a * &bc).into();
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn prop_mul_right_distributes_fp7(
            m in 1usize..=4,
            k in 1usize..=4,
            n in 1usize..=4,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            seed_c in any::<u64>(),
        ) {
            let a = random_fp_matrix::<7>(m, k, seed_a);
            let b = random_fp_matrix::<7>(m, k, seed_b);
            let c = random_fp_matrix::<7>(k, n, seed_c);
            let apb: FieldMatrix<Fp<7>> = (&a + &b).into();
            let lhs: FieldMatrix<Fp<7>> = (&apb * &c).into();
            let ac: FieldMatrix<Fp<7>> = (&a * &c).into();
            let bc: FieldMatrix<Fp<7>> = (&b * &c).into();
            let rhs: FieldMatrix<Fp<7>> = (&ac + &bc).into();
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn prop_gemm_matches_naive_gf2_8(
            m in 1usize..=5,
            k in 1usize..=8,
            n in 1usize..=5,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let a = random_gf2m8_matrix(m, k, seed_a);
            let b = random_gf2m8_matrix(k, n, seed_b);
            let got: FieldMatrix<Gf2m8> = (&a * &b).into();
            prop_assert_eq!(got, naive_gemm(&a, &b));
        }
    }

    const MERSENNE_31: u64 = 2_147_483_647;

    /// `a` with its `[i, i]` cells overwritten by `F::one()`.
    fn materialise_unit_diag<F: FiniteField>(a: &FieldMatrix<F>) -> FieldMatrix<F> {
        let mut out = a.clone();
        let one = a.get(0, 0).one_like();
        let n = out.rows().min(out.cols());
        for i in 0..n {
            out.set(i, i, one.clone());
        }
        out
    }

    /// Axpy reference: `out ← α · A · B + β · out` computed via naive gemm.
    fn axpy_reference<F: FiniteField>(
        alpha: F,
        a: &FieldMatrix<F>,
        b: &FieldMatrix<F>,
        beta: F,
        out: &FieldMatrix<F>,
    ) -> FieldMatrix<F> {
        let prod = naive_gemm(a, b);
        let m = a.rows();
        let n = b.cols();
        let mut acc = out.clone();
        for i in 0..m {
            for j in 0..n {
                let v = alpha.clone() * prod.get(i, j) + beta.clone() * out.get(i, j);
                acc.set(i, j, v);
            }
        }
        acc
    }

    struct AxpyDiagCase<F> {
        m: usize,
        k: usize,
        n: usize,
        alpha: F,
        beta: F,
        diag_a: UnitDiag,
        diag_b: UnitDiag,
        seed: u64,
    }

    fn check_axpy_diag_fp<const P: u64>(case: AxpyDiagCase<Fp<P>>) {
        // Under `UnitDiag::Implicit` the diagonal is poisoned, to certify
        // that the kernel does not read those cells.
        let AxpyDiagCase {
            m,
            k,
            n,
            alpha,
            beta,
            diag_a,
            diag_b,
            seed,
        } = case;
        let mut a = random_fp_matrix::<P>(m, k, seed);
        let mut b = random_fp_matrix::<P>(k, n, seed.wrapping_add(11));
        let out0 = random_fp_matrix::<P>(m, n, seed.wrapping_add(23));
        if diag_a == UnitDiag::Implicit {
            for d in 0..m.min(k) {
                a.set(
                    d,
                    d,
                    Fp::<P>::new((seed.wrapping_add(d as u64) % P).wrapping_add(2) % P),
                );
            }
        }
        if diag_b == UnitDiag::Implicit {
            for d in 0..k.min(n) {
                b.set(
                    d,
                    d,
                    Fp::<P>::new((seed.wrapping_add(d as u64 + 7) % P).wrapping_add(2) % P),
                );
            }
        }
        let mut got = out0.clone();
        gemm_axpy_into_view_diag(diag_a, alpha, &a, diag_b, &b, beta, got.submat_mut(.., ..));
        let a_ref = if diag_a == UnitDiag::Implicit {
            materialise_unit_diag(&a)
        } else {
            a.clone()
        };
        let b_ref = if diag_b == UnitDiag::Implicit {
            materialise_unit_diag(&b)
        } else {
            b.clone()
        };
        let want = axpy_reference(alpha, &a_ref, &b_ref, beta, &out0);
        assert_eq!(
            got, want,
            "gemm_axpy_into_view_diag m={} k={} n={}",
            m, k, n
        );
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_stored_matches_axpy_fp7() {
        for &(m, k, n) in &[(2usize, 2, 2), (3, 4, 5), (5, 5, 5), (7, 3, 11)] {
            check_axpy_diag_fp::<7>(AxpyDiagCase {
                m,
                k,
                n,
                alpha: f(2),
                beta: f(3),
                diag_a: UnitDiag::Stored,
                diag_b: UnitDiag::Stored,
                seed: 0xA0 + (m * k * n) as u64,
            });
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_implicit_b_fp7() {
        for &(m, k, n) in &[(2usize, 2, 2), (3, 5, 4), (4, 7, 7), (1, 5, 5)] {
            check_axpy_diag_fp::<7>(AxpyDiagCase {
                m,
                k,
                n,
                alpha: f(1),
                beta: f(0),
                diag_a: UnitDiag::Stored,
                diag_b: UnitDiag::Implicit,
                seed: 0xB0 + (m * k * n) as u64,
            });
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_implicit_a_fp7() {
        for &(m, k, n) in &[(3usize, 3, 4), (5, 5, 3), (7, 7, 7), (2, 5, 5)] {
            check_axpy_diag_fp::<7>(AxpyDiagCase {
                m,
                k,
                n,
                alpha: f(1),
                beta: f(2),
                diag_a: UnitDiag::Implicit,
                diag_b: UnitDiag::Stored,
                seed: 0xC0 + (m * k * n) as u64,
            });
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_both_implicit_fp7() {
        for &(m, k, n) in &[(3usize, 3, 3), (5, 5, 5), (7, 7, 7), (4, 4, 4)] {
            check_axpy_diag_fp::<7>(AxpyDiagCase {
                m,
                k,
                n,
                alpha: f(1),
                beta: f(1),
                diag_a: UnitDiag::Implicit,
                diag_b: UnitDiag::Implicit,
                seed: 0xD0 + (m * k * n) as u64,
            });
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_implicit_b_mersenne31() {
        for &(m, k, n) in &[(2usize, 5, 7), (8, 8, 8), (3, 11, 4), (5, 5, 5)] {
            check_axpy_diag_fp::<MERSENNE_31>(AxpyDiagCase {
                m,
                k,
                n,
                alpha: Fp::<MERSENNE_31>::new(7),
                beta: Fp::<MERSENNE_31>::new(0),
                diag_a: UnitDiag::Stored,
                diag_b: UnitDiag::Implicit,
                seed: 0xE0 + (m * k * n) as u64,
            });
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_implicit_b_gf2m8() {
        let cases: &[(usize, usize, usize)] =
            &[(2, 2, 2), (3, 5, 4), (4, 7, 7), (5, 5, 5), (8, 8, 3)];
        for &(m, k, n) in cases {
            let mut rng = {
                use rand::SeedableRng;
                rand::rngs::StdRng::seed_from_u64(0xF0 + (m * k * n) as u64)
            };
            let a = random_gf2m8_matrix(m, k, 0xF0 + (m * k * n) as u64);
            let mut b = random_gf2m8_matrix(k, n, 0xF1 + (m * k * n) as u64);
            for d in 0..k.min(n) {
                use rand::Rng;
                b.set(d, d, Gf2m8::new([(rng.gen::<u64>() & 0xFF).max(2)]));
            }
            let out0 = random_gf2m8_matrix(m, n, 0xF2 + (m * k * n) as u64);
            let alpha = Gf2m8::new([3]);
            let beta = Gf2m8::new([5]);
            let mut got = out0.clone();
            gemm_axpy_into_view_diag(
                UnitDiag::Stored,
                alpha,
                &a,
                UnitDiag::Implicit,
                &b,
                beta,
                got.submat_mut(.., ..),
            );
            let b_ref = materialise_unit_diag(&b);
            let want = axpy_reference(alpha, &a, &b_ref, beta, &out0);
            assert_eq!(
                got, want,
                "gemm_axpy_into_view_diag gf2m8 m={} k={} n={}",
                m, k, n
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_gemm_axpy_into_view_diag_stored_fp7(
            m in 1usize..=5,
            k in 1usize..=6,
            n in 1usize..=5,
            seed in any::<u64>(),
        ) {
            check_axpy_diag_fp::<7>(AxpyDiagCase {
                m, k, n,
                alpha: f(1), beta: f(0),
                diag_a: UnitDiag::Stored, diag_b: UnitDiag::Stored,
                seed,
            });
        }

        #[test]
        fn prop_gemm_axpy_into_view_diag_implicit_b_fp7(
            m in 1usize..=5,
            k in 1usize..=6,
            n in 1usize..=5,
            alpha_v in 0u64..7,
            beta_v in 0u64..7,
            seed in any::<u64>(),
        ) {
            check_axpy_diag_fp::<7>(AxpyDiagCase {
                m, k, n,
                alpha: f(alpha_v), beta: f(beta_v),
                diag_a: UnitDiag::Stored, diag_b: UnitDiag::Implicit,
                seed,
            });
        }

        #[test]
        fn prop_gemm_axpy_into_view_diag_implicit_a_mersenne31(
            m in 1usize..=5,
            k in 1usize..=6,
            n in 1usize..=5,
            seed in any::<u64>(),
        ) {
            check_axpy_diag_fp::<MERSENNE_31>(AxpyDiagCase {
                m, k, n,
                alpha: Fp::<MERSENNE_31>::new(1),
                beta: Fp::<MERSENNE_31>::new(0),
                diag_a: UnitDiag::Implicit, diag_b: UnitDiag::Stored,
                seed,
            });
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_zero_inner_dim_only_betas_out() {
        let m = 3;
        let n = 4;
        let a = random_fp_matrix::<7>(m, 0, 1);
        let b = random_fp_matrix::<7>(0, n, 2);
        let out0 = random_fp_matrix::<7>(m, n, 3);
        let mut got = out0.clone();
        let beta = f(2);
        gemm_axpy_into_view_diag(
            UnitDiag::Implicit,
            f(5),
            &a,
            UnitDiag::Implicit,
            &b,
            beta,
            got.submat_mut(.., ..),
        );
        let mut want = out0.clone();
        for r in 0..m {
            for c in 0..n {
                want.set(r, c, beta * out0.get(r, c));
            }
        }
        assert_eq!(got, want);
    }

    #[test]
    fn test_gemm_axpy_into_view_diag_empty_outer_dim() {
        let a = random_fp_matrix::<7>(0, 3, 1);
        let b = random_fp_matrix::<7>(3, 4, 2);
        let mut out = random_fp_matrix::<7>(0, 4, 3);
        gemm_axpy_into_view_diag(
            UnitDiag::Stored,
            f(1),
            &a,
            UnitDiag::Stored,
            &b,
            f(1),
            out.submat_mut(.., ..),
        );
        assert_eq!(out.shape(), (0, 4));
    }

    /// `a · b` through the scalar `Fp<P>` operators, bypassing every SIMD
    /// path.
    fn scalar_gemm_reference<const P: u64>(
        a: &FieldMatrix<Fp<P>>,
        b: &FieldMatrix<Fp<P>>,
    ) -> FieldMatrix<Fp<P>> {
        assert_eq!(a.cols, b.rows);
        let mut out = FieldMatrix::<Fp<P>>::zeros(a.rows, b.cols);
        for i in 0..a.rows {
            for j in 0..b.cols {
                let mut acc = Fp::<P>::new(0);
                for t in 0..a.cols {
                    acc += a.get(i, t) * b.get(t, j);
                }
                out.set(i, j, acc);
            }
        }
        out
    }

    fn check_small_prime_f32<const P: u64>() {
        const WORD_BOUNDARY_LENS: &[usize] = &[0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257];
        const F_PATH_EXTRA_LENS: &[usize] = &[32, 134, 268, 512, 1024];

        let lens: Vec<usize> = WORD_BOUNDARY_LENS
            .iter()
            .chain(F_PATH_EXTRA_LENS.iter())
            .copied()
            .collect();

        for &n in &lens {
            // Above n = 257, m and k shrink to n / 4 so the scalar reference
            // stays inside the per-test budget.
            let k = if n > 257 { n / 4 } else { n };
            let m = if n > 257 { n / 4 } else { n };

            let a = random_fp_matrix::<P>(m, k, 0xA5A5_A5A5 ^ n as u64);
            let b = random_fp_matrix::<P>(k, n, 0x5A5A_5A5A ^ n as u64);

            let got = gemm(&a, &b);
            let want = scalar_gemm_reference::<P>(&a, &b);

            assert_eq!(
                got.shape(),
                want.shape(),
                "F-path: shape mismatch P={P} n={n}",
            );
            for i in 0..m {
                for j in 0..n {
                    assert_eq!(
                        got.get(i, j),
                        want.get(i, j),
                        "F-path: cell mismatch P={P} n={n} i={i} j={j}",
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "slow: f32 GEMM correctness check over small prime p=7; scalar reference at n=1024 exceeds 5s"]
    fn check_small_prime_f32_p7() {
        check_small_prime_f32::<7>();
    }

    #[test]
    #[ignore = "slow: f32 GEMM correctness check over small prime p=31; scalar reference at n=1024 exceeds 5s"]
    fn check_small_prime_f32_p31() {
        check_small_prime_f32::<31>();
    }

    #[test]
    #[ignore = "slow: f32 GEMM correctness check over small prime p=251; scalar reference at n=1024 exceeds 5s"]
    fn check_small_prime_f32_p251() {
        check_small_prime_f32::<251>();
    }

    /// Scalar oracle for `gemm_axpy_into_view`:
    /// `out[i, j] := α · Σ a[i, t] · b[t, j] + β · out[i, j]` through
    /// `FiniteField` operators only.
    fn scalar_axpy_reference<const P: u64>(
        alpha: Fp<P>,
        a: &FieldMatrix<Fp<P>>,
        b: &FieldMatrix<Fp<P>>,
        beta: Fp<P>,
        out: &mut FieldMatrix<Fp<P>>,
    ) {
        assert_eq!(a.cols, b.rows);
        assert_eq!(out.rows, a.rows);
        assert_eq!(out.cols, b.cols);
        let m = a.rows;
        let n = b.cols;
        let k = a.cols;
        for i in 0..m {
            for j in 0..n {
                let mut acc = Fp::<P>::new(0);
                for t in 0..k {
                    acc += a.get(i, t) * b.get(t, j);
                }
                let c_old = out.get(i, j);
                out.set(i, j, alpha * acc + beta * c_old);
            }
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_fp251_small_prime_path() {
        const P: u64 = 251;
        for &n in &[16usize, 64, 256] {
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0x4019_5C09 ^ n as u64);
            let b = random_fp_matrix::<P>(k, n, 0x5C09_4019 ^ n as u64);
            let c = random_fp_matrix::<P>(m, n, 0xCCCC_DDDD ^ n as u64);

            let alpha = Fp::<P>::new(3);
            let beta = Fp::<P>::new(5);

            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );

            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);

            for i in 0..m {
                for j in 0..n {
                    assert_eq!(
                        got.get(i, j),
                        want.get(i, j),
                        "Fp<251> gemm_axpy_into_view cell mismatch at n={n} \
                         i={i} j={j} alpha=3 beta=5"
                    );
                }
            }
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_small_prime_boundary_lengths() {
        fn check<const P: u64>() {
            const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            for &n in LENS {
                if n == 0 {
                    let a = random_fp_matrix::<P>(2, 3, 0xA000 ^ n as u64);
                    let b = random_fp_matrix::<P>(3, 0, 0xB000 ^ n as u64);
                    let mut got = FieldMatrix::<Fp<P>>::zeros(2, 0);
                    gemm_axpy_into_view(
                        Fp::<P>::new(1),
                        &a.submat(.., ..),
                        &b.submat(.., ..),
                        Fp::<P>::new(1),
                        got.submat_mut(.., ..),
                    );
                    assert_eq!(got.shape(), (2, 0), "P={P} n=0 empty out shape");
                    continue;
                }
                // (α, β) pairs: trsm submul = (−1, 1), addmul = (1, 1),
                // identity-update = (0, 1), copy-overwrite = (1, 0).
                let pairs: &[(u64, u64)] = &[(P - 1, 1), (1, 1), (0, 1), (1, 0), (3, 5)];
                let m = n;
                let k = n;
                let a = random_fp_matrix::<P>(m, k, 0xA1A1 ^ (n as u64 * 1031));
                let b = random_fp_matrix::<P>(k, n, 0xB2B2 ^ (n as u64 * 2069));
                let c = random_fp_matrix::<P>(m, n, 0xC3C3 ^ (n as u64 * 4093));
                for &(av, bv) in pairs {
                    let alpha = Fp::<P>::new(av);
                    let beta = Fp::<P>::new(bv);
                    let mut got = c.clone();
                    gemm_axpy_into_view(
                        alpha,
                        &a.submat(.., ..),
                        &b.submat(.., ..),
                        beta,
                        got.submat_mut(.., ..),
                    );
                    let mut want = c.clone();
                    scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
                    for i in 0..m {
                        for j in 0..n {
                            assert_eq!(
                                got.get(i, j),
                                want.get(i, j),
                                "Fp<{P}> gemm_axpy_into_view mismatch at n={n} \
                                 alpha={av} beta={bv} i={i} j={j}"
                            );
                        }
                    }
                }
            }
        }
        check::<7>();
        check::<31>();
        check::<127>();
        check::<241>();
        check::<251>();
    }

    #[test]
    fn test_gemm_axpy_into_view_fp65521_medium_prime_boundary_lengths() {
        const P: u64 = 65521;
        const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
        for &n in LENS {
            if n == 0 {
                let a = random_fp_matrix::<P>(2, 3, 0xA0A0 ^ n as u64);
                let b = random_fp_matrix::<P>(3, 0, 0xB0B0 ^ n as u64);
                let mut got = FieldMatrix::<Fp<P>>::zeros(2, 0);
                gemm_axpy_into_view(
                    Fp::<P>::new(1),
                    &a.submat(.., ..),
                    &b.submat(.., ..),
                    Fp::<P>::new(1),
                    got.submat_mut(.., ..),
                );
                assert_eq!(got.shape(), (2, 0), "Fp<65521> n=0 empty out shape");
                continue;
            }
            let pairs: &[(u64, u64)] = &[(P - 1, 1), (1, 1), (0, 1), (1, 0), (7, 11)];
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0xA9A9 ^ (n as u64 * 1031));
            let b = random_fp_matrix::<P>(k, n, 0xB8B8 ^ (n as u64 * 2069));
            let c = random_fp_matrix::<P>(m, n, 0xC7C7 ^ (n as u64 * 4093));
            for &(av, bv) in pairs {
                let alpha = Fp::<P>::new(av);
                let beta = Fp::<P>::new(bv);
                let mut got = c.clone();
                gemm_axpy_into_view(
                    alpha,
                    &a.submat(.., ..),
                    &b.submat(.., ..),
                    beta,
                    got.submat_mut(.., ..),
                );
                let mut want = c.clone();
                scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
                for i in 0..m {
                    for j in 0..n {
                        assert_eq!(
                            got.get(i, j),
                            want.get(i, j),
                            "Fp<65521> gemm_axpy_into_view mismatch at n={n} \
                             alpha={av} beta={bv} i={i} j={j}"
                        );
                    }
                }
            }
        }
    }

    /// `A` is a strided sub-view whose parent holds non-zero values outside
    /// it; those must stay out of the product.
    #[test]
    fn test_gemm_axpy_into_view_fp251_strided_a() {
        const P: u64 = 251;
        let parent_a = random_fp_matrix::<P>(128, 128, 0xAB12_CD34);
        let a_view = parent_a.submat(32..96, 16..48);
        let m = a_view.rows();
        let k = a_view.cols();
        let n = 32;
        let b = random_fp_matrix::<P>(k, n, 0xDE56_F789);
        let c = random_fp_matrix::<P>(m, n, 0x9999_8888);
        let alpha = Fp::<P>::new(P - 1);
        let beta = Fp::<P>::new(1);
        let mut got = c.clone();
        gemm_axpy_into_view(
            alpha,
            &a_view,
            &b.submat(.., ..),
            beta,
            got.submat_mut(.., ..),
        );
        let mut a_contig = FieldMatrix::<Fp<P>>::zeros(m, k);
        for i in 0..m {
            for j in 0..k {
                a_contig.set(i, j, parent_a.get(32 + i, 16 + j));
            }
        }
        let mut want = c.clone();
        scalar_axpy_reference::<P>(alpha, &a_contig, &b, beta, &mut want);
        for i in 0..m {
            for j in 0..n {
                assert_eq!(
                    got.get(i, j),
                    want.get(i, j),
                    "strided A: cell mismatch at i={i} j={j} m={m} k={k} n={n}"
                );
            }
        }
    }

    proptest! {
        #[test]
        fn prop_gemm_axpy_into_view_fp251_matches_oracle(
            seed in 0u64..256,
            n_idx in 0usize..8,
            alpha_v in 0u64..251,
            beta_v in 0u64..251,
        ) {
            const P: u64 = 251;
            const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            let n = LENS[n_idx];
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0xA000 ^ seed);
            let b = random_fp_matrix::<P>(k, n, 0xB000 ^ seed);
            let c = random_fp_matrix::<P>(m, n, 0xC000 ^ seed);
            let alpha = Fp::<P>::new(alpha_v);
            let beta = Fp::<P>::new(beta_v);
            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );
            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
            for i in 0..m {
                for j in 0..n {
                    prop_assert_eq!(got.get(i, j), want.get(i, j));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_gemm_axpy_into_view_fp7_matches_oracle(
            seed in 0u64..256,
            n_idx in 0usize..8,
            alpha_v in 0u64..7,
            beta_v in 0u64..7,
        ) {
            const P: u64 = 7;
            const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            let n = LENS[n_idx];
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0xA100 ^ seed);
            let b = random_fp_matrix::<P>(k, n, 0xB100 ^ seed);
            let c = random_fp_matrix::<P>(m, n, 0xC100 ^ seed);
            let alpha = Fp::<P>::new(alpha_v);
            let beta = Fp::<P>::new(beta_v);
            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );
            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
            for i in 0..m {
                for j in 0..n {
                    prop_assert_eq!(got.get(i, j), want.get(i, j));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_gemm_axpy_into_view_fp31_matches_oracle(
            seed in 0u64..256,
            n_idx in 0usize..8,
            alpha_v in 0u64..31,
            beta_v in 0u64..31,
        ) {
            const P: u64 = 31;
            const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            let n = LENS[n_idx];
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0xA200 ^ seed);
            let b = random_fp_matrix::<P>(k, n, 0xB200 ^ seed);
            let c = random_fp_matrix::<P>(m, n, 0xC200 ^ seed);
            let alpha = Fp::<P>::new(alpha_v);
            let beta = Fp::<P>::new(beta_v);
            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );
            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
            for i in 0..m {
                for j in 0..n {
                    prop_assert_eq!(got.get(i, j), want.get(i, j));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_gemm_axpy_into_view_fp127_matches_oracle(
            seed in 0u64..256,
            n_idx in 0usize..8,
            alpha_v in 0u64..127,
            beta_v in 0u64..127,
        ) {
            const P: u64 = 127;
            const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            let n = LENS[n_idx];
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0xA300 ^ seed);
            let b = random_fp_matrix::<P>(k, n, 0xB300 ^ seed);
            let c = random_fp_matrix::<P>(m, n, 0xC300 ^ seed);
            let alpha = Fp::<P>::new(alpha_v);
            let beta = Fp::<P>::new(beta_v);
            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );
            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
            for i in 0..m {
                for j in 0..n {
                    prop_assert_eq!(got.get(i, j), want.get(i, j));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_gemm_axpy_into_view_fp241_matches_oracle(
            seed in 0u64..256,
            n_idx in 0usize..8,
            alpha_v in 0u64..241,
            beta_v in 0u64..241,
        ) {
            const P: u64 = 241;
            const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            let n = LENS[n_idx];
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0xA400 ^ seed);
            let b = random_fp_matrix::<P>(k, n, 0xB400 ^ seed);
            let c = random_fp_matrix::<P>(m, n, 0xC400 ^ seed);
            let alpha = Fp::<P>::new(alpha_v);
            let beta = Fp::<P>::new(beta_v);
            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );
            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
            for i in 0..m {
                for j in 0..n {
                    prop_assert_eq!(got.get(i, j), want.get(i, j));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_gemm_axpy_into_view_fp65521_matches_oracle(
            seed in 0u64..256,
            n_idx in 0usize..8,
            alpha_v in 0u64..65521,
            beta_v in 0u64..65521,
        ) {
            const P: u64 = 65521;
            const LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];
            let n = LENS[n_idx];
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0xA500 ^ seed);
            let b = random_fp_matrix::<P>(k, n, 0xB500 ^ seed);
            let c = random_fp_matrix::<P>(m, n, 0xC500 ^ seed);
            let alpha = Fp::<P>::new(alpha_v);
            let beta = Fp::<P>::new(beta_v);
            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );
            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
            for i in 0..m {
                for j in 0..n {
                    prop_assert_eq!(got.get(i, j), want.get(i, j));
                }
            }
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_mersenne31_simd_path() {
        const P: u64 = (1u64 << 31) - 1;
        // 16³ equals the conservative default of
        // `gemm.axpy_fast_path_min_volume`, which the inclusive dispatch
        // admits; no profile is installed in this binary.
        for &n in &[16usize, 64, 256] {
            let m = n;
            let k = n;
            let a = random_fp_matrix::<P>(m, k, 0x6A7D_4C8E ^ n as u64);
            let b = random_fp_matrix::<P>(k, n, 0x4C8E_6A7D ^ n as u64);
            let c = random_fp_matrix::<P>(m, n, 0x8E7D_4A6C ^ n as u64);

            let alpha = Fp::<P>::new(3);
            let beta = Fp::<P>::new(5);

            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );

            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);

            for i in 0..m {
                for j in 0..n {
                    assert_eq!(
                        got.get(i, j),
                        want.get(i, j),
                        "Fp<M31> gemm_axpy_into_view cell mismatch at n={n} \
                         i={i} j={j} alpha=3 beta=5"
                    );
                }
            }
        }
    }

    #[test]
    fn test_gemm_axpy_into_view_mersenne31_boundary_lengths() {
        const P: u64 = (1u64 << 31) - 1;
        // 8 is the AVX2 M31 lane width; boundary lengths around it.
        const LENS: &[usize] = &[0, 1, 7, 8, 9, 63, 64, 65];
        let pairs: &[(u64, u64)] = &[(P - 1, 1), (1, 1), (0, 1), (1, 0), (3, 5)];
        for &n in LENS {
            if n == 0 {
                let a = random_fp_matrix::<P>(2, 3, 0xBB00 ^ n as u64);
                let b = random_fp_matrix::<P>(3, 0, 0xCC00 ^ n as u64);
                let mut got = FieldMatrix::<Fp<P>>::zeros(2, 0);
                gemm_axpy_into_view(
                    Fp::<P>::new(1),
                    &a.submat(.., ..),
                    &b.submat(.., ..),
                    Fp::<P>::new(1),
                    got.submat_mut(.., ..),
                );
                assert_eq!(got.shape(), (2, 0), "M31 n=0 empty out shape");
                continue;
            }
            let m = n;
            let k = n;
            for &(av, bv) in pairs {
                let alpha = Fp::<P>::new(av);
                let beta = Fp::<P>::new(bv);
                let a = random_fp_matrix::<P>(m, k, 0xAA00 ^ n as u64 ^ av);
                let b = random_fp_matrix::<P>(k, n, 0xBB00 ^ n as u64 ^ av);
                let c = random_fp_matrix::<P>(m, n, 0xCC00 ^ n as u64 ^ av);
                let mut got = c.clone();
                gemm_axpy_into_view(
                    alpha,
                    &a.submat(.., ..),
                    &b.submat(.., ..),
                    beta,
                    got.submat_mut(.., ..),
                );
                let mut want = c.clone();
                scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
                for i in 0..m {
                    for j in 0..n {
                        assert_eq!(
                            got.get(i, j),
                            want.get(i, j),
                            "M31 boundary: n={n} alpha={av} beta={bv} i={i} j={j}"
                        );
                    }
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_gemm_axpy_into_view_mersenne31_matches_oracle(
            seed in 0u64..256,
            n_idx in 0usize..8,
            alpha_v in 0u64..((1u64 << 31) - 1),
            beta_v in 0u64..((1u64 << 31) - 1),
        ) {
            const P: u64 = (1u64 << 31) - 1;
            const LENS: &[usize] = &[0, 1, 7, 8, 9, 63, 64, 65];
            let n = LENS[n_idx];
            let m = n;
            let k = n;
            if n == 0 {
                return Ok(());
            }
            let a = random_fp_matrix::<P>(m, k, 0xA500 ^ seed);
            let b = random_fp_matrix::<P>(k, n, 0xB500 ^ seed);
            let c = random_fp_matrix::<P>(m, n, 0xC500 ^ seed);
            let alpha = Fp::<P>::new(alpha_v);
            let beta = Fp::<P>::new(beta_v);
            let mut got = c.clone();
            gemm_axpy_into_view(
                alpha,
                &a.submat(.., ..),
                &b.submat(.., ..),
                beta,
                got.submat_mut(.., ..),
            );
            let mut want = c.clone();
            scalar_axpy_reference::<P>(alpha, &a, &b, beta, &mut want);
            for i in 0..m {
                for j in 0..n {
                    prop_assert_eq!(got.get(i, j), want.get(i, j));
                }
            }
        }
    }

    #[test]
    fn test_transposed_rows_and_cols_methods() {
        let m = FieldMatrix::<F>::zeros(3, 7);
        let t = m.t();
        assert_eq!(t.rows(), 7);
        assert_eq!(t.cols(), 3);
    }

    #[test]
    fn test_row_range_and_col_range() {
        let m = FieldMatrix::<F>::identity(5);
        let rv = m.row_range(1..3);
        assert_eq!(rv.rows(), 2);
        assert_eq!(rv.cols(), 5);
        let cv = m.col_range(2..4);
        assert_eq!(cv.rows(), 5);
        assert_eq!(cv.cols(), 2);
    }

    #[test]
    fn test_swap_rows_same_row_is_noop() {
        let mut m = FieldMatrix::<F>::identity(3);
        m.swap_rows(1, 1);
        assert_eq!(m.get(1, 1), f(1));
    }

    #[test]
    fn test_axpy_row_zero_cols_is_noop() {
        let mut m = FieldMatrix::<F>::zeros(3, 0);
        m.axpy_row(0, 1, f(5));
        assert_eq!(m.shape(), (3, 0));
    }

    #[test]
    fn test_axpy_row_dst_eq_src_scales_in_place() {
        // row[dst] += factor * row[dst]  ⇔  row[dst] = (1 + factor) * row[dst].
        let mut m = FieldMatrix::<F>::zeros(2, 3);
        m.set(1, 0, f(2));
        m.set(1, 1, f(3));
        m.set(1, 2, f(1));
        // factor = 2, so row[1] = (1 + 2) * [2, 3, 1] = [6, 9, 3] mod 7 = [6, 2, 3]
        m.axpy_row(1, 1, f(2));
        assert_eq!(m.get(1, 0), f(6));
        assert_eq!(m.get(1, 1), f(2)); // 9 mod 7 = 2
        assert_eq!(m.get(1, 2), f(3));
    }

    #[test]
    fn test_axpy_row_dst_less_than_src_path() {
        let mut m = FieldMatrix::<F>::zeros(3, 2);
        m.set(0, 0, f(1));
        m.set(0, 1, f(2));
        m.set(2, 0, f(3));
        m.set(2, 1, f(4));
        // row[0] += 1 * row[2]  →  [1+3, 2+4] = [4, 6] mod 7
        m.axpy_row(0, 2, f(1));
        assert_eq!(m.get(0, 0), f(4));
        assert_eq!(m.get(0, 1), f(6));
    }

    #[test]
    fn test_find_pivot_row_out_of_bounds_returns_none() {
        let m = FieldMatrix::<F>::identity(3);
        assert_eq!(m.find_pivot_row(5, 0), None); // col OOB
        assert_eq!(m.find_pivot_row(0, 5), None); // start_row OOB
    }

    #[test]
    fn test_is_square_non_square_returns_false() {
        let rect = FieldMatrix::<F>::zeros(2, 3);
        assert!(!rect.is_square());
        let sq = FieldMatrix::<F>::zeros(3, 3);
        assert!(sq.is_square());
    }

    #[test]
    fn test_is_symmetric_non_square_returns_false() {
        let m = FieldMatrix::<F>::zeros(2, 3);
        assert!(!m.is_symmetric());
    }

    #[test]
    fn test_is_symmetric_asymmetric_matrix_returns_false() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(0, 1, f(1)); // m[0,1] = 1 but m[1,0] = 0 → asymmetric
        assert!(!m.is_symmetric());
    }

    #[cfg(feature = "rand")]
    #[test]
    fn test_random_seeded_is_deterministic() {
        let a = FieldMatrix::<F>::random_seeded(4, 5, 0xC0FFEE);
        let b = FieldMatrix::<F>::random_seeded(4, 5, 0xC0FFEE);
        assert_eq!(a, b);
        assert_eq!(a.shape(), (4, 5));
    }
}
