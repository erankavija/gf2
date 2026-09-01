//! Relative field extensions, algebraic field identity, and validation
//! certificates.
//!
//! This module supplies one relation, one identity, and one conversion rule
//! for "the field $B$ sits inside the field $E$", and it subsumes the three
//! unrelated presentations the crate carries otherwise: the binomial towers
//! configured through [`ExtConfig`], the runtime
//! [`Gf2mField_`] parameters, and the absolute Frobenius on
//! [`FiniteFieldExt`].
//!
//! # Canonical prime coordinates
//!
//! Every construct here rests on one definition. Let $F$ be a field of
//! characteristic $p$ with $d = [F : \mathbb{F}_p]$.
//!
//! - If $F = \mathbb{F}_p$ then $d = 1$ and the coordinate vector of $a$ is
//!   $[\hat{a}]$, the canonical representative in $[0, p)$.
//! - If $F = B\lbrack x\rbrack/(f)$ with $r = \deg f \ge 2$, $d_B = [B : \mathbb{F}_p]$,
//!   and $\xi$ the class of $x$, then $d = r\,d_B$, every $a \in F$ is
//!   uniquely $a = \sum_{i<r} a_i \xi^{i}$ with $a_i \in B$, and the
//!   coordinate vector of $a$ is the concatenation of the coordinate vectors
//!   of $a_0, \ldots, a_{r-1}$.
//!
//! Coordinate index $i\,d_B + j$ therefore carries the $j$-th base coordinate
//! of $a_i$: **the base coordinate varies fastest**.
//! [`FieldIdentity::write_prime_coords`] emits exactly this vector and
//! [`FieldIdentity::from_prime_coords`] inverts it.
//!
//! The **canonical index** $\iota(a) = \sum_{k<d} c_k\, p^{k}$ is a bijection
//! $F \to [0, |F|)$. For $\mathrm{GF}(2^m)$ it is the stored integer value of
//! a [`Gf2mElement_`], and for $\mathrm{GF}(p)$ it is [`Fp::value`].
//!
//! # Identity
//!
//! [`FieldId`] is a hash-consed algebraic description of a field
//! *presentation*: two carriers denote the same field exactly when their
//! identities are equal. Equality is presentation equality, not abstract
//! isomorphism — $\mathbb{F}_2\lbrack x\rbrack/(x^4+x+1)$ and
//! $\mathbb{F}_4\lbrack y\rbrack/(y^2+y+\omega)$ are isomorphic and carry different
//! identities. That is deliberate: the coordinates a matrix file stores are
//! basis-dependent, so identity has to pin the basis for a load to be sound.
//! Equal identity implies a canonical isomorphism, which is exactly the map
//! [`convert_element`] performs.
//!
//! [`FieldId`] says *which field*; [`ElementRepr`] says *how the bytes of one
//! element are laid out*. They version independently and neither contains the
//! other.
//!
//! # The extension relation
//!
//! An extension is a **value**, not a type-level relation: for
//! [`Gf2mElement_`] the base and the extension can share a Rust type while
//! denoting different fields, and the embedding needs the runtime field
//! parameters to produce elements at all. [`FieldExtension`] is that witness.
//! Its required surface is small — a certificate, the zero of $B$, the
//! embedding, and the checked restriction — and everything else is derived:
//!
//! ```
//! use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension};
//! use gf2_core::field::FiniteField;
//! use gf2_core::gf2m::Gf2mField;
//! use gf2_core::gfp::Fp;
//!
//! // GF(2) inside GF(2^4) presented by x^4 + x + 1.
//! let field = Gf2mField::new(4, 0b10011);
//! let ext = BinaryPrimeExt::new(field.clone())?;
//!
//! assert_eq!(ext.base_degree(), 1);
//! assert_eq!(ext.ext_degree(), 4);
//! assert_eq!(ext.relative_degree(), 4);
//! assert_eq!(ext.ext_order(), Some(16));
//! assert_eq!(ext.ext_unit_group_order(), Some(15));
//!
//! // The embedding carries GF(2) onto the two Frobenius-fixed elements.
//! assert_eq!(ext.embed(&Fp::<2>::new(1)), field.one());
//! assert!(ext.contains(&field.element(1)));
//! assert!(!ext.contains(&field.element(2)));
//! assert_eq!(ext.try_restrict(&field.element(2)), None);
//!
//! // The relative Frobenius is x -> x^(|B|^k), with k taken modulo r.
//! let a = field.element(0b1011);
//! assert_eq!(ext.relative_frobenius(&a, 1), a.clone() * a.clone());
//! assert_eq!(ext.relative_frobenius(&a, 4), a);
//! # Ok::<(), gf2_core::field::extension::FieldError>(())
//! ```
//!
//! # Certificates
//!
//! Validating a field presentation is expensive, and callers construct over
//! the same presentation again and again. A certificate is the **memo** of
//! that validation: [`ExtensionCertificate`] is `Arc`-backed, carries the two
//! [`FieldId`]s it covers, and is matched by identity comparison
//! ([`ExtensionCertificate::matches`]) rather than by re-deriving anything.
//! [`CertificateBasis`] records which validation the memo stands for.
//!
//! Two constructors validate, and two reuse the memo:
//!
//! - [`BinaryPrimeExt::new`] decides the runtime field's defining polynomial
//!   with [`prove_irreducible`] and records
//!   [`CertificateBasis::Proved`]. Deciding is `O(m³)`.
//! - [`ConstExt::new`] records [`CertificateBasis::Declared`], the basis for
//!   an [`ExtConfig`] non-residue, which the type fixes rather than decides.
//! - [`BinaryPrimeExt::from_certificate_unchecked`] and
//!   [`ConstExt::from_certificate_unchecked`] take a memo the caller already
//!   holds and skip the validation entirely. This is what makes repeated
//!   construction over one presentation cheap.
//!
//! The `_unchecked` suffix carries the contract, as it does elsewhere in
//! Rust: no memory safety is at stake and no `unsafe` is involved, but the
//! caller promises the memo really does stand for the pair the witness names.
//! Both reuse paths check only that the certificate's identities match —
//! a structural comparison linear in the modulus degree, and in particular
//! free of any decision procedure — which catches an honest mix-up; neither
//! re-derives. A memo of
//! something else yields a mathematically invalid witness, the same way wrong
//! parameters do anywhere else. This is the shape
//! `@/inv/caller-trusted-fast-paths` fixes for the whole project: validation
//! catches mistakes and amortizes cost, a path that skips it for performance
//! is a distinct `_unchecked` method with its precondition documented, and a
//! violated precondition is caller error rather than grounds for hardening.
//!
//! [`crate::field::irreducibility`] is the other producer: proving a
//! polynomial irreducible yields an [`IrreducibilityCertificate`](crate::field::irreducibility::IrreducibilityCertificate), and
//! [`IrreducibilityCertificate::extension_certificate`](crate::field::irreducibility::IrreducibilityCertificate::extension_certificate) promotes it to the
//! memo the two reuse constructors take. [`TrivialExt`] has no such
//! constructor because it performs no validation to memoize: $E = B$ holds
//! for any carrier by construction.
//!
//! ```
//! use gf2_core::field::extension::{BinaryPrimeExt, CertificateBasis, FieldExtension};
//! use gf2_core::gf2m::Gf2mField;
//!
//! // Validation happens once.
//! let field = Gf2mField::new(4, 0b10011);
//! let decided = BinaryPrimeExt::new(field.clone())?;
//! assert_eq!(decided.certificate().basis(), CertificateBasis::Proved);
//!
//! // Every later construction over the same presentation reuses the memo.
//! let memo = decided.certificate().clone();
//! assert!(memo.matches(decided.base_id(), decided.ext_id()));
//! let reused = BinaryPrimeExt::from_certificate_unchecked(field, memo)?;
//! assert_eq!(reused, decided);
//! # Ok::<(), gf2_core::field::extension::FieldError>(())
//! ```
//!
//! [`OrderCertificate`] follows the same shape for multiplicative order:
//! factoring $|E^{*}|$ once serves every divisor, through
//! [`OrderCertificate::divisor`].
//!
//! # Errors
//!
//! Every fallible operation in this module reports [`FieldError`], one
//! variant per distinguishable condition. Invalid input never panics; the
//! panics that do exist are documented and fire only when an implementation
//! violates a contract this crate states elsewhere.

use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::field::irreducibility::prove_irreducible;
use crate::field::{ConstField, FieldPoly, FiniteField, FiniteFieldExt};
use crate::gf2m::{Gf2mElement_, Gf2mField_, Gf2mWide, Gf2mWideConfig, UintExt};
use crate::gfp::Fp;
use crate::gfpn::{CubicExt, ExtConfig, QuadraticExt};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Failure conditions of the algebra layer: identity construction, coordinate
/// conversion, extension validation, and identity encoding.
///
/// One variant per distinguishable condition, so callers branch on structure
/// rather than on message text.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldError {
    /// A base and an extension were paired across different characteristics.
    CharacteristicMismatch {
        /// Characteristic of the proposed base field.
        base: u64,
        /// Characteristic of the proposed extension field.
        ext: u64,
    },
    /// A characteristic outside the representable range $1 < p \le 2^{63}$.
    ///
    /// Mirrors the const assertions of [`Fp`]; primality of $p$ stays the
    /// caller's contract, exactly as [`Fp`] documents.
    CharacteristicOutOfRange {
        /// The rejected characteristic.
        characteristic: u64,
    },
    /// $[B : \mathbb{F}_p]$ does not divide $[E : \mathbb{F}_p]$, so no
    /// relative degree exists.
    DegreeNotDivisible {
        /// Absolute degree of the base field.
        base_degree: usize,
        /// Absolute degree of the extension field.
        ext_degree: usize,
    },
    /// The extension does not reach the base by following its base chain, so
    /// the pair has no tower presentation.
    NotATower {
        /// Identity of the proposed base field.
        base: FieldId,
        /// Identity of the proposed extension field.
        ext: FieldId,
    },
    /// Two carriers that had to denote the same field do not.
    IdentityMismatch {
        /// Identity the operation required.
        expected: FieldId,
        /// Identity the operand actually carries.
        found: FieldId,
    },
    /// The element lies outside the base field, so it has no restriction.
    NotInBase,
    /// The leading coefficient of a modulus is not the one of its base field.
    NonMonicModulus,
    /// A modulus of degree zero, which presents no quotient at all.
    ModulusDegreeTooSmall {
        /// The rejected degree.
        degree: usize,
    },
    /// A coordinate vector of the wrong length for the field it addresses.
    CoordinateCountMismatch {
        /// Number of coordinates the field requires.
        expected: usize,
        /// Number of coordinates supplied.
        found: usize,
    },
    /// A coordinate outside the canonical range $[0, p)$.
    CoordinateOutOfRange {
        /// Position of the offending coordinate in the vector.
        index: usize,
        /// The offending value.
        value: u64,
        /// The characteristic that bounds it.
        characteristic: u64,
    },
    /// A modulus that was required to be irreducible has a proper factor.
    ReducibleModulus {
        /// The shape of factor the irreducibility decision found.
        witness: FactorWitness,
    },
    /// No element of the requested multiplicative order exists, because the
    /// order does not divide the order of the unit group.
    NoElementOfOrder {
        /// The requested multiplicative order.
        requested: u64,
        /// $|E^{*}|$, which the request must divide.
        unit_group_order: u128,
    },
    /// The factorization a deterministic order search needs is unavailable
    /// for this order.
    OrderFactorizationUnavailable {
        /// The order whose factorization is out of reach.
        order: u128,
    },
    /// A field too large to materialize in the requested representation.
    UnsupportedSize {
        /// Absolute degree of the field.
        degree: usize,
        /// Characteristic of the field.
        characteristic: u64,
    },
    /// An encoded identity carrying an unknown version byte.
    EncodingVersionUnsupported {
        /// The version byte found in the stream.
        found: u8,
    },
    /// An encoded identity that is truncated, over-long, or carries an
    /// unknown tag.
    MalformedEncoding,
    /// A cyclotomic modulus must be nonzero.
    InvalidCyclotomicModulus {
        /// The rejected modulus.
        modulus: u64,
    },
    /// The multiplier and modulus are not coprime, so multiplication by the
    /// multiplier is not a permutation of the residue classes.
    NonCoprimeCyclotomicParameters {
        /// The normalized multiplier modulo `modulus`.
        q_mod_n: u64,
        /// The cyclotomic modulus.
        modulus: u64,
        /// The greatest common divisor of `q_mod_n` and `modulus`.
        gcd: u64,
    },
    /// The requested cyclotomic modulus cannot be represented as an in-memory
    /// partition on this target.
    CyclotomicModulusTooLarge {
        /// The rejected modulus.
        modulus: u64,
    },
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CharacteristicMismatch { base, ext } => write!(
                f,
                "base characteristic {base} differs from extension characteristic {ext}"
            ),
            Self::CharacteristicOutOfRange { characteristic } => write!(
                f,
                "characteristic {characteristic} is outside the range 1 < p <= 2^63"
            ),
            Self::DegreeNotDivisible {
                base_degree,
                ext_degree,
            } => write!(
                f,
                "base degree {base_degree} does not divide extension degree {ext_degree}"
            ),
            Self::NotATower { base, ext } => write!(
                f,
                "a degree-{} extension is not presented as a tower over its degree-{} base",
                ext.degree(),
                base.degree()
            ),
            Self::IdentityMismatch { expected, found } => write!(
                f,
                "expected a field of degree {} over characteristic {}, found degree {} over characteristic {}",
                expected.degree(),
                expected.characteristic(),
                found.degree(),
                found.characteristic()
            ),
            Self::NotInBase => write!(f, "the element does not lie in the base field"),
            Self::NonMonicModulus => write!(f, "the modulus is not monic"),
            Self::ModulusDegreeTooSmall { degree } => {
                write!(f, "modulus degree {degree} is below the minimum of one")
            }
            Self::CoordinateCountMismatch { expected, found } => write!(
                f,
                "expected {expected} canonical coordinates, found {found}"
            ),
            Self::CoordinateOutOfRange {
                index,
                value,
                characteristic,
            } => write!(
                f,
                "coordinate {index} is {value}, outside the range [0, {characteristic})"
            ),
            Self::ReducibleModulus { witness } => {
                write!(f, "the modulus is reducible: {witness}")
            }
            Self::NoElementOfOrder {
                requested,
                unit_group_order,
            } => write!(
                f,
                "no element of order {requested} exists in a unit group of order {unit_group_order}"
            ),
            Self::OrderFactorizationUnavailable { order } => {
                write!(f, "the factorization of {order} is unavailable")
            }
            Self::UnsupportedSize {
                degree,
                characteristic,
            } => write!(
                f,
                "a field of degree {degree} over characteristic {characteristic} is too large to materialize"
            ),
            Self::EncodingVersionUnsupported { found } => {
                write!(f, "unsupported field identity encoding version {found}")
            }
            Self::MalformedEncoding => write!(f, "malformed field identity encoding"),
            Self::InvalidCyclotomicModulus { modulus } => {
                write!(f, "cyclotomic modulus {modulus} must be nonzero")
            }
            Self::NonCoprimeCyclotomicParameters {
                q_mod_n,
                modulus,
                gcd,
            } => write!(
                f,
                "cyclotomic multiplier {q_mod_n} and modulus {modulus} have gcd {gcd}, expected 1"
            ),
            Self::CyclotomicModulusTooLarge { modulus } => write!(
                f,
                "cyclotomic modulus {modulus} cannot be represented on this target"
            ),
        }
    }
}

impl std::error::Error for FieldError {}

/// Which shape of factor an irreducibility decision found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FactorWitness {
    /// The polynomial has a root in the base field.
    BaseFieldRoot,
    /// The distinct-degree step split off a factor of this degree.
    DistinctDegreeSplit {
        /// Degree of the split-off factor.
        degree: usize,
    },
    /// The greatest-common-divisor step produced a proper factor.
    ProperFactor {
        /// Degree of the proper factor.
        degree: usize,
    },
}

impl fmt::Display for FactorWitness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseFieldRoot => write!(f, "it has a root in the base field"),
            Self::DistinctDegreeSplit { degree } => {
                write!(
                    f,
                    "distinct-degree splitting found a factor of degree {degree}"
                )
            }
            Self::ProperFactor { degree } => {
                write!(f, "a proper factor of degree {degree} was found")
            }
        }
    }
}

/// Adjusts a coordinate index reported by a sub-slice conversion so that it
/// names a position in the whole coordinate vector.
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

// ---------------------------------------------------------------------------
// Algebraic field identity
// ---------------------------------------------------------------------------

/// Which $\mathbb{F}_p$-basis the coordinates of a quotient field name.
///
/// Identity has to pin basis semantics for a stored coordinate vector to be
/// interpretable, so a future normal-basis carrier cannot silently share an
/// identity with a polynomial-basis one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Basis {
    /// Powers of the class of `x`, base coordinate varying fastest.
    Polynomial,
}

/// A monic modulus over a named base field, in canonical coordinates.
///
/// The coefficients are stored flattened: coefficient $i$ occupies
/// `base_degree` consecutive coordinates, and a modulus of degree $r$ stores
/// all $r + 1$ coefficients including the leading one, so decoding is
/// self-delimiting given the base.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModulusId {
    coeffs: Arc<[u64]>,
    degree: usize,
    base_degree: usize,
}

impl ModulusId {
    /// Validates length, monicity, and coordinate range against `base`.
    ///
    /// `coeffs` is the flattened coefficient vector $c_0, \ldots, c_r$, each
    /// coefficient occupying `base.degree()` canonical coordinates.
    ///
    /// # Errors
    ///
    /// - [`FieldError::CoordinateCountMismatch`] when the length is not a
    ///   multiple of the base degree.
    /// - [`FieldError::ModulusDegreeTooSmall`] when fewer than two
    ///   coefficients are supplied, so the modulus has degree zero.
    /// - [`FieldError::CoordinateOutOfRange`] when a coordinate is at or
    ///   above the characteristic.
    /// - [`FieldError::NonMonicModulus`] when the leading coefficient is not
    ///   the one of the base field.
    ///
    /// # Complexity
    ///
    /// Linear in the number of coordinates.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::{FieldId, ModulusId};
    ///
    /// // x^4 + x + 1 over GF(2), constant term first.
    /// let base = FieldId::prime(2)?;
    /// let modulus = ModulusId::new(&base, vec![1, 1, 0, 0, 1])?;
    /// assert_eq!(modulus.degree(), 4);
    /// assert_eq!(modulus.coefficient(1), &[1]);
    /// # Ok::<(), gf2_core::field::extension::FieldError>(())
    /// ```
    pub fn new(base: &FieldId, coeffs: Vec<u64>) -> Result<Self, FieldError> {
        let base_degree = base.degree();
        let characteristic = base.characteristic();

        if !coeffs.len().is_multiple_of(base_degree) {
            return Err(FieldError::CoordinateCountMismatch {
                expected: base_degree * coeffs.len().div_ceil(base_degree),
                found: coeffs.len(),
            });
        }
        let coefficient_count = coeffs.len() / base_degree;
        if coefficient_count < 2 {
            return Err(FieldError::ModulusDegreeTooSmall {
                degree: coefficient_count.saturating_sub(1),
            });
        }
        let degree = coefficient_count - 1;

        for (index, &value) in coeffs.iter().enumerate() {
            if value >= characteristic {
                return Err(FieldError::CoordinateOutOfRange {
                    index,
                    value,
                    characteristic,
                });
            }
        }

        let leading = &coeffs[degree * base_degree..];
        if leading[0] != 1 || leading[1..].iter().any(|&c| c != 0) {
            return Err(FieldError::NonMonicModulus);
        }

        Ok(Self {
            coeffs: coeffs.into(),
            degree,
            base_degree,
        })
    }

