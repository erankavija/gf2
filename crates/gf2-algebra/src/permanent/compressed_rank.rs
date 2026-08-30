//! Exact compressed-state propagation for three-column permanental rank.
//!
//! For rows `u`, `v`, and `x`, define the symmetric bilinear contraction
//!
//! ```text
//! phi(u, v) = (u2*v3 + u3*v2,
//!              u1*v3 + u3*v1,
//!              u1*v2 + u2*v1).
//! ```
//!
//! Expansion in the coordinates of `x` gives the contraction identity
//! `per(u, v, x) = phi(u, v) dot x`. For a valid prefix, let `U` be its row
//! span and `V` the span of contractions of distinct prefix rows. An appended row
//! is valid exactly when `x` lies in `V`'s orthogonal complement. The successor
//! is
//!
//! ```text
//! U' = U + <x>,              V' = V + phi(U, x).
//! ```
//!
//! The second update is basis-independent because `u -> phi(u, x)` is linear.
//! Inductively, `(U, V)` is therefore sufficient: it decides every admissible
//! next row and determines the canonical successor without retaining a row
//! history.
//!
//! [`CompressedTransitionTable`] groups the admissible rows by successor. If
//! `C_t(S)` counts valid ordered prefixes in state `S` and `m(S,S')` is an
//! edge multiplicity, exact propagation applies
//! `C_(t+1)(S') = sum_S C_t(S) m(S,S')`. The terminal count divided by
//! `q^(3n)` is returned as an arbitrary-precision [`ExactProbability`]. This
//! is a finite algebraic count, not an estimator, interval, or statistical
//! interpretation.
//!
//! # Validation and safety
//!
//! Persisted subspaces reject noncanonical RREF bytes, and persisted states are
//! accepted only when canonical transition replay reconstructs them. The module
//! contains no unsafe code, randomness, sampling, device work, or fallback
//! path; every supported field and every transition is exact.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;
use num_bigint::BigUint;

use super::ExactProbability;

/// Version byte for canonical subspace and state encodings.
pub const CANONICAL_ENCODING_VERSION: u8 = 1;

/// Domain-separation byte fed before a canonical subspace hash key.
pub const CANONICAL_SUBSPACE_HASH_DOMAIN: u8 = b'S';

const SUBSPACE_KEY_LEN: usize = 10;
const SUBSPACE_ENCODING_LEN: usize = 12;
const STATE_ENCODING_LEN: usize = 22;

mod sealed {
    pub trait Sealed {}

    impl Sealed for gf2_core::gfp::Fp<3> {}
    impl Sealed for gf2_core::gfp::Fp<5> {}
    impl Sealed for gf2_core::gfp::Fp<7> {}
}

/// A prime field supported by the compressed three-column recurrence.
///
/// The trait is sealed: its complete implementation set is `Fp<3>`, `Fp<5>`,
/// and `Fp<7>`. This keeps the state encoding's one-byte field identity and
/// the exhaustive validation domain closed.
pub trait SupportedPrimeField: sealed::Sealed + FiniteField + Copy + Send + Sync + 'static {
    /// Prime field order as its canonical one-byte identity.
    const ORDER: u8;

    /// Constructs a field element from a canonical residue.
    ///
    /// Returns `None` rather than reducing when `residue >= Self::ORDER`.
    fn from_residue(residue: u8) -> Option<Self>;

    /// Returns the canonical residue in `0..Self::ORDER`.
    fn residue(self) -> u8;
}

macro_rules! impl_supported_field {
    ($order:literal) => {
        impl SupportedPrimeField for Fp<$order> {
            const ORDER: u8 = $order;

            fn from_residue(residue: u8) -> Option<Self> {
                (residue < Self::ORDER).then(|| Self::new(u64::from(residue)))
            }

            fn residue(self) -> u8 {
                self.value() as u8
            }
        }
    };
}

impl_supported_field!(3);
impl_supported_field!(5);
impl_supported_field!(7);

/// A canonical three-coordinate vector over a supported prime field.
///
/// Coordinates are stored as their unique residues. Construction rejects an
/// out-of-field byte rather than silently reducing it.
pub struct Vector3<F: SupportedPrimeField> {
    residues: [u8; 3],
    field: PhantomData<F>,
}

