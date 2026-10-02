//! Kernel operations for matrix algorithms.
//!
//! This module provides low-level primitives for matrix operations over GF(2).
//! Smart backend dispatch automatically selects scalar or SIMD implementations
//! based on buffer size and CPU capabilities.

/// Resolved in-place XOR operation for fixed-width hot loops.
///
/// Call [`resolve_xor_inplace`] once before entering a loop that repeatedly
/// XORs rows with the same word width, then invoke the returned function
/// pointer inside the loop. This hoists backend selection and SIMD runtime
/// probing out of the hot path while preserving [`xor_inplace`] semantics.
pub type XorInplaceFn = fn(&mut [u64], &[u64]);

#[inline]
fn scalar_xor_inplace(dst: &mut [u64], src: &[u64]) {
    debug_assert_eq!(
        dst.len(),
        src.len(),
        "xor_inplace: dst and src must have same length"
    );

    crate::kernels::Backend::xor(&crate::kernels::scalar::SCALAR_BACKEND, dst, src);
}

/// Resolves the best in-place XOR implementation for `word_len` words.
///
/// The returned function pointer is either the scalar backend XOR or, when
/// the `simd` feature is enabled and the runtime backend is available, the
/// detected SIMD `LogicalFns::xor_fn`. Resolving once is preferable in
/// repeated row-XOR loops because it avoids a per-call backend-size branch and
/// SIMD detection probe.
///
/// # Arguments
///
/// * `word_len` - The fixed number of `u64` words that each later XOR call
///   will process.
///
/// # Examples
///
/// ```
/// use gf2_core::kernels::ops::resolve_xor_inplace;
///
/// let xor = resolve_xor_inplace(2);
/// let mut dst = vec![0xFF, 0x00];
/// let src = vec![0x0F, 0xF0];
/// xor(&mut dst, &src);
/// assert_eq!(dst, vec![0xF0, 0xF0]);
/// ```
///
/// # Complexity
///
/// O(1) to resolve; the returned function runs in O(`word_len`).
#[inline]
pub fn resolve_xor_inplace(word_len: usize) -> XorInplaceFn {
    use crate::kernels::backend::select_backend_for_size;

    match select_backend_for_size(word_len) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => crate::simd::maybe_simd()
            .map(|backend| backend.xor_fn)
            .unwrap_or(scalar_xor_inplace),
        crate::kernels::backend::SelectedBackend::Scalar => scalar_xor_inplace,
    }
}

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
#[inline]
pub fn xor_inplace(dst: &mut [u64], src: &[u64]) {
    debug_assert_eq!(
        dst.len(),
        src.len(),
        "xor_inplace: dst and src must have same length"
    );

    let xor = resolve_xor_inplace(dst.len());
    xor(dst, src);
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

/// Route [`resolve_popcount`] and [`resolve_and_popcount`] take for a width.
///
/// Reported by [`popcount_route`] and [`and_popcount_route`] so a caller, a
/// test or a benchmark arm can observe the SIMD boundary at run time instead
/// of inferring it from build configuration, as `matrix::matvec_route` does
/// for the matrix-vector product.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopcountRoute {
    /// The scalar backend's portable count, which every width below the
    /// bit-backend SIMD threshold retains and which a host without the kernel
    /// bundle takes at every width.
    Scalar,
    /// Every vector through a `VPSHUFB` nibble lookup summed by `VPSADBW`.
    SimdNibbleLut,
}

impl PopcountRoute {
    /// A stable lowercase name for provenance records.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::SimdNibbleLut => "simd-nibble-lut",
        }
    }
}

