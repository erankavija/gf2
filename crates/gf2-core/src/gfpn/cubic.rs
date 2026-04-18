//! Cubic extension field arithmetic: elements `c0 + c1·v + c2·v²` where `v³ = β`.
//!
//! [`CubicExt<C>`] implements a degree-3 extension of any base field that
//! implements [`ConstField`], parameterized by an [`ExtConfig`] specifying the
//! non-residue β.
//!
//! # Multiplication
//!
//! Uses the Karatsuba-style 6-mul formula (6 base-field multiplications instead
//! of 9 schoolbook):
//!
//! ```text
//! v0 = a0·b0,  v1 = a1·b1,  v2 = a2·b2
//! x  = (a1+a2)(b1+b2) − v1 − v2          // a1·b2 + a2·b1
//! y  = (a0+a1)(b0+b1) − v0 − v1          // a0·b1 + a1·b0
//! z  = (a0+a2)(b0+b2) − v0 + v1 − v2     // a0·b2 + a1·b1 + a2·b0
//! c0 = v0 + β·x
//! c1 = y  + β·v2
//! c2 = z
//! ```
//!
//! Reference: Devegili, O hEigeartaigh, Scott, Dahab (ePrint 2006/471).
//!
//! # Inversion
//!
//! Uses the adjugate/norm method: compute cofactors s0, s1, s2, then
//! `a⁻¹ = (s0, s1, s2) / norm(a)` with a single base-field inversion.
//!
//! Reference: Beuchat et al. (ePrint 2010/354).
//!
//! # Examples
//!
//! ```
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::{ExtConfig, CubicExt};
//! use gf2_core::field::{FiniteField, ConstField};
//!
//! struct Fq3Config;
//! impl ExtConfig for Fq3Config {
//!     type BaseField = Fp<7>;
//!     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3); // β = 3 (cubic non-residue mod 7)
//! }
//! type Fq3 = CubicExt<Fq3Config>;
//!
//! let a = Fq3::new(Fp::new(3), Fp::new(5), Fp::new(2));
//! assert_eq!(a.c0().value(), 3);
//! assert_eq!(a.c1().value(), 5);
//! assert_eq!(a.c2().value(), 2);
//!
//! // Field axioms hold
//! assert!(Fq3::zero().is_zero());
//! assert!(Fq3::one().is_one());
//! assert!((a * a.inv().unwrap()).is_one());
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

use crate::field::{ConstField, FiniteField};

use super::ExtConfig;

// ---------------------------------------------------------------------------
// Wide accumulator
// ---------------------------------------------------------------------------

/// Wide accumulator for [`CubicExt`]: three base-field wide components.
///
/// Stores the coefficients `(c0, c1, c2)` of a cubic-extension product using
/// the base field's `Wide` type. Multiple products may be accumulated via
/// `+=` before a single [`FiniteField::reduce_wide`] call reduces back into
/// the field. The per-component limit is inherited from
/// [`FiniteField::max_unreduced_additions`] on the base field.
///
/// # Type Parameters
///
/// * `W` — The base field's wide type (e.g., `u128` for `Fp<P>`).
///
/// # Examples
///
/// ```
/// use gf2_core::field::FiniteField;
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{CubicExt, ExtConfig};
///
/// struct Cfg;
/// impl ExtConfig for Cfg {
///     type BaseField = Fp<7>;
///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
/// }
/// type Fq3 = CubicExt<Cfg>;
///
/// let a = Fq3::new(Fp::new(1), Fp::new(2), Fp::new(3));
/// let b = Fq3::new(Fp::new(4), Fp::new(5), Fp::new(6));
///
/// let mut acc = a.mul_to_wide(&b);
/// acc += a.mul_to_wide(&b);
/// let reduced = Fq3::reduce_wide(&acc);
/// assert_eq!(reduced, a * b + a * b);
/// ```
pub struct CubicExtWide<W> {
    /// Unreduced constant coefficient.
    c0: W,
    /// Unreduced coefficient of `v`.
    c1: W,
    /// Unreduced coefficient of `v²`.
    c2: W,
}

impl<W> CubicExtWide<W> {
    /// Creates a new wide accumulator from three component-wise wide values.
    ///
    /// # Arguments
    ///
    /// * `c0` — Wide value for the constant coefficient.
    /// * `c1` — Wide value for the coefficient of `v`.
    /// * `c2` — Wide value for the coefficient of `v²`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfpn::CubicExtWide;
    ///
    /// let w = CubicExtWide::<u128>::new(1u128, 2u128, 3u128);
    /// assert_eq!(w.c0(), &1u128);
    /// assert_eq!(w.c1(), &2u128);
    /// assert_eq!(w.c2(), &3u128);
    /// ```
    #[inline]
    pub const fn new(c0: W, c1: W, c2: W) -> Self {
        Self { c0, c1, c2 }
    }

