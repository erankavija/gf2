//! The CPU LDPC, 5G NR and DVB-T2 decode stages give byte-identical outputs
//! for one worker, two workers and the pool default, on the scheduling paths
//! below (a whole batch through `TopologyExecutor::run`, contiguous chunks on
//! the workers of a rayon pool, and the stage-driven DVB-T2 sweep, which
//! decodes one frame per call) and on every route of [`ldpc_batch_routes`]. A
//! decoder built from each stage's code, algorithm and selectors asserts that
//! the route a case names is the route it takes.

mod common;

use std::num::NonZeroUsize;
use std::sync::Arc;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::dvb_t2::concat::{DvbT2Concat, DvbT2ConcatBatchDecoder};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::nr_5g::{max_payload_for_lifting, Nr5gRateMatchedBatchDecoder};
use gf2_coding::ldpc::{BatchRoute, DecoderAlgorithm, DecoderConfig, LaneBackend, QuasiCyclicLdpc};
#[cfg(feature = "hip")]
use gf2_coding::ldpc::{DecodedWord, LdpcBatchDecoder};
use gf2_coding::modem::DemapMethod;
use gf2_coding::test_support::{ldpc_batch_route_of, ldpc_batch_routes};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::tuning::DecodeSelectors;
use gf2_coding::{CodeRate, Llr};
use gf2_core::BitVec;
use rayon::prelude::*;

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch};
use gf2_sim::channels::es_n0_db_to_n0;
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::stage::{erase, AnyStage, TypedBatch};
use gf2_sim::stages::nr_5g::Nr5gDecode;
use gf2_sim::stages::DvbT2Decode;
use gf2_sim::testutil::AwgnLlrSource;
use gf2_sim::{Scheduler, TopologyExecutor};

const SEED: u64 = 0xDE16_0FC5;

const WAVE: usize = 8;

/// Not a multiple of [`WAVE`], so a batch ends in a partial wave.
const LLR_FRAMES: usize = 19;

struct Route {
    selectors: DecodeSelectors,
    backend: LaneBackend,
    route: BatchRoute,
    label: String,
}

fn routes() -> Vec<Route> {
    ldpc_batch_routes()
        .into_iter()
        .map(|(selectors, backend, route)| Route {
            label: format!("{route:?} on {backend:?}"),
            selectors: DecodeSelectors::try_new(
                selectors.lane_min_frames(),
                selectors.lane_max_edges(),
                worker_counts().into_iter().max().unwrap(),
                selectors.granules_per_task().get(),
            )
            .unwrap()
            .with_qc_route(selectors.qc_route()),
            backend,
            route,
        })
        .collect()
}

fn worker_counts() -> [usize; 3] {
    [1, 2, rayon::current_num_threads()]
}

fn workers(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).expect("worker count is non-zero")
}

/// The decode stage as `route` builds it: the lane backend is the detected one
/// unless the route holds the portable kernels.
fn on_backend<S>(stage: S, route: &Route, portable: impl FnOnce(S) -> S) -> S {
    match route.backend {
        LaneBackend::Portable => portable(stage),
        LaneBackend::Avx2 => {
            assert_eq!(LaneBackend::detected(), LaneBackend::Avx2);
            stage
        }
    }
}

fn hard_frames(batch: &dyn TypedBatch) -> Vec<BitVec> {
    batch
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("the chain ends in HardDecisionBatch")
        .frames
        .clone()
}

fn decode_via_executor(stage: Box<dyn AnyStage>, llrs: &[Vec<Llr>], count: usize) -> Vec<BitVec> {
    let pipeline = common::linear_pipeline(vec![stage], SEED, workers(count));
    let out = TopologyExecutor::run(
        &pipeline,
        &Scheduler::from_pipeline(&pipeline),
        Box::new(LlrBatch::new(llrs.to_vec())),
    )
    .expect("the decode pipeline runs")
    .into_single()
    .expect("a one-stage pipeline has one sink");
    hard_frames(out.as_ref())
}

