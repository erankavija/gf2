//! Dense vector of finite field elements with arithmetic operations.
//!
//! This module provides [`FieldVec<F>`], a newtype wrapper around `Vec<F>` for any
//! type implementing [`FiniteField`], together with [`StridedIter`] for column access
//! in row-major matrix layouts.

use crate::field::{ConstField, FiniteField};
use std::ops::Index;

// ── FieldVec ─────────────────────────────────────────────────────────────────

/// A dense vector of finite field elements.
///
/// `FieldVec<F>` wraps `Vec<F>` and exposes arithmetic operations
/// (dot product, scale, axpy, element-wise add/sub/mul) and functional
/// combinators (map, fold, zip_with) over any type implementing [`FiniteField`].
///
/// # Examples
///
/// ```
/// use gf2_core::field::FieldVec;
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let v = FieldVec::from(vec![field.element(3), field.element(5)]);
/// assert_eq!(v.len(), 2);
/// assert_eq!(v[0], field.element(3));
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldVec<F: FiniteField> {
    data: Vec<F>,
}

// ── Constructors ─────────────────────────────────────────────────────────────

impl<F: FiniteField> FieldVec<F> {
    /// Creates an empty `FieldVec`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// let v = FieldVec::<Gf2mElement>::new();
    /// assert!(v.is_empty());
    /// ```
    pub fn new() -> Self {
        FieldVec { data: Vec::new() }
    }

    /// Creates a `FieldVec` of length `n` filled with `zero.zero_like()`.
    ///
    /// Use this for fields whose zero element is only known at runtime (e.g. `Gf2mElement`).
    /// For [`ConstField`] types, prefer [`FieldVec::zeros`] which requires no argument.
    ///
    /// # Arguments
    ///
    /// * `n` - Number of elements.
    /// * `zero` - Any field element; `zero_like()` supplies the additive identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, FiniteField};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let v = FieldVec::zeros_from(4, &field.zero());
    /// assert_eq!(v.len(), 4);
    /// assert!(v.iter().all(|e| e.is_zero()));
    /// ```
    pub fn zeros_from(n: usize, zero: &F) -> Self {
        FieldVec {
            data: (0..n).map(|_| zero.zero_like()).collect(),
        }
    }

    /// Creates a `FieldVec` with capacity for `n` elements but length zero.
    ///
    /// # Arguments
    ///
    /// * `n` - Initial capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// let v = FieldVec::<Gf2mElement>::with_capacity(16);
    /// assert!(v.is_empty());
    /// ```
    pub fn with_capacity(n: usize) -> Self {
        FieldVec {
            data: Vec::with_capacity(n),
        }
    }
}

impl<F: ConstField> FieldVec<F> {
    /// Creates a `FieldVec` of length `n` filled with `F::zero()`.
    ///
    /// Only available for [`ConstField`] types (those implementing `Copy` with
    /// zero-cost identity constructors). For runtime-configured fields use
    /// [`FieldVec::zeros_from`].
    ///
    /// # Arguments
    ///
    /// * `n` - Number of elements.
    ///
    /// # Complexity
    ///
    /// O(n).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, FiniteField};
    /// use gf2_core::gfp::Fp;
    ///
    /// let v = FieldVec::<Fp<7>>::zeros(4);
    /// assert_eq!(v.len(), 4);
    /// assert!(v.iter().all(|e| e.is_zero()));
    /// ```
    pub fn zeros(n: usize) -> Self {
        FieldVec {
            data: vec![F::zero(); n],
        }
    }
}

// ── Default ──────────────────────────────────────────────────────────────────

impl<F: FiniteField> Default for FieldVec<F> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Element access ────────────────────────────────────────────────────────────

impl<F: FiniteField> FieldVec<F> {
    /// Returns a reference to the element at index `i`.
    ///
    /// # Arguments
    ///
    /// * `i` - Zero-based index.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let v = FieldVec::from(vec![field.element(3)]);
    /// assert_eq!(v.get(0).value(), 3);
    /// ```
    pub fn get(&self, i: usize) -> &F {
        &self.data[i]
    }

