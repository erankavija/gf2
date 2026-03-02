//! GF(p) — Prime Field Arithmetic
//!
//! Arithmetic over prime fields GF(p) using a const-generic representation.
//! Elements are integers modulo a prime `P`, with standard modular arithmetic.
//!
//! # Examples
//!
//! ```
//! use gf2_core::gfp::Fp;
//! use gf2_core::field::{FiniteField, ConstField, FiniteFieldExt};
//!
//! // GF(7): integers {0, 1, 2, 3, 4, 5, 6} with arithmetic mod 7
//! let a = Fp::<7>::new(3);
//! let b = Fp::<7>::new(5);
//!
//! // Addition: (3 + 5) mod 7 = 1
//! assert_eq!(a + b, Fp::<7>::new(1));
//!
//! // Multiplication: (3 * 5) mod 7 = 1, so 3 and 5 are inverses
//! assert_eq!(a * b, Fp::<7>::new(1));
//! assert_eq!(a.inv(), Some(b));
//!
//! // ConstField provides zero-argument constructors
//! assert!(Fp::<7>::zero().is_zero());
//! assert!(Fp::<7>::one().is_one());
//! ```

use std::fmt;
use std::hash::Hash;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

use crate::field::{ConstField, FiniteField};

/// An element of the prime field GF(P) for a compile-time-known prime `P`.
///
/// Elements are integers in `[0, P)` with arithmetic modulo `P`. The type
/// parameter `P` must be prime with `1 < P <= 2^63`; primality is not checked
/// at compile time but is required for correctness.
///
/// This implementation uses naive modular reduction (`%`). Montgomery
/// multiplication provides faster arithmetic and is tracked separately.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::Fp;
/// use gf2_core::field::{FiniteField, FiniteFieldExt};
///
/// let a = Fp::<5>::new(3);
/// let b = Fp::<5>::new(4);
///
/// // Fermat's little theorem: a^(p-1) = 1 for non-zero a
/// assert!(a.pow(4).is_one());
///
/// // Additive inverse: -3 mod 5 = 2
/// assert_eq!(-a, Fp::<5>::new(2));
/// ```
///
/// # Panics
///
/// Construction panics (via const assertion) if `P < 2` or `P > 2^63`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Fp<const P: u64>(u64);

impl<const P: u64> Fp<P> {
    /// Compile-time validation that P is valid.
    const VALIDATED: () = {
        assert!(P > 1, "Fp<P>: P must be > 1");
        assert!(
            P <= (1u64 << 63),
            "Fp<P>: P must be <= 2^63 for overflow-safe addition"
        );
    };

    /// Creates a new element from a representative value, reduced modulo `P`.
    ///
    /// # Arguments
    ///
    /// * `value` - Any `u64`; will be reduced to `[0, P)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = Fp::<7>::new(10); // 10 mod 7 = 3
    /// assert_eq!(a.value(), 3);
    /// ```
    #[inline]
    pub fn new(value: u64) -> Self {
        #[allow(clippy::let_unit_value)]
        let _ = Self::VALIDATED;
        Self(value % P)
    }

    /// Returns the inner representative value in `[0, P)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    ///
    /// assert_eq!(Fp::<7>::new(3).value(), 3);
    /// ```
    #[inline]
    pub fn value(self) -> u64 {
        self.0
    }
}

impl<const P: u64> fmt::Display for Fp<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Arithmetic operators
// ---------------------------------------------------------------------------

impl<const P: u64> Add for Fp<P> {
    type Output = Self;

    /// Modular addition with conditional correction.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn add(self, rhs: Self) -> Self {
        let sum = self.0 + rhs.0;
        if sum >= P {
            Self(sum - P)
        } else {
            Self(sum)
        }
    }
}

impl<const P: u64> Sub for Fp<P> {
    type Output = Self;

    /// Modular subtraction.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            Self(self.0 - rhs.0)
        } else {
            Self(self.0 + P - rhs.0)
        }
    }
}

impl<const P: u64> Mul for Fp<P> {
    type Output = Self;