fn decode_via_chunks(stage: &dyn AnyStage, llrs: &[Vec<Llr>], count: usize) -> Vec<BitVec> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(count)
        .build()
        .expect("the worker pool builds");
    let chunk = llrs.len().div_ceil(count);
    let parts: Vec<Vec<BitVec>> = pool.install(|| {
        llrs.par_chunks(chunk)
            .map(|frames| {
                let mut scratch = stage.default_scratch();
                let out = stage
                    .process_any(&LlrBatch::new(frames.to_vec()), scratch.as_mut())
                    .expect("the chunk decodes");
                hard_frames(out.as_ref())
            })
            .collect()
    });
    parts.into_iter().flatten().collect()
}

fn conservative(routes: &[Route]) -> &Route {
    routes
        .iter()
        .find(|route| route.route == BatchRoute::SingleFrame)
        .expect("the routes include the conservative selectors")
}

/// Asserts the decoded frames equal the conservative route's at one worker on
/// every route, worker count and path. `full_wave` gives the route a decoder
/// of the stage's code, algorithm and selectors takes for a full wave. `sent`
/// are the words the frames decode to when the decode succeeds; the batch must
/// mix successes and failures, or every route would agree trivially.
fn assert_invariant(
    name: &str,
    code: &gf2_coding::ldpc::LdpcCode,
    build: impl Fn(&Route) -> Box<dyn AnyStage>,
    full_wave: impl Fn(&Route, NonZeroUsize) -> BatchRoute,
    llrs: &[Vec<Llr>],
    sent: &[BitVec],
) {
    let routes = routes();
    let reference = decode_via_executor(build(conservative(&routes)), llrs, 1);
    let exact = reference.iter().zip(sent).filter(|(r, s)| r == s).count();
    assert!(
        exact > 0 && exact < llrs.len(),
        "{name}: {exact} of {} frames decode exactly; the batch must mix outcomes",
        llrs.len()
    );
    for route in &routes {
        let stage = build(route);
        for count in worker_counts() {
            assert_eq!(
                full_wave(route, workers(count)),
                ldpc_batch_route_of(code, route.route),
                "{name}: {} takes another route at {count} workers",
                route.label
            );
            assert_eq!(
                decode_via_executor(build(route), llrs, count),
                reference,
                "{name}: executor, {count} workers, {}",
                route.label
            );
            assert_eq!(
                decode_via_chunks(stage.as_ref(), llrs, count),
                reference,
                "{name}: chunks, {count} workers, {}",
                route.label
            );
        }
    }
}

#[test]
fn dvb_t2_decode_is_worker_and_route_invariant() {
    const SIGMAS: [f64; 6] = [0.3, 0.6, 1.3, 0.55, 1.0, 0.66];
    let mut codec = DvbT2Concat::new(FrameSize::Short, CodeRate::Rate1_2).unwrap();
    codec.set_max_ldpc_iterations(12);
    let codec = Arc::new(codec);

    let mut source = AwgnLlrSource::new(0x391f_5419);
    let sent: Vec<BitVec> = (0..LLR_FRAMES)
        .map(|f| BitVec::random_seeded(codec.k_bch(), f as u64))
        .collect();
    let llrs: Vec<Vec<Llr>> = sent
        .iter()
        .enumerate()
        .map(|(f, bbframe)| source.frame_for_codeword(&codec.encode(bbframe), SIGMAS[f % 6]))
        .collect();

    assert_invariant(
        "DVB-T2 decode",
        &codec.ldpc_code(),
        |route| {
            let stage = DvbT2Decode::with_selectors(codec.clone(), route.selectors.clone());
            erase(on_backend(stage, route, DvbT2Decode::with_portable_lanes))
        },
        |route, workers| {
            let decoder = DvbT2ConcatBatchDecoder::with_selectors(&codec, route.selectors.clone());
            on_backend(decoder, route, DvbT2ConcatBatchDecoder::with_portable_lanes)
                .ldpc()
                .batch_route_with_workers(WAVE, workers)
        },
        &llrs,
        &sent,
    );
}

