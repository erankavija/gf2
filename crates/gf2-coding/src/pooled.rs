//! Parallel batch decoding over one inner decoder per worker.

use std::num::NonZeroUsize;
use std::sync::{Mutex, MutexGuard};

use gf2_core::BitVec;

use crate::llr::Llr;
use crate::traits::{BatchSoftDecoder, DecodeOutcome};
use crate::tuning::DecodeSelectors;

/// Batch decoder that spreads a batch over the workers of a rayon pool, each
/// on its own inner decoder.
///
/// # Worker model
///
/// The pool holds one inner decoder per worker of the rayon pool current at
/// construction, built once from the factory. [`decode_batch`] cuts a batch
/// into tasks of `granules_per_task` groups of the inner decoder's
/// [`batch_granularity`](BatchSoftDecoder::batch_granularity) frames, the last
/// task taking the remainder, and runs the tasks on the caller's pool; it
/// opens no pool of its own. A task decodes on the inner decoder of the worker
/// that runs it, and results land in input order. With inner decoders that
/// meet the bounds below, results do not depend on worker count or
/// scheduling; the integration test `pooled_batch_decoder` holds them to the
/// shared batch contract and to the inner decoder's own results under one
/// worker, two workers and the default pool.
///
/// A `decode_batch` under a pool of another width, or outside any pool, stays
/// correct: workers beyond the inner decoders share them by index, and a
/// shared decoder serialises its tasks. Each task passes the execution pool's
/// configured width to its inner decoder, preserving a larger explicit caller
/// width. Without the `parallel` feature a batch
/// decodes sequentially on the first inner decoder.
///
/// # Bounds and cost
///
/// The inner decoder is `Send`, keeps no state between calls that changes a
/// later result, decodes a batch of any length, and runs no rayon work on the
/// calling pool inside `decode_batch`: a worker holds its decoder's lock for
/// the whole task and could otherwise steal a task that locks it again. Every
/// decoder the factory builds gives the same result for the same frame; the
/// trait does not state this, and the pool does not check it. Each worker
/// owns a full inner decoder, so the workspace cost is that of one inner
/// decoder times the worker count.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "parallel")] {
/// use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, QuasiCyclicLdpc};
/// use gf2_coding::pooled::PooledBatchDecoder;
/// use gf2_coding::traits::{BatchSoftDecoder, IterativeBatchDecoder};
///
/// let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(vec![vec![0, 1, -1], vec![1, 0, 2]], 8));
/// let rayon_pool = rayon::ThreadPoolBuilder::new().num_threads(2).build().unwrap();
/// let pool = rayon_pool.install(|| {
///     PooledBatchDecoder::new(|| IterativeBatchDecoder::new(LdpcDecoder::new(code.clone()), 20))
/// });
/// assert_eq!(pool.batch_granularity(), 1);
/// # }
/// ```
///
/// [`decode_batch`]: BatchSoftDecoder::decode_batch
pub struct PooledBatchDecoder<D> {
    decoders: Vec<Mutex<D>>,
    frame_len: usize,
    decoded_len: usize,
    granularity: usize,
    task_frames: usize,
}

impl<D: BatchSoftDecoder + Send> PooledBatchDecoder<D> {
    /// Builds the pool with the decode selectors of the process-wide coding
    /// tuning section; see [`with_selectors`](Self::with_selectors).
    ///
    /// # Panics
    ///
    /// Panics as [`with_selectors`](Self::with_selectors) does, and as
    /// [`tuning::active`](crate::tuning::active) does.
    pub fn new(factory: impl FnMut() -> D) -> Self {
        Self::with_selectors(factory, crate::tuning::active().decode())
    }

