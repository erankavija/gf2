//! Generic `permanent_ryser<F>` driver — field-generic Ryser-formula permanent.
//!
//! Implements Ryser's inclusion-exclusion formula in Gray-code subset order,
//! giving an `O(n · 2^n)` algorithm that is exact over any `FiniteField`. The
//! Gray-code walk reduces each subset's column-sum update to a single element
//! add or subtract per row, matching the pseudocode in
//! `dev/plans/ae82bd73-gf2-algebra-permanent/gf2_algebra_permanent.md` §6 / §7.3.
//!
//! This module is the **correctness oracle** for the fast bipedal kernels
//! (`permanent_bipedal3`, `permanent_bipedal5`, `permanent_bipedal7`) that land
//! in later waves. Performance is intentionally secondary: no SIMD, no rayon,
//! no specialisation.

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
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::permanent_ryser;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_7: permanent = 1·1 + 0·0 = 1
/// let id: Vec<Fp<7>> = vec![
///     Fp::<7>::new(1), Fp::<7>::new(0),
///     Fp::<7>::new(0), Fp::<7>::new(1),
/// ];
/// assert_eq!(permanent_ryser::<Fp<7>>(&id, 2), Fp::<7>::new(1));
///
/// // 2×2 all-ones over F_5: permanent = 1+1 = 2 = 2! mod 5
/// let ones: Vec<Fp<5>> = vec![Fp::<5>::new(1); 4];
/// assert_eq!(permanent_ryser::<Fp<5>>(&ones, 2), Fp::<5>::new(2));
/// ```
///
/// # Panics
///
/// Panics if `matrix.len() != n * n`.
///
/// Panics if `n > 63`. The Gray-code subset enumerator
/// [`crate::gray::gray_code_iter`] uses a single-`u64` register and is
/// only well-defined for `n ≤ 63` (Rust's shift-by-full-type-width
/// `1u64 << 64` is undefined behaviour). The `n` range that this driver
/// is intended to serve — exhaustive cross-checks for `n ≤ 16` plus
/// up-to-`n = 32` correctness comparisons against bipedal kernels — sits
/// well within the 63 bound; multi-word streaming for `n > 63` is the
/// scope of W3-T14 and uses a separate driver.
///
/// Also panics when `n == 0` if `F::zero_hint()` returns `None`. All
/// `ConstField` types (every concrete `FiniteField` impl in this
/// workspace, including `Fp<P>`, `QuadraticExt`, `CubicExt`, `Gf2mElement`,
/// `Gf2mWide`) return `Some` from `zero_hint`, so the `n == 0` branch
/// works for every shipped field. The panic is reachable only for
/// hypothetical runtime-context `FiniteField` types whose zero element
/// is not derivable without a field-context handle. For such fields,
/// callers should special-case `n == 0` upstream.
///
/// # Complexity
///
/// `O(n · 2^n)` field operations, `O(n)` extra space for the column-sum
/// accumulators. No heap allocation beyond the `col_sum` vector. Intended
/// for `n ≤ 16` exhaustive cross-checks; larger `n` (up to 63) are
/// mathematically correct but require `2^n` Gray steps.
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

    // Edge case: the 0×0 matrix has exactly one permutation (the empty one),
    // whose product over an empty index set is the vacuous product 1.
    //
    // For n == 0, the matrix slice is empty so we cannot bootstrap a field
    // element from it.  `FiniteField::zero_hint()` returns `Some(zero)` for
    // every `ConstField` (all prime fields and GF(2^m) constant fields), which
    // covers every realistic caller.  Runtime-context fields (e.g. a
    // dynamically-configured `Gf2mElement`) cannot produce a zero without a
    // field witness and should pass n ≥ 1 matrices.
    if n == 0 {
        return F::zero_hint()
            .expect(
                "permanent_ryser: n == 0 requires a field with a static zero (ConstField or \
                 FiniteField::zero_hint returning Some); runtime-context fields must pass n ≥ 1",
            )
            .one_like();
    }

    // Bootstrap identity elements from the first matrix entry.  For n ≥ 1 the
    // slice is non-empty, so no ConstField bound is needed.
    let zero = matrix[0].zero_like();
    let one = matrix[0].one_like();

    // col_sum[i] accumulates sum_{j ∈ S} A[i, j] for the current subset S.
    // Starts at zero (empty subset, which is excluded from the Ryser sum).
    let mut col_sum: Vec<F> = (0..n).map(|_| zero.clone()).collect();
    let mut total = zero;

    // Track |S| (popcount of the current Gray-code subset register) as a
    // plain usize. The gray_code_iter parity invariant guarantees that the
    // running sum of parity values equals popcount(g_k) at every step, so
    // incrementing/decrementing here stays in sync with the Gray walk.
    let mut subset_size: usize = 0;

    // Walk all 2^n - 1 non-empty subsets in Gray-code order.
    // gray_code_iter(n) yields (flip, parity):
    //   flip   — index of the column that just toggled (entered or left S)
    //   parity — +1 if the column just entered S (ADD), -1 if it just left (SUB)
    //
    // The parity is derived inside gray_code_iter from g_k = k ^ (k >> 1):
    // if bit `flip` of g_k is 1, parity = +1; if 0, parity = -1.
    // This is correct. The trap — testing (k >> flip) & 1 — is avoided because
    // gray_code_iter already resolves the sign correctly using g_k, not k.
    for (flip, parity) in gray_code_iter(n) {
        // Update col_sum[i] and subset_size.
        if parity == 1 {
            subset_size += 1;
            for i in 0..n {
                // AddAssign<&F> avoids cloning matrix entries.
                col_sum[i] += &matrix[i * n + flip];
            }
        } else {
            subset_size -= 1;
            for i in 0..n {
                // FiniteField provides Sub<&F>; clone col_sum[i] by value so
                // we can pass a borrow of matrix entry on the right-hand side.
                // One clone per inner-loop iteration; for Fp<P> this is a u64 copy.
                col_sum[i] = col_sum[i].clone() - &matrix[i * n + flip];
            }
        }

        // Compute term = prod_{i=0}^{n-1} col_sum[i].
        // Use Mul<&F> to avoid consuming col_sum entries; x is &F from the iterator.
        let term = col_sum.iter().fold(one.clone(), |p, x| p * x);

        // Ryser sign for this subset: (-1)^|S|.
        // Odd |S| → contribution is -term; even |S| → +term.
        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total += term;
        }
    }

    // Apply the outer (-1)^n factor from Ryser's formula.
    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::random_matrix;
    use gf2_core::field::{ConstField, FiniteField};
    use gf2_core::gfp::Fp;

    // -----------------------------------------------------------------------
    // Naive reference: sum over all n! permutations (Heap's algorithm)
    // -----------------------------------------------------------------------

    /// Compute the permanent by enumerating all `n!` permutations.
    ///
    /// Uses Heap's algorithm (iterative) to visit each permutation in O(1)
    /// amortised time per swap, accumulating `prod_{i} A[i, sigma(i)]` into a
    /// running field sum. For `n ≤ 8` this is at most 40 320 permutations —
    /// trivially fast in release mode. Not intended for `n > 10`.
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

        // Evaluate the initial permutation (identity).
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
                // Evaluate this permutation.
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

    // Deterministic pseudo-random matrix generator lives in
    // `crate::testutil::random_matrix` (SSOT for all permanent_* cross-check
    // tests in this crate); imported above.

    // -----------------------------------------------------------------------
    // Unit tests
    // -----------------------------------------------------------------------

    /// `permanent_ryser` panics when `n > 63` (Gray-code register bound).
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

    /// Identity matrix `I_n` has permanent = 1 (exactly one permutation with all
    /// diagonal entries = 1, all others 0).
    ///
    /// Tested for `n ∈ {1, 2, 3, 4, 5}` over `Fp<3>`, `Fp<5>`, `Fp<7>`.
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

    /// All-ones `n×n` matrix has permanent = `n!` (there are `n!` permutations,
    /// each contributing a product of `n` ones).
    ///
    /// `n!` is computed in the field to account for reductions modulo `P`:
    /// `(1..=n).fold(F::one(), |a, k| a * F::new(k as u64))`.
    ///
    /// Tested for `n ∈ {1, 2, 3, 4, 5}` over `Fp<3>`, `Fp<5>`, `Fp<7>`.
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

    // -----------------------------------------------------------------------
    // Cross-checks: permanent_ryser vs naive_permanent_factorial
    // -----------------------------------------------------------------------

    /// Cross-check `permanent_ryser` against `naive_permanent_factorial` for 100
    /// random matrices per `(n, F)` combination.
    ///
    /// Covers `n ∈ {1, 2, 3, 4, 5}` × `F ∈ {Fp<3>, Fp<5>, Fp<7>}` = 15
    /// combinations, 100 matrices each = 1 500 independent cross-checks.
    /// Seeds are derived deterministically from `(n, P)` to ensure reproducibility.
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

    /// Cross-check for `n = 8` over `Fp<3>`.
    ///
    /// Exercises the full Gray walk of 255 steps and verifies correctness at a
    /// larger `k` range (trailing_zeros up to 7). Uses one deterministic matrix;
    /// `8! = 40 320` permutations is fast in release mode.
    ///
    /// This is the word-boundary correctness test called for in the T7 success
    /// criteria: `permanent_ryser` for `n = 8` uses `2^8 - 1 = 255` Gray steps,
    /// covering `trailing_zeros` values 0 through 7.
    #[test]
    fn test_permanent_cross_check_n8_fp3() {
        let mat = random_matrix::<3>(8, 0xdead_beef_cafe_babe);
        let ryser = permanent_ryser::<Fp<3>>(&mat, 8);
        let naive = naive_permanent_factorial::<Fp<3>>(&mat, 8);
        assert_eq!(ryser, naive, "ryser != naive for n=8 Fp<3>");
    }

    /// Diagonal-zero matrix (all diagonal entries zero, off-diagonal entries
    /// deterministically generated). Cross-checked against naive.
    #[test]
    fn test_permanent_diagonal_zero() {
        // Build a 4×4 matrix with zeroed diagonal and deterministic off-diagonal.
        let mut mat = random_matrix::<7>(4, 0xf00d_cafe);
        for i in 0..4 {
            mat[i * 4 + i] = Fp::<7>::zero();
        }
        let ryser = permanent_ryser::<Fp<7>>(&mat, 4);
        let naive = naive_permanent_factorial::<Fp<7>>(&mat, 4);
        assert_eq!(ryser, naive, "ryser != naive for diagonal-zero 4×4 Fp<7>");
    }

    // -----------------------------------------------------------------------
    // Non-ConstField path: RuntimeFp7 newtype wrapper
    // -----------------------------------------------------------------------

    /// Test-only wrapper around `Fp<7>` that impls `FiniteField` but NOT
    /// `ConstField`, used to verify `permanent_ryser`'s `<F: FiniteField>`
    /// generalisation against a non-`ConstField` `FiniteField` instance.
    ///
    /// The wrapper delegates every `FiniteField` method to the inner `Fp<7>`
    /// but does not provide `ConstField::zero()` / `ConstField::one()`.
    /// Functionally identical to `Fp<7>` for permanent computation; if
    /// `permanent_ryser::<RuntimeFp7>` returns the same value as
    /// `permanent_ryser::<Fp<7>>` on the same matrix, the FiniteField
    /// generality is proven by demonstration.
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
        // Crucially: do NOT override `zero_hint()`. Default impl returns None.
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

    /// Cross-check `permanent_ryser` against `Fp<7>` using the `RuntimeFp7`
    /// newtype that impls `FiniteField` but NOT `ConstField`.
    ///
    /// Verifies that the `<F: FiniteField>` generalisation introduced in
    /// cycle 1 is exercised by a non-`ConstField` type: both computations
    /// must agree for the same 3×3 matrix.
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

    /// Verifies the documented `n == 0` panic for `RuntimeFp7`, which returns
    /// `None` from `zero_hint()` (the default impl).
    ///
    /// `Fp<7>` (a `ConstField`) does NOT panic here because its `zero_hint()`
    /// returns `Some`. This test specifically exercises the non-`ConstField`
    /// branch to confirm the documented panic fires.
    #[test]
    fn test_permanent_ryser_runtime_field_n0_panics() {
        let result = std::panic::catch_unwind(|| permanent_ryser::<RuntimeFp7>(&[], 0));
        assert!(
            result.is_err(),
            "permanent_ryser must panic when n=0 and F::zero_hint() is None"
        );
    }
}