    /// Returns the degree $r$ of the modulus.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Returns the canonical coordinates of coefficient `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i` exceeds [`degree`](Self::degree).
    pub fn coefficient(&self, i: usize) -> &[u64] {
        &self.coeffs[i * self.base_degree..(i + 1) * self.base_degree]
    }

    /// Returns the flattened coordinates of every coefficient, lowest first.
    pub fn coefficients(&self) -> &[u64] {
        &self.coeffs
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum FieldIdRepr {
    Prime {
        characteristic: u64,
    },
    Quotient {
        base: FieldId,
        modulus: ModulusId,
        basis: Basis,
    },
}

/// Canonical algebraic identity of a finite field presentation.
///
/// Two carriers denote the same field exactly when their `FieldId`s are
/// equal. Cloning is an `Arc` bump; equality and hashing are structural.
///
/// Three normalization rules make the representation canonical, so that
/// structural equality is the identity predicate:
///
/// 1. **Degree-one quotients collapse.** [`FieldId::quotient`] with
///    $\deg f = 1$ returns the base identity, because $B\lbrack x\rbrack/(x - c)$ has the
///    same $\mathbb{F}_p$-basis and the same coordinates as $B$. This is what
///    keeps `Gf2mField::new(1, 0b11)` from aliasing $\mathrm{GF}(2)$ under a
///    second name.
/// 2. **Moduli are monic and reduced.** [`ModulusId::new`] rejects a
///    non-monic leading coefficient and any coordinate outside $[0, p)$.
/// 3. **Degrees are minimal by construction.** A modulus of degree
///    $r \ge 2$ stores all $r + 1$ coefficients.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{Basis, FieldId, ModulusId};
///
/// let gf2 = FieldId::prime(2)?;
/// let modulus = ModulusId::new(&gf2, vec![1, 1, 0, 0, 1])?; // x^4 + x + 1
/// let gf16 = FieldId::quotient(gf2.clone(), modulus, Basis::Polynomial)?;
///
/// assert_eq!(gf16.degree(), 4);
/// assert_eq!(gf16.order(), Some(16));
/// assert_eq!(gf16.base(), Some(&gf2));
/// assert!(gf16.is_tower_over(&gf2));
///
/// // A degree-one quotient is the base field itself.
/// let collapsed = FieldId::quotient(
///     gf2.clone(),
///     ModulusId::new(&gf2, vec![1, 1])?,
///     Basis::Polynomial,
/// )?;
/// assert_eq!(collapsed, gf2);
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FieldId(Arc<FieldIdRepr>);

impl FieldId {
    /// Returns the identity of the prime field $\mathbb{F}_p$.
    ///
    /// # Errors
    ///
    /// [`FieldError::CharacteristicOutOfRange`] unless
    /// $1 < p \le 2^{63}$. Primality of $p$ stays the caller's contract.
    pub fn prime(characteristic: u64) -> Result<Self, FieldError> {
        if characteristic <= 1 || characteristic > (1u64 << 63) {
            return Err(FieldError::CharacteristicOutOfRange { characteristic });
        }
        Ok(Self(Arc::new(FieldIdRepr::Prime { characteristic })))
    }

    /// Returns the identity of $B\lbrack x\rbrack/(f)$ for the base `base`, the modulus
    /// `modulus`, and the coordinate basis `basis`.
    ///
    /// A modulus of degree one collapses to `base`, because the quotient has
    /// the same coordinates as its base.
    ///
    /// # Errors
    ///
    /// - [`FieldError::CoordinateCountMismatch`] when the modulus was built
    ///   over a base of a different degree.
    /// - [`FieldError::CoordinateOutOfRange`] when a modulus coordinate is at
    ///   or above the characteristic of `base`, which happens when the
    ///   modulus was built over a base of equal degree but different
    ///   characteristic.
    pub fn quotient(base: FieldId, modulus: ModulusId, basis: Basis) -> Result<Self, FieldError> {
        let base_degree = base.degree();
        if modulus.base_degree != base_degree {
            return Err(FieldError::CoordinateCountMismatch {
                expected: base_degree,
                found: modulus.base_degree,
            });
        }
        let characteristic = base.characteristic();
        for (index, &value) in modulus.coefficients().iter().enumerate() {
            if value >= characteristic {
                return Err(FieldError::CoordinateOutOfRange {
                    index,
                    value,
                    characteristic,
                });
            }
        }
        if modulus.degree == 1 {
            return Ok(base);
        }
        Ok(Self(Arc::new(FieldIdRepr::Quotient {
            base,
            modulus,
            basis,
        })))
    }

    /// Returns the characteristic $p$.
    pub fn characteristic(&self) -> u64 {
        let mut node = self;
        loop {
            match &*node.0 {
                FieldIdRepr::Prime { characteristic } => return *characteristic,
                FieldIdRepr::Quotient { base, .. } => node = base,
            }
        }
    }

    /// Returns the absolute degree $[F : \mathbb{F}_p]$.
    pub fn degree(&self) -> usize {
        let mut degree = 1usize;
        let mut node = self;
        loop {
            match &*node.0 {
                FieldIdRepr::Prime { .. } => return degree,
                FieldIdRepr::Quotient { base, modulus, .. } => {
                    degree *= modulus.degree;
                    node = base;
                }
            }
        }
    }

    /// Returns $|F|$, or `None` when it exceeds `u128::MAX`.
    pub fn order(&self) -> Option<u128> {
        let degree = u32::try_from(self.degree()).ok()?;
        u128::from(self.characteristic()).checked_pow(degree)
    }

    /// Returns $|F^{*}| = |F| - 1$, or `None` when it exceeds `u128::MAX`.
    pub fn unit_group_order(&self) -> Option<u128> {
        self.order().map(|order| order - 1)
    }

    /// Returns the base of this presentation, or `None` for a prime field.
    pub fn base(&self) -> Option<&FieldId> {
        match &*self.0 {
            FieldIdRepr::Prime { .. } => None,
            FieldIdRepr::Quotient { base, .. } => Some(base),
        }
    }

    /// Returns the modulus of this presentation, or `None` for a prime field.
    pub fn modulus(&self) -> Option<&ModulusId> {
        match &*self.0 {
            FieldIdRepr::Prime { .. } => None,
            FieldIdRepr::Quotient { modulus, .. } => Some(modulus),
        }
    }

    /// Returns the coordinate basis of this presentation, or `None` for a
    /// prime field, whose single coordinate needs no basis choice.
    pub fn basis(&self) -> Option<Basis> {
        match &*self.0 {
            FieldIdRepr::Prime { .. } => None,
            FieldIdRepr::Quotient { basis, .. } => Some(*basis),
        }
    }

    /// Returns `true` when `self` reaches `base` by following
    /// [`base`](Self::base) zero or more times.
    pub fn is_tower_over(&self, base: &FieldId) -> bool {
        let mut node = self;
        loop {
            if node == base {
                return true;
            }
            match node.base() {
                Some(next) => node = next,
                None => return false,
            }
        }
    }

    /// Returns the bytes per canonical coordinate,
    /// $\lceil \mathrm{bitlen}(p-1)/8 \rceil$.
    pub fn coordinate_width(&self) -> usize {
        let bits = u64::BITS - (self.characteristic() - 1).leading_zeros();
        (bits as usize).div_ceil(8)
    }

    /// Encodes this identity into the self-delimiting wire form.
    ///
    /// The stream is a version byte followed by the root node. A node is a
    /// tag byte and then either a little-endian `u64` characteristic (prime)
    /// or the base node, a basis tag, a little-endian `u32` degree, and
    /// $r + 1$ coefficients (quotient). Each coefficient is $d_B$
    /// coordinates and each coordinate is
    /// [`coordinate_width`](Self::coordinate_width) little-endian bytes. Both
    /// the characteristic and the base degree are known by the time
    /// coefficients are read, so the stream decodes without a length prefix.
    ///
    /// The encoding is hand-written rather than `serde`-derived because the
    /// `io` feature is optional and a `serde` representation is not a
    /// stability contract.
    ///
    /// # Panics
    ///
    /// Panics if a modulus degree exceeds `u32::MAX`. No construction in this
    /// crate can produce one: a modulus of degree $r$ owns $r + 1$ stored
    /// coefficients.
    ///
    /// # Complexity
    ///
    /// Linear in the total number of modulus coordinates in the tower.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::FieldId;
    ///
    /// let gf2 = FieldId::prime(2)?;
    /// assert_eq!(FieldId::decode(&gf2.encode())?, gf2);
    /// # Ok::<(), gf2_core::field::extension::FieldError>(())
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![FIELD_ID_ENCODING_VERSION];
        self.encode_node(&mut out);
        out
    }

    fn encode_node(&self, out: &mut Vec<u8>) {
        match &*self.0 {
            FieldIdRepr::Prime { characteristic } => {
                out.push(FIELD_ID_TAG_PRIME);
                out.extend_from_slice(&characteristic.to_le_bytes());
            }
            FieldIdRepr::Quotient {
                base,
                modulus,
                basis,
            } => {
                out.push(FIELD_ID_TAG_QUOTIENT);
                base.encode_node(out);
                out.push(match basis {
                    Basis::Polynomial => BASIS_TAG_POLYNOMIAL,
                });
                let degree =
                    u32::try_from(modulus.degree).expect("a stored modulus degree fits in u32");
                out.extend_from_slice(&degree.to_le_bytes());
                let width = base.coordinate_width();
                for coordinate in modulus.coefficients() {
                    out.extend_from_slice(&coordinate.to_le_bytes()[..width]);
                }
            }
        }
    }

    /// Decodes an identity produced by [`encode`](Self::encode).
    ///
    /// # Errors
    ///
    /// - [`FieldError::EncodingVersionUnsupported`] for an unknown version
    ///   byte.
    /// - [`FieldError::MalformedEncoding`] for a truncated stream, an unknown
    ///   node or basis tag, trailing bytes after the root node, or a tower
    ///   nested deeper than any encodable identity.
    /// - The construction errors of [`FieldId::prime`], [`ModulusId::new`],
    ///   and [`FieldId::quotient`] for a well-formed stream describing an
    ///   invalid field.
    ///
    /// # Complexity
    ///
    /// Linear in the length of `bytes`.
    pub fn decode(bytes: &[u8]) -> Result<Self, FieldError> {
        let mut cursor = ByteCursor {
            bytes,
            offset: 0usize,
        };
        let version = cursor.take_u8()?;
        if version != FIELD_ID_ENCODING_VERSION {
            return Err(FieldError::EncodingVersionUnsupported { found: version });
        }
        let id = decode_node(&mut cursor, 0)?;
        if cursor.offset != bytes.len() {
            return Err(FieldError::MalformedEncoding);
        }
        Ok(id)
    }
}

/// Version byte of the [`FieldId`] wire encoding.
///
/// Stored alongside every encoded identity so a reader can reject a stream it
/// does not understand instead of misreading it.
pub const FIELD_ID_ENCODING_VERSION: u8 = 1;

const FIELD_ID_TAG_PRIME: u8 = 0x01;
const FIELD_ID_TAG_QUOTIENT: u8 = 0x02;
const BASIS_TAG_POLYNOMIAL: u8 = 0x01;

/// Deepest tower a decoder accepts. A quotient node has degree at least two,
/// so an identity of depth `d` has absolute degree at least `2^d`; no
/// identity this crate can build reaches this bound.
const MAX_DECODE_DEPTH: usize = 64;

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl ByteCursor<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], FieldError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(FieldError::MalformedEncoding)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(FieldError::MalformedEncoding)?;
        self.offset = end;
        Ok(slice)
    }

    fn take_u8(&mut self) -> Result<u8, FieldError> {
        Ok(self.take(1)?[0])
    }

    fn take_u32(&mut self) -> Result<u32, FieldError> {
        let bytes: [u8; 4] = self.take(4)?.try_into().expect("four bytes");
        Ok(u32::from_le_bytes(bytes))
    }

    fn take_u64(&mut self) -> Result<u64, FieldError> {
        let bytes: [u8; 8] = self.take(8)?.try_into().expect("eight bytes");
        Ok(u64::from_le_bytes(bytes))
    }

    fn take_coordinate(&mut self, width: usize) -> Result<u64, FieldError> {
        let mut buffer = [0u8; 8];
        buffer[..width].copy_from_slice(self.take(width)?);
        Ok(u64::from_le_bytes(buffer))
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

fn decode_node(cursor: &mut ByteCursor<'_>, depth: usize) -> Result<FieldId, FieldError> {
    if depth > MAX_DECODE_DEPTH {
        return Err(FieldError::MalformedEncoding);
    }
    match cursor.take_u8()? {
        FIELD_ID_TAG_PRIME => FieldId::prime(cursor.take_u64()?),
        FIELD_ID_TAG_QUOTIENT => {
            let base = decode_node(cursor, depth + 1)?;
            let basis = match cursor.take_u8()? {
                BASIS_TAG_POLYNOMIAL => Basis::Polynomial,
                _ => return Err(FieldError::MalformedEncoding),
            };
            let degree = cursor.take_u32()? as usize;
            let base_degree = base.degree();
            let width = base.coordinate_width();
            let count = degree
                .checked_add(1)
                .and_then(|coefficients| coefficients.checked_mul(base_degree))
                .ok_or(FieldError::MalformedEncoding)?;
            if count > cursor.remaining() / width {
                return Err(FieldError::MalformedEncoding);
            }
            let mut coeffs = Vec::with_capacity(count);
            for _ in 0..count {
                coeffs.push(cursor.take_coordinate(width)?);
            }
            let modulus = ModulusId::new(&base, coeffs)?;
            FieldId::quotient(base, modulus, basis)
        }
        _ => Err(FieldError::MalformedEncoding),
    }
}

// ---------------------------------------------------------------------------
// Element wire representation
// ---------------------------------------------------------------------------

/// Version byte of the [`ElementRepr`] vocabulary.
///
/// [`FieldId`] and [`ElementRepr`] version independently: adding a packing
/// bumps this constant and leaves every stored identity valid.
pub const ELEMENT_REPR_VERSION: u8 = 1;

/// How one field element is packed into bytes.
///
/// This is deliberately not part of [`FieldId`]. Two files with equal
/// identity and different representations hold matrices over the *same*
/// field, and a loader converts packings without touching identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ElementRepr {
    /// Canonical coordinates, each in `coord_width` little-endian bytes, in
    /// canonical order. `coord_width` is
    /// [`FieldId::coordinate_width`] of the field the element belongs to.
    PrimeCoordsLe {
        /// Bytes per canonical coordinate.
        coord_width: u8,
    },
}

// ---------------------------------------------------------------------------
// Identity on the element types
// ---------------------------------------------------------------------------

/// A finite field that knows its algebraic identity and its canonical
/// $\mathbb{F}_p$-coordinates.
///
/// Identity attaches to a *value*, because half the crate's fields carry
/// their parameters at runtime. The shape mirrors the existing static escape
/// hatches [`FiniteField::zero_hint`] and
/// [`FiniteField::cardinality_log2_hint`]: a method for the general case, and
/// a static hint that only context-free fields answer.
///
/// Two laws tie this surface to [`FiniteField`], and the shared conformance
/// harness checks both for every carrier:
///
/// $$
/// x.\mathrm{field\\_id}().\mathrm{degree}() = x.\mathrm{extension\\_degree}(),
/// \qquad
/// x.\mathrm{field\\_id}().\mathrm{characteristic}() = x.\mathrm{characteristic}()
/// $$
pub trait FieldIdentity: FiniteField<Characteristic = u64> {
    /// Returns the identity of the field this element belongs to.
    fn field_id(&self) -> FieldId;

    /// Returns the identity determinable from the type alone.
    ///
    /// [`ConstField`] carriers override this; runtime-context carriers keep
    /// the `None` default. When it answers, it answers with the identity
    /// every element of the type reports.
    fn field_id_hint() -> Option<FieldId> {
        None
    }

    /// Appends this element's canonical coordinates to `out`, which the
    /// implementation clears first.
    ///
    /// Writes exactly `field_id().degree()` values, each in $[0, p)$, with
    /// the base coordinate varying fastest.
    fn write_prime_coords(&self, out: &mut Vec<u64>);

    /// Rebuilds an element of *this element's* field from canonical
    /// coordinates. `self` is the field witness; its value is ignored.
    ///
    /// # Errors
    ///
    /// - [`FieldError::CoordinateCountMismatch`] when `coords` does not have
    ///   `field_id().degree()` entries.
    /// - [`FieldError::CoordinateOutOfRange`] when an entry is at or above
    ///   the characteristic.
    // `self` is the field witness rather than the value being converted, which
    // is what a runtime-context carrier needs to produce an element at all;
    // an associated function could not serve `Gf2mElement_`.
    #[allow(clippy::wrong_self_convention)]
    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError>;
}

/// Builds the identity of $\mathrm{GF}(2)\lbrack x\rbrack/(f)$ from the $m + 1$
/// coefficient bits of $f$, lowest first.
fn binary_field_id(coefficients: Vec<u64>) -> Result<FieldId, FieldError> {
    let base = FieldId::prime(2)?;
    binary_field_id_from_modulus(ModulusId::new(&base, coefficients)?)
}

/// Builds the identity of $\mathrm{GF}(2)\lbrack x\rbrack/(f)$ from an already
/// validated modulus.
fn binary_field_id_from_modulus(modulus: ModulusId) -> Result<FieldId, FieldError> {
    FieldId::quotient(FieldId::prime(2)?, modulus, Basis::Polynomial)
}

/// Returns the identity of a runtime binary extension field.
///
/// # Errors
///
/// [`FieldError::NonMonicModulus`] when the defining polynomial has no term
/// of degree `field.degree()`, which violates the contract of
/// [`Gf2mField_::new`].
fn gf2m_field_id<V: UintExt>(field: &Gf2mField_<V>) -> Result<FieldId, FieldError> {
    binary_field_id_from_modulus(gf2m_modulus_id(field)?)
}

/// Returns the defining polynomial of a runtime binary extension field in
/// canonical coordinates.
///
/// # Errors
///
/// [`FieldError::NonMonicModulus`] when the polynomial has no term of degree
/// `field.degree()`, which violates the contract of [`Gf2mField_::new`].
fn gf2m_modulus_id<V: UintExt>(field: &Gf2mField_<V>) -> Result<ModulusId, FieldError> {
    let degree = field.degree();
    let polynomial = field.primitive_polynomial();
    let mut coefficients = Vec::with_capacity(degree + 1);
    for i in 0..=degree {
        coefficients.push(u64::from(polynomial.bit(i as u32)));
    }
    ModulusId::new(&FieldId::prime(2)?, coefficients)
}

