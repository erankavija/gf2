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
    BlockCode, BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess, SymbolMatrix,
    SymbolSequence,
};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::{FieldIdentity, FieldVec, FiniteField};
use std::fmt;

/// A linear code obtained by appending one coordinate whose value makes the
/// complete symbol sum zero.
///
/// The appended coordinate is the final coordinate, at position `n` in the
/// derived coordinate space.  Encoding first delegates to the mother and
/// then appends `-sum(codeword)`.  Thus the binary specialization appends the
/// overall parity bit and agrees with the legacy extended-BCH convention,
/// whose parity bit is the last `[message | parity | overall-parity]`
/// coordinate.
///
/// The wrapper preserves the mother's dimension: its parameters are
/// `(n + 1, k)`.  It does not compute a minimum distance.  Hamming distance
/// can stay the same or increase by one; for a binary mother with odd minimum
/// distance, the even-weight extension increases it by one.
///
/// The returned [`Self::coordinate_map`] is complete: it contains all `n + 1`
/// derived positions in order.  The appended position is represented as a
/// fresh coordinate (`None`) because it has no mother preimage; query it with
/// [`CoordinateMap::mother_position_opt`].  This complete representation also
/// lets callers compose provenance through another derived wrapper via
/// [`Self::with_coordinate_map`].
///
/// # Examples
///
/// ```
/// use gf2_coding::transform::Extended;
/// use gf2_coding::traits::block::{BlockCode, BlockEncoder};
/// use gf2_coding::{CodeError, LinearBlockCode};
/// use gf2_core::BitVec;
///
/// let mother = LinearBlockCode::hamming(3);
/// let extended = Extended::new(mother)?;
/// let message = BitVec::ones(extended.k());
/// let codeword = extended.encode(&message)?;
/// assert_eq!(codeword.len(), extended.n());
/// assert_eq!(codeword.count_ones() % 2, 0);
/// assert_eq!(extended.extension_position(), extended.n() - 1);
/// # Ok::<(), CodeError>(())
/// ```
pub struct Extended<C>
where
    C: BlockCode,
{
    mother: C,
    map: CoordinateMap,
}

impl<C> Clone for Extended<C>
where
    C: BlockCode + Clone,
{
    fn clone(&self) -> Self {
        Self {
            mother: self.mother.clone(),
            map: self.map.clone(),
        }
    }
}

impl<C> fmt::Debug for Extended<C>
where
    C: BlockCode + fmt::Debug,
    C::Symbol: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Extended")
            .field("mother", &self.mother)
            .field("map", &self.map)
            .finish()
    }
}

impl<C> Extended<C>
where
    C: BlockCode,
{
    /// Extends `mother` by one final zero-sum coordinate.
    ///
    /// The constructor rejects a mother length that cannot be increased by
    /// one in the `usize` representation.  The coordinate map is the
    /// identity over the mother's positions followed by one fresh position.
    pub fn new(mother: C) -> Result<Self, CodeError> {
        mother
            .n()
            .checked_add(1)
            .ok_or(CodeError::UnsupportedSize {
                size: mother.n() as u128 + 1,
            })?;
        Ok(Self {
            map: CoordinateMap::identity(mother.n()).append_fresh(1)?,
            mother,
        })
    }

    /// Extends `mother` using an existing map from the mother's coordinates
    /// to an ultimate ancestor coordinate space.
    ///
    /// `mother_map` must contain one entry for every inherited mother
    /// coordinate.  Its derived length must therefore be `mother.n()` before
    /// the fresh extension coordinate is appended to it.
    pub fn with_coordinate_map(mother: C, mother_map: CoordinateMap) -> Result<Self, CodeError> {
        mother
            .n()
            .checked_add(1)
            .ok_or(CodeError::UnsupportedSize {
                size: mother.n() as u128 + 1,
            })?;
        if mother_map.derived_len() != mother.n() {
            return Err(CodeError::CoordinateCountMismatch {
                expected: mother.n(),
                actual: mother_map.derived_len(),
            });
        }
        Ok(Self {
            mother,
            map: mother_map.append_fresh(1)?,
        })
    }

    /// Returns the mother code by shared reference.
    pub fn mother(&self) -> &C {
        &self.mother
    }

    /// Consumes the wrapper and returns its immediate mother code.
    pub fn into_mother(self) -> C {
        self.mother
    }

    /// Returns the complete map from derived coordinates to mother
    /// coordinates.  The extension position is included and is fresh, so
    /// [`CoordinateMap::mother_position_opt`] returns `Ok(None)` for it.
    pub fn coordinate_map(&self) -> &CoordinateMap {
        &self.map
    }

    /// Returns the final coordinate containing the zero-sum symbol.
    pub fn extension_position(&self) -> usize {
        self.mother.n()
    }

    /// Shortens the extended code and composes complete coordinate
    /// provenance, retaining any fresh coordinates that remain exposed.
    pub fn shorten(
        self,
        coordinates: impl IntoIterator<Item = usize>,
    ) -> Result<Shortened<Self>, CodeError>
    where
        C: GeneratorMatrixAccess,
    {
        let provenance = self.map.clone();
        build_shortened(self, coordinates, provenance)
    }

    /// Punctures the extended code and composes complete coordinate
    /// provenance, retaining any fresh coordinates that remain exposed.
    pub fn puncture(
        self,
        coordinates: impl IntoIterator<Item = usize>,
    ) -> Result<Punctured<Self>, CodeError>
    where
        C: GeneratorMatrixAccess,
    {
        let provenance = self.map.clone();
        build_punctured(self, coordinates, provenance)
    }
}

impl<C> BlockCode for Extended<C>
where
    C: BlockCode,
{
    type Symbol = C::Symbol;
    type Symbols = C::Symbols;

    fn symbol_zero(&self) -> Self::Symbol {
        self.mother.symbol_zero()
    }

    fn k(&self) -> usize {
        self.mother.k()
    }

    fn n(&self) -> usize {
        self.mother.n() + 1
    }
}

impl<C> BlockEncoder for Extended<C>
where
    C: BlockCode + BlockEncoder,
{
    /// Encodes with the mother and appends the additive inverse of its symbol
    /// sum.
    ///
    /// # Complexity
    ///
    /// O(`mother.encode` + `n`) field operations and one temporary `n`-symbol
    /// buffer.
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
        let mut mother_codeword = C::Symbols::zeroed(self.mother.n(), &zero);
        self.mother.encode_into(message, &mut mother_codeword)?;

        let mut sum = zero.zero_like();
        for position in 0..self.mother.n() {
            let symbol = mother_codeword
                .get(position)
                .expect("validated mother codeword length");
            sum += &symbol;
            codeword.set(position, symbol)?;
        }
        codeword.set(self.extension_position(), -sum)?;
        Ok(())
    }
}

impl<C> GeneratorMatrixAccess for Extended<C>
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

        let mother_generator = self.mother.generator_matrix()?;
        for row in 0..self.k() {
            let mut sum = self.symbol_zero().zero_like();
            for column in 0..self.mother.n() {
                let value = mother_generator
                    .get(row, column)
                    .expect("validated mother generator shape");
                sum += &value;
                out.set(row, column, value)?;
            }
            out.set(row, self.extension_position(), -sum)?;
        }
        Ok(())
    }

    fn is_systematic(&self) -> Result<bool, CodeError> {
        self.mother.is_systematic()
    }

    /// Delegates to the mother, whose coordinates keep their positions: the
    /// zero-sum coordinate is appended after all of them.
    fn has_canonical_message_order(&self) -> Result<bool, CodeError> {
        self.mother.has_canonical_message_order()
    }
}

