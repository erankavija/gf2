//! Nominated form 2: the BMI2-gated scalar funnel over an unrolled word pair.
//!
//! Rust 1.95 exposes no `_shld_u64` / `_shrd_u64` intrinsic (`core::arch`
//! ships only `_bzhi_u64`, `_pdep_u64`, `_pext_u64` and `_mulx_u64` under
//! BMI2), so the double-precision shift can only be asked for as an
//! expression the backend may fold. Both expressions are kept: `_pair` is the
//! shift-and-complement pair the production loop writes, `_dp` is the 128-bit
//! funnel that names the double-precision shift directly. The emitted
//! assembly for each is the feasibility evidence.

/// Residual left funnel, shift-and-complement pair, unrolled by two.
///
/// # Safety
///
/// BMI2 must be available at run time; the `target_feature` attribute makes
/// that a precondition of the call. `ws < data.len()` and `b` in `1..64` are
/// required. Every address is derived from `data.as_mut_ptr()`; the guards
/// keep the lowest read index `j - ws - 1` at or above zero and the highest
/// write index below `data.len()`.
#[inline(never)]
#[no_mangle]
#[target_feature(enable = "bmi2")]
pub unsafe fn shift_left_funnel_bmi2_pair(data: &mut [u64], ws: usize, b: u32) {
    let inv = 64 - b;
    let p = data.as_mut_ptr();
    let mut i = data.len();
    while i >= ws + 3 {
        let j = i - 1;
        *p.add(j) = (*p.add(j - ws) << b) | (*p.add(j - ws - 1) >> inv);
        let j2 = i - 2;
        *p.add(j2) = (*p.add(j2 - ws) << b) | (*p.add(j2 - ws - 1) >> inv);
        i -= 2;
    }
    while i > ws + 1 {
        let j = i - 1;
        *p.add(j) = (*p.add(j - ws) << b) | (*p.add(j - ws - 1) >> inv);
        i = j;
    }
}

/// Residual left funnel, 128-bit double-precision expression, unrolled by two.
///
/// # Safety
///
/// Identical to [`shift_left_funnel_bmi2_pair`].
#[inline(never)]
#[no_mangle]
#[target_feature(enable = "bmi2")]
pub unsafe fn shift_left_funnel_bmi2_dp(data: &mut [u64], ws: usize, b: u32) {
    let p = data.as_mut_ptr();
    let mut i = data.len();
    while i >= ws + 3 {
        let j = i - 1;
        *p.add(j) = funnel_high(*p.add(j - ws), *p.add(j - ws - 1), b);
        let j2 = i - 2;
        *p.add(j2) = funnel_high(*p.add(j2 - ws), *p.add(j2 - ws - 1), b);
        i -= 2;
    }
    while i > ws + 1 {
        let j = i - 1;
        *p.add(j) = funnel_high(*p.add(j - ws), *p.add(j - ws - 1), b);
        i = j;
    }
}

/// Residual right funnel, shift-and-complement pair, unrolled by two.
///
/// # Safety
///
/// Identical to [`shift_left_funnel_bmi2_pair`], with the read window
/// `i + ws ..= i + ws + 1` bounded above by `data.len() - 1`.
#[inline(never)]
#[no_mangle]
#[target_feature(enable = "bmi2")]
pub unsafe fn shift_right_funnel_bmi2_pair(data: &mut [u64], ws: usize, b: u32) {
    let inv = 64 - b;
    let p = data.as_mut_ptr();
    let limit = data.len() - ws - 1;
    let mut i = 0;
    while i + 2 <= limit {
        *p.add(i) = (*p.add(i + ws) >> b) | (*p.add(i + ws + 1) << inv);
        *p.add(i + 1) = (*p.add(i + ws + 1) >> b) | (*p.add(i + ws + 2) << inv);
        i += 2;
    }
    while i < limit {
        *p.add(i) = (*p.add(i + ws) >> b) | (*p.add(i + ws + 1) << inv);
        i += 1;
    }
}

/// Residual right funnel, 128-bit double-precision expression, unrolled by two.
///
/// # Safety
///
/// Identical to [`shift_right_funnel_bmi2_pair`].
#[inline(never)]
#[no_mangle]
#[target_feature(enable = "bmi2")]
pub unsafe fn shift_right_funnel_bmi2_dp(data: &mut [u64], ws: usize, b: u32) {
    let p = data.as_mut_ptr();
    let limit = data.len() - ws - 1;
    let mut i = 0;
    while i + 2 <= limit {
        *p.add(i) = funnel_low(*p.add(i + ws + 1), *p.add(i + ws), b);
        *p.add(i + 1) = funnel_low(*p.add(i + ws + 2), *p.add(i + ws + 1), b);
        i += 2;
    }
    while i < limit {
        *p.add(i) = funnel_low(*p.add(i + ws + 1), *p.add(i + ws), b);
        i += 1;
    }
}

/// High half of `(hi:lo) << b`, the left funnel's word.
///
/// The `& 63` is load-bearing, not defensive: a `u128` shift is defined for
/// every count below 128 while `shld` is not, so without it the backend
/// guards each `shld` with a `test`/`cmov` pair against a count of 64 or more.
#[inline(always)]
fn funnel_high(hi: u64, lo: u64, b: u32) -> u64 {
    ((((hi as u128) << 64 | lo as u128) << (b & 63)) >> 64) as u64
}

/// Low half of `(hi:lo) >> b`, the right funnel's word. The `& 63` carries the
/// same weight as in [`funnel_high`].
#[inline(always)]
fn funnel_low(hi: u64, lo: u64, b: u32) -> u64 {
    (((hi as u128) << 64 | lo as u128) >> (b & 63)) as u64
}
