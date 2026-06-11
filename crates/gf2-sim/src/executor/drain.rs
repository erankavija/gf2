//! GPU drain-for-checkpoint and the checkpointed hybrid CPU+GPU sweep
//! (Phase C task `571c11c4`, design doc §4 "Drain commit contract").
//!
//! This module adds checkpoint/resume to the hybrid scheduler (`75c22fa8`):
//!
//! * [`StreamInFlight`] — the per-stream "in-flight batches" tally, so the
//!   drain knows when every stream is idle.
//! * [`Scheduler::drain_for_checkpoint`] — §4 steps 1–2: per-stream
//!   `hipStreamSynchronize()` on each **owned** stream (never
//!   `hipDeviceSynchronize()`, which would block unrelated contexts), then a
//!   tally check enforcing the no-partial-batches commit contract.
//! * [`Scheduler::run_sweep_checkpointed`] — the checkpointed SNR sweep over
//!   the hybrid (or, without a GPU, the unchanged CPU `5f12e7ff`) executor;
//!   [`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed) is the
//!   convenience entry point.
//!
//! # The §4 drain commit contract, as implemented
//!
//! At every heartbeat boundary (and at the SIGINT stop):
//!
//! 1. the round's rayon `join` settles every worker — each in-flight GPU batch
//!    **completes** (the per-stream synchronize inside the stream-ordered
//!    decode call returns) and increments its worker's progress **before** the
//!    flush;
//! 2. [`Scheduler::drain_for_checkpoint`] then synchronizes each owned stream
//!    per-stream and verifies the [`StreamInFlight`] tally is zero — no partial
//!    batches are ever recorded;
//! 3. the per-worker `frames_in_worker` counts are latched **after** the drain
//!    and written atomically via
//!    [`CheckpointWriter`](crate::checkpoint::CheckpointWriter) (the
//!    `rng_word_pos` each [`WorkerState`](crate::checkpoint::WorkerState)
//!    records is the §4-formula position with the real `worker_idx`, kept for
//!    v2 schema fidelity — no executor reads it back).
//!
//! # The hybrid resume model (§4 amendment 2026-06-10)
//!
//! The landed C.1 scheduler distributes **strided** partitions (worker `w` of
//! `W` owns global frames `w, w+W, …`) and keys every frame's RNG on the
//! global frame index (`worker_offset(seed, snr_idx, 0, g)`, the §3
//! logical-worker-0 convention). Resume therefore restores each worker's
//! **progress** from `worker_states[].frames_in_worker` — worker `w` continues
//! at global frame `w + frames_in_worker·W` — and folds the saved counters;
//! per-frame RNG positions are re-derived from the global index exactly as in
//! an uninterrupted run, so byte-identity holds by construction at the same
//! seed and worker count.
//!
//! # Batch alignment (byte-identity of `mean_iters` vs the C.1 reference)
//!
//! Heartbeat rounds are sized in whole per-worker batches of
//! `BATCH_FRAMES` (the C.1 double-buffer unit), and a SIGINT stops workers at
//! a **batch boundary**. Every batch the checkpointed runner ever launches is
//! therefore the same `chunks(BATCH_FRAMES)` slice of the worker's partition
//! that the uncheckpointed scheduler launches — across interrupts and resumes
//! — so the per-frame GPU decode results (hard codewords **and** BP iteration
//! counts) match the C.1 hybrid reference exactly, batch for batch.
//!
//! # Same-host scope (criterion 2) and cross-path resume
//!
//! Resume byte-identity is asserted on the **same host** (the single-gfx1030
//! CI): the v2 checkpoint JSON itself is host-independent, but §11 only
//! relaxes CPU-vs-GPU byte-identity to three columns, so a checkpoint written
//! by one device path and resumed on different silicon is *portable* yet not
//! *bit-attested*. Cross-**path** resume (CPU-written → hybrid-resumed or vice
//! versa) is rejected up front: `gpu_enabled` is part of
//! [`config_hash`](crate::checkpoint::config_hash), because the two executors
//! record differently shaped `worker_states[]` and path-specific
//! `total_iterations`. The residual gap — a `gpu_enabled` config that degraded
//! to the CPU path (no device) and is later resumed where a device exists — is
//! guarded by the worker-state sum validation in the hybrid runner, which
//! cannot distinguish every such state; same-host resume, the asserted scope,
//! never hits it.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::checkpoint::{
    config_hash, run_snr_point_checkpointed, CheckpointReader, CheckpointV2, CheckpointWriter,
    CheckpointedRun, SweepError,
};
use crate::config::PipelineConfig;
use crate::error::{BuildError, FatalError, StageError};
use crate::executor::results::{SimulationResults, SnrPointResult};
use crate::executor::{RunPlan, Scheduler};
use crate::frame_sim::DvbT2BicmFrameSim;
use crate::pipeline::Pipeline;

