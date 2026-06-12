//! DVB-T2 quickstart: build a pipeline, run a short SNR sweep, print the summary.
//!
//! The shortest path from nothing to a frame-error-rate number. It builds the
//! production DVB-T2 BICM pipeline through the typestate preset
//! ([`Pipeline::dvb_t2`]), configures a single waterfall SNR point with a small
//! frame budget, drives the sweep with the convenience entry point
//! [`Pipeline::run`], and prints the per-point columns.
//!
//! For the same chain hand-wired through the low-level graph API, see
//! `examples/dvb_t2_graph_api.rs`; for the compile-time-checked builder order,
//! see `examples/dvb_t2_typestate.rs`; for a non-standard chain with a custom
//! stage, see `examples/novel_chain_via_graph.rs`.
//!
//! Runtime: ~5 s (24 frames of n = 64800 LDPC decode across the local CPU).
//!
//! Run with: `cargo run -p gf2-sim --example dvb_t2_quickstart --release`

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::Pipeline;

fn main() {
    // Build the seven-stage DVB-T2 BICM pipeline: r1/2 16-QAM Normal, sum-product
    // LDPC decode, exact log-MAP demap, AWGN at the 6.0 dB waterfall point.
    let mut pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(6.0))
        .seed(0xDE16_0FC5)
        .parallelism(NonZeroUsize::new(4).expect("4 is non-zero"))
        .build()
        .expect("r1/2 16-QAM Normal is an in-scope MODCOD");

    // Configure the sweep on the built pipeline's config, then run it.
    pipeline.config_mut().esn0_db_points = vec![6.0];
    pipeline.config_mut().max_frames = 24;
    let results = pipeline.run().expect("the DVB-T2 sweep runs end-to-end");

    println!("DVB-T2 r1/2 16-QAM Normal quickstart");
    println!("Es/N0(dB)  frames  errors  FER       mean_iters");
    for p in &results.per_point {
        println!(
            "{:<10.2} {:<7} {:<7} {:<9.6} {:<10.3}",
            p.es_n0_db, p.frames, p.errors, p.fer, p.mean_iters
        );
    }
}
