//! Quadratic extension field arithmetic: elements `c0 + c1·u` where `u² = β`.
//!
//! [`QuadraticExt<C>`] implements a degree-2 extension of any base field that
//! implements [`ConstField`], parameterized by an [`ExtConfig`] specifying the
//! non-residue β.
//!
//! # Multiplication
//!
//! Uses the Karatsuba method (3 base-field multiplications instead of 4):
//!
//! ```text
//! v0 = a0·b0
//! v1 = a1·b1
//! c0 = v0 + β·v1
//! c1 = (a0+a1)(b0+b1) − v0 − v1
//! ```
//!
//! Reference: Devegili, O hEigeartaigh, Scott, Dahab (ePrint 2006/471).
//!
//! # Inversion
//!
//! Uses the norm-based method: `a⁻¹ = conjugate(a) / norm(a)` where
//! `norm(a) = a0² − β·a1²`.
//!
//! # Examples
//!
//! ```
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::{ExtConfig, QuadraticExt};
//! use gf2_core::field::{FiniteField, ConstField};
//!
//! struct Fq2Config;
//! impl ExtConfig for Fq2Config {
//!     type BaseField = Fp<7>;
//!     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1 mod 7
//! }
//! type Fq2 = QuadraticExt<Fq2Config>;
//!
//! let a = Fq2::new(Fp::new(3), Fp::new(5));
//! assert_eq!(a.c0().value(), 3);
//! assert_eq!(a.c1().value(), 5);
//!
//! // Field axioms hold
//! assert!(Fq2::zero().is_zero());
//! assert!(Fq2::one().is_one());
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

/// Wide accumulator for [`QuadraticExt`]: two base-field wide components.
///
/// Stores the two component coefficients `(c0, c1)` of a quadratic-extension
/// product using the base field's `Wide` type. Multiple products may be
/// accumulated via `+=` before a single [`FiniteField::reduce_wide`] call
/// brings the accumulator back into the field. The per-component limit on the
/// number of products is governed by
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
/// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
///
/// struct Cfg;
/// impl ExtConfig for Cfg {
///     type BaseField = Fp<7>;
///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1
/// }
/// type Fq2 = QuadraticExt<Cfg>;
///
/// let a = Fq2::new(Fp::new(3), Fp::new(5));
/// let b = Fq2::new(Fp::new(2), Fp::new(4));
///
/// // Accumulate two products in wide, reduce once.
/// let mut acc = a.mul_to_wide(&b);
/// acc += a.mul_to_wide(&b);
/// let reduced = Fq2::reduce_wide(&acc);
/// assert_eq!(reduced, a * b + a * b);
/// ```
pub struct QuadraticExtWide<W> {
    /// Unreduced constant coefficient.
    c0: W,
    /// Unreduced coefficient of `u`.
    c1: W,
}

impl<W> QuadraticExtWide<W> {
    /// Creates a new wide accumulator from component-wise wide values.
    ///
    /// # Arguments
    ///
    /// * `c0` — Wide value for the constant coefficient.
    /// * `c1` — Wide value for the coefficient of `u`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfpn::QuadraticExtWide;
    ///
    /// let w = QuadraticExtWide::<u128>::new(20u128, 18u128);
    /// assert_eq!(w.c0(), &20u128);
    /// assert_eq!(w.c1(), &18u128);
    /// ```
    #[inline]
    pub const fn new(c0: W, c1: W) -> Self {
        Self { c0, c1 }
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
}

impl<W: Clone> Clone for QuadraticExtWide<W> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            c0: self.c0.clone(),
            c1: self.c1.clone(),
        }
    }
}

impl<W: Copy> Copy for QuadraticExtWide<W> {}

impl<W: fmt::Debug> fmt::Debug for QuadraticExtWide<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuadraticExtWide")
            .field("c0", &self.c0)
            .field("c1", &self.c1)
            .finish()
    }
}

impl<W: PartialEq> PartialEq for QuadraticExtWide<W> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.c0 == other.c0 && self.c1 == other.c1
    }
}

impl<W: Eq> Eq for QuadraticExtWide<W> {}

