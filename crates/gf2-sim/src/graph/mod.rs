//! Graph-based chain construction API.
//!
//! A caller [`Chain::add`]s type-erased stages, [`Chain::connect`]s producers
//! to consumers with a runtime type check, optionally registers GPU→CPU
//! fallbacks ([`Chain::register_fallback`]), and [`Chain::build`]s a
//! [`Pipeline`] whose stage list is a topological order of the DAG. A stage
//! may have several outgoing and several incoming edges. [`Chain::build`]
//! lists the conditions it checks under its `# Errors`.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::connector::{Edge, StageId};
use crate::error::BuildError;
use crate::pipeline::Pipeline;
use crate::stage::AnyStage;
use crate::PipelineConfig;

/// The default per-edge batch size recorded on [`Edge`]s minted by
/// [`Chain::connect`].
///
/// The graph API joins stages by identity, not by negotiated buffer size.
const DEFAULT_EDGE_BATCH_SIZE: usize = 1;

/// A mutable builder for a stage graph that compiles into a [`Pipeline`].
///
/// # Examples
///
/// [`dvb_t2_bicm_stages`](crate::stages::dvb_t2_bicm_stages) returns the
/// forward and inverse stages type-erased; each is added in order and
/// connected to the next. The forward→inverse hop is a noiseless
/// `SymbolBatch` edge.
///
/// ```
/// use gf2_sim::graph::Chain;
/// use gf2_sim::stages::{dvb_t2_bicm_stages, DEFAULT_DEMAP_NOISE_VAR};
/// use gf2_coding::CodeRate;
/// use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
/// use gf2_coding::modem::DemapMethod;
///
/// // The factory wires the codec, interleaver, and modem for one MODCOD into
/// // erased forward + inverse stages. This example connects map → demap with no
/// // channel (a noiseless roundtrip), so the demapper uses the default N0.
/// let factory = dvb_t2_bicm_stages(
///     CodeRate::Rate1_2,
///     DvbT2Modulation::Qam16,
///     DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
///     DemapMethod::ExactLogMap,
///     DEFAULT_DEMAP_NOISE_VAR,
/// );
///
/// let mut chain = Chain::new();
/// let mut ids = Vec::new();
/// // Forward path: encode → interleave → map.
/// for stage in factory.forward {
///     ids.push(chain.add(stage));
/// }
/// // Inverse path: demap → deinterleave → decode.
/// for stage in factory.inverse {
///     ids.push(chain.add(stage));
/// }
///
/// // Join consecutively: fwd0 → fwd1 → fwd2 → inv0 → inv1 → inv2. The
/// // fwd2 → inv0 hop is the noiseless SymbolBatch → SymbolBatch gap.
/// for pair in ids.windows(2) {
///     chain
///         .connect(pair[0], pair[1])
///         .expect("each consecutive BICM hop is type-compatible");
/// }
///
/// let pipeline = chain.build().expect("the full BICM chain is a valid DAG");
/// assert_eq!(pipeline.stage_count(), 6, "six BICM stages");
/// assert_eq!(pipeline.edges().len(), 5, "five consecutive edges");
/// ```
pub struct Chain {
    /// `StageId(i)` ⇒ `stages[i]`.
    stages: Vec<Box<dyn AnyStage>>,
    edges: Vec<Edge>,
    /// `(gpu_stage, cpu_fallback_stage)` registrations.
    fallbacks: Vec<(StageId, StageId)>,
    config: Option<PipelineConfig>,
}

impl Default for Chain {
    fn default() -> Self {
        Self::new()
    }
}

