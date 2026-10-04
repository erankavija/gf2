//! Hand-wires the seven-stage DVB-T2 BICM chain (encode, interleave, map, AWGN,
//! demap, deinterleave, decode) through the graph API (`Chain::new` / `add` /
//! `connect` / `build`) and runs it at the r1/2 16-QAM waterfall point and seed
//! of `examples/dvb_t2_typestate.rs`.
//!
//! Run with: `cargo run -p gf2-sim --example dvb_t2_graph_api --release`

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::channels::{es_n0_db_to_n0, Awgn};
use gf2_sim::graph::Chain;
use gf2_sim::stage::erase;
use gf2_sim::stages::dvb_t2_bicm_stages;
use gf2_sim::{Pipeline, PipelineConfig, Scheduler, TopologyExecutor};

fn main() {
    const FRAMES: usize = 8;
    const SEED: u64 = 0xDE16_0FC5;
    const ES_N0_DB: f32 = 6.0;

    let rate = CodeRate::Rate1_2;
    let modulation = DvbT2Modulation::Qam16;
    let decoder = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);

    // `dvb_t2_bicm_stages` returns the forward and inverse stage vectors; the
    // AWGN channel goes between them.
    let n0 = es_n0_db_to_n0(ES_N0_DB);
    let factory = dvb_t2_bicm_stages(rate, modulation, decoder, DemapMethod::ExactLogMap, n0);

    let mut chain = Chain::new();
    let mut ids = Vec::with_capacity(7);
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    ids.push(chain.add(erase(Awgn::new(ES_N0_DB, modulation.bits_per_cell()))));
    for stage in factory.inverse {
        ids.push(chain.add(stage));
    }
    for pair in ids.windows(2) {
        chain
            .connect(pair[0], pair[1])
            .expect("each consecutive BICM hop is type-compatible");
    }

    let config = PipelineConfig {
        seed: SEED,
        esn0_db_points: Vec::new(),
        target_errors: 0,
        max_frames: 0,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(4).expect("4 is non-zero"),
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
    };

    let pipeline: Pipeline = chain
        .with_config(config)
        .build()
        .expect("the full BICM chain is a valid DAG");

    println!(
        "DVB-T2 r1/2 16-QAM Normal @ {ES_N0_DB} dB (waterfall, graph API): \
         {} stages, seed {:#010x}",
        pipeline.stage_count(),
        pipeline.config().seed,
    );

    let scheduler = Scheduler::from_pipeline(&pipeline);
    let counters = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, FRAMES)
        .expect("stage-driven sweep runs end-to-end");

    assert_eq!(counters.frames, FRAMES as u64, "all frames executed");

    println!(
        "{:<14} {:<8} {:<8} {:<10} {:<12}",
        "MODCOD", "frames", "errors", "FER", "mean_iters"
    );
    println!(
        "{:<14} {:<8} {:<8} {:<10.6} {:<12.3}",
        "r1/2 16-QAM",
        counters.frames,
        counters.errors,
        counters.fer(),
        counters.mean_iters(),
    );
}