    /// Replaces the element at index `i` with `val`.
    ///
    /// # Arguments
    ///
    /// * `i` - Zero-based index.
    /// * `val` - New value.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, FiniteField};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut v = FieldVec::zeros_from(3, &field.zero());
    /// v.set(1, field.element(7));
    /// assert_eq!(v.get(1).value(), 7);
    /// ```
    pub fn set(&mut self, i: usize, val: F) {
        self.data[i] = val;
    }

    /// Appends `val` to the end of the vector.
    ///
    /// # Arguments
    ///
    /// * `val` - Element to append.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut v = FieldVec::new();
    /// v.push(field.element(3));
    /// assert_eq!(v.len(), 1);
    /// assert_eq!(v[0], field.element(3));
    /// ```
    pub fn push(&mut self, val: F) {
        self.data.push(val);
    }

    /// Returns the elements as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let v = FieldVec::from(vec![field.element(1), field.element(2)]);
    /// assert_eq!(v.as_slice().len(), 2);
    /// ```
    pub fn as_slice(&self) -> &[F] {
        &self.data
    }

    /// Returns the elements as a mutable slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut v = FieldVec::from(vec![field.element(1), field.element(2)]);
    /// v.as_mut_slice()[0] = field.element(9);
    /// assert_eq!(v[0], field.element(9));
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [F] {
        &mut self.data
    }

    /// Returns the number of elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// assert_eq!(FieldVec::<Gf2mElement>::new().len(), 0);
    /// ```
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the vector contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// assert!(FieldVec::<Gf2mElement>::new().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ── Index ─────────────────────────────────────────────────────────────────────

impl<F: FiniteField> Index<usize> for FieldVec<F> {
    type Output = F;

    fn index(&self, i: usize) -> &F {
        &self.data[i]
    }
}

// ── Iteration ─────────────────────────────────────────────────────────────────

impl<F: FiniteField> FieldVec<F> {
    /// Returns an iterator over shared references to elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let v = FieldVec::from(vec![field.element(3), field.element(5)]);
    /// assert_eq!(v.iter().count(), 2);
    /// ```
    pub fn iter(&self) -> std::slice::Iter<'_, F> {
        self.data.iter()
    }

    /// Returns an iterator over mutable references to elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, FiniteField};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut v = FieldVec::from(vec![field.element(3), field.element(5)]);
    /// for e in v.iter_mut() {
    ///     *e = e.zero_like();
    /// }
    /// assert!(v.iter().all(|e| e.is_zero()));
    /// ```
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, F> {
        self.data.iter_mut()
    }
}

