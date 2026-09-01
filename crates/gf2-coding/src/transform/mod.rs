//! Transformations that derive one code coordinate space from another.
//!
//! Coordinate positions are zero-based and are always interpreted in the
//! coordinate space of the value being transformed.  A transformation's
//! [`CoordinateMap`] points in the other direction: position `i` in the
//! derived code maps to the corresponding position in its mother code.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::transform::Shortened;
//! use gf2_coding::traits::block::BlockCode;
//! use gf2_coding::{CodeError, LinearBlockCode};
//! use gf2_core::BitMatrix;
//!
//! let mut generator = BitMatrix::zeros(2, 3);
//! generator.set(0, 0, true);
//! generator.set(0, 2, true);
//! generator.set(1, 1, true);
//! generator.set(1, 2, true);
//! let code = LinearBlockCode::new_systematic(generator, None);
//!
//! // The returned code consists of mother codewords whose position 0 is
//! // zero, with that position removed from the exposed coordinate space.
//! let shortened = Shortened::new(code, [0])?;
//! assert_eq!(shortened.n(), 2);
//! assert_eq!(shortened.k(), 1);
//! assert_eq!(shortened.coordinate_map().mother_position(0)?, 1);
//! # Ok::<(), CodeError>(())
//! ```

pub mod coordinate_map;

pub use coordinate_map::CoordinateMap;

use crate::error::CodeError;
use crate::traits::block::{
    BlockCode, BlockEncoder, GeneratorMatrixAccess, SymbolMatrix, SymbolSequence,
};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::{FieldIdentity, FieldVec, FiniteField};
use std::fmt;

/// A linear code obtained by shortening a mother code on a coordinate set.
///
/// For a mother generator matrix `G` and a selected set `S`, the shortened
/// code is the image of
///
/// ```text
/// { m G : (m G)[s] = 0 for every s in S }
/// ```
///
/// The constructor computes a basis of the message vectors satisfying those
/// constraints, forms the resulting generator, and reduces it to RREF.  Its
/// rank is the derived dimension; no dimension subtraction formula is used.
/// The RREF pivot columns are exposed through [`Self::information_set`], so
/// the information coordinates need not be the mother's systematic
/// coordinates.  The stored map uses derived-to-mother positions and retains
/// the original order of the remaining coordinates.
///
/// `C` needs the canonical block-code and generator-matrix traits.  The
/// resulting wrapper owns its generator and therefore encodes through the
/// generic symbol representation supplied by `C`, including nonbinary fields.
///
/// Use [`Self::new`] for a base code.  To shorten an already shortened code,
/// consume it with [`Self::shorten`]; that operation composes the new map
/// with the existing provenance map.
pub struct Shortened<C>
where
    C: BlockCode,
{
    mother: C,
    map: CoordinateMap,
    local_map: CoordinateMap,
    removed: Box<[usize]>,
    generator: FieldMatrix<C::Symbol>,
    information_set: Box<[usize]>,
}

impl<C> Clone for Shortened<C>
where
    C: BlockCode + Clone,
{
    fn clone(&self) -> Self {
        Self {
            mother: self.mother.clone(),
            map: self.map.clone(),
            local_map: self.local_map.clone(),
            removed: self.removed.clone(),
            generator: self.generator.clone(),
            information_set: self.information_set.clone(),
        }
    }
}

impl<C> fmt::Debug for Shortened<C>
where
    C: BlockCode + fmt::Debug,
    C::Symbol: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Shortened")
            .field("mother", &self.mother)
            .field("map", &self.map)
            .field("removed", &self.removed)
            .field("generator", &self.generator)
            .field("information_set", &self.information_set)
            .finish()
    }
}