impl<W: Default> Default for QuadraticExtWide<W> {
    #[inline]
    fn default() -> Self {
        Self {
            c0: W::default(),
            c1: W::default(),
        }
    }
}

impl<W: Add<Output = W>> Add for QuadraticExtWide<W> {
    type Output = Self;

    /// Component-wise wide addition.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            c0: self.c0 + rhs.c0,
            c1: self.c1 + rhs.c1,
        }
    }
}

impl<W: AddAssign> AddAssign for QuadraticExtWide<W> {
    /// Component-wise wide `+=`.
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.c0 += rhs.c0;
        self.c1 += rhs.c1;
    }
}

/// Element of a quadratic extension field: `c0 + c1·u` where `u² = β`.
///
/// Parameterized by a config type `C: ExtConfig` that specifies the base field
/// and non-residue. Two extensions with different configs are distinct types.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
/// use gf2_core::field::{FiniteField, ConstField};
///
/// struct Fq2Config;
/// impl ExtConfig for Fq2Config {
///     type BaseField = Fp<7>;
///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1
/// }
/// type Fq2 = QuadraticExt<Fq2Config>;
///
/// let a = Fq2::new(Fp::new(3), Fp::new(5));
/// let b = Fq2::new(Fp::new(2), Fp::new(4));
/// let c = a * b;
///
/// assert_eq!(a.c0().value(), 3);
/// assert_eq!(a.c1().value(), 5);
/// assert!(Fq2::zero().is_zero());
/// assert!(Fq2::one().is_one());
/// ```
pub struct QuadraticExt<C: ExtConfig> {
    c0: C::BaseField,
    c1: C::BaseField,
}

// Manual trait impls to avoid derive adding bounds on C itself.
// Only C::BaseField needs these traits (guaranteed by ConstField: Copy + FiniteField).

impl<C: ExtConfig> Clone for QuadraticExt<C> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: ExtConfig> Copy for QuadraticExt<C> {}

impl<C: ExtConfig> PartialEq for QuadraticExt<C> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.c0 == other.c0 && self.c1 == other.c1
    }
}

impl<C: ExtConfig> Eq for QuadraticExt<C> {}

impl<C: ExtConfig> Hash for QuadraticExt<C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.c0.hash(state);
        self.c1.hash(state);
    }
}

impl<C: ExtConfig> QuadraticExt<C> {
    /// Creates a new element `c0 + c1·u`.
    ///
    /// # Arguments
    ///
    /// * `c0` - The constant component.
    /// * `c1` - The coefficient of `u`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6);
    /// }
    ///
    /// let a = QuadraticExt::<Cfg>::new(Fp::new(3), Fp::new(5));
    /// ```
    #[inline]
    pub const fn new(c0: C::BaseField, c1: C::BaseField) -> Self {
        Self { c0, c1 }
    }

    /// Returns the constant component `c0`.
    #[inline]
    pub const fn c0(&self) -> C::BaseField {
        self.c0
    }

    /// Returns the coefficient of `u` (component `c1`).
    #[inline]
    pub const fn c1(&self) -> C::BaseField {
        self.c1
    }

    /// Returns the conjugate: `c0 − c1·u`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6);
    /// }
    ///
    /// let a = QuadraticExt::<Cfg>::new(Fp::new(3), Fp::new(5));
    /// let conj = a.conjugate();
    /// assert_eq!(conj.c0().value(), 3);
    /// assert_eq!(conj.c1().value(), 2); // −5 mod 7 = 2
    /// ```
    pub fn conjugate(&self) -> Self {
        Self::new(self.c0, -self.c1)
    }

    /// Returns the field norm: `c0² − β·c1²` (a base field element).
    ///
    /// The norm is multiplicative: `N(a·b) = N(a)·N(b)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
    /// use gf2_core::field::FiniteField;
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6);
    /// }
    /// type Fq2 = QuadraticExt<Cfg>;
    ///
    /// // N(1 + u) = 1² − (−1)·1² = 1 + 1 = 2
    /// let a = Fq2::new(Fp::new(1), Fp::new(1));
    /// assert_eq!(a.norm().value(), 2);
    /// ```
    pub fn norm(&self) -> C::BaseField {
        let t0 = self.c0 * self.c0;
        let t1 = self.c1 * self.c1;
        t0 - C::mul_by_non_residue(t1)
    }
}

