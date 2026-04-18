//! Karatsuba vs naive cross-verification for tower extensions.
//!
//! Validates that the optimised Karatsuba multiplication in
//! [`QuadraticExt`] (3M) and Karatsuba-style multiplication in
//! [`CubicExt`] (6M) produce identical results to independently-written
//! schoolbook polynomial multiplication (4M for quadratic, 9M for cubic).
//!
//! # Independence
//!
//! The naive reference implementations live in [`common::naive_quadratic_mul`]
//! / [`common::naive_cubic_mul`] (shared with the other tower integration
//! tests). They use only the public accessors of [`QuadraticExt`] /
//! [`CubicExt`] and the base field's own `+`/`*` operators. They deliberately
//! do **not** call the optimised `Mul` impl or any of its helpers, so a bug
//! in the Karatsuba code path cannot silently hide behind a shared
//! subroutine.
//!
//! # Coverage
//!
//! Per the design plan (`dev/plans/karatsuba_cross_verification.md`):
//!
//! | Extension | Base field | β     | Notes                             |
//! |-----------|-----------|-------|-----------------------------------|
//! | Quadratic | Fp<7>      | −1    | Overridden `mul_by_non_residue` (negation) |
//! | Quadratic | Fp<7>      | 3     | Default `mul_by_non_residue`      |
//! | Quadratic | Fp<101>    | −2    | Larger prime                      |
//! | Quadratic | Fp<65537>  | 3     | Fermat prime                      |
//! | Cubic     | Fp<7>      | 3     | Overridden `mul_by_non_residue`   |
//! | Cubic     | Fp<31>     | 11    | Default `mul_by_non_residue`      |
//! | Cubic     | Fp<101>    | 2     | Larger prime                      |
//!
//! # Case counts
//!
//! All tests run 10 000 proptest cases per configuration, as specified
//! in the issue. The `square` consistency tests (`a.square() == naive_mul(a, a)`)
//! also run 10 000 cases each.

use gf2_core::field::{ConstField, FiniteField, FiniteFieldExt};
use gf2_core::gfp::Fp;
use proptest::prelude::*;

mod common;
use common::{
    naive_cubic_mul, naive_quadratic_mul, Fq2Fp101NegTwo, Fq2Fp7NegOne, Fq2Large, Fq2Small,
    Fq3Fp101Beta2, Fq3Fp31Beta11, Fq3Fp7Beta3,
};

// ---------------------------------------------------------------------------
// Unit tests for the naive references — hand-checked against simple examples.
// These confirm the shared naive implementations in `common` are themselves
// correct, before we use them as a reference for the Karatsuba cross-check.
// ---------------------------------------------------------------------------

#[test]
fn test_naive_quadratic_reference_hand_check_neg_one() {
    // (3 + 2u)(4 + 5u) over Fp<7> with β = −1.
    // = 12 + 15u + 8u + 10u² = 12 + 23u − 10 = 2 + 2u (mod 7).
    let a = Fq2Fp7NegOne::new(Fp::new(3), Fp::new(2));
    let b = Fq2Fp7NegOne::new(Fp::new(4), Fp::new(5));
    let c = naive_quadratic_mul(a, b);
    assert_eq!(c.c0().value(), 2);
    assert_eq!(c.c1().value(), 2);
}

#[test]
fn test_naive_quadratic_reference_hand_check_beta_three() {
    // (2 + 3u)(1 + 4u) over Fp<7> with β = 3.
    // = 2 + 8u + 3u + 12u² = 2 + 11u + 36 = 38 + 11u = 3 + 4u (mod 7).
    let a = Fq2Small::new(Fp::new(2), Fp::new(3));
    let b = Fq2Small::new(Fp::new(1), Fp::new(4));
    let c = naive_quadratic_mul(a, b);
    assert_eq!(c.c0().value(), 3);
    assert_eq!(c.c1().value(), 4);
}

#[test]
fn test_naive_quadratic_reference_hand_check_u_squared_is_beta() {
    // u² = β for every config.
    let u = Fq2Fp101NegTwo::new(Fp::new(0), Fp::new(1));
    let u_sq = naive_quadratic_mul(u, u);
    assert_eq!(u_sq, Fq2Fp101NegTwo::from_base(Fp::new(99)));
}

#[test]
fn test_naive_cubic_reference_hand_check_v_cubed_is_beta() {
    // v³ = β. Compute v·v → v², then (v²)·v → v³ = β.
    let v = Fq3Fp7Beta3::new(Fp::new(0), Fp::new(1), Fp::new(0));
    let v_sq = naive_cubic_mul(v, v);
    assert_eq!(v_sq, Fq3Fp7Beta3::new(Fp::new(0), Fp::new(0), Fp::new(1)));

    let v_cubed = naive_cubic_mul(v_sq, v);
    assert_eq!(v_cubed, Fq3Fp7Beta3::from_base(Fp::new(3)));
}

