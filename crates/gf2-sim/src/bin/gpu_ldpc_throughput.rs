//! GPU LDPC belief-propagation decode-stage throughput benchmark (issue
//! `a930be7f`, parallelism-pays receipt).
//!
//! Measures **decode-stage** frames/second for the DVB-T2 r1/2 (n = 64800)
//! LDPC code at a waterfall operating point (sigma = 0.80, mean BP ~25.7
//! iterations — matching the `3fcb7025` full-chain mean_iters ~25.24),
//! decode-vs-decode (the user-approved 2026-06-09 apples-to-apples
//! amendment): the GPU
//! [`GpuLdpcBp`](gf2_sim::gpu::ldpc_bp::GpuLdpcBp) decode kernel (H2D + BP
//! iterations + D2H, per-worker-owned device decoder) against the CPU
//! [`LdpcDecoder::decode_to_codeword`](gf2_coding::ldpc::LdpcDecoder) decode
//! stage measured in isolation, at 1 thread and 24 threads. SumProduct,
//! early-termination on — matching the gate config.
//!
//! Full-frame baselines (`c0b1702d` 1.6216 fps single-thread / `3fcb7025`
//! 21.44 fps 24-thread) are printed for CONTEXT only; the gate is decode-vs-decode.
//!
//! Manually invoked (not a nextest test). Without `--features hip` it prints a
//! notice and exits 0.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --features hip --bin gpu_ldpc_throughput -- \
//!     --frames 200 --repeats 3 --max-iters 50
//! ```

fn main() {
    #[cfg(not(feature = "hip"))]
    {
        eprintln!(
            "gpu_ldpc_throughput requires --features hip (HIP/ROCm GPU). \
             Rebuild with: cargo run -p gf2-sim --release --features hip \
             --bin gpu_ldpc_throughput"
        );
    }

    #[cfg(feature = "hip")]
    imp::run();
}

