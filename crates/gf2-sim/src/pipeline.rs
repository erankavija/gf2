//! The [`Pipeline`] — a built, runnable graph of stages.
//!
//! Lifts the §1 "`Pipeline` and `BatchHandle`" block of the Phase 0 design doc
//! (`dev/active/ec530af9-pipeline-design.md`) into a Phase A skeleton. The
//! batch-submission and run methods (`submit`, `collect`, `run_with_decoder`,
//! `run_parallel`) land with the graph (`c09d3e95`), parallel (`3fcb7025`),
//! and migration (`bbf6b6ee`) waves; only the owning data structure and the
//! [`BatchHandle`] are scaffolded here.

use std::collections::HashMap;

use crate::checkpoint::SweepError;
use crate::config::PipelineConfig;
use crate::connector::{Edge, StageId};
use crate::error::StageError;
use crate::executor::{CheckpointedSweep, RunPlan, Scheduler, SimulationResults};
use crate::stage::AnyStage;

/// A built, runnable pipeline.
///
/// Owns a heterogeneous list of type-erased stages ([`AnyStage`]), the edges
/// connecting them, the registered CPU fallbacks (for GPU-OOM substitution,
/// design doc §8), and the run configuration.
///
/// The build entry points and run methods are introduced by later waves; this
/// is the Phase A owning structure that lets those waves fan out without
/// touching each other's files.
pub struct Pipeline {
    /// The type-erased stages, in topological order.
    stages: Vec<Box<dyn AnyStage>>,
    /// The directed edges connecting the stages.
    edges: Vec<Edge>,
    /// CPU fallbacks registered per GPU stage (design doc §8).
    fallbacks: HashMap<StageId, Box<dyn AnyStage>>,
    /// The run configuration.
    config: PipelineConfig,
    /// How to run this pipeline (set by a preset builder). `None` for a chain
    /// built directly via the graph API with no run plan attached — such a
    /// pipeline is inspectable (`stages()` / `edges()`) but not [`run`](Pipeline::run)nable.
    run_plan: Option<RunPlan>,
}

impl Pipeline {
    /// Assembles a pipeline from its already-validated parts.
    ///
    /// Crate-private: the only caller is [`Chain::build`](crate::graph::Chain::build)
    /// (the graph wave `c09d3e95`), which performs the topological sort, type
    /// re-validation, cycle/connectivity checks, and GPU-fallback extraction
    /// before handing the ordered parts here. Keeping this out of the public API
    /// preserves the design-doc §1 invariant that a `Pipeline` is only ever
    /// obtained through a validating builder.
    ///
    /// # Arguments
    ///
    /// * `stages` — the type-erased stages in topological order.
    /// * `edges` — the directed edges among those stages.
    /// * `fallbacks` — CPU fallbacks keyed by the GPU stage they substitute.
    /// * `config` — the run configuration.
    pub(crate) fn from_parts(
        stages: Vec<Box<dyn AnyStage>>,
        edges: Vec<Edge>,
        fallbacks: HashMap<StageId, Box<dyn AnyStage>>,
        config: PipelineConfig,
    ) -> Self {
        Self {
            stages,
            edges,
            fallbacks,
            config,
            run_plan: None,
        }
    }

    /// Attaches a [`RunPlan`] so this pipeline becomes [`run`](Pipeline::run)nable.
    ///
    /// Crate-private: only a preset builder (e.g. [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2))
    /// knows the validated run parameters, so it sets the plan after
    /// [`Chain::build`](crate::graph::Chain::build) returns the inspectable
    /// pipeline.
    pub(crate) fn set_run_plan(&mut self, plan: RunPlan) {
        self.run_plan = Some(plan);
    }

    /// The pipeline's [`RunPlan`], if a preset attached one.
    ///
    /// Consumed by the [`Scheduler`] to reconstruct the per-SNR frame kernel.
    pub(crate) fn run_plan(&self) -> Option<RunPlan> {
        self.run_plan
    }

