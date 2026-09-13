//! MSRV feasibility evidence for a vectorised byte-field successor
//! (jit:19513245).
//!
//! The prototype this assessment measures uses no intrinsic, so its own MSRV
//! feasibility is established by compiling it with the pinned toolchain. A
//! vectorised successor would not be: the byte-oriented kernel that ISA-L and
//! GF-Complete use is a split-table shuffle, which on this host's AVX2
//! instruction set means `_mm256_shuffle_epi8` over two 16-entry nibble
//! tables, plus the byte gather and scatter a `u64`-lane representation needs
//! around it. This file names every intrinsic such a kernel uses and is
//! compiled at the repository MSRV, so the feasibility claim is a compile
//! rather than an instruction list.
//!
//! It is compiled with `--crate-type lib` and never linked into a measured
//! executable: nothing here is timed, and the functions are deliberately
//! unexercised. `target_feature` correctness, not only availability, is what
//! the compile establishes, because each function carries the attribute a
//! shipped kernel would carry.
//!
//! Usage (the launcher runs this and records the result):
//!
//! ```text
//! rustc --edition 2021 -O --crate-type lib --emit metadata \
//!     dev/active/19513245/survey/msrv-intrinsics.rs -o <temporary>
//! ```

#![allow(dead_code)]
#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::{
    __m256i, _mm256_and_si256, _mm256_cvtepu8_epi64, _mm256_extracti128_si256,
    _mm256_loadu_si256, _mm256_set1_epi8, _mm256_setr_epi8, _mm256_shuffle_epi8,
    _mm256_srli_epi64, _mm256_storeu_si256, _mm256_xor_si256, _mm_storel_epi64,
};

/// Whether this host has the instruction set a vectorised successor needs,
/// observed at run time exactly as gf2-core's kernel dispatch observes it.
pub fn avx2_available() -> bool {
    std::is_x86_feature_detected!("avx2")
}

/// The split-table byte multiply-accumulate: `y[i] ^= a * x[i]` over 32
/// bytes, with the coefficient's products split into a low-nibble and a
/// high-nibble table of sixteen bytes each, broadcast into both 128-bit
/// lanes.
///
/// # Safety
///
/// The caller must ensure AVX2 is available (see [`avx2_available`]) and that
/// `x` and `y` each address at least 32 readable, respectively writable,
/// bytes. The pointers need no alignment: the loads and stores are the
/// unaligned forms.
#[target_feature(enable = "avx2")]
pub unsafe fn split_table_axpy32(y: *mut u8, x: *const u8, low: &[u8; 16], high: &[u8; 16]) {
    let low_table = broadcast16(low);
    let high_table = broadcast16(high);
    let mask = _mm256_set1_epi8(0x0F);
    let source = _mm256_loadu_si256(x.cast());
    let low_nibbles = _mm256_and_si256(source, mask);
    let high_nibbles = _mm256_and_si256(_mm256_srli_epi64(source, 4), mask);
    let product = _mm256_xor_si256(
        _mm256_shuffle_epi8(low_table, low_nibbles),
        _mm256_shuffle_epi8(high_table, high_nibbles),
    );
    let target = _mm256_loadu_si256(y.cast());
    _mm256_storeu_si256(y.cast(), _mm256_xor_si256(target, product));
}

/// Broadcasts a sixteen-byte table into both 128-bit lanes, which is what
/// `_mm256_shuffle_epi8`'s per-lane indexing requires.
///
/// # Safety
///
/// The caller must ensure AVX2 is available.
#[target_feature(enable = "avx2")]
unsafe fn broadcast16(table: &[u8; 16]) -> __m256i {
    _mm256_setr_epi8(
        table[0] as i8, table[1] as i8, table[2] as i8, table[3] as i8,
        table[4] as i8, table[5] as i8, table[6] as i8, table[7] as i8,
        table[8] as i8, table[9] as i8, table[10] as i8, table[11] as i8,
        table[12] as i8, table[13] as i8, table[14] as i8, table[15] as i8,
        table[0] as i8, table[1] as i8, table[2] as i8, table[3] as i8,
        table[4] as i8, table[5] as i8, table[6] as i8, table[7] as i8,
        table[8] as i8, table[9] as i8, table[10] as i8, table[11] as i8,
        table[12] as i8, table[13] as i8, table[14] as i8, table[15] as i8,
    )
}

/// The scatter a `u64`-lane element representation needs after a byte
/// kernel: four canonical bytes widened into four `u64` lanes.
///
/// # Safety
///
/// The caller must ensure AVX2 is available, that `bytes` addresses at least
/// four readable bytes and that `lanes` addresses at least four writable
/// `u64` values.
#[target_feature(enable = "avx2")]
pub unsafe fn scatter_bytes_to_lanes(lanes: *mut u64, bytes: *const u8) {
    let widened = _mm256_cvtepu8_epi64(std::arch::x86_64::_mm_loadu_si32(bytes.cast()));
    _mm256_storeu_si256(lanes.cast(), widened);
}

/// The gather in the other direction: the low bytes of four `u64` lanes
/// collected into four consecutive bytes.
///
/// # Safety
///
/// The caller must ensure AVX2 is available, that `lanes` addresses at least
/// four readable `u64` values and that `bytes` addresses at least eight
/// writable bytes.
#[target_feature(enable = "avx2")]
pub unsafe fn gather_lanes_to_bytes(bytes: *mut u8, lanes: *const u64) {
    let loaded = _mm256_loadu_si256(lanes.cast());
    let selector = _mm256_setr_epi8(
        0, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 8, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1,
    );
    let shuffled = _mm256_shuffle_epi8(loaded, selector);
    let low = _mm256_extracti128_si256(shuffled, 0);
    _mm_storel_epi64(bytes.cast(), low);
}
