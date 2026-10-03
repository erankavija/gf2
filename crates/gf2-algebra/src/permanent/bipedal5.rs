//! `permanent_bipedal5` — Gray-code Ryser permanent over `F_5`.
//!
//! Walks the Gray-code subset order, updates a single `Packed5` column-sum
//! word (one `u64`-triple of bit-planes) by one [`Packed5::add`] or
//! [`Packed5::sub`] per step, and folds the first `n` lanes via
//! [`Packed5::fold_mul_first_n`]. `permanent_bipedal5` panics for `n > 63`.
//!
//! Compiled only when the `f5` Cargo feature is enabled.

use gf2_core::gfp::Fp;

use crate::gray::gray_code_iter;
use crate::packed::packed5::{Packed5, Packed5Matrix};
use crate::packed::{PackedField, PackedFieldVec};

/// Compute the permanent of an `n × n` matrix over `F_5`, using the single-word
/// Gray-code Ryser fast path.
///
/// The permanent of an `n × n` matrix `A` over `F_5` is:
///
/// ```text
/// perm(A) = sum over all permutations sigma of prod_{i=0}^{n-1} A[i, sigma(i)]
/// ```
///
/// Evaluated via Ryser's inclusion-exclusion formula in Gray-code order:
///
/// ```text
/// perm(A) = (-1)^n * sum_{S ⊆ [n], S ≠ ∅} (-1)^|S| * prod_{i=0}^{n-1} sum_{j ∈ S} A[i,j]
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > 63` (single-word path requires `n ≤ 63`).
///
/// # Complexity
///
/// `O(n · 2^n)` field operations over `Fp<5>`:
/// - Matrix prep: `O(n^2)` one-time lane-by-lane column extraction.
/// - Gray walk: `2^n - 1` steps, each with 1 [`Packed5::add`] or
///   [`Packed5::sub`] (O(1), pure bit-plane logic on a single `u64`-triple)
///   plus 1 [`Packed5::fold_mul_first_n`] (O(n) lane-decode passes,
///   bounded constant at n ≤ 63).
/// - Space: `O(n)` extra (the `columns` Vec plus one `Packed5` col-sum word).
pub fn permanent_bipedal5(mat: &Packed5Matrix) -> Fp<5> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal5: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 63,
        "permanent_bipedal5: single-word path requires n <= 63; got n = {}. \
         For n > 63 use permanent_ryser::<Fp<5>>.",
        n,
    );

    permanent_bipedal5_singleword(mat)
}

/// Single-word implementation behind [`permanent_bipedal5`], with the same
/// shape contract.
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` or `mat.cols() > 63` (the
/// single-word path requires `n <= 63`).
///
/// # Complexity
///
/// `O(n · 2^n)` — same as [`permanent_bipedal5`].
pub fn permanent_bipedal5_singleword(mat: &Packed5Matrix) -> Fp<5> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal5_singleword: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 63,
        "permanent_bipedal5_singleword: single-word path requires n <= 63; got n = {}",
        n
    );

    // The 0×0 matrix has one permutation (the empty one), with product 1.
    if n == 0 {
        return Fp::<5>::new(1);
    }

    // Lane i of columns[j] holds A[i,j] for i in 0..n; lanes n..63 are 0.
    let mut columns: Vec<Packed5> = Vec::with_capacity(n);
    for j in 0..n {
        let col_vec = mat.column(j);
        let mut col = Packed5::zero();
        for i in 0..n {
            col = col.with_lane(i, col_vec.get(i));
        }
        columns.push(col);
    }

    // Lane i of col_sum holds Σ_{j ∈ S} A[i,j] mod 5; lanes n..63 stay 0.
    let mut col_sum = Packed5::zero();

    let mut total = Fp::<5>::new(0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packed::Packed5Matrix;
    use crate::permanent::ryser::permanent_ryser;
    use crate::testutil::random_matrix;
    use gf2_core::gfp::Fp;

    fn to_packed5_matrix(row_major: &[Fp<5>], n: usize) -> Packed5Matrix {
        Packed5Matrix::from_row_major(row_major, n, n)
    }

    /// `permanent_bipedal5` of the 0×0 matrix is `Fp::<5>::new(1)` (vacuous product).
    #[test]
    fn test_permanent5_empty_matrix() {
        let m = Packed5Matrix::from_row_major(&[], 0, 0);
        assert_eq!(
            permanent_bipedal5(&m),
            Fp::<5>::new(1),
            "0×0 permanent must be 1"
        );
    }

    /// A 1×1 matrix `[v]` has permanent = `v`.
    #[test]
    fn test_permanent5_1x1() {
        for v in 0u64..5 {
            let row = vec![Fp::<5>::new(v)];
            let m = Packed5Matrix::from_row_major(&row, 1, 1);
            assert_eq!(
                permanent_bipedal5(&m),
                Fp::<5>::new(v),
                "1×1 permanent of [{v}] must be {v}"
            );
        }
    }

    /// `I_n` has permanent = 1 for `n ∈ {1, 2, 3, 4}`.
    #[test]
    fn test_permanent5_identity_n() {
        for n in 1..=4usize {
            let mut id = vec![Fp::<5>::new(0); n * n];
            for i in 0..n {
                id[i * n + i] = Fp::<5>::new(1);
            }
            let m = Packed5Matrix::from_row_major(&id, n, n);
            assert_eq!(
                permanent_bipedal5(&m),
                Fp::<5>::new(1),
                "identity permanent must be 1 for n={n}"
            );
        }
    }

    /// All-ones `n×n` matrix: permanent = `n! mod 5` for `n ∈ {1, 2, 3, 4}`.
    #[test]
    fn test_permanent5_all_ones_n() {
        // n! mod 5: {1, 2, 1, 4}
        let expected = [1u64, 2, 1, 4];
        for n in 1..=4usize {
            let ones = vec![Fp::<5>::new(1); n * n];
            let m = Packed5Matrix::from_row_major(&ones, n, n);
            assert_eq!(
                permanent_bipedal5(&m),
                Fp::<5>::new(expected[n - 1]),
                "all-ones permanent for n={n} must be {} (= n! mod 5)",
                expected[n - 1]
            );
        }
    }

    #[test]
    fn test_permanent5_2x2_known_vector() {
        let data: Vec<Fp<5>> = vec![
            Fp::<5>::new(1),
            Fp::<5>::new(2),
            Fp::<5>::new(3),
            Fp::<5>::new(4),
        ];
        let m = Packed5Matrix::from_row_major(&data, 2, 2);
        // perm = 1*4 + 2*3 = 4 + 6 = 10 mod 5 = 0
        assert_eq!(permanent_bipedal5(&m), Fp::<5>::new(0));
    }

    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_permanent5_panics_on_non_square() {
        let data = vec![Fp::<5>::new(0); 3 * 5];
        let m = Packed5Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal5(&m);
    }

    #[test]
    #[should_panic(expected = "single-word path requires n <=")]
    fn test_permanent5_panics_on_n_64() {
        let data = vec![Fp::<5>::new(0); 64 * 64];
        let m = Packed5Matrix::from_row_major(&data, 64, 64);
        let _ = permanent_bipedal5(&m);
    }

    macro_rules! cross_check_n {
        ($name:ident, $n:expr) => {
            #[test]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xc6d5_b4a3_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<5>(n, seed);
                    let mat = to_packed5_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<5>>(&row_major, n);
                    let actual = permanent_bipedal5(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
        ($name:ident, $n:expr, slow) => {
            #[test]
            #[ignore = "sim: per-n cross-check (n>14, 1000 matrices) — slow oracle, multi-second runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xc6d5_b4a3_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<5>(n, seed);
                    let mat = to_packed5_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<5>>(&row_major, n);
                    let actual = permanent_bipedal5(&mat);
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
    cross_check_n!(test_cross_check_n15, 15, slow);
    cross_check_n!(test_cross_check_n16, 16, slow);

    macro_rules! boundary_check_n {
        ($name:ident, $n:expr, $trials:expr) => {
            #[test]
            #[ignore = "sim: word-boundary cross-check (n > 14, sparse batch) — slow oracle"]
            fn $name() {
                let n: usize = $n;
                let trials: u64 = $trials;
                let seed_base: u64 = 0xb02d_4147_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..trials {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<5>(n, seed);
                    let mat = to_packed5_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<5>>(&row_major, n);
                    let actual = permanent_bipedal5(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch (boundary): n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    boundary_check_n!(test_boundary_cross_check_n20, 20, 20);
    boundary_check_n!(test_boundary_cross_check_n24, 24, 10);
}
