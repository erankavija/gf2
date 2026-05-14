//! Fixed-width and variable-length packed `F_5` encoding — Candidate D.
//!
//! [`Packed5`] packs exactly **64** independent `F_5` lanes into three
//! `u64` bit-planes `(b0, b1, b2)`, carrying the canonical 3-bit value of
//! each `F_5` element. One `u64`-triple covers 64 elements.
//!
//! # Encoding
//!
//! Each `F_5` element `x ∈ {0, 1, 2, 3, 4}` is stored as a 3-bit canonical
//! value across the three bit-planes:
//!
//! | `x` | `b2` bit | `b1` bit | `b0` bit |
//! |-----|----------|----------|----------|
//! |  0  |    0     |    0     |    0     |
//! |  1  |    0     |    0     |    1     |
//! |  2  |    0     |    1     |    0     |
//! |  3  |    0     |    1     |    1     |
//! |  4  |    1     |    0     |    0     |
//!
//! Codepoints `5..=7` are redundant and never produced by canonical packings.
//! Decoding a non-canonical codepoint maps to 0 — the encode-decode is
//! total/well-defined.
//!
//! # Algorithm (R1 §5)
//!
//! Every binary op uses a **5-way decode** of each operand into mutually-
//! exclusive selectors `e_0..e_4` (where `e_i = 1` iff the element equals
//! `i`), followed by a **5×5 cross-product** that gates `e_a[i] & e_b[j]`
//! into per-result selectors `r_0..r_4`, then an **encode** that combines
//! `r_0..r_4` into output bit-planes `(c0, c1, c2)`.
//!
//! - Decode (per operand): 11 ops (3 NOTs + 8 ANDs). 22 for both.
//! - Cross-product: 20 ANDs for add/sub, 16 for mul.
//! - Result-tree ORs: 16 for add/sub, 12 for mul.
//! - Encode: 2 ORs (`c0 = r1 | r3`, `c1 = r2 | r3`, `c2 = r4`).
//! - Total: 60 ops/`u64`-triple for add/sub, 52 for mul.
//!
//! `neg` is a unary op (single-operand 1×5 remap, not a cross-product)
//! implemented as decode → permute selectors `(e_0, e_1, e_2, e_3, e_4)`
//! → `(e_0, e_4, e_3, e_2, e_1)` for the `(5 − x) mod 5` table → encode.
//!
//! # Transliteration source
//!
//! `dev/research/f5_packing/src/cand_d.rs` — the reference prototype.
//! The decision doc is `dev/plans/r1_f5_encoding_decision.md` (W4, T17).
//!
//! # Feature gating
//!
//! Compiled when the `f5` Cargo feature is enabled. See `Cargo.toml`.

use core::fmt;

use gf2_core::gfp::Fp;

use super::{PackedField, PackedFieldVec};

// ---------------------------------------------------------------------------
// Internal decode / encode / apply helpers
// ---------------------------------------------------------------------------

/// Decode a single `(b0, b1, b2)` operand word-triple into 5 mutually-
/// exclusive selectors `e[0]..e[4]`.
///
/// The decode uses 3 NOTs and 8 ANDs (11 ops, shared sub-expressions).
/// A lane carries `e[i] = 1` iff that lane's canonical value equals `i`.
///
/// Canonical values 0..=4 produce exactly one hot selector bit per lane;
/// redundant codepoints 5..=7 map to all-zero (treated as 0 at decode time).
#[inline]
fn decode5(b0: u64, b1: u64, b2: u64) -> [u64; 5] {
    let n0 = !b0;
    let n1 = !b1;
    let n2 = !b2;
    let n2n1 = n2 & n1;
    let n2_1 = n2 & b1;
    let n1n0 = n1 & n0;
    let e0 = n2n1 & n0;
    let e1 = n2n1 & b0;
    let e2 = n2_1 & n0;
    let e3 = n2_1 & b0;
    let e4 = b2 & n1n0;
    [e0, e1, e2, e3, e4]
}

/// Encode per-result selectors `r[0]..r[4]` into output bit-planes `(c0, c1, c2)`.
///
/// Encoding per the 3-bit canonical mapping:
/// - `c0 = r[1] | r[3]` (b0 bit is set for values 1 and 3)
/// - `c1 = r[2] | r[3]` (b1 bit is set for values 2 and 3)
/// - `c2 = r[4]`        (b2 bit is set for value 4)
#[inline]
fn encode5(r: [u64; 5]) -> (u64, u64, u64) {
    let c0 = r[1] | r[3];
    let c1 = r[2] | r[3];
    let c2 = r[4];
    (c0, c1, c2)
}

/// F_5 addition Boolean circuit — direct transliteration of R1 §5 ADD cells.
///
/// Cross-product cells (i + j) mod 5 == k for k ∈ {1,2,3,4}:
/// - r[1]: (0,1),(1,0),(2,4),(3,3),(4,2) — 5 ANDs + 4 ORs
/// - r[2]: (0,2),(1,1),(2,0),(3,4),(4,3) — 5 ANDs + 4 ORs
/// - r[3]: (0,3),(1,2),(2,1),(3,0),(4,4) — 5 ANDs + 4 ORs
/// - r[4]: (0,4),(1,3),(2,2),(3,1),(4,0) — 5 ANDs + 4 ORs
///
/// Total: 22 decode + 20 ANDs + 16 ORs + 2 encode ORs = **60 ops** per u64-triple.
#[inline]
fn add_circuit(ea: [u64; 5], eb: [u64; 5]) -> (u64, u64, u64) {
    let r1 =
        (ea[0] & eb[1]) | (ea[1] & eb[0]) | (ea[2] & eb[4]) | (ea[3] & eb[3]) | (ea[4] & eb[2]);
    let r2 =
        (ea[0] & eb[2]) | (ea[1] & eb[1]) | (ea[2] & eb[0]) | (ea[3] & eb[4]) | (ea[4] & eb[3]);
    let r3 =
        (ea[0] & eb[3]) | (ea[1] & eb[2]) | (ea[2] & eb[1]) | (ea[3] & eb[0]) | (ea[4] & eb[4]);
    let r4 =
        (ea[0] & eb[4]) | (ea[1] & eb[3]) | (ea[2] & eb[2]) | (ea[3] & eb[1]) | (ea[4] & eb[0]);
    encode5([0, r1, r2, r3, r4])
}

/// F_5 subtraction Boolean circuit — direct transliteration of R1 §5 SUB cells.
///
/// Cross-product cells (i - j + 5) mod 5 == k for k ∈ {1,2,3,4}:
/// - r[1]: (0,4),(1,0),(2,1),(3,2),(4,3) — 5 ANDs + 4 ORs
/// - r[2]: (0,3),(1,4),(2,0),(3,1),(4,2) — 5 ANDs + 4 ORs
/// - r[3]: (0,2),(1,3),(2,4),(3,0),(4,1) — 5 ANDs + 4 ORs
/// - r[4]: (0,1),(1,2),(2,3),(3,4),(4,0) — 5 ANDs + 4 ORs
///
/// Total: 22 decode + 20 ANDs + 16 ORs + 2 encode ORs = **60 ops** per u64-triple.
#[inline]
fn sub_circuit(ea: [u64; 5], eb: [u64; 5]) -> (u64, u64, u64) {
    let r1 =
        (ea[0] & eb[4]) | (ea[1] & eb[0]) | (ea[2] & eb[1]) | (ea[3] & eb[2]) | (ea[4] & eb[3]);
    let r2 =
        (ea[0] & eb[3]) | (ea[1] & eb[4]) | (ea[2] & eb[0]) | (ea[3] & eb[1]) | (ea[4] & eb[2]);
    let r3 =
        (ea[0] & eb[2]) | (ea[1] & eb[3]) | (ea[2] & eb[4]) | (ea[3] & eb[0]) | (ea[4] & eb[1]);
    let r4 =
        (ea[0] & eb[1]) | (ea[1] & eb[2]) | (ea[2] & eb[3]) | (ea[3] & eb[4]) | (ea[4] & eb[0]);
    encode5([0, r1, r2, r3, r4])
}

