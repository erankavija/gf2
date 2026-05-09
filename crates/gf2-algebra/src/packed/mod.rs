//! Packed finite-field abstractions.
//!
//! Hosts the [`PackedField`] trait that abstracts lane-parallel
//! arithmetic over a small prime field, plus the concrete
//! `Bipedal{3,5,7}` element / vector / matrix types that implement it,
//! and the [`scalar::ScalarPackedFp3`] correctness oracle.
//!
//! The trait surface is fixed by `dev/plans/d1b_packed_field_api.md`
//! (user-approved 2026-05-09; see JIT issue `9fe275d3`'s description
//! `## Approval` section) and frozen at the W6 `gate:api-freeze`. The
//! W1-T1 skeleton declared the module tree only; this module (W1-T2)
//! lands the [`PackedField`] trait and the [`scalar::ScalarPackedFp3`]
//! reference implementation. The concrete `Bipedal3` impl lands in
//! W1-T3, and the `PackedFieldVec` companion trait lands in a later
//! wave (parent §13). Until that point, the
//! [`scalar::ScalarPackedFp3`] oracle is the only [`PackedField`]
//! implementor in the workspace.
//!
//! # Cross-checking strategy
//!
//! Other `PackedField<Fp<3>>` impls (notably `bipedal3::Bipedal3` in
//! W1-T3 and `bipedal5::Bipedal5` / `bipedal7::Bipedal7` later) are
//! validated against [`scalar::ScalarPackedFp3`] by routing the same
//! random inputs through both impls and asserting `lane(i)` agrees
//! across all 64 lanes. The oracle has no SIMD or bit-packing
//! optimisations — it is intentionally one `Fp<3>` per lane.

use gf2_core::field::FiniteField;

pub mod bipedal3;
pub mod scalar;

#[cfg(feature = "f5")]
pub mod bipedal5;

#[cfg(feature = "f7")]
pub mod bipedal7;

pub use scalar::ScalarPackedFp3;

/// Fixed-LANES lane-parallel arithmetic over an underlying scalar field `F`.
///
/// A `PackedField<F>` value carries [`Self::LANES`] independent
/// `F`-elements in one Rust value. Lane operations are constant-time
/// across all lanes (no per-lane branching); a `LANES = 64` instance
/// maps onto a pair of `u64`s in the bipedal3 encoding, a
/// `LANES = 256` instance maps onto an AVX2 `__m256i` pair, and so on.
///
/// The trait surface is fixed by
/// `dev/plans/d1b_packed_field_api.md` §2.1, user-approved 2026-05-09
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
    /// permitted (the future `Bipedal5` encoding packs 21 lanes per
    /// `u64`, for example).
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
