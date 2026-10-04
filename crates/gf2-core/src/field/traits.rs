//! Trait hierarchy for finite field arithmetic.

use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// SIMD panel-kernel lane class of the panelized PLE base case, reported by
/// [`FiniteField::simd_ple_panel_lane`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlePanelLane {
    /// Byte-lane AVX2 panel kernel (`Fp<P>` for `P <= 251`).
    Byte,
    /// u16-lane AVX2 panel kernel (`Fp<P>` for `252 <= P < 65536`).
    U16,
}

/// Core trait for finite field elements.
///
/// Provides arithmetic operations, identity elements, and a wide accumulator type
/// for delayed-reduction dot products.
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
    fn characteristic(&self) -> Self::Characteristic;

    /// Returns the extension degree [F : F_p].
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
    fn inv(&self) -> Option<Self>;

    /// Returns the additive identity (zero) in the same field as `self`.
    fn zero_like(&self) -> Self;

    /// Returns the multiplicative identity (one) in the same field as `self`.
    fn one_like(&self) -> Self;

    /// Returns the additive identity when the type alone determines the field,
    /// as it does for every [`ConstField`] in this crate, and `None` otherwise.
    ///
    /// Lets a constructor fabricate a zero while holding neither an element nor
    /// a field descriptor, as in the product of an `m×0` and a `0×n` matrix.
    fn zero_hint() -> Option<Self> {
        None
    }

    /// Returns `floor(log2(|F|))` when the type alone determines the field
    /// cardinality, and `None` for runtime-context fields such as
    /// [`crate::gf2m::Gf2mElement`].
    ///
    /// An algorithm that branches on `|F|` takes a deterministic alternative
    /// on `None`.
    fn cardinality_log2_hint() -> Option<u32> {
        None
    }

    /// Converts this element to the wide accumulator type.
    fn to_wide(&self) -> Self::Wide;

    /// Multiplies two elements and returns the result in the wide type (before reduction).
    ///
    /// The product is stored in `Wide` so that many such products can be summed
    /// (up to [`max_unreduced_additions`](Self::max_unreduced_additions) times)
    /// before a single [`reduce_wide`](Self::reduce_wide) call brings the accumulator
    /// back into the field.
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

    /// Hook for a batched dot product of `a` and `b` over single-word GF(2^m).
    ///
    /// `None` (the default) sends the caller to the generic
    /// [`mul_product_sum_wide`](Self::mul_product_sum_wide) path. The scratch
    /// buffers are caller-owned so that they are reused across output cells.
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

    /// Hook for a SIMD dot product of `a` and `b` over a prime field.
    ///
    /// `None` (the default) sends the caller to the
    /// [`mul_product_sum_wide`](Self::mul_product_sum_wide) loop. An override
    /// clears the caller-owned packing buffers before use.
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

    /// Hook that packs `xs` into `out` as `u16` lanes for
    /// [`try_fp_simd_dot_packed_u16`](Self::try_fp_simd_dot_packed_u16),
    /// clearing `out` first. `None` (the default) means the field has no
    /// packed dot path.
    #[doc(hidden)]
    #[inline]
    fn try_pack_fp_medium_u16(xs: &[Self], out: &mut Vec<u16>) -> Option<()> {
        let _ = (xs, out);
        None
    }

    /// Dot product of two slices packed by
    /// [`try_pack_fp_medium_u16`](Self::try_pack_fp_medium_u16); `None` (the
    /// default) means the field has no packed dot path.
    #[doc(hidden)]
    #[inline]
    fn try_fp_simd_dot_packed_u16(a_packed: &[u16], b_packed: &[u16]) -> Option<Self> {
        let _ = (a_packed, b_packed);
        None
    }

    /// Hook for a SIMD dot product of same-length slices, returning the
    /// reduced sum; `None` (the default) sends the caller to the delayed
    /// reduction loop. A trait hook, so that generic callers need no
    /// `SimdVecOps` bound.
    #[doc(hidden)]
    #[inline]
    fn try_simd_dot_product(a: &[Self], b: &[Self]) -> Option<Self> {
        let _ = (a, b);
        None
    }

    /// Hook for a field that computes a whole matrix product itself.
    ///
    /// `a` is the `m × k` row-major left operand, `b_t` the `n × k` row-major
    /// transpose of the right operand, and `out` the `m × n` row-major
    /// destination. Returns `true` when `out` is populated and `false` (the
    /// default) when the caller should take the per-cell path.
    #[doc(hidden)]
    #[inline]
    fn try_simd_gemm_classical(
        a: &[Self],
        b_t: &[Self],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [Self],
    ) -> bool {
        let _ = (a, b_t, m, k, n, out);
        false
    }

    /// Non-allocating probe: `true` when callers should allocate contiguous
    /// operand scratch for
    /// [`try_simd_gemm_classical`](Self::try_simd_gemm_classical) or
    /// restructure an algorithm around it. Defaults to `false`.
    ///
    /// [`crate::field::matrix::gemm`] calls the hook without consulting this
    /// probe, so a field whose hook populates `out` may answer `false` here;
    /// the single-word GF(2^8) representations do.
    #[doc(hidden)]
    #[inline]
    fn has_simd_gemm_classical() -> bool {
        false
    }

    /// Hook returning a reducer that holds packed basis columns and computes
    /// `(residual, coeffs)` with `residual = v − Σ coeffs[j]·basis[j]` for
    /// cyclic decomposition; `None` (the default) sends the caller to the
    /// scalar reduction.
    #[doc(hidden)]
    #[inline]
    fn try_make_basis_reducer(
        n: usize,
    ) -> Option<Box<dyn crate::field::matrix::BasisReducer<Self>>> {
        let _ = n;
        None
    }

    /// Hook returning a [`crate::field::matrix::ChainPolyArith`] handle for
    /// the chain-polynomial updates of cyclic decomposition; `None` (the
    /// default) sends the caller to the `FieldPoly` path.
    #[doc(hidden)]
    #[inline]
    fn try_make_chain_poly_arith(
        n: usize,
    ) -> Option<Box<dyn crate::field::matrix::ChainPolyArith<Self>>> {
        let _ = n;
        None
    }

    /// Non-allocating probe for
    /// [`try_make_chain_poly_arith`](Self::try_make_chain_poly_arith).
    /// Defaults to `false`.
    #[doc(hidden)]
    #[inline]
    fn chain_poly_arith_available() -> bool {
        false
    }

    /// Hook for an accelerated `y[i] += a · x[i]`. Returns `true` when `y` is
    /// updated and `false` (the default) when the caller should run the
    /// scalar loop.
    #[doc(hidden)]
    #[inline]
    fn try_simd_axpy(y: &mut [Self], a: &Self, x: &[Self]) -> bool {
        let _ = (y, a, x);
        false
    }

    /// Hook that packs the row-major `m × k` matrix `a` once and returns a
    /// handle for repeated matrix-vector products; `None` (the default)
    /// means the field has no packed path.
    #[doc(hidden)]
    #[inline]
    fn try_prepack_matvec(
        a: &[Self],
        m: usize,
        k: usize,
    ) -> Option<Box<dyn crate::field::matrix::PackedMatvec<Self>>> {
        let _ = (a, m, k);
        None
    }

    /// Hook for a whole matrix-vector product `out = A · x`, with `a` the
    /// `m × k` row-major matrix, `x` of length `k` and `out` of length `m`.
    /// Returns `true` when `out` is populated and `false` (the default) when
    /// the caller should take the per-row path.
    #[doc(hidden)]
    #[inline]
    fn try_simd_matvec(a: &[Self], x: &[Self], m: usize, k: usize, out: &mut [Self]) -> bool {
        let _ = (a, x, m, k, out);
        false
    }

    /// Hook for a whole sparse-times-dense product `out = A · B`, with `A` in
    /// CSR form (`a_row_ptr` of length `m + 1`, `a_col_idx` and `a_values` of
    /// length `nnz`), `b` a `b_rows × n` row-major matrix and `out` the
    /// `m × n` row-major destination. Returns `true` when `out` is populated
    /// and `false` (the default) when the caller should take the per-row
    /// path.
    #[doc(hidden)]
    #[inline]
    fn try_simd_spmm(
        a_row_ptr: &[usize],
        a_col_idx: &[usize],
        a_values: &[Self],
        b: &[Self],
        b_rows: usize,
        n: usize,
        out: &mut [Self],
    ) -> bool {
        let _ = (a_row_ptr, a_col_idx, a_values, b, b_rows, n, out);
        false
    }

    /// Hook for the minimal polynomial of the square matrix `a`, computed by
    /// scalar Wiedemann over an extension field and descended to the base
    /// field.
    ///
    /// An override verifies that the descended polynomial annihilates `a`
    /// over the base field before returning `Some`. `None` (the default)
    /// sends the caller to the base-field path.
    #[doc(hidden)]
    #[inline]
    fn try_extension_wiedemann_minpoly(
        a: &crate::field::matrix::FieldMatrix<Self>,
    ) -> Option<crate::field::poly::FieldPoly<Self>> {
        let _ = a;
        None
    }

    /// Maximum number of wide-type additions before reduction is required to avoid overflow.
    ///
    /// Returns `usize::MAX` if overflow is impossible (e.g., binary fields where addition is XOR).
    /// Dot-product implementations use this to chunk their work.
    fn max_unreduced_additions() -> usize;

    /// Per-cell operand magnitude bound for the Strassen–Winograd recursion
    /// gate in [`crate::field::winograd`]: the largest integer value of a
    /// canonical element viewed in `Self::Wide`.
    ///
    /// A prime-field implementation must override this to return `p - 1`.
    /// The default `u128::MAX` is for binary extension fields, where addition
    /// is XOR and the gate is skipped.
    fn theorem_4_operand_bound() -> u128 {
        u128::MAX
    }

    /// Hook for a panelized PLE base case on the column window
    /// `[col_lo, col_hi)` of the `m`-row, `parent_cols`-column row-major
    /// `matrix`.
    ///
    /// On `Some(rank)` the window holds the in-place PLE compact storage in
    /// the convention of `crate::field::ple::ple_base_direct`, row swaps are
    /// applied to whole rows of `matrix` and recorded in `perm` (length `m`),
    /// and absolute pivot column indices are pushed to `pivot_cols` left to
    /// right. `None` (the default) sends the caller to `ple_base_direct`.
    #[doc(hidden)]
    #[inline]
    fn try_simd_ple_panel_base(
        matrix: &mut [Self],
        parent_cols: usize,
        m: usize,
        col_lo: usize,
        col_hi: usize,
        perm: &mut [usize],
        pivot_cols: &mut Vec<usize>,
    ) -> Option<usize> {
        let _ = (matrix, parent_cols, m, col_lo, col_hi, perm, pivot_cols);
        None
    }

    /// Non-allocating probe for
    /// [`try_simd_ple_panel_base`](Self::try_simd_ple_panel_base): the lane
    /// class of the registered kernel, or `None` (the default) when there is
    /// none.
    #[doc(hidden)]
    #[inline]
    fn simd_ple_panel_lane() -> Option<PlePanelLane> {
        None
    }
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
    /// for example, `Gf2mWide<N, Cfg>` with `Cfg::M >= 128`. Callers that
    /// need a non-panicking width probe should use [`Self::order_log2`].
    fn order() -> u128;

    /// Returns `floor(log2(order))`.
    ///
    /// The default computes `Self::order().ilog2()` and panics when `order()`
    /// does; an impl whose order exceeds `u128::MAX` must override it.
    fn order_log2() -> u32 {
        Self::order().ilog2()
    }
}

