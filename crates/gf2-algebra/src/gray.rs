//! Gray-code subset enumeration used by Ryser's permanent formula and the
//! `permanent_bipedal*` kernels, re-exported as [`crate::permanent::gray`].

/// Convert a Gray-code sequential index to the corresponding subset bitmask.
///
/// The binary-reflected Gray code maps index `k` to the subset bitmask
/// `g(k) = k ^ (k >> 1)`. Bit `j` set in the result means column `j` is
/// present in the `k`-th non-empty subset visited by [`gray_code_iter`].
#[inline]
pub fn gray_code_index_to_subset(k: u64) -> u64 {
    k ^ (k >> 1)
}

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
/// # Panics
///
/// Panics if `n >= 64`. For `n == 0` the iterator yields zero items (the
/// empty universe has only the empty subset, which is excluded).
///
/// # Complexity
///
/// `O(2^n)` time, `O(1)` space.
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
/// Inspecting `(k >> flip) & 1` instead of `(g_k >> flip) & 1` is a trap: with
/// `flip = trailing_zeros(k)`, bit `flip` of `k` is always `1` by construction,
/// so the predicate is identically true and the loop only ever adds, never
/// subtracts.
#[inline]
pub fn gray_code_iter(n: usize) -> impl Iterator<Item = (usize, i8)> {
    assert!(
        n <= 63,
        "gray_code_iter: n must satisfy n <= 63; got n = {n}"
    );
    let upper: u128 = 1u128 << n;
    (1u128..upper).map(|k| {
        let flip = k.trailing_zeros() as usize;
        let g_k = k ^ (k >> 1);
        let parity: i8 = if ((g_k >> flip) & 1) == 1 { 1 } else { -1 };
        (flip, parity)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-verified sequence for `n = 3` matching the canonical
    /// binary-reflected Gray code.
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

    #[test]
    fn test_gray_code_index_to_subset_k0() {
        assert_eq!(gray_code_index_to_subset(0), 0);
    }

    /// Hand-verified values matching the canonical BRGC formula `g(k) = k ^ (k>>1)`.
    #[test]
    fn test_gray_code_index_to_subset_hand_checked() {
        assert_eq!(gray_code_index_to_subset(1), 0b001);
        assert_eq!(gray_code_index_to_subset(2), 0b011);
        assert_eq!(gray_code_index_to_subset(3), 0b010);
        assert_eq!(gray_code_index_to_subset(4), 0b110);
        assert_eq!(gray_code_index_to_subset(5), 0b111);
        assert_eq!(gray_code_index_to_subset(6), 0b101);
        assert_eq!(gray_code_index_to_subset(7), 0b100);
    }

    #[test]
    fn test_gray_code_index_to_subset_consistent_with_iter_n4() {
        let n = 4;
        let mut register: u64 = 0;
        for (step, (flip, _parity)) in gray_code_iter(n).enumerate() {
            let k = (step + 1) as u64;
            register ^= 1u64 << flip;
            assert_eq!(
                gray_code_index_to_subset(k),
                register,
                "mismatch at k={k}: gray_code_index_to_subset={:#06b}, register={register:#06b}",
                gray_code_index_to_subset(k)
            );
        }
    }
}
