//! Scalar trait for modem I/Q coordinates and demapper arithmetic.
//!
//! The [`ModemScalar`] trait is a sealed abstraction over `f32` and `f64` that
//! lets constellation storage and demapper math remain generic without
//! opening the trait up to arbitrary external implementations. See the
//! epic design document (`dev/active/d4851c3d-modem-framework/d4851c3d-modem-framework-design.md`)
//! for why a fixed-point flavor is intentionally deferred.

mod sealed {
    /// Sealing trait preventing downstream implementations of
    /// [`super::ModemScalar`].
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// Scalar used for constellation I/Q coordinates and demapper math.
///
/// This trait is **sealed**: only [`f32`] and [`f64`] implement it. The
/// sealing keeps specialization tractable and preserves the option to add
/// fixed-point or half-precision flavors later as a non-breaking change.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::ModemScalar;
///
/// fn midpoint<S: ModemScalar>(a: S, b: S) -> S {
///     let half = S::one() / S::two();
///     (a + b).mul_add(half, S::zero())
/// }
/// assert!((midpoint(0.0_f32, 2.0_f32) - 1.0_f32).abs() < 1e-6);
/// assert!((midpoint(0.0_f64, 2.0_f64) - 1.0_f64).abs() < 1e-12);
/// ```
pub trait ModemScalar:
    sealed::Sealed
    + Copy
    + PartialOrd
    + core::fmt::Debug
    + core::ops::Add<Output = Self>
    + core::ops::Sub<Output = Self>
    + core::ops::Mul<Output = Self>
    + core::ops::Div<Output = Self>
    + core::ops::Neg<Output = Self>
    + 'static
{
    /// Additive identity.
    fn zero() -> Self;
    /// Multiplicative identity.
    fn one() -> Self;
    /// The value `2`.
    fn two() -> Self;
    /// Lossless or rounding conversion from [`f64`].
    fn from_f64(v: f64) -> Self;
    /// Conversion to `f32` for producing [`crate::Llr`] values.
    fn to_f32(self) -> f32;
    /// Lossless widening conversion to `f64`.
    ///
    /// For `f64` this is the identity; for `f32` this preserves the exact
    /// value. Builders that compute normalization in `f64` use this to
    /// avoid the precision loss of routing through `f32`.
    fn to_f64(self) -> f64;
    /// Square root.
    fn sqrt(self) -> Self;
    /// Absolute value.
    fn abs(self) -> Self;
    /// Fused multiply-add `self * a + b`.
    fn mul_add(self, a: Self, b: Self) -> Self;
    /// Natural exponential.
    fn exp(self) -> Self;
    /// Natural logarithm.
    fn ln(self) -> Self;
    /// Minimum of `self` and `other`.
    fn min(self, other: Self) -> Self;
    /// Maximum of `self` and `other`.
    fn max(self, other: Self) -> Self;

    /// Tolerance used by [`super::ModemSpec`] when validating the
    /// post-normalization mean symbol energy equals `1`.
    ///
    /// Set to `1e-5` for `f32` and `1e-10` for `f64`.
    #[doc(hidden)]
    fn unit_energy_tolerance() -> Self;
}

impl ModemScalar for f32 {
    #[inline]
    fn zero() -> Self {
        0.0
    }
    #[inline]
    fn one() -> Self {
        1.0
    }
    #[inline]
    fn two() -> Self {
        2.0
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        v as f32
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }
    #[inline]
    fn sqrt(self) -> Self {
        f32::sqrt(self)
    }
    #[inline]
    fn abs(self) -> Self {
        f32::abs(self)
    }
    #[inline]
    fn mul_add(self, a: Self, b: Self) -> Self {
        f32::mul_add(self, a, b)
    }
    #[inline]
    fn exp(self) -> Self {
        f32::exp(self)
    }
    #[inline]
    fn ln(self) -> Self {
        f32::ln(self)
    }
    #[inline]
    fn min(self, other: Self) -> Self {
        f32::min(self, other)
    }
    #[inline]
    fn max(self, other: Self) -> Self {
        f32::max(self, other)
    }
    #[inline]
    fn unit_energy_tolerance() -> Self {
        1.0e-5
    }
}

impl ModemScalar for f64 {
    #[inline]
    fn zero() -> Self {
        0.0
    }
    #[inline]
    fn one() -> Self {
        1.0
    }
    #[inline]
    fn two() -> Self {
        2.0
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        v
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    #[inline]
    fn abs(self) -> Self {
        f64::abs(self)
    }
    #[inline]
    fn mul_add(self, a: Self, b: Self) -> Self {
        f64::mul_add(self, a, b)
    }
    #[inline]
    fn exp(self) -> Self {
        f64::exp(self)
    }
    #[inline]
    fn ln(self) -> Self {
        f64::ln(self)
    }
    #[inline]
    fn min(self, other: Self) -> Self {
        f64::min(self, other)
    }
    #[inline]
    fn max(self, other: Self) -> Self {
        f64::max(self, other)
    }
    #[inline]
    fn unit_energy_tolerance() -> Self {
        1.0e-10
    }
}

/// Default scalar for modem presets and most downstream code.
///
/// Kept at [`f32`] to match [`crate::Llr`] and maximize SIMD lane density.
pub type DefaultScalar = f32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_constants_f32() {
        assert_eq!(<f32 as ModemScalar>::zero(), 0.0);
        assert_eq!(<f32 as ModemScalar>::one(), 1.0);
        assert_eq!(<f32 as ModemScalar>::two(), 2.0);
    }

    #[test]
    fn test_scalar_constants_f64() {
        assert_eq!(<f64 as ModemScalar>::zero(), 0.0);
        assert_eq!(<f64 as ModemScalar>::one(), 1.0);
        assert_eq!(<f64 as ModemScalar>::two(), 2.0);
    }

    #[test]
    fn test_scalar_from_f64_roundtrip() {
        assert!((<f32 as ModemScalar>::from_f64(0.25) - 0.25_f32).abs() < 1e-7);
        assert!((<f64 as ModemScalar>::from_f64(0.25) - 0.25_f64).abs() < 1e-15);
    }

    #[test]
    fn test_scalar_sqrt_and_abs() {
        assert!((<f32 as ModemScalar>::sqrt(4.0) - 2.0).abs() < 1e-6);
        assert!((<f64 as ModemScalar>::sqrt(9.0) - 3.0).abs() < 1e-12);
        assert_eq!(<f32 as ModemScalar>::abs(-1.5_f32), 1.5_f32);
        assert_eq!(<f64 as ModemScalar>::abs(-2.5_f64), 2.5_f64);
    }
}