/// Reads a binary modulus back as a polynomial over $\mathrm{GF}(2)$, the
/// form [`prove_irreducible`] decides.
fn binary_modulus_polynomial(modulus: &ModulusId) -> FieldPoly<Fp<2>> {
    FieldPoly::new(
        modulus
            .coefficients()
            .iter()
            .map(|&coordinate| Fp::<2>::new(coordinate))
            .collect(),
    )
}

/// Returns the identity of a compile-time multi-word binary extension field.
fn gf2m_wide_field_id<const N: usize, Cfg: Gf2mWideConfig<N>>() -> FieldId {
    let mut coefficients = Vec::with_capacity(Cfg::M + 1);
    for i in 0..Cfg::M {
        coefficients.push((Cfg::MODULUS[i >> 6] >> (i & 63)) & 1);
    }
    // The leading coefficient at bit `M` is implicit in `Gf2mWideConfig`.
    coefficients.push(1);
    binary_field_id(coefficients).expect("a Gf2mWideConfig modulus is monic over GF(2)")
}

/// Returns the identity of $B\lbrack x\rbrack/(x^{r} - \beta)$, the binomial presentation
/// the `ExtConfig` towers use.
fn binomial_field_id<B: FieldIdentity>(
    non_residue: &B,
    relative_degree: usize,
) -> Result<FieldId, FieldError> {
    let base = non_residue.field_id();
    let modulus = binomial_modulus_id(&base, non_residue, relative_degree)?;
    FieldId::quotient(base, modulus, Basis::Polynomial)
}

/// Returns the modulus $x^{r} - \beta$ in the canonical coordinates of
/// `base`.
fn binomial_modulus_id<B: FieldIdentity>(
    base: &FieldId,
    non_residue: &B,
    relative_degree: usize,
) -> Result<ModulusId, FieldError> {
    let base_degree = base.degree();
    let mut coeffs = Vec::with_capacity((relative_degree + 1) * base_degree);
    let mut buffer = Vec::new();

    (-non_residue.clone()).write_prime_coords(&mut buffer);
    coeffs.extend_from_slice(&buffer);
    coeffs.resize(relative_degree * base_degree, 0);
    non_residue.one_like().write_prime_coords(&mut buffer);
    coeffs.extend_from_slice(&buffer);

    ModulusId::new(base, coeffs)
}

/// The identity of `Fp<P>` is the prime field itself.
///
/// # Panics
///
/// [`field_id`](FieldIdentity::field_id) panics when `P` is outside
/// $1 < P \le 2^{63}$, the range [`Fp`] asserts at construction.
impl<const P: u64> FieldIdentity for Fp<P> {
    fn field_id(&self) -> FieldId {
        Self::field_id_hint().expect("Fp<P> asserts 1 < P <= 2^63 at construction")
    }

    fn field_id_hint() -> Option<FieldId> {
        FieldId::prime(P).ok()
    }

    fn write_prime_coords(&self, out: &mut Vec<u64>) {
        out.clear();
        out.push(self.value());
    }

    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError> {
        let [value] = coords else {
            return Err(FieldError::CoordinateCountMismatch {
                expected: 1,
                found: coords.len(),
            });
        };
        if *value >= P {
            return Err(FieldError::CoordinateOutOfRange {
                index: 0,
                value: *value,
                characteristic: P,
            });
        }
        Ok(Self::new(*value))
    }
}

/// The identity of a runtime `GF(2^m)` element is the quotient of
/// $\mathrm{GF}(2)$ by the field's defining polynomial. Coordinates are the
/// bits of the stored value, lowest first.
///
/// # Panics
///
/// [`field_id`](FieldIdentity::field_id) panics when the field's defining
/// polynomial has no term of degree `m`, which violates the contract of
/// [`Gf2mField_::new`]. [`BinaryPrimeExt::new`] reports the same condition as
/// a typed error instead.
impl<V: UintExt> FieldIdentity for Gf2mElement_<V> {
    fn field_id(&self) -> FieldId {
        gf2m_field_id(&self.field())
            .expect("Gf2mField_::new takes a defining polynomial of degree m")
    }

    fn write_prime_coords(&self, out: &mut Vec<u64>) {
        out.clear();
        let value = self.value();
        for i in 0..self.extension_degree() {
            out.push(u64::from(value.bit(i as u32)));
        }
    }

    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError> {
        let field = self.field();
        let degree = field.degree();
        if coords.len() != degree {
            return Err(FieldError::CoordinateCountMismatch {
                expected: degree,
                found: coords.len(),
            });
        }
        let mut value = V::ZERO;
        for (index, &coordinate) in coords.iter().enumerate() {
            match coordinate {
                0 => {}
                1 => value = value | (V::ONE << (index as u32)),
                _ => {
                    return Err(FieldError::CoordinateOutOfRange {
                        index,
                        value: coordinate,
                        characteristic: 2,
                    })
                }
            }
        }
        Ok(field.element(value))
    }
}

/// The identity of a multi-word `GF(2^M)` element comes from its
/// configuration, whose leading modulus coefficient is implicit.
impl<const N: usize, Cfg: Gf2mWideConfig<N>> FieldIdentity for Gf2mWide<N, Cfg> {
    fn field_id(&self) -> FieldId {
        gf2m_wide_field_id::<N, Cfg>()
    }

    fn field_id_hint() -> Option<FieldId> {
        Some(gf2m_wide_field_id::<N, Cfg>())
    }

    fn write_prime_coords(&self, out: &mut Vec<u64>) {
        out.clear();
        let words = self.words();
        for i in 0..Cfg::M {
            out.push((words[i >> 6] >> (i & 63)) & 1);
        }
    }

    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError> {
        if coords.len() != Cfg::M {
            return Err(FieldError::CoordinateCountMismatch {
                expected: Cfg::M,
                found: coords.len(),
            });
        }
        let mut words = [0u64; N];
        for (index, &coordinate) in coords.iter().enumerate() {
            match coordinate {
                0 => {}
                1 => words[index >> 6] |= 1u64 << (index & 63),
                _ => {
                    return Err(FieldError::CoordinateOutOfRange {
                        index,
                        value: coordinate,
                        characteristic: 2,
                    })
                }
            }
        }
        Ok(Self::new(words))
    }
}

/// The identity of a quadratic tower element is the quotient of its base by
/// $x^{2} - \beta$, so the [`ExtConfig`] non-residue *is* the modulus and no
/// parallel configuration concept appears.
impl<C: ExtConfig> FieldIdentity for QuadraticExt<C>
where
    C::BaseField: FieldIdentity,
{
    fn field_id(&self) -> FieldId {
        Self::field_id_hint().expect("a binomial tower presents its base as a tower step")
    }

    fn field_id_hint() -> Option<FieldId> {
        binomial_field_id(&non_residue::<C>(), 2).ok()
    }

    fn write_prime_coords(&self, out: &mut Vec<u64>) {
        out.clear();
        let mut buffer = Vec::new();
        self.c0().write_prime_coords(&mut buffer);
        out.extend_from_slice(&buffer);
        self.c1().write_prime_coords(&mut buffer);
        out.extend_from_slice(&buffer);
    }

    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError> {
        let witness = <C::BaseField as ConstField>::zero();
        let base_degree = witness.extension_degree();
        if coords.len() != 2 * base_degree {
            return Err(FieldError::CoordinateCountMismatch {
                expected: 2 * base_degree,
                found: coords.len(),
            });
        }
        let c0 = witness.from_prime_coords(&coords[..base_degree])?;
        let c1 = witness
            .from_prime_coords(&coords[base_degree..])
            .map_err(|error| shift_coordinate_index(error, base_degree))?;
        Ok(Self::new(c0, c1))
    }
}

/// The identity of a cubic tower element is the quotient of its base by
/// $x^{3} - \beta$.
impl<C: ExtConfig> FieldIdentity for CubicExt<C>
where
    C::BaseField: FieldIdentity,
{
    fn field_id(&self) -> FieldId {
        Self::field_id_hint().expect("a binomial tower presents its base as a tower step")
    }

    fn field_id_hint() -> Option<FieldId> {
        binomial_field_id(&non_residue::<C>(), 3).ok()
    }

    fn write_prime_coords(&self, out: &mut Vec<u64>) {
        out.clear();
        let mut buffer = Vec::new();
        self.c0().write_prime_coords(&mut buffer);
        out.extend_from_slice(&buffer);
        self.c1().write_prime_coords(&mut buffer);
        out.extend_from_slice(&buffer);
        self.c2().write_prime_coords(&mut buffer);
        out.extend_from_slice(&buffer);
    }

    fn from_prime_coords(&self, coords: &[u64]) -> Result<Self, FieldError> {
        let witness = <C::BaseField as ConstField>::zero();
        let base_degree = witness.extension_degree();
        if coords.len() != 3 * base_degree {
            return Err(FieldError::CoordinateCountMismatch {
                expected: 3 * base_degree,
                found: coords.len(),
            });
        }
        let c0 = witness.from_prime_coords(&coords[..base_degree])?;
        let c1 = witness
            .from_prime_coords(&coords[base_degree..2 * base_degree])
            .map_err(|error| shift_coordinate_index(error, base_degree))?;
        let c2 = witness
            .from_prime_coords(&coords[2 * base_degree..])
            .map_err(|error| shift_coordinate_index(error, 2 * base_degree))?;
        Ok(Self::new(c0, c1, c2))
    }
}

/// Returns the non-residue $\beta$ of an [`ExtConfig`] as $\beta \cdot 1$, so
/// that the same expression reads the associated constant and the
/// verification-only method form.
fn non_residue<C: ExtConfig>() -> C::BaseField {
    C::mul_by_non_residue(<C::BaseField as ConstField>::one())
}

// ---------------------------------------------------------------------------
// Certificates
// ---------------------------------------------------------------------------

/// What an extension's validity rests on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CertificateBasis {
    /// $E = B$.
    Identity,
    /// The modulus was decided irreducible by a proof procedure.
    Proved,
    /// The modulus comes from the repository's verified polynomial registry.
    Registry,
    /// The modulus is an implementor's declared contract, taken on trust at
    /// the type level. Nothing was checked, and recording that honestly is
    /// the point of the variant.
    Declared,
}

#[derive(Debug, PartialEq, Eq)]
struct ExtensionCertificateRepr {
    base: FieldId,
    ext: FieldId,
    relative_degree: usize,
    basis: CertificateBasis,
}

/// Evidence that $B \subseteq E$ with the recorded structure.
///
/// Certificates are cheap to clone, carry [`FieldId`]s, and decide reuse by
/// comparison rather than by re-derivation: a constructor that holds a
/// certificate asks [`matches`](Self::matches) instead of re-running the
/// validation that produced it.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{
///     Basis, CertificateBasis, ExtensionCertificate, FieldId, ModulusId,
/// };
///
/// let gf2 = FieldId::prime(2)?;
/// let gf16 = FieldId::quotient(
///     gf2.clone(),
///     ModulusId::new(&gf2, vec![1, 1, 0, 0, 1])?,
///     Basis::Polynomial,
/// )?;
///
/// let certificate =
///     ExtensionCertificate::from_parts(gf2.clone(), gf16.clone(), CertificateBasis::Registry)?;
/// assert_eq!(certificate.relative_degree(), 4);
/// assert!(certificate.matches(&gf2, &gf16));
///
/// // Reuse is an identity comparison, never a re-derivation.
/// assert!(!certificate.matches(&gf16, &gf2));
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionCertificate(Arc<ExtensionCertificateRepr>);

impl ExtensionCertificate {
    /// Returns the certificate of the trivial extension $E = B$.
    pub fn trivial(id: FieldId) -> Self {
        Self(Arc::new(ExtensionCertificateRepr {
            base: id.clone(),
            ext: id,
            relative_degree: 1,
            basis: CertificateBasis::Identity,
        }))
    }

    /// Validates that the characteristics agree and that `ext` reaches `base`
    /// through its base chain, then records the pair.
    ///
    /// The `basis` is caller-asserted provenance, recorded as given: this
    /// constructor validates the identity relationship only, never the claim
    /// the basis makes about how the pair was validated. A certificate built
    /// here feeds the `_unchecked` reuse constructors on the caller's own
    /// responsibility (`@/inv/caller-trusted-fast-paths`).
    ///
    /// # Errors
    ///
    /// - [`FieldError::CharacteristicMismatch`] when the two characteristics
    ///   differ.
    /// - [`FieldError::NotATower`] when `ext` does not reach `base` by
    ///   following its base chain.
    /// - [`FieldError::DegreeNotDivisible`] when the base degree does not
    ///   divide the extension degree.
    ///
    /// # Complexity
    ///
    /// Linear in the height of the tower.
    pub fn from_parts(
        base: FieldId,
        ext: FieldId,
        basis: CertificateBasis,
    ) -> Result<Self, FieldError> {
        let base_characteristic = base.characteristic();
        let ext_characteristic = ext.characteristic();
        if base_characteristic != ext_characteristic {
            return Err(FieldError::CharacteristicMismatch {
                base: base_characteristic,
                ext: ext_characteristic,
            });
        }
        if !ext.is_tower_over(&base) {
            return Err(FieldError::NotATower { base, ext });
        }
        let base_degree = base.degree();
        let ext_degree = ext.degree();
        if !ext_degree.is_multiple_of(base_degree) {
            return Err(FieldError::DegreeNotDivisible {
                base_degree,
                ext_degree,
            });
        }
        Ok(Self(Arc::new(ExtensionCertificateRepr {
            relative_degree: ext_degree / base_degree,
            base,
            ext,
            basis,
        })))
    }

    /// Returns the identity of the base field.
    pub fn base_id(&self) -> &FieldId {
        &self.0.base
    }

    /// Returns the identity of the extension field.
    pub fn ext_id(&self) -> &FieldId {
        &self.0.ext
    }

    /// Returns $r = [E : B]$.
    pub fn relative_degree(&self) -> usize {
        self.0.relative_degree
    }

    /// Returns what this certificate's validity rests on.
    pub fn basis(&self) -> CertificateBasis {
        self.0.basis
    }

    /// Returns `true` when this certificate already covers the pair, so a
    /// construction can reuse it instead of revalidating.
    pub fn matches(&self, base: &FieldId, ext: &FieldId) -> bool {
        self.0.base == *base && self.0.ext == *ext
    }
}

#[derive(Debug, PartialEq, Eq)]
struct OrderCertificateRepr {
    field_id: FieldId,
    order: u64,
    prime_factors: Vec<u64>,
}

/// Evidence that an element has exact multiplicative order $n$, carrying the
/// factorization that made the check possible.
///
/// Factoring $|E^{*}|$ is the expensive input to every deterministic order
/// search, so a certificate carries its factorization and
/// [`divisor`](Self::divisor) derives a certificate for any divisor of the
/// recorded order by filtering it. Factoring once therefore serves every
/// $n \mid |E^{*}|$.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{FieldIdentity, OrderCertificate};
/// use gf2_core::gf2m::Gf2mField;
///
/// // |GF(2^4)*| = 15 = 3 * 5.
/// let gf16 = Gf2mField::new(4, 0b10011).zero().field_id();
/// assert_eq!(gf16.unit_group_order(), Some(15));
///
/// let certificate = OrderCertificate::new(gf16, 15, vec![3, 5])?;
/// assert_eq!(certificate.order(), 15);
///
/// let five = certificate.divisor(5).expect("5 divides 15");
/// assert_eq!(five.prime_factors(), &[5]);
/// assert!(certificate.divisor(4).is_none());
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderCertificate(Arc<OrderCertificateRepr>);

impl OrderCertificate {
    /// Records a validated order together with the distinct prime factors
    /// that decided it.
    ///
    /// `prime_factors` must be strictly ascending, and dividing `order` by
    /// them exhaustively must leave one — that completeness is what makes the
    /// certificate reusable through [`divisor`](Self::divisor).
    ///
    /// # Errors
    ///
    /// - [`FieldError::NoElementOfOrder`] when `order` does not divide
    ///   $|E^{*}|$, so no element of that order exists.
    /// - [`FieldError::OrderFactorizationUnavailable`] when `prime_factors`
    ///   is not a strictly ascending, complete factor list for `order`.
    ///
    /// # What this does not check
    ///
    /// Primality of the recorded factors stays the caller's contract, exactly
    /// as it does for [`Fp`]. Ascending order, divisibility, and completeness
    /// together admit a composite entry whose powers exhaust `order`, and
    /// [`divisor`](Self::divisor) would then filter against a factor that is
    /// not prime.
    ///
    /// The certificate also records evidence rather than producing it: it
    /// asserts that a check ran, and neither exhibits an element of `order`
    /// nor verifies that `field_id` names a field whose modulus is
    /// irreducible.
    ///
    /// # Complexity
    ///
    /// `O(k log order)` for `k` recorded factors.
    pub fn new(field_id: FieldId, order: u64, prime_factors: Vec<u64>) -> Result<Self, FieldError> {
        if let Some(unit_group_order) = field_id.unit_group_order() {
            if order == 0 || !unit_group_order.is_multiple_of(u128::from(order)) {
                return Err(FieldError::NoElementOfOrder {
                    requested: order,
                    unit_group_order,
                });
            }
        }
        let unavailable = || FieldError::OrderFactorizationUnavailable {
            order: u128::from(order),
        };
        let mut residue = order;
        let mut previous = 1u64;
        for &factor in &prime_factors {
            if factor <= previous || !residue.is_multiple_of(factor) {
                return Err(unavailable());
            }
            previous = factor;
            while residue.is_multiple_of(factor) {
                residue /= factor;
            }
        }
        if residue != 1 {
            return Err(unavailable());
        }
        Ok(Self(Arc::new(OrderCertificateRepr {
            field_id,
            order,
            prime_factors,
        })))
    }

    /// Returns the exact multiplicative order this certificate records.
    pub fn order(&self) -> u64 {
        self.0.order
    }

    /// Returns the distinct prime factors of [`order`](Self::order),
    /// ascending.
    pub fn prime_factors(&self) -> &[u64] {
        &self.0.prime_factors
    }

    /// Returns the identity of the field the order lives in.
    pub fn field_id(&self) -> &FieldId {
        &self.0.field_id
    }

    /// Derives a certificate for a divisor `n` of [`order`](Self::order) by
    /// filtering the stored factorization, or `None` when `n` does not
    /// divide it.
    ///
    /// This is the reuse path: no new factorization is performed, because
    /// every prime of `n` already divides the recorded order.
    ///
    /// # Complexity
    ///
    /// Linear in the number of recorded factors.
    pub fn divisor(&self, n: u64) -> Option<OrderCertificate> {
        if n == 0 || !self.0.order.is_multiple_of(n) {
            return None;
        }
        let prime_factors = self
            .0
            .prime_factors
            .iter()
            .copied()
            .filter(|factor| n.is_multiple_of(*factor))
            .collect();
        Some(Self(Arc::new(OrderCertificateRepr {
            field_id: self.0.field_id.clone(),
            order: n,
            prime_factors,
        })))
    }
}