/// F_5 multiplication Boolean circuit — direct transliteration of R1 §5 MUL cells.
///
/// Cross-product cells (i * j) mod 5 == k for k ∈ {1,2,3,4}
/// (cells with i=0 or j=0 always yield 0, so they go to r[0] which is unused):
/// - r[1]: (1,1),(2,3),(3,2),(4,4) — 4 ANDs + 3 ORs
/// - r[2]: (1,2),(2,1),(3,4),(4,3) — 4 ANDs + 3 ORs
/// - r[3]: (1,3),(2,4),(3,1),(4,2) — 4 ANDs + 3 ORs
/// - r[4]: (1,4),(2,2),(3,3),(4,1) — 4 ANDs + 3 ORs
///
/// Total: 22 decode + 16 ANDs + 12 ORs + 2 encode ORs = **52 ops** per u64-triple.
#[inline]
fn mul_circuit(ea: [u64; 5], eb: [u64; 5]) -> (u64, u64, u64) {
    let r1 = (ea[1] & eb[1]) | (ea[2] & eb[3]) | (ea[3] & eb[2]) | (ea[4] & eb[4]);
    let r2 = (ea[1] & eb[2]) | (ea[2] & eb[1]) | (ea[3] & eb[4]) | (ea[4] & eb[3]);
    let r3 = (ea[1] & eb[3]) | (ea[2] & eb[4]) | (ea[3] & eb[1]) | (ea[4] & eb[2]);
    let r4 = (ea[1] & eb[4]) | (ea[2] & eb[2]) | (ea[3] & eb[3]) | (ea[4] & eb[1]);
    encode5([0, r1, r2, r3, r4])
}

// ---------------------------------------------------------------------------
// Packed5 — fixed-width 64-lane F_5 encoding
// ---------------------------------------------------------------------------

/// Fixed-width packed `F_5` element encoding 64 lanes in three `u64`
/// bit-planes `(b0, b1, b2)` using the bit-sliced Boolean circuit of R1
/// Candidate D.
///
/// Each lane `i` (0 ≤ `i` < 64) stores one `F_5` element as the 3-bit
/// canonical value across bit positions `i` of `(b0, b1, b2)`:
///
/// | `F_5` value | `b2` bit | `b1` bit | `b0` bit |
/// |-------------|----------|----------|----------|
/// |      0      |    0     |    0     |    0     |
/// |      1      |    0     |    0     |    1     |
/// |      2      |    0     |    1     |    0     |
/// |      3      |    0     |    1     |    1     |
/// |      4      |    1     |    0     |    0     |
///
/// Codepoints `5..=7` are redundant and never produced by arithmetic ops.
/// Decoding any non-canonical codepoint returns 0.
///
/// # Algorithm
///
/// All binary ops use a 5-way decode-then-cross-product Boolean circuit
/// derived from the 5×5 `F_5` truth tables (R1 §5). No LUT, no runtime
/// tables, no `OnceLock`, no `unsafe`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedField, Packed5};
/// use gf2_core::gfp::Fp;
///
/// let a = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(2));
/// let b = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
/// let s = a.add(b);
/// assert_eq!(s.lane(0), Fp::<5>::new(0)); // 2 + 3 == 0 mod 5
/// assert_eq!(s.lane(63), Fp::<5>::new(0));
/// ```
///
/// # Complexity
///
/// All operations are `O(1)` — a fixed number of word-level bitwise
/// instructions independent of the number of lanes.
#[derive(Copy, Clone)]
pub struct Packed5 {
    b0: u64,
    b1: u64,
    b2: u64,
}

// ---------------------------------------------------------------------------
// Manual PartialEq / Eq — canonical-decode equality.
//
// Two `Packed5` values are equal iff every lane decodes to the same F_5
// value. We compare decoded selector arrays: `decode5` maps all redundant
// codepoints (5..=7) to the same all-zeros result, so two words have equal
// decoded lanes iff their selector arrays match.
// ---------------------------------------------------------------------------

impl PartialEq for Packed5 {
    /// Canonical-decode equality: two values are equal iff every decoded
    /// lane is equal.
    ///
    /// Non-canonical codepoints (5..=7) in either operand are decoded to 0
    /// before comparison via `decode5`, satisfying the trait contract
    /// (D1b §3.4 and mod.rs §PackedField::eq).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{Packed5, PackedField};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
    /// let b = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
    /// assert_eq!(a, b);
    ///
    /// let c = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(2));
    /// assert_ne!(a, c);
    /// ```
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Canonical-decode equality: two lanes are equal iff they decode to
        // the same F_5 value (0..=4). Redundant codepoints (5..=7) decode to 0.
        //
        // decode5 maps redundant codepoints to all-zero selectors (e[0..4] = 0),
        // whereas canonical 0 gives e[0]=1, e[1..4]=0. So we cannot compare the
        // full selector array. Instead, we compare only e[1..4]: two lanes are
        // equal iff they agree on which of {1,2,3,4} is selected. Zero (canonical
        // or redundant) has e[1..4]=0 in both cases.
        let sa = decode5(self.b0, self.b1, self.b2);
        let sb = decode5(other.b0, other.b1, other.b2);
        sa[1] == sb[1] && sa[2] == sb[2] && sa[3] == sb[3] && sa[4] == sb[4]
    }
}

impl Eq for Packed5 {}

// ---------------------------------------------------------------------------
// Manual Hash — consistent with PartialEq (canonical-decode).
// ---------------------------------------------------------------------------

impl core::hash::Hash for Packed5 {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Hash the decoded lane values to be consistent with Eq.
        for i in 0..64usize {
            self.lane(i).value().hash(state);
        }
    }
}

// ---------------------------------------------------------------------------
// Default — all-zero.
// ---------------------------------------------------------------------------

impl Default for Packed5 {
    fn default() -> Self {
        Self::zero()
    }
}

// ---------------------------------------------------------------------------
// Manual Debug — print lane values (0..=4).
// ---------------------------------------------------------------------------

impl fmt::Debug for Packed5 {
    /// Formats the value as a 64-element array of decoded lane values
    /// (each in `{0, 1, 2, 3, 4}`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed5;
    /// use gf2_algebra::packed::PackedField;
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
    /// let s = format!("{:?}", v);
    /// assert!(s.contains("lanes"));
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lanes = core::array::from_fn::<u64, 64, _>(|i| self.lane(i).value());
        f.debug_struct("Packed5").field("lanes", &lanes).finish()
    }
}

// ---------------------------------------------------------------------------
// PackedField<Fp<5>>
// ---------------------------------------------------------------------------

impl PackedField<Fp<5>> for Packed5 {
    /// Number of independent `F_5` lanes packed into one `Packed5`.
    ///
    /// Fixed at 64 to match the `u64`-triple encoding width.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    /// assert_eq!(<Packed5 as PackedField<Fp<5>>>::LANES, 64);
    /// ```
    const LANES: usize = 64;

