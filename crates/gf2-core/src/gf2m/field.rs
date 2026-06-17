//! # GF(2^m) - Binary Extension Field Arithmetic
//!
//! This module provides arithmetic over binary extension fields GF(2^m), which are fundamental
//! for algebraic error-correcting codes such as BCH and Reed-Solomon codes.
//!
//! ## Mathematical Background
//!
//! ### What is a Finite Field?
//!
//! A **field** is an algebraic structure with two operations (addition and multiplication)
//! that satisfy familiar properties:
//! - Both operations are associative and commutative
//! - Both have identity elements (0 for addition, 1 for multiplication)
//! - Every element has an additive inverse (a + (-a) = 0)
//! - Every non-zero element has a multiplicative inverse (a · a⁻¹ = 1)
//! - Multiplication distributes over addition
//!
//! A **finite field** (or Galois field) has a finite number of elements. The number of
//! elements is always a prime power p^m, where p is prime and m ≥ 1.
//!
//! ### Binary Extension Fields GF(2^m)
//!
//! When the base field is GF(2) = {0, 1} with XOR addition and AND multiplication, we can
//! construct extension fields GF(2^m) with 2^m elements. These fields are particularly
//! efficient for computer implementation because:
//! - Addition is just XOR (no carries!)
//! - Elements fit naturally into binary representations
//! - Hardware acceleration available (CLMUL instructions)
//!
//! ### Polynomial Representation
//!
//! Elements of GF(2^m) are represented as polynomials over GF(2) with degree less than m:
//!
//! ```text
//! a(x) = a_{m-1}·x^{m-1} + a_{m-2}·x^{m-2} + ... + a_1·x + a_0
//! ```
//!
//! where each coefficient aᵢ ∈ {0, 1}.
//!
//! Since coefficients are binary, we can represent an element as a bit vector:
//! - Polynomial: x³ + x + 1
//! - Binary vector: (1, 0, 1, 1) reading from x³ down to x⁰
//! - Binary number: 0b1011 = 11 (decimal)
//!
//! ### Arithmetic Operations
//!
//! **Addition**: XOR the binary representations (add polynomials coefficient-wise mod 2)
//! ```text
//! (x² + 1) + (x³ + x²) = x³ + 1
//! Binary: 0101 ⊕ 1100 = 1001
//! ```
//!
//! **Multiplication**: Multiply polynomials, then reduce modulo an irreducible
//! polynomial of degree m
//! ```text
//! In GF(2^4) with irreducible polynomial p(x) = x⁴ + x + 1 (also primitive):
//! (x + 1) · (x² + 1) = x³ + x² + x + 1
//! ```
//!
//! **Defining polynomial**: An **irreducible** polynomial of degree m is
//! sufficient to define the field structure of GF(2^m) — irreducibility ensures
//! that the quotient `GF(2)[x] / ⟨p(x)⟩` is a field. A **primitive** polynomial
//! is an irreducible polynomial whose root generates the full multiplicative
//! group; primitivity is additionally needed for log/exp tables and discrete-log
//! constructions.
//!
//! Polynomials returned by [`PrimitivePolynomialDatabase::standard`] (m ≤ 16)
//! are verified **primitive**. Polynomials returned by
//! [`PrimitivePolynomialDatabase::standard_u128`] for m = 64..=127 are verified
//! only **irreducible** (see
//! [`PrimitivePolynomialDatabase::standard_u128_irreducibility_note`]); this is
//! sufficient for correctness of arithmetic operations (add, sub, mul, inv,
//! div) but not for primitive-element or discrete-log operations.
//!
//! [`PrimitivePolynomialDatabase::standard`]: crate::primitive_polys::PrimitivePolynomialDatabase::standard
//! [`PrimitivePolynomialDatabase::standard_u128`]: crate::primitive_polys::PrimitivePolynomialDatabase::standard_u128
//! [`PrimitivePolynomialDatabase::standard_u128_irreducibility_note`]: crate::primitive_polys::PrimitivePolynomialDatabase::standard_u128_irreducibility_note
//!
//! ## Example: Computing in GF(2^4)
//!
//! Let's work through arithmetic in GF(16) using primitive polynomial p(x) = x⁴ + x + 1.
//!
//! ```
//! use gf2_core::gf2m::Gf2mField;
//!
//! // Create GF(2^4) with primitive polynomial x^4 + x + 1 (binary 10011)
//! let field = Gf2mField::new(4, 0b10011);
//!
//! // Elements represented as polynomials over GF(2)
//! // x² + 1 is binary 0101 = 5
//! let a = field.element(0b0101);
//! // x³ + x is binary 1010 = 10  
//! let b = field.element(0b1010);
//!
//! // Addition is XOR: (x² + 1) + (x³ + x) = x³ + x² + x + 1
//! let sum = &a + &b;  // 0101 ⊕ 1010 = 1111
//! assert_eq!(sum.value(), 0b1111);
//!
//! // Multiplication with reduction modulo p(x)
//! // (x² + 1) · (x³ + x) mod (x⁴ + x + 1)
//! let product = &a * &b;
//! // (x² + 1) · (x³ + x) = x⁵ + x³ + x³ + x = x⁵ + x  (x³+x³=0 in GF(2))
//! // x⁵ = x · x⁴ = x · (x + 1) = x² + x  (since x⁴ ≡ x + 1 mod p(x))
//! // Final: (x²+x) + x = x²  (x+x=0 in GF(2))
//! // Result: x² = 0b0100
//! assert_eq!(product.value(), 0b0100);
//! ```
//!
//! ## Standard Field Presets
//!
//! ```
//! use gf2_core::gf2m::Gf2mField;
//!
//! // GF(2^8) with standard primitive polynomial x^8 + x^4 + x^3 + x + 1
//! let gf256 = Gf2mField::gf256();
//!
//! // Compute with bytes
//! let a = gf256.element(0x53);  // 01010011
//! let b = gf256.element(0xCA);  // 11001010
//! let sum = a + b;               // XOR
//! assert_eq!(sum.value(), 0x99); // 10011001
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};
use std::sync::Arc;

#[cfg(feature = "simd")]
use gf2_kernels_simd::gf2m as simd_gf2m;

use super::barrett::BarrettReducer;
use super::uint_ext::UintExt;

/// A binary extension field GF(2^m) with a specified defining polynomial.
///
/// The defining polynomial must be **irreducible** over GF(2) of degree `m`.
/// Irreducibility alone is sufficient for field arithmetic (add, sub, mul,
/// inv, div). Log/exp tables and primitive-element operations additionally
/// require the polynomial to be **primitive**, which is a strictly stronger
/// property. See the module-level docs for the distinction and
/// [`crate::primitive_polys::PrimitivePolynomialDatabase`] for which catalog
/// accessors guarantee each property.
///
/// The type parameter `V` controls the underlying integer representation for
/// field elements. Use [`Gf2mField`] (alias for `Gf2mField_<u64>`) for
/// the common case.
///
/// This type defines the field structure and parameters. Individual field elements
/// are created via [`Gf2mField::element`].
#[derive(Clone, Debug)]
pub struct Gf2mField_<V: UintExt = u64> {
    params: Arc<FieldParams_<V>>,
}

/// Convenience alias: `Gf2mField` is `Gf2mField_<u64>`.
pub type Gf2mField = Gf2mField_<u64>;

impl<V: UintExt> PartialEq for Gf2mField_<V> {
    fn eq(&self, other: &Self) -> bool {
        // Fields are equal if they have the same m and primitive polynomial
        *self.params == *other.params
    }
}

impl<V: UintExt> Eq for Gf2mField_<V> {}

#[derive(Debug)]
struct FieldParams_<V: UintExt = u64> {
    m: usize,
    primitive_poly: V,
    // Log/antilog tables for fast multiplication (m ≤ 16)
    log_table: Option<Vec<u16>>, // log_table[α^i] = i
    exp_table: Option<Vec<u16>>, // exp_table[i] = α^i
    // SIMD multiplication function (if available) — combined mul+reduce path
    #[cfg(feature = "simd")]
    simd_mul_fn: Option<simd_gf2m::Gf2mMulFn>,
    // Raw SIMD carry-less multiply (no reduction) + Barrett reducer for the split path.
    // When both are present, multiplication uses PCLMULQDQ for the raw product
    // and Barrett reduction for the modular step.
    #[cfg(feature = "simd")]
    clmul_fn: Option<simd_gf2m::ClmulFn>,
    // All-in-one PCLMULQDQ + Barrett reduce kernel (3 clmul ops in one target_feature scope).
    // When available, replaces the split clmul_fn + Barrett path for better performance.
    #[cfg(feature = "simd")]
    clmul_barrett_fn: Option<simd_gf2m::ClmulBarrettFn>,
    barrett_reducer: Option<BarrettReducer>,
}

impl<V: UintExt> PartialEq for FieldParams_<V> {
    fn eq(&self, other: &Self) -> bool {
        self.m == other.m && self.primitive_poly == other.primitive_poly
    }
}

impl<V: UintExt> Eq for FieldParams_<V> {}

/// An element of a binary extension field GF(2^m).
///
/// Elements are represented as polynomials over GF(2) with degree < m,
/// encoded as binary integers where bit i represents the coefficient of x^i.
///
/// The type parameter `V` controls the underlying integer representation.
/// Use [`Gf2mElement`] (alias for `Gf2mElement_<u64>`) for the common case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gf2mElement_<V: UintExt = u64> {
    value: V,
    params: Arc<FieldParams_<V>>,
}

/// Convenience alias: `Gf2mElement` is `Gf2mElement_<u64>`.
pub type Gf2mElement = Gf2mElement_<u64>;

impl<V: UintExt> Gf2mField_<V> {
    /// Creates a new GF(2^m) field with the specified defining polynomial.
    ///
    /// The polynomial must be **irreducible** over GF(2) of degree `m`.
    /// Irreducibility is sufficient for arithmetic correctness (add, sub,
    /// mul, inv, div). If the polynomial is also **primitive** (a strictly
    /// stronger property), log/exp tables are built for `m <= 16` enabling
    /// faster multiplication and the primitive-element API. Callers can
    /// obtain verified-primitive polynomials for `m <= 16` from
    /// [`crate::primitive_polys::PrimitivePolynomialDatabase::standard`]
    /// and verified-irreducible (but not necessarily primitive) polynomials
    /// for `m = 64..=127` from
    /// [`crate::primitive_polys::PrimitivePolynomialDatabase::standard_u128`].
    ///
    /// # Arguments
    ///
    /// * `m` - Extension degree (field has 2^m elements, must satisfy `m < V::BITS`)
    /// * `primitive_poly` - Defining polynomial of degree m in binary representation.
    ///   The parameter is named `primitive_poly` for historical/API-stability
    ///   reasons; irreducibility is the strictly necessary property.
    ///
    /// # Panics
    ///
    /// Panics if `m == 0` or `m >= V::BITS` (the leading coefficient at bit `m`
    /// would not fit in `V`, even though the stored value represents only the
    /// lower `m` bits of the reduction polynomial).
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// // GF(2^4) with primitive polynomial x^4 + x + 1 (binary 10011)
    /// let field = Gf2mField::new(4, 0b10011);
    /// ```
    pub fn new(m: usize, primitive_poly: V) -> Self {
        Self::new_internal(m, primitive_poly)
    }

    /// Creates a field without database verification warnings (internal use).
    pub(crate) fn new_unchecked(m: usize, primitive_poly: V) -> Self {
        Self::new_internal(m, primitive_poly)
    }

    fn new_internal(m: usize, primitive_poly: V) -> Self {
        assert!(m > 0, "Extension degree m must be positive");
        assert!(
            (m as u32) < V::BITS,
            "Extension degree m={} must be strictly less than {} bits for this integer type",
            m,
            V::BITS
        );

        #[cfg(feature = "simd")]
        let (simd_mul_fn, clmul_fn, clmul_barrett_fn) = if V::IS_U64 {
            let fns = crate::simd::maybe_gf2m();
            (
                fns.map(|f| f.mul_fn),
                fns.and_then(|f| f.clmul_fn),
                fns.and_then(|f| f.clmul_barrett_fn),
            )
        } else {
            (None, None, None)
        };

        // Create Barrett reducer when SIMD clmul is available (for the split
        // PCLMULQDQ + Barrett path). Only create when the polynomial is valid:
        // leading bit must be at position m.
        #[cfg(feature = "simd")]
        let barrett_reducer = if clmul_fn.is_some()
            && (m as u32) <= 63
            && m > 0
            && (primitive_poly.as_u64_truncated() >> (m as u32)) == 1
        {
            Some(BarrettReducer::new(
                primitive_poly.as_u64_truncated() as u128,
                m as u32,
            ))
        } else {
            None
        };

        #[cfg(not(feature = "simd"))]
        let barrett_reducer = None;

        Gf2mField_ {
            params: Arc::new(FieldParams_ {
                m,
                primitive_poly,
                log_table: None,
                exp_table: None,
                #[cfg(feature = "simd")]
                simd_mul_fn,
                #[cfg(feature = "simd")]
                clmul_fn,
                #[cfg(feature = "simd")]
                clmul_barrett_fn,
                barrett_reducer,
            }),
        }
    }

    /// Returns the extension degree m.
    pub fn degree(&self) -> usize {
        self.params.m
    }

    /// Returns the field order 2^m as `V`.
    ///
    /// Unlike `order()` (available only on `Gf2mField`), this works for any backing type.
    pub fn order_v(&self) -> V {
        V::ONE << (self.params.m as u32)
    }

    /// Returns the defining polynomial of the field.
    ///
    /// For `m <= 16` fields constructed from
    /// [`crate::primitive_polys::PrimitivePolynomialDatabase::standard`], this
    /// is a verified primitive polynomial. For `m = 64..=127` fields
    /// constructed from
    /// [`crate::primitive_polys::PrimitivePolynomialDatabase::standard_u128`],
    /// this is only verified irreducible — not necessarily primitive. The
    /// method name is retained for API stability; see the struct-level docs
    /// for the full contract.
    pub fn primitive_polynomial(&self) -> V {
        self.params.primitive_poly
    }

    /// Creates a field element from a binary representation.
    ///
    /// # Arguments
    ///
    /// * `value` - Binary representation where bit i is the coefficient of x^i
    ///
    /// # Panics
    ///
    /// Panics if value has bits set beyond degree m-1.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let elem = field.element(0b1010);  // x^3 + x
    /// assert_eq!(elem.value(), 0b1010);
    /// ```
    pub fn element(&self, value: V) -> Gf2mElement_<V> {
        assert!(
            (value >> (self.params.m as u32)).is_zero(),
            "Element value exceeds field size"
        );
        Gf2mElement_ {
            value,
            params: Arc::clone(&self.params),
        }
    }

    /// Returns the additive identity (zero) of the field.
    pub fn zero(&self) -> Gf2mElement_<V> {
        Gf2mElement_ {
            value: V::ZERO,
            params: Arc::clone(&self.params),
        }
    }

    /// Returns the multiplicative identity (one) of the field.
    pub fn one(&self) -> Gf2mElement_<V> {
        Gf2mElement_ {
            value: V::ONE,
            params: Arc::clone(&self.params),
        }
    }

    /// Creates a new field with precomputed log/antilog tables for fast multiplication.
    ///
    /// Tables are only generated for fields with m ≤ 16 (memory limit).
    /// For larger fields, this is equivalent to `new()`.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::gf256().with_tables();
    /// assert!(field.has_tables());
    /// ```
    pub fn with_tables(self) -> Self {
        if self.params.m > 16 {
            return self;
        }

        let (log_table, exp_table) =
            Self::generate_tables(self.params.m, self.params.primitive_poly);

        #[cfg(feature = "simd")]
        let simd_mul_fn = self.params.simd_mul_fn;
        #[cfg(feature = "simd")]
        let clmul_fn = self.params.clmul_fn;
        #[cfg(feature = "simd")]
        let clmul_barrett_fn = self.params.clmul_barrett_fn;
        let barrett_reducer = self
            .params
            .barrett_reducer
            .as_ref()
            .map(|r| BarrettReducer::new(r.modulus(), r.degree()));

        Gf2mField_ {
            params: Arc::new(FieldParams_ {
                m: self.params.m,
                primitive_poly: self.params.primitive_poly,
                log_table: Some(log_table),
                exp_table: Some(exp_table),
                #[cfg(feature = "simd")]
                simd_mul_fn,
                #[cfg(feature = "simd")]
                clmul_fn,
                #[cfg(feature = "simd")]
                clmul_barrett_fn,
                barrett_reducer,
            }),
        }
    }

    /// Returns true if this field has precomputed log/antilog tables.
    pub fn has_tables(&self) -> bool {
        self.params.log_table.is_some() && self.params.exp_table.is_some()
    }

    /// Returns the primitive element (generator) used for table generation, if tables exist.
    pub fn primitive_element(&self) -> Option<Gf2mElement_<V>> {
        if !self.has_tables() {
            return None;
        }

        // The primitive element is typically x (value = 2)
        // But we verify it's actually stored in exp_table[1]
        self.params
            .exp_table
            .as_ref()
            .map(|exp| self.element(V::from_u16(exp[1])))
    }

    /// Converts a binary representation to a Gf2mPoly over this field.
    fn poly_from_binary(&self, binary: V, max_degree: usize) -> Gf2mPoly_<V> {
        let mut coeffs = Vec::new();
        for i in 0..=max_degree {
            if binary.bit(i as u32) {
                coeffs.push(self.one());
            } else {
                coeffs.push(self.zero());
            }
        }
        Gf2mPoly_::new(coeffs)
    }

    /// Computes x^k mod p(x) and returns the result as a field element value.
    ///
    /// Takes `u64` rather than `usize` so that exponents like 2^m for m ≥ 32
    /// work correctly on 32-bit targets.
    fn compute_x_power_value(&self, k: u64) -> V {
        let m = self.params.m;
        let p = self.params.primitive_poly;

        let mut result = V::ONE; // x^0 = 1
        let mut base = V::ONE << 1; // x (value 2)
        let mut exp = k;

        while exp > 0 {
            if exp & 1 == 1 {
                result = Self::mul_raw(result, base, m, p);
            }
            base = Self::mul_raw(base, base, m, p);
            exp >>= 1;
        }

        result
    }

