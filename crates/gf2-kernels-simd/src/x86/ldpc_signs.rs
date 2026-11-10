//! AVX2 sign packing: one `vpmovmskb` per 32 decisions.
#![deny(unsafe_op_in_unsafe_fn)]

use crate::ldpc_signs::{check_word_count, pack_signs_scalar, PackSignsFn};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

pub(crate) fn detect() -> Option<PackSignsFn> {
    std::arch::is_x86_feature_detected!("avx2").then_some(pack_signs_avx2 as PackSignsFn)
}

fn pack_signs_avx2(values: &[i8], words: &mut [u64]) {
    check_word_count(values, words);
    // SAFETY: detect publishes this entry only on AVX2 hosts.
    unsafe { pack_signs_avx2_unchecked(values, words) }
}

/// # Safety
/// The host must support AVX2.
#[target_feature(enable = "avx2")]
unsafe fn pack_signs_avx2_unchecked(values: &[i8], words: &mut [u64]) {
    let full = values.len() / 64;
    for (word, chunk) in words.iter_mut().zip(values.chunks_exact(64)) {
        // SAFETY: the chunk holds 64 values; unaligned loads are permitted.
        let (low, high) = unsafe {
            (
                _mm256_loadu_si256(chunk.as_ptr().cast()),
                _mm256_loadu_si256(chunk.as_ptr().add(32).cast()),
            )
        };
        let low = u64::from(_mm256_movemask_epi8(low) as u32);
        let high = u64::from(_mm256_movemask_epi8(high) as u32);
        *word = low | high << 32;
    }
    pack_signs_scalar(&values[full * 64..], &mut words[full..]);
}