impl<F: SupportedPrimeField> Copy for Vector3<F> {}

impl<F: SupportedPrimeField> Clone for Vector3<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F: SupportedPrimeField> fmt::Debug for Vector3<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Vector3")
            .field(&self.residues)
            .finish()
    }
}

impl<F: SupportedPrimeField> PartialEq for Vector3<F> {
    fn eq(&self, other: &Self) -> bool {
        self.residues == other.residues
    }
}

impl<F: SupportedPrimeField> Eq for Vector3<F> {}

impl<F: SupportedPrimeField> PartialOrd for Vector3<F> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<F: SupportedPrimeField> Ord for Vector3<F> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.residues.cmp(&other.residues)
    }
}

impl<F: SupportedPrimeField> Hash for Vector3<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.residues);
    }
}

impl<F: SupportedPrimeField> Vector3<F> {
    /// Constructs a vector from canonical residues.
    ///
    /// Returns the first out-of-field residue on failure.
    pub fn from_residues(residues: [u8; 3]) -> Result<Self, u8> {
        if let Some(&invalid) = residues.iter().find(|&&residue| residue >= F::ORDER) {
            return Err(invalid);
        }
        Ok(Self {
            residues,
            field: PhantomData,
        })
    }

    /// Constructs a vector from field elements.
    #[must_use]
    pub fn from_elements(elements: [F; 3]) -> Self {
        Self {
            residues: elements.map(SupportedPrimeField::residue),
            field: PhantomData,
        }
    }

    /// Returns the canonical residue coordinates.
    #[must_use]
    pub const fn residues(self) -> [u8; 3] {
        self.residues
    }

    /// Returns the coordinates as field elements.
    #[must_use]
    pub fn elements(self) -> [F; 3] {
        self.residues.map(|residue| {
            F::from_residue(residue).expect("Vector3 stores only canonical residues")
        })
    }

    fn dot(self, other: Self) -> u8 {
        (0..3).fold(0, |sum, index| {
            add_mod(
                sum,
                multiply_mod(self.residues[index], other.residues[index], F::ORDER),
                F::ORDER,
            )
        })
    }
}

/// Failure to decode a canonical subspace byte representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSubspaceDecodeError {
    /// The byte slice is not the exact 12-byte representation.
    WrongLength {
        /// Required byte length.
        expected: usize,
        /// Supplied byte length.
        actual: usize,
    },
    /// The representation uses an unknown version byte.
    UnsupportedVersion(u8),
    /// The encoded field does not match the type-level field.
    WrongField {
        /// Type-level field order.
        expected: u8,
        /// Encoded field-order byte.
        actual: u8,
    },
    /// The encoded dimension is greater than three.
    InvalidDimension(u8),
    /// One basis entry is not a canonical field residue.
    InvalidResidue(u8),
    /// A row after the declared dimension is not all zero.
    NonzeroPadding,
    /// The encoded basis is not already the unique RREF basis.
    NonCanonicalBasis,
}

impl fmt::Display for CanonicalSubspaceDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { expected, actual } => {
                write!(formatter, "expected {expected} bytes, received {actual}")
            }
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported canonical encoding version {version}"
                )
            }
            Self::WrongField { expected, actual } => {
                write!(
                    formatter,
                    "expected field order {expected}, received {actual}"
                )
            }
            Self::InvalidDimension(dimension) => {
                write!(formatter, "subspace dimension {dimension} exceeds three")
            }
            Self::InvalidResidue(residue) => {
                write!(formatter, "basis residue {residue} is outside the field")
            }
            Self::NonzeroPadding => formatter.write_str("unused basis rows must be zero"),
            Self::NonCanonicalBasis => formatter.write_str("basis is not canonical RREF"),
        }
    }
}

impl std::error::Error for CanonicalSubspaceDecodeError {}

/// A subspace of `F^3` in its unique reduced-row-echelon representation.
///
/// Identity, ordering, hashing, and encoding all use the key consisting of the
/// dimension followed by the row-major padded `3 x 3` RREF basis.
pub struct CanonicalSubspace<F: SupportedPrimeField> {
    dimension: u8,
    basis: [[u8; 3]; 3],
    field: PhantomData<F>,
}

