//! [`TwoAdicField`] gives, for a field whose multiplicative group order is
//! `m · 2^k` with `m` odd, the exponent `k` and a fixed primitive `2^k`-th
//! root of unity; radix-2 NTT lengths are capped at `2^k`. The implementations
//! cover Proth primes, whose exponent [`ProthTwoAdicity`] reads from
//! [`classify`] at compile time.

use crate::field::{FiniteField, FiniteFieldExt};
use crate::gfp::specialized::{classify, PrimeShape};
use crate::gfp::Fp;

/// Finite field whose multiplicative group contains a power-of-two subgroup,
/// as radix-2 NTT butterflies require.
///
/// Binary extension fields `GF(2^m)` have odd multiplicative order, hence
/// two-adicity 0, and do not implement this trait.
pub trait TwoAdicField: FiniteField {
    /// The largest `k` such that `2^k` divides `|F^*|`.
    ///
    /// For a prime field `GF(P)` this is the exponent `k` in the
    /// factorisation `P − 1 = m · 2^k` with `m` odd.
    const TWO_ADICITY: u32;

    /// A fixed generator of the `2^TWO_ADICITY`-th roots-of-unity subgroup.
    ///
    /// All smaller `2^j`-th primitive roots are obtained by squaring
    /// this generator `TWO_ADICITY − j` times — see
    /// [`two_adic_root_of_unity`](Self::two_adic_root_of_unity).
    fn two_adic_generator() -> Self;

    /// A primitive `2^k`-th root of unity, for any `k` in `[0, TWO_ADICITY]`.
    ///
    /// By convention the 2^0-th root of unity is 1, the 2^1-th is −1,
    /// and the 2^TWO_ADICITY-th is [`two_adic_generator`](Self::two_adic_generator).
    ///
    /// # Panics
    ///
    /// Panics if `k > TWO_ADICITY` — a larger power-of-two root of unity
    /// does not exist in the field.
    ///
    /// # Complexity
    ///
    /// `O(TWO_ADICITY − k)` field multiplications.
    fn two_adic_root_of_unity(k: u32) -> Self {
        assert!(
            k <= Self::TWO_ADICITY,
            "requested 2^{k}-th root of unity exceeds field two-adicity {}",
            Self::TWO_ADICITY
        );
        Self::two_adic_generator().pow(1u64 << (Self::TWO_ADICITY - k))
    }
}

/// Compile-time two-adicity of a prime `P`, read from [`classify`].
///
/// [`TWO_ADICITY`](Self::TWO_ADICITY) is `n` for a Proth prime
/// `P = k · 2^n + 1` and `0` for every other shape.
pub struct ProthTwoAdicity<const P: u64>;

impl<const P: u64> ProthTwoAdicity<P> {
    /// Two-adicity of `P` — the largest `n` with `2^n | (P − 1)`, assuming
    /// `P` has Proth shape. Zero otherwise.
    pub const TWO_ADICITY: u32 = match classify(P) {
        PrimeShape::Proth { n, .. } => n,
        _ => 0,
    };
}

// Generators are the `@/citation/Plonky2026` constants: a
// multiplicative generator of F^* raised to the cofactor `(P − 1) / 2^TWO_ADICITY`.

/// `Fp<65537>` — the Fermat prime `2^16 + 1`.
///
/// `P − 1 = 2^16` and `3` is a primitive root mod 65537
/// (`@/citation/HardyWright2008` §7.3), so `3` itself is the primitive
/// 2^16-th root of unity.
impl TwoAdicField for Fp<65537> {
    const TWO_ADICITY: u32 = ProthTwoAdicity::<65537>::TWO_ADICITY;

    fn two_adic_generator() -> Self {
        Fp::<65537>::new(3)
    }
}

/// `Fp<BABYBEAR_P>` where `BABYBEAR_P = 15 · 2^27 + 1 = 2_013_265_921`.
///
/// `TWO_ADICITY = 27`. The `@/citation/Plonky2026` generator `g_mult = 31` is
/// a multiplicative generator of `F^*`; the 2^27-th primitive root of
/// unity is then `31^15 mod P = 440_564_289 = 0x1a42_7a41`.
impl TwoAdicField for Fp<{ BABYBEAR_P }> {
    const TWO_ADICITY: u32 = ProthTwoAdicity::<{ BABYBEAR_P }>::TWO_ADICITY;

