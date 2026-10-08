//! GF(2^m) field multiplication kernels built on PCLMULQDQ carry-less
//! multiplication.

/// GF(2^m) field multiplication: `(a, b, m, primitive_poly)` to
/// `a * b mod primitive_poly`, for `1 <= m <= 64` and `a`, `b` below `2^m`.
///
/// `primitive_poly` holds the generator's coefficients of `x^0..x^63`. The
/// leading coefficient `x^m` is implied; for `m <= 63` it may also be set.
///
/// # Panics
/// Panics if `m` is outside `1..=64`.
pub type Gf2mMulFn = fn(u64, u64, usize, u64) -> u64;

/// Raw carry-less multiplication: the full 128-bit product of two 64-bit
/// polynomials, without reduction.
pub type ClmulFn = fn(u64, u64) -> u128;

/// Batch carry-less multiplication function (no reduction).
///
/// Computes `out[i] = a[i] * b[i]` (carry-less, no reduction) for each index.
///
/// # Panics
/// Panics if slices have different lengths.
pub type ClmulBatchFn = fn(&[u64], &[u64], &mut [u128]);

/// All-in-one carry-less multiply + Barrett reduce function.
///
/// `(a, b, mu, modulus, degree)` to `a * b mod P(x)`, where `a` and `b` are
/// `m`-bit field elements, `mu = x^(2m) / P(x)` is the Barrett constant,
/// `modulus` is the irreducible `P(x)` and `degree` is `m`. `mu` and
/// `modulus` fit in `u64` for `m <= 63`.
pub type ClmulBarrettFn = fn(u64, u64, u64, u64, u32) -> u64;

/// Preferred raw carry-less batch implementation, subject to CPU support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClmulBatchLane {
    /// One 128-bit PCLMULQDQ product at a time.
    Sequential,
    /// Two products per YMM VPCLMULQDQ instruction.
    Ymm,
}

/// Bundle of GF(2^m) multiplication functions for different field sizes.
pub struct Gf2mFns {
    /// General multiplication (PCLMULQDQ product, then reduction).
    pub mul_fn: Gf2mMulFn,
    /// Raw carry-less multiply (no reduction). Available when PCLMULQDQ is present.
    pub clmul_fn: Option<ClmulFn>,
    /// Batch carry-less multiply (no reduction), selected at detection time.
    /// The default is sequential PCLMULQDQ. `None` if no PCLMULQDQ.
    pub clmul_batch_fn: Option<ClmulBatchFn>,
    /// Runtime-observed lane tag of [`Gf2mFns::clmul_batch_fn`], one of
    /// `"avx2+vpclmulqdq-ymm"` or `"pclmulqdq-scalar-xmm"`. `Some` exactly
    /// when `clmul_batch_fn` is `Some`.
    pub clmul_batch_path: Option<&'static str>,
    /// All-in-one carry-less multiply + Barrett reduce. `None` if no
    /// PCLMULQDQ.
    pub clmul_barrett_fn: Option<ClmulBarrettFn>,
}

/// Detect the default GF(2^m) function bundle.
///
/// The raw batch lane is sequential PCLMULQDQ.
/// Returns `None` without PCLMULQDQ and SSE4.1; callers use their scalar path.
pub fn detect() -> Option<Gf2mFns> {
    detect_with_clmul_batch_preference(ClmulBatchLane::Sequential)
}

