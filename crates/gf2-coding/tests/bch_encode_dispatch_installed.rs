//! Guarded fresh-process witness that an installed profile moves the batch
//! entry points onto the table family, and a batch onto the fold overrides'
//! accumulator count and run length over a workspace whose construction read
//! no profile, without moving the bytes they write, at every worker count the
//! workload-selection contract declares.

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_coding::bch::encode::EncodeFamily;

#[test]
fn an_installed_profile_selects_the_table_family() {
    let result =
        fresh::fresh_tuning_process(fresh::FreshProcessCase::TableRemainderCompiled).unwrap();
    assert_eq!(result["family"], EncodeFamily::TableRemainder.name());
    assert_eq!(result["agrees_with_reference"], true);
    assert_eq!(result["resolution"], "installed");
}

#[test]
fn workspace_construction_reads_no_profile_and_a_batch_takes_the_fold_overrides() {
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::ClmulFoldOverrides).unwrap();
    assert_eq!(result["family"], EncodeFamily::ClmulFold.name());
    assert_eq!(result["agrees_with_reference"], true);
    assert_eq!(result["resolution"], "installed");
    let field = |name: &str, index: usize| result[name][index].as_u64().unwrap();
    let structural = result["structural_accumulators"].as_u64().unwrap();
    assert_eq!(field("built", 0), structural);
    assert_eq!(
        field("built", 1),
        result["structural_run_blocks"]
            .as_u64()
            .unwrap()
            .next_multiple_of(structural.max(1))
    );
    let accumulators = fresh::FOLD_ACCUMULATORS as u64;
    assert_ne!(structural, accumulators);
    assert_eq!(field("overridden", 0), accumulators);
    assert_eq!(
        field("overridden", 1),
        (fresh::FOLD_RUN_BLOCKS as u64).next_multiple_of(accumulators)
    );
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}
