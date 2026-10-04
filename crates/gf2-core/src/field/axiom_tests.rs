//! Property-based law suites for [`FiniteField`] implementations, driven
//! through proptest's `TestRunner`: [`test_field_axioms`] for the field
//! axioms, and [`test_field_identity_laws`] and [`test_extension_laws`] for
//! the [`crate::field::extension`] abstraction.

use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

use crate::field::extension::{
    convert_element, FieldError, FieldExtension, FieldId, FieldIdentity,
};
// The concrete witnesses are named only by the in-module `#[test]` functions,
// which the `test-support` build of this module compiles away.
#[cfg(test)]
use crate::field::extension::{BinaryPrimeExt, ConstExt, TrivialExt};
use crate::field::{ConstField, FiniteField, FiniteFieldExt};
use crate::gf2m::{Gf2mElement, Gf2mElement_, Gf2mField, Gf2mField_, Gf2mWide, Gf2mWideConfig};
use crate::gfp::Fp;
#[cfg(test)]
use crate::gfpn::quotient::{
    ConstGf125, ConstGf16, ConstGf81, Gf125QuotientConfig, Gf16QuotientConfig, Gf81QuotientConfig,
};
#[cfg(test)]
use crate::gfpn::{ConstQuotient, ConstQuotientConfig, QuotientElement, QuotientField};
use crate::gfpn::{CubicExt, ExtConfig, QuadraticExt};

/// Number of random test cases per axiom for the default entry points
/// [`test_field_axioms`] and [`test_const_field_axioms`].
const CASES_PER_AXIOM: u32 = 1000;

/// Number of random test cases per law for the extension entry points
/// [`test_field_identity_laws`] and [`test_extension_laws`]. Lower than
/// [`CASES_PER_AXIOM`] because each extension law drives up to `[E : B]`
/// Frobenius steps, every one of which is a full field exponentiation.
const CASES_PER_EXTENSION_LAW: u32 = 128;

/// Strategy that generates uniformly random `Gf2mElement` values (including zero).
fn gf2m_strategy(field: &Gf2mField) -> BoxedStrategy<Gf2mElement> {
    let field = field.clone();
    let max_val = (1u64 << field.degree()) - 1;
    (0..=max_val).prop_map(move |v| field.element(v)).boxed()
}

/// Strategy that generates uniformly random `Gf2mElement_<u128>` values
/// (including zero) for fields with extension degree up to 127.
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

/// Runs the shared field-law suite for a [`FiniteField`] implementation,
/// including the [`FiniteFieldExt`] laws, with `CASES_PER_AXIOM` cases per
/// law.
///
/// `strategy` samples the field, including zero; `characteristic` is the
/// prime `p`.
///
/// # Panics
///
/// Panics on the first law violation, reporting the shrunk failing input.
pub fn test_field_axioms<F: FiniteField + Debug>(strategy: BoxedStrategy<F>, characteristic: u64)
where
    F::Characteristic: Into<u64>,
{
    test_field_axioms_with_cases(strategy, characteristic, CASES_PER_AXIOM);
}

/// [`test_field_axioms`] with an explicit proptest case count per axiom.
///
/// # Panics
///
/// Panics on the first law violation.
pub fn test_field_axioms_with_cases<F: FiniteField + Debug>(
    strategy: BoxedStrategy<F>,
    characteristic: u64,
    cases: u32,
) where
    F::Characteristic: Into<u64>,
{
    check_additive_associativity(&mut runner(cases), &strategy);
    check_additive_commutativity(&mut runner(cases), &strategy);
    check_additive_identity(&mut runner(cases), &strategy);
    check_additive_inverse(&mut runner(cases), &strategy);
    check_subtraction_consistency(&mut runner(cases), &strategy);

    check_multiplicative_associativity(&mut runner(cases), &strategy);
    check_multiplicative_commutativity(&mut runner(cases), &strategy);
    check_multiplicative_identity(&mut runner(cases), &strategy);
    check_multiplicative_inverse(&mut runner(cases), &strategy);
    check_division_consistency(&mut runner(cases), &strategy);

    check_distributivity(&mut runner(cases), &strategy);
    check_zero_annihilation(&mut runner(cases), &strategy);

    check_characteristic(&mut runner(cases), &strategy, characteristic);

    check_hash_consistency(&mut runner(cases), &strategy);

    check_wide_roundtrip(&mut runner(cases), &strategy);
    check_mul_wide_consistency(&mut runner(cases), &strategy);

    check_square_consistency(&mut runner(cases), &strategy);
    check_pow_consistency(&mut runner(cases), &strategy);
    check_frobenius_consistency(&mut runner(cases), &strategy, characteristic);
    check_freshman_dream(&mut runner(cases), &strategy, characteristic);
}

/// Builds a [`TestRunner`] budgeted for one law.
///
/// A runner counts successes cumulatively across
/// [`TestRunner::run`](proptest::test_runner::TestRunner::run) calls and stops
/// generating once the count reaches its configured case budget, so a runner
/// shared by several laws executes cases for the first one only and returns
/// success for the rest without sampling. Every law therefore gets its own
/// runner, and `cases` is a per-law budget.
fn runner(cases: u32) -> TestRunner {
    TestRunner::new(ProptestConfig::with_cases(cases))
}

/// Runs [`test_field_axioms`] for a [`ConstField`] and additionally checks
/// that `F::zero()` and `F::one()` satisfy the identity predicates and that
/// `F::order()` equals `p^m` for `m = extension_degree()`.
///
/// `characteristic` is the prime `p`, also for a tower.
///
/// # Panics
///
/// Panics on the first violation.
pub fn test_const_field_axioms<F: ConstField + Debug>(
    strategy: BoxedStrategy<F>,
    characteristic: u64,
) where
    F::Characteristic: Into<u64>,
{
    test_const_field_axioms_with_cases(strategy, characteristic, CASES_PER_AXIOM);
}

