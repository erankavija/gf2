//! Within-SNR frame parallelism, deterministic per-worker [`ChaCha20Rng`] seek,
//! and order-independent counter aggregation (design doc §3, §11).
//!
//! This module is the Phase A core of the `gf2-sim` parallel executor. It
//! provides the three primitives the Phase C hybrid executor (`de160fc5` /
//! `75c22fa8`) and the channel stages (`db9836e4`) build on:
//!
//! 1. [`worker_offset`] — the design-doc §3 per-worker ChaCha20 word-position
//!    seek (`worker_idx`, `frame_idx_in_worker` → `u128` offset).
//! 2. [`WorkerCtx`] — a per-worker simulation context owning an independent
//!    [`ChaCha20Rng`] seeded via [`worker_offset`], plus [`WorkerCounters`].
//! 3. [`run_snr_point`] — the frame-batch dispatch primitive: it runs a
//!    per-frame closure across `parallelism` rayon workers within one SNR point,
//!    each worker owning its own seeked RNG, then reduces the per-worker
//!    counters in **`worker_idx` order** (the SSOT aggregation order).
//!
//! # Determinism contract (design doc §3, §11)
//!
//! The headline guarantee is **byte-identical `fer` / `frames` / `errors` /
//! `mean_iters`** across worker counts `{1, 2, 4, 8, 24}` for a fixed seed.
//! This module achieves it with two rules:
//!
//! * **Per-frame RNG is keyed on the global frame index.** Every global frame
//!   `g` in an SNR point draws its channel noise from a `ChaCha20Rng` seeked to
//!   [`worker_offset`]`(seed, snr_idx, 0, g)`. Because the noise — and therefore
//!   the per-frame decode verdict — is a pure function of `g` alone, it does not
//!   matter which physical rayon worker happens to process frame `g`: the
//!   per-frame outcome is identical. The `worker_idx` parameter of
//!   [`worker_offset`] is reserved for the Phase C executor / GPU paths, where a
//!   worker owns a *fixed* partition of the frame space and seeds its whole
//!   partition from a single starting offset; the CPU within-SNR path treats the
//!   SNR point as one logical stream (`worker_idx = 0`) indexed by global frame,
//!   which is exactly [`worker_offset`] with the worker term zero. See
//!   [`run_snr_point`] for the dispatch and [`WorkerCtx::reseek_to_frame`] for
//!   the per-frame seek.
//! * **Aggregation iterates workers in `worker_idx` order.** Even though `u64`
//!   counter sums are order-invariant, the design doc fixes `worker_idx` order
//!   as the SSOT so a future migration to non-associative accumulators stays
//!   reproducible. [`WorkerCounters::reduce_in_worker_order`] enforces it.
//!
//! # No `unsafe`
//!
//! The crate is `#![deny(unsafe_code)]`; this module adds none. All
//! parallelism is via `rayon`, all RNG seeking via `rand_chacha`'s safe
//! `set_word_pos`.

use std::num::NonZeroUsize;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rayon::prelude::*;

/// ChaCha20 words reserved per SNR point (design doc §3): `2^56`.
///
/// Far above any practical run; guarantees distinct SNR points never share
/// stream regions.
pub const SNR_STRIDE: u128 = 1 << 56;

/// ChaCha20 words reserved per worker partition (design doc §3): `2^40` (1 TB).
///
/// At [`FRAME_STRIDE`]` = 2^16` this admits `2^24` (≈ 16 M) frames per worker
/// partition before a worker would run into the next worker's region.
pub const WORKER_STRIDE: u128 = 1 << 40;

/// ChaCha20 words reserved per frame (design doc §3): `2^16` (65536 words =
/// 512 KB).
///
/// Roughly 3× headroom over the worst-case per-frame noise draw for DVB-T2
/// 64-QAM and 5G NR BG1 `Z = 384` (see design-doc §3 arithmetic check).
pub const FRAME_STRIDE: u128 = 1 << 16;

/// Debug-assert headroom (design doc §3): a frame must draw at most
/// `FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN` ChaCha20 words.
pub const DEBUG_ASSERT_WORD_MARGIN: u128 = 1024;

