//! Trait-surface stub for `gf2-algebra::packed::{PackedField, PackedFieldVec}`
//! and `gf2-algebra::permanent::Permanent`.
//!
//! This crate is a **non-workspace research stub** living at
//! `dev/research/packed_field_stub/`. It exists to prove the W0 / D1b trait
//! surface compiles against the real `gf2-core::field::FiniteField` and
//! `gf2-core::gfp::Fp<P>` bounds, without committing the surface to the
//! production workspace until the user-approval gate clears.
//!
//! See `dev/plans/d1b_packed_field_api.md` for the design rationale.
//!
//! # Layout
//!
//! - [`PackedField`] — fixed-LANES lane-parallel arithmetic over `F`.
//! - [`PackedFieldVec`] — variable-length analogue, `Vec<u64>`-backed.
//! - [`Permanent`] — an associated-type trait for "this matrix can produce a
//!   permanent over its field."
//! - [`bipedal3::Bipedal3`] — element type packing 64 lanes of `Fp<3>` into
//!   two `u64`s; demonstrates `PackedField<Fp<3>>`.
//! - [`bipedal3::Bipedal3Vec`] — variable-length analogue; demonstrates
//!   `PackedFieldVec<Fp<3>>`.
//! - [`bipedal3::Bipedal3Matrix`] — square matrix shell; demonstrates
//!   `Permanent<Fp<3>>` with a stub permanent body.
//!
//! # Examples
//!
//! ```
//! use packed_field_stub::{Bipedal3, Bipedal3Matrix, PackedField, Permanent, F3};
//!
//! // Demonstrate the trait surface against the real `Fp<3>`:
//! let v = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
//! assert_eq!(v.lane(0), F3::new(2));
//!
//! let m = Bipedal3Matrix::zeros(2);
//! assert_eq!(<Bipedal3Matrix as Permanent>::permanent(&m), F3::new(0));
//! ```

#![deny(unsafe_code)]
#![deny(missing_docs)]

use gf2_core::field::FiniteField;

/// Fixed-LANES lane-parallel arithmetic over an underlying scalar field `F`.
///
/// A `PackedField<F>` value carries [`Self::LANES`] independent `F`-elements
/// in one Rust value. Lane operations are constant-time across all lanes
/// (no per-lane branching); a `LANES = 64` instance maps onto a pair of
/// `u64`s, a `LANES = 256` instance maps onto an AVX2 `__m256i` pair, etc.
///
/// # Trait bounds
///
/// `Copy + Eq + Debug` — all decided values, no allocation, no
/// fallible construction. `Eq` is the **canonical-decode** equality:
/// two packed values are equal iff every decoded lane is equal in `F`,
/// regardless of any internal redundancy in the encoding (see the
/// alternative-zero discussion in `d1b_packed_field_api.md` §3).
///
/// # Lane semantics
///
/// `lane(i)` returns the canonical `F` value of lane `i`, decoding any
/// implementation-internal redundancy (e.g. the bipedal `(0, 1)`
/// alternative-zero codeword decodes to `Fp<3>::ZERO`). `with_lane(i, x)`
/// writes the canonical encoding of `x` into lane `i`.
///
/// # No `unsafe`
///
/// This trait is `#![deny(unsafe_code)]`; SIMD intrinsics live behind
/// safe function-pointer bundles in `gf2-kernels-simd`.
///
/// # Examples
///
/// ```
/// use packed_field_stub::{Bipedal3, PackedField, F3};
///
/// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
/// let b = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
/// let s = a.add(b);
/// assert_eq!(s.lane(0), F3::new(0)); // 1 + 2 == 0 mod 3
/// ```
pub trait PackedField<F: FiniteField>: Copy + Eq + core::fmt::Debug {
    /// Number of independent `F`-lanes packed into one `Self`.
    ///
    /// Must be positive. Power-of-two is preferred for SIMD-friendly
    /// mapping where feasible, but non-power-of-two values are permitted
    /// (e.g., a future `Bipedal5` encoding packs 21 lanes per `u64`).
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// assert_eq!(<Bipedal3 as PackedField<F3>>::LANES, 64);
    /// ```
    const LANES: usize;

    /// All-lanes-zero constant.
    ///
    /// # Complexity
    ///
    /// `O(1)`: returns a constant-width all-zero encoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let z = <Bipedal3 as PackedField<F3>>::zero();
    /// assert!(z.all_zero());
    /// ```
    fn zero() -> Self;

    /// All-lanes-one constant.
    ///
    /// # Complexity
    ///
    /// `O(1)`: returns a constant-width all-one encoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let o = <Bipedal3 as PackedField<F3>>::one();
    /// assert_eq!(o.lane(0), F3::new(1));
    /// assert_eq!(o.lane(63), F3::new(1));
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
    /// `O(1)`: a fixed-width broadcast irrespective of `LANES`, since
    /// `LANES` is a compile-time constant and the encoded result is one
    /// machine value (or a fixed-tuple of machine values).
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let v = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
    /// for i in 0..<Bipedal3 as PackedField<F3>>::LANES {
    ///     assert_eq!(v.lane(i), F3::new(2));
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
    /// `O(1)`: a fixed number of word-level bitwise ops, independent of
    /// `LANES` (the per-lane work is implicit in the encoding's
    /// bit-parallel formulas).
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
    /// let b = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
    /// assert_eq!(a.add(b).lane(0), F3::new(1)); // 2 + 2 == 1 mod 3
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
    /// `O(1)`: a fixed number of word-level bitwise ops, independent of
    /// `LANES`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(0));
    /// let b = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
    /// assert_eq!(a.sub(b).lane(0), F3::new(2)); // 0 - 1 == 2 mod 3
    /// ```
    fn sub(self, rhs: Self) -> Self;

    /// Lane-wise additive inverse.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a fixed number of word-level bitwise ops, independent of
    /// `LANES`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
    /// assert_eq!(a.neg().lane(0), F3::new(2)); // -1 == 2 mod 3
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
    /// `O(1)`: a fixed number of word-level bitwise ops, independent of
    /// `LANES`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
    /// let b = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
    /// assert_eq!(a.mul(b).lane(0), F3::new(1)); // 2 * 2 == 1 mod 3
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
    /// `O(1)`: bit-extract at a fixed index plus a constant decode.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let v = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
    /// assert_eq!(v.lane(0), F3::new(2));
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
    /// `O(1)`: a constant number of bit-mask updates at a fixed index.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let v = <Bipedal3 as PackedField<F3>>::zero();
    /// let v = v.with_lane(7, F3::new(2));
    /// assert_eq!(v.lane(7), F3::new(2));
    /// assert_eq!(v.lane(0), F3::new(0));
    /// ```
    fn with_lane(self, i: usize, x: F) -> Self;

