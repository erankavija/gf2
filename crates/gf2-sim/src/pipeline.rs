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
        }
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
    batch_id: u64,
    /// The SNR-point index this batch belongs to.
    snr_idx: u32,
}

impl BatchHandle {
    /// Constructs a handle for the given batch and SNR-point index.
    ///
    /// Crate-private: handles are minted only by `Pipeline::submit` (landing
    /// with the graph wave `c09d3e95`); callers obtain them opaquely and feed
    /// them back to `Pipeline::collect`. The design doc (§1) specifies private
    /// fields so the buffer-reference machinery can be added later without a
    /// breaking change.
    // Scaffolding: the only caller (`Pipeline::submit`) lands with the graph
    // wave `c09d3e95`; exercised now by the unit test below.
    #[allow(dead_code)]
    pub(crate) fn new(batch_id: u64, snr_idx: u32) -> Self {
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
