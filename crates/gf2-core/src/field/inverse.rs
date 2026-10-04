//! Matrix inversion, linear-system solving, and determinant over an
//! arbitrary [`FiniteField`], composed from the PLE decomposition and the
//! triangular kernels (`@/citation/DumasPernet2012` §2.3, Table 2).

use crate::field::matrix::FieldMatrix;
#[cfg(any(test, feature = "test-support"))]
use crate::field::matrix::QuietObservations;
use crate::field::matrix::{ObservationPolicy, RecordObservations};
#[cfg(test)]
use crate::field::triangular::TRSM_BLOCKED_PANEL_SIZE;
use crate::field::triangular::{
    trsm_lower, trsm_lower_blocked_with_policy, trsm_lower_with_policy, trsm_route_resolved,
    trsm_upper, trsm_upper_blocked_with_policy, trsm_upper_with_policy, trtri_lower, trtri_upper,
    trtrm, TrsmRoute,
};
#[cfg(test)]
use crate::field::triangular::{trsm_lower_blocked, trsm_upper_blocked};
use crate::field::vec::FieldVec;
use crate::field::FiniteField;
use crate::tuning;

/// Conservative default for `dense_inverse.blocked_min_dim()`, consumed
/// by [`crate::tuning::CoreTuning::CONSERVATIVE`].
pub(crate) const BLOCKED_INVERT_THRESHOLD: usize = 16;

/// The selected arm of the [`FieldMatrix::inv`] dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvRoute {
    /// Use the scalar-pivot PLE + `trtri` + `trtrm` driver.
    ScalarPle,
    /// Use the panelized blocked-invert driver (`blocked_inv_panelized`).
    BlockedPanelized,
}

/// Reports the [`FieldMatrix::inv`] arm for a matrix of dimension `n`.
///
/// The comparison uses the active `dense_inverse.blocked_min_dim()` profile
/// value: matrices with `n` at or above it take the panelized blocked path,
/// below it the scalar-pivot path.
#[must_use]
pub fn inv_route(n: usize) -> InvRoute {
    inv_route_resolved(tuning::active().dense_inverse().blocked_min_dim(), n)
}