    /// Modular multiplication using u128 intermediate.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self(((self.0 as u128 * rhs.0 as u128) % P as u128) as u64)
    }
}

impl<const P: u64> Neg for Fp<P> {
    type Output = Self;

    /// Additive inverse: `-a = P - a` for non-zero, `0` for zero.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    fn neg(self) -> Self {
        if self.0 == 0 {
            self
        } else {
            Self(P - self.0)
        }
    }
}

/// Modular exponentiation via square-and-multiply.
///
/// # Complexity
///
/// O(log exp) multiplications.
fn mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    if modulus == 1 {
        return 0;
    }
    let mut result = 1u64;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = ((result as u128 * base as u128) % modulus as u128) as u64;
        }
        exp >>= 1;
        if exp > 0 {
            base = ((base as u128 * base as u128) % modulus as u128) as u64;
        }
    }
    result
}

impl<const P: u64> Div for Fp<P> {
    type Output = Self;

    /// Division via multiplicative inverse.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    ///
    /// # Complexity
    ///
    /// O(log P) (inverse via Fermat's little theorem).
    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inv().expect("division by zero in Fp")
    }
}

// ---------------------------------------------------------------------------
// AddAssign
// ---------------------------------------------------------------------------

impl<const P: u64> AddAssign for Fp<P> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<const P: u64> AddAssign<&Self> for Fp<P> {
    #[inline]
    fn add_assign(&mut self, rhs: &Self) {
        *self = *self + *rhs;
    }
}

// ---------------------------------------------------------------------------
// Reference-forwarding operators (Fp is Copy, so dereference and delegate)
// ---------------------------------------------------------------------------

impl<const P: u64> Add<&Fp<P>> for Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn add(self, rhs: &Fp<P>) -> Fp<P> {
        self + *rhs
    }
}

impl<const P: u64> Add for &Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn add(self, rhs: Self) -> Fp<P> {
        *self + *rhs
    }
}

impl<const P: u64> Sub<&Fp<P>> for Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn sub(self, rhs: &Fp<P>) -> Fp<P> {
        self - *rhs
    }
}

impl<const P: u64> Sub for &Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn sub(self, rhs: Self) -> Fp<P> {
        *self - *rhs
    }
}

impl<const P: u64> Mul<&Fp<P>> for Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn mul(self, rhs: &Fp<P>) -> Fp<P> {
        self * *rhs
    }
}

impl<const P: u64> Mul for &Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn mul(self, rhs: Self) -> Fp<P> {
        *self * *rhs
    }
}

impl<const P: u64> Div<&Fp<P>> for Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn div(self, rhs: &Fp<P>) -> Fp<P> {
        self / *rhs
    }
}

impl<const P: u64> Div for &Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn div(self, rhs: Self) -> Fp<P> {
        *self / *rhs
    }
}

impl<const P: u64> Neg for &Fp<P> {
    type Output = Fp<P>;
    #[inline]
    fn neg(self) -> Fp<P> {
        -(*self)
    }
}

// ---------------------------------------------------------------------------
// FiniteField implementation
// ---------------------------------------------------------------------------

impl<const P: u64> FiniteField for Fp<P> {
    type Characteristic = u64;
    type Wide = u128;

    #[inline]
    fn characteristic(&self) -> u64 {
        P
    }

    #[inline]
    fn extension_degree(&self) -> usize {
        1
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.0 == 1
    }

    /// Multiplicative inverse via Fermat's little theorem: `a^(P-2) mod P`.
    ///
    /// # Complexity
    ///
    /// O(log P) multiplications.
    fn inv(&self) -> Option<Self> {
        if self.0 == 0 {
            None
        } else {
            Some(Self(mod_pow(self.0, P - 2, P)))
        }
    }

    #[inline]
    fn zero_like(&self) -> Self {
        Self(0)
    }

    #[inline]
    fn one_like(&self) -> Self {
        Self(1)
    }

    #[inline]
    fn to_wide(&self) -> u128 {
        self.0 as u128
    }

    #[inline]
    fn mul_to_wide(&self, rhs: &Self) -> u128 {
        self.0 as u128 * rhs.0 as u128
    }

