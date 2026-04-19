//! Structure-of-Arrays (SoA) batch layout for GF(p^n) extension fields.
//!
//! This module provides [`BatchExtField<F, N>`], a storage layout that
//! interleaves the coefficients of many independent extension-field elements
//! by coefficient position rather than by element. Where a standard
//! Array-of-Structures (AoS) layout stores elements as
//!
//! ```text
//! [{c0, c1}, {c0, c1}, {c0, c1}, …]
//! ```
//!
//! the SoA layout stores each coefficient index in its own contiguous buffer:
//!
//! ```text
//! coeffs[0] = [elem0.c0, elem1.c0, elem2.c0, …]
//! coeffs[1] = [elem0.c1, elem1.c1, elem2.c1, …]
//! ```
//!
//! This gives two important properties:
//!
//! 1. Coefficient-level arithmetic (base-field add, sub, mul) becomes a
//!    single pass over a contiguous `&[F]`, which is the shape that SIMD
//!    kernels want. Cross-lane dependencies at the polynomial level are
//!    replaced by a small number of independent vector passes.
//! 2. Extension-field multiplication decomposes cleanly into base-field
//!    batch operations. For `BatchExtField<Fp<P>, 2>`, Karatsuba becomes
//!    three base-field vector multiplies plus a handful of vector
//!    adds/subs, irrespective of batch size.
//!
//! # SIMD integration
//!
//! [`BatchExtField::batch_mul_quadratic`] dispatches through the
//! [`SimdKaratsubaHook`] trait. For the specialised case
//! `F = Fp<65537>` on AVX2 hosts the trait routes into the fused AVX2
//! Karatsuba kernel exposed by `gf2-kernels-simd::fp65537`
//! (`batch_karatsuba_fn`): every 8-lane 256-bit vector iteration reads
//! `a0, a1, b0, b1` once, keeps the seven Karatsuba intermediates in
//! registers, and writes `out_c0, out_c1` once. The reduction exploits
//! `2^16 ≡ -1 (mod 65537)` — the product splits at the 16-bit boundary
//! and a single `lo + P - hi` fold plus one branchless canonicalisation
//! delivers a canonical u32 per lane.
//!
//! For any other base field (or when AVX2 is unavailable, the `simd`
//! feature is disabled, or we build on a non-x86 target) the trait's
//! default `None` arm triggers the scalar straight-line combine, whose
//! inner loop is branchless and cross-lane-dependency-free so LLVM's
//! auto-vectoriser can widen it opportunistically.
//!
//! [`crate::field::FieldVec`] uses the *same* AVX2 `Fp<65537>` kernels
//! at the element-wise level via the
//! [`crate::gfp::SimdVecOps`] trait — `mul_vec`, `add_vec`, and
//! `sub_vec` transparently route through `gf2-kernels-simd` on AVX2
//! hosts. The pack/unpack helpers in
//! [`crate::gfp::simd_ops`] are shared between the two surfaces so there
//! is a single source of truth for the Montgomery-canonical fast path.
//!
//! # Measured performance
//!
//! Benchmarked on the reference Zen 3 host (AMD Ryzen 9 5900X) via
//! `cargo bench -p gf2-core --bench soa_batch --features simd -- --quick`
//! at `N = 1000` GF(p²) elements over `Fp<65537>` with `β = 3`:
//!
//! | workload                                        | time     | vs baseline |
//! |-------------------------------------------------|---------:|------------:|
//! | sequential `QuadraticExt::mul` (AoS, scalar)    | 15.60 µs |   1.00× |
//! | `BatchExtField::batch_mul_quadratic` (SoA)      |  1.97 µs |   7.91× |
//! | SoA including AoS↔SoA transpose                 |  5.06 µs |   3.08× |
//!
//! Both the pure SoA path and the end-to-end AoS→SoA→AoS path beat the
//! issue's ≥3× target. The fused AVX2 Karatsuba kernel is the source
//! of the speedup: it replaces nine heap-allocated intermediate buffers
//! with zero (all staging happens in AVX2 registers), and replaces
//! thirteen scalar Montgomery multiplies per output element with three
//! packed 8-lane 64-bit integer multiplies plus a one-step modular
//! reduction. The AoS↔SoA transpose cost (~3 µs) dominates the
//! end-to-end path at this size, so the `with_transpose` row
//! converges toward the pure SoA row as `N` grows.
//!
//! Regenerate the table when either the base field's arithmetic or the
//! SIMD kernels change.
//!
//! # Examples
//!
//! ```
//! use gf2_core::field::FiniteField;
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::{BatchExtField, ExtConfig, QuadraticExt};
//!
//! struct Cfg;
//! impl ExtConfig for Cfg {
//!     type BaseField = Fp<65537>;
//!     const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
//! }
//! type Fq2 = QuadraticExt<Cfg>;
//!
//! let xs: Vec<Fq2> = (0..8)
//!     .map(|i| Fq2::new(Fp::new(i + 1), Fp::new(2 * i + 3)))
//!     .collect();
//! let ys: Vec<Fq2> = (0..8)
//!     .map(|i| Fq2::new(Fp::new(7 * i + 5), Fp::new(11 * i + 2)))
//!     .collect();
//!
//! let batch_x = BatchExtField::<Fp<65537>, 2>::from_quadratic::<Cfg>(&xs);
//! let batch_y = BatchExtField::<Fp<65537>, 2>::from_quadratic::<Cfg>(&ys);
//! let batch_z = batch_x.batch_mul_quadratic::<Cfg>(&batch_y);
//! let zs = batch_z.to_quadratic::<Cfg>();
//!
//! for ((x, y), z) in xs.iter().zip(ys.iter()).zip(zs.iter()) {
//!     assert_eq!(*x * *y, *z);
//! }
//! ```

