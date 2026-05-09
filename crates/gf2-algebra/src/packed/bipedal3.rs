//! Fixed-width packed `F_3` element encoding ("bipedal3").
//!
//! [`Bipedal3`] packs exactly **64** independent `F_3` lanes into two
//! `u64` words (`mag` and `sgn`).  Arithmetic follows the bitwise
//! formulas of Scheinerman 2024 (arXiv 2407.20205v2, Theorem 2.1 /
//! Algorithm 2).  The implementation is extractable by Charon/Aeneas:
//! every operation is a flat, straight-line expression — no closures,
//! no iterators, no helper traits.
//!
//! # Encoding
//!
//! Each `F_3` element `x ∈ {0, 1, 2}` is stored as `(mag_bit, sgn_bit)`:
//!
//! | `x` | `mag` bit | `sgn` bit | note                      |
//! |-----|-----------|-----------|---------------------------|
//! |  0  |     0     |     0     | canonical zero             |
//! |  1  |     1     |     0     |                            |
//! |  2  |     1     |     1     | `≡ −1 (mod 3)`            |
//! | alt |     0     |     1     | **alternative zero** — never produced by |
//! |     |           |           | `add/sub/mul/neg` from canonical inputs; |
//! |     |           |           | `lane`, `all_zero`, and `Eq` treat it as 0. |
//!
//! Bit `s` of `mag` and bit `s` of `sgn` encode lane `s`.
//!
//! # Op cost (per `Bipedal3` = 64 `F_3` elements)
//!
//! - **add**: 6 word-level ops (CSE: 2 temporaries).
//! - **sub**: 6 word-level ops (direct paper §2.2 transliteration).
//! - **mul**: 2 word-level ops.
//! - **neg**: 1 word-level op.
//!
//! # Cross-check oracle
//!
//! [`super::ScalarPackedFp3`] is the canonical reference; the proptest
//! suite in this module routes random inputs through both and asserts
//! per-lane equality for all four operations.
//!
//! # Status
//!
//! W1-T3: body implemented.  `Bipedal3Vec` (T4) and `Bipedal3Matrix`
//! (T5) will be added in subsequent issues; this module only hosts the
//! fixed-width element.

use core::fmt;

use gf2_core::gfp::Fp;

use super::{PackedField, PackedFieldVec};

/// Fixed-width packed `F_3` element encoding 64 lanes in a `(mag, sgn)`
/// `u64` pair using the bitwise formulas of Scheinerman 2024.
///
/// Each lane `i` (0 ≤ `i` < 64) stores one `F_3` element in the pair of
/// bits `(mag >> i) & 1` and `(sgn >> i) & 1`, using the encoding:
///
/// | `F_3` value | `mag` bit | `sgn` bit |
/// |-------------|-----------|-----------|
/// |      0      |     0     |     0     |
/// |      1      |     1     |     0     |
/// |      2      |     1     |     1     |
///
/// The codeword `(mag=0, sgn=1)` is an *alternative zero*: it is never
/// produced by the arithmetic operations but may appear in manually
/// constructed values. [`Bipedal3::lane`], [`Bipedal3::all_zero`], and
/// the `PartialEq` / `Eq` implementations all treat it as zero.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedField, Bipedal3};
/// use gf2_core::gfp::Fp;
///
/// let a = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
/// let b = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
/// let s = a.add(b);
/// assert_eq!(s.lane(0), Fp::<3>::new(0)); // 1 + 2 == 0 mod 3
/// assert!(s.all_zero());
/// ```
///
/// # Complexity
///
/// All operations are `O(1)` — a fixed number of word-level bitwise
/// instructions independent of the number of lanes.
#[derive(Clone, Copy)]
pub struct Bipedal3 {
    mag: u64,
    sgn: u64,
}

// ---------------------------------------------------------------------------
// Manual PartialEq / Eq — canonical-decode equality.
//
// Two `Bipedal3` values are equal iff every lane decodes to the same
// `F_3` value.  Because the alternative-zero codeword `(mag=0, sgn=1)`
// decodes to 0, the `sgn` bit for a lane is irrelevant whenever the
// corresponding `mag` bit is 0.  Concretely:
//
//   a == b  iff  a.mag == b.mag
//                && (a.sgn ^ b.sgn) & a.mag == 0
//
// This is equivalent to per-lane `lane(i) == other.lane(i)` for all i,
// but faster: two comparisons and one AND instead of 64 scalar decodes.
// ---------------------------------------------------------------------------

impl PartialEq for Bipedal3 {
    /// Canonical-decode equality: two values are equal iff every decoded
    /// lane is equal, regardless of the `sgn` bit on lanes whose `mag`
    /// bit is 0 (alternative-zero lanes).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{Bipedal3, PackedField};
    /// use gf2_core::gfp::Fp;
    ///
    /// // Canonical zero equals alt-zero (mag=0, sgn=MAX).
    /// let canon = <Bipedal3 as PackedField<Fp<3>>>::zero();
    /// let alt = Bipedal3::from_raw(0, u64::MAX);
    /// assert_eq!(canon, alt);
    ///
    /// // 1 != 2 even in the same lane.
    /// let one = Bipedal3::splat_raw(1, 0); // every lane = 1
    /// let two = Bipedal3::splat_raw(1, 1); // every lane = 2
    /// assert_ne!(one, two);
    /// ```
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.mag != other.mag {
            return false;
        }
        // mag bits equal; check that sgn differs only on lanes where
        // mag == 0 (where sgn is don't-care for canonical-decode).
        (self.sgn ^ other.sgn) & self.mag == 0
    }
}

impl Eq for Bipedal3 {}

// ---------------------------------------------------------------------------
// Manual Debug — print lane values (0/1/2) as an array, not raw bits.
// ---------------------------------------------------------------------------

impl fmt::Debug for Bipedal3 {
    /// Formats the value as a 64-element array of decoded lane values
    /// (each in `{0, 1, 2}`), matching the style of
    /// [`ScalarPackedFp3`]'s `Debug` impl for stable `assert_eq!`
    /// messages.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3;
    /// use gf2_algebra::packed::PackedField;
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// let s = format!("{:?}", v);
    /// assert!(s.contains("lanes"));
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bipedal3")
            .field(
                "lanes",
                &core::array::from_fn::<u64, 64, _>(|i| {
                    let m = (self.mag >> i) & 1;
                    let g = (self.sgn >> i) & 1;
                    if m == 0 {
                        0u64
                    } else if g == 0 {
                        1u64
                    } else {
                        2u64
                    }
                }),
            )
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Internal constructors used by tests (same module — struct fields visible).
// ---------------------------------------------------------------------------

impl Bipedal3 {
    /// Construct a `Bipedal3` from raw `(mag, sgn)` words.
    ///
    /// This is a low-level escape hatch for unit tests that need to
    /// inject specific bit patterns (e.g. the alternative-zero
    /// codeword).  Production code should use [`PackedField::splat`],
    /// [`PackedField::with_lane`], or the arithmetic ops.
    ///
    /// # Arguments
    ///
    /// * `mag` — raw magnitude word; bit `i` is the `mag` bit of lane `i`.
    /// * `sgn` — raw sign word; bit `i` is the `sgn` bit of lane `i`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3;
    ///
    /// // All lanes = 1 (mag=1, sgn=0 per lane).
    /// let v = Bipedal3::from_raw(u64::MAX, 0);
    /// use gf2_algebra::packed::PackedField;
    /// use gf2_core::gfp::Fp;
    /// assert_eq!(v, <Bipedal3 as PackedField<Fp<3>>>::one());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn from_raw(mag: u64, sgn: u64) -> Self {
        Self { mag, sgn }
    }

    /// Broadcast a single `(mag_bit, sgn_bit)` pair to all 64 lanes.
    ///
    /// A helper for internal tests; `splat_raw(1, 0)` gives all-1s,
    /// `splat_raw(1, 1)` gives all-2s, and `splat_raw(0, 0)` gives
    /// all-zeros.
    ///
    /// # Arguments
    ///
    /// * `mag_bit` — 0 or 1; broadcast to every lane's `mag` bit.
    /// * `sgn_bit` — 0 or 1; broadcast to every lane's `sgn` bit.
    ///
    /// # Panics
    ///
    /// Does not panic; values outside 0/1 simply saturate to 0 or 1 via
    /// the mask.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3;
    /// use gf2_algebra::packed::PackedField;
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Bipedal3::splat_raw(1, 0);
    /// assert_eq!(v, <Bipedal3 as PackedField<Fp<3>>>::one());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn splat_raw(mag_bit: u64, sgn_bit: u64) -> Self {
        Self {
            mag: 0u64.wrapping_sub(mag_bit & 1), // 0 → 0, 1 → u64::MAX
            sgn: 0u64.wrapping_sub(sgn_bit & 1),
        }
    }
}

// ---------------------------------------------------------------------------
// PackedField<Fp<3>>
// ---------------------------------------------------------------------------

impl PackedField<Fp<3>> for Bipedal3 {
    /// Number of independent `F_3` lanes packed into one `Bipedal3`.
    ///
    /// Fixed at 64 to match the `u64`-pair encoding width.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    /// assert_eq!(<Bipedal3 as PackedField<Fp<3>>>::LANES, 64);
    /// ```
    const LANES: usize = 64;

