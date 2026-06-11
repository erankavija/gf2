//! Hybrid CPU/GPU executor (design doc §6 / §8, Phase C).
//!
//! Owned by Phase C. `75c22fa8` lands the foundational [`Scheduler`]: it pairs
//! each rayon worker with one HIP stream and overlaps CPU preparation of batch
//! `N+1` against GPU execution of batch `N`, plus the runnable
//! [`SimulationResults`] surface [`Pipeline::run`](crate::Pipeline::run)
//! returns. `de160fc5` adds the [`TopologyExecutor`]: per-stage-driven DAG
//! execution (topo order, fan-in/fan-out, execution-class routing, per-stage
//! spans, defensive execution-start validation). Resume (`571c11c4`) and OOM
//! auto-fallback dispatch (`42eac5cc`) build on these.
//!
//! # Module map
//!
//! | Item | Purpose |
//! |------|---------|
//! | [`Scheduler`] | the hybrid worker-pool + stream-pool engine driving the run |
//! | [`TopologyExecutor`] / [`DagOutputs`] | per-stage-driven DAG execution in topological order (`de160fc5`) |
//! | [`RunPlan`] | how a built [`Pipeline`](crate::Pipeline) is run (DVB-T2 BICM preset) |
//! | [`SimulationResults`] / [`SnrPointResult`] | the per-SNR-point aggregate columns (SSOT [`WorkerCounters`](crate::WorkerCounters) projection) |
//! | [`OverlapTimeline`] / [`ActivityInterval`] / [`ActivityKind`] | CPU↔GPU overlap attestation (criterion 1) |

mod results;
mod scheduler;
mod topology;

pub use results::{SimulationResults, SnrPointResult};
pub use scheduler::{ActivityInterval, ActivityKind, OverlapTimeline, RunPlan, Scheduler};
pub use topology::{DagOutputs, TopologyExecutor, NO_STREAM};
