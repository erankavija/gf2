//! Packed `F_7` element / vector encoding.
//!
//! Each `u64` packs 16 elements at 4-bit slots; slot `i` occupies bits
//! `[4i .. 4i+4)` and holds a canonical value `0..=6`, so bit `4i+3` is
//! always zero. Binary ops go through three 64 KiB compile-time lookup
//! tables indexed by `a_byte | (b_byte << 8)`, each byte holding two adjacent
//! slots: 8 lookups per `u64`. A byte pair containing a slot value ≥ 7 maps
//! to 0.

use core::fmt;

use gf2_core::gfp::Fp;

use super::{PackedField, PackedFieldVec};

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

/// Addition LUT, indexed by `a_byte | (b_byte << 8)`: each result nibble is
/// `(a + b) % 7` of the matching operand nibbles; 0 when any nibble is ≥ 7.
pub static ADD_LUT: [u8; 65536] = build_add_lut();

/// Subtraction LUT, indexed by `a_byte | (b_byte << 8)`: each result nibble is
/// `(a - b + 7) % 7` of the matching operand nibbles; 0 when any nibble is ≥ 7.
pub static SUB_LUT: [u8; 65536] = build_sub_lut();

/// Multiplication LUT, indexed by `a_byte | (b_byte << 8)`: each result nibble is
/// `(a * b) % 7` of the matching operand nibbles; 0 when any nibble is ≥ 7.
pub static MUL_LUT: [u8; 65536] = build_mul_lut();

/// Apply a binary LUT op to one pair of packed `u64` words: 8 lookups.
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

/// 16 `F_7` lanes in one `u64`, encoded as in the [module docs](self).
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
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Default)]
pub struct Packed7 {
    w: u64,
}

/// Number of F_7 lanes packed into one [`Packed7`].
pub const LANES: usize = 16;

impl Packed7 {
    /// Pack 16 `F_7` elements, lane `i` from `values[i]`.
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
    /// # Panics
    ///
    /// Panics if `i >= LANES`.
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
    #[inline]
    pub fn to_array(self) -> [Fp<7>; 16] {
        core::array::from_fn(|i| self.lane(i))
    }

    /// All-lanes-zero constant.
    #[inline]
    pub fn zero() -> Self {
        Self { w: 0 }
    }

    /// All-lanes-one constant.
    #[inline]
    pub fn one() -> Self {
        Self {
            w: 0x1111_1111_1111_1111u64,
        }
    }

    /// Broadcast scalar `x` to all 16 lanes.
    #[inline]
    pub fn splat(x: Fp<7>) -> Self {
        let v = x.value(); // 0..=6
        let w = v.wrapping_mul(0x1111_1111_1111_1111u64);
        Self { w }
    }

    /// Write the canonical encoding of `x` into lane `i`, returning the updated value.
    ///
    /// # Panics
    ///
    /// Panics if `i >= LANES`.
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
    #[inline]
    pub fn all_zero(self) -> bool {
        self.w == 0
    }

    /// Product of the first `n` lanes; lanes `n..LANES` are ignored.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0` or `n > LANES`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7;
    /// use gf2_core::gfp::Fp;
    ///
    /// // [3, 2, 1, ...] → 3 * 2 * 1 = 6 mod 7
    /// let mut p = Packed7::one();
    /// p = p.with_lane(0, Fp::<7>::new(3));
    /// p = p.with_lane(1, Fp::<7>::new(2));
    /// assert_eq!(p.fold_mul_first_n(2), Fp::<7>::new(6));
    ///
    /// // all-ones, first 3 lanes: 1 * 1 * 1 = 1
    /// assert_eq!(Packed7::one().fold_mul_first_n(3), Fp::<7>::new(1));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` scalar multiplications.
    pub fn fold_mul_first_n(self, n: usize) -> Fp<7> {
        assert!(
            (1..=LANES).contains(&n),
            "Packed7::fold_mul_first_n: n must satisfy 1 <= n <= {LANES}; got n = {n}"
        );
        let mut acc = Fp::<7>::new(1);
        for i in 0..n {
            let nibble = (self.w >> (4 * i)) & 0xf;
            let v = Fp::<7>::new(nibble);
            acc = acc * v;
        }
        acc
    }
}

