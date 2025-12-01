//! Kernel operations for matrix algorithms.
//!
//! This module provides low-level primitives for matrix operations over GF(2).
//! Smart backend dispatch automatically selects scalar or SIMD implementations
//! based on buffer size and CPU capabilities.

/// XORs source slice into destination slice in-place.
///
/// This operation is used heavily in matrix algorithms (M4RM multiplication,
/// Gauss-Jordan elimination) and benefits significantly from SIMD acceleration
/// on large buffers.
///
/// # Arguments
///
/// * `dst` - Destination slice to be modified
/// * `src` - Source slice to XOR with destination
///
/// # Panics
///
/// Panics in debug mode if slices have different lengths.
///
/// # Examples
///
/// ```
/// use gf2_core::kernels::ops::xor_inplace;
///
/// let mut dst = vec![0xFF, 0x00];
/// let src = vec![0x0F, 0xF0];
/// xor_inplace(&mut dst, &src);
/// assert_eq!(dst, vec![0xF0, 0xF0]);
/// ```
#[inline]
pub fn xor_inplace(dst: &mut [u64], src: &[u64]) {
    debug_assert_eq!(
        dst.len(),
        src.len(),
        "xor_inplace: dst and src must have same length"
    );

    use crate::kernels::{backend::select_backend_for_size, Backend};

    match select_backend_for_size(dst.len()) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                backend.xor(dst, src);
            } else {
                // Fallback if SIMD not available at runtime
                crate::kernels::scalar::SCALAR_BACKEND.xor(dst, src);
            }
        }
        crate::kernels::backend::SelectedBackend::Scalar => {
            crate::kernels::scalar::SCALAR_BACKEND.xor(dst, src);
        }
    }
}

/// Performs bitwise AND: dst\[i\] &= src\[i\] for all i.
///
/// Automatically selects the best backend based on buffer size.
///
/// # Arguments
///
/// * `dst` - Destination slice to be modified
/// * `src` - Source slice to AND with destination
///
/// # Panics
///
/// Panics in debug mode if slices have different lengths.
#[inline]
pub fn and_inplace(dst: &mut [u64], src: &[u64]) {
    debug_assert_eq!(
        dst.len(),
        src.len(),
        "and_inplace: dst and src must have same length"
    );

    use crate::kernels::{backend::select_backend_for_size, Backend};

    match select_backend_for_size(dst.len()) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                backend.and(dst, src);
            } else {
                crate::kernels::scalar::SCALAR_BACKEND.and(dst, src);
            }
        }
        crate::kernels::backend::SelectedBackend::Scalar => {
            crate::kernels::scalar::SCALAR_BACKEND.and(dst, src);
        }
    }
}

/// Performs bitwise OR: dst\[i\] |= src\[i\] for all i.
///
/// Automatically selects the best backend based on buffer size.
///
/// # Arguments
///
/// * `dst` - Destination slice to be modified
/// * `src` - Source slice to OR with destination
///
/// # Panics
///
/// Panics in debug mode if slices have different lengths.
#[inline]
pub fn or_inplace(dst: &mut [u64], src: &[u64]) {
    debug_assert_eq!(
        dst.len(),
        src.len(),
        "or_inplace: dst and src must have same length"
    );

    use crate::kernels::{backend::select_backend_for_size, Backend};

    match select_backend_for_size(dst.len()) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                backend.or(dst, src);
            } else {
                crate::kernels::scalar::SCALAR_BACKEND.or(dst, src);
            }
        }
        crate::kernels::backend::SelectedBackend::Scalar => {
            crate::kernels::scalar::SCALAR_BACKEND.or(dst, src);
        }
    }
}

/// Performs bitwise NOT: buf\[i\] = !buf\[i\] for all i.
///
/// Automatically selects the best backend based on buffer size.
#[inline]
pub fn not_inplace(buf: &mut [u64]) {
    use crate::kernels::{backend::select_backend_for_size, Backend};

    match select_backend_for_size(buf.len()) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                backend.not(buf);
            } else {
                crate::kernels::scalar::SCALAR_BACKEND.not(buf);
            }
        }
        crate::kernels::backend::SelectedBackend::Scalar => {
            crate::kernels::scalar::SCALAR_BACKEND.not(buf);
        }
    }
}

