//! Polynomial quotient extension fields in runtime and compile-time form.
//!
//! [`QuotientField`] is the reference implementation of a polynomial quotient
//! $E = B\lbrack x\rbrack/(f)$. A [`QuotientElement`] stores exactly `degree(f)` base-field
//! coefficients in ascending order, so `[a0, a1, ...]` denotes
//! $a_0 + a_1 x + \cdots$. The modulus is supplied in the same constant-first
//! convention, must be monic, and is validated as irreducible by
//! [`QuotientField::new`]. Multiplication uses schoolbook convolution followed
//! by the rewrite
//! $x^r = -\sum_{i=0}^{r-1} f_i x^i$. Inversion uses polynomial extended
//! Euclid, and Frobenius iterates exponentiation by the prime characteristic.
//!
//! The presentation identity is exactly
//! [`FieldId::quotient`](crate::field::extension::FieldId::quotient) with the
//! base identity, the certificate's [`ModulusId`], and [`Basis::Polynomial`].
//! Canonical prime coordinates concatenate the coordinates of each stored
//! base coefficient, with the base coordinate varying fastest.
//!
//! # The compile-time form
//!
//! [`ConstQuotient`] carries the same field with the presentation fixed by a
//! [`ConstQuotientConfig`] implementor instead of by runtime values. Elements
//! are `[B; R]` arrays, the modulus lives in the config's associated constant
//! rather than in any element, and the arithmetic monomorphizes over it, so a
//! compile-time element stores no descriptor and consults no certificate.
//! [`ConstQuotient::extension`] decides the declared modulus and yields a
//! [`ConstQuotientExt`] witness; [`ConstQuotient::extension_unchecked`] takes
//! the declaration on trust.
//!
//! # Equivalence of the two forms
//!
//! Both forms of one presentation are the same field under three propositions,
//! which the file's differential suite checks over
//! $\mathrm{GF}(2^4)$, $\mathrm{GF}(5^3)$, and $\mathrm{GF}(3^4)$ over
//! $\mathrm{GF}(9)$:
//!
//! 1. **Identity agreement.** `<ConstQuotient<R, C> as FieldIdentity>::field_id_hint()`
//!    equals `Some(runtime_witness.field_id())`.
//! 2. **Coordinate agreement.** [`FieldIdentity::write_prime_coords`] produces
//!    equal vectors for corresponding elements.
//! 3. **Observable equivalence.**
//!    [`convert_element`](crate::field::extension::convert_element) commutes
//!    with addition, subtraction, multiplication, inversion, `pow`,
//!    [`FieldExtension::relative_frobenius`],
//!    [`minimal_polynomial`](crate::field::extension::minimal_polynomial), and
//!    [`canonical_generator`](crate::field::extension::canonical_generator).
//!
//! Because the shared [`FieldId`] pins the basis, `convert_element` between the
//! two carriers is the identity map on canonical coordinates.
//!
//! # Walkthrough
//!
//! ```
//! use gf2_core::field::extension::{FieldExtension, FieldIdentity};
//! use gf2_core::field::{ConstField, FieldPoly, FiniteField};
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::QuotientField;
//!
//! // GF(5^3) = GF(5)[x] / (x^3 + x + 1).
//! let field = QuotientField::new(
//!     Fp::<5>::zero(),
//!     FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]),
//! )?;
//! let x = field.indeterminate();
//! let a = field.element(vec![Fp::new(2), Fp::new(3)])?;
//!
//! assert_eq!(field.relative_degree(), 3);
//! assert_eq!(a.coefficients(), &[Fp::new(2), Fp::new(3), Fp::new(0)]);
//! assert!((a.clone() * a.inv().unwrap()).is_one());
//! assert_eq!(field.relative_frobenius(&x, 3), x);
//! assert_eq!(a.field_id(), field.ext_id().clone());
//! # Ok::<(), gf2_core::field::FieldError>(())
//! ```
//!
//! The same field declared at compile time, and the transport between the two
//! carriers:
//!
//! ```
//! use gf2_core::field::extension::{convert_element, FieldExtension, FieldIdentity};
//! use gf2_core::field::{ConstField, FiniteField};
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::{ConstQuotient, ConstQuotientConfig};
//!
//! // GF(5^3) = GF(5)[x] / (x^3 + x + 1); the leading one is implicit.
//! struct Gf125;
//! impl ConstQuotientConfig<3> for Gf125 {
//!     type BaseField = Fp<5>;
//!     const MODULUS: [Fp<5>; 3] = [Fp::new(1), Fp::new(1), Fp::new(0)];
//! }
//! type Field = ConstQuotient<3, Gf125>;
//!
//! // The validating path decides the declaration before handing back a witness.
//! let ext = Field::extension()?;
//! assert_eq!(ext.relative_degree(), 3);
//!
//! let a = Field::new([Fp::new(2), Fp::new(3), Fp::new(0)]);
//! assert!((a * a.inv().unwrap()).is_one());
//! assert_eq!(ext.relative_frobenius(&Field::indeterminate(), 3), Field::indeterminate());
//!
//! // One field, two carriers: equal identity, and coordinate-preserving transport.
//! let runtime = Field::runtime_field()?;
//! assert_eq!(Field::field_id_hint(), Some(runtime.ext_id().clone()));
//! let transported = convert_element(&a, &runtime.ext_zero())?;
//! assert_eq!(convert_element(&transported, &Field::zero())?, a);
//! # Ok::<(), gf2_core::field::FieldError>(())
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};
use std::sync::Arc;

use crate::field::extension::{
    Basis, ConstExt, ConstSimpleExtension, ExtensionCertificate, FieldError, FieldExtension,
    FieldId, FieldIdentity, ModulusId,
};
use crate::field::irreducibility::{prove_irreducible, IrreducibilityCertificate};
use crate::field::{ConstField, FieldPoly, FiniteField, FiniteFieldExt};

struct QuotientParams<F: FieldIdentity> {
    base_zero: F,
    modulus: FieldPoly<F>,
    degree: usize,
    absolute_degree: usize,
    field_id: FieldId,
    certificate: ExtensionCertificate,
}

/// A runtime descriptor and extension witness for $B\lbrack x\rbrack/(f)$.
///
/// The descriptor owns the canonicalized base witness, the monic modulus, and
/// the validation certificate. Clones share those parameters through an
/// `Arc`; independently constructed descriptors compare equal when both the
/// presentation and certificate agree.
///
/// # Examples
///
/// ```
/// use gf2_core::field::{ConstField, FieldPoly};
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::QuotientField;
///
/// let field = QuotientField::new(
///     Fp::<2>::zero(),
///     FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]),
/// )?;
/// assert_eq!(field.modulus().degree(), Some(3));
/// # Ok::<(), gf2_core::field::FieldError>(())
/// ```
pub struct QuotientField<F: FieldIdentity> {
    params: Arc<QuotientParams<F>>,
}

impl<F: FieldIdentity> Clone for QuotientField<F> {
    fn clone(&self) -> Self {
        Self {
            params: Arc::clone(&self.params),
        }
    }
}

impl<F: FieldIdentity> fmt::Debug for QuotientField<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotientField")
            .field("base", self.params.certificate.base_id())
            .field("extension", &self.params.field_id)
            .field("modulus", &self.params.modulus)
            .field("certificate_basis", &self.params.certificate.basis())
            .finish()
    }
}

impl<F: FieldIdentity> PartialEq for QuotientField<F> {
    fn eq(&self, other: &Self) -> bool {
        self.params.field_id == other.params.field_id
            && self.params.certificate == other.params.certificate
    }
}

impl<F: FieldIdentity> Eq for QuotientField<F> {}

/// Short name for the runtime quotient extension witness.
///
/// This alias emphasizes the [`FieldExtension`] role while
/// [`QuotientField`] emphasizes element construction; both names denote the
/// same descriptor.
///
/// # Examples
///
/// ```
/// use gf2_core::field::{ConstField, FieldPoly};
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::QuotientExt;
///
/// let extension = QuotientExt::new(
///     Fp::<3>::zero(),
///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
/// )?;
/// assert_eq!(extension.modulus().degree(), Some(2));
/// # Ok::<(), gf2_core::field::FieldError>(())
/// ```
pub type QuotientExt<F> = QuotientField<F>;

impl<F: FieldIdentity> QuotientField<F> {
    /// Validates `modulus` completely and constructs $B\lbrack x\rbrack/(f)$.
    ///
    /// `base` is an element witness for $B$; its value is ignored. The
    /// modulus coefficients are canonicalized into that witness's runtime
    /// context before [`prove_irreducible`] runs. This is the deciding path;
    /// use [`from_certificate`](Self::from_certificate) to consume an
    /// existing irreducibility result.
    ///
    /// # Errors
    ///
    /// - [`FieldError::ModulusDegreeTooSmall`] for a constant or zero modulus.
    /// - [`FieldError::NonMonicModulus`] for a leading coefficient other than one.
    /// - [`FieldError::IdentityMismatch`] when a coefficient is not in `base`.
    /// - [`FieldError::ReducibleModulus`] when the complete decision finds a factor.
    /// - [`FieldError::UnsupportedSize`] when coefficient or absolute-degree
    ///   materialization would overflow `usize`.
    /// - Coordinate errors reported by [`FieldIdentity::from_prime_coords`]
    ///   if a base carrier rejects its own canonical coordinates.
    ///
    /// # Complexity
    ///
    /// The validation has the `O(d log(p) r^3)` conservative bound of
    /// [`prove_irreducible`], for base degree `d`, characteristic `p`, and
    /// modulus degree `r`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldError, FieldPoly};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let valid = QuotientField::new(
    ///     Fp::<5>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// assert_eq!(valid.order()?, 125);
    ///
    /// let reducible = FieldPoly::new(vec![Fp::<5>::new(4), Fp::new(0), Fp::new(1)]);
    /// assert!(matches!(
    ///     QuotientField::new(Fp::<5>::zero(), reducible),
    ///     Err(FieldError::ReducibleModulus { .. })
    /// ));
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn new(base: F, modulus: FieldPoly<F>) -> Result<Self, FieldError> {
        let base_zero = base.zero_like();
        let modulus = canonicalize_modulus(&base_zero, modulus)?;
        let certificate = prove_irreducible(&modulus, &base_zero)?;
        Self::from_certificate(base_zero, certificate)
    }

