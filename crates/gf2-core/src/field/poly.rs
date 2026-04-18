//! Generic univariate polynomials over any [`FiniteField`].
//!
//! `FieldPoly<F>` is the generic sibling of the specialised
//! [`Gf2mPoly`](crate::gf2m::Gf2mPoly) type in `gf2-core`. It stores
//! coefficients in **ascending-degree** order (`coeffs[i]` is the
//! coefficient of `x^i`) and supports any field type that implements the
//! [`FiniteField`] trait — binary extension fields
//! ([`Gf2mElement`](crate::gf2m::Gf2mElement)), prime fields
//! ([`Fp<P>`](crate::gfp::Fp)), and future tower extensions all compose
//! uniformly.
//!
//! # The normalisation invariant
//!
//! **Every** constructor and every mutating operation on a `FieldPoly`
//! leaves the polynomial in *normalised* form: the `coeffs` vector has no
//! trailing zero coefficients. The zero polynomial is the unique
//! polynomial with an empty `coeffs` vector; every other polynomial's
//! final coefficient is non-zero.
//!
//! This is the polynomial analogue of the [`BitVec`](crate::BitVec)
//! `mask_tail` invariant and is the single most important correctness
//! rule in this module. All arithmetic (`Add`, `Sub`, `Neg`, `Mul`,
//! `scale`, …) is written so that it calls [`FieldPoly::normalise`]
//! before returning. Equality is *structural* — two `FieldPoly`s are
//! equal iff their normalised `coeffs` slices compare equal element-wise.
//!
//! # Scope of this module (Task 1 of the `bdf95060` story)
//!
//! This file deliberately contains only the **core type** and its
//! **basic arithmetic surface**:
//!
//! - Construction: [`FieldPoly::new`], [`FieldPoly::zero_like`],
//!   [`FieldPoly::one_like`], [`FieldPoly::constant`],
//!   [`FieldPoly::monomial`], [`FieldPoly::from_coeffs_trimmed`].
//! - Queries: [`FieldPoly::degree`], [`FieldPoly::is_zero`],
//!   [`FieldPoly::coeff`], [`FieldPoly::leading_coeff`],
//!   [`FieldPoly::len`], [`FieldPoly::iter`].
//! - Operator overloads: `Add`, `Sub`, `Neg`, `AddAssign`, `SubAssign`
//!   in both owned and borrowed RHS forms.
//! - Scalar multiplication: [`FieldPoly::mul_scalar`],
//!   [`FieldPoly::scale`].
//! - Schoolbook polynomial multiplication through the `Mul` operator.
//!
//! The following capabilities are **out of scope** for this module file
//! and land in later tasks (`crates/gf2-core/src/field/poly.rs`
//! extensions or sibling files):
//!
//! - Karatsuba multiplication, Euclidean division and GCD, Horner
//!   evaluation (Task 2).
//! - Subproduct-tree batch evaluation (Task 3).
//! - Lagrange interpolation (Task 4).
//! - Balanced product tree / batch GCD (Task 5).
//! - NTT-friendly field trait and NTT-based multiplication
//!   (Tasks 6 and 7).
//!
//! The `Mul` implementation here is therefore plain O(n · m)
//! schoolbook; Task 2 will replace it with a Karatsuba dispatch sharing
//! the same operator surface.

use crate::field::FiniteField;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// A univariate polynomial over a finite field `F`.
///
/// Coefficients are stored in ascending-degree order: `coeffs[i]` is the
/// coefficient of `x^i`. The coefficient vector is **always normalised**
/// — empty (for the zero polynomial) or non-empty with a non-zero
/// trailing element.
///
/// See the [module documentation](self) for the full invariant statement
/// and the scope of this file.
///
/// # Examples
///
/// ```
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// // 2x + 3 over Fp<7>
/// let p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]);
/// assert_eq!(p.degree(), Some(1));
/// assert_eq!(p.coeff(0), Fp::<7>::new(3));
/// assert_eq!(p.coeff(1), Fp::<7>::new(2));
/// ```
// The semantic "empty" predicate for a polynomial is `is_zero` (see the
// normalisation invariant in the module docs): the zero polynomial is
// exactly the one with no stored coefficients. Clippy's
// `len_without_is_empty` would push us to spell that as `is_empty`, but
// `is_zero` is both more meaningful for readers and matches the
// `Gf2mPoly` convention already established in this crate.
#[allow(clippy::len_without_is_empty)]
#[derive(Clone)]
pub struct FieldPoly<F: FiniteField> {
    coeffs: Vec<F>,
}

impl<F: FiniteField> FieldPoly<F> {
    // -----------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------

