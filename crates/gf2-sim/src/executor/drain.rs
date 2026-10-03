//! GPU drain-for-checkpoint and the checkpointed hybrid CPU+GPU sweep.
//!
//! * [`StreamInFlight`] — the per-stream "in-flight batches" tally, so the
//!   drain knows when every stream is idle.
//! * [`Scheduler::drain_for_checkpoint`] — per-stream `hipStreamSynchronize()`
//!   on each **owned** stream (never `hipDeviceSynchronize()`, which would
//!   block unrelated contexts), then a tally check enforcing the
//!   no-partial-batches commit contract.
//! * [`Scheduler::run_sweep_checkpointed`] — the checkpointed SNR sweep over
//!   the hybrid executor or, without a GPU, the CPU runner;
//!   [`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed) is the
//!   convenience entry point.
//!
//! # Failure semantics
//!
//! A recoverable GPU fault during a checkpointed sweep is **propagated**
//! ([`SweepError::Stage`](crate::snr_checkpoint::SweepError::Stage)), aborting
//! the sweep resumably, where the uncheckpointed scheduler substitutes the CPU
//! fallback. Substituting would record a different `mean_iters`/decode for the
//! faulted frames, breaking the same-path resume byte-identity of all four
//! columns (`mean_iters` included). The fault leaves the last committed
//! heartbeat checkpoint intact, and a subsequent resume continues from it
//! byte-identically; regression-guarded by
//! `tests/hybrid_resume.rs::hybrid_checkpointed_recoverable_fault_aborts_resumably`.
//!
//! # Drain commit contract
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
//!    [`CheckpointWriter`](crate::snr_checkpoint::CheckpointWriter).
//!
//! # Batch alignment
//!
//! Heartbeat rounds are sized in whole per-worker batches of `BATCH_FRAMES`,
//! and a SIGINT stops workers at a **batch boundary**. Every batch the
//! checkpointed runner launches is therefore the same `chunks(BATCH_FRAMES)`
//! slice of the worker's partition that the uncheckpointed scheduler launches
//! — across interrupts and resumes — so the per-frame GPU decode results (hard
//! codewords **and** BP iteration counts) match it batch for batch.
//!
//! # Cross-path resume
//!
//! `gpu_enabled` is part of [`config_hash`](crate::snr_checkpoint::config_hash),
//! so a CPU-written checkpoint is rejected by a hybrid resume and vice versa:
//! the two executors record differently shaped `worker_states[]` and
//! path-specific `total_iterations`. A `gpu_enabled` config that degraded to
//! the CPU path (no device) and is resumed where a device exists is guarded by
//! the per-worker batch-alignment check in `run_point_hybrid_checkpointed`,
//! which rejects a `frames_in_worker` that is neither `BATCH_FRAMES`-aligned
//! nor the partition end with a typed `ExecutionValidation` error.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::PipelineConfig;
use crate::error::{BuildError, FatalError, StageError};
use crate::executor::results::{SimulationResults, SnrPointResult};
use crate::executor::{RunPlan, Scheduler};
use crate::frame_sim::DvbT2BicmFrameSim;
use crate::pipeline::Pipeline;
use crate::snr_checkpoint::{
    config_hash, run_snr_point_checkpointed, CheckpointReader, CheckpointV2, CheckpointWriter,
    CheckpointedRun, SweepError,
};

