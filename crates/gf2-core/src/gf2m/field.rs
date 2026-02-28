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
//! **Multiplication**: Multiply polynomials, then reduce modulo a primitive polynomial
//! ```text
//! In GF(2^4) with primitive polynomial p(x) = x⁴ + x + 1:
//! (x + 1) · (x² + 1) = x³ + x² + x + 1
//! ```
//!
//! **Primitive Polynomial**: An irreducible polynomial of degree m that generates
//! the full multiplicative group. Required to define field structure.
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

use super::uint_ext::UintExt;

/// A binary extension field GF(2^m) with a specified primitive polynomial.
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
    // SIMD multiplication function (if available)
    #[cfg(feature = "simd")]
    simd_mul_fn: Option<simd_gf2m::Gf2mMulFn>,
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
    /// Creates a new GF(2^m) field with the specified primitive polynomial.
    ///
    /// # Arguments
    ///
    /// * `m` - Extension degree (field has 2^m elements, must fit in `V`)
    /// * `primitive_poly` - Primitive polynomial of degree m in binary representation
    ///
    /// # Panics
    ///
    /// Panics if `m > V::BITS` or `m == 0`.
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
        let simd_mul_fn = if V::IS_U64 {
            simd_gf2m::detect().map(|fns| fns.mul_fn)
        } else {
            None
        };

        Gf2mField_ {
            params: Arc::new(FieldParams_ {
                m,
                primitive_poly,
                log_table: None,
                exp_table: None,
                #[cfg(feature = "simd")]
                simd_mul_fn,
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

    /// Returns the primitive polynomial.
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

        Gf2mField_ {
            params: Arc::new(FieldParams_ {
                m: self.params.m,
                primitive_poly: self.params.primitive_poly,
                log_table: Some(log_table),
                exp_table: Some(exp_table),
                #[cfg(feature = "simd")]
                simd_mul_fn,
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
            if n % p == 0 {
                factors.push(p);
                while n % p == 0 {
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

        // Priority 2: Use SIMD if available (faster than schoolbook for larger m)
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

        // Priority 3: Fallback to schoolbook multiplication
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
}

// ============================================================================
// Polynomial Operations over GF(2^m)
// ============================================================================

/// A polynomial with coefficients in GF(2^m).
///
/// Coefficients are stored in ascending order: `coeffs[i]` is the coefficient of x^i.
/// The polynomial is automatically normalized to remove leading zero coefficients.
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
/// A polynomial with coefficients in GF(2^m).
///
/// Coefficients are stored in ascending order: `coeffs[i]` is the coefficient of x^i.
/// The polynomial is automatically normalized to remove leading zero coefficients.
#[derive(Clone, Debug)]
pub struct Gf2mPoly_<V: UintExt = u64> {
    coeffs: Vec<Gf2mElement_<V>>,
}

/// Convenience alias: `Gf2mPoly` is `Gf2mPoly_<u64>`.
pub type Gf2mPoly = Gf2mPoly_<u64>;

impl<V: UintExt> Gf2mPoly_<V> {
    /// Creates a new polynomial from coefficients.
    ///
    /// Coefficients are in ascending order: `coeffs[i]` is the coefficient of x^i.
    /// Leading zero coefficients are automatically removed.
    pub fn new(coeffs: Vec<Gf2mElement_<V>>) -> Self {
        let mut poly = Gf2mPoly_ { coeffs };
        poly.normalize();
        poly
    }

    /// Creates the zero polynomial.
    pub fn zero(field: &Gf2mField_<V>) -> Self {
        Gf2mPoly_ {
            coeffs: vec![field.zero()],
        }
    }

    /// Creates a constant polynomial.
    pub fn constant(value: Gf2mElement_<V>) -> Self {
        Gf2mPoly_ {
            coeffs: vec![value],
        }
    }

    /// Returns the degree of the polynomial, or None if it's the zero polynomial.
    pub fn degree(&self) -> Option<usize> {
        if self.is_zero() {
            None
        } else {
            Some(self.coeffs.len() - 1)
        }
    }

    /// Returns true if this is the zero polynomial.
    pub fn is_zero(&self) -> bool {
        self.coeffs.len() == 1 && self.coeffs[0].is_zero()
    }

    /// Returns the coefficient of x^i.
    pub fn coeff(&self, i: usize) -> Gf2mElement_<V> {
        if i < self.coeffs.len() {
            self.coeffs[i].clone()
        } else {
            // Return zero for coefficients beyond degree
            Gf2mElement_ {
                value: V::ZERO,
                params: self.coeffs[0].params.clone(),
            }
        }
    }

    /// Removes leading zero coefficients.
    fn normalize(&mut self) {
        while self.coeffs.len() > 1 && self.coeffs.last().unwrap().is_zero() {
            self.coeffs.pop();
        }
    }

    /// Constructs a polynomial from a BitVec over GF(2^m).
    ///
    /// Each bit in the BitVec is interpreted as a coefficient in GF(2^m):
    /// - `false` (0) → field.zero()
    /// - `true` (1) → field.one()
    ///
    /// The polynomial is in ascending degree order: bit i is the coefficient of x^i.
    ///
    /// # Arguments
    ///
    /// * `bits` - BitVec containing binary coefficients
    /// * `field` - The field to use for creating elements
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut bits = BitVec::new();
    /// bits.push_bit(true);  // x^0 term
    /// bits.push_bit(false); // x^1 term
    /// bits.push_bit(true);  // x^2 term
    ///
    /// let poly = Gf2mPoly::from_bitvec(&bits, &field);
    /// assert_eq!(poly.degree(), Some(2));
    /// assert!(poly.coeff(0).is_one());
    /// assert!(poly.coeff(1).is_zero());
    /// assert!(poly.coeff(2).is_one());
    /// ```
    pub fn from_bitvec(bits: &crate::BitVec, field: &Gf2mField_<V>) -> Self {
        if bits.is_empty() {
            return Self::zero(field);
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

        Self::new(coeffs)
    }

    /// Converts polynomial to BitVec, extracting binary coefficients.
    ///
    /// Only extracts the binary value of coefficients (0 or 1). Non-binary
    /// coefficients in the field are treated as 1 (non-zero).
    ///
    /// # Arguments
    ///
    /// * `len` - Desired length of output BitVec (may exceed polynomial degree)
    ///
    /// # Returns
    ///
    /// BitVec where bit i = true iff coefficient of x^i is non-zero
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = Gf2mPoly::new(vec![
    ///     field.one(),   // x^0
    ///     field.zero(),  // x^1
    ///     field.one(),   // x^2
    /// ]);
    ///
    /// let bits = poly.to_bitvec(5);
    /// assert_eq!(bits.len(), 5);
    /// assert!(bits.get(0));   // x^0 term present
    /// assert!(!bits.get(1));  // x^1 term absent
    /// assert!(bits.get(2));   // x^2 term present
    /// assert!(!bits.get(3));  // x^3 term absent (beyond degree)
    /// assert!(!bits.get(4));  // x^4 term absent
    /// ```
    ///
    /// # Notes
    ///
    /// Coefficients beyond the polynomial degree are treated as zero.
    /// This is useful for BCH and other coding applications where
    /// codewords have fixed length.
    pub fn to_bitvec(&self, len: usize) -> crate::BitVec {
        let mut bits = crate::BitVec::new();

        for i in 0..len {
            let coeff = self.coeff(i);
            bits.push_bit(!coeff.is_zero());
        }

        bits
    }

    /// Converts polynomial to BitVec with minimal length (degree + 1).
    ///
    /// Convenience method equivalent to `to_bitvec(degree + 1)`.
    /// For the zero polynomial, returns an empty BitVec.
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
    /// assert_eq!(bits.len(), 3); // degree 2, so length 3
    /// ```
    pub fn to_bitvec_minimal(&self) -> crate::BitVec {
        let len = self.degree().map(|d| d + 1).unwrap_or(0);
        self.to_bitvec(len)
    }

    /// Constructs a polynomial from a BitVec with reversed coefficient mapping.
    ///
    /// Maps bit i → coefficient of x^(n-1-i), where bit 0 is the highest degree.
    /// Inverse of [`from_bitvec`](Self::from_bitvec).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut bits = BitVec::new();
    /// bits.push_bit(true);  // bit 0 -> x^2
    /// bits.push_bit(false); // bit 1 -> x^1
    /// bits.push_bit(true);  // bit 2 -> x^0
    ///
    /// let poly = Gf2mPoly::from_bitvec_reversed(&bits, &field);
    /// assert_eq!(poly.degree(), Some(2));
    /// ```
    pub fn from_bitvec_reversed(bits: &crate::BitVec, field: &Gf2mField_<V>) -> Self {
        if bits.is_empty() {
            return Self::zero(field);
        }

        let n = bits.len();
        let coeffs: Vec<Gf2mElement_<V>> = (0..n)
            .map(|i| {
                // bit i maps to coefficient of x^(n-1-i)
                // so coefficient of x^j comes from bit (n-1-j)
                let bit_index = n - 1 - i;
                if bits.get(bit_index) {
                    field.one()
                } else {
                    field.zero()
                }
            })
            .collect();

        Self::new(coeffs)
    }

    /// Converts polynomial to BitVec with reversed coefficient mapping.
    ///
    /// Maps coefficient of x^i → bit (len-1-i), where bit 0 is the highest degree.
    /// Inverse of [`to_bitvec`](Self::to_bitvec).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = Gf2mPoly::new(vec![
    ///     field.one(),   // x^0
    ///     field.zero(),  // x^1
    ///     field.one(),   // x^2
    /// ]);
    ///
    /// let bits = poly.to_bitvec_reversed(5);
    /// assert_eq!(bits.len(), 5);
    /// assert!(bits.get(2));  // x^2 at bit 2
    /// assert!(bits.get(4));  // x^0 at bit 4
    /// ```
    pub fn to_bitvec_reversed(&self, len: usize) -> crate::BitVec {
        let mut bits = crate::BitVec::new();

        for i in 0..len {
            // bit i should contain coefficient of x^(len-1-i)
            let degree = len - 1 - i;
            let coeff = self.coeff(degree);
            bits.push_bit(!coeff.is_zero());
        }

        bits
    }

    /// Creates a polynomial from a list of exponents.
    ///
    /// Each exponent in the list corresponds to a term with coefficient 1.
    /// For example, `[0, 2, 5]` represents `1 + x² + x⁵`.
    ///
    /// This is particularly useful for constructing generator polynomials
    /// from standard tables (e.g., BCH, Goppa codes) where polynomials are
    /// often specified as lists of exponents.
    ///
    /// # Arguments
    ///
    /// * `field` - The field over which the polynomial is defined
    /// * `exponents` - Slice of exponents where coefficients are 1
    ///
    /// # Duplicate Exponents
    ///
    /// Duplicate exponents are handled correctly via GF(2) addition:
    /// - Even occurrences cancel out: `x² + x² = 0`
    /// - Odd occurrences remain: `x² + x² + x² = x²`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    ///
    /// // Create polynomial: 1 + x + x^4
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
    /// # Real-World Example: DVB-T2 BCH Generator
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// // DVB-T2 short frame uses GF(2^14)
    /// let field = Gf2mField::new(14, 0b100000000100001);
    ///
    /// // g_1(x) from ETSI EN 302 755
    /// let g1 = Gf2mPoly::from_exponents(&field, &[0, 1, 3, 5, 14]);
    /// assert_eq!(g1.degree(), Some(14));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(max_exp) where max_exp is the largest exponent in the list.
    ///
    /// # Panics
    ///
    /// Panics if `exponents` is empty.
    pub fn from_exponents(field: &Gf2mField_<V>, exponents: &[usize]) -> Self {
        assert!(!exponents.is_empty(), "exponents cannot be empty");

        let max_exp = exponents.iter().copied().max().unwrap();
        let mut coeffs = vec![field.zero(); max_exp + 1];

        // Add 1 to each specified exponent
        // In GF(2), repeated additions cancel: a + a = 0
        for &exp in exponents {
            coeffs[exp] = &coeffs[exp] + &field.one();
        }

        Self::new(coeffs)
    }

    /// Creates a monomial: `c·xⁿ`.
    ///
    /// A monomial is a polynomial with a single term.
    ///
    /// # Arguments
    ///
    /// * `coeff` - The coefficient (may be any field element)
    /// * `degree` - The exponent of x
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let alpha = field.element(0b0010); // α
    ///
    /// // Create α·x³
    /// let poly = Gf2mPoly::monomial(alpha.clone(), 3);
    ///
    /// assert_eq!(poly.degree(), Some(3));
    /// assert_eq!(poly.coeff(0), field.zero());
    /// assert_eq!(poly.coeff(3), alpha);
    /// ```
    ///
    /// # Special Cases
    ///
    /// - `monomial(c, 0)` returns constant polynomial `c`
    /// - `monomial(0, n)` returns zero polynomial regardless of n
    ///
    /// # Complexity
    ///
    /// O(degree) for coefficient vector allocation.
    pub fn monomial(coeff: Gf2mElement_<V>, degree: usize) -> Self {
        if coeff.is_zero() {
            return Self::zero(&Gf2mField_ {
                params: coeff.params.clone(),
            });
        }

        let field = Gf2mField_ {
            params: coeff.params.clone(),
        };
        let mut coeffs = vec![field.zero(); degree + 1];
        coeffs[degree] = coeff;

        Self::new(coeffs)
    }

    /// Creates the polynomial `x` (the indeterminate).
    ///
    /// This is equivalent to `monomial(field.one(), 1)` and is provided
    /// as a convenience for building polynomials programmatically.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let x = Gf2mPoly::x(&field);
    ///
    /// assert_eq!(x.degree(), Some(1));
    /// assert_eq!(x.coeff(0), field.zero());
    /// assert_eq!(x.coeff(1), field.one());
    ///
    /// // Use x to build polynomials
    /// let p = Gf2mPoly::from_exponents(&field, &[0, 2]); // 1 + x²
    /// let result = &p * &x; // (1 + x²) * x = x + x³
    /// ```
    pub fn x(field: &Gf2mField_<V>) -> Self {
        Self::monomial(field.one(), 1)
    }

    /// Creates a polynomial from its roots.
    ///
    /// Constructs the polynomial `(x - r₁)(x - r₂)...(x - rₙ)` where
    /// `rᵢ` are the roots.
    ///
    /// This is fundamental for BCH and Reed-Solomon code construction,
    /// where generator polynomials are defined by consecutive roots
    /// (powers of a primitive element).
    ///
    /// # Arguments
    ///
    /// * `roots` - Slice of field elements that are roots of the polynomial
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::gf256().with_tables();
    /// let alpha = field.primitive_element().unwrap();
    ///
    /// // BCH generator: g(x) = (x - α)(x - α²)
    /// let alpha2 = &alpha * &alpha;
    /// let g = Gf2mPoly::from_roots(&[alpha.clone(), alpha2.clone()]);
    ///
    /// // Verify roots
    /// assert!(g.eval(&alpha).is_zero());
    /// assert!(g.eval(&alpha2).is_zero());
    /// ```
    ///
    /// # Real-World Example: DVB-T2 BCH Code
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::gf256().with_tables();
    /// let alpha = field.primitive_element().unwrap();
    ///
    /// // t=3 BCH code: consecutive roots α, α², α³, α⁴, α⁵, α⁶
    /// let mut roots = Vec::new();
    /// let mut power = alpha.clone();
    /// for _ in 0..6 {
    ///     roots.push(power.clone());
    ///     power = &power * &alpha;
    /// }
    ///
    /// let generator = Gf2mPoly::from_roots(&roots);
    /// assert_eq!(generator.degree(), Some(6));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n²) where n is the number of roots (sequential multiplication).
    /// Uses existing optimized polynomial multiplication which switches
    /// to Karatsuba for large degrees.
    ///
    /// # Panics
    ///
    /// Panics if `roots` is empty.
    pub fn from_roots(roots: &[Gf2mElement_<V>]) -> Self {
        assert!(!roots.is_empty(), "roots cannot be empty");

        // Get field from first root
        let field = Gf2mField_ {
            params: roots[0].params.clone(),
        };

        // Start with (x - r₀)
        // Note: In GF(2^m), -r = r, so x - r = x + r
        let mut result = Self::new(vec![
            roots[0].clone(), // constant term
            field.one(),      // x coefficient
        ]);

        // Multiply by (x - rᵢ) for remaining roots
        for root in &roots[1..] {
            let factor = Self::new(vec![root.clone(), field.one()]);
            result = &result * &factor;
        }

        result
    }

    /// Computes the product of multiple polynomials.
    ///
    /// Returns `p₁(x) · p₂(x) · ... · pₙ(x)`.
    ///
    /// # Arguments
    ///
    /// * `polys` - Slice of polynomials to multiply
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let p1 = Gf2mPoly::from_exponents(&field, &[0, 1]);    // 1 + x
    /// let p2 = Gf2mPoly::from_exponents(&field, &[0, 2]);    // 1 + x²
    /// let p3 = Gf2mPoly::from_exponents(&field, &[0, 1, 2]); // 1 + x + x²
    ///
    /// let product = Gf2mPoly::product(&[p1, p2, p3]);
    /// // (1 + x)(1 + x²)(1 + x + x²)
    /// ```
    ///
    /// # Real-World Example: DVB-T2 BCH Generator
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(14, 0b100000000100001);
    ///
    /// // DVB-T2 t=3: g(x) = g_1(x) · g_2(x) · g_3(x)
    /// let g1 = Gf2mPoly::from_exponents(&field, &[0, 1, 3, 5, 14]);
    /// let g2 = Gf2mPoly::from_exponents(&field, &[0, 6, 8, 11, 14]);
    /// let g3 = Gf2mPoly::from_exponents(&field, &[0, 1, 2, 6, 9, 10, 14]);
    ///
    /// let generator = Gf2mPoly::product(&[g1, g2, g3]);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n · d²) where n is number of polynomials and d is average degree.
    /// Uses existing optimized polynomial multiplication.
    ///
    /// # Panics
    ///
    /// Panics if `polys` is empty.
    pub fn product(polys: &[Gf2mPoly_<V>]) -> Self {
        assert!(!polys.is_empty(), "cannot compute product of empty list");

        if polys.len() == 1 {
            return polys[0].clone();
        }

        // Sequential multiplication using existing optimized * operator
        polys
            .iter()
            .skip(1)
            .fold(polys[0].clone(), |acc, p| &acc * p)
    }

    /// Evaluates the polynomial at a given point using Horner's method.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// // p(x) = 1 + 2x + 3x^2
    /// let poly = Gf2mPoly::new(vec![
    ///     field.element(1),
    ///     field.element(2),
    ///     field.element(3),
    /// ]);
    /// let x = field.element(5);
    /// let result = poly.eval(&x);
    /// // result = 1 + 2*5 + 3*5^2
    /// ```
    pub fn eval(&self, x: &Gf2mElement_<V>) -> Gf2mElement_<V> {
        if self.coeffs.is_empty() {
            panic!("Cannot evaluate empty polynomial");
        }

        // Horner's method: a_n*x^n + ... + a_1*x + a_0
        // = ((...((a_n)*x + a_{n-1})*x + ... + a_1)*x + a_0
        let mut result = self.coeffs.last().unwrap().clone();

        for i in (0..self.coeffs.len() - 1).rev() {
            result = &(&result * x) + &self.coeffs[i];
        }

        result
    }

    /// Evaluates the polynomial at multiple points.
    ///
    /// This is useful for BCH syndrome computation where you need to evaluate
    /// the same polynomial at multiple consecutive powers (α, α², α³, ...).
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let poly = Gf2mPoly::new(vec![
    ///     field.element(1),
    ///     field.element(2),
    ///     field.element(3),
    /// ]);
    ///
    /// // Evaluate at multiple points
    /// let points = vec![field.element(1), field.element(2), field.element(5)];
    /// let results = poly.eval_batch(&points);
    ///
    /// assert_eq!(results.len(), 3);
    /// // Each result is p(points[i])
    /// ```
    pub fn eval_batch(&self, points: &[Gf2mElement_<V>]) -> Vec<Gf2mElement_<V>> {
        points.iter().map(|x| self.eval(x)).collect()
    }

    /// Divides this polynomial by another, returning (quotient, remainder).
    ///
    /// Ensures that: dividend = quotient * divisor + remainder
    /// where degree(remainder) < degree(divisor).
    ///
    /// # Panics
    ///
    /// Panics if divisor is zero.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let dividend = Gf2mPoly::new(vec![field.element(1), field.element(1), field.element(1)]);
    /// let divisor = Gf2mPoly::new(vec![field.element(1), field.element(1)]);
    ///
    /// let (quotient, remainder) = dividend.div_rem(&divisor);
    /// // Verify: quotient * divisor + remainder = dividend
    /// assert_eq!(&(&quotient * &divisor) + &remainder, dividend);
    /// ```
    pub fn div_rem(&self, divisor: &Gf2mPoly_<V>) -> (Gf2mPoly_<V>, Gf2mPoly_<V>) {
        if divisor.is_zero() {
            panic!("division by zero polynomial");
        }

        let field = Gf2mField_ {
            params: self.coeffs[0].params.clone(),
        };

        // If dividend degree < divisor degree, quotient is 0 and remainder is dividend
        if self.degree().is_none() {
            return (Gf2mPoly_::zero(&field), self.clone());
        }

        let dividend_deg = self.degree().unwrap();
        let divisor_deg = divisor.degree().unwrap();

        if dividend_deg < divisor_deg {
            return (Gf2mPoly_::zero(&field), self.clone());
        }

        // Long division algorithm
        let mut remainder = self.clone();
        let mut quotient_coeffs = vec![field.zero(); dividend_deg - divisor_deg + 1];

        let divisor_lead = divisor.coeffs.last().unwrap();

        while let Some(rem_deg) = remainder.degree() {
            if rem_deg < divisor_deg {
                break;
            }

            // Compute the next quotient coefficient
            let rem_lead = remainder.coeffs.last().unwrap();
            let q_coeff = rem_lead / divisor_lead;
            let q_deg = rem_deg - divisor_deg;

            quotient_coeffs[q_deg] = q_coeff.clone();

            // Subtract q_coeff * x^q_deg * divisor from remainder
            for i in 0..divisor.coeffs.len() {
                let sub_term = &q_coeff * &divisor.coeffs[i];
                remainder.coeffs[i + q_deg] = &remainder.coeffs[i + q_deg] + &sub_term;
            }

            remainder.normalize();
        }

        let quotient = Gf2mPoly_::new(quotient_coeffs);
        (quotient, remainder)
    }

    /// Computes the greatest common divisor (GCD) of two polynomials using Euclidean algorithm.
    ///
    /// Returns a monic polynomial (leading coefficient is 1) that is the GCD.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// // p1 = (x + 1)(x + 2) = x^2 + 3x + 2
    /// let a = Gf2mPoly::new(vec![field.element(1), field.element(1)]);
    /// let b = Gf2mPoly::new(vec![field.element(2), field.element(1)]);
    /// let p1 = &a * &b;
    ///
    /// // p2 = (x + 1)(x + 3) = x^2 + 4x + 3
    /// let c = Gf2mPoly::new(vec![field.element(3), field.element(1)]);
    /// let p2 = &a * &c;
    ///
    /// let gcd = Gf2mPoly::gcd(&p1, &p2);
    /// // GCD should be (x + 1) or a scalar multiple
    /// assert_eq!(gcd.degree(), Some(1));
    /// ```
    pub fn gcd(a: &Gf2mPoly_<V>, b: &Gf2mPoly_<V>) -> Gf2mPoly_<V> {
        let mut r0 = a.clone();
        let mut r1 = b.clone();

        while !r1.is_zero() {
            let (_, remainder) = r0.div_rem(&r1);
            r0 = r1;
            r1 = remainder;
        }

        // Make the GCD monic (leading coefficient = 1)
        if let Some(lead) = r0.coeffs.last() {
            if !lead.is_zero() && !lead.is_one() {
                let inv = lead.inverse().unwrap();
                let mut monic_coeffs = Vec::with_capacity(r0.coeffs.len());
                for coeff in &r0.coeffs {
                    monic_coeffs.push(&inv * coeff);
                }
                return Gf2mPoly_::new(monic_coeffs);
            }
        }

        r0
    }

    /// Multiplies two polynomials using schoolbook algorithm.
    ///
    /// This is the baseline O(n²) algorithm, used for small polynomials
    /// and as a subroutine in Karatsuba multiplication.
    fn mul_schoolbook(&self, rhs: &Gf2mPoly_<V>) -> Gf2mPoly_<V> {
        if self.is_zero() || rhs.is_zero() {
            return Gf2mPoly_::zero(&Gf2mField_ {
                params: self.coeffs[0].params.clone(),
            });
        }

        let deg_self = self.degree().unwrap();
        let deg_rhs = rhs.degree().unwrap();
        let result_deg = deg_self + deg_rhs;

        let field = Gf2mField_ {
            params: self.coeffs[0].params.clone(),
        };
        let mut coeffs = vec![field.zero(); result_deg + 1];

        for i in 0..=deg_self {
            for j in 0..=deg_rhs {
                let term = &self.coeffs[i] * &rhs.coeffs[j];
                coeffs[i + j] = &coeffs[i + j] + &term;
            }
        }

        Gf2mPoly_::new(coeffs)
    }

    /// Multiplies two polynomials using Karatsuba algorithm.
    ///
    /// This recursive algorithm achieves O(n^1.585) complexity by splitting
    /// polynomials and reducing the number of recursive multiplications from 4 to 3.
    ///
    /// For polynomials p(x) and q(x) of degree n:
    /// 1. Split at midpoint m = n/2:
    ///    - p(x) = p_hi(x)·x^m + p_lo(x)
    ///    - q(x) = q_hi(x)·x^m + q_lo(x)
    /// 2. Compute 3 products:
    ///    - z₂ = p_hi · q_hi
    ///    - z₀ = p_lo · q_lo
    ///    - z₁ = (p_hi + p_lo) · (q_hi + q_lo) - z₂ - z₀
    /// 3. Recombine: p·q = z₂·x^(2m) + z₁·x^m + z₀
    fn mul_karatsuba(&self, rhs: &Gf2mPoly_<V>) -> Gf2mPoly_<V> {
        const KARATSUBA_THRESHOLD: usize = 32;

        if self.is_zero() || rhs.is_zero() {
            return Gf2mPoly_::zero(&Gf2mField_ {
                params: self.coeffs[0].params.clone(),
            });
        }

        let deg_self = self.degree().unwrap();
        let deg_rhs = rhs.degree().unwrap();

        // Use schoolbook for small polynomials
        if deg_self < KARATSUBA_THRESHOLD || deg_rhs < KARATSUBA_THRESHOLD {
            return self.mul_schoolbook(rhs);
        }

        // Split at midpoint
        let m = deg_self.max(deg_rhs) / 2 + 1;

        let field = Gf2mField_ {
            params: self.coeffs[0].params.clone(),
        };

        // Split self into p_lo + p_hi * x^m
        let p_lo_coeffs: Vec<_> = self.coeffs.iter().take(m).cloned().collect();
        let p_hi_coeffs: Vec<_> = self.coeffs.iter().skip(m).cloned().collect();

        let p_lo = if p_lo_coeffs.is_empty() {
            Gf2mPoly_::zero(&field)
        } else {
            Gf2mPoly_::new(p_lo_coeffs)
        };

        let p_hi = if p_hi_coeffs.is_empty() {
            Gf2mPoly_::zero(&field)
        } else {
            Gf2mPoly_::new(p_hi_coeffs)
        };

        // Split rhs into q_lo + q_hi * x^m
        let q_lo_coeffs: Vec<_> = rhs.coeffs.iter().take(m).cloned().collect();
        let q_hi_coeffs: Vec<_> = rhs.coeffs.iter().skip(m).cloned().collect();

        let q_lo = if q_lo_coeffs.is_empty() {
            Gf2mPoly_::zero(&field)
        } else {
            Gf2mPoly_::new(q_lo_coeffs)
        };

        let q_hi = if q_hi_coeffs.is_empty() {
            Gf2mPoly_::zero(&field)
        } else {
            Gf2mPoly_::new(q_hi_coeffs)
        };

        // Three recursive multiplications
        let z0 = p_lo.mul_karatsuba(&q_lo);
        let z2 = p_hi.mul_karatsuba(&q_hi);

        let p_sum = &p_hi + &p_lo;
        let q_sum = &q_hi + &q_lo;
        let z1_full = p_sum.mul_karatsuba(&q_sum);
        let z1 = &(&z1_full + &z2) + &z0;

        // Combine: z2 * x^(2m) + z1 * x^m + z0
        let mut result_coeffs = vec![field.zero(); deg_self + deg_rhs + 1];

        // Add z0 coefficients
        for (i, coeff) in z0.coeffs.iter().enumerate() {
            result_coeffs[i] = coeff.clone();
        }

        // Add z1 * x^m coefficients
        for (i, coeff) in z1.coeffs.iter().enumerate() {
            result_coeffs[i + m] = &result_coeffs[i + m] + coeff;
        }

        // Add z2 * x^(2m) coefficients
        for (i, coeff) in z2.coeffs.iter().enumerate() {
            result_coeffs[i + 2 * m] = &result_coeffs[i + 2 * m] + coeff;
        }

        Gf2mPoly_::new(result_coeffs)
    }
}

impl<V: UintExt> PartialEq for Gf2mPoly_<V> {
    fn eq(&self, other: &Self) -> bool {
        if self.coeffs.len() != other.coeffs.len() {
            return false;
        }
        self.coeffs
            .iter()
            .zip(other.coeffs.iter())
            .all(|(a, b)| a == b)
    }
}

impl<V: UintExt> Eq for Gf2mPoly_<V> {}

// Polynomial addition
impl<V: UintExt> Add for &Gf2mPoly_<V> {
    type Output = Gf2mPoly_<V>;

    fn add(self, rhs: Self) -> Self::Output {
        let max_len = self.coeffs.len().max(rhs.coeffs.len());
        let mut coeffs = Vec::with_capacity(max_len);

        for i in 0..max_len {
            let a = self.coeff(i);
            let b = rhs.coeff(i);
            coeffs.push(&a + &b);
        }

        Gf2mPoly_::new(coeffs)
    }
}

impl<V: UintExt> Add for Gf2mPoly_<V> {
    type Output = Gf2mPoly_<V>;

    fn add(self, rhs: Self) -> Self::Output {
        &self + &rhs
    }
}

// Polynomial multiplication
impl<V: UintExt> Mul for &Gf2mPoly_<V> {
    type Output = Gf2mPoly_<V>;

    fn mul(self, rhs: Self) -> Self::Output {
        // Use Karatsuba for large polynomials, schoolbook for small
        self.mul_karatsuba(rhs)
    }
}

impl<V: UintExt> Mul for Gf2mPoly_<V> {
    type Output = Gf2mPoly_<V>;

    fn mul(self, rhs: Self) -> Self::Output {
        &self * &rhs
    }
}

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

    #[test]
    fn test_karatsuba_vs_schoolbook_small() {
        // Test that Karatsuba matches schoolbook for polynomials below threshold
        let field = Gf2mField::new(4, 0b10011);
        let p1 = Gf2mPoly_::new(vec![field.element(1), field.element(2), field.element(3)]);
        let p2 = Gf2mPoly_::new(vec![field.element(4), field.element(5)]);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
    }

    #[test]
    fn test_karatsuba_vs_schoolbook_at_threshold() {
        // Test degree 32 (at threshold boundary)
        let field = Gf2mField::gf256();
        let coeffs1: Vec<_> = (0..33).map(|i| field.element((i % 256) as u64)).collect();
        let coeffs2: Vec<_> = (0..33)
            .map(|i| field.element(((i * 7) % 256) as u64))
            .collect();

        let p1 = Gf2mPoly_::new(coeffs1);
        let p2 = Gf2mPoly_::new(coeffs2);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
    }

    #[test]
    fn test_karatsuba_vs_schoolbook_above_threshold() {
        // Test degree 64 (well above threshold)
        let field = Gf2mField::gf256();
        let coeffs1: Vec<_> = (0..65).map(|i| field.element((i % 256) as u64)).collect();
        let coeffs2: Vec<_> = (0..65)
            .map(|i| field.element(((i * 13) % 256) as u64))
            .collect();

        let p1 = Gf2mPoly_::new(coeffs1);
        let p2 = Gf2mPoly_::new(coeffs2);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
    }

    #[test]
    fn test_karatsuba_degree_100() {
        // Test degree 100 polynomials (realistic BCH-like scenario)
        let field = Gf2mField::gf256();
        let coeffs1: Vec<_> = (0..101).map(|i| field.element((i % 256) as u64)).collect();
        let coeffs2: Vec<_> = (0..101)
            .map(|i| field.element(((i * 17) % 256) as u64))
            .collect();

        let p1 = Gf2mPoly_::new(coeffs1);
        let p2 = Gf2mPoly_::new(coeffs2);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
        assert_eq!(result_karatsuba.degree(), Some(200)); // deg(p1) + deg(p2)
    }

    #[test]
    fn test_karatsuba_degree_200() {
        // Test degree 200 polynomials (critical BCH-255 benchmark)
        let field = Gf2mField::gf256();
        let coeffs1: Vec<_> = (0..201).map(|i| field.element((i % 256) as u64)).collect();
        let coeffs2: Vec<_> = (0..201)
            .map(|i| field.element(((i * 19) % 256) as u64))
            .collect();

        let p1 = Gf2mPoly_::new(coeffs1);
        let p2 = Gf2mPoly_::new(coeffs2);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
        assert_eq!(result_karatsuba.degree(), Some(400));
    }

    #[test]
    fn test_karatsuba_with_zero() {
        let field = Gf2mField::gf256();
        let p1 = Gf2mPoly_::new(vec![field.element(1), field.element(2)]);
        let zero = Gf2mPoly_::zero(&field);

        assert_eq!(p1.mul_karatsuba(&zero), zero);
        assert_eq!(zero.mul_karatsuba(&p1), zero);
    }

    #[test]
    fn test_karatsuba_different_degrees() {
        // Test polynomials with very different degrees
        let field = Gf2mField::gf256();
        let p1_coeffs: Vec<_> = (0..100).map(|i| field.element((i % 256) as u64)).collect();
        let p2_coeffs: Vec<_> = (0..10).map(|i| field.element((i % 256) as u64)).collect();

        let p1 = Gf2mPoly_::new(p1_coeffs);
        let p2 = Gf2mPoly_::new(p2_coeffs);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
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

            // (p1 + p2)(x) = p1(x) + p2(x)
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

        // Karatsuba property tests
        #[test]
        fn prop_karatsuba_equals_schoolbook_small(
            deg1 in 1usize..10,
            deg2 in 1usize..10,
            seed in 1u64..256
        ) {
            let field = Gf2mField::gf256();

            let coeffs1: Vec<_> = (0..=deg1)
                .map(|i| field.element((i as u64 * seed) % 256))
                .collect();
            let coeffs2: Vec<_> = (0..=deg2)
                .map(|i| field.element((i as u64 * seed * 7) % 256))
                .collect();

            let p1 = Gf2mPoly_::new(coeffs1);
            let p2 = Gf2mPoly_::new(coeffs2);

            let result_karatsuba = p1.mul_karatsuba(&p2);
            let result_schoolbook = p1.mul_schoolbook(&p2);

            prop_assert_eq!(result_karatsuba, result_schoolbook);
        }

        #[test]
        fn prop_karatsuba_equals_schoolbook_medium(
            deg1 in 30usize..60,
            deg2 in 30usize..60,
            seed in 1u64..256
        ) {
            let field = Gf2mField::gf256();

            let coeffs1: Vec<_> = (0..=deg1)
                .map(|i| field.element((i as u64 * seed) % 256))
                .collect();
            let coeffs2: Vec<_> = (0..=deg2)
                .map(|i| field.element((i as u64 * seed * 11) % 256))
                .collect();

            let p1 = Gf2mPoly_::new(coeffs1);
            let p2 = Gf2mPoly_::new(coeffs2);

            let result_karatsuba = p1.mul_karatsuba(&p2);
            let result_schoolbook = p1.mul_schoolbook(&p2);

            prop_assert_eq!(result_karatsuba, result_schoolbook);
        }

        #[test]
        fn prop_karatsuba_equals_schoolbook_large(
            deg1 in 100usize..150,
            deg2 in 100usize..150,
            seed in 1u64..256
        ) {
            let field = Gf2mField::gf256();

            let coeffs1: Vec<_> = (0..=deg1)
                .map(|i| field.element((i as u64 * seed) % 256))
                .collect();
            let coeffs2: Vec<_> = (0..=deg2)
                .map(|i| field.element((i as u64 * seed * 13) % 256))
                .collect();

            let p1 = Gf2mPoly_::new(coeffs1);
            let p2 = Gf2mPoly_::new(coeffs2);

            let result_karatsuba = p1.mul_karatsuba(&p2);
            let result_schoolbook = p1.mul_schoolbook(&p2);

            prop_assert_eq!(result_karatsuba, result_schoolbook);
        }
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
                    bits.push_bit((i * seed as usize) % 3 == 0);
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
                        if (i as u64 * seed) % 3 == 0 {
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
    #[should_panic(expected = "cannot compute product of empty list")]
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
}