impl<F: FiniteField> IntoIterator for FieldVec<F> {
    type Item = F;
    type IntoIter = std::vec::IntoIter<F>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a, F: FiniteField> IntoIterator for &'a FieldVec<F> {
    type Item = &'a F;
    type IntoIter = std::slice::Iter<'a, F>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

impl<F: FiniteField> FromIterator<F> for FieldVec<F> {
    fn from_iter<I: IntoIterator<Item = F>>(iter: I) -> Self {
        FieldVec {
            data: iter.into_iter().collect(),
        }
    }
}

// ── Conversion ────────────────────────────────────────────────────────────────

impl<F: FiniteField> From<Vec<F>> for FieldVec<F> {
    fn from(v: Vec<F>) -> Self {
        FieldVec { data: v }
    }
}

impl<F: FiniteField> From<FieldVec<F>> for Vec<F> {
    fn from(fv: FieldVec<F>) -> Self {
        fv.data
    }
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

impl<F: FiniteField> FieldVec<F> {
    /// Computes `∑ self[i] * rhs[i]` in the wide accumulator type.
    ///
    /// The accumulator is seeded from the first pair via [`FiniteField::mul_to_wide`],
    /// then subsequent products are accumulated with `+=`.
    /// Call [`FiniteField::reduce_wide`] on the result to obtain a field element.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand vector; must have the same length as `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()` or either vector is empty.
    ///
    /// # Complexity
    ///
    /// O(n) multiplications.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, FiniteField};
    /// use gf2_core::gf2m::{Gf2mElement, Gf2mField};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = FieldVec::from(vec![field.element(3), field.element(5)]);
    /// let b = FieldVec::from(vec![field.element(2), field.element(1)]);
    /// let wide = a.dot_product(&b);
    /// let result = <Gf2mElement as FiniteField>::reduce_wide(&wide);
    /// // 3*2 XOR 5*1 = 6 XOR 5 = 3 in GF(16)
    /// assert_eq!(result, field.element(3));
    /// ```
    pub fn dot_product(&self, rhs: &Self) -> F::Wide {
        assert_eq!(
            self.len(),
            rhs.len(),
            "dot_product: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        assert!(!self.is_empty(), "dot_product: vectors must not be empty");

        let mut iter = self.data.iter().zip(rhs.data.iter());
        let (a0, b0) = iter.next().unwrap();
        let mut acc = a0.mul_to_wide(b0);
        for (a, b) in iter {
            acc += a.mul_to_wide(b);
        }
        acc
    }

    /// Returns a new `FieldVec` with each element multiplied by scalar `a`.
    ///
    /// # Arguments
    ///
    /// * `a` - Scalar field element.
    ///
    /// # Complexity
    ///
    /// O(n) multiplications.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let v = FieldVec::from(vec![field.element(3), field.element(5)]);
    /// let scaled = v.scale(&field.element(2));
    /// assert_eq!(scaled[0], field.element(3) * field.element(2));
    /// assert_eq!(scaled[1], field.element(5) * field.element(2));
    /// ```
    pub fn scale(&self, a: &F) -> Self {
        FieldVec {
            data: self.data.iter().map(|e| e.clone() * a.clone()).collect(),
        }
    }

    /// In-place fused multiply-add: `self[i] += a * rhs[i]` for all `i`.
    ///
    /// # Arguments
    ///
    /// * `a` - Scalar field element.
    /// * `rhs` - Right-hand vector; must have the same length as `self`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Complexity
    ///
    /// O(n) multiplications.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let mut y = FieldVec::from(vec![field.element(1), field.element(2)]);
    /// let x = FieldVec::from(vec![field.element(3), field.element(4)]);
    /// y.axpy(&field.element(2), &x);
    /// assert_eq!(y[0], field.element(1) + field.element(2) * field.element(3));
    /// assert_eq!(y[1], field.element(2) + field.element(2) * field.element(4));
    /// ```
    pub fn axpy(&mut self, a: &F, rhs: &Self) {
        assert_eq!(
            self.len(),
            rhs.len(),
            "axpy: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        for (y, x) in self.data.iter_mut().zip(rhs.data.iter()) {
            *y += a.clone() * x.clone();
        }
    }
}

// ── Element-wise ops ──────────────────────────────────────────────────────────

impl<F: FiniteField> FieldVec<F> {
    /// Returns `self[i] + rhs[i]` element-wise.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = FieldVec::from(vec![field.element(5), field.element(3)]);
    /// let b = FieldVec::from(vec![field.element(1), field.element(2)]);
    /// let c = a.add_vec(&b);
    /// assert_eq!(c[0], field.element(5 ^ 1));
    /// assert_eq!(c[1], field.element(3 ^ 2));
    /// ```
    pub fn add_vec(&self, rhs: &Self) -> Self {
        assert_eq!(
            self.len(),
            rhs.len(),
            "add_vec: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        FieldVec {
            data: self
                .data
                .iter()
                .zip(rhs.data.iter())
                .map(|(a, b)| a.clone() + b.clone())
                .collect(),
        }
    }

    /// Returns `self[i] - rhs[i]` element-wise.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = FieldVec::from(vec![field.element(5), field.element(3)]);
    /// let b = FieldVec::from(vec![field.element(1), field.element(2)]);
    /// let c = a.sub_vec(&b);
    /// // In GF(2^m), sub == add
    /// assert_eq!(c[0], field.element(5 ^ 1));
    /// ```
    pub fn sub_vec(&self, rhs: &Self) -> Self {
        assert_eq!(
            self.len(),
            rhs.len(),
            "sub_vec: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        FieldVec {
            data: self
                .data
                .iter()
                .zip(rhs.data.iter())
                .map(|(a, b)| a.clone() - b.clone())
                .collect(),
        }
    }

