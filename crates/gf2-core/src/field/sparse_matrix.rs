//! Placeholder for the sparse finite-field matrix type.
//!
//! The full CSR/CSC sparse representation and its API surface are owned by
//! story `8a90882e` in epic `bb85c68a`. The minimal [`SparseFieldMatrix<F>`]
//! defined here exists solely so that [`FieldMatrix::to_sparse`](super::matrix::FieldMatrix::to_sparse)
//! can carry the signature required by the `ab791e27` contract without
//! depending on the yet-to-be-implemented sparse module.
//!
//! **Do not rely on the representation here.** When `8a90882e` lands this
//! type will be replaced wholesale — likely with a CSR-style layout and a
//! much larger operator surface. Downstream code that imports
//! `SparseFieldMatrix` should either (a) wait for `8a90882e` to land, or
//! (b) treat this type as opaque and interact with it only via the
//! conversion helpers provided by `FieldMatrix`.
//
// 8a90882e — owning story for the full sparse surface.

use crate::field::FiniteField;

/// Sparse dense-equivalent matrix over a [`FiniteField`].
///
/// Stored as a triplet list `(row, col, value)` of non-zero entries.
/// This shape is intentionally minimal; story `8a90882e` replaces it with
/// a CSR layout and adds arithmetic, matvec, conversion helpers, and
/// iterator adaptors.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::sparse_matrix::SparseFieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 3);
/// m.set(0, 1, Fp::<7>::new(5));
/// m.set(1, 2, Fp::<7>::new(3));
/// let s: SparseFieldMatrix<Fp<7>> = m.to_sparse();
/// assert_eq!(s.shape(), (2, 3));
/// assert_eq!(s.nnz(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct SparseFieldMatrix<F: FiniteField> {
    rows: usize,
    cols: usize,
    // 8a90882e will replace this with (row_ptr, col_idx, values) CSR arrays.
    triplets: Vec<(usize, usize, F)>,
}

impl<F: FiniteField> SparseFieldMatrix<F> {
    /// Constructs a sparse matrix from `rows × cols` plus a triplet list of
    /// non-zero entries. Crate-private because the public entry-point is
    /// [`FieldMatrix::to_sparse`](super::matrix::FieldMatrix::to_sparse); the
    /// final public constructors will be re-designed in story `8a90882e`.
    pub(crate) fn from_dense_stub(
        rows: usize,
        cols: usize,
        triplets: Vec<(usize, usize, F)>,
    ) -> Self {
        Self {
            rows,
            cols,
            triplets,
        }
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
    /// let s = m.to_sparse();
    /// assert_eq!(s.shape(), (3, 5));
    /// ```
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Number of rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(4, 2);
    /// assert_eq!(m.to_sparse().rows(), 4);
    /// ```
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = FieldMatrix::<Fp<7>>::zeros(4, 2);
    /// assert_eq!(m.to_sparse().cols(), 2);
    /// ```
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of stored non-zero entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 3);
    /// m.set(0, 0, Fp::<7>::new(1));
    /// m.set(2, 2, Fp::<7>::new(5));
    /// assert_eq!(m.to_sparse().nnz(), 2);
    /// ```
    pub fn nnz(&self) -> usize {
        self.triplets.len()
    }

    /// Returns a slice over the stored `(row, col, value)` triplets.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// m.set(1, 0, Fp::<7>::new(4));
    /// let s = m.to_sparse();
    /// let t = s.triplets();
    /// assert_eq!(t.len(), 1);
    /// assert_eq!(t[0].0, 1);
    /// assert_eq!(t[0].1, 0);
    /// assert_eq!(t[0].2, Fp::<7>::new(4));
    /// ```
    pub fn triplets(&self) -> &[(usize, usize, F)] {
        &self.triplets
    }
}