fn inv_route_resolved(blocked_min_dim: usize, n: usize) -> InvRoute {
    if n >= blocked_min_dim {
        InvRoute::BlockedPanelized
    } else {
        InvRoute::ScalarPle
    }
}

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns the matrix inverse `A⁻¹`, or `None` if `self` is singular.
    ///
    /// Starts from the PLE decomposition `P · L · E = self`
    /// (`@/citation/DumasPernet2012` §2.3, Table 2). The route reported by
    /// [`inv_route`] either inverts `L` and `E` and composes `E⁻¹ · L⁻¹`
    /// in place with [`trtrm`], or solves `L · Y = I` and `E · X = Y`;
    /// both then apply `Pᵀ` on the right.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations.
    pub fn inv(&self) -> Option<FieldMatrix<F>> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::inv: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            // The 0×0 matrix is its own inverse.
            return Some(self.clone());
        }

        if inv_route(n) == InvRoute::BlockedPanelized {
            return blocked_inv_panelized(self);
        }

        let (perm, mut l, mut e, rank) = self.ple();
        if rank < n {
            return None;
        }
        // Full rank ⇒ L is n×n unit lower-triangular and E is n×n
        // upper-triangular.
        trtri_lower(l.submat_mut(.., ..));
        trtri_upper(e.submat_mut(.., ..));

        // `trtrm` overwrites `L⁻¹` with `E⁻¹ · L⁻¹`. It treats its first
        // operand as unit lower-triangular and does not read the diagonal.
        trtrm(l.submat_mut(.., ..), e.submat(.., ..));

        // Apply Pᵀ on the right: (M · Pᵀ)[i, j] = M[i, perm[j]].
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
    /// Returns `Some(x)` with `A · x == b` iff `A` is non-singular, and
    /// `None` for every rank-deficient `A`: inconsistent systems and
    /// underdetermined compatible systems are not distinguished.
    ///
    /// # Panics
    ///
    /// * Panics if `self` is not square.
    /// * Panics if `b.len() != self.rows()`.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations — dominated by the PLE.
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

    /// Solves `A · X = B` for a right-hand-side matrix `B` with one PLE
    /// decomposition and two triangular solves
    /// (`@/citation/DumasPernet2012` §2.3, Table 2).
    ///
    /// Returns `Some(X)` with `A · X == B` iff `A` is non-singular, and
    /// `None` for every rank-deficient `A`.
    ///
    /// # Panics
    ///
    /// * Panics if `self` is not square.
    /// * Panics if `b.rows() != self.rows()`.
    /// * Panics if `b` has no entries and the field has no static zero.
    ///
    /// # Complexity
    ///
    /// `O(n³ + n² · k)` field operations.
    pub fn solve_batch(&self, b: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
        self.solve_batch_with_policy::<RecordObservations>(b)
    }

    fn solve_batch_with_policy<O: ObservationPolicy>(
        &self,
        b: &FieldMatrix<F>,
    ) -> Option<FieldMatrix<F>> {
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
        let (perm, l, e, rank) = self.ple_with_policy::<O>();
        if rank < n {
            return None;
        }
        // Y = Pᵀ · B. `Permutation::apply` computes
        // (P · B)[i] = B[perm[i]], so Pᵀ takes the inverse permutation.
        let mut y = perm.inverse().apply(b);
        let tuning = tuning::active();
        let triangular = tuning.triangular();
        let trsm_blocked_min_dim = triangular.trsm_blocked_min_dim();
        let trsm_panel_rows = triangular.trsm_panel_rows();

        // Solve L · Y' = Y in place. L is n×n unit lower-triangular at
        // full rank.
        if F::has_simd_gemm_classical()
            && trsm_route_resolved(trsm_blocked_min_dim, n) == TrsmRoute::Blocked
        {
            trsm_lower_blocked_with_policy::<F, O>(
                l.submat(.., ..),
                y.submat_mut(.., ..),
                trsm_panel_rows,
            );
        } else {
            trsm_lower_with_policy::<F, O>(l.submat(.., ..), y.submat_mut(.., ..));
        }

        // Solve E · X = Y' in place. E is n×n upper-triangular at full
        // rank (pivots on the leading diagonal because rank == n).
        if F::has_simd_gemm_classical()
            && trsm_route_resolved(trsm_blocked_min_dim, n) == TrsmRoute::Blocked
        {
            trsm_upper_blocked_with_policy::<F, O>(
                e.submat(.., ..),
                y.submat_mut(.., ..),
                trsm_panel_rows,
            );
        } else {
            trsm_upper_with_policy::<F, O>(e.submat(.., ..), y.submat_mut(.., ..));
        }

        Some(y)
    }

    /// [`solve_batch`](Self::solve_batch) under the zero-sized quiet
    /// observation policy: the same validation, profile reads and
    /// computation, with no route, panel, or GEMM observations.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`solve_batch`](Self::solve_batch).
    ///
    /// # Complexity
    ///
    /// `O(n³ + n² · k)` field operations.
    #[cfg(any(test, feature = "test-support"))]
    pub fn solve_batch_quiet_for_test(&self, b: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
        self.solve_batch_with_policy::<QuietObservations>(b)
    }

    /// Returns the determinant `det(self)`: the field zero iff
    /// `rank(self) < n`, and `1` for the `0×0` matrix.
    ///
    /// From the PLE decomposition `P · L · E = self`
    /// (`@/citation/DumasPernet2012` §2.3, Table 2), at full rank
    /// `det(self) = sign(P) · ∏ E[i, i]`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square. Panics on a `0×0` matrix over a
    /// field without a static zero.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations (one PLE).
    pub fn det(&self) -> F {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::det: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            // Empty product: det of the 0×0 matrix is 1.
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
        let one = zero.one_like();
        let mut det = one.clone();
        for i in 0..n {
            det = det * e.get(i, i);
        }
        if permutation_sign_is_negative(perm.indices()) {
            det = -det;
        }
        det
    }
}

/// Returns `A⁻¹`, or `None` if `a` is singular; see [`FieldMatrix::inv`].
pub fn inv<F: FiniteField>(a: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
    a.inv()
}

/// Solves `A · x = b`; see [`FieldMatrix::solve`].
pub fn solve<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldVec<F>) -> Option<FieldVec<F>> {
    a.solve(b)
}

/// Returns `det(a)`; see [`FieldMatrix::det`].
pub fn det<F: FiniteField>(a: &FieldMatrix<F>) -> F {
    a.det()
}

/// Returns `true` iff the permutation has odd parity (i.e. `sign = −1`).
///
/// Counts inversions in `O(n²)`.
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

