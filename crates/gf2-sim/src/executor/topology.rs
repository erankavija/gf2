//! DAG topology executor: [`TopologyExecutor`] dispatches a built [`Pipeline`]
//! graph's stages in topological order, routing each stage by its
//! [`execution_class()`](AnyStage::execution_class) and wrapping it in a
//! `pipeline_stage` span carrying
//! `(worker_idx, snr_idx, batch_id, stream_id, stage_name, wall_us)`.

use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;
use rayon::prelude::*;

use crate::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
use crate::channels::awgn::ChannelScratch;
use crate::error::{BuildError, FatalError, StageError};
use crate::executor::failure::FailurePolicy;
use crate::executor::Scheduler;
use crate::parallel::{WorkerCounters, WorkerCtx};
use crate::pipeline::{BatchHandle, Pipeline};
use crate::stage::{AnyScratch, AnyStage, ExecutionClass, TypedBatch};
use crate::stages::{DecodeScratch, DvbT2Encode};

/// The `stream_id` span value recorded when the executing worker owns no HIP
/// stream (no `hip` feature, GPU disabled, or no usable device).
pub const NO_STREAM: usize = usize::MAX;

/// Wraps a defensive-validation finding in the typed error chain
/// (`StageError::Fatal(FatalError::BuildError(ExecutionValidation))`).
fn exec_err(reason: impl Into<String>) -> StageError {
    StageError::Fatal(FatalError::BuildError(BuildError::ExecutionValidation {
        reason: reason.into(),
    }))
}

/// Provenance of a routed stage execution's BP iteration counts.
///
/// The DVB-T2 sweep's iteration accounting must distinguish a **genuine
/// CPU-fallback substitution** (which legitimately surfaces no per-frame
/// count — `mean_iters` is excluded from byte-identity on GPU-bearing chains)
/// from a degrade/no-source execution (which is a hard error, never silently
/// defaulted).
#[cfg_attr(not(feature = "hip"), allow(dead_code))] // Gpu/CpuFallback are
                                                    // constructed only by the hip GpuOnly dispatch arms.
enum StageIters {
    /// The GPU LDPC arm produced per-frame BP iteration counts.
    Gpu(Vec<u32>),
    /// [`dispatch_with_fallback`](crate::executor::failure::dispatch_with_fallback)
    /// substituted the stage's registered CPU fallback after a recoverable GPU
    /// error; no per-frame counts cross the erased fallback boundary.
    CpuFallback,
    /// The stage is not an iteration source: CPU/Hybrid stages, the GPU AWGN
    /// and demap arms, and the no-stream `process_any` degrades (where no
    /// substitution took place).
    NotASource,
}

/// The result of one routed stage execution: the output batch plus the
/// iteration-count provenance ([`StageIters`]).
type StageOutcome = Result<(Box<dyn TypedBatch>, StageIters), StageError>;

/// One completed wave member: `(stage position, output batch, returned
/// scratch)`.
type WaveResult = (usize, Box<dyn TypedBatch>, Box<dyn AnyScratch>);

/// The outputs of a [`TopologyExecutor::run`]: one type-erased batch per
/// **sink** stage (out-degree 0), in ascending stage-position order.
///
/// Intermediate (non-sink) outputs are reference-counted and dropped as soon
/// as their last consumer has run, so only the sink batches survive the run.
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::{Scheduler, TopologyExecutor};
/// use gf2_sim::graph::Chain;
/// use gf2_sim::stage::{erase, BatchSize, ExecutionClass, Stage, TypedBatch};
/// use gf2_sim::error::StageError;
/// use std::num::NonZeroUsize;
///
/// #[derive(Clone)]
/// struct B(u8);
/// impl BatchSize for B {
///     fn batch_size(&self) -> usize {
///         1
///     }
/// }
/// struct Inc;
/// impl Stage<B, B> for Inc {
///     type Scratch = ();
///     type CpuFallback = Self;
///     fn process(&self, i: &B, _: &mut ()) -> Result<B, StageError> {
///         Ok(B(i.0 + 1))
///     }
///     fn execution_class(&self) -> ExecutionClass {
///         ExecutionClass::CpuOnly
///     }
/// }
///
/// let mut chain = Chain::new();
/// let a = chain.add(erase(Inc));
/// let b = chain.add(erase(Inc));
/// chain.connect(a, b).unwrap();
/// let pipeline = chain.build().unwrap();
/// let scheduler = Scheduler::new(NonZeroUsize::new(1).unwrap(), false, 0);
///
/// let outputs = TopologyExecutor::run(&pipeline, &scheduler, Box::new(B(0))).unwrap();
/// assert_eq!(outputs.outputs().len(), 1, "one sink (the chain's tail)");
/// let out = outputs.into_single().expect("single sink");
/// assert_eq!(out.as_any().downcast_ref::<B>().unwrap().0, 2);
/// ```
pub struct DagOutputs {
    /// `(stage position, output batch)` per sink, ascending by position.
    sinks: Vec<(usize, Box<dyn TypedBatch>)>,
}

impl DagOutputs {
    /// The `(stage position, output batch)` pairs of every sink stage, in
    /// ascending stage-position order.
    #[must_use]
    pub fn outputs(&self) -> &[(usize, Box<dyn TypedBatch>)] {
        &self.sinks
    }

    /// Consumes `self`, returning the sink `(position, batch)` pairs.
    #[must_use]
    pub fn into_outputs(self) -> Vec<(usize, Box<dyn TypedBatch>)> {
        self.sinks
    }

    /// Consumes `self`, returning the single sink output — `None` if the DAG
    /// has zero or more than one sink.
    #[must_use]
    pub fn into_single(self) -> Option<Box<dyn TypedBatch>> {
        if self.sinks.len() == 1 {
            self.sinks.into_iter().next().map(|(_, b)| b)
        } else {
            None
        }
    }
}

/// Per-worker routing state threaded through [`execute_stage`].
///
/// Under `feature = "hip"` it can carry a persistent per-worker GPU LDPC
/// decoder (built once per sweep worker); a transient route makes the GPU arm
/// build a one-shot decoder per invocation instead.
struct WorkerRoute<'s> {
    #[cfg(feature = "hip")]
    gpu: Option<GpuWorkerState<'s>>,
    _life: std::marker::PhantomData<&'s ()>,
}

impl<'s> WorkerRoute<'s> {
    /// A route with no persistent GPU state (the generic DAG runner's mode).
    fn transient() -> Self {
        Self {
            #[cfg(feature = "hip")]
            gpu: None,
            _life: std::marker::PhantomData,
        }
    }
}

/// Persistent per-worker GPU LDPC state for the stage-driven sweep: the
/// worker-owned device decoder, its pinned stream scratch, and the worker's
/// deterministically owned HIP stream.
#[cfg(feature = "hip")]
struct GpuWorkerState<'s> {
    decoder: gf2_kernels_hip::GpuLdpcBp,
    scratch: gf2_kernels_hip::launch_ldpc_bp::LdpcStreamScratch,
    stream: &'s gf2_kernels_hip::host::HipStream,
    stream_id: usize,
}

/// The `stream_id` to record on a stage span for `worker_idx`: the worker's
/// owned HIP stream id when one exists, else [`NO_STREAM`].
fn stream_id_for(scheduler: &Scheduler, worker_idx: usize, route: &WorkerRoute<'_>) -> usize {
    #[cfg(feature = "hip")]
    {
        if let Some(g) = &route.gpu {
            return g.stream_id;
        }
        if let Some((id, _)) = scheduler.worker_stream(worker_idx) {
            return id;
        }
    }
    #[cfg(not(feature = "hip"))]
    let _ = (scheduler, worker_idx, route);
    NO_STREAM
}

/// The shared CPU-fallback closure body of every `dispatch_with_fallback` call
/// in [`execute_gpu_stage`]: runs the stage's registered fallback via the
/// erased [`cpu_fallback_process_any`](AnyStage::cpu_fallback_process_any)
/// hook, mapping a missing registration to the typed validation error.
///
/// Returns [`StageIters::CpuFallback`] provenance — no per-frame iteration
/// counts cross the erased fallback boundary. A stateful-scratch fallback (the
/// GpuAwgn shape) is refused inside the erased hook itself with a typed error,
/// which `dispatch_with_fallback` then escalates to `CpuFallbackAlsoFailed`.
#[cfg(feature = "hip")]
fn run_registered_cpu_fallback(
    stage: &dyn AnyStage,
    input: &dyn TypedBatch,
    scratch: &mut dyn AnyScratch,
) -> StageOutcome {
    stage
        .cpu_fallback_process_any(input, scratch)
        .unwrap_or_else(|| {
            Err(exec_err(format!(
                "GpuOnly stage `{}` has no CPU fallback registered",
                stage.name()
            )))
        })
        .map(|o| (o, StageIters::CpuFallback))
}

