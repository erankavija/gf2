//! Deterministic selection of irreducible extension-field moduli.
//!
//! A selected modulus is a [`FieldPoly`] whose coefficients belong to the
//! supplied field witness.  Consequently, selecting over a prime field and
//! selecting over a tower field produce different, relative presentations;
//! the latter is never flattened to a polynomial over the prime field.
//!
//! # Selection rule
//!
//! [`select_modulus`] first asks the built-in registry for a Conway entry and
//! then for another entry carrying the registry's verification evidence.  A
//! registry entry is accepted as-is: its provenance is the same kind of
//! evidence documented by [`crate::primitive_polys`].  The binary adapter is
//! the existing `u64`/`u128` registry, so its public accessors remain the
//! source-compatible way to inspect those entries.
//!
//! If no registry entry applies, the selector searches the complete finite
//! candidate range in this exact order.  For a base field of order `q` and a
//! requested degree `r`, candidate rank `s` is written in base `q` as
//! `s = c_0 + c_1 q + ... + c_(r-1) q^(r-1)`.  The candidate is
//! `x^r + c_(r-1)x^(r-1) + ... + c_1x + c_0`, so the constant coefficient
//! varies fastest.  Each coefficient's rank is its canonical index, whose
//! prime-field digits are in the coordinate order defined by [`FieldId`].
//! Ranks start at zero and increase by one.  The first candidate for which
//! [`prove_irreducible`] succeeds is returned.  There is no randomness,
//! parallel iteration, or unordered collection in this path.
//!
//! This order is part of the field identity contract: changing it changes
//! the presentation selected for a serialized field.  The search range is
//! represented by `u128`; requests whose range or coefficient materialization
//! cannot be represented return a typed error.

use std::fmt;

use crate::field::extension::{FieldError, FieldId, FieldIdentity, ModulusId};
use crate::field::irreducibility::prove_irreducible;
use crate::field::FieldPoly;

/// The provenance class of a registry modulus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegistryProvenance {
    /// A canonical Conway polynomial from the registry's cited source.
    Conway,
    /// A registry polynomial with the primitive guarantee documented by its
    /// source and the adapter.
    VerifiedPrimitive,
    /// A registry polynomial with an irreducibility guarantee, but no
    /// independently established primitivity guarantee.
    VerifiedIrreducible,
}

/// A monic polynomial encoded in canonical coordinates for a registry
/// adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    coefficients: Vec<u64>,
    provenance: RegistryProvenance,
}

impl RegistryEntry {
    /// Creates an adapter entry.
    ///
    /// `coefficients` are flattened in the same order accepted by
    /// [`ModulusId::new`]: coefficient zero first, with the base field's
    /// canonical coordinates varying fastest.  The selector validates the
    /// degree, coordinate range, and monicity against the requested base.
    pub fn new(coefficients: Vec<u64>, provenance: RegistryProvenance) -> Self {
        Self {
            coefficients,
            provenance,
        }
    }

    /// Returns the flattened canonical coefficient coordinates.
    pub fn coefficients(&self) -> &[u64] {
        &self.coefficients
    }

    /// Returns the registry evidence class.
    pub fn provenance(&self) -> RegistryProvenance {
        self.provenance
    }
}

/// A source of deterministic, pre-verified modulus entries.
///
/// `conway` is queried before `verified`.  An implementation with several
/// entries for one `(base, degree)` pair must return the canonical Conway
/// entry from `conway`; the selector then never lets a non-Conway entry shadow
/// it.  Both methods default to no entry, making a registry-less search easy
/// to use in tests and in future adapters.
pub trait ModulusRegistry {
    /// Returns the canonical Conway entry for `(base, degree)`, if present.
    fn conway(&self, _base: &FieldId, _degree: usize) -> Option<RegistryEntry> {
        None
    }

    /// Returns another entry whose verification evidence is documented by the
    /// adapter, if present.
    fn verified(&self, _base: &FieldId, _degree: usize) -> Option<RegistryEntry> {
        None
    }
}

/// An empty registry, useful for explicitly requesting the deterministic
/// search path.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyRegistry;

impl ModulusRegistry for EmptyRegistry {}

