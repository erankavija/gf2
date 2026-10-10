//! The CPU LDPC, 5G NR and DVB-T2 decode stages give byte-identical outputs
//! for one worker, two workers and the pool default, on every scheduling path
//! below, and on every decode route of
//! [`ldpc_batch_routes`]: the conservative selectors, and the lane route on the
//! portable and on the AVX2 backend.
//!
//! Scheduling paths: a whole batch through `TopologyExecutor::run`; contiguous
//! chunks of a batch decoded by the workers of a rayon pool; and the
//! stage-driven DVB-T2 sweep, which decodes one frame per call on strided
//! workers.

mod common;

use std::num::NonZeroUsize;
use std::sync::Arc;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::nr_5g::max_payload_for_lifting;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LaneBackend, LdpcCode, QuasiCyclicLdpc};
use gf2_coding::modem::DemapMethod;
use gf2_coding::test_support::ldpc_batch_routes;
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

/// Not a multiple of the eight-lane decode wave, so a batch ends in a partial
/// wave.
const LLR_FRAMES: usize = 19;

/// A decode setup of [`ldpc_batch_routes`].
struct Route {
    selectors: DecodeSelectors,
    backend: LaneBackend,
    label: String,
}

fn routes() -> Vec<Route> {
    ldpc_batch_routes()
        .into_iter()
        .map(|(selectors, backend, route)| Route {
            label: format!("{route:?} on {backend:?}"),
            selectors,
            backend,
        })
        .collect()
}

/// One worker, two workers and the pool default.
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

/// `llrs` as one batch through a one-stage pipeline of `workers` workers.
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

/// `llrs` in contiguous chunks, one per worker of a pool of `count` workers,
/// each decoded on its own scratch.
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

/// Asserts the decoded frames equal one reference on every route, worker
/// count and path. `sent` are the words the frames decode to when the decode
/// succeeds; the batch must mix successes and failures, or every route would
/// agree trivially.
fn assert_invariant(
    name: &str,
    build: impl Fn(&Route) -> Box<dyn AnyStage>,
    llrs: &[Vec<Llr>],
    sent: &[BitVec],
) {
    let routes = routes();
    let reference = decode_via_executor(build(&routes[0]), llrs, 1);
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
        |route| {
            let stage = DvbT2Decode::with_selectors(codec.clone(), route.selectors.clone());
            erase(on_backend(stage, route, DvbT2Decode::with_portable_lanes))
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
        |route| {
            let stage = Nr5gDecode::with_selectors(
                code.clone(),
                DecoderAlgorithm::NormalizedMinSum(0.75),
                max_iterations,
                route.selectors.clone(),
            );
            erase(on_backend(stage, route, Nr5gDecode::with_portable_lanes))
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
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16));
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
        |route| {
            let stage =
                CpuLdpcBp::with_selectors(code.clone(), config, 20, route.selectors.clone());
            erase(on_backend(stage, route, CpuLdpcBp::with_portable_lanes))
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
    let reference = run(&routes[0], 1);
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

#[test]
fn dvb_t2_sweep_is_worker_and_route_invariant() {
    const FRAMES: usize = 4;
    let (rate, modulation, es_n0_db) = (CodeRate::Rate1_2, DvbT2Modulation::Qam16, 6.0_f32);
    let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);

    let run = |route: &Route, count: usize| -> WorkerCounters {
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
    let reference = run(&routes[0], 1);
    assert_eq!(reference.frames, FRAMES as u64);
    assert!(
        reference.errors > 0 && reference.errors < reference.frames,
        "{} of {} frames errored; the sweep must mix outcomes",
        reference.errors,
        reference.frames
    );
    for route in &routes {
        for count in worker_counts() {
            common::assert_four_columns_byte_identical(
                &run(route, count),
                &reference,
                &format!("DVB-T2 sweep, {count} workers, {}", route.label),
            );
        }
    }
}