#[test]
fn nr_decode_is_worker_and_route_invariant() {
    const SIGMAS: [f64; 6] = [0.3, 0.7, 1.4, 0.65, 1.0, 0.75];
    let (z, max_iterations) = (16, 25);
    let k = max_payload_for_lifting(1, z);
    let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, 2 * k, k));

    let mut source = AwgnLlrSource::new(0x9e0c_d346);
    let sent: Vec<BitVec> = (0..LLR_FRAMES)
        .map(|f| BitVec::random_seeded(k, f as u64))
        .collect();
    let llrs: Vec<Vec<Llr>> = sent
        .iter()
        .enumerate()
        .map(|(f, message)| source.frame_for_codeword(&code.encode(message), SIGMAS[f % 6]))
        .collect();

    assert_invariant(
        "NR decode",
        code.mother_code(),
        |route| {
            let stage = Nr5gDecode::with_selectors(
                code.clone(),
                DecoderAlgorithm::NormalizedMinSum(0.75),
                max_iterations,
                route.selectors.clone(),
            );
            erase(on_backend(stage, route, Nr5gDecode::with_portable_lanes))
        },
        |route, workers| {
            let decoder = Nr5gRateMatchedBatchDecoder::with_selectors(
                (*code).clone(),
                DecoderAlgorithm::NormalizedMinSum(0.75),
                max_iterations,
                route.selectors.clone(),
            );
            on_backend(
                decoder,
                route,
                Nr5gRateMatchedBatchDecoder::with_portable_lanes,
            )
            .batch_route_with_workers(WAVE, workers)
        },
        &llrs,
        &sent,
    );
}

#[cfg(feature = "hip")]
#[test]
fn cpu_ldpc_decode_is_worker_and_route_invariant() {
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_sim::gpu::ldpc_bp::CpuLdpcBp;

    const SIGMAS: [f64; 6] = [0.3, 0.6, 1.2, 0.55, 0.9, 0.7];
    let base = vec![
        vec![0, 3, 7, 1, 0, -1],
        vec![5, 0, 11, -1, 2, 9],
        vec![9, 13, 0, 6, -1, 4],
    ];
    let code = gf2_coding::ldpc::LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16));
    let encoder = LdpcEncoder::new(code.clone());
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);

    let mut source = AwgnLlrSource::new(0x86d9_260e);
    let sent: Vec<BitVec> = (0..LLR_FRAMES)
        .map(|f| encoder.encode(&BitVec::random_seeded(encoder.k(), f as u64)))
        .collect();
    let llrs: Vec<Vec<Llr>> = sent
        .iter()
        .enumerate()
        .map(|(f, codeword)| source.frame_for_codeword(codeword, SIGMAS[f % 6]))
        .collect();

    assert_invariant(
        "CPU LDPC decode",
        &code,
        |route| {
            let stage =
                CpuLdpcBp::with_selectors(code.clone(), config, 20, route.selectors.clone());
            erase(on_backend(stage, route, CpuLdpcBp::with_portable_lanes))
        },
        |route, workers| {
            let decoder = LdpcBatchDecoder::with_selectors(
                code.clone(),
                config,
                20,
                DecodedWord::Codeword,
                route.selectors.clone(),
            );
            on_backend(decoder, route, LdpcBatchDecoder::with_portable_lanes)
                .batch_route_with_workers(WAVE, workers)
        },
        &llrs,
        &sent,
    );
}