/// Computes the per-worker ChaCha20 word-position seek offset (design doc §3).
///
/// This is the verbatim design-doc §3 seek scheme:
///
/// ```text
/// worker_offset(seed, snr_idx, worker_idx, frame_idx_in_worker) =
///     snr_idx * SNR_STRIDE                       // 2^56 words per SNR
///   + (worker_idx as u128) * WORKER_STRIDE       // 2^40 words per worker
///   + (frame_idx_in_worker as u128) * FRAME_STRIDE  // 2^16 words per frame
/// ```
///
/// The `seed` argument is part of the §3 signature but does *not* enter the
/// offset: the base seed selects the ChaCha20 *stream* (via
/// [`ChaCha20Rng::seed_from_u64`]); the offset selects the *position* within
/// that stream. [`WorkerCtx::new`] combines the two.
///
/// # Arguments
///
/// * `seed` — base RNG seed (selects the stream; see note above).
/// * `snr_idx` — zero-based index of the SNR point.
/// * `worker_idx` — zero-based worker partition index. The CPU within-SNR path
///   passes `0` and indexes by global frame (see module docs); the Phase C
///   executor passes the physical worker index.
/// * `frame_idx_in_worker` — zero-based frame index within the worker partition
///   (the global frame index when `worker_idx == 0`).
///
/// # Returns
///
/// The absolute ChaCha20 word position to seek to via
/// [`ChaCha20Rng::set_word_pos`].
///
/// # Complexity
///
/// `O(1)` — three `u128` multiplies and two adds.
///
/// # Examples
///
/// ```
/// use gf2_sim::parallel::{worker_offset, SNR_STRIDE, WORKER_STRIDE, FRAME_STRIDE};
///
/// // SNR 0, worker 0, frame 0 → start of the stream.
/// assert_eq!(worker_offset(42, 0, 0, 0), 0);
/// // Frame term scales by FRAME_STRIDE.
/// assert_eq!(worker_offset(42, 0, 0, 3), 3 * FRAME_STRIDE);
/// // Worker and SNR terms add their strides.
/// assert_eq!(
///     worker_offset(42, 2, 1, 5),
///     2 * SNR_STRIDE + WORKER_STRIDE + 5 * FRAME_STRIDE
/// );
/// ```
#[inline]
#[must_use]
pub fn worker_offset(
    seed: u64,
    snr_idx: usize,
    worker_idx: usize,
    frame_idx_in_worker: usize,
) -> u128 {
    let _ = seed; // seed selects the stream, not the offset (see docs).
    (snr_idx as u128) * SNR_STRIDE
        + (worker_idx as u128) * WORKER_STRIDE
        + (frame_idx_in_worker as u128) * FRAME_STRIDE
}

/// Order-independent per-worker simulation counters (design doc §3, §11).
///
/// Each worker accumulates into its own `WorkerCounters`; the SNR-point reducer
/// sums them in `worker_idx` order via [`reduce_in_worker_order`]. All fields
/// are `u64` (integer-exact, never excluded from byte-identity per design-doc
/// §10/§11).
///
/// [`reduce_in_worker_order`]: WorkerCounters::reduce_in_worker_order
///
/// # Examples
///
/// ```
/// use gf2_sim::parallel::WorkerCounters;
///
/// let mut a = WorkerCounters::default();
/// a.record_frame(/* errored */ true, /* iterations */ 12, /* bits */ 100, /* bit_errors */ 3);
/// let mut b = WorkerCounters::default();
/// b.record_frame(false, 1, 100, 0);
///
/// let total = WorkerCounters::reduce_in_worker_order(&[a, b]);
/// assert_eq!(total.frames, 2);
/// assert_eq!(total.errors, 1);
/// assert_eq!(total.total_iterations, 13);
/// assert_eq!(total.total_bits, 200);
/// assert_eq!(total.total_bit_errors, 3);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerCounters {
    /// Frames simulated by this worker.
    pub frames: u64,
    /// Frames in error (decoded BBFRAME ≠ transmitted BBFRAME).
    pub errors: u64,
    /// Sum of decoder iteration counts across this worker's frames.
    pub total_iterations: u64,
    /// Sum of information bits across this worker's frames.
    pub total_bits: u64,
    /// Sum of bit errors across this worker's frames.
    pub total_bit_errors: u64,
}

