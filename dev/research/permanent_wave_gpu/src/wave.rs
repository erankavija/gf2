//! Stub for the F_3 wave-cooperative Ryser control.

#[cfg(feature = "fixture-oracle")]
use crate::{fixtures::Fixture, EvaluationResult};
use crate::{DispatchResult, Unsupported};

const UNAVAILABLE: &str = "F_3 wave-cooperative kernel implementation has not landed";

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(UNAVAILABLE))
}

#[cfg(feature = "fixture-oracle")]
pub(crate) fn evaluate(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(UNAVAILABLE))
}
