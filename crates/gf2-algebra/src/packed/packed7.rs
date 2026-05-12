//! Packed `F_7` element / vector encoding — Candidate A (R2 decision).
//!
//! # Encoding (R2 §1 / §5)
//!
//! Each `u64` packs **16 elements** at 4-bit-aligned slots. Slot `i`
//! occupies bits `[4i .. 4i+4)`. Canonical values are `0..=6`; the high
//! bit of each slot (bit `4i+3`) is reserved and always zero for canonical
//! packings (since `6 = 0b0110 < 8`, the high bit is never set).
//!
//! # Binary operations (R2 §5)
//!
//! Binary ops use a 64 KiB lookup table keyed by a packed 16-bit
//! `(a_byte | (b_byte << 8))` index, where each input byte holds two
//! adjacent 4-bit-slot elements. Each lookup yields a 1-byte result
//! containing two packed 4-bit results.
//!
//! Per `u64` (16 F_7 ops): 8 LUT lookups + ~16 shift/mask ops.
//!
//! # LUTs
//!
//! Three 64 KiB static const arrays:
//! - `ADD_LUT`: `(a + b) mod 7` per nibble pair.
//! - `SUB_LUT`: `(a - b + 7) mod 7` per nibble pair.
//! - `MUL_LUT`: `(a * b) mod 7` per nibble pair.
//!
//! Computed at compile time via `const fn`; no `OnceLock`, Kani-friendly.
//! Total footprint in `.rodata`: 3 × 64 KiB = 192 KiB.
//!
//! Non-canonical nibble inputs (nibble value ≥ 7) produce a LUT result of 0
//! (safe but undefined; canonical packings never produce them).
//!
//! # Status
//!
//! W4 — F_7 packed type + ops per R2 Candidate A decision
//! (`dev/plans/r2_f7_encoding_decision.md`). Transliterated from
//! `dev/research/f7_packing/src/cand_a.rs`.

use core::fmt;

use gf2_core::gfp::Fp;

use super::{PackedField, PackedFieldVec};

// ---------------------------------------------------------------------------
// Compile-time LUT construction
// ---------------------------------------------------------------------------

/// Build the `ADD_LUT` at compile time.
///
/// `ADD_LUT[a_byte as usize | ((b_byte as usize) << 8)]` returns a byte
/// whose low nibble is `(a_lo + b_lo) % 7` and high nibble is
/// `(a_hi + b_hi) % 7`, where `a_lo = a_byte & 0xf`, `a_hi = (a_byte >> 4) & 0xf`,
/// etc. Non-canonical nibbles (≥ 7) produce 0.
const fn build_add_lut() -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    let mut ap: usize = 0;
    while ap < 256 {
        let a0 = (ap & 0xf) as u8;
        let a1 = (ap >> 4) as u8;
        let mut bp: usize = 0;
        while bp < 256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                let r0 = (a0 + b0) % 7;
                let r1 = (a1 + b1) % 7;
                let key = (bp << 8) | ap;
                lut[key] = r0 | (r1 << 4);
            }
            bp += 1;
        }
        ap += 1;
    }
    lut
}

/// Build the `SUB_LUT` at compile time.
///
/// `SUB_LUT[a_byte as usize | ((b_byte as usize) << 8)]` returns a byte
/// whose low nibble is `(a_lo - b_lo + 7) % 7` and high nibble is
/// `(a_hi - b_hi + 7) % 7`. Non-canonical nibbles (≥ 7) produce 0.
const fn build_sub_lut() -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    let mut ap: usize = 0;
    while ap < 256 {
        let a0 = (ap & 0xf) as u8;
        let a1 = (ap >> 4) as u8;
        let mut bp: usize = 0;
        while bp < 256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                let r0 = (a0 + 7 - b0) % 7;
                let r1 = (a1 + 7 - b1) % 7;
                let key = (bp << 8) | ap;
                lut[key] = r0 | (r1 << 4);
            }
            bp += 1;
        }
        ap += 1;
    }
    lut
}

/// Build the `MUL_LUT` at compile time.
///
/// `MUL_LUT[a_byte as usize | ((b_byte as usize) << 8)]` returns a byte
/// whose low nibble is `(a_lo * b_lo) % 7` and high nibble is
/// `(a_hi * b_hi) % 7`. Non-canonical nibbles (≥ 7) produce 0.
const fn build_mul_lut() -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    let mut ap: usize = 0;
    while ap < 256 {
        let a0 = (ap & 0xf) as u8;
        let a1 = (ap >> 4) as u8;
        let mut bp: usize = 0;
        while bp < 256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                let r0 = (a0 * b0) % 7;
                let r1 = (a1 * b1) % 7;
                let key = (bp << 8) | ap;
                lut[key] = r0 | (r1 << 4);
            }
            bp += 1;
        }
        ap += 1;
    }
    lut
}

/// Addition LUT: 64 KiB, built at compile time, resident in `.rodata`.
///
/// `ADD_LUT[key]` where `key = (a_byte as usize) | ((b_byte as usize) << 8)`.
/// Low nibble of result = `(a_lo + b_lo) % 7`; high nibble = `(a_hi + b_hi) % 7`.
pub static ADD_LUT: [u8; 65536] = build_add_lut();

/// Subtraction LUT: 64 KiB, built at compile time, resident in `.rodata`.
///
/// `SUB_LUT[key]` where `key = (a_byte as usize) | ((b_byte as usize) << 8)`.
/// Low nibble of result = `(a_lo - b_lo + 7) % 7`; high nibble = `(a_hi - b_hi + 7) % 7`.
pub static SUB_LUT: [u8; 65536] = build_sub_lut();

