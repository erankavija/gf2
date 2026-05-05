//! Trait hierarchy for finite field arithmetic.
//!
//! This module defines a generic abstraction for finite fields that supports:
//! - Binary extension fields GF(2^m) via [`Gf2mElement`](crate::gf2m::Gf2mElement)
//! - Prime fields GF(p) (future)
//! - Tower extension fields GF(p^n) (future)
//!
//! # Trait Overview
//!
//! - [`FiniteField`]: Core trait with arithmetic, identity elements, and wide accumulation.
//! - [`ConstField`]: Extension for fields whose elements are `Copy` and have const-like constructors.
//! - [`FiniteFieldExt`]: Blanket-implemented convenience methods (`square`, `pow`, `frobenius`).

use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// Core trait for finite field elements.
///
/// Provides arithmetic operations, identity elements, and a wide accumulator type
/// for delayed-reduction dot products.
///
/// # Associated Types
///
/// - `Characteristic`: The field characteristic (e.g., `u64` for small primes).
/// - `Wide`: A wider accumulator type that can hold sums of products before reduction.
///
/// # Examples
///
/// ```
/// use gf2_core::field::FiniteField;
/// use gf2_core::gf2m::Gf2mField;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let a = field.element(5);
/// let b = field.element(3);
///
/// assert!(!a.is_zero());
/// assert!(field.zero().is_zero());
///
/// let inv = a.inv().expect("non-zero element has inverse");
/// assert!((a * inv).is_one());
/// ```
pub trait FiniteField:
    Sized
    + Clone
    + PartialEq
    + Eq
    + Hash
    + Debug
    + Add<Output = Self>
    + for<'a> Add<&'a Self, Output = Self>
    + Sub<Output = Self>
    + for<'a> Sub<&'a Self, Output = Self>
    + Mul<Output = Self>
    + for<'a> Mul<&'a Self, Output = Self>
    + Div<Output = Self>
    + for<'a> Div<&'a Self, Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + for<'a> AddAssign<&'a Self>
{
    /// The field characteristic (prime p such that p·1 = 0).
    type Characteristic: Clone + Debug + PartialEq + Eq;

    /// A wider type for accumulating sums of products without intermediate reduction.
    ///
    /// For binary fields, `Wide = Self` since XOR never overflows.
    /// For prime fields, this is typically a double-width integer (e.g., `u128` for `u64` elements).
    type Wide: Clone + Add<Output = Self::Wide> + AddAssign;

    /// Returns the field characteristic.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let a = Gf2mField::new(4, 0b10011).element(5);
    /// assert_eq!(a.characteristic(), 2u64);
    /// ```
    fn characteristic(&self) -> Self::Characteristic;

    /// Returns the extension degree [F : F_p].
    ///
    /// For a prime field GF(p), this returns 1.
    /// For GF(p^m), this returns m.
    ///
    /// # Panics
    ///
    /// May panic if the extension degree is not statically known (e.g., runtime-configured fields).
    fn extension_degree(&self) -> usize;

    /// Returns `true` if this element is the additive identity (zero).
    fn is_zero(&self) -> bool;

    /// Returns `true` if this element is the multiplicative identity (one).
    fn is_one(&self) -> bool;

    /// Computes the multiplicative inverse, or `None` if this element is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(7);
    /// let inv = a.inv().unwrap();
    /// assert!((a * inv).is_one());
    /// ```
    fn inv(&self) -> Option<Self>;

    /// Returns the additive identity (zero) in the same field as `self`.
    fn zero_like(&self) -> Self;

    /// Returns the multiplicative identity (one) in the same field as `self`.
    fn one_like(&self) -> Self;

    /// Returns the additive identity when the field's context is known
    /// purely from the type (no runtime field witness required).
    ///
    /// Implementations that satisfy [`ConstField`] return `Some(Self::zero())`;
    /// every other `FiniteField` returns `None`. This is a *static escape
    /// hatch* used by constructors that need to fabricate a zero but hold
    /// neither an existing element nor a field descriptor — the canonical
    /// example is multiplying an `m×0` matrix by a `0×n` matrix with both
    /// factors carrying empty storage.
    ///
    /// Override this in any [`ConstField`] impl to return `Some(Self::zero())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// // Compile-time field: zero is always available.
    /// assert_eq!(<Fp<7> as FiniteField>::zero_hint(), Some(Fp::<7>::new(0)));
    ///
    /// // Runtime-context field: no zero without a witness.
    /// assert!(<Gf2mElement as FiniteField>::zero_hint().is_none());
    /// ```
    fn zero_hint() -> Option<Self> {
        None
    }

    /// Returns `floor(log2(|F|))` when the field's cardinality can be
    /// determined statically from the type, or `None` for runtime-context
    /// fields (e.g. [`crate::gf2m::Gf2mElement`]) whose extension degree
    /// is only known at runtime.
    ///
    /// This is a *static escape hatch* used by algorithms that branch on
    /// the field cardinality `q` (e.g. the Las-Vegas Keller–Gehrig
    /// charpoly path in [`crate::field::charpoly`], whose probabilistic
    /// guarantee `q > 2 n²` is meaningless when `q` is unknown). Callers
    /// receive `None` for any field that cannot supply a compile-time
    /// cardinality and must fall back to a deterministic alternative.
    ///
    /// Every [`ConstField`] implementation in this crate overrides this
    /// to return `Some(<Self as ConstField>::order_log2())`. Runtime-
    /// context `FiniteField` impls keep the default `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// // Compile-time field: cardinality known.
    /// assert_eq!(<Fp<7> as FiniteField>::cardinality_log2_hint(), Some(2));
    ///
    /// // Runtime-context field: no static cardinality available.
    /// assert!(<Gf2mElement as FiniteField>::cardinality_log2_hint().is_none());
    /// ```
    fn cardinality_log2_hint() -> Option<u32> {
        None
    }

    /// Converts this element to the wide accumulator type.
    ///
    /// For binary extension fields GF(2^m), `Wide = Self` so this is a clone.
    /// For prime fields GF(p), this widens to a double-width integer (e.g., `u64` → `u128`)
    /// to leave headroom for accumulating sums of products without overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// // GF(2^4): Wide = Self, so to_wide is identity
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(7);
    /// let wide = a.to_wide();
    /// let back = <gf2_core::gf2m::Gf2mElement as FiniteField>::reduce_wide(&wide);
    /// assert_eq!(back, a);
    /// ```
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    ///
    /// // Fp<7>: Wide = u128, so to_wide widens the canonical value
    /// let a = Fp::<7>::new(5);
    /// let wide: u128 = a.to_wide();
    /// assert_eq!(wide, 5u128);
    /// ```
    fn to_wide(&self) -> Self::Wide;

    /// Multiplies two elements and returns the result in the wide type (before reduction).
    ///
    /// The product is stored in `Wide` so that many such products can be summed
    /// (up to [`max_unreduced_additions`](Self::max_unreduced_additions) times)
    /// before a single [`reduce_wide`](Self::reduce_wide) call brings the accumulator
    /// back into the field.
    ///
    /// # Arguments
    ///
    /// * `rhs` — The right-hand multiplicand.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(5);
    /// let b = field.element(3);
    /// let wide = a.mul_to_wide(&b);
    /// let reduced = <gf2_core::gf2m::Gf2mElement as FiniteField>::reduce_wide(&wide);
    /// assert_eq!(reduced, a * b);
    /// ```
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = Fp::<7>::new(5);
    /// let b = Fp::<7>::new(4);
    /// let wide: u128 = a.mul_to_wide(&b);
    /// // 5 * 4 = 20 stored unreduced in u128
    /// assert_eq!(wide, 20u128);
    /// assert_eq!(Fp::<7>::reduce_wide(&wide), Fp::<7>::new(6)); // 20 mod 7 = 6
    /// ```
    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide;

    /// Multiplies two elements in the representation preferred by delayed
    /// product-sum kernels.
    ///
    /// The default implementation is the canonical unreduced product from
    /// [`mul_to_wide`](Self::mul_to_wide). Prime fields whose storage is not
    /// canonical (for example Montgomery-form [`crate::gfp::Fp`]) may override
    /// this to accumulate raw storage products instead, provided
    /// [`reduce_product_sum_wide`](Self::reduce_product_sum_wide) converts the
    /// accumulated representation back to a valid field element.
    ///
    /// # Correctness contract
    ///
    /// The magnitude of each returned product must be bounded by
    /// `theorem_4_operand_bound()²`, so summing at most
    /// [`max_unreduced_additions`](Self::max_unreduced_additions) such products
    /// cannot overflow `Self::Wide`.
    #[inline]
    fn mul_product_sum_wide(&self, rhs: &Self) -> Self::Wide {
        self.mul_to_wide(rhs)
    }

    /// Reduces a wide accumulator back to a field element.
    ///
    /// After accumulating up to [`max_unreduced_additions`](Self::max_unreduced_additions)
    /// wide products, call this to obtain the canonical field element.
    ///
    /// # Arguments
    ///
    /// * `wide` — The accumulated wide value to reduce.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(5);
    /// let wide = a.to_wide();
    /// let back = <gf2_core::gf2m::Gf2mElement as FiniteField>::reduce_wide(&wide);
    /// assert_eq!(back, a);
    /// ```
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    ///
    /// // Accumulate two products in u128, then reduce once
    /// let a = Fp::<7>::new(5);
    /// let b = Fp::<7>::new(4);
    /// let c = Fp::<7>::new(3);
    /// let d = Fp::<7>::new(6);
    /// let wide = a.mul_to_wide(&b) + c.mul_to_wide(&d);
    /// // 5*4 + 3*6 = 20 + 18 = 38 → 38 mod 7 = 3
    /// assert_eq!(Fp::<7>::reduce_wide(&wide), Fp::<7>::new(3));
    /// ```
    fn reduce_wide(wide: &Self::Wide) -> Self;

    /// Reduces a delayed product-sum accumulator back to a field element.
    ///
    /// This is paired with
    /// [`mul_product_sum_wide`](Self::mul_product_sum_wide). The default
    /// implementation reduces the same canonical accumulator as
    /// [`reduce_wide`](Self::reduce_wide). Implementations that accumulate a
    /// storage-domain product sum must override both methods together and
    /// document the representation-specific proof.
    #[inline]
    fn reduce_product_sum_wide(wide: &Self::Wide) -> Self {
        Self::reduce_wide(wide)
    }

    /// Hidden matrix-kernel hook for GF(2^m) batch product sums.
    ///
    /// Most fields return `None` and continue through the generic
    /// [`mul_product_sum_wide`](Self::mul_product_sum_wide) delayed-reduction
    /// path. Single-word GF(2^m) implementations for `m ∈ {8, 16, 32}` may
    /// override this hook to export operands to canonical `u64` lanes, call the
    /// batched carry-less multiply kernel once for the whole dot product, and
    /// XOR-reduce the products back into one field element. The scratch buffers
    /// are caller-owned so matrix multiplication can reuse allocations across
    /// output cells.
    ///
    /// This is crate-internal API surface for `FieldMatrix` performance work;
    /// downstream code should use the public matrix/vector APIs instead.
    #[doc(hidden)]
    #[inline]
    fn try_gf2m_u64_batch_dot_product(
        a: &[Self],
        b: &[Self],
        zero: &Self,
        scratch_a: &mut Vec<u64>,
        scratch_b: &mut Vec<u64>,
        scratch_products: &mut Vec<u64>,
    ) -> Option<Self> {
        let _ = (a, b, zero, scratch_a, scratch_b, scratch_products);
        None
    }

    /// Hidden matrix-kernel hook for prime-field SIMD batch dot products.
    ///
    /// Companion to [`try_gf2m_u64_batch_dot_product`] for `Fp<P>` instead
    /// of `GF(2^m)`. Most fields (including the GF(2^m) families and
    /// generic-Montgomery primes) return `None` and the caller falls back
    /// through the [`mul_product_sum_wide`](Self::mul_product_sum_wide)
    /// delayed-reduction loop. The medium-prime `Fp<P>` impl
    /// (`P ∈ (251, 65536)`) overrides this hook to route the dot through
    /// the AVX2 16-lane u16 Barrett kernel in
    /// `gf2-kernels-simd::fp_medium`, accumulating in 64-bit lanes and
    /// reducing once at the very end.
    ///
    /// `scratch_a` / `scratch_b` are caller-owned packing buffers reused
    /// across the surrounding GEMM traversal so the SIMD path pays its
    /// `u64 → u16` truncation cost only once per matrix cell. The hook
    /// is responsible for `clear()`-ing each buffer before use.
    ///
    /// This is crate-internal API surface for `FieldMatrix` performance
    /// work; downstream code should use the public matrix/vector APIs.
    #[doc(hidden)]
    #[inline]
    fn try_fp_simd_dot_product(
        a: &[Self],
        b: &[Self],
        scratch_a: &mut Vec<u16>,
        scratch_b: &mut Vec<u16>,
    ) -> Option<Self> {
        let _ = (a, b, scratch_a, scratch_b);
        None
    }

    /// GEMM-internal hook: pack a slice of `Fp<P>` raw storage values
    /// into `Vec<u16>` once, so the inner dot kernel can read pre-packed
    /// canonical-storage lanes without re-truncating per cell.
    ///
    /// Returns `Some(())` if the field is eligible for the medium-prime
    /// fast path (`Fp<P>` with `P ∈ (251, 65536)`); returns `None` for
    /// every other field, signalling that the GEMM caller should not
    /// attempt the pre-pack-then-SIMD-dot path. Implementations are
    /// responsible for `clear()`-ing `out` before pushing.
    ///
    /// This is crate-internal API surface for `FieldMatrix` performance
    /// work; downstream code should use the public matrix/vector APIs.
    #[doc(hidden)]
    #[inline]
    fn try_pack_fp_medium_u16(xs: &[Self], out: &mut Vec<u16>) -> Option<()> {
        let _ = (xs, out);
        None
    }

    /// GEMM-internal hook: SIMD batch dot product using pre-packed u16
    /// canonical-storage slices.
    ///
    /// Companion to [`try_pack_fp_medium_u16`]. Given two `&[u16]` slices
    /// produced by the pack hook, computes the storage-domain dot
    /// product, applies one Montgomery REDC, and returns the result as
    /// a `Self`. Returns `None` for every field that does not implement
    /// the medium-prime fast path.
    #[doc(hidden)]
    #[inline]
    fn try_fp_simd_dot_packed_u16(a_packed: &[u16], b_packed: &[u16]) -> Option<Self> {
        let _ = (a_packed, b_packed);
        None
    }

    /// Maximum number of wide-type additions before reduction is required to avoid overflow.
    ///
    /// Returns `usize::MAX` if overflow is impossible (e.g., binary fields where addition is XOR).
    /// For prime fields, this is computed as `floor(u128::MAX / (P-1)^2)` — the number of
    /// unreduced `mul_to_wide` products that can be safely summed in a `u128` accumulator
    /// without wrapping. Dot-product implementations use this to chunk their work.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// // GF(2^m): XOR never overflows, so no reduction limit
    /// assert_eq!(<Gf2mElement as FiniteField>::max_unreduced_additions(), usize::MAX);
    /// ```
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    ///
    /// // Small prime: many products fit in u128
    /// let k = <Fp<7> as FiniteField>::max_unreduced_additions();
    /// assert!(k > 1_000_000);
    ///
    /// // Large prime near 2^63: only a handful of products fit
    /// let k2 = <Fp<9_223_372_036_854_775_783> as FiniteField>::max_unreduced_additions();
    /// assert!(k2 >= 1 && k2 < 100);
    /// ```
    fn max_unreduced_additions() -> usize;

    /// Per-cell operand magnitude bound used by the Strassen–Winograd
    /// theorem-4 recursion gate in [`crate::field::winograd`].
    ///
    /// Returns the maximum absolute integer value a canonical field
    /// element can take when viewed as an element of `Self::Wide`. For
    /// prime fields this is `p - 1`; for binary extension fields where
    /// `Wide = Self` and addition is XOR the theorem is vacuous, and the
    /// sentinel `u128::MAX` signals to callers that the bound check can
    /// be skipped entirely (matching the behaviour of
    /// `max_unreduced_additions() == usize::MAX`).
    ///
    /// Used by `gemm_winograd_inner` to decide whether the theorem-4
    /// cell-magnitude bound at the next recursion depth would exceed the
    /// field's delayed-reduction headroom:
    ///
    /// ```text
    /// theorem_4_bound(level + 1, k, theorem_4_operand_bound())
    ///     > max_unreduced_additions() * theorem_4_operand_bound()²
    /// ```
    ///
    /// When true, the recursion falls back to the classical gemm at the
    /// current level.
    ///
    /// # Overrides
    ///
    /// **Every prime-field implementation MUST override this** to return
    /// `(P - 1) as u128` (or the moral equivalent for the specific
    /// field) — the default value is calibrated for binary fields, where
    /// the theorem is vacuous, and is too loose for prime fields.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::FiniteField;
    /// use gf2_core::gfp::Fp;
    /// use gf2_core::gf2m::Gf2mElement;
    ///
    /// assert_eq!(<Fp<7> as FiniteField>::theorem_4_operand_bound(), 6);
    /// assert_eq!(
    ///     <Gf2mElement as FiniteField>::theorem_4_operand_bound(),
    ///     u128::MAX,
    /// );
    /// ```
    fn theorem_4_operand_bound() -> u128 {
        u128::MAX
    }

    /// Square-matrix size at or below which Strassen–Winograd recursion
    /// falls back to the classical blocked `gemm`. Empirically tuned per
    /// field: the default `128` is calibrated against Mersenne-31 and
    /// `Gf2mWide<1, Gf2m8>` in `benches/strassen_threshold.rs` at
    /// `n = 2048`, where both fields cross over at ≈ 128.
    ///
    /// Fields with materially heavier scalar MACs than Mersenne-31 — for
    /// example `Goldilocks` (128-bit reduction path) — should override
    /// this to a smaller value because a single multiply costs more, so
    /// trading multiplies for block adds pays off earlier. Fields with
    /// much lighter MACs (e.g. GF(2) bit-packed) should override upwards
    /// because Winograd's block-add bookkeeping never beats the native
    /// XOR-heavy inner loop at small sizes.
    ///
    /// This knob is **soft** — correctness is independent of it. The
    /// Winograd implementation is bit-exact equal to the classical
    /// `gemm` at every threshold value, as asserted by the property
    /// tests in `src/field/winograd.rs`.
    const WINOGRAD_THRESHOLD: usize = 128;

    /// Base-case threshold for the block-recursive triangular primitives
    /// in [`crate::field::triangular`] (`trsm`, `trmm`, `trtri`, `trtrm`,
    /// per Dumas–Pernet §2.1 algorithms 2.1–2.4). Recursion stops at sizes
    /// `≤ TRI_BASE_THRESHOLD` and falls through to a small direct loop
    /// (back-substitution for `trsm`, schoolbook for `trmm`/`trtri`).
    ///
    /// The default `32` is empirically chosen for Mersenne-31 and small
    /// `Gf2mWide` instances; fields with materially heavier per-MAC cost
    /// (e.g. `Fp<P>` with `P` close to `2^63`, or tower extensions) may
    /// override this to a smaller value because the recursion's `gemm`
    /// dispatch overhead is amortised over fewer cells. Lighter fields
    /// (e.g. GF(2) bit-packed) may benefit from a larger threshold.
    ///
    /// Like [`Self::WINOGRAD_THRESHOLD`], this knob is **soft**:
    /// correctness is independent of it. Property tests in
    /// `src/field/triangular.rs` exercise `trsm`/`trmm`/`trtri`/`trtrm`
    /// at sizes that straddle the threshold and assert bit-exact
    /// agreement with the classical `gemm`-of-dense expansion.
    const TRI_BASE_THRESHOLD: usize = 32;
}

