//! Guarded fresh-process witnesses that selecting a family beyond the
//! reference reaches no allocator on the workspace batch path, first call
//! included.
//!
//! The conservative profile keeps every code on the scalar reference, so the
//! in-crate allocation witnesses of `8f68699b` never exercise a family that
//! reads precomputed state. These cases install profiles that select one and
//! watch the same buffer shapes.

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_coding::bch::encode::EncodeFamily;

/// Asserts the shape claims of one allocation case.
fn assert_allocation_free(case: fresh::FreshProcessCase, family: EncodeFamily) {
    let result = fresh::fresh_tuning_process(case).unwrap();
    assert_eq!(result["family"], family.name());
    assert_eq!(result["resolution"], "installed");
    assert_eq!(
        result["workspace_shape_stable_from_creation"], true,
        "a workspace is prepared for {family} when it is built, so its first \
         batch grows no buffer"
    );
    assert_eq!(
        result["scratch_shape_stable_after_first_batch"], true,
        "the thread scratch of the allocating batch path sizes once and is \
         reused afterwards under {family}"
    );
}

#[test]
fn a_selected_table_family_allocates_nothing_over_a_workspace() {
    assert_allocation_free(
        fresh::FreshProcessCase::TableRemainderAllocation,
        EncodeFamily::TableRemainder,
    );
}

#[test]
fn a_selected_bit_sliced_family_allocates_nothing_over_a_workspace() {
    assert_allocation_free(
        fresh::FreshProcessCase::BitsliceInterleavedAllocation,
        EncodeFamily::BitsliceInterleaved,
    );
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}
