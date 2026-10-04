//! Permanental rank deficiency for rectangular matrices over a finite field.
//!
//! The permanental rank of a matrix is the largest `r` for which some
//! `r × r` submatrix has nonzero permanent. For an `n × k` matrix `A` with
//! `k ≤ n`,
//!
//! ```text
//! per-rank(A) < k  <=>  every k × k row submatrix of A has zero permanent,
//! ```
//!
//! a conjunction over the `C(n, k)` row subsets. [`permanental_rank_status`]
//! walks those subsets and stops at the first nonzero `k × k` permanent. The
//! event is the one studied in `@/citation/GGK2025`, whose theorem
//! hypothesises `k ≤ 0.1 · sqrt(n)`.

use gf2_core::field::FiniteField;

use crate::permanent::permanent_ryser;

/// Whether an `n × k` matrix with `k ≤ n` attains permanental rank `k`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PermanentalRank {
    /// `per-rank(A) < k`: every `k × k` row submatrix has zero permanent.
    Deficient,
    /// `per-rank(A) == k`: at least one `k × k` row submatrix has nonzero
    /// permanent.
    Full,
}

/// The result of a permanental-rank decision together with its evaluation cost.
///
/// `permanent_evaluations` counts the `k × k` row-submatrix permanents that
/// the production predicate actually evaluates. It includes the final
/// nonzero witness when the decision is [`PermanentalRank::Full`], and equals
/// `C(n, k)` for a deficient matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermanentalRankEvaluation {
    /// Whether the matrix has permanental rank below `k`.
    pub status: PermanentalRank,
    /// Number of `k × k` permanent evaluations performed by the predicate.
    pub permanent_evaluations: usize,
}

impl PermanentalRank {
    /// `true` for [`PermanentalRank::Deficient`].
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
///
/// # Arguments
///
/// * `matrix` — flat row-major slice of `n * k` field elements;
///   `matrix[i * k + j]` is the entry at row `i`, column `j`.
/// * `n` — number of rows.
/// * `k` — number of columns; must satisfy `k <= n`.
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
/// submatrix buffer.
///
/// `k = 0` returns [`PermanentalRank::Full`]: the single `0 × 0` submatrix has
/// permanent `1`, so `per-rank(A) = 0` and `0 < 0` is false.
pub fn permanental_rank_status<F: FiniteField>(
    matrix: &[F],
    n: usize,
    k: usize,
) -> PermanentalRank {
    permanental_rank_status_with_stats::<F>(matrix, n, k).status
}

/// Decide permanental rank and report the number of square permanent
/// evaluations used by the production predicate.
///
/// The traversal and decision are those of [`permanental_rank_status`], which
/// states the matrix representation, panics, and complexity contract.
#[must_use]
pub fn permanental_rank_status_with_stats<F: FiniteField>(
    matrix: &[F],
    n: usize,
    k: usize,
) -> PermanentalRankEvaluation {
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

    // The empty submatrix has permanent 1, which is nonzero, so per-rank is 0
    // and the strict inequality `0 < 0` fails.
    if k == 0 {
        return PermanentalRankEvaluation {
            status: PermanentalRank::Full,
            permanent_evaluations: 0,
        };
    }

    // `rows` holds the current k-subset in strictly increasing order, starting
    // at the lexicographically first subset {0, 1, ..., k-1}.
    let mut rows: Vec<usize> = (0..k).collect();
    let mut submatrix: Vec<F> = Vec::with_capacity(k * k);
    let mut permanent_evaluations = 0;

    loop {
        submatrix.clear();
        for &row in &rows {
            submatrix.extend_from_slice(&matrix[row * k..(row + 1) * k]);
        }

        // Early exit: one nonzero k × k permanent already witnesses per-rank k.
        permanent_evaluations += 1;
        if !permanent_ryser::<F>(&submatrix, k).is_zero() {
            return PermanentalRankEvaluation {
                status: PermanentalRank::Full,
                permanent_evaluations,
            };
        }

        // Advance to the next subset in lexicographic order: find the
        // rightmost position that has not yet reached its ceiling `i + n - k`,
        // bump it, and repack every position to its right.
        let Some(pivot) = (0..k).rev().find(|&i| rows[i] != i + n - k) else {
            // Every subset was exhausted with a zero permanent.
            return PermanentalRankEvaluation {
                status: PermanentalRank::Deficient,
                permanent_evaluations,
            };
        };
        rows[pivot] += 1;
        for i in pivot + 1..k {
            rows[i] = rows[i - 1] + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::field::ConstField;
    use gf2_core::gfp::Fp;

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
        assert_eq!(
            permanental_rank_status_with_stats::<Fp<3>>(&[], 4, 0),
            PermanentalRankEvaluation {
                status: PermanentalRank::Full,
                permanent_evaluations: 0,
            }
        );
    }

    #[test]
    fn test_evaluation_stats_match_early_exit_and_full_scan() {
        let mut full = vec![Fp::<3>::zero(); 4 * 2];
        full[0] = Fp::<3>::one();
        full[3] = Fp::<3>::one();
        assert_eq!(
            permanental_rank_status_with_stats::<Fp<3>>(&full, 4, 2),
            PermanentalRankEvaluation {
                status: PermanentalRank::Full,
                permanent_evaluations: 1,
            }
        );

        let deficient = vec![Fp::<3>::zero(); 4 * 2];
        assert_eq!(
            permanental_rank_status_with_stats::<Fp<3>>(&deficient, 4, 2),
            PermanentalRankEvaluation {
                status: PermanentalRank::Deficient,
                permanent_evaluations: 6,
            }
        );
    }

    /// The predicate exits at the first nonzero permanent.
    ///
    /// The matrix is `32 × 16` over `F_3` whose first sixteen rows are the
    /// identity, so the lexicographically first row subset `{0, ..., 15}` has
    /// permanent `1`. A full scan evaluates `C(32, 16) = 601 080 390`
    /// submatrix permanents of `2^16` Gray steps each.
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