// ---------------------------------------------------------------------------
// Exact multiplicative orders
// ---------------------------------------------------------------------------

/// Returns the canonical-index element with the requested index.
fn element_at_canonical_index<X: FieldExtension>(
    ext: &X,
    index: u128,
) -> Result<X::Ext, FieldError> {
    let characteristic = u128::from(ext.characteristic());
    let mut remaining = index;
    let mut coordinates = Vec::with_capacity(ext.ext_degree());
    for _ in 0..ext.ext_degree() {
        coordinates.push((remaining % characteristic) as u64);
        remaining /= characteristic;
    }
    debug_assert_eq!(remaining, 0, "canonical index exceeds the field order");
    ext.ext_zero().from_prime_coords(&coordinates)
}

/// Checks exact multiplicative order using a complete distinct-prime list.
fn has_exact_order<F: FiniteField>(element: &F, order: u64, prime_factors: &[u64]) -> bool {
    element.pow(order).is_one()
        && prime_factors
            .iter()
            .all(|&prime| !element.pow(order / prime).is_one())
}

/// Multiplies modulo `modulus` without overflowing a `u64` intermediate.
fn modular_mul(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    (u128::from(lhs) * u128::from(rhs) % u128::from(modulus)) as u64
}

/// Adds modulo `modulus` without overflowing a `u64` intermediate.
fn modular_add(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((u128::from(lhs) + u128::from(rhs)) % u128::from(modulus)) as u64
}

/// Raises a `u64` to a power modulo `modulus`.
fn modular_pow(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1u64 % modulus;
    base %= modulus;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = modular_mul(result, base, modulus);
        }
        exponent >>= 1;
        if exponent != 0 {
            base = modular_mul(base, base, modulus);
        }
    }
    result
}

/// Raises a `u64` to a `usize` power modulo `modulus`.
fn modular_pow_usize(mut base: u64, mut exponent: usize, modulus: u64) -> u64 {
    let mut result = 1u64 % modulus;
    base %= modulus;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = modular_mul(result, base, modulus);
        }
        exponent >>= 1;
        if exponent != 0 {
            base = modular_mul(base, base, modulus);
        }
    }
    result
}

/// Returns the greatest common divisor of two `u64`s.
fn gcd_u64(mut lhs: u64, mut rhs: u64) -> u64 {
    while rhs != 0 {
        let remainder = lhs % rhs;
        lhs = rhs;
        rhs = remainder;
    }
    lhs
}

/// Deterministic Miller–Rabin primality test for a `u64`.
fn is_prime_u64(value: u64) -> bool {
    if value < 2 {
        return false;
    }
    for prime in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if value == prime {
            return true;
        }
        if value.is_multiple_of(prime) {
            return false;
        }
    }

    let mut odd_part = value - 1;
    let mut powers_of_two = 0;
    while odd_part.is_multiple_of(2) {
        odd_part /= 2;
        powers_of_two += 1;
    }

    // These bases are deterministic for every n < 2^64.
    for base in [2u64, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_265_022] {
        if base.is_multiple_of(value) {
            continue;
        }
        let mut witness = modular_pow(base, odd_part, value);
        if witness == 1 || witness == value - 1 {
            continue;
        }
        let mut probably_prime = false;
        for _ in 1..powers_of_two {
            witness = modular_mul(witness, witness, value);
            if witness == value - 1 {
                probably_prime = true;
                break;
            }
        }
        if !probably_prime {
            return false;
        }
    }
    true
}

/// Finds one non-trivial factor with a deterministic, bounded Pollard-rho
/// search. The bound is deliberate: an unexpectedly difficult `u64`
/// factorization reports [`FieldError::OrderFactorizationUnavailable`] instead
/// of making canonical selection loop without a limit.
fn pollard_rho_factor(value: u64) -> Option<u64> {
    if value.is_multiple_of(2) {
        return Some(2);
    }
    if value.is_multiple_of(3) {
        return Some(3);
    }

    const MAX_SEEDS: u64 = 128;
    const MAX_ITERATIONS: usize = 100_000;
    for seed in 1..=MAX_SEEDS {
        let constant = seed;
        let mut tortoise = 2u64;
        let mut hare = 2u64;
        for _ in 0..MAX_ITERATIONS {
            tortoise = modular_add(modular_mul(tortoise, tortoise, value), constant, value);
            hare = modular_add(modular_mul(hare, hare, value), constant, value);
            hare = modular_add(modular_mul(hare, hare, value), constant, value);
            let difference = tortoise.abs_diff(hare);
            let factor = gcd_u64(difference, value);
            if factor > 1 && factor < value {
                return Some(factor);
            }
        }
    }
    None
}

/// Recursively factors a `u64`, returning distinct factors after sorting.
fn factor_u64_rec(value: u64, factors: &mut Vec<u64>) -> bool {
    if value == 1 {
        return true;
    }
    if is_prime_u64(value) {
        factors.push(value);
        return true;
    }
    let Some(factor) = pollard_rho_factor(value) else {
        return false;
    };
    factor_u64_rec(factor, factors) && factor_u64_rec(value / factor, factors)
}

/// Factors a supported order into distinct ascending prime factors.
fn factor_order(order: u64) -> Option<Vec<u64>> {
    if order == 0 {
        return None;
    }
    let mut factors = Vec::new();
    if !factor_u64_rec(order, &mut factors) {
        return None;
    }
    factors.sort_unstable();
    factors.dedup();
    Some(factors)
}

/// The canonical generator of $E^{*}$: the element of least canonical index
/// $\iota$ whose multiplicative order is exactly $|E^{*}|$.
///
/// The scan is normative: candidates are considered in the order
/// $\iota = 2, 3, 4, \ldots$, where $\iota$ is the base-$p$ index emitted by
/// [`FieldIdentity::write_prime_coords`]. Index `0` is zero and index `1` is
/// one, so neither is considered; for $|E^{*}| = 1$, the result is one.
/// Every candidate is checked as $x^N = 1$ and
/// $x^{N/q} \ne 1$ for every distinct prime $q \mid N$.
///
/// # Errors
///
/// - [`FieldError::OrderFactorizationUnavailable`] when $|E^{*}|$ does not
///   fit the supported `u64` certificate/factorization range, or its bounded
///   factorization cannot complete.
/// - [`FieldError::NoElementOfOrder`] when the carrier reports an invalid
///   zero unit-group order.
///
/// # Complexity
///
/// Factoring costs the bounded deterministic `u64` factorization procedure.
/// The candidate search costs $O(|E^{*}|)$ candidates, each requiring
/// $O(\log |E^{*}|)$ field multiplications per order check and
/// $O([E:\mathbb{F}_p])$ coordinate conversion.
pub fn canonical_generator<X: FieldExtension>(
    ext: &X,
) -> Result<(X::Ext, OrderCertificate), FieldError> {
    let unit_group_order = ext
        .ext_unit_group_order()
        .ok_or(FieldError::OrderFactorizationUnavailable { order: u128::MAX })?;
    let order =
        u64::try_from(unit_group_order).map_err(|_| FieldError::OrderFactorizationUnavailable {
            order: unit_group_order,
        })?;
    if order == 0 {
        return Err(FieldError::NoElementOfOrder {
            requested: order,
            unit_group_order,
        });
    }
    let prime_factors = factor_order(order).ok_or(FieldError::OrderFactorizationUnavailable {
        order: unit_group_order,
    })?;
    let certificate = OrderCertificate::new(ext.ext_id().clone(), order, prime_factors)?;

    if order == 1 {
        let element = ext.ext_one();
        debug_assert!(has_exact_order(
            &element,
            order,
            certificate.prime_factors()
        ));
        return Ok((element, certificate));
    }

    for index in 2..=u128::from(order) {
        let candidate = element_at_canonical_index(ext, index)?;
        if has_exact_order(&candidate, order, certificate.prime_factors()) {
            return Ok((candidate, certificate));
        }
    }

    // A finite field's multiplicative group is cyclic. Reaching this branch
    // means the FieldExtension carrier did not model the field named by its
    // certificate, so report the failed exact-order request without panicking.
    Err(FieldError::NoElementOfOrder {
        requested: order,
        unit_group_order,
    })
}

/// Derives the deterministic element of exact multiplicative order `n`.
///
/// Let $N = |E^{*}|$. The operation requires $n \mid N$, obtains the
/// canonical generator $g$ from [`canonical_generator`], and returns
/// $g^{N/n}$. It validates the result exactly as $x^n = 1$ and
/// $x^{n/q} \ne 1$ for every distinct prime $q \mid n$. The returned
/// [`OrderCertificate`] carries that complete factor list.
///
/// # Errors
///
/// - [`FieldError::NoElementOfOrder`] when `n` is zero or does not divide
///   $|E^{*}|$.
/// - [`FieldError::OrderFactorizationUnavailable`] when the relevant unit
///   group order cannot be represented or factored by the supported
///   deterministic procedure.
///
/// # Complexity
///
/// Includes [`canonical_generator`] and one additional exponentiation taking
/// $O(\log N)$ field multiplications.
pub fn element_of_exact_order<X: FieldExtension>(
    ext: &X,
    n: u64,
) -> Result<(X::Ext, OrderCertificate), FieldError> {
    let unit_group_order = ext
        .ext_unit_group_order()
        .ok_or(FieldError::OrderFactorizationUnavailable { order: u128::MAX })?;
    if n == 0 || !unit_group_order.is_multiple_of(u128::from(n)) {
        return Err(FieldError::NoElementOfOrder {
            requested: n,
            unit_group_order,
        });
    }

    let (generator, full_certificate) = canonical_generator(ext)?;
    let certificate = full_certificate
        .divisor(n)
        .ok_or(FieldError::NoElementOfOrder {
            requested: n,
            unit_group_order,
        })?;
    let element = generator.pow(full_certificate.order() / n);
    let exact = has_exact_order(&element, n, certificate.prime_factors());
    debug_assert!(exact);
    if !exact {
        return Err(FieldError::NoElementOfOrder {
            requested: n,
            unit_group_order,
        });
    }
    Ok((element, certificate))
}

// ---------------------------------------------------------------------------
// Cyclotomic cosets
// ---------------------------------------------------------------------------

/// A deterministic partition of residues modulo a cyclotomic modulus.
///
/// Cosets are ordered by their smallest representative. Each coset starts at
/// that representative and then lists the orbit produced by repeated
/// multiplication by `q` modulo `n`. A partition returned by
/// [`cyclotomic_closure`] contains only the cosets meeting its seed set.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::cyclotomic_cosets_mod;
///
/// let partition = cyclotomic_cosets_mod(2, 15)?;
/// assert_eq!(partition.cosets().len(), 5);
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CosetPartition {
    n: u64,
    q_mod_n: u64,
    cosets: Vec<Vec<u64>>,
}

impl CosetPartition {
    /// Returns the partition's cosets in deterministic order.
    ///
    /// The outer order is by smallest representative. The first member of
    /// every inner vector is that representative, and later members follow
    /// the multiplication orbit rather than numeric sorting.
    ///
    /// # Complexity
    ///
    /// `O(1)`; the returned slice borrows the partition.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::cyclotomic_cosets_mod;
    ///
    /// let partition = cyclotomic_cosets_mod(2, 15)?;
    /// assert_eq!(partition.cosets()[2], vec![3, 6, 12, 9]);
    /// # Ok::<(), gf2_core::field::extension::FieldError>(())
    /// ```
    pub fn cosets(&self) -> &[Vec<u64>] {
        &self.cosets
    }

    /// Returns the sorted union of all residues in the partition.
    ///
    /// For a full partition this is `0..n`; for a closure it is the closed
    /// defining set selected by the supplied seeds.
    ///
    /// # Complexity
    ///
    /// `O(k log k)`, where `k` is the number of returned residues.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::cyclotomic_cosets_mod;
    ///
    /// let partition = cyclotomic_cosets_mod(2, 15)?;
    /// assert_eq!(partition.defining_set(), (0..15).collect::<Vec<_>>());
    /// # Ok::<(), gf2_core::field::extension::FieldError>(())
    /// ```
    pub fn defining_set(&self) -> Vec<u64> {
        let count = self.cosets.iter().map(Vec::len).sum();
        let mut defining_set = Vec::with_capacity(count);
        for coset in &self.cosets {
            defining_set.extend(coset.iter().copied());
        }
        defining_set.sort_unstable();
        defining_set
    }

    /// Returns whether `exponent` belongs to the partition.
    ///
    /// The exponent is reduced modulo the partition's `n`, so an equivalent
    /// representative such as `n + 3` is treated as exponent `3`.
    ///
    /// # Complexity
    ///
    /// `O(k)` in the number of stored residues.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::cyclotomic_cosets_mod;
    ///
    /// let partition = cyclotomic_cosets_mod(2, 15)?;
    /// assert!(partition.contains(12));
    /// assert!(partition.contains(27));
    /// # Ok::<(), gf2_core::field::extension::FieldError>(())
    /// ```
    pub fn contains(&self, exponent: u64) -> bool {
        let residue = exponent % self.n;
        self.cosets.iter().any(|coset| coset.contains(&residue))
    }

    /// Returns the zero-based index of the coset containing `exponent`.
    ///
    /// The exponent is reduced modulo the partition's `n`. `None` means that
    /// the residue is outside this partition, which can occur for a closure
    /// that contains only a subset of all cosets.
    ///
    /// # Complexity
    ///
    /// `O(k)` in the number of stored residues.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::cyclotomic_cosets_mod;
    ///
    /// let partition = cyclotomic_cosets_mod(2, 15)?;
    /// assert_eq!(partition.coset_of(3), Some(2));
    /// assert_eq!(partition.coset_of(18), Some(2));
    /// # Ok::<(), gf2_core::field::extension::FieldError>(())
    /// ```
    pub fn coset_of(&self, exponent: u64) -> Option<usize> {
        let residue = exponent % self.n;
        self.cosets
            .iter()
            .position(|coset| coset.contains(&residue))
    }
}

/// Returns the `q`-cyclotomic cosets modulo `n`.
///
/// The multiplier is `q = |B| mod n`, where `B` is the base field named by
/// `ext`. It is computed as `p^[B:F_p] mod n`, so the base-field order need
/// not fit in a `u64` or even in `u128`. Every residue modulo `n` belongs to
/// exactly one returned coset.
///
/// # Errors
///
/// - [`FieldError::InvalidCyclotomicModulus`] when `n` is zero.
/// - [`FieldError::NonCoprimeCyclotomicParameters`] when `gcd(n, q) != 1`.
/// - [`FieldError::CyclotomicModulusTooLarge`] when `n` cannot be represented
///   as an in-memory partition on this target.
///
/// # Complexity
///
/// `O(n)` arithmetic steps and `O(n)` memory for the visited table and the
/// returned partition.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{cyclotomic_cosets, BinaryPrimeExt};
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let ext = BinaryPrimeExt::new(field)?;
/// let partition = cyclotomic_cosets(&ext, 15)?;
/// assert_eq!(partition.cosets()[1], vec![1, 2, 4, 8]);
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn cyclotomic_cosets<X: FieldExtension>(ext: &X, n: u64) -> Result<CosetPartition, FieldError> {
    let q_mod_n = base_order_mod(ext, n)?;
    cyclotomic_cosets_mod(q_mod_n, n)
}

/// Returns the `q`-cyclotomic closure of `seeds`, partitioned into cosets.
///
/// Each seed is interpreted modulo `n`. The result contains every residue
/// reachable from a seed by repeated multiplication by `q = |B| mod n`, and
/// no other residues. Duplicate seeds and seeds outside `[0, n)` therefore
/// have no special effect.
///
/// # Errors
///
/// Returns the same errors as [`cyclotomic_cosets`].
///
/// # Complexity
///
/// `O(n + n s)` in the straightforward scan, where `s` is the number of
/// supplied seeds, and `O(n)` memory for the full partition plus the result.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{cyclotomic_closure, BinaryPrimeExt};
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let ext = BinaryPrimeExt::new(field)?;
/// let closure = cyclotomic_closure(&ext, 15, &[3, 5])?;
/// assert_eq!(closure.defining_set(), vec![3, 5, 6, 9, 10, 12]);
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn cyclotomic_closure<X: FieldExtension>(
    ext: &X,
    n: u64,
    seeds: &[u64],
) -> Result<CosetPartition, FieldError> {
    let full_partition = cyclotomic_cosets(ext, n)?;
    let CosetPartition {
        n,
        q_mod_n,
        cosets: all_cosets,
    } = full_partition;
    let too_large = |_| FieldError::CyclotomicModulusTooLarge { modulus: n };

    // Selected cosets are moved out of the owned full partition, so no
    // element storage is reallocated; only the outer vector grows, and it
    // grows fallibly per the too-large contract.
    let mut cosets: Vec<Vec<u64>> = Vec::new();
    for coset in all_cosets {
        let selected = coset
            .iter()
            .any(|&member| seeds.iter().any(|&seed| seed % n == member));
        if selected {
            cosets.try_reserve(1).map_err(too_large)?;
            cosets.push(coset);
        }
    }

    Ok(CosetPartition { n, q_mod_n, cosets })
}

/// Returns the `q`-cyclotomic cosets modulo `n` from an already reduced
/// multiplier.
///
/// The supplied `q_mod_n` is normalized once more modulo `n`, making the
/// function convenient for callers that hold `q` rather than its reduced
/// representative. The multiplication map must be a permutation, so
/// `gcd(n, q) = 1` is required.
///
/// # Errors
///
/// - [`FieldError::InvalidCyclotomicModulus`] when `n` is zero.
/// - [`FieldError::NonCoprimeCyclotomicParameters`] when `gcd(n, q) != 1`.
/// - [`FieldError::CyclotomicModulusTooLarge`] when `n` cannot be represented
///   as an in-memory partition on this target.
///
/// # Complexity
///
/// `O(n)` arithmetic steps and `O(n)` memory for the visited table and the
/// returned partition.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::cyclotomic_cosets_mod;
///
/// let partition = cyclotomic_cosets_mod(2, 15)?;
/// assert_eq!(partition.cosets()[4], vec![7, 14, 13, 11]);
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn cyclotomic_cosets_mod(q_mod_n: u64, n: u64) -> Result<CosetPartition, FieldError> {
    if n == 0 {
        return Err(FieldError::InvalidCyclotomicModulus { modulus: n });
    }

    let q_mod_n = q_mod_n % n;
    let gcd = gcd_u64(q_mod_n, n);
    if gcd != 1 {
        return Err(FieldError::NonCoprimeCyclotomicParameters {
            q_mod_n,
            modulus: n,
            gcd,
        });
    }

    let residue_count =
        usize::try_from(n).map_err(|_| FieldError::CyclotomicModulusTooLarge { modulus: n })?;
    let too_large = |_| FieldError::CyclotomicModulusTooLarge { modulus: n };
    let mut visited = Vec::new();
    visited
        .try_reserve_exact(residue_count)
        .map_err(too_large)?;
    visited.resize(residue_count, false);
    let mut cosets: Vec<Vec<u64>> = Vec::new();

    for representative in 0..residue_count {
        if visited[representative] {
            continue;
        }

        // First orbit pass counts the coset so its storage can be reserved
        // fallibly; every allocation scaling with `n` reports the typed
        // too-large error instead of aborting.
        let mut coset_len = 0usize;
        let mut current = representative as u64;
        loop {
            coset_len += 1;
            current = modular_mul(current, q_mod_n, n);
            if current as usize == representative {
                break;
            }
        }

        let mut coset = Vec::new();
        coset.try_reserve_exact(coset_len).map_err(too_large)?;
        let mut current = representative as u64;
        while !visited[current as usize] {
            visited[current as usize] = true;
            coset.push(current);
            current = modular_mul(current, q_mod_n, n);
        }
        cosets.try_reserve(1).map_err(too_large)?;
        cosets.push(coset);
    }

    Ok(CosetPartition { n, q_mod_n, cosets })
}