    /// Returns `true` iff every lane decodes to `F`'s additive identity.
    ///
    /// Implementations MUST canonicalise: a redundant non-canonical
    /// "zero" codeword (e.g. bipedal `(0, 1)`) still answers `true`.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a constant-width comparison against the all-zero encoding,
    /// independent of `LANES`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let z = <Bipedal3 as PackedField<F3>>::zero();
    /// assert!(z.all_zero());
    /// let o = <Bipedal3 as PackedField<F3>>::one();
    /// assert!(!o.all_zero());
    /// ```
    fn all_zero(self) -> bool;
}

/// Variable-length lane-parallel storage over an underlying scalar field `F`.
///
/// `PackedFieldVec<F>` is the heap-backed analogue of [`PackedField<F>`].
/// Storage is a `Vec<u64>` (or pair of `Vec<u64>`s, etc.) with a length in
/// elements; the tail-mask invariant per CLAUDE.md §Key design invariants
/// applies — bits beyond the logical length must be zero after every
/// mutating op.
///
/// # `fold_mul` placement
///
/// The `fold_mul` reduction (lane-product across all lanes of a single
/// packed word, used inside the Gray-code Ryser inner loop) is **not** on
/// this trait. It lives as an inherent method on each concrete
/// `PackedFieldVec` impl so each type can pick its own log-tree shape and
/// SIMD reduction strategy. See `d1b_packed_field_api.md` §3.
///
/// # `Eq`
///
/// `Eq` is the **canonical-decode** equality across the full logical
/// length — same contract as [`PackedField::lane`].
///
/// # Examples
///
/// ```
/// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
///
/// let xs = [F3::new(0), F3::new(1), F3::new(2)];
/// let v = Bipedal3Vec::from_field_slice(&xs);
/// assert_eq!(v.len(), 3);
/// assert_eq!(v.get(2), F3::new(2));
/// ```
pub trait PackedFieldVec<F: FiniteField>: Clone + Eq + core::fmt::Debug {
    /// The fixed-LANES element type for this variable-length storage.
    type Element: PackedField<F>;

    /// Construct an all-zero vector of `len` field elements.
    ///
    /// # Arguments
    ///
    /// * `len` — number of `F`-elements (lanes) the vector holds.
    ///
    /// # Complexity
    ///
    /// `O(len / Self::Element::LANES)` word-level allocations and zero
    /// writes. For `len = 0` no heap memory is reserved.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let v = <Bipedal3Vec as PackedFieldVec<F3>>::zeros(8);
    /// assert_eq!(v.len(), 8);
    /// assert!(v.all_zero());
    /// ```
    fn zeros(len: usize) -> Self;

    /// Construct from a slice of `F` elements.
    ///
    /// # Arguments
    ///
    /// * `xs` — slice of field elements; index `i` becomes element `i`.
    ///
    /// # Complexity
    ///
    /// `O(xs.len())` element-level encodes. The implementation MAY use
    /// word-level packing internally, but the asymptotic bound is
    /// linear in the input length.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let v = Bipedal3Vec::from_field_slice(&[F3::new(2), F3::new(1)]);
    /// assert_eq!(v.get(0), F3::new(2));
    /// assert_eq!(v.get(1), F3::new(1));
    /// ```
    fn from_field_slice(xs: &[F]) -> Self;

    /// Number of `F`-elements stored.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a stored length field is returned directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let v = <Bipedal3Vec as PackedFieldVec<F3>>::zeros(5);
    /// assert_eq!(v.len(), 5);
    /// ```
    fn len(&self) -> usize;

    /// `true` iff `len() == 0`.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a single comparison against zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let v = <Bipedal3Vec as PackedFieldVec<F3>>::zeros(0);
    /// assert!(v.is_empty());
    /// ```
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Decode element `i` to canonical `F`.
    ///
    /// # Arguments
    ///
    /// * `i` — element index in `0..self.len()`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a single word index plus a constant-width bit-extract and
    /// decode.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let v = Bipedal3Vec::from_field_slice(&[F3::new(2)]);
    /// assert_eq!(v.get(0), F3::new(2));
    /// ```
    fn get(&self, i: usize) -> F;

    /// In-place `self += rhs`. Lengths must match.
    ///
    /// # Arguments
    ///
    /// * `rhs` — vector added pointwise into `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Complexity
    ///
    /// `O(self.len() / Self::Element::LANES)` word-level adds, i.e.
    /// `O(self.len())` lane-equivalent work, plus a final tail-mask
    /// step that is amortised `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let mut a = Bipedal3Vec::from_field_slice(&[F3::new(1)]);
    /// let b = Bipedal3Vec::from_field_slice(&[F3::new(2)]);
    /// a.add_assign(&b);
    /// assert_eq!(a.get(0), F3::new(0)); // 1 + 2 == 0 mod 3
    /// ```
    fn add_assign(&mut self, rhs: &Self);

    /// In-place `self -= rhs`. Lengths must match.
    ///
    /// # Arguments
    ///
    /// * `rhs` — vector subtracted pointwise from `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Complexity
    ///
    /// `O(self.len() / Self::Element::LANES)` word-level subtracts, i.e.
    /// `O(self.len())` lane-equivalent work, plus a final tail-mask
    /// step that is amortised `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let mut a = Bipedal3Vec::from_field_slice(&[F3::new(0)]);
    /// let b = Bipedal3Vec::from_field_slice(&[F3::new(1)]);
    /// a.sub_assign(&b);
    /// assert_eq!(a.get(0), F3::new(2)); // 0 - 1 == 2 mod 3
    /// ```
    fn sub_assign(&mut self, rhs: &Self);