impl<F: SupportedPrimeField> Copy for CanonicalSubspace<F> {}

impl<F: SupportedPrimeField> Clone for CanonicalSubspace<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F: SupportedPrimeField> fmt::Debug for CanonicalSubspace<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalSubspace")
            .field("field_order", &F::ORDER)
            .field("dimension", &self.dimension)
            .field("basis", &self.basis)
            .finish()
    }
}

impl<F: SupportedPrimeField> PartialEq for CanonicalSubspace<F> {
    fn eq(&self, other: &Self) -> bool {
        self.key_bytes() == other.key_bytes()
    }
}

impl<F: SupportedPrimeField> Eq for CanonicalSubspace<F> {}

impl<F: SupportedPrimeField> PartialOrd for CanonicalSubspace<F> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<F: SupportedPrimeField> Ord for CanonicalSubspace<F> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key_bytes().cmp(&other.key_bytes())
    }
}

impl<F: SupportedPrimeField> Hash for CanonicalSubspace<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(CANONICAL_SUBSPACE_HASH_DOMAIN);
        state.write_u8(F::ORDER);
        state.write(&self.key_bytes());
    }
}

impl<F: SupportedPrimeField> CanonicalSubspace<F> {
    /// Returns the zero subspace.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            dimension: 0,
            basis: [[0; 3]; 3],
            field: PhantomData,
        }
    }

    /// Returns the span of `vectors`, normalized to unique RREF.
    ///
    /// # Complexity
    ///
    /// `O(v)` storage and `O(v)` fixed-width row operations for `v` input
    /// vectors; the ambient dimension is the constant three.
    #[must_use]
    pub fn from_vectors(vectors: &[Vector3<F>]) -> Self {
        Self::from_residue_vectors(vectors.iter().map(|vector| vector.residues))
    }

    /// Returns the subspace dimension.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension as usize
    }

    /// Returns the padded row-major canonical RREF basis residues.
    #[must_use]
    pub const fn basis_residues(&self) -> [[u8; 3]; 3] {
        self.basis
    }

    /// Returns the unpadded canonical RREF basis as field vectors.
    #[must_use]
    pub fn basis_vectors(&self) -> Vec<Vector3<F>> {
        self.basis[..self.dimension()]
            .iter()
            .copied()
            .map(|residues| {
                Vector3::from_residues(residues)
                    .expect("CanonicalSubspace stores only canonical residues")
            })
            .collect()
    }

    /// Returns the stable key: dimension followed by nine padded basis bytes.
    #[must_use]
    pub fn key_bytes(&self) -> [u8; SUBSPACE_KEY_LEN] {
        let mut key = [0; SUBSPACE_KEY_LEN];
        key[0] = self.dimension;
        for (offset, residue) in self.basis.iter().flatten().copied().enumerate() {
            key[offset + 1] = residue;
        }
        key
    }

    /// Returns version, field order, and the stable subspace key.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> [u8; SUBSPACE_ENCODING_LEN] {
        let mut bytes = [0; SUBSPACE_ENCODING_LEN];
        bytes[0] = CANONICAL_ENCODING_VERSION;
        bytes[1] = F::ORDER;
        bytes[2..].copy_from_slice(&self.key_bytes());
        bytes
    }

    /// Decodes and validates a canonical subspace representation.
    ///
    /// Wrong versions and fields, noncanonical residues, nonzero padding, and
    /// any basis that would change under RREF normalization are rejected.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, CanonicalSubspaceDecodeError> {
        if bytes.len() != SUBSPACE_ENCODING_LEN {
            return Err(CanonicalSubspaceDecodeError::WrongLength {
                expected: SUBSPACE_ENCODING_LEN,
                actual: bytes.len(),
            });
        }
        if bytes[0] != CANONICAL_ENCODING_VERSION {
            return Err(CanonicalSubspaceDecodeError::UnsupportedVersion(bytes[0]));
        }
        if bytes[1] != F::ORDER {
            return Err(CanonicalSubspaceDecodeError::WrongField {
                expected: F::ORDER,
                actual: bytes[1],
            });
        }
        let key: [u8; SUBSPACE_KEY_LEN] = bytes[2..]
            .try_into()
            .expect("length was checked before extracting the key");
        Self::from_key_bytes(key)
    }

    /// Returns whether `vector` lies in this subspace.
    #[must_use]
    pub fn contains(&self, vector: Vector3<F>) -> bool {
        self.sum_vector(vector) == *self
    }

    /// Returns the canonical orthogonal complement under the standard dot
    /// product.
    #[must_use]
    pub fn orthogonal_complement(&self) -> Self {
        let pivots: Vec<_> = self.basis[..self.dimension()]
            .iter()
            .map(|row| {
                row.iter()
                    .position(|&residue| residue != 0)
                    .expect("canonical basis rows are nonzero")
            })
            .collect();
        let vectors: Vec<_> = (0..3)
            .filter(|column| !pivots.contains(column))
            .map(|free_column| {
                let mut vector = [0; 3];
                vector[free_column] = 1;
                for (row, &pivot) in pivots.iter().enumerate() {
                    vector[pivot] = negate_mod(self.basis[row][free_column], F::ORDER);
                }
                vector
            })
            .collect();
        Self::from_residue_vectors(vectors)
    }

    fn from_key_bytes(key: [u8; SUBSPACE_KEY_LEN]) -> Result<Self, CanonicalSubspaceDecodeError> {
        let dimension = key[0];
        if dimension > 3 {
            return Err(CanonicalSubspaceDecodeError::InvalidDimension(dimension));
        }
        if let Some(&residue) = key[1..].iter().find(|&&residue| residue >= F::ORDER) {
            return Err(CanonicalSubspaceDecodeError::InvalidResidue(residue));
        }
        let mut basis = [[0; 3]; 3];
        for (destination, source) in basis.iter_mut().flatten().zip(&key[1..]) {
            *destination = *source;
        }
        if basis[usize::from(dimension)..]
            .iter()
            .flatten()
            .any(|&residue| residue != 0)
        {
            return Err(CanonicalSubspaceDecodeError::NonzeroPadding);
        }
        let candidate = Self {
            dimension,
            basis,
            field: PhantomData,
        };
        let normalized =
            Self::from_residue_vectors(candidate.basis[..candidate.dimension()].iter().copied());
        if normalized != candidate {
            return Err(CanonicalSubspaceDecodeError::NonCanonicalBasis);
        }
        Ok(candidate)
    }

    fn from_residue_vectors(vectors: impl IntoIterator<Item = [u8; 3]>) -> Self {
        let mut rows: Vec<_> = vectors.into_iter().filter(|row| *row != [0; 3]).collect();
        debug_assert!(rows.iter().flatten().all(|&residue| residue < F::ORDER));
        let mut rank = 0;
        for column in 0..3 {
            let Some(pivot) = (rank..rows.len()).find(|&row| rows[row][column] != 0) else {
                continue;
            };
            rows.swap(rank, pivot);
            let inverse = inverse_mod(rows[rank][column], F::ORDER);
            for entry in &mut rows[rank] {
                *entry = multiply_mod(*entry, inverse, F::ORDER);
            }
            let pivot_row = rows[rank];
            for (row_index, row) in rows.iter_mut().enumerate() {
                if row_index == rank {
                    continue;
                }
                let factor = row[column];
                for entry in column..3 {
                    row[entry] = subtract_mod(
                        row[entry],
                        multiply_mod(factor, pivot_row[entry], F::ORDER),
                        F::ORDER,
                    );
                }
            }
            rank += 1;
            if rank == 3 {
                break;
            }
        }
        let mut basis = [[0; 3]; 3];
        basis[..rank].copy_from_slice(&rows[..rank]);
        Self {
            dimension: rank as u8,
            basis,
            field: PhantomData,
        }
    }

    fn sum_vector(&self, vector: Vector3<F>) -> Self {
        Self::from_residue_vectors(
            self.basis[..self.dimension()]
                .iter()
                .copied()
                .chain([vector.residues]),
        )
    }

    fn sum_subspace(&self, other: &Self) -> Self {
        Self::from_residue_vectors(
            self.basis[..self.dimension()]
                .iter()
                .chain(&other.basis[..other.dimension()])
                .copied(),
        )
    }
}