/// Blanket-implemented convenience methods for all [`FiniteField`] types.
pub trait FiniteFieldExt: FiniteField {
    /// Computes `self * self`.
    fn square(&self) -> Self {
        self.clone() * self.clone()
    }

    /// Computes `self^exp` using square-and-multiply.
    ///
    /// # Complexity
    ///
    /// O(log exp) field multiplications.
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
    /// # Panics
    ///
    /// Panics if `p^k` overflows `u64`.
    fn frobenius(&self, k: usize) -> Self
    where
        Self::Characteristic: Into<u64>,
    {
        let p: u64 = self.characteristic().into();
        let mut exp = 1u64;
        for _ in 0..k {
            exp = exp.checked_mul(p).expect("Frobenius exponent overflow");
        }
        self.pow(exp)
    }
}

impl<T: FiniteField> FiniteFieldExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::Gf2mField;

    fn generic_field_test<F: FiniteField>(a: F, b: F) {
        assert_eq!(a.clone() + b.clone(), b.clone() + a.clone());
        assert_eq!(a.clone() * b.clone(), b.clone() * a.clone());

        let zero = a.zero_like();
        assert_eq!(a.clone() + zero.clone(), a);

        let one = a.one_like();
        assert_eq!(a.clone() * one.clone(), a);

        assert!((a.clone() - a.clone()).is_zero());

        if !a.is_zero() {
            let inv = a.inv().expect("non-zero element has inverse");
            assert!((a.clone() * inv).is_one());
        }

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

    // Expected values: `@/citation/SageMath2026`, GF(2^4) with modulus x^4+x+1.
    #[test]
    fn test_square_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        assert_eq!(field.element(5).square(), field.element(2));
        assert_eq!(field.element(10).square(), field.element(8));
    }

    #[test]
    fn test_pow_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        assert_eq!(field.element(3).pow(5), field.element(6));
        assert_eq!(field.element(7).pow(10), field.element(7));
        assert_eq!(field.element(9).pow(13), field.element(4));
        assert_eq!(field.element(13).pow(4), field.element(11));
        // Fermat: pow(6, 15) = 1
        assert_eq!(field.element(6).pow(15), field.element(1));
        assert_eq!(field.element(5).pow(0), field.element(1));
        assert_eq!(field.element(1).pow(0), field.element(1));
    }

    #[test]
    fn test_frobenius_gf16() {
        let field = Gf2mField::new(4, 0b10011);

        assert_eq!(field.element(5).frobenius(1), field.element(2));
        assert_eq!(field.element(5).frobenius(2), field.element(4));
        assert_eq!(field.element(7).frobenius(1), field.element(6));
        assert_eq!(field.element(10).frobenius(1), field.element(8));
    }

    #[test]
    fn test_gf256_inv() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        let inv = a.inv().unwrap();
        assert!((a * inv).is_one());
    }

    #[test]
    fn test_gf256_pow() {
        let field = Gf2mField::gf256();
        // Fermat: a^255 = 1 for any non-zero a
        assert!(field.element(0x53).pow(255).is_one());
        let a = field.element(0x53);
        let a2 = a.square();
        let a4 = a2.square();
        assert_eq!(a.pow(7), a.clone() * a2 * a4);
    }

    #[test]
    fn test_gf256_square() {
        let field = Gf2mField::gf256();
        let a = field.element(0x53);
        assert_eq!(a.square(), a.clone() * a);
    }

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
        assert_eq!(b.value(), 3);
    }

    #[test]
    fn test_mixed_receiver_ops() {
        let field = Gf2mField::new(4, 0b10011);
        let a = field.element(5);
        let b = field.element(3);

        assert_eq!(a.clone() + &b, &a + &b);
        assert_eq!(a.clone() - &b, &a - &b);
        assert_eq!(a.clone() * &b, &a * &b);
        assert_eq!(a.clone() / &b, &a / &b);
    }
}
