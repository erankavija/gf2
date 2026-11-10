//! Hard decisions of signed values packed under canonical bit indexing.

#![forbid(unsafe_code)]

/// Packs decisions: bit `i & 63` of `words[i >> 6]` is set exactly when
/// `values[i] < 0`, and the bits past `values.len()` are zero. Takes
/// O(`values.len()`) time.
///
/// # Panics
///
/// Panics unless `words.len() == values.len().div_ceil(64)`.
pub type PackSignsFn = fn(&[i8], &mut [u64]);

/// The portable [`PackSignsFn`].
///
/// # Panics
///
/// Panics unless `words.len() == values.len().div_ceil(64)`.
pub fn pack_signs_portable(values: &[i8], words: &mut [u64]) {
    check_word_count(values, words);
    pack_signs_scalar(values, words);
}

/// The AVX2 [`PackSignsFn`] when the host supports it; `None` on other
/// targets or hosts.
#[must_use]
pub fn detect() -> Option<PackSignsFn> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return crate::x86::ldpc_signs::detect();
    }
    #[allow(unreachable_code)]
    None
}

pub(crate) fn check_word_count(values: &[i8], words: &[u64]) {
    assert_eq!(
        words.len(),
        values.len().div_ceil(64),
        "one word per 64 decisions"
    );
}

pub(crate) fn pack_signs_scalar(values: &[i8], words: &mut [u64]) {
    for (word, chunk) in words.iter_mut().zip(values.chunks(64)) {
        *word = chunk.iter().enumerate().fold(0, |word, (bit, &value)| {
            word | (u64::from(value < 0) << bit)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packers() -> Vec<PackSignsFn> {
        let mut packers: Vec<PackSignsFn> = vec![pack_signs_portable];
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        assert_eq!(
            detect().is_some(),
            std::arch::is_x86_feature_detected!("avx2")
        );
        packers.extend(detect());
        packers
    }

    #[test]
    fn every_backend_packs_signs_by_the_definition() {
        let mut state = 0x5eed_u64;
        for pack in packers() {
            for len in [0usize, 1, 31, 32, 63, 64, 65, 127, 128, 129, 1000] {
                let values: Vec<i8> = (0..len)
                    .map(|_| {
                        state = state
                            .wrapping_mul(6_364_136_223_846_793_005)
                            .wrapping_add(1_442_695_040_888_963_407);
                        (state >> 56) as u8 as i8
                    })
                    .collect();
                let mut words = vec![u64::MAX; len.div_ceil(64)];
                pack(&values, &mut words);
                for (i, &value) in values.iter().enumerate() {
                    assert_eq!(
                        words[i >> 6] >> (i & 63) & 1 == 1,
                        value < 0,
                        "len {len} bit {i}"
                    );
                }
                if len % 64 != 0 {
                    assert_eq!(words[len >> 6] >> (len % 64), 0, "len {len} tail");
                }
            }
        }
    }

    #[test]
    fn a_sign_word_count_mismatch_panics_on_every_backend() {
        for pack in packers() {
            for (len, words) in [(65, 1), (64, 2), (0, 1)] {
                let result = std::panic::catch_unwind(|| pack(&vec![0; len], &mut vec![0; words]));
                assert!(result.is_err(), "{len} values, {words} words");
            }
        }
    }
}