/// [`test_const_field_axioms`] with an explicit case count per axiom.
///
/// When `F::order_log2() > 127` the order exceeds `u128::MAX` and `order()`
/// would panic, so the check is `order_log2() == ilog2(p) · m` instead.
///
/// # Panics
///
/// Panics on the first violation.
pub fn test_const_field_axioms_with_cases<F: ConstField + Debug>(
    strategy: BoxedStrategy<F>,
    characteristic: u64,
    cases: u32,
) where
    F::Characteristic: Into<u64>,
{
    test_field_axioms_with_cases(strategy, characteristic, cases);

    assert!(F::zero().is_zero(), "ConstField::zero() must be zero");
    assert!(F::one().is_one(), "ConstField::one() must be one");

    let one = F::one();
    let m = one.extension_degree() as u32;
    let order_log2 = F::order_log2();

    if order_log2 <= 127 {
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
    } else {
        // `order()` would panic; `ilog2(p) · m` is exact for characteristic 2.
        let expected_log2 = (characteristic as u128).ilog2() * m;
        assert_eq!(
            order_log2,
            expected_log2,
            "ConstField::order_log2() must be log2(p) * m = {} * {} = {} for fields with order > u128::MAX",
            (characteristic as u128).ilog2(),
            m,
            expected_log2
        );
    }
}

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
    let nonzero = strategy.clone().prop_filter("non-zero", |a| !a.is_zero());
    runner
        .run(&nonzero, |a| {
            let inv = a.inv().expect("non-zero element must have inverse");
            prop_assert!((a.clone() * inv.clone()).is_one(), "a * inv(a) != 1");
            prop_assert!((inv * a).is_one(), "inv(a) * a != 1");
            Ok(())
        })
        .expect("multiplicative inverse (non-zero)");

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

fn check_distributivity<F: FiniteField + Debug>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(
            &(strategy.clone(), strategy.clone(), strategy.clone()),
            |(a, b, c)| {
                let lhs = a.clone() * (b.clone() + c.clone());
                let rhs = (a.clone() * b.clone()) + (a.clone() * c.clone());
                prop_assert_eq!(lhs, rhs, "left distributivity: a*(b+c) != a*b + a*c");

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
            let one = a.one_like();
            let sum_one = scalar_mul(&one, p);
            prop_assert!(
                sum_one.is_zero(),
                "sum of p={} copies of one is not zero",
                p
            );

            let sum_a = scalar_mul(&a, p);
            prop_assert!(sum_a.is_zero(), "p·a != 0 for p={}", p);

            Ok(())
        })
        .expect("characteristic");
}

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
            prop_assert!(a.pow(0).is_one(), "a.pow(0) != 1");

            prop_assert_eq!(a.pow(1), a.clone(), "a.pow(1) != a");

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
            let lhs = (a.clone() + b.clone()).pow(p);
            let rhs = a.pow(p) + b.pow(p);
            prop_assert_eq!(lhs, rhs, "Freshman's dream: (a+b)^p != a^p + b^p");
            Ok(())
        })
        .expect("Freshman's dream");
}

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

/// GF(2^8) over `x^8 + x^4 + x^3 + x + 1` (0x11B).
#[test]
fn test_gf2_8_poly_11b_field_axioms() {
    let field = Gf2mField::new(8, 0b1_0001_1011);
    test_field_axioms(gf2m_strategy(&field), 2);
}

/// GF(2^8) over `x^8 + x^4 + x^3 + x^2 + 1` (0x11D).
#[test]
fn test_gf2_8_poly_11d_field_axioms() {
    let field = Gf2mField::new(8, 0b1_0001_1101);
    test_field_axioms(gf2m_strategy(&field), 2);
}

#[test]
fn test_gf2_16_field_axioms() {
    let field = Gf2mField::gf65536();
    test_field_axioms(gf2m_strategy(&field), 2);
}

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

/// A degree that fits in `u64`, carried in `u128` storage.
#[test]
fn test_gf2_8_via_u128_field_axioms() {
    let field = Gf2mField_::<u128>::new(8, 0b100011101);
    test_field_axioms(gf2m_u128_strategy(&field), 2);
}

/// Strategy that samples [`Fp<P>`] uniformly, including zero.
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

/// `Fp<P>` for the 31-bit Mersenne prime `2^31 - 1` (compile-time specialised).
#[test]
fn test_fp_mersenne31_const_field_axioms() {
    const M31: u64 = (1u64 << 31) - 1;
    test_const_field_axioms(fp_strategy::<M31>(), M31);
}

/// `Fp<P>` for the BabyBear Proth prime `15·2^27 + 1 = 2013265921`
/// (compile-time specialised).
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

/// Strategy that samples [`Gf2mWide<N, Cfg>`] uniformly, including zero.
pub fn gf2m_wide_strategy<const N: usize, Cfg: Gf2mWideConfig<N>>(
) -> BoxedStrategy<Gf2mWide<N, Cfg>> {
    proptest::collection::vec(any::<u64>(), 2 * N)
        .prop_map(|vals| {
            let mut words = [0u64; N];
            for i in 0..N {
                words[i] = vals[2 * i] ^ vals[2 * i + 1];
            }
            Gf2mWide::<N, Cfg>::new(words)
        })
        .boxed()
}

/// GF(2^256) over `x^256 + x^10 + x^5 + x^2 + 1` (`@/citation/Seroussi1998`
/// Table 1, m = 256).
#[cfg(test)]
struct Gf2m256TestConfig;

#[cfg(test)]
impl Gf2mWideConfig<4> for Gf2m256TestConfig {
    const M: usize = 256;
    /// `x^10 + x^5 + x^2 + 1 = 0x425`; high bit at position 256 is implicit.
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    const NAME: &'static str = "Gf2m256TestConfig";
}

/// For `M = 256`, `order_log2() > 127`, so the harness checks `order_log2`
/// in place of `order()`. Runs 100 cases per axiom; the full budget is
/// [`test_axioms_gf2m_wide_256_stress`].
#[test]
fn test_axioms_gf2m_wide_256() {
    test_const_field_axioms_with_cases(gf2m_wide_strategy::<4, Gf2m256TestConfig>(), 2, 100);
}

