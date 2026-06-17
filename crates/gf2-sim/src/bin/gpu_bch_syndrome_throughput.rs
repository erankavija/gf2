//! GPU BCH syndrome-evaluation throughput benchmark (issue `9012f8a0`,
//! design doc §11).
//!
//! Measures **syndrome-evaluation** frames/second for the DVB-T2 Normal Rate
//! 1/2 BCH code (n = 32400, GF(2^16), 2t = 24 — the design workload), decode
//! sub-step vs decode sub-step: the GPU
//! [`compute_syndromes_batch_gpu`](gf2_coding::bch::BchDecoder::compute_syndromes_batch_gpu)
//! path (H2D of packed coeff streams + Horner kernel + D2H of syndromes) against
//! the CPU [`compute_syndromes`](gf2_coding::bch::BchDecoder::compute_syndromes)
//! measured **in isolation** (NO Berlekamp-Massey / Chien), at 1 thread and at
//! the full rayon pool. The honest **best existing production CPU path is
//! single-thread**: the rayon-24T `compute_syndromes` is anomalously *slower*
//! than 1T due to `Arc<FieldParams>` refcount contention (see the receipt), so
//! 1T is the gate divisor, not 24T.
//!
//! The `[hard]` gate is GPU syndrome throughput >= 5x the best existing CPU
//! path (single-thread); both 1T and 24T are reported. This follows the
//! `a930be7f` decode-vs-decode precedent (avoid GPU-vs-serial category
//! confusion).
//!
//! Also reports a batch-size sweep (64 / 256 / 1024 / 4096 by default) and a
//! coarse host-side phase split (coeff repack+H2D vs kernel+D2H), plus the
//! hardware / ROCm metadata recorded in the receipt.
//!
//! Manually invoked (not a nextest test). Without `--features hip` it prints a
//! notice and exits 0.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --features hip \
//!     --bin gpu_bch_syndrome_throughput -- \
//!     --frames 1024 --repeats 5 --sweep 64,256,1024,4096
//! ```

fn main() {
    #[cfg(not(feature = "hip"))]
    {
        eprintln!(
            "gpu_bch_syndrome_throughput requires --features hip (HIP/ROCm GPU). \
             Rebuild with: cargo run -p gf2-sim --release --features hip \
             --bin gpu_bch_syndrome_throughput"
        );
    }

    #[cfg(feature = "hip")]
    imp::run();
}

#[cfg(feature = "hip")]
mod imp {
    use std::time::Instant;

    use gf2_coding::bch::dvb_t2::FrameSize;
    use gf2_coding::bch::{BchCode, BchDecoder, BchEncoder};
    use gf2_coding::traits::BlockEncoder;
    use gf2_coding::CodeRate;
    use gf2_core::BitVec;
    use gf2_kernels_hip::host::device_mem_info;
    use rayon::prelude::*;

    const SEED: u64 = 0x9012_F8A0_C0DE_0001;

    /// Deterministic SplitMix64 for reproducible frame population.
    struct SplitMix64(u64);
    impl SplitMix64 {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn below(&mut self, bound: usize) -> usize {
            (self.next_u64() % bound as u64) as usize
        }
    }

