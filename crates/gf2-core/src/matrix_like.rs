//! Shared matrix surface of [`BitMatrix`](crate::matrix::BitMatrix) and
//! [`FieldMatrix<F>`](crate::field::matrix::FieldMatrix): [`MatrixLike`] for
//! reads and [`MatrixLikeMut`] for writes, so that an immutable view
//! implements the read half only.

/// Shared read-only matrix surface.
///
/// Every matrix-shaped type in `gf2-core` that exposes a *row-major* 2-D
/// indexing model implements this trait.  The element type is generic so that
/// [`BitMatrix`](crate::matrix::BitMatrix) can implement `MatrixLike<bool>`
/// and [`FieldMatrix<F>`](crate::field::matrix::FieldMatrix) can implement
/// `MatrixLike<F>`.
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

// Lets a generic `A: MatrixLike<Elem>` operand be a reference; `Owned` is the
// referent's, so `(&m).transpose()` returns the same owned type.
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
