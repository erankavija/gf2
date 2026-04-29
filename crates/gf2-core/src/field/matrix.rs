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
//! The `Mul` body is a classical O(n³) cache-blocked `gemm` backed by
//! delayed-reduction dot-product kernels; Strassen-Winograd is layered above
//! this base case in the field algorithms.

use std::fmt;
use std::ops::{Bound, Index, RangeBounds};

use crate::field::{ConstField, FieldVec, FiniteField};
use crate::matrix_like::{MatrixLike, MatrixLikeMut};

// Re-export the PLE-decomposition permutation type so callers of
// `field::matrix` (the natural module surface for matrix algorithms)
// can refer to it without crossing module boundaries. The canonical
// home is [`crate::field::ple::Permutation`]; PLE-derived methods on
// [`FieldMatrix`] are implemented in that module.
pub use crate::field::ple::Permutation;

// ─── Test-only allocation counter ─────────────────────────────────────────────
//
// Exposed only under `#[cfg(test)]`; the production path is a single
// thread-local increment that LLVM can elide when the counter is dead.
// The counter is bumped exactly once per `FieldMatrix::new` invocation
// — the canonical "fresh allocation" entry point for trsm/trmm/trtri/
// trtrm scratches and for test fixtures. The triangular-allocation
// regression tests use this counter to certify the per-recursion-
// level allocation budget.
//
// **Thread-local on purpose.** Earlier the counter was a process-wide
// `AtomicU64`; that races against any other test running in the same
// `cargo test` worker pool because the JIT regression tests reset and
// read the counter from the same thread that runs the
// trsm/trmm/trtri/trtrm call. Making the counter `thread_local!`
// scopes the count to the test's own thread, which matches the
// recursive single-threaded execution model of the triangular
// primitives and gives deterministic numbers under both `cargo
// nextest` (process-per-test) and `cargo test --release` (thread pool).
#[cfg(test)]
thread_local! {
    static FIELDMATRIX_NEW_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Test-only: returns the cumulative count of [`FieldMatrix::new`]
/// allocations on **this thread** since the last
/// [`reset_fieldmatrix_new_count`]. Thread-local so concurrent tests
/// in `cargo test --release` do not contaminate each other's counts.
#[cfg(test)]
pub(crate) fn fieldmatrix_new_count() -> u64 {
    FIELDMATRIX_NEW_COUNT.with(|c| c.get())
}

/// Test-only: zeroes the per-thread [`FieldMatrix::new`] counter.
#[cfg(test)]
pub(crate) fn reset_fieldmatrix_new_count() {
    FIELDMATRIX_NEW_COUNT.with(|c| c.set(0));
}

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
    /// Rows of the logically transposed matrix (i.e. columns of the backing
    /// matrix).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(2, 5);
    /// let t = m.t();
    /// assert_eq!(t.rows(), 5);
    /// ```
    pub fn rows(&self) -> usize {
        self.0.cols()
    }

    /// Columns of the logically transposed matrix (i.e. rows of the backing
    /// matrix).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(2, 5);
    /// let t = m.t();
    /// assert_eq!(t.cols(), 2);
    /// ```
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
        #[cfg(test)]
        FIELDMATRIX_NEW_COUNT.with(|c| c.set(c.get() + 1));
        let data: FieldVec<F> = (0..rows * cols).map(|_| fill.clone()).collect();
        Self { rows, cols, data }
    }

    /// Crate-private: build a [`FieldMatrix`] directly from a pre-sized
    /// [`FieldVec`] payload.
    ///
    /// Used by the sparse module (story `8a90882e`) to hand back a dense
    /// matrix whose backing storage was allocated in one shot. The caller
    /// must guarantee `data.len() == rows * cols` (or `data.len() == 0`
    /// when either dimension is zero); this invariant is debug-asserted.
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
    /// Crate-private accessor for the raw backing slice.
    ///
    /// Used by the expression-template kernels in
    /// [`crate::field::expr`] (story `d48a3cfd/T2`) so they can feed the
    /// existing `dot_product_slices` helper without reaching through the
    /// `MatrixLike::get` interface element-by-element. This is strictly
    /// row-major over `rows * cols` cells.
    #[doc(hidden)]
    pub(crate) fn as_data_slice(&self) -> &[F] {
        self.data.as_slice()
    }

    /// Crate-private mutable counterpart to [`Self::as_data_slice`].
    ///
    /// Used by the blocked fused-gemm kernels in
    /// [`crate::field::expr`] (story `d48a3cfd/T2` R3) to write into the
    /// output matrix row-block directly, matching T1's `gemm` inner loop
    /// shape while folding `β·C` into the same store.
    #[doc(hidden)]
    pub(crate) fn as_data_mut_slice(&mut self) -> &mut [F] {
        self.data.as_mut_slice()
    }

    /// Constructs a matrix from a `Vec` of row vectors, one [`FieldVec<F>`] per row.
    ///
    /// # Arguments
    ///
    /// * `rows` - Non-empty list of equal-length row vectors.
    ///
    /// # Panics
    ///
    /// Panics if `rows` is empty or if the rows have unequal lengths.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let r0: FieldVec<Fp<7>> = vec![Fp::new(1), Fp::new(2)].into_iter().collect();
    /// let r1: FieldVec<Fp<7>> = vec![Fp::new(3), Fp::new(4)].into_iter().collect();
    /// let m = FieldMatrix::from_rows(vec![r0, r1]);
    /// assert_eq!(m.shape(), (2, 2));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(rows × cols).
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
    /// Creates a `rows × cols` matrix initialised to zero.
    ///
    /// Named `with_capacity` for Armadillo parity (it mirrors
    /// `arma::mat(rows, cols, fill::none)`); in safe Rust this is equivalent
    /// to [`FieldMatrix::zeros`] because the `fill::none` no-init optimisation
    /// cannot be expressed without `unsafe`, which `gf2-core` denies. Use
    /// this when the caller will overwrite every element and the zero-fill
    /// cost is acceptable.
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
    /// let m = FieldMatrix::<Fp<7>>::with_capacity(4, 4);
    /// assert_eq!(m.rows(), 4);
    /// assert_eq!(m.cols(), 4);
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(0));
    /// ```
    pub fn with_capacity(rows: usize, cols: usize) -> Self {
        Self::zeros(rows, cols)
    }

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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(3, 5);
    /// assert_eq!(m.rows(), 3);
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
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(3, 5);
    /// assert_eq!(m.cols(), 5);
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
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(3, 5);
    /// assert_eq!(m.shape(), (3, 5));
    /// ```
    #[inline]
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Returns `true` if the matrix is square.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let sq = FieldMatrix::<Fp<7>>::identity(4);
    /// assert!(sq.is_square());
    /// let rect = FieldMatrix::<Fp<7>>::zeros(2, 3);
    /// assert!(!rect.is_square());
    /// ```
    #[inline]
    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    /// Returns `true` if either dimension is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(0, 5);
    /// assert!(m.is_empty());
    /// let n = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// assert!(!n.is_empty());
    /// ```
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
    /// # Complexity
    ///
    /// O(cols) element swaps; no allocation.
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
    /// # Complexity
    ///
    /// O(rows − start_row) comparisons in the worst case; returns early on the
    /// first non-zero entry found.
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
    ///
    /// # Complexity
    ///
    /// O(rows · cols) scalar comparisons; the output allocates `nnz`
    /// `(col_idx, value)` pairs plus a `(rows + 1)` row-pointer array.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 3);
    /// m.set(0, 1, Fp::<7>::new(2));
    /// m.set(1, 2, Fp::<7>::new(5));
    /// let s = m.to_sparse();
    /// assert_eq!(s.shape(), (2, 3));
    /// assert_eq!(s.nnz(), 2);
    /// ```
    pub fn to_sparse(&self) -> crate::field::sparse_matrix::SparseFieldMatrix<F> {
        crate::field::sparse_matrix::SparseFieldMatrix::from_dense(self)
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
    /// # Complexity
    ///
    /// O(min(rows, cols)) element clones.
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
    /// # Complexity
    ///
    /// O(min(rows, cols)) field additions.
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
    /// # Panics
    ///
    /// Panics if `x.len() != self.cols()`. Also panics if
    /// `self.rows > 0 && self.cols == 0` because the output is a length-
    /// `self.rows` zero vector but neither `x` (empty) nor `self.data`
    /// (empty) supplies an `F` instance to seed the zero vector under the
    /// `F: FiniteField` bound; use `F: ConstField` or ensure the matrix has
    /// at least one column.
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
        // Obtain a zero element without requiring `F: ConstField`. When
        // `cols > 0`, `x[0]` is available; otherwise the only candidate is
        // a matrix entry, which requires `rows > 0 && cols > 0`. For the
        // pathological shape `(rows > 0, cols == 0)` we fall back to the
        // static escape hatch `F::zero_hint()` (returns `Some` for all
        // `ConstField` impls), and only panic if that also returns `None`.
        let zero: F = if self.cols > 0 {
            x.as_slice()[0].zero_like()
        } else if let Some(z) = F::zero_hint() {
            z
        } else {
            // `self.rows > 0 && self.cols == 0` on a runtime-context field.
            panic!(
                "FieldMatrix::matvec: producing length-{} zero vector from \
                 ({}×0) matrix requires a zero witness; use F: ConstField \
                 or ensure the matrix has at least one column",
                self.rows, self.rows
            );
        };
        let mut y: FieldVec<F> = FieldVec::zeros_from(self.rows, &zero);
        // Delegate each row to the delayed-reduction dot-product kernel so
        // large-prime fields (where a naive running accumulator would have
        // to reduce on every multiply) and GF(2^m) (where Wide = Self so a
        // single XOR chain is possible) share the same code path.
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
    /// Panics if `x.len() != self.rows()`. Also panics if
    /// `self.rows == 0 && self.cols > 0` because the output is a length-
    /// `self.cols` zero vector but neither `x` (empty) nor `self.data`
    /// (empty) supplies an `F` instance to seed the zero vector under the
    /// `F: FiniteField` bound; use `F: ConstField` or ensure the matrix has
    /// at least one row.
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
        // Same `zero_like` pattern as `matvec`: prefer `x[0]` when the input
        // vector is non-empty, else fall back to `F::zero_hint()` (which
        // returns `Some` for `ConstField` impls), and only panic when that
        // also fails.
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
        // Rank-1 update reformulated as per-output-cell dot products over the
        // transposed matrix, so we reuse the delayed-reduction kernel. The
        // one-shot transpose is O(rows · cols) and keeps the inner loop
        // strictly contiguous on both operands. Identical algebraic result to
        // the previous column-walk; faster for large-prime fields because it
        // defers reductions by the same §1.2 kmax scheduling the classical
        // gemm path uses.
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

// ─── MatrixLike impl ──────────────────────────────────────────────────────────

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

// ─── MatView / MatViewMut / ColView ──────────────────────────────────────────

/// Zero-copy immutable submatrix view.
///
/// A `MatView` borrows a rectangular window of a parent [`FieldMatrix`].
/// Rows are contiguous in memory; stepping between rows uses the parent's
/// full row stride (`parent_cols`) so views over column ranges remain
/// aligned to the parent's row-major layout.
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
/// // Element (0, 0) of the view is m[(1, 1)] of the parent.
/// assert_eq!(v.get(0, 0), Fp::<7>::new(1));
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// let v = m.submat(1..3, 0..4);
    /// assert_eq!(v.rows(), 2);
    /// ```
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns in the view.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// let v = m.submat(0..4, 1..3);
    /// assert_eq!(v.cols(), 2);
    /// ```
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Element at `(r, c)`, relative to the view's origin.
    ///
    /// # Panics
    ///
    /// Panics if `r >= self.rows()` or `c >= self.cols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// let v = m.submat(1..3, 1..3);
    /// assert_eq!(v.get(0, 0), Fp::<7>::new(1));
    /// assert_eq!(v.get(0, 1), Fp::<7>::new(0));
    /// ```
    pub fn get(&self, r: usize, c: usize) -> F {
        assert!(r < self.rows && c < self.cols, "MatView::get out of bounds");
        self.data[(self.row_offset + r) * self.parent_cols + self.col_offset + c].clone()
    }

    /// Crate-internal: returns the contiguous slice backing logical row
    /// `r` of this view. Rows of a row-major view are contiguous in the
    /// parent buffer at offset
    /// `(row_offset + r) * parent_cols + col_offset`, length `cols`, so
    /// no allocation is needed. Used by the in-place gemm kernels in
    /// [`crate::field::matrix::gemm_axpy_into_view`] to feed
    /// [`crate::field::vec::dot_product_slices`] without materialising a
    /// per-row scratch buffer.
    ///
    /// # Panics
    ///
    /// Panics if `r >= self.rows()`.
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
    ///
    /// The returned matrix owns its storage, so it can outlive the parent
    /// buffer the view borrowed from.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) element clones plus one allocation of that size.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// let v = m.submat(1..3, 1..3);
    /// let owned = v.to_owned();
    /// assert_eq!(owned.shape(), (2, 2));
    /// assert_eq!(owned.get(0, 0), Fp::<7>::new(1));
    /// ```
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
    /// Mirrors [`FieldMatrix::submat`] but operates on an existing view.
    ///
    /// # Panics
    ///
    /// Panics if either range exceeds the view's dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// let outer = m.submat(0..4, 0..4);
    /// let inner = outer.submat(1..3, 1..3);
    /// assert_eq!(inner.get(0, 0), Fp::<7>::new(1));
    /// ```
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
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 3);
/// m.submat_mut(0..2, 0..2).fill(Fp::<7>::new(2));
/// assert_eq!(m.get(0, 0), Fp::<7>::new(2));
/// assert_eq!(m.get(1, 1), Fp::<7>::new(2));
/// // Cells outside the view are untouched.
/// assert_eq!(m.get(2, 2), Fp::<7>::new(0));
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(4, 4);
    /// let v = m.submat_mut(1..3, 0..4);
    /// assert_eq!(v.rows(), 2);
    /// ```
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Column count.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(4, 4);
    /// let v = m.submat_mut(0..4, 1..3);
    /// assert_eq!(v.cols(), 2);
    /// ```
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Element at `(r, c)`, relative to the view's origin.
    ///
    /// # Panics
    ///
    /// Panics if `r >= self.rows()` or `c >= self.cols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::identity(3);
    /// let v = m.submat_mut(1..3, 1..3);
    /// assert_eq!(v.get(0, 0), Fp::<7>::new(1));
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// {
    ///     let mut v = m.submat_mut(0..2, 0..2);
    ///     v.set(1, 0, Fp::<7>::new(5));
    /// }
    /// assert_eq!(m.get(1, 0), Fp::<7>::new(5));
    /// ```
    pub fn set(&mut self, r: usize, c: usize, v: F) {
        assert!(
            r < self.rows && c < self.cols,
            "MatViewMut::set out of bounds"
        );
        let idx = (self.row_offset + r) * self.parent_cols + self.col_offset + c;
        self.data[idx] = v;
    }

    /// Fills every cell of the view with `value`.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) element clones.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// m.submat_mut(0..2, 0..2).fill(Fp::<7>::new(4));
    /// assert_eq!(m.get(1, 1), Fp::<7>::new(4));
    /// assert_eq!(m.get(2, 2), Fp::<7>::new(0));
    /// ```
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
    ///
    /// # Complexity
    ///
    /// O(rows · cols) element clones; no allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let src = FieldMatrix::<Fp<7>>::identity(2);
    /// let mut dst = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// dst.submat_mut(0..2, 0..2).assign(&src);
    /// assert_eq!(dst.get(0, 0), Fp::<7>::new(1));
    /// assert_eq!(dst.get(1, 1), Fp::<7>::new(1));
    /// ```
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
    ///
    /// # Complexity
    ///
    /// O(cols) element swaps; no allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// m.set(0, 0, Fp::<7>::new(2));
    /// m.submat_mut(0..2, 0..3).swap_rows(0, 1);
    /// assert_eq!(m.get(1, 0), Fp::<7>::new(2));
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(0));
    /// ```
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
    ///
    /// The returned matrix owns its storage, so it can outlive the borrow.
    ///
    /// # Complexity
    ///
    /// O(rows · cols) element clones plus one allocation of that size.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::identity(3);
    /// let v = m.submat_mut(0..2, 0..2);
    /// let owned = v.to_owned();
    /// assert_eq!(owned.shape(), (2, 2));
    /// assert_eq!(owned.get(0, 0), Fp::<7>::new(1));
    /// ```
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

    /// Returns a sub-view restricted to the rectangle `(rows, cols)`,
    /// borrowing the same backing storage. The borrow chain is
    /// `MatViewMut → MatViewMut`, so the returned sub-view inherits the
    /// parent's lifetime and write access.
    ///
    /// # Panics
    ///
    /// Panics if either range exceeds the view's dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(4, 4);
    /// let mut v = m.submat_mut(0..4, 0..4);
    /// v.submat_mut(1..3, 1..3).fill(Fp::<7>::new(5));
    /// assert_eq!(m.get(2, 2), Fp::<7>::new(5));
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(0));
    /// ```
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

    /// Returns an immutable sub-view restricted to `(rows, cols)`. Useful
    /// for passing the view as a read-only argument while retaining the
    /// outer mutable borrow.
    ///
    /// # Panics
    ///
    /// Panics if either range exceeds the view's dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::identity(4);
    /// let v = m.submat_mut(0..4, 0..4);
    /// let inner = v.submat(1..3, 1..3);
    /// assert_eq!(inner.get(0, 0), Fp::<7>::new(1));
    /// ```
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
    /// rectangle. Equivalent to `self.submat(.., ..)` but more concise at
    /// call sites that need to hand the view as a read-only argument.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::identity(2);
    /// let v = m.submat_mut(.., ..);
    /// assert_eq!(v.as_view().get(1, 1), Fp::<7>::new(1));
    /// ```
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

    /// Splits the view into two disjoint mutable views at row `mid`,
    /// consuming `self`. Returns `(top, bot)` where `top` holds rows
    /// `0..mid` and `bot` holds rows `mid..rows`. Because rows of a
    /// row-major view are contiguous chunks of `parent_cols` cells, the
    /// split is implemented as a `slice::split_at_mut` at the row
    /// boundary, so the two halves borrow disjoint regions of the
    /// backing store and can be passed to separate routines (or to one
    /// routine as `&dst` + `&src`) without aliasing the same `FieldVec`.
    ///
    /// This is the zero-allocation primitive that the `triangular`
    /// module's recursive primitives use to avoid `to_owned()`
    /// snapshots when the recursion needs to read one half of `B` while
    /// writing the other half.
    ///
    /// # Panics
    ///
    /// Panics if `mid > self.rows()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(4, 2);
    /// let v = m.submat_mut(.., ..);
    /// let (mut top, mut bot) = v.split_rows_mut(2);
    /// top.set(0, 0, Fp::<7>::new(3));
    /// bot.set(0, 0, Fp::<7>::new(5));
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(3));
    /// assert_eq!(m.get(2, 0), Fp::<7>::new(5));
    /// ```
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

    /// Reborrows this mutable view as a fresh `MatViewMut<'_, F>` with
    /// a shorter lifetime. Equivalent to `self.submat_mut(.., ..)`.
    ///
    /// Useful when a routine needs to pass a `MatViewMut` to a callee
    /// without consuming the outer borrow.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// let mut v = m.submat_mut(.., ..);
    /// {
    ///     let mut r = v.reborrow();
    ///     r.set(0, 0, Fp::<7>::new(3));
    /// }
    /// assert_eq!(m.get(0, 0), Fp::<7>::new(3));
    /// ```
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
/// // Column 1 of the 3×3 identity is (0, 1, 0).
/// assert_eq!(c.get(0), Fp::<7>::new(0));
/// assert_eq!(c.get(1), Fp::<7>::new(1));
/// assert_eq!(c.get(2), Fp::<7>::new(0));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ColView<'a, F> {
    data: &'a [F],
    start: usize,
    stride: usize,
    len: usize,
}