/// Inversion through PLE and two triangular solves on the identity
/// (`@/citation/Higham2002` §14.1): `L · Y = I`, `E · X = Y`, then `Pᵀ`
/// applied on the right. Returns `None` if `a` is rank-deficient.
fn blocked_inv_panelized<F: FiniteField>(a: &FieldMatrix<F>) -> Option<FieldMatrix<F>> {
    let n = a.rows();
    debug_assert_eq!(n, a.cols(), "blocked_inv_panelized: non-square input");

    let (perm, l, e, rank) = a.ple();

    if rank < n {
        return None;
    }

    // `inv` handles `n == 0` before this route, so `a.get(0, 0)` exists.
    let zero = a.get(0, 0).zero_like();
    let one = zero.one_like();
    let mut y = FieldMatrix::new(n, n, zero.clone());
    for i in 0..n {
        y.set(i, i, one.clone());
    }

    trsm_lower(l.submat(.., ..), y.submat_mut(.., ..));

    trsm_upper(e.submat(.., ..), y.submat_mut(.., ..));

    // Apply Pᵀ on the right: A⁻¹[i, j] = X[i, perm[j]].
    let mut out = FieldMatrix::new(n, n, zero);
    let perm_idx = perm.indices();
    for i in 0..n {
        for (j, &src_col) in perm_idx.iter().enumerate() {
            out.set(i, j, y.get(i, src_col));
        }
    }
    Some(out)
}

// Zero-sized results for `solve_batch`, whose zero witness may have to
// come from `F::zero_hint`.
impl<F: FiniteField> FieldMatrix<F> {
    fn new_empty_like(rows: usize, cols: usize, template: &FieldMatrix<F>) -> FieldMatrix<F> {
        if rows == 0 || cols == 0 {
            let zero = if !template.is_empty() {
                template.get(0, 0).zero_like()
            } else if let Some(z) = F::zero_hint() {
                z
            } else {
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

    /// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
    struct InvGf2m8Cfg;
    impl Gf2mWideConfig<1> for InvGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "InvGf2m8Cfg";
    }
    type Gf2m8 = Gf2mWide<1, InvGf2m8Cfg>;

    /// GF(2^16) with the Conway polynomial.
    struct InvGf2m16Cfg;
    impl Gf2mWideConfig<1> for InvGf2m16Cfg {
        const M: usize = 16;
        const MODULUS: [u64; 1] = [0x002D];
        const NAME: &'static str = "InvGf2m16Cfg";
    }
    type Gf2m16 = Gf2mWide<1, InvGf2m16Cfg>;

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        random_gf2m_wide_1::<InvGf2m8Cfg>(rows, cols, seed)
    }
    fn random_gf2m8_invertible(n: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        random_gf2m_wide_1_invertible::<InvGf2m8Cfg>(n, seed)
    }
    fn random_gf2m16_invertible(n: usize, seed: u64) -> FieldMatrix<Gf2m16> {
        random_gf2m_wide_1_invertible::<InvGf2m16Cfg>(n, seed)
    }

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
        let f1 = random_fp::<MERSENNE_31>(4, 1, 0x11);
        let f2 = random_fp::<MERSENNE_31>(1, 4, 0x22);
        let a = gemm(&f1, &f2);
        if a.rank() < 4 {
            assert!(a.inv().is_none());
        }
    }

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

    /// Rank-2 4×4 matrix: rows 0 and 2 hold `[1, 2, 3, 4]`, rows 1 and 3
    /// hold `[5, 6, 7, 8]`.
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

    #[test]
    fn test_solve_rank_deficient_nonzero_returns_none() {
        let a = rank_deficient_nonzero_4x4_fp_m31();
        assert_eq!(a.rank(), 2, "setup: expected rank-2 matrix");
        let b: FieldVec<Fp<MERSENNE_31>> = (0..4)
            .map(|i| Fp::<MERSENNE_31>::new(i as u64 + 1))
            .collect();
        assert!(a.solve(&b).is_none());
    }

    #[test]
    fn test_solve_batch_rank_deficient_nonzero_returns_none() {
        let a = rank_deficient_nonzero_4x4_fp_m31();
        assert_eq!(a.rank(), 2, "setup: expected rank-2 matrix");
        let b = random_fp::<MERSENNE_31>(4, 3, 0xABCDEF);
        assert!(a.solve_batch(&b).is_none());
    }