    /// Creates a polynomial from a coefficient vector, trimming trailing
    /// zero coefficients.
    ///
    /// The input is in ascending-degree order: `coeffs[i]` is the
    /// coefficient of `x^i`. Trailing zero coefficients are stripped so
    /// that the returned polynomial satisfies the
    /// [module normalisation invariant](self).
    ///
    /// # Arguments
    ///
    /// * `coeffs` — coefficient vector in ascending-degree order.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // Trailing zeros are removed.
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(0), Fp::<7>::new(0)]);
    /// assert_eq!(p.degree(), Some(0));
    /// assert_eq!(p.len(), 1);
    /// ```
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // An all-zero input yields the zero polynomial.
    /// let p = FieldPoly::new(vec![Fp::<7>::new(0); 5]);
    /// assert!(p.is_zero());
    /// assert_eq!(p.degree(), None);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` in the length of `coeffs`.
    pub fn new(coeffs: Vec<F>) -> Self {
        let mut poly = FieldPoly { coeffs };
        poly.normalise();
        poly
    }

    /// Explicitly trimmed constructor; semantically identical to
    /// [`FieldPoly::new`].
    ///
    /// Both names are exposed so that call sites can communicate intent:
    /// `new` is the general-purpose constructor, while
    /// `from_coeffs_trimmed` documents that the caller is relying on the
    /// routine to strip trailing zeros. A future "trust me, already
    /// normalised" variant (if added) would live alongside these two,
    /// which is why the explicit name exists today.
    ///
    /// # Arguments
    ///
    /// * `coeffs` — coefficient vector in ascending-degree order.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::from_coeffs_trimmed(vec![
    ///     Fp::<7>::new(4),
    ///     Fp::<7>::new(0),
    ///     Fp::<7>::new(0),
    /// ]);
    /// assert_eq!(p.degree(), Some(0));
    /// assert_eq!(p.coeff(0), Fp::<7>::new(4));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` in the length of `coeffs`.
    pub fn from_coeffs_trimmed(coeffs: Vec<F>) -> Self {
        Self::new(coeffs)
    }

    /// Returns the zero polynomial in the same field as `sample`.
    ///
    /// The `sample` parameter is accepted — instead of using
    /// [`Default`] or a static constant — because some field types
    /// (notably [`Gf2mElement_<V>`](crate::gf2m::Gf2mElement_)) carry a
    /// runtime handle on the field parameters. Passing a sample lets
    /// callers construct polynomials over runtime-configured fields
    /// without any static registration.
    ///
    /// For field types that *don't* need the sample, it is simply
    /// ignored: the zero polynomial has an empty `coeffs` vector.
    ///
    /// # Arguments
    ///
    /// * `_sample` — any field element; only consumed to nail down the
    ///   type parameter `F` (and, for runtime-configured fields, to
    ///   carry their field context into any subsequent operations on
    ///   the returned zero polynomial via [`FieldPoly::coeff`] fallbacks
    ///   once additional coefficients appear).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert!(z.is_zero());
    /// assert_eq!(z.degree(), None);
    /// assert_eq!(z.len(), 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn zero_like(_sample: &F) -> Self {
        FieldPoly { coeffs: Vec::new() }
    }

    /// Returns the constant-`1` polynomial in the same field as
    /// `sample`.
    ///
    /// # Arguments
    ///
    /// * `sample` — any field element; used to obtain a `one` element
    ///   in the same field via [`FiniteField::one_like`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p: FieldPoly<Fp<7>> = FieldPoly::one_like(&Fp::<7>::new(0));
    /// assert_eq!(p.degree(), Some(0));
    /// assert_eq!(p.coeff(0), Fp::<7>::new(1));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn one_like(sample: &F) -> Self {
        FieldPoly {
            coeffs: vec![sample.one_like()],
        }
    }

    /// Creates a constant (degree-0) polynomial `c`.
    ///
    /// If `c` is the zero element the result is the zero polynomial
    /// (empty `coeffs` vector), satisfying the
    /// [normalisation invariant](self).
    ///
    /// # Arguments
    ///
    /// * `c` — the constant value.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::constant(Fp::<7>::new(5));
    /// assert_eq!(p.degree(), Some(0));
    /// assert_eq!(p.coeff(0), Fp::<7>::new(5));
    ///
    /// let z = FieldPoly::constant(Fp::<7>::new(0));
    /// assert!(z.is_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn constant(c: F) -> Self {
        if c.is_zero() {
            FieldPoly { coeffs: Vec::new() }
        } else {
            FieldPoly { coeffs: vec![c] }
        }
    }

    /// Creates the monomial `coeff · x^degree`.
    ///
    /// If `coeff` is the zero element the result is the zero polynomial
    /// regardless of `degree`.
    ///
    /// # Arguments
    ///
    /// * `coeff` — coefficient of the single non-zero term.
    /// * `degree` — exponent of `x`; may be `0`, in which case the
    ///   result is equivalent to [`FieldPoly::constant`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // 3·x^4
    /// let p = FieldPoly::monomial(Fp::<7>::new(3), 4);
    /// assert_eq!(p.degree(), Some(4));
    /// assert_eq!(p.coeff(0), Fp::<7>::new(0));
    /// assert_eq!(p.coeff(4), Fp::<7>::new(3));
    /// ```
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // 0·x^5 = 0
    /// let z = FieldPoly::monomial(Fp::<7>::new(0), 5);
    /// assert!(z.is_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(degree)` to allocate and zero-fill the coefficient vector.
    pub fn monomial(coeff: F, degree: usize) -> Self {
        if coeff.is_zero() {
            return FieldPoly { coeffs: Vec::new() };
        }
        let zero = coeff.zero_like();
        let mut coeffs = vec![zero; degree + 1];
        coeffs[degree] = coeff;
        FieldPoly { coeffs }
    }

    // -----------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------

    /// Returns the degree of the polynomial, or `None` for the zero
    /// polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert_eq!(z.degree(), None);
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(2)]);
    /// assert_eq!(p.degree(), Some(1));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn degree(&self) -> Option<usize> {
        if self.coeffs.is_empty() {
            None
        } else {
            Some(self.coeffs.len() - 1)
        }
    }

    /// Returns `true` iff this is the zero polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert!(z.is_zero());
    ///
    /// let p = FieldPoly::constant(Fp::<7>::new(1));
    /// assert!(!p.is_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn is_zero(&self) -> bool {
        self.coeffs.is_empty()
    }

    /// Returns the coefficient of `x^i` as an owned value.
    ///
    /// If `i` is greater than the current degree the method returns a
    /// zero element obtained from an existing stored coefficient (via
    /// [`FiniteField::zero_like`]).
    ///
    /// # Arguments
    ///
    /// * `i` — exponent of the requested coefficient.
    ///
    /// # Panics
    ///
    /// Panics if the polynomial is the zero polynomial (`coeffs` is
    /// empty) — no stored coefficient exists from which to derive a
    /// runtime-contextualised zero. Callers in that situation must
    /// instead use [`FieldPoly::is_zero`] to branch, or keep their own
    /// zero sample. This constraint is specific to field types that
    /// carry a runtime field handle (e.g.
    /// [`Gf2mElement`](crate::gf2m::Gf2mElement)); `Fp<P>` never hits
    /// it in practice because `Fp::<P>::new(0)` is always available.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // 2x + 3
    /// let p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]);
    /// assert_eq!(p.coeff(0), Fp::<7>::new(3));
    /// assert_eq!(p.coeff(1), Fp::<7>::new(2));
    /// // Out-of-range: returns zero.
    /// assert_eq!(p.coeff(10), Fp::<7>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn coeff(&self, i: usize) -> F {
        if i < self.coeffs.len() {
            self.coeffs[i].clone()
        } else {
            // Out-of-range: conjure a zero in the same field by using
            // any stored coefficient as a "witness". For the zero
            // polynomial this path would panic because `coeffs[0]`
            // does not exist — see the `# Panics` section.
            assert!(
                !self.coeffs.is_empty(),
                "FieldPoly::coeff: cannot return zero_like on the zero \
                 polynomial; callers must branch on is_zero()"
            );
            self.coeffs[0].zero_like()
        }
    }

    /// Returns a reference to the leading (highest-degree) coefficient,
    /// or `None` for the zero polynomial.
    ///
    /// By the [normalisation invariant](self), the returned reference
    /// is never to a zero element.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(5)]);
    /// assert_eq!(p.leading_coeff(), Some(&Fp::<7>::new(5)));
    ///
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert!(z.leading_coeff().is_none());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn leading_coeff(&self) -> Option<&F> {
        self.coeffs.last()
    }

    /// Returns the number of stored coefficients (`degree + 1` for a
    /// non-zero polynomial, `0` for the zero polynomial).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(3)]);
    /// assert_eq!(p.len(), 3);
    ///
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert_eq!(z.len(), 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn len(&self) -> usize {
        self.coeffs.len()
    }

    /// Returns an iterator over the coefficients in ascending-degree
    /// order.
    ///
    /// The iterator yields exactly [`FieldPoly::len`] items. For the
    /// zero polynomial it yields nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(3)]);
    /// let collected: Vec<Fp<7>> = p.iter().cloned().collect();
    /// assert_eq!(collected.len(), 3);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)` to construct; iteration is `O(n)` overall.
    pub fn iter(&self) -> impl Iterator<Item = &F> {
        self.coeffs.iter()
    }

    // -----------------------------------------------------------------
    // Scalar multiplication
    // -----------------------------------------------------------------

    /// Returns `self` multiplied by a scalar.
    ///
    /// If `c` is the zero element the result is the zero polynomial,
    /// preserving the [normalisation invariant](self).
    ///
    /// # Arguments
    ///
    /// * `c` — scalar multiplier.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // (2x + 3) * 2 = 4x + 6 over Fp<7>
    /// let p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]);
    /// let q = p.mul_scalar(&Fp::<7>::new(2));
    /// assert_eq!(q.coeff(0), Fp::<7>::new(6));
    /// assert_eq!(q.coeff(1), Fp::<7>::new(4));
    ///
    /// // Multiplying by zero produces the zero polynomial.
    /// let z = p.mul_scalar(&Fp::<7>::new(0));
    /// assert!(z.is_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` field multiplications, where `n = self.len()`.
    pub fn mul_scalar(&self, c: &F) -> Self {
        if c.is_zero() || self.is_zero() {
            return FieldPoly { coeffs: Vec::new() };
        }
        let coeffs: Vec<F> = self.coeffs.iter().map(|a| a.clone() * c.clone()).collect();
        FieldPoly::new(coeffs)
    }

    /// Multiplies this polynomial in place by a scalar.
    ///
    /// Equivalent to `*self = self.mul_scalar(c)`; the result satisfies
    /// the [normalisation invariant](self). Multiplying by zero
    /// collapses the polynomial to zero.
    ///
    /// # Arguments
    ///
    /// * `c` — scalar multiplier.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]);
    /// p.scale(&Fp::<7>::new(2));
    /// assert_eq!(p.coeff(0), Fp::<7>::new(6));
    /// assert_eq!(p.coeff(1), Fp::<7>::new(4));
    ///
    /// p.scale(&Fp::<7>::new(0));
    /// assert!(p.is_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` field multiplications.
    pub fn scale(&mut self, c: &F) {
        if c.is_zero() {
            self.coeffs.clear();
            return;
        }
        for a in self.coeffs.iter_mut() {
            *a = a.clone() * c.clone();
        }
        self.normalise();
    }

    // -----------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------

    /// Trims trailing zero coefficients so the invariant holds.
    ///
    /// Called at the end of every constructor and mutating operation.
    fn normalise(&mut self) {
        while let Some(last) = self.coeffs.last() {
            if last.is_zero() {
                self.coeffs.pop();
            } else {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------
// Equality: structural, after normalisation
// ---------------------------------------------------------------------

impl<F: FiniteField> PartialEq for FieldPoly<F> {
    fn eq(&self, other: &Self) -> bool {
        // Both sides are normalised by construction; a length check is
        // sufficient to short-circuit mismatches.
        self.coeffs == other.coeffs
    }
}

impl<F: FiniteField> Eq for FieldPoly<F> {}

// ---------------------------------------------------------------------
// Debug: descending-degree with non-zero terms only; "0" for the zero
// polynomial.
// ---------------------------------------------------------------------

impl<F: FiniteField> fmt::Debug for FieldPoly<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.coeffs.is_empty() {
            return f.write_str("0");
        }

        let mut first = true;
        // Iterate from highest to lowest degree.
        for i in (0..self.coeffs.len()).rev() {
            let c = &self.coeffs[i];
            if c.is_zero() {
                continue;
            }
            if !first {
                f.write_str(" + ")?;
            }
            first = false;
            match i {
                0 => write!(f, "{c:?}")?,
                1 => {
                    if c.is_one() {
                        f.write_str("x")?;
                    } else {
                        write!(f, "{c:?}x")?;
                    }
                }
                _ => {
                    if c.is_one() {
                        write!(f, "x^{i}")?;
                    } else {
                        write!(f, "{c:?}x^{i}")?;
                    }
                }
            }
        }
        // If every coefficient was zero (shouldn't happen given the
        // invariant, but keeps Debug total) we fall through to "0".
        if first {
            f.write_str("0")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Addition
// ---------------------------------------------------------------------

/// Helper: coefficient-wise add of two slices, returning a new
/// normalised polynomial. `rhs_sign` is `true` for `+`, `false` for `-`.
fn add_impl<F: FiniteField>(lhs: &[F], rhs: &[F], rhs_is_neg: bool) -> FieldPoly<F> {
    let max_len = lhs.len().max(rhs.len());
    let mut coeffs: Vec<F> = Vec::with_capacity(max_len);
    for i in 0..max_len {
        let a = lhs.get(i);
        let b = rhs.get(i);
        let merged = match (a, b) {
            (Some(a), Some(b)) => {
                if rhs_is_neg {
                    a.clone() - b.clone()
                } else {
                    a.clone() + b.clone()
                }
            }
            (Some(a), None) => a.clone(),
            (None, Some(b)) => {
                if rhs_is_neg {
                    -b.clone()
                } else {
                    b.clone()
                }
            }
            (None, None) => unreachable!(),
        };
        coeffs.push(merged);
    }
    FieldPoly::new(coeffs)
}

impl<F: FiniteField> Add<FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

    /// Adds two polynomials coefficient-wise.
    ///
    /// # Complexity
    ///
    /// `O(max(n, m))` field additions.
    fn add(self, rhs: FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, false)
    }
}

impl<'a, F: FiniteField> Add<&'a FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn add(self, rhs: &'a FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, false)
    }
}

impl<F: FiniteField> Add<FieldPoly<F>> for &FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn add(self, rhs: FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, false)
    }
}

