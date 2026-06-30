//! Tracing / observability setup for campaign runs.
//!
//! [`install_campaign_subscriber`] is the public entry point the DVB-T2 AWGN
//! campaign binary calls to install a JSON-lines tracing subscriber whose
//! events land in the file named by [`PipelineConfig::tracing_log_path`].
//! Its role mirrors `setup_tracing_guard` in `gf2_coding::simulation`
//! (design doc §12), with one deliberate difference:
//!
//! **The subscriber is installed as the PROCESS-GLOBAL default**
//! (`tracing::subscriber::set_global_default`), not a thread-local one.
//! The `gf2-sim` sweep runs its frame loops inside rayon-pool workers and
//! `thread::scope` helper threads (see `executor/drain.rs` /
//! `executor/hybrid_core.rs`); a thread-local default (`set_default`) would
//! cover only the installing thread, silently dropping every event emitted
//! from a worker — including the per-frame `campaign_heartbeat` events that
//! are the whole point of the monitoring channel. A binary owns its process,
//! so one global subscriber is the correct shape.
//!
//! Global-default semantics: it can be set **once** per process. A second
//! call (with a `tracing_log_path` set) returns the
//! [`SetGlobalDefaultError`] from `tracing`, which the caller must surface
//! as a clear error. There is no uninstall; the subscriber lives for the
//! remainder of the process.

use std::sync::Mutex;

pub use tracing::subscriber::SetGlobalDefaultError;

use crate::config::PipelineConfig;

/// Installs the campaign tracing subscriber as the process-global default.
///
/// While the process lives, campaign tracing events (`campaign_start`,
/// `campaign_heartbeat`, `snr_point_completed`, plus any `tracing::warn!`
/// the library emits) are routed to the JSON-lines sink named by
/// [`PipelineConfig::tracing_log_path`]. Each emitted tracing event is
/// written as one self-contained JSON object followed by a newline (`\n`).
///
/// The subscriber is installed via
/// [`tracing::subscriber::set_global_default`], so it covers **all
/// threads** — in particular the rayon-pool workers and double-buffer
/// helper threads the sweep executor runs frames on. It can be installed
/// only once per process and is never uninstalled.
///
/// When `config.tracing_log_path` is `None` the function is a no-op and
/// returns `Ok(())`: nothing is installed.
///
/// When the file cannot be opened, a warning is printed to stderr and the
/// function returns `Ok(())` with tracing disabled (matching the legacy
/// `setup_tracing_guard` behaviour: a broken log sink degrades the run's
/// observability, it does not abort the campaign).
///
/// # Arguments
///
/// * `config` — the live pipeline configuration; its `tracing_log_path`
///   selects the sink.
///
/// # Errors
///
/// Returns the [`SetGlobalDefaultError`] from `tracing` if a global default
/// subscriber has already been set in this process (the global default can
/// only be set once).
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
/// use gf2_sim::observability::install_campaign_subscriber;
/// use gf2_sim::PipelineConfig;
///
/// let cfg = PipelineConfig {
///     seed: 0,
///     esn0_db_points: vec![5.0],
///     target_errors: 100,
///     max_frames: 1000,
///     heartbeat_every_frames: 0,
///     checkpoint_dir: None,
///     tracing_log_path: None,
///     parallelism: NonZeroUsize::new(1).unwrap(),
///     gpu_enabled: false,
///     strict_gpu: false,
///     diagnostic_dump_dir: None,
///     inject_gpu_oom_modulus: None,
/// };
/// // `tracing_log_path: None` → no-op, Ok(()).
/// install_campaign_subscriber(&cfg).unwrap();
/// ```
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
        // tracing_log_path = None → early return Ok(()) before touching global state.
        let cfg = minimal_config(None);
        let result = install_campaign_subscriber(&cfg);
        assert!(result.is_ok(), "None path must return Ok: {result:?}");
    }

    #[test]
    fn test_install_campaign_subscriber_bad_path_is_noop() {
        // A path in a nonexistent directory → file::open Err → Ok(()) degraded.
        let cfg = minimal_config(Some(std::path::PathBuf::from(
            "/this/path/cannot/exist/trace_gf2sim.json",
        )));
        // The file-open error returns Ok(()) before set_global_default is called.
        let result = install_campaign_subscriber(&cfg);
        assert!(
            result.is_ok(),
            "bad-path must return Ok (degraded): {result:?}"
        );
    }
}
