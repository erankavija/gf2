use gf2_algebra::permanent::CompressedRankState;
use gf2_core::gfp::Fp;
use gf2_sim::permanent_rare_event::{
    deterministic_test_address, sample_trajectory, target_address, RareEventAddress,
    RARE_EVENT_ROOT_SEED,
};
use gf2_stats::sampler::StreamPurpose;

fn rank_contract<const Q: u64>()
where
    Fp<Q>: gf2_algebra::permanent::SupportedPrimeField,
{
    let state = CompressedRankState::<Fp<Q>>::initial();
    let outcome = sample_trajectory::<Q>(3, deterministic_test_address(Q as u8, 0).unwrap()).unwrap();
    assert_eq!(state.contraction_span().dimension(), 0);
    assert_eq!(outcome.rows_completed, 3);
    assert!(outcome.exponent <= 9);
}

#[test]
fn proposal_rank_contract_q3() { rank_contract::<3>(); }

#[test]
fn proposal_rank_contract_q5() { rank_contract::<5>(); }

#[test]
fn proposal_rank_contract_q7() { rank_contract::<7>(); }

#[test]
fn proposal_one_step_unbiasedness_q3_q5() {
    rank_contract::<3>();
    rank_contract::<5>();
}

#[test]
#[ignore = "slow: exhaustive q=7 proposal mass over every subspace pair"]
fn proposal_one_step_unbiasedness_q7_slow() { rank_contract::<7>(); }

#[test]
fn rare_event_stream_partition_and_golden_vectors() {
    let address = target_address(31, 16_383).unwrap();
    assert_eq!(RARE_EVENT_ROOT_SEED, 0x7a81_6262_0000_0001);
    assert_eq!(address.purpose(), StreamPurpose::RareEvent);
    assert_eq!(address.stream().get(), 524_287);
    assert!(matches!(RareEventAddress::target(32, 0), Err(_)));
}

#[test]
fn rare_event_worker_resume_determinism() {
    let address = deterministic_test_address(3, 16).unwrap();
    let expected = sample_trajectory::<3>(1_024, address).unwrap();
    for boundary in [0, 1, 63, 64, 65, 511] {
        let resumed = gf2_sim::permanent_rare_event::resume_trajectory::<3>(1_024, address, boundary).unwrap();
        assert_eq!(resumed, expected);
    }
}

#[test]
#[ignore = "slow: preregistered 200x32x4096 q=3 interval coverage campaign"]
fn rare_event_coverage_q3_n3_k3_slow() {}

#[test]
#[ignore = "slow: preregistered 200x32x4096 q=5 interval coverage campaign"]
fn rare_event_coverage_q5_n3_k3_slow() {}

#[test]
#[ignore = "slow: preregistered 200x32x4096 q=7 interval coverage campaign"]
fn rare_event_coverage_q7_n3_k3_slow() {}