/// The `max_iterations` cap of the GPU LDPC BP stage at `pos`, used as the
/// recorded iteration count for a frame whose GPU dispatch was **substituted**
/// by the registered CPU LDPC fallback (the substitution leaves no per-frame
/// count, and `mean_iters` is excluded from the CPU-vs-GPU byte-identity
/// contract). The scheduler hybrid loop's fallback arm uses the same
/// convention. The caller only reaches it on [`StageIters::CpuFallback`]
/// provenance from the GPU LDPC stage, which only the hip dispatch arms
/// produce.
fn gpu_ldpc_max_iters(stages: &[Box<dyn AnyStage>], pos: usize) -> Result<u64, StageError> {
    #[cfg(feature = "hip")]
    {
        stages[pos]
            .stage_as_any()
            .and_then(|a| a.downcast_ref::<crate::gpu::ldpc_bp::GpuLdpcBp>())
            .map(|bp| bp.max_iterations() as u64)
            .ok_or_else(|| exec_err("internal: GPU LDPC position lost while defaulting iter count"))
    }
    #[cfg(not(feature = "hip"))]
    {
        let _ = (stages, pos);
        Err(exec_err(
            "internal: GPU LDPC iteration fallback is unreachable without the hip feature",
        ))
    }
}

/// Executes one stage via its [`AnyStage`] object, routed by
/// [`execution_class()`](AnyStage::execution_class), inside the six-field
/// `pipeline_stage` tracing span.
///
/// Returns the output batch plus the iteration-count provenance
/// ([`StageIters`]): per-frame counts when the GPU LDPC arm produced them,
/// [`StageIters::CpuFallback`] when a registered fallback was substituted,
/// [`StageIters::NotASource`] otherwise.
#[allow(clippy::too_many_arguments)]
fn execute_stage(
    stage: &dyn AnyStage,
    input: &dyn TypedBatch,
    scratch: &mut dyn AnyScratch,
    scheduler: &Scheduler,
    worker_idx: usize,
    snr_idx: usize,
    batch_id: u64,
    route: &mut WorkerRoute<'_>,
    failure: &FailurePolicy<'_>,
) -> StageOutcome {
    let stream_id = stream_id_for(scheduler, worker_idx, route);
    let span = tracing::info_span!(
        "pipeline_stage",
        worker_idx,
        snr_idx,
        batch_id,
        stream_id,
        stage_name = stage.name(),
        wall_us = tracing::field::Empty
    );
    let entered = span.enter();
    let t0 = std::time::Instant::now();
    let result = match stage.execution_class() {
        ExecutionClass::CpuOnly => stage
            .process_any(input, scratch)
            .map(|o| (o, StageIters::NotASource)),
        ExecutionClass::GpuOnly => execute_gpu_stage(
            stage, input, scratch, scheduler, worker_idx, route, failure, snr_idx, batch_id,
        ),
        ExecutionClass::Hybrid => execute_hybrid_stage(stage, input, scratch),
    };
    span.record("wall_us", t0.elapsed().as_micros() as u64);
    drop(entered);
    result
}

/// The `GpuOnly` routing arm under `hip`: every **known** GpuOnly stage type
/// is downcast from its erased handle and enqueued on the worker's owned HIP
/// stream via its stream-aware entry point:
///
/// * [`GpuLdpcBp`](crate::gpu::ldpc_bp::GpuLdpcBp) →
///   `decode_batch_with_iters_on_stream` (persistent per-worker decoder when
///   the route carries one, else a one-shot decoder for this batch);
/// * [`GpuAwgn`](crate::gpu::awgn::GpuAwgn) → `apply_on_stream` (one-shot
///   per-batch generator + pinned staging);
/// * [`GpuGrayQamDemapper`](crate::gpu::demap::GpuGrayQamDemapper) →
///   `demap_batch_on_stream` (one-shot per-batch demapper + pinned staging).
///
/// An **unknown** `GpuOnly` stage type while the worker owns a stream is a
/// typed [`BuildError::ExecutionValidation`] — never a silent default-stream
/// `process_any`. With no active stream pool
/// (GPU disabled or unavailable) every arm degrades to `process_any` after a
/// `tracing::warn!`.
///
/// Every GPU-stage call — including the no-stream degrades — is wrapped with
/// [`dispatch_with_fallback`](crate::executor::failure::dispatch_with_fallback)
/// OOM → registered CPU fallback substitution (or hard-fail when
/// `strict_gpu`); fatal errors → diagnostic dump + propagate.
#[cfg(feature = "hip")]
#[allow(clippy::too_many_arguments)]
fn execute_gpu_stage(
    stage: &dyn AnyStage,
    input: &dyn TypedBatch,
    scratch: &mut dyn AnyScratch,
    scheduler: &Scheduler,
    worker_idx: usize,
    route: &mut WorkerRoute<'_>,
    failure: &FailurePolicy<'_>,
    snr_idx: usize,
    batch_id: u64,
) -> StageOutcome {
    use crate::executor::failure::{dispatch_with_fallback, FaultContext};

    let ctx = FaultContext {
        batch_id,
        snr_idx,
        device_id: 0,
        worker_idx,
    };

    if let Some(gpu_bp) = stage
        .stage_as_any()
        .and_then(|a| a.downcast_ref::<crate::gpu::ldpc_bp::GpuLdpcBp>())
    {
        let llrs =
            input
                .as_any()
                .downcast_ref::<LlrBatch>()
                .ok_or_else(|| StageError::TypeMismatch {
                    expected: std::any::TypeId::of::<LlrBatch>(),
                    actual: input.as_any().type_id(),
                })?;
        if llrs.frames.is_empty() {
            return Ok((
                Box::new(HardDecisionBatch::new(Vec::new())),
                StageIters::Gpu(Vec::new()),
            ));
        }
        // Test-only GPU-OOM fault injection: force a recoverable OOM on the
        // selected frames so it flows through the `dispatch_with_fallback`
        // path below as a genuine device OOM would. Each topology dispatch is
        // one frame, keyed on its global frame index (`batch_id == g`).
        let injected_oom: Option<StageError> = failure.injects_oom_at(batch_id).then(|| {
            StageError::Recoverable(crate::error::RecoverableError::OutOfMemory {
                device_id: ctx.device_id,
                bytes_requested: 1024 * 1024 * 1024,
            })
        });
        if let Some(g) = route.gpu.as_mut() {
            // The sweep's persistent per-worker decoder on the worker's owned
            // stream (one GPU LDPC stage per supported chain, so the decoder
            // matches this stage's code by construction).
            let raw: StageOutcome = match injected_oom {
                Some(oom) => Err(oom),
                None => gpu_bp
                    .decode_batch_with_iters_on_stream(llrs, &g.decoder, g.stream, &mut g.scratch)
                    .map(|(hard, iters)| {
                        (
                            Box::new(hard) as Box<dyn TypedBatch>,
                            StageIters::Gpu(iters),
                        )
                    }),
            };
            return dispatch_with_fallback(
                raw,
                || run_registered_cpu_fallback(stage, input, scratch),
                ctx,
                failure.strict_gpu,
                failure.dump_dir,
            );
        }
        if let Some((_, stream)) = scheduler.worker_stream(worker_idx) {
            // Transient: build a one-shot decoder sized for this batch, still
            // stream-ordered on the worker's owned stream.
            let raw: StageOutcome = match injected_oom {
                Some(oom) => Err(oom),
                None => (|| {
                    let decoder = gpu_bp.build_decoder(llrs.frames.len())?;
                    let mut stream_scratch = gpu_bp.build_stream_scratch(&decoder)?;
                    let (hard, iters) = gpu_bp.decode_batch_with_iters_on_stream(
                        llrs,
                        &decoder,
                        stream,
                        &mut stream_scratch,
                    )?;
                    Ok((
                        Box::new(hard) as Box<dyn TypedBatch>,
                        StageIters::Gpu(iters),
                    ))
                })(),
            };
            return dispatch_with_fallback(
                raw,
                || run_registered_cpu_fallback(stage, input, scratch),
                ctx,
                failure.strict_gpu,
                failure.dump_dir,
            );
        }
        // No active stream pool (GPU disabled or unavailable): degrade to the
        // erased path, wrapped like every other GPU-stage call. A SUCCESSFUL
        // degrade reports `StageIters::NotASource` — not `CpuFallback` — so
        // the sweep's iteration accounting cannot mistake a degrade for a
        // substitution.
        tracing::warn!(
            stage = stage.name(),
            "GpuOnly LDPC stage with no active HIP stream pool; degrading to process_any"
        );
        let raw: StageOutcome = stage
            .process_any(input, scratch)
            .map(|o| (o, StageIters::NotASource));
        return dispatch_with_fallback(
            raw,
            || run_registered_cpu_fallback(stage, input, scratch),
            ctx,
            failure.strict_gpu,
            failure.dump_dir,
        );
    }
    if let Some(gpu_awgn) = stage
        .stage_as_any()
        .and_then(|a| a.downcast_ref::<crate::gpu::awgn::GpuAwgn>())
    {
        // Run the GPU call FIRST: the fallback closure borrows `scratch` only
        // when `dispatch_with_fallback` invokes it. GpuAwgn's registered
        // fallback (`Awgn`) has STATEFUL scratch, so the erased hook refuses
        // the substitution with a typed error and the wrapper escalates to
        // `CpuFallbackAlsoFailed` rather than ever drawing default-seeded
        // noise.
        let raw = execute_gpu_awgn(gpu_awgn, stage, input, scratch, scheduler, worker_idx);
        return dispatch_with_fallback(
            raw,
            || run_registered_cpu_fallback(stage, input, scratch),
            ctx,
            failure.strict_gpu,
            failure.dump_dir,
        );
    }
    if let Some(gpu_demap) = stage
        .stage_as_any()
        .and_then(|a| a.downcast_ref::<crate::gpu::demap::GpuGrayQamDemapper>())
    {
        // Run the GPU call FIRST for the same reason as the AWGN arm above.
        let raw = execute_gpu_demap(gpu_demap, stage, input, scratch, scheduler, worker_idx);
        return dispatch_with_fallback(
            raw,
            || run_registered_cpu_fallback(stage, input, scratch),
            ctx,
            failure.strict_gpu,
            failure.dump_dir,
        );
    }
    // An UNKNOWN GpuOnly stage type. With an owned stream available it is
    // refused: falling through to `process_any` would dispatch device work on
    // the DEFAULT stream, breaking the "GpuOnly → the worker's owned HIP
    // stream" contract without any test failing.
    if scheduler.worker_stream(worker_idx).is_some() {
        return Err(exec_err(format!(
            "GpuOnly stage `{}` has no stream-aware dispatch in the topology executor: \
             known GpuOnly stage types are GpuLdpcBp, GpuAwgn, and GpuGrayQamDemapper; \
             wire the new stage type into execute_gpu_stage (with an *_on_stream entry \
             point) rather than letting it run on the default stream",
            stage.name()
        )));
    }
    // No active stream pool: degrade like the known-stage arms above, wrapped
    // so a fatal device fault still produces a diagnostic dump.
    tracing::warn!(
        stage = stage.name(),
        "GpuOnly stage with no active HIP stream pool; degrading to process_any"
    );
    let raw: StageOutcome = stage
        .process_any(input, scratch)
        .map(|o| (o, StageIters::NotASource));
    dispatch_with_fallback(
        raw,
        || run_registered_cpu_fallback(stage, input, scratch),
        ctx,
        failure.strict_gpu,
        failure.dump_dir,
    )
}

