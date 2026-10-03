//! Within-SNR frame parallelism: the per-frame [`ChaCha20Rng`] seek
//! ([`worker_offset`], [`WorkerCtx`]), frame dispatch across rayon workers
//! ([`run_snr_point`], [`map_indices_in_order`]) and counter reduction in
//! `worker_idx` order ([`WorkerCounters`]).

use std::num::NonZeroUsize;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rayon::prelude::*;

/// ChaCha20 32-bit words reserved per SNR point: `2^56`.
///
/// All strides are in the unit of [`ChaCha20Rng::set_word_pos`] and
/// [`ChaCha20Rng::get_word_pos`].
pub const SNR_STRIDE: u128 = 1 << 56;

/// ChaCha20 32-bit words reserved per worker partition: `2^40`, which admits
/// `2^20` frames per partition at [`FRAME_STRIDE`]` = 2^20`.
pub const WORKER_STRIDE: u128 = 1 << 40;

/// ChaCha20 32-bit words reserved per frame: `2^20`.
///
/// The stride exceeds the largest per-frame draw of any supported modulation,
/// that of r1/2 QPSK Normal: 32400 symbols × 2 axes × 4 words per noise sample
/// plus the BBFRAME fill, checked by
/// `tests::test_worst_case_frame_draw_under_stride`.
pub const FRAME_STRIDE: u128 = 1 << 20;

/// Debug-assert headroom: a frame draws at most
/// `FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN` ChaCha20 32-bit words.
pub const DEBUG_ASSERT_WORD_MARGIN: u128 = 1024;

/// Computes the ChaCha20 word-position seek offset
/// `snr_idx * SNR_STRIDE + worker_idx * WORKER_STRIDE + frame_idx_in_worker * FRAME_STRIDE`.
///
/// `seed` does not enter the offset: it selects the ChaCha20 stream (via
/// [`ChaCha20Rng::seed_from_u64`]) and the offset selects the position within
/// it. The CPU within-SNR path passes `worker_idx = 0` and the global frame
/// index.
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

/// The global indices worker `worker_idx` processes under the strided
/// partition of `indices` across `num_workers`.
///
/// Worker `w` of `W` takes `start + w`, `start + w + W`, `start + w + 2W`, …
/// below `end`, so workers `0..W` cover every index of `indices` exactly once.
/// A `worker_idx` at or above `num_workers` yields an empty iterator or a
/// subsequence that overlaps another worker's.
pub fn worker_index_partition(
    indices: std::ops::Range<u64>,
    worker_idx: usize,
    num_workers: NonZeroUsize,
) -> impl Iterator<Item = u64> + Clone {
    let start = indices.start.saturating_add(worker_idx as u64);
    (start..indices.end).step_by(num_workers.get())
}

/// Evaluates every global index of `indices` across `parallelism` rayon
/// workers and returns the outcomes in global index order.
///
/// The result depends on `indices` and `evaluate`, not on `parallelism`
/// (`tests::test_map_indices_in_order_is_byte_identical_across_worker_counts`),
/// provided `make_state`, called once per worker, produces equivalent state on
/// every call and no index's outcome depends on the state left by another. A
/// single worker runs the range on the calling thread.
///
/// # Panics
///
/// Panics when `indices` is longer than `usize::MAX`.
///
/// # Complexity
///
/// `O(indices.len())` `evaluate` calls plus one `make_state` call per worker;
/// all outcomes are held in memory at once.
pub fn map_indices_in_order<S, T, M, F>(
    indices: std::ops::Range<u64>,
    parallelism: NonZeroUsize,
    make_state: M,
    evaluate: F,
) -> Vec<T>
where
    M: Fn() -> S + Sync,
    F: Fn(u64, &mut S) -> T + Sync,
    T: Send,
{
    let num_workers = parallelism.get();
    let len = usize::try_from(indices.end.saturating_sub(indices.start))
        .expect("dispatched index range must fit the host word size");
    if num_workers == 1 {
        let mut state = make_state();
        return indices.map(|index| evaluate(index, &mut state)).collect();
    }

    let per_worker: Vec<Vec<T>> = (0..num_workers)
        .into_par_iter()
        .map(|worker_idx| {
            let mut state = make_state();
            worker_index_partition(indices.clone(), worker_idx, parallelism)
                .map(|index| evaluate(index, &mut state))
                .collect()
        })
        .collect();

    // Reassemble the strided subsequences into index order: the outcome for
    // `indices.start + offset` sits at position `offset / num_workers` of
    // worker `offset % num_workers`.
    let mut cursors: Vec<_> = per_worker.into_iter().map(Vec::into_iter).collect();
    let mut ordered = Vec::with_capacity(len);
    for offset in 0..len {
        ordered.push(
            cursors[offset % num_workers]
                .next()
                .expect("the strided partition covers every dispatched index"),
        );
    }
    ordered
}

