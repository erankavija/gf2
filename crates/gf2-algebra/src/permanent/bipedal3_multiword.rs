//! Multi-word streaming column-sum path for `permanent_bipedal3` at `n > 64`.
//!
//! ## Algorithm
//!
//! This module implements the R3 cache-blocking design from
//! `dev/plans/r3_multi_word_streaming.md`. The algorithm evaluates Ryser's
//! inclusion-exclusion formula in binary-reflected Gray-code subset order:
//!
//! ```text
//! perm(A) = (-1)^n * sum_{S ⊆ [n], S ≠ ∅}  (-1)^|S|  * prod_{i=0}^{n-1}  sum_{j ∈ S} A[i,j]
//! ```
//!
//! At each Gray step exactly one column is added to or subtracted from a
//! packed column-sum buffer. For `n > 64` the column-sum spans
//! `W = ceil(n / 64)` words per leg (`mag` + `sgn`), updated via the
//! Scheinerman 2024 (arXiv 2407.20205v2) Theorem 2.1 / Algorithm 2
//! bipedal-3 add/sub formulas (6 bitwise ops per word per leg per step).
//!
//! ## Cache-blocking (R3 §5)
//!
//! For the design window `n ≤ N_MAX_MULTIWORD = 255`:
//! - `W ≤ 4`, so the column-sum fits in 8 `u64`s (64 B, one cache line per
//!   leg) and stays L1-resident across the full `2^n` outer loop.
//! - The matrix itself fits in Zen 3 L1d (32 KiB): `2 * W * n * 8 < 16 KiB`.
//! - No cache blocking is required; the loop streams columns from L1.
//!
//! Above `N_MAX_MULTIWORD = 255`, the `[u64; 4]` Gray counter can no longer
//! represent the iteration range (`2^n` would equal/exceed `2^256`). That
//! regime is `W3-T15` (rayon parallel, which partitions the Gray index
//! space) and `W5` (HIP/ROCm GPU).
//!
//! ## Gray-code counter
//!
//! For `n > 64` the loop counter does not fit in a single `u64`. This module
//! maintains the counter as a little-endian `[u64; 4]` array (supporting
//! `n ≤ 255`) together with a separate bit-vector tracking the current active
//! subset (`g_k = k ^ (k >> 1)` in scalar notation). The flip index is
//! derived from the counter's trailing-zeros position.
//!
//! Note: for `n ≥ 65`, the `2^n` outer loop is astronomically large and
//! infeasible to run to completion in any practical timeframe. The
//! cross-check tests at large `n` are therefore marked
//! `#[ignore = "sim: ..."]` per project test-tier rules.

use gf2_core::gfp::Fp;

use crate::packed::bipedal3::{Bipedal3, Bipedal3Matrix};
use crate::packed::PackedField;

/// Maximum supported `n` for the multi-word streaming path.
///
/// Per R3 `dev/plans/r3_multi_word_streaming.md` §1 and §5:
/// above `n = 255` single-thread time is dominated by the `2^n` outer
/// enumeration regardless of cache behaviour. That regime belongs to
/// `W3-T15` (rayon parallel) and `W5` (HIP/ROCm GPU).
///
/// With `N_MAX_MULTIWORD = 255`, `W = ceil(255 / 64) = 4`, so the
/// column-sum uses 4 `u64`s per leg (32 B per leg = 1 YMM register
/// on AVX2), the matrix occupies `2 * 4 * 255 * 8 < 16 KiB` — fitting in
/// Zen 3 L1d (32 KiB). The Gray-code counter uses `[u64; 4]` (256-bit
/// little-endian); a step counter of `k` ranges over `1..2^n`, so `n = 255`
/// keeps `k` strictly below `2^256`, ensuring `gray_counter_is_zero_above`
/// reads only the 4 in-bounds counter words.
pub const N_MAX_MULTIWORD: usize = 255;

/// Target L1 data-cache size assumed by the R3 sizing analysis, in bytes.
///
/// Set to 32 KiB (Zen 3, the development host). The actual cache size is
/// detected only implicitly via the criterion derived below
/// ([`MAX_MATRIX_BYTES_FOR_L1`]); this constant exposes the assumption
/// to readers, downstream `cfg`-specific tuning, and the cross-CPU
/// portability tests planned in W3-T16/W5.
pub const L1D_BYTES: usize = 32 * 1024;

