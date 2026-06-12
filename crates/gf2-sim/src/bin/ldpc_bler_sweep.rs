//! LDPC-only AWGN-BPSK BLER sweep for the external-library comparison harness.
//!
//! Part of issue `18e69a1a` (`dev/benchmarks/gf2-sim/comparison/`). This is the
//! `gf2-sim`-side curve generator for the side-by-side comparison against
//! aff3ct. It runs the **isolated LDPC decoder** over an AWGN-BPSK channel —
//! deliberately *no* QAM, *no* bit interleaver, *no* BCH outer code — so the
//! comparison is apples-to-apples with an aff3ct run on the same parity-check
//! matrix and the same channel (`--mdm-type BPSK`, `--src-type AZCW`).
//!
//! # Why all-zero codeword (AZCW)?
//!
//! Both sides transmit the all-zero codeword (always a valid codeword of any
//! linear code) over AWGN-BPSK. This removes the encoder from the comparison
//! entirely: aff3ct's systematic encoder and `gf2-coding`'s IRA/RU encoder
//! produce *different* codewords for a nonzero message even from the same `H`,
//! which would make a nonzero-message BLER comparison sensitive to the encoder
//! rather than the decoder. With AZCW both sides decode noisy realisations of
//! the same transmitted word and a **frame error** is "decoder output is not
//! all-zero on the K message bits" — aff3ct's exact default FER definition.
//!
//! # Channel and LLR convention
//!
//! BPSK maps bit `b -> 1 - 2b` (all-zero codeword -> every symbol `+1`). The
//! AWGN sample is `r = +1 + N(0, sigma)`, with the channel LLR `2r/N0`,
//! `N0 = 2·sigma²`. The noise std maps from **Es/N0** (energy per *coded*
//! symbol over noise PSD): `sigma = sqrt(1 / (2 · 10^(EsN0_dB/10)))` for unit
//! symbol energy `Es = 1`. This matches aff3ct's `--sim-noise-type ESN0`
//! with `--mdm-type BPSK` (Es is the BPSK symbol energy, here 1). The CSV
//! header records the convention. The deterministic noise source is
//! [`gf2_sim::testutil::AwgnLlrSource`] (the SSOT SplitMix64 + Box-Muller
//! channel-LLR generator); the per-Es/N0-point seed is `base_seed ^ point_idx`
//! so points are independent yet reproducible.
//!
//! # Codes
//!
//! Selected by `--code`, matching `export_alist`'s exports (the AList fed to
//! aff3ct is the same `H`):
//!
//! * `dvb-t2-r12` — `LdpcCode::dvb_t2_normal(Rate1_2)` (N = 64800, K = 32400).
//! * `nr-bg1-r12` — the mother code of
//!   `QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448)` (BG1, Z = 384;
//!   N = 68·384 = 26112, K = 22·384 = 8448). The comparison decodes the
//!   mother code directly (matching the exported AList), not the rate-matched
//!   short code.
//!
//! # Decoder
//!
//! Normalized min-sum (NMS) with scale 0.75 and early termination, the 5G NR
//! default and a standard DVB-T2 choice — matched on the aff3ct side with
//! `--dec-implem NMS --dec-type BP_FLOODING` (flooding schedule) and the same
//! `--dec-ite` cap and `--dec-norm-factor 0.75`.
//!
//! # Output
//!
//! A CSV with header `es_n0_db,gf2_sim_bler,gf2_sim_fps` (the columns the
//! comparison driver merges with the aff3ct columns into the committed
//! `*-vs-aff3ct.csv`). One row per Es/N0 point.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --features test-support --bin ldpc_bler_sweep -- \
//!     --code dvb-t2-r12 --esn0-range 0.8:1.6:0.2 \
//!     --max-frames 20000 --target-errors 200 --max-iter 50 \
//!     --seed 42 --output gf2_dvb.csv
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rayon::prelude::*;

use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcDecoder, QuasiCyclicLdpc};
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_coding::{CodeRate, LdpcCode};
use gf2_sim::testutil::AwgnLlrSource;

/// Default NMS scale (5G NR standard; also a common DVB-T2 choice).
const NMS_SCALE: f32 = 0.75;

/// Which committed comparison code to sweep (matches `export_alist`).
#[derive(Clone, Copy)]
enum Code {
    DvbT2R12,
    NrBg1R12,
}