/// Failure to reconstruct a compressed state from canonical bytes and rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompressedStateDecodeError {
    /// The byte slice is not the exact 22-byte state representation.
    WrongLength {
        /// Required byte length.
        expected: usize,
        /// Supplied byte length.
        actual: usize,
    },
    /// The representation uses an unknown version byte.
    UnsupportedVersion(u8),
    /// The encoded field does not match the type-level field.
    WrongField {
        /// Type-level field order.
        expected: u8,
        /// Encoded field-order byte.
        actual: u8,
    },
    /// The row-span key is malformed.
    InvalidRowSpan(CanonicalSubspaceDecodeError),
    /// The contraction-span key is malformed.
    InvalidContractionSpan(CanonicalSubspaceDecodeError),
    /// A replay row is inadmissible at the indicated zero-based prefix index.
    InadmissibleReplayRow(usize),
    /// Canonical row replay does not reconstruct the encoded state.
    ReplayMismatch,
}

impl fmt::Display for CompressedStateDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { expected, actual } => {
                write!(formatter, "expected {expected} bytes, received {actual}")
            }
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported canonical encoding version {version}"
                )
            }
            Self::WrongField { expected, actual } => {
                write!(
                    formatter,
                    "expected field order {expected}, received {actual}"
                )
            }
            Self::InvalidRowSpan(error) => write!(formatter, "invalid row span: {error}"),
            Self::InvalidContractionSpan(error) => {
                write!(formatter, "invalid contraction span: {error}")
            }
            Self::InadmissibleReplayRow(index) => {
                write!(formatter, "row {index} is inadmissible during replay")
            }
            Self::ReplayMismatch => formatter.write_str("replayed rows do not match encoded state"),
        }
    }
}