/// Multiplication LUT: 64 KiB, built at compile time, resident in `.rodata`.
///
/// `MUL_LUT[key]` where `key = (a_byte as usize) | ((b_byte as usize) << 8)`.
/// Low nibble of result = `(a_lo * b_lo) % 7`; high nibble = `(a_hi * b_hi) % 7`.
pub static MUL_LUT: [u8; 65536] = build_mul_lut();

// ---------------------------------------------------------------------------
// Core word-level operation
// ---------------------------------------------------------------------------

/// Apply a binary LUT op to a single pair of packed-F_7 `u64` words.
///
/// # Arguments
///
/// * `a` — first packed word (16 F_7 elements at 4-bit slots 0..=15).
/// * `b` — second packed word.
/// * `lut` — one of `ADD_LUT`, `SUB_LUT`, or `MUL_LUT`.
///
/// # Complexity
///
/// 8 LUT lookups + 8 shift/mask/OR assembler ops.
#[inline]
fn binary_op_word(a: u64, b: u64, lut: &[u8; 65536]) -> u64 {
    let mut r: u64 = 0;
    let mut i = 0;
    while i < 8 {
        let ap = ((a >> (8 * i)) & 0xff) as usize;
        let bp = ((b >> (8 * i)) & 0xff) as usize;
        let key = ap | (bp << 8);
        r |= (lut[key] as u64) << (8 * i);
        i += 1;
    }
    r
}

// ---------------------------------------------------------------------------
// Packed7 — fixed-width 16-lane packed F_7
// ---------------------------------------------------------------------------

/// Fixed-width packed `F_7` element encoding 16 lanes in one `u64`.
///
/// Each `F_7` element occupies a 4-bit-aligned slot: slot `i` (0 ≤ i < 16)
/// lives in bits `[4i, 4i+4)`. The high bit of each slot (bit `4i+3`) is
/// reserved and always zero for canonical values (since 6 = 0b0110 < 8).
///
/// Binary ops use 8 LUT lookups per `u64` word, giving 16 element results per
/// lookup round. The LUTs are 64 KiB each and are built at compile time —
/// no runtime initialisation, no `OnceLock`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedField, Packed7};
/// use gf2_core::gfp::Fp;
///
/// let a = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(3));
/// let b = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(5));
/// let s = a.add(b);
/// assert_eq!(s.lane(0), Fp::<7>::new(1)); // (3 + 5) % 7 = 1
/// ```
///
/// # Complexity
///
/// All fixed-width operations are `O(1)` — a fixed number of LUT lookups
/// and word-level ops independent of the number of lanes.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Default)]
pub struct Packed7 {
    w: u64,
}

/// Number of F_7 lanes packed into one [`Packed7`].
pub const LANES: usize = 16;

impl Packed7 {
    /// Construct a `Packed7` from an array of 16 `F_7` elements.
    ///
    /// # Arguments
    ///
    /// * `values` — exactly 16 canonical `F_7` values (each in `0..=6`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// let arr = [Fp::<7>::new(0); 16];
    /// let p = Packed7::pack(&arr);
    /// assert_eq!(p.lane(0), Fp::<7>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(LANES)`.
    #[inline]
    pub fn pack(values: &[Fp<7>; 16]) -> Self {
        let mut w = 0u64;
        let mut i = 0;
        while i < 16 {
            w |= values[i].value() << (4 * i);
            i += 1;
        }
        Self { w }
    }

    /// Decode lane `i` to a canonical `F_7` value.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..LANES`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= LANES`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = Packed7::pack(&[Fp::<7>::new(5); 16]);
    /// assert_eq!(p.lane(0), Fp::<7>::new(5));
    /// assert_eq!(p.lane(15), Fp::<7>::new(5));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn lane(self, i: usize) -> Fp<7> {
        assert!(
            i < LANES,
            "Packed7::lane: index {i} out of range (LANES = {LANES})"
        );
        let nibble = (self.w >> (4 * i)) & 0xf;
        Fp::<7>::new(nibble)
    }

    /// Decode all 16 lanes into an array.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = Packed7::pack(&[Fp::<7>::new(3); 16]);
    /// let arr = p.to_array();
    /// assert!(arr.iter().all(|&x| x == Fp::<7>::new(3)));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(LANES)`.
    #[inline]
    pub fn to_array(self) -> [Fp<7>; 16] {
        core::array::from_fn(|i| self.lane(i))
    }

    /// All-lanes-zero constant.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    ///
    /// let z = Packed7::zero();
    /// assert!(z.all_zero());
    /// ```
    #[inline]
    pub fn zero() -> Self {
        Self { w: 0 }
    }

    /// All-lanes-one constant.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// let o = Packed7::one();
    /// assert_eq!(o.lane(0), Fp::<7>::new(1));
    /// assert_eq!(o.lane(15), Fp::<7>::new(1));
    /// ```
    #[inline]
    pub fn one() -> Self {
        // Every 4-bit slot = 1: set bit 0 of each slot.
        // Slots at positions 4i have bit 0 = 1 when the packed nibble = 1.
        // Pattern: 0x1111_1111_1111_1111 (each nibble = 1).
        Self {
            w: 0x1111_1111_1111_1111u64,
        }
    }

    /// Broadcast scalar `x` to all 16 lanes.
    ///
    /// # Arguments
    ///
    /// * `x` — `F_7` scalar to replicate across all lanes.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Packed7::splat(Fp::<7>::new(5));
    /// for i in 0..16 { assert_eq!(v.lane(i), Fp::<7>::new(5)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn splat(x: Fp<7>) -> Self {
        let v = x.value(); // 0..=6
                           // Replicate a single nibble v to all 16 slots.
                           // Each nibble slot i is at bits [4i, 4i+4); we multiply v by 0x1111...
                           // to broadcast it into every nibble position.
        let w = v.wrapping_mul(0x1111_1111_1111_1111u64);
        Self { w }
    }

    /// Write the canonical encoding of `x` into lane `i`, returning the updated value.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..LANES`.
    /// * `x` — scalar to write into lane `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= LANES`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Packed7::zero();
    /// let v = v.with_lane(3, Fp::<7>::new(6));
    /// assert_eq!(v.lane(3), Fp::<7>::new(6));
    /// assert_eq!(v.lane(0), Fp::<7>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn with_lane(self, i: usize, x: Fp<7>) -> Self {
        assert!(
            i < LANES,
            "Packed7::with_lane: index {i} out of range (LANES = {LANES})"
        );
        let mask = 0xfu64 << (4 * i);
        let val = x.value() << (4 * i);
        Self {
            w: (self.w & !mask) | (val & mask),
        }
    }

    /// Returns `true` iff every lane decodes to `F_7`'s additive identity (0).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// assert!(Packed7::zero().all_zero());
    /// let nz = Packed7::splat(Fp::<7>::new(1));
    /// assert!(!nz.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn all_zero(self) -> bool {
        self.w == 0
    }
}

