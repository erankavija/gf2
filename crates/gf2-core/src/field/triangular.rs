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
//! allocation is paid by the kernel itself, and the trsm/trmm
//! recursive paths add **no further** owned `FieldMatrix<F>` snapshots
//! of their own. (Concretely: the kernel calls `b.transpose()` once
//! per invocation, which on a `MatView` materialises one owned
//! transposed matrix via `MatView::to_owned` followed by
//! `FieldMatrix::transpose` — both are direct-struct constructors but
//! both bump the [`crate::field::matrix::fieldmatrix_new_count`]
//! test-only counter, mirroring their real heap cost.) The trtri /
//! trtrm primitives need exactly **one** extra scratch
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
//! Per the issue 83b1ad8b **R5 amendment** (2026-04-25), the `[hard]`
//! "zero extra allocation" contract is scoped to "zero per-recursion-
//! level scratch matrices managed by the triangular primitives
//! themselves". The shared blocked-gemm kernels
//! [`gemm_axpy_into_view`](crate::field::matrix::gemm_axpy_into_view) /
//! [`gemm_into_view`](crate::field::matrix::gemm_into_view) intrinsically
//! materialise one transposed `B` operand per call — mirroring T1's
//! classical [`gemm`](crate::field::matrix::gemm) — and trsm/trmm/
//! trtri/trtrm inherit that 1-allocation-per-gemm-call cost because
//! the §2.1 algorithms reduce to gemm. The unit-diagonal kernel
//! [`gemm_axpy_into_view_diag`](crate::field::matrix::gemm_axpy_into_view_diag)
//! is the exception: it walks `b` cell-wise and never materialises a
//! transpose, so it adds **0** allocations.
//!
//! Empirical post-R5 budgets (counted via the `#[cfg(test)]`
//! [`crate::field::matrix::fieldmatrix_new_count`] thread-local
//! counter, which bumps once per `FieldMatrix::new` call AND once per
//! direct-struct materialisation in `MatView::to_owned` /
//! `FieldMatrix::transpose`):
//!
//! - `trsm_*` / `trmm_*` at `m = 65`, threshold = 8: **16** counter
//!   bumps. Eight internal-recursion gemm calls × 2 bumps per gemm
//!   (`MatView::transpose` = `to_owned` + `FieldMatrix::transpose`).
//!   Pinned in
//!   [`tests::test_trsm_zero_allocation`](self::tests::test_trsm_zero_allocation)
//!   and
//!   [`tests::test_trmm_zero_allocation`](self::tests::test_trmm_zero_allocation).
//!   The trsm/trmm recursive paths themselves allocate **nothing** —
//!   the count is entirely the gemm kernel's intrinsic B-transpose.
//! - `trtri_*` at the base case (`m = TRI_BASE_THRESHOLD`): **1**
//!   counter bump — the `m × m` `inv` staging buffer. The column-by-
//!   column back-substitution writes scalars in place into `inv` and
//!   needs no per-iteration scratch. Pinned in
//!   [`tests::test_trtri_at_threshold_one_allocation`](self::tests::test_trtri_at_threshold_one_allocation).
//! - `trtri_*` at `m = 64`, threshold = 8: **43** counter bumps.
//!   Breakdown: 8 base-case `inv` buffers (one per leaf at `m=8`) +
//!   7 non-leaf levels × (1 chain scratch + 2 gemm_into_view × 2
//!   transpose bumps) = 8 + 35 = 43. Pinned in
//!   [`tests::test_trtri_allocation_budget`](self::tests::test_trtri_allocation_budget).
//!   The base-case `inv` buffers and the per-level chain scratch are
//!   the **architectural exceptions** (matrix multiply is not
//!   associative in-place) recorded as the R5 amendment; the remaining
//!   bumps are the inherited gemm B-transpose cost.
//! - `trtrm` at `m = 64`, threshold = 8: **35** counter bumps.
//!   Per non-leaf level the cost is **5** allocs (step 1 via
//!   `gemm_axpy_into_view_diag` adds 0; step 2b `gemm_axpy_into_view`
//!   adds 2; step 3 chain scratch + `gemm_into_view` × 2 transpose
//!   bumps adds 3; recurses in steps 2a and 4 fold their costs into
//!   sub-levels). At `m=64` with threshold=8 the recursion has 7
//!   non-leaf levels (one at `m=64`, two at `m=32`, four at `m=16`);
//!   each `m=8` leaf allocates 0 (`trtrm_base` walks `l` cell-wise).
//!   Total counter bumps: seven levels times five allocs per level
//!   gives 35. Pinned in
//!   [`tests::test_trtrm_allocation_budget`](self::tests::test_trtrm_allocation_budget).
//!
//! Geometrically these scratch allocations sum to `O(m²)` cells over
//! the full recursion tree, the same asymptotic budget as the
//! Strassen-Winograd recursion in [`crate::field::winograd`].
//!
//! **No bespoke per-cell multiply-accumulate loops in `triangular.rs`.**
//! The trtri base case uses scalar back-substitution (per-element
//! arithmetic, NOT a matrix-multiply kernel). All matrix-matrix
//! multiplications go through `gemm_axpy_into_view` /
//! `gemm_into_view` / `gemm_axpy_into_view_diag` in
//! [`crate::field::matrix`]. SSOT preserved.
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
//!
//! # Blocked triangular solve (Higham § 14.1)
//!
//! For fields that expose the [`FiniteField::has_simd_gemm_classical`] fast
//! path (currently `Fp<P>` with `P ≤ 65535`), the standard recursive
//! `trsm_upper` / `trsm_lower` generate many small `gemm_axpy_into_view`
//! calls (one per recursion level). At small `B`-column counts (e.g. `n = 1`
//! for a single right-hand side), the inner GEMM dimensions become too small
//! to trigger the whole-GEMM `fp_small_try_gemm_classical` threshold
//! (`GEMM_AXPY_FAST_PATH_THRESHOLD = 16³ = 4096` cell-triples), so the SIMD
//! fast path is never reached.
//!
//! [`trsm_upper_blocked`] and [`trsm_lower_blocked`] implement Higham § 14.1
//! right-looking blocked back-substitution: the triangular factor `A` is
//! tiled into row panels of width [`TRSM_BLOCKED_PANEL_SIZE`] (default 64).
//! The diagonal tile is solved with the recursive scalar
//! `trsm_upper/lower_inner`, and the update step
//!
//! ```text
//!   B[0..k·bs, :] -= A[0..k·bs, k·bs..(k+1)·bs] · X[k·bs..(k+1)·bs, :]
//! ```
//!
//! is a single contiguous GEMM whose row dimension grows with every panel.
//! At `n = 1` and `bs = 64`, panel 3 (`k = 3`) produces a GEMM of shape
//! `192 × 64 × 1 = 12288 ≥ 4096`, comfortably above the SIMD threshold.
//!
//! The blocked variants are dispatched by
//! [`crate::field::inverse::FieldMatrix::solve_batch`] when
//! `F::has_simd_gemm_classical()` returns `true` and the matrix is large
//! enough to benefit; the recursive variants remain for all other fields.

