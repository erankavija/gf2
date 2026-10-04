//! Structure-of-Arrays batch layout for GF(p^n) extension fields:
//! [`BatchExtField<F, N>`] stores coefficient `i` of every element in one
//! contiguous buffer, so extension-field arithmetic runs as base-field passes
//! over slices. The quadratic and cubic products dispatch through
//! [`SimdKaratsubaHook`].

use std::array;

use crate::field::{ConstField, FiniteField};
use crate::gfp::{Fp, SimdVecOps};
use crate::gfpn::{CubicExt, ExtConfig, QuadraticExt};

#[inline]
fn ext_non_residue<C: ExtConfig>() -> C::BaseField {
    #[cfg(not(verify_lean))]
    {
        C::NON_RESIDUE
    }
    #[cfg(verify_lean)]
    {
        C::NON_RESIDUE()
    }
}

/// Batch of GF(p^n) extension-field elements stored in Structure-of-Arrays
/// layout.
///
/// `coeffs[i]` holds the `i`-th coefficient of every element in the batch.
/// All inner vectors have the same length, the *batch size* reported by
/// [`BatchExtField::len`]; [`BatchExtField::new`] enforces this. `F` is the
/// coefficient (base) field and `N` the number of coefficients per element
/// (the extension degree; `N = 1` for base-field batches).
#[derive(Clone, Debug)]
pub struct BatchExtField<F: FiniteField, const N: usize> {
    coeffs: [Vec<F>; N],
}

impl<F: FiniteField, const N: usize> BatchExtField<F, N> {
    /// Creates a new `BatchExtField` from `N` equal-length coefficient vectors.
    ///
    /// # Panics
    ///
    /// Panics if the inner vectors differ in length.
    pub fn new(coeffs: [Vec<F>; N]) -> Self {
        if N > 0 {
            let expected = coeffs[0].len();
            for (i, lane) in coeffs.iter().enumerate().skip(1) {
                assert_eq!(
                    lane.len(),
                    expected,
                    "BatchExtField::new: coefficient vector {i} has length {} but lane 0 has length {expected}",
                    lane.len(),
                );
            }
        }
        Self { coeffs }
    }

    /// Constructs a batch of `len` extension-field zeros.
    ///
    /// `sample` is any base-field element; only its
    /// [`FiniteField::zero_like`] is consulted, which supports base fields
    /// whose identity element depends on runtime configuration.
    pub fn zeros(len: usize, sample: &F) -> Self {
        let coeffs: [Vec<F>; N] =
            array::from_fn(|_| (0..len).map(|_| sample.zero_like()).collect());
        Self { coeffs }
    }

    /// Returns the batch size (length of each coefficient vector).
    pub fn len(&self) -> usize {
        if N == 0 {
            0
        } else {
            self.coeffs[0].len()
        }
    }

    /// Returns `true` if the batch contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the `i`-th coefficient lane, a contiguous slice of length
    /// [`Self::len`].
    ///
    /// # Panics
    ///
    /// Panics if `i >= N`.
    pub fn coeff(&self, i: usize) -> &[F] {
        &self.coeffs[i]
    }

    /// Returns `true` if all coefficient lanes have the same length, the
    /// representation invariant [`BatchExtField::new`] enforces.
    pub fn is_valid(&self) -> bool {
        if N == 0 {
            return true;
        }
        let expected = self.coeffs[0].len();
        self.coeffs.iter().all(|lane| lane.len() == expected)
    }
}

impl<F: FiniteField, const N: usize> BatchExtField<F, N> {
    /// Element-wise addition across the batch.
    ///
    /// Returns a new batch whose `i`-th element equals `self[i] + other[i]`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    pub fn batch_add(&self, other: &Self) -> Self {
        assert_eq!(
            self.len(),
            other.len(),
            "batch_add: length mismatch ({} vs {})",
            self.len(),
            other.len()
        );
        let coeffs: [Vec<F>; N] = array::from_fn(|i| {
            self.coeffs[i]
                .iter()
                .zip(other.coeffs[i].iter())
                .map(|(x, y)| x.clone() + y.clone())
                .collect()
        });
        Self { coeffs }
    }

    /// Element-wise subtraction across the batch.
    ///
    /// Returns a new batch whose `i`-th element equals `self[i] - other[i]`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    pub fn batch_sub(&self, other: &Self) -> Self {
        assert_eq!(
            self.len(),
            other.len(),
            "batch_sub: length mismatch ({} vs {})",
            self.len(),
            other.len()
        );
        let coeffs: [Vec<F>; N] = array::from_fn(|i| {
            self.coeffs[i]
                .iter()
                .zip(other.coeffs[i].iter())
                .map(|(x, y)| x.clone() - y.clone())
                .collect()
        });
        Self { coeffs }
    }
}

impl<F: ConstField + SimdKaratsubaHook + Send + Sync> BatchExtField<F, 2> {
    /// Converts a slice of [`QuadraticExt<C>`] elements into SoA form (the
    /// AoS→SoA transpose).
    pub fn from_quadratic<C: ExtConfig<BaseField = F>>(elements: &[QuadraticExt<C>]) -> Self {
        let len = elements.len();
        let mut c0 = Vec::with_capacity(len);
        let mut c1 = Vec::with_capacity(len);
        for e in elements {
            c0.push(e.c0());
            c1.push(e.c1());
        }
        Self { coeffs: [c0, c1] }
    }

    /// Converts the SoA batch back into an AoS `Vec<QuadraticExt<C>>`.
    pub fn to_quadratic<C: ExtConfig<BaseField = F>>(&self) -> Vec<QuadraticExt<C>> {
        self.coeffs[0]
            .iter()
            .zip(self.coeffs[1].iter())
            .map(|(c0, c1)| QuadraticExt::<C>::new(*c0, *c1))
            .collect()
    }

    /// Element-wise Karatsuba multiplication for batched quadratic-extension
    /// elements.
    ///
    /// For each batch index `i`, computes
    ///
    /// ```text
    /// (self[i].c0 + self[i].c1·u) · (other[i].c0 + other[i].c1·u)
    /// ```
    ///
    /// via Karatsuba's identity:
    ///
    /// ```text
    /// v0 = self.c0 · other.c0
    /// v1 = self.c1 · other.c1
    /// out.c0 = v0 + β · v1
    /// out.c1 = (self.c0 + self.c1)·(other.c0 + other.c1) − v0 − v1
    /// ```
    ///
    /// [`SimdKaratsubaHook`] selects the backend. The cost per batch element
    /// is 3 base-field multiplications, 3 additions, 2 subtractions, and one
    /// `mul_by_non_residue`.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    pub fn batch_mul_quadratic<C: ExtConfig<BaseField = F>>(&self, other: &Self) -> Self {
        assert_eq!(
            self.len(),
            other.len(),
            "batch_mul_quadratic: length mismatch ({} vs {})",
            self.len(),
            other.len()
        );