impl std::error::Error for CompressedStateDecodeError {}

/// Canonical row-span and contraction-span state for a valid row prefix.
///
/// The fields are private. Production construction begins at [`Self::initial`]
/// and proceeds through [`Self::successor`], or validates persisted identity by
/// replaying rows with [`Self::from_replayed_bytes`].
pub struct CompressedRankState<F: SupportedPrimeField> {
    row_span: CanonicalSubspace<F>,
    contraction_span: CanonicalSubspace<F>,
}

impl<F: SupportedPrimeField> Copy for CompressedRankState<F> {}

impl<F: SupportedPrimeField> Clone for CompressedRankState<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F: SupportedPrimeField> fmt::Debug for CompressedRankState<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompressedRankState")
            .field("row_span", &self.row_span)
            .field("contraction_span", &self.contraction_span)
            .finish()
    }
}

impl<F: SupportedPrimeField> PartialEq for CompressedRankState<F> {
    fn eq(&self, other: &Self) -> bool {
        self.row_span == other.row_span && self.contraction_span == other.contraction_span
    }
}

impl<F: SupportedPrimeField> Eq for CompressedRankState<F> {}

impl<F: SupportedPrimeField> PartialOrd for CompressedRankState<F> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<F: SupportedPrimeField> Ord for CompressedRankState<F> {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.row_span, &self.contraction_span).cmp(&(&other.row_span, &other.contraction_span))
    }
}

impl<F: SupportedPrimeField> Hash for CompressedRankState<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.row_span.key_bytes());
        state.write(&self.contraction_span.key_bytes());
    }
}

impl<F: SupportedPrimeField> CompressedRankState<F> {
    /// Returns the empty-prefix state `(0, 0)`.
    #[must_use]
    pub const fn initial() -> Self {
        Self {
            row_span: CanonicalSubspace::zero(),
            contraction_span: CanonicalSubspace::zero(),
        }
    }

    /// Returns the canonical row span `U`.
    #[must_use]
    pub const fn row_span(&self) -> &CanonicalSubspace<F> {
        &self.row_span
    }

    /// Returns the canonical contraction span `V`.
    #[must_use]
    pub const fn contraction_span(&self) -> &CanonicalSubspace<F> {
        &self.contraction_span
    }