// ---------------------------------------------------------------------------
// PackedField<Fp<7>> for Packed7
// ---------------------------------------------------------------------------

impl PackedField<Fp<7>> for Packed7 {
    /// Number of F_7 lanes packed into one [`Packed7`].
    ///
    /// Fixed at 16 to match the 4-bit-slot encoding width in a `u64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    /// assert_eq!(<Packed7 as PackedField<Fp<7>>>::LANES, 16);
    /// ```
    const LANES: usize = LANES;

    /// Returns the all-zeros `Packed7` (every lane = 0).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let z = <Packed7 as PackedField<Fp<7>>>::zero();
    /// assert!(z.all_zero());
    /// for i in 0..16 { assert_eq!(z.lane(i), Fp::<7>::new(0)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn zero() -> Self {
        Packed7::zero()
    }

    /// Returns the all-ones `Packed7` (every lane = 1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let o = <Packed7 as PackedField<Fp<7>>>::one();
    /// for i in 0..16 { assert_eq!(o.lane(i), Fp::<7>::new(1)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn one() -> Self {
        Packed7::one()
    }

    /// Broadcasts scalar `x` to all 16 lanes.
    ///
    /// # Arguments
    ///
    /// * `x` — `F_7` scalar to replicate across all lanes.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(3));
    /// for i in 0..16 { assert_eq!(v.lane(i), Fp::<7>::new(3)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn splat(x: Fp<7>) -> Self {
        Packed7::splat(x)
    }

    /// Lane-wise sum: `self + rhs` pointwise mod 7.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(5));
    /// let b = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(3));
    /// assert_eq!(a.add(b).lane(0), Fp::<7>::new(1)); // (5 + 3) % 7 = 1
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 8 LUT lookups + shift/mask ops.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            w: binary_op_word(self.w, rhs.w, &ADD_LUT),
        }
    }

    /// Lane-wise difference: `self - rhs` pointwise mod 7.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(2));
    /// let b = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(5));
    /// assert_eq!(a.sub(b).lane(0), Fp::<7>::new(4)); // (2 - 5 + 7) % 7 = 4
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 8 LUT lookups + shift/mask ops.
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            w: binary_op_word(self.w, rhs.w, &SUB_LUT),
        }
    }

    /// Lane-wise additive inverse: `-(self)` pointwise mod 7.
    ///
    /// Implemented as `0 - self` via `SUB_LUT`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(1));
    /// assert_eq!(a.neg().lane(0), Fp::<7>::new(6)); // -1 ≡ 6 mod 7
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 8 LUT lookups.
    #[inline]
    fn neg(self) -> Self {
        Self {
            w: binary_op_word(0u64, self.w, &SUB_LUT),
        }
    }

    /// Lane-wise product: `self * rhs` pointwise mod 7.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(3));
    /// let b = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(3));
    /// assert_eq!(a.mul(b).lane(0), Fp::<7>::new(2)); // (3 * 3) % 7 = 2
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 8 LUT lookups + shift/mask ops.
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            w: binary_op_word(self.w, rhs.w, &MUL_LUT),
        }
    }

    /// Decode lane `i` to a canonical `F_7` value.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..16`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 16`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(4));
    /// assert_eq!(v.lane(0), Fp::<7>::new(4));
    /// assert_eq!(v.lane(15), Fp::<7>::new(4));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn lane(self, i: usize) -> Fp<7> {
        Packed7::lane(self, i)
    }

    /// Write the canonical encoding of `x` into lane `i`.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..16`.
    /// * `x` — scalar to write.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 16`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Packed7 as PackedField<Fp<7>>>::zero();
    /// let v = v.with_lane(7, Fp::<7>::new(5));
    /// assert_eq!(v.lane(7), Fp::<7>::new(5));
    /// assert_eq!(v.lane(0), Fp::<7>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn with_lane(self, i: usize, x: Fp<7>) -> Self {
        Packed7::with_lane(self, i, x)
    }

    /// Returns `true` iff every lane decodes to `F_7`'s additive identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed7};
    /// use gf2_core::gfp::Fp;
    ///
    /// let z = <Packed7 as PackedField<Fp<7>>>::zero();
    /// assert!(z.all_zero());
    /// let o = <Packed7 as PackedField<Fp<7>>>::one();
    /// assert!(!o.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn all_zero(self) -> bool {
        Packed7::all_zero(self)
    }
}

