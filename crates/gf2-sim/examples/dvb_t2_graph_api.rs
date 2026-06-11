//! DVB-T2 BICM pipeline via the low-level graph API (`Chain`).
//!
//! Demonstrates how to hand-wire the complete DVB-T2 BICM chain — the same
//! seven stages the typestate builder produces — using the explicit
//! `Chain::new() / add() / connect() / build()` graph API:
//!
//! ```text
//! Chain::new()
//!     .add(DvbT2Encode)      // BitPackedBatch → BitPackedBatch
//!     .add(BitInterleave)    // BitPackedBatch → BitPackedBatch
//!     .add(GrayQamMap)       // BitPackedBatch → SymbolBatch
//!     .add(Awgn)             // SymbolBatch    → SymbolBatch  (channel)
//!     .add(GrayQamDemap)     // SymbolBatch    → LlrBatch
//!     .add(BitDeinterleave)  // LlrBatch       → LlrBatch
//!     .add(DvbT2Decode)      // LlrBatch       → HardDecisionBatch
//!     + connect each stage to the next (type-checked)
//!     .build()               // topological sort + validation → Pipeline
//! ```
//!
//! The resulting [`Pipeline`] is structurally and execution-wise identical to
//! one built through the typestate preset (see `examples/dvb_t2_typestate.rs`).
//! The structural + single-frame execution equivalence is formally proved by
//! `tests/preset_vs_graph.rs`; `tests/preset_vs_graph_byte_identity.rs` proves
//! the run-level four-column (`fer`/`frames`/`errors`/`mean_iters`)
//! byte-identity across all six in-scope MODCODs (50 waterfall frames per
//! MODCOD, per the AMENDMENT 2026-06-11 on issue 8c8302c8).
//!
//! Like the typestate example, this runs at the r1/2 16-QAM **waterfall**
//! Es/N0 (6.0 dB) and the same seed, so the simulation counters (`frames`,
//! `errors`, `fer`, `mean_iters`) are byte-identical to the typestate output
//! — the stdout formatting may differ between the two examples. The example
//! body exceeds 50 code lines because the `PipelineConfig` struct literal
//! (12 fields) and the graph wiring loop are unavoidably verbose; both are
//! reader-facing demonstrations of the graph API surface.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-sim --example dvb_t2_graph_api --release
//! ```

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

    // Build the chain via the graph API: dvb_t2_bicm_stages gives forward +
    // inverse stage vecs; we add them with the AWGN channel in between, then
    // connect consecutive pairs and build.
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
