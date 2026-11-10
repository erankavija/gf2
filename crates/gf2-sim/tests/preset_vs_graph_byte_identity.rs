//! For six DVB-T2 MODCODs (`rate ∈ {1/2, 2/3, 3/4}` × `{16-QAM, 64-QAM}`) the
//! typestate preset (`Pipeline::dvb_t2`) and a hand-wired `Chain` produce
//! byte-identical `frames` / `errors` / `fer` / `mean_iters` when driven
//! through `TopologyExecutor::run_dvb_t2_snr_point` at a fixed seed. Every leg
//! runs at a waterfall Es/N0 where `0 < errors < frames`.
//!
//! The 5G NR preset (`Pipeline::nr_5g`) and its hand-wired `Chain` have no
//! sweep entry point: one batch of seeded messages is folded through each
//! pipeline's stages for the decoded frames and the same four columns, and
//! run through `TopologyExecutor::run` at one worker and at the pool size.

mod common;

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_coding::ldpc::nr_5g::max_payload_for_lifting;
use gf2_core::BitVec;

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch};
use gf2_sim::channels::es_n0_db_to_n0;
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::presets::nr_5g::{self, BaseGraph, Nr5gDecoderConfig, Nr5gRate, NrModulation};
use gf2_sim::stage::{AnyScratch, TypedBatch};
use gf2_sim::stages::batch_decode::BatchDecodeScratch;
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

/// The per-MODCOD Es/N0 points are calibrated at this seed.
const SEED: u64 = 0xDE16_0FC5;

const SMOKE_FRAMES: usize = 2;

const SLOW_FRAMES: usize = 50;

const PARALLELISM: usize = 24;

struct ModcodPoint {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    /// Waterfall Es/N0 in dB at [`SEED`].
    es_n0_db: f32,
    label: &'static str,
}

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

fn build_preset(rate: CodeRate, modulation: DvbT2Modulation, es_n0_db: f32) -> Pipeline {
    Pipeline::dvb_t2()
        .modcod(Modcod::Normal { rate, modulation })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(es_n0_db))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(PARALLELISM).expect("24 is non-zero"))
        .build()
        .expect("in-scope MODCOD builds via the preset")
}

fn build_graph(rate: CodeRate, modulation: DvbT2Modulation, es_n0_db: f32) -> Pipeline {
    common::build_dvb_t2_graph_chain(
        rate,
        modulation,
        decoder_config(),
        DemapMethod::ExactLogMap,
        es_n0_db,
        es_n0_db_to_n0(es_n0_db),
        SEED,
        NonZeroUsize::new(PARALLELISM).expect("24 is non-zero"),
    )
}

/// The two arms run concurrently: each builds its own pipeline and shares no
/// state with the other.
fn assert_byte_identical(point: &ModcodPoint, max_frames: usize) {
    let ModcodPoint {
        rate,
        modulation,
        es_n0_db,
        label,
    } = *point;

    let (preset_c, graph_c) = std::thread::scope(|s| {
        let preset_arm = s.spawn(move || {
            let pipeline = build_preset(rate, modulation, es_n0_db);
            let scheduler = Scheduler::from_pipeline(&pipeline);
            TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, max_frames)
                .expect("stage-driven preset sweep runs")
        });
        let graph_arm = s.spawn(move || {
            let pipeline = build_graph(rate, modulation, es_n0_db);
            let scheduler = Scheduler::from_pipeline(&pipeline);
            TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, max_frames)
                .expect("stage-driven graph sweep runs")
        });
        let preset_c: WorkerCounters = preset_arm.join().expect("preset arm thread");
        let graph_c: WorkerCounters = graph_arm.join().expect("graph arm thread");
        (preset_c, graph_c)
    });

    assert_eq!(
        preset_c.frames, max_frames as u64,
        "{label}: preset ran {}/{max_frames} frames",
        preset_c.frames
    );
    assert_eq!(
        graph_c.frames, max_frames as u64,
        "{label}: graph ran {}/{max_frames} frames",
        graph_c.frames
    );

    // Without a mixed verdict the `errors`/`fer` columns compare 0 == 0.
    assert!(
        preset_c.errors > 0 && preset_c.errors < preset_c.frames,
        "{label}: expected a mixed decode-success/failure sweep at the waterfall, got \
         {}/{} errored frames (re-pin Es/N0 if the chain changes)",
        preset_c.errors,
        preset_c.frames
    );

    common::assert_four_columns_byte_identical(
        &preset_c,
        &graph_c,
        &format!("(actual=preset, baseline=graph) {label}"),
    );

    eprintln!(
        "{label}: frames={} errors={} fer={:.6} mean_iters={:.6} (preset==graph, non-vacuous)",
        preset_c.frames,
        preset_c.errors,
        preset_c.fer(),
        preset_c.mean_iters(),
    );
}

