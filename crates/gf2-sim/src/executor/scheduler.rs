//! The hybrid CPU/GPU pipeline scheduler behind
//! [`Pipeline::run`](crate::Pipeline::run). Each rayon worker owns a strided
//! partition of the SNR point's global frames (worker `w` of `W` takes
//! `w, w+W, w+2W, …`) and one HIP stream: it enqueues the GPU LDPC decode of
//! batch `N` on that stream, prepares batch `N+1` on the CPU meanwhile, awaits
//! completion per-stream (never via device-wide sync), then runs the BCH
//! decode-tail and information-bit error count on the CPU.
//!
//! # Stage routing by execution class
//!
//! [`run`](Scheduler::run) routes by each stage's
//! [`execution_class()`](crate::stage::AnyStage::execution_class): a
//! [`GpuOnly`](crate::ExecutionClass) LDPC BP decode stage is downcast to its
//! concrete type and enqueued on the worker's owned HIP stream; otherwise the
//! point runs the CPU dispatch [`run_snr_point`]. A
//! [`Hybrid`](crate::ExecutionClass)-class stage selects no GPU dispatch.
//!
//! # Determinism
//!
//! Each global frame `g`'s randomness is keyed on `g` alone, via the
//! within-SNR seek [`worker_offset`](crate::parallel::worker_offset)`(seed,
//! snr_idx, 0, g)`, so the per-frame outcome is independent of which physical
//! worker, or how many, processed it, and the hybrid path is run-to-run
//! byte-identical at a fixed seed. Per-worker counters are reduced in
//! `worker_idx` order via
//! [`WorkerCounters::reduce_in_worker_order`](crate::WorkerCounters::reduce_in_worker_order).
//!
//! AWGN stays on the CPU on both paths; only the LDPC inner decode moves to
//! the device, so the channel LLRs the device consumes are byte-identical to
//! the CPU path's.
//!
//! # Without `hip`
//!
//! A `gpu_enabled` config logs a `tracing::warn!` at scheduler construction
//! and the run takes the CPU-only path.

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Instant;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::DecoderConfig;
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use crate::error::StageError;
use crate::executor::results::{SimulationResults, SnrPointResult};
use crate::frame_sim::DvbT2BicmFrameSim;
use crate::parallel::{run_snr_point, WorkerCounters};
use crate::pipeline::{BatchHandle, Pipeline};

/// The kind of activity an [`OverlapTimeline`] interval records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    /// CPU batch preparation (encode → interleave → map → AWGN → demap) and the
    /// CPU BCH decode-tail / error count.
    CpuPrep,
    /// GPU LDPC belief-propagation decode of a batch on the worker's stream.
    GpuDecode,
}

/// One recorded activity interval: a `(worker, stream, kind, start, end)` span,
/// in microseconds since the run's start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivityInterval {
    /// The rayon worker index that produced the activity.
    pub worker_idx: usize,
    /// The HIP stream id the worker owns (`worker_idx % n_streams`).
    pub stream_id: usize,
    /// Whether this is CPU prep or GPU decode.
    pub kind: ActivityKind,
    /// Interval start, microseconds since run start.
    pub start_us: u128,
    /// Interval end, microseconds since run start.
    pub end_us: u128,
}

/// A timeline of CPU/GPU activity intervals, used to attest CPU↔GPU overlap.
///
/// The hybrid scheduler records one [`ActivityInterval`] per CPU-prep and per
/// GPU-decode span (the same boundaries the `tracing` spans mark).
#[derive(Debug, Clone, Default)]
pub struct OverlapTimeline {
    /// All recorded intervals, in completion order.
    pub intervals: Vec<ActivityInterval>,
}

