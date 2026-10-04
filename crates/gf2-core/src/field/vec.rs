//! Dense vector of finite field elements with arithmetic operations.
//!
//! This module provides [`FieldVec<F>`], a newtype wrapper around `Vec<F>` for any
//! type implementing [`FiniteField`], together with [`StridedIter`] for column access
//! in row-major matrix layouts.

use crate::field::{ConstField, FiniteField};
use std::ops::Index;
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicUsize, Ordering};

/// Default for the tuning profile's `field_vec.dot_chunk_len` field: the
/// elements per chunk of the SIMD dot-product walk.
///
/// The chunk sizes three stack scratch buffers (`u64`, `u64`, `u128`), 8 KiB
/// at 256 elements, so it is a compile-time constant.
pub(crate) const DOT_CHUNK_LEN: usize = 256;

/// The `field_vec.dot_chunk_len` value `simd_dot_product` uses as its
/// stack-buffer length and walk step.
///
/// Baked because only a compile-time constant can size a stack array. The
/// default build resolves it to [`DOT_CHUNK_LEN`];
/// `RUSTFLAGS="--cfg gf2_tuning_baked"` resolves it to
/// `crate::tuning::baked::DOT_CHUNK_LEN`. Installing a runtime profile does
/// not move this length.
#[cfg(all(feature = "simd", gf2_tuning_baked))]
const DOT_CHUNK_LEN_SELECTED: usize = crate::tuning::baked::DOT_CHUNK_LEN;

/// The `field_vec.dot_chunk_len` value `simd_dot_product` uses as its
/// stack-buffer length and walk step; see the baked arm for the mechanism.
#[cfg(all(feature = "simd", not(gf2_tuning_baked)))]
const DOT_CHUNK_LEN_SELECTED: usize = DOT_CHUNK_LEN;

#[cfg(not(feature = "simd"))]
const DOT_CHUNK_LEN_SELECTED: usize = DOT_CHUNK_LEN;

/// Returns the compile-time dot-product chunk selected by this build.
///
/// Calibration evidence uses this test-support seam to distinguish the
/// ordinary or baked compiled constant from an installed tuning candidate.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[must_use]
pub const fn selected_dot_chunk_len() -> usize {
    DOT_CHUNK_LEN_SELECTED
}

/// Widest chunk any SIMD dot-product walk has filled since the last reset.
#[cfg(any(test, feature = "test-support"))]
static MAX_EFFECTIVE_DOT_CHUNK_LEN: AtomicUsize = AtomicUsize::new(0);

/// Clears the test-support observation of the SIMD dot-product walk's chunk.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_max_effective_dot_chunk_len() {
    MAX_EFFECTIVE_DOT_CHUNK_LEN.store(0, Ordering::Relaxed);
}

/// Returns the widest chunk the SIMD dot-product walk has filled since the
/// last [`reset_max_effective_dot_chunk_len`].
///
/// This observation is available in tests and `test-support` builds so the
/// baked witness can verify that `field_vec.dot_chunk_len()` reaches the walk
/// rather than merely producing an equal result. A walk over `n` elements
/// fills chunks of `min(step, n - offset)` elements, so the widest chunk of a
/// walk with `n` above the step is the step itself. It stays `0` when the
/// host offers no SIMD carry-less multiply and the walk never runs.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn max_effective_dot_chunk_len() -> usize {
    MAX_EFFECTIVE_DOT_CHUNK_LEN.load(Ordering::Relaxed)
}

/// A dense vector of finite field elements.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldVec<F: FiniteField> {
    data: Vec<F>,
}

impl<F: FiniteField> FieldVec<F> {
    /// Creates an empty `FieldVec`.
    pub fn new() -> Self {
        FieldVec { data: Vec::new() }
    }

    /// Creates a `FieldVec` of length `n` filled with `zero.zero_like()`.
    ///
    /// Use this for fields whose zero element is only known at runtime (e.g. `Gf2mElement`).
    /// For [`ConstField`] types, prefer [`FieldVec::zeros`] which requires no argument.
    pub fn zeros_from(n: usize, zero: &F) -> Self {
        FieldVec {
            data: (0..n).map(|_| zero.zero_like()).collect(),
        }
    }

