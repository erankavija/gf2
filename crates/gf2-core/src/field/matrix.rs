//! Dense row-major matrix over an arbitrary [`FiniteField`].
//!
//! [`FieldMatrix<F>`] is the finite-field companion of
//! [`BitMatrix`](crate::matrix::BitMatrix). The public surface mirrors
//! `BitMatrix` method-for-method and, where the underlying operation makes
//! sense over a general field, carries the same name. Armadillo-style
//! convenience constructors (`zeros`, `ones`, `identity`, `random`) are
//! provided for [`ConstField`] instantiations.
//!
//! # Storage
//!
//! Elements are held in a [`FieldVec<F>`] laid out row-major with stride
//! `cols`; element `(r, c)` lives at linear index `r * cols + c`.
//!
//! # Views
//!
//! [`MatView`], [`MatViewMut`], and [`ColView`] are zero-copy borrow-only
//! windows into a parent matrix. Views may span disjoint column blocks and
//! are thus described by `(row_offset, col_offset, rows, cols, stride)`.
//! They implement [`MatrixLike<F>`] so generic algorithms recurse into
//! submatrices without allocating.
//!
//! # Operator layer
//!
//! All four owned/ref combinations are provided for `Add`, `Sub`, and `Mul`,
//! plus `Neg`, scalar `F * &M` / `&M * F`, and `Index<(usize, usize)>`.
//! The `Mul` body is a classical O(n³) `gemm`; delayed-reduction dot-product
//! kernels and Strassen-Winograd are reserved for story `d48a3cfd`.

use std::fmt;
use std::ops::{Add, Bound, Index, Mul, Neg, RangeBounds, Sub};

use crate::field::{ConstField, FieldVec, FiniteField};
use crate::matrix_like::{MatrixLike, MatrixLikeMut};

// ─── Transposed proxy ─────────────────────────────────────────────────────────

/// Minimal lazy-transpose proxy.
///
/// This type is a *placeholder* for the expression-template layer designed
/// in issue `cdcebf6a` and implemented in `d48a3cfd`. For now it simply
/// wraps a reference and exposes `rows`/`cols` swapped; a future
/// `Evaluate<F>` impl will plug into fused `A·B + C` fgemm calls.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let t = a.t();
/// assert_eq!(t.rows(), 3);
/// assert_eq!(t.cols(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transposed<M>(pub M);

impl<F: FiniteField> Transposed<&FieldMatrix<F>> {
    /// Rows of the logically transposed matrix (i.e. columns of the backing matrix).
    pub fn rows(&self) -> usize {
        self.0.cols()
    }

    /// Columns of the logically transposed matrix (i.e. rows of the backing matrix).
    pub fn cols(&self) -> usize {
        self.0.rows()
    }
}

// ─── FieldMatrix ──────────────────────────────────────────────────────────────

/// Row-major dense matrix over a [`FiniteField`].
///
/// # Storage
///
/// Entries are stored row-major in a single [`FieldVec<F>`] of length
/// `rows * cols`. Element `(r, c)` lives at linear index `r * cols + c`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 3);
/// m.set(0, 1, Fp::<7>::new(4));
/// assert_eq!(m.get(0, 1), Fp::<7>::new(4));
/// assert_eq!(m.shape(), (2, 3));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMatrix<F: FiniteField> {
    rows: usize,
    cols: usize,
    data: FieldVec<F>,
}

