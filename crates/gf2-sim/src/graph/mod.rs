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
    /// # Fallback targets must not be connected
    ///
    /// A stage that is (or will be) registered as a CPU fallback target via
    /// [`register_fallback`](Chain::register_fallback) is **not** a node in the
    /// pipeline DAG and must not be connected by any edge. This method does not
    /// know the fallback registrations (they may be added later), so it records
    /// the edge regardless; [`build`](Chain::build) then rejects any edge
    /// incident to a fallback target with
    /// [`BuildError::FallbackTargetHasEdge`] rather than silently dropping it.
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
    /// This method only records the pairing; all validation is deferred to
    /// [`Chain::build`], which enforces every fallback invariant. In particular,
    /// for the registration to build successfully:
    ///
    /// * `gpu_stage` must be GPU-capable (`GpuOnly` or `Hybrid`) — only such a
    ///   stage can OOM on the GPU.
    /// * `cpu_stage` must be CPU-capable (`CpuOnly` or `Hybrid`) — it runs on
    ///   the CPU when the substitution fires.
    /// * `cpu_stage` must have the same input and output element types as
    ///   `gpu_stage`.
    /// * each `gpu_stage` may be registered at most once, each `cpu_stage` may
    ///   back at most one GPU stage, and no stage may appear in both roles
    ///   (this also forbids `gpu_stage == cpu_stage`).
    /// * `cpu_stage` must NOT be [`connect`](Chain::connect)ed by any graph edge
    ///   — a fallback target is a substitution target reachable only on OOM, not
    ///   a DAG node, so an incident edge is rejected rather than silently lost.
    ///
    /// See [`Chain::build`]'s `# Errors` for the exact `BuildError` returned
    /// when any of these is violated.
    ///
    /// # Arguments
    ///
    /// * `gpu_stage` — the GPU-capable stage that may OOM.
    /// * `cpu_stage` — the CPU-capable stage to substitute (must already be
    ///   added; it is not separately connected by edges).
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
        // Records the pairing only; every fallback invariant (in-range ids, no
        // duplicate GPU registration, no reused CPU target, no role overlap,
        // GPU-capability, CPU-capability, type compatibility) is enforced by
        // [`Chain::build`], which returns a typed `BuildError` for a malformed
        // registration rather than panicking.
        self.fallbacks.push((gpu_stage, cpu_stage));
    }

    /// Topologically sorts the graph and compiles it into a [`Pipeline`].
    ///
    /// Performs, in order: fallback registration validation, edge type
    /// re-validation, a Kahn topological sort (which also detects cycles), and a
    /// weak-connectivity check over the non-fallback stages. Branching DAGs
    /// (fan-out / fan-in) are supported; the resulting stage order is a valid
    /// linearisation of the DAG.
    ///
    /// ## Edge `from`/`to` contract
    ///
    /// After the topo sort each [`Edge`]'s `from` and `to` fields are remapped
    /// to **post-sort positions** in [`Pipeline::stages()`]. An edge `from = i`
    /// means `pipeline.stages()[i]` is the producer; `to = j` means
    /// `pipeline.stages()[j]` is the consumer. This is the only contract
    /// [`Pipeline::edges()`] documents; the original insertion-order
    /// [`StageId`]s are not preserved in the built pipeline.
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
    ///   [`connect`](Chain::connect)ed by a graph edge (on either end); a
    ///   fallback target is not a DAG node, so such an edge would be silently
    ///   lost.
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
        // 0. Validate every fallback/edge invariant up front so malformed input
        //    fails via `Result` rather than panicking or silently producing a
        //    Pipeline whose `stages()`/`edges()`/`fallbacks` do not faithfully
        //    represent what was registered. The materialiser below indexes
        //    `slots[cpu]`, `take()`s each target once, looks up `new_index_of`
        //    for each `gpu` key and each edge endpoint, and (formerly) filtered
        //    edges — every one of those is now backed by an invariant here.
        //    `register_fallback`/`connect` perform only minimal validation, so
        //    EVERY malformed combination must be rejected here.
        //
        //    The full, EXHAUSTIVE invariant set, in evaluation order. Each names
        //    the failure it prevents in the materialisation path:
        //      (1) gpu id in range          — else `new_index_of[&gpu]` / slot
        //          indexing reaches a non-existent stage.
        //      (2) cpu id in range          — else `slots[cpu]` indexing panics.
        //      (3) no duplicate gpu          — else the fallback HashMap silently
        //          drops one entry while both CPU stages are moved out.
        //      (4) no duplicate cpu target   — else `slots[cpu].take()` runs
        //          twice (second `take()` is `None` → panic). NOTE this also
        //          covers the degenerate `gpu == cpu` SAME-registration case
        //          indirectly, but (5) catches `gpu == cpu` first as an overlap.
        //      (5) no role overlap           — a stage may not be BOTH a fallback
        //          target (a `cpu`) AND a GPU stage that has its own fallback (a
        //          `gpu`). Includes `register_fallback(g, g)` (gpu == cpu), which
        //          is a self-overlap. A fallback target is excluded from
        //          `graph_nodes`, so it never enters `new_index_of`; were it also
        //          a `gpu` key the materialiser's `new_index_of[&gpu]` would panic.
        //      (6) gpu is GPU-capable        — only `GpuOnly`/`Hybrid` stages can
        //          OOM on the GPU; registering a fallback for a `CpuOnly` stage
        //          is meaningless.
        //      (7) cpu is CPU-capable        — a `GpuOnly` stage cannot serve as a
        //          CPU fallback (it cannot run on the CPU at all).
        //      (8) type compatibility        — the CPU fallback must have the same
        //          input and output element types as the GPU stage it
        //          substitutes; else the executor hits a runtime downcast fault.
        //      (9) no edge incident to a fallback target — a fallback target is
        //          not a DAG node; an edge touching it would be SILENTLY DROPPED
        //          by the materialiser's reduced-graph edge handling, losing
        //          registered topology. Checked at step 2b below (it needs the
        //          edge list, which the per-pair pass does not). All other edge
        //          invariants (endpoints in range, type compatibility) are
        //          enforced eagerly by `connect` and re-checked at step 2.
        let n_stages = self.stages.len() as u32;

        // (1)+(2): bounds, checked first so all later indexing is safe.
        for &(gpu, cpu) in &self.fallbacks {
            if gpu.0 >= n_stages {
                return Err(BuildError::Disconnected { stages: vec![gpu] });
            }
            if cpu.0 >= n_stages {
                return Err(BuildError::Disconnected { stages: vec![cpu] });
            }
        }

        // The GPU keys and CPU targets across all registrations. Built up front
        // so the role-overlap check (5) can see the complete picture before the
        // per-pair pass; ids are already known in range from the loop above.
        let registered_gpu: HashSet<StageId> = self.fallbacks.iter().map(|&(gpu, _)| gpu).collect();
        let fallback_targets: HashSet<StageId> =
            self.fallbacks.iter().map(|&(_, cpu)| cpu).collect();

        // (5): a stage cannot play both roles. Report the lowest such id for a
        //      deterministic error. This MUST be caught before materialisation:
        //      a fallback target is removed from `graph_nodes`, so it never
        //      enters `new_index_of`; if it were also a `gpu` key the
        //      `new_index_of[&gpu]` lookup in materialisation would panic.
        if let Some(&conflict) = {
            let mut overlap: Vec<&StageId> =
                registered_gpu.intersection(&fallback_targets).collect();
            overlap.sort();
            overlap.first().copied()
        } {
            return Err(BuildError::FallbackRoleConflict { stage: conflict });
        }

        // (3),(4),(6),(7),(8): the per-pair pass. Bounds (1)/(2) and overlap (5)
        //    are already guaranteed, so all `self.stages[..]` indexing is safe.
        let mut seen_gpu: HashSet<StageId> = HashSet::new();
        let mut seen_cpu: HashSet<StageId> = HashSet::new();
        for &(gpu, cpu) in &self.fallbacks {
            // (3) Each GPU stage may be registered at most once; a second
            //     registration would silently overwrite the first in the
            //     HashMap while still moving both CPU stages out of `slots`.
            if !seen_gpu.insert(gpu) {
                return Err(BuildError::DuplicateFallback { gpu_stage: gpu });
            }
            // (4) A CPU fallback backs exactly one GPU stage; otherwise the
            //     materialiser would move the same boxed stage out twice.
            if !seen_cpu.insert(cpu) {
                return Err(BuildError::Disconnected { stages: vec![cpu] });
            }
            // (6) The GPU stage must be able to run on the GPU (and thus OOM);
            //     registering a fallback for a `CpuOnly` stage is meaningless.
            if matches!(
                self.stages[gpu.0 as usize].execution_class(),
                crate::stage::ExecutionClass::CpuOnly
            ) {
                return Err(BuildError::FallbackForCpuStage { gpu_stage: gpu });
            }
            // (7) The CPU fallback must be able to run on the CPU; a `GpuOnly`
            //     stage cannot substitute for the GPU stage on OOM.
            if matches!(
                self.stages[cpu.0 as usize].execution_class(),
                crate::stage::ExecutionClass::GpuOnly
            ) {
                return Err(BuildError::FallbackNotCpuCapable { cpu_stage: cpu });
            }
            // (8) The CPU fallback must have the same input and output element
            //     types as the GPU stage it substitutes; otherwise the executor
            //     would encounter a type-downcast failure at runtime when the
            //     fallback is invoked.
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

        // 1. Fallback presence: every GPU-only stage needs a registered CPU
        //    fallback. The set of stages that ARE fallbacks is excluded from the
        //    graph (they are substitution targets, reachable only on OOM).
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

        // 2b. Fallback-target edge invariant (fallback invariant 9): a CPU
        //     fallback target is NOT a DAG node — it is excluded from
        //     `graph_nodes` and the topological order, and the materialiser's
        //     edge filter (below) keeps only edges fully inside the reduced
        //     graph. An edge incident to a fallback target (on EITHER end) would
        //     therefore be SILENTLY DROPPED, yielding a Pipeline whose `edges()`
        //     misrepresent the registered topology. `register_fallback` documents
        //     a fallback target as "not separately connected by edges", so reject
        //     such an edge explicitly rather than erasing it. The lowest-id
        //     offending edge is reported for determinism.
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
        // into the keyed map, and remap edges to post-sort positions.
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

        // Build an old-StageId → new-position map so edge endpoints can be
        // remapped. After topo sort `order[new_pos]` is the old StageId, so
        // `new_index_of[old_id] = new_pos`.
        //
        // This is the fix for Bug 1: without this remap, `Pipeline::edges()`
        // would return edges whose `from`/`to` still carry insertion-order
        // StageIds, which no longer index `Pipeline::stages()` correctly for
        // any non-identity topo reorder.
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
                // The fallback map is keyed by the GPU stage's new post-sort
                // position so the executor can look up by index into stages().
                // The role-overlap check (invariant 5 above) guarantees every
                // `gpu` key is a graph node and therefore present in
                // `new_index_of`, so this lookup never panics on validated input.
                let new_gpu = *new_index_of
                    .get(&gpu)
                    .expect("a GPU fallback key is a graph node (no role overlap)");
                (new_gpu, stage)
            })
            .collect();

        // Remap every edge to post-sort positions. No edge is dropped here:
        // invariant (9) above already rejected any edge incident to a fallback
        // target, so every endpoint is a graph node and is present in
        // `new_index_of`. We therefore map (never filter) — silently filtering
        // is exactly the topology-loss bug invariant (9) eliminates. The
        // `.expect()` documents that the lookup cannot fail on validated input.
        let ordered_edges: Vec<Edge> = self
            .edges
            .into_iter()
            .map(|e| Edge {
                // Remap from/to to post-sort positions so that
                // `pipeline.stages()[edge.from.0]` is the actual producer and
                // `pipeline.stages()[edge.to.0]` is the actual consumer.
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

    /// A Hybrid identity over `BitPackedBatch` (CPU-capable AND GPU-capable, so
    /// it is valid in either fallback role individually).
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

    // --- Bug 1 regression: edge StageIds are remapped to post-topo positions ---

    /// Stages added in REVERSE topological order (consumer before producer).
    ///
    /// After `build()`, `Pipeline::edges()[0].from` must index the producer in
    /// `Pipeline::stages()` and `.to` must index the consumer, regardless of
    /// insertion order. Without the post-sort remap, the edge would still carry
    /// the old insertion-order StageIds and would index the wrong stages.
    #[test]
    fn test_edge_positions_remapped_after_non_topo_insertion() {
        // Insert in REVERSE order: consumer (C), middle (M), producer (P).
        // Connections: P → M → C.
        // Insertion: C = StageId(0), M = StageId(1), P = StageId(2).
        // Topo order (ascending id, zero-in-degree first): [P, M, C]
        //   i.e. order = [StageId(2), StageId(1), StageId(0)].
        // Post-sort positions: P→0, M→1, C→2.
        // Edge P→M should become from=0, to=1; edge M→C should become from=1, to=2.
        let mut chain = Chain::new();
        let c = chain.add(erase(SymToLlr)); // consumer: SymbolBatch → LlrBatch
        let m = chain.add(erase(BitToSym)); // middle:   BitPackedBatch → SymbolBatch
        let p = chain.add(erase(BitId)); //   producer: BitPackedBatch → BitPackedBatch
        chain.connect(p, m).unwrap(); // P → M: BitPacked → BitPacked (M input)
        chain.connect(m, c).unwrap(); // M → C: Symbol → Symbol (C input)
        let pipeline = chain.build().expect("valid non-topo-inserted chain");

        assert_eq!(pipeline.stage_count(), 3);
        assert_eq!(pipeline.edges().len(), 2);

        // Topo order puts P first (only zero-in-degree node), then M, then C.
        // post-sort: position 0 = P (BitPacked→BitPacked), 1 = M (BitPacked→Symbol),
        //            2 = C (Symbol→Llr).
        use crate::batch::{BitPackedBatch, LlrBatch, SymbolBatch};
        use std::any::TypeId;
        let stages = pipeline.stages();
        assert_eq!(stages[0].input_type(), TypeId::of::<BitPackedBatch>());
        assert_eq!(stages[0].output_type(), TypeId::of::<BitPackedBatch>());
        assert_eq!(stages[1].input_type(), TypeId::of::<BitPackedBatch>());
        assert_eq!(stages[1].output_type(), TypeId::of::<SymbolBatch>());
        assert_eq!(stages[2].input_type(), TypeId::of::<SymbolBatch>());
        assert_eq!(stages[2].output_type(), TypeId::of::<LlrBatch>());

        // The P→M edge must point to position 0 → 1.
        // The M→C edge must point to position 1 → 2.
        // Without the remap, they would still carry the old stale ids (2→1 and 1→0),
        // which would index the WRONG stages (C and M, instead of P, M, C).
        let edges = pipeline.edges();
        // Sort edges by `from` for a deterministic check order.
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

    // --- Bug 2a regression: duplicate GPU stage in fallback registrations ---

    /// Registering the same GPU stage with two different CPU fallbacks must
    /// return `BuildError::DuplicateFallback`, not silently discard one entry.
    ///
    /// Without this check, `HashMap::collect` would silently keep only the
    /// last mapping while still moving both CPU stages out of `slots`.
    #[test]
    fn test_build_rejects_duplicate_gpu_fallback_registration() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId));
        let cpu1 = chain.add(erase(BitId));
        let cpu2 = chain.add(erase(BitId));
        chain.register_fallback(g, cpu1);
        chain.register_fallback(g, cpu2); // same GPU stage registered again
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

    // --- Bug 2b regression: type-incompatible fallback pair ---

    /// A CPU fallback with a different input/output type than the GPU stage
    /// must return `BuildError::FallbackTypeMismatch`.
    ///
    /// Without this check, the executor would encounter a runtime type-downcast
    /// failure the first time GPU OOM triggers the substitution.
    #[test]
    fn test_build_rejects_type_incompatible_fallback() {
        // GpuBitId: BitPackedBatch → BitPackedBatch (GpuOnly).
        // SymToLlr: SymbolBatch → LlrBatch (CpuOnly) — types differ on both ends.
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId)); // BitPacked → BitPacked, GpuOnly
        let wrong_cpu = chain.add(erase(SymToLlr)); // Symbol → Llr, CpuOnly
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

    // --- Gap 1 regression: role overlap (a stage is both gpu and cpu target) ---

    /// A stage registered as BOTH a GPU stage with its own fallback AND another
    /// GPU stage's CPU fallback target must return `BuildError::FallbackRoleConflict`,
    /// and crucially must NOT panic.
    ///
    /// Without this check the overlapping stage is removed from `graph_nodes`
    /// (as a fallback target) so it never enters `new_index_of`; the
    /// materialiser's `new_index_of[&gpu]` lookup would then panic. We use a
    /// Hybrid stage for the overlapping node so the role-overlap check (and not
    /// the GPU-/CPU-capability checks) is the one that fires.
    #[test]
    fn test_build_rejects_fallback_role_overlap_without_panic() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId)); // pure GPU stage
        let x = chain.add(erase(HybridBitId)); // plays both roles below
        let c = chain.add(erase(BitId)); // pure CPU fallback for x
                                         // x is a CPU fallback target for g ...
        chain.register_fallback(g, x);
        // ... and x is also a GPU stage with its own fallback c.
        chain.register_fallback(x, c);
        // `build()` must return an error, never panic.
        match chain.build() {
            Err(BuildError::FallbackRoleConflict { stage }) => assert_eq!(stage, x),
            Err(other) => panic!("expected FallbackRoleConflict for role overlap, got {other:?}"),
            Ok(_) => panic!("expected FallbackRoleConflict, got a built pipeline"),
        }
    }

    // --- Gap 2 regression: CPU fallback is not CPU-capable ---

    /// Registering a `GpuOnly` stage as a CPU fallback must return
    /// `BuildError::FallbackNotCpuCapable` — a GpuOnly stage cannot run on the
    /// CPU when the substitution fires.
    #[test]
    fn test_build_rejects_gpu_only_stage_as_cpu_fallback() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId)); // the GPU stage needing a fallback
        let bad_cpu = chain.add(erase(GpuBitId)); // GpuOnly — not CPU-capable
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

    // --- Invariant 6 regression: fallback registered for a CpuOnly stage ---

    /// Registering a fallback for a `CpuOnly` stage (which cannot OOM on the
    /// GPU) must return `BuildError::FallbackForCpuStage`.
    #[test]
    fn test_build_rejects_fallback_for_cpu_only_stage() {
        let mut chain = Chain::new();
        let cpu_gpu = chain.add(erase(BitId)); // CpuOnly — cannot OOM on GPU
        let cpu_fb = chain.add(erase(BitId)); // a valid CPU fallback otherwise
        chain.register_fallback(cpu_gpu, cpu_fb);
        match chain.build() {
            Err(BuildError::FallbackForCpuStage { gpu_stage }) => {
                assert_eq!(gpu_stage, cpu_gpu);
            }
            Err(other) => panic!("expected FallbackForCpuStage for CpuOnly stage, got {other:?}"),
            Ok(_) => panic!("expected FallbackForCpuStage, got a built pipeline"),
        }
    }

    // --- Positive: Hybrid stages are valid in either fallback role ---

    /// A `Hybrid` GPU stage with a `Hybrid` CPU fallback (distinct stages, no
    /// role overlap) builds successfully: Hybrid is both GPU-capable and
    /// CPU-capable, so it passes invariants 6 and 7.
    #[test]
    fn test_build_accepts_hybrid_stage_and_hybrid_fallback() {
        let mut chain = Chain::new();
        let gpu = chain.add(erase(HybridBitId)); // GPU-capable (Hybrid)
        let cpu = chain.add(erase(HybridBitId)); // CPU-capable (Hybrid)
        chain.register_fallback(gpu, cpu);
        let pipeline = chain
            .build()
            .expect("hybrid gpu + hybrid fallback is valid");
        // Only the GPU stage is a graph node; the fallback is a substitution target.
        assert_eq!(pipeline.stage_count(), 1);
        assert_eq!(pipeline.fallback_count(), 1);
    }

    // --- Gap regression: edge incident to a fallback target (silent topology loss) ---

    /// An edge whose CONSUMER (`to`) end is a CPU fallback target must return
    /// `BuildError::FallbackTargetHasEdge`, not silently drop the edge.
    ///
    /// Without this check, `connect(x, c)` + `register_fallback(g, c)` builds
    /// successfully but the materialiser filters the `x → c` edge out (c is not
    /// a graph node), producing a Pipeline whose `edges()` misrepresent the
    /// registered topology.
    #[test]
    fn test_build_rejects_edge_into_fallback_target() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId)); // GPU stage needing a fallback
        let x = chain.add(erase(BitId)); // an ordinary producer
        let c = chain.add(erase(BitId)); // the CPU fallback target
        chain.register_fallback(g, c);
        chain.connect(x, c).unwrap(); // illegal: edge INTO the fallback target
        match chain.build() {
            Err(BuildError::FallbackTargetHasEdge { stage, edge_peer }) => {
                assert_eq!(stage, c, "the fallback target is the offending stage");
                assert_eq!(edge_peer, x, "the other endpoint is the producer x");
            }
            Err(other) => panic!("expected FallbackTargetHasEdge, got {other:?}"),
            Ok(_) => panic!("expected FallbackTargetHasEdge, got a built pipeline"),
        }
    }

    /// An edge whose PRODUCER (`from`) end is a CPU fallback target must equally
    /// return `BuildError::FallbackTargetHasEdge`.
    #[test]
    fn test_build_rejects_edge_out_of_fallback_target() {
        let mut chain = Chain::new();
        let g = chain.add(erase(GpuBitId)); // GPU stage needing a fallback
        let c = chain.add(erase(BitId)); // the CPU fallback target
        let y = chain.add(erase(BitId)); // an ordinary consumer
        chain.register_fallback(g, c);
        chain.connect(c, y).unwrap(); // illegal: edge OUT OF the fallback target
        match chain.build() {
            Err(BuildError::FallbackTargetHasEdge { stage, edge_peer }) => {
                assert_eq!(stage, c, "the fallback target is the offending stage");
                assert_eq!(edge_peer, y, "the other endpoint is the consumer y");
            }
            Err(other) => panic!("expected FallbackTargetHasEdge, got {other:?}"),
            Ok(_) => panic!("expected FallbackTargetHasEdge, got a built pipeline"),
        }
    }

    /// A fallback whose target has NO incident edge still builds, and the GPU
    /// stage's own graph edge is faithfully preserved (not dropped). This is the
    /// counterpart confirming the invariant-9 check does not over-reject.
    #[test]
    fn test_build_accepts_fallback_with_no_incident_edge_preserving_graph_edge() {
        let mut chain = Chain::new();
        // Graph path: src → g (GPU). g has a CPU fallback c with no edges.
        let src = chain.add(erase(BitId)); // ordinary producer
        let g = chain.add(erase(GpuBitId)); // GPU stage, in the graph
        let c = chain.add(erase(BitId)); // CPU fallback target, NOT connected
        chain.connect(src, g).unwrap(); // src → g is a real graph edge
        chain.register_fallback(g, c);
        let pipeline = chain
            .build()
            .expect("fallback target with no incident edge is valid");
        // Two graph nodes (src, g); c is a substitution target.
        assert_eq!(pipeline.stage_count(), 2);
        assert_eq!(pipeline.fallback_count(), 1);
        // The src → g edge SURVIVES (it was not dropped along with c's removal).
        assert_eq!(
            pipeline.edges().len(),
            1,
            "the real graph edge is preserved"
        );
    }

    // --- Audit: gpu == cpu in a single registration is a self-overlap ---

    /// `register_fallback(g, g)` (a stage as its own fallback) is a degenerate
    /// role overlap and must be rejected with `BuildError::FallbackRoleConflict`,
    /// never panic in materialisation.
    #[test]
    fn test_build_rejects_self_fallback() {
        let mut chain = Chain::new();
        let g = chain.add(erase(HybridBitId)); // Hybrid so capability checks pass
        chain.register_fallback(g, g); // g is its own fallback: self-overlap
        match chain.build() {
            Err(BuildError::FallbackRoleConflict { stage }) => assert_eq!(stage, g),
            Err(other) => panic!("expected FallbackRoleConflict for self-fallback, got {other:?}"),
            Ok(_) => panic!("expected FallbackRoleConflict, got a built pipeline"),
        }
    }
}
