//! `permanent_mod3_reference` — faithful Rust port of Scheinerman 2024
//! Algorithm 1 / Listing 1 (Julia naive Ryser).
//!
//! This module provides [`permanent_mod3_reference`], a scalar `i32`
//! implementation of Ryser's formula specialised to `F_3`. It mirrors the
//! paper's Julia listing as closely as possible — explicit `% 3` reductions,
//! a self-contained Gray walk, no bipedal trick, no SIMD — so that it serves
//! as the **paper-baseline denominator** for the `permanent_bipedal3` 50×
//! headline speedup criterion.
//!
//! # Status
//!
//! W2-T8 complete — paper Julia port.

use gf2_core::gfp::Fp;

/// Compute the permanent of an `n × n` matrix over `F_3` using Ryser's
/// formula with a self-contained Gray-code walk and scalar `i32 % 3`
/// arithmetic.
///
/// This is a **faithful Rust port of Scheinerman 2024 (arxiv 2407.20205v2)
/// Algorithm 1 / Listing 1**, the Julia `permanent_mod3` function that
/// uses naive scalar `Int` arithmetic with `% 3` reductions. It has no
/// bipedal trick, no SIMD, and no shared helpers — matching the paper's
/// listing line-by-line. Its role in this workspace is to be the
/// **50× speedup denominator** against which `permanent_bipedal3` kernels
/// are benchmarked. The paper reports a single-thread ratio of 86.9× over
/// this baseline on a 4.20 GHz desktop at `n = 36`.
///
/// The algorithm evaluates Ryser's inclusion-exclusion formula:
///
/// ```text
/// perm(A) = (-1)^n * sum_{S ⊆ [n], S ≠ ∅}  (-1)^|S|  * prod_{i=0}^{n-1}  sum_{j ∈ S} A[i,j]
/// ```
///
/// Subsets are enumerated in binary-reflected Gray-code order (self-contained
/// loop, not via `gray_code_iter`) so each step updates one column of the
/// running row-sums vector `cs`.
///
/// # Arguments
///
/// * `matrix` — Flat row-major slice of `n × n` elements of `Fp<3>`.
///   `matrix[i * n + j]` is the entry at row `i`, column `j`.
/// * `n` — Matrix dimension (number of rows = number of columns).
///
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::permanent_mod3_reference;
/// use gf2_core::gfp::Fp;
///
/// // 0×0 matrix: permanent = 1 (vacuous product)
/// assert_eq!(permanent_mod3_reference(&[], 0), Fp::<3>::new(1));
///
/// // 1×1 matrix [2]: permanent = 2
/// assert_eq!(permanent_mod3_reference(&[Fp::<3>::new(2)], 1), Fp::<3>::new(2));
///
/// // 2×2 identity over F_3: permanent = 1
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// assert_eq!(permanent_mod3_reference(&id, 2), Fp::<3>::new(1));
///
/// // 2×2 all-ones over F_3: permanent = 2! mod 3 = 2
/// let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 4];
/// assert_eq!(permanent_mod3_reference(&ones, 2), Fp::<3>::new(2));
///
/// // 3×3 all-ones over F_3: permanent = 3! mod 3 = 0
/// let ones3: Vec<Fp<3>> = vec![Fp::<3>::new(1); 9];
/// assert_eq!(permanent_mod3_reference(&ones3, 3), Fp::<3>::new(0));
/// ```
///
/// # Panics
///
/// Panics if `n > 63`. The self-contained Gray-code loop uses a single `u64`
/// register (`1u64 << n`) which is only well-defined for `n ≤ 63`; shifting
/// by 64 is undefined behaviour in Rust.
///
/// Panics if `matrix.len() != n * n`.
///
/// # Complexity
///
/// `O(n · 2^n)` scalar operations (one `% 3` multiply and `n` column-sum
/// updates per Gray step). Extra space is `O(n)` for the `cs` accumulator
/// vector. Performance is comparable to or slightly slower than the generic
/// `permanent_ryser::<Fp<3>>` because the explicit `% 3` per-step reductions
/// prevent accumulating several additions before reducing, unlike the
/// Montgomery-form `Fp<3>` ops used by the generic driver.
pub fn permanent_mod3_reference(matrix: &[Fp<3>], n: usize) -> Fp<3> {
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

    if n == 0 {
        return Fp::<3>::new(1);
    }

    // Internal scalar state — i32 arithmetic per paper Listing 1, NOT Fp<3>
    // Montgomery-form ops. The whole point of this baseline is to mirror the
    // paper's "naive Julia Int" implementation. Reduce to Fp<3> only at exit.
    let mut cs = vec![0i32; n];
    let mut total: i32 = 0;

    let upper: u64 = 1u64 << n;
    for k in 1..upper {
        // flip: the index of the column that toggles in Gray(k) vs Gray(k-1).
        let flip = k.trailing_zeros() as usize;
        // g_k: the current Gray code word.
        let g_k = k ^ (k >> 1);
        // added: true if column `flip` just entered the subset (bit is 1 in g_k).
        let added = ((g_k >> flip) & 1) == 1;

        if added {
            for i in 0..n {
                cs[i] = (cs[i] + matrix[i * n + flip].value() as i32) % 3;
            }
        } else {
            // Use (cs[i] - val + 3) % 3 to stay non-negative, mirroring the
            // paper's `((cs[i] + 3) - A[i, flip+1]) % 3`.
            for i in 0..n {
                cs[i] = ((cs[i] - matrix[i * n + flip].value() as i32) + 3) % 3;
            }
        }

        // Compute prod_{i=0}^{n-1} cs[i] mod 3.
        let mut prod: i32 = 1;
        for &c in &cs {
            prod = (prod * c) % 3;
        }

        // Ryser sign: (-1)^|S| where |S| = popcount(g_k).
        // Odd |S| → subtract; even |S| → add.
        let card = g_k.count_ones() as usize;
        if card % 2 == 1 {
            total = ((total - prod) + 3) % 3;
        } else {
            total = (total + prod) % 3;
        }
    }

    // Apply the outer (-1)^n factor from Ryser's formula.
    if n % 2 == 1 {
        total = (3 - total) % 3;
    }

    Fp::<3>::new(total as u64)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::gfp::Fp;

    use crate::permanent::permanent_ryser;

    // -----------------------------------------------------------------------
    // LCG pseudo-random matrix generator (mirrors the one in ryser.rs)
    // -----------------------------------------------------------------------

    /// Generate a deterministic pseudo-random `n×n` matrix of `Fp<3>` elements.
    ///
    /// Uses Knuth's MMIX LCG: `x_{k+1} = a * x_k + c mod 2^64`, then takes
    /// `x mod 3` as the element value. Reproducible across runs.
    fn random_fp3_matrix(n: usize, seed: u64) -> Vec<Fp<3>> {
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
    // Hand-checked test vectors
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Panic tests
    // -----------------------------------------------------------------------

    /// Panics when n > 63 (Gray-code register overflow).
    #[test]
    #[should_panic(
        expected = "permanent_mod3_reference: n = 64 exceeds the single-u64 Gray-code register's n <= 63 bound"
    )]
    fn test_reference_panics_n_exceeds_63() {
        let matrix: Vec<Fp<3>> = vec![Fp::<3>::new(0); 64 * 64];
        let _ = permanent_mod3_reference(&matrix, 64);
    }

    /// Panics when matrix.len() != n * n.
    #[test]
    #[should_panic(
        expected = "permanent_mod3_reference: matrix.len() (3) must equal n * n (4) where n = 2"
    )]
    fn test_reference_panics_shape_mismatch() {
        let matrix: Vec<Fp<3>> = vec![Fp::<3>::new(0); 3];
        let _ = permanent_mod3_reference(&matrix, 2);
    }

    // -----------------------------------------------------------------------
    // Cross-check: permanent_mod3_reference vs permanent_ryser::<Fp<3>>
    // -----------------------------------------------------------------------

    /// Cross-check `permanent_mod3_reference` against `permanent_ryser::<Fp<3>>`
    /// for 100 random matrices at each n in {1, 2, 3, 4, 5}.
    ///
    /// This is the primary correctness criterion for T8: if the two ever differ,
    /// the reference port has a bug. Seeds are derived deterministically from n.
    #[test]
    fn test_reference_cross_check_random_small() {
        for n in 1usize..=5 {
            let seed_base: u64 = 0xA5B6_0000u64.wrapping_add(n as u64);
            for trial in 0..100u64 {
                let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                let mat = random_fp3_matrix(n, seed);
                let reference = permanent_mod3_reference(&mat, n);
                let ryser = permanent_ryser::<Fp<3>>(&mat, n);
                assert_eq!(
                    reference, ryser,
                    "permanent_mod3_reference != permanent_ryser for n={n} trial={trial}"
                );
            }
        }
    }
}