    /// Returns a reference to the wide `c0` component.
    #[inline]
    pub const fn c0(&self) -> &W {
        &self.c0
    }

    /// Returns a reference to the wide `c1` component.
    #[inline]
    pub const fn c1(&self) -> &W {
        &self.c1
    }

    /// Returns a reference to the wide `c2` component.
    #[inline]
    pub const fn c2(&self) -> &W {
        &self.c2
    }
}

impl<W: Clone> Clone for CubicExtWide<W> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            c0: self.c0.clone(),
            c1: self.c1.clone(),
            c2: self.c2.clone(),
        }
    }
}

impl<W: Copy> Copy for CubicExtWide<W> {}

impl<W: fmt::Debug> fmt::Debug for CubicExtWide<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CubicExtWide")
            .field("c0", &self.c0)
            .field("c1", &self.c1)
            .field("c2", &self.c2)
            .finish()
    }
}

impl<W: PartialEq> PartialEq for CubicExtWide<W> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.c0 == other.c0 && self.c1 == other.c1 && self.c2 == other.c2
    }
}

impl<W: Eq> Eq for CubicExtWide<W> {}

impl<W: Default> Default for CubicExtWide<W> {
    #[inline]
    fn default() -> Self {
        Self {
            c0: W::default(),
            c1: W::default(),
            c2: W::default(),
        }
    }
}

impl<W: Add<Output = W>> Add for CubicExtWide<W> {
    type Output = Self;

    /// Component-wise wide addition.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            c0: self.c0 + rhs.c0,
            c1: self.c1 + rhs.c1,
            c2: self.c2 + rhs.c2,
        }
    }
}

impl<W: AddAssign> AddAssign for CubicExtWide<W> {
    /// Component-wise wide `+=`.
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.c0 += rhs.c0;
        self.c1 += rhs.c1;
        self.c2 += rhs.c2;
    }
}

/// Element of a cubic extension field: `c0 + c1·v + c2·v²` where `v³ = β`.
///
/// Parameterized by a config type `C: ExtConfig` that specifies the base field
/// and non-residue. Two extensions with different configs are distinct types.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{ExtConfig, CubicExt};
/// use gf2_core::field::{FiniteField, ConstField};
///
/// struct Fq3Config;
/// impl ExtConfig for Fq3Config {
///     type BaseField = Fp<7>;
///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
/// }
/// type Fq3 = CubicExt<Fq3Config>;
///
/// let a = Fq3::new(Fp::new(3), Fp::new(5), Fp::new(2));
/// let b = Fq3::new(Fp::new(1), Fp::new(4), Fp::new(6));
/// let c = a * b;
///
/// assert_eq!(a.c0().value(), 3);
/// assert_eq!(a.c1().value(), 5);
/// assert_eq!(a.c2().value(), 2);
/// assert!(Fq3::zero().is_zero());
/// assert!(Fq3::one().is_one());
/// ```
pub struct CubicExt<C: ExtConfig> {
    c0: C::BaseField,
    c1: C::BaseField,
    c2: C::BaseField,
}

// Manual trait impls to avoid derive adding bounds on C itself.
// Only C::BaseField needs these traits (guaranteed by ConstField: Copy + FiniteField).

impl<C: ExtConfig> Clone for CubicExt<C> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: ExtConfig> Copy for CubicExt<C> {}

impl<C: ExtConfig> PartialEq for CubicExt<C> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.c0 == other.c0 && self.c1 == other.c1 && self.c2 == other.c2
    }
}

impl<C: ExtConfig> Eq for CubicExt<C> {}

impl<C: ExtConfig> Hash for CubicExt<C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.c0.hash(state);
        self.c1.hash(state);
        self.c2.hash(state);
    }
}

impl<C: ExtConfig> CubicExt<C> {
    /// Creates a new element `c0 + c1·v + c2·v²`.
    ///
    /// # Arguments
    ///
    /// * `c0` - The constant component.
    /// * `c1` - The coefficient of `v`.
    /// * `c2` - The coefficient of `v²`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ExtConfig, CubicExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    /// }
    ///
    /// let a = CubicExt::<Cfg>::new(Fp::new(1), Fp::new(2), Fp::new(3));
    /// ```
    #[inline]
    pub const fn new(c0: C::BaseField, c1: C::BaseField, c2: C::BaseField) -> Self {
        Self { c0, c1, c2 }
    }

    /// Returns the constant component `c0`.
    #[inline]
    pub const fn c0(&self) -> C::BaseField {
        self.c0
    }

    /// Returns the coefficient of `v` (component `c1`).
    #[inline]
    pub const fn c1(&self) -> C::BaseField {
        self.c1
    }

