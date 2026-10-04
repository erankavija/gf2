//! Property-based tests for rank and select operations using proptest.

use gf2_core::BitVec;
use proptest::prelude::*;

proptest! {
    #[test]
    fn rank_counts_ones(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for i in 0..bv.len() {
            let expected = (0..=i).filter(|&j| bv.get(j)).count();
            prop_assert_eq!(bv.rank(i), expected);
        }
    }

    #[test]
    fn rank_is_monotonic(bytes in prop::collection::vec(any::<u8>(), 1..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        if bv.len() > 1 {
            for i in 0..bv.len() - 1 {
                prop_assert!(bv.rank(i) <= bv.rank(i + 1));
            }
        }
    }

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

    #[test]
    fn rank_last_equals_count_ones(bytes in prop::collection::vec(any::<u8>(), 1..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        if !bv.is_empty() {
            prop_assert_eq!(bv.rank(bv.len() - 1), bv.count_ones());
        }
    }

    #[test]
    fn select_returns_set_bits(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones() {
            if let Some(pos) = bv.select(k) {
                prop_assert!(bv.get(pos), "select({}) = {} but bit is not set", k, pos);
            }
        }
    }

    #[test]
    fn select_out_of_range_is_none(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let total = bv.count_ones();

        prop_assert_eq!(bv.select(total), None);
        prop_assert_eq!(bv.select(total + 1), None);
    }

    #[test]
    fn rank_select_invariant(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones() {
            if let Some(i) = bv.select(k) {
                prop_assert_eq!(bv.rank(i), k + 1);
            }
        }
    }

    #[test]
    fn select_is_monotonic(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);

        for k in 0..bv.count_ones().saturating_sub(1) {
            if let (Some(pos_k), Some(pos_k1)) = (bv.select(k), bv.select(k + 1)) {
                prop_assert!(pos_k < pos_k1);
            }
        }
    }

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

    #[test]
    fn rank_all_zeros(len in 0usize..1000) {
        let bv = BitVec::zeros(len);

        for i in 0..len {
            prop_assert_eq!(bv.rank(i), 0);
        }
    }

    #[test]
    fn rank_all_ones(len in 1usize..1000) {
        let bv = BitVec::ones(len);

        for i in 0..len {
            prop_assert_eq!(bv.rank(i), i + 1);
        }
    }

    #[test]
    fn select_all_ones(len in 1usize..1000) {
        let bv = BitVec::ones(len);

        for k in 0..len {
            prop_assert_eq!(bv.select(k), Some(k));
        }
    }

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
