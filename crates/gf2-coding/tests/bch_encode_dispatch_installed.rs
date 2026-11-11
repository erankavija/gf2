//! Guarded fresh-process witness that an installed profile moves the batch
//! entry points onto the table family, and a fold run override onto its run
//! length, without moving the bytes they write, at every worker count the
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
fn an_installed_fold_run_override_sets_the_run_length() {
    let result =
        fresh::fresh_tuning_process(fresh::FreshProcessCase::ClmulFoldRunOverride).unwrap();
    assert_eq!(result["family"], EncodeFamily::ClmulFold.name());
    assert_eq!(result["agrees_with_reference"], true);
    assert_eq!(result["resolution"], "installed");
    let chunk = result["accumulators"].as_u64().unwrap().max(1);
    let run = result["run_blocks"].as_u64().unwrap();
    assert_eq!(run, (fresh::FOLD_RUN_BLOCKS as u64).next_multiple_of(chunk));
    let structural = result["structural_run_blocks"].as_u64().unwrap();
    assert_ne!(run, structural.next_multiple_of(chunk));
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}
