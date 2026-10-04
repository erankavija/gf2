//! The [`Backend`] trait for word kernels and the length-based backend
//! selection.

/// Bulk operations on `u64` slices and single-word bit primitives. Every
/// implementation returns the same results.
pub trait Backend: Send + Sync {
    /// Human-readable backend name.
    fn name(&self) -> &'static str;

    /// Performs bitwise AND: dst\[i\] &= src\[i\] for all i.
    ///
    /// # Panics
    /// May panic if dst.len() != src.len() in debug builds.
    fn and(&self, dst: &mut [u64], src: &[u64]);

    /// Performs bitwise OR: dst\[i\] |= src\[i\] for all i.
    ///
    /// # Panics
    /// May panic if dst.len() != src.len() in debug builds.
    fn or(&self, dst: &mut [u64], src: &[u64]);

    /// Performs bitwise XOR: dst\[i\] ^= src\[i\] for all i.
    ///
    /// # Panics
    /// May panic if dst.len() != src.len() in debug builds.
    fn xor(&self, dst: &mut [u64], src: &[u64]);

    /// Performs bitwise NOT: buf\[i\] = !buf\[i\] for all i.
    fn not(&self, buf: &mut [u64]);

    /// Counts the number of set bits across all words.
    fn popcount(&self, buf: &[u64]) -> u64;

    /// Computes XOR parity of a single word (true if odd number of 1s).
    fn parity(&self, word: u64) -> bool {
        crate::kernels::scalar::primitives::parity(word)
    }

    /// Counts trailing zeros in a word (position of lowest set bit).
    ///
    /// Returns 64 if word is zero.
    fn trailing_zeros(&self, word: u64) -> u32 {
        crate::kernels::scalar::primitives::trailing_zeros(word)
    }

    /// Counts leading zeros in a word (63 - position of highest set bit).
    ///
    /// Returns 64 if word is zero.
    fn leading_zeros(&self, word: u64) -> u32 {
        crate::kernels::scalar::primitives::leading_zeros(word)
    }
}

/// Backend selection result.
pub enum SelectedBackend {
    /// Pure Rust scalar implementation.
    Scalar,
    /// SIMD implementation from `gf2-kernels-simd`.
    #[cfg(feature = "simd")]
    Simd,
}

impl SelectedBackend {
    /// Returns the name of the selected backend.
    pub fn name(&self) -> &'static str {
        match self {
            SelectedBackend::Scalar => "scalar",
            #[cfg(feature = "simd")]
            SelectedBackend::Simd => "simd",
        }
    }
}

/// Conservative-table value for `bit_backend.simd_min_words`.
pub(crate) const SIMD_MIN_WORDS_DEFAULT: usize = 8;

#[cfg(all(any(test, feature = "simd"), gf2_tuning_baked))]
const SIMD_MIN_WORDS: usize = crate::tuning::baked::SIMD_MIN_WORDS;

#[cfg(all(any(test, feature = "simd"), not(gf2_tuning_baked)))]
const SIMD_MIN_WORDS: usize = SIMD_MIN_WORDS_DEFAULT;