#[test]
fn test_naive_cubic_reference_hand_check_square_of_one_plus_v() {
    // (1 + v)² = 1 + 2v + v² (no reduction needed: degree < 3).
    let a = Fq3Fp7Beta3::new(Fp::new(1), Fp::new(1), Fp::new(0));
    let c = naive_cubic_mul(a, a);
    assert_eq!(c, Fq3Fp7Beta3::new(Fp::new(1), Fp::new(2), Fp::new(1)));
}

#[test]
fn test_naive_cubic_reference_hand_check_general_fp31() {
    // (2 + 3v + 5v²)(1 + 4v + 6v²) with β = 11 over Fp<31>.
    // Schoolbook degree-4 product (computed by hand, then reduced mod v³=11):
    //
    // d0 = 2·1 = 2
    // d1 = 2·4 + 3·1 = 8 + 3 = 11
    // d2 = 2·6 + 3·4 + 5·1 = 12 + 12 + 5 = 29
    // d3 = 3·6 + 5·4 = 18 + 20 = 38
    // d4 = 5·6 = 30
    //
    // c0 = d0 + 11·d3 = 2 + 11·38 = 2 + 418 = 420
    // c1 = d1 + 11·d4 = 11 + 11·30 = 11 + 330 = 341
    // c2 = d2 = 29
    //
    // Reduce mod 31:
    // 420 = 13·31 + 17  → c0 = 17
    // 341 = 11·31 + 0   → c1 = 0
    // 29                → c2 = 29
    let a = Fq3Fp31Beta11::new(Fp::new(2), Fp::new(3), Fp::new(5));
    let b = Fq3Fp31Beta11::new(Fp::new(1), Fp::new(4), Fp::new(6));
    let c = naive_cubic_mul(a, b);
    assert_eq!(c.c0().value(), 17);
    assert_eq!(c.c1().value(), 0);
    assert_eq!(c.c2().value(), 29);
}