// ─── Constructors ─────────────────────────────────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Creates an `rows × cols` matrix with every entry equal to `fill`.
    ///
    /// # Arguments
    ///
    /// * `rows` - Row count.
    /// * `cols` - Column count.
    /// * `fill` - Value used for every cell (cloned `rows * cols` times).
    ///
    /// # Complexity
    ///
    /// O(rows · cols) clones and one allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::new(2, 3, Fp::<7>::new(2));
    /// assert_eq!(m.get(1, 2), Fp::<7>::new(2));
    /// ```
    pub fn new(rows: usize, cols: usize, fill: F) -> Self {
        let data: FieldVec<F> = (0..rows * cols).map(|_| fill.clone()).collect();
        Self { rows, cols, data }
    }

    /// Builds a matrix by stacking `rows` into a rectangular shape.
    ///
    /// # Panics
    ///
    /// Panics if the input is empty or any row has a different length than
    /// the first row.
    ///
    /// # Complexity
    ///
    /// O(rows · cols).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, matrix::FieldMatrix};
    /// use gf2_core::gfp::Fp;
    ///
    /// let r0 = FieldVec::from(vec![Fp::<7>::new(1), Fp::<7>::new(2)]);
    /// let r1 = FieldVec::from(vec![Fp::<7>::new(3), Fp::<7>::new(4)]);
    /// let m = FieldMatrix::from_rows(vec![r0, r1]);
    /// assert_eq!(m.shape(), (2, 2));
    /// assert_eq!(m.get(1, 0), Fp::<7>::new(3));
    /// ```
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

    /// Allocates an uninitialised shell with capacity for `rows * cols`
    /// elements but length zero.
    ///
    /// Callers are responsible for filling the matrix before calling any
    /// access method; most callers prefer [`FieldMatrix::zeros`] or
    /// [`FieldMatrix::new`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::with_capacity(4, 4);
    /// assert_eq!(m.rows(), 0);
    /// assert_eq!(m.cols(), 0);
    /// ```
    pub fn with_capacity(rows: usize, cols: usize) -> Self {
        Self {
            rows: 0,
            cols: 0,
            data: FieldVec::with_capacity(rows * cols),
        }
    }
}

impl<F: ConstField> FieldMatrix<F> {
    /// Returns a `rows × cols` zero matrix.
    ///
    /// # Complexity
    ///
    /// O(rows · cols).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(3, 4);
    /// assert_eq!(m.shape(), (3, 4));
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(0));
    /// ```
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: FieldVec::zeros(rows * cols),
        }
    }

    /// Returns a `rows × cols` matrix filled with the multiplicative identity.
    ///
    /// # Complexity
    ///
    /// O(rows · cols).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::ones(2, 2);
    /// assert_eq!(m.get(1, 1), Fp::<7>::new(1));
    /// ```
    pub fn ones(rows: usize, cols: usize) -> Self {
        Self::new(rows, cols, F::one())
    }

    /// Returns the `n × n` identity matrix.
    ///
    /// # Complexity
    ///
    /// O(n²) to zero-fill plus O(n) to place the diagonal.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// assert_eq!(id.get(0, 0), Fp::<7>::new(1));
    /// assert_eq!(id.get(0, 1), Fp::<7>::new(0));
    /// ```
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, F::one());
        }
        m
    }
}

/// Enables `Fp<P>` to participate in generic random generation via
/// `rng.gen::<Fp<P>>()`. This local impl is narrow in scope (it lives next to
/// the matrix type that needs it) and avoids having to touch the `gfp`
/// module. `BitMatrix::random` uses a different, word-fill strategy.
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
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    /// use rand::SeedableRng;
    ///
    /// let mut rng = rand::rngs::StdRng::seed_from_u64(0xBAD5EED);
    /// let m = FieldMatrix::<Fp<7>>::random(4, 4, &mut rng);
    /// assert_eq!(m.shape(), (4, 4));
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows · cols).
    pub fn random<R: rand::Rng + ?Sized>(rows: usize, cols: usize, rng: &mut R) -> Self {
        let data: FieldVec<F> = (0..rows * cols).map(|_| rng.gen::<F>()).collect();
        Self { rows, cols, data }
    }

    /// Returns a `rows × cols` matrix from a seeded `StdRng`.
    ///
    /// Useful for reproducible tests and benchmarks.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")] {
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = FieldMatrix::<Fp<7>>::random_seeded(4, 4, 42);
    /// let b = FieldMatrix::<Fp<7>>::random_seeded(4, 4, 42);
    /// assert_eq!(a, b);
    /// # }
    /// ```
    pub fn random_seeded(rows: usize, cols: usize, seed: u64) -> Self {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        Self::random(rows, cols, &mut rng)
    }
}

