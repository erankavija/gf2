//! Tracing / observability setup for campaign runs.
//!
//! [`install_campaign_subscriber`] is the public entry point the DVB-T2 AWGN
//! campaign binary calls to install a JSON-lines tracing subscriber whose
//! events land in the file named by [`PipelineConfig::tracing_log_path`].
//! Its effect mirrors `setup_tracing_guard` in `gf2_coding::simulation`
//! (design doc §12).
//!
//! The returned [`CampaignSubscriberGuard`] is an RAII wrapper that drops
//! the `tracing::subscriber::DefaultGuard`, restoring the previous subscriber
//! when the campaign binary exits or an early return unwinds the frame.
//!
//! When `tracing_log_path` is `None` the function is a no-op and the returned
//! guard holds nothing.

use std::sync::Mutex;

use crate::config::PipelineConfig;

/// RAII guard returned by [`install_campaign_subscriber`].
///
/// Dropping the guard uninstalls whatever subscriber was installed, matching
/// the `DefaultGuard` semantics of the legacy `setup_tracing_guard`.
#[derive(Debug)]
#[must_use = "dropping the guard immediately uninstalls the campaign subscriber"]
pub struct CampaignSubscriberGuard {
    _guard: Option<tracing::subscriber::DefaultGuard>,
}

impl Drop for CampaignSubscriberGuard {
    fn drop(&mut self) {
        // `_guard` drops here, restoring the previous subscriber (or the
        // `NoSubscriber` default).  Explicit `drop` is not needed; the field
        // drop is sufficient and is the canonical `DefaultGuard` contract.
    }
}

/// Installs the campaign tracing subscriber and returns an RAII guard.
///
/// While the returned guard is alive, campaign tracing events
/// (`campaign_start`, `snr_completed`, `heartbeat`) are routed to the
/// JSON-lines sink named by [`PipelineConfig::tracing_log_path`].  Each
/// emitted tracing event is written as one self-contained JSON object
/// followed by a newline (`\n`).  Dropping the guard uninstalls the
/// subscriber and restores the previous one (or the no-op default).
///
/// When `config.tracing_log_path` is `None` the function is a no-op: nothing
/// is installed and the returned guard holds nothing.
///
/// # Arguments
///
/// * `config` — the live pipeline configuration; its `tracing_log_path`
///   selects the sink.
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
/// let _guard = install_campaign_subscriber(&cfg);
/// // tracing events are routed until `_guard` is dropped.
/// ```
///
/// # Must use
///
/// Returns the concrete [`CampaignSubscriberGuard`] (rather than `impl Drop`)
/// so the type's `#[must_use]` propagates to call sites: dropping the guard
/// immediately — `install_campaign_subscriber(&cfg);` — uninstalls the
/// subscriber and is almost certainly a bug.  Bind it to a live `let`.
pub fn install_campaign_subscriber(config: &PipelineConfig) -> CampaignSubscriberGuard {
    use tracing_subscriber::{fmt, prelude::*, registry};

    let Some(path) = config.tracing_log_path.as_ref() else {
        return CampaignSubscriberGuard { _guard: None };
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
            return CampaignSubscriberGuard { _guard: None };
        }
    };

    let layer = fmt::layer()
        .json()
        .with_writer(Mutex::new(file))
        .with_span_list(false)
        .with_current_span(true);
    let subscriber = registry().with(layer);
    CampaignSubscriberGuard {
        _guard: Some(subscriber.set_default()),
    }
}
