//! Provenance maps for coordinates of derived codes.
//!
//! [`CoordinateMap`] always points from a derived code to its mother code: an
//! input position is a derived-code position and the returned value is the
//! corresponding mother-code position.  Composition follows the usual
//! function convention.  For `outer.compose(&inner)`, `inner` is applied
//! first and `outer` second, so the result is `outer \u{2218} inner`.
//!
//! Maps whose coordinates are an increasing contiguous range are stored as a
//! start and length, including identity maps (which use start `0`).  Every
//! other valid map is stored as an explicit injective coordinate vector.  The
//! vector constructor applies the same rule automatically, and composition
//! canonicalizes its result, so a regular result remains a compact range.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::transform::CoordinateMap;
//!
//! // Four derived positions address mother positions 2, 3, 4, and 5.
//! let selected = CoordinateMap::range(8, 2, 4).unwrap();
//! // This map reorders those four positions before the selection is applied.
//! let reversed = CoordinateMap::from_permutation(4, [3, 2, 1, 0]).unwrap();
//! let composed = selected.compose(&reversed).unwrap();
//!
//! assert_eq!(composed.derived_len(), 4);
//! assert_eq!(composed.mother_position(0).unwrap(), 5);
//! assert_eq!(composed.mother_position(3).unwrap(), 2);
//! ```

use crate::error::CodeError;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Representation {
    Range { start: usize, length: usize },
    Permutation(Vec<usize>),
}

/// A checked mapping from derived-code coordinates to mother-code coordinates.
///
/// A map is injective: a mother coordinate cannot be the provenance of two
/// derived positions.  This is the coordinate contract needed by shortening,
/// puncturing, and coordinate reordering.  The map stores both the mother
/// length and the derived length, so trailing mother coordinates that are not
/// present in a derived code remain distinguishable from a shorter mother
/// code.
#[derive(Clone, Debug)]
pub struct CoordinateMap {
    mother_length: usize,
    representation: Representation,
}

impl CoordinateMap {
    /// Creates the identity map for a code of `length` coordinates.
    ///
    /// The resulting map has both mother and derived length equal to
    /// `length`, and is stored as a compact range.  This constructor cannot
    /// fail and does not panic.
    pub fn identity(length: usize) -> Self {
        Self::new_range(length, 0, length)
    }

    /// Creates a compact map for `start..start + derived_length` in a mother
    /// code of `mother_length` coordinates.
    ///
    /// The range is inclusive at `start` and exclusive at its end.  An empty
    /// range is valid when `start <= mother_length`.  Returns
    /// [`CodeError::CoordinateOutOfRange`] when the range does not fit or its
    /// start is outside the mother code.  User input is reported as an error;
    /// this method does not panic.
    pub fn range(
        mother_length: usize,
        start: usize,
        derived_length: usize,
    ) -> Result<Self, CodeError> {
        if start > mother_length {
            return Err(CodeError::CoordinateOutOfRange {
                coordinate: start,
                length: mother_length,
            });
        }

        let Some(end) = start.checked_add(derived_length) else {
            return Err(CodeError::CoordinateOutOfRange {
                coordinate: mother_length,
                length: mother_length,
            });
        };
        if end > mother_length {
            return Err(CodeError::CoordinateOutOfRange {
                coordinate: mother_length,
                length: mother_length,
            });
        }

        Ok(Self::new_range(mother_length, start, derived_length))
    }

    /// Creates a map from an explicit ordering of mother coordinates.
    ///
    /// `coordinates` contains one mother position for each derived position.
    /// Every position must be below `mother_length`, and positions must be
    /// distinct.  A vector that is an increasing contiguous range is stored
    /// as the compact range representation automatically; all other vectors
    /// are retained as an explicit permutation.  Returns a typed
    /// [`CodeError`] for invalid input and does not panic on caller input.
    pub fn from_permutation(
        mother_length: usize,
        coordinates: impl AsRef<[usize]>,
    ) -> Result<Self, CodeError> {
        let coordinates = coordinates.as_ref();
        let mut seen = HashSet::with_capacity(coordinates.len());
        for &coordinate in coordinates {
            if coordinate >= mother_length {
                return Err(CodeError::CoordinateOutOfRange {
                    coordinate,
                    length: mother_length,
                });
            }
            if !seen.insert(coordinate) {
                return Err(CodeError::DuplicateCoordinate { coordinate });
            }
        }

        if let Some(start) = contiguous_start(coordinates) {
            Ok(Self::new_range(mother_length, start, coordinates.len()))
        } else {
            Ok(Self {
                mother_length,
                representation: Representation::Permutation(coordinates.to_vec()),
            })
        }
    }