impl OverlapTimeline {
    /// The fraction of GPU-decode wall-time that overlaps **some** CPU-prep
    /// wall-time, in `[0, 1]`.
    ///
    /// Computed by sweeping the union of all interval endpoints: a sub-interval
    /// counts toward the overlap numerator when at least one `GpuDecode` and at
    /// least one `CpuPrep` interval are simultaneously active over it, and
    /// toward the denominator whenever any `GpuDecode` is active. Returns `0.0`
    /// when no GPU activity was recorded (the CPU-only path).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::executor::{ActivityInterval, ActivityKind, OverlapTimeline};
    ///
    /// // One GPU interval [0,10] fully covered by a CPU interval [0,10].
    /// let tl = OverlapTimeline {
    ///     intervals: vec![
    ///         ActivityInterval { worker_idx: 0, stream_id: 0, kind: ActivityKind::GpuDecode, start_us: 0, end_us: 10 },
    ///         ActivityInterval { worker_idx: 1, stream_id: 1, kind: ActivityKind::CpuPrep,  start_us: 0, end_us: 10 },
    ///     ],
    /// };
    /// assert!((tl.gpu_overlap_fraction() - 1.0).abs() < 1e-9);
    /// ```
    #[must_use]
    pub fn gpu_overlap_fraction(&self) -> f64 {
        if self.intervals.is_empty() {
            return 0.0;
        }
        let mut points: Vec<u128> = Vec::with_capacity(self.intervals.len() * 2);
        for iv in &self.intervals {
            points.push(iv.start_us);
            points.push(iv.end_us);
        }
        points.sort_unstable();
        points.dedup();

        let mut gpu_active_total: u128 = 0;
        let mut gpu_and_cpu_total: u128 = 0;
        for w in points.windows(2) {
            let (lo, hi) = (w[0], w[1]);
            if hi <= lo {
                continue;
            }
            let mid = lo + (hi - lo) / 2; // sample point inside the sub-interval
            let gpu = self.intervals.iter().any(|iv| {
                iv.kind == ActivityKind::GpuDecode && iv.start_us <= mid && mid < iv.end_us
            });
            if !gpu {
                continue;
            }
            let cpu = self.intervals.iter().any(|iv| {
                iv.kind == ActivityKind::CpuPrep && iv.start_us <= mid && mid < iv.end_us
            });
            let span = hi - lo;
            gpu_active_total += span;
            if cpu {
                gpu_and_cpu_total += span;
            }
        }
        if gpu_active_total == 0 {
            0.0
        } else {
            gpu_and_cpu_total as f64 / gpu_active_total as f64
        }
    }
}

/// How a built [`Pipeline`] is run: the parameters the scheduler needs to drive
/// frames. The DVB-T2 BICM preset is the only run plan.
///
/// Attached to the [`Pipeline`] by the preset builder ([`Pipeline::dvb_t2`]) so
/// [`Pipeline::run`] can reconstruct the [`DvbT2BicmFrameSim`] kernel per SNR
/// point. Stage routing is separate: the scheduler walks the type-erased stage
/// list by execution class, while the per-frame CPU work runs through this
/// plan's frame kernel.
///
/// [`Pipeline::dvb_t2`]: crate::Pipeline::dvb_t2
/// [`Pipeline::run`]: crate::Pipeline::run
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunPlan {
    /// A DVB-T2 BICM-AWGN run over the Normal FECFRAME.
    Dvbt2 {
        /// The LDPC code rate.
        rate: CodeRate,
        /// The QAM modulation order.
        modulation: DvbT2Modulation,
        /// The LDPC belief-propagation decoder configuration.
        decoder: DecoderConfig,
        /// The soft-demap method.
        demap: DemapMethod,
    },
}

/// The hybrid CPU/GPU scheduler: a rayon worker pool paired (under `hip`) with a
/// HIP stream pool.
///
/// Built from a [`Pipeline`]'s [`PipelineConfig`](crate::PipelineConfig). The
/// `rayon_pool` fans frames across `parallelism` workers; under the `hip`
/// feature with `gpu_enabled` set, `hip_pool` hands each worker a distinct
/// stream (`worker_idx % n_streams`) for its LDPC decode.
pub struct Scheduler {
    rayon_pool: rayon::ThreadPool,
    #[cfg(feature = "hip")]
    hip_pool: Option<gf2_kernels_hip::host::HipStreamPool>,
    /// Whether the GPU path is active (honoured only under `hip`; a warn-and-
    /// degrade no-op otherwise — hence read only on the `hip` build).
    #[cfg_attr(not(feature = "hip"), allow(dead_code))]
    gpu_enabled: bool,
    parallelism: NonZeroUsize,
    seed: u64,
}

