//! Property-based axiom test harness for [`FiniteField`] implementations.
//!
//! Provides a generic, reusable test suite that verifies all field axioms using
//! proptest's programmatic `TestRunner` API. Adding a new field type requires only
//! writing a `proptest::Strategy` and calling [`test_field_axioms`].
//!
//! # Axioms tested
//!
//! - Additive group: associativity, commutativity, identity, inverse, subtraction consistency
//! - Multiplicative group: associativity, commutativity, identity, inverse, division consistency
//! - Ring: distributivity, zero annihilation
//! - Characteristic: `p` copies of one = zero
//! - Hash consistency: equal elements have equal hashes
//! - Wide accumulator: roundtrip and mul consistency

use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

use crate::field::{ConstField, FiniteField, FiniteFieldExt};
use crate::gf2m::{Gf2mElement, Gf2mElement_, Gf2mField, Gf2mField_, Gf2mWide, Gf2mWideConfig};
use crate::gfp::Fp;

/// Number of random test cases per axiom.
const CASES_PER_AXIOM: u32 = 1000;

fn config() -> ProptestConfig {
    ProptestConfig::with_cases(CASES_PER_AXIOM)
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Strategy that generates uniformly random `Gf2mElement` values (including zero).
fn gf2m_strategy(field: &Gf2mField) -> BoxedStrategy<Gf2mElement> {
    let field = field.clone();
    let max_val = (1u64 << field.degree()) - 1;
    (0..=max_val).prop_map(move |v| field.element(v)).boxed()
}

/// Strategy that generates uniformly random `Gf2mElement_<u128>` values
/// (including zero) for fields with extension degree up to 127.
///
/// Draws two independent `u64`s, concatenates them into a `u128`, and masks to
/// the field degree. This gives full coverage for all catalogued `m` values in
/// the u128 range even though proptest does not natively produce `u128`s.
fn gf2m_u128_strategy(field: &Gf2mField_<u128>) -> BoxedStrategy<Gf2mElement_<u128>> {
    let field_clone = field.clone();
    let m = field.degree();
    (any::<u64>(), any::<u64>())
        .prop_map(move |(hi, lo)| {
            let v = ((hi as u128) << 64) | (lo as u128);
            let mask: u128 = if m >= 128 {
                u128::MAX
            } else {
                (1u128 << m) - 1
            };
            field_clone.element(v & mask)
        })
        .boxed()
}

// ---------------------------------------------------------------------------
// Generic axiom harness
// ---------------------------------------------------------------------------

/// Run the full field axiom test suite for a [`FiniteField`] implementation.
///
/// Drives a `proptest` [`TestRunner`] through every axiom the trait must
/// satisfy: additive group (associativity, commutativity, identity, inverse,
/// subtraction consistency), multiplicative group (associativity,
/// commutativity, identity, inverse, division consistency), ring
/// distributivity, zero annihilation, field characteristic (`p · a = 0`),
/// hash consistency, wide-accumulator roundtrip / mul consistency, and the
/// [`FiniteFieldExt`] extras (`square`, `pow`, Frobenius, Freshman's dream).
/// Used both by the crate's own in-module `#[test]` functions and by
/// integration tests that layer new tower shapes on top of existing base
/// fields.
///
/// # Arguments
///
/// * `strategy` — a `proptest` [`BoxedStrategy<F>`] that samples uniformly
///   random field elements, **including zero** (each axiom either handles
///   zero natively or filters it out locally where needed, e.g., the
///   multiplicative-inverse axiom).
/// * `characteristic` — the prime characteristic `p` of the field as a
///   `u64`. For GF(2ᵐ) this is 2; for `Fp<P>` or towers over it, `P` itself.
///   The value is used both by the characteristic axiom (`p·a = 0`) and by
///   Frobenius / Freshman's-dream checks (`a^p`, `(a+b)^p = a^p + b^p`).
///
/// # Examples
///
/// ```
/// use gf2_core::field::axiom_tests::{fp_strategy, test_field_axioms};
///
/// // Exercise Fp<7> under the canonical axiom harness. Uses the module's
/// // built-in 1000-cases-per-axiom configuration.
/// test_field_axioms(fp_strategy::<7>(), 7);
/// ```
///
/// # Panics
///
/// Panics (via `proptest`'s `TestRunner`) on the first axiom violation it
/// detects — that is the entire point of the harness. A proptest shrink
/// report is included in the panic message.
///
/// # Complexity
///
/// `O(CASES_PER_AXIOM × axiom_count × cost_of_field_ops)` where
/// `CASES_PER_AXIOM` is the module-level constant (currently 1000) and
/// `axiom_count` is roughly 18.
pub fn test_field_axioms<F: FiniteField + Debug>(strategy: BoxedStrategy<F>, characteristic: u64)
where
    F::Characteristic: Into<u64>,
{
    let mut runner = TestRunner::new(config());

    // Additive group
    check_additive_associativity(&mut runner, &strategy);
    check_additive_commutativity(&mut runner, &strategy);
    check_additive_identity(&mut runner, &strategy);
    check_additive_inverse(&mut runner, &strategy);
    check_subtraction_consistency(&mut runner, &strategy);

    // Multiplicative group
    check_multiplicative_associativity(&mut runner, &strategy);
    check_multiplicative_commutativity(&mut runner, &strategy);
    check_multiplicative_identity(&mut runner, &strategy);
    check_multiplicative_inverse(&mut runner, &strategy);
    check_division_consistency(&mut runner, &strategy);

    // Ring axioms
    check_distributivity(&mut runner, &strategy);
    check_zero_annihilation(&mut runner, &strategy);

    // Characteristic
    check_characteristic(&mut runner, &strategy, characteristic);

    // Hash consistency
    check_hash_consistency(&mut runner, &strategy);

    // Wide accumulator
    check_wide_roundtrip(&mut runner, &strategy);
    check_mul_wide_consistency(&mut runner, &strategy);

    // FiniteFieldExt convenience methods
    check_square_consistency(&mut runner, &strategy);
    check_pow_consistency(&mut runner, &strategy);
    check_frobenius_consistency(&mut runner, &strategy, characteristic);
    check_freshman_dream(&mut runner, &strategy, characteristic);
}

/// Run the full axiom suite for a [`ConstField`] implementation.
///
/// Superset of [`test_field_axioms`]: runs every `FiniteField` axiom and
/// additionally checks that the const identities agree with the instance
/// identities (`F::zero().is_zero()`, `F::one().is_one()`) and that
/// `F::order()` matches `p^m` where `m = extension_degree()`. Used by every
/// `Fp<P>` and tower-extension test in the crate.
///
/// # Arguments
///
/// * `strategy` — a `proptest` [`BoxedStrategy<F>`] sampling uniformly over
///   the field, including zero.
/// * `characteristic` — the prime characteristic `p` of the field as a
///   `u64`. For towers, this is the base-prime `P`, not `p^m`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::axiom_tests::{fp_strategy, test_const_field_axioms};
///
/// // Exercise Fp<7> under the full ConstField axiom harness (also checks
/// // `F::order() == 7^1` and const zero/one identities).
/// test_const_field_axioms(fp_strategy::<7>(), 7);
/// ```
///
/// # Panics
///
/// Panics on the first axiom violation the underlying `TestRunner` detects,
/// or if `F::order()` disagrees with `p^m`, or if the const zero/one do not
/// match the predicates. See [`test_field_axioms`] for the axiom catalogue.
///
/// # Complexity
///
/// Same big-O as [`test_field_axioms`]:
/// `O(CASES_PER_AXIOM × axiom_count × cost_of_field_ops)`. The const-only
/// extra checks are three `assert!` calls and contribute a constant.
pub fn test_const_field_axioms<F: ConstField + Debug>(
    strategy: BoxedStrategy<F>,
    characteristic: u64,
) where
    F::Characteristic: Into<u64>,
{
    test_field_axioms(strategy, characteristic);

    assert!(F::zero().is_zero(), "ConstField::zero() must be zero");
    assert!(F::one().is_one(), "ConstField::one() must be one");

    let one = F::one();
    let m = one.extension_degree() as u32;
    let p128 = characteristic as u128;
    let expected_order = p128.pow(m);
    assert_eq!(
        F::order(),
        expected_order,
        "ConstField::order() must be p^m = {}^{} = {}",
        characteristic,
        m,
        expected_order
    );
}

// ---------------------------------------------------------------------------
// Additive group axioms
// ---------------------------------------------------------------------------

fn check_additive_associativity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(
            &(strategy.clone(), strategy.clone(), strategy.clone()),
            |(a, b, c)| {
                let lhs = (a.clone() + b.clone()) + c.clone();
                let rhs = a + (b + c);
                prop_assert_eq!(lhs, rhs, "additive associativity: (a+b)+c != a+(b+c)");
                Ok(())
            },
        )
        .expect("additive associativity");
}