impl<'a, F: FiniteField> ColView<'a, F> {
    /// Number of elements in the column.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(4);
    /// assert_eq!(m.col(0).len(), 4);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the column is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// assert!(!m.col(1).is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Element at position `i` along the column.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// assert_eq!(m.col(1).get(1), Fp::<7>::new(1));
    /// assert_eq!(m.col(1).get(0), Fp::<7>::new(0));
    /// ```
    pub fn get(&self, i: usize) -> F {
        assert!(i < self.len, "ColView::get index {} out of bounds", i);
        self.data[self.start + i * self.stride].clone()
    }

    /// Iterator over references to each column element.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::identity(3);
    /// let non_zero = m.col(1).iter().filter(|e| **e != Fp::<7>::new(0)).count();
    /// assert_eq!(non_zero, 1);
    /// ```
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

// NOTE: The eager `Add`/`Sub`/`Neg` operator overloads that T1 (issue
// `91c06222`) provided here have been moved to the expression-template
// layer in `crate::field::expr` (story `d48a3cfd/T2`, issue `7e6183bb`).
// See `dev/plans/expression_templates_design.md` §4.5 for the migration
// rationale.
//
// The new impls return proxy types (`Sum`, `NegProxy`, `FusedProductPlus`,
// …) instead of `FieldMatrix<F>`, so `&a * &b + &c` fuses to a single
// kernel call on the evaluation boundary. Call sites that need the
// materialised matrix write `(&a + &b).into()` (or rely on type inference
// at the binding site).

// Row-tile height for the blocked classical gemm. Sized to keep the
// active working set (one row block of `A`, one column block of `B`, one
// output block) in L1/L2 on commodity x86_64 and aarch64 cores. The value
// is a soft knob — correctness is independent of it, so it can be retuned
// in issue `64c88ae4` (the terminal benchmark story) without touching
// callers.
pub(crate) const GEMM_ROW_TILE: usize = 32;

// Column-tile width for the blocked classical gemm. See `GEMM_ROW_TILE`
// for the tuning rationale.
pub(crate) const GEMM_COL_TILE: usize = 64;

/// Classical blocked gemm over `F: FiniteField` with delayed reduction.
///
/// Implements the §1.2 Dumas–Pernet pattern: transpose `B` once so the inner
/// kernel is a cache-friendly row·row dot product, then for each `(i, j)`
/// cell compute `∑_k a[i,k] · b[k,j]` via a slice-level delayed-reduction
/// dot product. That kernel chunks its accumulation by
/// [`FiniteField::max_unreduced_additions`] so the `Wide` accumulator never
/// overflows; this function asserts (in debug builds) that the inner
/// dimension either fits under kmax or is correctly chunked downstream.
///
/// SIMD — where available — is inherited from the slice product-sum kernels.
/// Single-word GF(2^m) fields with `m ∈ {8, 16, 32}` additionally use a
/// matrix-level batch hook that exports each row/column dot product to `u64`
/// lanes and calls the VPCLMULQDQ-aware batch multiply kernel once per output
/// cell, reusing scratch buffers across the blocked traversal. Other fields,
/// unsupported GF(2^m) degrees, and builds without a detected SIMD kernel keep
/// the scalar/delayed-reduction fallback. Strassen–Winograd recursion is
/// explicitly out of scope (that is issue `ad597ede`).
///
/// # Arguments
///
/// * `a` - Left operand of shape `m × k`. Its column count must equal
///   `b.rows`.
/// * `b` - Right operand of shape `k × n`. Its row count must equal
///   `a.cols`.
///
/// The result has shape `m × n` with entry `(i, j) = ∑_{t=0}^{k-1}
/// a[i, t] · b[t, j]`.
///
/// # Panics
///
/// Panics if `a.cols != b.rows`. Also panics if `a.rows > 0 && b.cols > 0 &&
/// a.cols == 0` (equivalently `b.rows == 0`) **and** both inputs carry no
/// elements; in that degenerate configuration no `F` instance is available
/// from either factor so the output's zero matrix cannot be materialised for
/// a runtime-context field. Use `F: ConstField` or ensure at least one
/// factor is non-empty to avoid this panic — this matches the contract
/// locked by `ab791e27`.
///
/// # Complexity
///
/// The classical triple-loop cost is `O(m · k · n)` field multiplications
/// plus the same count of field additions, amortised against a single
/// transpose of `B` costing `O(k · n)` clones for cache locality. The
/// slice-level dot-product kernel accumulates in the field's `Wide` type
/// and folds back to canonical form at most once per `kmax` additions,
/// where `kmax = F::max_unreduced_additions()`. The per-MAC reduction
/// count is therefore reduced by a factor of `kmax` relative to an
/// eager-reduction inner loop — for Mersenne-31 that is a ~2³¹ headroom,
/// and for the small `Gf2m` fields it coincides with unbounded
/// accumulation (`kmax == usize::MAX`).
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 2], [3, 4]] over GF(7)
/// let a = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(1), Fp::<7>::new(2)].into_iter().collect(),
///     vec![Fp::<7>::new(3), Fp::<7>::new(4)].into_iter().collect(),
/// ]);
/// // B = [[5, 6], [7, 8]] over GF(7)
/// let b = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(5), Fp::<7>::new(6)].into_iter().collect(),
///     vec![Fp::<7>::new(7), Fp::<7>::new(8)].into_iter().collect(),
/// ]);
///
/// // A·B = [[19, 22], [43, 50]]  →  mod 7  →  [[5, 1], [1, 1]]
/// let c = gemm(&a, &b);
/// assert_eq!(c.shape(), (2, 2));
/// assert_eq!(c.get(0, 0), Fp::<7>::new(5));
/// assert_eq!(c.get(0, 1), Fp::<7>::new(1));
/// assert_eq!(c.get(1, 0), Fp::<7>::new(1));
/// assert_eq!(c.get(1, 1), Fp::<7>::new(1));
/// ```
pub fn gemm<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
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

    // From here: a.rows > 0 && b.cols > 0, so the output has `a.rows * b.cols
    // > 0` cells and its backing storage MUST be the same length. We need a
    // zero element to materialise those cells. Source one from whichever
    // factor is non-empty, or — if both are empty — use `F::zero_hint()`
    // which returns `Some(F::zero())` on `ConstField` implementations and
    // `None` on runtime-context fields like `Gf2mElement`.
    let zero: F = if !a.data.as_slice().is_empty() {
        a.data.as_slice()[0].zero_like()
    } else if !b.data.as_slice().is_empty() {
        b.data.as_slice()[0].zero_like()
    } else if let Some(z) = F::zero_hint() {
        z
    } else {
        // a.cols == 0 (equivalently b.rows == 0) and both factors carry no
        // storage. The output's semantic value is the m×n zero matrix, but
        // we have no `F` instance to clone for runtime-context fields. The
        // type-only escape hatch `F::zero_hint()` also returned `None`, so
        // there is no way to fabricate a zero here.
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

    // Dumas–Pernet §1.2 classical-bound sanity check. The slice-level dot
    // product chunks by `kmax` so the Wide accumulator never overflows even
    // for huge inner dims, but we document the invariant here so future
    // readers see the theorem-4 / §1.2 contract at the call site. In debug
    // builds we also assert that every whole-row reduction respects the
    // bound — this is a tautology against the chunked kernel but serves as
    // an anchored regression gate if anyone later inlines the reduction.
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
    let mut scratch_a = Vec::<u64>::new();
    let mut scratch_b = Vec::<u64>::new();
    let mut scratch_products = Vec::<u64>::new();

    // Blocked traversal over output tiles. The inner kernel is a single
    // `dot_product_slices` call per output cell.
    for i_blk in (0..a.rows).step_by(GEMM_ROW_TILE) {
        let i_end = (i_blk + GEMM_ROW_TILE).min(a.rows);
        for j_blk in (0..b.cols).step_by(GEMM_COL_TILE) {
            let j_end = (j_blk + GEMM_COL_TILE).min(b.cols);
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
                    } else {
                        *out_cell = crate::field::vec::dot_product_slices(a_row, b_col, &zero);
                    }
                }
            }
        }
    }
    out
}