    /// In-place `self *= rhs`. Lengths must match.
    ///
    /// # Arguments
    ///
    /// * `rhs` — vector multiplied pointwise into `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Complexity
    ///
    /// `O(self.len() / Self::Element::LANES)` word-level multiplies,
    /// i.e. `O(self.len())` lane-equivalent work, plus a final
    /// tail-mask step that is amortised `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let mut a = Bipedal3Vec::from_field_slice(&[F3::new(2)]);
    /// let b = Bipedal3Vec::from_field_slice(&[F3::new(2)]);
    /// a.mul_assign(&b);
    /// assert_eq!(a.get(0), F3::new(1)); // 2 * 2 == 1 mod 3
    /// ```
    fn mul_assign(&mut self, rhs: &Self);

    /// `true` iff every element is the additive identity of `F`.
    ///
    /// # Complexity
    ///
    /// `O(self.len() / Self::Element::LANES)` word-level checks; early
    /// exit on the first non-zero word is permitted but not required.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let v = <Bipedal3Vec as PackedFieldVec<F3>>::zeros(4);
    /// assert!(v.all_zero());
    /// ```
    fn all_zero(&self) -> bool;
}

/// "This matrix-like value can produce its permanent over `F`."
///
/// The associated [`Self::Field`] type pins the scalar field; the
/// `permanent` method consumes `&self` because some impls (e.g. Ryser
/// over a packed matrix) walk a Gray-code state internally and hold
/// scratch buffers behind interior mutability.
///
/// # Why an associated type instead of a parameter
///
/// Each concrete matrix type knows its own `F` at compile time
/// (`Bipedal3Matrix` is always over `Fp<3>`). An associated type lets
/// callers write `M::Field` without naming the parameter.
///
/// # Examples
///
/// ```
/// use packed_field_stub::{Bipedal3Matrix, Permanent, F3};
///
/// let m = Bipedal3Matrix::zeros(3);
/// let p: F3 = m.permanent();
/// assert_eq!(p, F3::new(0)); // permanent of the zero matrix is 0
/// ```
pub trait Permanent {
    /// The scalar field over which the permanent is computed.
    type Field: FiniteField;

    /// Compute the permanent.
    ///
    /// # Complexity
    ///
    /// Implementation-defined; for Ryser over `n × n` it is `O(n · 2^n)`
    /// field operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Matrix, Permanent, F3};
    /// let m = Bipedal3Matrix::zeros(2);
    /// assert_eq!(<Bipedal3Matrix as Permanent>::permanent(&m), F3::new(0));
    /// ```
    fn permanent(&self) -> Self::Field;
}

// ---------------------------------------------------------------------------
// Bipedal3 demonstration: PackedField<Fp<3>> + PackedFieldVec<Fp<3>> +
// Permanent (with Field = Fp<3>).
// ---------------------------------------------------------------------------

/// `Fp<3>` type alias for the demonstration.
///
/// # Examples
///
/// ```
/// use packed_field_stub::F3;
/// let x = F3::new(2);
/// assert_eq!(x.value(), 2);
/// ```
pub type F3 = gf2_core::gfp::Fp<3>;

pub use bipedal3::{Bipedal3, Bipedal3Matrix, Bipedal3Vec};

pub mod bipedal3 {
    //! Bipedal F_3 element / vec / matrix demonstrating the trait surface.
    //!
    //! See the parent module docs for the trait contracts. The encoding is
    //! the Scheinerman 2024 §2.2 bipedal map: `0 ↦ (0, 0)`, `1 ↦ (1, 0)`,
    //! `2 ↦ (1, 1)`. The `(0, 1)` codeword is the alternative-zero — it
    //! decodes to `0` in `F_3` but is never produced by a constructor in
    //! this crate (the `lane` decoder accepts it for robustness).

    use super::{FiniteField, PackedField, PackedFieldVec, Permanent, F3};

    /// 64 lanes of F_3 packed into two `u64`s.
    ///
    /// `mag` is the "is nonzero" plane; `sgn` is the "is `−1`" plane.
    /// Encoding: lane `i` decodes via `psi(mag_i, sgn_i)` where
    /// `psi(false, _) = 0`, `psi(true, false) = 1`, `psi(true, true) = 2`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3, PackedField, F3};
    /// let v = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
    /// assert_eq!(v.lane(0), F3::new(1));
    /// ```
    #[derive(Copy, Clone, Debug)]
    pub struct Bipedal3 {
        /// "Nonzero" mask plane (lane `i` is nonzero iff bit `i` is set).
        pub mag: u64,
        /// "Sign" plane (only meaningful when `mag` bit is set).
        pub sgn: u64,
    }

    impl Bipedal3 {
        /// All-lanes-zero constant.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3, PackedField, F3};
        /// assert!(Bipedal3::ZERO.all_zero());
        /// ```
        pub const ZERO: Self = Self { mag: 0, sgn: 0 };
        /// All-lanes-one constant.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3, PackedField, F3};
        /// assert_eq!(Bipedal3::ONE.lane(0), F3::new(1));
        /// ```
        pub const ONE: Self = Self { mag: !0, sgn: 0 };

        /// Canonicalised plane pair: `(mag, sgn & mag)` — clears the
        /// alternative-zero bits so bit-equality coincides with
        /// canonical-decode equality.
        #[inline]
        const fn canonical_planes(self) -> (u64, u64) {
            (self.mag, self.sgn & self.mag)
        }

        /// Paper §2.2 add formula (6 ops).
        ///
        /// # Arguments
        ///
        /// * `r` — second operand; lanes added pointwise mod 3.
        ///
        /// # Complexity
        ///
        /// `O(1)`: exactly six bitwise ops on a fixed pair of `u64`s,
        /// independent of the 64 lanes packed inside.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3, PackedField, F3};
        /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
        /// let b = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
        /// assert_eq!(a.add_const(b).lane(0), F3::new(2));
        /// ```
        #[inline]
        pub const fn add_const(self, r: Self) -> Self {
            let t = self.mag ^ self.sgn ^ r.sgn;
            let u = r.mag & t;
            Self {
                mag: u | (self.mag ^ r.mag),
                sgn: u ^ self.sgn,
            }
        }

