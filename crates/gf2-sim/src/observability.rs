//! Tracing / observability setup for campaign runs.
//!
//! [`install_campaign_subscriber`] is the public entry point the DVB-T2 AWGN
//! campaign binary calls (once `bbf6b6ee` migrates it) to install a
//! JSON-lines tracing subscriber. Its effect mirrors the private
//! `setup_tracing_guard` in `gf2_coding::simulation` (design doc §12).
//!
//! This is the Phase A stub: it returns an RAII guard whose `Drop` uninstalls
//! the subscriber. The concrete subscriber wiring (JSON layer over the
//! configured `tracing_log_path`) is filled in by `bbf6b6ee`.

use crate::config::PipelineConfig;

/// RAII guard returned by [`install_campaign_subscriber`].
///
/// Dropping the guard uninstalls whatever subscriber was installed, matching
/// the `DefaultGuard` semantics of the legacy `setup_tracing_guard`.
#[derive(Debug)]
#[must_use = "dropping the guard immediately uninstalls the campaign subscriber"]
pub struct CampaignSubscriberGuard {
    // Phase A stub: holds no installed subscriber yet. `bbf6b6ee` replaces
    // this with the `tracing::subscriber::DefaultGuard` once the JSON layer
    // is wired to `config.tracing_log_path`.
    _private: (),
}

impl Drop for CampaignSubscriberGuard {
    fn drop(&mut self) {
        // Phase A stub: no installed subscriber to uninstall yet. `bbf6b6ee`
        // replaces this guard with the held `DefaultGuard`, whose own `Drop`
        // restores the previous subscriber.
    }
}

/// Installs the campaign tracing subscriber and returns an RAII guard.
///
/// While the returned guard is alive, campaign tracing events (`campaign_start`,
/// `snr_completed`, `heartbeat`) are routed to the JSON-lines sink configured
/// by [`PipelineConfig::tracing_log_path`]. Dropping the guard uninstalls it.
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
/// subscriber and is almost certainly a bug. Bind it to a live `let`.
pub fn install_campaign_subscriber(config: &PipelineConfig) -> CampaignSubscriberGuard {
    // Phase A stub. The full implementation (owned by `bbf6b6ee`) opens
    // `config.tracing_log_path` in append mode and installs a JSON
    // `tracing_subscriber::fmt` layer, returning its `DefaultGuard`.
    let _ = config;
    CampaignSubscriberGuard { _private: () }
}
