//! Complete irreducibility validation for finite-field polynomials.
//!
//! The implementation uses Rabin's criterion over the field named by the
//! coefficient witness.  Its certificate records the canonical base-field
//! identity and modulus, so a quotient-field constructor can reuse the
//! validation without running the decision procedure again.

use std::sync::Arc;

use crate::field::extension::{
    Basis, CertificateBasis, ExtensionCertificate, FactorWitness, FieldError, FieldId,
    FieldIdentity, ModulusId,
};
use crate::field::{FieldPoly, FiniteField};

/// Evidence that a polynomial is irreducible over a named finite field.
///
/// The certificate owns the canonical base-field identity and modulus
/// description used by the decision procedure.  It is therefore cheap to
/// clone and can be promoted to an [`ExtensionCertificate`] by quotient-field
/// constructors without repeating the irreducibility test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrreducibilityCertificate(Arc<IrreducibilityCertificateRepr>);

#[derive(Debug, PartialEq, Eq)]
struct IrreducibilityCertificateRepr {
    base_id: FieldId,
    modulus: ModulusId,
    method: IrreducibilityMethod,
}

impl IrreducibilityCertificate {
    /// Returns the identity of the coefficient field.
    pub fn base_id(&self) -> &FieldId {
        &self.0.base_id
    }

    /// Returns the canonical modulus recorded by this certificate.
    pub fn modulus(&self) -> &ModulusId {
        &self.0.modulus
    }

    /// Returns the complete decision procedure that produced this evidence.
    pub fn method(&self) -> IrreducibilityMethod {
        self.0.method
    }

    /// Promotes this evidence to a certificate for the polynomial quotient
    /// extension named by the base and modulus.
    ///
    /// # Panics
    ///
    /// Panics only if the internally stored certificate is inconsistent with
    /// [`ModulusId`] or [`FieldId`] invariants.  Such a value cannot be
    /// produced by [`prove_irreducible`].
    pub fn extension_certificate(&self) -> ExtensionCertificate {
        let extension = FieldId::quotient(
            self.base_id().clone(),
            self.modulus().clone(),
            Basis::Polynomial,
        )
        .expect("an irreducibility certificate stores a modulus over its base");
        ExtensionCertificate::from_parts(
            self.base_id().clone(),
            extension,
            CertificateBasis::Proved,
        )
        .expect("an irreducibility certificate stores a tower quotient")
    }
}

/// Complete algorithms that can provide irreducibility evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrreducibilityMethod {
    /// Rabin's finite-field irreducibility criterion.
    Rabin,
    /// Distinct-degree factor detection.
    DistinctDegree,
    /// A verified repository polynomial registry entry.
    Registry,
}

