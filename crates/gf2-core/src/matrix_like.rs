//! Shared matrix surface used by both [`BitMatrix`](crate::matrix::BitMatrix)
//! and [`FieldMatrix<F>`](crate::field::matrix::FieldMatrix).
//!
//! Generic algorithms (Gauss-Jordan, PLE, solve, rank) are written against
//! [`MatrixLike`] / [`MatrixLikeMut`] so they can run unchanged on dense
//! matrices and on zero-copy submatrix views.
//!
//! # Read / write split
//!
//! The surface is split in two:
//!
//! - [`MatrixLike<Elem>`] — read-only operations: shape, element read, copying
//!   transpose.
//! - [`MatrixLikeMut<Elem>`] — adds the mutators (`set`, `swap_rows`) and
//!   requires [`MatrixLike<Elem>`] as a super-trait.
//!
//! The split means that an immutable `MatView<'_, F>` can implement the
//! read-only trait without having to `panic!("read-only")` on the mutators.
//! See `dev/active/ab791e27-design.md` for the design rationale.

/// Shared read-only matrix surface.
///
/// Every matrix-shaped type in `gf2-core` that exposes a *row-major* 2-D
/// indexing model implements this trait.  The element type is generic so that
/// [`BitMatrix`](crate::matrix::BitMatrix) can implement `MatrixLike<bool>`
/// and [`FieldMatrix<F>`](crate::field::matrix::FieldMatrix) can implement
/// `MatrixLike<F>`.
///
/// # Examples
///
/// ```
/// use gf2_core::matrix::BitMatrix;
/// use gf2_core::matrix_like::MatrixLike;
///
/// let m = BitMatrix::identity(4);
/// assert_eq!(m.rows(), 4);
/// assert_eq!(m.cols(), 4);
/// assert_eq!(MatrixLike::<bool>::get(&m, 0, 0), true);
/// assert_eq!(MatrixLike::<bool>::shape(&m), (4, 4));
/// assert!(MatrixLike::<bool>::is_square(&m));
/// ```
pub trait MatrixLike<Elem> {
    /// Number of rows.
    fn rows(&self) -> usize;

    /// Number of columns.
    fn cols(&self) -> usize;

    /// Value at `(row, col)`.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()` or `col >= self.cols()`.
    fn get(&self, row: usize, col: usize) -> Elem;

    /// Returns a fresh owned matrix that is the transpose of `self`.
    fn transpose(&self) -> Self
    where
        Self: Sized;

    /// Returns `(rows, cols)`.
    #[inline]
    fn shape(&self) -> (usize, usize) {
        (self.rows(), self.cols())
    }

    /// Returns `true` if the matrix is square.
    #[inline]
    fn is_square(&self) -> bool {
        self.rows() == self.cols()
    }

    /// Returns `true` if either dimension is zero.
    #[inline]
    fn is_empty(&self) -> bool {
        self.rows() == 0 || self.cols() == 0
    }
}

/// Mutating extension of [`MatrixLike`].
///
/// Types that support in-place element writes and row swaps implement this in
/// addition to [`MatrixLike`]. Read-only views (e.g. `MatView`) deliberately
/// do not.
///
/// # Examples
///
/// ```
/// use gf2_core::matrix::BitMatrix;
/// use gf2_core::matrix_like::{MatrixLike, MatrixLikeMut};
///
/// let mut m = BitMatrix::zeros(3, 3);
/// MatrixLikeMut::<bool>::set(&mut m, 0, 1, true);
/// assert!(MatrixLike::<bool>::get(&m, 0, 1));
///
/// MatrixLikeMut::<bool>::swap_rows(&mut m, 0, 1);
/// assert!(MatrixLike::<bool>::get(&m, 1, 1));
/// assert!(!MatrixLike::<bool>::get(&m, 0, 1));
/// ```
pub trait MatrixLikeMut<Elem>: MatrixLike<Elem> {
    /// Writes `v` at `(row, col)`.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.rows()` or `col >= self.cols()`.
    fn set(&mut self, row: usize, col: usize, v: Elem);

    /// Swaps rows `r1` and `r2`. A no-op when `r1 == r2`.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of range.
    fn swap_rows(&mut self, r1: usize, r2: usize);
}
