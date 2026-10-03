//! Generic SIMD framework for bipedal-like `(mag, sgn)` finite-field encodings.
//!
//! [`framework::BatchedBipedalLike`] over [`lanes::BipedalLogicalLanes`] serves
//! F_3 through [`bipedal3::Config3`]. F_5 ([`packed5`], 3-plane bit-sliced) and
//! F_7 ([`packed7`], 3-bit digits with a 2^16 LUT) use dedicated AVX2 batch
//! entry points, because neither encoding fits the 2-stream `(mag, sgn)` shape.

pub mod bipedal3;
pub mod framework;
pub mod lanes;
pub mod packed5;
pub mod packed7;

pub use bipedal3::Config3;
pub use framework::{BatchedBipedalLike, BipedalLikeConfig};
pub use lanes::BipedalLogicalLanes;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use bipedal3::Bipedal3x4;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use lanes::Avx2Lane;

/// AVX2 batch entry points for the F_3 instantiation.
///
/// `Config3` monomorphisations of the generic `crate::x86::bipedal_avx2`
/// entry points, giving F_3 callers a non-generic path independent of the
/// private `x86` module. Callers must runtime-detect AVX2 before invoking
/// these functions.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod avx2 {
    use crate::bipedal::Config3;

    /// `Config3`-monomorphised wrapper over
    /// `crate::x86::bipedal_avx2::run_add_batch`.
    ///
    /// # Safety
    ///
    /// AVX2 must be available at runtime; all six slices share length
    /// divisible by 4. See the generic entry point for the full contract.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_add_batch(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) {
        // SAFETY: AVX2 + slice-shape are caller's preconditions; forwarded.
        unsafe {
            crate::x86::bipedal_avx2::run_add_batch::<Config3>(
                mag1, sgn1, mag2, sgn2, out_mag, out_sgn,
            )
        }
    }

    /// `Config3`-monomorphised wrapper over
    /// `crate::x86::bipedal_avx2::run_sub_batch`.
    ///
    /// # Safety
    ///
    /// AVX2 must be available at runtime; all six slices share length
    /// divisible by 4.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_sub_batch(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) {
        // SAFETY: AVX2 + slice-shape are caller's preconditions; forwarded.
        unsafe {
            crate::x86::bipedal_avx2::run_sub_batch::<Config3>(
                mag1, sgn1, mag2, sgn2, out_mag, out_sgn,
            )
        }
    }

    /// `Config3`-monomorphised wrapper over
    /// `crate::x86::bipedal_avx2::run_mul_batch`.
    ///
    /// # Safety
    ///
    /// AVX2 must be available at runtime; all six slices share length
    /// divisible by 4.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_mul_batch(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) {
        // SAFETY: AVX2 + slice-shape are caller's preconditions; forwarded.
        unsafe {
            crate::x86::bipedal_avx2::run_mul_batch::<Config3>(
                mag1, sgn1, mag2, sgn2, out_mag, out_sgn,
            )
        }
    }

    /// `Config3`-monomorphised wrapper over
    /// `crate::x86::bipedal_avx2::run_neg_batch`.
    ///
    /// # Safety
    ///
    /// AVX2 must be available at runtime; all four slices share length
    /// divisible by 4.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_neg_batch(
        mag: &[u64],
        sgn: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) {
        // SAFETY: AVX2 + slice-shape are caller's preconditions; forwarded.
        unsafe { crate::x86::bipedal_avx2::run_neg_batch::<Config3>(mag, sgn, out_mag, out_sgn) }
    }
}

/// Two-operand bipedal binary kernel: `(m1, s1) op (m2, s2) -> (out_mag, out_sgn)`.
///
/// Used by [`BipedalAvx2Fns::add_fn`], [`BipedalAvx2Fns::sub_fn`], and
/// [`BipedalAvx2Fns::mul_fn`]. All six slices share length divisible by 4.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type BipedalBinaryKernelFn = fn(&[u64], &[u64], &[u64], &[u64], &mut [u64], &mut [u64]);