use std::array;

use crate::field::{ConstField, FiniteField};
use crate::gfp::Fp;
use crate::gfpn::{ExtConfig, QuadraticExt};

// ---------------------------------------------------------------------------
// Core SoA batch container
// ---------------------------------------------------------------------------

/// Batch of GF(p^n) extension-field elements stored in Structure-of-Arrays
/// layout.
///
/// `coeffs[i]` holds the `i`-th coefficient of every element in the batch.
/// All inner vectors must have the same length; that common length is the
/// *batch size* reported by [`BatchExtField::len`]. The constructor
/// [`BatchExtField::new`] enforces this precondition.
///
/// `N` is the extension degree of the polynomial basis. For a quadratic
/// extension `N = 2`, for cubic `N = 3`, and so on. For scalar (base-field)
/// batches, use `N = 1`.
///
/// The type is generic over any [`FiniteField`] `F` so the same layout can
/// serve prime fields, tower extensions, or binary fields.
///
/// # Type Parameters
///
/// * `F` — the coefficient (base) field type.
/// * `N` — the number of coefficients per element (extension degree).
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::BatchExtField;
///
/// // A batch of three GF(p²)-like elements with hand-constructed coefficients.
/// let batch = BatchExtField::<Fp<7>, 2>::new([
///     vec![Fp::new(1), Fp::new(2), Fp::new(3)],
///     vec![Fp::new(4), Fp::new(5), Fp::new(6)],
/// ]);
/// assert_eq!(batch.len(), 3);
/// ```
#[derive(Clone, Debug)]
pub struct BatchExtField<F: FiniteField, const N: usize> {
    coeffs: [Vec<F>; N],
}

impl<F: FiniteField, const N: usize> BatchExtField<F, N> {
    /// Creates a new `BatchExtField` from `N` equal-length coefficient vectors.
    ///
    /// # Arguments
    ///
    /// * `coeffs` — an array of `N` `Vec<F>`s. Every inner vector must have
    ///   the same length.
    ///
    /// # Panics
    ///
    /// Panics if the inner vectors differ in length. The caller is expected
    /// to uphold this invariant; a panic on mismatch is the safest failure
    /// mode because a silent size disagreement would silently corrupt later
    /// arithmetic.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let batch = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(1), Fp::new(2)],
    ///     vec![Fp::new(3), Fp::new(4)],
    ///  ]);
    /// assert_eq!(batch.len(), 2);
    /// ```
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
    /// The `sample` argument supplies a base-field element from which the
    /// runtime-configured additive identity can be derived via
    /// [`FiniteField::zero_like`]. This mirrors the API style of
    /// [`crate::field::FieldVec::zeros_from`] and supports base fields whose
    /// identity element depends on runtime configuration.
    ///
    /// # Arguments
    ///
    /// * `len` — the number of extension-field elements to allocate.
    /// * `sample` — any base-field element; only `zero_like` is consulted.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let zeros = BatchExtField::<Fp<7>, 2>::zeros(4, &Fp::new(0));
    /// assert_eq!(zeros.len(), 4);
    /// for lane in zeros.coeff(0).iter().chain(zeros.coeff(1).iter()) {
    ///     assert!(lane.is_zero());
    /// }
    /// ```
    pub fn zeros(len: usize, sample: &F) -> Self {
        let coeffs: [Vec<F>; N] =
            array::from_fn(|_| (0..len).map(|_| sample.zero_like()).collect());
        Self { coeffs }
    }

