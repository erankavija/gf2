use std::collections::BTreeSet;

use gf2_algebra::permanent::{
    canonical_subspaces_for_test, CanonicalSubspace, CompressedRankState, SupportedPrimeField,
    Vector3,
};
use gf2_core::gfp::Fp;
use gf2_sim::permanent_rare_event::artifact::sha256_hex;
use gf2_sim::permanent_rare_event::{
    checkpoint_trajectory, coverage_address, deterministic_test_address, resume_trajectory,
    sample_trajectories_in_order, sample_trajectory, target_address, ProposalSupport,
    COVERAGE_INDEX_START, COVERAGE_REPLICATES, COVERAGE_RUNS, COVERAGE_TRAJECTORIES_PER_RUN,
    RARE_EVENT_PURPOSE_TAG, RARE_EVENT_ROOT_SEED, RESERVED_INDEX_START, TARGET_RUNS,
    TARGET_TRAJECTORIES_PER_RUN, TEST_INDEX_START,
};
use gf2_stats::sampler::{MatrixAddress, StreamIndex, StreamPurpose};
use gf2_stats::weighted::{ExponentHistogram, WeightedRuns};

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

fn seed_words(address: MatrixAddress) -> [u64; 4] {
    let seed = address.seed();
    std::array::from_fn(|word| u64::from_le_bytes(seed[word * 8..word * 8 + 8].try_into().unwrap()))
}

fn append_golden_trajectory<const Q: u64>(bytes: &mut Vec<u8>, address: MatrixAddress)
where
    Fp<Q>: SupportedPrimeField,
{
    let outcome = sample_trajectory::<Q>(3, address).unwrap();
    bytes.extend_from_slice(&address.seed());
    bytes.extend_from_slice(&outcome.exponent.to_le_bytes());
    bytes.extend_from_slice(&(outcome.rows_completed as u64).to_le_bytes());
    bytes.extend_from_slice(&outcome.stream_index.to_le_bytes());
    bytes.extend_from_slice(&outcome.terminal_state);
}

#[test]
fn rare_event_stream_partition_and_golden_vectors() {
    let first_target = target_address(0, 0).unwrap();
    let last_target = target_address(TARGET_RUNS - 1, TARGET_TRAJECTORIES_PER_RUN - 1).unwrap();
    assert_eq!(first_target.stream().get(), 0);
    assert_eq!(last_target.stream().get(), 524_287);
    assert!(target_address(TARGET_RUNS, 0).is_err());
    assert!(target_address(0, TARGET_TRAJECTORIES_PER_RUN).is_err());

    let first_coverage = coverage_address(3, 0, 0, 0).unwrap();
    let last_coverage = coverage_address(
        7,
        COVERAGE_REPLICATES - 1,
        COVERAGE_RUNS - 1,
        COVERAGE_TRAJECTORIES_PER_RUN - 1,
    )
    .unwrap();
    assert_eq!(first_coverage.stream().get(), COVERAGE_INDEX_START);
    assert_eq!(
        last_coverage.stream().get(),
        COVERAGE_INDEX_START + 26_214_399
    );
    assert!(coverage_address(3, COVERAGE_REPLICATES, 0, 0).is_err());

    let purposes = [
        StreamPurpose::Validation,
        StreamPurpose::Timing,
        StreamPurpose::CampaignCell,
        StreamPurpose::RareEvent,
    ];
    for (offset, purpose) in purposes.into_iter().enumerate() {
        let address = MatrixAddress::new(
            RARE_EVENT_ROOT_SEED,
            gf2_stats::sampler::FieldOrder::F3,
            3,
            purpose,
            StreamIndex::new(9).unwrap(),
        );
        assert_eq!(seed_words(address)[3], ((offset as u64 + 1) << 56) | 9);
    }

    let mut golden_seeds = BTreeSet::new();
    let mut golden_bytes = Vec::new();
    for (field_slot, q) in [3_u8, 5, 7].into_iter().enumerate() {
        for case in 0_u16..16 {
            let address = deterministic_test_address(q, 3, case).unwrap();
            assert_eq!(
                address.stream().get(),
                TEST_INDEX_START | ((field_slot as u64) << 16) | u64::from(case)
            );
            assert!(golden_seeds.insert(address.seed()));
            match q {
                3 => append_golden_trajectory::<3>(&mut golden_bytes, address),
                5 => append_golden_trajectory::<5>(&mut golden_bytes, address),
                7 => append_golden_trajectory::<7>(&mut golden_bytes, address),
                _ => unreachable!(),
            }
        }
    }
    assert_eq!(golden_seeds.len(), 48);
    assert_eq!(
        sha256_hex(&golden_bytes),
        "7d1520598f7d0cd7d634007c4d54aa9c68a521403cbdf99b7f2d0bda995c034a"
    );
    assert_eq!(
        seed_words(deterministic_test_address(3, 3, 0).unwrap()),
        [
            0x7a81_6262_0000_0001,
            3,
            3,
            (4_u64 << 56) | TEST_INDEX_START,
        ]
    );
    assert_eq!(RARE_EVENT_PURPOSE_TAG, 4);
    assert_eq!(RESERVED_INDEX_START, 3_u64 << 54);
}

#[test]
fn rare_event_worker_resume_determinism() {
    let addresses: Vec<_> = (16_u16..32)
        .map(|case| deterministic_test_address(3, 1_024, case).unwrap())
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

fn coverage_contract<const Q: u64>(anchor_numerator: u64, anchor_denominator: u64)
where
    Fp<Q>: SupportedPrimeField,
{
    let mut coverage_count = 0_u16;
    for replicate in 0..COVERAGE_REPLICATES {
        let mut runs = Vec::with_capacity(usize::from(COVERAGE_RUNS));
        for run in 0..COVERAGE_RUNS {
            let mut histogram = ExponentHistogram::new(Q as u32, 9).unwrap();
            for trajectory in 0..COVERAGE_TRAJECTORIES_PER_RUN {
                let address = coverage_address(Q as u8, replicate, run, trajectory).unwrap();
                let outcome = sample_trajectory::<Q>(3, address).unwrap();
                histogram.record(outcome.exponent).unwrap();
            }
            runs.push(histogram);
        }
        let summary = WeightedRuns::from_histograms(runs).unwrap();
        let interval = summary
            .student_interval(1_019_756_723_u64, 500_000_000_u64)
            .unwrap();
        coverage_count += u16::from(
            interval
                .contains_exact(anchor_numerator, anchor_denominator)
                .unwrap(),
        );
    }
    assert!(
        coverage_count >= 180,
        "preregistered coverage count {coverage_count}/200 is below 180/200"
    );
}

#[test]
#[ignore = "slow: preregistered 200x32x4096 q=3 interval coverage campaign"]
fn rare_event_coverage_q3_n3_k3_slow() {
    coverage_contract::<3>(907, 2_187);
}

#[test]
#[ignore = "slow: preregistered 200x32x4096 q=5 interval coverage campaign"]
fn rare_event_coverage_q5_n3_k3_slow() {
    coverage_contract::<5>(17_581, 78_125);
}

#[test]
#[ignore = "slow: preregistered 200x32x4096 q=7 interval coverage campaign"]
fn rare_event_coverage_q7_n3_k3_slow() {
    coverage_contract::<7>(126_295, 823_543);
}
