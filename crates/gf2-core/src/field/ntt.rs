//! Radix-2 Number Theoretic Transform (NTT) over a [`TwoAdicField`].
//!
//! [`ntt_inplace`] is the decimation-in-time (Cooley–Tukey) transform behind
//! [`FieldPoly::mul_ntt`](crate::field::FieldPoly::mul_ntt): a bit-reversal
//! permutation, then per stage `s` the butterflies
//! `(u, v) → (u + ω·v, u − ω·v)` over the powers of
//! `F::two_adic_root_of_unity(s)`.

use crate::field::TwoAdicField;

/// In-place radix-2 decimation-in-time NTT over a [`TwoAdicField`].
///
/// `inverse = true` runs the butterflies with `ω^{-1}` and leaves the result
/// unscaled: the round trip `inv(forward(x)) = x` needs a division of each
/// element by `n`. Lengths 0 and 1 are left unchanged.
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

    // Bit-reversal permutation by reverse-increment counter (Gentleman & Sande,
    // 1966); `i < j` swaps each pair once.
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

    // Stage `s` merges blocks of size `m = 2^s`; the twiddle is the primitive
    // `m`-th root of unity, inverted for the inverse transform.
    let mut m = 2usize;
    while m <= n {
        let s = m.trailing_zeros();
        let w_m = if inverse {
            F::two_adic_root_of_unity(s)
                .inv()
                .expect("two-adic root of unity is always invertible (non-zero)")
        } else {
            F::two_adic_root_of_unity(s)
        };

        let half = m >> 1;
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
            let mut rng = crate::rng::Lcg::new(seed | 1);
            let data: Vec<Fp<65537>> = (0..n)
                .map(|_| Fp::<65537>::new((rng.next_u64() >> 33) % 65537))
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
            // Degree up to 64 ⇒ length up to 65.
            a_len in 0usize..=65,
            b_len in 0usize..=65,
            seed in any::<u64>(),
        ) {
            let mut rng = crate::rng::Lcg::new(seed | 1);
            let mut next = || Fp::<65537>::new((rng.next_u64() >> 33) % 65537);
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
