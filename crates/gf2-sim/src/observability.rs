//! Tracing setup for campaign runs.
//!
//! [`install_campaign_subscriber`] installs a JSON-lines subscriber as the
//! process-global default: the sweep emits events from rayon workers and
//! helper threads, which a thread-local default does not cover. The global
//! default is set once per process and never uninstalled.

use std::sync::Mutex;

pub use tracing::subscriber::SetGlobalDefaultError;

use crate::config::PipelineConfig;

/// Installs the campaign tracing subscriber as the process-global default.
///
/// Each tracing event is written as one JSON object per line to
/// [`PipelineConfig::tracing_log_path`]. When that path is `None`, nothing is
/// installed. When the file cannot be opened, a warning goes to stderr and
/// the function returns `Ok(())` with tracing disabled.
///
/// # Errors
///
/// Returns the [`SetGlobalDefaultError`] from `tracing` if a global default
/// subscriber is already set in this process.
pub fn install_campaign_subscriber(config: &PipelineConfig) -> Result<(), SetGlobalDefaultError> {
    use tracing_subscriber::{fmt, prelude::*, registry};

    let Some(path) = config.tracing_log_path.as_ref() else {
        return Ok(());
    };

    let file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "Warning: cannot open tracing log {} — tracing disabled: {e}",
                path.display()
            );
            return Ok(());
        }
    };

    let layer = fmt::layer()
        .json()
        .with_writer(Mutex::new(file))
        .with_span_list(false)
        .with_current_span(true);
    let subscriber = registry().with(layer);
    tracing::subscriber::set_global_default(subscriber)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    fn minimal_config(path: Option<std::path::PathBuf>) -> PipelineConfig {
        PipelineConfig {
            seed: 0,
            esn0_db_points: vec![],
            target_errors: 0,
            max_frames: 0,
            heartbeat_every_frames: 0,
            checkpoint_dir: None,
            tracing_log_path: path,
            parallelism: NonZeroUsize::new(1).unwrap(),
            gpu_enabled: false,
            strict_gpu: false,
            diagnostic_dump_dir: None,
            inject_gpu_oom_modulus: None,
        }
    }

    #[test]
    fn test_install_campaign_subscriber_noop_when_no_path() {
        let cfg = minimal_config(None);
        let result = install_campaign_subscriber(&cfg);
        assert!(result.is_ok(), "None path must return Ok: {result:?}");
    }

    #[test]
    fn test_install_campaign_subscriber_bad_path_is_noop() {
        let cfg = minimal_config(Some(std::path::PathBuf::from(
            "/this/path/cannot/exist/trace_gf2sim.json",
        )));
        let result = install_campaign_subscriber(&cfg);
        assert!(
            result.is_ok(),
            "bad-path must return Ok (degraded): {result:?}"
        );
    }
}