/// `2^256` does not fit in `u128`; [`ConstField::order_log2`] is the non-panicking probe.
#[test]
#[should_panic(expected = "Gf2mWide::order exceeds u128 for M = 256")]
fn test_order_panics_at_m256() {
    let _ = <Gf2mWide<4, Gf2m256TestConfig> as crate::field::ConstField>::order();
}

/// `test_axioms_gf2m_wide_256` at the full `CASES_PER_AXIOM` budget.
#[test]
#[ignore]
fn test_axioms_gf2m_wide_256_stress() {
    test_const_field_axioms(gf2m_wide_strategy::<4, Gf2m256TestConfig>(), 2);
}

/// GF(2^8) over `x^8 + x^4 + x^3 + x + 1`; low-order bits `0x1b`.
#[cfg(test)]
struct Gf256Poly11bTestConfig;

#[cfg(test)]
impl Gf2mWideConfig<1> for Gf256Poly11bTestConfig {
    const M: usize = 8;
    /// `x^4 + x^3 + x + 1 = 0x1b`; the high bit at position 8 is implicit.
    const MODULUS: [u64; 1] = [0x1b];
    const NAME: &'static str = "Gf256Poly11bTestConfig";
}

/// GF(2^8) over `x^8 + x^4 + x^3 + x^2 + 1`; low-order bits `0x1d`.
#[cfg(test)]
struct Gf256Poly11dTestConfig;

#[cfg(test)]
impl Gf2mWideConfig<1> for Gf256Poly11dTestConfig {
    const M: usize = 8;
    /// `x^4 + x^3 + x^2 + 1 = 0x1d`; the high bit at position 8 is implicit.
    const MODULUS: [u64; 1] = [0x1d];
    const NAME: &'static str = "Gf256Poly11dTestConfig";
}

/// The single-word wide GF(2^8) representation over the 0x11B modulus, the
/// companion of [`test_gf2_8_poly_11b_field_axioms`] on the other
/// representation of the same field.
#[test]
fn test_axioms_gf2m_wide_gf256_poly_11b() {
    test_const_field_axioms(gf2m_wide_strategy::<1, Gf256Poly11bTestConfig>(), 2);
}

/// The single-word wide GF(2^8) representation over the 0x11D modulus.
#[test]
fn test_axioms_gf2m_wide_gf256_poly_11d() {
    test_const_field_axioms(gf2m_wide_strategy::<1, Gf256Poly11dTestConfig>(), 2);
}

/// Strategy that generates uniformly random [`QuadraticExt<C>`] values from a
/// strategy over the base field.
///
/// Draws two independent base-field coefficients and pairs them into
/// `c0 + c1·u`. Composing this with itself samples a nested tower: feeding
/// `quadratic_strategy::<Inner>(fp_strategy::<3>())` into
/// `quadratic_strategy::<Outer>` samples GF(3⁴) presented over GF(3²).
pub fn quadratic_strategy<C>(base: BoxedStrategy<C::BaseField>) -> BoxedStrategy<QuadraticExt<C>>
where
    C: ExtConfig + 'static,
    C::BaseField: 'static,
{
    (base.clone(), base)
        .prop_map(|(c0, c1)| QuadraticExt::<C>::new(c0, c1))
        .boxed()
}

/// Strategy that generates uniformly random [`CubicExt<C>`] values from a
/// strategy over the base field.
///
/// The cubic counterpart of [`quadratic_strategy`]: draws three independent
/// base-field coefficients and assembles `c0 + c1·v + c2·v²`.
pub fn cubic_strategy<C>(base: BoxedStrategy<C::BaseField>) -> BoxedStrategy<CubicExt<C>>
where
    C: ExtConfig + 'static,
    C::BaseField: 'static,
{
    (base.clone(), base.clone(), base)
        .prop_map(|(c0, c1, c2)| CubicExt::<C>::new(c0, c1, c2))
        .boxed()
}

#[cfg(test)]
fn quotient_strategy<F>(
    field: &QuotientField<F>,
    base: BoxedStrategy<F>,
) -> BoxedStrategy<QuotientElement<F>>
where
    F: FieldIdentity + 'static,
{
    let field = field.clone();
    let degree = field.relative_degree();
    proptest::collection::vec(base, degree)
        .prop_map(move |coefficients| {
            field
                .element(coefficients)
                .expect("the strategy draws coefficients from the field's base")
        })
        .boxed()
}

/// The compile-time counterpart of [`quotient_strategy`]: the presentation
/// comes from the type, so only a base-field strategy is needed.
#[cfg(test)]
fn const_quotient_strategy<const R: usize, C>(
    base: BoxedStrategy<C::BaseField>,
) -> BoxedStrategy<ConstQuotient<R, C>>
where
    C: ConstQuotientConfig<R> + 'static,
    C::BaseField: 'static,
{
    proptest::collection::vec(base, R)
        .prop_map(|coefficients| ConstQuotient::<R, C>::reduce(&coefficients))
        .boxed()
}

/// Run the shared [`FieldIdentity`] law suite for one carrier.
///
/// Every carrier that names its algebraic identity runs this suite, which
/// pins the four contracts the rest of the extension machinery relies on:
///
/// - **Identity agreement.** `field_id().degree()` equals
///   [`FiniteField::extension_degree`] and `field_id().characteristic()`
///   equals [`FiniteField::characteristic`].
/// - **Hint agreement.** When [`FieldIdentity::field_id_hint`] answers, it
///   answers with the same identity every element reports.
/// - **Canonical coordinates.** [`FieldIdentity::write_prime_coords`] emits
///   exactly `degree()` values, each in `[0, p)`, and
///   [`FieldIdentity::from_prime_coords`] inverts it; a short vector and an
///   out-of-range coordinate are rejected with the typed errors.
/// - **Conversion and encoding.** [`convert_element`] into the same field is
///   the identity, and `FieldId::decode(id.encode())` round-trips.
///
/// # Panics
///
/// Panics — via a proptest `TestRunner` abort — on the first law violation,
/// with a shrink report in the panic message.
///
/// # Complexity
///
/// `O(cases × degree)` per law: the coordinate and encoding laws are linear
/// in the absolute degree of the field.
pub fn test_field_identity_laws<F: FieldIdentity>(strategy: BoxedStrategy<F>) {
    test_field_identity_laws_with_cases(strategy, CASES_PER_EXTENSION_LAW);
}

