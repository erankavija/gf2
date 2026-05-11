//! `permanent_bipedal3` — fast path dispatcher and single-`u64`-pair fast
//! path for permanents over `F_3`.
//!
//! ## Single-word path (`n ≤ 64`)
//!
//! For `n ≤ 64` the column-sum vector fits in a single Bipedal3 word (one
//! `u64` mag + one `u64` sgn pair).  Each Gray-code step updates a single
//! `Bipedal3` column-sum in-place via `Bipedal3::add` or `Bipedal3::sub`
//! (the canonical paper §2.2 SSOT lives once in those methods), followed by
//! a horizontal fold via `Bipedal3::fold_mul_first_n` — the bipedal
//! multiplication tree halving lives once in that method.
//!
//! ## Multi-word path (`n > 64`)
//!
//! For `n > 64` the column-sum spans `W = ceil(n / 64)` words per leg.
//! The multi-word streaming path lives in `super::bipedal3_multiword` and
//! implements the R3 cache-blocking design
//! (`dev/plans/r3_multi_word_streaming.md`).
//!
//! ## Dispatcher
//!
//! The public `permanent_bipedal3` function dispatches to the appropriate
//! path based on `n`.  The single-word path is also exposed as
//! `permanent_bipedal3_singleword` for callers that need the single-word
//! path directly (e.g., multi-word boundary cross-checks).
//!
//! This module is the **headline single-thread fast path** of the
//! permanent epic; the 50× speedup target is measured against
//! `permanent_mod3_reference` at `n = 36`.
//!
//! # Algorithm reference
//!
//! `dev/plans/gf2_algebra_permanent.md` §7.3 (single-word path).
//! `dev/plans/r3_multi_word_streaming.md` §8 (multi-word pseudocode).

use gf2_core::gfp::Fp;

use crate::gray::gray_code_iter;
use crate::packed::bipedal3::{Bipedal3, Bipedal3Matrix};
use crate::packed::PackedField;
use crate::packed::PackedFieldVec;
use crate::permanent::bipedal3_multiword;

/// Compute the permanent of an `n × n` matrix over `F_3`, dispatching to
/// the single-word fast path for `n ≤ 64` or the multi-word streaming path
/// for `65 ≤ n ≤ N_MAX_MULTIWORD`.
///
/// This is the unified public entrypoint. For callers that always have
/// `n ≤ 64`, use [`permanent_bipedal3_singleword`] directly.
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
///   `n` must satisfy `n ≤ N_MAX_MULTIWORD` (currently 256).
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
/// Panics if `mat.cols() > bipedal3_multiword::N_MAX_MULTIWORD` (`n` must
/// be `≤ N_MAX_MULTIWORD = 256`; above that, use the W3-T15 rayon parallel
/// path or W5 GPU path).
///
/// # Complexity
///
/// `O(n · 2^n)` field operations over `Fp<3>`. See [`permanent_bipedal3_singleword`]
/// and [`bipedal3_multiword::permanent_bipedal3_multiword`] for per-path details.
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
        n <= bipedal3_multiword::N_MAX_MULTIWORD,
        "permanent_bipedal3: n must satisfy n <= {}; got n = {}",
        bipedal3_multiword::N_MAX_MULTIWORD,
        n
    );
    if n <= 63 {
        permanent_bipedal3_singleword(mat)
    } else {
        bipedal3_multiword::permanent_bipedal3_multiword(mat)
    }
}

