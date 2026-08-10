//! Stub for the F_3 wave-cooperative Ryser control.

use crate::{fixtures::Fixture, DispatchResult, EvaluationResult, Unsupported};

const UNAVAILABLE: &str = "F_3 wave-cooperative kernel implementation has not landed";

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(UNAVAILABLE))
}

pub(crate) fn evaluate(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(UNAVAILABLE))
}