impl<C: ExtConfig> QuadraticExt<C> {
    /// Embeds a base field element into the extension: `a ↦ a + 0·u`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6);
    /// }
    ///
    /// let a = QuadraticExt::<Cfg>::from_base(Fp::new(3));
    /// assert_eq!(a.c0().value(), 3);
    /// assert_eq!(a.c1().value(), 0);
    /// ```
    #[inline]
    pub fn from_base(value: C::BaseField) -> Self {
        Self::new(value, C::BaseField::zero())
    }
}

// ---------------------------------------------------------------------------
// Display and Debug
// ---------------------------------------------------------------------------

impl<C: ExtConfig> fmt::Display for QuadraticExt<C>
where
    C::BaseField: fmt::Display,
{
    /// Formats as `"c0 + c1·u"`, omitting zero terms.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c0_zero = self.c0.is_zero();
        let c1_zero = self.c1.is_zero();
        match (c0_zero, c1_zero) {
            (true, true) => write!(f, "0"),
            (false, true) => write!(f, "{}", self.c0),
            (true, false) => write!(f, "{}·u", self.c1),
            (false, false) => write!(f, "{} + {}·u", self.c0, self.c1),
        }
    }
}

impl<C: ExtConfig> fmt::Debug for QuadraticExt<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QuadraticExt({:?}, {:?})", self.c0, self.c1)
    }
}

// ---------------------------------------------------------------------------
// Arithmetic operators
// ---------------------------------------------------------------------------

impl<C: ExtConfig> Add for QuadraticExt<C> {
    type Output = Self;

    /// Component-wise addition: `(a0+b0) + (a1+b1)·u`.
    ///
    /// # Complexity
    ///
    /// 2 base-field additions.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.c0 + rhs.c0, self.c1 + rhs.c1)
    }
}

impl<C: ExtConfig> Sub for QuadraticExt<C> {
    type Output = Self;

    /// Component-wise subtraction: `(a0−b0) + (a1−b1)·u`.
    ///
    /// # Complexity
    ///
    /// 2 base-field subtractions.
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.c0 - rhs.c0, self.c1 - rhs.c1)
    }
}

impl<C: ExtConfig> Neg for QuadraticExt<C> {
    type Output = Self;

    /// Component-wise negation: `(−a0) + (−a1)·u`.
    ///
    /// # Complexity
    ///
    /// 2 base-field negations.
    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.c0, -self.c1)
    }
}

impl<C: ExtConfig> Mul for QuadraticExt<C> {
    type Output = Self;

    /// Karatsuba multiplication using 3 base-field multiplications.
    ///
    /// # Complexity
    ///
    /// 3M + 5A + 1B (M = base mul, A = add/sub, B = mul_by_non_residue).
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let v0 = self.c0 * rhs.c0;
        let v1 = self.c1 * rhs.c1;
        let c0 = v0 + C::mul_by_non_residue(v1);
        let c1 = (self.c0 + self.c1) * (rhs.c0 + rhs.c1) - v0 - v1;
        Self::new(c0, c1)
    }
}

impl<C: ExtConfig> Div for QuadraticExt<C> {
    type Output = Self;

    /// Division via multiplicative inverse.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` is zero.
    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inv().expect("division by zero in QuadraticExt")
    }
}

// ---------------------------------------------------------------------------
// AddAssign
// ---------------------------------------------------------------------------

impl<C: ExtConfig> AddAssign for QuadraticExt<C> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<C: ExtConfig> AddAssign<&Self> for QuadraticExt<C> {
    #[inline]
    fn add_assign(&mut self, rhs: &Self) {
        *self = *self + *rhs;
    }
}

// ---------------------------------------------------------------------------
// Reference-forwarding operators (QuadraticExt is Copy, so dereference)
// ---------------------------------------------------------------------------

