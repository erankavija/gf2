//! Property-based tests for rank and select operations using proptest.

use gf2_core::BitVec;
use proptest::prelude::*;

proptest! {
    /// Property: rank(i) counts set bits in [0..=i]
    #[test]
    fn rank_counts_ones(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for i in 0..bv.len() {
            let expected = (0..=i).filter(|&j| bv.get(j)).count();
            prop_assert_eq!(bv.rank(i), expected);
        }
    }

    /// Property: rank is monotonically increasing
    #[test]
    fn rank_is_monotonic(bytes in prop::collection::vec(any::<u8>(), 1..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        if bv.len() > 1 {
            for i in 0..bv.len() - 1 {
                prop_assert!(bv.rank(i) <= bv.rank(i + 1));
            }
        }
    }

    /// Property: rank increases by at most 1 between consecutive positions
    #[test]
    fn rank_increments_by_at_most_one(bytes in prop::collection::vec(any::<u8>(), 1..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        if bv.len() > 1 {
            for i in 0..bv.len() - 1 {
                let diff = bv.rank(i + 1) - bv.rank(i);
                prop_assert!(diff <= 1);
            }
        }
    }

    /// Property: rank(len-1) equals count_ones()
    #[test]
    fn rank_last_equals_count_ones(bytes in prop::collection::vec(any::<u8>(), 1..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        if !bv.is_empty() {
            prop_assert_eq!(bv.rank(bv.len() - 1), bv.count_ones());
        }
    }

    /// Property: select returns positions where bits are set
    #[test]
    fn select_returns_set_bits(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones() {
            if let Some(pos) = bv.select(k) {
                prop_assert!(bv.get(pos), "select({}) = {} but bit is not set", k, pos);
            }
        }
    }

    /// Property: select(k) is None for k >= count_ones()
    #[test]
    fn select_out_of_range_is_none(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let total = bv.count_ones();

        prop_assert_eq!(bv.select(total), None);
        prop_assert_eq!(bv.select(total + 1), None);
    }

    /// Property: rank(select(k)) = k + 1 for all valid k
    #[test]
    fn rank_select_invariant(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones() {
            if let Some(i) = bv.select(k) {
                prop_assert_eq!(bv.rank(i), k + 1);
            }
        }
    }

    /// Property: select is monotonically increasing
    #[test]
    fn select_is_monotonic(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones().saturating_sub(1) {
            if let (Some(pos_k), Some(pos_k1)) = (bv.select(k), bv.select(k + 1)) {
                prop_assert!(pos_k < pos_k1);
            }
        }
    }

    /// Property: select positions cover all set bits
    #[test]
    fn select_covers_all_ones(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        let mut selected_positions = Vec::new();
        for k in 0..bv.count_ones() {
            if let Some(pos) = bv.select(k) {
                selected_positions.push(pos);
            }
        }

        let expected_positions: Vec<usize> = (0..bv.len()).filter(|&i| bv.get(i)).collect();
        prop_assert_eq!(selected_positions, expected_positions);
    }

    /// Property: rank of select(k) - 1 positions equals k
    #[test]
    fn rank_before_select(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones() {
            if let Some(i) = bv.select(k) {
                if i > 0 {
                    prop_assert_eq!(bv.rank(i - 1), k);
                }
            }
        }
    }

    /// Property: All zeros => rank always 0
    #[test]
    fn rank_all_zeros(len in 0usize..1000) {
        let bv = BitVec::zeros(len);

        for i in 0..len {
            prop_assert_eq!(bv.rank(i), 0);
        }
    }

    /// Property: All ones => rank(i) = i + 1
    #[test]
    fn rank_all_ones(len in 1usize..1000) {
        let bv = BitVec::ones(len);

        for i in 0..len {
            prop_assert_eq!(bv.rank(i), i + 1);
        }
    }

    /// Property: All ones => select(k) = k
    #[test]
    fn select_all_ones(len in 1usize..1000) {
        let bv = BitVec::ones(len);

        for k in 0..len {
            prop_assert_eq!(bv.select(k), Some(k));
        }
    }

    /// Property: rank differences equal bit values
    #[test]
    fn rank_diff_equals_bit(bytes in prop::collection::vec(any::<u8>(), 1..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        if bv.len() > 1 {
            for i in 0..bv.len() - 1 {
                let diff = bv.rank(i + 1) - bv.rank(i);
                let bit_val = if bv.get(i + 1) { 1 } else { 0 };
                prop_assert_eq!(diff, bit_val);
            }
        }
    }
}
