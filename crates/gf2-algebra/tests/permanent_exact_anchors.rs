//! Exact campaign anchors for permanent-zero fractions and determinant singularity.
//!
//! The enumeration side retains integer counts.  Floating point is used only
//! when the production Clopper-Pearson interval API receives the already exact
//! probability at its comparison boundary.

use gf2_algebra::permanent::{
    enumerate_permanent_zero_probability, permanent_ryser, ExactProbability,
};
use gf2_core::gfp::Fp;
use gf2_stats::intervals::clopper_pearson_interval;
use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose};

const VALIDATION_ROOT: u64 = 0x4453_4B2F_0000_0001;
const VALIDATION_STREAM: u64 = 0;
const VALIDATION_DRAWS_PER_CELL: u64 = 4_096;
const VALIDATION_INTERVAL_LEVEL: f64 = 0.999;

const ANCHOR_CELLS: &[(u64, usize)] = &[
    (3, 1),
    (3, 2),
    (3, 3),
    (3, 4),
    (5, 1),
    (5, 2),
    (5, 3),
    (7, 1),
    (7, 2),
    (7, 3),
];

fn expected_permanent_anchor(field_order: u64, dimension: usize) -> ExactProbability {
    match (field_order, dimension) {
        (3, 1) => ExactProbability::from_counts(1, 3),
        (3, 2) => ExactProbability::from_counts(33, 81),
        (3, 3) => ExactProbability::from_counts(8_163, 19_683),
        (3, 4) => ExactProbability::from_counts(17_116_353, 43_046_721),
        (5, 1) => ExactProbability::from_counts(1, 5),
        (5, 2) => ExactProbability::from_counts(145, 625),
        (5, 3) => ExactProbability::from_counts(439_525, 1_953_125),
        (7, 1) => ExactProbability::from_counts(1, 7),
        (7, 2) => ExactProbability::from_counts(385, 2_401),
        (7, 3) => ExactProbability::from_counts(6_188_455, 40_353_607),
        _ => panic!("unsupported permanent anchor q={field_order}, n={dimension}"),
    }
}

fn historical_order_three_reading(field_order: u64) -> ExactProbability {
    // Preserved from the historical receipt named in exact-anchors.csv.
    match field_order {
        3 => ExactProbability::from_counts(8_163, 19_683),
        5 => ExactProbability::from_counts(439_525, 1_953_125),
        7 => ExactProbability::from_counts(6_188_455, 40_353_607),
        _ => panic!("historical order-three readings cover q=3,5,7 only"),
    }
}

fn assert_permanent_anchor(field_order: u64, dimension: usize) {
    let observed = enumerate_permanent_zero_probability(field_order, dimension);
    assert_eq!(
        observed,
        expected_permanent_anchor(field_order, dimension),
        "exhaustive permanent anchor disagreed at q={field_order}, n={dimension}"
    );
    if dimension == 3 {
        assert_eq!(
            observed,
            historical_order_three_reading(field_order),
            "exhaustive order-three count disagreed with the preserved historical reading at q={field_order}"
        );
    }
}

#[test]
fn permanent_anchor_fast_q3_up_to_order_three() {
    for dimension in 1..=3 {
        assert_permanent_anchor(3, dimension);
    }
}

#[test]
fn permanent_anchor_fast_q5_up_to_order_three() {
    for dimension in 1..=3 {
        assert_permanent_anchor(5, dimension);
    }
}

#[test]
fn permanent_anchor_fast_q7_up_to_order_two() {
    for dimension in 1..=2 {
        assert_permanent_anchor(7, dimension);
    }
}

#[test]
#[ignore = "slow: exhaustive q=3, n=4 permanent anchor enumerates 3^16 matrices"]
fn permanent_anchor_slow_q3_order_four() {
    assert_permanent_anchor(3, 4);
}

#[test]
#[ignore = "slow: exhaustive q=7, n=3 permanent anchor enumerates 7^9 matrices"]
fn permanent_anchor_slow_q7_order_three() {
    assert_permanent_anchor(7, 3);
}

fn determinant_singular_probability(field_order: u64, dimension: usize) -> ExactProbability {
    let total = field_order.pow((dimension * dimension) as u32);
    let invertible = (0..dimension).fold(1_u64, |count, exponent| {
        count * (field_order.pow(dimension as u32) - field_order.pow(exponent as u32))
    });
    ExactProbability::from_counts(total - invertible, total)
}

