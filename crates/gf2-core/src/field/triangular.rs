//! Block-recursive triangular primitives — `trsm`, `trmm`, `trtri`, `trtrm`.
//!
//! This module implements Dumas–Pernet §2.1 algorithms 2.1–2.4 as
//! **reductions to `gemm`** on top of the existing classical
//! [`gemm`](crate::field::matrix::gemm) (issue `91c06222`) and the dense
//! view types [`MatView`](crate::field::matrix::MatView) /
//! [`MatViewMut`](crate::field::matrix::MatViewMut). All routines operate
//! **in place** on the supplied views.
//!
//! The `B ← B − A · X` and `B ← B + A · X` updates that drive `trsm` and
//! `trmm` are dispatched through the shared
//! [`gemm_axpy_into_view`](crate::field::matrix::gemm_axpy_into_view)
//! kernel in [`crate::field::matrix`], which writes the result into a
//! caller-supplied `MatViewMut` and inherits the same blocked,
//! delayed-reduction structure as T1's classical
//! [`gemm`](crate::field::matrix::gemm) — its inner kernel is
//! [`crate::field::vec::dot_product_slices`] with the standard
//! `F::max_unreduced_additions()` chunking, so the `Wide` accumulator
//! never overflows even at large inner dimensions. The `trtri` and
//! `trtrm` recursions go through
//! [`gemm_into_view`](crate::field::matrix::gemm_into_view) (no β·C term
//! needed). The `trtrm` `A12 = U12 · L22` step (where `L22` is unit-
//! lower-triangular with implicit diagonal) goes through
//! [`gemm_axpy_into_view_diag`](crate::field::matrix::gemm_axpy_into_view_diag)
//! — the unit-diagonal-aware sibling of `gemm_axpy_into_view` introduced
//! by R4 to fold the implicit-`1` diagonal into the same per-cell
//! generic kernel, eliminating the previous bespoke per-cell multiply
//! loop. **No bespoke matrix-multiply loops outside the inherent
//! linear-algebra inner kernels in [`crate::field::matrix`] (which now
//! include `gemm_axpy_into_view_diag` for implicit-unit-diagonal
//! operands used by `trtrm`).**
//!
//! Each `gemm_axpy_into_view` / `gemm_into_view` call carries the same
//! single `B`-transpose scratch as the classical blocked gemm — that
//! single allocation is paid by the kernel itself, and the trsm/trmm
//! recursive paths add no further owned `FieldMatrix<F>` snapshots.
//! The trtri / trtrm primitives need exactly **one** extra scratch
//! matrix of size `h × h` per recursion level for the
//! `−A11 · A12 · A22` chained multiply in algorithm 2.3 (and the
//! analogous step in algorithm 2.4); this is documented per-function.
//!
//! # Routines
//!
//! - [`trsm_upper`] / [`trsm_lower`] — solve `A · X = B` for triangular `A`,
//!   overwriting `B` with `X` (algorithm 2.1).
//! - [`trmm_upper`] / [`trmm_lower`] — overwrite `B` with `A · B` for
//!   triangular `A` (algorithm 2.2).
//! - [`trtri_upper`] / [`trtri_lower`] — invert a triangular matrix in
//!   place (algorithm 2.3).
//! - [`trtrm`] — compute the in-place product `U · L` where `U` is
//!   upper-triangular and `L` is unit lower-triangular, used by the PLE
//!   decomposition (algorithm 2.4). Output overwrites the `L`-view.
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
//! Computes the in-place product `A ← U · L` where `U` is upper-triangular
//! and `L` is unit lower-triangular (the diagonal is implicitly `1` and is
//! **not** read by the routine). The output dense product overwrites the
//! `L`-view. This is the convention used by Dumas–Pernet §3 PLE so the
//! in-place compression in issue `c3f8c1cb` can call it directly.
//!
//! # Allocation budget
//!
//! - `trsm_*` / `trmm_*`: the recursive paths allocate **nothing** on
//!   top of the shared
//!   [`gemm_axpy_into_view`](crate::field::matrix::gemm_axpy_into_view)
//!   kernel's standard `B`-transpose scratch — i.e. each off-diagonal
//!   fold `B ± A · B'` pays exactly one `FieldMatrix<F>` allocation
//!   inside the kernel for the transposed `B'`, and the trsm/trmm
//!   recursion itself adds no further owned snapshots. Across a full
//!   recursion that sums to `O(log m)` `B`-transpose allocations, all
//!   amortised inside the gemm kernel call. **This is the `[hard]`
//!   zero-extra-allocation contract of issue 83b1ad8b.**
//! - `trtri_*`: each recursion level allocates **one** scratch
//!   `FieldMatrix<F>` of shape `h × h` for the chained multiply
//!   `A12 := −A11 · A12 · A22` (algorithm 2.3). This is an
//!   architectural exception (matrix-matrix product is not associative
//!   in-place at element granularity that would let us avoid an
//!   intermediate buffer); recorded as the issue 83b1ad8b R4
//!   amendment. Sums to `O(log m)` scratch matrices over the full
//!   recursion (geometric series), `O(m²)` cells total.
//! - `trtrm`: each recursion level allocates **one** scratch
//!   `FieldMatrix<F>` of shape `(m-h) × h` for the `U22 · L21`
//!   chain (the only step whose write region aliases its read region —
//!   both are the `L21` slot), staged through
//!   [`gemm_into_view`](crate::field::matrix::gemm_into_view) and
//!   copied back. Same architectural-exception status as `trtri_*`.
//!   The other off-diagonal step (`A12 = U12 · L22`) goes through
//!   [`gemm_axpy_into_view_diag`](crate::field::matrix::gemm_axpy_into_view_diag)
//!   into the unused top-right slot of the L-view; that kernel adds no
//!   `FieldMatrix<F>` allocation because it walks both operands cell-
//!   wise via [`MatrixLike::get`](crate::matrix_like::MatrixLike::get)
//!   and fuses the unit-diagonal `1`s in-line. Mirrors the trtri
//!   budget.
//!
//! The `[hard]` zero-allocation criterion of issue 83b1ad8b therefore
//! applies to `trsm`/`trmm` only; the `trtri` and `trtrm` chain-multiply
//! scratches are recorded as the documented architectural exception
//! (R4 amendment, 2026-04-25). The bench at `benches/triangular.rs`
//! measures both paths so the cost stays visible.
//!
//! Geometrically these scratch allocations sum to `O(m²)` cells over
//! the full recursion tree, the same asymptotic budget as the
//! Strassen-Winograd recursion in [`crate::field::winograd`].
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