    /// Returns the batch size (length of each coefficient vector).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let batch = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(1); 5],
    ///     vec![Fp::new(2); 5],
    /// ]);
    /// assert_eq!(batch.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        if N == 0 {
            0
        } else {
            self.coeffs[0].len()
        }
    }

    /// Returns `true` if the batch contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let batch = BatchExtField::<Fp<7>, 2>::new([vec![], vec![]]);
    /// assert!(batch.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a reference to the `i`-th coefficient lane as a slice.
    ///
    /// This is the primary entry point for code that wants to run SIMD or
    /// auto-vectorised kernels over a single coefficient position: the
    /// returned slice is contiguous and of length [`Self::len`].
    ///
    /// # Arguments
    ///
    /// * `i` — coefficient index in `0..N`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= N`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let batch = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(1), Fp::new(2)],
    ///     vec![Fp::new(3), Fp::new(4)],
    /// ]);
    /// assert_eq!(batch.coeff(1)[0].value(), 3);
    /// ```
    pub fn coeff(&self, i: usize) -> &[F] {
        &self.coeffs[i]
    }

    /// Returns `true` if all coefficient lanes have the same length.
    ///
    /// The constructor [`BatchExtField::new`] maintains this invariant, so
    /// valid instances always return `true`. The predicate is exposed so
    /// that callers who construct batches through a different path (e.g.
    /// deserialization) can validate before use.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let batch = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(1), Fp::new(2)],
    ///     vec![Fp::new(3), Fp::new(4)],
    /// ]);
    /// assert!(batch.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        if N == 0 {
            return true;
        }
        let expected = self.coeffs[0].len();
        self.coeffs.iter().all(|lane| lane.len() == expected)
    }
}

// ---------------------------------------------------------------------------
// Element-wise (coefficient-by-coefficient) arithmetic
// ---------------------------------------------------------------------------

