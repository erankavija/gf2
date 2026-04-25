//! Block-recursive triangular primitives — `trsm`, `trmm`, `trtri`, `trtrm`.
//!
//! This module implements Dumas–Pernet §2.1 algorithms 2.1–2.4 on top of the
//! existing classical [`gemm`](crate::field::matrix::gemm) (issue
//! `91c06222`) and the dense view types
//! [`MatView`](crate::field::matrix::MatView) /
//! [`MatViewMut`](crate::field::matrix::MatViewMut). All routines operate
//! **in place** on the supplied views: no extra `FieldMatrix<F>` is
//! materialised beyond the unavoidable scratch the recursive `gemm` calls
//! allocate for their owned outputs.
//!
//! # Routines
//!
//! - [`trsm_upper`] / [`trsm_lower`] — solve `A · X = B` for triangular `A`,
//!   overwriting `B` with `X` (algorithm 2.1).
//! - [`trmm_upper`] / [`trmm_lower`] — overwrite `B` with `A · B` for
//!   triangular `A` (algorithm 2.2).
//! - [`trtri_upper`] / [`trtri_lower`] — invert a triangular matrix in
//!   place (algorithm 2.3).
//! - [`trtrm`] — compute the in-place product `L · U` where `L` is unit
//!   lower-triangular and `U` is upper-triangular, used by the PLE
//!   decomposition (algorithm 2.4).
//!
//! # Recursion structure
//!
//! Each primitive splits its leading triangular operand at `h = m / 2` into
//! a 2×2 block layout
//!
//! ```text
//!     ┌─────┬─────┐
//!     │ A11 │ A12 │     B = ┌────┐
//! A = │     │     │         │ B1 │
//!     ├─────┼─────┤         ├────┤
//!     │  0  │ A22 │         │ B2 │
//!     └─────┴─────┘         └────┘
//! ```
//!
//! and recurses on `A11`/`A22` plus a single `gemm` on the off-diagonal
//! block. Below [`FiniteField::TRI_BASE_THRESHOLD`] the recursion peels
//! down to the corresponding direct loop (back-substitution for `trsm`,
//! schoolbook for `trmm`/`trtri`).
//!
//! ## `trsm_upper(A, B)` (algorithm 2.1)
//!
//! ```text
//!     trsm_upper(A22, B2)        // recurse on the lower half
//!     B1 -= A12 · B2             // gemm with α = −1, β = +1
//!     trsm_upper(A11, B1)        // recurse on the upper half
//! ```
//!
//! `trsm_lower` is the mirror image (top half first, then `B2 -= A21 · B1`,
//! then bottom half).
//!
//! ## `trmm_upper(A, B)` (algorithm 2.2)
//!
//! ```text
//!     trmm_upper(A11, B1)        // recurse on the upper half
//!     B1 += A12 · B2             // gemm with α = +1, β = +1
//!     trmm_upper(A22, B2)        // recurse on the lower half
//! ```
//!
//! ## `trtri_upper(A)` (algorithm 2.3)
//!
//! ```text
//!     trtri_upper(A11)           // recursive
//!     trtri_upper(A22)           // recursive
//!     A12 := −A11 · A12 · A22    // two off-diagonal multiplies + sign
//! ```
//!
//! ## `trtrm(L, U)` (algorithm 2.4)
//!
//! Computes the in-place product `L ← L · U` where `L` is unit
//! lower-triangular (the diagonal is implicitly `1` and is **not** read by
//! the routine) and `U` is upper-triangular. The output overwrites `L` and
//! is the dense product. This is the convention used by Dumas–Pernet §3
//! PLE so the in-place compression in issue `c3f8c1cb` can call it
//! directly.
//!
//! # Allocation budget
//!
//! The trsm / trmm path uses a crate-private fused helper
//! [`submul_into_view`] / [`addmul_into_view`] that performs `C ± A · B`
//! directly on a `MatViewMut` **without** materialising an intermediate
//! `A · B` matrix. The trtri / trtrm paths necessarily materialise one
//! `FieldMatrix<F>` per off-diagonal multiply (because [`gemm`] returns an
//! owned matrix); the allocation footprint per recursion level is
//! `O((m/2)²)` cells, geometrically summing to `O(m²)` over the full
//! recursion tree — the same asymptotic budget as the existing Strassen
//! recursion in [`crate::field::winograd`].
//!
//! # Bit-exact correctness
//!
//! Because every off-diagonal step delegates to the classical [`gemm`] and
//! the base-case loops use only field add/sub/mul/div, the output is
//! **bit-exact** equal to the equivalent `gemm`-of-dense expansion at
//! every recursion depth. The proptests in this module exercise the four
//! primitives across `Fp<7>`, `Fp<MERSENNE_31>`, and `Gf2mWide<8>`,
//! straddling the threshold and odd recursion splits.
//!
//! # Singular matrices
//!
//! `trsm_upper` / `trsm_lower` panic with a clear message on a zero
//! diagonal pivot, and `trtri_upper` / `trtri_lower` panic when any
//! diagonal element fails to invert (i.e. the input matrix is singular).
//! `trmm` and `trtrm` make no inversion calls and so cannot fail on a
//! singular argument.

use crate::field::matrix::{gemm, FieldMatrix, MatView, MatViewMut};
use crate::field::FiniteField;

// ─── Public API ─────────────────────────────────────────────────────────────

/// Solves the upper-triangular linear system `A · X = B` in place,
/// overwriting `b` with the solution `X`.
///
/// Implements Dumas–Pernet §2.1 algorithm 2.1 (the upper variant) via a
/// block-recursive split that peels at `h = m / 2`, recurses on the lower
/// half `A22 · X2 = B2`, then folds in the off-diagonal contribution
/// `B1 -= A12 · X2` and recurses on the upper half `A11 · X1 = B1`. Below
/// [`FiniteField::TRI_BASE_THRESHOLD`] the recursion drops into a direct
/// back-substitution loop.
///
/// # Arguments
///
/// * `a` — Square `m × m` upper-triangular view. Cells strictly below the
///   diagonal are not read; the diagonal cells are read once each per
///   division.
/// * `b` — Right-hand-side `m × n` view; on return holds `X = A⁻¹ · B`.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
/// * Panics with `trsm_upper: zero pivot at A[i, i] = 0 — matrix is singular`
///   if any diagonal element is the field zero.
///
/// # Complexity
///
/// `O(m² · n)` field operations, identical to the classical
/// back-substitution; the recursive split shifts constants but not the
/// asymptotic profile.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::triangular::trsm_upper;
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 2], [0, 3]]  (upper triangular, GF(7))
/// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// a.set(0, 0, Fp::<7>::new(1));
/// a.set(0, 1, Fp::<7>::new(2));
/// a.set(1, 1, Fp::<7>::new(3));
/// // B = [[1], [3]]
/// let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
/// b.set(0, 0, Fp::<7>::new(1));
/// b.set(1, 0, Fp::<7>::new(3));
/// trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
/// // Expect X with A·X = B: x1 = 3/3 = 1, x0 = 1 - 2·1 = -1 ≡ 6 (mod 7).
/// assert_eq!(b.get(1, 0), Fp::<7>::new(1));
/// assert_eq!(b.get(0, 0), Fp::<7>::new(6));
/// ```
pub fn trsm_upper<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_upper: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_upper: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    trsm_upper_inner(a, b);
}

