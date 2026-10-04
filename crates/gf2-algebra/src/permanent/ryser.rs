//! Generic `permanent_ryser<F>` driver: Ryser's inclusion-exclusion formula in
//! Gray-code subset order, an `O(n · 2^n)` algorithm that is exact over any
//! `FiniteField`. The Gray-code walk reduces each subset's column-sum update to
//! a single element add or subtract per row. This module is the correctness
//! oracle for the bipedal permanents (`permanent_bipedal3`,
//! `permanent_bipedal5`, `permanent_bipedal7`) and uses no SIMD or rayon.

use gf2_core::field::FiniteField;

use crate::gray::gray_code_iter;

/// Compute the permanent of an `n × n` matrix over any [`FiniteField`] using
/// Ryser's formula in Gray-code subset order.
///
/// The permanent of an `n × n` matrix `A` is
///
/// ```text
/// perm(A) = sum over all permutations sigma of prod_{i=0}^{n-1} A[i, sigma(i)]
/// ```
///
/// This function evaluates it via Ryser's inclusion-exclusion formula:
///
/// ```text
/// perm(A) = (-1)^n  *  sum_{S ⊆ [n], S ≠ ∅}  (-1)^|S|  *  prod_{i=0}^{n-1}  sum_{j ∈ S} A[i,j]
/// ```
///
/// Subsets are enumerated in binary-reflected Gray-code order so that each
/// step updates only one column sum (one add or subtract per row), giving
/// `O(n · 2^n)` total field operations.
///
/// # Arguments
///
/// * `matrix` — Flat row-major slice of `n × n` field elements.
///   `matrix[i * n + j]` is the entry at row `i`, column `j`.
/// * `n` — Matrix dimension (number of rows = number of columns).
///
/// # Panics
///
/// Panics if `matrix.len() != n * n`.
///
/// Panics if `n > 63`: the Gray-code subset enumerator
/// [`crate::gray::gray_code_iter`] uses a single-`u64` register.
///
/// Panics when `n == 0` if `F::zero_hint()` returns `None`, which is the
/// trait default for a field whose zero element needs a field-context
/// handle.
///
/// # Complexity
///
/// `O(n · 2^n)` field operations, `O(n)` extra space for the column-sum
/// accumulators. No heap allocation beyond the `col_sum` vector.
pub fn permanent_ryser<F: FiniteField>(matrix: &[F], n: usize) -> F {
    assert!(
        n <= 63,
        "permanent_ryser: n = {} exceeds the single-u64 Gray-code register's n <= 63 bound; \
         use multi-word streaming (W3-T14) for n > 63",
        n,
    );
    assert_eq!(
        matrix.len(),
        n * n,
        "permanent_ryser: matrix.len() ({}) must equal n * n ({}) where n = {}",
        matrix.len(),
        n * n,
        n,
    );

    // The 0×0 matrix has one permutation (the empty one), with product 1.
    // The slice is empty, so the identity comes from `zero_hint` instead of
    // a matrix entry.
    if n == 0 {
        return F::zero_hint()
            .expect(
                "permanent_ryser: n == 0 requires a field with a static zero (ConstField or \
                 FiniteField::zero_hint returning Some); runtime-context fields must pass n ≥ 1",
            )
            .one_like();
    }

    let zero = matrix[0].zero_like();
    let one = matrix[0].one_like();

    // col_sum[i] accumulates sum_{j ∈ S} A[i, j] for the current subset S.
    let mut col_sum: Vec<F> = (0..n).map(|_| zero.clone()).collect();
    let mut total = zero;

    // |S|: the running sum of gray_code_iter parities equals popcount(g_k).
    let mut subset_size: usize = 0;

    // flip is the column that entered (parity +1) or left (parity -1) S.
    for (flip, parity) in gray_code_iter(n) {
        if parity == 1 {
            subset_size += 1;
            for i in 0..n {
                col_sum[i] += &matrix[i * n + flip];
            }
        } else {
            subset_size -= 1;
            for i in 0..n {
                // FiniteField has no SubAssign<&F>; one clone per entry.
                col_sum[i] = col_sum[i].clone() - &matrix[i * n + flip];
            }
        }

        let term = col_sum.iter().fold(one.clone(), |p, x| p * x);

        // Ryser sign: (-1)^|S|.
        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total += term;
        }
    }

    // Outer (-1)^n factor of Ryser's formula.
    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::random_matrix;
    use gf2_core::field::{ConstField, FiniteField};
    use gf2_core::gfp::Fp;

    /// Compute the permanent by enumerating all `n!` permutations with
    /// Heap's algorithm (iterative).
    fn naive_permanent_factorial<F: FiniteField>(matrix: &[F], n: usize) -> F {
        assert_eq!(matrix.len(), n * n);
        if n == 0 {
            return F::zero_hint()
                .expect("naive_permanent_factorial: n==0 needs zero_hint")
                .one_like();
        }

        let zero = matrix[0].zero_like();
        let one = matrix[0].one_like();

        let mut perm: Vec<usize> = (0..n).collect();
        let mut total = zero;
        let mut c = vec![0usize; n]; // Heap's control vector

        let mut term = one.clone();
        for i in 0..n {
            term = term * &matrix[i * n + perm[i]];
        }
        total += term;

        let mut i = 0usize;
        while i < n {
            if c[i] < i {
                if i.is_multiple_of(2) {
                    perm.swap(0, i);
                } else {
                    perm.swap(c[i], i);
                }
                let mut term = one.clone();
                for row in 0..n {
                    term = term * &matrix[row * n + perm[row]];
                }
                total += term;
                c[i] += 1;
                i = 0;
            } else {
                c[i] = 0;
                i += 1;
            }
        }

        total
    }

    #[test]
    #[should_panic(expected = "exceeds the single-u64 Gray-code register's n <= 63 bound")]
    fn test_permanent_ryser_panics_on_n_exceeding_63() {
        let matrix: Vec<Fp<3>> = vec![Fp::<3>::new(0); 64 * 64];
        let _ = permanent_ryser::<Fp<3>>(&matrix, 64);
    }

    /// The 0×0 matrix has permanent = 1 (vacuous product over the empty permutation).
    #[test]
    fn test_permanent_empty_matrix() {
        assert_eq!(
            permanent_ryser::<Fp<3>>(&[], 0),
            Fp::<3>::one(),
            "0×0 permanent should be one"
        );
    }

    /// A 1×1 matrix `[a]` has permanent = `a`.
    #[test]
    fn test_permanent_1x1() {
        for v in 0u64..3 {
            let a = Fp::<3>::new(v);
            let result = permanent_ryser::<Fp<3>>(&[a], 1);
            assert_eq!(result, a, "1×1 permanent of [{v}] should be {v}");
        }
    }

    /// Identity matrix `I_n` has permanent = 1.
    #[test]
    fn test_permanent_identity_matrix() {
        fn check_identity<const P: u64>(n: usize) {
            let mut id = vec![Fp::<P>::zero(); n * n];
            for i in 0..n {
                id[i * n + i] = Fp::<P>::one();
            }
            let result = permanent_ryser::<Fp<P>>(&id, n);
            assert_eq!(
                result,
                Fp::<P>::one(),
                "identity permanent should be one for n={n} P={P}"
            );
        }

        for n in 1..=5 {
            check_identity::<3>(n);
            check_identity::<5>(n);
            check_identity::<7>(n);
        }
    }

    /// All-ones `n×n` matrix has permanent = `n!`, computed in the field.
    #[test]
    fn test_permanent_all_ones() {
        fn check_all_ones<const P: u64>(n: usize) {
            let ones = vec![Fp::<P>::one(); n * n];
            let result = permanent_ryser::<Fp<P>>(&ones, n);
            let n_factorial: Fp<P> =
                (1..=n).fold(Fp::<P>::one(), |acc, k| acc * Fp::<P>::new(k as u64));
            assert_eq!(
                result, n_factorial,
                "all-ones permanent should be n! mod P for n={n} P={P}"
            );
        }

        for n in 1..=5 {
            check_all_ones::<3>(n);
            check_all_ones::<5>(n);
            check_all_ones::<7>(n);
        }
    }

    /// Cross-check `permanent_ryser` against `naive_permanent_factorial` for 100
    /// random matrices per `(n, F)` combination.
    #[test]
    fn test_permanent_cross_check_random_small() {
        fn cross_check<const P: u64>(n: usize, seed_base: u64) {
            for trial in 0..100u64 {
                let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                let mat = random_matrix::<P>(n, seed);
                let ryser = permanent_ryser::<Fp<P>>(&mat, n);
                let naive = naive_permanent_factorial::<Fp<P>>(&mat, n);
                assert_eq!(ryser, naive, "ryser != naive for n={n} P={P} trial={trial}");
            }
        }

        for n in 1..=5 {
            cross_check::<3>(n, 0x1234_0000u64.wrapping_add(n as u64));
            cross_check::<5>(n, 0x5678_0000u64.wrapping_add(n as u64));
            cross_check::<7>(n, 0x9abc_0000u64.wrapping_add(n as u64));
        }
    }

    /// Cross-check for `n = 8` over `Fp<3>`: `2^8 - 1 = 255` Gray steps,
    /// covering `trailing_zeros` values 0 through 7.
    #[test]
    fn test_permanent_cross_check_n8_fp3() {
        let mat = random_matrix::<3>(8, 0xdead_beef_cafe_babe);
        let ryser = permanent_ryser::<Fp<3>>(&mat, 8);
        let naive = naive_permanent_factorial::<Fp<3>>(&mat, 8);
        assert_eq!(ryser, naive, "ryser != naive for n=8 Fp<3>");
    }

    #[test]
    fn test_permanent_diagonal_zero() {
        let mut mat = random_matrix::<7>(4, 0xf00d_cafe);
        for i in 0..4 {
            mat[i * 4 + i] = Fp::<7>::zero();
        }
        let ryser = permanent_ryser::<Fp<7>>(&mat, 4);
        let naive = naive_permanent_factorial::<Fp<7>>(&mat, 4);
        assert_eq!(ryser, naive, "ryser != naive for diagonal-zero 4×4 Fp<7>");
    }

    /// Wrapper around `Fp<7>` that implements `FiniteField` but not
    /// `ConstField` and keeps the default `zero_hint`.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct RuntimeFp7(Fp<7>);

    impl RuntimeFp7 {
        fn new(v: u64) -> Self {
            Self(Fp::<7>::new(v))
        }
    }

    impl core::ops::Add for RuntimeFp7 {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }
    impl core::ops::Add<&RuntimeFp7> for RuntimeFp7 {
        type Output = Self;
        fn add(self, rhs: &Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }
    impl core::ops::Sub for RuntimeFp7 {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            Self(self.0 - rhs.0)
        }
    }
    impl core::ops::Sub<&RuntimeFp7> for RuntimeFp7 {
        type Output = Self;
        fn sub(self, rhs: &Self) -> Self {
            Self(self.0 - rhs.0)
        }
    }
    impl core::ops::Mul for RuntimeFp7 {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self {
            Self(self.0 * rhs.0)
        }
    }
    impl core::ops::Mul<&RuntimeFp7> for RuntimeFp7 {
        type Output = Self;
        fn mul(self, rhs: &Self) -> Self {
            Self(self.0 * rhs.0)
        }
    }
    impl core::ops::Div for RuntimeFp7 {
        type Output = Self;
        fn div(self, rhs: Self) -> Self {
            Self(self.0 / rhs.0)
        }
    }
    impl core::ops::Div<&RuntimeFp7> for RuntimeFp7 {
        type Output = Self;
        fn div(self, rhs: &Self) -> Self {
            Self(self.0 / rhs.0)
        }
    }
    impl core::ops::Neg for RuntimeFp7 {
        type Output = Self;
        fn neg(self) -> Self {
            Self(-self.0)
        }
    }
    impl core::ops::AddAssign for RuntimeFp7 {
        fn add_assign(&mut self, rhs: Self) {
            self.0 += rhs.0;
        }
    }
    impl core::ops::AddAssign<&RuntimeFp7> for RuntimeFp7 {
        fn add_assign(&mut self, rhs: &Self) {
            self.0 += &rhs.0;
        }
    }

    impl gf2_core::field::FiniteField for RuntimeFp7 {
        type Characteristic = u64;
        type Wide = u128;

        fn characteristic(&self) -> u64 {
            self.0.characteristic()
        }
        fn extension_degree(&self) -> usize {
            self.0.extension_degree()
        }
        fn is_zero(&self) -> bool {
            self.0.is_zero()
        }
        fn is_one(&self) -> bool {
            self.0.is_one()
        }
        fn inv(&self) -> Option<Self> {
            self.0.inv().map(Self)
        }
        fn zero_like(&self) -> Self {
            Self(self.0.zero_like())
        }
        fn one_like(&self) -> Self {
            Self(self.0.one_like())
        }
        // `zero_hint()` keeps its default, which returns None.
        fn to_wide(&self) -> u128 {
            self.0.to_wide()
        }
        fn mul_to_wide(&self, rhs: &Self) -> u128 {
            self.0.mul_to_wide(&rhs.0)
        }
        fn reduce_wide(wide: &u128) -> Self {
            Self(<Fp<7> as gf2_core::field::FiniteField>::reduce_wide(wide))
        }
        fn max_unreduced_additions() -> usize {
            <Fp<7> as gf2_core::field::FiniteField>::max_unreduced_additions()
        }
    }

    /// `permanent_ryser` agrees between `Fp<7>` and the non-`ConstField`
    /// `RuntimeFp7` on the same 3×3 matrix.
    #[test]
    fn test_permanent_ryser_runtime_field_3x3() {
        let entries: Vec<u64> = vec![1, 2, 3, 4, 5, 6, 0, 1, 2];
        let m_const: Vec<Fp<7>> = entries.iter().map(|&v| Fp::<7>::new(v)).collect();
        let m_runtime: Vec<RuntimeFp7> = entries.iter().map(|&v| RuntimeFp7::new(v)).collect();
        let p_const = permanent_ryser::<Fp<7>>(&m_const, 3);
        let p_runtime = permanent_ryser::<RuntimeFp7>(&m_runtime, 3);
        assert_eq!(
            p_const,
            p_runtime.0,
            "permanent_ryser must produce identical results for ConstField and FiniteField-only types"
        );
    }

    /// The documented `n == 0` panic fires for `RuntimeFp7`, whose
    /// `zero_hint()` returns `None`.
    #[test]
    fn test_permanent_ryser_runtime_field_n0_panics() {
        let result = std::panic::catch_unwind(|| permanent_ryser::<RuntimeFp7>(&[], 0));
        assert!(
            result.is_err(),
            "permanent_ryser must panic when n=0 and F::zero_hint() is None"
        );
    }
}