    /// Creates a `FieldVec` with capacity for `n` elements but length zero.
    pub fn with_capacity(n: usize) -> Self {
        FieldVec {
            data: Vec::with_capacity(n),
        }
    }
}

impl<F: ConstField> FieldVec<F> {
    /// Creates a `FieldVec` of length `n` filled with `F::zero()`.
    pub fn zeros(n: usize) -> Self {
        FieldVec {
            data: vec![F::zero(); n],
        }
    }
}

impl<F: FiniteField> Default for FieldVec<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: FiniteField> FieldVec<F> {
    /// Returns a reference to the element at index `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    pub fn get(&self, i: usize) -> &F {
        &self.data[i]
    }

    /// Replaces the element at index `i` with `val`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    pub fn set(&mut self, i: usize, val: F) {
        self.data[i] = val;
    }

    /// Appends `val` to the end of the vector.
    pub fn push(&mut self, val: F) {
        self.data.push(val);
    }

    /// Returns the elements as a slice.
    pub fn as_slice(&self) -> &[F] {
        &self.data
    }

    /// Returns the elements as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [F] {
        &mut self.data
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the vector contains no elements.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<F: FiniteField> Index<usize> for FieldVec<F> {
    type Output = F;

    fn index(&self, i: usize) -> &F {
        &self.data[i]
    }
}

impl<F: FiniteField> FieldVec<F> {
    /// Returns an iterator over shared references to elements.
    pub fn iter(&self) -> std::slice::Iter<'_, F> {
        self.data.iter()
    }

    /// Returns an iterator over mutable references to elements.
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

impl<F: FiniteField> FieldVec<F> {
    /// Computes `∑ self[i] * rhs[i]` with delayed reduction.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()` or either vector is empty.
    ///
    /// # Complexity
    ///
    /// O(n) multiplications and `⌈n / kmax⌉` reductions, where
    /// `kmax = F::max_unreduced_additions()`.
    pub fn dot_product(&self, rhs: &Self) -> F {
        assert_eq!(
            self.len(),
            rhs.len(),
            "dot_product: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        assert!(!self.is_empty(), "dot_product: vectors must not be empty");
        if let Some(value) = F::try_simd_dot_product(&self.data, &rhs.data) {
            return value;
        }
        let zero = self.data[0].zero_like();
        dot_product_slices(&self.data, &rhs.data, &zero)
    }
}

/// Dot product of two equal-length slices with delayed reduction.
///
/// Accumulates at most
/// [`FiniteField::max_unreduced_additions`](crate::field::FiniteField::max_unreduced_additions)
/// wide products before reducing (`@/citation/DumasPernet2012` §1.2,
/// theorem 4 classical case), so the `Wide` accumulator does not overflow.
/// `zero.zero_like()` is the result for empty input and seeds the chunked
/// reductions.
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
        // At most kmax wide products per chunk (`@/citation/DumasPernet2012` §1.2).
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
    pub fn scale(&self, a: &F) -> Self {
        FieldVec {
            data: self.data.iter().map(|e| e.clone() * a.clone()).collect(),
        }
    }

    /// In-place fused multiply-add: `self[i] += a * rhs[i]` for all `i`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    pub fn axpy(&mut self, a: &F, rhs: &Self) {
        assert_eq!(
            self.len(),
            rhs.len(),
            "axpy: length mismatch ({} vs {})",
            self.len(),
            rhs.len()
        );
        if F::try_simd_axpy(self.data.as_mut_slice(), a, rhs.data.as_slice()) {
            return;
        }
        for (y, x) in self.data.iter_mut().zip(rhs.data.iter()) {
            *y += a.clone() * x.clone();
        }
    }
}

