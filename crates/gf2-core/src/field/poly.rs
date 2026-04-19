//! Generic univariate polynomials over any [`FiniteField`].
//!
//! `FieldPoly<F>` is the single-source-of-truth polynomial type in
//! `gf2-core`. It stores coefficients in **ascending-degree** order
//! (`coeffs[i]` is the coefficient of `x^i`) and supports any field type
//! that implements the [`FiniteField`] trait — binary extension fields
//! ([`Gf2mElement`](crate::gf2m::Gf2mElement)), prime fields
//! ([`Fp<P>`](crate::gfp::Fp)), and tower extensions all compose
//! uniformly.
//!
//! The legacy binary-field alias
//! [`Gf2mPoly_<V>`](crate::gf2m::Gf2mPoly_) /
//! [`Gf2mPoly`](crate::gf2m::Gf2mPoly) is now a thin `pub type` alias to
//! `FieldPoly<Gf2mElement_<V>>`, preserved only so existing BCH / DVB-T2
//! call-sites continue to compile without churn. All algorithmic code
//! lives here.
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
//! `scale`, `div_rem`, `gcd`, …) is written so that it calls
//! [`FieldPoly::normalise`] before returning. Equality is *structural* —
//! two `FieldPoly`s are equal iff their normalised `coeffs` slices
//! compare equal element-wise.
//!
//! # Scope
//!
//! This file provides the core type and **all** algorithmic operations
//! that are generic over `F: FiniteField`:
//!
//! - Construction: [`FieldPoly::new`], [`FieldPoly::zero_like`],
//!   [`FieldPoly::one_like`], [`FieldPoly::constant`],
//!   [`FieldPoly::monomial`], [`FieldPoly::from_coeffs_trimmed`],
//!   [`FieldPoly::from_roots`], [`FieldPoly::product`].
//! - Queries: [`FieldPoly::degree`], [`FieldPoly::is_zero`],
//!   [`FieldPoly::coeff`], [`FieldPoly::leading_coeff`],
//!   [`FieldPoly::len`], [`FieldPoly::iter`].
//! - Operator overloads: `Add`, `Sub`, `Neg`, `AddAssign`, `SubAssign`
//!   in both owned and borrowed RHS forms.
//! - Scalar multiplication: [`FieldPoly::mul_scalar`],
//!   [`FieldPoly::scale`].
//! - Multiplication through the `Mul` operator: dispatches to
//!   schoolbook below [`KARATSUBA_THRESHOLD`] and to Karatsuba above.
//! - Euclidean division [`FieldPoly::div_rem`] and GCD
//!   [`FieldPoly::gcd`].
//! - Evaluation: [`FieldPoly::eval`] (Horner),
//!   [`FieldPoly::eval_batch`] (naive per-point loop), and
//!   [`FieldPoly::batch_evaluate`] (subproduct-tree,
//!   `O(M(n) log k + k log² k)`).
//!
//! Further algorithmic upgrades (Lagrange interpolation, balanced
//! product + batch GCD, NTT multiplication) land in sibling tasks on top
//! of this surface.
//!
//! # `batch_evaluate` benchmark results
//!
//! Measured on `Fp<65537>` with
//! `cargo bench -p gf2-core --bench field_poly -- --quick` on the repo's
//! reference Zen 3 host. Each cell is the median total wall-clock time
//! for one invocation on a polynomial of length `n` evaluated at `k`
//! points. The *subproduct-tree* column reports the time for the raw
//! subproduct path (bypassing the [`SUBPRODUCT_THRESHOLD`] gate);
//! `speedup = naive / subproduct`, so values **< 1** mean the naive
//! per-point Horner baseline wins.
//!
//! | `n`  | `k`  | naive Horner | subproduct tree | naive/subproduct |
//! |-----:|-----:|-------------:|----------------:|-----------------:|
//! |   16 |   16 |      0.88 µs |         6.64 µs |            0.13× |
//! |   16 |   64 |       3.5 µs |          35 µs  |            0.10× |
//! |   16 |  256 |        14 µs |         215 µs  |            0.06× |
//! |   16 | 1024 |        56 µs |         1.50 ms |            0.04× |
//! |   64 |   16 |       3.7 µs |          12 µs  |            0.30× |
//! |   64 |   64 |        15 µs |          55 µs  |            0.27× |
//! |   64 |  256 |        59 µs |         293 µs  |            0.20× |
//! |   64 | 1024 |       237 µs |         1.82 ms |            0.13× |
//! |  256 |   16 |        15 µs |          35 µs  |            0.41× |
//! |  256 |   64 |        59 µs |         115 µs  |            0.51× |
//! |  256 |  256 |       234 µs |         515 µs  |            0.45× |
//! |  256 | 1024 |       939 µs |         2.73 ms |            0.34× |
//! | 1024 |   16 |        59 µs |         127 µs  |            0.46× |
//! | 1024 |   64 |       234 µs |         237 µs  |            0.99× |
//! | 1024 |  256 |       940 µs |         1.34 ms |            0.70× |
//! | 1024 | 1024 |       3.75 ms|         6.00 ms |            0.63× |
//!
//! **The naive path wins on every benchmarked cell** on `Fp<65537>`.
//! This is expected given the scalar-field cost profile: a single
//! `Fp<65537>` multiplication is ≈ 3.6 ns (inlined Barrett-style
//! reduction over `u64`), while the subproduct tree incurs a fixed
//! constant factor of additional `Vec<F>` allocations from every
//! intermediate [`FieldPoly::mul`] and [`FieldPoly::div_rem`]. The
//! theoretical `O(M(n) log k + k log² k)` asymptotic win assumes a
//! fast-polynomial-division primitive built on FFT / Newton iteration;
//! with schoolbook [`FieldPoly::div_rem`] the reduction phase remains
//! `O(n · k)` — the same as naive — so only the allocation overhead
//! shows up on the clock.
//!
//! The subproduct path is therefore guarded by
//! [`SUBPRODUCT_THRESHOLD`] and is only expected to pay off on fields
//! with significantly more expensive scalar arithmetic than `Fp<65537>`
//! (large-prime Montgomery, tower extensions, …) or once a fast
//! polynomial-division kernel lands alongside this API. The
//! implementation is retained unchanged so that a future
//! FFT-multiplication drop-in (Task 6 of the `bdf95060` story) lights up
//! the expected speedup without further API churn. Regenerate this
//! table with `cargo bench -p gf2-core --bench field_poly`.

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
/// Over a compile-time prime field `Fp<7>`:
///
/// ```
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// // 2x + 3 over Fp<7>
/// let p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]);
/// assert_eq!(p.degree(), Some(1));
/// assert_eq!(p.try_coeff(0), Some(&Fp::<7>::new(3)));
/// assert_eq!(p.try_coeff(1), Some(&Fp::<7>::new(2)));
/// ```
///
/// Over a runtime-configured binary extension field `Gf2mElement`
/// (e.g. GF(2^4) with reduction polynomial `x^4 + x + 1`):
///
/// ```
/// use gf2_core::field::{FieldPoly, FiniteField};
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let p = FieldPoly::new(vec![field.element(5), field.element(3)]);
/// assert_eq!(p.degree(), Some(1));
/// // Polynomial arithmetic composes with the runtime field:
/// let q = FieldPoly::new(vec![field.element(2), field.element(1)]);
/// let sum = &p + &q;
/// assert_eq!(sum.degree(), Some(1));
/// assert_eq!(sum.try_coeff(0), Some(&(field.element(5) + field.element(2))));
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
    /// assert_eq!(p.try_coeff(0), Some(&Fp::<7>::new(4)));
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
    ///   type parameter `F`. The sample is **not** stored anywhere on
    ///   the returned polynomial: the zero polynomial has an empty
    ///   `coeffs` vector. Callers that need a zero element derived from
    ///   a specific field context on an empty polynomial should use
    ///   [`FieldPoly::coeff_or_zero`] and pass the sample at that call
    ///   site.
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
    /// assert_eq!(p.try_coeff(0), Some(&Fp::<7>::new(1)));
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
    /// assert_eq!(p.try_coeff(0), Some(&Fp::<7>::new(5)));
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
    /// assert_eq!(p.try_coeff(0), Some(&Fp::<7>::new(0)));
    /// assert_eq!(p.try_coeff(4), Some(&Fp::<7>::new(3)));
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

    /// Returns a reference to the coefficient of `x^i`, or `None` if
    /// `i` is out of range (including the whole zero-polynomial case).
    ///
    /// This method is **total** over `usize`: it never panics. Callers
    /// that always want a field element in hand (treating "past the
    /// degree" as a genuine zero) should use
    /// [`FieldPoly::coeff_or_zero`] with a sample field element.
    ///
    /// # Arguments
    ///
    /// * `i` — exponent of the requested coefficient.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // 2x + 3
    /// let p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]);
    /// assert_eq!(p.try_coeff(0), Some(&Fp::<7>::new(3)));
    /// assert_eq!(p.try_coeff(1), Some(&Fp::<7>::new(2)));
    /// // Out-of-range: returns None.
    /// assert_eq!(p.try_coeff(10), None);
    ///
    /// // The zero polynomial returns None for every index.
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert_eq!(z.try_coeff(0), None);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn try_coeff(&self, i: usize) -> Option<&F> {
        self.coeffs.get(i)
    }

    /// Returns the `i`-th coefficient by value.
    ///
    /// Behaviour (matching `dev/plans/bdf95060_breakdown.md` Task 1):
    /// * `0 <= i < self.len()`: returns `self.coeffs[i].clone()`.
    /// * `i >= self.len()` on a non-zero polynomial: returns a zero
    ///   built from an existing coefficient via
    ///   [`FiniteField::zero_like`].
    /// * `self.is_zero()` (empty storage): panics, because no sample
    ///   is available to derive a zero from. Use [`FieldPoly::try_coeff`]
    ///   or [`FieldPoly::coeff_or_zero`] on polynomials that may be zero.
    ///
    /// # Arguments
    ///
    /// * `i` — exponent of the requested coefficient.
    ///
    /// # Panics
    ///
    /// Panics if called on the zero polynomial (no field context
    /// available to derive a zero element). Callers with a field
    /// sample should use [`FieldPoly::coeff_or_zero`]; callers that
    /// want a clean `Option` should use [`FieldPoly::try_coeff`].
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
    /// // Out-of-range on a non-zero polynomial: the zero element.
    /// assert_eq!(p.coeff(10), Fp::<7>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn coeff(&self, i: usize) -> F {
        if let Some(c) = self.coeffs.get(i) {
            c.clone()
        } else {
            // Safe: we just checked self.coeffs.get(i) was None; if
            // the slice is non-empty, index 0 is valid.
            assert!(
                !self.coeffs.is_empty(),
                "FieldPoly::coeff called on the zero polynomial (no field sample available); \
                 use try_coeff or coeff_or_zero instead"
            );
            self.coeffs[0].zero_like()
        }
    }

    /// Returns the `i`-th coefficient, or a zero element built from
    /// `sample` when `i` is out of range (including the zero
    /// polynomial).
    ///
    /// This is the total variant of [`FieldPoly::coeff`] for callers
    /// that already have a field-element sample in hand.
    /// [`FieldPoly::coeff`] is preferable when the caller can act on
    /// `Option`.
    ///
    /// # Arguments
    ///
    /// * `i` — exponent of the requested coefficient.
    /// * `sample` — any field element; used only when the request is
    ///   out of range, to build a zero in the correct field via
    ///   [`FiniteField::zero_like`]. For field types that carry a
    ///   runtime field handle (e.g.
    ///   [`Gf2mElement`](crate::gf2m::Gf2mElement)) this ensures the
    ///   returned zero is in the caller's intended field.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]);
    /// assert_eq!(p.coeff_or_zero(0, &Fp::<7>::new(0)), Fp::<7>::new(3));
    /// assert_eq!(p.coeff_or_zero(10, &Fp::<7>::new(0)), Fp::<7>::new(0));
    ///
    /// // Works on the zero polynomial too.
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert_eq!(z.coeff_or_zero(0, &Fp::<7>::new(0)), Fp::<7>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn coeff_or_zero(&self, i: usize, sample: &F) -> F {
        self.try_coeff(i)
            .cloned()
            .unwrap_or_else(|| sample.zero_like())
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
    // Inherent multiplication (schoolbook / Karatsuba dispatch)
    // -----------------------------------------------------------------

    /// Polynomial multiplication with schoolbook/Karatsuba dispatch.
    ///
    /// Below [`KARATSUBA_THRESHOLD`] coefficients this routes through the
    /// schoolbook kernel; above it, recursive Karatsuba. Callers observe
    /// a single, normalised `FieldPoly<F>` and do not need to choose
    /// between the two.
    ///
    /// Equivalent to the [`core::ops::Mul`] trait impls
    /// (`&FieldPoly * &FieldPoly`) that delegate to this inherent
    /// method. The zero polynomial on either side produces the zero
    /// polynomial.
    ///
    /// # Arguments
    ///
    /// * `other` — polynomial to multiply `self` by.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // (x + 2)(x + 3) = x^2 + 5x + 6 (mod 7)
    /// let a = FieldPoly::new(vec![Fp::<7>::new(2), Fp::<7>::new(1)]);
    /// let b = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(1)]);
    /// let c = a.mul(&b);
    /// assert_eq!(c.degree(), Some(2));
    /// assert_eq!(c.coeff(0), Fp::<7>::new(6));
    /// assert_eq!(c.coeff(1), Fp::<7>::new(5));
    /// assert_eq!(c.coeff(2), Fp::<7>::new(1));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n·m)` base-field multiplications in the schoolbook regime and
    /// `O(n^{log₂ 3})` in the Karatsuba regime, where `n = self.len()`
    /// and `m = other.len()`.
    pub fn mul(&self, other: &Self) -> Self {
        mul_impl(&self.coeffs, &other.coeffs)
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
    /// assert_eq!(q.try_coeff(0), Some(&Fp::<7>::new(6)));
    /// assert_eq!(q.try_coeff(1), Some(&Fp::<7>::new(4)));
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
    /// assert_eq!(p.try_coeff(0), Some(&Fp::<7>::new(6)));
    /// assert_eq!(p.try_coeff(1), Some(&Fp::<7>::new(4)));
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
    // Evaluation
    // -----------------------------------------------------------------

    /// Evaluates the polynomial at a point using Horner's method.
    ///
    /// Computes `self(x)` as
    /// `((…((a_n · x) + a_{n-1}) · x + …) · x + a_0)`.
    ///
    /// On the zero polynomial this returns `x.zero_like()` — the
    /// additive identity in the same field as `x` — matching the
    /// "empty polynomial = zero" convention documented in the
    /// `bdf95060` breakdown (Task 2 of the epic) so callers never need
    /// to special-case the zero polynomial around `eval`.
    ///
    /// # Arguments
    ///
    /// * `x` — the evaluation point.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // p(x) = 3x² + 2x + 1 over Fp<7>
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(3)]);
    /// // p(2) = 12 + 4 + 1 = 17 ≡ 3 (mod 7)
    /// assert_eq!(p.eval(&Fp::<7>::new(2)), Fp::<7>::new(3));
    ///
    /// // The zero polynomial evaluates to zero in the field of `x`.
    /// use gf2_core::field::FiniteField;
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert_eq!(z.eval(&Fp::<7>::new(5)), Fp::<7>::new(5).zero_like());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` field multiplications and `O(n)` field additions, where
    /// `n = self.len()`. `O(1)` on the zero polynomial.
    pub fn eval(&self, x: &F) -> F {
        // Empty polynomial ≡ 0. Use `x` as the field-context sample so
        // runtime-configured fields (e.g. `Gf2mElement`) produce a zero
        // in the caller's intended field.
        if self.coeffs.is_empty() {
            return x.zero_like();
        }

        // Horner: start from the leading coefficient and fold down.
        let mut result = self.coeffs.last().unwrap().clone();
        for i in (0..self.coeffs.len() - 1).rev() {
            result = result * x.clone() + self.coeffs[i].clone();
        }
        result
    }

    /// Evaluates the polynomial at every point in `points`, returning
    /// the values in the same order.
    ///
    /// This is a naive per-point loop. A subproduct-tree algorithm with
    /// better asymptotics is provided in a follow-up task.
    ///
    /// On the zero polynomial every result is `x.zero_like()` for the
    /// corresponding point, matching the total
    /// [`FieldPoly::eval`](Self::eval) contract.
    ///
    /// # Arguments
    ///
    /// * `points` — slice of evaluation points.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(2)]);
    /// let ys = p.eval_batch(&[Fp::<7>::new(0), Fp::<7>::new(1), Fp::<7>::new(3)]);
    /// assert_eq!(ys, vec![Fp::<7>::new(1), Fp::<7>::new(3), Fp::<7>::new(0)]);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n · k)` field operations for `n = self.len()` and
    /// `k = points.len()`.
    pub fn eval_batch(&self, points: &[F]) -> Vec<F> {
        points.iter().map(|x| self.eval(x)).collect()
    }

    /// Evaluates the polynomial at every point in `points` using a
    /// subproduct-tree algorithm, or the naive per-point Horner fallback
    /// when the inputs are below the [`SUBPRODUCT_THRESHOLD`] crossover.
    ///
    /// With schoolbook polynomial arithmetic this routine costs
    /// `O(n · k + k² log k)` field operations for
    /// `n = self.degree() + 1` and `k = points.len()`. The asymptotic
    /// target is `O(M(n) log k + k log² k)` — achievable with an
    /// FFT-backed polynomial multiplication (`M(n) = O(n log n)`) and a
    /// fast polynomial division built on Newton's iteration for the
    /// reciprocal; both are sibling tasks (see the `bdf95060` story).
    /// Until those land, this implementation is slower than the naive
    /// Horner baseline on fields with cheap scalar arithmetic such as
    /// `Fp<65537>` (see the benchmark table in the module docstring),
    /// which is why [`SUBPRODUCT_THRESHOLD`] is set conservatively.
    ///
    /// # Algorithm
    ///
    /// 1. **Leaves**: build `M_i = x - points[i]` for every point.
    /// 2. **Subproduct tree** (bottom-up): pair-merge siblings through
    ///    [`FieldPoly::Mul`] (which in turn dispatches schoolbook /
    ///    Karatsuba above [`KARATSUBA_THRESHOLD`]), recording every
    ///    internal node.
    /// 3. **Reduction** (top-down): starting from `self mod root`,
    ///    reduce modulo each internal node via [`FieldPoly::div_rem`]
    ///    and descend to the leaves. Each leaf-modulus remainder is a
    ///    constant whose value is the Horner evaluation `self(point_i)`.
    ///
    /// The agreement with per-point Horner is exhaustive (see the
    /// proptests in this module) — this function returns the **same**
    /// `Vec<F>` as `points.iter().map(|p| self.eval(p)).collect()`.
    ///
    /// # Arguments
    ///
    /// * `points` — slice of evaluation points. May be empty, contain
    ///   zeros, or contain duplicates.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // p(x) = 3x² + 2x + 1 over Fp<7>
    /// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(3)]);
    /// let xs = vec![Fp::<7>::new(0), Fp::<7>::new(1), Fp::<7>::new(4)];
    /// let ys = p.batch_evaluate(&xs);
    /// assert_eq!(ys, vec![Fp::<7>::new(1), Fp::<7>::new(6), Fp::<7>::new(1)]);
    /// // Agrees with per-point Horner.
    /// assert_eq!(ys, xs.iter().map(|x| p.eval(x)).collect::<Vec<_>>());
    /// ```
    ///
    /// On the zero polynomial every result is `x.zero_like()` for the
    /// corresponding point, matching the total [`FieldPoly::eval`]
    /// contract:
    ///
    /// ```
    /// use gf2_core::field::{FieldPoly, FiniteField};
    /// use gf2_core::gfp::Fp;
    ///
    /// let z: FieldPoly<Fp<7>> = FieldPoly::zero_like(&Fp::<7>::new(0));
    /// assert_eq!(
    ///     z.batch_evaluate(&[Fp::<7>::new(1), Fp::<7>::new(2)]),
    ///     vec![Fp::<7>::new(0), Fp::<7>::new(0)],
    /// );
    /// ```
    ///
    /// # Complexity
    ///
    /// With the current schoolbook-backed [`FieldPoly::mul`] and
    /// [`FieldPoly::div_rem`] primitives the subproduct path runs in
    /// `O(n · k + k² log k)` field operations for `n = self.degree() + 1`
    /// and `k = points.len()`. Below [`SUBPRODUCT_THRESHOLD`] in either
    /// dimension it falls back to the `O(n · k)` per-point Horner loop,
    /// which wins on `Fp<65537>` on every benchmarked cell (see the
    /// module docstring for the measured table).
    ///
    /// # Panics
    ///
    /// Does not panic on valid inputs: `points.is_empty()` returns an
    /// empty `Vec`, duplicate or zero points are accepted, and the zero
    /// polynomial evaluates to `x.zero_like()` at every point.
    pub fn batch_evaluate(&self, points: &[F]) -> Vec<F> {
        // Small-input fallback: below the crossover, the overhead of
        // building the subproduct tree (O(k) polynomial multiplications
        // plus O(k) Euclidean divisions) exceeds the savings compared to
        // k Horner folds of length n. See the benchmark table in the
        // module docstring for the measured crossover on Fp<65537>; both
        // gates are relaxed so that a zero-polynomial / single-coefficient
        // input short-circuits immediately.
        let n = self.len();
        if points.len() < SUBPRODUCT_THRESHOLD || n < SUBPRODUCT_THRESHOLD {
            return self.eval_batch(points);
        }

        batch_evaluate_subproduct(self, points)
    }

    // -----------------------------------------------------------------
    // Construction from roots and products
    // -----------------------------------------------------------------

    /// Builds the monic polynomial whose roots are exactly `roots`:
    /// `(x - r_0)(x - r_1) · … · (x - r_{n-1})`.
    ///
    /// # Arguments
    ///
    /// * `roots` — field elements to use as roots.
    ///
    /// # Panics
    ///
    /// Panics if `roots` is empty (no field sample available).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // (x - 1)(x - 2) = x² - 3x + 2 over Fp<7>
    /// let p = FieldPoly::from_roots(&[Fp::<7>::new(1), Fp::<7>::new(2)]);
    /// assert_eq!(p.eval(&Fp::<7>::new(1)), Fp::<7>::new(0));
    /// assert_eq!(p.eval(&Fp::<7>::new(2)), Fp::<7>::new(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n²)` field multiplications via sequential folding. A balanced
    /// product tree (Task 5 of the `bdf95060` story) reduces this to
    /// `O(M(n) log n)`.
    pub fn from_roots(roots: &[F]) -> Self {
        assert!(
            !roots.is_empty(),
            "FieldPoly::from_roots: roots cannot be empty"
        );

        let one = roots[0].one_like();
        let mut result = FieldPoly::new(vec![-roots[0].clone(), one.clone()]);
        for root in &roots[1..] {
            let factor = FieldPoly::new(vec![-root.clone(), one.clone()]);
            result = &result * &factor;
        }
        result
    }

    /// Computes the product of a slice of polynomials using a naive
    /// linear fold.
    ///
    /// # Arguments
    ///
    /// * `polys` — slice of polynomials to multiply.
    ///
    /// # Panics
    ///
    /// Panics if `polys` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p1 = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(1)]); // x + 1
    /// let p2 = FieldPoly::new(vec![Fp::<7>::new(2), Fp::<7>::new(1)]); // x + 2
    /// let p3 = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(1)]); // x + 3
    /// let prod = FieldPoly::product(&[p1.clone(), p2.clone(), p3.clone()]);
    /// // (1+1)(1+2)(1+3) = 2*3*4 = 24 = 3 mod 7
    /// assert_eq!(prod.eval(&Fp::<7>::new(1)), Fp::<7>::new(3));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n · d²)` where `n = polys.len()` and `d` is the average
    /// degree. A balanced product tree (Task 5) reduces the polynomial
    /// multiplication cost to `O(M(nd) log n)`.
    pub fn product(polys: &[FieldPoly<F>]) -> Self {
        assert!(
            !polys.is_empty(),
            "FieldPoly::product: polys cannot be empty"
        );

        if polys.len() == 1 {
            return polys[0].clone();
        }
        polys
            .iter()
            .skip(1)
            .fold(polys[0].clone(), |acc, p| &acc * p)
    }

    // -----------------------------------------------------------------
    // Euclidean division and GCD
    // -----------------------------------------------------------------

    /// Divides `self` by `divisor`, returning the quotient and
    /// remainder.
    ///
    /// The result satisfies `self = quotient · divisor + remainder`
    /// with `deg(remainder) < deg(divisor)` (or remainder is the zero
    /// polynomial).
    ///
    /// # Arguments
    ///
    /// * `divisor` — the polynomial to divide by.
    ///
    /// # Panics
    ///
    /// Panics if `divisor` is the zero polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // (x² + x + 1) / (x + 1) over Fp<7>
    /// let dividend = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(1), Fp::<7>::new(1)]);
    /// let divisor  = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(1)]);
    /// let (q, r) = dividend.div_rem(&divisor);
    /// // Verify: q·divisor + r = dividend.
    /// assert_eq!(&(&q * &divisor) + &r, dividend);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O((n - m) · m)` field operations, where `n = self.len()` and
    /// `m = divisor.len()`.
    pub fn div_rem(&self, divisor: &FieldPoly<F>) -> (FieldPoly<F>, FieldPoly<F>) {
        assert!(
            !divisor.is_zero(),
            "FieldPoly::div_rem: division by zero polynomial"
        );

        // If self is zero, both quotient and remainder are zero.
        let Some(dividend_deg) = self.degree() else {
            return (
                FieldPoly { coeffs: Vec::new() },
                FieldPoly { coeffs: Vec::new() },
            );
        };
        let divisor_deg = divisor.degree().unwrap();

        if dividend_deg < divisor_deg {
            return (FieldPoly { coeffs: Vec::new() }, self.clone());
        }

        let zero = self.coeffs[0].zero_like();
        let mut remainder_coeffs = self.coeffs.clone();
        let mut quotient_coeffs = vec![zero.clone(); dividend_deg - divisor_deg + 1];

        let divisor_lead = divisor.coeffs.last().unwrap().clone();
        // Current degree of the working remainder, tracked without
        // re-scanning the vector.
        let mut rem_len = remainder_coeffs.len();

        while rem_len > 0 && rem_len > divisor_deg {
            let rem_deg = rem_len - 1;
            let rem_lead = remainder_coeffs[rem_deg].clone();
            if rem_lead.is_zero() {
                // Skip spurious leading zero and shrink the window.
                rem_len -= 1;
                continue;
            }
            let q_coeff = rem_lead / divisor_lead.clone();
            let q_deg = rem_deg - divisor_deg;

            quotient_coeffs[q_deg] = q_coeff.clone();

            // remainder -= q_coeff · x^q_deg · divisor
            for i in 0..divisor.coeffs.len() {
                let sub_term = q_coeff.clone() * divisor.coeffs[i].clone();
                let slot = i + q_deg;
                let cur = remainder_coeffs[slot].clone();
                remainder_coeffs[slot] = cur - sub_term;
            }
            // Shrink remainder window past the (now-zero) leading term.
            rem_len = rem_deg;
            while rem_len > 0 && remainder_coeffs[rem_len - 1].is_zero() {
                rem_len -= 1;
            }
        }

        remainder_coeffs.truncate(rem_len);
        let quotient = FieldPoly::new(quotient_coeffs);
        let remainder = FieldPoly::new(remainder_coeffs);
        (quotient, remainder)
    }

    /// Returns the monic greatest common divisor of `a` and `b`, using
    /// the Euclidean algorithm.
    ///
    /// The result is always monic (leading coefficient is `1`) unless
    /// both inputs are zero, in which case the zero polynomial is
    /// returned.
    ///
    /// # Arguments
    ///
    /// * `a` — first polynomial.
    /// * `b` — second polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // Shared factor (x - 1): p1 = (x - 1)(x - 2), p2 = (x - 1)(x - 3).
    /// let xm1 = FieldPoly::new(vec![-Fp::<7>::new(1), Fp::<7>::new(1)]);
    /// let xm2 = FieldPoly::new(vec![-Fp::<7>::new(2), Fp::<7>::new(1)]);
    /// let xm3 = FieldPoly::new(vec![-Fp::<7>::new(3), Fp::<7>::new(1)]);
    /// let p1 = &xm1 * &xm2;
    /// let p2 = &xm1 * &xm3;
    /// let g = FieldPoly::gcd(&p1, &p2);
    /// assert_eq!(g, xm1);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n²)` field operations in the worst case, where `n` is the
    /// maximum degree.
    pub fn gcd(a: &FieldPoly<F>, b: &FieldPoly<F>) -> FieldPoly<F> {
        let mut r0 = a.clone();
        let mut r1 = b.clone();

        while !r1.is_zero() {
            let (_, remainder) = r0.div_rem(&r1);
            r0 = r1;
            r1 = remainder;
        }

        // Make the result monic (leading coefficient = 1).
        if let Some(lead) = r0.coeffs.last() {
            if !lead.is_one() {
                if let Some(inv) = lead.inv() {
                    let monic: Vec<F> = r0.coeffs.iter().map(|c| c.clone() * inv.clone()).collect();
                    return FieldPoly::new(monic);
                }
            }
        }
        r0
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
// Multiplication — schoolbook / Karatsuba dispatch
// ---------------------------------------------------------------------

/// Crossover threshold between schoolbook and Karatsuba multiplication.
///
/// Operand degrees strictly less than [`KARATSUBA_THRESHOLD`] use the
/// schoolbook algorithm; above that threshold both operands recurse
/// through Karatsuba. A quick microbenchmark on `Gf2mElement<u64>` at
/// GF(2^14) places the crossover near 32; smaller prime fields benefit
/// from the same value.
pub const KARATSUBA_THRESHOLD: usize = 32;

/// Crossover threshold for [`FieldPoly::batch_evaluate`] between the
/// subproduct-tree algorithm and the naive per-point Horner fallback.
///
/// When either `points.len()` or `self.len()` (the polynomial length, i.e.
/// degree + 1) is strictly less than this value,
/// [`FieldPoly::batch_evaluate`] falls through to
/// [`FieldPoly::eval_batch`] (`O(n · k)` naive Horner).
///
/// The value is tuned from the benchmark harness in
/// `crates/gf2-core/benches/field_poly.rs`. On `Fp<65537>`, where a
/// single scalar multiplication is ≈ 3.6 ns, the overhead of building
/// the subproduct tree (and the `Vec<F>` allocations that accompany
/// every intermediate [`FieldPoly::mul`] / [`FieldPoly::div_rem`]) is
/// never amortised at the benchmarked sizes; the naive path wins on
/// every cell in the `n, k ∈ {16, 64, 256, 1024}` matrix. See the module
/// docstring for the measured table. A conservative default of `256`
/// keeps the fallback active for those cheap-arithmetic fields until a
/// fast polynomial-division primitive (FFT / Newton) lands alongside
/// this API — at which point the asymptotic win materialises and the
/// threshold can be lowered. Fields with substantially more expensive
/// scalar arithmetic (large-prime Montgomery, tower extensions) tip the
/// balance earlier; callers can already dispatch the subproduct path
/// manually by skipping `batch_evaluate` and reusing the existing
/// [`FieldPoly::mul`] + [`FieldPoly::div_rem`] primitives on a tree
/// they build themselves.
pub const SUBPRODUCT_THRESHOLD: usize = 256;

/// Raw subproduct-tree batch evaluation, exposed **only** for the
/// benchmark harness (see [`FieldPoly::batch_evaluate`] for the stable
/// entry point). Callers bypass the [`SUBPRODUCT_THRESHOLD`]
/// performance gate and pay the full tree cost unconditionally.
///
/// # Arguments
///
/// * `poly` — the polynomial to evaluate.
/// * `points` — slice of evaluation points. Must be non-empty
///   (`debug_assert!`ed); may contain zeros and duplicates. Empty
///   `points` slices must go through [`FieldPoly::batch_evaluate`],
///   which handles that case in the fallback path.
///
/// # Algorithm
///
/// 1. Build the leaves `M_i = x - points[i]`.
/// 2. Bottom-up, pair-merge siblings through [`FieldPoly::Mul`],
///    carrying an odd final node up without a partner. The tree is
///    retained in full as a flat `Vec<Vec<FieldPoly<F>>>`.
/// 3. Top-down, reduce `poly` modulo the root, then split each
///    remainder across its children via [`FieldPoly::div_rem`] until
///    every leaf holds the constant `poly(points[i])`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly::batch_evaluate_subproduct;
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// let p = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(2), Fp::<7>::new(3)]);
/// let xs = vec![Fp::<7>::new(0), Fp::<7>::new(1), Fp::<7>::new(4)];
/// let ys = batch_evaluate_subproduct(&p, &xs);
/// // Agrees with per-point Horner.
/// assert_eq!(ys, xs.iter().map(|x| p.eval(x)).collect::<Vec<_>>());
/// ```
///
/// # Panics
///
/// Panics (via `debug_assert`) in debug builds if `points` is empty.
/// Release builds are left to exhibit UB-free but arbitrary behaviour
/// — in practice the leaf construction needs `points[0]` to
/// materialise `one_like()`, so the function does panic with an
/// out-of-bounds index on an empty slice. Callers must route through
/// [`FieldPoly::batch_evaluate`] for the total contract.
///
/// # Complexity
///
/// `O(n · k + k² log k)` field operations for `n = poly.len()` and
/// `k = points.len()` when backed by schoolbook
/// [`FieldPoly::div_rem`]. The target asymptotic
/// `O(M(n) log k + k log² k)` requires an FFT / Newton-iteration fast
/// polynomial division primitive (Task 6 of the `bdf95060` story).
///
/// This function is exposed as `pub` strictly so the benchmark harness
/// (`benches/field_poly.rs`) can compare the raw subproduct cost
/// against the naive baseline without being masked by the threshold
/// dispatch in [`FieldPoly::batch_evaluate`]. External callers should
/// always go through the public entry point, which guards the
/// subproduct path with [`SUBPRODUCT_THRESHOLD`].
pub fn batch_evaluate_subproduct<F: FiniteField>(poly: &FieldPoly<F>, points: &[F]) -> Vec<F> {
    debug_assert!(!points.is_empty());

    let k = points.len();

    // Build the leaves: M_i = x - points[i], stored in
    // ascending-degree order `[-points[i], 1]`.
    let one = points[0].one_like();
    let leaves: Vec<FieldPoly<F>> = points
        .iter()
        .map(|p| FieldPoly::new(vec![-p.clone(), one.clone()]))
        .collect();

    // Bottom-up: pair-merge siblings. Odd tail at each level carries
    // up unchanged. `levels[0]` = leaves; `levels[last]` = root.
    let mut levels: Vec<Vec<FieldPoly<F>>> = vec![leaves];
    while levels.last().unwrap().len() > 1 {
        let cur = levels.last().unwrap();
        let mut next: Vec<FieldPoly<F>> = Vec::with_capacity(cur.len().div_ceil(2));
        let mut i = 0;
        while i + 1 < cur.len() {
            next.push(&cur[i] * &cur[i + 1]);
            i += 2;
        }
        if i < cur.len() {
            // Odd tail: carry the last node up without a partner. The
            // reduction phase below handles single-child descents.
            next.push(cur[i].clone());
        }
        levels.push(next);
    }

    // Top-down reduction.
    //
    // Invariant maintained while descending: for every node `j` at
    // level `h`, the polynomial `rems[h][j]` equals `poly mod levels[h][j]`
    // and has degree strictly less than `levels[h][j].degree()`. At the
    // leaves (level 0) each modulus is linear, so `rems[0][j]` is the
    // constant `poly(points[j])`.
    //
    // We only materialise a single "current level" of remainders at a
    // time to keep peak memory proportional to the widest tree level.
    let root_level = levels.len() - 1;
    debug_assert_eq!(levels[root_level].len(), 1);

    let (_, root_rem) = poly.div_rem(&levels[root_level][0]);
    let mut cur_rems: Vec<FieldPoly<F>> = vec![root_rem];

    for h in (0..root_level).rev() {
        let children = &levels[h];
        let parents = &levels[h + 1];
        debug_assert_eq!(cur_rems.len(), parents.len());

        let mut next_rems: Vec<FieldPoly<F>> = Vec::with_capacity(children.len());
        for (p_idx, rem) in cur_rems.iter().enumerate() {
            let left_idx = 2 * p_idx;
            let right_idx = left_idx + 1;

            if right_idx < children.len() {
                // Paired parent: split `rem` across both children.
                let (_, left_rem) = rem.div_rem(&children[left_idx]);
                let (_, right_rem) = rem.div_rem(&children[right_idx]);
                next_rems.push(left_rem);
                next_rems.push(right_rem);
            } else {
                // Odd carry-up: parent equals its lone child exactly, so
                // the remainder passes through unchanged.
                debug_assert_eq!(&children[left_idx], &parents[p_idx]);
                next_rems.push(rem.clone());
            }
        }
        cur_rems = next_rems;
    }

    debug_assert_eq!(cur_rems.len(), k);

    // Extract constant remainders. A remainder mod (x - point_i) is
    // either the zero polynomial (value zero) or a constant; the
    // normalised representation is `[]` for zero and `[c]` for c != 0.
    cur_rems
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            if r.is_zero() {
                points[i].zero_like()
            } else {
                debug_assert_eq!(r.len(), 1, "remainder mod linear must be constant");
                r.coeffs.into_iter().next().unwrap()
            }
        })
        .collect()
}