/// Solves the lower-triangular linear system `A · X = B` in place,
/// overwriting `b` with the solution `X`.
///
/// Mirror of [`trsm_upper`]; recurses top-half first, folds
/// `B2 -= A21 · X1`, then recurses bottom-half. Implements Dumas–Pernet
/// §2.1 algorithm 2.1 (the lower variant).
///
/// # Arguments
///
/// * `a` — Square `m × m` lower-triangular view. Cells strictly above the
///   diagonal are not read.
/// * `b` — Right-hand-side `m × n` view; on return holds `X = A⁻¹ · B`.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
/// * Panics with `trsm_lower: zero pivot at A[i, i] = 0 — matrix is singular`
///   on a zero diagonal element.
///
/// # Complexity
///
/// `O(m² · n)` field operations.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::triangular::trsm_lower;
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 0], [2, 3]]  (lower triangular, GF(7))
/// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// a.set(0, 0, Fp::<7>::new(1));
/// a.set(1, 0, Fp::<7>::new(2));
/// a.set(1, 1, Fp::<7>::new(3));
/// // B = [[1], [3]]  → expect x0 = 1, x1 = (3 - 2·1)/3 = 1/3 ≡ 5 (mod 7).
/// let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
/// b.set(0, 0, Fp::<7>::new(1));
/// b.set(1, 0, Fp::<7>::new(3));
/// trsm_lower(a.submat(.., ..), b.submat_mut(.., ..));
/// assert_eq!(b.get(0, 0), Fp::<7>::new(1));
/// assert_eq!(b.get(1, 0), Fp::<7>::new(5));
/// ```
pub fn trsm_lower<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_lower: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_lower: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    trsm_lower_inner(a, b);
}

/// Multiplies `B ← A · B` in place for upper-triangular `A`.
///
/// Implements Dumas–Pernet §2.1 algorithm 2.2 (the upper variant): recurse
/// on the upper half `B1 ← A11 · B1`, fold `B1 += A12 · B2`, then recurse
/// on the lower half `B2 ← A22 · B2`. Below
/// [`FiniteField::TRI_BASE_THRESHOLD`] the recursion drops into a direct
/// schoolbook loop ordered to preserve in-place semantics (rows are
/// updated top-down, accumulating into a per-cell scalar before writing).
///
/// # Arguments
///
/// * `a` — Square `m × m` upper-triangular view. Cells strictly below the
///   diagonal are not read.
/// * `b` — `m × n` view; on return holds `A · B`.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
///
/// # Complexity
///
/// `O(m² · n)` field operations.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::triangular::trmm_upper;
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 2], [0, 3]]  → A·B for B = [[4],[5]] is [[4 + 10],[15]] = [[14],[15]] mod 7 = [[0],[1]].
/// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// a.set(0, 0, Fp::<7>::new(1));
/// a.set(0, 1, Fp::<7>::new(2));
/// a.set(1, 1, Fp::<7>::new(3));
/// let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
/// b.set(0, 0, Fp::<7>::new(4));
/// b.set(1, 0, Fp::<7>::new(5));
/// trmm_upper(a.submat(.., ..), b.submat_mut(.., ..));
/// assert_eq!(b.get(0, 0), Fp::<7>::new(0));
/// assert_eq!(b.get(1, 0), Fp::<7>::new(1));
/// ```
pub fn trmm_upper<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trmm_upper: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trmm_upper: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    trmm_upper_inner(a, b);
}

/// Multiplies `B ← A · B` in place for lower-triangular `A`.
///
/// Mirror of [`trmm_upper`]; recurses bottom-half first, folds
/// `B2 += A21 · B1`, then recurses top-half. Implements Dumas–Pernet §2.1
/// algorithm 2.2 (the lower variant).
///
/// # Arguments
///
/// * `a` — Square `m × m` lower-triangular view. Cells strictly above the
///   diagonal are not read.
/// * `b` — `m × n` view; on return holds `A · B`.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
///
/// # Complexity
///
/// `O(m² · n)` field operations.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::triangular::trmm_lower;
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 0], [2, 3]]  → A·B for B = [[4],[5]] is [[4],[8 + 15]] = [[4],[23]] mod 7 = [[4],[2]].
/// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// a.set(0, 0, Fp::<7>::new(1));
/// a.set(1, 0, Fp::<7>::new(2));
/// a.set(1, 1, Fp::<7>::new(3));
/// let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
/// b.set(0, 0, Fp::<7>::new(4));
/// b.set(1, 0, Fp::<7>::new(5));
/// trmm_lower(a.submat(.., ..), b.submat_mut(.., ..));
/// assert_eq!(b.get(0, 0), Fp::<7>::new(4));
/// assert_eq!(b.get(1, 0), Fp::<7>::new(2));
/// ```
pub fn trmm_lower<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trmm_lower: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trmm_lower: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    trmm_lower_inner(a, b);
}

/// Inverts an upper-triangular matrix in place.
///
/// Implements Dumas–Pernet §2.1 algorithm 2.3 (the upper variant):
/// recursively invert `A11` and `A22`, then overwrite `A12` with
/// `−A11 · A12 · A22`. The cells strictly below the diagonal are not
/// touched. Below [`FiniteField::TRI_BASE_THRESHOLD`] the recursion drops
/// into a column-by-column direct inversion that mirrors the forward
/// step of back-substitution against the identity.
///
/// # Arguments
///
/// * `a` — Square `m × m` upper-triangular view; on return holds
///   `A⁻¹` (still upper-triangular).
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics with `trtri_upper: zero pivot at A[i, i] = 0 — matrix is singular`
///   if any diagonal cell is the field zero (i.e. fails to invert).
///
/// # Complexity
///
/// `O(m³)` field operations.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::field::triangular::trtri_upper;
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 2], [0, 3]] over GF(7) — invert and check A · A⁻¹ = I.
/// let original = {
///     let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
///     a.set(0, 0, Fp::<7>::new(1));
///     a.set(0, 1, Fp::<7>::new(2));
///     a.set(1, 1, Fp::<7>::new(3));
///     a
/// };
/// let mut a_inv = original.clone();
/// trtri_upper(a_inv.submat_mut(.., ..));
/// let prod = gemm(&original, &a_inv);
/// let id = FieldMatrix::<Fp<7>>::identity(2);
/// assert_eq!(prod, id);
/// ```
pub fn trtri_upper<F: FiniteField>(a: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trtri_upper: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    trtri_upper_inner(a);
}

/// Inverts a lower-triangular matrix in place.
///
/// Mirror of [`trtri_upper`]: cells strictly above the diagonal are not
/// touched, and the off-diagonal block becomes `−A22⁻¹ · A21 · A11⁻¹` after
/// the two recursive inversions.
///
/// # Arguments
///
/// * `a` — Square `m × m` lower-triangular view; on return holds
///   `A⁻¹` (still lower-triangular).
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics with `trtri_lower: zero pivot at A[i, i] = 0 — matrix is singular`
///   if any diagonal cell is the field zero.
///
/// # Complexity
///
/// `O(m³)` field operations.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::field::triangular::trtri_lower;
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 0], [2, 3]] over GF(7).
/// let original = {
///     let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
///     a.set(0, 0, Fp::<7>::new(1));
///     a.set(1, 0, Fp::<7>::new(2));
///     a.set(1, 1, Fp::<7>::new(3));
///     a
/// };
/// let mut a_inv = original.clone();
/// trtri_lower(a_inv.submat_mut(.., ..));
/// let prod = gemm(&original, &a_inv);
/// let id = FieldMatrix::<Fp<7>>::identity(2);
/// assert_eq!(prod, id);
/// ```
pub fn trtri_lower<F: FiniteField>(a: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trtri_lower: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    trtri_lower_inner(a);
}