impl Chain {
    /// Creates an empty chain.
    ///
    /// The built [`Pipeline`] receives a default [`PipelineConfig`] (single
    /// worker, no SNR sweep) unless one is supplied via [`Chain::with_config`].
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            edges: Vec::new(),
            fallbacks: Vec::new(),
            config: None,
        }
    }

    /// Sets the [`PipelineConfig`] the built [`Pipeline`] carries.
    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Adds a type-erased stage and returns its [`StageId`].
    ///
    /// A concrete `Stage<I, O>` is erased via [`erase`](crate::stage::erase).
    pub fn add(&mut self, stage: Box<dyn AnyStage>) -> StageId {
        let id = StageId(self.stages.len() as u32);
        self.stages.push(stage);
        id
    }

    /// Connects a producer stage to a consumer stage, comparing the producer's
    /// [`output_type()`](AnyStage::output_type) with the consumer's
    /// [`input_type()`](AnyStage::input_type). An error records no edge.
    ///
    /// A CPU fallback target of
    /// [`register_fallback`](Chain::register_fallback) is outside the pipeline
    /// DAG: this method records an edge to or from it, and
    /// [`build`](Chain::build) rejects that edge with
    /// [`BuildError::FallbackTargetHasEdge`].
    ///
    /// # Errors
    ///
    /// * [`BuildError::TypeMismatch`] if the producer output type and consumer
    ///   input type differ.
    /// * [`BuildError::Disconnected`] if either id refers to no added stage.
    pub fn connect(&mut self, from: StageId, to: StageId) -> Result<(), BuildError> {
        let from_stage = self
            .stage(from)
            .ok_or_else(|| BuildError::Disconnected { stages: vec![from] })?;
        let to_stage = self
            .stage(to)
            .ok_or_else(|| BuildError::Disconnected { stages: vec![to] })?;

        let from_type = from_stage.output_type();
        let to_type = to_stage.input_type();
        if from_type != to_type {
            return Err(BuildError::TypeMismatch {
                from_stage: from,
                from_type,
                to_stage: to,
                to_type,
            });
        }

        self.edges.push(Edge {
            from,
            to,
            element_type: from_type,
            batch_size: DEFAULT_EDGE_BATCH_SIZE,
        });
        Ok(())
    }

    /// Registers a CPU fallback stage for a GPU stage.
    ///
    /// [`Chain::build`] moves `cpu_stage` out of the stage list into the
    /// pipeline's fallback table keyed by `gpu_stage`, so the CPU fallback is a
    /// substitution target outside the DAG. This method records the pairing;
    /// [`Chain::build`] validates it and requires that:
    ///
    /// * `gpu_stage` is GPU-capable (`GpuOnly` or `Hybrid`).
    /// * `cpu_stage` is CPU-capable (`CpuOnly` or `Hybrid`).
    /// * `cpu_stage` has the same input and output element types as
    ///   `gpu_stage`.
    /// * each `gpu_stage` is registered at most once, each `cpu_stage` backs at
    ///   most one GPU stage, and no stage appears in both roles (which forbids
    ///   `gpu_stage == cpu_stage`).
    /// * `cpu_stage` has no [`connect`](Chain::connect)ed edge.
    ///
    /// See [`Chain::build`]'s `# Errors` for the `BuildError` each violation
    /// returns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::graph::Chain;
    /// use gf2_sim::stage::{erase, BatchSize, ExecutionClass, Stage};
    /// use gf2_sim::error::StageError;
    ///
    /// #[derive(Clone)]
    /// struct B(Vec<u8>);
    /// impl BatchSize for B {
    ///     fn batch_size(&self) -> usize {
    ///         self.0.len()
    ///     }
    /// }
    ///
    /// // A GPU stage (declares GpuOnly) and its CPU twin.
    /// struct Gpu;
    /// impl Stage<B, B> for Gpu {
    ///     type Scratch = ();
    ///     type CpuFallback = Self;
    ///     fn process(&self, i: &B, _: &mut ()) -> Result<B, StageError> {
    ///         Ok(i.clone())
    ///     }
    ///     fn execution_class(&self) -> ExecutionClass {
    ///         ExecutionClass::GpuOnly
    ///     }
    /// }
    /// struct Cpu;
    /// impl Stage<B, B> for Cpu {
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
    /// let gpu = chain.add(erase(Gpu));
    /// let cpu = chain.add(erase(Cpu));
    /// chain.register_fallback(gpu, cpu);
    /// // The GPU stage now has a fallback, so build() does not reject it.
    /// let pipeline = chain.build().unwrap();
    /// assert_eq!(pipeline.fallback_count(), 1);
    /// // The CPU twin is a substitution target, not a graph node.
    /// assert_eq!(pipeline.stage_count(), 1);
    /// ```
    pub fn register_fallback(&mut self, gpu_stage: StageId, cpu_stage: StageId) {
        self.fallbacks.push((gpu_stage, cpu_stage));
    }

    /// Topologically sorts the graph and compiles it into a [`Pipeline`].
    ///
    /// Performs, in order: fallback registration validation, edge type
    /// re-validation, a Kahn topological sort (which also detects cycles), and a
    /// weak-connectivity check over the non-fallback stages.
    ///
    /// Each [`Edge`]'s `from` and `to` are remapped to post-sort positions in
    /// [`Pipeline::stages()`]: `pipeline.stages()[from]` is the producer and
    /// `pipeline.stages()[to]` the consumer. The built pipeline drops the
    /// insertion-order [`StageId`]s.
    ///
    /// # Errors
    ///
    /// * [`BuildError::NoFallback`] — a GPU-only stage has no registered CPU
    ///   fallback.
    /// * [`BuildError::DuplicateFallback`] — the same GPU stage was registered
    ///   with more than one CPU fallback.
    /// * [`BuildError::FallbackRoleConflict`] — a single stage was registered
    ///   in both roles (as a GPU stage with its own fallback and as another
    ///   stage's CPU fallback target).
    /// * [`BuildError::FallbackForCpuStage`] — a fallback was registered for a
    ///   `CpuOnly` stage, which cannot OOM on the GPU.
    /// * [`BuildError::FallbackNotCpuCapable`] — a registered CPU fallback is a
    ///   `GpuOnly` stage and so cannot run on the CPU.
    /// * [`BuildError::FallbackTypeMismatch`] — a registered CPU fallback has
    ///   incompatible input or output types compared to the GPU stage it
    ///   substitutes.
    /// * [`BuildError::FallbackTargetHasEdge`] — a CPU fallback target was
    ///   [`connect`](Chain::connect)ed by a graph edge (on either end).
    /// * [`BuildError::TypeMismatch`] — an edge joins incompatible types.
    /// * [`BuildError::Cyclic`] — the graph contains a cycle.
    /// * [`BuildError::Disconnected`] — the graph is not a single
    ///   weakly-connected component (e.g. multiple disjoint roots/sinks), or a
    ///   fallback registration references an out-of-range stage id or reuses one
    ///   CPU fallback for more than one GPU stage. The offending id(s) are
    ///   listed in `stages`.
    pub fn build(mut self) -> Result<Pipeline, BuildError> {
        // The checks below back every index, `take()` and `new_index_of`
        // lookup of the materialisation at the end of this function.
        let n_stages = self.stages.len() as u32;

        // Bounds first, so all later indexing is safe.
        for &(gpu, cpu) in &self.fallbacks {
            if gpu.0 >= n_stages {
                return Err(BuildError::Disconnected { stages: vec![gpu] });
            }
            if cpu.0 >= n_stages {
                return Err(BuildError::Disconnected { stages: vec![cpu] });
            }
        }

        let registered_gpu: HashSet<StageId> = self.fallbacks.iter().map(|&(gpu, _)| gpu).collect();
        let fallback_targets: HashSet<StageId> =
            self.fallbacks.iter().map(|&(_, cpu)| cpu).collect();

        // A stage cannot play both roles (this covers `gpu == cpu`): a fallback
        // target never enters `new_index_of`, so it cannot also be a `gpu` key.
        // The lowest such id is reported.
        if let Some(&conflict) = {
            let mut overlap: Vec<&StageId> =
                registered_gpu.intersection(&fallback_targets).collect();
            overlap.sort();
            overlap.first().copied()
        } {
            return Err(BuildError::FallbackRoleConflict { stage: conflict });
        }

        let mut seen_gpu: HashSet<StageId> = HashSet::new();
        let mut seen_cpu: HashSet<StageId> = HashSet::new();
        for &(gpu, cpu) in &self.fallbacks {
            // A second registration would overwrite the first in the fallback
            // map while both CPU stages are moved out of `slots`.
            if !seen_gpu.insert(gpu) {
                return Err(BuildError::DuplicateFallback { gpu_stage: gpu });
            }
            // The materialiser moves each CPU fallback out of `slots` once.
            if !seen_cpu.insert(cpu) {
                return Err(BuildError::Disconnected { stages: vec![cpu] });
            }
            if matches!(
                self.stages[gpu.0 as usize].execution_class(),
                crate::stage::ExecutionClass::CpuOnly
            ) {
                return Err(BuildError::FallbackForCpuStage { gpu_stage: gpu });
            }
            if matches!(
                self.stages[cpu.0 as usize].execution_class(),
                crate::stage::ExecutionClass::GpuOnly
            ) {
                return Err(BuildError::FallbackNotCpuCapable { cpu_stage: cpu });
            }
            let gpu_in = self.stages[gpu.0 as usize].input_type();
            let gpu_out = self.stages[gpu.0 as usize].output_type();
            let cpu_in = self.stages[cpu.0 as usize].input_type();
            let cpu_out = self.stages[cpu.0 as usize].output_type();
            if gpu_in != cpu_in || gpu_out != cpu_out {
                return Err(BuildError::FallbackTypeMismatch {
                    gpu_stage: gpu,
                    cpu_stage: cpu,
                    gpu_input_type: gpu_in,
                    cpu_input_type: cpu_in,
                    gpu_output_type: gpu_out,
                    cpu_output_type: cpu_out,
                });
            }
        }

        for (idx, stage) in self.stages.iter().enumerate() {
            let id = StageId(idx as u32);
            if fallback_targets.contains(&id) {
                continue;
            }
            if matches!(
                stage.execution_class(),
                crate::stage::ExecutionClass::GpuOnly
            ) && !registered_gpu.contains(&id)
            {
                return Err(BuildError::NoFallback { gpu_stage: id });
            }
        }

        // Edge types, as in `connect`; an out-of-range endpoint is reported as
        // disconnected.
        for edge in &self.edges {
            let from = self
                .stage(edge.from)
                .ok_or_else(|| BuildError::Disconnected {
                    stages: vec![edge.from],
                })?;
            let to = self
                .stage(edge.to)
                .ok_or_else(|| BuildError::Disconnected {
                    stages: vec![edge.to],
                })?;
            let from_type = from.output_type();
            let to_type = to.input_type();
            if from_type != to_type {
                return Err(BuildError::TypeMismatch {
                    from_stage: edge.from,
                    from_type,
                    to_stage: edge.to,
                    to_type,
                });
            }
        }

        // A fallback target is outside `graph_nodes`, so an edge incident to it
        // has no post-sort endpoint. The lowest-id offending edge is reported.
        if let Some((stage, edge_peer)) = self
            .edges
            .iter()
            .filter_map(|e| {
                if fallback_targets.contains(&e.from) {
                    Some((e.from, e.to))
                } else if fallback_targets.contains(&e.to) {
                    Some((e.to, e.from))
                } else {
                    None
                }
            })
            .min()
        {
            return Err(BuildError::FallbackTargetHasEdge { stage, edge_peer });
        }

        let graph_nodes: Vec<StageId> = (0..self.stages.len() as u32)
            .map(StageId)
            .filter(|id| !fallback_targets.contains(id))
            .collect();

        let order = topological_order(&graph_nodes, &self.edges, &fallback_targets)?;
        if let Some(disconnected) = weakly_disconnected(&graph_nodes, &self.edges) {
            return Err(BuildError::Disconnected {
                stages: disconnected,
            });
        }

        let config = self.config.take().unwrap_or_else(default_pipeline_config);

        let mut slots: Vec<Option<Box<dyn AnyStage>>> = self.stages.into_iter().map(Some).collect();

        let ordered_stages: Vec<Box<dyn AnyStage>> = order
            .iter()
            .map(|id| {
                slots[id.0 as usize]
                    .take()
                    .expect("topo order references each graph node exactly once")
            })
            .collect();

        // `order[new_pos]` is the insertion-order StageId, so
        // `new_index_of[old_id] = new_pos`.
        let new_index_of: HashMap<StageId, StageId> = order
            .iter()
            .enumerate()
            .map(|(new_pos, &old_id)| (old_id, StageId(new_pos as u32)))
            .collect();

        let fallback_map: HashMap<StageId, Box<dyn AnyStage>> = self
            .fallbacks
            .iter()
            .map(|&(gpu, cpu)| {
                let stage = slots[cpu.0 as usize]
                    .take()
                    .expect("a fallback target is taken exactly once");
                // Keyed by the GPU stage's post-sort position in `stages()`.
                let new_gpu = *new_index_of
                    .get(&gpu)
                    .expect("a GPU fallback key is a graph node (no role overlap)");
                (new_gpu, stage)
            })
            .collect();

        let ordered_edges: Vec<Edge> = self
            .edges
            .into_iter()
            .map(|e| Edge {
                from: *new_index_of
                    .get(&e.from)
                    .expect("edge endpoints are graph nodes (no fallback-target edges)"),
                to: *new_index_of
                    .get(&e.to)
                    .expect("edge endpoints are graph nodes (no fallback-target edges)"),
                element_type: e.element_type,
                batch_size: e.batch_size,
            })
            .collect();

        Ok(Pipeline::from_parts(
            ordered_stages,
            ordered_edges,
            fallback_map,
            config,
        ))
    }

    fn stage(&self, id: StageId) -> Option<&dyn AnyStage> {
        self.stages.get(id.0 as usize).map(|b| b.as_ref())
    }
}

