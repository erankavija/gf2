//! BCH-specific construction and decoding errors.
//!
//! [`BchError`] is the boundary between BCH algorithms and the lower-level
//! error layers.  Field failures remain [`FieldError`] sources, generic code
//! and buffer failures remain [`CodeError`] sources, and the variants in this
//! module add only BCH mathematical validation context.

use crate::bch::encode::EncodeFamily;
use crate::error::CodeError;
use gf2_core::field::extension::FieldError;
use std::fmt;

/// Failures produced while constructing or decoding a BCH code.
///
/// A decoder reports a received-buffer/type problem through [`Self::Decode`]
/// and the general [`CodeError`] it wraps.  A bounded-distance decoder's
/// inability to find a verified correction is an algorithmic outcome (the
/// designed `Uncorrectable` status), not an error in this enum.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BchError {
    /// A field-extension operation rejected its input or witness.
    Field(FieldError),
    /// A generic code or construction buffer validation failed.
    Code(CodeError),
    /// A BCH decoder received a buffer or erased value that failed generic
    /// code validation.
    Decode(CodeError),
    /// A workspace was built for a different code than the one using it.
    WorkspaceMismatch {
        /// Fingerprint of the decoder's own code.
        expected_stamp: u64,
        /// Fingerprint carried by the supplied workspace.
        actual_stamp: u64,
    },
    /// An explicitly named encoding family is one this code's representation
    /// does not implement for its plan.
    EncodeFamilyUnavailable {
        /// The family the caller named.
        family: EncodeFamily,
    },
    /// A length conversion received zero, which cannot define a cyclic code.
    InvalidLength {
        /// The rejected length.
        length: u64,
    },
    /// A designed-distance conversion received zero.
    InvalidDesignedDistance {
        /// The rejected designed distance.
        designed_distance: u64,
    },
    /// The requested length shares a factor with the base-field
    /// characteristic, so it would define a repeated-root cyclic code.
    LengthNotCoprimeToCharacteristic {
        /// Requested cyclic-code length.
        length: u64,
        /// Characteristic of the base field.
        characteristic: u64,
    },
    /// The requested order does not divide the splitting field's
    /// multiplicative-group order.
    LengthDoesNotDivideUnitGroup {
        /// Requested root order and code length.
        length: u64,
        /// Order of the splitting field's multiplicative group.
        unit_group_order: u128,
    },
    /// A non-primitive construction was given the primitive length instead of
    /// a proper divisor of the splitting field's unit-group order.
    LengthNotProperlyNonPrimitive {
        /// Requested non-primitive length.
        length: u64,
        /// Primitive length of the splitting field.
        unit_group_order: u128,
    },
    /// An explicit root does not have the exact order required by the spec.
    RootOrderMismatch {
        /// Required multiplicative order.
        expected: u64,
        /// Multiplicative order observed for the supplied root.
        actual: u128,
    },
    /// A root exponent is not a member of `0..length`.
    RootExponentOutOfRange {
        /// Rejected root exponent.
        exponent: u64,
        /// Length whose residue range was required.
        length: u64,
    },
    /// A designed distance is outside the allowed range `1..=length + 1`.
    DesignedDistanceOutOfRange {
        /// Rejected designed distance.
        designed_distance: u64,
        /// Code length used for the upper bound.
        length: u64,
    },
    /// An explicitly supplied generator polynomial is zero.
    GeneratorPolynomialZero,
    /// An explicitly supplied generator polynomial is not monic.
    GeneratorPolynomialNotMonic,
    /// A coefficient of an explicit generator polynomial does not restrict to
    /// the base field.
    GeneratorCoefficientNotInBase {
        /// Coefficient index in ascending polynomial order.
        index: usize,
    },
    /// The restricted generator polynomial is not a divisor of `x^length - 1`.
    GeneratorNotDivisorOfCyclicPolynomial {
        /// Length used to form the cyclic polynomial.
        length: u64,
    },
}