/// Per-stream "in-flight GPU batches" tally (design doc §4, deliverable 1).
///
/// Each hybrid worker increments its owned stream's slot immediately before
/// enqueuing a batch's stream-ordered GPU work and decrements it once the
/// per-stream synchronize inside the decode call has returned (the batch is
/// off the device). [`Scheduler::drain_for_checkpoint`] consults the tally to
/// know every stream is idle before the checkpoint is written: a non-zero
/// count after the worker join means a worker abandoned a batch mid-flight (a
/// fault path), and the drain refuses to commit — the §4 "no partial batches"
/// contract.
///
/// Purely host-side bookkeeping (atomics, no `unsafe`, no HIP types), so it is
/// available — and the drain's tally check runs — on every build.
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::StreamInFlight;
///
/// let tally = StreamInFlight::new(2);
/// tally.enqueued(0);
/// assert_eq!(tally.in_flight(0), 1);
/// assert_eq!(tally.total_in_flight(), 1);
/// tally.completed(0);
/// assert_eq!(tally.total_in_flight(), 0);
/// ```
#[derive(Debug)]
pub struct StreamInFlight {
    /// One in-flight count per stream id.
    counts: Vec<AtomicU64>,
}

impl StreamInFlight {
    /// Creates a tally for `n_streams` streams, all idle.
    ///
    /// # Arguments
    ///
    /// * `n_streams` — the stream-pool size (one slot per stream id).
    #[must_use]
    pub fn new(n_streams: usize) -> Self {
        Self {
            counts: (0..n_streams).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    /// The number of stream slots in this tally.
    #[must_use]
    pub fn streams(&self) -> usize {
        self.counts.len()
    }

    /// Records one batch enqueued on `stream_id` (call immediately before the
    /// stream-ordered launch).
    ///
    /// # Panics
    ///
    /// Panics if `stream_id >= self.streams()`.
    pub fn enqueued(&self, stream_id: usize) {
        self.counts[stream_id].fetch_add(1, Ordering::AcqRel);
    }

    /// Records one batch completed on `stream_id` (call once the per-stream
    /// synchronize for that batch has returned).
    ///
    /// # Panics
    ///
    /// Panics if `stream_id >= self.streams()`, or if the stream's count is
    /// already zero (a completion without a matching [`enqueued`](Self::enqueued)
    /// is a bookkeeping bug).
    pub fn completed(&self, stream_id: usize) {
        let prev = self.counts[stream_id].fetch_sub(1, Ordering::AcqRel);
        assert!(
            prev > 0,
            "StreamInFlight::completed(stream {stream_id}) without a matching enqueued"
        );
    }

    /// The number of batches currently in flight on `stream_id`.
    ///
    /// # Panics
    ///
    /// Panics if `stream_id >= self.streams()`.
    #[must_use]
    pub fn in_flight(&self, stream_id: usize) -> u64 {
        self.counts[stream_id].load(Ordering::Acquire)
    }

    /// The total number of batches in flight across all streams.
    #[must_use]
    pub fn total_in_flight(&self) -> u64 {
        self.counts.iter().map(|c| c.load(Ordering::Acquire)).sum()
    }
}

/// The outcome of a checkpointed [`Pipeline`] sweep
/// ([`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed) /
/// [`Scheduler::run_sweep_checkpointed`]).
///
/// `results.per_point` holds one [`SnrPointResult`] per SNR point reached, in
/// sweep order; when `interrupted` is `true` the sweep stopped early on
/// SIGINT/SIGTERM and the **last** entry is the interrupted point's partial
/// aggregate (its resumable checkpoint was already flushed — resume with
/// [`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed)`(true)`).
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::{CheckpointedSweep, SimulationResults};
///
/// let sweep = CheckpointedSweep {
///     results: SimulationResults::empty(),
///     interrupted: false,
/// };
/// assert!(!sweep.interrupted);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointedSweep {
    /// The per-SNR-point aggregates for every point reached.
    pub results: SimulationResults,
    /// `true` if the sweep stopped early on SIGINT/SIGTERM.
    pub interrupted: bool,
}

/// Builds the `ExecutionValidation` stage error this module reports for
/// checkpointed-run configuration / drain-contract violations.
fn execution_validation(reason: String) -> StageError {
    StageError::Fatal(FatalError::BuildError(BuildError::ExecutionValidation {
        reason,
    }))
}

impl Scheduler {
    /// Drains the GPU for a checkpoint flush (design doc §4 steps 1–2,
    /// deliverable 1): synchronizes each **owned** HIP stream per-stream and
    /// verifies the [`StreamInFlight`] tally shows every stream idle.
    ///
    /// Stream synchronization is **per-stream**
    /// ([`HipStream::synchronize`](gf2_kernels_hip::host::HipStream::synchronize),
    /// i.e. `hipStreamSynchronize()`) — never `hipDeviceSynchronize()`, which
    /// would block unrelated contexts (§4). The owned streams are the ones the
    /// hybrid workers select by fixed index (`worker_idx % n_streams` via
    /// `HipStreamPool::get`, never `acquire()`); each is synchronized exactly
    /// once. On a CPU-only scheduler (or a build without the `hip` feature)
    /// there are no streams to drain and only the tally check runs.
    ///
    /// Call **after** the worker join and **before** latching `worker_states[]`
    /// / writing the checkpoint, so the recorded per-worker progress reflects
    /// only fully completed batches (the §4 "no partial batches" commit
    /// contract).
    ///
    /// # Arguments
    ///
    /// * `tally` — the run's per-stream in-flight tally. A non-zero count here
    ///   (after the join, no worker can still be running) means a worker
    ///   abandoned a batch mid-flight, and the drain refuses to commit.
    ///
    /// # Errors
    ///
    /// * A mapped [`StageError`] if a per-stream synchronize faults
    ///   (via [`map_hip_error`](crate::gpu::map_hip_error)).
    /// * [`FatalError::BuildError`]`(`[`BuildError::ExecutionValidation`]`)` if
    ///   any stream still shows in-flight batches — the checkpoint must not be
    ///   written in that state.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroUsize;
    /// use gf2_sim::executor::StreamInFlight;
    /// use gf2_sim::Scheduler;
    ///
    /// // A CPU-only scheduler has no streams; an idle tally drains cleanly.
    /// let sched = Scheduler::new(NonZeroUsize::new(2).unwrap(), false, 7);
    /// let tally = StreamInFlight::new(2);
    /// assert!(sched.drain_for_checkpoint(&tally).is_ok());
    /// ```
    ///
    /// # Complexity
    ///
    /// One `hipStreamSynchronize` per owned stream (blocking until that
    /// stream's enqueued work completes), plus an `O(streams)` tally scan.
    pub fn drain_for_checkpoint(&self, tally: &StreamInFlight) -> Result<(), StageError> {
        // §4 step 2: per-stream synchronize on every OWNED stream, each exactly
        // once (worker_idx -> worker_idx % n_streams can map several workers to
        // one stream).
        #[cfg(feature = "hip")]
        if self.gpu_active() {
            let mut synced = std::collections::HashSet::new();
            for worker_idx in 0..self.parallelism().get() {
                if let Some((stream_id, stream)) = self.worker_stream(worker_idx) {
                    if synced.insert(stream_id) {
                        stream
                            .synchronize()
                            .map_err(|e| crate::gpu::map_hip_error(e, "drain_for_checkpoint"))?;
                    }
                }
            }
        }

        // §4 "no partial batches": after the worker join every enqueued batch
        // must have completed. A non-zero tally cannot recover here (no worker
        // is running), so refuse to commit rather than record partial progress.
        for stream_id in 0..tally.streams() {
            let in_flight = tally.in_flight(stream_id);
            if in_flight != 0 {
                return Err(execution_validation(format!(
                    "drain_for_checkpoint: stream {stream_id} still reports {in_flight} \
                     in-flight batch(es) after the worker join; a worker abandoned a \
                     batch mid-flight, so the checkpoint is not written (design doc §4 \
                     drain commit contract)"
                )));
            }
        }
        Ok(())
    }