impl<F: FiniteField, const N: usize> BatchExtField<F, N> {
    /// Element-wise addition across the batch.
    ///
    /// Returns a new batch whose `i`-th element equals `self[i] + other[i]`.
    /// Because the extension field's addition is coefficient-wise, the
    /// operation is a lane-parallel base-field add on every coefficient
    /// position.
    ///
    /// # Arguments
    ///
    /// * `other` — right-hand batch. Must match `self` in batch size.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    ///
    /// # Complexity
    ///
    /// `O(N · len)` base-field additions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let a = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(1), Fp::new(2)],
    ///     vec![Fp::new(3), Fp::new(4)],
    /// ]);
    /// let b = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(6), Fp::new(5)],
    ///     vec![Fp::new(4), Fp::new(3)],
    /// ]);
    /// let c = a.batch_add(&b);
    /// assert_eq!(c.coeff(0)[0].value(), 0); // (1 + 6) mod 7
    /// assert_eq!(c.coeff(1)[1].value(), 0); // (4 + 3) mod 7
    /// ```
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
    /// # Arguments
    ///
    /// * `other` — right-hand batch. Must match `self` in batch size.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    ///
    /// # Complexity
    ///
    /// `O(N · len)` base-field subtractions.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::BatchExtField;
    ///
    /// let a = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(5), Fp::new(5)],
    ///     vec![Fp::new(5), Fp::new(5)],
    /// ]);
    /// let b = BatchExtField::<Fp<7>, 2>::new([
    ///     vec![Fp::new(1), Fp::new(2)],
    ///     vec![Fp::new(3), Fp::new(4)],
    /// ]);
    /// let c = a.batch_sub(&b);
    /// assert_eq!(c.coeff(0)[0].value(), 4);
    /// assert_eq!(c.coeff(1)[1].value(), 1);
    /// ```
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

// ---------------------------------------------------------------------------
// Quadratic extension specialisations (N = 2)
// ---------------------------------------------------------------------------

impl<F: ConstField + SimdKaratsubaHook> BatchExtField<F, 2> {
    /// Converts a slice of [`QuadraticExt<C>`] elements into SoA form.
    ///
    /// This is the AoS→SoA transpose. Cost is `O(len)` base-field copies
    /// and two allocations (one per coefficient lane).
    ///
    /// # Type Parameters
    ///
    /// * `C` — extension config whose base field matches `F`.
    ///
    /// # Arguments
    ///
    /// * `elements` — slice of scalar extension-field elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{BatchExtField, ExtConfig, QuadraticExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    /// }
    /// type Fq2 = QuadraticExt<Cfg>;
    ///
    /// let xs = vec![
    ///     Fq2::new(Fp::new(1), Fp::new(2)),
    ///     Fq2::new(Fp::new(3), Fp::new(4)),
    /// ];
    /// let batch = BatchExtField::<Fp<7>, 2>::from_quadratic::<Cfg>(&xs);
    /// assert_eq!(batch.len(), 2);
    /// assert_eq!(batch.coeff(0)[0].value(), 1);
    /// assert_eq!(batch.coeff(1)[1].value(), 4);
    /// ```
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
    ///
    /// This is the SoA→AoS transpose. Cost is `O(len)` base-field copies
    /// and one allocation.
    ///
    /// # Type Parameters
    ///
    /// * `C` — extension config whose base field matches `F`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{BatchExtField, ExtConfig, QuadraticExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    /// }
    /// type Fq2 = QuadraticExt<Cfg>;
    ///
    /// let xs = vec![
    ///     Fq2::new(Fp::new(1), Fp::new(2)),
    ///     Fq2::new(Fp::new(3), Fp::new(4)),
    /// ];
    /// let batch = BatchExtField::<Fp<7>, 2>::from_quadratic::<Cfg>(&xs);
    /// let roundtrip = batch.to_quadratic::<Cfg>();
    /// assert_eq!(roundtrip, xs);
    /// ```
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
    /// For the specialised base field `Fp<65537>` on AVX2 hosts with the
    /// `simd` feature, the implementation routes through the fused AVX2
    /// Karatsuba kernel in `gf2-kernels-simd::fp65537::batch_karatsuba_fn`
    /// — all seven Karatsuba intermediates stay in registers across the
    /// 8-lane vector loop. For any other `F` (or when AVX2 is unavailable)
    /// the scalar straight-line combine runs element by element.
    /// [`crate::field::FieldVec`]'s element-wise ops share the same
    /// `Fp<65537>` SIMD kernels via the [`crate::gfp::SimdVecOps`] trait.
    /// Total cost: 3 base-field multiplications, 2 additions, 3
    /// subtractions, and one `mul_by_non_residue` per batch element.
    ///
    /// # Type Parameters
    ///
    /// * `C` — extension config supplying the non-residue β.
    ///
    /// # Arguments
    ///
    /// * `other` — right-hand batch. Must match `self` in batch size.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != other.len()`.
    ///
    /// # Complexity
    ///
    /// `O(len)`: 3 base-field multiplications per lane plus a constant
    /// number of base-field additions and one `mul_by_non_residue` per
    /// lane.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::{BatchExtField, ExtConfig, QuadraticExt};
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<7>;
    ///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1
    ///     fn mul_by_non_residue(x: Fp<7>) -> Fp<7> { -x }
    /// }
    /// type Fq2 = QuadraticExt<Cfg>;
    ///
    /// let a = vec![Fq2::new(Fp::new(3), Fp::new(2))];
    /// let b = vec![Fq2::new(Fp::new(4), Fp::new(5))];
    /// let batch_a = BatchExtField::<Fp<7>, 2>::from_quadratic::<Cfg>(&a);
    /// let batch_b = BatchExtField::<Fp<7>, 2>::from_quadratic::<Cfg>(&b);
    /// let batch_c = batch_a.batch_mul_quadratic::<Cfg>(&batch_b);
    /// let c = batch_c.to_quadratic::<Cfg>();
    /// assert_eq!(c[0], a[0] * b[0]);
    /// ```
    pub fn batch_mul_quadratic<C: ExtConfig<BaseField = F>>(&self, other: &Self) -> Self {
        assert_eq!(
            self.len(),
            other.len(),
            "batch_mul_quadratic: length mismatch ({} vs {})",
            self.len(),
            other.len()
        );

        let a0 = self.coeff(0);
        let a1 = self.coeff(1);
        let b0 = other.coeff(0);
        let b1 = other.coeff(1);

        let (out_c0, out_c1) = batch_karatsuba::<F, C>(a0, a1, b0, b1);
        Self {
            coeffs: [out_c0, out_c1],
        }
    }
}

// ---------------------------------------------------------------------------
// Karatsuba back-ends: scalar (generic) and SIMD-specialised (Fp<65537>)
// ---------------------------------------------------------------------------

/// Straight-line scalar Karatsuba combine over any `F: ConstField`.
///
/// The loop body is deliberately branchless and carries no cross-lane
/// dependencies so the compiler's auto-vectoriser has a chance to widen
/// it for amenable base fields.
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