// ─── View-based gemm kernels (zero scratch beyond gemm's B-transpose) ─────────

/// In-place gemm kernel `out ← A · B` writing into a [`MatViewMut`].
///
/// Mirrors [`gemm`]'s blocked traversal but lets the caller own `out`
/// (so the routine itself returns nothing and allocates no
/// `FieldMatrix` on top of `gemm`'s standard B-transpose scratch).
///
/// The single allocation this routine performs is the `B`-transpose
/// scratch via `MatrixLike::transpose`, which mirrors the historical
/// behaviour of [`gemm`]. No additional `FieldMatrix<F>` is
/// materialised — `out` is overwritten cell by cell via the same
/// `dot_product_slices` primitive [`gemm`] uses.
///
/// # Arguments
///
/// * `a` — left operand, shape `m × k`. Any [`MatrixLike<F>`].
/// * `b` — right operand, shape `k × n`. Any [`MatrixLike<F>`].
/// * `out` — destination view, shape `m × n`.
///
/// # Panics
///
/// Panics if `a.cols() != b.rows()`, `out.rows() != a.rows()`, or
/// `out.cols() != b.cols()`.
///
/// # Complexity
///
/// `O(m · k · n)` field multiplications, plus the one-shot transpose of
/// `B` for cache locality (`O(k · n)` clones).
pub(crate) fn gemm_into_view<F, A, B>(a: &A, b: &B, mut out: MatViewMut<'_, F>)
where
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
    // Transpose `B` into an owned `B::Owned` so the inner dot product
    // walks contiguous memory. This is the only allocation this kernel
    // performs.
    let b_t = b.transpose();
    for i_blk in (0..m).step_by(GEMM_ROW_TILE) {
        let i_end = (i_blk + GEMM_ROW_TILE).min(m);
        for j_blk in (0..n).step_by(GEMM_COL_TILE) {
            let j_end = (j_blk + GEMM_COL_TILE).min(n);
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
}

/// Fused kernel `out ← α · A · B + β · out`.
///
/// In-place axpy form of [`gemm_into_view`] where the destination view
/// doubles as the `C` operand of the classical `α · A · B + β · C`
/// shape. Each cell is computed as
///
/// ```text
///     out[i, j] := α · (A · B)[i, j] + β · out[i, j]
/// ```
///
/// reading `out[i, j]` BEFORE writing the new value, so the kernel is
/// safe even though `out` aliases its own `C` operand. This is the
/// idiom the [`crate::field::triangular`] `trsm` and `trmm` routines
/// need: `submul` is `α = −1, β = 1`, `addmul` is `α = 1, β = 1`. The
/// caller-supplied [`MatViewMut`] lets the kernel write into a
/// rectangular sub-window of a parent buffer without paying any
/// `to_owned()` snapshot cost.
///
/// # Shape contract
///
/// * `a.cols() == b.rows()`
/// * `out.shape() == (a.rows(), b.cols())`
///
/// Both are asserted with clear panic messages.
///
/// # Aliasing
///
/// The kernel is correct under the trsm/trmm idiom where `out`
/// **doubles as `C`** (the kernel reads `out.get(i, j)` once before
/// writing `out.set(i, j, …)` for the same cell). Aliasing `out`'s
/// underlying buffer with the operand views `a` or `b` is **undefined**
/// — the borrow checker will normally enforce this for `MatView` /
/// `MatViewMut` callers because `MatView` borrows the parent buffer
/// immutably while `MatViewMut` borrows it mutably; obtaining both for
/// the same parent slice is impossible without going through
/// [`MatViewMut::split_rows_mut`] (which returns disjoint windows).
///
/// # Delayed reduction
///
/// The inner accumulation is delegated to
/// [`crate::field::vec::dot_product_slices`] — the same delayed-
/// reduction primitive [`gemm`] uses — so the `Wide` accumulator
/// chunks every `F::max_unreduced_additions()` MACs. The single
/// allocation this kernel performs is the standard `B`-transpose
/// scratch (the same one [`gemm`] and [`gemm_into_view`] pay), giving
/// the inner kernel cache-friendly contiguous row·row dot products.
///
/// # Complexity
///
/// `O(m · k · n)` field multiplies plus the one-time `O(k · n)`
/// transpose of `B` for cache locality. The β·out fold adds one
/// extra mul + one extra read + one extra add per output cell,
/// dominated by the inner dot product for any non-degenerate inner
/// dim.
pub(crate) fn gemm_axpy_into_view<F>(
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
    if m == 0 || n == 0 {
        return;
    }
    if k == 0 {
        // Empty inner dim: A·B is the zero matrix, so out ← β · out.
        // Read each cell, scale by β, write back.
        for i in 0..m {
            for j in 0..n {
                let v = beta.clone() * out.get(i, j);
                out.set(i, j, v);
            }
        }
        return;
    }
    let zero: F = a.get(0, 0).zero_like();
    // Transpose `B` once so the inner kernel walks contiguous memory in
    // both operands. Mirrors `gemm` / `gemm_into_view`. This is the
    // single allocation this routine performs on top of the caller-
    // supplied views.
    let b_t = b.transpose();
    // Blocked traversal over output tiles. The inner kernel is one
    // `dot_product_slices` per cell — the same delayed-reduction
    // primitive `gemm` uses. The `β · out[i, j]` fold reads the cell
    // BEFORE writing the new value at (i, j), so the routine is safe
    // even when `out` aliases its own `C` operand.
    for i_blk in (0..m).step_by(GEMM_ROW_TILE) {
        let i_end = (i_blk + GEMM_ROW_TILE).min(m);
        for j_blk in (0..n).step_by(GEMM_COL_TILE) {
            let j_end = (j_blk + GEMM_COL_TILE).min(n);
            for i in i_blk..i_end {
                let a_row = a.row_slice(i);
                debug_assert_eq!(a_row.len(), k);
                for j in j_blk..j_end {
                    let b_col = b_t.row(j);
                    debug_assert_eq!(b_col.len(), k);
                    let prod = crate::field::vec::dot_product_slices(a_row, b_col, &zero);
                    let c_old = out.get(i, j);
                    out.set(i, j, alpha.clone() * prod + beta.clone() * c_old);
                }
            }
        }
    }
}

// ─── gemm_axpy_into_view_diag — implicit unit-diagonal variant ────────────────

/// Diagonal-handling flag for [`gemm_axpy_into_view_diag`].
///
/// Distinguishes operands whose diagonal cells are physically present in
/// storage from operands whose diagonal is logically all-ones (the
/// storage cell may carry garbage and must NOT be read on the diagonal).
/// This is the convention `trtrm` uses for the unit-lower-triangular
/// `L` operand: the diagonal cells of `L` are reused for `L21` /
/// product-output entries during the in-place compression, so the
/// storage's `[i, i]` cell does not hold `1` and reading it would yield
/// the wrong value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitDiag {
    /// Operand storage holds the actual diagonal values.
    Stored,
    /// Operand storage's diagonal is logically all-ones (read as
    /// `F::one()` synthesised from `zero_like().one_like()`),
    /// regardless of what the underlying buffer holds. The kernel
    /// **never** reads the `[i, i]` storage cell on a unit-diagonal
    /// operand.
    Implicit,
}

/// Read-time wrapper that synthesises `F::one()` on the diagonal of any
/// [`MatrixLike`] operand. Used internally by
/// [`gemm_axpy_into_view_diag`] so the existing per-cell `gemm` kernel
/// can be reused without a special-case loop.
///
/// The wrapper does NOT mutate the underlying matrix; it just
/// substitutes `F::one()` for `get(i, i)` reads. Strict-upper or
/// strict-lower regions are forwarded verbatim — callers that need a
/// triangular operand must arrange for those regions to be zero in the
/// underlying storage.
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
        // Materialise an owned transpose. We synthesise the unit
        // diagonal at clone time so the resulting buffer carries
        // the logical 1 on the diagonal cells. Callers (this module's
        // `gemm_axpy_into_view_diag`) only ever call `transpose()` on
        // the right-hand operand, so the materialised owned matrix
        // sits in the same role the existing kernel uses for `b_t`.
        //
        // Implementation: clone the underlying transpose, then
        // overwrite the diagonal of the owned buffer with `self.one`.
        // We can't do that in general (the `Owned` trait surface is
        // read-only), so instead we route through an explicit
        // materialise-via-`get` path. The only caller is
        // `gemm_axpy_into_view_diag` below, which does NOT use the
        // wrapper's `transpose()` — it constructs its own per-cell
        // accumulator. We still implement the method so the trait is
        // satisfied; an unused-method audit is fine to leave it as a
        // delegating default.
        self.inner.transpose()
    }
}

