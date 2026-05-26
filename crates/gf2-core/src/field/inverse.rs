//! Matrix inversion, linear-system solving, and determinant over an
//! arbitrary [`FiniteField`].
//!
//! Issue `ae1d1e88`. Implements Dumas–Pernet §2.3 Table 2 by composing the
//! PLE decomposition (issue `c3f8c1cb`) with the triangular primitives
//! (issue `83b1ad8b`):
//!
//! - [`FieldMatrix::inv`] / [`inv`] — `A⁻¹ = E⁻¹ · L⁻¹ · Pᵀ` where
//!   `(P, L, E, r) = self.ple()`. Defined iff `r == n == m`; returns
//!   `None` on rank-deficient input. Uses the in-place `trtrm` primitive
//!   (issue `d1a5fea8`) to compose `E⁻¹ · L⁻¹` directly into `L⁻¹`'s
//!   storage, replacing the prior dense `n × n` `gemm` target. This
//!   halves the constant factor on the dense `n³` work versus the
//!   pre-`d1a5fea8` driver and removes one full `n × n` allocation
//!   (the `temp = E⁻¹ · L⁻¹` materialisation).
//! - [`FieldMatrix::solve`] / [`solve`] — solve `A · x = b` for a single
//!   column `b`. Routes through [`solve_batch`](FieldMatrix::solve_batch)
//!   on a `n × 1` right-hand side.
//! - [`FieldMatrix::solve_batch`] / [`solve`] — solve `A · X = B` for a
//!   matrix `B`. Returns `None` on singular `A`. Uses one `trsm_lower`
//!   for `L · Y = Pᵀ B` and one `trsm_upper` for `E · X = Y`.
//! - [`FieldMatrix::det`] / [`det`] — `det(A) = sign(P) · ∏ E[i, i]`.
//!   Returns the field zero on rank-deficient input.
//!
//! All matrix–matrix multiplications go through
//! [`gemm_into_view`](crate::field::matrix::gemm_into_view); all
//! triangular solves through
//! [`trsm_lower`](crate::field::triangular::trsm_lower) /
//! [`trsm_upper`](crate::field::triangular::trsm_upper); all triangular
//! inversions through
//! [`trtri_lower`](crate::field::triangular::trtri_lower) /
//! [`trtri_upper`](crate::field::triangular::trtri_upper); the final
//! upper-times-unit-lower product through
//! [`trtrm`](crate::field::triangular::trtrm). No bespoke kernels.
//!
//! # Allocation budget
//!
//! Each operation's `FieldMatrix::new` count is pinned in
//! `tests::test_*_allocation_budget_*`. The dominant cost is paid by the
//! upstream PLE call (see [`crate::field::ple`]) and by the kernels'
//! intrinsic gemm B-transpose materialisation; the inverse / solve
//! drivers add only the small handful of clones documented per
//! function.
//!
//! # Edge cases
//!
//! `n == 0` is supported (the 0×0 matrix has determinant 1, inverse
//! itself, and the trivial empty system has the unique empty solution).
//! `n == 1` reduces to scalar inversion when `A[0,0] != 0` and to
//! `None`/zero determinant when `A[0,0] == 0`. Singular and
//! rank-deficient inputs never panic; they return `None` (inv, solve)
//! or the field zero (det).
//!
//! # Non-square inputs
//!
//! `inv`, `solve`, `solve_batch`, and `det` panic with a clear message on
//! non-square `self`. The pseudo-inverse for rectangular / rank-
//! deficient matrices is intentionally out of scope; callers needing
//! that should use [`crate::field::ple`]'s row-echelon form together
//! with [`crate::field::ple::FieldMatrix::nullspace`] to assemble the
//! Moore–Penrose pseudo-inverse manually.

use crate::field::matrix::FieldMatrix;
use crate::field::triangular::{
    trsm_lower, trsm_lower_blocked, trsm_upper, trsm_upper_blocked, trtri_lower, trtri_upper,
    trtrm, TRSM_BLOCKED_PANEL_SIZE,
};
use crate::field::vec::FieldVec;
use crate::field::FiniteField;

// ─── Blocked-invert constants ─────────────────────────────────────────────────

/// Minimum matrix size at which `FieldMatrix::inv` takes the panelized
/// fast path instead of the scalar-pivot driver.
///
/// Below this threshold the scalar-PLE + `trtri` + `trtrm` driver is
/// competitive with (or faster than) the panelized path because the
/// GEMM inner dimensions are too small to amortise the packing overhead
/// of `fp_small_try_gemm_classical` / `gemm_axpy_into_view`. The value
/// 16 was selected empirically: the `fieldmatrix_solve` bench shows a
/// crossover for small primes in the range n ∈ [14, 18] across
/// GF(7), GF(251), and GF(65521) (see `dev/bench_results/
/// 2026-05-26-8df0c501-blocked-invert.md` § 2 for the sweep).
/// For n ≥ 16 the panelized path is equal-or-faster on every prime tested.
const BLOCKED_INVERT_THRESHOLD: usize = 16;