    /// Returns the number of stages in this pipeline.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Returns the type-erased stages in topological order.
    ///
    /// The order is the linearisation [`Chain::build`](crate::graph::Chain::build)
    /// produced; consumers (the Phase C executor `de160fc5`, and roundtrip tests
    /// that drive the chain via [`AnyStage::process_any`])
    /// step through the stages in this order.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::graph::Chain;
    /// use gf2_sim::stage::{erase, BatchSize, ExecutionClass, Stage};
    /// use gf2_sim::error::StageError;
    ///
    /// #[derive(Clone)]
    /// struct B(u8);
    /// impl BatchSize for B {
    ///     fn batch_size(&self) -> usize {
    ///         1
    ///     }
    /// }
    /// struct Id;
    /// impl Stage<B, B> for Id {
    ///     type Scratch = ();
    ///     type CpuFallback = Self;
    ///     fn process(&self, i: &B, _: &mut ()) -> Result<B, StageError> {
    ///         Ok(i.clone())
    ///     }
    ///     fn execution_class(&self) -> ExecutionClass {
    ///         ExecutionClass::CpuOnly
    ///     }
    /// }
    ///
    /// let mut chain = Chain::new();
    /// let a = chain.add(erase(Id));
    /// let b = chain.add(erase(Id));
    /// chain.connect(a, b).unwrap();
    /// let pipeline = chain.build().unwrap();
    /// // The first stage's input type matches the second stage's input type
    /// // (both `B`), and there are two stages in topological order.
    /// assert_eq!(pipeline.stages().len(), 2);
    /// ```
    pub fn stages(&self) -> &[Box<dyn AnyStage>] {
        &self.stages
    }

    /// Returns the edges connecting the stages.
    ///
    /// The `from` and `to` fields of each [`Edge`] are **positions in
    /// [`stages()`](Pipeline::stages)**, not the original insertion-order
    /// [`StageId`]s. `pipeline.stages()[edge.from.0]` is the producer stage
    /// and `pipeline.stages()[edge.to.0]` is the consumer stage.
    ///
    /// [`Chain::build`](crate::graph::Chain::build) remaps all edge endpoints
    /// to post-topo-sort positions before handing them here, so this contract
    /// holds regardless of the stage insertion order.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Returns the run configuration.
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Returns the number of registered CPU fallbacks.
    pub fn fallback_count(&self) -> usize {
        self.fallbacks.len()
    }

    /// Runs the pipeline over its configured SNR sweep, returning the aggregate
    /// [`SimulationResults`] (design doc §12 migration target).
    ///
    /// This is the convenience entry point downstream campaign code (`bbf6b6ee`,
    /// the calibration receipt `0d9cb8e3`) calls. It constructs a [`Scheduler`]
    /// from this pipeline's [`PipelineConfig`](crate::PipelineConfig) and drives
    /// it: when `gpu_enabled` is set (and the `hip` feature is built in with a
    /// usable device), the hybrid CPU+GPU overlap path runs; otherwise the
    /// CPU-only within-SNR frame-parallel path runs. The two paths agree on the
    /// three contractual columns `fer` / `frames` / `errors` (design doc §11);
    /// `mean_iters` is run-to-run deterministic on a fixed path.
    ///
    /// The sweep parameters (`esn0_db_points`, `max_frames`, `seed`,
    /// `parallelism`, `gpu_enabled`) come from the config; set them on the
    /// builder / config before calling.
    ///
    /// # Errors
    ///
    /// Returns a [`StageError`] if the pipeline carries no [`RunPlan`] (built via
    /// the graph API without a preset) or a GPU stage faults fatally.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::num::NonZeroUsize;
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::dvb_t2::{Channel, Modcod};
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// let mut pipeline = Pipeline::dvb_t2()
    ///     .modcod(Modcod::Normal { rate: CodeRate::Rate1_2, modulation: DvbT2Modulation::Qam16 })
    ///     .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
    ///     .demap(DemapMethod::MaxLog)
    ///     .channel(Channel::awgn(6.0))
    ///     .parallelism(NonZeroUsize::new(24).unwrap())
    ///     .build()
    ///     .unwrap();
    /// // Configure the sweep, then run (heavy: a full n=64800 decode per frame).
    /// pipeline.config_mut().esn0_db_points = vec![6.0];
    /// pipeline.config_mut().max_frames = 200;
    /// let results = pipeline.run().unwrap();
    /// assert_eq!(results.per_point.len(), 1);
    /// ```
    pub fn run(&self) -> Result<SimulationResults, StageError> {
        let scheduler = Scheduler::from_pipeline(self);
        let handle = BatchHandle::new(0, 0);
        scheduler.run(self, handle)
    }

    /// Alias for [`run`](Pipeline::run) matching the §12 migration table name
    /// (`SimulationRunner::run_with_decoder` → `Pipeline::run_with_decoder`).
    ///
    /// # Errors
    ///
    /// See [`run`](Pipeline::run).
    pub fn run_with_decoder(&self) -> Result<SimulationResults, StageError> {
        self.run()
    }

    /// Alias for [`run`](Pipeline::run) matching the §12 migration table name
    /// (`SimulationRunner::run_coded_iterative_parallel` →
    /// `Pipeline::run_parallel`). The worker count is taken from the config's
    /// `parallelism` (set it on the builder); this alias exists for the
    /// migration call-site naming.
    ///
    /// # Errors
    ///
    /// See [`run`](Pipeline::run).
    pub fn run_parallel(&self) -> Result<SimulationResults, StageError> {
        self.run()
    }

