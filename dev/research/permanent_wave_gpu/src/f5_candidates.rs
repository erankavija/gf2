//! Stubs for the F_5 byte-control and three-plane candidate paths.

use crate::{DispatchResult, Unsupported};

/// Dispatch stub for the F_5 byte-oriented modular arithmetic control.
pub(crate) fn byte_control() -> DispatchResult {
    Err(Unsupported::new(
        "F_5 byte-oriented modular arithmetic control has not landed",
    ))
}

/// Dispatch stub for the F_5 canonical three-plane accumulator.
pub(crate) fn three_plane() -> DispatchResult {
    Err(Unsupported::new(
        "F_5 canonical three-plane accumulator has not landed",
    ))
}