fn check_additive_commutativity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(&(strategy.clone(), strategy.clone()), |(a, b)| {
            prop_assert_eq!(
                a.clone() + b.clone(),
                b + a,
                "additive commutativity: a+b != b+a"
            );
            Ok(())
        })
        .expect("additive commutativity");
}

fn check_additive_identity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let zero = a.zero_like();
            prop_assert_eq!(a.clone() + zero.clone(), a.clone(), "a + 0 != a");
            prop_assert_eq!(zero + a.clone(), a, "0 + a != a");
            Ok(())
        })
        .expect("additive identity");
}

fn check_additive_inverse<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let neg_a = -a.clone();
            prop_assert!((a.clone() + neg_a.clone()).is_zero(), "a + (-a) != 0");
            prop_assert!((neg_a + a).is_zero(), "(-a) + a != 0");
            Ok(())
        })
        .expect("additive inverse");
}

fn check_subtraction_consistency<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(&(strategy.clone(), strategy.clone()), |(a, b)| {
            let sub = a.clone() - b.clone();
            let add_neg = a + (-b);
            prop_assert_eq!(sub, add_neg, "a - b != a + (-b)");
            Ok(())
        })
        .expect("subtraction consistency");
}

// ---------------------------------------------------------------------------
// Multiplicative group axioms
// ---------------------------------------------------------------------------

