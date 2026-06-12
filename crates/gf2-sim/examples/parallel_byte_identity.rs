//! Parallel byte-identity: the same config at parallelism {1, 24} agrees exactly.
//!
//! The within-SNR frame-parallel executor's headline guarantee (design doc §11,
//! "CPU-only / CPU-parallel contract"): at a fixed seed the four columns
//! `fer` / `frames` / `errors` / `mean_iters` are **byte-identical** across
//! worker counts. This example runs one DVB-T2 SNR point twice — once on a
//! single worker, once on 24 — and asserts the four columns match bit-for-bit.
//! (`ber` is deliberately excluded: its f32 horizontal reduction is
//! non-associative, so it is not contractual; see §11.)
//!
//! The byte-identity holds because every frame's RNG is keyed on the *global*
//! frame index via the §3 `worker_offset` seek, so the per-frame outcome is a
//! pure function of the frame index regardless of which physical worker ran it.
//!
//! Runtime: ~20 s (the 1-thread arm runs ~1.6 frames/s, so 24 frames is the
//! budget that keeps both arms comfortably bounded while staying non-vacuous —
//! at the 6.0 dB waterfall a mix of errored and clean frames is exercised).
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

/// Builds and runs the r1/2 16-QAM Normal sweep at the given worker count,
/// returning the single SNR point.
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

    // The §11 four-column byte-identity contract.
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