impl Packed7 {
    /// [`PackedField::add`] as an inherent method: a fixed proof target for
    /// `proofs/Gf2Algebra/Proofs/Packed7Correctness.lean`, independent of
    /// trait dispatch.
    #[inline]
    pub fn add_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<7>>>::add(self, rhs)
    }

    /// [`PackedField::sub`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn sub_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<7>>>::sub(self, rhs)
    }

    /// [`PackedField::mul`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn mul_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<7>>>::mul(self, rhs)
    }

    /// [`PackedField::neg`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn neg_inherent(self) -> Self {
        <Self as PackedField<Fp<7>>>::neg(self)
    }
}

impl PackedField<Fp<7>> for Packed7 {
    const LANES: usize = LANES;

    #[inline]
    fn zero() -> Self {
        Packed7::zero()
    }

    #[inline]
    fn one() -> Self {
        Packed7::one()
    }

    #[inline]
    fn splat(x: Fp<7>) -> Self {
        Packed7::splat(x)
    }

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            w: binary_op_word(self.w, rhs.w, &ADD_LUT),
        }
    }

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            w: binary_op_word(self.w, rhs.w, &SUB_LUT),
        }
    }

    /// `0 - self` through `SUB_LUT`.
    #[inline]
    fn neg(self) -> Self {
        Self {
            w: binary_op_word(0u64, self.w, &SUB_LUT),
        }
    }

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            w: binary_op_word(self.w, rhs.w, &MUL_LUT),
        }
    }

    #[inline]
    fn lane(self, i: usize) -> Fp<7> {
        Packed7::lane(self, i)
    }

    #[inline]
    fn with_lane(self, i: usize, x: Fp<7>) -> Self {
        Packed7::with_lane(self, i, x)
    }

    #[inline]
    fn all_zero(self) -> bool {
        Packed7::all_zero(self)
    }
}

/// Variable-length packed `F_7` vector: `len_lanes` elements in a `Vec<u64>`,
/// 16 elements per word at 4-bit slots.
///
/// # Mask-tail invariant
///
/// Slots beyond `len_lanes` in the last word are zero; every mutating
/// operation restores this through `Packed7Vec::mask_tail`.
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
/// Lane-wise operations are `O(ceil(len_lanes / 16))`; `get` is `O(1)`.
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

    /// Zero all slots beyond `self.len_lanes` in the last word.
    fn mask_tail(&mut self) {
        let n = self.words.len();
        if n == 0 {
            return;
        }
        let used = self.len_lanes - 16 * (n - 1); // lanes in last word
        if used == 16 {
            return; // full word; no padding to mask
        }
        let mask = (1u64 << (4 * used)) - 1;
        self.words[n - 1] &= mask;
    }

    /// Decode logical position `i` to a canonical `F_7` value.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
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
    pub fn neg_assign(&mut self) {
        for w in self.words.iter_mut() {
            *w = binary_op_word(0u64, *w, &SUB_LUT);
        }
        self.mask_tail();
    }

    /// Raw packed words; slots beyond `self.len()` in the last word are zero.
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
    #[inline]
    pub fn raw_words(&self) -> &[u64] {
        &self.words
    }
}

impl PartialEq for Packed7Vec {
    /// Canonical-decode equality: two vectors are equal iff they have the
    /// same `len_lanes` and every decoded lane is equal.
    ///
    /// The mask-tail invariant makes the word-by-word comparison exact.
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

impl fmt::Debug for Packed7Vec {
    /// Formats the value as a `Vec` of decoded lane values (each 0..=6).
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

impl PackedFieldVec<Fp<7>> for Packed7Vec {
    type Element = Packed7;

    fn zeros(len: usize) -> Self {
        let n_words = Self::n_words(len);
        Self {
            words: vec![0u64; n_words],
            len_lanes: len,
        }
    }

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

    #[inline]
    fn len(&self) -> usize {
        self.len_lanes
    }