fn check_multiplicative_associativity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(
            &(strategy.clone(), strategy.clone(), strategy.clone()),
            |(a, b, c)| {
                let lhs = (a.clone() * b.clone()) * c.clone();
                let rhs = a * (b * c);
                prop_assert_eq!(lhs, rhs, "multiplicative associativity: (a*b)*c != a*(b*c)");
                Ok(())
            },
        )
        .expect("multiplicative associativity");
}

fn check_multiplicative_commutativity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(&(strategy.clone(), strategy.clone()), |(a, b)| {
            prop_assert_eq!(
                a.clone() * b.clone(),
                b * a,
                "multiplicative commutativity: a*b != b*a"
            );
            Ok(())
        })
        .expect("multiplicative commutativity");
}

fn check_multiplicative_identity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let one = a.one_like();
            prop_assert_eq!(a.clone() * one.clone(), a.clone(), "a * 1 != a");
            prop_assert_eq!(one * a.clone(), a, "1 * a != a");
            Ok(())
        })
        .expect("multiplicative identity");
}

fn check_multiplicative_inverse<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    // Non-zero elements must have inverses
    let nonzero = strategy.clone().prop_filter("non-zero", |a| !a.is_zero());
    runner
        .run(&nonzero, |a| {
            let inv = a.inv().expect("non-zero element must have inverse");
            prop_assert!((a.clone() * inv.clone()).is_one(), "a * inv(a) != 1");
            prop_assert!((inv * a).is_one(), "inv(a) * a != 1");
            Ok(())
        })
        .expect("multiplicative inverse (non-zero)");

    // Zero must not have an inverse
    runner
        .run(strategy, |a| {
            let zero = a.zero_like();
            prop_assert!(zero.inv().is_none(), "zero must not have an inverse");
            Ok(())
        })
        .expect("zero has no inverse");
}