    fn two_adic_generator() -> Self {
        Fp::<{ BABYBEAR_P }>::new(0x1a42_7a41)
    }
}

/// `Fp<KOALABEAR_P>` where `KOALABEAR_P = 127 · 2^24 + 1 = 2_130_706_433`.
///
/// `TWO_ADICITY = 24`. The `@/citation/Plonky2026` generator `g_mult = 3` is
/// a multiplicative generator of `F^*`; the 2^24-th primitive root of
/// unity is then `3^127 mod P = 1_791_270_792 = 0x6ac4_9f88`.
impl TwoAdicField for Fp<{ KOALABEAR_P }> {
    const TWO_ADICITY: u32 = ProthTwoAdicity::<{ KOALABEAR_P }>::TWO_ADICITY;

    fn two_adic_generator() -> Self {
        Fp::<{ KOALABEAR_P }>::new(0x6ac4_9f88)
    }
}

/// BabyBear Proth prime: `15 · 2^27 + 1 = 2_013_265_921`.
///
/// The 31-bit BabyBear field of `@/citation/Plonky2026`.
pub const BABYBEAR_P: u64 = 15 * (1u64 << 27) + 1;

/// KoalaBear Proth prime: `127 · 2^24 + 1 = 2_130_706_433`, equivalently
/// `2^31 − 2^24 + 1`.
///
/// The 31-bit KoalaBear field of `@/citation/Plonky2026`.
pub const KOALABEAR_P: u64 = 127 * (1u64 << 24) + 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FiniteFieldExt;

    #[test]
    fn test_two_adicity_constants_match_classify() {
        assert_eq!(<Fp<65537> as TwoAdicField>::TWO_ADICITY, 16);
        assert_eq!(<Fp<{ BABYBEAR_P }> as TwoAdicField>::TWO_ADICITY, 27);
        assert_eq!(<Fp<{ KOALABEAR_P }> as TwoAdicField>::TWO_ADICITY, 24);
    }

    #[test]
    fn test_babybear_p_value_matches_plonky3_constant() {
        assert_eq!(BABYBEAR_P, 2_013_265_921);
    }

    #[test]
    fn test_koalabear_p_value_matches_plonky3_constant() {
        assert_eq!(KOALABEAR_P, 2_130_706_433);
        assert_eq!(KOALABEAR_P, (1u64 << 31) - (1u64 << 24) + 1);
    }

    fn assert_generator_primitive<F: TwoAdicField + Copy>() {
        let g = F::two_adic_generator();
        let one = g.one_like();
        assert!(
            g.pow(1u64 << F::TWO_ADICITY) == one,
            "two_adic_generator^(2^TWO_ADICITY) must equal 1",
        );
        assert!(F::TWO_ADICITY >= 1);
        assert!(
            g.pow(1u64 << (F::TWO_ADICITY - 1)) != one,
            "two_adic_generator^(2^(TWO_ADICITY − 1)) must not equal 1",
        );
    }

    #[test]
    fn test_generator_is_primitive_fp_65537() {
        assert_generator_primitive::<Fp<65537>>();
    }

    #[test]
    fn test_generator_is_primitive_babybear() {
        assert_generator_primitive::<Fp<{ BABYBEAR_P }>>();
    }

    #[test]
    fn test_generator_is_primitive_koalabear() {
        assert_generator_primitive::<Fp<{ KOALABEAR_P }>>();
    }

    fn assert_small_roots<F: TwoAdicField + Copy>() {
        let one = F::two_adic_generator().one_like();

        let w0 = F::two_adic_root_of_unity(0);
        assert_eq!(w0, one, "2^0-th root of unity must be 1");

        let w1 = F::two_adic_root_of_unity(1);
        assert!(w1.pow(2).is_one(), "2^1-th root squared must be 1");
        assert!(w1 != one, "2^1-th root must be primitive (not 1)");

        let w_max = F::two_adic_root_of_unity(F::TWO_ADICITY);
        assert_eq!(
            w_max,
            F::two_adic_generator(),
            "2^TWO_ADICITY-th root must be the canonical generator",
        );

        // For every k ∈ [0, TWO_ADICITY], squaring w_k gives w_{k−1}.
        let mut w = F::two_adic_generator();
        for k in (1..=F::TWO_ADICITY).rev() {
            let expected = F::two_adic_root_of_unity(k);
            assert_eq!(
                w, expected,
                "ladder mismatch: iterated squaring disagrees with root-of-unity accessor at k={k}",
            );
            w = w * w;
        }
        assert_eq!(w, one, "final squaring from the 2^1-th root must give 1",);
    }

    #[test]
    fn test_small_roots_fp_65537() {
        assert_small_roots::<Fp<65537>>();
    }

    #[test]
    fn test_small_roots_babybear() {
        assert_small_roots::<Fp<{ BABYBEAR_P }>>();
    }

    #[test]
    fn test_small_roots_koalabear() {
        assert_small_roots::<Fp<{ KOALABEAR_P }>>();
    }

    #[test]
    #[should_panic(expected = "exceeds field two-adicity")]
    fn test_root_of_unity_panics_when_k_exceeds_two_adicity_fp_65537() {
        let _ = <Fp<65537> as TwoAdicField>::two_adic_root_of_unity(17);
    }

    #[test]
    #[should_panic(expected = "exceeds field two-adicity")]
    fn test_root_of_unity_panics_when_k_exceeds_two_adicity_babybear() {
        let _ = <Fp<{ BABYBEAR_P }> as TwoAdicField>::two_adic_root_of_unity(28);
    }

    #[test]
    #[should_panic(expected = "exceeds field two-adicity")]
    fn test_root_of_unity_panics_when_k_exceeds_two_adicity_koalabear() {
        let _ = <Fp<{ KOALABEAR_P }> as TwoAdicField>::two_adic_root_of_unity(25);
    }

    #[test]
    fn test_generator_value_is_3_fp_65537() {
        assert_eq!(<Fp<65537> as TwoAdicField>::two_adic_generator().value(), 3,);
    }

    #[test]
    fn test_generator_value_matches_plonky3_babybear() {
        assert_eq!(
            <Fp<{ BABYBEAR_P }> as TwoAdicField>::two_adic_generator().value(),
            0x1a42_7a41,
        );
    }

    #[test]
    fn test_generator_value_matches_plonky3_koalabear() {
        assert_eq!(
            <Fp<{ KOALABEAR_P }> as TwoAdicField>::two_adic_generator().value(),
            0x6ac4_9f88,
        );
    }

    #[test]
    fn test_generator_matches_cofactor_exponentiation_fp_65537() {
        // 3 is a primitive root mod 65537 (`@/citation/HardyWright2008` §7.3).
        // Cofactor m = (P-1)/2^16 = 1, so the two-adic generator is 3^1 = 3.
        let g_mult = Fp::<65537>::new(3);
        let cofactor: u64 = (65537u64 - 1) >> 16; // = 1
        let reconstructed = g_mult.pow(cofactor);
        assert_eq!(
            reconstructed,
            <Fp<65537> as TwoAdicField>::two_adic_generator(),
        );
    }

    #[test]
    fn test_generator_matches_cofactor_exponentiation_babybear() {
        // `@/citation/Plonky2026` uses g_mult = 31 as a multiplicative generator.
        // Cofactor m = (P-1)/2^27 = 15.
        let g_mult = Fp::<{ BABYBEAR_P }>::new(31);
        let cofactor: u64 = (BABYBEAR_P - 1) >> 27; // = 15
        let reconstructed = g_mult.pow(cofactor);
        assert_eq!(
            reconstructed,
            <Fp<{ BABYBEAR_P }> as TwoAdicField>::two_adic_generator(),
        );
    }

    #[test]
    fn test_generator_matches_cofactor_exponentiation_koalabear() {
        // `@/citation/Plonky2026` uses g_mult = 3 as a multiplicative generator.
        // Cofactor m = (P-1)/2^24 = 127.
        let g_mult = Fp::<{ KOALABEAR_P }>::new(3);
        let cofactor: u64 = (KOALABEAR_P - 1) >> 24; // = 127
        let reconstructed = g_mult.pow(cofactor);
        assert_eq!(
            reconstructed,
            <Fp<{ KOALABEAR_P }> as TwoAdicField>::two_adic_generator(),
        );
    }
}