    /// Returns the coefficient of `v²` (component `c2`).
    #[inline]
    pub const fn c2(&self) -> C::BaseField {
        self.c2
    }

    /// Returns the field norm: `a0³ + β·a1³ + β²·a2³ − 3β·a0·a1·a2` (a base field element).
    ///
    /// The norm is multiplicative: `N(a·b) = N(a)·N(b)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ExtConfig, CubicExt};
    /// use gf2_core::field::{FiniteField, ConstField};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    /// }
    /// type Fq3 = CubicExt<Cfg>;
    ///
    /// // N(1) = 1³ + 3·0³ + 9·0³ − 0 = 1
    /// assert_eq!(Fq3::one().norm().value(), 1);
    /// ```
    pub fn norm(&self) -> C::BaseField {
        let (a0, a1, a2) = (self.c0, self.c1, self.c2);

        // Cofactors
        let s0 = a0 * a0 - C::mul_by_non_residue(a1 * a2);
        let s1 = C::mul_by_non_residue(a2 * a2) - a0 * a1;
        let s2 = a1 * a1 - a0 * a2;

        // Norm = a0·s0 + β·(a2·s1 + a1·s2)
        a0 * s0 + C::mul_by_non_residue(a2 * s1 + a1 * s2)
    }
}

impl<C: ExtConfig> CubicExt<C> {
    /// Embeds a base field element into the extension: `a ↦ a + 0·v + 0·v²`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ExtConfig, CubicExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    /// }
    ///
    /// let a = CubicExt::<Cfg>::from_base(Fp::new(3));
    /// assert_eq!(a.c0().value(), 3);
    /// assert_eq!(a.c1().value(), 0);
    /// assert_eq!(a.c2().value(), 0);
    /// ```
    #[inline]
    pub fn from_base(value: C::BaseField) -> Self {
        let z = C::BaseField::zero();
        Self::new(value, z, z)
    }
}

// ---------------------------------------------------------------------------
// Display and Debug
// ---------------------------------------------------------------------------

impl<C: ExtConfig> fmt::Display for CubicExt<C>
where
    C::BaseField: fmt::Display,
{
    /// Formats as `"c0 + c1·v + c2·v²"`, omitting zero terms.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c0_zero = self.c0.is_zero();
        let c1_zero = self.c1.is_zero();
        let c2_zero = self.c2.is_zero();

        if c0_zero && c1_zero && c2_zero {
            return write!(f, "0");
        }

        let mut first = true;

        if !c0_zero {
            write!(f, "{}", self.c0)?;
            first = false;
        }

        if !c1_zero {
            if !first {
                write!(f, " + ")?;
            }
            write!(f, "{}·v", self.c1)?;
            first = false;
        }

        if !c2_zero {
            if !first {
                write!(f, " + ")?;
            }
            write!(f, "{}·v²", self.c2)?;
        }

        Ok(())
    }
}

impl<C: ExtConfig> fmt::Debug for CubicExt<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CubicExt({:?}, {:?}, {:?})", self.c0, self.c1, self.c2)
    }
}

// ---------------------------------------------------------------------------
// Arithmetic operators
// ---------------------------------------------------------------------------

impl<C: ExtConfig> Add for CubicExt<C> {
    type Output = Self;

    /// Component-wise addition: `(a0+b0) + (a1+b1)·v + (a2+b2)·v²`.
    ///
    /// # Complexity
    ///
    /// 3 base-field additions.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.c0 + rhs.c0, self.c1 + rhs.c1, self.c2 + rhs.c2)
    }
}

impl<C: ExtConfig> Sub for CubicExt<C> {
    type Output = Self;

    /// Component-wise subtraction: `(a0−b0) + (a1−b1)·v + (a2−b2)·v²`.
    ///
    /// # Complexity
    ///
    /// 3 base-field subtractions.
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.c0 - rhs.c0, self.c1 - rhs.c1, self.c2 - rhs.c2)
    }
}

impl<C: ExtConfig> Neg for CubicExt<C> {
    type Output = Self;

    /// Component-wise negation: `(−a0) + (−a1)·v + (−a2)·v²`.
    ///
    /// # Complexity
    ///
    /// 3 base-field negations.
    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.c0, -self.c1, -self.c2)
    }
}

impl<C: ExtConfig> Mul for CubicExt<C> {
    type Output = Self;

    /// Karatsuba-style multiplication using 6 base-field multiplications.
    ///
    /// # Complexity
    ///
    /// 6M + 13A + 2B (M = base mul, A = add/sub, B = mul_by_non_residue).
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let (a0, a1, a2) = (self.c0, self.c1, self.c2);
        let (b0, b1, b2) = (rhs.c0, rhs.c1, rhs.c2);

