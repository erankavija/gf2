//! Fresh-process default witness for the algebra-owned tuning section.

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_algebra::tuning::CHUNK_SUBSETS;

#[test]
fn first_algebra_access_uses_the_crate_owned_conservative_declaration() {
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::ConservativeDefault)
        .expect("fresh-process default witness succeeds");

    assert_eq!(result["chunk"], CHUNK_SUBSETS);
    assert_eq!(result["resolution"], "frozen-before-install");
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}