    /// Consumes complete irreducibility evidence and constructs its quotient.
    ///
    /// The modulus is rebuilt from the certificate's canonical coordinates in
    /// the runtime context supplied by `base`. No irreducibility decision is
    /// repeated.
    ///
    /// # Errors
    ///
    /// - [`FieldError::IdentityMismatch`] when `base` is not the certified base.
    /// - [`FieldError::UnsupportedSize`] when the absolute extension degree
    ///   or an arithmetic work-buffer length would overflow `usize`.
    /// - Coordinate errors from [`FieldIdentity::from_prime_coords`] if the
    ///   base carrier rejects a certified coefficient coordinate vector.
    ///
    /// # Complexity
    ///
    /// Linear in the number of canonical modulus coordinates; no polynomial
    /// decision procedure runs.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{prove_irreducible, ConstField, FieldPoly};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let base = Fp::<2>::zero();
    /// let modulus = FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]);
    /// let certificate = prove_irreducible(&modulus, &base)?;
    /// let field = QuotientField::from_certificate(base, certificate)?;
    /// assert_eq!(field.modulus(), &modulus);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn from_certificate(
        base: F,
        certificate: IrreducibilityCertificate,
    ) -> Result<Self, FieldError> {
        let base_zero = base.zero_like();
        let expected = base_zero.field_id();
        if certificate.base_id() != &expected {
            return Err(FieldError::IdentityMismatch {
                expected,
                found: certificate.base_id().clone(),
            });
        }
        checked_degrees(&expected, certificate.modulus().degree())?;
        let modulus = polynomial_from_modulus(&base_zero, certificate.modulus())?;
        let extension_certificate = certificate.extension_certificate();
        build_field(base_zero, modulus, extension_certificate)
    }

    /// Constructs a quotient from a caller-supplied validation memo.
    ///
    /// This method validates the base, modulus shape, monicity, materialized
    /// sizes, and the certificate's two identities. It deliberately does not
    /// decide irreducibility or verify what the certificate's provenance
    /// variant claims. The identity comparisons are structural and linear in
    /// the modulus coordinates; they are not a decision procedure.
    ///
    /// # Contract
    ///
    /// The caller promises that `certificate` memoizes a valid irreducibility
    /// decision for exactly `base` and `modulus`. Violating that precondition
    /// gives unspecified mathematical results (GIGO), never memory unsafety.
    /// This is the caller-trusted validation-cost fast path fixed by
    /// `@/inv/caller-trusted-fast-paths`.
    ///
    /// # Errors
    ///
    /// - [`FieldError::ModulusDegreeTooSmall`],
    ///   [`FieldError::NonMonicModulus`], or
    ///   [`FieldError::IdentityMismatch`] for structurally invalid inputs.
    /// - [`FieldError::UnsupportedSize`] when materialized degree arithmetic
    ///   would overflow `usize`.
    /// - Coordinate errors from canonicalizing base-field coefficients.
    ///
    /// # Complexity
    ///
    /// Linear in the modulus coordinate count; irreducibility is not tested.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::FieldExtension;
    /// use gf2_core::field::{ConstField, FieldPoly};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let base = Fp::<2>::zero();
    /// let modulus = FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]);
    /// let decided = QuotientField::new(base, modulus.clone())?;
    /// let reused = QuotientField::from_certificate_unchecked(
    ///     base,
    ///     modulus,
    ///     decided.certificate().clone(),
    /// )?;
    /// assert_eq!(reused, decided);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn from_certificate_unchecked(
        base: F,
        modulus: FieldPoly<F>,
        certificate: ExtensionCertificate,
    ) -> Result<Self, FieldError> {
        let base_zero = base.zero_like();
        let modulus = canonicalize_modulus(&base_zero, modulus)?;
        build_field(base_zero, modulus, certificate)
    }

    /// Returns the canonical monic modulus, with its constant coefficient first.
    ///
    /// # Complexity
    ///
    /// `O(1)`; the returned polynomial is borrowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldPoly, FiniteField};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<3>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// assert_eq!(field.modulus().iter().map(|coefficient| coefficient.value()).collect::<Vec<_>>(), vec![1, 0, 1]);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn modulus(&self) -> &FieldPoly<F> {
        &self.params.modulus
    }

    /// Reduces a base-coefficient polynomial to its canonical quotient element.
    ///
    /// `coefficients[i]` is the coefficient of $x^i$. Any length is accepted;
    /// values of degree at least the modulus degree are reduced. Stored output
    /// always has exactly `relative_degree()` coefficients, including zero
    /// padding.
    ///
    /// # Errors
    ///
    /// - [`FieldError::IdentityMismatch`] when any coefficient is not in the
    ///   descriptor's base field.
    /// - Coordinate errors if the base carrier cannot rebuild one of its own
    ///   canonical values.
    ///
    /// # Complexity
    ///
    /// For `n` input coefficients and modulus degree `r`, polynomial long
    /// division costs `O(n r)` base-field operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldPoly, FiniteField};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<3>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// // x^2 = -1 = 2 in this quotient.
    /// let reduced = field.element(vec![Fp::new(0), Fp::new(0), Fp::new(1)])?;
    /// assert_eq!(reduced.coefficients(), &[Fp::new(2), Fp::new(0)]);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn element(&self, coefficients: Vec<F>) -> Result<QuotientElement<F>, FieldError> {
        let coefficients = coefficients
            .into_iter()
            .map(|coefficient| canonicalize_coefficient(&self.params.base_zero, &coefficient))
            .collect::<Result<Vec<_>, _>>()?;
        let polynomial = FieldPoly::new(coefficients);
        let (_, remainder) = polynomial.div_rem(&self.params.modulus);
        Ok(self.element_from_remainder(&remainder))
    }

    /// Returns the class of the polynomial indeterminate $x$.
    ///
    /// For a degree-one modulus this is its reduced constant representative;
    /// for every higher degree its coefficient vector is `[0, 1, 0, ...]`.
    ///
    /// # Complexity
    ///
    /// `O(r)` base-field operations for modulus degree `r`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldPoly, FiniteField};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<2>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// assert_eq!(field.indeterminate().coefficients(), &[Fp::new(0), Fp::new(1), Fp::new(0)]);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn indeterminate(&self) -> QuotientElement<F> {
        self.element(vec![
            self.params.base_zero.clone(),
            self.params.base_zero.one_like(),
        ])
        .expect("the quotient indeterminate uses canonical base coefficients")
    }

    /// Returns the field order $|B|^r$.
    ///
    /// # Errors
    ///
    /// [`FieldError::UnsupportedSize`] when the order exceeds `u128::MAX`.
    ///
    /// # Complexity
    ///
    /// `O(1)` checked exponentiation on the identity metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldPoly};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<3>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// assert_eq!(field.order()?, 9);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn order(&self) -> Result<u128, FieldError> {
        self.params
            .field_id
            .order()
            .ok_or_else(|| self.unsupported())
    }

    /// Materializes every element in ascending canonical-index order.
    ///
    /// This convenience is intentionally explicit and uncached. Index `i` is
    /// decoded in base `p` into canonical prime coordinates before rebuilding
    /// the corresponding quotient element.
    ///
    /// # Errors
    ///
    /// - [`FieldError::UnsupportedSize`] when the field order exceeds either
    ///   `u128::MAX` or `usize::MAX`, or when allocation cannot be reserved.
    /// - Coordinate reconstruction errors from the base carrier.
    ///
    /// # Complexity
    ///
    /// `O(|E| [E:GF(p)])` time and `O(|E| r)` stored base coefficients.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldPoly, FiniteField};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<3>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// let elements = field.elements()?;
    /// assert_eq!(elements.len(), 9);
    /// assert!(elements[0].is_zero());
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn elements(&self) -> Result<Vec<QuotientElement<F>>, FieldError> {
        let order = self.order()?;
        let count = usize::try_from(order).map_err(|_| self.unsupported())?;
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(count)
            .map_err(|_| self.unsupported())?;
        for index in 0..order {
            elements.push(self.element_at_canonical_index(index)?);
        }
        Ok(elements)
    }

    fn element_from_remainder(&self, remainder: &FieldPoly<F>) -> QuotientElement<F> {
        let mut coefficients = vec![self.params.base_zero.clone(); self.params.degree];
        for (destination, source) in coefficients.iter_mut().zip(remainder.iter()) {
            *destination = source.clone();
        }
        QuotientElement {
            coefficients,
            params: Arc::clone(&self.params),
        }
    }

    fn element_at_canonical_index(
        &self,
        mut index: u128,
    ) -> Result<QuotientElement<F>, FieldError> {
        let characteristic = u128::from(self.params.field_id.characteristic());
        let mut coordinates = Vec::with_capacity(self.params.absolute_degree);
        for _ in 0..self.params.absolute_degree {
            coordinates.push((index % characteristic) as u64);
            index /= characteristic;
        }
        debug_assert_eq!(index, 0);
        self.element_from_prime_coords(&coordinates)
    }

    fn element_from_prime_coords(
        &self,
        coordinates: &[u64],
    ) -> Result<QuotientElement<F>, FieldError> {
        let base_degree = self.params.certificate.base_id().degree();
        let coefficients = coordinates
            .chunks_exact(base_degree)
            .enumerate()
            .map(|(coefficient, chunk)| {
                self.params
                    .base_zero
                    .from_prime_coords(chunk)
                    .map_err(|error| shift_coordinate_index(error, coefficient * base_degree))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(QuotientElement {
            coefficients,
            params: Arc::clone(&self.params),
        })
    }

    fn unsupported(&self) -> FieldError {
        FieldError::UnsupportedSize {
            degree: self.params.absolute_degree,
            characteristic: self.params.field_id.characteristic(),
        }
    }
}

fn canonicalize_modulus<F: FieldIdentity>(
    base_zero: &F,
    modulus: FieldPoly<F>,
) -> Result<FieldPoly<F>, FieldError> {
    let degree = modulus
        .degree()
        .ok_or(FieldError::ModulusDegreeTooSmall { degree: 0 })?;
    if degree == 0 {
        return Err(FieldError::ModulusDegreeTooSmall { degree: 0 });
    }
    checked_degrees(&base_zero.field_id(), degree)?;
    let coefficients = modulus
        .iter()
        .map(|coefficient| canonicalize_coefficient(base_zero, coefficient))
        .collect::<Result<Vec<_>, _>>()?;
    let canonical = FieldPoly::new(coefficients);
    if !canonical.leading_coeff().is_some_and(FiniteField::is_one) {
        return Err(FieldError::NonMonicModulus);
    }
    Ok(canonical)
}

fn canonicalize_coefficient<F: FieldIdentity>(
    base_zero: &F,
    coefficient: &F,
) -> Result<F, FieldError> {
    let expected = base_zero.field_id();
    let found = coefficient.field_id();
    if expected != found {
        return Err(FieldError::IdentityMismatch { expected, found });
    }
    let mut coordinates = Vec::new();
    coefficient.write_prime_coords(&mut coordinates);
    base_zero.from_prime_coords(&coordinates)
}

fn checked_degrees(base: &FieldId, degree: usize) -> Result<usize, FieldError> {
    let unsupported = || FieldError::UnsupportedSize {
        degree,
        characteristic: base.characteristic(),
    };
    degree
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(unsupported)?;
    base.degree().checked_mul(degree).ok_or_else(unsupported)
}

fn polynomial_from_modulus<F: FieldIdentity>(
    base_zero: &F,
    modulus: &ModulusId,
) -> Result<FieldPoly<F>, FieldError> {
    let base_degree = base_zero.field_id().degree();
    let coefficients = (0..=modulus.degree())
        .map(|index| {
            base_zero
                .from_prime_coords(modulus.coefficient(index))
                .map_err(|error| shift_coordinate_index(error, index * base_degree))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FieldPoly::new(coefficients))
}

fn modulus_id<F: FieldIdentity>(
    base_id: &FieldId,
    modulus: &FieldPoly<F>,
) -> Result<ModulusId, FieldError> {
    let mut coordinates = Vec::new();
    let mut coefficient_coordinates = Vec::new();
    for coefficient in modulus.iter() {
        coefficient.write_prime_coords(&mut coefficient_coordinates);
        coordinates.extend_from_slice(&coefficient_coordinates);
    }
    ModulusId::new(base_id, coordinates)
}

fn build_field<F: FieldIdentity>(
    base_zero: F,
    modulus: FieldPoly<F>,
    certificate: ExtensionCertificate,
) -> Result<QuotientField<F>, FieldError> {
    let base_id = base_zero.field_id();
    let degree = modulus
        .degree()
        .ok_or(FieldError::ModulusDegreeTooSmall { degree: 0 })?;
    let absolute_degree = checked_degrees(&base_id, degree)?;
    let identity = FieldId::quotient(
        base_id.clone(),
        modulus_id(&base_id, &modulus)?,
        Basis::Polynomial,
    )?;
    if certificate.base_id() != &base_id {
        return Err(FieldError::IdentityMismatch {
            expected: base_id,
            found: certificate.base_id().clone(),
        });
    }
    if certificate.ext_id() != &identity {
        return Err(FieldError::IdentityMismatch {
            expected: identity,
            found: certificate.ext_id().clone(),
        });
    }
    Ok(QuotientField {
        params: Arc::new(QuotientParams {
            base_zero,
            modulus,
            degree,
            absolute_degree,
            field_id: certificate.ext_id().clone(),
            certificate,
        }),
    })
}

fn shift_coordinate_index(error: FieldError, offset: usize) -> FieldError {
    match error {
        FieldError::CoordinateOutOfRange {
            index,
            value,
            characteristic,
        } => FieldError::CoordinateOutOfRange {
            index: index + offset,
            value,
            characteristic,
        },
        other => other,
    }
}

/// An element of a runtime polynomial quotient field.
///
/// Coefficients are always reduced to degree below the modulus and padded to
/// exactly the relative degree. Equality and hashing include the algebraic
/// [`FieldId`], so equal coordinate vectors in distinct presentations remain
/// distinct values.
///
/// # Examples
///
/// ```
/// use gf2_core::field::{ConstField, FieldPoly};
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{QuotientElement, QuotientField};
///
/// let field = QuotientField::new(
///     Fp::<3>::zero(),
///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
/// )?;
/// let element: QuotientElement<Fp<3>> = field.element(vec![Fp::new(2), Fp::new(1)])?;
/// assert_eq!(element.coefficients(), &[Fp::new(2), Fp::new(1)]);
/// # Ok::<(), gf2_core::field::FieldError>(())
/// ```
pub struct QuotientElement<F: FieldIdentity> {
    coefficients: Vec<F>,
    params: Arc<QuotientParams<F>>,
}

impl<F: FieldIdentity> Clone for QuotientElement<F> {
    fn clone(&self) -> Self {
        Self {
            coefficients: self.coefficients.clone(),
            params: Arc::clone(&self.params),
        }
    }
}

impl<F: FieldIdentity> fmt::Debug for QuotientElement<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotientElement")
            .field("coefficients", &self.coefficients)
            .field("field", &self.params.field_id)
            .finish()
    }
}

