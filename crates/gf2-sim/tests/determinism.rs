//! Determinism contract tests (design doc §11).
//!
//! Owned by `48a0db6c`. This Phase A file holds only a smoke check that the
//! crate links and the public scaffolding is constructible; the byte-identity
//! assertions across worker counts and resume-from-checkpoint land in that
//! task.

use std::num::NonZeroUsize;

use gf2_sim::PipelineConfig;

#[test]
fn smoke_pipeline_config_constructs() {
    let cfg = PipelineConfig {
        seed: 0xC0DE_F00D,
        esn0_db_points: vec![4.0, 5.0, 6.0],
        target_errors: 100,
        max_frames: 1_000,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(1).unwrap(),
        strict_gpu: false,
    };

    assert_eq!(cfg.esn0_db_points.len(), 3);
    assert_eq!(cfg.seed, 0xC0DE_F00D);
    assert!(!cfg.strict_gpu);
}