impl<'b, F: FiniteField> Add<&'b FieldPoly<F>> for &FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn add(self, rhs: &'b FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, false)
    }
}

// ---------------------------------------------------------------------
// Subtraction
// ---------------------------------------------------------------------

impl<F: FiniteField> Sub<FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

    /// Subtracts `rhs` from `self` coefficient-wise.
    ///
    /// # Complexity
    ///
    /// `O(max(n, m))` field operations.
    fn sub(self, rhs: FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, true)
    }
}

impl<'a, F: FiniteField> Sub<&'a FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn sub(self, rhs: &'a FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, true)
    }
}

impl<F: FiniteField> Sub<FieldPoly<F>> for &FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn sub(self, rhs: FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, true)
    }
}

impl<'b, F: FiniteField> Sub<&'b FieldPoly<F>> for &FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn sub(self, rhs: &'b FieldPoly<F>) -> FieldPoly<F> {
        add_impl(&self.coeffs, &rhs.coeffs, true)
    }
}

// ---------------------------------------------------------------------
// Negation
// ---------------------------------------------------------------------

impl<F: FiniteField> Neg for FieldPoly<F> {
    type Output = FieldPoly<F>;

    /// Returns the additive inverse, coefficient-wise.
    ///
    /// # Complexity
    ///
    /// `O(n)` field negations. The result is already normalised because
    /// negation preserves the "last coefficient non-zero" property: if
    /// the input's trailing coefficient was non-zero, its negation is
    /// non-zero too (fields have no zero divisors).
    fn neg(self) -> FieldPoly<F> {
        let coeffs: Vec<F> = self.coeffs.into_iter().map(|c| -c).collect();
        // Negation is an additive-group bijection, so the "no trailing
        // zeros" invariant is preserved — but we still run normalise()
        // defensively in case `FiniteField::neg` yields a fresh zero
        // for some exotic implementation.
        FieldPoly::new(coeffs)
    }
}