        let v0 = a0i * b0i;
        let v1 = a1i * b1i;
        let cross = (a0i + a1i) * (b0i + b1i);

        out_c0.push(v0 + C::mul_by_non_residue(v1));
        out_c1.push(cross - v0 - v1);
    }
    (out_c0, out_c1)
}

/// Top-level Karatsuba combine, generic over any `F: ConstField`.
///
/// Dispatches through the sealed [`SimdKaratsubaHook`] trait: the default
/// impl for every `ConstField` returns `None`, and a specialised impl for
/// `Fp<65537>` invokes the AVX2 kernel in `gf2-kernels-simd` when
/// available. Other `F` and other runtime configurations fall back to
/// [`scalar_karatsuba`].
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

/// SIMD-dispatch hook for the Karatsuba combine used by
/// [`BatchExtField::batch_mul_quadratic`].
///
/// Every implementation returns `None` by default (scalar fallback).
/// `Fp<P>` provides a blanket impl that transparently routes through the
/// AVX2 kernel in `gf2-kernels-simd` when `P = 65537`, and returns
/// `None` for every other prime; the non-specialised path then runs the
/// scalar straight-line Karatsuba.
///
/// This trait exists only to enable dispatch; it is a crate-internal
/// extension point. External users should treat it as sealed — the
/// default method is the contract they see — and should not write their
/// own impls. All trait bounds on [`BatchExtField::batch_mul_quadratic`]
/// are satisfied automatically for `Fp<P>` through the blanket impl
/// below.
pub trait SimdKaratsubaHook: ConstField {
    /// Attempts to compute the Karatsuba combine for a quadratic
    /// extension element-wise over this base field using a SIMD kernel.
    ///
    /// Returns `None` when no SIMD kernel is available for `Self`, at
    /// which point the caller falls back to
    /// [`scalar_karatsuba`](super::batch::scalar_karatsuba) (which is
    /// purely internal; its semantics are folded into
    /// [`BatchExtField::batch_mul_quadratic`]).
    ///
    /// # Arguments
    ///
    /// * `a0`, `a1`, `b0`, `b1` — SoA coefficient lanes of two batches;
    ///   every slice must have identical length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gfpn::SimdKaratsubaHook;
    /// use gf2_core::gfpn::ExtConfig;
    ///
    /// struct Cfg;
    /// impl ExtConfig for Cfg {
    ///     type BaseField = Fp<65537>;
    ///     const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
    /// }
    ///
    /// let a0 = vec![Fp::<65537>::new(1), Fp::<65537>::new(2)];
    /// let a1 = vec![Fp::<65537>::new(3), Fp::<65537>::new(4)];
    /// let b0 = vec![Fp::<65537>::new(5), Fp::<65537>::new(6)];
    /// let b1 = vec![Fp::<65537>::new(7), Fp::<65537>::new(8)];
    /// let _ = <Fp<65537> as SimdKaratsubaHook>::try_simd_karatsuba::<Cfg>(
    ///     &a0, &a1, &b0, &b1,
    /// );
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` base-field operations (three multiplications, two adds,
    /// one non-residue scale, two subtractions). When specialised, each
    /// operation is an 8-lane AVX2 batch pass.
    #[inline]
    fn try_simd_karatsuba<C: ExtConfig<BaseField = Self>>(
        _a0: &[Self],
        _a1: &[Self],
        _b0: &[Self],
        _b1: &[Self],
    ) -> Option<(Vec<Self>, Vec<Self>)> {
        None
    }
}

/// Blanket default-None impl for every `Fp<P>` except the overridden
/// `Fp<65537>`. We cannot provide a blanket `impl<F: ConstField> SimdKaratsubaHook for F`
/// *and* a specialised impl for `Fp<65537>` without the `specialization`
/// feature, so the hook is only implemented for the concrete types where
/// we need it. Callers of [`BatchExtField::batch_mul_quadratic`] restrict
/// `F` to `ConstField + SimdKaratsubaHook`; the generic impl in this file
/// satisfies that bound for every `Fp<P>` by virtue of the blanket impl
/// below, which carries the default `None` unless explicitly overridden.
impl<const P: u64> SimdKaratsubaHook for Fp<P> {
    #[inline]
    fn try_simd_karatsuba<C: ExtConfig<BaseField = Self>>(
        a0: &[Self],
        a1: &[Self],
        b0: &[Self],
        b1: &[Self],
    ) -> Option<(Vec<Self>, Vec<Self>)> {
        // Compile-time branch: `P == 65537` folds to true exactly for
        // `Fp<65537>` and the entire branch is elided for other primes.
        if P == 65537 {
            return fp65537_simd_impl::<P, C>(a0, a1, b0, b1);
        }
        None
    }
}

/// AVX2 Karatsuba combine for `Fp<P>` specialised at `P = 65537`.
///
/// The caller (the `SimdKaratsubaHook` impl for `Fp<P>`) gates on
/// `P == 65537` at compile time. For that single monomorphisation,
/// `Fp<P>::raw_storage()` is known to equal the canonical value
/// because `R = 2^64 ≡ 1 (mod 65537)`, so we bypass the REDC round-trip
/// of `.value()`.
///
/// Returns `None` on non-AVX2 hardware or when the `simd` feature is
/// disabled; the caller falls back to the scalar Karatsuba path.
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

    // Pack through the shared `gfp::simd_ops::fp65537_pack` helper so that
    // FieldVec and BatchExtField both go through one implementation. For
    // `P = 65537`, `raw_storage()` returns the canonical value because
    // `R = 2^64 ≡ 1 (mod P)` (Montgomery coincides with canonical).
    let a0_u32 = fp65537_pack::<P>(a0);
    let a1_u32 = fp65537_pack::<P>(a1);
    let b0_u32 = fp65537_pack::<P>(b0);
    let b1_u32 = fp65537_pack::<P>(b1);

    // Single fused SIMD pass: reads each input slice once, writes each
    // output slice once, and keeps all seven Karatsuba intermediates in
    // AVX2 registers. This is the crucial speedup vs. a per-op
    // composition — eliminating nine heap buffers and six extra memory
    // round-trips. This fused pass is also the reason we keep a direct
    // path here rather than expressing the Karatsuba combine purely in
    // terms of `FieldVec::mul_vec`/`add_vec`/`sub_vec` calls: doing so
    // would re-introduce the per-op intermediate buffers and lose ~5×
    // of the measured speedup. FieldVec's element-wise ops still route
    // through the same underlying `Fp65537Fns` table via `SimdVecOps`,
    // so the two surfaces stay consistent.
    let beta_u32 = C::NON_RESIDUE.raw_storage() as u32;
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

#[cfg(not(feature = "simd"))]
#[inline]
fn fp65537_simd_impl<const P: u64, C: ExtConfig<BaseField = Fp<P>>>(
    _a0: &[Fp<P>],
    _a1: &[Fp<P>],
    _b0: &[Fp<P>],
    _b1: &[Fp<P>],
) -> Option<(Vec<Fp<P>>, Vec<Fp<P>>)> {
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfp::Fp;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Test configs
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Constructor / invariant tests
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Round-trip AoS↔SoA
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // batch_mul_quadratic: correctness on hand-computed cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_mul_small_handcrafted() {
        // (3 + 2u)·(4 + 5u) over GF(7), β = −1:
        //   v0 = 12 = 5, v1 = 10 = 3
        //   c0 = 5 + (−1)·3 = 2
        //   c1 = (3+2)(4+5) − 12 − 10 = 5·9 − 22 = 45 − 22 = 23 = 2
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
        // Exhaustively check every GF(7²) × GF(7²) pair in a single batch.
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

    // -----------------------------------------------------------------------
    // batch_add / batch_sub
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Proptest: Fp<65537>, β = 3 (matches the issue's success criterion)
    // -----------------------------------------------------------------------

    fn fq2_big_strategy() -> impl Strategy<Value = Fq2Big> {
        (0..65537u64, 0..65537u64).prop_map(|(c0, c1)| Fq2Big::new(Fp::new(c0), Fp::new(c1)))
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
        fn prop_roundtrip_fp65537(
            xs in proptest::collection::vec(fq2_big_strategy(), 0..=64)
        ) {
            let batch = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&xs);
            prop_assert!(batch.is_valid());
            let round = batch.to_quadratic::<CfgBeta3>();
            prop_assert_eq!(round, xs);
        }
    }
}