    /// Runs the pipeline's SNR sweep with heartbeat + SNR-boundary + SIGINT
    /// checkpointing and `--resume` support (design doc §4; task `571c11c4`).
    ///
    /// Like [`run`](Pipeline::run), but every SNR point checkpoints to
    /// `config.checkpoint_dir` at the heartbeat cadence
    /// (`heartbeat_every_frames`), at the SNR boundary, and on
    /// SIGINT/SIGTERM. On the hybrid CPU+GPU path the in-flight GPU batches
    /// are drained per-stream
    /// ([`Scheduler::drain_for_checkpoint`](crate::Scheduler::drain_for_checkpoint))
    /// before every flush; with `resume`, prior checkpoints are loaded —
    /// completed points fold their saved counters, and a partial point
    /// continues byte-identically (the hybrid path restores each worker's
    /// strided-partition progress from `worker_states[].frames_in_worker`;
    /// the CPU path resumes via the global `frames_completed`, `5f12e7ff`).
    ///
    /// # Arguments
    ///
    /// * `resume` — `true` to load and continue from existing checkpoints in
    ///   `config.checkpoint_dir`; `false` to start every point fresh.
    ///
    /// # Errors
    ///
    /// A [`SweepError`]: `Load` for an invalid/mismatched checkpoint, `Io` for
    /// a failed checkpoint write, `Stage` for a GPU fault, a failed drain, a
    /// missing `checkpoint_dir`, or a missing [`RunPlan`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::num::NonZeroUsize;
    /// use gf2_sim::Pipeline;
    /// use gf2_sim::presets::dvb_t2::{Channel, Modcod};
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    /// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// let mut pipeline = Pipeline::dvb_t2()
    ///     .modcod(Modcod::Normal { rate: CodeRate::Rate1_2, modulation: DvbT2Modulation::Qam16 })
    ///     .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
    ///     .demap(DemapMethod::MaxLog)
    ///     .channel(Channel::awgn(6.0))
    ///     .parallelism(NonZeroUsize::new(8).unwrap())
    ///     .checkpoint_dir(Some("/tmp/ck".into()))
    ///     .with_gpu(true)
    ///     .build()
    ///     .unwrap();
    /// pipeline.config_mut().esn0_db_points = vec![6.0, 6.5];
    /// pipeline.config_mut().max_frames = 200;
    /// pipeline.config_mut().heartbeat_every_frames = 64;
    /// let sweep = pipeline.run_checkpointed(false).unwrap();
    /// if sweep.interrupted {
    ///     // SIGINT flushed a resumable checkpoint; continue later with:
    ///     // pipeline.run_checkpointed(true)
    /// }
    /// ```
    pub fn run_checkpointed(&self, resume: bool) -> Result<CheckpointedSweep, SweepError> {
        let scheduler = Scheduler::from_pipeline(self);
        scheduler.run_sweep_checkpointed(self, resume, &|_, _| {})
    }

    /// Mutable access to the run configuration, so a caller can set the sweep
    /// (`esn0_db_points`, `max_frames`, …) on a pipeline the preset built with
    /// empty sweep defaults before [`run`](Pipeline::run).
    pub fn config_mut(&mut self) -> &mut PipelineConfig {
        &mut self.config
    }
}

/// An opaque handle returned by `Pipeline::submit` and consumed by
/// `Pipeline::collect`.
///
/// Carries the SoA buffer references the pipeline allocated for this batch's
/// lifetime (design doc §1/§2). The buffer-reference machinery is introduced
/// by the graph/parallel waves; this Phase A handle records the identifying
/// fields only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchHandle {
    /// The unique batch identifier.
    batch_id: u64,
    /// The SNR-point index this batch belongs to.
    snr_idx: u32,
}

impl BatchHandle {
    /// Constructs a handle for the given batch and SNR-point index.
    ///
    /// Public so a caller driving the [`Scheduler`](crate::Scheduler) engine
    /// directly (`Scheduler::run(&pipeline, handle)`) can mint the starting
    /// batch handle; the high-level [`Pipeline::run`](crate::Pipeline::run)
    /// convenience entry point mints one internally. The fields stay private so
    /// the buffer-reference machinery (design doc §1) can be added later without
    /// a breaking change.
    #[must_use]
    pub fn new(batch_id: u64, snr_idx: u32) -> Self {
        Self { batch_id, snr_idx }
    }

    /// Returns the unique batch identifier.
    pub fn batch_id(&self) -> u64 {
        self.batch_id
    }

    /// Returns the SNR-point index this batch belongs to.
    pub fn snr_idx(&self) -> u32 {
        self.snr_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_handle_accessors_are_read_only() {
        let h = BatchHandle::new(7, 3);
        assert_eq!(h.batch_id(), 7);
        assert_eq!(h.snr_idx(), 3);
    }
}