impl<F: FiniteField + SimdVecOps> FieldVec<F> {
    /// Returns `self[i] + rhs[i]` element-wise.
    ///
    /// Dispatches through [`SimdVecOps::try_simd_add_vec`] and falls back to
    /// the scalar loop when the hook declines.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
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
    /// Dispatches through [`SimdVecOps::try_simd_sub_vec`] and falls back to
    /// the scalar loop when the hook declines.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
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
    /// Dispatches through [`SimdVecOps::try_simd_mul_vec`] and falls back to
    /// the scalar loop when the hook declines.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
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

impl<F: FiniteField> FieldVec<F> {
    /// Applies `f` to each element, returning a new `FieldVec<G>`.
    pub fn map<G: FiniteField, Map: FnMut(&F) -> G>(&self, f: Map) -> FieldVec<G> {
        FieldVec {
            data: self.data.iter().map(f).collect(),
        }
    }

    /// Reduces the vector to a single value by applying `f` left-to-right.
    pub fn fold<B, Func: FnMut(B, &F) -> B>(&self, init: B, f: Func) -> B {
        self.data.iter().fold(init, f)
    }

    /// Combines two equal-length vectors element-wise using `f`, returning a new `FieldVec<G>`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
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

/// Iterator that steps through a slice with a fixed stride.
///
/// Useful for column access in row-major matrix layouts stored as flat slices.
pub struct StridedIter<'a, F> {
    slice: &'a [F],
    pos: usize,
    stride: usize,
}

impl<'a, F> StridedIter<'a, F> {
    /// Creates a `StridedIter` starting at index `start`, advancing by `stride` each step.
    ///
    /// # Panics
    ///
    /// Panics if `stride == 0`.
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

pub use crate::gfp::SimdVecOps;

use crate::gf2m::Gf2mElement;

impl FieldVec<Gf2mElement> {
    /// Dot product for GF(2^m) through the batched carry-less multiply
    /// kernel: all 128-bit products are XORed into one accumulator, which is
    /// Barrett-reduced once.
    ///
    /// Falls back to [`dot_product`](FieldVec::dot_product) without the
    /// `simd` feature or when the kernel is unavailable at runtime.
    ///
    /// # Panics
    ///
    /// Panics if the vectors have different lengths or are empty.
    pub fn simd_dot_product(&self, rhs: &Self) -> Gf2mElement {
        self.simd_dot_product_chunked::<DOT_CHUNK_LEN_SELECTED>(rhs)
    }

    fn simd_dot_product_chunked<const CHUNK: usize>(&self, rhs: &Self) -> Gf2mElement {
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

        self.try_simd_dot_product_chunked::<CHUNK>(rhs)
            .unwrap_or_else(|| self.dot_product(rhs))
    }

    /// Returns `None` when the batch kernel is unavailable. Chunks of `CHUNK`
    /// elements keep the scratch buffers on the stack.
    #[cfg(feature = "simd")]
    fn try_simd_dot_product_chunked<const CHUNK: usize>(&self, rhs: &Self) -> Option<Gf2mElement> {
        debug_assert!(CHUNK > 0, "SIMD dot-product chunk must be non-zero");
        let sample = &self.data[0];
        let batch_fn = sample.clmul_batch_fn()?;
        let clmul_fn = sample.clmul_fn()?;
        let reducer = sample.barrett_reducer()?;

        let mut a_buf = [0u64; CHUNK];
        let mut b_buf = [0u64; CHUNK];
        let mut p_buf = [0u128; CHUNK];

        let mut acc: u128 = 0;
        let mut offset = 0;
        let n = self.len();
        let mut widest_executed_chunk = 0usize;

        while offset < n {
            let end = (offset + CHUNK).min(n);
            let chunk_len = end - offset;
            widest_executed_chunk = widest_executed_chunk.max(chunk_len);

            for (i, (a, b)) in self.data[offset..end]
                .iter()
                .zip(&rhs.data[offset..end])
                .enumerate()
            {
                a_buf[i] = a.value();
                b_buf[i] = b.value();
            }

            batch_fn(
                &a_buf[..chunk_len],
                &b_buf[..chunk_len],
                &mut p_buf[..chunk_len],
            );

            for &p in &p_buf[..chunk_len] {
                acc ^= p;
            }

            offset = end;
        }

        #[cfg(any(test, feature = "test-support"))]
        MAX_EFFECTIVE_DOT_CHUNK_LEN.fetch_max(widest_executed_chunk, Ordering::Relaxed);

        let result = reducer.reduce_with_clmul(acc, clmul_fn);

        Some(sample.with_raw_value(result))
    }