/// [`test_field_identity_laws`] with an explicit case count per law.
///
/// # Panics
///
/// Same as [`test_field_identity_laws`].
pub fn test_field_identity_laws_with_cases<F: FieldIdentity>(
    strategy: BoxedStrategy<F>,
    cases: u32,
) {
    check_identity_agrees_with_field(&mut runner(cases), &strategy);
    check_identity_hint_agrees(&mut runner(cases), &strategy);
    check_prime_coordinate_round_trip(&mut runner(cases), &strategy);
    check_prime_coordinate_rejections(&mut runner(cases), &strategy);
    check_convert_element_identity(&mut runner(cases), &strategy);
    check_field_id_encoding_round_trip(&mut runner(cases), &strategy);
}

fn check_identity_agrees_with_field<F: FieldIdentity>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let id = a.field_id();
            prop_assert_eq!(
                id.degree(),
                a.extension_degree(),
                "field_id().degree() != extension_degree()"
            );
            prop_assert_eq!(
                id.characteristic(),
                a.characteristic(),
                "field_id().characteristic() != characteristic()"
            );
            Ok(())
        })
        .expect("field identity agrees with FiniteField");
}

fn check_identity_hint_agrees<F: FieldIdentity>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            if let Some(hint) = F::field_id_hint() {
                prop_assert_eq!(hint, a.field_id(), "field_id_hint() != field_id()");
            }
            Ok(())
        })
        .expect("field identity hint agrees");
}

fn check_prime_coordinate_round_trip<F: FieldIdentity>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let id = a.field_id();
            let p = id.characteristic();
            let mut coords = Vec::new();
            a.write_prime_coords(&mut coords);
            prop_assert_eq!(coords.len(), id.degree(), "coordinate count != degree");
            for (i, c) in coords.iter().enumerate() {
                prop_assert!(*c < p, "coordinate {} = {} is outside [0, {})", i, c, p);
            }
            let rebuilt = a.from_prime_coords(&coords);
            prop_assert_eq!(
                rebuilt,
                Ok(a.clone()),
                "from_prime_coords(write_prime_coords(a)) != a"
            );
            Ok(())
        })
        .expect("prime coordinate round trip");
}

fn check_prime_coordinate_rejections<F: FieldIdentity>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let id = a.field_id();
            let p = id.characteristic();
            let mut coords = Vec::new();
            a.write_prime_coords(&mut coords);

            let mut short = coords.clone();
            short.pop();
            prop_assert_eq!(
                a.from_prime_coords(&short),
                Err(FieldError::CoordinateCountMismatch {
                    expected: id.degree(),
                    found: id.degree() - 1,
                }),
                "a coordinate vector one short must be rejected by count"
            );

            let mut out_of_range = coords;
            out_of_range[0] = p;
            prop_assert_eq!(
                a.from_prime_coords(&out_of_range),
                Err(FieldError::CoordinateOutOfRange {
                    index: 0,
                    value: p,
                    characteristic: p,
                }),
                "a coordinate equal to p must be rejected by range"
            );
            Ok(())
        })
        .expect("prime coordinate rejections");
}

fn check_convert_element_identity<F: FieldIdentity>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            prop_assert_eq!(
                convert_element(&a, &a),
                Ok(a.clone()),
                "convert_element into the same field is not the identity"
            );
            Ok(())
        })
        .expect("convert_element identity");
}

fn check_field_id_encoding_round_trip<F: FieldIdentity>(
    runner: &mut TestRunner,
    strategy: &BoxedStrategy<F>,
) {
    runner
        .run(strategy, |a| {
            let id = a.field_id();
            let decoded = FieldId::decode(&id.encode());
            prop_assert_eq!(decoded, Ok(id), "FieldId::decode(id.encode()) != id");
            Ok(())
        })
        .expect("field id encoding round trip");
}

/// Run the shared [`FieldExtension`] law suite for one extension witness.
///
/// Every witness of the relation `B ⊆ E` runs this suite. It pins the
/// semantics the extension surface fixes:
///
/// - **Structure.** The certificate covers the witness's own pair,
///   `ext_degree() == base_degree() · relative_degree()`, `|E| = |B|^r`,
///   `|E*| = |E| − 1`, and the two identities share a characteristic.
/// - **Embedding.** `embed` is an injective ring homomorphism carrying the
///   base identities to the extension identities.
/// - **Membership and restriction.** `contains(x)` holds exactly when
///   `try_restrict(x)` answers, `try_restrict(embed(a)) == Some(a)`,
///   `embed(restrict(x)) == x` for members, and `restrict` reports
///   [`FieldError::NotInBase`] for non-members.
/// - **Relative Frobenius.** `φ⁰` and `φʳ` are the identity, `φ` is additive
///   and multiplicative, `φ` has period `r`, and its fixed set is exactly the
///   image of `embed`.
///
/// # Panics
///
/// Panics on the first law violation: directly for the structural laws, which
/// need no random input, and through a proptest `TestRunner` abort for the
/// rest.
///
/// # Complexity
///
/// `O(cases × r × d_B × cost_of_pow)`: each Frobenius law drives up to
/// `r · d_B` field exponentiations by the characteristic.
pub fn test_extension_laws<X: FieldExtension>(
    ext: &X,
    base_strategy: BoxedStrategy<X::Base>,
    ext_strategy: BoxedStrategy<X::Ext>,
) {
    test_extension_laws_with_cases(ext, base_strategy, ext_strategy, CASES_PER_EXTENSION_LAW);
}

