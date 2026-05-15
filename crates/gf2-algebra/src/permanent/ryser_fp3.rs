//! `permanent_ryser_fp3` — F_3-monomorphised Ryser driver for Lean V2
//! extraction.
//!
//! This module exists solely as a Charon/Aeneas extraction target for the
//! D3 Lean proof (JIT issue 0606186a). It re-states the same Gray-code
//! Ryser walk that lives in [`super::ryser::permanent_ryser`] but pinned
//! to `Fp<3>` and operating directly on a row-major `&[Fp<3>]` slice, so
//! that:
//!
//! 1. **No generic trait dispatch** appears in the extracted body. The
//!    Charon `permanent_ryser_fp3` translation contains only `Fp<3>`
//!    arithmetic and `gray_code_iter` indexing — both already verified
//!    via `Gf2Core::Proofs::MontgomeryRoundtrip` (V0) and the D3 proof's
//!    own Gray-code lemmas (V2).
//! 2. **The algorithm body is bit-identical to the generic
//!    `permanent_ryser::<Fp<3>>`** — both follow the same `Gray-code` →
//!    `update col_sum` → `fold product` → `accumulate with sign` →
//!    `apply (-1)^n` pipeline. The bit-identical structure means the
//!    Lean correctness theorem for `permanent_ryser_fp3` transfers
//!    semantically to `permanent_ryser::<F>` for every other valid
//!    `FiniteField` `F`, because the only place the algorithm cares
//!    about the choice of field is at the abstract `+ / - / * / 0 / 1`
//!    operations — which the `FiniteField` axiomatisation (verified
//!    elsewhere) constrains to behave the same way under decoding to
//!    `ZMod P`.
//!
//! The Lean theorem statement is recorded in the D3 sketch
//! (`dev/plans/d3_lean_ryser_sketch.md` §1) and the proof lives at
//! `proofs/Gf2Algebra/Proofs/RyserBounded.lean`.

use gf2_core::gfp::Fp;