    /// Returns exactly whether `row` lies in `V`'s orthogonal complement.
    #[must_use]
    pub fn is_admissible(&self, row: Vector3<F>) -> bool {
        self.contraction_span.basis[..self.contraction_span.dimension()]
            .iter()
            .copied()
            .all(|basis| {
                Vector3::<F>::from_residues(basis)
                    .expect("canonical basis residues are valid")
                    .dot(row)
                    == 0
            })
    }

    /// Returns the canonical image subspace `phi(U, row)`.
    ///
    /// The result is basis-independent because contraction with a fixed second
    /// argument is linear in its first argument.
    #[must_use]
    pub fn contraction_image(&self, row: Vector3<F>) -> CanonicalSubspace<F> {
        CanonicalSubspace::from_residue_vectors(
            self.row_span.basis[..self.row_span.dimension()]
                .iter()
                .copied()
                .map(|basis| contraction_residues(basis, row.residues, F::ORDER)),
        )
    }

    /// Returns the canonical successor, or `None` when the row would create a
    /// nonzero three-row permanent.
    #[must_use]
    pub fn successor(&self, row: Vector3<F>) -> Option<Self> {
        if !self.is_admissible(row) {
            return None;
        }
        let image = self.contraction_image(row);
        Some(Self {
            row_span: self.row_span.sum_vector(row),
            contraction_span: self.contraction_span.sum_subspace(&image),
        })
    }

    /// Groups every admissible row, including zero, by canonical successor.
    ///
    /// The returned transitions are in state order. Their multiplicities sum
    /// to `q^(3-dim(V))`.
    ///
    /// # Complexity
    ///
    /// Visits exactly `q^3` rows and stores at most that many distinct edges.
    #[must_use]
    pub fn transition_multiplicities(&self) -> Vec<CompressedTransition<F>> {
        let mut multiplicities = BTreeMap::new();
        for row in all_vectors::<F>() {
            if let Some(successor) = self.successor(row) {
                *multiplicities.entry(successor).or_insert(0_u64) += 1;
            }
        }
        let expected = u64::from(F::ORDER).pow((3 - self.contraction_span.dimension()) as u32);
        let observed: u64 = multiplicities.values().sum();
        assert_eq!(
            observed, expected,
            "compressed transition mass must equal |V^perp|"
        );
        multiplicities
            .into_iter()
            .map(|(successor, multiplicity)| CompressedTransition {
                successor,
                multiplicity,
            })
            .collect()
    }

    /// Returns version, field order, the row-span key, and the contraction-span
    /// key in that order.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> [u8; STATE_ENCODING_LEN] {
        let mut bytes = [0; STATE_ENCODING_LEN];
        bytes[0] = CANONICAL_ENCODING_VERSION;
        bytes[1] = F::ORDER;
        bytes[2..12].copy_from_slice(&self.row_span.key_bytes());
        bytes[12..22].copy_from_slice(&self.contraction_span.key_bytes());
        bytes
    }

    /// Validates a persisted state by canonical decoding and transition replay.
    ///
    /// Replay is part of validation: callers cannot introduce an arbitrary
    /// canonical pair that is unrelated to the supplied valid prefix.
    pub fn from_replayed_bytes(
        bytes: &[u8],
        rows: &[Vector3<F>],
    ) -> Result<Self, CompressedStateDecodeError> {
        let decoded = Self::decode_canonical(bytes)?;
        let mut replayed = Self::initial();
        for (index, &row) in rows.iter().enumerate() {
            replayed = replayed
                .successor(row)
                .ok_or(CompressedStateDecodeError::InadmissibleReplayRow(index))?;
        }
        if replayed != decoded {
            return Err(CompressedStateDecodeError::ReplayMismatch);
        }
        Ok(decoded)
    }

    /// Constructs a raw canonical pair for exhaustive transition validation.
    ///
    /// This bypass is available only with the crate's `test-support` feature;
    /// scientific callers construct reachable states by transition replay.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[must_use]
    pub const fn from_subspaces_for_test(
        row_span: CanonicalSubspace<F>,
        contraction_span: CanonicalSubspace<F>,
    ) -> Self {
        Self {
            row_span,
            contraction_span,
        }
    }

    fn decode_canonical(bytes: &[u8]) -> Result<Self, CompressedStateDecodeError> {
        if bytes.len() != STATE_ENCODING_LEN {
            return Err(CompressedStateDecodeError::WrongLength {
                expected: STATE_ENCODING_LEN,
                actual: bytes.len(),
            });
        }
        if bytes[0] != CANONICAL_ENCODING_VERSION {
            return Err(CompressedStateDecodeError::UnsupportedVersion(bytes[0]));
        }
        if bytes[1] != F::ORDER {
            return Err(CompressedStateDecodeError::WrongField {
                expected: F::ORDER,
                actual: bytes[1],
            });
        }
        let row_key = bytes[2..12]
            .try_into()
            .expect("state length was checked before extracting the row key");
        let contraction_key = bytes[12..22]
            .try_into()
            .expect("state length was checked before extracting the contraction key");
        Ok(Self {
            row_span: CanonicalSubspace::from_key_bytes(row_key)
                .map_err(CompressedStateDecodeError::InvalidRowSpan)?,
            contraction_span: CanonicalSubspace::from_key_bytes(contraction_key)
                .map_err(CompressedStateDecodeError::InvalidContractionSpan)?,
        })
    }
}