    /// Returns the all-zeros `Packed5` (every lane = 0).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let z = <Packed5 as PackedField<Fp<5>>>::zero();
    /// assert!(z.all_zero());
    /// for i in 0..64 { assert_eq!(z.lane(i), Fp::<5>::new(0)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn zero() -> Self {
        Self {
            b0: 0,
            b1: 0,
            b2: 0,
        }
    }

    /// Returns the all-ones `Packed5` (every lane = 1).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let o = <Packed5 as PackedField<Fp<5>>>::one();
    /// for i in 0..64 { assert_eq!(o.lane(i), Fp::<5>::new(1)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn one() -> Self {
        // Value 1: b2=0, b1=0, b0=1
        Self {
            b0: u64::MAX,
            b1: 0,
            b2: 0,
        }
    }

    /// Broadcasts scalar `x` to all 64 lanes.
    ///
    /// # Arguments
    ///
    /// * `x` — scalar `F_5` value to replicate across all lanes.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
    /// for i in 0..64 { assert_eq!(v.lane(i), Fp::<5>::new(3)); }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    fn splat(x: Fp<5>) -> Self {
        let v = x.value();
        // Canonical encoding: b0 = bit 0, b1 = bit 1, b2 = bit 2.
        let b0_bit = if (v & 1) != 0 { u64::MAX } else { 0 };
        let b1_bit = if (v & 2) != 0 { u64::MAX } else { 0 };
        let b2_bit = if (v & 4) != 0 { u64::MAX } else { 0 };
        Self {
            b0: b0_bit,
            b1: b1_bit,
            b2: b2_bit,
        }
    }

    /// Lane-wise sum: `self[i] + rhs[i]` mod 5.
    ///
    /// # Arguments
    ///
    /// * `rhs` — the other operand; lanes are added pointwise mod 5.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
    /// let b = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(4));
    /// assert_eq!(a.add(b).lane(0), Fp::<5>::new(2)); // 3 + 4 == 2 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 60 word-level bitwise operations.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        let ea = decode5(self.b0, self.b1, self.b2);
        let eb = decode5(rhs.b0, rhs.b1, rhs.b2);
        let (c0, c1, c2) = add_circuit(ea, eb);
        Self {
            b0: c0,
            b1: c1,
            b2: c2,
        }
    }

    /// Lane-wise difference: `self[i] - rhs[i]` mod 5.
    ///
    /// # Arguments
    ///
    /// * `rhs` — the operand subtracted lane-by-lane from `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(1));
    /// let b = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
    /// assert_eq!(a.sub(b).lane(0), Fp::<5>::new(3)); // 1 - 3 == 3 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 60 word-level bitwise operations.
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let ea = decode5(self.b0, self.b1, self.b2);
        let eb = decode5(rhs.b0, rhs.b1, rhs.b2);
        let (c0, c1, c2) = sub_circuit(ea, eb);
        Self {
            b0: c0,
            b1: c1,
            b2: c2,
        }
    }

    /// Lane-wise additive inverse: `-self[i]` mod 5.
    ///
    /// Negation uses a 5-way decode + 5-cell result encoding. For F_5:
    /// neg(0)=0, neg(1)=4, neg(2)=3, neg(3)=2, neg(4)=1.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(2));
    /// assert_eq!(a.neg().lane(0), Fp::<5>::new(3)); // -2 == 3 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: decode + 4 conditional re-encodes + encode.
    #[inline]
    fn neg(self) -> Self {
        // neg: 0->0, 1->4, 2->3, 3->2, 4->1
        // Decode, then re-map: r[0]=e[0], r[1]=e[4], r[2]=e[3], r[3]=e[2], r[4]=e[1]
        let e = decode5(self.b0, self.b1, self.b2);
        let r = [e[0], e[4], e[3], e[2], e[1]];
        let (c0, c1, c2) = encode5(r);
        Self {
            b0: c0,
            b1: c1,
            b2: c2,
        }
    }

    /// Lane-wise product: `self[i] * rhs[i]` mod 5.
    ///
    /// # Arguments
    ///
    /// * `rhs` — the other operand; lanes are multiplied pointwise mod 5.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
    /// let b = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(4));
    /// assert_eq!(a.mul(b).lane(0), Fp::<5>::new(2)); // 3 * 4 == 2 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 52 word-level bitwise operations.
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let ea = decode5(self.b0, self.b1, self.b2);
        let eb = decode5(rhs.b0, rhs.b1, rhs.b2);
        let (c0, c1, c2) = mul_circuit(ea, eb);
        Self {
            b0: c0,
            b1: c1,
            b2: c2,
        }
    }

    /// Decode lane `i` to a canonical `F_5` value.
    ///
    /// Non-canonical codepoints (5..=7) decode to `Fp::<5>::new(0)`.
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
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(4));
    /// assert_eq!(v.lane(0), Fp::<5>::new(4));
    /// assert_eq!(v.lane(63), Fp::<5>::new(4));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: three bit-extracts and a decode.
    #[inline]
    fn lane(self, i: usize) -> Fp<5> {
        assert!(
            i < Self::LANES,
            "Packed5::lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        let bit0 = (self.b0 >> i) & 1;
        let bit1 = (self.b1 >> i) & 1;
        let bit2 = (self.b2 >> i) & 1;
        let v = bit0 | (bit1 << 1) | (bit2 << 2);
        // v is in 0..=7; canonical values are 0..=4; 5..=7 map to 0.
        if v < 5 {
            Fp::<5>::new(v)
        } else {
            Fp::<5>::new(0)
        }
    }

    /// Write the canonical encoding of `x` into lane `i`.
    ///
    /// # Arguments
    ///
    /// * `i` — lane index in `0..64`.
    /// * `x` — scalar `F_5` value to write into lane `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = <Packed5 as PackedField<Fp<5>>>::zero();
    /// let v = v.with_lane(7, Fp::<5>::new(4));
    /// assert_eq!(v.lane(7), Fp::<5>::new(4));
    /// assert_eq!(v.lane(0), Fp::<5>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: three bit-mask and bit-set operations.
    #[inline]
    fn with_lane(self, i: usize, x: Fp<5>) -> Self {
        assert!(
            i < Self::LANES,
            "Packed5::with_lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        let v = x.value();
        let b0_bit = v & 1;
        let b1_bit = (v >> 1) & 1;
        let b2_bit = (v >> 2) & 1;
        let mask = !(1u64 << i);
        Self {
            b0: (self.b0 & mask) | (b0_bit << i),
            b1: (self.b1 & mask) | (b1_bit << i),
            b2: (self.b2 & mask) | (b2_bit << i),
        }
    }

    /// Returns `true` iff every lane decodes to `F_5`'s additive identity (0).
    ///
    /// A lane decodes to a non-zero value iff one of `e[1]`, `e[2]`, `e[3]`,
    /// or `e[4]` from `decode5` is set. Redundant codepoints 5..=7 have all
    /// selectors zero (decode to 0), so checking `e[1]|e[2]|e[3]|e[4] == 0`
    /// correctly canonicalizes them as zero (D1b §3.5).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// assert!(<Packed5 as PackedField<Fp<5>>>::zero().all_zero());
    /// let v = <Packed5 as PackedField<Fp<5>>>::zero().with_lane(3, Fp::<5>::new(1));
    /// assert!(!v.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`: 11 decode ops and one OR + comparison.
    #[inline]
    fn all_zero(self) -> bool {
        // A lane is non-zero iff e[1]|e[2]|e[3]|e[4] != 0.
        // Redundant codepoints produce all-zero selectors, correctly treated as 0.
        let e = decode5(self.b0, self.b1, self.b2);
        (e[1] | e[2] | e[3] | e[4]) == 0
    }
}

// ---------------------------------------------------------------------------
// Packed5Vec — variable-length packed F_5 vector
// ---------------------------------------------------------------------------

/// Variable-length packed `F_5` vector storing `len_lanes` elements as
/// three parallel `Vec<u64>` planes (`b0`, `b1`, `b2`), each of length
/// `ceil(len_lanes / 64)`.
///
/// The encoding of each element matches [`Packed5`]: element at logical
/// position `i` lives in word `i >> 6` at bit `i & 63` of all three planes.
///
/// # Mask-tail invariant
///
/// Bits beyond `len_lanes` in the last word of all three planes must always
/// be zero. Every mutating operation calls [`Packed5Vec::mask_tail`] to
/// enforce this invariant — it is the most critical correctness invariant
/// in this codebase (CLAUDE.md §Key design invariants #1).
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
/// use gf2_core::gfp::Fp;
///
/// let v = Packed5Vec::zeros(5);
/// assert_eq!(v.len(), 5);
/// assert!(v.all_zero());
/// ```
///
/// # Complexity
///
/// Construction and lane-wise operations are `O(ceil(len_lanes / 64))`.
/// Individual lane access ([`get`][`Packed5Vec::get`]) is `O(1)`.
#[derive(Clone)]
pub struct Packed5Vec {
    b0: Vec<u64>,
    b1: Vec<u64>,
    b2: Vec<u64>,
    len_lanes: usize,
}

// ---------------------------------------------------------------------------
// mask_tail and inherent methods
// ---------------------------------------------------------------------------

impl Packed5Vec {
    /// Zero out all bits beyond `self.len_lanes` in the last word of all
    /// three planes.
    ///
    /// **This invariant must hold after every mutation.** Failing to call
    /// `mask_tail` after any write violates the project's key correctness
    /// invariant (CLAUDE.md §Key design invariants #1).
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn mask_tail(&mut self) {
        let n_words = self.b0.len();
        if n_words == 0 {
            return;
        }
        let used = self.len_lanes - 64 * (n_words - 1);
        if used == 64 {
            return; // full last word; no padding to mask
        }
        let mask = (1u64 << used) - 1;
        let last = n_words - 1;
        self.b0[last] &= mask;
        self.b1[last] &= mask;
        self.b2[last] &= mask;
    }

    /// In-place lane-wise additive inverse: `self[i] = -self[i]` for every `i`.
    ///
    /// This method is inherent (not on the trait) because `PackedFieldVec`'s
    /// frozen surface (D1b §2.2) does not include `neg_assign`. Negation
    /// is expressed at the element level via `PackedField::neg`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut v = Packed5Vec::from_field_slice(&[
    ///     Fp::<5>::new(0), Fp::<5>::new(1), Fp::<5>::new(2),
    ///     Fp::<5>::new(3), Fp::<5>::new(4),
    /// ]);
    /// v.neg_assign();
    /// assert_eq!(v.get(0), Fp::<5>::new(0)); // -0 == 0
    /// assert_eq!(v.get(1), Fp::<5>::new(4)); // -1 == 4 mod 5
    /// assert_eq!(v.get(2), Fp::<5>::new(3)); // -2 == 3 mod 5
    /// assert_eq!(v.get(3), Fp::<5>::new(2)); // -3 == 2 mod 5
    /// assert_eq!(v.get(4), Fp::<5>::new(1)); // -4 == 1 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    pub fn neg_assign(&mut self) {
        // neg: e0->e0, e1->e4, e2->e3, e3->e2, e4->e1
        // Expressed in bit-planes: negation swaps values 1<->4 and 2<->3.
        // 0 -> 0: (b2=0, b1=0, b0=0) -> (b2=0, b1=0, b0=0)
        // 1 -> 4: (b2=0, b1=0, b0=1) -> (b2=1, b1=0, b0=0)
        // 2 -> 3: (b2=0, b1=1, b0=0) -> (b2=0, b1=1, b0=1)
        // 3 -> 2: (b2=0, b1=1, b0=1) -> (b2=0, b1=1, b0=0)
        // 4 -> 1: (b2=1, b1=0, b0=0) -> (b2=0, b1=0, b0=1)
        //
        // So we apply the neg formula per word:
        // new_b0 = (e1 | e3_keep) ... but this is easiest via decode5/encode5.
        // Alternatively express directly:
        // new_b2 = old_b0 & ~old_b1 & ~old_b2  (only e1 maps to b2=1)
        // new_b1 = old_b1 & ~old_b2             (e2 and e3 both keep b1=1)
        //          Note: e3=(b2=0,b1=1,b0=1) -> neg -> e2=(b2=0,b1=1,b0=0): b1 stays
        //                e2=(b2=0,b1=1,b0=0) -> neg -> e3=(b2=0,b1=1,b0=1): b1 stays
        //          So new_b1 = b1 & !b2 (both e2 and e3 map to values with b1=1)
        // new_b0 = (b2 & !b1) | (b1 & !b2 & b0)
        //          e4 -> e1: b2=1,b1=0 -> b0=1; and e3->e2: b1=1,b0=1 -> b0=0 (no)
        //          Wait: e3=(b2=0,b1=1,b0=1) -> e2=(b2=0,b1=1,b0=0): b0=0
        //                e2=(b2=0,b1=1,b0=0) -> e3=(b2=0,b1=1,b0=1): b0=1
        //          So new_b0 = (e2: has b0=0) or (e4->e1: b0=1) or (e3->e2: b0=0)
        //          Cleaner: new_b0 = b2_old & !b1_old (e4 has b2=1,b1=0,b0=0 -> e1: b0=1)
        //                         | b1_old & !b0_old & !b2_old (e2 has b1=1,b0=0 -> e3: b0=1)
        //
        // The cleanest implementation: decode + remap + encode per word.
        let n = self.b0.len();
        for w in 0..n {
            let e = decode5(self.b0[w], self.b1[w], self.b2[w]);
            let r = [e[0], e[4], e[3], e[2], e[1]];
            let (c0, c1, c2) = encode5(r);
            self.b0[w] = c0;
            self.b1[w] = c1;
            self.b2[w] = c2;
        }
        self.mask_tail();
    }
}