/// Detect a GF(2^m) bundle with a preferred raw-batch lane.
///
/// YMM requires AVX2, VPCLMULQDQ, PCLMULQDQ and SSE4.1, without AVX512VL.
/// An unsupported preference falls back to sequential PCLMULQDQ; without
/// PCLMULQDQ and SSE4.1 this returns `None`. Selection happens once here,
/// and the returned safe function pointer needs no per-call feature check.
/// Other functions in the bundle have the same selection as [`detect`].
pub fn detect_with_clmul_batch_preference(preference: ClmulBatchLane) -> Option<Gf2mFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86(preference);
    }
    #[allow(unreachable_code)]
    {
        let _ = preference;
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86(preference: ClmulBatchLane) -> Option<Gf2mFns> {
    use std::arch::is_x86_feature_detected;

    if is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1") {
        let (batch, path): (ClmulBatchFn, _) =
            if preference == ClmulBatchLane::Ymm && crate::x86::clmul::ymm_batch_lane_supported() {
                (
                    clmul_batch_ymm_safe,
                    crate::x86::clmul::CLMUL_BATCH_PATH_YMM,
                )
            } else {
                (clmul_batch_safe, crate::x86::clmul::CLMUL_BATCH_PATH_XMM)
            };
        Some(Gf2mFns {
            mul_fn: gf2m_mul_pclmul_safe,
            clmul_fn: Some(clmul_u64_safe),
            clmul_batch_fn: Some(batch),
            clmul_batch_path: Some(path),
            clmul_barrett_fn: Some(clmul_barrett_reduce_safe),
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_u64_safe(a: u64, b: u64) -> u128 {
    // SAFETY: detect_x86 publishes this pointer only with PCLMULQDQ + SSE4.1.
    unsafe { crate::x86::clmul::clmul_u64(a, b) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_batch_safe(a: &[u64], b: &[u64], out: &mut [u128]) {
    // SAFETY: detect_x86 publishes this pointer only with PCLMULQDQ + SSE4.1.
    unsafe { crate::x86::clmul::clmul_batch(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_batch_ymm_safe(a: &[u64], b: &[u64], out: &mut [u128]) {
    crate::x86::clmul::assert_batch_lengths(a, b, out);
    // SAFETY: detect_x86 checks all four features via ymm_batch_lane_supported.
    unsafe { crate::x86::clmul::clmul_batch_vpclmul(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_barrett_reduce_safe(a: u64, b: u64, mu: u64, modulus: u64, degree: u32) -> u64 {
    // SAFETY: detect_x86 publishes this pointer only with PCLMULQDQ + SSE4.1.
    unsafe { crate::x86::clmul::clmul_barrett_reduce(a, b, mu, modulus, degree) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn gf2m_mul_pclmul_safe(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
    assert!(
        (1..=64).contains(&m),
        "gf2m mul kernel: degree m must be in 1..=64"
    );
    // SAFETY: detect_x86 publishes this pointer only with PCLMULQDQ + SSE4.1.
    unsafe { gf2m_mul_pclmul(a, b, m, primitive_poly) }
}

/// Multiply two GF(2^m) elements using PCLMULQDQ.
///
/// # Safety
///
/// The host supports PCLMULQDQ and SSE4.1; the product halves are read with
/// `_mm_extract_epi64`. Every argument is a value, so no pointer, length or
/// aliasing condition applies.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
unsafe fn gf2m_mul_pclmul(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    if a == 0 || b == 0 {
        return 0;
    }

    let a_reg = _mm_set_epi64x(0, a as i64);
    let b_reg = _mm_set_epi64x(0, b as i64);
    let product = _mm_clmulepi64_si128::<0x00>(a_reg, b_reg);

    let lo = _mm_extract_epi64::<0>(product) as u64;
    let hi = _mm_extract_epi64::<1>(product) as u64;

    match m {
        8 => reduce_gf256(lo, hi, primitive_poly),
        16 => reduce_gf65536(lo, hi, primitive_poly),
        _ => reduce_generic(lo, hi, m, primitive_poly),
    }
}

/// Reduction for GF(2^8).
///
/// # Safety
///
/// No target feature, pointer, length or aliasing condition applies: the body
/// is integer arithmetic on values.
#[inline(always)]
unsafe fn reduce_gf256(lo: u64, hi: u64, primitive_poly: u64) -> u64 {
    // The product has degree at most 14.

    let mut result = lo;

    for bit_idx in (8..15).rev() {
        if (result >> bit_idx) & 1 == 1 {
            result ^= primitive_poly << (bit_idx - 8);
        }
    }

    if hi != 0 {
        for i in 0..7 {
            if (hi >> i) & 1 == 1 {
                result ^= primitive_poly << (64 + i - 8);
            }
        }
    }

    result & 0xFF
}

/// Reduction for GF(2^16).
///
/// # Safety
///
/// No target feature, pointer, length or aliasing condition applies: the body
/// is integer arithmetic on values.
#[inline(always)]
unsafe fn reduce_gf65536(lo: u64, hi: u64, primitive_poly: u64) -> u64 {
    // The product has degree at most 30.
    let mut result = lo;

    for bit_idx in (16..31).rev() {
        if (result >> bit_idx) & 1 == 1 {
            result ^= primitive_poly << (bit_idx - 16);
        }
    }

    if hi != 0 {
        for i in 0..15 {
            if (hi >> i) & 1 == 1 {
                result ^= primitive_poly << (64 + i - 16);
            }
        }
    }

    result & 0xFFFF
}

/// Reduction of the product `hi:lo` for `1 <= m <= 64`: each step cancels
/// the leading term at or above `x^m` with a shifted copy of the generator.
///
/// # Safety
///
/// No target feature, pointer, length or aliasing condition applies: the body
/// is integer arithmetic on values.
#[inline(always)]
unsafe fn reduce_generic(lo: u64, hi: u64, m: usize, primitive_poly: u64) -> u64 {
    let generator = (1u128 << m) | u128::from(primitive_poly);
    let mut product = (u128::from(hi) << 64) | u128::from(lo);

    loop {
        let excess = product >> m;
        if excess == 0 {
            // `product` is below `2^m`, so it fits in 64 bits.
            return product as u64;
        }
        // The generator's `x^m` lands on the leading term of `product`, at
        // most bit 127.
        product ^= generator << excess.ilog2();
    }
}

#[cfg(test)]
mod tests {
    use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

    use gf2_core::field::axiom_tests::test_field_axioms_with_cases;
    use gf2_core::field::{prove_irreducible, FieldPoly, FiniteField};
    use gf2_core::gf2m::Gf2mField_;
    use gf2_core::gfp::Fp;
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn clmul_batch_default_retains_sequential_lane() {
        if let Some(fns) = detect() {
            assert_eq!(fns.clmul_batch_path, Some("pclmulqdq-scalar-xmm"));
        }
    }

    #[test]
    fn clmul_batch_preferences_preserve_length_contract() {
        for preference in [ClmulBatchLane::Sequential, ClmulBatchLane::Ymm] {
            let Some(fns) = detect_with_clmul_batch_preference(preference) else {
                continue;
            };
            let batch = fns.clmul_batch_fn.unwrap();
            assert!(std::panic::catch_unwind(|| batch(&[1], &[], &mut [0])).is_err());
            assert!(std::panic::catch_unwind(|| batch(&[1], &[1], &mut [])).is_err());
        }
    }

    /// An AVX2 + VPCLMULQDQ host can select the 256-bit raw-batch lane.
    ///
    /// VPCLMULQDQ at 256 bits is VEX-encodable, so AVX512VL is not part of the
    /// lane's feature contract: a host with AVX2 and VPCLMULQDQ and without
    /// AVX512VL reaches the YMM lane.
    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn clmul_batch_selects_ymm_lane_on_avx2_vpclmulqdq_host() {
        use std::arch::is_x86_feature_detected;

        crate::contract::require(&[(
            "avx2+vpclmulqdq+pclmulqdq+sse4.1",
            is_x86_feature_detected!("avx2")
                && is_x86_feature_detected!("vpclmulqdq")
                && is_x86_feature_detected!("pclmulqdq")
                && is_x86_feature_detected!("sse4.1"),
        )]);

        let fns = detect_with_clmul_batch_preference(ClmulBatchLane::Ymm)
            .expect("a PCLMULQDQ host publishes a GF(2^m) bundle");
        assert_eq!(
            fns.clmul_batch_path,
            Some("avx2+vpclmulqdq-ymm"),
            "AVX2+VPCLMULQDQ host must select the YMM raw-batch lane \
             (avx512vl on this host: {})",
            is_x86_feature_detected!("avx512vl")
        );
    }

    /// The published lane tag accompanies the published batch kernel.
    #[test]
    fn clmul_batch_path_accompanies_clmul_batch_fn() {
        let fns = crate::contract::require_detected("gf2m::detect()", detect());
        assert_eq!(
            fns.clmul_batch_fn.is_some(),
            fns.clmul_batch_path.is_some(),
            "clmul_batch_path must be published exactly when clmul_batch_fn is"
        );
        if let Some(path) = fns.clmul_batch_path {
            assert!(
                path == "avx2+vpclmulqdq-ymm" || path == "pclmulqdq-scalar-xmm",
                "unexpected raw-batch lane tag: {path}"
            );
        }
    }

    #[test]
    fn test_detection() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1") {
                assert!(fns.is_some());
            }
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_gf256_mul() {
        use std::arch::is_x86_feature_detected;

        crate::contract::require(&[(
            "pclmulqdq+sse4.1",
            is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1"),
        )]);

        let fns = detect().unwrap();

        // GF(2^8) with x^8 + x^4 + x^3 + x + 1
        let m = 8;
        let p = 0x11B;

        assert_eq!((fns.mul_fn)(1, 5, m, p), 5);
        assert_eq!((fns.mul_fn)(5, 1, m, p), 5);

        assert_eq!((fns.mul_fn)(0, 5, m, p), 0);
        assert_eq!((fns.mul_fn)(5, 0, m, p), 0);

        let a = 0x53;
        let b = 0xCA;
        let result = (fns.mul_fn)(a, b, m, p);
        let expected = scalar_mul(a, b, m, p);
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_gf16_mul() {
        use std::arch::is_x86_feature_detected;

        crate::contract::require(&[(
            "pclmulqdq+sse4.1",
            is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1"),
        )]);

        let fns = detect().unwrap();
        let m = 4;
        let p = 0x13; // x^4 + x + 1

        // (x^2 + 1) * (x^3 + x) in GF(16)
        let a = 0b0101;
        let b = 0b1010;
        let result = (fns.mul_fn)(a, b, m, p);
        let expected = scalar_mul(a, b, m, p);
        assert_eq!(result, expected);
    }

    /// Whether `x^m` plus the polynomial with coefficient word `low` is
    /// irreducible over GF(2).
    fn is_irreducible(m: usize, low: u64) -> bool {
        let one = Fp::<2>::new(1);
        let coeffs = (0..m)
            .map(|i| Fp::<2>::new((low >> i) & 1))
            .chain([one])
            .collect();
        prove_irreducible(&FieldPoly::new(coeffs), &one).is_ok()
    }

    fn low_mask(m: usize) -> u64 {
        u64::MAX >> (64 - m)
    }

    /// The irreducible degree-`m` generators with the smallest and the
    /// largest coefficient word below `x^m`: a sparse one and a dense one.
    fn generators(m: usize) -> [u64; 2] {
        let odd = || (0..=low_mask(m) >> 1).map(|k| 2 * k + 1);
        let pick = |found: Option<u64>| found.expect("an irreducible polynomial of every degree");
        [
            pick(odd().find(|&low| is_irreducible(m, low))),
            pick(odd().rev().find(|&low| is_irreducible(m, low))),
        ]
    }

    fn kernel_mul() -> Gf2mMulFn {
        crate::contract::require_detected("gf2m::detect()", detect()).mul_fn
    }

    /// `mul_fn` against schoolbook multiplication in `u128` storage, which
    /// takes no kernel, with the leading coefficient both implied and given.
    #[test]
    fn mul_fn_matches_the_scalar_reference_at_every_degree() {
        let mul = kernel_mul();
        for m in 1..=64 {
            let mask = low_mask(m);
            for low in generators(m) {
                let field = Gf2mField_::<u128>::new(m, (1u128 << m) | u128::from(low));
                let mut rng = gf2_core::rng::Lcg::new(m as u64);
                let mut operand = || rng.next_u64() >> (64 - m);
                let edges = [1, mask, mask ^ 1, 1 << (m - 1)];
                let pairs = edges
                    .iter()
                    .flat_map(|&a| edges.iter().map(move |&b| (a, b)))
                    .chain((0..256).map(|_| (operand(), operand())));
                for (a, b) in pairs {
                    let expected =
                        (&field.element(a.into()) * &field.element(b.into())).value() as u64;
                    assert_eq!(
                        mul(a, b, m, low),
                        expected,
                        "m={m} generator={low:#x} a={a:#x} b={b:#x}"
                    );
                    if m < 64 {
                        assert_eq!(
                            mul(a, b, m, low | (1 << m)),
                            expected,
                            "m={m} generator={low:#x} with x^m given, a={a:#x} b={b:#x}"
                        );
                    }
                }
            }
        }
    }

    /// GF(2^m) element whose product is the detected `mul_fn`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct KernelElement {
        value: u64,
        m: usize,
        low: u64,
    }

    impl KernelElement {
        fn with_value(self, value: u64) -> Self {
            Self { value, ..self }
        }
    }

    #[allow(clippy::suspicious_arithmetic_impl)]
    impl Add for KernelElement {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            self.with_value(self.value ^ rhs.value)
        }
    }

    impl Mul for KernelElement {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self {
            self.with_value(kernel_mul()(self.value, rhs.value, self.m, self.low))
        }
    }

    #[allow(clippy::suspicious_arithmetic_impl)]
    impl Sub for KernelElement {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            self + rhs
        }
    }

    #[allow(clippy::suspicious_arithmetic_impl)]
    impl Div for KernelElement {
        type Output = Self;
        fn div(self, rhs: Self) -> Self {
            self * rhs.inv().expect("division by zero")
        }
    }

    impl Neg for KernelElement {
        type Output = Self;
        fn neg(self) -> Self {
            self
        }
    }

    impl AddAssign for KernelElement {
        fn add_assign(&mut self, rhs: Self) {
            *self = *self + rhs;
        }
    }

    macro_rules! by_reference {
        ($($op:ident :: $method:ident),*) => {$(
            impl $op<&KernelElement> for KernelElement {
                type Output = Self;
                fn $method(self, rhs: &Self) -> Self {
                    $op::$method(self, *rhs)
                }
            }
        )*};
    }
    by_reference!(Add::add, Sub::sub, Mul::mul, Div::div);

    impl AddAssign<&KernelElement> for KernelElement {
        fn add_assign(&mut self, rhs: &Self) {
            *self += *rhs;
        }
    }

    impl FiniteField for KernelElement {
        type Characteristic = u64;
        type Wide = Self;

        fn characteristic(&self) -> u64 {
            2
        }

        fn extension_degree(&self) -> usize {
            self.m
        }

        fn is_zero(&self) -> bool {
            self.value == 0
        }

        fn is_one(&self) -> bool {
            self.value == 1
        }

        /// `a^(2^m - 2)`, the product of `a^(2^i)` over `1 <= i < m`.
        fn inv(&self) -> Option<Self> {
            if self.is_zero() {
                return None;
            }
            let mut power = *self;
            let mut inverse = self.one_like();
            for _ in 1..self.m {
                power = power * power;
                inverse = inverse * power;
            }
            Some(inverse)
        }

        fn zero_like(&self) -> Self {
            self.with_value(0)
        }

        fn one_like(&self) -> Self {
            self.with_value(1)
        }

        fn to_wide(&self) -> Self {
            *self
        }

        fn mul_to_wide(&self, rhs: &Self) -> Self {
            *self * *rhs
        }

        fn reduce_wide(wide: &Self) -> Self {
            *wide
        }

        fn max_unreduced_additions() -> usize {
            usize::MAX
        }
    }

    #[test]
    fn mul_fn_satisfies_the_field_laws_at_every_degree() {
        for m in 1..=64 {
            for low in generators(m) {
                let strategy = any::<u64>()
                    .prop_map(move |word| KernelElement {
                        value: word & low_mask(m),
                        m,
                        low,
                    })
                    .boxed();
                test_field_axioms_with_cases(strategy, 2, 64);
            }
        }
    }

    fn scalar_mul(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
        let mut result = 0u64;
        let mut temp = a;

        for i in 0..m {
            if (b >> i) & 1 == 1 {
                result ^= temp;
            }

            let will_overflow = (temp & (1u64 << (m - 1))) != 0;
            temp <<= 1;

            if will_overflow {
                temp ^= primitive_poly;
            }
        }

        result & ((1u64 << m) - 1)
    }
}
