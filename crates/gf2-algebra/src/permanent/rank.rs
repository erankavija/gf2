//! Permanental rank deficiency for rectangular matrices over a finite field.
//!
//! # What the predicate decides
//!
//! The **permanental rank** of a matrix is the largest `r` for which some
//! `r × r` submatrix has nonzero permanent. For an `n × k` matrix `A` with
//! `k ≤ n` the permanental rank is at most `k`, and every `k × k` submatrix
//! uses all `k` columns together with a `k`-subset of the rows. Hence
//!
//! ```text
//! per-rank(A) < k  <=>  every k × k row submatrix of A has zero permanent,
//! ```
//!
//! so deciding the event is a **conjunction over the `C(n, k)` row subsets**,
//! not the evaluation of a single number. [`permanental_rank_status`] walks
//! those subsets and stops at the first nonzero `k × k` permanent.
//!
//! # A vanishing rectangular permanent is a different quantity
//!
//! The scalar "rectangular permanent" of an `n × k` matrix — the sum over all
//! injections from the `k` columns into the `n` rows — is **not** the quantity
//! this module tests. Expanding that sum by which rows it uses gives
//!
//! ```text
//! rect-per(A) = sum over k-subsets S of rows of  perm(A_S),
//! ```
//!
//! a sum of exactly the `C(n, k)` submatrix permanents whose *individual*
//! vanishing the rank condition asks about. A sum can vanish through
//! cancellation while every summand is nonzero, so `rect-per(A) = 0` neither
//! implies nor is implied by `per-rank(A) < k`. Over `F_3`,
//!
//! ```text
//! A = [[1, 0],
//!      [0, 1],
//!      [1, 1]]
//! ```
//!
//! has all three `2 × 2` row-submatrix permanents equal to `1`, so
//! `per-rank(A) = 2` is full, while `rect-per(A) = 1 + 1 + 1 = 0 mod 3`.
//! The integration test `test_rectangular_permanent_vanishes_but_submatrix_does_not`
//! in `tests/permanental_rank.rs` pins both quantities on this matrix.
//!
//! # Scope of validation
//!
//! The theorem that motivates the event — `@/citation/GGK2025`, "Ghasemi,
//! Gross, Kopparty — Permanental Rank versus Determinantal Rank of Random
//! Matrices over Finite Fields, APPROX/RANDOM 2025" — hypothesises
//! `k ≤ 0.1 · sqrt(n)`. Even `k = 3` then needs `n ≥ 900`, where the
//! deficiency probability is about `3 · 3^-900`: no Monte Carlo campaign
//! observes a single event. Every `(n, k)` pair a sampling campaign can reach
//! therefore lies **outside** that hypothesis, so agreement between this
//! predicate, an independent brute-force oracle, and the `k / q^n` heuristic
//! supports the implementation and the heuristic. It is not evidence about
//! the theorem in its proven range.
//!
//! # No statistical machinery
//!
//! This module depends on [`gf2_core::field::FiniteField`] and on the crate's
//! own [`permanent_ryser`] square kernel, and on nothing else. It decides one
//! matrix and reports nothing about rates; estimating
//! `Pr[per-rank(A) < k]` over a sample belongs to a campaign driver, which
//! calls this predicate rather than living beside it.

use gf2_core::field::FiniteField;

use crate::permanent::permanent_ryser;

/// Whether an `n × k` matrix with `k ≤ n` attains permanental rank `k`.
///
/// The two variants are the closed vocabulary of the decision that
/// [`permanental_rank_status`] and its brute-force oracle return; comparing
/// the two implementations compares values of this type.
///
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::PermanentalRank;
///
/// assert!(PermanentalRank::Deficient.is_deficient());
/// assert!(!PermanentalRank::Full.is_deficient());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PermanentalRank {
    /// `per-rank(A) < k`: every `k × k` row submatrix has zero permanent.
    Deficient,
    /// `per-rank(A) == k`: at least one `k × k` row submatrix has nonzero
    /// permanent.
    Full,
}

impl PermanentalRank {
    /// `true` for [`PermanentalRank::Deficient`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::permanent::PermanentalRank;
    ///
    /// assert_eq!(PermanentalRank::Deficient.is_deficient(), true);
    /// ```
    #[inline]
    pub const fn is_deficient(self) -> bool {
        matches!(self, Self::Deficient)
    }
}