// ─── Shape / element access ───────────────────────────────────────────────────

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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(1));
    /// assert_eq!(m.get(0, 1), Fp::<7>::new(0));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// m.set(1, 0, Fp::<7>::new(5));
    /// assert_eq!(m.get(1, 0), Fp::<7>::new(5));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(2);
    /// assert_eq!(m.get_unchecked(0, 0), Fp::<7>::new(1));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// let r = m.row(1);
    /// assert_eq!(r.len(), 3);
    /// assert_eq!(r[1], Fp::<7>::new(1));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// {
    ///     let r = m.row_mut(0);
    ///     r[1] = Fp::<7>::new(3);
    /// }
    /// assert_eq!(m.get(0, 1), Fp::<7>::new(3));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// let c = m.col(1);
    /// assert_eq!(c.len(), 3);
    /// assert_eq!(c.get(1), Fp::<7>::new(1));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// let ones = m.col_iter(0).filter(|e| **e == Fp::<7>::new(1)).count();
    /// assert_eq!(ones, 1);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// let v = m.submat(1..3, 1..3);
    /// assert_eq!(v.rows(), 2);
    /// assert_eq!(v.cols(), 2);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// m.submat_mut(0..2, 0..2).fill(Fp::<7>::new(2));
    /// assert_eq!(m.get(1, 1), Fp::<7>::new(2));
    /// assert_eq!(m.get(2, 2), Fp::<7>::new(0));
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// assert_eq!(m.row_range(1..3).rows(), 2);
    /// ```
    pub fn row_range(&self, rows: impl RangeBounds<usize>) -> MatView<'_, F> {
        self.submat(rows, ..)
    }

    /// Convenience: submatrix selecting all rows and a contiguous column range.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// assert_eq!(m.col_range(1..3).cols(), 2);
    /// ```
    pub fn col_range(&self, cols: impl RangeBounds<usize>) -> MatView<'_, F> {
        self.submat(.., cols)
    }
}

// ─── Row ops (needed by Gauss-Jordan / PLE) ───────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Swaps rows `r1` and `r2`. A no-op when `r1 == r2`.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// m.set(0, 0, Fp::<7>::new(1));
    /// m.swap_rows(0, 1);
    /// assert_eq!(m.get(1, 0), Fp::<7>::new(1));
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
    ///
    /// # Complexity
    ///
    /// O(cols) multiplications.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(1, 3);
    /// m.set(0, 0, Fp::<7>::new(1));
    /// m.set(0, 1, Fp::<7>::new(2));
    /// m.scale_row(0, Fp::<7>::new(3));
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(3));
    /// assert_eq!(m.get(0, 1), Fp::<7>::new(6));
    /// ```
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
    /// This is the finite-field counterpart of
    /// [`BitMatrix::row_xor`](crate::matrix::BitMatrix::row_xor). When
    /// `factor == F::one()` it behaves exactly like `row_xor` over GF(2)
    /// since `a + 1·b == a XOR b` in `GF(2)`.
    ///
    /// # Panics
    ///
    /// Panics if either row index is out of bounds.
    ///
    /// # Complexity
    ///
    /// O(cols) multiply-adds.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// m.set(0, 0, Fp::<7>::new(2));
    /// m.set(1, 0, Fp::<7>::new(3));
    /// m.axpy_row(1, 0, Fp::<7>::new(4)); // row1 += 4·row0
    /// // row1[0] = 3 + 4·2 = 11 ≡ 4 (mod 7)
    /// assert_eq!(m.get(1, 0), Fp::<7>::new(4));
    /// ```
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

    /// Returns the first row `>= start_row` with a non-zero entry in `col`.
    ///
    /// Mirrors [`BitMatrix::find_pivot_row`](crate::matrix::BitMatrix::find_pivot_row).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// m.set(2, 1, Fp::<7>::new(4));
    /// assert_eq!(m.find_pivot_row(1, 0), Some(2));
    /// assert_eq!(m.find_pivot_row(0, 0), None);
    /// ```
    pub fn find_pivot_row(&self, col: usize, start_row: usize) -> Option<usize> {
        if col >= self.cols || start_row >= self.rows {
            return None;
        }
        let zero = self.data.get(0).zero_like();
        (start_row..self.rows).find(|&r| self.data.get(r * self.cols + col) != &zero)
    }
}

