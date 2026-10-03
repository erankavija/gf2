//! Packed `F_5` encoding, compiled under the `f5` Cargo feature: 64 lanes per
//! three `u64` bit-planes `(b0, b1, b2)`, where bit `k` of a lane's canonical
//! value `0..=4` lives in plane `bk`. Codepoints `5..=7` are redundant, are
//! never produced by the arithmetic, and decode to 0.

use core::fmt;

use gf2_core::gfp::Fp;

use super::{PackedField, PackedFieldVec};

/// Decode a `(b0, b1, b2)` word-triple into one-hot selectors: a lane's bit
/// of `e[i]` is set iff that lane's value equals `i`. Redundant codepoints
/// `5..=7` set no selector.
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

#[inline]
fn encode5(r: [u64; 5]) -> (u64, u64, u64) {
    let c0 = r[1] | r[3];
    let c1 = r[2] | r[3];
    let c2 = r[4];
    (c0, c1, c2)
}

/// F_5 addition circuit: `r[k]` collects the cells with `(i + j) mod 5 == k`.
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

/// F_5 subtraction circuit: `r[k]` collects the cells with `(i - j) mod 5 == k`.
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

/// F_5 multiplication circuit: `r[k]` collects the cells with `(i * j) mod 5 == k`.
#[inline]
fn mul_circuit(ea: [u64; 5], eb: [u64; 5]) -> (u64, u64, u64) {
    let r1 = (ea[1] & eb[1]) | (ea[2] & eb[3]) | (ea[3] & eb[2]) | (ea[4] & eb[4]);
    let r2 = (ea[1] & eb[2]) | (ea[2] & eb[1]) | (ea[3] & eb[4]) | (ea[4] & eb[3]);
    let r3 = (ea[1] & eb[3]) | (ea[2] & eb[4]) | (ea[3] & eb[1]) | (ea[4] & eb[2]);
    let r4 = (ea[1] & eb[4]) | (ea[2] & eb[2]) | (ea[3] & eb[3]) | (ea[4] & eb[1]);
    encode5([0, r1, r2, r3, r4])
}

/// 64 `F_5` lanes in three `u64` bit-planes `(b0, b1, b2)`, encoded as in the
/// [module docs](self).
#[derive(Copy, Clone)]
pub struct Packed5 {
    b0: u64,
    b1: u64,
    b2: u64,
}

impl PartialEq for Packed5 {
    /// Canonical-decode equality: two values are equal iff every decoded
    /// lane is equal.
    ///
    /// Non-canonical codepoints (5..=7) compare equal to 0.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Redundant codepoints decode to all-zero selectors while canonical 0
        // sets `e[0]`, so only `e[1..=4]` are compared.
        let sa = decode5(self.b0, self.b1, self.b2);
        let sb = decode5(other.b0, other.b1, other.b2);
        sa[1] == sb[1] && sa[2] == sb[2] && sa[3] == sb[3] && sa[4] == sb[4]
    }
}

impl Eq for Packed5 {}

impl core::hash::Hash for Packed5 {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Hash the decoded lane values to be consistent with Eq.
        for i in 0..64usize {
            self.lane(i).value().hash(state);
        }
    }
}

impl Default for Packed5 {
    fn default() -> Self {
        Self::zero()
    }
}

impl fmt::Debug for Packed5 {
    /// Formats as a 64-element array of decoded lane values in `0..=4`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lanes = core::array::from_fn::<u64, 64, _>(|i| self.lane(i).value());
        f.debug_struct("Packed5").field("lanes", &lanes).finish()
    }
}

impl Packed5 {
    /// The bit-plane words `(b0, b1, b2)`: bit `i` of each word belongs to
    /// lane `i`, and bit `k` of a lane's canonical value lives in `bk`.
    /// Public constructors and arithmetic keep every lane in `0..=4`.
    #[must_use]
    #[inline]
    pub const fn to_raw_planes(self) -> (u64, u64, u64) {
        (self.b0, self.b1, self.b2)
    }

    /// [`PackedField::add`] as an inherent method: a fixed proof target for
    /// `proofs/Gf2Algebra/Proofs/Packed5Correctness.lean`, independent of
    /// trait dispatch.
    #[inline]
    pub fn add_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<5>>>::add(self, rhs)
    }

    /// [`PackedField::sub`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn sub_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<5>>>::sub(self, rhs)
    }

    /// [`PackedField::mul`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn mul_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<5>>>::mul(self, rhs)
    }

    /// [`PackedField::neg`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn neg_inherent(self) -> Self {
        <Self as PackedField<Fp<5>>>::neg(self)
    }
}