    #[cfg(not(feature = "simd"))]
    fn try_simd_dot_product_chunked<const CHUNK: usize>(&self, _rhs: &Self) -> Option<Gf2mElement> {
        let _ = CHUNK;
        None
    }
}

/// One admissible compile-time SIMD dot-product chunk for calibration.
///
/// Accessors and function resolution are constant-time and do not panic. The
/// returned function inherits [`FieldVec::simd_dot_product`]'s length,
/// non-empty-input, fallback, and linear-time contracts.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DotChunkCandidate {
    /// 128 elements (4 KiB of stack scratch).
    C128,
    /// 256 elements (8 KiB of stack scratch).
    C256,
    /// 512 elements (16 KiB of stack scratch).
    C512,
}

#[cfg(any(test, feature = "test-support"))]
impl DotChunkCandidate {
    /// All admissible chunks in protocol order.
    pub const ALL: [Self; 3] = [Self::C128, Self::C256, Self::C512];

    /// Returns the chunk length.
    #[must_use]
    pub const fn chunk_len(self) -> usize {
        match self {
            Self::C128 => 128,
            Self::C256 => 256,
            Self::C512 => 512,
        }
    }

    /// Resolves this candidate to the shared production SIMD dot body.
    pub fn function(self) -> DotChunkCandidateFn {
        match self {
            Self::C128 => simd_dot_candidate::<128>,
            Self::C256 => simd_dot_candidate::<256>,
            Self::C512 => simd_dot_candidate::<512>,
        }
    }
}

/// Function-pointer type for a monomorphized SIMD dot candidate.
#[cfg(any(test, feature = "test-support"))]
pub type DotChunkCandidateFn = fn(&FieldVec<Gf2mElement>, &FieldVec<Gf2mElement>) -> Gf2mElement;

#[cfg(any(test, feature = "test-support"))]
fn simd_dot_candidate<const CHUNK: usize>(
    lhs: &FieldVec<Gf2mElement>,
    rhs: &FieldVec<Gf2mElement>,
) -> Gf2mElement {
    lhs.simd_dot_product_chunked::<CHUNK>(rhs)
}

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
        let col: Vec<_> = StridedIter::new(&data, 0, 2).collect();
        assert_eq!(col, vec![&f.element(0), &f.element(2), &f.element(4)]);

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

    #[test]
    fn test_dot_product_fp7_matches_elementwise() {
        use crate::gfp::Fp;
        // [3, 5, 6] · [2, 4, 1] = 3*2 + 5*4 + 6*1 = 6 + 20 + 6 = 32 ≡ 4 (mod 7)
        let a = FieldVec::from(vec![Fp::<7>::new(3), Fp::<7>::new(5), Fp::<7>::new(6)]);
        let b = FieldVec::from(vec![Fp::<7>::new(2), Fp::<7>::new(4), Fp::<7>::new(1)]);
        let dot = a.dot_product(&b);

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
        // P near 2^63: kmax = u128::MAX / (P-1)^2 ≈ 4, so n = 100 spans many chunks.
        const P: u64 = 9_223_372_036_854_775_783; // largest prime <= 2^63
        let n = 100;
        let a: FieldVec<Fp<P>> = (0..n).map(|i| Fp::<P>::new(i + 1)).collect();
        let b: FieldVec<Fp<P>> = (0..n).map(|i| Fp::<P>::new(i + 100)).collect();
        let dot = a.dot_product(&b);

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

        let manual = (f.element(0x53) * f.element(0x12))
            + (f.element(0xCA) * f.element(0x34))
            + (f.element(0x01) * f.element(0x56))
            + (f.element(0xFF) * f.element(0x78));
        assert_eq!(dot, manual);
    }

