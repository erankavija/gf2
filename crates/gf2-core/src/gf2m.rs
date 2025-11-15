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
        Gf2mField {
            params: Rc::new(FieldParams {
                m,
                primitive_poly,
                log_table: None,
                exp_table: None,
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

        Gf2mField {
            params: Rc::new(FieldParams {
                m: self.params.m,
                primitive_poly: self.params.primitive_poly,
                log_table: Some(log_table),
                exp_table: Some(exp_table),
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

        // Use table-based multiplication if available
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

        // Fallback to schoolbook multiplication
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

        // Schoolbook multiplication
        for i in 0..=deg_self {
            for j in 0..=deg_rhs {
                let term = &self.coeffs[i] * &rhs.coeffs[j];
                coeffs[i + j] = &coeffs[i + j] + &term;
            }
        }

        Gf2mPoly::new(coeffs)
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
    }
}
