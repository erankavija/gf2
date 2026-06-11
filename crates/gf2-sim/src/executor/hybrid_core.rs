//! The shared hybrid CPU/GPU double-buffer core (epic task `bb11c2e6`).
//!
//! Both the uncheckpointed C.1 scheduler loop
//! ([`Scheduler::run`](crate::Scheduler::run) → `scheduler.rs`) and the
//! checkpointed `571c11c4` drain loop
//! ([`run_sweep_checkpointed`](crate::Scheduler::run_sweep_checkpointed) →
//! `drain.rs`) drive the identical design-doc §6 overlap protocol: each worker
//! owns one HIP stream and double-buffers the CPU preparation of batch `N+1`
//! against the stream-ordered GPU LDPC decode of batch `N`. Before `bb11c2e6`
//! that protocol was transcribed twice; this module factors it into ONE
//! generic core ([`run_hybrid_double_buffer`]) parameterized by the two
//! per-batch behaviours that genuinely differ between the callers:
//!
//! 1. **decode dispatch** ([`BatchHooks::decode_batch`]) — the scheduler wraps
//!    the GPU decode in [`dispatch_with_fallback`](crate::executor::failure::dispatch_with_fallback)
//!    (OOM → CPU fallback unless `strict_gpu`) plus the test-only OOM
//!    injection; the checkpointed drain loop brackets the decode with the
//!    [`StreamInFlight`](crate::executor::StreamInFlight) tally and
//!    **propagates** a recoverable fault instead of substituting the fallback
//!    (the deliberate failure-semantics divergence — see below);
//! 2. **stop-after-batch** ([`BatchHooks::stop_after_batch`]) — the scheduler
//!    runs every batch; the checkpointed loop checks
//!    [`is_interrupted`](crate::checkpoint::is_interrupted) at each batch
//!    boundary so a SIGINT stops at a batch-aligned point.
//!
//! The **instrumentation** (the `tracing` `pipeline_stage` spans + the
//! [`OverlapTimeline`](crate::executor::OverlapTimeline) intervals) now lives
//! in the core too, so BOTH callers emit identical spans — closing the second
//! `571c11c4` "intentional divergence" (deliverable 3): the checkpointed loop
//! gains span parity for free by sharing this core.
//!
//! # Failure-semantics divergence is now an explicit hook, not an omission
//!
//! The two callers must differ on ONE axis — what a recoverable GPU fault does:
//!
//! * the uncheckpointed scheduler **substitutes the CPU LDPC fallback** (a
//!   mixed CPU+GPU run is byte-identical on the §11 three-column verdict, so a
//!   transparent fallback is correct there);
//! * the checkpointed drain loop **propagates the fault** so the sweep aborts
//!   *resumably* (a CPU-substituted frame would record a different
//!   `mean_iters`/RNG draw than the GPU path, and resume restores from a
//!   checkpoint whose byte-identity contract — §11 four-column,
//!   `mean_iters` INCLUDED on the same-path resume comparison — would then
//!   silently break).
//!
//! That difference is realised entirely **inside each caller's `decode_batch`
//! hook**: the core never makes a fallback-vs-propagate decision itself. The
//! checkpointed caller's hook is the explicit policy decision (epic task
//! `bb11c2e6`, OPTION (a) — keep abort-resumably) that `571c11c4` documented as
//! a divergence and deferred. See `executor/drain.rs` for the hook site and
//! [`run_sweep_checkpointed`](crate::Scheduler::run_sweep_checkpointed) /
//! [`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed) for the
//! public contract.
//!
//! (Compiled only under `feature = "hip"` — the `mod` declaration in
//! `executor/mod.rs` carries the `#[cfg]`.)

use std::time::Instant;

use gf2_kernels_hip::host::HipStream;

use crate::executor::scheduler::{ActivityKind, OverlapTimeline};
use crate::frame_sim::{DvbT2BicmFrameSim, FramePrep};
use crate::parallel::{WorkerCounters, WorkerCtx};

/// One GPU decode batch's result: the per-frame hard codewords and BP iteration
/// counts (or a [`StageError`](crate::error::StageError) on a device fault).
/// The single SSOT alias both hybrid callers share (it was duplicated in
/// `scheduler.rs` and `drain.rs` before `bb11c2e6`).
pub(crate) type GpuBatchResult =
    Result<(Vec<gf2_core::BitVec>, Vec<u32>), crate::error::StageError>;