/// Compute the permanent of an `n × n` row-major matrix over `F_3` using
/// Ryser's formula in Gray-code subset order. **F_3-monomorphised
/// Charon/Aeneas extraction target for the Lean D3 proof.**
///
/// The body is structurally identical to
/// [`super::ryser::permanent_ryser`] at `F = Fp<3>` — see that function's
/// rustdoc for the full algorithm derivation. This wrapper exists so the
/// Charon LLBC contains a non-generic entrypoint whose only field-typed
/// operations are concrete `Fp<3>` arithmetic, which Aeneas translates
/// cleanly. The Lean correctness theorem
/// `permanent_ryser_fp3_correct` in
/// `proofs/Gf2Algebra/Proofs/RyserBounded.lean` is proved against
/// **this** function (Charon-extracted), not the generic one.
///
/// # Arguments
///
/// * `matrix` — Row-major slice of `n × n` `Fp<3>` entries.
///   `matrix[i * n + j]` is the entry at row `i`, column `j`.
/// * `n` — Matrix dimension. Must satisfy `n <= 63`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::ryser_fp3::permanent_ryser_fp3;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_3: permanent = 1
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// assert_eq!(permanent_ryser_fp3(&id, 2), Fp::<3>::new(1));
///
/// // 2×2 all-ones over F_3: permanent = 2! mod 3 = 2
/// let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 4];
/// assert_eq!(permanent_ryser_fp3(&ones, 2), Fp::<3>::new(2));
/// ```
///
/// # Panics
///
/// Panics if `matrix.len() != n * n`. Panics if `n > 63` (the single-`u64`
/// Gray-code register bound; see [`crate::gray::gray_code_iter`]).
///
/// # Complexity
///
/// `O(n · 2^n)` `Fp<3>` operations, `O(n)` extra space.
pub fn permanent_ryser_fp3(matrix: &[Fp<3>], n: usize) -> Fp<3> {
    assert!(
        n <= 63,
        "permanent_ryser_fp3: n = {} exceeds the single-u64 Gray-code register's n <= 63 bound",
        n,
    );
    assert_eq!(
        matrix.len(),
        n * n,
        "permanent_ryser_fp3: matrix.len() ({}) must equal n * n ({}) where n = {}",
        matrix.len(),
        n * n,
        n,
    );

    // Edge case: the 0×0 matrix has permanent = 1 (vacuous product).
    if n == 0 {
        return Fp::<3>::new(1);
    }

    // col_sum[i] accumulates sum_{j ∈ S} A[i, j] over F_3.
    // Built with a plain index loop (not `(0..n).map(...).collect()`) so the
    // Charon extraction contains no `Iterator::map` closure — Aeneas's Lean
    // model handles plain `Range::next` cleanly but the `Map`-adapter shape
    // bakes an opaque closure type into the loop body.
    let mut col_sum: Vec<Fp<3>> = Vec::with_capacity(n);
    {
        let mut i = 0usize;
        while i < n {
            col_sum.push(Fp::<3>::new(0));
            i += 1;
        }
    }
    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // Gray-code walk inlined as a plain `while k < upper` loop (not
    // `for ... in gray_code_iter(n)`) — same reason as col_sum above:
    // avoids `Iterator::map` in the extracted body.  The widening to
    // `u128` matches `crate::gray::gray_code_iter`'s internal counter
    // type so the bound `1u128 << n` is well-defined for `n <= 63`.
    let upper: u128 = 1u128 << n;
    let mut k: u128 = 1;
    while k < upper {
        let flip = k.trailing_zeros() as usize;
        let g_k = k ^ (k >> 1);
        let parity: i8 = if ((g_k >> flip) & 1) == 1 { 1 } else { -1 };

        // We use explicit `let new = a OP b; a = new` rather than the
        // `OP=`-assign forms because `Fp<3>` only implements `AddAssign`
        // (not `SubAssign` / `MulAssign`).  Allowing
        // `clippy::assign_op_pattern` keeps the Charon extraction body
        // free of `OP=` desugaring on the unsupported ops.
        #[allow(clippy::assign_op_pattern)]
        if parity == 1 {
            subset_size += 1;
            let mut i = 0usize;
            while i < n {
                col_sum[i] = col_sum[i] + matrix[i * n + flip];
                i += 1;
            }
        } else {
            subset_size -= 1;
            let mut i = 0usize;
            while i < n {
                col_sum[i] = col_sum[i] - matrix[i * n + flip];
                i += 1;
            }
        }

        // term = prod_{i=0}^{n-1} col_sum[i].
        let mut term = Fp::<3>::new(1);
        let mut i = 0usize;
        #[allow(clippy::assign_op_pattern)]
        while i < n {
            term = term * col_sum[i];
            i += 1;
        }

        #[allow(clippy::assign_op_pattern)]
        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total = total + term;
        }

        k += 1;
    }

    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permanent::ryser::permanent_ryser;
    use crate::testutil::random_matrix;

    /// Cross-check `permanent_ryser_fp3` against the generic
    /// `permanent_ryser::<Fp<3>>` on small random matrices. The two
    /// implementations are structurally identical so they must agree
    /// bit-for-bit on every input.
    #[test]
    fn test_permanent_ryser_fp3_matches_generic_small() {
        for n in 1..=5usize {
            for trial in 0u64..50 {
                let seed = 0xfa11_0606u64
                    .wrapping_add((n as u64).wrapping_mul(1_000_003))
                    .wrapping_add(trial.wrapping_mul(7_919));
                let mat = random_matrix::<3>(n, seed);
                let mono = permanent_ryser_fp3(&mat, n);
                let generic = permanent_ryser::<Fp<3>>(&mat, n);
                assert_eq!(
                    mono, generic,
                    "mismatch at n={n}, trial={trial}, seed={seed:#018x}"
                );
            }
        }
    }

    /// The 0×0 matrix has permanent = 1.
    #[test]
    fn test_permanent_ryser_fp3_empty() {
        assert_eq!(permanent_ryser_fp3(&[], 0), Fp::<3>::new(1));
    }

    /// A 1×1 matrix `[a]` has permanent = a.
    #[test]
    fn test_permanent_ryser_fp3_1x1() {
        for v in 0u64..3 {
            assert_eq!(permanent_ryser_fp3(&[Fp::<3>::new(v)], 1), Fp::<3>::new(v));
        }
    }

    /// Panics on n > 63.
    #[test]
    #[should_panic(expected = "n <= 63")]
    fn test_permanent_ryser_fp3_panics_on_n_64() {
        let matrix: Vec<Fp<3>> = vec![Fp::<3>::new(0); 64 * 64];
        let _ = permanent_ryser_fp3(&matrix, 64);
    }

    /// Panics on wrong-length matrix.
    #[test]
    #[should_panic(expected = "must equal n * n")]
    fn test_permanent_ryser_fp3_panics_on_wrong_length() {
        let matrix: Vec<Fp<3>> = vec![Fp::<3>::new(0); 5];
        let _ = permanent_ryser_fp3(&matrix, 3);
    }
}
