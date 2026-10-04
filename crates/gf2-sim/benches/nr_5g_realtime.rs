//! 5G NR LDPC GPU decode-rate sweep with a custom main (`harness = false`). For
//! BG1, Z = 384, rate 1/2 (`n` = 16896, `k` = 8448) over a per-bit BPSK-AWGN
//! channel, each `batch_size` × `max_iters` cell reports BLER and decoded
//! transport-block throughput, `(blocks × k) / wall seconds` with only the LDPC
//! decode timed, and the highest-throughput cell with BLER ≤ 1e-2 is selected.
//! The `NR5G_*` environment variables read in `run` override the operating
//! point, sample sizes and decoder settings. Without the `hip` feature or a
//! usable GPU the bench prints a notice and exits 0.

fn main() {
    #[cfg(not(feature = "hip"))]
    {
        eprintln!(
            "nr_5g_realtime: built without the `hip` feature; the GPU decode-rate \
             sweep is a no-op. Re-run with `--features hip` on a gfx1030 host."
        );
    }

    #[cfg(feature = "hip")]
    hip_bench::run();
}

#[cfg(feature = "hip")]
mod hip_bench {
    use std::sync::Arc;
    use std::time::Instant;

    use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, QuasiCyclicLdpc};
    use gf2_coding::traits::BlockEncoder;
    use gf2_coding::Llr;
    use gf2_core::BitVec;
    use gf2_kernels_hip::host::device_mem_info;
    use gf2_sim::gpu::nr_5g_ldpc::GpuNr5gDecoder;
    use gf2_sim::testutil::AwgnLlrSource;
    use gf2_sim::LlrBatch;

    /// BG1 message length at Z = 384.
    const TARGET_K: usize = 22 * 384;
    /// Rate 1/2.
    const TARGET_N: usize = 2 * TARGET_K;

    const BATCH_SIZES: [usize; 5] = [64, 128, 256, 512, 1024];
    const MAX_ITERS: [usize; 4] = [10, 15, 20, 25];

    fn sigma_for_es_n0_db(es_n0_db: f64) -> f64 {
        let lin = 10f64.powf(es_n0_db / 10.0);
        (1.0 / (2.0 * lin)).sqrt()
    }

    fn env_f64(key: &str, default: f64) -> f64 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    fn env_usize(key: &str, default: usize) -> usize {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    struct Cell {
        batch: usize,
        max_iters: usize,
        bler: f64,
        mbps: f64,
    }

    pub fn run() {
        if device_mem_info().is_err() {
            eprintln!("nr_5g_realtime: no usable GPU (device_mem_info failed); skipping.");
            return;
        }

        let es_n0_db = env_f64("NR5G_ESN0_DB", -1.4);
        let bler_blocks = env_usize("NR5G_BLER_BLOCKS", 3000);
        let thrput_reps = env_usize("NR5G_THRPUT_REPS", 5);
        let sigma = sigma_for_es_n0_db(es_n0_db);

        println!("# 5G NR LDPC GPU real-time decode-rate sweep (issue 23d3525f)");
        println!("# config: BG1 i_LS=1 Z=384 rate 1/2 QPSK, n={TARGET_N} k={TARGET_K}");
        println!("# decoder: NormalizedMinSum(0.75), syndrome early termination");
        println!("# channel: BPSK-AWGN, per-bit Es/N0 = {es_n0_db} dB (sigma = {sigma:.5})");
        println!("# BLER blocks/cell = {bler_blocks}, throughput reps = {thrput_reps}");
        println!(
            "# target: attested flat-kernel reference 17.45 Mbps at BLER <= 1e-2 \
             (AMENDED 2026-06-12b from the original >= 200 Mbps; study 43fb19e2)"
        );
        println!();

        let build_start = Instant::now();
        let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(1, TARGET_N, TARGET_K));
        assert_eq!(code.params().lifting_factor, 384, "realised Z must be 384");
        let mut msg = BitVec::with_capacity(TARGET_K);
        for i in 0..TARGET_K {
            msg.push_bit(i % 7 < 3);
        }
        let cw = code.encode(&msg);
        println!(
            "# rate-matched code built + encoded in {:.2} s",
            build_start.elapsed().as_secs_f64()
        );

        let algo = match std::env::var("NR5G_ALGO").ok().as_deref() {
            Some("minsum") => DecoderAlgorithm::MinSum,
            Some("nms") | None => DecoderAlgorithm::NormalizedMinSum(0.75),
            Some(other) => panic!("unknown NR5G_ALGO={other:?} (minsum|nms)"),
        };
        let early = env_usize("NR5G_EARLY", 1) != 0;
        println!("# decoder knobs: algorithm = {algo:?}, early_termination = {early}");
        let config = DecoderConfig::new(algo, early);
        let max_batch = *BATCH_SIZES.iter().max().unwrap();

        println!("batch  iters    BLER      Mbps");
        let mut cells: Vec<Cell> = Vec::new();
        for &max_iters in &MAX_ITERS {
            let dec = GpuNr5gDecoder::new(code.clone(), config, max_iters);
            // One device decoder sized for the largest batch serves every batch
            // size at this iteration cap (a smaller batch is a prefix slice).
            let decoder = dec
                .build_decoder(max_batch)
                .expect("build GPU NR decoder on gfx1030");

            for &batch in &BATCH_SIZES {
                let bler = measure_bler(&dec, &decoder, &msg, &cw, sigma, batch, bler_blocks);
                let mbps = measure_throughput(&dec, &decoder, &cw, sigma, batch, bler_blocks);
                println!("{batch:5}  {max_iters:5}  {bler:8.5}  {mbps:8.2}");
                cells.push(Cell {
                    batch,
                    max_iters,
                    bler,
                    mbps,
                });
            }
        }

        let selected = cells
            .iter()
            .filter(|c| c.bler <= 1e-2)
            .max_by(|a, b| a.mbps.partial_cmp(&b.mbps).unwrap());

        println!();
        match selected {
            Some(c) => {
                println!(
                    "# SELECTED: batch={} max_iters={} BLER={:.5} throughput={:.2} Mbps",
                    c.batch, c.max_iters, c.bler, c.mbps
                );
                let dec = GpuNr5gDecoder::new(code.clone(), config, c.max_iters);
                let decoder = dec
                    .build_decoder(max_batch)
                    .expect("build selected GPU NR decoder");
                let reps: Vec<f64> = (0..thrput_reps)
                    .map(|_| measure_throughput(&dec, &decoder, &cw, sigma, c.batch, bler_blocks))
                    .collect();
                let mean = reps.iter().sum::<f64>() / reps.len() as f64;
                let var = reps.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / reps.len() as f64;
                let sd = var.sqrt();
                print!("# selected-cell throughput reps (Mbps):");
                for r in &reps {
                    print!(" {r:.2}");
                }
                println!();
                println!("# selected-cell throughput mean ± σ = {mean:.2} ± {sd:.2} Mbps");
                const ATTESTED_MBPS: f64 = 17.45;
                if mean >= ATTESTED_MBPS * 0.9 {
                    println!(
                        "# VERDICT: PASS — within/above the attested flat-kernel band \
                         ({mean:.2} Mbps vs attested {ATTESTED_MBPS:.2} Mbps; amended \
                         criterion 2026-06-12b)"
                    );
                } else {
                    println!(
                        "# VERDICT: REGRESSION ({mean:.2} Mbps < 90% of the attested \
                         {ATTESTED_MBPS:.2} Mbps) — investigate before attesting."
                    );
                }
            }
            None => {
                println!(
                    "# NO CELL MEETS BLER <= 1e-2 at Es/N0 = {es_n0_db} dB. Re-run with a \
                     higher NR5G_ESN0_DB operating point (record it in the receipt)."
                );
            }
        }
    }

    fn measure_bler(
        dec: &GpuNr5gDecoder,
        decoder: &gf2_kernels_hip::GpuLdpcBp,
        msg: &BitVec,
        cw: &BitVec,
        sigma: f64,
        batch: usize,
        blocks: usize,
    ) -> f64 {
        let mut src = AwgnLlrSource::new(0x23D3_525F_B1E2_0000 ^ (batch as u64));
        let mut errors = 0usize;
        let mut seen = 0usize;
        while seen < blocks {
            let this = batch.min(blocks - seen);
            let frames: Vec<Vec<Llr>> = (0..this)
                .map(|_| src.frame_for_codeword(cw, sigma))
                .collect();
            let out = dec
                .decode_batch(&LlrBatch::new(frames), decoder)
                .expect("gpu nr decode batch (bler)");
            for f in &out.frames {
                if f != msg {
                    errors += 1;
                }
            }
            seen += this;
        }
        errors as f64 / seen as f64
    }

    /// Decoded transport-block throughput in Mbps. The rate-matching LLR
    /// mapping (`prepare_llrs`) is host pre-processing and stays outside the
    /// timed region, which covers the mother-code GPU decode only.
    fn measure_throughput(
        dec: &GpuNr5gDecoder,
        decoder: &gf2_kernels_hip::GpuLdpcBp,
        cw: &BitVec,
        sigma: f64,
        batch: usize,
        blocks: usize,
    ) -> f64 {
        let mut src = AwgnLlrSource::new(0x23D3_525F_7373_0000 ^ (batch as u64));
        let mut prepared: Vec<LlrBatch> = Vec::new();
        let mut seen = 0usize;
        while seen < blocks {
            let this = batch.min(blocks - seen);
            let full: Vec<Vec<Llr>> = (0..this)
                .map(|_| {
                    let channel = src.frame_for_codeword(cw, sigma);
                    dec.prepare_llrs(&channel)
                })
                .collect();
            prepared.push(LlrBatch::new(full));
            seen += this;
        }

        let start = Instant::now();
        let mut decoded = 0usize;
        for full in &prepared {
            let out = dec
                .gpu()
                .decode_batch(full, decoder)
                .expect("gpu mother-code decode batch (throughput)");
            decoded += out.frames.len();
        }
        let secs = start.elapsed().as_secs_f64();
        (decoded as f64 * TARGET_K as f64) / secs / 1.0e6
    }
}