/// The per-worker GPU decode state [`run_hybrid_double_buffer`] mutates per
/// batch: the device LDPC decoder sized for one batch and the pinned stream
/// scratch. Held separately from the read-only `sim` clone so the
/// double-buffer's CPU-prep helper thread can borrow `&sim` while the worker
/// thread borrows `&mut` this device state — the two never alias.
///
/// `Send`-only, owned per worker, never shared by `&` (the HIP host
/// concurrency model). The checkpointed loop persists the bundle across
/// heartbeat rounds; the uncheckpointed loop builds it once per point.
pub(crate) struct WorkerDevice<'a> {
    /// The per-worker device LDPC decoder, sized for one `BATCH_FRAMES` batch.
    pub(crate) device: &'a gf2_kernels_hip::GpuLdpcBp,
    /// The per-worker pinned host staging the stream-ordered transfers use.
    pub(crate) scratch: &'a mut gf2_kernels_hip::launch_ldpc_bp::LdpcStreamScratch,
}

/// The fixed identifiers + sinks the core threads through every instrumented
/// interval for one worker's run.
///
/// Bundled so [`run_hybrid_double_buffer`]'s signature stays readable and the
/// two callers populate exactly the same context.
pub(crate) struct HybridRunCtx<'a> {
    /// The rayon worker index (appears in spans + intervals).
    pub(crate) worker_idx: usize,
    /// The HIP stream id the worker owns (`worker_idx % n_streams`).
    pub(crate) stream_id: usize,
    /// The SNR-point index keying the §3 RNG seek.
    pub(crate) snr_idx: usize,
    /// The base ChaCha20 seed (design doc §3).
    pub(crate) seed: u64,
    /// The overlap-attestation timeline sink. `Some` on the uncheckpointed
    /// scheduler (its overlap criterion reads the intervals); `None` on the
    /// checkpointed drain path, which never reads intervals — the
    /// `pipeline_stage` spans (always emitted) are its observable parity.
    /// A `None` sink avoids unbounded dead interval accumulation + lock
    /// traffic over a long checkpointed campaign point.
    pub(crate) timeline: Option<&'a std::sync::Mutex<OverlapTimeline>>,
    /// The run-start instant the interval microsecond stamps are relative to.
    pub(crate) run_start: Instant,
}

/// The per-batch behaviours that differ between the hybrid callers (`bb11c2e6`
/// deliverable 1). Everything else — the double-buffer skeleton, the per-frame
/// RNG seek, the BCH decode-tail, the instrumentation — is the shared core's,
/// identical across callers.
///
/// Implemented as a trait taken by `&mut` generic so the per-batch dispatch is
/// **monomorphized** (no `dyn` in the hot loop), preserving the C.1 hybrid
/// throughput.
pub(crate) trait BatchHooks {
    /// Decodes one already-CPU-prepped batch on the GPU and returns its
    /// per-frame hard codewords + BP iteration counts.
    ///
    /// This is the SOLE failure-semantics decision point. The scheduler's impl
    /// wraps [`dispatch_with_fallback`](crate::executor::failure::dispatch_with_fallback)
    /// (substitute the CPU fallback on a recoverable fault, unless
    /// `strict_gpu`); the checkpointed drain loop's impl brackets the decode
    /// with the [`StreamInFlight`](crate::executor::StreamInFlight) tally and
    /// **propagates** a recoverable fault unchanged (abort-resumably).
    ///
    /// * `device` — the worker's borrowed device decoder + scratch.
    /// * `stream` — the worker's owned HIP stream.
    /// * `batch_idx` — the worker-local batch index (0-based).
    /// * `first_global_frame` — the batch's first global frame index (the
    ///   OOM-injection keying surface, and the `FaultContext::batch_id`).
    /// * `preps` — the CPU-prepared frames whose `llrs` feed the GPU decode.
    fn decode_batch(
        &mut self,
        device: &mut WorkerDevice<'_>,
        stream: &HipStream,
        batch_idx: usize,
        first_global_frame: u64,
        preps: &[FramePrep],
    ) -> GpuBatchResult;

    /// Whether the worker should stop after recording `batch_idx` (a SIGINT
    /// landed at a batch boundary). The scheduler returns `false` always; the
    /// checkpointed loop returns [`is_interrupted`](crate::checkpoint::is_interrupted).
    fn stop_after_batch(&self, batch_idx: usize) -> bool;
}

