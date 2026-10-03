//! Generic univariate polynomials over any [`FiniteField`].
//!
//! [`FieldPoly`] stores coefficients in ascending-degree order (`coeffs[i]` is
//! the coefficient of `x^i`) over any [`FiniteField`], including fields with
//! runtime parameters; [`Gf2mPoly_<V>`](crate::gf2m::Gf2mPoly_) is an alias of
//! `FieldPoly<Gf2mElement_<V>>`.
//!
//! # The normalisation invariant
//!
//! Every constructor and every mutating operation leaves the `coeffs` vector
//! without trailing zero coefficients. The zero polynomial is the unique
//! polynomial with an empty `coeffs` vector; every other polynomial's final
//! coefficient is non-zero. Equality is structural on the normalised
//! coefficients.
//!
//! # Dispatch
//!
//! Operations generic over `F: FiniteField` use schoolbook or Karatsuba
//! multiplication and schoolbook division. The `F: TwoAdicField` entry points
//! ([`mul_fast`], [`FieldPoly::div_rem_auto`],
//! [`FieldPoly::batch_evaluate_auto`]) add the NTT and Newton-iteration paths
//! under separate names, because the generic impls cannot be specialised.
//! Every dispatcher compares against a value of the active
//! [`crate::tuning::CoreTuning`] profile.