/// Extension of [`FiniteField`] for types that are `Copy` and have zero-cost identity constructors.
///
/// This is appropriate for fields with compile-time-known parameters (const generics or
/// zero-sized config types), where elements don't carry runtime field context.
pub trait ConstField: FiniteField + Copy {
    /// Returns the additive identity (zero).
    fn zero() -> Self;

    /// Returns the multiplicative identity (one).
    fn one() -> Self;

    /// Returns the number of elements in the field.
    ///
    /// # Panics
    ///
    /// Impls are permitted to panic when the order exceeds `u128::MAX` —
    /// for example, `Gf2mWide<N, Cfg>` with `Cfg::M >= 128` (GF(2^256)
    /// and above). Callers that need a non-panicking width probe should
    /// use [`Self::order_log2`] first.
    fn order() -> u128;

    /// Returns `floor(log2(order))`. Always safe to call, even for
    /// fields whose order exceeds `u128::MAX`.
    ///
    /// The default implementation computes `Self::order().ilog2()` and
    /// therefore panics when `order()` does. Impls whose order is too
    /// large to fit in a `u128` (e.g. `Gf2mWide<4, _>` for `GF(2^256)`)
    /// MUST override this method so that callers can still query the
    /// field's bit-width without triggering the overflow panic on
    /// `order()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::ConstField;
    /// use gf2_core::gfp::Fp;
    ///
    /// assert_eq!(<Fp<7> as ConstField>::order_log2(), 2); // floor(log2(7))
    /// assert_eq!(<Fp<17> as ConstField>::order_log2(), 4);
    /// ```
    fn order_log2() -> u32 {
        Self::order().ilog2()
    }
}