// ---------------------------------------------------------------------------
// Packed7Vec — variable-length packed F_7 vector
// ---------------------------------------------------------------------------

/// Variable-length packed `F_7` vector storing `len_lanes` elements as a
/// `Vec<u64>` of words, each word holding 16 elements at 4-bit-aligned slots.
///
/// # Mask-tail invariant
///
/// Padding nibbles beyond `len_lanes` in the last word must always be zero.
/// Every mutating operation calls [`Packed7Vec::mask_tail`] to enforce this
/// invariant. This is the most critical correctness invariant in this
/// codebase (CLAUDE.md §Key design invariants #1).
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
/// use gf2_core::gfp::Fp;
///
/// let v = Packed7Vec::zeros(5);
/// assert_eq!(v.len(), 5);
/// assert!(v.all_zero());
/// ```
///
/// # Complexity
///
/// Construction and lane-wise operations are `O(ceil(len_lanes / 16))`.
/// Individual lane access ([`get`][`Packed7Vec::get`]) is `O(1)`.
#[derive(Clone)]
pub struct Packed7Vec {
    words: Vec<u64>,
    len_lanes: usize,
}

impl Packed7Vec {
    /// Number of `u64` words needed to store `len` lanes.
    #[inline]
    fn n_words(len: usize) -> usize {
        len.div_ceil(16)
    }

    /// Zero out all nibbles beyond `self.len_lanes` in the last word.
    ///
    /// **This invariant must hold after every mutation.** Failing to call
    /// `mask_tail` after any write violates the key correctness invariant.
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn mask_tail(&mut self) {
        let n = self.words.len();
        if n == 0 {
            return;
        }
        let used = self.len_lanes - 16 * (n - 1); // lanes in last word
        if used == 16 {
            return; // full word; no padding to mask
        }
        // Each slot is 4 bits; `used` slots means the mask spans 4*used bits.
        let mask = (1u64 << (4 * used)) - 1;
        self.words[n - 1] &= mask;
    }

    /// Decode logical position `i` to a canonical `F_7` value.
    ///
    /// # Arguments
    ///
    /// * `i` — logical position in `0..self.len()`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Packed7Vec::from_field_slice(&[Fp::<7>::new(4), Fp::<7>::new(2)]);
    /// assert_eq!(v.get(0), Fp::<7>::new(4));
    /// assert_eq!(v.get(1), Fp::<7>::new(2));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn get(&self, i: usize) -> Fp<7> {
        assert!(
            i < self.len_lanes,
            "Packed7Vec::get: index {i} out of range (len = {})",
            self.len_lanes
        );
        let w = i / 16;
        let s = i % 16;
        let nibble = (self.words[w] >> (4 * s)) & 0xf;
        Fp::<7>::new(nibble)
    }

    /// Lane-wise in-place additive inverse: `self[i] = -self[i]` for every `i`.
    ///
    /// This is an inherent method (not on `PackedFieldVec`) because the frozen
    /// trait surface (D1b §2.2) does not include `neg_assign`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut v = Packed7Vec::from_field_slice(&[
    ///     Fp::<7>::new(0), Fp::<7>::new(1), Fp::<7>::new(3),
    /// ]);
    /// v.neg_assign();
    /// assert_eq!(v.get(0), Fp::<7>::new(0));  // -0 = 0
    /// assert_eq!(v.get(1), Fp::<7>::new(6));  // -1 ≡ 6 mod 7
    /// assert_eq!(v.get(2), Fp::<7>::new(4));  // -3 ≡ 4 mod 7
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 16))` word-level ops.
    pub fn neg_assign(&mut self) {
        for w in self.words.iter_mut() {
            *w = binary_op_word(0u64, *w, &SUB_LUT);
        }
        self.mask_tail();
    }

    /// Borrow the raw packed word slice.
    ///
    /// The last word may be partially filled; nibble slots at positions
    /// `self.len() % 16 .. 15` within the last word are zero (tail-masking
    /// invariant).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Packed7Vec::from_field_slice(&[Fp::<7>::new(3), Fp::<7>::new(5)]);
    /// // Lane 0 = 3 in slot 0; lane 1 = 5 in slot 1.
    /// assert_eq!(v.raw_words()[0] & 0xff, (5 << 4) | 3);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn raw_words(&self) -> &[u64] {
        &self.words
    }
}

// ---------------------------------------------------------------------------
// Manual PartialEq / Eq — canonical-decode equality
// ---------------------------------------------------------------------------

impl PartialEq for Packed7Vec {
    /// Canonical-decode equality: two vectors are equal iff they have the
    /// same `len_lanes` and every decoded lane is equal.
    ///
    /// The mask-tail invariant ensures padding nibbles are 0 on both sides,
    /// so a direct word-by-word comparison is correct.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = Packed7Vec::from_field_slice(&[Fp::<7>::new(1), Fp::<7>::new(3)]);
    /// let b = Packed7Vec::from_field_slice(&[Fp::<7>::new(1), Fp::<7>::new(3)]);
    /// assert_eq!(a, b);
    ///
    /// let c = Packed7Vec::from_field_slice(&[Fp::<7>::new(0)]);
    /// assert_ne!(a, c); // different len_lanes
    /// ```
    fn eq(&self, other: &Self) -> bool {
        self.len_lanes == other.len_lanes && self.words == other.words
    }
}