/// One nonzero-multiplicity edge in the compressed state graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompressedTransition<F: SupportedPrimeField> {
    successor: CompressedRankState<F>,
    multiplicity: u64,
}

impl<F: SupportedPrimeField> CompressedTransition<F> {
    /// Returns the edge's canonical successor state.
    #[must_use]
    pub const fn successor(&self) -> &CompressedRankState<F> {
        &self.successor
    }

    /// Returns the number of admissible rows producing the successor.
    #[must_use]
    pub const fn multiplicity(&self) -> u64 {
        self.multiplicity
    }
}

/// Precomputed reachable compressed-state graph for one supported field.
///
/// Construction explores the finite graph once and caches each state's exact
/// transition multiplicities. Repeated propagations reuse those edges.
#[derive(Clone, Debug)]
pub struct CompressedTransitionTable<F: SupportedPrimeField> {
    initial: CompressedRankState<F>,
    transitions: BTreeMap<CompressedRankState<F>, Vec<CompressedTransition<F>>>,
}

impl<F: SupportedPrimeField> Default for CompressedTransitionTable<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: SupportedPrimeField> CompressedTransitionTable<F> {
    /// Precomputes all reachable states and exact transition multiplicities.
    ///
    /// # Complexity
    ///
    /// Visits `q^3` candidate rows for every reachable state and retains only
    /// nonzero-multiplicity edges. Storage is linear in the retained graph.
    #[must_use]
    pub fn new() -> Self {
        let initial = CompressedRankState::initial();
        let mut pending = BTreeSet::from([initial]);
        let mut transitions = BTreeMap::new();
        while let Some(state) = pending.pop_first() {
            if transitions.contains_key(&state) {
                continue;
            }
            let edges = state.transition_multiplicities();
            for edge in &edges {
                if !transitions.contains_key(edge.successor()) {
                    pending.insert(*edge.successor());
                }
            }
            transitions.insert(state, edges);
        }
        Self {
            initial,
            transitions,
        }
    }

    /// Returns the empty-prefix state.
    #[must_use]
    pub const fn initial_state(&self) -> &CompressedRankState<F> {
        &self.initial
    }

