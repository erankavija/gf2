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
use std::ops::{Add, Mul};
use std::rc::Rc;

/// A binary extension field GF(2^m) with a specified primitive polynomial.
///
/// This type defines the field structure and parameters. Individual field elements
/// are created via [`Gf2mField::element`].
#[derive(Clone, Debug)]
pub struct Gf2mField {
    params: Rc<FieldParams>,
}

#[derive(Debug, PartialEq, Eq)]
struct FieldParams {
    m: usize,
    primitive_poly: u64,
}

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
            params: Rc::new(FieldParams { m, primitive_poly }),
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

        let m = self.params.m;
        let primitive_poly = self.params.primitive_poly;

        // Schoolbook polynomial multiplication with reduction
        let mut result = 0u64;
        let mut temp = self.value;

        for i in 0..m {
            // If bit i of rhs is set, add temp to result
            if (rhs.value >> i) & 1 == 1 {
                result ^= temp;
            }

            // Multiply temp by x (shift left)
            let will_overflow = (temp & (1u64 << (m - 1))) != 0;
            temp <<= 1;

            // If we would overflow (x^m term created), reduce by primitive polynomial
            if will_overflow {
                // x^m ≡ primitive_poly (mod primitive_poly)
                // Since primitive_poly = x^m + lower_terms, we XOR with lower_terms
                temp ^= primitive_poly;
            }
        }

        Gf2mElement {
            value: result & ((1u64 << m) - 1), // Mask to m bits
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
}