/// Computes a Kahn topological order of `nodes`, treating only edges between two
/// graph nodes (endpoints not in `fallback_targets`) as graph edges.
///
/// Returns [`BuildError::Cyclic`] listing the unscheduled nodes if a cycle
/// prevents a complete ordering.
fn topological_order(
    nodes: &[StageId],
    edges: &[Edge],
    fallback_targets: &HashSet<StageId>,
) -> Result<Vec<StageId>, BuildError> {
    let node_set: HashSet<StageId> = nodes.iter().copied().collect();

    let mut indegree: HashMap<StageId, usize> = nodes.iter().map(|&n| (n, 0)).collect();
    let mut adj: HashMap<StageId, Vec<StageId>> = nodes.iter().map(|&n| (n, Vec::new())).collect();
    for e in edges {
        if fallback_targets.contains(&e.from) || fallback_targets.contains(&e.to) {
            continue;
        }
        if !node_set.contains(&e.from) || !node_set.contains(&e.to) {
            continue;
        }
        adj.get_mut(&e.from)
            .expect("from is a graph node")
            .push(e.to);
        *indegree.get_mut(&e.to).expect("to is a graph node") += 1;
    }

    // Zero-in-degree nodes ascending by id, for a deterministic order.
    let mut ready: VecDeque<StageId> = {
        let mut seeds: Vec<StageId> = nodes.iter().copied().filter(|n| indegree[n] == 0).collect();
        seeds.sort();
        seeds.into_iter().collect()
    };

    let mut order: Vec<StageId> = Vec::with_capacity(nodes.len());
    while let Some(n) = ready.pop_front() {
        order.push(n);
        let mut newly_ready: Vec<StageId> = Vec::new();
        for &succ in &adj[&n] {
            let d = indegree.get_mut(&succ).expect("successor is a graph node");
            *d -= 1;
            if *d == 0 {
                newly_ready.push(succ);
            }
        }
        newly_ready.sort();
        for s in newly_ready {
            ready.push_back(s);
        }
    }

    if order.len() != nodes.len() {
        // The unscheduled nodes lie on or downstream of a cycle.
        let scheduled: HashSet<StageId> = order.iter().copied().collect();
        let mut involved: Vec<StageId> = nodes
            .iter()
            .copied()
            .filter(|n| !scheduled.contains(n))
            .collect();
        involved.sort();
        return Err(BuildError::Cyclic { involved });
    }

    Ok(order)
}