impl Scheduler {
    /// Builds a scheduler for `parallelism` workers.
    ///
    /// Under the `hip` feature with `gpu_enabled`, a `HipStreamPool` (from
    /// `gf2-kernels-hip`) of `parallelism` streams on device 0 is created so
    /// worker `i` owns stream `i % parallelism`. If the pool cannot be built (no
    /// device / unsupported arch), the scheduler logs a `tracing::warn!` and
    /// degrades to the CPU path.
    ///
    /// # Panics
    ///
    /// Panics if the rayon thread-pool cannot be created (an OS resource fault).
    #[must_use]
    pub fn new(parallelism: NonZeroUsize, gpu_enabled: bool, seed: u64) -> Self {
        let rayon_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(parallelism.get())
            .thread_name(|i| format!("gf2-sim-sched-{i}"))
            .build()
            .expect("rayon thread pool");

        #[cfg(feature = "hip")]
        let hip_pool = if gpu_enabled {
            match gf2_kernels_hip::host::HipStreamPool::new(0, parallelism.get()) {
                Ok(pool) => Some(pool),
                Err(e) => {
                    tracing::warn!(
                        error = ?e,
                        "HIP stream pool unavailable; hybrid scheduler degrading to CPU-only path"
                    );
                    None
                }
            }
        } else {
            None
        };

        #[cfg(not(feature = "hip"))]
        if gpu_enabled {
            tracing::warn!(
                "with_gpu(true) set but the crate was built without the `hip` feature; \
                 running the CPU-only path"
            );
        }

        Self {
            rayon_pool,
            #[cfg(feature = "hip")]
            hip_pool,
            gpu_enabled,
            parallelism,
            seed,
        }
    }

    /// Builds a scheduler from a [`Pipeline`]'s config.
    #[must_use]
    pub fn from_pipeline(pipeline: &Pipeline) -> Self {
        let cfg = pipeline.config();
        Self::new(cfg.parallelism, cfg.gpu_enabled, cfg.seed)
    }

    /// Whether this scheduler will actually dispatch GPU work (true only when
    /// `gpu_enabled` **and** a HIP stream pool was successfully built).
    #[must_use]
    pub fn gpu_active(&self) -> bool {
        #[cfg(feature = "hip")]
        {
            self.gpu_enabled && self.hip_pool.is_some()
        }
        #[cfg(not(feature = "hip"))]
        {
            false
        }
    }

    /// The scheduler's rayon worker pool; the topology executor dispatches its
    /// waves and sweep workers inside it.
    pub(crate) fn rayon_pool(&self) -> &rayon::ThreadPool {
        &self.rayon_pool
    }

    /// The configured worker count.
    pub(crate) fn parallelism(&self) -> NonZeroUsize {
        self.parallelism
    }

    /// The base ChaCha20 seed.
    pub(crate) fn seed(&self) -> u64 {
        self.seed
    }

    /// The HIP stream worker `worker_idx` deterministically owns, with its id
    /// (`worker_idx % n_streams`), or `None` when no GPU pool is active.
    ///
    /// The topology executor routes a `GpuOnly`
    /// stage's launches onto this stream. Selection is by fixed index
    /// (`HipStreamPool::get`), never `acquire()` — the pool's round-robin
    /// cursor advances in call order, which is scheduler-dependent under a
    /// parallel iterator and would desynchronise the recorded `stream_id` from
    /// the stream actually used.
    #[cfg(feature = "hip")]
    pub(crate) fn worker_stream(
        &self,
        worker_idx: usize,
    ) -> Option<(usize, &gf2_kernels_hip::host::HipStream)> {
        if !self.gpu_enabled {
            return None;
        }
        let pool = self.hip_pool.as_ref()?;
        let stream_id = worker_idx % pool.len();
        Some((stream_id, pool.get(stream_id)))
    }