/// Per-worker simulation counters, summed across workers by
/// [`reduce_in_worker_order`](WorkerCounters::reduce_in_worker_order).
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
    #[inline]
    pub fn record_frame(&mut self, errored: bool, iterations: u64, bits: u64, bit_errors: u64) {
        self.frames += 1;
        self.errors += u64::from(errored);
        self.total_iterations += iterations;
        self.total_bits += bits;
        self.total_bit_errors += bit_errors;
    }

    #[inline]
    fn add(&mut self, other: &WorkerCounters) {
        self.frames += other.frames;
        self.errors += other.errors;
        self.total_iterations += other.total_iterations;
        self.total_bits += other.total_bits;
        self.total_bit_errors += other.total_bit_errors;
    }

    /// Reduces per-worker counters, indexed by `worker_idx`, into their
    /// field-wise sum, iterating in slice order.
    #[must_use]
    pub fn reduce_in_worker_order(workers: &[WorkerCounters]) -> WorkerCounters {
        let mut total = WorkerCounters::default();
        for w in workers {
            total.add(w);
        }
        total
    }

    /// Frame error rate (`errors / frames`), or `0.0` when no frames ran.
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

/// Per-worker simulation context: a [`ChaCha20Rng`] on the stream selected by
/// `seed`, repositioned per frame by
/// [`reseek_to_frame`](Self::reseek_to_frame), plus the worker's
/// [`WorkerCounters`].
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
    /// The RNG is left at word position 0; call
    /// [`reseek_to_frame`](Self::reseek_to_frame) before each frame.
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

    /// Seeks the RNG to
    /// [`worker_offset`]`(seed, snr_idx, worker_idx, frame_idx_in_worker)`.
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

    /// The RNG's current absolute word position (ChaCha20 32-bit words).
    #[inline]
    #[must_use]
    pub fn current_word_pos(&self) -> u128 {
        self.rng.get_word_pos()
    }

    /// Debug-asserts that the RNG has advanced at most
    /// `FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN` words past the start of frame
    /// `frame_idx_in_worker`'s region. No-op in release builds.
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

/// Runs global frames `0..max_frames` of one SNR point across `parallelism`
/// rayon workers and returns the counters reduced in `worker_idx` order.
///
/// Before `sim_frame(g, ctx, state)` runs, the worker's RNG is reseeked to
/// [`worker_offset`]`(seed, snr_idx, 0, g)`. When `sim_frame` draws all
/// randomness from `ctx.rng_mut()` and `make_state`, called once per worker,
/// produces equivalent state on every call, each [`FrameOutcome`] is a
/// function of `g` and the aggregate does not depend on `parallelism`
/// (`tests::test_run_snr_point_byte_identical_smoke` compares 1 and 2
/// workers). Every frame of `0..max_frames` runs; there is no early stop on an
/// error target.
///
/// # Complexity
///
/// `O(max_frames)` `sim_frame` calls plus one `make_state` call per worker.
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
    run_snr_point_range(
        seed,
        snr_idx,
        0..max_frames,
        parallelism,
        make_state,
        sim_frame,
    )
    .counters
}

/// Per-worker breakdown of a [`run_snr_point_range`] dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnrPointRangeOutcome {
    /// The `worker_idx`-ordered aggregate counters over the dispatched range.
    pub counters: WorkerCounters,
    /// Frames each worker simulated in this dispatch, indexed by `worker_idx`.
    pub per_worker_frames: Vec<u64>,
}