    /// Determinant by cofactor expansion along row 0, `O(n!)`.
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
        for j in 0..n {
            let aij = a.get(0, j);
            if aij == zero {
                continue;
            }
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
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        assert_eq!(a.det(), Fp::<MERSENNE_31>::new(0));
        let b = random_fp_invertible::<MERSENNE_31>(5, 0xC0FFEE);
        assert_ne!(b.det(), Fp::<MERSENNE_31>::new(0));
    }

    #[test]
    fn test_det_zero_for_rank_deficient_nonzero() {
        let a = rank_deficient_nonzero_4x4_fp_m31();
        assert_eq!(a.rank(), 2, "setup: expected rank-2 matrix");
        assert_eq!(a.det(), Fp::<MERSENNE_31>::new(0));
    }

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

    #[test]
    fn test_solve_batch_with_identity_yields_inverse() {
        let a = random_fp_invertible::<MERSENNE_31>(4, 0x9999);
        let id = FieldMatrix::<Fp<MERSENNE_31>>::identity(4);
        let x = a.solve_batch(&id).expect("invertible");
        let a_inv = a.inv().expect("invertible");
        assert_eq!(x, a_inv);
    }

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
        // The zero leading entry forces a row swap.
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

    // Reference driver (`@/citation/DumasPernet2012` §2.3, Table 2): PLE,
    // two `trtri`, one dense `gemm`, column permutation.
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

    // Exact `fieldmatrix_new_count` readings for the current drivers. They
    // change with the recursion strategy, the dispatch thresholds and the
    // allocation footprint of the PLE, gemm, trsm, trtri and trtrm kernels.
    const EXPECTED_INV_N4: u64 = 17;
    const EXPECTED_INV_N64: u64 = 294;
    const EXPECTED_INV_N1024: u64 = 5246;
    const EXPECTED_SOLVE_N64: u64 = 294;
    const EXPECTED_DET_N64: u64 = 264;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

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

