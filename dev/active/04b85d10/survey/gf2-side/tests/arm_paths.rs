//! Behavioural checks of the consumer harness that need no timing and no
//! lock: the arm-path vocabulary the plan derivation writes into arm
//! environments, the bank rule the cache-state contract implies, and the
//! preparation of the two arms the allocation cell compares.

use std::collections::BTreeMap;

use consumer_profile_gf2_side::{bank_count, prepare, ArmPath, Case};
use tuning_campaign_support::timing::FIXTURE_BANKS;

/// Every spelling `build-plan.py` emits decodes, and an unknown one is
/// refused before it can produce samples.
#[test]
fn arm_paths_round_trip_the_plan_vocabulary() {
    for spelling in [
        "current",
        "ops-dispatched",
        "ops-resolved",
        "scalar-backend",
        "simd-backend",
        "count-ones",
        "find-first-one",
        "family-poly-remainder-scalar",
        "family-table-remainder",
        "family-bitslice-interleaved",
        "family-clmul-fold",
        "caller-buffer",
        "transpose-scalar",
        "transpose-detected",
    ] {
        assert!(ArmPath::parse(spelling).is_ok(), "{spelling} must decode");
    }
    assert!(ArmPath::parse("family-fold").is_err());
    assert!(ArmPath::parse("").is_err());
}

/// A streaming cell rotates through every fixture bank; every other cache
/// state reads one.
#[test]
fn streaming_cells_use_every_bank() {
    assert_eq!(bank_count("streaming", FIXTURE_BANKS), FIXTURE_BANKS);
    assert_eq!(bank_count("warm", FIXTURE_BANKS), 1);
    assert_eq!(bank_count("cold", FIXTURE_BANKS), 1);
}

fn case(workload: &str, size: &[(&str, u64)], seed: u64) -> Case {
    Case {
        workload: workload.to_owned(),
        size: size
            .iter()
            .map(|(key, value)| ((*key).to_owned(), *value))
            .collect::<BTreeMap<_, _>>(),
        seed,
    }
}

/// The allocation cell's two arms prepare from the same case and report the
/// routes the receipts record; a path the workload does not serve is refused.
#[test]
fn allocation_cell_arms_prepare_and_name_their_routes() {
    let bch = case(
        "bch-encode-batch-alloc",
        &[("degree", 8), ("batch", 4)],
        304,
    );
    let allocating =
        prepare(&bch, ArmPath::Current, "warm", FIXTURE_BANKS).expect("allocating arm");
    assert!(allocating.selected_path.starts_with("current-allocating/"));
    let caller = prepare(&bch, ArmPath::CallerBuffer, "warm", FIXTURE_BANKS).expect("caller arm");
    assert!(caller.selected_path.starts_with("caller-buffer/"));
    assert!(prepare(&bch, ArmPath::TransposeScalar, "warm", FIXTURE_BANKS).is_err());
}

/// Both arms of a timed cell leave a non-zero sink after one call, so the
/// timed body is never dead code.
#[test]
fn timed_bodies_produce_a_live_result() {
    let xor = case("row-xor", &[("rows", 64), ("words", 4)], 101);
    let mut prepared =
        prepare(&xor, ArmPath::OpsDispatched, "warm", FIXTURE_BANKS).expect("row xor");
    prepared.run(0);
    let first = prepared.sink();
    prepared.run(0);
    assert_ne!(first, prepared.sink(), "the XOR pair alternates the bank");
}
