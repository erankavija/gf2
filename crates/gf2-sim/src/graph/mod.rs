//! Graph-based chain construction API (design doc §9, "Graph API").
//!
//! Owned by `c09d3e95`. This module provides [`Chain`], the low-level
//! graph-builder surface for composing **novel** stage topologies that the
//! typestate DVB-T2 / 5G-NR presets (`81d05bab`) wrap. A caller [`Chain::add`]s
//! type-erased stages, [`Chain::connect`]s producers to consumers (with a
//! runtime connector type-check), optionally registers GPU→CPU OOM fallbacks
//! ([`Chain::register_fallback`], design doc §8), and finally
//! [`Chain::build`]s a topologically ordered [`Pipeline`].
//!
//! # Branching DAGs
//!
//! A stage may have multiple outgoing edges (fan-out) and multiple incoming
//! edges (fan-in). [`Chain::build`] produces a [`Pipeline`] whose stage list is
//! a valid topological order of the DAG; the Phase C executor (`de160fc5`)
//! consumes that order later. This task only *produces* a correctly ordered
//! pipeline — it does not execute it.
//!
//! # What `build()` checks
//!
//! 1. **Fallbacks** — every GPU-only stage must have a registered CPU fallback,
//!    else [`BuildError::NoFallback`] (design doc §8).
//! 2. **Type compatibility** — every edge's producer `output_type()` must match
//!    its consumer `input_type()`, else [`BuildError::TypeMismatch`]. (Already
//!    enforced eagerly by [`Chain::connect`]; re-checked at build for edges
//!    added by other paths.)
//! 3. **Acyclicity** — a cycle yields [`BuildError::Cyclic`].
//! 4. **Connectivity** — the graph (over non-fallback stages) must be a single
//!    weakly-connected component; multiple disjoint roots/sinks yield
//!    [`BuildError::Disconnected`].

use std::collections::{HashMap, HashSet, VecDeque};

use crate::connector::{Edge, StageId};
use crate::error::BuildError;
use crate::pipeline::Pipeline;
use crate::stage::AnyStage;
use crate::PipelineConfig;

/// The default per-edge batch size recorded on [`Edge`]s minted by
/// [`Chain::connect`].
///
/// The graph API joins stages by identity, not by negotiated buffer size, so a
/// neutral non-zero default is recorded. Presets that care about exact SoA
/// buffer sizing set per-edge sizes through their own wiring; the executor
/// (`de160fc5`) reads [`Edge::batch_size`] when it allocates buffers.
const DEFAULT_EDGE_BATCH_SIZE: usize = 1;