/// Single-operand bipedal unary kernel: `(mag, sgn) -> (out_mag, out_sgn)`.
///
/// Used by [`BipedalAvx2Fns::neg_fn`]. All four slices share length
/// divisible by 4.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type BipedalUnaryKernelFn = fn(&[u64], &[u64], &mut [u64], &mut [u64]);

/// Four-matrix single-word F_3 permanent kernel.
///
/// Each input element is one packed column with one `u64` word per matrix
/// lane. Both slices have the same length `n <= 63`; results are canonical
/// residues in matrix-lane order.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type BipedalPermanent4KernelFn = fn(&[[u64; 4]], &[[u64; 4]]) -> [u64; 4];

/// Function-pointer bundle for the F_3 bipedal AVX2 batch kernels.
///
/// [`detect_avx2`] returns this bundle only when the host supports AVX2. The
/// arithmetic kernels require same-length slices whose length is divisible by
/// 4 (one AVX2 lane).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[derive(Copy, Clone)]
pub struct BipedalAvx2Fns {
    /// Apply F_3 add.
    pub add_fn: BipedalBinaryKernelFn,
    /// Apply F_3 sub (`(m1, s1) - (m2, s2)`).
    pub sub_fn: BipedalBinaryKernelFn,
    /// Apply F_3 mul.
    pub mul_fn: BipedalBinaryKernelFn,
    /// Apply F_3 neg.
    pub neg_fn: BipedalUnaryKernelFn,
    /// Evaluate four single-word F_3 permanents in one AVX2 Gray walk.
    pub permanent4_fn: BipedalPermanent4KernelFn,
}

/// Detect AVX2 at runtime and return a [`BipedalAvx2Fns`] bundle if available.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks AVX2.
/// Callers must then fall back to scalar arithmetic.
///
/// The detection result is cached in a `OnceLock`.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn detect_avx2() -> Option<BipedalAvx2Fns> {
    use std::sync::OnceLock;
    static FNS: OnceLock<Option<BipedalAvx2Fns>> = OnceLock::new();
    *FNS.get_or_init(detect_avx2_uncached)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_avx2_uncached() -> Option<BipedalAvx2Fns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(BipedalAvx2Fns {
            add_fn: add_safe,
            sub_fn: sub_safe,
            mul_fn: mul_safe,
            neg_fn: neg_safe,
            permanent4_fn: permanent4_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn add_safe(m1: &[u64], s1: &[u64], m2: &[u64], s2: &[u64], om: &mut [u64], os: &mut [u64]) {
    // SAFETY: `detect_avx2` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2::run_add_batch::<Config3>(m1, s1, m2, s2, om, os) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn sub_safe(m1: &[u64], s1: &[u64], m2: &[u64], s2: &[u64], om: &mut [u64], os: &mut [u64]) {
    // SAFETY: `detect_avx2` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2::run_sub_batch::<Config3>(m1, s1, m2, s2, om, os) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn mul_safe(m1: &[u64], s1: &[u64], m2: &[u64], s2: &[u64], om: &mut [u64], os: &mut [u64]) {
    // SAFETY: `detect_avx2` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2::run_mul_batch::<Config3>(m1, s1, m2, s2, om, os) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn neg_safe(m: &[u64], s: &[u64], om: &mut [u64], os: &mut [u64]) {
    // SAFETY: `detect_avx2` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2::run_neg_batch::<Config3>(m, s, om, os) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn permanent4_safe(columns_mag: &[[u64; 4]], columns_sgn: &[[u64; 4]]) -> [u64; 4] {
    // SAFETY: `detect_avx2` only exposes this pointer after runtime AVX2
    // detection; the kernel checks both slice shape and dimension before load.
    unsafe { crate::x86::bipedal_avx2::run_permanent4(columns_mag, columns_sgn) }
}