        #[test]
        fn proptest_inv_round_trip_gf2m8(
            n in 1usize..=6,
            seed in any::<u64>(),
        ) {
            let a = random_gf2m8_invertible(n, seed);
            let inv = a.inv().expect("full-rank");
            let prod = gemm(&a, &inv);
            let prod2 = gemm(&inv, &a);
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

        #[test]
        fn proptest_solve_round_trip_fp_m31(
            n in 1usize..=6,
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let a = random_fp_invertible::<MERSENNE_31>(n, seed_a);
            let b = crate::field::test_random_matrix::random_fp_vec::<MERSENNE_31>(n, seed_b);
            let x = a.solve(&b).expect("full-rank");
            let bb = a.matvec(&x);
            prop_assert_eq!(bb, b);
        }

        /// Holds for singular factors too: both sides are then zero.
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

    /// Sizes on both sides of `BLOCKED_INVERT_THRESHOLD` and of 64.
    const INV_BOUNDARY_LENS: &[usize] = &[1, 15, 16, 17, 63, 64, 65];

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 8, .. proptest::test_runner::Config::default() })]

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

    #[test]
    fn test_blocked_inv_rank_deficient_fp7() {
        for &n in &[16usize, 32, 64] {
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

    #[test]
    fn test_blocked_inv_dispatch_boundary_fp7() {
        let threshold = super::BLOCKED_INVERT_THRESHOLD;
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

    #[test]
    #[serial]
    fn test_blocked_inv_allocation_budget_n64_fp7() {
        let a = random_fp_invertible::<7>(64, 0x000B_10C7_u64);
        reset_fieldmatrix_new_count();
        let _ = a.inv();
        let allocs = fieldmatrix_new_count();
        assert!(
            allocs <= 700,
            "blocked inv(64×64 Fp<7>) allocs={} exceeds upper bound 700",
            allocs
        );
    }

    const SOLVE_BOUNDARY_LENS: &[usize] = &[1, 15, 16, 17, 63, 64, 65];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

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

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

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

    #[test]
    fn test_solve_batch_fp65521_n64_single_rhs_correctness() {
        let a = random_fp_invertible::<65521>(64, 0xDEAD_9138_D86C_0001);
        let b = random_fp::<65521>(64, 1, 0xBEEF_9138_D86C_0001);
        let x = a.solve_batch(&b).expect("full-rank 64×64 GF(65521)");
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "A·X != B for GF(65521) n=64 k=1");
    }

    #[test]
    fn test_solve_batch_fp65521_n64_square_rhs_correctness() {
        let a = random_fp_invertible::<65521>(64, 0xDEAD_9138_D86C_0002);
        let b = random_fp::<65521>(64, 64, 0xBEEF_9138_D86C_0002);
        let x = a.solve_batch(&b).expect("full-rank 64×64 GF(65521)");
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "A·X != B for GF(65521) n=64 k=64");
    }

    #[test]
    fn test_solve_batch_fp65521_n64_rank_deficient_returns_none() {
        let a = random_fp_rank_deficient::<65521>(64, 64, 32, 0xDEAD_9138_D86C_0003);
        let b = random_fp::<65521>(64, 1, 0xBEEF_9138_D86C_0003);
        assert!(
            a.solve_batch(&b).is_none(),
            "GF(65521) n=64 rank-deficient: expected None"
        );
    }

    fn build_borderline_input<const P: u64>(n: usize, regime: &str) -> FieldMatrix<Fp<P>> {
        let seed = P
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(n as u64)
            .wrapping_add(if regime == "deficient" { 0x1234 } else { 0 });
        if regime == "deficient" {
            let rank = (n / 2).max(1);
            let f = random_fp::<P>(n, rank, seed);
            let g = random_fp::<P>(rank, n, seed.wrapping_add(0xCAFE));
            gemm(&f, &g)
        } else {
            random_fp_invertible::<P>(n, seed)
        }
    }

    /// Times the phases of `row_echelon`.
    fn decompose_echelon<const P: u64>(a: &FieldMatrix<Fp<P>>, regime: &str, trial: usize) {
        let n = a.rows();
        let t = std::time::Instant::now();
        let (p, l, e, _r) = a.ple();
        let ns_ple = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        let l_full = {
            let zero = a.get(0, 0).zero_like();
            let one = zero.one_like();
            let mut lf = FieldMatrix::new(n, n, zero);
            for i in 0..l.rows() {
                for j in 0..l.cols() {
                    lf.set(i, j, l.get(i, j));
                }
            }
            for j in l.cols()..n {
                lf.set(j, j, one);
            }
            lf
        };
        let ns_pad_l = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        let mut p_t = {
            let zero = a.get(0, 0).zero_like();
            let one = zero.one_like();
            let mut pt = FieldMatrix::new(n, n, zero);
            for (i, &src) in p.indices().iter().enumerate() {
                pt.set(src, i, one);
            }
            pt
        };
        let ns_build_pt = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        trsm_lower(l_full.submat(.., ..), p_t.submat_mut(.., ..));
        let ns_trsm = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        let _e_full = {
            let zero = a.get(0, 0).zero_like();
            let r = e.rows();
            let mut ef = FieldMatrix::new(n, n, zero);
            for i in 0..r {
                for j in 0..n {
                    ef.set(i, j, e.get(i, j));
                }
            }
            ef
        };
        let ns_build_e = t.elapsed().as_nanos();
        eprintln!("decomp,echelon,ple,GF(251),{n},{regime},{trial},{ns_ple}");
        eprintln!("decomp,echelon,pad_l_full,GF(251),{n},{regime},{trial},{ns_pad_l}");
        eprintln!("decomp,echelon,build_pt,GF(251),{n},{regime},{trial},{ns_build_pt}");
        eprintln!("decomp,echelon,trsm_lower,GF(251),{n},{regime},{trial},{ns_trsm}");
        eprintln!("decomp,echelon,build_e_full,GF(251),{n},{regime},{trial},{ns_build_e}");
    }

    /// Times the phases of the blocked `inv` route.
    fn decompose_invert<const P: u64>(a: &FieldMatrix<Fp<P>>, regime: &str, trial: usize) {
        let n = a.rows();
        let t = std::time::Instant::now();
        let (perm, l, e, rank) = a.ple();
        let ns_ple = t.elapsed().as_nanos();
        if rank < n {
            eprintln!("decomp,invert,ple,GF(251),{n},{regime},{trial},{ns_ple}");
            eprintln!("decomp,invert,rank_deficient_early_exit,GF(251),{n},{regime},{trial},0");
            return;
        }
        let t = std::time::Instant::now();
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut y = FieldMatrix::new(n, n, zero);
        for i in 0..n {
            y.set(i, i, one);
        }
        let ns_build_i = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        trsm_lower(l.submat(.., ..), y.submat_mut(.., ..));
        let ns_trsm_lower = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        trsm_upper(e.submat(.., ..), y.submat_mut(.., ..));
        let ns_trsm_upper = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        let perm_idx = perm.indices();
        let mut out = FieldMatrix::new(n, n, zero);
        for i in 0..n {
            for (j, &src_col) in perm_idx.iter().enumerate() {
                out.set(i, j, y.get(i, src_col));
            }
        }
        let ns_col_perm = t.elapsed().as_nanos();
        eprintln!("decomp,invert,ple,GF(251),{n},{regime},{trial},{ns_ple}");
        eprintln!("decomp,invert,build_identity,GF(251),{n},{regime},{trial},{ns_build_i}");
        eprintln!("decomp,invert,trsm_lower,GF(251),{n},{regime},{trial},{ns_trsm_lower}");
        eprintln!("decomp,invert,trsm_upper,GF(251),{n},{regime},{trial},{ns_trsm_upper}");
        eprintln!("decomp,invert,col_perm,GF(251),{n},{regime},{trial},{ns_col_perm}");
    }

    /// Times the phases of `solve`.
    fn decompose_solve<const P: u64>(
        a: &FieldMatrix<Fp<P>>,
        b: &FieldVec<Fp<P>>,
        regime: &str,
        trial: usize,
    ) {
        let n = a.rows();
        let t = std::time::Instant::now();
        let zero = b.get(0).zero_like();
        let mut b_mat = FieldMatrix::new(n, 1, zero);
        for i in 0..n {
            b_mat.set(i, 0, *b.get(i));
        }
        let ns_wrap = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        let (perm, l, e, rank) = a.ple();
        let ns_ple = t.elapsed().as_nanos();
        if rank < n {
            eprintln!("decomp,solve,wrap_b,GF(251),{n},{regime},{trial},{ns_wrap}");
            eprintln!("decomp,solve,ple,GF(251),{n},{regime},{trial},{ns_ple}");
            eprintln!("decomp,solve,rank_deficient_early_exit,GF(251),{n},{regime},{trial},0");
            return;
        }
        let t = std::time::Instant::now();
        let mut y = perm.inverse().apply(&b_mat);
        let ns_perm = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        if <Fp<P> as FiniteField>::has_simd_gemm_classical() && n >= TRSM_BLOCKED_PANEL_SIZE {
            trsm_lower_blocked(
                l.submat(.., ..),
                y.submat_mut(.., ..),
                TRSM_BLOCKED_PANEL_SIZE,
            );
        } else {
            trsm_lower(l.submat(.., ..), y.submat_mut(.., ..));
        }
        let ns_trsm_lower = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        if <Fp<P> as FiniteField>::has_simd_gemm_classical() && n >= TRSM_BLOCKED_PANEL_SIZE {
            trsm_upper_blocked(
                e.submat(.., ..),
                y.submat_mut(.., ..),
                TRSM_BLOCKED_PANEL_SIZE,
            );
        } else {
            trsm_upper(e.submat(.., ..), y.submat_mut(.., ..));
        }
        let ns_trsm_upper = t.elapsed().as_nanos();
        let t = std::time::Instant::now();
        let mut x = FieldVec::zeros_from(n, b.get(0));
        for i in 0..n {
            x.set(i, y.get(i, 0));
        }
        let ns_unwrap = t.elapsed().as_nanos();
        eprintln!("decomp,solve,wrap_b,GF(251),{n},{regime},{trial},{ns_wrap}");
        eprintln!("decomp,solve,ple,GF(251),{n},{regime},{trial},{ns_ple}");
        eprintln!("decomp,solve,row_perm,GF(251),{n},{regime},{trial},{ns_perm}");
        eprintln!("decomp,solve,trsm_lower,GF(251),{n},{regime},{trial},{ns_trsm_lower}");
        eprintln!("decomp,solve,trsm_upper,GF(251),{n},{regime},{trial},{ns_trsm_upper}");
        eprintln!("decomp,solve,unwrap,GF(251),{n},{regime},{trial},{ns_unwrap}");
    }

    /// Prints per-phase and whole-operation wall times as CSV on stderr.
    #[test]
    #[ignore = "slow: GF(251)/n=64 cost decomposition for d36cc414 (~10 s)"]
    fn test_gf251_n64_borderline_cost_decomposition() {
        const ECHELON_CELLS: &[(usize, &str)] = &[(64, "deficient")];
        const INVERT_CELLS: &[(usize, &str)] = &[(64, "uniform"), (256, "deficient")];
        const SOLVE_CELLS: &[(usize, &str)] = &[(64, "uniform"), (64, "deficient")];

        eprintln!("--- d36cc414-decomp BEGIN ---");
        eprintln!("op,phase,field,n,regime,trial,wall_ns");

        for &(n, regime) in ECHELON_CELLS {
            let a = build_borderline_input::<251>(n, regime);
            // Warm up
            for _ in 0..3 {
                let _ = a.row_echelon();
            }
            for trial in 1..=5 {
                decompose_echelon::<251>(&a, regime, trial);
            }
        }
        for &(n, regime) in INVERT_CELLS {
            let a = build_borderline_input::<251>(n, regime);
            for _ in 0..3 {
                let _ = a.inv();
            }
            for trial in 1..=5 {
                decompose_invert::<251>(&a, regime, trial);
            }
        }
        for &(n, regime) in SOLVE_CELLS {
            let a = build_borderline_input::<251>(n, regime);
            let zero = a.get(0, 0).zero_like();
            let mut b = FieldVec::zeros_from(n, &zero);
            for i in 0..n {
                b.set(i, Fp::<251>::new(((i as u64).wrapping_mul(13) + 7) % 251));
            }
            for _ in 0..3 {
                let _ = a.solve(&b);
            }
            for trial in 1..=5 {
                decompose_solve::<251>(&a, &b, regime, trial);
            }
        }
        eprintln!("--- d36cc414-decomp END ---");

        eprintln!("--- d36cc414-whole BEGIN ---");
        eprintln!("op,field,n,regime,trial,wall_ns");
        for &(n, regime) in ECHELON_CELLS {
            let a = build_borderline_input::<251>(n, regime);
            for _ in 0..3 {
                let _ = a.row_echelon();
            }
            for trial in 1..=5 {
                let t = std::time::Instant::now();
                let _ = a.row_echelon();
                let ns = t.elapsed().as_nanos();
                eprintln!("echelon,GF(251),{n},{regime},{trial},{ns}");
            }
        }
        for &(n, regime) in INVERT_CELLS {
            let a = build_borderline_input::<251>(n, regime);
            for _ in 0..3 {
                let _ = a.inv();
            }
            for trial in 1..=5 {
                let t = std::time::Instant::now();
                let _ = a.inv();
                let ns = t.elapsed().as_nanos();
                eprintln!("invert,GF(251),{n},{regime},{trial},{ns}");
            }
        }
        for &(n, regime) in SOLVE_CELLS {
            let a = build_borderline_input::<251>(n, regime);
            let zero = a.get(0, 0).zero_like();
            let mut b = FieldVec::zeros_from(n, &zero);
            for i in 0..n {
                b.set(i, Fp::<251>::new(((i as u64).wrapping_mul(13) + 7) % 251));
            }
            for _ in 0..3 {
                let _ = a.solve(&b);
            }
            for trial in 1..=5 {
                let t = std::time::Instant::now();
                let _ = a.solve(&b);
                let ns = t.elapsed().as_nanos();
                eprintln!("solve,GF(251),{n},{regime},{trial},{ns}");
            }
        }
        eprintln!("--- d36cc414-whole END ---");
    }

    #[test]
    fn test_det_n_zero_returns_one_fp7() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let d = a.det();
        assert_eq!(d, Fp::<7>::new(1), "det(0×0) must equal 1");
    }

    #[test]
    fn test_solve_batch_n_zero_returns_empty_solution_fp7() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let b = FieldMatrix::<Fp<7>>::zeros(0, 3);
        let x = a
            .solve_batch(&b)
            .expect("0×0 system has a unique empty solution");
        assert_eq!(x.rows(), 0);
        assert_eq!(x.cols(), 3);
    }

    #[test]
    fn test_solve_batch_k_zero_returns_empty_solution_fp7() {
        let a = random_fp_invertible::<7>(4, 0xEE01);
        let b = FieldMatrix::<Fp<7>>::zeros(4, 0);
        let x = a
            .solve_batch(&b)
            .expect("invertible A, k=0 has a unique empty solution");
        assert_eq!(x.rows(), 4);
        assert_eq!(x.cols(), 0);
    }
}