    /// Runs `pipeline`'s configured SNR sweep with heartbeat + SNR-boundary +
    /// SIGINT checkpointing and `--resume` support (deliverables 2–3).
    ///
    /// Per SNR point: when this scheduler is GPU-active and the pipeline
    /// carries a `GpuOnly` LDPC stage, the point runs on the **checkpointed
    /// hybrid executor** (strided partitions, double-buffered CPU prep ∥ GPU
    /// decode, per-stream drain before every flush); otherwise it runs on the
    /// unchanged CPU runner
    /// [`run_snr_point_checkpointed`](crate::checkpoint::run_snr_point_checkpointed)
    /// (`5f12e7ff` semantics: resume via the global `frames_completed`).
    ///
    /// With `resume`, each point's `<checkpoint_dir>/snr_<NNNN>.json` is loaded
    /// first: completed points fold their saved counters and are skipped; a
    /// partial point resumes — on the hybrid path by restoring each worker's
    /// strided-partition progress from `worker_states[].frames_in_worker` (the
    /// §4 2026-06-10 amendment; `rng_word_pos` is never read back). The
    /// resumed aggregate is byte-identical to an uninterrupted run at the same
    /// seed and worker count because every frame's outcome is a pure function
    /// of its global frame index.
    ///
    /// On SIGINT (or [`request_interrupt`](crate::checkpoint::request_interrupt))
    /// the in-flight GPU batches complete, the streams are drained, a resumable
    /// checkpoint is flushed, and the sweep returns with `interrupted = true`
    /// (criterion 1).
    ///
    /// # Arguments
    ///
    /// * `pipeline` — the built pipeline; must carry a
    ///   [`RunPlan`](crate::executor::RunPlan) (preset-built) and a
    ///   `checkpoint_dir` in its config.
    /// * `resume` — `true` to load and continue from existing checkpoints.
    /// * `frame_observer` — called once per frame as `(snr_idx, global_frame)`
    ///   while the point is simulating (hybrid path: after the frame's CPU
    ///   prep; CPU path: after the frame completes). Campaigns can emit
    ///   progress from it; tests use it to land a deterministic
    ///   [`request_interrupt`](crate::checkpoint::request_interrupt)
    ///   mid-flight. Pass `&|_, _| {}` if unused.
    ///
    /// # Returns
    ///
    /// A [`CheckpointedSweep`]; see its docs for the `interrupted` semantics.
    ///
    /// # Errors
    ///
    /// [`SweepError::Load`] for an invalid/mismatched checkpoint,
    /// [`SweepError::Io`] for a failed checkpoint write, [`SweepError::Stage`]
    /// for a GPU fault, a failed drain, or a missing `checkpoint_dir` /
    /// [`RunPlan`](crate::executor::RunPlan).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_sim::{Pipeline, Scheduler};
    /// # let pipeline: Pipeline = unimplemented!();
    /// let scheduler = Scheduler::from_pipeline(&pipeline);
    /// let sweep = scheduler
    ///     .run_sweep_checkpointed(&pipeline, /* resume */ false, &|_, _| {})
    ///     .unwrap();
    /// assert!(!sweep.interrupted);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(sum over points of frames_run)` frame kernels across `parallelism`
    /// workers, plus one atomic checkpoint write per heartbeat round.
    pub fn run_sweep_checkpointed(
        &self,
        pipeline: &Pipeline,
        resume: bool,
        frame_observer: &(dyn Fn(usize, usize) + Sync),
    ) -> Result<CheckpointedSweep, SweepError> {
        let plan = pipeline.run_plan().ok_or_else(|| {
            SweepError::Stage(execution_validation(
                "run_sweep_checkpointed requires a preset-built pipeline carrying a RunPlan"
                    .to_string(),
            ))
        })?;
        let config = pipeline.config();
        let dir = config.checkpoint_dir.clone().ok_or_else(|| {
            SweepError::Stage(execution_validation(
                "run_sweep_checkpointed requires PipelineConfig::checkpoint_dir".to_string(),
            ))
        })?;
        let expected_hash = config_hash(config);
        let writer = CheckpointWriter::new(&dir).map_err(SweepError::Io)?;
        let reader = CheckpointReader::new(dir, expected_hash.clone());

        let RunPlan::Dvbt2 {
            rate,
            modulation,
            decoder,
            demap,
        } = plan;

        let mut per_point = Vec::with_capacity(config.esn0_db_points.len());
        let mut interrupted = false;
        for (snr_idx, &es_n0_db) in config.esn0_db_points.iter().enumerate() {
            let loaded = if resume {
                reader.load(snr_idx).map_err(SweepError::Load)?
            } else {
                None
            };
            let template = DvbT2BicmFrameSim::new(rate, modulation, es_n0_db, decoder, demap);
            let run = self.run_point_checkpointed(
                pipeline,
                &template,
                config,
                snr_idx,
                es_n0_db,
                &writer,
                &expected_hash,
                loaded,
                frame_observer,
            )?;
            per_point.push(SnrPointResult::from_counters(es_n0_db, run.counters));
            if run.interrupted {
                // The interrupted point's resumable checkpoint is already on
                // disk; stop the sweep (mirrors `checkpoint::run_sweep_checkpointed`).
                interrupted = true;
                break;
            }
        }

        Ok(CheckpointedSweep {
            results: SimulationResults { per_point },
            interrupted,
        })
    }