impl<C> Shortened<C>
where
    C: BlockCode + GeneratorMatrixAccess,
{
    /// Shortens `mother` on `coordinates`.
    ///
    /// `coordinates` is an arbitrary set of positions in `mother`, not a
    /// count and not necessarily a systematic prefix.  Coordinates are
    /// sorted for the deletion map, so their input order has no semantic
    /// effect.  The resulting generator is an RREF basis, and
    /// [`Self::information_set`] contains its pivot columns in derived-code
    /// coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::CoordinateOutOfRange`] for an out-of-range
    /// position and [`CodeError::DuplicateCoordinate`] for a repeated
    /// position.  The complete coordinate set is valid and produces the
    /// zero-length, zero-dimensional boundary code.  Matrix-access errors
    /// from the mother code are propagated unchanged.
    pub fn new(mother: C, coordinates: impl IntoIterator<Item = usize>) -> Result<Self, CodeError> {
        let provenance = CoordinateMap::identity(mother.n());
        build_shortened(mother, coordinates, provenance)
    }

    /// Shortens `mother` using an already-known map from `mother` to an
    /// ultimate ancestor coordinate space.
    ///
    /// This is useful for another derived-code wrapper that carries its own
    /// provenance.  `mother_map.derived_len()` must equal `mother.n()`; the
    /// resulting map is `mother_map` composed with the deletion map.
    /// [`Self::shorten`] is the convenient form for chaining `Shortened`
    /// values.
    pub fn with_coordinate_map(
        mother: C,
        mother_map: CoordinateMap,
        coordinates: impl IntoIterator<Item = usize>,
    ) -> Result<Self, CodeError> {
        if mother_map.derived_len() != mother.n() {
            return Err(CodeError::CoordinateCountMismatch {
                expected: mother.n(),
                actual: mother_map.derived_len(),
            });
        }
        build_shortened(mother, coordinates, mother_map)
    }

    /// Shortens an existing shortened code and composes coordinate
    /// provenance.
    ///
    /// The positions are in the current (inner) code coordinate space.  The
    /// returned map points directly to the original mother of the chain.
    pub fn shorten(
        self,
        coordinates: impl IntoIterator<Item = usize>,
    ) -> Result<Shortened<Self>, CodeError> {
        let provenance = self.map.clone();
        build_shortened(self, coordinates, provenance)
    }

    /// Shortens the first `count` conventional systematic positions.
    ///
    /// The conventional layout is positions `0..k`; this helper is therefore
    /// a shorthand for [`Self::new`] with the set `0..count`.  It delegates to
    /// the coordinate-set constructor, including its validation.
    pub fn shorten_first(mother: C, count: usize) -> Result<Self, CodeError> {
        let coordinates = conventional_prefix(&mother, count)?;
        Self::new(mother, coordinates)
    }

    /// Shortens the last `count` conventional systematic positions.
    ///
    /// The conventional layout is positions `0..k`; this helper therefore
    /// is a shorthand for [`Self::new`] with the set `k-count..k`.  It
    /// delegates to the coordinate-set constructor, including its
    /// validation.
    pub fn shorten_last(mother: C, count: usize) -> Result<Self, CodeError> {
        let coordinates = conventional_suffix(&mother, count)?;
        Self::new(mother, coordinates)
    }

    /// Alias for [`Self::shorten_last`], the usual count-based shortening
    /// convention.
    pub fn by_count(mother: C, count: usize) -> Result<Self, CodeError> {
        Self::shorten_last(mother, count)
    }

    /// Alias for [`Self::shorten_first`].
    pub fn from_systematic_prefix(mother: C, count: usize) -> Result<Self, CodeError> {
        Self::shorten_first(mother, count)
    }

    /// Alias for [`Self::shorten_last`].
    pub fn from_systematic_suffix(mother: C, count: usize) -> Result<Self, CodeError> {
        Self::shorten_last(mother, count)
    }

    /// Returns the mother code by shared reference.
    pub fn mother(&self) -> &C {
        &self.mother
    }

    /// Consumes the wrapper and returns its immediate mother code.
    pub fn into_mother(self) -> C {
        self.mother
    }

    /// Returns the map from derived coordinates to the original mother
    /// coordinates.
    pub fn coordinate_map(&self) -> &CoordinateMap {
        &self.map
    }

    /// Returns the positions removed from the immediate mother coordinate
    /// space, in ascending order.
    pub fn shortened_positions(&self) -> &[usize] {
        &self.removed
    }

    /// Returns the derived coordinates that form an information set.
    ///
    /// The positions are in ascending pivot order and are coordinates of the
    /// shortened code, not positions in the mother code.  Restricting the
    /// RREF generator to these columns gives the identity matrix.
    pub fn information_set(&self) -> &[usize] {
        &self.information_set
    }

    /// Alias for [`Self::information_set`].
    pub fn information_positions(&self) -> &[usize] {
        self.information_set()
    }

    /// Returns a codeword in the immediate mother's coordinate space by
    /// reinserting zeroes at the shortened positions.
    ///
    /// This is the canonical lift used by the shortening contract.  Since
    /// the wrapper's generator was formed from message vectors satisfying
    /// the zero constraints, the returned word is a mother-codeword and is
    /// zero at every position in [`Self::shortened_positions`].
    pub fn extend_codeword(&self, codeword: &C::Symbols) -> Result<C::Symbols, CodeError> {
        if codeword.len() != self.n() {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.n(),
                actual: codeword.len(),
            });
        }

        let mut extended = C::Symbols::zeroed(self.mother.n(), &self.symbol_zero());
        for derived_position in 0..self.n() {
            let mother_position = self
                .local_map
                .mother_position(derived_position)
                .expect("local coordinate map was built from the derived length");
            let symbol = codeword
                .get(derived_position)
                .expect("validated derived codeword length");
            extended.set(mother_position, symbol)?;
        }
        Ok(extended)
    }

    /// Alias for [`Self::extend_codeword`].
    pub fn lift_codeword(&self, codeword: &C::Symbols) -> Result<C::Symbols, CodeError> {
        self.extend_codeword(codeword)
    }
}

