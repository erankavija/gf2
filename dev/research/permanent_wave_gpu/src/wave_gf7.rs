//! Stub for the F_7 permanent-shaped wave kernel comparison.

use crate::{DispatchResult, Unsupported};

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(
        "F_7 permanent-shaped wave kernel implementation has not landed",
    ))
}
