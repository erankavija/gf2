//! Binary-field-specific polynomial helpers.
//!
//! These inherent methods are attached to
//! [`FieldPoly<Gf2mElement_<V>>`](crate::field::FieldPoly) (and therefore
//! also to the [`Gf2mPoly_<V>`](crate::gf2m::Gf2mPoly_) alias) because
//! they rely on the GF(2^m) encoding of field elements as bits rather
//! than on the generic [`FiniteField`](crate::field::FiniteField)
//! surface.
//!
//! Callers that need these routines across a generic field type do not
//! get the helpers; the operations themselves are meaningless outside a
//! field whose elements admit a binary encoding.
//!
//! # Helpers
//!
//! - [`FieldPoly::<Gf2mElement_<V>>::from_bitvec`]
//! - [`FieldPoly::<Gf2mElement_<V>>::to_bitvec`]
//! - [`FieldPoly::<Gf2mElement_<V>>::to_bitvec_minimal`]
//! - [`FieldPoly::<Gf2mElement_<V>>::from_bitvec_reversed`]
//! - [`FieldPoly::<Gf2mElement_<V>>::to_bitvec_reversed`]
//! - [`FieldPoly::<Gf2mElement_<V>>::from_exponents`]
//! - [`FieldPoly::<Gf2mElement_<V>>::x`]
//! - [`FieldPoly::<Gf2mElement_<V>>::zero`] (field-aware legacy shim)

use crate::field::FieldPoly;
use crate::gf2m::{Gf2mElement_, Gf2mField_, UintExt};
use crate::BitVec;

impl<V: UintExt> FieldPoly<Gf2mElement_<V>> {
    /// Constructs a polynomial from a BitVec over GF(2^m).
    ///
    /// Each bit in the BitVec is interpreted as a coefficient in
    /// GF(2^m):
    /// - `false` (0) → `field.zero()`
    /// - `true` (1)  → `field.one()`
    ///
    /// The polynomial is in ascending-degree order: bit `i` is the
    /// coefficient of `x^i`.
    ///
    /// # Arguments
    ///
    /// * `bits` — BitVec containing binary coefficients.
    /// * `field` — the field to use when creating elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut bits = BitVec::new();
    /// bits.push_bit(true);  // x^0
    /// bits.push_bit(false); // x^1
    /// bits.push_bit(true);  // x^2
    ///
    /// let poly = Gf2mPoly::from_bitvec(&bits, &field);
    /// assert_eq!(poly.degree(), Some(2));
    /// assert!(poly.coeff(0).is_one());
    /// assert!(poly.coeff(1).is_zero());
    /// assert!(poly.coeff(2).is_one());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(bits.len())`.
    pub fn from_bitvec(bits: &BitVec, field: &Gf2mField_<V>) -> Self {
        if bits.is_empty() {
            return FieldPoly::zero_like(&field.zero());
        }

        let coeffs: Vec<Gf2mElement_<V>> = (0..bits.len())
            .map(|i| {
                if bits.get(i) {
                    field.one()
                } else {
                    field.zero()
                }
            })
            .collect();

        FieldPoly::new(coeffs)
    }

    /// Converts the polynomial to a `BitVec` by extracting the binary
    /// flag of every coefficient (is-non-zero).
    ///
    /// # Arguments
    ///
    /// * `len` — desired length of the output `BitVec` (may exceed
    ///   polynomial degree).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = Gf2mPoly::new(vec![field.one(), field.zero(), field.one()]);
    ///
    /// let bits = poly.to_bitvec(5);
    /// assert_eq!(bits.len(), 5);
    /// assert!(bits.get(0));
    /// assert!(!bits.get(1));
    /// assert!(bits.get(2));
    /// assert!(!bits.get(3));
    /// assert!(!bits.get(4));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(len)`.
    pub fn to_bitvec(&self, len: usize) -> BitVec {
        let mut bits = BitVec::new();
        for i in 0..len {
            let is_nonzero = self.try_coeff(i).map(|c| !c.is_zero()).unwrap_or(false);
            bits.push_bit(is_nonzero);
        }
        bits
    }

    /// Converts the polynomial to a `BitVec` with minimal length
    /// (`degree + 1`).
    ///
    /// Returns an empty `BitVec` for the zero polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = Gf2mPoly::new(vec![field.one(), field.zero(), field.one()]);
    ///
    /// let bits = poly.to_bitvec_minimal();
    /// assert_eq!(bits.len(), 3);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(degree)`.
    pub fn to_bitvec_minimal(&self) -> BitVec {
        let len = self.degree().map(|d| d + 1).unwrap_or(0);
        self.to_bitvec(len)
    }