impl<C: ExtConfig> Add<&QuadraticExt<C>> for QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn add(self, rhs: &QuadraticExt<C>) -> QuadraticExt<C> {
        self + *rhs
    }
}

impl<C: ExtConfig> Add for &QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn add(self, rhs: Self) -> QuadraticExt<C> {
        *self + *rhs
    }
}

impl<C: ExtConfig> Sub<&QuadraticExt<C>> for QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn sub(self, rhs: &QuadraticExt<C>) -> QuadraticExt<C> {
        self - *rhs
    }
}

impl<C: ExtConfig> Sub for &QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn sub(self, rhs: Self) -> QuadraticExt<C> {
        *self - *rhs
    }
}

impl<C: ExtConfig> Mul<&QuadraticExt<C>> for QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn mul(self, rhs: &QuadraticExt<C>) -> QuadraticExt<C> {
        self * *rhs
    }
}

impl<C: ExtConfig> Mul for &QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn mul(self, rhs: Self) -> QuadraticExt<C> {
        *self * *rhs
    }
}

impl<C: ExtConfig> Div<&QuadraticExt<C>> for QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn div(self, rhs: &QuadraticExt<C>) -> QuadraticExt<C> {
        self / *rhs
    }
}

impl<C: ExtConfig> Div for &QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn div(self, rhs: Self) -> QuadraticExt<C> {
        *self / *rhs
    }
}

impl<C: ExtConfig> Neg for &QuadraticExt<C> {
    type Output = QuadraticExt<C>;
    #[inline]
    fn neg(self) -> QuadraticExt<C> {
        -(*self)
    }
}

// ---------------------------------------------------------------------------
// FiniteField implementation
// ---------------------------------------------------------------------------

impl<C: ExtConfig> FiniteField for QuadraticExt<C> {
    type Characteristic = <C::BaseField as FiniteField>::Characteristic;
    type Wide = QuadraticExtWide<<C::BaseField as FiniteField>::Wide>;

    #[inline]
    fn characteristic(&self) -> Self::Characteristic {
        self.c0.characteristic()
    }

    #[inline]
    fn extension_degree(&self) -> usize {
        2 * self.c0.extension_degree()
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.c0.is_zero() && self.c1.is_zero()
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.c0.is_one() && self.c1.is_zero()
    }

    /// Norm-based inversion: `a⁻¹ = conjugate(a) / norm(a)`.
    ///
    /// # Complexity
    ///
    /// 1I + 2S + 3M + 1B + 1A (I = base inversion).
    fn inv(&self) -> Option<Self> {
        let t0 = self.c0 * self.c0;
        let t1 = self.c1 * self.c1;
        let norm = t0 - C::mul_by_non_residue(t1);
        norm.inv()
            .map(|norm_inv| Self::new(self.c0 * norm_inv, -(self.c1 * norm_inv)))
    }

    #[inline]
    fn zero_like(&self) -> Self {
        let z = self.c0.zero_like();
        Self::new(z, z)
    }

    #[inline]
    fn one_like(&self) -> Self {
        Self::new(self.c0.one_like(), self.c0.zero_like())
    }

    #[inline]
    fn zero_hint() -> Option<Self> {
        // QuadraticExt is always ConstField; delegate to the typed zero.
        Some(<Self as ConstField>::zero())
    }

    /// Static cardinality hint: `2 · log2(|BaseField|)` (since
    /// `|F²| = |F|²`). Returns `None` only if the base field cannot
    /// supply its own static hint. See
    /// [`FiniteField::cardinality_log2_hint`].
    #[inline]
    fn cardinality_log2_hint() -> Option<u32> {
        C::BaseField::cardinality_log2_hint().and_then(|b| b.checked_mul(2))
    }

    /// Component-wise widening: each base coefficient is lifted via
    /// [`FiniteField::to_wide`].
    #[inline]
    fn to_wide(&self) -> Self::Wide {
        QuadraticExtWide::new(self.c0.to_wide(), self.c1.to_wide())
    }