/// Provides the canonical parity-check matrix for a zero-sum extension.
///
/// Each mother parity-check row is copied into the inherited coordinates and
/// receives a zero in the fresh final coordinate.  The final row contains
/// the multiplicative identity of the symbol field in every coordinate, so
/// it checks the zero-sum extension constraint.  The result therefore has
/// the same coordinate convention as [`Extended::encode_into`]: the fresh
/// extension coordinate is at position `self.mother.n()`, the final position
/// (`self.n() - 1`) of the derived code.
impl<C> ParityCheckMatrixAccess for Extended<C>
where
    C: BlockCode + ParityCheckMatrixAccess,
{
    type ParityCheckMatrix = C::ParityCheckMatrix;

    fn parity_check_rows(&self) -> usize {
        self.mother.parity_check_rows() + 1
    }

    fn parity_check_matrix_into(&self, out: &mut Self::ParityCheckMatrix) -> Result<(), CodeError> {
        if out.rows() != self.parity_check_rows() || out.cols() != self.n() {
            return Err(CodeError::ShapeMismatch {
                expected_rows: self.parity_check_rows(),
                expected_cols: self.n(),
                actual_rows: out.rows(),
                actual_cols: out.cols(),
            });
        }

        let mother_parity = self.mother.parity_check_matrix()?;
        let zero = self.symbol_zero();
        for row in 0..self.mother.parity_check_rows() {
            for column in 0..self.mother.n() {
                let value = mother_parity
                    .get(row, column)
                    .expect("mother parity-check shape matches its contract");
                out.set(row, column, value)?;
            }
            out.set(row, self.extension_position(), zero.zero_like())?;
        }

        let one = zero.one_like();
        let extension_row = self.mother.parity_check_rows();
        for column in 0..self.n() {
            out.set(extension_row, column, one.clone())?;
        }
        Ok(())
    }
}

/// How a [`Shortened`] value derives its code from its mother.
///
/// [`Shortened::derivation`] reports it, so which construction a value took
/// is an observable property rather than something inferred from timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShortenedDerivation {
    /// The mother's code restricted to the messages that are zero on the
    /// removed message positions.
    SystematicRestriction,
    /// A reduced basis of the mother codewords that vanish on the removed
    /// coordinates.
    RankDerived,
}

/// The data one [`ShortenedDerivation`] stores.
#[derive(Clone, Debug)]
enum ShortenedData<F: FiniteField> {
    /// The RREF generator of the derived code.
    RankDerived { generator: FieldMatrix<F> },
    /// The mother message positions the derived code keeps, in ascending
    /// order.  Entry `i` is the mother position carrying derived message
    /// symbol `i`.
    SystematicRestriction {
        kept_message_positions: Box<[usize]>,
    },
}

