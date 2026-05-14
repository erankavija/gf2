//! `permanent_bipedal7` — Gray-code Ryser permanent over `F_7`.
//!
//! ## Single-word path (`n ≤ LANES = 16`)
//!
//! For `n ≤ 16` the column-sum vector fits in a single [`Packed7`] word
//! (16 F_7 lanes in one `u64` at 4-bit-aligned slots). Each Gray-code step
//! updates the single [`Packed7`] column-sum in-place via
//! [`PackedField::add`] or [`PackedField::sub`], followed by a horizontal
//! fold via [`Packed7::fold_mul_first_n`] — the LUT-based multiplication
//! tree SSOT lives once in that method.
//!
//! ## Single-word size bound
//!
//! The matrix must satisfy `n ≤ Packed7::LANES = 16`. The column-sum
//! accumulator for `n` rows fits in one [`Packed7`] word (16 lanes per
//! `u64`) exactly when `n ≤ 16`. Multi-word paths are out of scope for
//! this issue.
//!
//! ## Algorithm
//!
//! Ryser's inclusion-exclusion formula in Gray-code order:
//!
//! ```text
//! perm(A) = (-1)^n * Σ_{S ⊆ [n], S ≠ ∅} (-1)^|S| * ∏_{i=0}^{n-1} Σ_{j ∈ S} A[i,j]
//! ```
//!
//! The Gray-code walk visits all `2^n - 1` non-empty subsets, updating the
//! column-sum at each step with a single add or sub (one column entering or
//! leaving the subset), then folding all `n` row-sums via `fold_mul_first_n`.
//!
//! ## Feature gating
//!
//! Compiled only when the `f7` Cargo feature is enabled (D1c §2).
//!
//! # Algorithm reference
//!
//! `dev/plans/gf2_algebra_permanent.md` §6 (F_7 packed permanent).
//! Mirrors the F_3 path in `crate::permanent::bipedal3`.

use gf2_core::gfp::Fp;

use crate::gray::gray_code_iter;
use crate::packed::packed7::{Packed7, Packed7Matrix, LANES};
use crate::packed::PackedField;

/// Compute the permanent of an `n × n` matrix over `F_7` using the
/// single-[`Packed7`]-word fast path.
///
/// The permanent of an `n × n` matrix `A` over `F_7` is:
///
/// ```text
/// perm(A) = Σ_{σ ∈ S_n} ∏_{i=0}^{n-1} A[i, σ(i)]
/// ```
///
/// Evaluated via Ryser's inclusion-exclusion formula in Gray-code order.
/// At each of the `2^n - 1` non-empty subset steps the column-sum
/// [`Packed7`] is updated with one lane-wise add or sub (one packed LUT
/// op), then folded to a single `F_7` scalar via
/// [`Packed7::fold_mul_first_n`].
///
/// ## Single-word size bound
///
/// `n` must satisfy `n ≤ LANES = 16`. [`Packed7`] packs exactly 16 lanes
/// per `u64`; for `n ≤ 16` the row-count fits in one word and no
/// multi-word path is needed. See [`crate::packed::Packed7`] for the
/// encoding details (R2 Candidate A, 4-bit-aligned slots).
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Packed7Matrix`] (column-major, `rows == cols`),
///   with `n ≤ LANES`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Packed7Matrix;
/// use gf2_algebra::permanent::permanent_bipedal7;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_7: permanent = 1
/// let id: Vec<Fp<7>> = vec![
///     Fp::<7>::new(1), Fp::<7>::new(0),
///     Fp::<7>::new(0), Fp::<7>::new(1),
/// ];
/// let m = Packed7Matrix::from_row_major(&id, 2, 2);
/// assert_eq!(permanent_bipedal7(&m), Fp::<7>::new(1));
///
/// // 2×2 all-ones over F_7: permanent = 2! mod 7 = 2
/// let ones: Vec<Fp<7>> = vec![Fp::<7>::new(1); 4];
/// let m2 = Packed7Matrix::from_row_major(&ones, 2, 2);
/// assert_eq!(permanent_bipedal7(&m2), Fp::<7>::new(2));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > LANES` (`n` must be `≤ LANES = 16`; above
/// that the single-word accumulator overflows — multi-word support is
/// out of scope for this issue).
///
/// # Complexity
///
/// `O(n · 2^n)` field operations over `Fp<7>`:
/// - Matrix prep: `O(n^2)` lane-by-lane column extraction.
/// - Gray walk: `2^n - 1` steps, each with 1 lane-wise `Packed7` add
///   or sub (8 LUT lookups) plus 1 `Packed7::fold_mul_first_n` (≤ 16
///   scalar mul ops).
/// - Space: `O(n)` extra (the `columns` Vec plus one `Packed7` word).
pub fn permanent_bipedal7(mat: &Packed7Matrix) -> Fp<7> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal7: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= LANES,
        "permanent_bipedal7: single-word path requires n <= {LANES}; got n = {n}"
    );
    permanent_bipedal7_singleword(mat)
}