impl Code {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "dvb-t2-r12" => Ok(Self::DvbT2R12),
            "nr-bg1-r12" => Ok(Self::NrBg1R12),
            other => Err(format!(
                "unknown --code '{other}' (expected 'dvb-t2-r12' or 'nr-bg1-r12')"
            )),
        }
    }

    /// Builds the `LdpcCode` whose `H` matches the exported AList.
    fn build(self) -> LdpcCode {
        match self {
            Self::DvbT2R12 => LdpcCode::dvb_t2_normal(CodeRate::Rate1_2),
            Self::NrBg1R12 => {
                let rm = QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448);
                rm.mother_code().clone()
            }
        }
    }
}

/// Parsed CLI configuration.
struct Cfg {
    code: Code,
    esn0: Vec<f64>,
    max_frames: u64,
    target_errors: u64,
    max_iter: usize,
    seed: u64,
    output: PathBuf,
}

/// Parses `a:b:step` into an inclusive `f64` range (same convention as the
/// DVB-T2 campaign binary's `--esn0-range`).
fn parse_range(s: &str) -> Result<Vec<f64>, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("--esn0-range '{s}' must be 'start:stop:step'"));
    }
    let start: f64 = parts[0].parse().map_err(|_| "bad start".to_string())?;
    let stop: f64 = parts[1].parse().map_err(|_| "bad stop".to_string())?;
    let step: f64 = parts[2].parse().map_err(|_| "bad step".to_string())?;
    if step <= 0.0 {
        return Err("step must be positive".to_string());
    }
    let n = ((stop - start) / step).round() as i64;
    Ok((0..=n)
        .map(|i| start + (i as f64) * step)
        .filter(|&v| v <= stop + step * 1e-3)
        .collect())
}

fn parse_args() -> Cfg {
    let mut code: Option<Code> = None;
    let mut esn0: Option<Vec<f64>> = None;
    let mut max_frames: u64 = 20000;
    let mut target_errors: u64 = 200;
    let mut max_iter: usize = 50;
    let mut seed: u64 = 42;
    let mut output: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || {
            args.next()
                .unwrap_or_else(|| die(&format!("{a} requires a value")))
        };
        match a.as_str() {
            "--code" => {
                code = Some(Code::parse(&next()).unwrap_or_else(|e| die(&e)));
            }
            "--esn0-range" => {
                esn0 = Some(parse_range(&next()).unwrap_or_else(|e| die(&e)));
            }
            "--max-frames" => {
                max_frames = next().parse().unwrap_or_else(|_| die("bad --max-frames"))
            }
            "--target-errors" => {
                target_errors = next()
                    .parse()
                    .unwrap_or_else(|_| die("bad --target-errors"))
            }
            "--max-iter" => max_iter = next().parse().unwrap_or_else(|_| die("bad --max-iter")),
            "--seed" => seed = next().parse().unwrap_or_else(|_| die("bad --seed")),
            "--output" => output = Some(PathBuf::from(next())),
            "-h" | "--help" => {
                println!(
                    "ldpc_bler_sweep --code <dvb-t2-r12|nr-bg1-r12> \\\n\
                     \t--esn0-range start:stop:step --max-frames N --target-errors N \\\n\
                     \t--max-iter N --seed S --output file.csv"
                );
                std::process::exit(0);
            }
            other => die(&format!("unknown argument '{other}'")),
        }
    }

    Cfg {
        code: code.unwrap_or_else(|| die("--code is required")),
        esn0: esn0.unwrap_or_else(|| die("--esn0-range is required")),
        max_frames,
        target_errors,
        max_iter,
        seed,
        output: output.unwrap_or_else(|| die("--output is required")),
    }
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

/// Es/N0 (dB) -> AWGN noise std for unit-energy BPSK (`Es = 1`):
/// `sigma = sqrt(1 / (2 · 10^(EsN0/10)))`. Equivalent to `N0 = 1/10^(EsN0/10)`
/// and `sigma² = N0/2`.
fn esn0_db_to_sigma(esn0_db: f64) -> f64 {
    let esn0_lin = 10f64.powf(esn0_db / 10.0);
    (1.0 / (2.0 * esn0_lin)).sqrt()
}

