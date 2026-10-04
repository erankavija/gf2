//! DVB-T2 quickstart: builds the DVB-T2 BICM pipeline through the typestate
//! preset ([`Pipeline::dvb_t2`]), runs one waterfall SNR point with a small
//! frame budget through [`Pipeline::run`], and prints the per-point columns.
//! Run with `cargo run -p gf2-sim --example dvb_t2_quickstart --release`.

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::Pipeline;

fn main() {
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