    /// Runs one checkpointed SNR point, routing to the hybrid executor when a
    /// GPU stage is active and to the unchanged CPU `5f12e7ff` runner
    /// otherwise.
    #[allow(clippy::too_many_arguments)]
    fn run_point_checkpointed(
        &self,
        pipeline: &Pipeline,
        template: &DvbT2BicmFrameSim,
        config: &PipelineConfig,
        snr_idx: usize,
        es_n0_db: f64,
        writer: &CheckpointWriter,
        expected_hash: &str,
        resume: Option<CheckpointV2>,
        frame_observer: &(dyn Fn(usize, usize) + Sync),
    ) -> Result<CheckpointedRun, SweepError> {
        #[cfg(feature = "hip")]
        if self.gpu_active() {
            if let Some(gpu_stage) = super::scheduler::find_gpu_ldpc_stage(pipeline) {
                return self.run_point_hybrid_checkpointed(
                    template,
                    gpu_stage,
                    config,
                    snr_idx,
                    es_n0_db,
                    writer,
                    expected_hash,
                    resume,
                    frame_observer,
                );
            }
        }
        #[cfg(not(feature = "hip"))]
        let _ = pipeline; // no device backend: every point routes to the CPU runner

        // CPU arm: the 5f12e7ff checkpointed runner, semantics UNCHANGED
        // (resume via the global `frames_completed`; chunk-restart striding).
        // Runs inside this scheduler's rayon pool so `parallelism` is honoured.
        self.rayon_pool()
            .install(|| {
                run_snr_point_checkpointed(
                    config,
                    snr_idx,
                    es_n0_db,
                    writer,
                    expected_hash,
                    resume,
                    || template.clone(),
                    |g, ctx, sim| {
                        let outcome = sim.simulate_frame(g, ctx);
                        frame_observer(snr_idx, g);
                        outcome
                    },
                    |_, _| {},
                )
            })
            .map_err(SweepError::Io)
    }
}

#[cfg(feature = "hip")]
mod hybrid_checkpoint {
    use super::*;
    use crate::batch::LlrBatch;
    use crate::checkpoint::{build_checkpoint, is_interrupted, loaded_counters};
    use crate::executor::scheduler::BATCH_FRAMES;
    use crate::frame_sim::FramePrep;
    use crate::parallel::{WorkerCounters, WorkerCtx};
    use gf2_kernels_hip::launch_ldpc_bp::LdpcStreamScratch;
    use gf2_kernels_hip::GpuLdpcBp as KernelGpuLdpcBp;
    use rayon::prelude::*;