impl WorkerCounters {
    /// Records one completed frame into the counters.
    ///
    /// # Arguments
    ///
    /// * `errored` — `true` if the frame is in error (any information-bit
    ///   mismatch); increments [`errors`](Self::errors).
    /// * `iterations` — decoder iteration count for the frame.
    /// * `bits` — number of information bits compared for the frame.
    /// * `bit_errors` — number of mismatched information bits for the frame.
    #[inline]
    pub fn record_frame(&mut self, errored: bool, iterations: u64, bits: u64, bit_errors: u64) {
        self.frames += 1;
        self.errors += u64::from(errored);
        self.total_iterations += iterations;
        self.total_bits += bits;
        self.total_bit_errors += bit_errors;
    }

    /// Adds another worker's counters into `self` (field-wise `u64` sum).
    #[inline]
    fn add(&mut self, other: &WorkerCounters) {
        self.frames += other.frames;
        self.errors += other.errors;
        self.total_iterations += other.total_iterations;
        self.total_bits += other.total_bits;
        self.total_bit_errors += other.total_bit_errors;
    }

    /// Reduces per-worker counters into a single total **in `worker_idx`
    /// order** (design doc §3 SSOT aggregation order).
    ///
    /// The input slice must be indexed by `worker_idx` (element `i` is worker
    /// `i`'s counters). The reduction iterates `0..workers.len()` in order; this
    /// is the mandated SSOT order even though `u64` addition is associative.
    ///
    /// # Arguments
    ///
    /// * `workers` — per-worker counters, indexed by `worker_idx`.
    ///
    /// # Returns
    ///
    /// The field-wise sum of all workers' counters.
    ///
    /// # Complexity
    ///
    /// `O(workers.len())`.
    #[must_use]
    pub fn reduce_in_worker_order(workers: &[WorkerCounters]) -> WorkerCounters {
        let mut total = WorkerCounters::default();
        // Iterate strictly in worker_idx (slice) order — the SSOT order.
        for w in workers {
            total.add(w);
        }
        total
    }

    /// Frame error rate (`errors / frames`), or `0.0` when no frames ran.
    ///
    /// This is a derived `f64` ratio of the two integer-exact counters, so it
    /// is itself byte-identical across worker counts.
    #[must_use]
    pub fn fer(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.errors as f64 / self.frames as f64
        }
    }

    /// Mean decoder iterations per frame, or `0.0` when no frames ran.
    #[must_use]
    pub fn mean_iters(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.total_iterations as f64 / self.frames as f64
        }
    }
}

/// Per-worker simulation context: an independent, seeked [`ChaCha20Rng`] plus
/// the worker's [`WorkerCounters`] (design doc §3).
///
/// A `WorkerCtx` owns one ChaCha20 stream (selected by the base `seed`) and is
/// repositioned per frame via [`reseek_to_frame`](Self::reseek_to_frame) so the
/// frame's noise draw starts at the §3 word offset. This is the reusable
/// surface the Phase C executor (`de160fc5`) and channel stages (`db9836e4`)
/// consume.
///
/// # Examples
///
/// ```
/// use gf2_sim::parallel::WorkerCtx;
///
/// // Two contexts on the same seed seeked to the same (snr, worker, frame)
/// // produce byte-identical draws.
/// let mut a = WorkerCtx::new(7, 0, 0);
/// let mut b = WorkerCtx::new(7, 0, 0);
/// a.reseek_to_frame(0);
/// b.reseek_to_frame(0);
/// use rand::Rng;
/// let xa: u64 = a.rng_mut().random();
/// let xb: u64 = b.rng_mut().random();
/// assert_eq!(xa, xb);
/// ```
pub struct WorkerCtx {
    seed: u64,
    snr_idx: usize,
    worker_idx: usize,
    rng: ChaCha20Rng,
    counters: WorkerCounters,
}

impl WorkerCtx {
    /// Builds a worker context with a fresh [`ChaCha20Rng`] for `(seed,
    /// snr_idx, worker_idx)`.
    ///
    /// The RNG is created from `seed` (selecting the stream) and left at word
    /// position 0; call [`reseek_to_frame`](Self::reseek_to_frame) before each
    /// frame to position it at the §3 offset.
    ///
    /// # Arguments
    ///
    /// * `seed` — base RNG seed.
    /// * `snr_idx` — zero-based SNR-point index.
    /// * `worker_idx` — zero-based worker partition index.
    #[must_use]
    pub fn new(seed: u64, snr_idx: usize, worker_idx: usize) -> Self {
        Self {
            seed,
            snr_idx,
            worker_idx,
            rng: ChaCha20Rng::seed_from_u64(seed),
            counters: WorkerCounters::default(),
        }
    }