        // Three diagonal products
        let v0 = a0 * b0;
        let v1 = a1 * b1;
        let v2 = a2 * b2;

        // Three cross products via Karatsuba identity
        let x = (a1 + a2) * (b1 + b2) - v1 - v2; // a1·b2 + a2·b1
        let y = (a0 + a1) * (b0 + b1) - v0 - v1; // a0·b1 + a1·b0
        let z = (a0 + a2) * (b0 + b2) - v0 + v1 - v2; // a0·b2 + a1·b1 + a2·b0

        // Assemble with reduction: v³ = β
        let c0 = v0 + C::mul_by_non_residue(x);
        let c1 = y + C::mul_by_non_residue(v2);
        let c2 = z;

        Self::new(c0, c1, c2)
    }
}

impl<C: ExtConfig> Div for CubicExt<C> {
    type Output = Self;

    /// Division via multiplicative inverse.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inv().expect("division by zero in CubicExt")
    }
}

// ---------------------------------------------------------------------------
// AddAssign
// ---------------------------------------------------------------------------

impl<C: ExtConfig> AddAssign for CubicExt<C> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<C: ExtConfig> AddAssign<&Self> for CubicExt<C> {
    #[inline]
    fn add_assign(&mut self, rhs: &Self) {
        *self = *self + *rhs;
    }
}

// ---------------------------------------------------------------------------
// Reference-forwarding operators (CubicExt is Copy, so dereference)
// ---------------------------------------------------------------------------

impl<C: ExtConfig> Add<&CubicExt<C>> for CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn add(self, rhs: &CubicExt<C>) -> CubicExt<C> {
        self + *rhs
    }
}

impl<C: ExtConfig> Add for &CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn add(self, rhs: Self) -> CubicExt<C> {
        *self + *rhs
    }
}

impl<C: ExtConfig> Sub<&CubicExt<C>> for CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn sub(self, rhs: &CubicExt<C>) -> CubicExt<C> {
        self - *rhs
    }
}

impl<C: ExtConfig> Sub for &CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn sub(self, rhs: Self) -> CubicExt<C> {
        *self - *rhs
    }
}

impl<C: ExtConfig> Mul<&CubicExt<C>> for CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn mul(self, rhs: &CubicExt<C>) -> CubicExt<C> {
        self * *rhs
    }
}

impl<C: ExtConfig> Mul for &CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn mul(self, rhs: Self) -> CubicExt<C> {
        *self * *rhs
    }
}

impl<C: ExtConfig> Div<&CubicExt<C>> for CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn div(self, rhs: &CubicExt<C>) -> CubicExt<C> {
        self / *rhs
    }
}

impl<C: ExtConfig> Div for &CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn div(self, rhs: Self) -> CubicExt<C> {
        *self / *rhs
    }
}

impl<C: ExtConfig> Neg for &CubicExt<C> {
    type Output = CubicExt<C>;
    #[inline]
    fn neg(self) -> CubicExt<C> {
        -(*self)
    }
}

// ---------------------------------------------------------------------------
// FiniteField implementation
// ---------------------------------------------------------------------------

impl<C: ExtConfig> FiniteField for CubicExt<C> {
    type Characteristic = <C::BaseField as FiniteField>::Characteristic;
    type Wide = CubicExtWide<<C::BaseField as FiniteField>::Wide>;

    #[inline]
    fn characteristic(&self) -> Self::Characteristic {
        self.c0.characteristic()
    }

    #[inline]
    fn extension_degree(&self) -> usize {
        3 * self.c0.extension_degree()
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.c0.is_zero() && self.c1.is_zero() && self.c2.is_zero()
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.c0.is_one() && self.c1.is_zero() && self.c2.is_zero()
    }

    /// Adjugate/norm inversion with a single base-field inversion.
    ///
    /// # Complexity
    ///
    /// 9M + 3S + 1I + 9A + 4B (I = base inversion).
    fn inv(&self) -> Option<Self> {
        let (a0, a1, a2) = (self.c0, self.c1, self.c2);

        // Cofactors of the adjugate
        let s0 = a0 * a0 - C::mul_by_non_residue(a1 * a2);
        let s1 = C::mul_by_non_residue(a2 * a2) - a0 * a1;
        let s2 = a1 * a1 - a0 * a2;

        // Norm = a0·s0 + β·(a2·s1 + a1·s2)
        let norm = a0 * s0 + C::mul_by_non_residue(a2 * s1 + a1 * s2);

        norm.inv()
            .map(|norm_inv| Self::new(s0 * norm_inv, s1 * norm_inv, s2 * norm_inv))
    }

    #[inline]
    fn zero_like(&self) -> Self {
        let z = self.c0.zero_like();
        Self::new(z, z, z)
    }