fn check_division_consistency<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    let nonzero = strategy
        .clone()
        .prop_filter("non-zero divisor", |a| !a.is_zero());
    runner
        .run(&(strategy.clone(), nonzero), |(a, b)| {
            let div_result = a.clone() / b.clone();
            let mul_inv_result = a * b.inv().unwrap();
            prop_assert_eq!(div_result, mul_inv_result, "a/b != a * inv(b)");
            Ok(())
        })
        .expect("division consistency");
}

// ---------------------------------------------------------------------------
// Ring axioms
// ---------------------------------------------------------------------------

fn check_distributivity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(
            &(strategy.clone(), strategy.clone(), strategy.clone()),
            |(a, b, c)| {
                // Left distributivity
                let lhs = a.clone() * (b.clone() + c.clone());
                let rhs = (a.clone() * b.clone()) + (a.clone() * c.clone());
                prop_assert_eq!(lhs, rhs, "left distributivity: a*(b+c) != a*b + a*c");

                // Right distributivity
                let lhs2 = (a.clone() + b.clone()) * c.clone();
                let rhs2 = (a * c.clone()) + (b * c);
                prop_assert_eq!(lhs2, rhs2, "right distributivity: (a+b)*c != a*c + b*c");
                Ok(())
            },
        )
        .expect("distributivity");
}

fn check_zero_annihilation<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let zero = a.zero_like();
            prop_assert!((a.clone() * zero.clone()).is_zero(), "a * 0 != 0");
            prop_assert!((zero * a).is_zero(), "0 * a != 0");
            Ok(())
        })
        .expect("zero annihilation");
}

// ---------------------------------------------------------------------------
// Characteristic
// ---------------------------------------------------------------------------

/// Compute `scalar * elem` using double-and-add in O(log scalar) additions.
fn scalar_mul<F: FiniteField>(elem: &F, scalar: u64) -> F {
    let mut result = elem.zero_like();
    let mut base = elem.clone();
    let mut s = scalar;
    while s > 0 {
        if s & 1 == 1 {
            result += base.clone();
        }
        base = base.clone() + base;
        s >>= 1;
    }
    result
}

fn check_characteristic<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
    p: u64,
) {
    runner
        .run(strategy, |a| {
            // Sum of p copies of one must be zero
            let one = a.one_like();
            let sum_one = scalar_mul(&one, p);
            prop_assert!(
                sum_one.is_zero(),
                "sum of p={} copies of one is not zero",
                p
            );

            // Sum of p copies of any element a must be zero (p·a = 0)
            let sum_a = scalar_mul(&a, p);
            prop_assert!(sum_a.is_zero(), "p·a != 0 for p={}", p);

            Ok(())
        })
        .expect("characteristic");
}

// ---------------------------------------------------------------------------
// Hash consistency
// ---------------------------------------------------------------------------

fn check_hash_consistency<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let b = a.clone();
            prop_assert_eq!(&a, &b, "clone should be equal");
            prop_assert_eq!(
                compute_hash(&a),
                compute_hash(&b),
                "equal elements must have equal hashes"
            );

            // Also verify that a + 0 produces the same hash as a
            let a_plus_zero = a.clone() + a.zero_like();
            prop_assert_eq!(&a, &a_plus_zero, "a + 0 should equal a");
            prop_assert_eq!(
                compute_hash(&a),
                compute_hash(&a_plus_zero),
                "a and a+0 must have equal hashes"
            );

            Ok(())
        })
        .expect("hash consistency");
}

fn compute_hash<T: Hash>(val: &T) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    val.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Wide accumulator
// ---------------------------------------------------------------------------

fn check_wide_roundtrip<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let wide = a.to_wide();
            let back = F::reduce_wide(&wide);
            prop_assert_eq!(back, a, "reduce_wide(to_wide(a)) != a");
            Ok(())
        })
        .expect("wide roundtrip");
}

fn check_mul_wide_consistency<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(&(strategy.clone(), strategy.clone()), |(a, b)| {
            let wide_product = a.mul_to_wide(&b);
            let reduced = F::reduce_wide(&wide_product);
            let direct = a * b;
            prop_assert_eq!(reduced, direct, "reduce_wide(mul_to_wide(a,b)) != a * b");
            Ok(())
        })
        .expect("mul_to_wide consistency");
}