    /// A mixed frame population (valid + correctable + uncorrectable errors).
    fn population(
        encoder: &BchEncoder,
        k: usize,
        n: usize,
        t: usize,
        frames: usize,
    ) -> Vec<BitVec> {
        let mut rng = SplitMix64::new(SEED);
        let mut out = Vec::with_capacity(frames);
        for f in 0..frames {
            let mut msg = BitVec::zeros(k);
            for i in 0..k {
                if rng.next_u64() & 1 == 1 {
                    msg.set(i, true);
                }
            }
            let mut cw = encoder.encode(&msg);
            let errors = match f % 3 {
                0 => 0,
                1 => 1 + rng.below(t),
                _ => (t + 1) + rng.below(t + 1),
            };
            let mut done = 0;
            while done < errors {
                let pos = rng.below(n);
                cw.set(pos, !cw.get(pos));
                done += 1;
            }
            out.push(cw);
        }
        out
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut frames = 1024usize;
        let mut repeats = 5usize;
        let mut sweep = vec![64usize, 256, 1024, 4096];
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--frames" => {
                    frames = args[i + 1].parse().expect("--frames N");
                    i += 2;
                }
                "--repeats" => {
                    repeats = args[i + 1].parse().expect("--repeats N");
                    i += 2;
                }
                "--sweep" => {
                    sweep = args[i + 1]
                        .split(',')
                        .map(|s| s.parse().expect("--sweep n,n,..."))
                        .collect();
                    i += 2;
                }
                other => panic!("unknown arg {other}"),
            }
        }

        if device_mem_info().is_err() {
            eprintln!("no usable GPU (device_mem_info failed); aborting");
            std::process::exit(1);
        }

        // Design workload: DVB-T2 Normal Rate 1/2 BCH.
        let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
        let n = code.n();
        let k = code.k();
        let t = code.t();
        let two_t = 2 * t;
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let threads = rayon::current_num_threads();
        println!("# GPU BCH syndrome throughput — DVB-T2 Normal r1/2");
        println!("# n={n} k={k} t={t} 2t={two_t} field=GF(2^16)");
        println!("# rayon threads = {threads}");
        println!("# frames={frames} repeats={repeats} sweep={sweep:?}");
        println!();

        // Build the largest population once; sub-slice for the sweep.
        let max_frames = *sweep.iter().max().unwrap().max(&frames);
        let frames_all = population(&encoder, k, n, t, max_frames);

        // --- Main operating point (the gate) -------------------------------
        let pop = &frames_all[..frames];

        // GPU: median fps over `repeats` of the full compute_syndromes_batch_gpu.
        let gpu_fps = {
            // Warm up (allocates device buffers, JITs).
            let _ = decoder
                .compute_syndromes_batch_gpu(pop)
                .expect("gpu warmup");
            let mut best = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let s = decoder.compute_syndromes_batch_gpu(pop).expect("gpu eval");
                let dt = t0.elapsed().as_secs_f64();
                std::hint::black_box(&s);
                best = best.min(dt);
            }
            frames as f64 / best
        };

        // CPU 1-thread: compute_syndromes in isolation. fps is a rate, so this
        // is measured on a smaller subset (single-thread n=32400 syndrome eval
        // is ~ms/frame; the full batch would take minutes) and reported as fps.
        // This IS the gate divisor: 1T is the honest best existing production CPU
        // path, because the rayon-24T path is anomalously slower (Arc contention).
        let cpu1_count = frames.min(64);
        let cpu1_fps = {
            let sub = &frames_all[..cpu1_count];
            for f in sub {
                std::hint::black_box(decoder.compute_syndromes(f));
            }
            let mut best = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let mut acc = 0u64;
                for f in sub {
                    let s = decoder.compute_syndromes(f);
                    acc = acc.wrapping_add(s[0].value());
                }
                std::hint::black_box(acc);
                best = best.min(t0.elapsed().as_secs_f64());
            }
            cpu1_count as f64 / best
        };

        // CPU rayon pool: compute_syndromes in isolation (context only — slower
        // than 1T due to Arc<FieldParams> refcount contention; see the receipt).
        let cpu24_fps = {
            let _: Vec<_> = pop
                .par_iter()
                .map(|f| decoder.compute_syndromes(f))
                .collect();
            let mut best = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let v: Vec<u64> = pop
                    .par_iter()
                    .map(|f| decoder.compute_syndromes(f)[0].value())
                    .collect();
                std::hint::black_box(&v);
                best = best.min(t0.elapsed().as_secs_f64());
            }
            frames as f64 / best
        };

        let speedup_vs_1t = gpu_fps / cpu1_fps;
        let speedup_vs_24t = gpu_fps / cpu24_fps;

        println!("## Operating point (frames = {frames})");
        println!("GPU  syndrome fps : {gpu_fps:>14.1}");
        println!("CPU  1T  fps      : {cpu1_fps:>14.1}   (measured on {cpu1_count} frames; best existing CPU path)");
        println!("CPU {threads}T  fps      : {cpu24_fps:>14.1}   (context only — slower than 1T, Arc contention)");
        println!("speedup vs 1T     : {speedup_vs_1t:>14.2}x   <-- [hard] gate (>= 5x vs best existing CPU path)");
        println!("speedup vs {threads}T     : {speedup_vs_24t:>14.2}x   (context only)");
        println!(
            "GATE (>= 5x vs 1T): {}",
            if speedup_vs_1t >= 5.0 {
                "PASS"
            } else {
                "FAIL"
            }
        );
        println!();

        // --- Batch-size sweep ----------------------------------------------
        println!("## Batch-size sweep (GPU vs CPU{threads}T)");
        println!(
            "{:>8}  {:>14}  {:>14}  {:>12}",
            "batch", "GPU fps", "CPU24T fps", "speedup"
        );
        for &b in &sweep {
            if b > max_frames {
                continue;
            }
            let sub = &frames_all[..b];
            let _ = decoder.compute_syndromes_batch_gpu(sub).expect("warmup");
            let mut gbest = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let s = decoder.compute_syndromes_batch_gpu(sub).expect("eval");
                std::hint::black_box(&s);
                gbest = gbest.min(t0.elapsed().as_secs_f64());
            }
            let g = b as f64 / gbest;
            let mut cbest = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let v: Vec<u64> = sub
                    .par_iter()
                    .map(|f| decoder.compute_syndromes(f)[0].value())
                    .collect();
                std::hint::black_box(&v);
                cbest = cbest.min(t0.elapsed().as_secs_f64());
            }
            let c = b as f64 / cbest;
            println!("{b:>8}  {g:>14.1}  {c:>14.1}  {:>11.2}x", g / c);
        }
        println!();

        // --- Coarse phase split --------------------------------------------
        // Host-side repack cost (build the packed coeff streams) measured in
        // isolation, vs the full GPU call; the remainder is H2D + kernel + D2H +
        // rehydrate. A finer H2D/kernel/D2H split would need wrapper-level
        // instrumentation; this coarse split is the "where practical" §11 ask.
        {
            let pop = &frames_all[..frames];
            let mut repack_best = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                // Use the SAME packer as the production hook (SSOT) so the
                // measured repack cost matches the real path exactly.
                let wpf = n.div_ceil(64);
                let mut streams: Vec<u64> = Vec::with_capacity(frames * wpf);
                for frame in pop {
                    streams.extend_from_slice(&decoder.pack_coeff_stream(frame));
                }
                std::hint::black_box(&streams);
                repack_best = repack_best.min(t0.elapsed().as_secs_f64());
            }
            let full_best = frames as f64 / gpu_fps;
            let repack_frac = repack_best / full_best * 100.0;
            println!("## Coarse phase split (frames = {frames})");
            println!(
                "host coeff repack : {:>10.3} ms  ({repack_frac:.1}% of full call)",
                repack_best * 1e3
            );
            println!(
                "device+xfer       : {:>10.3} ms  ({:.1}% of full call)",
                (full_best - repack_best) * 1e3,
                100.0 - repack_frac
            );
        }
    }
}