    /// Returns the Hadamard product `self[i] * rhs[i]` element-wise.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    ///
    /// # Complexity
    ///
    /// O(n) multiplications.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = FieldVec::from(vec![field.element(3), field.element(5)]);
    /// let b = FieldVec::from(vec![field.element(2), field.element(1)]);
    /// let c = a.mul_vec(&b);
    /// assert_eq!(c[0], field.element(3) * field.element(2));
    /// assert_eq!(c[1], field.element(5) * field.element(1));
    /// ```
    pub fn mul_vec(&self, rhs: &Self) -> Self {
        assert_eq!(
            self.len(),
            rhs.len(),
            "mul_vec: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        FieldVec {
            data: self
                .data
                .iter()
                .zip(rhs.data.iter())
                .map(|(a, b)| a.clone() * b.clone())
                .collect(),
        }
    }
}

// ── Functional ────────────────────────────────────────────────────────────────

impl<F: FiniteField> FieldVec<F> {
    /// Applies `f` to each element, returning a new `FieldVec<G>`.
    ///
    /// # Arguments
    ///
    /// * `f` - Mapping function.
    ///
    /// # Complexity
    ///
    /// O(n) applications of `f`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, FiniteField};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let v = FieldVec::from(vec![field.element(3), field.element(5)]);
    /// // x + x = 0 in characteristic-2 fields
    /// let doubled = v.map(|e| e.clone() + e.clone());
    /// assert!(doubled.iter().all(|e| e.is_zero()));
    /// ```
    pub fn map<G: FiniteField, Map: FnMut(&F) -> G>(&self, f: Map) -> FieldVec<G> {
        FieldVec {
            data: self.data.iter().map(f).collect(),
        }
    }

    /// Reduces the vector to a single value by applying `f` left-to-right.
    ///
    /// # Arguments
    ///
    /// * `init` - Initial accumulator value.
    /// * `f` - Combining function: `(accumulator, element) → new_accumulator`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let v = FieldVec::from(vec![field.element(1), field.element(2), field.element(4)]);
    /// let sum = v.fold(field.zero(), |acc, e| acc + e.clone());
    /// assert_eq!(sum, field.element(7)); // 1 XOR 2 XOR 4 = 7
    /// ```
    pub fn fold<B, Func: FnMut(B, &F) -> B>(&self, init: B, f: Func) -> B {
        self.data.iter().fold(init, f)
    }

    /// Combines two equal-length vectors element-wise using `f`, returning a new `FieldVec<G>`.
    ///
    /// # Arguments
    ///
    /// * `other` - Right-hand vector; must have the same length as `self`.
    /// * `f` - Combining function applied to corresponding pairs.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    ///
    /// # Complexity
    ///
    /// O(n) applications of `f`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = FieldVec::from(vec![field.element(3), field.element(5)]);
    /// let b = FieldVec::from(vec![field.element(2), field.element(1)]);
    /// let c = a.zip_with(&b, |x, y| x.clone() + y.clone());
    /// assert_eq!(c[0], field.element(3 ^ 2));
    /// assert_eq!(c[1], field.element(5 ^ 1));
    /// ```
    pub fn zip_with<G: FiniteField, Func: FnMut(&F, &F) -> G>(
        &self,
        other: &Self,
        mut f: Func,
    ) -> FieldVec<G> {
        assert_eq!(
            self.len(),
            other.len(),
            "zip_with: length mismatch ({} vs {})",
            self.len(),
            other.len()
        );
        FieldVec {
            data: self
                .data
                .iter()
                .zip(other.data.iter())
                .map(|(a, b)| f(a, b))
                .collect(),
        }
    }
}

// ── StridedIter ───────────────────────────────────────────────────────────────

/// Iterator that steps through a slice with a fixed stride.
///
/// Useful for column access in row-major matrix layouts stored as flat slices.
///
/// # Examples
///
/// ```
/// use gf2_core::field::StridedIter;
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let data = vec![
///     field.element(0), field.element(1),
///     field.element(2), field.element(3),
///     field.element(4), field.element(5),
/// ];
/// // Column 0 (stride 2): indices 0, 2, 4
/// let col: Vec<_> = StridedIter::new(&data, 0, 2).collect();
/// assert_eq!(col, vec![&field.element(0), &field.element(2), &field.element(4)]);
/// ```
pub struct StridedIter<'a, F> {
    slice: &'a [F],
    pos: usize,
    stride: usize,
}

impl<'a, F> StridedIter<'a, F> {
    /// Creates a `StridedIter` starting at index `start`, advancing by `stride` each step.
    ///
    /// # Arguments
    ///
    /// * `slice` - The underlying data slice.
    /// * `start` - Index of the first element to yield.
    /// * `stride` - Number of positions to advance between consecutive yields. Must be ≥ 1.
    ///
    /// # Panics
    ///
    /// Panics if `stride == 0` (would cause non-terminating iteration).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::StridedIter;
    ///
    /// let data = [10u32, 20, 30, 40, 50];
    /// let vals: Vec<_> = StridedIter::new(&data, 1, 2).copied().collect();
    /// assert_eq!(vals, vec![20, 40]);
    /// ```
    pub fn new(slice: &'a [F], start: usize, stride: usize) -> Self {
        assert!(stride > 0, "StridedIter: stride must be at least 1");
        StridedIter {
            slice,
            pos: start,
            stride,
        }
    }
}

impl<'a, F> Iterator for StridedIter<'a, F> {
    type Item = &'a F;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.slice.len() {
            let item = &self.slice[self.pos];
            self.pos += self.stride;
            Some(item)
        } else {
            None
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FiniteField;
    use crate::gf2m::{Gf2mElement, Gf2mField};

    #[test]
    fn test_new_is_empty() {
        let v = FieldVec::<Gf2mElement>::new();
        assert_eq!(v.len(), 0);
        assert!(v.is_empty());
    }

    #[test]
    fn test_zeros_creates_zero_vector() {
        let f = Gf2mField::new(4, 0b10011);
        let v = FieldVec::zeros_from(4, &f.zero());
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|e| e.is_zero()));
    }

