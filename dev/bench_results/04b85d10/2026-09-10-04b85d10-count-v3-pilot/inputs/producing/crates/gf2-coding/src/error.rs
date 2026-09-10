//! Errors shared by generic coding and code-transformation APIs.
//!
//! The error layer belongs to `gf2-coding`, rather than to an individual code
//! family.  It therefore contains no BCH-specific vocabulary.  Generic code
//! traits, coordinate maps, matrix materializers, and runtime-erased handles
//! can all report these conditions without exposing a concrete implementation.

use gf2_core::field::extension::FieldId;
use std::any::TypeId;
use std::fmt;

/// A process-local identity for a symbol or matrix representation.
///
/// This token is intentionally separate from [`FieldId`].  A field identity
/// describes the mathematical field and is suitable for serialization;
/// this identity describes the Rust representation used by an erased handle
/// and must not be used as a wire-format identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RepresentationId(TypeId);

impl RepresentationId {
    /// Returns the representation identity for the `'static` Rust type `T`.
    ///
    /// Two calls for the same type return equal identities.  Different Rust
    /// types are expected to have different identities within one process.
    pub fn of<T: 'static>() -> Self {
        Self(TypeId::of::<T>())
    }
}

/// A capability whose absence is an ordinary property of a code value.
///
/// This semantic vocabulary lets callers distinguish an unsupported operation
/// from a failed attempt to materialize a supported operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CodeCapability {
    /// Access to the canonical generator matrix is not available.
    GeneratorMatrix,
    /// Access to a parity-check matrix is not available.
    ParityCheckMatrix,
}

/// Alias for the capability vocabulary used by [`CodeError`].
pub type Capability = CodeCapability;

impl fmt::Display for CodeCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratorMatrix => formatter.write_str("generator matrix"),
            Self::ParityCheckMatrix => formatter.write_str("parity-check matrix"),
        }
    }
}

/// Failures shared by coding, encoding, matrix, and transformation APIs.
///
/// Each variant describes a caller-visible invalid condition.  BCH-specific
/// construction and decoding failures belong to [`crate::bch::error::BchError`]
/// and are deliberately absent here.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CodeError {
    /// A coordinate refers to a position outside `0..length`.
    CoordinateOutOfRange {
        /// The invalid coordinate.
        coordinate: usize,
        /// Number of coordinates in the addressed codeword.
        length: usize,
    },
    /// A coordinate set contains the same coordinate more than once.
    DuplicateCoordinate {
        /// The coordinate repeated in the set.
        coordinate: usize,
    },
    /// A generic indexed operation received an index outside `0..length`.
    IndexOutOfBounds {
        /// The invalid index.
        index: usize,
        /// Number of addressable entries.
        length: usize,
    },
    /// A coordinate map or coordinate list has the wrong number of entries.
    CoordinateCountMismatch {
        /// Number of coordinates required by the operation.
        expected: usize,
        /// Number of coordinates supplied by the caller.
        actual: usize,
    },
    /// A symbol or codeword buffer has the wrong one-dimensional length.
    BufferLengthMismatch {
        /// Required number of entries.
        expected: usize,
        /// Number of entries supplied.
        actual: usize,
    },
    /// A matrix or rectangular buffer has the wrong dimensions.
    ShapeMismatch {
        /// Required row count.
        expected_rows: usize,
        /// Required column count.
        expected_cols: usize,
        /// Supplied row count.
        actual_rows: usize,
        /// Supplied column count.
        actual_cols: usize,
    },
    /// An element or value belongs to a different mathematical field.
    FieldMismatch {
        /// Field identity required by the operation.
        expected: FieldId,
        /// Field identity carried by the supplied value.
        found: FieldId,
    },
    /// An erased value has the wrong Rust representation type.
    RepresentationMismatch {
        /// Representation identity required by the operation.
        expected: RepresentationId,
        /// Representation identity carried by the supplied value.
        found: RepresentationId,
    },
    /// A requested allocation or materialized dimension exceeds the supported
    /// `usize` representation.
    UnsupportedSize {
        /// Requested size before conversion to an in-memory dimension.
        size: u128,
    },
    /// The requested operation is not a capability of this code value.
    CapabilityUnavailable {
        /// Capability the caller attempted to use.
        capability: CodeCapability,
    },
}

