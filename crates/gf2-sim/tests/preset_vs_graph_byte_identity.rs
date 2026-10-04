//! For six DVB-T2 MODCODs (`rate ∈ {1/2, 2/3, 3/4}` × `{16-QAM, 64-QAM}`) the
//! typestate preset (`Pipeline::dvb_t2`) and a hand-wired `Chain` produce
//! byte-identical `frames` / `errors` / `fer` / `mean_iters` when driven
//! through `TopologyExecutor::run_dvb_t2_snr_point` at a fixed seed. Every leg
//! runs at a waterfall Es/N0 where `0 < errors < frames`.

mod common;

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::channels::es_n0_db_to_n0;
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
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
            es_n0_db: 6.0,
            label: "smoke r1/2 16-QAM @6.0dB",
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