impl<F: FiniteField> Neg for &FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn neg(self) -> FieldPoly<F> {
        let coeffs: Vec<F> = self.coeffs.iter().map(|c| -c.clone()).collect();
        FieldPoly::new(coeffs)
    }
}

// ---------------------------------------------------------------------
// AddAssign / SubAssign
// ---------------------------------------------------------------------

impl<F: FiniteField> AddAssign<FieldPoly<F>> for FieldPoly<F> {
    fn add_assign(&mut self, rhs: FieldPoly<F>) {
        *self = add_impl(&self.coeffs, &rhs.coeffs, false);
    }
}

impl<'a, F: FiniteField> AddAssign<&'a FieldPoly<F>> for FieldPoly<F> {
    fn add_assign(&mut self, rhs: &'a FieldPoly<F>) {
        *self = add_impl(&self.coeffs, &rhs.coeffs, false);
    }
}

impl<F: FiniteField> SubAssign<FieldPoly<F>> for FieldPoly<F> {
    fn sub_assign(&mut self, rhs: FieldPoly<F>) {
        *self = add_impl(&self.coeffs, &rhs.coeffs, true);
    }
}

impl<'a, F: FiniteField> SubAssign<&'a FieldPoly<F>> for FieldPoly<F> {
    fn sub_assign(&mut self, rhs: &'a FieldPoly<F>) {
        *self = add_impl(&self.coeffs, &rhs.coeffs, true);
    }
}