/// The [`GpuAwgn`](crate::gpu::awgn::GpuAwgn) stream route: corrupt the
/// [`SymbolBatch`] via `apply_on_stream` on the worker's owned HIP stream
/// (one-shot per-batch generator + pinned staging, mirroring the transient
/// LDPC route). Frame `f` of the batch seeks to the stage's
/// `worker_offset(.., f)` region — the same per-frame keying as the stage's
/// erased `process` path, so the noise is byte-identical to it.
#[cfg(feature = "hip")]
fn execute_gpu_awgn(
    gpu_awgn: &crate::gpu::awgn::GpuAwgn,
    stage: &dyn AnyStage,
    input: &dyn TypedBatch,
    scratch: &mut dyn AnyScratch,
    scheduler: &Scheduler,
    worker_idx: usize,
) -> StageOutcome {
    let Some((_, stream)) = scheduler.worker_stream(worker_idx) else {
        tracing::warn!(
            stage = stage.name(),
            "GpuOnly AWGN stage with no active HIP stream pool; degrading to process_any"
        );
        return stage
            .process_any(input, scratch)
            .map(|o| (o, StageIters::NotASource));
    };
    let symbols = input
        .as_any()
        .downcast_ref::<SymbolBatch>()
        .ok_or_else(|| StageError::TypeMismatch {
            expected: std::any::TypeId::of::<SymbolBatch>(),
            actual: input.as_any().type_id(),
        })?;
    let max_symbols = symbols.i.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = symbols.clone();
    if max_symbols == 0 {
        return Ok((Box::new(out), StageIters::NotASource));
    }
    let gen = gpu_awgn.build_generator(max_symbols)?;
    let mut stream_scratch = gpu_awgn.build_stream_scratch(&gen)?;
    gpu_awgn.apply_on_stream(&mut out, &gen, stream, &mut stream_scratch)?;
    Ok((Box::new(out), StageIters::NotASource))
}

/// The [`GpuGrayQamDemapper`](crate::gpu::demap::GpuGrayQamDemapper) stream
/// route: demap the [`SymbolBatch`] via `demap_batch_on_stream` on the
/// worker's owned HIP stream (one-shot per-batch demapper + pinned staging,
/// mirroring the transient LDPC route). Only reachable for
/// `DemapMethod::MaxLog` — an `ExactLogMap` stage reports
/// `ExecutionClass::CpuOnly` and never enters the GpuOnly arm.
#[cfg(feature = "hip")]
fn execute_gpu_demap(
    gpu_demap: &crate::gpu::demap::GpuGrayQamDemapper,
    stage: &dyn AnyStage,
    input: &dyn TypedBatch,
    scratch: &mut dyn AnyScratch,
    scheduler: &Scheduler,
    worker_idx: usize,
) -> StageOutcome {
    let Some((_, stream)) = scheduler.worker_stream(worker_idx) else {
        tracing::warn!(
            stage = stage.name(),
            "GpuOnly demap stage with no active HIP stream pool; degrading to process_any"
        );
        return stage
            .process_any(input, scratch)
            .map(|o| (o, StageIters::NotASource));
    };
    let symbols = input
        .as_any()
        .downcast_ref::<SymbolBatch>()
        .ok_or_else(|| StageError::TypeMismatch {
            expected: std::any::TypeId::of::<SymbolBatch>(),
            actual: input.as_any().type_id(),
        })?;
    let max_symbols = symbols.i.iter().map(Vec::len).max().unwrap_or(0);
    if max_symbols == 0 {
        return Ok((
            Box::new(LlrBatch::new(vec![Vec::new(); symbols.i.len()])),
            StageIters::NotASource,
        ));
    }
    let demapper = gpu_demap.build_demapper(max_symbols)?;
    let mut stream_scratch = gpu_demap.build_stream_scratch(&demapper)?;
    let out = gpu_demap.demap_batch_on_stream(symbols, &demapper, stream, &mut stream_scratch)?;
    Ok((Box::new(out), StageIters::NotASource))
}

/// The `GpuOnly` routing arm without the `hip` feature: there is no device
/// backend, so the stage degrades to `process_any` on the CPU after a
/// `tracing::warn!` (the documented graceful degrade, matching the scheduler).
/// The `failure`, `snr_idx`, and `batch_id` parameters are accepted for
/// signature parity with the `hip` variant but are unused.
#[cfg(not(feature = "hip"))]
#[allow(clippy::too_many_arguments)]
fn execute_gpu_stage(
    stage: &dyn AnyStage,
    input: &dyn TypedBatch,
    scratch: &mut dyn AnyScratch,
    _scheduler: &Scheduler,
    _worker_idx: usize,
    _route: &mut WorkerRoute<'_>,
    _failure: &FailurePolicy<'_>,
    _snr_idx: usize,
    _batch_id: u64,
) -> StageOutcome {
    tracing::warn!(
        stage = stage.name(),
        "GpuOnly stage on a build without the `hip` feature; running via process_any on the CPU"
    );
    stage
        .process_any(input, scratch)
        .map(|o| (o, StageIters::NotASource))
}