// ─── Public methods on FieldMatrix ───────────────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns the matrix inverse `A⁻¹` if `self` is non-singular.
    ///
    /// Implements Dumas–Pernet §2.3 Table 2 with the §5.2 in-place
    /// composition variant (issue `d1a5fea8`). Computes the PLE
    /// decomposition `P · L · E = self`. If `rank < n`, returns `None`.
    /// Otherwise inverts each triangular factor in place
    /// ([`trtri_lower`](crate::field::triangular::trtri_lower) on `L`,
    /// [`trtri_upper`](crate::field::triangular::trtri_upper) on `E`),
    /// then composes `M = E⁻¹ · L⁻¹` **in place into `L⁻¹`'s storage**
    /// via [`trtrm`](crate::field::triangular::trtrm) (the upper-times-
    /// unit-lower product kernel that exploits `L⁻¹`'s unit-lower
    /// structure to halve the dense work versus a generic `gemm`).
    /// Finally applies `Pᵀ` on the right by column-permuting `M` into
    /// the result.
    ///
    /// **Algorithm-choice note.** Prior versions of this driver used a
    /// full `n × n` `gemm_into_view` for the `E⁻¹ · L⁻¹` step, requiring
    /// a fresh `n × n` allocation and ~`n³` field operations on top of
    /// the two triangular inversions. The `trtrm` formulation reuses
    /// `L⁻¹`'s storage (no extra `n × n` allocation) and reduces that
    /// step's cost to ~`n³ / 2` because one operand is unit lower-
    /// triangular. The total dense `n³` work drops from `≈ 1.33 n³`
    /// (PLE-share excluded) to `≈ 0.83 n³`, closing the gap to
    /// fflas-ffpack's `dgetri`-style in-place driver. See
    /// `dev/bench_results/2026-05-07-d1a5fea8-invert-inplace.md` for
    /// the per-cell ratios.
    ///
    /// # Arguments
    ///
    /// * `self` — Square `n × n` input. Not modified.
    ///
    /// # Returns
    ///
    /// `Some(A⁻¹)` if `self` is invertible (i.e. `rank(self) == n`),
    /// `None` otherwise. Never panics on a singular input.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations (one PLE + two triangular inversions +
    /// one in-place upper-times-unit-lower product via `trtrm`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::{gemm, FieldMatrix};
    /// use gf2_core::gfp::Fp;
    ///
    /// // A = [[2, 3], [1, 4]] over GF(7).
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 0, Fp::<7>::new(2));
    /// a.set(0, 1, Fp::<7>::new(3));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(4));
    /// let a_inv = a.inv().expect("invertible");
    /// let prod = gemm(&a, &a_inv);
    /// assert_eq!(prod, FieldMatrix::<Fp<7>>::identity(2));
    /// ```
    pub fn inv(&self) -> Option<FieldMatrix<F>> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::inv: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            // 0×0 matrix: its own inverse (the unique empty linear map).
            // The output carries empty storage; a zero witness is not
            // required for either ConstField or runtime-context fields.
            return Some(self.clone());
        }

        // Panelized fast path (issue 8df0c501, design feb15da9).
        //
        // For n ≥ BLOCKED_INVERT_THRESHOLD the blocked-invert algorithm
        // (Higham §14.1) replaces the scalar-pivot PLE + trtri + trtrm
        // driver with:
        //   1. panelized_ple(A) — wide Schur updates via gemm_axpy_into_view,
        //      which routes to fp_small_try_gemm_classical (post-40195c09)
        //      for small primes and to fp_medium for GF(65521).
        //   2. trsm_lower(L, I_n) — forward solve.
        //   3. trsm_upper(E, Y)   — back solve.
        //   4. column-permute by Pᵀ.
        //
        // The result is returned directly. The scalar-pivot path is not
        // reached when n ≥ BLOCKED_INVERT_THRESHOLD; returning None from
        // blocked_inv_panelized signals rank-deficiency (same contract as
        // the scalar path's `if rank < n { return None; }` guard).
        if n >= BLOCKED_INVERT_THRESHOLD {
            return blocked_inv_panelized(self);
        }

        let (perm, mut l, mut e, rank) = self.ple();
        if rank < n {
            return None;
        }
        // Full rank ⇒ L is n×n unit lower-triangular, E is n×n with
        // pivots on the leading diagonal (i.e. upper-triangular).
        // Invert in place via the §2.3 algorithm 2.3 primitives.
        trtri_lower(l.submat_mut(.., ..));
        trtri_upper(e.submat_mut(.., ..));

        // Compose M = E⁻¹ · L⁻¹ in place into L⁻¹'s storage via the
        // in-place upper-times-unit-lower product. `trtrm(L_mut, U)`
        // computes `A = U · L` and writes `A` over `L`'s view; the
        // `L`-operand is treated as unit-lower with implicit diagonal,
        // matching `L⁻¹`'s structure (the explicit `1`s on the diagonal
        // from `trtri_lower` are not read by `trtrm`).
        //
        // Algorithm-choice rationale: see method-level docs and
        // `dev/bench_results/2026-05-07-d1a5fea8-invert-inplace.md`.
        // Allocation budget: NO `n × n` scratch (the prior driver
        // allocated `temp` of shape `n × n` for the `gemm` target);
        // `trtrm`'s recursion adds the documented per-level
        // `(m-h) × h` scratch for the `U22 · L21` chain.
        trtrm(l.submat_mut(.., ..), e.submat(.., ..));

        // Apply Pᵀ on the right: (M · Pᵀ)[i, j] = M[i, perm[j]].
        // Materialise the column-permuted output. We cannot do this in
        // place (column-permutation in row-major storage would alias
        // sources and sinks within a row); a fresh allocation is the
        // standard library-style cost.
        let zero = self.get(0, 0).zero_like();
        let mut out = FieldMatrix::new(n, n, zero);
        let perm_idx = perm.indices();
        for i in 0..n {
            for (j, &src_col) in perm_idx.iter().enumerate() {
                out.set(i, j, l.get(i, src_col));
            }
        }
        Some(out)
    }

    /// Solves `A · x = b` for a single column `b`.
    ///
    /// Contract:
    ///
    /// * Returns `Some(x)` with `A · x == b` iff `A` is square and
    ///   non-singular (`rank(A) == n`).
    /// * Returns `None` iff `A` is square and rank-deficient
    ///   (`rank(A) < n`). This is the entire singular-system signal —
    ///   inconsistent rank-deficient systems and underdetermined
    ///   compatible systems are not distinguished, both report `None`.
    /// * Panics if `A` is non-square. Square inputs are a precondition
    ///   not enforceable in Rust's type system, so the violation is
    ///   surfaced at the call site rather than swallowed.
    ///
    /// Callers needing least-squares or pseudo-inverse semantics over
    /// rank-deficient compatible systems should compose
    /// [`row_echelon`](crate::field::ple::FieldMatrix::row_echelon)
    /// and
    /// [`nullspace`](crate::field::ple::FieldMatrix::nullspace) from
    /// the PLE module directly; the Moore–Penrose pseudo-inverse is
    /// out of scope here (see "Non-square inputs" in the module docs).
    ///
    /// Implements Dumas–Pernet §2.3 Table 2 by composing
    /// [`solve_batch`](Self::solve_batch) with a `n × 1` right-hand side.
    ///
    /// # Arguments
    ///
    /// * `b` — Right-hand side; `b.len()` must equal `self.rows()`.
    ///
    /// # Returns
    ///
    /// `Some(x)` with `self · x == b` if `self` is invertible, `None`
    /// otherwise.
    ///
    /// # Panics
    ///
    /// * Panics if `self` is not square.
    /// * Panics if `b.len() != self.rows()`.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations — dominated by the PLE.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::field::vec::FieldVec;
    /// use gf2_core::gfp::Fp;
    ///
    /// // A = [[2, 3], [1, 4]] over GF(7), b = [4, 1].
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 0, Fp::<7>::new(2));
    /// a.set(0, 1, Fp::<7>::new(3));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(4));
    /// let b = FieldVec::from(vec![Fp::<7>::new(4), Fp::<7>::new(1)]);
    /// let x = a.solve(&b).expect("invertible");
    /// // Cross-check: A · x == b.
    /// let bb = a.matvec(&x);
    /// assert_eq!(bb, b);
    /// ```
    pub fn solve(&self, b: &FieldVec<F>) -> Option<FieldVec<F>> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::solve: input must be square (got {}×{})",
            m, n
        );
        assert_eq!(
            b.len(),
            m,
            "FieldMatrix::solve: b.len() ({}) != rows ({})",
            b.len(),
            m
        );
        if n == 0 {
            // Trivial empty system: the unique solution is the empty vector.
            return Some(FieldVec::new());
        }
        // Wrap b as an n × 1 matrix and reuse the batch path.
        let zero = b.get(0).zero_like();
        let mut b_mat = FieldMatrix::new(n, 1, zero);
        for i in 0..n {
            b_mat.set(i, 0, b.get(i).clone());
        }
        let x_mat = self.solve_batch(&b_mat)?;
        let mut x = FieldVec::zeros_from(n, b.get(0));
        for i in 0..n {
            x.set(i, x_mat.get(i, 0));
        }
        Some(x)
    }

    /// Solves `A · X = B` for a right-hand-side matrix `B`.
    ///
    /// Equivalent to applying [`solve`](Self::solve) to each column of
    /// `B`; the matrix path benefits from a single PLE + two `trsm`
    /// calls instead of `k` independent solves.
    ///
    /// Contract (mirrors [`solve`](Self::solve)):
    ///
    /// * Returns `Some(X)` with `A · X == B` iff `A` is square and
    ///   non-singular.
    /// * Returns `None` iff `A` is square and rank-deficient. As with
    ///   [`solve`](Self::solve), this is the entire singular-system
    ///   signal; rectangular / pseudo-inverse semantics are out of
    ///   scope (see the module docs).
    /// * Panics if `A` is non-square or if `B.rows() != A.rows()`.
    ///
    /// Implements Dumas–Pernet §2.3 Table 2:
    ///
    /// 1. `(P, L, E, r) = self.ple()`.
    /// 2. If `r < n`, return `None`.
    /// 3. `Y = Pᵀ · B` (row-permute `B` by `perm`).
    /// 4. `L · Y' = Y` solved in place via
    ///    [`trsm_lower`](crate::field::triangular::trsm_lower).
    /// 5. `E · X = Y'` solved in place via
    ///    [`trsm_upper`](crate::field::triangular::trsm_upper).
    ///
    /// # Arguments
    ///
    /// * `b` — Right-hand side `n × k` matrix.
    ///
    /// # Returns
    ///
    /// `Some(X)` with `self · X == b` if `self` is invertible, `None`
    /// otherwise.
    ///
    /// # Panics
    ///
    /// * Panics if `self` is not square.
    /// * Panics if `b.rows() != self.rows()`.
    ///
    /// # Complexity
    ///
    /// `O(n³ + n² · k)` field operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::{gemm, FieldMatrix};
    /// use gf2_core::gfp::Fp;
    ///
    /// // A = [[2, 3], [1, 4]] over GF(7); B = identity ⇒ X = A⁻¹.
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 0, Fp::<7>::new(2));
    /// a.set(0, 1, Fp::<7>::new(3));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(4));
    /// let b = FieldMatrix::<Fp<7>>::identity(2);
    /// let x = a.solve_batch(&b).expect("invertible");
    /// let prod = gemm(&a, &x);
    /// assert_eq!(prod, b);
    /// ```
    pub fn solve_batch(&self, b: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::solve_batch: input must be square (got {}×{})",
            m, n
        );
        assert_eq!(
            b.rows(),
            m,
            "FieldMatrix::solve_batch: b.rows() ({}) != self.rows() ({})",
            b.rows(),
            m
        );
        let k = b.cols();
        if n == 0 {
            return Some(FieldMatrix::new_empty_like(0, k, b));
        }
        if k == 0 {
            return Some(FieldMatrix::new_empty_like(n, 0, b));
        }
        let (perm, l, e, rank) = self.ple();
        if rank < n {
            return None;
        }
        // Build Y = Pᵀ · B by row-permuting b through perm.indices().
        // (Pᵀ · B)[i, *] = B[k, *] where perm[k] = i, i.e.
        // k = perm⁻¹(i). Equivalently, the inverse permutation applied
        // to b. Permutation::apply(&b) computes (P · B)[i] = B[perm[i]],
        // so we use perm.inverse().apply(b) for Pᵀ · B.
        let mut y = perm.inverse().apply(b);

        // Solve L · Y' = Y in place. L is n×n unit lower-triangular at
        // full rank. Result overwrites y.
        // Dispatch to the blocked variant when the field exposes the AVX2
        // whole-GEMM fast path and the matrix is large enough to benefit
        // (the first blocking update lands at panel k=1, GEMM shape
        // bs × bs × k, which hits the threshold at bs = 64 even for k=1).
        if F::has_simd_gemm_classical() && n >= TRSM_BLOCKED_PANEL_SIZE {
            trsm_lower_blocked(
                l.submat(.., ..),
                y.submat_mut(.., ..),
                TRSM_BLOCKED_PANEL_SIZE,
            );
        } else {
            trsm_lower(l.submat(.., ..), y.submat_mut(.., ..));
        }

        // Solve E · X = Y' in place. E is n×n upper-triangular at full
        // rank (pivots on the leading diagonal because rank == n).
        if F::has_simd_gemm_classical() && n >= TRSM_BLOCKED_PANEL_SIZE {
            trsm_upper_blocked(
                e.submat(.., ..),
                y.submat_mut(.., ..),
                TRSM_BLOCKED_PANEL_SIZE,
            );
        } else {
            trsm_upper(e.submat(.., ..), y.submat_mut(.., ..));
        }

        Some(y)
    }

    /// Returns the determinant `det(self)`.
    ///
    /// Implements Dumas–Pernet §2.3 Table 2: from the PLE decomposition
    /// `P · L · E = self`, `det(self) = sign(P) · det(L) · det(E)`.
    /// `L` is unit lower-trapezoidal so `det(L) = 1` (when full rank).
    /// `E`'s pivot values lie on the leading diagonal of its `r × r`
    /// square block, and at full rank `det(E) = ∏ E[i, i]`.
    ///
    /// At rank `< n` the determinant is the field zero.
    ///
    /// # Sign of the permutation
    ///
    /// `sign(P) ∈ {+1, −1}` is the parity of the number of
    /// transpositions in `P`. Over a field of characteristic 2 the
    /// distinction collapses (`+1 == −1`), but for `Fp<7>`,
    /// `Fp<MERSENNE_31>`, and any other odd-characteristic field the
    /// sign matters: a determinant computed without the sign can flip
    /// across permutations and break test-vector equality.
    ///
    /// # Arguments
    ///
    /// * `self` — Square `n × n` input. Not modified.
    ///
    /// # Returns
    ///
    /// The field element `det(self)`. Equal to the field zero iff
    /// `rank(self) < n`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations (one PLE).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// // det([[2, 3], [1, 4]]) = 2·4 - 3·1 = 5 over GF(7).
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 0, Fp::<7>::new(2));
    /// a.set(0, 1, Fp::<7>::new(3));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(4));
    /// assert_eq!(a.det(), Fp::<7>::new(5));
    /// ```
    pub fn det(&self) -> F {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::det: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            // Convention: det of the 0×0 matrix is 1 (empty product). We
            // need a witness for `F::one()`. ConstField has it for free;
            // for a runtime-context field on a 0×0 input there is no
            // existing element to clone, so fall back to F::zero_hint
            // and synthesise one_like; if that fails (Gf2mElement on
            // 0×0 input has no field witness), panic with a clear message.
            if let Some(z) = F::zero_hint() {
                return z.one_like();
            }
            panic!(
                "FieldMatrix::det: cannot synthesise det = 1 for an \
                 empty (0×0) matrix over a runtime-context field; use \
                 F: ConstField"
            );
        }
        let (perm, _l, e, rank) = self.ple();
        let zero = self.get(0, 0).zero_like();
        if rank < n {
            return zero;
        }
        // Product of the leading diagonal of E (the pivot values).
        let one = zero.one_like();
        let mut det = one.clone();
        for i in 0..n {
            det = det * e.get(i, i);
        }
        // Multiply by sign(P): parity of the number of inversions in
        // perm.indices(). Over characteristic 2 this is a no-op
        // (-1 == 1), but the explicit multiply keeps the code generic.
        if permutation_sign_is_negative(perm.indices()) {
            det = -det;
        }
        det
    }
}

