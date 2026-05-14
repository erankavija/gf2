//! Generic SIMD framework for bipedal-like `(mag, sgn)` finite-field encodings.
//!
//! This module hosts the [`framework::BatchedBipedalLike`] template plus the
//! [`lanes::BipedalLogicalLanes`] lane abstraction that lets a single body
//! serve every `(prime, ISA)` instantiation. F_3 is the only encoding wired
//! today (via [`f3::Config3`] / [`f3::Bipedal3x4`]); F_5 D-bit-sliced and F_7
//! LUT-A land on top of this same scaffolding in W4.
//!
//! Architectural decision recorded in `dev/plans/r4_simd_batching_decision.md`:
//! the generic framework wins over per-prime hand-rolled kernels by tie-break
//! (every microbench cell within `[0.83, 1.20]` ratio; criterion-4 says
//! generic on tie; full data in §5 of that doc).
//!
//! ## Module layout
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`framework`]  | The `BipedalLikeConfig` trait + `BatchedBipedalLike` generic struct. |
//! | [`lanes`]      | The `BipedalLogicalLanes` trait + `Avx2Lane` impl. |
//! | [`bipedal3`]   | F_3 instantiation: `Config3` + `Bipedal3x4` type alias. |
//!
//! The actual AVX2 batch entry points (`run_*_batch`) live in
//! [`crate::x86::bipedal_avx2`] so the asm-artefact-present gate fires on
//! source changes.
//!
//! ## Adding a new prime
//!
//! Adding a new prime requires only a new [`framework::BipedalLikeConfig`]
//! impl in `crates/gf2-kernels-simd/src/bipedal/bipedal<prime>.rs`. The new impl
//! supplies the `MagLane` / `SgnLane` associated types (typically
//! [`lanes::Avx2Lane`] for both today; future F_5 D-bit-sliced may diverge),
//! the `PRIME` and `U64_PER_LANE_PAIR` constants, and the lane-level
//! `add_lane / sub_lane / mul_lane / neg_lane` formulas.
//!
//! For configs whose `MagLane` and `SgnLane` both resolve to
//! [`lanes::Avx2Lane`], the generic AVX2 entry points in
//! [`crate::x86::bipedal_avx2`] (`run_add_batch::<C>`, `run_sub_batch::<C>`,
//! `run_mul_batch::<C>`, `run_neg_batch::<C>`) automatically monomorphise
//! over the new config; no kernel code changes are required. (The where
//! clause `C: BipedalLikeConfig<MagLane = Avx2Lane, SgnLane = Avx2Lane>`
//! enforces the lane-shape contract.)
//!
//! Concrete steps:
//!
//! 1. Implement [`framework::BipedalLikeConfig`] for a new zero-sized struct.
//!    Pick `MagLane` / `SgnLane` from existing lane impls (today only
//!    [`lanes::Avx2Lane`]; future AVX-512 / AArch64 backends each
//!    contribute a new lane impl).
//! 2. (Optional) Define a type alias
//!    `pub type ... = BatchedBipedalLike<NewConfig>;` for ergonomics.
//! 3. Call the existing generic `run_*_batch::<NewConfig>` entry points
//!    from your safe wrapper / runtime-detection bundle (provided your
//!    `MagLane` and `SgnLane` are both `Avx2Lane`).
//!
//! No changes to the framework, the lane traits, or the AVX2 entry-point
//! definitions are needed unless the new prime introduces a new lane
//! primitive (e.g. byte-shuffle for F_7's LUT-A table lookup) or picks a
//! lane shape other than `(Avx2Lane, Avx2Lane)`.

pub mod bipedal3;
pub mod framework;
pub mod lanes;
pub mod packed5;
pub mod packed7;

pub use bipedal3::Config3;
pub use framework::{BatchedBipedalLike, BipedalLikeConfig};
pub use lanes::BipedalLogicalLanes;
pub use packed5::Config5;
pub use packed7::Config7;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use bipedal3::Bipedal3x4;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use lanes::Avx2Lane;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use packed7::Bipedal7x4;

/// AVX2 batch entry points for the F_3 instantiation.
///
/// The four functions in this module are thin `Config3`-monomorphised
/// shims over the generic [`crate::x86::bipedal_avx2::run_add_batch`]
/// (and its `sub` / `mul` / `neg` siblings). The generic entry points
/// are already `#[target_feature(enable = "avx2")]`; this module gives
/// F_3 callers a stable, non-generic path that does not depend on the
/// private `x86` module layout. Callers must runtime-detect AVX2 before
/// invoking these functions.
///
/// To target a different prime in the future, call the generic entry
/// point in [`crate::x86::bipedal_avx2`] directly with the appropriate
/// `BipedalLikeConfig` — no per-prime shim module is required.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::{
///     run_add_batch, run_sub_batch, run_mul_batch, run_neg_batch,
/// };
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices are length 4 (= one AVX2 lane).
///     unsafe {
///         run_add_batch(&v, &v, &v, &v, &mut out_m, &mut out_s);
///         run_sub_batch(&v, &v, &v, &v, &mut out_m, &mut out_s);
///         run_mul_batch(&v, &v, &v, &v, &mut out_m, &mut out_s);
///         run_neg_batch(&v, &v, &mut out_m, &mut out_s);
///     }
/// }
/// ```
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod avx2 {
    use crate::bipedal::Config3;

    /// `Config3`-monomorphised wrapper over
    /// [`crate::x86::bipedal_avx2::run_add_batch`].
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
    /// [`crate::x86::bipedal_avx2::run_sub_batch`].
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
    /// [`crate::x86::bipedal_avx2::run_mul_batch`].
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
    /// [`crate::x86::bipedal_avx2::run_neg_batch`].
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

/// Function-pointer bundle for the F_3 bipedal AVX2 batch kernels.
///
/// Mirrors the [`crate::LogicalFns`] / [`crate::fp65537::Fp65537Fns`] pattern:
/// runtime detection ([`detect_avx2`]) returns this bundle when the host
/// supports AVX2, and `None` otherwise. The function-pointer fields are
/// safe to call (the safety preconditions have already been discharged by
/// the detection).
///
/// All four operations (`add`, `sub`, `mul`, `neg`) take same-length
/// `&[u64]` streams (input) and `&mut [u64]` buffers (output) where the
/// length is divisible by 4 (one AVX2 lane = 4 × u64). `add` / `sub` / `mul`
/// are 6-tuple arity (two `(mag, sgn)` operands + two outputs);
/// `neg` is 2-input + 2-output.
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
}

/// Detect AVX2 at runtime and return a [`BipedalAvx2Fns`] bundle if available.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks AVX2.
/// Callers must then fall back to scalar arithmetic.
///
/// Mirrors the project's `gf2_core::simd::maybe_simd()` SSOT pattern: the
/// detection result is cached in a `OnceLock` so the first call performs
/// CPUID, all subsequent calls are a cheap atomic load. Returning a value
/// rather than `&'static` keeps the function-pointer fields `Copy`-friendly
/// for callers that want to bind locally.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::bipedal::detect_avx2;
/// let maybe_fns = detect_avx2();
/// // `maybe_fns.is_some()` on any AVX2-capable x86_64 host.
/// let _ = maybe_fns;
/// ```
///
/// # Complexity
///
/// `O(1)`; the first call performs CPUID + `OnceLock` initialisation,
/// subsequent calls return the cached value.
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