// ---------------------------------------------------------------------
// Schoolbook multiplication
// ---------------------------------------------------------------------

/// Core schoolbook polynomial multiplication, O(n · m) in field mults.
///
/// Karatsuba and NTT dispatches are deferred to later tasks in the
/// `bdf95060` story; this routine is what the `Mul` operator calls
/// today.
fn mul_impl<F: FiniteField>(lhs: &[F], rhs: &[F]) -> FieldPoly<F> {
    if lhs.is_empty() || rhs.is_empty() {
        return FieldPoly { coeffs: Vec::new() };
    }

    // Pre-allocate the result: degree = (n-1) + (m-1), length = n + m - 1.
    let zero = lhs[0].zero_like();
    let out_len = lhs.len() + rhs.len() - 1;
    let mut coeffs: Vec<F> = vec![zero; out_len];

    for (i, a) in lhs.iter().enumerate() {
        if a.is_zero() {
            continue;
        }
        for (j, b) in rhs.iter().enumerate() {
            if b.is_zero() {
                continue;
            }
            // coeffs[i+j] += a * b
            let prod = a.clone() * b.clone();
            coeffs[i + j] += prod;
        }
    }

    FieldPoly::new(coeffs)
}

impl<F: FiniteField> Mul<FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

    /// Schoolbook polynomial multiplication.
    ///
    /// # Complexity
    ///
    /// `O(n · m)` field multiplications, where `n = self.len()` and
    /// `m = rhs.len()`. See the [module documentation](self) for why
    /// Karatsuba / NTT are not dispatched here.
    fn mul(self, rhs: FieldPoly<F>) -> FieldPoly<F> {
        mul_impl(&self.coeffs, &rhs.coeffs)
    }
}

impl<'a, F: FiniteField> Mul<&'a FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn mul(self, rhs: &'a FieldPoly<F>) -> FieldPoly<F> {
        mul_impl(&self.coeffs, &rhs.coeffs)
    }
}

impl<F: FiniteField> Mul<FieldPoly<F>> for &FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn mul(self, rhs: FieldPoly<F>) -> FieldPoly<F> {
        mul_impl(&self.coeffs, &rhs.coeffs)
    }
}

impl<'b, F: FiniteField> Mul<&'b FieldPoly<F>> for &FieldPoly<F> {
    type Output = FieldPoly<F>;