impl<C> BlockCode for Shortened<C>
where
    C: BlockCode,
{
    type Symbol = C::Symbol;
    type Symbols = C::Symbols;

    fn symbol_zero(&self) -> Self::Symbol {
        self.mother.symbol_zero()
    }

    fn k(&self) -> usize {
        self.generator.rows()
    }

    fn n(&self) -> usize {
        self.generator.cols()
    }
}

impl<C> BlockEncoder for Shortened<C>
where
    C: BlockCode,
{
    /// Encodes a message with the shortened code's RREF generator.
    ///
    /// # Complexity
    ///
    /// O(`k · n`) field operations.
    fn encode_into(
        &self,
        message: &Self::Symbols,
        codeword: &mut Self::Symbols,
    ) -> Result<(), CodeError> {
        if message.len() != self.k() {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.k(),
                actual: message.len(),
            });
        }
        if codeword.len() != self.n() {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.n(),
                actual: codeword.len(),
            });
        }

        let zero = self.symbol_zero();
        for column in 0..self.n() {
            let mut value = zero.zero_like();
            for row in 0..self.k() {
                let message_symbol = message
                    .get(row)
                    .expect("validated shortened message length");
                value += self.generator.get(row, column) * message_symbol;
            }
            codeword.set(column, value)?;
        }
        Ok(())
    }
}

impl<C> GeneratorMatrixAccess for Shortened<C>
where
    C: BlockCode + GeneratorMatrixAccess,
{
    type GeneratorMatrix = C::GeneratorMatrix;

    fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
        if out.rows() != self.k() || out.cols() != self.n() {
            return Err(CodeError::ShapeMismatch {
                expected_rows: self.k(),
                expected_cols: self.n(),
                actual_rows: out.rows(),
                actual_cols: out.cols(),
            });
        }

        for row in 0..self.k() {
            for column in 0..self.n() {
                let value = self.generator.get(row, column);
                out.set(row, column, value)?;
            }
        }
        Ok(())
    }

    fn is_systematic(&self) -> Result<bool, CodeError> {
        Ok(self.information_set.iter().copied().eq(0..self.k()))
    }
}

