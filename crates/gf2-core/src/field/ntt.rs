//! Radix-2 Number Theoretic Transform (NTT) over a [`TwoAdicField`].
//!
//! This module provides the low-level forward / inverse NTT primitive
//! [`ntt_inplace`] used by the fast polynomial multiplication path
//! [`FieldPoly::mul_ntt`](crate::field::FieldPoly::mul_ntt) (a.k.a. the
//! free function [`mul_fast`](crate::field::poly::mul_fast)).
//!
//! # Algorithm
//!
//! Classical **decimation-in-time** (DIT) radix-2 NTT, identical in shape
//! to the Cooley–Tukey FFT. For a length `n = 2^k` with `k ≤ F::TWO_ADICITY`:
//!
//! 1. **Bit-reversal permutation** — reorder `data` so that position `i`
//!    after the permutation contains the element originally at position
//!    `bit_reverse(i, k)`.
//! 2. **Butterflies** — for each stage `s = 1 .. k`, the half-length
//!    `m/2 = 2^(s-1)` block is combined with its partner via
//!    `(u, v) → (u + ω·v, u − ω·v)`, where `ω` iterates the powers of
//!    the primitive `m`-th root of unity `ω_m = F::two_adic_root_of_unity(s)`.
//!
//! The inverse transform uses `ω_m^{-1}` everywhere; callers that need
//! the usual `F(F^{-1}(x)) = x` identity must additionally scale by
//! `n^{-1}` after the inverse pass. [`FieldPoly::mul_ntt`] does that
//! scaling internally.
//!
//! # Why radix-2 suffices
//!
//! The [`TwoAdicField`] contract guarantees a primitive `2^k`-th root of
//! unity for every `k ≤ TWO_ADICITY`. A radix-2 transform length is
//! therefore always available up to that cap; on `Fp<65537>` that's
//! `n ≤ 2^16 = 65_536` — far beyond any realistic polynomial size we
//! feed through `FieldPoly::mul_ntt`.
//!
//! # NTT-vs-Karatsuba benchmark on `Fp<65537>`
//!
//! Measured on the repo's reference host with
//! `cargo bench -p gf2-core --bench field_poly -- --quick`. Each cell
//! is the median wall-clock time for one call on *two* polynomials of
//! length `n` (degree `n − 1`), so the output length is `2n − 1`. The
//! `speedup` column is `karatsuba_mul / ntt_mul`; values above 1 mean
//! NTT wins.
//!
//! | `n`   | Karatsuba (via `Mul`) | NTT (`mul_ntt`) | speedup |
//! |------:|----------------------:|----------------:|--------:|
//! |    64 |              13.62 µs |        12.89 µs |   1.06× |
//! |   128 |              43.03 µs |        27.68 µs |   1.55× |
//! |   256 |             132.23 µs |        59.82 µs |   2.21× |
//! |   512 |             403.43 µs |       129.20 µs |   3.12× |
//! |  1024 |               1.22 ms |       279.85 µs |   4.37× |
//!
//! Crossover is effectively at `n = 64` — NTT already ties Karatsuba
//! on that size and wins decisively from `n = 128` onwards. The tuned
//! [`NTT_THRESHOLD`](crate::field::poly::NTT_THRESHOLD) is therefore
//! `128`, and [`mul_fast`](crate::field::poly::mul_fast) routes
//! operands whose output length exceeds that constant through
//! [`FieldPoly::mul_ntt`]. Regenerate the table with
//! `cargo bench -p gf2-core --bench field_poly -- --quick`.

use crate::field::TwoAdicField;

