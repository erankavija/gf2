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
//! the 200-frame four-column (`fer`/`frames`/`errors`/`mean_iters`)
//! byte-identity across all six in-scope MODCODs.
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

use gf2_sim::channels::Awgn;
use gf2_sim::graph::Chain;
use gf2_sim::stage::erase;
use gf2_sim::stages::dvb_t2_bicm_stages;
use gf2_sim::{Pipeline, PipelineConfig, Scheduler, TopologyExecutor};

/// Derives the soft demapper's noise variance `N0 = 2*sigma^2` from an AWGN
/// channel's Es/N0 in dB.
///
/// Uses the same f64-computed, once-rounded arithmetic that the preset's
/// `Channel::demap_noise_var` uses (the SSOT formula), ensuring the graph-built
/// pipeline's demapper N0 is bit-identical to the typestate preset's.
///
/// # Arguments
///
/// * `es_n0_db` — channel Es/N0 in dB.
fn demap_noise_var(es_n0_db: f32) -> f32 {
    let es_n0_lin = 10.0_f64.powf(f64::from(es_n0_db) / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (2.0 * sigma_sq) as f32
}

fn main() {
    const FRAMES: usize = 8;
    const SEED: u64 = 0xC0DE_F00D;
    const ES_N0_DB: f32 = 6.5;

    let rate = CodeRate::Rate1_2;
    let modulation = DvbT2Modulation::Qam16;
    let decoder = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);

    // -----------------------------------------------------------------------
    // 1. Build the same chain by hand via the graph API.
    //
    //    dvb_t2_bicm_stages returns a factory with .forward and .inverse
    //    stage vecs and a .codec reference (for k_bch).  We add them in order,
    //    insert the AWGN channel between the forward and inverse halves, then
    //    connect every consecutive pair.
    // -----------------------------------------------------------------------
    let n0 = demap_noise_var(ES_N0_DB);
    let factory = dvb_t2_bicm_stages(rate, modulation, decoder, DemapMethod::ExactLogMap, n0);

    let mut chain = Chain::new();
    let mut ids = Vec::with_capacity(7);
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    // The channel stage occupies the SymbolBatch → SymbolBatch slot between
    // the forward and inverse halves.
    ids.push(chain.add(erase(Awgn::new(ES_N0_DB, modulation.bits_per_cell()))));
    for stage in factory.inverse {
        ids.push(chain.add(stage));
    }
    for pair in ids.windows(2) {
        chain
            .connect(pair[0], pair[1])
            .expect("each consecutive BICM hop is type-compatible");
    }

    // Mirror the PipelineConfig the preset would set, so the two pipelines
    // are config-equivalent (required by the byte-identity test).
    let config = PipelineConfig {
        seed: SEED,
        esn0_db_points: Vec::new(),
        target_errors: 0,
        max_frames: 0,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(1).expect("1 is non-zero"),
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
        "DVB-T2 r1/2 16-QAM Normal (graph API): {} stages, seed {:#010x}",
        pipeline.stage_count(),
        pipeline.config().seed,
    );
    assert_eq!(
        pipeline.stage_count(),
        7,
        "forward(3) + channel(1) + inverse(3) = 7 stages"
    );

    // -----------------------------------------------------------------------
    // 2. Run the chain through the public stage-driven executor entry point.
    //
    //    TopologyExecutor::run_dvb_t2_snr_point accepts any &Pipeline,
    //    including graph-built ones, and uses Scheduler::from_pipeline for
    //    the rayon pool.  The byte-identity to the typestate path is formally
    //    proved by tests/preset_vs_graph_byte_identity.rs.
    // -----------------------------------------------------------------------
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