/// Distinguishable failures from deterministic modulus selection.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModulusSelectionError {
    /// The supplied carrier does not describe a valid prime-characteristic
    /// finite field identity.
    InvalidBase {
        /// Identity reported by the carrier.
        base: FieldId,
        /// Why the identity cannot be used for selection.
        reason: BaseValidationError,
    },
    /// Degree zero has no monic extension modulus candidate.
    NoCandidates {
        /// Identity of the coefficient field.
        base: FieldId,
        /// Requested polynomial degree.
        degree: usize,
    },
    /// The candidate range or the requested coefficients cannot be represented
    /// by the deterministic implementation.
    UnsupportedSize {
        /// Identity of the coefficient field.
        base: FieldId,
        /// Requested polynomial degree.
        degree: usize,
    },
    /// A registry entry does not satisfy the structural contract for the
    /// requested base and degree.
    InvalidRegistryEntry {
        /// Identity of the coefficient field.
        base: FieldId,
        /// Requested polynomial degree.
        degree: usize,
        /// Evidence class supplied by the adapter.
        provenance: RegistryProvenance,
        /// Structural validation failure.
        error: FieldError,
    },
    /// A structurally valid registry coefficient could not be rebuilt in the
    /// supplied carrier.
    RegistryMaterialization {
        /// Identity of the coefficient field.
        base: FieldId,
        /// Requested polynomial degree.
        degree: usize,
        /// Coefficient index that failed to materialize.
        coefficient: usize,
        /// Carrier conversion failure.
        error: FieldError,
    },
    /// A fallback candidate could not be rebuilt in the supplied carrier.
    CandidateMaterialization {
        /// Identity of the coefficient field.
        base: FieldId,
        /// Requested polynomial degree.
        degree: usize,
        /// Coefficient index that failed to materialize.
        coefficient: usize,
        /// Carrier conversion failure.
        error: FieldError,
    },
    /// A non-reducibility error occurred while proving a fallback candidate.
    Verification {
        /// Identity of the coefficient field.
        base: FieldId,
        /// Requested polynomial degree.
        degree: usize,
        /// Rank of the candidate in the specified search order.
        rank: u128,
        /// Error returned by the complete irreducibility decision.
        error: FieldError,
    },
}

/// Reasons a coefficient carrier is rejected before candidate enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BaseValidationError {
    /// The characteristic is composite; `FieldId` itself deliberately leaves
    /// primality to its caller, while selection requires an actual field.
    CompositeCharacteristic,
    /// The carrier's characteristic disagrees with its identity.
    CharacteristicMismatch,
    /// The carrier's extension degree disagrees with its identity.
    DegreeMismatch,
    /// The identity's cardinality is not representable by the selector.
    CardinalityUnavailable,
}

impl fmt::Display for BaseValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompositeCharacteristic => f.write_str("base characteristic is composite"),
            Self::CharacteristicMismatch => {
                f.write_str("carrier and identity characteristics differ")
            }
            Self::DegreeMismatch => f.write_str("carrier and identity degrees differ"),
            Self::CardinalityUnavailable => f.write_str("base cardinality is unavailable"),
        }
    }
}

impl fmt::Display for ModulusSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase { base, reason } => {
                write!(
                    f,
                    "cannot select over base of degree {}: {reason}",
                    base.degree()
                )
            }
            Self::NoCandidates { degree, .. } => {
                write!(f, "no monic modulus candidates exist for degree {degree}")
            }
            Self::UnsupportedSize { base, degree } => write!(
                f,
                "modulus search for degree {degree} over the degree-{} base is too large",
                base.degree()
            ),
            Self::InvalidRegistryEntry {
                degree,
                provenance,
                error,
                ..
            } => write!(
                f,
                "{provenance:?} registry entry for degree {degree} is invalid: {error}"
            ),
            Self::RegistryMaterialization {
                coefficient, error, ..
            } => write!(
                f,
                "registry coefficient {coefficient} cannot be materialized: {error}"
            ),
            Self::CandidateMaterialization {
                coefficient, error, ..
            } => write!(
                f,
                "candidate coefficient {coefficient} cannot be materialized: {error}"
            ),
            Self::Verification { rank, error, .. } => {
                write!(f, "candidate rank {rank} could not be verified: {error}")
            }
        }
    }
}

/// Selects a modulus using the built-in Conway/verified binary registry and
/// the deterministic verified fallback search.
///
/// The returned [`FieldPoly`] has coefficients in the same field as `base`.
/// In particular, if `base` is a non-prime tower field, all candidate
/// coefficients are elements of that tower field and the result is a
/// relative extension presentation.
pub fn select_modulus<F: FieldIdentity>(
    base: &F,
    degree: usize,
) -> Result<FieldPoly<F>, ModulusSelectionError> {
    select_modulus_with_registry(
        base,
        degree,
        &crate::primitive_polys::PrimitivePolynomialDatabase,
    )
}