/// Inner single-word implementation — called by [`permanent_bipedal7`] after
/// the shape assertions pass.
///
/// Exposed as `pub` so callers that always have `n ≤ LANES` can call it
/// directly without re-checking assertions, and so tests can verify it
/// independently (matching the `bipedal3` pattern where
/// `permanent_bipedal3_singleword` is also `pub`).
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Packed7Matrix`] (column-major, `rows == cols`),
///   with `n ≤ LANES`. No additional assertions — the caller must have
///   already validated the matrix shape.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Packed7Matrix;
/// use gf2_algebra::permanent::bipedal7::permanent_bipedal7_singleword;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_7: permanent = 1
/// let id: Vec<Fp<7>> = vec![
///     Fp::<7>::new(1), Fp::<7>::new(0),
///     Fp::<7>::new(0), Fp::<7>::new(1),
/// ];
/// let m = Packed7Matrix::from_row_major(&id, 2, 2);
/// assert_eq!(permanent_bipedal7_singleword(&m), Fp::<7>::new(1));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` or `mat.cols() > LANES`.
///
/// # Complexity
///
/// `O(n · 2^n)` — see [`permanent_bipedal7`] for detailed breakdown.
pub fn permanent_bipedal7_singleword(mat: &Packed7Matrix) -> Fp<7> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal7_singleword: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= LANES,
        "permanent_bipedal7_singleword: single-word path requires n <= {LANES}; got n = {n}"
    );

    // Edge case: the 0×0 matrix has exactly one permutation (the empty
    // one), whose product over an empty index set is the vacuous product 1.
    if n == 0 {
        return Fp::<7>::new(1);
    }

    // One-time matrix-prep: extract each column j into a Packed7 word.
    // Lane i of columns[j] holds A[i,j] for i in 0..n; lanes n..15 are 0
    // (the additive identity, i.e. nibble value 0).
    //
    // Cost: O(n^2) — dominated by the O(n · 2^n) Gray walk for n >= 4.
    let mut columns: Vec<Packed7> = Vec::with_capacity(n);
    for j in 0..n {
        let col_vec = mat.column(j);
        let mut col = Packed7::zero();
        for i in 0..n {
            col = col.with_lane(i, col_vec.get(i));
        }
        columns.push(col);
    }

    // Column-sum accumulator as a single Packed7 word.
    // Lane i of col_sum holds Σ_{j ∈ S} A[i,j] mod 7.
    // Lanes n..15 stay 0 throughout (add/sub leave them at 0, and
    // fold_mul_first_n only reads lanes 0..n).
    let mut col_sum = Packed7::zero();

    // Running Ryser accumulator and subset-size counter.
    let mut total = Fp::<7>::new(0);
    let mut subset_size: usize = 0;

    // Gray walk: enumerate all 2^n - 1 non-empty subsets of [n].
    // At each step (flip, parity):
    //   flip   — which column just entered or left S
    //   parity — +1 (entered, ADD) or -1 (left, SUB)
    for (flip, parity) in gray_code_iter(n) {
        if parity == 1 {
            // col_sum += columns[flip]: lane-wise F_7 add via ADD_LUT.
            subset_size += 1;
            col_sum = col_sum.add(columns[flip]);
        } else {
            // col_sum -= columns[flip]: lane-wise F_7 sub via SUB_LUT.
            subset_size -= 1;
            col_sum = col_sum.sub(columns[flip]);
        }

        // Horizontal fold: product of the first n lanes via MUL_LUT.
        // fold_mul_first_n treats lanes n..15 as 1 (no contribution).
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
    use crate::packed::Packed7Matrix;
    use crate::permanent::ryser::permanent_ryser;
    use crate::testutil::random_matrix;
    use gf2_core::gfp::Fp;

    /// Wrap a row-major `Vec<Fp<7>>` into a `Packed7Matrix`.
    fn to_packed7_matrix(row_major: &[Fp<7>], n: usize) -> Packed7Matrix {
        Packed7Matrix::from_row_major(row_major, n, n)
    }

    // -----------------------------------------------------------------------
    // Hand-checked vectors
    // -----------------------------------------------------------------------

    /// `permanent_bipedal7` of the 0×0 matrix is `Fp::<7>::new(1)` (vacuous product).
    #[test]
    fn test_permanent_bipedal7_empty_matrix() {
        let m = Packed7Matrix::from_row_major(&[], 0, 0);
        assert_eq!(
            permanent_bipedal7(&m),
            Fp::<7>::new(1),
            "0×0 permanent must be 1"
        );
    }

    /// A 1×1 matrix `[v]` has permanent = `v`.
    #[test]
    fn test_permanent_bipedal7_1x1() {
        for v in 0u64..7 {
            let row = vec![Fp::<7>::new(v)];
            let m = Packed7Matrix::from_row_major(&row, 1, 1);
            assert_eq!(
                permanent_bipedal7(&m),
                Fp::<7>::new(v),
                "1×1 permanent of [{v}] must be {v}"
            );
        }
    }

    /// `I_n` has permanent = 1 for `n ∈ {1, 2, 3, 4}`.
    #[test]
    fn test_permanent_bipedal7_identity_n() {
        for n in 1..=4usize {
            let mut id = vec![Fp::<7>::new(0); n * n];
            for i in 0..n {
                id[i * n + i] = Fp::<7>::new(1);
            }
            let m = Packed7Matrix::from_row_major(&id, n, n);
            assert_eq!(
                permanent_bipedal7(&m),
                Fp::<7>::new(1),
                "identity permanent must be 1 for n={n}"
            );
        }
    }

    /// All-ones `n×n` matrix: permanent = `n! mod 7`.
    ///
    /// n! mod 7: n=1→1, n=2→2, n=3→6, n=4→24%7=3, n=5→120%7=1, n=6→720%7=720-102*7=6, n=7→5040%7=0.
    #[test]
    fn test_permanent_bipedal7_all_ones_n() {
        // n! mod 7: 1, 2, 6, 3, 1, 6, 0
        let expected = [1u64, 2, 6, 3, 1, 6, 0];
        for n in 1..=7usize {
            let ones = vec![Fp::<7>::new(1); n * n];
            let m = Packed7Matrix::from_row_major(&ones, n, n);
            assert_eq!(
                permanent_bipedal7(&m),
                Fp::<7>::new(expected[n - 1]),
                "all-ones permanent for n={n} must be {} (= n! mod 7)",
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
    fn test_permanent_bipedal7_panics_on_non_square() {
        let data = vec![Fp::<7>::new(0); 3 * 5];
        let m = Packed7Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal7(&m);
    }

    /// `n > LANES` panics.
    #[test]
    #[should_panic(expected = "single-word path requires n <=")]
    fn test_permanent_bipedal7_panics_on_n_exceeding_lanes() {
        let n = LANES + 1;
        let data = vec![Fp::<7>::new(0); n * n];
        let m = Packed7Matrix::from_row_major(&data, n, n);
        let _ = permanent_bipedal7(&m);
    }

    // -----------------------------------------------------------------------
    // Cross-checks: permanent_bipedal7 vs permanent_ryser<Fp<7>>
    //
    // Per the issue success criteria: 1000 random matrices for each
    // n ∈ {1, …, 14} (covers epic success criterion 6).
    //
    // Timing analysis (release mode):
    //   Each Ryser call: O(n · 2^n) ops.
    //   n=1..12: 2^12 = 4096 steps × 1000 matrices — fast tier (well < 5 s).
    //   n=13:    2^13 = 8192 steps × 1000 matrices — borderline; in practice
    //            the Ryser scalar oracle dominates. Kept in fast tier; if it
    //            exceeds 5 s, split to slow tier.
    //   n=14:    2^14 = 16384 steps × 1000 matrices — slow tier.
    // -----------------------------------------------------------------------

    macro_rules! cross_check_n {
        ($name:ident, $n:expr) => {
            #[test]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xf7b1_9d3e_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<7>(n, seed);
                    let mat = to_packed7_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<7>>(&row_major, n);
                    let actual = permanent_bipedal7(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
        ($name:ident, $n:expr, slow) => {
            #[test]
            #[ignore = "sim: per-n cross-check (n>=13, 1000 matrices) — slow oracle, multi-second runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xf7b1_9d3e_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<7>(n, seed);
                    let mat = to_packed7_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<7>>(&row_major, n);
                    let actual = permanent_bipedal7(&mat);
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
    // n=13..14: slow tier (1000 matrices × Ryser O(n·2^n) > 5 s per test).
    cross_check_n!(test_cross_check_n13, 13, slow);
    cross_check_n!(test_cross_check_n14, 14, slow);
}
