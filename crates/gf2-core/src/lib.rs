//! # gf2-core: GF(2) primitives
//!
//! Bit strings ([`BitVec`], [`BitSlice`], [`BitSliceMut`]), dense
//! ([`BitMatrix`]) and sparse ([`sparse`]) GF(2) matrices, and finite-field
//! arithmetic ([`field`], [`gf2m`], [`gfp`], [`gfpn`]).
//!
//! ## Design Invariants
//!
//! - **Storage**: Dense contiguous `u64` words in little-endian bit order.
//! - **Bit Numbering**: Within each word, bit `i` maps to `word = i >> 6`, `mask = 1u64 << (i & 63)`.
//! - **Tail Masking**: Padding bits beyond `len_bits` in the last word are always zeroed.

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
/// benchmark suite and the example CSV emitter; the C counterpart is
/// `benchmarks/reference/seed_helpers.h`.
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
    /// Detection requires only `bmi2`. On `None`, [`crate::residual_shift`]
    /// runs its portable funnel.
    #[inline]
    pub fn maybe_shift_funnel() -> Option<&'static ShiftFunnelFns> {
        SHIFT_FUNNEL_FNS
            .get_or_init(gf2_kernels_simd::shift_funnel::detect)
            .as_ref()
    }

    /// Returns the 64×64 bit-block transpose kernel of the first lane in
    /// `gf2_kernels_simd::transpose::PRODUCTION_PREFERENCE` the host supports.
    #[inline]
    pub fn maybe_transpose() -> Option<&'static TransposeFns> {
        TRANSPOSE_FNS
            .get_or_init(gf2_kernels_simd::transpose::detect)
            .as_ref()
    }

    /// Returns the best available GF(2^m) SIMD function bundle, if any.
    #[inline]
    pub fn maybe_gf2m() -> Option<&'static Gf2mFns> {
        GF2M_FNS
            .get_or_init(gf2_kernels_simd::gf2m::detect)
            .as_ref()
    }

    /// Returns the best available batch element-wise GF(2^m) multiply/square
    /// SIMD kernel for `m ∈ {8, 16, 32}`, if any.
    ///
    /// `None` on hosts lacking AVX2, VPCLMULQDQ, PCLMULQDQ or SSE4.1.
    #[inline]
    pub fn maybe_gf2m_batch() -> Option<&'static Gf2mBatchFns> {
        GF2M_BATCH_FNS
            .get_or_init(gf2_kernels_simd::gf2m_batch::detect)
            .as_ref()
    }

    /// Returns the best available panelized GF(2^m) GEMM kernel, if any.
    ///
    /// `None` on hosts lacking AVX2, VPCLMULQDQ, PCLMULQDQ or SSE4.1.
    #[inline]
    pub fn maybe_gf2m_gemm() -> Option<&'static Gf2mGemmFns> {
        GF2M_GEMM_FNS
            .get_or_init(gf2_kernels_simd::gf2m_gemm::detect)
            .as_ref()
    }

    /// Returns the best available Mersenne-prime SIMD batch kernels, if any.
    ///
    /// AVX2 kernels for `Fp<2^31 - 1>`; `None` on non-AVX2 hardware.
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

    /// Returns the medium-prime `Fp<P>` AVX2 + FMA3 f64-cascade GEMM kernel
    /// for `P ∈ (251, 65535]`, if any.
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
    /// Provides AVX2 multiply / add / sub / dot kernels for odd primes with
    /// `P ≤ 251`. Returns `None` on non-AVX2 hardware.
    #[inline]
    pub fn maybe_fp_small() -> Option<&'static SmallPrimeFns> {
        FP_SMALL_FNS
            .get_or_init(gf2_kernels_simd::fp_small::detect)
            .as_ref()
    }

    /// Returns the small-prime `Fp<P>` AVX2 + FMA3 f32-cascade GEMM
    /// kernel, if any.
    ///
    /// A register-blocked GEMM micro-kernel for `Fp<P>` with `P ≤ 251`.
    /// [`crate::gfp::simd_ops::prime_gemm_route`] reports the cells that
    /// dispatch routes to it.
    ///
    /// Returns `None` on hosts without AVX2 + FMA3.
    #[inline]
    pub fn maybe_fp_small_f32() -> Option<&'static SmallPrimeF32Fns> {
        FP_SMALL_F32_FNS
            .get_or_init(gf2_kernels_simd::fp_small_f32::detect)
            .as_ref()
    }

    /// Returns the small-prime `Fp<P>` AVX2 integer panel-packed GEMM kernel
    /// (`@/citation/GotoGeijn2008`, `@/citation/VanZee2015`) for `P ≤ 251`,
    /// if any.
    ///
    /// Automatic dispatch never selects it; the opt-in toggle is
    /// [`crate::gfp::simd_ops::set_route_c_gf251_enabled`].
    ///
    /// Returns `None` on non-AVX2 hardware.
    #[inline]
    pub fn maybe_fp_small_panel() -> Option<&'static SmallPrimePanelFns> {
        FP_SMALL_PANEL_FNS
            .get_or_init(gf2_kernels_simd::fp_small_panel::detect)
            .as_ref()
    }

    /// Returns the small-prime panelized PLE base-case kernel for `Fp<P>`
    /// with `P <= 251`, if any.
    ///
    /// `None` on non-AVX2 hosts; callers fall back to the scalar
    /// `ple_base_direct` path.
    #[inline]
    pub fn maybe_fp_small_ple() -> Option<&'static SmallPrimePlePanelFns> {
        FP_SMALL_PLE_FNS
            .get_or_init(gf2_kernels_simd::fp_small_ple::detect)
            .as_ref()
    }

    /// Returns the medium-prime panelized PLE base-case kernel for `Fp<P>`
    /// with `P ∈ (251, 65536)`, if any.
    ///
    /// `None` on non-AVX2 hosts; callers fall back to the scalar
    /// `ple_base_direct` path.
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