/// Blanket-implemented convenience methods for all [`FiniteField`] types.
///
/// Provides `square`, `pow`, and `frobenius` built on top of the core trait.
pub trait FiniteFieldExt: FiniteField {
    /// Computes `self * self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(5);
    /// let sq = a.square();
    /// assert_eq!(sq, a.clone() * a);
    /// ```
    fn square(&self) -> Self {
        self.clone() * self.clone()
    }

    /// Computes `self^exp` using square-and-multiply.
    ///
    /// # Complexity
    ///
    /// O(log exp) field multiplications.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(6);
    /// // Fermat's little theorem: a^(2^4 - 1) = 1 for non-zero a
    /// assert!(a.pow(15).is_one());
    /// ```
    fn pow(&self, exp: u64) -> Self {
        if exp == 0 {
            return self.one_like();
        }

        let mut result = self.one_like();
        let mut base = self.clone();
        let mut e = exp;

        while e > 0 {
            if e & 1 == 1 {
                result = result * base.clone();
            }
            e >>= 1;
            if e > 0 {
                base = base.clone() * base.clone();
            }
        }

        result
    }

    /// Computes the k-th iterated Frobenius endomorphism: `self^(p^k)`.
    ///
    /// The Frobenius map φ: x → x^p is a field automorphism of GF(p^m).
    /// This computes φ^k(x) = x^(p^k).
    ///
    /// # Arguments
    ///
    /// * `k` - Number of Frobenius iterations.
    ///
    /// # Panics
    ///
    /// Panics if the characteristic cannot be converted to `u64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::{FiniteField, FiniteFieldExt};
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let a = field.element(5);
    /// // In GF(2^4): frobenius(a, 1) = a^2
    /// assert_eq!(a.frobenius(1), a.square());
    /// ```
    fn frobenius(&self, k: usize) -> Self
    where
        Self::Characteristic: Into<u64>,
    {
        let p: u64 = self.characteristic().into();
        // Compute p^k as exponent
        let mut exp = 1u64;
        for _ in 0..k {
            exp = exp.checked_mul(p).expect("Frobenius exponent overflow");
        }
        self.pow(exp)
    }
}

