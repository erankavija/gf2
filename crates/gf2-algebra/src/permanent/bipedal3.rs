//! `permanent_bipedal3` — single-`u64`-pair fast path for permanents over
//! `F_3`, restricted to matrices with `n ≤ 64`.
//!
//! For `n ≤ 64` the column-sum vector fits in a single [`Bipedal3`] (one
//! `u64` mag + one `u64` sgn pair).  Each Gray-code step is a single
//! [`Bipedal3::add`] or [`Bipedal3::sub`] against the toggled column
//! (also extracted as a `Bipedal3`), followed by a horizontal fold of
//! the `n` active lanes via scalar `Fp<3>` multiplications.
//!
//! This module is the **headline single-thread fast path** of the
//! permanent epic; the 50× speedup target is measured against
//! `permanent_mod3_reference` at `n = 36`.  Multi-word streaming for
//! `n > 64` lands in W3-T14 and lives in a separate module.
//!
//! # Algorithm reference
//!
//! `dev/plans/gf2_algebra_permanent.md` §7.3 (single-word path).

use gf2_core::gfp::Fp;

use crate::gray::gray_code_iter;
use crate::packed::bipedal3::{Bipedal3, Bipedal3Matrix};
use crate::packed::{PackedField, PackedFieldVec};

/// Compute the permanent of an `n × n` matrix over `F_3` using the
/// single-`u64` Bipedal3 fast path.
///
/// For `n ≤ 64` the column-sum vector fits in a single [`Bipedal3`]
/// (one `u64` mag + one `u64` sgn pair), so each Gray-code step
/// performs exactly one [`Bipedal3::add`] or [`Bipedal3::sub`] followed
/// by a horizontal lane-fold of `n` scalar multiplications.
///
/// The permanent of an `n × n` matrix `A` over `F_3` is:
///
/// ```text
/// perm(A) = sum over all permutations sigma of prod_{i=0}^{n-1} A[i, sigma(i)]
/// ```
///
/// Evaluated via Ryser's inclusion-exclusion formula in Gray-code order
/// (see `permanent_ryser` for the generic version):
///
/// ```text
/// perm(A) = (-1)^n * sum_{S ⊆ [n], S ≠ ∅} (-1)^|S| * prod_{i=0}^{n-1} sum_{j ∈ S} A[i,j]
/// ```
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Bipedal3Matrix`] (column-major, `rows == cols`).
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::permanent_bipedal3;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_3: permanent = 1
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// let m = Bipedal3Matrix::from_row_major(&id, 2, 2);
/// assert_eq!(permanent_bipedal3(&m), Fp::<3>::new(1));
///
/// // 2×2 all-ones over F_3: permanent = 2! mod 3 = 2
/// let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 4];
/// let m2 = Bipedal3Matrix::from_row_major(&ones, 2, 2);
/// assert_eq!(permanent_bipedal3(&m2), Fp::<3>::new(2));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > 64` (single-`u64` fast path requires `n ≤ 64`).
///
/// # Complexity
///
/// `O(n · 2^n)` field operations over `Fp<3>`:
/// - Matrix prep: `O(n^2)` one-time lane-by-lane column extraction.
/// - Gray walk: `2^n - 1` steps, each with 1 `Bipedal3` add/sub (O(1)) plus
///   1 horizontal fold of `n` `Fp<3>` multiplications.
/// - Space: `O(n)` extra (the `columns` Vec plus `col_sum`).
pub fn permanent_bipedal3(mat: &Bipedal3Matrix) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 64,
        "permanent_bipedal3: single-u64 fast path requires n <= 64; got n = {}",
        n
    );

    // Edge case: the 0×0 matrix has exactly one permutation (the empty
    // one), whose product over an empty index set is the vacuous product 1.
    if n == 0 {
        return Fp::<3>::new(1);
    }

    // One-time matrix-prep: extract each column into a single Bipedal3.
    //
    // Each column in `mat` is a Bipedal3Vec with `len_lanes = n ≤ 64`.
    // Bipedal3Vec's internal storage is private, so we reconstruct the
    // Bipedal3 lane-by-lane via `col_vec.get(i)` and `with_lane`.  This
    // is O(n) per column, O(n^2) total — dominated by the O(n · 2^n)
    // Gray walk for any n ≥ 4. Lanes n..64 remain zero (the Bipedal3 is
    // initialised to all-zeros).
    let columns: Vec<Bipedal3> = (0..n)
        .map(|j| {
            let col_vec = mat.column(j);
            let mut col_b3 = <Bipedal3 as PackedField<Fp<3>>>::zero();
            for i in 0..n {
                col_b3 = col_b3.with_lane(i, col_vec.get(i));
            }
            col_b3
        })
        .collect();

    // Column-sum accumulator: a single Bipedal3 whose active lanes 0..n
    // hold sum_{j ∈ S} A[i, j] for the current Gray-code subset S.
    // Lanes n..64 are always zero (padding bits, unaffected by our ops).
    let mut col_sum = <Bipedal3 as PackedField<Fp<3>>>::zero();

    // Running Ryser accumulator and subset-size counter.
    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // Gray walk: enumerate all 2^n - 1 non-empty subsets of [n].
    // At each step (flip, parity):
    //   flip   — which column just entered or left S
    //   parity — +1 (entered, ADD) or -1 (left, SUB)
    for (flip, parity) in gray_code_iter(n) {
        // Update col_sum: one Bipedal3 add or sub.
        if parity == 1 {
            subset_size += 1;
            col_sum = col_sum.add(columns[flip]);
        } else {
            subset_size -= 1;
            col_sum = col_sum.sub(columns[flip]);
        }

        // Horizontal fold: product of col_sum's active lanes 0..n.
        // Lanes n..64 are padded zero (by the Bipedal3Vec mask_tail
        // invariant propagated through our ops); we exclude them from
        // the fold — col_sum.lane(i) for i ≥ n would return Fp::<3>::new(0)
        // and including them would zero out every term.
        let mut term = Fp::<3>::new(1);
        for i in 0..n {
            term = term * col_sum.lane(i);
        }

        // Ryser sign: (-1)^|S|.
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
    use crate::packed::Bipedal3Matrix;
    use crate::permanent::ryser::permanent_ryser;
    use gf2_core::gfp::Fp;

    // -----------------------------------------------------------------------
    // Deterministic pseudo-random matrix generator (LCG — same as ryser.rs)
    // -----------------------------------------------------------------------

    /// Generate a deterministic pseudo-random `n×n` matrix of `Fp<3>` elements.
    ///
    /// Uses Knuth's MMIX LCG: `x_{k+1} = a * x_k + c mod 2^64`, then takes
    /// `x mod 3` as the element value. Reproducible across runs.
    fn random_matrix_fp3(n: usize, seed: u64) -> Vec<Fp<3>> {
        let mut state = seed;
        let mut out = Vec::with_capacity(n * n);
        for _ in 0..n * n {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            out.push(Fp::<3>::new(state % 3));
        }
        out
    }

    // -----------------------------------------------------------------------
    // Hand-checked vectors
    // -----------------------------------------------------------------------

    /// `permanent_bipedal3` of the 0×0 matrix is `Fp::<3>::new(1)` (vacuous product).
    #[test]
    fn test_permanent_empty_matrix() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 0);
        assert_eq!(
            permanent_bipedal3(&m),
            Fp::<3>::new(1),
            "0×0 permanent must be 1"
        );
    }

    /// A 1×1 matrix `[v]` has permanent = `v`.
    #[test]
    fn test_permanent_1x1() {
        for v in 0u64..3 {
            let row = vec![Fp::<3>::new(v)];
            let m = Bipedal3Matrix::from_row_major(&row, 1, 1);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(v),
                "1×1 permanent of [{v}] must be {v}"
            );
        }
    }

    /// `I_n` has permanent = 1 for `n ∈ {1, 2, 3, 4}`.
    #[test]
    fn test_permanent_identity_n() {
        for n in 1..=4usize {
            let mut id = vec![Fp::<3>::new(0); n * n];
            for i in 0..n {
                id[i * n + i] = Fp::<3>::new(1);
            }
            let m = Bipedal3Matrix::from_row_major(&id, n, n);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(1),
                "identity permanent must be 1 for n={n}"
            );
        }
    }

    /// All-ones `n×n` matrix: permanent = `n! mod 3` for `n ∈ {1, 2, 3, 4}`.
    ///
    /// n! mod 3: n=1 → 1, n=2 → 2, n=3 → 6 ≡ 0, n=4 → 24 ≡ 0.
    #[test]
    fn test_permanent_all_ones_n() {
        // n! mod 3: {1, 2, 0, 0}
        let expected = [1u64, 2, 0, 0];
        for n in 1..=4usize {
            let ones = vec![Fp::<3>::new(1); n * n];
            let m = Bipedal3Matrix::from_row_major(&ones, n, n);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(expected[n - 1]),
                "all-ones permanent for n={n} must be {} (= n! mod 3)",
                expected[n - 1]
            );
        }
    }

    // -----------------------------------------------------------------------
    // Panic tests
    // -----------------------------------------------------------------------

    /// Non-square matrix panics.
    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_permanent_bipedal3_panics_on_non_square() {
        let data = vec![Fp::<3>::new(0); 3 * 5];
        let m = Bipedal3Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal3(&m);
    }

    /// `n = 65` exceeds the single-u64 fast path limit and panics.
    #[test]
    #[should_panic(expected = "single-u64 fast path requires n <= 64")]
    fn test_permanent_bipedal3_panics_on_n_exceeding_64() {
        let data = vec![Fp::<3>::new(0); 65 * 65];
        let m = Bipedal3Matrix::from_row_major(&data, 65, 65);
        let _ = permanent_bipedal3(&m);
    }

    // -----------------------------------------------------------------------
    // Cross-checks: permanent_bipedal3 vs permanent_ryser (default tier)
    // -----------------------------------------------------------------------

    /// Cross-check `permanent_bipedal3` against `permanent_ryser::<Fp<3>>`
    /// on 1000 random matrices for each `n ∈ {1..=12}`.
    ///
    /// Default tier (no `#[ignore]`): fits within the 5 s per-test budget
    /// in release mode.  Seeds are derived deterministically from `n`.
    #[test]
    fn test_permanent_bipedal3_cross_check_n1_to_n12() {
        for n in 1usize..=12 {
            let seed_base: u64 = 0xb085_7ae9_0000_0000_u64.wrapping_add(n as u64);
            for trial in 0u64..1000 {
                let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                let row_major = random_matrix_fp3(n, seed);
                let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
                let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                let actual = permanent_bipedal3(&mat);
                assert_eq!(
                    actual, expected,
                    "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                );
            }
        }
    }

    /// Cross-check `permanent_bipedal3` against `permanent_ryser::<Fp<3>>`
    /// on 100 random matrices for each `n ∈ {13..=16}`.
    ///
    /// Default tier (no `#[ignore]`): `permanent_ryser` at `n = 16` runs
    /// `2^16 - 1 = 65 535` Gray steps; 100 trials keeps the test under 5 s
    /// in release mode.  Seeds are distinct from the `n ∈ {1..=12}` batch.
    #[test]
    fn test_permanent_bipedal3_cross_check_n13_to_n16() {
        for n in 13usize..=16 {
            let seed_base: u64 = 0xb085_7ae9_1000_0000_u64.wrapping_add(n as u64);
            for trial in 0u64..100 {
                let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                let row_major = random_matrix_fp3(n, seed);
                let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
                let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                let actual = permanent_bipedal3(&mat);
                assert_eq!(
                    actual, expected,
                    "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Cross-checks: large n (slow tier — must not run in default CI)
    // -----------------------------------------------------------------------

    /// Cross-check for `n ∈ {20, 24, 28, 32}` on 100 random matrices each.
    ///
    /// Marked `#[ignore]` because Ryser's reference at n=32 requires
    /// `n * 2^32 ≈ 137 billion` Fp<3> ops and far exceeds the 5 s budget.
    /// Run only under the nightly slow tier:
    /// `cargo nextest run --profile slow --run-ignored ignored-only`.
    #[test]
    #[ignore = "sim: large-n cross-check (n=20..32) exceeds 5s budget"]
    fn test_permanent_bipedal3_cross_check_large_n() {
        for n in [20usize, 24, 28, 32] {
            let seed_base: u64 = 0xb085_7ae9_1000_0000_u64.wrapping_add(n as u64);
            for trial in 0u64..100 {
                let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                let row_major = random_matrix_fp3(n, seed);
                let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
                let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                let actual = permanent_bipedal3(&mat);
                assert_eq!(
                    actual, expected,
                    "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                );
            }
        }
    }
}