fn build_shortened<C>(
    mother: C,
    coordinates: impl IntoIterator<Item = usize>,
    provenance: CoordinateMap,
) -> Result<Shortened<C>, CodeError>
where
    C: BlockCode + GeneratorMatrixAccess,
{
    if provenance.derived_len() != mother.n() {
        return Err(CodeError::CoordinateCountMismatch {
            expected: mother.n(),
            actual: provenance.derived_len(),
        });
    }

    let removed = validate_coordinate_set(mother.n(), coordinates)?;
    let kept = (0..mother.n())
        .filter(|position| removed.binary_search(position).is_err())
        .collect::<Vec<_>>();
    let local_map = CoordinateMap::from_permutation(mother.n(), &kept)?;
    let map = provenance.compose(&local_map)?;

    let mother_generator = mother.generator_matrix()?;
    let zero = mother.symbol_zero();
    let generator = materialize_generator(&mother_generator, &zero);
    let data = derive_shortened_data(&generator, &removed, &kept, &zero);

    Ok(Shortened {
        mother,
        map,
        local_map,
        removed: removed.into_boxed_slice(),
        generator: data.generator,
        information_set: data.information_set,
    })
}

fn validate_coordinate_set(
    length: usize,
    coordinates: impl IntoIterator<Item = usize>,
) -> Result<Vec<usize>, CodeError> {
    let mut coordinates = coordinates.into_iter().collect::<Vec<_>>();
    for &coordinate in &coordinates {
        if coordinate >= length {
            return Err(CodeError::CoordinateOutOfRange { coordinate, length });
        }
    }
    coordinates.sort_unstable();
    for pair in coordinates.windows(2) {
        if pair[0] == pair[1] {
            return Err(CodeError::DuplicateCoordinate {
                coordinate: pair[0],
            });
        }
    }
    // The full coordinate set is valid: it yields the zero-length,
    // zero-dimensional boundary code. Out-of-range and duplicate checks
    // above already cap the count at `length`.
    Ok(coordinates)
}

fn conventional_prefix<C>(mother: &C, count: usize) -> Result<Vec<usize>, CodeError>
where
    C: BlockCode,
{
    if count > mother.k() {
        return Err(CodeError::CoordinateOutOfRange {
            coordinate: count,
            length: mother.k(),
        });
    }
    Ok((0..count).collect())
}

fn conventional_suffix<C>(mother: &C, count: usize) -> Result<Vec<usize>, CodeError>
where
    C: BlockCode,
{
    if count > mother.k() {
        return Err(CodeError::CoordinateOutOfRange {
            coordinate: count,
            length: mother.k(),
        });
    }
    Ok(((mother.k() - count)..mother.k()).collect())
}

struct DerivedLinearData<F: FiniteField> {
    generator: FieldMatrix<F>,
    information_set: Box<[usize]>,
}

fn materialize_generator<F, M>(matrix: &M, zero: &F) -> FieldMatrix<F>
where
    F: FieldIdentity,
    M: SymbolMatrix<F>,
{
    let mut generator = FieldMatrix::new(matrix.rows(), matrix.cols(), zero.clone());
    for row in 0..matrix.rows() {
        for column in 0..matrix.cols() {
            let value = matrix
                .get(row, column)
                .expect("generator matrix shape was validated by its trait");
            generator.set(row, column, value);
        }
    }
    generator
}

fn derive_shortened_data<F>(
    mother_generator: &FieldMatrix<F>,
    removed: &[usize],
    kept: &[usize],
    zero: &F,
) -> DerivedLinearData<F>
where
    F: FiniteField,
{
    let message_dimension = mother_generator.rows();
    let mut constraints = FieldMatrix::new(removed.len(), message_dimension, zero.clone());
    for (constraint, &coordinate) in removed.iter().enumerate() {
        for message_row in 0..message_dimension {
            constraints.set(
                constraint,
                message_row,
                mother_generator.get(message_row, coordinate),
            );
        }
    }

    let null_basis = if removed.is_empty() {
        identity_basis(message_dimension, zero)
    } else {
        constraints.nullspace()
    };

    let mut candidate = FieldMatrix::new(null_basis.len(), kept.len(), zero.clone());
    for (basis_row, message_vector) in null_basis.iter().enumerate() {
        for (derived_column, &mother_column) in kept.iter().enumerate() {
            let mut value = zero.zero_like();
            for message_row in 0..message_dimension {
                value += message_vector.get(message_row).clone()
                    * mother_generator.get(message_row, mother_column);
            }
            candidate.set(basis_row, derived_column, value);
        }
    }

    reduce_to_rref_basis(candidate, zero)
}

