//! Packed finite-field abstractions.
//!
//! Hosts the [`PackedField`] trait that abstracts lane-parallel
//! arithmetic over a small prime field, plus the concrete
//! `Bipedal3` (F_3), `packed5::Packed5` (F_5, R1 Candidate D),
//! `packed7::Packed7` (F_7, R2 Candidate A) element / vector types,
//! and the [`scalar::ScalarPackedFp3`] correctness oracle.
//!
//! The trait surface is fixed by `dev/plans/9fe275d3/d1b_packed_field_api.md`
//! (user-approved 2026-05-09; see JIT issue `9fe275d3`'s description
//! `## Approval` section) and frozen at the W6 `gate:api-freeze`.
//!
//! # Cross-checking strategy
//!
//! Each concrete `PackedField` impl validates against the scalar
//! `Fp<P>` reference (and, for F_3, against [`scalar::ScalarPackedFp3`])
//! by routing the same random inputs through both implementations and
//! asserting per-lane decoded equality. The reference impls have no
//! SIMD or bit-packing optimisations — they exist purely as a
//! correctness oracle.

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
/// A `PackedField<F>` value carries [`Self::LANES`] independent
/// `F`-elements in one Rust value. Lane operations are constant-time
/// across all lanes (no per-lane branching); a `LANES = 64` instance
/// maps onto a pair of `u64`s in the bipedal3 encoding, a
/// `LANES = 256` instance maps onto an AVX2 `__m256i` pair, and so on.
///
/// The trait surface is fixed by
/// `dev/plans/9fe275d3/d1b_packed_field_api.md` §2.1, user-approved 2026-05-09
/// (JIT issue `9fe275d3`'s description `## Approval` section). The
/// signatures are frozen at the W6 `gate:api-freeze` of the
/// `gf2-algebra-permanent` epic; in-loop amendment is permitted only
/// before that gate fires.
///
/// # Trait bounds
///
/// `Copy + Eq + Debug` — all decided values, no allocation, no
/// fallible construction. `Eq` is the **canonical-decode** equality:
/// two packed values are equal iff every decoded lane is equal in `F`,
/// regardless of any internal redundancy in the encoding (D1b §3.4).
///
/// # Lane semantics
///
/// `lane(i)` returns the canonical `F` value of lane `i`, decoding any
/// implementation-internal redundancy (e.g. the bipedal `(0, 1)`
/// alternative-zero codeword decodes to `Fp::<3>::new(0)`).
/// `with_lane(i, x)` writes the canonical encoding of `x` into lane
/// `i` (D1b §3.5).
///
/// # No `unsafe`
///
/// This trait is `#![deny(unsafe_code)]`-compatible; SIMD intrinsics
/// live behind safe function-pointer bundles in `gf2-kernels-simd`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
/// use gf2_core::gfp::Fp;
///
/// let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
/// let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
/// let s = a.add(b);
/// assert_eq!(s.lane(0), Fp::<3>::new(0)); // 1 + 2 == 0 mod 3
/// ```
pub trait PackedField<F: FiniteField>: Copy + Eq + core::fmt::Debug {
    /// Number of independent `F`-lanes packed into one `Self`.
    ///
    /// Must be positive. A power-of-two is preferred for SIMD-friendly
    /// mapping where feasible, but non-power-of-two values are
    /// permitted (e.g. `Packed7` packs 16 lanes per `u64`, and other
    /// future per-prime encodings may pack non-power-of-two lane
    /// counts).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// assert_eq!(<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES, 64);
    /// ```
    const LANES: usize;

    /// All-lanes-zero constant.
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings (e.g. bipedal3); `O(LANES)`
    /// for scalar-array encodings (e.g. [`ScalarPackedFp3`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
    /// assert!(z.all_zero());
    /// ```
    fn zero() -> Self;

