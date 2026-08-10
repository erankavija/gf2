//! Stubs for the F_5 byte-control and three-plane candidate paths.

#[cfg(feature = "fixture-oracle")]
use crate::{fixtures::Fixture, EvaluationResult};
use crate::{DispatchResult, Unsupported};

const BYTE_CONTROL_UNAVAILABLE: &str =
    "F_5 byte-oriented modular arithmetic control has not landed";
const THREE_PLANE_UNAVAILABLE: &str = "F_5 canonical three-plane accumulator has not landed";

/// Dispatch stub for the F_5 byte-oriented modular arithmetic control.
pub(crate) fn byte_control() -> DispatchResult {
    Err(Unsupported::new(BYTE_CONTROL_UNAVAILABLE))
}

/// Dispatch stub for the F_5 canonical three-plane accumulator.
pub(crate) fn three_plane() -> DispatchResult {
    Err(Unsupported::new(THREE_PLANE_UNAVAILABLE))
}

#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate_byte_control(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(BYTE_CONTROL_UNAVAILABLE))
}

#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate_three_plane(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(THREE_PLANE_UNAVAILABLE))
}
