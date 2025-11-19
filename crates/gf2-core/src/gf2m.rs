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
use std::ops::{Add, Div, Mul};
use std::rc::Rc;

#[cfg(feature = "simd")]
use gf2_kernels_simd::gf2m as simd_gf2m;

/// A binary extension field GF(2^m) with a specified primitive polynomial.
///
/// This type defines the field structure and parameters. Individual field elements
/// are created via [`Gf2mField::element`].
#[derive(Clone, Debug)]
pub struct Gf2mField {
    params: Rc<FieldParams>,
}

#[derive(Debug)]
struct FieldParams {
    m: usize,
    primitive_poly: u64,
    // Log/antilog tables for fast multiplication (m ≤ 16)
    log_table: Option<Vec<u16>>, // log_table[α^i] = i
    exp_table: Option<Vec<u16>>, // exp_table[i] = α^i
    // SIMD multiplication function (if available)
    #[cfg(feature = "simd")]
    simd_mul_fn: Option<simd_gf2m::Gf2mMulFn>,
}

impl PartialEq for FieldParams {
    fn eq(&self, other: &Self) -> bool {
        self.m == other.m && self.primitive_poly == other.primitive_poly
    }
}

impl Eq for FieldParams {}

/// An element of a binary extension field GF(2^m).
///
/// Elements are represented as polynomials over GF(2) with degree < m,
/// encoded as binary integers where bit i represents the coefficient of x^i.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gf2mElement {
    value: u64,
    params: Rc<FieldParams>,
}