    /// All-lanes-one constant.
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings; `O(LANES)` for scalar-array
    /// encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let o = <ScalarPackedFp3 as PackedField<Fp<3>>>::one();
    /// assert_eq!(o.lane(0), Fp::<3>::new(1));
    /// assert_eq!(o.lane(63), Fp::<3>::new(1));
    /// ```
    fn one() -> Self;

    /// Broadcast scalar `x` to every lane.
    ///
    /// # Arguments
    ///
    /// * `x` — scalar to be replicated across all `LANES` lanes.
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings; `O(LANES)` for scalar-array
    /// encodings such as [`ScalarPackedFp3`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let v = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// for i in 0..<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES {
    ///     assert_eq!(v.lane(i), Fp::<3>::new(2));
    /// }
    /// ```
    fn splat(x: F) -> Self;

    /// Lane-wise sum.
    ///
    /// # Arguments
    ///
    /// * `rhs` — the other operand; lanes are added pointwise.
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings (a fixed number of word-level
    /// bitwise ops, independent of `LANES`); `O(LANES)` for
    /// scalar-array encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// assert_eq!(a.add(b).lane(0), Fp::<3>::new(1)); // 2 + 2 == 1 mod 3
    /// ```
    fn add(self, rhs: Self) -> Self;

    /// Lane-wise difference.
    ///
    /// # Arguments
    ///
    /// * `rhs` — the operand subtracted from `self` lane-by-lane.
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings; `O(LANES)` for scalar-array
    /// encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(0));
    /// let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
    /// assert_eq!(a.sub(b).lane(0), Fp::<3>::new(2)); // 0 - 1 == 2 mod 3
    /// ```
    fn sub(self, rhs: Self) -> Self;

    /// Lane-wise additive inverse.
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings; `O(LANES)` for scalar-array
    /// encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
    /// assert_eq!(a.neg().lane(0), Fp::<3>::new(2)); // -1 == 2 mod 3
    /// ```
    fn neg(self) -> Self;

    /// Lane-wise product.
    ///
    /// # Arguments
    ///
    /// * `rhs` — the other operand; lanes are multiplied pointwise.
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings; `O(LANES)` for scalar-array
    /// encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// assert_eq!(a.mul(b).lane(0), Fp::<3>::new(1)); // 2 * 2 == 1 mod 3
    /// ```
    fn mul(self, rhs: Self) -> Self;

    /// Decode lane `i` to a canonical `F` value.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..Self::LANES`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= Self::LANES`.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a bit-extract or array-index at a fixed position plus a
    /// constant decode.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let v = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// assert_eq!(v.lane(0), Fp::<3>::new(2));
    /// ```
    fn lane(self, i: usize) -> F;

    /// Encode `x` into lane `i`, returning the updated value.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..Self::LANES`.
    /// * `x` — scalar to write into lane `i` (in canonical encoding).
    ///
    /// # Panics
    ///
    /// Panics if `i >= Self::LANES`.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a constant number of bit-mask updates or an array store
    /// at a fixed index.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let v = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
    /// let v = v.with_lane(7, Fp::<3>::new(2));
    /// assert_eq!(v.lane(7), Fp::<3>::new(2));
    /// assert_eq!(v.lane(0), Fp::<3>::new(0));
    /// ```
    fn with_lane(self, i: usize, x: F) -> Self;

    /// Returns `true` iff every lane decodes to `F`'s additive identity.
    ///
    /// Implementations MUST canonicalise: a redundant non-canonical
    /// "zero" codeword (e.g. bipedal `(0, 1)`) still answers `true`
    /// (D1b §3.5).
    ///
    /// # Complexity
    ///
    /// `O(1)` for fixed-width encodings (a constant-width comparison
    /// against the all-zero encoding, independent of `LANES`);
    /// `O(LANES)` for scalar-array encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
    /// use gf2_core::gfp::Fp;
    /// let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
    /// assert!(z.all_zero());
    /// let o = <ScalarPackedFp3 as PackedField<Fp<3>>>::one();
    /// assert!(!o.all_zero());
    /// ```
    fn all_zero(self) -> bool;
}

/// Variable-length lane-parallel container of `F`-elements.
///
/// Where [`PackedField<F>`] is a fixed-`LANES` packed value (one Rust
/// value, no allocation), `PackedFieldVec<F>` is a heap-allocated
/// sequence of `F`-elements with element-wise lane semantics. Each
/// logical position `0..len()` holds one `F`; the [`Self::Element`]
/// associated type points at the matching fixed-width packed type that
/// SIMD-batched implementations may use under the hood — the scalar
/// reference [`scalar::ScalarPackedFp3Vec`] does not require this hook
/// and stores one `F` per logical position directly.
///
/// The trait surface is fixed verbatim by
/// `dev/plans/9fe275d3/d1b_packed_field_api.md` §2.2, user-approved 2026-05-09
/// (JIT issue `9fe275d3`'s description `## Approval` section). The
/// signatures are frozen at the W6 `gate:api-freeze` of the
/// `gf2-algebra-permanent` epic; in-loop amendment is permitted only
/// before that gate fires.
///
/// # Trait bounds
///
/// `Clone + Eq + Debug` — heap-backed (so not `Copy`), value-equal on
/// canonical-decode (D1b §3.4), and printable for `assert_eq!` panic
/// messages.
///
/// # Length semantics
///
/// All in-place operators ([`Self::add_assign`], [`Self::sub_assign`],
/// [`Self::mul_assign`]) require `self.len() == rhs.len()`; passing
/// mismatched lengths panics. D1b §2.2 leaves the convention to each
/// impl; this trait fixes it as `length-equal-or-panic` so callers
/// have a uniform contract across all impls.
///
/// # No `unsafe`
///
/// This trait is `#![deny(unsafe_code)]`-compatible; SIMD intrinsics
/// live behind safe function-pointer bundles in `gf2-kernels-simd`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
/// use gf2_core::gfp::Fp;
///
/// let xs = [Fp::<3>::new(1), Fp::<3>::new(2), Fp::<3>::new(0)];
/// let v = ScalarPackedFp3Vec::from_field_slice(&xs);
/// assert_eq!(v.len(), 3);
/// assert_eq!(v.get(1), Fp::<3>::new(2));
/// ```
pub trait PackedFieldVec<F: FiniteField>: Clone + Eq + core::fmt::Debug {
    /// Fixed-LANES packed companion type used by SIMD-batched impls.
    ///
    /// The scalar reference does not need this hook (it stores one
    /// `F` per logical position directly), but `Bipedal3Vec` and
    /// future SIMD-batched impls will store internal data in
    /// `Self::Element` chunks. Carrying the bound on the trait makes
    /// the relationship visible to downstream generic code without
    /// committing the scalar reference to actually use it.
    type Element: PackedField<F>;

