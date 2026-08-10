//! Stub for the standalone F_7 three-plane Mersenne accumulator candidate.

use crate::{fixtures::Fixture, DispatchResult, EvaluationResult, Unsupported};

const UNAVAILABLE: &str = "F_7 three-plane Mersenne accumulator implementation has not landed";

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(UNAVAILABLE))
}

pub(crate) fn evaluate(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(UNAVAILABLE))
}