fn identity_basis<F>(dimension: usize, zero: &F) -> Vec<FieldVec<F>>
where
    F: FiniteField,
{
    let one = zero.one_like();
    (0..dimension)
        .map(|pivot| {
            let mut vector = FieldVec::zeros_from(dimension, zero);
            vector.set(pivot, one.clone());
            vector
        })
        .collect()
}

fn reduce_to_rref_basis<F>(candidate: FieldMatrix<F>, zero: &F) -> DerivedLinearData<F>
where
    F: FiniteField,
{
    if candidate.rows() == 0 || candidate.cols() == 0 {
        // A candidate with no rows has nothing to reduce; one with no
        // columns (full-coordinate shortening, including rank-deficient
        // mothers with a nonempty nullspace) has rank zero regardless of
        // its row count. Both yield the zero-dimensional boundary data.
        return DerivedLinearData {
            generator: FieldMatrix::new(0, candidate.cols(), zero.clone()),
            information_set: Box::new([]),
        };
    }

    let (_transform, rref) = candidate.rref();
    let rref_zero = rref.get(0, 0).zero_like();
    let mut pivot_columns = Vec::new();
    let mut previous_pivot = None;
    for row in 0..rref.rows() {
        let start = previous_pivot.map_or(0, |pivot| pivot + 1);
        let pivot = (start..rref.cols()).find(|&column| rref.get(row, column) != rref_zero);
        let Some(pivot) = pivot else {
            break;
        };
        pivot_columns.push(pivot);
        previous_pivot = Some(pivot);
    }

    let rank = pivot_columns.len();
    let mut generator = FieldMatrix::new(rank, rref.cols(), zero.clone());
    for row in 0..rank {
        for column in 0..rref.cols() {
            generator.set(row, column, rref.get(row, column));
        }
    }

    DerivedLinearData {
        generator,
        information_set: pivot_columns.into_boxed_slice(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::block::{BlockCode, GeneratorMatrixAccess};
    use crate::LinearBlockCode;
    use gf2_core::field::matrix::FieldMatrix;
    use gf2_core::field::FieldVec;
    use gf2_core::gfp::Fp;
    use gf2_core::{BitMatrix, BitVec};
    use proptest::prelude::*;

    #[derive(Clone, Debug)]
    struct DenseTestCode<F: FiniteField> {
        generator: FieldMatrix<F>,
        zero: F,
    }

    impl<F: FieldIdentity + 'static> BlockCode for DenseTestCode<F> {
        type Symbol = F;
        type Symbols = FieldVec<F>;

        fn symbol_zero(&self) -> Self::Symbol {
            self.zero.clone()
        }

        fn k(&self) -> usize {
            self.generator.rows()
        }

        fn n(&self) -> usize {
            self.generator.cols()
        }
    }

    impl<F: FieldIdentity + 'static> GeneratorMatrixAccess for DenseTestCode<F> {
        type GeneratorMatrix = FieldMatrix<F>;

        fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
            if out.shape() != self.generator.shape() {
                return Err(CodeError::ShapeMismatch {
                    expected_rows: self.k(),
                    expected_cols: self.n(),
                    actual_rows: out.rows(),
                    actual_cols: out.cols(),
                });
            }
            for row in 0..self.k() {
                for column in 0..self.n() {
                    out.set(row, column, self.generator.get(row, column));
                }
            }
            Ok(())
        }

        fn is_systematic(&self) -> Result<bool, CodeError> {
            Ok(false)
        }
    }

    fn direct_shortened_rank<F>(generator: &FieldMatrix<F>, removed: &[usize]) -> usize
    where
        F: FiniteField,
    {
        let zero = if generator.rows() > 0 && generator.cols() > 0 {
            generator.get(0, 0).zero_like()
        } else {
            F::zero_hint().expect("test fields have a static zero")
        };
        let kept = (0..generator.cols())
            .filter(|column| !removed.contains(column))
            .collect::<Vec<_>>();
        let mut constraints = FieldMatrix::new(removed.len(), generator.rows(), zero.clone());
        for (row, &column) in removed.iter().enumerate() {
            for message_row in 0..generator.rows() {
                constraints.set(row, message_row, generator.get(message_row, column));
            }
        }
        let basis = if removed.is_empty() {
            identity_basis(generator.rows(), &zero)
        } else {
            constraints.nullspace()
        };
        let mut result = FieldMatrix::new(basis.len(), kept.len(), zero.clone());
        for (row, vector) in basis.iter().enumerate() {
            for (column, &mother_column) in kept.iter().enumerate() {
                let mut value = zero.zero_like();
                for message_row in 0..generator.rows() {
                    value +=
                        vector.get(message_row).clone() * generator.get(message_row, mother_column);
                }
                result.set(row, column, value);
            }
        }
        result.rank()
    }

    fn fp5_matrix(rows: usize, cols: usize, values: &[u8]) -> FieldMatrix<Fp<5>> {
        let mut matrix = FieldMatrix::zeros(rows, cols);
        for row in 0..rows {
            for column in 0..cols {
                matrix.set(
                    row,
                    column,
                    Fp::<5>::new(values[row * cols + column] as u64),
                );
            }
        }
        matrix
    }

    #[test]
    fn shortening_uses_a_non_systematic_information_set() {
        let mut generator = BitMatrix::zeros(2, 3);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        let code = LinearBlockCode::new_systematic(generator, None);
        let shortened = Shortened::new(code, [1]).unwrap();

        assert_eq!(shortened.k(), 1);
        assert_eq!(shortened.n(), 2);
        assert_eq!(shortened.information_set(), &[1]);
        assert!(!shortened.is_systematic().unwrap());
        assert_eq!(shortened.coordinate_map().mother_position(0).unwrap(), 0);
        assert_eq!(shortened.coordinate_map().mother_position(1).unwrap(), 2);
    }

    #[test]
    fn shortened_codeword_lifts_to_a_zero_constrained_mother_word() {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 2, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let shortened = Shortened::new(mother.clone(), [0]).unwrap();
        let mut message = BitVec::zeros(1);
        message.set(0, true);
        let word = shortened.encode(&message).unwrap();
        let lifted = shortened.extend_codeword(&word).unwrap();

        assert!(!lifted.get(0));
        assert_eq!(lifted.len(), mother.n());
        let mut found = false;
        for candidate in 0..(1usize << mother.k()) {
            let mut mother_message = BitVec::zeros(mother.k());
            for bit in 0..mother.k() {
                mother_message.set(bit, (candidate >> bit) & 1 == 1);
            }
            if mother.encode(&mother_message).unwrap() == lifted {
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn chained_shortening_composes_maps_and_matches_combined_set() {
        let mut generator = BitMatrix::zeros(3, 6);
        generator.set(0, 0, true);
        generator.set(0, 3, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        generator.set(1, 4, true);
        generator.set(2, 2, true);
        generator.set(2, 4, true);
        generator.set(2, 5, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let first = Shortened::new(mother.clone(), [1]).unwrap();
        let chained = first.shorten([2]).unwrap();
        let combined = Shortened::new(mother, [1, 3]).unwrap();

        assert_eq!(chained.n(), combined.n());
        assert_eq!(chained.k(), combined.k());
        assert_eq!(chained.information_set(), combined.information_set());
        for position in 0..chained.n() {
            assert_eq!(
                chained.coordinate_map().mother_position(position),
                combined.coordinate_map().mother_position(position)
            );
        }
    }

    #[test]
    fn count_helpers_delegate_to_coordinate_sets() {
        let mut generator = BitMatrix::zeros(3, 5);
        for diagonal in 0..3 {
            generator.set(diagonal, diagonal, true);
        }
        let first =
            Shortened::shorten_first(LinearBlockCode::new_systematic(generator.clone(), None), 2)
                .unwrap();
        let explicit_first = Shortened::new(
            LinearBlockCode::new_systematic(generator.clone(), None),
            [0, 1],
        )
        .unwrap();
        let last =
            Shortened::shorten_last(LinearBlockCode::new_systematic(generator.clone(), None), 2)
                .unwrap();
        let explicit_last =
            Shortened::new(LinearBlockCode::new_systematic(generator, None), [1, 2]).unwrap();

        assert_eq!(
            first.generator_matrix().unwrap(),
            explicit_first.generator_matrix().unwrap()
        );
        assert_eq!(
            last.generator_matrix().unwrap(),
            explicit_last.generator_matrix().unwrap()
        );
    }

    #[test]
    fn invalid_coordinate_sets_use_code_errors() {
        let mut generator = BitMatrix::zeros(1, 4);
        generator.set(0, 0, true);
        let code = LinearBlockCode::new_systematic(generator, None);

        assert!(matches!(
            Shortened::new(code.clone(), [4]),
            Err(CodeError::CoordinateOutOfRange { .. })
        ));
        assert!(matches!(
            Shortened::new(code.clone(), [1, 1]),
            Err(CodeError::DuplicateCoordinate { .. })
        ));
        // Shortening on the complete coordinate set is VALID: it produces the
        // zero-length, zero-dimensional boundary code.
        let boundary = Shortened::new(code, [0, 1, 2, 3]).unwrap();
        assert_eq!(boundary.n(), 0);
        assert_eq!(boundary.k(), 0);
        assert!(boundary.information_set().is_empty());
    }

    #[test]
    fn full_coordinate_shortening_of_a_rank_deficient_generic_code_is_the_boundary_code() {
        // A rank-deficient generator leaves a nonempty nullspace, so the
        // constraint solve produces candidate rows even though no column
        // survives; the result must still be the zero-dimensional code.
        let values = [1u8, 2, 0, 1, 2, 4, 0, 2, 3, 1, 0, 3];
        let generator = fp5_matrix(3, 4, &values);
        let code = DenseTestCode {
            generator,
            zero: Fp::<5>::new(0),
        };
        let boundary = Shortened::new(code, [0, 1, 2, 3]).unwrap();
        assert_eq!(boundary.n(), 0);
        assert_eq!(boundary.k(), 0);
        assert!(boundary.information_set().is_empty());
    }

    proptest! {
        #[test]
        fn binary_dimension_matches_direct_generator_computation(
            rows in 1usize..=4,
            cols in 2usize..=7,
            values in prop::collection::vec(any::<bool>(), 8..=28),
            mask in any::<u8>(),
        ) {
            let mut generator = BitMatrix::zeros(rows, cols);
            for row in 0..rows {
                for column in 0..cols {
                    generator.set(row, column, values[(row * cols + column) % values.len()]);
                }
            }
            let removed = (0..cols)
                .filter(|column| (mask >> (column % 8)) & 1 == 1)
                .take(cols - 1)
                .collect::<Vec<_>>();
            let code = LinearBlockCode::new_systematic(generator.clone(), None);
            let shortened = Shortened::new(code, removed.clone()).unwrap();
            let mut dense = FieldMatrix::zeros(rows, cols);
            for row in 0..rows {
                for column in 0..cols {
                    dense.set(row, column, Fp::<2>::new(generator.get(row, column) as u64));
                }
            }
            prop_assert_eq!(shortened.k(), direct_shortened_rank(&dense, &removed));
        }

        #[test]
        fn nonbinary_dimension_matches_direct_generator_computation(
            rows in 1usize..=4,
            cols in 2usize..=7,
            values in prop::collection::vec(0u8..5, 8..=28),
            mask in any::<u8>(),
        ) {
            let mut values = values;
            values.resize(rows * cols, 0);
            let generator = fp5_matrix(rows, cols, &values);
            let removed = (0..cols)
                .filter(|column| (mask >> (column % 8)) & 1 == 1)
                .take(cols - 1)
                .collect::<Vec<_>>();
            let code = DenseTestCode { generator: generator.clone(), zero: Fp::<5>::new(0) };
            let shortened = Shortened::new(code, removed.clone()).unwrap();
            prop_assert_eq!(shortened.k(), direct_shortened_rank(&generator, &removed));
        }
    }
}