/// [`test_extension_laws`] with an explicit case count per law.
///
/// # Panics
///
/// Same as [`test_extension_laws`].
pub fn test_extension_laws_with_cases<X: FieldExtension>(
    ext: &X,
    base_strategy: BoxedStrategy<X::Base>,
    ext_strategy: BoxedStrategy<X::Ext>,
    cases: u32,
) {
    check_extension_structure(ext);

    check_embedding_homomorphism(&mut runner(cases), ext, &base_strategy);
    check_embedding_injective(&mut runner(cases), ext, &base_strategy);
    check_restriction_round_trip(&mut runner(cases), ext, &base_strategy);
    check_membership_matches_restriction(&mut runner(cases), ext, &ext_strategy);
    check_relative_frobenius(&mut runner(cases), ext, &ext_strategy);
}

fn check_extension_structure<X: FieldExtension>(ext: &X) {
    let certificate = ext.certificate();
    assert!(
        certificate.matches(ext.base_id(), ext.ext_id()),
        "certificate does not cover the pair it witnesses"
    );

    let r = ext.relative_degree();
    assert!(r >= 1, "relative degree must be at least one");
    assert_eq!(
        certificate.relative_degree(),
        r,
        "certificate and trait disagree on the relative degree"
    );
    assert_eq!(
        ext.ext_degree(),
        ext.base_degree() * r,
        "ext_degree() != base_degree() * relative_degree()"
    );
    assert_eq!(
        ext.base_id().characteristic(),
        ext.ext_id().characteristic(),
        "base and extension characteristics differ"
    );
    assert_eq!(ext.characteristic(), ext.base_id().characteristic());

    if let (Some(base_order), Some(ext_order)) = (ext.base_order(), ext.ext_order()) {
        let expected = base_order
            .checked_pow(u32::try_from(r).expect("relative degree fits in u32"))
            .expect("|B|^r fits in u128 whenever |E| does");
        assert_eq!(ext_order, expected, "|E| != |B|^r");
        assert_eq!(ext.ext_unit_group_order(), Some(ext_order - 1));
    }

    let base_zero = ext.base_zero();
    let base_one = ext.base_one();
    assert!(base_zero.is_zero(), "base_zero() is not zero");
    assert!(base_one.is_one(), "base_one() is not one");
    assert!(ext.ext_zero().is_zero(), "ext_zero() is not zero");
    assert!(ext.ext_one().is_one(), "ext_one() is not one");
    assert_eq!(&base_zero.field_id(), ext.base_id());
    assert_eq!(&ext.ext_zero().field_id(), ext.ext_id());

    assert_eq!(ext.embed(&base_zero), ext.ext_zero(), "embed(0) != 0");
    assert_eq!(ext.embed(&base_one), ext.ext_one(), "embed(1) != 1");
    assert!(ext.contains(&ext.ext_zero()), "zero must lie in the base");
    assert!(ext.contains(&ext.ext_one()), "one must lie in the base");
    assert_eq!(ext.restrict(&ext.ext_zero()), Ok(base_zero));
    assert_eq!(ext.try_restrict(&ext.ext_one()), Some(base_one));
}

fn check_embedding_homomorphism<X: FieldExtension>(
    runner: &mut TestRunner,
    ext: &X,
    base_strategy: &BoxedStrategy<X::Base>,
) {
    runner
        .run(&(base_strategy.clone(), base_strategy.clone()), |(a, b)| {
            let ea = ext.embed(&a);
            let eb = ext.embed(&b);
            prop_assert_eq!(
                ext.embed(&(a.clone() + b.clone())),
                ea.clone() + eb.clone(),
                "embed(a + b) != embed(a) + embed(b)"
            );
            prop_assert_eq!(
                ext.embed(&(a * b)),
                ea * eb,
                "embed(a * b) != embed(a) * embed(b)"
            );
            Ok(())
        })
        .expect("embedding is a ring homomorphism");
}

fn check_embedding_injective<X: FieldExtension>(
    runner: &mut TestRunner,
    ext: &X,
    base_strategy: &BoxedStrategy<X::Base>,
) {
    runner
        .run(&(base_strategy.clone(), base_strategy.clone()), |(a, b)| {
            if a != b {
                prop_assert_ne!(ext.embed(&a), ext.embed(&b), "embed is not injective");
            }
            Ok(())
        })
        .expect("embedding is injective");
}

fn check_restriction_round_trip<X: FieldExtension>(
    runner: &mut TestRunner,
    ext: &X,
    base_strategy: &BoxedStrategy<X::Base>,
) {
    runner
        .run(base_strategy, |a| {
            let embedded = ext.embed(&a);
            prop_assert!(
                ext.contains(&embedded),
                "the image of embed must lie in the base"
            );
            prop_assert_eq!(
                ext.try_restrict(&embedded),
                Some(a),
                "try_restrict(embed(a)) != Some(a)"
            );
            Ok(())
        })
        .expect("restriction round trip");
}

fn check_membership_matches_restriction<X: FieldExtension>(
    runner: &mut TestRunner,
    ext: &X,
    ext_strategy: &BoxedStrategy<X::Ext>,
) {
    runner
        .run(ext_strategy, |x| {
            let restricted = ext.try_restrict(&x);
            prop_assert_eq!(
                ext.contains(&x),
                restricted.is_some(),
                "contains(x) disagrees with try_restrict(x).is_some()"
            );
            match restricted {
                Some(b) => {
                    prop_assert_eq!(ext.embed(&b), x.clone(), "embed(restrict(x)) != x");
                    prop_assert_eq!(
                        ext.restrict(&x),
                        Ok(b),
                        "restrict disagrees with try_restrict"
                    );
                }
                None => prop_assert_eq!(
                    ext.restrict(&x),
                    Err(FieldError::NotInBase),
                    "restrict of a non-member must report NotInBase"
                ),
            }
            Ok(())
        })
        .expect("membership matches restriction");
}