fn determinant_is_singular(entries: &[u64], field_order: u64, dimension: usize) -> bool {
    let mut reduced = [0_u64; 9];
    reduced[..entries.len()].copy_from_slice(entries);
    for column in 0..dimension {
        let Some(pivot) = (column..dimension).find(|&row| reduced[row * dimension + column] != 0)
        else {
            return true;
        };
        if pivot != column {
            for index in 0..dimension {
                reduced.swap(column * dimension + index, pivot * dimension + index);
            }
        }
        let inverse = modular_power(
            reduced[column * dimension + column],
            field_order - 2,
            field_order,
        );
        for row in (column + 1)..dimension {
            let factor = reduced[row * dimension + column] * inverse % field_order;
            for index in column..dimension {
                let pivot_entry = reduced[column * dimension + index];
                let offset = row * dimension + index;
                reduced[offset] = (reduced[offset] + field_order
                    - factor * pivot_entry % field_order)
                    % field_order;
            }
        }
    }
    false
}

fn modular_power(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = result * base % modulus;
        }
        base = base * base % modulus;
        exponent >>= 1;
    }
    result
}

fn enumerate_determinant_singular_probability(
    field_order: u64,
    dimension: usize,
) -> ExactProbability {
    let matrix_count = field_order.pow((dimension * dimension) as u32);
    let mut entries = [0_u64; 9];
    let mut singular_count = 0_u64;
    for encoded in 0..matrix_count {
        let mut remaining = encoded;
        for entry in &mut entries[..(dimension * dimension)] {
            *entry = remaining % field_order;
            remaining /= field_order;
        }
        singular_count += u64::from(determinant_is_singular(
            &entries[..(dimension * dimension)],
            field_order,
            dimension,
        ));
    }
    ExactProbability::from_counts(singular_count, matrix_count)
}

#[test]
fn determinant_enumeration_matches_the_finite_n_formula() {
    for &(field_order, dimension) in &[(3, 1), (3, 2), (3, 3), (5, 2), (5, 3), (7, 2)] {
        assert_eq!(
            enumerate_determinant_singular_probability(field_order, dimension),
            determinant_singular_probability(field_order, dimension),
            "determinant formula disagreed at q={field_order}, n={dimension}"
        );
    }
}

fn field_order(field_order: u64) -> FieldOrder {
    match field_order {
        3 => FieldOrder::F3,
        5 => FieldOrder::F5,
        7 => FieldOrder::F7,
        _ => unreachable!("anchor cells use only the supported prime fields"),
    }
}

fn sampled_zero_count<const Q: u64>(field_order: FieldOrder, dimension: usize) -> u64 {
    let address = MatrixAddress::new(
        VALIDATION_ROOT,
        field_order,
        dimension,
        StreamPurpose::Validation,
        StreamIndex::new(VALIDATION_STREAM).expect("validation stream fits in 56 bits"),
    );
    let mut sampler = MatrixSampler::<Q>::new(address).expect("field order matches sampler");
    let mut entries = [Fp::<Q>::new(0); 16];
    let mut zero_count = 0_u64;
    for _ in 0..VALIDATION_DRAWS_PER_CELL {
        let matrix = &mut entries[..(dimension * dimension)];
        sampler.fill_next_matrix(matrix);
        zero_count += u64::from(permanent_ryser(matrix, dimension) == Fp::<Q>::new(0));
    }
    zero_count
}

#[test]
fn validation_stream_sampler_and_clopper_pearson_cover_every_exact_anchor() {
    for &(order, dimension) in ANCHOR_CELLS {
        let zero_count = match order {
            3 => sampled_zero_count::<3>(field_order(order), dimension),
            5 => sampled_zero_count::<5>(field_order(order), dimension),
            7 => sampled_zero_count::<7>(field_order(order), dimension),
            _ => unreachable!("anchor cells use only the supported prime fields"),
        };
        let exact = expected_permanent_anchor(order, dimension);
        let (lower, upper) = clopper_pearson_interval(
            zero_count,
            VALIDATION_DRAWS_PER_CELL,
            VALIDATION_INTERVAL_LEVEL,
        );
        let exact_as_f64 = exact.zero_count() as f64 / exact.matrix_count() as f64;
        assert!(
            lower <= exact_as_f64 && exact_as_f64 <= upper,
            "validation interval {lower:?}..={upper:?} excluded exact {}/{} at q={order}, n={dimension}; \
             root={VALIDATION_ROOT:#x}, purpose={:?}, stream={VALIDATION_STREAM}, draws={VALIDATION_DRAWS_PER_CELL}, level={VALIDATION_INTERVAL_LEVEL}",
            exact.zero_count(),
            exact.matrix_count(),
            StreamPurpose::Validation,
        );
    }
}