impl fmt::Display for CodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOutOfRange {
                coordinate,
                length,
            } => write!(
                formatter,
                "coordinate {coordinate} is outside the codeword range 0..{length}"
            ),
            Self::DuplicateCoordinate { coordinate } => {
                write!(formatter, "coordinate {coordinate} occurs more than once")
            }
            Self::IndexOutOfBounds { index, length } => write!(
                formatter,
                "index {index} is outside the addressable range 0..{length}"
            ),
            Self::CoordinateCountMismatch { expected, actual } => write!(
                formatter,
                "expected {expected} coordinates, found {actual}"
            ),
            Self::BufferLengthMismatch { expected, actual } => write!(
                formatter,
                "expected a buffer with {expected} entries, found {actual}"
            ),
            Self::ShapeMismatch {
                expected_rows,
                expected_cols,
                actual_rows,
                actual_cols,
            } => write!(
                formatter,
                "expected shape {expected_rows} x {expected_cols}, found {actual_rows} x {actual_cols}"
            ),
            Self::FieldMismatch { expected, found } => write!(
                formatter,
                "field identity mismatch: expected {expected:?}, found {found:?}"
            ),
            Self::RepresentationMismatch { expected, found } => write!(
                formatter,
                "representation identity mismatch: expected {expected:?}, found {found:?}"
            ),
            Self::UnsupportedSize { size } => write!(
                formatter,
                "requested size {size} cannot be represented in memory"
            ),
            Self::CapabilityUnavailable { capability } => {
                write!(formatter, "{capability} capability is unavailable")
            }
        }
    }
}

impl std::error::Error for CodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_error_describes_shape_and_coordinate_context() {
        let shape = CodeError::ShapeMismatch {
            expected_rows: 2,
            expected_cols: 3,
            actual_rows: 4,
            actual_cols: 5,
        };
        assert!(shape.to_string().contains("2 x 3"));
        assert!(shape.to_string().contains("4 x 5"));

        let coordinate = CodeError::CoordinateOutOfRange {
            coordinate: 7,
            length: 7,
        };
        assert!(coordinate.to_string().contains('7'));
    }

    #[test]
    fn code_error_variants_capture_general_failure_context() {
        use gf2_core::field::FieldIdentity;
        use gf2_core::gfp::Fp;

        let field_two = Fp::<2>::new(0).field_id();
        let field_three = Fp::<3>::new(0).field_id();
        let errors = [
            CodeError::CoordinateOutOfRange {
                coordinate: 8,
                length: 8,
            },
            CodeError::DuplicateCoordinate { coordinate: 4 },
            CodeError::IndexOutOfBounds {
                index: 9,
                length: 9,
            },
            CodeError::CoordinateCountMismatch {
                expected: 3,
                actual: 2,
            },
            CodeError::BufferLengthMismatch {
                expected: 15,
                actual: 14,
            },
            CodeError::ShapeMismatch {
                expected_rows: 2,
                expected_cols: 3,
                actual_rows: 3,
                actual_cols: 2,
            },
            CodeError::FieldMismatch {
                expected: field_two,
                found: field_three,
            },
            CodeError::RepresentationMismatch {
                expected: RepresentationId::of::<u8>(),
                found: RepresentationId::of::<u16>(),
            },
            CodeError::UnsupportedSize { size: u128::MAX },
            CodeError::CapabilityUnavailable {
                capability: CodeCapability::ParityCheckMatrix,
            },
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn representation_ids_are_semantic_type_tokens() {
        assert_eq!(RepresentationId::of::<u8>(), RepresentationId::of::<u8>());
        assert_ne!(RepresentationId::of::<u8>(), RepresentationId::of::<u16>());
    }
}