/// Decides whether `f` is irreducible over the field represented by `witness`.
///
/// The decision uses Rabin's criterion.  For a monic degree-`n` polynomial
/// over $𝔽_q$, it checks
/// `x^(q^n) = x (mod f)` and
/// `gcd(f, x^(q^(n/r)) - x) = 1` for every distinct prime `r` dividing `n`.
/// The required powers are computed by iterating the absolute Frobenius
/// `d = [B : GF(p)]` times per `q`-Frobenius step, so neither `q` nor any
/// `q^k` exponent has to fit in a machine integer.
///
/// The coefficient identities are checked before arithmetic, and the
/// resulting modulus is stored in canonical prime coordinates.  A failed
/// gcd is reported as either a base-field root or a proper factor; when the
/// final Rabin equality fails, a distinct-degree scan supplies the witness
/// factor class.
///
/// # Examples
///
/// ```
/// use gf2_core::field::{prove_irreducible, ConstField, FieldPoly, IrreducibilityMethod};
/// use gf2_core::gfp::Fp;
///
/// let base = Fp::<5>::zero();
/// let modulus = FieldPoly::new(vec![
///     Fp::<5>::new(1),
///     Fp::<5>::new(1),
///     Fp::<5>::new(0),
///     Fp::<5>::new(1),
/// ]);
/// let certificate = prove_irreducible(&modulus, &base)?;
/// assert_eq!(certificate.method(), IrreducibilityMethod::Rabin);
/// assert_eq!(certificate.modulus().degree(), 3);
/// # Ok::<(), gf2_core::field::FieldError>(())
/// ```
///
/// # Errors
///
/// - [`FieldError::ModulusDegreeTooSmall`] for a constant or zero polynomial.
/// - [`FieldError::NonMonicModulus`] when the leading coefficient is not one.
/// - [`FieldError::IdentityMismatch`] when a coefficient belongs to a
///   different runtime field than `witness`.
/// - [`FieldError::ReducibleModulus`] with a [`FactorWitness`] when a proper
///   factor class is found.
/// - [`FieldError::UnsupportedSize`] if canonical modulus-coordinate storage
///   cannot be sized for the input degree.
///
/// # Complexity
///
/// Let `n` be the polynomial degree, `d` the base-field degree over its prime
/// field, and `p` its characteristic.  Rabin's checks use
/// `O(d log p · n)` modular polynomial multiplications plus the gcds for the
/// distinct prime divisors of `n`; with the schoolbook polynomial substrate
/// this is `O(d log p · n^3)` field operations in the conservative worst case.
/// A distinct-degree scan is run only to obtain a witness after the final
/// Rabin equality fails and has the same polynomial-operation bound.
pub fn prove_irreducible<F: FieldIdentity>(
    f: &FieldPoly<F>,
    witness: &F,
) -> Result<IrreducibilityCertificate, FieldError> {
    let degree = f
        .degree()
        .ok_or(FieldError::ModulusDegreeTooSmall { degree: 0 })?;
    let base_id = witness.field_id();

    if degree == 0 {
        return Err(FieldError::ModulusDegreeTooSmall { degree: 0 });
    }

    for coefficient in f.iter() {
        let found = coefficient.field_id();
        if found != base_id {
            return Err(FieldError::IdentityMismatch {
                expected: base_id.clone(),
                found,
            });
        }
    }

    if !f
        .leading_coeff()
        .expect("a non-zero polynomial has a leading coefficient")
        .is_one()
    {
        return Err(FieldError::NonMonicModulus);
    }

    let coefficient_count = degree
        .checked_add(1)
        .ok_or_else(|| unsupported_size(degree, base_id.characteristic()))?;
    let coordinate_count = coefficient_count
        .checked_mul(base_id.degree())
        .ok_or_else(|| unsupported_size(degree, base_id.characteristic()))?;
    let modulus = modulus_id(f, &base_id, coordinate_count);

    // Every monic linear polynomial is irreducible.  Handling it directly
    // also avoids reducing the degree-one indeterminate modulo itself.
    if degree == 1 {
        return Ok(certificate(base_id, modulus, IrreducibilityMethod::Rabin));
    }

    let characteristic = base_id.characteristic();
    let base_degree = base_id.degree();
    let x = FieldPoly::monomial(witness.one_like(), 1);

    // Check the degree-one class first.  This gives the most useful witness
    // even when a later Rabin gcd would also contain higher-degree factors.
    let mut frobenius = q_frobenius(&x, f, witness, base_degree, characteristic);
    let root_gcd = FieldPoly::gcd(f, &(&frobenius - &x));
    if !is_one(&root_gcd) {
        return Err(FieldError::ReducibleModulus {
            witness: FactorWitness::BaseFieldRoot,
        });
    }

    let mut current_step = 1usize;
    let mut targets: Vec<usize> = prime_divisors(degree)
        .into_iter()
        .map(|prime| degree / prime)
        .filter(|&target| target > 1)
        .collect();
    targets.sort_unstable();

    for target in targets {
        advance_q_frobenius(
            &mut frobenius,
            target - current_step,
            f,
            witness,
            base_degree,
            characteristic,
        );
        current_step = target;

        let gcd = FieldPoly::gcd(f, &(&frobenius - &x));
        if !is_one(&gcd) {
            let gcd_degree = gcd
                .degree()
                .expect("the gcd of a non-zero modulus is non-zero");
            let factor_witness = if gcd_degree == degree {
                FactorWitness::DistinctDegreeSplit { degree: target }
            } else {
                FactorWitness::ProperFactor { degree: gcd_degree }
            };
            return Err(FieldError::ReducibleModulus {
                witness: factor_witness,
            });
        }
    }

    advance_q_frobenius(
        &mut frobenius,
        degree - current_step,
        f,
        witness,
        base_degree,
        characteristic,
    );
    if frobenius != x {
        // A reducible degree-n polynomial has an irreducible factor of
        // degree at most n / 2, so this scan must find a witness over a
        // genuine finite field.  Keep a typed defensive result for a
        // malformed FiniteField implementation rather than panicking.
        let factor_witness =
            distinct_degree_witness(f, &x, witness, base_degree, characteristic, degree / 2)
                .unwrap_or(FactorWitness::ProperFactor { degree: degree - 1 });
        return Err(FieldError::ReducibleModulus {
            witness: factor_witness,
        });
    }

    Ok(certificate(base_id, modulus, IrreducibilityMethod::Rabin))
}