    /// Returns the number of reachable canonical states.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.transitions.len()
    }

    /// Returns the cached ordered outgoing transitions for `state`.
    ///
    /// # Panics
    ///
    /// Panics when `state` is not reachable from the empty prefix. This cannot
    /// occur for a state obtained from [`Self::initial_state`], an edge in this
    /// table, or validated transition replay.
    #[must_use]
    pub fn transitions(&self, state: &CompressedRankState<F>) -> &[CompressedTransition<F>] {
        self.transitions
            .get(state)
            .expect("compressed state is not reachable from the empty prefix")
    }

    /// Propagates exact arbitrary-precision counts through `rows` steps.
    ///
    /// The returned `BTreeMap` uses canonical state order, so iteration and
    /// reduction are deterministic.
    ///
    /// # Complexity
    ///
    /// `O(rows * |E|)` arbitrary-precision additions in the retained graph and
    /// `O(|S|)` count storage.
    #[must_use]
    pub fn counts_after_rows(&self, rows: usize) -> BTreeMap<CompressedRankState<F>, BigUint> {
        let mut counts = BTreeMap::from([(self.initial, BigUint::from(1_u8))]);
        for _ in 0..rows {
            let mut next = BTreeMap::new();
            for (state, count) in counts {
                for edge in self.transitions(&state) {
                    let contribution = &count * edge.multiplicity;
                    *next
                        .entry(edge.successor)
                        .or_insert_with(|| BigUint::from(0_u8)) += contribution;
                }
            }
            counts = next;
        }
        counts
    }

    /// Returns the exact three-column permanental-rank-deficiency probability.
    ///
    /// The raw numerator is the sum of valid terminal state counts and the raw
    /// denominator is exactly `q^(3*rows)`. Reduced values remain exact base-ten
    /// integers through [`ExactProbability::reduced_decimal`]; no floating-point
    /// conversion occurs here.
    ///
    /// # Panics
    ///
    /// Panics if `3 * rows` overflows `usize`.
    #[must_use]
    pub fn deficiency_probability(&self, rows: usize) -> ExactProbability {
        let counts = self.counts_after_rows(rows);
        let valid_count: BigUint = counts.into_values().sum();
        let exponent = rows
            .checked_mul(3)
            .expect("three-column matrix exponent overflows usize");
        let matrix_count = arbitrary_power(u64::from(F::ORDER), exponent);
        ExactProbability::from_big_counts(valid_count, matrix_count)
    }
}

/// Computes the exact three-column deficiency probability with a freshly
/// precomputed transition table.
///
/// Reuse [`CompressedTransitionTable`] directly when evaluating more than one
/// row count over the same field.
///
/// # Panics
///
/// Panics if `3 * rows` overflows `usize`, as specified by
/// [`CompressedTransitionTable::deficiency_probability`].
///
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::exact_permanental_rank_deficiency;
/// use gf2_core::gfp::Fp;
/// use num_bigint::BigUint;
///
/// let probability = exact_permanental_rank_deficiency::<Fp<3>>(1_024);
/// assert_eq!(probability.matrix_count(), &BigUint::from(3_u8).pow(3_072));
/// ```
#[must_use]
pub fn exact_permanental_rank_deficiency<F: SupportedPrimeField>(rows: usize) -> ExactProbability {
    CompressedTransitionTable::<F>::new().deficiency_probability(rows)
}

fn all_vectors<F: SupportedPrimeField>() -> impl Iterator<Item = Vector3<F>> {
    (0..F::ORDER).flat_map(|first| {
        (0..F::ORDER).flat_map(move |second| {
            (0..F::ORDER).map(move |third| Vector3 {
                residues: [first, second, third],
                field: PhantomData,
            })
        })
    })
}

fn contraction_residues(left: [u8; 3], right: [u8; 3], order: u8) -> [u8; 3] {
    [
        add_mod(
            multiply_mod(left[1], right[2], order),
            multiply_mod(left[2], right[1], order),
            order,
        ),
        add_mod(
            multiply_mod(left[0], right[2], order),
            multiply_mod(left[2], right[0], order),
            order,
        ),
        add_mod(
            multiply_mod(left[0], right[1], order),
            multiply_mod(left[1], right[0], order),
            order,
        ),
    ]
}

fn add_mod(left: u8, right: u8, order: u8) -> u8 {
    (left + right) % order
}

fn subtract_mod(left: u8, right: u8, order: u8) -> u8 {
    (left + order - right) % order
}

fn negate_mod(value: u8, order: u8) -> u8 {
    (order - value) % order
}

fn multiply_mod(left: u8, right: u8, order: u8) -> u8 {
    (left * right) % order
}

fn inverse_mod(value: u8, order: u8) -> u8 {
    debug_assert_ne!(value, 0);
    let mut result = 1;
    for _ in 0..order - 2 {
        result = multiply_mod(result, value, order);
    }
    result
}

fn arbitrary_power(base: u64, mut exponent: usize) -> BigUint {
    let mut result = BigUint::from(1_u8);
    let mut factor = BigUint::from(base);
    while exponent != 0 {
        if exponent & 1 != 0 {
            result *= &factor;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = &factor * &factor;
        }
    }
    result
}