    /// Karatsuba multiplication at the tower level followed by component-wise
    /// widening.
    ///
    /// This is the "practical" (Option 1) choice from the design plan: each
    /// individual product is fully reduced at the base field, but the resulting
    /// extension element is stored in the wide accumulator so that sums of
    /// many such products (dot products) can be accumulated without further
    /// per-product reduction. The accumulation budget is the base field's
    /// [`max_unreduced_additions`](FiniteField::max_unreduced_additions).
    ///
    /// # Complexity
    ///
    /// 3M + 5A + 1B in the base field plus 2 widening copies.
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
        )
    }

    /// Delegates to the base field: because each tower component accumulates
    /// independently, the safe per-component budget on unreduced products
    /// equals [`C::BaseField::max_unreduced_additions`].
    fn max_unreduced_additions() -> usize {
        C::BaseField::max_unreduced_additions()
    }
}

// ---------------------------------------------------------------------------
// ConstField implementation
// ---------------------------------------------------------------------------

impl<C: ExtConfig> ConstField for QuadraticExt<C> {
    #[inline]
    fn zero() -> Self {
        Self::new(C::BaseField::zero(), C::BaseField::zero())
    }

    #[inline]
    fn one() -> Self {
        Self::new(C::BaseField::one(), C::BaseField::zero())
    }