/// Selects a modulus with an explicitly supplied registry adapter.
///
/// The adapter's Conway entries are tried first, followed by its other
/// verified entries.  If neither method supplies an entry, candidates are
/// enumerated by the complete rank order documented at the module level and
/// accepted only after [`prove_irreducible`] succeeds.
pub fn select_modulus_with_registry<F: FieldIdentity, R: ModulusRegistry>(
    base: &F,
    degree: usize,
    registry: &R,
) -> Result<FieldPoly<F>, ModulusSelectionError> {
    let base_id = validate_base(base)?;
    if degree == 0 {
        return Err(ModulusSelectionError::NoCandidates {
            base: base_id,
            degree,
        });
    }

    if let Some(entry) = registry.conway(&base_id, degree) {
        return materialize_registry_entry(base, &base_id, degree, entry);
    }
    if let Some(entry) = registry.verified(&base_id, degree) {
        return materialize_registry_entry(base, &base_id, degree, entry);
    }

    let candidate_count = candidate_count(&base_id, degree)?;
    let mut rank = 0u128;
    while rank < candidate_count {
        let candidate = materialize_candidate(base, &base_id, degree, rank)?;
        match prove_irreducible(&candidate, &base.zero_like()) {
            Ok(_) => return Ok(candidate),
            Err(FieldError::ReducibleModulus { .. }) => {}
            Err(error) => {
                return Err(ModulusSelectionError::Verification {
                    base: base_id,
                    degree,
                    rank,
                    error,
                })
            }
        }
        rank += 1;
    }

    Err(ModulusSelectionError::NoCandidates {
        base: base_id,
        degree,
    })
}

/// Selects a modulus and returns its canonical identity representation.
///
/// This is the identity-facing companion to [`select_modulus`].  The
/// polynomial decision still runs over the supplied carrier so fallback
/// candidates receive full irreducibility verification.
pub fn select_modulus_id<F: FieldIdentity>(
    base: &F,
    degree: usize,
) -> Result<ModulusId, ModulusSelectionError> {
    let polynomial = select_modulus(base, degree)?;
    modulus_id(&base.field_id(), &polynomial).map_err(|error| {
        ModulusSelectionError::CandidateMaterialization {
            base: base.field_id(),
            degree,
            coefficient: polynomial.len().saturating_sub(1),
            error,
        }
    })
}

fn validate_base<F: FieldIdentity>(base: &F) -> Result<FieldId, ModulusSelectionError> {
    let base_id = base.field_id();
    let characteristic = base.characteristic();
    if base_id.characteristic() != characteristic {
        return Err(ModulusSelectionError::InvalidBase {
            base: base_id,
            reason: BaseValidationError::CharacteristicMismatch,
        });
    }
    if base_id.degree() != base.extension_degree() {
        return Err(ModulusSelectionError::InvalidBase {
            base: base_id,
            reason: BaseValidationError::DegreeMismatch,
        });
    }
    if !is_prime(characteristic) {
        return Err(ModulusSelectionError::InvalidBase {
            base: base_id,
            reason: BaseValidationError::CompositeCharacteristic,
        });
    }
    if base_id.order().is_none() {
        return Err(ModulusSelectionError::InvalidBase {
            base: base_id,
            reason: BaseValidationError::CardinalityUnavailable,
        });
    }
    Ok(base_id)
}

fn candidate_count(base: &FieldId, degree: usize) -> Result<u128, ModulusSelectionError> {
    let order = base
        .order()
        .ok_or_else(|| ModulusSelectionError::UnsupportedSize {
            base: base.clone(),
            degree,
        })?;
    let mut count = 1u128;
    for _ in 0..degree {
        count = count
            .checked_mul(order)
            .ok_or_else(|| ModulusSelectionError::UnsupportedSize {
                base: base.clone(),
                degree,
            })?;
    }
    if count == 0 {
        return Err(ModulusSelectionError::NoCandidates {
            base: base.clone(),
            degree,
        });
    }
    Ok(count)
}