fn check_relative_frobenius<X: FieldExtension>(
    runner: &mut TestRunner,
    ext: &X,
    ext_strategy: &BoxedStrategy<X::Ext>,
) {
    let r = u32::try_from(ext.relative_degree()).expect("relative degree fits in u32");
    runner
        .run(&(ext_strategy.clone(), ext_strategy.clone()), |(x, y)| {
            prop_assert_eq!(
                ext.relative_frobenius(&x, 0),
                x.clone(),
                "phi^0 is not the identity"
            );
            prop_assert_eq!(
                ext.relative_frobenius(&x, r),
                x.clone(),
                "phi^r is not the identity"
            );

            let phi_x = ext.relative_frobenius(&x, 1);
            let phi_y = ext.relative_frobenius(&y, 1);
            prop_assert_eq!(
                ext.relative_frobenius(&(x.clone() + y.clone()), 1),
                phi_x.clone() + phi_y.clone(),
                "phi(x + y) != phi(x) + phi(y)"
            );
            prop_assert_eq!(
                ext.relative_frobenius(&(x.clone() * y), 1),
                phi_x.clone() * phi_y,
                "phi(x * y) != phi(x) * phi(y)"
            );
            prop_assert_eq!(
                ext.relative_frobenius(&x, r + 1),
                phi_x.clone(),
                "phi^k is not periodic with period r"
            );
            prop_assert_eq!(
                phi_x == x,
                ext.contains(&x),
                "the fixed set of phi is not the image of embed"
            );
            Ok(())
        })
        .expect("relative Frobenius");
}

/// GF(7²) as GF(7)[u]/(u² − 3); 3 is a quadratic non-residue modulo 7.
#[cfg(test)]
struct Gf49Config;

#[cfg(test)]
impl ExtConfig for Gf49Config {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
}

/// GF(7³) as GF(7)[v]/(v³ − 3); the cubes modulo 7 are {1, 6}, so 3 is not a
/// cube and `v³ − 3` has no root in GF(7). A cubic without roots is
/// irreducible.
#[cfg(test)]
struct Gf343Config;

#[cfg(test)]
impl ExtConfig for Gf343Config {
    type BaseField = Fp<7>;
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
}

/// GF(3²) as GF(3)[u]/(u² + 1); the squares modulo 3 are {0, 1}, so −1 = 2 is
/// a non-residue.
///
/// Visible to the crate because the compile-time quotient declarations in
/// `gfpn::quotient` extend this same carrier, so both quotient forms of
/// GF(3⁴) sit over one base type.
#[cfg(test)]
pub(crate) struct Gf9Config;

#[cfg(test)]
impl ExtConfig for Gf9Config {
    type BaseField = Fp<3>;
    const NON_RESIDUE: Fp<3> = Fp::<3>::new(2);
}

#[cfg(test)]
pub(crate) type Gf9 = QuadraticExt<Gf9Config>;

/// GF(3⁴) as GF(3²)[y]/(y² − (1 + u)).
///
/// `1 + u` is a non-square in GF(9): the squares of GF(9)* are its four
/// fourth-roots of unity, and `(1 + u)² = 2u`, so `(1 + u)⁴ = 4u² = −4 = 2 ≠ 1`.
#[cfg(test)]
struct Gf81Config;

#[cfg(test)]
impl ExtConfig for Gf81Config {
    type BaseField = Gf9;
    const NON_RESIDUE: Gf9 = QuadraticExt::new(Fp::<3>::new(1), Fp::<3>::new(1));
}

#[cfg(test)]
fn quotient_gf16() -> QuotientField<Fp<2>> {
    QuotientField::new(
        Fp::<2>::new(0),
        crate::field::FieldPoly::new(vec![
            Fp::new(1),
            Fp::new(1),
            Fp::new(0),
            Fp::new(0),
            Fp::new(1),
        ]),
    )
    .expect("x^4 + x + 1 is irreducible over GF(2)")
}

#[cfg(test)]
fn quotient_gf125() -> QuotientField<Fp<5>> {
    QuotientField::new(
        Fp::<5>::new(0),
        crate::field::FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]),
    )
    .expect("x^3 + x + 1 is irreducible over GF(5)")
}

#[cfg(test)]
fn quotient_gf81() -> QuotientField<Gf9> {
    let base = Gf9::zero();
    let beta = Gf9::new(Fp::<3>::new(1), Fp::<3>::new(1));
    QuotientField::new(
        base,
        crate::field::FieldPoly::new(vec![-beta, base, Gf9::one()]),
    )
    .expect("y^2 - (1 + u) is irreducible over GF(9)")
}

#[test]
fn test_quotient_gf16_field_axioms() {
    let field = quotient_gf16();
    test_field_axioms(quotient_strategy(&field, fp_strategy::<2>()), 2);
}

#[test]
fn test_quotient_gf125_field_axioms() {
    let field = quotient_gf125();
    test_field_axioms(quotient_strategy(&field, fp_strategy::<5>()), 5);
}

#[test]
fn test_quotient_gf81_over_gf9_field_axioms() {
    let field = quotient_gf81();
    test_field_axioms(
        quotient_strategy(&field, quadratic_strategy::<Gf9Config>(fp_strategy::<3>())),
        3,
    );
}

// Each test below builds its witness through the validating
// `ConstQuotient::extension` path, so `prove_irreducible` decides every
// in-tree compile-time declaration.

#[test]
fn test_const_quotient_gf16_const_field_axioms() {
    ConstGf16::extension().expect("x^4 + x + 1 is irreducible over GF(2)");
    test_const_field_axioms(
        const_quotient_strategy::<4, Gf16QuotientConfig>(fp_strategy::<2>()),
        2,
    );
}

