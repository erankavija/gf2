//! Kernel operations for matrix algorithms.
//!
//! This module provides low-level primitives for matrix operations over GF(2).
//! Currently provides scalar implementations; SIMD optimizations can be added later.

/// XORs source slice into destination slice in-place.
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
    let len = dst.len().min(src.len());
    for i in 0..len {
        dst[i] ^= src[i];
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
    #[should_panic(expected = "xor_inplace: dst and src must have same length")]
    #[cfg(debug_assertions)]
    fn test_xor_inplace_length_mismatch_panics() {
        let mut dst = vec![0xFF];
        let src = vec![0x0F, 0xF0];
        xor_inplace(&mut dst, &src);
    }
}