/// Runs one Es/N0 point: decodes frames until `target_errors` frame errors or
/// `max_frames` frames, whichever first. Returns `(bler, frames, errors, fps)`.
///
/// Frames are split across rayon workers in fixed contiguous chunks; each
/// worker owns an independent `AwgnLlrSource` seeded by `point_seed` mixed with
/// the worker's first global frame index, so the result is reproducible and
/// the work parallelises. Because the run stops early on the error budget, the
/// frame count is the global frame index at which the budget was hit (rounded
/// up to a chunk boundary) — recorded exactly in the CSV.
fn run_point(
    code: &LdpcCode,
    point_seed: u64,
    sigma: f64,
    max_frames: u64,
    target_errors: u64,
    max_iter: usize,
) -> (f64, u64, u64, f64) {
    let n = code.n();
    let k = code.k();
    let chunk: u64 = 256;

    let start = Instant::now();
    let mut frames_done: u64 = 0;
    let mut errors: u64 = 0;

    // Process in waves of `chunk * num_workers` frames so we can check the
    // error budget between waves while still parallelising each wave.
    let workers = rayon::current_num_threads().max(1) as u64;
    let wave = chunk * workers;

    while frames_done < max_frames && errors < target_errors {
        let wave_start = frames_done;
        let wave_end = (wave_start + wave).min(max_frames);
        let wave_len = wave_end - wave_start;

        // Each worker handles one `chunk`-sized slice of this wave.
        let slice_starts: Vec<u64> = (wave_start..wave_end).step_by(chunk as usize).collect();
        let wave_errors: u64 = slice_starts
            .par_iter()
            .map(|&slice_start| {
                let slice_end = (slice_start + chunk).min(wave_end);
                // Per-slice independent, reproducible noise stream.
                let mut src = AwgnLlrSource::new(point_seed ^ slice_start.wrapping_mul(0x9E37));
                let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(NMS_SCALE), true);
                let mut dec = LdpcDecoder::with_config(code.clone(), cfg);
                let mut local_err = 0u64;
                for _ in slice_start..slice_end {
                    let llrs = src.frame_all_zero(n, sigma);
                    let res = dec.decode_iterative(&llrs, max_iter);
                    // Frame error: any of the K message bits is nonzero.
                    debug_assert_eq!(res.decoded_bits.len(), k);
                    if (0..k).any(|i| res.decoded_bits.get(i)) {
                        local_err += 1;
                    }
                }
                local_err
            })
            .sum();

        frames_done += wave_len;
        errors += wave_errors;
    }

    let secs = start.elapsed().as_secs_f64();
    let fps = if secs > 0.0 {
        frames_done as f64 / secs
    } else {
        0.0
    };
    let bler = if frames_done > 0 {
        errors as f64 / frames_done as f64
    } else {
        0.0
    };
    (bler, frames_done, errors, fps)
}

fn main() {
    let cfg = parse_args();
    let code = cfg.code.build();
    eprintln!(
        "ldpc_bler_sweep: code N={} K={} (rate {:.4}), {} Es/N0 points, \
         max_frames={} target_errors={} max_iter={} seed={}",
        code.n(),
        code.k(),
        code.k() as f64 / code.n() as f64,
        cfg.esn0.len(),
        cfg.max_frames,
        cfg.target_errors,
        cfg.max_iter,
        cfg.seed,
    );

    let mut rows: Vec<(f64, f64, f64)> = Vec::with_capacity(cfg.esn0.len());
    for (idx, &esn0_db) in cfg.esn0.iter().enumerate() {
        let sigma = esn0_db_to_sigma(esn0_db);
        let point_seed = cfg.seed ^ (idx as u64).wrapping_mul(0x1234_5678_9ABC_DEF1);
        let (bler, frames, errors, fps) = run_point(
            &code,
            point_seed,
            sigma,
            cfg.max_frames,
            cfg.target_errors,
            cfg.max_iter,
        );
        eprintln!(
            "  Es/N0={esn0_db:>5.2} dB  BLER={bler:.3e}  frames={frames}  \
             errors={errors}  fps={fps:.1}"
        );
        rows.push((esn0_db, bler, fps));
    }

    let mut f = std::fs::File::create(&cfg.output)
        .unwrap_or_else(|e| die(&format!("cannot create {}: {e}", cfg.output.display())));
    writeln!(f, "es_n0_db,gf2_sim_bler,gf2_sim_fps").unwrap();
    for (esn0_db, bler, fps) in &rows {
        writeln!(f, "{esn0_db},{bler:.6e},{fps:.3}").unwrap();
    }
    eprintln!("wrote {}", cfg.output.display());
}