/// In-place product of a unit lower-triangular `L` with an upper-triangular
/// `U`.
///
/// Computes the dense `m × m` product `L · U` and writes it back into the
/// `l` view. `L` is treated as **unit** lower-triangular: the diagonal
/// cells are implicitly `1` and the routine does not read them. Cells
/// strictly above `L`'s diagonal are likewise not read. `U` is upper
/// triangular; cells strictly below `U`'s diagonal are not read.
///
/// This is the §2.1 algorithm 2.4 convention used by Dumas–Pernet's PLE
/// decomposition (issue `c3f8c1cb`): the post-pivot in-place product
/// reconstitutes the dense matrix from its compressed `[L \ U]` storage.
///
/// # Arguments
///
/// * `l` — Square `m × m` view holding the **strictly lower-triangular**
///   part of `L`. On return contains the dense product `L · U`.
/// * `u` — Square `m × m` upper-triangular view (unmodified).
///
/// # Panics
///
/// * Panics if `l.rows() != l.cols()`, `u.rows() != u.cols()`, or
///   `l.rows() != u.rows()`.
///
/// # Complexity
///
/// `O(m³)` field operations — same asymptotic cost as a dense `gemm`,
/// halved by the triangular structure.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::field::triangular::trtrm;
/// use gf2_core::gfp::Fp;
///
/// // L = [[1, 0], [4, 1]]  (unit lower; the diagonal is implicit so we
/// // store 0 in the cell — trtrm will not read it).
/// let mut l_compressed = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// l_compressed.set(1, 0, Fp::<7>::new(4));
/// // U = [[2, 3], [0, 5]].
/// let mut u = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// u.set(0, 0, Fp::<7>::new(2));
/// u.set(0, 1, Fp::<7>::new(3));
/// u.set(1, 1, Fp::<7>::new(5));
///
/// // Compute L · U the slow way for the cross-check.
/// let l_dense = {
///     let mut m = FieldMatrix::<Fp<7>>::identity(2);
///     m.set(1, 0, Fp::<7>::new(4));
///     m
/// };
/// let expected = gemm(&l_dense, &u);
///
/// trtrm(l_compressed.submat_mut(.., ..), u.submat(.., ..));
/// assert_eq!(l_compressed, expected);
/// ```
pub fn trtrm<F: FiniteField>(l: MatViewMut<'_, F>, u: MatView<'_, F>) {
    assert_eq!(
        l.rows(),
        l.cols(),
        "trtrm: L must be square ({}×{})",
        l.rows(),
        l.cols()
    );
    assert_eq!(
        u.rows(),
        u.cols(),
        "trtrm: U must be square ({}×{})",
        u.rows(),
        u.cols()
    );
    assert_eq!(
        l.rows(),
        u.rows(),
        "trtrm: L and U must have equal size ({} vs {})",
        l.rows(),
        u.rows()
    );
    trtrm_inner(l, u);
}

// ─── Internal helpers (no public surface) ───────────────────────────────────

/// Fused in-place `dst -= a · b` on a mutable view, **without** allocating
/// the intermediate `a · b` matrix. Used by the trsm/trmm recursive path
/// to fold the off-diagonal contribution into the working view directly.
fn submul_into_view<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    dst: &mut MatViewMut<'_, F>,
) {
    let (m, k) = a.shape();
    let (kb, n) = b.shape();
    debug_assert_eq!(k, kb, "submul_into_view: inner-dim mismatch");
    debug_assert_eq!(
        (m, n),
        (dst.rows(), dst.cols()),
        "submul_into_view: dst shape mismatch",
    );
    if m == 0 || n == 0 || k == 0 {
        return;
    }
    // Per-cell accumulation: reads dst[i,j], subtracts ∑_t a[i,t]·b[t,j],
    // writes back. We could use a delayed-reduction `Wide` accumulator
    // here, but that ties this routine to F::Wide; deferring to the
    // straightforward field-level accumulation keeps the code generic and
    // mirrors the trtri path. The hot path is still the `gemm` calls in
    // trtri/trtrm.
    for i in 0..m {
        for j in 0..n {
            let mut acc = dst.get(i, j);
            for t in 0..k {
                acc += -(a.get(i, t) * b.get(t, j));
            }
            dst.set(i, j, acc);
        }
    }
}

/// Fused in-place `dst += a · b` on a mutable view. Mirror of
/// [`submul_into_view`] used by the trmm path.
fn addmul_into_view<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    dst: &mut MatViewMut<'_, F>,
) {
    let (m, k) = a.shape();
    let (kb, n) = b.shape();
    debug_assert_eq!(k, kb, "addmul_into_view: inner-dim mismatch");
    debug_assert_eq!(
        (m, n),
        (dst.rows(), dst.cols()),
        "addmul_into_view: dst shape mismatch",
    );
    if m == 0 || n == 0 || k == 0 {
        return;
    }
    for i in 0..m {
        for j in 0..n {
            let mut acc = dst.get(i, j);
            for t in 0..k {
                acc += a.get(i, t) * b.get(t, j);
            }
            dst.set(i, j, acc);
        }
    }
}

// ─── trsm_upper ─────────────────────────────────────────────────────────────

fn trsm_upper_inner<F: FiniteField>(a: MatView<'_, F>, mut b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        trsm_upper_base(&a, &mut b);
        return;
    }
    let h = m / 2;
    // Recurse on the lower half: A22 · X2 = B2.
    trsm_upper_inner(a.submat(h..m, h..m), b.submat_mut(h..m, ..));
    // Fold off-diagonal: B1 -= A12 · X2.
    {
        let a12 = a.submat(0..h, h..m).to_owned();
        let x2 = b.submat(h..m, ..).to_owned();
        let mut b1 = b.submat_mut(0..h, ..);
        submul_into_view(&a12, &x2, &mut b1);
    }
    // Recurse on the upper half: A11 · X1 = B1.
    trsm_upper_inner(a.submat(0..h, 0..h), b.submat_mut(0..h, ..));
}

fn trsm_upper_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    // Back-substitution from row m-1 up to row 0.
    for i in (0..m).rev() {
        let pivot = a.get(i, i);
        let inv = pivot.inv().unwrap_or_else(|| {
            panic!(
                "trsm_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            )
        });
        for j in 0..n {
            let v = b.get(i, j) * inv.clone();
            b.set(i, j, v);
        }
        for k in 0..i {
            let aki = a.get(k, i);
            for j in 0..n {
                let v = b.get(k, j) - aki.clone() * b.get(i, j);
                b.set(k, j, v);
            }
        }
    }
}

// ─── trsm_lower ─────────────────────────────────────────────────────────────