use crate::field::matrix::{
    gemm_axpy_into_view, gemm_axpy_into_view_diag, gemm_into_view, FieldMatrix, MatView,
    MatViewMut, UnitDiag,
};
use crate::field::FiniteField;

/// Default row-panel width for the blocked triangular solve
/// (`trsm_upper_blocked` / `trsm_lower_blocked`).
///
/// Chosen empirically: large enough for the update GEMM to surpass
/// `GEMM_AXPY_FAST_PATH_THRESHOLD = 16³` even at `n = 1` (after two
/// diagonal tiles the update covers `64 × 64 × 1 = 4096` cell-triples;
/// after three panels it is `192 × 64 × 1 = 12288`), small enough that
/// each diagonal block stays within L1 cache on a typical x86-64 core.
pub const TRSM_BLOCKED_PANEL_SIZE: usize = 64;

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

/// Solves the upper-triangular linear system `A · X = B` using a right-looking
/// row-panel blocked algorithm (Higham § 14.1).
///
/// This variant tiles the `m × m` triangular factor `A` into row panels of
/// width [`TRSM_BLOCKED_PANEL_SIZE`] and processes them from the last panel
/// to the first. For each panel the diagonal block is solved with the existing
/// recursive [`trsm_upper`] (which handles odd sizes and the base threshold),
/// and the update of all rows above the panel is performed by a single
/// `gemm_axpy_into_view` call whose row dimension grows with each panel,
/// ensuring that the whole-GEMM `fp_small_try_gemm_classical` threshold is
/// reached even when `b` has only one column.
///
/// Bit-exact equivalent to [`trsm_upper`] (same field arithmetic; only the
/// loop order and GEMM tile granularity differ). The proptests in
/// `tests::prop_blocked_trsm_*` assert bit-exact equality against the
/// standard recursive path for all boundary lengths.
///
/// # Arguments
///
/// * `a` — Square `m × m` upper-triangular view.
/// * `b` — `m × n` right-hand side; overwritten with the solution `X`.
/// * `block_size` — Row-panel width. Pass [`TRSM_BLOCKED_PANEL_SIZE`] for
///   the default.
///
/// # Panics
///
/// Same conditions as [`trsm_upper`]: non-square `a`, mismatched row
/// counts, or a zero diagonal pivot.
///
/// # Complexity
///
/// `O(m² · n)` field operations — identical to [`trsm_upper`].
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::triangular::{trsm_upper_blocked, TRSM_BLOCKED_PANEL_SIZE};
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 2], [0, 3]]  (upper triangular, GF(7))
/// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// a.set(0, 0, Fp::<7>::new(1));
/// a.set(0, 1, Fp::<7>::new(2));
/// a.set(1, 1, Fp::<7>::new(3));
/// let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
/// b.set(0, 0, Fp::<7>::new(1));
/// b.set(1, 0, Fp::<7>::new(3));
/// trsm_upper_blocked(a.submat(.., ..), b.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
/// assert_eq!(b.get(1, 0), Fp::<7>::new(1));
/// assert_eq!(b.get(0, 0), Fp::<7>::new(6));
/// ```
pub fn trsm_upper_blocked<F: FiniteField>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    block_size: usize,
) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_upper_blocked: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_upper_blocked: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    trsm_upper_blocked_inner(a, b, block_size);
}