    /// Returns the all-zeros `Bipedal3` (every lane = 0).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let z = <Bipedal3 as PackedField<Fp<3>>>::zero();
    /// assert!(z.all_zero());
    /// for i in 0..64 { assert_eq!(z.lane(i), Fp::<3>::new(0)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn zero() -> Self {
        Self { mag: 0, sgn: 0 }
    }

    /// Returns the all-ones `Bipedal3` (every lane = 1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let o = <Bipedal3 as PackedField<Fp<3>>>::one();
    /// for i in 0..64 { assert_eq!(o.lane(i), Fp::<3>::new(1)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn one() -> Self {
        Self {
            mag: u64::MAX,
            sgn: 0,
        }
    }

    /// Broadcasts scalar `x` to all 64 lanes.
    ///
    /// # Arguments
    ///
    /// * `x` — scalar `F_3` value to replicate across all lanes.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// for i in 0..64 { assert_eq!(v.lane(i), Fp::<3>::new(2)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn splat(x: Fp<3>) -> Self {
        let v = x.value(); // 0, 1, or 2
                           // Encoding: 0 → (0,0), 1 → (1,0), 2 → (1,1).
        let mag_bit = if v != 0 { 1u64 } else { 0u64 };
        let sgn_bit = if v == 2 { 1u64 } else { 0u64 };
        Self::splat_raw(mag_bit, sgn_bit)
    }

    /// Lane-wise sum using Scheinerman 2024 Algorithm 2 (6 ops, CSE).
    ///
    /// # Arguments
    ///
    /// * `rhs` — the other operand; lanes are added pointwise mod 3.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// let b = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// assert_eq!(a.add(b).lane(0), Fp::<3>::new(1)); // 2+2=4≡1 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 6 word-level bitwise operations.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        let am = self.mag;
        let asg = self.sgn;
        let bm = rhs.mag;
        let bsg = rhs.sgn;
        // Algorithm 2 (6 ops with CSE).
        let t = am ^ asg ^ bsg;
        let u = bm & t;
        Self {
            mag: u | (am ^ bm),
            sgn: u ^ asg,
        }
    }

    /// Lane-wise difference: `self - rhs` pointwise mod 3.
    ///
    /// Direct paper §2.2 / Theorem 2.1 subtraction transliteration:
    /// `t = s1 ⊕ s2; u = m1 ∧ t; m_- = u | (m1 ⊕ m2); s_- = u ⊕ (m2 ⊕ s2)`
    /// — 6 word-level bitwise operations.
    ///
    /// # Arguments
    ///
    /// * `rhs` — the operand subtracted lane-by-lane from `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(0));
    /// let b = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
    /// assert_eq!(a.sub(b).lane(0), Fp::<3>::new(2)); // 0-1=-1≡2 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 6 word-level bitwise operations.
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let am = self.mag;
        let asg = self.sgn;
        let bm = rhs.mag;
        let bsg = rhs.sgn;
        // Canonical paper §2.2 / Theorem 2.1 sub transliteration. 6 ops total.
        let t = asg ^ bsg; // op 1
        let u = am & t; // op 2
        Self {
            mag: u | (am ^ bm),  // op 3 (XOR) + op 4 (OR)
            sgn: u ^ (bm ^ bsg), // op 5 (XOR) + op 6 (XOR)
        }
    }

    /// Lane-wise additive inverse: `−self` mod 3.
    ///
    /// For `x ∈ F_3`: `neg(0)=0`, `neg(1)=2`, `neg(2)=1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
    /// assert_eq!(a.neg().lane(0), Fp::<3>::new(2)); // -1≡2 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 1 word-level XOR.
    #[inline]
    fn neg(self) -> Self {
        Self {
            mag: self.mag,
            sgn: self.sgn ^ self.mag,
        }
    }

    /// Lane-wise product using Scheinerman 2024 (2 ops).
    ///
    /// # Arguments
    ///
    /// * `rhs` — the other operand; lanes are multiplied pointwise mod 3.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// let b = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// assert_eq!(a.mul(b).lane(0), Fp::<3>::new(1)); // 2*2=4≡1 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 2 word-level bitwise operations.
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            mag: self.mag & rhs.mag,
            sgn: self.sgn ^ rhs.sgn,
        }
    }

    /// Decode lane `i` to a canonical `F_3` value.
    ///
    /// The alternative-zero codeword `(mag=0, sgn=1)` is canonicalised
    /// to `Fp::<3>::new(0)`.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..64`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Bipedal3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
    /// assert_eq!(v.lane(0), Fp::<3>::new(2));
    /// assert_eq!(v.lane(63), Fp::<3>::new(2));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: two bit-extracts and a decode.
    #[inline]
    fn lane(self, i: usize) -> Fp<3> {
        assert!(
            i < Self::LANES,
            "Bipedal3::lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        let m = (self.mag >> i) & 1;
        let g = (self.sgn >> i) & 1;
        // (mag=0, sgn=*) → 0 (canonicalises alt-zero)
        // (mag=1, sgn=0) → 1
        // (mag=1, sgn=1) → 2
        if m == 0 {
            Fp::<3>::new(0)
        } else if g == 0 {
            Fp::<3>::new(1)
        } else {
            Fp::<3>::new(2)
        }
    }

    /// Write the canonical encoding of `x` into lane `i`.
    ///
    /// Always writes the canonical codeword for `x`; any pre-existing
    /// alternative-zero codeword at lane `i` is overwritten with the
    /// canonical encoding (D1b §3.5).
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..64`.
    /// * `x` — scalar `F_3` value to write into lane `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Bipedal3 as PackedField<Fp<3>>>::zero();
    /// let v = v.with_lane(7, Fp::<3>::new(2));
    /// assert_eq!(v.lane(7), Fp::<3>::new(2));
    /// assert_eq!(v.lane(0), Fp::<3>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: constant number of bit-mask and bit-set operations.
    #[inline]
    fn with_lane(self, i: usize, x: Fp<3>) -> Self {
        assert!(
            i < Self::LANES,
            "Bipedal3::with_lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        let v = x.value(); // 0, 1, or 2
        let mag_bit = if v != 0 { 1u64 } else { 0u64 };
        let sgn_bit = if v == 2 { 1u64 } else { 0u64 };
        let mask = 1u64 << i;
        Self {
            mag: (self.mag & !mask) | (mag_bit << i),
            // Always write canonical sgn; clear old sgn bit first.
            sgn: (self.sgn & !mask) | (sgn_bit << i),
        }
    }

    /// Returns `true` iff every lane decodes to 0.
    ///
    /// Implemented as `self.mag == 0`: since a lane's value is zero iff
    /// its `mag` bit is 0 (the `sgn` bit is irrelevant when `mag=0`),
    /// testing `mag` alone suffices.  This also correctly handles the
    /// alternative-zero codeword `(mag=0, sgn=1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Bipedal3};
    /// use gf2_core::gfp::Fp;
    ///
    /// assert!(<Bipedal3 as PackedField<Fp<3>>>::zero().all_zero());
    /// // Alternative-zero codeword: mag=0, sgn=u64::MAX.
    /// let alt = Bipedal3::from_raw(0, u64::MAX);
    /// assert!(alt.all_zero());
    /// // One lane set to 1 → not all-zero.
    /// let v = <Bipedal3 as PackedField<Fp<3>>>::zero().with_lane(3, Fp::<3>::new(1));
    /// assert!(!v.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: one 64-bit comparison.
    #[inline]
    fn all_zero(self) -> bool {
        self.mag == 0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::ScalarPackedFp3;
    use super::*;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Strategy: a single `Fp<3>` element drawn uniformly from `{0, 1, 2}`.
    fn fp3_strat() -> impl Strategy<Value = Fp<3>> {
        (0u64..3).prop_map(Fp::<3>::new)
    }

    /// Strategy: a `Bipedal3` with every lane independently drawn from
    /// `{0, 1, 2}`.
    fn bipedal_strat() -> impl Strategy<Value = Bipedal3> {
        prop::collection::vec(fp3_strat(), 64).prop_map(|v| {
            let mut p = Bipedal3::zero();
            for (i, x) in v.into_iter().enumerate() {
                p = p.with_lane(i, x);
            }
            p
        })
    }

    /// Strategy: a `Bipedal3` where some lanes may be the alternative-zero
    /// codeword `(mag=0, sgn=1)`.
    fn bipedal_with_alt_zero_strat() -> impl Strategy<Value = Bipedal3> {
        // Build a canonical Bipedal3 first, then independently set each
        // sgn bit to 0 or 1 for lanes where mag=0 (injecting alt-zeros).
        bipedal_strat().prop_flat_map(|b| {
            // For lanes where mag=0, independently choose sgn=0 or sgn=1.
            (any::<u64>()).prop_map(move |extra_sgn| {
                // extra_sgn bits apply only to mag=0 lanes.
                let zero_lanes = !b.mag; // bits set where lane is 0
                Bipedal3 {
                    mag: b.mag,
                    sgn: b.sgn | (extra_sgn & zero_lanes),
                }
            })
        })
    }

    /// Strategy: a matching `ScalarPackedFp3` that has the same per-lane
    /// values as a `Bipedal3`, used for cross-checking.
    fn scalar_from_bipedal(b: &Bipedal3) -> ScalarPackedFp3 {
        let mut s = ScalarPackedFp3::zero();
        for i in 0..64 {
            s = s.with_lane(i, b.lane(i));
        }
        s
    }

    // -----------------------------------------------------------------------
    // LANES constant
    // -----------------------------------------------------------------------

    #[test]
    fn test_lanes_const_is_64() {
        assert_eq!(<Bipedal3 as PackedField<Fp<3>>>::LANES, 64);
    }

    // -----------------------------------------------------------------------
    // Truth-table tests for all 9 (a,b) pairs
    // -----------------------------------------------------------------------

    /// Add truth table: all 9 pairs from {0,1,2}^2.
    #[test]
    fn test_add_truth_table() {
        // F_3 addition table.
        let expected: [[u64; 3]; 3] = [
            [0, 1, 2], // 0+0, 0+1, 0+2
            [1, 2, 0], // 1+0, 1+1, 1+2
            [2, 0, 1], // 2+0, 2+1, 2+2
        ];
        for a_v in 0u64..3 {
            for b_v in 0u64..3 {
                let a = Bipedal3::splat(Fp::<3>::new(a_v));
                let b = Bipedal3::splat(Fp::<3>::new(b_v));
                let result = a.add(b);
                let got = result.lane(0).value();
                let exp = expected[a_v as usize][b_v as usize];
                assert_eq!(got, exp, "add({a_v}, {b_v}): expected {exp}, got {got}");
                // All lanes should agree.
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), exp);
                }
            }
        }
    }

    /// Sub truth table: all 9 pairs from {0,1,2}^2.
    #[test]
    fn test_sub_truth_table() {
        // F_3 subtraction table (a - b mod 3).
        let expected: [[u64; 3]; 3] = [
            [0, 2, 1], // 0-0, 0-1, 0-2
            [1, 0, 2], // 1-0, 1-1, 1-2
            [2, 1, 0], // 2-0, 2-1, 2-2
        ];
        for a_v in 0u64..3 {
            for b_v in 0u64..3 {
                let a = Bipedal3::splat(Fp::<3>::new(a_v));
                let b = Bipedal3::splat(Fp::<3>::new(b_v));
                let result = a.sub(b);
                let got = result.lane(0).value();
                let exp = expected[a_v as usize][b_v as usize];
                assert_eq!(got, exp, "sub({a_v}, {b_v}): expected {exp}, got {got}");
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), exp);
                }
            }
        }
    }

    /// Mul truth table: all 9 pairs from {0,1,2}^2.
    #[test]
    fn test_mul_truth_table() {
        // F_3 multiplication table.
        let expected: [[u64; 3]; 3] = [
            [0, 0, 0], // 0*0, 0*1, 0*2
            [0, 1, 2], // 1*0, 1*1, 1*2
            [0, 2, 1], // 2*0, 2*1, 2*2
        ];
        for a_v in 0u64..3 {
            for b_v in 0u64..3 {
                let a = Bipedal3::splat(Fp::<3>::new(a_v));
                let b = Bipedal3::splat(Fp::<3>::new(b_v));
                let result = a.mul(b);
                let got = result.lane(0).value();
                let exp = expected[a_v as usize][b_v as usize];
                assert_eq!(got, exp, "mul({a_v}, {b_v}): expected {exp}, got {got}");
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), exp);
                }
            }
        }
    }

    /// Neg truth table: all 3 single inputs.
    #[test]
    fn test_neg_truth_table() {
        // F_3 negation: -0=0, -1=2, -2=1.
        let expected = [0u64, 2, 1];
        for v in 0u64..3 {
            let a = Bipedal3::splat(Fp::<3>::new(v));
            let result = a.neg();
            let got = result.lane(0).value();
            let exp = expected[v as usize];
            assert_eq!(got, exp, "neg({v}): expected {exp}, got {got}");
            for i in 1..64 {
                assert_eq!(result.lane(i).value(), exp);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Alt-zero codeword
    // -----------------------------------------------------------------------

    /// `lane(i)` canonicalises the alt-zero codeword `(mag=0, sgn=1)` to 0.
    #[test]
    fn test_lane_canonicalises_alt_zero() {
        // Construct directly — field access allowed within the same module.
        let v = Bipedal3 {
            mag: 0,
            sgn: 1 << 5,
        };
        assert_eq!(
            v.lane(5),
            Fp::<3>::new(0),
            "alt-zero at lane 5 must decode to 0"
        );
        // Other lanes must also be zero.
        for i in 0..64 {
            assert_eq!(v.lane(i), Fp::<3>::new(0));
        }
    }

    /// `Bipedal3::zero() == Bipedal3 { mag: 0, sgn: u64::MAX }`.
    #[test]
    fn test_eq_alt_zero_equals_canonical_zero() {
        let canon = Bipedal3::zero();
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        assert_eq!(
            canon, alt,
            "canonical zero and all-alt-zero must compare equal"
        );
    }

    /// `with_lane` always writes canonical encoding; the raw `sgn` bit must
    /// be 0 when writing 0, even if the lane previously held an alt-zero.
    #[test]
    fn test_with_lane_canonicalises() {
        // Start from a value where lane 0 is alt-zero (mag=0, sgn=1).
        let start = Bipedal3 {
            mag: u64::MAX, // all lanes = 1
            sgn: 0,
        };
        // Write 0 into lane 0.
        let result = start.with_lane(0, Fp::<3>::new(0));
        // mag bit 0 must be 0 (not nonzero).
        assert_eq!(result.mag & 1, 0, "mag bit 0 must be cleared");
        // sgn bit 0 must be 0 (canonical encoding of 0).
        assert_eq!(result.sgn & 1, 0, "sgn bit 0 must be canonical (0)");
        // Other lanes unaffected.
        for i in 1..64 {
            assert_eq!(result.lane(i), Fp::<3>::new(1));
        }
    }

    /// Round-trip: `with_lane(i, lane(i))` is idempotent.
    #[test]
    fn test_with_lane_roundtrip() {
        let mut v = Bipedal3::zero();
        for i in 0..64 {
            v = v.with_lane(i, Fp::<3>::new((i as u64) % 3));
        }
        for i in 0..64 {
            let v2 = v.with_lane(i, v.lane(i));
            assert_eq!(v, v2, "round-trip failed at lane {i}");
        }
    }

    // -----------------------------------------------------------------------
    // all_zero
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_zero_canonical_zero() {
        assert!(Bipedal3::zero().all_zero());
    }

    #[test]
    fn test_all_zero_alt_zero() {
        // mag=0, sgn=MAX — all alternative-zero codewords.
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        assert!(alt.all_zero(), "alt-zero must be reported as all-zero");
    }

    #[test]
    fn test_all_zero_one_nonzero_lane() {
        let v = Bipedal3::zero().with_lane(17, Fp::<3>::new(2));
        assert!(!v.all_zero());
    }

    // -----------------------------------------------------------------------
    // Panic tests
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_out_of_range_64() {
        let _ = Bipedal3::zero().lane(64);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_out_of_range_65() {
        let _ = Bipedal3::zero().lane(65);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_out_of_range_64() {
        let _ = Bipedal3::zero().with_lane(64, Fp::<3>::new(1));
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_out_of_range_65() {
        let _ = Bipedal3::zero().with_lane(65, Fp::<3>::new(1));
    }

    // -----------------------------------------------------------------------
    // Alt-zero through every op (non-randomised, explicit)
    // -----------------------------------------------------------------------

    /// Alt-zero inputs through add must produce the same result as canonical zero.
    #[test]
    fn test_alt_zero_through_add() {
        // alt_zero in all lanes: mag=0, sgn=u64::MAX.
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        let one = Bipedal3::splat(Fp::<3>::new(1));

        // alt_zero + one should equal canonical_zero + one = one.
        let r_alt = alt.add(one);
        let r_can = Bipedal3::zero().add(one);
        assert_eq!(r_alt, r_can, "alt_zero + one != canonical_zero + one");

        // one + alt_zero should equal one + canonical_zero = one.
        let r_alt2 = one.add(alt);
        assert_eq!(r_alt2, r_can, "one + alt_zero != one + canonical_zero");
    }

    /// Alt-zero inputs through sub.
    #[test]
    fn test_alt_zero_through_sub() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        let two = Bipedal3::splat(Fp::<3>::new(2));

        // 2 - alt_zero == 2 - 0 == 2.
        let r = two.sub(alt);
        assert_eq!(r.lane(0).value(), 2, "2 - alt_zero lane 0 must be 2");

        // alt_zero - 2 == 0 - 2 == 1.
        let r2 = alt.sub(two);
        assert_eq!(r2.lane(0).value(), 1, "alt_zero - 2 lane 0 must be 1");
    }

    /// Alt-zero inputs through mul.
    #[test]
    fn test_alt_zero_through_mul() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        let two = Bipedal3::splat(Fp::<3>::new(2));

        // 2 * alt_zero == 2 * 0 == 0.
        let r = two.mul(alt);
        assert!(r.all_zero(), "2 * alt_zero must be 0");

        // alt_zero * 2 == 0 * 2 == 0.
        let r2 = alt.mul(two);
        assert!(r2.all_zero(), "alt_zero * 2 must be 0");
    }

    /// Alt-zero inputs through neg.
    #[test]
    fn test_alt_zero_through_neg() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };

        // neg(alt_zero) — alt_zero has mag=0 so neg formula gives:
        //   mag' = 0, sgn' = u64::MAX ^ 0 = u64::MAX
        // which is still alt-zero, which decodes to 0.
        let r = alt.neg();
        // All lanes must decode to 0.
        for i in 0..64 {
            assert_eq!(
                r.lane(i),
                Fp::<3>::new(0),
                "neg(alt_zero) lane {i} must be 0"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Proptest cross-check vs ScalarPackedFp3 oracle (1000 cases each)
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig { cases: 1000, .. ProptestConfig::default() })]

        /// add: Bipedal3 agrees with ScalarPackedFp3 on every lane.
        #[test]
        fn test_proptest_add_matches_scalar(
            a in bipedal_strat(),
            b in bipedal_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.add(b);
            let sr = sa.add(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "add lane {} mismatch", i
                );
            }
        }

        /// sub: Bipedal3 agrees with ScalarPackedFp3 on every lane.
        #[test]
        fn test_proptest_sub_matches_scalar(
            a in bipedal_strat(),
            b in bipedal_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.sub(b);
            let sr = sa.sub(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "sub lane {} mismatch", i
                );
            }
        }

        /// mul: Bipedal3 agrees with ScalarPackedFp3 on every lane.
        #[test]
        fn test_proptest_mul_matches_scalar(
            a in bipedal_strat(),
            b in bipedal_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.mul(b);
            let sr = sa.mul(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "mul lane {} mismatch", i
                );
            }
        }

        /// neg: Bipedal3 agrees with ScalarPackedFp3 on every lane.
        #[test]
        fn test_proptest_neg_matches_scalar(a in bipedal_strat()) {
            let sa = scalar_from_bipedal(&a);
            let br = a.neg();
            let sr = sa.neg();
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "neg lane {} mismatch", i
                );
            }
        }

        /// add with alt-zero codewords agrees with ScalarPackedFp3.
        #[test]
        fn test_proptest_add_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
            b in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.add(b);
            let sr = sa.add(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "add (alt-zero) lane {} mismatch", i
                );
            }
        }

        /// sub with alt-zero codewords agrees with ScalarPackedFp3.
        #[test]
        fn test_proptest_sub_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
            b in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.sub(b);
            let sr = sa.sub(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "sub (alt-zero) lane {} mismatch", i
                );
            }
        }

        /// mul with alt-zero codewords agrees with ScalarPackedFp3.
        #[test]
        fn test_proptest_mul_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
            b in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.mul(b);
            let sr = sa.mul(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "mul (alt-zero) lane {} mismatch", i
                );
            }
        }

        /// neg with alt-zero codewords agrees with ScalarPackedFp3.
        #[test]
        fn test_proptest_neg_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let br = a.neg();
            let sr = sa.neg();
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "neg (alt-zero) lane {} mismatch", i
                );
            }
        }
    }
}