        #[cfg(feature = "parallel")]
        {
            let tuning = crate::tuning::active();
            let soa_batch = tuning.soa_batch();
            if crate::compute::field::should_parallelize_soa_batch_resolved(
                soa_batch.parallel_min_len(),
                self.len(),
            ) {
                return crate::compute::field::batch_mul_quadratic_parallel::<F, C>(
                    self,
                    other,
                    soa_batch.parallel_chunk_len(),
                );
            }
        }

        let a0 = self.coeff(0);
        let a1 = self.coeff(1);
        let b0 = other.coeff(0);
        let b1 = other.coeff(1);

        let (out_c0, out_c1) = batch_karatsuba::<F, C>(a0, a1, b0, b1);
        Self {
            coeffs: [out_c0, out_c1],
        }
    }

    /// Element-wise squaring for batched quadratic-extension elements.
    ///
    /// For each batch index `i`, computes `self[i]²` using the same SoA
    /// Karatsuba backend as [`Self::batch_mul_quadratic`], with the left
    /// and right input lanes aliased.
    pub fn batch_square_quadratic<C: ExtConfig<BaseField = F>>(&self) -> Self {
        #[cfg(feature = "parallel")]
        {
            let tuning = crate::tuning::active();
            let soa_batch = tuning.soa_batch();
            if crate::compute::field::should_parallelize_soa_batch_resolved(
                soa_batch.parallel_min_len(),
                self.len(),
            ) {
                return crate::compute::field::batch_square_quadratic_parallel::<F, C>(
                    self,
                    soa_batch.parallel_chunk_len(),
                );
            }
        }

        let a0 = self.coeff(0);
        let a1 = self.coeff(1);
        let (out_c0, out_c1) = batch_karatsuba::<F, C>(a0, a1, a0, a1);
        Self {
            coeffs: [out_c0, out_c1],
        }
    }
}

impl<F: ConstField + SimdKaratsubaHook + Send + Sync> BatchExtField<F, 3> {
    /// Converts a slice of [`CubicExt<C>`] elements into SoA form (the
    /// AoS→SoA transpose).
    pub fn from_cubic<C: ExtConfig<BaseField = F>>(elements: &[CubicExt<C>]) -> Self {
        let len = elements.len();
        let mut c0 = Vec::with_capacity(len);
        let mut c1 = Vec::with_capacity(len);
        let mut c2 = Vec::with_capacity(len);
        for e in elements {
            c0.push(e.c0());
            c1.push(e.c1());
            c2.push(e.c2());
        }
        Self {
            coeffs: [c0, c1, c2],
        }
    }

    /// Converts a cubic SoA batch back into an AoS `Vec<CubicExt<C>>`.
    pub fn to_cubic<C: ExtConfig<BaseField = F>>(&self) -> Vec<CubicExt<C>> {
        self.coeffs[0]
            .iter()
            .zip(self.coeffs[1].iter())
            .zip(self.coeffs[2].iter())
            .map(|((c0, c1), c2)| CubicExt::<C>::new(*c0, *c1, *c2))
            .collect()
    }

    /// Element-wise Karatsuba-3 multiplication for batched cubic-extension
    /// elements.
    ///
    /// For each batch index `i`, computes `self[i] * other[i]` using the same
    /// six-product formula as `CubicExt::mul`, but over coefficient lanes in
    /// Structure-of-Arrays order. [`SimdKaratsubaHook`] selects the backend.
    /// The cost per batch element is 6 base-field multiplications, 9
    /// additions, 6 subtractions, and two non-residue scales.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{BatchExtField, CubicExt, ExtConfig};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    /// }
    /// type Fq3 = CubicExt<Cfg>;
    ///
    /// let a = vec![Fq3::new(Fp::new(1), Fp::new(2), Fp::new(3))];
    /// let b = vec![Fq3::new(Fp::new(4), Fp::new(5), Fp::new(6))];
    /// let ba = BatchExtField::<Fp<7>, 3>::from_cubic::<Cfg>(&a);
    /// let bb = BatchExtField::<Fp<7>, 3>::from_cubic::<Cfg>(&b);
    /// let got = ba.batch_mul_cubic::<Cfg>(&bb).to_cubic::<Cfg>();
    /// assert_eq!(got[0], a[0] * b[0]);
    /// ```
    pub fn batch_mul_cubic<C: ExtConfig<BaseField = F>>(&self, other: &Self) -> Self {
        assert_eq!(
            self.len(),
            other.len(),
            "batch_mul_cubic: length mismatch ({} vs {})",
            self.len(),
            other.len()
        );

        #[cfg(feature = "parallel")]
        {
            let tuning = crate::tuning::active();
            let soa_batch = tuning.soa_batch();
            if crate::compute::field::should_parallelize_soa_batch_resolved(
                soa_batch.parallel_min_len(),
                self.len(),
            ) {
                return crate::compute::field::batch_mul_cubic_parallel::<F, C>(
                    self,
                    other,
                    soa_batch.parallel_chunk_len(),
                );
            }
        }

        let coeffs = batch_cubic_karatsuba::<F, C>(
            self.coeff(0),
            self.coeff(1),
            self.coeff(2),
            other.coeff(0),
            other.coeff(1),
            other.coeff(2),
        );
        Self { coeffs }
    }

    /// Element-wise squaring for batched cubic-extension elements.
    ///
    /// Computes `self[i]²` by applying the cubic SoA Karatsuba-3 combine of
    /// [`Self::batch_mul_cubic`] with the left and right coefficient lanes
    /// aliased.
    pub fn batch_square_cubic<C: ExtConfig<BaseField = F>>(&self) -> Self {
        #[cfg(feature = "parallel")]
        {
            let tuning = crate::tuning::active();
            let soa_batch = tuning.soa_batch();
            if crate::compute::field::should_parallelize_soa_batch_resolved(
                soa_batch.parallel_min_len(),
                self.len(),
            ) {
                return crate::compute::field::batch_square_cubic_parallel::<F, C>(
                    self,
                    soa_batch.parallel_chunk_len(),
                );
            }
        }

        let coeffs = batch_cubic_karatsuba::<F, C>(
            self.coeff(0),
            self.coeff(1),
            self.coeff(2),
            self.coeff(0),
            self.coeff(1),
            self.coeff(2),
        );
        Self { coeffs }
    }
}

#[inline]
fn batch_add_lane<F>(a: &[F], b: &[F]) -> Vec<F>
where
    F: ConstField + SimdKaratsubaHook,
{
    debug_assert_eq!(a.len(), b.len());
    if let Some(out) = F::try_simd_add_vec(a, b) {
        return out;
    }
    a.iter().zip(b.iter()).map(|(x, y)| *x + *y).collect()
}

#[inline]
fn batch_sub_lane<F>(a: &[F], b: &[F]) -> Vec<F>
where
    F: ConstField + SimdKaratsubaHook,
{
    debug_assert_eq!(a.len(), b.len());
    if let Some(out) = F::try_simd_sub_vec(a, b) {
        return out;
    }
    a.iter().zip(b.iter()).map(|(x, y)| *x - *y).collect()
}

