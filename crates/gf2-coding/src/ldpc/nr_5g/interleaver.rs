//! 5G NR rate-matching bit interleaver — 3GPP TS 38.212 clause 5.4.2.2.
//!
//! After bit selection (clause 5.4.2.1, embodied in this crate by the
//! [`Nr5gRateMatchedCode`](super::Nr5gRateMatchedCode) rate-matching surface),
//! the length-`E` rate-matched bit sequence `e_0, e_1, ..., e_{E-1}` is
//! interleaved into `f_0, f_1, ..., f_{E-1}` by a block interleaver
//! parameterised by the modulation order `Q_m` (bits per QAM symbol). This
//! reduces the dependency between adjacent coded bits mapped to the same QAM
//! symbol in a bit-interleaved coded-modulation (BICM) chain.
//!
//! # The mapping (TS 38.212 §5.4.2.2, verbatim)
//!
//! The spec defines, for `Q_m` the modulation order:
//!
//! ```text
//! for j = 0 to E/Q_m - 1
//!     for i = 0 to Q_m - 1
//!         f_{i + j*Q_m} = e_{i*(E/Q_m) + j}
//!     end for
//! end for
//! ```
//!
//! Equivalently, the `E` input bits are written **row by row** into a
//! `Q_m × (E/Q_m)` matrix (row `i`, column `j` holds `e_{i*(E/Q_m)+j}`) and read
//! out **column by column** (`f_{i + j*Q_m}` is row `i` of column `j`). `E` must
//! be divisible by `Q_m` for the matrix to be rectangular; this is guaranteed
//! when `E = target_n` is a multiple of the modulation's bits-per-symbol.
//!
//! [`output_interleaver`] materialises the **gather permutation** `perm` with
//! `perm[i + j*Q_m] = i*(E/Q_m) + j`, so that `f[p] = e[perm[p]]`.
//!
//! # External validation
//!
//! The `perm[i + j*Q_m] = i*(E/Q_m) + j` gather index is byte-for-byte the
//! `generate_out_int` routine in NVIDIA Sionna
//! (`src/sionna/phy/fec/ldpc/encoding.py`, `LDPC5GEncoder.generate_out_int`,
//! main branch as of 2026-06; Apache-2.0), which builds the same permutation
//! with `perm_seq[i + j*num_bits_per_symbol] = i*(n/num_bits_per_symbol) + j`
//! and the inverse via `np.argsort(perm_seq)`. Sionna is the same reference the
//! BG1/BG2 shift tables under `data/ldpc/nr_5g/` are validated against
//! (`data/ldpc/nr_5g/PROVENANCE.md`). The worked example in this module's tests
//! (`Q_m = 2`, `E = 6` → `perm = [0, 3, 1, 4, 2, 5]`) is derived directly from
//! the spec loop above and reproduces Sionna's `generate_out_int(6, 2)`.

use crate::llr::Llr;
use gf2_core::BitVec;