use crate::field::{FiniteField, TwoAdicField};
use crate::tuning;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// A univariate polynomial over a finite field `F`.
///
/// Coefficients are stored in ascending-degree order: `coeffs[i]` is the
/// coefficient of `x^i`. The coefficient vector is **always normalised**
/// — empty (for the zero polynomial) or non-empty with a non-zero
/// trailing element.
///
/// See the [module documentation](self) for the invariant.
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
// `is_zero` is the emptiness predicate, so no `is_empty` exists.
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
    pub fn new(coeffs: Vec<F>) -> Self {
        let mut poly = FieldPoly { coeffs };
        poly.normalise();
        poly
    }

    /// Alias of [`FieldPoly::new`] whose name states that trailing zeros are
    /// stripped.
    pub fn from_coeffs_trimmed(coeffs: Vec<F>) -> Self {
        Self::new(coeffs)
    }

    /// Returns the zero polynomial in the same field as `sample`.
    ///
    /// `_sample` only fixes the type `F`; it is not stored.
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
    pub fn zero_like(_sample: &F) -> Self {
        FieldPoly { coeffs: Vec::new() }
    }

    /// Returns the constant-`1` polynomial in the same field as
    /// `sample`.
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
    pub fn degree(&self) -> Option<usize> {
        if self.coeffs.is_empty() {
            None
        } else {
            Some(self.coeffs.len() - 1)
        }
    }

    /// Returns `true` iff this is the zero polynomial.
    pub fn is_zero(&self) -> bool {
        self.coeffs.is_empty()
    }

    /// Returns a reference to the coefficient of `x^i`, or `None` if
    /// `i` is out of range (including the whole zero-polynomial case).
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
    pub fn try_coeff(&self, i: usize) -> Option<&F> {
        self.coeffs.get(i)
    }

    /// Returns the `i`-th coefficient by value; for `i >= self.len()` on a
    /// non-zero polynomial, a zero built with [`FiniteField::zero_like`].
    ///
    /// # Panics
    ///
    /// Panics on the zero polynomial, which has no coefficient to derive a
    /// zero from. [`FieldPoly::try_coeff`] and [`FieldPoly::coeff_or_zero`]
    /// are total.
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
    pub fn coeff(&self, i: usize) -> F {
        if let Some(c) = self.coeffs.get(i) {
            c.clone()
        } else {
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
    pub fn leading_coeff(&self) -> Option<&F> {
        self.coeffs.last()
    }

    /// Returns the number of stored coefficients (`degree + 1` for a
    /// non-zero polynomial, `0` for the zero polynomial).
    pub fn len(&self) -> usize {
        self.coeffs.len()
    }

    /// Returns an iterator over the coefficients in ascending-degree
    /// order.
    pub fn iter(&self) -> impl Iterator<Item = &F> {
        self.coeffs.iter()
    }

    // -----------------------------------------------------------------
    // Inherent multiplication (schoolbook / Karatsuba dispatch)
    // -----------------------------------------------------------------

    /// Polynomial multiplication: schoolbook when either operand degree is
    /// below the active `polynomial.karatsuba_min_degree()` value, recursive
    /// Karatsuba otherwise. The [`core::ops::Mul`] impls delegate here.
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
    pub fn mul_scalar(&self, c: &F) -> Self {
        if c.is_zero() || self.is_zero() {
            return FieldPoly { coeffs: Vec::new() };
        }
        let coeffs: Vec<F> = self.coeffs.iter().map(|a| a.clone() * c.clone()).collect();
        FieldPoly::new(coeffs)
    }

    /// Multiplies this polynomial in place by a scalar.
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
    /// The zero polynomial evaluates to `x.zero_like()`.
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
    pub fn eval(&self, x: &F) -> F {
        if self.coeffs.is_empty() {
            return x.zero_like();
        }

        let mut result = self.coeffs.last().unwrap().clone();
        for i in (0..self.coeffs.len() - 1).rev() {
            result = result * x.clone() + self.coeffs[i].clone();
        }
        result
    }

    /// Evaluates the polynomial at every point in `points`, returning
    /// the values in the same order.
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
    pub fn eval_batch(&self, points: &[F]) -> Vec<F> {
        points.iter().map(|x| self.eval(x)).collect()
    }

    /// Evaluates the polynomial at a square matrix `A`, returning
    /// `p(A) = c_d · A^d + c_{d-1} · A^{d-1} + … + c_1 · A + c_0 · I`.
    ///
    /// The zero polynomial evaluates to the `n × n` zero matrix.
    ///
    /// # Panics
    ///
    /// Panics if `a` is not square.
    ///
    /// # Complexity
    ///
    /// `O(d · n³)` field operations for `d = self.degree()`: Horner with one
    /// [`gemm`](crate::field::matrix::gemm) per coefficient.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // p(x) = x² + 1 over Fp<7>; A = identity ⇒ p(A) = 2 · I.
    /// let p = FieldPoly::new(vec![
    ///     Fp::<7>::new(1),
    ///     Fp::<7>::new(0),
    ///     Fp::<7>::new(1),
    /// ]);
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// let pa = p.eval_at_matrix(&id);
    /// assert_eq!(pa.get(0, 0), Fp::<7>::new(2));
    /// assert_eq!(pa.get(1, 1), Fp::<7>::new(2));
    /// assert_eq!(pa.get(2, 2), Fp::<7>::new(2));
    /// assert_eq!(pa.get(0, 1), Fp::<7>::new(0));
    /// ```
    pub fn eval_at_matrix(
        &self,
        a: &crate::field::matrix::FieldMatrix<F>,
    ) -> crate::field::matrix::FieldMatrix<F> {
        use crate::field::matrix::{gemm_into_view, FieldMatrix};
        let (m, n) = a.shape();
        assert_eq!(
            m, n,
            "FieldPoly::eval_at_matrix: input must be square (got {}×{})",
            m, n
        );
        // A 0×0 result needs a zero witness: a coefficient, else
        // `F::zero_hint()`.
        if n == 0 {
            let zero_opt: Option<F> = self
                .coeffs
                .first()
                .map(|c| c.zero_like())
                .or_else(F::zero_hint);
            return match zero_opt {
                Some(z) => FieldMatrix::<F>::new(0, 0, z),
                // No witness: `a` is already the empty 0×0 matrix.
                None => a.clone(),
            };
        }
        let zero: F = a.get(0, 0).zero_like();
        let Some(deg) = self.degree() else {
            return FieldMatrix::new(n, n, zero);
        };
        let mut result = FieldMatrix::<F>::new(n, n, zero.clone());
        let lead = self.coeffs[deg].clone();
        for i in 0..n {
            result.set(i, i, lead.clone());
        }
        // Horner: result := result · a + c_k · I.
        let mut scratch = FieldMatrix::<F>::new(n, n, zero.clone());
        for k in (0..deg).rev() {
            gemm_into_view(&result, a, scratch.submat_mut(.., ..));
            let ck = self.coeffs[k].clone();
            if !ck.is_zero() {
                for i in 0..n {
                    let cur = scratch.get(i, i);
                    scratch.set(i, i, cur + ck.clone());
                }
            }
            // `gemm_into_view` overwrites every cell, so the swapped-in
            // scratch needs no reset.
            std::mem::swap(&mut result, &mut scratch);
        }
        result
    }

    /// Evaluates the polynomial at every point in `points`: by a subproduct
    /// tree over schoolbook [`FieldPoly::div_rem`] when both `self.len()` and
    /// `points.len()` reach the active `polynomial.subproduct_min_len()`
    /// value, by per-point Horner otherwise.
    ///
    /// Returns the same values as [`FieldPoly::eval_batch`]; `points` may be
    /// empty or contain duplicates. [`FieldPoly::batch_evaluate_auto`] is the
    /// [`TwoAdicField`] form with [`FieldPoly::div_rem_auto`] reductions.
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
    /// `O(n · k + k² log k)` field operations on the subproduct path and
    /// `O(n · k)` on the Horner path, for `n = self.len()` and
    /// `k = points.len()`.
    pub fn batch_evaluate(&self, points: &[F]) -> Vec<F> {
        match batch_evaluate_route(self.coeffs.len(), points.len()) {
            BatchEvaluateRoute::Horner => self.eval_batch(points),
            BatchEvaluateRoute::SubproductTree => batch_evaluate_subproduct(self, points),
        }
    }

    // -----------------------------------------------------------------
    // Construction from roots and products
    // -----------------------------------------------------------------

    /// Builds the monic polynomial whose roots are exactly `roots`:
    /// `(x - r_0)(x - r_1) · … · (x - r_{n-1})`.
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
    /// `O(M(nd) log n)` via [`FieldPoly::batch_mul`], where `n` is
    /// the number of roots and `M(k)` is the cost of multiplying two
    /// degree-`k` polynomials.
    pub fn from_roots(roots: &[F]) -> Self {
        assert!(
            !roots.is_empty(),
            "FieldPoly::from_roots: roots cannot be empty"
        );

        let one = roots[0].one_like();
        let factors: Vec<FieldPoly<F>> = roots
            .iter()
            .map(|r| FieldPoly::new(vec![-r.clone(), one.clone()]))
            .collect();
        FieldPoly::batch_mul(&factors)
    }

    /// Computes the product of a non-empty slice of polynomials.
    ///
    /// Delegates to [`FieldPoly::batch_mul`].
    ///
    /// # Panics
    ///
    /// Panics if `polys` is empty. Use [`FieldPoly::batch_mul_with_field`]
    /// if an empty slice must return the multiplicative identity.
    ///
    /// # Complexity
    ///
    /// `O(M(nd) log n)` — same as [`FieldPoly::batch_mul`].
    pub fn product(polys: &[FieldPoly<F>]) -> Self {
        assert!(
            !polys.is_empty(),
            "FieldPoly::product: polys cannot be empty"
        );
        FieldPoly::batch_mul(polys)
    }

    // -----------------------------------------------------------------
    // Batch product and GCD
    // -----------------------------------------------------------------

    /// Computes the product of a non-empty slice of polynomials by a balanced
    /// binary product tree.
    ///
    /// # Panics
    ///
    /// Panics if `polys` is empty (no field sample available to construct
    /// the multiplicative identity). Use [`FieldPoly::batch_mul_with_field`]
    /// when an empty slice must return the constant-1 polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // (x + 1)(x + 2)(x + 3) over Fp<7>
    /// let polys: Vec<FieldPoly<Fp<7>>> = [1u64, 2, 3]
    ///     .iter()
    ///     .map(|&c| FieldPoly::new(vec![Fp::<7>::new(c), Fp::<7>::new(1)]))
    ///     .collect();
    /// let prod = FieldPoly::batch_mul(&polys);
    /// // Evaluate at x = 1: (1+1)(1+2)(1+3) = 2·3·4 = 24 ≡ 3 (mod 7).
    /// assert_eq!(prod.eval(&Fp::<7>::new(1)), Fp::<7>::new(3));
    /// assert_eq!(prod.degree(), Some(3));
    /// ```
    ///
    /// Single-element slice is the identity:
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(5), Fp::<7>::new(1)]);
    /// assert_eq!(FieldPoly::batch_mul(std::slice::from_ref(&p)), p);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(M(nd) log n)` field operations, where `n = polys.len()`, `d` is
    /// the average polynomial degree, and `M(k)` is the cost of
    /// multiplying two degree-`k` polynomials (`O(k²)` schoolbook or
    /// `O(k^{log₂ 3})` Karatsuba).
    pub fn batch_mul(polys: &[Self]) -> Self {
        assert!(
            !polys.is_empty(),
            "FieldPoly::batch_mul: polys cannot be empty; \
             use batch_mul_with_field for empty-slice support"
        );

        if polys.len() == 1 {
            return polys[0].clone();
        }

        let mut current: Vec<Self> = polys.to_vec();
        while current.len() > 1 {
            let mut next: Vec<Self> = Vec::with_capacity(current.len().div_ceil(2));
            let mut i = 0;
            while i + 1 < current.len() {
                next.push(&current[i] * &current[i + 1]);
                i += 2;
            }
            if i < current.len() {
                // Odd tail: carry the unpaired element up unchanged.
                next.push(current.remove(i));
            }
            current = next;
        }
        current.remove(0)
    }

    /// Computes the product of a slice of polynomials using a balanced
    /// binary product tree, returning the constant-1 polynomial (in the
    /// same field as `sample`) when `polys` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let sample = Fp::<7>::new(0);
    ///
    /// // Empty slice returns the multiplicative identity.
    /// let prod = FieldPoly::batch_mul_with_field(&sample, &[]);
    /// assert_eq!(prod, FieldPoly::one_like(&sample));
    ///
    /// // Non-empty slice works identically to batch_mul.
    /// let polys: Vec<FieldPoly<Fp<7>>> = [1u64, 2]
    ///     .iter()
    ///     .map(|&c| FieldPoly::new(vec![Fp::<7>::new(c), Fp::<7>::new(1)]))
    ///     .collect();
    /// let prod2 = FieldPoly::batch_mul_with_field(&sample, &polys);
    /// assert_eq!(prod2, FieldPoly::batch_mul(&polys));
    /// ```
    pub fn batch_mul_with_field(sample: &F, polys: &[Self]) -> Self {
        if polys.is_empty() {
            return FieldPoly::one_like(sample);
        }
        FieldPoly::batch_mul(polys)
    }

    /// Computes a single GCD of a non-empty slice of polynomials by
    /// folding pairwise from the first element using
    /// [`FieldPoly::gcd`].
    ///
    /// The result is monic, or zero when every element is zero.
    ///
    /// # Panics
    ///
    /// Panics if `polys` is empty (no canonical GCD identity exists on an
    /// empty set of polynomials).
    ///
    /// # Examples
    ///
    /// Shared factor:
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // d = x + 1; a = d·(x+2), b = d·(x+3), c = d·(x+4)
    /// let d  = FieldPoly::new(vec![Fp::<7>::new(1), Fp::<7>::new(1)]);
    /// let xp2 = FieldPoly::new(vec![Fp::<7>::new(2), Fp::<7>::new(1)]);
    /// let xp3 = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(1)]);
    /// let xp4 = FieldPoly::new(vec![Fp::<7>::new(4), Fp::<7>::new(1)]);
    /// let polys = vec![&d * &xp2, &d * &xp3, &d * &xp4];
    /// let g = FieldPoly::batch_gcd(&polys);
    /// // d divides every element, so d divides gcd([a*d, b*d, c*d]).
    /// // Equivalently: gcd([a*d, b*d, c*d]) = d · gcd(a, b, c).
    /// // Verify d | g (i.e. g is divisible by d):
    /// let (_, r) = g.div_rem(&d);
    /// assert!(r.is_zero());
    /// ```
    ///
    /// Single-element slice returns a monic version of that element:
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<7>::new(3), Fp::<7>::new(2)]); // 2x + 3
    /// let g = FieldPoly::batch_gcd(std::slice::from_ref(&p));
    /// // gcd of a single element is the monic form of that element.
    /// assert_eq!(g.leading_coeff(), Some(&Fp::<7>::new(1)));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n · G)` where `n = polys.len()` and `G` is the cost of a
    /// single [`FieldPoly::gcd`] call.
    pub fn batch_gcd(polys: &[Self]) -> Self {
        assert!(
            !polys.is_empty(),
            "FieldPoly::batch_gcd: polys cannot be empty (no GCD identity on an empty set)"
        );

        let first = polys[0].clone();
        if polys.len() == 1 {
            // gcd(a, a) is monic(a).
            return FieldPoly::gcd(&first, &first);
        }
        polys
            .iter()
            .skip(1)
            .fold(first, |acc, p| FieldPoly::gcd(&acc, p))
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
        // Length of the working remainder, tracked without re-scanning.
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

    /// Returns the monic least common multiple `lcm(a, b) = a·b /
    /// gcd(a, b)`.
    ///
    /// By convention `lcm(a, 0) = lcm(0, b) = 0`: if either input is
    /// the zero polynomial, the result is the zero polynomial.
    ///
    /// # Panics
    ///
    /// Panics if both `a` and `b` are the zero polynomial and `F` is
    /// a runtime-context field with no static zero witness
    /// (`F::zero_hint()` returns `None`, e.g. `Gf2mElement`) — there
    /// is no `F` value anywhere to seed the zero result. `ConstField`
    /// types, and any call where at least one operand is non-zero,
    /// never hit this path.
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
    /// let l = FieldPoly::lcm(&p1, &p2);
    /// assert_eq!(l, &p1 * &xm3); // (x - 1)(x - 2)(x - 3)
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n²)` field operations in the worst case, where `n` is the
    /// maximum degree.
    pub fn lcm(a: &FieldPoly<F>, b: &FieldPoly<F>) -> FieldPoly<F> {
        if a.is_zero() || b.is_zero() {
            // A zero result needs a witness: a coefficient of the non-zero
            // operand, else `F::zero_hint()`.
            let sample = if let Some(c) = a.iter().next() {
                c.clone()
            } else if let Some(c) = b.iter().next() {
                c.clone()
            } else if let Some(z) = F::zero_hint() {
                z
            } else {
                unreachable!(
                    "FieldPoly::lcm: lcm of two zero polynomials over a \
                     runtime-context field with no zero witness"
                );
            };
            return FieldPoly::zero_like(&sample);
        }
        let g = FieldPoly::gcd(a, b);
        let (q, _r) = (a * b).div_rem(&g);
        if let Some(lead) = q.coeffs.last() {
            if !lead.is_one() {
                if let Some(inv) = lead.inv() {
                    let monic: Vec<F> = q.coeffs.iter().map(|c| c.clone() * inv.clone()).collect();
                    return FieldPoly::new(monic);
                }
            }
        }
        q
    }

    // -----------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------

    /// Trims trailing zero coefficients so the invariant holds.
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

/// Coefficient-wise `lhs + rhs`, or `lhs − rhs` when `rhs_is_neg`, normalised.
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

    fn neg(self) -> FieldPoly<F> {
        let coeffs: Vec<F> = self.coeffs.into_iter().map(|c| -c).collect();
        // `new` re-normalises in case `FiniteField::neg` yields a zero.
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

/// Conservative default for `polynomial.karatsuba_min_degree()` in the active
/// [`crate::tuning::CoreTuning`].
///
/// Operand degrees strictly less than the active profile value use the
/// schoolbook algorithm; at or above it both operands recurse through
/// Karatsuba. [`crate::tuning::CoreTuning::CONSERVATIVE`] consumes this
/// constant.
pub const KARATSUBA_THRESHOLD: usize = 32;

/// The selected arm of the [`FieldPoly::mul`] schoolbook/Karatsuba
/// dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MulRoute {
    /// Use schoolbook multiplication.
    Schoolbook,
    /// Use recursive Karatsuba multiplication.
    Karatsuba,
}

/// Reports the [`FieldPoly::mul`] arm for two operand degrees.
///
/// The comparison uses the active
/// `polynomial.karatsuba_min_degree()` profile value. Empty operands are
/// handled by the dispatcher before this selector is called.
#[must_use]
pub fn mul_route(lhs_degree: usize, rhs_degree: usize) -> MulRoute {
    mul_route_resolved(
        tuning::active().polynomial().karatsuba_min_degree(),
        lhs_degree,
        rhs_degree,
    )
}

/// Reports the [`FieldPoly::mul`] arm for two operand degrees against an
/// already-resolved `karatsuba_min_degree`.
fn mul_route_resolved(
    karatsuba_min_degree: usize,
    lhs_degree: usize,
    rhs_degree: usize,
) -> MulRoute {
    if lhs_degree < karatsuba_min_degree || rhs_degree < karatsuba_min_degree {
        MulRoute::Schoolbook
    } else {
        MulRoute::Karatsuba
    }
}

/// Conservative default for `polynomial.subproduct_min_len()` in the active
/// [`crate::tuning::CoreTuning`] for [`FieldPoly::batch_evaluate`] (generic,
/// schoolbook [`FieldPoly::div_rem`]) and
/// [`FieldPoly::batch_evaluate_auto`] ([`TwoAdicField`], Newton-iteration
/// [`FieldPoly::div_rem_auto`]) between the subproduct-tree algorithm
/// and the naive per-point Horner fallback.
///
/// When `points.len()` or `self.coeffs.len()` is strictly less than the active
/// profile value, the corresponding dispatcher falls through to
/// [`FieldPoly::eval_batch`] (`O(n · k)` naive Horner); at or above
/// the threshold it pays for the subproduct tree build and top-down
/// reduction.
pub const SUBPRODUCT_THRESHOLD: usize = 4096;

/// The selected arm of batch evaluation dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchEvaluateRoute {
    /// Use per-point Horner evaluation.
    Horner,
    /// Use the subproduct-tree algorithm.
    SubproductTree,
}

/// Reports the arm selected by [`FieldPoly::batch_evaluate`].
///
/// The comparison uses the active `polynomial.subproduct_min_len()` profile
/// value and applies to both the polynomial coefficient count and the point
/// count.
#[must_use]
pub fn batch_evaluate_route(poly_len: usize, points_len: usize) -> BatchEvaluateRoute {
    let subproduct_min_len = tuning::active().polynomial().subproduct_min_len();
    if points_len < subproduct_min_len || poly_len < subproduct_min_len {
        BatchEvaluateRoute::Horner
    } else {
        BatchEvaluateRoute::SubproductTree
    }
}

/// Reports the arm selected by [`FieldPoly::batch_evaluate_auto`].
///
/// This has the same size policy as [`batch_evaluate_route`], while naming
/// the `TwoAdicField`-specialised dispatcher explicitly.
#[must_use]
pub fn batch_evaluate_auto_route(poly_len: usize, points_len: usize) -> BatchEvaluateRoute {
    let subproduct_min_len = tuning::active().polynomial().subproduct_min_len();
    if points_len < subproduct_min_len || poly_len < subproduct_min_len {
        BatchEvaluateRoute::Horner
    } else {
        BatchEvaluateRoute::SubproductTree
    }
}

/// Builds the subproduct tree for a slice of evaluation points.
///
/// Returns a flat `Vec<Vec<FieldPoly<F>>>` where `levels[0]` holds the
/// linear leaves `M_i = x - points[i]`, `levels[h + 1]` holds the
/// pairwise products of `levels[h]`, and `levels.last()` is a
/// single-element vector containing the full product
/// `M(x) = ∏ (x - points[i])`. Odd-sized levels carry the last node up
/// unchanged.
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly::build_subproduct_tree;
/// use gf2_core::gfp::Fp;
///
/// let levels = build_subproduct_tree(&[Fp::<7>::new(1), Fp::<7>::new(2)]);
/// assert_eq!(levels.len(), 2);              // leaves + root
/// assert_eq!(levels[0].len(), 2);           // two leaves
/// assert_eq!(levels[1].len(), 1);           // single root
/// ```
///
/// # Panics
///
/// Panics if `points` is empty.
///
/// # Complexity
///
/// `k − 1` polynomial multiplications for `k = points.len()`.
pub fn build_subproduct_tree<F: FiniteField>(points: &[F]) -> Vec<Vec<FieldPoly<F>>> {
    debug_assert!(!points.is_empty());

    let one = points[0].one_like();
    let leaves: Vec<FieldPoly<F>> = points
        .iter()
        .map(|p| FieldPoly::new(vec![-p.clone(), one.clone()]))
        .collect();

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
            // Odd tail: carry the last node up without a partner.
            next.push(cur[i].clone());
        }
        levels.push(next);
    }
    levels
}