fn materialize_registry_entry<F: FieldIdentity>(
    base: &F,
    base_id: &FieldId,
    degree: usize,
    entry: RegistryEntry,
) -> Result<FieldPoly<F>, ModulusSelectionError> {
    let provenance = entry.provenance;
    let modulus = ModulusId::new(base_id, entry.coefficients).map_err(|error| {
        ModulusSelectionError::InvalidRegistryEntry {
            base: base_id.clone(),
            degree,
            provenance,
            error,
        }
    })?;
    if modulus.degree() != degree {
        return Err(ModulusSelectionError::InvalidRegistryEntry {
            base: base_id.clone(),
            degree,
            provenance,
            error: FieldError::ModulusDegreeTooSmall {
                degree: modulus.degree(),
            },
        });
    }

    let coefficients = (0..=degree)
        .map(|index| {
            base.from_prime_coords(modulus.coefficient(index))
                .map_err(|error| ModulusSelectionError::RegistryMaterialization {
                    base: base_id.clone(),
                    degree,
                    coefficient: index,
                    error,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FieldPoly::new(coefficients))
}

fn materialize_candidate<F: FieldIdentity>(
    base: &F,
    base_id: &FieldId,
    degree: usize,
    mut rank: u128,
) -> Result<FieldPoly<F>, ModulusSelectionError> {
    let order = base_id
        .order()
        .ok_or_else(|| ModulusSelectionError::UnsupportedSize {
            base: base_id.clone(),
            degree,
        })?;
    let base_degree = base_id.degree();
    let characteristic = u128::from(base_id.characteristic());
    let coefficient_count =
        degree
            .checked_add(1)
            .ok_or_else(|| ModulusSelectionError::UnsupportedSize {
                base: base_id.clone(),
                degree,
            })?;
    let mut coefficients = Vec::with_capacity(coefficient_count);

    for index in 0..degree {
        let coefficient_index = rank % order;
        rank /= order;
        let mut value = coefficient_index;
        let mut coordinates = Vec::with_capacity(base_degree);
        for _ in 0..base_degree {
            coordinates.push((value % characteristic) as u64);
            value /= characteristic;
        }
        if value != 0 {
            return Err(ModulusSelectionError::CandidateMaterialization {
                base: base_id.clone(),
                degree,
                coefficient: index,
                error: FieldError::UnsupportedSize {
                    degree,
                    characteristic: base_id.characteristic(),
                },
            });
        }
        coefficients.push(base.from_prime_coords(&coordinates).map_err(|error| {
            ModulusSelectionError::CandidateMaterialization {
                base: base_id.clone(),
                degree,
                coefficient: index,
                error,
            }
        })?);
    }
    coefficients.push(base.one_like());
    Ok(FieldPoly::new(coefficients))
}

fn modulus_id<F: FieldIdentity>(
    base: &FieldId,
    polynomial: &FieldPoly<F>,
) -> Result<ModulusId, FieldError> {
    let mut coordinates = Vec::new();
    let mut coefficient_coordinates = Vec::new();
    for coefficient in polynomial.iter() {
        coefficient.write_prime_coords(&mut coefficient_coordinates);
        coordinates.extend_from_slice(&coefficient_coordinates);
    }
    ModulusId::new(base, coordinates)
}

fn is_prime(value: u64) -> bool {
    if value < 2 {
        return false;
    }
    for small in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if value == small {
            return true;
        }
        if value.is_multiple_of(small) {
            return false;
        }
    }

    let mut exponent = value - 1;
    let trailing = exponent.trailing_zeros();
    exponent >>= trailing;
    for witness in [2u64, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_260_222] {
        if witness % value == 0 {
            continue;
        }
        let mut current = mod_pow_u128(u128::from(witness % value), exponent, value);
        if current == 1 || current == u128::from(value - 1) {
            continue;
        }
        let mut composite = true;
        for _ in 1..trailing {
            current = (current * current) % u128::from(value);
            if current == u128::from(value - 1) {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

fn mod_pow_u128(mut base: u128, mut exponent: u64, modulus: u64) -> u128 {
    let modulus = u128::from(modulus);
    let mut result = 1u128;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = (result * base) % modulus;
        }
        exponent >>= 1;
        if exponent != 0 {
            base = (base * base) % modulus;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::extension::FieldIdentity;
    use crate::field::irreducibility::prove_irreducible;
    use crate::field::{ConstField, FieldPoly, FiniteField};
    use crate::gfp::Fp;
    use crate::gfpn::{ExtConfig, QuadraticExt};

    fn coefficients<F: FieldIdentity>(polynomial: &FieldPoly<F>) -> Vec<Vec<u64>> {
        polynomial
            .iter()
            .map(|coefficient| {
                let mut coordinates = Vec::new();
                coefficient.write_prime_coords(&mut coordinates);
                coordinates
            })
            .collect()
    }

    #[test]
    fn fallback_pins_binary_selection_order() {
        let selected = select_modulus_with_registry(&Fp::<2>::zero(), 4, &EmptyRegistry).unwrap();
        assert_eq!(
            coefficients(&selected),
            vec![vec![1], vec![1], vec![0], vec![0], vec![1]]
        );
    }

    #[test]
    fn fallback_pins_prime_field_selection_order() {
        let selected = select_modulus_with_registry(&Fp::<3>::zero(), 2, &EmptyRegistry).unwrap();
        assert_eq!(coefficients(&selected), vec![vec![1], vec![0], vec![1]]);
    }

    struct Gf9Config;

    impl ExtConfig for Gf9Config {
        type BaseField = Fp<3>;
        const NON_RESIDUE: Fp<3> = Fp::<3>::new(2);
    }

    type Gf9 = QuadraticExt<Gf9Config>;

    #[test]
    fn fallback_pins_relative_tower_selection() {
        let base = <Gf9 as ConstField>::zero();
        let selected = select_modulus_with_registry(&base, 2, &EmptyRegistry).unwrap();
        let coordinate_vectors = coefficients(&selected);
        assert_eq!(coordinate_vectors, vec![vec![1, 1], vec![0, 0], vec![1, 0]]);
        assert_eq!(selected.coeff(0).field_id(), base.field_id());
        assert!(prove_irreducible(&selected, &base).is_ok());
    }

    #[test]
    fn built_in_registry_prefers_the_conway_entry() {
        let selected = select_modulus(&Fp::<2>::zero(), 32).unwrap();
        let mut value = 0u64;
        for (index, coefficient) in selected.iter().enumerate() {
            if coefficient.is_one() {
                value |= 1u64 << index;
            }
        }
        assert_eq!(value, 0x1_0000_8299);
    }

    #[test]
    fn fallback_is_stable_and_verified() {
        let base = Fp::<5>::zero();
        let first = select_modulus_with_registry(&base, 3, &EmptyRegistry).unwrap();
        let second = select_modulus_with_registry(&base, 3, &EmptyRegistry).unwrap();
        assert_eq!(coefficients(&first), coefficients(&second));
        assert!(prove_irreducible(&first, &base).is_ok());
    }

    #[test]
    fn binary_adapter_matches_existing_registry() {
        let selected = select_modulus(&Fp::<2>::zero(), 8).unwrap();
        let mut value = 0u64;
        for (index, coefficient) in selected.iter().enumerate() {
            if coefficient.is_one() {
                value |= 1u64 << index;
            }
        }
        assert_eq!(
            Some(value),
            crate::primitive_polys::PrimitivePolynomialDatabase::standard(8)
        );
    }

    #[test]
    fn binary_u128_adapter_matches_existing_registry() {
        let selected = select_modulus(&Fp::<2>::zero(), 64).unwrap();
        let mut value = 0u128;
        for (index, coefficient) in selected.iter().enumerate() {
            if coefficient.is_one() {
                value |= 1u128 << index;
            }
        }
        assert_eq!(
            Some(value),
            crate::primitive_polys::PrimitivePolynomialDatabase::standard_u128(64)
        );
    }

    #[test]
    fn typed_selection_errors_cover_invalid_empty_and_oversized_requests() {
        assert!(matches!(
            select_modulus_with_registry(&Fp::<2>::zero(), 0, &EmptyRegistry),
            Err(ModulusSelectionError::NoCandidates { degree: 0, .. })
        ));
        assert!(matches!(
            select_modulus_with_registry(&Fp::<4>::new(0), 2, &EmptyRegistry),
            Err(ModulusSelectionError::InvalidBase {
                reason: BaseValidationError::CompositeCharacteristic,
                ..
            })
        ));

        let binary = crate::gf2m::Gf2mField_::<u128>::new(127, (1u128 << 127) | 0b11);
        let element = binary.element(0);
        assert!(matches!(
            select_modulus_with_registry(&element, 2, &EmptyRegistry),
            Err(ModulusSelectionError::UnsupportedSize { .. })
        ));
    }

    #[test]
    fn primality_check_handles_large_u63_values() {
        assert!(is_prime(2));
        assert!(is_prime(2_305_843_009_213_693_951));
        assert!(!is_prime(2_305_843_009_213_693_952));
    }

    #[test]
    fn selected_identity_uses_the_actual_base_presentation() {
        let base = <Gf9 as ConstField>::zero();
        let id = select_modulus_id(&base, 2).unwrap();
        assert_eq!(id.coefficients().len(), 6);
        assert_eq!(base.field_id().degree(), 2);
    }
}