// ===========================================================================
// Bipedal3Vec — variable-length packed F_3 vector
// ===========================================================================

/// Variable-length packed `F_3` vector storing `len_lanes` elements as
/// two parallel `Vec<u64>` words (`mag` and `sgn`), each of length
/// `ceil(len_lanes / 64)`.
///
/// The encoding of each element matches [`Bipedal3`]: element at logical
/// position `i` lives in word `i >> 6` at bit `i & 63` of both `mag` and
/// `sgn`.
///
/// # Mask-tail invariant
///
/// Bits beyond `len_lanes` in the last word of both `mag` and `sgn` must
/// always be zero. Every mutating operation calls [`Bipedal3Vec::mask_tail`]
/// to enforce this invariant — it is the most critical correctness invariant
/// in this codebase (CLAUDE.md §Key design invariants #1).
///
/// # Encoding summary
///
/// | `F_3` value | `mag` bit | `sgn` bit |
/// |-------------|-----------|-----------|
/// |      0      |     0     |     0     |
/// |      1      |     1     |     0     |
/// |      2      |     1     |     1     |
///
/// The alternative-zero codeword `(mag=0, sgn=1)` is treated as canonical
/// zero in [`get`][`Bipedal3Vec::get`], [`all_zero`][`PackedFieldVec::all_zero`],
/// and [`PartialEq`].
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
/// use gf2_core::gfp::Fp;
///
/// let v = Bipedal3Vec::zeros(5);
/// assert_eq!(v.len(), 5);
/// assert!(v.all_zero());
/// ```
///
/// # Complexity
///
/// Construction and lane-wise operations are `O(ceil(len_lanes / 64))`.
/// Individual lane access ([`get`][`Bipedal3Vec::get`]) is `O(1)`.
#[derive(Clone)]
pub struct Bipedal3Vec {
    mag: Vec<u64>,
    sgn: Vec<u64>,
    len_lanes: usize,
}

// ---------------------------------------------------------------------------
// mask_tail
// ---------------------------------------------------------------------------

impl Bipedal3Vec {
    /// Zero out all bits beyond `self.len_lanes` in the last word of both
    /// `mag` and `sgn`.
    ///
    /// **This invariant must hold after every mutation.** Failing to call
    /// `mask_tail` after any write violates the project's key correctness
    /// invariant (CLAUDE.md §Key design invariants #1). Arithmetic
    /// operations use word-parallel formulas over the full word including
    /// padding bits; without masking, stray padding bits silently corrupt
    /// `all_zero`, `add_assign`, `sub_assign`, `mul_assign`, and `fold_mul`.
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn mask_tail(&mut self) {
        let n_words = self.mag.len();
        if n_words == 0 {
            return;
        }
        let used = self.len_lanes - 64 * (n_words - 1);
        if used == 64 {
            return; // full word; no padding to mask
        }
        let mask = (1u64 << used) - 1;
        let last = n_words - 1;
        self.mag[last] &= mask;
        self.sgn[last] &= mask;
    }