    /// Runs `pipeline` over its configured SNR sweep, driving the stages through
    /// the worker pool with double-buffered async CPU/GPU overlap.
    ///
    /// `batch` is unused: the full sweep is taken from the
    /// pipeline's [`PipelineConfig`](crate::PipelineConfig) (`esn0_db_points`,
    /// `max_frames`, `seed`, `parallelism`, `gpu_enabled`, `strict_gpu`).
    ///
    /// GPU stage errors are handled via
    /// [`dispatch_with_fallback`](crate::executor::failure::dispatch_with_fallback):
    /// a recoverable OOM substitutes the registered `CpuLdpcBp` fallback
    /// (with a `tracing::warn!`), unless `strict_gpu` is set (promotion to
    /// [`FatalError::OutOfMemory`](crate::FatalError::OutOfMemory)). Fatal
    /// errors write a JSON diagnostic dump to the configured
    /// `diagnostic_dump_dir` and propagate.
    ///
    /// # Errors
    ///
    /// Returns a [`StageError`] if a GPU stage faults fatally (a missing run
    /// plan is a [`FatalError::BuildError`](crate::FatalError::BuildError)).
    ///
    /// # Complexity
    ///
    /// `O(sum over points of max_frames)` frame kernels across `parallelism`
    /// workers.
    pub fn run(
        &self,
        pipeline: &Pipeline,
        batch: BatchHandle,
    ) -> Result<SimulationResults, StageError> {
        let (results, _timeline) = self.run_instrumented(pipeline, batch)?;
        Ok(results)
    }

    /// Like [`run`](Self::run) but also returns the [`OverlapTimeline`].
    ///
    /// # Errors
    ///
    /// See [`run`](Self::run).
    pub fn run_instrumented(
        &self,
        pipeline: &Pipeline,
        _batch: BatchHandle,
    ) -> Result<(SimulationResults, OverlapTimeline), StageError> {
        let plan = pipeline.run_plan().ok_or_else(|| {
            StageError::Fatal(crate::error::FatalError::BuildError(
                crate::error::BuildError::Disconnected { stages: Vec::new() },
            ))
        })?;
        let cfg = pipeline.config();
        let max_frames = cfg.max_frames as usize;

        let strict_gpu = cfg.strict_gpu;
        let dump_dir = cfg
            .diagnostic_dump_dir
            .clone()
            .unwrap_or_else(crate::executor::failure::default_dump_dir);
        let inject_oom_modulus = cfg.inject_gpu_oom_modulus;

        let timeline = Mutex::new(OverlapTimeline::default());
        let run_start = Instant::now();

        let mut per_point = Vec::with_capacity(cfg.esn0_db_points.len());
        for (snr_idx, &es_n0_db) in cfg.esn0_db_points.iter().enumerate() {
            let counters = self.run_one_point(
                pipeline,
                plan,
                snr_idx,
                es_n0_db,
                max_frames,
                &timeline,
                run_start,
                strict_gpu,
                &dump_dir,
                inject_oom_modulus,
            )?;
            per_point.push(SnrPointResult::from_counters(es_n0_db, counters));
        }

        let timeline = timeline.into_inner().expect("overlap timeline mutex");
        Ok((SimulationResults { per_point }, timeline))
    }

