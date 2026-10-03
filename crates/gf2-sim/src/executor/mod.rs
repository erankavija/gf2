//! Hybrid CPU/GPU executor: the [`Scheduler`] pairs each rayon worker with one
//! HIP stream and overlaps CPU preparation of batch `N+1` against GPU execution
//! of batch `N`; the [`TopologyExecutor`] runs a pipeline DAG stage by stage in
//! topological order; [`failure`] holds the OOM fallback dispatch and the
//! hard-fail diagnostic dump; the crate-private `drain` module adds the
//! checkpointed sweep ([`Scheduler::run_sweep_checkpointed`]).

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
