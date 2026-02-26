//! Trait hierarchy for finite field arithmetic.
//!
//! This module defines a generic abstraction for finite fields that supports:
//! - Binary extension fields GF(2^m) via [`Gf2mElement`](crate::gf2m::Gf2mElement)
//! - Prime fields GF(p) (future)
//! - Tower extension fields GF(p^n) (future)
//!
//! # Trait Overview
//!
//! - [`FiniteField`]: Core trait with arithmetic, identity elements, and wide accumulation.
//! - [`ConstField`]: Extension for fields whose elements are `Copy` and have const-like constructors.
//! - [`FiniteFieldExt`]: Blanket-implemented convenience methods (`square`, `pow`, `frobenius`).

use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// Core trait for finite field elements.
///
/// Provides arithmetic operations, identity elements, and a wide accumulator type
/// for delayed-reduction dot products.
///
/// # Associated Types
///
/// - `Characteristic`: The field characteristic (e.g., `u64` for small primes).
/// - `Wide`: A wider accumulator type that can hold sums of products before reduction.
///
/// # Examples
///
/// ```
/// use gf2_core::field::FiniteField;
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let a = field.element(5);
/// let b = field.element(3);
///
/// assert!(!a.is_zero());
/// assert!(field.zero().is_zero());
///
/// let inv = a.inv().expect("non-zero element has inverse");
/// assert!((a * inv).is_one());
/// ```
pub trait FiniteField:
    Sized
    + Clone
    + PartialEq
    + Eq
    + Hash
    + Debug
    + Add<Output = Self>
    + for<'a> Add<&'a Self, Output = Self>
    + Sub<Output = Self>
    + for<'a> Sub<&'a Self, Output = Self>
    + Mul<Output = Self>
    + for<'a> Mul<&'a Self, Output = Self>
    + Div<Output = Self>
    + for<'a> Div<&'a Self, Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + for<'a> AddAssign<&'a Self>
{
    /// The field characteristic (prime p such that p·1 = 0).
    type Characteristic: Clone + Debug + PartialEq + Eq;

    /// A wider type for accumulating sums of products without intermediate reduction.
    ///
    /// For binary fields, `Wide = Self` since XOR never overflows.
    /// For prime fields, this is typically a double-width integer (e.g., `u128` for `u64` elements).
    type Wide: Clone + Add<Output = Self::Wide> + AddAssign;

    /// Returns the field characteristic.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let a = Gf2mField::new(4, 0b10011).element(5);
    /// assert_eq!(a.characteristic(), 2u64);
    /// ```
    fn characteristic(&self) -> Self::Characteristic;

    /// Returns the extension degree [F : F_p].
    ///
    /// For a prime field GF(p), this returns 1.
    /// For GF(p^m), this returns m.
    ///
    /// # Panics
    ///
    /// May panic if the extension degree is not statically known (e.g., runtime-configured fields).
    fn extension_degree(&self) -> usize;

    /// Returns `true` if this element is the additive identity (zero).
    fn is_zero(&self) -> bool;

    /// Returns `true` if this element is the multiplicative identity (one).
    fn is_one(&self) -> bool;

    /// Computes the multiplicative inverse, or `None` if this element is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(7);
    /// let inv = a.inv().unwrap();
    /// assert!((a * inv).is_one());
    /// ```
    fn inv(&self) -> Option<Self>;

    /// Returns the additive identity (zero) in the same field as `self`.
    fn zero_like(&self) -> Self;

    /// Returns the multiplicative identity (one) in the same field as `self`.
    fn one_like(&self) -> Self;

    /// Converts this element to the wide accumulator type.
    fn to_wide(&self) -> Self::Wide;

    /// Multiplies two elements and returns the result in the wide type (before reduction).
    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide;

    /// Reduces a wide accumulator back to a field element.
    fn reduce_wide(wide: &Self::Wide) -> Self;

    /// Maximum number of wide-type additions before reduction is required to avoid overflow.
    ///
    /// Returns `usize::MAX` if overflow is impossible (e.g., binary fields where addition is XOR).
    fn max_unreduced_additions() -> usize;
}

