//! Lane-width logical primitives used by the generic bipedal-like SIMD
//! framework.
//!
//! A lane impl supplies the lane-wise AND, XOR, OR, AND-NOT, plus loads and
//! stores, for one register width. The framework's per-prime arithmetic
//! ([`crate::bipedal::framework`]) is written entirely in terms of this trait,
//! which is what allows a single body to serve every encoding currently
//! shipping (F_3, F_5, F_7).
//!
//! ## Inlining contract
//!
//! Every method on this trait must be `#[inline(always)]`. The framework's
//! kernel entry points carry `#[target_feature(enable = "avx2")]`; without
//! the always-inline annotation here, rustc cannot inline an AVX2-emitting
//! trait method into a target-feature-enabled function and the resulting
//! codegen regresses by 12-34x (R4 §4.1; verified during the R4 microbench).
//!
//! All `pub unsafe fn` here carry a top-of-function `// SAFETY:` comment
//! per CLAUDE.md §Key design invariants 3.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Lane-width logical primitives required by the generic bipedal-like
/// framework.
///
/// An impl supplies the lane-wise AND, XOR, OR, and AND-NOT (`a AND NOT b`)
/// for one register width plus the load and store. The framework calls
/// these primitives in the same order the per-prime kernels would, but is
/// generic over the underlying lane width.
///
/// # Safety
///
/// All methods are `unsafe` because they assume the corresponding hardware
/// feature is present at runtime. Callers must runtime-detect the feature
/// before calling any method.
pub trait BipedalLogicalLanes: Copy {
    /// How many `u64` words this lane spans.
    ///
    /// For [`Avx2Lane`] this is 4 (256 bits / 64).
    const U64_PER_LANE: usize;

    /// Load a lane from a `&[u64]` slice at the given word offset.
    ///
    /// # Arguments
    ///
    /// * `src` — source slice; must contain at least `offset + U64_PER_LANE` words.
    /// * `offset` — starting u64 word index.
    ///
    /// # Safety
    ///
    /// `offset + Self::U64_PER_LANE <= src.len()` and the corresponding
    /// hardware feature must be available at runtime.
    unsafe fn loadu(src: &[u64], offset: usize) -> Self;

    /// Store a lane to a `&mut [u64]` slice at the given word offset.
    ///
    /// # Arguments
    ///
    /// * `dst` — destination slice; must contain at least `offset + U64_PER_LANE` words.
    /// * `offset` — starting u64 word index.
    /// * `v` — value to store.
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

    /// Lane-wise `a AND NOT b` — bits set in `a` that are clear in `b`.
    ///
    /// On AVX2 this maps to a single `vpandn` instruction. Required by
    /// some bipedal-like primes whose `neg` formula is most compactly
    /// expressed as `mag AND NOT sgn` (when the `(mag, sgn)` encoding
    /// uses the alt-zero form `(0, 1)` for canonicalisation).
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn andn(a: Self, b: Self) -> Self;
}

/// AVX2 256-bit lane (4 × `u64`) impl of [`BipedalLogicalLanes`].
///
/// Used by the F_3 instantiation [`crate::bipedal::Bipedal3x4`].
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::{Avx2Lane, BipedalLogicalLanes};
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slice is length 4 (= U64_PER_LANE).
///     let _lane: Avx2Lane = unsafe { Avx2Lane::loadu(&v, 0) };
/// }
/// ```
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
        // `_mm256_andnot_si256(x, y)` computes `(NOT x) AND y`, i.e. `andn(b, a)`
        // in our convention. We swap the operand order so the result is
        // `a AND NOT b` as documented.
        unsafe { Avx2Lane(_mm256_andnot_si256(b.0, a.0)) }
    }
}