// ---------------------------------------------------------------------------
// FiniteFieldExt axioms
// ---------------------------------------------------------------------------

fn check_square_consistency<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let sq = a.square();
            let mul = a.clone() * a;
            prop_assert_eq!(sq, mul, "square(a) != a * a");
            Ok(())
        })
        .expect("square consistency");
}

fn check_pow_consistency<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            // pow(0) == 1
            prop_assert!(a.pow(0).is_one(), "a.pow(0) != 1");

            // pow(1) == a
            prop_assert_eq!(a.pow(1), a.clone(), "a.pow(1) != a");

            // pow(a+b) == pow(a) * pow(b)
            let exp_a = 3u64;
            let exp_b = 5u64;
            let lhs = a.pow(exp_a + exp_b);
            let rhs = a.pow(exp_a) * a.pow(exp_b);
            prop_assert_eq!(lhs, rhs, "a.pow(a+b) != a.pow(a) * a.pow(b)");

            Ok(())
        })
        .expect("pow consistency");
}

fn check_frobenius_consistency<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
    p: u64,
) where
    F::Characteristic: Into<u64>,
{
    runner
        .run(strategy, |a| {
            // frobenius(1) == a^p
            let frob = a.frobenius(1);
            let pow_p = a.pow(p);
            prop_assert_eq!(frob, pow_p, "frobenius(1) != a^p");
            Ok(())
        })
        .expect("frobenius consistency");
}

fn check_freshman_dream<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
    p: u64,
) where
    F::Characteristic: Into<u64>,
{
    runner
        .run(&(strategy.clone(), strategy.clone()), |(a, b)| {
            // Freshman's dream: (a + b)^p == a^p + b^p
            let lhs = (a.clone() + b.clone()).pow(p);
            let rhs = a.pow(p) + b.pow(p);
            prop_assert_eq!(lhs, rhs, "Freshman's dream: (a+b)^p != a^p + b^p");
            Ok(())
        })
        .expect("Freshman's dream");
}

// ---------------------------------------------------------------------------
// Concrete tests for Gf2mElement
// ---------------------------------------------------------------------------

#[test]
fn test_gf2_4_field_axioms() {
    let field = Gf2mField::new(4, 0b10011);
    test_field_axioms(gf2m_strategy(&field), 2);
}

#[test]
fn test_gf2_8_field_axioms() {
    let field = Gf2mField::gf256();
    test_field_axioms(gf2m_strategy(&field), 2);
}

#[test]
fn test_gf2_16_field_axioms() {
    let field = Gf2mField::gf65536();
    test_field_axioms(gf2m_strategy(&field), 2);
}

// ---------------------------------------------------------------------------
// Concrete tests for Gf2mElement_<u128> at the full m = 17..=127 range
// ---------------------------------------------------------------------------

/// Helper: build a `Gf2mField_<u128>` using the catalogued standard polynomial.
#[cfg(test)]
fn gf2m_u128_field_from_standard(m: usize) -> Gf2mField_<u128> {
    let poly = crate::primitive_polys::PrimitivePolynomialDatabase::standard_u128(m)
        .unwrap_or_else(|| panic!("no u128 polynomial catalogued for m={}", m));
    Gf2mField_::<u128>::new(m, poly)
}

#[test]
fn test_gf2_64_u128_field_axioms() {
    let field = gf2m_u128_field_from_standard(64);
    test_field_axioms(gf2m_u128_strategy(&field), 2);
}

#[test]
fn test_gf2_80_u128_field_axioms() {
    let field = gf2m_u128_field_from_standard(80);
    test_field_axioms(gf2m_u128_strategy(&field), 2);
}

#[test]
fn test_gf2_100_u128_field_axioms() {
    let field = gf2m_u128_field_from_standard(100);
    test_field_axioms(gf2m_u128_strategy(&field), 2);
}

#[test]
fn test_gf2_127_u128_field_axioms() {
    let field = gf2m_u128_field_from_standard(127);
    test_field_axioms(gf2m_u128_strategy(&field), 2);
}