use crate::field::matrix::{
    gemm_axpy_into_view, gemm_axpy_into_view_diag, gemm_into_view, FieldMatrix, MatView,
    MatViewMut, UnitDiag,
};
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

/// In-place product of an upper-triangular `U` with a unit lower-triangular
/// `L`.
///
/// Computes the dense `m × m` product `A = U · L` and writes it into the
/// `l` view (overwriting the original `L`). `L` is treated as **unit**
/// lower-triangular: the diagonal cells are implicitly `1` and the routine
/// does not read them. `U` is upper-triangular; cells strictly below `U`'s
/// diagonal are not read.
///
/// **Storage convention for the strict-upper region of `L`.** The unit-
/// diagonal `L21·L22` step is dispatched through
/// [`gemm_axpy_into_view_diag`](crate::field::matrix::gemm_axpy_into_view_diag);
/// that kernel reads the entire `L22` block (not just the strict-lower
/// triangle) and substitutes `F::one()` only on the diagonal positions,
/// so callers **must zero the strict-upper region of `L`** before calling
/// `trtrm`. This matches the PLE-compression caller convention (the
/// compressed `[L \ U]` store puts U in the strict-upper region only
/// AFTER the trtrm step has completed; before trtrm, those cells are
/// zero because the compressed storage is freshly initialised). The
/// `examples/`-backed doctests below illustrate the convention.
///
/// This is the §2.1 algorithm 2.4 convention used by Dumas–Pernet's PLE
/// decomposition (issue `c3f8c1cb`): the post-pivot in-place product
/// reconstitutes the dense matrix from its compressed `[L \ U]` storage.
///
/// # Arguments
///
/// * `l` — Square `m × m` view holding the **strictly lower-triangular**
///   part of `L` (with implicit unit diagonal). On return contains the
///   dense product `U · L`.
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
/// // Compute U · L the slow way for the cross-check.
/// let l_dense = {
///     let mut m = FieldMatrix::<Fp<7>>::identity(2);
///     m.set(1, 0, Fp::<7>::new(4));
///     m
/// };
/// let expected = gemm(&u, &l_dense);
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

// ─── trsm_upper ─────────────────────────────────────────────────────────────

