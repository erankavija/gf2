//! Pipeline configuration, the v2 successor to
//! [`gf2_coding::simulation::SimulationConfig`].
//!
//! Lifts the §1 "`PipelineConfig`" block and the §12 migration mapping of the
//! Phase 0 design doc (`dev/active/ec530af9-pipeline-design.md`) into code.
//! The [`From<&SimulationConfig>`] impl makes the `bbf6b6ee` migration
//! mechanical.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use gf2_coding::simulation::SimulationConfig;

/// Configuration for a [`Pipeline`](crate::Pipeline) run.
///
/// Mirrors the run-control fields of
/// [`gf2_coding::simulation::SimulationConfig`] and adds the Phase 0 pipeline
/// knobs (`parallelism`, `strict_gpu`, `diagnostic_dump_dir`). A
/// [`From<&SimulationConfig>`] impl is provided so existing campaign configs
/// convert directly.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
/// use gf2_sim::PipelineConfig;
///
/// let cfg = PipelineConfig {
///     seed: 0xC0DE_F00D,
///     esn0_db_points: vec![4.0, 4.5, 5.0],
///     target_errors: 100,
///     max_frames: 10_000_000,
///     heartbeat_every_frames: 1000,
///     checkpoint_dir: None,
///     tracing_log_path: None,
///     parallelism: NonZeroUsize::new(1).unwrap(),
///     gpu_enabled: false,
///     strict_gpu: false,
///     diagnostic_dump_dir: None,
/// };
/// assert_eq!(cfg.esn0_db_points.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Base RNG seed for the per-worker ChaCha20 streams (design doc §3).
    pub seed: u64,
    /// The Es/N0 points (in dB) to simulate.
    pub esn0_db_points: Vec<f64>,
    /// Minimum number of frame errors to collect per SNR point.
    pub target_errors: u64,
    /// Maximum number of frames to simulate per SNR point.
    pub max_frames: u64,
    /// Within-SNR heartbeat / checkpoint cadence, in frames.
    ///
    /// A value of `0` disables within-SNR heartbeats (only completed SNR
    /// points are checkpointed).
    pub heartbeat_every_frames: u64,
    /// Optional directory for v2 per-SNR checkpoint files.
    pub checkpoint_dir: Option<PathBuf>,
    /// Optional path for JSON-lines tracing output.
    pub tracing_log_path: Option<PathBuf>,
    /// Number of parallel workers.
    pub parallelism: NonZeroUsize,
    /// When set, the hybrid executor offloads the heavy GPU-bound stages (LDPC
    /// belief-propagation decode, and max-log demap) to the HIP device,
    /// overlapping device execution of one batch with CPU preparation of the
    /// next (Phase C scheduler `75c22fa8`). When unset (the default), every
    /// stage runs on the CPU.
    ///
    /// Without the `hip` Cargo feature this flag has no effect: a pipeline built
    /// with it set degrades gracefully to the CPU path after a `tracing::warn!`
    /// (there is no device backend to dispatch to).
    pub gpu_enabled: bool,
    /// When set, GPU out-of-memory is promoted to a fatal error instead of
    /// falling back to the CPU stage (design doc §8).
    pub strict_gpu: bool,
    /// Directory for JSON hard-fail diagnostic dumps (one file per fatal GPU
    /// stage event, written atomically via a `.tmp` sibling + rename).
    ///
    /// Defaults to `None`; when `None` the executor uses
    /// `dev/benchmarks/gf2-sim/diagnostic-dumps/` (from
    /// [`default_dump_dir`](crate::executor::failure::default_dump_dir)).
    /// Tests should set this to a temp dir to avoid polluting the repo.
    pub diagnostic_dump_dir: Option<PathBuf>,
}

impl From<&SimulationConfig> for PipelineConfig {
    /// Converts a legacy [`SimulationConfig`] into a [`PipelineConfig`].
    ///
    /// Field mapping (design doc §12):
    ///
    /// * `rng_seed` → `seed` (defaulting to `0` when `None`, since the new
    ///   pipeline always uses a fixed seed for deterministic per-worker seek).
    /// * `eb_n0_range_db` → `esn0_db_points` (the SNR-point vector moves
    ///   verbatim; callers that work in Eb/N0 convert upstream).
    /// * `min_errors` / `max_frames` widen `usize` → `u64`.
    /// * `heartbeat_every_frames: Option<usize>` → `u64` (`None` ⇒ `0`).
    /// * `checkpoint_dir` / `tracing_log_path` move verbatim.
    /// * `parallelism` defaults to `1`; `gpu_enabled` and `strict_gpu` default
    ///   to `false` (none has a legacy source field).
    fn from(c: &SimulationConfig) -> Self {
        Self {
            seed: c.rng_seed.unwrap_or(0),
            esn0_db_points: c.eb_n0_range_db.clone(),
            target_errors: c.min_errors as u64,
            max_frames: c.max_frames as u64,
            heartbeat_every_frames: c.heartbeat_every_frames.unwrap_or(0) as u64,
            checkpoint_dir: c.checkpoint_dir.clone(),
            tracing_log_path: c.tracing_log_path.clone(),
            parallelism: NonZeroUsize::new(1).expect("1 is non-zero"),
            gpu_enabled: false,
            strict_gpu: false,
            diagnostic_dump_dir: None,
        }
    }
}