/// Extension of [`FiniteField`] for types that are `Copy` and have zero-cost identity constructors.
///
/// This is appropriate for fields with compile-time-known parameters (const generics or
/// zero-sized config types), where elements don't carry runtime field context.
pub trait ConstField: FiniteField + Copy {
    /// Returns the additive identity (zero).
    fn zero() -> Self;

    /// Returns the multiplicative identity (one).
    fn one() -> Self;

    /// Returns the number of elements in the field.
    fn order() -> u64;
}

/// Blanket-implemented convenience methods for all [`FiniteField`] types.
///
/// Provides `square`, `pow`, and `frobenius` built on top of the core trait.
pub trait FiniteFieldExt: FiniteField {
    /// Computes `self * self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(5);
    /// let sq = a.square();
    /// assert_eq!(sq, a.clone() * a);
    /// ```
    fn square(&self) -> Self {
        self.clone() * self.clone()
    }

    /// Computes `self^exp` using square-and-multiply.
    ///
    /// # Complexity
    ///
    /// O(log exp) field multiplications.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(6);
    /// // Fermat's little theorem: a^(2^4 - 1) = 1 for non-zero a
    /// assert!(a.pow(15).is_one());
    /// ```
    fn pow(&self, exp: u64) -> Self {
        if exp == 0 {
            return self.one_like();
        }

        let mut result = self.one_like();
        let mut base = self.clone();
        let mut e = exp;

        while e > 0 {
            if e & 1 == 1 {
                result = result * base.clone();
            }
            e >>= 1;
            if e > 0 {
                base = base.clone() * base.clone();
            }
        }

        result
    }

    /// Computes the k-th iterated Frobenius endomorphism: `self^(p^k)`.
    ///
    /// The Frobenius map φ: x → x^p is a field automorphism of GF(p^m).
    /// This computes φ^k(x) = x^(p^k).
    ///
    /// # Arguments
    ///
    /// * `k` - Number of Frobenius iterations.
    ///
    /// # Panics
    ///
    /// Panics if the characteristic cannot be converted to `u64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(5);
    /// // In GF(2^4): frobenius(a, 1) = a^2
    /// assert_eq!(a.frobenius(1), a.square());
    /// ```
    fn frobenius(&self, k: usize) -> Self
    where
        Self::Characteristic: Into<u64>,
    {
        let p: u64 = self.characteristic().into();
        // Compute p^k as exponent
        let mut exp = 1u64;
        for _ in 0..k {
            exp = exp.checked_mul(p).expect("Frobenius exponent overflow");
        }
        self.pow(exp)
    }
}