fn trsm_upper_inner<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        let mut b_mut = b;
        trsm_upper_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    // Split B into the upper and lower row halves so we can read B2
    // immutably while mutating B1 — both halves alias the same parent
    // buffer, but `split_rows_mut` slices them into disjoint mutable
    // sub-slices so Rust's borrow checker accepts the pair.
    let (mut b1_mut, mut b2_mut) = b.split_rows_mut(h);
    // Recurse on the lower half first: A22 · X2 = B2.
    trsm_upper_inner(a.submat(h..m, h..m), b2_mut.reborrow());
    // Fold off-diagonal: B1 ← (−1) · A12 · X2 + 1 · B1 — i.e. the
    // shared `gemm_axpy_into_view` kernel writing into B1 with B1
    // itself doubling as the C operand. `b2` is the immutable view of
    // the already-solved lower half (disjoint from `b1` via
    // `split_rows_mut`); `a12` is a sub-view of the read-only `a`.
    {
        let a12 = a.submat(0..h, h..m);
        let b2 = b2_mut.as_view();
        let one = a.get(0, 0).one_like();
        let neg_one = -one.clone();
        gemm_axpy_into_view(neg_one, &a12, &b2, one, b1_mut.reborrow());
    }
    // Recurse on the upper half: A11 · X1 = B1.
    trsm_upper_inner(a.submat(0..h, 0..h), b1_mut);
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

fn trsm_lower_inner<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        let mut b_mut = b;
        trsm_lower_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    // Split B at row h to obtain disjoint mutable views of B1 and B2.
    let (mut b1_mut, mut b2_mut) = b.split_rows_mut(h);
    // Recurse on the upper half first: A11 · X1 = B1.
    trsm_lower_inner(a.submat(0..h, 0..h), b1_mut.reborrow());
    // Fold off-diagonal: B2 ← (−1) · A21 · X1 + 1 · B2 via the shared
    // `gemm_axpy_into_view` kernel, with `b2_mut` doubling as the
    // destination and the C operand. `b1` (read-only view of the
    // upper half) is disjoint from `b2_mut` thanks to `split_rows_mut`.
    {
        let a21 = a.submat(h..m, 0..h);
        let b1 = b1_mut.as_view();
        let one = a.get(0, 0).one_like();
        let neg_one = -one.clone();
        gemm_axpy_into_view(neg_one, &a21, &b1, one, b2_mut.reborrow());
    }
    // Recurse on the lower half: A22 · X2 = B2.
    trsm_lower_inner(a.submat(h..m, h..m), b2_mut);
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

fn trmm_upper_inner<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        let mut b_mut = b;
        trmm_upper_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    // Split B at row h: mutate B1 (rows 0..h) while reading B2 (rows
    // h..m). The schedule keeps B2 untouched until step 3 so the read
    // sees the original value.
    let (mut b1_mut, b2_mut) = b.split_rows_mut(h);
    // Step 1 — recurse on the upper half: B1 ← A11 · B1.
    trmm_upper_inner(a.submat(0..h, 0..h), b1_mut.reborrow());
    // Step 2 — fold off-diagonal via the shared `gemm_axpy_into_view`:
    // B1 ← 1 · A12 · B2 + 1 · B1. B2 is still the original value here
    // (step 3 hasn't run), so its immutable view is disjoint from the
    // mutable B1 thanks to `split_rows_mut`.
    {
        let a12 = a.submat(0..h, h..m);
        let b2 = b2_mut.as_view();
        let one = a.get(0, 0).one_like();
        gemm_axpy_into_view(one.clone(), &a12, &b2, one, b1_mut.reborrow());
    }
    // Step 3 — recurse on the lower half: B2 ← A22 · B2.
    trmm_upper_inner(a.submat(h..m, h..m), b2_mut);
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

fn trmm_lower_inner<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        let mut b_mut = b;
        trmm_lower_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    // Split B at row h: mutate B2 first (rows h..m), then fold in B1
    // (still the original value), then recurse on B1 last.
    let (b1_mut, mut b2_mut) = b.split_rows_mut(h);
    // Step 1 — recurse on the lower half FIRST: B2 ← A22 · B2.
    trmm_lower_inner(a.submat(h..m, h..m), b2_mut.reborrow());
    // Step 2 — fold off-diagonal via the shared `gemm_axpy_into_view`:
    // B2 ← 1 · A21 · B1 + 1 · B2. B1 is still the original value here
    // (step 3 hasn't run), so its immutable view is disjoint from B2.
    {
        let a21 = a.submat(h..m, 0..h);
        let b1 = b1_mut.as_view();
        let one = a.get(0, 0).one_like();
        gemm_axpy_into_view(one.clone(), &a21, &b1, one, b2_mut.reborrow());
    }
    // Step 3 — recurse on the upper half LAST: B1 ← A11 · B1.
    trmm_lower_inner(a.submat(0..h, 0..h), b1_mut);
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