/// Decide whether the `n × k` matrix `matrix` (with `k ≤ n`) has permanental
/// rank below `k`.
///
/// Enumerates the `C(n, k)` row subsets in lexicographic order, evaluates the
/// permanent of each `k × k` row submatrix with [`permanent_ryser`], and
/// returns [`PermanentalRank::Full`] at the **first nonzero permanent**. Only
/// when every subset yields zero is the answer [`PermanentalRank::Deficient`].
/// See the module documentation for why this conjunction — and not a scalar
/// rectangular permanent — is the quantity of interest.
///
/// # Arguments
///
/// * `matrix` — flat row-major slice of `n * k` field elements;
///   `matrix[i * k + j]` is the entry at row `i`, column `j`.
/// * `n` — number of rows.
/// * `k` — number of columns; must satisfy `k <= n`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::{permanental_rank_status, PermanentalRank};
/// use gf2_core::gfp::Fp;
///
/// // A 3x2 matrix over F_3 whose last row is the sum of the first two.
/// // Every 2x2 row submatrix has permanent 1, so the rank is full.
/// let a: Vec<Fp<3>> = [1, 0, 0, 1, 1, 1]
///     .iter()
///     .map(|&v| Fp::<3>::new(v))
///     .collect();
/// assert_eq!(permanental_rank_status::<Fp<3>>(&a, 3, 2), PermanentalRank::Full);
///
/// // A zero column forces every 2x2 submatrix permanent to vanish.
/// let b: Vec<Fp<3>> = [1, 0, 2, 0, 1, 0]
///     .iter()
///     .map(|&v| Fp::<3>::new(v))
///     .collect();
/// assert_eq!(
///     permanental_rank_status::<Fp<3>>(&b, 3, 2),
///     PermanentalRank::Deficient
/// );
/// ```
///
/// # Panics
///
/// Panics if `k > n`: permanental rank `k` is unattainable when the matrix has
/// fewer rows than columns, so the caller has passed the shape transposed.
///
/// Panics if `matrix.len() != n * k`.
///
/// Panics if `k > 63`, inherited from [`permanent_ryser`], whose Gray-code
/// subset register is a single `u64`.
///
/// # Complexity
///
/// `O(C(n, k) · k · 2^k)` field operations in the worst case — the all-zero
/// matrix, where no subset exits early — and `O(k^2)` extra space for the
/// submatrix buffer. The early exit dominates in practice: a nonzero
/// permanent is the overwhelmingly common case, so typical cost is a small
/// constant number of `k × k` permanents rather than `C(n, k)` of them.
///
/// `k = 0` returns [`PermanentalRank::Full`]: the single `0 × 0` submatrix has
/// permanent `1`, so `per-rank(A) = 0` and `0 < 0` is false.
pub fn permanental_rank_status<F: FiniteField>(
    matrix: &[F],
    n: usize,
    k: usize,
) -> PermanentalRank {
    assert!(
        k <= n,
        "permanental_rank_status: k ({k}) must not exceed n ({n}); permanental rank k needs \
         at least k rows, so a k > n shape is a transposed argument",
    );
    assert_eq!(
        matrix.len(),
        n * k,
        "permanental_rank_status: matrix.len() ({}) must equal n * k ({}) where n = {n}, k = {k}",
        matrix.len(),
        n * k,
    );

    todo!("row-subset enumeration over permanent_ryser lands in the next commit")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::field::ConstField;
    use gf2_core::gfp::Fp;

    /// Build a flat row-major `n × k` matrix over `F_P` from residue literals.
    fn matrix<const P: u64>(values: &[u64]) -> Vec<Fp<P>> {
        values.iter().map(|&v| Fp::<P>::new(v)).collect()
    }

    /// `k > n` is a transposed shape and panics rather than silently deciding.
    #[test]
    #[should_panic(expected = "k (3) must not exceed n (2)")]
    fn test_panics_when_k_exceeds_n() {
        let a = matrix::<3>(&[1, 0, 0, 0, 1, 0]);
        let _ = permanental_rank_status::<Fp<3>>(&a, 2, 3);
    }

    /// A slice whose length is not `n * k` panics.
    #[test]
    #[should_panic(expected = "matrix.len() (5) must equal n * k (6)")]
    fn test_panics_on_shape_mismatch() {
        let a = matrix::<3>(&[1, 0, 0, 1, 1]);
        let _ = permanental_rank_status::<Fp<3>>(&a, 3, 2);
    }

    /// `k > 63` reaches `permanent_ryser`'s Gray-code register bound.
    #[test]
    #[should_panic(expected = "exceeds the single-u64 Gray-code register's n <= 63 bound")]
    fn test_panics_when_k_exceeds_gray_register() {
        let a = vec![Fp::<3>::one(); 64 * 64];
        let _ = permanental_rank_status::<Fp<3>>(&a, 64, 64);
    }

    /// `k = 0`: the single empty submatrix has permanent 1, so `0 < 0` fails
    /// and the rank is full.
    #[test]
    fn test_k_zero_is_full() {
        assert_eq!(
            permanental_rank_status::<Fp<3>>(&[], 4, 0),
            PermanentalRank::Full,
            "the empty submatrix has permanent 1, so per-rank(A) = 0 is not < 0"
        );
    }

    /// The predicate exits at the first nonzero permanent (REQ-01).
    ///
    /// The matrix is `32 × 16` over `F_3` whose first sixteen rows are the
    /// identity, so the lexicographically first row subset `{0, ..., 15}` has
    /// permanent `1`. This test terminates only because of that early exit: a
    /// full scan would evaluate `C(32, 16) = 601 080 390` submatrix
    /// permanents of `2^16` Gray steps each, roughly `6 · 10^14` field
    /// operations, which the fast tier's five-second per-test kill would cut
    /// off by many orders of magnitude.
    #[test]
    fn test_exits_at_first_nonzero_permanent() {
        let n = 32;
        let k = 16;
        let mut a = vec![Fp::<3>::zero(); n * k];
        for i in 0..k {
            a[i * k + i] = Fp::<3>::one();
        }
        assert_eq!(
            permanental_rank_status::<Fp<3>>(&a, n, k),
            PermanentalRank::Full,
            "the leading identity block witnesses full permanental rank at the first subset"
        );
    }
}