#[cfg(feature = "hip")]
mod imp {
    use std::time::Instant;

    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder};
    use gf2_coding::{CodeRate, Llr};
    use gf2_sim::gpu::ldpc_bp::GpuLdpcBp;
    use gf2_sim::LlrBatch;

    const SEED: u64 = 0xA930_BE7F_C0DE;
    const FULL_FRAME_1T_BASELINE_FPS: f64 = 1.6216;
    const FULL_FRAME_24T_BASELINE_FPS: f64 = 21.44;

    fn awgn_llr_frame(state: &mut u64, n: usize, sigma: f64) -> Vec<Llr> {
        let n0 = 2.0 * sigma * sigma;
        let mut next = || {
            *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = *state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) >> 11
        };
        let uniform = |v: u64| v as f64 * (1.0 / 9007199254740992.0);
        (0..n)
            .map(|_| {
                let mut u1 = uniform(next());
                let u2 = uniform(next());
                if u1 < 1e-15 {
                    u1 = 1e-15;
                }
                let r = (-2.0 * u1.ln()).sqrt();
                let noise = r * (std::f64::consts::TAU * u2).cos() * sigma;
                Llr::new((2.0 * (1.0 + noise) / n0) as f32)
            })
            .collect()
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut frames = 200usize;
        let mut repeats = 3usize;
        let mut max_iters = 50usize;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--frames" => {
                    i += 1;
                    frames = args[i].parse().unwrap();
                }
                "--repeats" => {
                    i += 1;
                    repeats = args[i].parse().unwrap();
                }
                "--max-iters" => {
                    i += 1;
                    max_iters = args[i].parse().unwrap();
                }
                other => panic!("unknown arg: {other}"),
            }
            i += 1;
        }

        // Waterfall operating point for DVB-T2 r1/2 (all-zero-codeword BPSK):
        // sigma = 0.80 yields mean BP depth ~25.7 iterations with successful
        // decode, matching the `3fcb7025` full-chain mean_iters ~25.24 so the
        // decode-vs-decode comparison exercises a realistic iteration count
        // (not the trivial 1-iteration clean-channel case).
        let sigma = 0.80f64;
        let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        let n = code.n();
        let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);

        println!("# gpu_ldpc_throughput (issue a930be7f) — decode-vs-decode");
        println!("# config: DVB-T2 r1/2 n={n}, SumProduct, early-term on, waterfall sigma={sigma:.4} (mean BP ~25.7 iters)");
        println!("# frames={frames} repeats={repeats} max_iters={max_iters} seed={SEED:#x}");

        // Pre-generate the frame population once (shared by all paths).
        let mut state = SEED;
        let llr_frames: Vec<Vec<Llr>> = (0..frames)
            .map(|_| awgn_llr_frame(&mut state, n, sigma))
            .collect();

        // ---- GPU decode-stage throughput (batch all frames per launch) ----
        let stage = GpuLdpcBp::new(code.clone(), config, max_iters);
        let decoder = stage.build_decoder(frames).expect("build GPU LDPC decoder");
        let batch = LlrBatch::new(llr_frames.clone());
        let mut gpu_fps = Vec::new();
        for _ in 0..repeats {
            let t0 = Instant::now();
            let out = stage.decode_batch(&batch, &decoder).expect("gpu decode");
            let secs = t0.elapsed().as_secs_f64();
            std::hint::black_box(&out);
            gpu_fps.push(frames as f64 / secs);
        }
        let (gpu_mean, gpu_sigma) = mean_sigma(&gpu_fps);

        // ---- CPU single-thread decode-stage throughput ----
        let mut cpu1_fps = Vec::new();
        for _ in 0..repeats {
            let mut dec = LdpcDecoder::with_config(code.clone(), config);
            let t0 = Instant::now();
            let mut acc = 0u64;
            for f in &llr_frames {
                let cw = dec.decode_to_codeword(f, max_iters).decoded_bits;
                acc = acc.wrapping_add(cw.count_ones() as u64);
            }
            let secs = t0.elapsed().as_secs_f64();
            std::hint::black_box(acc);
            cpu1_fps.push(frames as f64 / secs);
        }
        let (cpu1_mean, cpu1_sigma) = mean_sigma(&cpu1_fps);

        // ---- CPU 24-thread decode-stage throughput (rayon batch) ----
        //
        // Mirrors `LdpcDecoder::decode_batch_with_config`'s `parallel`-feature
        // body (a `par_iter` of per-frame `with_config` decoders) but drives the
        // rayon pool from `gf2-sim` (which depends on `rayon`) directly, so the
        // 24-thread number is genuine even though `gf2-sim` does not enable
        // `gf2-coding/parallel`. The thread count is the rayon global pool size
        // (24 on this 12C/24T host), reported below.
        use rayon::prelude::*;
        let threads = rayon::current_num_threads();
        let mut cpu24_fps = Vec::new();
        for _ in 0..repeats {
            let t0 = Instant::now();
            let results: Vec<u64> = (0..frames)
                .into_par_iter()
                .map(|fi| {
                    let mut dec = LdpcDecoder::with_config(code.clone(), config);
                    let cw = dec
                        .decode_to_codeword(&llr_frames[fi], max_iters)
                        .decoded_bits;
                    cw.count_ones() as u64
                })
                .collect();
            let secs = t0.elapsed().as_secs_f64();
            std::hint::black_box(&results);
            cpu24_fps.push(frames as f64 / secs);
        }
        let (cpu24_mean, cpu24_sigma) = mean_sigma(&cpu24_fps);

        println!();
        println!("GPU  decode-stage fps:        {gpu_mean:.2} +/- {gpu_sigma:.2}");
        println!("CPU  decode-stage fps (1T):   {cpu1_mean:.4} +/- {cpu1_sigma:.4}");
        println!("CPU  decode-stage fps ({threads}T):  {cpu24_mean:.2} +/- {cpu24_sigma:.2}");
        println!();
        println!(
            "GPU / CPU-1-thread decode-stage speedup:  {:.2}x (gate >= 10x)",
            gpu_mean / cpu1_mean
        );
        println!(
            "GPU / CPU-24-thread decode-stage speedup: {:.2}x (gate >= 3x)",
            gpu_mean / cpu24_mean
        );
        println!();
        println!("# context only (full-frame baselines, NOT the gate metric):");
        println!("#   GPU decode-stage fps vs full-frame 1T baseline ({FULL_FRAME_1T_BASELINE_FPS} fps): {:.1}x", gpu_mean / FULL_FRAME_1T_BASELINE_FPS);
        println!("#   GPU decode-stage fps vs full-frame 24T baseline ({FULL_FRAME_24T_BASELINE_FPS} fps): {:.1}x", gpu_mean / FULL_FRAME_24T_BASELINE_FPS);
    }

    fn mean_sigma(xs: &[f64]) -> (f64, f64) {
        let n = xs.len() as f64;
        let mean = xs.iter().sum::<f64>() / n;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        (mean, var.sqrt())
    }
}