        /// Paper §2.2 sub formula (6 ops).
        ///
        /// # Arguments
        ///
        /// * `r` — second operand; lanes subtracted pointwise mod 3.
        ///
        /// # Complexity
        ///
        /// `O(1)`: exactly six bitwise ops on a fixed pair of `u64`s,
        /// independent of the 64 lanes packed inside.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3, PackedField, F3};
        /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(0));
        /// let b = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
        /// assert_eq!(a.sub_const(b).lane(0), F3::new(2));
        /// ```
        #[inline]
        pub const fn sub_const(self, r: Self) -> Self {
            let t = self.sgn ^ r.sgn;
            let u = self.mag & t;
            Self {
                mag: u | (self.mag ^ r.mag),
                sgn: u ^ (r.mag ^ r.sgn),
            }
        }

        /// Paper §2.2 mul formula (2 ops).
        ///
        /// # Arguments
        ///
        /// * `r` — second operand; lanes multiplied pointwise mod 3.
        ///
        /// # Complexity
        ///
        /// `O(1)`: exactly two bitwise ops on a fixed pair of `u64`s,
        /// independent of the 64 lanes packed inside.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3, PackedField, F3};
        /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
        /// let b = <Bipedal3 as PackedField<F3>>::splat(F3::new(2));
        /// assert_eq!(a.mul_const(b).lane(0), F3::new(1));
        /// ```
        #[inline]
        pub const fn mul_const(self, r: Self) -> Self {
            Self {
                mag: self.mag & r.mag,
                sgn: self.sgn ^ r.sgn,
            }
        }