impl Gf2mField {
    /// Creates a new GF(2^m) field with the specified primitive polynomial.
    ///
    /// # Arguments
    ///
    /// * `m` - Extension degree (field has 2^m elements, currently limited to m ≤ 64)
    /// * `primitive_poly` - Primitive polynomial of degree m in binary representation
    ///
    /// # Panics
    ///
    /// Panics if m > 64 (not yet supported) or m == 0.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// // GF(2^4) with primitive polynomial x^4 + x + 1 (binary 10011)
    /// let field = Gf2mField::new(4, 0b10011);
    /// ```
    pub fn new(m: usize, primitive_poly: u64) -> Self {
        assert!(m > 0, "Extension degree m must be positive");
        assert!(m <= 64, "Extension degree m > 64 not yet supported");

        #[cfg(feature = "simd")]
        let simd_mul_fn = simd_gf2m::detect().map(|fns| fns.mul_fn);

        Gf2mField {
            params: Rc::new(FieldParams {
                m,
                primitive_poly,
                log_table: None,
                exp_table: None,
                #[cfg(feature = "simd")]
                simd_mul_fn,
            }),
        }
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
    /// assert_eq!(gf256.order(), 256);
    /// ```
    pub fn gf256() -> Self {
        // x^8 + x^4 + x^3 + x + 1 = binary 100011011
        Gf2mField::new(8, 0b100011011)
    }

    /// Creates a GF(2^16) field with standard primitive polynomial x^16 + x^12 + x^3 + x + 1.
    ///
    /// # Example
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let gf65536 = Gf2mField::gf65536();
    /// assert_eq!(gf65536.order(), 65536);
    /// ```
    pub fn gf65536() -> Self {
        // x^16 + x^12 + x^3 + x + 1 = binary 10001000000001011
        Gf2mField::new(16, 0b10001000000001011)
    }

    /// Returns the extension degree m.
    pub fn degree(&self) -> usize {
        self.params.m
    }

    /// Returns the number of elements in the field (2^m).
    pub fn order(&self) -> usize {
        1 << self.params.m
    }

    /// Returns the primitive polynomial.
    pub fn primitive_polynomial(&self) -> u64 {
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
    pub fn element(&self, value: u64) -> Gf2mElement {
        let max_value = (1u64 << self.params.m) - 1;
        assert!(
            value <= max_value,
            "Element value {} exceeds field size (max {})",
            value,
            max_value
        );
        Gf2mElement {
            value,
            params: Rc::clone(&self.params),
        }
    }

    /// Returns the additive identity (zero) of the field.
    pub fn zero(&self) -> Gf2mElement {
        self.element(0)
    }

    /// Returns the multiplicative identity (one) of the field.
    pub fn one(&self) -> Gf2mElement {
        self.element(1)
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

        Gf2mField {
            params: Rc::new(FieldParams {
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
    pub fn primitive_element(&self) -> Option<Gf2mElement> {
        if !self.has_tables() {
            return None;
        }

        // The primitive element is typically x (value = 2)
        // But we verify it's actually stored in exp_table[1]
        self.params
            .exp_table
            .as_ref()
            .map(|exp| self.element(exp[1] as u64))
    }

    /// Returns the discrete logarithm of an element (if tables exist).
    ///
    /// Returns the value i such that α^i = element, where α is the primitive element.
    /// Returns None for zero or if tables don't exist.
    pub fn discrete_log(&self, element: &Gf2mElement) -> Option<u16> {
        if element.is_zero() || !self.has_tables() {
            return None;
        }

        self.params
            .log_table
            .as_ref()
            .map(|log| log[element.value() as usize])
    }

    /// Returns α^i where α is the primitive element (if tables exist).
    pub fn exp_value(&self, i: usize) -> Option<Gf2mElement> {
        if !self.has_tables() {
            return None;
        }

        self.params.exp_table.as_ref().map(|exp| {
            let order = (1 << self.params.m) - 1; // Multiplicative group order
            let idx = i % order;
            self.element(exp[idx] as u64)
        })
    }

    /// Generates log and exp tables for a field.
    ///
    /// Returns (log_table, exp_table) where:
    /// - exp_table[i] = α^i for i = 0..2^m-1
    /// - log_table[α^i] = i
    ///
    /// The primitive element α is found by testing candidates.
    fn generate_tables(m: usize, primitive_poly: u64) -> (Vec<u16>, Vec<u16>) {
        let order = (1usize << m) - 1; // Multiplicative group order

        // Find a primitive element (generator of the multiplicative group)
        let alpha = Self::find_primitive_element(m, primitive_poly, order);

        // Build exp_table: exp[i] = α^i
        let mut exp_table = vec![0u16; order];
        let mut current = 1u64;

        for elem in exp_table.iter_mut() {
            *elem = current as u16;

            // Multiply by alpha (using schoolbook since we don't have tables yet)
            current = Self::mul_raw(current, alpha, m, primitive_poly);
        }

        // Build log_table: log[α^i] = i (inverse mapping)
        let mut log_table = vec![0u16; 1 << m];
        for (i, &exp_val) in exp_table.iter().enumerate() {
            log_table[exp_val as usize] = i as u16;
        }
        // log[0] is undefined, set to 0 by convention
        log_table[0] = 0;

        (log_table, exp_table)
    }

    /// Finds a primitive element for GF(2^m).
    ///
    /// Tests candidates starting from x (value = 2) until we find one that
    /// generates the full multiplicative group of order 2^m - 1.
    fn find_primitive_element(m: usize, primitive_poly: u64, order: usize) -> u64 {
        // Try candidates starting from 2 (which represents x)
        for candidate in 2..(1u64 << m) {
            if Self::is_primitive(candidate, m, primitive_poly, order) {
                return candidate;
            }
        }
        panic!("No primitive element found (should not happen for valid primitive polynomial)");
    }

    /// Tests if an element is primitive (generates the full multiplicative group).
    fn is_primitive(elem: u64, m: usize, primitive_poly: u64, order: usize) -> bool {
        let mut current = elem;

        // Check that elem^order = 1 and no smaller power equals 1
        for _ in 1..order {
            if current == 1 {
                return false; // Order is too small
            }
            current = Self::mul_raw(current, elem, m, primitive_poly);
        }

        current == 1 // Should cycle back to 1 after exactly 'order' steps
    }

    /// Raw multiplication without tables (used during table generation).
    fn mul_raw(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
        if a == 0 || b == 0 {
            return 0;
        }

        let mut result = 0u64;
        let mut temp = a;

        for i in 0..m {
            if (b >> i) & 1 == 1 {
                result ^= temp;
            }

            let will_overflow = (temp & (1u64 << (m - 1))) != 0;
            temp <<= 1;

            if will_overflow {
                temp ^= primitive_poly;
            }
        }

        result & ((1u64 << m) - 1)
    }
}

impl Gf2mElement {
    /// Returns the binary representation of this element.
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Returns true if this is the zero element.
    pub fn is_zero(&self) -> bool {
        self.value == 0
    }

    /// Returns true if this is the multiplicative identity (one).
    pub fn is_one(&self) -> bool {
        self.value == 1
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
    pub fn inverse(&self) -> Option<Gf2mElement> {
        if self.is_zero() {
            return None;
        }

        if self.is_one() {
            return Some(Gf2mElement {
                value: 1,
                params: Rc::clone(&self.params),
            });
        }

        // Use field multiplication to compute inverse via exponentiation
        // In GF(2^m), a^(2^m - 1) = 1 for all non-zero a
        // Therefore a^(-1) = a^(2^m - 2)
        let m = self.params.m;
        let exp = (1u64 << m) - 2;

        let mut result = Gf2mElement {
            value: 1,
            params: Rc::clone(&self.params),
        };
        let mut base = self.clone();
        let mut e = exp;

        // Square-and-multiply algorithm
        while e > 0 {
            if e & 1 == 1 {
                result = &result * &base;
            }
            base = &base * &base;
            e >>= 1;
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
    pub fn minimal_polynomial(&self) -> Gf2mPoly {
        // Special case: minimal polynomial of 0 is x
        if self.is_zero() {
            return Gf2mPoly::new(vec![
                Gf2mElement {
                    value: 0,
                    params: Rc::clone(&self.params),
                },
                Gf2mElement {
                    value: 1,
                    params: Rc::clone(&self.params),
                },
            ]);
        }

        // Find all conjugates: α, α^2, α^4, α^8, ... until we cycle back
        let mut conjugates = Vec::new();
        let mut current = self.clone();

        loop {
            // Check if we've seen this conjugate before
            if conjugates
                .iter()
                .any(|c: &Gf2mElement| c.value == current.value)
            {
                break;
            }
            conjugates.push(current.clone());

            // Square to get next conjugate: α^(2^i)
            current = &current * &current;
        }

        // Build minimal polynomial as product of (x - conjugate) terms
        // Start with polynomial 1
        let one = Gf2mElement {
            value: 1,
            params: Rc::clone(&self.params),
        };
        let mut result = Gf2mPoly::constant(one);

        for conjugate in conjugates {
            // Build (x - conjugate) = x + conjugate (since -1 = 1 in GF(2))
            let term = Gf2mPoly::new(vec![
                conjugate,
                Gf2mElement {
                    value: 1,
                    params: Rc::clone(&self.params),
                },
            ]);

            // Multiply into result
            result = &result * &term;
        }

        result
    }
}

// Addition in GF(2^m) is XOR
impl Add for &Gf2mElement {
    type Output = Gf2mElement;

    fn add(self, rhs: Self) -> Self::Output {
        assert!(
            Rc::ptr_eq(&self.params, &rhs.params),
            "Cannot add elements from different fields"
        );
        Gf2mElement {
            value: self.value ^ rhs.value,
            params: Rc::clone(&self.params),
        }
    }
}

impl Add for Gf2mElement {
    type Output = Gf2mElement;

    fn add(self, rhs: Self) -> Self::Output {
        &self + &rhs
    }
}

// Multiplication in GF(2^m) - polynomial multiplication with reduction
impl Mul for &Gf2mElement {
    type Output = Gf2mElement;

    fn mul(self, rhs: Self) -> Self::Output {
        assert!(
            Rc::ptr_eq(&self.params, &rhs.params),
            "Cannot multiply elements from different fields"
        );

        if self.value == 0 || rhs.value == 0 {
            return Gf2mElement {
                value: 0,
                params: Rc::clone(&self.params),
            };
        }

        // Priority 1: Use table-based multiplication if available (fastest for small m)
        if let (Some(log_table), Some(exp_table)) = (
            self.params.log_table.as_ref(),
            self.params.exp_table.as_ref(),
        ) {
            let log_a = log_table[self.value as usize] as usize;
            let log_b = log_table[rhs.value as usize] as usize;
            let order = (1 << self.params.m) - 1;
            let log_result = (log_a + log_b) % order;

            return Gf2mElement {
                value: exp_table[log_result] as u64,
                params: Rc::clone(&self.params),
            };
        }

        // Priority 2: Use SIMD if available (faster than schoolbook for larger m)
        #[cfg(feature = "simd")]
        if let Some(simd_mul_fn) = self.params.simd_mul_fn {
            let result = simd_mul_fn(
                self.value,
                rhs.value,
                self.params.m,
                self.params.primitive_poly,
            );
            return Gf2mElement {
                value: result,
                params: Rc::clone(&self.params),
            };
        }

        // Priority 3: Fallback to schoolbook multiplication
        let m = self.params.m;
        let primitive_poly = self.params.primitive_poly;

        let mut result = 0u64;
        let mut temp = self.value;

        for i in 0..m {
            if (rhs.value >> i) & 1 == 1 {
                result ^= temp;
            }

            let will_overflow = (temp & (1u64 << (m - 1))) != 0;
            temp <<= 1;

            if will_overflow {
                temp ^= primitive_poly;
            }
        }

        Gf2mElement {
            value: result & ((1u64 << m) - 1),
            params: Rc::clone(&self.params),
        }
    }
}

impl Mul for Gf2mElement {
    type Output = Gf2mElement;

    fn mul(self, rhs: Self) -> Self::Output {
        &self * &rhs
    }
}

// Division in GF(2^m) - multiply by multiplicative inverse
impl Div for &Gf2mElement {
    type Output = Gf2mElement;

    fn div(self, rhs: Self) -> Self::Output {
        assert!(
            Rc::ptr_eq(&self.params, &rhs.params),
            "Cannot divide elements from different fields"
        );

        let inv = rhs.inverse().expect("division by zero");
        self * &inv
    }
}

impl Div for Gf2mElement {
    type Output = Gf2mElement;

    fn div(self, rhs: Self) -> Self::Output {
        &self / &rhs
    }
}

impl fmt::Display for Gf2mElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#b}", self.value)
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
#[derive(Clone, Debug)]
pub struct Gf2mPoly {
    coeffs: Vec<Gf2mElement>,
}

impl Gf2mPoly {
    /// Creates a new polynomial from coefficients.
    ///
    /// Coefficients are in ascending order: `coeffs[i]` is the coefficient of x^i.
    /// Leading zero coefficients are automatically removed.
    pub fn new(coeffs: Vec<Gf2mElement>) -> Self {
        let mut poly = Gf2mPoly { coeffs };
        poly.normalize();
        poly
    }

    /// Creates the zero polynomial.
    pub fn zero(field: &Gf2mField) -> Self {
        Gf2mPoly {
            coeffs: vec![field.zero()],
        }
    }

    /// Creates a constant polynomial.
    pub fn constant(value: Gf2mElement) -> Self {
        Gf2mPoly {
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
    pub fn coeff(&self, i: usize) -> Gf2mElement {
        if i < self.coeffs.len() {
            self.coeffs[i].clone()
        } else {
            // Return zero for coefficients beyond degree
            let field = Gf2mField {
                params: self.coeffs[0].params.clone(),
            };
            field.zero()
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
    pub fn from_bitvec(bits: &crate::BitVec, field: &Gf2mField) -> Self {
        if bits.is_empty() {
            return Self::zero(field);
        }

        let coeffs: Vec<Gf2mElement> = (0..bits.len())
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
    pub fn eval(&self, x: &Gf2mElement) -> Gf2mElement {
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
    pub fn eval_batch(&self, points: &[Gf2mElement]) -> Vec<Gf2mElement> {
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
    pub fn div_rem(&self, divisor: &Gf2mPoly) -> (Gf2mPoly, Gf2mPoly) {
        if divisor.is_zero() {
            panic!("division by zero polynomial");
        }

        let field = Gf2mField {
            params: self.coeffs[0].params.clone(),
        };

        // If dividend degree < divisor degree, quotient is 0 and remainder is dividend
        if self.degree().is_none() {
            return (Gf2mPoly::zero(&field), self.clone());
        }

        let dividend_deg = self.degree().unwrap();
        let divisor_deg = divisor.degree().unwrap();

        if dividend_deg < divisor_deg {
            return (Gf2mPoly::zero(&field), self.clone());
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

        let quotient = Gf2mPoly::new(quotient_coeffs);
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
    pub fn gcd(a: &Gf2mPoly, b: &Gf2mPoly) -> Gf2mPoly {
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
                return Gf2mPoly::new(monic_coeffs);
            }
        }

        r0
    }

    /// Multiplies two polynomials using schoolbook algorithm.
    ///
    /// This is the baseline O(n²) algorithm, used for small polynomials
    /// and as a subroutine in Karatsuba multiplication.
    fn mul_schoolbook(&self, rhs: &Gf2mPoly) -> Gf2mPoly {
        if self.is_zero() || rhs.is_zero() {
            return Gf2mPoly::zero(&Gf2mField {
                params: self.coeffs[0].params.clone(),
            });
        }

        let deg_self = self.degree().unwrap();
        let deg_rhs = rhs.degree().unwrap();
        let result_deg = deg_self + deg_rhs;

        let field = Gf2mField {
            params: self.coeffs[0].params.clone(),
        };
        let mut coeffs = vec![field.zero(); result_deg + 1];

        for i in 0..=deg_self {
            for j in 0..=deg_rhs {
                let term = &self.coeffs[i] * &rhs.coeffs[j];
                coeffs[i + j] = &coeffs[i + j] + &term;
            }
        }

        Gf2mPoly::new(coeffs)
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
    fn mul_karatsuba(&self, rhs: &Gf2mPoly) -> Gf2mPoly {
        const KARATSUBA_THRESHOLD: usize = 32;

        if self.is_zero() || rhs.is_zero() {
            return Gf2mPoly::zero(&Gf2mField {
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

        let field = Gf2mField {
            params: self.coeffs[0].params.clone(),
        };

        // Split self into p_lo + p_hi * x^m
        let p_lo_coeffs: Vec<_> = self.coeffs.iter().take(m).cloned().collect();
        let p_hi_coeffs: Vec<_> = self.coeffs.iter().skip(m).cloned().collect();

        let p_lo = if p_lo_coeffs.is_empty() {
            Gf2mPoly::zero(&field)
        } else {
            Gf2mPoly::new(p_lo_coeffs)
        };

        let p_hi = if p_hi_coeffs.is_empty() {
            Gf2mPoly::zero(&field)
        } else {
            Gf2mPoly::new(p_hi_coeffs)
        };

        // Split rhs into q_lo + q_hi * x^m
        let q_lo_coeffs: Vec<_> = rhs.coeffs.iter().take(m).cloned().collect();
        let q_hi_coeffs: Vec<_> = rhs.coeffs.iter().skip(m).cloned().collect();

        let q_lo = if q_lo_coeffs.is_empty() {
            Gf2mPoly::zero(&field)
        } else {
            Gf2mPoly::new(q_lo_coeffs)
        };

        let q_hi = if q_hi_coeffs.is_empty() {
            Gf2mPoly::zero(&field)
        } else {
            Gf2mPoly::new(q_hi_coeffs)
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

        Gf2mPoly::new(result_coeffs)
    }
}

impl PartialEq for Gf2mPoly {
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

impl Eq for Gf2mPoly {}

// Polynomial addition
impl Add for &Gf2mPoly {
    type Output = Gf2mPoly;

    fn add(self, rhs: Self) -> Self::Output {
        let max_len = self.coeffs.len().max(rhs.coeffs.len());
        let mut coeffs = Vec::with_capacity(max_len);

        for i in 0..max_len {
            let a = self.coeff(i);
            let b = rhs.coeff(i);
            coeffs.push(&a + &b);
        }

        Gf2mPoly::new(coeffs)
    }
}

impl Add for Gf2mPoly {
    type Output = Gf2mPoly;

    fn add(self, rhs: Self) -> Self::Output {
        &self + &rhs
    }
}

// Polynomial multiplication
impl Mul for &Gf2mPoly {
    type Output = Gf2mPoly;

    fn mul(self, rhs: Self) -> Self::Output {
        // Use Karatsuba for large polynomials, schoolbook for small
        self.mul_karatsuba(rhs)
    }
}

impl Mul for Gf2mPoly {
    type Output = Gf2mPoly;

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
        let poly = Gf2mPoly::new(coeffs);

        assert_eq!(poly.degree(), Some(2));
        assert!(!poly.is_zero());
    }

    #[test]
    fn test_zero_poly() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::zero(&field);

        assert!(poly.is_zero());
        assert_eq!(poly.degree(), None);
    }

    #[test]
    fn test_constant_poly() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::constant(field.element(5));

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
        let poly = Gf2mPoly::new(coeffs);

        assert_eq!(poly.degree(), Some(1)); // Leading zeros removed
    }

    #[test]
    fn test_poly_coeff_access() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::new(vec![field.element(1), field.element(2), field.element(3)]);

        assert_eq!(poly.coeff(0).value(), 1);
        assert_eq!(poly.coeff(1).value(), 2);
        assert_eq!(poly.coeff(2).value(), 3);
        assert_eq!(poly.coeff(10).value(), 0); // Beyond degree returns zero
    }

    #[test]
    fn test_poly_addition() {
        let field = Gf2mField::new(4, 0b10011);
        // p1(x) = 1 + 2x + 3x^2
        let p1 = Gf2mPoly::new(vec![field.element(1), field.element(2), field.element(3)]);
        // p2(x) = 4 + 5x
        let p2 = Gf2mPoly::new(vec![field.element(4), field.element(5)]);

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
        let p1 = Gf2mPoly::constant(field.element(2));
        // p2(x) = 3
        let p2 = Gf2mPoly::constant(field.element(3));

        let product = &p1 * &p2;
        // product = 2 * 3 = 6 in the field
        assert_eq!(product.degree(), Some(0));
        assert_eq!(product.coeff(0), &field.element(2) * &field.element(3));
    }

    #[test]
    fn test_poly_multiplication_linear() {
        let field = Gf2mField::new(4, 0b10011);
        // p1(x) = 1 + x (coeffs: [1, 1])
        let p1 = Gf2mPoly::new(vec![field.element(1), field.element(1)]);
        // p2(x) = 2 + x (coeffs: [2, 1])
        let p2 = Gf2mPoly::new(vec![field.element(2), field.element(1)]);

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
        let p1 = Gf2mPoly::new(vec![field.element(1), field.element(2), field.element(3)]);
        let p2 = Gf2mPoly::new(vec![field.element(4), field.element(5)]);

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

        let p1 = Gf2mPoly::new(coeffs1);
        let p2 = Gf2mPoly::new(coeffs2);

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

        let p1 = Gf2mPoly::new(coeffs1);
        let p2 = Gf2mPoly::new(coeffs2);

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

        let p1 = Gf2mPoly::new(coeffs1);
        let p2 = Gf2mPoly::new(coeffs2);

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

        let p1 = Gf2mPoly::new(coeffs1);
        let p2 = Gf2mPoly::new(coeffs2);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
        assert_eq!(result_karatsuba.degree(), Some(400));
    }

    #[test]
    fn test_karatsuba_with_zero() {
        let field = Gf2mField::gf256();
        let p1 = Gf2mPoly::new(vec![field.element(1), field.element(2)]);
        let zero = Gf2mPoly::zero(&field);

        assert_eq!(p1.mul_karatsuba(&zero), zero);
        assert_eq!(zero.mul_karatsuba(&p1), zero);
    }

    #[test]
    fn test_karatsuba_different_degrees() {
        // Test polynomials with very different degrees
        let field = Gf2mField::gf256();
        let p1_coeffs: Vec<_> = (0..100).map(|i| field.element((i % 256) as u64)).collect();
        let p2_coeffs: Vec<_> = (0..10).map(|i| field.element((i % 256) as u64)).collect();

        let p1 = Gf2mPoly::new(p1_coeffs);
        let p2 = Gf2mPoly::new(p2_coeffs);

        let result_karatsuba = p1.mul_karatsuba(&p2);
        let result_schoolbook = p1.mul_schoolbook(&p2);

        assert_eq!(result_karatsuba, result_schoolbook);
    }

    // Evaluation tests

    #[test]
    fn test_poly_eval_constant() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::constant(field.element(5));
        let x = field.element(7);

        let result = poly.eval(&x);
        assert_eq!(result.value(), 5); // Constant polynomial
    }

    #[test]
    fn test_poly_eval_linear() {
        let field = Gf2mField::new(4, 0b10011);
        // p(x) = 2 + 3x
        let poly = Gf2mPoly::new(vec![field.element(2), field.element(3)]);
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
        let poly = Gf2mPoly::new(vec![field.element(1), field.element(2), field.element(3)]);
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
        let dividend = Gf2mPoly::new(vec![field.element(1), field.element(1), field.element(1)]);
        // divisor: x + 1
        let divisor = Gf2mPoly::new(vec![field.element(1), field.element(1)]);

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
        let dividend = Gf2mPoly::new(vec![field.element(1), field.zero(), field.element(1)]);
        // divisor: x + 1
        let divisor = Gf2mPoly::new(vec![field.element(1), field.element(1)]);

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
        let dividend = Gf2mPoly::new(vec![field.element(2), field.element(4), field.element(6)]);
        let divisor = Gf2mPoly::constant(field.element(2));

        let (quotient, remainder) = dividend.div_rem(&divisor);

        // Dividing by constant: each coefficient divided by constant
        assert_eq!(quotient.degree(), Some(2));
        assert!(remainder.is_zero());
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_poly_div_by_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let dividend = Gf2mPoly::constant(field.element(1));
        let divisor = Gf2mPoly::zero(&field);

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
                        Gf2mPoly::new(vec![field.element(a), field.element(b), field.element(c)]);
                    let divisor = Gf2mPoly::new(vec![field.element(1), field.element(2)]);

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
        let poly = Gf2mPoly::new(vec![field.element(1), field.element(2)]);
        let points: Vec<Gf2mElement> = vec![];
        let results = poly.eval_batch(&points);
        assert!(results.is_empty());
    }

    #[test]
    fn test_poly_eval_batch_single() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::new(vec![field.element(3), field.element(2)]);
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
        let poly = Gf2mPoly::new(vec![field.element(1), field.element(2), field.element(3)]);

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
        let poly = Gf2mPoly::new(vec![field.element(5), field.element(3), field.element(7)]);

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
        let p1 = Gf2mPoly::new(vec![field.element(1), field.element(1)]);
        // p2 = x + 2
        let p2 = Gf2mPoly::new(vec![field.element(2), field.element(1)]);

        let gcd = Gf2mPoly::gcd(&p1, &p2);

        // Coprime polynomials, GCD should be constant (degree 0)
        assert_eq!(gcd.degree(), Some(0));
        assert!(gcd.coeff(0).is_one()); // Monic GCD
    }

    #[test]
    fn test_gcd_common_factor() {
        let field = Gf2mField::new(4, 0b10011);
        // Common factor: (x + 1)
        let common = Gf2mPoly::new(vec![field.element(1), field.element(1)]);

        // p1 = (x + 1)(x + 2)
        let f1 = Gf2mPoly::new(vec![field.element(2), field.element(1)]);
        let p1 = &common * &f1;

        // p2 = (x + 1)(x + 3)
        let f2 = Gf2mPoly::new(vec![field.element(3), field.element(1)]);
        let p2 = &common * &f2;

        let gcd = Gf2mPoly::gcd(&p1, &p2);

        // GCD should be (x + 1) up to scalar multiple
        assert_eq!(gcd.degree(), Some(1));
        assert!(gcd.coeff(1).is_one()); // Monic
    }

    #[test]
    fn test_gcd_identical() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::new(vec![field.element(1), field.element(2), field.element(3)]);

        let gcd = Gf2mPoly::gcd(&poly, &poly);

        // GCD of polynomial with itself is the polynomial (made monic)
        assert_eq!(gcd.degree(), poly.degree());
        assert!(gcd.coeff(gcd.degree().unwrap()).is_one()); // Monic
    }

    #[test]
    fn test_gcd_with_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::new(vec![field.element(1), field.element(2)]);
        let zero = Gf2mPoly::zero(&field);

        let gcd = Gf2mPoly::gcd(&poly, &zero);

        // GCD with zero is the non-zero polynomial (made monic)
        assert_eq!(gcd.degree(), poly.degree());
    }

    // BitVec conversion tests

    #[test]
    fn test_from_bitvec_empty() {
        let field = Gf2mField::new(4, 0b10011);
        let bits = BitVec::new();
        let poly = Gf2mPoly::from_bitvec(&bits, &field);
        assert!(poly.is_zero());
    }

    #[test]
    fn test_from_bitvec_all_zeros() {
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(false);
        bits.push_bit(false);
        bits.push_bit(false);
        let poly = Gf2mPoly::from_bitvec(&bits, &field);
        assert!(poly.is_zero());
    }

    #[test]
    fn test_from_bitvec_simple() {
        let field = Gf2mField::new(4, 0b10011);
        let mut bits = BitVec::new();
        bits.push_bit(true); // x^0
        bits.push_bit(false); // x^1
        bits.push_bit(true); // x^2

        let poly = Gf2mPoly::from_bitvec(&bits, &field);
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

        let poly = Gf2mPoly::from_bitvec(&bits, &field);
        assert_eq!(poly.degree(), Some(4));
        for i in 0..5 {
            assert!(poly.coeff(i).is_one(), "Coefficient {} should be one", i);
        }
    }

    #[test]
    fn test_to_bitvec_zero_polynomial() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::zero(&field);

        let bits = poly.to_bitvec(5);
        assert_eq!(bits.len(), 5);
        for i in 0..5 {
            assert!(!bits.get(i), "Bit {} should be zero", i);
        }
    }

    #[test]
    fn test_to_bitvec_simple() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::new(vec![
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
        let poly = Gf2mPoly::new(vec![
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
        let poly = Gf2mPoly::zero(&field);

        let bits = poly.to_bitvec_minimal();
        assert_eq!(bits.len(), 0);
    }

    #[test]
    fn test_to_bitvec_minimal_degree_two() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::new(vec![
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

        let poly = Gf2mPoly::from_bitvec(&original, &field);
        let recovered = poly.to_bitvec(original.len());

        assert_eq!(original.len(), recovered.len());
        for i in 0..original.len() {
            assert_eq!(original.get(i), recovered.get(i), "Bit {} mismatch", i);
        }
    }

    #[test]
    fn test_roundtrip_poly_to_bitvec_to_poly() {
        let field = Gf2mField::new(4, 0b10011);
        let original = Gf2mPoly::new(vec![
            field.element(1),
            field.element(0),
            field.element(1),
            field.element(0),
            field.element(1),
        ]);

        let bits = original.to_bitvec_minimal();
        let recovered = Gf2mPoly::from_bitvec(&bits, &field);

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
            let p1 = Gf2mPoly::new(vec![field.element(a), field.element(b), field.element(c)]);
            let p2 = Gf2mPoly::new(vec![field.element(d), field.element(e), field.element(f)]);

            prop_assert_eq!(&p1 + &p2, &p2 + &p1);
        }

        #[test]
        fn prop_poly_mul_commutative(a in 1u64..8, b in 1u64..8, c in 1u64..8, d in 1u64..8) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly::new(vec![field.element(a), field.element(b)]);
            let p2 = Gf2mPoly::new(vec![field.element(c), field.element(d)]);

            prop_assert_eq!(&p1 * &p2, &p2 * &p1);
        }

        #[test]
        fn prop_poly_div_rem_invariant(a in 1u64..8, b in 1u64..8, c in 1u64..8, d in 1u64..4) {
            let field = Gf2mField::new(4, 0b10011);
            let dividend = Gf2mPoly::new(vec![field.element(a), field.element(b), field.element(c)]);
            let divisor = Gf2mPoly::new(vec![field.element(d), field.element(1)]);

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
            let p1 = Gf2mPoly::new(vec![field.element(a), field.element(1)]);
            let p2 = Gf2mPoly::new(vec![field.element(b), field.element(1)]);
            let x = field.element(x_val);

            // (p1 + p2)(x) = p1(x) + p2(x)
            let left = (&p1 + &p2).eval(&x);
            let right = &p1.eval(&x) + &p2.eval(&x);

            prop_assert_eq!(left, right);
        }

        #[test]
        fn prop_poly_eval_mul_distributive(a in 1u64..8, b in 1u64..8, x_val in 1u64..16) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly::new(vec![field.element(a), field.element(1)]);
            let p2 = Gf2mPoly::new(vec![field.element(b), field.element(1)]);
            let x = field.element(x_val);

            // (p1 * p2)(x) = p1(x) * p2(x)
            let left = (&p1 * &p2).eval(&x);
            let right = &p1.eval(&x) * &p2.eval(&x);

            prop_assert_eq!(left, right);
        }

        #[test]
        fn prop_gcd_divides_both(a in 1u64..8, b in 1u64..8, c in 1u64..8, d in 1u64..8) {
            let field = Gf2mField::new(4, 0b10011);
            let p1 = Gf2mPoly::new(vec![field.element(a), field.element(b), field.element(1)]);
            let p2 = Gf2mPoly::new(vec![field.element(c), field.element(d), field.element(1)]);

            let gcd = Gf2mPoly::gcd(&p1, &p2);

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

            let p1 = Gf2mPoly::new(coeffs1);
            let p2 = Gf2mPoly::new(coeffs2);

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

            let p1 = Gf2mPoly::new(coeffs1);
            let p2 = Gf2mPoly::new(coeffs2);

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

            let p1 = Gf2mPoly::new(coeffs1);
            let p2 = Gf2mPoly::new(coeffs2);

            let result_karatsuba = p1.mul_karatsuba(&p2);
            let result_schoolbook = p1.mul_schoolbook(&p2);

            prop_assert_eq!(result_karatsuba, result_schoolbook);
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

                let poly = Gf2mPoly::from_bitvec(&bv, &field);
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
                let poly = Gf2mPoly::new(elements);

                let bits = poly.to_bitvec_minimal();

                if let Some(deg) = poly.degree() {
                    prop_assert_eq!(bits.len(), deg + 1);
                } else {
                    prop_assert_eq!(bits.len(), 0);
                }
            }
        }
    }
}
