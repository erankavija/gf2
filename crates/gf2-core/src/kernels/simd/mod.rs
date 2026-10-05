//! [`Backend`] over the AVX2 kernels of `gf2-kernels-simd`, selected by
//! runtime CPU detection.

use crate::kernels::Backend;
use std::sync::LazyLock;

/// [`Backend`] over the safe function bundle that `gf2-kernels-simd` detects.
#[derive(Copy, Clone)]
pub struct SimdBackend {
    fns: gf2_kernels_simd::LogicalFns,
    name: &'static str,
}

impl SimdBackend {
    /// Returns the backend for the current CPU, or `None` if it lacks the
    /// required SIMD instructions.
    pub fn detect() -> Option<Self> {
        gf2_kernels_simd::detect().map(|fns| SimdBackend { fns, name: "avx2" })
    }
}

impl Backend for SimdBackend {
    fn name(&self) -> &'static str {
        self.name
    }

    fn and(&self, dst: &mut [u64], src: &[u64]) {
        (self.fns.and_fn)(dst, src)
    }

    fn or(&self, dst: &mut [u64], src: &[u64]) {
        (self.fns.or_fn)(dst, src)
    }

    fn xor(&self, dst: &mut [u64], src: &[u64]) {
        (self.fns.xor_fn)(dst, src)
    }

    fn not(&self, buf: &mut [u64]) {
        (self.fns.not_fn)(buf)
    }

    fn popcount(&self, buf: &[u64]) -> u64 {
        (self.fns.popcnt_fn)(buf)
    }

    // Single-word operations keep the trait's scalar defaults.
}

/// Global SIMD backend instance, lazily initialized on first access.
pub static SIMD_BACKEND: LazyLock<Option<SimdBackend>> = LazyLock::new(SimdBackend::detect);

/// Get the SIMD backend if available.
#[inline]
pub fn maybe_simd() -> Option<&'static SimdBackend> {
    SIMD_BACKEND.as_ref()
}

// The GF(2^m) accessors below expose `crate::simd`, which is `pub(crate)` and
// owns the kernel detection state, to external test crates.

/// GF(2^m) batch kernel bundle; `None` on hosts lacking AVX2, VPCLMULQDQ,
/// PCLMULQDQ or SSE4.1.
#[inline]
pub fn maybe_gf2m_batch() -> Option<&'static gf2_kernels_simd::gf2m_batch::Gf2mBatchFns> {
    crate::simd::maybe_gf2m_batch()
}

/// GF(2^m) panelized GEMM kernel bundle, under the host requirements of
/// [`maybe_gf2m_batch`].
#[inline]
pub fn maybe_gf2m_gemm() -> Option<&'static gf2_kernels_simd::gf2m_gemm::Gf2mGemmFns> {
    crate::simd::maybe_gf2m_gemm()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_backend_detection() {
        let backend = SimdBackend::detect();

        if let Some(backend) = backend {
            println!("SIMD backend available: {}", backend.name());
            assert!(!backend.name().is_empty());
        } else {
            println!("SIMD backend not available on this CPU");
        }
    }

    #[test]
    fn test_simd_backend_implements_trait() {
        if let Some(backend) = &*SIMD_BACKEND {
            let _name = backend.name();
            assert!(!_name.is_empty());
        }
    }
}
