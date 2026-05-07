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
    /// Computes `∑ self[i] * rhs[i]` with delayed reduction.
    ///
    /// Uses [`FiniteField::max_unreduced_additions`] to determine how many wide
    /// multiply-add accumulations can be performed before reduction is needed to
    /// avoid overflow. This is both a correctness requirement (prevents `u128`
    /// overflow for large primes) and a performance optimisation (minimises
    /// expensive Montgomery reductions).
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
    /// O(n) multiplications and `⌈n / kmax⌉` reductions, where
    /// `kmax = F::max_unreduced_additions()`.
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
    /// let result = a.dot_product(&b);
    /// // 3*2 XOR 5*1 = 6 XOR 5 = 3 in GF(16)
    /// assert_eq!(result, field.element(3));
    /// ```
    ///
    /// ```
    /// use gf2_core::field::{FieldVec, ConstField};
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = FieldVec::from(vec![Fp::<7>::new(3), Fp::<7>::new(5)]);
    /// let b = FieldVec::from(vec![Fp::<7>::new(2), Fp::<7>::new(4)]);
    /// // 3*2 + 5*4 = 6 + 20 = 26 ≡ 5 (mod 7)
    /// assert_eq!(a.dot_product(&b), Fp::<7>::new(5));
    /// ```
    pub fn dot_product(&self, rhs: &Self) -> F {
        assert_eq!(
            self.len(),
            rhs.len(),
            "dot_product: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        assert!(!self.is_empty(), "dot_product: vectors must not be empty");
        // SIMD fast path: dispatches through
        // `FiniteField::try_simd_dot_product` (currently the small-prime
        // AVX2 byte-lane kernel for `Fp<P>` with `P <= 251`); falls back
        // to the chunked-Wide kernel when no specialised SIMD path
        // applies.
        if let Some(value) = F::try_simd_dot_product(&self.data, &rhs.data) {
            return value;
        }
        // Delegate to the slice-based kernel. Using the first element as the
        // zero witness matches the previous behaviour exactly (`self.data[0]
        // .zero_like()`).
        let zero = self.data[0].zero_like();
        dot_product_slices(&self.data, &rhs.data, &zero)
    }
}

/// Slice-level dot product with delayed reduction.
///
/// This is the inner kernel shared by [`FieldVec::dot_product`] and the
/// [`FieldMatrix`](crate::field::matrix::FieldMatrix) classical `gemm` /
/// `matvec` / `matvec_transpose` paths. It exists so those callers can
/// compute `∑ a[i] * b[i]` over arbitrary borrowed slices without first
/// materialising a `FieldVec`.
///
/// Correctness contract (Dumas–Pernet §1.2, theorem 4 classical case):
/// accumulates at most
/// [`FiniteField::max_unreduced_additions`](crate::field::FiniteField::max_unreduced_additions)
/// wide products before reducing, so the `Wide` accumulator never overflows
/// for any finite field this crate models. For `Fp<P>` this uses the
/// storage-domain product-sum hook: Montgomery fields accumulate raw
/// `(aR)·(bR)` products and do one modulo-`P` plus one REDC per chunk,
/// rather than converting both operands out of Montgomery form on every
/// multiply. The product bound is unchanged because every raw storage word
/// is still `< P`.
///
/// # Arguments
///
/// * `a`, `b` — Slices of equal length `n >= 0`.
/// * `zero` — Any field element; `zero.zero_like()` is used to seed the
///   accumulator for the empty-input case and for chunked reductions.
///
/// # Panics
///
/// Panics in debug builds if `a.len() != b.len()`.
#[inline]
pub(crate) fn dot_product_slices<F: FiniteField>(a: &[F], b: &[F], zero: &F) -> F {
    debug_assert_eq!(
        a.len(),
        b.len(),
        "dot_product_slices: length mismatch ({} vs {})",
        a.len(),
        b.len()
    );
    if a.is_empty() {
        return zero.zero_like();
    }

    // Prime-field SIMD dot hook — overridden by `Fp<P>` for medium primes
    // (`P ∈ (251, 65536)`) to route through the AVX2 16-lane u16 Barrett
    // kernel in `gf2-kernels-simd::fp_medium`. For every other field the
    // default returns `None` and we continue through the delayed-reduction
    // path below.
    //
    // This entry point allocates scratch buffers locally; the GEMM kernel
    // calls the hook directly with reused scratches to amortise the
    // packing cost across many output cells.
    let mut scratch_a: Vec<u16> = Vec::new();
    let mut scratch_b: Vec<u16> = Vec::new();
    if let Some(value) = F::try_fp_simd_dot_product(a, b, &mut scratch_a, &mut scratch_b) {
        return value;
    }

    let kmax = F::max_unreduced_additions();

    if kmax == usize::MAX {
        // Fast path: no overflow possible (e.g., GF(2^m) where Wide = Self).
        let mut acc = a[0].mul_product_sum_wide(&b[0]);
        for (x, y) in a[1..].iter().zip(b[1..].iter()) {
            acc += x.mul_product_sum_wide(y);
        }
        F::reduce_product_sum_wide(&acc)
    } else if kmax == 0 {
        // Degenerate: reduce after every multiply.
        let mut acc = a[0].clone() * b[0].clone();
        for (x, y) in a[1..].iter().zip(b[1..].iter()) {
            acc += &(x.clone() * y);
        }
        acc
    } else {
        // General case: chunk by kmax, accumulate in Wide, reduce at boundaries.
        // Each chunk contributes at most kmax wide products — this is the
        // delayed-reduction bound theorem 4 / §1.2 of Dumas–Pernet.
        let mut result = zero.zero_like();
        let mut offset = 0usize;
        while offset < a.len() {
            let chunk_size = (a.len() - offset).min(kmax);
            debug_assert!(
                chunk_size <= kmax,
                "dot_product_slices: chunk size {} exceeds kmax {}",
                chunk_size,
                kmax
            );
            let mut acc = a[offset].mul_product_sum_wide(&b[offset]);
            for i in 1..chunk_size {
                acc += a[offset + i].mul_product_sum_wide(&b[offset + i]);
            }
            result += &F::reduce_product_sum_wide(&acc);
            offset += chunk_size;
        }
        result
    }
}