        /// Negation: zero stays zero; nonzero flips sign.
        ///
        /// # Complexity
        ///
        /// `O(1)`: a single bitwise XOR on a fixed pair of `u64`s,
        /// independent of the 64 lanes packed inside.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3, PackedField, F3};
        /// let a = <Bipedal3 as PackedField<F3>>::splat(F3::new(1));
        /// assert_eq!(a.neg_const().lane(0), F3::new(2));
        /// ```
        #[inline]
        pub const fn neg_const(self) -> Self {
            Self {
                mag: self.mag,
                sgn: self.sgn ^ self.mag,
            }
        }
    }

    impl PartialEq for Bipedal3 {
        /// Canonical-decode equality (NOT bit-pattern equality).
        ///
        /// `(mag=0, sgn=0)` and `(mag=0, sgn=1)` both decode to `0` and so
        /// compare equal under this `==`. See `d1b_packed_field_api.md`
        /// §3.4 for the rationale.
        fn eq(&self, other: &Self) -> bool {
            self.canonical_planes() == other.canonical_planes()
        }
    }

    impl Eq for Bipedal3 {}

    impl PackedField<F3> for Bipedal3 {
        const LANES: usize = 64;

        #[inline]
        fn zero() -> Self {
            Self::ZERO
        }

        #[inline]
        fn one() -> Self {
            Self::ONE
        }

        #[inline]
        fn splat(x: F3) -> Self {
            // F_3 has only three values, so we do a tiny match.
            match x.value() {
                0 => Self::ZERO,
                1 => Self::ONE,
                _ => Self { mag: !0, sgn: !0 }, // 2 ≡ −1
            }
        }

        #[inline]
        fn add(self, rhs: Self) -> Self {
            self.add_const(rhs)
        }

        #[inline]
        fn sub(self, rhs: Self) -> Self {
            self.sub_const(rhs)
        }

        #[inline]
        fn neg(self) -> Self {
            self.neg_const()
        }

        #[inline]
        fn mul(self, rhs: Self) -> Self {
            self.mul_const(rhs)
        }

        #[inline]
        fn lane(self, i: usize) -> F3 {
            assert!(i < Self::LANES, "lane index out of range");
            let m = (self.mag >> i) & 1 == 1;
            let s = (self.sgn >> i) & 1 == 1;
            // psi: (false, _) → 0; (true, false) → 1; (true, true) → 2.
            if !m {
                F3::new(0)
            } else if !s {
                F3::new(1)
            } else {
                F3::new(2)
            }
        }

        #[inline]
        fn with_lane(self, i: usize, x: F3) -> Self {
            assert!(i < Self::LANES, "lane index out of range");
            let bit = 1u64 << i;
            let (m_set, s_set) = match x.value() {
                0 => (false, false), // canonical zero — never alt-zero
                1 => (true, false),
                _ => (true, true), // 2 ≡ −1
            };
            let mag = if m_set {
                self.mag | bit
            } else {
                self.mag & !bit
            };
            let sgn = if s_set {
                self.sgn | bit
            } else {
                self.sgn & !bit
            };
            Self { mag, sgn }
        }

        #[inline]
        fn all_zero(self) -> bool {
            // Canonical and alt-zero both have mag = 0.
            self.mag == 0
        }
    }

    /// Variable-length bipedal F_3 storage.
    ///
    /// Pair of `Vec<u64>` planes with a logical length in F_3 elements.
    /// Tail-mask invariant: bits beyond `len % 64` in the last word of
    /// each plane are zero after every mutating op.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
    /// let xs = [F3::new(0), F3::new(1), F3::new(2)];
    /// let v = Bipedal3Vec::from_field_slice(&xs);
    /// assert_eq!(v.len(), 3);
    /// assert_eq!(v.get(2), F3::new(2));
    /// ```
    #[derive(Clone, Debug)]
    pub struct Bipedal3Vec {
        mag: Vec<u64>,
        sgn: Vec<u64>,
        len: usize,
    }

    impl Bipedal3Vec {
        const ELEMS_PER_WORD: usize = 64;

        fn n_words(len: usize) -> usize {
            len.div_ceil(Self::ELEMS_PER_WORD)
        }

        fn tail_mask(len: usize) -> u64 {
            let r = len % Self::ELEMS_PER_WORD;
            if r == 0 {
                !0
            } else {
                (1u64 << r) - 1
            }
        }

        fn mask_tail(&mut self) {
            if self.len.is_multiple_of(Self::ELEMS_PER_WORD) {
                return;
            }
            if let Some(last) = self.mag.last_mut() {
                *last &= Self::tail_mask(self.len);
            }
            if let Some(last) = self.sgn.last_mut() {
                *last &= Self::tail_mask(self.len);
            }
        }

        /// Inherent `fold_mul`: lane-product across the whole vector.
        ///
        /// Lives as inherent (not on `PackedFieldVec<F>`) so each impl
        /// can choose its own reduction tree. For Bipedal3Vec this is
        /// folding `mul` over all words and then reducing the surviving
        /// 64-lane pair to a single F_3 element via popcount tricks.
        ///
        /// This stub returns `Fp<3>::ZERO` unconditionally — the real
        /// implementation lives in W2 / W3. The signature is what
        /// matters for the trait surface.
        ///
        /// # Complexity
        ///
        /// Real implementation (W2 / W3): `O(self.len() / 64)`
        /// word-level multiplies plus an `O(log 64)` lane-fold to
        /// reduce the surviving pair of `u64` planes to a single F_3
        /// element. The stub body is `O(1)` because it short-circuits
        /// to `F3::new(0)`.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3Vec, PackedFieldVec, F3};
        /// let v = Bipedal3Vec::from_field_slice(&[F3::new(1), F3::new(2)]);
        /// // Stub body: always returns the F_3 zero element.
        /// assert_eq!(v.fold_mul(), F3::new(0));
        /// ```
        pub fn fold_mul(&self) -> F3 {
            let _ = (&self.mag, &self.sgn, self.len);
            F3::new(0)
        }
    }

    impl PartialEq for Bipedal3Vec {
        fn eq(&self, other: &Self) -> bool {
            if self.len != other.len {
                return false;
            }
            // Canonical-decode equality lifted to the vector: both planes
            // must agree after canonicalisation.
            let n = self.mag.len();
            for w in 0..n {
                if self.mag[w] != other.mag[w] {
                    return false;
                }
                let s_self = self.sgn[w] & self.mag[w];
                let s_other = other.sgn[w] & other.mag[w];
                if s_self != s_other {
                    return false;
                }
            }
            true
        }
    }

    impl Eq for Bipedal3Vec {}

    impl PackedFieldVec<F3> for Bipedal3Vec {
        type Element = Bipedal3;

        fn zeros(len: usize) -> Self {
            let n = Self::n_words(len);
            Self {
                mag: vec![0u64; n],
                sgn: vec![0u64; n],
                len,
            }
        }

        fn from_field_slice(xs: &[F3]) -> Self {
            let mut out = Self::zeros(xs.len());
            for (i, x) in xs.iter().enumerate() {
                let w = i / Self::ELEMS_PER_WORD;
                let s = i % Self::ELEMS_PER_WORD;
                let bit = 1u64 << s;
                let v = x.value();
                if v != 0 {
                    out.mag[w] |= bit;
                }
                if v == 2 {
                    out.sgn[w] |= bit;
                }
            }
            out.mask_tail();
            out
        }

        fn len(&self) -> usize {
            self.len
        }

        fn get(&self, i: usize) -> F3 {
            assert!(i < self.len, "index out of range");
            let w = i / Self::ELEMS_PER_WORD;
            let s = i % Self::ELEMS_PER_WORD;
            let m = (self.mag[w] >> s) & 1 == 1;
            if !m {
                return F3::new(0);
            }
            let g = (self.sgn[w] >> s) & 1 == 1;
            if g {
                F3::new(2)
            } else {
                F3::new(1)
            }
        }

        fn add_assign(&mut self, rhs: &Self) {
            assert_eq!(self.len, rhs.len, "length mismatch");
            for w in 0..self.mag.len() {
                let am = self.mag[w];
                let asg = self.sgn[w];
                let bm = rhs.mag[w];
                let bsg = rhs.sgn[w];
                let t = am ^ asg ^ bsg;
                let u = bm & t;
                self.mag[w] = u | (am ^ bm);
                self.sgn[w] = u ^ asg;
            }
            self.mask_tail();
        }

        fn sub_assign(&mut self, rhs: &Self) {
            assert_eq!(self.len, rhs.len, "length mismatch");
            for w in 0..self.mag.len() {
                let am = self.mag[w];
                let asg = self.sgn[w];
                let bm = rhs.mag[w];
                let bsg = rhs.sgn[w];
                let t = asg ^ bsg;
                let u = am & t;
                self.mag[w] = u | (am ^ bm);
                self.sgn[w] = u ^ (bm ^ bsg);
            }
            self.mask_tail();
        }

        fn mul_assign(&mut self, rhs: &Self) {
            assert_eq!(self.len, rhs.len, "length mismatch");
            for w in 0..self.mag.len() {
                self.mag[w] &= rhs.mag[w];
                self.sgn[w] ^= rhs.sgn[w];
            }
            self.mask_tail();
        }

        fn all_zero(&self) -> bool {
            self.mag.iter().all(|&w| w == 0)
        }
    }

    /// Square `n × n` bipedal F_3 matrix shell.
    ///
    /// Column-major: each column is a `Bipedal3Vec` of length `n`. This
    /// stub stores the columns as a flat `Vec<Bipedal3Vec>`; the W1/T5
    /// implementation will switch to the unified column-major
    /// `(mag, sgn)` `Vec<u64>` layout per `dev/plans/r3_multi_word_streaming.md`.
    ///
    /// # Examples
    ///
    /// ```
    /// use packed_field_stub::{Bipedal3Matrix, Permanent, F3};
    /// let m = Bipedal3Matrix::zeros(3);
    /// assert_eq!(m.n(), 3);
    /// assert_eq!(<Bipedal3Matrix as Permanent>::permanent(&m), F3::new(0));
    /// ```
    #[derive(Clone, Debug)]
    pub struct Bipedal3Matrix {
        n: usize,
        cols: Vec<Bipedal3Vec>,
    }

    impl Bipedal3Matrix {
        /// Construct a zero `n × n` bipedal F_3 matrix.
        ///
        /// # Arguments
        ///
        /// * `n` — side length of the square matrix.
        ///
        /// # Complexity
        ///
        /// `O(n^2 / 64)` word-level zero writes (`n` columns each with
        /// `n` lanes packed into `⌈n / 64⌉` words), i.e. `O(n^2)`
        /// lane-equivalent work.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::Bipedal3Matrix;
        /// let m = Bipedal3Matrix::zeros(4);
        /// assert_eq!(m.n(), 4);
        /// ```
        pub fn zeros(n: usize) -> Self {
            Self {
                n,
                cols: (0..n).map(|_| Bipedal3Vec::zeros(n)).collect(),
            }
        }

        /// Side length.
        ///
        /// # Complexity
        ///
        /// `O(1)`: returns the stored side-length field directly.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::Bipedal3Matrix;
        /// let m = Bipedal3Matrix::zeros(7);
        /// assert_eq!(m.n(), 7);
        /// ```
        pub fn n(&self) -> usize {
            self.n
        }

        /// Borrow column `j`.
        ///
        /// # Arguments
        ///
        /// * `j` — column index in `0..self.n()`.
        ///
        /// # Panics
        ///
        /// Panics if `j >= self.n()`.
        ///
        /// # Complexity
        ///
        /// `O(1)`: a single slice index returns a borrow of the
        /// pre-stored column. (The column itself owns `O(n / 64)` words
        /// of storage, but the borrow is constant-time.)
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3Matrix, PackedFieldVec};
        /// let m = Bipedal3Matrix::zeros(3);
        /// assert_eq!(m.column(0).len(), 3);
        /// ```
        pub fn column(&self, j: usize) -> &Bipedal3Vec {
            &self.cols[j]
        }
    }

    impl Permanent for Bipedal3Matrix {
        type Field = F3;

        /// Stub implementation returning the F_3 zero element.
        ///
        /// The real `permanent_bipedal3_single` (W2/T9) and
        /// `permanent_bipedal3_multi` (W3/T14) will live in `gf2-algebra`
        /// per D1a §2 and replace this shell. The stub exists only to
        /// confirm the trait surface compiles.
        ///
        /// # Complexity
        ///
        /// Stub body: `O(1)` — short-circuits to `F3::new(0)`. The real
        /// implementation will be Gray-code Ryser at `O(n · 2^n)` field
        /// operations, matching the `Permanent::permanent` contract.
        ///
        /// # Examples
        ///
        /// ```
        /// use packed_field_stub::{Bipedal3Matrix, Permanent, F3};
        /// let m = Bipedal3Matrix::zeros(2);
        /// assert_eq!(<Bipedal3Matrix as Permanent>::permanent(&m), F3::new(0));
        /// ```
        fn permanent(&self) -> F3 {
            // Reference fields so they are not "unused".
            let _ = (self.n, self.cols.len());
            F3::new(0)
        }
    }

    // -----------------------------------------------------------------------
    // Compile-time bound checks.
    //
    // These functions never run; their existence is what proves the impls
    // satisfy the trait bounds against the *real* `gf2_core::field::FiniteField`
    // and `gf2_core::gfp::Fp<3>` types.
    // -----------------------------------------------------------------------

    fn _assert_packed_field<T: PackedField<F3>>() {}
    fn _assert_packed_field_vec<T: PackedFieldVec<F3>>() {}
    fn _assert_permanent<T: Permanent<Field = F3>>() {}

    fn _bound_checks() {
        _assert_packed_field::<Bipedal3>();
        _assert_packed_field_vec::<Bipedal3Vec>();
        _assert_permanent::<Bipedal3Matrix>();
    }

    // Use `FiniteField` directly so the import is not flagged unused on
    // crates that wire it via the `super` re-export.
    #[allow(dead_code)]
    fn _f3_is_finite_field<F: FiniteField>(x: F) -> F {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::bipedal3::{Bipedal3, Bipedal3Matrix, Bipedal3Vec};
    use super::{PackedField, PackedFieldVec, Permanent, F3};

    #[test]
    fn bipedal3_packed_field_basic() {
        let z = <Bipedal3 as PackedField<F3>>::zero();
        let o = <Bipedal3 as PackedField<F3>>::one();
        assert!(z.all_zero());
        assert!(!o.all_zero());
        assert_eq!(z.lane(0), F3::new(0));
        assert_eq!(o.lane(0), F3::new(1));
        assert_eq!(o.lane(63), F3::new(1));

        // splat round-trip across all three F_3 values.
        for v in 0u64..3 {
            let p = <Bipedal3 as PackedField<F3>>::splat(F3::new(v));
            for i in 0..<Bipedal3 as PackedField<F3>>::LANES {
                assert_eq!(p.lane(i), F3::new(v), "splat lane {i} for {v}");
            }
        }

        // with_lane round-trip.
        let mut p = <Bipedal3 as PackedField<F3>>::zero();
        p = p.with_lane(7, F3::new(2));
        p = p.with_lane(13, F3::new(1));
        assert_eq!(p.lane(7), F3::new(2));
        assert_eq!(p.lane(13), F3::new(1));
        assert_eq!(p.lane(0), F3::new(0));
    }

    #[test]
    fn bipedal3_alt_zero_canonical_eq() {
        // Bit pattern (mag=0, sgn=1) is the alternative-zero codeword;
        // it must compare equal to canonical zero (mag=0, sgn=0).
        let canonical = Bipedal3 { mag: 0, sgn: 0 };
        let alt = Bipedal3 { mag: 0, sgn: !0 };
        assert_eq!(canonical, alt);
        assert!(alt.all_zero());
        for i in 0..<Bipedal3 as PackedField<F3>>::LANES {
            assert_eq!(alt.lane(i), F3::new(0));
        }
    }

    #[test]
    fn bipedal3_lane_arithmetic_matches_fp3() {
        // Spot-check: build two Bipedal3 values from F_3 canonical lanes,
        // run add/sub/mul through the trait, decode each lane, compare
        // against direct Fp<3> arithmetic.
        let mut a = <Bipedal3 as PackedField<F3>>::zero();
        let mut b = <Bipedal3 as PackedField<F3>>::zero();
        let lanes = [0u64, 1, 2, 1, 2, 0, 2, 1];
        let rhs = [1u64, 2, 0, 1, 2, 2, 1, 0];
        for (i, (&la, &lb)) in lanes.iter().zip(rhs.iter()).enumerate() {
            a = a.with_lane(i, F3::new(la));
            b = b.with_lane(i, F3::new(lb));
        }

        let s = a.add(b);
        let d = a.sub(b);
        let p = a.mul(b);

        for (i, (&la, &lb)) in lanes.iter().zip(rhs.iter()).enumerate() {
            let (fa, fb) = (F3::new(la), F3::new(lb));
            assert_eq!(s.lane(i), fa + fb, "add lane {i}");
            assert_eq!(d.lane(i), fa - fb, "sub lane {i}");
            assert_eq!(p.lane(i), fa * fb, "mul lane {i}");
        }
    }

    #[test]
    fn bipedal3_vec_round_trip() {
        let xs: Vec<F3> = (0u64..200).map(|i| F3::new(i % 3)).collect();
        let v = Bipedal3Vec::from_field_slice(&xs);
        assert_eq!(v.len(), xs.len());
        for (i, x) in xs.iter().enumerate() {
            assert_eq!(v.get(i), *x, "lane {i}");
        }
    }

    #[test]
    fn bipedal3_vec_arithmetic() {
        let len = 130; // crosses the 64- and 128-bit word boundaries
        let xs: Vec<F3> = (0..len).map(|i| F3::new((i as u64) % 3)).collect();
        let ys: Vec<F3> = (0..len).map(|i| F3::new(((i as u64) + 1) % 3)).collect();
        let mut a = Bipedal3Vec::from_field_slice(&xs);
        let b = Bipedal3Vec::from_field_slice(&ys);

        a.add_assign(&b);
        for i in 0..len {
            assert_eq!(a.get(i), xs[i] + ys[i], "add at {i}");
        }

        // Reset and try sub.
        let mut a = Bipedal3Vec::from_field_slice(&xs);
        a.sub_assign(&b);
        for i in 0..len {
            assert_eq!(a.get(i), xs[i] - ys[i], "sub at {i}");
        }

        // Reset and try mul.
        let mut a = Bipedal3Vec::from_field_slice(&xs);
        a.mul_assign(&b);
        for i in 0..len {
            assert_eq!(a.get(i), xs[i] * ys[i], "mul at {i}");
        }
    }

    #[test]
    fn permanent_stub_returns_zero() {
        let m = Bipedal3Matrix::zeros(4);
        assert_eq!(m.n(), 4);
        assert_eq!(m.permanent(), F3::new(0));
    }

    // -----------------------------------------------------------------
    // Word-boundary edge-case tests for `Bipedal3Vec`.
    //
    // CLAUDE.md §Testing requires coverage at lengths {0, 1, 63, 64, 65,
    // 127, 128, 129} to exercise:
    //   - the `n_words` / `tail_mask` boundary between exact-word and
    //     partial-word layouts (`bipedal3.rs:n_words`, `:tail_mask`),
    //   - tail-mask invariant after every mutating op (per CLAUDE.md §3
    //     "Key design invariants"),
    //   - `lane`/`with_lane`/`all_zero` agreement with `Fp<3>` reference
    //     across the full logical length.
    // -----------------------------------------------------------------

    fn build_pair(len: usize) -> (Vec<F3>, Vec<F3>, Bipedal3Vec, Bipedal3Vec) {
        let xs: Vec<F3> = (0..len).map(|i| F3::new((i as u64) % 3)).collect();
        let ys: Vec<F3> = (0..len)
            .map(|i| F3::new(((i as u64) * 2 + 1) % 3))
            .collect();
        let a = Bipedal3Vec::from_field_slice(&xs);
        let b = Bipedal3Vec::from_field_slice(&ys);
        (xs, ys, a, b)
    }

    fn check_boundary_at(len: usize) {
        let (xs, ys, a0, b) = build_pair(len);

        // Sanity: round-trip via from_field_slice + get.
        assert_eq!(a0.len(), len, "len at boundary {len}");
        assert_eq!(b.len(), len, "len at boundary {len}");
        for (i, x) in xs.iter().enumerate() {
            assert_eq!(a0.get(i), *x, "round-trip mismatch at len {len} idx {i}");
        }

        // add_assign — tail-mask invariant: padding bits beyond `len`
        // must remain zero, otherwise `add_assign(&self_zero)` from a
        // freshly constructed zero vec would diverge from the reference.
        let mut a = a0.clone();
        a.add_assign(&b);
        for i in 0..len {
            assert_eq!(a.get(i), xs[i] + ys[i], "add at len {len} idx {i}");
        }

        // sub_assign.
        let mut a = a0.clone();
        a.sub_assign(&b);
        for i in 0..len {
            assert_eq!(a.get(i), xs[i] - ys[i], "sub at len {len} idx {i}");
        }

        // mul_assign.
        let mut a = a0.clone();
        a.mul_assign(&b);
        for i in 0..len {
            assert_eq!(a.get(i), xs[i] * ys[i], "mul at len {len} idx {i}");
        }

        // all_zero: subtracting from self must yield all zeros over the
        // full logical length. If the tail mask had leaked spurious bits,
        // mag-tail-bits would survive and `all_zero()` would be wrong.
        let mut a = a0.clone();
        a.sub_assign(&a0);
        assert!(a.all_zero(), "self - self != 0 at len {len}");

        // fold_mul: stub returns F3::ZERO unconditionally; signature must
        // be exercised at every length per the trait surface contract.
        assert_eq!(a0.fold_mul(), F3::new(0), "fold_mul at len {len}");

        // is_empty.
        assert_eq!(a0.is_empty(), len == 0, "is_empty at len {len}");
    }

    fn check_lane_boundary_packed() {
        // Bipedal3 (single-word) boundary lanes 0, 1, 63 — there is no
        // 64-lane index because LANES = 64 and `lane(64)` panics. The
        // multi-word boundaries (64, 65, 127, 128, 129) live on
        // Bipedal3Vec, exercised by `check_boundary_at`.
        let v = <Bipedal3 as PackedField<F3>>::zero();
        for i in [0usize, 1, 63] {
            let v = v.with_lane(i, F3::new(2));
            assert_eq!(v.lane(i), F3::new(2), "with_lane/lane at idx {i}");
            // Other lanes still zero; `all_zero` must remain false.
            assert!(!v.all_zero(), "all_zero false after with_lane at {i}");
        }
    }

    #[test]
    fn bipedal3_packed_field_lane_boundaries() {
        check_lane_boundary_packed();
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_0() {
        check_boundary_at(0);
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_1() {
        check_boundary_at(1);
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_63() {
        check_boundary_at(63);
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_64() {
        check_boundary_at(64);
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_65() {
        check_boundary_at(65);
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_127() {
        check_boundary_at(127);
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_128() {
        check_boundary_at(128);
    }

    #[test]
    fn bipedal3_vec_word_boundary_len_129() {
        check_boundary_at(129);
    }

    // -----------------------------------------------------------------
    // Property-based tests via `proptest` for the mathematical
    // invariants of `Bipedal3` and `Bipedal3Vec` per CLAUDE.md §Testing
    // ("Property-based tests use `proptest`").
    //
    // Strategy:
    //   - Generate `Bipedal3` values via 64 `Fp<3>` lanes built through
    //     `with_lane`, so generated values are always canonical.
    //   - Reference oracle: do the lane-by-lane operation directly on
    //     `Fp<3>` and compare against the packed result through `lane`.
    // -----------------------------------------------------------------

    use proptest::prelude::*;

    fn arb_fp3() -> impl Strategy<Value = F3> {
        (0u64..3).prop_map(F3::new)
    }

    fn arb_bipedal3() -> impl Strategy<Value = (Bipedal3, [F3; 64])> {
        prop::array::uniform32(arb_fp3()).prop_flat_map(|first32| {
            prop::array::uniform32(arb_fp3()).prop_map(move |last32| {
                let mut lanes = [F3::new(0); 64];
                lanes[..32].copy_from_slice(&first32);
                lanes[32..].copy_from_slice(&last32);
                let mut v = <Bipedal3 as PackedField<F3>>::zero();
                for (i, x) in lanes.iter().enumerate() {
                    v = v.with_lane(i, *x);
                }
                (v, lanes)
            })
        })
    }

    fn arb_bipedal3_vec(max_len: usize) -> impl Strategy<Value = (Bipedal3Vec, Vec<F3>)> {
        prop::collection::vec(arb_fp3(), 0..=max_len)
            .prop_map(|xs| (Bipedal3Vec::from_field_slice(&xs), xs))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        // add is commutative on canonical-decoded lanes.
        #[test]
        fn prop_add_commutative((a, _la) in arb_bipedal3(), (b, _lb) in arb_bipedal3()) {
            prop_assert_eq!(a.add(b), b.add(a));
        }

        // add is associative on canonical-decoded lanes.
        #[test]
        fn prop_add_associative(
            (a, _) in arb_bipedal3(),
            (b, _) in arb_bipedal3(),
            (c, _) in arb_bipedal3(),
        ) {
            prop_assert_eq!(a.add(b).add(c), a.add(b.add(c)));
        }

        // mul distributes over add: a * (b + c) == a*b + a*c (lane-wise).
        #[test]
        fn prop_mul_distributes_over_add(
            (a, _) in arb_bipedal3(),
            (b, _) in arb_bipedal3(),
            (c, _) in arb_bipedal3(),
        ) {
            let lhs = a.mul(b.add(c));
            let rhs = a.mul(b).add(a.mul(c));
            prop_assert_eq!(lhs, rhs);
        }

        // with_lane is a left-inverse of lane at the written index.
        #[test]
        fn prop_with_lane_lane_round_trip(
            (v, _) in arb_bipedal3(),
            i in 0usize..64,
            x in arb_fp3(),
        ) {
            let v2 = v.with_lane(i, x);
            prop_assert_eq!(v2.lane(i), x);
        }

        // with_lane preserves all other lanes.
        #[test]
        fn prop_with_lane_preserves_others(
            (v, lanes) in arb_bipedal3(),
            i in 0usize..64,
            x in arb_fp3(),
        ) {
            let v2 = v.with_lane(i, x);
            for (j, &lane_j) in lanes.iter().enumerate() {
                if j != i {
                    prop_assert_eq!(v2.lane(j), lane_j);
                }
            }
        }

        // add(v, neg(v)) == zero for all v.
        #[test]
        fn prop_add_neg_is_zero((v, _) in arb_bipedal3()) {
            let z = v.add(v.neg());
            prop_assert!(z.all_zero());
            prop_assert_eq!(z, <Bipedal3 as PackedField<F3>>::zero());
        }

        // Lane-wise `add` matches `Fp<3>::add` per lane.
        #[test]
        fn prop_add_matches_fp3_per_lane(
            (a, la) in arb_bipedal3(),
            (b, lb) in arb_bipedal3(),
        ) {
            let s = a.add(b);
            for i in 0..64 {
                prop_assert_eq!(s.lane(i), la[i] + lb[i]);
            }
        }

        // Lane-wise `sub` matches `Fp<3>::sub` per lane.
        #[test]
        fn prop_sub_matches_fp3_per_lane(
            (a, la) in arb_bipedal3(),
            (b, lb) in arb_bipedal3(),
        ) {
            let s = a.sub(b);
            for i in 0..64 {
                prop_assert_eq!(s.lane(i), la[i] - lb[i]);
            }
        }

        // Lane-wise `mul` matches `Fp<3>::mul` per lane.
        #[test]
        fn prop_mul_matches_fp3_per_lane(
            (a, la) in arb_bipedal3(),
            (b, lb) in arb_bipedal3(),
        ) {
            let s = a.mul(b);
            for i in 0..64 {
                prop_assert_eq!(s.lane(i), la[i] * lb[i]);
            }
        }

        // Vec add_assign matches Fp<3> reference across all lanes (covers
        // arbitrary lengths up to 200, including word-boundary regions).
        #[test]
        fn prop_vec_add_matches_fp3(
            (mut a, xs) in arb_bipedal3_vec(200),
            shift in 0u64..3,
        ) {
            let ys: Vec<F3> = xs.iter().map(|_| F3::new(shift)).collect();
            let b = Bipedal3Vec::from_field_slice(&ys);
            a.add_assign(&b);
            for (i, x) in xs.iter().enumerate() {
                prop_assert_eq!(a.get(i), *x + F3::new(shift));
            }
        }

        // Vec self - self == zero (tail-mask invariant + correctness).
        #[test]
        fn prop_vec_self_minus_self_zero((v, _) in arb_bipedal3_vec(200)) {
            let mut a = v.clone();
            a.sub_assign(&v);
            prop_assert!(a.all_zero());
        }

        // Bipedal3Vec::fold_mul stub always returns F3::ZERO regardless
        // of length or content. Pinned at the trait-surface stub layer;
        // the real implementation in W2/W3 will replace this property.
        #[test]
        fn prop_vec_fold_mul_stub_zero((v, _) in arb_bipedal3_vec(200)) {
            prop_assert_eq!(v.fold_mul(), F3::new(0));
        }
    }
}
