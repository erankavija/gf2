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
//! See `dev/active/ab791e27-design-fieldmatrix-f-finitefield-dense-matrix-ty/ab791e27-design.md` for the design rationale.
//!
//! # Owned transpose
//!
//! [`MatrixLike::transpose`] returns an [`MatrixLike::Owned`] matrix rather
//! than `Self`. Concrete row-major matrices (`BitMatrix`, `FieldMatrix<F>`)
//! set `Owned = Self` and return the obvious in-kind transpose; zero-copy
//! views instead materialise a fresh owned matrix of the parent type. This
//! keeps the trait method total (no panicking impl) while still allowing
//! generic code to obtain a transpose from any `MatrixLike` without assuming
//! ownership.

/// Shared read-only matrix surface.
///
/// Every matrix-shaped type in `gf2-core` that exposes a *row-major* 2-D
/// indexing model implements this trait.  The element type is generic so that
/// [`BitMatrix`](crate::matrix::BitMatrix) can implement `MatrixLike<bool>`
/// and [`FieldMatrix<F>`](crate::field::matrix::FieldMatrix) can implement
/// `MatrixLike<F>`.
///
/// # Owned associated type
///
/// [`MatrixLike::transpose`] returns `Self::Owned`, not `Self`. For concrete
/// matrix types `Owned = Self`; for borrow-only views `Owned` is the parent
/// owned matrix type, so the view's transpose materialises a fresh copy.
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
    /// Owned matrix type produced by [`transpose`](Self::transpose).
    ///
    /// Concrete owned matrices set this to `Self`. Borrow-only views set it
    /// to the parent owned type (e.g. `MatView<'_, F>::Owned = FieldMatrix<F>`).
    type Owned: MatrixLike<Elem>;

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

    /// Returns a freshly allocated owned matrix that is the transpose of
    /// `self`.
    ///
    /// Views materialise a new `Self::Owned` because a row-major slice cannot
    /// be reinterpreted in-place as column-major without data motion.
    fn transpose(&self) -> Self::Owned;

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

// Reference-forwarding impl — lets proxy algebra compose over `&FieldMatrix<F>`
// as a `MatrixLike<Elem>` operand. Without this, `Product<&FieldMatrix<F>,
// &FieldMatrix<F>>` cannot satisfy its `A: MatrixLike<F>` bound because the
// concrete `MatrixLike<F>` impl lives on `FieldMatrix<F>`, not `&FieldMatrix<F>`.
//
// The blanket delegates every method to the borrowed operand; `Owned` matches
// the underlying matrix's `Owned` so `(&m).transpose()` still returns an
// owned result of the natural kind.
impl<Elem, T: MatrixLike<Elem> + ?Sized> MatrixLike<Elem> for &T {
    type Owned = T::Owned;

    #[inline]
    fn rows(&self) -> usize {
        (**self).rows()
    }

    #[inline]
    fn cols(&self) -> usize {
        (**self).cols()
    }

    #[inline]
    fn get(&self, row: usize, col: usize) -> Elem {
        (**self).get(row, col)
    }

    #[inline]
    fn transpose(&self) -> Self::Owned {
        (**self).transpose()
    }
}
