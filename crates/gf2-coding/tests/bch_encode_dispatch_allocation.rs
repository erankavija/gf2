//! Guarded fresh-process witness that selecting the table family reaches no
//! allocator on the workspace batch path, first call included.
//!
//! The conservative profile keeps every code on the scalar reference, so the
//! in-crate allocation witnesses of `8f68699b` never exercise a family that
//! reads precomputed state. This case installs a profile that selects one and
//! watches the same buffer shapes.

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_coding::bch::encode::EncodeFamily;

#[test]
fn a_selected_table_family_allocates_nothing_over_a_workspace() {
    let result =
        fresh::fresh_tuning_process(fresh::FreshProcessCase::TableRemainderAllocation).unwrap();
    assert_eq!(result["family"], EncodeFamily::TableRemainder.name());
    assert_eq!(result["resolution"], "installed");
    assert_eq!(
        result["workspace_shape_stable_from_creation"], true,
        "a workspace is prepared for the family when it is built, so its \
         first batch grows no buffer"
    );
    assert_eq!(
        result["scratch_shape_stable_after_first_batch"], true,
        "the thread scratch of the allocating batch path sizes once and is \
         reused afterwards"
    );
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}