// ─── Free-function aliases (Armadillo-style ergonomics) ──────────────────────

/// Free-function alias for [`FieldMatrix::inv`].
///
/// # Examples
///
/// ```
/// use gf2_core::field::inverse::inv;
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let id = FieldMatrix::<Fp<7>>::identity(3);
/// assert_eq!(inv(&id).unwrap(), id);
/// ```
pub fn inv<F: FiniteField>(a: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
    a.inv()
}

/// Free-function alias for [`FieldMatrix::solve`].
///
/// # Examples
///
/// ```
/// use gf2_core::field::inverse::solve;
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::vec::FieldVec;
/// use gf2_core::gfp::Fp;
///
/// let id = FieldMatrix::<Fp<7>>::identity(2);
/// let b = FieldVec::from(vec![Fp::<7>::new(3), Fp::<7>::new(5)]);
/// assert_eq!(solve(&id, &b).unwrap(), b);
/// ```
pub fn solve<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldVec<F>) -> Option<FieldVec<F>> {
    a.solve(b)
}

/// Free-function alias for [`FieldMatrix::det`].
///
/// # Examples
///
/// ```
/// use gf2_core::field::inverse::det;
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let id = FieldMatrix::<Fp<7>>::identity(4);
/// assert_eq!(det(&id), Fp::<7>::new(1));
/// ```
pub fn det<F: FiniteField>(a: &FieldMatrix<F>) -> F {
    a.det()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns `true` iff the permutation has odd parity (i.e. `sign = −1`).
///
/// Counts inversions in `O(n²)`; for the small `n` typical of test
/// matrices this is fine, and the call is amortised against the
/// `O(n³)` PLE that produced the permutation. Equivalent to counting
/// transpositions in any decomposition (parity is well-defined modulo 2).
fn permutation_sign_is_negative(perm: &[usize]) -> bool {
    let n = perm.len();
    let mut inversions: usize = 0;
    for i in 0..n {
        for j in (i + 1)..n {
            if perm[i] > perm[j] {
                inversions += 1;
            }
        }
    }
    inversions % 2 == 1
}

// ─── Blocked-invert driver (issue 8df0c501, design feb15da9) ─────────────────

/// Blocked GF(p) matrix inversion via panelized PLE (Higham §14.1).
///
/// Implements the four-step algorithm from design doc `feb15da9`:
///
/// 1. `(perm, L, E, rank) = A.ple()` — panelized PLE decomposition
///    (the `ple()` method already dispatches through the panelized kernel
///    introduced in issue `6823c8a0`; no separate function is needed).
/// 2. If `rank < n`, return `None` (rank-deficient).
/// 3. Build the `n × n` identity `I`.
/// 4. `Y = L⁻¹ · I` via `trsm_lower(L, I)` — forward solve.
/// 5. `X = E⁻¹ · Y` via `trsm_upper(E, Y)` — back solve.
/// 6. Apply `Pᵀ` on the right: `out[i, j] = X[i, perm[j]]`.
///
/// The `trsm_lower` and `trsm_upper` calls are block-recursive and
/// dispatch to `gemm_axpy_into_view`, which (after issue `40195c09`)
/// routes to `fp_small_try_gemm_classical` for small primes and to the
/// pre-packed u16 medium-prime kernel for GF(65521).
///
/// # Returns
///
/// `Some(A⁻¹)` if `self` is full-rank, `None` if rank-deficient.
fn blocked_inv_panelized<F: FiniteField>(a: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
    let n = a.rows();
    debug_assert_eq!(n, a.cols(), "blocked_inv_panelized: non-square input");

    // Step 1: panelized PLE decomposition. `a.ple()` already dispatches
    // through the panelized kernel (issue 6823c8a0) for eligible fields.
    let (perm, l, e, rank) = a.ple();

    // Step 2: rank-deficiency check.
    if rank < n {
        return None;
    }

    // Step 3: build n×n identity. We need a zero and one witness — safe
    // because n >= BLOCKED_INVERT_THRESHOLD >= 1, so `a.get(0, 0)` exists.
    let zero = a.get(0, 0).zero_like();
    let one = zero.one_like();
    let mut y = FieldMatrix::new(n, n, zero.clone());
    for i in 0..n {
        y.set(i, i, one.clone());
    }

    // Step 4: forward solve L · Y = I in place.
    // L is unit lower-triangular at full rank.
    trsm_lower(l.submat(.., ..), y.submat_mut(.., ..));

    // Step 5: back solve E · X = Y in place.
    // E is upper-triangular with nonzero diagonal (full rank).
    trsm_upper(e.submat(.., ..), y.submat_mut(.., ..));
    // Y now holds E⁻¹ · L⁻¹ · I = (LE)⁻¹.

    // Step 6: apply Pᵀ on the right: A⁻¹[i, j] = Y[i, perm[j]].
    let mut out = FieldMatrix::new(n, n, zero);
    let perm_idx = perm.indices();
    for i in 0..n {
        for (j, &src_col) in perm_idx.iter().enumerate() {
            out.set(i, j, y.get(i, src_col));
        }
    }
    Some(out)
}

// Convenience constructor for empty result matrices that may need to
// source a zero witness from the right-hand side `b` rather than from
// `self`. Lives here as a private helper rather than on `FieldMatrix`
// because it is only ever called in the empty-shape edge cases of
// [`FieldMatrix::solve_batch`].
impl<F: FiniteField> FieldMatrix<F> {
    fn new_empty_like(rows: usize, cols: usize, template: &FieldMatrix<F>) -> FieldMatrix<F> {
        if rows == 0 || cols == 0 {
            // Use a template witness if available, else zero_hint.
            let zero = if !template.is_empty() {
                template.get(0, 0).zero_like()
            } else if let Some(z) = F::zero_hint() {
                z
            } else {
                // Pathological: B is empty AND the field has no static
                // zero. Fall back to an unreachable-style panic; in
                // practice no caller can hit this because b.rows() ==
                // self.rows() == n >= 0 and at least one of the four
                // dimensions is > 0 in any non-trivial call.
                panic!(
                    "solve_batch: cannot synthesise empty-result zero \
                     witness for runtime-context field with empty inputs"
                );
            };
            return FieldMatrix::new(rows, cols, zero);
        }
        FieldMatrix::new(rows, cols, template.get(0, 0).zero_like())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::{fieldmatrix_new_count, gemm, reset_fieldmatrix_new_count};
    use crate::field::test_random_matrix::{
        random_fp, random_fp_invertible, random_fp_rank_deficient, random_gf2m_wide_1,
        random_gf2m_wide_1_invertible,
    };
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use proptest::prelude::*;
    use serial_test::serial;

    const MERSENNE_31: u64 = 2_147_483_647;

    /// AES-irreducible Gf2mWide<8>.
    struct InvGf2m8Cfg;
    impl Gf2mWideConfig<1> for InvGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "InvGf2m8Cfg";
    }
    type Gf2m8 = Gf2mWide<1, InvGf2m8Cfg>;

    /// Gf2mWide<16>: same Conway polynomial as the PLE test.
    struct InvGf2m16Cfg;
    impl Gf2mWideConfig<1> for InvGf2m16Cfg {
        const M: usize = 16;
        const MODULUS: [u64; 1] = [0x002D];
        const NAME: &'static str = "InvGf2m16Cfg";
    }
    type Gf2m16 = Gf2mWide<1, InvGf2m16Cfg>;

    // Convenience aliases that monomorphise the shared generic helpers
    // in `field::test_random_matrix` to this module's configs. Keeping
    // the call sites short below.
    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        random_gf2m_wide_1::<InvGf2m8Cfg>(rows, cols, seed)
    }
    fn random_gf2m8_invertible(n: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        random_gf2m_wide_1_invertible::<InvGf2m8Cfg>(n, seed)
    }
    fn random_gf2m16_invertible(n: usize, seed: u64) -> FieldMatrix<Gf2m16> {
        random_gf2m_wide_1_invertible::<InvGf2m16Cfg>(n, seed)
    }

    // ── Hard SC#1 — A · A⁻¹ == I across five fields ───────────────────────────

    fn check_inv_round_trip<F: FiniteField>(a: &FieldMatrix<F>) {
        let n = a.rows();
        let inverse = a.inv().expect("input must be invertible");
        let prod = gemm(a, &inverse);
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        for i in 0..n {
            for j in 0..n {
                let expected = if i == j { one.clone() } else { zero.clone() };
                assert_eq!(prod.get(i, j), expected, "A·A⁻¹[{}, {}] != I", i, j);
            }
        }
        // Also check the symmetric direction A⁻¹ · A = I.
        let prod2 = gemm(&inverse, a);
        for i in 0..n {
            for j in 0..n {
                let expected = if i == j { one.clone() } else { zero.clone() };
                assert_eq!(prod2.get(i, j), expected, "A⁻¹·A[{}, {}] != I", i, j);
            }
        }
    }

    #[test]
    fn test_inv_random_fp7() {
        for seed in 0..5u64 {
            let a = random_fp_invertible::<7>(4, seed);
            check_inv_round_trip(&a);
        }
    }

    #[test]
    fn test_inv_random_fp65521() {
        for seed in 0..5u64 {
            let a = random_fp_invertible::<65521>(5, seed);
            check_inv_round_trip(&a);
        }
    }

    #[test]
    fn test_inv_random_mersenne31() {
        for seed in 0..5u64 {
            let a = random_fp_invertible::<MERSENNE_31>(6, seed);
            check_inv_round_trip(&a);
        }
    }

    #[test]
    fn test_inv_random_gf2m8() {
        for seed in 0..5u64 {
            let a = random_gf2m8_invertible(5, seed);
            check_inv_round_trip(&a);
        }
    }

    #[test]
    fn test_inv_random_gf2m16() {
        for seed in 0..3u64 {
            let a = random_gf2m16_invertible(4, seed);
            check_inv_round_trip(&a);
        }
    }

    // ── Hard SC#2 — None on singular inputs, no panic ────────────────────────

    #[test]
    fn test_inv_singular_zero_matrix() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        assert!(a.inv().is_none());
    }

    #[test]
    fn test_inv_singular_duplicated_row() {
        let mut a = random_fp::<MERSENNE_31>(4, 4, 0xDEAD);
        for j in 0..4 {
            let v = a.get(0, j);
            a.set(2, j, v);
        }
        // a may already be singular; if not, force it.
        assert!(a.rank() < 4);
        assert!(a.inv().is_none());
    }

    #[test]
    fn test_inv_singular_zero_column() {
        let mut a = random_fp::<MERSENNE_31>(4, 4, 0xBEEF);
        for i in 0..4 {
            a.set(i, 2, Fp::<MERSENNE_31>::new(0));
        }
        assert!(a.rank() < 4);
        assert!(a.inv().is_none());
    }

    #[test]
    fn test_inv_singular_outer_product() {
        // 4×4 rank-1 matrix.
        let f1 = random_fp::<MERSENNE_31>(4, 1, 0x11);
        let f2 = random_fp::<MERSENNE_31>(1, 4, 0x22);
        let a = gemm(&f1, &f2);
        if a.rank() < 4 {
            assert!(a.inv().is_none());
        }
    }

    // ── Hard SC#3 — solve correctness ────────────────────────────────────────

    fn check_solve<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldVec<F>) {
        let x = a.solve(b).expect("invertible");
        let bb = a.matvec(&x);
        assert_eq!(bb, *b, "A · x != b");
    }

    #[test]
    fn test_solve_random_fp7() {
        for seed in 0..5u64 {
            let a = random_fp_invertible::<7>(4, seed);
            let b: FieldVec<Fp<7>> = (0..4)
                .map(|i| Fp::<7>::new((seed + i as u64) % 7))
                .collect();
            check_solve(&a, &b);
        }
    }

    #[test]
    fn test_solve_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp_invertible::<MERSENNE_31>(6, seed);
            let b: FieldVec<Fp<MERSENNE_31>> = (0..6)
                .map(|i| Fp::<MERSENNE_31>::new((seed + i as u64) * 3 + 1))
                .collect();
            check_solve(&a, &b);
        }
    }

    #[test]
    fn test_solve_random_gf2m8() {
        for seed in 0..3u64 {
            let a = random_gf2m8_invertible(5, seed);
            let b: FieldVec<Gf2m8> = (0..5)
                .map(|i| Gf2m8::new([(seed + i as u64) & 0xFF]))
                .collect();
            check_solve(&a, &b);
        }
    }

    #[test]
    fn test_solve_singular_returns_none() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        let b: FieldVec<Fp<MERSENNE_31>> =
            (0..4).map(|i| Fp::<MERSENNE_31>::new(i as u64)).collect();
        assert!(a.solve(&b).is_none());
    }

    /// Build a deterministic rank-2 4x4 `Fp<MERSENNE_31>` matrix used
    /// by the rank-deficient non-zero correctness tests below. Rows
    /// 0 and 2 hold `[1, 2, 3, 4]`; rows 1 and 3 hold `[5, 6, 7, 8]`.
    /// Row duplication is the simplest non-zero rank drop and exercises
    /// the rank-detection path inside the PLE driver for a structurally
    /// singular but non-trivial input.
    fn rank_deficient_nonzero_4x4_fp_m31() -> FieldMatrix<Fp<MERSENNE_31>> {
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        for j in 0..4usize {
            a.set(0, j, Fp::<MERSENNE_31>::new(j as u64 + 1));
            a.set(2, j, Fp::<MERSENNE_31>::new(j as u64 + 1));
            a.set(1, j, Fp::<MERSENNE_31>::new(j as u64 + 5));
            a.set(3, j, Fp::<MERSENNE_31>::new(j as u64 + 5));
        }
        debug_assert_eq!(a.rank(), 2, "rank_deficient_nonzero_4x4_fp_m31 invariant");
        a
    }

    /// `solve` returns `None` on a rank-deficient non-zero input.
    #[test]
    fn test_solve_rank_deficient_nonzero_returns_none() {
        let a = rank_deficient_nonzero_4x4_fp_m31();
        assert_eq!(a.rank(), 2, "setup: expected rank-2 matrix");
        let b: FieldVec<Fp<MERSENNE_31>> = (0..4)
            .map(|i| Fp::<MERSENNE_31>::new(i as u64 + 1))
            .collect();
        assert!(a.solve(&b).is_none());
    }

    /// `solve_batch` returns `None` on a rank-deficient non-zero input.
    #[test]
    fn test_solve_batch_rank_deficient_nonzero_returns_none() {
        let a = rank_deficient_nonzero_4x4_fp_m31();
        assert_eq!(a.rank(), 2, "setup: expected rank-2 matrix");
        let b = random_fp::<MERSENNE_31>(4, 3, 0xABCDEF);
        assert!(a.solve_batch(&b).is_none());
    }

    // ── Hard SC#4 — det correctness ──────────────────────────────────────────

    /// Brute-force determinant via cofactor expansion. Used as oracle
    /// for `n ≤ 4`. Quadratic in `n!` so only invoked for tiny `n`.
    fn det_oracle<F: FiniteField>(a: &FieldMatrix<F>) -> F {
        let n = a.rows();
        assert_eq!(n, a.cols());
        if n == 0 {
            // Convention: empty product = 1.
            return F::zero_hint().unwrap().one_like();
        }
        if n == 1 {
            return a.get(0, 0);
        }
        let zero = a.get(0, 0).zero_like();
        let mut acc = zero.clone();
        // Expand along row 0.
        for j in 0..n {
            let aij = a.get(0, j);
            if aij == zero {
                continue;
            }
            // Build (n−1)×(n−1) minor.
            let mut minor = FieldMatrix::new(n - 1, n - 1, zero.clone());
            for r in 1..n {
                let mut cc = 0;
                for c in 0..n {
                    if c == j {
                        continue;
                    }
                    minor.set(r - 1, cc, a.get(r, c));
                    cc += 1;
                }
            }
            let m_det = det_oracle(&minor);
            let term = aij * m_det;
            if j % 2 == 0 {
                acc += term;
            } else {
                acc = acc - term;
            }
        }
        acc
    }

    #[test]
    fn test_det_random_fp7_n3() {
        for seed in 0..6u64 {
            let a = random_fp::<7>(3, 3, seed);
            let d = a.det();
            let oracle = det_oracle(&a);
            assert_eq!(d, oracle, "det mismatch on seed {}", seed);
        }
    }

    #[test]
    fn test_det_random_fp65521_n4() {
        for seed in 0..4u64 {
            let a = random_fp::<65521>(4, 4, seed);
            let d = a.det();
            let oracle = det_oracle(&a);
            assert_eq!(d, oracle);
        }
    }

    #[test]
    fn test_det_random_gf2m8_n3() {
        for seed in 0..3u64 {
            let a = random_gf2m8(3, 3, seed);
            let d = a.det();
            let oracle = det_oracle(&a);
            assert_eq!(d, oracle);
        }
    }

    #[test]
    fn test_det_zero_iff_singular() {
        // Singular => det == 0.
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        assert_eq!(a.det(), Fp::<MERSENNE_31>::new(0));

        // Random invertible => det != 0.
        let b = random_fp_invertible::<MERSENNE_31>(5, 0xC0FFEE);
        assert_ne!(b.det(), Fp::<MERSENNE_31>::new(0));
    }

    /// `det` returns the field zero for a rank-deficient non-zero matrix.
    ///
    /// The zero-matrix test above is the simplest singular case; this
    /// test covers the structurally distinct "rank < n but non-zero
    /// entries" path through the PLE driver's pivot detection.
    #[test]
    fn test_det_zero_for_rank_deficient_nonzero() {
        let a = rank_deficient_nonzero_4x4_fp_m31();
        assert_eq!(a.rank(), 2, "setup: expected rank-2 matrix");
        assert_eq!(a.det(), Fp::<MERSENNE_31>::new(0));
    }

    /// `inv` returns `None` for a rank-deficient non-zero matrix.
    ///
    /// Complements `test_inv_singular_zero_matrix` for the case where
    /// the matrix has non-zero entries but is structurally singular.
    #[test]
    fn test_inv_rank_deficient_nonzero_returns_none() {
        let a = rank_deficient_nonzero_4x4_fp_m31();
        assert_eq!(a.rank(), 2, "setup: expected rank-2 matrix");
        assert!(a.inv().is_none());
    }

    #[test]
    fn test_det_identity() {
        let a = FieldMatrix::<Fp<7>>::identity(5);
        assert_eq!(a.det(), Fp::<7>::new(1));
    }

    #[test]
    fn test_det_diagonal() {
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        a.set(0, 0, Fp::<MERSENNE_31>::new(2));
        a.set(1, 1, Fp::<MERSENNE_31>::new(3));
        a.set(2, 2, Fp::<MERSENNE_31>::new(5));
        a.set(3, 3, Fp::<MERSENNE_31>::new(7));
        // Product 2·3·5·7 = 210.
        assert_eq!(a.det(), Fp::<MERSENNE_31>::new(210));
    }

    #[test]
    fn test_det_permutation_matrix_signed() {
        // 3-cycle permutation matrix:
        //   row 0 ← col 2, row 1 ← col 0, row 2 ← col 1
        // [[0, 0, 1], [1, 0, 0], [0, 1, 0]]
        // 3-cycle = product of 2 transpositions ⇒ even ⇒ sign = +1.
        // All pivots are 1 ⇒ det = 1.
        let mut a = FieldMatrix::<Fp<7>>::zeros(3, 3);
        a.set(0, 2, Fp::<7>::new(1));
        a.set(1, 0, Fp::<7>::new(1));
        a.set(2, 1, Fp::<7>::new(1));
        let d = a.det();
        let oracle = det_oracle(&a);
        assert_eq!(d, oracle);
        assert_eq!(d, Fp::<7>::new(1));
    }

    #[test]
    fn test_det_single_swap_negative_sign() {
        // [[0, 1], [1, 0]]: one swap ⇒ sign = −1, pivots = 1 each ⇒
        // det = −1 ≡ 6 (mod 7).
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        a.set(0, 1, Fp::<7>::new(1));
        a.set(1, 0, Fp::<7>::new(1));
        assert_eq!(a.det(), Fp::<7>::new(6));
    }

    // ── Hard SC#5 — solve_batch matches per-column solve ────────────────────

    #[test]
    fn test_solve_batch_matches_per_column() {
        let n = 5;
        let k = 3;
        let a = random_fp_invertible::<MERSENNE_31>(n, 0xABCDE);
        let b = random_fp::<MERSENNE_31>(n, k, 0x12345);
        let x_batch = a.solve_batch(&b).expect("invertible");
        for col in 0..k {
            let mut bv = FieldVec::zeros_from(n, &Fp::<MERSENNE_31>::new(0));
            for i in 0..n {
                bv.set(i, b.get(i, col));
            }
            let x_col = a.solve(&bv).expect("invertible");
            for i in 0..n {
                assert_eq!(x_batch.get(i, col), *x_col.get(i), "col {}", col);
            }
        }
    }

    #[test]
    fn test_solve_batch_singular_returns_none() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(3, 3);
        let b = FieldMatrix::<Fp<MERSENNE_31>>::zeros(3, 2);
        assert!(a.solve_batch(&b).is_none());
    }

    // ── Hard SC#6 — solve / inv composition ─────────────────────────────────

    #[test]
    fn test_solve_batch_with_identity_yields_inverse() {
        let a = random_fp_invertible::<MERSENNE_31>(4, 0x9999);
        let id = FieldMatrix::<Fp<MERSENNE_31>>::identity(4);
        let x = a.solve_batch(&id).expect("invertible");
        let a_inv = a.inv().expect("invertible");
        assert_eq!(x, a_inv);
    }

    // ── Edge cases (Hard SC list) ───────────────────────────────────────────

    #[test]
    fn test_inv_n_eq_0() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let inv = a.inv().expect("0×0 is invertible");
        assert_eq!(inv.shape(), (0, 0));
    }

    #[test]
    fn test_inv_n_eq_1_invertible() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(1, 1);
        a.set(0, 0, Fp::<7>::new(3));
        let inv = a.inv().expect("3 ≠ 0");
        // 3 · 5 = 15 ≡ 1 (mod 7).
        assert_eq!(inv.get(0, 0), Fp::<7>::new(5));
    }

    #[test]
    fn test_inv_n_eq_1_singular() {
        let a = FieldMatrix::<Fp<7>>::zeros(1, 1);
        assert!(a.inv().is_none());
    }

    #[test]
    fn test_inv_identity() {
        let a = FieldMatrix::<Fp<7>>::identity(5);
        assert_eq!(a.inv().unwrap(), a);
    }

    #[test]
    fn test_inv_diagonal() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(3, 3);
        a.set(0, 0, Fp::<7>::new(2));
        a.set(1, 1, Fp::<7>::new(3));
        a.set(2, 2, Fp::<7>::new(5));
        let inv = a.inv().expect("non-zero diag");
        let prod = gemm(&a, &inv);
        assert_eq!(prod, FieldMatrix::<Fp<7>>::identity(3));
    }

    #[test]
    fn test_inv_permutation_matrix() {
        // [[0, 1, 0], [0, 0, 1], [1, 0, 0]] over GF(7).
        let mut a = FieldMatrix::<Fp<7>>::zeros(3, 3);
        a.set(0, 1, Fp::<7>::new(1));
        a.set(1, 2, Fp::<7>::new(1));
        a.set(2, 0, Fp::<7>::new(1));
        let inv = a.inv().expect("permutation matrices are invertible");
        let prod = gemm(&a, &inv);
        assert_eq!(prod, FieldMatrix::<Fp<7>>::identity(3));
    }

    #[test]
    fn test_inv_near_singular_single_zero_pivot() {
        // Matrix that pivots on a zero in its leading column but is
        // still invertible after row swaps:
        //    [[0, 1, 2], [3, 4, 5], [6, 0, 1]]
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(3, 3);
        a.set(0, 0, Fp::<MERSENNE_31>::new(0));
        a.set(0, 1, Fp::<MERSENNE_31>::new(1));
        a.set(0, 2, Fp::<MERSENNE_31>::new(2));
        a.set(1, 0, Fp::<MERSENNE_31>::new(3));
        a.set(1, 1, Fp::<MERSENNE_31>::new(4));
        a.set(1, 2, Fp::<MERSENNE_31>::new(5));
        a.set(2, 0, Fp::<MERSENNE_31>::new(6));
        a.set(2, 1, Fp::<MERSENNE_31>::new(0));
        a.set(2, 2, Fp::<MERSENNE_31>::new(1));
        if a.rank() == 3 {
            check_inv_round_trip(&a);
        }
    }

    #[test]
    fn test_solve_n_eq_0() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let b = FieldVec::<Fp<7>>::new();
        let x = a.solve(&b).unwrap();
        assert_eq!(x.len(), 0);
    }

    #[test]
    fn test_solve_n_eq_1() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(1, 1);
        a.set(0, 0, Fp::<7>::new(3));
        let b = FieldVec::from(vec![Fp::<7>::new(2)]);
        // 3·x = 2 mod 7 ⇒ x = 2·3⁻¹ = 2·5 = 10 ≡ 3 (mod 7).
        let x = a.solve(&b).unwrap();
        assert_eq!(*x.get(0), Fp::<7>::new(3));
    }

    #[test]
    fn test_det_n_eq_0() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        // Empty product = 1.
        assert_eq!(a.det(), Fp::<7>::new(1));
    }

    #[test]
    fn test_det_n_eq_1() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(1, 1);
        a.set(0, 0, Fp::<7>::new(4));
        assert_eq!(a.det(), Fp::<7>::new(4));
    }

    // ── Hard SC#7 — Free-function aliases ────────────────────────────────────

    #[test]
    fn test_free_function_aliases_match_methods() {
        let a = random_fp_invertible::<MERSENNE_31>(4, 0xF11F);
        assert_eq!(a.inv(), super::inv(&a));
        assert_eq!(a.det(), super::det(&a));
        let b: FieldVec<Fp<MERSENNE_31>> = (0..4)
            .map(|i| Fp::<MERSENNE_31>::new(i as u64 + 1))
            .collect();
        assert_eq!(a.solve(&b), super::solve(&a, &b));
    }

    // ── d1a5fea8 — Bit-exact equivalence with prior Dumas–Pernet driver ──────
    //
    // Reference implementation of the prior Dumas–Pernet Table 2 driver
    // (PLE + 2 trtri + 1 dense gemm + permutation). The production
    // `inv()` uses the in-place trtrm composition (issue d1a5fea8). The
    // tests below cross-check that the two drivers return bit-exact
    // identical matrices on randomized invertible inputs across all
    // five fields, so any future tuning of the in-place driver can be
    // detected as a divergence (vs only correctness vs identity).

    fn inv_reference_dumas_pernet<F: FiniteField>(a: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
        use crate::field::matrix::gemm_into_view;
        use crate::field::triangular::{trtri_lower, trtri_upper};
        let (m, n) = a.shape();
        assert_eq!(m, n);
        if n == 0 {
            return Some(a.clone());
        }
        let (perm, mut l, mut e, rank) = a.ple();
        if rank < n {
            return None;
        }
        trtri_lower(l.submat_mut(.., ..));
        trtri_upper(e.submat_mut(.., ..));
        let zero = a.get(0, 0).zero_like();
        let mut temp = FieldMatrix::new(n, n, zero.clone());
        gemm_into_view(&e, &l, temp.submat_mut(.., ..));
        let mut out = FieldMatrix::new(n, n, zero);
        let perm_idx = perm.indices();
        for i in 0..n {
            for (j, &src_col) in perm_idx.iter().enumerate() {
                out.set(i, j, temp.get(i, src_col));
            }
        }
        Some(out)
    }

    fn assert_inv_matches_reference<F: FiniteField>(a: &FieldMatrix<F>) {
        let new_inv = a.inv().expect("input must be invertible");
        let ref_inv = inv_reference_dumas_pernet(a).expect("input must be invertible");
        assert_eq!(
            new_inv, ref_inv,
            "in-place inv() differs from Dumas–Pernet reference"
        );
    }

    #[test]
    fn test_inv_matches_reference_fp7() {
        for n in [2usize, 4, 8, 16, 32] {
            for seed in 0..3u64 {
                let a = random_fp_invertible::<7>(n, seed * 13 + n as u64);
                assert_inv_matches_reference(&a);
            }
        }
    }

    #[test]
    fn test_inv_matches_reference_fp251() {
        for n in [2usize, 4, 8, 16, 32] {
            for seed in 0..3u64 {
                let a = random_fp_invertible::<251>(n, seed * 17 + n as u64);
                assert_inv_matches_reference(&a);
            }
        }
    }

    #[test]
    fn test_inv_matches_reference_fp65521() {
        for n in [2usize, 4, 8, 16, 32] {
            for seed in 0..3u64 {
                let a = random_fp_invertible::<65521>(n, seed * 19 + n as u64);
                assert_inv_matches_reference(&a);
            }
        }
    }

    #[test]
    fn test_inv_matches_reference_mersenne31() {
        for n in [2usize, 4, 8, 16, 32, 64] {
            for seed in 0..3u64 {
                let a = random_fp_invertible::<MERSENNE_31>(n, seed * 23 + n as u64);
                assert_inv_matches_reference(&a);
            }
        }
    }

    #[test]
    fn test_inv_matches_reference_gf2m8() {
        for n in [2usize, 4, 8, 16, 32] {
            for seed in 0..3u64 {
                let a = random_gf2m8_invertible(n, seed * 29 + n as u64);
                assert_inv_matches_reference(&a);
            }
        }
    }

    #[test]
    fn test_inv_matches_reference_gf2m16() {
        for n in [2usize, 4, 8, 16, 32] {
            for seed in 0..3u64 {
                let a = random_gf2m16_invertible(n, seed * 31 + n as u64);
                assert_inv_matches_reference(&a);
            }
        }
    }

    // ── Hard SC#8 — Allocation budget ────────────────────────────────────────

    // Pinned allocation counts. Update only when the recursion strategy
    // or the underlying gemm/trsm/trtri kernels change their allocation
    // footprint. Counts come from `FIELDMATRIX_NEW_COUNT`, which bumps
    // on every `FieldMatrix::new` (also covering `FieldMatrix::clone`,
    // `MatView::transpose`, `MatViewMut::to_owned`, etc.).
    //
    // Empirical numbers obtained on the current driver; not derived
    // from a closed-form because the recursion's allocation profile
    // depends on PLE's branching and the gemm kernel's internal
    // B-transpose. See module rustdoc for the breakdown formula.

    #[test]
    #[serial]
    fn test_inv_allocation_budget_n4_fp_m31() {
        let a = random_fp_invertible::<MERSENNE_31>(4, 0xC0DE);
        reset_fieldmatrix_new_count();
        let _ = a.inv();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_INV_N4,
            "inv(4×4) allocs should be exactly {EXPECTED_INV_N4}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    fn test_inv_allocation_budget_n64_fp_m31() {
        let a = random_fp_invertible::<MERSENNE_31>(64, 0xC0DF);
        reset_fieldmatrix_new_count();
        let _ = a.inv();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_INV_N64,
            "inv(64×64) allocs should be exactly {EXPECTED_INV_N64}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    #[ignore = "slow: inv(1024×1024) over Fp<MERSENNE_31>"]
    fn test_inv_allocation_budget_n1024_fp_m31() {
        let a = random_fp_invertible::<MERSENNE_31>(1024, 0xC0E0);
        reset_fieldmatrix_new_count();
        let _ = a.inv();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_INV_N1024,
            "inv(1024×1024) allocs should be exactly {EXPECTED_INV_N1024}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    fn test_solve_allocation_budget_n64_fp_m31() {
        let a = random_fp_invertible::<MERSENNE_31>(64, 0xC0E1);
        let b: FieldVec<Fp<MERSENNE_31>> = (0..64u64).map(Fp::<MERSENNE_31>::new).collect();
        reset_fieldmatrix_new_count();
        let _ = a.solve(&b);
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_SOLVE_N64,
            "solve(64×64) allocs should be exactly {EXPECTED_SOLVE_N64}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    fn test_det_allocation_budget_n64_fp_m31() {
        let a = random_fp_invertible::<MERSENNE_31>(64, 0xC0E2);
        reset_fieldmatrix_new_count();
        let _ = a.det();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_DET_N64,
            "det(64×64) allocs should be exactly {EXPECTED_DET_N64}; got {allocs}"
        );
    }

    // Pinned counts. These values are pinned empirically against the
    // current view-based driver and the upstream PLE driver in
    // `crate::field::ple`. Update only when the recursion strategy or
    // the underlying gemm/trsm/trtri kernels change their allocation
    // footprint. Each count is the exact `FIELDMATRIX_NEW_COUNT`
    // reading observed for the corresponding operation:
    //
    //   inv(n × n) = ple(n × n)            // upstream PLE budget
    //              + (trtri's base-case `inv` scratches and outer
    //                 chain scratches at each peeled level — one
    //                 per peel for L and one for E)
    //              + trtrm(L⁻¹, E⁻¹) recursion budget   // d1a5fea8
    //                                                       // in-place
    //              + 1 (column-permuted output)
    //
    //   solve_batch(n × n, n × k) = ple + 1 (perm.inverse().apply)
    //              + 2 trsm calls × kernel B-transpose tree
    //
    //   det(n × n) = ple                   // L is dropped
    //
    // The empirical numbers are the sum of these per-call costs at
    // the chosen recursion thresholds. Numbers below match the
    // post-c3f8c1cb PLE budget plus the small additions above. The
    // jit:73ec5da3 R1 rework dropped TRI_BASE_THRESHOLD from 32 to 8
    // (selected by Criterion sweep on Mersenne-31; see the evidence
    // doc); the deeper trsm recursion at threshold=8 inflates the
    // allocation counts at small n by ~4–30% versus the threshold=32
    // baseline, in exchange for 1–7% wall-time gains on the target
    // PLE/TRSM cells.
    //
    // d1a5fea8 (in-place compose):
    //   - n=4 dropped 19 → 17: trtrm at base case (no scratch) replaces
    //     the prior gemm path's 3 allocs (`temp` + 2 B-transpose).
    //   - n=64 grew 353 → 386: trtrm's recursive (m-h)×h scratch and
    //     per-level gemm_axpy/gemm B-transpose copies sum to ~36 allocs
    //     above the prior single-gemm path; in exchange the n×n `temp`
    //     allocation is gone and the dense `n³` work is halved.
    //
    // 8df0c501 (blocked-invert via panelized PLE):
    //   - n=4 unchanged: n=4 < BLOCKED_INVERT_THRESHOLD=16, so the scalar
    //     path (trtri + trtrm) still runs; count stays at 17.
    //   - n=64 changed 386 → 294: blocked path uses ple + 1 identity +
    //     2 trsm calls + 1 output. The panelized ple has the same alloc
    //     budget; the two trsm calls (each with n×n RHS) are cheaper than
    //     trtri_lower + trtri_upper + trtrm in terms of intermediate
    //     scratch because trsm on the wide RHS folds the output directly
    //     into the RHS buffer rather than materialising a separate `(m-h)×h`
    //     scratch per level. Measured 2026-05-26 on Fp<MERSENNE_31> n=64.
    const EXPECTED_INV_N4: u64 = 17;
    const EXPECTED_INV_N64: u64 = 294;
    // Pinned 2026-05-08 by user-authorized direct measurement under the
    // d1a5fea8 in-place trtrm driver (slow-tier nextest run, walls 1.89s
    // on the host pinned in `dev/bench_results/2026-05-07-d1a5fea8-
    // invert-inplace.md`). The earlier extrapolation (5645) was 22%
    // below the actual measurement; the test now uses `assert_eq!`
    // against this pinned value.
    //
    // Re-measured 2026-05-27 (8df0c501 R1 rework) under the blocked-invert
    // driver (blocked_inv_panelized path). n=1024 >= BLOCKED_INVERT_THRESHOLD,
    // so the blocked path (ple + identity + 2 trsm + column-permute) now runs
    // instead of the old scalar-pivot + trtri + trtrm driver. The new count
    // (5246) is lower than the old value (6898) because the two trsm calls on
    // the n×n identity RHS fold output directly into the RHS buffer instead of
    // materialising a separate (m-h)×h scratch per recursion level.
    // Measured by slow-tier nextest run on 2026-05-27 (worktree agent-8df0c501).
    const EXPECTED_INV_N1024: u64 = 5246;
    const EXPECTED_SOLVE_N64: u64 = 294;
    const EXPECTED_DET_N64: u64 = 264;

    // ── Property-based tests (proptest) ──────────────────────────────────────
    //
    // Per `CLAUDE.md` testing convention: TDD plus property-based
    // tests for mathematical invariants. The block below sweeps many
    // seeds at small bounded sizes (`n ∈ 1..=6`) so each case stays
    // well under the 5 s per-test wall-clock cap, and the per-block
    // `cases = 32` budget keeps the aggregate suite cost negligible.
    //
    // Invariants checked:
    //   1. `A · A⁻¹ == I`           (inverse round-trip, full-rank `A`).
    //   2. `A · solve(A, b) == b`   (solve round-trip).
    //   3. `det(A · B) == det(A) · det(B)`  (multiplicativity).
    //   4. `det(A) == 0  iff  rank(A) < n`  (singularity criterion).
    //
    // Both Fp<MERSENNE_31> (odd characteristic) and Gf2m8
    // (characteristic 2) are exercised so any sign-related bug
    // affecting only odd characteristics is caught alongside
    // characteristic-2 specific failures.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// `A · A⁻¹ == I` for full-rank `A` over Fp<MERSENNE_31>.
        #[test]
        fn proptest_inv_round_trip_fp_m31(
            n in 1usize..=6,
            seed in any::<u64>(),
        ) {
            let a = random_fp_invertible::<MERSENNE_31>(n, seed);
            let inv = a.inv().expect("full-rank");
            let prod = gemm(&a, &inv);
            let id = FieldMatrix::<Fp<MERSENNE_31>>::identity(n);
            prop_assert_eq!(prod, id);
        }

        /// `A · A⁻¹ == I` and `A⁻¹ · A == I` for full-rank `A` over Gf2m8.
        #[test]
        fn proptest_inv_round_trip_gf2m8(
            n in 1usize..=6,
            seed in any::<u64>(),
        ) {
            let a = random_gf2m8_invertible(n, seed);
            let inv = a.inv().expect("full-rank");
            let prod = gemm(&a, &inv);
            let prod2 = gemm(&inv, &a);
            // Compare to the identity element-wise. Gf2m8 is `Copy`,
            // so the zero/one witnesses are cheap to use directly.
            let zero = Gf2m8::new([0]);
            let one = Gf2m8::new([1]);
            for i in 0..n {
                for j in 0..n {
                    let expected = if i == j { one } else { zero };
                    prop_assert_eq!(prod.get(i, j), expected);
                    prop_assert_eq!(prod2.get(i, j), expected);
                }
            }
        }

        /// `A · solve(A, b) == b` for full-rank `A` and arbitrary `b`.
        #[test]
        fn proptest_solve_round_trip_fp_m31(
            n in 1usize..=6,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let a = random_fp_invertible::<MERSENNE_31>(n, seed_a);
            // Use the shared random_fp_vec helper to obtain an
            // arbitrary right-hand side over the same field.
            let b = crate::field::test_random_matrix::random_fp_vec::<MERSENNE_31>(n, seed_b);
            let x = a.solve(&b).expect("full-rank");
            let bb = a.matvec(&x);
            prop_assert_eq!(bb, b);
        }

        /// `det(A · B) == det(A) · det(B)` for square `A`, `B`.
        ///
        /// `A` and `B` are arbitrary (possibly singular); the
        /// identity holds in either case because both sides are zero
        /// when either factor is singular.
        #[test]
        fn proptest_det_multiplicative_fp_m31(
            n in 1usize..=5,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let a = random_fp::<MERSENNE_31>(n, n, seed_a);
            let b = random_fp::<MERSENNE_31>(n, n, seed_b);
            let ab = gemm(&a, &b);
            let lhs = ab.det();
            let rhs = a.det() * b.det();
            prop_assert_eq!(lhs, rhs);
        }

        /// `det(A) == 0  iff  rank(A) < n` for square `A`.
        #[test]
        fn proptest_det_zero_iff_singular_fp_m31(
            n in 1usize..=5,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let d = a.det();
            let r = a.rank();
            let zero = Fp::<MERSENNE_31>::new(0);
            if r < n {
                prop_assert_eq!(d, zero, "rank {} < n {} but det != 0", r, n);
            } else {
                prop_assert_ne!(d, zero, "rank == n but det == 0");
            }
        }

        /// `det(A) == 0  iff  rank(A) < n` for square `A` over Gf2m8.
        ///
        /// Independent of `proptest_det_zero_iff_singular_fp_m31` to
        /// catch any characteristic-2-specific sign-handling bug.
        #[test]
        fn proptest_det_zero_iff_singular_gf2m8(
            n in 1usize..=5,
            seed in any::<u64>(),
        ) {
            let a = random_gf2m8(n, n, seed);
            let d = a.det();
            let r = a.rank();
            let zero = a.get(0, 0).zero_like();
            if r < n {
                prop_assert_eq!(d, zero, "rank {} < n {} but det != 0", r, n);
            } else {
                prop_assert_ne!(d, zero, "rank == n but det == 0");
            }
        }
    }

    // ── SC#2 — Blocked-invert boundary sweep proptests (issue 8df0c501) ────────
    //
    // These tests satisfy the literal reading of SC#2 from jit:8df0c501:
    //   "Bit-exact correctness: A · A⁻¹ = I for all proptest cases across
    //    GF(7), GF(31), GF(127), GF(241), GF(251), GF(65521) at boundary lengths."
    //
    // Pattern mirror: ple.rs prop_ple_panelized_boundary_sweep_fp* (lines 3270-3392).
    //
    // Design:
    //   - Each proptest macro runs `cases: 8`, seed in `0u64..1_000_000`.
    //   - Inside the macro body, ALL square n ∈ {1, 15, 16, 17, 63, 64, 65}
    //     are tested exhaustively for each seed.
    //   - For each (seed, n): generate a random invertible n×n matrix via
    //     `random_fp_invertible`, call `.inv()`, assert A · A⁻¹ == I_n.
    //   - n=0 is excluded because a 0×0 matrix is trivially its own inverse
    //     and does not exercise any blocked/scalar dispatch path.
    //
    // Run command:
    //   cargo nextest run -p gf2-core --release --all-features --profile ci \
    //     -E 'test(prop_blocked_inv_product_fp)'

    /// Boundary sizes to exhaustively iterate per proptest seed.
    /// n ∈ {1, 15, 16, 17, 63, 64, 65} covers below/at/above panel width
    /// (b=16) and below/at/above a 64-element SIMD-lane register.
    const INV_BOUNDARY_LENS: &[usize] = &[1, 15, 16, 17, 63, 64, 65];

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 8, .. proptest::test_runner::Config::default() })]

        /// `A · A⁻¹ == I_n` at all boundary sizes over GF(7).
        ///
        /// Addresses SC#2 of jit:8df0c501. Seeds drive the matrix generator;
        /// the inner loop exhaustively covers all boundary n values so both
        /// the scalar path (n < 16) and the blocked path (n >= 16) are hit.
        #[test]
        fn prop_blocked_inv_product_fp7(seed in 0u64..1_000_000) {
            for &n in INV_BOUNDARY_LENS {
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9));
                let a = random_fp_invertible::<7>(n, mseed);
                let a_inv = a.inv();
                proptest::prop_assert!(a_inv.is_some(), "inv returned None for n={}", n);
                let a_inv = a_inv.unwrap();
                let prod = gemm(&a, &a_inv);
                let id = FieldMatrix::<Fp<7>>::identity(n);
                proptest::prop_assert_eq!(&prod, &id,
                    "A·A⁻¹ != I for GF(7) n={} seed={}", n, seed);
                let prod2 = gemm(&a_inv, &a);
                proptest::prop_assert_eq!(&prod2, &id,
                    "A⁻¹·A != I for GF(7) n={} seed={}", n, seed);
            }
        }

        /// `A · A⁻¹ == I_n` at all boundary sizes over GF(31).
        #[test]
        fn prop_blocked_inv_product_fp31(seed in 0u64..1_000_000) {
            for &n in INV_BOUNDARY_LENS {
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9));
                let a = random_fp_invertible::<31>(n, mseed);
                let a_inv = a.inv();
                proptest::prop_assert!(a_inv.is_some(), "inv returned None for n={}", n);
                let a_inv = a_inv.unwrap();
                let prod = gemm(&a, &a_inv);
                let id = FieldMatrix::<Fp<31>>::identity(n);
                proptest::prop_assert_eq!(&prod, &id,
                    "A·A⁻¹ != I for GF(31) n={} seed={}", n, seed);
                let prod2 = gemm(&a_inv, &a);
                proptest::prop_assert_eq!(&prod2, &id,
                    "A⁻¹·A != I for GF(31) n={} seed={}", n, seed);
            }
        }

        /// `A · A⁻¹ == I_n` at all boundary sizes over GF(127).
        #[test]
        fn prop_blocked_inv_product_fp127(seed in 0u64..1_000_000) {
            for &n in INV_BOUNDARY_LENS {
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9));
                let a = random_fp_invertible::<127>(n, mseed);
                let a_inv = a.inv();
                proptest::prop_assert!(a_inv.is_some(), "inv returned None for n={}", n);
                let a_inv = a_inv.unwrap();
                let prod = gemm(&a, &a_inv);
                let id = FieldMatrix::<Fp<127>>::identity(n);
                proptest::prop_assert_eq!(&prod, &id,
                    "A·A⁻¹ != I for GF(127) n={} seed={}", n, seed);
                let prod2 = gemm(&a_inv, &a);
                proptest::prop_assert_eq!(&prod2, &id,
                    "A⁻¹·A != I for GF(127) n={} seed={}", n, seed);
            }
        }

        /// `A · A⁻¹ == I_n` at all boundary sizes over GF(241).
        #[test]
        fn prop_blocked_inv_product_fp241(seed in 0u64..1_000_000) {
            for &n in INV_BOUNDARY_LENS {
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9));
                let a = random_fp_invertible::<241>(n, mseed);
                let a_inv = a.inv();
                proptest::prop_assert!(a_inv.is_some(), "inv returned None for n={}", n);
                let a_inv = a_inv.unwrap();
                let prod = gemm(&a, &a_inv);
                let id = FieldMatrix::<Fp<241>>::identity(n);
                proptest::prop_assert_eq!(&prod, &id,
                    "A·A⁻¹ != I for GF(241) n={} seed={}", n, seed);
                let prod2 = gemm(&a_inv, &a);
                proptest::prop_assert_eq!(&prod2, &id,
                    "A⁻¹·A != I for GF(241) n={} seed={}", n, seed);
            }
        }

        /// `A · A⁻¹ == I_n` at all boundary sizes over GF(251).
        #[test]
        fn prop_blocked_inv_product_fp251(seed in 0u64..1_000_000) {
            for &n in INV_BOUNDARY_LENS {
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9));
                let a = random_fp_invertible::<251>(n, mseed);
                let a_inv = a.inv();
                proptest::prop_assert!(a_inv.is_some(), "inv returned None for n={}", n);
                let a_inv = a_inv.unwrap();
                let prod = gemm(&a, &a_inv);
                let id = FieldMatrix::<Fp<251>>::identity(n);
                proptest::prop_assert_eq!(&prod, &id,
                    "A·A⁻¹ != I for GF(251) n={} seed={}", n, seed);
                let prod2 = gemm(&a_inv, &a);
                proptest::prop_assert_eq!(&prod2, &id,
                    "A⁻¹·A != I for GF(251) n={} seed={}", n, seed);
            }
        }

        /// `A · A⁻¹ == I_n` at all boundary sizes over GF(65521).
        #[test]
        fn prop_blocked_inv_product_fp65521(seed in 0u64..1_000_000) {
            for &n in INV_BOUNDARY_LENS {
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9));
                let a = random_fp_invertible::<65521>(n, mseed);
                let a_inv = a.inv();
                proptest::prop_assert!(a_inv.is_some(), "inv returned None for n={}", n);
                let a_inv = a_inv.unwrap();
                let prod = gemm(&a, &a_inv);
                let id = FieldMatrix::<Fp<65521>>::identity(n);
                proptest::prop_assert_eq!(&prod, &id,
                    "A·A⁻¹ != I for GF(65521) n={} seed={}", n, seed);
                let prod2 = gemm(&a_inv, &a);
                proptest::prop_assert_eq!(&prod2, &id,
                    "A⁻¹·A != I for GF(65521) n={} seed={}", n, seed);
            }
        }
    }

    // ── SC#5.2 — Rank-deficient inputs return None (blocked path) ───────────────

    /// Rank-deficient inputs at boundary sizes return `None` without panicking.
    ///
    /// Mirrors design doc feb15da9 §5.2 (rank-deficient failure mode). Tests
    /// n ∈ {16, 32, 64} (all at/above BLOCKED_INVERT_THRESHOLD) to exercise
    /// the panelized path's early-exit on rank detection.
    #[test]
    fn test_blocked_inv_rank_deficient_fp7() {
        for &n in &[16usize, 32, 64] {
            // Rank-n/2 matrix: duplicate first n/2 rows into second n/2.
            let mut a = random_fp::<7>(n, n, 0x000D_EAD7_u64.wrapping_add(n as u64));
            for j in 0..n {
                let v = a.get(0, j);
                a.set(n / 2, j, v);
            }
            assert!(
                a.inv().is_none(),
                "blocked inv should return None for rank-deficient GF(7) n={}",
                n
            );
        }
    }

    #[test]
    fn test_blocked_inv_rank_deficient_fp251() {
        for &n in &[16usize, 32, 64] {
            let mut a = random_fp::<251>(n, n, 0x0DEA_D251_u64.wrapping_add(n as u64));
            for j in 0..n {
                let v = a.get(0, j);
                a.set(n / 2, j, v);
            }
            assert!(
                a.inv().is_none(),
                "blocked inv should return None for rank-deficient GF(251) n={}",
                n
            );
        }
    }

    #[test]
    fn test_blocked_inv_rank_deficient_fp65521() {
        for &n in &[16usize, 32, 64] {
            let mut a = random_fp::<65521>(n, n, 0xDEAD_6552_1000_u64.wrapping_add(n as u64));
            for j in 0..n {
                let v = a.get(0, j);
                a.set(n / 2, j, v);
            }
            assert!(
                a.inv().is_none(),
                "blocked inv should return None for rank-deficient GF(65521) n={}",
                n
            );
        }
    }

    // ── SC#5.3 — Dispatch-boundary correctness ────────────────────────────────
    //
    // Verifies that inv() returns correct results at n = THRESHOLD-1 (scalar
    // path) and n = THRESHOLD+1 (blocked path), and that both agree with
    // the reference Dumas–Pernet driver.

    #[test]
    fn test_blocked_inv_dispatch_boundary_fp7() {
        let threshold = super::BLOCKED_INVERT_THRESHOLD;
        // n just below threshold — scalar path.
        let n_below = threshold - 1;
        if n_below >= 1 {
            for seed in 0..3u64 {
                let a = random_fp_invertible::<7>(n_below, seed * 37 + n_below as u64);
                let new_inv = a.inv().expect("invertible");
                let ref_inv = inv_reference_dumas_pernet(&a).expect("invertible");
                assert_eq!(
                    new_inv, ref_inv,
                    "dispatch-boundary: inv differs from reference at n={} (below threshold)",
                    n_below
                );
            }
        }
        // n just above threshold — blocked path.
        let n_above = threshold + 1;
        for seed in 0..3u64 {
            let a = random_fp_invertible::<7>(n_above, seed * 41 + n_above as u64);
            let new_inv = a.inv().expect("invertible");
            let ref_inv = inv_reference_dumas_pernet(&a).expect("invertible");
            assert_eq!(
                new_inv, ref_inv,
                "dispatch-boundary: inv differs from reference at n={} (above threshold)",
                n_above
            );
        }
    }

    #[test]
    fn test_blocked_inv_dispatch_boundary_fp251() {
        let threshold = super::BLOCKED_INVERT_THRESHOLD;
        let n_below = threshold - 1;
        if n_below >= 1 {
            for seed in 0..3u64 {
                let a = random_fp_invertible::<251>(n_below, seed * 43 + n_below as u64);
                let new_inv = a.inv().expect("invertible");
                let ref_inv = inv_reference_dumas_pernet(&a).expect("invertible");
                assert_eq!(
                    new_inv, ref_inv,
                    "dispatch-boundary fp251: at n={} (below threshold)",
                    n_below
                );
            }
        }
        let n_above = threshold + 1;
        for seed in 0..3u64 {
            let a = random_fp_invertible::<251>(n_above, seed * 47 + n_above as u64);
            let new_inv = a.inv().expect("invertible");
            let ref_inv = inv_reference_dumas_pernet(&a).expect("invertible");
            assert_eq!(
                new_inv, ref_inv,
                "dispatch-boundary fp251: at n={} (above threshold)",
                n_above
            );
        }
    }

    // ── SC#5.4 — Allocation budget for blocked path ───────────────────────────
    //
    // Pins the FieldMatrix::new count for the blocked-path at n=64 over
    // Fp<MERSENNE_31>. The blocked path allocates:
    //   - ple() budget (as before)
    //   - 1 n×n identity scratch (Y)
    //   - trsm_lower budget (block-recursive + gemm_axpy B-transpose)
    //   - trsm_upper budget
    //   - 1 n×n output
    // The exact count is measured empirically at first run; the test uses
    // `assert!(allocs <= UPPER_BOUND)` with a documented expected value.

    #[test]
    #[serial]
    fn test_blocked_inv_allocation_budget_n64_fp7() {
        let a = random_fp_invertible::<7>(64, 0x000B_10C7_u64);
        reset_fieldmatrix_new_count();
        let _ = a.inv();
        let allocs = fieldmatrix_new_count();
        // The blocked path at n=64 over GF(7):
        //   ple(64×64) + 1 identity + trsm_lower + trsm_upper + 1 output.
        // An upper bound of 700 is conservative relative to the prior scalar
        // path at n=64 (386 allocs); the blocked path has two full trsm calls
        // instead of two trtri + one trtrm, so it uses more scratch buffers.
        // This bound is tightened after empirical measurement.
        assert!(
            allocs <= 700,
            "blocked inv(64×64 Fp<7>) allocs={} exceeds upper bound 700",
            allocs
        );
    }

    // ── SC#2 extended — existing reference-match tests cover n≤32 ─────────────
    // The existing test_inv_matches_reference_fp7/fp251/fp65521/mersenne31 tests
    // now exercise both the scalar path (n ≤ 15) and the blocked path (n = 16, 32).
    // No new test is needed; the boundary coverage is already present.
    // ─── Blocked solve_batch correctness — boundary-length sweep ─────────
    //
    // These tests verify that the blocked-TRSM dispatch in solve_batch
    // is correct: A · solve_batch(A, B) == B for square full-rank A and
    // arbitrary B.  The blocked path is only activated when
    // F::has_simd_gemm_classical() is true AND n >= TRSM_BLOCKED_PANEL_SIZE,
    // so we include sizes both below and at/above the panel boundary.
    //
    // Note: rank-deficient inputs are covered by test_solve_batch_rank_deficient_*
    // above; here we focus on correctness across the blocked/unblocked
    // boundary sizes (1, 15, 16, 17, 63, 64, 65) for six primes.

    const SOLVE_BOUNDARY_LENS: &[usize] = &[1, 15, 16, 17, 63, 64, 65];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

        /// Blocked solve_batch round-trip: A · X == B for Fp<7>.
        #[test]
        fn prop_blocked_solve_boundary_sweep_fp7(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                for &k in SOLVE_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((k as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_invertible::<7>(n, mseed);
                    let b = random_fp::<7>(n, k, mseed.wrapping_add(0xC1));
                    let x = a.solve_batch(&b).expect("full-rank");
                    let recon = gemm(&a, &x);
                    proptest::prop_assert_eq!(&recon, &b, "Fp<7> n={} k={}", n, k);
                }
            }
        }

        /// Blocked solve_batch round-trip: A · X == B for Fp<31>.
        #[test]
        fn prop_blocked_solve_boundary_sweep_fp31(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                for &k in SOLVE_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((k as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_invertible::<31>(n, mseed);
                    let b = random_fp::<31>(n, k, mseed.wrapping_add(0xC2));
                    let x = a.solve_batch(&b).expect("full-rank");
                    let recon = gemm(&a, &x);
                    proptest::prop_assert_eq!(&recon, &b, "Fp<31> n={} k={}", n, k);
                }
            }
        }

        /// Blocked solve_batch round-trip: A · X == B for Fp<127>.
        #[test]
        fn prop_blocked_solve_boundary_sweep_fp127(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                for &k in SOLVE_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((k as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_invertible::<127>(n, mseed);
                    let b = random_fp::<127>(n, k, mseed.wrapping_add(0xC3));
                    let x = a.solve_batch(&b).expect("full-rank");
                    let recon = gemm(&a, &x);
                    proptest::prop_assert_eq!(&recon, &b, "Fp<127> n={} k={}", n, k);
                }
            }
        }

        /// Blocked solve_batch round-trip: A · X == B for Fp<241>.
        #[test]
        fn prop_blocked_solve_boundary_sweep_fp241(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                for &k in SOLVE_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((k as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_invertible::<241>(n, mseed);
                    let b = random_fp::<241>(n, k, mseed.wrapping_add(0xC4));
                    let x = a.solve_batch(&b).expect("full-rank");
                    let recon = gemm(&a, &x);
                    proptest::prop_assert_eq!(&recon, &b, "Fp<241> n={} k={}", n, k);
                }
            }
        }

        /// Blocked solve_batch round-trip: A · X == B for Fp<251>.
        #[test]
        fn prop_blocked_solve_boundary_sweep_fp251(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                for &k in SOLVE_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((k as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_invertible::<251>(n, mseed);
                    let b = random_fp::<251>(n, k, mseed.wrapping_add(0xC5));
                    let x = a.solve_batch(&b).expect("full-rank");
                    let recon = gemm(&a, &x);
                    proptest::prop_assert_eq!(&recon, &b, "Fp<251> n={} k={}", n, k);
                }
            }
        }

        /// Blocked solve_batch round-trip: A · X == B for Fp<65521>.
        #[test]
        fn prop_blocked_solve_boundary_sweep_fp65521(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                for &k in SOLVE_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((n as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((k as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_invertible::<65521>(n, mseed);
                    let b = random_fp::<65521>(n, k, mseed.wrapping_add(0xC6));
                    let x = a.solve_batch(&b).expect("full-rank");
                    let recon = gemm(&a, &x);
                    proptest::prop_assert_eq!(&recon, &b, "Fp<65521> n={} k={}", n, k);
                }
            }
        }
    }

    // ─── Blocked solve_batch correctness — rank-deficient sweep ──────────
    //
    // These tests verify that solve_batch returns None for any rank-deficient
    // square matrix, covering all 6 primes required by SC#2.
    //
    // Construction: A = F · G where F is n × rank and G is rank × n (outer
    // product), giving rank(A) = rank < n. We iterate over SOLVE_BOUNDARY_LENS
    // as the matrix dimension n, and set rank = n / 2 (skip n < 2 since
    // rank-deficiency requires rank < n and rank >= 1).
    //
    // The proptest seed parameterizes the outer-product factors (different
    // seeds → different rank-deficient matrices), giving broad coverage beyond
    // the narrow deterministic cases in test_solve_batch_rank_deficient_*.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

        /// solve_batch returns None for rank-deficient Fp<7> at boundary sizes.
        #[test]
        fn prop_blocked_solve_rank_deficient_fp7(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                let rank = n / 2;
                if rank == 0 {
                    continue;
                }
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0xDEAD_BEEF))
                    .wrapping_add(7);
                let a = random_fp_rank_deficient::<7>(n, n, rank, mseed);
                let b = random_fp::<7>(n, n, mseed.wrapping_add(0xD1));
                proptest::prop_assert!(
                    a.solve_batch(&b).is_none(),
                    "Fp<7> n={n} rank={rank}: expected None (singular A)"
                );
            }
        }

        /// solve_batch returns None for rank-deficient Fp<31> at boundary sizes.
        #[test]
        fn prop_blocked_solve_rank_deficient_fp31(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                let rank = n / 2;
                if rank == 0 {
                    continue;
                }
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0xDEAD_BEEF))
                    .wrapping_add(31);
                let a = random_fp_rank_deficient::<31>(n, n, rank, mseed);
                let b = random_fp::<31>(n, n, mseed.wrapping_add(0xD2));
                proptest::prop_assert!(
                    a.solve_batch(&b).is_none(),
                    "Fp<31> n={n} rank={rank}: expected None (singular A)"
                );
            }
        }

        /// solve_batch returns None for rank-deficient Fp<127> at boundary sizes.
        #[test]
        fn prop_blocked_solve_rank_deficient_fp127(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                let rank = n / 2;
                if rank == 0 {
                    continue;
                }
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0xDEAD_BEEF))
                    .wrapping_add(127);
                let a = random_fp_rank_deficient::<127>(n, n, rank, mseed);
                let b = random_fp::<127>(n, n, mseed.wrapping_add(0xD3));
                proptest::prop_assert!(
                    a.solve_batch(&b).is_none(),
                    "Fp<127> n={n} rank={rank}: expected None (singular A)"
                );
            }
        }

        /// solve_batch returns None for rank-deficient Fp<241> at boundary sizes.
        #[test]
        fn prop_blocked_solve_rank_deficient_fp241(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                let rank = n / 2;
                if rank == 0 {
                    continue;
                }
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0xDEAD_BEEF))
                    .wrapping_add(241);
                let a = random_fp_rank_deficient::<241>(n, n, rank, mseed);
                let b = random_fp::<241>(n, n, mseed.wrapping_add(0xD4));
                proptest::prop_assert!(
                    a.solve_batch(&b).is_none(),
                    "Fp<241> n={n} rank={rank}: expected None (singular A)"
                );
            }
        }

        /// solve_batch returns None for rank-deficient Fp<251> at boundary sizes.
        #[test]
        fn prop_blocked_solve_rank_deficient_fp251(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                let rank = n / 2;
                if rank == 0 {
                    continue;
                }
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0xDEAD_BEEF))
                    .wrapping_add(251);
                let a = random_fp_rank_deficient::<251>(n, n, rank, mseed);
                let b = random_fp::<251>(n, n, mseed.wrapping_add(0xD5));
                proptest::prop_assert!(
                    a.solve_batch(&b).is_none(),
                    "Fp<251> n={n} rank={rank}: expected None (singular A)"
                );
            }
        }

        /// solve_batch returns None for rank-deficient Fp<65521> at boundary sizes.
        #[test]
        fn prop_blocked_solve_rank_deficient_fp65521(seed in 0u64..1_000_000) {
            for &n in SOLVE_BOUNDARY_LENS {
                let rank = n / 2;
                if rank == 0 {
                    continue;
                }
                let mseed = seed
                    .wrapping_add((n as u64).wrapping_mul(0xDEAD_BEEF))
                    .wrapping_add(65521);
                let a = random_fp_rank_deficient::<65521>(n, n, rank, mseed);
                let b = random_fp::<65521>(n, n, mseed.wrapping_add(0xD6));
                proptest::prop_assert!(
                    a.solve_batch(&b).is_none(),
                    "Fp<65521> n={n} rank={rank}: expected None (singular A)"
                );
            }
        }
    }
}