fn trsm_lower_inner<F: FiniteField>(a: MatView<'_, F>, mut b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        trsm_lower_base(&a, &mut b);
        return;
    }
    let h = m / 2;
    // Recurse on the upper half: A11 · X1 = B1.
    trsm_lower_inner(a.submat(0..h, 0..h), b.submat_mut(0..h, ..));
    // Fold off-diagonal: B2 -= A21 · X1.
    {
        let a21 = a.submat(h..m, 0..h).to_owned();
        let x1 = b.submat(0..h, ..).to_owned();
        let mut b2 = b.submat_mut(h..m, ..);
        submul_into_view(&a21, &x1, &mut b2);
    }
    // Recurse on the lower half: A22 · X2 = B2.
    trsm_lower_inner(a.submat(h..m, h..m), b.submat_mut(h..m, ..));
}

fn trsm_lower_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    // Forward substitution from row 0 down to row m-1.
    for i in 0..m {
        let pivot = a.get(i, i);
        let inv = pivot.inv().unwrap_or_else(|| {
            panic!(
                "trsm_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            )
        });
        for j in 0..n {
            let v = b.get(i, j) * inv.clone();
            b.set(i, j, v);
        }
        for k in (i + 1)..m {
            let aki = a.get(k, i);
            for j in 0..n {
                let v = b.get(k, j) - aki.clone() * b.get(i, j);
                b.set(k, j, v);
            }
        }
    }
}

// ─── trmm_upper ─────────────────────────────────────────────────────────────

fn trmm_upper_inner<F: FiniteField>(a: MatView<'_, F>, mut b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        trmm_upper_base(&a, &mut b);
        return;
    }
    let h = m / 2;
    // Recurse on the upper half: B1 ← A11 · B1.
    trmm_upper_inner(a.submat(0..h, 0..h), b.submat_mut(0..h, ..));
    // Fold off-diagonal: B1 += A12 · B2  (B2 is still the original B2,
    // because the lower half has not been recursed on yet).
    {
        let a12 = a.submat(0..h, h..m).to_owned();
        let b2 = b.submat(h..m, ..).to_owned();
        let mut b1 = b.submat_mut(0..h, ..);
        addmul_into_view(&a12, &b2, &mut b1);
    }
    // Recurse on the lower half: B2 ← A22 · B2.
    trmm_upper_inner(a.submat(h..m, h..m), b.submat_mut(h..m, ..));
}

fn trmm_upper_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 {
        return;
    }
    // For upper triangular A and B ← A·B in place: row i depends on
    // rows ≥ i, so rows must be processed top-down. For each (i, j) we
    // accumulate ∑_{k=i}^{m-1} A[i,k]·B[k,j] in a scalar, then write.
    let zero: F = a.get(0, 0).zero_like();
    for i in 0..m {
        for j in 0..n {
            let mut acc = zero.clone();
            for k in i..m {
                acc += a.get(i, k) * b.get(k, j);
            }
            b.set(i, j, acc);
        }
    }
}

// ─── trmm_lower ─────────────────────────────────────────────────────────────

fn trmm_lower_inner<F: FiniteField>(a: MatView<'_, F>, mut b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        trmm_lower_base(&a, &mut b);
        return;
    }
    let h = m / 2;
    // Recurse on the lower half FIRST: B2 ← A22 · B2.
    trmm_lower_inner(a.submat(h..m, h..m), b.submat_mut(h..m, ..));
    // Fold off-diagonal: B2 += A21 · B1 (B1 is still the original B1).
    {
        let a21 = a.submat(h..m, 0..h).to_owned();
        let b1 = b.submat(0..h, ..).to_owned();
        let mut b2 = b.submat_mut(h..m, ..);
        addmul_into_view(&a21, &b1, &mut b2);
    }
    // Recurse on the upper half LAST: B1 ← A11 · B1.
    trmm_lower_inner(a.submat(0..h, 0..h), b.submat_mut(0..h, ..));
}

fn trmm_lower_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 {
        return;
    }
    // Lower triangular A: row i depends on rows ≤ i, so process rows
    // bottom-up to keep in-place semantics correct.
    let zero: F = a.get(0, 0).zero_like();
    for i in (0..m).rev() {
        for j in 0..n {
            let mut acc = zero.clone();
            for k in 0..=i {
                acc += a.get(i, k) * b.get(k, j);
            }
            b.set(i, j, acc);
        }
    }
}

// ─── trtri_upper ────────────────────────────────────────────────────────────

fn trtri_upper_inner<F: FiniteField>(mut a: MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        trtri_upper_base(&mut a);
        return;
    }
    let h = m / 2;
    // Recursively invert the diagonal blocks A11, A22 in place.
    trtri_upper_inner(a.submat_mut(0..h, 0..h));
    trtri_upper_inner(a.submat_mut(h..m, h..m));
    // Off-diagonal: A12 ← −A11_inv · A12 · A22_inv.
    let a11_inv = a.submat(0..h, 0..h).to_owned();
    let a12_old = a.submat(0..h, h..m).to_owned();
    let a22_inv = a.submat(h..m, h..m).to_owned();
    // tmp = A11_inv · A12_old   (one gemm, one scratch matrix)
    let tmp = gemm(&a11_inv, &a12_old);
    // a12_new = tmp · A22_inv  (second gemm, second scratch matrix)
    let a12_new = gemm(&tmp, &a22_inv);
    // Negate and write back.
    let mut a12_dst = a.submat_mut(0..h, h..m);
    for r in 0..h {
        for c in 0..(m - h) {
            a12_dst.set(r, c, -a12_new.get(r, c));
        }
    }
}

fn trtri_upper_base<F: FiniteField>(a: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    // Build A⁻¹ column by column. For an upper triangular A:
    //   A⁻¹[i, i] = 1 / A[i, i]
    //   A⁻¹[i, j] = − (1 / A[i, i]) · ∑_{k=i+1}^{j} A[i, k] · A⁻¹[k, j]   (i < j)
    // We compute the inverse from the bottom-right corner up so each row's
    // dependencies are already in place.
    //
    // We materialise A⁻¹ in a separate owned matrix to avoid destroying
    // entries of A that later iterations still need. The final write-back
    // is a single pass over the upper-triangular cells of `a`.
    let snapshot = a.to_owned();
    let n_cells = m * m;
    let zero: F = snapshot.get(0, 0).zero_like();
    let mut inv = FieldMatrix::<F>::new(m, m, zero.clone());
    debug_assert_eq!(inv.shape(), (m, m));
    debug_assert_eq!(n_cells, m * m);

    // Validate diagonals up front for a clear panic message.
    for i in 0..m {
        if snapshot.get(i, i).is_zero() {
            panic!(
                "trtri_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            );
        }
    }
    // Compute inverses from the last column back to the first.
    for j in (0..m).rev() {
        // Diagonal cell: 1 / A[j, j].
        let pivot_inv = snapshot.get(j, j).inv().unwrap_or_else(|| {
            panic!(
                "trtri_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                j, j
            )
        });
        inv.set(j, j, pivot_inv.clone());
        // Above-diagonal cells: rows i = j-1, j-2, …, 0.
        for i in (0..j).rev() {
            let mut acc = zero.clone();
            for k in (i + 1)..=j {
                acc += snapshot.get(i, k) * inv.get(k, j);
            }
            let aii_inv = snapshot.get(i, i).inv().unwrap_or_else(|| {
                panic!(
                    "trtri_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                    i, i
                )
            });
            inv.set(i, j, -(aii_inv * acc));
        }
    }
    // Write the upper triangle of `inv` back into `a`.
    for r in 0..m {
        for c in r..m {
            a.set(r, c, inv.get(r, c));
        }
    }
}

// ─── trtri_lower ────────────────────────────────────────────────────────────

fn trtri_lower_inner<F: FiniteField>(mut a: MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        trtri_lower_base(&mut a);
        return;
    }
    let h = m / 2;
    // Recursively invert the diagonal blocks A11, A22 in place.
    trtri_lower_inner(a.submat_mut(0..h, 0..h));
    trtri_lower_inner(a.submat_mut(h..m, h..m));
    // Off-diagonal: A21 ← −A22_inv · A21 · A11_inv.
    let a11_inv = a.submat(0..h, 0..h).to_owned();
    let a21_old = a.submat(h..m, 0..h).to_owned();
    let a22_inv = a.submat(h..m, h..m).to_owned();
    let tmp = gemm(&a22_inv, &a21_old);
    let a21_new = gemm(&tmp, &a11_inv);
    let mut a21_dst = a.submat_mut(h..m, 0..h);
    for r in 0..(m - h) {
        for c in 0..h {
            a21_dst.set(r, c, -a21_new.get(r, c));
        }
    }
}

