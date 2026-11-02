//! Configured permanent sampling, exact decisions, and resumable evidence.

mod config;
pub mod dataset;
mod decide;
mod evaluate;
pub mod runner;
pub mod validation;

pub use config::*;
pub use decide::*;
pub use evaluate::*;

/// Configuration, evaluation, or persistence failure.
#[derive(Debug)]
pub enum CampaignError {
    /// Invalid parameters or inconsistent evidence.
    Invalid(String),
    /// Filesystem failure.
    Io(std::io::Error),
    /// JSON failure.
    Json(serde_json::Error),
    /// Checkpoint failure.
    Checkpoint(gf2_sim::checkpoint::CheckpointLoadError),
    /// A requested backend cannot run on this build or host.
    BackendUnavailable,
}
impl std::fmt::Display for CampaignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(s) => f.write_str(s),
            Self::Io(e) => e.fmt(f),
            Self::Json(e) => e.fmt(f),
            Self::Checkpoint(e) => e.fmt(f),
            Self::BackendUnavailable => f.write_str("backend unavailable"),
        }
    }
}
impl std::error::Error for CampaignError {}
macro_rules! from_error {
    ($t:ty, $v:ident) => {
        impl From<$t> for CampaignError {
            fn from(e: $t) -> Self {
                Self::$v(e)
            }
        }
    };
}
from_error!(std::io::Error, Io);
from_error!(serde_json::Error, Json);
from_error!(gf2_sim::checkpoint::CheckpointLoadError, Checkpoint);
pub(crate) fn invalid<T>(message: impl Into<String>) -> Result<T, CampaignError> {
    Err(CampaignError::Invalid(message.into()))
}

pub(crate) fn guard_dispatch<T>(
    dispatch: impl FnOnce() -> Result<T, CampaignError>,
) -> Result<T, CampaignError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(dispatch)).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("unknown panic");
        invalid(format!("backend dispatch panicked: {message}"))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn dispatch_panics_become_reported_failures() {
        let result = super::guard_dispatch::<()>(|| panic!("kernel diagnostic"));
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("kernel diagnostic"));
    }
}
