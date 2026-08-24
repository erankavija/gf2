//! Conformance tests for canonical reliability index ordering.

use gf2_coding::llr::{Llr, ReliabilityPermutation};
use proptest::prelude::*;

#[test]
fn empty_and_singleton_inputs_have_identity_orders() {
    let empty = ReliabilityPermutation::from_magnitudes(&[]);
    assert_eq!(empty.ascending(), &[] as &[usize]);
    assert_eq!(empty.descending(), &[] as &[usize]);

    let singleton = ReliabilityPermutation::from_magnitudes(&[3.5]);
    assert_eq!(singleton.ascending(), &[0]);
    assert_eq!(singleton.descending(), &[0]);
}

#[test]
fn repeated_magnitudes_keep_original_index_ties_in_both_directions() {
    let order = ReliabilityPermutation::from_magnitudes(&[2.0, 1.0, 2.0, 0.0, 1.0]);

    assert_eq!(order.ascending(), &[3, 1, 4, 0, 2]);
    assert_eq!(order.descending(), &[0, 2, 1, 4, 3]);
}

#[test]
fn signed_zero_and_infinities_use_natural_numeric_order() {
    let order = ReliabilityPermutation::from_magnitudes(&[
        f32::INFINITY,
        -0.0,
        0.0,
        2.0,
        f32::NEG_INFINITY,
    ]);

    assert_eq!(order.ascending(), &[4, 1, 2, 3, 0]);
    assert_eq!(order.descending(), &[0, 3, 1, 2, 4]);
}

#[test]
#[should_panic(expected = "NaN")]
fn nan_magnitudes_are_rejected() {
    let _ = ReliabilityPermutation::from_magnitudes(&[1.0, f32::NAN]);
}

#[test]
fn llr_wrapper_orders_by_magnitude() {
    let llrs = [Llr::new(-2.0), Llr::new(1.0), Llr::new(-1.0), Llr::new(2.0)];
    let from_llrs = Llr::reliability_permutation(&llrs);
    let from_magnitudes = ReliabilityPermutation::from_magnitudes(&[2.0, 1.0, 1.0, 2.0]);

    assert_eq!(from_llrs.ascending(), from_magnitudes.ascending());
    assert_eq!(from_llrs.descending(), from_magnitudes.descending());
}

proptest! {
    #[test]
    fn permutation_is_a_total_order_for_all_non_nan_inputs(
        magnitudes in prop::collection::vec(
            any::<f32>().prop_filter("magnitude must not be NaN", |value| !value.is_nan()),
            0..32,
        )
    ) {
        let order = ReliabilityPermutation::from_magnitudes(&magnitudes);

        let mut expected_ascending: Vec<usize> = (0..magnitudes.len()).collect();
        expected_ascending.sort_by(|&left, &right| {
            magnitudes[left]
                .partial_cmp(&magnitudes[right])
                .expect("the strategy excludes NaN")
                .then(left.cmp(&right))
        });

        let mut expected_descending: Vec<usize> = (0..magnitudes.len()).collect();
        expected_descending.sort_by(|&left, &right| {
            magnitudes[right]
                .partial_cmp(&magnitudes[left])
                .expect("the strategy excludes NaN")
                .then(left.cmp(&right))
        });

        prop_assert_eq!(order.ascending(), expected_ascending.as_slice());
        prop_assert_eq!(order.descending(), expected_descending.as_slice());

        let mut seen = vec![false; magnitudes.len()];
        for &index in order.ascending() {
            prop_assert!(!seen[index]);
            seen[index] = true;
        }
        prop_assert!(seen.into_iter().all(|was_seen| was_seen));
    }
}