    /// Repositions the RNG to the start of `frame_idx_in_worker`'s noise region.
    ///
    /// Seeks to [`worker_offset`]`(seed, snr_idx, worker_idx, frame_idx_in_worker)`.
    /// After this call the next `random()` draws begin at the frame's reserved
    /// 512 KB ChaCha20 region.
    ///
    /// # Arguments
    ///
    /// * `frame_idx_in_worker` — zero-based frame index within this worker's
    ///   partition (the global frame index when `worker_idx == 0`).
    pub fn reseek_to_frame(&mut self, frame_idx_in_worker: usize) {
        let pos = worker_offset(
            self.seed,
            self.snr_idx,
            self.worker_idx,
            frame_idx_in_worker,
        );
        self.rng.set_word_pos(pos);
    }

    /// Mutable access to the worker's [`ChaCha20Rng`] for channel noise draws.
    #[inline]
    pub fn rng_mut(&mut self) -> &mut ChaCha20Rng {
        &mut self.rng
    }

    /// Mutable access to the worker's [`WorkerCounters`].
    #[inline]
    pub fn counters_mut(&mut self) -> &mut WorkerCounters {
        &mut self.counters
    }

    /// The worker's accumulated [`WorkerCounters`] (by value).
    #[inline]
    #[must_use]
    pub fn counters(&self) -> WorkerCounters {
        self.counters
    }

    /// This worker's `worker_idx`.
    #[inline]
    #[must_use]
    pub fn worker_idx(&self) -> usize {
        self.worker_idx
    }

    /// Debug-asserts the per-frame draw stayed within its reserved region.
    ///
    /// Call after a frame's noise draws. Checks that the current word position
    /// has not advanced past `frame_start + FRAME_STRIDE -
    /// DEBUG_ASSERT_WORD_MARGIN` (design doc §3 debug assert). No-op in release
    /// builds.
    ///
    /// # Arguments
    ///
    /// * `frame_idx_in_worker` — the frame whose region was just drawn from.
    pub fn debug_assert_frame_budget(&self, frame_idx_in_worker: usize) {
        debug_assert!({
            let start = worker_offset(
                self.seed,
                self.snr_idx,
                self.worker_idx,
                frame_idx_in_worker,
            );
            let now = self.rng.get_word_pos();
            let drawn = now.saturating_sub(start);
            drawn <= FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN
        });
    }
}

/// Outcome of simulating a single frame, returned by the per-frame closure
/// passed to [`run_snr_point`].
///
/// The counters reducer turns these into the order-independent SNR-point
/// totals. All fields are integer-exact (`u64` / `bool`), so the resulting
/// `fer` / `frames` / `errors` / `mean_iters` are byte-identical across worker
/// counts.
///
/// # Examples
///
/// ```
/// use gf2_sim::parallel::FrameOutcome;
///
/// let ok = FrameOutcome { errored: false, iterations: 1, info_bits: 32400, bit_errors: 0 };
/// assert!(!ok.errored);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameOutcome {
    /// `true` if the frame is in error (any information-bit mismatch).
    pub errored: bool,
    /// Decoder iteration count for the frame.
    pub iterations: u64,
    /// Information bits compared for the frame.
    pub info_bits: u64,
    /// Information-bit errors for the frame.
    pub bit_errors: u64,
}