/// Returns `Some(stages)` if the undirected graph over `nodes` is not a single
/// connected component, where `stages` is the (sorted) set of nodes outside the
/// component containing the lowest-id node.
///
/// An empty or single-node graph is connected (returns `None`).
fn weakly_disconnected(nodes: &[StageId], edges: &[Edge]) -> Option<Vec<StageId>> {
    if nodes.len() <= 1 {
        return None;
    }
    let node_set: HashSet<StageId> = nodes.iter().copied().collect();

    let mut adj: HashMap<StageId, Vec<StageId>> = nodes.iter().map(|&n| (n, Vec::new())).collect();
    for e in edges {
        if !node_set.contains(&e.from) || !node_set.contains(&e.to) {
            continue;
        }
        adj.get_mut(&e.from).expect("from is a node").push(e.to);
        adj.get_mut(&e.to).expect("to is a node").push(e.from);
    }

    let start = *nodes.iter().min().expect("non-empty");
    let mut seen: HashSet<StageId> = HashSet::new();
    let mut queue: VecDeque<StageId> = VecDeque::new();
    seen.insert(start);
    queue.push_back(start);
    while let Some(n) = queue.pop_front() {
        for &nbr in &adj[&n] {
            if seen.insert(nbr) {
                queue.push_back(nbr);
            }
        }
    }

    if seen.len() == nodes.len() {
        None
    } else {
        let mut outside: Vec<StageId> = nodes
            .iter()
            .copied()
            .filter(|n| !seen.contains(n))
            .collect();
        outside.sort();
        Some(outside)
    }
}

