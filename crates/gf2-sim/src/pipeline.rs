//! The [`Pipeline`] — a built, runnable graph of stages.

use std::collections::HashMap;

use crate::config::PipelineConfig;
use crate::connector::{Edge, StageId};
use crate::error::StageError;
use crate::executor::{CheckpointedSweep, RunPlan, Scheduler, SimulationResults};
use crate::snr_checkpoint::SweepError;
use crate::stage::AnyStage;

/// A built, runnable pipeline.
///
/// Owns a heterogeneous list of type-erased stages ([`AnyStage`]), the edges
/// connecting them, the registered CPU fallbacks (for GPU-OOM substitution),
/// and the run configuration.
pub struct Pipeline {
    /// The type-erased stages, in topological order.
    stages: Vec<Box<dyn AnyStage>>,
    /// The directed edges connecting the stages.
    edges: Vec<Edge>,
    /// CPU fallbacks registered per GPU stage.
    fallbacks: HashMap<StageId, Box<dyn AnyStage>>,
    /// The run configuration.
    config: PipelineConfig,
    /// How to run this pipeline (set by a preset builder). `None` for a chain
    /// built directly via the graph API with no run plan attached — such a
    /// pipeline is inspectable (`stages()` / `edges()`) but not [`run`](Pipeline::run)nable.
    run_plan: Option<RunPlan>,
}

impl Pipeline {
    /// Assembles a pipeline from parts that
    /// [`Chain::build`](crate::graph::Chain::build) has validated and
    /// topologically ordered.
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
    pub(crate) fn set_run_plan(&mut self, plan: RunPlan) {
        self.run_plan = Some(plan);
    }

    /// The pipeline's [`RunPlan`], if a preset attached one.
    pub(crate) fn run_plan(&self) -> Option<RunPlan> {
        self.run_plan
    }

    /// Returns the number of stages in this pipeline.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Returns the type-erased stages in topological order.
    pub fn stages(&self) -> &[Box<dyn AnyStage>] {
        &self.stages
    }

    /// Returns the edges connecting the stages.
    ///
    /// The `from` and `to` fields of each [`Edge`] are **positions in
    /// [`stages()`](Pipeline::stages)**, not the original insertion-order
    /// [`StageId`]s. `pipeline.stages()[edge.from.0]` is the producer stage
    /// and `pipeline.stages()[edge.to.0]` is the consumer stage.
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

    /// Runs the pipeline over the SNR sweep of its [`PipelineConfig`].
    ///
    /// The hybrid CPU+GPU path runs when `gpu_enabled` is set and the `hip`
    /// feature provides a usable device; otherwise the CPU within-SNR
    /// frame-parallel path runs.
    ///
    /// # Errors
    ///
    /// Returns a [`StageError`] if the pipeline carries no [`RunPlan`] (built via
    /// the graph API without a preset) or a GPU stage faults fatally.
    pub fn run(&self) -> Result<SimulationResults, StageError> {
        let scheduler = Scheduler::from_pipeline(self);
        let handle = BatchHandle::new(0, 0);
        scheduler.run(self, handle)
    }

    /// Alias for [`run`](Pipeline::run).
    ///
    /// # Errors
    ///
    /// See [`run`](Pipeline::run).
    pub fn run_with_decoder(&self) -> Result<SimulationResults, StageError> {
        self.run()
    }

    /// Alias for [`run`](Pipeline::run).
    ///
    /// # Errors
    ///
    /// See [`run`](Pipeline::run).
    pub fn run_parallel(&self) -> Result<SimulationResults, StageError> {
        self.run()
    }

    /// Runs the SNR sweep with checkpointing and resume.
    ///
    /// Every SNR point checkpoints to `config.checkpoint_dir` at the
    /// `heartbeat_every_frames` cadence, at the SNR boundary, and on
    /// SIGINT/SIGTERM. On the hybrid CPU+GPU path the in-flight GPU batches
    /// are drained per stream
    /// ([`Scheduler::drain_for_checkpoint`](crate::Scheduler::drain_for_checkpoint))
    /// before every flush. With `resume`, existing checkpoints in
    /// `config.checkpoint_dir` are loaded: completed points contribute their
    /// saved counters and a partial point continues. A recoverable GPU fault
    /// aborts the sweep and leaves the last committed checkpoint, where
    /// [`run`](Pipeline::run) substitutes the CPU fallback.
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

/// Identifies one batch of a run by batch id and SNR-point index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchHandle {
    /// The unique batch identifier.
    batch_id: u64,
    /// The SNR-point index this batch belongs to.
    snr_idx: u32,
}

impl BatchHandle {
    /// Constructs a handle for the given batch and SNR-point index.
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

    fn empty_sweep_pipeline() -> crate::Pipeline {
        use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
        use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
        use gf2_coding::modem::DemapMethod;
        use gf2_coding::CodeRate;
        let mut p = crate::Pipeline::dvb_t2()
            .modcod(crate::presets::dvb_t2::Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
            .demap(DemapMethod::ExactLogMap)
            .channel(crate::presets::dvb_t2::Channel::awgn(9.0_f32))
            .seed(42)
            .build()
            .expect("DVB-T2 r1/2 16-QAM builds");
        p.config_mut().esn0_db_points = vec![];
        p
    }

    #[test]
    fn test_run_with_decoder_empty_sweep_returns_ok() {
        let p = empty_sweep_pipeline();
        let r = p.run_with_decoder().expect("empty sweep must succeed");
        assert_eq!(r.per_point.len(), 0);
    }

    #[test]
    fn test_run_parallel_empty_sweep_returns_ok() {
        let p = empty_sweep_pipeline();
        let r = p.run_parallel().expect("empty sweep must succeed");
        assert_eq!(r.per_point.len(), 0);
    }
}