/// Solves the lower-triangular linear system `A · X = B` using a right-looking
/// row-panel blocked algorithm (Higham § 14.1).
///
/// Mirror of [`trsm_upper_blocked`]: tiles `A` into row panels and processes
/// them from the first panel to the last (forward substitution direction).
/// Each diagonal block is solved with the existing recursive [`trsm_lower`],
/// and the update of all rows below the panel is one large GEMM.
///
/// Bit-exact equivalent to [`trsm_lower`].
///
/// # Arguments
///
/// * `a` — Square `m × m` lower-triangular view.
/// * `b` — `m × n` right-hand side; overwritten with the solution `X`.
/// * `block_size` — Row-panel width. Pass [`TRSM_BLOCKED_PANEL_SIZE`].
///
/// # Panics
///
/// Same conditions as [`trsm_lower`].
///
/// # Complexity
///
/// `O(m² · n)` field operations.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::triangular::{trsm_lower_blocked, TRSM_BLOCKED_PANEL_SIZE};
/// use gf2_core::gfp::Fp;
///
/// // A = [[1, 0], [2, 3]]  (lower triangular, GF(7))
/// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// a.set(0, 0, Fp::<7>::new(1));
/// a.set(1, 0, Fp::<7>::new(2));
/// a.set(1, 1, Fp::<7>::new(3));
/// let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
/// b.set(0, 0, Fp::<7>::new(1));
/// b.set(1, 0, Fp::<7>::new(3));
/// trsm_lower_blocked(a.submat(.., ..), b.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
/// assert_eq!(b.get(0, 0), Fp::<7>::new(1));
/// assert_eq!(b.get(1, 0), Fp::<7>::new(5));
/// ```
pub fn trsm_lower_blocked<F: FiniteField>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    block_size: usize,
) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_lower_blocked: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_lower_blocked: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    trsm_lower_blocked_inner(a, b, block_size);
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