    /// Returns the number of coordinates in the derived code.
    ///
    /// This is an O(1) operation for both compact and explicit maps and does
    /// not panic.
    pub fn derived_len(&self) -> usize {
        match &self.representation {
            Representation::Range { length, .. } => *length,
            Representation::Permutation(coordinates) => coordinates.len(),
        }
    }

    /// Returns the number of coordinates in the mother code.
    ///
    /// This is an O(1) operation and does not panic.
    pub fn mother_len(&self) -> usize {
        self.mother_length
    }

    /// Looks up the mother-code coordinate for a derived-code position.
    ///
    /// Returns [`CodeError::CoordinateOutOfRange`] when `derived_position` is
    /// not in `0..self.derived_len()`.  Valid lookups are O(1) for both
    /// representations.  This method does not panic on caller input.
    pub fn mother_position(&self, derived_position: usize) -> Result<usize, CodeError> {
        if derived_position >= self.derived_len() {
            return Err(CodeError::CoordinateOutOfRange {
                coordinate: derived_position,
                length: self.derived_len(),
            });
        }

        Ok(self.mother_position_unchecked(derived_position))
    }

    /// Composes this map after `inner`, returning `self \u{2218} inner`.
    ///
    /// `self.derived_len()` must equal `inner.mother_len()`: the coordinates
    /// produced by `inner` must be valid inputs to this map.  A mismatch is
    /// reported as [`CodeError::CoordinateCountMismatch`].  Two range maps
    /// compose directly as a range; every other result is canonicalized so an
    /// increasing contiguous result is still stored without a vector.  The
    /// operation does not panic on caller input.
    pub fn compose(&self, inner: &Self) -> Result<Self, CodeError> {
        if self.derived_len() != inner.mother_len() {
            return Err(CodeError::CoordinateCountMismatch {
                expected: self.derived_len(),
                actual: inner.mother_len(),
            });
        }

        if let (
            Representation::Range {
                start: outer_start, ..
            },
            Representation::Range {
                start: inner_start,
                length: inner_length,
            },
        ) = (&self.representation, &inner.representation)
        {
            let Some(start) = outer_start.checked_add(*inner_start) else {
                return Err(CodeError::CoordinateOutOfRange {
                    coordinate: self.mother_length,
                    length: self.mother_length,
                });
            };
            return Self::range(self.mother_length, start, *inner_length);
        }

        let coordinates = (0..inner.derived_len())
            .map(|position| {
                let intermediate = inner.mother_position_unchecked(position);
                self.mother_position_unchecked(intermediate)
            })
            .collect::<Vec<_>>();

        // Both operands are injective and their lengths are compatible, so
        // this cannot fail unless an internal representation invariant has
        // been violated.  Keeping the checked constructor here makes that
        // invariant explicit and avoids an unchecked public construction path.
        Self::from_permutation(self.mother_length, coordinates)
    }

    fn new_range(mother_length: usize, start: usize, length: usize) -> Self {
        Self {
            mother_length,
            representation: Representation::Range { start, length },
        }
    }

    fn mother_position_unchecked(&self, derived_position: usize) -> usize {
        match &self.representation {
            Representation::Range { start, length } => {
                debug_assert!(derived_position < *length);
                debug_assert!(start.checked_add(derived_position).is_some());
                start + derived_position
            }
            Representation::Permutation(coordinates) => {
                debug_assert!(derived_position < coordinates.len());
                coordinates[derived_position]
            }
        }
    }
}

impl PartialEq for CoordinateMap {
    fn eq(&self, other: &Self) -> bool {
        self.mother_length == other.mother_length
            && self.derived_len() == other.derived_len()
            && (0..self.derived_len()).all(|position| {
                self.mother_position_unchecked(position)
                    == other.mother_position_unchecked(position)
            })
    }
}

impl Eq for CoordinateMap {}