/// Counts the number of set bits across all words.
///
/// Automatically selects the best backend based on buffer size.
#[inline]
pub fn popcount(buf: &[u64]) -> u64 {
    use crate::kernels::{backend::select_backend_for_size, Backend};

    match select_backend_for_size(buf.len()) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => {
            if let Some(backend) = crate::kernels::simd::maybe_simd() {
                backend.popcount(buf)
            } else {
                crate::kernels::scalar::SCALAR_BACKEND.popcount(buf)
            }
        }
        crate::kernels::backend::SelectedBackend::Scalar => {
            crate::kernels::scalar::SCALAR_BACKEND.popcount(buf)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_inplace_basic() {
        let mut dst = vec![0xFF, 0x00];
        let src = vec![0x0F, 0xF0];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xF0, 0xF0]);
    }

    #[test]
    fn test_xor_inplace_identical() {
        let mut dst = vec![0xAAAAAAAAAAAAAAAAu64, 0x5555555555555555u64];
        let src = vec![0xAAAAAAAAAAAAAAAAu64, 0x5555555555555555u64];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0, 0]);
    }

    #[test]
    fn test_xor_inplace_empty() {
        let mut dst: Vec<u64> = vec![];
        let src: Vec<u64> = vec![];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst.len(), 0);
    }

    #[test]
    fn test_xor_inplace_single_word() {
        let mut dst = vec![0xFFFFFFFFFFFFFFFFu64];
        let src = vec![0x0F0F0F0F0F0F0F0Fu64];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xF0F0F0F0F0F0F0F0u64]);
    }

    #[test]
    fn test_xor_inplace_small_buffer() {
        // 7 words - should use scalar backend
        let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; 7];
        let src = vec![0x0F0F0F0F0F0F0F0Fu64; 7];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xF0F0F0F0F0F0F0F0u64; 7]);
    }

    #[test]
    fn test_xor_inplace_at_threshold() {
        // Exactly 8 words - should potentially use SIMD
        let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; 8];
        let src = vec![0x0F0F0F0F0F0F0F0Fu64; 8];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xF0F0F0F0F0F0F0F0u64; 8]);
    }

    #[test]
    fn test_xor_inplace_large_buffer() {
        // Large buffer - should use SIMD if available
        let mut dst = vec![0xAAAAAAAAAAAAAAAAu64; 256];
        let src = vec![0x5555555555555555u64; 256];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xFFFFFFFFFFFFFFFFu64; 256]);
    }

    #[test]
    #[should_panic(expected = "xor_inplace: dst and src must have same length")]
    #[cfg(debug_assertions)]
    fn test_xor_inplace_length_mismatch_panics() {
        let mut dst = vec![0xFF];
        let src = vec![0x0F, 0xF0];
        xor_inplace(&mut dst, &src);
    }

    // AND operation tests
    #[test]
    fn test_and_inplace_basic() {
        let mut dst = vec![0xFF, 0xFF];
        let src = vec![0x0F, 0xF0];
        and_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0x0F, 0xF0]);
    }

    #[test]
    fn test_and_inplace_large_buffer() {
        let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; 256];
        let src = vec![0x5555555555555555u64; 256];
        and_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0x5555555555555555u64; 256]);
    }

    // OR operation tests
    #[test]
    fn test_or_inplace_basic() {
        let mut dst = vec![0xF0, 0x0F];
        let src = vec![0x0F, 0xF0];
        or_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xFF, 0xFF]);
    }

    #[test]
    fn test_or_inplace_large_buffer() {
        let mut dst = vec![0xAAAAAAAAAAAAAAAAu64; 256];
        let src = vec![0x5555555555555555u64; 256];
        or_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xFFFFFFFFFFFFFFFFu64; 256]);
    }

    // NOT operation tests
    #[test]
    fn test_not_inplace_basic() {
        let mut buf = vec![0xFFFFFFFFFFFFFFFFu64, 0x0000000000000000u64];
        not_inplace(&mut buf);
        assert_eq!(buf, vec![0x0000000000000000u64, 0xFFFFFFFFFFFFFFFFu64]);
    }

    #[test]
    fn test_not_inplace_large_buffer() {
        let mut buf = vec![0xAAAAAAAAAAAAAAAAu64; 256];
        not_inplace(&mut buf);
        assert_eq!(buf, vec![0x5555555555555555u64; 256]);
    }

    #[test]
    fn test_not_inplace_empty() {
        let mut buf: Vec<u64> = vec![];
        not_inplace(&mut buf);
        assert_eq!(buf.len(), 0);
    }

    // Popcount operation tests
    #[test]
    fn test_popcount_basic() {
        let buf = vec![0xFFu64, 0xF0F0F0F0F0F0F0F0u64];
        let count = popcount(&buf);
        assert_eq!(count, 8 + 32);
    }

    #[test]
    fn test_popcount_empty() {
        let buf: Vec<u64> = vec![];
        let count = popcount(&buf);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_popcount_large_buffer() {
        let buf = vec![0xFFFFFFFFFFFFFFFFu64; 256];
        let count = popcount(&buf);
        assert_eq!(count, 64 * 256);
    }

    #[test]
    fn test_popcount_zeros() {
        let buf = vec![0u64; 100];
        let count = popcount(&buf);
        assert_eq!(count, 0);
    }
}
