//! Lane-parallel packed arithmetic over small prime fields: the
//! [`PackedField`] and [`PackedFieldVec`] traits, the `Bipedal3` (F_3),
//! `packed5::Packed5` (F_5) and `packed7::Packed7` (F_7) encodings, and the
//! [`scalar::ScalarPackedFp3`] reference oracle.

use gf2_core::field::FiniteField;

pub mod bipedal3;
pub mod scalar;

#[cfg(feature = "f5")]
pub mod packed5;

#[cfg(feature = "f7")]
pub mod packed7;

pub use bipedal3::{Bipedal3, Bipedal3Matrix, Bipedal3Vec};
pub use scalar::{ScalarPackedFp3, ScalarPackedFp3Vec};

#[cfg(feature = "f5")]
pub use packed5::{Packed5, Packed5Matrix, Packed5Vec};

#[cfg(feature = "f7")]
pub use packed7::{Packed7, Packed7Matrix, Packed7Vec};

/// Fixed-LANES lane-parallel arithmetic over an underlying scalar field `F`.
///
/// A value carries [`Self::LANES`] independent `F`-elements. `Eq` is
/// canonical-decode equality: two values are equal iff every decoded lane
/// is equal in `F`, regardless of redundancy in the encoding.
///
/// # Complexity
///
/// `lane` and `with_lane` are `O(1)`. Every other method is `O(1)` for
/// fixed-width encodings (e.g. bipedal3) and `O(LANES)` for scalar-array
/// encodings (e.g. [`ScalarPackedFp3`]).
pub trait PackedField<F: FiniteField>: Copy + Eq + core::fmt::Debug {
    /// Number of independent `F`-lanes packed into one `Self`.
    ///
    /// Must be positive.
    const LANES: usize;

    /// All-lanes-zero constant.
    fn zero() -> Self;

    /// All-lanes-one constant.
    fn one() -> Self;

    /// Broadcast scalar `x` to every lane.
    fn splat(x: F) -> Self;

    /// Lane-wise sum.
    fn add(self, rhs: Self) -> Self;

    /// Lane-wise difference.
    fn sub(self, rhs: Self) -> Self;

    /// Lane-wise additive inverse.
    fn neg(self) -> Self;

    /// Lane-wise product.
    fn mul(self, rhs: Self) -> Self;

    /// Decode lane `i` to a canonical `F` value.
    ///
    /// # Panics
    ///
    /// Panics if `i >= Self::LANES`.
    fn lane(self, i: usize) -> F;

    /// Encode `x` into lane `i`, returning the updated value.
    ///
    /// # Panics
    ///
    /// Panics if `i >= Self::LANES`.
    fn with_lane(self, i: usize, x: F) -> Self;

    /// Returns `true` iff every lane decodes to `F`'s additive identity.
    ///
    /// A redundant non-canonical zero codeword (e.g. bipedal `(0, 1)`)
    /// still answers `true`.
    fn all_zero(self) -> bool;
}

/// Variable-length lane-parallel container of `F`-elements.
///
/// Each logical position `0..len()` holds one `F`. `Eq` is canonical-decode
/// equality.
pub trait PackedFieldVec<F: FiniteField>: Clone + Eq + core::fmt::Debug {
    /// Fixed-LANES packed companion type.
    type Element: PackedField<F>;

    /// Construct a vector of `len` zeros.
    fn zeros(len: usize) -> Self;

    /// Construct a vector with `get(i) == xs[i]` for every `i`.
    fn from_field_slice(xs: &[F]) -> Self;

    /// Number of logical `F`-positions held by this vector.
    fn len(&self) -> usize;

    /// Returns `true` iff `self.len() == 0`.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Decode logical position `i` to a canonical `F` value.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    fn get(&self, i: usize) -> F;

    /// Lane-wise in-place sum: `self[i] += rhs[i]` for every `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    fn add_assign(&mut self, rhs: &Self);

    /// Lane-wise in-place difference: `self[i] -= rhs[i]` for every `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    fn sub_assign(&mut self, rhs: &Self);

    /// Lane-wise in-place product: `self[i] *= rhs[i]` for every `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    fn mul_assign(&mut self, rhs: &Self);

    /// Returns `true` iff every logical position decodes to `F`'s
    /// additive identity.
    ///
    /// A redundant non-canonical zero codeword (e.g. bipedal `(0, 1)`)
    /// still answers `true`, as does the empty vector.
    fn all_zero(&self) -> bool;
}
