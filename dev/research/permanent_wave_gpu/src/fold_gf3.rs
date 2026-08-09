//! Stub for the F_3 zero-mask sign-popcount fold candidate.

use crate::{DispatchResult, Unsupported};

pub(crate) fn run() -> DispatchResult {
    Err(Unsupported::new(
        "F_3 zero-mask sign-popcount fold implementation has not landed",
    ))
}