/// Computes the base-field cardinality modulo `n` without materializing the
/// cardinality itself.
fn base_order_mod<X: FieldExtension>(ext: &X, n: u64) -> Result<u64, FieldError> {
    if n == 0 {
        return Err(FieldError::InvalidCyclotomicModulus { modulus: n });
    }
    Ok(modular_pow_usize(
        ext.characteristic(),
        ext.base_degree(),
        n,
    ))
}

// ---------------------------------------------------------------------------
// Relative-field derived operations
// ---------------------------------------------------------------------------

/// Returns the conjugate orbit of `x` under the relative Frobenius.
///
/// The returned orbit starts with `x` and contains each distinct element in
/// ascending Frobenius-step order. Its length is the degree of the minimal
/// polynomial of `x` over the base field. The input must be an element of the
/// extension field named by `ext`.
///
/// # Complexity
///
/// `O(r²)` equality checks and `O(r)` relative-Frobenius applications, where
/// `r = ext.relative_degree()`; the orbit length is at most `r`.
///
/// # Panics
///
/// Panics if the relative Frobenius repeats an element other than the initial
/// element or does not return to it within `r` steps. Either case means the
/// `FieldExtension` witness violates its finite-field orbit contract.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{conjugates, BinaryPrimeExt, FieldExtension};
/// use gf2_core::field::FiniteFieldExt;
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let ext = BinaryPrimeExt::new(field.clone())?;
/// let orbit = conjugates(&ext, &field.element(2));
/// assert_eq!(orbit.len(), 4);
/// assert_eq!(orbit[0], field.element(2));
/// assert_eq!(orbit[1], field.element(2).square());
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn conjugates<X: FieldExtension>(ext: &X, x: &X::Ext) -> Vec<X::Ext> {
    let relative_degree = ext.relative_degree();
    let mut orbit = Vec::with_capacity(relative_degree);
    let mut current = x.clone();

    for _ in 0..=relative_degree {
        if let Some(first_repeat) = orbit.iter().position(|conjugate| conjugate == &current) {
            assert_eq!(
                first_repeat, 0,
                "FieldExtension invariant violated: relative Frobenius orbit is not cyclic"
            );
            return orbit;
        }
        orbit.push(current.clone());
        current = ext.relative_frobenius(&current, 1);
    }

    panic!("FieldExtension invariant violated: relative Frobenius orbit exceeded relative degree");
}

/// Returns the monic minimal polynomial of `x` over the base field.
///
/// The polynomial is formed as
/// `Π (T - φ_B^i(x))` over the distinct conjugate orbit and each coefficient
/// is checked and restricted into `B`. Its degree is the relative-Frobenius
/// orbit size, so it is minimal over the base field. The input must be an
/// element of the extension field named by `ext`.
///
/// # Errors
///
/// Returns [`FieldError::IdentityMismatch`] when `x` does not belong to the
/// extension field named by `ext`.
///
/// # Panics
///
/// Panics if a product coefficient cannot be restricted to the base field.
/// Frobenius invariance makes that failure an internal `FieldExtension`
/// invariant violation, not a recoverable algebraic result.
///
/// # Complexity
///
/// `O(r²)` extension-field operations with the generic polynomial product,
/// plus `O(r)` checked restrictions, where `r = ext.relative_degree()`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{minimal_polynomial, BinaryPrimeExt};
/// use gf2_core::gf2m::Gf2mField;
/// use gf2_core::gfp::Fp;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let ext = BinaryPrimeExt::new(field.clone())?;
/// let polynomial = minimal_polynomial(&ext, &field.element(2))?;
/// let coefficients: Vec<_> = (0..=polynomial.degree().unwrap())
///     .map(|i| polynomial.coeff(i))
///     .collect();
/// assert_eq!(coefficients, vec![Fp::<2>::new(1), Fp::<2>::new(1),
///     Fp::<2>::new(0), Fp::<2>::new(0), Fp::<2>::new(1)]);
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn minimal_polynomial<X: FieldExtension>(
    ext: &X,
    x: &X::Ext,
) -> Result<FieldPoly<X::Base>, FieldError> {
    ensure_extension_element(ext, x)?;
    let orbit = conjugates(ext, x);
    let product = FieldPoly::from_roots(&orbit);
    let coefficients = (0..=orbit.len())
        .map(|index| restrict_invariant(ext, &product.coeff(index)))
        .collect();
    Ok(FieldPoly::new(coefficients))
}

/// Returns the relative field trace of `x` from `E` to `B`.
///
/// Computes `Tr_{E/B}(x) = Σ_{i=0}^{r-1} φ_B^i(x)`, including repeated
/// conjugates when `x` lies in a proper intermediate subfield, and restricts
/// the result to `B`. The input must be an element of the extension field
/// named by `ext`.
///
/// # Errors
///
/// Returns [`FieldError::IdentityMismatch`] when `x` does not belong to the
/// extension field named by `ext`.
///
/// # Panics
///
/// Panics if the trace is not restrictable to the base field. That is an
/// internal `FieldExtension` invariant violation because the trace is fixed
/// by the relative Frobenius.
///
/// # Complexity
///
/// `O(r)` relative-Frobenius applications and extension-field additions, plus
/// one checked restriction, where `r = ext.relative_degree()`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{relative_trace, BinaryPrimeExt};
/// use gf2_core::gf2m::Gf2mField;
/// use gf2_core::gfp::Fp;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let ext = BinaryPrimeExt::new(field.clone())?;
/// let trace = relative_trace(&ext, &field.element(2))?;
/// assert_eq!(trace, Fp::<2>::new(0));
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn relative_trace<X: FieldExtension>(ext: &X, x: &X::Ext) -> Result<X::Base, FieldError> {
    ensure_extension_element(ext, x)?;
    let mut trace = x.zero_like();
    let mut current = x.clone();
    for _ in 0..ext.relative_degree() {
        trace += current.clone();
        current = ext.relative_frobenius(&current, 1);
    }
    Ok(restrict_invariant(ext, &trace))
}

/// Returns the relative field norm of `x` from `E` to `B`.
///
/// Computes `N_{E/B}(x) = Π_{i=0}^{r-1} φ_B^i(x)`, including repeated
/// conjugates when `x` lies in a proper intermediate subfield, and restricts
/// the result to `B`. The input must be an element of the extension field
/// named by `ext`.
///
/// # Errors
///
/// Returns [`FieldError::IdentityMismatch`] when `x` does not belong to the
/// extension field named by `ext`.
///
/// # Panics
///
/// Panics if the norm is not restrictable to the base field. That is an
/// internal `FieldExtension` invariant violation because the norm is fixed by
/// the relative Frobenius.
///
/// # Complexity
///
/// `O(r)` relative-Frobenius applications and extension-field multiplications,
/// plus one checked restriction, where `r = ext.relative_degree()`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{relative_norm, BinaryPrimeExt};
/// use gf2_core::gf2m::Gf2mField;
/// use gf2_core::gfp::Fp;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let ext = BinaryPrimeExt::new(field.clone())?;
/// let norm = relative_norm(&ext, &field.element(2))?;
/// assert_eq!(norm, Fp::<2>::new(1));
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn relative_norm<X: FieldExtension>(ext: &X, x: &X::Ext) -> Result<X::Base, FieldError> {
    ensure_extension_element(ext, x)?;
    let mut norm = x.one_like();
    let mut current = x.clone();
    for _ in 0..ext.relative_degree() {
        norm = norm * current.clone();
        current = ext.relative_frobenius(&current, 1);
    }
    Ok(restrict_invariant(ext, &norm))
}

fn ensure_extension_element<X: FieldExtension>(ext: &X, x: &X::Ext) -> Result<(), FieldError> {
    let expected = ext.ext_id().clone();
    let found = x.field_id();
    if expected == found {
        Ok(())
    } else {
        Err(FieldError::IdentityMismatch { expected, found })
    }
}

fn restrict_invariant<X: FieldExtension>(ext: &X, x: &X::Ext) -> X::Base {
    match ext.restrict(x) {
        Ok(value) => value,
        Err(error) => panic!(
            "FieldExtension invariant violated: Frobenius-fixed value is not in the base field: {error}"
        ),
    }
}

// ---------------------------------------------------------------------------
// The extension relation
// ---------------------------------------------------------------------------

/// The relation "$E$ is an extension of $B$", witnessed by a value.
///
/// Implementors are cheap to clone. Equality compares the certificate and any
/// runtime field parameters the witness carries.
///
/// The required surface is four methods; everything else is a projection of
/// the certificate or a one-line derivation kept adjacent to its primitive.
/// [`contains`](Self::contains) and
/// [`relative_frobenius`](Self::relative_frobenius) are provided rather than
/// required so a carrier with a table-backed or structural shortcut may
/// override them, and the laws pin the meaning either way.
///
/// Every method is stated for elements of the fields the witness names:
/// `embed` takes an element of [`base_id`](Self::base_id), and `try_restrict`,
/// `contains`, and `relative_frobenius` take an element of
/// [`ext_id`](Self::ext_id). Passing a carrier value from an unrelated field
/// is outside the contract.
///
/// The trait is not object-safe, and static dispatch is the point; a
/// type-erased handle is a separate concern that does not constrain this
/// surface.
pub trait FieldExtension: Clone + fmt::Debug + Eq {
    /// Element type of the base field $B$.
    type Base: FieldIdentity;
    /// Element type of the extension field $E$.
    type Ext: FieldIdentity;

    /// Returns the evidence that this pair is a valid extension.
    fn certificate(&self) -> &ExtensionCertificate;

    /// Returns the zero of $B$.
    ///
    /// This is the single required element witness; every other witness
    /// derives from it.
    fn base_zero(&self) -> Self::Base;

    /// Returns the image of $x \in B$ under the field embedding
    /// $B \hookrightarrow E$.
    fn embed(&self, x: &Self::Base) -> Self::Ext;

    /// Returns the preimage of `x` under [`embed`](Self::embed), or `None`
    /// when $x \notin B$.
    fn try_restrict(&self, x: &Self::Ext) -> Option<Self::Base>;

    /// Returns the identity of the base field.
    fn base_id(&self) -> &FieldId {
        self.certificate().base_id()
    }

    /// Returns the identity of the extension field.
    fn ext_id(&self) -> &FieldId {
        self.certificate().ext_id()
    }

    /// Returns the characteristic shared by both fields.
    fn characteristic(&self) -> u64 {
        self.base_id().characteristic()
    }

    /// Returns $d_B = [B : \mathbb{F}_p]$.
    fn base_degree(&self) -> usize {
        self.base_id().degree()
    }

    /// Returns $d_E = [E : \mathbb{F}_p]$.
    fn ext_degree(&self) -> usize {
        self.ext_id().degree()
    }

    /// Returns $r = [E : B]$.
    ///
    /// Exact division; the certificate guarantees $d_B \mid d_E$.
    fn relative_degree(&self) -> usize {
        self.certificate().relative_degree()
    }

    /// Returns $|B|$, or `None` when it exceeds `u128::MAX`.
    fn base_order(&self) -> Option<u128> {
        self.base_id().order()
    }

    /// Returns $|E|$, or `None` when it exceeds `u128::MAX`.
    fn ext_order(&self) -> Option<u128> {
        self.ext_id().order()
    }

    /// Returns $|E^{*}|$, or `None` when it exceeds `u128::MAX`.
    fn ext_unit_group_order(&self) -> Option<u128> {
        self.ext_id().unit_group_order()
    }

    /// Returns the one of $B$.
    fn base_one(&self) -> Self::Base {
        self.base_zero().one_like()
    }

    /// Returns the zero of $E$.
    fn ext_zero(&self) -> Self::Ext {
        self.embed(&self.base_zero())
    }

    /// Returns the one of $E$.
    fn ext_one(&self) -> Self::Ext {
        self.embed(&self.base_one())
    }

    /// Returns $\varphi_B^{\,k}(x) = x^{|B|^{k}}$, the $k$-th power of the
    /// generator of $\mathrm{Gal}(E/B)$.
    ///
    /// $k$ is reduced modulo $r$ first, since $\varphi_B^{\,r}$ is the
    /// identity on $E$. The implementation iterates the absolute Frobenius
    /// $d_B \cdot (k \bmod r)$ times, so $|B|$ never has to fit in a `u64` —
    /// a $\mathrm{GF}(2^{127})$ base would otherwise overflow the exponent.
    ///
    /// # Complexity
    ///
    /// $d_B \cdot (k \bmod r)$ exponentiations by the characteristic.
    fn relative_frobenius(&self, x: &Self::Ext, k: u32) -> Self::Ext {
        let steps = self.base_degree() * (k as usize % self.relative_degree());
        let characteristic = self.characteristic();
        let mut y = x.clone();
        for _ in 0..steps {
            y = y.pow(characteristic);
        }
        y
    }

    /// Returns `true` when $x \in B$, decided as $\varphi_B(x) = x$.
    ///
    /// The fixed field of $\varphi_B$ on $E$ is precisely $B$, so this is a
    /// correct generic default and only [`try_restrict`](Self::try_restrict),
    /// which must produce a `Base` value, stays required.
    fn contains(&self, x: &Self::Ext) -> bool {
        self.relative_frobenius(x, 1) == *x
    }

    /// Returns the preimage of `x` under [`embed`](Self::embed).
    ///
    /// # Errors
    ///
    /// [`FieldError::NotInBase`] when `x` does not lie in the base field.
    fn restrict(&self, x: &Self::Ext) -> Result<Self::Base, FieldError> {
        self.try_restrict(x).ok_or(FieldError::NotInBase)
    }
}

// ---------------------------------------------------------------------------
// Conversion between carriers of one field
// ---------------------------------------------------------------------------

/// Transports `src` into the field witnessed by `dst_witness`.
///
/// Because [`FieldId`] pins the basis, this is the identity map on coordinate
/// vectors, and it is the unique basis-preserving isomorphism between two
/// carriers of equal identity. No search, no isomorphism-finding, no
/// ambiguity — which is why identity is presentation identity rather than
/// abstract isomorphism class.
///
/// # Errors
///
/// - [`FieldError::IdentityMismatch`] unless the two identities are equal.
/// - The coordinate errors of [`FieldIdentity::from_prime_coords`] when the
///   destination carrier rejects a coordinate vector its own identity
///   admits, which is an implementation defect rather than a caller error.
///
/// # Complexity
///
/// Linear in the absolute degree of the field.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{convert_element, FieldError, FieldIdentity};
/// use gf2_core::gf2m::{Gf2mField, Gf2mField_};
///
/// // Two carriers of the same GF(2^8) presentation: u64-backed and u128-backed.
/// let narrow = Gf2mField::gf256();
/// let wide = Gf2mField_::<u128>::new(8, 0b100011101);
/// assert_eq!(narrow.element(0x53).field_id(), wide.element(0x53).field_id());
///
/// let transported = convert_element(&narrow.element(0x53), &wide.zero())?;
/// assert_eq!(transported, wide.element(0x53));
///
/// // A different defining polynomial is a different presentation.
/// let other = Gf2mField::new(8, 0b100011011);
/// assert!(matches!(
///     convert_element(&narrow.element(0x53), &other.zero()),
///     Err(FieldError::IdentityMismatch { .. })
/// ));
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
pub fn convert_element<S, D>(src: &S, dst_witness: &D) -> Result<D, FieldError>
where
    S: FieldIdentity,
    D: FieldIdentity,
{
    let expected = dst_witness.field_id();
    let found = src.field_id();
    if expected != found {
        return Err(FieldError::IdentityMismatch { expected, found });
    }
    let mut coords = Vec::new();
    src.write_prime_coords(&mut coords);
    dst_witness.from_prime_coords(&coords)
}

/// [`convert_element`] with the destination's [`ConstField::zero`] as the
/// witness.
///
/// # Errors
///
/// The same conditions as [`convert_element`].
pub fn convert_into_const<S, D>(src: &S) -> Result<D, FieldError>
where
    S: FieldIdentity,
    D: ConstField + FieldIdentity,
{
    convert_element(src, &D::zero())
}

// ---------------------------------------------------------------------------
// Concrete witnesses
// ---------------------------------------------------------------------------

/// The extension $\mathrm{GF}(2) \subset \mathrm{GF}(2^m)$ for a runtime
/// binary field.
///
/// The embedding carries the two elements of $\mathrm{GF}(2)$ onto the zero
/// and one of the runtime field, and membership is the Frobenius fixed-point
/// test $x^{2} = x$.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension};
/// use gf2_core::gf2m::Gf2mField;
///
/// let ext = BinaryPrimeExt::new(Gf2mField::gf256())?;
/// assert_eq!(ext.relative_degree(), 8);
/// assert_eq!(ext.ext_unit_group_order(), Some(255));
/// assert_eq!(ext.field().degree(), 8);
/// # Ok::<(), gf2_core::field::extension::FieldError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryPrimeExt<V: UintExt = u64> {
    field: Gf2mField_<V>,
    certificate: ExtensionCertificate,
}

impl<V: UintExt> BinaryPrimeExt<V> {
    /// Builds the witness for $\mathrm{GF}(2) \subset \mathrm{GF}(2^m)$,
    /// deciding the defining polynomial's irreducibility.
    ///
    /// [`Gf2mField_::new`] takes irreducibility on trust, so a runtime field
    /// can name a quotient ring that is not a field. This constructor settles
    /// the question with [`prove_irreducible`] and records the answer as
    /// [`CertificateBasis::Proved`]. When $m = 1$ the degree-one quotient
    /// collapses onto $\mathrm{GF}(2)$ itself, which is
    /// [`CertificateBasis::Identity`] and has nothing to decide.
    ///
    /// Deciding costs `O(m³)` polynomial-coefficient operations. Repeated
    /// construction over one presentation should hold the resulting
    /// certificate and go through
    /// [`from_certificate_unchecked`](Self::from_certificate_unchecked),
    /// which skips the decision entirely.
    ///
    /// # Errors
    ///
    /// - [`FieldError::NonMonicModulus`] when the field's defining polynomial
    ///   has no term of degree `m`, violating the contract of
    ///   [`Gf2mField_::new`].
    /// - [`FieldError::ReducibleModulus`] with a [`FactorWitness`] when the
    ///   defining polynomial factors, so the carrier is not a field.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::extension::{BinaryPrimeExt, FieldError};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// // x^4 + 1 = (x + 1)^4 is monic of degree four, and not a field modulus.
    /// assert!(matches!(
    ///     BinaryPrimeExt::new(Gf2mField::new(4, 0b10001)),
    ///     Err(FieldError::ReducibleModulus { .. })
    /// ));
    /// ```
    pub fn new(field: Gf2mField_<V>) -> Result<Self, FieldError> {
        let base = FieldId::prime(2)?;
        let modulus = gf2m_modulus_id(&field)?;
        let certificate = if modulus.degree() == 1 {
            // The quotient collapses onto GF(2); every monic linear modulus
            // is irreducible, so there is nothing to decide.
            ExtensionCertificate::trivial(base)
        } else {
            prove_irreducible(&binary_modulus_polynomial(&modulus), &Fp::<2>::new(0))?
                .extension_certificate()
        };
        Ok(Self { field, certificate })
    }