// ---------------------------------------------------------------------------
// Manual PartialEq / Eq — canonical-decode equality
// ---------------------------------------------------------------------------

impl PartialEq for Packed5Vec {
    /// Canonical-decode equality: two vectors are equal iff they have the
    /// same `len_lanes` and every decoded lane is equal.
    ///
    /// The mask-tail invariant ensures padding bits are 0 on both sides,
    /// so the per-word test is safe for canonically-produced values.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = Packed5Vec::from_field_slice(&[Fp::<5>::new(1), Fp::<5>::new(4)]);
    /// let b = Packed5Vec::from_field_slice(&[Fp::<5>::new(1), Fp::<5>::new(4)]);
    /// assert_eq!(a, b);
    ///
    /// let c = Packed5Vec::from_field_slice(&[Fp::<5>::new(0)]);
    /// assert_ne!(a, c); // different len_lanes
    /// ```
    fn eq(&self, other: &Self) -> bool {
        if self.len_lanes != other.len_lanes {
            return false;
        }
        // Per-lane canonical-decode equality: two lanes are equal iff they decode
        // to the same F_5 value (0..=4). Redundant codepoints (5..=7) decode to 0.
        //
        // decode5 maps redundant codepoints to all-zero selectors (e[0..4] = 0),
        // while canonical 0 gives e[0]=1. We compare only e[1..4] — both canonical
        // and redundant zeros give e[1..4]=0, so they compare equal.
        // mask_tail ensures padding bits are zero; padding lanes have e[1..4]=0
        // on both sides, so full-word comparison is safe.
        for w in 0..self.b0.len() {
            let sa = decode5(self.b0[w], self.b1[w], self.b2[w]);
            let sb = decode5(other.b0[w], other.b1[w], other.b2[w]);
            if sa[1] != sb[1] || sa[2] != sb[2] || sa[3] != sb[3] || sa[4] != sb[4] {
                return false;
            }
        }
        true
    }
}

impl Eq for Packed5Vec {}

// ---------------------------------------------------------------------------
// Manual Debug — print decoded lane values
// ---------------------------------------------------------------------------

impl fmt::Debug for Packed5Vec {
    /// Formats the value as a `Vec` of decoded lane values (each `0..=4`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Packed5Vec::from_field_slice(&[Fp::<5>::new(2), Fp::<5>::new(4)]);
    /// let s = format!("{:?}", v);
    /// assert!(s.contains("lanes"));
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lanes: Vec<u64> = (0..self.len_lanes).map(|i| self.get(i).value()).collect();
        f.debug_struct("Packed5Vec").field("lanes", &lanes).finish()
    }
}

// ---------------------------------------------------------------------------
// PackedFieldVec<Fp<5>>
// ---------------------------------------------------------------------------

impl PackedFieldVec<Fp<5>> for Packed5Vec {
    type Element = Packed5;

