//! The hybrid CPU/GPU pipeline scheduler (Phase C foundational task
//! `75c22fa8`, design doc §6 / §8 / §11).
//!
//! [`Scheduler`] is the engine [`Pipeline::run`](crate::Pipeline::run) drives.
//! It pairs each rayon worker with one HIP stream and overlaps CPU preparation
//! of batch `N+1` against GPU execution of batch `N`:
//!
//! 1. each worker owns a fixed strided partition of the SNR point's global
//!    frames (worker `w` of `W` takes frames `w, w+W, w+2W, …`); within its
//!    partition it processes frames in **batches**;
//! 2. for each batch `N` the worker (a) holds the CPU-prepared LLRs of batch
//!    `N`, (b) enqueues the GPU LDPC belief-propagation decode of batch `N` —
//!    every kernel launch **and** every pinned-staged H2D/D2H transfer — on
//!    its owned stream, and (c) prepares batch `N+1` on the CPU **while**
//!    batch `N` decodes on the GPU; completion is awaited **per-stream**
//!    (`hipStreamSynchronize` inside the decode call), never via device-wide
//!    sync, so two workers' batches on different streams genuinely overlap;
//! 3. once batch `N`'s device codewords come back, the worker runs the SSOT BCH
//!    decode-tail + information-bit error count on the CPU and records the
//!    per-worker counters.
//!
//! # Stage routing by execution class (deliverable 3)
//!
//! [`run`](Scheduler::run) derives its dispatch from the **pipeline's stage
//! list**: it walks [`Pipeline::stages`](crate::Pipeline::stages) matching each
//! stage's [`execution_class()`](crate::stage::AnyStage::execution_class).
//! A discovered [`GpuOnly`](crate::ExecutionClass) stage (the LDPC BP decode
//! the DVB-T2 preset places under `with_gpu(true)`) is downcast to its
//! concrete type and enqueued on the worker's owned HIP stream; every
//! [`CpuOnly`](crate::ExecutionClass) stage (encode, interleave, QAM map,
//! AWGN, demap, BCH decode-tail, error count) runs on the rayon worker — an
//! all-`CpuOnly` pipeline therefore routes to the pinned SSOT CPU dispatch
//! ([`run_snr_point`]), the byte-identity-pinned CpuOnly routing outcome. The
//! DVB-T2 BICM chain has no [`Hybrid`](crate::ExecutionClass)-class stage
//! today (a hybrid stage would split each batch between CPU and GPU); the
//! routing match arm for it is present so a future hybrid stage slots in
//! without reopening the loop.
//!
//! # Determinism (design doc §3 / §11; this task's criterion 3)
//!
//! Each global frame `g`'s randomness is keyed on `g` alone, via the
//! within-SNR seek [`worker_offset`](crate::parallel::worker_offset)`(seed,
//! snr_idx, 0, g)` (the §3 "logical worker 0" convention — `worker_idx` is
//! reserved but **not** used to key the stream, so the per-frame outcome is a
//! pure function of `g` regardless of which physical worker, or how many,
//! processed it). This keeps the byte-identity across worker counts that
//! `3fcb7025` established intact, and makes the hybrid path **run-to-run
//! byte-identical** at a fixed seed (the same device path twice — so
//! `mean_iters` is deterministic here, even though it is EXCLUDED from
//! CPU-vs-GPU byte-identity per §11). Per-worker counters are reduced in
//! `worker_idx` order via
//! [`WorkerCounters::reduce_in_worker_order`](crate::WorkerCounters::reduce_in_worker_order).
//!
//! AWGN stays on the CPU on **both** the CPU-only and hybrid paths (the heavy,
//! GPU-worth stage is LDPC decode); only the LDPC inner decode moves to the
//! device. The channel LLRs the device consumes are therefore byte-identical to
//! the CPU path's, so the only CPU-vs-GPU difference is the §11 BP-convergence
//! ULP drift (which does not change the frame verdict).
//!
//! # Graceful degradation without `hip`
//!
//! Built without the `hip` feature, the scheduler has no device backend: a
//! `gpu_enabled` config is honoured as a one-shot `tracing::warn!` and the run
//! falls back to the CPU-only path (deliverable: documented degrade).

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
/// GPU-decode span (the same boundaries the `tracing` spans mark), so a smoke
/// test can compute the fraction of GPU-active wall-time that overlaps some
/// CPU-active wall-time (deliverable 4 / criterion 1).
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
        // Collect and sort the unique endpoints.
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
/// frames (the DVB-T2 BICM preset is the only run plan today).
///
/// Attached to the [`Pipeline`] by the preset builder ([`Pipeline::dvb_t2`]) so
/// [`Pipeline::run`] can reconstruct the validated [`DvbT2BicmFrameSim`] kernel
/// per SNR point. (Stage **routing** is separate: the scheduler walks the
/// type-erased stage list by execution class — deliverable 3 — while the
/// per-frame CPU work runs through this plan's frame kernel.)
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
    /// Under the `hip` feature with `gpu_enabled`, a [`HipStreamPool`] of
    /// `parallelism` streams on device 0 is created so worker `i` owns stream
    /// `i % parallelism`. If the pool cannot be built (no device / unsupported
    /// arch), the error is mapped and the scheduler degrades to the CPU path
    /// after a `tracing::warn!` (the OOM/unsupported-arch policy is the
    /// executor's, design doc §8; here we simply fall back so a run never aborts
    /// for lack of a GPU).
    ///
    /// [`HipStreamPool`]: gf2_kernels_hip::host::HipStreamPool
    ///
    /// # Arguments
    ///
    /// * `parallelism` — worker count (and stream count under `hip`).
    /// * `gpu_enabled` — whether to offload the GPU-bound stages.
    /// * `seed` — the base ChaCha20 seed (design doc §3).
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

    /// The scheduler's rayon worker pool (crate-internal: the topology
    /// executor `de160fc5` dispatches its waves and sweep workers inside it).
    pub(crate) fn rayon_pool(&self) -> &rayon::ThreadPool {
        &self.rayon_pool
    }

    /// The configured worker count (crate-internal, for the topology executor).
    pub(crate) fn parallelism(&self) -> NonZeroUsize {
        self.parallelism
    }

    /// The base ChaCha20 seed (crate-internal, for the topology executor).
    pub(crate) fn seed(&self) -> u64 {
        self.seed
    }

    /// The HIP stream worker `worker_idx` deterministically owns, with its id
    /// (`worker_idx % n_streams`), or `None` when no GPU pool is active.
    ///
    /// Crate-internal: the topology executor (`de160fc5`) routes a `GpuOnly`
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
    /// `batch` selects the first SNR point's [`BatchHandle::snr_idx`] as the
    /// sweep's starting point context; the full sweep is taken from the
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
    /// # Arguments
    ///
    /// * `pipeline` — the built pipeline (must carry a [`RunPlan`], i.e. have
    ///   been built via the DVB-T2 preset).
    /// * `batch` — the batch handle identifying the run's starting batch/SNR.
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

    /// Like [`run`](Self::run) but also returns the [`OverlapTimeline`] for the
    /// overlap-attestation smoke test (deliverable 4 / criterion 1).
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
                pipeline, plan, snr_idx, es_n0_db, max_frames, &timeline, run_start, strict_gpu,
                &dump_dir, inject_oom_modulus,
            )?;
            per_point.push(SnrPointResult::from_counters(es_n0_db, counters));
        }

        let timeline = timeline.into_inner().expect("overlap timeline mutex");
        Ok((SimulationResults { per_point }, timeline))
    }

    /// Runs one SNR point, returning its aggregate [`WorkerCounters`].
    ///
    /// Dispatch is derived from `pipeline`'s stage list by execution class
    /// (deliverable 3, see the [module docs](self)): a discovered `GpuOnly`
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
            // Deliverable 3: route by the stage list's execution classes. The
            // hybrid loop's GPU stage comes FROM the pipeline (the preset
            // placed it there with its fallback registered), never from a
            // template reconstruction.
            if let Some(gpu_stage) = hybrid::find_gpu_ldpc_stage(pipeline) {
                return self.run_point_hybrid(
                    &template, gpu_stage, snr_idx, max_frames, timeline, run_start, strict_gpu,
                    dump_dir, inject_oom_modulus,
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

        // CpuOnly routing outcome: the within-SNR frame-parallel dispatch (SSOT
        // `3fcb7025`), running inside this scheduler's rayon pool so the worker
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

#[cfg(feature = "hip")]
mod hybrid {
    use super::*;
    use crate::parallel::WorkerCtx;
    use crate::stage::ExecutionClass;
    use gf2_kernels_hip::host::HipStream;
    use rayon::prelude::*;

    /// Frames per GPU decode batch (the double-buffer unit). Sized so the device
    /// LDPC kernel amortises its per-launch overhead while keeping two batches'
    /// worth of host LLR scratch modest.
    const BATCH_FRAMES: usize = 16;

    /// One GPU decode batch's result: the per-frame hard codewords and BP
    /// iteration counts (or a [`StageError`] on a device fault).
    type GpuBatchResult = Result<(Vec<gf2_core::BitVec>, Vec<u32>), StageError>;

    /// Routes `pipeline`'s stage list by [`ExecutionClass`] (deliverable 3) and
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
    /// * [`Hybrid`](ExecutionClass::Hybrid) — would split each batch between
    ///   CPU and GPU. No `Hybrid`-class stage exists in the DVB-T2 BICM chain
    ///   today; the arm is present so a future hybrid stage slots into the
    ///   routing without reopening this loop.
    pub(super) fn find_gpu_ldpc_stage(
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
                // No Hybrid-class stage exists today (see the routing doc
                // above); a future hybrid stage adds its per-batch CPU/GPU
                // split here.
                ExecutionClass::Hybrid => None,
            })
    }

    impl Scheduler {
        /// The hybrid CPU+GPU per-SNR-point driver (design doc §6 overlap
        /// protocol). Each worker owns one HIP stream and double-buffers CPU prep
        /// of batch `N+1` against the GPU LDPC decode of batch `N`.
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
                            // Deliverable 1: worker i OWNS stream i % n_streams,
                            // selected by deterministic index — `pool.acquire()`
                            // would hand out streams in racy call order under
                            // `into_par_iter`, desynchronising the recorded
                            // `stream_id` from the stream actually used. ALL of
                            // this worker's GPU work (launches + pinned
                            // transfers) is enqueued on this stream, and
                            // completion is awaited per-stream
                            // (`hipStreamSynchronize` inside the decode call),
                            // never via device-wide sync.
                            let stream = pool.get(stream_id);
                            self.worker_partition_hybrid(
                                template, gpu_stage, stream, worker_idx, stream_id, snr_idx,
                                max_frames, seed, timeline, run_start, strict_gpu, dump_dir,
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
        /// `stream` is the worker's owned HIP stream: the GPU decode of every
        /// batch is enqueued on it (launches + pinned transfers) and awaited
        /// per-stream inside
        /// [`decode_batch_with_iters_on_stream`](crate::gpu::ldpc_bp::GpuLdpcBp::decode_batch_with_iters_on_stream)
        /// (deliverables 2b / 2d).
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
            // The worker's global frames: worker_idx, +num_workers, … < max_frames.
            let my_frames: Vec<usize> = (worker_idx..max_frames).step_by(num_workers).collect();
            if my_frames.is_empty() {
                return Ok(WorkerCounters::default());
            }

            // Per-worker simulator clone (own decoder for the CPU BCH tail), a
            // per-worker device LDPC decoder sized for one batch, and the
            // per-worker pinned staging the stream-ordered transfers use.
            let sim = template.clone();
            let device = gpu_stage.build_decoder(BATCH_FRAMES)?;
            let mut scratch = gpu_stage.build_stream_scratch(&device)?;
            // The per-worker RNG context (logical worker 0: per-frame seek keyed
            // on the GLOBAL frame index, the §3 byte-identity rule).
            let mut ctx = WorkerCtx::new(seed, snr_idx, 0);

            let mut counters = WorkerCounters::default();

            // CPU-prep one batch of frames into (messages, llrs), inside the
            // stage's tracing span + timeline interval (deliverable 4).
            let prep_batch = |batch_id: usize,
                              frames: &[usize],
                              ctx: &mut WorkerCtx|
             -> Vec<crate::frame_sim::FramePrep> {
                traced_interval(
                    timeline,
                    worker_idx,
                    snr_idx,
                    batch_id,
                    stream_id,
                    "CpuPrep",
                    ActivityKind::CpuPrep,
                    run_start,
                    || {
                        frames
                            .iter()
                            .map(|&g| {
                                ctx.reseek_to_frame(g);
                                sim.prepare_frame(g, ctx)
                            })
                            .collect()
                    },
                )
            };

            // Double-buffer: prep batch 0, then for each batch overlap its GPU
            // decode with the CPU prep of the next batch.
            //
            // The owned device decoder is `!Sync` (it owns device buffers), so
            // it must stay on THIS worker thread. We therefore keep the
            // GPU-blocking call on the worker thread and spawn the CPU prep of
            // the next batch on a scoped helper thread (which captures only
            // `Sync` state — `&sim` is `Sync` via its decoder `Mutex`). The two
            // run concurrently: the helper preps batch N+1 on the CPU while this
            // thread blocks on the GPU decode of batch N. (`std::thread::scope`
            // keeps the borrows safe with no `'static` requirement.)
            let batches: Vec<&[usize]> = my_frames.chunks(BATCH_FRAMES).collect();
            let mut prepared = prep_batch(0, batches[0], &mut ctx);

            for bi in 0..batches.len() {
                let cur_preps = std::mem::take(&mut prepared);
                let next_idx = bi + 1;

                let (gpu_res, next_preps): (GpuBatchResult, Vec<crate::frame_sim::FramePrep>) =
                    std::thread::scope(|scope| {
                        // CPU prep of batch N+1 on a helper thread (Send-only capture).
                        let cpu_handle = scope.spawn(|| {
                            if next_idx < batches.len() {
                                // The per-frame outcome is keyed on the GLOBAL frame
                                // index (§3), not on ctx history, so a throwaway ctx
                                // seeked per frame preserves byte-identity.
                                let mut next_ctx = WorkerCtx::new(seed, snr_idx, 0);
                                prep_batch(next_idx, batches[next_idx], &mut next_ctx)
                            } else {
                                Vec::new()
                            }
                        });

                        // GPU decode of batch N on THIS (worker) thread: every
                        // launch and transfer is stream-ordered on the worker's
                        // owned stream; completion is the per-stream synchronize
                        // inside the decode call (deliverable 2d — no
                        // device-wide sync in the steady-state loop).
                        //
                        // The result is wrapped by `dispatch_with_fallback`
                        // (issue `42eac5cc`): a recoverable OOM triggers a
                        // `tracing::warn!` and re-runs the batch on the CPU
                        // LDPC fallback; a fatal error writes a JSON diagnostic
                        // dump and propagates.
                        let llr_batch_for_fallback = crate::batch::LlrBatch::new(
                            cur_preps.iter().map(|p| p.llrs.clone()).collect(),
                        );
                        // The batch's first global frame index: used to key the
                        // test-only OOM injection (`42eac5cc`) so the modulus
                        // semantics mirror the topology executor's per-frame
                        // keying — only the batch's first frame index is
                        // checked; the whole batch is injected (or not) as one
                        // unit. This is the documented two-surface keying for
                        // `PipelineConfig::inject_gpu_oom_modulus`.
                        let first_global_frame = batches[bi].first().copied().unwrap_or(0) as u64;
                        let gpu_res = traced_interval(
                            timeline,
                            worker_idx,
                            snr_idx,
                            bi,
                            stream_id,
                            "GpuLdpcBp",
                            ActivityKind::GpuDecode,
                            run_start,
                            || -> GpuBatchResult {
                                use crate::executor::failure::{
                                    dispatch_with_fallback, FaultContext, FailurePolicy,
                                };
                                use crate::stage::Stage as _;
                                let ctx = FaultContext {
                                    batch_id: first_global_frame,
                                    snr_idx,
                                    device_id: 0,
                                    worker_idx,
                                };
                                let policy = FailurePolicy {
                                    strict_gpu,
                                    dump_dir,
                                    inject_gpu_oom_modulus: inject_oom_modulus,
                                };
                                // Test-only OOM injection (`42eac5cc`): force a
                                // recoverable OOM when the modulus fires on this
                                // batch's first global frame index, driving the
                                // production `dispatch_with_fallback` path as a
                                // genuine device OOM would.
                                let raw: GpuBatchResult = if policy.injects_oom_at(first_global_frame) {
                                    Err(StageError::Recoverable(
                                        crate::error::RecoverableError::OutOfMemory {
                                            device_id: 0,
                                            bytes_requested: 1024 * 1024 * 1024,
                                        },
                                    ))
                                } else {
                                    gpu_stage
                                        .decode_batch_with_iters_on_stream(
                                            &llr_batch_for_fallback,
                                            &device,
                                            stream,
                                            &mut scratch,
                                        )
                                        .map(|(hard, iters)| (hard.frames, iters))
                                };
                                dispatch_with_fallback(
                                    raw,
                                    || {
                                        // CPU fallback: run the registered CpuLdpcBp
                                        // on the same LLR batch. The per-frame
                                        // iteration count is recorded as
                                        // `max_iterations` (the stage's cap), the
                                        // shared fallback-iters convention (L1)
                                        // documented at the topology executor's
                                        // `gpu_ldpc_max_iters` helper: since
                                        // `mean_iters` is §11-excluded from the
                                        // CPU-vs-GPU contract on a mixed-path run,
                                        // any consistent value is acceptable; the
                                        // cap is the documented convention so the
                                        // two surfaces agree.
                                        let max_iters = gpu_stage.max_iterations() as u32;
                                        let fb = gpu_stage.cpu_fallback().ok_or_else(|| {
                                            StageError::Fatal(
                                                crate::error::FatalError::CpuFallbackAlsoFailed {
                                                    original: Box::new(
                                                        crate::error::RecoverableError::Transient(
                                                            "no CPU fallback on GpuLdpcBp".into(),
                                                        ),
                                                    ),
                                                },
                                            )
                                        })?;
                                        let hard = fb.process(&llr_batch_for_fallback, &mut ())?;
                                        let n = hard.frames.len();
                                        Ok((hard.frames, vec![max_iters; n]))
                                    },
                                    ctx,
                                    strict_gpu,
                                    dump_dir,
                                )
                            },
                        );

                        let next_preps = cpu_handle.join().expect("CPU prep helper thread");
                        (gpu_res, next_preps)
                    });

                let (codewords, iters) = gpu_res?;
                prepared = next_preps;

                // CPU BCH decode-tail + error count for the just-decoded batch.
                traced_interval(
                    timeline,
                    worker_idx,
                    snr_idx,
                    bi,
                    stream_id,
                    "CpuDecodeTail",
                    ActivityKind::CpuPrep,
                    run_start,
                    || {
                        for (i, prep) in cur_preps.iter().enumerate() {
                            let outcome = sim.decode_codeword_to_outcome(
                                &prep.message,
                                &codewords[i],
                                iters[i] as u64,
                            );
                            counters.record_frame(
                                outcome.errored,
                                outcome.iterations,
                                outcome.info_bits,
                                outcome.bit_errors,
                            );
                        }
                    },
                );
            }

            Ok(counters)
        }
    }

    fn elapsed_us(run_start: Instant) -> u128 {
        run_start.elapsed().as_micros()
    }

    /// Runs `f` as one instrumented stage interval (deliverable 4): a
    /// `tracing` span named `pipeline_stage` carrying
    /// `(worker_idx, snr_idx, batch_id, stream_id, stage_name, wall_us)` —
    /// entered for the duration of `f`, with the measured `wall_us` recorded
    /// on the span just before close — plus the matching
    /// [`ActivityInterval`] appended to the [`OverlapTimeline`] (criterion 1's
    /// overlap-attestation surface; the span and the interval mark the same
    /// boundaries).
    #[allow(clippy::too_many_arguments)]
    fn traced_interval<T>(
        timeline: &Mutex<OverlapTimeline>,
        worker_idx: usize,
        snr_idx: usize,
        batch_id: usize,
        stream_id: usize,
        stage_name: &'static str,
        kind: ActivityKind,
        run_start: Instant,
        f: impl FnOnce() -> T,
    ) -> T {
        let span = tracing::info_span!(
            "pipeline_stage",
            worker_idx,
            snr_idx,
            batch_id,
            stream_id,
            stage_name,
            wall_us = tracing::field::Empty
        );
        let entered = span.enter();
        let start_us = elapsed_us(run_start);
        let out = f();
        let end_us = elapsed_us(run_start);
        span.record("wall_us", (end_us - start_us) as u64);
        drop(entered);
        timeline
            .lock()
            .expect("overlap timeline mutex")
            .intervals
            .push(ActivityInterval {
                worker_idx,
                stream_id,
                kind,
                start_us,
                end_us,
            });
        out
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
        // A CPU-only scheduler (gpu_enabled=false) must report gpu_active=false
        // and build cleanly.
        let sched = Scheduler::new(NonZeroUsize::new(2).unwrap(), false, 7);
        assert!(!sched.gpu_active());
    }
}