#[inline]
fn batch_mul_lane<F>(a: &[F], b: &[F]) -> Vec<F>
where
    F: ConstField + SimdKaratsubaHook,
{
    debug_assert_eq!(a.len(), b.len());
    if let Some(out) = F::try_simd_mul_vec(a, b) {
        return out;
    }
    a.iter().zip(b.iter()).map(|(x, y)| *x * *y).collect()
}

#[inline]
fn batch_mul_by_non_residue_lane<F, C>(xs: &[F]) -> Vec<F>
where
    F: ConstField + SimdKaratsubaHook,
    C: ExtConfig<BaseField = F>,
{
    let beta = vec![ext_non_residue::<C>(); xs.len()];
    if let Some(out) = F::try_simd_mul_vec(xs, &beta) {
        return out;
    }
    xs.iter().map(|x| C::mul_by_non_residue(*x)).collect()
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn batch_cubic_karatsuba<F, C>(
    a0: &[F],
    a1: &[F],
    a2: &[F],
    b0: &[F],
    b1: &[F],
    b2: &[F],
) -> [Vec<F>; 3]
where
    F: ConstField + SimdKaratsubaHook,
    C: ExtConfig<BaseField = F>,
{
    if let Some(out) = F::try_simd_cubic_karatsuba::<C>(a0, a1, a2, b0, b1, b2) {
        return out;
    }

    let v0 = batch_mul_lane(a0, b0);
    let v1 = batch_mul_lane(a1, b1);
    let v2 = batch_mul_lane(a2, b2);

    let a1_plus_a2 = batch_add_lane(a1, a2);
    let b1_plus_b2 = batch_add_lane(b1, b2);
    let cross12 = batch_mul_lane(&a1_plus_a2, &b1_plus_b2);
    let x_minus_v1 = batch_sub_lane(&cross12, &v1);
    let x = batch_sub_lane(&x_minus_v1, &v2);

    let a0_plus_a1 = batch_add_lane(a0, a1);
    let b0_plus_b1 = batch_add_lane(b0, b1);
    let cross01 = batch_mul_lane(&a0_plus_a1, &b0_plus_b1);
    let y_minus_v0 = batch_sub_lane(&cross01, &v0);
    let y = batch_sub_lane(&y_minus_v0, &v1);

    let a0_plus_a2 = batch_add_lane(a0, a2);
    let b0_plus_b2 = batch_add_lane(b0, b2);
    let cross02 = batch_mul_lane(&a0_plus_a2, &b0_plus_b2);
    let z_minus_v0 = batch_sub_lane(&cross02, &v0);
    let z_plus_v1 = batch_add_lane(&z_minus_v0, &v1);
    let z = batch_sub_lane(&z_plus_v1, &v2);

    let beta_x = batch_mul_by_non_residue_lane::<F, C>(&x);
    let beta_v2 = batch_mul_by_non_residue_lane::<F, C>(&v2);
    let c0 = batch_add_lane(&v0, &beta_x);
    let c1 = batch_add_lane(&y, &beta_v2);

    [c0, c1, z]
}

/// Straight-line scalar Karatsuba combine over any `F: ConstField`. The
/// loop body is branchless and carries no cross-lane dependencies.
#[inline]
fn scalar_karatsuba<F, C>(a0: &[F], a1: &[F], b0: &[F], b1: &[F]) -> (Vec<F>, Vec<F>)
where
    F: ConstField,
    C: ExtConfig<BaseField = F>,
{
    let n = a0.len();
    let mut out_c0 = Vec::with_capacity(n);
    let mut out_c1 = Vec::with_capacity(n);
    for i in 0..n {
        let a0i = a0[i];
        let a1i = a1[i];
        let b0i = b0[i];
        let b1i = b1[i];

        let sum_a = a0i + a1i;
        let sum_b = b0i + b1i;
        let v0 = a0i * b0i;
        let v1 = a1i * b1i;
        let cross = sum_a * sum_b;
        let beta_v1 = C::mul_by_non_residue(v1);

        out_c0.push(v0 + beta_v1);
        out_c1.push(cross - v0 - v1);
    }
    (out_c0, out_c1)
}

/// Top-level Karatsuba combine, generic over any `F: ConstField`.
///
/// Dispatches through [`SimdKaratsubaHook`] and falls back to
/// [`scalar_karatsuba`] when the hook declines.
#[inline]
fn batch_karatsuba<F, C>(a0: &[F], a1: &[F], b0: &[F], b1: &[F]) -> (Vec<F>, Vec<F>)
where
    F: ConstField + SimdKaratsubaHook,
    C: ExtConfig<BaseField = F>,
{
    if let Some(out) = F::try_simd_karatsuba::<C>(a0, a1, b0, b1) {
        return out;
    }
    scalar_karatsuba::<F, C>(a0, a1, b0, b1)
}

/// SIMD dispatch hook of the [`BatchExtField`] quadratic and cubic products;
/// a base field implements it to supply kernels.
///
/// Every method returns `None` by default: a declined quadratic combine runs
/// the scalar loop, a declined cubic combine composes the lane methods (the
/// scalar loop on the parallel path), and a declined lane method runs the
/// scalar loop. In the `Fp<P>` impl both combines at `P = 65537` use the fused
/// AVX2 kernels of `gf2-kernels-simd` when the `simd` feature is on and the
/// host has AVX2, and decline otherwise. At any other `P` the quadratic
/// combine composes the [`crate::gfp::SimdVecOps`] hooks, declining when one
/// of them declines, and the cubic combine declines. The lane methods
/// forward to `SimdVecOps` for every `P`.
pub trait SimdKaratsubaHook: ConstField {
    /// Attempts to compute the Karatsuba combine for a quadratic
    /// extension element-wise over this base field using a SIMD kernel.
    ///
    /// `a0`, `a1`, `b0`, `b1` are the SoA coefficient lanes of two batches
    /// and have identical length. Returns `None` when no SIMD kernel is
    /// available for `Self`.
    #[inline]
    fn try_simd_karatsuba<C: ExtConfig<BaseField = Self>>(
        _a0: &[Self],
        _a1: &[Self],
        _b0: &[Self],
        _b1: &[Self],
    ) -> Option<(Vec<Self>, Vec<Self>)> {
        None
    }

    /// Attempts a fused SIMD Karatsuba-3 combine for cubic-extension batches.
    ///
    /// Computes all three output coefficient lanes `[c0, c1, c2]` of the
    /// element-wise product in `BaseField[X] / (X^3 - C::NON_RESIDUE)` from
    /// the left lanes `a0`, `a1`, `a2` and the right lanes `b0`, `b1`, `b2`.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn try_simd_cubic_karatsuba<C: ExtConfig<BaseField = Self>>(
        _a0: &[Self],
        _a1: &[Self],
        _a2: &[Self],
        _b0: &[Self],
        _b1: &[Self],
        _b2: &[Self],
    ) -> Option<[Vec<Self>; 3]> {
        None
    }

    /// Attempts a SIMD batch addition over one base-field coefficient lane.
    #[inline]
    fn try_simd_add_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }

    /// Attempts a SIMD batch subtraction over one base-field coefficient lane.
    #[inline]
    fn try_simd_sub_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }

    /// Attempts a SIMD batch multiplication over one base-field coefficient lane.
    #[inline]
    fn try_simd_mul_vec(_a: &[Self], _b: &[Self]) -> Option<Vec<Self>> {
        None
    }
}