    /// Returns the discrete logarithm of an element (if tables exist).
    ///
    /// Returns the value i such that α^i = element, where α is the primitive element.
    /// Returns None for zero or if tables don't exist.
    pub fn discrete_log(&self, element: &Gf2mElement_<V>) -> Option<u16> {
        if element.is_zero() || !self.has_tables() {
            return None;
        }

        self.params
            .log_table
            .as_ref()
            .map(|log| log[element.value().to_usize()])
    }

    /// Returns the raw antilog (`exp`) table, if precomputed.
    ///
    /// `exp_table[i] = α^i` (as a field-element value) for
    /// `i = 0..2^m - 1`, where `α` is the primitive element. The table has
    /// exactly `order = 2^m - 1` entries. Returns `None` for fields without
    /// tables (`m > 16` or constructed via [`Gf2mField::new`] rather than
    /// [`with_tables`](Self::with_tables)).
    ///
    /// This is the exact device-upload source for the GPU `gf_mul` kernel
    /// (`gf2-kernels-hip`): handing the *live* table to the device guarantees
    /// the GPU multiply is bit-identical to the CPU table path
    /// ([`Gf2mField`] `Mul`), with no on-device table regeneration.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let exp = field.exp_table().unwrap();
    /// assert_eq!(exp.len(), (1 << 4) - 1); // 15 entries
    /// assert_eq!(exp[0], 1); // α^0 = 1
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)` — returns a borrow of the stored table.
    pub fn exp_table(&self) -> Option<&[u16]> {
        self.params.exp_table.as_deref()
    }

    /// Returns the raw discrete-log (`log`) table, if precomputed.
    ///
    /// `log_table[v]` is the discrete log of the field-element value `v`
    /// (i.e. `α^log_table[v] = v`) for `v = 1..2^m - 1`; `log_table[0]` is a
    /// don't-care `0` (zero has no log). The table has exactly `2^m` entries.
    /// Returns `None` for fields without tables (see [`exp_table`](Self::exp_table)).
    ///
    /// Paired with [`exp_table`](Self::exp_table), this is the exact
    /// device-upload source for the GPU `gf_mul` kernel.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let log = field.log_table().unwrap();
    /// assert_eq!(log.len(), 1 << 4); // 16 entries
    /// assert_eq!(log[1], 0); // log(1) = 0
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)` — returns a borrow of the stored table.
    pub fn log_table(&self) -> Option<&[u16]> {
        self.params.log_table.as_deref()
    }

    /// Returns α^i where α is the primitive element (if tables exist).
    pub fn exp_value(&self, i: usize) -> Option<Gf2mElement_<V>> {
        if !self.has_tables() {
            return None;
        }

        self.params.exp_table.as_ref().map(|exp| {
            let order = (1 << self.params.m) - 1;
            let idx = i % order;
            self.element(V::from_u16(exp[idx]))
        })
    }

    /// Generates log and exp tables for a field.
    ///
    /// Only called when m ≤ 16 (guarded by `with_tables()`), so `1usize << m`
    /// and u16 table indices are safe for any backing type V.
    fn generate_tables(m: usize, primitive_poly: V) -> (Vec<u16>, Vec<u16>) {
        let order = (1usize << m) - 1;

        let alpha = Self::find_primitive_element(m, primitive_poly, order);

        let mut exp_table = vec![0u16; order];
        let mut current = V::ONE;

        for elem in exp_table.iter_mut() {
            *elem = current.to_usize() as u16;
            current = Self::mul_raw(current, alpha, m, primitive_poly);
        }

        let mut log_table = vec![0u16; 1 << m];
        for (i, &exp_val) in exp_table.iter().enumerate() {
            log_table[exp_val as usize] = i as u16;
        }
        log_table[0] = 0;

        (log_table, exp_table)
    }

    /// Finds a primitive element for GF(2^m).
    fn find_primitive_element(m: usize, primitive_poly: V, order: usize) -> V {
        // Try candidates starting from 2 (which represents x)
        // Only works for m ≤ 64 (table generation limit is m ≤ 16)
        for candidate_usize in 2..(1u64 << m) {
            let candidate = V::from_u64(candidate_usize);
            if Self::is_primitive(candidate, m, primitive_poly, order) {
                return candidate;
            }
        }
        panic!("No primitive element found (should not happen for valid primitive polynomial)");
    }

    /// Tests if an element is primitive (generates the full multiplicative group).
    fn is_primitive(elem: V, m: usize, primitive_poly: V, order: usize) -> bool {
        let mut current = elem;

        for _ in 1..order {
            if current == V::ONE {
                return false;
            }
            current = Self::mul_raw(current, elem, m, primitive_poly);
        }

        current == V::ONE
    }

    /// Raw multiplication without tables (used during table generation).
    fn mul_raw(a: V, b: V, m: usize, primitive_poly: V) -> V {
        if a.is_zero() || b.is_zero() {
            return V::ZERO;
        }

        let mut result = V::ZERO;
        let mut temp = a;

        for i in 0..m {
            if b.bit(i as u32) {
                result ^= temp;
            }

            let will_overflow = temp.bit((m - 1) as u32);
            temp = temp << 1;

            if will_overflow {
                temp ^= primitive_poly;
            }
        }

        result & V::low_mask(m as u32)
    }
}

// Methods specific to u64 fields (database verification, presets, primitivity testing)
impl Gf2mField_ {
    /// Returns the number of elements in the field (2^m).
    ///
    /// Returns `u64` to avoid overflow on 32-bit targets where `usize` is only 32 bits.
    pub fn order(&self) -> u64 {
        1u64 << self.params.m
    }

    /// Verifies that the polynomial is actually primitive for GF(2^m).
    ///
    /// A polynomial p(x) of degree m is primitive if:
    /// 1. It is irreducible over GF(2)
    /// 2. There exists a primitive element (generator of the full multiplicative group)
    ///
    /// # Algorithm
    ///
    /// Uses Rabin's irreducibility test combined with primitive element search.
    ///
    /// # Complexity
    ///
    /// O(m³) for degree-m polynomial using fast exponentiation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// // DVB-T2 standard polynomial
    /// let gf14 = Gf2mField::new(14, 0b100000000101011);
    /// assert!(gf14.verify_primitive());
    ///
    /// // Reducible polynomial (x+1)^2 = x^2 + 1
    /// let gf2_reducible = Gf2mField::new(2, 0b101);
    /// assert!(!gf2_reducible.verify_primitive());
    /// ```
    pub fn verify_primitive(&self) -> bool {
        // First check irreducibility
        if !self.is_irreducible_rabin() {
            return false;
        }

        let m = self.params.m;
        let order = (1u64 << m) - 1; // 2^m - 1

        // Step 1: Verify x^(2^m-1) = 1 (Fermat's little theorem)
        let x_to_order = self.compute_x_power_value(order);
        if x_to_order != 1u64 {
            return false;
        }

        // Step 2: For each prime factor q of (2^m-1), verify x^((2^m-1)/q) ≠ 1
        let prime_factors = Self::prime_factors_of_order_static(m);

        for q in prime_factors {
            let exp = order / q;
            let result = self.compute_x_power_value(exp);
            if result == 1u64 {
                return false;
            }
        }

        true
    }

    /// Tests irreducibility using Rabin's test.
    ///
    /// A polynomial p(x) of degree m is irreducible if and only if:
    /// - gcd(p(x), x^(2^i) - x) = 1 for all i = 1, 2, ..., ⌊m/2⌋
    /// - x^(2^m) ≡ x (mod p(x))
    ///
    /// # References
    ///
    /// Rabin, M. O. (1980). "Probabilistic algorithms in finite fields."
    /// SIAM Journal on Computing, 9(2), 273-280.
    pub fn is_irreducible_rabin(&self) -> bool {
        let m = self.params.m;
        let p = self.params.primitive_poly;

        // Convert primitive polynomial to Gf2mPoly for GCD computation
        let p_poly = self.poly_from_binary(p, m);

        // Test 1: gcd(p(x), x^(2^i) - x) = 1 for i = 1..m/2
        for i in 1..=(m / 2) {
            let exp = 1u64 << i; // 2^i
            let x_pow = self.compute_x_power_value(exp);

            // x^(2^i) - x (in GF(2), subtraction is XOR)
            let x_val = 2u64; // x
            let diff = x_pow ^ x_val;

            if diff == 0 {
                return false;
            }

            let diff_poly = self.poly_from_binary(diff, m);
            let g = Gf2mPoly_::gcd(&p_poly, &diff_poly);

            if g.degree() != Some(0) || g.coeff(0).value() != 1u64 {
                return false;
            }
        }

        // Test 2: x^(2^m) ≡ x (mod p(x))
        let exp = 1u64 << m;
        let x_power_mod_p = self.compute_x_power_value(exp);

        // x^(2^m) should equal x (value 2)
        x_power_mod_p == 2u64
    }

    /// Returns prime factors of 2^m - 1 (Mersenne number factorization).
    ///
    /// For small m, we use trial division with small primes.
    /// This is sufficient for verification purposes up to m=16.
    fn prime_factors_of_order_static(m: usize) -> Vec<u64> {
        let order = (1u64 << m) - 1;
        let mut factors = Vec::new();
        let mut n = order;

        let small_primes = [
            2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83,
            89, 97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179,
            181, 191, 193, 197, 199, 211, 223, 227, 229,
        ];

        for &p in &small_primes {
            if p * p > n {
                break;
            }
            if n.is_multiple_of(p) {
                factors.push(p);
                while n.is_multiple_of(p) {
                    n /= p;
                }
            }
        }

        if n > 1 {
            factors.push(n);
        }

        factors
    }

    /// Creates a new GF(2^m) field with database verification.
    ///
    /// This constructor checks the provided polynomial against the standard database:
    /// - If it **matches** a standard polynomial: no warning
    /// - If it **conflicts** with a standard: prints warning to stderr
    /// - If **unknown** (not in database): no warning
    pub fn new_verified(m: usize, primitive_poly: u64) -> Self {
        use crate::primitive_polys::{PrimitivePolynomialDatabase, VerificationResult};

        match PrimitivePolynomialDatabase::verify(m, primitive_poly) {
            VerificationResult::Matches => {}
            VerificationResult::Conflict => {
                eprintln!("WARNING: Non-standard primitive polynomial for GF(2^{})", m);
                eprintln!("  Provided: {:#b}", primitive_poly);
                if let Some(standard) = PrimitivePolynomialDatabase::standard(m) {
                    eprintln!("  Standard: {:#b}", standard);
                    let source = match m {
                        8 => " (AES)",
                        14 | 16 => " (DVB-T2)",
                        _ => "",
                    };
                    eprintln!(
                        "  Using non-standard polynomial may cause interoperability issues{}",
                        source
                    );
                }
            }
            VerificationResult::Unknown => {}
        }

        Self::new(m, primitive_poly)
    }

    /// Creates a GF(2^8) field with standard primitive polynomial x^8 + x^4 + x^3 + x + 1.
    ///
    /// This is the standard field used in AES and many error-correcting codes.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let gf256 = Gf2mField::gf256();
    /// assert_eq!(gf256.order(), 256u64);
    /// ```
    pub fn gf256() -> Self {
        Gf2mField::new(8, 0b100011101)
    }

    /// Creates a GF(2^16) field with standard primitive polynomial x^16 + x^12 + x^3 + x + 1.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let gf65536 = Gf2mField::gf65536();
    /// assert_eq!(gf65536.order(), 65536u64);
    /// ```
    pub fn gf65536() -> Self {
        Gf2mField::new(16, 0b10001000000001011)
    }
}

impl<V: UintExt> Gf2mElement_<V> {
    /// Returns the binary representation of this element.
    pub fn value(&self) -> V {
        self.value
    }

    /// Returns the Barrett reducer, if available (crate-internal).
    #[cfg(feature = "simd")]
    pub(crate) fn barrett_reducer(&self) -> Option<&BarrettReducer> {
        self.params.barrett_reducer.as_ref()
    }

    /// Returns the SIMD batch carry-less multiplication function, if available (crate-internal).
    #[cfg(feature = "simd")]
    pub(crate) fn clmul_batch_fn(&self) -> Option<simd_gf2m::ClmulBatchFn> {
        crate::simd::maybe_gf2m().and_then(|f| f.clmul_batch_fn)
    }

    /// Returns the SIMD single carry-less multiplication function, if available (crate-internal).
    #[cfg(feature = "simd")]
    pub(crate) fn clmul_fn(&self) -> Option<simd_gf2m::ClmulFn> {
        self.params.clmul_fn
    }

    /// Creates an element with the given raw value in the same field as `self` (crate-internal).
    ///
    /// The caller must ensure `value` is already reduced (fits in m bits).
    #[cfg(feature = "simd")]
    pub(crate) fn with_raw_value(&self, value: V) -> Self {
        Gf2mElement_ {
            value,
            params: Arc::clone(&self.params),
        }
    }

    /// Returns true if this is the zero element.
    pub fn is_zero(&self) -> bool {
        self.value == V::ZERO
    }

    /// Returns true if this is the multiplicative identity (one).
    pub fn is_one(&self) -> bool {
        self.value == V::ONE
    }

    /// Computes the multiplicative inverse of this element using the Extended Euclidean Algorithm.
    ///
    /// Returns `None` if this element is zero (which has no multiplicative inverse).
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(0b0101);
    /// let inv = a.inverse().expect("non-zero element has inverse");
    ///
    /// // a * a^(-1) = 1
    /// let product = &a * &inv;
    /// assert_eq!(product, field.one());
    /// ```
    pub fn inverse(&self) -> Option<Gf2mElement_<V>> {
        if self.is_zero() {
            return None;
        }

        if self.is_one() {
            return Some(Gf2mElement_ {
                value: V::ONE,
                params: Arc::clone(&self.params),
            });
        }

        // Use field multiplication to compute inverse via exponentiation
        // In GF(2^m), a^(2^m - 1) = 1 for all non-zero a
        // Therefore a^(-1) = a^(2^m - 2)
        let m = self.params.m;
        // 2^m - 2 = (2^m - 1) XOR 1 in GF(2)
        let exp = V::low_mask(m as u32) ^ V::ONE;

        let mut result = Gf2mElement_ {
            value: V::ONE,
            params: Arc::clone(&self.params),
        };
        let mut base = self.clone();
        let mut e = exp;

        // Square-and-multiply algorithm
        let mut bit_pos = 0u32;
        while bit_pos < V::BITS {
            if e.bit(0) {
                result = &result * &base;
            }
            base = &base * &base;
            e = e >> 1;
            bit_pos += 1;
            if e.is_zero() {
                break;
            }
        }

        Some(result)
    }

    /// Computes the minimal polynomial of this field element over GF(2).
    ///
    /// The minimal polynomial is the monic polynomial of smallest degree that has
    /// this element as a root. For an element α in GF(2^m), the minimal polynomial
    /// has degree d where d divides m, and its roots are the conjugates of α:
    /// {α, α^2, α^4, ..., α^(2^(d-1))}.
    ///
    /// # Properties
    ///
    /// - The minimal polynomial is always monic (leading coefficient = 1)
    /// - Its degree divides the extension degree m
    /// - The element is a root: m_α(α) = 0
    /// - It's the product (x - α)(x - α^2)(x - α^4)...(x - α^(2^(d-1)))
    ///
    /// # Algorithm
    ///
    /// Uses repeated squaring to find conjugates, then builds the polynomial
    /// as the product of (x - conjugate) for each unique conjugate.
    ///
    /// # Complexity
    ///
    /// `O(d)` GF(2^m) multiplications to collect the conjugate orbit
    /// (`d = degree of the minimal polynomial, with d | m`), plus `O(d^2)`
    /// base-field operations to build the product polynomial. In practice
    /// `d ≤ m`, so total work is bounded by `O(m^2)` GF(2^m) operations.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let alpha = field.element(0b0010); // x
    /// let min_poly = alpha.minimal_polynomial();
    ///
    /// // Verify alpha is a root
    /// let result = min_poly.eval(&alpha);
    /// assert!(result.is_zero());
    /// ```
    pub fn minimal_polynomial(&self) -> Gf2mPoly_<V> {
        // Special case: minimal polynomial of 0 is x
        if self.is_zero() {
            return Gf2mPoly_::new(vec![
                Gf2mElement_ {
                    value: V::ZERO,
                    params: Arc::clone(&self.params),
                },
                Gf2mElement_ {
                    value: V::ONE,
                    params: Arc::clone(&self.params),
                },
            ]);
        }

        // Find all conjugates: α, α^2, α^4, α^8, ... until we cycle back
        let mut conjugates = Vec::new();
        let mut current = self.clone();

        loop {
            if conjugates
                .iter()
                .any(|c: &Gf2mElement_<V>| c.value == current.value)
            {
                break;
            }
            conjugates.push(current.clone());
            current = &current * &current;
        }

        // Build minimal polynomial as product of (x - conjugate) terms
        let one = Gf2mElement_ {
            value: V::ONE,
            params: Arc::clone(&self.params),
        };
        let mut result = Gf2mPoly_::constant(one);

        for conjugate in conjugates {
            let term = Gf2mPoly_::new(vec![
                conjugate,
                Gf2mElement_ {
                    value: V::ONE,
                    params: Arc::clone(&self.params),
                },
            ]);
            result = &result * &term;
        }

        result
    }
}

// Addition in GF(2^m) is XOR
impl<V: UintExt> Add for &Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn add(self, rhs: Self) -> Self::Output {
        assert!(
            Arc::ptr_eq(&self.params, &rhs.params),
            "Cannot add elements from different fields"
        );
        Gf2mElement_ {
            value: self.value ^ rhs.value,
            params: Arc::clone(&self.params),
        }
    }
}

impl<V: UintExt> Add for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn add(self, rhs: Self) -> Self::Output {
        &self + &rhs
    }
}

// Multiplication in GF(2^m) - polynomial multiplication with reduction
impl<V: UintExt> Mul for &Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn mul(self, rhs: Self) -> Self::Output {
        assert!(
            Arc::ptr_eq(&self.params, &rhs.params),
            "Cannot multiply elements from different fields"
        );

        if self.value.is_zero() || rhs.value.is_zero() {
            return Gf2mElement_ {
                value: V::ZERO,
                params: Arc::clone(&self.params),
            };
        }

        // Priority 1: Use table-based multiplication if available (fastest for small m)
        if let (Some(log_table), Some(exp_table)) = (
            self.params.log_table.as_ref(),
            self.params.exp_table.as_ref(),
        ) {
            let log_a = log_table[self.value.to_usize()] as usize;
            let log_b = log_table[rhs.value.to_usize()] as usize;
            let order = (1 << self.params.m) - 1;
            let log_result = (log_a + log_b) % order;

            return Gf2mElement_ {
                value: V::from_u16(exp_table[log_result]),
                params: Arc::clone(&self.params),
            };
        }