/// Reports the route [`resolve_popcount`] takes for `word_len` words.
///
/// The boundary is the compile-time counterpart of
/// `bit_backend.simd_min_words`. Every width below it reports
/// [`PopcountRoute::Scalar`], and so does every width on a host without the
/// kernel bundle. The carry-save candidate is not selected because its
/// confirmation receipt does not qualify under the shared measurement
/// contract.
#[must_use]
pub fn popcount_route(word_len: usize) -> PopcountRoute {
    use crate::kernels::backend::select_backend_for_size;

    match select_backend_for_size(word_len) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => {
            if crate::simd::maybe_simd().is_none() {
                PopcountRoute::Scalar
            } else {
                PopcountRoute::SimdNibbleLut
            }
        }
        crate::kernels::backend::SelectedBackend::Scalar => PopcountRoute::Scalar,
    }
}

/// Reports the route [`resolve_and_popcount`] takes for `word_len` words.
///
/// The two resolvers share the SIMD boundary, so this reports what
/// [`popcount_route`] reports: scalar below the threshold or when the SIMD
/// bundle is unavailable, and the established fused nibble-lookup route at and
/// above the threshold when that bundle is available. The carry-save candidate
/// remains available in the kernel bundle for conformance and research but is
/// not selected automatically.
#[must_use]
pub fn and_popcount_route(word_len: usize) -> PopcountRoute {
    popcount_route(word_len)
}

/// Resolved population-count operation for fixed-width hot loops.
///
/// Call [`resolve_popcount`] once before entering a loop that repeatedly
/// counts buffers of the same word width, then invoke the returned function
/// pointer inside the loop, as [`XorInplaceFn`] does for in-place XOR.
pub type PopcountFn = fn(&[u64]) -> u64;

/// Resolved fused AND-population-count operation for fixed-width hot loops.
pub type AndPopcountFn = fn(&[u64], &[u64]) -> u64;

#[inline]
fn scalar_popcount(buf: &[u64]) -> u64 {
    crate::kernels::Backend::popcount(&crate::kernels::scalar::SCALAR_BACKEND, buf)
}

#[inline]
fn scalar_and_popcount(lhs: &[u64], rhs: &[u64]) -> u64 {
    lhs.iter()
        .zip(rhs)
        .map(|(left, right)| u64::from((left & right).count_ones()))
        .sum()
}

/// Resolves the established population-count implementation for `word_len`
/// words.
///
/// At and above the bit-backend SIMD threshold, `LogicalFns::popcnt_fn`
/// counts every vector through a `VPSHUFB` nibble lookup. The surveyed scalar
/// POPCNT and Harley-Seal candidates remain available in the detected bundle,
/// but neither confirmation receipt qualifies for production selection under
/// the shared measurement contract.
///
/// Every width below the SIMD threshold keeps the scalar backend's portable
/// count. The bundle's `LogicalFns::popcnt_scalar_fn`, which counts one word at
/// a time with the host's `POPCNT` instruction, loses to that count through a
/// resolved call on the measured host, so this region retains its established
/// implementation: cell `popcount-w4-dispatch` of the confirmation receipt
/// under `dev/bench_results/5cbb6545/` records the regression that decides it.
///
/// Without the feature or the backend every width takes the scalar fallback.
/// The SIMD boundary is a compile-time constant, per DEC-G in
/// `dev/active/220cab0b/design.md`, and its profile field carries it for
/// calibration.
///
/// # Complexity
///
/// O(1) to resolve; the returned function runs in O(`word_len`).
#[inline]
pub fn resolve_popcount(word_len: usize) -> PopcountFn {
    use crate::kernels::backend::select_backend_for_size;

    match select_backend_for_size(word_len) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => crate::simd::maybe_simd()
            .map(|backend| backend.popcnt_fn)
            .unwrap_or(scalar_popcount),
        crate::kernels::backend::SelectedBackend::Scalar => scalar_popcount,
    }
}