    /// One GPU decode batch's result: the per-frame hard codewords and BP
    /// iteration counts (or a [`StageError`] on a device fault). Mirrors the
    /// uncheckpointed scheduler's alias.
    type GpuBatchResult = Result<(Vec<gf2_core::BitVec>, Vec<u32>), StageError>;

    /// Per-worker device state persisted across heartbeat rounds within one
    /// SNR point: the worker's own frame-kernel clone (own BCH decode-tail),
    /// its device LDPC decoder sized for one [`BATCH_FRAMES`] batch, and its
    /// pinned staging. All `Send`-only, owned per worker, never shared by `&`
    /// (the HIP host concurrency model).
    struct WorkerGpuState {
        sim: DvbT2BicmFrameSim,
        device: KernelGpuLdpcBp,
        scratch: LdpcStreamScratch,
    }

    impl Scheduler {
        /// The checkpointed hybrid per-SNR-point driver (deliverables 2–3).
        ///
        /// Processes the point in heartbeat **rounds**. A round covers the
        /// aligned global-frame range up to the next multiple of
        /// `R = ceil(heartbeat / (BATCH_FRAMES·W)) · BATCH_FRAMES · W`
        /// (`heartbeat_every_frames = 0` ⇒ one round to `max_frames`), so
        /// every checkpoint boundary lands on whole per-worker batches and the
        /// batch composition matches the uncheckpointed C.1 scheduler exactly
        /// (see the module docs). Within a round each worker runs the C.1
        /// double-buffer (CPU prep of batch `N+1` ∥ stream-ordered GPU decode
        /// of batch `N` on its owned stream), checking
        /// [`is_interrupted`](crate::checkpoint::is_interrupted) at each batch
        /// boundary: on SIGINT the in-flight batch completes and is recorded,
        /// no further batch is enqueued, and the already-prepped next batch is
        /// discarded (its frames were never recorded; resume re-preps them
        /// byte-identically from the global-frame-keyed RNG).
        ///
        /// After every round: worker faults propagate,
        /// [`drain_for_checkpoint`](Scheduler::drain_for_checkpoint) runs
        /// (§4 steps 1–2), the counters and per-worker `frames_in_worker` are
        /// latched **after** the drain, and the v2 checkpoint is written
        /// atomically.
        #[allow(clippy::too_many_arguments)]
        pub(super) fn run_point_hybrid_checkpointed(
            &self,
            template: &DvbT2BicmFrameSim,
            gpu_stage: &crate::gpu::ldpc_bp::GpuLdpcBp,
            config: &PipelineConfig,
            snr_idx: usize,
            es_n0_db: f64,
            writer: &CheckpointWriter,
            expected_hash: &str,
            resume: Option<CheckpointV2>,
            frame_observer: &(dyn Fn(usize, usize) + Sync),
        ) -> Result<CheckpointedRun, SweepError> {
            let num_workers = self.parallelism().get();
            let seed = self.seed();
            let max_frames = config.max_frames as usize;
            let target_errors = config.target_errors;

            // Heartbeat round size in global frames, rounded UP to whole
            // per-worker batches so flush boundaries are batch-aligned.
            let round_frames: usize = if config.heartbeat_every_frames == 0 {
                max_frames.max(1)
            } else {
                let batches_per_worker = (config.heartbeat_every_frames as usize)
                    .div_ceil(BATCH_FRAMES * num_workers)
                    .max(1);
                batches_per_worker * BATCH_FRAMES * num_workers
            };

            // Resume restore (§4 amendment 2026-06-10): fold the saved
            // counters and restore each worker's strided-partition PROGRESS
            // from `frames_in_worker`. `rng_word_pos` is NOT read back —
            // per-frame RNG positions are re-derived from the global index.
            let mut total = WorkerCounters::default();
            let mut done: Vec<u64> = vec![0; num_workers];
            if let Some(ref ck) = resume {
                if ck.completed || ck.frames_completed as usize >= max_frames {
                    return Ok(CheckpointedRun {
                        counters: loaded_counters(ck),
                        completed: true,
                        interrupted: false,
                    });
                }
                total = loaded_counters(ck);
                for ws in &ck.worker_states {
                    if ws.worker_idx < num_workers {
                        done[ws.worker_idx] = ws.frames_in_worker;
                    }
                }
                // Hybrid worker_states are strided-partition prefixes, so the
                // per-worker counts must sum to the global frames_completed.
                // (config_hash includes gpu_enabled, so a CPU-path checkpoint
                // cannot reach here; this guards residual corruption and the
                // degraded-then-GPU edge documented in the module docs.)
                let sum: u64 = done.iter().sum();
                if sum != ck.frames_completed {
                    return Err(SweepError::Load(FatalError::BuildError(
                        BuildError::ExecutionValidation {
                            reason: format!(
                                "hybrid resume: worker_states frames sum {sum} != \
                                 frames_completed {}; the checkpoint was not written by \
                                 the hybrid strided-partition executor",
                                ck.frames_completed
                            ),
                        },
                    )));
                }
            }

            // Per-worker device state, built lazily on each worker's first
            // round and persisted across rounds (decoder + pinned staging are
            // expensive; the C.1 scheduler also builds them once per point).
            let mut states: Vec<Option<WorkerGpuState>> = (0..num_workers).map(|_| None).collect();
            let tally = StreamInFlight::new(num_workers);

            let mut completed = false;
            let mut interrupted = false;

            while (total.frames as usize) < max_frames {
                if is_interrupted() {
                    interrupted = true;
                    break;
                }

                // The next aligned round boundary above the least-advanced
                // worker. Workers ahead of it simply contribute no frames this
                // round (possible after a ragged SIGINT stop).
                let min_next = (0..num_workers)
                    .map(|w| w + done[w] as usize * num_workers)
                    .filter(|&g| g < max_frames)
                    .min();
                let Some(min_next) = min_next else {
                    break; // defensive: every partition exhausted
                };
                let round_end = (((min_next / round_frames) + 1) * round_frames).min(max_frames);

                let per_worker: Vec<Result<(WorkerCounters, u64), StageError>> =
                    self.rayon_pool().install(|| {
                        states
                            .par_iter_mut()
                            .enumerate()
                            .map(|(worker_idx, slot)| {
                                self.worker_round_hybrid(
                                    template,
                                    gpu_stage,
                                    slot,
                                    &tally,
                                    worker_idx,
                                    snr_idx,
                                    done[worker_idx] as usize,
                                    round_end,
                                    seed,
                                    frame_observer,
                                )
                            })
                            .collect()
                    });

                // Propagate a worker fault first (no checkpoint is written for
                // a faulted round), then drain (§4 steps 1-2) BEFORE latching.
                let mut round: Vec<(WorkerCounters, u64)> = Vec::with_capacity(num_workers);
                for r in per_worker {
                    round.push(r.map_err(SweepError::Stage)?);
                }
                self.drain_for_checkpoint(&tally)
                    .map_err(SweepError::Stage)?;

                // §4 step 3: latch AFTER the drain. Reduce the round's counters
                // in worker_idx order (the SSOT order), fold into the total,
                // and advance the authoritative per-worker progress.
                let round_counters: Vec<WorkerCounters> = round.iter().map(|(c, _)| *c).collect();
                total = WorkerCounters::reduce_in_worker_order(&[
                    total,
                    WorkerCounters::reduce_in_worker_order(&round_counters),
                ]);
                for (w, (_, frames_done)) in round.iter().enumerate() {
                    done[w] += frames_done;
                }

                let reached_target = target_errors > 0 && total.errors >= target_errors;
                completed = total.frames as usize >= max_frames || reached_target;

                // §4 step 4: latch worker_states[] from the SSOT and write the
                // v2 JSON atomically (tmp + fsync + rename + dir-fsync).
                let ckpt = build_checkpoint(
                    config,
                    snr_idx,
                    es_n0_db,
                    expected_hash,
                    &total,
                    &done,
                    completed,
                );
                writer.write(&ckpt).map_err(SweepError::Io)?;

                if completed {
                    break;
                }
                if is_interrupted() {
                    // The SIGINT flush above is the resumable checkpoint.
                    interrupted = true;
                    break;
                }
            }

            Ok(CheckpointedRun {
                counters: total,
                completed,
                interrupted,
            })
        }