#[test]
fn test_const_quotient_gf125_const_field_axioms() {
    ConstGf125::extension().expect("x^3 + x + 1 is irreducible over GF(5)");
    test_const_field_axioms(
        const_quotient_strategy::<3, Gf125QuotientConfig>(fp_strategy::<5>()),
        5,
    );
}

#[test]
fn test_const_quotient_gf81_over_gf9_const_field_axioms() {
    ConstGf81::extension().expect("y^2 - (1 + u) is irreducible over GF(9)");
    test_const_field_axioms(
        const_quotient_strategy::<2, Gf81QuotientConfig>(quadratic_strategy::<Gf9Config>(
            fp_strategy::<3>(),
        )),
        3,
    );
}

#[test]
fn test_identity_laws_fp2() {
    test_field_identity_laws(fp_strategy::<2>());
}

#[test]
fn test_identity_laws_fp7() {
    test_field_identity_laws(fp_strategy::<7>());
}

#[test]
fn test_identity_laws_fp_mersenne61() {
    test_field_identity_laws(fp_strategy::<2305843009213693951>());
}

/// `m = 1` is the smallest constructible GF(2^m); `Gf2mField_::new` rejects
/// `m = 0`, so the degenerate word-boundary case for this carrier is one bit,
/// not zero.
#[test]
fn test_identity_laws_gf2m_degree_one() {
    let field = Gf2mField::new(1, 0b11);
    test_field_identity_laws(gf2m_strategy(&field));
}

#[test]
fn test_identity_laws_gf2m_16() {
    let field = Gf2mField::new(4, 0b10011);
    test_field_identity_laws(gf2m_strategy(&field));
}

#[test]
fn test_identity_laws_gf2m_256() {
    let field = Gf2mField::gf256();
    test_field_identity_laws(gf2m_strategy(&field));
}

/// Coordinate index 63 is the last bit of the first `u64` lane; `m = 64`
/// makes it the top coordinate of the element.
#[test]
fn test_identity_laws_gf2m_u128_degree_64() {
    let field = gf2m_u128_field_from_standard(64);
    test_field_identity_laws(gf2m_u128_strategy(&field));
}

/// `m = 65` puts coordinates 63, 64, and 65 in play at once, crossing the
/// `u64` lane boundary inside a `u128` carrier.
#[test]
fn test_identity_laws_gf2m_u128_degree_65() {
    let field = gf2m_u128_field_from_standard(65);
    test_field_identity_laws(gf2m_u128_strategy(&field));
}

#[test]
fn test_identity_laws_gf2m_u128_degree_127() {
    let field = gf2m_u128_field_from_standard(127);
    test_field_identity_laws(gf2m_u128_strategy(&field));
}

/// `Gf2mWide<4, _>` is the multi-word carrier: 256 coordinates crossing every
/// internal `u64` lane boundary, and an order beyond `u128`.
#[test]
fn test_identity_laws_gf2m_wide_256() {
    test_field_identity_laws_with_cases(gf2m_wide_strategy::<4, Gf2m256TestConfig>(), 32);
}

#[test]
fn test_identity_laws_quadratic_ext_gf49() {
    test_field_identity_laws(quadratic_strategy::<Gf49Config>(fp_strategy::<7>()));
}

#[test]
fn test_identity_laws_cubic_ext_gf343() {
    test_field_identity_laws(cubic_strategy::<Gf343Config>(fp_strategy::<7>()));
}

/// The nested tower: coordinates of GF(3⁴) flatten through GF(3²), so the
/// base coordinate varies fastest and `field_id()` nests two quotient nodes.
#[test]
fn test_identity_laws_nested_tower_gf81() {
    test_field_identity_laws(quadratic_strategy::<Gf81Config>(quadratic_strategy::<
        Gf9Config,
    >(fp_strategy::<3>())));
}

#[test]
fn test_identity_laws_quotient_gf16() {
    let field = quotient_gf16();
    test_field_identity_laws(quotient_strategy(&field, fp_strategy::<2>()));
}

#[test]
fn test_identity_laws_quotient_gf125() {
    let field = quotient_gf125();
    test_field_identity_laws(quotient_strategy(&field, fp_strategy::<5>()));
}

#[test]
fn test_identity_laws_quotient_gf81_over_gf9() {
    let field = quotient_gf81();
    test_field_identity_laws(quotient_strategy(
        &field,
        quadratic_strategy::<Gf9Config>(fp_strategy::<3>()),
    ));
}

/// The compile-time carrier answers `field_id_hint`, so the hint-agreement law
/// is live here in a way the runtime quotient cannot exercise.
#[test]
fn test_identity_laws_const_quotient_gf16() {
    test_field_identity_laws(const_quotient_strategy::<4, Gf16QuotientConfig>(
        fp_strategy::<2>(),
    ));
}

#[test]
fn test_identity_laws_const_quotient_gf125() {
    test_field_identity_laws(const_quotient_strategy::<3, Gf125QuotientConfig>(
        fp_strategy::<5>(),
    ));
}

#[test]
fn test_identity_laws_const_quotient_gf81_over_gf9() {
    test_field_identity_laws(const_quotient_strategy::<2, Gf81QuotientConfig>(
        quadratic_strategy::<Gf9Config>(fp_strategy::<3>()),
    ));
}

/// GF(2) ⊂ GF(2⁴) over `x⁴ + x + 1`.
#[test]
fn test_extension_laws_gf2_in_gf16() {
    let field = Gf2mField::new(4, 0b10011);
    let ext = BinaryPrimeExt::new(field.clone()).expect("GF(2) sits inside GF(2^4)");
    test_extension_laws(&ext, fp_strategy::<2>(), gf2m_strategy(&field));
}

#[test]
fn test_extension_laws_gf2_in_gf256() {
    let field = Gf2mField::gf256();
    let ext = BinaryPrimeExt::new(field.clone()).expect("GF(2) sits inside GF(2^8)");
    test_extension_laws(&ext, fp_strategy::<2>(), gf2m_strategy(&field));
}

