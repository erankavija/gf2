//! # gf2-core - High-Performance GF(2) Primitives
//!
//! This crate provides efficient bit string and matrix operations with a focus on GF(2)
//! arithmetic. It powers the coding-theory tooling of the companion `gf2-coding` crate.
//!
//! ## Core Types
//!
//! - [`BitVec`]: An owning, growable bit string backed by `Vec<u64>`.
//! - [`BitSlice`], [`BitSliceMut`]: Borrowed bit-range views.
//! - [`BitMatrix`]: A row-major, bit-packed boolean matrix for GF(2) linear algebra.
//! - [`SpBitMatrix`]: A sparse matrix in CSR format for low-density matrices, with
//!   [`SpBitMatrixDual`], [`SpBitMatrixBlockCsr`], [`SparseBitMatrix`], and
//!   [`RowPermutation`] in the [`sparse`] module.
//!
//! ## Design Invariants
//!
//! - **Storage**: Dense contiguous `u64` words in little-endian bit order.
//! - **Bit Numbering**: Within each word, bit `i` maps to `word = i >> 6`, `mask = 1u64 << (i & 63)`.
//! - **Tail Masking**: Padding bits beyond `len_bits` in the last word are always zeroed.
//!
//! ## Examples
//!
//! ```
//! use gf2_core::BitVec;
//!
//! let mut bv = BitVec::new();
//! bv.push_bit(true);
//! bv.push_bit(false);
//! bv.push_bit(true);
//!
//! assert_eq!(bv.len(), 3);
//! assert_eq!(bv.get(0), true);
//! assert_eq!(bv.get(1), false);
//! assert_eq!(bv.count_ones(), 2);
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod alg;
mod bitslice;
mod bitvec;
pub mod compute;
pub mod field;
pub mod gf2m;
pub mod gfp;
pub mod gfpn;

#[cfg(feature = "io")]
pub mod io;

// Primitive polynomial database submodule
pub mod kernels;
mod macros;
pub mod matrix;
pub mod matrix_like;
pub mod primitive_polys;
pub mod residual_shift;
pub mod sparse;
pub mod tuning;

pub mod rng;

/// Deterministic SplitMix64-based seed and matrix-fill helpers shared by the
/// gf2-core benchmark suite (`benches/`) and example CSV emitter
/// (`examples/`). Mirrors `benchmarks/reference/seed_helpers.h`
/// bit-for-bit so cross-library benchmark inputs match exactly.
///
/// Gated behind `cfg(any(test, feature = "test-support"))` so it never
/// reaches release builds of downstream consumers.
#[cfg(any(test, feature = "test-support"))]
pub mod bench_seed;

#[cfg(feature = "test-support")]
pub mod test_scratch;

pub use bitslice::{BitSlice, BitSliceMut};
pub use bitvec::BitVec;
pub use matrix::BitMatrix;
pub use sparse::{
    RowPermutation, SpBitMatrix, SpBitMatrixBlockCsr, SpBitMatrixDual, SparseBitMatrix,
};

// Optional SIMD accessor: compiled only when the "simd" feature is enabled.
// This module contains no unsafe code; unsafe is isolated in the separate
// gf2-kernels-simd crate.
#[cfg(feature = "simd")]
pub(crate) mod simd {
    use gf2_kernels_simd::fp65537::Fp65537Fns;
    use gf2_kernels_simd::fp_generic::FpGenericFns;
    use gf2_kernels_simd::fp_medium::MediumPrimeFns;
    use gf2_kernels_simd::fp_medium_f64::FpMediumF64Fns;
    use gf2_kernels_simd::fp_medium_ple::MediumPrimePlePanelFns;
    use gf2_kernels_simd::fp_small::SmallPrimeFns;
    use gf2_kernels_simd::fp_small_f32::SmallPrimeF32Fns;
    use gf2_kernels_simd::fp_small_panel::SmallPrimePanelFns;
    use gf2_kernels_simd::fp_small_ple::SmallPrimePlePanelFns;
    use gf2_kernels_simd::gf2m::Gf2mFns;
    use gf2_kernels_simd::gf2m_batch::Gf2mBatchFns;
    use gf2_kernels_simd::gf2m_gemm::Gf2mGemmFns;
    use gf2_kernels_simd::gf2m_wide::{ClmulWide256Fns, ClmulWide571Fns, Gf2mWideFns};
    use gf2_kernels_simd::mersenne::MersenneFns;
    use gf2_kernels_simd::shift_funnel::ShiftFunnelFns;
    use gf2_kernels_simd::transpose::TransposeFns;
    use gf2_kernels_simd::LogicalFns;
    use std::sync::OnceLock;