#[test]
fn test_preset_vs_graph_smoke_r12_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 5.9,
            label: "smoke r1/2 16-QAM @5.9dB",
        },
        SMOKE_FRAMES,
    );
}

#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r1/2 16-QAM waterfall"]
fn test_preset_vs_graph_50f_r12_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 6.0,
            label: "50f r1/2 16-QAM @6.0dB",
        },
        SLOW_FRAMES,
    );
}

#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r1/2 64-QAM waterfall"]
fn test_preset_vs_graph_50f_r12_64qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam64,
            es_n0_db: 10.3,
            label: "50f r1/2 64-QAM @10.3dB",
        },
        SLOW_FRAMES,
    );
}

#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r2/3 16-QAM waterfall"]
fn test_preset_vs_graph_50f_r23_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate2_3,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 8.8,
            label: "50f r2/3 16-QAM @8.8dB",
        },
        SLOW_FRAMES,
    );
}

#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r2/3 64-QAM waterfall"]
fn test_preset_vs_graph_50f_r23_64qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate2_3,
            modulation: DvbT2Modulation::Qam64,
            es_n0_db: 13.8,
            label: "50f r2/3 64-QAM @13.8dB",
        },
        SLOW_FRAMES,
    );
}

#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r3/4 16-QAM waterfall"]
fn test_preset_vs_graph_50f_r34_16qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate3_4,
            modulation: DvbT2Modulation::Qam16,
            es_n0_db: 10.0,
            label: "50f r3/4 16-QAM @10.0dB",
        },
        SLOW_FRAMES,
    );
}

#[test]
#[ignore = "sim: 50-frame preset-vs-graph byte-identity, r3/4 64-QAM waterfall"]
fn test_preset_vs_graph_50f_r34_64qam() {
    assert_byte_identical(
        &ModcodPoint {
            rate: CodeRate::Rate3_4,
            modulation: DvbT2Modulation::Qam64,
            es_n0_db: 15.4,
            label: "50f r3/4 64-QAM @15.4dB",
        },
        SLOW_FRAMES,
    );
}

const NR_MAX_ITERATIONS: usize = 25;

/// Not a multiple of the eight-lane decode wave, so the batch ends in a
/// partial wave.
const NR_SMOKE_FRAMES: usize = 20;

/// The largest Z = 384 QPSK batch whose noise draw fits the frame stride the
/// AWGN stage asserts per call in debug builds.
const NR_SLOW_FRAMES: usize = 15;

/// A rate-1/2 5G NR point at the full payload of its lifting size.
#[derive(Clone, Copy)]
struct NrPoint {
    base_graph: BaseGraph,
    z: usize,
    modulation: NrModulation,
    /// Waterfall Es/N0 in dB at [`SEED`].
    es_n0_db: f32,
    label: &'static str,
}

fn build_nr_preset(point: NrPoint, workers: usize) -> Pipeline {
    Pipeline::nr_5g()
        .base_graph(point.base_graph)
        .lifting_size(point.z)
        .rate(Nr5gRate::R1_2)
        .decoder(Nr5gDecoderConfig::normalized_min_sum(NR_MAX_ITERATIONS))
        .demap(point.modulation, DemapMethod::ExactLogMap)
        .channel(nr_5g::Channel::awgn(point.es_n0_db))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(workers).expect("worker count is non-zero"))
        .build()
        .expect("in-scope 5G NR tuple builds via the preset")
}

fn build_nr_graph(point: NrPoint, workers: usize) -> Pipeline {
    let target_k = max_payload_for_lifting(point.base_graph.number(), point.z);
    common::build_nr_5g_graph_chain(
        point.base_graph.number(),
        2 * target_k,
        target_k,
        point.modulation.bits_per_symbol(),
        DecoderAlgorithm::NormalizedMinSum(0.75),
        NR_MAX_ITERATIONS,
        DemapMethod::ExactLogMap,
        point.es_n0_db,
        es_n0_db_to_n0(point.es_n0_db),
        SEED,
        NonZeroUsize::new(workers).expect("worker count is non-zero"),
    )
}

fn nr_messages(point: NrPoint, frames: usize) -> Vec<BitVec> {
    let k = max_payload_for_lifting(point.base_graph.number(), point.z);
    (0..frames as u64)
        .map(|f| BitVec::random_seeded(k, SEED ^ f))
        .collect()
}

fn hard_frames(batch: &dyn TypedBatch) -> Vec<BitVec> {
    batch
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("the 5G NR chain ends in HardDecisionBatch")
        .frames
        .clone()
}

