//! Measures BCH syndrome-evaluation frames per second for the DVB-T2 Normal
//! rate-1/2 mother code: the GPU `compute_syndromes_batch_gpu` path against the
//! CPU syndrome evaluation, timed through
//! [`correct_in_place`](gf2_coding::bch::BinaryBchDecoder::correct_in_place),
//! at one thread and at the rayon pool size, plus a batch-size sweep and the
//! host-staging share of the GPU call. Without `--features hip` the binary
//! prints a notice and exits 0.

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

    use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, FrameSize};
    use gf2_coding::bch::{BchDecodeOutcome, BinaryBchCode, BinaryBchDecoder};
    use gf2_coding::traits::block::BlockEncoder;
    use gf2_coding::CodeRate;
    use gf2_core::BitVec;
    use gf2_kernels_hip::host::device_mem_info;
    use rayon::prelude::*;

    const SEED: u64 = 0x9012_F8A0_C0DE_0001;

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

    fn population(
        code: &BinaryBchCode,
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
            let mut cw = code.encode(&msg).expect("a k-bit message encodes");
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

        // The DVB-T2 outer code decodes through its mother code, which is the
        // code the device syndrome evaluation covers.
        let dvb_t2 = dvb_t2_bch_code(FrameSize::Normal, CodeRate::Rate1_2)
            .expect("the DVB-T2 Normal r1/2 outer code");
        let code = dvb_t2.mother().code();
        let n = code.n();
        let k = code.k();
        let t = code.correction_radius();
        let two_t = 2 * t;
        let decoder = BinaryBchDecoder::new(code);

        let threads = rayon::current_num_threads();
        println!("# GPU BCH syndrome throughput — DVB-T2 Normal r1/2 mother code");
        println!("# n={n} k={k} t={t} 2t={two_t} field=GF(2^16)");
        println!("# rayon threads = {threads}");
        println!("# frames={frames} repeats={repeats} sweep={sweep:?}");
        println!();

        let max_frames = *sweep.iter().max().unwrap().max(&frames);
        let frames_all = population(code, k, n, t, max_frames);
        // The CPU arms time `correct_in_place` over error-free codewords (every
        // third population frame), where it returns right after the syndrome
        // evaluation.
        let mut clean_all: Vec<BitVec> = frames_all
            .iter()
            .step_by(3)
            .cycle()
            .take(max_frames)
            .cloned()
            .collect();
        {
            let mut workspace = decoder.workspace();
            let mut probe = clean_all[0].clone();
            assert_eq!(
                decoder
                    .correct_in_place(&mut probe, &mut workspace)
                    .unwrap(),
                BchDecodeOutcome::NoErrors,
                "the encoder's codewords are codewords of the decoder's code"
            );
        }

        let pop = &frames_all[..frames];

        let gpu_fps = {
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

        let cpu1_count = frames.min(64);
        let cpu1_fps = {
            let sub = &mut clean_all[..cpu1_count];
            let mut workspace = decoder.workspace();
            for f in sub.iter_mut() {
                std::hint::black_box(decoder.correct_in_place(f, &mut workspace).unwrap());
            }
            let mut best = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let mut clean = 0usize;
                for f in sub.iter_mut() {
                    let outcome = decoder.correct_in_place(f, &mut workspace).unwrap();
                    clean += usize::from(outcome == BchDecodeOutcome::NoErrors);
                }
                std::hint::black_box(clean);
                best = best.min(t0.elapsed().as_secs_f64());
            }
            cpu1_count as f64 / best
        };

        let cpu24_fps = {
            let run_pool = |words: &mut [BitVec]| {
                words
                    .par_iter_mut()
                    .map_init(
                        || decoder.workspace(),
                        |workspace, f| {
                            usize::from(
                                decoder.correct_in_place(f, workspace).unwrap()
                                    == BchDecodeOutcome::NoErrors,
                            )
                        },
                    )
                    .sum::<usize>()
            };
            run_pool(&mut clean_all[..frames]);
            let mut best = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let v = run_pool(&mut clean_all[..frames]);
                std::hint::black_box(v);
                best = best.min(t0.elapsed().as_secs_f64());
            }
            frames as f64 / best
        };

        let speedup_vs_1t = gpu_fps / cpu1_fps;
        let speedup_vs_24t = gpu_fps / cpu24_fps;

        println!("## Operating point (frames = {frames})");
        println!("GPU  syndrome fps : {gpu_fps:>14.1}");
        println!("CPU  1T  fps      : {cpu1_fps:>14.1}   (measured on {cpu1_count} frames; best existing CPU path)");
        println!("CPU {threads}T  fps      : {cpu24_fps:>14.1}   (context only)");
        println!("speedup vs 1T     : {speedup_vs_1t:>14.2}x   <-- [hard] gate (>= 5x vs best existing CPU path)");
        println!("speedup vs {threads}T     : {speedup_vs_24t:>14.2}x   (context only)");
        println!(
            "GATE (>= 5x vs 1T): {}",
            if speedup_vs_1t >= 5.0 { "PASS" } else { "FAIL" }
        );
        println!();

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
            let csub = &mut clean_all[..b];
            let mut cbest = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                let v: usize = csub
                    .par_iter_mut()
                    .map_init(
                        || decoder.workspace(),
                        |workspace, f| {
                            usize::from(
                                decoder.correct_in_place(f, workspace).unwrap()
                                    == BchDecodeOutcome::NoErrors,
                            )
                        },
                    )
                    .sum();
                std::hint::black_box(v);
                cbest = cbest.min(t0.elapsed().as_secs_f64());
            }
            let c = b as f64 / cbest;
            println!("{b:>8}  {g:>14.1}  {c:>14.1}  {:>11.2}x", g / c);
        }
        println!();

        // Host-side staging is timed in isolation; the remainder of the full GPU
        // call is H2D, kernel, D2H and rehydration.
        {
            let pop = &frames_all[..frames];
            let mut stage_best = f64::INFINITY;
            for _ in 0..repeats {
                let t0 = Instant::now();
                // A canonical word is already the device coefficient stream
                // (coordinate i is the coefficient of x^i), so staging is the
                // concatenation of the packed words.
                let wpf = n.div_ceil(64);
                let mut streams: Vec<u64> = Vec::with_capacity(frames * wpf);
                for frame in pop {
                    streams.extend_from_slice(&frame.words()[..wpf]);
                }
                std::hint::black_box(&streams);
                stage_best = stage_best.min(t0.elapsed().as_secs_f64());
            }
            let full_best = frames as f64 / gpu_fps;
            let stage_frac = stage_best / full_best * 100.0;
            println!("## Coarse phase split (frames = {frames})");
            println!(
                "host coeff staging: {:>10.3} ms  ({stage_frac:.1}% of full call)",
                stage_best * 1e3
            );
            println!(
                "device+xfer       : {:>10.3} ms  ({:.1}% of full call)",
                (full_best - stage_best) * 1e3,
                100.0 - stage_frac
            );
        }
    }
}