    /// Runs one SNR point, returning its aggregate [`WorkerCounters`].
    ///
    /// Dispatch is derived from `pipeline`'s stage list by execution class
    /// (see the [module docs](self)): a discovered `GpuOnly`
    /// stage routes the point to the hybrid CPU∥GPU driver; an all-`CpuOnly`
    /// stage list routes to the pinned SSOT CPU dispatch below.
    #[allow(clippy::too_many_arguments)]
    fn run_one_point(
        &self,
        pipeline: &Pipeline,
        plan: RunPlan,
        snr_idx: usize,
        es_n0_db: f64,
        max_frames: usize,
        timeline: &Mutex<OverlapTimeline>,
        run_start: Instant,
        strict_gpu: bool,
        dump_dir: &std::path::Path,
        inject_oom_modulus: Option<u64>,
    ) -> Result<WorkerCounters, StageError> {
        let RunPlan::Dvbt2 {
            rate,
            modulation,
            decoder,
            demap,
        } = plan;
        let template = DvbT2BicmFrameSim::new(rate, modulation, es_n0_db, decoder, demap);

        #[cfg(feature = "hip")]
        if self.gpu_active() {
            // The hybrid loop's GPU stage comes FROM the pipeline (the preset
            // placed it there with its fallback registered), never from a
            // template reconstruction.
            if let Some(gpu_stage) = hybrid::find_gpu_ldpc_stage(pipeline) {
                return self.run_point_hybrid(
                    &template,
                    gpu_stage,
                    snr_idx,
                    max_frames,
                    timeline,
                    run_start,
                    strict_gpu,
                    dump_dir,
                    inject_oom_modulus,
                );
            }
            // gpu_enabled but the stage list carries no GpuOnly stage (e.g. a
            // graph-API chain without the preset's GPU placement): every stage
            // is CpuOnly, so the SSOT CPU dispatch below IS the routing outcome.
        }
        #[cfg(not(feature = "hip"))]
        let _ = pipeline; // no device backend: every stage routes CpuOnly
                          // strict_gpu, dump_dir, and inject_oom_modulus only
                          // apply on the GPU path.
        let _ = (strict_gpu, dump_dir, inject_oom_modulus);

        // CpuOnly routing outcome: the within-SNR frame-parallel dispatch,
        // running inside this scheduler's rayon pool so the worker
        // count is honoured. Byte-identical to the `run_snr_point` contract
        // (pinned by `tests/pipeline_run_cpu.rs`).
        let _ = (timeline, run_start); // unused on the CPU path
        let counters = self.rayon_pool.install(|| {
            run_snr_point(
                self.seed,
                snr_idx,
                max_frames,
                self.parallelism,
                || template.clone(),
                |g, ctx, sim| sim.simulate_frame(g, ctx),
            )
        });
        Ok(counters)
    }
}

/// GPU-stage discovery, re-exported so the checkpointed hybrid runner
/// (`executor::drain`) shares this scheduler's routing.
#[cfg(feature = "hip")]
pub(crate) use hybrid::find_gpu_ldpc_stage;

#[cfg(feature = "hip")]
mod hybrid {
    use super::*;
    use crate::executor::hybrid_core::{
        run_hybrid_double_buffer, BatchHooks, GpuBatchResult, HybridRunCtx, WorkerDevice,
        BATCH_FRAMES,
    };
    use crate::stage::ExecutionClass;
    use gf2_kernels_hip::host::HipStream;
    use rayon::prelude::*;

    /// Routes `pipeline`'s stage list by [`ExecutionClass`] and
    /// returns the discovered `GpuOnly` LDPC BP decode stage, if any.
    ///
    /// The three routing arms:
    ///
    /// * [`CpuOnly`](ExecutionClass::CpuOnly) — runs on the rayon worker (the
    ///   SSOT frame kernel covers the whole CPU BICM chain), so it contributes
    ///   no GPU dispatch here.
    /// * [`GpuOnly`](ExecutionClass::GpuOnly) — enqueued on the worker's owned
    ///   HIP stream. The only `GpuOnly` stage the DVB-T2 preset places is the
    ///   LDPC BP decode, downcast back to its concrete type via
    ///   [`stage_as_any`](crate::stage::AnyStage::stage_as_any).
    /// * [`Hybrid`](ExecutionClass::Hybrid) — contributes no GPU dispatch
    ///   here; the DVB-T2 BICM chain has no `Hybrid`-class stage.
    pub(crate) fn find_gpu_ldpc_stage(
        pipeline: &Pipeline,
    ) -> Option<&crate::gpu::ldpc_bp::GpuLdpcBp> {
        pipeline
            .stages()
            .iter()
            .find_map(|stage| match stage.execution_class() {
                ExecutionClass::CpuOnly => None,
                ExecutionClass::GpuOnly => stage
                    .stage_as_any()
                    .and_then(|s| s.downcast_ref::<crate::gpu::ldpc_bp::GpuLdpcBp>()),
                ExecutionClass::Hybrid => None,
            })
    }

