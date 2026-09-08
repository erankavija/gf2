//! AVX-512 bipedal F_3 batch entry points — compile-time stub.
//!
//! This module is compiled only when `target_feature = "avx512f"` is set at
//! compile time. The dev host (AMD Ryzen 9 5900X) has no AVX-512, so this
//! path is **never exercised in practice** — it exists to satisfy the
//! `[aspirational]` criterion 4 of JIT issue `d181e95b` (T12) and to
//! provide a forward-compatible hook for a future AVX-512 host.
//!
//! ## Design intent
//!
//! The AVX2 entry points in [`super::bipedal_avx2`] use 256-bit `ymm`
//! registers (4 × u64 per lane). A future AVX-512 implementation would use
//! 512-bit `zmm` registers (8 × u64 per lane), doubling throughput for
//! large batches.  The kernel body for F_3 remains structurally identical
//! (the same `and / xor / or` formulas), only the lane width changes.
//!
//! When AVX-512 hardware is available, replace the `unimplemented!()` bodies
//! below with `#[target_feature(enable = "avx512f")]` unsafe functions using
//! `_mm512_and_si512`, `_mm512_xor_si512`, and `_mm512_or_si512`.  Update
//! `[aspirational]` → `[hard]` in the JIT issue and regenerate
//! `crates/gf2-kernels-simd/src/x86/asm/bipedal_avx512.asm.txt`.
//!
//! ## Slice contract (same as AVX2)
//!
//! All slice lengths must be divisible by 8 (one ZMM lane = 8 × u64).
//! The entry points panic in debug mode if that invariant is violated.

#![cfg(target_feature = "avx512f")]

use crate::bipedal::Config3;

/// AVX-512 F_3 add batch (stub — not yet implemented).
///
/// # Safety
///
/// AVX-512F must be available at runtime; all six slices must share a length
/// that is divisible by 8.
///
/// # Panics
///
/// Always panics: this stub is provided for compile-time forward-compat only.
#[target_feature(enable = "avx512f")]
pub unsafe fn run_add_batch_avx512(
    _mag1: &[u64],
    _sgn1: &[u64],
    _mag2: &[u64],
    _sgn2: &[u64],
    _out_mag: &mut [u64],
    _out_sgn: &mut [u64],
) {
    // SAFETY: caller guarantees AVX-512F availability and aligned slice
    // lengths; this stub never reaches unsafe code — it unconditionally
    // panics to signal that the implementation is not yet written.
    unimplemented!(
        "bipedal_avx512::run_add_batch_avx512 for Config3 (prime={}) \
         is a forward-compat stub; implement on a host with AVX-512 hardware",
        <Config3 as crate::bipedal::BipedalLikeConfig>::PRIME
    )
}

/// AVX-512 F_3 sub batch (stub — not yet implemented).
///
/// # Safety
///
/// See [`run_add_batch_avx512`] for the safety contract.
///
/// # Panics
///
/// Always panics: this stub is provided for compile-time forward-compat only.
#[target_feature(enable = "avx512f")]
pub unsafe fn run_sub_batch_avx512(
    _mag1: &[u64],
    _sgn1: &[u64],
    _mag2: &[u64],
    _sgn2: &[u64],
    _out_mag: &mut [u64],
    _out_sgn: &mut [u64],
) {
    // SAFETY: see run_add_batch_avx512.
    unimplemented!(
        "bipedal_avx512::run_sub_batch_avx512 for Config3 (prime={}) \
         is a forward-compat stub",
        <Config3 as crate::bipedal::BipedalLikeConfig>::PRIME
    )
}

/// AVX-512 F_3 mul batch (stub — not yet implemented).
///
/// # Safety
///
/// See [`run_add_batch_avx512`] for the safety contract.
///
/// # Panics
///
/// Always panics: this stub is provided for compile-time forward-compat only.
#[target_feature(enable = "avx512f")]
pub unsafe fn run_mul_batch_avx512(
    _mag1: &[u64],
    _sgn1: &[u64],
    _mag2: &[u64],
    _sgn2: &[u64],
    _out_mag: &mut [u64],
    _out_sgn: &mut [u64],
) {
    // SAFETY: see run_add_batch_avx512.
    unimplemented!(
        "bipedal_avx512::run_mul_batch_avx512 for Config3 (prime={}) \
         is a forward-compat stub",
        <Config3 as crate::bipedal::BipedalLikeConfig>::PRIME
    )
}

/// AVX-512 F_3 neg batch (stub — not yet implemented).
///
/// # Safety
///
/// AVX-512F must be available at runtime; all four slices must share a length
/// divisible by 8.
///
/// # Panics
///
/// Always panics: this stub is provided for compile-time forward-compat only.
#[target_feature(enable = "avx512f")]
pub unsafe fn run_neg_batch_avx512(
    _mag: &[u64],
    _sgn: &[u64],
    _out_mag: &mut [u64],
    _out_sgn: &mut [u64],
) {
    // SAFETY: see run_add_batch_avx512.
    unimplemented!(
        "bipedal_avx512::run_neg_batch_avx512 for Config3 (prime={}) \
         is a forward-compat stub",
        <Config3 as crate::bipedal::BipedalLikeConfig>::PRIME
    )
}