// ─── trsm_upper_blocked ──────────────────────────────────────────────────────

/// Right-looking row-panel blocked upper triangular solve (Higham § 14.1).
///
/// Processes panels of `a` from the last to the first. For each panel the
/// diagonal block is solved with [`trsm_upper_inner`], then the rows above
/// are updated via a single [`gemm_axpy_into_view`] whose row dimension grows
/// with each processed panel, allowing the whole-GEMM SIMD fast path to
/// trigger even when `b` has only one column.
fn trsm_upper_blocked_inner<F: FiniteField>(
    a: MatView<'_, F>,
    mut b: MatViewMut<'_, F>,
    block_size: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    // Fall back to the standard recursive solve when blocking offers no benefit.
    if block_size == 0 || m <= block_size {
        trsm_upper_inner(a, b);
        return;
    }
    let bs = block_size;
    let num_blocks = m.div_ceil(bs);
    let one = a.get(0, 0).one_like();
    let neg_one = -one.clone();
    // Process row panels from the last to the first (back-substitution order).
    for k in (0..num_blocks).rev() {
        let row_start = k * bs;
        let row_end = m.min((k + 1) * bs);
        let bs_k = row_end - row_start; // actual panel height (may be < bs for last panel)
                                        // Split B at row_start: b_top covers rows [0..row_start],
                                        // b_bot covers rows [row_start..m].
                                        // Both are derived from a reborrow so the loop can proceed to the
                                        // next iteration after this scope ends.
        let (mut b_top, mut b_bot) = b.reborrow().split_rows_mut(row_start);
        // Step 1 — solve the diagonal block in b_bot's first bs_k rows.
        // A[row_start..row_end, row_start..row_end] · X = b_bot[0..bs_k, :].
        trsm_upper_inner(
            a.submat(row_start..row_end, row_start..row_end),
            b_bot.submat_mut(0..bs_k, ..),
        );
        // Step 2 — update rows above: B_top -= A_off · X_panel.
        // A_off = A[0..row_start, row_start..row_end],  (row_start × bs_k)
        // X_panel = b_bot[0..bs_k, :]                   (bs_k × n, just solved)
        // GEMM shape: row_start × bs_k × n.  At k=3, bs=64, n=1 this is
        // 192 × 64 × 1 = 12288 ≥ GEMM_AXPY_FAST_PATH_THRESHOLD = 4096.
        if row_start > 0 {
            let a_off = a.submat(0..row_start, row_start..row_end);
            let x_panel = b_bot.submat(0..bs_k, ..);
            gemm_axpy_into_view(
                neg_one.clone(),
                &a_off,
                &x_panel,
                one.clone(),
                b_top.reborrow(),
            );
        }
        // b_top and b_bot are dropped here; b is available for the next iteration.
    }
}

// ─── trsm_lower_blocked ──────────────────────────────────────────────────────