impl PackedField<Fp<5>> for Packed5 {
    const LANES: usize = 64;

    #[inline]
    fn zero() -> Self {
        Self {
            b0: 0,
            b1: 0,
            b2: 0,
        }
    }

    #[inline]
    fn one() -> Self {
        Self {
            b0: u64::MAX,
            b1: 0,
            b2: 0,
        }
    }

    #[inline]
    fn splat(x: Fp<5>) -> Self {
        let v = x.value();
        let b0_bit = if (v & 1) != 0 { u64::MAX } else { 0 };
        let b1_bit = if (v & 2) != 0 { u64::MAX } else { 0 };
        let b2_bit = if (v & 4) != 0 { u64::MAX } else { 0 };
        Self {
            b0: b0_bit,
            b1: b1_bit,
            b2: b2_bit,
        }
    }

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

    #[inline]
    fn neg(self) -> Self {
        // neg: 0->0, 1->4, 2->3, 3->2, 4->1
        let e = decode5(self.b0, self.b1, self.b2);
        let r = [e[0], e[4], e[3], e[2], e[1]];
        let (c0, c1, c2) = encode5(r);
        Self {
            b0: c0,
            b1: c1,
            b2: c2,
        }
    }

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

    /// Non-canonical codepoints (5..=7) decode to 0.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 64`.
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
        if v < 5 {
            Fp::<5>::new(v)
        } else {
            Fp::<5>::new(0)
        }
    }

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

    /// Redundant codepoints 5..=7 set no selector, so they count as zero.
    #[inline]
    fn all_zero(self) -> bool {
        let e = decode5(self.b0, self.b1, self.b2);
        (e[1] | e[2] | e[3] | e[4]) == 0
    }
}

/// Variable-length packed `F_5` vector: `len_lanes` elements in three
/// parallel `Vec<u64>` planes (`b0`, `b1`, `b2`) of `ceil(len_lanes / 64)`
/// words. Element `i` lives in word `i >> 6` at bit `i & 63` of every plane,
/// in the [`Packed5`] encoding.
///
/// # Mask-tail invariant
///
/// Bits beyond `len_lanes` in the last word of every plane are zero; every
/// mutating operation restores this through `Packed5Vec::mask_tail`.
///
/// # Complexity
///
/// Lane-wise operations are `O(ceil(len_lanes / 64))`; `get` is `O(1)`.
#[derive(Clone)]
pub struct Packed5Vec {
    b0: Vec<u64>,
    b1: Vec<u64>,
    b2: Vec<u64>,
    len_lanes: usize,
}