/// Also exercise `Gf2mField_<u128>` at a small `m` that fits in `u64` to prove
/// the generic backend routes correctly regardless of storage width.
#[test]
fn test_gf2_8_via_u128_field_axioms() {
    let field = Gf2mField_::<u128>::new(8, 0b100011101);
    test_field_axioms(gf2m_u128_strategy(&field), 2);
}

// ---------------------------------------------------------------------------
// Strategies and concrete tests for Fp<P>
// ---------------------------------------------------------------------------

/// Strategy that generates uniformly random [`Fp<P>`] values (including zero).
///
/// The canonical sampler used by every prime-field axiom test in the crate
/// and by tower-extension integration tests that need to sample Fp
/// coefficients. Each draw is a uniform `u64` in `0..P`, then lifted through
/// `Fp::<P>::new`, so zero is produced with probability `1/P`.
///
/// # Arguments
///
/// * `P` — the const-generic prime modulus. Must be a prime so that
///   `Fp<P>` is actually a field; the strategy does not itself check
///   primality (the [`Fp`] construction does).
///
/// # Examples
///
/// ```
/// use gf2_core::field::axiom_tests::{fp_strategy, test_field_axioms};
///
/// // Compose the strategy with the axiom harness to exercise Fp<7>.
/// test_field_axioms(fp_strategy::<7>(), 7);
/// ```
///
/// # Panics
///
/// None. The strategy is infallible: every sampled `u64` in `0..P` is a
/// valid input to `Fp::<P>::new`.
///
/// # Complexity
///
/// Constant time per sample.
pub fn fp_strategy<const P: u64>() -> BoxedStrategy<Fp<P>> {
    (0..P).prop_map(Fp::<P>::new).boxed()
}

#[test]
fn test_fp2_const_field_axioms() {
    test_const_field_axioms(fp_strategy::<2>(), 2);
}

#[test]
fn test_fp3_const_field_axioms() {
    test_const_field_axioms(fp_strategy::<3>(), 3);
}

#[test]
fn test_fp5_const_field_axioms() {
    test_const_field_axioms(fp_strategy::<5>(), 5);
}

#[test]
fn test_fp7_const_field_axioms() {
    test_const_field_axioms(fp_strategy::<7>(), 7);
}

#[test]
fn test_fp65537_const_field_axioms() {
    test_const_field_axioms(fp_strategy::<65537>(), 65537);
}

#[test]
fn test_fp_mersenne61_const_field_axioms() {
    test_const_field_axioms(fp_strategy::<2305843009213693951>(), 2305843009213693951);
}

// ---------------------------------------------------------------------------
// Specialized primes: axiom coverage
// ---------------------------------------------------------------------------

/// `Fp<P>` for the 31-bit Mersenne prime `2^31 - 1` (compile-time specialised).
#[test]
fn test_fp_mersenne31_const_field_axioms() {
    const M31: u64 = (1u64 << 31) - 1;
    test_const_field_axioms(fp_strategy::<M31>(), M31);
}

/// `Fp<P>` for the BabyBear Proth prime `15·2^27 + 1 = 2013265921`
/// (compile-time specialised; widely used in zk-STARK frameworks).
#[test]
fn test_fp_babybear_const_field_axioms() {
    use crate::field::two_adic::BABYBEAR_P;
    test_const_field_axioms(fp_strategy::<BABYBEAR_P>(), BABYBEAR_P);
}

/// `Fp<P>` for the KoalaBear Proth prime `2^31 - 2^24 + 1 = 2130706433`
/// (compile-time specialised).
#[test]
fn test_fp_koalabear_const_field_axioms() {
    use crate::field::two_adic::KOALABEAR_P;
    test_const_field_axioms(fp_strategy::<KOALABEAR_P>(), KOALABEAR_P);
}

// Goldilocks strategy and axiom test.
use crate::gfp::specialized::{GoldilocksFp, GOLDILOCKS_PRIME};

fn goldilocks_strategy() -> BoxedStrategy<GoldilocksFp> {
    (0..GOLDILOCKS_PRIME).prop_map(GoldilocksFp::new).boxed()
}

