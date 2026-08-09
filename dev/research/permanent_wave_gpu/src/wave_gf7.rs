//! Stubs for the F_7 permanent-shaped kernel candidates.

use crate::{DispatchResult, Unsupported};

/// Dispatch stub for the F_7 lookup-table arithmetic representation control.
pub(crate) fn lookup_table_control() -> DispatchResult {
    Err(Unsupported::new(
        "F_7 permanent-shaped lookup-table control has not landed",
    ))
}

/// Dispatch stub for the F_7 permanent-shaped three-plane kernel.
pub(crate) fn three_plane() -> DispatchResult {
    Err(Unsupported::new(
        "F_7 permanent-shaped three-plane kernel has not landed",
    ))
}
