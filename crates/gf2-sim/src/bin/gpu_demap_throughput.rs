//! GPU Gray-QAM max-log demap-stage throughput benchmark (issue `d3f1616a`,
//! parallelism-pays receipt).
//!
//! Measures **demap-stage** symbols/second (and frames/second) for DVB-T2
//! 16-QAM / 64-QAM max-log soft demapping, demap-vs-demap (the user-approved
//! 2026-06-09 apples-to-apples amendment): the GPU
//! [`GpuGrayQamDemapper`](gf2_sim::gpu::demap::GpuGrayQamDemapper) `demap_batch`
//! kernel (H2D + kernel + D2H, per-worker-owned device demapper) against the
//! CPU [`FastGrayQamDemapper`](gf2_coding::modem::FastGrayQamDemapper)
//! demap-step measured in isolation, at 1 thread and 24 threads. MaxLog only
//! (the GPU kernel computes max-log only).
//!
//! The gate divisor is the **single-thread CPU `FastGrayQamDemapper`
//! demap-step** at the matching modulation/method (the apples-to-apples
//! amendment): the full-frame `c0b1702d` 1.6216 fps baseline is category-confused
//! for a demap-only kernel (demap is a small fraction of a frame) and is printed
//! for CONTEXT only.
//!
//! Manually invoked (not a nextest test). Without `--features hip` it prints a
//! notice and exits 0.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --features hip --bin gpu_demap_throughput -- \
//!     --frames 64 --symbols 16200 --repeats 5
//! ```

fn main() {
    #[cfg(not(feature = "hip"))]
    {
        eprintln!(
            "gpu_demap_throughput requires --features hip (HIP/ROCm GPU). \
             Rebuild with: cargo run -p gf2-sim --release --features hip \
             --bin gpu_demap_throughput"
        );
    }

    #[cfg(feature = "hip")]
    imp::run();
}

#[cfg(feature = "hip")]
mod imp {
    use std::time::Instant;

    use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
    use gf2_coding::modem::{
        BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, ModemSpec,
    };
    use gf2_coding::Llr;
    use gf2_sim::batch::SymbolBatch;
    use gf2_sim::gpu::demap::GpuGrayQamDemapper;
    use rayon::prelude::*;

    const SEED: u64 = 0xD3F1_616A_C0DE;
    const FULL_FRAME_1T_BASELINE_FPS: f64 = 1.6216;

    /// Deterministic signed-unit f32 stream (SplitMix64 → [-1.5, 1.5)).
    fn fill_iq(state: &mut u64, n: usize) -> (Vec<f32>, Vec<f32>) {
        let mut next = || {
            *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = *state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            (((z >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0) * 1.5
        };
        let i: Vec<f32> = (0..n).map(|_| next()).collect();
        let q: Vec<f32> = (0..n).map(|_| next()).collect();
        (i, q)
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut frames = 64usize;
        let mut symbols = 16_200usize; // ~one DVB-T2 16-QAM FECFRAME worth of cells
        let mut repeats = 5usize;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--frames" => {
                    i += 1;
                    frames = args[i].parse().unwrap();
                }
                "--symbols" => {
                    i += 1;
                    symbols = args[i].parse().unwrap();
                }
                "--repeats" => {
                    i += 1;
                    repeats = args[i].parse().unwrap();
                }
                other => panic!("unknown arg: {other}"),
            }
            i += 1;
        }

        let threads = rayon::current_num_threads();
        println!("# gpu_demap_throughput (issue d3f1616a) — demap-vs-demap, MaxLog");
        println!("# frames={frames} symbols/frame={symbols} repeats={repeats} seed={SEED:#x} threads={threads}");
        let noise_var = 0.35f32;