#[test]
fn nr_chain_is_worker_and_route_invariant() {
    let (base_graph, z, es_n0_db) = (1_u8, 16, 6.5_f32);
    let k = max_payload_for_lifting(base_graph, z);
    let messages: Vec<BitVec> = (0..LLR_FRAMES as u64)
        .map(|f| BitVec::random_seeded(k, SEED ^ f))
        .collect();

    let run = |route: &Route, count: usize| {
        let pipeline = common::build_nr_5g_graph_chain_with_decode(
            base_graph,
            2 * k,
            k,
            4,
            DemapMethod::ExactLogMap,
            es_n0_db,
            es_n0_db_to_n0(es_n0_db),
            SEED,
            workers(count),
            |code| {
                let decoder = Nr5gRateMatchedBatchDecoder::with_selectors(
                    (*code).clone(),
                    DecoderAlgorithm::NormalizedMinSum(0.75),
                    25,
                    route.selectors.clone(),
                );
                let decoder = on_backend(
                    decoder,
                    route,
                    Nr5gRateMatchedBatchDecoder::with_portable_lanes,
                );
                assert_eq!(
                    decoder.batch_route_with_workers(WAVE, workers(count)),
                    route.route,
                    "{}",
                    route.label
                );
                let stage = Nr5gDecode::with_selectors(
                    code,
                    DecoderAlgorithm::NormalizedMinSum(0.75),
                    25,
                    route.selectors.clone(),
                );
                on_backend(stage, route, Nr5gDecode::with_portable_lanes)
            },
        );
        let out = TopologyExecutor::run(
            &pipeline,
            &Scheduler::from_pipeline(&pipeline),
            Box::new(BitPackedBatch::new(messages.clone())),
        )
        .expect("the 5G NR chain runs")
        .into_single()
        .expect("a linear chain has one sink");
        hard_frames(out.as_ref())
    };

    let routes = routes();
    let reference = run(conservative(&routes), 1);
    let exact = reference
        .iter()
        .zip(&messages)
        .filter(|(r, s)| r == s)
        .count();
    assert!(
        exact > 0 && exact < messages.len(),
        "{exact} of {} frames decode exactly; the batch must mix outcomes",
        messages.len()
    );
    for route in &routes {
        for count in worker_counts() {
            assert_eq!(
                run(route, count),
                reference,
                "NR chain, {count} workers, {}",
                route.label
            );
        }
    }
}

/// The sweep on every route whose one-frame decode takes `target` equals the
/// sweep on the conservative selectors at one worker.
fn assert_sweep_invariant(target: BatchRoute) {
    const FRAMES: usize = 4;
    let (rate, modulation, es_n0_db) = (CodeRate::Rate1_2, DvbT2Modulation::Qam16, 6.4_f32);
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);

    let one_frame = |codec: &DvbT2Concat, route: &Route, workers| {
        let decoder = DvbT2ConcatBatchDecoder::with_selectors(codec, route.selectors.clone());
        on_backend(decoder, route, DvbT2ConcatBatchDecoder::with_portable_lanes)
            .ldpc()
            .batch_route_with_workers(1, workers)
    };
    let run = |route: &Route, taken: BatchRoute, count: usize| -> WorkerCounters {
        let pipeline = common::build_dvb_t2_graph_chain_with_decode(
            rate,
            modulation,
            config,
            DemapMethod::ExactLogMap,
            es_n0_db,
            es_n0_db_to_n0(es_n0_db),
            SEED,
            workers(count),
            |codec| {
                assert_eq!(
                    one_frame(&codec, route, workers(count)),
                    taken,
                    "{}",
                    route.label
                );
                let stage = DvbT2Decode::with_selectors(codec, route.selectors.clone());
                on_backend(stage, route, DvbT2Decode::with_portable_lanes)
            },
        );
        TopologyExecutor::run_dvb_t2_snr_point(
            &pipeline,
            &Scheduler::from_pipeline(&pipeline),
            0,
            FRAMES,
        )
        .expect("the stage-driven sweep runs")
    };

    let routes = routes();
    let mut probe = DvbT2Concat::new(FrameSize::Normal, rate).unwrap();
    probe.set_decoder_config(config);
    let selected: Vec<&Route> = routes
        .iter()
        .filter(|route| one_frame(&probe, route, NonZeroUsize::MIN) == target)
        .collect();
    assert!(
        !selected.is_empty() || routes.iter().all(|route| route.route != target),
        "no route takes {target:?} for a one-frame decode"
    );
    let reference = run(conservative(&routes), BatchRoute::SingleFrame, 1);
    assert_eq!(reference.frames, FRAMES as u64);
    assert!(
        reference.errors > 0 && reference.errors < reference.frames,
        "{} of {} frames errored; the sweep must mix outcomes",
        reference.errors,
        reference.frames
    );
    for route in selected {
        for count in worker_counts() {
            common::assert_four_columns_byte_identical(
                &run(route, target, count),
                &reference,
                &format!("DVB-T2 sweep, {count} workers, {}", route.label),
            );
        }
    }
}

