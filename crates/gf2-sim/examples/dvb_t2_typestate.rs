//! DVB-T2 BICM chain via the typestate fluent builder (`Pipeline::dvb_t2`):
//! Normal r1/2 16-QAM at the waterfall Es/N0 (6.0 dB), so the summary shows
//! a mixed verdict (3/8 errored frames at this seed) rather than the
//! informationless all-zeros of an above-threshold point. The builder
//! enforces the call order at compile time (`.decoder()` before `.modcod()`
//! does not compile). Graph-API version: `examples/dvb_t2_graph_api.rs`.
//!
//! Run with: `cargo run -p gf2-sim --example dvb_t2_typestate --release`

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

fn main() {
    // 8 frames keep the heavy n = 64800 waterfall decodes to a few seconds;
    // this pinned seed yields the mixed 3/8-errored verdict at 6.0 dB.
    const FRAMES: usize = 8;
    const SEED: u64 = 0xDE16_0FC5;
    const ES_N0_DB: f32 = 6.0;

    let pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(ES_N0_DB))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(4).unwrap())
        .build()
        .expect("r1/2 16-QAM Normal is an in-scope MODCOD");

    let scheduler = Scheduler::from_pipeline(&pipeline);
    let c = TopologyExecutor::run_dvb_t2_snr_point(&pipeline, &scheduler, 0, FRAMES)
        .expect("stage-driven sweep runs end-to-end");

    println!("DVB-T2 typestate preset @ {ES_N0_DB} dB (waterfall), seed {SEED:#010x}");
    println!("MODCOD        frames  errors  FER       mean_iters");
    let (f, e, fer, mi) = (c.frames, c.errors, c.fer(), c.mean_iters());
    println!("r1/2 16-QAM   {f:<7} {e:<7} {fer:<9.6} {mi:<10.3}");
}