impl fmt::Display for BchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(error) => write!(formatter, "BCH field operation failed: {error}"),
            Self::Code(error) => write!(formatter, "BCH code validation failed: {error}"),
            Self::Decode(error) => write!(formatter, "BCH decode input failed validation: {error}"),
            Self::WorkspaceMismatch {
                expected_stamp,
                actual_stamp,
            } => write!(
                formatter,
                "BCH workspace was built for a different code \
                 (expected fingerprint {expected_stamp:#018x}, found {actual_stamp:#018x})"
            ),
            Self::EncodeFamilyUnavailable { family } => write!(
                formatter,
                "encoding family {family} is not available for this code's representation"
            ),
            Self::InvalidLength { length } => {
                write!(formatter, "BCH length {length} is not positive")
            }
            Self::InvalidDesignedDistance { designed_distance } => write!(
                formatter,
                "designed distance {designed_distance} is not positive"
            ),
            Self::LengthNotCoprimeToCharacteristic {
                length,
                characteristic,
            } => write!(
                formatter,
                "BCH length {length} is not coprime to characteristic {characteristic}"
            ),
            Self::LengthDoesNotDivideUnitGroup {
                length,
                unit_group_order,
            } => write!(
                formatter,
                "BCH length {length} does not divide unit-group order {unit_group_order}"
            ),
            Self::LengthNotProperlyNonPrimitive {
                length,
                unit_group_order,
            } => write!(
                formatter,
                "BCH non-primitive length {length} is not a proper divisor of {unit_group_order}"
            ),
            Self::RootOrderMismatch { expected, actual } => write!(
                formatter,
                "BCH root has order {actual}, expected exact order {expected}"
            ),
            Self::RootExponentOutOfRange { exponent, length } => write!(
                formatter,
                "root exponent {exponent} is outside the range 0..{length}"
            ),
            Self::DesignedDistanceOutOfRange {
                designed_distance,
                length,
            } => write!(
                formatter,
                "designed distance {designed_distance} is outside 1..={length}+1"
            ),
            Self::GeneratorPolynomialZero => {
                formatter.write_str("BCH generator polynomial must be nonzero")
            }
            Self::GeneratorPolynomialNotMonic => {
                formatter.write_str("BCH generator polynomial must be monic")
            }
            Self::GeneratorCoefficientNotInBase { index } => write!(
                formatter,
                "BCH generator coefficient {index} does not lie in the base field"
            ),
            Self::GeneratorNotDivisorOfCyclicPolynomial { length } => write!(
                formatter,
                "BCH generator is not a divisor of x^{length} - 1"
            ),
        }
    }
}

impl std::error::Error for BchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Field(error) => Some(error),
            Self::Code(error) | Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FieldError> for BchError {
    fn from(error: FieldError) -> Self {
        Self::Field(error)
    }
}

impl From<CodeError> for BchError {
    fn from(error: CodeError) -> Self {
        Self::Code(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn bch_error_preserves_field_and_code_sources() {
        let field = BchError::from(FieldError::NotInBase);
        assert!(field.source().is_some());

        let code = BchError::from(CodeError::BufferLengthMismatch {
            expected: 15,
            actual: 14,
        });
        assert!(code.source().is_some());

        let decode = BchError::Decode(CodeError::BufferLengthMismatch {
            expected: 15,
            actual: 14,
        });
        assert!(decode.source().is_some());
    }

    #[test]
    fn bch_error_describes_root_order_context() {
        let error = BchError::RootOrderMismatch {
            expected: 15,
            actual: 3,
        };
        let rendered = error.to_string();
        assert!(rendered.contains("15"));
        assert!(rendered.contains("3"));
    }

    #[test]
    fn bch_error_variants_capture_construction_context() {
        let errors = [
            BchError::InvalidLength { length: 0 },
            BchError::InvalidDesignedDistance {
                designed_distance: 0,
            },
            BchError::LengthNotCoprimeToCharacteristic {
                length: 6,
                characteristic: 2,
            },
            BchError::LengthDoesNotDivideUnitGroup {
                length: 5,
                unit_group_order: 12,
            },
            BchError::LengthNotProperlyNonPrimitive {
                length: 15,
                unit_group_order: 15,
            },
            BchError::RootOrderMismatch {
                expected: 15,
                actual: 3,
            },
            BchError::RootExponentOutOfRange {
                exponent: 15,
                length: 15,
            },
            BchError::DesignedDistanceOutOfRange {
                designed_distance: 17,
                length: 15,
            },
            BchError::GeneratorPolynomialZero,
            BchError::GeneratorPolynomialNotMonic,
            BchError::GeneratorCoefficientNotInBase { index: 2 },
            BchError::GeneratorNotDivisorOfCyclicPolynomial { length: 15 },
            BchError::EncodeFamilyUnavailable {
                family: EncodeFamily::TableRemainder,
            },
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