impl<F: FiniteField> FieldVec<F> {
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
        // SIMD fast path for `Fp<P>` with `P ≤ 65521` (issue d1dd266c).
        // Falls through to the scalar zip-loop when no kernel is
        // registered for the field, when AVX2 is unavailable at
        // runtime, or when the `simd` feature is disabled.
        if F::try_simd_axpy(self.data.as_mut_slice(), a, rhs.data.as_slice()) {
            return;
        }
        for (y, x) in self.data.iter_mut().zip(rhs.data.iter()) {
            *y += a.clone() * x.clone();
        }
    }
}

// ── Element-wise ops ──────────────────────────────────────────────────────────

impl<F: FiniteField + SimdVecOps> FieldVec<F> {
    /// Returns `self[i] + rhs[i]` element-wise.
    ///
    /// For base fields with a SIMD kernel (including supported `Fp<P>` primes
    /// on AVX2 hosts with the `simd` feature) this dispatches through
    /// [`SimdVecOps::try_simd_add_vec`]; other base fields, unsupported
    /// hardware, and `simd`-disabled builds fall back to the scalar
    /// element-wise loop.
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
        if let Some(out) = F::try_simd_add_vec(&self.data, &rhs.data) {
            return FieldVec { data: out };
        }
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
    /// For base fields with a SIMD kernel (including supported `Fp<P>` primes
    /// on AVX2 hosts with the `simd` feature) this dispatches through
    /// [`SimdVecOps::try_simd_sub_vec`]; other base fields, unsupported
    /// hardware, and `simd`-disabled builds fall back to the scalar
    /// element-wise loop.
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
        if let Some(out) = F::try_simd_sub_vec(&self.data, &rhs.data) {
            return FieldVec { data: out };
        }
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
    /// For base fields with a SIMD kernel (including supported `Fp<P>` primes
    /// on AVX2 hosts with the `simd` feature) this dispatches through
    /// [`SimdVecOps::try_simd_mul_vec`]; other base fields, unsupported
    /// hardware, and `simd`-disabled builds fall back to the scalar
    /// element-wise loop.
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
        if let Some(out) = F::try_simd_mul_vec(&self.data, &rhs.data) {
            return FieldVec { data: out };
        }
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

// ── SimdVecOps: element-wise SIMD dispatch hook for FieldVec ────────────────
//
// `FieldVec::mul_vec`/`add_vec`/`sub_vec` consult this trait's `try_simd_*`
// hooks before falling back to scalar loops. Every base field may return
// `None` (the default, giving scalar behaviour) or override the hook to
// route through a kernel in `gf2-kernels-simd`.
//
// `Fp<65537>` and supported Montgomery `Fp<P>` primes route to AVX2 kernels
// through the central helpers in [`crate::gfp::simd_ops`]. Unsupported fields
// inherit the `None` default through the same blanket impl.

pub use crate::gfp::SimdVecOps;

// ── GF(2^m)-specific SIMD-accelerated dot product ───────────────────────────

use crate::gf2m::Gf2mElement;

impl FieldVec<Gf2mElement> {
    /// SIMD-accelerated dot product for GF(2^m) using PCLMULQDQ batch kernel.
    ///
    /// Uses `clmul_batch` to perform all carry-less multiplications in a single
    /// vectorised pass (VPCLMULQDQ when available, sequential PCLMULQDQ otherwise),
    /// XORs all 128-bit products into one accumulator, then Barrett-reduces once.
    ///
    /// Falls back to the generic [`dot_product`](FieldVec::dot_product) when
    /// PCLMULQDQ is not available at runtime.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand; must have the same length as `self`.
    ///
    /// # Panics
    ///
    /// Panics if the vectors have different lengths or are empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FieldVec;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(8, 0x11B);
    /// let a = FieldVec::from(vec![field.element(0x53), field.element(0xCA)]);
    /// let b = FieldVec::from(vec![field.element(0x12), field.element(0x34)]);
    /// let simd_result = a.simd_dot_product(&b);
    /// assert_eq!(simd_result, a.dot_product(&b));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) carry-less multiplications (batched) + O(n) XOR accumulation +
    /// O(1) Barrett reduction.
    pub fn simd_dot_product(&self, rhs: &Self) -> Gf2mElement {
        assert_eq!(
            self.len(),
            rhs.len(),
            "simd_dot_product: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        assert!(
            !self.is_empty(),
            "simd_dot_product: vectors must not be empty"
        );

        self.try_simd_dot_product(rhs)
            .unwrap_or_else(|| self.dot_product(rhs))
    }