    #[inline]
    fn one_like(&self) -> Self {
        Self::new(self.c0.one_like(), self.c0.zero_like(), self.c0.zero_like())
    }

    /// Component-wise widening: each base coefficient is lifted via
    /// [`FiniteField::to_wide`].
    #[inline]
    fn to_wide(&self) -> Self::Wide {
        CubicExtWide::new(self.c0.to_wide(), self.c1.to_wide(), self.c2.to_wide())
    }

    /// Karatsuba-style multiplication at the tower level followed by
    /// component-wise widening (Option 1 of the design plan). Individual
    /// products are fully reduced in the base field, but the result is stored
    /// in a three-component wide accumulator so that sums of products can be
    /// accumulated without per-product reduction.
    ///
    /// # Complexity
    ///
    /// 6M + 13A + 2B in the base field plus 3 widening copies.
    #[inline]
    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide {
        (*self * *rhs).to_wide()
    }

    /// Reduces a wide accumulator component-wise via the base field's
    /// [`FiniteField::reduce_wide`].
    #[inline]
    fn reduce_wide(wide: &Self::Wide) -> Self {
        Self::new(
            C::BaseField::reduce_wide(&wide.c0),
            C::BaseField::reduce_wide(&wide.c1),
            C::BaseField::reduce_wide(&wide.c2),
        )
    }

    /// Delegates to the base field. Because each tower coefficient accumulates
    /// independently, the per-component limit equals
    /// [`C::BaseField::max_unreduced_additions`].
    fn max_unreduced_additions() -> usize {
        C::BaseField::max_unreduced_additions()
    }
}

// ---------------------------------------------------------------------------
// ConstField implementation
// ---------------------------------------------------------------------------

impl<C: ExtConfig> ConstField for CubicExt<C> {
    #[inline]
    fn zero() -> Self {
        let z = C::BaseField::zero();
        Self::new(z, z, z)
    }

    #[inline]
    fn one() -> Self {
        Self::new(
            C::BaseField::one(),
            C::BaseField::zero(),
            C::BaseField::zero(),
        )
    }

