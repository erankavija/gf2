//! Stub for the F_5 byte-control and three-plane candidate comparison.

use crate::{DispatchResult, Unsupported};

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(
        "F_5 byte-control and three-plane implementations have not landed",
    ))
}
