//! Stub for the standalone F_7 three-plane Mersenne accumulator candidate.

use crate::{DispatchResult, Unsupported};

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(
        "F_7 three-plane Mersenne accumulator implementation has not landed",
    ))
}