fn contiguous_start(coordinates: &[usize]) -> Option<usize> {
    let Some(&start) = coordinates.first() else {
        return Some(0);
    };
    if coordinates
        .iter()
        .enumerate()
        .all(|(offset, &coordinate)| coordinate.checked_sub(start) == Some(offset))
    {
        Some(start)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn chain_dimensions() -> impl Strategy<Value = (usize, usize, usize, usize)> {
        (0usize..=8)
            .prop_flat_map(|mother_length| (Just(mother_length), 0usize..=mother_length))
            .prop_flat_map(|(mother_length, middle_length)| {
                (
                    Just(mother_length),
                    Just(middle_length),
                    0usize..=middle_length,
                )
            })
            .prop_flat_map(|(mother_length, middle_length, inner_length)| {
                (
                    Just(mother_length),
                    Just(middle_length),
                    Just(inner_length),
                    0usize..=inner_length,
                )
            })
    }

    fn mixed_map(derived_length: usize, mother_length: usize, regular: bool) -> CoordinateMap {
        if regular {
            CoordinateMap::range(
                mother_length,
                mother_length - derived_length,
                derived_length,
            )
            .unwrap()
        } else {
            let coordinates = (0..mother_length)
                .rev()
                .take(derived_length)
                .collect::<Vec<_>>();
            CoordinateMap::from_permutation(mother_length, coordinates).unwrap()
        }
    }

    #[test]
    fn identity_and_contiguous_vectors_use_the_range_representation() {
        let identity = CoordinateMap::identity(7);
        assert!(matches!(
            identity.representation,
            Representation::Range { .. }
        ));

        let from_vector = CoordinateMap::from_permutation(10, [3, 4, 5, 6]).unwrap();
        assert!(matches!(
            from_vector.representation,
            Representation::Range {
                start: 3,
                length: 4
            }
        ));

        let non_regular = CoordinateMap::from_permutation(10, [6, 4, 5]).unwrap();
        assert!(matches!(
            non_regular.representation,
            Representation::Permutation(_)
        ));

        let empty = CoordinateMap::from_permutation(10, []).unwrap();
        assert!(matches!(
            empty.representation,
            Representation::Range {
                start: 0,
                length: 0
            }
        ));
    }

    #[test]
    fn regular_map_matches_its_explicit_vector_expansion() {
        let regular = CoordinateMap::range(12, 4, 5).unwrap();
        let expanded = (0..regular.derived_len())
            .map(|position| regular.mother_position(position).unwrap())
            .collect::<Vec<_>>();
        let explicit = CoordinateMap::from_permutation(12, expanded).unwrap();

        for position in 0..regular.derived_len() {
            assert_eq!(
                regular.mother_position(position),
                explicit.mother_position(position)
            );
        }
    }

    proptest! {
        #[test]
        fn composition_is_associative_over_mixed_representations(
            (mother_length, middle_length, inner_length, derived_length) in chain_dimensions(),
            regular in prop::array::uniform3(any::<bool>()),
        ) {
            let outer = mixed_map(middle_length, mother_length, regular[0]);
            let middle = mixed_map(inner_length, middle_length, regular[1]);
            let inner = mixed_map(derived_length, inner_length, regular[2]);

            let left = outer.compose(&middle).unwrap().compose(&inner).unwrap();
            let right = outer.compose(&middle.compose(&inner).unwrap()).unwrap();

            prop_assert_eq!(&left, &right);
            for position in 0..derived_length {
                prop_assert_eq!(
                    left.mother_position(position).unwrap(),
                    right.mother_position(position).unwrap()
                );
            }
        }

        #[test]
        fn composed_lookup_matches_stepwise_round_trip(
            (mother_length, middle_length, _inner_length, derived_length) in chain_dimensions(),
            regular_outer in any::<bool>(),
            regular_inner in any::<bool>(),
        ) {
            let outer = mixed_map(middle_length, mother_length, regular_outer);
            let inner = mixed_map(derived_length, middle_length, regular_inner);
            let composed = outer.compose(&inner).unwrap();

            for position in 0..derived_length {
                let stepwise = outer
                    .mother_position(inner.mother_position(position).unwrap())
                    .unwrap();
                prop_assert_eq!(composed.mother_position(position).unwrap(), stepwise);
            }
        }
    }

    #[test]
    fn invalid_lookup_and_composition_report_code_errors() {
        let map = CoordinateMap::identity(3);
        assert_eq!(
            map.mother_position(3),
            Err(CodeError::CoordinateOutOfRange {
                coordinate: 3,
                length: 3,
            })
        );

        let outer = CoordinateMap::identity(4);
        let incompatible = CoordinateMap::identity(3);
        assert_eq!(
            outer.compose(&incompatible),
            Err(CodeError::CoordinateCountMismatch {
                expected: 4,
                actual: 3,
            })
        );
    }

    #[test]
    fn constructors_reject_invalid_coordinate_ranges() {
        assert_eq!(
            CoordinateMap::range(4, 3, 2),
            Err(CodeError::CoordinateOutOfRange {
                coordinate: 4,
                length: 4,
            })
        );
        assert_eq!(
            CoordinateMap::from_permutation(4, [0, 0]),
            Err(CodeError::DuplicateCoordinate { coordinate: 0 })
        );
        assert_eq!(
            CoordinateMap::from_permutation(4, [4]),
            Err(CodeError::CoordinateOutOfRange {
                coordinate: 4,
                length: 4,
            })
        );
    }
}