    #[inline]
    fn order() -> u128 {
        let base_order = C::BaseField::order();
        base_order * base_order
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
    // Test config: GF(7²) with β = 6 (= −1 mod 7)
    // -----------------------------------------------------------------------

    struct Fq2Config;
    impl ExtConfig for Fq2Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1

        #[inline]
        fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
            -x // fast path: β = −1
        }
    }
    type Fq2 = QuadraticExt<Fq2Config>;

    // -----------------------------------------------------------------------
    // Axiom test harness (required for success)
    // -----------------------------------------------------------------------

    #[test]
    fn test_quadratic_ext_fp7_field_axioms() {
        let strategy = (0..7u64, 0..7u64)
            .prop_map(|(c0, c1)| Fq2::new(Fp::new(c0), Fp::new(c1)))
            .boxed();
        test_const_field_axioms(strategy, 7);
    }

    // -----------------------------------------------------------------------
    // Construction and accessors
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_and_accessors() {
        let a = Fq2::new(Fp::new(3), Fp::new(5));
        assert_eq!(a.c0().value(), 3);
        assert_eq!(a.c1().value(), 5);
    }

    #[test]
    fn test_zero_and_one() {
        assert!(Fq2::zero().is_zero());
        assert!(Fq2::one().is_one());
        assert!(!Fq2::zero().is_one());
        assert!(!Fq2::one().is_zero());
    }

    // -----------------------------------------------------------------------
    // Embedding
    // -----------------------------------------------------------------------

    #[test]
    fn test_embedding() {
        // from_base embeds base field element as (k, 0)
        // Note: generic From<C::BaseField> cannot be implemented due to
        // coherence conflict with blanket From<T> for T.
        for k in 0..7u64 {
            let embedded = Fq2::from_base(Fp::<7>::new(k));
            assert_eq!(embedded, Fq2::new(Fp::new(k), Fp::new(0)));
        }
    }

    // -----------------------------------------------------------------------
    // Display and Debug
    // -----------------------------------------------------------------------

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Fq2::new(Fp::new(3), Fp::new(5))), "3 + 5·u");
        assert_eq!(format!("{}", Fq2::new(Fp::new(3), Fp::new(0))), "3");
        assert_eq!(format!("{}", Fq2::new(Fp::new(0), Fp::new(5))), "5·u");
        assert_eq!(format!("{}", Fq2::new(Fp::new(0), Fp::new(0))), "0");
    }

    #[test]
    fn test_debug() {
        // Debug uses {:?} on base-field coefficients so that tower
        // base fields without `fmt::Display` still produce a useful
        // debug representation. For `Fp<7>` the Debug format is
        // `Fp<7>(value)`.
        let a = Fq2::new(Fp::new(3), Fp::new(5));
        assert_eq!(format!("{:?}", a), "QuadraticExt(Fp<7>(3), Fp<7>(5))");
        assert_eq!(
            format!("{:?}", Fq2::zero()),
            "QuadraticExt(Fp<7>(0), Fp<7>(0))"
        );
    }

    // -----------------------------------------------------------------------
    // Extension degree, order, characteristic
    // -----------------------------------------------------------------------

    #[test]
    fn test_extension_degree() {
        assert_eq!(Fq2::one().extension_degree(), 2);
    }

    #[test]
    fn test_order() {
        assert_eq!(Fq2::order(), 49);
    }

    #[test]
    fn test_characteristic() {
        assert_eq!(Fq2::one().characteristic(), 7u64);
    }

    // -----------------------------------------------------------------------
    // Conjugate and norm
    // -----------------------------------------------------------------------

    #[test]
    fn test_conjugate() {
        let a = Fq2::new(Fp::new(3), Fp::new(5));
        let conj = a.conjugate();
        assert_eq!(conj.c0().value(), 3);
        assert_eq!(conj.c1().value(), 2); // −5 mod 7 = 2
    }

    #[test]
    fn test_norm_multiplicative() {
        for a0 in 0..7u64 {
            for a1 in 0..7u64 {
                let a = Fq2::new(Fp::new(a0), Fp::new(a1));
                if a.is_zero() {
                    continue;
                }
                for b0 in 0..7u64 {
                    for b1 in 0..7u64 {
                        let b = Fq2::new(Fp::new(b0), Fp::new(b1));
                        if b.is_zero() {
                            continue;
                        }
                        let n_ab = (a * b).norm();
                        let na_nb = a.norm() * b.norm();
                        assert_eq!(
                            n_ab, na_nb,
                            "N(a*b) != N(a)*N(b) for a=({a0},{a1}), b=({b0},{b1})"
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Known value tests (hand-computed for GF(7²) with β = −1)
    // -----------------------------------------------------------------------

    #[test]
    fn test_known_u_squared_is_beta() {
        // u² = β = 6 = −1 mod 7
        let u = Fq2::new(Fp::new(0), Fp::new(1));
        let u_sq = u * u;
        assert_eq!(u_sq, Fq2::from_base(Fp::new(6)));
    }

    #[test]
    fn test_known_conjugate_product() {
        // (1 + u)(1 − u) = 1 − u² = 1 − (−1) = 2
        let a = Fq2::new(Fp::new(1), Fp::new(1));
        let b = Fq2::new(Fp::new(1), Fp::new(6)); // 1 − u = 1 + 6·u
        assert_eq!(a * b, Fq2::from_base(Fp::new(2)));
    }

    #[test]
    fn test_known_multiplication() {
        // (3 + 2u)(4 + 5u) = 12 + 15u + 8u + 10u²
        //                   = 12 + 23u + 10·(−1)
        //                   = 2 + 23u = 2 + 2u (mod 7)
        let a = Fq2::new(Fp::new(3), Fp::new(2));
        let b = Fq2::new(Fp::new(4), Fp::new(5));
        let c = a * b;
        assert_eq!(c.c0().value(), 2);
        assert_eq!(c.c1().value(), 2);
    }

    // -----------------------------------------------------------------------
    // Exhaustive multiplication cross-check (all 49 × 49 pairs)
    // -----------------------------------------------------------------------

    #[test]
    fn test_karatsuba_matches_naive_exhaustive() {
        // Naive: (a0+a1·u)(b0+b1·u) = (a0·b0 + β·a1·b1) + (a0·b1 + a1·b0)·u
        // with β = 6 mod 7
        for a0 in 0..7u64 {
            for a1 in 0..7u64 {
                for b0 in 0..7u64 {
                    for b1 in 0..7u64 {
                        let a = Fq2::new(Fp::new(a0), Fp::new(a1));
                        let b = Fq2::new(Fp::new(b0), Fp::new(b1));
                        let c = a * b;

                        let naive_c0 = (a0 * b0 + 6 * a1 * b1) % 7;
                        let naive_c1 = (a0 * b1 + a1 * b0) % 7;

                        assert_eq!(
                            c.c0().value(),
                            naive_c0,
                            "c0 mismatch: ({a0}+{a1}u)*({b0}+{b1}u)"
                        );
                        assert_eq!(
                            c.c1().value(),
                            naive_c1,
                            "c1 mismatch: ({a0}+{a1}u)*({b0}+{b1}u)"
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Exhaustive inversion round-trip (all 48 non-zero elements)
    // -----------------------------------------------------------------------

    #[test]
    fn test_inversion_roundtrip_exhaustive() {
        let one = Fq2::one();
        for c0 in 0..7u64 {
            for c1 in 0..7u64 {
                let a = Fq2::new(Fp::new(c0), Fp::new(c1));
                if a.is_zero() {
                    assert!(a.inv().is_none());
                    continue;
                }
                let inv = a.inv().expect("non-zero element must have inverse");
                assert_eq!(a * inv, one, "a * inv(a) != 1 for a = ({c0}, {c1})");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Reference operators
    // -----------------------------------------------------------------------

    #[test]
    #[allow(clippy::op_ref)]
    fn test_ref_operators() {
        let a = Fq2::new(Fp::new(3), Fp::new(5));
        let b = Fq2::new(Fp::new(2), Fp::new(4));
        assert_eq!(a + b, &a + &b);
        assert_eq!(a + &b, &a + &b);
        assert_eq!(a - b, &a - &b);
        assert_eq!(a * b, &a * &b);
        assert_eq!(a / b, &a / &b);
        assert_eq!(-a, -&a);
    }

    #[test]
    fn test_add_assign() {
        let mut a = Fq2::new(Fp::new(3), Fp::new(5));
        a += Fq2::new(Fp::new(2), Fp::new(4));
        assert_eq!(a, Fq2::new(Fp::new(5), Fp::new(2)));
    }

    #[test]
    fn test_add_assign_ref() {
        let mut a = Fq2::new(Fp::new(3), Fp::new(5));
        let b = Fq2::new(Fp::new(2), Fp::new(4));
        a += &b;
        assert_eq!(a, Fq2::new(Fp::new(5), Fp::new(2)));
        assert_eq!(b.c0().value(), 2); // b still valid
    }

    // -----------------------------------------------------------------------
    // Size: no runtime overhead
    // -----------------------------------------------------------------------

    #[test]
    fn test_size_of() {
        assert_eq!(std::mem::size_of::<Fq2>(), 2 * std::mem::size_of::<Fp<7>>());
    }

    // -----------------------------------------------------------------------
    // Wide accumulator tests (issue d11b769a)
    // -----------------------------------------------------------------------

    /// Wide is a real two-component accumulator, not an alias for `Self`.
    #[test]
    fn test_wide_type_is_not_self() {
        // Same memory footprint as two base-field Wide values.
        assert_eq!(
            std::mem::size_of::<<Fq2 as FiniteField>::Wide>(),
            2 * std::mem::size_of::<<Fp<7> as FiniteField>::Wide>(),
        );
    }

    /// `max_unreduced_additions` must equal the base field's bound, regardless
    /// of the underlying prime. For GF(7) the bound saturates at `usize::MAX`
    /// because `(p-1)²` is tiny compared to `u128::MAX`; the meaningful
    /// assertion — that the bound is *finite* — is covered by the Mersenne-61
    /// test below.
    #[test]
    fn test_max_unreduced_additions_is_base_field_bound() {
        let k = <Fq2 as FiniteField>::max_unreduced_additions();
        let base = <Fp<7> as FiniteField>::max_unreduced_additions();
        assert_eq!(k, base);
    }

    /// For a large enough prime, `max_unreduced_additions()` is finite (not
    /// `usize::MAX`). This proves the tower no longer returns the sentinel
    /// `usize::MAX` placeholder that the old `Wide = Self` implementation
    /// used to hand out.
    #[test]
    fn test_max_unreduced_additions_finite_for_large_prime() {
        // GF(Mersenne61²) with β = 2.
        struct MConfig;
        impl ExtConfig for MConfig {
            type BaseField = Fp<2305843009213693951>;
            const NON_RESIDUE: Fp<2305843009213693951> = Fp::<2305843009213693951>::new(2);
        }
        type MFq2 = QuadraticExt<MConfig>;

        let k = <MFq2 as FiniteField>::max_unreduced_additions();
        let base = <Fp<2305843009213693951> as FiniteField>::max_unreduced_additions();
        assert_eq!(k, base, "tower bound must equal base field bound");
        assert_ne!(
            k,
            usize::MAX,
            "bound must be finite for a large prime (no usize::MAX sentinel)"
        );
        assert!(k >= 1);
    }

    /// `reduce_wide(to_wide(a)) == a` for all 49 elements of GF(7²).
    #[test]
    fn test_wide_roundtrip_exhaustive() {
        for c0 in 0..7u64 {
            for c1 in 0..7u64 {
                let a = Fq2::new(Fp::new(c0), Fp::new(c1));
                let wide = a.to_wide();
                let back = <Fq2 as FiniteField>::reduce_wide(&wide);
                assert_eq!(back, a, "roundtrip failed at ({c0}, {c1})");
            }
        }
    }

    /// `reduce_wide(mul_to_wide(a, b)) == a * b` for all 49×49 pairs.
    #[test]
    fn test_mul_to_wide_consistency_exhaustive() {
        for a0 in 0..7u64 {
            for a1 in 0..7u64 {
                for b0 in 0..7u64 {
                    for b1 in 0..7u64 {
                        let a = Fq2::new(Fp::new(a0), Fp::new(a1));
                        let b = Fq2::new(Fp::new(b0), Fp::new(b1));
                        let wide = a.mul_to_wide(&b);
                        let reduced = <Fq2 as FiniteField>::reduce_wide(&wide);
                        assert_eq!(
                            reduced,
                            a * b,
                            "mul_to_wide failed at ({a0},{a1})·({b0},{b1})"
                        );
                    }
                }
            }
        }
    }

    /// Dot-product accumulation: N products summed in wide, reduced once,
    /// matches element-wise multiply-and-add.
    #[test]
    fn test_dot_product_accumulation_proptest() {
        // Generate ≥200 cases of a random-length dot product (1..=32 terms).
        let mut runner =
            proptest::test_runner::TestRunner::new(proptest::test_runner::Config::with_cases(200));
        let strategy = proptest::collection::vec((0..7u64, 0..7u64, 0..7u64, 0..7u64), 1..=32);
        runner
            .run(&strategy, |pairs| {
                let mut acc: <Fq2 as FiniteField>::Wide = <Fq2 as FiniteField>::Wide::default();
                let mut expected = Fq2::zero();
                for (a0, a1, b0, b1) in &pairs {
                    let a = Fq2::new(Fp::new(*a0), Fp::new(*a1));
                    let b = Fq2::new(Fp::new(*b0), Fp::new(*b1));
                    acc += a.mul_to_wide(&b);
                    expected += a * b;
                }
                let got = <Fq2 as FiniteField>::reduce_wide(&acc);
                prop_assert_eq!(got, expected);
                Ok(())
            })
            .expect("dot-product accumulation must match element-wise");
    }

    /// `QuadraticExtWide`-level Add and AddAssign are component-wise.
    #[test]
    fn test_wide_add_and_add_assign() {
        let w1 = <Fq2 as FiniteField>::Wide::new(10u128, 20u128);
        let w2 = <Fq2 as FiniteField>::Wide::new(3u128, 5u128);
        let sum = w1 + w2;
        assert_eq!(*sum.c0(), 13u128);
        assert_eq!(*sum.c1(), 25u128);

        let mut w = <Fq2 as FiniteField>::Wide::new(10u128, 20u128);
        w += <Fq2 as FiniteField>::Wide::new(1u128, 2u128);
        assert_eq!(*w.c0(), 11u128);
        assert_eq!(*w.c1(), 22u128);
    }

    /// `QuadraticExtWide` Debug output is informative and not `Self`-typed.
    #[test]
    fn test_wide_debug_format() {
        let w = <Fq2 as FiniteField>::Wide::new(10u128, 20u128);
        let s = format!("{:?}", w);
        assert!(s.contains("QuadraticExtWide"), "unexpected debug: {s}");
        assert!(s.contains("10"));
        assert!(s.contains("20"));
    }
}