/// A blanket `impl<F: ConstField> SimdKaratsubaHook for F` cannot coexist
/// with a specialised impl for `Fp<65537>` without the `specialization`
/// feature, so the hook is implemented for every `Fp<P>` and branches on
/// `P`.
impl<const P: u64> SimdKaratsubaHook for Fp<P> {
    #[inline]
    fn try_simd_karatsuba<C: ExtConfig<BaseField = Self>>(
        a0: &[Self],
        a1: &[Self],
        b0: &[Self],
        b1: &[Self],
    ) -> Option<(Vec<Self>, Vec<Self>)> {
        if P == 65537 {
            return fp65537_simd_impl::<P, C>(a0, a1, b0, b1);
        }
        fp_simd_composed_impl::<P, C>(a0, a1, b0, b1)
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn try_simd_cubic_karatsuba<C: ExtConfig<BaseField = Self>>(
        a0: &[Self],
        a1: &[Self],
        a2: &[Self],
        b0: &[Self],
        b1: &[Self],
        b2: &[Self],
    ) -> Option<[Vec<Self>; 3]> {
        if P == 65537 {
            return fp65537_simd_cubic_impl::<P, C>(a0, a1, a2, b0, b1, b2);
        }
        None
    }

    #[inline]
    fn try_simd_add_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        <Self as SimdVecOps>::try_simd_add_vec(a, b)
    }

    #[inline]
    fn try_simd_sub_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        <Self as SimdVecOps>::try_simd_sub_vec(a, b)
    }

    #[inline]
    fn try_simd_mul_vec(a: &[Self], b: &[Self]) -> Option<Vec<Self>> {
        <Self as SimdVecOps>::try_simd_mul_vec(a, b)
    }
}

/// Generic `Fp<P>` Karatsuba composition over the shared base-field SIMD hooks.
///
/// Returns `None` if any base-field add/sub/mul hook declines.
#[inline]
fn fp_simd_composed_impl<const P: u64, C: ExtConfig<BaseField = Fp<P>>>(
    a0: &[Fp<P>],
    a1: &[Fp<P>],
    b0: &[Fp<P>],
    b1: &[Fp<P>],
) -> Option<(Vec<Fp<P>>, Vec<Fp<P>>)> {
    let n = a0.len();
    let sum_a = <Fp<P> as SimdVecOps>::try_simd_add_vec(a0, a1)?;
    let sum_b = <Fp<P> as SimdVecOps>::try_simd_add_vec(b0, b1)?;

    let v0 = <Fp<P> as SimdVecOps>::try_simd_mul_vec(a0, b0)?;
    let v1 = <Fp<P> as SimdVecOps>::try_simd_mul_vec(a1, b1)?;
    let cross = <Fp<P> as SimdVecOps>::try_simd_mul_vec(&sum_a, &sum_b)?;

    let beta = ext_non_residue::<C>();
    let beta_v1 = if beta.is_zero() {
        vec![Fp::<P>::zero(); n]
    } else if beta.is_one() {
        v1.clone()
    } else {
        let beta_vec = vec![beta; n];
        <Fp<P> as SimdVecOps>::try_simd_mul_vec(&v1, &beta_vec)?
    };

    let out_c0 = <Fp<P> as SimdVecOps>::try_simd_add_vec(&v0, &beta_v1)?;
    let tmp = <Fp<P> as SimdVecOps>::try_simd_sub_vec(&cross, &v0)?;
    let out_c1 = <Fp<P> as SimdVecOps>::try_simd_sub_vec(&tmp, &v1)?;

    Some((out_c0, out_c1))
}

/// AVX2 Karatsuba combine for `Fp<P>` specialised at `P = 65537`.
///
/// The caller (the `SimdKaratsubaHook` impl for `Fp<P>`) gates on
/// `P == 65537`, for which `Fp<P>::raw_storage()` equals the canonical
/// value because `R = 2^64 ≡ 1 (mod 65537)`.
///
/// Returns `None` on non-AVX2 hardware.
#[cfg(feature = "simd")]
fn fp65537_simd_impl<const P: u64, C: ExtConfig<BaseField = Fp<P>>>(
    a0: &[Fp<P>],
    a1: &[Fp<P>],
    b0: &[Fp<P>],
    b1: &[Fp<P>],
) -> Option<(Vec<Fp<P>>, Vec<Fp<P>>)> {
    use crate::gfp::simd_ops::{fp65537_pack, fp65537_unpack};
    debug_assert_eq!(P, 65537, "fp65537_simd_impl requires P = 65537");

    let fns = crate::simd::maybe_fp65537()?;
    let n = a0.len();

    let a0_u32 = fp65537_pack::<P>(a0);
    let a1_u32 = fp65537_pack::<P>(a1);
    let b0_u32 = fp65537_pack::<P>(b0);
    let b1_u32 = fp65537_pack::<P>(b1);

    // One fused SIMD pass: the Karatsuba intermediates stay in AVX2
    // registers.
    let beta_u32 = ext_non_residue::<C>().raw_storage() as u32;
    let mut out_c0 = vec![0u32; n];
    let mut out_c1 = vec![0u32; n];
    (fns.batch_karatsuba_fn)(
        &a0_u32,
        &a1_u32,
        &b0_u32,
        &b1_u32,
        beta_u32,
        &mut out_c0,
        &mut out_c1,
    );

    Some((fp65537_unpack::<P>(&out_c0), fp65537_unpack::<P>(&out_c1)))
}