    /// Reduce all `len_lanes` packed `F_3` elements to a single `Fp<3>` via
    /// the bipedal multiplication tree.
    ///
    /// The reduction applies the bipedal `mul` formula
    /// (`mag' = am & bm; sgn' = asg ^ bsg`) word-by-word to accumulate a
    /// 64-lane running product, then horizontally reduces those 64 lanes to
    /// a single scalar. Padding bits in the last word are set to the
    /// multiplicative identity `(mag=1, sgn=0)` so they do not perturb the
    /// result.
    ///
    /// An empty vector (`len_lanes == 0`) returns the multiplicative identity
    /// `Fp::<3>::new(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// // Product of [1, 2, 1] = 1*2*1 = 2 mod 3.
    /// let v = Bipedal3Vec::from_field_slice(&[
    ///     Fp::<3>::new(1), Fp::<3>::new(2), Fp::<3>::new(1),
    /// ]);
    /// assert_eq!(v.fold_mul(), Fp::<3>::new(2));
    ///
    /// // Empty product = multiplicative identity = 1.
    /// let empty = Bipedal3Vec::zeros(0);
    /// assert_eq!(empty.fold_mul(), Fp::<3>::new(1));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(len_lanes / 64))` word-level ops for the cross-word reduction,
    /// plus `O(64)` scalar lane decodes for the final horizontal reduction.
    pub fn fold_mul(&self) -> Fp<3> {
        if self.len_lanes == 0 {
            // Empty product = multiplicative identity.
            return Fp::<3>::new(1);
        }
        let n_words = self.mag.len();
        // Identity for paper mul: (mag=1, sgn=0) decodes to F_3 element 1.
        let mut acc_mag = u64::MAX;
        let mut acc_sgn = 0u64;

        // Full words (all 64 bits are active).
        for w in 0..n_words - 1 {
            acc_mag &= self.mag[w];
            acc_sgn ^= self.sgn[w];
        }

        // Last (possibly partial) word: set padding lanes to identity (mag=1, sgn=0).
        let used = self.len_lanes - 64 * (n_words - 1);
        let used_mask = if used == 64 {
            u64::MAX
        } else {
            (1u64 << used) - 1
        };
        // Padding lanes contribute mag=1 (identity) so they don't zero out acc_mag.
        let last_m = self.mag[n_words - 1] | !used_mask;
        // Padding lanes contribute sgn=0 (identity) so they don't perturb acc_sgn.
        let last_s = self.sgn[n_words - 1] & used_mask;
        acc_mag &= last_m;
        acc_sgn ^= last_s;

        // Horizontal reduce 64-lane (acc_mag, acc_sgn) to single Fp<3>.
        // Each lane contributes its decoded value to the running product.
        let mut result = Fp::<3>::new(1);
        for lane in 0..64u64 {
            let m = (acc_mag >> lane) & 1;
            let s = (acc_sgn >> lane) & 1;
            let v = if m == 0 {
                Fp::<3>::new(0)
            } else if s == 0 {
                Fp::<3>::new(1)
            } else {
                Fp::<3>::new(2)
            };
            result = result * v;
        }
        result
    }

    /// Lane-wise in-place additive inverse: `self[i] = -self[i]` for every `i`.
    ///
    /// Applies the bipedal `neg` formula per word: `mag' = mag; sgn' = sgn ^ mag`.
    /// `neg_assign` is **inherent on `Bipedal3Vec`**, not on `PackedFieldVec`,
    /// because the frozen `PackedFieldVec` trait surface (D1b §2.2) does not
    /// include a `neg_assign` method — the trait carries `add_assign`,
    /// `sub_assign`, `mul_assign`, and `all_zero` only, with negation expressed
    /// at the element level via `PackedField::neg`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut v = Bipedal3Vec::from_field_slice(&[
    ///     Fp::<3>::new(0), Fp::<3>::new(1), Fp::<3>::new(2),
    /// ]);
    /// v.neg_assign();
    /// assert_eq!(v.get(0), Fp::<3>::new(0));
    /// assert_eq!(v.get(1), Fp::<3>::new(2)); // -1 ≡ 2 mod 3
    /// assert_eq!(v.get(2), Fp::<3>::new(1)); // -2 ≡ 1 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))` word-level XOR operations.
    pub fn neg_assign(&mut self) {
        for w in 0..self.mag.len() {
            // Paper neg formula: mag stays, sgn XORed with mag.
            self.sgn[w] ^= self.mag[w];
        }
        self.mask_tail();
    }
}

// ---------------------------------------------------------------------------
// Manual PartialEq / Eq — canonical-decode equality
// ---------------------------------------------------------------------------

impl PartialEq for Bipedal3Vec {
    /// Canonical-decode equality: two vectors are equal iff they have the
    /// same `len_lanes` and every decoded lane is equal.
    ///
    /// Because the alternative-zero codeword `(mag=0, sgn=1)` decodes to 0,
    /// the `sgn` bit of a lane is irrelevant when its `mag` bit is 0.
    /// Concretely, per word `w`:
    ///   equal iff `self.mag[w] == other.mag[w]`
    ///          and `(self.sgn[w] ^ other.sgn[w]) & self.mag[w] == 0`
    ///
    /// The mask-tail invariant ensures padding bits are 0 on both sides,
    /// so the per-word test is safe.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(1), Fp::<3>::new(2)]);
    /// let b = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(1), Fp::<3>::new(2)]);
    /// assert_eq!(a, b);
    ///
    /// let c = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(0)]);
    /// assert_ne!(a, c); // different len_lanes
    /// ```
    fn eq(&self, other: &Self) -> bool {
        if self.len_lanes != other.len_lanes {
            return false;
        }
        for w in 0..self.mag.len() {
            if self.mag[w] != other.mag[w] {
                return false;
            }
            if (self.sgn[w] ^ other.sgn[w]) & self.mag[w] != 0 {
                return false;
            }
        }
        true
    }
}

impl Eq for Bipedal3Vec {}

// ---------------------------------------------------------------------------
// Manual Debug — print decoded lane values
// ---------------------------------------------------------------------------

impl fmt::Debug for Bipedal3Vec {
    /// Formats the value as a `Vec` of decoded lane values (each `0`, `1`,
    /// or `2`), matching the style of [`ScalarPackedFp3Vec`]'s `Debug`
    /// impl for stable `assert_eq!` messages.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(1), Fp::<3>::new(2)]);
    /// let s = format!("{:?}", v);
    /// assert!(s.contains("lanes"));
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lanes: Vec<u64> = (0..self.len_lanes)
            .map(|i| {
                let w = i >> 6;
                let b = i & 63;
                let m = (self.mag[w] >> b) & 1;
                let g = (self.sgn[w] >> b) & 1;
                if m == 0 {
                    0u64
                } else if g == 0 {
                    1u64
                } else {
                    2u64
                }
            })
            .collect();
        f.debug_struct("Bipedal3Vec")
            .field("lanes", &lanes)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// PackedFieldVec<Fp<3>>
// ---------------------------------------------------------------------------

impl PackedFieldVec<Fp<3>> for Bipedal3Vec {
    type Element = Bipedal3;

    /// Construct a vector of `len` zero `F_3` elements.
    ///
    /// # Arguments
    ///
    /// * `len` — number of logical `F_3` positions in the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    ///
    /// let v = Bipedal3Vec::zeros(65);
    /// assert_eq!(v.len(), 65);
    /// assert!(v.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(len / 64))`.
    fn zeros(len: usize) -> Self {
        let n_words = len.div_ceil(64);
        Self {
            mag: vec![0u64; n_words],
            sgn: vec![0u64; n_words],
            len_lanes: len,
        }
    }

    /// Construct a vector by encoding every element of `xs`.
    ///
    /// Position `i` is set to the canonical bipedal encoding of `xs[i]`.
    /// `mask_tail` is called defensively at the end to enforce the
    /// zero-padding invariant.
    ///
    /// # Arguments
    ///
    /// * `xs` — source slice; the result has `xs.len()` logical positions
    ///   and `get(i) == xs[i]` for every `i`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let xs = [Fp::<3>::new(0), Fp::<3>::new(1), Fp::<3>::new(2)];
    /// let v = Bipedal3Vec::from_field_slice(&xs);
    /// for i in 0..3 {
    ///     assert_eq!(v.get(i), xs[i]);
    /// }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(xs.len())`.
    fn from_field_slice(xs: &[Fp<3>]) -> Self {
        let len = xs.len();
        let n_words = len.div_ceil(64);
        let mut mag = vec![0u64; n_words];
        let mut sgn = vec![0u64; n_words];
        for (i, &x) in xs.iter().enumerate() {
            let v = x.value();
            let w = i >> 6;
            let b = i & 63;
            if v != 0 {
                mag[w] |= 1u64 << b;
            }
            if v == 2 {
                sgn[w] |= 1u64 << b;
            }
        }
        let mut result = Self {
            mag,
            sgn,
            len_lanes: len,
        };
        result.mask_tail();
        result
    }

    /// Number of logical `F_3` positions held by this vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    ///
    /// assert_eq!(Bipedal3Vec::zeros(100).len(), 100);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn len(&self) -> usize {
        self.len_lanes
    }