// ─── Derived operations ───────────────────────────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns an owned transpose.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) copies.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 3);
    /// m.set(0, 2, Fp::<7>::new(3));
    /// let t = m.transpose();
    /// assert_eq!(t.shape(), (3, 2));
    /// assert_eq!(t.get(2, 0), Fp::<7>::new(3));
    /// ```
    pub fn transpose(&self) -> Self {
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

    /// Returns a lazy transpose proxy borrowing `self`.
    ///
    /// The proxy is a stub in this story; fused-expression semantics land in
    /// issue `d48a3cfd`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// let t = m.t();
    /// assert_eq!(t.rows(), 4);
    /// ```
    pub fn t(&self) -> Transposed<&Self> {
        Transposed(self)
    }

    /// Returns the diagonal as a [`FieldVec`].
    ///
    /// Length equals `min(rows, cols)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// let d = m.diag();
    /// assert_eq!(d.len(), 3);
    /// assert_eq!(d[0], Fp::<7>::new(1));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// assert_eq!(m.trace(), Fp::<7>::new(3));
    /// ```
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
    ///
    /// # Complexity
    ///
    /// O(n²) in the worst case; returns `false` early on the first mismatch.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// assert!(id.is_symmetric());
    /// ```
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
    /// Panics if `x.len() != self.cols()`.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) multiply-adds.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, matrix::FieldMatrix};
    /// use gf2_core::gfp::Fp;
    ///
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// let x = FieldVec::from(vec![Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(3)]);
    /// let y = id.matvec(&x);
    /// assert_eq!(y[1], Fp::<7>::new(2));
    /// ```
    pub fn matvec(&self, x: &FieldVec<F>) -> FieldVec<F>
    where
        F: ConstField,
    {
        assert_eq!(
            x.len(),
            self.cols,
            "FieldMatrix::matvec: x.len() ({}) != cols ({})",
            x.len(),
            self.cols
        );
        let mut y: FieldVec<F> = FieldVec::zeros(self.rows);
        for r in 0..self.rows {
            let row = &self.data.as_slice()[r * self.cols..(r + 1) * self.cols];
            let mut acc = F::zero();
            for (a, b) in row.iter().zip(x.as_slice().iter()) {
                acc += (*a) * (*b);
            }
            y.set(r, acc);
        }
        y
    }

    /// Computes `y = Aᵀ · x`.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.rows()`.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) multiply-adds.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, matrix::FieldMatrix};
    /// use gf2_core::gfp::Fp;
    ///
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// let x = FieldVec::from(vec![Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(3)]);
    /// let y = id.matvec_transpose(&x);
    /// assert_eq!(y[2], Fp::<7>::new(3));
    /// ```
    pub fn matvec_transpose(&self, x: &FieldVec<F>) -> FieldVec<F>
    where
        F: ConstField,
    {
        assert_eq!(
            x.len(),
            self.rows,
            "FieldMatrix::matvec_transpose: x.len() ({}) != rows ({})",
            x.len(),
            self.rows
        );
        let mut y: FieldVec<F> = FieldVec::zeros(self.cols);
        for r in 0..self.rows {
            let xr = x[r];
            let row = &self.data.as_slice()[r * self.cols..(r + 1) * self.cols];
            for (j, a) in row.iter().enumerate() {
                let cur = y[j];
                y.set(j, cur + (*a) * xr);
            }
        }
        y
    }
}

// ─── MatrixLike impl ──────────────────────────────────────────────────────────

impl<F: FiniteField> MatrixLike<F> for FieldMatrix<F> {
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

// ─── MatView / MatViewMut / ColView ──────────────────────────────────────────

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
}