impl<F: FieldIdentity> PartialEq for QuotientElement<F> {
    fn eq(&self, other: &Self) -> bool {
        self.params.field_id == other.params.field_id && self.coefficients == other.coefficients
    }
}

impl<F: FieldIdentity> Eq for QuotientElement<F> {}

impl<F: FieldIdentity> Hash for QuotientElement<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.params.field_id.hash(state);
        self.coefficients.hash(state);
    }
}

impl<F: FieldIdentity> QuotientElement<F> {
    /// Returns the fixed-length base-coefficient vector, constant first.
    ///
    /// # Complexity
    ///
    /// `O(1)`; the slice is borrowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldPoly};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<3>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// let a = field.element(vec![Fp::new(2)])?;
    /// assert_eq!(a.coefficients(), &[Fp::new(2), Fp::new(0)]);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn coefficients(&self) -> &[F] {
        &self.coefficients
    }

    /// Returns a descriptor for this element's field.
    ///
    /// # Complexity
    ///
    /// `O(1)`; cloning the descriptor increments an `Arc` count.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldExtension, FieldPoly};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<3>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// assert_eq!(field.indeterminate().field().ext_id(), field.ext_id());
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn field(&self) -> QuotientField<F> {
        QuotientField {
            params: Arc::clone(&self.params),
        }
    }

    /// Applies the `k`-fold absolute Frobenius $a \mapsto a^{p^k}$.
    ///
    /// `k` is reduced modulo the absolute degree before exponentiation, so
    /// the exponent itself never overflows `u64`.
    ///
    /// # Complexity
    ///
    /// `O((k mod d) log(p) r^2)` base-field operations for absolute degree
    /// `d` and relative degree `r`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{ConstField, FieldPoly};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::QuotientField;
    ///
    /// let field = QuotientField::new(
    ///     Fp::<2>::zero(),
    ///     FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]),
    /// )?;
    /// let x = field.indeterminate();
    /// assert_eq!(x.frobenius(3), x);
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn frobenius(&self, k: usize) -> Self {
        let mut result = self.clone();
        let steps = k % self.params.absolute_degree;
        let characteristic = self.params.field_id.characteristic();
        for _ in 0..steps {
            result = result.pow(characteristic);
        }
        result
    }

    fn assert_compatible(&self, other: &Self) {
        assert!(
            self.params.field_id == other.params.field_id,
            "quotient arithmetic requires elements of one algebraic field"
        );
    }

    fn coefficients_in_context(&self, other: &Self) -> Vec<F> {
        if Arc::ptr_eq(&self.params, &other.params) {
            return other.coefficients.clone();
        }
        other
            .coefficients
            .iter()
            .map(|coefficient| {
                canonicalize_coefficient(&self.params.base_zero, coefficient)
                    .expect("equal quotient identities have equal base-coordinate semantics")
            })
            .collect()
    }

    fn multiply(&self, other: &Self) -> Self {
        self.assert_compatible(other);
        let degree = self.params.degree;
        let zero = self.params.base_zero.clone();
        let mut product = vec![zero; 2 * degree - 1];
        let right_coefficients = self.coefficients_in_context(other);
        for (left_index, left) in self.coefficients.iter().enumerate() {
            for (right_index, right) in right_coefficients.iter().enumerate() {
                product[left_index + right_index] += left.clone() * right;
            }
        }
        for high in (degree..product.len()).rev() {
            let leading = product[high].clone();
            if leading.is_zero() {
                continue;
            }
            let offset = high - degree;
            for (index, coefficient) in self.params.modulus.iter().take(degree).enumerate() {
                product[offset + index] =
                    product[offset + index].clone() - leading.clone() * coefficient;
            }
        }
        product.truncate(degree);
        Self {
            coefficients: product,
            params: Arc::clone(&self.params),
        }
    }

    fn inverse_euclid(&self) -> Option<Self> {
        let reduced = euclid_inverse(
            &self.params.modulus,
            &FieldPoly::new(self.coefficients.clone()),
        )?;
        Some(self.field().element_from_remainder(&reduced))
    }
}

/// Returns the residue $u$ with $u \cdot v \equiv 1 \pmod{f}$ for a monic `modulus`
/// $f$ and a representative `value` $v$, or `None` when $v$ is zero.
///
/// This is the single extended-Euclid implementation both quotient carriers
/// use, so the runtime and compile-time forms invert by the same procedure
/// rather than by two parallel ones. It returns the Bezout coefficient reduced
/// modulo `f`; each carrier stores it in its own coefficient layout.
///
/// When `f` is reducible the Bezout run still terminates, and a `value` sharing
/// a factor with `f` yields a residue that is not an inverse. That is the
/// declared-modulus caller contract, not a condition this function decides.
///
/// # Complexity
///
/// `O(r^2)` base-field operations for modulus degree `r`, using the schoolbook
/// polynomial substrate.
fn euclid_inverse<F: FieldIdentity>(
    modulus: &FieldPoly<F>,
    value: &FieldPoly<F>,
) -> Option<FieldPoly<F>> {
    if value.is_zero() {
        return None;
    }
    let sample = modulus.leading_coeff()?;
    let mut old_remainder = modulus.clone();
    let mut remainder = value.clone();
    let mut old_coefficient = FieldPoly::zero_like(sample);
    let mut coefficient = FieldPoly::one_like(sample);

    while !remainder.is_zero() {
        let (quotient, next_remainder) = old_remainder.div_rem(&remainder);
        let next_coefficient = &old_coefficient - &(&quotient * &coefficient);
        old_remainder = remainder;
        remainder = next_remainder;
        old_coefficient = coefficient;
        coefficient = next_coefficient;
    }

    let inverse_leading = old_remainder.leading_coeff()?.inv()?;
    old_coefficient.scale(&inverse_leading);
    let (_, reduced) = old_coefficient.div_rem(modulus);
    Some(reduced)
}

impl<F: FieldIdentity> Add for &QuotientElement<F> {
    type Output = QuotientElement<F>;

    /// Adds coefficient-wise in `O(r)` base-field additions.
    fn add(self, rhs: Self) -> Self::Output {
        self.assert_compatible(rhs);
        let right_coefficients = self.coefficients_in_context(rhs);
        QuotientElement {
            coefficients: self
                .coefficients
                .iter()
                .zip(&right_coefficients)
                .map(|(left, right)| left.clone() + right)
                .collect(),
            params: Arc::clone(&self.params),
        }
    }
}

impl<F: FieldIdentity> Add for QuotientElement<F> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        &self + &rhs
    }
}

impl<F: FieldIdentity> Add<&Self> for QuotientElement<F> {
    type Output = Self;

    fn add(self, rhs: &Self) -> Self::Output {
        &self + rhs
    }
}

impl<F: FieldIdentity> AddAssign for QuotientElement<F> {
    fn add_assign(&mut self, rhs: Self) {
        *self = &*self + &rhs;
    }
}

impl<F: FieldIdentity> AddAssign<&Self> for QuotientElement<F> {
    fn add_assign(&mut self, rhs: &Self) {
        *self = &*self + rhs;
    }
}

impl<F: FieldIdentity> Sub for &QuotientElement<F> {
    type Output = QuotientElement<F>;