fn trtri_lower_base<F: FiniteField>(a: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    let snapshot = a.to_owned();
    let zero: F = snapshot.get(0, 0).zero_like();
    let mut inv = FieldMatrix::<F>::new(m, m, zero.clone());
    for i in 0..m {
        if snapshot.get(i, i).is_zero() {
            panic!(
                "trtri_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            );
        }
    }
    // Lower triangular inverse: build from the top-left corner down.
    //   A⁻¹[i, i] = 1 / A[i, i]
    //   A⁻¹[i, j] = − (1 / A[i, i]) · ∑_{k=j}^{i-1} A[i, k] · A⁻¹[k, j]   (i > j)
    for j in 0..m {
        let pivot_inv = snapshot.get(j, j).inv().unwrap_or_else(|| {
            panic!(
                "trtri_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                j, j
            )
        });
        inv.set(j, j, pivot_inv);
        for i in (j + 1)..m {
            let mut acc = zero.clone();
            for k in j..i {
                acc += snapshot.get(i, k) * inv.get(k, j);
            }
            let aii_inv = snapshot.get(i, i).inv().unwrap_or_else(|| {
                panic!(
                    "trtri_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                    i, i
                )
            });
            inv.set(i, j, -(aii_inv * acc));
        }
    }
    // Write the lower triangle back.
    for r in 0..m {
        for c in 0..=r {
            a.set(r, c, inv.get(r, c));
        }
    }
}

// ─── trtrm ──────────────────────────────────────────────────────────────────

/// In-place product `L ← L · U`, where `L` is unit lower-triangular (the
/// diagonal is implicit `1`) and `U` is upper-triangular.
///
/// Block split per Dumas–Pernet §2.1 algorithm 2.4 with
///
/// ```text
///     L = [[L11,   0],   U = [[U11, U12],
///          [L21, L22]]        [  0, U22]]
/// ```
///
/// (`L11`, `L22` are themselves unit lower-triangular). The output blocks
/// are
///
/// ```text
///     M11 = L11 · U11
///     M12 = L11 · U12
///     M21 = L21 · U11 + L22 · U21    // U21 ≡ 0 here, so this is L21 · U11
///     M22 = L21 · U12 + L22 · U22
/// ```
///
/// Since `U21 ≡ 0`, the recursion uses two `trtrm` calls plus three
/// off-diagonal `gemm`/`trmm`/`trsm` operations:
///
/// ```text
///     M22 ← L21 · U12 + trtrm(L22, U22)        // step 1: build M22
///     trtrm(L11, U11)         in place           // step 2: M11 lives in L11
///     trmm_upper(U11ᵀ from old L21 · old U11)    // … below
/// ```
///
/// To keep the implementation simple and robust, we evaluate the product
/// via gemm at the recursion boundary (rather than recursing on
/// trtrm/trmm) — the recursion divides the working set in half but still
/// terminates at the base-case schoolbook loop below the threshold.
/// Bit-exactness vs the dense `gemm`-of-expanded-L expansion is
/// asserted by the proptests in this module.
fn trtrm_inner<F: FiniteField>(mut l: MatViewMut<'_, F>, u: MatView<'_, F>) {
    let m = l.rows();
    if m == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        trtrm_base(&mut l, &u);
        return;
    }
    let h = m / 2;

    // Snapshot the four blocks we will need from their original (pre-write)
    // values. With unit lower-triangular L and upper-triangular U, the
    // four output cells are
    //   M11 = L11 · U11
    //   M12 = L11 · U12
    //   M21 = L21 · U11
    //   M22 = L21 · U12 + L22 · U22
    // The schedule below writes them in the order M21 → M22 → M12 → M11
    // so each step reads only un-overwritten cells of L.
    //
    // Notation: `l21` is the strictly-lower-triangular off-diagonal block
    // L[h..m, 0..h] (untouched by the recursion); `u11`, `u12`, `u22` are
    // the corresponding blocks of U.
    let l21 = l.submat(h..m, 0..h).to_owned();
    let u11 = u.submat(0..h, 0..h).to_owned();
    let u12 = u.submat(0..h, h..m).to_owned();

    // Step 1 — M22 = L21 · U12 + L22 · U22.
    //   1a. Recurse into the lower-right block: L22 ← L22 · U22.
    {
        let l22_view = l.submat_mut(h..m, h..m);
        let u22_view = u.submat(h..m, h..m);
        trtrm_inner(l22_view, u22_view);
    }
    //   1b. Add L21 · U12 into the lower-right block.
    {
        let mut m22 = l.submat_mut(h..m, h..m);
        addmul_into_view(&l21, &u12, &mut m22);
    }

    // Step 2 — M21 = L21 · U11. We can use trmm_upper here because L21
    // is a generic h × (m-h) block — but trmm requires a square
    // triangular A as the left factor. Instead use trmm_upper with U11ᵀ:
    // (U11 · X)ᵀ = Xᵀ · U11ᵀ. Easier: compute L21 · U11 via gemm directly
    // and write into the lower-left block.
    {
        let prod = gemm(&l21, &u11);
        let mut m21 = l.submat_mut(h..m, 0..h);
        for r in 0..(m - h) {
            for c in 0..h {
                m21.set(r, c, prod.get(r, c));
            }
        }
    }

    // Step 3 — M12 = L11 · U12. L11 is unit lower-triangular and is still
    // in its original form (we have not touched L11 yet). Use
    // trmm_lower on a copy of U12 to compute L11_dense · U12 in place.
    //
    // Build the dense L11 (with explicit unit diagonal) so trmm_lower can
    // operate on it (trmm_lower reads diagonal entries to multiply by).
    let mut u12_working = u12.clone();
    {
        let mut l11_dense = l.submat(0..h, 0..h).to_owned();
        let one: F = u11.get(0, 0).one_like();
        for d in 0..h {
            l11_dense.set(d, d, one.clone());
        }
        trmm_lower(l11_dense.submat(.., ..), u12_working.submat_mut(.., ..));
    }
    {
        let mut m12 = l.submat_mut(0..h, h..m);
        for r in 0..h {
            for c in 0..(m - h) {
                m12.set(r, c, u12_working.get(r, c));
            }
        }
    }

    // Step 4 — M11 = L11 · U11. Recurse into the upper-left block.
    {
        let l11_view = l.submat_mut(0..h, 0..h);
        let u11_view = u.submat(0..h, 0..h);
        trtrm_inner(l11_view, u11_view);
    }
}