#[test]
fn dvb_t2_sweep_is_worker_invariant_on_conservative_selectors() {
    assert_sweep_invariant(BatchRoute::SingleFrame);
}

#[test]
fn dvb_t2_sweep_is_worker_invariant_on_portable_lanes() {
    assert_sweep_invariant(BatchRoute::Lane(LaneBackend::Portable));
}

#[test]
fn dvb_t2_sweep_is_worker_invariant_on_avx2_lanes() {
    assert_sweep_invariant(BatchRoute::Lane(LaneBackend::Avx2));
}

mod worker_context {
    //! Decode concurrency follows execution, including reuse of stage scratch.

    use std::num::NonZeroUsize;
    use std::sync::{Arc, Mutex};

    use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
    use gf2_coding::Llr;
    use gf2_core::BitVec;
    use gf2_sim::batch::LlrBatch;
    use gf2_sim::bler_sweep::slice_errors;
    use gf2_sim::stage::Stage;
    use gf2_sim::stages::batch_decode::{BatchDecode, BatchDecodeScratch};

    struct RecordingDecoder(Arc<Mutex<Vec<usize>>>);

    impl BatchSoftDecoder for RecordingDecoder {
        fn frame_len(&self) -> usize {
            3
        }

        fn decoded_len(&self) -> usize {
            3
        }

        fn batch_granularity(&self) -> usize {
            1
        }

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
            self.0.lock().unwrap().push(workers.get());
            for ((frame, word), outcome) in frames.iter().zip(decoded).zip(outcomes) {
                *word = BitVec::new();
                for llr in *frame {
                    word.push_bit(llr.hard_decision());
                }
                *outcome = DecodeOutcome::default();
            }
        }
    }

    #[test]
    fn stage_and_sweep_pass_execution_workers_when_decoders_are_reused() {
        let stage_calls = Arc::new(Mutex::new(Vec::new()));
        let sweep_calls = Arc::new(Mutex::new(Vec::new()));
        let factories = Arc::new(Mutex::new(0));
        let stage = BatchDecode::new({
            let calls = stage_calls.clone();
            let factories = factories.clone();
            move || {
                *factories.lock().unwrap() += 1;
                RecordingDecoder(calls.clone())
            }
        });
        let mut scratch = BatchDecodeScratch::default();
        let mut decoder = RecordingDecoder(sweep_calls.clone());
        let input = LlrBatch::new(vec![vec![Llr::new(1.0), Llr::new(-2.0), Llr::new(3.0)]]);
        assert!(rayon::current_thread_index().is_none());
        let expected = stage.process(&input, &mut scratch).unwrap().frames;
        let expected_errors = slice_errors(&mut decoder, 42, 1.0, 5);

        for width in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(width)
                .build()
                .unwrap();
            pool.install(|| {
                assert_eq!(
                    stage.process(&input, &mut scratch).unwrap().frames,
                    expected
                );
                assert_eq!(slice_errors(&mut decoder, 42, 1.0, 5), expected_errors);
            });
        }
        assert_eq!(
            stage.process(&input, &mut scratch).unwrap().frames,
            expected
        );
        assert_eq!(slice_errors(&mut decoder, 42, 1.0, 5), expected_errors);
        assert_eq!(*stage_calls.lock().unwrap(), [1, 1, 2, 4, 1]);
        assert_eq!(*sweep_calls.lock().unwrap(), [1, 1, 2, 4, 1]);
        assert_eq!(*factories.lock().unwrap(), 1);
    }
}
