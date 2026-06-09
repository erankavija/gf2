//! Hybrid CPU/GPU executor (design doc §6 / §8, Phase C).
//!
//! Owned by Phase C. This task (`75c22fa8`) lands the foundational
//! [`Scheduler`]: it pairs each rayon worker with one HIP stream and overlaps
//! CPU preparation of batch `N+1` against GPU execution of batch `N`, plus the
//! runnable [`SimulationResults`] surface [`Pipeline::run`](crate::Pipeline::run)
//! returns. The DAG executor (`de160fc5`), resume (`571c11c4`), and OOM
//! auto-fallback dispatch (`42eac5cc`) build on it.
//!
//! # Module map
//!
//! | Item | Purpose |
//! |------|---------|
//! | [`Scheduler`] | the hybrid worker-pool + stream-pool engine driving the run |
//! | [`RunPlan`] | how a built [`Pipeline`](crate::Pipeline) is run (DVB-T2 BICM preset) |
//! | [`SimulationResults`] / [`SnrPointResult`] | the per-SNR-point aggregate columns (SSOT [`WorkerCounters`](crate::WorkerCounters) projection) |
//! | [`OverlapTimeline`] / [`ActivityInterval`] / [`ActivityKind`] | CPU↔GPU overlap attestation (criterion 1) |

mod results;
mod scheduler;

pub use results::{SimulationResults, SnrPointResult};
pub use scheduler::{ActivityInterval, ActivityKind, OverlapTimeline, RunPlan, Scheduler};