    /// Attempts the SIMD batch path. Returns `None` when hardware support is
    /// unavailable, so the caller can fall back.
    ///
    /// Processes the vectors in fixed-size chunks to keep scratch buffers on the
    /// stack and avoid per-call heap allocations.
    #[cfg(feature = "simd")]
    fn try_simd_dot_product(&self, rhs: &Self) -> Option<Gf2mElement> {
        // Grab SIMD function pointers from the first element's field params.
        let sample = &self.data[0];
        let batch_fn = sample.clmul_batch_fn()?;
        let clmul_fn = sample.clmul_fn()?;
        let reducer = sample.barrett_reducer()?;

        // Process in chunks that fit comfortably on the stack.
        // 256 elements = 256*8 (a) + 256*8 (b) + 256*16 (products) = 8 KiB.
        const CHUNK: usize = 256;
        let mut a_buf = [0u64; CHUNK];
        let mut b_buf = [0u64; CHUNK];
        let mut p_buf = [0u128; CHUNK];

        let mut acc: u128 = 0;
        let mut offset = 0;
        let n = self.len();

        while offset < n {
            let end = (offset + CHUNK).min(n);
            let chunk_len = end - offset;

            // Extract raw u64 values into stack buffers.
            for (i, (a, b)) in self.data[offset..end]
                .iter()
                .zip(&rhs.data[offset..end])
                .enumerate()
            {
                a_buf[i] = a.value();
                b_buf[i] = b.value();
            }

            // Batch carry-less multiply (VPCLMULQDQ when available).
            batch_fn(
                &a_buf[..chunk_len],
                &b_buf[..chunk_len],
                &mut p_buf[..chunk_len],
            );

            // XOR-accumulate the 128-bit products.
            for &p in &p_buf[..chunk_len] {
                acc ^= p;
            }

            offset = end;
        }

        // Single Barrett reduction at the very end.
        let result = reducer.reduce_with_clmul(acc, clmul_fn);

        Some(sample.with_raw_value(result))
    }

    #[cfg(not(feature = "simd"))]
    fn try_simd_dot_product(&self, _rhs: &Self) -> Option<Gf2mElement> {
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
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
        let result = a.dot_product(&b);
        assert_eq!(result, f.element(3));
    }

