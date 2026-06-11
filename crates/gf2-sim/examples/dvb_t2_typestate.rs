//! DVB-T2 BICM pipeline via the typestate fluent builder (`Pipeline::dvb_t2`).
//!
//! Demonstrates how to wire the complete DVB-T2 BICM chain — BCH encode,
//! LDPC encode, bit interleave, Gray-QAM map, AWGN channel, soft demap,
//! bit deinterleave, LDPC decode, BCH decode — for the Normal r1/2 16-QAM
//! MODCOD using the typestate preset API:
//!
//! ```text
//! Pipeline::dvb_t2()
//!     .modcod(...)   // compile-time: selects MODCOD
//!     .decoder(...)  // compile-time: sets BP decoder config
//!     .demap(...)    // compile-time: sets demapper method
//!     .channel(...)  // compile-time: sets channel + derives demapper N0
//!     .seed(...)     // optional
//!     .build()       // produces a Pipeline, or a typed BuildError
//! ```
//!
//! The compiler enforces the call order: calling `.decoder()` before
//! `.modcod()` is a compile-time error (the method only exists on the
//! `NeedsDecoder` state, not on `NeedsModcod`).
//!
//! For a side-by-side comparison via the lower-level graph API see
//! `examples/dvb_t2_graph_api.rs`.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-sim --example dvb_t2_typestate --release
//! ```

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

fn main() {
    // Use a small frame count so the example completes in a few seconds even
    // on a single core.  n = 64800 LDPC decode is heavy; 8 frames is
    // representative without being slow.
    const FRAMES: usize = 8;
    const SEED: u64 = 0xC0DE_F00D;

    // Build the seven-stage CPU DVB-T2 BICM chain via the typestate builder.
    // The builder enforces the required call order at compile time.
    let pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(6.5))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(1).unwrap())
        .build()
        .expect("r1/2 16-QAM Normal is an in-scope MODCOD");

    println!(
        "DVB-T2 r1/2 16-QAM Normal: {} stages, seed {:#010x}",
        pipeline.stage_count(),
        pipeline.config().seed,
    );
    assert_eq!(
        pipeline.stage_count(),
        7,
        "forward(3) + channel(1) + inverse(3) = 7 stages"
    );

    // Drive FRAMES frames through the stage-driven executor.
    // TopologyExecutor::run_dvb_t2_snr_point takes any &Pipeline (including
    // graph-built ones) and uses Scheduler::from_pipeline for the rayon pool.
    let scheduler = Scheduler::from_pipeline(&pipeline);
    let counters = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, FRAMES)
        .expect("stage-driven sweep runs end-to-end");

    assert_eq!(counters.frames, FRAMES as u64, "all frames executed");

    println!(
        "{:<12} {:<8} {:<8} {:<10} {:<12}",
        "MODCOD", "frames", "errors", "FER", "mean_iters"
    );
    println!(
        "{:<12} {:<8} {:<8} {:<10.6} {:<12.3}",
        "r1/2 16-QAM",
        counters.frames,
        counters.errors,
        counters.fer(),
        counters.mean_iters(),
    );
}