/// GF(2) ⊂ GF(2¹): the degree-one quotient collapses onto the prime field, so
/// the pair is the trivial extension under a runtime carrier.
#[test]
fn test_extension_laws_gf2_in_gf2m_degree_one() {
    let field = Gf2mField::new(1, 0b11);
    let ext = BinaryPrimeExt::new(field.clone()).expect("GF(2^1) is GF(2)");
    assert_eq!(ext.relative_degree(), 1);
    assert_eq!(ext.base_id(), ext.ext_id());
    test_extension_laws(&ext, fp_strategy::<2>(), gf2m_strategy(&field));
}

/// GF(2) ⊂ GF(2⁶⁵), a runtime carrier whose elements cross the `u64` lane
/// boundary.
#[test]
fn test_extension_laws_gf2_in_gf2m_degree_65() {
    let field = gf2m_u128_field_from_standard(65);
    let ext = BinaryPrimeExt::new(field.clone()).expect("GF(2) sits inside GF(2^65)");
    test_extension_laws_with_cases(&ext, fp_strategy::<2>(), gf2m_u128_strategy(&field), 32);
}

#[test]
fn test_extension_laws_fp7_in_gf49() {
    let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
    test_extension_laws(
        &ext,
        fp_strategy::<7>(),
        quadratic_strategy::<Gf49Config>(fp_strategy::<7>()),
    );
}

#[test]
fn test_extension_laws_fp7_in_gf343() {
    let ext = ConstExt::<CubicExt<Gf343Config>>::new();
    test_extension_laws(
        &ext,
        fp_strategy::<7>(),
        cubic_strategy::<Gf343Config>(fp_strategy::<7>()),
    );
}

/// GF(3²) ⊂ GF(3⁴): `base_degree() > 1`, so the relative Frobenius takes two
/// absolute steps per relative step.
#[test]
fn test_extension_laws_gf9_in_gf81() {
    let ext = ConstExt::<QuadraticExt<Gf81Config>>::new();
    assert_eq!(ext.base_degree(), 2);
    assert_eq!(ext.ext_degree(), 4);
    assert_eq!(ext.relative_degree(), 2);
    test_extension_laws(
        &ext,
        quadratic_strategy::<Gf9Config>(fp_strategy::<3>()),
        quadratic_strategy::<Gf81Config>(quadratic_strategy::<Gf9Config>(fp_strategy::<3>())),
    );
}

#[test]
fn test_extension_laws_fp2_in_quotient_gf16() {
    let field = quotient_gf16();
    test_extension_laws(
        &field,
        fp_strategy::<2>(),
        quotient_strategy(&field, fp_strategy::<2>()),
    );
}

#[test]
fn test_extension_laws_fp5_in_quotient_gf125() {
    let field = quotient_gf125();
    test_extension_laws(
        &field,
        fp_strategy::<5>(),
        quotient_strategy(&field, fp_strategy::<5>()),
    );
}

#[test]
fn test_extension_laws_gf9_in_quotient_gf81() {
    let field = quotient_gf81();
    test_extension_laws(
        &field,
        quadratic_strategy::<Gf9Config>(fp_strategy::<3>()),
        quotient_strategy(&field, quadratic_strategy::<Gf9Config>(fp_strategy::<3>())),
    );
}

/// GF(2) ⊂ GF(2⁴) through the compile-time quotient, over the same
/// non-binomial modulus its runtime twin uses.
#[test]
fn test_extension_laws_fp2_in_const_quotient_gf16() {
    let ext = ConstGf16::extension().expect("x^4 + x + 1 is irreducible over GF(2)");
    test_extension_laws(
        &ext,
        fp_strategy::<2>(),
        const_quotient_strategy::<4, Gf16QuotientConfig>(fp_strategy::<2>()),
    );
}

#[test]
fn test_extension_laws_fp5_in_const_quotient_gf125() {
    let ext = ConstGf125::extension().expect("x^3 + x + 1 is irreducible over GF(5)");
    test_extension_laws(
        &ext,
        fp_strategy::<5>(),
        const_quotient_strategy::<3, Gf125QuotientConfig>(fp_strategy::<5>()),
    );
}

/// GF(3²) ⊂ GF(3⁴) through the compile-time quotient: `base_degree() > 1`, so
/// each relative Frobenius step takes two absolute ones.
#[test]
fn test_extension_laws_gf9_in_const_quotient_gf81() {
    let ext = ConstGf81::extension().expect("y^2 - (1 + u) is irreducible over GF(9)");
    assert_eq!(ext.base_degree(), 2);
    assert_eq!(ext.ext_degree(), 4);
    test_extension_laws(
        &ext,
        quadratic_strategy::<Gf9Config>(fp_strategy::<3>()),
        const_quotient_strategy::<2, Gf81QuotientConfig>(quadratic_strategy::<Gf9Config>(
            fp_strategy::<3>(),
        )),
    );
}

#[test]
fn test_extension_laws_trivial_fp7() {
    let ext = TrivialExt::new(Fp::<7>::new(0));
    test_extension_laws(&ext, fp_strategy::<7>(), fp_strategy::<7>());
}

#[test]
fn test_extension_laws_trivial_gf256() {
    let field = Gf2mField::gf256();
    let ext = TrivialExt::new(field.zero());
    test_extension_laws(&ext, gf2m_strategy(&field), gf2m_strategy(&field));
}

/// The trivial extension over the multi-word carrier, whose order exceeds
/// `u128` and therefore exercises the `None` arm of the order laws.
#[test]
fn test_extension_laws_trivial_gf2m_wide_256() {
    let ext = TrivialExt::new(<Gf2mWide<4, Gf2m256TestConfig> as ConstField>::zero());
    assert_eq!(ext.ext_order(), None);
    test_extension_laws_with_cases(
        &ext,
        gf2m_wide_strategy::<4, Gf2m256TestConfig>(),
        gf2m_wide_strategy::<4, Gf2m256TestConfig>(),
        32,
    );
}
