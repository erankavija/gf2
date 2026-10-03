//! `permanent_bipedal7` — Gray-code Ryser permanent over `F_7`.
//!
//! The column-sum vector is a single [`Packed7`] word (16 F_7 lanes in one
//! `u64` at 4-bit-aligned slots), so the matrix must satisfy
//! `n ≤ Packed7::LANES = 16`. Ryser's inclusion-exclusion formula
//!
//! ```text
//! perm(A) = (-1)^n * Σ_{S ⊆ [n], S ≠ ∅} (-1)^|S| * ∏_{i=0}^{n-1} Σ_{j ∈ S} A[i,j]
//! ```
//!
//! is walked in Gray-code order over the `2^n - 1` non-empty subsets: each
//! step updates the column-sum with one [`PackedField::add`] or
//! [`PackedField::sub`] and folds the first `n` lanes via
//! [`Packed7::fold_mul_first_n`].
//!
//! Compiled only when the `f7` Cargo feature is enabled.

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
/// Panics if `mat.cols() > LANES` (`LANES = 16`, the lane count of the
/// single-word accumulator).
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

/// Single-word implementation behind [`permanent_bipedal7`], with the same
/// shape contract.
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

    // The 0×0 matrix has one permutation (the empty one), with product 1.
    if n == 0 {
        return Fp::<7>::new(1);
    }

    // Lane i of columns[j] holds A[i,j] for i in 0..n; lanes n..15 are 0.
    let mut columns: Vec<Packed7> = Vec::with_capacity(n);
    for j in 0..n {
        let col_vec = mat.column(j);
        let mut col = Packed7::zero();
        for i in 0..n {
            col = col.with_lane(i, col_vec.get(i));
        }
        columns.push(col);
    }

    // Lane i of col_sum holds Σ_{j ∈ S} A[i,j] mod 7; lanes n..15 stay 0.
    let mut col_sum = Packed7::zero();

    let mut total = Fp::<7>::new(0);
    let mut subset_size: usize = 0;

    // flip is the column that entered (parity +1) or left (parity -1) S.
    for (flip, parity) in gray_code_iter(n) {
        if parity == 1 {
            subset_size += 1;
            col_sum = col_sum.add(columns[flip]);
        } else {
            subset_size -= 1;
            col_sum = col_sum.sub(columns[flip]);
        }

        let term = col_sum.fold_mul_first_n(n);

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

    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_permanent_bipedal7_panics_on_non_square() {
        let data = vec![Fp::<7>::new(0); 3 * 5];
        let m = Packed7Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal7(&m);
    }

    #[test]
    #[should_panic(expected = "single-word path requires n <=")]
    fn test_permanent_bipedal7_panics_on_n_exceeding_lanes() {
        let n = LANES + 1;
        let data = vec![Fp::<7>::new(0); n * n];
        let m = Packed7Matrix::from_row_major(&data, n, n);
        let _ = permanent_bipedal7(&m);
    }

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
    cross_check_n!(test_cross_check_n13, 13);
    cross_check_n!(test_cross_check_n14, 14);
}