#[cfg(feature = "simd")]
#[allow(clippy::too_many_arguments)]
fn fp65537_simd_cubic_impl<const P: u64, C: ExtConfig<BaseField = Fp<P>>>(
    a0: &[Fp<P>],
    a1: &[Fp<P>],
    a2: &[Fp<P>],
    b0: &[Fp<P>],
    b1: &[Fp<P>],
    b2: &[Fp<P>],
) -> Option<[Vec<Fp<P>>; 3]> {
    use crate::gfp::simd_ops::{fp65537_pack, fp65537_unpack};
    debug_assert_eq!(P, 65537, "fp65537_simd_cubic_impl requires P = 65537");

    let fns = crate::simd::maybe_fp65537()?;
    let n = a0.len();

    let a0_u32 = fp65537_pack::<P>(a0);
    let a1_u32 = fp65537_pack::<P>(a1);
    let a2_u32 = fp65537_pack::<P>(a2);
    let b0_u32 = fp65537_pack::<P>(b0);
    let b1_u32 = fp65537_pack::<P>(b1);
    let b2_u32 = fp65537_pack::<P>(b2);

    let beta_u32 = ext_non_residue::<C>().raw_storage() as u32;
    let mut out_c0 = vec![0u32; n];
    let mut out_c1 = vec![0u32; n];
    let mut out_c2 = vec![0u32; n];
    (fns.batch_cubic_karatsuba_fn)(
        &a0_u32,
        &a1_u32,
        &a2_u32,
        &b0_u32,
        &b1_u32,
        &b2_u32,
        beta_u32,
        &mut out_c0,
        &mut out_c1,
        &mut out_c2,
    );

    Some([
        fp65537_unpack::<P>(&out_c0),
        fp65537_unpack::<P>(&out_c1),
        fp65537_unpack::<P>(&out_c2),
    ])
}

#[cfg(not(feature = "simd"))]
#[inline]
#[allow(clippy::extra_unused_type_parameters, clippy::too_many_arguments)]
fn fp65537_simd_cubic_impl<const P: u64, C: ExtConfig<BaseField = Fp<P>>>(
    _a0: &[Fp<P>],
    _a1: &[Fp<P>],
    _a2: &[Fp<P>],
    _b0: &[Fp<P>],
    _b1: &[Fp<P>],
    _b2: &[Fp<P>],
) -> Option<[Vec<Fp<P>>; 3]> {
    None
}