    /// Construct a vector of `len` zero `F_5` elements.
    ///
    /// # Arguments
    ///
    /// * `len` — number of logical `F_5` positions in the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    ///
    /// let v = Packed5Vec::zeros(65);
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
            b0: vec![0u64; n_words],
            b1: vec![0u64; n_words],
            b2: vec![0u64; n_words],
            len_lanes: len,
        }
    }

    /// Construct a vector by encoding every element of `xs`.
    ///
    /// Position `i` is set to the canonical 3-plane encoding of `xs[i]`.
    /// `mask_tail` is called at the end to enforce the zero-padding invariant.
    ///
    /// # Arguments
    ///
    /// * `xs` — source slice; the result has `xs.len()` logical positions
    ///   and `get(i) == xs[i]` for every `i`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let xs = [Fp::<5>::new(0), Fp::<5>::new(2), Fp::<5>::new(4)];
    /// let v = Packed5Vec::from_field_slice(&xs);
    /// for i in 0..3 {
    ///     assert_eq!(v.get(i), xs[i]);
    /// }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(xs.len())`.
    fn from_field_slice(xs: &[Fp<5>]) -> Self {
        let len = xs.len();
        let n_words = len.div_ceil(64);
        let mut b0 = vec![0u64; n_words];
        let mut b1 = vec![0u64; n_words];
        let mut b2 = vec![0u64; n_words];
        for (i, &x) in xs.iter().enumerate() {
            let v = x.value();
            let w = i >> 6;
            let s = i & 63;
            if (v & 1) != 0 {
                b0[w] |= 1u64 << s;
            }
            if (v & 2) != 0 {
                b1[w] |= 1u64 << s;
            }
            if (v & 4) != 0 {
                b2[w] |= 1u64 << s;
            }
        }
        let mut result = Self {
            b0,
            b1,
            b2,
            len_lanes: len,
        };
        result.mask_tail();
        result
    }

    /// Number of logical `F_5` positions held by this vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    ///
    /// assert_eq!(Packed5Vec::zeros(100).len(), 100);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn len(&self) -> usize {
        self.len_lanes
    }

    /// Decode logical position `i` to a canonical `F_5` value.
    ///
    /// Non-canonical codepoints (5..=7) decode to `Fp::<5>::new(0)`.
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
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = Packed5Vec::from_field_slice(&[Fp::<5>::new(3)]);
    /// assert_eq!(v.get(0), Fp::<5>::new(3));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    fn get(&self, i: usize) -> Fp<5> {
        assert!(
            i < self.len_lanes,
            "Packed5Vec::get: index {} out of range (len = {})",
            i,
            self.len_lanes
        );
        let w = i >> 6;
        let s = i & 63;
        let bit0 = (self.b0[w] >> s) & 1;
        let bit1 = (self.b1[w] >> s) & 1;
        let bit2 = (self.b2[w] >> s) & 1;
        let v = bit0 | (bit1 << 1) | (bit2 << 2);
        if v < 5 {
            Fp::<5>::new(v)
        } else {
            Fp::<5>::new(0)
        }
    }

    /// Lane-wise in-place sum: `self[i] += rhs[i]` for every `i`.
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
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Packed5Vec::from_field_slice(&[Fp::<5>::new(3), Fp::<5>::new(4)]);
    /// let b = Packed5Vec::from_field_slice(&[Fp::<5>::new(4), Fp::<5>::new(4)]);
    /// a.add_assign(&b);
    /// assert_eq!(a.get(0), Fp::<5>::new(2)); // 3 + 4 = 2 mod 5
    /// assert_eq!(a.get(1), Fp::<5>::new(3)); // 4 + 4 = 3 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn add_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Packed5Vec::add_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.b0.len() {
            let ea = decode5(self.b0[w], self.b1[w], self.b2[w]);
            let eb = decode5(rhs.b0[w], rhs.b1[w], rhs.b2[w]);
            let (c0, c1, c2) = add_circuit(ea, eb);
            self.b0[w] = c0;
            self.b1[w] = c1;
            self.b2[w] = c2;
        }
        self.mask_tail();
    }

    /// Lane-wise in-place difference: `self[i] -= rhs[i]` for every `i`.
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
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Packed5Vec::from_field_slice(&[Fp::<5>::new(1)]);
    /// let b = Packed5Vec::from_field_slice(&[Fp::<5>::new(3)]);
    /// a.sub_assign(&b);
    /// assert_eq!(a.get(0), Fp::<5>::new(3)); // 1 - 3 = 3 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn sub_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Packed5Vec::sub_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.b0.len() {
            let ea = decode5(self.b0[w], self.b1[w], self.b2[w]);
            let eb = decode5(rhs.b0[w], rhs.b1[w], rhs.b2[w]);
            let (c0, c1, c2) = sub_circuit(ea, eb);
            self.b0[w] = c0;
            self.b1[w] = c1;
            self.b2[w] = c2;
        }
        self.mask_tail();
    }

    /// Lane-wise in-place product: `self[i] *= rhs[i]` for every `i`.
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
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = Packed5Vec::from_field_slice(&[Fp::<5>::new(3)]);
    /// let b = Packed5Vec::from_field_slice(&[Fp::<5>::new(4)]);
    /// a.mul_assign(&b);
    /// assert_eq!(a.get(0), Fp::<5>::new(2)); // 3 * 4 = 2 mod 5
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn mul_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Packed5Vec::mul_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.b0.len() {
            let ea = decode5(self.b0[w], self.b1[w], self.b2[w]);
            let eb = decode5(rhs.b0[w], rhs.b1[w], rhs.b2[w]);
            let (c0, c1, c2) = mul_circuit(ea, eb);
            self.b0[w] = c0;
            self.b1[w] = c1;
            self.b2[w] = c2;
        }
        self.mask_tail();
    }

    /// Returns `true` iff every logical position decodes to `F_5`'s
    /// additive identity (0).
    ///
    /// Uses `decode5` per word and checks that no non-zero result selector
    /// is set, i.e. `(e[1] | e[2] | e[3] | e[4]) == 0`. This handles both
    /// canonical zero codepoints and redundant non-canonical codepoints
    /// (5..=7) that decode to 0 (D1b §3.5 canonicalization contract).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed5Vec};
    /// use gf2_core::gfp::Fp;
    ///
    /// assert!(Packed5Vec::zeros(10).all_zero());
    /// let nz = Packed5Vec::from_field_slice(&[Fp::<5>::new(1)]);
    /// assert!(!nz.all_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(ceil(self.len() / 64))`.
    fn all_zero(&self) -> bool {
        // A lane is non-zero iff e[1]|e[2]|e[3]|e[4] != 0.
        // Redundant codepoints (5..=7) produce all-zero selectors, correctly
        // treated as zero (D1b §3.5 canonicalization contract).
        // mask_tail guarantees padding bits are zero, so padding lanes produce
        // all-zero selectors, contributing nothing to the OR — safe to test
        // full words including the partial last word.
        self.b0
            .iter()
            .zip(self.b1.iter())
            .zip(self.b2.iter())
            .all(|((&b0, &b1), &b2)| {
                let e = decode5(b0, b1, b2);
                (e[1] | e[2] | e[3] | e[4]) == 0
            })
    }
}

// ---------------------------------------------------------------------------
// Packed5 — fold_mul_first_n
// ---------------------------------------------------------------------------

impl Packed5 {
    /// Reduce the first `n` lanes of `self` to a single `Fp<5>` via
    /// lane-wise multiplication.
    ///
    /// Lanes `n..63` are treated as the multiplicative identity (`1`) and do
    /// not contribute to the result. An all-zero column-sum (any active lane
    /// is 0) yields `Fp::<5>::new(0)`.
    ///
    /// This is the F_5 analogue of `Bipedal3::fold_mul_first_n`; both are used
    /// by the single-word Ryser permanent kernels to fold the per-step
    /// column-sum vector into a scalar product.
    ///
    /// # Arguments
    ///
    /// * `n` — number of active lanes to fold (must satisfy `1 <= n <= 64`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedField, Packed5};
    /// use gf2_core::gfp::Fp;
    ///
    /// // Three lanes set to 2; product over F_5 = 2^3 mod 5 = 3.
    /// let v = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(2));
    /// assert_eq!(v.fold_mul_first_n(3), Fp::<5>::new(3)); // 2*2*2 = 8 mod 5 = 3
    ///
    /// // Single lane set to 3; product = 3.
    /// let w = <Packed5 as PackedField<Fp<5>>>::zero().with_lane(0, Fp::<5>::new(3));
    /// assert_eq!(w.fold_mul_first_n(1), Fp::<5>::new(3));
    ///
    /// // Any zero lane collapses the product to 0.
    /// let z = <Packed5 as PackedField<Fp<5>>>::zero();
    /// assert_eq!(z.fold_mul_first_n(2), Fp::<5>::new(0));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `n == 0` or `n > 64`.
    ///
    /// # Complexity
    ///
    /// `O(n)` — decodes each of the `n` active lanes and multiplies them
    /// into a running `Fp<5>` accumulator. At `n <= 64` this is a bounded
    /// constant in the asymptotic sense.
    pub fn fold_mul_first_n(self, n: usize) -> Fp<5> {
        assert!(
            (1..=64).contains(&n),
            "Packed5::fold_mul_first_n: n must satisfy 1 <= n <= 64; got n = {n}"
        );
        // There is no bit-sliced halving-fold for F_5 analogous to the Bipedal3
        // trick (which exploits the fact that F_3 mul is XOR on sgn and AND on
        // mag). F_5 multiplication requires a full decode-cross-product-encode
        // circuit, so we decode each active lane and multiply them one by one.
        //
        // This is still O(1) in the asymptotic sense because n <= 64 is a fixed
        // bound, and the 64 individual lane decodes are all in-register bit ops.
        let mut acc = Fp::<5>::new(1); // multiplicative identity
        for i in 0..n {
            let lane_val = self.lane(i);
            acc = acc * lane_val;
        }
        acc
    }
}

// ---------------------------------------------------------------------------
// Packed5Matrix — column-major rectangular matrix of packed F_5 values
// ---------------------------------------------------------------------------

