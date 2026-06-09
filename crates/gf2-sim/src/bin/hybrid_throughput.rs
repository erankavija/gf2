//! Hybrid CPU+GPU pipeline throughput benchmark (issue `75c22fa8`,
//! parallelism-pays receipt).
//!
//! Measures end-to-end **full-frame** frames/second for the calibration chain
//! (DVB-T2 r1/2 16-QAM, FrameSize::Normal n=64800) at a deep-waterfall Es/N0,
//! three ways:
//!
//! * **CPU 1-thread** — the CPU-only pipeline path (`with_gpu(false)`,
//!   parallelism = 1): the single-thread full-frame baseline (context).
//! * **CPU 24-thread** — the CPU-only pipeline path at the host thread count:
//!   the gate's divisor (the `3fcb7025` 21.44 fps baseline lives here).
//! * **CPU+GPU hybrid** — the hybrid scheduler (`with_gpu(true)`): each worker
//!   prepares the next batch on the CPU while the GPU LDPC-decodes the current
//!   batch, with the heavy decode off the CPU critical path.
//!
//! The gate (criterion 2): combined CPU+GPU ≥ 1.5× the CPU-24-thread baseline.
//!
//! Manually invoked (not a nextest test). Without `--features hip` it prints a
//! notice and exits 0.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --features hip --bin hybrid_throughput -- \
//!     --frames 240 --repeats 3 --es-n0 6.0
//! ```

fn main() {
    #[cfg(not(feature = "hip"))]
    {
        eprintln!(
            "hybrid_throughput requires --features hip (HIP/ROCm GPU). \
             Rebuild with: cargo run -p gf2-sim --release --features hip \
             --bin hybrid_throughput"
        );
    }

    #[cfg(feature = "hip")]
    imp::run();
}

#[cfg(feature = "hip")]
mod imp {
    use std::num::NonZeroUsize;
    use std::time::Instant;

    use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
    use gf2_coding::modem::DemapMethod;
    use gf2_coding::CodeRate;
    use gf2_sim::presets::dvb_t2::{Channel, Modcod};
    use gf2_sim::Pipeline;

    const SEED: u64 = 0x75C2_2FA8_C0DE;
    // The `3fcb7025` canonical CPU baselines (printed for context / the gate
    // divisor): single-thread headline 1.6216 fps, 24-thread 21.44 fps.
    const CPU_1T_HEADLINE_FPS: f64 = 1.6216;
    const CPU_24T_BASELINE_FPS: f64 = 21.44;

    fn build(workers: usize, gpu: bool, es_n0: f32, frames: u64) -> Pipeline {
        let mut p = Pipeline::dvb_t2()
            .modcod(Modcod::Normal {
                rate: CodeRate::Rate1_2,
                modulation: DvbT2Modulation::Qam16,
            })
            .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
            .demap(DemapMethod::MaxLog)
            .channel(Channel::awgn(es_n0))
            .parallelism(NonZeroUsize::new(workers).unwrap())
            .seed(SEED)
            .with_gpu(gpu)
            .build()
            .expect("in-scope MODCOD builds");
        p.config_mut().esn0_db_points = vec![es_n0 as f64];
        p.config_mut().max_frames = frames;
        p
    }

    fn time_run(p: &Pipeline, frames: u64, repeats: usize) -> (Vec<f64>, f64) {
        let mut fps = Vec::new();
        let mut last_fer = 0.0;
        for _ in 0..repeats {
            let t0 = Instant::now();
            let r = p.run().expect("pipeline run");
            let secs = t0.elapsed().as_secs_f64();
            last_fer = r.per_point[0].fer;
            std::hint::black_box(&r);
            fps.push(frames as f64 / secs);
        }
        (fps, last_fer)
    }

    fn mean_sigma(xs: &[f64]) -> (f64, f64) {
        let n = xs.len() as f64;
        let mean = xs.iter().sum::<f64>() / n;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        (mean, var.sqrt())
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut frames = 240u64;
        let mut repeats = 3usize;
        let mut es_n0 = 6.0f32;
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
                "--es-n0" => {
                    i += 1;
                    es_n0 = args[i].parse().unwrap();
                }
                other => panic!("unknown arg: {other}"),
            }
            i += 1;
        }

        let threads = rayon::current_num_threads();
        println!("# hybrid_throughput (issue 75c22fa8) — full-frame fps");
        println!(
            "# config: DVB-T2 r1/2 16-QAM Normal (n=64800), SumProduct early-term on, \
             MaxLog demap, Es/N0={es_n0} dB"
        );
        println!("# frames={frames} repeats={repeats} seed={SEED:#x} host_threads={threads}");

        // Host quietness diagnostic (a loaded host UNDERSTATES throughput and
        // invalidates the receipt — see CLAUDE.md parallelism-pays gate).
        if let Ok(la) = std::fs::read_to_string("/proc/loadavg") {
            println!("# /proc/loadavg: {}", la.trim());
        }

        // ---- CPU 1-thread (context baseline) ----
        let cpu1 = build(1, false, es_n0, frames);
        let (cpu1_fps, cpu1_fer) = time_run(&cpu1, frames, repeats);
        let (cpu1_mean, cpu1_sigma) = mean_sigma(&cpu1_fps);

        // ---- CPU 24-thread (gate divisor) ----
        let cpu24 = build(threads, false, es_n0, frames);
        let (cpu24_fps, cpu24_fer) = time_run(&cpu24, frames, repeats);
        let (cpu24_mean, cpu24_sigma) = mean_sigma(&cpu24_fps);

        // ---- CPU+GPU hybrid ----
        let hybrid = build(threads, true, es_n0, frames);
        let (hyb_fps, hyb_fer) = time_run(&hybrid, frames, repeats);
        let (hyb_mean, hyb_sigma) = mean_sigma(&hyb_fps);

        println!();
        println!("CPU  1-thread  full-frame fps: {cpu1_mean:.4} +/- {cpu1_sigma:.4}  (fer={cpu1_fer:.4})");
        println!("CPU  {threads}-thread full-frame fps: {cpu24_mean:.2} +/- {cpu24_sigma:.2}  (fer={cpu24_fer:.4})");
        println!(
            "CPU+GPU hybrid full-frame fps: {hyb_mean:.2} +/- {hyb_sigma:.2}  (fer={hyb_fer:.4})"
        );
        println!();
        println!(
            "Hybrid / CPU-{threads}-thread speedup: {:.2}x  (gate >= 1.5x)",
            hyb_mean / cpu24_mean
        );
        println!(
            "Hybrid / CPU-1-thread speedup:  {:.1}x  (context)",
            hyb_mean / cpu1_mean
        );
        println!();
        println!("# canonical baselines (3fcb7025) for cross-check:");
        println!("#   CPU 1-thread headline:  {CPU_1T_HEADLINE_FPS} fps");
        println!("#   CPU 24-thread baseline: {CPU_24T_BASELINE_FPS} fps");
        println!(
            "#   hybrid vs canonical 24-thread baseline: {:.2}x",
            hyb_mean / CPU_24T_BASELINE_FPS
        );
    }
}