    /// Builds the witness from a certificate the caller already holds, doing
    /// no validation of its own.
    ///
    /// The certificate is a **memo** of validation performed earlier. The
    /// irreducibility decision that [`new`](Self::new) runs is `O(m³)`, so a
    /// caller constructing repeatedly over one presentation runs it once and
    /// presents the result here. This method skips that decision entirely:
    /// its only check is the identity comparison below — structural, linear
    /// in the modulus degree, free of any decision procedure — which is there
    /// to catch an honest mix-up, not to re-derive anything.
    ///
    /// # Contract
    ///
    /// The caller promises that the certificate really does memoize a
    /// validation of the pair `field` names. Nothing here confirms it. A
    /// certificate that memoizes something else — a modulus that was never
    /// decided, or one that was decided and found reducible — produces a
    /// witness over a carrier that is not a field, and every result computed
    /// through it is meaningless. That is caller error in the same way that
    /// passing wrong parameters to any other `_unchecked` API is: nothing in
    /// this crate can detect it after the fact, which is precisely why the
    /// name says so. `@/inv/caller-trusted-fast-paths` fixes this contract
    /// shape project-wide.
    ///
    /// Use [`new`](Self::new) when the modulus has not already been decided.
    ///
    /// # Errors
    ///
    /// - [`FieldError::NonMonicModulus`] when the field's defining polynomial
    ///   has no term of degree `m`.
    /// - [`FieldError::IdentityMismatch`] when the certificate names a
    ///   different pair than `field` does.
    pub fn from_certificate_unchecked(
        field: Gf2mField_<V>,
        certificate: ExtensionCertificate,
    ) -> Result<Self, FieldError> {
        let base = FieldId::prime(2)?;
        if certificate.base_id() != &base {
            return Err(FieldError::IdentityMismatch {
                expected: base,
                found: certificate.base_id().clone(),
            });
        }
        let ext = gf2m_field_id(&field)?;
        if certificate.ext_id() != &ext {
            return Err(FieldError::IdentityMismatch {
                expected: ext,
                found: certificate.ext_id().clone(),
            });
        }
        Ok(Self { field, certificate })
    }

    /// Returns the extension field this witness embeds into.
    pub fn field(&self) -> &Gf2mField_<V> {
        &self.field
    }
}

impl<V: UintExt> FieldExtension for BinaryPrimeExt<V> {
    type Base = Fp<2>;
    type Ext = Gf2mElement_<V>;

    fn certificate(&self) -> &ExtensionCertificate {
        &self.certificate
    }

    fn base_zero(&self) -> Self::Base {
        Fp::<2>::new(0)
    }

    fn embed(&self, x: &Self::Base) -> Self::Ext {
        if x.is_zero() {
            self.field.zero()
        } else {
            self.field.one()
        }
    }

    fn try_restrict(&self, x: &Self::Ext) -> Option<Self::Base> {
        let value = x.value();
        if value == V::ZERO {
            Some(Fp::<2>::new(0))
        } else if value == V::ONE {
            Some(Fp::<2>::new(1))
        } else {
            None
        }
    }
}

/// A compile-time field presented as a simple extension of a compile-time
/// base.
///
/// [`QuadraticExt`] and [`CubicExt`] implement it by forwarding to their
/// existing inherent `from_base`, so the binomial towers gain the extension
/// relation without a second configuration concept.
pub trait ConstSimpleExtension: ConstField + FieldIdentity {
    /// Element type of the compile-time base field.
    type ConstBase: ConstField + FieldIdentity;

    /// Returns the image of `x` under the embedding of the base.
    fn from_base(x: Self::ConstBase) -> Self;

    /// Returns the preimage of `self` under [`from_base`](Self::from_base),
    /// or `None` when the higher coefficients do not vanish.
    fn try_into_base(self) -> Option<Self::ConstBase>;

    /// Returns the modulus this extension is presented by.
    ///
    /// # Panics
    ///
    /// Panics if the implementor's modulus is not monic over its base with
    /// in-range coordinates, which no correct implementation produces.
    fn modulus_id() -> ModulusId;
}

impl<C: ExtConfig> ConstSimpleExtension for QuadraticExt<C>
where
    C::BaseField: FieldIdentity,
{
    type ConstBase = C::BaseField;

    fn from_base(x: Self::ConstBase) -> Self {
        QuadraticExt::<C>::from_base(x)
    }

    fn try_into_base(self) -> Option<Self::ConstBase> {
        self.c1().is_zero().then_some(self.c0())
    }

    fn modulus_id() -> ModulusId {
        let beta = non_residue::<C>();
        binomial_modulus_id(&beta.field_id(), &beta, 2).expect("x^2 - beta is monic over its base")
    }
}

impl<C: ExtConfig> ConstSimpleExtension for CubicExt<C>
where
    C::BaseField: FieldIdentity,
{
    type ConstBase = C::BaseField;

    fn from_base(x: Self::ConstBase) -> Self {
        CubicExt::<C>::from_base(x)
    }

    fn try_into_base(self) -> Option<Self::ConstBase> {
        (self.c1().is_zero() && self.c2().is_zero()).then_some(self.c0())
    }

    fn modulus_id() -> ModulusId {
        let beta = non_residue::<C>();
        binomial_modulus_id(&beta.field_id(), &beta, 3).expect("x^3 - beta is monic over its base")
    }
}

/// Witness for any [`ConstSimpleExtension`]; zero-sized apart from the
/// certificate.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{ConstExt, FieldExtension};
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
///
/// struct Gf49Config;
/// impl ExtConfig for Gf49Config {
///     type BaseField = Fp<7>;
///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
/// }
///
/// let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
/// assert_eq!(ext.relative_degree(), 2);
/// assert_eq!(ext.ext_order(), Some(49));
///
/// // The class of x is not in GF(7), and its Frobenius conjugate is not itself.
/// let u = QuadraticExt::<Gf49Config>::new(Fp::new(0), Fp::new(1));
/// assert!(!ext.contains(&u));
/// assert_eq!(ext.try_restrict(&u), None);
/// assert_eq!(ext.relative_frobenius(&u, 2), u);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstExt<E: ConstSimpleExtension> {
    certificate: ExtensionCertificate,
    marker: PhantomData<fn() -> E>,
}

impl<E: ConstSimpleExtension> ConstExt<E> {
    /// Builds the witness for the extension `E` presents over its base.
    ///
    /// # Panics
    ///
    /// Panics if the [`ConstSimpleExtension`] implementation is inconsistent,
    /// that is, if `E`'s identity is not a tower over its base's identity.
    pub fn new() -> Self {
        let base = <E::ConstBase as ConstField>::zero().field_id();
        let ext = <E as ConstField>::zero().field_id();
        let certificate = ExtensionCertificate::from_parts(base, ext, CertificateBasis::Declared)
            .expect("a ConstSimpleExtension presents its extension as a tower over its base");
        Self {
            certificate,
            marker: PhantomData,
        }
    }

    /// Builds the witness from a certificate the caller already holds, doing
    /// no validation of its own.
    ///
    /// [`new`](Self::new) records [`CertificateBasis::Declared`], because an
    /// [`ExtConfig`] non-residue is fixed by the type rather than decided. A
    /// caller that has decided the binomial modulus — through
    /// [`prove_irreducible`], or through a verified registry — presents the
    /// resulting certificate here instead, so the witness carries the stronger
    /// basis and the structural checks are not repeated.
    ///
    /// # Contract
    ///
    /// As with [`BinaryPrimeExt::from_certificate_unchecked`], the certificate
    /// is a memo of validation performed earlier and the caller promises it
    /// memoizes the pair `E` and its base name. Only the structural identity
    /// comparison runs — linear in the modulus degree, no decision procedure;
    /// a memo of something else yields a witness over a carrier that is not
    /// a field. See `@/inv/caller-trusted-fast-paths`.
    ///
    /// # Errors
    ///
    /// [`FieldError::IdentityMismatch`] when the certificate names a different
    /// pair than `E` and its base do.
    pub fn from_certificate_unchecked(
        certificate: ExtensionCertificate,
    ) -> Result<Self, FieldError> {
        let base = <E::ConstBase as ConstField>::zero().field_id();
        if certificate.base_id() != &base {
            return Err(FieldError::IdentityMismatch {
                expected: base,
                found: certificate.base_id().clone(),
            });
        }
        let ext = <E as ConstField>::zero().field_id();
        if certificate.ext_id() != &ext {
            return Err(FieldError::IdentityMismatch {
                expected: ext,
                found: certificate.ext_id().clone(),
            });
        }
        Ok(Self {
            certificate,
            marker: PhantomData,
        })
    }
}

impl<E: ConstSimpleExtension> Default for ConstExt<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: ConstSimpleExtension> FieldExtension for ConstExt<E> {
    type Base = E::ConstBase;
    type Ext = E;

    fn certificate(&self) -> &ExtensionCertificate {
        &self.certificate
    }

    fn base_zero(&self) -> Self::Base {
        <E::ConstBase as ConstField>::zero()
    }

    fn embed(&self, x: &Self::Base) -> Self::Ext {
        E::from_base(*x)
    }

    fn try_restrict(&self, x: &Self::Ext) -> Option<Self::Base> {
        (*x).try_into_base()
    }
}

/// The trivial extension $E = B$ over any identity-carrying field.
///
/// $r = 1$ is legal and required: a splitting field can coincide with its
/// base, and then the embedding and the restriction are identities and the
/// relative Frobenius is the identity for every $k$.
///
/// # Examples
///
/// ```
/// use gf2_core::field::extension::{FieldExtension, TrivialExt};
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::gf256();
/// let ext = TrivialExt::new(field.zero());
///
/// assert_eq!(ext.relative_degree(), 1);
/// assert_eq!(ext.base_id(), ext.ext_id());
///
/// let a = field.element(0x53);
/// assert!(ext.contains(&a));
/// assert_eq!(ext.relative_frobenius(&a, 7), a);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrivialExt<F: FieldIdentity> {
    zero: F,
    certificate: ExtensionCertificate,
}

impl<F: FieldIdentity> TrivialExt<F> {
    /// Builds the trivial extension of the field `witness` belongs to.
    ///
    /// The value of `witness` is ignored; only its field matters.
    ///
    /// There is no certificate-consuming counterpart, because there is no
    /// validation to reuse: $E = B$ holds for any carrier by construction, so
    /// [`ExtensionCertificate::trivial`] decides nothing and the witness makes
    /// no claim about the irreducibility of whatever modulus `F` presents.
    pub fn new(witness: F) -> Self {
        let certificate = ExtensionCertificate::trivial(witness.field_id());
        Self {
            zero: witness.zero_like(),
            certificate,
        }
    }
}

impl<F: FieldIdentity> FieldExtension for TrivialExt<F> {
    type Base = F;
    type Ext = F;

    fn certificate(&self) -> &ExtensionCertificate {
        &self.certificate
    }

    fn base_zero(&self) -> Self::Base {
        self.zero.clone()
    }

    fn embed(&self, x: &Self::Base) -> Self::Ext {
        x.clone()
    }