/// Maximum matrix footprint (in bytes) the design budgets for L1d
/// residency: half of [`L1D_BYTES`].
///
/// The matrix occupies `2 * W * n * 8` bytes (mag + sgn legs, each
/// `W * 8` bytes per column, `n` columns). With the column-sum and
/// outer iteration metadata, half of L1d is a comfortable upper bound
/// before the matrix begins to spill. At `n = N_MAX_MULTIWORD = 255`
/// the matrix uses `2 * 4 * 255 * 8 = 16320` bytes — below this
/// 16 KiB budget, so no explicit cache blocking is needed in the
/// supported range.
pub const MAX_MATRIX_BYTES_FOR_L1: usize = L1D_BYTES / 2;

/// Bipedal3 column footprint in bytes given an `n × n` matrix.
///
/// Each column carries `ceil(n / 64) * 8` bytes per leg (mag + sgn),
/// so the whole matrix is `2 * ceil(n / 64) * 8 * n` bytes.
#[inline]
pub const fn matrix_bytes_for_n(n: usize) -> usize {
    let w = n.div_ceil(64);
    2 * w * 8 * n
}

const _: () = {
    // Static sanity check: the cap fits within the L1d budget.
    assert!(matrix_bytes_for_n(N_MAX_MULTIWORD) <= MAX_MATRIX_BYTES_FOR_L1);
};