/// Column-major rectangular matrix of `F_5` elements, stored as one
/// [`Packed5Vec`] per column.
///
/// Each column `j` is a [`Packed5Vec`] of length `rows`; the entry at
/// row `i`, column `j` is `self.column(j).get(i)`.
///
/// The column-major layout is the primary access pattern for the Gray-code
/// Ryser permanent kernel ([`crate::permanent::permanent_bipedal5`]):
/// storing each column as a contiguous `Packed5Vec` allows the per-step
/// column-sum update (`col_sum.add_assign(columns[flip])` or `sub_assign`)
/// to operate on the column data without scatter-gather.
///
/// # Mask-tail invariant
///
/// Each column is a [`Packed5Vec`] and inherits its mask-tail invariant:
/// padding bits beyond `rows` in the last word of each column's planes
/// are always zero.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Packed5Matrix;
/// use gf2_core::gfp::Fp;
///
/// let data: Vec<Fp<5>> = vec![
///     Fp::<5>::new(1), Fp::<5>::new(2),
///     Fp::<5>::new(3), Fp::<5>::new(4),
/// ];
/// let m = Packed5Matrix::from_row_major(&data, 2, 2);
/// assert_eq!(m.rows(), 2);
/// assert_eq!(m.cols(), 2);
/// assert_eq!(m.get(0, 1), Fp::<5>::new(2));
/// assert_eq!(m.get(1, 0), Fp::<5>::new(3));
/// ```
///
/// # Complexity
///
/// Construction is `O(rows * cols)`; column access is `O(1)`;
/// individual element access is `O(1)`.
pub struct Packed5Matrix {
    /// One `Packed5Vec` per column, each of length `rows`.
    columns: Vec<Packed5Vec>,
    rows: usize,
    cols: usize,
}

// ---------------------------------------------------------------------------
// Manual PartialEq / Eq for Packed5Matrix
// ---------------------------------------------------------------------------

impl PartialEq for Packed5Matrix {
    /// Shape-equal and per-column canonical-decode equal.
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows && self.cols == other.cols && self.columns == other.columns
    }
}

impl Eq for Packed5Matrix {}

// ---------------------------------------------------------------------------
// Manual Debug for Packed5Matrix
// ---------------------------------------------------------------------------

impl fmt::Debug for Packed5Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rows: Vec<Vec<u64>> = (0..self.rows)
            .map(|i| {
                (0..self.cols)
                    .map(|j| self.columns[j].get(i).value())
                    .collect()
            })
            .collect();
        f.debug_struct("Packed5Matrix")
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("data", &rows)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Packed5Matrix inherent methods
// ---------------------------------------------------------------------------

impl Packed5Matrix {
    /// Construct a matrix from a row-major `Fp<5>` slice.
    ///
    /// The entry at row `i`, column `j` is `data[i * cols + j]`. The slice
    /// is re-encoded in column-major order: each column `j` becomes a
    /// [`Packed5Vec`] of length `rows` containing `data[0*cols+j]`,
    /// `data[1*cols+j]`, ..., `data[(rows-1)*cols+j]`.
    ///
    /// Empty matrices (`rows == 0` or `cols == 0`) are allowed.
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
    /// use gf2_algebra::packed::Packed5Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<5>> = vec![
    ///     Fp::<5>::new(0), Fp::<5>::new(1), Fp::<5>::new(2),
    ///     Fp::<5>::new(3), Fp::<5>::new(4), Fp::<5>::new(0),
    /// ];
    /// let m = Packed5Matrix::from_row_major(&data, 2, 3);
    /// assert_eq!(m.rows(), 2);
    /// assert_eq!(m.cols(), 3);
    /// assert_eq!(m.get(0, 2), Fp::<5>::new(2));
    /// assert_eq!(m.get(1, 1), Fp::<5>::new(4));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(rows * cols)`.
    pub fn from_row_major(data: &[Fp<5>], rows: usize, cols: usize) -> Self {
        assert_eq!(
            data.len(),
            rows * cols,
            "Packed5Matrix::from_row_major: data.len() ({}) != rows ({}) * cols ({})",
            data.len(),
            rows,
            cols
        );
        let columns: Vec<Packed5Vec> = (0..cols)
            .map(|j| {
                let col_data: Vec<Fp<5>> = (0..rows).map(|i| data[i * cols + j]).collect();
                Packed5Vec::from_field_slice(&col_data)
            })
            .collect();
        Self {
            columns,
            rows,
            cols,
        }
    }