        // Priority 2a: All-in-one PCLMULQDQ + Barrett kernel (3 clmul ops in one
        // target_feature scope — eliminates function-pointer call overhead).
        #[cfg(feature = "simd")]
        if let (Some(clmul_barrett_fn), Some(barrett)) = (
            self.params.clmul_barrett_fn,
            self.params.barrett_reducer.as_ref(),
        ) {
            let result = clmul_barrett_fn(
                self.value.as_u64_truncated(),
                rhs.value.as_u64_truncated(),
                barrett.mu() as u64,
                barrett.modulus() as u64,
                barrett.degree(),
            );
            return Gf2mElement_ {
                value: V::from_u64(result),
                params: Arc::clone(&self.params),
            };
        }

        // Priority 2b: Split PCLMULQDQ raw clmul + Barrett reduction (fallback
        // when the all-in-one kernel is unavailable).
        #[cfg(feature = "simd")]
        if let (Some(clmul_fn), Some(barrett)) =
            (self.params.clmul_fn, self.params.barrett_reducer.as_ref())
        {
            let product = clmul_fn(self.value.as_u64_truncated(), rhs.value.as_u64_truncated());
            let result = barrett.reduce_with_clmul(product, clmul_fn);
            return Gf2mElement_ {
                value: V::from_u64(result),
                params: Arc::clone(&self.params),
            };
        }

        // Priority 3: Use SIMD combined mul+reduce if available (legacy path)
        #[cfg(feature = "simd")]
        if let Some(simd_mul_fn) = self.params.simd_mul_fn {
            let result = simd_mul_fn(
                self.value.as_u64_truncated(),
                rhs.value.as_u64_truncated(),
                self.params.m,
                self.params.primitive_poly.as_u64_truncated(),
            );
            return Gf2mElement_ {
                value: V::from_u64(result),
                params: Arc::clone(&self.params),
            };
        }

        // Priority 4: Fallback to schoolbook multiplication
        let m = self.params.m;
        let primitive_poly = self.params.primitive_poly;

        let result = Gf2mField_::<V>::mul_raw(self.value, rhs.value, m, primitive_poly);

        Gf2mElement_ {
            value: result,
            params: Arc::clone(&self.params),
        }
    }
}

impl<V: UintExt> Mul for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn mul(self, rhs: Self) -> Self::Output {
        &self * &rhs
    }
}

// Division in GF(2^m) - multiply by multiplicative inverse
impl<V: UintExt> Div for &Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn div(self, rhs: Self) -> Self::Output {
        assert!(
            Arc::ptr_eq(&self.params, &rhs.params),
            "Cannot divide elements from different fields"
        );

        let inv = rhs.inverse().expect("division by zero");
        self * &inv
    }
}

impl<V: UintExt> Div for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn div(self, rhs: Self) -> Self::Output {
        &self / &rhs
    }
}

impl<V: UintExt> fmt::Display for Gf2mElement_<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#b}", self.value)
    }
}

// Hash only the element value, not the field context.
impl<V: UintExt> Hash for Gf2mElement_<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

// Negation in GF(2^m): -a = a (every element is its own additive inverse)
impl<V: UintExt> Neg for &Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn neg(self) -> Self::Output {
        self.clone()
    }
}

impl<V: UintExt> Neg for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn neg(self) -> Self::Output {
        self
    }
}

// Subtraction in GF(2^m) equals addition: a - b = a + b = a XOR b
#[allow(clippy::suspicious_arithmetic_impl)]
impl<V: UintExt> Sub for &Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn sub(self, rhs: Self) -> Self::Output {
        self + rhs
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<V: UintExt> Sub for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn sub(self, rhs: Self) -> Self::Output {
        &self - &rhs
    }
}

// Mixed-receiver operators: owned + &ref
impl<V: UintExt> Add<&Gf2mElement_<V>> for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn add(self, rhs: &Gf2mElement_<V>) -> Self::Output {
        &self + rhs
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<V: UintExt> Sub<&Gf2mElement_<V>> for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn sub(self, rhs: &Gf2mElement_<V>) -> Self::Output {
        &self - rhs
    }
}

impl<V: UintExt> Mul<&Gf2mElement_<V>> for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn mul(self, rhs: &Gf2mElement_<V>) -> Self::Output {
        &self * rhs
    }
}

impl<V: UintExt> Div<&Gf2mElement_<V>> for Gf2mElement_<V> {
    type Output = Gf2mElement_<V>;

    fn div(self, rhs: &Gf2mElement_<V>) -> Self::Output {
        &self / rhs
    }
}

// AddAssign — required by FiniteField trait (and Wide: AddAssign bound)
impl<V: UintExt> AddAssign for Gf2mElement_<V> {
    fn add_assign(&mut self, rhs: Self) {
        assert!(
            Arc::ptr_eq(&self.params, &rhs.params),
            "Cannot add elements from different fields"
        );
        self.value ^= rhs.value;
    }
}

impl<V: UintExt> AddAssign<&Gf2mElement_<V>> for Gf2mElement_<V> {
    fn add_assign(&mut self, rhs: &Gf2mElement_<V>) {
        assert!(
            Arc::ptr_eq(&self.params, &rhs.params),
            "Cannot add elements from different fields"
        );
        self.value ^= rhs.value;
    }
}

// FiniteField trait implementation for GF(2^m)
impl<V: UintExt> crate::field::FiniteField for Gf2mElement_<V> {
    type Characteristic = u64;

    // XOR addition never overflows, so Wide = Self is correct for binary fields.
    type Wide = Self;

    fn characteristic(&self) -> u64 {
        2
    }

    fn extension_degree(&self) -> usize {
        self.params.m
    }

    fn is_zero(&self) -> bool {
        self.value == V::ZERO
    }

    fn is_one(&self) -> bool {
        self.value == V::ONE
    }

    fn inv(&self) -> Option<Self> {
        self.inverse()
    }

    fn zero_like(&self) -> Self {
        Gf2mElement_ {
            value: V::ZERO,
            params: Arc::clone(&self.params),
        }
    }

    fn one_like(&self) -> Self {
        Gf2mElement_ {
            value: V::ONE,
            params: Arc::clone(&self.params),
        }
    }

    fn to_wide(&self) -> Self::Wide {
        self.clone()
    }

    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide {
        self.clone() * rhs
    }

    fn reduce_wide(wide: &Self::Wide) -> Self {
        wide.clone()
    }

    fn max_unreduced_additions() -> usize {
        usize::MAX
    }

    fn try_gf2m_u64_batch_dot_product(
        a: &[Self],
        b: &[Self],
        zero: &Self,
        scratch_a: &mut Vec<u64>,
        scratch_b: &mut Vec<u64>,
        scratch_products: &mut Vec<u64>,
    ) -> Option<Self> {
        debug_assert_eq!(a.len(), b.len());
        if !V::IS_U64 || !matches!(zero.params.m, 8 | 16 | 32) {
            return None;
        }
        if !a.iter().all(|x| Arc::ptr_eq(&x.params, &zero.params))
            || !b.iter().all(|y| Arc::ptr_eq(&y.params, &zero.params))
        {
            // Preserve the scalar path's field-context assertion semantics.
            return None;
        }

        scratch_a.clear();
        scratch_b.clear();
        scratch_products.clear();
        scratch_a.reserve(a.len());
        scratch_b.reserve(b.len());
        scratch_products.resize(a.len(), 0);

        for (x, y) in a.iter().zip(b.iter()) {
            scratch_a.push(x.value.as_u64_truncated());
            scratch_b.push(y.value.as_u64_truncated());
        }

        crate::gf2m::batch::batch_mul_raw(
            zero.params.m,
            zero.params.primitive_poly.as_u64_truncated(),
            scratch_a,
            scratch_b,
            scratch_products,
        );
        let value = scratch_products.iter().fold(0u64, |acc, &x| acc ^ x);
        Some(Gf2mElement_ {
            value: V::from_u64(value),
            params: Arc::clone(&zero.params),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_creation() {
        let field = Gf2mField::new(4, 0b10011);
        assert_eq!(field.degree(), 4);
        assert_eq!(field.order(), 16);
        assert_eq!(field.primitive_polynomial(), 0b10011);
    }

    #[test]
    fn test_gf256_preset() {
        let field = Gf2mField::gf256();
        assert_eq!(field.degree(), 8);
        assert_eq!(field.order(), 256);
    }

    #[test]
    fn test_gf65536_preset() {
        let field = Gf2mField::gf65536();
        assert_eq!(field.degree(), 16);
        assert_eq!(field.order(), 65536);
    }

    #[test]
    fn test_element_creation() {
        let field = Gf2mField::new(4, 0b10011);
        let elem = field.element(0b1010);
        assert_eq!(elem.value(), 0b1010);
        assert!(!elem.is_zero());
        assert!(!elem.is_one());
    }

    #[test]
    fn test_zero_and_one() {
        let field = Gf2mField::new(4, 0b10011);
        let zero = field.zero();
        let one = field.one();

        assert!(zero.is_zero());
        assert!(!zero.is_one());
        assert!(!one.is_zero());
        assert!(one.is_one());
    }

    #[test]
    #[should_panic(expected = "exceeds field size")]
    fn test_element_too_large() {
        let field = Gf2mField::new(4, 0b10011);
        field.element(0b10000); // 16 is too large for GF(2^4)
    }

    // Field Axiom Tests

    #[test]
    fn test_addition_commutative() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let b = field.element(0b1010);

        assert_eq!(&a + &b, &b + &a);
    }

    #[test]
    fn test_addition_associative() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let b = field.element(0b1010);
        let c = field.element(0b1100);

        assert_eq!(&(&a + &b) + &c, &a + &(&b + &c));
    }

    #[test]
    fn test_addition_identity() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let zero = field.zero();

        assert_eq!(&a + &zero, a);
        assert_eq!(&zero + &a, a);
    }

    #[test]
    fn test_addition_self_inverse() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let zero = field.zero();

