//! AVX-512 bipedal F_3 batch entry points, compiled only under
//! `target_feature = "avx512f"`. Every entry point panics unconditionally;
//! [`super::bipedal_avx2`] holds the implemented kernels.

#![cfg(target_feature = "avx512f")]

use crate::bipedal::Config3;

/// AVX-512 F_3 add batch.
///
/// # Safety
///
/// AVX-512F must be available at runtime; all six slices must share a length
/// that is divisible by 8.
///
/// # Panics
///
/// Always panics: the kernel has no implementation.
#[target_feature(enable = "avx512f")]
pub unsafe fn run_add_batch_avx512(
    _mag1: &[u64],
    _sgn1: &[u64],
    _mag2: &[u64],
    _sgn2: &[u64],
    _out_mag: &mut [u64],
    _out_sgn: &mut [u64],
) {
    // SAFETY: the body reaches no unsafe operation; it panics unconditionally.
    unimplemented!(
        "bipedal_avx512::run_add_batch_avx512 for Config3 (prime={}) \
         is a forward-compat stub; implement on a host with AVX-512 hardware",
        <Config3 as crate::bipedal::BipedalLikeConfig>::PRIME
    )
}

/// AVX-512 F_3 sub batch.
///
/// # Safety
///
/// See [`run_add_batch_avx512`] for the safety contract.
///
/// # Panics
///
/// Always panics: the kernel has no implementation.
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

/// AVX-512 F_3 mul batch.
///
/// # Safety
///
/// See [`run_add_batch_avx512`] for the safety contract.
///
/// # Panics
///
/// Always panics: the kernel has no implementation.
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

/// AVX-512 F_3 neg batch.
///
/// # Safety
///
/// AVX-512F must be available at runtime; all four slices must share a length
/// divisible by 8.
///
/// # Panics
///
/// Always panics: the kernel has no implementation.
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