impl Eq for Packed7Vec {}

// ---------------------------------------------------------------------------
// Manual Debug
// ---------------------------------------------------------------------------

impl fmt::Debug for Packed7Vec {
    /// Formats the value as a `Vec` of decoded lane values (each 0..=6).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Packed7Vec::from_field_slice(&[Fp::<7>::new(3), Fp::<7>::new(6)]);
    /// let s = format!("{:?}", v);
    /// assert!(s.contains("lanes"));
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lanes: Vec<u64> = (0..self.len_lanes)
            .map(|i| {
                let w = i / 16;
                let s = i % 16;
                (self.words[w] >> (4 * s)) & 0xf
            })
            .collect();
        f.debug_struct("Packed7Vec").field("lanes", &lanes).finish()
    }
}

// ---------------------------------------------------------------------------
// PackedFieldVec<Fp<7>> for Packed7Vec
// ---------------------------------------------------------------------------

impl PackedFieldVec<Fp<7>> for Packed7Vec {
    type Element = Packed7;

    /// Construct a vector of `len` zero `F_7` elements.
    ///
    /// # Arguments
    ///
    /// * `len` — number of logical `F_7` positions in the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    ///
    /// let v = Packed7Vec::zeros(17);
    /// assert_eq!(v.len(), 17);
    /// assert!(v.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(len / 16))`.
    fn zeros(len: usize) -> Self {
        let n_words = Self::n_words(len);
        Self {
            words: vec![0u64; n_words],
            len_lanes: len,
        }
    }

    /// Construct a vector by encoding every element of `xs`.
    ///
    /// # Arguments
    ///
    /// * `xs` — source slice; the result has `xs.len()` logical positions
    ///   and `get(i) == xs[i]` for every `i`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let xs = [Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(6)];
    /// let v = Packed7Vec::from_field_slice(&xs);
    /// for i in 0..3 {
    ///     assert_eq!(v.get(i), xs[i]);
    /// }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(xs.len())`.
    fn from_field_slice(xs: &[Fp<7>]) -> Self {
        let len = xs.len();
        let n_words = Self::n_words(len);
        let mut words = vec![0u64; n_words];
        for (i, &x) in xs.iter().enumerate() {
            let w = i / 16;
            let s = i % 16;
            words[w] |= x.value() << (4 * s);
        }
        let mut out = Self {
            words,
            len_lanes: len,
        };
        out.mask_tail();
        out
    }

