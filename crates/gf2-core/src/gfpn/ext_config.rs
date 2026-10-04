//! [`ExtConfig`]: the base field and non-residue β that define the binomial
//! extensions `x² − β` (`QuadraticExt<C>`) and `x³ − β` (`CubicExt<C>`).

use crate::field::ConstField;

/// Configuration specifying the irreducible polynomial for a field extension.
///
/// For quadratic extensions (`QuadraticExt<C>`): defines β such that u² = β,
/// giving the irreducible polynomial x² − β.
///
/// For cubic extensions (`CubicExt<C>`): defines β such that v³ = β,
/// giving the irreducible polynomial x³ − β.
///
/// The associated `BaseField` must implement [`ConstField`] so that extension
/// types can themselves implement `ConstField` for nested towers. The trait
/// has no `'static` supertrait bound: extension element types only store
/// base-field coefficients, and code that needs a static config can add that
/// bound locally.
pub trait ExtConfig {
    /// The base field being extended.
    ///
    /// Only [`ConstField`] is required at the algebra layer. The
    /// `fmt::Display` bound that individual extension types need (e.g. for
    /// `impl Display for QuadraticExt<C>`) is added as a `where`-clause at
    /// those presentation impls rather than inherited here, so a base field
    /// that does not implement `Display` can still participate in tower
    /// arithmetic.
    type BaseField: ConstField;

    /// The non-residue β defining the extension polynomial.
    ///
    /// For quadratic extensions: the irreducible polynomial is x² − β.
    /// For cubic extensions: the irreducible polynomial is x³ − β.
    #[cfg(not(verify_lean))]
    const NON_RESIDUE: Self::BaseField;

    /// Returns the non-residue β defining the extension polynomial.
    ///
    /// This method-only shape is used only by `scripts/verify-lean.sh` so
    /// Charon (`@/citation/AeneasVerif2026`) can continue extracting the quadratic and cubic arithmetic while
    /// avoiding its associated-const type-checking bug. The uppercase name is
    /// deliberate: Aeneas then generates the same Lean trait field name as the
    /// normal associated const, keeping downstream proof workarounds stable.
    #[cfg(verify_lean)]
    #[allow(non_snake_case)]
    fn NON_RESIDUE() -> Self::BaseField;

    /// Multiply a base field element by the non-residue β.
    ///
    /// Default implementation uses generic multiplication. Override when
    /// the non-residue has special structure (e.g. negation for β = −1).
    #[inline]
    fn mul_by_non_residue(x: Self::BaseField) -> Self::BaseField {
        #[cfg(not(verify_lean))]
        {
            x * Self::NON_RESIDUE
        }
        #[cfg(verify_lean)]
        {
            x * Self::NON_RESIDUE()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FiniteField;
    use crate::gfp::Fp;

    /// Config for GF(7²) with β = −1 (= 6 mod 7), giving x² + 1.
    struct Fq2NegOneConfig;

    impl ExtConfig for Fq2NegOneConfig {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(6);

        #[inline]
        fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
            -x
        }
    }

    /// Config for GF(7²) with β = 3 (a quadratic non-residue mod 7).
    struct Fq2Beta3Config;

    impl ExtConfig for Fq2Beta3Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    /// Config for GF(13²) with β = 2 (a quadratic non-residue mod 13).
    struct Fp13Ext2Config;

    impl ExtConfig for Fp13Ext2Config {
        type BaseField = Fp<13>;
        const NON_RESIDUE: Fp<13> = Fp::<13>::new(2);
    }

    #[test]
    fn test_config_compiles_and_returns_correct_non_residue() {
        assert_eq!(Fq2NegOneConfig::NON_RESIDUE.value(), 6);
    }

    #[test]
    fn test_config_beta3_non_residue() {
        assert_eq!(Fq2Beta3Config::NON_RESIDUE.value(), 3);
    }

    #[test]
    fn test_config_fp13_non_residue() {
        assert_eq!(Fp13Ext2Config::NON_RESIDUE.value(), 2);
    }

    #[test]
    fn test_non_residue_is_const() {
        const BETA: Fp<7> = Fq2NegOneConfig::NON_RESIDUE;
        assert_eq!(BETA.value(), 6);
    }

    #[test]
    fn test_mul_by_non_residue_default_matches_manual() {
        for i in 0..7u64 {
            let x = Fp::<7>::new(i);
            let expected = x * Fq2Beta3Config::NON_RESIDUE;
            let actual = Fq2Beta3Config::mul_by_non_residue(x);
            assert_eq!(
                actual, expected,
                "mul_by_non_residue({}) should equal {} * β",
                i, i
            );
        }
    }

    #[test]
    fn test_mul_by_non_residue_override_matches_manual() {
        for i in 0..7u64 {
            let x = Fp::<7>::new(i);
            let expected = x * Fq2NegOneConfig::NON_RESIDUE;
            let actual = Fq2NegOneConfig::mul_by_non_residue(x);
            assert_eq!(
                actual, expected,
                "overridden mul_by_non_residue({}) should match generic",
                i
            );
        }
    }

    #[test]
    fn test_mul_by_non_residue_fp13_all_elements() {
        for i in 0..13u64 {
            let x = Fp::<13>::new(i);
            let expected = x * Fp13Ext2Config::NON_RESIDUE;
            let actual = Fp13Ext2Config::mul_by_non_residue(x);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_config_is_zero_sized() {
        assert_eq!(std::mem::size_of::<Fq2NegOneConfig>(), 0);
        assert_eq!(std::mem::size_of::<Fq2Beta3Config>(), 0);
        assert_eq!(std::mem::size_of::<Fp13Ext2Config>(), 0);
    }

    #[test]
    fn test_mul_by_non_residue_zero_gives_zero() {
        let zero = Fp::<7>::new(0);
        assert!(Fq2NegOneConfig::mul_by_non_residue(zero).is_zero());
        assert!(Fq2Beta3Config::mul_by_non_residue(zero).is_zero());
    }

    #[test]
    fn test_mul_by_non_residue_one_gives_beta() {
        use crate::field::ConstField;

        let one = Fp::<7>::one();
        assert_eq!(
            Fq2NegOneConfig::mul_by_non_residue(one),
            Fq2NegOneConfig::NON_RESIDUE
        );
        assert_eq!(
            Fq2Beta3Config::mul_by_non_residue(one),
            Fq2Beta3Config::NON_RESIDUE
        );
    }

    #[test]
    fn test_distinct_configs_same_base_field() {
        assert_ne!(Fq2NegOneConfig::NON_RESIDUE, Fq2Beta3Config::NON_RESIDUE);
    }

    fn generic_mul_by_non_residue<C: ExtConfig>(x: C::BaseField) -> C::BaseField {
        C::mul_by_non_residue(x)
    }

    #[test]
    fn test_generic_usage() {
        let x = Fp::<7>::new(4);
        let result = generic_mul_by_non_residue::<Fq2Beta3Config>(x);
        assert_eq!(result.value(), 5);
    }
}
