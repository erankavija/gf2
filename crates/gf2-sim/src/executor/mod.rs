//! Hybrid CPU/GPU executor (design doc §6 / §8, Phase C).
//!
//! Owned by Phase C. `75c22fa8` lands the foundational [`Scheduler`]: it pairs
//! each rayon worker with one HIP stream and overlaps CPU preparation of batch
//! `N+1` against GPU execution of batch `N`, plus the runnable
//! [`SimulationResults`] surface [`Pipeline::run`](crate::Pipeline::run)
//! returns. `de160fc5` adds the [`TopologyExecutor`]: per-stage-driven DAG
//! execution (topo order, fan-in/fan-out, execution-class routing, per-stage
//! spans, defensive execution-start validation). `42eac5cc` adds OOM
//! auto-fallback dispatch and hard-fail diagnostic dumps ([`failure`]).
//! `571c11c4` adds the GPU drain-for-checkpoint and the checkpointed hybrid
//! sweep with `--resume` ([`drain`] module:
//! [`Scheduler::drain_for_checkpoint`],
//! [`Scheduler::run_sweep_checkpointed`],
//! [`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed)).
//! `bb11c2e6` factors the uncheckpointed scheduler loop and the checkpointed
//! drain loop onto ONE shared double-buffer core (`hybrid_core`, `feature =
//! "hip"`), parameterized by per-batch hooks: the failure-semantics hook (the
//! scheduler substitutes the CPU fallback; the checkpointed sweep propagates
//! the fault, aborting resumably) and the stop-after-batch hook (the
//! checkpointed loop stops at batch-aligned interrupt points). It also gives
//! the checkpointed loop `pipeline_stage` span parity for free.
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
//! | [`failure`] | [`dispatch_with_fallback`](failure::dispatch_with_fallback): OOM auto-fallback + hard-fail diagnostic dump (`42eac5cc`) |
//! | [`StreamInFlight`] / [`CheckpointedSweep`] | per-stream drain tally + checkpointed hybrid sweep outcome (`571c11c4`) |

mod drain;
pub mod failure;
#[cfg(feature = "hip")]
mod hybrid_core;
mod results;
mod scheduler;
mod topology;

pub use drain::{CheckpointedSweep, StreamInFlight};
pub use failure::{default_dump_dir, dispatch_with_fallback, FaultContext};
pub use results::{SimulationResults, SnrPointResult};
pub use scheduler::{ActivityInterval, ActivityKind, OverlapTimeline, RunPlan, Scheduler};
pub use topology::{DagOutputs, TopologyExecutor, NO_STREAM};