    /// Decode logical position `i` to a canonical `F_3` value.
    ///
    /// The alternative-zero codeword `(mag=0, sgn=1)` is canonicalised to
    /// `Fp::<3>::new(0)`.
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
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(2)]);
    /// assert_eq!(v.get(0), Fp::<3>::new(2));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn get(&self, i: usize) -> Fp<3> {
        assert!(
            i < self.len_lanes,
            "Bipedal3Vec::get: index {} out of range (len = {})",
            i,
            self.len_lanes
        );
        let w = i >> 6;
        let b = i & 63;
        let m = (self.mag[w] >> b) & 1;
        let g = (self.sgn[w] >> b) & 1;
        if m == 0 {
            Fp::<3>::new(0)
        } else if g == 0 {
            Fp::<3>::new(1)
        } else {
            Fp::<3>::new(2)
        }
    }

    /// Lane-wise in-place sum: `self[i] += rhs[i]` for every `i`.
    ///
    /// Applies the Scheinerman 2024 Algorithm 2 add formula per word:
    /// `t = am ^ asg ^ bsg; u = bm & t; mag' = u | (am ^ bm); sgn' = u ^ asg`
    ///
    /// # Arguments
    ///
    /// * `rhs` — operand of equal length; positions are added pointwise.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(1), Fp::<3>::new(2)]);
    /// let b = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(2), Fp::<3>::new(2)]);
    /// a.add_assign(&b);
    /// assert_eq!(a.get(0), Fp::<3>::new(0)); // 1 + 2 = 0 mod 3
    /// assert_eq!(a.get(1), Fp::<3>::new(1)); // 2 + 2 = 1 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn add_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Bipedal3Vec::add_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.mag.len() {
            let am = self.mag[w];
            let asg = self.sgn[w];
            let bm = rhs.mag[w];
            let bsg = rhs.sgn[w];
            // Scheinerman 2024 Algorithm 2 — 6 bitwise ops per word.
            let t = am ^ asg ^ bsg;
            let u = bm & t;
            self.mag[w] = u | (am ^ bm);
            self.sgn[w] = u ^ asg;
        }
        self.mask_tail();
    }

    /// Lane-wise in-place difference: `self[i] -= rhs[i]` for every `i`.
    ///
    /// Applies the canonical paper §2.2 sub formula per word:
    /// `t = asg ^ bsg; u = am & t; mag' = u | (am ^ bm); sgn' = u ^ (bm ^ bsg)`
    ///
    /// # Arguments
    ///
    /// * `rhs` — operand of equal length; subtracted pointwise from `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(0)]);
    /// let b = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(1)]);
    /// a.sub_assign(&b);
    /// assert_eq!(a.get(0), Fp::<3>::new(2)); // 0 - 1 = 2 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn sub_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Bipedal3Vec::sub_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.mag.len() {
            let am = self.mag[w];
            let asg = self.sgn[w];
            let bm = rhs.mag[w];
            let bsg = rhs.sgn[w];
            // Canonical paper §2.2 sub transliteration — 6 bitwise ops per word.
            let t = asg ^ bsg; // op 1
            let u = am & t; // op 2
            self.mag[w] = u | (am ^ bm); // ops 3+4
            self.sgn[w] = u ^ (bm ^ bsg); // ops 5+6
        }
        self.mask_tail();
    }

    /// Lane-wise in-place product: `self[i] *= rhs[i]` for every `i`.
    ///
    /// Applies the bipedal mul formula per word:
    /// `mag' = am & bm; sgn' = asg ^ bsg`
    ///
    /// # Arguments
    ///
    /// * `rhs` — operand of equal length; multiplied pointwise into `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(2)]);
    /// let b = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(2)]);
    /// a.mul_assign(&b);
    /// assert_eq!(a.get(0), Fp::<3>::new(1)); // 2 * 2 = 1 mod 3
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn mul_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Bipedal3Vec::mul_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.mag.len() {
            // Paper mul: 2 bitwise ops per word.
            self.mag[w] &= rhs.mag[w];
            self.sgn[w] ^= rhs.sgn[w];
        }
        self.mask_tail();
    }

    /// Returns `true` iff every logical position decodes to `F_3`'s
    /// additive identity (0).
    ///
    /// Implemented as `self.mag.iter().all(|&w| w == 0)`: a lane's value
    /// is zero iff its `mag` bit is 0 (the `sgn` bit is irrelevant when
    /// `mag=0`), so testing `mag` alone suffices. The alternative-zero
    /// codeword `(mag=0, sgn=1)` is correctly reported as zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Bipedal3Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// assert!(Bipedal3Vec::zeros(10).all_zero());
    /// let nz = Bipedal3Vec::from_field_slice(&[Fp::<3>::new(1)]);
    /// assert!(!nz.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn all_zero(&self) -> bool {
        self.mag.iter().all(|&w| w == 0)
    }
}