    #[test]
    fn test_dot_product_orthogonal() {
        // [1, 0] · [0, 1] = 1*0 XOR 0*1 = 0
        let f = Gf2mField::new(4, 0b10011);
        let a = FieldVec::from(vec![f.one(), f.zero()]);
        let b = FieldVec::from(vec![f.zero(), f.one()]);
        let result = a.dot_product(&b);
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

    // ── Fp dot product tests ──────────────────────────────────────────────────

    #[test]
    fn test_dot_product_fp7_matches_elementwise() {
        use crate::gfp::Fp;
        // [3, 5, 6] · [2, 4, 1] = 3*2 + 5*4 + 6*1 = 6 + 20 + 6 = 32 ≡ 4 (mod 7)
        let a = FieldVec::from(vec![Fp::<7>::new(3), Fp::<7>::new(5), Fp::<7>::new(6)]);
        let b = FieldVec::from(vec![Fp::<7>::new(2), Fp::<7>::new(4), Fp::<7>::new(1)]);
        let dot = a.dot_product(&b);

        // Element-wise reference
        let manual = Fp::<7>::new(3) * Fp::<7>::new(2)
            + Fp::<7>::new(5) * Fp::<7>::new(4)
            + Fp::<7>::new(6) * Fp::<7>::new(1);
        assert_eq!(dot, manual);
        assert_eq!(dot, Fp::<7>::new(4));
    }

    #[test]
    fn test_dot_product_fp65521_matches_elementwise() {
        use crate::gfp::Fp;
        let vals_a: Vec<u64> = vec![100, 200, 300, 400, 500];
        let vals_b: Vec<u64> = vec![600, 700, 800, 900, 1000];
        let a: FieldVec<Fp<65521>> = vals_a.iter().map(|&v| Fp::<65521>::new(v)).collect();
        let b: FieldVec<Fp<65521>> = vals_b.iter().map(|&v| Fp::<65521>::new(v)).collect();
        let dot = a.dot_product(&b);

        // Manual: 100*600 + 200*700 + 300*800 + 400*900 + 500*1000
        //       = 60000 + 140000 + 240000 + 360000 + 500000 = 1300000
        // 1300000 mod 65521 = 1300000 - 19*65521 = 1300000 - 1244899 = 55101
        let manual: Fp<65521> = vals_a
            .iter()
            .zip(vals_b.iter())
            .map(|(&ai, &bi)| Fp::<65521>::new(ai) * Fp::<65521>::new(bi))
            .fold(Fp::<65521>::new(0), |acc, x| acc + x);
        assert_eq!(dot, manual);
    }

    #[test]
    fn test_dot_product_fp65521_boundary_lengths() {
        use crate::gfp::Fp;
        for n in [1, 2, 100, 1000] {
            let a: FieldVec<Fp<65521>> = FieldVec::from(
                (0..n)
                    .map(|i| Fp::<65521>::new((i as u64 * 7 + 3) % 65521))
                    .collect::<Vec<_>>(),
            );
            let b: FieldVec<Fp<65521>> = FieldVec::from(
                (0..n)
                    .map(|i| Fp::<65521>::new((i as u64 * 13 + 11) % 65521))
                    .collect::<Vec<_>>(),
            );
            let result = a.dot_product(&b);
            let expected: Fp<65521> = a
                .iter()
                .zip(b.iter())
                .fold(Fp::<65521>::zero(), |acc, (ai, bi)| acc + (*ai * *bi));
            assert_eq!(result, expected, "Fp<65521> dot product mismatch at n={n}");
        }
    }

    #[test]
    fn test_dot_product_large_prime_no_overflow() {
        use crate::gfp::Fp;
        // P near 2^63: kmax = u128::MAX / (P-1)^2 ≈ 4, so chunking kicks in
        // for vectors longer than ~5 elements. The old code would accumulate
        // without chunking and could overflow u128 for long vectors.
        const P: u64 = 9_223_372_036_854_775_783; // largest prime <= 2^63
        let n = 100;
        let a: FieldVec<Fp<P>> = (0..n).map(|i| Fp::<P>::new(i + 1)).collect();
        let b: FieldVec<Fp<P>> = (0..n).map(|i| Fp::<P>::new(i + 100)).collect();
        let dot = a.dot_product(&b);

        // Verify against element-wise computation (which reduces per multiply)
        let manual: Fp<P> = (0..n)
            .map(|i| Fp::<P>::new(i + 1) * Fp::<P>::new(i + 100))
            .fold(Fp::<P>::new(0), |acc, x| acc + x);
        assert_eq!(dot, manual);
    }

    #[test]
    fn test_dot_product_large_prime_boundary_lengths() {
        use crate::gfp::Fp;
        const P: u64 = 9_223_372_036_854_775_783;
        for n in [1, 2, 1000] {
            let a: FieldVec<Fp<P>> = FieldVec::from(
                (0..n)
                    .map(|i| Fp::<P>::new((i as u64 * 7 + 3) % P))
                    .collect::<Vec<_>>(),
            );
            let b: FieldVec<Fp<P>> = FieldVec::from(
                (0..n)
                    .map(|i| Fp::<P>::new((i as u64 * 13 + 11) % P))
                    .collect::<Vec<_>>(),
            );
            let result = a.dot_product(&b);
            let expected: Fp<P> = a
                .iter()
                .zip(b.iter())
                .fold(Fp::<P>::zero(), |acc, (ai, bi)| acc + (*ai * *bi));
            assert_eq!(result, expected, "Fp<{P}> dot product mismatch at n={n}");
        }
    }

    #[test]
    fn test_dot_product_gf2m_unchanged() {
        // GF(2^8) dot product should match element-wise XOR of products
        let f = Gf2mField::gf256();
        let a = FieldVec::from(vec![
            f.element(0x53),
            f.element(0xCA),
            f.element(0x01),
            f.element(0xFF),
        ]);
        let b = FieldVec::from(vec![
            f.element(0x12),
            f.element(0x34),
            f.element(0x56),
            f.element(0x78),
        ]);
        let dot = a.dot_product(&b);

        // Reference: element-wise multiply and XOR
        let manual = (f.element(0x53) * f.element(0x12))
            + (f.element(0xCA) * f.element(0x34))
            + (f.element(0x01) * f.element(0x56))
            + (f.element(0xFF) * f.element(0x78));
        assert_eq!(dot, manual);
    }

    #[test]
    fn test_dot_product_fp7_fast_path() {
        use crate::gfp::Fp;
        // For Fp<7>, kmax = u128::MAX / 36 ≈ 9.4e36 — effectively no chunking needed
        // for short vectors. This exercises the general-case path but with a huge kmax.
        let a = FieldVec::from(vec![Fp::<7>::new(6), Fp::<7>::new(6)]);
        let b = FieldVec::from(vec![Fp::<7>::new(6), Fp::<7>::new(6)]);
        // 6*6 + 6*6 = 36 + 36 = 72 ≡ 2 (mod 7)
        assert_eq!(a.dot_product(&b), Fp::<7>::new(2));
    }

    #[test]
    fn test_dot_product_length_one() {
        use crate::gfp::Fp;
        let a = FieldVec::from(vec![Fp::<7>::new(5)]);
        let b = FieldVec::from(vec![Fp::<7>::new(3)]);
        // 5*3 = 15 ≡ 1 (mod 7)
        assert_eq!(a.dot_product(&b), Fp::<7>::new(1));
    }

    #[test]
    fn test_dot_product_length_two() {
        use crate::gfp::Fp;
        let a = FieldVec::from(vec![Fp::<7>::new(3), Fp::<7>::new(4)]);
        let b = FieldVec::from(vec![Fp::<7>::new(2), Fp::<7>::new(5)]);
        // 3*2 + 4*5 = 6 + 20 = 26 ≡ 5 (mod 7)
        assert_eq!(a.dot_product(&b), Fp::<7>::new(5));
    }

    #[test]
    #[should_panic(expected = "vectors must not be empty")]
    fn test_dot_product_empty_fp_panics() {
        use crate::gfp::Fp;
        let a = FieldVec::<Fp<7>>::new();
        let b = FieldVec::<Fp<7>>::new();
        let _ = a.dot_product(&b);
    }

    #[test]
    fn test_dot_product_large_prime_max_residues() {
        use crate::gfp::Fp;
        // P near 2^63, use values close to P-1
        const P: u64 = 9_223_372_036_854_775_783;
        let p_minus_1 = Fp::<P>::new(P - 1);
        // Create vectors where all elements are P-1 (worst case for overflow)
        let a = FieldVec::from(vec![p_minus_1; 100]);
        let b = FieldVec::from(vec![p_minus_1; 100]);

        let result = a.dot_product(&b);

        // Verify against element-wise computation
        let expected: Fp<P> = (0..100).fold(Fp::<P>::zero(), |acc, _| acc + p_minus_1 * p_minus_1);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_dot_product_fp65521_long_vector() {
        use crate::gfp::Fp;
        let n = 10000;
        let a: FieldVec<Fp<65521>> = FieldVec::from(
            (1..=n)
                .map(|i| Fp::<65521>::new(i as u64 % 65521))
                .collect::<Vec<_>>(),
        );
        let b: FieldVec<Fp<65521>> = FieldVec::from(
            (1..=n)
                .map(|i| Fp::<65521>::new((i * 3 + 7) as u64 % 65521))
                .collect::<Vec<_>>(),
        );
        let result = a.dot_product(&b);
        // Verify against element-wise
        let expected: Fp<65521> = a
            .iter()
            .zip(b.iter())
            .fold(Fp::<65521>::zero(), |acc, (ai, bi)| acc + (*ai * *bi));
        assert_eq!(result, expected);
    }

    #[test]
    fn test_dot_product_large_prime_long_vector() {
        use crate::gfp::Fp;
        const P: u64 = 9_223_372_036_854_775_783;
        let n = 10000usize;
        let a: FieldVec<Fp<P>> = FieldVec::from(
            (0..n)
                .map(|i| Fp::<P>::new((i as u64 * 7 + 3) % P))
                .collect::<Vec<_>>(),
        );
        let b: FieldVec<Fp<P>> = FieldVec::from(
            (0..n)
                .map(|i| Fp::<P>::new((i as u64 * 13 + 11) % P))
                .collect::<Vec<_>>(),
        );
        let result = a.dot_product(&b);
        // Verify against element-wise
        let expected: Fp<P> = a
            .iter()
            .zip(b.iter())
            .fold(Fp::<P>::zero(), |acc, (ai, bi)| acc + (*ai * *bi));
        assert_eq!(result, expected);
    }

    #[test]
    fn test_dot_product_gf2m_long_vector() {
        let field = Gf2mField::new(8, 0x11B);
        let n = 10000;
        let a: FieldVec<Gf2mElement> = FieldVec::from(
            (0..n)
                .map(|i| field.element((i * 7 + 3) as u64 % 256))
                .collect::<Vec<_>>(),
        );
        let b: FieldVec<Gf2mElement> = FieldVec::from(
            (0..n)
                .map(|i| field.element((i * 13 + 11) as u64 % 256))
                .collect::<Vec<_>>(),
        );
        let result = a.dot_product(&b);
        let expected = a
            .iter()
            .zip(b.iter())
            .fold(field.zero(), |acc, (ai, bi)| acc + (ai.clone() * bi));
        assert_eq!(result, expected);
    }

    // ── Property-based tests ──────────────────────────────────────────────────

    proptest::proptest! {
        /// dot_product is bilinear: (a·x) · y = a * (x · y) for scalar a.
        #[test]
        fn prop_dot_product_scale_linear(
            raw_a in 1u32..15,
            xs in proptest::collection::vec(1u32..15, 2..500),
            ys in proptest::collection::vec(1u32..15, 2..500),
        ) {
            // Restrict to equal lengths
            let len = xs.len().min(ys.len());
            let f = Gf2mField::new(4, 0b10011);
            let a = f.element(raw_a as u64);
            let x: FieldVec<Gf2mElement> = xs[..len].iter().map(|&v| f.element(v as u64)).collect();
            let y: FieldVec<Gf2mElement> = ys[..len].iter().map(|&v| f.element(v as u64)).collect();

            // (a*x) · y == a * (x · y)  — bilinearity
            let ax = x.scale(&a);
            let lhs = ax.dot_product(&y);
            let rhs = a.clone() * x.dot_product(&y);
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// dot_product distributes over addition for Fp<7>:
        /// a · (b + c) == a · b + a · c.
        #[test]
        fn prop_dot_product_additive_linear_fp7(
            xs in proptest::collection::vec(0u64..7, 2..500),
            ys in proptest::collection::vec(0u64..7, 2..500),
            zs in proptest::collection::vec(0u64..7, 2..500),
        ) {
            use crate::gfp::Fp;
            let len = xs.len().min(ys.len()).min(zs.len());
            let a: FieldVec<Fp<7>> = xs[..len].iter().map(|&v| Fp::<7>::new(v)).collect();
            let b: FieldVec<Fp<7>> = ys[..len].iter().map(|&v| Fp::<7>::new(v)).collect();
            let c: FieldVec<Fp<7>> = zs[..len].iter().map(|&v| Fp::<7>::new(v)).collect();

            let b_plus_c = b.add_vec(&c);
            let lhs = a.dot_product(&b_plus_c);
            let rhs = a.dot_product(&b) + a.dot_product(&c);
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// dot_product scalar linearity for Fp<7>:
        /// (k * a) · b == k * (a · b).
        #[test]
        fn prop_dot_product_scale_linear_fp7(
            k_raw in 0u64..7,
            xs in proptest::collection::vec(0u64..7, 2..500),
            ys in proptest::collection::vec(0u64..7, 2..500),
        ) {
            use crate::gfp::Fp;
            let len = xs.len().min(ys.len());
            let k = Fp::<7>::new(k_raw);
            let a: FieldVec<Fp<7>> = xs[..len].iter().map(|&v| Fp::<7>::new(v)).collect();
            let b: FieldVec<Fp<7>> = ys[..len].iter().map(|&v| Fp::<7>::new(v)).collect();

            let lhs = a.scale(&k).dot_product(&b);
            let rhs = k * a.dot_product(&b);
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// dot_product distributes over addition for Fp<65521>:
        /// a · (b + c) == a · b + a · c.
        #[test]
        fn prop_dot_product_additive_linear_fp65521(
            xs in proptest::collection::vec(0u64..65521, 2..500),
            ys in proptest::collection::vec(0u64..65521, 2..500),
            zs in proptest::collection::vec(0u64..65521, 2..500),
        ) {
            use crate::gfp::Fp;
            let len = xs.len().min(ys.len()).min(zs.len());
            let a: FieldVec<Fp<65521>> = xs[..len].iter().map(|&v| Fp::<65521>::new(v)).collect();
            let b: FieldVec<Fp<65521>> = ys[..len].iter().map(|&v| Fp::<65521>::new(v)).collect();
            let c: FieldVec<Fp<65521>> = zs[..len].iter().map(|&v| Fp::<65521>::new(v)).collect();

            let b_plus_c = b.add_vec(&c);
            let lhs = a.dot_product(&b_plus_c);
            let rhs = a.dot_product(&b) + a.dot_product(&c);
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// dot_product scalar linearity for Fp<65521>:
        /// (k * a) · b == k * (a · b).
        #[test]
        fn prop_dot_product_scale_linear_fp65521(
            k_raw in 0u64..65521,
            xs in proptest::collection::vec(0u64..65521, 2..500),
            ys in proptest::collection::vec(0u64..65521, 2..500),
        ) {
            use crate::gfp::Fp;
            let len = xs.len().min(ys.len());
            let k = Fp::<65521>::new(k_raw);
            let a: FieldVec<Fp<65521>> = xs[..len].iter().map(|&v| Fp::<65521>::new(v)).collect();
            let b: FieldVec<Fp<65521>> = ys[..len].iter().map(|&v| Fp::<65521>::new(v)).collect();

            let lhs = a.scale(&k).dot_product(&b);
            let rhs = k * a.dot_product(&b);
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// dot_product commutativity for Fp<7>: a · b == b · a.
        #[test]
        fn prop_dot_product_commutative_fp7(
            xs in proptest::collection::vec(0u64..7, 2..500),
            ys in proptest::collection::vec(0u64..7, 2..500),
        ) {
            use crate::gfp::Fp;
            let len = xs.len().min(ys.len());
            let a: FieldVec<Fp<7>> = xs[..len].iter().map(|&v| Fp::<7>::new(v)).collect();
            let b: FieldVec<Fp<7>> = ys[..len].iter().map(|&v| Fp::<7>::new(v)).collect();
            proptest::prop_assert_eq!(a.dot_product(&b), b.dot_product(&a));
        }

        /// dot_product commutativity for Fp<65521>: a · b == b · a.
        #[test]
        fn prop_dot_product_commutative_fp65521(
            xs in proptest::collection::vec(0u64..65521, 2..500),
            ys in proptest::collection::vec(0u64..65521, 2..500),
        ) {
            use crate::gfp::Fp;
            let len = xs.len().min(ys.len());
            let a: FieldVec<Fp<65521>> = xs[..len].iter().map(|&v| Fp::<65521>::new(v)).collect();
            let b: FieldVec<Fp<65521>> = ys[..len].iter().map(|&v| Fp::<65521>::new(v)).collect();
            proptest::prop_assert_eq!(a.dot_product(&b), b.dot_product(&a));
        }

        /// dot_product commutativity for a large prime near 2^63.
        #[test]
        fn prop_dot_product_commutative_large_prime(
            vals_a in proptest::collection::vec(0u64..9_223_372_036_854_775_783u64, 2..500),
        ) {
            use crate::gfp::Fp;
            const P: u64 = 9_223_372_036_854_775_783;
            let a: FieldVec<Fp<P>> = FieldVec::from(
                vals_a.iter().map(|&v| Fp::<P>::new(v)).collect::<Vec<_>>(),
            );
            // Use reversed values for b to get different vectors
            let b: FieldVec<Fp<P>> = FieldVec::from(
                vals_a.iter().rev().map(|&v| Fp::<P>::new(v)).collect::<Vec<_>>(),
            );
            proptest::prop_assert_eq!(a.dot_product(&b), b.dot_product(&a));
        }

        /// dot_product additive bilinearity for a large prime near 2^63:
        /// a · (b + c) == a · b + a · c.
        #[test]
        fn prop_dot_product_additive_linear_large_prime(
            vals_a in proptest::collection::vec(0u64..100u64, 2..500),
            vals_b in proptest::collection::vec(0u64..100u64, 2..500),
            vals_c in proptest::collection::vec(0u64..100u64, 2..500),
        ) {
            use crate::gfp::Fp;
            const P: u64 = 9_223_372_036_854_775_783;
            let n = vals_a.len().min(vals_b.len()).min(vals_c.len());
            let a: FieldVec<Fp<P>> = FieldVec::from(vals_a[..n].iter().map(|&v| Fp::<P>::new(v)).collect::<Vec<_>>());
            let b: FieldVec<Fp<P>> = FieldVec::from(vals_b[..n].iter().map(|&v| Fp::<P>::new(v)).collect::<Vec<_>>());
            let c: FieldVec<Fp<P>> = FieldVec::from(vals_c[..n].iter().map(|&v| Fp::<P>::new(v)).collect::<Vec<_>>());
            // b + c element-wise
            let bc: FieldVec<Fp<P>> = FieldVec::from(
                b.iter().zip(c.iter()).map(|(bi, ci)| *bi + *ci).collect::<Vec<_>>()
            );
            let lhs = a.dot_product(&bc);
            let rhs = a.dot_product(&b) + a.dot_product(&c);
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// dot_product scalar bilinearity for a large prime near 2^63:
        /// (k * a) · b == k * (a · b).
        #[test]
        fn prop_dot_product_scale_linear_large_prime(
            k_raw in 0u64..100u64,
            xs in proptest::collection::vec(0u64..100u64, 2..500),
            ys in proptest::collection::vec(0u64..100u64, 2..500),
        ) {
            use crate::gfp::Fp;
            const P: u64 = 9_223_372_036_854_775_783;
            let len = xs.len().min(ys.len());
            let k = Fp::<P>::new(k_raw);
            let a: FieldVec<Fp<P>> = xs[..len].iter().map(|&v| Fp::<P>::new(v)).collect();
            let b: FieldVec<Fp<P>> = ys[..len].iter().map(|&v| Fp::<P>::new(v)).collect();

            let lhs = a.scale(&k).dot_product(&b);
            let rhs = k * a.dot_product(&b);
            proptest::prop_assert_eq!(lhs, rhs);
        }

        /// axpy correctness: y after axpy equals element-wise y[i] + a*x[i].
        #[test]
        fn prop_axpy_matches_manual(
            raw_a in 0u32..15,
            xs in proptest::collection::vec(0u32..15, 2..500),
            ys in proptest::collection::vec(0u32..15, 2..500),
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
            elems in proptest::collection::vec(0u32..15, 3..500),
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

        /// simd_dot_product commutativity for random GF(2^8) vectors.
        #[test]
        fn prop_simd_dot_product_commutative_gf256(
            xs in proptest::collection::vec(0u64..256, 2..500),
            ys in proptest::collection::vec(0u64..256, 2..500),
        ) {
            let f = Gf2mField::gf256();
            let len = xs.len().min(ys.len());
            let a: FieldVec<Gf2mElement> = xs[..len].iter().map(|&v| f.element(v)).collect();
            let b: FieldVec<Gf2mElement> = ys[..len].iter().map(|&v| f.element(v)).collect();
            proptest::prop_assert_eq!(a.simd_dot_product(&b), b.simd_dot_product(&a));
        }
    }

    // ── SIMD dot product tests ──────────────────────────────────────────────

    #[test]
    fn test_simd_dot_product_matches_scalar_all_m() {
        use crate::primitive_polys::PrimitivePolynomialDatabase;

        for m in 2..=16usize {
            let poly = PrimitivePolynomialDatabase::standard(m).unwrap();
            let f = Gf2mField::new(m, poly);
            let order = 1u64 << m;

            for &n in &[1, 2, 100, 1000] {
                let a_vals: Vec<Gf2mElement> =
                    (0..n).map(|i| f.element((i * 37 + 13) % order)).collect();
                let b_vals: Vec<Gf2mElement> =
                    (0..n).map(|i| f.element((i * 53 + 7) % order)).collect();

                let a = FieldVec::from(a_vals);
                let b = FieldVec::from(b_vals);

                let scalar = a.dot_product(&b);
                let simd = a.simd_dot_product(&b);
                assert_eq!(
                    scalar, simd,
                    "SIMD/scalar mismatch for GF(2^{m}) at n={n}: scalar={:?}, simd={:?}",
                    scalar, simd
                );
            }
        }
    }

    #[test]
    fn test_simd_dot_product_matches_scalar_gf256_1000() {
        let f = Gf2mField::gf256();
        let a_vals: Vec<Gf2mElement> = (0..1000).map(|i| f.element((i * 37 + 13) % 256)).collect();
        let b_vals: Vec<Gf2mElement> = (0..1000).map(|i| f.element((i * 53 + 7) % 256)).collect();

        let a = FieldVec::from(a_vals);
        let b = FieldVec::from(b_vals);

        assert_eq!(a.dot_product(&b), a.simd_dot_product(&b));
    }

    #[test]
    fn test_simd_dot_product_length_one() {
        let f = Gf2mField::gf256();
        let a = FieldVec::from(vec![f.element(0x53)]);
        let b = FieldVec::from(vec![f.element(0xCA)]);
        assert_eq!(a.dot_product(&b), a.simd_dot_product(&b));
    }

    #[test]
    fn test_simd_dot_product_length_two() {
        let f = Gf2mField::gf256();
        let a = FieldVec::from(vec![f.element(0x53), f.element(0xCA)]);
        let b = FieldVec::from(vec![f.element(0x12), f.element(0x34)]);
        assert_eq!(a.dot_product(&b), a.simd_dot_product(&b));
    }

    // ── FieldVec<Fp<65537>> SIMD-dispatch tests ────────────────────────────
    //
    // `mul_vec`/`add_vec`/`sub_vec` transparently route through the AVX2
    // kernel via `SimdVecOps`. These tests verify the unified surface
    // produces the same result as a hand-rolled scalar reference loop
    // (exercising the boundary and overflow cases that distinguish the
    // SIMD path). When the `simd` feature is off or AVX2 is unavailable,
    // the fallback arm runs; the tests are written so either path yields
    // the same answer.

    fn fp65537_scalar_mul_vec(a: &[u64], b: &[u64]) -> Vec<u64> {
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x * y) % 65537)
            .collect()
    }

    fn fp65537_scalar_add_vec(a: &[u64], b: &[u64]) -> Vec<u64> {
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x + y) % 65537)
            .collect()
    }

    fn fp65537_scalar_sub_vec(a: &[u64], b: &[u64]) -> Vec<u64> {
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x + 65537 - y) % 65537)
            .collect()
    }

    #[test]
    fn test_mul_vec_fp65537_matches_scalar_reference() {
        use crate::gfp::Fp;
        for &n in &[0usize, 1, 7, 8, 9, 16, 17, 100, 1000] {
            let xs: Vec<u64> = (0..n as u64).map(|i| (i * 12345) % 65537).collect();
            let ys: Vec<u64> = (0..n as u64).map(|i| (i * 67890 + 7) % 65537).collect();
            let a: FieldVec<Fp<65537>> = xs.iter().map(|&v| Fp::<65537>::new(v)).collect();
            let b: FieldVec<Fp<65537>> = ys.iter().map(|&v| Fp::<65537>::new(v)).collect();
            let out = a.mul_vec(&b);
            let expected = fp65537_scalar_mul_vec(&xs, &ys);
            for (i, e) in expected.iter().enumerate() {
                assert_eq!(out[i].value(), *e, "mul_vec n={n} i={i}");
            }
        }
    }

    #[test]
    fn test_mul_vec_fp65537_boundary_values() {
        use crate::gfp::Fp;
        // 0, 1, p-1 = 65536, p/2 = 32768, saturation case 65536*65536
        let raw = [0u64, 1, 65536, 32768, 1, 65536, 0, 65535, 65536];
        let rev: Vec<u64> = raw.iter().rev().copied().collect();
        let a: FieldVec<Fp<65537>> = raw.iter().map(|&v| Fp::<65537>::new(v)).collect();
        let b: FieldVec<Fp<65537>> = rev.iter().map(|&v| Fp::<65537>::new(v)).collect();
        let out = a.mul_vec(&b);
        let expected = fp65537_scalar_mul_vec(&raw, &rev);
        for (i, e) in expected.iter().enumerate() {
            assert_eq!(out[i].value(), *e, "boundary mul i={i}");
        }
    }

    #[test]
    fn test_add_vec_fp65537_matches_scalar_reference() {
        use crate::gfp::Fp;
        for &n in &[0usize, 1, 8, 9, 100, 1000] {
            let xs: Vec<u64> = (0..n as u64).map(|i| (i * 4093) % 65537).collect();
            let ys: Vec<u64> = (0..n as u64).map(|i| (i * 9973) % 65537).collect();
            let a: FieldVec<Fp<65537>> = xs.iter().map(|&v| Fp::<65537>::new(v)).collect();
            let b: FieldVec<Fp<65537>> = ys.iter().map(|&v| Fp::<65537>::new(v)).collect();
            let out = a.add_vec(&b);
            let expected = fp65537_scalar_add_vec(&xs, &ys);
            for (i, e) in expected.iter().enumerate() {
                assert_eq!(out[i].value(), *e, "add_vec n={n} i={i}");
            }
        }
    }

    #[test]
    fn test_sub_vec_fp65537_matches_scalar_reference() {
        use crate::gfp::Fp;
        for &n in &[0usize, 1, 8, 9, 100, 1000] {
            let xs: Vec<u64> = (0..n as u64).map(|i| (i * 4093) % 65537).collect();
            let ys: Vec<u64> = (0..n as u64).map(|i| (i * 9973) % 65537).collect();
            let a: FieldVec<Fp<65537>> = xs.iter().map(|&v| Fp::<65537>::new(v)).collect();
            let b: FieldVec<Fp<65537>> = ys.iter().map(|&v| Fp::<65537>::new(v)).collect();
            let out = a.sub_vec(&b);
            let expected = fp65537_scalar_sub_vec(&xs, &ys);
            for (i, e) in expected.iter().enumerate() {
                assert_eq!(out[i].value(), *e, "sub_vec n={n} i={i}");
            }
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]

        #[test]
        fn prop_mul_vec_fp65537_matches_scalar(
            xs in proptest::collection::vec(0u64..65537, 1..=1024),
            ys in proptest::collection::vec(0u64..65537, 1..=1024),
        ) {
            use crate::gfp::Fp;
            let n = xs.len().min(ys.len());
            let a: FieldVec<Fp<65537>> =
                xs[..n].iter().map(|&v| Fp::<65537>::new(v)).collect();
            let b: FieldVec<Fp<65537>> =
                ys[..n].iter().map(|&v| Fp::<65537>::new(v)).collect();
            let out = a.mul_vec(&b);
            let expected = fp65537_scalar_mul_vec(&xs[..n], &ys[..n]);
            for (i, e) in expected.iter().enumerate() {
                proptest::prop_assert_eq!(out[i].value(), *e);
            }
        }

        #[test]
        fn prop_add_vec_fp65537_matches_scalar(
            xs in proptest::collection::vec(0u64..65537, 1..=1024),
            ys in proptest::collection::vec(0u64..65537, 1..=1024),
        ) {
            use crate::gfp::Fp;
            let n = xs.len().min(ys.len());
            let a: FieldVec<Fp<65537>> =
                xs[..n].iter().map(|&v| Fp::<65537>::new(v)).collect();
            let b: FieldVec<Fp<65537>> =
                ys[..n].iter().map(|&v| Fp::<65537>::new(v)).collect();
            let out = a.add_vec(&b);
            let expected = fp65537_scalar_add_vec(&xs[..n], &ys[..n]);
            for (i, e) in expected.iter().enumerate() {
                proptest::prop_assert_eq!(out[i].value(), *e);
            }
        }

        #[test]
        fn prop_sub_vec_fp65537_matches_scalar(
            xs in proptest::collection::vec(0u64..65537, 1..=1024),
            ys in proptest::collection::vec(0u64..65537, 1..=1024),
        ) {
            use crate::gfp::Fp;
            let n = xs.len().min(ys.len());
            let a: FieldVec<Fp<65537>> =
                xs[..n].iter().map(|&v| Fp::<65537>::new(v)).collect();
            let b: FieldVec<Fp<65537>> =
                ys[..n].iter().map(|&v| Fp::<65537>::new(v)).collect();
            let out = a.sub_vec(&b);
            let expected = fp65537_scalar_sub_vec(&xs[..n], &ys[..n]);
            for (i, e) in expected.iter().enumerate() {
                proptest::prop_assert_eq!(out[i].value(), *e);
            }
        }
    }

    /// Verifies the `simd_dot_product` fallback path returns the correct scalar
    /// result. When the `simd` feature is disabled, `try_simd_dot_product` always
    /// returns `None` and `simd_dot_product` falls back to `dot_product`. When the
    /// `simd` feature is enabled but PCLMULQDQ is unavailable at runtime, the same
    /// fallback triggers because `clmul_batch_fn()` returns `None`.
    ///
    /// This test runs on both feature configurations: with `simd` it validates the
    /// end-to-end result (SIMD or fallback, whichever the hardware picks), without
    /// `simd` it exercises the fallback path directly.
    #[test]
    fn test_simd_dot_product_fallback_correctness() {
        let f = Gf2mField::new(4, 0b10011); // GF(2^4), x^4 + x + 1
        let a = FieldVec::from(vec![
            f.element(0x3),
            f.element(0x7),
            f.element(0xA),
            f.element(0xF),
        ]);
        let b = FieldVec::from(vec![
            f.element(0x5),
            f.element(0x2),
            f.element(0xC),
            f.element(0x1),
        ]);

        let scalar = a.dot_product(&b);
        let simd = a.simd_dot_product(&b);

        assert_eq!(
            scalar, simd,
            "simd_dot_product must match dot_product (fallback or SIMD): \
             scalar={scalar:?}, simd={simd:?}"
        );

        // Also verify against a manually computed value to ensure the scalar
        // path itself is correct: sum of pairwise GF(2^4) products.
        let expected = f.element(0x3) * f.element(0x5)
            + f.element(0x7) * f.element(0x2)
            + f.element(0xA) * f.element(0xC)
            + f.element(0xF) * f.element(0x1);
        assert_eq!(
            scalar, expected,
            "dot_product must match hand-computed result: scalar={scalar:?}, expected={expected:?}"
        );
    }
}
