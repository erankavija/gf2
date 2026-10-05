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

/// The list form of a shared equivalence test: every implementation a build
/// publishes, the portable reference first, so the test compares at least one
/// implementation on every host and reports which.
///
/// This module's own tests run the [`Backend`] cases over
/// [`contract::backends`], and the `simd_equiv_*` integration tests of this
/// crate run theirs through [`contract::assert_each`].
#[cfg(any(test, feature = "test-support"))]
pub mod contract {
    use super::Backend;

    /// One implementation under test.
    pub struct Implementation<T> {
        /// Names the implementation in the report.
        pub label: String,
        /// What a check is handed.
        pub subject: T,
    }

    impl<T> Implementation<T> {
        /// `subject` under `label`.
        pub fn new(label: impl Into<String>, subject: T) -> Self {
            Self {
                label: label.into(),
                subject,
            }
        }
    }

    /// Every [`Backend`] of this build: the scalar reference, present on
    /// every host, and the detected SIMD backend under the `simd` feature.
    pub fn backends() -> Vec<Implementation<&'static dyn Backend>> {
        let scalar: &'static dyn Backend = &crate::kernels::scalar::SCALAR_BACKEND;
        #[allow(unused_mut)]
        let mut backends = vec![Implementation::new(scalar.name(), scalar)];
        #[cfg(feature = "simd")]
        if let Some(simd) = crate::kernels::simd::maybe_simd() {
            backends.push(Implementation::new(simd.name(), simd as &dyn Backend));
        }
        backends
    }

    /// Runs `check` on every implementation, printing each label before its
    /// run so a failure follows the name of the implementation that failed.
    ///
    /// # Panics
    ///
    /// Panics when `implementations` is empty or `check` panics.
    pub fn assert_each<T>(implementations: &[Implementation<T>], check: impl Fn(&T)) {
        assert!(
            !implementations.is_empty(),
            "no implementation was enumerated"
        );
        for implementation in implementations {
            println!("implementation: {}", implementation.label);
            check(&implementation.subject);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::contract::{assert_each, backends};
    use super::*;
    use crate::bench_seed::splitmix64;
    use crate::kernels::test_utils;

    /// Word counts on both sides of the four-word vector step and of the
    /// selection threshold, up to several cache lines.
    const WORD_COUNTS: [usize; 26] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16, 17, 63, 64, 65, 127, 128, 256, 512,
        1024, 1025,
    ];

    fn words(count: usize, seed: u64) -> Vec<u64> {
        let mut state = seed;
        (0..count).map(|_| splitmix64(&mut state)).collect()
    }

    /// Requires `kernel` to leave in `dst` the word-by-word image of `word`.
    fn assert_binary(
        operation: &str,
        kernel: impl Fn(&dyn Backend, &mut [u64], &[u64]),
        word: impl Fn(u64, u64) -> u64,
    ) {
        assert_each(&backends(), |backend| {
            for count in WORD_COUNTS {
                let dst = words(count, 0xDEAD_BEEF);
                let src = words(count, 0xCAFE_BABE);
                let expected: Vec<u64> = dst.iter().zip(&src).map(|(&a, &b)| word(a, b)).collect();
                let mut got = dst;
                kernel(*backend, &mut got, &src);
                assert_eq!(
                    got,
                    expected,
                    "{} {operation}, {count} words",
                    backend.name()
                );
            }
        });
    }

    #[test]
    fn every_backend_xors_word_by_word() {
        assert_binary(
            "xor",
            |backend, dst, src| backend.xor(dst, src),
            |a, b| a ^ b,
        );
    }

    #[test]
    fn every_backend_ands_word_by_word() {
        assert_binary(
            "and",
            |backend, dst, src| backend.and(dst, src),
            |a, b| a & b,
        );
    }

    #[test]
    fn every_backend_ors_word_by_word() {
        assert_binary("or", |backend, dst, src| backend.or(dst, src), |a, b| a | b);
    }

    #[test]
    fn every_backend_complements_word_by_word() {
        assert_each(&backends(), |backend| {
            for count in WORD_COUNTS {
                let original = words(count, 0xFEED_FACE);
                let expected: Vec<u64> = original.iter().map(|&word| !word).collect();
                let mut got = original.clone();
                backend.not(&mut got);
                assert_eq!(got, expected, "{} not, {count} words", backend.name());
                backend.not(&mut got);
                assert_eq!(got, original, "{} not twice, {count} words", backend.name());
            }
        });
    }

    #[test]
    fn every_backend_counts_set_bits() {
        assert_each(&backends(), |backend| {
            for count in WORD_COUNTS {
                let buf = words(count, 0xC0FF_EE00);
                let expected: u64 = buf.iter().map(|word| u64::from(word.count_ones())).sum();
                assert_eq!(
                    backend.popcount(&buf),
                    expected,
                    "{}, {count} words",
                    backend.name()
                );
                assert_eq!(backend.popcount(&vec![0u64; count]), 0);
                assert_eq!(backend.popcount(&vec![u64::MAX; count]), 64 * count as u64);
            }
        });
    }

    #[test]
    fn every_backend_xor_is_an_involution() {
        assert_each(&backends(), |backend| {
            for count in WORD_COUNTS {
                let original = words(count, 0x1111_1111);
                let src = words(count, 0x2222_2222);
                let mut buf = original.clone();
                backend.xor(&mut buf, &src);
                backend.xor(&mut buf, &src);
                assert_eq!(buf, original, "{}, {count} words", backend.name());

                let mut alternating = vec![0xAAAA_AAAA_AAAA_AAAAu64; count];
                backend.xor(&mut alternating, &vec![0x5555_5555_5555_5555u64; count]);
                assert_eq!(alternating, vec![u64::MAX; count]);
            }
        });
    }

    #[test]
    fn every_backend_answers_the_fixed_vectors() {
        assert_each(&backends(), |backend| {
            test_utils::test_backend_and_equivalence(*backend);
            test_utils::test_backend_or_equivalence(*backend);
            test_utils::test_backend_xor_equivalence(*backend);
            test_utils::test_backend_not_equivalence(*backend);
            test_utils::test_backend_popcount_equivalence(*backend);
            test_utils::test_backend_parity_xor_property(*backend);
            test_utils::test_backend_parity_correctness(*backend);
            test_utils::test_backend_trailing_zeros_correctness(*backend);
            test_utils::test_backend_leading_zeros_correctness(*backend);
            test_utils::test_backend_empty_slices(*backend);
            test_utils::test_backend_single_word(*backend);
            test_utils::test_backend_large_slice(*backend);
        });
    }

    #[test]
    #[should_panic(expected = "no implementation was enumerated")]
    fn an_empty_implementation_list_is_refused() {
        assert_each(&Vec::<contract::Implementation<()>>::new(), |_| {});
    }

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
