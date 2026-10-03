//! Run configuration of a [`Pipeline`](crate::Pipeline).

use std::num::NonZeroUsize;
use std::path::PathBuf;

use gf2_coding::simulation::SimulationConfig;

/// Configuration for a [`Pipeline`](crate::Pipeline) run.
///
/// Converts from [`SimulationConfig`] through [`From`].
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Base RNG seed for the per-worker ChaCha20 streams.
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
    /// When set, the hybrid executor offloads GPU-capable stages to the HIP
    /// device; when unset, every stage runs on the CPU.
    ///
    /// Without the `hip` Cargo feature, or when no HIP stream pool can be
    /// built, a set flag logs a `tracing::warn!` and the CPU path runs.
    pub gpu_enabled: bool,
    /// When set, GPU out-of-memory is promoted to a fatal error instead of
    /// falling back to the CPU stage.
    pub strict_gpu: bool,
    /// Directory for JSON hard-fail diagnostic dumps (one file per fatal GPU
    /// stage event, written atomically via a `.tmp` sibling + rename).
    ///
    /// `None` selects
    /// [`default_dump_dir`](crate::executor::failure::default_dump_dir).
    pub diagnostic_dump_dir: Option<PathBuf>,
    /// Test-only GPU out-of-memory fault injection.
    ///
    /// `Some(m)` with `m >= 1` makes the GPU LDPC dispatch raise
    /// [`RecoverableError::OutOfMemory`](crate::error::RecoverableError::OutOfMemory)
    /// instead of launching the kernel: the topology executor on each frame
    /// whose global index `g` satisfies `g % m == 0`, the scheduler and the
    /// checkpointed sweep on each batch whose first global frame index does.
    /// The topology executor and the scheduler route the error through the
    /// fallback dispatch (CPU fallback, or fatal under `strict_gpu`); the
    /// checkpointed sweep propagates it before any flush, leaving the run
    /// resumable. Excluded from
    /// [`config_hash`](crate::snr_checkpoint::config_hash).
    pub inject_gpu_oom_modulus: Option<u64>,
}

impl From<&SimulationConfig> for PipelineConfig {
    /// `rng_seed: None` maps to seed `0`. `eb_n0_range_db` is copied into
    /// `esn0_db_points` without unit conversion.
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
            inject_gpu_oom_modulus: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::simulation::SimulationConfig;
    use std::num::NonZeroUsize;
    use std::path::PathBuf;

    #[test]
    fn test_from_simulation_config_maps_all_fields() {
        let sc = SimulationConfig {
            rng_seed: Some(42),
            eb_n0_range_db: vec![3.0, 4.0, 5.0],
            min_errors: 50,
            max_frames: 10000,
            max_decoder_iterations: 50,
            heartbeat_every_frames: Some(100),
            checkpoint_dir: Some(PathBuf::from("/tmp/cp")),
            tracing_log_path: Some(PathBuf::from("/tmp/trace.json")),
            output_path: None,
        };
        let pc = PipelineConfig::from(&sc);
        assert_eq!(pc.seed, 42);
        assert_eq!(pc.esn0_db_points, vec![3.0, 4.0, 5.0]);
        assert_eq!(pc.target_errors, 50);
        assert_eq!(pc.max_frames, 10000);
        assert_eq!(pc.heartbeat_every_frames, 100);
        assert_eq!(pc.checkpoint_dir, Some(PathBuf::from("/tmp/cp")));
        assert_eq!(pc.tracing_log_path, Some(PathBuf::from("/tmp/trace.json")));
        assert_eq!(pc.parallelism, NonZeroUsize::new(1).unwrap());
        assert!(!pc.gpu_enabled);
        assert!(!pc.strict_gpu);
        assert!(pc.diagnostic_dump_dir.is_none());
        assert!(pc.inject_gpu_oom_modulus.is_none());
    }

    #[test]
    fn test_from_simulation_config_none_seed_defaults_to_zero() {
        let sc = SimulationConfig {
            rng_seed: None,
            eb_n0_range_db: vec![],
            min_errors: 0,
            max_frames: 0,
            max_decoder_iterations: 0,
            heartbeat_every_frames: None,
            checkpoint_dir: None,
            tracing_log_path: None,
            output_path: None,
        };
        let pc = PipelineConfig::from(&sc);
        assert_eq!(pc.seed, 0);
        assert_eq!(pc.heartbeat_every_frames, 0);
    }
}
