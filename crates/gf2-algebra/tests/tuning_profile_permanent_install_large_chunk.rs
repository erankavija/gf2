//! Fresh-process installed algebra-section witness for a 4,000,000-subset chunk.

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

#[test]
fn installed_large_chunk_reaches_the_production_callee() {
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::Chunk4000000).unwrap();
    assert_eq!(result["chunk"], 4_000_000);
    assert_eq!(result["resolution"], "installed");
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}