/// The [`PipelineConfig`] applied when a [`Chain`] is built without an
/// explicit config: single worker, empty SNR sweep, no checkpointing.
fn default_pipeline_config() -> PipelineConfig {
    PipelineConfig {
        seed: 0,
        esn0_db_points: Vec::new(),
        target_errors: 0,
        max_frames: 0,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: std::num::NonZeroUsize::new(1).expect("1 is non-zero"),
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::{BitPackedBatch, LlrBatch, SymbolBatch};
    use crate::error::StageError;
    use crate::stage::{erase, ExecutionClass, Stage};
    use gf2_core::BitVec;

    struct BitId;
    impl Stage<BitPackedBatch, BitPackedBatch> for BitId {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &BitPackedBatch, _: &mut ()) -> Result<BitPackedBatch, StageError> {
            Ok(i.clone())
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

    struct BitToSym;
    impl Stage<BitPackedBatch, SymbolBatch> for BitToSym {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &BitPackedBatch, _: &mut ()) -> Result<SymbolBatch, StageError> {
            let n = i.frames.len();
            Ok(SymbolBatch::new(vec![vec![]; n], vec![vec![]; n]))
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

    struct SymToLlr;
    impl Stage<SymbolBatch, LlrBatch> for SymToLlr {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &SymbolBatch, _: &mut ()) -> Result<LlrBatch, StageError> {
            Ok(LlrBatch::new(vec![vec![]; i.i.len()]))
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

    struct GpuBitId;
    impl Stage<BitPackedBatch, BitPackedBatch> for GpuBitId {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &BitPackedBatch, _: &mut ()) -> Result<BitPackedBatch, StageError> {
            Ok(i.clone())
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::GpuOnly
        }
    }

    struct HybridBitId;
    impl Stage<BitPackedBatch, BitPackedBatch> for HybridBitId {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &BitPackedBatch, _: &mut ()) -> Result<BitPackedBatch, StageError> {
            Ok(i.clone())
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::Hybrid
        }
    }

    #[test]
    fn test_add_assigns_sequential_ids() {
        let mut chain = Chain::new();
        let a = chain.add(erase(BitId));
        let b = chain.add(erase(BitId));
        assert_eq!(a, StageId(0));
        assert_eq!(b, StageId(1));
    }

    #[test]
    fn test_connect_compatible_types_records_edge() {
        let mut chain = Chain::new();
        let a = chain.add(erase(BitToSym));
        let b = chain.add(erase(SymToLlr));
        chain.connect(a, b).expect("Symbol → Symbol is compatible");
        let pipeline = chain.build().expect("valid linear chain");
        assert_eq!(pipeline.stage_count(), 2);
        assert_eq!(pipeline.edges().len(), 1);
    }

    #[test]
    fn test_connect_incompatible_types_is_type_mismatch() {
        let mut chain = Chain::new();
        let s = chain.add(erase(SymToLlr));
        let b = chain.add(erase(BitId));
        match chain.connect(s, b) {
            Err(BuildError::TypeMismatch {
                from_stage,
                to_stage,
                ..
            }) => {
                assert_eq!(from_stage, s);
                assert_eq!(to_stage, b);
            }
            other => panic!("expected TypeMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_connect_unknown_id_is_disconnected() {
        let mut chain = Chain::new();
        let a = chain.add(erase(BitId));
        match chain.connect(a, StageId(99)) {
            Err(BuildError::Disconnected { stages }) => assert_eq!(stages, vec![StageId(99)]),
            other => panic!("expected Disconnected, got {other:?}"),
        }
    }

    #[test]
    fn test_build_detects_cycle() {
        let mut chain = Chain::new();
        let a = chain.add(erase(BitId));
        let b = chain.add(erase(BitId));
        chain.connect(a, b).unwrap();
        chain.connect(b, a).unwrap();
        match chain.build() {
            Err(BuildError::Cyclic { involved }) => {
                assert_eq!(involved, vec![a, b]);
            }
            Err(other) => panic!("expected Cyclic, got {other:?}"),
            Ok(_) => panic!("expected Cyclic, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_detects_disconnected_components() {
        let mut chain = Chain::new();
        let a = chain.add(erase(BitId));
        let b = chain.add(erase(BitId));
        chain.connect(a, b).unwrap();
        let c = chain.add(erase(BitId));
        let d = chain.add(erase(BitId));
        chain.connect(c, d).unwrap();
        match chain.build() {
            Err(BuildError::Disconnected { stages }) => {
                // The reported set is the complement of the component holding
                // the lowest id.
                assert_eq!(stages, vec![c, d]);
            }
            Err(other) => panic!("expected Disconnected, got {other:?}"),
            Ok(_) => panic!("expected Disconnected, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_rejects_gpu_stage_without_fallback() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        match chain.build() {
            Err(BuildError::NoFallback { gpu_stage }) => assert_eq!(gpu_stage, g),
            Err(other) => panic!("expected NoFallback, got {other:?}"),
            Ok(_) => panic!("expected NoFallback, got a built pipeline"),
        }
    }

    #[test]
    fn test_register_fallback_satisfies_gpu_stage_and_extracts_cpu() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let cpu = chain.add(erase(BitId));
        chain.register_fallback(g, cpu);
        let pipeline = chain.build().expect("gpu stage now has a fallback");
        assert_eq!(pipeline.stage_count(), 1);
        assert_eq!(pipeline.fallback_count(), 1);
    }

    #[test]
    fn test_build_rejects_duplicate_fallback_target() {
        let mut chain = Chain::new();
        let g1 = chain.add(erase(GpuBitId));
        let g2 = chain.add(erase(GpuBitId));
        let cpu = chain.add(erase(BitId));
        chain.register_fallback(g1, cpu);
        chain.register_fallback(g2, cpu);
        match chain.build() {
            Err(BuildError::Disconnected { stages }) => assert_eq!(stages, vec![cpu]),
            Err(other) => panic!("expected Disconnected for duplicate fallback, got {other:?}"),
            Ok(_) => panic!("expected Disconnected, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_rejects_out_of_range_fallback_id() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let bogus = StageId(99);
        chain.register_fallback(g, bogus);
        match chain.build() {
            Err(BuildError::Disconnected { stages }) => assert_eq!(stages, vec![bogus]),
            Err(other) => panic!("expected Disconnected for out-of-range id, got {other:?}"),
            Ok(_) => panic!("expected Disconnected, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_branching_dag_topological_order() {
        let mut chain = Chain::new();
        let a = chain.add(erase(BitId));
        let b = chain.add(erase(BitId));
        let c = chain.add(erase(BitId));
        let d = chain.add(erase(BitId));
        chain.connect(a, b).unwrap();
        chain.connect(a, c).unwrap();
        chain.connect(b, d).unwrap();
        chain.connect(c, d).unwrap();
        let pipeline = chain.build().expect("valid DAG");
        assert_eq!(pipeline.stage_count(), 4);
        assert_eq!(pipeline.edges().len(), 4);
    }

    #[test]
    fn test_empty_chain_builds_empty_pipeline() {
        let chain = Chain::new();
        let pipeline = chain.build().expect("empty chain is vacuously valid");
        assert_eq!(pipeline.stage_count(), 0);
    }

    #[test]
    fn test_single_stage_chain_is_connected() {
        let mut chain = Chain::new();
        chain.add(erase(BitId));
        let pipeline = chain.build().expect("a single stage is connected");
        assert_eq!(pipeline.stage_count(), 1);
    }

    #[test]
    fn test_bitid_processes_a_real_batch() {
        let s = BitId;
        let out = s
            .process(&BitPackedBatch::new(vec![BitVec::zeros(8)]), &mut ())
            .unwrap();
        assert_eq!(out.frames[0].len(), 8);
    }

    #[test]
    fn test_edge_positions_remapped_after_non_topo_insertion() {
        // Insertion: C = StageId(0), M = StageId(1), P = StageId(2), with
        // P → M → C. Post-sort positions: P→0, M→1, C→2.
        let mut chain = Chain::new();
        let c = chain.add(erase(SymToLlr));
        let m = chain.add(erase(BitToSym));
        let p = chain.add(erase(BitId));
        chain.connect(p, m).unwrap();
        chain.connect(m, c).unwrap();
        let pipeline = chain.build().expect("valid non-topo-inserted chain");

        assert_eq!(pipeline.stage_count(), 3);
        assert_eq!(pipeline.edges().len(), 2);

        use crate::batch::{BitPackedBatch, LlrBatch, SymbolBatch};
        use std::any::TypeId;
        let stages = pipeline.stages();
        assert_eq!(stages[0].input_type(), TypeId::of::<BitPackedBatch>());
        assert_eq!(stages[0].output_type(), TypeId::of::<BitPackedBatch>());
        assert_eq!(stages[1].input_type(), TypeId::of::<BitPackedBatch>());
        assert_eq!(stages[1].output_type(), TypeId::of::<SymbolBatch>());
        assert_eq!(stages[2].input_type(), TypeId::of::<SymbolBatch>());
        assert_eq!(stages[2].output_type(), TypeId::of::<LlrBatch>());

        let edges = pipeline.edges();
        let mut sorted_edges = edges.to_vec();
        sorted_edges.sort_by_key(|e| e.from);

        assert_eq!(
            sorted_edges[0].from,
            crate::connector::StageId(0),
            "P→M edge from must be position 0 (P)"
        );
        assert_eq!(
            sorted_edges[0].to,
            crate::connector::StageId(1),
            "P→M edge to must be position 1 (M)"
        );
        assert_eq!(
            sorted_edges[1].from,
            crate::connector::StageId(1),
            "M→C edge from must be position 1 (M)"
        );
        assert_eq!(
            sorted_edges[1].to,
            crate::connector::StageId(2),
            "M→C edge to must be position 2 (C)"
        );
    }

    #[test]
    fn test_build_rejects_duplicate_gpu_fallback_registration() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let cpu1 = chain.add(erase(BitId));
        let cpu2 = chain.add(erase(BitId));
        chain.register_fallback(g, cpu1);
        chain.register_fallback(g, cpu2);
        match chain.build() {
            Err(BuildError::DuplicateFallback { gpu_stage }) => {
                assert_eq!(gpu_stage, g);
            }
            Err(other) => {
                panic!("expected DuplicateFallback for duplicate GPU registration, got {other:?}")
            }
            Ok(_) => panic!("expected DuplicateFallback, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_rejects_type_incompatible_fallback() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let wrong_cpu = chain.add(erase(SymToLlr));
        chain.register_fallback(g, wrong_cpu);
        match chain.build() {
            Err(BuildError::FallbackTypeMismatch {
                gpu_stage,
                cpu_stage,
                ..
            }) => {
                assert_eq!(gpu_stage, g);
                assert_eq!(cpu_stage, wrong_cpu);
            }
            Err(other) => {
                panic!("expected FallbackTypeMismatch for incompatible fallback, got {other:?}")
            }
            Ok(_) => panic!("expected FallbackTypeMismatch, got a built pipeline"),
        }
    }

    /// The overlapping stage is Hybrid so the role-overlap check fires before
    /// the capability checks.
    #[test]
    fn test_build_rejects_fallback_role_overlap_without_panic() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let x = chain.add(erase(HybridBitId));
        let c = chain.add(erase(BitId));
        chain.register_fallback(g, x);
        chain.register_fallback(x, c);
        match chain.build() {
            Err(BuildError::FallbackRoleConflict { stage }) => assert_eq!(stage, x),
            Err(other) => panic!("expected FallbackRoleConflict for role overlap, got {other:?}"),
            Ok(_) => panic!("expected FallbackRoleConflict, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_rejects_gpu_only_stage_as_cpu_fallback() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let bad_cpu = chain.add(erase(GpuBitId));
        chain.register_fallback(g, bad_cpu);
        match chain.build() {
            Err(BuildError::FallbackNotCpuCapable { cpu_stage }) => {
                assert_eq!(cpu_stage, bad_cpu);
            }
            Err(other) => {
                panic!("expected FallbackNotCpuCapable for GpuOnly fallback, got {other:?}")
            }
            Ok(_) => panic!("expected FallbackNotCpuCapable, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_rejects_fallback_for_cpu_only_stage() {
        let mut chain = Chain::new();
        let cpu_gpu = chain.add(erase(BitId));
        let cpu_fb = chain.add(erase(BitId));
        chain.register_fallback(cpu_gpu, cpu_fb);
        match chain.build() {
            Err(BuildError::FallbackForCpuStage { gpu_stage }) => {
                assert_eq!(gpu_stage, cpu_gpu);
            }
            Err(other) => panic!("expected FallbackForCpuStage for CpuOnly stage, got {other:?}"),
            Ok(_) => panic!("expected FallbackForCpuStage, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_accepts_hybrid_stage_and_hybrid_fallback() {
        let mut chain = Chain::new();
        let gpu = chain.add(erase(HybridBitId));
        let cpu = chain.add(erase(HybridBitId));
        chain.register_fallback(gpu, cpu);
        let pipeline = chain
            .build()
            .expect("hybrid gpu + hybrid fallback is valid");
        assert_eq!(pipeline.stage_count(), 1);
        assert_eq!(pipeline.fallback_count(), 1);
    }

    #[test]
    fn test_build_rejects_edge_into_fallback_target() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let x = chain.add(erase(BitId));
        let c = chain.add(erase(BitId));
        chain.register_fallback(g, c);
        chain.connect(x, c).unwrap();
        match chain.build() {
            Err(BuildError::FallbackTargetHasEdge { stage, edge_peer }) => {
                assert_eq!(stage, c, "the fallback target is the offending stage");
                assert_eq!(edge_peer, x, "the other endpoint is the producer x");
            }
            Err(other) => panic!("expected FallbackTargetHasEdge, got {other:?}"),
            Ok(_) => panic!("expected FallbackTargetHasEdge, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_rejects_edge_out_of_fallback_target() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let c = chain.add(erase(BitId));
        let y = chain.add(erase(BitId));
        chain.register_fallback(g, c);
        chain.connect(c, y).unwrap();
        match chain.build() {
            Err(BuildError::FallbackTargetHasEdge { stage, edge_peer }) => {
                assert_eq!(stage, c, "the fallback target is the offending stage");
                assert_eq!(edge_peer, y, "the other endpoint is the consumer y");
            }
            Err(other) => panic!("expected FallbackTargetHasEdge, got {other:?}"),
            Ok(_) => panic!("expected FallbackTargetHasEdge, got a built pipeline"),
        }
    }

    #[test]
    fn test_build_accepts_fallback_with_no_incident_edge_preserving_graph_edge() {
        let mut chain = Chain::new();
        let src = chain.add(erase(BitId));
        let g = chain.add(erase(GpuBitId));
        let c = chain.add(erase(BitId));
        chain.connect(src, g).unwrap();
        chain.register_fallback(g, c);
        let pipeline = chain
            .build()
            .expect("fallback target with no incident edge is valid");
        assert_eq!(pipeline.stage_count(), 2);
        assert_eq!(pipeline.fallback_count(), 1);
        assert_eq!(
            pipeline.edges().len(),
            1,
            "the real graph edge is preserved"
        );
    }

    #[test]
    fn test_build_rejects_self_fallback() {
        let mut chain = Chain::new();
        let g = chain.add(erase(HybridBitId)); // Hybrid so capability checks pass
        chain.register_fallback(g, g);
        match chain.build() {
            Err(BuildError::FallbackRoleConflict { stage }) => assert_eq!(stage, g),
            Err(other) => panic!("expected FallbackRoleConflict for self-fallback, got {other:?}"),
            Ok(_) => panic!("expected FallbackRoleConflict, got a built pipeline"),
        }
    }

    #[test]
    fn test_chain_default_is_same_as_new() {
        let chain = Chain::default();
        let pipeline = chain.build().unwrap();
        assert_eq!(
            pipeline.stage_count(),
            0,
            "default chain builds an empty pipeline"
        );
    }
}