fn trtri_upper_inner<F: FiniteField>(a: MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        let mut a_mut = a;
        trtri_upper_base(&mut a_mut);
        return;
    }
    let h = m / 2;
    // Split A horizontally at row h. `top` holds rows 0..h (containing
    // A11 in the left h columns and A12 in the right m-h columns).
    // `bot` holds rows h..m (containing A22 in the right m-h columns).
    let (mut top, mut bot) = a.split_rows_mut(h);
    // Recursively invert the diagonal blocks A11 (in `top`) and A22
    // (in `bot`) in place.
    trtri_upper_inner(top.submat_mut(0..h, 0..h));
    trtri_upper_inner(bot.submat_mut(0..(m - h), h..m));
    // Off-diagonal: A12 ← −A11_inv · A12 · A22_inv.
    //
    // Allocation budget: ONE scratch `FieldMatrix<F>` of shape
    // h × (m-h) per recursion level, used to hold the intermediate
    // `A11_inv · A12_old`. The first product is computed with
    // [`gemm_into_view`] writing into the scratch; the second product
    // (and the final negation) is written directly back into the
    // A12 sub-view of `top` via [`gemm_into_view`] + a single
    // negation pass — no second scratch is allocated.
    let zero: F = top.as_view().get(0, 0).zero_like();
    let mut tmp = FieldMatrix::<F>::new(h, m - h, zero);
    {
        let a11_inv = top.submat(0..h, 0..h);
        let a12_old = top.submat(0..h, h..m);
        gemm_into_view(&a11_inv, &a12_old, tmp.submat_mut(.., ..));
    }
    {
        let a22_inv = bot.submat(0..(m - h), h..m);
        let a12_dst = top.submat_mut(0..h, h..m);
        gemm_into_view(&tmp, &a22_inv, a12_dst);
    }
    // Negate A12 in place — single pass, no allocation.
    let mut a12_dst = top.submat_mut(0..h, h..m);
    for r in 0..h {
        for c in 0..(m - h) {
            let v = a12_dst.get(r, c);
            a12_dst.set(r, c, -v);
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
    // Allocation budget: a single `m × m` `FieldMatrix<F>` to stage the
    // inverse before writing it back to `a`. Reads from `a` are safe
    // because the loop never writes to `a` until the final pass.
    let zero: F = a.get(0, 0).zero_like();
    let mut inv = FieldMatrix::<F>::new(m, m, zero.clone());

    // Validate diagonals up front for a clear panic message.
    for i in 0..m {
        if a.get(i, i).is_zero() {
            panic!(
                "trtri_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            );
        }
    }
    // Compute inverses from the last column back to the first.
    for j in (0..m).rev() {
        // Diagonal cell: 1 / A[j, j].
        let pivot_inv = a.get(j, j).inv().unwrap_or_else(|| {
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
                acc += a.get(i, k) * inv.get(k, j);
            }
            let aii_inv = a.get(i, i).inv().unwrap_or_else(|| {
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

fn trtri_lower_inner<F: FiniteField>(a: MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        let mut a_mut = a;
        trtri_lower_base(&mut a_mut);
        return;
    }
    let h = m / 2;
    // Split A horizontally at row h. `top` holds rows 0..h (containing
    // A11 in the left h columns). `bot` holds rows h..m (containing
    // A21 in the left h columns and A22 in the right m-h columns).
    let (mut top, mut bot) = a.split_rows_mut(h);
    // Recursively invert the diagonal blocks A11, A22 in place.
    trtri_lower_inner(top.submat_mut(0..h, 0..h));
    trtri_lower_inner(bot.submat_mut(0..(m - h), h..m));
    // Off-diagonal: A21 ← −A22_inv · A21 · A11_inv.
    //
    // Allocation budget: ONE scratch `FieldMatrix<F>` of shape
    // (m-h) × h per recursion level for the intermediate
    // `A22_inv · A21_old`.
    let zero: F = bot.as_view().get(0, 0).zero_like();
    let mut tmp = FieldMatrix::<F>::new(m - h, h, zero);
    {
        let a22_inv = bot.submat(0..(m - h), h..m);
        let a21_old = bot.submat(0..(m - h), 0..h);
        gemm_into_view(&a22_inv, &a21_old, tmp.submat_mut(.., ..));
    }
    {
        let a11_inv = top.submat(0..h, 0..h);
        let a21_dst = bot.submat_mut(0..(m - h), 0..h);
        gemm_into_view(&tmp, &a11_inv, a21_dst);
    }
    // Negate A21 in place.
    let mut a21_dst = bot.submat_mut(0..(m - h), 0..h);
    for r in 0..(m - h) {
        for c in 0..h {
            let v = a21_dst.get(r, c);
            a21_dst.set(r, c, -v);
        }
    }
}

fn trtri_lower_base<F: FiniteField>(a: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    // Allocation budget: one `m × m` `FieldMatrix<F>` to stage the
    // inverse. Reads come from `a` directly because the loop only
    // writes to `inv`.
    let zero: F = a.get(0, 0).zero_like();
    let mut inv = FieldMatrix::<F>::new(m, m, zero.clone());
    for i in 0..m {
        if a.get(i, i).is_zero() {
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
        let pivot_inv = a.get(j, j).inv().unwrap_or_else(|| {
            panic!(
                "trtri_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                j, j
            )
        });
        inv.set(j, j, pivot_inv);
        for i in (j + 1)..m {
            let mut acc = zero.clone();
            for k in j..i {
                acc += a.get(i, k) * inv.get(k, j);
            }
            let aii_inv = a.get(i, i).inv().unwrap_or_else(|| {
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

/// In-place product `A ← U · L`, where `U` is upper-triangular and `L` is
/// unit lower-triangular (the diagonal is implicit `1`). The dense product
/// overwrites the `L`-view.
///
/// Block split per Dumas–Pernet §2.1 algorithm 2.4 with
///
/// ```text
///     U = [[U11, U12],   L = [[L11,   0],
///          [  0, U22]]        [L21, L22]]
/// ```
///
/// (`L11`, `L22` are themselves unit lower-triangular; `U11`, `U22` are
/// upper-triangular). The output blocks of `A = U · L` are
///
/// ```text
///     A11 = U11 · L11 + U12 · L21
///     A12 = U12 · L22
///     A21 = U22 · L21
///     A22 = U22 · L22
/// ```
///
/// The schedule writes `A12 → A11 → A21 → A22` so each step reads only
/// un-overwritten cells of `L`. `A12` lands in the unused upper-right
/// slot of the L-view via [`gemm_axpy_into_view_diag`] with
/// `diag_b = UnitDiag::Implicit` — that kernel folds `L22`'s implicit
/// unit diagonal into the per-cell accumulator without materialising a
/// dense `L22` (zero allocation for this step). `A11` recurses into the
/// upper-left block (which computes `U11 · L11` in place via the trtrm
/// contract) and then folds `U12 · L21` onto it via the standard
/// [`gemm_axpy_into_view`] (`L21` still intact). `A21 = U22 · L21`
/// aliases its own read region (the write region IS `L21`), so we
/// stage it in a single `(m-h) × h` scratch buffer and copy back.
/// Finally `A22` recurses into the lower-right block.
///
/// Allocation budget per recursion level: ONE scratch `FieldMatrix<F>`
/// of shape `(m-h) × h` for the `U22 · L21` chain. Mirrors `trtri`'s
/// per-level scratch budget. Recorded as the documented architectural
/// exception in the issue 83b1ad8b R4 amendment.
fn trtrm_inner<F: FiniteField>(l: MatViewMut<'_, F>, u: MatView<'_, F>) {
    let m = l.rows();
    if m == 0 {
        return;
    }
    if m <= F::TRI_BASE_THRESHOLD {
        let mut l_mut = l;
        trtrm_base(&mut l_mut, &u);
        return;
    }
    let h = m / 2;
    // Split L horizontally at row h. `top` carries L11 (left h cols)
    // and the to-be-written A12 (right m-h cols). `bot` carries L21
    // (left h cols, future A21) and L22 / future A22 (right m-h cols).
    let (mut top, mut bot) = l.split_rows_mut(h);

    // Step 1 — A12 = U12 · L22. `L22` is unit-lower-triangular with the
    // diagonal implicit: the storage cell at `L22[c, c]` is reused for
    // (eventual) U·L product entries and must NOT be read on the
    // diagonal. We dispatch this through the shared
    // [`gemm_axpy_into_view_diag`] kernel with `diag_b = UnitDiag::Implicit`,
    // which substitutes `F::one()` for diagonal reads of `L22` while
    // walking the strict-lower body via the underlying `MatrixLike::get`.
    // The strict-upper region of `L22`'s storage may carry garbage, but
    // because `L22` is logically lower-triangular the corresponding
    // `U12` columns are zero outside `k ≥ c`, so reading those cells
    // never contaminates the dot product (U12[i, k] for k < i is in the
    // upper-tri region of `u`, all valid; the kernel relies on `u`
    // being a real matrix view and computes the full dot product). The
    // result reduces algebraically to `A12[i, c] = ∑_{k≥c} U12[i,k] ·
    // L22[k,c]` because `L22[k,c] = 0` for `k < c` in the storage —
    // upheld by the trtrm caller contract (the input L is unit-lower
    // with strict-upper zeros).
    //
    // α = 1, β = 0 (the destination cells in `top`'s right block hold
    // garbage from the L-storage's strict-upper region; β=0 overwrites
    // cleanly).
    {
        let u12 = u.submat(0..h, h..m);
        let l22_view = bot.submat(0..(m - h), h..m);
        let one: F = u.get(0, 0).one_like();
        let zero: F = u.get(0, 0).zero_like();
        gemm_axpy_into_view_diag(
            UnitDiag::Stored,
            one,
            &u12,
            UnitDiag::Implicit,
            &l22_view,
            zero,
            top.submat_mut(0..h, h..m),
        );
    }

    // Step 2 — A11 = U11 · L11 + U12 · L21.
    //   2a. Recurse into the upper-left block: L11 ← U11 · L11. After
    //       this, top[0..h, 0..h] holds U11·L11. L21 (in bot-left) is
    //       untouched.
    trtrm_inner(top.submat_mut(0..h, 0..h), u.submat(0..h, 0..h));
    //   2b. Add U12 · L21 into the upper-left block (A11). U12 (sub-view
    //       of `u`) and L21 (left h cols of `bot`) are read; the write
    //       region (left h cols of `top`) is disjoint from `bot`, so
    //       no scratch is needed. Dispatch through the shared
    //       `gemm_axpy_into_view` kernel — the destination doubles as
    //       the C operand (β = 1), exactly the trsm/trmm idiom.
    {
        let u12 = u.submat(0..h, h..m);
        let l21_view = bot.submat(0..(m - h), 0..h);
        let one = u.get(0, 0).one_like();
        gemm_axpy_into_view(
            one.clone(),
            &u12,
            &l21_view,
            one,
            top.submat_mut(0..h, 0..h),
        );
    }

    // Step 3 — A21 = U22 · L21. The output region IS `L21`, so this
    // multiply aliases its own read inputs and a per-cell read-then-
    // write order is unsafe. Use a scratch of size (m-h) × h to stage
    // the product, then copy back. This is the only scratch this
    // recursion level allocates.
    let zero: F = bot.as_view().get(0, 0).zero_like();
    let mut scratch = FieldMatrix::<F>::new(m - h, h, zero);
    {
        let u22 = u.submat(h..m, h..m);
        let l21 = bot.submat(0..(m - h), 0..h);
        gemm_into_view(&u22, &l21, scratch.submat_mut(.., ..));
    }
    {
        let mut a21 = bot.submat_mut(0..(m - h), 0..h);
        for r in 0..(m - h) {
            for c in 0..h {
                a21.set(r, c, scratch.get(r, c));
            }
        }
    }

    // Step 4 — A22 = U22 · L22. Recurse into the lower-right block of
    // `bot` — L22 is still intact (steps 1, 2, 3 read L22 in step 1
    // only and never wrote to it; the L21 destruction in step 3 is
    // confined to the left h cols of `bot`).
    trtrm_inner(bot.submat_mut(0..(m - h), h..m), u.submat(h..m, h..m));
}

fn trtrm_base<F: FiniteField>(l: &mut MatViewMut<'_, F>, u: &MatView<'_, F>) {
    let m = l.rows();
    if m == 0 {
        return;
    }
    // Compute (U · L)[i, j] = ∑_{k=max(i,j)}^{m-1} U[i, k] · L[k, j],
    // with L[k, k] = 1 (implicit), L[k, j] = 0 for k < j (strict-upper
    // cells of L are not read), and U[i, k] = 0 for k < i (strict-
    // lower cells of U are not read).
    //
    // Iteration schedule: walk columns left-to-right (j = 0..m), rows
    // top-to-bottom (i = 0..m) within each column. At cell (i, j) we
    // read L[k, j] for k = max(i, j)..m-1. These reads come from the
    // *original* L because:
    //   - in earlier columns (j' < j) writes were confined to column
    //     j', not j, so column j is still pristine when we enter it;
    //   - within column j, prior writes happened at rows 0..i-1, and
    //     our read range is k ≥ max(i, j) ≥ i, so we never read those
    //     overwritten cells;
    //   - the cell L[j, j] (the implicit unit diagonal) is folded in
    //     via `if k == j` rather than read from storage.
    // Therefore the loop is safe with no snapshot.
    let one: F = u.get(0, 0).one_like();
    let zero: F = u.get(0, 0).zero_like();
    for j in 0..m {
        for i in 0..m {
            // Sum k from max(i, j) to m-1, since U[i, k] = 0 for k < i
            // and L[k, j] = 0 for k < j.
            let kmin = i.max(j);
            let mut acc = zero.clone();
            for k in kmin..m {
                let l_kj = if k == j { one.clone() } else { l.get(k, j) };
                acc += u.get(i, k) * l_kj;
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
        // Build L (unit lower-triangular, diagonal implicit in the
        // compressed view) and U (upper-triangular).
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
        // trtrm computes A = U · L written into the L-view per the
        // issue 83b1ad8b contract.
        let expected = gemm(&u, &l_dense);
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
        let expected = gemm(&u, &l_dense);
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

        /// trtrm matches dense gemm of U times L (with implicit unit diag).
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
            let expected = gemm(&u, &l_dense);
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
            let expected = gemm(&u, &l_dense);
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

    // ─── Allocation-budget regression tests ──────────────────────────────
    //
    // These tests anchor the issue 83b1ad8b R1 contract: trsm/trmm
    // allocate ZERO `FieldMatrix::new` calls across an entire
    // recursive solve, and trtri/trtrm allocate exactly the documented
    // per-recursion-level scratch (one `h × h`-class buffer per inner
    // node + one `m × m` inverse stage per base call). Tests are
    // serialised because the counter is process-global.
    use crate::field::matrix::{fieldmatrix_new_count, reset_fieldmatrix_new_count};
    use serial_test::serial;

    /// `trsm_upper` and `trsm_lower` must perform ZERO
    /// `FieldMatrix::new` calls over a full recursive solve. The
    /// off-diagonal fold goes through the shared
    /// [`gemm_axpy_into_view`](crate::field::matrix::gemm_axpy_into_view)
    /// kernel, which transposes `B` in place via direct struct
    /// construction (`FieldMatrix::transpose` / `to_owned` bypass
    /// `FieldMatrix::new`); only the per-cell counter bumps when the
    /// caller materialises new matrices, and trsm/trmm don't.
    #[test]
    #[serial]
    fn test_trsm_zero_allocation() {
        // n = 65 forces recursion through several levels with the
        // default TRI_BASE_THRESHOLD = 32. The base case has no
        // allocation either (no snapshot, no inv buffer).
        let m = 65;
        let n = 6;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA0FC);
        let b_upper = random_fp::<MERSENNE_31>(m, n, 0xA0FD);
        let mut x_upper = b_upper.clone();
        reset_fieldmatrix_new_count();
        trsm_upper(a_upper.submat(.., ..), x_upper.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 0,
            "trsm_upper must not allocate any FieldMatrix::new during recursion (got {})",
            allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA0FE);
        let b_lower = random_fp::<MERSENNE_31>(m, n, 0xA0FF);
        let mut x_lower = b_lower.clone();
        reset_fieldmatrix_new_count();
        trsm_lower(a_lower.submat(.., ..), x_lower.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 0,
            "trsm_lower must not allocate any FieldMatrix::new during recursion (got {})",
            allocs
        );
    }

    /// `trmm_upper` and `trmm_lower` must perform ZERO
    /// `FieldMatrix::new` calls. Same contract as `trsm`: the
    /// off-diagonal `B += A · B'` step goes through
    /// [`gemm_axpy_into_view`](crate::field::matrix::gemm_axpy_into_view),
    /// which only materialises a `B`-transpose via the direct-struct
    /// path that bypasses the counter.
    #[test]
    #[serial]
    fn test_trmm_zero_allocation() {
        let m = 65;
        let n = 6;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA1FC);
        let b = random_fp::<MERSENNE_31>(m, n, 0xA1FD);
        let mut got_upper = b.clone();
        reset_fieldmatrix_new_count();
        trmm_upper(a_upper.submat(.., ..), got_upper.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 0,
            "trmm_upper must not allocate any FieldMatrix::new during recursion (got {})",
            allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA1FE);
        let mut got_lower = b.clone();
        reset_fieldmatrix_new_count();
        trmm_lower(a_lower.submat(.., ..), got_lower.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 0,
            "trmm_lower must not allocate any FieldMatrix::new during recursion (got {})",
            allocs
        );
    }

    /// `trtri` allocates exactly the documented scratch:
    ///  - ONE `h × (m-h)` chained-multiply scratch per recursive level
    ///    (algorithm 2.3 step `A12 := −A11_inv · A12 · A22_inv`).
    ///  - ONE `m × m` inverse buffer per base-case leaf.
    ///
    /// At `m = 65, threshold = 32` the recursion peels once: top-level
    /// scratch (1) + 2 base calls (one for A11 at h=32, one for A22 at
    /// m-h=33) → 3 FieldMatrix::new invocations.
    #[test]
    #[serial]
    fn test_trtri_allocation_budget() {
        // m = 64 with threshold = 32: exactly one recursion level,
        // both halves land in the base case at h = 32. Total allocs:
        // 1 outer chained-multiply scratch (h × (m-h) = 32 × 32) + 2
        // base-case inv buffers (one per leaf trtri_upper_base call).
        // Total = 3.
        let m = 64;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA2FC);
        let mut a_inv = a_upper.clone();
        reset_fieldmatrix_new_count();
        trtri_upper(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 3,
            "trtri_upper at m={} (threshold=32) expected 3 FieldMatrix::new \
             calls (1 outer scratch + 2 leaf inv); got {}",
            m, allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA2FD);
        let mut a_inv = a_lower.clone();
        reset_fieldmatrix_new_count();
        trtri_lower(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 3,
            "trtri_lower at m={} expected 3 FieldMatrix::new calls; got {}",
            m, allocs
        );
    }

    /// `trtri` at exactly the base-case size (m == TRI_BASE_THRESHOLD)
    /// allocates exactly ONE `m × m` inverse buffer — no recursive
    /// scratch.
    #[test]
    #[serial]
    fn test_trtri_at_threshold_one_allocation() {
        let m = <Fp<MERSENNE_31> as FiniteField>::TRI_BASE_THRESHOLD;
        let a = random_upper_fp::<MERSENNE_31>(m, 0xA3FC);
        let mut a_inv = a.clone();
        reset_fieldmatrix_new_count();
        trtri_upper(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 1,
            "trtri_upper at base-case size (m={}) expected 1 FieldMatrix::new \
             (the inv buffer); got {}",
            m, allocs
        );
    }

    /// `trtrm` allocates exactly ONE `(m-h) × h` scratch per recursion
    /// level (the `L21 · U11` chain — the only step whose write
    /// region aliases its read region). Base case is allocation-free.
    /// At m = 64, threshold = 32: 1 recursion level → 1 scratch, both
    /// sub-recursions fall straight into the base case.
    #[test]
    #[serial]
    fn test_trtrm_allocation_budget() {
        let m = 64;
        // Build a unit-lower L (diagonal implicit zero in storage) and
        // an upper U.
        let mut l = random_fp::<MERSENNE_31>(m, m, 0xA4FC);
        for r in 0..m {
            for c in r..m {
                l.set(r, c, Fp::<MERSENNE_31>::new(0));
            }
        }
        let mut u = random_fp::<MERSENNE_31>(m, m, 0xA4FD);
        for r in 0..m {
            for c in 0..r {
                u.set(r, c, Fp::<MERSENNE_31>::new(0));
            }
        }
        reset_fieldmatrix_new_count();
        trtrm(l.submat_mut(.., ..), u.submat(.., ..));
        let allocs = fieldmatrix_new_count();
        // 1 outer L21·U11 scratch ((m-h) × h = 32 × 32). Base case
        // does not allocate. Two recursive sub-calls (L11 and L22)
        // each fall through to the base case → 0 each.
        assert_eq!(
            allocs, 1,
            "trtrm at m={} (threshold=32) expected 1 FieldMatrix::new \
             (the L21·U11 chain scratch); got {}",
            m, allocs
        );
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
