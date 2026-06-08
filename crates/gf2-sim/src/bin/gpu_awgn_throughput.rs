//! GPU AWGN noise-generation throughput benchmark (issue `f6004add`,
//! parallelism-pays receipt).
//!
//! Measures **AWGN-step** frames/second for the DVB-T2 r1/2 16-QAM canonical
//! config (n_ldpc = 64800, 16-QAM = 4 bits/symbol → 16200 symbols/frame, 32400
//! noise samples/frame) at Es/N0 = 6.5 dB, comparing:
//!
//! * the GPU [`GpuAwgn`](gf2_sim::gpu::awgn::GpuAwgn) noise step (one device
//!   launch + read-back per frame, per-worker-owned generator), against
//! * a single CPU thread doing the **same** AWGN noise step via
//!   [`channels::Awgn`](gf2_sim::channels::Awgn).
//!
//! This is an apples-to-apples *AWGN-only* comparison: both paths add complex
//! Gaussian noise to the same per-frame symbol count, so the ratio isolates the
//! noise-sampling speedup the GPU kernel delivers. (The full-frame
//! encode+channel+decode baseline of 1.6216 fps in `baseline-single-thread.md`
//! is dominated by LDPC BP decode, which this kernel does not touch; reporting
//! the AWGN-only ratio here keeps the receipt honest about what `f6004add`
//! actually accelerates.)
//!
//! This is a manually-invoked benchmark, not a nextest test (it can exceed the
//! 5 s fast-tier limit at large frame counts).
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --features hip --bin gpu_awgn_throughput -- \
//!     --frames 2000 --repeats 5 --es-n0 6.5
//! ```
//!
//! Defaults: `--frames 2000 --repeats 5 --es-n0 6.5`. Without `--features hip`
//! the binary prints a notice and exits 0.

fn main() {
    #[cfg(not(feature = "hip"))]
    {
        eprintln!(
            "gpu_awgn_throughput requires --features hip (HIP/ROCm GPU). \
             Rebuild with: cargo run -p gf2-sim --release --features hip \
             --bin gpu_awgn_throughput"
        );
    }

    #[cfg(feature = "hip")]
    imp::run();
}

#[cfg(feature = "hip")]
mod imp {
    use std::time::Instant;

    use gf2_sim::channels::Awgn;
    use gf2_sim::gpu::awgn::GpuAwgn;
    use gf2_sim::parallel::WorkerCtx;
    use gf2_sim::SymbolBatch;

    // DVB-T2 r1/2 16-QAM Normal: n_ldpc = 64800, 4 bits/symbol → 16200 symbols.
    const SYMBOLS_PER_FRAME: usize = 16200;
    const SEED: u64 = 0xC0DE_F00D;
    const SINGLE_THREAD_FULL_FRAME_BASELINE_FPS: f64 = 1.6216;

    pub fn run() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut frames: usize = 2000;
        let mut repeats: usize = 5;
        let mut es_n0_db: f32 = 6.5;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--frames" => {
                    i += 1;
                    frames = args[i].parse().expect("--frames <usize>");
                }
                "--repeats" => {
                    i += 1;
                    repeats = args[i].parse().expect("--repeats <usize>");
                }
                "--es-n0" => {
                    i += 1;
                    es_n0_db = args[i].parse().expect("--es-n0 <f32>");
                }
                other => panic!("unknown arg: {other}"),
            }
            i += 1;
        }

        println!("# gpu_awgn_throughput (issue f6004add)");
        println!("# config: r1/2 16-QAM Normal, {SYMBOLS_PER_FRAME} symbols/frame, Es/N0 = {es_n0_db} dB");
        println!("# frames={frames} repeats={repeats} seed={SEED:#x}");

        // A flat input symbol batch (one frame at a time, reused). The exact
        // symbol values are irrelevant to the noise-generation cost.
        let template_i: Vec<f32> = vec![1.0; SYMBOLS_PER_FRAME];
        let template_q: Vec<f32> = vec![0.0; SYMBOLS_PER_FRAME];

        // ---- CPU single-thread AWGN-step throughput ----------------------
        let cpu = Awgn::new(es_n0_db, 4);
        let mut cpu_fps = Vec::new();
        for _ in 0..repeats {
            let mut ctx = WorkerCtx::new(SEED, 0, 0);
            let t0 = Instant::now();
            let mut acc = 0.0f64; // prevent dead-code elimination
            for f in 0..frames {
                let mut batch =
                    SymbolBatch::new(vec![template_i.clone()], vec![template_q.clone()]);
                cpu.apply_for_frame(&mut batch, &mut ctx, f);
                acc += batch.i[0][0] as f64;
            }
            let secs = t0.elapsed().as_secs_f64();
            std::hint::black_box(acc);
            cpu_fps.push(frames as f64 / secs);
        }
        let (cpu_mean, cpu_sigma) = mean_sigma(&cpu_fps);

        // ---- GPU AWGN-step throughput ------------------------------------
        let gpu = GpuAwgn::new(es_n0_db, 4).with_seek(SEED, 0, 0);
        let generator = gpu
            .build_generator(SYMBOLS_PER_FRAME)
            .expect("build GPU noise generator (requires a gfx1030 GPU)");
        let mut gpu_fps = Vec::new();
        for _ in 0..repeats {
            let t0 = Instant::now();
            let mut acc = 0.0f64;
            for f in 0..frames {
                let mut bi = template_i.clone();
                let mut bq = template_q.clone();
                gpu.apply_for_frame(&mut bi, &mut bq, f, &generator)
                    .expect("gpu awgn frame");
                acc += bi[0] as f64;
            }
            let secs = t0.elapsed().as_secs_f64();
            std::hint::black_box(acc);
            gpu_fps.push(frames as f64 / secs);
        }
        let (gpu_mean, gpu_sigma) = mean_sigma(&gpu_fps);

        println!();
        println!("CPU  AWGN-step fps: {cpu_mean:.2} +/- {cpu_sigma:.2} (single thread)");
        println!("GPU  AWGN-step fps: {gpu_mean:.2} +/- {gpu_sigma:.2}");
        println!(
            "GPU / CPU-1-thread AWGN-step speedup: {:.2}x",
            gpu_mean / cpu_mean
        );
        println!(
            "GPU AWGN-step fps vs full-frame single-thread baseline ({SINGLE_THREAD_FULL_FRAME_BASELINE_FPS} fps): {:.1}x",
            gpu_mean / SINGLE_THREAD_FULL_FRAME_BASELINE_FPS
        );
        println!(
            "CPU AWGN-step fps vs full-frame single-thread baseline: {:.1}x",
            cpu_mean / SINGLE_THREAD_FULL_FRAME_BASELINE_FPS
        );
    }

    fn mean_sigma(xs: &[f64]) -> (f64, f64) {
        let n = xs.len() as f64;
        let mean = xs.iter().sum::<f64>() / n;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        (mean, var.sqrt())
    }
}