impl<F: FiniteField> MatrixLike<F> for MatView<'_, F> {
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

    fn transpose(&self) -> Self {
        // MatView is borrow-only; returning a fresh MatView pointing to the
        // same data but with rows/cols swapped would be incorrect because the
        // backing buffer is row-major for the original orientation. Users who
        // want an owned transpose must call `.to_owned().transpose()` instead.
        unimplemented!(
            "MatView::transpose would require an owned copy; call `.to_owned().transpose()`"
        )
    }
}

/// Zero-copy mutable submatrix view.
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
    pub fn get(&self, r: usize, c: usize) -> F {
        assert!(
            r < self.rows && c < self.cols,
            "MatViewMut::get out of bounds"
        );
        self.data[(self.row_offset + r) * self.parent_cols + self.col_offset + c].clone()
    }

    /// Writes `v` at `(r, c)`.
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
}

impl<F: FiniteField> MatrixLike<F> for MatViewMut<'_, F> {
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

    fn transpose(&self) -> Self {
        unimplemented!(
            "MatViewMut::transpose would require an owned copy; call `.to_owned().transpose()`"
        )
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

// ─── Index / Display / arithmetic operators ───────────────────────────────────

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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(2);
    /// let rendered = format!("{}", m);
    /// assert!(rendered.contains('┌'));
    /// assert!(rendered.contains('└'));
    /// ```
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

// Element-wise addition / subtraction helpers ---------------------------------

fn elementwise_add<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    assert_eq!(a.shape(), b.shape(), "FieldMatrix::add: shape mismatch");
    let data: FieldVec<F> = a
        .data
        .as_slice()
        .iter()
        .zip(b.data.as_slice().iter())
        .map(|(x, y)| x.clone() + y.clone())
        .collect();
    FieldMatrix {
        rows: a.rows,
        cols: a.cols,
        data,
    }
}

fn elementwise_sub<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    assert_eq!(a.shape(), b.shape(), "FieldMatrix::sub: shape mismatch");
    let data: FieldVec<F> = a
        .data
        .as_slice()
        .iter()
        .zip(b.data.as_slice().iter())
        .map(|(x, y)| x.clone() - y.clone())
        .collect();
    FieldMatrix {
        rows: a.rows,
        cols: a.cols,
        data,
    }
}

impl<F: FiniteField> Add for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: Self) -> Self::Output {
        elementwise_add(&self, &rhs)
    }
}
impl<F: FiniteField> Add<&FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_add(&self, rhs)
    }
}
impl<F: FiniteField> Add<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_add(self, &rhs)
    }
}
impl<F: FiniteField> Add for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_add(self, rhs)
    }
}

impl<F: FiniteField> Sub for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: Self) -> Self::Output {
        elementwise_sub(&self, &rhs)
    }
}
impl<F: FiniteField> Sub<&FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_sub(&self, rhs)
    }
}
impl<F: FiniteField> Sub<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_sub(self, &rhs)
    }
}
impl<F: FiniteField> Sub for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_sub(self, rhs)
    }
}

impl<F: FiniteField> Neg for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn neg(self) -> Self::Output {
        let data: FieldVec<F> = self.data.into_iter().map(|e| -e).collect();
        FieldMatrix {
            rows: self.rows,
            cols: self.cols,
            data,
        }
    }
}

impl<F: FiniteField> Neg for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn neg(self) -> Self::Output {
        let data: FieldVec<F> = self.data.as_slice().iter().map(|e| -e.clone()).collect();
        FieldMatrix {
            rows: self.rows,
            cols: self.cols,
            data,
        }
    }
}