    impl Scheduler {
        /// The hybrid CPU+GPU per-SNR-point driver. Each worker owns one HIP
        /// stream and double-buffers CPU prep of batch `N+1` against the GPU
        /// LDPC decode of batch `N`.
        ///
        /// `gpu_stage` is the pipeline-discovered `GpuOnly` LDPC decode stage
        /// (from [`find_gpu_ldpc_stage`]); each worker builds its own device
        /// decoder + pinned staging from it (device buffers are per-worker,
        /// never shared).
        #[allow(clippy::too_many_arguments)]
        pub(super) fn run_point_hybrid(
            &self,
            template: &DvbT2BicmFrameSim,
            gpu_stage: &crate::gpu::ldpc_bp::GpuLdpcBp,
            snr_idx: usize,
            max_frames: usize,
            timeline: &Mutex<OverlapTimeline>,
            run_start: Instant,
            strict_gpu: bool,
            dump_dir: &std::path::Path,
            inject_oom_modulus: Option<u64>,
        ) -> Result<WorkerCounters, StageError> {
            let pool = self
                .hip_pool
                .as_ref()
                .expect("gpu_active() implies a stream pool");
            let n_streams = pool.len();
            let num_workers = self.parallelism.get();
            let seed = self.seed;

            let per_worker: Vec<Result<WorkerCounters, StageError>> =
                self.rayon_pool.install(|| {
                    (0..num_workers)
                        .into_par_iter()
                        .map(|worker_idx| {
                            let stream_id = worker_idx % n_streams;
                            // Worker i owns stream i % n_streams, selected by
                            // deterministic index — `pool.acquire()` would hand
                            // out streams in racy call order under
                            // `into_par_iter`, desynchronising the recorded
                            // `stream_id` from the stream actually used.
                            let stream = pool.get(stream_id);
                            self.worker_partition_hybrid(
                                template,
                                gpu_stage,
                                stream,
                                worker_idx,
                                stream_id,
                                snr_idx,
                                max_frames,
                                seed,
                                timeline,
                                run_start,
                                strict_gpu,
                                dump_dir,
                                inject_oom_modulus,
                            )
                        })
                        .collect()
                });

            // Reduce in worker_idx (slice) order — the SSOT aggregation order.
            let mut counters = Vec::with_capacity(per_worker.len());
            for r in per_worker {
                counters.push(r?);
            }
            Ok(WorkerCounters::reduce_in_worker_order(&counters))
        }