fn certificate(
    base_id: FieldId,
    modulus: ModulusId,
    method: IrreducibilityMethod,
) -> IrreducibilityCertificate {
    IrreducibilityCertificate(Arc::new(IrreducibilityCertificateRepr {
        base_id,
        modulus,
        method,
    }))
}

fn modulus_id<F: FieldIdentity>(
    polynomial: &FieldPoly<F>,
    base_id: &FieldId,
    coordinate_count: usize,
) -> ModulusId {
    let mut coordinates = Vec::with_capacity(coordinate_count);
    let mut coefficient_coordinates = Vec::new();
    for coefficient in polynomial.iter() {
        coefficient.write_prime_coords(&mut coefficient_coordinates);
        coordinates.extend_from_slice(&coefficient_coordinates);
    }
    ModulusId::new(base_id, coordinates)
        .expect("validated polynomial coefficients produce a valid modulus identity")
}

fn unsupported_size(degree: usize, characteristic: u64) -> FieldError {
    FieldError::UnsupportedSize {
        degree,
        characteristic,
    }
}

fn is_one<F: FiniteField>(polynomial: &FieldPoly<F>) -> bool {
    polynomial.degree() == Some(0) && polynomial.leading_coeff().is_some_and(FiniteField::is_one)
}

fn prime_divisors(mut value: usize) -> Vec<usize> {
    let mut divisors = Vec::new();
    let mut factor = 2usize;
    while factor <= value / factor {
        if value.is_multiple_of(factor) {
            divisors.push(factor);
            while value.is_multiple_of(factor) {
                value /= factor;
            }
        }
        factor += 1;
    }
    if value > 1 {
        divisors.push(value);
    }
    divisors
}

fn advance_q_frobenius<F: FieldIdentity>(
    value: &mut FieldPoly<F>,
    steps: usize,
    modulus: &FieldPoly<F>,
    witness: &F,
    base_degree: usize,
    characteristic: u64,
) {
    for _ in 0..steps {
        *value = q_frobenius(value, modulus, witness, base_degree, characteristic);
    }
}

fn q_frobenius<F: FieldIdentity>(
    value: &FieldPoly<F>,
    modulus: &FieldPoly<F>,
    witness: &F,
    base_degree: usize,
    characteristic: u64,
) -> FieldPoly<F> {
    let mut result = value.clone();
    for _ in 0..base_degree {
        result = pow_mod(&result, characteristic, modulus, witness);
    }
    result
}

fn pow_mod<F: FieldIdentity>(
    value: &FieldPoly<F>,
    exponent: u64,
    modulus: &FieldPoly<F>,
    witness: &F,
) -> FieldPoly<F> {
    let mut result = FieldPoly::one_like(witness);
    let (_, mut power) = value.div_rem(modulus);
    let mut exponent = exponent;

    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul_mod(&result, &power, modulus);
        }
        exponent >>= 1;
        if exponent != 0 {
            power = mul_mod(&power, &power, modulus);
        }
    }
    result
}

fn mul_mod<F: FieldIdentity>(
    left: &FieldPoly<F>,
    right: &FieldPoly<F>,
    modulus: &FieldPoly<F>,
) -> FieldPoly<F> {
    let product = left * right;
    let (_, remainder) = product.div_rem(modulus);
    remainder
}

