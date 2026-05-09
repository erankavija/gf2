//! Gray-code subset enumeration used by Ryser's permanent formula and the
//! `permanent_bipedal*` kernels.
//!
//! Hosts the [`gray_code_iter`] enumerator that walks the `2^n - 1`
//! non-empty subsets of a length-`n` universe by toggling one bit per
//! step. Each step yields `(flip_index, parity)`: which bit toggled,
//! and whether the toggled bit was just added (`+1`) or removed (`-1`)
//! from the running subset.
//!
//! See `dev/plans/d1a_gf2_algebra_boundary.md` §4.2 for why this lives
//! in `gf2-algebra` rather than alongside the unrelated M4RM Gray table
//! in `gf2-core::alg::m4rm`. The formula derivation is in
//! `dev/plans/r3_multi_word_streaming.md` §3 and §6.
//!
//! Re-exported as [`crate::permanent::gray`] for callers that want the
//! permanent-grouped path.

/// Gray-code subset enumerator yielding `(flip_index, parity)` for
/// `k` in `1..2^n`.
///
/// At each step `k` exactly one bit of the current subset toggles.
/// `flip_index` is which bit toggled; `parity` is `+1` if the bit was
/// added (it is now set in the Gray-code register `g(k) = k ^ (k >> 1)`)
/// or `-1` if removed (now clear in `g(k)`).
///
/// The cumulative XOR of `1 << flip_index` across the iteration walks
/// the binary-reflected Gray code `g(1), g(2), ..., g(2^n - 1)`,
/// visiting every non-empty subset of `[n]` exactly once. The running
/// sum of `parity` equals the popcount of the current subset.
///
/// # Arguments
///
/// * `n` — universe size; the iterator yields `2^n - 1` items. Must
///   satisfy `n <= 63` because the implementation uses `1u64 << n` to
///   bound the iteration, and `1u64 << 64` is undefined behaviour per
///   the Rust reference (shift by the full type width). The permanent
///   inner loop targets `n <= 16` exhaustively per W1-T6 success
///   criteria, but larger `n` works correctly up to `n = 63`; iteration
///   cost is `O(2^n)` so callers will rarely exceed `n = 36` in practice
///   (epic doc §7.3).
///
/// # Examples
///
/// ```
/// use gf2_algebra::gray::gray_code_iter;
///
/// let items: Vec<_> = gray_code_iter(3).collect();
/// assert_eq!(items.len(), 7); // 2^3 - 1
///
/// // First four items trace the binary-reflected Gray code:
/// // {} -> {0} -> {0,1} -> {1} -> {1,2}
/// assert_eq!(items[0], (0, 1)); // add bit 0
/// assert_eq!(items[1], (1, 1)); // add bit 1
/// assert_eq!(items[2], (0, -1)); // remove bit 0
/// assert_eq!(items[3], (2, 1)); // add bit 2
/// ```
///
/// # Panics
///
/// Panics in debug builds (and is undefined behaviour in release) if
/// `n >= 64`, because the bound `1u64 << n` shifts by ≥ the type width.
/// For `n == 0` the iterator yields zero items (the empty universe has
/// only the empty subset, which is excluded). Callers MUST guarantee
/// `n <= 63`. The permanent driver enforces `n <= 63` upstream via
/// matrix dimension checks (epic doc §7.3).
///
/// # Complexity
///
/// `O(2^n)` time, `O(1)` space. Iterator state is one `u64` index plus
/// the universe size; no heap allocation.
///
/// # Formula
///
/// At Gray step `k`:
///
/// 1. `flip = trailing_zeros(k)` — the unique bit that toggles.
/// 2. `g_k = k ^ (k >> 1)` — the binary-reflected Gray code value,
///    which equals the active subset's bit-vector representation.
/// 3. `parity = +1` if bit `flip` is set in `g_k` (just added), else
///    `-1` (just removed).
///
/// Inspecting `(k >> flip) & 1` instead of `(g_k >> flip) & 1` is a
/// trap: with `flip = trailing_zeros(k)`, bit `flip` of `k` is always
/// `1` by construction, so the predicate is identically true and the
/// loop only ever adds, never subtracts. See
/// `dev/plans/r3_multi_word_streaming.md` §6 for the worked derivation.
#[inline]
pub fn gray_code_iter(n: usize) -> impl Iterator<Item = (usize, i8)> {
    // For n == 0 the upper bound `1u64 << 0 == 1` makes the range
    // `1..1` empty, which is the desired behaviour (no non-empty
    // subsets of the empty universe). For n in 1..=63 the bound
    // `1u64 << n` is well-defined; `1u64 << 64` is UB per the Rust
    // reference, so the issue contract restricts the iterator to
    // `n <= 63`.
    let upper = 1u64 << n;
    (1u64..upper).map(|k| {
        let flip = k.trailing_zeros() as usize;
        let g_k = k ^ (k >> 1);
        // Bit `flip` of `g_k` is the new state of that bit in the
        // active subset after the toggle. Set => column just entered
        // the subset (ADD, parity +1); clear => column just left
        // (SUB, parity -1).
        let parity: i8 = if ((g_k >> flip) & 1) == 1 { 1 } else { -1 };
        (flip, parity)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-verified sequence for `n = 3` matching the canonical
    /// binary-reflected Gray code: subset register starts at 0, after
    /// `k = 1` toggles bit 0 -> `{0}`, after `k = 2` toggles bit 1 ->
    /// `{0,1}`, after `k = 3` toggles bit 0 -> `{1}`, after `k = 4`
    /// toggles bit 2 -> `{1,2}`, after `k = 5` toggles bit 0 ->
    /// `{0,1,2}`, after `k = 6` toggles bit 1 -> `{0,2}`, after
    /// `k = 7` toggles bit 0 -> `{2}`.
    ///
    /// Expected `(flip, parity)` sequence:
    ///
    /// ```text
    /// k = 1: (0, +1)   subset = {0}
    /// k = 2: (1, +1)   subset = {0,1}
    /// k = 3: (0, -1)   subset = {1}
    /// k = 4: (2, +1)   subset = {1,2}
    /// k = 5: (0, +1)   subset = {0,1,2}
    /// k = 6: (1, -1)   subset = {0,2}
    /// k = 7: (0, -1)   subset = {2}
    /// ```
    #[test]
    fn test_gray_code_iter_k_1_to_4_traces_paper_table() {
        let items: Vec<_> = gray_code_iter(3).collect();
        assert_eq!(items.len(), 7);
        assert_eq!(items[0], (0, 1), "k=1: add bit 0 -> {{0}}");
        assert_eq!(items[1], (1, 1), "k=2: add bit 1 -> {{0,1}}");
        assert_eq!(items[2], (0, -1), "k=3: remove bit 0 -> {{1}}");
        assert_eq!(items[3], (2, 1), "k=4: add bit 2 -> {{1,2}}");
        assert_eq!(items[4], (0, 1), "k=5: add bit 0 -> {{0,1,2}}");
        assert_eq!(items[5], (1, -1), "k=6: remove bit 1 -> {{0,2}}");
        assert_eq!(items[6], (0, -1), "k=7: remove bit 0 -> {{2}}");
    }

    fn count_for(n: usize) -> usize {
        gray_code_iter(n).count()
    }

    #[test]
    fn test_gray_code_iter_yields_pow2_minus_one_items_n_1() {
        assert_eq!(count_for(1), (1usize << 1) - 1);
    }

    #[test]
    fn test_gray_code_iter_yields_pow2_minus_one_items_n_2() {
        assert_eq!(count_for(2), (1usize << 2) - 1);
    }

    #[test]
    fn test_gray_code_iter_yields_pow2_minus_one_items_n_3() {
        assert_eq!(count_for(3), (1usize << 3) - 1);
    }

    #[test]
    fn test_gray_code_iter_yields_pow2_minus_one_items_n_4() {
        assert_eq!(count_for(4), (1usize << 4) - 1);
    }

    #[test]
    fn test_gray_code_iter_yields_pow2_minus_one_items_n_8() {
        assert_eq!(count_for(8), (1usize << 8) - 1);
    }

    #[test]
    fn test_gray_code_iter_yields_pow2_minus_one_items_n_16() {
        assert_eq!(count_for(16), (1usize << 16) - 1);
    }

    /// Walk the toggles into a running `u64` subset register and
    /// collect every intermediate value. Assert the multiset equals
    /// `{1, 2, ..., 2^n - 1}` exactly — every non-empty subset of
    /// `[n]` visited exactly once. Implements criterion 2.
    fn assert_visits_every_nonempty_subset(n: usize) {
        let mut register: u64 = 0;
        let mut visited: Vec<u64> = Vec::with_capacity((1usize << n) - 1);
        for (flip, _parity) in gray_code_iter(n) {
            register ^= 1u64 << flip;
            visited.push(register);
        }
        assert_eq!(visited.len(), (1usize << n) - 1);
        let mut sorted = visited.clone();
        sorted.sort_unstable();
        let expected: Vec<u64> = (1u64..(1u64 << n)).collect();
        assert_eq!(
            sorted, expected,
            "n = {} did not enumerate every non-empty subset exactly once",
            n
        );
    }

    #[test]
    fn test_gray_code_iter_visits_every_nonempty_subset_n_1() {
        assert_visits_every_nonempty_subset(1);
    }

    #[test]
    fn test_gray_code_iter_visits_every_nonempty_subset_n_2() {
        assert_visits_every_nonempty_subset(2);
    }

    #[test]
    fn test_gray_code_iter_visits_every_nonempty_subset_n_3() {
        assert_visits_every_nonempty_subset(3);
    }

    #[test]
    fn test_gray_code_iter_visits_every_nonempty_subset_n_4() {
        assert_visits_every_nonempty_subset(4);
    }

    #[test]
    fn test_gray_code_iter_visits_every_nonempty_subset_n_8() {
        assert_visits_every_nonempty_subset(8);
    }

    #[test]
    fn test_gray_code_iter_visits_every_nonempty_subset_n_16() {
        assert_visits_every_nonempty_subset(16);
    }

    /// At every step, the running sum of `parity` from `k = 1..K`
    /// equals `popcount(register_at_step_K)`. Asserts the invariant
    /// at every step for `n in {1,2,3,4,8}`. Implements criterion 3.
    fn assert_running_parity_matches_popcount_per_step(n: usize) {
        let mut register: u64 = 0;
        let mut parity_sum: i64 = 0;
        for (flip, parity) in gray_code_iter(n) {
            register ^= 1u64 << flip;
            parity_sum += parity as i64;
            assert_eq!(
                parity_sum,
                register.count_ones() as i64,
                "n = {}, register = {:b}, parity_sum {} != popcount {}",
                n,
                register,
                parity_sum,
                register.count_ones()
            );
        }
    }

    #[test]
    fn test_gray_code_iter_parity_matches_running_popcount_n_1() {
        assert_running_parity_matches_popcount_per_step(1);
    }

    #[test]
    fn test_gray_code_iter_parity_matches_running_popcount_n_2() {
        assert_running_parity_matches_popcount_per_step(2);
    }

    #[test]
    fn test_gray_code_iter_parity_matches_running_popcount_n_3() {
        assert_running_parity_matches_popcount_per_step(3);
    }

    #[test]
    fn test_gray_code_iter_parity_matches_running_popcount_n_4() {
        assert_running_parity_matches_popcount_per_step(4);
    }

    #[test]
    fn test_gray_code_iter_parity_matches_running_popcount_n_8() {
        assert_running_parity_matches_popcount_per_step(8);
    }

    /// At `n = 16` the per-step assertion is 65535 checks; we still
    /// run the per-step invariant since the cost is negligible in
    /// release mode, fully covering criterion 3 at the largest
    /// exhaustive universe size required by criterion 4.
    #[test]
    fn test_gray_code_iter_parity_matches_running_popcount_n_16() {
        assert_running_parity_matches_popcount_per_step(16);
    }

    /// `n = 0` is degenerate but well-defined: the empty universe has
    /// only the empty subset, and the iterator excludes it, so the
    /// stream is empty.
    #[test]
    fn test_gray_code_iter_yields_zero_items_n_0() {
        assert_eq!(gray_code_iter(0).count(), 0);
    }
}