fn trtrm_base<F: FiniteField>(l: &mut MatViewMut<'_, F>, u: &MatView<'_, F>) {
    let m = l.rows();
    if m == 0 {
        return;
    }
    // Compute (L · U)[i, j] = ∑_{k=0}^{m-1} L[i, k] · U[k, j], with the
    // convention L[k, k] = 1 and L[i, k] = 0 for k > i (strictly upper
    // cells of L are not read), and U[k, j] = 0 for k > j (strictly lower
    // cells of U are not read).
    //
    // To preserve in-place semantics while overwriting L, snapshot the
    // strictly lower triangle of L first.
    let snap = l.to_owned();
    let one: F = u.get(0, 0).one_like();
    let zero: F = u.get(0, 0).zero_like();
    for i in 0..m {
        for j in 0..m {
            // Sum k from 0 to min(i, j), since U[k, j] = 0 for k > j.
            let kmax = i.min(j);
            let mut acc = zero.clone();
            for k in 0..=kmax {
                let l_ik = if i == k { one.clone() } else { snap.get(i, k) };
                acc += l_ik * u.get(k, j);
            }
            l.set(i, j, acc);
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::gemm;
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use proptest::prelude::*;
    use rand::{Rng, SeedableRng};

    // Test config: GF(2^8) with AES irreducible via `Gf2mWide<1>`.
    struct TriGf2m8Cfg;
    impl Gf2mWideConfig<1> for TriGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "TriGf2m8Cfg";
    }
    type TriGf2m8 = Gf2mWide<1, TriGf2m8Cfg>;

    const MERSENNE_31: u64 = 2_147_483_647;

    // ─── Random matrix builders ──────────────────────────────────────────

    fn random_fp<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
            }
        }
        m
    }

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<TriGf2m8> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<TriGf2m8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, TriGf2m8::new([rng.gen::<u64>() & 0xFF]));
            }
        }
        m
    }

    /// Returns an upper-triangular `m × m` matrix with non-zero diagonal.
    /// Cells strictly below the diagonal are zero.
    fn random_upper_fp<const P: u64>(m: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut mat = random_fp::<P>(m, m, seed);
        for r in 0..m {
            for c in 0..r {
                mat.set(r, c, Fp::<P>::new(0));
            }
            // Force a non-zero diagonal.
            if mat.get(r, r) == Fp::<P>::new(0) {
                mat.set(r, r, Fp::<P>::new(1));
            }
        }
        mat
    }

    fn random_lower_fp<const P: u64>(m: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut mat = random_fp::<P>(m, m, seed);
        for r in 0..m {
            for c in (r + 1)..m {
                mat.set(r, c, Fp::<P>::new(0));
            }
            if mat.get(r, r) == Fp::<P>::new(0) {
                mat.set(r, r, Fp::<P>::new(1));
            }
        }
        mat
    }

    fn random_upper_gf2m8(m: usize, seed: u64) -> FieldMatrix<TriGf2m8> {
        let mut mat = random_gf2m8(m, m, seed);
        for r in 0..m {
            for c in 0..r {
                mat.set(r, c, TriGf2m8::new([0]));
            }
            if mat.get(r, r) == TriGf2m8::new([0]) {
                mat.set(r, r, TriGf2m8::new([1]));
            }
        }
        mat
    }

    fn random_lower_gf2m8(m: usize, seed: u64) -> FieldMatrix<TriGf2m8> {
        let mut mat = random_gf2m8(m, m, seed);
        for r in 0..m {
            for c in (r + 1)..m {
                mat.set(r, c, TriGf2m8::new([0]));
            }
            if mat.get(r, r) == TriGf2m8::new([0]) {
                mat.set(r, r, TriGf2m8::new([1]));
            }
        }
        mat
    }

    // ─── Threshold sanity ────────────────────────────────────────────────

    #[test]
    fn test_tri_threshold_default() {
        assert_eq!(<Fp<7> as FiniteField>::TRI_BASE_THRESHOLD, 32);
        assert_eq!(<Fp<MERSENNE_31> as FiniteField>::TRI_BASE_THRESHOLD, 32);
        assert_eq!(<TriGf2m8 as FiniteField>::TRI_BASE_THRESHOLD, 32);
    }

    // ─── trsm_upper / trsm_lower correctness ─────────────────────────────

    fn check_trsm_upper_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_upper_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(1));
        let mut x = b.clone();
        trsm_upper(a.submat(.., ..), x.submat_mut(.., ..));
        // Verify: A · X == B.
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_upper round-trip m={} n={}", m, n);
    }

    fn check_trsm_lower_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_lower_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(2));
        let mut x = b.clone();
        trsm_lower(a.submat(.., ..), x.submat_mut(.., ..));
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_lower round-trip m={} n={}", m, n);
    }

    fn check_trsm_upper_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_upper_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(3));
        let mut x = b.clone();
        trsm_upper(a.submat(.., ..), x.submat_mut(.., ..));
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_upper round-trip gf2m m={} n={}", m, n);
    }

    fn check_trsm_lower_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_lower_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(4));
        let mut x = b.clone();
        trsm_lower(a.submat(.., ..), x.submat_mut(.., ..));
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_lower round-trip gf2m m={} n={}", m, n);
    }

    #[test]
    fn test_trsm_upper_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trsm_upper_fp::<7>(m, n, 0x1000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trsm_lower_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trsm_lower_fp::<7>(m, n, 0x2000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trsm_upper_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trsm_upper_fp::<MERSENNE_31>(m, 8, 0x3000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_lower_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trsm_lower_fp::<MERSENNE_31>(m, 8, 0x4000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_upper_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trsm_upper_gf2m(m, 4, 0x5000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_lower_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trsm_lower_gf2m(m, 4, 0x6000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_empty() {
        // n = 0 in both A and B: no-op, no panic.
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let mut b = FieldMatrix::<Fp<7>>::zeros(0, 5);
        trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
        trsm_lower(a.submat(.., ..), b.submat_mut(.., ..));
        // n=0 columns in B (m > 0): also a no-op.
        let a2 = random_upper_fp::<7>(4, 0x77);
        let mut b2 = FieldMatrix::<Fp<7>>::zeros(4, 0);
        trsm_upper(a2.submat(.., ..), b2.submat_mut(.., ..));
    }

    #[test]
    fn test_trsm_n_one() {
        // m = 1, n = 3: trivial division.
        let mut a = FieldMatrix::<Fp<7>>::zeros(1, 1);
        a.set(0, 0, Fp::<7>::new(3));
        let mut b = FieldMatrix::<Fp<7>>::zeros(1, 3);
        b.set(0, 0, Fp::<7>::new(6));
        b.set(0, 1, Fp::<7>::new(3));
        b.set(0, 2, Fp::<7>::new(1));
        trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
        // 6/3=2, 3/3=1, 1/3 = 1·3⁻¹ = 5 mod 7 (since 3·5=15≡1).
        assert_eq!(b.get(0, 0), Fp::<7>::new(2));
        assert_eq!(b.get(0, 1), Fp::<7>::new(1));
        assert_eq!(b.get(0, 2), Fp::<7>::new(5));
    }

    #[test]
    #[should_panic(expected = "trsm_upper: zero pivot")]
    fn test_trsm_upper_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        a.set(0, 0, Fp::<7>::new(1));
        a.set(0, 1, Fp::<7>::new(2));
        // a.set(1, 1, 0) — singular.
        let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
        b.set(0, 0, Fp::<7>::new(1));
        b.set(1, 0, Fp::<7>::new(2));
        trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
    }

    #[test]
    #[should_panic(expected = "trsm_lower: zero pivot")]
    fn test_trsm_lower_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        // a.set(0, 0, 0) — singular.
        a.set(1, 0, Fp::<7>::new(2));
        a.set(1, 1, Fp::<7>::new(3));
        let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
        b.set(0, 0, Fp::<7>::new(1));
        b.set(1, 0, Fp::<7>::new(2));
        trsm_lower(a.submat(.., ..), b.submat_mut(.., ..));
    }

    // ─── trmm_upper / trmm_lower correctness ─────────────────────────────

    fn check_trmm_upper_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_upper_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(11));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_upper(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_upper m={} n={}", m, n);
    }

    fn check_trmm_lower_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_lower_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(13));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_lower(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_lower m={} n={}", m, n);
    }

    fn check_trmm_upper_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_upper_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(15));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_upper(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_upper gf2m m={} n={}", m, n);
    }

    fn check_trmm_lower_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_lower_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(17));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_lower(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_lower gf2m m={} n={}", m, n);
    }

    #[test]
    fn test_trmm_upper_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trmm_upper_fp::<7>(m, n, 0x7000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trmm_lower_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trmm_lower_fp::<7>(m, n, 0x8000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trmm_upper_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trmm_upper_fp::<MERSENNE_31>(m, 4, 0x9000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_lower_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trmm_lower_fp::<MERSENNE_31>(m, 4, 0xA000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_upper_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trmm_upper_gf2m(m, 4, 0xB000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_lower_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trmm_lower_gf2m(m, 4, 0xC000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_empty() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let mut b = FieldMatrix::<Fp<7>>::zeros(0, 5);
        trmm_upper(a.submat(.., ..), b.submat_mut(.., ..));
        trmm_lower(a.submat(.., ..), b.submat_mut(.., ..));
    }

    // ─── trtri_upper / trtri_lower correctness ───────────────────────────

    fn check_trtri_upper_fp<const P: u64>(m: usize, seed: u64) {
        let a = random_upper_fp::<P>(m, seed);
        let mut a_inv = a.clone();
        trtri_upper(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        let id = FieldMatrix::<Fp<P>>::identity(m);
        assert_eq!(prod, id, "trtri_upper A·A⁻¹ = I, m={}", m);
    }

    fn check_trtri_lower_fp<const P: u64>(m: usize, seed: u64) {
        let a = random_lower_fp::<P>(m, seed);
        let mut a_inv = a.clone();
        trtri_lower(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        let id = FieldMatrix::<Fp<P>>::identity(m);
        assert_eq!(prod, id, "trtri_lower A·A⁻¹ = I, m={}", m);
    }

    fn check_trtri_upper_gf2m(m: usize, seed: u64) {
        let a = random_upper_gf2m8(m, seed);
        let mut a_inv = a.clone();
        trtri_upper(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        // Identity over Gf2m8 — element 1 == TriGf2m8::new([1]).
        let mut id = FieldMatrix::<TriGf2m8>::zeros(m, m);
        for i in 0..m {
            id.set(i, i, TriGf2m8::new([1]));
        }
        assert_eq!(prod, id, "trtri_upper A·A⁻¹ = I gf2m, m={}", m);
    }

    fn check_trtri_lower_gf2m(m: usize, seed: u64) {
        let a = random_lower_gf2m8(m, seed);
        let mut a_inv = a.clone();
        trtri_lower(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        let mut id = FieldMatrix::<TriGf2m8>::zeros(m, m);
        for i in 0..m {
            id.set(i, i, TriGf2m8::new([1]));
        }
        assert_eq!(prod, id, "trtri_lower A·A⁻¹ = I gf2m, m={}", m);
    }

    #[test]
    fn test_trtri_upper_small_fp7() {
        for &m in &[1usize, 2, 3, 5, 7, 16, 32, 33, 64] {
            check_trtri_upper_fp::<7>(m, 0xD000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_lower_small_fp7() {
        for &m in &[1usize, 2, 3, 5, 7, 16, 32, 33, 64] {
            check_trtri_lower_fp::<7>(m, 0xE000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_upper_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trtri_upper_fp::<MERSENNE_31>(m, 0xF000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_lower_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trtri_lower_fp::<MERSENNE_31>(m, 0x10000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_upper_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trtri_upper_gf2m(m, 0x11000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_lower_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trtri_lower_gf2m(m, 0x12000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_empty() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        trtri_upper(a.submat_mut(.., ..));
        trtri_lower(a.submat_mut(.., ..));
    }

    #[test]
    #[should_panic(expected = "trtri_upper: zero pivot")]
    fn test_trtri_upper_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        a.set(0, 0, Fp::<7>::new(1));
        a.set(0, 1, Fp::<7>::new(2));
        // a[1, 1] = 0 — singular.
        trtri_upper(a.submat_mut(.., ..));
    }

    #[test]
    #[should_panic(expected = "trtri_lower: zero pivot")]
    fn test_trtri_lower_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        // a[0, 0] = 0 — singular.
        a.set(1, 0, Fp::<7>::new(2));
        a.set(1, 1, Fp::<7>::new(3));
        trtri_lower(a.submat_mut(.., ..));
    }

    // ─── trtrm correctness ───────────────────────────────────────────────

    fn check_trtrm_fp<const P: u64>(m: usize, seed: u64) {
        // Build L (unit lower-triangular) and U (upper-triangular).
        let mut l = random_fp::<P>(m, m, seed);
        for r in 0..m {
            for c in r..m {
                l.set(r, c, Fp::<P>::new(0)); // zero out diagonal+upper for storage
            }
        }
        let mut u = random_fp::<P>(m, m, seed.wrapping_add(7));
        for r in 0..m {
            for c in 0..r {
                u.set(r, c, Fp::<P>::new(0));
            }
        }
        // Dense L: insert implicit unit diagonal.
        let l_dense = {
            let mut tmp = l.clone();
            for d in 0..m {
                tmp.set(d, d, Fp::<P>::new(1));
            }
            tmp
        };
        let expected = gemm(&l_dense, &u);
        let mut got = l.clone();
        trtrm(got.submat_mut(.., ..), u.submat(.., ..));
        assert_eq!(got, expected, "trtrm m={}", m);
    }

    fn check_trtrm_gf2m(m: usize, seed: u64) {
        let mut l = random_gf2m8(m, m, seed);
        for r in 0..m {
            for c in r..m {
                l.set(r, c, TriGf2m8::new([0]));
            }
        }
        let mut u = random_gf2m8(m, m, seed.wrapping_add(7));
        for r in 0..m {
            for c in 0..r {
                u.set(r, c, TriGf2m8::new([0]));
            }
        }
        let l_dense = {
            let mut tmp = l.clone();
            for d in 0..m {
                tmp.set(d, d, TriGf2m8::new([1]));
            }
            tmp
        };
        let expected = gemm(&l_dense, &u);
        let mut got = l.clone();
        trtrm(got.submat_mut(.., ..), u.submat(.., ..));
        assert_eq!(got, expected, "trtrm gf2m m={}", m);
    }

    #[test]
    fn test_trtrm_small_fp7() {
        for &m in &[1usize, 2, 3, 5, 7, 16, 32, 33, 64] {
            check_trtrm_fp::<7>(m, 0x13000 + m as u64);
        }
    }

    #[test]
    fn test_trtrm_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trtrm_fp::<MERSENNE_31>(m, 0x14000 + m as u64);
        }
    }

    #[test]
    fn test_trtrm_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trtrm_gf2m(m, 0x15000 + m as u64);
        }
    }

    #[test]
    fn test_trtrm_empty() {
        let mut l = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let u = FieldMatrix::<Fp<7>>::zeros(0, 0);
        trtrm(l.submat_mut(.., ..), u.submat(.., ..));
    }

    // ─── Property-based tests ────────────────────────────────────────────

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

        /// trsm round-trip on Fp<7>: A · X = B for upper-triangular A.
        #[test]
        fn prop_trsm_upper_fp7(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_upper_fp::<7>(m, seed);
            let b = random_fp::<7>(m, n, seed.wrapping_add(0x71));
            let mut x = b.clone();
            trsm_upper(a.submat(.., ..), x.submat_mut(.., ..));
            let recon = gemm(&a, &x);
            prop_assert_eq!(recon, b);
        }

        #[test]
        fn prop_trsm_lower_fp7(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_lower_fp::<7>(m, seed);
            let b = random_fp::<7>(m, n, seed.wrapping_add(0x72));
            let mut x = b.clone();
            trsm_lower(a.submat(.., ..), x.submat_mut(.., ..));
            let recon = gemm(&a, &x);
            prop_assert_eq!(recon, b);
        }

        /// trmm matches dense gemm: A · B for upper-triangular A.
        #[test]
        fn prop_trmm_upper_fp31(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_upper_fp::<MERSENNE_31>(m, seed);
            let b = random_fp::<MERSENNE_31>(m, n, seed.wrapping_add(0x73));
            let expected = gemm(&a, &b);
            let mut got = b.clone();
            trmm_upper(a.submat(.., ..), got.submat_mut(.., ..));
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_trmm_lower_fp31(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_lower_fp::<MERSENNE_31>(m, seed);
            let b = random_fp::<MERSENNE_31>(m, n, seed.wrapping_add(0x74));
            let expected = gemm(&a, &b);
            let mut got = b.clone();
            trmm_lower(a.submat(.., ..), got.submat_mut(.., ..));
            prop_assert_eq!(got, expected);
        }

        /// trtri round-trip: A · A⁻¹ = I.
        #[test]
        fn prop_trtri_upper_fp7(m in 1usize..40, seed in 0u64..256) {
            let a = random_upper_fp::<7>(m, seed);
            let mut a_inv = a.clone();
            trtri_upper(a_inv.submat_mut(.., ..));
            let prod = gemm(&a, &a_inv);
            let id = FieldMatrix::<Fp<7>>::identity(m);
            prop_assert_eq!(prod, id);
        }

        #[test]
        fn prop_trtri_lower_fp7(m in 1usize..40, seed in 0u64..256) {
            let a = random_lower_fp::<7>(m, seed);
            let mut a_inv = a.clone();
            trtri_lower(a_inv.submat_mut(.., ..));
            let prod = gemm(&a, &a_inv);
            let id = FieldMatrix::<Fp<7>>::identity(m);
            prop_assert_eq!(prod, id);
        }

        /// trtrm matches dense gemm of L (with implicit unit diag) times U.
        #[test]
        fn prop_trtrm_fp7(m in 1usize..40, seed in 0u64..256) {
            let mut l = random_fp::<7>(m, m, seed);
            for r in 0..m {
                for c in r..m {
                    l.set(r, c, Fp::<7>::new(0));
                }
            }
            let mut u = random_fp::<7>(m, m, seed.wrapping_add(0x75));
            for r in 0..m {
                for c in 0..r {
                    u.set(r, c, Fp::<7>::new(0));
                }
            }
            let l_dense = {
                let mut tmp = l.clone();
                for d in 0..m {
                    tmp.set(d, d, Fp::<7>::new(1));
                }
                tmp
            };
            let expected = gemm(&l_dense, &u);
            let mut got = l.clone();
            trtrm(got.submat_mut(.., ..), u.submat(.., ..));
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_trtrm_gf2m8(m in 1usize..30, seed in 0u64..128) {
            let mut l = random_gf2m8(m, m, seed);
            for r in 0..m {
                for c in r..m {
                    l.set(r, c, TriGf2m8::new([0]));
                }
            }
            let mut u = random_gf2m8(m, m, seed.wrapping_add(0x76));
            for r in 0..m {
                for c in 0..r {
                    u.set(r, c, TriGf2m8::new([0]));
                }
            }
            let l_dense = {
                let mut tmp = l.clone();
                for d in 0..m {
                    tmp.set(d, d, TriGf2m8::new([1]));
                }
                tmp
            };
            let expected = gemm(&l_dense, &u);
            let mut got = l.clone();
            trtrm(got.submat_mut(.., ..), u.submat(.., ..));
            prop_assert_eq!(got, expected);
        }
    }

    // ─── Spot check the helper view-to-owned/round-trip path ─────────────

    #[test]
    fn test_view_helpers_roundtrip() {
        let m = FieldMatrix::<Fp<7>>::identity(3);
        let v = m.submat(.., ..);
        let owned = v.to_owned();
        assert_eq!(owned, m);
    }

    /// Straddle the threshold: at exactly TRI_BASE_THRESHOLD we hit the
    /// base case; just above it, the recursion peels at least once.
    #[test]
    fn test_recursive_split_just_above_threshold() {
        let m = <Fp<MERSENNE_31> as FiniteField>::TRI_BASE_THRESHOLD + 1;
        check_trsm_upper_fp::<MERSENNE_31>(m, 4, 0xABCD);
        check_trsm_lower_fp::<MERSENNE_31>(m, 4, 0xABCE);
        check_trmm_upper_fp::<MERSENNE_31>(m, 4, 0xABCF);
        check_trmm_lower_fp::<MERSENNE_31>(m, 4, 0xABD0);
        check_trtri_upper_fp::<MERSENNE_31>(m, 0xABD1);
        check_trtri_lower_fp::<MERSENNE_31>(m, 0xABD2);
        check_trtrm_fp::<MERSENNE_31>(m, 0xABD3);
    }
}