    #[test]
    fn test_from_vec_round_trip() {
        let f = Gf2mField::new(4, 0b10011);
        let original = vec![f.element(3), f.element(5), f.element(7)];
        let fv = FieldVec::from(original.clone());
        let back: Vec<_> = Vec::from(fv);
        assert_eq!(back, original);
    }

    #[test]
    fn test_get_set_index() {
        let f = Gf2mField::new(4, 0b10011);
        let mut v = FieldVec::zeros_from(3, &f.zero());
        v.set(1, f.element(7));
        assert_eq!(v.get(1), &f.element(7));
        assert_eq!(v[1], f.element(7));
        assert!(v.get(0).is_zero());
        assert!(v.get(2).is_zero());
    }

    #[test]
    fn test_push() {
        let f = Gf2mField::new(4, 0b10011);
        let mut v = FieldVec::new();
        v.push(f.element(3));
        v.push(f.element(5));
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], f.element(3));
        assert_eq!(v[1], f.element(5));
    }

    #[test]
    fn test_len_is_empty() {
        let f = Gf2mField::new(4, 0b10011);
        let mut v = FieldVec::new();
        assert!(v.is_empty());
        v.push(f.element(1));
        assert!(!v.is_empty());
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn test_dot_product_gf16() {
        // [3, 5] · [2, 1] = 3*2 XOR 5*1 = 6 XOR 5 = 3
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.element(3), f.element(5)]);
        let b = FieldVec::from(vec![f.element(2), f.element(1)]);
        let wide = a.dot_product(&b);
        let result = <Gf2mElement as FiniteField>::reduce_wide(&wide);
        assert_eq!(result, f.element(3));
    }

    #[test]
    fn test_dot_product_orthogonal() {
        // [1, 0] · [0, 1] = 1*0 XOR 0*1 = 0
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.one(), f.zero()]);
        let b = FieldVec::from(vec![f.zero(), f.one()]);
        let wide = a.dot_product(&b);
        let result = <Gf2mElement as FiniteField>::reduce_wide(&wide);
        assert!(result.is_zero());
    }

    #[test]
    #[should_panic]
    fn test_dot_product_length_mismatch_panics() {
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.element(1), f.element(2)]);
        let b = FieldVec::from(vec![f.element(3)]);
        let _ = a.dot_product(&b);
    }

    #[test]
    #[should_panic]
    fn test_dot_product_empty_panics() {
        let a = FieldVec::<Gf2mElement>::new();
        let b = FieldVec::<Gf2mElement>::new();
        let _ = a.dot_product(&b);
    }

    #[test]
    fn test_scale() {
        let f = Gf2mField::new(4, 0b10011);
        let v = FieldVec::from(vec![f.element(3), f.element(5)]);
        let scaled = v.scale(&f.element(2));
        assert_eq!(scaled[0], f.element(3) * f.element(2));
        assert_eq!(scaled[1], f.element(5) * f.element(2));
    }

    #[test]
    fn test_axpy() {
        let f = Gf2mField::new(4, 0b10011);
        let mut y = FieldVec::from(vec![f.element(1), f.element(2)]);
        let x = FieldVec::from(vec![f.element(3), f.element(4)]);
        y.axpy(&f.element(2), &x);
        // y[i] = old_y[i] + a * x[i]
        assert_eq!(y[0], f.element(1) + f.element(2) * f.element(3));
        assert_eq!(y[1], f.element(2) + f.element(2) * f.element(4));
    }

    #[test]
    fn test_add_vec() {
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.element(5), f.element(3)]);
        let b = FieldVec::from(vec![f.element(1), f.element(2)]);
        let c = a.add_vec(&b);
        assert_eq!(c[0], f.element(5 ^ 1));
        assert_eq!(c[1], f.element(3 ^ 2));
    }

    #[test]
    fn test_sub_vec() {
        // In GF(2^m), sub == add
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.element(5), f.element(3)]);
        let b = FieldVec::from(vec![f.element(1), f.element(2)]);
        let c = a.sub_vec(&b);
        assert_eq!(c[0], f.element(5 ^ 1));
        assert_eq!(c[1], f.element(3 ^ 2));
    }

    #[test]
    fn test_mul_vec() {
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.element(3), f.element(5)]);
        let b = FieldVec::from(vec![f.element(2), f.element(1)]);
        let c = a.mul_vec(&b);
        assert_eq!(c[0], f.element(3) * f.element(2));
        assert_eq!(c[1], f.element(5) * f.element(1));
    }

    #[test]
    fn test_map() {
        let f = Gf2mField::new(4, 0b10011);
        let v = FieldVec::from(vec![f.element(3), f.element(5)]);
        // x + x = 0 in characteristic 2
        let doubled: FieldVec<Gf2mElement> = v.map(|e| e.clone() + e.clone());
        assert!(doubled.iter().all(|e| e.is_zero()));
    }

    #[test]
    fn test_fold() {
        let f = Gf2mField::new(4, 0b10011);
        let v = FieldVec::from(vec![f.element(1), f.element(2), f.element(4)]);
        let sum = v.fold(f.zero(), |acc, e| acc + e.clone());
        assert_eq!(sum, f.element(7)); // 1 XOR 2 XOR 4 = 7
    }

    #[test]
    fn test_zip_with() {
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.element(3), f.element(5)]);
        let b = FieldVec::from(vec![f.element(2), f.element(1)]);
        let c: FieldVec<Gf2mElement> = a.zip_with(&b, |x, y| x.clone() + y.clone());
        assert_eq!(c[0], f.element(3 ^ 2));
        assert_eq!(c[1], f.element(5 ^ 1));
    }

    #[test]
    #[should_panic]
    fn test_zip_with_length_mismatch_panics() {
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.element(1), f.element(2)]);
        let b = FieldVec::from(vec![f.element(3)]);
        let _: FieldVec<Gf2mElement> = a.zip_with(&b, |x, y| x.clone() + y.clone());
    }

    #[test]
    fn test_into_iter_owned() {
        let f = Gf2mField::new(4, 0b10011);
        let expected = vec![f.element(1), f.element(2), f.element(3)];
        let v = FieldVec::from(expected.clone());
        let collected: Vec<_> = v.into_iter().collect();
        assert_eq!(collected, expected);
    }

    #[test]
    fn test_into_iter_borrowed() {
        let f = Gf2mField::new(4, 0b10011);
        let v = FieldVec::from(vec![f.element(1), f.element(2), f.element(3)]);
        let collected: Vec<_> = (&v).into_iter().collect();
        assert_eq!(collected, vec![&f.element(1), &f.element(2), &f.element(3)]);
        // v still valid
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn test_from_iterator() {
        let f = Gf2mField::new(4, 0b10011);
        let v: FieldVec<Gf2mElement> = vec![f.element(1), f.element(2), f.element(3)]
            .into_iter()
            .collect();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], f.element(1));
        assert_eq!(v[2], f.element(3));
    }

    #[test]
    fn test_strided_iter() {
        let f = Gf2mField::new(4, 0b10011);
        let data = vec![
            f.element(0),
            f.element(1),
            f.element(2),
            f.element(3),
            f.element(4),
            f.element(5),
        ];
        // stride 2 from start 0 → indices 0, 2, 4
        let col: Vec<_> = StridedIter::new(&data, 0, 2).collect();
        assert_eq!(col, vec![&f.element(0), &f.element(2), &f.element(4)]);

        // stride 2 from start 1 → indices 1, 3, 5
        let col2: Vec<_> = StridedIter::new(&data, 1, 2).collect();
        assert_eq!(col2, vec![&f.element(1), &f.element(3), &f.element(5)]);
    }

    #[test]
    fn test_clone_and_eq() {
        let f = Gf2mField::new(4, 0b10011);
        let v = FieldVec::from(vec![f.element(3), f.element(5), f.element(7)]);
        let w = v.clone();
        assert_eq!(v, w);

        let x = FieldVec::from(vec![f.element(3), f.element(5), f.element(6)]);
        assert_ne!(v, x);
    }

    // ── Property-based tests ──────────────────────────────────────────────────

    proptest::proptest! {
        /// dot_product is bilinear: (a·x) · y = a * (x · y) for scalar a.
        #[test]
        fn prop_dot_product_scale_linear(
            raw_a in 1u32..15,
            xs in proptest::collection::vec(1u32..15, 1..8),
            ys in proptest::collection::vec(1u32..15, 1..8),
        ) {
            // Restrict to equal lengths
            let len = xs.len().min(ys.len());
            let f = Gf2mField::new(4, 0b10011);
            let a = f.element(raw_a as u64);
            let x: FieldVec<Gf2mElement> = xs[..len].iter().map(|&v| f.element(v as u64)).collect();
            let y: FieldVec<Gf2mElement> = ys[..len].iter().map(|&v| f.element(v as u64)).collect();

            // (a*x) · y == a * (x · y)  — bilinearity
            let ax = x.scale(&a);
            let lhs = <Gf2mElement as FiniteField>::reduce_wide(&ax.dot_product(&y));
            let rhs = a.clone() * <Gf2mElement as FiniteField>::reduce_wide(&x.dot_product(&y));
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// axpy correctness: y after axpy equals element-wise y[i] + a*x[i].
        #[test]
        fn prop_axpy_matches_manual(
            raw_a in 0u32..15,
            xs in proptest::collection::vec(0u32..15, 1..8),
            ys in proptest::collection::vec(0u32..15, 1..8),
        ) {
            let len = xs.len().min(ys.len());
            let f = Gf2mField::new(4, 0b10011);
            let a = f.element(raw_a as u64);
            let x: FieldVec<Gf2mElement> = xs[..len].iter().map(|&v| f.element(v as u64)).collect();
            let mut y: FieldVec<Gf2mElement> = ys[..len].iter().map(|&v| f.element(v as u64)).collect();
            let y_orig = y.clone();

            y.axpy(&a, &x);

            for i in 0..len {
                proptest::prop_assert_eq!(y[i].clone(), y_orig[i].clone() + a.clone() * x[i].clone());
            }
        }

        /// add_vec associativity: (a + b) + c == a + (b + c).
        #[test]
        fn prop_add_vec_associative(
            elems in proptest::collection::vec(0u32..15, 3..9),
        ) {
            let n = elems.len() / 3;
            if n == 0 { return Ok(()); }
            let f = Gf2mField::new(4, 0b10011);
            let mk = |slice: &[u32]| -> FieldVec<Gf2mElement> {
                slice.iter().map(|&v| f.element(v as u64)).collect()
            };
            let a = mk(&elems[..n]);
            let b = mk(&elems[n..2*n]);
            let c = mk(&elems[2*n..3*n]);

            let lhs = a.add_vec(&b).add_vec(&c);
            let rhs = a.add_vec(&b.add_vec(&c));
            proptest::prop_assert_eq!(lhs, rhs);
        }
    }
}