/// Core schoolbook polynomial multiplication, `O(n · m)` in field mults.
fn mul_schoolbook_impl<F: FiniteField>(lhs: &[F], rhs: &[F]) -> FieldPoly<F> {
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

/// Slice-level addition used by Karatsuba recombination.
fn slice_add<F: FiniteField>(a: &[F], b: &[F]) -> Vec<F> {
    let max_len = a.len().max(b.len());
    let mut out = Vec::with_capacity(max_len);
    for i in 0..max_len {
        match (a.get(i), b.get(i)) {
            (Some(x), Some(y)) => out.push(x.clone() + y.clone()),
            (Some(x), None) => out.push(x.clone()),
            (None, Some(y)) => out.push(y.clone()),
            (None, None) => unreachable!(),
        }
    }
    out
}

/// Karatsuba multiplication. `lhs` and `rhs` must be non-empty and
/// normalised; the caller (`mul_impl`) guarantees that. Returns raw
/// coefficients of length `lhs.len() + rhs.len() - 1` **without**
/// normalising — the top-level `FieldPoly::new` at the entry point does
/// the final trim.
fn mul_karatsuba_raw<F: FiniteField>(lhs: &[F], rhs: &[F]) -> Vec<F> {
    debug_assert!(!lhs.is_empty() && !rhs.is_empty());

    let deg_lhs = lhs.len() - 1;
    let deg_rhs = rhs.len() - 1;

    if deg_lhs < KARATSUBA_THRESHOLD || deg_rhs < KARATSUBA_THRESHOLD {
        let out = mul_schoolbook_impl(lhs, rhs);
        // Rehydrate to an unnormalised-length Vec for caller's combine
        // step: pad to (lhs.len() + rhs.len() - 1) with zeros.
        let out_len = lhs.len() + rhs.len() - 1;
        let zero = lhs[0].zero_like();
        let mut padded = out.coeffs;
        padded.resize(out_len, zero);
        return padded;
    }

    // Split point: midpoint of the larger operand.
    let m = (deg_lhs.max(deg_rhs) / 2) + 1;

    // p_lo, p_hi  (low and high halves of lhs about x^m)
    let (p_lo_slice, p_hi_slice) = if lhs.len() > m {
        (&lhs[..m], &lhs[m..])
    } else {
        (lhs, &[] as &[F])
    };
    // q_lo, q_hi
    let (q_lo_slice, q_hi_slice) = if rhs.len() > m {
        (&rhs[..m], &rhs[m..])
    } else {
        (rhs, &[] as &[F])
    };

    // z0 = p_lo · q_lo
    let z0 = if p_lo_slice.is_empty() || q_lo_slice.is_empty() {
        Vec::new()
    } else {
        mul_karatsuba_raw(p_lo_slice, q_lo_slice)
    };
    // z2 = p_hi · q_hi
    let z2 = if p_hi_slice.is_empty() || q_hi_slice.is_empty() {
        Vec::new()
    } else {
        mul_karatsuba_raw(p_hi_slice, q_hi_slice)
    };
    // (p_lo + p_hi) · (q_lo + q_hi)
    let p_sum = slice_add(p_lo_slice, p_hi_slice);
    let q_sum = slice_add(q_lo_slice, q_hi_slice);
    let z1_full = if p_sum.is_empty() || q_sum.is_empty() {
        Vec::new()
    } else {
        mul_karatsuba_raw(&p_sum, &q_sum)
    };

    // z1 = z1_full - z0 - z2  (over a field, subtraction is addition of
    // the additive inverse)
    let mut z1: Vec<F> = z1_full;
    for (i, c) in z0.iter().enumerate() {
        if i < z1.len() {
            z1[i] = z1[i].clone() - c.clone();
        } else {
            z1.push(-c.clone());
        }
    }
    for (i, c) in z2.iter().enumerate() {
        if i < z1.len() {
            z1[i] = z1[i].clone() - c.clone();
        } else {
            z1.push(-c.clone());
        }
    }

    // result = z0 + z1 · x^m + z2 · x^(2m)
    let out_len = lhs.len() + rhs.len() - 1;
    let zero = lhs[0].zero_like();
    let mut result = vec![zero; out_len];
    for (i, c) in z0.iter().enumerate() {
        result[i] = result[i].clone() + c.clone();
    }
    for (i, c) in z1.iter().enumerate() {
        let slot = i + m;
        if slot < out_len {
            result[slot] = result[slot].clone() + c.clone();
        }
    }
    for (i, c) in z2.iter().enumerate() {
        let slot = i + 2 * m;
        if slot < out_len {
            result[slot] = result[slot].clone() + c.clone();
        }
    }
    result
}

/// Top-level multiplication dispatcher used by the `Mul` operator and
/// the inherent [`FieldPoly::mul`] method. Handles zero-polynomial
/// short-circuits, then routes to schoolbook or Karatsuba based on
/// operand degrees.
fn mul_impl<F: FiniteField>(lhs: &[F], rhs: &[F]) -> FieldPoly<F> {
    if lhs.is_empty() || rhs.is_empty() {
        return FieldPoly { coeffs: Vec::new() };
    }

    let deg_lhs = lhs.len() - 1;
    let deg_rhs = rhs.len() - 1;
    if deg_lhs < KARATSUBA_THRESHOLD || deg_rhs < KARATSUBA_THRESHOLD {
        return mul_schoolbook_impl(lhs, rhs);
    }

    let coeffs = mul_karatsuba_raw(lhs, rhs);
    FieldPoly::new(coeffs)
}

impl<F: FiniteField> Mul<FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

    /// Polynomial multiplication with schoolbook/Karatsuba dispatch.
    ///
    /// Below [`KARATSUBA_THRESHOLD`] coefficients the schoolbook
    /// implementation is used; above it, recursive Karatsuba. The
    /// dispatch is transparent to callers — the operator always yields a
    /// normalised `FieldPoly<F>`.
    ///
    /// # Complexity
    ///
    /// `O(n · m)` field multiplications in the schoolbook regime and
    /// `O(n^{log₂ 3})` in the Karatsuba regime, where `n = self.len()`
    /// and `m = rhs.len()`. NTT-based multiplication for
    /// `TwoAdicField` inputs is a separate task (`e0b6f940`).
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
        assert_eq!(o.try_coeff(0), Some(&fp7(1)));
    }

    #[test]
    fn test_constant() {
        let p = FieldPoly::constant(fp7(5));
        assert_eq!(p.degree(), Some(0));
        assert_eq!(p.try_coeff(0), Some(&fp7(5)));
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
        assert_eq!(p.try_coeff(0), Some(&fp7(0)));
        assert_eq!(p.try_coeff(3), Some(&fp7(0)));
        assert_eq!(p.try_coeff(4), Some(&fp7(3)));
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
        assert_eq!(q.try_coeff(0), Some(&fp7(2)));
    }

    // -----------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------

    #[test]
    fn test_coeff_in_range_returns_some() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2)]);
        assert_eq!(p.try_coeff(0), Some(&fp7(1)));
        assert_eq!(p.try_coeff(1), Some(&fp7(2)));
    }

    #[test]
    fn test_try_coeff_out_of_range_returns_none() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2)]);
        assert_eq!(p.try_coeff(2), None);
        assert_eq!(p.try_coeff(100), None);
    }

    #[test]
    fn test_try_coeff_on_zero_poly_returns_none() {
        // try_coeff is the total Option-returning variant; the zero
        // polynomial returns None for every index without panicking.
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert_eq!(z.try_coeff(0), None);
        assert_eq!(z.try_coeff(1), None);
        assert_eq!(z.try_coeff(100), None);
    }

    #[test]
    fn test_coeff_or_zero_in_range() {
        let p = FieldPoly::new(vec![fp7(3), fp7(5)]);
        assert_eq!(p.coeff_or_zero(0, &fp7(0)), fp7(3));
        assert_eq!(p.coeff_or_zero(1, &fp7(0)), fp7(5));
    }

    #[test]
    fn test_coeff_or_zero_out_of_range() {
        let p = FieldPoly::new(vec![fp7(3), fp7(5)]);
        assert_eq!(p.coeff_or_zero(2, &fp7(0)), fp7(0));
        assert_eq!(p.coeff_or_zero(100, &fp7(0)), fp7(0));
    }

    #[test]
    fn test_coeff_or_zero_on_zero_poly() {
        // coeff_or_zero must be total even on the zero polynomial.
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert_eq!(z.coeff_or_zero(0, &fp7(0)), fp7(0));
        assert_eq!(z.coeff_or_zero(100, &fp7(0)), fp7(0));
    }

    #[test]
    fn test_coeff_or_zero_on_zero_poly_gf2m() {
        // Same totality test for a runtime-configured field: the sample
        // carries the field context and the returned zero lives in the
        // correct field.
        let field = Gf2mField::new(4, 0b10011);
        let z: FieldPoly<Gf2mElement> = FieldPoly::zero_like(&field.zero());
        let out = z.coeff_or_zero(0, &field.zero());
        assert!(out.is_zero());
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
        assert_eq!(c.try_coeff(0), Some(&fp7(4)));
        assert_eq!(c.try_coeff(1), Some(&fp7(6)));
        assert_eq!(c.degree(), Some(1));
    }

    #[test]
    fn test_add_cancels_leading_term() {
        // (2x + 1) + (5x + 3) = 7x + 4 = 0·x + 4 = constant 4 in Fp<7>.
        let a = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let b = FieldPoly::new(vec![fp7(3), fp7(5)]);
        let c = a + b;
        assert_eq!(c.degree(), Some(0));
        assert_eq!(c.try_coeff(0), Some(&fp7(4)));
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
        assert_eq!(c.try_coeff(0), Some(&fp7(4)));
        assert_eq!(c.try_coeff(1), Some(&fp7(4)));
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
        assert_eq!(n.try_coeff(0), Some(&fp7(4))); // -3 mod 7 = 4
        assert_eq!(n.try_coeff(1), Some(&fp7(2))); // -5 mod 7 = 2
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
        assert_eq!(b.try_coeff(1), Some(&fp7(4)));
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
        assert_eq!(b.try_coeff(0), Some(&fp7(1)));
    }

    // -----------------------------------------------------------------
    // Scalar multiplication
    // -----------------------------------------------------------------

    #[test]
    fn test_mul_scalar_basic() {
        // (2x + 3) * 2 = 4x + 6
        let p = FieldPoly::new(vec![fp7(3), fp7(2)]);
        let q = p.mul_scalar(&fp7(2));
        assert_eq!(q.try_coeff(0), Some(&fp7(6)));
        assert_eq!(q.try_coeff(1), Some(&fp7(4)));
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
        assert_eq!(c.try_coeff(0), Some(&fp7(4)));
        assert_eq!(c.try_coeff(1), Some(&fp7(4))); // 2*4 + 1*3 = 11 mod 7 = 4
        assert_eq!(c.try_coeff(2), Some(&fp7(6)));
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
        assert_eq!(product.try_coeff(0), Some(&(a1.clone() * b1.clone())));
        assert_eq!(
            product.try_coeff(1),
            Some(&(a1 * b2.clone() + a2.clone() * b1))
        );
        assert_eq!(product.try_coeff(2), Some(&(a2 * b2)));
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

    /// Thread-local shared GF(2^4) field used by the `Gf2mElement`
    /// proptests. All polynomials in a single test case share the same
    /// `Arc<FieldParams>`, so their elements compare equal-fielded for
    /// arithmetic ops (division across distinct field handles panics).
    fn gf16_field() -> Gf2mField {
        thread_local! {
            static FIELD: Gf2mField = Gf2mField::new(4, 0b10011);
        }
        FIELD.with(|f| f.clone())
    }

    /// Strategy: *non-zero* polynomial over `Gf2mElement` in GF(2^4).
    fn any_nonzero_gf16_poly() -> impl Strategy<Value = FieldPoly<Gf2mElement>> {
        (1usize..=5, 1u64..16).prop_flat_map(|(n, last)| {
            (
                prop::collection::vec(0u64..16, n.saturating_sub(1)),
                Just(last),
            )
                .prop_map(move |(mut mid, last)| {
                    mid.push(last);
                    let field = gf16_field();
                    FieldPoly::new(
                        mid.into_iter()
                            .map(|v| field.element(v))
                            .collect::<Vec<_>>(),
                    )
                })
        })
    }

    // -----------------------------------------------------------------
    // Horner evaluation + batch eval + from_roots + product
    // -----------------------------------------------------------------

    #[test]
    fn test_eval_horner_matches_expansion() {
        // p(x) = 3x² + 2x + 1 over Fp<7>
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        // p(4) = 48 + 8 + 1 = 57 ≡ 57 mod 7 = 1
        assert_eq!(p.eval(&fp7(4)), fp7(1));
        // p(0) = 1
        assert_eq!(p.eval(&fp7(0)), fp7(1));
        // p(1) = 3 + 2 + 1 = 6
        assert_eq!(p.eval(&fp7(1)), fp7(6));
    }

    #[test]
    fn test_eval_on_zero_polynomial_returns_zero() {
        // "Empty polynomial = zero" convention from the bdf95060 Task 2
        // breakdown: eval on the zero polynomial returns x.zero_like()
        // regardless of the evaluation point.
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert_eq!(z.eval(&fp7(3)), fp7(0));
        assert_eq!(z.eval(&fp7(0)), fp7(0));
    }

    #[test]
    fn test_eval_batch_on_zero_polynomial_returns_zeros() {
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        let ys = z.eval_batch(&[fp7(1), fp7(2), fp7(3)]);
        assert_eq!(ys, vec![fp7(0), fp7(0), fp7(0)]);
    }

    #[test]
    fn test_eval_batch_matches_individual() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        let xs = vec![fp7(0), fp7(1), fp7(2), fp7(5)];
        let ys = p.eval_batch(&xs);
        for (x, y) in xs.iter().zip(ys.iter()) {
            assert_eq!(p.eval(x), *y);
        }
    }

    #[test]
    fn test_eval_batch_empty_points_on_zero_poly_ok() {
        // Vacuous: an empty points slice must not panic even on the zero
        // polynomial.
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert_eq!(z.eval_batch(&[]), Vec::<FP7>::new());
    }

    // -----------------------------------------------------------------
    // batch_evaluate (subproduct tree)
    //
    // All unit tests below deliberately exercise the fallback path
    // (k < SUBPRODUCT_THRESHOLD or n < SUBPRODUCT_THRESHOLD) as well as
    // the subproduct path. The agreement proptests at the bottom of the
    // module cover the subproduct branch at scale.
    // -----------------------------------------------------------------

    #[test]
    fn test_batch_evaluate_k1() {
        // Single point: agrees with Horner eval.
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        let ys = p.batch_evaluate(&[fp7(4)]);
        assert_eq!(ys, vec![p.eval(&fp7(4))]);
    }

    #[test]
    fn test_batch_evaluate_k2() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        let xs = vec![fp7(0), fp7(5)];
        let ys = p.batch_evaluate(&xs);
        assert_eq!(ys, xs.iter().map(|x| p.eval(x)).collect::<Vec<_>>());
    }

    #[test]
    fn test_batch_evaluate_duplicate_points() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        let xs = vec![fp7(2), fp7(2), fp7(5), fp7(2)];
        let ys = p.batch_evaluate(&xs);
        assert_eq!(ys, xs.iter().map(|x| p.eval(x)).collect::<Vec<_>>());
        // Duplicates map to identical outputs.
        assert_eq!(ys[0], ys[1]);
        assert_eq!(ys[0], ys[3]);
    }

    #[test]
    fn test_batch_evaluate_contains_zero() {
        let p = FieldPoly::new(vec![fp7(2), fp7(3), fp7(1)]);
        let xs = vec![fp7(0), fp7(1), fp7(3)];
        let ys = p.batch_evaluate(&xs);
        assert_eq!(ys, xs.iter().map(|x| p.eval(x)).collect::<Vec<_>>());
        // p(0) = constant term = 2.
        assert_eq!(ys[0], fp7(2));
    }

    #[test]
    fn test_batch_evaluate_degree_zero_polynomial() {
        // Constant polynomial: every evaluation is the constant itself.
        let p = FieldPoly::constant(fp7(4));
        let xs = vec![fp7(0), fp7(1), fp7(6)];
        let ys = p.batch_evaluate(&xs);
        assert_eq!(ys, vec![fp7(4), fp7(4), fp7(4)]);
    }

    #[test]
    fn test_batch_evaluate_zero_polynomial() {
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        let xs = vec![fp7(0), fp7(1), fp7(3)];
        let ys = z.batch_evaluate(&xs);
        assert_eq!(ys, vec![fp7(0); 3]);
    }

    #[test]
    fn test_batch_evaluate_empty_points() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        assert_eq!(p.batch_evaluate(&[]), Vec::<FP7>::new());
    }

    #[test]
    fn test_batch_evaluate_exercises_subproduct_path_fp7() {
        // Inputs sized at >= SUBPRODUCT_THRESHOLD in both dimensions so
        // the fast path is exercised directly in the unit harness.
        let n = SUBPRODUCT_THRESHOLD + 4;
        let k = SUBPRODUCT_THRESHOLD + 3;
        let p_coeffs: Vec<FP7> = (0..=n).map(|i| fp7((i as u64 * 3 + 1) % 7)).collect();
        let p = FieldPoly::new(p_coeffs);
        let xs: Vec<FP7> = (0..k).map(|i| fp7((i as u64 * 5) % 7)).collect();

        let fast = p.batch_evaluate(&xs);
        let naive: Vec<FP7> = xs.iter().map(|x| p.eval(x)).collect();
        assert_eq!(fast, naive);
    }

    #[test]
    fn test_batch_evaluate_exercises_subproduct_path_gf16() {
        let field = Gf2mField::new(4, 0b10011);
        let n = SUBPRODUCT_THRESHOLD + 4;
        let k = SUBPRODUCT_THRESHOLD + 3;
        let p_coeffs: Vec<Gf2mElement> = (0..=n)
            .map(|i| field.element(((i as u64) * 7 + 1) & 0xF))
            .collect();
        let p = FieldPoly::new(p_coeffs);
        let xs: Vec<Gf2mElement> = (0..k)
            .map(|i| field.element((i as u64 * 11) & 0xF))
            .collect();

        let fast = p.batch_evaluate(&xs);
        let naive: Vec<Gf2mElement> = xs.iter().map(|x| p.eval(x)).collect();
        assert_eq!(fast, naive);
    }

    #[test]
    fn test_batch_evaluate_odd_sized_point_set_fp7() {
        // Odd k forces the odd-tail-carry branch in the bottom-up tree
        // build and the corresponding single-child descent during the
        // top-down reduction.
        let n = SUBPRODUCT_THRESHOLD + 4;
        let k = SUBPRODUCT_THRESHOLD + 1; // deliberately odd
        let p_coeffs: Vec<FP7> = (0..=n).map(|i| fp7((i as u64 * 2 + 1) % 7)).collect();
        let p = FieldPoly::new(p_coeffs);
        let xs: Vec<FP7> = (0..k).map(|i| fp7((i as u64 * 3) % 7)).collect();

        let fast = p.batch_evaluate(&xs);
        let naive: Vec<FP7> = xs.iter().map(|x| p.eval(x)).collect();
        assert_eq!(fast, naive);
    }

    #[test]
    fn test_from_roots_roots_vanish() {
        let roots = vec![fp7(1), fp7(2), fp7(3)];
        let p = FieldPoly::from_roots(&roots);
        for r in &roots {
            assert_eq!(p.eval(r), fp7(0));
        }
        assert_eq!(p.degree(), Some(3));
    }

    #[test]
    fn test_product_matches_sequential_mul() {
        let p1 = FieldPoly::new(vec![fp7(1), fp7(1)]);
        let p2 = FieldPoly::new(vec![fp7(2), fp7(1)]);
        let p3 = FieldPoly::new(vec![fp7(3), fp7(1)]);
        let prod = FieldPoly::product(&[p1.clone(), p2.clone(), p3.clone()]);
        assert_eq!(prod, &(&p1 * &p2) * &p3);
    }

    #[test]
    fn test_product_singleton_is_identity() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3)]);
        assert_eq!(FieldPoly::product(std::slice::from_ref(&p)), p);
    }

    #[test]
    #[should_panic(expected = "polys cannot be empty")]
    fn test_product_empty_panics() {
        FieldPoly::<FP7>::product(&[]);
    }

    // -----------------------------------------------------------------
    // Euclidean division / gcd
    // -----------------------------------------------------------------

    #[test]
    fn test_div_rem_identity() {
        let dividend = FieldPoly::new(vec![fp7(1), fp7(2), fp7(3), fp7(4)]);
        let divisor = FieldPoly::new(vec![fp7(1), fp7(1)]);
        let (q, r) = dividend.div_rem(&divisor);
        assert!(r.degree().map(|d| d < 1).unwrap_or(true));
        assert_eq!(&(&q * &divisor) + &r, dividend);
    }

    #[test]
    fn test_div_rem_exact_division() {
        let a = FieldPoly::new(vec![fp7(1), fp7(1)]);
        let b = FieldPoly::new(vec![fp7(2), fp7(1)]);
        let prod = &a * &b;
        let (q, r) = prod.div_rem(&a);
        assert!(r.is_zero());
        assert_eq!(q, b);
    }

    #[test]
    fn test_div_rem_dividend_smaller_than_divisor() {
        let dividend = FieldPoly::new(vec![fp7(5)]);
        let divisor = FieldPoly::new(vec![fp7(1), fp7(1)]);
        let (q, r) = dividend.div_rem(&divisor);
        assert!(q.is_zero());
        assert_eq!(r, dividend);
    }

    #[test]
    fn test_div_rem_zero_dividend() {
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        let divisor = FieldPoly::new(vec![fp7(1), fp7(1)]);
        let (q, r) = z.div_rem(&divisor);
        assert!(q.is_zero());
        assert!(r.is_zero());
    }

    #[test]
    #[should_panic(expected = "division by zero polynomial")]
    fn test_div_rem_by_zero_panics() {
        let dividend = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let zero: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        let _ = dividend.div_rem(&zero);
    }

    #[test]
    fn test_gcd_shared_linear_factor() {
        // (x - 1)(x - 2) and (x - 1)(x - 3); gcd should be monic (x - 1).
        let xm1 = FieldPoly::new(vec![-fp7(1), fp7(1)]);
        let xm2 = FieldPoly::new(vec![-fp7(2), fp7(1)]);
        let xm3 = FieldPoly::new(vec![-fp7(3), fp7(1)]);
        let p1 = &xm1 * &xm2;
        let p2 = &xm1 * &xm3;
        let g = FieldPoly::gcd(&p1, &p2);
        assert_eq!(g, xm1);
    }

    #[test]
    fn test_gcd_coprime_polynomials() {
        let p1 = FieldPoly::new(vec![-fp7(1), fp7(1)]);
        let p2 = FieldPoly::new(vec![-fp7(2), fp7(1)]);
        let g = FieldPoly::gcd(&p1, &p2);
        // Coprime linear factors — gcd is monic constant 1.
        assert_eq!(g, FieldPoly::constant(fp7(1)));
    }

    #[test]
    fn test_gcd_zero_arg_is_other() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        let g = FieldPoly::gcd(&p, &z);
        // gcd(p, 0) is p made monic.
        // p has leading coefficient 2, so monic(p) = p * 2^(-1) = p * 4 in Fp<7>.
        let p_monic = p.mul_scalar(&fp7(2).inv().unwrap());
        assert_eq!(g, p_monic);
    }

    #[test]
    fn test_gcd_bezout_witness_on_random_pairs_fp7() {
        // Bézout-style witness: with shared factor g and coprime cofactors
        // c1, c2, the polynomials g·c1 and g·c2 must have gcd equal to
        // the monic form of g. Fixing g and varying cofactors covers the
        // "random pair" case the issue success criteria call for while
        // avoiding the rare degenerate case where nominally-coprime
        // cofactors share a hidden factor under Fp<7>'s small base field.
        let g = FieldPoly::new(vec![fp7(3), fp7(1), fp7(1)]); // x² + x + 3
        let cofactors = [
            FieldPoly::new(vec![fp7(1), fp7(1)]),         // x + 1
            FieldPoly::new(vec![fp7(2), fp7(1)]),         // x + 2
            FieldPoly::new(vec![fp7(4), fp7(1)]),         // x + 4
            FieldPoly::new(vec![fp7(1), fp7(0), fp7(1)]), // x² + 1
            FieldPoly::new(vec![fp7(5), fp7(2), fp7(1)]), // x² + 2x + 5
        ];
        let g_monic = g.mul_scalar(&g.leading_coeff().unwrap().inv().unwrap());
        // For each pair of cofactors (c_i, c_j), gcd(g·c_i, g·c_j) must
        // be a scalar-constant multiple of g — equality of monic forms.
        for i in 0..cofactors.len() {
            for j in (i + 1)..cofactors.len() {
                let p1 = &g * &cofactors[i];
                let p2 = &g * &cofactors[j];
                let actual = FieldPoly::gcd(&p1, &p2);
                // The gcd must at least contain g as a factor.
                let (_, r) = actual.div_rem(&g_monic);
                assert!(
                    r.is_zero(),
                    "gcd(p1, p2) must be divisible by the shared monic factor g"
                );
                // And g must divide the gcd.
                let (_, r2) = g_monic.div_rem(&actual);
                assert!(
                    r2.is_zero(),
                    "the shared monic factor g must divide gcd(p1, p2)"
                );
            }
        }
    }

    // -----------------------------------------------------------------
    // Karatsuba cross-check: degrees above the threshold must agree with
    // schoolbook results.
    // -----------------------------------------------------------------

    #[test]
    fn test_karatsuba_matches_schoolbook_fp7() {
        // Construct two polynomials with degree well above KARATSUBA_THRESHOLD.
        let n = KARATSUBA_THRESHOLD + 8;
        let a_coeffs: Vec<FP7> = (0..=n).map(|i| fp7(((i as u64) * 3 + 1) % 7)).collect();
        let b_coeffs: Vec<FP7> = (0..=n).map(|i| fp7(((i as u64) * 5 + 2) % 7)).collect();
        let a = FieldPoly::new(a_coeffs.clone());
        let b = FieldPoly::new(b_coeffs.clone());

        let karatsuba = &a * &b;
        let schoolbook = mul_schoolbook_impl(&a_coeffs, &b_coeffs);
        assert_eq!(karatsuba, schoolbook);
    }

    #[test]
    fn test_karatsuba_matches_schoolbook_gf16() {
        let field = Gf2mField::new(4, 0b10011);
        let n = KARATSUBA_THRESHOLD + 6;
        let a_coeffs: Vec<Gf2mElement> = (0..=n)
            .map(|i| field.element(((i as u64) * 7 + 1) & 0xF))
            .collect();
        let b_coeffs: Vec<Gf2mElement> = (0..=n)
            .map(|i| field.element(((i as u64) * 3 + 2) & 0xF))
            .collect();
        let a = FieldPoly::new(a_coeffs.clone());
        let b = FieldPoly::new(b_coeffs.clone());

        let karatsuba = &a * &b;
        let schoolbook = mul_schoolbook_impl(&a_coeffs, &b_coeffs);
        assert_eq!(karatsuba, schoolbook);
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

        #[test]
        fn prop_div_rem_identity_fp7(
            a in any_nonzero_fp7_poly(),
            b in any_nonzero_fp7_poly(),
        ) {
            let (q, r) = a.div_rem(&b);
            // r.degree() < b.degree() (or r = 0)
            let db = b.degree().unwrap();
            match r.degree() {
                None => {}
                Some(d) => prop_assert!(d < db),
            }
            prop_assert_eq!(&(&q * &b) + &r, a);
        }

        #[test]
        fn prop_gcd_divides_both_fp7(
            a in any_nonzero_fp7_poly(),
            b in any_nonzero_fp7_poly(),
        ) {
            let g = FieldPoly::gcd(&a, &b);
            prop_assume!(!g.is_zero());
            let (_, ra) = a.div_rem(&g);
            let (_, rb) = b.div_rem(&g);
            prop_assert!(ra.is_zero());
            prop_assert!(rb.is_zero());
        }

        #[test]
        fn prop_gcd_commutative_fp7(
            a in any_fp7_poly(),
            b in any_fp7_poly(),
        ) {
            prop_assume!(!a.is_zero() || !b.is_zero());
            prop_assert_eq!(FieldPoly::gcd(&a, &b), FieldPoly::gcd(&b, &a));
        }

        #[test]
        fn prop_eval_matches_expansion_fp7(
            a in any_nonzero_fp7_poly(),
            x in 0u64..7,
        ) {
            let x = fp7(x);
            let mut expected = fp7(0);
            let mut pow = fp7(1);
            for i in 0..a.len() {
                expected += a.coeff(i) * pow;
                pow = pow * x;
            }
            prop_assert_eq!(a.eval(&x), expected);
        }

        #[test]
        fn prop_karatsuba_matches_schoolbook_fp7(
            a in prop::collection::vec(0u64..7, KARATSUBA_THRESHOLD..=KARATSUBA_THRESHOLD + 4),
            b in prop::collection::vec(0u64..7, KARATSUBA_THRESHOLD..=KARATSUBA_THRESHOLD + 4),
        ) {
            let a_coeffs: Vec<FP7> = a.into_iter().map(fp7).collect();
            let b_coeffs: Vec<FP7> = b.into_iter().map(fp7).collect();
            // Force non-zero leading coefficients.
            if a_coeffs.iter().all(FiniteField::is_zero) || b_coeffs.iter().all(FiniteField::is_zero) {
                return Ok(());
            }
            let a_poly = FieldPoly::new(a_coeffs.clone());
            let b_poly = FieldPoly::new(b_coeffs.clone());
            let school = mul_schoolbook_impl(&a_coeffs, &b_coeffs);
            prop_assert_eq!(&a_poly * &b_poly, school);
        }

        // ---------------------------------------------------------
        // Gf2mElement proptests (required by the issue success
        // criteria: div_rem identity + gcd commutativity + gcd
        // divides both inputs, all over GF(2^m)).
        // ---------------------------------------------------------

        #[test]
        fn prop_div_rem_identity_gf16(
            a in any_nonzero_gf16_poly(),
            b in any_nonzero_gf16_poly(),
        ) {
            let (q, r) = a.div_rem(&b);
            let db = b.degree().unwrap();
            match r.degree() {
                None => {}
                Some(d) => prop_assert!(d < db),
            }
            prop_assert_eq!(&(&q * &b) + &r, a);
        }

        #[test]
        fn prop_gcd_divides_both_gf16(
            a in any_nonzero_gf16_poly(),
            b in any_nonzero_gf16_poly(),
        ) {
            let g = FieldPoly::gcd(&a, &b);
            prop_assume!(!g.is_zero());
            let (_, ra) = a.div_rem(&g);
            let (_, rb) = b.div_rem(&g);
            prop_assert!(ra.is_zero());
            prop_assert!(rb.is_zero());
        }

        #[test]
        fn prop_gcd_commutative_gf16(
            a in any_nonzero_gf16_poly(),
            b in any_nonzero_gf16_poly(),
        ) {
            prop_assert_eq!(FieldPoly::gcd(&a, &b), FieldPoly::gcd(&b, &a));
        }

        #[test]
        fn prop_eval_matches_expansion_gf16(
            a in any_nonzero_gf16_poly(),
            x_val in 0u64..16,
        ) {
            let field = gf16_field();
            let x = field.element(x_val);
            let mut expected = field.zero();
            let mut pow = field.one();
            for i in 0..a.len() {
                expected += a.coeff(i) * pow.clone();
                pow = pow * x.clone();
            }
            prop_assert_eq!(a.eval(&x), expected);
        }

        #[test]
        fn prop_karatsuba_matches_schoolbook_gf16(
            a_len in KARATSUBA_THRESHOLD..=KARATSUBA_THRESHOLD + 4,
            b_len in KARATSUBA_THRESHOLD..=KARATSUBA_THRESHOLD + 4,
            a_seed in 0u64..16,
            b_seed in 0u64..16,
        ) {
            let field = gf16_field();
            // Deterministic coefficient generation from the seeds keeps the
            // input space small enough to finish within the 64-case budget
            // while still exercising the Karatsuba dispatch on GF(2^4).
            let a_coeffs: Vec<Gf2mElement> = (0..a_len)
                .map(|i| field.element((a_seed.wrapping_add(i as u64 * 7 + 1)) & 0xF))
                .collect();
            let b_coeffs: Vec<Gf2mElement> = (0..b_len)
                .map(|i| field.element((b_seed.wrapping_add(i as u64 * 5 + 2)) & 0xF))
                .collect();
            if a_coeffs.iter().all(FiniteField::is_zero) || b_coeffs.iter().all(FiniteField::is_zero) {
                return Ok(());
            }
            let a_poly = FieldPoly::new(a_coeffs.clone());
            let b_poly = FieldPoly::new(b_coeffs.clone());
            let school = mul_schoolbook_impl(&a_coeffs, &b_coeffs);
            prop_assert_eq!(&a_poly * &b_poly, school);
        }

        // -----------------------------------------------------------------
        // batch_evaluate agreement with per-point Horner.
        //
        // Two families of proptests:
        //   * `prop_batch_evaluate_matches_per_point_*` exercises the public
        //     `batch_evaluate` entry-point, which dispatches to either the
        //     subproduct tree or the naive Horner fallback depending on
        //     `SUBPRODUCT_THRESHOLD`.
        //   * `prop_batch_evaluate_subproduct_matches_per_point_*` calls
        //     the internal `batch_evaluate_subproduct` helper directly so
        //     the subproduct branch is exercised on small random inputs
        //     regardless of the public threshold.
        // -----------------------------------------------------------------

        #[test]
        fn prop_batch_evaluate_matches_per_point_fp7(
            poly_coeffs in prop::collection::vec(0u64..7, 0..40),
            point_vals in prop::collection::vec(0u64..7, 0..40),
        ) {
            let poly = FieldPoly::new(poly_coeffs.into_iter().map(fp7).collect::<Vec<_>>());
            let points: Vec<FP7> = point_vals.into_iter().map(fp7).collect();

            let fast = poly.batch_evaluate(&points);
            let naive: Vec<FP7> = points.iter().map(|x| poly.eval(x)).collect();
            prop_assert_eq!(fast, naive);
        }

        #[test]
        fn prop_batch_evaluate_subproduct_matches_per_point_fp7(
            poly_coeffs in prop::collection::vec(0u64..7, 1..20),
            point_vals in prop::collection::vec(0u64..7, 1..20),
        ) {
            let poly = FieldPoly::new(poly_coeffs.into_iter().map(fp7).collect::<Vec<_>>());
            let points: Vec<FP7> = point_vals.into_iter().map(fp7).collect();
            // Exercise the subproduct branch directly on small inputs;
            // the public `batch_evaluate` short-circuits these sizes to
            // the naive path, so we poke the helper through the
            // crate-private wrapper below. Non-empty points are
            // guaranteed by the strategy range; the zero polynomial is
            // tolerated by the helper because the leaf-remainder
            // extraction hands back `x.zero_like()` whenever `poly.div_rem`
            // returns the zero remainder.
            let fast = batch_evaluate_subproduct(&poly, &points);
            let naive: Vec<FP7> = points.iter().map(|x| poly.eval(x)).collect();
            prop_assert_eq!(fast, naive);
        }

        #[test]
        fn prop_batch_evaluate_idempotent_fp7(
            poly_coeffs in prop::collection::vec(0u64..7, 0..40),
            point_vals in prop::collection::vec(0u64..7, 0..40),
        ) {
            let poly = FieldPoly::new(poly_coeffs.into_iter().map(fp7).collect::<Vec<_>>());
            let points: Vec<FP7> = point_vals.into_iter().map(fp7).collect();

            let first = poly.batch_evaluate(&points);
            let second = poly.batch_evaluate(&points);
            prop_assert_eq!(first, second);
        }

        #[test]
        fn prop_batch_evaluate_matches_per_point_gf16(
            poly_vals in prop::collection::vec(0u64..16, 0..40),
            point_vals in prop::collection::vec(0u64..16, 0..40),
        ) {
            let field = gf16_field();
            let poly = FieldPoly::new(
                poly_vals.into_iter().map(|v| field.element(v)).collect::<Vec<_>>(),
            );
            let points: Vec<Gf2mElement> = point_vals
                .into_iter()
                .map(|v| field.element(v))
                .collect();

            let fast = poly.batch_evaluate(&points);
            let naive: Vec<Gf2mElement> = points.iter().map(|x| poly.eval(x)).collect();
            prop_assert_eq!(fast, naive);
        }

        #[test]
        fn prop_batch_evaluate_subproduct_matches_per_point_gf16(
            poly_vals in prop::collection::vec(0u64..16, 1..20),
            point_vals in prop::collection::vec(0u64..16, 1..20),
        ) {
            let field = gf16_field();
            let poly = FieldPoly::new(
                poly_vals.into_iter().map(|v| field.element(v)).collect::<Vec<_>>(),
            );
            let points: Vec<Gf2mElement> = point_vals
                .into_iter()
                .map(|v| field.element(v))
                .collect();
            let fast = batch_evaluate_subproduct(&poly, &points);
            let naive: Vec<Gf2mElement> = points.iter().map(|x| poly.eval(x)).collect();
            prop_assert_eq!(fast, naive);
        }

        #[test]
        fn prop_batch_evaluate_idempotent_gf16(
            poly_vals in prop::collection::vec(0u64..16, 0..40),
            point_vals in prop::collection::vec(0u64..16, 0..40),
        ) {
            let field = gf16_field();
            let poly = FieldPoly::new(
                poly_vals.into_iter().map(|v| field.element(v)).collect::<Vec<_>>(),
            );
            let points: Vec<Gf2mElement> = point_vals
                .into_iter()
                .map(|v| field.element(v))
                .collect();

            let first = poly.batch_evaluate(&points);
            let second = poly.batch_evaluate(&points);
            prop_assert_eq!(first, second);
        }
    }
}