    /// Number of rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed5Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = Packed5Matrix::from_row_major(&[], 0, 3);
    /// assert_eq!(m.rows(), 0);
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
    /// use gf2_algebra::packed::Packed5Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let m = Packed5Matrix::from_row_major(&[], 3, 0);
    /// assert_eq!(m.cols(), 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Borrow the `j`-th column as a `&Packed5Vec` of length `rows`.
    ///
    /// This is the primary access pattern for the Gray-code Ryser permanent
    /// kernel: iterating `column(j)` for `j` in `0..cols` is zero-copy.
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
    /// use gf2_algebra::packed::{Packed5Matrix, PackedFieldVec};
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<5>> = vec![
    ///     Fp::<5>::new(1), Fp::<5>::new(2),
    ///     Fp::<5>::new(3), Fp::<5>::new(4),
    /// ];
    /// let m = Packed5Matrix::from_row_major(&data, 2, 2);
    /// assert_eq!(m.column(1).get(0), Fp::<5>::new(2));
    /// assert_eq!(m.column(1).get(1), Fp::<5>::new(4));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    #[inline]
    pub fn column(&self, j: usize) -> &Packed5Vec {
        assert!(
            j < self.cols,
            "Packed5Matrix::column: index {} out of range (cols = {})",
            j,
            self.cols
        );
        &self.columns[j]
    }

    /// Decode entry at row `i`, column `j` to a canonical `F_5` value.
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
    /// use gf2_algebra::packed::Packed5Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<5>> = vec![
    ///     Fp::<5>::new(1), Fp::<5>::new(2),
    ///     Fp::<5>::new(3), Fp::<5>::new(4),
    /// ];
    /// let m = Packed5Matrix::from_row_major(&data, 2, 2);
    /// assert_eq!(m.get(0, 0), Fp::<5>::new(1));
    /// assert_eq!(m.get(1, 1), Fp::<5>::new(4));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn get(&self, i: usize, j: usize) -> Fp<5> {
        assert!(
            i < self.rows,
            "Packed5Matrix::get: row index {} out of range (rows = {})",
            i,
            self.rows
        );
        self.column(j).get(i)
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

    /// Strategy: a single `Fp<5>` element drawn uniformly from `{0..=4}`.
    fn fp5_strat() -> impl Strategy<Value = Fp<5>> {
        (0u64..5).prop_map(Fp::<5>::new)
    }

    /// Strategy: a `Packed5` with every lane independently drawn from `{0..=4}`.
    fn packed5_strat() -> impl Strategy<Value = Packed5> {
        prop::collection::vec(fp5_strat(), 64).prop_map(|v| {
            let mut p = Packed5::zero();
            for (i, x) in v.into_iter().enumerate() {
                p = p.with_lane(i, x);
            }
            p
        })
    }

    /// Scalar F_5 add: `(a + b) % 5`.
    fn scalar_add(a: u64, b: u64) -> u64 {
        (a + b) % 5
    }

    /// Scalar F_5 sub: `(a + 5 - b) % 5`.
    fn scalar_sub(a: u64, b: u64) -> u64 {
        (a + 5 - b) % 5
    }

    /// Scalar F_5 mul: `(a * b) % 5`.
    fn scalar_mul(a: u64, b: u64) -> u64 {
        (a * b) % 5
    }

    /// Scalar F_5 neg: `(5 - a) % 5`.
    fn scalar_neg(a: u64) -> u64 {
        (5 - a) % 5
    }

    // -----------------------------------------------------------------------
    // LANES constant
    // -----------------------------------------------------------------------

    #[test]
    fn test_lanes_const_is_64() {
        assert_eq!(<Packed5 as PackedField<Fp<5>>>::LANES, 64);
    }

    // -----------------------------------------------------------------------
    // Exhaustive 5×5 tests for each binary op
    // -----------------------------------------------------------------------

    /// Add: all 25 pairs from {0,1,2,3,4}^2.
    #[test]
    fn test_add_exhaustive_5x5() {
        for a in 0u64..5 {
            for b in 0u64..5 {
                let pa = Packed5::splat(Fp::<5>::new(a));
                let pb = Packed5::splat(Fp::<5>::new(b));
                let result = pa.add(pb);
                let expected = scalar_add(a, b);
                let got = result.lane(0).value();
                assert_eq!(
                    got, expected,
                    "add({a}, {b}): expected {expected}, got {got}"
                );
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), expected, "add({a},{b}) lane {i}");
                }
            }
        }
    }

    /// Sub: all 25 pairs from {0,1,2,3,4}^2.
    #[test]
    fn test_sub_exhaustive_5x5() {
        for a in 0u64..5 {
            for b in 0u64..5 {
                let pa = Packed5::splat(Fp::<5>::new(a));
                let pb = Packed5::splat(Fp::<5>::new(b));
                let result = pa.sub(pb);
                let expected = scalar_sub(a, b);
                let got = result.lane(0).value();
                assert_eq!(
                    got, expected,
                    "sub({a}, {b}): expected {expected}, got {got}"
                );
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), expected, "sub({a},{b}) lane {i}");
                }
            }
        }
    }

    /// Mul: all 25 pairs from {0,1,2,3,4}^2.
    #[test]
    fn test_mul_exhaustive_5x5() {
        for a in 0u64..5 {
            for b in 0u64..5 {
                let pa = Packed5::splat(Fp::<5>::new(a));
                let pb = Packed5::splat(Fp::<5>::new(b));
                let result = pa.mul(pb);
                let expected = scalar_mul(a, b);
                let got = result.lane(0).value();
                assert_eq!(
                    got, expected,
                    "mul({a}, {b}): expected {expected}, got {got}"
                );
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), expected, "mul({a},{b}) lane {i}");
                }
            }
        }
    }

    /// Neg: all 5 single inputs.
    #[test]
    fn test_neg_exhaustive_5() {
        for a in 0u64..5 {
            let pa = Packed5::splat(Fp::<5>::new(a));
            let result = pa.neg();
            let expected = scalar_neg(a);
            let got = result.lane(0).value();
            assert_eq!(got, expected, "neg({a}): expected {expected}, got {got}");
            for i in 1..64 {
                assert_eq!(result.lane(i).value(), expected, "neg({a}) lane {i}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Per-lane mixed tests
    // -----------------------------------------------------------------------

    /// Pack two arrays with values cycling through 0..5, run add, compare lane-by-lane.
    #[test]
    fn test_add_mixed_lanes() {
        let mut a_arr = [Fp::<5>::new(0); 64];
        let mut b_arr = [Fp::<5>::new(0); 64];
        for i in 0..64 {
            a_arr[i] = Fp::<5>::new((i as u64) % 5);
            b_arr[i] = Fp::<5>::new(((i + 1) as u64) % 5);
        }
        let mut pa = Packed5::zero();
        let mut pb = Packed5::zero();
        for i in 0..64 {
            pa = pa.with_lane(i, a_arr[i]);
            pb = pb.with_lane(i, b_arr[i]);
        }
        let result = pa.add(pb);
        for i in 0..64 {
            let expected = scalar_add(a_arr[i].value(), b_arr[i].value());
            assert_eq!(
                result.lane(i).value(),
                expected,
                "add mixed lane {i}: expected {expected}, got {}",
                result.lane(i).value()
            );
        }
    }

    /// Sub mixed lanes.
    #[test]
    fn test_sub_mixed_lanes() {
        let mut pa = Packed5::zero();
        let mut pb = Packed5::zero();
        for i in 0..64 {
            pa = pa.with_lane(i, Fp::<5>::new((i as u64) % 5));
            pb = pb.with_lane(i, Fp::<5>::new(((i + 3) as u64) % 5));
        }
        let result = pa.sub(pb);
        for i in 0..64 {
            let a = (i as u64) % 5;
            let b = ((i + 3) as u64) % 5;
            let expected = scalar_sub(a, b);
            assert_eq!(result.lane(i).value(), expected, "sub mixed lane {i}");
        }
    }

    /// Mul mixed lanes.
    #[test]
    fn test_mul_mixed_lanes() {
        let mut pa = Packed5::zero();
        let mut pb = Packed5::zero();
        for i in 0..64 {
            pa = pa.with_lane(i, Fp::<5>::new((i as u64) % 5));
            pb = pb.with_lane(i, Fp::<5>::new(((i * 2 + 1) as u64) % 5));
        }
        let result = pa.mul(pb);
        for i in 0..64 {
            let a = (i as u64) % 5;
            let b = ((i * 2 + 1) as u64) % 5;
            let expected = scalar_mul(a, b);
            assert_eq!(result.lane(i).value(), expected, "mul mixed lane {i}");
        }
    }

    // -----------------------------------------------------------------------
    // Panic tests
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_out_of_range_64() {
        let _ = Packed5::zero().lane(64);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_out_of_range_64() {
        let _ = Packed5::zero().with_lane(64, Fp::<5>::new(1));
    }

    // -----------------------------------------------------------------------
    // all_zero
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_zero_canonical() {
        assert!(Packed5::zero().all_zero());
    }

    #[test]
    fn test_all_zero_one_nonzero_lane() {
        let v = Packed5::zero().with_lane(0, Fp::<5>::new(1));
        assert!(!v.all_zero());
    }

    #[test]
    fn test_all_zero_one_is_not_zero() {
        assert!(!Packed5::one().all_zero());
    }

    // -----------------------------------------------------------------------
    // Proptest cross-check (1000 cases) vs scalar Fp<5> per-lane
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

        /// add: Packed5 result matches scalar per-lane.
        #[test]
        fn test_proptest_add_matches_scalar(
            a in packed5_strat(),
            b in packed5_strat(),
        ) {
            let r = a.add(b);
            for i in 0..64 {
                let expected = scalar_add(a.lane(i).value(), b.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected, "add lane {}", i);
            }
        }

        /// sub: Packed5 result matches scalar per-lane.
        #[test]
        fn test_proptest_sub_matches_scalar(
            a in packed5_strat(),
            b in packed5_strat(),
        ) {
            let r = a.sub(b);
            for i in 0..64 {
                let expected = scalar_sub(a.lane(i).value(), b.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected, "sub lane {}", i);
            }
        }

        /// mul: Packed5 result matches scalar per-lane.
        #[test]
        fn test_proptest_mul_matches_scalar(
            a in packed5_strat(),
            b in packed5_strat(),
        ) {
            let r = a.mul(b);
            for i in 0..64 {
                let expected = scalar_mul(a.lane(i).value(), b.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected, "mul lane {}", i);
            }
        }

        /// neg: Packed5 result matches scalar per-lane.
        #[test]
        fn test_proptest_neg_matches_scalar(a in packed5_strat()) {
            let r = a.neg();
            for i in 0..64 {
                let expected = scalar_neg(a.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected, "neg lane {}", i);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Packed5Vec word-boundary tests
    // -----------------------------------------------------------------------

    /// Helper: build a Packed5Vec of length `len` with values `i % 5`.
    fn make_vec(len: usize) -> Packed5Vec {
        let xs: Vec<Fp<5>> = (0..len).map(|i| Fp::<5>::new((i as u64) % 5)).collect();
        Packed5Vec::from_field_slice(&xs)
    }

    /// Helper: verify mask_tail invariant — padding bits in last word of all
    /// three planes must be zero.
    fn assert_mask_tail_invariant(v: &Packed5Vec) {
        let n_words = v.b0.len();
        if n_words == 0 {
            return;
        }
        let used = v.len_lanes - 64 * (n_words - 1);
        if used == 64 {
            return; // full last word, no padding
        }
        let mask = (1u64 << used) - 1;
        let last = n_words - 1;
        assert_eq!(
            v.b0[last] & !mask,
            0,
            "mask_tail violated: b0 padding bits non-zero at len={}",
            v.len_lanes
        );
        assert_eq!(
            v.b1[last] & !mask,
            0,
            "mask_tail violated: b1 padding bits non-zero at len={}",
            v.len_lanes
        );
        assert_eq!(
            v.b2[last] & !mask,
            0,
            "mask_tail violated: b2 padding bits non-zero at len={}",
            v.len_lanes
        );
    }

    macro_rules! test_vec_word_boundary {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let len = $len;

                // zeros
                let z = Packed5Vec::zeros(len);
                assert_eq!(z.len(), len);
                assert!(z.all_zero(), "zeros({len}) should be all_zero");
                assert_mask_tail_invariant(&z);

                // from_field_slice round-trip
                let a = make_vec(len);
                assert_eq!(a.len(), len);
                assert_mask_tail_invariant(&a);
                for i in 0..len {
                    assert_eq!(
                        a.get(i),
                        Fp::<5>::new((i as u64) % 5),
                        "from_field_slice({len}).get({i})"
                    );
                }

                // add_assign
                let mut va = make_vec(len);
                let vb = make_vec(len);
                va.add_assign(&vb);
                assert_mask_tail_invariant(&va);
                for i in 0..len {
                    let ai = (i as u64) % 5;
                    let bi = (i as u64) % 5;
                    assert_eq!(
                        va.get(i).value(),
                        scalar_add(ai, bi),
                        "add_assign({len}) lane {i}"
                    );
                }

                // sub_assign
                let mut va = make_vec(len);
                let vb = make_vec(len);
                va.sub_assign(&vb);
                assert_mask_tail_invariant(&va);
                for i in 0..len {
                    let ai = (i as u64) % 5;
                    let bi = (i as u64) % 5;
                    assert_eq!(
                        va.get(i).value(),
                        scalar_sub(ai, bi),
                        "sub_assign({len}) lane {i}"
                    );
                }

                // mul_assign
                let mut va = make_vec(len);
                let vb = make_vec(len);
                va.mul_assign(&vb);
                assert_mask_tail_invariant(&va);
                for i in 0..len {
                    let ai = (i as u64) % 5;
                    let bi = (i as u64) % 5;
                    assert_eq!(
                        va.get(i).value(),
                        scalar_mul(ai, bi),
                        "mul_assign({len}) lane {i}"
                    );
                }

                // neg_assign
                let mut va = make_vec(len);
                va.neg_assign();
                assert_mask_tail_invariant(&va);
                for i in 0..len {
                    let ai = (i as u64) % 5;
                    assert_eq!(
                        va.get(i).value(),
                        scalar_neg(ai),
                        "neg_assign({len}) lane {i}"
                    );
                }
            }
        };
    }

    #[test]
    fn test_vec_len_0() {
        let z = Packed5Vec::zeros(0);
        assert_eq!(z.len(), 0);
        assert!(z.all_zero());
        assert!(z.is_empty());
    }

    test_vec_word_boundary!(test_vec_len_1, 1);
    test_vec_word_boundary!(test_vec_len_63, 63);
    test_vec_word_boundary!(test_vec_len_64, 64);
    test_vec_word_boundary!(test_vec_len_65, 65);
    test_vec_word_boundary!(test_vec_len_127, 127);
    test_vec_word_boundary!(test_vec_len_128, 128);
    test_vec_word_boundary!(test_vec_len_129, 129);

    // -----------------------------------------------------------------------
    // Packed5Vec — length mismatch panics
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_vec_add_assign_length_mismatch() {
        let mut a = Packed5Vec::zeros(5);
        let b = Packed5Vec::zeros(6);
        a.add_assign(&b);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_vec_sub_assign_length_mismatch() {
        let mut a = Packed5Vec::zeros(5);
        let b = Packed5Vec::zeros(6);
        a.sub_assign(&b);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_vec_mul_assign_length_mismatch() {
        let mut a = Packed5Vec::zeros(5);
        let b = Packed5Vec::zeros(6);
        a.mul_assign(&b);
    }

    // -----------------------------------------------------------------------
    // Canonicalization contract tests (Finding 2 rework)
    //
    // Redundant codepoints 5..=7: decode5 produces all-zero selectors for them
    // (none of e[0..4] is hot), so they decode to 0 semantically. The
    // all_zero and eq implementations check e[1]|e[2]|e[3]|e[4] == 0 to
    // canonicalize redundant zero codepoints correctly.
    // -----------------------------------------------------------------------

    /// Build a Packed5 directly from raw bit-plane values (test-only).
    /// Used to inject redundant codepoints without going through the public API.
    fn packed5_raw(b0: u64, b1: u64, b2: u64) -> Packed5 {
        Packed5 { b0, b1, b2 }
    }

    /// Build a single-word Packed5Vec directly from raw bit-plane values (test-only).
    fn packed5vec_raw(b0: u64, b1: u64, b2: u64, len_lanes: usize) -> Packed5Vec {
        Packed5Vec {
            b0: vec![b0],
            b1: vec![b1],
            b2: vec![b2],
            len_lanes,
        }
    }

    // --- Packed5::all_zero ---

    #[test]
    fn test_all_zero_canonical_zero() {
        assert!(
            Packed5::zero().all_zero(),
            "canonical zero must report all_zero"
        );
    }

    #[test]
    fn test_all_zero_redundant_codepoint_5() {
        // Lane 0 = codepoint 5 (b0=1, b1=0, b2=1). Decodes to 0.
        let raw = packed5_raw(1u64, 0u64, 1u64);
        assert!(raw.all_zero(), "redundant codepoint 5 must report all_zero");
    }

    #[test]
    fn test_all_zero_redundant_codepoint_6() {
        // Lane 0 = codepoint 6 (b0=0, b1=1, b2=1). Decodes to 0.
        let raw = packed5_raw(0u64, 1u64, 1u64);
        assert!(raw.all_zero(), "redundant codepoint 6 must report all_zero");
    }

    #[test]
    fn test_all_zero_redundant_codepoint_7() {
        // Lane 0 = codepoint 7 (b0=1, b1=1, b2=1). Decodes to 0.
        let raw = packed5_raw(1u64, 1u64, 1u64);
        assert!(raw.all_zero(), "redundant codepoint 7 must report all_zero");
    }

    #[test]
    fn test_all_zero_canonical_one_not_zero() {
        // Lane 0 = canonical 1 (b0=1, b1=0, b2=0). Not zero.
        let raw = packed5_raw(1u64, 0u64, 0u64);
        assert!(!raw.all_zero(), "canonical 1 must not report all_zero");
    }

    // --- Packed5::eq canonicalization ---

    #[test]
    fn test_packed5_eq_redundant_codepoint_5_equals_canonical_zero() {
        // Lane 0 = codepoint 5 (redundant zero) vs. canonical zero.
        let lhs = packed5_raw(1u64, 0u64, 1u64);
        let rhs = Packed5::zero();
        assert_eq!(lhs, rhs, "codepoint 5 must equal canonical zero");
    }

    #[test]
    fn test_packed5_eq_redundant_codepoint_6_equals_canonical_zero() {
        let lhs = packed5_raw(0u64, 1u64, 1u64);
        let rhs = Packed5::zero();
        assert_eq!(lhs, rhs, "codepoint 6 must equal canonical zero");
    }

    #[test]
    fn test_packed5_eq_canonical_values_equal() {
        let a = Packed5::splat(Fp::<5>::new(3));
        let b = Packed5::splat(Fp::<5>::new(3));
        assert_eq!(a, b);
    }

    #[test]
    fn test_packed5_eq_different_values_not_equal() {
        let a = Packed5::splat(Fp::<5>::new(2));
        let b = Packed5::splat(Fp::<5>::new(3));
        assert_ne!(a, b);
    }

    // --- Packed5Vec::all_zero ---

    #[test]
    fn test_packed5vec_all_zero_zeros_is_zero() {
        assert!(Packed5Vec::zeros(1).all_zero());
        assert!(Packed5Vec::zeros(64).all_zero());
        assert!(Packed5Vec::zeros(65).all_zero());
    }

    #[test]
    fn test_packed5vec_all_zero_redundant_codepoint_5() {
        // Lane 0 = codepoint 5 (b0=1, b1=0, b2=1), rest canonical 0.
        let raw = packed5vec_raw(1u64, 0u64, 1u64, 64);
        assert!(
            raw.all_zero(),
            "Packed5Vec: codepoint 5 must report all_zero"
        );
    }

    #[test]
    fn test_packed5vec_all_zero_canonical_one_not_zero() {
        // Lane 0 = canonical 1.
        let raw = packed5vec_raw(1u64, 0u64, 0u64, 64);
        assert!(
            !raw.all_zero(),
            "Packed5Vec: canonical 1 must not report all_zero"
        );
    }

    // --- Packed5Vec::eq canonicalization ---

    #[test]
    fn test_packed5vec_eq_redundant_zero_equals_canonical_zero() {
        // Lane 0 = codepoint 5 (redundant zero) vs. full canonical zero vec.
        let lhs = packed5vec_raw(1u64, 0u64, 1u64, 64);
        let rhs = Packed5Vec::zeros(64);
        assert_eq!(
            lhs, rhs,
            "Packed5Vec: codepoint 5 must equal canonical zero"
        );
    }

    #[test]
    fn test_packed5vec_eq_same_canonical_values() {
        let a = Packed5Vec::from_field_slice(&[Fp::<5>::new(1), Fp::<5>::new(3)]);
        let b = Packed5Vec::from_field_slice(&[Fp::<5>::new(1), Fp::<5>::new(3)]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_packed5vec_eq_different_values_not_equal() {
        let a = Packed5Vec::from_field_slice(&[Fp::<5>::new(1)]);
        let b = Packed5Vec::from_field_slice(&[Fp::<5>::new(2)]);
        assert_ne!(a, b);
    }
}