    /// Subtracts coefficient-wise in `O(r)` base-field subtractions.
    fn sub(self, rhs: Self) -> Self::Output {
        self.assert_compatible(rhs);
        let right_coefficients = self.coefficients_in_context(rhs);
        QuotientElement {
            coefficients: self
                .coefficients
                .iter()
                .zip(&right_coefficients)
                .map(|(left, right)| left.clone() - right)
                .collect(),
            params: Arc::clone(&self.params),
        }
    }
}

impl<F: FieldIdentity> Sub for QuotientElement<F> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        &self - &rhs
    }
}

impl<F: FieldIdentity> Sub<&Self> for QuotientElement<F> {
    type Output = Self;

    fn sub(self, rhs: &Self) -> Self::Output {
        &self - rhs
    }
}

impl<F: FieldIdentity> Neg for &QuotientElement<F> {
    type Output = QuotientElement<F>;

    /// Negates every coefficient in `O(r)` base-field negations.
    fn neg(self) -> Self::Output {
        QuotientElement {
            coefficients: self
                .coefficients
                .iter()
                .map(|coefficient| -coefficient.clone())
                .collect(),
            params: Arc::clone(&self.params),
        }
    }
}

impl<F: FieldIdentity> Neg for QuotientElement<F> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        -&self
    }
}

impl<F: FieldIdentity> Mul for &QuotientElement<F> {
    type Output = QuotientElement<F>;

    /// Multiplies and reduces in `O(r^2)` base-field operations.
    fn mul(self, rhs: Self) -> Self::Output {
        self.multiply(rhs)
    }
}

impl<F: FieldIdentity> Mul for QuotientElement<F> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        &self * &rhs
    }
}

impl<F: FieldIdentity> Mul<&Self> for QuotientElement<F> {
    type Output = Self;

    fn mul(self, rhs: &Self) -> Self::Output {
        &self * rhs
    }
}

impl<F: FieldIdentity> Div for &QuotientElement<F> {
    type Output = QuotientElement<F>;

    /// Divides through extended-Euclid inversion.
    ///
    /// # Panics
    ///
    /// Panics when `rhs` is zero, matching the crate's other field carriers.
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self::Output {
        self * &rhs
            .inv()
            .expect("division by zero in a polynomial quotient field")
    }
}

impl<F: FieldIdentity> Div for QuotientElement<F> {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        &self / &rhs
    }
}

impl<F: FieldIdentity> Div<&Self> for QuotientElement<F> {
    type Output = Self;

    fn div(self, rhs: &Self) -> Self::Output {
        &self / rhs
    }
}

impl<F: FieldIdentity> FiniteField for QuotientElement<F> {
    type Characteristic = u64;
    type Wide = Self;

    fn characteristic(&self) -> Self::Characteristic {
        self.params.field_id.characteristic()
    }

    fn extension_degree(&self) -> usize {
        self.params.absolute_degree
    }

    fn is_zero(&self) -> bool {
        self.coefficients.iter().all(FiniteField::is_zero)
    }

    fn is_one(&self) -> bool {
        self.coefficients[0].is_one() && self.coefficients[1..].iter().all(FiniteField::is_zero)
    }

    /// Inverts with polynomial extended Euclid in `O(r^2)` base-field
    /// operations using schoolbook polynomial arithmetic.
    fn inv(&self) -> Option<Self> {
        self.inverse_euclid()
    }

    fn zero_like(&self) -> Self {
        QuotientElement {
            coefficients: vec![self.params.base_zero.clone(); self.params.degree],
            params: Arc::clone(&self.params),
        }
    }

    fn one_like(&self) -> Self {
        let mut coefficients = vec![self.params.base_zero.clone(); self.params.degree];
        coefficients[0] = self.params.base_zero.one_like();
        QuotientElement {
            coefficients,
            params: Arc::clone(&self.params),
        }
    }

    fn to_wide(&self) -> Self::Wide {
        self.clone()
    }

    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide {
        self * rhs
    }

    fn reduce_wide(wide: &Self::Wide) -> Self {
        wide.clone()
    }

    fn max_unreduced_additions() -> usize {
        usize::MAX
    }
}

impl<F: FieldIdentity> FieldIdentity for QuotientElement<F> {
    fn field_id(&self) -> FieldId {
        self.params.field_id.clone()
    }

    fn write_prime_coords(&self, out: &mut Vec<u64>) {
        out.clear();
        let mut coefficient_coordinates = Vec::new();
        for coefficient in &self.coefficients {
            coefficient.write_prime_coords(&mut coefficient_coordinates);
            out.extend_from_slice(&coefficient_coordinates);
        }
    }

    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError> {
        if coords.len() != self.params.absolute_degree {
            return Err(FieldError::CoordinateCountMismatch {
                expected: self.params.absolute_degree,
                found: coords.len(),
            });
        }
        self.field().element_from_prime_coords(coords)
    }
}

impl<F: FieldIdentity> FieldExtension for QuotientField<F> {
    type Base = F;
    type Ext = QuotientElement<F>;

    fn certificate(&self) -> &ExtensionCertificate {
        &self.params.certificate
    }

    fn base_zero(&self) -> Self::Base {
        self.params.base_zero.clone()
    }

    fn embed(&self, x: &Self::Base) -> Self::Ext {
        self.element(vec![x.clone()])
            .expect("FieldExtension::embed receives an element of its named base")
    }

    fn try_restrict(&self, x: &Self::Ext) -> Option<Self::Base> {
        if x.params.field_id != self.params.field_id
            || x.coefficients[1..]
                .iter()
                .any(|coefficient| !coefficient.is_zero())
        {
            return None;
        }
        canonicalize_coefficient(&self.params.base_zero, &x.coefficients[0]).ok()
    }

    fn contains(&self, x: &Self::Ext) -> bool {
        x.params.field_id == self.params.field_id
            && x.coefficients[1..].iter().all(FiniteField::is_zero)
    }
}

// ---------------------------------------------------------------------------
// Compile-time configured quotient
// ---------------------------------------------------------------------------

/// Compile-time declaration of the quotient $B\lbrack x\rbrack/(f)$ of relative degree `R`.
///
/// The implementor is a zero-sized marker naming the base field and the monic
/// modulus, in the spirit of [`ExtConfig`](crate::gfpn::ExtConfig) but for an
/// arbitrary degree rather than a binomial one. [`MODULUS`](Self::MODULUS)
/// holds the low coefficients $f_0, \ldots, f_{R-1}$ constant first; the
/// leading $f_R = 1$ is implicit, the same convention
/// [`Gf2mWideConfig`](crate::gf2m::Gf2mWideConfig) uses for its top bit.
///
/// Declaring the modulus does not decide it. [`ConstQuotient::extension`] runs
/// [`prove_irreducible`] over the declaration and reports a typed error when it
/// is reducible; see `@/inv/caller-trusted-fast-paths` for the trust model
/// [`ConstQuotient::extension_unchecked`] applies instead.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::FieldExtension;
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{ConstQuotient, ConstQuotientConfig};
///
/// // GF(2^4) = GF(2)[x] / (x^4 + x + 1). The leading one is not listed.
/// struct Gf16;
/// impl ConstQuotientConfig<4> for Gf16 {
///     type BaseField = Fp<2>;
///     const MODULUS: [Fp<2>; 4] = [Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(0)];
/// }
///
/// assert_eq!(ConstQuotient::<4, Gf16>::extension()?.ext_order(), Some(16));
/// # Ok::<(), gf2_core::field::FieldError>(())
/// ```
pub trait ConstQuotientConfig<const R: usize> {
    /// The base field $B$, itself a compile-time carrier with an identity.
    type BaseField: ConstField + FieldIdentity;

    /// The low modulus coefficients $f_0, \ldots, f_{R-1}$, constant first.
    ///
    /// The leading coefficient $f_R = 1$ is implicit, so the declared modulus
    /// is $x^{R} + \sum_{i<R} f_i x^{i}$ and is monic by construction.
    const MODULUS: [Self::BaseField; R];
}

/// An element of the compile-time quotient $B\lbrack x\rbrack/(f)$ declared by `C`.
///
/// The element is exactly its `R` base coefficients, constant first: no modulus,
/// no descriptor, and no certificate is stored per value, and every arithmetic
/// path monomorphizes over [`ConstQuotientConfig::MODULUS`]. Validation is
/// therefore never on an arithmetic path — it happens once, when a caller asks
/// [`extension`](Self::extension) for a witness.
///
/// This carrier and [`QuotientField`] present the same field whenever they
/// declare the same base and modulus: the two share one [`FieldId`], write
/// identical canonical coordinates, and
/// [`convert_element`](crate::field::extension::convert_element) transports
/// elements between them. The module documentation states the full equivalence
/// contract.
///
/// Equality and hashing cover the coefficients only, because the type already
/// fixes the presentation.
pub struct ConstQuotient<const R: usize, C: ConstQuotientConfig<R>> {
    coefficients: [C::BaseField; R],
}

/// The extension witness for a compile-time quotient, and the compile-time
/// counterpart of [`QuotientExt`].
///
/// Build it with [`ConstQuotient::extension`] (deciding) or
/// [`ConstQuotient::extension_unchecked`] (trusting).
pub type ConstQuotientExt<const R: usize, C> = ConstExt<ConstQuotient<R, C>>;

/// Returns the canonical [`ModulusId`] of the declaration `C`, restoring the
/// implicit leading one.
fn const_modulus_id<const R: usize, C: ConstQuotientConfig<R>>() -> Result<ModulusId, FieldError> {
    let base_zero = <C::BaseField as ConstField>::zero();
    let base_id = base_zero.field_id();
    let modulus = C::MODULUS;
    let mut coordinates = Vec::with_capacity((R + 1) * base_id.degree());
    let mut buffer = Vec::new();
    for coefficient in modulus.iter() {
        coefficient.write_prime_coords(&mut buffer);
        coordinates.extend_from_slice(&buffer);
    }
    <C::BaseField as ConstField>::one().write_prime_coords(&mut buffer);
    coordinates.extend_from_slice(&buffer);
    ModulusId::new(&base_id, coordinates)
}

impl<const R: usize, C: ConstQuotientConfig<R>> ConstQuotient<R, C> {
    /// Wraps an already reduced coefficient array, constant first.
    ///
    /// Every array of `R` base coefficients is a canonical representative, so
    /// no reduction runs. Use [`reduce`](Self::reduce) for a longer vector.
    ///
    /// # Panics
    ///
    /// Fails to compile, rather than panicking, when `R` is zero: a quotient
    /// modulus has degree at least one.
    ///
    /// # Complexity
    ///
    /// `O(1)`; the array is moved.
    pub fn new(coefficients: [C::BaseField; R]) -> Self {
        const { assert!(R > 0, "a quotient modulus has degree at least one") };
        Self { coefficients }
    }

