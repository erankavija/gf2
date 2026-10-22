//! The pooled batch decoder over an iterative LDPC batch decoder: the shared
//! batch contract, equality with the inner decoder's own results for several
//! worker counts and task sizes, and the partition each task decodes.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderConfig, LaneBackend, LdpcBatchDecoder, LdpcCode, LdpcDecoder,
    LdpcEncoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::pooled::PooledBatchDecoder;
use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
use gf2_coding::traits::{
    BatchSoftDecoder, BlockEncoder, DecodeOutcome, IterativeBatchDecoder, IterativeSoftDecoder,
};
use gf2_coding::tuning::DecodeSelectors;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;

const SEED: u64 = 0x1d4b_05d5;
const CAP: usize = 20;
const GROUP: usize = 3;
const SIGMAS: [f64; 12] = [
    0.45, 0.9, 1.6, 0.8, 0.5, 1.4, 0.85, 0.95, 0.4, 1.8, 0.75, 1.0,
];

fn code() -> LdpcCode {
    let base = vec![
        vec![0, 3, 7, 1, 0, -1],
        vec![5, 0, 11, -1, 2, 9],
        vec![9, 13, 0, 6, -1, 4],
    ];
    LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16))
}

fn frames() -> Vec<Vec<Llr>> {
    frames_of(SIGMAS.len())
}

/// `count` frames, the noise deviations cycling through [`SIGMAS`].
fn frames_of(count: usize) -> Vec<Vec<Llr>> {
    let encoder = LdpcEncoder::new(code());
    let mut rng = StdRng::seed_from_u64(SEED);
    (0..count)
        .map(|f| SIGMAS[f % SIGMAS.len()])
        .map(|sigma| {
            let codeword = encoder.encode(&BitVec::random(encoder.k(), &mut rng));
            let channel = AwgnChannel::from_variance(sigma * sigma);
            let scale = 2.0 / channel.variance();
            (0..codeword.len())
                .map(|i| {
                    let symbol = if codeword.get(i) { -1.0 } else { 1.0 };
                    Llr::new((scale * (symbol + channel.transmit(0.0, &mut rng))) as f32)
                })
                .collect()
        })
        .collect()
}

/// Batch sizes the inner decoder is called with, shared across workers.
type Sizes = Arc<Mutex<Vec<usize>>>;

/// An iterative batch decoder that reports granularity [`GROUP`] and records
/// the size of each batch it decodes.
struct Grouped {
    inner: IterativeBatchDecoder<LdpcDecoder>,
    sizes: Sizes,
}

impl BatchSoftDecoder for Grouped {
    fn frame_len(&self) -> usize {
        self.inner.frame_len()
    }

    fn decoded_len(&self) -> usize {
        self.inner.decoded_len()
    }

    fn batch_granularity(&self) -> usize {
        GROUP
    }

    fn decode_batch(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
    ) {
        self.sizes.lock().unwrap().push(frames.len());
        self.inner.decode_batch(frames, decoded, outcomes);
    }
}

fn grouped(sizes: &Sizes) -> Grouped {
    Grouped {
        inner: IterativeBatchDecoder::new(LdpcDecoder::new(code()), CAP),
        sizes: Arc::clone(sizes),
    }
}

fn selectors(granules_per_task: usize) -> DecodeSelectors {
    DecodeSelectors::try_new(0, 0, 1, granules_per_task).unwrap()
}

fn decode_all<D: BatchSoftDecoder>(
    decoder: &mut D,
    frames: &[Vec<Llr>],
) -> (Vec<BitVec>, Vec<DecodeOutcome>) {
    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
    let mut decoded = vec![BitVec::new(); frames.len()];
    let mut outcomes = vec![DecodeOutcome::default(); frames.len()];
    decoder.decode_batch(&views, &mut decoded, &mut outcomes);
    (decoded, outcomes)
}

fn assert_equals_inner(pool: &mut PooledBatchDecoder<Grouped>, frames: &[Vec<Llr>]) {
    let mut single = grouped(&Sizes::default());
    assert_eq!(decode_all(pool, frames), decode_all(&mut single, frames));
}

#[test]
fn pool_satisfies_the_batch_contract() {
    let frames = frames();
    let mut reference = LdpcDecoder::new(code());
    for granules in [1, 2, 5] {
        let sizes = Sizes::default();
        let mut pool = PooledBatchDecoder::with_selectors(|| grouped(&sizes), &selectors(granules));
        batch_soft_decoder_contract(
            &mut pool,
            |llrs| reference.decode_iterative(llrs, CAP),
            &frames,
            ConvergenceMix::EarlyLateAndNonConverging,
        );
    }
}

#[test]
fn results_equal_the_inner_decoder_under_the_default_pool() {
    let frames = frames();
    for granules in [1, 2, 3, 5] {
        let sizes = Sizes::default();
        let mut pool = PooledBatchDecoder::with_selectors(|| grouped(&sizes), &selectors(granules));
        assert_equals_inner(&mut pool, &frames[..11]);
        assert_equals_inner(&mut pool, &frames);
    }
}