// Silence "unused" warnings for the strategies that are only referenced by
// in-crate `#[test]` functions when the module is compiled under
// `feature = "test-support"` without `cfg(test)`.
#[cfg(all(feature = "test-support", not(test)))]
#[allow(dead_code)]
fn _keep_private_strategies_alive() {
    let _ = gf2m_strategy;
    let _ = gf2m_u128_strategy;
    let _ = goldilocks_strategy;
}

#[test]
fn test_goldilocks_const_field_axioms() {
    test_const_field_axioms(goldilocks_strategy(), GOLDILOCKS_PRIME);
}

// ---------------------------------------------------------------------------
// Strategy and concrete tests for Gf2mWide<N, Cfg>
// ---------------------------------------------------------------------------

/// Strategy that generates uniformly random [`Gf2mWide<N, Cfg>`] values
/// (including zero) for any multi-word binary extension field.
///
/// Draws `2 * N` independent `u64` values, pairs them so that adjacent pairs
/// are XOR-mixed for bit coverage, packs the result into a `[u64; N]` array,
/// and delegates to [`Gf2mWide::new`] which tail-masks any bits above
/// `Cfg::M` to zero. The strategy therefore produces uniformly distributed
/// elements across all `2^(Cfg::M)` field elements without over-counting the
/// all-zero element.
///
/// # Arguments
///
/// * `N` — const generic word count; must match `Cfg`'s `N`.
/// * `Cfg` — the [`Gf2mWideConfig`] marker that selects the irreducible
///   polynomial and extension degree.
///
/// # Examples
///
/// ```
/// use gf2_core::field::axiom_tests::{gf2m_wide_strategy, test_field_axioms};
/// use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
///
/// struct Gf2m256DocConfig;
/// impl Gf2mWideConfig<4> for Gf2m256DocConfig {
///     const M: usize = 256;
///     const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
/// }
///
/// // Build the strategy and run a single field-axiom check (the full
/// // harness runs 100 cases per axiom; here we just verify it compiles).
/// let strat = gf2m_wide_strategy::<4, Gf2m256DocConfig>();
/// // Calling test_field_axioms would exercise all axioms; omitted here to
/// // keep the doctest fast.
/// let _ = strat;
/// ```
///
/// # Panics
///
/// None from the strategy itself. [`Gf2mWide::new`] panics in debug builds if
/// its internal `debug_assert!` fires — but since we always pass raw `u64`
/// arrays (which may have high bits), we call `new` rather than `from_words`,
/// so the masking is applied unconditionally.
///
/// # Complexity
///
/// Constant time per sample: `O(N)` XOR operations and one call to
/// `Gf2mWide::new`.
pub fn gf2m_wide_strategy<const N: usize, Cfg: Gf2mWideConfig<N>>(
) -> BoxedStrategy<Gf2mWide<N, Cfg>> {
    // Generate 2*N u64 values. Adjacent pairs are XOR-mixed to pack extra
    // entropy into N words, giving good coverage of all bit positions even
    // though proptest draws u64 values independently.
    proptest::collection::vec(any::<u64>(), 2 * N)
        .prop_map(|vals| {
            // Pack pairs into N words: word[i] = vals[2*i] ^ vals[2*i+1]
            let mut words = [0u64; N];
            for i in 0..N {
                words[i] = vals[2 * i] ^ vals[2 * i + 1];
            }
            Gf2mWide::<N, Cfg>::new(words)
        })
        .boxed()
}

// ---------------------------------------------------------------------------
// Test config: GF(2^256) with irreducible x^256 + x^10 + x^5 + x^2 + 1
// (Seroussi HPL-98-135 Table 1, m = 256).
// ---------------------------------------------------------------------------

/// GF(2^256) test configuration.
///
/// Irreducible polynomial: `x^256 + x^10 + x^5 + x^2 + 1`.
/// Cited from Seroussi, "Table of Low-Weight Binary Irreducible Polynomials",
/// HP Laboratories technical report HPL-98-135 (1998), Table 1 row m = 256.
///
/// Low-order bits: `x^10 + x^5 + x^2 + 1 = 1024 + 32 + 4 + 1 = 1061 = 0x425`.
#[cfg(test)]
struct Gf2m256TestConfig;