    #[inline]
    fn order() -> u128 {
        let base_order = C::BaseField::order();
        base_order * base_order * base_order
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::axiom_tests::test_const_field_axioms;
    use crate::gfp::Fp;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Test config: GF(7³) with β = 3 (a cubic non-residue mod 7)
    // Cubes mod 7: 0³=0, 1³=1, 2³=1, 3³=6, 4³=1, 5³=6, 6³=6 → {0,1,6}
    // So 3 is a cubic non-residue.
    // -----------------------------------------------------------------------

    struct Fq3Config;
    impl ExtConfig for Fq3Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3); // β = 3
    }
    type Fq3 = CubicExt<Fq3Config>;

    // -----------------------------------------------------------------------
    // Axiom test harness (required for success)
    // -----------------------------------------------------------------------

    #[test]
    fn test_cubic_ext_fp7_field_axioms() {
        let strategy = (0..7u64, 0..7u64, 0..7u64)
            .prop_map(|(c0, c1, c2)| Fq3::new(Fp::new(c0), Fp::new(c1), Fp::new(c2)))
            .boxed();
        test_const_field_axioms(strategy, 7);
    }

    // -----------------------------------------------------------------------
    // Construction and accessors
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_and_accessors() {
        let a = Fq3::new(Fp::new(3), Fp::new(5), Fp::new(2));
        assert_eq!(a.c0().value(), 3);
        assert_eq!(a.c1().value(), 5);
        assert_eq!(a.c2().value(), 2);
    }

    #[test]
    fn test_zero_and_one() {
        assert!(Fq3::zero().is_zero());
        assert!(Fq3::one().is_one());
        assert!(!Fq3::zero().is_one());
        assert!(!Fq3::one().is_zero());
    }

    // -----------------------------------------------------------------------
    // Embedding
    // -----------------------------------------------------------------------

    #[test]
    fn test_embedding() {
        for k in 0..7u64 {
            let embedded = Fq3::from_base(Fp::<7>::new(k));
            assert_eq!(embedded, Fq3::new(Fp::new(k), Fp::new(0), Fp::new(0)));
        }
    }

    // -----------------------------------------------------------------------
    // Display and Debug
    // -----------------------------------------------------------------------

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", Fq3::new(Fp::new(1), Fp::new(2), Fp::new(3))),
            "1 + 2·v + 3·v²"
        );
        assert_eq!(
            format!("{}", Fq3::new(Fp::new(5), Fp::new(0), Fp::new(0))),
            "5"
        );
        assert_eq!(
            format!("{}", Fq3::new(Fp::new(0), Fp::new(4), Fp::new(0))),
            "4·v"
        );
        assert_eq!(
            format!("{}", Fq3::new(Fp::new(0), Fp::new(0), Fp::new(6))),
            "6·v²"
        );
        assert_eq!(
            format!("{}", Fq3::new(Fp::new(3), Fp::new(0), Fp::new(5))),
            "3 + 5·v²"
        );
        assert_eq!(
            format!("{}", Fq3::new(Fp::new(0), Fp::new(2), Fp::new(4))),
            "2·v + 4·v²"
        );
        assert_eq!(
            format!("{}", Fq3::new(Fp::new(1), Fp::new(3), Fp::new(0))),
            "1 + 3·v"
        );
        assert_eq!(format!("{}", Fq3::zero()), "0");
    }

    #[test]
    fn test_debug() {
        // Debug uses {:?} on base-field coefficients so that tower
        // base fields without `fmt::Display` still produce a useful
        // debug representation. For `Fp<7>` the Debug format is
        // `Fp<7>(value)`.
        let a = Fq3::new(Fp::new(3), Fp::new(5), Fp::new(2));
        assert_eq!(format!("{:?}", a), "CubicExt(Fp<7>(3), Fp<7>(5), Fp<7>(2))");
        assert_eq!(
            format!("{:?}", Fq3::zero()),
            "CubicExt(Fp<7>(0), Fp<7>(0), Fp<7>(0))"
        );
    }

    // -----------------------------------------------------------------------
    // Extension degree, order, characteristic
    // -----------------------------------------------------------------------

    #[test]
    fn test_extension_degree() {
        assert_eq!(Fq3::one().extension_degree(), 3);
    }

    #[test]
    fn test_order() {
        assert_eq!(Fq3::order(), 343);
    }

    #[test]
    fn test_characteristic() {
        assert_eq!(Fq3::one().characteristic(), 7u64);
    }

    // -----------------------------------------------------------------------
    // Known value tests (hand-computed for GF(7³) with β = 3)
    // -----------------------------------------------------------------------

    #[test]
    fn test_known_v_cubed_is_beta() {
        // v³ = β = 3
        let v = Fq3::new(Fp::new(0), Fp::new(1), Fp::new(0));
        let v_cubed = v * v * v;
        assert_eq!(v_cubed, Fq3::from_base(Fp::new(3)));
    }

    #[test]
    fn test_known_multiplication() {
        // (1 + v)(1 + v) = 1 + 2v + v²
        let a = Fq3::new(Fp::new(1), Fp::new(1), Fp::new(0));
        let c = a * a;
        assert_eq!(c, Fq3::new(Fp::new(1), Fp::new(2), Fp::new(1)));

        // (1 + v)(v²) = v² + v³ = v² + 3 = 3 + 0·v + 1·v²
        let b = Fq3::new(Fp::new(0), Fp::new(0), Fp::new(1));
        let d = a * b;
        assert_eq!(d, Fq3::new(Fp::new(3), Fp::new(0), Fp::new(1)));
    }

    // -----------------------------------------------------------------------
    // Norm tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_norm_of_one() {
        assert_eq!(Fq3::one().norm().value(), 1);
    }

    #[test]
    fn test_norm_multiplicative() {
        // Test over a representative subset (all pairs would be 343² = 117649)
        for a0 in 0..7u64 {
            for a1 in 0..7u64 {
                let a2 = (a0 + a1) % 7; // single representative a2 per (a0, a1)
                let a = Fq3::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
                if a.is_zero() {
                    continue;
                }
                for b0 in 0..7u64 {
                    for b1 in 0..7u64 {
                        let b2 = (b0 * 2 + b1) % 7;
                        let b = Fq3::new(Fp::new(b0), Fp::new(b1), Fp::new(b2));
                        if b.is_zero() {
                            continue;
                        }
                        let n_ab = (a * b).norm();
                        let na_nb = a.norm() * b.norm();
                        assert_eq!(
                            n_ab, na_nb,
                            "N(a*b) != N(a)*N(b) for a=({a0},{a1},{a2}), b=({b0},{b1},{b2})"
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Exhaustive multiplication cross-check (Karatsuba vs naive)
    // -----------------------------------------------------------------------

    #[test]
    fn test_karatsuba_matches_naive_representative() {
        // Naive schoolbook: c0 = a0*b0 + β*(a1*b2 + a2*b1)
        //                   c1 = a0*b1 + a1*b0 + β*a2*b2
        //                   c2 = a0*b2 + a1*b1 + a2*b0
        // Test all 343 × 343 = 117649 pairs exhaustively
        let beta = 3u64;
        for a0 in 0..7u64 {
            for a1 in 0..7u64 {
                for a2 in 0..7u64 {
                    for b0 in 0..7u64 {
                        for b1 in 0..7u64 {
                            for b2 in 0..7u64 {
                                let a = Fq3::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
                                let b = Fq3::new(Fp::new(b0), Fp::new(b1), Fp::new(b2));
                                let c = a * b;

                                let naive_c0 = (a0 * b0 + beta * (a1 * b2 + a2 * b1)) % 7;
                                let naive_c1 = (a0 * b1 + a1 * b0 + beta * a2 * b2) % 7;
                                let naive_c2 = (a0 * b2 + a1 * b1 + a2 * b0) % 7;

                                assert_eq!(
                                    c.c0().value(),
                                    naive_c0,
                                    "c0: ({a0}+{a1}v+{a2}v²)*({b0}+{b1}v+{b2}v²)"
                                );
                                assert_eq!(
                                    c.c1().value(),
                                    naive_c1,
                                    "c1: ({a0}+{a1}v+{a2}v²)*({b0}+{b1}v+{b2}v²)"
                                );
                                assert_eq!(
                                    c.c2().value(),
                                    naive_c2,
                                    "c2: ({a0}+{a1}v+{a2}v²)*({b0}+{b1}v+{b2}v²)"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Exhaustive inversion round-trip (all 342 non-zero elements)
    // -----------------------------------------------------------------------

    #[test]
    fn test_inversion_roundtrip_exhaustive() {
        let one = Fq3::one();
        for c0 in 0..7u64 {
            for c1 in 0..7u64 {
                for c2 in 0..7u64 {
                    let a = Fq3::new(Fp::new(c0), Fp::new(c1), Fp::new(c2));
                    if a.is_zero() {
                        assert!(a.inv().is_none());
                        continue;
                    }
                    let inv = a.inv().expect("non-zero element must have inverse");
                    assert_eq!(a * inv, one, "a * inv(a) != 1 for a = ({c0}, {c1}, {c2})");
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Reference operators
    // -----------------------------------------------------------------------

    #[test]
    #[allow(clippy::op_ref)]
    fn test_ref_operators() {
        let a = Fq3::new(Fp::new(3), Fp::new(5), Fp::new(2));
        let b = Fq3::new(Fp::new(1), Fp::new(4), Fp::new(6));
        assert_eq!(a + b, &a + &b);
        assert_eq!(a + &b, &a + &b);
        assert_eq!(a - b, &a - &b);
        assert_eq!(a * b, &a * &b);
        assert_eq!(a / b, &a / &b);
        assert_eq!(-a, -&a);
    }

    #[test]
    fn test_add_assign() {
        let mut a = Fq3::new(Fp::new(3), Fp::new(5), Fp::new(2));
        a += Fq3::new(Fp::new(1), Fp::new(4), Fp::new(6));
        assert_eq!(a, Fq3::new(Fp::new(4), Fp::new(2), Fp::new(1)));
    }

    #[test]
    fn test_add_assign_ref() {
        let mut a = Fq3::new(Fp::new(3), Fp::new(5), Fp::new(2));
        let b = Fq3::new(Fp::new(1), Fp::new(4), Fp::new(6));
        a += &b;
        assert_eq!(a, Fq3::new(Fp::new(4), Fp::new(2), Fp::new(1)));
        assert_eq!(b.c0().value(), 1); // b still valid
    }

    // -----------------------------------------------------------------------
    // Size: no runtime overhead
    // -----------------------------------------------------------------------

    #[test]
    fn test_size_of() {
        assert_eq!(std::mem::size_of::<Fq3>(), 3 * std::mem::size_of::<Fp<7>>());
    }

    // -----------------------------------------------------------------------
    // Wide accumulator tests (issue d11b769a)
    // -----------------------------------------------------------------------

    /// Wide is a real three-component accumulator, not an alias for `Self`.
    #[test]
    fn test_wide_type_is_not_self() {
        assert_eq!(
            std::mem::size_of::<<Fq3 as FiniteField>::Wide>(),
            3 * std::mem::size_of::<<Fp<7> as FiniteField>::Wide>(),
        );
    }

    /// `max_unreduced_additions` must equal the base field's bound. For GF(7)
    /// the bound saturates at `usize::MAX` because `(p-1)²` is tiny; the
    /// meaningful finite case is covered by the Mersenne-61 test below.
    #[test]
    fn test_max_unreduced_additions_is_base_field_bound() {
        let k = <Fq3 as FiniteField>::max_unreduced_additions();
        let base = <Fp<7> as FiniteField>::max_unreduced_additions();
        assert_eq!(k, base);
    }

    /// For a large prime the bound is finite, proving we no longer return
    /// the `usize::MAX` sentinel that the old `Wide = Self` placeholder used.
    #[test]
    fn test_max_unreduced_additions_finite_for_large_prime() {
        // GF(Mersenne61³) with β = 3.
        struct MConfig;
        impl ExtConfig for MConfig {
            type BaseField = Fp<2305843009213693951>;
            const NON_RESIDUE: Fp<2305843009213693951> = Fp::<2305843009213693951>::new(3);
        }
        type MFq3 = CubicExt<MConfig>;

        let k = <MFq3 as FiniteField>::max_unreduced_additions();
        let base = <Fp<2305843009213693951> as FiniteField>::max_unreduced_additions();
        assert_eq!(k, base);
        assert_ne!(k, usize::MAX);
        assert!(k >= 1);
    }

    /// `reduce_wide(to_wide(a)) == a` for all 343 elements of GF(7³).
    #[test]
    fn test_wide_roundtrip_exhaustive() {
        for c0 in 0..7u64 {
            for c1 in 0..7u64 {
                for c2 in 0..7u64 {
                    let a = Fq3::new(Fp::new(c0), Fp::new(c1), Fp::new(c2));
                    let wide = a.to_wide();
                    let back = <Fq3 as FiniteField>::reduce_wide(&wide);
                    assert_eq!(back, a, "roundtrip failed at ({c0}, {c1}, {c2})");
                }
            }
        }
    }

    /// `reduce_wide(mul_to_wide(a, b)) == a * b` — representative subset.
    #[test]
    fn test_mul_to_wide_consistency_representative() {
        // Full exhaustive would be 343² = 117649 pairs; that's fine but slow.
        // Representative sweep already validates the path; the proptest below
        // provides the randomised coverage.
        for a0 in 0..7u64 {
            for a1 in 0..7u64 {
                for a2 in 0..7u64 {
                    let a = Fq3::new(Fp::new(a0), Fp::new(a1), Fp::new(a2));
                    let b = Fq3::new(
                        Fp::new((a0 + 1) % 7),
                        Fp::new((a1 * 2) % 7),
                        Fp::new((a2 + 3) % 7),
                    );
                    let wide = a.mul_to_wide(&b);
                    let reduced = <Fq3 as FiniteField>::reduce_wide(&wide);
                    assert_eq!(reduced, a * b);
                }
            }
        }
    }

    /// Dot-product accumulation over random cases.
    #[test]
    fn test_dot_product_accumulation_proptest() {
        let mut runner =
            proptest::test_runner::TestRunner::new(proptest::test_runner::Config::with_cases(200));
        let strategy = proptest::collection::vec(
            (0..7u64, 0..7u64, 0..7u64, 0..7u64, 0..7u64, 0..7u64),
            1..=32,
        );
        runner
            .run(&strategy, |pairs| {
                let mut acc: <Fq3 as FiniteField>::Wide = <Fq3 as FiniteField>::Wide::default();
                let mut expected = Fq3::zero();
                for (a0, a1, a2, b0, b1, b2) in &pairs {
                    let a = Fq3::new(Fp::new(*a0), Fp::new(*a1), Fp::new(*a2));
                    let b = Fq3::new(Fp::new(*b0), Fp::new(*b1), Fp::new(*b2));
                    acc += a.mul_to_wide(&b);
                    expected += a * b;
                }
                let got = <Fq3 as FiniteField>::reduce_wide(&acc);
                prop_assert_eq!(got, expected);
                Ok(())
            })
            .expect("cubic dot-product accumulation must match element-wise");
    }

    /// `CubicExtWide` Add and AddAssign are component-wise.
    #[test]
    fn test_wide_add_and_add_assign() {
        let w1 = <Fq3 as FiniteField>::Wide::new(1u128, 2u128, 3u128);
        let w2 = <Fq3 as FiniteField>::Wide::new(10u128, 20u128, 30u128);
        let sum = w1 + w2;
        assert_eq!(*sum.c0(), 11u128);
        assert_eq!(*sum.c1(), 22u128);
        assert_eq!(*sum.c2(), 33u128);

        let mut w = <Fq3 as FiniteField>::Wide::new(1u128, 2u128, 3u128);
        w += <Fq3 as FiniteField>::Wide::new(5u128, 6u128, 7u128);
        assert_eq!(*w.c0(), 6u128);
        assert_eq!(*w.c1(), 8u128);
        assert_eq!(*w.c2(), 10u128);
    }

    /// `CubicExtWide` Debug output is informative and not `Self`-typed.
    #[test]
    fn test_wide_debug_format() {
        let w = <Fq3 as FiniteField>::Wide::new(1u128, 2u128, 3u128);
        let s = format!("{:?}", w);
        assert!(s.contains("CubicExtWide"), "unexpected debug: {s}");
        assert!(s.contains("1") && s.contains("2") && s.contains("3"));
    }
}
