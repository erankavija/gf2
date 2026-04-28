//! # gf2-core - High-Performance GF(2) Primitives
//!
//! This crate provides efficient bit string and matrix operations with a focus on GF(2)
//! arithmetic. It powers higher-level coding theory and compression tooling provided by
//! the companion `gf2-coding` crate.
//!
//! ## Core Types
//!
//! - [`BitVec`]: An owning, growable bit string backed by `Vec<u64>`.
//! - [`BitMatrix`]: A row-major, bit-packed boolean matrix for GF(2) linear algebra.
//! - [`SpBitMatrix`]: A sparse matrix in CSR format for low-density matrices.
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
pub mod sparse;

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

pub use bitslice::{BitSlice, BitSliceMut};
pub use bitvec::BitVec;
pub use matrix::BitMatrix;
pub use sparse::{SpBitMatrix, SpBitMatrixDual};

// Optional SIMD accessor: compiled only when the "simd" feature is enabled.
// This module contains no unsafe code; unsafe is isolated in the separate
// gf2-kernels-simd crate.
#[cfg(feature = "simd")]
pub(crate) mod simd {
    use gf2_kernels_simd::fp65537::Fp65537Fns;
    use gf2_kernels_simd::fp_generic::FpGenericFns;
    use gf2_kernels_simd::gf2m::Gf2mFns;
    use gf2_kernels_simd::gf2m_batch::Gf2mBatchFns;
    use gf2_kernels_simd::gf2m_wide::ClmulWide256Fns;
    use gf2_kernels_simd::mersenne::MersenneFns;
    use gf2_kernels_simd::transpose::TransposeFns;
    use gf2_kernels_simd::LogicalFns;
    use std::sync::OnceLock;

    static FNS: OnceLock<Option<LogicalFns>> = OnceLock::new();
    static GF2M_FNS: OnceLock<Option<Gf2mFns>> = OnceLock::new();
    static GF2M_BATCH_FNS: OnceLock<Option<Gf2mBatchFns>> = OnceLock::new();
    static MERSENNE_FNS: OnceLock<Option<MersenneFns>> = OnceLock::new();
    static FP65537_FNS: OnceLock<Option<Fp65537Fns>> = OnceLock::new();
    static FP_GENERIC_FNS: OnceLock<Option<FpGenericFns>> = OnceLock::new();
    static GF2M_WIDE256_FNS: OnceLock<Option<ClmulWide256Fns>> = OnceLock::new();
    static TRANSPOSE_FNS: OnceLock<Option<TransposeFns>> = OnceLock::new();

    #[inline]
    pub fn maybe_simd() -> Option<&'static LogicalFns> {
        FNS.get_or_init(gf2_kernels_simd::detect).as_ref()
    }

    /// Returns the best available 64×64 bit-block transpose kernels, if any.
    ///
    /// On x86_64 with AVX2 this returns the PSHUFB-based byte-tile lane;
    /// otherwise the scalar Hacker's Delight bit-twiddle lane is published
    /// (always available). Returns `None` only if the host platform has
    /// no implementation at all (currently impossible on supported targets).
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

    /// Returns the best available 4-limb (GF(2^256)) carry-less multiply
    /// kernel, if any.
    ///
    /// Preference order: AVX2+VPCLMULQDQ (YMM) → PCLMULQDQ scalar-lane
    /// (XMM). The kernel produces only the unreduced 8-limb carry-less
    /// product; Barrett reduction is applied by the caller. Returns `None`
    /// when no PCLMULQDQ is present; callers must fall back to the pure-Rust
    /// scalar `clmul_wide` implementation. An AVX-512VL+VPCLMULQDQ (ZMM)
    /// lane is not currently provided — the test host (Zen 3) has no
    /// AVX-512 hardware. The required `_mm512_*` carry-less-multiply
    /// intrinsics are stable since Rust 1.89, so the lane can be added
    /// when AVX-512 hardware is in scope.
    #[inline]
    pub fn maybe_gf2m_wide256() -> Option<&'static ClmulWide256Fns> {
        GF2M_WIDE256_FNS
            .get_or_init(gf2_kernels_simd::gf2m_wide::detect)
            .as_ref()
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
    pub fn maybe_gf2m_wide256() -> Option<()> {
        None
    }

    #[allow(dead_code)]
    #[inline]
    pub fn maybe_gf2m_batch() -> Option<()> {
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