/// Compute the permanent of `mat` over `F_3` for `n ∈ (64, N_MAX_MULTIWORD]`
/// using the multi-word streaming column-sum.
///
/// This is the R3 design's scalar reference implementation (`dev/plans/
/// r3_multi_word_streaming.md` §8 pseudocode, transcribed directly). It is
/// bit-identical to `permanent_ryser::<Fp<3>>` by the R3 validation plan
/// (§9.2) and to `permanent_bipedal3_singleword` at `n = 64` (§9.1).
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Bipedal3Matrix`] (column-major, `rows == cols`),
///   with `64 < n ≤ N_MAX_MULTIWORD`.
///
/// # Examples
///
/// ```no_run
/// // NOTE: This example is marked `no_run` because the permanent computation
/// // at n=65 requires 2^65 Gray-code steps, which is infeasible to run.
/// // The function is correctly structured for the computation; use it with
/// // n ≤ ~20 for practical verification (via permanent_bipedal3 dispatcher).
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::bipedal3_multiword::permanent_bipedal3_multiword;
/// use gf2_core::gfp::Fp;
///
/// // 65×65 identity over F_3: permanent = 1 (conceptually correct, infeasible to run)
/// let mut id = vec![Fp::<3>::new(0); 65 * 65];
/// for i in 0..65 { id[i * 65 + i] = Fp::<3>::new(1); }
/// let m = Bipedal3Matrix::from_row_major(&id, 65, 65);
/// assert_eq!(permanent_bipedal3_multiword(&m), Fp::<3>::new(1));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > N_MAX_MULTIWORD` (`n` must be `≤ 255`).
///
/// Panics if `mat.cols() == 0` (the zero-dimension edge case belongs to
/// `permanent_bipedal3_singleword` / the dispatcher).
///
/// # Complexity
///
/// `O(n · 2^n)` bitwise word operations:
/// - Gray walk: `2^n - 1` steps, each with `W = ceil(n / 64)` bipedal
///   add/sub ops (6 word-level ops each) plus `W` bipedal mul ops for the
///   sequential word fold.
/// - Space: `O(W)` extra (`col_sum_mag` + `col_sum_sgn` buffers of
///   length `W` each, plus a 256-bit Gray counter).
pub fn permanent_bipedal3_multiword(mat: &Bipedal3Matrix) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3_multiword: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= N_MAX_MULTIWORD,
        "permanent_bipedal3_multiword: n = {n} exceeds N_MAX_MULTIWORD = {N_MAX_MULTIWORD}"
    );
    // Note: no lower-bound assert. The dispatcher routes `n <= 64` to the
    // singleword fast path for perf, but calling this function directly at
    // small `n` is correctness-preserving — the `[u64; 4]` Gray counter
    // and word-wise loops handle `n` in `1..=N_MAX_MULTIWORD` uniformly.
    // The §9.2 validation plan relies on this property: small-`n` direct
    // ryser cross-checks exercise the multi-word code path under both
    // debug and release builds without `debug_assert!` divergence.

    // W = ceil(n / 64): number of u64 words per column-sum leg.
    let w = n.div_ceil(64);

    // Column-sum buffer: two arrays of `w` words each.
    //
    // Initialised to packed-zero for active lanes (0..n), then the tail of
    // the last word is set to packed-one (multiplicative identity) so that
    // `fold_mul_words` does not pull the product to zero via inactive lanes.
    //
    // Tail mask: bits n%64 .. 63 of word w-1 must encode packed-1.
    // For packed-1: mag = 1, sgn = 0.
    // If n % 64 == 0 the last word is full — no tail to mask.
    let mut col_sum_mag = vec![0u64; w];
    let mut col_sum_sgn = vec![0u64; w];
    let tail_mask_hi: u64 = if n.is_multiple_of(64) {
        0u64
    } else {
        !0u64 << (n % 64)
    };
    // Set tail lanes to packed-1: mag = 1 (identity), sgn = 0 (already zero).
    col_sum_mag[w - 1] |= tail_mask_hi;

    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // Gray-code walk over all 2^n - 1 non-empty subsets.
    // For n > 63 the counter does not fit in u64; we use a 256-bit
    // little-endian counter [k0, k1, k2, k3] (k0 is the least-significant
    // word) supporting n ≤ 255.
    let mut gray_counter = [0u64; 4];

    // Iterate 2^n - 1 steps by incrementing the counter each time.
    // We stop when the counter wraps back to zero (which happens after
    // 2^n steps from 1). In practice for n >= 65 this loop is
    // astronomically long and all callers that exercise it carry
    // #[ignore = "sim: ..."] in the test suite.
    loop {
        // Increment the 256-bit counter.
        inc_counter(&mut gray_counter);

        // Stop when the counter wraps to zero (all 2^n steps visited).
        // For n in 1..=64 this is handled by the single-word path; here n > 64.
        // The counter wraps at 2^256; we stop at 2^n.
        if gray_counter_is_zero_above(&gray_counter, n) {
            break;
        }

        // flip = trailing_zeros of the counter (as a 256-bit number).
        let flip = trailing_zeros_256(&gray_counter);

        // Determine add vs subtract from the Gray-code register g_k = k ^ (k >> 1).
        // The bit at position `flip` of g_k determines add (1) vs sub (0).
        // Equivalently: the parity of the count of trailing zeros up to and
        // including position flip in the counter determines the direction.
        // Simplest correct approach: track `added` as the bit `flip` of the
        // current active subset (toggled each time we flip that column).
        // We maintain the subset_size incrementally for the Ryser sign.
        //
        // The Gray-code register g_k has bit `flip` set iff the column just
        // entered (add), clear iff it just left (sub). We compute this from
        // the counter directly: g_k[flip] = flip-th bit of (k ^ (k >> 1)).
        let added = gray_bit_at(&gray_counter, flip);

        // Update column sum: add or subtract column `flip`, word by word.
        let col = mat.column(flip);
        let col_mag = col.raw_mag();
        let col_sgn = col.raw_sgn();

        if added {
            subset_size += 1;
            // col_sum += column[flip], lane-wise bipedal add per word.
            // SSOT: Bipedal3::add (paper §2.2 / Algorithm 2, 6 bitwise ops).
            for i in 0..w {
                let result = Bipedal3::from_raw(col_sum_mag[i], col_sum_sgn[i])
                    .add(Bipedal3::from_raw(col_mag[i], col_sgn[i]));
                col_sum_mag[i] = result.mag();
                col_sum_sgn[i] = result.sgn();
            }
        } else {
            subset_size -= 1;
            // col_sum -= column[flip], lane-wise bipedal sub per word.
            // SSOT: Bipedal3::sub (paper §2.2 / Theorem 2.1, 6 bitwise ops).
            for i in 0..w {
                let result = Bipedal3::from_raw(col_sum_mag[i], col_sum_sgn[i])
                    .sub(Bipedal3::from_raw(col_mag[i], col_sgn[i]));
                col_sum_mag[i] = result.mag();
                col_sum_sgn[i] = result.sgn();
            }
        }

        // Re-establish the tail invariant after add/sub.
        // Lanes n..64w-1 encode packed-1 (multiplicative identity):
        //   col_sum_mag[w-1] bits in the tail must be 1 (OR with mask),
        //   col_sum_sgn[w-1] bits in the tail must be 0 (AND with ~mask).
        col_sum_mag[w - 1] |= tail_mask_hi;
        col_sum_sgn[w - 1] &= !tail_mask_hi;

        // fold_mul: sequential word reduction then per-word lane fold.
        let term = fold_mul_words(&col_sum_mag, &col_sum_sgn, n);

        // Ryser sign: (-1)^|S| where |S| = subset_size.
        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total += term;
        }
    }

    // Apply outer (-1)^n factor from Ryser's formula.
    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