// Classical O(n³) gemm used as the eager fallback. Story `d48a3cfd` will
// replace the body with delayed-reduction / Strassen-Winograd paths.
fn gemm<F: FiniteField + ConstField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    assert_eq!(
        a.cols, b.rows,
        "FieldMatrix::mul: inner dimensions must match ({} vs {})",
        a.cols, b.rows
    );
    let mut out = FieldMatrix::<F>::zeros(a.rows, b.cols);
    if a.rows == 0 || b.cols == 0 || a.cols == 0 {
        return out;
    }
    for i in 0..a.rows {
        for k in 0..a.cols {
            let aik = *a.data.get(i * a.cols + k);
            if aik == F::zero() {
                continue;
            }
            let out_row_start = i * out.cols;
            let b_row_start = k * b.cols;
            let (out_slice, b_slice) = (
                &mut out.data.as_mut_slice()[out_row_start..out_row_start + out.cols],
                &b.data.as_slice()[b_row_start..b_row_start + b.cols],
            );
            for j in 0..b.cols {
                out_slice[j] += aik * b_slice[j];
            }
        }
    }
    out
}

impl<F: FiniteField + ConstField> Mul for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: Self) -> Self::Output {
        gemm(&self, &rhs)
    }
}
impl<F: FiniteField + ConstField> Mul<&FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: &FieldMatrix<F>) -> Self::Output {
        gemm(&self, rhs)
    }
}
impl<F: FiniteField + ConstField> Mul<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: FieldMatrix<F>) -> Self::Output {
        gemm(self, &rhs)
    }
}
impl<F: FiniteField + ConstField> Mul for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: &FieldMatrix<F>) -> Self::Output {
        gemm(self, rhs)
    }
}

// Scalar multiplication. Restricted to `ConstField` to keep the right-hand
// `&M * F` overload syntactically tractable without a blanket clash.
impl<F: ConstField> Mul<F> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: F) -> Self::Output {
        let data: FieldVec<F> = self.data.as_slice().iter().map(|e| (*e) * rhs).collect();
        FieldMatrix {
            rows: self.rows,
            cols: self.cols,
            data,
        }
    }
}

impl<F: ConstField> Mul<F> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: F) -> Self::Output {
        &self * rhs
    }
}

// `F * &M` is provided via inherent method to avoid orphan-rule trouble for
// generic `F` parameters. Implemented for the concrete `Fp<P>` family below.
impl<const P: u64> Mul<&FieldMatrix<crate::gfp::Fp<P>>> for crate::gfp::Fp<P> {
    type Output = FieldMatrix<crate::gfp::Fp<P>>;
    fn mul(self, rhs: &FieldMatrix<crate::gfp::Fp<P>>) -> Self::Output {
        rhs * self
    }
}

impl<const P: u64> Mul<FieldMatrix<crate::gfp::Fp<P>>> for crate::gfp::Fp<P> {
    type Output = FieldMatrix<crate::gfp::Fp<P>>;
    fn mul(self, rhs: FieldMatrix<crate::gfp::Fp<P>>) -> Self::Output {
        &rhs * self
    }
}