impl Packed5Vec {
    /// Zero all bits beyond `self.len_lanes` in the last word of every plane.
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
    pub fn neg_assign(&mut self) {
        // neg swaps 1<->4 and 2<->3 by permuting the selectors.
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

impl PartialEq for Packed5Vec {
    /// Canonical-decode equality: two vectors are equal iff they have the
    /// same `len_lanes` and every decoded lane is equal.
    fn eq(&self, other: &Self) -> bool {
        if self.len_lanes != other.len_lanes {
            return false;
        }
        // Only `e[1..=4]` are compared, so redundant and canonical zeros
        // agree; mask_tail keeps padding lanes zero on both sides.
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

impl fmt::Debug for Packed5Vec {
    /// Formats the value as a `Vec` of decoded lane values (each `0..=4`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lanes: Vec<u64> = (0..self.len_lanes).map(|i| self.get(i).value()).collect();
        f.debug_struct("Packed5Vec").field("lanes", &lanes).finish()
    }
}

impl PackedFieldVec<Fp<5>> for Packed5Vec {
    type Element = Packed5;

    fn zeros(len: usize) -> Self {
        let n_words = len.div_ceil(64);
        Self {
            b0: vec![0u64; n_words],
            b1: vec![0u64; n_words],
            b2: vec![0u64; n_words],
            len_lanes: len,
        }
    }

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

    fn len(&self) -> usize {
        self.len_lanes
    }

    /// Non-canonical codepoints (5..=7) decode to 0.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
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

    /// Redundant codepoints 5..=7 set no selector, so they count as zero.
    fn all_zero(&self) -> bool {
        // mask_tail keeps padding lanes zero, so whole words can be tested.
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

impl Packed5 {
    /// Product of the first `n` lanes; lanes `n..64` are ignored.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0` or `n > 64`.
    ///
    /// # Complexity
    ///
    /// `O(n)` scalar lane decodes and multiplications.
    pub fn fold_mul_first_n(self, n: usize) -> Fp<5> {
        assert!(
            (1..=64).contains(&n),
            "Packed5::fold_mul_first_n: n must satisfy 1 <= n <= 64; got n = {n}"
        );
        // F_5 has no bit-sliced halving fold like Bipedal3's, so the active
        // lanes are decoded and multiplied one by one.
        let mut acc = Fp::<5>::new(1); // multiplicative identity
        for i in 0..n {
            let lane_val = self.lane(i);
            acc = acc * lane_val;
        }
        acc
    }
}

/// Rectangular `rows × cols` matrix of `F_5` values, stored column-major as
/// one [`Packed5Vec`] of length `rows` per column, the access pattern of
/// [`crate::permanent::permanent_bipedal5`].
pub struct Packed5Matrix {
    /// One `Packed5Vec` per column, each of length `rows`.
    columns: Vec<Packed5Vec>,
    rows: usize,
    cols: usize,
}

impl PartialEq for Packed5Matrix {
    /// Shape-equal and per-column canonical-decode equal.
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows && self.cols == other.cols && self.columns == other.columns
    }
}

impl Eq for Packed5Matrix {}

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

impl Packed5Matrix {
    /// Construct a matrix from a row-major `Fp<5>` slice.
    ///
    /// The entry at row `i`, column `j` is `data[i * cols + j]`. `rows == 0`
    /// or `cols == 0` is allowed.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != rows * cols`.
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
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Borrow the `j`-th column as a `&Packed5Vec` of length `rows`.
    ///
    /// # Panics
    ///
    /// Panics if `j >= self.cols()`.
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
    /// # Panics
    ///
    /// Panics if `i >= self.rows()` or `j >= self.cols()`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn fp5_strat() -> impl Strategy<Value = Fp<5>> {
        (0u64..5).prop_map(Fp::<5>::new)
    }

    fn packed5_strat() -> impl Strategy<Value = Packed5> {
        prop::collection::vec(fp5_strat(), 64).prop_map(|v| {
            let mut p = Packed5::zero();
            for (i, x) in v.into_iter().enumerate() {
                p = p.with_lane(i, x);
            }
            p
        })
    }

    fn scalar_add(a: u64, b: u64) -> u64 {
        (a + b) % 5
    }

    fn scalar_sub(a: u64, b: u64) -> u64 {
        (a + 5 - b) % 5
    }

    fn scalar_mul(a: u64, b: u64) -> u64 {
        (a * b) % 5
    }

    fn scalar_neg(a: u64) -> u64 {
        (5 - a) % 5
    }

    #[test]
    fn test_lanes_const_is_64() {
        assert_eq!(<Packed5 as PackedField<Fp<5>>>::LANES, 64);
    }

    #[test]
    fn raw_planes_preserve_the_canonical_lane_encoding() {
        let value = Packed5::zero()
            .with_lane(0, Fp::<5>::new(1))
            .with_lane(1, Fp::<5>::new(2))
            .with_lane(2, Fp::<5>::new(3))
            .with_lane(3, Fp::<5>::new(4));
        assert_eq!(value.to_raw_planes(), (0b0101, 0b0110, 0b1000));
    }

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

    proptest! {
        #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

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

        #[test]
        fn test_proptest_neg_matches_scalar(a in packed5_strat()) {
            let r = a.neg();
            for i in 0..64 {
                let expected = scalar_neg(a.lane(i).value());
                prop_assert_eq!(r.lane(i).value(), expected, "neg lane {}", i);
            }
        }
    }

    fn make_vec(len: usize) -> Packed5Vec {
        let xs: Vec<Fp<5>> = (0..len).map(|i| Fp::<5>::new((i as u64) % 5)).collect();
        Packed5Vec::from_field_slice(&xs)
    }

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

                let z = Packed5Vec::zeros(len);
                assert_eq!(z.len(), len);
                assert!(z.all_zero(), "zeros({len}) should be all_zero");
                assert_mask_tail_invariant(&z);

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

    /// Injects redundant codepoints that the public API cannot produce.
    fn packed5_raw(b0: u64, b1: u64, b2: u64) -> Packed5 {
        Packed5 { b0, b1, b2 }
    }

    fn packed5vec_raw(b0: u64, b1: u64, b2: u64, len_lanes: usize) -> Packed5Vec {
        Packed5Vec {
            b0: vec![b0],
            b1: vec![b1],
            b2: vec![b2],
            len_lanes,
        }
    }

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