/// A linear code obtained by shortening a mother code on a coordinate set.
///
/// For a mother generator matrix `G` and a selected set `S`, the shortened
/// code is the image of
///
/// ```text
/// { m G : (m G)[s] = 0 for every s in S }
/// ```
///
/// Two derivations produce that code.  [`Self::derivation`] reports which one
/// a value holds, and both agree on the coordinate map, the dimension, the
/// information set, the generator, and every encoded codeword; their
/// parity-check matrices are bases of one dual space, equal whenever the
/// mother's own check matrix is the canonical one.  The stored map uses
/// derived-to-mother positions and retains the original order of the
/// remaining coordinates.
///
/// [`ShortenedDerivation::SystematicRestriction`] applies when the mother
/// carries message symbol `i` at coordinate `i` — it reports both
/// [`GeneratorMatrixAccess::is_systematic`] and
/// [`GeneratorMatrixAccess::has_canonical_message_order`] — and every removed
/// coordinate is below `k`.  Coordinate `s` of a mother codeword is then
/// message symbol `s`, so the constraints read `m[s] = 0` and the shortened
/// code is the mother's restricted to the messages vanishing on the removed
/// positions.  Its dimension is `k - |S|`, its length `n - |S|`, and its
/// information set the kept message positions in order, `0..k - |S|`.  The
/// construction reads the mother's parameters and the coordinate set alone,
/// so it costs O(`n`) beyond the mother's own construction: it materializes
/// no generator, solves no nullspace, and reduces nothing to RREF.
///
/// [`ShortenedDerivation::RankDerived`] is the derivation for every other
/// coordinate set and mother.  It materializes the mother generator, solves
/// the nullspace of the removed-coordinate constraints, forms the resulting
/// generator, and reduces it to RREF.  That rank is the derived dimension; no
/// dimension subtraction formula is used.  The RREF pivot columns are exposed
/// through [`Self::information_set`], so the information coordinates need not
/// be the mother's systematic coordinates.  The construction costs the
/// mother's `k × n` materialization and O(`k² n`) field operations, and the
/// value retains a dense `k' × n'` generator.
///
/// `C` needs the canonical block-code and generator-matrix traits, and
/// encoding needs the mother's [`BlockEncoder`].  Both derivations carry the
/// generic symbol representation supplied by `C`, including nonbinary fields.
/// When an extension coordinate is retained while shortening an [`Extended`]
/// value, its map entry is preserved as a fresh (`None`) coordinate.
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
    information_set: Box<[usize]>,
    data: ShortenedData<C::Symbol>,
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
            information_set: self.information_set.clone(),
            data: self.data.clone(),
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
            .field("information_set", &self.information_set)
            .field("data", &self.data)
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
    /// effect.  [`Self::information_set`] contains the information
    /// coordinates of the result in derived-code coordinates.
    ///
    /// The mother's [`GeneratorMatrixAccess::is_systematic`] and
    /// [`GeneratorMatrixAccess::has_canonical_message_order`] reports and
    /// the coordinate set select the derivation, as
    /// [`ShortenedDerivation`] describes; the mother's generator is
    /// materialized only for [`ShortenedDerivation::RankDerived`].
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
    /// the coordinate-set constructor, including its validation.  Its set
    /// lies below `k` by construction, so a mother carrying its message
    /// symbols in coordinates `0..k` takes
    /// [`ShortenedDerivation::SystematicRestriction`].
    pub fn shorten_first(mother: C, count: usize) -> Result<Self, CodeError> {
        let coordinates = conventional_prefix(&mother, count)?;
        Self::new(mother, coordinates)
    }

    /// Shortens the last `count` conventional systematic positions.
    ///
    /// The conventional layout is positions `0..k`; this helper therefore
    /// is a shorthand for [`Self::new`] with the set `k-count..k`.  It
    /// delegates to the coordinate-set constructor, including its
    /// validation.  Its set lies below `k` by construction, so a mother
    /// carrying its message symbols in coordinates `0..k` takes
    /// [`ShortenedDerivation::SystematicRestriction`].
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

    /// Extends this shortened code while retaining its complete coordinate
    /// provenance, including any fresh coordinates from an earlier extension.
    pub fn extend(self) -> Result<Extended<Self>, CodeError> {
        let provenance = self.map.clone();
        Extended::with_coordinate_map(self, provenance)
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

    /// Returns the derivation this value holds.
    ///
    /// [`ShortenedDerivation`] describes when each one applies and what it
    /// costs.
    pub fn derivation(&self) -> ShortenedDerivation {
        match &self.data {
            ShortenedData::RankDerived { .. } => ShortenedDerivation::RankDerived,
            ShortenedData::SystematicRestriction { .. } => {
                ShortenedDerivation::SystematicRestriction
            }
        }
    }

    /// Returns the derived coordinates that form an information set.
    ///
    /// The positions are ascending and are coordinates of the shortened
    /// code, not positions in the mother code.  Restricting the generator to
    /// these columns gives the identity matrix, so they are the RREF pivots
    /// under [`ShortenedDerivation::RankDerived`] and the kept message
    /// positions in order, `0..k()`, under
    /// [`ShortenedDerivation::SystematicRestriction`].
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
    /// This is the canonical lift used by the shortening contract.  Both
    /// derivations produce codewords of mother words that satisfy the zero
    /// constraints, so the returned word is a mother codeword and is zero at
    /// every position in [`Self::shortened_positions`].
    pub fn extend_codeword(&self, codeword: &C::Symbols) -> Result<C::Symbols, CodeError> {
        if codeword.len() != self.n() {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.n(),
                actual: codeword.len(),
            });
        }

        let mut extended = C::Symbols::zeroed(self.mother.n(), &self.symbol_zero());
        for derived_position in 0..self.n() {
            let mother_position = self.mother_position(derived_position);
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

impl<C> Shortened<C>
where
    C: BlockCode,
{
    /// Returns the immediate mother coordinate of a derived position.
    ///
    /// # Panics
    ///
    /// Panics if `derived_position` is at or above [`BlockCode::n`], which
    /// the deletion map covers by construction.
    fn mother_position(&self, derived_position: usize) -> usize {
        self.local_map
            .mother_position(derived_position)
            .expect("the local deletion map covers every derived position")
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

    /// Returns the derived dimension.
    ///
    /// The information set holds one coordinate per basis vector under both
    /// derivations, so its length is the dimension.
    fn k(&self) -> usize {
        self.information_set.len()
    }

    /// Returns the derived length, the number of coordinates the deletion
    /// map keeps.
    fn n(&self) -> usize {
        self.local_map.derived_len()
    }
}

impl<C> BlockEncoder for Shortened<C>
where
    C: BlockEncoder,
{
    /// Encodes a message under the value's [`ShortenedDerivation`].
    ///
    /// [`ShortenedDerivation::RankDerived`] multiplies the message by the
    /// stored RREF generator.  [`ShortenedDerivation::SystematicRestriction`]
    /// writes the message symbols at the kept message positions of an
    /// otherwise zero mother message, encodes that with the mother, and
    /// drops the removed coordinates.
    ///
    /// # Complexity
    ///
    /// O(`k · n`) field operations for the rank-derived path.  One mother
    /// encode plus O(`n`) symbol moves for the systematic restriction, in
    /// two buffers of the mother's size.
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
        match &self.data {
            ShortenedData::RankDerived { generator } => {
                for column in 0..self.n() {
                    let mut value = zero.zero_like();
                    for row in 0..self.k() {
                        let message_symbol = message
                            .get(row)
                            .expect("validated shortened message length");
                        value += generator.get(row, column) * message_symbol;
                    }
                    codeword.set(column, value)?;
                }
            }
            ShortenedData::SystematicRestriction {
                kept_message_positions,
            } => {
                let mut mother_message = Self::Symbols::zeroed(self.mother.k(), &zero);
                for (derived_index, &mother_index) in kept_message_positions.iter().enumerate() {
                    let symbol = message
                        .get(derived_index)
                        .expect("validated shortened message length");
                    mother_message.set(mother_index, symbol)?;
                }
                let mut mother_codeword = Self::Symbols::zeroed(self.mother.n(), &zero);
                self.mother
                    .encode_into(&mother_message, &mut mother_codeword)?;
                for derived_position in 0..self.n() {
                    let mother_position = self.mother_position(derived_position);
                    let symbol = mother_codeword
                        .get(mother_position)
                        .expect("the deletion map ranges over the mother coordinates");
                    codeword.set(derived_position, symbol)?;
                }
            }
        }
        Ok(())
    }
}

impl<C> GeneratorMatrixAccess for Shortened<C>
where
    C: BlockCode + GeneratorMatrixAccess,
{
    type GeneratorMatrix = C::GeneratorMatrix;

    /// Writes the `k() × n()` generator.
    ///
    /// [`ShortenedDerivation::RankDerived`] copies the stored RREF
    /// generator.  [`ShortenedDerivation::SystematicRestriction`] holds no
    /// generator, so it materializes the mother's `k × n` matrix into one
    /// temporary buffer of the mother's size and copies the kept rows
    /// restricted to the kept columns out of it.  That temporary is the only
    /// memory beyond `out`, and it is released before the call returns.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::ShapeMismatch`] when `out` is not `k() × n()`,
    /// and propagates the mother's matrix-access errors.
    ///
    /// # Complexity
    ///
    /// O(`k · n`) symbol moves for the rank-derived path.  The mother's own
    /// materialization plus O(`k · n`) symbol moves for the systematic
    /// restriction.
    fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
        if out.rows() != self.k() || out.cols() != self.n() {
            return Err(CodeError::ShapeMismatch {
                expected_rows: self.k(),
                expected_cols: self.n(),
                actual_rows: out.rows(),
                actual_cols: out.cols(),
            });
        }

        match &self.data {
            ShortenedData::RankDerived { generator } => {
                for row in 0..self.k() {
                    for column in 0..self.n() {
                        let value = generator.get(row, column);
                        out.set(row, column, value)?;
                    }
                }
            }
            ShortenedData::SystematicRestriction {
                kept_message_positions,
            } => {
                let mut mother_generator = Self::GeneratorMatrix::zeroed(
                    self.mother.k(),
                    self.mother.n(),
                    &self.symbol_zero(),
                );
                self.mother.generator_matrix_into(&mut mother_generator)?;
                for (row, &mother_row) in kept_message_positions.iter().enumerate() {
                    for column in 0..self.n() {
                        let value = mother_generator
                            .get(mother_row, self.mother_position(column))
                            .expect("the mother generator has the mother's shape");
                        out.set(row, column, value)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn is_systematic(&self) -> Result<bool, CodeError> {
        Ok(self.information_set.iter().copied().eq(0..self.k()))
    }
}

/// Provides a parity-check matrix for a shortened code.
///
/// [`ShortenedDerivation::SystematicRestriction`] deletes the removed
/// columns from the mother's check matrix.  Every derived codeword is a
/// mother codeword that is zero on those coordinates, so the deleted columns
/// contribute nothing to a syndrome, and the surviving rows stay independent
/// because the removed coordinates are message coordinates while the
/// mother's check matrix is invertible on the parity ones.  The row count is
/// therefore the mother's.
///
/// [`ShortenedDerivation::RankDerived`] builds the dual of its own RREF
/// generator: one row per non-pivot column `j`, carrying the field identity
/// at `j` and the negated generator column `j` at the pivot coordinates.
///
/// Each derivation writes a basis of the same dual space.  The two bases
/// coincide when the mother's check matrix is the canonical
/// `[-Pᵀ | I]`, as the repository's matrix contract writes it; a mother
/// carrying another basis passes that basis on to the systematic
/// restriction.
impl<C> ParityCheckMatrixAccess for Shortened<C>
where
    C: ParityCheckMatrixAccess,
{
    type ParityCheckMatrix = C::ParityCheckMatrix;

    fn parity_check_rows(&self) -> usize {
        match &self.data {
            ShortenedData::RankDerived { .. } => self.redundancy(),
            ShortenedData::SystematicRestriction { .. } => self.mother.parity_check_rows(),
        }
    }

    /// Writes the `parity_check_rows() × n()` check matrix.
    ///
    /// The systematic restriction materializes the mother's check matrix
    /// into one temporary buffer of the mother's size, `(n - k) × n`, and
    /// copies its kept columns out of it.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::ShapeMismatch`] when `out` is not
    /// `parity_check_rows() × n()`, and propagates the mother's
    /// matrix-access errors, including
    /// [`CodeError::CapabilityUnavailable`] for a mother that holds no
    /// check matrix.
    ///
    /// # Complexity
    ///
    /// O((`n - k`) `· n`) symbol moves, plus the mother's own
    /// materialization for the systematic restriction.
    fn parity_check_matrix_into(&self, out: &mut Self::ParityCheckMatrix) -> Result<(), CodeError> {
        let rows = self.parity_check_rows();
        if out.rows() != rows || out.cols() != self.n() {
            return Err(CodeError::ShapeMismatch {
                expected_rows: rows,
                expected_cols: self.n(),
                actual_rows: out.rows(),
                actual_cols: out.cols(),
            });
        }

        match &self.data {
            ShortenedData::RankDerived { generator } => {
                let zero = self.symbol_zero();
                let one = zero.one_like();
                let free_columns = (0..self.n())
                    .filter(|column| self.information_set.binary_search(column).is_err());
                for (row, free_column) in free_columns.enumerate() {
                    for column in 0..self.n() {
                        let value = if column == free_column {
                            one.clone()
                        } else {
                            match self.information_set.binary_search(&column) {
                                Ok(pivot) => -generator.get(pivot, free_column),
                                Err(_) => zero.zero_like(),
                            }
                        };
                        out.set(row, column, value)?;
                    }
                }
            }
            ShortenedData::SystematicRestriction { .. } => {
                let mut mother_check = Self::ParityCheckMatrix::zeroed(
                    self.mother.parity_check_rows(),
                    self.mother.n(),
                    &self.symbol_zero(),
                );
                self.mother.parity_check_matrix_into(&mut mother_check)?;
                for row in 0..rows {
                    for column in 0..self.n() {
                        let value = mother_check
                            .get(row, self.mother_position(column))
                            .expect("the mother check matrix has the mother's shape");
                        out.set(row, column, value)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// A linear code obtained by puncturing a mother code on a coordinate set.
///
/// For a mother generator matrix `G` and a selected set `S`, the punctured
/// code is the projection of the mother code onto the coordinates not in
/// `S`.  The projected generator is reduced to RREF, so its rank is the
/// derived dimension and [`Self::information_set`] reports pivots of the
/// projected code rather than the mother's systematic positions.  In
/// particular, puncturing can reduce the dimension when distinct mother
/// messages have the same projection.
///
/// `C` needs the canonical block-code and generator-matrix traits.  The
/// resulting wrapper owns its generator and therefore encodes through the
/// generic symbol representation supplied by `C`, including nonbinary
/// fields.  Its coordinate map preserves the original order of the kept
/// coordinates and composes through [`Self::puncture`].  When the mother is
/// an [`Extended`] value, a retained fresh coordinate remains explicitly
/// represented in the map.
///
/// The complete coordinate set is valid: it produces the zero-length,
/// zero-dimensional boundary code with an empty information set.
pub struct Punctured<C>
where
    C: BlockCode,
{
    mother: C,
    map: CoordinateMap,
    removed: Box<[usize]>,
    generator: FieldMatrix<C::Symbol>,
    information_set: Box<[usize]>,
}

impl<C> Clone for Punctured<C>
where
    C: BlockCode + Clone,
{
    fn clone(&self) -> Self {
        Self {
            mother: self.mother.clone(),
            map: self.map.clone(),
            removed: self.removed.clone(),
            generator: self.generator.clone(),
            information_set: self.information_set.clone(),
        }
    }
}

impl<C> fmt::Debug for Punctured<C>
where
    C: BlockCode + fmt::Debug,
    C::Symbol: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Punctured")
            .field("mother", &self.mother)
            .field("map", &self.map)
            .field("removed", &self.removed)
            .field("generator", &self.generator)
            .field("information_set", &self.information_set)
            .finish()
    }
}

impl<C> Punctured<C>
where
    C: BlockCode + GeneratorMatrixAccess,
{
    /// Punctures `mother` on `coordinates`.
    ///
    /// `coordinates` is an arbitrary set of positions in `mother`, not a
    /// count and not necessarily a systematic prefix.  Coordinates are
    /// sorted for the deletion map, so their input order has no semantic
    /// effect.  The resulting projected generator is an RREF basis, and
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
        build_punctured(mother, coordinates, provenance)
    }

    /// Punctures `mother` using an already-known map from `mother` to an
    /// ultimate ancestor coordinate space.
    ///
    /// `mother_map.derived_len()` must equal `mother.n()`; the resulting map
    /// is `mother_map` composed with the deletion map.  [`Self::puncture`] is
    /// the convenient form for chaining `Punctured` values.
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
        build_punctured(mother, coordinates, mother_map)
    }

    /// Punctures an existing punctured code and composes coordinate
    /// provenance.
    ///
    /// The positions are in the current (inner) code coordinate space.  The
    /// returned map points directly to the original mother of the chain.
    pub fn puncture(
        self,
        coordinates: impl IntoIterator<Item = usize>,
    ) -> Result<Punctured<Self>, CodeError> {
        let provenance = self.map.clone();
        build_punctured(self, coordinates, provenance)
    }

    /// Punctures the first `count` conventional systematic positions.
    ///
    /// The conventional layout is positions `0..k`; this helper is a
    /// shorthand for [`Self::new`] with the set `0..count`.  It delegates to
    /// the coordinate-set constructor, including its validation.
    pub fn puncture_first(mother: C, count: usize) -> Result<Self, CodeError> {
        let coordinates = conventional_prefix(&mother, count)?;
        Self::new(mother, coordinates)
    }

    /// Punctures the last `count` conventional systematic positions.
    ///
    /// The conventional layout is positions `0..k`; this helper is a
    /// shorthand for [`Self::new`] with the set `k-count..k`.  It delegates
    /// to the coordinate-set constructor, including its validation.
    pub fn puncture_last(mother: C, count: usize) -> Result<Self, CodeError> {
        let coordinates = conventional_suffix(&mother, count)?;
        Self::new(mother, coordinates)
    }

    /// Alias for [`Self::puncture_last`], the usual count-based puncturing
    /// convention.
    pub fn by_count(mother: C, count: usize) -> Result<Self, CodeError> {
        Self::puncture_last(mother, count)
    }

    /// Alias for [`Self::puncture_first`].
    pub fn from_systematic_prefix(mother: C, count: usize) -> Result<Self, CodeError> {
        Self::puncture_first(mother, count)
    }

    /// Alias for [`Self::puncture_last`].
    pub fn from_systematic_suffix(mother: C, count: usize) -> Result<Self, CodeError> {
        Self::puncture_last(mother, count)
    }

    /// Extends this punctured code while retaining its complete coordinate
    /// provenance, including any fresh coordinates from an earlier extension.
    pub fn extend(self) -> Result<Extended<Self>, CodeError> {
        let provenance = self.map.clone();
        Extended::with_coordinate_map(self, provenance)
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
    pub fn punctured_positions(&self) -> &[usize] {
        &self.removed
    }

    /// Returns the derived coordinates that form an information set.
    ///
    /// The positions are in ascending pivot order and are coordinates of the
    /// punctured code, not positions in the mother code.  Restricting the
    /// RREF generator to these columns gives the identity matrix.
    pub fn information_set(&self) -> &[usize] {
        &self.information_set
    }

    /// Alias for [`Self::information_set`].
    pub fn information_positions(&self) -> &[usize] {
        self.information_set()
    }
}

impl<C> BlockCode for Punctured<C>
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

impl<C> BlockEncoder for Punctured<C>
where
    C: BlockCode,
{
    /// Encodes a message with the punctured code's RREF generator.
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
                    .expect("validated punctured message length");
                value += self.generator.get(row, column) * message_symbol;
            }
            codeword.set(column, value)?;
        }
        Ok(())
    }
}

impl<C> GeneratorMatrixAccess for Punctured<C>
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

/// Builds a shortened code, selecting its [`ShortenedDerivation`].
///
/// The selection reads the mother's parameters and systematic reports only,
/// so the mother's generator is materialized for
/// [`ShortenedDerivation::RankDerived`] and never for
/// [`ShortenedDerivation::SystematicRestriction`].
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

    // Every removed coordinate is a message coordinate of a mother that
    // carries message symbol `i` at coordinate `i`, so the constraints read
    // `m[s] = 0` and the derived code is the mother's restricted to the
    // messages that vanish there.
    let removes_message_positions_only = removed.last().is_none_or(|&last| last < mother.k());
    if removes_message_positions_only
        && mother.is_systematic()?
        && mother.has_canonical_message_order()?
    {
        let kept_message_positions = kept
            .iter()
            .copied()
            .take_while(|&position| position < mother.k())
            .collect::<Box<[usize]>>();
        let information_set = (0..kept_message_positions.len()).collect::<Box<[usize]>>();
        return Ok(Shortened {
            mother,
            map,
            local_map,
            removed: removed.into_boxed_slice(),
            information_set,
            data: ShortenedData::SystematicRestriction {
                kept_message_positions,
            },
        });
    }

    let mother_generator = mother.generator_matrix()?;
    let zero = mother.symbol_zero();
    let generator = materialize_generator(&mother_generator, &zero);
    let data = derive_shortened_data(&generator, &removed, &kept, &zero);

    Ok(Shortened {
        mother,
        map,
        local_map,
        removed: removed.into_boxed_slice(),
        information_set: data.information_set,
        data: ShortenedData::RankDerived {
            generator: data.generator,
        },
    })
}

fn build_punctured<C>(
    mother: C,
    coordinates: impl IntoIterator<Item = usize>,
    provenance: CoordinateMap,
) -> Result<Punctured<C>, CodeError>
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
    let data = derive_punctured_data(&generator, &kept, &zero);

    Ok(Punctured {
        mother,
        map,
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

fn derive_punctured_data<F>(
    mother_generator: &FieldMatrix<F>,
    kept: &[usize],
    zero: &F,
) -> DerivedLinearData<F>
where
    F: FiniteField,
{
    let mut candidate = FieldMatrix::new(mother_generator.rows(), kept.len(), zero.clone());
    for row in 0..mother_generator.rows() {
        for (derived_column, &mother_column) in kept.iter().enumerate() {
            candidate.set(
                row,
                derived_column,
                mother_generator.get(row, mother_column),
            );
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
    use crate::test_support::RankDerivedMother;
    use crate::traits::block::{
        BlockCode, BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess,
    };
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

    impl<F: FieldIdentity + 'static> BlockEncoder for DenseTestCode<F> {
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

            for column in 0..self.n() {
                let mut value = self.zero.zero_like();
                for row in 0..self.k() {
                    value += self.generator.get(row, column).clone()
                        * message
                            .as_slice()
                            .get(row)
                            .expect("validated message length")
                            .clone();
                }
                SymbolSequence::set(codeword, column, value)?;
            }
            Ok(())
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

        /// Tests the generator's first `k` columns, the canonical layout
        /// this fixture stores.
        fn is_systematic(&self) -> Result<bool, CodeError> {
            let one = self.zero.one_like();
            for row in 0..self.k() {
                for column in 0..self.k().min(self.n()) {
                    let expected = if row == column {
                        one.clone()
                    } else {
                        self.zero.zero_like()
                    };
                    if self.generator.get(row, column) != expected {
                        return Ok(false);
                    }
                }
            }
            Ok(self.k() <= self.n())
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

    fn direct_punctured_rank<F>(generator: &FieldMatrix<F>, removed: &[usize]) -> usize
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
        let mut projected = FieldMatrix::new(generator.rows(), kept.len(), zero.clone());
        for row in 0..generator.rows() {
            for (column, &mother_column) in kept.iter().enumerate() {
                projected.set(row, column, generator.get(row, mother_column));
            }
        }
        if projected.cols() == 0 {
            0
        } else {
            projected.rank()
        }
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

    /// Returns the message carrying the field identity at `index` alone.
    fn basis_message<C: BlockCode>(code: &C, index: usize) -> C::Symbols {
        let zero = code.symbol_zero();
        let mut message = C::Symbols::zeroed(code.k(), &zero);
        message
            .set(index, zero.one_like())
            .expect("a basis index below the dimension");
        message
    }

    /// Shortens `code` on `coordinates` through both derivations and asserts
    /// that they agree on every observable the transformation contract
    /// exposes.
    ///
    /// [`RankDerivedMother`] reports no systematic layout and delegates
    /// everything else, so the second construction is the rank-derived
    /// derivation of the same code.  Codewords are compared on the message
    /// basis, which spans the message space of a linear code.
    fn assert_derivations_agree<C>(code: C, coordinates: &[usize])
    where
        C: BlockEncoder + GeneratorMatrixAccess + Clone,
    {
        let fast = Shortened::new(code.clone(), coordinates.iter().copied()).unwrap();
        let rank = Shortened::new(RankDerivedMother(code), coordinates.iter().copied()).unwrap();

        assert_eq!(
            fast.derivation(),
            ShortenedDerivation::SystematicRestriction
        );
        assert_eq!(rank.derivation(), ShortenedDerivation::RankDerived);
        assert_eq!(fast.k(), rank.k());
        assert_eq!(fast.n(), rank.n());
        assert_eq!(fast.information_set(), rank.information_set());
        assert_eq!(fast.shortened_positions(), rank.shortened_positions());
        assert_eq!(fast.is_systematic().unwrap(), rank.is_systematic().unwrap());
        assert_eq!(
            fast.generator_matrix().unwrap(),
            rank.generator_matrix().unwrap()
        );
        for position in 0..fast.n() {
            assert_eq!(
                fast.coordinate_map().mother_position_opt(position),
                rank.coordinate_map().mother_position_opt(position),
                "coordinate {position}"
            );
        }
        for index in 0..fast.k() {
            let message = basis_message(&fast, index);
            let fast_word = fast.encode(&message).unwrap();
            assert_eq!(fast_word, rank.encode(&message).unwrap(), "message {index}");
            assert_eq!(
                fast.extend_codeword(&fast_word).unwrap(),
                rank.extend_codeword(&fast_word).unwrap(),
                "lift of message {index}"
            );
        }
    }

    /// Asserts that both derivations produce the same parity-check matrix,
    /// and that it annihilates every generator row.
    ///
    /// The two matrices coincide for a mother whose check matrix is the
    /// canonical `[-Pᵀ | I]`, which every fixture here supplies.  A mother
    /// carrying another basis of the same dual space would give the
    /// systematic restriction an equally valid matrix in that basis, so the
    /// annihilation law rather than the entries is what generalizes.
    fn assert_parity_derivations_agree<C>(code: C, coordinates: &[usize])
    where
        C: BlockEncoder + GeneratorMatrixAccess + ParityCheckMatrixAccess + Clone,
    {
        let fast = Shortened::new(code.clone(), coordinates.iter().copied()).unwrap();
        let rank = Shortened::new(RankDerivedMother(code), coordinates.iter().copied()).unwrap();

        let fast_check = fast.parity_check_matrix().unwrap();
        assert_eq!(fast.parity_check_rows(), rank.parity_check_rows());
        assert_eq!(fast_check, rank.parity_check_matrix().unwrap());

        let generator = fast.generator_matrix().unwrap();
        let zero = fast.symbol_zero();
        for row in 0..fast.k() {
            for check in 0..fast.parity_check_rows() {
                let mut sum = zero.zero_like();
                for column in 0..fast.n() {
                    let generator_symbol = SymbolMatrix::get(&generator, row, column)
                        .expect("a generator entry within the derived shape");
                    let check_symbol = SymbolMatrix::get(&fast_check, check, column)
                        .expect("a check entry within the derived shape");
                    sum += generator_symbol * check_symbol;
                }
                assert_eq!(sum, zero.zero_like(), "row {row} against check {check}");
            }
        }
    }

    /// Returns the mother `[I_2 | P]` over GF(2) with `k = 2` and `n = 4`.
    fn binary_systematic_mother() -> LinearBlockCode {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(0, 3, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        let mut check = BitMatrix::zeros(2, 4);
        check.set(0, 0, true);
        check.set(0, 2, true);
        check.set(1, 0, true);
        check.set(1, 1, true);
        check.set(1, 3, true);
        LinearBlockCode::new_systematic(generator, Some(check))
    }

    /// Returns the mother `[I_2 | P]` over GF(5) with `k = 2` and `n = 5`.
    fn nonbinary_systematic_mother() -> DenseTestCode<Fp<5>> {
        DenseTestCode {
            generator: fp5_matrix(2, 5, &[1, 0, 2, 3, 4, 0, 1, 1, 4, 2]),
            zero: Fp::<5>::new(0),
        }
    }

    #[test]
    fn shortening_message_positions_of_a_systematic_mother_restricts_the_mother() {
        let mother = binary_systematic_mother();
        let shortened = Shortened::new(mother.clone(), [0]).unwrap();

        assert_eq!(
            shortened.derivation(),
            ShortenedDerivation::SystematicRestriction
        );
        assert_eq!(
            (shortened.k(), shortened.n()),
            (mother.k() - 1, mother.n() - 1)
        );
        assert!(shortened.is_systematic().unwrap());
        assert_eq!(shortened.information_set(), &[0]);
        // Row 1 of the mother generator with column 0 deleted.
        let generator = shortened.generator_matrix().unwrap();
        for column in 0..shortened.n() {
            assert_eq!(
                generator.get(0, column),
                mother_generator_bit(&mother, 1, column + 1)
            );
        }
    }

    /// Returns bit `(row, column)` of the mother's generator matrix.
    fn mother_generator_bit(mother: &LinearBlockCode, row: usize, column: usize) -> bool {
        BitMatrix::get(&mother.generator_matrix().unwrap(), row, column)
    }

    #[test]
    fn shortening_a_parity_coordinate_stays_rank_derived() {
        let mother = binary_systematic_mother();
        let shortened = Shortened::new(mother.clone(), [mother.k()]).unwrap();

        assert_eq!(shortened.derivation(), ShortenedDerivation::RankDerived);
        assert_eq!(shortened.n(), mother.n() - 1);
    }

    #[test]
    fn a_non_canonical_message_order_stays_rank_derived() {
        // `LinearBlockCode::hamming` records its message coordinates at the
        // columns of `H` that are not powers of two, so it reports
        // `is_systematic` while coordinate 0 is a parity coordinate. The
        // rank-derived path is then the only correct one, and it is the one
        // the construction takes.
        let mother = LinearBlockCode::hamming(3);
        assert!(mother.is_systematic().unwrap());
        assert!(!mother.has_canonical_message_order().unwrap());

        let shortened = Shortened::new(mother.clone(), [0]).unwrap();
        assert_eq!(shortened.derivation(), ShortenedDerivation::RankDerived);
        assert_eq!(shortened.k(), mother.k() - 1);

        // Every codeword of the result lifts to a mother codeword that is
        // zero at the removed coordinate, which the message-position reading
        // of coordinate 0 would violate.
        for index in 0..shortened.k() {
            let message = basis_message(&shortened, index);
            let lifted = shortened
                .extend_codeword(&shortened.encode(&message).unwrap())
                .unwrap();
            assert!(!lifted.get(0), "message {index}");
        }
    }

    #[test]
    fn both_derivations_agree_on_a_binary_mother() {
        let mother = binary_systematic_mother();
        for coordinates in [vec![], vec![0], vec![1], vec![0, 1]] {
            assert_derivations_agree(mother.clone(), &coordinates);
            assert_parity_derivations_agree(mother.clone(), &coordinates);
        }
    }

    #[test]
    fn both_derivations_agree_on_a_nonbinary_mother() {
        let mother = nonbinary_systematic_mother();
        for coordinates in [vec![], vec![0], vec![1], vec![0, 1]] {
            assert_derivations_agree(mother.clone(), &coordinates);
        }
    }

    #[test]
    fn both_derivations_satisfy_the_shared_code_contracts() {
        use crate::traits::block::conformance;

        let mother = binary_systematic_mother();
        let fast = Shortened::new(mother.clone(), [0]).unwrap();
        let rank = Shortened::new(RankDerivedMother(mother), [0]).unwrap();
        let one = fast.symbol_zero().one_like();
        let message = basis_message(&fast, 0);

        conformance::block_encoder_contract(&fast, &message);
        conformance::block_encoder_contract(&rank, &message);
        conformance::generator_matrix_contract(&fast);
        conformance::generator_matrix_contract(&rank);
        conformance::generator_rows_encode_basis(&fast, &one);
        conformance::generator_rows_encode_basis(&rank, &one);
        conformance::parity_check_matrix_contract(&fast);
        conformance::parity_check_matrix_contract(&rank);
        conformance::generator_parity_orthogonality(&fast);
        conformance::generator_parity_orthogonality(&rank);

        let mother = nonbinary_systematic_mother();
        let fast = Shortened::new(mother.clone(), [0]).unwrap();
        let rank = Shortened::new(RankDerivedMother(mother), [0]).unwrap();
        let one = fast.symbol_zero().one_like();
        let message = basis_message(&fast, 0);

        conformance::block_encoder_contract(&fast, &message);
        conformance::block_encoder_contract(&rank, &message);
        conformance::generator_matrix_contract(&fast);
        conformance::generator_matrix_contract(&rank);
        conformance::generator_rows_encode_basis(&fast, &one);
        conformance::generator_rows_encode_basis(&rank, &one);
    }

    #[test]
    fn shortening_every_message_position_reaches_the_zero_dimensional_code() {
        let mother = binary_systematic_mother();
        let boundary = Shortened::new(mother.clone(), 0..mother.k()).unwrap();

        assert_eq!(
            boundary.derivation(),
            ShortenedDerivation::SystematicRestriction
        );
        assert_eq!((boundary.k(), boundary.n()), (0, mother.n() - mother.k()));
        assert!(boundary.information_set().is_empty());
        assert!(boundary.is_systematic().unwrap());
        assert_eq!(
            boundary.encode(&BitVec::zeros(0)).unwrap(),
            BitVec::zeros(boundary.n())
        );
        assert_derivations_agree(mother.clone(), &(0..mother.k()).collect::<Vec<_>>());
        assert_parity_derivations_agree(mother.clone(), &(0..mother.k()).collect::<Vec<_>>());
    }

    #[test]
    fn an_empty_coordinate_set_keeps_the_mother_parameters() {
        let mother = binary_systematic_mother();
        let same = Shortened::new(mother.clone(), []).unwrap();

        assert_eq!(
            same.derivation(),
            ShortenedDerivation::SystematicRestriction
        );
        assert_eq!((same.k(), same.n()), (mother.k(), mother.n()));
        assert_eq!(
            same.generator_matrix().unwrap(),
            mother.generator_matrix().unwrap()
        );
    }

    #[test]
    fn extension_preserves_parameters_systematic_information_and_map() {
        let mother = LinearBlockCode::hamming(3);
        let extended = Extended::new(mother.clone()).unwrap();

        assert_eq!((extended.n(), extended.k()), (mother.n() + 1, mother.k()));
        assert!(extended.is_systematic().unwrap());
        assert_eq!(extended.extension_position(), mother.n());
        assert_eq!(extended.coordinate_map().mother_len(), mother.n());
        assert_eq!(extended.coordinate_map().derived_len(), extended.n());
        for position in 0..extended.n() {
            assert_eq!(
                extended.coordinate_map().mother_position_opt(position),
                if position < mother.n() {
                    Ok(Some(position))
                } else {
                    Ok(None)
                }
            );
        }
        assert_eq!(
            extended
                .coordinate_map()
                .mother_position(extended.extension_position()),
            Err(CodeError::CoordinateOutOfRange {
                coordinate: mother.n(),
                length: extended.n(),
            })
        );

        let generator = extended.generator_matrix().unwrap();
        for row in 0..extended.k() {
            let mut parity = false;
            for column in 0..extended.n() {
                parity ^= generator.get(row, column);
            }
            assert!(!parity);
        }
    }

    #[test]
    fn binary_extension_has_zero_sum_for_every_message() {
        let mother = LinearBlockCode::hamming(3);
        let extended = Extended::new(mother).unwrap();

        for raw_message in 0..(1usize << extended.k()) {
            let mut message = BitVec::zeros(extended.k());
            for position in 0..extended.k() {
                message.set(position, (raw_message >> position) & 1 == 1);
            }
            let codeword = extended.encode(&message).unwrap();
            assert_eq!(codeword.count_ones() % 2, 0, "message {raw_message}");
        }
    }

    proptest! {
        #[test]
        fn nonbinary_extension_has_zero_symbol_sum(
            values in prop::array::uniform6(0u8..5),
            message_values in prop::array::uniform2(0u8..5),
        ) {
            let mother = DenseTestCode {
                generator: fp5_matrix(2, 3, &values),
                zero: Fp::<5>::new(0),
            };
            let extended = Extended::new(mother).unwrap();
            let message: FieldVec<Fp<5>> = message_values
                .into_iter()
                .map(|value| Fp::<5>::new(value as u64))
                .collect();
            let codeword = extended.encode(&message).unwrap();

            let mut sum = Fp::<5>::new(0);
            for position in 0..codeword.len() {
                sum += *codeword
                    .as_slice()
                    .get(position)
                    .expect("encoded length is validated");
            }
            prop_assert!(sum.is_zero());
        }
    }

    #[test]
    fn extension_matches_legacy_extended_bch_parity_position() {
        use crate::bch::extended::ExtendedBchCode;
        use crate::bch::BchCode;
        use gf2_core::gf2m::Gf2mField;

        let field = Gf2mField::new(4, 0b10011).with_tables();
        let base = BchCode::new(15, 11, 1, field);
        let mother = LinearBlockCode::new_systematic(
            crate::traits::GeneratorMatrixAccess::generator_matrix(&base),
            None,
        );
        let extended = Extended::new(mother).unwrap();
        let legacy = ExtendedBchCode::from_bch(&base);

        for raw_message in [0usize, 1, 0x155, 0x2aa, 0x7ff] {
            let mut message = BitVec::zeros(extended.k());
            for position in 0..extended.k() {
                message.set(position, (raw_message >> position) & 1 == 1);
            }
            let actual = extended.encode(&message).unwrap();
            let expected = crate::traits::BlockEncoder::encode(&legacy, &message);
            assert_eq!(actual, expected, "message {raw_message}");
        }
    }

    #[test]
    fn extended_parity_check_matches_legacy_extended_bch() {
        use crate::bch::extended::ExtendedBchCode;
        let legacy = ExtendedBchCode::ebch_16_11();
        let legacy_generator = crate::traits::GeneratorMatrixAccess::generator_matrix(&legacy);
        let legacy_parity = legacy.parity_check();

        let mut mother_generator = BitMatrix::zeros(legacy.k(), legacy.n() - 1);
        for row in 0..mother_generator.rows() {
            for column in 0..mother_generator.cols() {
                mother_generator.set(row, column, legacy_generator.get(row, column));
            }
        }
        let mut mother_parity =
            BitMatrix::zeros(legacy_parity.rows() - 1, legacy_parity.cols() - 1);
        for row in 0..mother_parity.rows() {
            for column in 0..mother_parity.cols() {
                mother_parity.set(row, column, legacy_parity.get(row, column));
            }
        }
        let mother = LinearBlockCode::new_systematic(mother_generator, Some(mother_parity));
        let extended = Extended::new(mother).expect("an extended BCH code fits in memory");

        assert_eq!(
            extended.parity_check_matrix().unwrap(),
            *legacy.parity_check()
        );
    }

    #[test]
    fn nonbinary_extension_parity_check_annihilates_every_codeword() {
        let zero = Fp::<5>::new(0);
        let mother = crate::traits::block::conformance::RepetitionCode::new(3, zero);
        let extended = Extended::new(mother).unwrap();
        let parity_check = extended.parity_check_matrix().unwrap();

        assert_eq!(parity_check.rows(), 3);
        assert_eq!(parity_check.cols(), 4);
        for value in 0..5 {
            let message = FieldVec::from(vec![Fp::<5>::new(value)]);
            let codeword = extended.encode(&message).unwrap();
            for row in 0..parity_check.rows() {
                let mut syndrome = zero.zero_like();
                for column in 0..parity_check.cols() {
                    let coefficient = parity_check.get(row, column);
                    let symbol = *codeword
                        .as_slice()
                        .get(column)
                        .expect("encoded codeword has the declared length");
                    syndrome += coefficient * symbol;
                }
                assert!(
                    syndrome.is_zero(),
                    "nonzero syndrome at row {row}, value {value}"
                );
            }
        }
    }

    #[test]
    fn extension_map_composes_when_shortened_provenance_is_supplied() {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let shortened = Shortened::new(mother, [1]).unwrap();
        let provenance = shortened.coordinate_map().clone();
        let extended = shortened.extend().unwrap();

        assert_eq!(
            extended.coordinate_map().mother_len(),
            provenance.mother_len()
        );
        assert_eq!(
            extended.coordinate_map().derived_len(),
            provenance.derived_len() + 1
        );
        for position in 0..provenance.derived_len() {
            assert_eq!(
                extended.coordinate_map().mother_position(position),
                provenance.mother_position(position)
            );
        }
        assert_eq!(
            extended
                .coordinate_map()
                .mother_position_opt(provenance.derived_len()),
            Ok(None)
        );

        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let punctured = Punctured::new(mother, [1]).unwrap();
        let provenance = punctured.coordinate_map().clone();
        let extended = punctured.extend().unwrap();
        assert_eq!(
            extended.coordinate_map().mother_len(),
            provenance.mother_len()
        );
        assert_eq!(
            extended.coordinate_map().derived_len(),
            provenance.derived_len() + 1
        );
        assert_eq!(
            extended
                .coordinate_map()
                .mother_position_opt(provenance.derived_len()),
            Ok(None)
        );
    }

    #[test]
    fn extend_then_shorten_keeps_the_mother_coordinate_order() {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let extended = Extended::new(mother.clone()).unwrap();
        let shortened = extended.shorten([mother.n()]).unwrap();

        assert_eq!(shortened.n(), mother.n());
        assert_eq!(shortened.coordinate_map().mother_len(), mother.n());
        for position in 0..mother.n() {
            assert_eq!(
                shortened.coordinate_map().mother_position(position),
                Ok(position)
            );
        }
    }

    #[test]
    fn extend_then_puncture_retains_parity_coordinate_and_complete_map() {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let punctured = Extended::new(mother).unwrap().puncture([0]).unwrap();

        assert_eq!(punctured.coordinate_map().mother_len(), 4);
        assert_eq!(punctured.coordinate_map().derived_len(), punctured.n());
        assert_eq!(
            punctured.coordinate_map().mother_position_opt(0),
            Ok(Some(1))
        );
        assert_eq!(
            punctured.coordinate_map().mother_position_opt(1),
            Ok(Some(2))
        );
        assert_eq!(
            punctured.coordinate_map().mother_position_opt(2),
            Ok(Some(3))
        );
        assert_eq!(punctured.coordinate_map().mother_position_opt(3), Ok(None));
    }

    #[test]
    fn extend_then_shorten_retains_parity_coordinate_and_complete_map() {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let shortened = Extended::new(mother).unwrap().shorten([0]).unwrap();

        assert_eq!(shortened.coordinate_map().mother_len(), 4);
        assert_eq!(shortened.coordinate_map().derived_len(), shortened.n());
        assert_eq!(
            shortened.coordinate_map().mother_position_opt(0),
            Ok(Some(1))
        );
        assert_eq!(
            shortened.coordinate_map().mother_position_opt(1),
            Ok(Some(2))
        );
        assert_eq!(
            shortened.coordinate_map().mother_position_opt(2),
            Ok(Some(3))
        );
        assert_eq!(shortened.coordinate_map().mother_position_opt(3), Ok(None));
    }

    #[test]
    fn extend_then_puncture_on_parity_has_the_mother_equivalent_map() {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 3, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let punctured = Extended::new(mother.clone())
            .unwrap()
            .puncture([mother.n()])
            .unwrap();

        assert_eq!(punctured.coordinate_map().mother_len(), mother.n());
        assert_eq!(punctured.coordinate_map().derived_len(), mother.n());
        for position in 0..mother.n() {
            assert_eq!(
                punctured.coordinate_map().mother_position_opt(position),
                Ok(Some(position))
            );
        }
    }

    #[test]
    fn extending_an_extended_code_has_a_complete_map() {
        let mother = LinearBlockCode::hamming(3);
        let first = Extended::new(mother.clone()).unwrap();
        let second = Extended::new(first).unwrap();

        assert_eq!(second.n(), mother.n() + 2);
        assert_eq!(second.coordinate_map().mother_len(), mother.n() + 1);
        assert_eq!(second.coordinate_map().derived_len(), second.n());
        for position in 0..second.n() {
            assert_eq!(
                second.coordinate_map().mother_position_opt(position),
                if position < second.n() - 1 {
                    Ok(Some(position))
                } else {
                    Ok(None)
                }
            );
        }
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

    #[test]
    fn puncturing_can_drop_dimension_when_messages_collide() {
        let mut generator = BitMatrix::zeros(2, 3);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 2, true);
        let mother = LinearBlockCode::new_systematic(generator.clone(), None);
        let punctured = Punctured::new(mother, [0, 1]).unwrap();

        assert_eq!(punctured.n(), 1);
        assert_eq!(punctured.k(), 1);
        assert_eq!(punctured.information_set(), &[0]);
        assert_eq!(punctured.coordinate_map().mother_position(0).unwrap(), 2);

        let mut dense = FieldMatrix::zeros(2, 3);
        for row in 0..2 {
            for column in 0..3 {
                dense.set(row, column, Fp::<2>::new(generator.get(row, column) as u64));
            }
        }
        assert_eq!(punctured.k(), direct_punctured_rank(&dense, &[0, 1]));
    }

    #[test]
    fn chained_puncturing_composes_maps_and_matches_combined_set() {
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
        let first = Punctured::new(mother.clone(), [1]).unwrap();
        let chained = first.puncture([2]).unwrap();
        let combined = Punctured::new(mother, [1, 3]).unwrap();

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
    fn puncturing_count_helpers_delegate_to_coordinate_sets() {
        let mut generator = BitMatrix::zeros(3, 5);
        for diagonal in 0..3 {
            generator.set(diagonal, diagonal, true);
        }
        let first =
            Punctured::puncture_first(LinearBlockCode::new_systematic(generator.clone(), None), 2)
                .unwrap();
        let explicit_first = Punctured::new(
            LinearBlockCode::new_systematic(generator.clone(), None),
            [0, 1],
        )
        .unwrap();
        let last =
            Punctured::puncture_last(LinearBlockCode::new_systematic(generator.clone(), None), 2)
                .unwrap();
        let explicit_last =
            Punctured::new(LinearBlockCode::new_systematic(generator, None), [1, 2]).unwrap();

        assert_eq!(
            first.generator_matrix().unwrap(),
            explicit_first.generator_matrix().unwrap()
        );
        assert_eq!(
            last.generator_matrix().unwrap(),
            explicit_last.generator_matrix().unwrap()
        );
        assert!(Punctured::by_count(
            LinearBlockCode::new_systematic(BitMatrix::zeros(3, 5), None),
            0
        )
        .unwrap()
        .punctured_positions()
        .is_empty());
    }

    #[test]
    fn puncturing_preserves_kept_coordinate_order() {
        let mut generator = BitMatrix::zeros(2, 5);
        generator.set(0, 0, true);
        generator.set(1, 1, true);
        let code = LinearBlockCode::new_systematic(generator, None);
        let punctured = Punctured::new(code, [3, 1]).unwrap();

        assert_eq!(punctured.n(), 3);
        assert_eq!(punctured.punctured_positions(), &[1, 3]);
        assert_eq!(
            (0..punctured.n())
                .map(|position| punctured
                    .coordinate_map()
                    .mother_position(position)
                    .unwrap())
                .collect::<Vec<_>>(),
            vec![0, 2, 4]
        );
        assert!((0..punctured.n()).all(|position| {
            !punctured.punctured_positions().contains(
                &punctured
                    .coordinate_map()
                    .mother_position(position)
                    .unwrap(),
            )
        }));
    }

    #[test]
    fn puncturing_and_shortening_have_expected_subspace_dimensions() {
        let mut generator = BitMatrix::zeros(2, 4);
        generator.set(0, 0, true);
        generator.set(0, 2, true);
        generator.set(1, 1, true);
        generator.set(1, 2, true);
        let mother = LinearBlockCode::new_systematic(generator, None);
        let shortened = Shortened::new(mother.clone(), [0]).unwrap();
        let punctured = Punctured::new(mother, [0]).unwrap();

        assert!(shortened.k() <= punctured.k());
    }

    #[test]
    fn invalid_puncturing_coordinate_sets_use_code_errors() {
        let mut generator = BitMatrix::zeros(1, 4);
        generator.set(0, 0, true);
        let code = LinearBlockCode::new_systematic(generator, None);

        assert!(matches!(
            Punctured::new(code.clone(), [4]),
            Err(CodeError::CoordinateOutOfRange { .. })
        ));
        assert!(matches!(
            Punctured::new(code.clone(), [1, 1]),
            Err(CodeError::DuplicateCoordinate { .. })
        ));
        let boundary = Punctured::new(code, [0, 1, 2, 3]).unwrap();
        assert_eq!(boundary.n(), 0);
        assert_eq!(boundary.k(), 0);
        assert!(boundary.information_set().is_empty());
    }

    #[test]
    fn full_coordinate_puncturing_of_a_rank_deficient_nonbinary_code_is_boundary() {
        let values = [1u8, 2, 0, 1, 2, 4, 0, 2, 3, 1, 0, 3];
        let generator = fp5_matrix(3, 4, &values);
        let code = DenseTestCode {
            generator,
            zero: Fp::<5>::new(0),
        };
        let boundary = Punctured::new(code, [0, 1, 2, 3]).unwrap();

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

        #[test]
        fn binary_puncturing_dimension_matches_direct_projected_rank(
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
                .collect::<Vec<_>>();
            let code = LinearBlockCode::new_systematic(generator.clone(), None);
            let punctured = Punctured::new(code, removed.clone()).unwrap();
            let mut dense = FieldMatrix::zeros(rows, cols);
            for row in 0..rows {
                for column in 0..cols {
                    dense.set(row, column, Fp::<2>::new(generator.get(row, column) as u64));
                }
            }
            prop_assert_eq!(punctured.k(), direct_punctured_rank(&dense, &removed));
        }

        #[test]
        fn nonbinary_puncturing_dimension_matches_direct_projected_rank(
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
                .collect::<Vec<_>>();
            let code = DenseTestCode { generator: generator.clone(), zero: Fp::<5>::new(0) };
            let punctured = Punctured::new(code, removed.clone()).unwrap();
            prop_assert_eq!(punctured.k(), direct_punctured_rank(&generator, &removed));
        }

        #[test]
        fn binary_derivations_agree_on_random_systematic_mothers(
            rows in 1usize..=4,
            redundancy in 1usize..=4,
            parity in prop::collection::vec(any::<bool>(), 16),
            mask in any::<u8>(),
        ) {
            let cols = rows + redundancy;
            let mut generator = BitMatrix::zeros(rows, cols);
            for row in 0..rows {
                generator.set(row, row, true);
                for column in rows..cols {
                    generator.set(row, column, parity[(row * redundancy + column - rows) % parity.len()]);
                }
            }
            let code = LinearBlockCode::new_systematic(generator, None);
            let removed = (0..rows)
                .filter(|position| (mask >> (position % 8)) & 1 == 1)
                .collect::<Vec<_>>();
            assert_derivations_agree(code, &removed);
        }

        #[test]
        fn nonbinary_derivations_agree_on_random_systematic_mothers(
            rows in 1usize..=4,
            redundancy in 1usize..=4,
            parity in prop::collection::vec(0u8..5, 16),
            mask in any::<u8>(),
        ) {
            let cols = rows + redundancy;
            let mut values = vec![0u8; rows * cols];
            for row in 0..rows {
                values[row * cols + row] = 1;
                for column in rows..cols {
                    values[row * cols + column] =
                        parity[(row * redundancy + column - rows) % parity.len()];
                }
            }
            let code = DenseTestCode {
                generator: fp5_matrix(rows, cols, &values),
                zero: Fp::<5>::new(0),
            };
            let removed = (0..rows)
                .filter(|position| (mask >> (position % 8)) & 1 == 1)
                .collect::<Vec<_>>();
            assert_derivations_agree(code, &removed);
        }
    }
}