// ─── Range resolution ─────────────────────────────────────────────────────────

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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfp::Fp;

    type F = Fp<7>;
    fn f(v: u64) -> F {
        Fp::<7>::new(v)
    }

    #[test]
    fn zeros_and_identity() {
        let z = FieldMatrix::<F>::zeros(2, 3);
        assert_eq!(z.shape(), (2, 3));
        assert_eq!(z.get(1, 2), f(0));
        let id = FieldMatrix::<F>::identity(3);
        assert_eq!(id.get(2, 2), f(1));
        assert_eq!(id.get(0, 2), f(0));
    }

    #[test]
    fn set_and_get() {
        let mut m = FieldMatrix::<F>::zeros(2, 2);
        m.set(0, 1, f(3));
        assert_eq!(m.get(0, 1), f(3));
        assert_eq!(m[(0, 1)], f(3));
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn get_out_of_bounds_panics() {
        let m = FieldMatrix::<F>::zeros(2, 2);
        let _ = m.get(3, 0);
    }

    #[test]
    fn row_and_col_views() {
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
    fn submat_view_bounds() {
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
    fn submat_mut_fill() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.submat_mut(0..2, 1..3).fill(f(5));
        assert_eq!(m.get(0, 0), f(0));
        assert_eq!(m.get(0, 1), f(5));
        assert_eq!(m.get(1, 2), f(5));
        assert_eq!(m.get(2, 2), f(0));
    }

    #[test]
    fn swap_rows_and_scale() {
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
    fn axpy_row() {
        let mut m = FieldMatrix::<F>::zeros(2, 2);
        m.set(0, 0, f(1));
        m.set(1, 0, f(3));
        m.axpy_row(1, 0, f(2));
        assert_eq!(m.get(1, 0), f(3) + f(2) * f(1));
    }

    #[test]
    fn find_pivot_row_basic() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(1, 1, f(2));
        assert_eq!(m.find_pivot_row(1, 0), Some(1));
        assert_eq!(m.find_pivot_row(2, 0), None);
    }

    #[test]
    fn transpose_and_diag() {
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
    fn matvec_identity_and_general() {
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
    fn add_sub_neg() {
        let mut a = FieldMatrix::<F>::zeros(2, 2);
        let mut b = FieldMatrix::<F>::zeros(2, 2);
        a.set(0, 0, f(5));
        b.set(0, 0, f(3));
        assert_eq!((&a + &b).get(0, 0), f(1));
        assert_eq!((&a - &b).get(0, 0), f(2));
        assert_eq!((-&a).get(0, 0), f(7 - 5));
    }

    #[test]
    fn mul_identity() {
        let a = FieldMatrix::<F>::identity(3);
        let b = FieldMatrix::<F>::identity(3);
        let c = &a * &b;
        assert_eq!(c, FieldMatrix::<F>::identity(3));
    }

    #[test]
    fn mul_rectangular() {
        let mut a = FieldMatrix::<F>::zeros(2, 3);
        a.set(0, 0, f(1));
        a.set(0, 1, f(2));
        a.set(1, 2, f(3));
        let mut b = FieldMatrix::<F>::zeros(3, 2);
        b.set(0, 0, f(1));
        b.set(1, 1, f(4));
        b.set(2, 0, f(5));
        let c = &a * &b;
        assert_eq!(c.shape(), (2, 2));
        assert_eq!(c.get(0, 0), f(1));
        assert_eq!(c.get(0, 1), f(2) * f(4));
        assert_eq!(c.get(1, 0), f(3) * f(5));
    }

    #[test]
    fn scalar_mul_both_sides() {
        let mut a = FieldMatrix::<F>::zeros(2, 2);
        a.set(0, 1, f(3));
        let r1 = &a * f(2);
        let r2 = f(2) * &a;
        assert_eq!(r1, r2);
        assert_eq!(r1.get(0, 1), f(6));
    }

    #[test]
    fn display_contains_borders() {
        let m = FieldMatrix::<F>::identity(2);
        let s = format!("{}", m);
        assert!(s.contains('┌'));
        assert!(s.contains('└'));
    }

    #[test]
    fn matrixlike_trait_on_field_matrix() {
        let mut m = FieldMatrix::<F>::zeros(2, 2);
        <FieldMatrix<F> as MatrixLikeMut<F>>::set(&mut m, 1, 0, f(4));
        assert_eq!(<FieldMatrix<F> as MatrixLike<F>>::get(&m, 1, 0), f(4));
        assert_eq!(<FieldMatrix<F> as MatrixLike<F>>::shape(&m), (2, 2));
    }

    #[test]
    fn matrixlike_trait_on_matview() {
        let mut m = FieldMatrix::<F>::zeros(3, 3);
        m.set(1, 1, f(2));
        let v = m.submat(1..3, 1..3);
        assert_eq!(<MatView<F> as MatrixLike<F>>::rows(&v), 2);
        assert_eq!(<MatView<F> as MatrixLike<F>>::get(&v, 0, 0), f(2));
    }

    #[test]
    fn from_rows_roundtrip() {
        let r0 = FieldVec::from(vec![f(1), f(2)]);
        let r1 = FieldVec::from(vec![f(3), f(4)]);
        let m = FieldMatrix::from_rows(vec![r0, r1]);
        assert_eq!(m.shape(), (2, 2));
        assert_eq!(m.get(1, 1), f(4));
    }
}