/// Generic axpy-form gemm that accepts **operands with implicit unit
/// diagonals**.
///
/// Same contract as [`gemm_axpy_into_view`] (`out ← α · a · b + β · out`
/// per cell, reading `out[i, j]` BEFORE writing to handle the trsm/trmm
/// `C ≡ out` aliasing) but with explicit per-operand diagonal flags.
/// When `diag_a == UnitDiag::Implicit`, every read of the form
/// `a.get(i, i)` is replaced with `F::one()` (synthesised once via
/// `b.get(0, 0).zero_like().one_like()`); same for `diag_b`. Reads off
/// the diagonal pass through unchanged.
///
/// # Aliasing
///
/// Same per-cell read-then-write rule as [`gemm_axpy_into_view`]: `out`
/// may alias its own `C` operand, but operand views `a` / `b` must not
/// alias `out`'s underlying buffer. The borrow checker enforces this for
/// `MatView` / `MatViewMut` callers.
///
/// # Generic operands
///
/// Unlike [`gemm_axpy_into_view`] (which is hard-bound to `MatView<F>`
/// because it uses the `row_slice` fast path and the
/// `dot_product_slices` delayed-reduction primitive), this kernel takes
/// any [`MatrixLike<F>`] for `a` and `b`. It computes the inner dot
/// product **eagerly per cell** because a unit-diagonal operand has no
/// contiguous slice you can hand to `dot_product_slices` without first
/// materialising the implicit `1`s — and materialising would defeat the
/// point. For the `trtrm` use case the kernel is invoked exactly once
/// per recursion level (not in the inner O(n³) hot path of `trsm` /
/// `trmm`), so the eager-multiply cost is amortised.
///
/// # Complexity
///
/// `O(m · k · n)` field operations. No `B`-transpose scratch is paid
/// because the kernel walks `b` cell-wise via `MatrixLike::get`; the
/// per-cell `get` cost is the same as the existing
/// [`gemm_into_view`] generic path.
///
/// # Used by
///
/// [`crate::field::triangular::trtrm`] for the `A12 = U12 · L22` step
/// where `L22` is unit-lower-triangular with implicit diagonal.
pub(crate) fn gemm_axpy_into_view_diag<F, A, B>(
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
    // Blocked traversal over output tiles, matching `gemm_axpy_into_view`.
    // Inside each tile we compute the inner dot product eagerly, since a
    // unit-diagonal operand cannot expose a contiguous slice for
    // `dot_product_slices`.
    for i_blk in (0..m).step_by(GEMM_ROW_TILE) {
        let i_end = (i_blk + GEMM_ROW_TILE).min(m);
        for j_blk in (0..n).step_by(GEMM_COL_TILE) {
            let j_end = (j_blk + GEMM_COL_TILE).min(n);
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
}

/// Convenience constructor for [`UnitDiagView`]. `pub(crate)` so the
/// triangular module can wrap a `MatrixLike` operand and pass it
/// through to other generic kernels (e.g. `gemm_into_view`) when
/// useful. Currently used only inside this crate.
#[allow(dead_code)]
pub(crate) fn unit_diag_view<F: FiniteField, M: MatrixLike<F> + ?Sized>(
    inner: &M,
    one: F,
) -> UnitDiagView<'_, F, M> {
    UnitDiagView::new(inner, one)
}

// NOTE: The eager `Mul` operator overloads that T1 (`91c06222`) provided
// here have been moved to the expression-template layer in
// `crate::field::expr`. See `dev/plans/expression_templates_design.md` §4.5.
//
// `&a * &b` now returns `Product<&M, &M>`, a lazy proxy; pipe it through
// `.into()` to materialise, or compose it with `+` to reach a canonical
// fusion such as `FusedProductPlus<Product<_, _>, &M>` that dispatches one
// `gemm_with_beta` kernel call.

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
    use crate::gf2m::{Gf2mElement, Gf2mField};
    use crate::gfp::Fp;
    use proptest::prelude::*;

    type F = Fp<7>;
    fn f(v: u64) -> F {
        Fp::<7>::new(v)
    }

    // GF(2^4) with primitive polynomial x^4 + x + 1 = 0b10011; shared by the
    // Gf2mElement-based tests that exercise the `FiniteField`-only paths.
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
        // Round-7 regression: `with_capacity(rows, cols)` must return a
        // `rows × cols` matrix (zero-initialised in safe Rust), not a
        // permanent `0 × 0` matrix with only reserved backing storage.
        let m = FieldMatrix::<F>::with_capacity(4, 5);
        assert_eq!(m.shape(), (4, 5));
        assert_eq!(m.rows(), 4);
        assert_eq!(m.cols(), 5);
        for r in 0..4 {
            for c in 0..5 {
                assert_eq!(m.get(r, c), f(0));
            }
        }
        // Writes through the normal `set()` path must succeed for every
        // advertised cell (the earlier bug caused `set(3, 4, ..)` to panic).
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

    // ─── Left-scalar multiplication parity across every `ConstField` ─────
    //
    // The issue's API surface comment promises `F * M` and `M * F` for
    // every `ConstField`, not only `Fp<P>`. These regression tests lock
    // commutativity (`F * &M == &M * F`) and the owned/ref combos for
    // each concrete `ConstField` in the crate.

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

    // Ext-field test configs reused from the pattern established in
    // `gfpn::ext_config` tests: GF(7²) and GF(7³) with simple non-residues.
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

    // Test config for `Gf2mWide`: GF(2^4) with irreducible x^4 + x + 1.
    // Uses a single-word layout (N = 1). `MODULUS` stores only the low m
    // bits (implicit-leading-one convention documented on
    // `Gf2mWideConfig`), so `x^4 + x + 1` becomes `0b0011` = 3, with the
    // leading `x^4` term implicit at position M = 4.
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

    // Right-scalar multiplication must stay generic for runtime-context
    // fields that are deliberately **not** `ConstField`, such as
    // `Gf2mElement`. The design note (§8 of `dev/active/ab791e27-design.md`)
    // promises both `&M * F` and `M * F` for any `FiniteField`; left-scalar
    // `F * M` is not required here because `Gf2mElement` is not a
    // `ConstField` and the orphan rule blocks a single generic impl.
    #[test]
    fn test_right_scalar_mul_gf2m_element_generic() {
        use crate::matrix_like::MatrixLike;
        let field = gf16();
        // 3×3 non-trivial matrix over GF(2^4) so the element-wise check
        // exercises every row/column at least once.
        let values: &[&[u64]] = &[&[1, 2, 3], &[4, 5, 6], &[7, 8, 9]];
        let m = gf16_mat(&field, values);
        let k = field.element(11); // arbitrary non-zero scalar

        // `&M * F` (ref form) and `M * F` (owned form) must agree — no
        // separate arithmetic path. These return `Scale` proxies whose
        // `MatrixLike::get` implements the multiplication lazily.
        let right_ref = &m * k.clone();
        let right_owned = m.clone() * k.clone();

        // Element-wise cross-check: each entry equals `k * m[r][c]`.
        // `Gf2mElement` multiplication is commutative (GF(2^m) is a
        // commutative field), so `k * m[r][c] == m[r][c] * k`.
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

        // Shape is preserved.
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
        // A 2x3 view transposes to 3x2.
        assert_eq!(t.shape(), (3, 2));
        // Element (0,1)=2 in the view maps to (1,0)=2 in the transpose.
        assert_eq!(t.get(1, 0), f(2));
        // (1,2)=5 maps to (2,1)=5.
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

    // ── Property-based invariants ───────────────────────────────────────────
    //
    // Arithmetic invariants hold for every finite field; we spot-check with
    // two concrete ones:
    //   * `Fp<7>` — the workhorse prime-field testbed.
    //   * `Gf2mElement` in GF(2^4) — exercises the runtime-context path and
    //     ensures the `F: FiniteField` generalisation is not silently prime-
    //     specific.
    //
    // Dimensions are kept ≤ 6 so each proptest case remains well under the
    // 5s per-test nextest budget even for the `n³` `Mul` paths.

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

    // ─── Degenerate-dimension correctness tests ───────────────────────────

    #[test]
    fn test_gemm_m_times_zero_times_zero_times_n_returns_zero_matrix() {
        // (m=3, k=0) * (k=0, n=2) on a ConstField. Expected: 3×2 zero matrix
        // with backing storage of length 6, not an inconsistent empty buffer.
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
        // Storage invariant: data.len() == rows * cols. Accessing every
        // (r, c) above already exercises this through `FieldMatrix::get`,
        // which indexes `data[r * cols + c]`.
    }

    #[test]
    fn test_gemm_empty_outer_dim_returns_empty_storage() {
        // (0, k) * (k, n) and (m, k) * (k, 0) on the non-ConstField path.
        // The zero-outer-dim short circuit in `gemm` must NOT panic for
        // Gf2mElement even though we cannot synthesise a standalone zero;
        // the output carries an empty `FieldVec` because `rows * cols == 0`.
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
        // (3, 0) * (0, 2) on Gf2mElement. Both factors are empty, so gemm
        // has no `F` witness to materialise the 3×2 zero output and must
        // panic with the documented message. With the expression-template
        // layer the panic fires at evaluation time (on `gemm(&a, &b)`), not
        // at proxy construction.
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
        // (3, 0) * length-0 vec on Fp<7> returns length-3 zero vector.
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
        // (0, 3)ᵀ * length-0 vec on Fp<7> returns length-3 zero vector.
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
        // (3, 0) on Gf2mElement. matvec has no zero witness and must panic.
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
        // (0, 3) on Gf2mElement. matvec_transpose has no zero witness and must
        // panic because it cannot seed the length-3 output without a row to
        // borrow an element from, and Gf2mElement is not ConstField.
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
            // Runtime-context field: compare element-wise via MatrixLike.
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
            // (A+B)+C and A+(B+C) — element-wise, no ConstField available.
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
            // Distributivity over a runtime-context field: use `gemm`
            // directly (no `.into()` bridge for non-ConstField).
            let bc_proxy = &b + &c;
            // Materialise `b+c` via a helper.
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

    // Helper: materialise a `MatrixLike` proxy over runtime-context
    // `Gf2mElement` into an owned `FieldMatrix`. The public `From<Expr>`
    // bridge is `ConstField`-only; this helper fills the runtime-context gap.
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

    // ─── 91c06222: blocked gemm regression suite ──────────────────────────
    //
    // These tests lock the Dumas–Pernet §1.2 contract:
    //   1. `gemm` agrees with the naive triple loop on every field this
    //      crate models. (`Fp<7>`, `Fp<65521>`, Mersenne-31 `Fp<2^31-1>`,
    //      Gf2mElement GF(2^8), Gf2mWide<1, _> GF(2^8) via a config.)
    //   2. Operators eagerly allocate their result in all four
    //      owned/ref combinations.
    //   3. Block-boundary arithmetic is correct when dims straddle
    //      `GEMM_ROW_TILE` / `GEMM_COL_TILE`.
    //   4. Delayed-reduction chunking is correct when the inner dimension
    //      exceeds `F::max_unreduced_additions()` (forces at least one
    //      mid-dot-product reduce).
    //
    // Everything stays inside the 5-second nextest budget.

    use crate::field::FiniteField;

    // Test config for an 8-bit binary field via `Gf2mWide`. GF(2^8) with the
    // AES irreducible `x^8 + x^4 + x^3 + x + 1` (implicit leading bit ⇒ low
    // byte stores `0x1B`). Declared outside the macro'd ConstField family
    // so we can exercise the ConstField path for matrix mul.
    struct MatGf2m8AesCfg;
    impl crate::gf2m::Gf2mWideConfig<1> for MatGf2m8AesCfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "MatGf2m8AesCfg";
    }
    type Gf2m8 = crate::gf2m::Gf2mWide<1, MatGf2m8AesCfg>;

    /// Naive triple-loop gemm used as a reference for cross-checks. Every
    /// multiply is reduced immediately so this path deliberately avoids the
    /// `Wide` accumulator — it is the baseline the delayed-reduction path
    /// must match.
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
        // 16-bit prime. Exercises u128 wide accumulator with room to spare.
        for (m, k, n) in [(1, 1, 1), (3, 5, 2), (7, 11, 5)] {
            let a = random_fp_matrix::<65521>(m, k, 0xCAFEu64 ^ (m * k) as u64);
            let b = random_fp_matrix::<65521>(k, n, 0xBEEFu64 ^ (k * n) as u64);
            let got: FieldMatrix<Fp<65521>> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_matches_naive_fp_mersenne31() {
        // 2^31 - 1. Close to the upper edge of u32 where kmax is a few
        // million, so inner dims stay well inside one chunk.
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
        // GF(2^8) via `Gf2mWide`. XOR accumulator, kmax = usize::MAX.
        for (m, k, n) in [(1, 1, 1), (3, 5, 2), (7, 11, 5)] {
            let a = random_gf2m8_matrix(m, k, 0xF00Du64 ^ (m * k) as u64);
            let b = random_gf2m8_matrix(k, n, 0x1234u64 ^ (k * n) as u64);
            let got: FieldMatrix<Gf2m8> = (&a * &b).into();
            assert_eq!(got, naive_gemm(&a, &b), "{}x{}x{}", m, k, n);
        }
    }

    #[test]
    fn test_gemm_matches_naive_gf2_16_const() {
        // GF(2^16) via a dedicated Gf2mWide config. Exercises wider storage
        // but the same XOR-only delayed-reduction branch.
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
            // Covers skinny-inner development cells plus output tile
            // boundaries around 32 rows / 64 columns. The exact
            // 64c88ae4 skinny-output shapes are covered separately below
            // with a structured diagonal-left reference.
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
    }

    #[test]
    fn test_gemm_matches_naive_gf2m_element_runtime() {
        // Gf2mElement is the runtime-context field. Cross-check over
        // GF(2^4) with polynomial x^4 + x + 1. Runtime-context fields have
        // no `ConstField` bridge, so call `gemm` directly.
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
        // Dims straddle the GEMM_ROW_TILE (32) and GEMM_COL_TILE (64)
        // boundaries. A correctness bug in the tile-clamping (`i_end` /
        // `j_end`) would surface here.
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
        // (2 × 1001) * (1001 × 2) stresses the inner dot-product path with
        // a very deep k, exercising the multi-chunk reduction branch for
        // small primes (kmax is effectively unbounded here but the branch
        // still produces the right answer for long runs).
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
        // Build a matrix whose inner dim crosses kmax for a prime where
        // kmax is small enough to actually hit the multi-chunk code path.
        // For `Fp<9_223_372_036_854_775_783>` (near 2^63), kmax is a small
        // handful; the dot-product kernel *must* reduce at the boundary.
        //
        // We construct inputs `a[0,k] = 1`, `b[k,0] = 1` for all k, and
        // require `out[0,0] == k`. This is the cleanest numerical witness
        // that bounded accumulation didn't drop any terms.
        const P: u64 = 9_223_372_036_854_775_783;
        type Fpx = Fp<P>;
        let kmax = <Fpx as FiniteField>::max_unreduced_additions();
        assert!(kmax >= 1, "sanity: kmax must permit at least one product");
        assert!(
            kmax < 100,
            "sanity: this field should have a small kmax for the chunking path"
        );
        // Choose inner dim just above 2*kmax so we hit at least three
        // chunks; the last chunk is deliberately short (size 1) to cover
        // the `remaining.min(kmax)` clamp.
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

        // Same invariant at *exactly* kmax products — exercises the single-
        // chunk path where `remaining == kmax` on the first iteration.
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
        // Fp<7>, 3x3 square. The four combinations must yield the same
        // matrix; this catches any accidental divergence in the Mul impls.
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
        // And it is self-inverse: -(-a) == a.
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

        /// Blocked gemm must equal naive gemm over Fp<65521>.
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

        /// Mul is associative over Fp<7>: (A*B)*C == A*(B*C).
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

        /// Right distributivity over Fp<7>: (A + B) * C == A*C + B*C.
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

        /// Mul matches naive over GF(2^8) via Gf2mWide.
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

    // ─── gemm_axpy_into_view_diag (R4) — implicit-unit-diagonal kernel ────

    const MERSENNE_31: u64 = 2_147_483_647;

    /// Materialise `a` with an explicit unit diagonal (overwrites the
    /// `[i, i]` cell with `F::one()`) so the result can be fed to a
    /// stock gemm reference that has no implicit-diag concept.
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
        // Build a/b with explicit unit-diag cells so the stored cells
        // happen to coincide with `F::one()` only when stored ≡ implicit.
        // For the implicit case we *deliberately* poison the diagonal
        // with garbage to certify the kernel does not read those cells.
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
            // Poison: stuff random non-1 values into the diagonal of a.
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
        // Compute the kernel result.
        let mut got = out0.clone();
        gemm_axpy_into_view_diag(diag_a, alpha, &a, diag_b, &b, beta, got.submat_mut(.., ..));
        // Reference: materialise the unit-diag operands explicitly,
        // then run the naive axpy.
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
        // Sanity: with both diagonals stored, results must match the
        // existing axpy-into-view exactly (cross-check the new kernel
        // against the older one for the all-Stored case).
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
        // The trtrm A12 = U12 · L22 use case: b carries a logical unit
        // diagonal but the storage is poisoned.
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
        // Run the kernel over GF(2^8) via Gf2mWide so the SIMD-ish path is
        // exercised end-to-end with a unit-diag operand.
        let cases: &[(usize, usize, usize)] =
            &[(2, 2, 2), (3, 5, 4), (4, 7, 7), (5, 5, 5), (8, 8, 3)];
        for &(m, k, n) in cases {
            let mut rng = {
                use rand::SeedableRng;
                rand::rngs::StdRng::seed_from_u64(0xF0 + (m * k * n) as u64)
            };
            let a = random_gf2m8_matrix(m, k, 0xF0 + (m * k * n) as u64);
            let mut b = random_gf2m8_matrix(k, n, 0xF1 + (m * k * n) as u64);
            // Poison b's diagonal so we know the kernel ignores it.
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

        /// Property: with both diagonals stored, the new kernel agrees
        /// with the naive axpy reference.
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

        /// Property: implicit-diag b agrees with materialised reference.
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

        /// Property: implicit-diag a agrees with materialised reference.
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
        // k = 0: A·B = 0 ⇒ out ← β · out only, regardless of diag flags.
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
        // m = 0 or n = 0: no work, no panic.
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
        // Just verify it didn't panic and produced an empty result.
        assert_eq!(out.shape(), (0, 4));
    }
}