    fn try_restrict(&self, x: &Self::Ext) -> Option<Self::Base> {
        Some(x.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::{Gf2mField, Gf2mWideConfig};

    // -----------------------------------------------------------------------
    // Test configurations
    // -----------------------------------------------------------------------

    /// GF(7²) as GF(7)[u]/(u² − 3).
    struct Gf49Config;

    impl ExtConfig for Gf49Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    /// GF(7³) as GF(7)[v]/(v³ − 3).
    struct Gf343Config;

    impl ExtConfig for Gf343Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    /// GF(3²) as GF(3)[u]/(u² + 1).
    struct Gf9Config;

    impl ExtConfig for Gf9Config {
        type BaseField = Fp<3>;
        const NON_RESIDUE: Fp<3> = Fp::<3>::new(2);
    }

    type Gf9 = QuadraticExt<Gf9Config>;

    /// GF(3⁴) as GF(3²)[y]/(y² − (1 + u)).
    struct Gf81Config;

    impl ExtConfig for Gf81Config {
        type BaseField = Gf9;
        const NON_RESIDUE: Gf9 = QuadraticExt::new(Fp::<3>::new(1), Fp::<3>::new(1));
    }

    /// GF(2^64) with the pentanomial `x^64 + x^4 + x^3 + x + 1`, the entry
    /// [`PrimitivePolynomialDatabase::standard_u128`] catalogues for `m = 64`.
    /// The agreement is asserted in
    /// [`wide_config_agrees_with_the_polynomial_database`].
    struct Gf2m64TestConfig;

    impl Gf2mWideConfig<1> for Gf2m64TestConfig {
        const M: usize = 64;
        const MODULUS: [u64; 1] = [0x1b];
    }

    /// GF(2^256) with the irreducible pentanomial
    /// `x^256 + x^10 + x^5 + x^2 + 1`, cited from Seroussi, "Table of
    /// Low-Weight Binary Irreducible Polynomials", HP Laboratories technical
    /// report HPL-98-135 (1998), Table 1 row m = 256 — the same entry
    /// [`crate::gf2m::Gf2mWide`] documents.
    struct Gf2m256TestConfig;

    impl Gf2mWideConfig<4> for Gf2m256TestConfig {
        const M: usize = 256;
        const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    }

    fn gf16_id() -> FieldId {
        let base = FieldId::prime(2).unwrap();
        let modulus = ModulusId::new(&base, vec![1, 1, 0, 0, 1]).unwrap();
        FieldId::quotient(base, modulus, Basis::Polynomial).unwrap()
    }

    // -----------------------------------------------------------------------
    // FieldId construction and normalization
    // -----------------------------------------------------------------------

    #[test]
    fn prime_rejects_characteristics_outside_the_representable_range() {
        for characteristic in [0, 1] {
            assert_eq!(
                FieldId::prime(characteristic),
                Err(FieldError::CharacteristicOutOfRange { characteristic })
            );
        }
        let too_large = (1u64 << 63) + 1;
        assert_eq!(
            FieldId::prime(too_large),
            Err(FieldError::CharacteristicOutOfRange {
                characteristic: too_large
            })
        );
        assert!(FieldId::prime(1u64 << 63).is_ok());
    }

    #[test]
    fn prime_identity_reports_its_structure() {
        let gf5 = FieldId::prime(5).unwrap();
        assert_eq!(gf5.characteristic(), 5);
        assert_eq!(gf5.degree(), 1);
        assert_eq!(gf5.order(), Some(5));
        assert_eq!(gf5.unit_group_order(), Some(4));
        assert_eq!(gf5.base(), None);
        assert_eq!(gf5.modulus(), None);
        assert_eq!(gf5.basis(), None);
        assert!(gf5.is_tower_over(&gf5));
    }

    #[test]
    fn degree_one_quotients_collapse_onto_their_base() {
        let gf2 = FieldId::prime(2).unwrap();
        let linear = ModulusId::new(&gf2, vec![1, 1]).unwrap();
        assert_eq!(linear.degree(), 1);
        assert_eq!(
            FieldId::quotient(gf2.clone(), linear, Basis::Polynomial).unwrap(),
            gf2
        );
    }

    #[test]
    fn quotient_identity_reports_its_structure() {
        let gf2 = FieldId::prime(2).unwrap();
        let gf16 = gf16_id();
        assert_eq!(gf16.characteristic(), 2);
        assert_eq!(gf16.degree(), 4);
        assert_eq!(gf16.order(), Some(16));
        assert_eq!(gf16.unit_group_order(), Some(15));
        assert_eq!(gf16.base(), Some(&gf2));
        assert_eq!(gf16.basis(), Some(Basis::Polynomial));
        assert_eq!(gf16.modulus().unwrap().coefficient(4), &[1]);
        assert!(gf16.is_tower_over(&gf2));
        assert!(!gf2.is_tower_over(&gf16));
    }

    #[test]
    fn presentation_identity_separates_isomorphic_fields() {
        let gf2 = FieldId::prime(2).unwrap();
        let by_first = FieldId::quotient(
            gf2.clone(),
            ModulusId::new(&gf2, vec![1, 1, 0, 0, 1]).unwrap(),
            Basis::Polynomial,
        )
        .unwrap();
        let by_second = FieldId::quotient(
            gf2.clone(),
            ModulusId::new(&gf2, vec![1, 0, 0, 1, 1]).unwrap(),
            Basis::Polynomial,
        )
        .unwrap();
        assert_eq!(by_first.order(), by_second.order());
        assert_ne!(by_first, by_second);
    }

    #[test]
    fn order_is_none_beyond_u128() {
        let id = <Gf2mWide<4, Gf2m256TestConfig> as FieldIdentity>::field_id_hint().unwrap();
        assert_eq!(id.degree(), 256);
        assert_eq!(id.order(), None);
        assert_eq!(id.unit_group_order(), None);
    }

    #[test]
    fn wide_config_agrees_with_the_polynomial_database() {
        assert_eq!(
            crate::primitive_polys::PrimitivePolynomialDatabase::standard_u128(64),
            Some((1u128 << 64) | u128::from(Gf2m64TestConfig::MODULUS[0]))
        );
    }

    #[test]
    fn coordinate_width_follows_the_characteristic() {
        assert_eq!(FieldId::prime(2).unwrap().coordinate_width(), 1);
        assert_eq!(FieldId::prime(257).unwrap().coordinate_width(), 2);
        assert_eq!(FieldId::prime(1u64 << 63).unwrap().coordinate_width(), 8);
        // A quotient inherits the width of its prime field.
        assert_eq!(gf16_id().coordinate_width(), 1);
    }

    // -----------------------------------------------------------------------
    // ModulusId validation
    // -----------------------------------------------------------------------

    #[test]
    fn modulus_rejects_a_length_that_is_not_a_multiple_of_the_base_degree() {
        let gf9 = <QuadraticExt<Gf9Config> as FieldIdentity>::field_id_hint().unwrap();
        assert_eq!(
            ModulusId::new(&gf9, vec![1, 0, 1]),
            Err(FieldError::CoordinateCountMismatch {
                expected: 4,
                found: 3
            })
        );
    }

    #[test]
    fn modulus_rejects_a_degree_zero_polynomial() {
        let gf2 = FieldId::prime(2).unwrap();
        assert_eq!(
            ModulusId::new(&gf2, vec![1]),
            Err(FieldError::ModulusDegreeTooSmall { degree: 0 })
        );
        assert_eq!(
            ModulusId::new(&gf2, vec![]),
            Err(FieldError::ModulusDegreeTooSmall { degree: 0 })
        );
    }

    #[test]
    fn modulus_rejects_a_non_monic_leading_coefficient() {
        let gf5 = FieldId::prime(5).unwrap();
        assert_eq!(
            ModulusId::new(&gf5, vec![1, 1, 2]),
            Err(FieldError::NonMonicModulus)
        );
        let gf9 = <QuadraticExt<Gf9Config> as FieldIdentity>::field_id_hint().unwrap();
        // Leading coefficient `u`, not `1`.
        assert_eq!(
            ModulusId::new(&gf9, vec![1, 0, 0, 1]),
            Err(FieldError::NonMonicModulus)
        );
    }

    #[test]
    fn modulus_rejects_out_of_range_coordinates() {
        let gf5 = FieldId::prime(5).unwrap();
        assert_eq!(
            ModulusId::new(&gf5, vec![7, 0, 1]),
            Err(FieldError::CoordinateOutOfRange {
                index: 0,
                value: 7,
                characteristic: 5
            })
        );
    }

    #[test]
    fn quotient_rejects_a_modulus_built_over_a_different_base() {
        let gf5 = FieldId::prime(5).unwrap();
        let gf9 = <QuadraticExt<Gf9Config> as FieldIdentity>::field_id_hint().unwrap();
        let over_gf5 = ModulusId::new(&gf5, vec![3, 0, 1]).unwrap();
        assert_eq!(
            FieldId::quotient(gf9, over_gf5, Basis::Polynomial),
            Err(FieldError::CoordinateCountMismatch {
                expected: 2,
                found: 1
            })
        );
    }

    #[test]
    fn quotient_rejects_a_modulus_out_of_range_for_its_new_base() {
        // Same base degree, smaller characteristic: coordinate 3 is not a
        // coordinate of GF(3).
        let gf5 = FieldId::prime(5).unwrap();
        let gf3 = FieldId::prime(3).unwrap();
        let over_gf5 = ModulusId::new(&gf5, vec![3, 0, 1]).unwrap();
        assert_eq!(
            FieldId::quotient(gf3, over_gf5, Basis::Polynomial),
            Err(FieldError::CoordinateOutOfRange {
                index: 0,
                value: 3,
                characteristic: 3
            })
        );
    }

    // -----------------------------------------------------------------------
    // Identity encoding
    // -----------------------------------------------------------------------

    #[test]
    fn encoding_round_trips_through_a_nested_tower() {
        let gf81 = <QuadraticExt<Gf81Config> as FieldIdentity>::field_id_hint().unwrap();
        assert_eq!(gf81.degree(), 4);
        assert_eq!(FieldId::decode(&gf81.encode()), Ok(gf81));
    }

    #[test]
    fn encoding_uses_one_byte_per_binary_coordinate() {
        let gf16 = gf16_id();
        let bytes = gf16.encode();
        // version + quotient tag + (prime tag + u64) + basis + u32 + 5 coefficients
        assert_eq!(bytes.len(), 1 + 1 + 9 + 1 + 4 + 5);
        assert_eq!(bytes[0], FIELD_ID_ENCODING_VERSION);
    }

    #[test]
    fn element_repr_records_the_coordinate_width_of_its_field() {
        let binary = ElementRepr::PrimeCoordsLe {
            coord_width: gf16_id().coordinate_width() as u8,
        };
        let wide_characteristic = ElementRepr::PrimeCoordsLe {
            coord_width: FieldId::prime(257).unwrap().coordinate_width() as u8,
        };
        // Representation tracks the packing, which identity deliberately omits.
        assert_ne!(binary, wide_characteristic);
    }

    #[test]
    fn decode_rejects_an_unknown_version() {
        let mut bytes = gf16_id().encode();
        bytes[0] = 0xFE;
        assert_eq!(
            FieldId::decode(&bytes),
            Err(FieldError::EncodingVersionUnsupported { found: 0xFE })
        );
    }

    #[test]
    fn decode_rejects_truncated_and_over_long_streams() {
        let bytes = gf16_id().encode();
        assert_eq!(
            FieldId::decode(&bytes[..bytes.len() - 1]),
            Err(FieldError::MalformedEncoding)
        );

        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            FieldId::decode(&trailing),
            Err(FieldError::MalformedEncoding)
        );

        assert_eq!(FieldId::decode(&[]), Err(FieldError::MalformedEncoding));
    }

    #[test]
    fn decode_rejects_unknown_tags() {
        assert_eq!(
            FieldId::decode(&[FIELD_ID_ENCODING_VERSION, 0x7F]),
            Err(FieldError::MalformedEncoding)
        );

        let mut bytes = gf16_id().encode();
        // The basis tag sits after the version, the quotient tag, and the
        // nine-byte prime node.
        bytes[11] = 0x7F;
        assert_eq!(FieldId::decode(&bytes), Err(FieldError::MalformedEncoding));
    }

    #[test]
    fn decode_rejects_a_declared_degree_larger_than_the_stream() {
        let mut bytes = gf16_id().encode();
        // Overwrite the little-endian u32 degree with a value no stream of
        // this length can carry.
        bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(FieldId::decode(&bytes), Err(FieldError::MalformedEncoding));
    }

    // -----------------------------------------------------------------------
    // Canonical coordinates
    // -----------------------------------------------------------------------

    #[test]
    fn gf2m_coordinates_are_the_bits_of_the_stored_value() {
        let field = Gf2mField::new(4, 0b10011);
        let mut coords = Vec::new();
        field.element(0b1011).write_prime_coords(&mut coords);
        assert_eq!(coords, vec![1, 1, 0, 1]);
    }

    #[test]
    fn canonical_index_of_a_gf2m_element_is_its_stored_value() {
        let field = Gf2mField::new(4, 0b10011);
        for value in 0..16u64 {
            let mut coords = Vec::new();
            field.element(value).write_prime_coords(&mut coords);
            let index = coords
                .iter()
                .enumerate()
                .fold(0u64, |acc, (k, c)| acc + c * (1u64 << k));
            assert_eq!(index, value);
        }
    }

    #[test]
    fn tower_coordinates_vary_the_base_coordinate_fastest() {
        // (c00 + c01 u) + (c10 + c11 u) y has coordinates [c00, c01, c10, c11].
        let element = QuadraticExt::<Gf81Config>::new(
            QuadraticExt::<Gf9Config>::new(Fp::new(1), Fp::new(2)),
            QuadraticExt::<Gf9Config>::new(Fp::new(0), Fp::new(1)),
        );
        let mut coords = Vec::new();
        element.write_prime_coords(&mut coords);
        assert_eq!(coords, vec![1, 2, 0, 1]);
    }

    #[test]
    fn from_prime_coords_reports_the_absolute_index_of_a_bad_coordinate() {
        let witness = <QuadraticExt<Gf9Config> as ConstField>::zero();
        assert_eq!(
            witness.from_prime_coords(&[0, 5]),
            Err(FieldError::CoordinateOutOfRange {
                index: 1,
                value: 5,
                characteristic: 3
            })
        );
        let tower = <QuadraticExt<Gf81Config> as ConstField>::zero();
        assert_eq!(
            tower.from_prime_coords(&[0, 0, 0, 9]),
            Err(FieldError::CoordinateOutOfRange {
                index: 3,
                value: 9,
                characteristic: 3
            })
        );
    }

    #[test]
    fn tower_identity_nests_one_quotient_node_per_level() {
        let gf81 = <QuadraticExt<Gf81Config> as FieldIdentity>::field_id_hint().unwrap();
        let gf9 = <QuadraticExt<Gf9Config> as FieldIdentity>::field_id_hint().unwrap();
        let gf3 = FieldId::prime(3).unwrap();

        assert_eq!(gf81.base(), Some(&gf9));
        assert_eq!(gf9.base(), Some(&gf3));
        assert!(gf81.is_tower_over(&gf3));
        assert_eq!(gf81.degree(), 4);
        assert_eq!(gf81.order(), Some(81));
    }

    #[test]
    fn binomial_identity_records_the_non_residue_as_the_modulus() {
        // x^2 - 3 over GF(7) is x^2 + 4.
        let gf49 = <QuadraticExt<Gf49Config> as FieldIdentity>::field_id_hint().unwrap();
        assert_eq!(gf49.modulus().unwrap().coefficients(), &[4, 0, 1]);
        assert_eq!(
            <QuadraticExt<Gf49Config> as ConstSimpleExtension>::modulus_id().coefficients(),
            &[4, 0, 1]
        );

        // x^3 - 3 over GF(7) is x^3 + 4.
        let gf343 = <CubicExt<Gf343Config> as FieldIdentity>::field_id_hint().unwrap();
        assert_eq!(gf343.modulus().unwrap().coefficients(), &[4, 0, 0, 1]);
    }

    // -----------------------------------------------------------------------
    // Conversion between carriers
    // -----------------------------------------------------------------------

    #[test]
    fn convert_element_rejects_a_different_presentation() {
        let narrow = Gf2mField::gf256();
        let other = Gf2mField::new(8, 0b100011011);
        let error = convert_element(&narrow.element(0x53), &other.zero()).unwrap_err();
        assert!(matches!(error, FieldError::IdentityMismatch { .. }));
    }

    #[test]
    fn convert_into_const_moves_a_runtime_carrier_into_a_compile_time_one() {
        // The runtime carrier stores the leading term explicitly; the wide
        // configuration leaves it implicit. Both name the same presentation.
        let runtime = Gf2mField_::<u128>::new(64, (1u128 << 64) | 0x1b);
        let wide_id = <Gf2mWide<1, Gf2m64TestConfig> as FieldIdentity>::field_id_hint().unwrap();
        assert_eq!(runtime.zero().field_id(), wide_id);

        let converted: Gf2mWide<1, Gf2m64TestConfig> =
            convert_into_const(&runtime.element(0b1011)).unwrap();
        assert_eq!(converted.words(), &[0b1011]);

        // Transporting back reproduces the original element.
        let back: Gf2mElement_<u128> = convert_element(&converted, &runtime.zero()).unwrap();
        assert_eq!(back, runtime.element(0b1011));
    }

    // -----------------------------------------------------------------------
    // Certificates
    // -----------------------------------------------------------------------

    #[test]
    fn trivial_certificate_records_a_relative_degree_of_one() {
        let gf16 = gf16_id();
        let certificate = ExtensionCertificate::trivial(gf16.clone());
        assert_eq!(certificate.relative_degree(), 1);
        assert_eq!(certificate.basis(), CertificateBasis::Identity);
        assert!(certificate.matches(&gf16, &gf16));
    }

    #[test]
    fn from_parts_rejects_a_characteristic_mismatch() {
        let gf3 = FieldId::prime(3).unwrap();
        let gf16 = gf16_id();
        assert_eq!(
            ExtensionCertificate::from_parts(gf3, gf16, CertificateBasis::Proved),
            Err(FieldError::CharacteristicMismatch { base: 3, ext: 2 })
        );
    }

    #[test]
    fn from_parts_rejects_a_pair_that_is_not_a_tower() {
        let gf2 = FieldId::prime(2).unwrap();
        let gf16 = gf16_id();
        let other = FieldId::quotient(
            gf2,
            ModulusId::new(&FieldId::prime(2).unwrap(), vec![1, 0, 0, 1, 1]).unwrap(),
            Basis::Polynomial,
        )
        .unwrap();
        assert!(matches!(
            ExtensionCertificate::from_parts(other, gf16, CertificateBasis::Proved),
            Err(FieldError::NotATower { .. })
        ));
    }

    #[test]
    fn certificates_are_reused_by_comparison() {
        let gf2 = FieldId::prime(2).unwrap();
        let gf16 = gf16_id();
        let certificate =
            ExtensionCertificate::from_parts(gf2.clone(), gf16.clone(), CertificateBasis::Registry)
                .unwrap();

        // Cloning shares the allocation, and the clone still covers the pair.
        let held = certificate.clone();
        assert_eq!(held, certificate);
        assert!(held.matches(&gf2, &gf16));
        assert_eq!(held.basis(), CertificateBasis::Registry);

        // A construction over an unrelated pair does not reuse it.
        assert!(!held.matches(&gf16, &gf2));
    }

    #[test]
    fn a_held_certificate_covers_a_second_construction_of_the_same_pair() {
        let first = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap();
        let held = first.certificate().clone();

        // A second witness over the same presentation, built independently, is
        // already covered: reuse is a comparison, not a revalidation.
        let second = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap();
        assert!(held.matches(second.base_id(), second.ext_id()));
        assert_eq!(&held, second.certificate());

        // A different presentation of a field of the same size is not.
        let other = BinaryPrimeExt::new(Gf2mField::new(4, 0b11001)).unwrap();
        assert!(!held.matches(other.base_id(), other.ext_id()));
    }

    #[test]
    fn order_certificate_validates_its_factorization() {
        let gf16 = gf16_id();
        assert!(OrderCertificate::new(gf16.clone(), 15, vec![3, 5]).is_ok());

        // Incomplete: 5 is missing.
        assert_eq!(
            OrderCertificate::new(gf16.clone(), 15, vec![3]),
            Err(FieldError::OrderFactorizationUnavailable { order: 15 })
        );
        // Out of order.
        assert_eq!(
            OrderCertificate::new(gf16.clone(), 15, vec![5, 3]),
            Err(FieldError::OrderFactorizationUnavailable { order: 15 })
        );
        // Not a divisor of |E*| = 15.
        assert_eq!(
            OrderCertificate::new(gf16, 4, vec![2]),
            Err(FieldError::NoElementOfOrder {
                requested: 4,
                unit_group_order: 15
            })
        );
    }

    #[test]
    fn order_certificate_divisor_filters_the_stored_factorization() {
        let gf16 = gf16_id();
        let certificate = OrderCertificate::new(gf16.clone(), 15, vec![3, 5]).unwrap();
        assert_eq!(certificate.field_id(), &gf16);

        let three = certificate.divisor(3).unwrap();
        assert_eq!(three.order(), 3);
        assert_eq!(three.prime_factors(), &[3]);

        let one = certificate.divisor(1).unwrap();
        assert_eq!(one.order(), 1);
        assert!(one.prime_factors().is_empty());

        assert_eq!(certificate.divisor(15).unwrap(), certificate);
        assert!(certificate.divisor(2).is_none());
        assert!(certificate.divisor(0).is_none());
    }

    // -----------------------------------------------------------------------
    // Exact multiplicative orders
    // -----------------------------------------------------------------------

    #[test]
    fn exact_order_derivation_validates_order_exactness() {
        let field = Gf2mField::new(4, 0b10011);
        let ext = BinaryPrimeExt::new(field.clone()).unwrap();

        let (element, certificate) = element_of_exact_order(&ext, 5).unwrap();
        assert_eq!(certificate.order(), 5);
        assert_eq!(certificate.prime_factors(), &[5]);
        assert!(element.pow(5).is_one());
        assert!(!element.pow(5 / 5).is_one());

        // The full-order helper uses the same exact-order validation and
        // agrees with the existing table-backed primitive for this field.
        let (generator, certificate) = canonical_generator(&ext).unwrap();
        assert_eq!(generator, field.element(2));
        assert_eq!(certificate.order(), 15);
        assert!(generator.pow(15).is_one());
        for &prime in certificate.prime_factors() {
            assert!(!generator.pow(15 / prime).is_one());
        }
    }

    proptest::proptest! {
        #[test]
        fn prop_exact_order_binary(n in proptest::sample::select(vec![1u64, 3, 5, 15])) {
            let field = Gf2mField::new(4, 0b10011);
            let ext = BinaryPrimeExt::new(field).unwrap();
            let (element, certificate) = element_of_exact_order(&ext, n).unwrap();

            proptest::prop_assert_eq!(certificate.order(), n);
            proptest::prop_assert!(element.pow(n).is_one());
            for &prime in certificate.prime_factors() {
                proptest::prop_assert!(!element.pow(n / prime).is_one());
            }

            let (again, again_certificate) = element_of_exact_order(&ext, n).unwrap();
            proptest::prop_assert_eq!(again, element);
            proptest::prop_assert_eq!(again_certificate, certificate);
        }

        #[test]
        fn prop_exact_order_nonbinary(n in proptest::sample::select(
            vec![1u64, 2, 3, 4, 6, 8, 12, 16, 24, 48]
        )) {
            let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
            let (element, certificate) = element_of_exact_order(&ext, n).unwrap();

            proptest::prop_assert_eq!(certificate.order(), n);
            proptest::prop_assert!(element.pow(n).is_one());
            for &prime in certificate.prime_factors() {
                proptest::prop_assert!(!element.pow(n / prime).is_one());
            }

            let (again, again_certificate) = element_of_exact_order(&ext, n).unwrap();
            proptest::prop_assert_eq!(again, element);
            proptest::prop_assert_eq!(again_certificate, certificate);
        }
    }

    #[test]
    fn exact_order_selection_is_deterministic_across_carriers() {
        let narrow = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap();
        let wide = BinaryPrimeExt::new(Gf2mField_::<u128>::new(4, 0b10011u128)).unwrap();

        let (narrow_element, narrow_certificate) = element_of_exact_order(&narrow, 5).unwrap();
        let (wide_element, wide_certificate) = element_of_exact_order(&wide, 5).unwrap();
        let mut narrow_coords = Vec::new();
        let mut wide_coords = Vec::new();
        narrow_element.write_prime_coords(&mut narrow_coords);
        wide_element.write_prime_coords(&mut wide_coords);

        assert_eq!(narrow_coords, wide_coords);
        assert_eq!(narrow_certificate, wide_certificate);

        let (again, again_certificate) = element_of_exact_order(&narrow, 5).unwrap();
        assert_eq!(again, narrow_element);
        assert_eq!(again_certificate, narrow_certificate);

        // The same rule is deterministic for a nonbinary carrier as well.
        let first = ConstExt::<QuadraticExt<Gf49Config>>::new();
        let second = TrivialExt::new(QuadraticExt::<Gf49Config>::new(Fp::new(0), Fp::new(0)));
        let (first_element, first_certificate) = element_of_exact_order(&first, 8).unwrap();
        let (second_element, second_certificate) = element_of_exact_order(&second, 8).unwrap();
        assert_eq!(first_element, second_element);
        assert_eq!(first_certificate, second_certificate);
    }

    #[test]
    fn exact_order_supports_the_u64_unit_group_boundary() {
        let ext = TrivialExt::new(Gf2mWide::<1, Gf2m64TestConfig>::zero());
        let (generator, certificate) = canonical_generator(&ext).unwrap();

        assert_eq!(certificate.order(), u64::MAX);
        assert_eq!(generator, Gf2mWide::<1, Gf2m64TestConfig>::from_u64(2));
        assert!(generator.pow(u64::MAX).is_one());
        for &prime in certificate.prime_factors() {
            assert!(!generator.pow(u64::MAX / prime).is_one());
        }
    }

    #[test]
    fn exact_order_reports_no_such_order() {
        let ext = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap();
        assert_eq!(
            element_of_exact_order(&ext, 2),
            Err(FieldError::NoElementOfOrder {
                requested: 2,
                unit_group_order: 15,
            })
        );
    }

    #[test]
    fn exact_order_reports_factorization_unavailable() {
        struct Gf2m128TestConfig;

        impl Gf2mWideConfig<2> for Gf2m128TestConfig {
            const M: usize = 128;
            const MODULUS: [u64; 2] = [0x87, 0];
        }

        let ext = TrivialExt::new(Gf2mWide::<2, Gf2m128TestConfig>::zero());
        assert_eq!(
            canonical_generator(&ext),
            Err(FieldError::OrderFactorizationUnavailable { order: u128::MAX })
        );
        assert_eq!(
            element_of_exact_order(&ext, 1),
            Err(FieldError::OrderFactorizationUnavailable { order: u128::MAX })
        );
    }

    // -----------------------------------------------------------------------
    // Relative-field derived operations
    // -----------------------------------------------------------------------

    fn assert_coset_partition_properties(partition: &CosetPartition, full: bool) {
        let mut seen = Vec::new();
        for pair in partition.cosets().windows(2) {
            assert!(pair[0][0] < pair[1][0]);
        }

        for coset in partition.cosets() {
            assert!(!coset.is_empty());
            let representative = *coset.iter().min().unwrap();
            assert_eq!(coset[0], representative);

            for (index, &member) in coset.iter().enumerate() {
                assert!(member < partition.n);
                assert!(!seen.contains(&member), "cosets are not disjoint");
                seen.push(member);
                let successor = modular_mul(member, partition.q_mod_n, partition.n);
                assert_eq!(successor, coset[(index + 1) % coset.len()]);
            }
        }

        if full {
            assert_eq!(seen.len(), partition.n as usize);
            seen.sort_unstable();
            assert_eq!(seen, (0..partition.n).collect::<Vec<_>>());
        }
    }

    fn iterative_closure(q_mod_n: u64, n: u64, seeds: &[u64]) -> Vec<u64> {
        let mut closure = seeds.iter().map(|&seed| seed % n).collect::<Vec<_>>();
        closure.sort_unstable();
        closure.dedup();

        let mut index = 0;
        while index < closure.len() {
            let successor = modular_mul(closure[index], q_mod_n, n);
            if !closure.contains(&successor) {
                closure.push(successor);
            }
            index += 1;
        }
        closure.sort_unstable();
        closure
    }

    #[test]
    fn cyclotomic_cosets_match_the_worked_binary_vector_and_pure_form() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let ext = BinaryPrimeExt::new(field).unwrap();
        let from_extension = cyclotomic_cosets(&ext, 15).unwrap();
        let from_modulus = cyclotomic_cosets_mod(2, 15).unwrap();

        assert_eq!(from_extension, from_modulus);
        assert_eq!(
            from_extension.cosets(),
            &[
                vec![0],
                vec![1, 2, 4, 8],
                vec![3, 6, 12, 9],
                vec![5, 10],
                vec![7, 14, 13, 11],
            ]
        );
        assert_coset_partition_properties(&from_extension, true);
    }