        /// One worker's strided frame partition, double-buffered CPU/GPU.
        ///
        /// A thin wrapper over
        /// [`run_hybrid_double_buffer`](crate::executor::hybrid_core::run_hybrid_double_buffer):
        /// it computes the worker's strided partition, builds the per-worker
        /// device decoder + pinned staging, and supplies the uncheckpointed
        /// failure semantics via [`SchedulerBatchHooks`].
        #[allow(clippy::too_many_arguments)]
        fn worker_partition_hybrid(
            &self,
            template: &DvbT2BicmFrameSim,
            gpu_stage: &crate::gpu::ldpc_bp::GpuLdpcBp,
            stream: &HipStream,
            worker_idx: usize,
            stream_id: usize,
            snr_idx: usize,
            max_frames: usize,
            seed: u64,
            timeline: &Mutex<OverlapTimeline>,
            run_start: Instant,
            strict_gpu: bool,
            dump_dir: &std::path::Path,
            inject_oom_modulus: Option<u64>,
        ) -> Result<WorkerCounters, StageError> {
            let num_workers = self.parallelism.get();
            let my_frames: Vec<usize> = (worker_idx..max_frames).step_by(num_workers).collect();
            if my_frames.is_empty() {
                return Ok(WorkerCounters::default());
            }

            // Per-worker simulator clone (own decoder for the CPU BCH tail), a
            // per-worker device LDPC decoder sized for one batch, and the
            // per-worker pinned staging the stream-ordered transfers use.
            let sim = template.clone();
            let decoder = gpu_stage.build_decoder(BATCH_FRAMES)?;
            let mut scratch = gpu_stage.build_stream_scratch(&decoder)?;
            let mut device = WorkerDevice {
                device: &decoder,
                scratch: &mut scratch,
            };

            let run_ctx = HybridRunCtx {
                worker_idx,
                stream_id,
                snr_idx,
                seed,
                timeline: Some(timeline),
                run_start,
            };
            let mut hooks = SchedulerBatchHooks {
                gpu_stage,
                strict_gpu,
                dump_dir,
                inject_oom_modulus,
                worker_idx,
                snr_idx,
            };

            // The uncheckpointed scheduler runs every batch (no SIGINT stop) and
            // does not observe per-frame (the checkpointed runner does both).
            let (counters, _frames_done) = run_hybrid_double_buffer(
                &sim,
                &mut device,
                stream,
                &run_ctx,
                &my_frames,
                &mut hooks,
                &|_g| {},
            )?;
            Ok(counters)
        }
    }