/// In-place radix-2 decimation-in-time NTT over a [`TwoAdicField`].
///
/// Runs the forward transform when `inverse = false` and the unscaled
/// inverse transform when `inverse = true`. The inverse variant performs
/// the butterfly pass with `ω^{-1}` but **does not** scale the result by
/// `n^{-1}` — callers that want the round-trip identity
/// `inv(forward(x)) = x` must divide each element by `n` afterwards.
/// [`FieldPoly::mul_ntt`](crate::field::FieldPoly::mul_ntt) handles the
/// `n^{-1}` scaling internally.
///
/// # Arguments
///
/// * `data` — slice of length `n` where `n` is a power of two and
///   `n ≤ 2^F::TWO_ADICITY`. Overwritten in place with the transform.
///   `n = 1` is the identity (and `n = 0` is a no-op).
/// * `inverse` — `false` for the forward transform, `true` for the
///   unscaled inverse transform.
///
/// # Examples
///
/// Roundtrip on `Fp<65537>` (the 2^4-th roots case, `n = 16`):
///
/// ```
/// use gf2_core::field::{FiniteField, ntt::ntt_inplace};
/// use gf2_core::gfp::Fp;
///
/// let mut data: Vec<Fp<65537>> = (0..16u64).map(Fp::<65537>::new).collect();
/// let original = data.clone();
///
/// // Forward NTT followed by inverse NTT (and a final scaling by n^{-1})
/// // recovers the original vector.
/// ntt_inplace(&mut data, false);
/// ntt_inplace(&mut data, true);
/// let n_inv = Fp::<65537>::new(16).inv().unwrap();
/// for x in &mut data {
///     *x = x.clone() * n_inv.clone();
/// }
/// assert_eq!(data, original);
/// ```
///
/// # Panics
///
/// Panics if:
/// - `data.len()` is not a power of two, or
/// - `data.len() > 2^F::TWO_ADICITY` (the field does not host a
///   primitive root of unity at that length).
///
/// # Complexity
///
/// `O(n log n)` field multiplications and additions, `O(1)` additional
/// memory.
pub fn ntt_inplace<F: TwoAdicField>(data: &mut [F], inverse: bool) {
    let n = data.len();
    if n <= 1 {
        // Length 0: no-op; length 1: the transform is the identity.
        return;
    }
    assert!(
        n.is_power_of_two(),
        "ntt_inplace: length must be a power of two, got {n}",
    );
    let log_n = n.trailing_zeros();
    assert!(
        log_n <= F::TWO_ADICITY,
        "ntt_inplace: requested length 2^{log_n} exceeds field two-adicity 2^{}",
        F::TWO_ADICITY,
    );

    // Step 1: bit-reversal permutation. Standard iterative algorithm
    // using the "reverse-increment" counter; see e.g. Gentleman & Sande
    // (1966). Each element swaps with its bit-reversed partner exactly
    // once; we guard with `i < j` to skip the self-swap (`j = i`) and
    // the duplicate-swap (`j < i`) cases.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            data.swap(i, j);
        }
    }

    // Step 2: butterflies. Stage `s` (for `s = 1 ..= log_n`) merges
    // blocks of size `m = 2^s`. The twiddle for that stage is the
    // primitive `m`-th root of unity — for the inverse transform we use
    // its multiplicative inverse (the primitive `m`-th root of unity in
    // the opposite direction is `ω_m^{-1} = ω_m^{m-1}`).
    let mut m = 2usize;
    while m <= n {
        let s = m.trailing_zeros();
        let w_m = if inverse {
            // ω_m^{-1}: either invert directly, or equivalently raise
            // ω_m to the `m − 1`-th power. We invert because
            // `F::inv` is typically cheaper for fields with a fast
            // extended-Euclidean or Fermat-little-theorem implementation.
            F::two_adic_root_of_unity(s)
                .inv()
                .expect("two-adic root of unity is always invertible (non-zero)")
        } else {
            F::two_adic_root_of_unity(s)
        };

        let half = m >> 1;
        // Process each m-sized block.
        let mut k = 0usize;
        while k < n {
            let mut w = data[0].one_like();
            for offset in 0..half {
                let t = w.clone() * data[k + offset + half].clone();
                let u = data[k + offset].clone();
                data[k + offset] = u.clone() + t.clone();
                data[k + offset + half] = u - t;
                w = w * w_m.clone();
            }
            k += m;
        }

        m <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::poly::mul_fast;
    use crate::field::two_adic::{BABYBEAR_P, KOALABEAR_P};
    use crate::field::FieldPoly;
    use crate::gfp::Fp;
    use proptest::prelude::*;

    // --- Length guards ---

    #[test]
    fn test_ntt_inplace_noop_on_empty() {
        let mut data: Vec<Fp<65537>> = Vec::new();
        ntt_inplace(&mut data, false);
        ntt_inplace(&mut data, true);
        assert!(data.is_empty());
    }

    #[test]
    fn test_ntt_inplace_identity_on_length_one() {
        let mut data = vec![Fp::<65537>::new(7)];
        ntt_inplace(&mut data, false);
        assert_eq!(data, vec![Fp::<65537>::new(7)]);
        ntt_inplace(&mut data, true);
        assert_eq!(data, vec![Fp::<65537>::new(7)]);
    }

    #[test]
    #[should_panic(expected = "must be a power of two")]
    fn test_ntt_inplace_panics_on_non_power_of_two() {
        let mut data: Vec<Fp<65537>> = (0..3u64).map(Fp::<65537>::new).collect();
        ntt_inplace(&mut data, false);
    }

    #[test]
    #[should_panic(expected = "exceeds field two-adicity")]
    fn test_ntt_inplace_panics_on_oversize() {
        // Fp<65537> has TWO_ADICITY = 16, so length 2^17 is too big.
        let n: usize = 1 << 17;
        let mut data: Vec<Fp<65537>> = vec![Fp::<65537>::new(0); n];
        ntt_inplace(&mut data, false);
    }

    // --- Concrete round-trip ---

    fn ntt_roundtrip_recovers<F: TwoAdicField + Clone>(data: Vec<F>) {
        let original = data.clone();
        let n = data.len();
        if n == 0 {
            return;
        }
        let mut buf = data;
        ntt_inplace(&mut buf, false);
        ntt_inplace(&mut buf, true);
        // Scale by n^{-1} — construct "n" as repeated `one` additions so
        // that no specific constructor is required.
        let mut n_field = original[0].zero_like();
        let one = original[0].one_like();
        for _ in 0..n {
            n_field += one.clone();
        }
        let n_inv = n_field.inv().expect("n is non-zero in a TwoAdic field");
        for x in &mut buf {
            *x = x.clone() * n_inv.clone();
        }
        assert_eq!(buf, original);
    }

    #[test]
    fn test_roundtrip_fp65537_small() {
        for &log_n in &[0u32, 1, 2, 3, 4, 5, 6] {
            let n = 1usize << log_n;
            let data: Vec<Fp<65537>> = (0..n as u64).map(|v| Fp::<65537>::new(v + 1)).collect();
            ntt_roundtrip_recovers(data);
        }
    }

    #[test]
    fn test_roundtrip_babybear() {
        let data: Vec<Fp<{ BABYBEAR_P }>> = (0..16u64)
            .map(|v| Fp::<{ BABYBEAR_P }>::new(v * 1_000_003 + 1))
            .collect();
        ntt_roundtrip_recovers(data);
    }

    #[test]
    fn test_roundtrip_koalabear() {
        let data: Vec<Fp<{ KOALABEAR_P }>> = (0..32u64)
            .map(|v| Fp::<{ KOALABEAR_P }>::new(v * 2_654_435_761 + 1))
            .collect();
        ntt_roundtrip_recovers(data);
    }

    // --- Proptest: roundtrip recovers the input up to the 1/n scaling. ---

    proptest! {
        #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

        #[test]
        fn proptest_roundtrip_fp65537(
            log_n in 0u32..=8,
            seed in any::<u64>(),
        ) {
            let n = 1usize << log_n;
            let mut s = seed | 1;
            let data: Vec<Fp<65537>> = (0..n)
                .map(|_| {
                    s = s.wrapping_mul(6_364_136_223_846_793_005)
                         .wrapping_add(1_442_695_040_888_963_407);
                    Fp::<65537>::new((s >> 33) % 65537)
                })
                .collect();
            ntt_roundtrip_recovers(data);
        }
    }

    // --- Agreement with Karatsuba via FieldPoly::mul_ntt / mul_fast ---

    #[test]
    fn test_mul_fast_agrees_with_mul_small() {
        // A couple of hand-picked cases that exercise zero / constant /
        // small non-trivial operands.
        let zero: FieldPoly<Fp<65537>> = FieldPoly::zero_like(&Fp::<65537>::new(0));
        let p = FieldPoly::new(vec![
            Fp::<65537>::new(1),
            Fp::<65537>::new(2),
            Fp::<65537>::new(3),
        ]);
        assert_eq!(
            mul_fast(&zero, &p),
            FieldPoly::zero_like(&Fp::<65537>::new(0))
        );
        assert_eq!(
            mul_fast(&p, &zero),
            FieldPoly::zero_like(&Fp::<65537>::new(0))
        );

        // (x + 1)(x + 2) = x^2 + 3x + 2
        let a = FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(1)]);
        let b = FieldPoly::new(vec![Fp::<65537>::new(2), Fp::<65537>::new(1)]);
        let c = mul_fast(&a, &b);
        assert_eq!(
            c,
            FieldPoly::new(vec![
                Fp::<65537>::new(2),
                Fp::<65537>::new(3),
                Fp::<65537>::new(1),
            ])
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 16, ..ProptestConfig::default() })]

        #[test]
        fn proptest_mul_ntt_agrees_with_mul_karatsuba(
            a_len in 0usize..=64,
            b_len in 0usize..=64,
            seed in any::<u64>(),
        ) {
            let mut s = seed | 1;
            let mut next = || {
                s = s.wrapping_mul(6_364_136_223_846_793_005)
                     .wrapping_add(1_442_695_040_888_963_407);
                Fp::<65537>::new((s >> 33) % 65537)
            };
            let a_coeffs: Vec<Fp<65537>> = (0..a_len).map(|_| next()).collect();
            let b_coeffs: Vec<Fp<65537>> = (0..b_len).map(|_| next()).collect();
            let a = FieldPoly::new(a_coeffs);
            let b = FieldPoly::new(b_coeffs);

            // `mul` uses schoolbook / Karatsuba; `mul_ntt` uses the NTT
            // path when both sides are non-empty.
            let reference = a.mul(&b);
            let via_ntt = if a.is_zero() || b.is_zero() {
                FieldPoly::zero_like(&Fp::<65537>::new(0))
            } else {
                a.mul_ntt(&b)
            };
            prop_assert_eq!(reference, via_ntt);
        }
    }
}