fn distinct_degree_witness<F: FieldIdentity>(
    modulus: &FieldPoly<F>,
    x: &FieldPoly<F>,
    witness: &F,
    base_degree: usize,
    characteristic: u64,
    max_degree: usize,
) -> Option<FactorWitness> {
    let mut frobenius = x.clone();
    for factor_degree in 1..=max_degree {
        frobenius = q_frobenius(&frobenius, modulus, witness, base_degree, characteristic);
        let gcd = FieldPoly::gcd(modulus, &(&frobenius - x));
        if !is_one(&gcd) {
            let gcd_degree = gcd
                .degree()
                .expect("the gcd of a non-zero modulus is non-zero");
            return Some(if factor_degree == 1 {
                FactorWitness::BaseFieldRoot
            } else if gcd_degree == modulus.degree().unwrap() {
                FactorWitness::DistinctDegreeSplit {
                    degree: factor_degree,
                }
            } else {
                FactorWitness::ProperFactor { degree: gcd_degree }
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::extension::{FactorWitness, FieldError, FieldIdentity};
    use crate::field::{ConstField, FieldPoly, FiniteField};
    use crate::gfp::Fp;
    use crate::gfpn::{ExtConfig, QuadraticExt};

    fn fp2_modulus() -> FieldPoly<Fp<2>> {
        FieldPoly::new(vec![
            Fp::new(1),
            Fp::new(1),
            Fp::new(0),
            Fp::new(0),
            Fp::new(1),
        ])
    }

    #[test]
    fn accepts_gf2_irreducible_polynomial_and_builds_reusable_certificate() {
        let witness = Fp::<2>::zero();
        let certificate = prove_irreducible(&fp2_modulus(), &witness).unwrap();

        assert_eq!(certificate.method(), IrreducibilityMethod::Rabin);
        assert_eq!(certificate.base_id(), &witness.field_id());
        assert_eq!(certificate.modulus().coefficients(), &[1, 1, 0, 0, 1]);

        let extension = certificate.extension_certificate();
        assert_eq!(extension.relative_degree(), 4);
        assert_eq!(
            extension.basis(),
            crate::field::extension::CertificateBasis::Proved
        );
        let extension_id = FieldId::quotient(
            certificate.base_id().clone(),
            certificate.modulus().clone(),
            crate::field::extension::Basis::Polynomial,
        )
        .unwrap();
        assert!(extension.matches(certificate.base_id(), &extension_id,));
    }

    #[test]
    fn rejects_gf2_polynomial_with_a_base_field_root() {
        let witness = Fp::<2>::zero();
        let polynomial = FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]);

        assert_eq!(
            prove_irreducible(&polynomial, &witness),
            Err(FieldError::ReducibleModulus {
                witness: FactorWitness::BaseFieldRoot,
            })
        );
    }

    #[test]
    fn accepts_gf5_irreducible_cubic() {
        let witness = Fp::<5>::zero();
        let polynomial = FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]);

        assert_eq!(
            prove_irreducible(&polynomial, &witness).unwrap().method(),
            IrreducibilityMethod::Rabin
        );
    }

    #[test]
    fn identifies_a_gf5_distinct_degree_factor_class_without_a_root() {
        let witness = Fp::<5>::zero();
        // (x² + 2)(x² + 3) = x⁴ + 1.  Both factors are irreducible over GF(5).
        let polynomial = FieldPoly::new(vec![
            Fp::new(1),
            Fp::new(0),
            Fp::new(0),
            Fp::new(0),
            Fp::new(1),
        ]);

        assert_eq!(
            prove_irreducible(&polynomial, &witness),
            Err(FieldError::ReducibleModulus {
                witness: FactorWitness::DistinctDegreeSplit { degree: 2 },
            })
        );
    }

    #[test]
    fn identifies_a_gf5_proper_factor_after_the_final_rabin_check() {
        let witness = Fp::<5>::zero();
        // (x² + 2)(x³ + x + 1) has no base-field root, but is reducible.
        let polynomial = FieldPoly::new(vec![
            Fp::new(2),
            Fp::new(2),
            Fp::new(1),
            Fp::new(3),
            Fp::new(0),
            Fp::new(1),
        ]);

        assert_eq!(
            prove_irreducible(&polynomial, &witness),
            Err(FieldError::ReducibleModulus {
                witness: FactorWitness::ProperFactor { degree: 2 },
            })
        );
    }

    struct Gf9Config;

    impl ExtConfig for Gf9Config {
        type BaseField = Fp<3>;
        const NON_RESIDUE: Fp<3> = Fp::<3>::new(2);
    }

    type Gf9 = QuadraticExt<Gf9Config>;

    struct Gf7Beta6Config;

    impl ExtConfig for Gf7Beta6Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(6);
    }

    struct Gf7Beta3Config;

    impl ExtConfig for Gf7Beta3Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    struct Gf13Beta2Config;

    impl ExtConfig for Gf13Beta2Config {
        type BaseField = Fp<13>;
        const NON_RESIDUE: Fp<13> = Fp::<13>::new(2);
    }

    struct Gf7CubicBeta3Config;

    impl ExtConfig for Gf7CubicBeta3Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    struct Gf7CubicBeta2Config;

    impl ExtConfig for Gf7CubicBeta2Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(2);
    }

    struct Gf9BetaOnePlusUConfig;

    impl ExtConfig for Gf9BetaOnePlusUConfig {
        type BaseField = Gf9;
        const NON_RESIDUE: Gf9 = QuadraticExt::new(Fp::<3>::new(1), Fp::<3>::new(1));
    }

    type Gf81 = QuadraticExt<Gf9BetaOnePlusUConfig>;

    fn assert_binomial_irreducible<C: ExtConfig>(degree: usize)
    where
        C::BaseField: ConstField + FieldIdentity,
    {
        let witness = <C::BaseField as ConstField>::zero();
        let beta = C::mul_by_non_residue(<C::BaseField as ConstField>::one());
        let zero = witness.zero_like();
        let one = witness.one_like();
        let coefficients = match degree {
            2 => vec![-beta, zero, one],
            3 => vec![-beta, zero, zero, one],
            _ => panic!("test helper only covers quadratic and cubic moduli"),
        };
        let modulus = FieldPoly::new(coefficients);
        let certificate = prove_irreducible(&modulus, &witness).unwrap();
        assert_eq!(certificate.method(), IrreducibilityMethod::Rabin);
    }

    #[test]
    fn ext_config_moduli_are_irreducible() {
        // These are the modulus values declared by the in-tree field-bearing
        // ExtConfig implementations: ext_config.rs, the quadratic/cubic
        // field tests, extension.rs tower tests, and the Wiedemann configs.
        // Repeating a value is intentional where independent configs declare
        // the same modulus; it verifies each declaration's carrier shape.
        assert_binomial_irreducible::<Gf7Beta6Config>(2);
        assert_binomial_irreducible::<Gf7Beta3Config>(2);
        assert_binomial_irreducible::<Gf13Beta2Config>(2);
        assert_binomial_irreducible::<Gf7CubicBeta3Config>(3);
        assert_binomial_irreducible::<Gf7CubicBeta2Config>(3);
        assert_binomial_irreducible::<Gf9Config>(2);
        assert_binomial_irreducible::<Gf9BetaOnePlusUConfig>(2);

        struct Fp251QuadraticConfig;
        impl ExtConfig for Fp251QuadraticConfig {
            type BaseField = Fp<251>;
            const NON_RESIDUE: Fp<251> = Fp::<251>::new(250);
        }
        assert_binomial_irreducible::<Fp251QuadraticConfig>(2);

        struct Fp65537QuadraticConfig;
        impl ExtConfig for Fp65537QuadraticConfig {
            type BaseField = Fp<65537>;
            const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
        }
        assert_binomial_irreducible::<Fp65537QuadraticConfig>(2);

        // Exercise the relative GF(3²) base used by the GF(3⁴) tower.
        let tower_witness = Gf9::zero();
        let tower_beta = Gf9::new(Fp::<3>::new(1), Fp::<3>::new(1));
        let tower_modulus = FieldPoly::new(vec![
            -tower_beta,
            tower_witness.zero_like(),
            tower_witness.one_like(),
        ]);
        assert_eq!(
            prove_irreducible(&tower_modulus, &tower_witness)
                .unwrap()
                .method(),
            IrreducibilityMethod::Rabin
        );
        let tower_extension = Gf81::zero();
        assert_eq!(
            tower_extension.field_id().base(),
            Some(&tower_witness.field_id())
        );
    }

    #[test]
    fn accepts_and_rejects_polynomials_over_a_gf9_base() {
        let witness = Gf9::zero();
        let beta = Gf9::new(Fp::<3>::new(1), Fp::<3>::new(1));
        let irreducible = FieldPoly::new(vec![-beta, witness.zero_like(), witness.one_like()]);
        let reducible = FieldPoly::new(vec![
            -witness.one_like(),
            witness.zero_like(),
            witness.one_like(),
        ]);

        assert_eq!(
            prove_irreducible(&irreducible, &witness).unwrap().method(),
            IrreducibilityMethod::Rabin
        );
        assert!(matches!(
            prove_irreducible(&reducible, &witness),
            Err(FieldError::ReducibleModulus {
                witness: FactorWitness::BaseFieldRoot,
            })
        ));
    }

    #[test]
    fn accepts_a_polynomial_over_a_runtime_gf4_base() {
        let field = crate::gf2m::Gf2mField::new(2, 0b111);
        let witness = field.zero();
        // x² + x + α has no root in GF(4), where α is the class of x.
        let polynomial = FieldPoly::new(vec![
            field.element(2),
            witness.one_like(),
            witness.one_like(),
        ]);

        assert_eq!(
            prove_irreducible(&polynomial, &witness).unwrap().base_id(),
            &witness.field_id()
        );
    }

    #[test]
    fn rejects_non_monic_and_constant_inputs() {
        let witness = Fp::<5>::zero();
        let non_monic = FieldPoly::new(vec![Fp::new(1), Fp::new(2)]);
        let constant = FieldPoly::constant(Fp::<5>::new(1));

        assert_eq!(
            prove_irreducible(&non_monic, &witness),
            Err(FieldError::NonMonicModulus)
        );
        assert_eq!(
            prove_irreducible(&constant, &witness),
            Err(FieldError::ModulusDegreeTooSmall { degree: 0 })
        );
    }
}