/// Selects the backend for a buffer of `size` words: SIMD when the `simd`
/// feature is enabled and `size` reaches the compile-time threshold, scalar
/// otherwise.
///
/// The threshold is the conservative table's `bit_backend.simd_min_words`
/// value, or the baked profile's value in a build with
/// `RUSTFLAGS="--cfg gf2_tuning_baked"`. The flag is a declared cfg, not a
/// Cargo feature, so `--all-features` builds keep the conservative threshold.
/// An installed runtime profile does not change it.
#[inline]
pub fn select_backend_for_size(_size: usize) -> SelectedBackend {
    #[cfg(feature = "simd")]
    if _size >= SIMD_MIN_WORDS {
        return SelectedBackend::Simd;
    }

    SelectedBackend::Scalar
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend;

    impl Backend for MockBackend {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn and(&self, dst: &mut [u64], src: &[u64]) {
            for i in 0..dst.len().min(src.len()) {
                dst[i] &= src[i];
            }
        }

        fn or(&self, dst: &mut [u64], src: &[u64]) {
            for i in 0..dst.len().min(src.len()) {
                dst[i] |= src[i];
            }
        }

        fn xor(&self, dst: &mut [u64], src: &[u64]) {
            for i in 0..dst.len().min(src.len()) {
                dst[i] ^= src[i];
            }
        }

        fn not(&self, buf: &mut [u64]) {
            for word in buf.iter_mut() {
                *word = !*word;
            }
        }

        fn popcount(&self, buf: &[u64]) -> u64 {
            buf.iter().map(|w| w.count_ones() as u64).sum()
        }
    }

    #[test]
    fn test_backend_trait_and() {
        let backend = MockBackend;
        let mut dst = vec![0xFF, 0xFF];
        let src = vec![0x0F, 0xF0];
        backend.and(&mut dst, &src);
        assert_eq!(dst, vec![0x0F, 0xF0]);
    }

    #[test]
    fn test_backend_trait_or() {
        let backend = MockBackend;
        let mut dst = vec![0xF0, 0x0F];
        let src = vec![0x0F, 0xF0];
        backend.or(&mut dst, &src);
        assert_eq!(dst, vec![0xFF, 0xFF]);
    }

    #[test]
    fn test_backend_trait_xor() {
        let backend = MockBackend;
        let mut dst = vec![0xFF, 0xFF];
        let src = vec![0x0F, 0xF0];
        backend.xor(&mut dst, &src);
        assert_eq!(dst, vec![0xF0, 0x0F]);
    }

    #[test]
    fn test_backend_trait_not() {
        let backend = MockBackend;
        let mut buf = vec![0xFFFFFFFFFFFFFFFFu64, 0x0000000000000000u64];
        backend.not(&mut buf);
        assert_eq!(buf, vec![0x0000000000000000u64, 0xFFFFFFFFFFFFFFFFu64]);
    }

    #[test]
    fn test_backend_trait_popcount() {
        let backend = MockBackend;
        let buf = vec![0xFFu64, 0xF0F0F0F0F0F0F0F0u64];
        let count = backend.popcount(&buf);
        assert_eq!(count, 8 + 32);
    }

    #[test]
    fn test_backend_trait_parity_default() {
        let backend = MockBackend;
        assert!(!backend.parity(0));
        assert!(backend.parity(1));
        assert!(!backend.parity(3)); // 0b11 = 2 bits
        assert!(backend.parity(7)); // 0b111 = 3 bits
    }

    #[test]
    fn test_backend_trait_trailing_zeros_default() {
        let backend = MockBackend;
        assert_eq!(backend.trailing_zeros(0), 64);
        assert_eq!(backend.trailing_zeros(1), 0);
        assert_eq!(backend.trailing_zeros(2), 1);
        assert_eq!(backend.trailing_zeros(8), 3);
        assert_eq!(backend.trailing_zeros(1u64 << 63), 63);
    }

    #[test]
    fn test_backend_trait_leading_zeros_default() {
        let backend = MockBackend;
        assert_eq!(backend.leading_zeros(0), 64);
        assert_eq!(backend.leading_zeros(1), 63);
        assert_eq!(backend.leading_zeros(2), 62);
        assert_eq!(backend.leading_zeros(1u64 << 63), 0);
    }

    #[test]
    fn test_select_backend_small_size() {
        let backend = select_backend_for_size(1);
        assert_eq!(backend.name(), "scalar");

        let backend = select_backend_for_size(SIMD_MIN_WORDS - 1);
        assert_eq!(backend.name(), "scalar");
    }

    #[test]
    fn test_select_backend_at_threshold() {
        let backend = select_backend_for_size(SIMD_MIN_WORDS);
        #[cfg(feature = "simd")]
        assert_eq!(backend.name(), "simd");
        #[cfg(not(feature = "simd"))]
        assert_eq!(backend.name(), "scalar");
    }

    #[test]
    fn test_select_backend_large_size() {
        let backend = select_backend_for_size(16);
        #[cfg(feature = "simd")]
        assert_eq!(backend.name(), "simd");
        #[cfg(not(feature = "simd"))]
        assert_eq!(backend.name(), "scalar");

        let backend = select_backend_for_size(1000);
        #[cfg(feature = "simd")]
        assert_eq!(backend.name(), "simd");
        #[cfg(not(feature = "simd"))]
        assert_eq!(backend.name(), "scalar");
    }

    #[test]
    fn test_select_backend_empty() {
        let backend = select_backend_for_size(0);
        assert_eq!(backend.name(), "scalar");
    }

    #[test]
    fn test_backend_name() {
        let scalar = SelectedBackend::Scalar;
        assert_eq!(scalar.name(), "scalar");

        #[cfg(feature = "simd")]
        {
            let simd = SelectedBackend::Simd;
            assert_eq!(simd.name(), "simd");
        }
    }
}