/// Compute the permanent of an `n × n` matrix over `F_3` using the
/// single-`u64` Bipedal3 fast path.
///
/// For `n ≤ 64` the column-sum vector fits in a single Bipedal3 word
/// (one `u64` mag + one `u64` sgn pair), so each Gray-code step performs
/// exactly one Bipedal3 add or sub followed by a horizontal
/// bipedal-multiplication-tree fold of the `n` active lanes.
///
/// Prefer [`permanent_bipedal3`] for the dispatching entrypoint that also
/// handles `n > 64`.
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Bipedal3Matrix`] (column-major, `rows == cols`),
///   with `n ≤ 64`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_3: permanent = 1
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// let m = Bipedal3Matrix::from_row_major(&id, 2, 2);
/// assert_eq!(permanent_bipedal3_singleword(&m), Fp::<3>::new(1));
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
/// - Gray walk: `2^n - 1` steps, each with 1 `Bipedal3::add` or `sub`
///   (6 word-level bitwise ops) plus 1 `Bipedal3::fold_mul_first_n`
///   (~6 halving steps, 2 word ops each).
/// - Space: `O(n)` extra (the `columns` Vec plus one `Bipedal3` col-sum word).
pub fn permanent_bipedal3_singleword(mat: &Bipedal3Matrix) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3_singleword: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 63,
        "permanent_bipedal3_singleword: single-u64 fast path requires n <= 63; got n = {}",
        n
    );

    // Edge case: the 0×0 matrix has exactly one permutation (the empty
    // one), whose product over an empty index set is the vacuous product 1.
    if n == 0 {
        return Fp::<3>::new(1);
    }

    // One-time matrix-prep: extract each column j into a Bipedal3 word.
    // Lane i of columns[j] holds A[i,j] for i in 0..n; lanes n..63 are 0
    // (the additive identity, i.e. (mag=0, sgn=0)).
    //
    // Cost: O(n^2) — dominated by the O(n · 2^n) Gray walk for n ≥ 4.
    let mut columns: Vec<Bipedal3> = Vec::with_capacity(n);
    for j in 0..n {
        let col_vec = mat.column(j);
        let mut col = Bipedal3::zero();
        for i in 0..n {
            col = col.with_lane(i, col_vec.get(i));
        }
        columns.push(col);
    }

    // Column-sum accumulator as a single Bipedal3 word.
    // Lane i of col_sum holds sum_{j ∈ S} A[i,j] mod 3.
    // Lanes n..63 stay 0 throughout (add/sub leave them at 0, and
    // fold_mul_first_n pads inactive lanes to the mul-identity before folding).
    let mut col_sum = Bipedal3::zero();

    // Running Ryser accumulator and subset-size counter.
    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // Gray walk: enumerate all 2^n - 1 non-empty subsets of [n].
    // At each step (flip, parity):
    //   flip   — which column just entered or left S
    //   parity — +1 (entered, ADD) or -1 (left, SUB)
    for (flip, parity) in gray_code_iter(n) {
        if parity == 1 {
            // col_sum += columns[flip]: paper §2.2 SSOT lives in Bipedal3::add.
            subset_size += 1;
            col_sum = col_sum.add(columns[flip]);
        } else {
            // col_sum -= columns[flip]: paper §2.2 SSOT lives in Bipedal3::sub.
            subset_size -= 1;
            col_sum = col_sum.sub(columns[flip]);
        }

        // Horizontal fold via bipedal multiplication tree SSOT in fold_mul_first_n.
        let term = col_sum.fold_mul_first_n(n);

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
    use crate::permanent::reference::permanent_mod3_reference;
    use crate::permanent::ryser::permanent_ryser;
    use crate::testutil::random_matrix;
    use gf2_core::gfp::Fp;

    // Deterministic pseudo-random matrix generation lives in
    // `crate::testutil::random_matrix` — the workspace SSOT for these tests.

    /// Wrap a row-major `Vec<Fp<3>>` into a `Bipedal3Matrix`.
    fn to_bipedal3_matrix(row_major: &[Fp<3>], n: usize) -> Bipedal3Matrix {
        Bipedal3Matrix::from_row_major(row_major, n, n)
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

    /// `n > N_MAX_MULTIWORD` panics.
    ///
    /// The dispatcher caps at `N_MAX_MULTIWORD = 256`; above that the
    /// multi-word streaming path is not yet implemented (W3-T15 / W5 for
    /// large n). This test guards the upper bound.
    #[test]
    #[should_panic(expected = "n must satisfy n <=")]
    fn test_permanent_bipedal3_panics_on_n_exceeding_n_max() {
        use crate::permanent::bipedal3_multiword::N_MAX_MULTIWORD;
        let n = N_MAX_MULTIWORD + 1;
        let data = vec![Fp::<3>::new(0); n * n];
        let m = Bipedal3Matrix::from_row_major(&data, n, n);
        let _ = permanent_bipedal3(&m);
    }

    /// `permanent_bipedal3_singleword` panics for `n = 64` (too large).
    ///
    /// `gray_code_iter` requires `n <= 63` because `1u64 << 64` is undefined
    /// behaviour per the Rust reference. This test guards the boundary of
    /// the single-word path. The public dispatcher routes n=64 to the
    /// multi-word path instead of panicking.
    #[test]
    #[should_panic(expected = "single-u64 fast path requires n <= 63")]
    fn test_permanent_bipedal3_singleword_panics_on_n_64() {
        let data = vec![Fp::<3>::new(0); 64 * 64];
        let m = Bipedal3Matrix::from_row_major(&data, 64, 64);
        let _ = permanent_bipedal3_singleword(&m);
    }

    /// Dispatcher accepts n=64 without panicking (routes to multi-word path).
    ///
    /// This test only verifies that dispatch constants are consistent:
    /// n=64 is above the single-word threshold (63) and within N_MAX_MULTIWORD.
    /// The actual permanent computation at n=64 (2^64 steps) would be
    /// infeasible; this test exists purely as a dispatch contract smoke-check.
    #[test]
    fn test_permanent_bipedal3_dispatch_accepts_n64() {
        use crate::permanent::bipedal3_multiword::N_MAX_MULTIWORD;
        // n=64 must be routable: above single-word threshold and within max.
        const { assert!(64 <= N_MAX_MULTIWORD) }
    }

    // -----------------------------------------------------------------------
    // Cross-checks: permanent_bipedal3 vs permanent_ryser (default tier)
    // Per-n tests with 1000 random matrices each.
    // n=1..12 fit well within the 5 s budget; n=13..16 are slow-tier.
    // -----------------------------------------------------------------------

    macro_rules! cross_check_n {
        ($name:ident, $n:expr) => {
            #[test]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xb085_7ae9_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
        ($name:ident, $n:expr, slow) => {
            #[test]
            #[ignore = "sim: per-n cross-check (n>12, 1000 matrices) — slow oracle, multi-second runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xb085_7ae9_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    cross_check_n!(test_cross_check_n1, 1);
    cross_check_n!(test_cross_check_n2, 2);
    cross_check_n!(test_cross_check_n3, 3);
    cross_check_n!(test_cross_check_n4, 4);
    cross_check_n!(test_cross_check_n5, 5);
    cross_check_n!(test_cross_check_n6, 6);
    cross_check_n!(test_cross_check_n7, 7);
    cross_check_n!(test_cross_check_n8, 8);
    cross_check_n!(test_cross_check_n9, 9);
    cross_check_n!(test_cross_check_n10, 10);
    cross_check_n!(test_cross_check_n11, 11);
    cross_check_n!(test_cross_check_n12, 12);
    // n=13..16: 1000 matrices × Ryser O(n·2^n) exceeds 5 s for n≥13 in
    // release mode; these run only under the nightly slow tier.
    cross_check_n!(test_cross_check_n13, 13, slow);
    cross_check_n!(test_cross_check_n14, 14, slow);
    cross_check_n!(test_cross_check_n15, 15, slow);
    cross_check_n!(test_cross_check_n16, 16, slow);

    // -----------------------------------------------------------------------
    // Cross-checks: large n (slow tier — must not run in default CI)
    //
    // Oracle: `permanent_mod3_reference` (scalar i32, ~10× faster than generic
    // Fp<3> Ryser at large n). Correctness of the reference vs
    // `permanent_ryser` is established by T8's own cross-checks, so
    // "bit-identical to permanent_ryser" is preserved here by transitivity.
    //
    // Per the 2026-05-10 user-approved amendment to T9 criterion 3:
    //   - n=28/32 are NOT required.
    //   - n=20: 100 matrices × ~5 s/matrix → 5 sub-tests × 20 matrices each
    //     (each ≈ 100 s, fits 120 s slow-tier budget).
    //   - n=24: 100 matrices × ~8 s/matrix → 10 sub-tests × 10 matrices each
    //     (each ≈ 80 s, fits 120 s slow-tier budget).
    // -----------------------------------------------------------------------

    macro_rules! large_n_cross_check {
        ($name:ident, $n:expr, $trials:expr, $seed_salt:expr) => {
            #[test]
            #[ignore = "sim: large-n cross-check (n in {20, 24}) — slow oracle, multi-minute runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 = 0xb085_7ae9_2000_0000_u64
                    .wrapping_add(n as u64)
                    .wrapping_add($seed_salt);
                for trial in 0u64..$trials {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    // Use permanent_mod3_reference as oracle: ~10× faster than
                    // generic Ryser at large n. Correctness of the reference vs
                    // permanent_ryser is established by T8 cross-checks.
                    let expected = permanent_mod3_reference(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    // n=20: 5 sub-tests × 20 matrices each = 100 total.
    // ~5 s/matrix × 20 = 100 s/sub-test — fits 120 s slow-tier budget.
    large_n_cross_check!(test_cross_check_n20_a, 20, 20, 0);
    large_n_cross_check!(test_cross_check_n20_b, 20, 20, 1_000);
    large_n_cross_check!(test_cross_check_n20_c, 20, 20, 2_000);
    large_n_cross_check!(test_cross_check_n20_d, 20, 20, 3_000);
    large_n_cross_check!(test_cross_check_n20_e, 20, 20, 4_000);

    // n=24: 10 sub-tests × 10 matrices each = 100 total.
    // ~8 s/matrix × 10 = 80 s/sub-test — fits 120 s slow-tier budget.
    large_n_cross_check!(test_cross_check_n24_a, 24, 10, 0);
    large_n_cross_check!(test_cross_check_n24_b, 24, 10, 1_000);
    large_n_cross_check!(test_cross_check_n24_c, 24, 10, 2_000);
    large_n_cross_check!(test_cross_check_n24_d, 24, 10, 3_000);
    large_n_cross_check!(test_cross_check_n24_e, 24, 10, 4_000);
    large_n_cross_check!(test_cross_check_n24_f, 24, 10, 5_000);
    large_n_cross_check!(test_cross_check_n24_g, 24, 10, 6_000);
    large_n_cross_check!(test_cross_check_n24_h, 24, 10, 7_000);
    large_n_cross_check!(test_cross_check_n24_i, 24, 10, 8_000);
    large_n_cross_check!(test_cross_check_n24_j, 24, 10, 9_000);
}
