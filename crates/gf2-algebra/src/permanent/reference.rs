//! `permanent_mod3_reference` — Rust port of `@/citation/Scheinerman2024`
//! Algorithm 1 / Listing 1 (Julia naive Ryser).
//!
//! [`permanent_mod3_reference`] is a scalar `i32` implementation of Ryser's
//! formula specialised to `F_3`, with explicit `% 3` reductions and a
//! self-contained Gray walk.

use gf2_core::gfp::Fp;

/// Compute the permanent of an `n × n` matrix over `F_3` using Ryser's
/// formula with a self-contained Gray-code walk and scalar `i32 % 3`
/// arithmetic.
///
/// Ports `@/citation/Scheinerman2024` Algorithm 1 / Listing 1, the Julia
/// `permanent_mod3` function, line by line. It evaluates Ryser's
/// inclusion-exclusion formula:
///
/// ```text
/// perm(A) = (-1)^n * sum_{S ⊆ [n], S ≠ ∅}  (-1)^|S|  * prod_{i=0}^{n-1}  sum_{j ∈ S} A[i,j]
/// ```
///
/// # Arguments
///
/// * `matrix` — Flat row-major slice of `n × n` elements of `Fp<3>`.
///   `matrix[i * n + j]` is the entry at row `i`, column `j`.
/// * `n` — Matrix dimension (number of rows = number of columns).
///
/// # Panics
///
/// Panics if `n > 63`: the Gray-code loop bound is the single `u64`
/// `1u64 << n`.
///
/// Panics if `matrix.len() != n * n`.
///
/// # Complexity
///
/// `O(n · 2^n)` scalar operations (`n` column-sum updates and `n` `% 3`
/// multiplies per Gray step). Extra space is `O(n)` for the `cs` accumulator
/// vector.
pub fn permanent_mod3_reference(matrix: &[Fp<3>], n: usize) -> Fp<3> {
    // Listing 1, line 1-2: signature and shape assertion.
    assert!(
        n <= 63,
        "permanent_mod3_reference: n = {} exceeds the single-u64 Gray-code register's \
         n <= 63 bound",
        n
    );
    assert_eq!(
        matrix.len(),
        n * n,
        "permanent_mod3_reference: matrix.len() ({}) must equal n * n ({}) where n = {}",
        matrix.len(),
        n * n,
        n
    );

    // Listing 1, line 3: empty matrix — permanent of empty product = 1.
    if n == 0 {
        return Fp::<3>::new(1);
    }

    // Listing 1, line 4: column-sum vector cs initialised to zero.
    // i32 arithmetic as in the listing; reduced to Fp<3> only at exit.
    let mut cs = vec![0i32; n];

    // Listing 1, line 5: total = 0 accumulator.
    let mut total: i32 = 0;

    // Listing 1, line 6 ("for k in 1:(2^n - 1)"): Gray-code subset walk.
    let upper: u64 = 1u64 << n;
    for k in 1..upper {
        // Listing 1, line 7 ("flip = trailing_zeros(k)"): the column
        // that toggles in Gray(k) vs Gray(k-1).
        let flip = k.trailing_zeros() as usize;

        // Listing 1, line 8 ("g_k = k ⊻ (k >> 1)"): Gray-code register.
        let g_k = k ^ (k >> 1);

        // Listing 1, line 9 ("if (g_k >> flip) & 1 == 1"): ADD vs SUB.
        let added = ((g_k >> flip) & 1) == 1;

        if added {
            // Listing 1, lines 10-12
            // ("for i in 1:n: cs[i] = (cs[i] + A[i, flip+1]) % 3"):
            for i in 0..n {
                cs[i] = (cs[i] + matrix[i * n + flip].value() as i32) % 3;
            }
        } else {
            // Listing 1, lines 13-15
            // ("for i in 1:n: cs[i] = ((cs[i] + 3) - A[i, flip+1]) % 3"):
            for i in 0..n {
                cs[i] = ((cs[i] - matrix[i * n + flip].value() as i32) + 3) % 3;
            }
        }

        // Listing 1, lines 16-18
        // ("prod = 1; for i in 1:n: prod = (prod * cs[i]) % 3"):
        let mut prod: i32 = 1;
        for &c in &cs {
            prod = (prod * c) % 3;
        }

        // Listing 1, line 19 ("popcount(g_k) % 2"): Ryser sign
        // (-1)^|S| with |S| = popcount(g_k).
        let card = g_k.count_ones() as usize;
        if card % 2 == 1 {
            // Listing 1, line 20 ("total = (total - prod + 3) % 3"):
            total = ((total - prod) + 3) % 3;
        } else {
            // Listing 1, line 21 ("total = (total + prod) % 3"):
            total = (total + prod) % 3;
        }
    }

    // Listing 1, lines 23-25 ("if n is odd: total = (3 - total) % 3"):
    // the outer (-1)^n factor.
    if n % 2 == 1 {
        total = (3 - total) % 3;
    }

    // Listing 1, line 26 ("return total"): cast i32 → Fp<3>.
    Fp::<3>::new(total as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::gfp::Fp;
    use gf2_core::rng::Lcg;

    use crate::permanent::permanent_ryser;
    use crate::testutil::random_matrix_with_rng;

    /// Run `n_matrices` cross-checks of `permanent_mod3_reference` vs
    /// `permanent_ryser::<Fp<3>>` for matrices of dimension `n`.
    ///
    /// Seed is derived from `n` so each dimension gets an independent RNG
    /// stream, making failures from a given `n` reproducible.
    fn run_cross_check(n: usize, n_matrices: usize) {
        let mut rng = Lcg::new(0xBA5E_BA11_DEC0_DE57u64.wrapping_add(n as u64));
        for trial in 0..n_matrices {
            let m = random_matrix_with_rng::<3>(&mut rng, n);
            let expected = permanent_ryser::<Fp<3>>(&m, n);
            let actual = permanent_mod3_reference(&m, n);
            assert_eq!(
                actual, expected,
                "permanent_mod3_reference != permanent_ryser for n={n} trial={trial}"
            );
        }
    }

    /// n=0: permanent of the 0×0 matrix is 1 (vacuous product).
    #[test]
    fn test_reference_empty_matrix() {
        assert_eq!(
            permanent_mod3_reference(&[], 0),
            Fp::<3>::new(1),
            "0×0 permanent should be one"
        );
    }

    /// n=1: permanent of [a] is a for each element of F_3.
    #[test]
    fn test_reference_1x1() {
        for v in 0u64..3 {
            let a = Fp::<3>::new(v);
            let result = permanent_mod3_reference(&[a], 1);
            assert_eq!(result, a, "1×1 permanent of [{v}] should be {v}");
        }
    }

    /// 2×2 identity: permanent = 1.
    #[test]
    fn test_reference_2x2_identity() {
        let id: Vec<Fp<3>> = vec![
            Fp::<3>::new(1),
            Fp::<3>::new(0),
            Fp::<3>::new(0),
            Fp::<3>::new(1),
        ];
        assert_eq!(
            permanent_mod3_reference(&id, 2),
            Fp::<3>::new(1),
            "2×2 identity permanent should be 1"
        );
    }

    /// 2×2 all-ones: permanent = 2! mod 3 = 2.
    #[test]
    fn test_reference_2x2_all_ones() {
        let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 4];
        assert_eq!(
            permanent_mod3_reference(&ones, 2),
            Fp::<3>::new(2),
            "2×2 all-ones permanent should be 2"
        );
    }

    /// 3×3 all-ones: permanent = 3! mod 3 = 6 mod 3 = 0.
    #[test]
    fn test_reference_3x3_all_ones() {
        let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 9];
        assert_eq!(
            permanent_mod3_reference(&ones, 3),
            Fp::<3>::new(0),
            "3×3 all-ones permanent should be 0 (= 6 mod 3)"
        );
    }

    #[test]
    #[should_panic(
        expected = "permanent_mod3_reference: n = 64 exceeds the single-u64 Gray-code register's n <= 63 bound"
    )]
    fn test_reference_panics_n_exceeds_63() {
        let matrix: Vec<Fp<3>> = vec![Fp::<3>::new(0); 64 * 64];
        let _ = permanent_mod3_reference(&matrix, 64);
    }

    #[test]
    #[should_panic(
        expected = "permanent_mod3_reference: matrix.len() (3) must equal n * n (4) where n = 2"
    )]
    fn test_reference_panics_shape_mismatch() {
        let matrix: Vec<Fp<3>> = vec![Fp::<3>::new(0); 3];
        let _ = permanent_mod3_reference(&matrix, 2);
    }

    #[test]
    fn test_reference_cross_check_random_n1() {
        run_cross_check(1, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n2() {
        run_cross_check(2, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n3() {
        run_cross_check(3, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n4() {
        run_cross_check(4, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n5() {
        run_cross_check(5, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n6() {
        run_cross_check(6, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n7() {
        run_cross_check(7, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n8() {
        run_cross_check(8, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n9() {
        run_cross_check(9, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n10() {
        run_cross_check(10, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n11() {
        run_cross_check(11, 1000);
    }

    #[test]
    fn test_reference_cross_check_random_n12() {
        run_cross_check(12, 1000);
    }
}