    /// Number of logical `F_7` positions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    ///
    /// let v = Packed7Vec::zeros(7);
    /// assert_eq!(v.len(), 7);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn len(&self) -> usize {
        self.len_lanes
    }

    /// Decode logical position `i` to a canonical `F_7` value.
    ///
    /// # Arguments
    ///
    /// * `i` — logical position index in `0..self.len()`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let xs = [Fp::<7>::new(5)];
    /// let v = Packed7Vec::from_field_slice(&xs);
    /// assert_eq!(v.get(0), Fp::<7>::new(5));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn get(&self, i: usize) -> Fp<7> {
        Packed7Vec::get(self, i)
    }

    /// Lane-wise in-place sum: `self[i] += rhs[i]` for every `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Packed7Vec::from_field_slice(&[Fp::<7>::new(5), Fp::<7>::new(6)]);
    /// let b = Packed7Vec::from_field_slice(&[Fp::<7>::new(3), Fp::<7>::new(2)]);
    /// a.add_assign(&b);
    /// assert_eq!(a.get(0), Fp::<7>::new(1)); // (5 + 3) % 7 = 1
    /// assert_eq!(a.get(1), Fp::<7>::new(1)); // (6 + 2) % 7 = 1
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 16))`.
    fn add_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Packed7Vec::add_assign: length mismatch ({} != {})",
            self.len_lanes, rhs.len_lanes
        );
        for (wa, wb) in self.words.iter_mut().zip(rhs.words.iter()) {
            *wa = binary_op_word(*wa, *wb, &ADD_LUT);
        }
        self.mask_tail();
    }

    /// Lane-wise in-place difference: `self[i] -= rhs[i]` for every `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Packed7Vec::from_field_slice(&[Fp::<7>::new(2), Fp::<7>::new(0)]);
    /// let b = Packed7Vec::from_field_slice(&[Fp::<7>::new(5), Fp::<7>::new(1)]);
    /// a.sub_assign(&b);
    /// assert_eq!(a.get(0), Fp::<7>::new(4)); // (2 - 5 + 7) % 7 = 4
    /// assert_eq!(a.get(1), Fp::<7>::new(6)); // (0 - 1 + 7) % 7 = 6
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 16))`.
    fn sub_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Packed7Vec::sub_assign: length mismatch ({} != {})",
            self.len_lanes, rhs.len_lanes
        );
        for (wa, wb) in self.words.iter_mut().zip(rhs.words.iter()) {
            *wa = binary_op_word(*wa, *wb, &SUB_LUT);
        }
        self.mask_tail();
    }

    /// Lane-wise in-place product: `self[i] *= rhs[i]` for every `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Packed7Vec::from_field_slice(&[Fp::<7>::new(3), Fp::<7>::new(5)]);
    /// let b = Packed7Vec::from_field_slice(&[Fp::<7>::new(4), Fp::<7>::new(2)]);
    /// a.mul_assign(&b);
    /// assert_eq!(a.get(0), Fp::<7>::new(5)); // (3 * 4) % 7 = 5
    /// assert_eq!(a.get(1), Fp::<7>::new(3)); // (5 * 2) % 7 = 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 16))`.
    fn mul_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Packed7Vec::mul_assign: length mismatch ({} != {})",
            self.len_lanes, rhs.len_lanes
        );
        for (wa, wb) in self.words.iter_mut().zip(rhs.words.iter()) {
            *wa = binary_op_word(*wa, *wb, &MUL_LUT);
        }
        self.mask_tail();
    }

    /// Returns `true` iff every logical position decodes to `F_7`'s additive
    /// identity. The empty vector trivially answers `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// assert!(Packed7Vec::zeros(5).all_zero());
    /// let nz = Packed7Vec::from_field_slice(&[Fp::<7>::new(1)]);
    /// assert!(!nz.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 16))`.
    fn all_zero(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn fp7_strat() -> impl Strategy<Value = Fp<7>> {
        (0u64..7).prop_map(Fp::<7>::new)
    }

    fn packed7_strat() -> impl Strategy<Value = Packed7> {
        prop::collection::vec(fp7_strat(), 16).prop_map(|v| {
            let arr: [Fp<7>; 16] = core::array::from_fn(|i| v[i]);
            Packed7::pack(&arr)
        })
    }

    // Scalar reference ops
    fn scalar_add(a: u64, b: u64) -> u64 {
        (a + b) % 7
    }
    fn scalar_sub(a: u64, b: u64) -> u64 {
        (a + 7 - b) % 7
    }
    fn scalar_mul(a: u64, b: u64) -> u64 {
        (a * b) % 7
    }
    fn scalar_neg(a: u64) -> u64 {
        (7 - a) % 7
    }

    // -----------------------------------------------------------------------
    // LUT spot-check tests
    // -----------------------------------------------------------------------

    /// Verify a few hand-computed ADD_LUT entries.
    ///
    /// Key layout: `(a_byte as usize) | ((b_byte as usize) << 8)`.
    /// Byte layout: low nibble = element at even slot, high nibble = element at odd slot.
    ///
    /// Example: a_byte = 0x35 → a_lo=5, a_hi=3; b_byte = 0x24 → b_lo=4, b_hi=2.
    /// Result byte: low nibble = (5+4)%7=2, high nibble = (3+2)%7=5 → 0x52.
    #[test]
    fn test_add_lut_spot_check() {
        // (a_lo=3, a_hi=5) + (b_lo=4, b_hi=2) → (lo=(3+4)%7=0, hi=(5+2)%7=0) → 0x00
        let a_byte: usize = (5 << 4) | 3; // a_hi=5, a_lo=3
        let b_byte: usize = (2 << 4) | 4; // b_hi=2, b_lo=4
        let key = a_byte | (b_byte << 8);
        let result = ADD_LUT[key];
        assert_eq!(result & 0xf, scalar_add(3, 4) as u8, "add low nibble");
        assert_eq!(
            (result >> 4) & 0xf,
            scalar_add(5, 2) as u8,
            "add high nibble"
        );

        // (a_lo=6, a_hi=6) + (b_lo=1, b_hi=1) → (0+0)=(0,0) wait: (6+1)%7=0
        let a2: usize = (6 << 4) | 6;
        let b2: usize = (1 << 4) | 1;
        let key2 = a2 | (b2 << 8);
        let r2 = ADD_LUT[key2];
        assert_eq!(r2 & 0xf, scalar_add(6, 1) as u8);
        assert_eq!((r2 >> 4) & 0xf, scalar_add(6, 1) as u8);
    }

    /// Verify SUB_LUT entries.
    #[test]
    fn test_sub_lut_spot_check() {
        let a_byte: usize = (2 << 4) | 1; // a_hi=2, a_lo=1
        let b_byte: usize = (5 << 4) | 4; // b_hi=5, b_lo=4
        let key = a_byte | (b_byte << 8);
        let result = SUB_LUT[key];
        assert_eq!(result & 0xf, scalar_sub(1, 4) as u8, "sub low nibble");
        assert_eq!(
            (result >> 4) & 0xf,
            scalar_sub(2, 5) as u8,
            "sub high nibble"
        );
    }

    /// Verify MUL_LUT entries.
    #[test]
    fn test_mul_lut_spot_check() {
        let a_byte: usize = (4 << 4) | 3; // a_hi=4, a_lo=3
        let b_byte: usize = (5 << 4) | 4; // b_hi=5, b_lo=4
        let key = a_byte | (b_byte << 8);
        let result = MUL_LUT[key];
        assert_eq!(
            result & 0xf,
            scalar_mul(3, 4) as u8,
            "mul low nibble (3*4=12%7=5)"
        );
        assert_eq!(
            (result >> 4) & 0xf,
            scalar_mul(4, 5) as u8,
            "mul high nibble (4*5=20%7=6)"
        );
    }

    /// Non-canonical nibble inputs (≥ 7) produce 0 in the LUT.
    #[test]
    fn test_lut_non_canonical_yields_zero() {
        // a_lo = 7 (non-canonical): LUT result for that nibble pair must be 0
        let a_byte: usize = 7; // a_lo=7, a_hi=0
        let b_byte: usize = 0;
        let key = a_byte | (b_byte << 8);
        // The whole byte is 0 because a_lo is non-canonical (≥ 7).
        assert_eq!(ADD_LUT[key] & 0xf, 0, "non-canonical a_lo must yield 0");
    }

    // -----------------------------------------------------------------------
    // pack / lane / to_array roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_pack_unpack_roundtrip() {
        let arr: [Fp<7>; 16] = core::array::from_fn(|i| Fp::<7>::new((i as u64) % 7));
        let p = Packed7::pack(&arr);
        let out = p.to_array();
        assert_eq!(arr, out);
    }

    #[test]
    fn test_lane_all_values() {
        for v in 0u64..7 {
            let p = Packed7::splat(Fp::<7>::new(v));
            for i in 0..16 {
                assert_eq!(p.lane(i).value(), v, "lane {i}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_on_16() {
        let _ = Packed7::zero().lane(16);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_on_16() {
        let _ = Packed7::zero().with_lane(16, Fp::<7>::new(1));
    }

    // -----------------------------------------------------------------------
    // Exhaustive 7×7 tests for each op
    // -----------------------------------------------------------------------

    /// Exhaustive add: for all a, b ∈ {0..=6}, splat and verify all 16 lanes.
    #[test]
    fn test_exhaustive_add() {
        for a in 0u64..7 {
            for b in 0u64..7 {
                let pa = Packed7::splat(Fp::<7>::new(a));
                let pb = Packed7::splat(Fp::<7>::new(b));
                let r = pa.add(pb);
                let exp = scalar_add(a, b);
                for i in 0..16 {
                    assert_eq!(
                        r.lane(i).value(),
                        exp,
                        "add({a},{b}) lane {i}: expected {exp}"
                    );
                }
            }
        }
    }

    /// Exhaustive sub: for all a, b ∈ {0..=6}, splat and verify all 16 lanes.
    #[test]
    fn test_exhaustive_sub() {
        for a in 0u64..7 {
            for b in 0u64..7 {
                let pa = Packed7::splat(Fp::<7>::new(a));
                let pb = Packed7::splat(Fp::<7>::new(b));
                let r = pa.sub(pb);
                let exp = scalar_sub(a, b);
                for i in 0..16 {
                    assert_eq!(
                        r.lane(i).value(),
                        exp,
                        "sub({a},{b}) lane {i}: expected {exp}"
                    );
                }
            }
        }
    }

    /// Exhaustive mul: for all a, b ∈ {0..=6}, splat and verify all 16 lanes.
    #[test]
    fn test_exhaustive_mul() {
        for a in 0u64..7 {
            for b in 0u64..7 {
                let pa = Packed7::splat(Fp::<7>::new(a));
                let pb = Packed7::splat(Fp::<7>::new(b));
                let r = pa.mul(pb);
                let exp = scalar_mul(a, b);
                for i in 0..16 {
                    assert_eq!(
                        r.lane(i).value(),
                        exp,
                        "mul({a},{b}) lane {i}: expected {exp}"
                    );
                }
            }
        }
    }

    /// Exhaustive neg: for all a ∈ {0..=6}, splat and verify all 16 lanes.
    #[test]
    fn test_exhaustive_neg() {
        for a in 0u64..7 {
            let pa = Packed7::splat(Fp::<7>::new(a));
            let r = pa.neg();
            let exp = scalar_neg(a);
            for i in 0..16 {
                assert_eq!(r.lane(i).value(), exp, "neg({a}) lane {i}: expected {exp}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Per-lane mixed tests
    // -----------------------------------------------------------------------

    /// Pack two arrays with mixed values, run ops, verify per-lane vs scalar.
    #[test]
    fn test_per_lane_mixed_add() {
        let a_vals: [u64; 16] = [0, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 6, 0, 1];
        let b_vals: [u64; 16] = [6, 5, 4, 3, 2, 1, 0, 6, 5, 4, 3, 2, 1, 0, 6, 5];
        let a_arr: [Fp<7>; 16] = core::array::from_fn(|i| Fp::<7>::new(a_vals[i]));
        let b_arr: [Fp<7>; 16] = core::array::from_fn(|i| Fp::<7>::new(b_vals[i]));
        let pa = Packed7::pack(&a_arr);
        let pb = Packed7::pack(&b_arr);
        let r = pa.add(pb);
        for i in 0..16 {
            assert_eq!(
                r.lane(i).value(),
                scalar_add(a_vals[i], b_vals[i]),
                "lane {i}"
            );
        }
    }

    #[test]
    fn test_per_lane_mixed_mul() {
        let a_vals: [u64; 16] = [1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 6, 0, 1, 2];
        let b_vals: [u64; 16] = [2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3];
        let a_arr: [Fp<7>; 16] = core::array::from_fn(|i| Fp::<7>::new(a_vals[i]));
        let b_arr: [Fp<7>; 16] = core::array::from_fn(|i| Fp::<7>::new(b_vals[i]));
        let pa = Packed7::pack(&a_arr);
        let pb = Packed7::pack(&b_arr);
        let r = pa.mul(pb);
        for i in 0..16 {
            assert_eq!(
                r.lane(i).value(),
                scalar_mul(a_vals[i], b_vals[i]),
                "lane {i}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Proptest cross-check against scalar Fp<7> per-lane (1000 cases each)
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig { cases: 1000, .. ProptestConfig::default() })]

        #[test]
        fn test_proptest_add_matches_scalar(
            a in packed7_strat(),
            b in packed7_strat(),
        ) {
            let r = a.add(b);
            for i in 0..16 {
                let expected = scalar_add(a.lane(i).value(), b.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected);
            }
        }

        #[test]
        fn test_proptest_sub_matches_scalar(
            a in packed7_strat(),
            b in packed7_strat(),
        ) {
            let r = a.sub(b);
            for i in 0..16 {
                let expected = scalar_sub(a.lane(i).value(), b.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected);
            }
        }

        #[test]
        fn test_proptest_mul_matches_scalar(
            a in packed7_strat(),
            b in packed7_strat(),
        ) {
            let r = a.mul(b);
            for i in 0..16 {
                let expected = scalar_mul(a.lane(i).value(), b.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected);
            }
        }

        #[test]
        fn test_proptest_neg_matches_scalar(a in packed7_strat()) {
            let r = a.neg();
            for i in 0..16 {
                let expected = scalar_neg(a.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Packed7Vec — word-boundary tests
    // -----------------------------------------------------------------------

    fn make_vec(len: usize) -> (Packed7Vec, Packed7Vec) {
        // Build two vecs with alternating values.
        let a_vals: Vec<Fp<7>> = (0..len).map(|i| Fp::<7>::new((i as u64 * 3) % 7)).collect();
        let b_vals: Vec<Fp<7>> = (0..len)
            .map(|i| Fp::<7>::new((i as u64 * 5 + 1) % 7))
            .collect();
        (
            Packed7Vec::from_field_slice(&a_vals),
            Packed7Vec::from_field_slice(&b_vals),
        )
    }

    fn check_op_vec(len: usize, op: &str) {
        let (mut a, b) = make_vec(len);
        let a_vals: Vec<u64> = (0..len).map(|i| a.get(i).value()).collect();
        let b_vals: Vec<u64> = (0..len).map(|i| b.get(i).value()).collect();
        match op {
            "add" => {
                a.add_assign(&b);
                for (i, (&av, &bv)) in a_vals.iter().zip(b_vals.iter()).enumerate() {
                    assert_eq!(
                        a.get(i).value(),
                        scalar_add(av, bv),
                        "add len={len} lane {i}"
                    );
                }
            }
            "sub" => {
                a.sub_assign(&b);
                for (i, (&av, &bv)) in a_vals.iter().zip(b_vals.iter()).enumerate() {
                    assert_eq!(
                        a.get(i).value(),
                        scalar_sub(av, bv),
                        "sub len={len} lane {i}"
                    );
                }
            }
            "mul" => {
                a.mul_assign(&b);
                for (i, (&av, &bv)) in a_vals.iter().zip(b_vals.iter()).enumerate() {
                    assert_eq!(
                        a.get(i).value(),
                        scalar_mul(av, bv),
                        "mul len={len} lane {i}"
                    );
                }
            }
            "neg" => {
                a.neg_assign();
                for (i, &av) in a_vals.iter().enumerate() {
                    assert_eq!(a.get(i).value(), scalar_neg(av), "neg len={len} lane {i}");
                }
            }
            _ => panic!("unknown op"),
        }
    }

    #[test]
    fn test_packed7vec_word_boundaries() {
        for &len in &[0usize, 1, 15, 16, 17, 63, 64, 65, 127, 128, 129] {
            for op in &["add", "sub", "mul", "neg"] {
                check_op_vec(len, op);
            }
        }
    }

    /// Verify mask_tail invariant: padding nibbles in last word are always zero.
    #[test]
    fn test_packed7vec_mask_tail_invariant() {
        for &len in &[1usize, 15, 16, 17, 63, 64, 65, 127, 128, 129] {
            let v = Packed7Vec::zeros(len);
            if !v.words.is_empty() {
                let n = v.words.len();
                let used = len - 16 * (n - 1);
                if used < 16 {
                    let mask = (1u64 << (4 * used)) - 1;
                    assert_eq!(
                        v.words[n - 1] & !mask,
                        0,
                        "padding must be zero for len={len}"
                    );
                }
            }
            // After from_field_slice
            let vals: Vec<Fp<7>> = (0..len).map(|i| Fp::<7>::new((i as u64) % 7)).collect();
            let v2 = Packed7Vec::from_field_slice(&vals);
            if !v2.words.is_empty() {
                let n = v2.words.len();
                let used = len - 16 * (n - 1);
                if used < 16 {
                    let mask = (1u64 << (4 * used)) - 1;
                    assert_eq!(
                        v2.words[n - 1] & !mask,
                        0,
                        "padding must be zero after from_field_slice for len={len}"
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // all_zero and one
    // -----------------------------------------------------------------------

    #[test]
    fn test_zero_is_all_zero() {
        assert!(Packed7::zero().all_zero());
        assert!(Packed7Vec::zeros(0).all_zero());
        assert!(Packed7Vec::zeros(17).all_zero());
    }

    #[test]
    fn test_one_has_correct_lanes() {
        let o = Packed7::one();
        for i in 0..16 {
            assert_eq!(o.lane(i), Fp::<7>::new(1), "one lane {i}");
        }
    }

    #[test]
    fn test_splat_zero_is_all_zero() {
        let z = Packed7::splat(Fp::<7>::new(0));
        assert!(z.all_zero());
    }

    // -----------------------------------------------------------------------
    // PackedField trait delegation
    // -----------------------------------------------------------------------

    #[test]
    fn test_packed_field_trait_lanes() {
        assert_eq!(<Packed7 as PackedField<Fp<7>>>::LANES, 16);
    }

    #[test]
    fn test_packed_field_trait_ops() {
        let a = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(4));
        let b = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(5));
        assert_eq!(a.add(b).lane(0), Fp::<7>::new(2)); // (4+5)%7=2
        assert_eq!(a.sub(b).lane(0), Fp::<7>::new(6)); // (4-5+7)%7=6
        assert_eq!(a.mul(b).lane(0), Fp::<7>::new(6)); // (4*5)%7=6
        assert_eq!(a.neg().lane(0), Fp::<7>::new(3)); // (7-4)%7=3
    }
}
