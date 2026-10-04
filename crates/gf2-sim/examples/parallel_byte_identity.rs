//! Runs one DVB-T2 SNR point on 1 and on 24 workers at a fixed seed and asserts
//! that `fer` / `frames` / `errors` / `mean_iters` match bit-for-bit. Each
//! frame's RNG is keyed on the global frame index, so the per-frame outcome
//! does not depend on which worker ran it.
//!
//! Run with: `cargo run -p gf2-sim --example parallel_byte_identity --release`

use std::num::NonZeroUsize;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::executor::SnrPointResult;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::Pipeline;

const SEED: u64 = 0xDE16_0FC5;
const ES_N0_DB: f32 = 6.0;
const FRAMES: u64 = 24;

fn run_at(parallelism: usize) -> SnrPointResult {
    let mut pipeline = Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(ES_N0_DB))
        .seed(SEED)
        .parallelism(NonZeroUsize::new(parallelism).expect("parallelism is non-zero"))
        .build()
        .expect("r1/2 16-QAM Normal is an in-scope MODCOD");
    pipeline.config_mut().esn0_db_points = vec![f64::from(ES_N0_DB)];
    pipeline.config_mut().max_frames = FRAMES;
    let results = pipeline.run().expect("the DVB-T2 sweep runs end-to-end");
    results.per_point[0]
}

fn main() {
    let one = run_at(1);
    let many = run_at(24);

    println!("DVB-T2 r1/2 16-QAM Normal @ {ES_N0_DB} dB, seed {SEED:#010x}, {FRAMES} frames");
    println!("workers  frames  errors  FER          mean_iters");
    for (label, p) in [("1", &one), ("24", &many)] {
        println!(
            "{label:<8} {:<7} {:<7} {:<12.9} {:<10.6}",
            p.frames, p.errors, p.fer, p.mean_iters
        );
    }

    assert_eq!(one.frames, many.frames, "frames byte-identical");
    assert_eq!(one.errors, many.errors, "errors byte-identical");
    assert_eq!(one.fer.to_bits(), many.fer.to_bits(), "fer byte-identical");
    assert_eq!(
        one.mean_iters.to_bits(),
        many.mean_iters.to_bits(),
        "mean_iters byte-identical"
    );
    println!("\nbyte-identity: PASS (fer/frames/errors/mean_iters identical across {{1, 24}})");
}