    /// Construct a vector of `len` zeros.
    ///
    /// # Arguments
    ///
    /// * `len` — number of logical `F`-positions in the result.
    ///
    /// # Complexity
    ///
    /// `O(len)` for scalar-array encodings; `O(ceil(len / Element::LANES))`
    /// for fixed-width-chunked encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let z = ScalarPackedFp3Vec::zeros(5);
    /// assert_eq!(z.len(), 5);
    /// assert!(z.all_zero());
    /// ```
    fn zeros(len: usize) -> Self;

    /// Construct a vector by copying every element of `xs` into a
    /// fresh logical position.
    ///
    /// # Arguments
    ///
    /// * `xs` — source slice; the result has `xs.len()` logical
    ///   positions and `get(i) == xs[i]` for every `i`.
    ///
    /// # Complexity
    ///
    /// `O(xs.len())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let xs = [Fp::<3>::new(1), Fp::<3>::new(2)];
    /// let v = ScalarPackedFp3Vec::from_field_slice(&xs);
    /// assert_eq!(v.get(0), Fp::<3>::new(1));
    /// assert_eq!(v.get(1), Fp::<3>::new(2));
    /// ```
    fn from_field_slice(xs: &[F]) -> Self;

    /// Number of logical `F`-positions held by this vector.
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// let v = ScalarPackedFp3Vec::zeros(7);
    /// assert_eq!(v.len(), 7);
    /// ```
    fn len(&self) -> usize;

