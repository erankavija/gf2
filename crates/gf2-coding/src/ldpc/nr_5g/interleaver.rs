//! 5G NR rate-matching bit interleaver (`@/citation/ThreeGpp2020` clause
//! 5.4.2.2): after bit selection (clause 5.4.2.1,
//! [`Nr5gRateMatchedCode`](super::Nr5gRateMatchedCode)), a block interleaver
//! parameterised by the modulation order `Q_m` permutes the length-`E`
//! rate-matched sequence `e` into `f`.

use crate::llr::Llr;
use gf2_core::BitVec;

/// Builds the `@/citation/ThreeGpp2020` §5.4.2.2 output-interleaver gather permutation for a
/// length-`e_len` sequence at modulation order `q_m`.
///
/// The returned vector `perm` has length `e_len` and satisfies
/// `perm[i + j*q_m] = i*(e_len/q_m) + j` for `j ∈ [0, e_len/q_m)`,
/// `i ∈ [0, q_m)`. Interleaving is the gather `f[p] = e[perm[p]]`; the spec
/// formula is `f_{i + j*Q_m} = e_{i*(E/Q_m) + j}`, which writes `e` row by row
/// into a `Q_m × (E/Q_m)` matrix and reads it column by column. It is the
/// `generate_out_int` routine of `@/citation/Sionna2026`.
///
/// # Panics
///
/// Panics if `q_m == 0` or if `e_len` is not a multiple of `q_m` (the spec
/// requires a rectangular `Q_m × (E/Q_m)` interleaver matrix).
#[must_use]
pub fn output_interleaver(e_len: usize, q_m: usize) -> Vec<usize> {
    assert!(q_m != 0, "modulation order Q_m must be non-zero");
    assert!(
        e_len.is_multiple_of(q_m),
        "rate-matched length E = {e_len} must be a multiple of Q_m = {q_m}"
    );
    let cols = e_len / q_m;
    let mut perm = vec![0usize; e_len];
    for j in 0..cols {
        for i in 0..q_m {
            perm[i + j * q_m] = i * cols + j;
        }
    }
    perm
}

/// Builds the inverse of [`output_interleaver`] — the deinterleaver gather
/// permutation for length `e_len` at modulation order `q_m`.
///
/// The returned `inv` satisfies `inv[perm[p]] = p`, i.e. `inv` is the argsort of
/// `perm`. Deinterleaving an interleaved sequence `f` recovers `e` via the
/// gather `e[p] = f[inv[p]]`.
///
/// # Panics
///
/// Panics under the same conditions as [`output_interleaver`].
#[must_use]
pub fn inverse_interleaver(e_len: usize, q_m: usize) -> Vec<usize> {
    let perm = output_interleaver(e_len, q_m);
    let mut inv = vec![0usize; e_len];
    for (p, &src) in perm.iter().enumerate() {
        inv[src] = p;
    }
    inv
}

/// Interleaves a rate-matched bit sequence per `@/citation/ThreeGpp2020` §5.4.2.2.
///
/// Returns `f` where `f[p] = e[perm[p]]` and `perm = output_interleaver(E, q_m)`
/// with `E = e.len()`.
///
/// # Panics
///
/// Panics if `e.len()` is not a multiple of `q_m`, or `q_m == 0`.
#[must_use]
pub fn interleave_bits(e: &BitVec, q_m: usize) -> BitVec {
    let perm = output_interleaver(e.len(), q_m);
    let mut f = BitVec::with_capacity(e.len());
    for &src in &perm {
        f.push_bit(e.get(src));
    }
    f
}