/// Runs one worker's strided frame partition through the shared §6 double-buffer
/// overlap protocol (`bb11c2e6` deliverable 1).
///
/// `my_frames` is the worker's global-frame partition (already filtered to the
/// round / point range). The core: (1) chunks it into [`BATCH_FRAMES`] batches;
/// (2) CPU-preps batch 0; (3) for each batch overlaps its GPU decode (on the
/// worker thread, via [`BatchHooks::decode_batch`]) against the CPU prep of the
/// next batch (on a scoped helper thread); (4) runs the SSOT BCH decode-tail +
/// error count for the just-decoded batch and records the per-frame counters;
/// (5) stops after a batch when [`BatchHooks::stop_after_batch`] returns `true`.
/// Every CPU-prep, GPU-decode, and decode-tail interval is wrapped in a
/// `pipeline_stage` `tracing` span and appended to the
/// [`OverlapTimeline`](crate::executor::OverlapTimeline) (deliverable 3 — both
/// callers now instrument identically).
///
/// Per-frame randomness is keyed on the GLOBAL frame index via
/// [`WorkerCtx::reseek_to_frame`] (the §3 logical-worker-0 convention), so the
/// per-frame outcome is a pure function of the global index regardless of which
/// physical worker — or helper thread — prepped it: the byte-identity rule the
/// whole hybrid path rests on.
///
/// `observe_frame` fires once per CPU-prepped frame after its prep (on whichever
/// thread prepped it — the main worker thread for batch 0, the double-buffer
/// helper thread for batches `N+1`). The checkpointed loop fires the campaign
/// frame observer here; the uncheckpointed scheduler passes a no-op. It must be
/// `Sync` because the helper thread also calls it.
///
/// # Arguments
///
/// * `sim` — the worker's frame-kernel clone (owns its BCH decode-tail decoder).
/// * `device` — the worker's borrowed device decoder + scratch.
/// * `stream` — the worker's owned HIP stream (all launches + transfers
///   stream-ordered on it; completion awaited per-stream inside the decode).
/// * `run_ctx` — the worker/stream/snr identifiers + instrumentation sinks.
/// * `my_frames` — the worker's global-frame partition for this run/round.
/// * `hooks` — the per-batch decode-dispatch / stop behaviours.
/// * `observe_frame` — per-frame observation callback.
///
/// # Returns
///
/// The worker's `(WorkerCounters, frames_completed)`. `frames_completed` is a
/// whole number of batches except the partition tail (or fewer on a SIGINT
/// stop) — the checkpointed loop advances its per-worker progress by it.
///
/// # Errors
///
/// Propagates the first [`StageError`](crate::error::StageError) a batch's
/// [`BatchHooks::decode_batch`] returns (a fatal GPU fault, or — on the
/// checkpointed caller — a propagated recoverable fault).
pub(crate) fn run_hybrid_double_buffer<H, O>(
    sim: &DvbT2BicmFrameSim,
    device: &mut WorkerDevice<'_>,
    stream: &HipStream,
    run_ctx: &HybridRunCtx<'_>,
    my_frames: &[usize],
    hooks: &mut H,
    observe_frame: &O,
) -> Result<(WorkerCounters, u64), crate::error::StageError>
where
    H: BatchHooks,
    O: Fn(usize) + Sync,
{
    if my_frames.is_empty() {
        return Ok((WorkerCounters::default(), 0));
    }
    let batches: Vec<&[usize]> = my_frames.chunks(BATCH_FRAMES).collect();

    let mut counters = WorkerCounters::default();
    let mut frames_done: u64 = 0;

    // CPU-prep one batch of frames into FramePrep, inside the stage's tracing
    // span + timeline interval. The per-frame RNG seek is keyed on the GLOBAL
    // frame index (§3), so a throwaway `ctx` seeked per frame is byte-identical
    // regardless of which thread runs the prep. Captures only `&sim` /
    // `&observe_frame` (both `Sync`), never the `&mut` decode state — so the
    // helper thread's prep never aliases the worker thread's decode.
    let prep_batch = |batch_idx: usize, frames: &[usize]| -> Vec<FramePrep> {
        traced_interval(run_ctx, batch_idx, "CpuPrep", ActivityKind::CpuPrep, || {
            let mut ctx = WorkerCtx::new(run_ctx.seed, run_ctx.snr_idx, 0);
            frames
                .iter()
                .map(|&g| {
                    ctx.reseek_to_frame(g);
                    let prep = sim.prepare_frame(g, &mut ctx);
                    observe_frame(g);
                    prep
                })
                .collect()
        })
    };

    let mut prepared = prep_batch(0, batches[0]);

    for bi in 0..batches.len() {
        let cur_preps = std::mem::take(&mut prepared);
        let next_idx = bi + 1;
        let first_global_frame = batches[bi].first().copied().unwrap_or(0) as u64;

        // The owned device decoder is `!Sync` (it owns device buffers), so the
        // GPU-blocking call stays on THIS worker thread while a scoped helper
        // preps batch N+1 (capturing only `Sync` state). `std::thread::scope`
        // keeps the borrows safe with no `'static` requirement.
        let (gpu_res, next_preps): (GpuBatchResult, Vec<FramePrep>) = std::thread::scope(|scope| {
            let prep_batch_ref = &prep_batch;
            let batches_ref = &batches;
            let cpu_handle = scope.spawn(move || {
                if next_idx < batches_ref.len() {
                    prep_batch_ref(next_idx, batches_ref[next_idx])
                } else {
                    Vec::new()
                }
            });

            // GPU decode of batch N on THIS worker thread, inside the
            // GpuDecode span + interval. The caller's hook owns the failure
            // semantics (fallback vs propagate) and any tally bracketing —
            // the core only times and routes it.
            let gpu_res =
                traced_interval(run_ctx, bi, "GpuLdpcBp", ActivityKind::GpuDecode, || {
                    hooks.decode_batch(device, stream, bi, first_global_frame, &cur_preps)
                });

            let next_preps = cpu_handle.join().expect("CPU prep helper thread");
            (gpu_res, next_preps)
        });

        let (codewords, iters) = gpu_res?;
        prepared = next_preps;

        // CPU BCH decode-tail + error count for the just-decoded batch.
        traced_interval(run_ctx, bi, "CpuDecodeTail", ActivityKind::CpuPrep, || {
            for (i, prep) in cur_preps.iter().enumerate() {
                let outcome =
                    sim.decode_codeword_to_outcome(&prep.message, &codewords[i], iters[i] as u64);
                counters.record_frame(
                    outcome.errored,
                    outcome.iterations,
                    outcome.info_bits,
                    outcome.bit_errors,
                );
            }
        });
        frames_done += cur_preps.len() as u64;

        if hooks.stop_after_batch(bi) {
            break;
        }
    }

    Ok((counters, frames_done))
}