// Blanket implementation: every FiniteField automatically gets FiniteFieldExt
impl<T: FiniteField> FiniteFieldExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::Gf2mField;

    // --- Generic function test proving trait usability ---

    fn generic_field_test<F: FiniteField>(a: F, b: F) {
        // Commutativity: a + b == b + a
        assert_eq!(a.clone() + b.clone(), b.clone() + a.clone());
        // Commutativity: a * b == b * a
        assert_eq!(a.clone() * b.clone(), b.clone() * a.clone());

        // Additive identity
        let zero = a.zero_like();
        assert_eq!(a.clone() + zero.clone(), a);

        // Multiplicative identity
        let one = a.one_like();
        assert_eq!(a.clone() * one.clone(), a);

        // Subtraction (additive inverse)
        assert!((a.clone() - a.clone()).is_zero());

        // Multiplicative inverse (if non-zero)
        if !a.is_zero() {
            let inv = a.inv().expect("non-zero element has inverse");
            assert!((a.clone() * inv).is_one());
        }

        // Zero has no inverse
        assert!(zero.inv().is_none());
    }

    #[test]
    fn test_generic_field_gf16() {
        let field = Gf2mField::new(4, 0b10011);
        generic_field_test(field.element(5), field.element(3));
        generic_field_test(field.element(0), field.element(7));
        generic_field_test(field.element(1), field.element(15));
    }

    #[test]
    fn test_generic_field_gf256() {
        let field = Gf2mField::gf256();
        generic_field_test(field.element(0x53), field.element(0xCA));
    }

    // --- FiniteFieldExt: square() ---
    // SageMath: GF(2^4, 'a', modulus=x^4+x+1)

    #[test]
    fn test_square_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        // square(5) = 2: a^2+1 squared = a^2+a+1 ... SageMath says 2
        assert_eq!(field.element(5).square(), field.element(2));
        // square(10) = 8
        assert_eq!(field.element(10).square(), field.element(8));
    }

    // --- FiniteFieldExt: pow() ---

    #[test]
    fn test_pow_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        // pow(3, 5) = 6
        assert_eq!(field.element(3).pow(5), field.element(6));
        // pow(7, 10) = 7
        assert_eq!(field.element(7).pow(10), field.element(7));
        // pow(9, 13) = 4
        assert_eq!(field.element(9).pow(13), field.element(4));
        // pow(13, 4) = 11
        assert_eq!(field.element(13).pow(4), field.element(11));
        // Fermat: pow(6, 15) = 1
        assert_eq!(field.element(6).pow(15), field.element(1));
        // pow(a, 0) = 1 for any non-zero a
        assert_eq!(field.element(5).pow(0), field.element(1));
        assert_eq!(field.element(1).pow(0), field.element(1));
    }

    // --- FiniteFieldExt: frobenius() ---

    #[test]
    fn test_frobenius_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        // frobenius(5, 1) = 5^2 = 2
        assert_eq!(field.element(5).frobenius(1), field.element(2));
        // frobenius(5, 2) = 5^4 = 4
        assert_eq!(field.element(5).frobenius(2), field.element(4));
        // frobenius(7, 1) = 7^2 = 6
        assert_eq!(field.element(7).frobenius(1), field.element(6));
        // frobenius(10, 1) = 10^2 = 8
        assert_eq!(field.element(10).frobenius(1), field.element(8));
    }

    // --- GF(2^8) with polynomial x^8+x^4+x^3+x^2+1 (0b100011101) ---

    #[test]
    fn test_gf256_inv() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        // Verify a * inv(a) = 1
        let inv = a.inv().unwrap();
        assert!((a * inv).is_one());
    }

    #[test]
    fn test_gf256_pow() {
        let field = Gf2mField::gf256();
        // Fermat: a^255 = 1 for any non-zero a
        assert!(field.element(0x53).pow(255).is_one());
        // pow consistency: a^7 = a * a^2 * a^4
        let a = field.element(0x53);
        let a2 = a.square();
        let a4 = a2.square();
        assert_eq!(a.pow(7), a.clone() * a2 * a4);
    }

    #[test]
    fn test_gf256_square() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        // square(a) = a * a
        assert_eq!(a.square(), a.clone() * a);
    }

    // --- Trait method consistency ---

    #[test]
    fn test_characteristic_and_extension() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(5);
        assert_eq!(a.characteristic(), 2u64);
        assert_eq!(a.extension_degree(), 4);
    }

    #[test]
    fn test_wide_roundtrip() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(7);
        let wide = a.to_wide();
        let back = <crate::gf2m::Gf2mElement as FiniteField>::reduce_wide(&wide);
        assert_eq!(back, a);
    }

    #[test]
    fn test_addassign() {
        let field = Gf2mField::new(4, 0b10011);
        let mut a = field.element(5);
        let b = field.element(3);
        a += b;
        assert_eq!(a, field.element(5 ^ 3));
    }

    #[test]
    fn test_addassign_ref() {
        let field = Gf2mField::new(4, 0b10011);
        let mut a = field.element(5);
        let b = field.element(3);
        a += &b;
        assert_eq!(a, field.element(5 ^ 3));
        // b is still valid
        assert_eq!(b.value(), 3);
    }

    #[test]
    fn test_mixed_receiver_ops() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(5);
        let b = field.element(3);

        // owned + &ref
        assert_eq!(a.clone() + &b, &a + &b);
        assert_eq!(a.clone() - &b, &a - &b);
        assert_eq!(a.clone() * &b, &a * &b);
        assert_eq!(a.clone() / &b, &a / &b);
    }
}
