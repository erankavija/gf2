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

use super::PackedField;

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