    #[inline]
    fn reduce_wide(wide: &u128) -> Self {
        Self((*wide % P as u128) as u64)
    }

    fn max_unreduced_additions() -> usize {
        let max_product = (P as u128 - 1) * (P as u128 - 1);
        if max_product == 0 {
            return usize::MAX;
        }
        let k = u128::MAX / max_product;
        if k > usize::MAX as u128 {
            usize::MAX
        } else {
            k as usize
        }
    }
}

// ---------------------------------------------------------------------------
// ConstField implementation
// ---------------------------------------------------------------------------

impl<const P: u64> ConstField for Fp<P> {
    #[inline]
    fn zero() -> Self {
        #[allow(clippy::let_unit_value)]
        let _ = Self::VALIDATED;
        Self(0)
    }

    #[inline]
    fn one() -> Self {
        #[allow(clippy::let_unit_value)]
        let _ = Self::VALIDATED;
        Self(1)
    }

    #[inline]
    fn order() -> u128 {
        P as u128
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{ConstField, FiniteField, FiniteFieldExt};

    // --- Step 1: Construction ---

    #[test]
    fn test_new_reduces_mod_p() {
        assert_eq!(Fp::<7>::new(10).value(), 3);
        assert_eq!(Fp::<7>::new(7).value(), 0);
        assert_eq!(Fp::<7>::new(0).value(), 0);
        assert_eq!(Fp::<7>::new(6).value(), 6);
    }

    #[test]
    fn test_zero_and_one() {
        assert!(Fp::<5>::zero().is_zero());
        assert!(Fp::<5>::one().is_one());
        assert!(!Fp::<5>::zero().is_one());
        assert!(!Fp::<5>::one().is_zero());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Fp::<7>::new(5)), "5");
        assert_eq!(format!("{}", Fp::<7>::new(0)), "0");
    }

    // --- Step 2: Basic arithmetic (GF(7)) ---

    #[test]
    fn test_add_gf7() {
        assert_eq!(Fp::<7>::new(3) + Fp::<7>::new(5), Fp::<7>::new(1)); // 8 mod 7
        assert_eq!(Fp::<7>::new(0) + Fp::<7>::new(4), Fp::<7>::new(4));
        assert_eq!(Fp::<7>::new(6) + Fp::<7>::new(6), Fp::<7>::new(5)); // 12 mod 7
    }

    #[test]
    fn test_sub_gf7() {
        assert_eq!(Fp::<7>::new(5) - Fp::<7>::new(3), Fp::<7>::new(2));
        assert_eq!(Fp::<7>::new(3) - Fp::<7>::new(5), Fp::<7>::new(5)); // -2 mod 7
        assert_eq!(Fp::<7>::new(0) - Fp::<7>::new(1), Fp::<7>::new(6));
    }

    #[test]
    fn test_mul_gf7() {
        assert_eq!(Fp::<7>::new(3) * Fp::<7>::new(5), Fp::<7>::new(1)); // 15 mod 7
        assert_eq!(Fp::<7>::new(4) * Fp::<7>::new(0), Fp::<7>::new(0));
        assert_eq!(Fp::<7>::new(6) * Fp::<7>::new(6), Fp::<7>::new(1)); // 36 mod 7
    }

    #[test]
    fn test_neg_gf7() {
        assert_eq!(-Fp::<7>::new(3), Fp::<7>::new(4));
        assert_eq!(-Fp::<7>::new(0), Fp::<7>::new(0));
        assert_eq!(-Fp::<7>::new(1), Fp::<7>::new(6));
    }

    #[test]
    fn test_fp2_is_gf2() {
        type F2 = Fp<2>;
        assert_eq!(F2::new(0) + F2::new(0), F2::new(0));
        assert_eq!(F2::new(1) + F2::new(1), F2::new(0)); // 1+1=0 in char 2
        assert_eq!(F2::new(1) * F2::new(1), F2::new(1));
        assert_eq!(F2::new(1).inv(), Some(F2::new(1)));
    }

    // --- Step 3: Inversion + division ---

    #[test]
    fn test_mod_pow() {
        assert_eq!(mod_pow(3, 0, 7), 1);
        assert_eq!(mod_pow(3, 1, 7), 3);
        assert_eq!(mod_pow(3, 5, 7), 5); // 243 mod 7 = 5
        assert_eq!(mod_pow(2, 10, 1000), 24); // 1024 mod 1000
    }

    #[test]
    fn test_inv_gf7() {
        assert_eq!(Fp::<7>::new(3).inv(), Some(Fp::<7>::new(5))); // 3 * 5 = 15 ≡ 1
        assert_eq!(Fp::<7>::new(0).inv(), None);
        assert_eq!(Fp::<7>::new(1).inv(), Some(Fp::<7>::new(1)));
    }

    #[test]
    fn test_div_gf7() {
        // 6 / 3 = 6 * inv(3) = 6 * 5 = 30 mod 7 = 2
        assert_eq!(Fp::<7>::new(6) / Fp::<7>::new(3), Fp::<7>::new(2));
    }

    #[test]
    fn test_fermat_little_theorem() {
        for v in 1..7u64 {
            assert!(Fp::<7>::new(v).pow(6).is_one());
        }
    }

    // --- Step 4: Reference ops + AddAssign ---

    #[test]
    #[allow(clippy::op_ref)]
    fn test_ref_operators() {
        let a = Fp::<7>::new(3);
        let b = Fp::<7>::new(5);
        assert_eq!(a + b, Fp::<7>::new(1));
        assert_eq!(&a + &b, Fp::<7>::new(1));
        assert_eq!(a + &b, Fp::<7>::new(1));
        assert_eq!(&a - &b, Fp::<7>::new(5));
        assert_eq!(&a * &b, Fp::<7>::new(1));
        assert_eq!(&a / &b, a * b.inv().unwrap());
        assert_eq!(-&a, Fp::<7>::new(4));
    }

    #[test]
    fn test_add_assign() {
        let mut a = Fp::<7>::new(3);
        a += Fp::<7>::new(5);
        assert_eq!(a, Fp::<7>::new(1));
    }

    #[test]
    fn test_add_assign_ref() {
        let mut a = Fp::<7>::new(3);
        let b = Fp::<7>::new(5);
        a += &b;
        assert_eq!(a, Fp::<7>::new(1));
        assert_eq!(b.value(), 5); // b still valid
    }

    // --- Step 5: FiniteField trait ---

    #[test]
    fn test_characteristic_and_extension() {
        let a = Fp::<5>::new(3);
        assert_eq!(a.characteristic(), 5u64);
        assert_eq!(a.extension_degree(), 1);
    }

    #[test]
    fn test_wide_accumulator() {
        let a = Fp::<7>::new(5);
        let wide = a.to_wide();
        assert_eq!(wide, 5u128);
        let back = Fp::<7>::reduce_wide(&wide);
        assert_eq!(back, a);
    }

    #[test]
    fn test_mul_to_wide() {
        let a = Fp::<7>::new(5);
        let b = Fp::<7>::new(6);
        let wide = a.mul_to_wide(&b);
        assert_eq!(wide, 30u128); // unreduced
        assert_eq!(Fp::<7>::reduce_wide(&wide), Fp::<7>::new(2)); // 30 mod 7
    }

    #[test]
    fn test_max_unreduced_additions() {
        let k = Fp::<7>::max_unreduced_additions();
        assert!(k > 0);
        let max_product = 6u128 * 6u128;
        assert!(k as u128 <= u128::MAX / max_product);
    }

    #[test]
    fn test_mersenne_61_basic() {
        type M61 = Fp<2305843009213693951>;
        let a = M61::new(123456789);
        let b = M61::new(987654321);
        let sum = a + b;
        assert_eq!(sum.value(), 123456789 + 987654321);

        let inv = a.inv().unwrap();
        assert!((a * inv).is_one());
    }

    // --- Step 6: ConstField ---

    #[test]
    fn test_const_field_order() {
        assert_eq!(Fp::<7>::order(), 7u128);
        assert_eq!(Fp::<65537>::order(), 65537u128);
    }
}