/// Frames per GPU decode batch (the double-buffer unit). Sized so the device
/// LDPC kernel amortises its per-launch overhead while keeping two batches'
/// worth of host LLR scratch modest.
///
/// SSOT for BOTH hybrid callers: the uncheckpointed scheduler and the
/// checkpointed drain runner chunk their partitions by this exact unit, so
/// their batch composition — and therefore the per-frame GPU decode results —
/// match exactly (the checkpointed `mean_iters` byte-identity rests on it).
pub(crate) const BATCH_FRAMES: usize = 16;

/// Microseconds elapsed since `run_start`.
fn elapsed_us(run_start: Instant) -> u128 {
    run_start.elapsed().as_micros()
}

/// Runs `f` as one instrumented stage interval (deliverable 3): a `tracing`
/// span named `pipeline_stage` carrying
/// `(worker_idx, snr_idx, batch_id, stream_id, stage_name, wall_us)` — entered
/// for the duration of `f`, with the measured `wall_us` recorded on the span
/// just before close — plus the matching
/// [`ActivityInterval`](crate::executor::ActivityInterval) appended to the
/// [`OverlapTimeline`](crate::executor::OverlapTimeline) when the run context
/// carries a sink (the span and the interval mark the same boundaries; the
/// checkpointed path passes `None` and gets spans only).
///
/// Shared by both hybrid callers. Note: the `GpuLdpcBp` interval brackets the
/// caller's whole `decode_batch` hook, which on the scheduler path includes
/// the host-side staging of the fallback LLR clone — the GPU-active time in
/// the overlap attestation is therefore a slight over-count of device time.
fn traced_interval<T>(
    run_ctx: &HybridRunCtx<'_>,
    batch_id: usize,
    stage_name: &'static str,
    kind: ActivityKind,
    f: impl FnOnce() -> T,
) -> T {
    let worker_idx = run_ctx.worker_idx;
    let snr_idx = run_ctx.snr_idx;
    let stream_id = run_ctx.stream_id;
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
    let start_us = elapsed_us(run_ctx.run_start);
    let out = f();
    let end_us = elapsed_us(run_ctx.run_start);
    span.record("wall_us", (end_us - start_us) as u64);
    drop(entered);
    if let Some(timeline) = run_ctx.timeline {
        timeline
            .lock()
            .expect("overlap timeline mutex")
            .intervals
            .push(crate::executor::ActivityInterval {
                worker_idx,
                stream_id,
                kind,
                start_us,
                end_us,
            });
    }
    out
}
