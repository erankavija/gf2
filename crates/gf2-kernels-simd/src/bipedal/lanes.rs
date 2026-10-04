//! Lane-width logical primitives used by the generic bipedal-like SIMD
//! framework ([`crate::bipedal::framework`]). Every method impl must be
//! `#[inline(always)]`: rustc otherwise cannot inline an AVX2-emitting trait
//! method into the `#[target_feature(enable = "avx2")]` kernel entry points.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Lane-wise AND, XOR, OR, AND-NOT, load and store for one register width.
///
/// # Safety
///
/// All methods are `unsafe` because they assume the corresponding hardware
/// feature is present at runtime. Callers must runtime-detect the feature
/// before calling any method.
pub trait BipedalLogicalLanes: Copy {
    /// How many `u64` words this lane spans.
    const U64_PER_LANE: usize;

    /// Load a lane from `src` at word index `offset`.
    ///
    /// # Safety
    ///
    /// `offset + Self::U64_PER_LANE <= src.len()` and the corresponding
    /// hardware feature must be available at runtime.
    unsafe fn loadu(src: &[u64], offset: usize) -> Self;

    /// Store `v` to `dst` at word index `offset`.
    ///
    /// # Safety
    ///
    /// `offset + Self::U64_PER_LANE <= dst.len()` and the corresponding
    /// hardware feature must be available at runtime.
    unsafe fn storeu(dst: &mut [u64], offset: usize, v: Self);

    /// Lane-wise bitwise AND.
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn and(a: Self, b: Self) -> Self;

    /// Lane-wise bitwise XOR.
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn xor(a: Self, b: Self) -> Self;

    /// Lane-wise bitwise OR.
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn or(a: Self, b: Self) -> Self;

    /// Lane-wise `a AND NOT b`.
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn andn(a: Self, b: Self) -> Self;
}

/// AVX2 256-bit lane (4 × `u64`) impl of [`BipedalLogicalLanes`].
#[derive(Clone, Copy)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub struct Avx2Lane(pub __m256i);

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl BipedalLogicalLanes for Avx2Lane {
    const U64_PER_LANE: usize = 4;

    #[inline(always)]
    unsafe fn loadu(src: &[u64], offset: usize) -> Self {
        // SAFETY: caller ensures `offset + 4 <= src.len()` and AVX2 availability.
        unsafe {
            Avx2Lane(_mm256_loadu_si256(
                src.as_ptr().add(offset) as *const __m256i
            ))
        }
    }

    #[inline(always)]
    unsafe fn storeu(dst: &mut [u64], offset: usize, v: Self) {
        // SAFETY: caller ensures `offset + 4 <= dst.len()` and AVX2 availability.
        unsafe {
            _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v.0);
        }
    }

    #[inline(always)]
    unsafe fn and(a: Self, b: Self) -> Self {
        // SAFETY: AVX2 availability is the caller's precondition.
        unsafe { Avx2Lane(_mm256_and_si256(a.0, b.0)) }
    }

    #[inline(always)]
    unsafe fn xor(a: Self, b: Self) -> Self {
        // SAFETY: AVX2 availability is the caller's precondition.
        unsafe { Avx2Lane(_mm256_xor_si256(a.0, b.0)) }
    }

    #[inline(always)]
    unsafe fn or(a: Self, b: Self) -> Self {
        // SAFETY: AVX2 availability is the caller's precondition.
        unsafe { Avx2Lane(_mm256_or_si256(a.0, b.0)) }
    }

    #[inline(always)]
    unsafe fn andn(a: Self, b: Self) -> Self {
        // SAFETY: AVX2 availability is the caller's precondition.
        // `_mm256_andnot_si256(x, y)` computes `(NOT x) AND y`, hence the
        // swapped operands.
        unsafe { Avx2Lane(_mm256_andnot_si256(b.0, a.0)) }
    }
}