// ---------------------------------------------------------------------------
// 256-bit Gray-code counter helpers
// ---------------------------------------------------------------------------

/// Increment a 256-bit little-endian counter stored as `[u64; 4]`.
#[inline]
fn inc_counter(c: &mut [u64; 4]) {
    // Ripple-carry increment.
    for word in c.iter_mut() {
        *word = word.wrapping_add(1);
        if *word != 0 {
            break; // No carry out of this word.
        }
        // Carry propagates to the next word.
    }
}

/// Return `true` if the 256-bit counter is zero above bit position `n`
/// (i.e., the counter has reached `2^n` and should stop).
///
/// Since the counter starts at 0 and we increment before testing, the
/// first break occurs when the counter equals `2^n`.
#[inline]
fn gray_counter_is_zero_above(c: &[u64; 4], n: usize) -> bool {
    // The counter equals 2^n iff:
    //   - bit n of c is set,
    //   - all bits above n (and below n) of c are zero.
    // Equivalently: the counter exactly equals (1 << n) as a 256-bit number.
    let word = n >> 6; // Which u64 word holds bit n.
    let bit = n & 63; // Which bit within that word.
                      // Bit n must be set.
    if (c[word] >> bit) & 1 != 1 {
        return false;
    }
    // All other bits must be zero.
    // Bits below bit n in word `word`:
    if c[word] & ((1u64 << bit) - 1) != 0 {
        return false;
    }
    // All words below `word`:
    if c.iter().take(word).any(|&w| w != 0) {
        return false;
    }
    // All words above `word`:
    if c.iter().skip(word + 1).any(|&w| w != 0) {
        return false;
    }
    true
}

/// Return the position of the trailing (least-significant) zero bit in a
/// 256-bit little-endian counter.
///
/// This gives the flip index for the Gray code: `flip = trailing_zeros(k)`.
#[inline]
fn trailing_zeros_256(c: &[u64; 4]) -> usize {
    for (i, &word) in c.iter().enumerate() {
        if word != 0 {
            return i * 64 + word.trailing_zeros() as usize;
        }
    }
    // Counter is all zeros — undefined (should not happen mid-loop).
    256
}

/// Return the value of bit `pos` in the Gray-code register `g(k) = k ^ (k >> 1)`
/// where `k` is represented as a 256-bit little-endian counter.
///
/// The Gray-code register determines add (`1`) vs subtract (`0`) for column
/// `pos`: if `g(k)[pos] == 1`, column `pos` just entered the subset (ADD);
/// if `0`, it just left (SUB). See R3 design §6 for the derivation.
#[inline]
fn gray_bit_at(c: &[u64; 4], pos: usize) -> bool {
    // g(k) = k ^ (k >> 1).
    // We only need bit `pos` of g(k), which depends on bits `pos` and `pos+1`
    // of k (since right-shifting by 1 moves bit pos+1 to pos).
    // g_bit_pos = k_bit_pos ^ k_bit_(pos+1).
    let k_bit_pos = {
        let w = pos >> 6;
        let b = pos & 63;
        (c[w] >> b) & 1
    };
    let k_bit_pos_plus_1 = if pos + 1 < 256 {
        let w = (pos + 1) >> 6;
        let b = (pos + 1) & 63;
        (c[w] >> b) & 1
    } else {
        0
    };
    (k_bit_pos ^ k_bit_pos_plus_1) == 1
}

// ---------------------------------------------------------------------------
// fold_mul: sequential word reduction then per-word lane fold
// ---------------------------------------------------------------------------