#[cfg(not(feature = "simd"))]
#[inline]
#[allow(clippy::extra_unused_type_parameters)]
fn fp65537_simd_impl<const P: u64, C: ExtConfig<BaseField = Fp<P>>>(
    _a0: &[Fp<P>],
    _a1: &[Fp<P>],
    _b0: &[Fp<P>],
    _b1: &[Fp<P>],
) -> Option<(Vec<Fp<P>>, Vec<Fp<P>>)> {
    // `C` stays in this stub's signature so callers use the same generic
    // parameters with and without the `simd` feature.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfp::Fp;
    use proptest::prelude::*;

    struct CfgBeta3;
    impl ExtConfig for CfgBeta3 {
        type BaseField = Fp<65537>;
        const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
    }
    type Fq2Big = QuadraticExt<CfgBeta3>;

    struct CfgNeg1;
    impl ExtConfig for CfgNeg1 {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1
        fn mul_by_non_residue(x: Fp<7>) -> Fp<7> {
            -x
        }
    }
    type Fq2Small = QuadraticExt<CfgNeg1>;

    const GENERIC_P: u64 = 2_147_483_629;

    struct CfgGeneric;
    impl ExtConfig for CfgGeneric {
        type BaseField = Fp<GENERIC_P>;
        const NON_RESIDUE: Fp<GENERIC_P> = Fp::<GENERIC_P>::new(5);
    }
    type Fq2Generic = QuadraticExt<CfgGeneric>;

    struct CfgCubicSmall;
    impl ExtConfig for CfgCubicSmall {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }
    type Fq3Small = CubicExt<CfgCubicSmall>;

    struct CfgCubicBeta3;
    impl ExtConfig for CfgCubicBeta3 {
        type BaseField = Fp<65537>;
        const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
    }
    type Fq3Big = CubicExt<CfgCubicBeta3>;

    #[test]
    fn test_new_reports_correct_len() {
        let batch = BatchExtField::<Fp<7>, 2>::new([
            vec![Fp::new(1), Fp::new(2), Fp::new(3)],
            vec![Fp::new(4), Fp::new(5), Fp::new(6)],
        ]);
        assert_eq!(batch.len(), 3);
        assert!(!batch.is_empty());
        assert!(batch.is_valid());
    }

    #[test]
    fn test_empty_batch() {
        let batch = BatchExtField::<Fp<7>, 2>::new([vec![], vec![]]);
        assert_eq!(batch.len(), 0);
        assert!(batch.is_empty());
        assert!(batch.is_valid());
    }

    #[test]
    fn test_zeros_all_zero() {
        let batch = BatchExtField::<Fp<7>, 2>::zeros(5, &Fp::new(0));
        assert_eq!(batch.len(), 5);
        for lane_idx in 0..2 {
            for v in batch.coeff(lane_idx) {
                assert!(v.is_zero());
            }
        }
    }

    #[test]
    #[should_panic(expected = "BatchExtField::new")]
    fn test_new_panics_on_length_mismatch() {
        let _batch = BatchExtField::<Fp<7>, 2>::new([
            vec![Fp::new(1), Fp::new(2)],
            vec![Fp::new(3)], // shorter → panic
        ]);
    }

    #[test]
    #[should_panic(expected = "batch_add: length mismatch")]
    fn test_batch_add_panics_on_mismatched_lengths() {
        let a = BatchExtField::<Fp<7>, 2>::new([vec![Fp::new(1)], vec![Fp::new(2)]]);
        let b = BatchExtField::<Fp<7>, 2>::new([
            vec![Fp::new(1), Fp::new(2)],
            vec![Fp::new(3), Fp::new(4)],
        ]);
        let _ = a.batch_add(&b);
    }

    #[test]
    fn test_roundtrip_small_handcrafted() {
        let xs = vec![
            Fq2Small::new(Fp::new(0), Fp::new(0)),
            Fq2Small::new(Fp::new(1), Fp::new(6)),
            Fq2Small::new(Fp::new(3), Fp::new(4)),
            Fq2Small::new(Fp::new(5), Fp::new(2)),
        ];
        let batch = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&xs);
        assert_eq!(batch.len(), xs.len());
        assert_eq!(batch.coeff(0)[0].value(), 0);
        assert_eq!(batch.coeff(1)[1].value(), 6);
        assert_eq!(batch.to_quadratic::<CfgNeg1>(), xs);
    }

    #[test]
    fn test_roundtrip_empty() {
        let xs: Vec<Fq2Small> = vec![];
        let batch = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&xs);
        assert!(batch.is_empty());
        assert!(batch.to_quadratic::<CfgNeg1>().is_empty());
    }

    #[test]
    fn test_roundtrip_single_element() {
        let xs = vec![Fq2Small::new(Fp::new(2), Fp::new(5))];
        let batch = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&xs);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.to_quadratic::<CfgNeg1>(), xs);
    }

    #[test]
    fn test_batch_mul_small_handcrafted() {
        let a = vec![Fq2Small::new(Fp::new(3), Fp::new(2))];
        let b = vec![Fq2Small::new(Fp::new(4), Fp::new(5))];
        let ba = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&a);
        let bb = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&b);
        let bc = ba.batch_mul_quadratic::<CfgNeg1>(&bb);
        let c = bc.to_quadratic::<CfgNeg1>();
        assert_eq!(c[0], Fq2Small::new(Fp::new(2), Fp::new(2)));
    }

    #[test]
    fn test_batch_mul_matches_scalar_exhaustive_gf7() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        for a0 in 0..7u64 {
            for a1 in 0..7u64 {
                for b0 in 0..7u64 {
                    for b1 in 0..7u64 {
                        a.push(Fq2Small::new(Fp::new(a0), Fp::new(a1)));
                        b.push(Fq2Small::new(Fp::new(b0), Fp::new(b1)));
                    }
                }
            }
        }
        let ba = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&a);
        let bb = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&b);
        let bc = ba.batch_mul_quadratic::<CfgNeg1>(&bb);
        let c = bc.to_quadratic::<CfgNeg1>();
        for i in 0..a.len() {
            assert_eq!(c[i], a[i] * b[i], "mismatch at index {i}");
        }
    }

    #[test]
    fn test_batch_mul_empty_batch() {
        let a: Vec<Fq2Small> = vec![];
        let b: Vec<Fq2Small> = vec![];
        let ba = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&a);
        let bb = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&b);
        let bc = ba.batch_mul_quadratic::<CfgNeg1>(&bb);
        assert!(bc.is_empty());
    }

    #[test]
    #[should_panic(expected = "batch_mul_quadratic: length mismatch")]
    fn test_batch_mul_panics_on_length_mismatch() {
        let a = BatchExtField::<Fp<7>, 2>::new([vec![Fp::new(1)], vec![Fp::new(2)]]);
        let b = BatchExtField::<Fp<7>, 2>::new([
            vec![Fp::new(3), Fp::new(4)],
            vec![Fp::new(5), Fp::new(6)],
        ]);
        let _ = a.batch_mul_quadratic::<CfgNeg1>(&b);
    }

    #[test]
    fn test_batch_add_matches_scalar() {
        let a = vec![
            Fq2Small::new(Fp::new(1), Fp::new(2)),
            Fq2Small::new(Fp::new(3), Fp::new(4)),
        ];
        let b = vec![
            Fq2Small::new(Fp::new(5), Fp::new(6)),
            Fq2Small::new(Fp::new(0), Fp::new(1)),
        ];
        let ba = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&a);
        let bb = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&b);
        let bc = ba.batch_add(&bb);
        let c = bc.to_quadratic::<CfgNeg1>();
        for i in 0..a.len() {
            assert_eq!(c[i], a[i] + b[i]);
        }
    }

    #[test]
    fn test_batch_sub_matches_scalar() {
        let a = vec![
            Fq2Small::new(Fp::new(1), Fp::new(2)),
            Fq2Small::new(Fp::new(3), Fp::new(4)),
        ];
        let b = vec![
            Fq2Small::new(Fp::new(5), Fp::new(6)),
            Fq2Small::new(Fp::new(0), Fp::new(1)),
        ];
        let ba = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&a);
        let bb = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&b);
        let bc = ba.batch_sub(&bb);
        let c = bc.to_quadratic::<CfgNeg1>();
        for i in 0..a.len() {
            assert_eq!(c[i], a[i] - b[i]);
        }
    }

    #[test]
    fn test_batch_add_sub_available_for_non_simd_field() {
        use crate::gfp::specialized::GoldilocksFp;

        let a = BatchExtField::<GoldilocksFp, 2>::new([
            vec![GoldilocksFp::new(1), GoldilocksFp::new(2)],
            vec![GoldilocksFp::new(3), GoldilocksFp::new(4)],
        ]);
        let b = BatchExtField::<GoldilocksFp, 2>::new([
            vec![GoldilocksFp::new(5), GoldilocksFp::new(6)],
            vec![GoldilocksFp::new(7), GoldilocksFp::new(8)],
        ]);

        let sum = a.batch_add(&b);
        let diff = sum.batch_sub(&b);

        assert_eq!(sum.coeff(0)[0], GoldilocksFp::new(6));
        assert_eq!(sum.coeff(1)[1], GoldilocksFp::new(12));
        assert_eq!(diff.coeff(0), a.coeff(0));
        assert_eq!(diff.coeff(1), a.coeff(1));
    }

    #[test]
    fn test_batch_square_matches_scalar() {
        let xs = vec![
            Fq2Small::new(Fp::new(0), Fp::new(0)),
            Fq2Small::new(Fp::new(1), Fp::new(6)),
            Fq2Small::new(Fp::new(3), Fp::new(4)),
            Fq2Small::new(Fp::new(5), Fp::new(2)),
        ];
        let batch = BatchExtField::<Fp<7>, 2>::from_quadratic::<CfgNeg1>(&xs);
        let got = batch
            .batch_square_quadratic::<CfgNeg1>()
            .to_quadratic::<CfgNeg1>();
        for i in 0..xs.len() {
            assert_eq!(got[i], xs[i] * xs[i], "square mismatch at index {i}");
        }
    }

    #[test]
    fn test_batch_all_ops_match_scalar_fp65537() {
        let a: Vec<Fq2Big> = (0..17u64)
            .map(|i| Fq2Big::new(Fp::new(i * 97 + 3), Fp::new(i * 193 + 5)))
            .collect();
        let b: Vec<Fq2Big> = (0..17u64)
            .map(|i| Fq2Big::new(Fp::new(i * 389 + 7), Fp::new(i * 769 + 11)))
            .collect();

        let ba = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&a);
        let bb = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&b);

        let add = ba.batch_add(&bb).to_quadratic::<CfgBeta3>();
        let sub = ba.batch_sub(&bb).to_quadratic::<CfgBeta3>();
        let mul = ba
            .batch_mul_quadratic::<CfgBeta3>(&bb)
            .to_quadratic::<CfgBeta3>();
        let sqr = ba
            .batch_square_quadratic::<CfgBeta3>()
            .to_quadratic::<CfgBeta3>();

        for i in 0..a.len() {
            assert_eq!(add[i], a[i] + b[i], "add mismatch at index {i}");
            assert_eq!(sub[i], a[i] - b[i], "sub mismatch at index {i}");
            assert_eq!(mul[i], a[i] * b[i], "mul mismatch at index {i}");
            assert_eq!(sqr[i], a[i] * a[i], "square mismatch at index {i}");
        }
    }

    #[test]
    fn test_generic_fp_batch_mul_matches_scalar_and_hook_when_available() {
        let a: Vec<Fq2Generic> = (0..65u64)
            .map(|i| {
                Fq2Generic::new(
                    Fp::<GENERIC_P>::new(i.wrapping_mul(1_000_003).wrapping_add(17)),
                    Fp::<GENERIC_P>::new(i.wrapping_mul(3_000_017).wrapping_add(19)),
                )
            })
            .collect();
        let b: Vec<Fq2Generic> = (0..65u64)
            .map(|i| {
                Fq2Generic::new(
                    Fp::<GENERIC_P>::new(i.wrapping_mul(5_000_021).wrapping_add(23)),
                    Fp::<GENERIC_P>::new(i.wrapping_mul(7_000_027).wrapping_add(29)),
                )
            })
            .collect();

        let ba = BatchExtField::<Fp<GENERIC_P>, 2>::from_quadratic::<CfgGeneric>(&a);
        let bb = BatchExtField::<Fp<GENERIC_P>, 2>::from_quadratic::<CfgGeneric>(&b);
        let got = ba
            .batch_mul_quadratic::<CfgGeneric>(&bb)
            .to_quadratic::<CfgGeneric>();
        for i in 0..a.len() {
            assert_eq!(
                got[i],
                a[i] * b[i],
                "generic batch mul mismatch at index {i}"
            );
        }

        #[cfg(feature = "simd")]
        if crate::simd::maybe_fp_generic().is_some() {
            let (out_c0, out_c1) = <Fp<GENERIC_P> as SimdKaratsubaHook>::try_simd_karatsuba::<
                CfgGeneric,
            >(ba.coeff(0), ba.coeff(1), bb.coeff(0), bb.coeff(1))
            .expect("generic Fp SIMD Karatsuba hook should compose C3 batch ops");
            let via_hook = BatchExtField::<Fp<GENERIC_P>, 2>::new([out_c0, out_c1])
                .to_quadratic::<CfgGeneric>();
            assert_eq!(via_hook, got);
        }
    }

    #[test]
    fn test_cubic_roundtrip_edge_lengths() {
        for &len in &[0usize, 1, 2, 7, 8, 17, 64, 65] {
            let xs: Vec<Fq3Big> = (0..len as u64)
                .map(|i| Fq3Big::new(Fp::new(i + 1), Fp::new(3 * i + 2), Fp::new(5 * i + 3)))
                .collect();
            let batch = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&xs);
            assert_eq!(batch.len(), len);
            assert!(batch.is_valid());
            assert_eq!(batch.to_cubic::<CfgCubicBeta3>(), xs);
        }
    }

    #[test]
    fn test_cubic_batch_all_ops_match_scalar_fp7_handcrafted() {
        let a = vec![
            Fq3Small::new(Fp::new(0), Fp::new(0), Fp::new(0)),
            Fq3Small::new(Fp::new(1), Fp::new(2), Fp::new(3)),
            Fq3Small::new(Fp::new(6), Fp::new(5), Fp::new(4)),
        ];
        let b = vec![
            Fq3Small::new(Fp::new(1), Fp::new(0), Fp::new(6)),
            Fq3Small::new(Fp::new(4), Fp::new(3), Fp::new(2)),
            Fq3Small::new(Fp::new(0), Fp::new(1), Fp::new(2)),
        ];

        let ba = BatchExtField::<Fp<7>, 3>::from_cubic::<CfgCubicSmall>(&a);
        let bb = BatchExtField::<Fp<7>, 3>::from_cubic::<CfgCubicSmall>(&b);

        let add = ba.batch_add(&bb).to_cubic::<CfgCubicSmall>();
        let sub = ba.batch_sub(&bb).to_cubic::<CfgCubicSmall>();
        let mul = ba
            .batch_mul_cubic::<CfgCubicSmall>(&bb)
            .to_cubic::<CfgCubicSmall>();
        let sqr = ba
            .batch_square_cubic::<CfgCubicSmall>()
            .to_cubic::<CfgCubicSmall>();

        for i in 0..a.len() {
            assert_eq!(add[i], a[i] + b[i], "cubic add mismatch at index {i}");
            assert_eq!(sub[i], a[i] - b[i], "cubic sub mismatch at index {i}");
            assert_eq!(mul[i], a[i] * b[i], "cubic mul mismatch at index {i}");
            assert_eq!(sqr[i], a[i] * a[i], "cubic square mismatch at index {i}");
        }
    }

    #[test]
    fn test_cubic_batch_all_ops_match_scalar_fp65537_boundaries() {
        for &len in &[1usize, 7, 8, 15, 16, 17, 63, 64, 65] {
            let a: Vec<Fq3Big> = (0..len as u64)
                .map(|i| {
                    Fq3Big::new(
                        Fp::new(i.wrapping_mul(97).wrapping_add(3)),
                        Fp::new(i.wrapping_mul(193).wrapping_add(5)),
                        Fp::new(i.wrapping_mul(389).wrapping_add(7)),
                    )
                })
                .collect();
            let b: Vec<Fq3Big> = (0..len as u64)
                .map(|i| {
                    Fq3Big::new(
                        Fp::new(i.wrapping_mul(769).wrapping_add(11)),
                        Fp::new(i.wrapping_mul(1543).wrapping_add(13)),
                        Fp::new(i.wrapping_mul(3079).wrapping_add(17)),
                    )
                })
                .collect();

            let ba = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&a);
            let bb = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&b);

            let add = ba.batch_add(&bb).to_cubic::<CfgCubicBeta3>();
            let sub = ba.batch_sub(&bb).to_cubic::<CfgCubicBeta3>();
            let mul = ba
                .batch_mul_cubic::<CfgCubicBeta3>(&bb)
                .to_cubic::<CfgCubicBeta3>();
            let sqr = ba
                .batch_square_cubic::<CfgCubicBeta3>()
                .to_cubic::<CfgCubicBeta3>();

            for i in 0..len {
                assert_eq!(add[i], a[i] + b[i], "add mismatch at len {len}, index {i}");
                assert_eq!(sub[i], a[i] - b[i], "sub mismatch at len {len}, index {i}");
                assert_eq!(mul[i], a[i] * b[i], "mul mismatch at len {len}, index {i}");
                assert_eq!(
                    sqr[i],
                    a[i] * a[i],
                    "square mismatch at len {len}, index {i}"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "batch_mul_cubic: length mismatch")]
    fn test_cubic_batch_mul_panics_on_length_mismatch() {
        let a =
            BatchExtField::<Fp<7>, 3>::new([vec![Fp::new(1)], vec![Fp::new(2)], vec![Fp::new(3)]]);
        let b = BatchExtField::<Fp<7>, 3>::new([
            vec![Fp::new(4), Fp::new(5)],
            vec![Fp::new(6), Fp::new(0)],
            vec![Fp::new(1), Fp::new(2)],
        ]);
        let _ = a.batch_mul_cubic::<CfgCubicSmall>(&b);
    }

    fn fq2_big_strategy() -> impl Strategy<Value = Fq2Big> {
        (0..65537u64, 0..65537u64).prop_map(|(c0, c1)| Fq2Big::new(Fp::new(c0), Fp::new(c1)))
    }

    fn fq3_big_strategy() -> impl Strategy<Value = Fq3Big> {
        (0..65537u64, 0..65537u64, 0..65537u64)
            .prop_map(|(c0, c1, c2)| Fq3Big::new(Fp::new(c0), Fp::new(c1), Fp::new(c2)))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_batch_mul_matches_scalar_fp65537(
            pairs in proptest::collection::vec((fq2_big_strategy(), fq2_big_strategy()), 1..=64)
        ) {
            let a: Vec<Fq2Big> = pairs.iter().map(|(x, _)| *x).collect();
            let b: Vec<Fq2Big> = pairs.iter().map(|(_, y)| *y).collect();
            let expected: Vec<Fq2Big> = a.iter().zip(b.iter()).map(|(x, y)| *x * *y).collect();

            let ba = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&a);
            let bb = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&b);
            let bc = ba.batch_mul_quadratic::<CfgBeta3>(&bb);
            let c = bc.to_quadratic::<CfgBeta3>();

            prop_assert_eq!(c, expected);
        }

        #[test]
        fn prop_batch_square_matches_scalar_fp65537(
            xs in proptest::collection::vec(fq2_big_strategy(), 0..=64)
        ) {
            let expected: Vec<Fq2Big> = xs.iter().map(|x| *x * *x).collect();
            let batch = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&xs);
            let got = batch.batch_square_quadratic::<CfgBeta3>().to_quadratic::<CfgBeta3>();

            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_roundtrip_fp65537(
            xs in proptest::collection::vec(fq2_big_strategy(), 0..=64)
        ) {
            let batch = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&xs);
            prop_assert!(batch.is_valid());
            let round = batch.to_quadratic::<CfgBeta3>();
            prop_assert_eq!(round, xs);
        }

        #[test]
        fn prop_cubic_batch_mul_matches_scalar_fp65537(
            pairs in proptest::collection::vec((fq3_big_strategy(), fq3_big_strategy()), 1..=64)
        ) {
            let a: Vec<Fq3Big> = pairs.iter().map(|(x, _)| *x).collect();
            let b: Vec<Fq3Big> = pairs.iter().map(|(_, y)| *y).collect();
            let expected: Vec<Fq3Big> = a.iter().zip(b.iter()).map(|(x, y)| *x * *y).collect();

            let ba = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&a);
            let bb = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&b);
            let got = ba.batch_mul_cubic::<CfgCubicBeta3>(&bb).to_cubic::<CfgCubicBeta3>();

            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_cubic_batch_square_matches_scalar_fp65537(
            xs in proptest::collection::vec(fq3_big_strategy(), 0..=64)
        ) {
            let expected: Vec<Fq3Big> = xs.iter().map(|x| *x * *x).collect();
            let batch = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&xs);
            let got = batch.batch_square_cubic::<CfgCubicBeta3>().to_cubic::<CfgCubicBeta3>();

            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_cubic_roundtrip_fp65537(
            xs in proptest::collection::vec(fq3_big_strategy(), 0..=64)
        ) {
            let batch = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&xs);
            prop_assert!(batch.is_valid());
            let round = batch.to_cubic::<CfgCubicBeta3>();
            prop_assert_eq!(round, xs);
        }
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel_quadratic_chunks_are_bit_exact() {
        for &len in &[
            0usize,
            1,
            63,
            64,
            65,
            crate::compute::field::SOA_PARALLEL_CHUNK_LEN - 1,
            crate::compute::field::SOA_PARALLEL_CHUNK_LEN,
            crate::compute::field::SOA_PARALLEL_CHUNK_LEN + 17,
            crate::compute::field::SOA_PARALLEL_MIN_LEN + 19,
        ] {
            let a: Vec<Fq2Big> = (0..len)
                .map(|i| {
                    Fq2Big::new(
                        Fp::new((17 * i as u64 + 3) % 65537),
                        Fp::new((29 * i as u64 + 5) % 65537),
                    )
                })
                .collect();
            let b: Vec<Fq2Big> = (0..len)
                .map(|i| {
                    Fq2Big::new(
                        Fp::new((31 * i as u64 + 7) % 65537),
                        Fp::new((43 * i as u64 + 11) % 65537),
                    )
                })
                .collect();

            let ba = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&a);
            let bb = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&b);
            let expected_mul: Vec<Fq2Big> = a.iter().zip(b.iter()).map(|(x, y)| *x * *y).collect();
            let expected_square: Vec<Fq2Big> = a.iter().map(|x| *x * *x).collect();

            let got_mul =
                crate::compute::field::batch_mul_quadratic_parallel::<Fp<65537>, CfgBeta3>(
                    &ba,
                    &bb,
                    crate::compute::field::SOA_PARALLEL_CHUNK_LEN,
                )
                .to_quadratic::<CfgBeta3>();
            let got_square = crate::compute::field::batch_square_quadratic_parallel::<
                Fp<65537>,
                CfgBeta3,
            >(&ba, crate::compute::field::SOA_PARALLEL_CHUNK_LEN)
            .to_quadratic::<CfgBeta3>();

            assert_eq!(got_mul, expected_mul, "parallel Fq2 mul len {len}");
            assert_eq!(got_square, expected_square, "parallel Fq2 square len {len}");
        }
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel_cubic_chunks_are_bit_exact() {
        for &len in &[
            0usize,
            1,
            63,
            64,
            65,
            crate::compute::field::SOA_PARALLEL_CHUNK_LEN - 1,
            crate::compute::field::SOA_PARALLEL_CHUNK_LEN,
            crate::compute::field::SOA_PARALLEL_CHUNK_LEN + 17,
            crate::compute::field::SOA_PARALLEL_MIN_LEN + 19,
        ] {
            let a: Vec<Fq3Big> = (0..len)
                .map(|i| {
                    Fq3Big::new(
                        Fp::new((17 * i as u64 + 3) % 65537),
                        Fp::new((29 * i as u64 + 5) % 65537),
                        Fp::new((37 * i as u64 + 13) % 65537),
                    )
                })
                .collect();
            let b: Vec<Fq3Big> = (0..len)
                .map(|i| {
                    Fq3Big::new(
                        Fp::new((31 * i as u64 + 7) % 65537),
                        Fp::new((43 * i as u64 + 11) % 65537),
                        Fp::new((47 * i as u64 + 19) % 65537),
                    )
                })
                .collect();

            let ba = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&a);
            let bb = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&b);
            let expected_mul: Vec<Fq3Big> = a.iter().zip(b.iter()).map(|(x, y)| *x * *y).collect();
            let expected_square: Vec<Fq3Big> = a.iter().map(|x| *x * *x).collect();

            let got_mul =
                crate::compute::field::batch_mul_cubic_parallel::<Fp<65537>, CfgCubicBeta3>(
                    &ba,
                    &bb,
                    crate::compute::field::SOA_PARALLEL_CHUNK_LEN,
                )
                .to_cubic::<CfgCubicBeta3>();
            let got_square = crate::compute::field::batch_square_cubic_parallel::<
                Fp<65537>,
                CfgCubicBeta3,
            >(&ba, crate::compute::field::SOA_PARALLEL_CHUNK_LEN)
            .to_cubic::<CfgCubicBeta3>();

            assert_eq!(got_mul, expected_mul, "parallel Fq3 mul len {len}");
            assert_eq!(got_square, expected_square, "parallel Fq3 square len {len}");
        }
    }
}