// ---------------------------------------------------------------------------
// Bipedal3Vec tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod vec_tests {
    use super::super::ScalarPackedFp3Vec;
    use super::*;
    use proptest::prelude::*;

    // (No shared helpers needed — proptest strategies are inlined below.)

    // -----------------------------------------------------------------------
    // zeros / all_zero / len — word-boundary lengths
    // -----------------------------------------------------------------------

    macro_rules! test_zeros {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let v = Bipedal3Vec::zeros($len);
                assert_eq!(v.len(), $len, "len mismatch for zeros({})", $len);
                assert!(v.all_zero(), "zeros({}) should be all_zero", $len);
                for i in 0..$len {
                    assert_eq!(v.get(i), Fp::<3>::new(0), "zeros({}).get({}) != 0", $len, i);
                }
            }
        };
    }

    #[test]
    fn test_zeros_0() {
        let v = Bipedal3Vec::zeros(0);
        assert_eq!(v.len(), 0);
        assert!(v.all_zero());
    }
    test_zeros!(test_zeros_1, 1);
    test_zeros!(test_zeros_63, 63);
    test_zeros!(test_zeros_64, 64);
    test_zeros!(test_zeros_65, 65);
    test_zeros!(test_zeros_127, 127);
    test_zeros!(test_zeros_128, 128);
    test_zeros!(test_zeros_129, 129);

    // -----------------------------------------------------------------------
    // from_field_slice round-trip
    // -----------------------------------------------------------------------

    macro_rules! test_from_field_slice {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                // Build a deterministic test vector: lane i = (i * 7) % 3
                let xs: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new((i * 7 % 3) as u64))
                    .collect();
                let v = Bipedal3Vec::from_field_slice(&xs);
                assert_eq!(v.len(), $len);
                for i in 0..$len {
                    assert_eq!(
                        v.get(i),
                        xs[i],
                        "from_field_slice({}).get({}) mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_from_field_slice_0() {
        let v = Bipedal3Vec::from_field_slice(&[]);
        assert_eq!(v.len(), 0);
        assert!(v.all_zero());
    }
    test_from_field_slice!(test_from_field_slice_1, 1);
    test_from_field_slice!(test_from_field_slice_63, 63);
    test_from_field_slice!(test_from_field_slice_64, 64);
    test_from_field_slice!(test_from_field_slice_65, 65);
    test_from_field_slice!(test_from_field_slice_127, 127);
    test_from_field_slice!(test_from_field_slice_128, 128);
    test_from_field_slice!(test_from_field_slice_129, 129);

    // -----------------------------------------------------------------------
    // add_assign — cross-check vs ScalarPackedFp3Vec
    // -----------------------------------------------------------------------

    macro_rules! test_add_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let a_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new((i * 3 % 3) as u64))
                    .collect();
                let b_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i + 1) % 3) as u64))
                    .collect();
                let mut a = Bipedal3Vec::from_field_slice(&a_vals);
                let b = Bipedal3Vec::from_field_slice(&b_vals);
                let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
                let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
                a.add_assign(&b);
                sa.add_assign(&sb);
                for i in 0..$len {
                    assert_eq!(
                        a.get(i),
                        sa.get(i),
                        "add_assign({}) lane {} mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_add_assign_0() {
        // len=0: both sides empty, no lanes to check, must not panic.
        let mut a = Bipedal3Vec::zeros(0);
        let b = Bipedal3Vec::zeros(0);
        a.add_assign(&b);
        assert_eq!(a.len(), 0);
    }
    test_add_assign!(test_add_assign_1, 1);
    test_add_assign!(test_add_assign_63, 63);
    test_add_assign!(test_add_assign_64, 64);
    test_add_assign!(test_add_assign_65, 65);
    test_add_assign!(test_add_assign_127, 127);
    test_add_assign!(test_add_assign_128, 128);
    test_add_assign!(test_add_assign_129, 129);

    // -----------------------------------------------------------------------
    // sub_assign — cross-check vs ScalarPackedFp3Vec
    // -----------------------------------------------------------------------

    macro_rules! test_sub_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let a_vals: Vec<Fp<3>> = (0..$len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
                let b_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i + 2) % 3) as u64))
                    .collect();
                let mut a = Bipedal3Vec::from_field_slice(&a_vals);
                let b = Bipedal3Vec::from_field_slice(&b_vals);
                let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
                let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
                a.sub_assign(&b);
                sa.sub_assign(&sb);
                for i in 0..$len {
                    assert_eq!(
                        a.get(i),
                        sa.get(i),
                        "sub_assign({}) lane {} mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_sub_assign_0() {
        let mut a = Bipedal3Vec::zeros(0);
        let b = Bipedal3Vec::zeros(0);
        a.sub_assign(&b);
        assert_eq!(a.len(), 0);
    }
    test_sub_assign!(test_sub_assign_1, 1);
    test_sub_assign!(test_sub_assign_63, 63);
    test_sub_assign!(test_sub_assign_64, 64);
    test_sub_assign!(test_sub_assign_65, 65);
    test_sub_assign!(test_sub_assign_127, 127);
    test_sub_assign!(test_sub_assign_128, 128);
    test_sub_assign!(test_sub_assign_129, 129);

    // -----------------------------------------------------------------------
    // mul_assign — cross-check vs ScalarPackedFp3Vec
    // -----------------------------------------------------------------------

    macro_rules! test_mul_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let a_vals: Vec<Fp<3>> = (0..$len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
                let b_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i + 1) % 3) as u64))
                    .collect();
                let mut a = Bipedal3Vec::from_field_slice(&a_vals);
                let b = Bipedal3Vec::from_field_slice(&b_vals);
                let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
                let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
                a.mul_assign(&b);
                sa.mul_assign(&sb);
                for i in 0..$len {
                    assert_eq!(
                        a.get(i),
                        sa.get(i),
                        "mul_assign({}) lane {} mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_mul_assign_0() {
        let mut a = Bipedal3Vec::zeros(0);
        let b = Bipedal3Vec::zeros(0);
        a.mul_assign(&b);
        assert_eq!(a.len(), 0);
    }
    test_mul_assign!(test_mul_assign_1, 1);
    test_mul_assign!(test_mul_assign_63, 63);
    test_mul_assign!(test_mul_assign_64, 64);
    test_mul_assign!(test_mul_assign_65, 65);
    test_mul_assign!(test_mul_assign_127, 127);
    test_mul_assign!(test_mul_assign_128, 128);
    test_mul_assign!(test_mul_assign_129, 129);

    // -----------------------------------------------------------------------
    // neg_assign — truth table + word-boundary cross-check vs negation formula
    // -----------------------------------------------------------------------

    #[test]
    fn test_neg_assign_truth_table() {
        // Build vec [0, 1, 2], negate, assert [0, 2, 1].
        let mut v =
            Bipedal3Vec::from_field_slice(&[Fp::<3>::new(0), Fp::<3>::new(1), Fp::<3>::new(2)]);
        v.neg_assign();
        assert_eq!(v.get(0), Fp::<3>::new(0)); // -0 = 0
        assert_eq!(v.get(1), Fp::<3>::new(2)); // -1 ≡ 2 mod 3
        assert_eq!(v.get(2), Fp::<3>::new(1)); // -2 ≡ 1 mod 3
    }

    macro_rules! test_neg_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                // Deterministic pattern: lane i = (i * 7 + 3) % 3.
                let vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i * 7 + 3) % 3) as u64))
                    .collect();
                let mut v = Bipedal3Vec::from_field_slice(&vals);
                v.neg_assign();
                for i in 0..$len {
                    let orig = vals[i].value();
                    let expected = if orig == 0 { 0 } else { 3 - orig };
                    assert_eq!(
                        v.get(i).value(),
                        expected,
                        "neg_assign({}) lane {} mismatch (orig={})",
                        $len,
                        i,
                        orig
                    );
                }
            }
        };
    }

    #[test]
    fn test_neg_assign_0() {
        // len=0: must not panic.
        let mut v = Bipedal3Vec::zeros(0);
        v.neg_assign();
        assert_eq!(v.len(), 0);
    }
    test_neg_assign!(test_neg_assign_1, 1);
    test_neg_assign!(test_neg_assign_63, 63);
    test_neg_assign!(test_neg_assign_64, 64);
    test_neg_assign!(test_neg_assign_65, 65);
    test_neg_assign!(test_neg_assign_127, 127);
    test_neg_assign!(test_neg_assign_128, 128);
    test_neg_assign!(test_neg_assign_129, 129);

    // -----------------------------------------------------------------------
    // mask_tail invariant — non-multiple-of-64 lengths
    // -----------------------------------------------------------------------

    /// Helper: check mask_tail invariant for a given partial-word length.
    fn check_mask_tail(len: usize) {
        assert!(
            !len.is_multiple_of(64),
            "only partial-word lengths have padding"
        );
        let used = len % 64;
        let used_mask: u64 = (1u64 << used) - 1;
        let padding_mask: u64 = !used_mask;

        let xs: Vec<Fp<3>> = (0..len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
        let ys: Vec<Fp<3>> = (0..len)
            .map(|i| Fp::<3>::new(((i + 1) % 3) as u64))
            .collect();

        // After from_field_slice
        let v = Bipedal3Vec::from_field_slice(&xs);
        let last = v.mag.len() - 1;
        assert_eq!(
            (v.mag[last] | v.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after from_field_slice (len={len})"
        );

        // After add_assign
        let mut a = v.clone();
        let b = Bipedal3Vec::from_field_slice(&ys);
        a.add_assign(&b);
        let last = a.mag.len() - 1;
        assert_eq!(
            (a.mag[last] | a.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after add_assign (len={len})"
        );

        // After sub_assign
        let mut c = Bipedal3Vec::from_field_slice(&xs);
        let d = Bipedal3Vec::from_field_slice(&ys);
        c.sub_assign(&d);
        let last = c.mag.len() - 1;
        assert_eq!(
            (c.mag[last] | c.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after sub_assign (len={len})"
        );

        // After mul_assign
        let mut e = Bipedal3Vec::from_field_slice(&xs);
        let f = Bipedal3Vec::from_field_slice(&ys);
        e.mul_assign(&f);
        let last = e.mag.len() - 1;
        assert_eq!(
            (e.mag[last] | e.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after mul_assign (len={len})"
        );

        // After neg_assign
        let mut g = Bipedal3Vec::from_field_slice(&xs);
        g.neg_assign();
        let last = g.mag.len() - 1;
        assert_eq!(
            (g.mag[last] | g.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after neg_assign (len={len})"
        );
    }

    #[test]
    fn test_mask_tail_invariant_1() {
        check_mask_tail(1);
    }
    #[test]
    fn test_mask_tail_invariant_63() {
        check_mask_tail(63);
    }
    #[test]
    fn test_mask_tail_invariant_65() {
        check_mask_tail(65);
    }
    #[test]
    fn test_mask_tail_invariant_127() {
        check_mask_tail(127);
    }
    #[test]
    fn test_mask_tail_invariant_129() {
        check_mask_tail(129);
    }

    // -----------------------------------------------------------------------
    // fold_mul
    // -----------------------------------------------------------------------

    #[test]
    fn test_fold_mul_empty() {
        let v = Bipedal3Vec::zeros(0);
        assert_eq!(
            v.fold_mul(),
            Fp::<3>::new(1),
            "fold_mul of empty vec must be 1 (multiplicative identity)"
        );
    }

    macro_rules! test_fold_mul {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let xs: Vec<Fp<3>> = (0..$len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
                let v = Bipedal3Vec::from_field_slice(&xs);
                let expected = (0..$len).fold(Fp::<3>::new(1), |acc, i| acc * v.get(i));
                assert_eq!(v.fold_mul(), expected, "fold_mul({}) mismatch", $len);
            }
        };
    }

    test_fold_mul!(test_fold_mul_1, 1);
    test_fold_mul!(test_fold_mul_7, 7);
    test_fold_mul!(test_fold_mul_64, 64);
    test_fold_mul!(test_fold_mul_100, 100);
    test_fold_mul!(test_fold_mul_200, 200);

    // -----------------------------------------------------------------------
    // Eq: alt-zero == canonical zero
    // -----------------------------------------------------------------------

    #[test]
    fn test_eq_alt_zero_vs_canonical() {
        // Length-5 vec: canonical zero
        let canon = Bipedal3Vec::zeros(5);
        // Manually construct alt-zero in lane 2: mag=0, sgn=1<<2
        // (mag is 0, sgn has bit 2 set — this is the alt-zero codeword)
        let mut alt = Bipedal3Vec::zeros(5);
        alt.sgn[0] = 1 << 2; // inject alt-zero at lane 2
        assert_eq!(canon, alt, "canonical zero and alt-zero must compare equal");
    }

    // -----------------------------------------------------------------------
    // get panics out of range
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_0() {
        let v = Bipedal3Vec::zeros(0);
        let _ = v.get(0);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_1() {
        let v = Bipedal3Vec::zeros(1);
        let _ = v.get(1);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_64() {
        let v = Bipedal3Vec::zeros(64);
        let _ = v.get(64);
    }

    // -----------------------------------------------------------------------
    // add_assign panics on length mismatch
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_add_assign_panics_on_length_mismatch() {
        let mut a = Bipedal3Vec::zeros(3);
        let b = Bipedal3Vec::zeros(4);
        a.add_assign(&b);
    }

    // -----------------------------------------------------------------------
    // Proptest cross-checks vs ScalarPackedFp3Vec
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, .. ProptestConfig::default() })]

        /// add_assign: Bipedal3Vec agrees with ScalarPackedFp3Vec lane-by-lane.
        #[test]
        fn test_proptest_add_assign_matches_scalar(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            // Truncate / extend to exactly `len`.
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a = Bipedal3Vec::from_field_slice(&a_vals);
            let b = Bipedal3Vec::from_field_slice(&b_vals);
            let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
            let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
            a.add_assign(&b);
            sa.add_assign(&sb);
            for i in 0..len {
                prop_assert_eq!(a.get(i), sa.get(i), "add_assign lane {} mismatch (len={})", i, len);
            }
        }

        /// sub_assign: Bipedal3Vec agrees with ScalarPackedFp3Vec lane-by-lane.
        #[test]
        fn test_proptest_sub_assign_matches_scalar(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a = Bipedal3Vec::from_field_slice(&a_vals);
            let b = Bipedal3Vec::from_field_slice(&b_vals);
            let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
            let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
            a.sub_assign(&b);
            sa.sub_assign(&sb);
            for i in 0..len {
                prop_assert_eq!(a.get(i), sa.get(i), "sub_assign lane {} mismatch (len={})", i, len);
            }
        }

        /// mul_assign: Bipedal3Vec agrees with ScalarPackedFp3Vec lane-by-lane.
        #[test]
        fn test_proptest_mul_assign_matches_scalar(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a = Bipedal3Vec::from_field_slice(&a_vals);
            let b = Bipedal3Vec::from_field_slice(&b_vals);
            let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
            let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
            a.mul_assign(&b);
            sa.mul_assign(&sb);
            for i in 0..len {
                prop_assert_eq!(a.get(i), sa.get(i), "mul_assign lane {} mismatch (len={})", i, len);
            }
        }

        /// fold_mul: Bipedal3Vec::fold_mul agrees with scalar per-lane product.
        #[test]
        fn test_proptest_fold_mul_matches_scalar_fold(
            len in 0usize..200,
            vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let vals: Vec<Fp<3>> = vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let v = Bipedal3Vec::from_field_slice(&vals);
            let expected = (0..len).fold(Fp::<3>::new(1), |acc, i| acc * v.get(i));
            prop_assert_eq!(v.fold_mul(), expected, "fold_mul mismatch (len={})", len);
        }
    }

    // -----------------------------------------------------------------------
    // Helpers for per-chunk (Bipedal3) cross-check proptests
    // -----------------------------------------------------------------------

    /// Decompose a `Bipedal3Vec` into per-word `(Bipedal3, used_lanes)` pairs.
    ///
    /// Each element is a `(Bipedal3, usize)` where the `usize` is the number
    /// of valid lanes in that chunk (always 64 except possibly the final chunk).
    fn chunks_of(v: &Bipedal3Vec) -> Vec<(Bipedal3, usize)> {
        let n_words = v.mag.len();
        if n_words == 0 {
            return Vec::new();
        }
        let mut chunks = Vec::with_capacity(n_words);
        for w in 0..n_words {
            let used = if w + 1 == n_words {
                // Last word: may be partial.
                v.len_lanes - 64 * w
            } else {
                64
            };
            chunks.push((Bipedal3::from_raw(v.mag[w], v.sgn[w]), used));
        }
        chunks
    }

    /// Recompose a sequence of `(Bipedal3, used_lanes)` chunks back into a
    /// `Vec<Fp<3>>` of length `total_len`.
    fn compose_chunks(chunks: &[(Bipedal3, usize)], total_len: usize) -> Vec<Fp<3>> {
        let mut out = Vec::with_capacity(total_len);
        for (chunk, used) in chunks {
            for lane in 0..*used {
                out.push(chunk.lane(lane));
            }
        }
        debug_assert_eq!(out.len(), total_len);
        out
    }

    // -----------------------------------------------------------------------
    // Proptest per-chunk cross-checks vs Bipedal3 chunk operations
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, .. ProptestConfig::default() })]

        /// add_assign: Bipedal3Vec direct path matches per-chunk Bipedal3::add.
        #[test]
        fn test_proptest_add_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            // Direct Bipedal3Vec path.
            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            let b_vec = Bipedal3Vec::from_field_slice(&b_vals);
            a_vec.add_assign(&b_vec);
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            // Chunked Bipedal3 path.
            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let b_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&b_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .zip(b_chunks.into_iter())
                .map(|((ac, used), (bc, _))| (ac.add(bc), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "add chunked vs vec mismatch (len={})", len);
        }

        /// sub_assign: Bipedal3Vec direct path matches per-chunk Bipedal3::sub.
        #[test]
        fn test_proptest_sub_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            // Direct Bipedal3Vec path.
            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            let b_vec = Bipedal3Vec::from_field_slice(&b_vals);
            a_vec.sub_assign(&b_vec);
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            // Chunked Bipedal3 path.
            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let b_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&b_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .zip(b_chunks.into_iter())
                .map(|((ac, used), (bc, _))| (ac.sub(bc), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "sub chunked vs vec mismatch (len={})", len);
        }

        /// mul_assign: Bipedal3Vec direct path matches per-chunk Bipedal3::mul.
        #[test]
        fn test_proptest_mul_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            // Direct Bipedal3Vec path.
            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            let b_vec = Bipedal3Vec::from_field_slice(&b_vals);
            a_vec.mul_assign(&b_vec);
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            // Chunked Bipedal3 path.
            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let b_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&b_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .zip(b_chunks.into_iter())
                .map(|((ac, used), (bc, _))| (ac.mul(bc), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "mul chunked vs vec mismatch (len={})", len);
        }

        /// neg_assign: Bipedal3Vec direct path matches per-chunk Bipedal3::neg.
        #[test]
        fn test_proptest_neg_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            // Direct Bipedal3Vec path.
            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            a_vec.neg_assign();
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            // Chunked Bipedal3 path.
            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .map(|(c, used)| (c.neg(), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "neg chunked vs vec mismatch (len={})", len);
        }
    }
}

