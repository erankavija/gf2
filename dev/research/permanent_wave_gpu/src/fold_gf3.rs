//! Stub for the F_3 zero-mask sign-popcount fold candidate.

use crate::{fixtures::Fixture, DispatchResult, EvaluationResult, Unsupported};

const UNAVAILABLE: &str = "F_3 zero-mask sign-popcount fold implementation has not landed";

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(UNAVAILABLE))
}

pub(crate) fn evaluate(_fixture: &Fixture) -> EvaluationResult {
    Err(Unsupported::new(UNAVAILABLE))
}
