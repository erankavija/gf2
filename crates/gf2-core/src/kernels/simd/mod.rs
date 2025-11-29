//! SIMD backend implementation.
//!
//! This module wraps the `gf2-kernels-simd` crate and provides a `Backend`
//! implementation that uses AVX2/AVX-512 (x86) or NEON (ARM) instructions.
//!
//! The SIMD backend is optional and requires the `simd` feature flag.
//! Runtime CPU feature detection ensures we only use instructions that are available.

use crate::kernels::Backend;
use std::sync::LazyLock;

/// SIMD backend using AVX2/NEON instructions.
///
/// This backend wraps function pointers from `gf2-kernels-simd` which uses
/// unsafe intrinsics but exposes a safe API.
#[derive(Copy, Clone)]
pub struct SimdBackend {
    fns: gf2_kernels_simd::LogicalFns,
    name: &'static str,
}

impl SimdBackend {
    /// Attempt to detect and create a SIMD backend for the current CPU.
    ///
    /// Returns `None` if no suitable SIMD instructions are available.
    pub fn detect() -> Option<Self> {
        gf2_kernels_simd::detect().map(|fns| SimdBackend {
            fns,
            name: "avx2", // TODO: Actually detect which variant
        })
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

    // Single-word operations use scalar implementations
    // (SIMD doesn't help for single values)
}

/// Global SIMD backend instance, lazily initialized on first access.
pub static SIMD_BACKEND: LazyLock<Option<SimdBackend>> = LazyLock::new(SimdBackend::detect);

/// Get the SIMD backend if available.
#[inline]
pub fn maybe_simd() -> Option<&'static SimdBackend> {
    SIMD_BACKEND.as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_backend_detection() {
        // This test should pass even if SIMD is not available
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
            // Just verify the trait is implemented correctly
            let _name = backend.name();
            assert!(!_name.is_empty());
        }
    }

    // ===== SIMD vs Scalar Equivalence Tests =====
    // These tests verify that SIMD and Scalar backends produce identical results

    use rand::{Rng, SeedableRng};

    fn random_data(word_count: usize, seed: u64) -> Vec<u64> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        (0..word_count).map(|_| rng.gen()).collect()
    }

    #[test]
    fn test_xor_equivalence_comprehensive() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [
            1, 2, 3, 4, 7, 8, 9, 15, 16, 17, 63, 64, 65, 127, 128, 256, 512, 1024,
        ];

        for &size in &sizes {
            let mut dst_simd = random_data(size, 0xDEADBEEF);
            let mut dst_scalar = dst_simd.clone();
            let src = random_data(size, 0xCAFEBABE);

            simd.xor(&mut dst_simd, &src);
            scalar.xor(&mut dst_scalar, &src);

            assert_eq!(
                dst_simd, dst_scalar,
                "XOR mismatch at size {}: SIMD != Scalar",
                size
            );
        }
    }

    #[test]
    fn test_and_equivalence_comprehensive() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [1, 2, 4, 7, 8, 16, 64, 128, 256, 1024];

        for &size in &sizes {
            let mut dst_simd = random_data(size, 0x12345678);
            let mut dst_scalar = dst_simd.clone();
            let src = random_data(size, 0x87654321);

            simd.and(&mut dst_simd, &src);
            scalar.and(&mut dst_scalar, &src);

            assert_eq!(
                dst_simd, dst_scalar,
                "AND mismatch at size {}: SIMD != Scalar",
                size
            );
        }
    }

    #[test]
    fn test_or_equivalence_comprehensive() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [1, 2, 4, 7, 8, 16, 64, 128, 256, 1024];

        for &size in &sizes {
            let mut dst_simd = random_data(size, 0xABCDEF00);
            let mut dst_scalar = dst_simd.clone();
            let src = random_data(size, 0x00FEDCBA);

            simd.or(&mut dst_simd, &src);
            scalar.or(&mut dst_scalar, &src);

            assert_eq!(
                dst_simd, dst_scalar,
                "OR mismatch at size {}: SIMD != Scalar",
                size
            );
        }
    }

    #[test]
    fn test_not_equivalence_comprehensive() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [1, 2, 4, 7, 8, 16, 64, 128, 256, 1024];

        for &size in &sizes {
            let mut buf_simd = random_data(size, 0xFEEDFACE);
            let mut buf_scalar = buf_simd.clone();

            simd.not(&mut buf_simd);
            scalar.not(&mut buf_scalar);

            assert_eq!(
                buf_simd, buf_scalar,
                "NOT mismatch at size {}: SIMD != Scalar",
                size
            );
        }
    }

    #[test]
    fn test_popcount_equivalence_comprehensive() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [1, 2, 4, 7, 8, 16, 64, 128, 256, 1024];

        for &size in &sizes {
            let buf = random_data(size, 0xC0FFEE00);

            let count_simd = simd.popcount(&buf);
            let count_scalar = scalar.popcount(&buf);

            assert_eq!(
                count_simd, count_scalar,
                "popcount mismatch at size {}: SIMD={}, Scalar={}",
                size, count_simd, count_scalar
            );
        }
    }

    #[test]
    fn test_misaligned_sizes() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        // SIMD processes 4 words at a time, test non-aligned sizes
        let misaligned_sizes = [1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15];

        for &size in &misaligned_sizes {
            let mut dst_simd = random_data(size, 0xBAADF00D);
            let mut dst_scalar = dst_simd.clone();
            let src = random_data(size, 0xF00DFACE);

            simd.xor(&mut dst_simd, &src);
            scalar.xor(&mut dst_scalar, &src);

            assert_eq!(
                dst_simd, dst_scalar,
                "Misaligned XOR failed at size {}",
                size
            );
        }
    }

    #[test]
    fn test_pattern_zeros() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [8, 16, 64, 256];
        for &size in &sizes {
            let buf = vec![0u64; size];
            let count_simd = simd.popcount(&buf);
            let count_scalar = scalar.popcount(&buf);
            assert_eq!(count_simd, count_scalar);
            assert_eq!(count_simd, 0);
        }
    }

    #[test]
    fn test_pattern_ones() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [8, 16, 64, 256];
        for &size in &sizes {
            let buf = vec![0xFFFFFFFFFFFFFFFFu64; size];
            let count_simd = simd.popcount(&buf);
            let count_scalar = scalar.popcount(&buf);
            assert_eq!(count_simd, count_scalar);
            assert_eq!(count_simd, size as u64 * 64);
        }
    }

    #[test]
    fn test_pattern_alternating() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [8, 16, 64, 256];
        for &size in &sizes {
            let mut dst_simd = vec![0xAAAAAAAAAAAAAAAAu64; size];
            let mut dst_scalar = dst_simd.clone();
            let src = vec![0x5555555555555555u64; size];

            simd.xor(&mut dst_simd, &src);
            scalar.xor(&mut dst_scalar, &src);
            assert_eq!(dst_simd, dst_scalar);
        }
    }

    #[test]
    fn test_xor_inverse_property() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [8, 16, 64, 256];
        for &size in &sizes {
            let original = random_data(size, 0x11111111);
            let src = random_data(size, 0x22222222);

            let mut buf_simd = original.clone();
            let mut buf_scalar = original.clone();

            simd.xor(&mut buf_simd, &src);
            scalar.xor(&mut buf_scalar, &src);
            assert_eq!(buf_simd, buf_scalar);

            simd.xor(&mut buf_simd, &src);
            scalar.xor(&mut buf_scalar, &src);
            assert_eq!(buf_simd, buf_scalar);
            assert_eq!(buf_simd, original);
        }
    }

    #[test]
    fn test_not_inverse_property() {
        let simd = match maybe_simd() {
            Some(backend) => backend,
            None => return,
        };
        let scalar = &crate::kernels::scalar::ScalarBackend;

        let sizes = [8, 16, 64, 256];
        for &size in &sizes {
            let original = random_data(size, 0x33333333);
            let mut buf_simd = original.clone();
            let mut buf_scalar = original.clone();

            simd.not(&mut buf_simd);
            scalar.not(&mut buf_scalar);
            assert_eq!(buf_simd, buf_scalar);

            simd.not(&mut buf_simd);
            scalar.not(&mut buf_scalar);
            assert_eq!(buf_simd, buf_scalar);
            assert_eq!(buf_simd, original);
        }
    }
}