/// Folds `messages` as one batch through the stages of `pipeline` on default
/// scratches, as `TopologyExecutor::run` does, and returns the decoded frames
/// with their counters; the iteration counts come from the decode scratch.
fn fold_nr(pipeline: &Pipeline, messages: &[BitVec]) -> (Vec<BitVec>, WorkerCounters) {
    let mut batch: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(messages.to_vec()));
    let mut scratch: Box<dyn AnyScratch> = Box::new(());
    for stage in pipeline.stages() {
        scratch = stage.default_scratch();
        batch = stage
            .process_any(batch.as_ref(), scratch.as_mut())
            .expect("process_any succeeds in the 5G NR chain");
    }
    let decoded = hard_frames(batch.as_ref());
    // `as_mut` first: `Box<dyn AnyScratch>` is itself an `AnyScratch`.
    let iterations = &scratch
        .as_mut()
        .as_any_mut()
        .downcast_mut::<BatchDecodeScratch>()
        .expect("the 5G NR chain ends in a batch decode stage")
        .iterations;

    let mut counters = WorkerCounters::default();
    for ((sent, got), &iters) in messages.iter().zip(&decoded).zip(iterations) {
        let bit_errors = (0..sent.len())
            .filter(|&i| sent.get(i) != got.get(i))
            .count() as u64;
        counters.record_frame(bit_errors > 0, iters, sent.len() as u64, bit_errors);
    }
    (decoded, counters)
}

fn execute_nr(pipeline: &Pipeline, messages: &[BitVec]) -> Vec<BitVec> {
    let out = TopologyExecutor::run(
        pipeline,
        &Scheduler::from_pipeline(pipeline),
        Box::new(BitPackedBatch::new(messages.to_vec())),
    )
    .expect("the 5G NR chain runs to completion")
    .into_single()
    .expect("a linear chain has one sink");
    hard_frames(out.as_ref())
}

fn assert_nr_byte_identical(point: NrPoint, frames: usize) {
    let label = point.label;
    let messages = nr_messages(point, frames);
    let preset = build_nr_preset(point, PARALLELISM);
    let graph = build_nr_graph(point, PARALLELISM);

    let (preset_frames, preset_c) = fold_nr(&preset, &messages);
    let (graph_frames, graph_c) = fold_nr(&graph, &messages);

    assert_eq!(
        preset_c.frames, frames as u64,
        "{label}: preset decoded {}/{frames} frames",
        preset_c.frames
    );
    // Without a mixed verdict the `errors`/`fer` columns compare 0 == 0.
    assert!(
        preset_c.errors > 0 && preset_c.errors < preset_c.frames,
        "{label}: expected a mixed decode-success/failure batch at the waterfall, got \
         {}/{} errored frames (re-pin Es/N0 if the chain changes)",
        preset_c.errors,
        preset_c.frames
    );
    assert_eq!(
        preset_frames, graph_frames,
        "{label}: preset and graph decoded frames differ"
    );
    common::assert_four_columns_byte_identical(
        &preset_c,
        &graph_c,
        &format!("(actual=preset, baseline=graph) {label}"),
    );

    for workers in [1, PARALLELISM] {
        assert_eq!(
            execute_nr(&build_nr_preset(point, workers), &messages),
            preset_frames,
            "{label}: preset executor output at {workers} worker(s) differs from the fold"
        );
        assert_eq!(
            execute_nr(&build_nr_graph(point, workers), &messages),
            preset_frames,
            "{label}: graph executor output at {workers} worker(s) differs from the preset"
        );
    }

    eprintln!(
        "{label}: frames={} errors={} fer={:.6} mean_iters={:.6} (preset==graph, non-vacuous)",
        preset_c.frames,
        preset_c.errors,
        preset_c.fer(),
        preset_c.mean_iters(),
    );
}

#[test]
fn test_nr_preset_vs_graph_smoke_bg1_z16_16qam() {
    assert_nr_byte_identical(
        NrPoint {
            base_graph: BaseGraph::Bg1,
            z: 16,
            modulation: NrModulation::Qam16,
            es_n0_db: 6.5,
            label: "smoke NR BG1 Z=16 r1/2 16-QAM @6.5dB",
        },
        NR_SMOKE_FRAMES,
    );
}

#[test]
#[ignore = "sim: 15-frame NR preset-vs-graph byte-identity, BG1 Z=384 r1/2 QPSK waterfall"]
fn test_nr_preset_vs_graph_15f_bg1_z384_qpsk() {
    assert_nr_byte_identical(
        NrPoint {
            base_graph: BaseGraph::Bg1,
            z: 384,
            modulation: NrModulation::Qpsk,
            es_n0_db: 1.2,
            label: "15f NR BG1 Z=384 r1/2 QPSK @1.2dB",
        },
        NR_SLOW_FRAMES,
    );
}
