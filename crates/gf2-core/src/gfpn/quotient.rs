//! Runtime-configured polynomial quotient extension fields.
//!
//! This module is the reference implementation of a polynomial quotient
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
//! base coefficient, with the base coordinate varying fastest. A later
//! compile-time carrier of the same presentation therefore has one observable
//! target: the arithmetic, coordinates, Frobenius, embedding, and identity
//! defined here.
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

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};
use std::sync::Arc;

use crate::field::extension::{
    Basis, ExtensionCertificate, FieldError, FieldExtension, FieldId, FieldIdentity, ModulusId,
};
use crate::field::irreducibility::{prove_irreducible, IrreducibilityCertificate};
use crate::field::{FieldPoly, FiniteField, FiniteFieldExt};

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
        if self.is_zero() {
            return None;
        }
        let zero = &self.params.base_zero;
        let mut old_remainder = self.params.modulus.clone();
        let mut remainder = FieldPoly::new(self.coefficients.clone());
        let mut old_coefficient = FieldPoly::zero_like(zero);
        let mut coefficient = FieldPoly::one_like(zero);

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
        let (_, reduced) = old_coefficient.div_rem(&self.params.modulus);
        Some(self.field().element_from_remainder(&reduced))
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::extension::{
        Basis, CertificateBasis, ExtensionCertificate, FieldError, FieldExtension, FieldId,
        FieldIdentity, ModulusId,
    };
    use crate::field::irreducibility::prove_irreducible;
    use crate::field::{ConstField, FieldPoly, FiniteField};
    use crate::gfp::Fp;
    use crate::gfpn::{ExtConfig, QuadraticExt};
    use proptest::prelude::*;

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
}