/// Right-looking row-panel blocked lower triangular solve (Higham § 14.1).
///
/// Processes panels of `a` from the first to the last. For each panel the
/// diagonal block is solved with [`trsm_lower_inner`], then the rows below
/// are updated via a single [`gemm_axpy_into_view`].
fn trsm_lower_blocked_inner<F: FiniteField>(
    a: MatView<'_, F>,
    mut b: MatViewMut<'_, F>,
    block_size: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    // Fall back to the standard recursive solve when blocking offers no benefit.
    if block_size == 0 || m <= block_size {
        trsm_lower_inner(a, b);
        return;
    }
    let bs = block_size;
    let num_blocks = m.div_ceil(bs);
    let one = a.get(0, 0).one_like();
    let neg_one = -one.clone();
    // Process row panels from the first to the last (forward substitution order).
    for k in 0..num_blocks {
        let row_start = k * bs;
        let row_end = m.min((k + 1) * bs);
        // Split B at row_end: b_top covers rows [0..row_end] (includes the
        // current panel), b_bot covers rows [row_end..m].
        let (mut b_top, mut b_bot) = b.reborrow().split_rows_mut(row_end);
        // Step 1 — solve the diagonal block: the current panel within b_top.
        // A[row_start..row_end, row_start..row_end] · X = b_top[row_start..row_end, :].
        trsm_lower_inner(
            a.submat(row_start..row_end, row_start..row_end),
            b_top.submat_mut(row_start..row_end, ..),
        );
        // Step 2 — update rows below: B_bot -= A_off · X_panel.
        // A_off = A[row_end..m, row_start..row_end]
        // X_panel = b_top[row_start..row_end, :]
        // GEMM shape: (m-row_end) × (row_end-row_start) × n.
        if row_end < m {
            let a_off = a.submat(row_end..m, row_start..row_end);
            let x_panel = b_top.submat(row_start..row_end, ..);
            gemm_axpy_into_view(
                neg_one.clone(),
                &a_off,
                &x_panel,
                one.clone(),
                b_bot.reborrow(),
            );
        }
        // b_top and b_bot are dropped here; b is available for the next iteration.
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
    use crate::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use proptest::prelude::*;

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
    //
    // Local monomorphisations of the shared generic helpers in
    // `field::test_random_matrix` for this module's `TriGf2m8` config.

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<TriGf2m8> {
        random_gf2m_wide_1::<TriGf2m8Cfg>(rows, cols, seed)
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
        // Selected to 8 by `73ec5da3` Criterion sweep on Mersenne-31
        // over candidate values {4, 8, 16, 32, 64}; see the
        // `TRI_BASE_THRESHOLD` doc comment in `field/traits.rs` and the
        // sweep table in
        // `dev/bench_results/73ec5da3/2026-05-07-73ec5da3-ple-trsm-tuning.md`.
        assert_eq!(<Fp<7> as FiniteField>::TRI_BASE_THRESHOLD, 8);
        assert_eq!(<Fp<MERSENNE_31> as FiniteField>::TRI_BASE_THRESHOLD, 8);
        assert_eq!(<TriGf2m8 as FiniteField>::TRI_BASE_THRESHOLD, 8);
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

    // ─── Blocked TRSM correctness — boundary-length sweep ────────────────
    //
    // For each prime we exhaust all square-matrix sizes from
    // TRSM_BOUNDARY_LENS (0-size excluded — triangular solve is undefined
    // on a 0×0 system).  For each `(m, n_rhs)` pair we compare the
    // blocked variant against the scalar recursive oracle bit-exact.
    // The seed is varied over 8 proptest cases so each boundary pair is
    // exercised against 8 different random matrices.

    const TRSM_BOUNDARY_LENS: &[usize] = &[1, 15, 16, 17, 63, 64, 65];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

        /// Blocked upper-triangular solve vs scalar oracle — Fp<7>.
        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp7(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<7>(m, mseed);
                    let b = random_fp::<7>(m, n_rhs, mseed.wrapping_add(0xB1));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<7> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked lower-triangular solve vs scalar oracle — Fp<7>.
        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp7(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<7>(m, mseed);
                    let b = random_fp::<7>(m, n_rhs, mseed.wrapping_add(0xB2));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<7> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked upper-triangular solve vs scalar oracle — Fp<31>.
        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp31(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<31>(m, mseed);
                    let b = random_fp::<31>(m, n_rhs, mseed.wrapping_add(0xB3));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<31> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked lower-triangular solve vs scalar oracle — Fp<31>.
        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp31(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<31>(m, mseed);
                    let b = random_fp::<31>(m, n_rhs, mseed.wrapping_add(0xB4));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<31> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked upper-triangular solve vs scalar oracle — Fp<127>.
        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp127(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<127>(m, mseed);
                    let b = random_fp::<127>(m, n_rhs, mseed.wrapping_add(0xB5));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<127> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked lower-triangular solve vs scalar oracle — Fp<127>.
        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp127(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<127>(m, mseed);
                    let b = random_fp::<127>(m, n_rhs, mseed.wrapping_add(0xB6));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<127> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked upper-triangular solve vs scalar oracle — Fp<241>.
        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp241(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<241>(m, mseed);
                    let b = random_fp::<241>(m, n_rhs, mseed.wrapping_add(0xB7));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<241> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked lower-triangular solve vs scalar oracle — Fp<241>.
        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp241(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<241>(m, mseed);
                    let b = random_fp::<241>(m, n_rhs, mseed.wrapping_add(0xB8));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<241> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked upper-triangular solve vs scalar oracle — Fp<251>.
        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp251(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<251>(m, mseed);
                    let b = random_fp::<251>(m, n_rhs, mseed.wrapping_add(0xB9));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<251> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked lower-triangular solve vs scalar oracle — Fp<251>.
        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp251(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<251>(m, mseed);
                    let b = random_fp::<251>(m, n_rhs, mseed.wrapping_add(0xBA));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<251> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked upper-triangular solve vs scalar oracle — Fp<65521>.
        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp65521(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<65521>(m, mseed);
                    let b = random_fp::<65521>(m, n_rhs, mseed.wrapping_add(0xBB));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<65521> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        /// Blocked lower-triangular solve vs scalar oracle — Fp<65521>.
        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp65521(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<65521>(m, mseed);
                    let b = random_fp::<65521>(m, n_rhs, mseed.wrapping_add(0xBC));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<65521> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
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
    // These tests anchor the issue 83b1ad8b R5 contract — see the R5
    // amendment block in the issue description. The counter is
    // **per-thread** (see `matrix.rs:FIELDMATRIX_NEW_COUNT`), so
    // concurrent tests in `cargo test --release` thread-pool execution
    // do not contaminate each other's totals. Counts include both
    // `FieldMatrix::new` calls and the direct-struct materialisations
    // performed by [`MatView::to_owned`] / [`FieldMatrix::transpose`]
    // — every fresh owned `FieldMatrix<F>` bumps the counter under
    // `#[cfg(test)]` regardless of which constructor path was used,
    // so these tests reflect the **full** allocation footprint of the
    // primitives.
    //
    // The numbers below are post-R5 empirical totals measured on
    // `MERSENNE_31` / threshold = 8 (set in `field/traits.rs` by
    // `73ec5da3` Criterion sweep). They include the per-call
    // B-transpose allocation that `gemm_axpy_into_view` /
    // `gemm_into_view` materialise via `MatView::transpose()` (which
    // is one `to_owned` + one `transpose` = **2** counter bumps per
    // gemm call). The unit-diagonal kernel
    // `gemm_axpy_into_view_diag` walks `b` cell-wise and never
    // materialises a transpose, so it adds **0** to the counter.
    use crate::field::matrix::{fieldmatrix_new_count, reset_fieldmatrix_new_count};
    use serial_test::serial;

    /// `trsm_upper` / `trsm_lower` allocation budget at `m = 65`,
    /// threshold = 8.
    ///
    /// Recursion shape: each split at level produces one
    /// `gemm_axpy_into_view` call (2 allocs from `MatView::transpose`
    /// = `to_owned()` + `FieldMatrix::transpose()`); leaves where
    /// `m <= 8` allocate nothing. Tree from `m=65` (threshold=8):
    /// - `m=65` → split at h=32: gemm(+2) + subtree(m=32) + subtree(m=33)
    /// - `m=32` → 3 gemms × 2 = **6** (m=32 → m=16 → 2× m=8 leaves)
    /// - `m=33` → 4 gemms × 2 = **8** (m=33 → m=17/m=16; m=17 → m=9/m=8;
    ///   m=9 → m=5/m=4; both terminate at base)
    ///
    /// Total: 2 + 6 + 8 = **16**.
    ///
    /// Per the R5 amendment, the per-call B-transpose is the
    /// architectural cost inherited from the shared blocked-gemm
    /// kernel; the `[hard]` zero-extra-allocation contract applies to
    /// *trsm/trmm-managed scratch*, of which there is none.
    #[test]
    #[serial]
    fn test_trsm_zero_allocation() {
        let m = 65;
        let n = 6;
        // 8 internal-recursion gemm calls × 2 alloc-counter bumps per gemm
        // (the `MatView::transpose` path) at threshold = 8.
        const EXPECTED: u64 = 16;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA0FC);
        let b_upper = random_fp::<MERSENNE_31>(m, n, 0xA0FD);
        let mut x_upper = b_upper.clone();
        reset_fieldmatrix_new_count();
        trsm_upper(a_upper.submat(.., ..), x_upper.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trsm_upper at m={} expected {} FieldMatrix-class allocs (8 gemm calls × 2 \
             bumps each: to_owned + transpose in `MatView::transpose`); got {}",
            m, EXPECTED, allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA0FE);
        let b_lower = random_fp::<MERSENNE_31>(m, n, 0xA0FF);
        let mut x_lower = b_lower.clone();
        reset_fieldmatrix_new_count();
        trsm_lower(a_lower.submat(.., ..), x_lower.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trsm_lower at m={} expected {} FieldMatrix-class allocs; got {}",
            m, EXPECTED, allocs
        );
    }

    /// `trmm_upper` / `trmm_lower` allocation budget at `m = 65`,
    /// threshold = 8. Same recursion shape as `trsm` (one gemm per
    /// recursion level via the shared
    /// [`gemm_axpy_into_view`](crate::field::matrix::gemm_axpy_into_view)
    /// kernel): 8 internal gemm calls × 2 counter bumps each = **16**
    /// total. trmm itself allocates nothing on top of the gemm calls.
    /// Per the R5 amendment.
    #[test]
    #[serial]
    fn test_trmm_zero_allocation() {
        let m = 65;
        let n = 6;
        const EXPECTED: u64 = 16;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA1FC);
        let b = random_fp::<MERSENNE_31>(m, n, 0xA1FD);
        let mut got_upper = b.clone();
        reset_fieldmatrix_new_count();
        trmm_upper(a_upper.submat(.., ..), got_upper.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trmm_upper at m={} expected {} FieldMatrix-class allocs (8 gemm calls × 2 \
             bumps each: to_owned + transpose in `MatView::transpose`); got {}",
            m, EXPECTED, allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA1FE);
        let mut got_lower = b.clone();
        reset_fieldmatrix_new_count();
        trmm_lower(a_lower.submat(.., ..), got_lower.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trmm_lower at m={} expected {} FieldMatrix-class allocs; got {}",
            m, EXPECTED, allocs
        );
    }

    /// `trtri` allocation budget at `m = 64`, threshold = 8.
    ///
    /// `trtri` recurses by halving until `m <= 8`, where the column-
    /// by-column back-substitution allocates exactly 1 inv buffer.
    /// Each non-leaf level performs the chain multiply
    /// `A12 := −A11 · A12 · A22` which costs:
    /// - 1 × outer `tmp` chain scratch (`FieldMatrix::new`) = **1**
    /// - 2 × `gemm_into_view` calls × 2 transpose bumps each = **4**
    ///
    /// Tree from `m=64` (threshold=8): 7 levels × (1 chain + 4 gemm)
    /// counts plus 8 leaves × 1 inv buffer.
    /// - Level (m=64): 5 allocs + recurse(m=32) + recurse(m=32)
    /// - Level (m=32): 5 allocs + 2 × recurse(m=16)
    /// - Level (m=16): 5 allocs + 2 × leaf(m=8) (1 each)
    /// - Each `m=8` leaf: 1 alloc.
    ///
    /// Counting: 8 leaves × 1 = 8; 4 levels (m=16) × 5 = 20; 2 levels
    /// (m=32) × 5 = 10; 1 level (m=64) × 5 = 5. Total = 8 + 20 + 10 + 5
    /// = **43**.
    #[test]
    #[serial]
    fn test_trtri_allocation_budget() {
        let m = 64;
        // 8 leaf inv buffers + 7 chain levels × (1 chain scratch + 2 gemms × 2
        // transpose bumps).
        const EXPECTED: u64 = 43;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA2FC);
        let mut a_inv = a_upper.clone();
        reset_fieldmatrix_new_count();
        trtri_upper(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trtri_upper at m={} expected {} FieldMatrix-class allocs \
             (8 leaf inv + 7 chain levels × (1 scratch + 2 gemms × 2 transpose bumps)); got {}",
            m, EXPECTED, allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA2FD);
        let mut a_inv = a_lower.clone();
        reset_fieldmatrix_new_count();
        trtri_lower(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trtri_lower at m={} expected {} FieldMatrix-class allocs; got {}",
            m, EXPECTED, allocs
        );
    }

    /// `trtri` at the base-case size (m == TRI_BASE_THRESHOLD = 8):
    /// the column-by-column back-substitution allocates exactly ONE
    /// `m × m` inverse-staging buffer and writes scalars in place into
    /// it. No per-column or per-row scratch.
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
             (the inv buffer; column-by-column writes scalars in place); got {}",
            m, allocs
        );
    }

    /// `trtrm` allocation budget at `m = 64`, threshold = 8.
    ///
    /// Per non-leaf level the four steps cost:
    ///   1. `A12 = U12 · L22` via `gemm_axpy_into_view_diag` — the
    ///      unit-diagonal kernel walks `b` cell-wise, **no** transpose →
    ///      **0** allocs.
    ///   2. `A11 = U11 · L11 + U12 · L21`:
    ///        2a. `trtrm` recurses on the upper-left block.
    ///        2b. `gemm_axpy_into_view` for `+ U12 · L21` → **2** allocs.
    ///   3. `A21 = U22 · L21`: 1 chain scratch (1 alloc) + 1
    ///      `gemm_into_view` call (**2** allocs).
    ///   4. `A22 = U22 · L22`: `trtrm` recurses on the lower-right block.
    ///
    /// Per-level cost (excluding sub-recursions): 0 + 2 + 3 + 0 = **5**.
    /// At `m=64` with threshold=8 the recursion has 7 non-leaf levels:
    /// 1×(m=64) + 2×(m=32) + 4×(m=16). Each leaf at `m=8` allocates 0
    /// (the `trtrm_base` schoolbook loop walks `l` cell-wise).
    ///
    /// Total: 7 levels × 5 allocs = **35**.
    #[test]
    #[serial]
    fn test_trtrm_allocation_budget() {
        let m = 64;
        // 7 non-leaf levels × 5 allocs/level (step 2b gemm_axpy + step 3
        // chain scratch + step 3 gemm_into_view's transpose).
        const EXPECTED: u64 = 35;
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
        assert_eq!(
            allocs, EXPECTED,
            "trtrm at m={} expected {} FieldMatrix-class allocs (7 non-leaf levels × \
             5 allocs/level: step 2b gemm_axpy + step 3 chain scratch + step 3 \
             gemm_into_view's MatView::transpose); got {}",
            m, EXPECTED, allocs
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