    /// The uncheckpointed scheduler's per-batch decode dispatch: every GPU
    /// decode wrapped in
    /// [`dispatch_with_fallback`](crate::executor::failure::dispatch_with_fallback)
    /// (substitute the CPU LDPC fallback on a recoverable fault unless
    /// `strict_gpu`), plus the test-only OOM injection. The checkpointed drain
    /// loop propagates the fault instead (see `executor/drain.rs`).
    struct SchedulerBatchHooks<'a> {
        gpu_stage: &'a crate::gpu::ldpc_bp::GpuLdpcBp,
        strict_gpu: bool,
        dump_dir: &'a std::path::Path,
        inject_oom_modulus: Option<u64>,
        worker_idx: usize,
        snr_idx: usize,
    }

    impl BatchHooks for SchedulerBatchHooks<'_> {
        fn decode_batch(
            &mut self,
            device: &mut WorkerDevice<'_>,
            stream: &HipStream,
            _batch_idx: usize,
            first_global_frame: u64,
            preps: &[crate::frame_sim::FramePrep],
        ) -> GpuBatchResult {
            use crate::executor::failure::{dispatch_with_fallback, FailurePolicy, FaultContext};
            use crate::stage::Stage as _;

            let llr_batch_for_fallback =
                crate::batch::LlrBatch::new(preps.iter().map(|p| p.llrs.clone()).collect());
            let fault_ctx = FaultContext {
                batch_id: first_global_frame,
                snr_idx: self.snr_idx,
                device_id: 0,
                worker_idx: self.worker_idx,
            };
            let policy = FailurePolicy {
                strict_gpu: self.strict_gpu,
                dump_dir: self.dump_dir,
                inject_gpu_oom_modulus: self.inject_oom_modulus,
            };
            // Test-only OOM injection: the batch's first global frame index
            // keys the modulus and the whole batch is injected (or not) as one
            // unit, driving the production `dispatch_with_fallback` path as a
            // genuine device OOM would.
            let raw: GpuBatchResult = if policy.injects_oom_at(first_global_frame) {
                Err(StageError::Recoverable(
                    crate::error::RecoverableError::OutOfMemory {
                        device_id: 0,
                        bytes_requested: 1024 * 1024 * 1024,
                    },
                ))
            } else {
                self.gpu_stage
                    .decode_batch_with_iters_on_stream(
                        &llr_batch_for_fallback,
                        device.device,
                        stream,
                        device.scratch,
                    )
                    .map(|(hard, iters)| (hard.frames, iters))
            };
            dispatch_with_fallback(
                raw,
                || {
                    // CPU fallback: run the registered CpuLdpcBp on the same LLR
                    // batch. The per-frame iteration count is recorded as
                    // `max_iterations` (the stage's cap), the convention shared
                    // with the topology executor's `gpu_ldpc_max_iters`;
                    // `mean_iters` is excluded from the CPU-vs-GPU
                    // byte-identity contract.
                    let max_iters = self.gpu_stage.max_iterations() as u32;
                    let fb = self.gpu_stage.cpu_fallback().ok_or_else(|| {
                        StageError::Fatal(crate::error::FatalError::CpuFallbackAlsoFailed {
                            original: Box::new(crate::error::RecoverableError::Transient(
                                "no CPU fallback on GpuLdpcBp".into(),
                            )),
                        })
                    })?;
                    let hard = fb.process(&llr_batch_for_fallback, &mut ())?;
                    let n = hard.frames.len();
                    Ok((hard.frames, vec![max_iters; n]))
                },
                fault_ctx,
                self.strict_gpu,
                self.dump_dir,
            )
        }

        fn stop_after_batch(&self, _batch_idx: usize) -> bool {
            // The uncheckpointed scheduler has no SIGINT stop: it runs every
            // batch of the worker's partition to completion.
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlap_fraction_full_overlap() {
        let tl = OverlapTimeline {
            intervals: vec![
                ActivityInterval {
                    worker_idx: 0,
                    stream_id: 0,
                    kind: ActivityKind::GpuDecode,
                    start_us: 0,
                    end_us: 100,
                },
                ActivityInterval {
                    worker_idx: 1,
                    stream_id: 1,
                    kind: ActivityKind::CpuPrep,
                    start_us: 0,
                    end_us: 100,
                },
            ],
        };
        assert!((tl.gpu_overlap_fraction() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_overlap_fraction_half_overlap() {
        // GPU active [0,100]; CPU active only over [0,50] → 50% overlap.
        let tl = OverlapTimeline {
            intervals: vec![
                ActivityInterval {
                    worker_idx: 0,
                    stream_id: 0,
                    kind: ActivityKind::GpuDecode,
                    start_us: 0,
                    end_us: 100,
                },
                ActivityInterval {
                    worker_idx: 1,
                    stream_id: 1,
                    kind: ActivityKind::CpuPrep,
                    start_us: 0,
                    end_us: 50,
                },
            ],
        };
        let f = tl.gpu_overlap_fraction();
        assert!((f - 0.5).abs() < 1e-9, "expected 0.5, got {f}");
    }

    #[test]
    fn test_overlap_fraction_no_gpu_is_zero() {
        let tl = OverlapTimeline {
            intervals: vec![ActivityInterval {
                worker_idx: 0,
                stream_id: 0,
                kind: ActivityKind::CpuPrep,
                start_us: 0,
                end_us: 100,
            }],
        };
        assert_eq!(tl.gpu_overlap_fraction(), 0.0);
    }

    #[test]
    fn test_overlap_fraction_no_overlap_is_zero() {
        // GPU [0,50], CPU [50,100] — disjoint, no overlap.
        let tl = OverlapTimeline {
            intervals: vec![
                ActivityInterval {
                    worker_idx: 0,
                    stream_id: 0,
                    kind: ActivityKind::GpuDecode,
                    start_us: 0,
                    end_us: 50,
                },
                ActivityInterval {
                    worker_idx: 0,
                    stream_id: 0,
                    kind: ActivityKind::CpuPrep,
                    start_us: 50,
                    end_us: 100,
                },
            ],
        };
        assert_eq!(tl.gpu_overlap_fraction(), 0.0);
    }

    #[test]
    fn test_scheduler_cpu_only_runs_without_gpu() {
        let sched = Scheduler::new(NonZeroUsize::new(2).unwrap(), false, 7);
        assert!(!sched.gpu_active());
    }
}