// Blanket implementation: every FiniteField automatically gets FiniteFieldExt
impl<T: FiniteField> FiniteFieldExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::Gf2mField;

    // --- Generic function test proving trait usability ---

    fn generic_field_test<F: FiniteField>(a: F, b: F) {
        // Commutativity: a + b == b + a
        assert_eq!(a.clone() + b.clone(), b.clone() + a.clone());
        // Commutativity: a * b == b * a
        assert_eq!(a.clone() * b.clone(), b.clone() * a.clone());

        // Additive identity
        let zero = a.zero_like();
        assert_eq!(a.clone() + zero.clone(), a);

        // Multiplicative identity
        let one = a.one_like();
        assert_eq!(a.clone() * one.clone(), a);

        // Subtraction (additive inverse)
        assert!((a.clone() - a.clone()).is_zero());

        // Multiplicative inverse (if non-zero)
        if !a.is_zero() {
            let inv = a.inv().expect("non-zero element has inverse");
            assert!((a.clone() * inv).is_one());
        }

        // Zero has no inverse
        assert!(zero.inv().is_none());
    }

    #[test]
    fn test_generic_field_gf16() {
        let field = Gf2mField::new(4, 0b10011);
        generic_field_test(field.element(5), field.element(3));
        generic_field_test(field.element(0), field.element(7));
        generic_field_test(field.element(1), field.element(15));
    }

    #[test]
    fn test_generic_field_gf256() {
        let field = Gf2mField::gf256();
        generic_field_test(field.element(0x53), field.element(0xCA));
    }

    // --- FiniteFieldExt: square() ---
    // SageMath: GF(2^4, 'a', modulus=x^4+x+1)

    #[test]
    fn test_square_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        // square(5) = 2: a^2+1 squared = a^2+a+1 ... SageMath says 2
        assert_eq!(field.element(5).square(), field.element(2));
        // square(10) = 8
        assert_eq!(field.element(10).square(), field.element(8));
    }

    // --- FiniteFieldExt: pow() ---

    #[test]
    fn test_pow_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        // pow(3, 5) = 6
        assert_eq!(field.element(3).pow(5), field.element(6));
        // pow(7, 10) = 7
        assert_eq!(field.element(7).pow(10), field.element(7));
        // pow(9, 13) = 4
        assert_eq!(field.element(9).pow(13), field.element(4));
        // pow(13, 4) = 11
        assert_eq!(field.element(13).pow(4), field.element(11));
        // Fermat: pow(6, 15) = 1
        assert_eq!(field.element(6).pow(15), field.element(1));
        // pow(a, 0) = 1 for any non-zero a
        assert_eq!(field.element(5).pow(0), field.element(1));
        assert_eq!(field.element(1).pow(0), field.element(1));
    }

    // --- FiniteFieldExt: frobenius() ---

    #[test]
    fn test_frobenius_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        // frobenius(5, 1) = 5^2 = 2
        assert_eq!(field.element(5).frobenius(1), field.element(2));
        // frobenius(5, 2) = 5^4 = 4
        assert_eq!(field.element(5).frobenius(2), field.element(4));
        // frobenius(7, 1) = 7^2 = 6
        assert_eq!(field.element(7).frobenius(1), field.element(6));
        // frobenius(10, 1) = 10^2 = 8
        assert_eq!(field.element(10).frobenius(1), field.element(8));
    }

    // --- GF(2^8) with polynomial x^8+x^4+x^3+x^2+1 (0b100011101) ---

    #[test]
    fn test_gf256_inv() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        // Verify a * inv(a) = 1
        let inv = a.inv().unwrap();
        assert!((a * inv).is_one());
    }

    #[test]
    fn test_gf256_pow() {
        let field = Gf2mField::gf256();
        // Fermat: a^255 = 1 for any non-zero a
        assert!(field.element(0x53).pow(255).is_one());
        // pow consistency: a^7 = a * a^2 * a^4
        let a = field.element(0x53);
        let a2 = a.square();
        let a4 = a2.square();
        assert_eq!(a.pow(7), a.clone() * a2 * a4);
    }

    #[test]
    fn test_gf256_square() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        // square(a) = a * a
        assert_eq!(a.square(), a.clone() * a);
    }

    // --- Trait method consistency ---

    #[test]
    fn test_characteristic_and_extension() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(5);
        assert_eq!(a.characteristic(), 2u64);
        assert_eq!(a.extension_degree(), 4);
    }

    #[test]
    fn test_wide_roundtrip() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(7);
        let wide = a.to_wide();
        let back = <crate::gf2m::Gf2mElement as FiniteField>::reduce_wide(&wide);
        assert_eq!(back, a);
    }

    #[test]
    fn test_addassign() {
        let field = Gf2mField::new(4, 0b10011);
        let mut a = field.element(5);
        let b = field.element(3);
        a += b;
        assert_eq!(a, field.element(5 ^ 3));
    }

    #[test]
    fn test_addassign_ref() {
        let field = Gf2mField::new(4, 0b10011);
        let mut a = field.element(5);
        let b = field.element(3);
        a += &b;
        assert_eq!(a, field.element(5 ^ 3));
        // b is still valid
        assert_eq!(b.value(), 3);
    }

    #[test]
    fn test_mixed_receiver_ops() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(5);
        let b = field.element(3);

        // owned + &ref
        assert_eq!(a.clone() + &b, &a + &b);
        assert_eq!(a.clone() - &b, &a - &b);
        assert_eq!(a.clone() * &b, &a * &b);
        assert_eq!(a.clone() / &b, &a / &b);
    }
}