/// Runs one SNR point across `parallelism` rayon workers, returning the
/// `worker_idx`-ordered aggregate counters (design doc §3, §11).
///
/// This is the **frame-batch dispatch primitive** (issue deliverable 2). It
/// simulates global frames `0..max_frames` (the order is irrelevant to the
/// result, see below) by distributing them across `parallelism.get()` rayon
/// workers. Each worker owns its own [`WorkerCtx`] (an independent seeked
/// [`ChaCha20Rng`]). For every global frame `g` the worker reseeks its RNG to
/// the frame's §3 offset and invokes `sim_frame(g, ctx)`, which returns a
/// [`FrameOutcome`]. The per-worker counters are then reduced in `worker_idx`
/// order via [`WorkerCounters::reduce_in_worker_order`].
///
/// # Byte-identity across worker counts
///
/// Each global frame `g` is seeded *only* by `g` (the RNG is reseeked to
/// [`worker_offset`]`(seed, snr_idx, 0, g)` before the closure runs), so its
/// [`FrameOutcome`] is a pure function of `g`, independent of which physical
/// worker ran it or how many workers there are. The aggregate over the fixed
/// frame set `0..max_frames` is therefore byte-identical for any `parallelism`.
/// The closure receives the worker's `WorkerCtx` already positioned, so it
/// simply draws noise from `ctx.rng_mut()`.
///
/// Note: this primitive runs the full `max_frames` budget; early-stop on
/// `target_errors` is a Phase C executor concern (the executor will cap the
/// global-frame range it dispatches). Keeping the frame set fixed here is what
/// guarantees byte-identity.
///
/// # Per-worker mutable state
///
/// Many realistic per-frame kernels need **per-worker mutable state** that must
/// not be shared by `&` across workers — most importantly a decoder with
/// interior mutability (e.g. `gf2-coding`'s `DvbT2Concat` wraps its LDPC decoder
/// in a `Mutex`, so a single shared instance would serialise every worker on the
/// lock and erase the speedup). [`run_snr_point`] therefore builds one fresh
/// state value per worker via the `make_state` factory and threads it into the
/// frame closure by `&mut`. The factory must produce *equivalent* state on every
/// call (a clone of the same configured decoder), so the per-frame outcome stays
/// a pure function of the global frame index. For a stateless kernel, use
/// [`run_snr_point_stateless`].
///
/// # Arguments
///
/// * `seed` — base RNG seed.
/// * `snr_idx` — zero-based SNR-point index (selects the [`SNR_STRIDE`] region).
/// * `max_frames` — number of global frames to simulate (`0..max_frames`).
/// * `parallelism` — number of rayon workers to fan out across.
/// * `make_state` — per-worker state factory `() -> S`, called once per worker.
///   Must be `Sync` and produce equivalent state each call.
/// * `sim_frame` — per-frame closure `(global_frame_idx, &mut WorkerCtx, &mut S)
///   -> FrameOutcome`. Must be `Sync` and draw all randomness from
///   `ctx.rng_mut()` so the result stays a pure function of the frame index.
///
/// # Returns
///
/// The `worker_idx`-ordered aggregate [`WorkerCounters`] for the SNR point.
///
/// # Complexity
///
/// `O(max_frames)` frame closures fanned out across `parallelism` workers, plus
/// one `make_state` call per worker; the reduction is `O(parallelism)`.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
/// use gf2_sim::parallel::{run_snr_point, FrameOutcome};
/// use rand::Rng;
///
/// // Per-worker state: a running XOR fold (stands in for a decoder's scratch).
/// let one = run_snr_point(
///     99, 0, 64, NonZeroUsize::new(1).unwrap(),
///     || 0u64,
///     |_g, ctx, acc| {
///         let x: u64 = ctx.rng_mut().random();
///         *acc ^= x;
///         FrameOutcome { errored: x & 1 == 1, iterations: 1, info_bits: 8, bit_errors: x & 1 }
///     },
/// );
/// let eight = run_snr_point(
///     99, 0, 64, NonZeroUsize::new(8).unwrap(),
///     || 0u64,
///     |_g, ctx, acc| {
///         let x: u64 = ctx.rng_mut().random();
///         *acc ^= x;
///         FrameOutcome { errored: x & 1 == 1, iterations: 1, info_bits: 8, bit_errors: x & 1 }
///     },
/// );
/// // Byte-identical regardless of worker count.
/// assert_eq!(one, eight);
/// assert_eq!(one.frames, 64);
/// ```
pub fn run_snr_point<S, M, F>(
    seed: u64,
    snr_idx: usize,
    max_frames: usize,
    parallelism: NonZeroUsize,
    make_state: M,
    sim_frame: F,
) -> WorkerCounters
where
    M: Fn() -> S + Sync,
    F: Fn(usize, &mut WorkerCtx, &mut S) -> FrameOutcome + Sync,
{
    let num_workers = parallelism.get();

    // Per-worker counters, indexed by worker_idx. Each worker writes only its
    // own slot, so the fan-out is data-race free without locking.
    let per_worker: Vec<WorkerCounters> = (0..num_workers)
        .into_par_iter()
        .map(|worker_idx| {
            // Logical worker 0: the CPU within-SNR path keys the per-frame seek
            // on the global frame index (design-doc §3, module docs). The
            // physical `worker_idx` only decides *which* global frames this
            // worker processes, never the RNG stream those frames see.
            let mut ctx = WorkerCtx::new(seed, snr_idx, 0);
            // One fresh state per worker — never shared by `&` across threads.
            let mut state = make_state();

            // Contiguous strided assignment: worker `w` processes global frames
            // w, w + num_workers, w + 2*num_workers, ... < max_frames. The
            // assignment partitions 0..max_frames exactly once across workers
            // for any worker count; the RNG seek (keyed on g) makes the outcome
            // independent of the partitioning.
            let mut g = worker_idx;
            while g < max_frames {
                ctx.reseek_to_frame(g);
                let outcome = sim_frame(g, &mut ctx, &mut state);
                ctx.debug_assert_frame_budget(g);
                ctx.counters_mut().record_frame(
                    outcome.errored,
                    outcome.iterations,
                    outcome.info_bits,
                    outcome.bit_errors,
                );
                g += num_workers;
            }
            ctx.counters()
        })
        .collect();

    WorkerCounters::reduce_in_worker_order(&per_worker)
}