    /// Reduces a coefficient slice of any length modulo the declared modulus.
    ///
    /// `coefficients[i]` is the coefficient of $x^i$. The compile-time
    /// counterpart of [`QuotientField::element`], and the one construction path
    /// that allocates.
    ///
    /// # Complexity
    ///
    /// `O(n R)` base-field operations for `n` input coefficients.
    pub fn reduce(coefficients: &[C::BaseField]) -> Self {
        let (_, remainder) = FieldPoly::new(coefficients.to_vec()).div_rem(&Self::modulus());
        Self::from_remainder(&remainder)
    }

    /// Returns the fixed-length coefficient array, constant first.
    ///
    /// # Complexity
    ///
    /// `O(1)`; the array is borrowed.
    pub fn coefficients(&self) -> &[C::BaseField; R] {
        &self.coefficients
    }

    /// Returns the declared monic modulus with its implicit leading one
    /// restored, constant coefficient first.
    ///
    /// # Complexity
    ///
    /// `O(R)`; the polynomial is materialized from the associated constant.
    pub fn modulus() -> FieldPoly<C::BaseField> {
        let mut coefficients = C::MODULUS.to_vec();
        coefficients.push(<C::BaseField as ConstField>::one());
        FieldPoly::new(coefficients)
    }

    /// Returns the class of the polynomial indeterminate $x$.
    ///
    /// For a degree-one modulus this is its reduced constant representative;
    /// for every higher degree the coefficient vector is `[0, 1, 0, ...]`.
    ///
    /// # Complexity
    ///
    /// `O(R)` base-field operations.
    pub fn indeterminate() -> Self {
        Self::reduce(&[
            <C::BaseField as ConstField>::zero(),
            <C::BaseField as ConstField>::one(),
        ])
    }

    /// Decides the declared modulus and returns the extension witness.
    ///
    /// This is the validating construction path. It runs [`prove_irreducible`]
    /// over the declaration and, on success, returns a witness carrying
    /// [`CertificateBasis::Proved`](crate::field::extension::CertificateBasis::Proved).
    /// Every in-tree [`ConstQuotientConfig`] is built through this path in the
    /// shared conformance harness, so in-tree declarations are decided in CI.
    ///
    /// # Errors
    ///
    /// - [`FieldError::ReducibleModulus`] when the complete decision finds a
    ///   factor of the declared modulus.
    /// - [`FieldError::ModulusDegreeTooSmall`] or
    ///   [`FieldError::UnsupportedSize`] when the declared degree cannot name a
    ///   quotient.
    /// - [`FieldError::IdentityMismatch`] when the resulting certificate does
    ///   not name this declaration, which is an implementation defect rather
    ///   than a caller error.
    ///
    /// # Complexity
    ///
    /// The `O(d log(p) R^3)` conservative bound of [`prove_irreducible`], for
    /// base degree `d`, characteristic `p`, and relative degree `R`. The
    /// returned witness is the memo: holding it is what keeps repeated use from
    /// repeating the decision.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldError;
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ConstQuotient, ConstQuotientConfig};
    ///
    /// // x^4 + 1 = (x + 1)^4 over GF(2): a declaration that is not a field.
    /// struct Reducible;
    /// impl ConstQuotientConfig<4> for Reducible {
    ///     type BaseField = Fp<2>;
    ///     const MODULUS: [Fp<2>; 4] = [Fp::new(1), Fp::new(0), Fp::new(0), Fp::new(0)];
    /// }
    ///
    /// assert!(matches!(
    ///     ConstQuotient::<4, Reducible>::extension(),
    ///     Err(FieldError::ReducibleModulus { .. })
    /// ));
    /// ```
    pub fn extension() -> Result<ConstQuotientExt<R, C>, FieldError> {
        let base_zero = <C::BaseField as ConstField>::zero();
        let certificate = prove_irreducible(&Self::modulus(), &base_zero)?;
        ConstExt::from_certificate_unchecked(certificate.extension_certificate())
    }

    /// Returns the extension witness without deciding the declared modulus.
    ///
    /// The witness records
    /// [`CertificateBasis::Declared`](crate::field::extension::CertificateBasis::Declared),
    /// which is the honest name for evidence that rests on the type-level
    /// declaration alone.
    ///
    /// # Contract
    ///
    /// The caller promises that the declared modulus is irreducible over the
    /// declared base. Violating that precondition gives unspecified
    /// mathematical results (GIGO) — the carrier is then a ring that is not a
    /// field, and inversion in particular returns a residue that is not an
    /// inverse — never memory unsafety. This is the caller-trusted
    /// validation-cost fast path fixed by `@/inv/caller-trusted-fast-paths`;
    /// [`extension`](Self::extension) is the deciding counterpart.
    ///
    /// # Complexity
    ///
    /// `O(R)` structural work on the identities; no decision procedure runs.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::{CertificateBasis, FieldExtension};
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ConstQuotient, ConstQuotientConfig};
    ///
    /// struct Gf16;
    /// impl ConstQuotientConfig<4> for Gf16 {
    ///     type BaseField = Fp<2>;
    ///     const MODULUS: [Fp<2>; 4] = [Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(0)];
    /// }
    ///
    /// let trusted = ConstQuotient::<4, Gf16>::extension_unchecked();
    /// let decided = ConstQuotient::<4, Gf16>::extension()?;
    /// assert_eq!(trusted.certificate().basis(), CertificateBasis::Declared);
    /// assert_eq!(decided.certificate().basis(), CertificateBasis::Proved);
    /// assert_eq!(trusted.ext_id(), decided.ext_id());
    /// # Ok::<(), gf2_core::field::FieldError>(())
    /// ```
    pub fn extension_unchecked() -> ConstQuotientExt<R, C> {
        ConstExt::new()
    }

    /// Returns the runtime descriptor for this same presentation.
    ///
    /// The two carriers produce equal [`FieldId`]s, so this is the bridge a
    /// caller crosses to move a compile-time field into exploratory runtime
    /// code. It decides the modulus exactly as [`QuotientField::new`] does; a
    /// caller already holding an [`IrreducibilityCertificate`] uses
    /// [`QuotientField::from_certificate`] instead of paying twice.
    ///
    /// # Errors
    ///
    /// The conditions of [`QuotientField::new`], for the declared modulus.
    ///
    /// # Complexity
    ///
    /// One [`prove_irreducible`] decision over the declared modulus.
    pub fn runtime_field() -> Result<QuotientField<C::BaseField>, FieldError> {
        QuotientField::new(<C::BaseField as ConstField>::zero(), Self::modulus())
    }

    /// Copies a reduced polynomial into the fixed-size coefficient array,
    /// zero-padding the unused high positions.
    fn from_remainder(remainder: &FieldPoly<C::BaseField>) -> Self {
        let mut coefficients = [<C::BaseField as ConstField>::zero(); R];
        for (slot, value) in coefficients.iter_mut().zip(remainder.iter()) {
            *slot = *value;
        }
        Self::new(coefficients)
    }

    /// Multiplies and reduces without allocating.
    ///
    /// Horner's scheme over the left operand: each step multiplies the
    /// accumulator by $x$ — a shift plus the rewrite
    /// $x^{R} = -\sum_{i<R} f_i x^{i}$ — and then adds a scalar multiple of the
    /// right operand. A `2R - 1` convolution buffer is never materialized, so
    /// the whole product stays in one `R`-wide array.
    fn multiply(&self, other: &Self) -> Self {
        let modulus = C::MODULUS;
        let zero = <C::BaseField as ConstField>::zero();
        let mut accumulator = [zero; R];
        for index in (0..R).rev() {
            let carry = accumulator[R - 1];
            for position in (1..R).rev() {
                accumulator[position] = accumulator[position - 1];
            }
            accumulator[0] = zero;
            if !carry.is_zero() {
                for (slot, coefficient) in accumulator.iter_mut().zip(modulus.iter()) {
                    *slot = *slot - carry * *coefficient;
                }
            }
            let scalar = self.coefficients[index];
            if !scalar.is_zero() {
                for (slot, coefficient) in accumulator.iter_mut().zip(other.coefficients.iter()) {
                    *slot += scalar * *coefficient;
                }
            }
        }
        Self::new(accumulator)
    }

    /// Inverts through the shared [`euclid_inverse`] procedure.
    fn inverse(&self) -> Option<Self> {
        let reduced = euclid_inverse(
            &Self::modulus(),
            &FieldPoly::new(self.coefficients.to_vec()),
        )?;
        Some(Self::from_remainder(&reduced))
    }
}

// Manual trait impls, so that no bound lands on the config marker itself; only
// `C::BaseField` needs them, and `ConstField: FiniteField + Copy` supplies them.

impl<const R: usize, C: ConstQuotientConfig<R>> Clone for ConstQuotient<R, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Copy for ConstQuotient<R, C> {}