    /// Constructs a polynomial from a `BitVec` with reversed-coefficient
    /// mapping: `bit[i]` feeds coefficient `x^(n-1-i)`.
    ///
    /// This matches the DVB-T2 convention of "MSB-first" codewords where
    /// bit `0` is the highest-degree coefficient.
    ///
    /// # Arguments
    ///
    /// * `bits` — source bits.
    /// * `field` — field to use when creating elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut bits = BitVec::new();
    /// bits.push_bit(true);  // -> x^2
    /// bits.push_bit(false); // -> x^1
    /// bits.push_bit(true);  // -> x^0
    ///
    /// let poly = Gf2mPoly::from_bitvec_reversed(&bits, &field);
    /// assert_eq!(poly.degree(), Some(2));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(bits.len())`.
    pub fn from_bitvec_reversed(bits: &BitVec, field: &Gf2mField_<V>) -> Self {
        if bits.is_empty() {
            return FieldPoly::zero_like(&field.zero());
        }

        let n = bits.len();
        let coeffs: Vec<Gf2mElement_<V>> = (0..n)
            .map(|i| {
                let bit_index = n - 1 - i;
                if bits.get(bit_index) {
                    field.one()
                } else {
                    field.zero()
                }
            })
            .collect();

        FieldPoly::new(coeffs)
    }

    /// Converts the polynomial to a `BitVec` with reversed-coefficient
    /// mapping: coefficient of `x^i` ends up at `bit[len-1-i]`.
    ///
    /// # Arguments
    ///
    /// * `len` — desired length of the output `BitVec`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = Gf2mPoly::new(vec![field.one(), field.zero(), field.one()]);
    ///
    /// let bits = poly.to_bitvec_reversed(5);
    /// assert_eq!(bits.len(), 5);
    /// assert!(bits.get(2));  // x^2 at bit 2 (len - 1 - 2)
    /// assert!(bits.get(4));  // x^0 at bit 4
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(len)`.
    pub fn to_bitvec_reversed(&self, len: usize) -> BitVec {
        let mut bits = BitVec::new();
        if len == 0 {
            return bits;
        }
        for i in 0..len {
            let degree = len - 1 - i;
            let is_nonzero = self
                .try_coeff(degree)
                .map(|c| !c.is_zero())
                .unwrap_or(false);
            bits.push_bit(is_nonzero);
        }
        bits
    }

    /// Creates a polynomial from a list of exponents.
    ///
    /// Each exponent in the list corresponds to a term with coefficient
    /// `1`. Duplicate exponents cancel in GF(2) (even occurrences drop
    /// to zero; odd occurrences keep the term).
    ///
    /// # Arguments
    ///
    /// * `field` — field over which the polynomial is defined.
    /// * `exponents` — slice of exponents to include.
    ///
    /// # Panics
    ///
    /// Panics if `exponents` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = Gf2mPoly::from_exponents(&field, &[0, 1, 4]);
    ///
    /// assert_eq!(poly.degree(), Some(4));
    /// assert_eq!(poly.coeff(0), field.one());
    /// assert_eq!(poly.coeff(1), field.one());
    /// assert_eq!(poly.coeff(2), field.zero());
    /// assert_eq!(poly.coeff(3), field.zero());
    /// assert_eq!(poly.coeff(4), field.one());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(max(exponents))`.
    pub fn from_exponents(field: &Gf2mField_<V>, exponents: &[usize]) -> Self {
        assert!(!exponents.is_empty(), "exponents cannot be empty");

        let max_exp = exponents.iter().copied().max().unwrap();
        let mut coeffs = vec![field.zero(); max_exp + 1];

        for &exp in exponents {
            coeffs[exp] = &coeffs[exp] + &field.one();
        }

        FieldPoly::new(coeffs)
    }

    /// Creates the indeterminate `x` as a polynomial over `field`.
    ///
    /// Equivalent to
    /// [`FieldPoly::monomial`](crate::field::FieldPoly::monomial)`(field.one(), 1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let x = Gf2mPoly::x(&field);
    /// assert_eq!(x.degree(), Some(1));
    /// assert!(x.coeff(0).is_zero());
    /// assert!(x.coeff(1).is_one());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn x(field: &Gf2mField_<V>) -> Self {
        FieldPoly::monomial(field.one(), 1)
    }

    /// Legacy shim: returns the zero polynomial in the same field as
    /// `field`.
    ///
    /// Equivalent to
    /// [`FieldPoly::zero_like`](crate::field::FieldPoly::zero_like)`(&field.zero())`
    /// and is preserved so existing BCH / Reed–Solomon call-sites that
    /// historically used `Gf2mPoly::zero(&field)` continue to compile.
    ///
    /// New code should prefer
    /// [`FieldPoly::zero_like`](crate::field::FieldPoly::zero_like).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let z = Gf2mPoly::zero(&field);
    /// assert!(z.is_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn zero(field: &Gf2mField_<V>) -> Self {
        FieldPoly::zero_like(&field.zero())
    }
}
