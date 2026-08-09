//! Stub for the F_3 wave-cooperative Ryser control.

use crate::{DispatchResult, Unsupported};

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(
        "F_3 wave-cooperative kernel implementation has not landed",
    ))
}