// ---------------------------------------------------------------------------
// Property-based tests: Karatsuba == naive for 10 000 random pairs per config.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10_000,
        .. ProptestConfig::default()
    })]

    // -----------------------------------------------------------------------
    // QuadraticExt cross-verification
    // -----------------------------------------------------------------------

    #[test]
    fn test_karatsuba_matches_naive_fp7_neg_one(
        a0 in 0u64..7,
        a1 in 0u64..7,
        b0 in 0u64..7,
        b1 in 0u64..7,
    ) {
        let a = Fq2Fp7NegOne::new(Fp::new(a0), Fp::new(a1));
        let b = Fq2Fp7NegOne::new(Fp::new(b0), Fp::new(b1));
        prop_assert_eq!(a * b, naive_quadratic_mul(a, b));
    }

    #[test]
    fn test_karatsuba_matches_naive_fp7_beta_three(
        a0 in 0u64..7,
        a1 in 0u64..7,
        b0 in 0u64..7,
        b1 in 0u64..7,
    ) {
        let a = Fq2Small::new(Fp::new(a0), Fp::new(a1));
        let b = Fq2Small::new(Fp::new(b0), Fp::new(b1));
        prop_assert_eq!(a * b, naive_quadratic_mul(a, b));
    }

    #[test]
    fn test_karatsuba_matches_naive_fp101_neg_two(
        a0 in 0u64..101,
        a1 in 0u64..101,
        b0 in 0u64..101,
        b1 in 0u64..101,
    ) {
        let a = Fq2Fp101NegTwo::new(Fp::new(a0), Fp::new(a1));
        let b = Fq2Fp101NegTwo::new(Fp::new(b0), Fp::new(b1));
        prop_assert_eq!(a * b, naive_quadratic_mul(a, b));
    }

    #[test]
    fn test_karatsuba_matches_naive_fp65537_beta_three(
        a0 in 0u64..65537,
        a1 in 0u64..65537,
        b0 in 0u64..65537,
        b1 in 0u64..65537,
    ) {
        let a = Fq2Large::new(Fp::new(a0), Fp::new(a1));
        let b = Fq2Large::new(Fp::new(b0), Fp::new(b1));
        prop_assert_eq!(a * b, naive_quadratic_mul(a, b));
    }

    // -----------------------------------------------------------------------
    // CubicExt cross-verification
    // -----------------------------------------------------------------------

    #[test]
    fn test_karatsuba_matches_naive_cubic_fp7_beta_three(
        a0 in 0u64..7,
        a1 in 0u64..7,
        a2 in 0u64..7,
        b0 in 0u64..7,
        b1 in 0u64..7,
        b2 in 0u64..7,
    ) {
        let a = Fq3Fp7Beta3::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
        let b = Fq3Fp7Beta3::new(Fp::new(b0), Fp::new(b1), Fp::new(b2));
        prop_assert_eq!(a * b, naive_cubic_mul(a, b));
    }

    #[test]
    fn test_karatsuba_matches_naive_cubic_fp31_beta_eleven(
        a0 in 0u64..31,
        a1 in 0u64..31,
        a2 in 0u64..31,
        b0 in 0u64..31,
        b1 in 0u64..31,
        b2 in 0u64..31,
    ) {
        let a = Fq3Fp31Beta11::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
        let b = Fq3Fp31Beta11::new(Fp::new(b0), Fp::new(b1), Fp::new(b2));
        prop_assert_eq!(a * b, naive_cubic_mul(a, b));
    }

    #[test]
    fn test_karatsuba_matches_naive_cubic_fp101_beta_two(
        a0 in 0u64..101,
        a1 in 0u64..101,
        a2 in 0u64..101,
        b0 in 0u64..101,
        b1 in 0u64..101,
        b2 in 0u64..101,
    ) {
        let a = Fq3Fp101Beta2::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
        let b = Fq3Fp101Beta2::new(Fp::new(b0), Fp::new(b1), Fp::new(b2));
        prop_assert_eq!(a * b, naive_cubic_mul(a, b));
    }

    // -----------------------------------------------------------------------
    // Squaring consistency: a.square() == naive_mul(a, a).
    //
    // `FiniteFieldExt::square` is a default impl (`self * self`), so this
    // chains Karatsuba multiplication with the naive reference. It provides
    // an extra sanity check that the `square` path agrees with schoolbook.
    // -----------------------------------------------------------------------

    #[test]
    fn test_square_matches_naive_quadratic_fp7_neg_one(
        a0 in 0u64..7,
        a1 in 0u64..7,
    ) {
        let a = Fq2Fp7NegOne::new(Fp::new(a0), Fp::new(a1));
        prop_assert_eq!(a.square(), naive_quadratic_mul(a, a));
    }

    #[test]
    fn test_square_matches_naive_quadratic_fp7_beta_three(
        a0 in 0u64..7,
        a1 in 0u64..7,
    ) {
        let a = Fq2Small::new(Fp::new(a0), Fp::new(a1));
        prop_assert_eq!(a.square(), naive_quadratic_mul(a, a));
    }

    #[test]
    fn test_square_matches_naive_quadratic_fp101_neg_two(
        a0 in 0u64..101,
        a1 in 0u64..101,
    ) {
        let a = Fq2Fp101NegTwo::new(Fp::new(a0), Fp::new(a1));
        prop_assert_eq!(a.square(), naive_quadratic_mul(a, a));
    }

    #[test]
    fn test_square_matches_naive_quadratic_fp65537_beta_three(
        a0 in 0u64..65537,
        a1 in 0u64..65537,
    ) {
        let a = Fq2Large::new(Fp::new(a0), Fp::new(a1));
        prop_assert_eq!(a.square(), naive_quadratic_mul(a, a));
    }

    #[test]
    fn test_square_matches_naive_cubic_fp7_beta_three(
        a0 in 0u64..7,
        a1 in 0u64..7,
        a2 in 0u64..7,
    ) {
        let a = Fq3Fp7Beta3::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
        prop_assert_eq!(a.square(), naive_cubic_mul(a, a));
    }

    #[test]
    fn test_square_matches_naive_cubic_fp31_beta_eleven(
        a0 in 0u64..31,
        a1 in 0u64..31,
        a2 in 0u64..31,
    ) {
        let a = Fq3Fp31Beta11::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
        prop_assert_eq!(a.square(), naive_cubic_mul(a, a));
    }

    #[test]
    fn test_square_matches_naive_cubic_fp101_beta_two(
        a0 in 0u64..101,
        a1 in 0u64..101,
        a2 in 0u64..101,
    ) {
        let a = Fq3Fp101Beta2::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
        prop_assert_eq!(a.square(), naive_cubic_mul(a, a));
    }
}

// ---------------------------------------------------------------------------
// Sanity: zero and one obey the expected laws under the naive reference.
// ---------------------------------------------------------------------------

#[test]
fn test_naive_mul_zero_is_zero_quadratic() {
    let zero = Fq2Fp101NegTwo::zero();
    let a = Fq2Fp101NegTwo::new(Fp::new(42), Fp::new(17));
    assert!(naive_quadratic_mul(a, zero).is_zero());
    assert!(naive_quadratic_mul(zero, a).is_zero());
}

#[test]
fn test_naive_mul_one_is_identity_quadratic() {
    let one = Fq2Fp101NegTwo::one();
    let a = Fq2Fp101NegTwo::new(Fp::new(42), Fp::new(17));
    assert_eq!(naive_quadratic_mul(a, one), a);
    assert_eq!(naive_quadratic_mul(one, a), a);
}

#[test]
fn test_naive_mul_zero_is_zero_cubic() {
    let zero = Fq3Fp31Beta11::zero();
    let a = Fq3Fp31Beta11::new(Fp::new(7), Fp::new(13), Fp::new(21));
    assert!(naive_cubic_mul(a, zero).is_zero());
    assert!(naive_cubic_mul(zero, a).is_zero());
}

#[test]
fn test_naive_mul_one_is_identity_cubic() {
    let one = Fq3Fp31Beta11::one();
    let a = Fq3Fp31Beta11::new(Fp::new(7), Fp::new(13), Fp::new(21));
    assert_eq!(naive_cubic_mul(a, one), a);
    assert_eq!(naive_cubic_mul(one, a), a);
}