    #[test]
    fn cyclotomic_closure_selects_complete_seed_cosets() {
        let partition = cyclotomic_cosets_mod(2, 15).unwrap();
        let closure = cyclotomic_closure(
            &BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap(),
            15,
            &[3, 5, 20],
        )
        .unwrap();

        assert_eq!(closure.cosets(), &[vec![3, 6, 12, 9], vec![5, 10]]);
        assert_eq!(closure.defining_set(), vec![3, 5, 6, 9, 10, 12]);
        assert_eq!(closure.coset_of(18), Some(0));
        assert_eq!(closure.coset_of(1), None);
        assert_eq!(partition.defining_set(), (0..15).collect::<Vec<_>>());
    }

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(64))]

        #[test]
        fn prop_binary_cyclotomic_closure_laws(
            n in proptest::sample::select(vec![1u64, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 25, 27, 31]),
            seeds in proptest::collection::vec(0u64..64, 0..=8),
        ) {
            let field = Gf2mField::new(4, 0b10011);
            let ext = BinaryPrimeExt::new(field).unwrap();
            let full = cyclotomic_cosets(&ext, n).unwrap();
            let closure = cyclotomic_closure(&ext, n, &seeds).unwrap();
            let closed_set = closure.defining_set();
            let again = cyclotomic_closure(&ext, n, &closed_set).unwrap();

            assert_coset_partition_properties(&full, true);
            assert_coset_partition_properties(&closure, false);
            proptest::prop_assert_eq!(&closure, &again);
            proptest::prop_assert_eq!(
                closure.defining_set(),
                iterative_closure(full.q_mod_n, n, &seeds),
            );
        }

        #[test]
        fn prop_nonbinary_cyclotomic_closure_laws(
            n in proptest::sample::select(vec![1u64, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 15, 16, 17, 18, 20, 24, 30, 31, 32, 40, 48]),
            seeds in proptest::collection::vec(0u64..96, 0..=8),
        ) {
            let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
            let full = cyclotomic_cosets(&ext, n).unwrap();
            let closure = cyclotomic_closure(&ext, n, &seeds).unwrap();
            let closed_set = closure.defining_set();
            let again = cyclotomic_closure(&ext, n, &closed_set).unwrap();

            assert_coset_partition_properties(&full, true);
            assert_coset_partition_properties(&closure, false);
            proptest::prop_assert_eq!(&closure, &again);
            proptest::prop_assert_eq!(
                closure.defining_set(),
                iterative_closure(full.q_mod_n, n, &seeds),
            );
        }
    }

    #[test]
    fn cyclotomic_cosets_reject_invalid_moduli_and_non_coprime_parameters() {
        assert_eq!(
            cyclotomic_cosets_mod(2, 0),
            Err(FieldError::InvalidCyclotomicModulus { modulus: 0 })
        );
        assert_eq!(
            cyclotomic_cosets_mod(2, 4),
            Err(FieldError::NonCoprimeCyclotomicParameters {
                q_mod_n: 2,
                modulus: 4,
                gcd: 2,
            })
        );

        // A valid coprime modulus whose visited table can never be allocated
        // reports the typed too-large error instead of aborting.
        assert_eq!(
            cyclotomic_cosets_mod(2, u64::MAX),
            Err(FieldError::CyclotomicModulusTooLarge { modulus: u64::MAX })
        );
    }

    #[test]
    fn cyclotomic_coset_order_is_deterministic() {
        let first = cyclotomic_cosets_mod(17, 15).unwrap();
        let second = cyclotomic_cosets_mod(2, 15).unwrap();
        assert_eq!(first, second);

        let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
        let from_extension = cyclotomic_cosets(&ext, 48).unwrap();
        let from_modulus = cyclotomic_cosets_mod(7, 48).unwrap();
        assert_eq!(from_extension, from_modulus);
        assert_eq!(from_extension, cyclotomic_cosets(&ext, 48).unwrap());
    }

    fn assert_minimal_polynomial_properties<X: FieldExtension>(ext: &X, x: &X::Ext) {
        let orbit = conjugates(ext, x);
        let polynomial = minimal_polynomial(ext, x).unwrap();
        let degree = polynomial.degree().unwrap();

        // The orbit-size characterization is the minimality check: a proper
        // base-field divisor would have smaller degree and still vanish at x.
        assert_eq!(degree, orbit.len());
        assert!(polynomial.leading_coeff().unwrap().is_one());

        let embedded: FieldPoly<X::Ext> = FieldPoly::new(
            (0..=degree)
                .map(|index| ext.embed(&polynomial.coeff(index)))
                .collect(),
        );
        assert!(embedded.eval(x).is_zero());
        for conjugate in orbit {
            assert!(embedded.eval(&conjugate).is_zero());
        }

        for index in 0..=degree {
            let coefficient = polynomial.coeff(index);
            assert_eq!(
                ext.try_restrict(&ext.embed(&coefficient)),
                Some(coefficient)
            );
        }
    }

    fn assert_trace_norm_laws<X: FieldExtension>(ext: &X, x: &X::Ext, y: &X::Ext) {
        let trace_x = relative_trace(ext, x).unwrap();
        let trace_y = relative_trace(ext, y).unwrap();
        let norm_x = relative_norm(ext, x).unwrap();
        let norm_y = relative_norm(ext, y).unwrap();

        assert_eq!(
            ext.try_restrict(&ext.embed(&trace_x)),
            Some(trace_x.clone())
        );
        assert_eq!(
            ext.try_restrict(&ext.embed(&trace_y)),
            Some(trace_y.clone())
        );
        assert_eq!(
            relative_trace(ext, &(x.clone() + y.clone())).unwrap(),
            trace_x + trace_y
        );
        assert_eq!(
            relative_norm(ext, &(x.clone() * y.clone())).unwrap(),
            norm_x * norm_y
        );
    }

    fn gf81_element(value: u64) -> Gf81 {
        let lower = value % 9;
        let upper = value / 9;
        let lower = Gf9::new(Fp::new(lower % 3), Fp::new(lower / 3));
        let upper = Gf9::new(Fp::new(upper % 3), Fp::new(upper / 3));
        Gf81::new(lower, upper)
    }

    type Gf81 = QuadraticExt<Gf81Config>;

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(64))]

        #[test]
        fn prop_binary_minimal_polynomial_and_relative_laws(
            value in 0u64..16,
            other in 0u64..16,
        ) {
            let field = Gf2mField::new(4, 0b10011);
            let ext = BinaryPrimeExt::new(field.clone()).unwrap();
            let x = field.element(value);
            let y = field.element(other);

            assert_minimal_polynomial_properties(&ext, &x);
            assert_trace_norm_laws(&ext, &x, &y);

            let generic = minimal_polynomial(&ext, &x).unwrap();
            let legacy = x.minimal_polynomial();
            proptest::prop_assert_eq!(generic.degree(), legacy.degree());
            for index in 0..=generic.degree().unwrap() {
                proptest::prop_assert_eq!(
                    ext.embed(&generic.coeff(index)),
                    legacy.coeff(index)
                );
            }
        }

        #[test]
        fn prop_odd_prime_minimal_polynomial_and_relative_laws(
            value in 0u64..49,
            other in 0u64..49,
        ) {
            let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
            let x = QuadraticExt::new(Fp::new(value % 7), Fp::new(value / 7));
            let y = QuadraticExt::new(Fp::new(other % 7), Fp::new(other / 7));

            assert_minimal_polynomial_properties(&ext, &x);
            assert_trace_norm_laws(&ext, &x, &y);
        }

        #[test]
        fn prop_odd_tower_minimal_polynomial(
            value in 0u64..81,
        ) {
            let ext = ConstExt::<Gf81>::new();
            let x = gf81_element(value);

            assert_minimal_polynomial_properties(&ext, &x);
        }
    }

    #[test]
    fn derived_operations_reject_an_element_from_another_extension() {
        let ext = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap();
        let foreign = Gf2mField::new(4, 0b11001).element(2);

        assert!(matches!(
            minimal_polynomial(&ext, &foreign),
            Err(FieldError::IdentityMismatch { .. })
        ));
        assert!(matches!(
            relative_trace(&ext, &foreign),
            Err(FieldError::IdentityMismatch { .. })
        ));
        assert!(matches!(
            relative_norm(&ext, &foreign),
            Err(FieldError::IdentityMismatch { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // Witnesses
    // -----------------------------------------------------------------------

    #[test]
    fn binary_prime_ext_reports_the_worked_example() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let ext = BinaryPrimeExt::new(field.clone()).unwrap();

        assert_eq!(ext.base_id(), &FieldId::prime(2).unwrap());
        assert_eq!(ext.ext_id(), &gf16_id());
        assert_eq!(
            (ext.base_degree(), ext.ext_degree(), ext.relative_degree()),
            (1, 4, 4)
        );
        assert_eq!(
            (
                ext.base_order(),
                ext.ext_order(),
                ext.ext_unit_group_order()
            ),
            (Some(2), Some(16), Some(15))
        );
        assert_eq!(ext.certificate().basis(), CertificateBasis::Proved);
        assert_eq!(ext.field(), &field);

        assert_eq!(ext.embed(&Fp::<2>::new(1)), field.one());
        assert_eq!(ext.embed(&Fp::<2>::new(0)), field.zero());
        for value in 0..16u64 {
            let element = field.element(value);
            assert_eq!(ext.contains(&element), value <= 1);
        }
        // The relative Frobenius is squaring, and k is taken modulo 4.
        let a = field.element(0b1011);
        assert_eq!(ext.relative_frobenius(&a, 1), a.clone() * a.clone());
        assert_eq!(ext.relative_frobenius(&a, 5), ext.relative_frobenius(&a, 1));
    }

    #[test]
    fn binary_prime_ext_rejects_a_reducible_runtime_modulus() {
        // x^4 + 1 = (x + 1)^4 over GF(2): monic of degree four, so it passes
        // the identity checks, but it has a base-field root.
        assert_eq!(
            BinaryPrimeExt::new(Gf2mField::new(4, 0b10001)),
            Err(FieldError::ReducibleModulus {
                witness: FactorWitness::BaseFieldRoot,
            })
        );

        // x^4 + x^2 + 1 = (x^2 + x + 1)^2 has no root but is still reducible.
        assert!(matches!(
            BinaryPrimeExt::new(Gf2mField::new(4, 0b10101)),
            Err(FieldError::ReducibleModulus { .. })
        ));
    }

    #[test]
    fn binary_prime_ext_records_proved_evidence() {
        let ext = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap();
        assert_eq!(ext.certificate().basis(), CertificateBasis::Proved);
        assert_eq!(ext.ext_id(), &gf16_id());
    }

    #[test]
    fn from_certificate_unchecked_skips_the_decision() {
        let field = Gf2mField::new(4, 0b10011);
        let memo = BinaryPrimeExt::new(field.clone())
            .unwrap()
            .certificate()
            .clone();

        let reused = BinaryPrimeExt::from_certificate_unchecked(field.clone(), memo).unwrap();
        assert_eq!(reused, BinaryPrimeExt::new(field).unwrap());
    }

    #[test]
    fn from_certificate_unchecked_round_trips_every_certificate_new_produces() {
        // `new` records Proved for a genuine extension and Identity for the
        // m = 1 collapse; the memo path has to accept both, or it cannot
        // serve the constructor whose cost it exists to amortise.
        for field in [Gf2mField::new(4, 0b10011), Gf2mField::new(1, 0b11)] {
            let decided = BinaryPrimeExt::new(field.clone()).unwrap();
            let reused =
                BinaryPrimeExt::from_certificate_unchecked(field, decided.certificate().clone())
                    .unwrap();
            assert_eq!(reused, decided);
        }
    }

    #[test]
    fn from_certificate_unchecked_rejects_a_certificate_for_another_pair() {
        let memo = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011))
            .unwrap()
            .certificate()
            .clone();

        // Same size, different presentation: an honest mix-up the identity
        // match catches without any arithmetic.
        assert!(matches!(
            BinaryPrimeExt::from_certificate_unchecked(Gf2mField::new(4, 0b11001), memo),
            Err(FieldError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn const_ext_from_certificate_unchecked_reuses_the_memo() {
        let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
        let reused = ConstExt::<QuadraticExt<Gf49Config>>::from_certificate_unchecked(
            ext.certificate().clone(),
        )
        .unwrap();
        assert_eq!(reused, ext);
    }

    #[test]
    fn const_ext_from_certificate_unchecked_rejects_a_certificate_for_another_pair() {
        let foreign = ConstExt::<CubicExt<Gf343Config>>::new()
            .certificate()
            .clone();
        assert!(matches!(
            ConstExt::<QuadraticExt<Gf49Config>>::from_certificate_unchecked(foreign),
            Err(FieldError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn binary_prime_ext_rejects_a_polynomial_that_is_not_monic_of_degree_m() {
        // Degree-4 field with a polynomial of degree 1: bit 4 is clear.
        let field = Gf2mField::new(4, 0b00011);
        assert_eq!(BinaryPrimeExt::new(field), Err(FieldError::NonMonicModulus));
    }

    #[test]
    fn binary_prime_ext_over_a_degree_one_field_is_the_trivial_extension() {
        let ext = BinaryPrimeExt::new(Gf2mField::new(1, 0b11)).unwrap();
        assert_eq!(ext.base_id(), ext.ext_id());
        assert_eq!(ext.relative_degree(), 1);
        assert_eq!(ext.certificate().basis(), CertificateBasis::Identity);
    }

    #[test]
    fn const_ext_forwards_to_the_inherent_binomial_embedding() {
        let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
        let a = Fp::<7>::new(5);
        assert_eq!(ext.embed(&a), QuadraticExt::<Gf49Config>::from_base(a));
        assert_eq!(ext.try_restrict(&ext.embed(&a)), Some(a));

        let u = QuadraticExt::<Gf49Config>::new(Fp::new(0), Fp::new(1));
        assert_eq!(ext.try_restrict(&u), None);
        assert_eq!(ext.restrict(&u), Err(FieldError::NotInBase));
        assert_eq!(ConstExt::<QuadraticExt<Gf49Config>>::default(), ext);
    }

    #[test]
    fn const_ext_relative_frobenius_agrees_with_the_conjugate() {
        // Over a quadratic extension the relative Frobenius is the conjugation
        // the carrier already implements.
        let ext = ConstExt::<QuadraticExt<Gf49Config>>::new();
        for c0 in 0..7u64 {
            for c1 in 0..7u64 {
                let x = QuadraticExt::<Gf49Config>::new(Fp::new(c0), Fp::new(c1));
                assert_eq!(ext.relative_frobenius(&x, 1), x.conjugate());
            }
        }
    }

    #[test]
    fn tower_relative_frobenius_takes_two_absolute_steps() {
        // GF(3^2) subset GF(3^4): phi_B is x -> x^9, not x -> x^3.
        let ext = ConstExt::<QuadraticExt<Gf81Config>>::new();
        let y = QuadraticExt::<Gf81Config>::new(
            QuadraticExt::<Gf9Config>::new(Fp::new(1), Fp::new(0)),
            QuadraticExt::<Gf9Config>::new(Fp::new(1), Fp::new(0)),
        );
        assert_eq!(ext.relative_frobenius(&y, 1), y.pow(9));
        assert_ne!(ext.relative_frobenius(&y, 1), y.pow(3));
    }

    #[test]
    fn trivial_ext_is_the_identity_on_every_operation() {
        let field = Gf2mField::gf256();
        let ext = TrivialExt::new(field.element(0x53));
        let a = field.element(0xCA);

        assert_eq!(ext.base_zero(), field.zero());
        assert_eq!(ext.embed(&a), a);
        assert_eq!(ext.try_restrict(&a), Some(a.clone()));
        for k in 0..4u32 {
            assert_eq!(ext.relative_frobenius(&a, k), a);
        }
    }

    // -----------------------------------------------------------------------
    // Errors
    // -----------------------------------------------------------------------

    #[test]
    fn errors_render_their_distinguishing_data() {
        let rendered = FieldError::CoordinateOutOfRange {
            index: 2,
            value: 9,
            characteristic: 5,
        }
        .to_string();
        assert!(rendered.contains('2') && rendered.contains('9') && rendered.contains('5'));

        let reducible = FieldError::ReducibleModulus {
            witness: FactorWitness::DistinctDegreeSplit { degree: 3 },
        }
        .to_string();
        assert!(reducible.contains('3'));
    }
}
