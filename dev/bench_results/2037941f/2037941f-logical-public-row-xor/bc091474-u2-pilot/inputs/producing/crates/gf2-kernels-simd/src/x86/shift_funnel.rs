//! BMI2-gated residual bit-shift funnel kernels.
//!
//! Both kernels answer the funnel contract [`crate::shift_funnel`] states and
//! are reached only through [`crate::shift_funnel::detect`], which publishes a
//! safe wrapper once `is_x86_feature_detected!("bmi2")` holds.
//!
//! The expression each word is written with is the shift-and-complement pair
//! the portable funnel already writes. Under this module's
//! `#[target_feature(enable = "bmi2")]` scope it lowers to the variable-count
//! shifts BMI2 adds, which is what the scope is here for: Rust 1.95 exposes no
//! double-precision shift intrinsic, so the instruction is reachable only as an
//! expression the backend folds. Which expression reaches which instruction at
//! that compiler is settled by the planning-time record
//! `dev/active/c04dd4ac-zen3-shifts-and-permutations/shift-feasibility-record.md`,
//! and `src/x86/asm/shift_funnel.asm.txt` is this module's own emitted loop in
//! both directions.
//!
//! The word pair is unrolled so the two funnel chains are independent.

/// Residual left funnel: writes `data[word_shift + 1 ..]` descending.
///
/// Word `i` becomes
/// `(data[i - word_shift] << bit_shift) | (data[i - word_shift - 1] >> (64 - bit_shift))`.
/// `data[word_shift]` has no lower neighbour and is left to the caller.
///
/// # Safety
///
/// The caller must ensure that
///
/// - the `bmi2` processor feature is available at run time, which the
///   `#[target_feature]` scope makes a precondition of the call;
/// - `word_shift < data.len()`, so the first written index exists;
/// - `bit_shift` lies in `1..64`, so the complement shift stays below 64.
///
/// [`crate::shift_funnel::detect`] publishes a wrapper that establishes the
/// feature, and that wrapper asserts the other two before calling. Under them
/// every address here stays inside `data`: the lowest read index is
/// `i - word_shift - 1` at `i == word_shift + 1`, which is zero, and the
/// highest written index is `data.len() - 1`.
#[target_feature(enable = "bmi2")]
pub(crate) unsafe fn shift_left_funnel_bmi2(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    let inv = 64 - bit_shift;
    let p = data.as_mut_ptr();
    let mut i = data.len();
    while i >= word_shift + 3 {
        let hi = i - 1;
        *p.add(hi) = (*p.add(hi - word_shift) << bit_shift) | (*p.add(hi - word_shift - 1) >> inv);
        let lo = i - 2;
        *p.add(lo) = (*p.add(lo - word_shift) << bit_shift) | (*p.add(lo - word_shift - 1) >> inv);
        i -= 2;
    }
    while i > word_shift + 1 {
        let hi = i - 1;
        *p.add(hi) = (*p.add(hi - word_shift) << bit_shift) | (*p.add(hi - word_shift - 1) >> inv);
        i = hi;
    }
}

/// Residual right funnel: writes `data[.. data.len() - word_shift - 1]`
/// ascending.
///
/// Word `i` becomes
/// `(data[i + word_shift] >> bit_shift) | (data[i + word_shift + 1] << (64 - bit_shift))`.
/// The topmost surviving word has no higher neighbour and is left to the
/// caller.
///
/// # Safety
///
/// Identical to [`shift_left_funnel_bmi2`]. Under those preconditions the
/// highest read index is `limit - 2 + word_shift + 2 == data.len() - 1` in the
/// unrolled body and `limit - 1 + word_shift + 1 == data.len() - 1` in the
/// tail, and the highest written index is `limit - 1`.
#[target_feature(enable = "bmi2")]
pub(crate) unsafe fn shift_right_funnel_bmi2(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    let inv = 64 - bit_shift;
    let p = data.as_mut_ptr();
    // One loop-invariant bound rather than a per-iteration comparison against
    // two moving indices.
    let limit = data.len() - word_shift - 1;
    let mut i = 0;
    while i + 2 <= limit {
        *p.add(i) = (*p.add(i + word_shift) >> bit_shift) | (*p.add(i + word_shift + 1) << inv);
        *p.add(i + 1) =
            (*p.add(i + word_shift + 1) >> bit_shift) | (*p.add(i + word_shift + 2) << inv);
        i += 2;
    }
    while i < limit {
        *p.add(i) = (*p.add(i + word_shift) >> bit_shift) | (*p.add(i + word_shift + 1) << inv);
        i += 1;
    }
}