    #[inline]
    fn get(&self, i: usize) -> Fp<7> {
        Packed7Vec::get(self, i)
    }

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

    fn all_zero(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }
}

/// Rectangular `rows × cols` matrix of packed `F_7` values, stored
/// column-major as one [`Packed7Vec`] of length `rows` per column, the
/// access pattern of `permanent_bipedal7`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Packed7Matrix;
/// use gf2_core::gfp::Fp;
///
/// let data: Vec<Fp<7>> = (0..6u64).map(|v| Fp::<7>::new(v % 7)).collect();
/// let m = Packed7Matrix::from_row_major(&data, 2, 3);
/// assert_eq!(m.rows(), 2);
/// assert_eq!(m.cols(), 3);
/// assert_eq!(m.get(0, 0), Fp::<7>::new(0));
/// assert_eq!(m.get(1, 2), Fp::<7>::new(5));
/// ```
#[derive(Clone)]
pub struct Packed7Matrix {
    /// One `Packed7Vec` per column, each of length `rows`.
    columns: Vec<Packed7Vec>,
    rows: usize,
    cols: usize,
}

impl PartialEq for Packed7Matrix {
    /// Shape-equal and per-column canonical-decode equal.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<7>> = (0..4u64).map(|v| Fp::<7>::new(v % 7)).collect();
    /// let a = Packed7Matrix::from_row_major(&data, 2, 2);
    /// let b = Packed7Matrix::from_row_major(&data, 2, 2);
    /// assert_eq!(a, b);
    ///
    /// let c = Packed7Matrix::from_row_major(&data, 4, 1);
    /// assert_ne!(a, c); // different shape
    /// ```
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows && self.cols == other.cols && self.columns == other.columns
    }
}

impl Eq for Packed7Matrix {}

impl core::fmt::Debug for Packed7Matrix {
    /// Formats as `Packed7Matrix { rows, cols, data: [[row 0], [row 1], ...] }`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data: Vec<Vec<u64>> = (0..self.rows)
            .map(|i| {
                (0..self.cols)
                    .map(|j| self.columns[j].get(i).value())
                    .collect()
            })
            .collect();
        f.debug_struct("Packed7Matrix")
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("data", &data)
            .finish()
    }
}

impl Packed7Matrix {
    /// Construct a matrix from a row-major `Fp<7>` slice.
    ///
    /// The entry at row `i`, column `j` is `data[i * cols + j]`. `rows == 0`
    /// or `cols == 0` is allowed.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != rows * cols`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::Packed7Matrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// // 2×3 matrix
    /// let data: Vec<Fp<7>> = (0..6u64).map(|v| Fp::<7>::new(v % 7)).collect();
    /// let m = Packed7Matrix::from_row_major(&data, 2, 3);
    /// assert_eq!(m.rows(), 2);
    /// assert_eq!(m.cols(), 3);
    /// assert_eq!(m.get(0, 1), Fp::<7>::new(1));
    /// assert_eq!(m.get(1, 0), Fp::<7>::new(3));
    /// ```
    pub fn from_row_major(data: &[Fp<7>], rows: usize, cols: usize) -> Self {
        assert_eq!(
            data.len(),
            rows * cols,
            "Packed7Matrix::from_row_major: data.len() ({}) != rows ({}) * cols ({})",
            data.len(),
            rows,
            cols
        );
        let columns: Vec<Packed7Vec> = (0..cols)
            .map(|j| {
                let col_data: Vec<Fp<7>> = (0..rows).map(|i| data[i * cols + j]).collect();
                Packed7Vec::from_field_slice(&col_data)
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

    /// Decode the element at row `i`, column `j`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.rows()` or `j >= self.cols()`.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> Fp<7> {
        assert!(
            j < self.cols,
            "Packed7Matrix::get: column index {j} out of range (cols = {})",
            self.cols
        );
        self.columns[j].get(i)
    }

    /// Borrow the `j`-th column as a `&Packed7Vec` of length `rows`.
    ///
    /// # Panics
    ///
    /// Panics if `j >= self.cols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::packed::{PackedFieldVec, Packed7Matrix};
    /// use gf2_core::gfp::Fp;
    ///
    /// let data: Vec<Fp<7>> = vec![
    ///     Fp::<7>::new(1), Fp::<7>::new(2),
    ///     Fp::<7>::new(3), Fp::<7>::new(4),
    /// ];
    /// let m = Packed7Matrix::from_row_major(&data, 2, 2);
    /// assert_eq!(m.column(1).get(0), Fp::<7>::new(2));
    /// assert_eq!(m.column(1).get(1), Fp::<7>::new(4));
    /// ```
    #[inline]
    pub fn column(&self, j: usize) -> &Packed7Vec {
        assert!(
            j < self.cols,
            "Packed7Matrix::column: index {j} out of range (cols = {})",
            self.cols
        );
        &self.columns[j]
    }
}

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

        // (a_lo=6, a_hi=6) + (b_lo=1, b_hi=1) → (0, 0)
        let a2: usize = (6 << 4) | 6;
        let b2: usize = (1 << 4) | 1;
        let key2 = a2 | (b2 << 8);
        let r2 = ADD_LUT[key2];
        assert_eq!(r2 & 0xf, scalar_add(6, 1) as u8);
        assert_eq!((r2 >> 4) & 0xf, scalar_add(6, 1) as u8);
    }

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

