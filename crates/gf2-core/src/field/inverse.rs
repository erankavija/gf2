//! Matrix inversion, linear-system solving, and determinant over an
//! arbitrary [`FiniteField`].
//!
//! Issue `ae1d1e88`. Implements Dumas–Pernet §2.3 Table 2 by composing the
//! PLE decomposition (issue `c3f8c1cb`) with the triangular primitives
//! (issue `83b1ad8b`):
//!
//! - [`FieldMatrix::inv`] / [`inv`] — `A⁻¹ = E⁻¹ · L⁻¹ · Pᵀ` where
//!   `(P, L, E, r) = self.ple()`. Defined iff `r == n == m`; returns
//!   `None` on rank-deficient input.
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
//! [`trtri_upper`](crate::field::triangular::trtri_upper). No bespoke
//! kernels.
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

use crate::field::matrix::{gemm_into_view, FieldMatrix};
use crate::field::triangular::{trsm_lower, trsm_upper, trtri_lower, trtri_upper};
use crate::field::vec::FieldVec;
use crate::field::FiniteField;

// ─── Public methods on FieldMatrix ───────────────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns the matrix inverse `A⁻¹` if `self` is non-singular.
    ///
    /// Implements Dumas–Pernet §2.3 Table 2. Computes the PLE
    /// decomposition `P · L · E = self`. If `rank < n`, returns `None`.
    /// Otherwise inverts each triangular factor in place
    /// ([`trtri_lower`](crate::field::triangular::trtri_lower) on `L`,
    /// [`trtri_upper`](crate::field::triangular::trtri_upper) on `E`),
    /// composes `temp = E⁻¹ · L⁻¹` via
    /// [`gemm_into_view`](crate::field::matrix::gemm_into_view), and
    /// applies `Pᵀ` on the right by column-permuting `temp` into the
    /// result.
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
    /// one dense gemm).
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
        let (perm, mut l, mut e, rank) = self.ple();
        if rank < n {
            return None;
        }
        // Full rank ⇒ L is n×n unit lower-triangular, E is n×n with
        // pivots on the leading diagonal (i.e. upper-triangular).
        // Invert in place via the §2.3 algorithm 2.3 primitives.
        trtri_lower(l.submat_mut(.., ..));
        trtri_upper(e.submat_mut(.., ..));

        // temp = E⁻¹ · L⁻¹ via the standard view-based gemm kernel.
        let zero = self.get(0, 0).zero_like();
        let mut temp = FieldMatrix::new(n, n, zero.clone());
        gemm_into_view(&e, &l, temp.submat_mut(.., ..));

        // Apply Pᵀ on the right: (M · Pᵀ)[i, j] = M[i, perm[j]].
        // Materialise the column-permuted output. We cannot do this in
        // place (column-permutation in row-major storage would alias
        // sources and sinks within a row); a fresh allocation is the
        // standard library-style cost.
        let mut out = FieldMatrix::new(n, n, zero);
        let perm_idx = perm.indices();
        for i in 0..n {
            for (j, &src_col) in perm_idx.iter().enumerate() {
                out.set(i, j, temp.get(i, src_col));
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
        trsm_lower(l.submat(.., ..), y.submat_mut(.., ..));

        // Solve E · X = Y' in place. E is n×n upper-triangular at full
        // rank (pivots on the leading diagonal because rank == n).
        trsm_upper(e.submat(.., ..), y.submat_mut(.., ..));

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
        random_fp, random_fp_invertible, random_gf2m_wide_1, random_gf2m_wide_1_invertible,
    };
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
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
    //              + 1 (temp = E⁻¹ · L⁻¹)  // gemm result holder
    //              + 2 (gemm B-transpose:  // to_owned + transpose
    //                   one per gemm_into_view)
    //              + 1 (column-permuted output)
    //              + (trtri's base-case `inv` scratches and outer
    //                 chain scratches at each peeled level — one
    //                 per peel for L and one for E)
    //
    //   solve_batch(n × n, n × k) = ple + 1 (perm.inverse().apply)
    //              + 2 trsm calls × kernel B-transpose tree
    //
    //   det(n × n) = ple                   // L is dropped
    //
    // The empirical numbers are the sum of these per-call costs at
    // the chosen recursion thresholds. Numbers below match the
    // post-c3f8c1cb PLE budget (EXPECTED_PLE_N64 = 254 etc.) plus the
    // small additions above.
    const EXPECTED_INV_N4: u64 = 19;
    const EXPECTED_INV_N64: u64 = 271;
    const EXPECTED_INV_N1024: u64 = 4569;
    const EXPECTED_SOLVE_N64: u64 = 260;
    const EXPECTED_DET_N64: u64 = 254;
}
