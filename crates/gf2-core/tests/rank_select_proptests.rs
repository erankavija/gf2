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

/// Mutation applied between rank/select queries.
#[derive(Debug, Clone)]
enum Mutation {
    Set(usize, bool),
    Push(bool),
    Pop,
    And(Vec<bool>),
    Or(Vec<bool>),
    Xor(Vec<bool>),
    Not,
    ShiftLeft(usize),
    ShiftRight(usize),
    Resize(usize, bool),
    Clear,
    BitSliceMutFill(usize, usize),
    BitReverse,
    Polar,
}

fn mutation_strategy() -> impl Strategy<Value = Mutation> {
    let mask = prop::collection::vec(any::<bool>(), 0..130);
    prop_oneof![
        (0usize..200, any::<bool>()).prop_map(|(i, b)| Mutation::Set(i, b)),
        any::<bool>().prop_map(Mutation::Push),
        Just(Mutation::Pop),
        mask.clone().prop_map(Mutation::And),
        mask.clone().prop_map(Mutation::Or),
        mask.prop_map(Mutation::Xor),
        Just(Mutation::Not),
        (0usize..140).prop_map(Mutation::ShiftLeft),
        (0usize..140).prop_map(Mutation::ShiftRight),
        (0usize..200, any::<bool>()).prop_map(|(n, b)| Mutation::Resize(n, b)),
        Just(Mutation::Clear),
        (0usize..200, 0usize..200).prop_map(|(a, b)| Mutation::BitSliceMutFill(a, b)),
        Just(Mutation::BitReverse),
        Just(Mutation::Polar),
    ]
}

fn apply(bv: &mut BitVec, reference: &mut Vec<bool>, m: &Mutation) {
    let n = reference.len();
    let from_mask =
        |mask: &[bool]| -> Vec<bool> { (0..n).map(|i| *mask.get(i).unwrap_or(&false)).collect() };
    let to_bitvec = |bits: &[bool]| -> BitVec {
        let mut v = BitVec::new();
        bits.iter().for_each(|&b| v.push_bit(b));
        v
    };
    match m {
        Mutation::Set(i, b) if n > 0 => {
            let i = i % n;
            bv.set(i, *b);
            reference[i] = *b;
        }
        Mutation::Set(..) => {}
        Mutation::Push(b) => {
            bv.push_bit(*b);
            reference.push(*b);
        }
        Mutation::Pop => {
            assert_eq!(bv.pop_bit(), reference.pop());
        }
        Mutation::And(mask) => {
            let m = from_mask(mask);
            bv.bit_and_into(&to_bitvec(&m));
            reference.iter_mut().zip(&m).for_each(|(r, &x)| *r &= x);
        }
        Mutation::Or(mask) => {
            let m = from_mask(mask);
            bv.bit_or_into(&to_bitvec(&m));
            reference.iter_mut().zip(&m).for_each(|(r, &x)| *r |= x);
        }
        Mutation::Xor(mask) => {
            let m = from_mask(mask);
            bv.bit_xor_into(&to_bitvec(&m));
            reference.iter_mut().zip(&m).for_each(|(r, &x)| *r ^= x);
        }
        Mutation::Not => {
            bv.not_into();
            reference.iter_mut().for_each(|r| *r = !*r);
        }
        Mutation::ShiftLeft(k) => {
            bv.shift_left(*k);
            let old = reference.clone();
            for (i, r) in reference.iter_mut().enumerate() {
                *r = i >= *k && old[i - *k];
            }
        }
        Mutation::ShiftRight(k) => {
            bv.shift_right(*k);
            let old = reference.clone();
            for (i, r) in reference.iter_mut().enumerate() {
                *r = old.get(i + *k).copied().unwrap_or(false);
            }
        }
        Mutation::Resize(len, fill) => {
            bv.resize(*len, *fill);
            reference.resize(*len, *fill);
        }
        Mutation::Clear => {
            bv.clear();
            reference.clear();
        }
        Mutation::BitSliceMutFill(a, b) => {
            let (lo, hi) = (a.min(b).min(&n), a.max(b).min(&n));
            let (lo, hi) = (*lo, *hi);
            let mut view = bv.bit_slice_mut(lo..hi);
            for i in 0..hi - lo {
                view.set(i, true);
            }
            reference[lo..hi].iter_mut().for_each(|r| *r = true);
        }
        Mutation::BitReverse => {
            let p = if n == 0 {
                0
            } else {
                1usize << (usize::BITS - 1 - n.leading_zeros())
            };
            bv.bit_reverse_into(p);
            let width = p.trailing_zeros();
            let old = reference.clone();
            for (i, &bit) in old[..p].iter().enumerate().filter(|_| p > 1) {
                reference[i.reverse_bits() >> (usize::BITS - width)] = bit;
            }
        }
        Mutation::Polar => {
            let p = if n == 0 {
                0
            } else {
                1usize << (usize::BITS - 1 - n.leading_zeros())
            };
            if p > 1 {
                bv.polar_transform_into(p);
                for stride in
                    std::iter::successors(Some(1usize), |s| Some(s * 2)).take_while(|&s| s < p)
                {
                    for i in (0..p).step_by(2 * stride) {
                        for j in 0..stride {
                            reference[i + j + stride] ^= reference[i + j];
                        }
                    }
                }
            }
        }
    }
}

fn check_queries(bv: &BitVec, reference: &[bool]) -> Result<(), TestCaseError> {
    prop_assert_eq!(bv.len(), reference.len());
    let mut ones = 0usize;
    let mut positions = Vec::new();
    for (i, &b) in reference.iter().enumerate() {
        if b {
            ones += 1;
            positions.push(i);
        }
        prop_assert_eq!(bv.rank(i), ones, "rank({})", i);
    }
    for (k, &p) in positions.iter().enumerate() {
        prop_assert_eq!(bv.select(k), Some(p), "select({})", k);
    }
    prop_assert_eq!(bv.select(positions.len()), None);
    Ok(())
}

proptest! {
    #[test]
    fn rank_select_track_interleaved_mutations(
        start_len in prop_oneof![Just(0usize), Just(1), Just(63), Just(64), Just(65), 0usize..200],
        seed_bits in prop::collection::vec(any::<bool>(), 200),
        mutations in prop::collection::vec(mutation_strategy(), 1..12),
    ) {
        let mut reference: Vec<bool> = seed_bits[..start_len].to_vec();
        let mut bv = BitVec::new();
        reference.iter().for_each(|&b| bv.push_bit(b));

        check_queries(&bv, &reference)?;
        for m in &mutations {
            apply(&mut bv, &mut reference, m);
            check_queries(&bv, &reference)?;
        }
    }
}