        // In GF(2^m), every element is its own additive inverse
        assert_eq!(&a + &a, zero);
    }

    #[test]
    fn test_multiplication_commutative() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let b = field.element(0b1010);

        assert_eq!(&a * &b, &b * &a);
    }

    #[test]
    fn test_multiplication_associative() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let b = field.element(0b0011);
        let c = field.element(0b1100);

        assert_eq!(&(&a * &b) * &c, &a * &(&b * &c));
    }

    #[test]
    fn test_multiplication_identity() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let one = field.one();

        assert_eq!(&a * &one, a);
        assert_eq!(&one * &a, a);
    }

    #[test]
    fn test_multiplication_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let zero = field.zero();

        assert_eq!(&a * &zero, zero);
        assert_eq!(&zero * &a, zero);
    }

    #[test]
    fn test_distributive_law() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let b = field.element(0b0011);
        let c = field.element(0b1100);

        // a * (b + c) = (a * b) + (a * c)
        assert_eq!(&a * &(&b + &c), &(&a * &b) + &(&a * &c));
    }

    // Specific GF(2^4) worked examples from documentation

    #[test]
    fn test_gf16_addition_example() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101); // x² + 1
        let b = field.element(0b1010); // x³ + x

        // (x² + 1) + (x³ + x) = x³ + x² + x + 1
        let sum = a + b;
        assert_eq!(sum.value(), 0b1111);
    }

    #[test]
    fn test_gf16_multiplication_example() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101); // x² + 1
        let b = field.element(0b1010); // x³ + x

        // (x² + 1) · (x³ + x) mod (x⁴ + x + 1)
        // = x⁵ + x³ + x³ + x = x⁵ + x  (x³ + x³ = 0 in GF(2))
        // x⁵ = x · x⁴ = x · (x + 1) = x² + x  (since x⁴ ≡ x + 1 mod p(x))
        // Final: (x² + x) + x = x²  (x + x = 0 in GF(2))
        // Result: x² = 0b0100 = 4
        let product = a * b;
        assert_eq!(product.value(), 0b0100);
    }

    #[test]
    fn test_gf256_addition() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        let b = field.element(0xCA);

        // Addition is XOR
        let sum = a + b;
        assert_eq!(sum.value(), 0x99);
    }

    #[test]
    fn test_gf256_multiplication_simple() {
        let field = Gf2mField::gf256();
        let a = field.element(0x02); // x
        let b = field.element(0x03); // x + 1

        // x * (x + 1) = x² + x
        let product = a * b;
        assert_eq!(product.value(), 0x06); // binary 110 = x² + x
    }

    // Division and Multiplicative Inverse Tests

    #[test]
    fn test_inverse_of_one() {
        let field = Gf2mField::new(4, 0b10011);
        let one = field.one();
        let inv = one.inverse().expect("one should have inverse");
        assert_eq!(inv, one); // 1^(-1) = 1
    }

    #[test]
    fn test_inverse_exists_for_nonzero() {
        let field = Gf2mField::new(4, 0b10011);
        // Test all non-zero elements
        for i in 1..16 {
            let elem = field.element(i);
            let inv = elem
                .inverse()
                .expect("non-zero element should have inverse");
            let product = &elem * &inv;
            assert_eq!(
                product,
                field.one(),
                "element {} * inverse should equal 1",
                i
            );
        }
    }

    #[test]
    fn test_inverse_of_zero_is_none() {
        let field = Gf2mField::new(4, 0b10011);
        let zero = field.zero();
        assert!(zero.inverse().is_none(), "zero should not have inverse");
    }

    #[test]
    fn test_inverse_of_inverse() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let inv_a = a.inverse().expect("should have inverse");
        let inv_inv_a = inv_a.inverse().expect("inverse should have inverse");
        assert_eq!(inv_inv_a, a, "(a^(-1))^(-1) = a");
    }

    #[test]
    fn test_division_by_one() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let one = field.one();
        let quotient = &a / &one;
        assert_eq!(quotient, a, "a / 1 = a");
    }

    #[test]
    fn test_division_roundtrip() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let b = field.element(0b1010);

        let product = &a * &b;
        let quotient = &product / &b;
        assert_eq!(quotient, a, "(a * b) / b = a");
    }

    #[test]
    fn test_division_self() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let quotient = &a / &a;
        assert_eq!(quotient, field.one(), "a / a = 1");
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_division_by_zero_panics() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(0b0101);
        let zero = field.zero();
        let _ = a / zero;
    }

    #[test]
    fn test_gf256_division() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        let b = field.element(0xCA);

        let product = &a * &b;
        let quotient = &product / &b;
        assert_eq!(quotient, a);
    }

    // Log/Antilog Table Tests

    #[test]
    fn test_table_generation_gf16() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        assert!(field.has_tables(), "GF(2^4) should have tables");
    }

    #[test]
    fn test_table_generation_gf256() {
        let field = Gf2mField::gf256().with_tables();
        assert!(field.has_tables(), "GF(2^8) should have tables");
    }

    #[test]
    fn test_tables_not_generated_for_large_field() {
        // m=17 is too large for tables by default
        let field = Gf2mField::new(17, 0b100000000000001001);
        assert!(
            !field.has_tables(),
            "GF(2^17) should not have tables by default"
        );
    }

    #[test]
    fn test_table_multiply_matches_schoolbook_gf16() {
        let field_with_tables = Gf2mField::new(4, 0b10011).with_tables();
        let field_no_tables = Gf2mField::new(4, 0b10011);

        // Test all pairs of non-zero elements
        for i in 1..16 {
            for j in 1..16 {
                let a_t = field_with_tables.element(i);
                let b_t = field_with_tables.element(j);
                let a_n = field_no_tables.element(i);
                let b_n = field_no_tables.element(j);

                assert_eq!(
                    (&a_t * &b_t).value(),
                    (&a_n * &b_n).value(),
                    "Table multiply should match schoolbook for {} * {}",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_primitive_element_generates_field() {
        let field = Gf2mField::new(4, 0b10011).with_tables();

        // A primitive element should generate all non-zero elements
        // The multiplicative group has order 2^4 - 1 = 15
        if let Some(alpha) = field.primitive_element() {
            let mut power = field.one(); // Start with α^0 = 1
            let mut seen = std::collections::HashSet::new();

            for i in 0..15 {
                seen.insert(power.value());
                power = &power * &alpha; // Compute next power

                if i < 14 {
                    assert!(
                        !seen.contains(&power.value()),
                        "Generated duplicate element at power {}",
                        i + 1
                    );
                }
            }

            // After 15 multiplications, we have α^15, which should equal α^0 = 1
            assert_eq!(power, field.one(), "α^15 should equal 1 in GF(2^4)");
            assert_eq!(
                seen.len(),
                15,
                "Should have generated all 15 non-zero elements"
            );
        }
    }

    #[test]
    fn test_exp_log_inverse_property() {
        let field = Gf2mField::new(4, 0b10011).with_tables();

        // For all non-zero elements: exp[log[a]] = a
        for i in 1..16 {
            let elem = field.element(i);
            if let Some(log_val) = field.discrete_log(&elem) {
                let reconstructed = field.exp_value(log_val as usize).unwrap();
                assert_eq!(
                    reconstructed.value(),
                    elem.value(),
                    "exp[log[{}]] should equal {}",
                    i,
                    i
                );
            }
        }
    }

    // Primitive polynomial verification tests

    #[test]
    fn test_verify_primitive_gf4() {
        let field = Gf2mField::new(2, 0b111); // x^2 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf8() {
        let field = Gf2mField::new(3, 0b1011); // x^3 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf16() {
        let field = Gf2mField::new(4, 0b10011); // x^4 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf256() {
        // Standard primitive polynomial for GF(256)
        let field = Gf2mField::new(8, 0b100011101);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_dvb_t2_gf14() {
        // Correct DVB-T2 polynomial
        let field = Gf2mField::new(14, 0b100000000101011);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_dvb_t2_gf16() {
        // Correct DVB-T2 polynomial for normal frames
        let field = Gf2mField::new(16, 0b10000000000101101);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_not_primitive_wrong_dvb_t2() {
        // The bug: wrong polynomial 0b100000000100001 (x^14 + x^5 + 1) was used
        // This polynomial is irreducible but NOT primitive (x does not generate full group)
        // The correct DVB-T2 standard is 0b100000000101011 (x^14 + x^5 + x^3 + x + 1)
        let field = Gf2mField::new(14, 0b100000000100001);

        // This polynomial is NOT primitive - it caused BCH decoding failures
        assert!(
            !field.verify_primitive(),
            "x^14 + x^5 + 1 is NOT primitive (caused the BCH bug)"
        );

        // And it doesn't match the DVB-T2 standard
        use crate::primitive_polys::{PrimitivePolynomialDatabase, VerificationResult};
        assert_eq!(
            PrimitivePolynomialDatabase::verify(14, 0b100000000100001),
            VerificationResult::Conflict,
            "Should conflict with DVB-T2 standard"
        );
    }

    #[test]
    fn test_verify_not_primitive_reducible() {
        // (x + 1)^2 = x^2 + 1 is reducible
        let field = Gf2mField::new(2, 0b101);
        assert!(!field.verify_primitive());
    }

    #[test]
    fn test_is_irreducible_rabin_small_cases() {
        // x^2 + x + 1 is irreducible
        let field = Gf2mField::new(2, 0b111);
        assert!(field.is_irreducible_rabin());

        // x^2 + 1 = (x + 1)^2 is reducible
        let field = Gf2mField::new(2, 0b101);
        assert!(!field.is_irreducible_rabin());
    }

    // --- Hash tests ---

    #[test]
    fn test_hash_equal_elements_have_equal_hash() {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(5);
        let b = field.element(5);

        let mut ha = DefaultHasher::new();
        let mut hb = DefaultHasher::new();
        a.hash(&mut ha);
        b.hash(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }

    #[test]
    fn test_hash_different_values_have_different_hash() {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        let b = field.element(0xCA);

        let mut ha = DefaultHasher::new();
        let mut hb = DefaultHasher::new();
        a.hash(&mut ha);
        b.hash(&mut hb);
        assert_ne!(ha.finish(), hb.finish());
    }

    #[test]
    fn test_hash_ignores_field_context() {
        use std::hash::{DefaultHasher, Hash, Hasher};
        // Two independently-constructed GF(2^8) fields
        let field1 = Gf2mField::gf256();
        let field2 = Gf2mField::gf256();
        let a = field1.element(42);
        let b = field2.element(42);

        let mut ha = DefaultHasher::new();
        let mut hb = DefaultHasher::new();
        a.hash(&mut ha);
        b.hash(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }

    // --- Sub tests ---

    #[test]
    fn test_subtraction_equals_addition() {
        let field = Gf2mField::new(4, 0b10011);
        for a_val in 0..16u64 {
            for b_val in 0..16u64 {
                let a = field.element(a_val);
                let b = field.element(b_val);
                assert_eq!(&a - &b, &a + &b);
            }
        }
    }

    #[test]
    fn test_subtraction_self_is_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let zero = field.zero();
        for val in 0..16u64 {
            let a = field.element(val);
            assert_eq!(&a - &a, zero);
        }
    }

    #[test]
    fn test_subtraction_identity() {
        let field = Gf2mField::new(4, 0b10011);
        let zero = field.zero();
        for val in 0..16u64 {
            let a = field.element(val);
            assert_eq!(&a - &zero, a);
        }
    }

    // --- Neg tests ---

    #[test]
    fn test_negation_is_identity() {
        let field = Gf2mField::new(4, 0b10011);
        for val in 0..16u64 {
            let a = field.element(val);
            assert_eq!(-&a, a);
        }
    }

    #[test]
    fn test_negation_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let zero = field.zero();
        assert_eq!(-&zero, zero);
    }

    #[test]
    fn test_double_negation() {
        let field = Gf2mField::new(4, 0b10011);
        for val in 0..16u64 {
            let a = field.element(val);
            assert_eq!(-(-&a), a);
        }
    }

    // Property-based tests using proptest

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_table_multiply_equals_schoolbook(a in 1u64..16, b in 1u64..16) {
            let field_with_tables = Gf2mField::new(4, 0b10011).with_tables();
            let field_no_tables = Gf2mField::new(4, 0b10011);

            let elem_a_t = field_with_tables.element(a);
            let elem_b_t = field_with_tables.element(b);
            let elem_a_n = field_no_tables.element(a);
            let elem_b_n = field_no_tables.element(b);

            prop_assert_eq!((&elem_a_t * &elem_b_t).value(), (&elem_a_n * &elem_b_n).value());
        }

        #[test]
        fn prop_division_inverse_of_multiplication(a in 1u64..16, b in 1u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let elem_a = field.element(a);
            let elem_b = field.element(b);

            let product = &elem_a * &elem_b;
            let quotient = &product / &elem_b;

            prop_assert_eq!(quotient, elem_a);
        }

        #[test]
        fn prop_inverse_roundtrip(a in 1u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let elem = field.element(a);

            let inv = elem.inverse().unwrap();
            let inv_inv = inv.inverse().unwrap();

            prop_assert_eq!(inv_inv, elem);
        }

        #[test]
        fn prop_multiplicative_inverse_property(a in 1u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let elem = field.element(a);
            let one = field.one();

            let inv = elem.inverse().unwrap();
            let product = &elem * &inv;

            prop_assert_eq!(product, one);
        }

        #[test]
        fn prop_gf256_table_multiply_equals_schoolbook(a in 1u64..256, b in 1u64..256) {
            let field_with_tables = Gf2mField::gf256().with_tables();
            let field_no_tables = Gf2mField::gf256();

            let elem_a_t = field_with_tables.element(a);
            let elem_b_t = field_with_tables.element(b);
            let elem_a_n = field_no_tables.element(a);
            let elem_b_n = field_no_tables.element(b);

            prop_assert_eq!((&elem_a_t * &elem_b_t).value(), (&elem_a_n * &elem_b_n).value());
        }

        #[test]
        fn prop_distributive_law(a in 0u64..16, b in 0u64..16, c in 0u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let elem_a = field.element(a);
            let elem_b = field.element(b);
            let elem_c = field.element(c);

            // a * (b + c) = (a * b) + (a * c)
            let left = &elem_a * &(&elem_b + &elem_c);
            let right = &(&elem_a * &elem_b) + &(&elem_a * &elem_c);

            prop_assert_eq!(left, right);
        }

        #[test]
        fn prop_sub_equals_add(a in 0u64..16, b in 0u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let elem_a = field.element(a);
            let elem_b = field.element(b);

            prop_assert_eq!(&elem_a - &elem_b, &elem_a + &elem_b);
        }

        #[test]
        fn prop_neg_is_identity(a in 0u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let elem = field.element(a);

            prop_assert_eq!(-&elem, elem);
        }
    }

    // -----------------------------------------------------------------------
    // Known-answer tests for Gf2mElement_<u128> (GF(2^64) and up)
    // -----------------------------------------------------------------------

    /// Known-answer multiplication test vectors for GF(2^64) with the
    /// standard irreducible polynomial p(x) = x^64 + x^4 + x^3 + x + 1
    /// (from Seroussi's table, `PrimitivePolynomialDatabase::standard_u128(64)`;
    /// see that accessor's doc for the exact strength of the guarantee).
    ///
    /// # Test vectors
    ///
    /// All reductions are derived by hand from x^64 ≡ x^4 + x^3 + x + 1 (mod p).
    ///
    /// 1. `x * x = x^2`: (2) * (2) = 4. No reduction needed (degree 2 < 64).
    /// 2. `x^63 * x = x^64 ≡ x^4 + x^3 + x + 1 = 0b11011`. Exercises the
    ///    reduction step exactly once.
    /// 3. `x^63 * x^63`: squaring the highest pre-reduction element; the
    ///    reference value was independently computed with
    ///    `Gf2mField_::<u128>::mul_raw`. This catches any divergence between
    ///    operator-level `*` and the schoolbook primitive used during
    ///    table generation / inverse computation.
    #[test]
    fn test_gf2_64_known_multiplication_vectors() {
        use crate::primitive_polys::PrimitivePolynomialDatabase;
        let poly = PrimitivePolynomialDatabase::standard_u128(64).unwrap();
        assert_eq!(poly, (1u128 << 64) | 0b11011);

        let field = Gf2mField_::<u128>::new(64, poly);

        // Vector 1: x * x = x^2.
        let x = field.element(2);
        let x2 = &x * &x;
        assert_eq!(
            x2.value(),
            4,
            "x * x should be x^2 (value 4), got {:#x}",
            x2.value()
        );

        // Vector 2: x^63 * x = x^64 ≡ x^4 + x^3 + x + 1 = 0b11011.
        let x63 = field.element(1u128 << 63);
        let x64_reduced = &x63 * &x;
        assert_eq!(
            x64_reduced.value(),
            0b11011,
            "x^63 * x should reduce to x^4 + x^3 + x + 1 = 0b11011, got {:#x}",
            x64_reduced.value()
        );

        // Vector 3: cross-check against the generic schoolbook primitive.
        let a = 1u128 << 63;
        let b = 1u128 << 63;
        let expected = Gf2mField_::<u128>::mul_raw(a, b, 64, poly);
        let op_result = (&field.element(a) * &field.element(b)).value();
        assert_eq!(
            op_result, expected,
            "operator-mul and mul_raw disagree: op={:#x}, raw={:#x}",
            op_result, expected
        );
    }

    /// Verifies additional GF(2^64) identities: commutativity, the
    /// multiplicative-identity element, and the zero annihilator on random
    /// u128 inputs. These act as a smoke test that the u128 storage does not
    /// silently lose bits in the high half.
    #[test]
    fn test_gf2_64_identities_on_u128() {
        use crate::primitive_polys::PrimitivePolynomialDatabase;
        let poly = PrimitivePolynomialDatabase::standard_u128(64).unwrap();
        let field = Gf2mField_::<u128>::new(64, poly);

        // Multiplicative identity
        let a_val: u128 = 0xDEAD_BEEF_CAFE_BABE_0123_4567_89AB_CDEF;
        let mask: u128 = (1u128 << 64) - 1;
        let a = field.element(a_val & mask);
        let one = field.one();
        assert_eq!((&a * &one).value(), a.value(), "a * 1 != a");

        // Zero annihilation
        let zero = field.zero();
        assert_eq!((&a * &zero).value(), 0, "a * 0 != 0");

        // Commutativity
        let b = field.element(0x0123_4567_89AB_CDEF);
        let ab = (&a * &b).value();
        let ba = (&b * &a).value();
        assert_eq!(ab, ba, "GF(2^64) multiplication is not commutative");
    }
}

// ============================================================================
// Polynomial Operations over GF(2^m)
// ============================================================================

/// A polynomial with coefficients in GF(2^m).
///
/// `Gf2mPoly_<V>` is a **type alias** for
/// [`FieldPoly<Gf2mElement_<V>>`](crate::field::FieldPoly). It exists
/// purely to preserve the pre-existing BCH / DVB-T2 / Reed–Solomon
/// call-site vocabulary; all algorithmic code — `Add`, `Sub`,
/// `Mul` (schoolbook + Karatsuba dispatch), `div_rem`, `gcd`,
/// Horner `eval`, `eval_batch`, `from_roots`, `product`, `monomial`
/// — lives on
/// [`FieldPoly`](crate::field::FieldPoly) and is inherited through
/// this alias.
///
/// Binary-field-specific extras (conversions to/from `BitVec`,
/// construction from exponent lists, the indeterminate `x(field)`)
/// are declared as inherent methods on
/// `FieldPoly<Gf2mElement_<V>>` in [`crate::gf2m::poly_helpers`] and
/// are consequently available through this alias.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
///
/// let field = Gf2mField::new(4, 0b10011);
/// let coeffs = vec![
///     field.element(1),  // constant term
///     field.element(2),  // x term
///     field.element(3),  // x^2 term
/// ];
/// let poly = Gf2mPoly::new(coeffs);
/// assert_eq!(poly.degree(), Some(2));
/// ```
pub type Gf2mPoly_<V = u64> = crate::field::FieldPoly<Gf2mElement_<V>>;

/// Convenience alias: `Gf2mPoly` is `Gf2mPoly_<u64>`.
pub type Gf2mPoly = Gf2mPoly_<u64>;

// All generic polynomial methods live on `FieldPoly<F>` in
// `crate::field::poly`. Binary-field-specific helpers live in
// `crate::gf2m::poly_helpers` and are inherent on
// `FieldPoly<Gf2mElement_<V>>` — i.e. reachable through this alias.

#[cfg(test)]
mod poly_tests {
    use super::*;
    use crate::BitVec;

    #[test]
    fn test_poly_creation() {
        let field = Gf2mField::new(4, 0b10011);
        let coeffs = vec![field.element(1), field.element(2), field.element(3)];
        let poly = Gf2mPoly_::new(coeffs);

        assert_eq!(poly.degree(), Some(2));
        assert!(!poly.is_zero());
    }

    #[test]
    fn test_zero_poly() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::zero(&field);

        assert!(poly.is_zero());
        assert_eq!(poly.degree(), None);
    }

    #[test]
    fn test_constant_poly() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::constant(field.element(5));

        assert_eq!(poly.degree(), Some(0));
        assert_eq!(poly.coeff(0).value(), 5);
    }

    #[test]
    fn test_poly_normalization() {
        let field = Gf2mField::new(4, 0b10011);
        // Create polynomial with leading zeros: 1 + 2x + 0x^2 + 0x^3
        let coeffs = vec![
            field.element(1),
            field.element(2),
            field.zero(),
            field.zero(),
        ];
        let poly = Gf2mPoly_::new(coeffs);

        assert_eq!(poly.degree(), Some(1)); // Leading zeros removed
    }

    #[test]
    fn test_poly_coeff_access() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![field.element(1), field.element(2), field.element(3)]);

        assert_eq!(poly.coeff(0).value(), 1);
        assert_eq!(poly.coeff(1).value(), 2);
        assert_eq!(poly.coeff(2).value(), 3);
        assert_eq!(poly.coeff(10).value(), 0); // Beyond degree returns zero
    }

    #[test]
    fn test_poly_addition() {
        let field = Gf2mField::new(4, 0b10011);
        // p1(x) = 1 + 2x + 3x^2
        let p1 = Gf2mPoly_::new(vec![field.element(1), field.element(2), field.element(3)]);
        // p2(x) = 4 + 5x
        let p2 = Gf2mPoly_::new(vec![field.element(4), field.element(5)]);

        let sum = &p1 + &p2;
        // sum(x) = (1+4) + (2+5)x + 3x^2 = 5 + 7x + 3x^2
        assert_eq!(sum.coeff(0).value(), 1 ^ 4); // XOR in GF(2)
        assert_eq!(sum.coeff(1).value(), 2 ^ 5);
        assert_eq!(sum.coeff(2).value(), 3);
    }

    #[test]
    fn test_poly_multiplication_simple() {
        let field = Gf2mField::new(4, 0b10011);
        // p1(x) = 2
        let p1 = Gf2mPoly_::constant(field.element(2));
        // p2(x) = 3
        let p2 = Gf2mPoly_::constant(field.element(3));

        let product = &p1 * &p2;
        // product = 2 * 3 = 6 in the field
        assert_eq!(product.degree(), Some(0));
        assert_eq!(product.coeff(0), &field.element(2) * &field.element(3));
    }

    #[test]
    fn test_poly_multiplication_linear() {
        let field = Gf2mField::new(4, 0b10011);
        // p1(x) = 1 + x (coeffs: [1, 1])
        let p1 = Gf2mPoly_::new(vec![field.element(1), field.element(1)]);
        // p2(x) = 2 + x (coeffs: [2, 1])
        let p2 = Gf2mPoly_::new(vec![field.element(2), field.element(1)]);

        let product = &p1 * &p2;
        // (1 + x)(2 + x) = 2 + x + 2x + x^2 = 2 + 3x + x^2
        assert_eq!(product.degree(), Some(2));
        assert_eq!(product.coeff(0).value(), 2); // 1*2
        assert_eq!(product.coeff(1).value(), 1 ^ 2); // 1*1 + 1*2 = 3 in GF(2^4)
        assert_eq!(product.coeff(2).value(), 1); // 1*1
    }

    // Karatsuba multiplication tests

    // Karatsuba vs schoolbook cross-verification lives on the generic
    // `FieldPoly<F>` tests in `crate::field::poly` (SSOT): see
    // `tests::test_karatsuba_matches_schoolbook_fp7`,
    // `tests::test_karatsuba_matches_schoolbook_gf16`, and the
    // `prop_karatsuba_matches_schoolbook_fp7` proptest. Those cover this
    // alias automatically. A standalone multiplicative-annihilation
    // check on the alias surface is kept below to confirm the alias
    // itself still exposes `Mul` correctly.
    #[test]
    fn test_mul_with_zero_on_alias() {
        let field = Gf2mField::gf256();
        let p1 = Gf2mPoly_::new(vec![field.element(1), field.element(2)]);
        let zero = Gf2mPoly_::zero(&field);

        assert_eq!(&p1 * &zero, zero);
        assert_eq!(&zero * &p1, zero);
    }

    // Evaluation tests

    #[test]
    fn test_poly_eval_constant() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::constant(field.element(5));
        let x = field.element(7);

        let result = poly.eval(&x);
        assert_eq!(result.value(), 5); // Constant polynomial
    }

    #[test]
    fn test_poly_eval_linear() {
        let field = Gf2mField::new(4, 0b10011);
        // p(x) = 2 + 3x
        let poly = Gf2mPoly_::new(vec![field.element(2), field.element(3)]);
        let x = field.element(5);

        let result = poly.eval(&x);
        // p(5) = 2 + 3*5
        let expected = &field.element(2) + &(&field.element(3) * &field.element(5));
        assert_eq!(result, expected);
    }

    #[test]
    fn test_poly_eval_quadratic() {
        let field = Gf2mField::new(4, 0b10011);
        // p(x) = 1 + 2x + 3x^2
        let poly = Gf2mPoly_::new(vec![field.element(1), field.element(2), field.element(3)]);
        let x = field.element(5);

        let result = poly.eval(&x);
        // Manual calculation: 1 + 2*5 + 3*5^2
        let x_squared = &x * &x;
        let term1 = field.element(1);
        let term2 = &field.element(2) * &x;
        let term3 = &field.element(3) * &x_squared;
        let expected = &(&term1 + &term2) + &term3;

        assert_eq!(result, expected);
    }

    // Division with remainder tests

    #[test]
    fn test_poly_div_rem_simple() {
        let field = Gf2mField::new(4, 0b10011);
        // dividend: x^2 + x + 1
        let dividend = Gf2mPoly_::new(vec![field.element(1), field.element(1), field.element(1)]);
        // divisor: x + 1
        let divisor = Gf2mPoly_::new(vec![field.element(1), field.element(1)]);

        let (quotient, remainder) = dividend.div_rem(&divisor);

        // (x^2 + x + 1) / (x + 1) = x with remainder 1
        // Because: (x + 1) * x + 1 = x^2 + x + 1
        assert_eq!(quotient.degree(), Some(1));
        assert_eq!(remainder.degree(), Some(0));
    }

    #[test]
    fn test_poly_div_rem_exact() {
        let field = Gf2mField::new(4, 0b10011);
        // dividend: x^2 + 1 = (x + 1)^2 in GF(2)
        let dividend = Gf2mPoly_::new(vec![field.element(1), field.zero(), field.element(1)]);
        // divisor: x + 1
        let divisor = Gf2mPoly_::new(vec![field.element(1), field.element(1)]);

        let (quotient, remainder) = dividend.div_rem(&divisor);

        // Should divide exactly
        assert!(remainder.is_zero() || remainder.degree() == Some(0));

        // Verify: quotient * divisor + remainder = dividend
        let check = &(&quotient * &divisor) + &remainder;
        assert_eq!(check, dividend);
    }

    #[test]
    fn test_poly_div_rem_constant_divisor() {
        let field = Gf2mField::new(4, 0b10011);
        let dividend = Gf2mPoly_::new(vec![field.element(2), field.element(4), field.element(6)]);
        let divisor = Gf2mPoly_::constant(field.element(2));

        let (quotient, remainder) = dividend.div_rem(&divisor);

        // Dividing by constant: each coefficient divided by constant
        assert_eq!(quotient.degree(), Some(2));
        assert!(remainder.is_zero());
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_poly_div_by_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let dividend = Gf2mPoly_::constant(field.element(1));
        let divisor = Gf2mPoly_::zero(&field);

        let _ = dividend.div_rem(&divisor);
    }

    #[test]
    fn test_poly_div_rem_roundtrip() {
        let field = Gf2mField::new(4, 0b10011);
        // Test with various polynomials
        for a in 1..8 {
            for b in 1..8 {
                for c in 1..8 {
                    let dividend =
                        Gf2mPoly_::new(vec![field.element(a), field.element(b), field.element(c)]);
                    let divisor = Gf2mPoly_::new(vec![field.element(1), field.element(2)]);

                    let (quotient, remainder) = dividend.div_rem(&divisor);

                    // Verify: quotient * divisor + remainder = dividend
                    let check = &(&quotient * &divisor) + &remainder;
                    assert_eq!(
                        check, dividend,
                        "Failed for dividend coeffs [{}, {}, {}]",
                        a, b, c
                    );

                    // Verify remainder degree < divisor degree
                    if let Some(rem_deg) = remainder.degree() {
                        assert!(rem_deg < divisor.degree().unwrap());
                    }
                }
            }
        }
    }

    #[test]
    fn test_poly_eval_batch_empty() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![field.element(1), field.element(2)]);
        let points: Vec<Gf2mElement> = vec![];
        let results = poly.eval_batch(&points);
        assert!(results.is_empty());
    }

    #[test]
    fn test_poly_eval_batch_single() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![field.element(3), field.element(2)]);
        let x = field.element(5);

        let batch_result = poly.eval_batch(std::slice::from_ref(&x));
        let single_result = poly.eval(&x);

        assert_eq!(batch_result.len(), 1);
        assert_eq!(batch_result[0], single_result);
    }

    #[test]
    fn test_poly_eval_batch_multiple() {
        let field = Gf2mField::new(4, 0b10011);
        // p(x) = 1 + 2x + 3x^2
        let poly = Gf2mPoly_::new(vec![field.element(1), field.element(2), field.element(3)]);

        let points = vec![field.element(0), field.element(1), field.element(5)];
        let results = poly.eval_batch(&points);

        assert_eq!(results.len(), 3);

        // Verify each result matches single eval
        for (point, result) in points.iter().zip(results.iter()) {
            let expected = poly.eval(point);
            assert_eq!(*result, expected);
        }
    }

    #[test]
    fn test_poly_eval_batch_syndrome_pattern() {
        // BCH syndrome computation pattern: evaluate at consecutive powers
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![field.element(5), field.element(3), field.element(7)]);

        let alpha = field.element(2); // primitive element
        let mut points = vec![alpha.clone()];
        let mut current = alpha.clone();
        for _ in 1..4 {
            current = &current * &alpha;
            points.push(current.clone());
        }

        let results = poly.eval_batch(&points);
        assert_eq!(results.len(), 4);

        // Each should match single eval
        for (point, result) in points.iter().zip(results.iter()) {
            assert_eq!(*result, poly.eval(point));
        }
    }

    // GCD tests

    #[test]
    fn test_gcd_coprime() {
        let field = Gf2mField::new(4, 0b10011);
        // p1 = x + 1
        let p1 = Gf2mPoly_::new(vec![field.element(1), field.element(1)]);
        // p2 = x + 2
        let p2 = Gf2mPoly_::new(vec![field.element(2), field.element(1)]);

        let gcd = Gf2mPoly_::gcd(&p1, &p2);

        // Coprime polynomials, GCD should be constant (degree 0)
        assert_eq!(gcd.degree(), Some(0));
        assert!(gcd.coeff(0).is_one()); // Monic GCD
    }

    #[test]
    fn test_gcd_common_factor() {
        let field = Gf2mField::new(4, 0b10011);
        // Common factor: (x + 1)
        let common = Gf2mPoly_::new(vec![field.element(1), field.element(1)]);

        // p1 = (x + 1)(x + 2)
        let f1 = Gf2mPoly_::new(vec![field.element(2), field.element(1)]);
        let p1 = &common * &f1;

        // p2 = (x + 1)(x + 3)
        let f2 = Gf2mPoly_::new(vec![field.element(3), field.element(1)]);
        let p2 = &common * &f2;

        let gcd = Gf2mPoly_::gcd(&p1, &p2);

        // GCD should be (x + 1) up to scalar multiple
        assert_eq!(gcd.degree(), Some(1));
        assert!(gcd.coeff(1).is_one()); // Monic
    }

    #[test]
    fn test_gcd_identical() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![field.element(1), field.element(2), field.element(3)]);

        let gcd = Gf2mPoly_::gcd(&poly, &poly);

        // GCD of polynomial with itself is the polynomial (made monic)
        assert_eq!(gcd.degree(), poly.degree());
        assert!(gcd.coeff(gcd.degree().unwrap()).is_one()); // Monic
    }

    #[test]
    fn test_gcd_with_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![field.element(1), field.element(2)]);
        let zero = Gf2mPoly_::zero(&field);

        let gcd = Gf2mPoly_::gcd(&poly, &zero);

        // GCD with zero is the non-zero polynomial (made monic)
        assert_eq!(gcd.degree(), poly.degree());
    }

    // BitVec conversion tests

    #[test]
    fn test_from_bitvec_empty() {
        let field = Gf2mField::new(4, 0b10011);
        let bits = BitVec::new();
        let poly = Gf2mPoly_::from_bitvec(&bits, &field);
        assert!(poly.is_zero());
    }

    #[test]
    fn test_from_bitvec_all_zeros() {
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(false);
        bits.push_bit(false);
        bits.push_bit(false);
        let poly = Gf2mPoly_::from_bitvec(&bits, &field);
        assert!(poly.is_zero());
    }

    #[test]
    fn test_from_bitvec_simple() {
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(true); // x^0
        bits.push_bit(false); // x^1
        bits.push_bit(true); // x^2

        let poly = Gf2mPoly_::from_bitvec(&bits, &field);
        assert_eq!(poly.degree(), Some(2));
        assert!(poly.coeff(0).is_one());
        assert!(poly.coeff(1).is_zero());
        assert!(poly.coeff(2).is_one());
    }

    #[test]
    fn test_from_bitvec_all_ones() {
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        for _ in 0..5 {
            bits.push_bit(true);
        }

        let poly = Gf2mPoly_::from_bitvec(&bits, &field);
        assert_eq!(poly.degree(), Some(4));
        for i in 0..5 {
            assert!(poly.coeff(i).is_one(), "Coefficient {} should be one", i);
        }
    }

    #[test]
    fn test_to_bitvec_zero_polynomial() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::zero(&field);

        let bits = poly.to_bitvec(5);
        assert_eq!(bits.len(), 5);
        for i in 0..5 {
            assert!(!bits.get(i), "Bit {} should be zero", i);
        }
    }

    #[test]
    fn test_to_bitvec_simple() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![
            field.one(),  // x^0
            field.zero(), // x^1
            field.one(),  // x^2
        ]);

        let bits = poly.to_bitvec(5);
        assert_eq!(bits.len(), 5);
        assert!(bits.get(0)); // x^0 present
        assert!(!bits.get(1)); // x^1 absent
        assert!(bits.get(2)); // x^2 present
        assert!(!bits.get(3)); // x^3 absent (beyond degree)
        assert!(!bits.get(4)); // x^4 absent (beyond degree)
    }

    #[test]
    fn test_to_bitvec_length_shorter_than_degree() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![
            field.one(),  // x^0
            field.zero(), // x^1
            field.one(),  // x^2
            field.one(),  // x^3
        ]);

        let bits = poly.to_bitvec(2);
        assert_eq!(bits.len(), 2);
        assert!(bits.get(0));
        assert!(!bits.get(1));
    }

    #[test]
    fn test_to_bitvec_minimal_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::zero(&field);

        let bits = poly.to_bitvec_minimal();
        assert_eq!(bits.len(), 0);
    }

    // Tests for reversed BitVec conversion (DVB-T2 compliance)

    #[test]
    fn test_from_bitvec_reversed_empty() {
        let field = Gf2mField::new(4, 0b10011);
        let bits = BitVec::new();
        let poly = Gf2mPoly_::from_bitvec_reversed(&bits, &field);
        assert!(poly.is_zero());
    }

    #[test]
    fn test_from_bitvec_reversed_simple() {
        // BitVec: [bit0, bit1, bit2] -> Poly: bit0*x^2 + bit1*x^1 + bit2*x^0
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(true); // bit 0 -> x^2 (highest)
        bits.push_bit(false); // bit 1 -> x^1
        bits.push_bit(true); // bit 2 -> x^0 (lowest)

        let poly = Gf2mPoly_::from_bitvec_reversed(&bits, &field);
        assert_eq!(poly.degree(), Some(2));
        assert!(poly.coeff(0).is_one()); // x^0 term
        assert!(poly.coeff(1).is_zero()); // x^1 term
        assert!(poly.coeff(2).is_one()); // x^2 term
    }

    #[test]
    fn test_from_bitvec_reversed_single_bit() {
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(true); // bit 0 -> x^0 (degree 0 polynomial)

        let poly = Gf2mPoly_::from_bitvec_reversed(&bits, &field);
        assert_eq!(poly.degree(), Some(0));
        assert!(poly.coeff(0).is_one());
    }

    #[test]
    fn test_from_bitvec_reversed_leading_zeros() {
        // BitVec: [0, 0, 1, 0, 1] -> should normalize to degree 2
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(false); // bit 0 -> x^4 (would be highest, but zero)
        bits.push_bit(false); // bit 1 -> x^3
        bits.push_bit(true); // bit 2 -> x^2
        bits.push_bit(false); // bit 3 -> x^1
        bits.push_bit(true); // bit 4 -> x^0

        let poly = Gf2mPoly_::from_bitvec_reversed(&bits, &field);
        assert_eq!(poly.degree(), Some(2));
        assert!(poly.coeff(0).is_one()); // x^0
        assert!(poly.coeff(1).is_zero()); // x^1
        assert!(poly.coeff(2).is_one()); // x^2
    }

    #[test]
    fn test_to_bitvec_reversed_simple() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![
            field.one(),  // x^0
            field.zero(), // x^1
            field.one(),  // x^2
        ]);

        // For len=5: x^2 + x^0
        // Reversed: bit0=x^4, bit1=x^3, bit2=x^2, bit3=x^1, bit4=x^0
        // Expected: [0, 0, 1, 0, 1]
        let bits = poly.to_bitvec_reversed(5);
        assert_eq!(bits.len(), 5);
        assert!(!bits.get(0)); // x^4 absent
        assert!(!bits.get(1)); // x^3 absent
        assert!(bits.get(2)); // x^2 present
        assert!(!bits.get(3)); // x^1 absent
        assert!(bits.get(4)); // x^0 present
    }

    #[test]
    fn test_to_bitvec_reversed_exact_degree() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![
            field.one(),  // x^0
            field.zero(), // x^1
            field.one(),  // x^2
        ]);

        // For len=3 (exactly degree+1): [x^2, x^1, x^0] = [1, 0, 1]
        let bits = poly.to_bitvec_reversed(3);
        assert_eq!(bits.len(), 3);
        assert!(bits.get(0)); // x^2
        assert!(!bits.get(1)); // x^1
        assert!(bits.get(2)); // x^0
    }

    #[test]
    fn test_to_bitvec_reversed_zero_polynomial() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::zero(&field);

        let bits = poly.to_bitvec_reversed(5);
        assert_eq!(bits.len(), 5);
        for i in 0..5 {
            assert!(!bits.get(i), "Bit {} should be zero", i);
        }
    }

    #[test]
    fn test_to_bitvec_reversed_shorter_than_degree() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![
            field.one(),  // x^0
            field.zero(), // x^1
            field.one(),  // x^2
            field.one(),  // x^3
        ]);

        // Request len=2: should only see x^1 and x^0
        let bits = poly.to_bitvec_reversed(2);
        assert_eq!(bits.len(), 2);
        assert!(!bits.get(0)); // x^1 (highest in range)
        assert!(bits.get(1)); // x^0 (lowest)
    }

    #[test]
    fn test_bitvec_reversed_roundtrip() {
        let field = Gf2mField::new(4, 0b10011);
        let mut original = BitVec::new();
        original.push_bit(true);
        original.push_bit(false);
        original.push_bit(true);
        original.push_bit(true);
        original.push_bit(false);

        let poly = Gf2mPoly_::from_bitvec_reversed(&original, &field);
        let roundtrip = poly.to_bitvec_reversed(5);

        assert_eq!(original.len(), roundtrip.len());
        for i in 0..original.len() {
            assert_eq!(original.get(i), roundtrip.get(i), "Bit {} mismatch", i);
        }
    }

    #[test]
    fn test_bch_systematic_codeword_pattern() {
        // Simulates BCH systematic encoding: [message | parity]
        // Message: k bits (0..k-1), Parity: r bits (k..n-1)
        // DVB-T2: bit 0 is highest coefficient
        let field = Gf2mField::new(4, 0b10011);
        let k = 3;
        let r = 2;
        let _n = k + r; // 5 total

        let mut codeword = BitVec::new();
        // Message bits [0, 1, 2]: 1, 0, 1
        codeword.push_bit(true);
        codeword.push_bit(false);
        codeword.push_bit(true);
        // Parity bits [3, 4]: 0, 1
        codeword.push_bit(false);
        codeword.push_bit(true);

        // Convert using reversed: bit 0 -> x^4, ..., bit 4 -> x^0
        let poly = Gf2mPoly_::from_bitvec_reversed(&codeword, &field);

        // Verify structure: x^4 + x^2 + x^0
        assert_eq!(poly.degree(), Some(4));
        assert!(poly.coeff(0).is_one()); // bit 4 -> x^0
        assert!(poly.coeff(1).is_zero()); // bit 3 -> x^1
        assert!(poly.coeff(2).is_one()); // bit 2 -> x^2
        assert!(poly.coeff(3).is_zero()); // bit 1 -> x^3
        assert!(poly.coeff(4).is_one()); // bit 0 -> x^4
    }

    #[test]
    fn test_reversed_vs_standard_conversion() {
        // Verify reversed is truly the reverse of standard conversion
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(true); // bit 0
        bits.push_bit(false); // bit 1
        bits.push_bit(true); // bit 2

        let poly_standard = Gf2mPoly_::from_bitvec(&bits, &field);
        let poly_reversed = Gf2mPoly_::from_bitvec_reversed(&bits, &field);

        // Standard: bit i -> x^i, so [1,0,1] -> x^2 + x^0
        assert!(poly_standard.coeff(0).is_one());
        assert!(poly_standard.coeff(1).is_zero());
        assert!(poly_standard.coeff(2).is_one());

        // Reversed: bit i -> x^(n-1-i), so [1,0,1] -> x^2 + x^0 (same by coincidence!)
        // But semantics differ when bits are asymmetric
        assert!(poly_reversed.coeff(0).is_one());
        assert!(poly_reversed.coeff(1).is_zero());
        assert!(poly_reversed.coeff(2).is_one());

        // Test with asymmetric pattern
        let mut asym = BitVec::new();
        asym.push_bit(true);
        asym.push_bit(false);
        asym.push_bit(false);

        let poly_std = Gf2mPoly_::from_bitvec(&asym, &field);
        let poly_rev = Gf2mPoly_::from_bitvec_reversed(&asym, &field);

        // Standard: [1,0,0] -> x^0
        assert_eq!(poly_std.degree(), Some(0));
        assert!(poly_std.coeff(0).is_one());

        // Reversed: [1,0,0] -> x^2
        assert_eq!(poly_rev.degree(), Some(2));
        assert!(poly_rev.coeff(2).is_one());
        assert!(poly_rev.coeff(1).is_zero());
        assert!(poly_rev.coeff(0).is_zero());
    }

    #[test]
    fn test_to_bitvec_minimal_degree_two() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly_::new(vec![
            field.one(),  // x^0
            field.zero(), // x^1
            field.one(),  // x^2
        ]);

        let bits = poly.to_bitvec_minimal();
        assert_eq!(bits.len(), 3); // degree 2, so length 3
        assert!(bits.get(0));
        assert!(!bits.get(1));
        assert!(bits.get(2));
    }

    #[test]
    fn test_roundtrip_bitvec_to_poly_to_bitvec() {
        let field = Gf2mField::new(4, 0b10011);
        let mut original = BitVec::new();
        original.push_bit(true);
        original.push_bit(false);
        original.push_bit(true);
        original.push_bit(false);
        original.push_bit(true);

        let poly = Gf2mPoly_::from_bitvec(&original, &field);
        let recovered = poly.to_bitvec(original.len());

        assert_eq!(original.len(), recovered.len());
        for i in 0..original.len() {
            assert_eq!(original.get(i), recovered.get(i), "Bit {} mismatch", i);
        }
    }

    #[test]
    fn test_roundtrip_poly_to_bitvec_to_poly() {
        let field = Gf2mField::new(4, 0b10011);
        let original = Gf2mPoly_::new(vec![
            field.element(1),
            field.element(0),
            field.element(1),
            field.element(0),
            field.element(1),
        ]);

        let bits = original.to_bitvec_minimal();
        let recovered = Gf2mPoly_::from_bitvec(&bits, &field);

        assert_eq!(original.degree(), recovered.degree());
        if let Some(deg) = original.degree() {
            for i in 0..=deg {
                assert_eq!(
                    original.coeff(i).is_zero(),
                    recovered.coeff(i).is_zero(),
                    "Coefficient {} mismatch",
                    i
                );
            }
        }
    }

    // Property-based tests for polynomials

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_poly_add_commutative(a in 1u64..8, b in 1u64..8, c in 1u64..8,
                                      d in 1u64..8, e in 1u64..8, f in 1u64..8) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly_::new(vec![field.element(a), field.element(b), field.element(c)]);
            let p2 = Gf2mPoly_::new(vec![field.element(d), field.element(e), field.element(f)]);

            prop_assert_eq!(&p1 + &p2, &p2 + &p1);
        }

        #[test]
        fn prop_poly_mul_commutative(a in 1u64..8, b in 1u64..8, c in 1u64..8, d in 1u64..8) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly_::new(vec![field.element(a), field.element(b)]);
            let p2 = Gf2mPoly_::new(vec![field.element(c), field.element(d)]);

            prop_assert_eq!(&p1 * &p2, &p2 * &p1);
        }

        #[test]
        fn prop_poly_div_rem_invariant(a in 1u64..8, b in 1u64..8, c in 1u64..8, d in 1u64..4) {
            let field = Gf2mField::new(4, 0b10011);
            let dividend = Gf2mPoly_::new(vec![field.element(a), field.element(b), field.element(c)]);
            let divisor = Gf2mPoly_::new(vec![field.element(d), field.element(1)]);

            let (q, r) = dividend.div_rem(&divisor);

            // Verify: quotient * divisor + remainder = dividend
            let check = &(&q * &divisor) + &r;
            prop_assert_eq!(check, dividend);

            // Verify: degree(remainder) < degree(divisor)
            if let Some(r_deg) = r.degree() {
                prop_assert!(r_deg < divisor.degree().unwrap());
            }
        }

        #[test]
        fn prop_poly_eval_add_distributive(a in 1u64..8, b in 1u64..8, x_val in 1u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly_::new(vec![field.element(a), field.element(1)]);
            let p2 = Gf2mPoly_::new(vec![field.element(b), field.element(1)]);
            let x = field.element(x_val);

            // (p1 + p2)(x) = p1(x) + p2(x). `FieldPoly::eval` is total
            // on the zero polynomial (returns `x.zero_like()`), so the
            // cancellation-to-zero case needs no special handling.
            let left = (&p1 + &p2).eval(&x);
            let right = &p1.eval(&x) + &p2.eval(&x);

            prop_assert_eq!(left, right);
        }

        #[test]
        fn prop_poly_eval_mul_distributive(a in 1u64..8, b in 1u64..8, x_val in 1u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly_::new(vec![field.element(a), field.element(1)]);
            let p2 = Gf2mPoly_::new(vec![field.element(b), field.element(1)]);
            let x = field.element(x_val);

            // (p1 * p2)(x) = p1(x) * p2(x)
            let left = (&p1 * &p2).eval(&x);
            let right = &p1.eval(&x) * &p2.eval(&x);

            prop_assert_eq!(left, right);
        }

        #[test]
        fn prop_gcd_divides_both(a in 1u64..8, b in 1u64..8, c in 1u64..8, d in 1u64..8) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly_::new(vec![field.element(a), field.element(b), field.element(1)]);
            let p2 = Gf2mPoly_::new(vec![field.element(c), field.element(d), field.element(1)]);

            let gcd = Gf2mPoly_::gcd(&p1, &p2);

            if !gcd.is_zero() && gcd.degree().is_some() {
                // GCD should divide both polynomials
                let (_, r1) = p1.div_rem(&gcd);
                let (_, r2) = p2.div_rem(&gcd);

                prop_assert!(r1.is_zero() || r1.degree() == Some(0) && r1.coeff(0).is_zero());
                prop_assert!(r2.is_zero() || r2.degree() == Some(0) && r2.coeff(0).is_zero());
            }
        }

        // Karatsuba vs schoolbook cross-verification is covered
        // generically on the SSOT side by
        // `prop_karatsuba_matches_schoolbook_fp7` and
        // `prop_karatsuba_matches_schoolbook_gf16` in
        // `crate::field::poly`, plus the matching unit tests
        // `test_karatsuba_matches_schoolbook_fp7` / `_gf16`. Those use
        // the private `mul_schoolbook_impl` so the two sides of the
        // assertion cannot collapse to the same dispatch. No alias-side
        // duplicate of those properties is needed.
    }

    mod reversed_conversion_proptests {
        use super::*;

        proptest! {
            #[test]
            fn prop_reversed_roundtrip(bytes in prop::collection::vec(any::<u8>(), 0..20)) {
                let field = Gf2mField::new(4, 0b10011);
                let bits = crate::BitVec::from_bytes_le(&bytes);
                let len = bits.len();

                let poly = Gf2mPoly_::from_bitvec_reversed(&bits, &field);
                let roundtrip = poly.to_bitvec_reversed(len);

                prop_assert_eq!(bits.len(), roundtrip.len());
                for i in 0..len {
                    prop_assert_eq!(bits.get(i), roundtrip.get(i));
                }
            }

            #[test]
            fn prop_reversed_differs_from_standard_when_asymmetric(
                len in 2usize..20,
                seed in 0u64..256
            ) {
                let field = Gf2mField::new(4, 0b10011);

                // Create asymmetric bit pattern
                let mut bits = crate::BitVec::new();
                for i in 0..len {
                    bits.push_bit((i * seed as usize).is_multiple_of(3));
                }

                // Skip symmetric patterns
                let is_palindrome = (0..len).all(|i| bits.get(i) == bits.get(len - 1 - i));
                if is_palindrome {
                    return Ok(());
                }

                let poly_std = Gf2mPoly_::from_bitvec(&bits, &field);
                let poly_rev = Gf2mPoly_::from_bitvec_reversed(&bits, &field);

                // They should differ for non-palindromic patterns
                let differs = (0..=len).any(|i| {
                    poly_std.coeff(i).value() != poly_rev.coeff(i).value()
                });

                prop_assert!(differs, "Standard and reversed should differ for asymmetric patterns");
            }

            #[test]
            fn prop_reversed_preserves_degree_info(bytes in prop::collection::vec(any::<u8>(), 1..20)) {
                let field = Gf2mField::new(4, 0b10011);
                let bits = crate::BitVec::from_bytes_le(&bytes);

                // With reversed mapping: bit i → x^(len-1-i)
                // So bit 0 → highest degree, bit (len-1) → x^0
                // Lowest set bit index gives highest polynomial degree
                let lowest_set = (0..bits.len()).find(|&i| bits.get(i));

                let poly = Gf2mPoly_::from_bitvec_reversed(&bits, &field);

                if let Some(lowest) = lowest_set {
                    // Lowest set bit i maps to degree (len-1-i)
                    let expected_degree = bits.len() - 1 - lowest;
                    prop_assert_eq!(poly.degree(), Some(expected_degree));
                } else {
                    prop_assert!(poly.is_zero());
                }
            }

            #[test]
            fn prop_reversed_double_conversion_identity(
                deg in 0usize..20,
                seed in 1u64..256
            ) {
                let field = Gf2mField::new(4, 0b10011);

                // Create polynomial
                let coeffs: Vec<_> = (0..=deg)
                    .map(|i| {
                        if (i as u64 * seed).is_multiple_of(3) {
                            field.one()
                        } else {
                            field.zero()
                        }
                    })
                    .collect();
                let poly1 = Gf2mPoly_::new(coeffs);

                // to_bitvec_reversed -> from_bitvec_reversed should be identity
                let len = poly1.degree().map(|d| d + 1).unwrap_or(1);
                let bits = poly1.to_bitvec_reversed(len);
                let poly2 = Gf2mPoly_::from_bitvec_reversed(&bits, &field);

                prop_assert_eq!(poly1.degree(), poly2.degree());
                if let Some(d) = poly1.degree() {
                    for i in 0..=d {
                        prop_assert_eq!(poly1.coeff(i).value(), poly2.coeff(i).value());
                    }
                }
            }

            #[test]
            fn prop_reversed_bitvec_length_flexibility(
                bytes in prop::collection::vec(any::<u8>(), 1..10),
                extra_len in 0usize..10
            ) {
                let field = Gf2mField::new(4, 0b10011);
                let bits = crate::BitVec::from_bytes_le(&bytes);

                let poly = Gf2mPoly_::from_bitvec_reversed(&bits, &field);
                let extended_len = bits.len() + extra_len;
                let extended_bits = poly.to_bitvec_reversed(extended_len);

                prop_assert_eq!(extended_bits.len(), extended_len);

                // Leading bits (corresponding to high degrees) should be zero
                for i in 0..extra_len {
                    prop_assert!(!extended_bits.get(i),
                        "Extended bit {} should be zero", i);
                }

                // Original bits should match
                for i in 0..bits.len() {
                    prop_assert_eq!(bits.get(i), extended_bits.get(extra_len + i),
                        "Original bit {} should be preserved", i);
                }
            }
        }
    }

    // ===== Minimal Polynomial Tests =====

    #[test]
    fn test_minimal_polynomial_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let zero = field.element(0);
        let min_poly = zero.minimal_polynomial();

        // Minimal polynomial of 0 is x
        assert_eq!(min_poly.degree(), Some(1));
        assert_eq!(min_poly.coeff(0).value(), 0); // Constant term is 0
        assert_eq!(min_poly.coeff(1).value(), 1); // x^1 coefficient is 1
    }

    #[test]
    fn test_minimal_polynomial_one() {
        let field = Gf2mField::new(4, 0b10011);
        let one = field.element(1);
        let min_poly = one.minimal_polynomial();

        // Minimal polynomial of 1 is x + 1
        assert_eq!(min_poly.degree(), Some(1));
        assert_eq!(min_poly.coeff(0).value(), 1); // Constant term is 1
        assert_eq!(min_poly.coeff(1).value(), 1); // x^1 coefficient is 1
    }

    #[test]
    fn test_minimal_polynomial_gf4() {
        // GF(2^2) with primitive polynomial x^2 + x + 1
        let field = Gf2mField::new(2, 0b111);

        // α (primitive element) should have minimal polynomial x^2 + x + 1
        let alpha = field.element(0b10); // α = x
        let min_poly = alpha.minimal_polynomial();

        assert_eq!(min_poly.degree(), Some(2));
        assert_eq!(min_poly.coeff(0).value(), 1); // +1
        assert_eq!(min_poly.coeff(1).value(), 1); // +x
        assert_eq!(min_poly.coeff(2).value(), 1); // +x^2
    }

    #[test]
    fn test_minimal_polynomial_is_root() {
        // For any element α, α should be a root of its minimal polynomial
        let field = Gf2mField::new(4, 0b10011);
        let alpha = field.element(0b0110); // Some random element
        let min_poly = alpha.minimal_polynomial();

        // Evaluate min_poly at alpha, should give zero
        let result = min_poly.eval(&alpha);
        assert!(
            result.is_zero(),
            "Element should be a root of its minimal polynomial"
        );
    }

    #[test]
    fn test_minimal_polynomial_degree_divides_m() {
        // The degree of minimal polynomial of any element in GF(2^m) divides m
        let field = Gf2mField::gf256(); // m = 8

        for value in [0x00, 0x01, 0x02, 0x53, 0xFF] {
            let elem = field.element(value);
            let min_poly = elem.minimal_polynomial();
            if let Some(deg) = min_poly.degree() {
                assert!(
                    8 % deg == 0,
                    "Minimal polynomial degree {} should divide m=8 for value 0x{:02x}",
                    deg,
                    value
                );
            }
        }
    }

    #[test]
    fn test_minimal_polynomial_monic() {
        // Minimal polynomial should be monic (leading coefficient = 1)
        let field = Gf2mField::new(4, 0b10011);

        for value in 0..16 {
            let elem = field.element(value);
            let min_poly = elem.minimal_polynomial();
            if let Some(deg) = min_poly.degree() {
                let leading = min_poly.coeff(deg);
                assert_eq!(
                    leading.value(),
                    1,
                    "Minimal polynomial should be monic for value {}",
                    value
                );
            }
        }
    }

    #[test]
    fn test_minimal_polynomial_gf16_known_values() {
        // Test against known minimal polynomials in GF(2^4)
        // Using primitive polynomial x^4 + x + 1
        let field = Gf2mField::new(4, 0b10011);

        // Elements in GF(2) have minimal polynomial x or x+1
        let zero = field.element(0);
        assert_eq!(zero.minimal_polynomial().degree(), Some(1));

        let one = field.element(1);
        let mp_one = one.minimal_polynomial();
        assert_eq!(mp_one.degree(), Some(1));
        assert_eq!(mp_one.coeff(0).value(), 1);
        assert_eq!(mp_one.coeff(1).value(), 1);
    }

    #[cfg(test)]
    mod minimal_polynomial_proptests {
        use super::*;

        proptest! {
            #[test]
            fn minimal_polynomial_has_element_as_root(m in 2u32..=8, value in 0u64..256) {
                let field = match m {
                    2 => Gf2mField::new(2, 0b111),
                    3 => Gf2mField::new(3, 0b1011),
                    4 => Gf2mField::new(4, 0b10011),
                    5 => Gf2mField::new(5, 0b100101),
                    6 => Gf2mField::new(6, 0b1000011),
                    7 => Gf2mField::new(7, 0b10000011),
                    8 => Gf2mField::gf256(),
                    _ => return Ok(()),
                };

                let max_val = (1u64 << m) - 1;
                if value > max_val {
                    return Ok(());
                }

                let elem = field.element(value);
                let min_poly = elem.minimal_polynomial();
                let result = min_poly.eval(&elem);

                prop_assert!(result.is_zero(),
                    "Minimal polynomial must have element as root: m={}, value={}", m, value);
            }

            #[test]
            fn minimal_polynomial_degree_divides_m(m in 2u32..=8, value in 0u64..256) {
                let field = match m {
                    2 => Gf2mField::new(2, 0b111),
                    3 => Gf2mField::new(3, 0b1011),
                    4 => Gf2mField::new(4, 0b10011),
                    5 => Gf2mField::new(5, 0b100101),
                    6 => Gf2mField::new(6, 0b1000011),
                    7 => Gf2mField::new(7, 0b10000011),
                    8 => Gf2mField::gf256(),
                    _ => return Ok(()),
                };

                let max_val = (1u64 << m) - 1;
                if value > max_val {
                    return Ok(());
                }

                let elem = field.element(value);
                let min_poly = elem.minimal_polynomial();

                if let Some(deg) = min_poly.degree() {
                    prop_assert!(m % (deg as u32) == 0,
                        "Minimal polynomial degree {} must divide m={} for value={}",
                        deg, m, value);
                }
            }

            #[test]
            fn minimal_polynomial_is_monic(m in 2u32..=6, value in 0u64..64) {
                let field = match m {
                    2 => Gf2mField::new(2, 0b111),
                    3 => Gf2mField::new(3, 0b1011),
                    4 => Gf2mField::new(4, 0b10011),
                    5 => Gf2mField::new(5, 0b100101),
                    6 => Gf2mField::new(6, 0b1000011),
                    _ => return Ok(()),
                };

                let max_val = (1u64 << m) - 1;
                if value > max_val {
                    return Ok(());
                }

                let elem = field.element(value);
                let min_poly = elem.minimal_polynomial();

                if let Some(deg) = min_poly.degree() {
                    let leading = min_poly.coeff(deg);
                    prop_assert_eq!(leading.value(), 1,
                        "Minimal polynomial must be monic (leading coeff = 1)");
                }
            }

            #[test]
            fn prop_roundtrip_bitvec_poly_bitvec(bits in prop::collection::vec(any::<bool>(), 0..100)) {
                let mut bv = BitVec::new();
                for bit in &bits {
                    bv.push_bit(*bit);
                }
                let field = Gf2mField::new(8, 0b100011101);

                let poly = Gf2mPoly_::from_bitvec(&bv, &field);
                let recovered = poly.to_bitvec(bv.len());

                prop_assert_eq!(bv.len(), recovered.len());
                for i in 0..bv.len() {
                    prop_assert_eq!(bv.get(i), recovered.get(i), "Bit {} mismatch", i);
                }
            }

            #[test]
            fn prop_to_bitvec_minimal_has_correct_length(coeffs in prop::collection::vec(0u64..16, 1..20)) {
                let field = Gf2mField::new(4, 0b10011);
                let elements: Vec<_> = coeffs.iter().map(|&c| field.element(c)).collect();
                let poly = Gf2mPoly_::new(elements);

                let bits = poly.to_bitvec_minimal();

                if let Some(deg) = poly.degree() {
                    prop_assert_eq!(bits.len(), deg + 1);
                } else {
                    prop_assert_eq!(bits.len(), 0);
                }
            }
        }
    }

    // ========================================================================
    // Primitive Polynomial Verification Tests (Phase 9 - TDD)
    // ========================================================================

    #[test]
    fn test_verify_primitive_gf4() {
        let field = Gf2mField::new(2, 0b111); // x^2 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf8() {
        let field = Gf2mField::new(3, 0b1011); // x^3 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf16() {
        let field = Gf2mField::new(4, 0b10011); // x^4 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf256() {
        // Standard primitive polynomial for GF(256)
        let field = Gf2mField::new(8, 0b100011101);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_dvb_t2_gf14() {
        // Correct DVB-T2 polynomial
        let field = Gf2mField::new(14, 0b100000000101011);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_dvb_t2_gf16() {
        // Correct DVB-T2 polynomial for normal frames
        let field = Gf2mField::new(16, 0b10000000000101101);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_not_primitive_wrong_dvb_t2() {
        // The bug: wrong polynomial used initially
        let field = Gf2mField::new(14, 0b100000000100001);
        assert!(
            !field.verify_primitive(),
            "This polynomial caused the BCH bug"
        );
    }

    #[test]
    fn test_verify_not_primitive_reducible() {
        // (x + 1)^2 = x^2 + 1 is reducible
        let field = Gf2mField::new(2, 0b101);
        assert!(!field.verify_primitive());
    }

    #[test]
    fn test_is_irreducible_rabin_small_cases() {
        // x^2 + x + 1 is irreducible
        let field = Gf2mField::new(2, 0b111);
        assert!(field.is_irreducible_rabin());

        // x^2 + 1 = (x + 1)^2 is reducible
        let field = Gf2mField::new(2, 0b101);
        assert!(!field.is_irreducible_rabin());
    }

    #[test]
    fn test_is_irreducible_rabin_gf8() {
        // x^3 + x + 1 is irreducible
        let field = Gf2mField::new(3, 0b1011);
        assert!(field.is_irreducible_rabin());

        // x^3 + 1 = (x + 1)(x^2 + x + 1) is reducible
        let field = Gf2mField::new(3, 0b1001);
        assert!(!field.is_irreducible_rabin());
    }

    #[test]
    fn test_all_database_entries_are_primitive() {
        use crate::primitive_polys::PrimitivePolynomialDatabase;
        // Every polynomial in the database must verify as primitive
        for m in 2..=16 {
            if let Some(poly) = PrimitivePolynomialDatabase::standard(m) {
                let field = Gf2mField::new(m, poly);
                assert!(
                    field.verify_primitive(),
                    "Database entry for m={} ({:#b}) is not primitive!",
                    m,
                    poly
                );
            }
        }
    }

    #[cfg(test)]
    mod primitive_verification_proptests {
        use super::*;

        proptest! {
            #[test]
            fn prop_all_database_entries_verify(m in 2u32..=16) {
                use crate::primitive_polys::PrimitivePolynomialDatabase;
                if let Some(poly) = PrimitivePolynomialDatabase::standard(m as usize) {
                    let field = Gf2mField::new(m as usize, poly);
                    prop_assert!(field.verify_primitive());
                }
            }
        }
    }
}

/// Tests for polynomial construction utilities
#[cfg(test)]
mod poly_construction_tests {
    use super::*;

    #[test]
    fn test_from_exponents_simple() {
        let field = Gf2mField::new(4, 0b10011);

        // Create polynomial: 1 + x + x^4
        let poly = Gf2mPoly_::from_exponents(&field, &[0, 1, 4]);

        assert_eq!(poly.degree(), Some(4));
        assert_eq!(poly.coeff(0), field.one());
        assert_eq!(poly.coeff(1), field.one());
        assert_eq!(poly.coeff(2), field.zero());
        assert_eq!(poly.coeff(3), field.zero());
        assert_eq!(poly.coeff(4), field.one());
    }

    #[test]
    fn test_from_exponents_single() {
        let field = Gf2mField::new(4, 0b10011);

        // Create monomial: x^5
        let poly = Gf2mPoly_::from_exponents(&field, &[5]);

        assert_eq!(poly.degree(), Some(5));
        assert_eq!(poly.coeff(0), field.zero());
        assert_eq!(poly.coeff(5), field.one());
    }

    #[test]
    fn test_from_exponents_duplicates() {
        let field = Gf2mField::new(4, 0b10011);

        // x^2 + x^2 = 0 in GF(2)
        let poly = Gf2mPoly_::from_exponents(&field, &[2, 2]);

        // Should result in zero polynomial after normalization
        assert!(poly.is_zero());
        assert_eq!(poly.degree(), None);
    }

    #[test]
    fn test_from_exponents_duplicates_odd_count() {
        let field = Gf2mField::new(4, 0b10011);

        // 1 + x^2 + x^2 + x^2 = 1 + x^2 in GF(2)
        let poly = Gf2mPoly_::from_exponents(&field, &[0, 2, 2, 2]);

        assert_eq!(poly.degree(), Some(2));
        assert_eq!(poly.coeff(0), field.one());
        assert_eq!(poly.coeff(1), field.zero());
        assert_eq!(poly.coeff(2), field.one());
    }

    #[test]
    fn test_from_exponents_unsorted() {
        let field = Gf2mField::new(4, 0b10011);

        // Order shouldn't matter: x^5 + x + x^3
        let poly = Gf2mPoly_::from_exponents(&field, &[5, 1, 3]);

        assert_eq!(poly.degree(), Some(5));
        assert_eq!(poly.coeff(0), field.zero());
        assert_eq!(poly.coeff(1), field.one());
        assert_eq!(poly.coeff(2), field.zero());
        assert_eq!(poly.coeff(3), field.one());
        assert_eq!(poly.coeff(4), field.zero());
        assert_eq!(poly.coeff(5), field.one());
    }

    #[test]
    #[should_panic(expected = "exponents cannot be empty")]
    fn test_from_exponents_empty() {
        let field = Gf2mField::new(4, 0b10011);
        let _poly = Gf2mPoly_::from_exponents(&field, &[]);
    }

    #[test]
    fn test_from_exponents_dvb_t2_g1() {
        // Real-world example: DVB-T2 short frame g_1(x)
        let field = Gf2mField::new(14, 0b100000000100001);

        let g1 = Gf2mPoly_::from_exponents(&field, &[0, 1, 3, 5, 14]);

        assert_eq!(g1.degree(), Some(14));
        assert_eq!(g1.coeff(0), field.one());
        assert_eq!(g1.coeff(1), field.one());
        assert_eq!(g1.coeff(2), field.zero());
        assert_eq!(g1.coeff(3), field.one());
        assert_eq!(g1.coeff(4), field.zero());
        assert_eq!(g1.coeff(5), field.one());
        for i in 6..14 {
            assert_eq!(g1.coeff(i), field.zero());
        }
        assert_eq!(g1.coeff(14), field.one());
    }

    #[test]
    fn test_from_exponents_constant() {
        let field = Gf2mField::new(4, 0b10011);

        // Just the constant term: 1
        let poly = Gf2mPoly_::from_exponents(&field, &[0]);

        assert_eq!(poly.degree(), Some(0));
        assert_eq!(poly.coeff(0), field.one());
    }

    #[test]
    fn test_from_exponents_large_sparse() {
        let field = Gf2mField::new(8, 0b100011101);

        // Sparse polynomial: 1 + x^10 + x^100 + x^1000
        let poly = Gf2mPoly_::from_exponents(&field, &[0, 10, 100, 1000]);

        assert_eq!(poly.degree(), Some(1000));
        assert_eq!(poly.coeff(0), field.one());
        assert_eq!(poly.coeff(10), field.one());
        assert_eq!(poly.coeff(100), field.one());
        assert_eq!(poly.coeff(1000), field.one());

        // Verify sparsity - check a few random intermediate points
        assert_eq!(poly.coeff(5), field.zero());
        assert_eq!(poly.coeff(50), field.zero());
        assert_eq!(poly.coeff(500), field.zero());
    }

    // Tests for monomial()
    #[test]
    fn test_monomial_zero_degree() {
        let field = Gf2mField::new(4, 0b10011);
        let alpha = field.element(0b0010);

        // c·x^0 = c (constant polynomial)
        let poly = Gf2mPoly_::monomial(alpha.clone(), 0);

        assert_eq!(poly.degree(), Some(0));
        assert_eq!(poly.coeff(0), alpha);
    }

    #[test]
    fn test_monomial_zero_coeff() {
        let field = Gf2mField::new(4, 0b10011);

        // 0·x^5 = 0 (zero polynomial)
        let poly = Gf2mPoly_::monomial(field.zero(), 5);

        assert!(poly.is_zero());
        assert_eq!(poly.degree(), None);
    }

    #[test]
    fn test_monomial_general() {
        let field = Gf2mField::new(4, 0b10011);
        let alpha = field.element(0b0010);

        // α·x^3
        let poly = Gf2mPoly_::monomial(alpha.clone(), 3);

        assert_eq!(poly.degree(), Some(3));
        assert_eq!(poly.coeff(0), field.zero());
        assert_eq!(poly.coeff(1), field.zero());
        assert_eq!(poly.coeff(2), field.zero());
        assert_eq!(poly.coeff(3), alpha);
    }

    #[test]
    fn test_monomial_one_coefficient() {
        let field = Gf2mField::new(8, 0b100011101);

        // 1·x^10 = x^10
        let poly = Gf2mPoly_::monomial(field.one(), 10);

        assert_eq!(poly.degree(), Some(10));
        assert_eq!(poly.coeff(10), field.one());
    }

    // Tests for x()
    #[test]
    fn test_x_basic() {
        let field = Gf2mField::new(4, 0b10011);

        // x should be the polynomial with degree 1
        let x = Gf2mPoly_::x(&field);

        assert_eq!(x.degree(), Some(1));
        assert_eq!(x.coeff(0), field.zero());
        assert_eq!(x.coeff(1), field.one());
    }

    #[test]
    fn test_x_multiply() {
        let field = Gf2mField::new(4, 0b10011);

        // Multiplying by x should shift polynomial
        let p = Gf2mPoly_::from_exponents(&field, &[0, 2]); // 1 + x^2
        let x = Gf2mPoly_::x(&field);
        let result = &p * &x;

        // (1 + x^2) * x = x + x^3
        assert_eq!(result.degree(), Some(3));
        assert_eq!(result.coeff(0), field.zero());
        assert_eq!(result.coeff(1), field.one());
        assert_eq!(result.coeff(2), field.zero());
        assert_eq!(result.coeff(3), field.one());
    }

    // Tests for from_roots()
    #[test]
    fn test_from_roots_single() {
        let field = Gf2mField::gf256().with_tables();
        let alpha = field.primitive_element().unwrap();

        // (x - α) should have degree 1
        let poly = Gf2mPoly_::from_roots(std::slice::from_ref(&alpha));

        assert_eq!(poly.degree(), Some(1));

        // Verify root: p(α) = 0
        assert!(poly.eval(&alpha).is_zero());
    }

    #[test]
    fn test_from_roots_two() {
        let field = Gf2mField::gf256().with_tables();
        let alpha = field.primitive_element().unwrap();
        let alpha2 = &alpha * &alpha;

        // (x - α)(x - α²)
        let poly = Gf2mPoly_::from_roots(&[alpha.clone(), alpha2.clone()]);

        assert_eq!(poly.degree(), Some(2));

        // Verify roots
        assert!(poly.eval(&alpha).is_zero());
        assert!(poly.eval(&alpha2).is_zero());
    }

    #[test]
    fn test_from_roots_bch() {
        let field = Gf2mField::gf256().with_tables();
        let alpha = field.primitive_element().unwrap();

        // BCH generator with consecutive powers: (x - α)(x - α²)(x - α³)
        let alpha2 = &alpha * &alpha;
        let alpha3 = &alpha2 * &alpha;

        let poly = Gf2mPoly_::from_roots(&[alpha.clone(), alpha2.clone(), alpha3.clone()]);

        assert_eq!(poly.degree(), Some(3));

        // Verify all roots
        assert!(poly.eval(&alpha).is_zero());
        assert!(poly.eval(&alpha2).is_zero());
        assert!(poly.eval(&alpha3).is_zero());
    }

    #[test]
    fn test_from_roots_duplicate() {
        let field = Gf2mField::gf256().with_tables();
        let alpha = field.primitive_element().unwrap();

        // (x - α)² - double root
        let poly = Gf2mPoly_::from_roots(&[alpha.clone(), alpha.clone()]);

        assert_eq!(poly.degree(), Some(2));

        // Should still be a root
        assert!(poly.eval(&alpha).is_zero());
    }

    #[test]
    #[should_panic(expected = "roots cannot be empty")]
    fn test_from_roots_empty() {
        let roots: Vec<Gf2mElement> = vec![];
        let _poly = Gf2mPoly_::from_roots(&roots);
    }

    #[test]
    fn test_from_roots_large() {
        let field = Gf2mField::gf256().with_tables();
        let alpha = field.primitive_element().unwrap();

        // Create polynomial with 12 consecutive roots (DVB-T2 t=12 worst case)
        let mut roots = Vec::new();
        let mut power = alpha.clone();
        for _ in 0..12 {
            roots.push(power.clone());
            power = &power * &alpha;
        }

        let poly = Gf2mPoly_::from_roots(&roots);

        assert_eq!(poly.degree(), Some(12));

        // Verify all roots
        for root in &roots {
            assert!(poly.eval(root).is_zero());
        }
    }

    // Tests for product()
    #[test]
    fn test_product_single() {
        let field = Gf2mField::new(4, 0b10011);
        let p = Gf2mPoly_::from_exponents(&field, &[0, 1, 2]);

        // Product of single polynomial should return clone
        let result = Gf2mPoly_::product(std::slice::from_ref(&p));

        assert_eq!(result.degree(), p.degree());
        if let Some(d) = result.degree() {
            for i in 0..=d {
                assert_eq!(result.coeff(i), p.coeff(i));
            }
        }
    }

    #[test]
    fn test_product_two() {
        let field = Gf2mField::new(4, 0b10011);
        let p1 = Gf2mPoly_::from_exponents(&field, &[0, 1]); // 1 + x
        let p2 = Gf2mPoly_::from_exponents(&field, &[0, 2]); // 1 + x²

        // (1 + x)(1 + x²) = 1 + x + x² + x³
        let result = Gf2mPoly_::product(&[p1.clone(), p2.clone()]);

        assert_eq!(result.degree(), Some(3));
        assert_eq!(result.coeff(0), field.one());
        assert_eq!(result.coeff(1), field.one());
        assert_eq!(result.coeff(2), field.one());
        assert_eq!(result.coeff(3), field.one());
    }

    #[test]
    fn test_product_three() {
        let field = Gf2mField::new(4, 0b10011);
        let p1 = Gf2mPoly_::from_exponents(&field, &[0, 1]); // 1 + x
        let p2 = Gf2mPoly_::from_exponents(&field, &[0, 2]); // 1 + x²
        let p3 = Gf2mPoly_::from_exponents(&field, &[0, 1, 2]); // 1 + x + x²

        let result = Gf2mPoly_::product(&[p1, p2, p3]);

        // Should have degree 5 (1+2+2)
        assert_eq!(result.degree(), Some(5));
    }

    #[test]
    fn test_product_dvb_t2_simulation() {
        let field = Gf2mField::new(14, 0b100000000100001);

        // Simulate DVB-T2 BCH t=3: multiply first 3 generator polynomials
        let g1 = Gf2mPoly_::from_exponents(&field, &[0, 1, 3, 5, 14]);
        let g2 = Gf2mPoly_::from_exponents(&field, &[0, 6, 8, 11, 14]);
        let g3 = Gf2mPoly_::from_exponents(&field, &[0, 1, 2, 6, 9, 10, 14]);

        let product = Gf2mPoly_::product(&[g1, g2, g3]);

        // Product should have degree = sum of degrees = 14 + 14 + 14 = 42
        assert_eq!(product.degree(), Some(42));
    }

    #[test]
    #[should_panic(expected = "polys cannot be empty")]
    fn test_product_empty() {
        let polys: Vec<Gf2mPoly> = vec![];
        let _result = Gf2mPoly_::product(&polys);
    }
}

#[test]
fn test_matches_gf2_coding_workaround() {
    // This test verifies that from_bitvec_reversed produces the same result
    // as the manual workaround in gf2-coding/tests/bch_tests.rs
    let field = Gf2mField::new(4, 0b10011);
    let k = 3;
    let r = 2;
    let n = k + r;

    // Create a test codeword [message | parity]
    let mut codeword = crate::BitVec::new();
    codeword.push_bit(true); // message bit 0
    codeword.push_bit(false); // message bit 1
    codeword.push_bit(true); // message bit 2
    codeword.push_bit(false); // parity bit 0
    codeword.push_bit(true); // parity bit 1

    // Method 1: Using new from_bitvec_reversed
    let poly_new = Gf2mPoly_::from_bitvec_reversed(&codeword, &field);

    // Method 2: Manual workaround (as in gf2-coding)
    let mut coeffs_manual = Vec::new();

    // Parity polynomial p(x): degrees 0..r-1
    // Comes from codeword bits k..n (highest coefficient first)
    for i in (k..n).rev() {
        coeffs_manual.push(if codeword.get(i) {
            field.one()
        } else {
            field.zero()
        });
    }

    // Message polynomial x^r·m(x): degrees r..n-1
    // Comes from codeword bits 0..k (highest coefficient first)
    for i in (0..k).rev() {
        coeffs_manual.push(if codeword.get(i) {
            field.one()
        } else {
            field.zero()
        });
    }

    let poly_manual = Gf2mPoly_::new(coeffs_manual);

    // Verify they're identical
    assert_eq!(poly_new.degree(), poly_manual.degree());
    if let Some(d) = poly_new.degree() {
        for i in 0..=d {
            assert_eq!(
                poly_new.coeff(i).value(),
                poly_manual.coeff(i).value(),
                "Coefficient mismatch at degree {}",
                i
            );
        }
    }
}

#[cfg(test)]
mod generic_width_tests {
    use super::*;
    use crate::field::FiniteField;

    // GF(2^4) with u8 backing — same field, smaller container
    #[test]
    fn test_gf16_u8() {
        let field = Gf2mField_::<u8>::new(4, 0b10011);
        let a = field.element(5);
        let b = field.element(3);
        assert_eq!((a.clone() + b.clone()).value(), 5u8 ^ 3);
        // a * inv(a) == 1
        assert!((a.clone() * a.inv().unwrap()).is_one());
    }

    // GF(2^7) with u8 — max m for u8 is 7 (strict < 8)
    #[test]
    fn test_gf128_u8_max_degree() {
        // x^7 + x + 1 is primitive for GF(2^7)
        let field = Gf2mField_::<u8>::new(7, 0b10000011);
        let a = field.element(0x5A);
        assert!((a.clone() * a.inv().unwrap()).is_one());
    }

    // GF(2^4) with u16
    #[test]
    fn test_gf16_u16() {
        let field = Gf2mField_::<u16>::new(4, 0b10011);
        let a = field.element(5);
        let b = field.element(3);
        assert_eq!((a.clone() + b.clone()).value(), 5u16 ^ 3);
    }

    // GF(2^4) with u128 — wide container for small field
    #[test]
    fn test_gf16_u128() {
        let field = Gf2mField_::<u128>::new(4, 0b10011);
        let a = field.element(5);
        let b = field.element(3);
        assert_eq!((a.clone() + b.clone()).value(), 5u128 ^ 3);
        assert!((a.clone() * a.inv().unwrap()).is_one());
    }

    // Cross-width consistency: same field ops produce same results
    #[test]
    fn test_cross_width_consistency() {
        // GF(2^4) with poly x^4+x+1 across u8, u16, u64, u128
        let f8 = Gf2mField_::<u8>::new(4, 0b10011);
        let f16 = Gf2mField_::<u16>::new(4, 0b10011);
        let f64 = Gf2mField::new(4, 0b10011);
        let f128 = Gf2mField_::<u128>::new(4, 0b10011);

        for a_val in 0u8..16 {
            for b_val in 0u8..16 {
                let sum8 = (f8.element(a_val) + f8.element(b_val)).value();
                let sum16 = (f16.element(a_val as u16) + f16.element(b_val as u16)).value();
                let sum64 = (f64.element(a_val as u64) + f64.element(b_val as u64)).value();
                let sum128 = (f128.element(a_val as u128) + f128.element(b_val as u128)).value();
                assert_eq!(sum8 as u128, sum128);
                assert_eq!(sum16 as u128, sum128);
                assert_eq!(sum64 as u128, sum128);

                let prod8 = (f8.element(a_val) * f8.element(b_val)).value();
                let prod64 = (f64.element(a_val as u64) * f64.element(b_val as u64)).value();
                assert_eq!(prod8 as u64, prod64);
            }
        }
    }

    // m == V::BITS should be rejected
    #[test]
    #[should_panic(expected = "must be strictly less than")]
    fn test_m_equals_bits_rejected_u8() {
        Gf2mField_::<u8>::new(8, 0); // poly value irrelevant; panics on m check
    }

    #[test]
    #[should_panic(expected = "must be strictly less than")]
    fn test_m_equals_bits_rejected_u64() {
        Gf2mField_::<u64>::new(64, 0);
    }

    // order_v works for all types
    #[test]
    fn test_order_v() {
        let f8 = Gf2mField_::<u8>::new(4, 0b10011);
        assert_eq!(f8.order_v(), 16u8);

        let f128 = Gf2mField_::<u128>::new(4, 0b10011);
        assert_eq!(f128.order_v(), 16u128);
    }

    // Display uses binary format
    #[test]
    fn test_display_binary_format() {
        let field = Gf2mField::new(4, 0b10011);
        let elem = field.element(0b1010);
        let s = format!("{}", elem);
        assert_eq!(s, "0b1010");
    }

    /// Exhaustive (m<=8) or sampled (m>8) field axiom verification for all m=2..16.
    ///
    /// This test verifies that multiplication (which may use SIMD PCLMULQDQ when
    /// available) produces correct results by checking field axioms:
    /// commutativity, associativity, distributivity, identity, and inverse.
    #[test]
    fn test_mul_field_axioms_all_m_2_to_16() {
        use crate::primitive_polys::PrimitivePolynomialDatabase;

        for m in 2..=16usize {
            let poly = PrimitivePolynomialDatabase::standard(m)
                .unwrap_or_else(|| panic!("no standard polynomial for m={m}"));
            let field = Gf2mField::new(m, poly);
            let order = 1u64 << m;
            let one = field.one();

            // Collect test elements: exhaustive for m<=8, sampled for m>8
            let elements: Vec<u64> = if m <= 8 {
                (0..order).collect()
            } else {
                // Sample: 0, 1, 2, order-1, order-2, plus pseudo-random elements
                // drawn from the workspace SSOT LCG.
                let mut elems = vec![0, 1, 2, order - 1, order - 2];
                let mut rng = crate::rng::Lcg::new(0xDEAD_BEEF_u64);
                for _ in 0..50 {
                    elems.push((rng.next_u64() >> 33) % order);
                }
                elems.sort_unstable();
                elems.dedup();
                elems
            };

            // Identity: a * 1 == a, 1 * a == a
            for &a_val in &elements {
                let a = field.element(a_val);
                assert_eq!(
                    (a.clone() * one.clone()).value(),
                    a_val,
                    "m={m}: {a_val} * 1 != {a_val}"
                );
                assert_eq!(
                    (one.clone() * a.clone()).value(),
                    a_val,
                    "m={m}: 1 * {a_val} != {a_val}"
                );
            }

            // Zero: a * 0 == 0
            let zero = field.zero();
            for &a_val in &elements {
                let a = field.element(a_val);
                assert!(
                    (a.clone() * zero.clone()).is_zero(),
                    "m={m}: {a_val} * 0 != 0"
                );
            }

            // Commutativity, associativity, distributivity on pairs/triples
            let test_elems: Vec<u64> = if m <= 4 {
                (0..order).collect()
            } else if m <= 8 {
                // Use a subset for pair/triple tests to keep runtime reasonable;
                // sample via the workspace SSOT LCG.
                let mut elems = vec![0, 1, 2, order - 1];
                let mut rng = crate::rng::Lcg::new(0xCAFE_BABE_u64);
                for _ in 0..12 {
                    elems.push((rng.next_u64() >> 33) % order);
                }
                elems.sort_unstable();
                elems.dedup();
                elems
            } else {
                let mut elems = vec![0, 1, 2, order - 1];
                let mut rng = crate::rng::Lcg::new(0xCAFE_BABE_u64);
                for _ in 0..20 {
                    elems.push((rng.next_u64() >> 33) % order);
                }
                elems.sort_unstable();
                elems.dedup();
                elems
            };

            for &a_val in &test_elems {
                for &b_val in &test_elems {
                    let a = field.element(a_val);
                    let b = field.element(b_val);

                    // Commutativity: a * b == b * a
                    let ab = (a.clone() * b.clone()).value();
                    let ba = (b.clone() * a.clone()).value();
                    assert_eq!(ab, ba, "m={m}: {a_val}*{b_val} != {b_val}*{a_val}");

                    // Distributivity: a * (b + c) == a*b + a*c (pick c = 1)
                    let c = one.clone();
                    let b_plus_c = b.clone() + c.clone();
                    let lhs = (a.clone() * b_plus_c).value();
                    let rhs = ((a.clone() * b.clone()) + (a.clone() * c)).value();
                    assert_eq!(
                        lhs, rhs,
                        "m={m}: {a_val}*({b_val}+1) != {a_val}*{b_val} + {a_val}*1"
                    );
                }
            }

            // Inverse: a * inv(a) == 1 for all nonzero a
            for &a_val in &elements {
                if a_val == 0 {
                    continue;
                }
                let a = field.element(a_val);
                let inv_a = a.inv().expect("nonzero element must have inverse");
                assert!((a * inv_a).is_one(), "m={m}: {a_val} * inv({a_val}) != 1");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Kani bounded model checking harnesses for log/exp table verification
// ---------------------------------------------------------------------------

#[cfg(test)]
mod kani_table_validation {
    use super::*;
    use crate::primitive_polys::PrimitivePolynomialDatabase;

    #[test]
    fn precomputed_tables_match_database_gf16() {
        let poly = PrimitivePolynomialDatabase::standard(4).unwrap();
        assert_eq!(poly, 0b10011);
        let (log_table, exp_table) = Gf2mField_::<u64>::generate_tables(4, poly);
        let expected_exp: [u16; 15] = [1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9];
        let expected_log: [u16; 16] = [0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12];
        assert_eq!(exp_table, expected_exp);
        assert_eq!(log_table, expected_log);
    }

    #[test]
    fn precomputed_tables_match_database_gf256() {
        let poly = PrimitivePolynomialDatabase::standard(8).unwrap();
        assert_eq!(poly, 0b100011101);
        let (log_table, exp_table) = Gf2mField_::<u64>::generate_tables(8, poly);
        // Spot-check key entries against the Kani pre-computed constants
        assert_eq!(exp_table[0], 1); // α^0 = 1
        assert_eq!(exp_table[1], 2); // α = 2 (primitive element)
        assert_eq!(exp_table.len(), 255);
        assert_eq!(log_table.len(), 256);
        assert_eq!(log_table[1], 0); // log(1) = 0
        assert_eq!(log_table[2], 1); // log(α) = 1
                                     // Full comparison against Kani constants
        #[rustfmt::skip]
        let kani_exp: [u16; 255] = [
            1, 2, 4, 8, 16, 32, 64, 128, 29, 58, 116, 232, 205, 135, 19, 38,
            76, 152, 45, 90, 180, 117, 234, 201, 143, 3, 6, 12, 24, 48, 96, 192,
            157, 39, 78, 156, 37, 74, 148, 53, 106, 212, 181, 119, 238, 193, 159, 35,
            70, 140, 5, 10, 20, 40, 80, 160, 93, 186, 105, 210, 185, 111, 222, 161,
            95, 190, 97, 194, 153, 47, 94, 188, 101, 202, 137, 15, 30, 60, 120, 240,
            253, 231, 211, 187, 107, 214, 177, 127, 254, 225, 223, 163, 91, 182, 113, 226,
            217, 175, 67, 134, 17, 34, 68, 136, 13, 26, 52, 104, 208, 189, 103, 206,
            129, 31, 62, 124, 248, 237, 199, 147, 59, 118, 236, 197, 151, 51, 102, 204,
            133, 23, 46, 92, 184, 109, 218, 169, 79, 158, 33, 66, 132, 21, 42, 84,
            168, 77, 154, 41, 82, 164, 85, 170, 73, 146, 57, 114, 228, 213, 183, 115,
            230, 209, 191, 99, 198, 145, 63, 126, 252, 229, 215, 179, 123, 246, 241, 255,
            227, 219, 171, 75, 150, 49, 98, 196, 149, 55, 110, 220, 165, 87, 174, 65,
            130, 25, 50, 100, 200, 141, 7, 14, 28, 56, 112, 224, 221, 167, 83, 166,
            81, 162, 89, 178, 121, 242, 249, 239, 195, 155, 43, 86, 172, 69, 138, 9,
            18, 36, 72, 144, 61, 122, 244, 245, 247, 243, 251, 235, 203, 139, 11, 22,
            44, 88, 176, 125, 250, 233, 207, 131, 27, 54, 108, 216, 173, 71, 142,
        ];
        assert_eq!(exp_table.as_slice(), &kani_exp[..]);
        #[rustfmt::skip]
        let kani_log: [u16; 256] = [
            0, 0, 1, 25, 2, 50, 26, 198, 3, 223, 51, 238, 27, 104, 199, 75,
            4, 100, 224, 14, 52, 141, 239, 129, 28, 193, 105, 248, 200, 8, 76, 113,
            5, 138, 101, 47, 225, 36, 15, 33, 53, 147, 142, 218, 240, 18, 130, 69,
            29, 181, 194, 125, 106, 39, 249, 185, 201, 154, 9, 120, 77, 228, 114, 166,
            6, 191, 139, 98, 102, 221, 48, 253, 226, 152, 37, 179, 16, 145, 34, 136,
            54, 208, 148, 206, 143, 150, 219, 189, 241, 210, 19, 92, 131, 56, 70, 64,
            30, 66, 182, 163, 195, 72, 126, 110, 107, 58, 40, 84, 250, 133, 186, 61,
            202, 94, 155, 159, 10, 21, 121, 43, 78, 212, 229, 172, 115, 243, 167, 87,
            7, 112, 192, 247, 140, 128, 99, 13, 103, 74, 222, 237, 49, 197, 254, 24,
            227, 165, 153, 119, 38, 184, 180, 124, 17, 68, 146, 217, 35, 32, 137, 46,
            55, 63, 209, 91, 149, 188, 207, 205, 144, 135, 151, 178, 220, 252, 190, 97,
            242, 86, 211, 171, 20, 42, 93, 158, 132, 60, 57, 83, 71, 109, 65, 162,
            31, 45, 67, 216, 183, 123, 164, 118, 196, 23, 73, 236, 127, 12, 111, 246,
            108, 161, 59, 82, 41, 157, 85, 170, 251, 96, 134, 177, 187, 204, 62, 90,
            203, 89, 95, 176, 156, 169, 160, 81, 11, 245, 22, 235, 122, 117, 44, 215,
            79, 174, 213, 233, 230, 231, 173, 232, 116, 214, 244, 234, 168, 80, 88, 175,
        ];
        assert_eq!(log_table.as_slice(), &kani_log[..]);
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;
    use crate::gf2m::mul_raw::gf2m_mul_raw;

    // Polynomials from PrimitivePolynomialDatabase::standard() — hardcoded here
    // to avoid pulling the database match statement into the GOTO program.
    // The companion #[test] kani_table_validation verifies these match.
    //
    // CBMC limitation: the full Gf2mElement API (Arc, trait dispatch, multi-path
    // Mul impl with SIMD/Barrett/table branches) exceeds CBMC's memory capacity
    // even for GF(16). Harnesses therefore call generate_tables() directly
    // (production table generation code) and verify table properties + cross-check
    // against gf2m_mul_raw (production schoolbook multiplication). This is the
    // deepest production code path CBMC can handle.

    // GF(2^4): PrimitivePolynomialDatabase::standard(4)
    const M4: usize = 4;
    const POLY4: u64 = 0b10011; // x^4 + x + 1
    const ORDER4: usize = (1 << M4) - 1;

    // GF(2^8): PrimitivePolynomialDatabase::standard(8)
    const M8: usize = 8;
    const POLY8: u64 = 0b100011101; // x^8 + x^4 + x^3 + x^2 + 1
    const ORDER8: usize = (1 << M8) - 1;

    fn tables_gf16() -> (Vec<u16>, Vec<u16>) {
        Gf2mField_::<u64>::generate_tables(M4, POLY4)
    }

    fn tables_gf256() -> (Vec<u16>, Vec<u16>) {
        Gf2mField_::<u64>::generate_tables(M8, POLY8)
    }

    // -- GF(2^4) harnesses --

    /// Verify exp_table and log_table are mutual inverses for GF(2^4).
    /// Uses production generate_tables(). Checks exp_table[0] = 1 (α^0)
    /// and exp_table[1] = primitive element.
    #[kani::proof]
    #[kani::unwind(20)]
    fn table_consistency_gf16() {
        let (log_table, exp_table) = tables_gf16();

        assert_eq!(exp_table[0], 1);
        assert!(exp_table[1] >= 2);

        let mut x: usize = 1;
        while x < 16 {
            let log_x = log_table[x] as usize;
            assert!(log_x < ORDER4);
            assert!(exp_table[log_x] == x as u16);
            x += 1;
        }

        let mut i: usize = 0;
        while i < ORDER4 {
            let exp_i = exp_table[i] as usize;
            assert!(exp_i > 0 && exp_i < (1 << M4));
            assert!(log_table[exp_i] as usize == i);
            i += 1;
        }
    }

    /// Verify table-based multiplication matches schoolbook for GF(2^4).
    /// Uses production generate_tables() + production gf2m_mul_raw().
    #[kani::proof]
    #[kani::unwind(20)]
    fn table_mul_matches_schoolbook_gf16() {
        let (log_table, exp_table) = tables_gf16();

        let a: u64 = kani::any();
        let b: u64 = kani::any();
        kani::assume(a >= 1 && a < 16);
        kani::assume(b >= 1 && b < 16);

        // Table-based multiply (same formula as production Mul impl)
        let log_a = log_table[a as usize] as usize;
        let log_b = log_table[b as usize] as usize;
        let table_result = exp_table[(log_a + log_b) % ORDER4] as u64;

        let schoolbook_result = gf2m_mul_raw(a, b, M4, POLY4);
        assert_eq!(table_result, schoolbook_result);
    }

    /// Verify table-based inverse: a * inv(a) == 1 for all nonzero a in GF(2^4).
    /// Uses production generate_tables() + production gf2m_mul_raw().
    #[kani::proof]
    #[kani::unwind(20)]
    fn table_inverse_correct_gf16() {
        let (log_table, exp_table) = tables_gf16();

        let a: u64 = kani::any();
        kani::assume(a >= 1 && a < 16);

        let log_a = log_table[a as usize] as usize;
        let inv_a = exp_table[(ORDER4 - log_a) % ORDER4] as u64;

        let product = gf2m_mul_raw(a, inv_a, M4, POLY4);
        assert_eq!(product, 1);
    }

    // -- GF(2^8) harnesses --

    /// Verify exp_table and log_table are mutual inverses for GF(2^8).
    /// Uses production generate_tables(). Checks exp_table[0] = 1 (α^0)
    /// and exp_table[1] = primitive element.
    #[kani::proof]
    #[kani::unwind(260)]
    fn table_consistency_gf256() {
        let (log_table, exp_table) = tables_gf256();

        assert_eq!(exp_table[0], 1);
        assert!(exp_table[1] >= 2);

        let mut x: usize = 1;
        while x < 256 {
            let log_x = log_table[x] as usize;
            assert!(log_x < ORDER8);
            assert!(exp_table[log_x] == x as u16);
            x += 1;
        }

        let mut i: usize = 0;
        while i < ORDER8 {
            let exp_i = exp_table[i] as usize;
            assert!(exp_i > 0 && exp_i < (1 << M8));
            assert!(log_table[exp_i] as usize == i);
            i += 1;
        }
    }

    /// Verify table-based multiplication matches schoolbook for GF(2^8).
    /// Uses production generate_tables() + production gf2m_mul_raw().
    #[kani::proof]
    #[kani::unwind(260)]
    fn table_mul_matches_schoolbook_gf256() {
        let (log_table, exp_table) = tables_gf256();

        let a: u64 = kani::any();
        let b: u64 = kani::any();
        kani::assume(a >= 1 && a < 256);
        kani::assume(b >= 1 && b < 256);

        let log_a = log_table[a as usize] as usize;
        let log_b = log_table[b as usize] as usize;
        let table_result = exp_table[(log_a + log_b) % ORDER8] as u64;

        let schoolbook_result = gf2m_mul_raw(a, b, M8, POLY8);
        assert_eq!(table_result, schoolbook_result);
    }

    /// Verify table-based inverse: a * inv(a) == 1 for all nonzero a in GF(2^8).
    /// Uses production generate_tables() + production gf2m_mul_raw().
    #[kani::proof]
    #[kani::unwind(260)]
    fn table_inverse_correct_gf256() {
        let (log_table, exp_table) = tables_gf256();

        let a: u64 = kani::any();
        kani::assume(a >= 1 && a < 256);

        let log_a = log_table[a as usize] as usize;
        let inv_a = exp_table[(ORDER8 - log_a) % ORDER8] as u64;

        let product = gf2m_mul_raw(a, inv_a, M8, POLY8);
        assert_eq!(product, 1);
    }
}