// ===========================================================================
// Bipedal3Matrix — column-major rectangular matrix of packed F_3 values
// ===========================================================================

/// Rectangular `rows × cols` matrix of packed `F_3` values, stored
/// **column-major** as one [`Bipedal3Vec`] per column.
///
/// Each column `j` is a [`Bipedal3Vec`] of length `rows`; the entry at
/// row `i`, column `j` is `self.column(j).get(i)`.
///
/// # Column-major rationale
///
/// Ryser's formula (T7) and the single-word permanent path (T9) iterate
/// over columns in the inner loop, accumulating row-wise products.
/// Storing each column as a contiguous [`Bipedal3Vec`] allows those
/// algorithms to `column(j)` without scatter-gather, matching the access
/// pattern of the R3 multi-word streaming design
/// (`dev/plans/r3_multi_word_streaming.md` §2.1).
///
/// # Mask-tail invariant
///
/// Each column is a [`Bipedal3Vec`] and inherits its mask-tail invariant:
/// bits beyond `rows` in the last `u64` word of both `mag` and `sgn`
/// vectors are always zero (CLAUDE.md §Key design invariants #1).
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_core::gfp::Fp;
///
/// let data: Vec<Fp<3>> = (0..6u64).map(|v| Fp::<3>::new(v % 3)).collect();
/// let m = Bipedal3Matrix::from_row_major(&data, 2, 3);
/// assert_eq!(m.rows(), 2);
/// assert_eq!(m.cols(), 3);
/// assert_eq!(m.get(0, 0), Fp::<3>::new(0));
/// assert_eq!(m.get(1, 2), Fp::<3>::new(2));
/// ```
///
/// # Complexity
///
/// Construction is `O(rows * cols)`; column access is `O(1)`;
/// row reconstruction is `O(cols)`; transpose is `O(rows * cols)`.
#[derive(Clone)]
pub struct Bipedal3Matrix {
    /// One `Bipedal3Vec` per column, each of length `rows`.
    columns: Vec<Bipedal3Vec>,
    rows: usize,
    cols: usize,
}

// ---------------------------------------------------------------------------
// Manual PartialEq / Eq — shape + per-column canonical-decode equality
// ---------------------------------------------------------------------------

impl PartialEq for Bipedal3Matrix {
    /// Shape-equal and per-column canonical-decode equal.
    ///
    /// Two matrices are equal iff they have the same `rows` and `cols`,
    /// and every column pair compares equal under [`Bipedal3Vec`]'s
    /// canonical-decode `PartialEq` (which handles alternative-zero
    /// codewords transparently).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<3>> = (0..4u64).map(|v| Fp::<3>::new(v % 3)).collect();
    /// let a = Bipedal3Matrix::from_row_major(&data, 2, 2);
    /// let b = Bipedal3Matrix::from_row_major(&data, 2, 2);
    /// assert_eq!(a, b);
    ///
    /// let c = Bipedal3Matrix::from_row_major(&data, 4, 1);
    /// assert_ne!(a, c); // different shape
    /// ```
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows && self.cols == other.cols && self.columns == other.columns
    }
}

impl Eq for Bipedal3Matrix {}

// ---------------------------------------------------------------------------
// Manual Debug — print row-by-row for human readability
// ---------------------------------------------------------------------------

impl core::fmt::Debug for Bipedal3Matrix {
    /// Formats as `Bipedal3Matrix { rows, cols, data: [[row 0], [row 1], ...] }`
    /// with each row printed as a `Vec<u64>` of decoded lane values (`{0, 1, 2}`).
    ///
    /// Rows are listed top-to-bottom for human readability, even though the
    /// internal storage is column-major.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<3>> = vec![Fp::<3>::new(1), Fp::<3>::new(2)];
    /// let m = Bipedal3Matrix::from_row_major(&data, 1, 2);
    /// let s = format!("{:?}", m);
    /// assert!(s.contains("rows"));
    /// assert!(s.contains("cols"));
    /// ```
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Decode row by row so the output is human-readable top-to-bottom.
        let data: Vec<Vec<u64>> = (0..self.rows)
            .map(|i| {
                (0..self.cols)
                    .map(|j| self.columns[j].get(i).value())
                    .collect()
            })
            .collect();
        f.debug_struct("Bipedal3Matrix")
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("data", &data)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Bipedal3Matrix inherent methods
// ---------------------------------------------------------------------------

impl Bipedal3Matrix {
    /// Construct a matrix from a row-major `Fp<3>` slice.
    ///
    /// The entry at row `i`, column `j` is `data[i * cols + j]`. The slice
    /// is re-encoded in column-major order: each column `j` becomes a
    /// [`Bipedal3Vec`] of length `rows` containing `data[0*cols+j]`,
    /// `data[1*cols+j]`, ..., `data[(rows-1)*cols+j]`.
    ///
    /// Empty matrices (`rows == 0` or `cols == 0`) are allowed: they
    /// produce zero columns or zero-length columns respectively.
    ///
    /// # Arguments
    ///
    /// * `data` — row-major source slice of length `rows * cols`.
    /// * `rows` — number of rows.
    /// * `cols` — number of columns.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != rows * cols`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// // 2×3 matrix: [[0,1,2],[2,0,1]]
    /// let data: Vec<Fp<3>> = vec![
    ///     Fp::<3>::new(0), Fp::<3>::new(1), Fp::<3>::new(2),
    ///     Fp::<3>::new(2), Fp::<3>::new(0), Fp::<3>::new(1),
    /// ];
    /// let m = Bipedal3Matrix::from_row_major(&data, 2, 3);
    /// assert_eq!(m.rows(), 2);
    /// assert_eq!(m.cols(), 3);
    /// assert_eq!(m.get(0, 1), Fp::<3>::new(1));
    /// assert_eq!(m.get(1, 0), Fp::<3>::new(2));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(rows * cols)`.
    pub fn from_row_major(data: &[Fp<3>], rows: usize, cols: usize) -> Self {
        assert_eq!(
            data.len(),
            rows * cols,
            "Bipedal3Matrix::from_row_major: data.len() ({}) != rows ({}) * cols ({})",
            data.len(),
            rows,
            cols
        );
        // Build each column as a Bipedal3Vec.
        let columns: Vec<Bipedal3Vec> = (0..cols)
            .map(|j| {
                let col_data: Vec<Fp<3>> = (0..rows).map(|i| data[i * cols + j]).collect();
                Bipedal3Vec::from_field_slice(&col_data)
            })
            .collect();
        Self {
            columns,
            rows,
            cols,
        }
    }

    /// Inverse of [`from_row_major`][Self::from_row_major]: returns a row-major
    /// decoded `Vec<Fp<3>>` of length `rows * cols`.
    ///
    /// The entry at output index `i * cols + j` corresponds to matrix position
    /// `(i, j)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<3>> = (0..6u64).map(|v| Fp::<3>::new(v % 3)).collect();
    /// let m = Bipedal3Matrix::from_row_major(&data, 2, 3);
    /// let out = m.to_row_major();
    /// assert_eq!(out, data);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(rows * cols)`.
    pub fn to_row_major(&self) -> Vec<Fp<3>> {
        let mut out = Vec::with_capacity(self.rows * self.cols);
        for i in 0..self.rows {
            for j in 0..self.cols {
                out.push(self.columns[j].get(i));
            }
        }
        out
    }

    /// Number of rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = Bipedal3Matrix::from_row_major(&[], 0, 5);
    /// assert_eq!(m.rows(), 0);
    /// let m2 = Bipedal3Matrix::from_row_major(
    ///     &(0..10u64).map(|v| Fp::<3>::new(v % 3)).collect::<Vec<_>>(), 2, 5
    /// );
    /// assert_eq!(m2.rows(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = Bipedal3Matrix::from_row_major(&[], 5, 0);
    /// assert_eq!(m.cols(), 0);
    /// let m2 = Bipedal3Matrix::from_row_major(
    ///     &(0..10u64).map(|v| Fp::<3>::new(v % 3)).collect::<Vec<_>>(), 2, 5
    /// );
    /// assert_eq!(m2.cols(), 5);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Borrow the `j`-th column as a `&Bipedal3Vec` of length `rows`.
    ///
    /// This is the primary access pattern for column-major algorithms
    /// (Ryser T7, single-word permanent T9): iterating `column(j)` for
    /// `j` in `0..cols` is zero-copy.
    ///
    /// # Arguments
    ///
    /// * `j` — column index in `0..self.cols()`.
    ///
    /// # Panics
    ///
    /// Panics if `j >= self.cols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{Bipedal3Matrix, PackedFieldVec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<3>> = vec![
    ///     Fp::<3>::new(1), Fp::<3>::new(2),
    ///     Fp::<3>::new(0), Fp::<3>::new(1),
    /// ];
    /// let m = Bipedal3Matrix::from_row_major(&data, 2, 2);
    /// // Column 1: entries (0,1)=2 and (1,1)=1.
    /// assert_eq!(m.column(1).get(0), Fp::<3>::new(2));
    /// assert_eq!(m.column(1).get(1), Fp::<3>::new(1));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn column(&self, j: usize) -> &Bipedal3Vec {
        assert!(
            j < self.cols,
            "Bipedal3Matrix::column: index {} out of range (cols = {})",
            j,
            self.cols
        );
        &self.columns[j]
    }

    /// Reconstruct the `i`-th row as an owned `Bipedal3Vec` of length `cols`.
    ///
    /// Lane `j` of the returned vector equals `self.column(j).get(i)`.
    /// Row access requires reading from each column, so it is `O(cols)`.
    ///
    /// # Arguments
    ///
    /// * `i` — row index in `0..self.rows()`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.rows()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{Bipedal3Matrix, PackedFieldVec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<3>> = vec![
    ///     Fp::<3>::new(1), Fp::<3>::new(2), Fp::<3>::new(0),
    ///     Fp::<3>::new(2), Fp::<3>::new(0), Fp::<3>::new(1),
    /// ];
    /// let m = Bipedal3Matrix::from_row_major(&data, 2, 3);
    /// // Row 1: [2, 0, 1]
    /// let row1 = m.row(1);
    /// assert_eq!(row1.get(0), Fp::<3>::new(2));
    /// assert_eq!(row1.get(1), Fp::<3>::new(0));
    /// assert_eq!(row1.get(2), Fp::<3>::new(1));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(cols)`.
    pub fn row(&self, i: usize) -> Bipedal3Vec {
        assert!(
            i < self.rows,
            "Bipedal3Matrix::row: index {} out of range (rows = {})",
            i,
            self.rows
        );
        let row_data: Vec<Fp<3>> = (0..self.cols).map(|j| self.columns[j].get(i)).collect();
        Bipedal3Vec::from_field_slice(&row_data)
    }