/// Runs the global-frame sub-range `frames` of an SNR point across
/// `parallelism` rayon workers.
///
/// The per-frame seek is that of [`run_snr_point`], so the aggregate over a
/// frame set is the same whether it runs as one `0..N` dispatch or as `0..M`
/// then `M..N`, as the example checks.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
/// use gf2_sim::parallel::{run_snr_point_range, FrameOutcome};
/// use rand::Rng;
///
/// let sim = |_g: usize, ctx: &mut gf2_sim::parallel::WorkerCtx, _s: &mut ()| {
///     let x: u64 = ctx.rng_mut().random();
///     FrameOutcome { errored: x & 1 == 1, iterations: 1, info_bits: 8, bit_errors: x & 1 }
/// };
/// // Whole range in one dispatch.
/// let whole = run_snr_point_range(5, 0, 0..40, NonZeroUsize::new(4).unwrap(), || (), sim);
/// // Same range split into two chunks, summed.
/// let a = run_snr_point_range(5, 0, 0..17, NonZeroUsize::new(4).unwrap(), || (), sim);
/// let b = run_snr_point_range(5, 0, 17..40, NonZeroUsize::new(4).unwrap(), || (), sim);
/// let mut summed = a.counters;
/// summed = gf2_sim::parallel::WorkerCounters::reduce_in_worker_order(&[summed, b.counters]);
/// assert_eq!(whole.counters, summed);
/// ```
pub fn run_snr_point_range<S, M, F>(
    seed: u64,
    snr_idx: usize,
    frames: std::ops::Range<usize>,
    parallelism: NonZeroUsize,
    make_state: M,
    sim_frame: F,
) -> SnrPointRangeOutcome
where
    M: Fn() -> S + Sync,
    F: Fn(usize, &mut WorkerCtx, &mut S) -> FrameOutcome + Sync,
{
    let num_workers = parallelism.get();
    let start = frames.start;
    let end = frames.end;

    let per_worker: Vec<(WorkerCounters, u64)> = (0..num_workers)
        .into_par_iter()
        .map(|worker_idx| {
            // Logical worker 0: the seek is keyed on the global frame index;
            // the physical `worker_idx` only selects which frames run here.
            let mut ctx = WorkerCtx::new(seed, snr_idx, 0);
            let mut state = make_state();

            let mut frames_done: u64 = 0;
            for g in worker_index_partition(start as u64..end as u64, worker_idx, parallelism) {
                let g = usize::try_from(g).expect("frame index fits the host word size");
                ctx.reseek_to_frame(g);
                let outcome = sim_frame(g, &mut ctx, &mut state);
                ctx.debug_assert_frame_budget(g);
                ctx.counters_mut().record_frame(
                    outcome.errored,
                    outcome.iterations,
                    outcome.info_bits,
                    outcome.bit_errors,
                );
                frames_done += 1;
            }
            (ctx.counters(), frames_done)
        })
        .collect();

    let counters_only: Vec<WorkerCounters> = per_worker.iter().map(|(c, _)| *c).collect();
    let per_worker_frames: Vec<u64> = per_worker.iter().map(|(_, f)| *f).collect();
    SnrPointRangeOutcome {
        counters: WorkerCounters::reduce_in_worker_order(&counters_only),
        per_worker_frames,
    }
}