impl<const R: usize, C: ConstQuotientConfig<R>> PartialEq for ConstQuotient<R, C> {
    fn eq(&self, other: &Self) -> bool {
        self.coefficients == other.coefficients
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Eq for ConstQuotient<R, C> {}

impl<const R: usize, C: ConstQuotientConfig<R>> Hash for ConstQuotient<R, C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.coefficients.hash(state);
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> fmt::Debug for ConstQuotient<R, C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConstQuotient")
            .field("coefficients", &self.coefficients)
            .field("field", &<Self as FieldIdentity>::field_id_hint())
            .finish()
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Add for ConstQuotient<R, C> {
    type Output = Self;

    /// Adds coefficient-wise in `O(R)` base-field additions.
    fn add(mut self, rhs: Self) -> Self {
        for (slot, value) in self.coefficients.iter_mut().zip(rhs.coefficients.iter()) {
            *slot += *value;
        }
        self
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Sub for ConstQuotient<R, C> {
    type Output = Self;

    /// Subtracts coefficient-wise in `O(R)` base-field subtractions.
    fn sub(mut self, rhs: Self) -> Self {
        for (slot, value) in self.coefficients.iter_mut().zip(rhs.coefficients.iter()) {
            *slot = *slot - *value;
        }
        self
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Neg for ConstQuotient<R, C> {
    type Output = Self;

    /// Negates every coefficient in `O(R)` base-field negations.
    fn neg(mut self) -> Self {
        for slot in self.coefficients.iter_mut() {
            *slot = -*slot;
        }
        self
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Mul for ConstQuotient<R, C> {
    type Output = Self;

    /// Multiplies and reduces in `O(R^2)` base-field operations.
    fn mul(self, rhs: Self) -> Self {
        self.multiply(&rhs)
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Div for ConstQuotient<R, C> {
    type Output = Self;

    /// Divides through extended-Euclid inversion.
    ///
    /// # Panics
    ///
    /// Panics when `rhs` is zero, matching the crate's other field carriers.
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self {
        self * rhs
            .inv()
            .expect("division by zero in a polynomial quotient field")
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> AddAssign for ConstQuotient<R, C> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> AddAssign<&Self> for ConstQuotient<R, C> {
    fn add_assign(&mut self, rhs: &Self) {
        *self = *self + *rhs;
    }
}

// ---------------------------------------------------------------------------
// Reference-forwarding operators (ConstQuotient is Copy, so dereference)
// ---------------------------------------------------------------------------

impl<const R: usize, C: ConstQuotientConfig<R>> Add<&ConstQuotient<R, C>> for ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn add(self, rhs: &ConstQuotient<R, C>) -> ConstQuotient<R, C> {
        self + *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Add for &ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn add(self, rhs: Self) -> ConstQuotient<R, C> {
        *self + *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Sub<&ConstQuotient<R, C>> for ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn sub(self, rhs: &ConstQuotient<R, C>) -> ConstQuotient<R, C> {
        self - *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Sub for &ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn sub(self, rhs: Self) -> ConstQuotient<R, C> {
        *self - *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Mul<&ConstQuotient<R, C>> for ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn mul(self, rhs: &ConstQuotient<R, C>) -> ConstQuotient<R, C> {
        self * *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Mul for &ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn mul(self, rhs: Self) -> ConstQuotient<R, C> {
        *self * *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Div<&ConstQuotient<R, C>> for ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn div(self, rhs: &ConstQuotient<R, C>) -> ConstQuotient<R, C> {
        self / *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Div for &ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn div(self, rhs: Self) -> ConstQuotient<R, C> {
        *self / *rhs
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> Neg for &ConstQuotient<R, C> {
    type Output = ConstQuotient<R, C>;

    fn neg(self) -> ConstQuotient<R, C> {
        -(*self)
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> FiniteField for ConstQuotient<R, C> {
    type Characteristic = u64;
    type Wide = Self;

    fn characteristic(&self) -> Self::Characteristic {
        <C::BaseField as ConstField>::zero().characteristic()
    }

    fn extension_degree(&self) -> usize {
        R * <C::BaseField as ConstField>::zero().extension_degree()
    }

    fn is_zero(&self) -> bool {
        self.coefficients.iter().all(FiniteField::is_zero)
    }

    fn is_one(&self) -> bool {
        self.coefficients[0].is_one() && self.coefficients[1..].iter().all(FiniteField::is_zero)
    }

    /// Inverts with polynomial extended Euclid in `O(R^2)` base-field
    /// operations, through the same procedure the runtime carrier uses.
    fn inv(&self) -> Option<Self> {
        self.inverse()
    }

    fn zero_like(&self) -> Self {
        <Self as ConstField>::zero()
    }

    fn one_like(&self) -> Self {
        <Self as ConstField>::one()
    }

    fn zero_hint() -> Option<Self> {
        Some(<Self as ConstField>::zero())
    }

    /// Static cardinality hint `floor(log2(p^(d R)))`, computed from the
    /// characteristic and absolute degree so that it stays exact for an odd
    /// characteristic and answers `None` instead of panicking when the order
    /// exceeds `u128`.
    fn cardinality_log2_hint() -> Option<u32> {
        let base_zero = <C::BaseField as ConstField>::zero();
        let degree = R.checked_mul(base_zero.extension_degree())?;
        u128::from(base_zero.characteristic())
            .checked_pow(u32::try_from(degree).ok()?)
            .map(u128::ilog2)
    }

    fn to_wide(&self) -> Self::Wide {
        *self
    }

    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide {
        self * rhs
    }

    fn reduce_wide(wide: &Self::Wide) -> Self {
        *wide
    }

    /// Unbounded: `Wide` is the field itself, so an accumulated "wide" value is
    /// an ordinary reduced element and no accumulation budget applies.
    fn max_unreduced_additions() -> usize {
        usize::MAX
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> ConstField for ConstQuotient<R, C> {
    fn zero() -> Self {
        Self::new([<C::BaseField as ConstField>::zero(); R])
    }

    fn one() -> Self {
        let mut coefficients = [<C::BaseField as ConstField>::zero(); R];
        coefficients[0] = <C::BaseField as ConstField>::one();
        Self::new(coefficients)
    }

    /// Returns $|B|^{R}$.
    ///
    /// # Panics
    ///
    /// Panics when the order exceeds `u128::MAX`, which the trait permits;
    /// probe [`FiniteField::cardinality_log2_hint`] first for a non-panicking
    /// width query.
    fn order() -> u128 {
        <C::BaseField as ConstField>::order()
            .checked_pow(u32::try_from(R).expect("a relative degree fits in u32"))
            .expect("the compile-time quotient order exceeds u128")
    }
}

/// The identity is the declared presentation: the base identity, the declared
/// [`ModulusId`], and [`Basis::Polynomial`] — the same triple
/// [`QuotientField`] reports for the same declaration.
impl<const R: usize, C: ConstQuotientConfig<R>> FieldIdentity for ConstQuotient<R, C> {
    /// # Panics
    ///
    /// Panics when the declared modulus does not name a presentation over its
    /// base, which no `ConstQuotientConfig` implementation with in-range
    /// coefficients produces.
    fn field_id(&self) -> FieldId {
        Self::field_id_hint()
            .expect("a ConstQuotientConfig declares a monic modulus over its base field")
    }

    fn field_id_hint() -> Option<FieldId> {
        let base_id = <C::BaseField as ConstField>::zero().field_id();
        FieldId::quotient(base_id, const_modulus_id::<R, C>().ok()?, Basis::Polynomial).ok()
    }

    fn write_prime_coords(&self, out: &mut Vec<u64>) {
        out.clear();
        let mut buffer = Vec::new();
        for coefficient in &self.coefficients {
            coefficient.write_prime_coords(&mut buffer);
            out.extend_from_slice(&buffer);
        }
    }

    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError> {
        let base_zero = <C::BaseField as ConstField>::zero();
        let base_degree = base_zero.field_id().degree();
        let expected = R * base_degree;
        if coords.len() != expected {
            return Err(FieldError::CoordinateCountMismatch {
                expected,
                found: coords.len(),
            });
        }
        let mut coefficients = [base_zero; R];
        for (index, (slot, chunk)) in coefficients
            .iter_mut()
            .zip(coords.chunks_exact(base_degree))
            .enumerate()
        {
            *slot = base_zero
                .from_prime_coords(chunk)
                .map_err(|error| shift_coordinate_index(error, index * base_degree))?;
        }
        Ok(Self::new(coefficients))
    }
}

impl<const R: usize, C: ConstQuotientConfig<R>> ConstSimpleExtension for ConstQuotient<R, C> {
    type ConstBase = C::BaseField;

    fn from_base(x: Self::ConstBase) -> Self {
        let mut coefficients = [<C::BaseField as ConstField>::zero(); R];
        coefficients[0] = x;
        Self::new(coefficients)
    }

    fn try_into_base(self) -> Option<Self::ConstBase> {
        self.coefficients[1..]
            .iter()
            .all(FiniteField::is_zero)
            .then_some(self.coefficients[0])
    }

    fn modulus_id() -> ModulusId {
        const_modulus_id::<R, C>()
            .expect("a ConstQuotientConfig declares a monic modulus over its base field")
    }
}

// ---------------------------------------------------------------------------
// In-tree compile-time declarations
//
// Every declaration here is decided by `prove_irreducible` and registered in
// the shared conformance harness (`field/axiom_tests.rs`), which imports them,
// so no in-tree compile-time quotient rests on its declaration alone. The one
// exception is deliberately reducible and is named only by the rejection test.
// ---------------------------------------------------------------------------

// The GF(9) carrier is the harness's own `Gf9`, so the compile-time tower
// declaration below and the harness's runtime `quotient_gf81()` extend one
// base type rather than two spellings of the same field.
#[cfg(test)]
use crate::field::axiom_tests::{Gf9, Gf9Config};
#[cfg(test)]
use crate::gfp::Fp;
#[cfg(test)]
use crate::gfpn::QuadraticExt;

/// GF(2⁴) as GF(2)[x]/(x⁴ + x + 1), a non-binomial modulus: x⁴ − β is
/// reducible over GF(2) for both values of β, so no binomial tower presents
/// this field.
#[cfg(test)]
pub(crate) struct Gf16QuotientConfig;

#[cfg(test)]
impl ConstQuotientConfig<4> for Gf16QuotientConfig {
    type BaseField = Fp<2>;
    const MODULUS: [Fp<2>; 4] = [Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(0)];
}

/// GF(5³) as GF(5)[x]/(x³ + x + 1); the polynomial has no root in GF(5), and a
/// cubic without roots is irreducible.
#[cfg(test)]
pub(crate) struct Gf125QuotientConfig;

#[cfg(test)]
impl ConstQuotientConfig<3> for Gf125QuotientConfig {
    type BaseField = Fp<5>;
    const MODULUS: [Fp<5>; 3] = [Fp::new(1), Fp::new(1), Fp::new(0)];
}

/// GF(3⁴) as GF(9)[y]/(y² − (1 + u)), the tower-base case: the declared
/// coefficients live in a non-prime base, and −(1 + u) = 2 + 2u over GF(3).
#[cfg(test)]
pub(crate) struct Gf81QuotientConfig;

#[cfg(test)]
impl ConstQuotientConfig<2> for Gf81QuotientConfig {
    type BaseField = Gf9;
    const MODULUS: [Gf9; 2] = [
        QuadraticExt::new(Fp::<3>::new(2), Fp::<3>::new(2)),
        QuadraticExt::new(Fp::<3>::new(0), Fp::<3>::new(0)),
    ];
}

/// A deliberately reducible declaration: x⁴ + 1 = (x + 1)⁴ over GF(2). It
/// satisfies every trait bound and is not a field, which is exactly what the
/// validating path exists to catch; the conformance harness never registers it.
#[cfg(test)]
pub(crate) struct ReducibleGf16QuotientConfig;

#[cfg(test)]
impl ConstQuotientConfig<4> for ReducibleGf16QuotientConfig {
    type BaseField = Fp<2>;
    const MODULUS: [Fp<2>; 4] = [Fp::new(1), Fp::new(0), Fp::new(0), Fp::new(0)];
}

#[cfg(test)]
pub(crate) type ConstGf16 = ConstQuotient<4, Gf16QuotientConfig>;

#[cfg(test)]
pub(crate) type ConstGf125 = ConstQuotient<3, Gf125QuotientConfig>;

#[cfg(test)]
pub(crate) type ConstGf81 = ConstQuotient<2, Gf81QuotientConfig>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::axiom_tests::{fp_strategy, quadratic_strategy};
    use crate::field::extension::{
        canonical_generator, convert_element, convert_into_const, minimal_polynomial, Basis,
        CertificateBasis, ExtensionCertificate, FieldError, FieldExtension, FieldId, FieldIdentity,
        ModulusId,
    };
    use crate::field::irreducibility::prove_irreducible;
    use crate::field::{ConstField, FieldPoly, FiniteField};
    use crate::gfp::Fp;
    use crate::gfpn::{ExtConfig, QuadraticExt};
    use proptest::prelude::*;
    use proptest::test_runner::{Config as ProptestConfig, TestRunner};

    /// Per-property case budget for the differential suite. Lower than the
    /// axiom harness's budget because every case drives a relative Frobenius,
    /// an inversion, and a minimal-polynomial orbit in both carriers.
    const DIFFERENTIAL_CASES: u32 = 64;

    fn gf16_modulus() -> FieldPoly<Fp<2>> {
        FieldPoly::new(vec![
            Fp::new(1),
            Fp::new(1),
            Fp::new(0),
            Fp::new(0),
            Fp::new(1),
        ])
    }

    fn gf16() -> QuotientField<Fp<2>> {
        QuotientField::new(Fp::<2>::zero(), gf16_modulus()).unwrap()
    }

    #[test]
    fn validating_construction_rejects_reducible_modulus() {
        let reducible = FieldPoly::new(vec![
            Fp::<2>::new(1),
            Fp::<2>::new(0),
            Fp::<2>::new(0),
            Fp::<2>::new(0),
            Fp::<2>::new(1),
        ]);
        assert!(matches!(
            QuotientField::new(Fp::<2>::zero(), reducible),
            Err(FieldError::ReducibleModulus { .. })
        ));
    }

    #[test]
    fn validating_construction_rejects_degenerate_and_non_monic_moduli() {
        assert_eq!(
            QuotientField::new(Fp::<5>::zero(), FieldPoly::constant(Fp::<5>::new(1))),
            Err(FieldError::ModulusDegreeTooSmall { degree: 0 })
        );
        assert_eq!(
            QuotientField::new(
                Fp::<5>::zero(),
                FieldPoly::new(vec![Fp::<5>::new(1), Fp::<5>::new(2)])
            ),
            Err(FieldError::NonMonicModulus)
        );
    }

    #[test]
    fn certificate_construction_and_unchecked_reuse_round_trip() {
        let base = Fp::<2>::zero();
        let modulus = gf16_modulus();
        let irreducibility = prove_irreducible(&modulus, &base).unwrap();
        let decided = QuotientField::from_certificate(base, irreducibility).unwrap();
        assert_eq!(decided.certificate().basis(), CertificateBasis::Proved);

        let reused =
            QuotientField::from_certificate_unchecked(base, modulus, decided.certificate().clone())
                .unwrap();
        assert_eq!(reused, decided);
    }

    #[test]
    fn unchecked_reuse_rejects_a_certificate_for_another_modulus() {
        let decided = gf16();
        let other_modulus = FieldPoly::new(vec![
            Fp::<2>::new(1),
            Fp::<2>::new(0),
            Fp::<2>::new(0),
            Fp::<2>::new(1),
            Fp::<2>::new(1),
        ]);
        assert!(matches!(
            QuotientField::from_certificate_unchecked(
                Fp::<2>::zero(),
                other_modulus,
                decided.certificate().clone(),
            ),
            Err(FieldError::IdentityMismatch { .. })
        ));
    }

    struct Gf49Config;

    impl ExtConfig for Gf49Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    type Gf49 = QuadraticExt<Gf49Config>;

    #[test]
    fn arithmetic_and_identity_agree_with_quadratic_tower() {
        let base = Fp::<7>::zero();
        let modulus = FieldPoly::new(vec![Fp::new(4), Fp::new(0), Fp::new(1)]);
        let quotient = QuotientField::new(base, modulus).unwrap();
        let a = quotient.element(vec![Fp::new(2), Fp::new(5)]).unwrap();
        let b = quotient.element(vec![Fp::new(6), Fp::new(3)]).unwrap();
        let tower_a = Gf49::new(Fp::new(2), Fp::new(5));
        let tower_b = Gf49::new(Fp::new(6), Fp::new(3));

        assert_eq!(a.field_id(), tower_a.field_id());
        assert_eq!(
            (a.clone() + b.clone()).coefficients(),
            &[Fp::new(1), Fp::new(1)]
        );
        let product = a * b;
        let tower_product = tower_a * tower_b;
        assert_eq!(
            product.coefficients(),
            &[tower_product.c0(), tower_product.c1()]
        );
    }

    #[test]
    fn frobenius_has_absolute_and_relative_orders() {
        let field = gf16();
        let x = field.indeterminate();
        assert_eq!(x.frobenius(4), x);
        assert_eq!(field.relative_frobenius(&x, 4), x);
        assert_ne!(field.relative_frobenius(&x, 1), x);
    }

    #[test]
    fn embedding_membership_and_restriction_round_trip() {
        let field = gf16();
        for value in 0..2 {
            let base = Fp::<2>::new(value);
            let embedded = field.embed(&base);
            assert!(field.contains(&embedded));
            assert_eq!(field.restrict(&embedded), Ok(base));
        }
        let x = field.indeterminate();
        assert!(!field.contains(&x));
        assert_eq!(field.restrict(&x), Err(FieldError::NotInBase));
    }

    #[test]
    fn oversize_order_and_materialization_report_typed_error() {
        let base = Fp::<2>::zero();
        let degree = 128usize;
        let mut coefficients = vec![Fp::<2>::zero(); degree + 1];
        coefficients[0] = Fp::new(1);
        coefficients[1] = Fp::new(1);
        coefficients[degree] = Fp::new(1);
        let modulus = FieldPoly::new(coefficients);
        let base_id = base.field_id();
        let modulus_id = ModulusId::new(
            &base_id,
            modulus
                .iter()
                .map(|coefficient| coefficient.value())
                .collect(),
        )
        .unwrap();
        let ext_id = FieldId::quotient(base_id.clone(), modulus_id, Basis::Polynomial).unwrap();
        let certificate =
            ExtensionCertificate::from_parts(base_id, ext_id, CertificateBasis::Declared).unwrap();
        let field = QuotientField::from_certificate_unchecked(base, modulus, certificate).unwrap();
        let expected = FieldError::UnsupportedSize {
            degree,
            characteristic: 2,
        };

        assert_eq!(field.order(), Err(expected.clone()));
        assert_eq!(field.elements(), Err(expected));
    }

    #[test]
    fn usize_limited_materialization_reports_typed_error() {
        let base = Fp::<2>::zero();
        let degree = usize::BITS as usize;
        let mut coefficients = vec![Fp::<2>::zero(); degree + 1];
        coefficients[0] = Fp::new(1);
        coefficients[1] = Fp::new(1);
        coefficients[degree] = Fp::new(1);
        let modulus = FieldPoly::new(coefficients);
        let base_id = base.field_id();
        let modulus_id = ModulusId::new(
            &base_id,
            modulus
                .iter()
                .map(|coefficient| coefficient.value())
                .collect(),
        )
        .unwrap();
        let ext_id = FieldId::quotient(base_id.clone(), modulus_id, Basis::Polynomial).unwrap();
        let certificate =
            ExtensionCertificate::from_parts(base_id, ext_id, CertificateBasis::Declared).unwrap();
        let field = QuotientField::from_certificate_unchecked(base, modulus, certificate).unwrap();

        if usize::BITS < u128::BITS {
            assert!(field.order().is_ok(), "the order still fits u128");
        }
        assert_eq!(
            field.elements(),
            Err(FieldError::UnsupportedSize {
                degree,
                characteristic: 2,
            })
        );
    }

    #[test]
    fn identity_is_structural_across_instances_and_presentations() {
        let first = gf16();
        let second = gf16();
        let other_modulus = FieldPoly::new(vec![
            Fp::<2>::new(1),
            Fp::<2>::new(0),
            Fp::<2>::new(0),
            Fp::<2>::new(1),
            Fp::<2>::new(1),
        ]);
        let other = QuotientField::new(Fp::<2>::zero(), other_modulus).unwrap();
        let odd = QuotientField::new(
            Fp::<5>::zero(),
            FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(1)]),
        )
        .unwrap();

        assert_eq!(first.ext_id(), second.ext_id());
        assert_ne!(first.ext_id(), other.ext_id());
        assert_ne!(first.ext_id(), odd.ext_id());

        let first_element = first.element(vec![Fp::new(1), Fp::new(1)]).unwrap();
        let second_element = second.element(vec![Fp::new(0), Fp::new(1)]).unwrap();
        assert_eq!(
            first_element.clone() + second_element,
            first.element(vec![Fp::new(1)]).unwrap()
        );
    }

    proptest! {
        #[test]
        fn sampled_nonzero_elements_have_multiplicative_inverses(index in 1u64..16) {
            let field = gf16();
            let element = field
                .element((0..4).map(|bit| Fp::new((index >> bit) & 1)).collect())
                .unwrap();
            let inverse = element.inv().expect("a non-zero field element is invertible");
            prop_assert!((element * inverse).is_one());
        }
    }

    // -----------------------------------------------------------------------
    // Compile-time form: differential equivalence with the runtime form
    // -----------------------------------------------------------------------

    /// Checks the three obligations that make the compile-time and runtime
    /// carriers of one declaration the same field.
    ///
    /// 1. Identity agreement: the compile-time hint is the runtime identity.
    /// 2. Coordinate agreement: corresponding elements write equal canonical
    ///    coordinates.
    /// 3. Observable equivalence: `convert_element` commutes with addition,
    ///    subtraction, multiplication, inversion, `pow`, the relative
    ///    Frobenius, `minimal_polynomial`, and `canonical_generator`.
    fn assert_forms_agree<const R: usize, C>(base: BoxedStrategy<C::BaseField>)
    where
        C: ConstQuotientConfig<R> + 'static,
        C::BaseField: 'static,
    {
        let runtime =
            ConstQuotient::<R, C>::runtime_field().expect("the declaration is irreducible");
        let extension = ConstQuotient::<R, C>::extension().expect("the declaration is irreducible");

        // Obligation 1: identity agreement.
        assert_eq!(
            <ConstQuotient<R, C> as FieldIdentity>::field_id_hint(),
            Some(runtime.ext_id().clone()),
            "the compile-time hint must name the runtime presentation"
        );
        assert_eq!(extension.ext_id(), runtime.ext_id());
        assert_eq!(extension.base_id(), runtime.base_id());
        assert_eq!(extension.certificate().basis(), CertificateBasis::Proved);
        assert_eq!(extension.relative_degree(), R);

        let transport = |value: &ConstQuotient<R, C>| {
            convert_element(value, &runtime.ext_zero())
                .expect("both carriers name one algebraic field")
        };

        // Obligation 3 at field level: the deterministic generator agrees.
        let (const_generator, const_order) =
            canonical_generator(&extension).expect("a finite field has a cyclic unit group");
        let (runtime_generator, runtime_order) =
            canonical_generator(&runtime).expect("a finite field has a cyclic unit group");
        assert_eq!(const_order.order(), runtime_order.order());
        assert_eq!(transport(&const_generator), runtime_generator);

        let coefficients = proptest::collection::vec(base, R);
        let strategy = (coefficients.clone(), coefficients, 0u64..64u64, 0u32..8u32);
        TestRunner::new(ProptestConfig::with_cases(DIFFERENTIAL_CASES))
            .run(&strategy, |(left, right, exponent, steps)| {
                let a = ConstQuotient::<R, C>::reduce(&left);
                let b = ConstQuotient::<R, C>::reduce(&right);
                let a_runtime = runtime
                    .element(left)
                    .expect("the strategy draws base-field coefficients");
                let b_runtime = runtime
                    .element(right)
                    .expect("the strategy draws base-field coefficients");

                // Obligation 2: coordinate agreement.
                let mut const_coords = Vec::new();
                let mut runtime_coords = Vec::new();
                a.write_prime_coords(&mut const_coords);
                a_runtime.write_prime_coords(&mut runtime_coords);
                prop_assert_eq!(&const_coords, &runtime_coords);

                // Conversion is the identity on coordinates in both directions.
                prop_assert_eq!(transport(&a), a_runtime.clone());
                prop_assert_eq!(
                    convert_into_const::<_, ConstQuotient<R, C>>(&a_runtime).unwrap(),
                    a
                );

                // Obligation 3 at element level.
                prop_assert_eq!(transport(&(a + b)), a_runtime.clone() + b_runtime.clone());
                prop_assert_eq!(transport(&(a - b)), a_runtime.clone() - b_runtime.clone());
                prop_assert_eq!(transport(&(a * b)), a_runtime.clone() * b_runtime.clone());
                prop_assert_eq!(a.inv().map(|x| transport(&x)), a_runtime.inv());
                prop_assert_eq!(transport(&a.pow(exponent)), a_runtime.pow(exponent));
                prop_assert_eq!(
                    transport(&extension.relative_frobenius(&a, steps)),
                    runtime.relative_frobenius(&a_runtime, steps)
                );
                prop_assert_eq!(
                    minimal_polynomial(&extension, &a).unwrap(),
                    minimal_polynomial(&runtime, &a_runtime).unwrap()
                );
                Ok(())
            })
            .expect("the two carriers of one field are observably equivalent");
    }

    #[test]
    fn const_and_runtime_forms_agree_on_gf16() {
        assert_forms_agree::<4, Gf16QuotientConfig>(fp_strategy::<2>());
    }

    #[test]
    fn const_and_runtime_forms_agree_on_gf125() {
        assert_forms_agree::<3, Gf125QuotientConfig>(fp_strategy::<5>());
    }

    #[test]
    fn const_and_runtime_forms_agree_on_gf81_over_gf9() {
        assert_forms_agree::<2, Gf81QuotientConfig>(quadratic_strategy::<Gf9Config>(
            fp_strategy::<3>(),
        ));
    }

    #[test]
    fn const_validation_rejects_a_reducible_declaration() {
        assert!(matches!(
            ConstQuotient::<4, ReducibleGf16QuotientConfig>::extension(),
            Err(FieldError::ReducibleModulus { .. })
        ));
        assert!(matches!(
            ConstQuotient::<4, ReducibleGf16QuotientConfig>::runtime_field(),
            Err(FieldError::ReducibleModulus { .. })
        ));
    }

    #[test]
    fn const_unchecked_extension_trusts_a_valid_declaration() {
        let trusted = ConstGf16::extension_unchecked();
        let decided = ConstGf16::extension().unwrap();
        assert_eq!(trusted.certificate().basis(), CertificateBasis::Declared);
        assert_eq!(decided.certificate().basis(), CertificateBasis::Proved);
        assert_eq!(trusted.ext_id(), decided.ext_id());
        assert_eq!(trusted.relative_degree(), 4);

        let runtime = ConstGf16::runtime_field().unwrap();
        let element = ConstGf16::new([Fp::new(1), Fp::new(0), Fp::new(1), Fp::new(1)]);
        let transported = convert_element(&element, &runtime.ext_zero()).unwrap();
        assert_eq!(
            convert_element(&transported, &ConstGf16::zero()).unwrap(),
            element
        );
    }

    #[test]
    fn const_conversion_rejects_a_different_presentation() {
        let other = QuotientField::new(
            Fp::<2>::zero(),
            FieldPoly::new(vec![
                Fp::new(1),
                Fp::new(0),
                Fp::new(0),
                Fp::new(1),
                Fp::new(1),
            ]),
        )
        .unwrap();
        let element = ConstGf16::new([Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(0)]);
        assert!(matches!(
            convert_element(&element, &other.ext_zero()),
            Err(FieldError::IdentityMismatch { .. })
        ));
        assert!(matches!(
            convert_into_const::<_, ConstGf16>(&other.indeterminate()),
            Err(FieldError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn const_quotient_modulus_and_indeterminate_match_the_declaration() {
        let runtime = ConstGf16::runtime_field().unwrap();
        assert_eq!(&ConstGf16::modulus(), runtime.modulus());
        assert_eq!(ConstGf16::modulus().degree(), Some(4));
        assert_eq!(
            ConstGf16::indeterminate().coefficients(),
            &[Fp::new(0), Fp::new(1), Fp::new(0), Fp::new(0)]
        );
        // x^4 = x + 1 in this quotient, which is the rewrite the reduction uses.
        assert_eq!(
            ConstGf16::indeterminate().pow(4),
            ConstGf16::new([Fp::new(1), Fp::new(1), Fp::new(0), Fp::new(0)])
        );
    }

    #[test]
    fn const_quotient_over_a_tower_base_embeds_and_restricts() {
        let extension = ConstGf81::extension().unwrap();
        assert_eq!(extension.base_degree(), 2);
        assert_eq!(extension.ext_degree(), 4);
        assert_eq!(extension.relative_degree(), 2);

        let base = Gf9::new(Fp::new(1), Fp::new(2));
        let embedded = extension.embed(&base);
        assert!(extension.contains(&embedded));
        assert_eq!(extension.restrict(&embedded), Ok(base));

        let y = ConstGf81::indeterminate();
        assert!(!extension.contains(&y));
        assert_eq!(extension.restrict(&y), Err(FieldError::NotInBase));
    }

    #[test]
    fn const_quotient_coordinates_round_trip_through_the_runtime_carrier() {
        let runtime = ConstGf125::runtime_field().unwrap();
        for element in runtime.elements().unwrap() {
            let mut coordinates = Vec::new();
            element.write_prime_coords(&mut coordinates);
            let rebuilt = ConstGf125::zero().from_prime_coords(&coordinates).unwrap();
            let mut rebuilt_coordinates = Vec::new();
            rebuilt.write_prime_coords(&mut rebuilt_coordinates);
            assert_eq!(coordinates, rebuilt_coordinates);
            assert_eq!(
                convert_element(&rebuilt, &runtime.ext_zero()).unwrap(),
                element
            );
        }
        assert_eq!(
            ConstGf125::zero().from_prime_coords(&[0, 0]),
            Err(FieldError::CoordinateCountMismatch {
                expected: 3,
                found: 2
            })
        );
    }
    /// Checks that reduction depends only on the class of its input: adding any
    /// multiple of the modulus leaves the stored vector unchanged, and the
    /// stored vector always has exactly `relative_degree()` entries.
    ///
    /// The cofactor is drawn independently of the coefficient vector, so the
    /// property covers inputs both below and above the modulus degree.
    fn assert_reduction_is_invariant<const R: usize, C>(base: BoxedStrategy<C::BaseField>)
    where
        C: ConstQuotientConfig<R> + 'static,
        C::BaseField: 'static,
    {
        /// Case budget for the invariance property. Each case runs two
        /// divisions and one polynomial product, so it affords more cases than
        /// the differential suite.
        const INVARIANCE_CASES: u32 = 96;

        let field = ConstQuotient::<R, C>::runtime_field().expect("the declaration is irreducible");
        let modulus = ConstQuotient::<R, C>::modulus();
        let strategy = (
            proptest::collection::vec(base.clone(), 0..=2 * R),
            proptest::collection::vec(base, 0..=R),
        );
        TestRunner::new(ProptestConfig::with_cases(INVARIANCE_CASES))
            .run(&strategy, |(coefficients, cofactor)| {
                let reduced = field
                    .element(coefficients.clone())
                    .expect("the strategy draws base-field coefficients");
                prop_assert_eq!(reduced.coefficients().len(), field.relative_degree());

                let shifted =
                    &FieldPoly::new(coefficients) + &(&FieldPoly::new(cofactor) * &modulus);
                let shifted_reduced = field
                    .element(shifted.iter().cloned().collect())
                    .expect("polynomial arithmetic stays in the base field");
                prop_assert_eq!(
                    shifted_reduced.coefficients().len(),
                    field.relative_degree()
                );
                prop_assert_eq!(reduced, shifted_reduced);
                Ok(())
            })
            .expect("reduction depends only on the class of its input");
    }

    #[test]
    fn reduction_is_invariant_under_multiples_of_the_modulus() {
        assert_reduction_is_invariant::<4, Gf16QuotientConfig>(fp_strategy::<2>());
        assert_reduction_is_invariant::<3, Gf125QuotientConfig>(fp_strategy::<5>());
        assert_reduction_is_invariant::<2, Gf81QuotientConfig>(quadratic_strategy::<Gf9Config>(
            fp_strategy::<3>(),
        ));
    }

    /// Checks that `elements()` is indexed by the canonical index: the element
    /// at position `i` writes `field_id().degree()` prime coordinates, each
    /// below the characteristic, whose base-`p` value is `i`.
    fn assert_canonical_index_decodes<F: FieldIdentity>(field: &QuotientField<F>) {
        let order = field.order().expect("the field order is representable");
        let elements = field.elements().expect("the field enumerates");
        assert_eq!(u128::try_from(elements.len()), Ok(order));

        let mut seen = std::collections::HashSet::new();
        for (index, element) in elements.iter().enumerate() {
            let identity = element.field_id();
            let characteristic = u128::from(identity.characteristic());
            let mut coordinates = Vec::new();
            element.write_prime_coords(&mut coordinates);
            assert_eq!(coordinates.len(), identity.degree());

            let mut value = 0u128;
            for (position, coordinate) in coordinates.iter().enumerate() {
                let coordinate = u128::from(*coordinate);
                assert!(coordinate < characteristic);
                value += coordinate * characteristic.pow(u32::try_from(position).unwrap());
            }
            assert_eq!(value, u128::try_from(index).unwrap());
            assert!(seen.insert(coordinates));
        }
        assert_eq!(seen.len(), elements.len());
    }

    #[test]
    fn canonical_index_decodes_to_its_prime_coordinates() {
        assert_canonical_index_decodes(&gf16());
        assert_canonical_index_decodes(&ConstGf125::runtime_field().unwrap());
        assert_canonical_index_decodes(&ConstGf81::runtime_field().unwrap());
    }
}