/// Subproduct-tree batch evaluation with schoolbook [`FieldPoly::div_rem`]
/// reductions, without the `polynomial.subproduct_min_len()` gate of
/// [`FieldPoly::batch_evaluate`].
///
/// `poly` is reduced modulo the root of [`build_subproduct_tree`], then each
/// remainder modulo its node's children, until every leaf holds the constant
/// `poly(points[i])`. `points` may contain duplicates.
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
/// Panics if `points` is empty.
///
/// # Complexity
///
/// `O(n · k + k² log k)` field operations for `n = poly.len()` and
/// `k = points.len()`.
pub fn batch_evaluate_subproduct<F: FiniteField>(poly: &FieldPoly<F>, points: &[F]) -> Vec<F> {
    batch_evaluate_subproduct_with_reduce(poly, points, |a, b| a.div_rem(b).1)
}

/// [`TwoAdicField`] form of [`batch_evaluate_subproduct`]: the reductions use
/// [`FieldPoly::div_rem_auto`].
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly::batch_evaluate_subproduct_auto;
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// let p = FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(2), Fp::<65537>::new(3)]);
/// let xs = vec![Fp::<65537>::new(0), Fp::<65537>::new(1), Fp::<65537>::new(4)];
/// let ys = batch_evaluate_subproduct_auto(&p, &xs);
/// assert_eq!(ys, xs.iter().map(|x| p.eval(x)).collect::<Vec<_>>());
/// ```
///
/// # Panics
///
/// Panics if `points` is empty.
///
/// # Complexity
///
/// `O(M(n) · log k + k² log k)` field operations, where `M(n) = O(n log n)`
/// is the NTT multiplication cost.
pub fn batch_evaluate_subproduct_auto<F: TwoAdicField>(
    poly: &FieldPoly<F>,
    points: &[F],
) -> Vec<F> {
    batch_evaluate_subproduct_with_reduce(poly, points, |a, b| a.div_rem_auto(b).1)
}