        /// One worker's slice of one heartbeat round: its strided-partition
        /// frames `< round_end` not yet done, double-buffered CPU prep ∥ GPU
        /// decode on the worker's owned stream (the C.1 overlap protocol), with
        /// the [`StreamInFlight`] tally bracketing every stream-ordered decode
        /// and an [`is_interrupted`] check at each batch boundary.
        ///
        /// Returns the worker's round counters and the number of frames it
        /// completed (a whole number of batches, except the partition tail).
        #[allow(clippy::too_many_arguments)]
        fn worker_round_hybrid(
            &self,
            template: &DvbT2BicmFrameSim,
            gpu_stage: &crate::gpu::ldpc_bp::GpuLdpcBp,
            slot: &mut Option<WorkerGpuState>,
            tally: &StreamInFlight,
            worker_idx: usize,
            snr_idx: usize,
            done_in_worker: usize,
            round_end: usize,
            seed: u64,
            frame_observer: &(dyn Fn(usize, usize) + Sync),
        ) -> Result<(WorkerCounters, u64), StageError> {
            let num_workers = self.parallelism().get();
            // Partition frames with g < round_end: indices 0..part_end where
            // g = worker_idx + j * num_workers.
            let part_end = if round_end > worker_idx {
                (round_end - worker_idx).div_ceil(num_workers)
            } else {
                0
            };
            if done_in_worker >= part_end {
                return Ok((WorkerCounters::default(), 0));
            }

            let (stream_id, stream) = self
                .worker_stream(worker_idx)
                .expect("hybrid checkpointed runner requires an active stream pool");

            // Lazy per-point worker state (first round only).
            if slot.is_none() {
                let sim = template.clone();
                let device = gpu_stage.build_decoder(BATCH_FRAMES)?;
                let scratch = gpu_stage.build_stream_scratch(&device)?;
                *slot = Some(WorkerGpuState {
                    sim,
                    device,
                    scratch,
                });
            }
            let WorkerGpuState {
                sim,
                device,
                scratch,
            } = slot.as_mut().expect("worker state just initialised");

            // The worker's remaining round frames, in partition order. Chunked
            // by BATCH_FRAMES from a batch-aligned start (done_in_worker is a
            // whole number of batches except at the partition tail), so the
            // batch composition matches the uncheckpointed scheduler's.
            let my_frames: Vec<usize> = (done_in_worker..part_end)
                .map(|j| worker_idx + j * num_workers)
                .collect();
            let batches: Vec<&[usize]> = my_frames.chunks(BATCH_FRAMES).collect();

            // CPU-prep one batch: per-frame RNG seek keyed on the GLOBAL frame
            // index (§3 logical worker 0 — the byte-identity rule).
            let prep_batch = |frames: &[usize], ctx: &mut WorkerCtx| -> Vec<FramePrep> {
                frames
                    .iter()
                    .map(|&g| {
                        ctx.reseek_to_frame(g);
                        let prep = sim.prepare_frame(g, ctx);
                        frame_observer(snr_idx, g);
                        prep
                    })
                    .collect()
            };

            let mut counters = WorkerCounters::default();
            let mut frames_done: u64 = 0;
            let mut ctx = WorkerCtx::new(seed, snr_idx, 0);
            let mut prepared = prep_batch(batches[0], &mut ctx);

            for bi in 0..batches.len() {
                let cur_preps = std::mem::take(&mut prepared);
                let next_idx = bi + 1;

                // The C.1 double-buffer: the owned device decoder is `!Sync`,
                // so the GPU-blocking call stays on THIS worker thread while a
                // scoped helper preps batch N+1 (capturing only `Sync` state).
                let (gpu_res, next_preps): (GpuBatchResult, Vec<FramePrep>) =
                    std::thread::scope(|scope| {
                        let cpu_handle = scope.spawn(|| {
                            if next_idx < batches.len() {
                                // Keyed on the global frame index, so a fresh
                                // throwaway ctx preserves byte-identity.
                                let mut next_ctx = WorkerCtx::new(seed, snr_idx, 0);
                                prep_batch(batches[next_idx], &mut next_ctx)
                            } else {
                                Vec::new()
                            }
                        });

                        // Deliverable 1: the tally brackets the stream-ordered
                        // decode. `enqueued` before the launch; `completed`
                        // once the per-stream synchronize inside the decode
                        // call has returned (the batch is off the device). On
                        // a device fault the slot stays non-zero, so a later
                        // drain refuses to commit a checkpoint.
                        tally.enqueued(stream_id);
                        let gpu_res: GpuBatchResult = (|| {
                            let llr_batch =
                                LlrBatch::new(cur_preps.iter().map(|p| p.llrs.clone()).collect());
                            let (hard, iters) = gpu_stage.decode_batch_with_iters_on_stream(
                                &llr_batch, device, stream, scratch,
                            )?;
                            Ok((hard.frames, iters))
                        })();
                        if gpu_res.is_ok() {
                            tally.completed(stream_id);
                        }

                        (gpu_res, cpu_handle.join().expect("CPU prep helper thread"))
                    });

                let (codewords, iters) = gpu_res?;
                prepared = next_preps;

                // CPU BCH decode-tail + error count for the decoded batch.
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
                frames_done += cur_preps.len() as u64;

                // §4 SIGINT at a batch boundary: the in-flight batch above
                // COMPLETED and was recorded; stop before enqueuing another.
                // The already-prepped next batch is discarded — its frames were
                // never recorded, and resume re-preps them byte-identically.
                if is_interrupted() {
                    break;
                }
            }

            Ok((counters, frames_done))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    #[test]
    fn test_stream_in_flight_tally_arithmetic() {
        let tally = StreamInFlight::new(3);
        assert_eq!(tally.streams(), 3);
        assert_eq!(tally.total_in_flight(), 0);
        tally.enqueued(0);
        tally.enqueued(0);
        tally.enqueued(2);
        assert_eq!(tally.in_flight(0), 2);
        assert_eq!(tally.in_flight(1), 0);
        assert_eq!(tally.in_flight(2), 1);
        assert_eq!(tally.total_in_flight(), 3);
        tally.completed(0);
        tally.completed(2);
        assert_eq!(tally.in_flight(0), 1);
        assert_eq!(tally.total_in_flight(), 1);
        tally.completed(0);
        assert_eq!(tally.total_in_flight(), 0);
    }

    #[test]
    #[should_panic(expected = "without a matching enqueued")]
    fn test_stream_in_flight_completed_underflow_panics() {
        let tally = StreamInFlight::new(1);
        tally.completed(0);
    }

    #[test]
    fn test_drain_ok_on_idle_tally_without_gpu() {
        // A CPU-only scheduler has no streams; an idle tally drains cleanly.
        let sched = Scheduler::new(NonZeroUsize::new(2).unwrap(), false, 7);
        let tally = StreamInFlight::new(2);
        assert!(sched.drain_for_checkpoint(&tally).is_ok());
    }

    #[test]
    fn test_drain_refuses_in_flight_batches() {
        // §4 "no partial batches": a non-zero tally after the join is a
        // contract violation and the drain must refuse to commit.
        let sched = Scheduler::new(NonZeroUsize::new(2).unwrap(), false, 7);
        let tally = StreamInFlight::new(2);
        tally.enqueued(1);
        let err = sched
            .drain_for_checkpoint(&tally)
            .expect_err("in-flight batches must fail the drain");
        match err {
            StageError::Fatal(FatalError::BuildError(BuildError::ExecutionValidation {
                reason,
            })) => {
                assert!(
                    reason.contains("stream 1") && reason.contains("in-flight"),
                    "reason must name the offending stream: {reason}"
                );
            }
            other => panic!("expected ExecutionValidation, got {other:?}"),
        }
    }
}