    static FNS: OnceLock<Option<LogicalFns>> = OnceLock::new();
    static GF2M_FNS: OnceLock<Option<Gf2mFns>> = OnceLock::new();
    static GF2M_BATCH_FNS: OnceLock<Option<Gf2mBatchFns>> = OnceLock::new();
    static GF2M_GEMM_FNS: OnceLock<Option<Gf2mGemmFns>> = OnceLock::new();
    static MERSENNE_FNS: OnceLock<Option<MersenneFns>> = OnceLock::new();
    static FP65537_FNS: OnceLock<Option<Fp65537Fns>> = OnceLock::new();
    static FP_GENERIC_FNS: OnceLock<Option<FpGenericFns>> = OnceLock::new();
    static FP_MEDIUM_FNS: OnceLock<Option<MediumPrimeFns>> = OnceLock::new();
    static FP_MEDIUM_F64_FNS: OnceLock<Option<FpMediumF64Fns>> = OnceLock::new();
    static FP_MEDIUM_PLE_FNS: OnceLock<Option<MediumPrimePlePanelFns>> = OnceLock::new();
    static FP_SMALL_FNS: OnceLock<Option<SmallPrimeFns>> = OnceLock::new();
    static FP_SMALL_F32_FNS: OnceLock<Option<SmallPrimeF32Fns>> = OnceLock::new();
    static FP_SMALL_PANEL_FNS: OnceLock<Option<SmallPrimePanelFns>> = OnceLock::new();
    static FP_SMALL_PLE_FNS: OnceLock<Option<SmallPrimePlePanelFns>> = OnceLock::new();
    static GF2M_WIDE_FNS: OnceLock<Option<Gf2mWideFns>> = OnceLock::new();
    static SHIFT_FUNNEL_FNS: OnceLock<Option<ShiftFunnelFns>> = OnceLock::new();
    static TRANSPOSE_FNS: OnceLock<Option<TransposeFns>> = OnceLock::new();