/// A mutable builder for a stage graph that compiles into a [`Pipeline`].
///
/// Holds the type-erased stages, the directed edges joining them, and the
/// GPU→CPU fallback registrations. See the [module docs](self) for the build
/// contract.
///
/// # Examples
///
/// A minimal two-stage chain built, validated, and compiled to a [`Pipeline`]:
///
/// ```
/// use gf2_sim::graph::Chain;
/// use gf2_sim::stage::{erase, BatchSize, ExecutionClass, Stage};
/// use gf2_sim::error::StageError;
///
/// // Two distinct batch newtypes so the connector type-check has something to
/// // verify.
/// #[derive(Clone)]
/// struct Bits(Vec<u8>);
/// impl BatchSize for Bits {
///     fn batch_size(&self) -> usize {
///         self.0.len()
///     }
/// }
/// #[derive(Clone)]
/// struct Syms(Vec<u8>);
/// impl BatchSize for Syms {
///     fn batch_size(&self) -> usize {
///         self.0.len()
///     }
/// }
///
/// struct Modulate;
/// impl Stage<Bits, Syms> for Modulate {
///     type Scratch = ();
///     type CpuFallback = Self;
///     fn process(&self, input: &Bits, _: &mut ()) -> Result<Syms, StageError> {
///         Ok(Syms(input.0.clone()))
///     }
///     fn execution_class(&self) -> ExecutionClass {
///         ExecutionClass::CpuOnly
///     }
/// }
///
/// struct Sink;
/// impl Stage<Syms, Syms> for Sink {
///     type Scratch = ();
///     type CpuFallback = Self;
///     fn process(&self, input: &Syms, _: &mut ()) -> Result<Syms, StageError> {
///         Ok(input.clone())
///     }
///     fn execution_class(&self) -> ExecutionClass {
///         ExecutionClass::CpuOnly
///     }
/// }
///
/// let mut chain = Chain::new();
/// let a = chain.add(erase(Modulate));
/// let b = chain.add(erase(Sink));
/// chain.connect(a, b).unwrap();
/// let pipeline = chain.build().unwrap();
/// assert_eq!(pipeline.stage_count(), 2);
/// ```
///
/// ## A DVB-T2 BICM chain, constructed step by step
///
/// The same graph API expresses a full DVB-T2 BICM transmit+receive chain.
/// [`dvb_t2_bicm_stages`](crate::stages::dvb_t2_bicm_stages) hands back the
/// forward stages (BCH+LDPC encode → bit-interleave → Gray-QAM map) and the
/// inverse stages (Gray-QAM demap → bit-deinterleave → LDPC decode) already
/// type-erased; we `add` each in order, then `connect` them consecutively. The
/// forward→inverse hop is a noiseless `SymbolBatch` pass-through, type-checked
/// like every other edge, and `build()` topologically orders the six stages.
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
    /// Type-erased stages indexed by their [`StageId`] (`StageId(i)` ⇒
    /// `stages[i]`).
    stages: Vec<Box<dyn AnyStage>>,
    /// Directed producer→consumer edges.
    edges: Vec<Edge>,
    /// `(gpu_stage, cpu_fallback_stage)` registrations (design doc §8).
    fallbacks: Vec<(StageId, StageId)>,
    /// Optional run configuration applied to the built [`Pipeline`].
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
    /// The built [`Pipeline`] receives a neutral default [`PipelineConfig`]
    /// (single worker, no SNR sweep) unless one is supplied via
    /// [`Chain::with_config`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::graph::Chain;
    ///
    /// let chain = Chain::new();
    /// // A fresh chain compiles to an empty (zero-stage) pipeline.
    /// assert_eq!(chain.build().unwrap().stage_count(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            edges: Vec::new(),
            fallbacks: Vec::new(),
            config: None,
        }
    }

    /// Sets the [`PipelineConfig`] the built [`Pipeline`] will carry.
    ///
    /// # Arguments
    ///
    /// * `config` — the run configuration to attach to the built pipeline.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroUsize;
    /// use gf2_sim::graph::Chain;
    /// use gf2_sim::PipelineConfig;
    ///
    /// let cfg = PipelineConfig {
    ///     seed: 7,
    ///     esn0_db_points: vec![4.0],
    ///     target_errors: 100,
    ///     max_frames: 1000,
    ///     heartbeat_every_frames: 0,
    ///     checkpoint_dir: None,
    ///     tracing_log_path: None,
    ///     parallelism: NonZeroUsize::new(1).unwrap(),
    ///     strict_gpu: false,
    /// };
    /// let chain = Chain::new().with_config(cfg);
    /// let pipeline = chain.build().unwrap();
    /// assert_eq!(pipeline.config().seed, 7);
    /// ```
    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Adds a type-erased stage and returns its [`StageId`].
    ///
    /// The argument is a `Box<dyn AnyStage>`: a concrete `Stage<I, O>` is erased
    /// to this form via [`erase`](crate::stage::erase). Taking the already-erased
    /// box (rather than a generic `Stage<I, O>`) keeps the signature object-safe
    /// and lets a chain hold a heterogeneous stage list, exactly as the
    /// [`Pipeline`] does; it is also what the foundation's
    /// [`dvb_t2_bicm_stages`](crate::stages::dvb_t2_bicm_stages) factory already
    /// hands back.
    ///
    /// # Arguments
    ///
    /// * `stage` — the erased stage to insert.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::graph::Chain;
    /// use gf2_sim::stages::{DvbT2Encode};
    /// use gf2_sim::stage::erase;
    /// use std::sync::Arc;
    /// use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    ///
    /// let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());
    /// let mut chain = Chain::new();
    /// let id = chain.add(erase(DvbT2Encode::new(codec)));
    /// assert_eq!(id.0, 0);
    /// ```
    pub fn add(&mut self, stage: Box<dyn AnyStage>) -> StageId {
        let id = StageId(self.stages.len() as u32);
        self.stages.push(stage);
        id
    }

    /// Connects a producer stage to a consumer stage, type-checking at runtime.
    ///
    /// Compares the producing stage's [`output_type()`](AnyStage::output_type)
    /// against the consuming stage's [`input_type()`](AnyStage::input_type); a
    /// mismatch returns [`BuildError::TypeMismatch`] and records no edge. On a
    /// match, a directed [`Edge`] is recorded.
    ///
    /// # Arguments
    ///
    /// * `from` — the producing stage.
    /// * `to` — the consuming stage.
    ///
    /// # Errors
    ///
    /// * [`BuildError::TypeMismatch`] if the producer output type and consumer
    ///   input type differ.
    /// * [`BuildError::Disconnected`] if either id refers to no added stage
    ///   (an unknown id cannot participate in a connected graph).
    ///
    /// # Examples
    ///
    /// Connecting a `SymbolBatch` producer into a `BitPackedBatch` consumer is a
    /// type error:
    ///
    /// ```
    /// use gf2_sim::graph::Chain;
    /// use gf2_sim::stages::{GrayQamMap, DvbT2Encode};
    /// use gf2_sim::stage::erase;
    /// use gf2_sim::error::BuildError;
    /// use std::sync::Arc;
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    /// use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    ///
    /// let codec = Arc::new(DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap());
    /// let mut chain = Chain::new();
    /// // GrayQamMap outputs SymbolBatch; DvbT2Encode consumes BitPackedBatch.
    /// let map = chain.add(erase(GrayQamMap::new(DvbT2Modulation::Qam16)));
    /// let enc = chain.add(erase(DvbT2Encode::new(codec)));
    /// assert!(matches!(chain.connect(map, enc), Err(BuildError::TypeMismatch { .. })));
    /// ```
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

    /// Registers a CPU fallback stage for a GPU stage (design doc §8).
    ///
    /// On GPU out-of-memory the executor (`42eac5cc`) substitutes the registered
    /// CPU stage on the offending batch. [`Chain::build`] moves `cpu_stage` out
    /// of the topologically-ordered stage list and into the pipeline's fallback
    /// table keyed by `gpu_stage`; the CPU fallback is therefore **not** a node
    /// in the DAG (it is a substitution target, reachable only on OOM).
    ///
    /// # Arguments
    ///
    /// * `gpu_stage` — the GPU stage that may OOM.
    /// * `cpu_stage` — the CPU stage to substitute (must already be added; it is
    ///   not separately connected by edges).
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
        // Records the pairing only; validity (in-range ids, each CPU target used
        // for exactly one GPU stage) is checked by [`Chain::build`], which
        // returns [`BuildError::Disconnected`] for a malformed registration
        // rather than panicking.
        self.fallbacks.push((gpu_stage, cpu_stage));
    }

    /// Topologically sorts the graph and compiles it into a [`Pipeline`].
    ///
    /// Performs, in order: the GPU-fallback presence check, an edge type
    /// re-validation, a Kahn topological sort (which also detects cycles), and a
    /// weak-connectivity check over the non-fallback stages. Branching DAGs
    /// (fan-out / fan-in) are supported; the resulting stage order is a valid
    /// linearisation of the DAG.
    ///
    /// # Errors
    ///
    /// * [`BuildError::NoFallback`] — a GPU-only stage has no registered CPU
    ///   fallback.
    /// * [`BuildError::TypeMismatch`] — an edge joins incompatible types.
    /// * [`BuildError::Cyclic`] — the graph contains a cycle.
    /// * [`BuildError::Disconnected`] — the graph is not a single
    ///   weakly-connected component (e.g. multiple disjoint roots/sinks), or a
    ///   fallback registration references an out-of-range stage id or reuses one
    ///   CPU fallback for more than one GPU stage. The offending id(s) are
    ///   listed in `stages`.
    ///
    /// # Examples
    ///
    /// A linear three-stage chain compiles to a three-stage pipeline:
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
    /// let c = chain.add(erase(Id));
    /// chain.connect(a, b).unwrap();
    /// chain.connect(b, c).unwrap();
    /// let pipeline = chain.build().unwrap();
    /// assert_eq!(pipeline.stage_count(), 3);
    /// ```
    pub fn build(mut self) -> Result<Pipeline, BuildError> {
        // 0. Validate fallback registrations up front so malformed input fails
        //    via `Result` rather than panicking in the materialisation step
        //    below (which indexes `slots[cpu]` and `take()`s each target once).
        //    `register_fallback` performs no validation, so both an out-of-range
        //    id and a CPU target reused across multiple GPU stages are reachable
        //    here. Each referenced stage id must be in range, and each CPU
        //    fallback target must back exactly one GPU stage.
        let n_stages = self.stages.len() as u32;
        let mut seen_cpu: HashSet<StageId> = HashSet::new();
        for &(gpu, cpu) in &self.fallbacks {
            if gpu.0 >= n_stages {
                return Err(BuildError::Disconnected { stages: vec![gpu] });
            }
            if cpu.0 >= n_stages {
                return Err(BuildError::Disconnected { stages: vec![cpu] });
            }
            if !seen_cpu.insert(cpu) {
                // A CPU fallback backs exactly one GPU stage; otherwise the
                // materialiser would move the same boxed stage out twice.
                return Err(BuildError::Disconnected { stages: vec![cpu] });
            }
        }

        // 1. Fallback presence: every GPU-only stage needs a registered CPU
        //    fallback. The set of stages that ARE fallbacks is excluded from the
        //    graph (they are substitution targets, reachable only on OOM).
        let fallback_targets: HashSet<StageId> =
            self.fallbacks.iter().map(|&(_, cpu)| cpu).collect();
        let registered_gpu: HashSet<StageId> = self.fallbacks.iter().map(|&(gpu, _)| gpu).collect();

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

        // 2. Re-validate every edge's type compatibility (connect() already
        //    enforces this, but build() is the authoritative gate). Edges whose
        //    endpoints are out of range are treated as disconnected.
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

        // The graph nodes are every added stage that is NOT a fallback target.
        let graph_nodes: Vec<StageId> = (0..self.stages.len() as u32)
            .map(StageId)
            .filter(|id| !fallback_targets.contains(id))
            .collect();

        // 3. Topological sort (Kahn) over graph_nodes, also detecting cycles.
        let order = topological_order(&graph_nodes, &self.edges, &fallback_targets)?;

        // 4. Connectivity: the non-fallback graph must be a single
        //    weakly-connected component. An empty graph is vacuously connected.
        if let Some(disconnected) = weakly_disconnected(&graph_nodes, &self.edges) {
            return Err(BuildError::Disconnected {
                stages: disconnected,
            });
        }

        // Materialise the pipeline: move stages into topo order, split fallbacks
        // into the keyed map, and keep edges among graph nodes.
        let config = self.config.take().unwrap_or_else(default_pipeline_config);

        // Move every stage out of `self.stages` into an indexable slot so we can
        // relocate by id without cloning the boxed trait objects.
        let mut slots: Vec<Option<Box<dyn AnyStage>>> = self.stages.into_iter().map(Some).collect();

        let ordered_stages: Vec<Box<dyn AnyStage>> = order
            .iter()
            .map(|id| {
                slots[id.0 as usize]
                    .take()
                    .expect("topo order references each graph node exactly once")
            })
            .collect();

        let fallback_map: HashMap<StageId, Box<dyn AnyStage>> = self
            .fallbacks
            .iter()
            .map(|&(gpu, cpu)| {
                let stage = slots[cpu.0 as usize]
                    .take()
                    .expect("a fallback target is taken exactly once");
                (gpu, stage)
            })
            .collect();

        let graph_node_set: HashSet<StageId> = graph_nodes.iter().copied().collect();
        let ordered_edges: Vec<Edge> = self
            .edges
            .into_iter()
            .filter(|e| graph_node_set.contains(&e.from) && graph_node_set.contains(&e.to))
            .collect();

        Ok(Pipeline::from_parts(
            ordered_stages,
            ordered_edges,
            fallback_map,
            config,
        ))
    }

    /// Returns the erased stage for `id`, if it was added to this chain.
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

    // Adjacency + in-degrees over graph edges only.
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

    // Seed the queue with all zero-in-degree nodes, ascending by id for a
    // deterministic order.
    let mut ready: VecDeque<StageId> = {
        let mut seeds: Vec<StageId> = nodes.iter().copied().filter(|n| indegree[n] == 0).collect();
        seeds.sort();
        seeds.into_iter().collect()
    };

    let mut order: Vec<StageId> = Vec::with_capacity(nodes.len());
    while let Some(n) = ready.pop_front() {
        order.push(n);
        // Collect newly-ready successors, sorted, for determinism.
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
        // The unscheduled nodes (in-degree never reached zero) lie on cycles.
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

    // Undirected adjacency over graph edges.
    let mut adj: HashMap<StageId, Vec<StageId>> = nodes.iter().map(|&n| (n, Vec::new())).collect();
    for e in edges {
        if !node_set.contains(&e.from) || !node_set.contains(&e.to) {
            continue;
        }
        adj.get_mut(&e.from).expect("from is a node").push(e.to);
        adj.get_mut(&e.to).expect("to is a node").push(e.from);
    }

    // BFS from the lowest-id node.
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

/// The neutral [`PipelineConfig`] applied when a [`Chain`] is built without an
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
        strict_gpu: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::{BitPackedBatch, LlrBatch, SymbolBatch};
    use crate::error::StageError;
    use crate::stage::{erase, ExecutionClass, Stage};
    use gf2_core::BitVec;

    // --- Tiny test stages -------------------------------------------------

    /// Identity over `BitPackedBatch` (CPU).
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

    /// BitPacked → Symbol (CPU). Produces a degenerate symbol batch.
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

    /// Symbol → Llr (CPU).
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

    /// A GPU-only identity over `BitPackedBatch` (declares GpuOnly so it needs a
    /// registered fallback).
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
        // BitToSym outputs SymbolBatch; SymToLlr consumes SymbolBatch — ok.
        // But BitId consumes BitPackedBatch, so SymToLlr(out=Llr) → BitId is a
        // mismatch.
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
        chain.connect(b, a).unwrap(); // BitPacked → BitPacked both ways: a 2-cycle.
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
        // Component 1: a → b.
        let a = chain.add(erase(BitId));
        let b = chain.add(erase(BitId));
        chain.connect(a, b).unwrap();
        // Component 2: c → d, disjoint from the first.
        let c = chain.add(erase(BitId));
        let d = chain.add(erase(BitId));
        chain.connect(c, d).unwrap();
        match chain.build() {
            Err(BuildError::Disconnected { stages }) => {
                // The component containing the lowest id (a) is kept; c and d
                // are reported as outside it.
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
        // Only the GPU stage is a graph node; the CPU twin is a substitution
        // target.
        assert_eq!(pipeline.stage_count(), 1);
        assert_eq!(pipeline.fallback_count(), 1);
    }

    #[test]
    fn test_build_rejects_duplicate_fallback_target() {
        // One CPU stage registered as the fallback for two GPU stages: the
        // materialiser can only move the boxed CPU stage out once, so build()
        // must reject this via `Result`, not panic on the second `take()`.
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
        // A fallback referencing a stage id that `add()` never returned must
        // fail via `Result`, not an out-of-bounds index panic in materialisation.
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
        // Fan-out then fan-in:  a → b, a → c, b → d, c → d.
        // All edges are BitPacked → BitPacked.
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

    /// Sanity: a `BitVec`-carrying batch flows through the test stages so the
    /// `gf2_core` import is genuinely exercised.
    #[test]
    fn test_bitid_processes_a_real_batch() {
        let s = BitId;
        let out = s
            .process(&BitPackedBatch::new(vec![BitVec::zeros(8)]), &mut ())
            .unwrap();
        assert_eq!(out.frames[0].len(), 8);
    }
}
