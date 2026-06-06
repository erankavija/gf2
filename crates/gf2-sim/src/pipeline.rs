//! The [`Pipeline`] — a built, runnable graph of stages.
//!
//! Lifts the §1 "`Pipeline` and `BatchHandle`" block of the Phase 0 design doc
//! (`dev/active/ec530af9-pipeline-design.md`) into a Phase A skeleton. The
//! batch-submission and run methods (`submit`, `collect`, `run_with_decoder`,
//! `run_parallel`) land with the graph (`c09d3e95`), parallel (`3fcb7025`),
//! and migration (`bbf6b6ee`) waves; only the owning data structure and the
//! [`BatchHandle`] are scaffolded here.

use std::collections::HashMap;

use crate::config::PipelineConfig;
use crate::connector::{Edge, StageId};
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
}

impl Pipeline {
    /// Returns the number of stages in this pipeline.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Returns the edges connecting the stages.
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
    pub batch_id: u64,
    /// The SNR-point index this batch belongs to.
    pub snr_idx: u32,
}