    #[test]
    fn test_dot_product_fp7_fast_path() {
        use crate::gfp::Fp;
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
        const P: u64 = 9_223_372_036_854_775_783;
        let p_minus_1 = Fp::<P>::new(P - 1);
        // P-1 everywhere maximises every wide product.
        let a = FieldVec::from(vec![p_minus_1; 100]);
        let b = FieldVec::from(vec![p_minus_1; 100]);

        let result = a.dot_product(&b);

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

    proptest::proptest! {
        #[test]
        fn prop_dot_product_scale_linear(
            raw_a in 1u32..15,
            xs in proptest::collection::vec(1u32..15, 2..500),
            ys in proptest::collection::vec(1u32..15, 2..500),
        ) {
            let len = xs.len().min(ys.len());
            let f = Gf2mField::new(4, 0b10011);
            let a = f.element(raw_a as u64);
            let x: FieldVec<Gf2mElement> = xs[..len].iter().map(|&v| f.element(v as u64)).collect();
            let y: FieldVec<Gf2mElement> = ys[..len].iter().map(|&v| f.element(v as u64)).collect();

            let ax = x.scale(&a);
            let lhs = ax.dot_product(&y);
            let rhs = a.clone() * x.dot_product(&y);
            proptest::prop_assert_eq!(lhs, rhs);
        }

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

        #[test]
        fn prop_dot_product_commutative_large_prime(
            vals_a in proptest::collection::vec(0u64..9_223_372_036_854_775_783u64, 2..500),
        ) {
            use crate::gfp::Fp;
            const P: u64 = 9_223_372_036_854_775_783;
            let a: FieldVec<Fp<P>> = FieldVec::from(
                vals_a.iter().map(|&v| Fp::<P>::new(v)).collect::<Vec<_>>(),
            );
            let b: FieldVec<Fp<P>> = FieldVec::from(
                vals_a.iter().rev().map(|&v| Fp::<P>::new(v)).collect::<Vec<_>>(),
            );
            proptest::prop_assert_eq!(a.dot_product(&b), b.dot_product(&a));
        }

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
            let bc: FieldVec<Fp<P>> = FieldVec::from(
                b.iter().zip(c.iter()).map(|(bi, ci)| *bi + *ci).collect::<Vec<_>>()
            );
            let lhs = a.dot_product(&bc);
            let rhs = a.dot_product(&b) + a.dot_product(&c);
            proptest::prop_assert_eq!(lhs, rhs);
        }

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
            proptest::prop_assert_eq!(a.simd_dot_product(&b), a.dot_product(&b));
        }
    }

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

    #[test]
    #[should_panic(expected = "vectors must not be empty")]
    fn simd_dot_product_empty_panics() {
        let empty = FieldVec::<Gf2mElement>::new();
        empty.simd_dot_product(&empty);
    }

    /// The lengths cover 1, 2, odd tails, the 63/64/65 word boundary and the
    /// 255/256/257 boundary of the default `DOT_CHUNK_LEN`; GF(2^32) carries
    /// operand bits above bit 15.
    #[test]
    fn simd_dot_product_matches_scalar_at_boundaries() {
        use crate::primitive_polys::PrimitivePolynomialDatabase;

        let lengths = [
            1usize, 2, 3, 5, 63, 64, 65, 127, 128, 129, 255, 256, 257, 513,
        ];
        for m in [8usize, 16, 32] {
            let poly = PrimitivePolynomialDatabase::standard(m)
                .expect("standard primitive polynomial for this degree");
            let f = Gf2mField::new(m, poly);
            let mask = if m == 64 { u64::MAX } else { (1u64 << m) - 1 };
            let palette = [
                0u64,
                1,
                1u64 << (m - 1),
                mask,
                0xAAAA_AAAA_AAAA_AAAA & mask,
                0x5555_5555_5555_5555 & mask,
            ];
            for n in lengths {
                let a: FieldVec<Gf2mElement> = (0..n)
                    .map(|i| f.element(palette[i % palette.len()]))
                    .collect();
                let b: FieldVec<Gf2mElement> = (0..n)
                    .map(|i| f.element(palette[(i * 5 + 2) % palette.len()]))
                    .collect();
                assert_eq!(
                    a.simd_dot_product(&b),
                    a.dot_product(&b),
                    "GF(2^{m}) dot product differs from scalar at n={n}"
                );
            }
        }
    }

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

    /// Reaches the fallback only without the `simd` feature or without a
    /// runtime batch kernel; otherwise the kernel path runs.
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