/// Per-stream "in-flight GPU batches" tally.
///
/// Each hybrid worker increments its owned stream's slot immediately before
/// enqueuing a batch's stream-ordered GPU work and decrements it once the
/// per-stream synchronize inside the decode call has returned (the batch is
/// off the device). [`Scheduler::drain_for_checkpoint`] consults the tally to
/// know every stream is idle before the checkpoint is written: a non-zero
/// count after the worker join means a worker abandoned a batch mid-flight (a
/// fault path), and the drain refuses to commit (no partial batches).
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
/// aggregate. If the point completed at least one heartbeat round before the
/// interrupt, its latest heartbeat checkpoint is on disk and resume continues
/// from it; if the interrupt landed before the first round committed, no
/// checkpoint was written and resume restarts the point fresh. In either case,
/// resume with
/// [`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed)`(true)`.
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
    /// Drains the GPU for a checkpoint flush: synchronizes each **owned** HIP
    /// stream per-stream and verifies the [`StreamInFlight`] tally shows every
    /// stream idle.
    ///
    /// Synchronization is per-stream (`HipStream::synchronize`, i.e.
    /// `hipStreamSynchronize()`), never `hipDeviceSynchronize()`, which would
    /// block unrelated contexts. The owned streams are the ones the hybrid
    /// workers select by fixed index (`worker_idx % n_streams`); each is
    /// synchronized exactly once. On a CPU-only scheduler (or a build without
    /// the `hip` feature) only the tally check runs.
    ///
    /// Call **after** the worker join and **before** latching `worker_states[]`
    /// / writing the checkpoint, so the recorded per-worker progress reflects
    /// only fully completed batches.
    ///
    /// # Errors
    ///
    /// * A mapped [`StageError`] if a per-stream synchronize faults
    ///   (via the crate's `gpu::map_hip_error`).
    /// * [`FatalError::BuildError`]`(`[`BuildError::ExecutionValidation`]`)` if
    ///   any stream still shows in-flight batches after the join: a worker
    ///   abandoned a batch mid-flight, and the checkpoint must not be written.
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
        // Each owned stream exactly once (worker_idx % n_streams can map
        // several workers to one stream).
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

        // After the worker join every enqueued batch must have completed. A
        // non-zero tally cannot recover here (no worker is running), so refuse
        // to commit rather than record partial progress.
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
    /// SIGINT checkpointing and resume support.
    ///
    /// Unlike [`Pipeline::run`](crate::Pipeline::run), a recoverable GPU fault
    /// aborts the checkpointed sweep (resumably, returning a
    /// [`SweepError::Stage`]) instead of substituting the CPU fallback, for
    /// checkpoint byte-identity determinism. The last committed heartbeat
    /// checkpoint survives the abort, so a subsequent resume continues
    /// byte-identically.
    ///
    /// Per SNR point: when this scheduler is GPU-active and the pipeline
    /// carries a `GpuOnly` LDPC stage, the point runs on the **checkpointed
    /// hybrid executor** (strided partitions, double-buffered CPU prep ∥ GPU
    /// decode, per-stream drain before every flush); otherwise it runs on the
    /// CPU runner
    /// [`run_snr_point_checkpointed`](crate::snr_checkpoint::run_snr_point_checkpointed)
    /// (resume via the global `frames_completed`).
    ///
    /// With `resume`, each point's `<checkpoint_dir>/snr_<NNNN>.json` is loaded
    /// first: completed points fold their saved counters and are skipped; a
    /// partial point resumes — on the hybrid path by restoring each worker's
    /// strided-partition progress from `worker_states[].frames_in_worker`
    /// (worker `w` of `W` continues at global frame `w + frames_in_worker·W`;
    /// `rng_word_pos` is never read back). The
    /// resumed aggregate is byte-identical to an uninterrupted run at the same
    /// seed and worker count because every frame's outcome is a pure function
    /// of its global frame index.
    ///
    /// On SIGINT (or [`request_interrupt`](crate::snr_checkpoint::request_interrupt))
    /// the in-flight GPU batches complete, the streams are drained, a resumable
    /// checkpoint is flushed, and the sweep returns with `interrupted = true`.
    ///
    /// As each SNR point completes in this process, one `snr_point_completed`
    /// `tracing` event is emitted at the point boundary (carrying `es_n0_db`,
    /// `fer`, `ber`, `frames`, `errors`, `mean_iters`, `wall_seconds`).
    ///
    /// # Arguments
    ///
    /// * `pipeline` — the built pipeline; must carry a
    ///   [`RunPlan`](crate::executor::RunPlan) (preset-built) and a
    ///   `checkpoint_dir` in its config.
    /// * `frame_observer` — called once per frame as `(snr_idx, global_frame)`
    ///   while the point is simulating (hybrid path: after the frame's CPU
    ///   prep, from either the main worker thread or the double-buffer helper
    ///   thread; CPU path: after the frame completes). Campaigns can emit
    ///   progress from it; tests use it to land a deterministic
    ///   [`request_interrupt`](crate::snr_checkpoint::request_interrupt)
    ///   mid-flight. Pass `&|_, _| {}` if unused.
    ///   **Caveat (hybrid path):** frames prepped into the discarded next batch
    ///   at an interrupt point are *observed* by this callback but are not
    ///   recorded; they are re-observed (and recorded) on the subsequent resume
    ///   run. The observer may therefore fire for the same `global_frame` twice
    ///   across an interrupted + resumed run pair.
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
            // A point already COMPLETE in the loaded checkpoint is folded and
            // skipped with `completed = true`; it must not be logged again
            // below. Captured before `loaded` is moved.
            let was_loaded_complete = loaded.as_ref().is_some_and(|c| c.completed);
            let template = DvbT2BicmFrameSim::new(rate, modulation, es_n0_db, decoder, demap);
            let point_start = std::time::Instant::now();
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
            let point_wall_seconds = point_start.elapsed().as_secs_f64();
            let point = SnrPointResult::from_counters(es_n0_db, run.counters);
            // Emit `snr_point_completed` only for a point that completed in
            // THIS process: an interrupted point did not finish, and a point
            // already complete in the loaded checkpoint was logged by the
            // process that completed it. `wall_seconds` is this point's
            // measured wall time.
            let ber = if point.total_bits > 0 {
                point.total_bit_errors as f64 / point.total_bits as f64
            } else {
                0.0
            };
            if run.completed && !run.interrupted && !was_loaded_complete {
                tracing::info!(
                    name: "snr_point_completed",
                    event_type = "snr_point_completed",
                    es_n0_db = point.es_n0_db,
                    fer = point.fer,
                    ber,
                    frames = point.frames,
                    errors = point.errors,
                    mean_iters = point.mean_iters,
                    wall_seconds = point_wall_seconds,
                );
            }
            per_point.push(point);
            if run.interrupted {
                // Stop the sweep at the interrupted point.
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
    /// GPU stage is active and to the CPU runner otherwise.
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

        // CPU arm (resume via the global `frames_completed`), run inside this
        // scheduler's rayon pool so `parallelism` is honoured.
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
    use std::time::Instant;

    use super::*;
    use crate::batch::LlrBatch;
    use crate::executor::hybrid_core::{
        run_hybrid_double_buffer, BatchHooks, GpuBatchResult, HybridRunCtx, WorkerDevice,
        BATCH_FRAMES,
    };
    use crate::frame_sim::FramePrep;
    use crate::parallel::WorkerCounters;
    use crate::snr_checkpoint::{build_checkpoint, is_interrupted, loaded_counters};
    use gf2_kernels_hip::launch_ldpc_bp::LdpcStreamScratch;
    use gf2_kernels_hip::GpuLdpcBp as KernelGpuLdpcBp;
    use rayon::prelude::*;

    /// Validates that every worker's `done[w]` from a resumed checkpoint is
    /// `BATCH_FRAMES`-aligned or at the complete partition end.
    ///
    /// A misaligned count means the checkpoint was written by the CPU executor
    /// (which records per-chunk progress, not batch-aligned progress) —
    /// resuming it on the hybrid path would start `worker_round_hybrid`
    /// mid-batch, making the batch composition diverge from the uncheckpointed
    /// scheduler and silently breaking `mean_iters` byte-identity.
    ///
    /// Returns the first violation found, naming the worker, its `done` value,
    /// and the expected alignment.
    pub(super) fn validate_batch_alignment(
        done: &[u64],
        num_workers: usize,
        max_frames: usize,
    ) -> Result<(), SweepError> {
        for (w, &d) in done.iter().enumerate().take(num_workers) {
            if d == 0 {
                continue; // not started yet — always valid
            }
            let partition_end = if max_frames > w {
                (max_frames - w).div_ceil(num_workers)
            } else {
                0
            } as u64;
            let batch_aligned = d.is_multiple_of(BATCH_FRAMES as u64);
            let partition_complete = d == partition_end;
            if !batch_aligned && !partition_complete {
                return Err(SweepError::Load(FatalError::BuildError(
                    BuildError::ExecutionValidation {
                        reason: format!(
                            "hybrid resume: worker {w} done={d} is not \
                             BATCH_FRAMES({BATCH_FRAMES})-aligned and is not the \
                             partition end ({partition_end}); the checkpoint was \
                             written by a non-hybrid executor (degraded-then-GPU \
                             or corrupted)"
                        ),
                    },
                )));
            }
        }
        Ok(())
    }

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

    /// The checkpointed drain loop's per-batch decode dispatch: it brackets
    /// the stream-ordered GPU decode with the
    /// [`StreamInFlight`](crate::executor::StreamInFlight) tally and
    /// **propagates** a recoverable GPU fault unchanged, where the
    /// uncheckpointed scheduler's `dispatch_with_fallback` substitutes the CPU
    /// fallback (see the module docs for the reason).
    struct DrainBatchHooks<'a> {
        gpu_stage: &'a crate::gpu::ldpc_bp::GpuLdpcBp,
        tally: &'a crate::executor::StreamInFlight,
        stream_id: usize,
        /// **Test-only** GPU-OOM injection modulus (mirrors
        /// [`PipelineConfig::inject_gpu_oom_modulus`](crate::PipelineConfig::inject_gpu_oom_modulus)),
        /// keyed on the batch's first global frame index exactly as the
        /// uncheckpointed scheduler keys it. The injected recoverable OOM is
        /// **propagated**, driving the production fault path a genuine device
        /// OOM would.
        inject_oom_modulus: Option<u64>,
    }

    impl BatchHooks for DrainBatchHooks<'_> {
        fn decode_batch(
            &mut self,
            device: &mut WorkerDevice<'_>,
            stream: &gf2_kernels_hip::host::HipStream,
            _batch_idx: usize,
            first_global_frame: u64,
            preps: &[FramePrep],
        ) -> GpuBatchResult {
            use crate::executor::failure::injects_oom_at;

            // The tally brackets the stream-ordered decode:
            // `enqueued` before the launch; `completed` once the per-stream
            // synchronize inside the decode call has returned (the batch is off
            // the device). On a device fault the slot stays non-zero, so a later
            // drain refuses to commit a checkpoint.
            self.tally.enqueued(self.stream_id);
            let gpu_res: GpuBatchResult =
                if injects_oom_at(self.inject_oom_modulus, first_global_frame) {
                    // Test-only OOM injection, keyed on the batch's FIRST global
                    // frame index and propagated as a genuine device OOM would be.
                    Err(StageError::Recoverable(
                        crate::error::RecoverableError::OutOfMemory {
                            device_id: 0,
                            bytes_requested: 1024 * 1024 * 1024,
                        },
                    ))
                } else {
                    (|| {
                        let llr_batch =
                            LlrBatch::new(preps.iter().map(|p| p.llrs.clone()).collect());
                        let (hard, iters) = self.gpu_stage.decode_batch_with_iters_on_stream(
                            &llr_batch,
                            device.device,
                            stream,
                            device.scratch,
                        )?;
                        Ok((hard.frames, iters))
                    })()
                };
            if gpu_res.is_ok() {
                self.tally.completed(self.stream_id);
            }
            gpu_res
        }

        fn stop_after_batch(&self, _batch_idx: usize) -> bool {
            // SIGINT at a batch boundary: the in-flight batch COMPLETED and
            // was recorded; stop before enqueuing another. The already-prepped
            // next batch is discarded — its frames were never recorded, and
            // resume re-preps them byte-identically from the global-frame-keyed
            // RNG.
            is_interrupted()
        }
    }

    impl Scheduler {
        /// The checkpointed hybrid per-SNR-point driver.
        ///
        /// Processes the point in heartbeat **rounds**. A round covers the
        /// aligned global-frame range up to the next multiple of
        /// `R = ceil(heartbeat / (BATCH_FRAMES·W)) · BATCH_FRAMES · W`
        /// (`heartbeat_every_frames = 0` ⇒ one round to `max_frames`), so
        /// every checkpoint boundary lands on whole per-worker batches and the
        /// batch composition matches the uncheckpointed scheduler exactly
        /// (see the module docs). Within a round each worker runs the
        /// double-buffer (CPU prep of batch `N+1` ∥ stream-ordered GPU decode
        /// of batch `N` on its owned stream), checking
        /// [`is_interrupted`](crate::snr_checkpoint::is_interrupted) at each batch
        /// boundary: on SIGINT the in-flight batch completes and is recorded,
        /// no further batch is enqueued, and the already-prepped next batch is
        /// discarded (its frames were never recorded; resume re-preps them
        /// byte-identically from the global-frame-keyed RNG).
        ///
        /// After every round: worker faults propagate,
        /// [`drain_for_checkpoint`](Scheduler::drain_for_checkpoint) runs,
        /// the counters and per-worker `frames_in_worker` are
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

            // Resume restore: fold the saved counters and restore each
            // worker's strided-partition PROGRESS from `frames_in_worker`.
            // `rng_word_pos` is NOT read back — per-frame RNG positions are
            // re-derived from the global index.
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
                // Every worker's progress must be BATCH_FRAMES-aligned (or at the
                // complete partition end). See `validate_batch_alignment` for why.
                validate_batch_alignment(&done, num_workers, max_frames)?;
            }

            // Per-worker device state, built lazily on each worker's first
            // round and persisted across rounds.
            let mut states: Vec<Option<WorkerGpuState>> = (0..num_workers).map(|_| None).collect();
            let tally = StreamInFlight::new(num_workers);

            // The checkpointed sweep never reads `OverlapTimeline` intervals,
            // so it passes NO interval sink (`timeline: None` below) — a dead
            // sink would accumulate unboundedly over a long campaign point.
            let run_start = Instant::now();

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
                                    config.inject_gpu_oom_modulus,
                                    run_start,
                                    frame_observer,
                                )
                            })
                            .collect()
                    });

                // Propagate a worker fault first (no checkpoint is written for
                // a faulted round), then drain BEFORE latching.
                let mut round: Vec<(WorkerCounters, u64)> = Vec::with_capacity(num_workers);
                for r in per_worker {
                    round.push(r.map_err(SweepError::Stage)?);
                }
                self.drain_for_checkpoint(&tally)
                    .map_err(SweepError::Stage)?;

                // Latch AFTER the drain. Reduce the round's counters
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

                // Latch worker_states[] from the SSOT and write the
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
        /// decode on the worker's owned stream.
        ///
        /// A thin wrapper over
        /// [`run_hybrid_double_buffer`](crate::executor::hybrid_core::run_hybrid_double_buffer):
        /// it computes the worker's remaining round partition, lazily builds
        /// the persisted per-worker device state, supplies
        /// [`DrainBatchHooks`], and wires the campaign `frame_observer`
        /// through the core's per-frame observation hook.
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
            inject_oom_modulus: Option<u64>,
            run_start: Instant,
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

            let mut worker_device = WorkerDevice { device, scratch };
            let run_ctx = HybridRunCtx {
                worker_idx,
                stream_id,
                snr_idx,
                seed,
                timeline: None,
                run_start,
            };
            let mut hooks = DrainBatchHooks {
                gpu_stage,
                tally,
                stream_id,
                inject_oom_modulus,
            };

            // The campaign observer is `(snr_idx, global_frame)`; the core's
            // per-frame hook is keyed on the global frame only, so bind snr_idx.
            run_hybrid_double_buffer(
                sim,
                &mut worker_device,
                stream,
                &run_ctx,
                &my_frames,
                &mut hooks,
                &|g| frame_observer(snr_idx, g),
            )
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
        // A non-zero tally after the join is a contract violation and the
        // drain must refuse to commit.
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

    /// A misaligned `done[w]` (from a CPU-written checkpoint) is rejected with
    /// a typed `ExecutionValidation` error naming the offending worker;
    /// batch-aligned and partition-complete values are accepted.
    #[cfg(feature = "hip")]
    #[test]
    fn test_validate_batch_alignment_rejects_misaligned_worker() {
        use super::hybrid_checkpoint::validate_batch_alignment;
        use crate::executor::hybrid_core::BATCH_FRAMES;

        // BATCH_FRAMES = 16. A 2-worker, 34-frame run has:
        //   worker 0 partition: frames {0, 2, 4, …, 32} = 17 frames (indices 0..=16)
        //   worker 1 partition: frames {1, 3, 5, …, 33} = 17 frames (indices 0..=16)
        // Valid states: done=0 (not started), done=16 (one batch), done=17 (partition end).
        // Invalid: done=5 (neither 0, nor BATCH_FRAMES-aligned, nor partition-complete).

        assert!(
            validate_batch_alignment(&[0, 0], 2, 34).is_ok(),
            "all-zero is valid"
        );

        assert!(
            validate_batch_alignment(&[BATCH_FRAMES as u64, BATCH_FRAMES as u64], 2, 34).is_ok(),
            "batch-aligned is valid"
        );

        assert!(
            validate_batch_alignment(&[17, 17], 2, 34).is_ok(),
            "partition-complete tail is valid"
        );

        // Worker 0 misaligned: CPU wrote 5 frames (not a valid hybrid stop).
        let err = validate_batch_alignment(&[5, 0], 2, 34)
            .expect_err("misaligned done[0]=5 must be rejected");
        match err {
            crate::snr_checkpoint::SweepError::Load(FatalError::BuildError(
                BuildError::ExecutionValidation { reason },
            )) => {
                assert!(
                    reason.contains("worker 0") && reason.contains("done=5"),
                    "reason must name offending worker and value: {reason}"
                );
                assert!(
                    reason.contains(&BATCH_FRAMES.to_string()),
                    "reason must mention BATCH_FRAMES: {reason}"
                );
            }
            other => panic!("expected ExecutionValidation, got {other:?}"),
        }

        // Worker 1 misaligned, worker 0 fine.
        let err = validate_batch_alignment(&[BATCH_FRAMES as u64, 7], 2, 34)
            .expect_err("misaligned done[1]=7 must be rejected");
        match err {
            crate::snr_checkpoint::SweepError::Load(FatalError::BuildError(
                BuildError::ExecutionValidation { reason },
            )) => {
                assert!(
                    reason.contains("worker 1"),
                    "reason must name offending worker: {reason}"
                );
            }
            other => panic!("expected ExecutionValidation, got {other:?}"),
        }
    }

    // ── run_sweep_checkpointed paths ────────────────────────────────────────

    /// Helper: unique temp dir for a checkpointed sweep test.
    fn sweep_tmp(label: &str) -> gf2_core::test_scratch::Scratch {
        gf2_core::test_scratch::scratch(&format!("gf2-drain-sweep-{label}"))
    }

    /// A pipeline without a `RunPlan` (built via `from_parts`, not a preset)
    /// must be rejected with an `ExecutionValidation` error.
    #[test]
    fn test_run_sweep_checkpointed_rejects_missing_run_plan() {
        let scratch = gf2_core::test_scratch::scratch("gf2-drain-no-plan");
        let config = PipelineConfig {
            seed: 0,
            esn0_db_points: vec![5.0],
            target_errors: 0,
            max_frames: 1,
            heartbeat_every_frames: 0,
            checkpoint_dir: Some(scratch.path().to_path_buf()),
            tracing_log_path: None,
            parallelism: NonZeroUsize::new(1).unwrap(),
            gpu_enabled: false,
            strict_gpu: false,
            diagnostic_dump_dir: None,
            inject_gpu_oom_modulus: None,
        };
        let pipeline =
            Pipeline::from_parts(vec![], vec![], std::collections::HashMap::new(), config);
        let sched = Scheduler::new(NonZeroUsize::new(1).unwrap(), false, 0);
        let err = sched
            .run_sweep_checkpointed(&pipeline, false, &|_, _| {})
            .expect_err("no RunPlan must be an error");
        match err {
            SweepError::Stage(StageError::Fatal(FatalError::BuildError(
                BuildError::ExecutionValidation { reason },
            ))) => {
                assert!(
                    reason.contains("RunPlan"),
                    "error reason must mention RunPlan: {reason}"
                );
            }
            other => panic!("expected SweepError::Stage(ExecutionValidation), got {other:?}"),
        }
    }

    /// A pipeline WITH a `RunPlan` but WITHOUT a `checkpoint_dir` must be
    /// rejected.
    #[test]
    fn test_run_sweep_checkpointed_rejects_missing_checkpoint_dir() {
        use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
        use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
        use gf2_coding::modem::DemapMethod;
        use gf2_coding::CodeRate;

        let mut pipeline = Pipeline::dvb_t2()
            .modcod(crate::presets::dvb_t2::Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
            .demap(DemapMethod::MaxLog)
            .channel(crate::presets::dvb_t2::Channel::awgn(9.0_f32))
            .parallelism(NonZeroUsize::new(1).unwrap())
            .seed(42)
            // No .checkpoint_dir() → stays None
            .build()
            .expect("pipeline builds");
        pipeline.config_mut().esn0_db_points = vec![9.0];
        pipeline.config_mut().max_frames = 1;

        let sched = Scheduler::from_pipeline(&pipeline);
        let err = sched
            .run_sweep_checkpointed(&pipeline, false, &|_, _| {})
            .expect_err("missing checkpoint_dir must be an error");
        match err {
            SweepError::Stage(StageError::Fatal(FatalError::BuildError(
                BuildError::ExecutionValidation { reason },
            ))) => {
                assert!(
                    reason.contains("checkpoint_dir"),
                    "error reason must mention checkpoint_dir: {reason}"
                );
            }
            other => panic!("expected SweepError::Stage(ExecutionValidation), got {other:?}"),
        }
    }

    /// A properly configured pipeline with no `esn0_db_points` must complete
    /// immediately, returning an empty sweep with `interrupted = false`.
    #[test]
    fn test_run_sweep_checkpointed_empty_points_returns_immediately() {
        use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
        use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
        use gf2_coding::modem::DemapMethod;
        use gf2_coding::CodeRate;

        let tmp = sweep_tmp("empty");

        let mut pipeline = Pipeline::dvb_t2()
            .modcod(crate::presets::dvb_t2::Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
            .demap(DemapMethod::MaxLog)
            .channel(crate::presets::dvb_t2::Channel::awgn(9.0_f32))
            .parallelism(NonZeroUsize::new(1).unwrap())
            .seed(0)
            .checkpoint_dir(Some(tmp.path().to_path_buf()))
            .build()
            .expect("pipeline builds");
        pipeline.config_mut().esn0_db_points = vec![]; // no points: loop doesn't run
        pipeline.config_mut().max_frames = 1;

        let sched = Scheduler::from_pipeline(&pipeline);
        let sweep = sched
            .run_sweep_checkpointed(&pipeline, false, &|_, _| {})
            .expect("empty-points sweep must succeed");
        assert!(!sweep.interrupted, "no points → not interrupted");
        assert_eq!(
            sweep.results.per_point.len(),
            0,
            "no points → empty per_point"
        );
    }

    /// A checkpointed CPU sweep over one SNR point with one frame must
    /// complete successfully.
    ///
    /// Uses `gpu_enabled = false` so the CPU arm runs on both HIP and non-HIP
    /// builds.
    #[test]
    fn test_run_sweep_checkpointed_cpu_single_frame_completes() {
        use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
        use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
        use gf2_coding::modem::DemapMethod;
        use gf2_coding::CodeRate;

        let tmp = sweep_tmp("single");

        let mut pipeline = Pipeline::dvb_t2()
            .modcod(crate::presets::dvb_t2::Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
            .demap(DemapMethod::MaxLog)
            // 9 dB: well above the r1/2 16-QAM waterfall — fast decode.
            .channel(crate::presets::dvb_t2::Channel::awgn(9.0_f32))
            .parallelism(NonZeroUsize::new(1).unwrap())
            .seed(0xDEAD_BEEF_u64)
            .checkpoint_dir(Some(tmp.path().to_path_buf()))
            .build()
            .expect("pipeline builds");
        pipeline.config_mut().esn0_db_points = vec![9.0];
        pipeline.config_mut().max_frames = 1;
        pipeline.config_mut().target_errors = 0;
        pipeline.config_mut().heartbeat_every_frames = 0;

        let sched = Scheduler::from_pipeline(&pipeline);
        let frames_observed = std::sync::atomic::AtomicU64::new(0);
        let sweep = sched
            .run_sweep_checkpointed(&pipeline, false, &|_snr, _g| {
                frames_observed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
            .expect("single-frame checkpointed sweep must succeed");

        assert!(!sweep.interrupted, "not interrupted");
        assert_eq!(sweep.results.per_point.len(), 1, "one SNR point");
        assert_eq!(
            sweep.results.per_point[0].frames, 1,
            "exactly one frame simulated"
        );
        assert_eq!(
            frames_observed.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "frame observer must fire once"
        );
    }
}