/// Builds the TS 38.212 §5.4.2.2 output-interleaver gather permutation for a
/// length-`e_len` sequence at modulation order `q_m`.
///
/// The returned vector `perm` has length `e_len` and satisfies
/// `perm[i + j*q_m] = i*(e_len/q_m) + j` for `j ∈ [0, e_len/q_m)`,
/// `i ∈ [0, q_m)`. Interleaving is the gather `f[p] = e[perm[p]]`; the spec
/// formula is `f_{i + j*Q_m} = e_{i*(E/Q_m) + j}`.
///
/// # Arguments
///
/// * `e_len` — the rate-matched sequence length `E`. Must be a positive
///   multiple of `q_m`.
/// * `q_m` — the modulation order `Q_m` (bits per QAM symbol), e.g. `2` (QPSK),
///   `4` (16-QAM), `6` (64-QAM), `8` (256-QAM). Must be non-zero.
///
/// # Returns
///
/// The length-`e_len` gather permutation.
///
/// # Panics
///
/// Panics if `q_m == 0` or if `e_len` is not a multiple of `q_m` (the spec
/// requires a rectangular `Q_m × (E/Q_m)` interleaver matrix).
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::interleaver::output_interleaver;
///
/// // Q_m = 2, E = 6: rows = 2, cols = 3. Spec loop yields perm[0,3,1,4,2,5].
/// assert_eq!(output_interleaver(6, 2), vec![0, 3, 1, 4, 2, 5]);
/// ```
///
/// # Complexity
///
/// O(`e_len`).
#[must_use]
pub fn output_interleaver(e_len: usize, q_m: usize) -> Vec<usize> {
    assert!(q_m != 0, "modulation order Q_m must be non-zero");
    assert!(
        e_len % q_m == 0,
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
/// # Arguments
///
/// * `e_len` — the rate-matched sequence length `E`. Must be a positive
///   multiple of `q_m`.
/// * `q_m` — the modulation order `Q_m`. Must be non-zero.
///
/// # Returns
///
/// The length-`e_len` inverse (deinterleaver) gather permutation.
///
/// # Panics
///
/// Panics under the same conditions as [`output_interleaver`].
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::interleaver::{output_interleaver, inverse_interleaver};
///
/// let perm = output_interleaver(6, 2);
/// let inv = inverse_interleaver(6, 2);
/// // inv is the argsort of perm: applying perm then inv is the identity.
/// for p in 0..6 {
///     assert_eq!(inv[perm[p]], p);
/// }
/// ```
///
/// # Complexity
///
/// O(`e_len`).
#[must_use]
pub fn inverse_interleaver(e_len: usize, q_m: usize) -> Vec<usize> {
    let perm = output_interleaver(e_len, q_m);
    let mut inv = vec![0usize; e_len];
    for (p, &src) in perm.iter().enumerate() {
        inv[src] = p;
    }
    inv
}

/// Interleaves a rate-matched bit sequence per TS 38.212 §5.4.2.2.
///
/// Returns `f` where `f[p] = e[perm[p]]` and `perm = output_interleaver(E, q_m)`
/// with `E = e.len()`.
///
/// # Arguments
///
/// * `e` — the rate-matched codeword bits (length `E`, a multiple of `q_m`).
/// * `q_m` — the modulation order `Q_m`.
///
/// # Returns
///
/// The interleaved length-`E` bit sequence.
///
/// # Panics
///
/// Panics if `e.len()` is not a multiple of `q_m`, or `q_m == 0`.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::interleaver::interleave_bits;
/// use gf2_core::BitVec;
///
/// // e = [1,0,0,1,1,0] (bit i set per index), Q_m = 2.
/// let mut e = BitVec::zeros(6);
/// for &i in &[0usize, 3, 4] { e.set(i, true); }
/// // perm = [0,3,1,4,2,5] => f = [e0,e3,e1,e4,e2,e5] = [1,1,0,1,0,0].
/// let f = interleave_bits(&e, 2);
/// let bits: Vec<bool> = (0..6).map(|i| f.get(i)).collect();
/// assert_eq!(bits, vec![true, true, false, true, false, false]);
/// ```
///
/// # Complexity
///
/// O(`E`).
#[must_use]
pub fn interleave_bits(e: &BitVec, q_m: usize) -> BitVec {
    let perm = output_interleaver(e.len(), q_m);
    let mut f = BitVec::with_capacity(e.len());
    for &src in &perm {
        f.push_bit(e.get(src));
    }
    f
}

/// Deinterleaves an LLR sequence per the inverse of TS 38.212 §5.4.2.2.
///
/// The receive path carries soft LLRs, so the deinterleaver operates on `Llr`
/// values: it recovers the rate-matched-order LLRs `e_llr` from the
/// interleaved-order LLRs `f_llr` via `e_llr[p] = f_llr[inv[p]]` where
/// `inv = inverse_interleaver(E, q_m)`. Composing [`interleave_bits`] (forward,
/// in the bit domain) with this inverse (in the LLR domain) is the identity on
/// the bit/LLR positions.
///
/// # Arguments
///
/// * `f_llr` — the interleaved-order LLRs (length `E`, a multiple of `q_m`).
/// * `q_m` — the modulation order `Q_m`.
///
/// # Returns
///
/// The deinterleaved (rate-matched-order) length-`E` LLR sequence.
///
/// # Panics
///
/// Panics if `f_llr.len()` is not a multiple of `q_m`, or `q_m == 0`.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::interleaver::deinterleave_llrs;
/// use gf2_coding::llr::Llr;
///
/// // f = [10,11,12,13,14,15], Q_m = 2; inv = [0,2,4,1,3,5].
/// let f: Vec<Llr> = (10..16).map(|v| Llr::new(v as f32)).collect();
/// let e = deinterleave_llrs(&f, 2);
/// let vals: Vec<f32> = e.iter().map(|l| l.value()).collect();
/// assert_eq!(vals, vec![10.0, 12.0, 14.0, 11.0, 13.0, 15.0]);
/// ```
///
/// # Complexity
///
/// O(`E`).
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
    /// This reproduces Sionna's `generate_out_int(6, 2)` (Apache-2.0).
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

    /// Q_m = 6, E = 12 (rows = 6, cols = 2): the spec loop verbatim.
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

    /// The inverse permutation is the argsort of the forward permutation.
    #[test]
    fn test_inverse_is_argsort() {
        let perm = output_interleaver(12, 4);
        let inv = inverse_interleaver(12, 4);
        for (p, &src) in perm.iter().enumerate() {
            assert_eq!(inv[src], p, "inv must invert perm at position {p}");
        }
    }

    /// Both permutations are bijections (every index appears exactly once).
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
        /// For every supported Q_m and odd/even number of columns, the forward
        /// and inverse permutations are bijections over 0..E.
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
            // Build a distinct-valued LLR sequence as the rate-matched-order e.
            let e: Vec<Llr> = (0..e_len).map(|v| Llr::new(v as f32)).collect();
            // Interleave (gather with perm), as the bit interleaver does.
            let f: Vec<Llr> = perm.iter().map(|&src| e[src]).collect();
            // Deinterleave with the LLR inverse.
            let recovered = deinterleave_llrs(&f, q_m);
            for p in 0..e_len {
                prop_assert_eq!(recovered[p].value(), e[p].value());
            }
        }
    }

    /// `interleave_bits` then bit-domain inverse recovers the original bits.
    #[test]
    fn test_bit_roundtrip_qm6() {
        let q_m = 6;
        let e_len = q_m * 17; // 102 bits, cols = 17 (odd)
        let mut e = BitVec::zeros(e_len);
        for i in (0..e_len).step_by(3) {
            e.set(i, true);
        }
        let f = interleave_bits(&e, q_m);
        // Deinterleave the bits using the inverse permutation directly.
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