#[cfg(feature = "parallel")]
#[test]
fn results_equal_the_inner_decoder_for_one_and_two_workers() {
    let frames = frames();
    for granules in [1, 2, 3, 5] {
        for workers in [1, 2] {
            let rayon_pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            rayon_pool.install(|| {
                let sizes = Sizes::default();
                let mut pool =
                    PooledBatchDecoder::with_selectors(|| grouped(&sizes), &selectors(granules));
                assert_eq!(pool.workers(), workers);
                assert_equals_inner(&mut pool, &frames[..11]);
                assert_equals_inner(&mut pool, &frames);
            });
        }
    }
}

#[test]
fn each_task_decodes_granules_per_task_groups_and_the_last_the_remainder() {
    let frames = frames();
    for granules in [1, 2, 3] {
        let sizes = Sizes::default();
        let mut pool = PooledBatchDecoder::with_selectors(|| grouped(&sizes), &selectors(granules));
        sizes.lock().unwrap().clear();
        decode_all(&mut pool, &frames[..11]);
        let task = granules * GROUP;
        let mut seen = sizes.lock().unwrap().clone();
        seen.sort_unstable();
        let mut expected: Vec<usize> = (0..11usize.div_ceil(task))
            .map(|t| task.min(11 - t * task))
            .collect();
        expected.sort_unstable();
        assert_eq!(seen, expected, "granules_per_task {granules}");
    }
}

#[test]
fn an_empty_batch_decodes_nothing() {
    let sizes = Sizes::default();
    let mut pool = PooledBatchDecoder::with_selectors(|| grouped(&sizes), &selectors(2));
    assert_eq!(decode_all(&mut pool, &[]), (vec![], vec![]));
}

#[test]
#[should_panic(expected = "one decoded word per frame")]
fn a_decoded_slice_of_another_length_panics() {
    let sizes = Sizes::default();
    let mut pool = PooledBatchDecoder::with_selectors(|| grouped(&sizes), &selectors(2));
    let frames = frames();
    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
    let mut outcomes = vec![DecodeOutcome::default(); views.len()];
    pool.decode_batch(&views, &mut [], &mut outcomes);
}

/// Wave-batching LDPC decoder, granularity eight, with the lane route open.
fn lane_decoder() -> LdpcBatchDecoder {
    LdpcBatchDecoder::with_selectors(
        code(),
        DecoderConfig::default(),
        CAP,
        DecodedWord::Codeword,
        DecodeSelectors::try_new(1, usize::MAX, 24, 1).unwrap(),
    )
}

#[test]
fn pooled_ldpc_batch_decoder_satisfies_the_contract_and_equals_the_single_decoder() {
    let frames = frames_of(21);
    assert_eq!(
        lane_decoder().batch_route(21),
        BatchRoute::Lane(LaneBackend::detected())
    );
    let mut reference = LdpcDecoder::new(code());
    for granules in [1, 2] {
        let mut pool = PooledBatchDecoder::with_selectors(lane_decoder, &selectors(granules));
        assert_eq!(pool.batch_granularity(), 8);
        assert_eq!(pool.task_frames(), 8 * granules);
        batch_soft_decoder_contract(
            &mut pool,
            |llrs| reference.decode_to_codeword(llrs, CAP),
            &frames,
            ConvergenceMix::EarlyLateAndNonConverging,
        );
    }
}

#[derive(Clone)]
struct WorkerContext {
    seen: Arc<Mutex<Vec<NonZeroUsize>>>,
}

impl BatchSoftDecoder for WorkerContext {
    fn frame_len(&self) -> usize {
        0
    }
    fn decoded_len(&self) -> usize {
        0
    }
    fn batch_granularity(&self) -> usize {
        1
    }
    fn decode_batch(
        &mut self,
        _frames: &[&[Llr]],
        _decoded: &mut [BitVec],
        _outcomes: &mut [DecodeOutcome],
    ) {
        panic!("pool must forward the worker context")
    }
    fn decode_batch_with_workers(
        &mut self,
        _frames: &[&[Llr]],
        _decoded: &mut [BitVec],
        _outcomes: &mut [DecodeOutcome],
        workers: NonZeroUsize,
    ) {
        self.seen.lock().unwrap().push(workers);
    }
}

#[test]
fn pooled_worker_context_uses_the_execution_pool_and_preserves_larger_explicit_widths() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let factory = || WorkerContext { seen: seen.clone() };
    let mut pooled = PooledBatchDecoder::with_selectors(factory, &selectors(1));
    let mut run = |expected: usize, explicit: NonZeroUsize| {
        seen.lock().unwrap().clear();
        let mut decoded: [BitVec; 5] = Default::default();
        pooled.decode_batch_with_workers(
            &[&[] as &[Llr]; 5],
            &mut decoded,
            &mut [DecodeOutcome::default(); 5],
            explicit,
        );
        let seen = seen.lock().unwrap();
        assert!(!seen.is_empty());
        assert!(
            seen.iter().all(|workers| workers.get() == expected),
            "{seen:?}"
        );
    };
    #[cfg(feature = "parallel")]
    {
        for width in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(width)
                .build()
                .unwrap();
            pool.install(|| run(width, NonZeroUsize::MIN));
            pool.install(|| run(12, NonZeroUsize::new(12).unwrap()));
        }
        run(rayon::current_num_threads(), NonZeroUsize::MIN);
    }
    #[cfg(not(feature = "parallel"))]
    run(1, NonZeroUsize::MIN);
    run(24, NonZeroUsize::new(24).unwrap());
}
