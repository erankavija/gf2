//! Residual bit-shift funnel kernels.
//!
//! The funnel a bit shift by a non-multiple of 64 needs. The bundle is
//! detected on its own processor feature, so a host that has that feature
//! without AVX2 still reaches the kernels.
//!
//! # The funnel contract
//!
//! A residual shift of a little-endian `u64` buffer by `64 * word_shift +
//! bit_shift` bits rewrites each word from a pair of source words. Both kernels
//! take `(data, word_shift, bit_shift)` with `word_shift < data.len()` and
//! `bit_shift` in `1..64`, and write, in the order that makes the rewrite
//! correct in place:
//!
//! - [`ShiftFunnelFns::shift_left_funnel`] writes `data[word_shift + 1 ..]`,
//!   word `i` becoming
//!   `(data[i - word_shift] << bit_shift) | (data[i - word_shift - 1] >> (64 - bit_shift))`;
//! - [`ShiftFunnelFns::shift_right_funnel`] writes
//!   `data[.. data.len() - word_shift - 1]`, word `i` becoming
//!   `(data[i + word_shift] >> bit_shift) | (data[i + word_shift + 1] << (64 - bit_shift))`.
//!
//! Neither kernel touches the one word at the far end of its range that has no
//! neighbour to funnel in, and neither zeroes the words the shift vacates: the
//! caller owns both, as `gf2_core::BitVec`'s residual branch does. The buffer
//! carries no alignment requirement. A call outside either argument range
//! panics: the ranges are the kernels' safety conditions on their indices, so
//! the published wrappers check them rather than trust them.

/// Safe residual-funnel function pointer, taking `(data, word_shift, bit_shift)`.
pub type ShiftFunnelFn = fn(&mut [u64], usize, u32);

/// Bundle of dispatched residual-funnel kernels, in both directions.
#[derive(Copy, Clone)]
pub struct ShiftFunnelFns {
    /// Left funnel over `data[word_shift + 1 ..]`.
    pub shift_left_funnel: ShiftFunnelFn,
    /// Right funnel over `data[.. data.len() - word_shift - 1]`.
    pub shift_right_funnel: ShiftFunnelFn,
}

/// Detect and return the residual-funnel kernels, or `None` where this host or
/// target cannot run them.
///
/// The kernels need the `bmi2` processor feature, which this function tests at
/// run time; it is the whole safety condition of the `unsafe` kernels behind the
/// published wrappers. A caller that gets `None` runs its own portable funnel:
/// this bundle has no scalar lane, because the fallback is the loop the caller
/// already has.
///
/// # Examples
///
/// ```
/// if let Some(fns) = gf2_kernels_simd::shift_funnel::detect() {
///     // A two-word buffer shifted left by one bit, with word 0 left to the
///     // caller: word 1 takes the top bit of word 0 in its lowest position.
///     let mut data = [1u64 << 63, 0];
///     (fns.shift_left_funnel)(&mut data, 0, 1);
///     assert_eq!(data[1], 1);
/// }
/// ```
#[must_use]
pub fn detect() -> Option<ShiftFunnelFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        use std::arch::is_x86_feature_detected;
        if is_x86_feature_detected!("bmi2") {
            return Some(ShiftFunnelFns {
                shift_left_funnel: shift_left_funnel_bmi2_safe,
                shift_right_funnel: shift_right_funnel_bmi2_safe,
            });
        }
    }
    None
}

/// # Panics
///
/// Panics when `word_shift` is not below `data.len()` or `bit_shift` is outside
/// `1..64`, which are the kernel's preconditions on its indices.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn shift_left_funnel_bmi2_safe(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    assert_funnel_arguments(data.len(), word_shift, bit_shift);
    // SAFETY: `detect` publishes this pointer only when
    // `is_x86_feature_detected!("bmi2")` holds, and the assertion above
    // establishes the kernel's other two preconditions.
    unsafe { crate::x86::shift_funnel::shift_left_funnel_bmi2(data, word_shift, bit_shift) }
}

/// # Panics
///
/// As [`shift_left_funnel_bmi2_safe`].
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn shift_right_funnel_bmi2_safe(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    assert_funnel_arguments(data.len(), word_shift, bit_shift);
    // SAFETY: as above; `detect` established the `bmi2` feature.
    unsafe { crate::x86::shift_funnel::shift_right_funnel_bmi2(data, word_shift, bit_shift) }
}

/// The index preconditions every wrapper turns from a safety condition into a
/// panic.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
fn assert_funnel_arguments(words: usize, word_shift: usize, bit_shift: u32) {
    assert!(
        word_shift < words,
        "residual funnel: word_shift {word_shift} outside a {words}-word buffer"
    );
    assert!(
        (1..64).contains(&bit_shift),
        "residual funnel: bit_shift {bit_shift} outside 1..64"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The left contract as written, indexed word by word.
    fn left_reference(data: &mut [u64], word_shift: usize, bit_shift: u32) {
        let inv = 64 - bit_shift;
        for i in (word_shift + 1..data.len()).rev() {
            data[i] = (data[i - word_shift] << bit_shift) | (data[i - word_shift - 1] >> inv);
        }
    }

    /// The right contract as written, indexed word by word.
    fn right_reference(data: &mut [u64], word_shift: usize, bit_shift: u32) {
        let inv = 64 - bit_shift;
        for i in 0..data.len() - word_shift - 1 {
            data[i] = (data[i + word_shift] >> bit_shift) | (data[i + word_shift + 1] << inv);
        }
    }

    fn buffer(words: usize, seed: u64) -> Vec<u64> {
        let mut state = seed;
        (0..words)
            .map(|_| {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            })
            .collect()
    }

    #[test]
    fn the_detected_kernels_answer_the_funnel_contract() {
        let Some(fns) = detect() else {
            return;
        };
        let directions: [(&str, ShiftFunnelFn, ShiftFunnelFn); 2] = [
            ("left", fns.shift_left_funnel, left_reference),
            ("right", fns.shift_right_funnel, right_reference),
        ];
        for words in [1usize, 2, 3, 4, 5, 8, 9, 16, 17, 33, 64] {
            for word_shift in 0..words {
                for bit_shift in [1u32, 2, 7, 31, 32, 63] {
                    let source = buffer(words, 0xf8dd_4dde ^ words as u64);
                    for (direction, kernel, reference) in directions {
                        let mut expected = source.clone();
                        reference(&mut expected, word_shift, bit_shift);
                        let mut actual = source.clone();
                        kernel(&mut actual, word_shift, bit_shift);
                        assert_eq!(
                            actual, expected,
                            "{direction} funnel, words {words}, \
                             word_shift {word_shift}, bit_shift {bit_shift}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "word_shift")]
    fn a_word_shift_outside_the_buffer_panics() {
        let Some(fns) = detect() else {
            // No kernel on this host: reproduce the panic the wrapper would
            // raise, so the case stays a statement about the contract.
            panic!("residual funnel: word_shift outside the buffer");
        };
        (fns.shift_left_funnel)(&mut [0u64; 2], 2, 1);
    }

    #[test]
    #[should_panic(expected = "bit_shift")]
    fn a_bit_shift_outside_the_residual_range_panics() {
        let Some(fns) = detect() else {
            panic!("residual funnel: bit_shift outside 1..64");
        };
        (fns.shift_right_funnel)(&mut [0u64; 2], 0, 64);
    }
}
