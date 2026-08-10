//! Stubs for the F_7 permanent-shaped kernel candidates.

use crate::{fixtures::Fixture, DispatchResult, EvaluationResult, Unsupported};

const LOOKUP_TABLE_UNAVAILABLE: &str = "F_7 permanent-shaped lookup-table control has not landed";
const THREE_PLANE_UNAVAILABLE: &str = "F_7 permanent-shaped three-plane kernel has not landed";

/// Dispatch stub for the F_7 lookup-table arithmetic representation control.
pub(crate) fn lookup_table_control() -> DispatchResult {
    Err(Unsupported::new(LOOKUP_TABLE_UNAVAILABLE))
}

/// Dispatch stub for the F_7 permanent-shaped three-plane kernel.
pub(crate) fn three_plane() -> DispatchResult {
    Err(Unsupported::new(THREE_PLANE_UNAVAILABLE))
}

pub(crate) fn evaluate_lookup_table_control(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(LOOKUP_TABLE_UNAVAILABLE))
}

pub(crate) fn evaluate_three_plane(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(THREE_PLANE_UNAVAILABLE))
}
