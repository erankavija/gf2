use std::collections::BTreeSet;

use gf2_algebra::permanent::{
    canonical_subspaces_for_test, CanonicalSubspace, CompressedRankState, SupportedPrimeField,
    Vector3,
};
use gf2_core::gfp::Fp;
use gf2_stats::sampler::{FieldOrder, MatrixAddress, StreamIndex, StreamPurpose};
use gf2_stats::weighted::{ExponentHistogram, WeightedRuns};
use permanent_statistics::rare_event::{
    checkpoint_trajectory, resume_trajectory, sample_trajectories_in_order, sample_trajectory,
    ProposalSupport,
};

fn address(q: FieldOrder, rows: usize, index: u64) -> MatrixAddress {
    MatrixAddress::new(
        42,
        q,
        rows,
        StreamPurpose::RareEvent,
        StreamIndex::new(index).unwrap(),
    )
}

fn vector<F: SupportedPrimeField>(residues: [u8; 3]) -> Vector3<F> {
    Vector3::from_residues(residues).expect("test residues are canonical")
}

fn state_with_contraction_rank<F: SupportedPrimeField>(rank: usize) -> CompressedRankState<F> {
    let coordinate_vectors = [vector([1, 0, 0]), vector([0, 1, 0]), vector([0, 0, 1])];
    CompressedRankState::from_subspaces_for_test(
        CanonicalSubspace::zero(),
        CanonicalSubspace::from_vectors(&coordinate_vectors[..rank]),
    )
}

fn rank_contract<F: SupportedPrimeField>() {
    let q = usize::from(F::ORDER);
    for rank in 0..=3 {
        let state = state_with_contraction_rank::<F>(rank);
        let support = ProposalSupport::for_state(&state);
        assert_eq!(support.likelihood_exponent(), rank as u8);
        assert_eq!(support.rows().len(), q.pow((3 - rank) as u32));
        assert_eq!(
            support
                .rows()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            support.rows().len()
        );
        for &row in support.rows() {
            assert!(state.is_admissible(row));
            assert!(row.residues()[..rank].iter().all(|&residue| residue == 0));
            assert!(state.successor(row).is_some());
        }
        assert_eq!(support.rows().len() * q.pow(rank as u32), q.pow(3));
    }
}

#[test]
fn proposal_rank_contract_q3() {
    rank_contract::<Fp<3>>();
}

#[test]
fn proposal_rank_contract_q5() {
    rank_contract::<Fp<5>>();
}

#[test]
fn proposal_rank_contract_q7() {
    rank_contract::<Fp<7>>();
}

fn one_step_unbiasedness<F: SupportedPrimeField>() {
    let q = usize::from(F::ORDER);
    let subspaces = canonical_subspaces_for_test::<F>();
    for &row_span in &subspaces {
        for &contraction_span in &subspaces {
            let state = CompressedRankState::from_subspaces_for_test(row_span, contraction_span);
            let support = ProposalSupport::for_state(&state);
            let rank = contraction_span.dimension();
            assert_eq!(support.rows().len() * q.pow(rank as u32), q.pow(3));
            assert!(support
                .rows()
                .iter()
                .all(|&row| state.successor(row).is_some()));
        }
    }
}

#[test]
fn proposal_one_step_unbiasedness_q3_q5() {
    one_step_unbiasedness::<Fp<3>>();
    one_step_unbiasedness::<Fp<5>>();
}

#[test]
#[ignore = "slow: exhaustive q=7 proposal mass over every subspace pair"]
fn proposal_one_step_unbiasedness_q7_slow() {
    one_step_unbiasedness::<Fp<7>>();
}

#[test]
fn rare_event_worker_resume_determinism() {
    let addresses: Vec<_> = (16..32)
        .map(|case| address(FieldOrder::F3, 1_024, case))
        .collect();
    let expected = sample_trajectories_in_order::<3>(1_024, &addresses, 1).unwrap();
    for workers in [2, 7] {
        assert_eq!(
            sample_trajectories_in_order::<3>(1_024, &addresses, workers).unwrap(),
            expected
        );
    }
    for (address, outcome) in addresses.into_iter().zip(expected) {
        for boundary in [0, 1, 63, 64, 65, 511] {
            let checkpoint = checkpoint_trajectory::<3>(1_024, address, boundary).unwrap();
            assert_eq!(checkpoint.completed_prefix, boundary);
            if boundary == 0 {
                assert_eq!(checkpoint.exponent, 0);
            } else {
                assert_ne!(checkpoint.state, [0; 22]);
            }
            assert_eq!(
                resume_trajectory::<3>(1_024, address, boundary).unwrap(),
                outcome
            );
        }
    }
}

// At n=3 these rational anchors count uniform three-column matrices exactly.
// Exhaustive proposal expectation uses the same likelihood increments as the
// sampler, with integer rescaling rather than a Monte Carlo tolerance.
fn exact_expectation<F: SupportedPrimeField>(
    state: CompressedRankState<F>,
    remaining: usize,
    exponent: u32,
    proposal_exponent: u32,
    counts: &mut u64,
) {
    if remaining == 0 {
        assert_eq!(exponent + proposal_exponent, 9);
        *counts += 1;
        return;
    }
    let support = ProposalSupport::for_state(&state);
    let rank = u32::from(support.likelihood_exponent());
    for &row in support.rows() {
        exact_expectation(
            state.successor(row).unwrap(),
            remaining - 1,
            exponent + rank,
            proposal_exponent + 3 - rank,
            counts,
        );
    }
}

fn unbiasedness<const Q: u64>(numerator: u64, denominator: u64)
where
    Fp<Q>: SupportedPrimeField,
{
    let mut count = 0;
    exact_expectation::<Fp<Q>>(CompressedRankState::initial(), 3, 0, 0, &mut count);
    assert_eq!(count * denominator, numerator * Q.pow(9));
}

#[test]
fn likelihood_is_unbiased_against_exact_q3_case() {
    unbiasedness::<3>(907, 2_187);
}

#[test]
fn likelihood_is_unbiased_against_exact_q5_case() {
    unbiasedness::<5>(17_581, 78_125);
}

#[test]
#[ignore = "slow: exhaustive q=7 three-row proposal expectation"]
fn likelihood_is_unbiased_against_exact_q7_case() {
    unbiasedness::<7>(126_295, 823_543);
}

fn sampled_mean<const Q: u64>(q: FieldOrder, numerator: u64, denominator: u64)
where
    Fp<Q>: SupportedPrimeField,
{
    let mut histograms = Vec::new();
    for run in 0..16 {
        let mut histogram = ExponentHistogram::new(Q as u32, 9).unwrap();
        for trajectory in 0..1_024 {
            let result =
                sample_trajectory::<Q>(3, address(q, 3, run * 1_024 + trajectory)).unwrap();
            histogram.record(result.exponent).unwrap();
        }
        histograms.push(histogram);
    }
    let summary = WeightedRuns::from_histograms(histograms).unwrap();
    assert!(summary
        .student_interval(6_u8, 1_u8)
        .unwrap()
        .contains_exact(numerator, denominator)
        .unwrap());
}

#[test]
#[ignore = "slow: statistical check of sampled means against exact three-row cases"]
fn sampled_likelihood_means_agree_with_exact_cases() {
    sampled_mean::<3>(FieldOrder::F3, 907, 2_187);
    sampled_mean::<5>(FieldOrder::F5, 17_581, 78_125);
    sampled_mean::<7>(FieldOrder::F7, 126_295, 823_543);
}