/// [`run_snr_point`] for per-frame kernels without per-worker mutable state.
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
        assert_eq!(worker_offset(1, 2, 3, 4), worker_offset(999, 2, 3, 4));
    }

    #[test]
    fn test_strides_are_power_of_two_and_ordered() {
        assert_eq!(FRAME_STRIDE, 1 << 20);
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
        assert_eq!(WorkerCounters::default().fer(), 0.0);
        assert_eq!(WorkerCounters::default().mean_iters(), 0.0);
    }

    #[test]
    fn test_worker_ctx_reseek_is_deterministic() {
        let mut a = WorkerCtx::new(123, 1, 0);
        let mut b = WorkerCtx::new(123, 1, 0);
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

    #[test]
    fn test_run_snr_point_byte_identical_smoke() {
        let sim = |_g: usize, ctx: &mut WorkerCtx| {
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
    fn test_worker_index_partition_covers_the_range_exactly_once() {
        let range = 7_u64..40;
        for workers in [1_usize, 2, 3, 8, 24, 64] {
            let parallelism = NonZeroUsize::new(workers).unwrap();
            let mut covered: Vec<u64> = (0..workers)
                .flat_map(|w| worker_index_partition(range.clone(), w, parallelism))
                .collect();
            covered.sort_unstable();
            assert_eq!(
                covered,
                range.clone().collect::<Vec<_>>(),
                "{workers} workers must cover the range exactly once"
            );
        }
    }

    #[test]
    fn test_worker_index_partition_is_empty_past_the_range() {
        let one = NonZeroUsize::new(1).unwrap();
        assert_eq!(worker_index_partition(5..5, 0, one).count(), 0);
        // A worker index beyond the range start yields nothing rather than
        // wrapping the addition.
        let big = NonZeroUsize::new(8).unwrap();
        assert_eq!(worker_index_partition(0..3, 5, big).count(), 0);
    }

    #[test]
    fn test_map_indices_in_order_is_byte_identical_across_worker_counts() {
        let evaluate = |index: u64, _state: &mut ()| index.wrapping_mul(0x9E37_79B9);
        let reference = map_indices_in_order(0..97, NonZeroUsize::new(1).unwrap(), || (), evaluate);
        for workers in [2_usize, 3, 8, 24] {
            let observed =
                map_indices_in_order(0..97, NonZeroUsize::new(workers).unwrap(), || (), evaluate);
            assert_eq!(observed, reference, "{workers} workers must agree");
        }
        assert_eq!(reference.len(), 97);
        assert_eq!(reference[3], 3_u64.wrapping_mul(0x9E37_79B9));
    }

    #[test]
    fn test_map_indices_in_order_builds_one_state_per_worker() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let factory_calls = AtomicUsize::new(0);
        let outcomes = map_indices_in_order(
            0..20,
            NonZeroUsize::new(4).unwrap(),
            || {
                factory_calls.fetch_add(1, Ordering::Relaxed);
                0_u64
            },
            |index, seen| {
                *seen += 1;
                index
            },
        );
        assert_eq!(outcomes, (0..20).collect::<Vec<_>>());
        assert_eq!(factory_calls.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn test_per_worker_state_factory_runs_once_per_worker() {
        use std::sync::atomic::{AtomicUsize, Ordering};
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

    fn measure_frame_word_draw(
        rate: gf2_coding::CodeRate,
        modulation: gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation,
    ) -> u128 {
        use crate::frame_sim::DvbT2BicmFrameSim;
        use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
        use gf2_coding::modem::DemapMethod;

        // The noise draw count does not depend on SNR.
        let sim = DvbT2BicmFrameSim::new(
            rate,
            modulation,
            12.0,
            DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            DemapMethod::ExactLogMap,
        );
        let mut ctx = WorkerCtx::new(1, 0, 0);
        ctx.reseek_to_frame(0);
        let start = ctx.current_word_pos();
        let _ = sim.simulate_frame(0, &mut ctx);
        ctx.current_word_pos() - start
    }

    #[test]
    fn test_worst_case_frame_draw_under_stride() {
        use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
        use gf2_coding::CodeRate;

        let mods = [
            ("QPSK", DvbT2Modulation::Qpsk),
            ("16-QAM", DvbT2Modulation::Qam16),
            ("64-QAM", DvbT2Modulation::Qam64),
        ];
        let draws: Vec<(&str, u128)> = mods
            .iter()
            .map(|&(name, m)| (name, measure_frame_word_draw(CodeRate::Rate1_2, m)))
            .collect();

        for (name, d) in &draws {
            eprintln!(
                "per-frame ChaCha20 32-bit-word draw: {name} Normal = {d} \
                 (headroom {:.2}x)",
                FRAME_STRIDE as f64 / *d as f64
            );
        }

        let qpsk = draws[0].1;
        let qam16 = draws[1].1;
        let qam64 = draws[2].1;

        assert!(
            qpsk > qam16 && qam16 > qam64,
            "expected draw order QPSK > 16-QAM > 64-QAM (fewer bits/symbol ⇒ \
             more symbols); got qpsk={qpsk} 16qam={qam16} 64qam={qam64}"
        );

        let worst = draws.iter().map(|&(_, d)| d).max().expect("non-empty");
        assert_eq!(worst, qpsk, "QPSK must be the maximum draw");

        // 64800 noise samples × 4 words + ~1008 BBFRAME words ≈ 260 208.
        assert!(
            (250_000..=270_000).contains(&qpsk),
            "QPSK Normal per-frame draw {qpsk} outside the expected ~260 208-word \
             band; design-doc §3 arithmetic needs revisiting"
        );

        let budget = FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN;
        assert!(
            worst < budget,
            "worst-case per-frame draw {worst} (QPSK) must be < FRAME_STRIDE - \
             margin = {budget} (FRAME_STRIDE = {FRAME_STRIDE}); raise FRAME_STRIDE"
        );

        assert!(
            (worst as f64) * 3.0 <= FRAME_STRIDE as f64,
            "headroom factor < 3x: worst draw {worst} vs FRAME_STRIDE {FRAME_STRIDE}"
        );
    }
}