    fn mul(self, rhs: &'b FieldPoly<F>) -> FieldPoly<F> {
        mul_impl(&self.coeffs, &rhs.coeffs)
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::{Gf2mElement, Gf2mField};
    use crate::gfp::Fp;
    use proptest::prelude::*;

    // Shortcut aliases for test brevity.
    type FP7 = Fp<7>;

    fn fp7(n: u64) -> FP7 {
        FP7::new(n)
    }

    // -----------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------

    #[test]
    fn test_new_trims_trailing_zeros() {
        let p = FieldPoly::new(vec![fp7(1), fp7(0), fp7(0)]);
        assert_eq!(p.degree(), Some(0));
        assert_eq!(p.len(), 1);
        // Structural equality with `constant`.
        assert_eq!(p, FieldPoly::constant(fp7(1)));
    }

    #[test]
    fn test_new_all_zero_is_zero() {
        let p: FieldPoly<FP7> = FieldPoly::new(vec![fp7(0); 5]);
        assert!(p.is_zero());
        assert_eq!(p.degree(), None);
        assert_eq!(p.len(), 0);
        assert!(p.leading_coeff().is_none());
    }

    #[test]
    fn test_new_empty_is_zero() {
        let p: FieldPoly<FP7> = FieldPoly::new(vec![]);
        assert!(p.is_zero());
    }

    #[test]
    fn test_from_coeffs_trimmed_same_as_new() {
        let a = FieldPoly::from_coeffs_trimmed(vec![fp7(3), fp7(0)]);
        let b = FieldPoly::new(vec![fp7(3), fp7(0)]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_zero_like() {
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert!(z.is_zero());
        assert_eq!(z.degree(), None);
        assert_eq!(z.len(), 0);
    }

    #[test]
    fn test_one_like() {
        let o: FieldPoly<FP7> = FieldPoly::one_like(&fp7(0));
        assert_eq!(o.degree(), Some(0));
        assert_eq!(o.coeff(0), fp7(1));
    }

    #[test]
    fn test_constant() {
        let p = FieldPoly::constant(fp7(5));
        assert_eq!(p.degree(), Some(0));
        assert_eq!(p.coeff(0), fp7(5));
    }

    #[test]
    fn test_constant_zero_collapses() {
        let p = FieldPoly::constant(fp7(0));
        assert!(p.is_zero());
    }

    #[test]
    fn test_monomial() {
        let p = FieldPoly::monomial(fp7(3), 4);
        assert_eq!(p.degree(), Some(4));
        assert_eq!(p.coeff(0), fp7(0));
        assert_eq!(p.coeff(3), fp7(0));
        assert_eq!(p.coeff(4), fp7(3));
    }

    #[test]
    fn test_monomial_zero_coeff() {
        let p = FieldPoly::monomial(fp7(0), 5);
        assert!(p.is_zero());
    }

    #[test]
    fn test_monomial_degree_zero() {
        let p = FieldPoly::monomial(fp7(7), 0);
        // fp7(7) = fp7(0) because 7 mod 7 = 0; the monomial collapses.
        assert!(p.is_zero());

        let q = FieldPoly::monomial(fp7(2), 0);
        assert_eq!(q.degree(), Some(0));
        assert_eq!(q.coeff(0), fp7(2));
    }

    // -----------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------

    #[test]
    fn test_coeff_out_of_range_returns_zero() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2)]);
        assert_eq!(p.coeff(2), fp7(0));
        assert_eq!(p.coeff(100), fp7(0));
    }

    #[test]
    #[should_panic(expected = "cannot return zero_like on the zero")]
    fn test_coeff_on_zero_poly_panics() {
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        let _ = z.coeff(0);
    }

    #[test]
    fn test_leading_coeff() {
        let p = FieldPoly::new(vec![fp7(1), fp7(5)]);
        assert_eq!(p.leading_coeff(), Some(&fp7(5)));

        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert!(z.leading_coeff().is_none());
    }

    #[test]
    fn test_iter() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        let collected: Vec<FP7> = p.iter().cloned().collect();
        assert_eq!(collected, vec![fp7(1), fp7(2), fp7(3)]);

        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert_eq!(z.iter().count(), 0);
    }

    #[test]
    fn test_degree_and_len() {
        let p = FieldPoly::new(vec![fp7(4)]); // constant 4
        assert_eq!(p.degree(), Some(0));
        assert_eq!(p.len(), 1);

        let q = FieldPoly::new(vec![fp7(1), fp7(2)]); // linear
        assert_eq!(q.degree(), Some(1));
        assert_eq!(q.len(), 2);
    }

    // -----------------------------------------------------------------
    // Add / Sub / Neg
    // -----------------------------------------------------------------

    #[test]
    fn test_add_degree_after() {
        let a = FieldPoly::new(vec![fp7(1), fp7(2)]); // 2x + 1
        let b = FieldPoly::new(vec![fp7(3), fp7(4)]); // 4x + 3
        let c = &a + &b; // (2+4)x + (1+3) = 6x + 4
        assert_eq!(c.coeff(0), fp7(4));
        assert_eq!(c.coeff(1), fp7(6));
        assert_eq!(c.degree(), Some(1));
    }

    #[test]
    fn test_add_cancels_leading_term() {
        // (2x + 1) + (5x + 3) = 7x + 4 = 0·x + 4 = constant 4 in Fp<7>.
        let a = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let b = FieldPoly::new(vec![fp7(3), fp7(5)]);
        let c = a + b;
        assert_eq!(c.degree(), Some(0));
        assert_eq!(c.coeff(0), fp7(4));
    }

    #[test]
    fn test_add_all_ref_combinations() {
        let a = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let b = FieldPoly::new(vec![fp7(3), fp7(4)]);
        let expected = FieldPoly::new(vec![fp7(4), fp7(6)]);

        assert_eq!(a.clone() + b.clone(), expected);
        assert_eq!(&a + b.clone(), expected);
        assert_eq!(a.clone() + &b, expected);
        assert_eq!(&a + &b, expected);
    }

    #[test]
    fn test_sub_basic() {
        let a = FieldPoly::new(vec![fp7(5), fp7(6)]); // 6x + 5
        let b = FieldPoly::new(vec![fp7(1), fp7(2)]); // 2x + 1
        let c = a - b;
        assert_eq!(c.coeff(0), fp7(4));
        assert_eq!(c.coeff(1), fp7(4));
    }

    #[test]
    fn test_sub_cancels_leading_term() {
        let a = FieldPoly::new(vec![fp7(5), fp7(6)]);
        let c = a.clone() - a;
        assert!(c.is_zero());
    }

    #[test]
    fn test_sub_all_ref_combinations() {
        let a = FieldPoly::new(vec![fp7(5), fp7(6)]);
        let b = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let expected = FieldPoly::new(vec![fp7(4), fp7(4)]);

        assert_eq!(a.clone() - b.clone(), expected);
        assert_eq!(&a - b.clone(), expected);
        assert_eq!(a.clone() - &b, expected);
        assert_eq!(&a - &b, expected);
    }

    #[test]
    fn test_neg_basic() {
        let a = FieldPoly::new(vec![fp7(3), fp7(5)]);
        let n = -a.clone();
        assert_eq!(n.coeff(0), fp7(4)); // -3 mod 7 = 4
        assert_eq!(n.coeff(1), fp7(2)); // -5 mod 7 = 2
                                        // Double-negation is identity.
        assert_eq!(-n, a);
    }

    #[test]
    fn test_neg_of_zero_is_zero() {
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        let n = -z;
        assert!(n.is_zero());
    }

    // -----------------------------------------------------------------
    // AddAssign / SubAssign
    // -----------------------------------------------------------------

    #[test]
    fn test_add_assign_owned() {
        let mut a = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let b = FieldPoly::new(vec![fp7(3), fp7(4)]);
        a += b;
        assert_eq!(a, FieldPoly::new(vec![fp7(4), fp7(6)]));
    }

    #[test]
    fn test_add_assign_ref() {
        let mut a = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let b = FieldPoly::new(vec![fp7(3), fp7(4)]);
        a += &b;
        assert_eq!(a, FieldPoly::new(vec![fp7(4), fp7(6)]));
        // `b` still valid after reference-based add_assign.
        assert_eq!(b.coeff(1), fp7(4));
    }

    #[test]
    fn test_sub_assign_owned() {
        let mut a = FieldPoly::new(vec![fp7(5), fp7(6)]);
        let b = FieldPoly::new(vec![fp7(1), fp7(2)]);
        a -= b;
        assert_eq!(a, FieldPoly::new(vec![fp7(4), fp7(4)]));
    }

    #[test]
    fn test_sub_assign_ref() {
        let mut a = FieldPoly::new(vec![fp7(5), fp7(6)]);
        let b = FieldPoly::new(vec![fp7(1), fp7(2)]);
        a -= &b;
        assert_eq!(a, FieldPoly::new(vec![fp7(4), fp7(4)]));
        assert_eq!(b.coeff(0), fp7(1));
    }

    // -----------------------------------------------------------------
    // Scalar multiplication
    // -----------------------------------------------------------------

    #[test]
    fn test_mul_scalar_basic() {
        // (2x + 3) * 2 = 4x + 6
        let p = FieldPoly::new(vec![fp7(3), fp7(2)]);
        let q = p.mul_scalar(&fp7(2));
        assert_eq!(q.coeff(0), fp7(6));
        assert_eq!(q.coeff(1), fp7(4));
    }

    #[test]
    fn test_mul_scalar_zero_gives_zero_poly() {
        let p = FieldPoly::new(vec![fp7(3), fp7(2)]);
        let z = p.mul_scalar(&fp7(0));
        assert!(z.is_zero());
    }

    #[test]
    fn test_scale_equivalent_to_mul_scalar() {
        let p = FieldPoly::new(vec![fp7(3), fp7(2), fp7(1)]);
        let q = p.mul_scalar(&fp7(4));
        let mut r = p.clone();
        r.scale(&fp7(4));
        assert_eq!(q, r);
    }

    #[test]
    fn test_scale_by_zero_clears() {
        let mut p = FieldPoly::new(vec![fp7(3), fp7(2)]);
        p.scale(&fp7(0));
        assert!(p.is_zero());
    }

    // -----------------------------------------------------------------
    // Schoolbook multiplication
    // -----------------------------------------------------------------

    #[test]
    fn test_mul_degree_additive() {
        // (2x + 1) * (3x + 4) = 6x^2 + 11x + 4
        //                     = 6x^2 + 4x + 4 in Fp<7>
        let a = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let b = FieldPoly::new(vec![fp7(4), fp7(3)]);
        let c = &a * &b;
        assert_eq!(c.degree(), Some(2));
        assert_eq!(c.coeff(0), fp7(4));
        assert_eq!(c.coeff(1), fp7(4)); // 2*4 + 1*3 = 11 mod 7 = 4
        assert_eq!(c.coeff(2), fp7(6));
    }

    #[test]
    fn test_mul_by_zero_is_zero() {
        let a = FieldPoly::new(vec![fp7(3), fp7(2)]);
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert!((a.clone() * z.clone()).is_zero());
        assert!((z * a).is_zero());
    }

    #[test]
    fn test_mul_by_one_is_identity() {
        let a = FieldPoly::new(vec![fp7(3), fp7(2), fp7(1)]);
        let one: FieldPoly<FP7> = FieldPoly::one_like(&fp7(0));
        assert_eq!(&a * &one, a);
        assert_eq!(&one * &a, a);
    }

    #[test]
    fn test_mul_all_ref_combinations() {
        let a = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let b = FieldPoly::new(vec![fp7(4), fp7(3)]);
        let expected = a.clone() * b.clone();

        assert_eq!(&a * b.clone(), expected);
        assert_eq!(a.clone() * &b, expected);
        assert_eq!(&a * &b, expected);
    }

    // -----------------------------------------------------------------
    // Debug impl
    // -----------------------------------------------------------------

    #[test]
    fn test_debug_zero() {
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert_eq!(format!("{z:?}"), "0");
    }

    #[test]
    fn test_debug_constant() {
        let p = FieldPoly::constant(fp7(3));
        // The Fp<7> Debug is "Fp<7>(3)". We only insist the output
        // contains no x term when the degree is zero.
        let s = format!("{p:?}");
        assert!(!s.contains('x'));
    }

    #[test]
    fn test_debug_descending_order() {
        // 3x^2 + 1: linear term should come before constant in the
        // Debug string.
        let p = FieldPoly::new(vec![fp7(1), fp7(0), fp7(3)]);
        let s = format!("{p:?}");
        // Find positions of "x^2" and the final "1" constant.
        let x2_pos = s.find("x^2").expect("x^2 term missing");
        // Locate the '+' separator — everything after must be the
        // constant term. The invariant: x^2 term appears first.
        let plus = s.find('+').unwrap_or(usize::MAX);
        assert!(x2_pos < plus, "expected x^2 before constant in {s}");
    }

    #[test]
    fn test_debug_skips_zero_terms() {
        // x^2 + x (no constant term): should contain no isolated "0".
        let p = FieldPoly::new(vec![fp7(0), fp7(1), fp7(1)]);
        let s = format!("{p:?}");
        // Shouldn't end with "+ 0".
        assert!(!s.ends_with("0"), "unexpected zero term in {s}");
    }

    // -----------------------------------------------------------------
    // Gf2mElement smoke tests: the generic type parameter really works
    // with runtime-configured field types.
    // -----------------------------------------------------------------

    #[test]
    fn test_gf16_mul_schoolbook() {
        let field = Gf2mField::new(4, 0b10011);
        let a1 = field.element(5);
        let a2 = field.element(3);
        let b1 = field.element(2);
        let b2 = field.element(7);

        // (a1 + a2*x) * (b1 + b2*x) = (a1*b1) + (a1*b2 + a2*b1)*x + (a2*b2)*x^2
        let poly_a = FieldPoly::new(vec![a1.clone(), a2.clone()]);
        let poly_b = FieldPoly::new(vec![b1.clone(), b2.clone()]);
        let product = &poly_a * &poly_b;

        assert_eq!(product.degree(), Some(2));
        assert_eq!(product.coeff(0), a1.clone() * b1.clone());
        assert_eq!(product.coeff(1), a1 * b2.clone() + a2.clone() * b1);
        assert_eq!(product.coeff(2), a2 * b2);
    }

    #[test]
    fn test_gf16_add_sub_cycle() {
        let field = Gf2mField::new(4, 0b10011);
        let p: FieldPoly<Gf2mElement> =
            FieldPoly::new(vec![field.element(5), field.element(3), field.element(7)]);
        let q: FieldPoly<Gf2mElement> = FieldPoly::new(vec![field.element(1), field.element(6)]);

        let sum = &p + &q;
        let back = sum - q;
        assert_eq!(back, p);
    }

    #[test]
    fn test_gf16_scale_and_normalise() {
        let field = Gf2mField::new(4, 0b10011);
        let p = FieldPoly::new(vec![field.element(5), field.element(3), field.element(7)]);
        let mut q = p.clone();
        q.scale(&field.element(1)); // multiply by 1 — identity
        assert_eq!(q, p);

        let mut r = p;
        r.scale(&field.zero()); // multiply by 0 — collapses
        assert!(r.is_zero());
    }

    // -----------------------------------------------------------------
    // Proptests (tight budgets per CLAUDE.md 60s rule)
    // -----------------------------------------------------------------

    /// Strategy: generate a random `FieldPoly<Fp<7>>` with up to 5
    /// coefficients. Trailing zeros are allowed — they will be trimmed
    /// by `FieldPoly::new` and the resulting poly satisfies the
    /// invariant.
    fn any_fp7_poly() -> impl Strategy<Value = FieldPoly<FP7>> {
        prop::collection::vec(0u64..7, 0..5)
            .prop_map(|xs| FieldPoly::new(xs.into_iter().map(fp7).collect::<Vec<_>>()))
    }

    /// Strategy: *non-zero* polynomial over `Fp<7>`. Retries until the
    /// leading coefficient is non-zero to guarantee a real degree.
    fn any_nonzero_fp7_poly() -> impl Strategy<Value = FieldPoly<FP7>> {
        (1usize..=5, 1u64..7).prop_flat_map(|(n, last)| {
            (
                prop::collection::vec(0u64..7, n.saturating_sub(1)),
                Just(last),
            )
                .prop_map(move |(mut mid, last)| {
                    mid.push(last);
                    FieldPoly::new(mid.into_iter().map(fp7).collect::<Vec<_>>())
                })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_add_associative(
            a in any_fp7_poly(),
            b in any_fp7_poly(),
            c in any_fp7_poly(),
        ) {
            let lhs = (&a + &b) + &c;
            let rhs = &a + (&b + &c);
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn prop_mul_distributes_over_add(
            a in any_fp7_poly(),
            b in any_fp7_poly(),
            c in any_fp7_poly(),
        ) {
            let lhs = &a * &(&b + &c);
            let rhs = &(&a * &b) + &(&a * &c);
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn prop_degree_is_additive_on_mul(
            a in any_nonzero_fp7_poly(),
            b in any_nonzero_fp7_poly(),
        ) {
            let prod = &a * &b;
            let da = a.degree().unwrap();
            let db = b.degree().unwrap();
            // Fp<7> is a field (no zero divisors outside the zero
            // element), and a and b are non-zero by construction, so
            // the product is non-zero and its degree is exactly da+db.
            prop_assert_eq!(prod.degree(), Some(da + db));
        }
    }
}
