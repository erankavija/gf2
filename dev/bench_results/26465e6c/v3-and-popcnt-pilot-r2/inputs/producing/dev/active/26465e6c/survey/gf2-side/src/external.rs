//! Bindings to the vendored external kernels compiled by `build.rs`.

use std::os::raw::c_void;

unsafe extern "C" {
    fn survey_libpopcnt_words(words: *const u64, count: usize) -> u64;
    fn survey_libpopcnt_bytes(data: *const c_void, bytes: usize) -> u64;
    fn survey_libpopcnt_capabilities(out: *mut [i32; 4]);
    fn survey_mula_avx2_harley_seal_words(words: *const u64, count: usize) -> u64;
    fn survey_mula_avx2_harley_seal_bytes(data: *const u8, bytes: usize) -> u64;
}

/// Exact external compile commands and compiler versions of this build.
pub const BUILD_RECORD: &str = include_str!(concat!(env!("OUT_DIR"), "/external-build.json"));

/// libpopcnt `popcnt()` over whole words.
pub fn libpopcnt_words(words: &[u64]) -> u64 {
    // SAFETY: the pointer and length describe one live slice; libpopcnt reads
    // exactly `len * 8` bytes with unaligned loads.
    unsafe { survey_libpopcnt_words(words.as_ptr(), words.len()) }
}

/// libpopcnt `popcnt()` over a byte length, tail bytes included.
pub fn libpopcnt_bytes(bytes: &[u8]) -> u64 {
    // SAFETY: as above, over `bytes.len()` bytes.
    unsafe { survey_libpopcnt_bytes(bytes.as_ptr().cast(), bytes.len()) }
}

/// The capability word libpopcnt's dispatch derives from CPUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibpopcntCapabilities {
    /// `get_cpuid()` as libpopcnt computes it.
    pub flags: i32,
    /// Scalar `POPCNT` usable.
    pub popcnt: bool,
    /// AVX2 usable, including OS YMM state.
    pub avx2: bool,
    /// The AVX-512 VPOPCNTDQ path usable.
    pub avx512_vpopcntdq: bool,
}

/// Reads libpopcnt's own capability word.
pub fn libpopcnt_capabilities() -> LibpopcntCapabilities {
    let mut out = [0_i32; 4];
    // SAFETY: the callee writes exactly four `int32_t` values.
    unsafe { survey_libpopcnt_capabilities(&mut out) };
    LibpopcntCapabilities {
        flags: out[0],
        popcnt: out[0] & out[1] != 0,
        avx2: out[0] & out[2] != 0,
        avx512_vpopcntdq: out[0] & out[3] != 0,
    }
}

/// Mula's `popcnt_AVX2_harley_seal` over whole words.
///
/// # Safety
///
/// `words` must start on a 32-byte boundary and the host must support AVX2
/// and POPCNT: the reference loads through `const __m256i*`.
pub unsafe fn mula_words(words: &[u64]) -> u64 {
    // SAFETY: the caller upholds alignment and ISA support.
    unsafe { survey_mula_avx2_harley_seal_words(words.as_ptr(), words.len()) }
}

/// Mula's `popcnt_AVX2_harley_seal` over a byte length, tail bytes included.
///
/// # Safety
///
/// As [`mula_words`].
pub unsafe fn mula_bytes(bytes: &[u8]) -> u64 {
    // SAFETY: the caller upholds alignment and ISA support.
    unsafe { survey_mula_avx2_harley_seal_bytes(bytes.as_ptr(), bytes.len()) }
}