/// Deinterleaves an LLR sequence per the inverse of `@/citation/ThreeGpp2020` §5.4.2.2.
///
/// Recovers the rate-matched-order LLRs `e_llr` from the interleaved-order
/// LLRs `f_llr` via `e_llr[p] = f_llr[inv[p]]` where
/// `inv = inverse_interleaver(E, q_m)`.
///
/// # Panics
///
/// Panics if `f_llr.len()` is not a multiple of `q_m`, or `q_m == 0`.
#[must_use]
pub fn deinterleave_llrs(f_llr: &[Llr], q_m: usize) -> Vec<Llr> {
    let inv = inverse_interleaver(f_llr.len(), q_m);
    inv.iter().map(|&src| f_llr[src]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The spec's worked example: Q_m = 2, E = 6. Writing e row-by-row into a
    /// 2x3 matrix and reading column-by-column yields perm = [0,3,1,4,2,5].
    /// This reproduces `generate_out_int(6, 2)` of `@/citation/Sionna2026`.
    #[test]
    fn test_worked_example_qm2_e6() {
        assert_eq!(output_interleaver(6, 2), vec![0, 3, 1, 4, 2, 5]);
    }

    /// A second worked example at Q_m = 4, E = 8 (rows = 4, cols = 2).
    /// j=0: f0=e0, f1=e2, f2=e4, f3=e6; j=1: f4=e1, f5=e3, f6=e5, f7=e7.
    #[test]
    fn test_worked_example_qm4_e8() {
        assert_eq!(output_interleaver(8, 4), vec![0, 2, 4, 6, 1, 3, 5, 7]);
    }

    #[test]
    fn test_worked_example_qm6_e12() {
        // j=0: i=0..5 -> f0=e0,f1=e2,f2=e4,f3=e6,f4=e8,f5=e10
        // j=1: i=0..5 -> f6=e1,f7=e3,f8=e5,f9=e7,f10=e9,f11=e11
        assert_eq!(
            output_interleaver(12, 6),
            vec![0, 2, 4, 6, 8, 10, 1, 3, 5, 7, 9, 11]
        );
    }

    #[test]
    #[should_panic(expected = "must be a multiple of Q_m")]
    fn test_non_divisible_panics() {
        let _ = output_interleaver(7, 2);
    }

    #[test]
    #[should_panic(expected = "Q_m must be non-zero")]
    fn test_zero_qm_panics() {
        let _ = output_interleaver(6, 0);
    }

    #[test]
    fn test_inverse_is_argsort() {
        let perm = output_interleaver(12, 4);
        let inv = inverse_interleaver(12, 4);
        for (p, &src) in perm.iter().enumerate() {
            assert_eq!(inv[src], p, "inv must invert perm at position {p}");
        }
    }

    fn assert_bijection(perm: &[usize]) {
        let mut seen = vec![false; perm.len()];
        for &p in perm {
            assert!(p < perm.len(), "index {p} out of range");
            assert!(!seen[p], "index {p} appears twice");
            seen[p] = true;
        }
        assert!(seen.iter().all(|&s| s), "not all indices covered");
    }

    proptest! {
        #[test]
        fn prop_perm_is_bijection(
            q_m in prop::sample::select(vec![2usize, 4, 6, 8]),
            cols in 1usize..200,
        ) {
            let e_len = q_m * cols;
            let perm = output_interleaver(e_len, q_m);
            let inv = inverse_interleaver(e_len, q_m);
            assert_bijection(&perm);
            assert_bijection(&inv);
        }

        /// Forward (bit domain) then inverse (LLR domain) round-trips: an LLR
        /// vector indexed by position, interleaved as bits would be, then
        /// deinterleaved, recovers the original LLR ordering. We model the
        /// bit-domain interleave on the LLR positions by gathering with `perm`,
        /// then deinterleaving with the inverse — the composition is identity.
        #[test]
        fn prop_forward_inverse_identity(
            q_m in prop::sample::select(vec![2usize, 4, 6, 8]),
            cols in 1usize..100,
        ) {
            let e_len = q_m * cols;
            let perm = output_interleaver(e_len, q_m);
            let e: Vec<Llr> = (0..e_len).map(|v| Llr::new(v as f32)).collect();
            let f: Vec<Llr> = perm.iter().map(|&src| e[src]).collect();
            let recovered = deinterleave_llrs(&f, q_m);
            for p in 0..e_len {
                prop_assert_eq!(recovered[p].value(), e[p].value());
            }
        }
    }

    #[test]
    fn test_bit_roundtrip_qm6() {
        let q_m = 6;
        let e_len = q_m * 17; // 102 bits, cols = 17 (odd)
        let mut e = BitVec::zeros(e_len);
        for i in (0..e_len).step_by(3) {
            e.set(i, true);
        }
        let f = interleave_bits(&e, q_m);
        let inv = inverse_interleaver(e_len, q_m);
        let recovered: BitVec = {
            let mut bv = BitVec::with_capacity(e_len);
            for &src in &inv {
                bv.push_bit(f.get(src));
            }
            bv
        };
        for i in 0..e_len {
            assert_eq!(recovered.get(i), e.get(i), "bit {i} must round-trip");
        }
    }
}