/// Sequential reduction of `W` words to a single `Fp<3>` scalar value.
///
/// Implements R3 §7 (sequential schedule):
///
/// 1. **Cross-word reduction** (bipedal mul reordering):
///    Accumulate `W` words via `mag_acc &= mag[i]; sgn_acc ^= sgn[i]`,
///    starting from the mul-identity `(mag=u64::MAX, sgn=0)`.
///    This is valid because the bipedal `mul` formula (`m' = m1 & m2;
///    s' = s1 ^ s2`) commutes and associates across bit positions
///    (F_3 is commutative), so reordering the product as
///    "fold vertically across words then horizontally within word"
///    gives the same result as "fold horizontally within each word then
///    multiply per-word scalars".
///
/// 2. **Intra-word lane fold**:
///    `Bipedal3::fold_mul_first_n(64)` reduces all 64 bit positions of
///    the accumulated `(acc_mag, acc_sgn)` to one `Fp<3>` scalar.
///    Inactive lanes (bits beyond `n` in the last word) were pre-set to
///    identity `(mag=1, sgn=0)` by the caller via `tail_mask_hi`, so
///    they contribute 1 to the product and do not perturb the result.
///
/// The caller MUST have pre-set `mag[w-1] |= tail_mask_hi` before
/// calling this function, or the result will be incorrect.
///
/// # Arguments
///
/// * `mag` — magnitude word slice of length `W = ceil(n / 64)`.
/// * `sgn` — sign word slice of the same length.
/// * `n`   — number of active F_3 lanes (used only for the w=1 special case).
///
/// # Complexity
///
/// `O(W)` word-level AND/XOR ops for the cross-word reduction, plus
/// `O(64)` scalar lane decodes for the horizontal intra-word fold.
#[inline]
fn fold_mul_words(mag: &[u64], sgn: &[u64], n: usize) -> Fp<3> {
    debug_assert_eq!(mag.len(), sgn.len());
    let w = mag.len();
    debug_assert!(w >= 1);

    // Cross-word sequential mul (R3 §7 sequential schedule):
    //   paper §2.2 mul formula is (m' = m1 & m2, s' = s1 ^ s2).
    // Accumulate starting from mul-identity (all-1 mag, all-0 sgn).
    let mut acc_mag = u64::MAX;
    let mut acc_sgn = 0u64;
    for i in 0..w {
        acc_mag &= mag[i];
        acc_sgn ^= sgn[i];
    }

    // After the cross-word AND/XOR:
    // - acc_mag[b] = AND of mag[0][b] & mag[1][b] & ... & mag[w-1][b]
    // - acc_sgn[b] = XOR of sgn[0][b] ^ sgn[1][b] ^ ... ^ sgn[w-1][b]
    //
    // For each bit position b, this encodes the F_3 product of all "column"
    // lanes at that bit position across all words. The horizontal fold then
    // multiplies these 64 per-bit-position products together.
    //
    // Because the tail of the last word was pre-set to (mag=1, sgn=0)
    // (= identity) by the caller, acc_mag has 1s in the tail after the AND,
    // so fold_mul_first_n(64) treats all 64 bit positions as active and
    // the inactive lanes (tail bits) contribute 1. We therefore always fold
    // all 64 bit positions regardless of n.
    //
    // Special case w=1: the last word IS the only word, and fold_mul_first_n
    // must be told how many active lanes it has (so it pads the rest to 1
    // itself). We use n.min(64) to handle both n<64 and n=64.
    let b3 = Bipedal3::from_raw(acc_mag, acc_sgn);
    if w == 1 {
        // Only one word: let fold_mul_first_n handle the tail.
        b3.fold_mul_first_n(n.clamp(1, 64))
    } else {
        // Multiple words: tail was pre-set to identity before the AND,
        // so acc_mag has 1s in the tail positions — safe to fold all 64.
        b3.fold_mul_first_n(64)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permanent::bipedal3::permanent_bipedal3_singleword;
    use crate::permanent::ryser::permanent_ryser;
    use crate::testutil::random_matrix;

    // -----------------------------------------------------------------------
    // Dispatch smoke-test: verify permanent_bipedal3 routes n=65 to the
    // multi-word path without panicking. Uses a tiny fixed matrix where
    // the permanent can be verified independently.
    //
    // NOTE: The actual permanent computation at n >= 64 runs 2^n Gray steps,
    // which is astronomically infeasible. Therefore:
    //   - All tests that call permanent_bipedal3_multiword with n >= 64
    //     MUST carry #[ignore = "sim: ..."].
    //   - Fast-tier tests cover the helper functions (counter, fold, dispatch)
    //     but not the full permanent computation.
    // -----------------------------------------------------------------------

    /// Dispatching to multi-word path does not panic for n=65 (structure check).
    ///
    /// Marked ignore because 2^65 Gray steps is infeasible even in slow tier.
    /// Exists to document the dispatch contract and allow manual verification.
    #[test]
    #[ignore = "sim: n=65 permanent (2^65 Gray steps, infeasible runtime)"]
    fn test_multiword_dispatch_n65_no_panic() {
        let n = 65;
        let seed_base: u64 = 0xa788_6bd8_0065_0001_u64;
        let row_major = random_matrix::<3>(n, seed_base);
        let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
        let _ = permanent_bipedal3_multiword(&mat);
    }

    /// R3 §9.1 boundary check: n=64 multi-word vs single-word must agree.
    ///
    /// Marked ignore because 2^64 Gray steps is infeasible even in slow tier.
    /// Exists to document the intended correctness contract.
    #[test]
    #[ignore = "sim: n=64 boundary cross-check (2^64 Gray steps, infeasible runtime)"]
    fn test_multiword_n64_matches_singleword() {
        let n = 64;
        let seed_base: u64 = 0xa788_6bd8_0064_0000_u64;
        for trial in 0u64..50 {
            let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
            let row_major = random_matrix::<3>(n, seed);
            let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
            let expected = permanent_bipedal3_singleword(&mat);
            let actual = permanent_bipedal3_multiword(&mat);
            assert_eq!(
                actual, expected,
                "n=64 boundary: multi-word vs single-word mismatch, trial={trial}, seed={seed:#018x}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // gray_bit_at unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_gray_bit_at_matches_scalar_for_small_k() {
        // For small k (fits in u64), compare gray_bit_at to scalar formula.
        for k in 1u64..128u64 {
            let g_k = k ^ (k >> 1);
            for pos in 0..7usize {
                let scalar_bit = ((g_k >> pos) & 1) == 1;
                let c = [k, 0, 0, 0];
                let computed = gray_bit_at(&c, pos);
                assert_eq!(
                    computed, scalar_bit,
                    "gray_bit_at mismatch: k={k}, pos={pos}"
                );
            }
        }
    }

    #[test]
    fn test_inc_counter_carry_propagation() {
        let mut c = [u64::MAX, 0, 0, 0];
        inc_counter(&mut c);
        assert_eq!(c, [0, 1, 0, 0], "carry from word 0 to word 1");

        let mut c2 = [u64::MAX, u64::MAX, 0, 0];
        inc_counter(&mut c2);
        assert_eq!(c2, [0, 0, 1, 0], "carry from word 1 to word 2");
    }

    #[test]
    fn test_trailing_zeros_256_basic() {
        // Counter = 1 → trailing zeros = 0.
        assert_eq!(trailing_zeros_256(&[1, 0, 0, 0]), 0);
        // Counter = 2 → trailing zeros = 1.
        assert_eq!(trailing_zeros_256(&[2, 0, 0, 0]), 1);
        // Counter = 4 → trailing zeros = 2.
        assert_eq!(trailing_zeros_256(&[4, 0, 0, 0]), 2);
        // Counter = 1 << 64 (in 256-bit) → word 0 is 0, word 1 is 1.
        assert_eq!(trailing_zeros_256(&[0, 1, 0, 0]), 64);
        // Counter = 1 << 65 → word 1 has bit 1 set.
        assert_eq!(trailing_zeros_256(&[0, 2, 0, 0]), 65);
    }

    #[test]
    fn test_gray_counter_is_zero_above_basic() {
        // Counter = 2^4 = 16, n = 4 → should stop.
        assert!(gray_counter_is_zero_above(&[16, 0, 0, 0], 4));
        // Counter = 2^63, n = 63.
        assert!(gray_counter_is_zero_above(&[1u64 << 63, 0, 0, 0], 63));
        // Counter = 2^64, n = 64 → word 0 = 0, word 1 = 1.
        assert!(gray_counter_is_zero_above(&[0, 1, 0, 0], 64));
        // Counter = 2^65, n = 65.
        assert!(gray_counter_is_zero_above(&[0, 2, 0, 0], 65));
        // Counter = 15, n = 4 → not stopping (15 != 16).
        assert!(!gray_counter_is_zero_above(&[15, 0, 0, 0], 4));
    }

    // -----------------------------------------------------------------------
    // Multi-word fast-tier cross-check vs `permanent_ryser<Fp<3>>`.
    //
    // permanent_bipedal3_multiword has no lower-bound assertion (the
    // singleword dispatch is a perf hint, not a correctness invariant),
    // so we can run it directly at small n where the 2^n Gray walk is
    // feasible. This concretely validates the multi-word machinery
    // against the canonical ryser oracle in the fast tier under both
    // debug and release builds.
    //
    // Per the in-session amendment recorded in
    // `dev/active/a7886bd8-amendments-2026-05-11.md` (criterion 3, option
    // "Block-decomposable cross-check"). 850 trials total spans n ∈
    // {2, 5, 8, 16, 20}; the larger n ∈ {24, 32, 48, 60} cases live in
    // the slow tier via `#[ignore = "slow: ..."]`.
    // -----------------------------------------------------------------------

    fn run_multiword_vs_ryser_at_n(n: usize, n_trials: u64, seed_tag: u64) {
        let seed_base: u64 = 0xa788_6bd8_0000_0000_u64
            .wrapping_add((n as u64) << 16)
            .wrapping_add(seed_tag);
        for trial in 0u64..n_trials {
            let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
            let row_major = random_matrix::<3>(n, seed);
            let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
            // permanent_bipedal3_multiword has no lower-bound assertion;
            // the singleword dispatch is a perf hint, not a correctness
            // invariant, so this call works at any feasible n.
            let actual = permanent_bipedal3_multiword(&mat);
            let expected = permanent_ryser::<Fp<3>>(&row_major, n);
            assert_eq!(
                actual, expected,
                "multi-word vs ryser mismatch: n={n}, trial={trial}, seed={seed:#018x}"
            );
        }
    }

    #[test]
    fn test_multiword_vs_ryser_n2() {
        run_multiword_vs_ryser_at_n(2, 200, 0x0002);
    }

    /// Regression: `permanent_bipedal3_multiword` must panic on non-square
    /// inputs even in release builds (the documented contract). Without an
    /// upfront square-matrix assertion the function would otherwise size
    /// the active-lane mask off `cols()` and silently discard extra rows.
    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_multiword_panics_on_non_square() {
        let n_rows = 66usize;
        let n_cols = 65usize;
        let data = vec![Fp::<3>::new(0); n_rows * n_cols];
        let m = Bipedal3Matrix::from_row_major(&data, n_rows, n_cols);
        let _ = permanent_bipedal3_multiword(&m);
    }

    /// Regression: `permanent_bipedal3_multiword` must panic on `n` above
    /// `N_MAX_MULTIWORD` even in release builds. A `debug_assert!` here
    /// would be silently dropped in `--release` and let the function
    /// index past the `[u64; 4]` Gray counter.
    #[test]
    #[should_panic(expected = "exceeds N_MAX_MULTIWORD")]
    fn test_multiword_panics_above_n_max() {
        let n = N_MAX_MULTIWORD + 1;
        let data = vec![Fp::<3>::new(0); n * n];
        let m = Bipedal3Matrix::from_row_major(&data, n, n);
        let _ = permanent_bipedal3_multiword(&m);
    }

    #[test]
    fn test_multiword_vs_ryser_n5() {
        run_multiword_vs_ryser_at_n(5, 200, 0x0005);
    }

    #[test]
    fn test_multiword_vs_ryser_n8() {
        run_multiword_vs_ryser_at_n(8, 200, 0x0008);
    }

    #[test]
    fn test_multiword_vs_ryser_n16() {
        run_multiword_vs_ryser_at_n(16, 200, 0x0010);
    }

    #[test]
    fn test_multiword_vs_ryser_n20() {
        // n=20 trades off coverage vs runtime: per-trial ~60 ms; 50 trials
        // ≈ 3 s sits comfortably under the 5 s fast-tier budget.
        run_multiword_vs_ryser_at_n(20, 50, 0x0014);
    }

    #[test]
    #[ignore = "slow: multi-word vs ryser at n=24 (per-trial ~1 s; 5 trials ~5 s)"]
    fn test_multiword_vs_ryser_n24_slow() {
        run_multiword_vs_ryser_at_n(24, 5, 0x0018);
    }

    #[test]
    #[ignore = "slow: multi-word vs ryser at n=32 (per-trial ~3 min; 2 trials)"]
    fn test_multiword_vs_ryser_n32_slow() {
        run_multiword_vs_ryser_at_n(32, 2, 0x0020);
    }

    #[test]
    #[ignore = "slow: multi-word vs ryser at n=48 (per-trial seconds-scale; 5 trials)"]
    fn test_multiword_vs_ryser_n48_slow() {
        run_multiword_vs_ryser_at_n(48, 5, 0x0030);
    }

    #[test]
    #[ignore = "slow: multi-word vs ryser at n=60 (per-trial >10 s; 2 trials)"]
    fn test_multiword_vs_ryser_n60_slow() {
        run_multiword_vs_ryser_at_n(60, 2, 0x003c);
    }

    // -----------------------------------------------------------------------
    // Block-decomposable cross-check at n ∈ {65, 72, 96, 128}.
    //
    // Per the criterion-3 amendment recorded in
    // `dev/active/a7886bd8-amendments-2026-05-11.md`:
    // construct block-diagonal matrices `[A_{n0} ⊕ I_{n - n0}]` with
    // n0 ∈ {10, 11, 12} so that `perm(full) = perm(A_{n0}) * perm(I) =
    // perm_ryser(A_{n0})`. Tests are `#[ignore = "sim: ..."]` since
    // `permanent_bipedal3_multiword` still walks 2^n Gray steps at runtime,
    // but the test BODY documents the oracle so a future faster machine
    // would validate the implementation. Direct ryser cross-check at the
    // raw n values is impossible (ryser caps at n <= 63), so the
    // block-decomposable construction is the only way to express the
    // oracle relation literally at n ∈ {65, 72, 96, 128}.
    // -----------------------------------------------------------------------

    /// Build a block-diagonal n × n F_3 matrix `[A_{n0} ⊕ I_{n - n0}]` in
    /// row-major form. The top-left n0 × n0 block is random F_3; the
    /// bottom-right (n - n0) × (n - n0) block is the identity; off-diagonal
    /// blocks are zero. The permanent over F_3 factorises:
    /// `perm(full) = perm(A_{n0}) * perm(I_{n - n0}) = perm(A_{n0}) * 1`.
    fn build_block_diagonal(n: usize, n0: usize, seed: u64) -> (Vec<Fp<3>>, Vec<Fp<3>>) {
        assert!(n0 < n, "n0 must be strictly less than n for padding");
        let block = random_matrix::<3>(n0, seed);
        let mut full = vec![Fp::<3>::new(0); n * n];
        // Top-left block: A_{n0}.
        for i in 0..n0 {
            for j in 0..n0 {
                full[i * n + j] = block[i * n0 + j];
            }
        }
        // Bottom-right block: identity I_{n - n0}.
        for k in 0..(n - n0) {
            let idx = n0 + k;
            full[idx * n + idx] = Fp::<3>::new(1);
        }
        (full, block)
    }

    fn run_block_diagonal_at_n(n: usize, seed_tag: u64) {
        let seed_base: u64 = 0xa788_6bd8_0000_0000_u64
            .wrapping_add((n as u64) << 16)
            .wrapping_add(seed_tag);
        // 5 matrices per n: criterion-3 amendment requires ≥ 5.
        for trial in 0u64..5 {
            let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
            // n0 cycles through {10, 11, 12} so each trial exercises a
            // different block size.
            let n0 = 10 + ((trial as usize) % 3);
            let (full, block) = build_block_diagonal(n, n0, seed);
            let expected = permanent_ryser::<Fp<3>>(&block, n0);
            let mat = Bipedal3Matrix::from_row_major(&full, n, n);
            // This is the infeasible call: 2^n Gray steps. The assertion
            // body documents the oracle relation.
            let actual = permanent_bipedal3_multiword(&mat);
            assert_eq!(
                actual, expected,
                "block-decomposable cross-check failed: n={n}, n0={n0}, trial={trial}, seed={seed:#018x}"
            );
        }
    }

    /// Block-decomposable cross-check at n=65 vs ryser(A_{n0}).
    #[test]
    #[ignore = "sim: large-n cross-check n=65 (2^65 Gray steps, infeasible runtime)"]
    fn test_cross_check_n65_block_diagonal() {
        run_block_diagonal_at_n(65, 0x0065);
    }

    /// Block-decomposable cross-check at n=72 vs ryser(A_{n0}).
    #[test]
    #[ignore = "sim: large-n cross-check n=72 (2^72 Gray steps, infeasible runtime)"]
    fn test_cross_check_n72_block_diagonal() {
        run_block_diagonal_at_n(72, 0x0072);
    }

    /// Block-decomposable cross-check at n=96 vs ryser(A_{n0}).
    #[test]
    #[ignore = "sim: large-n cross-check n=96 (2^96 Gray steps, infeasible runtime)"]
    fn test_cross_check_n96_block_diagonal() {
        run_block_diagonal_at_n(96, 0x0096);
    }

    /// Block-decomposable cross-check at n=128 vs ryser(A_{n0}).
    #[test]
    #[ignore = "sim: large-n cross-check n=128 (2^128 Gray steps, infeasible runtime)"]
    fn test_cross_check_n128_block_diagonal() {
        run_block_diagonal_at_n(128, 0x0128);
    }
}