    /// Read the entry at row `i`, column `j`.
    ///
    /// # Arguments
    ///
    /// * `i` — row index in `0..self.rows()`.
    /// * `j` — column index in `0..self.cols()`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.rows()` or `j >= self.cols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<3>> = vec![
    ///     Fp::<3>::new(0), Fp::<3>::new(1),
    ///     Fp::<3>::new(2), Fp::<3>::new(0),
    /// ];
    /// let m = Bipedal3Matrix::from_row_major(&data, 2, 2);
    /// assert_eq!(m.get(0, 0), Fp::<3>::new(0));
    /// assert_eq!(m.get(0, 1), Fp::<3>::new(1));
    /// assert_eq!(m.get(1, 0), Fp::<3>::new(2));
    /// assert_eq!(m.get(1, 1), Fp::<3>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn get(&self, i: usize, j: usize) -> Fp<3> {
        assert!(
            i < self.rows,
            "Bipedal3Matrix::get: row index {} out of range (rows = {})",
            i,
            self.rows
        );
        assert!(
            j < self.cols,
            "Bipedal3Matrix::get: col index {} out of range (cols = {})",
            j,
            self.cols
        );
        self.columns[j].get(i)
    }

    /// Transpose: returns a `cols × rows` matrix where `transposed.get(j, i) == self.get(i, j)`.
    ///
    /// The result is built by materialising a row-major `Vec<Fp<3>>` buffer
    /// via [`to_row_major`][Self::to_row_major] and then transposing the
    /// index mapping before calling [`from_row_major`][Self::from_row_major]
    /// with swapped dimensions. This is obviously correct and `O(rows * cols)`.
    ///
    /// A performance-optimised path (direct column-to-row scatter) may be
    /// added in a later task if profiling identifies the `to_row_major`
    /// intermediary as a bottleneck.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Bipedal3Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<3>> = vec![
    ///     Fp::<3>::new(1), Fp::<3>::new(2), Fp::<3>::new(0),
    ///     Fp::<3>::new(2), Fp::<3>::new(0), Fp::<3>::new(1),
    /// ];
    /// let m = Bipedal3Matrix::from_row_major(&data, 2, 3);
    /// let t = m.transpose();
    /// assert_eq!(t.rows(), 3);
    /// assert_eq!(t.cols(), 2);
    /// // t.get(j, i) == m.get(i, j)
    /// assert_eq!(t.get(2, 0), m.get(0, 2));
    /// assert_eq!(t.get(0, 1), m.get(1, 0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(rows * cols)`.
    pub fn transpose(&self) -> Self {
        let rm = self.to_row_major();
        let mut tm = Vec::with_capacity(self.cols * self.rows);
        for j in 0..self.cols {
            for i in 0..self.rows {
                tm.push(rm[i * self.cols + j]);
            }
        }
        Self::from_row_major(&tm, self.cols, self.rows)
    }
}

// ---------------------------------------------------------------------------
// Bipedal3Matrix tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod matrix_tests {
    use super::*;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Deterministic entry pattern helper
    // -----------------------------------------------------------------------

    /// Build a deterministic `Vec<Fp<3>>` of `rows * cols` values.
    /// Entry `(i, j)` = `((i * 7 + j * 11 + 5) as u64) % 3`.
    fn det_data(rows: usize, cols: usize) -> Vec<Fp<3>> {
        (0..rows)
            .flat_map(|i| (0..cols).map(move |j| Fp::<3>::new(((i * 7 + j * 11 + 5) as u64) % 3)))
            .collect()
    }

    // -----------------------------------------------------------------------
    // from_row_major / to_row_major round-trip
    // -----------------------------------------------------------------------

    macro_rules! test_roundtrip {
        ($name:ident, $rows:expr, $cols:expr) => {
            #[test]
            fn $name() {
                let data = det_data($rows, $cols);
                let m = Bipedal3Matrix::from_row_major(&data, $rows, $cols);
                assert_eq!(m.rows(), $rows);
                assert_eq!(m.cols(), $cols);
                let out = m.to_row_major();
                assert_eq!(out, data, "roundtrip mismatch for {}x{}", $rows, $cols);
            }
        };
    }

    #[test]
    fn test_from_row_major_to_row_major_roundtrip_0x0() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 0);
        assert_eq!(m.rows(), 0);
        assert_eq!(m.cols(), 0);
        assert!(m.to_row_major().is_empty());
    }

    #[test]
    fn test_from_row_major_to_row_major_roundtrip_0x5() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 5);
        assert_eq!(m.rows(), 0);
        assert_eq!(m.cols(), 5);
        assert!(m.to_row_major().is_empty());
    }

    #[test]
    fn test_from_row_major_to_row_major_roundtrip_5x0() {
        let m = Bipedal3Matrix::from_row_major(&[], 5, 0);
        assert_eq!(m.rows(), 5);
        assert_eq!(m.cols(), 0);
        assert!(m.to_row_major().is_empty());
    }

    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_1x1, 1, 1);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_1x64, 1, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x1, 64, 1);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_63x63, 63, 63);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_63x64, 63, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x63, 64, 63);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x64, 64, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x65, 64, 65);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_65x64, 65, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_65x65, 65, 65);

    // -----------------------------------------------------------------------
    // get — per-entry accessor
    // -----------------------------------------------------------------------

    macro_rules! test_get {
        ($name:ident, $rows:expr, $cols:expr) => {
            #[test]
            fn $name() {
                let data = det_data($rows, $cols);
                let m = Bipedal3Matrix::from_row_major(&data, $rows, $cols);
                for i in 0..$rows {
                    for j in 0..$cols {
                        assert_eq!(
                            m.get(i, j),
                            data[i * $cols + j],
                            "get({},{}) mismatch for {}x{}",
                            i,
                            j,
                            $rows,
                            $cols
                        );
                    }
                }
            }
        };
    }

    test_get!(test_get_1x1, 1, 1);
    test_get!(test_get_1x64, 1, 64);
    test_get!(test_get_64x1, 64, 1);
    test_get!(test_get_63x63, 63, 63);
    test_get!(test_get_63x64, 63, 64);
    test_get!(test_get_64x63, 64, 63);
    test_get!(test_get_64x64, 64, 64);
    test_get!(test_get_65x65, 65, 65);

    // -----------------------------------------------------------------------
    // column and row accessor consistency
    // -----------------------------------------------------------------------

    #[test]
    fn test_column_returns_correct_vec() {
        let data = det_data(5, 3);
        let m = Bipedal3Matrix::from_row_major(&data, 5, 3);
        for j in 0..3 {
            for i in 0..5 {
                assert_eq!(
                    m.column(j).get(i),
                    m.get(i, j),
                    "column({}).get({}) != get({}, {})",
                    j,
                    i,
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_row_returns_correct_vec() {
        let data = det_data(5, 3);
        let m = Bipedal3Matrix::from_row_major(&data, 5, 3);
        for i in 0..5 {
            let row_vec = m.row(i);
            for j in 0..3 {
                assert_eq!(
                    row_vec.get(j),
                    m.get(i, j),
                    "row({}).get({}) != get({}, {})",
                    i,
                    j,
                    i,
                    j
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Panic tests
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_row() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.get(3, 0);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_col() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.get(0, 4);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_column_panics_out_of_range() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.column(4);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_row_panics_out_of_range() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.row(3);
    }

    #[test]
    #[should_panic(expected = "rows")]
    fn test_from_row_major_panics_on_length_mismatch() {
        // data.len() = 5 != 2*3 = 6
        let data: Vec<Fp<3>> = (0..5u64).map(|v| Fp::<3>::new(v % 3)).collect();
        let _ = Bipedal3Matrix::from_row_major(&data, 2, 3);
    }

    // -----------------------------------------------------------------------
    // Transpose tests
    // -----------------------------------------------------------------------

    macro_rules! test_transpose_roundtrip {
        ($name:ident, $rows:expr, $cols:expr) => {
            #[test]
            fn $name() {
                let data = det_data($rows, $cols);
                let m = Bipedal3Matrix::from_row_major(&data, $rows, $cols);
                let tt = m.transpose().transpose();
                assert_eq!(
                    m, tt,
                    "transpose().transpose() != self for {}x{}",
                    $rows, $cols
                );
            }
        };
    }

    test_transpose_roundtrip!(test_transpose_roundtrip_1x1, 1, 1);
    test_transpose_roundtrip!(test_transpose_roundtrip_5x7, 5, 7);
    test_transpose_roundtrip!(test_transpose_roundtrip_63x65, 63, 65);
    test_transpose_roundtrip!(test_transpose_roundtrip_64x64, 64, 64);
    test_transpose_roundtrip!(test_transpose_roundtrip_64x100, 64, 100);
    test_transpose_roundtrip!(test_transpose_roundtrip_130x17, 130, 17);

    #[test]
    fn test_transpose_value_check_5x3() {
        let data = det_data(5, 3);
        let m = Bipedal3Matrix::from_row_major(&data, 5, 3);
        let t = m.transpose();
        assert_eq!(t.rows(), 3);
        assert_eq!(t.cols(), 5);
        for i in 0..5 {
            for j in 0..3 {
                assert_eq!(
                    t.get(j, i),
                    m.get(i, j),
                    "transpose value mismatch at (i={}, j={}): t.get({},{})={:?}, m.get({},{})={:?}",
                    i, j, j, i, t.get(j, i), i, j, m.get(i, j)
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Proptest: double-transpose roundtrip with random shapes (100 cases)
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        /// transpose().transpose() == self for random shapes and random data.
        ///
        /// Generates `rows ∈ 0..130`, `cols ∈ 0..130`, and a random seed to
        /// build a deterministic `Vec<Fp<3>>` of `rows * cols` values.
        /// The double-transpose must equal the original matrix.
        #[test]
        fn test_proptest_transpose_roundtrip_random_shapes(
            rows in 0usize..130,
            cols in 0usize..130,
            seed in 0u64..u64::MAX,
        ) {
            // Build a seeded-deterministic data vector.
            let n = rows * cols;
            let data: Vec<Fp<3>> = (0..n)
                .map(|k| {
                    // Mix seed with index using a simple hash.
                    let h = seed.wrapping_mul(6364136223846793005)
                        .wrapping_add(k as u64)
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    Fp::<3>::new(h % 3)
                })
                .collect();
            let m = Bipedal3Matrix::from_row_major(&data, rows, cols);
            let tt = m.transpose().transpose();
            prop_assert_eq!(m, tt, "transpose roundtrip failed for {}x{}", rows, cols);
        }
    }

    // -----------------------------------------------------------------------
    // Word-boundary coverage confirmation
    //
    // The round-trip macro tests above already cover all word-boundary
    // leg values {1, 63, 64, 65} for both rows and cols via the combinations:
    //   1×1, 1×64, 64×1, 63×63, 63×64, 64×63, 64×64, 64×65, 65×64, 65×65.
    // The transpose tests add: 63×65, 64×100, 130×17.
    // No additional tests are needed to satisfy criterion 5.
    // -----------------------------------------------------------------------
}