/// Traversal shared by [`batch_evaluate_subproduct`] and
/// [`batch_evaluate_subproduct_auto`], which differ only in `reduce`.
fn batch_evaluate_subproduct_with_reduce<F, R>(
    poly: &FieldPoly<F>,
    points: &[F],
    reduce: R,
) -> Vec<F>
where
    F: FiniteField,
    R: Fn(&FieldPoly<F>, &FieldPoly<F>) -> FieldPoly<F>,
{
    debug_assert!(!points.is_empty());

    let k = points.len();
    let levels = build_subproduct_tree(points);

    // Invariant while descending: `rems[h][j] = poly mod levels[h][j]`. Only
    // the current level of remainders is kept.
    let root_level = levels.len() - 1;
    debug_assert_eq!(levels[root_level].len(), 1);

    let root_rem = reduce(poly, &levels[root_level][0]);
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
                let left_rem = reduce(rem, &children[left_idx]);
                let right_rem = reduce(rem, &children[right_idx]);
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

    // A remainder modulo a linear leaf is zero (`[]`) or a constant (`[c]`).
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
/// normalised; the caller (`mul_dispatch`) guarantees that. Returns raw
/// coefficients of length `lhs.len() + rhs.len() - 1` **without**
/// normalising — the top-level `FieldPoly::new` at the entry point does
/// the final trim.
fn mul_karatsuba_raw<F: FiniteField>(lhs: &[F], rhs: &[F], karatsuba_min_degree: usize) -> Vec<F> {
    debug_assert!(!lhs.is_empty() && !rhs.is_empty());

    let deg_lhs = lhs.len() - 1;
    let deg_rhs = rhs.len() - 1;

    if deg_lhs < karatsuba_min_degree || deg_rhs < karatsuba_min_degree {
        let out = mul_schoolbook_impl(lhs, rhs);
        // Pad back to the unnormalised length the combine step expects.
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
        mul_karatsuba_raw(p_lo_slice, q_lo_slice, karatsuba_min_degree)
    };
    // z2 = p_hi · q_hi
    let z2 = if p_hi_slice.is_empty() || q_hi_slice.is_empty() {
        Vec::new()
    } else {
        mul_karatsuba_raw(p_hi_slice, q_hi_slice, karatsuba_min_degree)
    };
    // (p_lo + p_hi) · (q_lo + q_hi)
    let p_sum = slice_add(p_lo_slice, p_hi_slice);
    let q_sum = slice_add(q_lo_slice, q_hi_slice);
    let z1_full = if p_sum.is_empty() || q_sum.is_empty() {
        Vec::new()
    } else {
        mul_karatsuba_raw(&p_sum, &q_sum, karatsuba_min_degree)
    };

    // z1 = z1_full - z0 - z2
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

/// Schoolbook/Karatsuba dispatch over an already-resolved
/// `karatsuba_min_degree`. `lhs` and `rhs` must be non-empty.
///
/// [`FieldPoly::mul`] (through `mul_impl`) and [`mul_fast`] resolve the active
/// profile once and then call this, so no call path reads `tuning::active()`
/// twice.
fn mul_dispatch<F: FiniteField>(lhs: &[F], rhs: &[F], karatsuba_min_degree: usize) -> FieldPoly<F> {
    debug_assert!(!lhs.is_empty() && !rhs.is_empty());

    let deg_lhs = lhs.len() - 1;
    let deg_rhs = rhs.len() - 1;
    match mul_route_resolved(karatsuba_min_degree, deg_lhs, deg_rhs) {
        MulRoute::Schoolbook => mul_schoolbook_impl(lhs, rhs),
        MulRoute::Karatsuba => FieldPoly::new(mul_karatsuba_raw(lhs, rhs, karatsuba_min_degree)),
    }
}

/// Top-level multiplication dispatcher used by the `Mul` operator and
/// the inherent [`FieldPoly::mul`] method. Handles zero-polynomial
/// short-circuits, resolves the active profile once, and delegates to
/// [`mul_dispatch`].
fn mul_impl<F: FiniteField>(lhs: &[F], rhs: &[F]) -> FieldPoly<F> {
    if lhs.is_empty() || rhs.is_empty() {
        return FieldPoly { coeffs: Vec::new() };
    }

    mul_dispatch(
        lhs,
        rhs,
        tuning::active().polynomial().karatsuba_min_degree(),
    )
}

impl<F: FiniteField> Mul<FieldPoly<F>> for FieldPoly<F> {
    type Output = FieldPoly<F>;

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
// NTT-based multiplication (TwoAdicField-specialised)
// ---------------------------------------------------------------------

/// Conservative default for `polynomial.karatsuba_max_out_len()` in the active
/// [`crate::tuning::CoreTuning`].
///
/// When the *output* length `lhs.len() + rhs.len() - 1` strictly exceeds the
/// active profile value, [`mul_fast`] routes through [`FieldPoly::mul_ntt`];
/// at or below it, through the schoolbook / Karatsuba dispatch.
///
/// The committed
/// `dev/benchmarks/tuning_profiles/2026-08-19-procedure-verification.md`
/// §Falsification record reports `mul_fast` at 3,793 ns for `out_len` 127 on
/// the `FieldPoly::mul` arm and 12,057 ns for `out_len` 129 on the NTT arm.
/// [`crate::tuning::CoreTuning::CONSERVATIVE`] consumes this constant.
pub const NTT_THRESHOLD: usize = 128;

/// The selected arm of the [`mul_fast`] Karatsuba/NTT dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MulFastRoute {
    /// Use the schoolbook/Karatsuba multiplication dispatcher.
    Karatsuba,
    /// Use NTT multiplication.
    Ntt,
}

/// Reports the arm selected by [`mul_fast`] for a product output length.
///
/// The comparison uses the active `polynomial.karatsuba_max_out_len()`
/// profile value.
#[must_use]
pub fn mul_fast_route(out_len: usize) -> MulFastRoute {
    mul_fast_route_resolved(
        tuning::active().polynomial().karatsuba_max_out_len(),
        out_len,
    )
}

/// Reports the [`mul_fast`] arm for a product output length against an
/// already-resolved `karatsuba_max_out_len`.
fn mul_fast_route_resolved(karatsuba_max_out_len: usize, out_len: usize) -> MulFastRoute {
    if out_len <= karatsuba_max_out_len {
        MulFastRoute::Karatsuba
    } else {
        MulFastRoute::Ntt
    }
}

impl<F: TwoAdicField> FieldPoly<F> {
    /// Multiplies two polynomials over a [`TwoAdicField`] via a radix-2
    /// NTT convolution.
    ///
    /// Both operands are padded to the next power of two
    /// `N ≥ self.len() + other.len() - 1`, transformed, multiplied
    /// elementwise, transformed back and scaled by `N^{-1}`. The result equals
    /// [`FieldPoly::mul`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // (x + 2)(x + 3) = x^2 + 5x + 6 over Fp<65537>
    /// let a = FieldPoly::new(vec![Fp::<65537>::new(2), Fp::<65537>::new(1)]);
    /// let b = FieldPoly::new(vec![Fp::<65537>::new(3), Fp::<65537>::new(1)]);
    /// let c = a.mul_ntt(&b);
    /// assert_eq!(c.coeff(0), Fp::<65537>::new(6));
    /// assert_eq!(c.coeff(1), Fp::<65537>::new(5));
    /// assert_eq!(c.coeff(2), Fp::<65537>::new(1));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the transform length `N` exceeds `2^F::TWO_ADICITY`.
    ///
    /// # Complexity
    ///
    /// `O(N log N)` field multiplications and additions, where
    /// `N = next_power_of_two(self.len() + other.len() - 1)`.
    pub fn mul_ntt(&self, other: &Self) -> Self {
        use crate::field::ntt::ntt_inplace;

        if self.is_zero() || other.is_zero() {
            return FieldPoly { coeffs: Vec::new() };
        }

        let out_len = self.coeffs.len() + other.coeffs.len() - 1;
        let n = out_len.next_power_of_two();

        let sample = &self.coeffs[0];
        let zero = sample.zero_like();

        let mut a: Vec<F> = self.coeffs.clone();
        a.resize(n, zero.clone());
        let mut b: Vec<F> = other.coeffs.clone();
        b.resize(n, zero);

        ntt_inplace(&mut a, false);
        ntt_inplace(&mut b, false);

        for (x, y) in a.iter_mut().zip(b.iter()) {
            *x = x.clone() * y.clone();
        }

        ntt_inplace(&mut a, true);

        // Scale by n^{-1}, with `n` built by repeated addition of `one`.
        let one = sample.one_like();
        let mut n_field = sample.zero_like();
        for _ in 0..n {
            n_field += one.clone();
        }
        let n_inv = n_field
            .inv()
            .expect("transform length n is always non-zero in a TwoAdic field");
        for x in a.iter_mut() {
            *x = x.clone() * n_inv.clone();
        }

        a.truncate(out_len);
        FieldPoly::new(a)
    }
}

/// Multiplies two polynomials over a [`TwoAdicField`] with dispatch
/// between Karatsuba / schoolbook (the same dispatcher [`FieldPoly::mul`]
/// uses) and NTT (via [`FieldPoly::mul_ntt`]).
///
/// The `Mul` operator cannot be specialised for [`TwoAdicField`] without the
/// nightly `specialization` feature, so NTT dispatch is this separate free
/// function. An output length at or below the active
/// `polynomial.karatsuba_max_out_len()` profile value uses the dispatcher of
/// [`FieldPoly::mul`]; a longer one uses [`FieldPoly::mul_ntt`].
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly::mul_fast;
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// // Small operands fall through to Karatsuba / schoolbook.
/// let a = FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(2)]);
/// let b = FieldPoly::new(vec![Fp::<65537>::new(3), Fp::<65537>::new(4)]);
/// let c = mul_fast(&a, &b);
/// assert_eq!(c, a.mul(&b));
/// ```
///
/// # Panics
///
/// Panics if the NTT arm is selected and the transform length exceeds
/// `2^F::TWO_ADICITY`.
///
/// # Complexity
///
/// `O(n · m)` field multiplications at or below the active profile value
/// and `O(N log N)` above, where `N = next_power_of_two(n + m - 1)`.
pub fn mul_fast<F: TwoAdicField>(a: &FieldPoly<F>, b: &FieldPoly<F>) -> FieldPoly<F> {
    if a.is_zero() || b.is_zero() {
        return FieldPoly { coeffs: Vec::new() };
    }
    let tuning = tuning::active();
    let polynomial = tuning.polynomial();
    let out_len = a.coeffs.len() + b.coeffs.len() - 1;
    match mul_fast_route_resolved(polynomial.karatsuba_max_out_len(), out_len) {
        MulFastRoute::Karatsuba => {
            mul_dispatch(&a.coeffs, &b.coeffs, polynomial.karatsuba_min_degree())
        }
        MulFastRoute::Ntt => a.mul_ntt(b),
    }
}

// ---------------------------------------------------------------------
// Newton-iteration fast division (TwoAdicField-specialised)
// ---------------------------------------------------------------------

/// Conservative default for `polynomial.div_rem_fast_min_len()` in the active
/// [`crate::tuning::CoreTuning`] between schoolbook
/// [`FieldPoly::div_rem`] and Newton-iteration [`FieldPoly::div_rem_fast`] on
/// a [`TwoAdicField`].
///
/// When either operand has fewer coefficients than the active profile value,
/// [`FieldPoly::div_rem_auto`] uses the schoolbook implementation; otherwise
/// the Newton-iteration path.
pub const DIV_REM_THRESHOLD: usize = 2048;

/// The selected arm of the [`FieldPoly::div_rem_auto`] dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DivRemAutoRoute {
    /// Use schoolbook long division.
    Schoolbook,
    /// Use Newton-iteration fast division.
    Fast,
}

/// Reports the arm selected by [`FieldPoly::div_rem_auto`] for two operand
/// coefficient counts.
///
/// The comparison uses the active `polynomial.div_rem_fast_min_len()` profile
/// value and applies to both the dividend and divisor.
#[must_use]
pub fn div_rem_auto_route(dividend_len: usize, divisor_len: usize) -> DivRemAutoRoute {
    let div_rem_fast_min_len = tuning::active().polynomial().div_rem_fast_min_len();
    if dividend_len < div_rem_fast_min_len || divisor_len < div_rem_fast_min_len {
        DivRemAutoRoute::Schoolbook
    } else {
        DivRemAutoRoute::Fast
    }
}

impl<F: TwoAdicField> FieldPoly<F> {
    /// Formal-power-series inverse `g` of `self` modulo `x^k`, so that
    /// `self · g ≡ 1 (mod x^k)`.
    ///
    /// Uses Newton's iteration on the reciprocal: starting from
    /// `g_0 = [self[0]^{-1}]` (correct modulo `x`), each step doubles the
    /// precision via
    ///
    /// ```text
    /// g_{i+1} = g_i · (2 − self · g_i)   mod x^{2·prec_i}
    /// ```
    ///
    /// so after `⌈log₂ k⌉` steps the precision reaches `k`. The products are
    /// truncated to the current precision and computed by [`mul_fast`].
    /// `k == 0` returns the zero polynomial.
    ///
    /// # Panics
    ///
    /// Panics if `self.coeffs[0]` is zero — the constant term must be a
    /// unit for the formal power series inverse to exist. Also panics if
    /// `self` is the zero polynomial.
    ///
    /// # Examples
    ///
    /// `(1 + x)` has inverse `1 − x + x² − x³ + … + (−x)^{k−1}` as a
    /// formal power series. Over `Fp<65537>`, the negation of `1` is
    /// `65_536`, so the coefficients alternate between `1` and `65_536`:
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // f(x) = 1 + x
    /// let f = FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(1)]);
    /// let g = f.invert_series(8);
    /// // g(x) = 1 − x + x² − x³ + x⁴ − x⁵ + x⁶ − x⁷
    /// let minus_one = -Fp::<65537>::new(1);
    /// let one = Fp::<65537>::new(1);
    /// let expected = FieldPoly::new(vec![
    ///     one,       minus_one, one,       minus_one,
    ///     one,       minus_one, one,       minus_one,
    /// ]);
    /// assert_eq!(g, expected);
    ///
    /// // Verify f · g ≡ 1 (mod x^8): the low 8 coefficients of the product
    /// // are [1, 0, 0, 0, 0, 0, 0, 0].
    /// let prod = f.mul(&g);
    /// assert_eq!(prod.coeff(0), Fp::<65537>::new(1));
    /// for i in 1..8 {
    ///     assert_eq!(prod.coeff(i), Fp::<65537>::new(0));
    /// }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(M(k))` field operations, where `M(k) = O(k log k)` is the cost
    /// of an `k`-point NTT-based multiplication via [`mul_fast`]. The
    /// Newton iteration's total work is a geometric sum dominated by the
    /// final doubling step at precision `≈ k`.
    pub fn invert_series(&self, k: usize) -> FieldPoly<F> {
        assert!(
            !self.is_zero(),
            "FieldPoly::invert_series: cannot invert the zero polynomial"
        );
        let c0 = &self.coeffs[0];
        assert!(
            !c0.is_zero(),
            "FieldPoly::invert_series: constant term must be non-zero"
        );

        if k == 0 {
            return FieldPoly { coeffs: Vec::new() };
        }

        let sample = c0;
        let one = sample.one_like();
        let two = one.clone() + one.clone();

        // g_0 = [1 / self[0]] — correct modulo x (precision 1).
        let c0_inv = c0
            .inv()
            .expect("constant term is non-zero; inverse exists in a field");
        let mut g = FieldPoly {
            coeffs: vec![c0_inv],
        };
        let mut prec: usize = 1;

        while prec < k {
            let next_prec = (prec * 2).min(k);

            let f_trunc = truncate_to_len(self, next_prec);

            // h = self · g   mod x^{next_prec}
            let fg = mul_fast(&f_trunc, &g);
            let h = truncate_to_len(&fg, next_prec);

            // r = 2 − h   mod x^{next_prec}
            let mut r_coeffs: Vec<F> = (0..next_prec).map(|_| sample.zero_like()).collect();
            r_coeffs[0] = two.clone();
            for (i, hc) in h.coeffs.iter().enumerate() {
                let cur = r_coeffs[i].clone();
                r_coeffs[i] = cur - hc.clone();
            }
            let r = FieldPoly::new(r_coeffs);

            // g ← g · r   mod x^{next_prec}
            let gr = mul_fast(&g, &r);
            g = truncate_to_len(&gr, next_prec);

            prec = next_prec;
        }

        truncate_to_len(&g, k)
    }

    /// `O(M(n))` polynomial division via Newton iteration on the reversed
    /// divisor's formal-power-series inverse.
    ///
    /// Returns the same `(quotient, remainder)` pair as
    /// [`FieldPoly::div_rem`].
    ///
    /// # Algorithm
    ///
    /// Let `n = deg(self)`, `m = deg(divisor)`, and `k = n − m`. Define
    /// the *reversed* polynomials
    /// `rev(p)(x) = x^{deg p} · p(1/x)`, i.e. just the coefficient vector
    /// reversed. Reversing the Euclidean identity modulo `x^{k+1}` yields
    ///
    /// ```text
    /// rev(self) ≡ rev(quotient) · rev(divisor)   (mod x^{k+1})
    /// ```
    ///
    /// so `rev(quotient) ≡ rev(self) · rev(divisor)^{-1} (mod x^{k+1})`
    /// where the inverse is a formal power series built by
    /// [`invert_series`](Self::invert_series). One final
    /// `remainder = self − divisor · quotient` recovers the remainder.
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
    /// // (x⁴ + 2x² + 1) / (x² + 1) = x² + 1 over Fp<65537>.
    /// let dividend = FieldPoly::new(vec![
    ///     Fp::<65537>::new(1),
    ///     Fp::<65537>::new(0),
    ///     Fp::<65537>::new(2),
    ///     Fp::<65537>::new(0),
    ///     Fp::<65537>::new(1),
    /// ]);
    /// let divisor = FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(0), Fp::<65537>::new(1)]);
    /// let (q, r) = dividend.div_rem_fast(&divisor);
    /// // Quotient: x² + 1. Remainder: 0.
    /// assert_eq!(q, FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(0), Fp::<65537>::new(1)]));
    /// assert!(r.is_zero());
    /// // Euclidean identity holds.
    /// assert_eq!(&(&q * &divisor) + &r, dividend);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(M(n))` field operations, where `M(n) = O(n log n)` is the cost of
    /// [`mul_fast`] at NTT sizes.
    pub fn div_rem_fast(&self, divisor: &FieldPoly<F>) -> (FieldPoly<F>, FieldPoly<F>) {
        assert!(
            !divisor.is_zero(),
            "FieldPoly::div_rem_fast: division by zero polynomial"
        );

        // Zero dividend: `divisor` supplies the field sample.
        let Some(dividend_deg) = self.degree() else {
            let sample = &divisor.coeffs[0];
            return (FieldPoly::zero_like(sample), FieldPoly::zero_like(sample));
        };
        let divisor_deg = divisor.degree().unwrap();

        if dividend_deg < divisor_deg {
            let sample = &self.coeffs[0];
            return (FieldPoly::zero_like(sample), self.clone());
        }

        // Constant divisor: quotient = self · divisor[0]^{-1}, remainder = 0.
        if divisor_deg == 0 {
            let sample = &self.coeffs[0];
            let inv = divisor.coeffs[0]
                .inv()
                .expect("divisor is non-zero constant; inverse exists in a field");
            let q = self.mul_scalar(&inv);
            return (q, FieldPoly::zero_like(sample));
        }

        let n = dividend_deg;
        let m = divisor_deg;
        let k = n - m;

        // The reverse of a normalised polynomial has a non-zero constant term,
        // so the series inverse exists.
        let rev_divisor = reverse_poly(divisor);
        let rev_inv = rev_divisor.invert_series(k + 1);

        let rev_dividend = reverse_poly(self);
        let rev_dividend_trunc = truncate_to_len(&rev_dividend, k + 1);

        // rev_quotient = (rev_dividend_trunc · rev_inv)  mod x^{k+1}.
        let prod = mul_fast(&rev_dividend_trunc, &rev_inv);
        let rev_quotient = truncate_to_len(&prod, k + 1);

        // Pad to length k + 1 before reversing so the quotient coefficients
        // land at their ascending-degree positions.
        let quotient = reverse_poly_padded(&rev_quotient, k + 1);

        // remainder = self − divisor · quotient.
        let dq = mul_fast(divisor, &quotient);
        let remainder = self - &dq;

        (quotient, remainder)
    }

    /// Dispatches between the schoolbook [`FieldPoly::div_rem`] and the
    /// Newton-iteration [`FieldPoly::div_rem_fast`] based on the active
    /// `polynomial.div_rem_fast_min_len()` profile value.
    ///
    /// The schoolbook path is selected when either operand has fewer
    /// coefficients than that value. Both arms return the same pair.
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
    /// let dividend = FieldPoly::new(vec![
    ///     Fp::<65537>::new(1),
    ///     Fp::<65537>::new(1),
    ///     Fp::<65537>::new(1),
    /// ]);
    /// let divisor = FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(1)]);
    /// let (q, r) = dividend.div_rem_auto(&divisor);
    /// // Same result as the schoolbook path.
    /// let (qs, rs) = dividend.div_rem(&divisor);
    /// assert_eq!(q, qs);
    /// assert_eq!(r, rs);
    /// ```
    ///
    /// # Complexity
    ///
    /// Matches the dispatched arm: `O((n − m) · m)` in the schoolbook
    /// regime and `O(M(n))` in the fast-division regime.
    pub fn div_rem_auto(&self, divisor: &FieldPoly<F>) -> (FieldPoly<F>, FieldPoly<F>) {
        match div_rem_auto_route(self.coeffs.len(), divisor.coeffs.len()) {
            DivRemAutoRoute::Schoolbook => self.div_rem(divisor),
            DivRemAutoRoute::Fast => self.div_rem_fast(divisor),
        }
    }

    /// [`TwoAdicField`] form of [`FieldPoly::batch_evaluate`]: the same values
    /// and the same `polynomial.subproduct_min_len()` gate, with the
    /// subproduct-tree reductions through [`FieldPoly::div_rem_auto`]
    /// ([`batch_evaluate_subproduct_auto`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// let p = FieldPoly::new(vec![Fp::<65537>::new(1), Fp::<65537>::new(2), Fp::<65537>::new(3)]);
    /// let xs = vec![Fp::<65537>::new(0), Fp::<65537>::new(1), Fp::<65537>::new(4)];
    /// let ys = p.batch_evaluate_auto(&xs);
    /// assert_eq!(ys, xs.iter().map(|x| p.eval(x)).collect::<Vec<_>>());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n · k)` on the Horner path; `O(M(n) log k + k² log k)` field
    /// operations with `M(n) = O(n log n)` on the subproduct path.
    pub fn batch_evaluate_auto(&self, points: &[F]) -> Vec<F> {
        match batch_evaluate_auto_route(self.coeffs.len(), points.len()) {
            BatchEvaluateRoute::Horner => self.eval_batch(points),
            BatchEvaluateRoute::SubproductTree => batch_evaluate_subproduct_auto(self, points),
        }
    }
}

// ---------------------------------------------------------------------
// Internal helpers for invert_series / div_rem_fast.
// ---------------------------------------------------------------------

/// Returns the polynomial formed by the first `len` coefficients of
/// `poly`, normalised.
fn truncate_to_len<F: FiniteField>(poly: &FieldPoly<F>, len: usize) -> FieldPoly<F> {
    if len == 0 {
        return FieldPoly { coeffs: Vec::new() };
    }
    let take = poly.coeffs.len().min(len);
    let coeffs: Vec<F> = poly.coeffs[..take].to_vec();
    FieldPoly::new(coeffs)
}

/// Returns the coefficient-reversed polynomial `x^d · poly(1/x)` for degree
/// `d`. Its constant term is the leading coefficient of `poly`, hence
/// non-zero for non-zero `poly`.
fn reverse_poly<F: FiniteField>(poly: &FieldPoly<F>) -> FieldPoly<F> {
    if poly.is_zero() {
        return FieldPoly { coeffs: Vec::new() };
    }
    let mut coeffs = poly.coeffs.clone();
    coeffs.reverse();
    FieldPoly::new(coeffs)
}

/// Reverses `poly` after zero-padding its coefficient vector to `pad_len`,
/// so a quotient whose reversal lost trailing zeros keeps its coefficients at
/// their ascending-degree positions.
fn reverse_poly_padded<F: FiniteField>(poly: &FieldPoly<F>, pad_len: usize) -> FieldPoly<F> {
    if pad_len == 0 {
        return FieldPoly { coeffs: Vec::new() };
    }
    if poly.is_zero() {
        return FieldPoly { coeffs: Vec::new() };
    }
    let sample = &poly.coeffs[0];
    let mut coeffs: Vec<F> = poly.coeffs.clone();
    while coeffs.len() < pad_len {
        coeffs.push(sample.zero_like());
    }
    coeffs.reverse();
    FieldPoly::new(coeffs)
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
    // Proptests (tight budgets per `@/inv/test-tier-budgets`)
    // -----------------------------------------------------------------

    /// Strategy: a random `FieldPoly<Fp<7>>` from fewer than 5 coefficients.
    fn any_fp7_poly() -> impl Strategy<Value = FieldPoly<FP7>> {
        prop::collection::vec(0u64..7, 0..5)
            .prop_map(|xs| FieldPoly::new(xs.into_iter().map(fp7).collect::<Vec<_>>()))
    }

    /// Strategy: *non-zero* polynomial over `Fp<7>` with a non-zero leading
    /// coefficient.
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
        // eval on the zero polynomial returns x.zero_like() regardless of the
        // evaluation point.
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
        // Call the raw subproduct kernel directly (bypassing the
        // SUBPRODUCT_THRESHOLD gate at 4096, which the tiny FP7 inputs
        // below would never cross). Verifies the tree construction +
        // top-down reduction math on moderately-sized inputs where the
        // algorithm's odd-tail and descent branches all fire.
        let n = 20;
        let k = 24;
        let p_coeffs: Vec<FP7> = (0..=n).map(|i| fp7((i as u64 * 3 + 1) % 7)).collect();
        let p = FieldPoly::new(p_coeffs);
        let xs: Vec<FP7> = (0..k).map(|i| fp7((i as u64 * 5) % 7)).collect();

        let fast = batch_evaluate_subproduct(&p, &xs);
        let naive: Vec<FP7> = xs.iter().map(|x| p.eval(x)).collect();
        assert_eq!(fast, naive);
    }

    #[test]
    fn test_batch_evaluate_exercises_subproduct_path_gf16() {
        let field = Gf2mField::new(4, 0b10011);
        let n = 20;
        let k = 24;
        let p_coeffs: Vec<Gf2mElement> = (0..=n)
            .map(|i| field.element(((i as u64) * 7 + 1) & 0xF))
            .collect();
        let p = FieldPoly::new(p_coeffs);
        let xs: Vec<Gf2mElement> = (0..k)
            .map(|i| field.element((i as u64 * 11) & 0xF))
            .collect();

        let fast = batch_evaluate_subproduct(&p, &xs);
        let naive: Vec<Gf2mElement> = xs.iter().map(|x| p.eval(x)).collect();
        assert_eq!(fast, naive);
    }

    #[test]
    fn test_batch_evaluate_odd_sized_point_set_fp7() {
        // Odd k forces the odd-tail-carry branch in the bottom-up tree
        // build and the corresponding single-child descent during the
        // top-down reduction. Dispatches straight to the raw tree
        // kernel so the branch fires regardless of the public
        // `batch_evaluate` threshold policy.
        let n = 20;
        let k = 23; // deliberately odd
        let p_coeffs: Vec<FP7> = (0..=n).map(|i| fp7((i as u64 * 2 + 1) % 7)).collect();
        let p = FieldPoly::new(p_coeffs);
        let xs: Vec<FP7> = (0..k).map(|i| fp7((i as u64 * 3) % 7)).collect();

        let fast = batch_evaluate_subproduct(&p, &xs);
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
    // batch_mul / batch_mul_with_field / batch_gcd
    // -----------------------------------------------------------------

    /// Helper: build a monic degree-1 polynomial `x + c` over Fp<7>.
    fn linear_fp7(c: u64) -> FieldPoly<FP7> {
        FieldPoly::new(vec![fp7(c), fp7(1)])
    }

    // --- batch_mul unit tests ---

    #[test]
    #[should_panic(expected = "polys cannot be empty")]
    fn test_batch_mul_empty_panics() {
        FieldPoly::<FP7>::batch_mul(&[]);
    }

    #[test]
    fn test_batch_mul_single() {
        let p = linear_fp7(3);
        assert_eq!(FieldPoly::batch_mul(std::slice::from_ref(&p)), p);
    }

    #[test]
    fn test_batch_mul_two() {
        let a = linear_fp7(1); // x + 1
        let b = linear_fp7(2); // x + 2
        let expected = &a * &b;
        let got = FieldPoly::batch_mul(&[a, b]);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_batch_mul_three() {
        // Odd tail exercises the carry-up branch.
        let a = linear_fp7(1);
        let b = linear_fp7(2);
        let c = linear_fp7(3);
        let expected = &(&a * &b) * &c;
        let got = FieldPoly::batch_mul(&[a, b, c]);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_batch_mul_four() {
        let polys: Vec<FieldPoly<FP7>> = (1..=4).map(linear_fp7).collect();
        let expected = FieldPoly::product(&polys);
        let got = FieldPoly::batch_mul(&polys);
        assert_eq!(got, expected);
    }

    // --- batch_mul_with_field unit tests ---

    #[test]
    fn test_batch_mul_with_field_empty_returns_one() {
        let sample = fp7(0);
        let prod = FieldPoly::batch_mul_with_field(&sample, &[]);
        assert_eq!(prod, FieldPoly::one_like(&sample));
    }

    #[test]
    fn test_batch_mul_with_field_single() {
        let p = linear_fp7(5);
        let sample = fp7(0);
        let got = FieldPoly::batch_mul_with_field(&sample, std::slice::from_ref(&p));
        assert_eq!(got, p);
    }

    #[test]
    fn test_batch_mul_with_field_two() {
        let a = linear_fp7(1);
        let b = linear_fp7(6);
        let sample = fp7(0);
        let expected = &a * &b;
        let got = FieldPoly::batch_mul_with_field(&sample, &[a, b]);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_batch_mul_with_field_three() {
        // Ensure the odd-tail branch is reached via with_field too.
        let polys: Vec<FieldPoly<FP7>> = (1..=3).map(linear_fp7).collect();
        let expected = FieldPoly::product(&polys);
        let got = FieldPoly::batch_mul_with_field(&fp7(0), &polys);
        assert_eq!(got, expected);
    }

    // --- batch_gcd unit tests ---

    #[test]
    #[should_panic(expected = "polys cannot be empty")]
    fn test_batch_gcd_empty_panics() {
        FieldPoly::<FP7>::batch_gcd(&[]);
    }

    #[test]
    fn test_batch_gcd_single_monic() {
        // gcd of a single element is its monic form.
        let p = FieldPoly::new(vec![fp7(3), fp7(2)]); // 2x + 3 — lead = 2
        let g = FieldPoly::batch_gcd(std::slice::from_ref(&p));
        // Leading coeff must be 1.
        assert_eq!(g.leading_coeff(), Some(&fp7(1)));
        // And the result must divide p.
        let (_, r) = p.div_rem(&g);
        assert!(r.is_zero());
    }

    #[test]
    fn test_batch_gcd_two_shared_factor() {
        let d = linear_fp7(1); // x + 1
        let a = &d * &linear_fp7(2); // (x+1)(x+2)
        let b = &d * &linear_fp7(3); // (x+1)(x+3)
        let g = FieldPoly::batch_gcd(&[a, b]);
        // d is a common factor, so g must be divisible by d.
        let (_, r) = g.div_rem(&d);
        assert!(r.is_zero(), "batch_gcd result should be divisible by d");
    }

    #[test]
    fn test_batch_gcd_three_shared_factor() {
        let d = linear_fp7(4); // x + 4
        let a = &d * &linear_fp7(1);
        let b = &d * &linear_fp7(2);
        let c = &d * &linear_fp7(3);
        let g = FieldPoly::batch_gcd(&[a, b, c]);
        // d is a common factor, so g must be divisible by d.
        let (_, r) = g.div_rem(&d);
        assert!(r.is_zero(), "batch_gcd result should be divisible by d");
    }

    // --- Gf2mElement unit tests ---

    #[test]
    fn test_batch_mul_gf16_two() {
        let field = Gf2mField::new(4, 0b10011);
        let a = FieldPoly::new(vec![field.element(5), field.element(1)]);
        let b = FieldPoly::new(vec![field.element(3), field.element(1)]);
        let expected = &a * &b;
        let got = FieldPoly::batch_mul(&[a, b]);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_batch_mul_with_field_gf16_empty() {
        let field = Gf2mField::new(4, 0b10011);
        let sample = field.zero();
        let prod = FieldPoly::batch_mul_with_field(&sample, &[]);
        assert_eq!(prod, FieldPoly::one_like(&sample));
    }

    // --- Proptests ---

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_batch_mul_agrees_with_fold_fp7(
            raw in prop::collection::vec(
                prop::collection::vec(0u64..7, 1..5),
                0..=8usize,
            ),
        ) {
            let polys: Vec<FieldPoly<FP7>> = raw.into_iter()
                .map(|cs| FieldPoly::new(cs.into_iter().map(fp7).collect::<Vec<_>>()))
                .collect();

            let sample = fp7(0);
            let one = FieldPoly::one_like(&sample);

            let expected = polys.iter().fold(one, |a, b| &a * b);
            let got = FieldPoly::batch_mul_with_field(&sample, &polys);
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_batch_mul_agrees_with_fold_gf16(
            raw in prop::collection::vec(
                prop::collection::vec(0u64..16, 1..5),
                0..=8usize,
            ),
        ) {
            let field = gf16_field();
            let sample = field.zero();
            let polys: Vec<FieldPoly<Gf2mElement>> = raw.into_iter()
                .map(|cs| FieldPoly::new(
                    cs.into_iter().map(|v| field.element(v)).collect::<Vec<_>>(),
                ))
                .collect();

            let one = FieldPoly::one_like(&sample);
            let expected = polys.iter().fold(one, |a, b| &a * b);
            let got = FieldPoly::batch_mul_with_field(&sample, &polys);
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_batch_gcd_divides_common_factor_fp7(
            // a, b, c: small non-zero polynomials used as coprime cofactors
            a_cs in prop::collection::vec(1u64..7, 1..4),
            b_cs in prop::collection::vec(1u64..7, 1..4),
            c_cs in prop::collection::vec(1u64..7, 1..4),
            // d: shared factor
            d_cs in prop::collection::vec(1u64..7, 1..4),
        ) {
            let a = FieldPoly::new(a_cs.into_iter().map(fp7).collect::<Vec<_>>());
            let b = FieldPoly::new(b_cs.into_iter().map(fp7).collect::<Vec<_>>());
            let c = FieldPoly::new(c_cs.into_iter().map(fp7).collect::<Vec<_>>());
            let d = FieldPoly::new(d_cs.into_iter().map(fp7).collect::<Vec<_>>());

            prop_assume!(!d.is_zero());

            let ad = &a * &d;
            let bd = &b * &d;
            let cd = &c * &d;

            let g = FieldPoly::batch_gcd(&[ad, bd, cd]);
            // Mathematical invariant: d | each a*d, b*d, c*d
            //   ⇒ d | gcd(a*d, b*d, c*d).
            // i.e. g is a multiple of d; we check g mod d = 0.
            prop_assume!(!d.is_zero());
            let (_, r) = g.div_rem(&d);
            prop_assert!(
                r.is_zero(),
                "d should divide batch_gcd([a*d, b*d, c*d]); got remainder {:?}",
                r
            );
        }
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
        // With shared factor g and coprime cofactors c1, c2, the polynomials
        // g·c1 and g·c2 have gcd equal to the monic form of g. Fixed
        // cofactors avoid random cofactors sharing a factor over Fp<7>.
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

    #[test]
    fn test_lcm_coprime_polynomials() {
        // Coprime linear factors: lcm(x - 1, x - 2) = (x - 1)(x - 2).
        let xm1 = FieldPoly::new(vec![-fp7(1), fp7(1)]);
        let xm2 = FieldPoly::new(vec![-fp7(2), fp7(1)]);
        let l = FieldPoly::lcm(&xm1, &xm2);
        assert_eq!(l, &xm1 * &xm2);
    }

    #[test]
    fn test_lcm_shared_linear_factor() {
        // p1 = (x - 1)(x - 2), p2 = (x - 1)(x - 3); the shared factor
        // (x - 1) is not duplicated in the LCM.
        let xm1 = FieldPoly::new(vec![-fp7(1), fp7(1)]);
        let xm2 = FieldPoly::new(vec![-fp7(2), fp7(1)]);
        let xm3 = FieldPoly::new(vec![-fp7(3), fp7(1)]);
        let p1 = &xm1 * &xm2;
        let p2 = &xm1 * &xm3;
        let l = FieldPoly::lcm(&p1, &p2);
        assert_eq!(l, &p1 * &xm3);
    }

    #[test]
    fn test_lcm_zero_arg_is_zero() {
        let p = FieldPoly::new(vec![fp7(1), fp7(2)]);
        let z: FieldPoly<FP7> = FieldPoly::zero_like(&fp7(0));
        assert!(FieldPoly::lcm(&p, &z).is_zero());
        assert!(FieldPoly::lcm(&z, &p).is_zero());
        assert!(FieldPoly::lcm(&z, &z).is_zero());
    }

    #[test]
    #[should_panic(expected = "lcm of two zero polynomials")]
    fn test_lcm_both_zero_runtime_context_panics() {
        // Gf2mElement is a runtime-context field (its zero carries no
        // static witness); with both operands zero and no non-zero
        // coefficient anywhere to borrow a witness from, there is no
        // `Gf2mElement` value to build the zero result from.
        let field = Gf2mField::new(4, 0b10011);
        let sample = field.element(1);
        let z: FieldPoly<Gf2mElement> = FieldPoly::zero_like(&sample);
        let _ = FieldPoly::lcm(&z, &z);
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
        // Gf2mElement proptests: div_rem identity, gcd commutativity, gcd
        // divides both inputs, over GF(2^m).
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
            // The public `batch_evaluate` routes these sizes to the Horner
            // path, so the subproduct helper is called directly.
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

    // -----------------------------------------------------------------
    // invert_series / div_rem_fast / div_rem_auto (TwoAdicField path)
    // -----------------------------------------------------------------

    type FP65537 = Fp<65537>;

    fn fp65537(n: u64) -> FP65537 {
        FP65537::new(n)
    }

    #[test]
    fn test_invert_series_of_one_plus_x_over_fp65537() {
        // (1 + x) has inverse 1 − x + x² − … + (−x)^7 modulo x^8.
        let f = FieldPoly::new(vec![fp65537(1), fp65537(1)]);
        let g = f.invert_series(8);
        assert_eq!(g.len(), 8);
        let minus_one = -fp65537(1);
        for i in 0..8 {
            let expected = if i % 2 == 0 { fp65537(1) } else { minus_one };
            assert_eq!(g.coeff(i), expected, "coeff {i} mismatch");
        }

        // f · g ≡ 1 (mod x^8).
        let prod = f.mul(&g);
        assert_eq!(prod.coeff(0), fp65537(1));
        for i in 1..8 {
            assert_eq!(prod.coeff(i), fp65537(0), "product coeff {i} not zero");
        }
    }

    #[test]
    fn test_invert_series_identity() {
        // The constant polynomial `c` has inverse `c^{-1}` modulo any x^k.
        let c = FieldPoly::constant(fp65537(7));
        let inv = c.invert_series(5);
        assert_eq!(inv.len(), 1);
        assert_eq!(inv.coeff(0) * fp65537(7), fp65537(1));
    }

    #[test]
    fn test_invert_series_k_zero_is_zero() {
        let f = FieldPoly::new(vec![fp65537(1), fp65537(2), fp65537(3)]);
        let g = f.invert_series(0);
        assert!(g.is_zero());
    }

    #[test]
    #[should_panic(expected = "constant term must be non-zero")]
    fn test_invert_series_zero_constant_term_panics() {
        // x has a zero constant term — not a unit in the power series ring.
        let f = FieldPoly::new(vec![fp65537(0), fp65537(1)]);
        let _ = f.invert_series(4);
    }

    #[test]
    fn test_invert_series_roundtrip_truncated() {
        // Proptest-lite: check that f · g ≡ 1 (mod x^k) for a few random
        // polynomials over Fp<65537>.
        let modulus: u64 = 65537;
        for seed in 1u64..=8 {
            let n = 5 + (seed as usize);
            let mut coeffs: Vec<FP65537> = (0..n)
                .map(|i| {
                    let v = ((seed.wrapping_mul(0x9E3779B1) ^ (i as u64).wrapping_mul(0x165667B1))
                        % (modulus - 1))
                        + 1;
                    fp65537(v)
                })
                .collect();
            // Guarantee a non-zero constant term.
            if coeffs[0].is_zero() {
                coeffs[0] = fp65537(1);
            }
            let f = FieldPoly::new(coeffs);
            let k = n + 3;
            let g = f.invert_series(k);
            let prod = f.mul(&g);
            assert_eq!(prod.coeff(0), fp65537(1));
            for i in 1..k {
                assert_eq!(
                    prod.coeff(i),
                    fp65537(0),
                    "seed={seed}, i={i} non-zero in f*g mod x^k"
                );
            }
        }
    }

    // --- div_rem_fast unit tests: edge cases. ---

    #[test]
    fn test_div_rem_fast_zero_dividend() {
        let zero: FieldPoly<FP65537> = FieldPoly::zero_like(&fp65537(0));
        let divisor = FieldPoly::new(vec![fp65537(1), fp65537(1)]);
        let (q, r) = zero.div_rem_fast(&divisor);
        assert!(q.is_zero());
        assert!(r.is_zero());
    }

    #[test]
    fn test_div_rem_fast_dividend_smaller_than_divisor() {
        // deg(dividend) < deg(divisor): quotient = 0, remainder = dividend.
        let dividend = FieldPoly::new(vec![fp65537(3), fp65537(2)]);
        let divisor = FieldPoly::new(vec![fp65537(1), fp65537(0), fp65537(1)]); // 1 + x^2
        let (q, r) = dividend.div_rem_fast(&divisor);
        assert!(q.is_zero());
        assert_eq!(r, dividend);
    }

    #[test]
    fn test_div_rem_fast_constant_divisor() {
        // Non-zero constant divisor: quotient = dividend · c^{-1},
        // remainder = 0.
        let dividend = FieldPoly::new(vec![fp65537(4), fp65537(6), fp65537(8)]);
        let divisor = FieldPoly::constant(fp65537(2));
        let (q, r) = dividend.div_rem_fast(&divisor);
        // 2^{-1} mod 65537 is 32769 (since 2·32769 = 65538 ≡ 1).
        let inv2 = fp65537(2).inv().unwrap();
        assert_eq!(q, dividend.mul_scalar(&inv2));
        assert!(r.is_zero());
    }

    #[test]
    fn test_div_rem_fast_dividend_equals_divisor() {
        // self / self == (1, 0).
        let divisor = FieldPoly::new(vec![fp65537(3), fp65537(2), fp65537(5)]);
        let (q, r) = divisor.div_rem_fast(&divisor);
        assert_eq!(q, FieldPoly::one_like(&fp65537(0)));
        assert!(r.is_zero());
    }

    #[test]
    fn test_div_rem_fast_matches_schoolbook_small() {
        // A small deterministic case: (x^4 + 2x^2 + 1) / (x^2 + 1) =
        // x^2 + 1 with remainder 0.
        let dividend = FieldPoly::new(vec![
            fp65537(1),
            fp65537(0),
            fp65537(2),
            fp65537(0),
            fp65537(1),
        ]);
        let divisor = FieldPoly::new(vec![fp65537(1), fp65537(0), fp65537(1)]);
        let (q_fast, r_fast) = dividend.div_rem_fast(&divisor);
        let (q_school, r_school) = dividend.div_rem(&divisor);
        assert_eq!(q_fast, q_school);
        assert_eq!(r_fast, r_school);
    }

    #[test]
    fn test_div_rem_auto_small_uses_schoolbook() {
        // For small operands, div_rem_auto must agree with div_rem.
        let dividend = FieldPoly::new(vec![fp65537(1), fp65537(1), fp65537(1)]);
        let divisor = FieldPoly::new(vec![fp65537(1), fp65537(1)]);
        let (q, r) = dividend.div_rem_auto(&divisor);
        let (qs, rs) = dividend.div_rem(&divisor);
        assert_eq!(q, qs);
        assert_eq!(r, rs);
    }

    #[test]
    fn test_div_rem_auto_large_agrees_with_fast() {
        // Construct operands above DIV_REM_THRESHOLD so the fast path is
        // dispatched, and verify agreement with div_rem_fast itself (which
        // in turn is cross-checked by the proptest below against
        // schoolbook).
        let n = DIV_REM_THRESHOLD + 16;
        let m = DIV_REM_THRESHOLD;
        let mut a_coeffs: Vec<FP65537> = (0..n)
            .map(|i| fp65537(((i as u64).wrapping_mul(0x9E3779B1) % 65536) + 1))
            .collect();
        let mut b_coeffs: Vec<FP65537> = (0..m)
            .map(|i| fp65537(((i as u64).wrapping_mul(0x165667B1) % 65536) + 1))
            .collect();
        // Ensure non-zero leading coefficients so `len() == degree + 1`
        // holds as expected.
        *a_coeffs.last_mut().unwrap() = fp65537(1);
        *b_coeffs.last_mut().unwrap() = fp65537(1);
        let a = FieldPoly::new(a_coeffs);
        let b = FieldPoly::new(b_coeffs);

        let (q_auto, r_auto) = a.div_rem_auto(&b);
        let (q_fast, r_fast) = a.div_rem_fast(&b);
        assert_eq!(q_auto, q_fast);
        assert_eq!(r_auto, r_fast);

        // Euclidean identity holds.
        assert_eq!(&(&q_auto * &b) + &r_auto, a);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Agreement test: `div_rem_fast` must match the
        /// schoolbook `div_rem` pair and satisfy the Euclidean identity
        /// `dividend = quotient · divisor + remainder` with
        /// `deg(remainder) < deg(divisor)`, for ≥ 500 random
        /// `(dividend, divisor)` pairs over `Fp<65537>`.
        #[test]
        fn prop_div_rem_fast_matches_schoolbook_fp65537(
            dividend_coeffs in prop::collection::vec(0u64..65537, 1..40),
            divisor_coeffs in prop::collection::vec(0u64..65537, 1..20),
            divisor_lead in 1u64..65537,
        ) {
            // Build normalised dividend (may be zero).
            let dividend = FieldPoly::new(
                dividend_coeffs.into_iter().map(fp65537).collect::<Vec<_>>(),
            );
            // Build a non-zero divisor by forcing a non-zero leading
            // coefficient appended to the random middle section.
            let mut dc: Vec<FP65537> = divisor_coeffs.into_iter().map(fp65537).collect();
            dc.push(fp65537(divisor_lead));
            let divisor = FieldPoly::new(dc);

            let (q_fast, r_fast) = dividend.div_rem_fast(&divisor);
            let (q_school, r_school) = dividend.div_rem(&divisor);

            prop_assert_eq!(&q_fast, &q_school);
            prop_assert_eq!(&r_fast, &r_school);

            // Euclidean identity + degree bound.
            let reconstructed = &(&q_fast * &divisor) + &r_fast;
            prop_assert_eq!(reconstructed, dividend);
            let db = divisor.degree().unwrap();
            match r_fast.degree() {
                None => {}
                Some(d) => prop_assert!(d < db),
            }
        }
    }

    // -----------------------------------------------------------------
    // batch_evaluate_auto + batch_evaluate_subproduct_auto agreement.
    //
    // Two families of coverage on Fp<65537>:
    //   1. A ≥ 500-case proptest on batch_evaluate_subproduct_auto at
    //      small random sizes, agreeing with per-point Horner. This
    //      exercises the subproduct-tree traversal and the
    //      div_rem_auto reduction closure directly on every random
    //      shape (odd / even k, duplicates, zero polynomial, etc.).
    //   2. A deterministic test that straddles SUBPRODUCT_THRESHOLD in
    //      both dimensions, so both branches of FieldPoly::batch_evaluate
    //      and FieldPoly::batch_evaluate_auto (naive fallback below
    //      threshold, subproduct tree above) are exercised against a
    //      naive Horner reference on Fp<65537>.
    // -----------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Agreement: `batch_evaluate_subproduct_auto` matches per-point
        /// Horner on ≥ 500 random inputs over `Fp<65537>`. Exercises
        /// the subproduct-tree traversal and the div_rem_auto reduction
        /// closure on every random shape (odd / even k, duplicates,
        /// zero polynomial, etc.) — the "below-threshold" side of the
        /// auto dispatcher.
        #[test]
        fn prop_batch_evaluate_subproduct_auto_matches_horner_fp65537(
            poly_coeffs in prop::collection::vec(0u64..65537, 1..40),
            point_vals in prop::collection::vec(0u64..65537, 1..40),
        ) {
            let poly = FieldPoly::new(poly_coeffs.into_iter().map(fp65537).collect::<Vec<_>>());
            let points: Vec<FP65537> = point_vals.into_iter().map(fp65537).collect();

            let fast = batch_evaluate_subproduct_auto(&poly, &points);
            let naive: Vec<FP65537> = points.iter().map(|x| poly.eval(x)).collect();
            prop_assert_eq!(fast, naive);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Agreement: the public [`FieldPoly::batch_evaluate`]
        /// dispatcher matches per-point Horner on ≥ 500 random
        /// inputs over `Fp<65537>`, with input sizes drawn from a
        /// set that straddles [`SUBPRODUCT_THRESHOLD`] = 4096: the
        /// pool `{16, 1024, 4096, 8192}` guarantees coverage of
        /// both the "below threshold" (naive Horner fallback) and
        /// "at/above threshold" (subproduct-tree) branches.
        #[test]
        #[ignore = "slow: 500-case proptest with 4096/8192 cells exceeds 5 s on CI hardware"]
        fn prop_batch_evaluate_dispatcher_matches_horner_straddling_threshold_fp65537(
            // Weighted size sampler: the below-threshold sizes 16 and 1024
            // carry most cases.
            n in prop_oneof![
                50 => Just(16usize),
                30 => Just(1024usize),
                10 => Just(4096usize),
                1 => Just(8192usize),
            ],
            k in prop_oneof![
                50 => Just(16usize),
                30 => Just(1024usize),
                10 => Just(4096usize),
                1 => Just(8192usize),
            ],
            poly_seed in 0u64..65537,
            point_seed in 0u64..65537,
        ) {
            // Poly of length n with a non-zero leading coefficient, so
            // degree == n - 1.
            let modulus: u64 = 65537;
            let mut coeffs: Vec<FP65537> = (0..n)
                .map(|i| {
                    let v = ((i as u64)
                        .wrapping_mul(0x9E3779B1)
                        .wrapping_add(poly_seed)
                        % (modulus - 1))
                        + 1;
                    fp65537(v)
                })
                .collect();
            *coeffs.last_mut().unwrap() = fp65537(1);
            let poly = FieldPoly::new(coeffs);

            let points: Vec<FP65537> = (0..k)
                .map(|i| {
                    fp65537(
                        ((i as u64)
                            .wrapping_mul(1_000_003)
                            .wrapping_add(point_seed)
                            % (modulus - 1))
                            + 1,
                    )
                })
                .collect();

            let via_dispatcher = poly.batch_evaluate(&points);
            let naive: Vec<FP65537> = points.iter().map(|x| poly.eval(x)).collect();
            prop_assert_eq!(via_dispatcher, naive);
        }
    }

    #[test]
    #[ignore = "slow: batch polynomial evaluation at n=4096/4200 over Fp<65537>"]
    fn test_batch_evaluate_auto_straddles_subproduct_threshold_fp65537() {
        // SUBPRODUCT_THRESHOLD = 4096 on Fp<65537>, compared against
        // `points.len()` and the polynomial's coefficient length
        // (len == degree + 1). We exercise cells on both sides of the
        // threshold in both dimensions so every branch of
        // FieldPoly::batch_evaluate / batch_evaluate_auto fires at
        // least once:
        //   - (n, k) = (64, 64)       → both lengths below → naive.
        //   - (n, k) = (4095, 4095)   → both below → naive.
        //   - (n, k) = (4096, 64)     → poly at threshold, k below →
        //                               still naive (k dimension gates).
        //   - (n, k) = (4096, 4096)   → both at threshold → subproduct.
        //   - (n, k) = (4200, 4200)   → both above → subproduct.
        // For every cell we verify agreement with a naive Horner sweep
        // (eval_batch) and cross-check that the two dispatchers and
        // the two raw subproduct helpers all produce identical output.
        let cells: &[(usize, usize)] = &[
            (64, 64),
            (4095, 4095),
            (4096, 64),
            (4096, 4096),
            (4200, 4200),
        ];

        for &(n, k) in cells {
            let modulus: u64 = 65537;
            // Build poly of length n with a guaranteed non-zero leading
            // coefficient so degree == n - 1.
            let mut p_coeffs: Vec<FP65537> = (0..n)
                .map(|i| {
                    let v = ((i as u64).wrapping_mul(0x9E3779B1) % (modulus - 1)) + 1;
                    fp65537(v)
                })
                .collect();
            *p_coeffs.last_mut().unwrap() = fp65537(1);
            let poly = FieldPoly::new(p_coeffs);

            // Build k distinct evaluation points.
            let points: Vec<FP65537> = (0..k)
                .map(|i| fp65537(((i as u64).wrapping_mul(1_000_003) % (modulus - 1)) + 1))
                .collect();

            let naive: Vec<FP65537> = points.iter().map(|x| poly.eval(x)).collect();
            let via_dispatcher = poly.batch_evaluate(&points);
            let via_dispatcher_auto = poly.batch_evaluate_auto(&points);
            let via_subproduct = batch_evaluate_subproduct(&poly, &points);
            let via_subproduct_auto = batch_evaluate_subproduct_auto(&poly, &points);

            assert_eq!(
                naive, via_dispatcher,
                "batch_evaluate disagrees with per-point Horner at n={n}, k={k}"
            );
            assert_eq!(
                naive, via_dispatcher_auto,
                "batch_evaluate_auto disagrees with per-point Horner at n={n}, k={k}"
            );
            assert_eq!(
                naive, via_subproduct,
                "batch_evaluate_subproduct disagrees with per-point Horner at n={n}, k={k}"
            );
            assert_eq!(
                naive, via_subproduct_auto,
                "batch_evaluate_subproduct_auto disagrees with per-point Horner at n={n}, k={k}"
            );
        }
    }
}
