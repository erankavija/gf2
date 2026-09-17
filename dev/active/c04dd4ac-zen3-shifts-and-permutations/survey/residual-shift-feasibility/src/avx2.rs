//! Nominated form 1: the AVX2 lane-crossing funnel.
//!
//! Each 256-bit group needs the neighbouring-word vector, one u64 away from
//! the group it shifts. `vperm2i128` moves the crossing half into place and
//! `vpalignr` — which aligns inside each 128-bit lane only — finishes the
//! byte alignment, so the pair together perform the crossing a single
//! `vpalignr` cannot.

use core::arch::x86_64::*;

/// Residual left funnel over the write window `ws + 1 ..= data.len() - 1`.
///
/// # Safety
///
/// AVX2 must be available at run time; the `target_feature` attribute makes
/// that a precondition of the call rather than a property of the build.
/// `ws < data.len()` and `b` in `1..64` are required. Every load and store
/// address is derived from `data.as_mut_ptr()` and bounded by the loop
/// conditions to `data`'s own allocation: a group at write base `w` touches
/// `[w, w + 4)` for the store and `[w - ws - 4, w - ws + 4)` for the two
/// loads, and the guard `i >= ws + 8` keeps the lower load index at or above
/// zero while `w + 4 <= data.len()` keeps the store in range.
#[inline(never)]
#[no_mangle]
#[target_feature(enable = "avx2")]
pub unsafe fn shift_left_funnel_avx2(data: &mut [u64], ws: usize, b: u32) {
    let n = data.len();
    let inv = 64 - b;
    let cb = _mm_cvtsi32_si128(b as i32);
    let cinv = _mm_cvtsi32_si128(inv as i32);
    let p = data.as_mut_ptr();

    // Descending groups. A group reads strictly below its own write base, so
    // the in-place overlap is safe in this direction for every `ws`.
    let mut i = n;
    while i >= ws + 8 {
        let w = i - 4;
        let s = w - ws;
        let cur = _mm256_loadu_si256(p.add(s).cast());
        let prev = _mm256_loadu_si256(p.add(s - 4).cast());
        // [src[s-1], src[s], src[s+1], src[s+2]]
        let t = _mm256_permute2x128_si256(prev, cur, 0x21);
        let nbr = _mm256_alignr_epi8(cur, t, 8);
        let out = _mm256_or_si256(_mm256_sll_epi64(cur, cb), _mm256_srl_epi64(nbr, cinv));
        _mm256_storeu_si256(p.add(w).cast(), out);
        i = w;
    }
    while i > ws + 1 {
        let j = i - 1;
        *p.add(j) = (*p.add(j - ws) << b) | (*p.add(j - ws - 1) >> inv);
        i = j;
    }
}

/// Residual right funnel over the write window `0 .. data.len() - ws - 1`.
///
/// # Safety
///
/// Same contract as [`shift_left_funnel_avx2`]. A group at write base `i`
/// stores `[i, i + 4)` and loads `[i + ws, i + ws + 8)`; the guard
/// `i <= vec_end` keeps both inside `data`, the load being the binding one.
#[inline(never)]
#[no_mangle]
#[target_feature(enable = "avx2")]
pub unsafe fn shift_right_funnel_avx2(data: &mut [u64], ws: usize, b: u32) {
    let n = data.len();
    let inv = 64 - b;
    let cb = _mm_cvtsi32_si128(b as i32);
    let cinv = _mm_cvtsi32_si128(inv as i32);
    let p = data.as_mut_ptr();
    let limit = n - ws - 1;
    // Highest write base whose group still loads inside `data`. It also
    // bounds the store, so one loop-invariant guard covers both and the
    // backend keeps no bound test inside the loop.
    let vec_end = n.saturating_sub(ws + 8);

    // Ascending groups. Loads start at or above the write base and precede
    // the store, so no group reads a word an earlier group rewrote.
    let mut i = 0;
    while n >= ws + 8 && i <= vec_end {
        let s = i + ws;
        let cur = _mm256_loadu_si256(p.add(s).cast());
        let next = _mm256_loadu_si256(p.add(s + 4).cast());
        // [src[s+1], src[s+2], src[s+3], src[s+4]]
        let t = _mm256_permute2x128_si256(cur, next, 0x21);
        let nbr = _mm256_alignr_epi8(t, cur, 8);
        let out = _mm256_or_si256(_mm256_srl_epi64(cur, cb), _mm256_sll_epi64(nbr, cinv));
        _mm256_storeu_si256(p.add(i).cast(), out);
        i += 4;
    }
    while i < limit {
        *p.add(i) = (*p.add(i + ws) >> b) | (*p.add(i + ws + 1) << inv);
        i += 1;
    }
}