    #[inline]
    pub fn maybe_simd() -> Option<&'static LogicalFns> {
        FNS.get_or_init(gf2_kernels_simd::detect).as_ref()
    }

    /// Returns the residual bit-shift funnel kernels, if any.
    ///
    /// This whole module is compiled only under this crate's `simd` cargo
    /// feature, which is not one of its defaults. The kernels need the `bmi2`
    /// processor feature, which
    /// [`gf2_kernels_simd::shift_funnel::detect`] tests at run time; the
    /// bundle is its own detection with its own feature, so a host that reports
    /// `bmi2` without AVX2 still reaches it. `None` means the residual branch of
    /// [`BitVec::shift_left`](crate::BitVec::shift_left) and
    /// [`shift_right`](crate::BitVec::shift_right) runs its portable funnel;
    /// [`crate::residual_shift`] owns that selection.
    #[inline]
    pub fn maybe_shift_funnel() -> Option<&'static ShiftFunnelFns> {
        SHIFT_FUNNEL_FNS
            .get_or_init(gf2_kernels_simd::shift_funnel::detect)
            .as_ref()
    }

    /// Returns the production 64×64 bit-block transpose kernels, if any.
    ///
    /// This whole module is compiled only under this crate's `simd` cargo
    /// feature, which is not one of its defaults; without it
    /// [`BitMatrix::transpose`](crate::matrix::BitMatrix::transpose) calls
    /// the portable kernel directly. The lane published here is the first
    /// entry of `gf2_kernels_simd::transpose::PRODUCTION_PREFERENCE` whose
    /// processor feature the host has, which ends at the scalar lane and so
    /// returns `None` on no supported target.
    ///
    /// The returned [`TransposeFns::transpose_64x64`] operates on
    /// fixed-size `&[u64; 64]` blocks; callers tile arbitrary
    /// `rows × cols` matrices on top of this primitive.
    #[inline]
    pub fn maybe_transpose() -> Option<&'static TransposeFns> {
        TRANSPOSE_FNS
            .get_or_init(gf2_kernels_simd::transpose::detect)
            .as_ref()
    }

    /// Returns the best available GF(2^m) SIMD function bundle, if any.
    ///
    /// This includes raw carry-less multiplication kernels (PCLMULQDQ/VPCLMULQDQ)
    /// as well as the legacy combined multiply+reduce function.
    #[inline]
    pub fn maybe_gf2m() -> Option<&'static Gf2mFns> {
        GF2M_FNS
            .get_or_init(gf2_kernels_simd::gf2m::detect)
            .as_ref()
    }

    /// Returns the best available batch element-wise GF(2^m) multiply/square
    /// SIMD kernel for `m ∈ {8, 16, 32}`, if any.
    ///
    /// Provides AVX2 + VPCLMULQDQ-on-YMM kernels that pack 2 element pairs
    /// per VPCLMULQDQ instruction with Barrett reduction in YMM lanes; the
    /// outer loop unrolls 4 ways for ILP across the dependent reduce step.
    /// Returns `None` on hosts lacking `avx2 + vpclmulqdq + sse4.1`; callers
    /// must fall back to per-element [`maybe_gf2m`] dispatch or pure-Rust.
    #[inline]
    pub fn maybe_gf2m_batch() -> Option<&'static Gf2mBatchFns> {
        GF2M_BATCH_FNS
            .get_or_init(gf2_kernels_simd::gf2m_batch::detect)
            .as_ref()
    }

    /// Returns the best available panelized GF(2^m) GEMM kernel, if any.
    ///
    /// Provides the AVX2 + VPCLMULQDQ broadcast-multiply-accumulate GEMM
    /// that replaces the per-output-cell `try_gf2m_u64_batch_dot_product`
    /// path. Returns `None` on non-AVX2 hosts; callers fall back to the
    /// existing per-cell path.
    #[inline]
    pub fn maybe_gf2m_gemm() -> Option<&'static Gf2mGemmFns> {
        GF2M_GEMM_FNS
            .get_or_init(gf2_kernels_simd::gf2m_gemm::detect)
            .as_ref()
    }

    /// Returns the best available Mersenne-prime SIMD batch kernels, if any.
    ///
    /// Currently provides AVX2 kernels for `Fp<2^31 - 1>`. Returns `None`
    /// on non-AVX2 hardware; callers must fall back to scalar loops.
    #[inline]
    pub fn maybe_mersenne() -> Option<&'static MersenneFns> {
        MERSENNE_FNS
            .get_or_init(gf2_kernels_simd::mersenne::detect)
            .as_ref()
    }

    /// Returns the best available `Fp<65537>` SIMD batch kernels, if any.
    ///
    /// Provides AVX2 lane-parallel multiply/add/sub kernels for the Fermat
    /// prime `P = 2^16 + 1`. Returns `None` on non-AVX2 hardware; callers
    /// must fall back to scalar loops.
    #[inline]
    pub fn maybe_fp65537() -> Option<&'static Fp65537Fns> {
        FP65537_FNS
            .get_or_init(gf2_kernels_simd::fp65537::detect)
            .as_ref()
    }

    /// Returns the best available generic Montgomery `Fp<P>` SIMD batch kernels, if any.
    ///
    /// Provides AVX2 lane-parallel add/sub/mul over internal Montgomery storage
    /// words for odd primes with `P <= 2^63`. Specialised Fermat/Mersenne
    /// kernels remain separate and should be preferred by callers.
    #[inline]
    pub fn maybe_fp_generic() -> Option<&'static FpGenericFns> {
        FP_GENERIC_FNS
            .get_or_init(gf2_kernels_simd::fp_generic::detect)
            .as_ref()
    }

    /// Returns the best available medium-prime `Fp<P>` SIMD batch kernels, if any.
    ///
    /// Provides AVX2 16-lane u16 Barrett-reduction kernels for primes
    /// `p ∈ (251, 65535]` (the `word-fits-in-u16` family — reference prime
    /// `GF(65521)`). Returns `None` on non-AVX2 hardware; callers must
    /// fall back to the generic Montgomery kernels or scalar loops.
    #[inline]
    pub fn maybe_fp_medium() -> Option<&'static MediumPrimeFns> {
        FP_MEDIUM_FNS
            .get_or_init(gf2_kernels_simd::fp_medium::detect)
            .as_ref()
    }

    /// Returns the medium-prime `Fp<P>` AVX2 + FMA3 f64-cascade GEMM kernel, if
    /// any (issue `0749dbad`).
    ///
    /// Provides an in-Rust f64-FMA dgemm micro-kernel for canonical-u16
    /// `Fp<P>` operands with `P ∈ (251, 65535]`. The kernel mirrors
    /// Route A's f32 cascade for `P ≤ 251` (in
    /// [`maybe_fp_small_f32`]) at f64 lane density: 4 f64 lanes per
    /// AVX2 register, `4 × 12` register tile, and a vectorised f64
    /// Barrett reduction (`r = x - p · round(x · (1/p))`) at the end
    /// of the k-axis.
    ///
    /// The f64 cascade targets GF(65521)/n=4096 (`@/issue/695350fd`) through the
    /// f64 FMA back-end, which the u16-lane `_mm256_pmullw + _mm256_pmulhuw`
    /// kernel does not use.
    ///
    /// Returns `None` on hosts without AVX2 + FMA3.
    #[inline]
    pub fn maybe_fp_medium_f64() -> Option<&'static FpMediumF64Fns> {
        FP_MEDIUM_F64_FNS
            .get_or_init(gf2_kernels_simd::fp_medium_f64::detect)
            .as_ref()
    }

    /// Returns the best available small-prime `Fp<P>` SIMD batch kernels, if any.
    ///
    /// Provides AVX2 byte-lane multiply / add / sub / dot kernels for
    /// odd primes with `P ≤ 251`. The kernels use 16-bit-lane Barrett
    /// reduction in 32-byte AVX2 registers, processing 16 elements per
    /// vector iteration (2× the throughput of the generic Montgomery
    /// path's 4-lane u64 multiply on AVX2). Returns `None` on non-AVX2
    /// hardware; callers must fall back to the generic Montgomery path
    /// or scalar loops. Specialised Fermat / Mersenne kernels for
    /// `P > 251` remain separate and dispatch above this branch.
    #[inline]
    pub fn maybe_fp_small() -> Option<&'static SmallPrimeFns> {
        FP_SMALL_FNS
            .get_or_init(gf2_kernels_simd::fp_small::detect)
            .as_ref()
    }

    /// Returns the small-prime `Fp<P>` AVX2 + FMA3 f32-cascade GEMM
    /// kernel, if any.
    ///
    /// Provides the **Candidate F** path from
    /// `dev/plans/small_prime_kernel_strategy.md` § 4.5 / § 5.5 / § 6.1
    /// — an in-Rust `_mm256_fmadd_ps`-based register-blocked sgemm
    /// micro-kernel for canonical-byte `Fp<P>` operands with `P ≤ 251`.
    ///
    /// **Status (per 662f7a15 Amendment C, 2026-05-06):** the kernel is
    /// fully implemented and tested but **not currently selected at
    /// runtime** on any in-scope cell. `select_f32_path::<P>` (in
    /// `crates/gf2-core/src/gfp/simd_ops.rs`) returns `false` for every
    /// `P ≤ 251` because empirical 5-trial CCX1-pinned bench at GF(7)..GF(251)
    /// at n ∈ {256, 1024} (`dev/bench_results/662f7a15/2026-05-06-662f7a15-prime-sweep-aggregate.csv`)
    /// shows Candidate C (the AVX2-only `_mm256_madd_epi16` kernel) beats
    /// Candidate F at every cell by 5–10 %. Production therefore routes
    /// to [`maybe_fp_small`] for these cells.
    ///
    /// Candidate F retains forward-compatibility value: on a host where F
    /// dominates, a calibration of the tuning profile's
    /// `prime_route.f32_min_prime` field lowers the prime window without
    /// touching this accessor or the kernel itself.
    /// Returns `None` on hosts without FMA3.
    ///
    /// Specialised Fermat / Mersenne kernels for `P > 251` remain
    /// separate and dispatch above this branch.
    #[inline]
    pub fn maybe_fp_small_f32() -> Option<&'static SmallPrimeF32Fns> {
        FP_SMALL_F32_FNS
            .get_or_init(gf2_kernels_simd::fp_small_f32::detect)
            .as_ref()
    }

    /// Returns the small-prime `Fp<P>` AVX2 pure-integer Goto/BLIS-style
    /// panelized GEMM kernel, if any.
    ///
    /// Provides **Route C** of `@/issue/615db3b9` (item 3) and the design note
    /// `@/issue/fc182ed5` — an explicit A/B panel-packed AVX2 register-blocked
    /// `_mm256_madd_epi16`-based GEMM for canonical-byte `Fp<P>` operands with
    /// `P ≤ 251`.
    ///
    /// The kernel is not selected by automatic dispatch; it is exposed only via
    /// the GF(251)-only opt-in toggle
    /// [`crate::gfp::simd_ops::set_route_c_gf251_enabled`]. Default
    /// production dispatch uses Candidate C ([`maybe_fp_small`]) owns all `p ≤
    /// 251` cells.
    ///
    /// Returns `None` on non-AVX2 hardware; callers must fall back to
    /// [`maybe_fp_small`] (the production Candidate C row-panel
    /// kernel) or scalar.
    #[inline]
    pub fn maybe_fp_small_panel() -> Option<&'static SmallPrimePanelFns> {
        FP_SMALL_PANEL_FNS
            .get_or_init(gf2_kernels_simd::fp_small_panel::detect)
            .as_ref()
    }

    /// Returns the best available small-prime panelized PLE base-case
    /// kernel for `Fp<P>` with `P <= 251` (issue `6823c8a0`, design
    /// `2e8c5a29`).
    ///
    /// Provides the AVX2 byte-lane Schur-update axpy kernel used by
    /// the new `ple_in_place_window` panel-base dispatch arm. Returns
    /// `None` on non-AVX2 hosts; callers fall back to the scalar
    /// `ple_base_direct` path.
    #[inline]
    pub fn maybe_fp_small_ple() -> Option<&'static SmallPrimePlePanelFns> {
        FP_SMALL_PLE_FNS
            .get_or_init(gf2_kernels_simd::fp_small_ple::detect)
            .as_ref()
    }

    /// Returns the best available medium-prime panelized PLE base-case
    /// kernel for `Fp<P>` with `P ∈ (251, 65536)` (issue `68db401b`,
    /// design `2e8c5a29` § 9).
    ///
    /// Provides the AVX2 u16-lane Schur-update axpy kernel used by the
    /// `ple_in_place_window` panel-base dispatch arm for medium primes.
    /// Returns `None` on non-AVX2 hosts; callers fall back to the
    /// scalar `ple_base_direct` path.
    #[inline]
    pub fn maybe_fp_medium_ple() -> Option<&'static MediumPrimePlePanelFns> {
        FP_MEDIUM_PLE_FNS
            .get_or_init(gf2_kernels_simd::fp_medium_ple::detect)
            .as_ref()
    }

    /// Returns the best available fixed-size wide GF(2^m) carry-less multiply
    /// kernels, if any.
    ///
    /// Preference order: AVX2+VPCLMULQDQ (YMM) → PCLMULQDQ scalar-lane (XMM).
    /// Kernels produce only unreduced carry-less products; Barrett reduction is
    /// applied by the caller. Returns `None` when no PCLMULQDQ is present.
    #[inline]
    pub fn maybe_gf2m_wide() -> Option<&'static Gf2mWideFns> {
        GF2M_WIDE_FNS
            .get_or_init(gf2_kernels_simd::gf2m_wide::detect_wide)
            .as_ref()
    }

    /// Returns the best available 4-limb (GF(2^256)) carry-less multiply
    /// kernel, if any.
    #[inline]
    pub fn maybe_gf2m_wide256() -> Option<&'static ClmulWide256Fns> {
        maybe_gf2m_wide().map(|fns| &fns.wide256)
    }

    /// Returns the best available 9-limb (GF(2^571)) carry-less multiply
    /// kernel, if any.
    #[inline]
    pub fn maybe_gf2m_wide571() -> Option<&'static ClmulWide571Fns> {
        maybe_gf2m_wide().map(|fns| &fns.wide571)
    }
}

#[cfg(not(feature = "simd"))]
pub(crate) mod simd {
    #[allow(dead_code)]
    #[inline]
    pub fn maybe_simd() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_mersenne() -> Option<()> {
        None
    }

    /// Without the `simd` feature no kernel bundle is compiled, so
    /// [`crate::residual_shift`] never reaches for one.
    #[allow(dead_code)]
    #[inline]
    pub fn maybe_shift_funnel() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp65537() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_generic() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_medium() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_medium_f64() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_small() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_small_f32() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_small_panel() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_small_ple() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_fp_medium_ple() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_gf2m_wide() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_gf2m_wide256() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_gf2m_wide571() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_gf2m_batch() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_gf2m_gemm() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_transpose() -> Option<()> {
        None
    }
}

#[cfg(test)]
mod bitvec_sync_tests;
