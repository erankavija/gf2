//! Configuration trait for algebraic field extensions.
//!
//! [`ExtConfig`] specifies the irreducible polynomial for a field extension via
//! a non-residue element β. Extension types like `QuadraticExt<C>` (x² − β) and
//! `CubicExt<C>` (x³ − β) are parameterized by a config type implementing this
//! trait.
//!
//! # Design
//!
//! The config is a zero-sized marker type — no runtime state, no per-element
//! overhead. The non-residue is an associated constant, requiring the base field
//! to support const construction (which `Fp<P>` does via `const fn new()`).
//!
//! # Examples
//!
//! ```
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::ExtConfig;
//!
//! /// GF(7²) via x² + 1 (i.e., x² − (−1), β = −1 = 6 mod 7).
//! struct Fq2Config;
//!
//! impl ExtConfig for Fq2Config {
//!     type BaseField = Fp<7>;
//!     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1
//!
//!     #[inline]
//!     fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
//!         -x // fast path: β = −1
//!     }
//! }
//! ```

use std::fmt;

use crate::field::ConstField;

/// Configuration specifying the irreducible polynomial for a field extension.
///
/// For quadratic extensions (`QuadraticExt<C>`): defines β such that u² = β,
/// giving the irreducible polynomial x² − β.
///
/// For cubic extensions (`CubicExt<C>`): defines β such that v³ = β,
/// giving the irreducible polynomial x³ − β.
///
/// # Type Parameters
///
/// Implementors are zero-sized marker types. The associated `BaseField` is the
/// field being extended, which must implement [`ConstField`] so that extension
/// types can themselves implement `ConstField` for nested towers.
///
/// # Overriding `mul_by_non_residue`
///
/// The default implementation uses generic multiplication, but specific configs
/// can override for efficiency:
/// - β = −1: just negation
/// - β = small constant: shift-and-add
/// - β from a lower tower level: exploit structure
pub trait ExtConfig: 'static {
    /// The base field being extended.
    type BaseField: ConstField + fmt::Display;

    /// The non-residue β defining the extension polynomial.
    ///
    /// For quadratic extensions: the irreducible polynomial is x² − β.
    /// For cubic extensions: the irreducible polynomial is x³ − β.
    const NON_RESIDUE: Self::BaseField;

    /// Multiply a base field element by the non-residue β.
    ///
    /// Default implementation uses generic multiplication. Override for
    /// efficiency when the non-residue has special structure.
    ///
    /// # Arguments
    ///
    /// * `x` - A base field element to multiply by β.
    #[inline]
    fn mul_by_non_residue(x: Self::BaseField) -> Self::BaseField {
        x * Self::NON_RESIDUE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FiniteField;
    use crate::gfp::Fp;

    // -----------------------------------------------------------------------
    // Test configs
    // -----------------------------------------------------------------------

    /// Config for GF(7²) with β = −1 (= 6 mod 7), giving x² + 1.
    struct Fq2NegOneConfig;

    impl ExtConfig for Fq2NegOneConfig {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // −1 mod 7

        #[inline]
        fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
            -x // fast path for β = −1
        }
    }

    /// Config for GF(7²) with β = 3 (a quadratic non-residue mod 7).
    struct Fq2Beta3Config;

    impl ExtConfig for Fq2Beta3Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
        // Uses default mul_by_non_residue (generic multiplication).
    }

    /// Config for GF(13²) with β = 2 (a quadratic non-residue mod 13).
    struct Fp13Ext2Config;

    impl ExtConfig for Fp13Ext2Config {
        type BaseField = Fp<13>;
        const NON_RESIDUE: Fp<13> = Fp::<13>::new(2);
    }

    // -----------------------------------------------------------------------
    // Tests: basic compilation and const non-residue access
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_compiles_and_returns_correct_non_residue() {
        assert_eq!(Fq2NegOneConfig::NON_RESIDUE.value(), 6); // −1 mod 7
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
        // Verify the non-residue can be used in a const context.
        const BETA: Fp<7> = Fq2NegOneConfig::NON_RESIDUE;
        assert_eq!(BETA.value(), 6);
    }

    // -----------------------------------------------------------------------
    // Tests: mul_by_non_residue correctness
    // -----------------------------------------------------------------------

    #[test]
    fn test_mul_by_non_residue_default_matches_manual() {
        // Default impl: x * β. Verify for all elements of GF(7).
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
        // Override impl (-x for β = −1). Verify for all elements of GF(7).
        for i in 0..7u64 {
            let x = Fp::<7>::new(i);
            let expected = x * Fq2NegOneConfig::NON_RESIDUE; // generic: x * 6 mod 7
            let actual = Fq2NegOneConfig::mul_by_non_residue(x); // override: -x
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

    // -----------------------------------------------------------------------
    // Tests: zero-sized config types (no runtime overhead)
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_is_zero_sized() {
        assert_eq!(std::mem::size_of::<Fq2NegOneConfig>(), 0);
        assert_eq!(std::mem::size_of::<Fq2Beta3Config>(), 0);
        assert_eq!(std::mem::size_of::<Fp13Ext2Config>(), 0);
    }

    // -----------------------------------------------------------------------
    // Tests: mul_by_non_residue identity and zero behavior
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Tests: different configs produce distinct types (static dispatch)
    // -----------------------------------------------------------------------

    #[test]
    fn test_distinct_configs_same_base_field() {
        // Two different configs over the same base field should produce
        // different non-residues. This verifies that the type system
        // distinguishes them (they would parameterize different QuadraticExt types).
        assert_ne!(Fq2NegOneConfig::NON_RESIDUE, Fq2Beta3Config::NON_RESIDUE);
    }

    // -----------------------------------------------------------------------
    // Tests: generic usage (static dispatch through type parameter)
    // -----------------------------------------------------------------------

    fn generic_mul_by_non_residue<C: ExtConfig>(x: C::BaseField) -> C::BaseField {
        C::mul_by_non_residue(x)
    }

    #[test]
    fn test_generic_usage() {
        let x = Fp::<7>::new(4);
        let result = generic_mul_by_non_residue::<Fq2Beta3Config>(x);
        // 4 * 3 mod 7 = 12 mod 7 = 5
        assert_eq!(result.value(), 5);
    }
}