    /// Returns `true` iff `self.len() == 0`.
    ///
    /// The default implementation is the natural one; concrete impls
    /// rarely need to override it.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// assert!(ScalarPackedFp3Vec::zeros(0).is_empty());
    /// assert!(!ScalarPackedFp3Vec::zeros(1).is_empty());
    /// ```
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Decode logical position `i` to a canonical `F` value.
    ///
    /// # Arguments
    ///
    /// * `i` — logical position index in `0..self.len()`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let xs = [Fp::<3>::new(2)];
    /// let v = ScalarPackedFp3Vec::from_field_slice(&xs);
    /// assert_eq!(v.get(0), Fp::<3>::new(2));
    /// ```
    fn get(&self, i: usize) -> F;

    /// Lane-wise in-place sum: `self[i] += rhs[i]` for every `i`.
    ///
    /// # Arguments
    ///
    /// * `rhs` — operand of equal length; positions are added pointwise.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()` (D1b §2.2 leaves the
    /// convention open; this trait fixes it as strict-equal-or-panic
    /// so callers have a uniform contract).
    ///
    /// # Complexity
    ///
    /// `O(self.len())` for scalar-array encodings;
    /// `O(self.len() / Element::LANES)` for fixed-width-chunked encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1), Fp::<3>::new(2)]);
    /// let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2), Fp::<3>::new(2)]);
    /// a.add_assign(&b);
    /// assert_eq!(a.get(0), Fp::<3>::new(0)); // 1 + 2 == 0 mod 3
    /// assert_eq!(a.get(1), Fp::<3>::new(1)); // 2 + 2 == 1 mod 3
    /// ```
    fn add_assign(&mut self, rhs: &Self);

    /// Lane-wise in-place difference: `self[i] -= rhs[i]` for every `i`.
    ///
    /// # Arguments
    ///
    /// * `rhs` — operand of equal length; subtracted pointwise from `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()` (same convention as
    /// [`Self::add_assign`]).
    ///
    /// # Complexity
    ///
    /// `O(self.len())` for scalar-array encodings;
    /// `O(self.len() / Element::LANES)` for fixed-width-chunked encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(0)]);
    /// let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
    /// a.sub_assign(&b);
    /// assert_eq!(a.get(0), Fp::<3>::new(2)); // 0 - 1 == 2 mod 3
    /// ```
    fn sub_assign(&mut self, rhs: &Self);

    /// Lane-wise in-place product: `self[i] *= rhs[i]` for every `i`.
    ///
    /// # Arguments
    ///
    /// * `rhs` — operand of equal length; multiplied pointwise into `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()` (same convention as
    /// [`Self::add_assign`]).
    ///
    /// # Complexity
    ///
    /// `O(self.len())` for scalar-array encodings;
    /// `O(self.len() / Element::LANES)` for fixed-width-chunked encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
    /// let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
    /// a.mul_assign(&b);
    /// assert_eq!(a.get(0), Fp::<3>::new(1)); // 2 * 2 == 1 mod 3
    /// ```
    fn mul_assign(&mut self, rhs: &Self);

    /// Returns `true` iff every logical position decodes to `F`'s
    /// additive identity.
    ///
    /// Implementations MUST canonicalise: a redundant non-canonical
    /// "zero" codeword (e.g. bipedal `(0, 1)`) still answers `true`
    /// (D1b §3.5). The empty vector trivially answers `true`.
    ///
    /// # Complexity
    ///
    /// `O(self.len())` for scalar-array encodings;
    /// `O(self.len() / Element::LANES)` for fixed-width-chunked encodings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// assert!(ScalarPackedFp3Vec::zeros(5).all_zero());
    /// let nz = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
    /// assert!(!nz.all_zero());
    /// ```
    fn all_zero(&self) -> bool;
}