/// Stateless convenience wrapper over [`run_snr_point`] for per-frame kernels
/// that need no per-worker mutable state.
///
/// Use this only when the frame closure is genuinely stateless (e.g. a synthetic
/// channel). For any kernel holding an interior-mutable decoder, use
/// [`run_snr_point`] with a per-worker `make_state` factory so the workers do not
/// serialise on a shared lock.
///
/// # Arguments
///
/// Same as [`run_snr_point`] minus `make_state`; `sim_frame` is
/// `(global_frame_idx, &mut WorkerCtx) -> FrameOutcome`.
///
/// # Returns
///
/// The `worker_idx`-ordered aggregate [`WorkerCounters`] for the SNR point.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
/// use gf2_sim::parallel::{run_snr_point_stateless, FrameOutcome};
/// use rand::Rng;
///
/// let sim = |_g: usize, ctx: &mut gf2_sim::parallel::WorkerCtx| {
///     let x: u64 = ctx.rng_mut().random();
///     FrameOutcome { errored: x & 1 == 1, iterations: 1, info_bits: 8, bit_errors: x & 1 }
/// };
/// let one = run_snr_point_stateless(99, 0, 64, NonZeroUsize::new(1).unwrap(), &sim);
/// let eight = run_snr_point_stateless(99, 0, 64, NonZeroUsize::new(8).unwrap(), &sim);
/// assert_eq!(one, eight);
/// ```
pub fn run_snr_point_stateless<F>(
    seed: u64,
    snr_idx: usize,
    max_frames: usize,
    parallelism: NonZeroUsize,
    sim_frame: &F,
) -> WorkerCounters
where
    F: Fn(usize, &mut WorkerCtx) -> FrameOutcome + Sync,
{
    run_snr_point(
        seed,
        snr_idx,
        max_frames,
        parallelism,
        || (),
        |g, ctx, ()| sim_frame(g, ctx),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_worker_offset_verbatim_formula() {
        // Matches the design-doc §3 three-term formula exactly.
        assert_eq!(worker_offset(0, 0, 0, 0), 0);
        assert_eq!(worker_offset(0, 0, 0, 1), FRAME_STRIDE);
        assert_eq!(worker_offset(0, 0, 1, 0), WORKER_STRIDE);
        assert_eq!(worker_offset(0, 1, 0, 0), SNR_STRIDE);
        assert_eq!(
            worker_offset(0, 3, 2, 7),
            3 * SNR_STRIDE + 2 * WORKER_STRIDE + 7 * FRAME_STRIDE
        );
    }

    #[test]
    fn test_worker_offset_seed_does_not_affect_offset() {
        // The seed selects the stream, not the position.
        assert_eq!(worker_offset(1, 2, 3, 4), worker_offset(999, 2, 3, 4));
    }

    #[test]
    fn test_strides_are_power_of_two_and_ordered() {
        assert_eq!(FRAME_STRIDE, 1 << 16);
        assert_eq!(WORKER_STRIDE, 1 << 40);
        assert_eq!(SNR_STRIDE, 1 << 56);
        const _: () = assert!(FRAME_STRIDE < WORKER_STRIDE);
        const _: () = assert!(WORKER_STRIDE < SNR_STRIDE);
    }

    #[test]
    fn test_reduce_in_worker_order_sums_all_fields() {
        let workers = [
            WorkerCounters {
                frames: 10,
                errors: 1,
                total_iterations: 50,
                total_bits: 1000,
                total_bit_errors: 3,
            },
            WorkerCounters {
                frames: 5,
                errors: 2,
                total_iterations: 25,
                total_bits: 500,
                total_bit_errors: 7,
            },
        ];
        let total = WorkerCounters::reduce_in_worker_order(&workers);
        assert_eq!(total.frames, 15);
        assert_eq!(total.errors, 3);
        assert_eq!(total.total_iterations, 75);
        assert_eq!(total.total_bits, 1500);
        assert_eq!(total.total_bit_errors, 10);
    }

    #[test]
    fn test_reduce_empty_is_zero() {
        let total = WorkerCounters::reduce_in_worker_order(&[]);
        assert_eq!(total, WorkerCounters::default());
    }

    #[test]
    fn test_fer_and_mean_iters_ratios() {
        let c = WorkerCounters {
            frames: 4,
            errors: 1,
            total_iterations: 8,
            total_bits: 0,
            total_bit_errors: 0,
        };
        assert!((c.fer() - 0.25).abs() < 1e-12);
        assert!((c.mean_iters() - 2.0).abs() < 1e-12);
        // No-frame guard.
        assert_eq!(WorkerCounters::default().fer(), 0.0);
        assert_eq!(WorkerCounters::default().mean_iters(), 0.0);
    }

    #[test]
    fn test_worker_ctx_reseek_is_deterministic() {
        let mut a = WorkerCtx::new(123, 1, 0);
        let mut b = WorkerCtx::new(123, 1, 0);
        // Seek both to frame 5 and compare a block of draws.
        a.reseek_to_frame(5);
        b.reseek_to_frame(5);
        for _ in 0..16 {
            let xa: u64 = a.rng_mut().random();
            let xb: u64 = b.rng_mut().random();
            assert_eq!(xa, xb);
        }
    }

    #[test]
    fn test_distinct_frames_use_distinct_streams() {
        let mut ctx = WorkerCtx::new(7, 0, 0);
        ctx.reseek_to_frame(0);
        let f0: u64 = ctx.rng_mut().random();
        ctx.reseek_to_frame(1);
        let f1: u64 = ctx.rng_mut().random();
        assert_ne!(f0, f1, "different frames must seek to different regions");
    }

    /// Fast-tier smoke guard for the seek + aggregation logic: a synthetic
    /// per-frame closure whose outcome is a pure function of the global frame
    /// index must aggregate byte-identically across {1, 2} workers. The full
    /// {1,2,4,8,24} × 3-config DVB-T2 regression lives in the ignored
    /// integration test (see `tests/`).
    #[test]
    fn test_run_snr_point_byte_identical_smoke() {
        let sim = |_g: usize, ctx: &mut WorkerCtx| {
            // Draw two f64s as the real channel does (Box-Muller pair), derive a
            // synthetic verdict from the stream so the outcome depends on the
            // frame's seek position.
            let u1: f64 = ctx.rng_mut().random();
            let u2: f64 = ctx.rng_mut().random();
            let errored = (u1 + u2) > 1.0;
            FrameOutcome {
                errored,
                iterations: if errored { 50 } else { 1 },
                info_bits: 32,
                bit_errors: u64::from(errored),
            }
        };

        let one = run_snr_point_stateless(0xABCD, 2, 40, NonZeroUsize::new(1).unwrap(), &sim);
        let two = run_snr_point_stateless(0xABCD, 2, 40, NonZeroUsize::new(2).unwrap(), &sim);
        assert_eq!(one, two);
        assert_eq!(one.frames, 40);
    }

    #[test]
    fn test_per_worker_state_factory_runs_once_per_worker() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // The factory must be invoked exactly `num_workers` times (once per
        // worker), confirming state is per-worker, not per-frame.
        let factory_calls = AtomicUsize::new(0);
        let counters = run_snr_point(
            1,
            0,
            20,
            NonZeroUsize::new(4).unwrap(),
            || {
                factory_calls.fetch_add(1, Ordering::Relaxed);
                0u64
            },
            |_g, ctx, acc| {
                let x: u64 = ctx.rng_mut().random();
                *acc = acc.wrapping_add(x);
                FrameOutcome {
                    errored: false,
                    iterations: 1,
                    info_bits: 1,
                    bit_errors: 0,
                }
            },
        );
        assert_eq!(counters.frames, 20);
        assert_eq!(factory_calls.load(Ordering::Relaxed), 4);
    }
}