    // -----------------------------------------------------------------------
    // Exhaustive LUT contract: the `{add,sub,mul}_lut_spec` axioms of
    // `proofs/Gf2Algebra/Proofs/Packed7Correctness.lean` state exactly what
    // these tests check over all 65536 keys.
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_lut_contract_exhaustive() {
        for (key, &got) in ADD_LUT.iter().enumerate() {
            let a0 = (key & 0xf) as u64;
            let a1 = ((key >> 4) & 0xf) as u64;
            let b0 = ((key >> 8) & 0xf) as u64;
            let b1 = ((key >> 12) & 0xf) as u64;
            let expected: u8 = if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                (((a0 + b0) % 7) | (((a1 + b1) % 7) << 4)) as u8
            } else {
                0
            };
            assert_eq!(got, expected, "ADD_LUT[{key:#06x}]");
        }
    }

    #[test]
    fn test_sub_lut_contract_exhaustive() {
        for (key, &got) in SUB_LUT.iter().enumerate() {
            let a0 = (key & 0xf) as u64;
            let a1 = ((key >> 4) & 0xf) as u64;
            let b0 = ((key >> 8) & 0xf) as u64;
            let b1 = ((key >> 12) & 0xf) as u64;
            let expected: u8 = if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                (((a0 + 7 - b0) % 7) | (((a1 + 7 - b1) % 7) << 4)) as u8
            } else {
                0
            };
            assert_eq!(got, expected, "SUB_LUT[{key:#06x}]");
        }
    }

    #[test]
    fn test_mul_lut_contract_exhaustive() {
        for (key, &got) in MUL_LUT.iter().enumerate() {
            let a0 = (key & 0xf) as u64;
            let a1 = ((key >> 4) & 0xf) as u64;
            let b0 = ((key >> 8) & 0xf) as u64;
            let b1 = ((key >> 12) & 0xf) as u64;
            let expected: u8 = if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                (((a0 * b0) % 7) | (((a1 * b1) % 7) << 4)) as u8
            } else {
                0
            };
            assert_eq!(got, expected, "MUL_LUT[{key:#06x}]");
        }
    }

    #[test]
    fn test_lut_non_canonical_yields_zero() {
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

    // -----------------------------------------------------------------------
    // fold_mul_first_n unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_fold_mul_first_n_single_lane() {
        let mut p = Packed7::one();
        p = p.with_lane(0, Fp::<7>::new(5));
        assert_eq!(p.fold_mul_first_n(1), Fp::<7>::new(5));
    }

    #[test]
    fn test_fold_mul_first_n_matches_scalar() {
        let vals: [u64; 16] = [3, 2, 6, 4, 1, 5, 2, 3, 4, 1, 6, 2, 3, 5, 1, 4];
        let arr: [Fp<7>; 16] = core::array::from_fn(|i| Fp::<7>::new(vals[i]));
        let p = Packed7::pack(&arr);
        for n in 1..=16 {
            let expected = vals[..n].iter().fold(1u64, |acc, &v| (acc * v) % 7);
            assert_eq!(
                p.fold_mul_first_n(n).value(),
                expected,
                "fold_mul_first_n({n}) mismatch"
            );
        }
    }

    #[test]
    fn test_fold_mul_first_n_all_ones() {
        let p = Packed7::one();
        for n in 1..=16 {
            assert_eq!(p.fold_mul_first_n(n), Fp::<7>::new(1), "n={n}");
        }
    }

    #[test]
    fn test_fold_mul_first_n_zero_lane() {
        let mut p = Packed7::one();
        p = p.with_lane(3, Fp::<7>::new(0));
        // Product of lanes 0..4 contains a zero
        assert_eq!(p.fold_mul_first_n(4), Fp::<7>::new(0));
    }

    #[test]
    #[should_panic(expected = "n must satisfy 1 <= n")]
    fn test_fold_mul_first_n_panic_n0() {
        let _ = Packed7::one().fold_mul_first_n(0);
    }

    #[test]
    #[should_panic(expected = "n must satisfy 1 <= n")]
    fn test_fold_mul_first_n_panic_n17() {
        let _ = Packed7::one().fold_mul_first_n(17);
    }

    // -----------------------------------------------------------------------
    // Inherent wrappers agree with the trait methods
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_inherent_matches_trait() {
        for a in 0u64..7 {
            for b in 0u64..7 {
                let pa = Packed7::splat(Fp::<7>::new(a));
                let pb = Packed7::splat(Fp::<7>::new(b));
                let via_inherent = Packed7::add_inherent(pa, pb);
                let via_trait = pa.add(pb);
                assert_eq!(via_inherent, via_trait, "add_inherent({a},{b})");
            }
        }
    }

    #[test]
    fn test_sub_inherent_matches_trait() {
        for a in 0u64..7 {
            for b in 0u64..7 {
                let pa = Packed7::splat(Fp::<7>::new(a));
                let pb = Packed7::splat(Fp::<7>::new(b));
                let via_inherent = Packed7::sub_inherent(pa, pb);
                let via_trait = pa.sub(pb);
                assert_eq!(via_inherent, via_trait, "sub_inherent({a},{b})");
            }
        }
    }

    #[test]
    fn test_mul_inherent_matches_trait() {
        for a in 0u64..7 {
            for b in 0u64..7 {
                let pa = Packed7::splat(Fp::<7>::new(a));
                let pb = Packed7::splat(Fp::<7>::new(b));
                let via_inherent = Packed7::mul_inherent(pa, pb);
                let via_trait = pa.mul(pb);
                assert_eq!(via_inherent, via_trait, "mul_inherent({a},{b})");
            }
        }
    }

    #[test]
    fn test_neg_inherent_matches_trait() {
        for a in 0u64..7 {
            let pa = Packed7::splat(Fp::<7>::new(a));
            let via_inherent = Packed7::neg_inherent(pa);
            let via_trait = pa.neg();
            assert_eq!(via_inherent, via_trait, "neg_inherent({a})");
        }
    }

    // -----------------------------------------------------------------------
    // Packed7Vec — PartialEq, Debug, panics, raw_words, is_empty
    // -----------------------------------------------------------------------

    #[test]
    fn test_packed7vec_partialeq_equal() {
        let xs: Vec<Fp<7>> = (0..20).map(|i| Fp::<7>::new((i as u64) % 7)).collect();
        let a = Packed7Vec::from_field_slice(&xs);
        let b = Packed7Vec::from_field_slice(&xs);
        assert_eq!(a, b);
    }

    #[test]
    fn test_packed7vec_partialeq_different_values() {
        let xs1: Vec<Fp<7>> = vec![Fp::<7>::new(1), Fp::<7>::new(2)];
        let xs2: Vec<Fp<7>> = vec![Fp::<7>::new(1), Fp::<7>::new(3)];
        let a = Packed7Vec::from_field_slice(&xs1);
        let b = Packed7Vec::from_field_slice(&xs2);
        assert_ne!(a, b);
    }

    #[test]
    fn test_packed7vec_partialeq_different_len() {
        let xs1: Vec<Fp<7>> = vec![Fp::<7>::new(1)];
        let xs2: Vec<Fp<7>> = vec![Fp::<7>::new(1), Fp::<7>::new(1)];
        let a = Packed7Vec::from_field_slice(&xs1);
        let b = Packed7Vec::from_field_slice(&xs2);
        assert_ne!(a, b);
    }

    #[test]
    fn test_packed7vec_debug_contains_lanes() {
        let xs: Vec<Fp<7>> = vec![Fp::<7>::new(3), Fp::<7>::new(6)];
        let v = Packed7Vec::from_field_slice(&xs);
        let s = format!("{v:?}");
        assert!(
            s.contains("lanes"),
            "debug output should contain 'lanes': {s}"
        );
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_packed7vec_get_panic_out_of_range() {
        let v = Packed7Vec::zeros(3);
        let _ = v.get(3);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_packed7vec_add_assign_length_mismatch() {
        let mut a = Packed7Vec::zeros(2);
        let b = Packed7Vec::zeros(3);
        a.add_assign(&b);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_packed7vec_sub_assign_length_mismatch() {
        let mut a = Packed7Vec::zeros(2);
        let b = Packed7Vec::zeros(3);
        a.sub_assign(&b);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_packed7vec_mul_assign_length_mismatch() {
        let mut a = Packed7Vec::zeros(2);
        let b = Packed7Vec::zeros(3);
        a.mul_assign(&b);
    }

    #[test]
    fn test_packed7vec_raw_words_encodes_lanes() {
        // Lane 0 = 3 (nibble 3), lane 1 = 5 (nibble 5) → first byte = 0x53
        let xs: Vec<Fp<7>> = vec![Fp::<7>::new(3), Fp::<7>::new(5)];
        let v = Packed7Vec::from_field_slice(&xs);
        let words = v.raw_words();
        assert!(!words.is_empty());
        assert_eq!(words[0] & 0xff, (5u64 << 4) | 3u64);
    }

    #[test]
    fn test_packed7vec_is_empty() {
        assert!(Packed7Vec::zeros(0).is_empty());
        assert!(!Packed7Vec::zeros(1).is_empty());
        assert!(!Packed7Vec::zeros(16).is_empty());
    }

    #[test]
    fn test_packed7vec_neg_assign_correctness() {
        let xs: Vec<Fp<7>> = (0..7).map(|i| Fp::<7>::new(i as u64)).collect();
        let mut v = Packed7Vec::from_field_slice(&xs);
        v.neg_assign();
        for i in 0..7usize {
            let expected = scalar_neg(i as u64);
            assert_eq!(v.get(i).value(), expected, "neg_assign lane {i}");
        }
    }

    // -----------------------------------------------------------------------
    // Packed7Matrix — construction, access, panics, Debug, PartialEq
    // -----------------------------------------------------------------------

    #[test]
    fn test_packed7matrix_shape_and_get() {
        let rows = 3usize;
        let cols = 4usize;
        let data: Vec<Fp<7>> = (0..(rows * cols) as u64)
            .map(|v| Fp::<7>::new(v % 7))
            .collect();
        let m = Packed7Matrix::from_row_major(&data, rows, cols);
        assert_eq!(m.rows(), rows);
        assert_eq!(m.cols(), cols);
        for i in 0..rows {
            for j in 0..cols {
                assert_eq!(m.get(i, j), data[i * cols + j], "get({i},{j}) mismatch");
            }
        }
    }

    #[test]
    fn test_packed7matrix_empty_rows() {
        let m = Packed7Matrix::from_row_major(&[], 0, 5);
        assert_eq!(m.rows(), 0);
        assert_eq!(m.cols(), 5);
    }

    #[test]
    fn test_packed7matrix_empty_cols() {
        let m = Packed7Matrix::from_row_major(&[], 5, 0);
        assert_eq!(m.rows(), 5);
        assert_eq!(m.cols(), 0);
    }

    #[test]
    fn test_packed7matrix_column_access() {
        let rows = 4usize;
        let cols = 3usize;
        let data: Vec<Fp<7>> = (0..(rows * cols) as u64)
            .map(|v| Fp::<7>::new(v % 7))
            .collect();
        let m = Packed7Matrix::from_row_major(&data, rows, cols);
        for j in 0..cols {
            let col = m.column(j);
            assert_eq!(col.len(), rows);
            for i in 0..rows {
                assert_eq!(col.get(i), data[i * cols + j], "column({j}).get({i})");
            }
        }
    }

    #[test]
    fn test_packed7matrix_partialeq_equal() {
        let data: Vec<Fp<7>> = (0..9u64).map(|v| Fp::<7>::new(v % 7)).collect();
        let a = Packed7Matrix::from_row_major(&data, 3, 3);
        let b = Packed7Matrix::from_row_major(&data, 3, 3);
        assert_eq!(a, b);
    }

    #[test]
    fn test_packed7matrix_partialeq_different_values() {
        let data1: Vec<Fp<7>> = (0..4u64).map(|v| Fp::<7>::new(v % 7)).collect();
        let data2: Vec<Fp<7>> = (1..5u64).map(|v| Fp::<7>::new(v % 7)).collect();
        let a = Packed7Matrix::from_row_major(&data1, 2, 2);
        let b = Packed7Matrix::from_row_major(&data2, 2, 2);
        assert_ne!(a, b);
    }

    #[test]
    fn test_packed7matrix_partialeq_different_shape() {
        let data: Vec<Fp<7>> = (0..4u64).map(|v| Fp::<7>::new(v % 7)).collect();
        let a = Packed7Matrix::from_row_major(&data, 2, 2);
        let b = Packed7Matrix::from_row_major(&data, 4, 1);
        assert_ne!(a, b);
    }

    #[test]
    fn test_packed7matrix_debug_format() {
        let data = vec![Fp::<7>::new(1), Fp::<7>::new(2)];
        let m = Packed7Matrix::from_row_major(&data, 1, 2);
        let s = format!("{m:?}");
        assert!(s.contains("rows"), "debug output missing 'rows': {s}");
        assert!(s.contains("cols"), "debug output missing 'cols': {s}");
    }

    #[test]
    #[should_panic(expected = "data.len()")]
    fn test_packed7matrix_from_row_major_length_mismatch_panic() {
        let data = vec![Fp::<7>::new(0); 5];
        let _ = Packed7Matrix::from_row_major(&data, 2, 3); // needs 6, got 5
    }

    #[test]
    #[should_panic(expected = "column index")]
    fn test_packed7matrix_get_col_out_of_range_panic() {
        let data: Vec<Fp<7>> = vec![Fp::<7>::new(1), Fp::<7>::new(2)];
        let m = Packed7Matrix::from_row_major(&data, 1, 2);
        let _ = m.get(0, 2);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_packed7matrix_column_out_of_range_panic() {
        let data: Vec<Fp<7>> = vec![Fp::<7>::new(1), Fp::<7>::new(2)];
        let m = Packed7Matrix::from_row_major(&data, 1, 2);
        let _ = m.column(2);
    }

    #[test]
    fn test_packed7matrix_permanent_cross_check() {
        use crate::permanent::bipedal7::permanent_bipedal7_singleword;
        use crate::permanent::ryser::permanent_ryser;

        // [[1,2,3],[4,5,6],[0,1,2]] mod 7
        let data: Vec<Fp<7>> = [1u64, 2, 3, 4, 5, 6, 0, 1, 2]
            .iter()
            .map(|&v| Fp::<7>::new(v))
            .collect();
        let m = Packed7Matrix::from_row_major(&data, 3, 3);
        let bipedal = permanent_bipedal7_singleword(&m);
        let ryser = permanent_ryser::<Fp<7>>(&data, 3);
        assert_eq!(bipedal, ryser, "permanent mismatch for 3×3 matrix");
    }
}