/// Resolves the established fused AND-population-count implementation for
/// `word_len` words.
///
/// The fused kernels count `lhs & rhs` without materializing the AND, so a
/// consumer that only needs the weight of an intersection avoids a temporary
/// buffer and a second pass. Automatic dispatch uses the scalar fused fallback
/// below the bit-backend SIMD threshold or when the SIMD bundle is unavailable.
/// At and above the threshold with that bundle available, it retains the
/// per-vector nibble lookup because the carry-save family's confirmation
/// receipt does not qualify for production selection.
///
/// The count covers the shorter of the two slices.
///
/// # Complexity
///
/// O(1) to resolve; the returned function runs in O(`word_len`).
#[inline]
pub fn resolve_and_popcount(word_len: usize) -> AndPopcountFn {
    use crate::kernels::backend::select_backend_for_size;

    match select_backend_for_size(word_len) {
        #[cfg(feature = "simd")]
        crate::kernels::backend::SelectedBackend::Simd => crate::simd::maybe_simd()
            .map(|backend| backend.and_popcnt_fn)
            .unwrap_or(scalar_and_popcount),
        crate::kernels::backend::SelectedBackend::Scalar => scalar_and_popcount,
    }
}

/// Counts the number of set bits across all words.
///
/// Automatically selects the established backend based on buffer size. The
/// carry-save comparator is not selected because its confirmation receipt does
/// not qualify under the shared measurement contract.
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

/// Counts the set bits of the intersection of two bit buffers.
///
/// Resolves the route for this width through [`resolve_and_popcount`] and
/// calls it once. The count covers the shorter slice.
#[inline]
pub fn and_popcount(lhs: &[u64], rhs: &[u64]) -> u64 {
    resolve_and_popcount(lhs.len().min(rhs.len()))(lhs, rhs)
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
        // Exactly 8 words - the conservative default boundary; a profile may
        // move the SIMD route.
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

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Verify that xor_inplace matches naive word-by-word XOR.
    ///
    /// For symbolic data of length 1-2, the dispatch function must produce
    /// the same result as simple `dst[i] ^= src[i]`.
    #[kani::proof]
    #[kani::unwind(3)]
    fn xor_equivalence() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 2);

        let mut dst = vec![0u64; len];
        let mut expected = vec![0u64; len];
        let mut src = vec![0u64; len];

        for i in 0..len {
            dst[i] = kani::any();
            expected[i] = dst[i];
            src[i] = kani::any();
        }

        // Naive reference
        for i in 0..len {
            expected[i] ^= src[i];
        }

        xor_inplace(&mut dst, &src);

        for i in 0..len {
            assert!(dst[i] == expected[i]);
        }
    }

    /// Verify that and_inplace matches naive word-by-word AND.
    #[kani::proof]
    #[kani::unwind(3)]
    fn and_equivalence() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 2);

        let mut dst = vec![0u64; len];
        let mut expected = vec![0u64; len];
        let mut src = vec![0u64; len];

        for i in 0..len {
            dst[i] = kani::any();
            expected[i] = dst[i];
            src[i] = kani::any();
        }

        for i in 0..len {
            expected[i] &= src[i];
        }

        and_inplace(&mut dst, &src);

        for i in 0..len {
            assert!(dst[i] == expected[i]);
        }
    }

    /// Verify that or_inplace matches naive word-by-word OR.
    #[kani::proof]
    #[kani::unwind(3)]
    fn or_equivalence() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 2);

        let mut dst = vec![0u64; len];
        let mut expected = vec![0u64; len];
        let mut src = vec![0u64; len];

        for i in 0..len {
            dst[i] = kani::any();
            expected[i] = dst[i];
            src[i] = kani::any();
        }

        for i in 0..len {
            expected[i] |= src[i];
        }

        or_inplace(&mut dst, &src);

        for i in 0..len {
            assert!(dst[i] == expected[i]);
        }
    }

    /// Verify that not_inplace matches naive word-by-word NOT.
    #[kani::proof]
    #[kani::unwind(3)]
    fn not_equivalence() {
        let len: usize = kani::any();
        kani::assume(len >= 1 && len <= 2);

        let mut buf = vec![0u64; len];
        let mut expected = vec![0u64; len];

        for i in 0..len {
            let v: u64 = kani::any();
            buf[i] = v;
            expected[i] = !v;
        }

        not_inplace(&mut buf);

        for i in 0..len {
            assert!(buf[i] == expected[i]);
        }
    }
}