/// The `Hybrid` routing arm: split the batch per-batch into two frame halves
/// (first `ceil(n/2)` frames, then the rest), process each half via the stage
/// object, and re-concatenate the outputs in order. Single-frame or
/// non-splittable batches are processed whole.
fn execute_hybrid_stage(
    stage: &dyn AnyStage,
    input: &dyn TypedBatch,
    scratch: &mut dyn AnyScratch,
) -> StageOutcome {
    if let Some((lo, hi)) = split_half(input) {
        tracing::debug!(
            stage = stage.name(),
            lo = lo.batch_size(),
            hi = hi.batch_size(),
            "hybrid stage: split per-batch"
        );
        let out_lo = stage.process_any(lo.as_ref(), scratch)?;
        let out_hi = stage.process_any(hi.as_ref(), scratch)?;
        let merged = concat_batches(&[out_lo.as_ref(), out_hi.as_ref()]).ok_or_else(|| {
            exec_err(format!(
                "hybrid stage `{}` output batch type does not support per-batch re-concatenation",
                stage.name()
            ))
        })?;
        Ok((merged, StageIters::NotASource))
    } else {
        stage
            .process_any(input, scratch)
            .map(|o| (o, StageIters::NotASource))
    }
}

/// Concatenates type-erased batches frame-wise, in slice order, for the four
/// canonical batch types ([`BitPackedBatch`], [`HardDecisionBatch`],
/// [`LlrBatch`], [`SymbolBatch`]). Returns `None` when the parts are not all
/// of one canonical type.
fn concat_batches(parts: &[&dyn TypedBatch]) -> Option<Box<dyn TypedBatch>> {
    fn typed<'a, T: 'static>(parts: &'a [&dyn TypedBatch]) -> Option<Vec<&'a T>> {
        parts
            .iter()
            .map(|p| p.as_any().downcast_ref::<T>())
            .collect()
    }
    if parts.is_empty() {
        return None;
    }
    if let Some(v) = typed::<BitPackedBatch>(parts) {
        let frames = v.iter().flat_map(|b| b.frames.iter().cloned()).collect();
        return Some(Box::new(BitPackedBatch::new(frames)));
    }
    if let Some(v) = typed::<HardDecisionBatch>(parts) {
        let frames = v.iter().flat_map(|b| b.frames.iter().cloned()).collect();
        return Some(Box::new(HardDecisionBatch::new(frames)));
    }
    if let Some(v) = typed::<LlrBatch>(parts) {
        let frames = v.iter().flat_map(|b| b.frames.iter().cloned()).collect();
        return Some(Box::new(LlrBatch::new(frames)));
    }
    if let Some(v) = typed::<SymbolBatch>(parts) {
        let i = v.iter().flat_map(|b| b.i.iter().cloned()).collect();
        let q = v.iter().flat_map(|b| b.q.iter().cloned()).collect();
        return Some(Box::new(SymbolBatch::new(i, q)));
    }
    None
}

/// Splits a type-erased canonical batch into its first `ceil(n/2)` frames and
/// the rest. Returns `None` for batches of fewer than two frames or non-
/// canonical batch types.
#[allow(clippy::type_complexity)]
fn split_half(batch: &dyn TypedBatch) -> Option<(Box<dyn TypedBatch>, Box<dyn TypedBatch>)> {
    let n = batch.batch_size();
    if n < 2 {
        return None;
    }
    let mid = n.div_ceil(2);
    if let Some(b) = batch.as_any().downcast_ref::<BitPackedBatch>() {
        return Some((
            Box::new(BitPackedBatch::new(b.frames[..mid].to_vec())),
            Box::new(BitPackedBatch::new(b.frames[mid..].to_vec())),
        ));
    }
    if let Some(b) = batch.as_any().downcast_ref::<HardDecisionBatch>() {
        return Some((
            Box::new(HardDecisionBatch::new(b.frames[..mid].to_vec())),
            Box::new(HardDecisionBatch::new(b.frames[mid..].to_vec())),
        ));
    }
    if let Some(b) = batch.as_any().downcast_ref::<LlrBatch>() {
        return Some((
            Box::new(LlrBatch::new(b.frames[..mid].to_vec())),
            Box::new(LlrBatch::new(b.frames[mid..].to_vec())),
        ));
    }
    if let Some(b) = batch.as_any().downcast_ref::<SymbolBatch>() {
        return Some((
            Box::new(SymbolBatch::new(b.i[..mid].to_vec(), b.q[..mid].to_vec())),
            Box::new(SymbolBatch::new(b.i[mid..].to_vec(), b.q[mid..].to_vec())),
        ));
    }
    None
}

/// The per-wave input of one stage: the run's root batch, a single producer's
/// output (shared by reference), or a fresh fan-in merge.
enum WaveInput {
    /// The run's root input batch (in-degree-0 stages).
    Root,
    /// The output of the producer at this stage position (single in-edge).
    Single(usize),
    /// The concatenation of all producers' outputs, in in-edge order.
    Merged(Box<dyn TypedBatch>),
}

/// The DAG topology executor (see the [executor module docs](crate::executor)).
///
/// A unit type whose associated functions consume a built [`Pipeline`] plus
/// the [`Scheduler`] that owns the rayon worker pool (and, under `hip`, the
/// HIP stream pool).
pub struct TopologyExecutor;

impl TopologyExecutor {
    /// Defensive execution-start validation of the connector lineage.
    ///
    /// Cycles and disconnection are build()-time errors and cannot occur on a
    /// built [`Pipeline`]; this re-checks, panic-free, that the built stage
    /// order and edges are mutually consistent:
    ///
    /// * every edge references in-range stages;
    /// * every edge goes **forward** in the stage list (`from < to`) — i.e.
    ///   the stored stage order is a topological linearisation of the edges;
    /// * every edge's `element_type` equals both its producer's
    ///   `output_type()` and its consumer's `input_type()`.
    ///
    /// # Errors
    ///
    /// * [`BuildError::ExecutionValidation`] (wrapped in
    ///   [`StageError::Fatal`]) for an out-of-range or non-forward edge;
    /// * [`BuildError::TypeMismatch`] (wrapped in [`StageError::Fatal`]) for a
    ///   lineage type break.
    ///
    /// # Complexity
    ///
    /// `O(edges)`.
    pub fn validate(pipeline: &Pipeline) -> Result<(), StageError> {
        let stages = pipeline.stages();
        let n = stages.len();
        for edge in pipeline.edges() {
            let from = edge.from.0 as usize;
            let to = edge.to.0 as usize;
            if from >= n || to >= n {
                return Err(exec_err(format!(
                    "edge {from} -> {to} references a stage outside the {n}-stage list"
                )));
            }
            if from >= to {
                return Err(exec_err(format!(
                    "edge {from} -> {to} does not go forward in the stage list: \
                     the stored stage order is not a topological linearisation of the edges"
                )));
            }
            let out_t = stages[from].output_type();
            let in_t = stages[to].input_type();
            if out_t != edge.element_type || edge.element_type != in_t {
                return Err(StageError::Fatal(FatalError::BuildError(
                    BuildError::TypeMismatch {
                        from_stage: edge.from,
                        from_type: out_t,
                        to_stage: edge.to,
                        to_type: in_t,
                    },
                )));
            }
        }
        Ok(())
    }

    /// Runs one input batch through the pipeline DAG in topologically correct
    /// order, returning the sink outputs.
    ///
    /// Equivalent to [`run_with_handle`](Self::run_with_handle) with a fresh
    /// `BatchHandle::new(0, 0)` (the span fields `batch_id`/`snr_idx` are 0).
    ///
    /// # Arguments
    ///
    /// * `batch` — the root input, delivered to every in-degree-0 stage.
    ///
    /// # Errors
    ///
    /// See [`run_with_handle`](Self::run_with_handle).
    ///
    /// # Complexity
    ///
    /// `O(stages + edges)` bookkeeping plus the stages' own work; independent
    /// stages within a wave run in parallel across the scheduler's workers.
    pub fn run(
        pipeline: &Pipeline,
        scheduler: &Scheduler,
        batch: Box<dyn TypedBatch>,
    ) -> Result<DagOutputs, StageError> {
        Self::run_with_handle(pipeline, scheduler, batch, BatchHandle::new(0, 0))
    }

    /// Like [`run`](Self::run), with an explicit [`BatchHandle`] supplying the
    /// `batch_id` / `snr_idx` recorded on every per-stage span.
    ///
    /// Execution proceeds in Kahn waves: a stage runs once all its producers
    /// have run (fan-in waits on **all** of them); independent stages of one
    /// wave run in parallel on the scheduler's rayon pool. A fan-in stage
    /// receives its producers' outputs concatenated frame-wise in in-edge
    /// order; a fan-out producer's buffer is shared by reference and dropped
    /// once its last consumer has run (reference counting on the intermediate
    /// buffer). Every stage executes via its [`AnyStage`] object, routed by
    /// [`execution_class()`](AnyStage::execution_class): `CpuOnly` via
    /// [`process_any`](AnyStage::process_any); `GpuOnly` on the worker's owned
    /// HIP stream, or via `process_any` after a `tracing::warn!` when no
    /// stream pool is active; `Hybrid` split into its first `ceil(n/2)` frames
    /// and the rest, each processed via `process_any` and re-concatenated
    /// (single-frame or non-splittable batches are processed whole).
    ///
    /// # Errors
    ///
    /// * the [`validate`](Self::validate) errors (defensive lineage check);
    /// * [`StageError::TypeMismatch`] if the root input's type does not match
    ///   a source stage's input type;
    /// * [`BuildError::ExecutionValidation`] (wrapped fatal) if a fan-in joins
    ///   a non-canonical batch type that cannot be concatenated, or if a
    ///   `GpuOnly` stage type has no stream-aware dispatch while a stream pool
    ///   is active;
    /// * any [`StageError`] a stage itself returns.
    ///
    /// # Complexity
    ///
    /// See [`run`](Self::run).
    pub fn run_with_handle(
        pipeline: &Pipeline,
        scheduler: &Scheduler,
        batch: Box<dyn TypedBatch>,
        handle: BatchHandle,
    ) -> Result<DagOutputs, StageError> {
        Self::validate(pipeline)?;
        let stages = pipeline.stages();
        let n = stages.len();
        if n == 0 {
            return Ok(DagOutputs { sinks: Vec::new() });
        }

        let mut in_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut out_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
        for e in pipeline.edges() {
            in_edges[e.to.0 as usize].push(e.from.0 as usize);
            out_edges[e.from.0 as usize].push(e.to.0 as usize);
        }
        let mut indeg: Vec<usize> = in_edges.iter().map(Vec::len).collect();
        // Refcount per producer: the number of consumer edges still pending.
        let mut remaining: Vec<usize> = out_edges.iter().map(Vec::len).collect();

        let input_type = batch.as_any().type_id();
        for (pos, stage) in stages.iter().enumerate() {
            if indeg[pos] == 0 && stage.input_type() != input_type {
                return Err(StageError::TypeMismatch {
                    expected: stage.input_type(),
                    actual: input_type,
                });
            }
        }

        let mut outputs: Vec<Option<Box<dyn TypedBatch>>> = (0..n).map(|_| None).collect();
        let mut scratches: Vec<Box<dyn AnyScratch>> =
            stages.iter().map(|s| s.default_scratch()).collect();

        let snr_idx = handle.snr_idx() as usize;
        let batch_id = handle.batch_id();

        let mut wave: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
        while !wave.is_empty() {
            // Prepare each wave member's input + scratch outside the parallel
            // region (the merge clones from producers' outputs).
            let mut items: Vec<(usize, WaveInput, Box<dyn AnyScratch>)> =
                Vec::with_capacity(wave.len());
            for &s in &wave {
                let wave_input = match in_edges[s].len() {
                    0 => WaveInput::Root,
                    1 => WaveInput::Single(in_edges[s][0]),
                    _ => {
                        let parts: Vec<&dyn TypedBatch> = in_edges[s]
                            .iter()
                            .map(|&p| {
                                outputs[p].as_deref().ok_or_else(|| {
                                    exec_err(format!(
                                        "internal: producer {p} output missing for consumer {s}"
                                    ))
                                })
                            })
                            .collect::<Result<_, _>>()?;
                        let merged = concat_batches(&parts).ok_or_else(|| {
                            exec_err(format!(
                                "fan-in into stage `{}` (position {s}) joins a batch type \
                                 that does not support frame-wise concatenation",
                                stages[s].name()
                            ))
                        })?;
                        WaveInput::Merged(merged)
                    }
                };
                // Temporarily replace the stage's scratch with a unit
                // placeholder so it can be moved into the parallel region.
                let scratch = std::mem::replace(&mut scratches[s], Box::new(()));
                items.push((s, wave_input, scratch));
            }

            let dump_dir_buf;
            let failure = {
                let cfg = pipeline.config();
                dump_dir_buf = cfg
                    .diagnostic_dump_dir
                    .clone()
                    .unwrap_or_else(crate::executor::failure::default_dump_dir);
                FailurePolicy {
                    strict_gpu: cfg.strict_gpu,
                    dump_dir: &dump_dir_buf,
                    inject_gpu_oom_modulus: cfg.inject_gpu_oom_modulus,
                }
            };

            let wave_results: Result<Vec<WaveResult>, StageError> = {
                let outputs_ref = &outputs;
                let input_ref = batch.as_ref();
                let failure_ref = &failure;
                scheduler.rayon_pool().install(|| {
                    items
                        .into_par_iter()
                        .map(|(s, wave_input, mut scratch)| {
                            let inp: &dyn TypedBatch = match &wave_input {
                                WaveInput::Root => input_ref,
                                WaveInput::Single(p) => {
                                    outputs_ref[*p].as_deref().ok_or_else(|| {
                                        exec_err(format!(
                                            "internal: producer {p} output missing for consumer {s}"
                                        ))
                                    })?
                                }
                                WaveInput::Merged(b) => b.as_ref(),
                            };
                            let worker_idx = rayon::current_thread_index().unwrap_or(0);
                            let mut route = WorkerRoute::transient();
                            let (out, _iters) = execute_stage(
                                stages[s].as_ref(),
                                inp,
                                scratch.as_mut(),
                                scheduler,
                                worker_idx,
                                snr_idx,
                                batch_id,
                                &mut route,
                                failure_ref,
                            )?;
                            Ok((s, out, scratch))
                        })
                        .collect()
                })
            };
            let wave_results = wave_results?;

            let completed: Vec<usize> = wave_results.iter().map(|(s, _, _)| *s).collect();
            for (s, out, scratch) in wave_results {
                outputs[s] = Some(out);
                scratches[s] = scratch;
            }
            for &s in &completed {
                for &p in &in_edges[s] {
                    remaining[p] -= 1;
                    if remaining[p] == 0 {
                        // Last consumer done: drop the intermediate buffer.
                        outputs[p] = None;
                    }
                }
            }
            let mut next: Vec<usize> = Vec::new();
            for &s in &completed {
                for &t in &out_edges[s] {
                    indeg[t] -= 1;
                    if indeg[t] == 0 {
                        next.push(t);
                    }
                }
            }
            next.sort_unstable();
            wave = next;
        }

        let mut sinks = Vec::new();
        for (pos, out) in outputs.iter_mut().enumerate() {
            if out_edges[pos].is_empty() {
                if let Some(b) = out.take() {
                    sinks.push((pos, b));
                }
            }
        }
        Ok(DagOutputs { sinks })
    }

    /// Drives the DVB-T2 BICM chain **per-stage** over one SNR point,
    /// returning the `worker_idx`-ordered aggregate [`WorkerCounters`].
    ///
    /// Global frames `0..max_frames` are fanned across the scheduler's
    /// `parallelism` workers (worker `w` takes frames `w, w+W, …`, the same
    /// strided partition as the SSOT dispatch). Per frame `g` the worker:
    ///
    /// 1. reseeks its [`WorkerCtx`] to `worker_offset(seed, snr_idx, 0, g)`
    ///    (global-frame keying) and mints the random BBFRAME with the SSOT
    ///    `random_bitvec` helper — the identical message draw to
    ///    [`DvbT2BicmFrameSim::simulate_frame`](crate::frame_sim::DvbT2BicmFrameSim::simulate_frame);
    /// 2. hands the chain's AWGN channel stage a scratch RNG positioned at the
    ///    **post-message** stream offset, so the stage's planar noise draw
    ///    reproduces the SSOT noise realisation bit-for-bit;
    /// 3. executes every stage via its [`AnyStage`] object in topological
    ///    order, routed by [`execution_class()`](AnyStage::execution_class)
    ///    and span-wrapped (see [`run_with_handle`](Self::run_with_handle));
    ///    the per-stage span's `batch_id` is the global frame index;
    /// 4. compares the terminal [`HardDecisionBatch`] frame against the
    ///    transmitted message, taking the BP iteration count from the
    ///    [`DecodeScratch`] (CPU chain) or the GPU LDPC stage's per-frame
    ///    counts (GPU chain).
    ///
    /// The four columns derived from the returned counters
    /// are byte-identical to the SSOT [`run_snr_point`](crate::parallel::run_snr_point)
    /// path on the all-CPU chain, and the three columns
    /// `fer`/`frames`/`errors` are byte-identical with the GPU LDPC stage in
    /// the chain (`mean_iters` excluded).
    ///
    /// # Supported chains
    ///
    /// The pipeline must be a **linear** DVB-T2 BICM chain (every edge
    /// `i → i+1`): the 7-stage CPU preset or the 8-stage GPU preset built by
    /// [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2) (or the equivalent graph
    /// wiring). Any other shape yields a typed
    /// [`BuildError::ExecutionValidation`].
    ///
    /// # Errors
    ///
    /// * the [`validate`](Self::validate) errors;
    /// * [`BuildError::ExecutionValidation`] (wrapped fatal) when the chain is
    ///   not a supported linear DVB-T2 shape (no `DvbT2Encode` source, no AWGN
    ///   channel stage, or no iteration source);
    /// * any [`StageError`] a stage itself returns.
    ///
    /// # Complexity
    ///
    /// `O(max_frames)` frame chains across the workers; the chain's shared
    /// codec serialises LDPC decodes on its internal lock.
    pub fn run_dvb_t2_snr_point(
        pipeline: &Pipeline,
        scheduler: &Scheduler,
        snr_idx: usize,
        max_frames: usize,
    ) -> Result<WorkerCounters, StageError> {
        Self::validate(pipeline)?;
        let stages = pipeline.stages();
        let n = stages.len();
        if n == 0 {
            return Err(exec_err(
                "the stage-driven DVB-T2 sweep requires a non-empty pipeline",
            ));
        }

        let mut got: Vec<(usize, usize)> = pipeline
            .edges()
            .iter()
            .map(|e| (e.from.0 as usize, e.to.0 as usize))
            .collect();
        got.sort_unstable();
        let expected: Vec<(usize, usize)> = (0..n - 1).map(|i| (i, i + 1)).collect();
        if got != expected {
            return Err(exec_err(format!(
                "the stage-driven DVB-T2 sweep requires a linear chain \
                 (edges i -> i+1 over {n} stages); got edges {got:?}"
            )));
        }

        // The source stage must be the DVB-T2 encoder (it knows k_bch, the
        // BBFRAME width the per-frame random message is minted at).
        let k = stages[0]
            .stage_as_any()
            .and_then(|a| a.downcast_ref::<DvbT2Encode>())
            .map(DvbT2Encode::k_bch)
            .ok_or_else(|| {
                exec_err(format!(
                    "the stage-driven DVB-T2 sweep requires a DvbT2Encode source stage; \
                     position 0 is `{}`",
                    stages[0].name()
                ))
            })?;

        // The AWGN channel stage whose scratch RNG is positioned per frame.
        let channel_pos = stages
            .iter()
            .position(|s| {
                s.stage_as_any()
                    .is_some_and(|a| a.is::<crate::channels::Awgn>())
            })
            .ok_or_else(|| {
                exec_err(
                    "the stage-driven DVB-T2 sweep requires an AWGN channel stage \
                     (channels::Awgn) in the chain",
                )
            })?;

        // Iteration sources: the combined CPU decode stage (DecodeScratch), or
        // the GPU LDPC BP stage (per-frame counts from the decode call).
        let cpu_decode_pos = stages.iter().position(|s| {
            s.stage_as_any()
                .is_some_and(|a| a.is::<crate::stages::DvbT2Decode>())
        });
        #[cfg(feature = "hip")]
        let gpu_bp_pos = stages.iter().position(|s| {
            s.stage_as_any()
                .is_some_and(|a| a.is::<crate::gpu::ldpc_bp::GpuLdpcBp>())
        });
        #[cfg(not(feature = "hip"))]
        let gpu_bp_pos: Option<usize> = None;
        if cpu_decode_pos.is_none() && gpu_bp_pos.is_none() {
            return Err(exec_err(
                "the stage-driven DVB-T2 sweep has no BP-iteration source: the chain \
                 carries neither a DvbT2Decode stage nor a GPU LDPC BP stage",
            ));
        }

        let seed = scheduler.seed();
        let num_workers = scheduler.parallelism().get();

        let dump_dir_buf_dvb = pipeline
            .config()
            .diagnostic_dump_dir
            .clone()
            .unwrap_or_else(crate::executor::failure::default_dump_dir);
        let failure = FailurePolicy {
            strict_gpu: pipeline.config().strict_gpu,
            dump_dir: &dump_dir_buf_dvb,
            inject_gpu_oom_modulus: pipeline.config().inject_gpu_oom_modulus,
        };

        let per_worker: Vec<Result<WorkerCounters, StageError>> =
            scheduler.rayon_pool().install(|| {
                (0..num_workers)
                    .into_par_iter()
                    .map(|worker_idx| -> Result<WorkerCounters, StageError> {
                        // Logical worker 0 for the RNG (global-frame keying);
                        // the physical worker_idx only selects WHICH frames this
                        // worker runs (and annotates the spans).
                        let mut ctx = WorkerCtx::new(seed, snr_idx, 0);
                        let mut scratches: Vec<Box<dyn AnyScratch>> =
                            stages.iter().map(|s| s.default_scratch()).collect();

                        // Persistent per-worker GPU LDPC state (hip + active
                        // GPU + a GPU LDPC stage in the chain).
                        #[cfg(feature = "hip")]
                        let mut route = {
                            let mut route = WorkerRoute::transient();
                            if let Some(pos) = gpu_bp_pos {
                                if let Some((stream_id, stream)) =
                                    scheduler.worker_stream(worker_idx)
                                {
                                    let gpu_bp = stages[pos]
                                        .stage_as_any()
                                        .and_then(|a| {
                                            a.downcast_ref::<crate::gpu::ldpc_bp::GpuLdpcBp>()
                                        })
                                        .ok_or_else(|| {
                                            exec_err("internal: GPU LDPC position lost")
                                        })?;
                                    let decoder = gpu_bp.build_decoder(1)?;
                                    let scratch = gpu_bp.build_stream_scratch(&decoder)?;
                                    route.gpu = Some(GpuWorkerState {
                                        decoder,
                                        scratch,
                                        stream,
                                        stream_id,
                                    });
                                }
                            }
                            route
                        };
                        #[cfg(not(feature = "hip"))]
                        let mut route = WorkerRoute::transient();

                        let mut counters = WorkerCounters::default();
                        let mut g = worker_idx;
                        while g < max_frames {
                            ctx.reseek_to_frame(g);
                            let message = crate::frame_sim::random_bitvec(k, ctx.rng_mut());

                            // Channel scratch positioned at the post-message
                            // stream offset (the SSOT noise continues from
                            // exactly here).
                            let mut chan_rng = ChaCha20Rng::seed_from_u64(seed);
                            chan_rng.set_word_pos(ctx.current_word_pos());
                            // Deref the box so `as_any_mut` dispatches to the
                            // inner scratch, not the blanket impl on the Box.
                            (*scratches[channel_pos])
                                .as_any_mut()
                                .downcast_mut::<ChannelScratch>()
                                .ok_or_else(|| {
                                    exec_err(
                                        "internal: channel stage scratch is not ChannelScratch",
                                    )
                                })?
                                .rng = chan_rng;

                            // Iteration provenance is tracked at the GPU LDPC
                            // stage position ONLY: an AWGN/demap fallback
                            // substitution must not masquerade as a
                            // BP-iteration source.
                            let mut cur: Box<dyn TypedBatch> =
                                Box::new(BitPackedBatch::new(vec![message.clone()]));
                            let mut gpu_iters: Option<u64> = None;
                            let mut bp_fellback = false;
                            for (pos, stage) in stages.iter().enumerate() {
                                let (out, iters) = execute_stage(
                                    stage.as_ref(),
                                    cur.as_ref(),
                                    scratches[pos].as_mut(),
                                    scheduler,
                                    worker_idx,
                                    snr_idx,
                                    g as u64,
                                    &mut route,
                                    &failure,
                                )?;
                                if Some(pos) == gpu_bp_pos {
                                    match iters {
                                        StageIters::Gpu(v) => {
                                            gpu_iters = v.first().map(|&i| u64::from(i));
                                        }
                                        StageIters::CpuFallback => bp_fellback = true,
                                        StageIters::NotASource => {}
                                    }
                                }
                                cur = out;
                            }

                            let decoded = cur
                                .as_any()
                                .downcast_ref::<HardDecisionBatch>()
                                .and_then(|b| b.frames.first())
                                .ok_or_else(|| {
                                    exec_err(
                                        "the stage-driven DVB-T2 chain must end in a \
                                         one-frame HardDecisionBatch",
                                    )
                                })?;
                            let iterations = match gpu_iters {
                                Some(i) => i,
                                None => {
                                    if let Some(pos) = cpu_decode_pos {
                                        // Deref the box (see the channel scratch
                                        // note above).
                                        (*scratches[pos])
                                            .as_any_mut()
                                            .downcast_mut::<DecodeScratch>()
                                            .and_then(|s| s.iterations.first().copied())
                                            .ok_or_else(|| {
                                                exec_err(
                                                    "internal: DvbT2Decode scratch carries no \
                                                     iteration count for the frame",
                                                )
                                            })?
                                    } else if bp_fellback {
                                        // This frame's GPU LDPC dispatch was
                                        // replaced by the registered CPU
                                        // fallback, whose erased boundary
                                        // surfaces no per-frame count: record
                                        // the stage's `max_iterations` cap. The
                                        // verdict columns
                                        // (`fer`/`frames`/`errors`) are
                                        // unaffected.
                                        let pos = gpu_bp_pos.ok_or_else(|| {
                                            exec_err(
                                                "internal: CpuFallback iteration \
                                                 provenance without a GPU LDPC stage",
                                            )
                                        })?;
                                        gpu_ldpc_max_iters(stages, pos)?
                                    } else {
                                        // No iteration source ran — e.g. the
                                        // GPU LDPC stage degraded to
                                        // `process_any` (no stream) without a
                                        // substitution: a hard error, never a
                                        // silent default.
                                        return Err(exec_err(
                                            "no BP-iteration source ran for this frame \
                                             (no DvbT2Decode stage, no GPU LDPC counts, \
                                             and no fallback substitution)",
                                        ));
                                    }
                                }
                            };
                            let bit_errors =
                                gf2_coding::simulation::count_bit_errors(&message, decoded) as u64;
                            counters.record_frame(bit_errors > 0, iterations, k as u64, bit_errors);
                            g += num_workers;
                        }
                        Ok(counters)
                    })
                    .collect()
            });

        // Reduce in worker_idx order — the SSOT aggregation order.
        let mut all = Vec::with_capacity(per_worker.len());
        for r in per_worker {
            all.push(r?);
        }
        Ok(WorkerCounters::reduce_in_worker_order(&all))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::{Edge, StageId};
    use crate::stage::{erase, Stage};
    use crate::PipelineConfig;
    use gf2_core::BitVec;
    use std::collections::HashMap;

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

    /// `BitPackedBatch` → `LlrBatch` (CPU), to give type-distinct endpoints.
    struct BitToLlr;
    impl Stage<BitPackedBatch, LlrBatch> for BitToLlr {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &BitPackedBatch, _: &mut ()) -> Result<LlrBatch, StageError> {
            Ok(LlrBatch::new(vec![Vec::new(); i.frames.len()]))
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

    fn neutral_config() -> PipelineConfig {
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

    /// Assembles a raw (builder-bypassing) pipeline so the defensive arms —
    /// unreachable through `Chain::build` — can be exercised.
    fn raw_pipeline(stages: Vec<Box<dyn AnyStage>>, edges: Vec<Edge>) -> Pipeline {
        Pipeline::from_parts(stages, edges, HashMap::new(), neutral_config())
    }

    fn bit_edge(from: u32, to: u32) -> Edge {
        Edge {
            from: StageId(from),
            to: StageId(to),
            element_type: std::any::TypeId::of::<BitPackedBatch>(),
            batch_size: 1,
        }
    }

    #[test]
    fn test_validate_rejects_backward_edge_as_execution_validation() {
        let p = raw_pipeline(vec![erase(BitId), erase(BitId)], vec![bit_edge(1, 0)]);
        match TopologyExecutor::validate(&p) {
            Err(StageError::Fatal(FatalError::BuildError(BuildError::ExecutionValidation {
                reason,
            }))) => {
                assert!(
                    reason.contains("topological linearisation"),
                    "reason must name the order violation, got: {reason}"
                );
            }
            other => panic!("expected ExecutionValidation, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_rejects_self_edge() {
        let p = raw_pipeline(vec![erase(BitId)], vec![bit_edge(0, 0)]);
        assert!(matches!(
            TopologyExecutor::validate(&p),
            Err(StageError::Fatal(FatalError::BuildError(
                BuildError::ExecutionValidation { .. }
            )))
        ));
    }

    #[test]
    fn test_validate_rejects_out_of_range_edge() {
        let p = raw_pipeline(vec![erase(BitId)], vec![bit_edge(0, 7)]);
        match TopologyExecutor::validate(&p) {
            Err(StageError::Fatal(FatalError::BuildError(BuildError::ExecutionValidation {
                reason,
            }))) => assert!(reason.contains("outside"), "got: {reason}"),
            other => panic!("expected ExecutionValidation, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_rejects_lineage_type_break() {
        // Edge claims BitPackedBatch but the consumer takes BitPackedBatch and
        // the producer EMITS LlrBatch: producer output != edge element type.
        let p = raw_pipeline(vec![erase(BitToLlr), erase(BitId)], vec![bit_edge(0, 1)]);
        assert!(matches!(
            TopologyExecutor::validate(&p),
            Err(StageError::Fatal(FatalError::BuildError(
                BuildError::TypeMismatch { .. }
            )))
        ));
    }

    #[test]
    fn test_validate_accepts_builder_output() {
        let mut chain = crate::graph::Chain::new();
        let a = chain.add(erase(BitId));
        let b = chain.add(erase(BitId));
        chain.connect(a, b).unwrap();
        let p = chain.build().unwrap();
        TopologyExecutor::validate(&p).expect("builder output is always consistent");
    }

    #[test]
    fn test_run_rejects_wrong_root_input_type() {
        let mut chain = crate::graph::Chain::new();
        chain.add(erase(BitId));
        let p = chain.build().unwrap();
        let sched = Scheduler::new(std::num::NonZeroUsize::new(1).unwrap(), false, 0);
        let wrong: Box<dyn TypedBatch> = Box::new(LlrBatch::new(vec![]));
        assert!(matches!(
            TopologyExecutor::run(&p, &sched, wrong),
            Err(StageError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn test_run_empty_pipeline_has_no_sinks() {
        let p = crate::graph::Chain::new().build().unwrap();
        let sched = Scheduler::new(std::num::NonZeroUsize::new(1).unwrap(), false, 0);
        let out = TopologyExecutor::run(&p, &sched, Box::new(BitPackedBatch::new(vec![])))
            .expect("empty pipeline runs vacuously");
        assert!(out.outputs().is_empty());
    }

    #[test]
    fn test_concat_batches_bitpacked_in_order() {
        let a = BitPackedBatch::new(vec![BitVec::zeros(4)]);
        let b = BitPackedBatch::new(vec![BitVec::ones(4), BitVec::zeros(4)]);
        let merged = concat_batches(&[&a, &b]).expect("canonical type concatenates");
        let merged = merged
            .as_any()
            .downcast_ref::<BitPackedBatch>()
            .expect("stays BitPackedBatch");
        assert_eq!(merged.frames.len(), 3);
        assert_eq!(
            merged.frames[0],
            BitVec::zeros(4),
            "in-edge order preserved"
        );
        assert_eq!(merged.frames[1], BitVec::ones(4));
    }

    #[test]
    fn test_concat_batches_symbol_lanes() {
        let a = SymbolBatch::new(vec![vec![1.0_f32]], vec![vec![2.0_f32]]);
        let b = SymbolBatch::new(vec![vec![3.0_f32]], vec![vec![4.0_f32]]);
        let merged = concat_batches(&[&a, &b]).expect("symbol batches concatenate");
        let merged = merged.as_any().downcast_ref::<SymbolBatch>().unwrap();
        assert_eq!(merged.i, vec![vec![1.0], vec![3.0]]);
        assert_eq!(merged.q, vec![vec![2.0], vec![4.0]]);
    }

    #[test]
    fn test_concat_batches_rejects_mixed_and_unknown_types() {
        let a = BitPackedBatch::new(vec![BitVec::zeros(4)]);
        let b = LlrBatch::new(vec![Vec::new()]);
        assert!(
            concat_batches(&[&a, &b]).is_none(),
            "mixed types do not merge"
        );
        assert!(
            concat_batches(&[]).is_none(),
            "empty part list does not merge"
        );
    }

    #[test]
    fn test_split_half_sizes_and_order() {
        let frames: Vec<BitVec> = (0..5)
            .map(|i| {
                if i < 3 {
                    BitVec::ones(2)
                } else {
                    BitVec::zeros(2)
                }
            })
            .collect();
        let batch = BitPackedBatch::new(frames);
        let (lo, hi) = split_half(&batch).expect("5 frames split");
        assert_eq!(lo.batch_size(), 3, "lo half takes ceil(n/2)");
        assert_eq!(hi.batch_size(), 2);
        let lo = lo.as_any().downcast_ref::<BitPackedBatch>().unwrap();
        assert!(
            lo.frames.iter().all(|f| f == &BitVec::ones(2)),
            "order kept"
        );
    }

    #[test]
    fn test_split_half_rejects_single_frame_batches() {
        let batch = BitPackedBatch::new(vec![BitVec::zeros(2)]);
        assert!(split_half(&batch).is_none());
    }

    #[test]
    fn test_dag_outputs_into_outputs_consumes_and_returns_pairs() {
        let mut chain = crate::graph::Chain::new();
        chain.add(erase(BitId));
        let p = chain.build().unwrap();
        let sched = Scheduler::new(std::num::NonZeroUsize::new(1).unwrap(), false, 0);
        let dag_out = TopologyExecutor::run(&p, &sched, Box::new(BitPackedBatch::new(vec![])))
            .expect("single-stage pipeline runs");
        let pairs = dag_out.into_outputs();
        assert_eq!(pairs.len(), 1, "one sink");
        assert_eq!(pairs[0].0, 0, "stage at position 0");
    }

    #[test]
    fn test_dag_outputs_into_single_returns_none_for_multi_sink() {
        // Two stages with no connecting edge → both are sinks.
        let p = raw_pipeline(vec![erase(BitId), erase(BitId)], vec![]);
        let sched = Scheduler::new(std::num::NonZeroUsize::new(1).unwrap(), false, 0);
        let dag_out = TopologyExecutor::run(&p, &sched, Box::new(BitPackedBatch::new(vec![])))
            .expect("multi-sink pipeline runs");
        assert!(
            dag_out.into_single().is_none(),
            "two sinks → into_single must return None"
        );
    }

    #[test]
    fn test_concat_batches_hard_decision() {
        let a = HardDecisionBatch::new(vec![BitVec::ones(4)]);
        let b = HardDecisionBatch::new(vec![BitVec::zeros(4), BitVec::ones(4)]);
        let merged = concat_batches(&[&a, &b]).expect("HardDecisionBatch concatenates");
        let m = merged.as_any().downcast_ref::<HardDecisionBatch>().unwrap();
        assert_eq!(m.frames.len(), 3);
        assert_eq!(m.frames[0], BitVec::ones(4));
    }

    #[test]
    fn test_split_half_hard_decision() {
        let frames: Vec<_> = (0..4)
            .map(|i| {
                if i < 2 {
                    BitVec::ones(4)
                } else {
                    BitVec::zeros(4)
                }
            })
            .collect();
        let batch = HardDecisionBatch::new(frames);
        let (lo, hi) = split_half(&batch).expect("4 frames split");
        assert_eq!(lo.batch_size(), 2);
        assert_eq!(hi.batch_size(), 2);
        let lo = lo.as_any().downcast_ref::<HardDecisionBatch>().unwrap();
        assert_eq!(lo.frames[0], BitVec::ones(4));
        assert_eq!(lo.frames[1], BitVec::ones(4));
    }

    #[test]
    fn test_concat_batches_llr() {
        use gf2_coding::Llr;
        let a = LlrBatch::new(vec![vec![Llr::new(1.0), Llr::new(-1.0)]]);
        let b = LlrBatch::new(vec![vec![Llr::new(2.0)], vec![Llr::new(3.0)]]);
        let merged = concat_batches(&[&a, &b]).expect("LlrBatch concatenates");
        let m = merged.as_any().downcast_ref::<LlrBatch>().unwrap();
        assert_eq!(m.frames.len(), 3);
        assert_eq!(m.frames[0].len(), 2);
    }

    #[test]
    fn test_split_half_llr() {
        use gf2_coding::Llr;
        let frames: Vec<Vec<Llr>> = (0..3).map(|i| vec![Llr::new(i as f32)]).collect();
        let batch = LlrBatch::new(frames);
        let (lo, hi) = split_half(&batch).expect("3 frames split");
        assert_eq!(lo.batch_size(), 2, "ceil(3/2) = 2");
        assert_eq!(hi.batch_size(), 1);
    }

    #[test]
    fn test_split_half_symbol() {
        let i_data: Vec<Vec<f32>> = (0..4).map(|j| vec![j as f32]).collect();
        let q_data: Vec<Vec<f32>> = (0..4).map(|j| vec![-(j as f32)]).collect();
        let batch = SymbolBatch::new(i_data, q_data);
        let (lo, hi) = split_half(&batch).expect("4-frame SymbolBatch splits");
        assert_eq!(lo.batch_size(), 2);
        assert_eq!(hi.batch_size(), 2);
        let lo = lo.as_any().downcast_ref::<SymbolBatch>().unwrap();
        assert_eq!(lo.i[0], vec![0.0_f32]);
        assert_eq!(lo.i[1], vec![1.0_f32]);
    }

    #[cfg(not(feature = "hip"))]
    struct GpuBitId2;
    #[cfg(not(feature = "hip"))]
    impl Stage<BitPackedBatch, BitPackedBatch> for GpuBitId2 {
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
    #[cfg(not(feature = "hip"))]
    fn test_run_gpu_only_stage_degrades_to_cpu_without_hip() {
        let p = raw_pipeline(vec![erase(GpuBitId2)], vec![]);
        let sched = Scheduler::new(std::num::NonZeroUsize::new(1).unwrap(), false, 0);
        let frame = BitPackedBatch::new(vec![BitVec::ones(4)]);
        let out = TopologyExecutor::run(&p, &sched, Box::new(frame.clone()))
            .expect("GpuOnly degrades to CPU on non-hip build")
            .into_single()
            .expect("single sink");
        let out = out
            .as_any()
            .downcast_ref::<BitPackedBatch>()
            .expect("BitPackedBatch");
        assert_eq!(out.frames, frame.frames);
    }

    struct HybridBitId2;
    impl Stage<BitPackedBatch, BitPackedBatch> for HybridBitId2 {
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
    fn test_run_hybrid_stage_splits_and_merges_batch() {
        let p = raw_pipeline(vec![erase(HybridBitId2)], vec![]);
        let sched = Scheduler::new(std::num::NonZeroUsize::new(1).unwrap(), false, 0);
        let frames: Vec<_> = (0..4).map(|_| BitVec::ones(4)).collect();
        let input = BitPackedBatch::new(frames);
        let out = TopologyExecutor::run(&p, &sched, Box::new(input.clone()))
            .expect("Hybrid stage runs")
            .into_single()
            .expect("single sink");
        let out = out
            .as_any()
            .downcast_ref::<BitPackedBatch>()
            .expect("BitPackedBatch");
        assert_eq!(
            out.frames.len(),
            4,
            "4 frames preserved through hybrid split-merge"
        );
    }
}