        for &modulation in &[DvbT2Modulation::Qam16, DvbT2Modulation::Qam64] {
            let m = modulation.bits_per_cell();
            let order = 1usize << m;
            println!();
            println!("== {modulation:?} (m={m}) ==");

            // Pre-generate the frame population once (shared by all paths).
            let mut state = SEED ^ (order as u64);
            let iq: Vec<(Vec<f32>, Vec<f32>)> =
                (0..frames).map(|_| fill_iq(&mut state, symbols)).collect();
            let total_symbols = (frames * symbols) as f64;

            // ---- GPU demap-stage throughput ----
            //
            // The whole population is demapped as ONE device launch (all
            // `frames * symbols` symbols concatenated into a single `SymbolBatch`
            // frame), amortising the per-call H2D/sync/D2H launch overhead — the
            // genuine batched-GPU throughput path, mirroring the LDPC bench's
            // single batched call. (The per-frame `Stage::process` loop is
            // launch-overhead-bound and is not the throughput path.)
            let stage = GpuGrayQamDemapper::new(modulation, DemapMethod::MaxLog, noise_var);
            let big = frames * symbols;
            let demapper = stage.build_demapper(big).expect("build GPU demapper");
            let cat_i: Vec<f32> = iq.iter().flat_map(|(i, _)| i.iter().copied()).collect();
            let cat_q: Vec<f32> = iq.iter().flat_map(|(_, q)| q.iter().copied()).collect();
            let batch = SymbolBatch::new(vec![cat_i], vec![cat_q]);
            let mut gpu_sps = Vec::new();
            for _ in 0..repeats {
                let t0 = Instant::now();
                let out = stage.demap_batch(&batch, &demapper).expect("gpu demap");
                let secs = t0.elapsed().as_secs_f64();
                std::hint::black_box(&out);
                gpu_sps.push(total_symbols / secs);
            }
            let (gpu_mean, gpu_sigma) = mean_sigma(&gpu_sps);

            // ---- CPU single-thread demap-step throughput ----
            // Deliberately NOT the shared `stages::GrayQamDemapCore` frame loop:
            // the per-iteration `out` allocation and raw `demap_llrs` call ARE
            // the measured quantity (benchmark geometry, not a stage duplicate).
            let cpu = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(order));
            let nv = vec![noise_var; symbols];
            let mut cpu1_sps = Vec::new();
            for _ in 0..repeats {
                let t0 = Instant::now();
                let mut acc = 0.0f32;
                for (ri, rq) in &iq {
                    let mut out = vec![Llr::zero(); symbols * m];
                    cpu.demap_llrs(
                        DemapInput {
                            rx_i: ri,
                            rx_q: rq,
                            gain_i: None,
                            gain_q: None,
                            noise_var: &nv,
                            method: DemapMethod::MaxLog,
                        },
                        &mut out,
                    );
                    acc += out[0].value();
                }
                let secs = t0.elapsed().as_secs_f64();
                std::hint::black_box(acc);
                cpu1_sps.push(total_symbols / secs);
            }
            let (cpu1_mean, cpu1_sigma) = mean_sigma(&cpu1_sps);

            // ---- CPU 24-thread demap-step throughput (rayon over frames) ----
            let mut cpu24_sps = Vec::new();
            for _ in 0..repeats {
                let t0 = Instant::now();
                let results: Vec<f32> = (0..frames)
                    .into_par_iter()
                    .map(|fi| {
                        // Per-frame independent demapper (the demap is a pure
                        // function of the frame's I/Q, deterministic per thread);
                        // hand-rolled for the same benchmark-geometry reason as
                        // the single-thread loop above.
                        let dem =
                            FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(order));
                        let (ri, rq) = &iq[fi];
                        let mut out = vec![Llr::zero(); symbols * m];
                        dem.demap_llrs(
                            DemapInput {
                                rx_i: ri,
                                rx_q: rq,
                                gain_i: None,
                                gain_q: None,
                                noise_var: &nv,
                                method: DemapMethod::MaxLog,
                            },
                            &mut out,
                        );
                        out[0].value()
                    })
                    .collect();
                let secs = t0.elapsed().as_secs_f64();
                std::hint::black_box(&results);
                cpu24_sps.push(total_symbols / secs);
            }
            let (cpu24_mean, cpu24_sigma) = mean_sigma(&cpu24_sps);

            let gpu_fps = gpu_mean / symbols as f64;
            println!("GPU  demap symbols/s:        {gpu_mean:.3e} +/- {gpu_sigma:.2e}  ({gpu_fps:.1} frames/s)");
            println!("CPU  demap symbols/s (1T):   {cpu1_mean:.3e} +/- {cpu1_sigma:.2e}");
            println!("CPU  demap symbols/s ({threads}T):  {cpu24_mean:.3e} +/- {cpu24_sigma:.2e}");
            println!(
                "GPU / CPU-1-thread demap speedup:  {:.2}x (gate >= 5x)",
                gpu_mean / cpu1_mean
            );
            println!(
                "GPU / CPU-{threads}-thread demap speedup: {:.2}x (diagnostic)",
                gpu_mean / cpu24_mean
            );
            println!(
                "# context only: GPU demap frames/s vs full-frame 1T baseline ({FULL_FRAME_1T_BASELINE_FPS} fps): {:.1}x",
                gpu_fps / FULL_FRAME_1T_BASELINE_FPS
            );
        }
    }

    fn mean_sigma(xs: &[f64]) -> (f64, f64) {
        let n = xs.len() as f64;
        let mean = xs.iter().sum::<f64>() / n;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        (mean, var.sqrt())
    }
}
