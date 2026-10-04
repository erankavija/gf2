//! Word-slice operations that dispatch between the scalar and SIMD backends
//! through [`select_backend_for_size`](crate::kernels::select_backend_for_size)
//! and runtime CPU detection.

/// In-place XOR resolved by [`resolve_xor_inplace`] for one word width, so a
/// loop over rows of that width selects the backend once.
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

/// Returns the XOR that [`xor_inplace`] applies to `word_len` words: the
/// detected SIMD `LogicalFns::xor_fn` when the backend selection is SIMD and
/// the kernel bundle is available, otherwise the scalar backend's.
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

/// Performs bitwise XOR: dst\[i\] ^= src\[i\] for all i.
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

/// Route [`resolve_popcount`] and [`resolve_and_popcount`] take for a width,
/// reported by [`popcount_route`] and [`and_popcount_route`].
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

/// Reports the route [`resolve_popcount`] takes for `word_len` words:
/// [`PopcountRoute::Scalar`] below the compile-time SIMD threshold and on a
/// host without the kernel bundle.
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

/// Reports the route [`resolve_and_popcount`] takes for `word_len` words,
/// which is the route [`popcount_route`] reports.
#[must_use]
pub fn and_popcount_route(word_len: usize) -> PopcountRoute {
    popcount_route(word_len)
}

/// Population count resolved by [`resolve_popcount`] for one word width.
pub type PopcountFn = fn(&[u64]) -> u64;

/// Fused AND-population-count resolved by [`resolve_and_popcount`] for one
/// word width.
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

/// Returns the population count for `word_len` words: `LogicalFns::popcnt_fn`,
/// a per-vector `VPSHUFB` nibble lookup, at and above the SIMD threshold when
/// the kernel bundle is available, otherwise the scalar backend's count.
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

/// Returns the count of `lhs & rhs` for `word_len` words, computed without
/// materializing the AND: `LogicalFns::and_popcnt_fn` at and above the SIMD
/// threshold when the kernel bundle is available, otherwise a scalar fused
/// count. The count covers the shorter of the two slices.
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

/// Counts the set bits of `lhs & rhs` over the shorter slice.
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
        // One word below the conservative SIMD boundary.
        let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; 7];
        let src = vec![0x0F0F0F0F0F0F0F0Fu64; 7];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xF0F0F0F0F0F0F0F0u64; 7]);
    }

    #[test]
    fn test_xor_inplace_at_threshold() {
        // The conservative SIMD boundary.
        let mut dst = vec![0xFFFFFFFFFFFFFFFFu64; 8];
        let src = vec![0x0F0F0F0F0F0F0F0Fu64; 8];
        xor_inplace(&mut dst, &src);
        assert_eq!(dst, vec![0xF0F0F0F0F0F0F0F0u64; 8]);
    }

    #[test]
    fn test_xor_inplace_large_buffer() {
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

        for i in 0..len {
            expected[i] ^= src[i];
        }

        xor_inplace(&mut dst, &src);

        for i in 0..len {
            assert!(dst[i] == expected[i]);
        }
    }

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