#[cfg(test)]
impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    const M: usize = 256;
    /// `x^10 + x^5 + x^2 + 1 = 0x425`; high bit at position 256 is implicit.
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    const NAME: &'static str = "Gf2m256TestConfig";
}

/// Axiom-harness integration test for `Gf2mWide<4, Gf2m256TestConfig>`.
///
/// Uses `test_field_axioms` (not `test_const_field_axioms`) because
/// `ConstField::order()` panics for `M >= 128`; that limitation is separately
/// documented by `test_order_panics_at_m256`. Runs 100 proptest cases per
/// axiom so the full workspace suite stays within the 60-second budget.
#[test]
fn test_axioms_gf2m_wide_256() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig::with_cases(100));

    // Run all field axioms using the 100-case local runner.
    let strategy = gf2m_wide_strategy::<4, Gf2m256TestConfig>();
    let characteristic = 2u64;

    // Additive group
    check_additive_associativity(&mut runner, &strategy);
    check_additive_commutativity(&mut runner, &strategy);
    check_additive_identity(&mut runner, &strategy);
    check_additive_inverse(&mut runner, &strategy);
    check_subtraction_consistency(&mut runner, &strategy);

    // Multiplicative group
    check_multiplicative_associativity(&mut runner, &strategy);
    check_multiplicative_commutativity(&mut runner, &strategy);
    check_multiplicative_identity(&mut runner, &strategy);
    check_multiplicative_inverse(&mut runner, &strategy);
    check_division_consistency(&mut runner, &strategy);

    // Ring axioms
    check_distributivity(&mut runner, &strategy);
    check_zero_annihilation(&mut runner, &strategy);

    // Characteristic
    check_characteristic(&mut runner, &strategy, characteristic);

    // Hash consistency
    check_hash_consistency(&mut runner, &strategy);

    // Wide accumulator
    check_wide_roundtrip(&mut runner, &strategy);
    check_mul_wide_consistency(&mut runner, &strategy);

    // FiniteFieldExt convenience methods
    check_square_consistency(&mut runner, &strategy);
    check_pow_consistency(&mut runner, &strategy);
    check_frobenius_consistency(&mut runner, &strategy, characteristic);
    check_freshman_dream(&mut runner, &strategy, characteristic);
}

/// Verify that `zero()` and `one()` constants satisfy their predicates for
/// `Gf2mWide<4, Gf2m256TestConfig>` without calling `order()`, which panics
/// for `M >= 128`.
#[test]
fn test_const_field_zero_one_gf2m_wide_256() {
    assert!(
        <Gf2mWide<4, Gf2m256TestConfig> as crate::field::ConstField>::zero().is_zero(),
        "Gf2mWide::<4, Gf2m256TestConfig>::zero() must satisfy is_zero()"
    );
    assert!(
        <Gf2mWide<4, Gf2m256TestConfig> as crate::field::ConstField>::one().is_one(),
        "Gf2mWide::<4, Gf2m256TestConfig>::one() must satisfy is_one()"
    );
}

/// Documents the u128 overflow limitation of `ConstField::order()` for large
/// extension degrees.
///
/// `order()` on `Gf2mWide<N, Cfg>` panics for `Cfg::M >= 128` because `2^M`
/// does not fit in a `u128`. Callers that need the order for large fields
/// should use `Cfg::M` directly.
#[test]
#[should_panic(expected = "Gf2mWide::order exceeds u128 for M = 256")]
fn test_order_panics_at_m256() {
    let _ = <Gf2mWide<4, Gf2m256TestConfig> as crate::field::ConstField>::order();
}

/// Stress variant of the GF(2^256) axiom harness at 1000 cases per axiom.
///
/// Skipped by default; run with `cargo test -- --ignored` for thoroughness.
/// Expected wall-clock time: ~2 seconds on a reference host with `--release`.
#[test]
#[ignore]
fn test_axioms_gf2m_wide_256_stress() {
    test_field_axioms(gf2m_wide_strategy::<4, Gf2m256TestConfig>(), 2);
}
