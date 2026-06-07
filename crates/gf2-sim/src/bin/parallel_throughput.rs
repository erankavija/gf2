//! Within-SNR parallel throughput benchmark (issue `3fcb7025`).
//!
//! Measures frames/second for the DVB-T2 r1/2 16-QAM canonical config at
//! Es/N0 = 6.5 dB across worker counts, establishing the canonical CPU
//! 24-thread baseline that downstream GPU receipts reference. Compares against
//! the single-thread headline baseline of 1.6216 fps
//! (`dev/benchmarks/gf2-sim/baseline-single-thread.md`).
//!
//! This is a manually-invoked benchmark, not a nextest test (it far exceeds the
//! 5 s fast-tier limit).
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --bin parallel_throughput -- \
//!     --frames 96 --workers 1,2,4,8,24 --repeats 3
//! ```
//!
//! Defaults: `--frames 96 --workers 1,24 --repeats 3 --es-n0 6.5`.

use std::num::NonZeroUsize;
use std::time::Instant;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_sim::frame_sim::DvbT2BicmFrameSim;
use gf2_sim::parallel::run_snr_point;

const SINGLE_THREAD_BASELINE_FPS: f64 = 1.6216;
const SEED: u64 = 0xC0DE_F00D;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut frames: usize = 96;
    let mut workers: Vec<usize> = vec![1, 24];
    let mut repeats: usize = 3;
    let mut es_n0_db: f64 = 6.5;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--frames" => {
                i += 1;
                frames = args[i].parse().expect("--frames <usize>");
            }
            "--workers" => {
                i += 1;
                workers = args[i]
                    .split(',')
                    .map(|s| s.parse().expect("--workers <comma-separated usize>"))
                    .collect();
            }
            "--repeats" => {
                i += 1;
                repeats = args[i].parse().expect("--repeats <usize>");
            }
            "--es-n0" => {
                i += 1;
                es_n0_db = args[i].parse().expect("--es-n0 <f64>");
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: parallel_throughput [--frames N] [--workers a,b,c] \
                     [--repeats N] [--es-n0 DB]\n\
                     Defaults: --frames 96 --workers 1,24 --repeats 3 --es-n0 6.5\n\
                     Measures DVB-T2 r1/2 16-QAM SumProduct/ExactLogMap throughput."
                );
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    eprintln!(
        "DVB-T2 r1/2 16-QAM, SumProduct, ExactLogMap, Es/N0={es_n0_db} dB, \
         {frames} frames/run, {repeats} repeats per worker count."
    );
    eprintln!("Single-thread headline baseline: {SINGLE_THREAD_BASELINE_FPS} fps.\n");

    let sim = DvbT2BicmFrameSim::new(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        es_n0_db,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
        DemapMethod::ExactLogMap,
    );

    println!(
        "{:>8} {:>12} {:>10} {:>10} {:>10}",
        "workers", "fps_mean", "fps_sigma", "speedup", "frames"
    );

    for &w in &workers {
        let p = NonZeroUsize::new(w).expect("worker count is non-zero");
        let mut fps_samples = Vec::with_capacity(repeats);
        let mut last_frames = 0u64;
        for _ in 0..repeats {
            let start = Instant::now();
            let counters = run_snr_point(
                SEED,
                0,
                frames,
                p,
                || sim.clone(),
                |g, ctx, s| s.simulate_frame(g, ctx),
            );
            let secs = start.elapsed().as_secs_f64();
            last_frames = counters.frames;
            let fps = counters.frames as f64 / secs;
            fps_samples.push(fps);
        }
        let mean = fps_samples.iter().sum::<f64>() / fps_samples.len() as f64;
        let var = fps_samples
            .iter()
            .map(|x| (x - mean) * (x - mean))
            .sum::<f64>()
            / fps_samples.len() as f64;
        let sigma = var.sqrt();
        let speedup = mean / SINGLE_THREAD_BASELINE_FPS;
        println!("{w:>8} {mean:>12.4} {sigma:>10.4} {speedup:>9.2}x {last_frames:>10}");
    }
}