    /// Builds the pool from `factory` with `selectors` giving
    /// `granules_per_task`.
    ///
    /// The factory runs once per worker of the current rayon pool, one
    /// without the `parallel` feature, and its decoders must agree in frame
    /// length, decoded length and granularity.
    ///
    /// # Panics
    ///
    /// Panics if the decoders disagree in shape, if the granularity is zero,
    /// or if a task size overflows `usize`. A panic of the factory or of an
    /// inner decoder's `frame_len`, `decoded_len` or `batch_granularity`
    /// propagates.
    pub fn with_selectors(mut factory: impl FnMut() -> D, selectors: &DecodeSelectors) -> Self {
        let decoders: Vec<D> = (0..worker_count()).map(|_| factory()).collect();
        let first = &decoders[0];
        let (frame_len, decoded_len, granularity) = (
            first.frame_len(),
            first.decoded_len(),
            first.batch_granularity(),
        );
        assert!(granularity >= 1, "a batch granularity is at least one");
        for decoder in &decoders {
            assert_eq!(
                (
                    decoder.frame_len(),
                    decoder.decoded_len(),
                    decoder.batch_granularity()
                ),
                (frame_len, decoded_len, granularity),
                "the factory builds decoders of one shape"
            );
        }
        let task_frames = granularity
            .checked_mul(selectors.granules_per_task().get())
            .expect("a task size fits usize");
        Self {
            decoders: decoders.into_iter().map(Mutex::new).collect(),
            frame_len,
            decoded_len,
            granularity,
            task_frames,
        }
    }

    /// Returns the number of inner decoders, one per worker of the pool the
    /// decoder was built under.
    pub fn workers(&self) -> usize {
        self.decoders.len()
    }

    /// Returns the number of frames one task decodes.
    pub fn task_frames(&self) -> usize {
        self.task_frames
    }
}

#[cfg(feature = "parallel")]
fn worker_count() -> usize {
    rayon::current_num_threads()
}

#[cfg(not(feature = "parallel"))]
fn worker_count() -> usize {
    1
}

fn lock<D>(decoder: &Mutex<D>) -> MutexGuard<'_, D> {
    decoder
        .lock()
        .expect("an inner decoder panicked in an earlier batch")
}

impl<D: BatchSoftDecoder + Send> BatchSoftDecoder for PooledBatchDecoder<D> {
    fn frame_len(&self) -> usize {
        self.frame_len
    }

    fn decoded_len(&self) -> usize {
        self.decoded_len
    }

    fn batch_granularity(&self) -> usize {
        self.granularity
    }

    /// Decodes tasks of [`task_frames`](PooledBatchDecoder::task_frames)
    /// frames in parallel under the `parallel` feature, and in order on the
    /// first inner decoder without it.
    ///
    /// # Panics
    ///
    /// Panics as [`BatchSoftDecoder::decode_batch`] does; a length mismatch of
    /// `decoded` or `outcomes` panics before any frame decodes. Panics if an
    /// inner decoder panicked in an earlier batch.
    fn decode_batch(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
    ) {
        self.decode_batch_with_workers(frames, decoded, outcomes, NonZeroUsize::MIN);
    }

    fn decode_batch_with_workers(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        workers: NonZeroUsize,
    ) {
        // This executor launches tasks on the current pool, including from outside
        // a Rayon worker; an outer executor can supply a larger concurrency.
        let workers = workers.max(NonZeroUsize::new(worker_count()).unwrap());
        assert_eq!(
            decoded.len(),
            frames.len(),
            "a batch needs one decoded word per frame"
        );
        assert_eq!(
            outcomes.len(),
            frames.len(),
            "a batch needs one outcome per frame"
        );
        let task = self.task_frames;
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            let decoders = &self.decoders;
            frames
                .par_chunks(task)
                .zip(decoded.par_chunks_mut(task))
                .zip(outcomes.par_chunks_mut(task))
                .for_each(|((frames, decoded), outcomes)| {
                    let worker = rayon::current_thread_index().unwrap_or(0) % decoders.len();
                    lock(&decoders[worker])
                        .decode_batch_with_workers(frames, decoded, outcomes, workers);
                });
        }
        #[cfg(not(feature = "parallel"))]
        {
            let mut decoder = lock(&self.decoders[0]);
            for ((frames, decoded), outcomes) in frames
                .chunks(task)
                .zip(decoded.chunks_mut(task))
                .zip(outcomes.chunks_mut(task))
            {
                decoder.decode_batch_with_workers(frames, decoded, outcomes, workers);
            }
        }
    }
}
