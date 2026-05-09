#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::is_x86_feature_detected;

use crate::LogicalFns;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod avx2;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod bipedal_avx2;
pub(crate) mod clmul;
pub(crate) mod fp65537;
pub(crate) mod fp_generic;
pub(crate) mod fp_medium;
pub(crate) mod fp_small;
pub(crate) mod fp_small_f32;
pub(crate) mod gf2m_batch;
pub(crate) mod gf2m_common;
pub(crate) mod gf2m_gemm;
pub(crate) mod gf2m_wide;
pub(crate) mod mersenne;
pub(crate) mod transpose;

#[allow(dead_code)]
pub(crate) fn detect_x86() -> Option<LogicalFns> {
    // Prefer AVX2; add AVX-512F later when kernels are ready.
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) && is_x86_feature_detected!("avx2") {
        return Some(avx2::fns());
    }
    None
}
