//! DVB-T2 BICM AWGN campaign runner — `gf2-sim` pipeline edition.
//!
//! Runs the full DVB-T2 BICM AWGN simulation for one `(rate, modulation)`
//! configuration, producing a CSV curve, a README, and per-SNR checkpoint
//! files.
//!
//! Six invocations (3 rates × 2 modulations) reproduce every curve required
//! by epic 2928ccce.
//!
//! # Migration (jit:bbf6b6ee, wave D.2 of epic gf2-sim `f9717e7e`)
//!
//! This is the **migrated** campaign binary. It drives the simulation through
//! the [`gf2_sim`] hybrid pipeline ([`Pipeline::dvb_t2`] typestate preset +
//! `Scheduler::run_sweep_checkpointed` for production sweeps — called directly
//! rather than through the `Pipeline::run_checkpointed` thin wrapper so the
//! per-frame `frame_observer` can emit live `campaign_heartbeat` tracing
//! events — and `Pipeline::run_with_decoder` for calibration), replacing the
//! legacy
//! `gf2_coding::simulation::SimulationRunner::run_with_decoder` call site of
//! the original binary, which lived at
//! `crates/gf2-coding/src/bin/dvb_t2_awgn_campaign.rs` until the
//! user-approved bbf6b6ee amendment DELETED that binary file (this one is THE
//! campaign binary; `cargo run --release --bin dvb_t2_awgn_campaign` resolves
//! unambiguously from the workspace root). The legacy
//! `gf2_coding::simulation::SimulationRunner` LIBRARY path is retained — only
//! the binary moved; a binary inside `gf2-coding` cannot call `gf2-sim`
//! (dependency cycle: `gf2-sim` depends on `gf2-coding`).
//!
//! The new pipeline parallelises every SNR point across rayon workers (and,
//! with `--gpu` on a `--features hip` build, offloads the heavy LDPC BP +
//! demap stages to the HIP device), so it is materially faster than the legacy
//! single-thread path (see `dev/benchmarks/gf2-sim/parallelism-receipts.md`).
//!
//! # BICM chain (per frame)
//!
//! ```text
//! BBFRAME → BCH+LDPC encode → bit interleave → QAM map → AWGN
//!                                                            ↓
//! BBFRAME ← BCH+LDPC decode ← bit deinterleave ← QAM demap
//! ```
//!
//! The pipeline owns this chain: the [`Pipeline::dvb_t2`] preset wires the
//! seven (CPU) or eight (GPU) stages, the AWGN channel injects noise from the
//! per-worker ChaCha20 stream, and the within-SNR frame-parallel executor
//! (or the hybrid CPU+GPU scheduler under `--gpu`) drives the frames. The
//! binary itself is a thin CLI front-end: parse args → build the pipeline via
//! the typestate preset → set the sweep on its [`PipelineConfig`] → run →
//! post-process the [`SimulationResults`] into the campaign CSV.
//!
//! # Es/N0 vs Eb/N0
//!
//! Unlike the legacy binary (which converted Es/N0 → Eb/N0 for the legacy
//! `SimulationConfig`), the `gf2-sim` pipeline's `esn0_db_points` *are* Es/N0
//! in dB directly: the frame kernel
//! [`DvbT2BicmFrameSim`](gf2_sim::frame_sim::DvbT2BicmFrameSim) takes Es/N0 and
//! derives sigma from it. The CLI `--esn0-range` values are therefore fed to
//! the config verbatim — no unit conversion happens in this binary.
//!
//! # Usage
//!
//! ## Smoke run (3 SNR points, small frame budget)
//!
//! ```bash
//! cargo run -p gf2-sim --release --bin dvb_t2_awgn_campaign -- \
//!     --rate 1/2 --modulation 16qam \
//!     --esn0-range 4.0:5.0:0.5 \
//!     --max-frames 100 --target-errors 5 \
//!     --output-dir /tmp/dvb_smoke --seed 42
//! ```
//!
//! ## Production run
//!
//! ```bash
//! cargo run -p gf2-sim --release --bin dvb_t2_awgn_campaign -- \
//!     --rate 1/2 --modulation 16qam \
//!     --esn0-range 4.0:7.0:0.5 \
//!     --target-errors 100 --max-frames 10000000 \
//!     --output-dir /tmp/dvb_r12_16qam --seed 42
//! ```
//!
//! ## Resuming after interruption
//!
//! ```bash
//! cargo run -p gf2-sim --release --bin dvb_t2_awgn_campaign -- \
//!     --rate 1/2 --modulation 16qam \
//!     --esn0-range 4.0:7.0:0.5 \
//!     --target-errors 100 --max-frames 10000000 \
//!     --output-dir /tmp/dvb_r12_16qam --seed 42 --resume
//! ```
//!
//! ## GPU offload (requires `--features hip`)
//!
//! ```bash
//! cargo run -p gf2-sim --release --features hip --bin dvb_t2_awgn_campaign -- \
//!     --rate 1/2 --modulation 16qam \
//!     --esn0-range 4.0:7.0:0.5 --target-errors 100 \
//!     --output-dir /tmp/dvb_r12_16qam --seed 42 --gpu
//! ```
//!
//! On a build **without** `--features hip`, passing `--gpu` returns a clear
//! error before any work runs.
//!
//! ## Calibration sweep
//!
//! ```bash
//! cargo run -p gf2-sim --release --bin dvb_t2_awgn_campaign -- \
//!     --calibrate --rate 1/2 --modulation 16qam \
//!     --output-dir /tmp/dvb_calib --seed 42 --calibrate-frames 1000
//! ```
//!
//! # Output layout
//!
//! Under `<output-dir>/`:
//! - `curve_<rate>_<mod>.csv` — per-SNR results (columns: `es_n0_db, fer, ber,
//!   frames, errors, mean_iters, wall_seconds`). Two columns are inherently
//!   non-deterministic across separate process invocations and are excluded
//!   from any byte-identity assertion: `wall_seconds` (average wall-clock time
//!   per SNR point for this invocation) and `ber` (a non-associative f32
//!   horizontal reduction over LDPC belief-propagation output; design doc §11
//!   always-excluded). The discrete columns `es_n0_db`, `fer`, `frames`,
//!   `errors`, and `mean_iters` are deterministic and asserted byte-identical
//!   across two runs at the same seed by the within-pipeline byte-identity
//!   integration test (`tests/campaign_byte_identity.rs`).
//!
//!   **`mean_iters` legacy-compatibility note**: the legacy binary recorded
//!   `iterations: 1` for converged frames (a quirk of its `DecoderResult`
//!   sentinel). This pipeline records the real BP iteration depth. Old-vs-new
//!   `mean_iters` curves are therefore **not comparable**; use only
//!   `fer`/`ber`/`frames`/`errors` for cross-binary comparison.
//! - `tracing.jsonl` — structured JSON-lines tracing log (one JSON object per
//!   line). Written unconditionally (both production and calibration runs) by
//!   the [`gf2_sim::observability::install_campaign_subscriber`] machinery
//!   (a **process-global** subscriber, so events from the executor's rayon /
//!   helper threads land too). The cross-epic e4849f07 multi-day sweep monitor
//!   watches this file. Events (each carries a matching `event_type` field):
//!   - `campaign_start` — once, at sweep start.
//!   - `campaign_heartbeat` — **live**, from the executor's per-frame
//!     `frame_observer`, every `--heartbeat-frames` observed frames per SNR
//!     point (production runs only; calibration has no checkpointed path).
//!     Approximate progress, not exact accounting: on the hybrid GPU path,
//!     frames in a batch discarded at an interrupt are observed-but-unrecorded
//!     and re-observed on resume, and the counter restarts each invocation.
//!   - `snr_point_completed` — one per point with `es_n0_db`/`fer`/`frames`/
//!     `errors`/`mean_iters`/`wall_seconds`, emitted **post-sweep** (after the
//!     run returns, when per-point results exist), not live at each boundary.
//! - `README.md` — invocation, seed, host info, total wall-clock.
//! - `checkpoints/` — per-SNR JSON files (v2 schema with BLAKE3-verified
//!   config hash), written by the pipeline's checkpoint subsystem.
//! - `calibration/calibration_<rate>_<mod>.csv` (only when `--calibrate`).
//!
//! The CSV schema is **identical** to the legacy binary's, so
//! `dev/benchmarks/dvb_t2_awgn/plot.py` keeps working unchanged.
//!
//! # Plotting
//!
//! After running the campaign, produce a PNG overlay with simulated FER and
//! ETSI TR 102 831 reference points using:
//!
//! ```bash
//! python3 dev/benchmarks/dvb_t2_awgn/plot.py \
//!     --curve-csv <output-dir>/curve_<rate>_<mod>.csv \
//!     --reference-toml crates/gf2-coding/data/dvb_t2_tr102831_reference.toml \
//!     --output <output-dir>/curve_<rate>_<mod>.png
//! ```

#![deny(unsafe_code)]

use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use gf2_coding::dvb_t2_bicm_harness::{mod_str, rate_display, rate_underscore};
use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;

use gf2_sim::executor::{Scheduler, SimulationResults, SnrPointResult};
use gf2_sim::observability::install_campaign_subscriber;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::Pipeline;

// ---------------------------------------------------------------------------
// CLI argument definitions (parsed manually, no clap dependency needed).
// ---------------------------------------------------------------------------

/// Parsed CLI arguments.
struct Args {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    /// Es/N0 sweep: start, stop (inclusive), step.
    esn0_range: Option<(f64, f64, f64)>,
    target_errors: usize,
    max_frames: usize,
    seed: u64,
    output_dir: PathBuf,
    resume: bool,
    gpu: bool,
    strict_gpu: bool,
    calibrate: bool,
    calibrate_frames: usize,
    /// Optional explicit 3-point bracket [low, center, high] for calibration.
    calibrate_bracket: Option<[f64; 3]>,
    /// LDPC belief-propagation decoder configuration.
    decoder: DecoderConfig,
    /// QAM soft-demapping method.
    demap: DemapMethod,
    /// Within-SNR heartbeat cadence in frames: drives both the heartbeat
    /// checkpoint flush AND the `campaign_heartbeat` tracing event. Ignored
    /// (forced to 0) for calibration runs.
    heartbeat_frames: u64,
}

fn print_usage() {
    eprintln!(
        "Usage: dvb_t2_awgn_campaign [OPTIONS]\n\
         \n\
         Options:\n\
           --rate <1/2|2/3|3/4>            Code rate (required)\n\
           --modulation <16qam|64qam>       Modulation order (required)\n\
           --esn0-range <start:stop:step>   Es/N0 sweep in dB (mutually exclusive with --calibrate)\n\
           --target-errors <N>              Min frame errors per SNR [default: 100]\n\
           --max-frames <N>                 Max frames per SNR [default: 10000000]\n\
           --seed <u64>                     RNG seed [default: 0xC0DEF00D]\n\
           --output-dir <path>              Output directory (required)\n\
           --resume                         Resume from existing checkpoints\n\
           --gpu                            Offload LDPC BP + demap to the HIP device (requires --features hip)\n\
           --strict-gpu                     Promote GPU out-of-memory to a fatal error (no CPU fallback)\n\
           --calibrate                      Run calibration sweep instead of full campaign\n\
           --calibrate-frames <N>           Frames per calibration point [default: 1000]\n\
           --calibrate-bracket <a:b:c>      Custom 3-point Es/N0 bracket for calibration\n\
           --decoder <spec>                 LDPC decoder: minsum | nms:<alpha> | oms:<beta> | sumproduct [default: minsum]\n\
           --demap <method>                 QAM demap: maxlog | exactlogmap [default: maxlog]\n\
           --heartbeat-frames <N>           Within-SNR heartbeat cadence: checkpoint flush + campaign_heartbeat tracing event every N frames [default: 1000]\n\
         "
    );
}

fn parse_code_rate(s: &str) -> Result<CodeRate, String> {
    match s {
        "1/2" => Ok(CodeRate::Rate1_2),
        "2/3" => Ok(CodeRate::Rate2_3),
        "3/4" => Ok(CodeRate::Rate3_4),
        other => Err(format!(
            "Unknown code rate '{}'; supported: 1/2, 2/3, 3/4",
            other
        )),
    }
}

fn parse_modulation(s: &str) -> Result<DvbT2Modulation, String> {
    match s.to_lowercase().as_str() {
        "16qam" => Ok(DvbT2Modulation::Qam16),
        "64qam" => Ok(DvbT2Modulation::Qam64),
        other => Err(format!(
            "Unknown modulation '{}'; supported: 16qam, 64qam",
            other
        )),
    }
}

fn parse_esn0_range(s: &str) -> Result<(f64, f64, f64), String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("Expected <start>:<stop>:<step>, got '{}'", s));
    }
    let start: f64 = parts[0]
        .parse()
        .map_err(|_| format!("Cannot parse start '{}' as f64", parts[0]))?;
    let stop: f64 = parts[1]
        .parse()
        .map_err(|_| format!("Cannot parse stop '{}' as f64", parts[1]))?;
    let step: f64 = parts[2]
        .parse()
        .map_err(|_| format!("Cannot parse step '{}' as f64", parts[2]))?;
    if step <= 0.0 {
        return Err(format!("Step must be positive, got {}", step));
    }
    if stop < start {
        return Err(format!("Stop ({}) must be >= start ({})", stop, start));
    }
    Ok((start, stop, step))
}

fn parse_bracket(s: &str) -> Result<[f64; 3], String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("Expected <a:b:c>, got '{}'", s));
    }
    let a: f64 = parts[0]
        .parse()
        .map_err(|_| format!("Cannot parse '{}' as f64", parts[0]))?;
    let b: f64 = parts[1]
        .parse()
        .map_err(|_| format!("Cannot parse '{}' as f64", parts[1]))?;
    let c: f64 = parts[2]
        .parse()
        .map_err(|_| format!("Cannot parse '{}' as f64", parts[2]))?;
    Ok([a, b, c])
}

fn parse_decoder(s: &str) -> Result<DecoderConfig, String> {
    let lower = s.to_lowercase();
    let algorithm = match lower.split(':').collect::<Vec<_>>().as_slice() {
        ["minsum"] => DecoderAlgorithm::MinSum,
        ["sumproduct"] | ["spa"] => DecoderAlgorithm::SumProduct,
        ["nms", alpha] => {
            let a: f32 = alpha
                .parse()
                .map_err(|_| format!("Cannot parse nms alpha '{}' as f32", alpha))?;
            // DecoderConfig::new panics on out-of-range alpha; validate here
            // so the CLI returns a clean error instead.
            if !a.is_finite() || a <= 0.0 || a > 1.0 {
                return Err(format!(
                    "nms alpha must be finite and in (0.0, 1.0]; got {}",
                    a
                ));
            }
            DecoderAlgorithm::NormalizedMinSum(a)
        }
        ["oms", beta] => {
            let b: f32 = beta
                .parse()
                .map_err(|_| format!("Cannot parse oms beta '{}' as f32", beta))?;
            // DecoderConfig::new panics on negative or non-finite beta;
            // validate here so the CLI returns a clean error instead.
            if !b.is_finite() || b < 0.0 {
                return Err(format!("oms beta must be finite and >= 0.0; got {}", b));
            }
            DecoderAlgorithm::OffsetMinSum(b)
        }
        _ => {
            return Err(format!(
                "Unknown decoder '{}'; supported: minsum, nms:<alpha>, oms:<beta>, sumproduct",
                s
            ))
        }
    };
    Ok(DecoderConfig::new(algorithm, true))
}

fn parse_demap(s: &str) -> Result<DemapMethod, String> {
    match s.to_lowercase().as_str() {
        "maxlog" => Ok(DemapMethod::MaxLog),
        "exactlogmap" | "exact" => Ok(DemapMethod::ExactLogMap),
        other => Err(format!(
            "Unknown demap method '{}'; supported: maxlog, exactlogmap",
            other
        )),
    }
}

fn parse_args() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut rate: Option<CodeRate> = None;
    let mut modulation: Option<DvbT2Modulation> = None;
    let mut esn0_range: Option<(f64, f64, f64)> = None;
    let mut target_errors: usize = 100;
    let mut max_frames: usize = 10_000_000;
    let mut seed: u64 = 0xC0DE_F00D;
    let mut output_dir: Option<PathBuf> = None;
    let mut resume = false;
    let mut gpu = false;
    let mut strict_gpu = false;
    let mut calibrate = false;
    let mut calibrate_frames: usize = 1000;
    let mut calibrate_bracket: Option<[f64; 3]> = None;
    let mut decoder = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
    let mut demap = DemapMethod::MaxLog;
    let mut heartbeat_frames: u64 = 1000;

    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--rate" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--rate requires a value".to_string())?;
                rate = Some(parse_code_rate(s)?);
            }
            "--modulation" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--modulation requires a value".to_string())?;
                modulation = Some(parse_modulation(s)?);
            }
            "--esn0-range" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--esn0-range requires a value".to_string())?;
                esn0_range = Some(parse_esn0_range(s)?);
            }
            "--target-errors" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--target-errors requires a value".to_string())?;
                target_errors = s
                    .parse()
                    .map_err(|_| format!("Cannot parse '--target-errors {}' as usize", s))?;
            }
            "--max-frames" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--max-frames requires a value".to_string())?;
                max_frames = s
                    .parse()
                    .map_err(|_| format!("Cannot parse '--max-frames {}' as usize", s))?;
            }
            "--seed" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--seed requires a value".to_string())?;
                // Accept hex (0x...) or decimal.
                if s.starts_with("0x") || s.starts_with("0X") {
                    seed = u64::from_str_radix(&s[2..], 16)
                        .map_err(|_| format!("Cannot parse '--seed {}' as hex u64", s))?;
                } else {
                    seed = s
                        .parse()
                        .map_err(|_| format!("Cannot parse '--seed {}' as u64", s))?;
                }
            }
            "--output-dir" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--output-dir requires a value".to_string())?;
                output_dir = Some(PathBuf::from(s));
            }
            "--resume" => {
                resume = true;
            }
            "--gpu" => {
                gpu = true;
            }
            "--strict-gpu" => {
                strict_gpu = true;
            }
            "--calibrate" => {
                calibrate = true;
            }
            "--calibrate-frames" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--calibrate-frames requires a value".to_string())?;
                calibrate_frames = s
                    .parse()
                    .map_err(|_| format!("Cannot parse '--calibrate-frames {}' as usize", s))?;
            }
            "--calibrate-bracket" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--calibrate-bracket requires a value".to_string())?;
                calibrate_bracket = Some(parse_bracket(s)?);
            }
            "--decoder" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--decoder requires a value".to_string())?;
                decoder = parse_decoder(s)?;
            }
            "--demap" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--demap requires a value".to_string())?;
                demap = parse_demap(s)?;
            }
            "--heartbeat-frames" => {
                i += 1;
                let s = argv
                    .get(i)
                    .ok_or_else(|| "--heartbeat-frames requires a value".to_string())?;
                heartbeat_frames = s
                    .parse()
                    .map_err(|_| format!("Cannot parse '--heartbeat-frames {}' as u64", s))?;
                if heartbeat_frames == 0 {
                    return Err("--heartbeat-frames must be >= 1".to_string());
                }
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            unknown => {
                return Err(format!("Unknown argument: '{}'", unknown));
            }
        }
        i += 1;
    }

    let rate = rate.ok_or_else(|| "--rate is required".to_string())?;
    let modulation = modulation.ok_or_else(|| "--modulation is required".to_string())?;
    let output_dir = output_dir.ok_or_else(|| "--output-dir is required".to_string())?;

    if calibrate && esn0_range.is_some() {
        return Err("--calibrate and --esn0-range are mutually exclusive".to_string());
    }
    if !calibrate && esn0_range.is_none() {
        return Err("One of --esn0-range or --calibrate is required".to_string());
    }

    Ok(Args {
        rate,
        modulation,
        esn0_range,
        target_errors,
        max_frames,
        seed,
        output_dir,
        resume,
        gpu,
        strict_gpu,
        calibrate,
        calibrate_frames,
        calibrate_bracket,
        decoder,
        demap,
        heartbeat_frames,
    })
}

// ---------------------------------------------------------------------------
// Naming helpers (campaign-local aliases that forward to the shared harness).
// ---------------------------------------------------------------------------

fn rate_str(r: CodeRate) -> &'static str {
    rate_underscore(r)
}

fn curve_csv_name(rate: CodeRate, modulation: DvbT2Modulation) -> String {
    format!("curve_{}_{}.csv", rate_str(rate), mod_str(modulation))
}

fn calib_csv_name(rate: CodeRate, modulation: DvbT2Modulation) -> String {
    format!("calibration_{}_{}.csv", rate_str(rate), mod_str(modulation))
}

// ---------------------------------------------------------------------------
// Reference TOML: load default calibration bracket for a MODCOD.
//
// Centers derived from ETSI TR 102 831 Table 44 (AWGN C/N at BER=1e-7 after
// LDPC, Normal 64800-bit blocks) minus ~1.5 dB to estimate the Es/N0 at
// FER=1e-4 waterfall (the QEF threshold is at BER=1e-7 ≈ FER=1e-11 after BCH;
// waterfall is ~1-2 dB below the table C/N).
// ---------------------------------------------------------------------------

/// Returns `[low, center, high]` Es/N0 values for the calibration bracket.
///
/// The center value is derived from ETSI TR 102 831 Table 44 AWGN C/N at
/// BER = 1e-7 after LDPC (Normal frame, 64800 bits).
fn default_calibration_bracket(rate: CodeRate, modulation: DvbT2Modulation) -> [f64; 3] {
    // ETSI TR 102 831 Table 44 AWGN C/N at BER=1e-7 after LDPC (Normal frames):
    //   16-QAM 1/2: 6.0 dB
    //   16-QAM 2/3: 8.9 dB
    //   16-QAM 3/4: 10.0 dB
    //   64-QAM 1/2: 9.9 dB
    //   64-QAM 2/3: 13.5 dB
    //   64-QAM 3/4: 15.1 dB
    // The waterfall knee (FER~1e-2..1e-4) sits ~1.5 dB below the QEF C/N.
    let center = match (rate, modulation) {
        (CodeRate::Rate1_2, DvbT2Modulation::Qam16) => 5.5,
        (CodeRate::Rate2_3, DvbT2Modulation::Qam16) => 8.0,
        (CodeRate::Rate3_4, DvbT2Modulation::Qam16) => 9.0,
        (CodeRate::Rate1_2, DvbT2Modulation::Qam64) => 9.0,
        (CodeRate::Rate2_3, DvbT2Modulation::Qam64) => 12.5,
        (CodeRate::Rate3_4, DvbT2Modulation::Qam64) => 14.0,
        _ => 8.0,
    };
    [center - 1.0, center, center + 1.0]
}

// ---------------------------------------------------------------------------
// SNR range builder.
// ---------------------------------------------------------------------------

fn build_snr_range(start: f64, stop: f64, step: f64) -> Vec<f64> {
    let n = ((stop - start) / step).round() as usize + 1;
    (0..n)
        .map(|i| {
            let v = start + i as f64 * step;
            // Round to 6 decimal places to avoid floating-point accumulation.
            (v * 1_000_000.0).round() / 1_000_000.0
        })
        .filter(|&v| v <= stop + step * 0.001)
        .collect()
}

// ---------------------------------------------------------------------------
// Campaign CSV writer.
// ---------------------------------------------------------------------------

const CAMPAIGN_CSV_HEADER: &str = "es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds";

fn write_campaign_csv(
    path: &Path,
    points: &[(f64, f64, f64, u64, u64, f64, f64)],
) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "{CAMPAIGN_CSV_HEADER}")?;
    for &(es_n0_db, fer, ber, frames, errors, mean_iters, wall_seconds) in points {
        writeln!(
            f,
            "{},{},{},{},{},{},{}",
            es_n0_db, fer, ber, frames, errors, mean_iters, wall_seconds
        )?;
    }
    Ok(())
}

/// Projects one [`SnrPointResult`] into a campaign CSV row.
///
/// `ber` is the bit error rate `total_bit_errors / total_bits`; `wall_seconds`
/// is the per-point average passed in (the executor does not expose per-point
/// timing). `es_n0_db` comes from the requested sweep point (it equals the
/// result's `es_n0_db`, which the pipeline carries through verbatim).
fn point_to_csv_row(
    es_n0_db: f64,
    p: &SnrPointResult,
    wall_seconds: f64,
) -> (f64, f64, f64, u64, u64, f64, f64) {
    let ber = if p.total_bits > 0 {
        p.total_bit_errors as f64 / p.total_bits as f64
    } else {
        0.0
    };
    (
        es_n0_db,
        p.fer,
        ber,
        p.frames,
        p.errors,
        p.mean_iters,
        wall_seconds,
    )
}

// ---------------------------------------------------------------------------
// Host info helper.
// ---------------------------------------------------------------------------

fn host_info() -> (String, String) {
    let whoami = std::process::Command::new("whoami")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let uname = std::process::Command::new("uname")
        .arg("-a")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    (whoami, uname)
}

// ---------------------------------------------------------------------------
// README writer.
// ---------------------------------------------------------------------------

fn write_readme(path: &Path, args: &Args, snr_points: &[f64], total_wall_seconds: f64) {
    let invocation: Vec<String> = std::env::args().collect();
    let (whoami, uname) = host_info();
    let content = format!(
        "# DVB-T2 BICM AWGN Campaign\n\
         \n\
         ## Invocation\n\
         \n\
         ```\n\
         {}\n\
         ```\n\
         \n\
         ## Configuration\n\
         \n\
         - Rate: {}\n\
         - Modulation: {}\n\
         - Es/N0 range: {:.2} : {:.2} ({} points)\n\
         - Target errors: {}\n\
         - Max frames: {}\n\
         - Seed: {:#018x}\n\
         \n\
         ## Host\n\
         \n\
         - User: {}\n\
         - System: {}\n\
         \n\
         ## Wall-clock\n\
         \n\
         Total: {:.1}s ({:.1} min)\n\
         \n\
         ## Plotting\n\
         \n\
         ```bash\n\
         python3 dev/benchmarks/dvb_t2_awgn/plot.py \\\n\
             --curve-csv curve_{}_{}.csv \\\n\
             --reference-toml crates/gf2-coding/data/dvb_t2_tr102831_reference.toml \\\n\
             --output curve_{}_{}.png\n\
         ```\n",
        invocation.join(" "),
        rate_display(args.rate),
        mod_str(args.modulation),
        snr_points.first().copied().unwrap_or(0.0),
        snr_points.last().copied().unwrap_or(0.0),
        snr_points.len(),
        args.target_errors,
        args.max_frames,
        args.seed,
        whoami,
        uname,
        total_wall_seconds,
        total_wall_seconds / 60.0,
        rate_str(args.rate),
        mod_str(args.modulation),
        rate_str(args.rate),
        mod_str(args.modulation),
    );
    if let Err(e) = std::fs::write(path, content) {
        eprintln!("Warning: failed to write README.md: {e}");
    }
}

// ---------------------------------------------------------------------------
// Pipeline construction.
// ---------------------------------------------------------------------------

/// Builds the DVB-T2 BICM pipeline for the campaign's MODCOD/decoder/demap.
///
/// Uses the lowest Es/N0 sweep point as the *channel* Es/N0 the preset builds
/// the AWGN stage and demapper noise variance from; the per-point Es/N0 sweep
/// is then applied through `config.esn0_db_points` and the executor rebuilds
/// the frame kernel per point from the [`RunPlan`](gf2_sim::executor::RunPlan)
/// (so the channel value the preset captures here does not pin the sweep —
/// only the `(rate, modulation, decoder, demap)` plan does).
///
/// All CLI run-control knobs are wired onto the built pipeline's
/// [`PipelineConfig`](gf2_sim::PipelineConfig) here: `esn0_db_points`,
/// `target_errors`, `max_frames`, `seed`, **`strict_gpu`** (from
/// `args.strict_gpu`), **`gpu_enabled`** (from `args.gpu`),
/// `tracing_log_path`, `checkpoint_dir`, and the heartbeat cadence. This is
/// the single CLI→config wiring point, exercised directly by the
/// `strict_gpu_flag_wires_to_config` and `gpu_enabled_flag_wires_to_config`
/// unit tests.
///
/// # Arguments
///
/// * `args` — the parsed CLI arguments.
/// * `esn0_points` — the resolved Es/N0 sweep points (dB).
/// * `target_errors` — the per-SNR frame-error early-exit budget (`0` disables).
/// * `max_frames` — the per-SNR maximum frame budget.
/// * `checkpoint_dir` — the per-SNR checkpoint directory, or `None`.
/// * `heartbeat_every_frames` — the within-SNR checkpoint cadence (`0` off).
/// * `tracing_log_path` — JSON-lines sink for tracing events, or `None`.
fn build_configured_pipeline(
    args: &Args,
    esn0_points: &[f64],
    target_errors: usize,
    max_frames: usize,
    checkpoint_dir: Option<PathBuf>,
    heartbeat_every_frames: u64,
    tracing_log_path: Option<PathBuf>,
) -> Result<Pipeline, String> {
    let channel_es_n0 = esn0_points.first().copied().unwrap_or(6.0);
    let modcod = Modcod::Normal {
        rate: args.rate,
        modulation: args.modulation,
    };
    let parallelism =
        NonZeroUsize::new(std::thread::available_parallelism().map_or(1, |n| n.get()))
            .unwrap_or(NonZeroUsize::new(1).expect("1 is non-zero"));

    let mut pipeline = Pipeline::dvb_t2()
        .modcod(modcod)
        .decoder(args.decoder)
        .demap(args.demap)
        .channel(Channel::awgn(channel_es_n0 as f32))
        .parallelism(parallelism)
        .seed(args.seed)
        .with_gpu(args.gpu)
        .build()
        .map_err(|e| format!("Cannot build DVB-T2 pipeline: {e:?}"))?;

    let cfg = pipeline.config_mut();
    cfg.esn0_db_points = esn0_points.to_vec();
    cfg.target_errors = target_errors as u64;
    cfg.max_frames = max_frames as u64;
    cfg.seed = args.seed;
    cfg.gpu_enabled = args.gpu;
    cfg.strict_gpu = args.strict_gpu;
    cfg.checkpoint_dir = checkpoint_dir;
    cfg.heartbeat_every_frames = heartbeat_every_frames;
    cfg.tracing_log_path = tracing_log_path;
    Ok(pipeline)
}

// ---------------------------------------------------------------------------
// Full campaign (production or calibration).
// ---------------------------------------------------------------------------

fn run_campaign(args: &Args) -> Result<(), String> {
    // GPU gating: `--gpu` only does anything on a `--features hip` build. On a
    // default build, fail fast with a clear error (deliverable / criterion:
    // "emits a clear error on default builds") rather than silently running on
    // the CPU and mislabelling the run.
    if args.gpu && !cfg!(feature = "hip") {
        return Err(
            "--gpu requires a build with --features hip (the HIP/ROCm GPU backend). \
             This binary was compiled WITHOUT the hip feature, so there is no device \
             path to dispatch to. Rebuild with `cargo run -p gf2-sim --release \
             --features hip --bin dvb_t2_awgn_campaign -- ... --gpu`, or drop --gpu \
             to run the CPU-parallel path."
                .to_string(),
        );
    }
    if args.strict_gpu && !args.gpu {
        return Err(
            "--strict-gpu only has meaning together with --gpu (it promotes a GPU \
             out-of-memory fault to fatal instead of falling back to the CPU stage). \
             Pass --gpu as well, or drop --strict-gpu."
                .to_string(),
        );
    }

    std::fs::create_dir_all(&args.output_dir)
        .map_err(|e| format!("Cannot create output dir: {e}"))?;

    let is_calib = args.calibrate;

    // Determine the Es/N0 sweep.
    let esn0_points: Vec<f64> = if is_calib {
        let bracket = args
            .calibrate_bracket
            .unwrap_or_else(|| default_calibration_bracket(args.rate, args.modulation));
        bracket.to_vec()
    } else {
        let (start, stop, step) = args.esn0_range.unwrap();
        build_snr_range(start, stop, step)
    };

    let target_errors = if is_calib {
        // Calibration: stop only at max_frames; 0 disables the error-count
        // early exit in the checkpointed executor (and calibration uses the
        // plain run path anyway, which has no early exit).
        0
    } else {
        args.target_errors
    };
    let max_frames_per_snr = if is_calib {
        args.calibrate_frames
    } else {
        args.max_frames
    };

    // Output paths.
    let csv_path = if is_calib {
        let calib_dir = args.output_dir.join("calibration");
        std::fs::create_dir_all(&calib_dir)
            .map_err(|e| format!("Cannot create calibration dir: {e}"))?;
        calib_dir.join(calib_csv_name(args.rate, args.modulation))
    } else {
        args.output_dir
            .join(curve_csv_name(args.rate, args.modulation))
    };

    // Calibration writes no checkpoints (short, fixed-frame sweeps); production
    // runs checkpoint per-SNR + heartbeat under <output-dir>/checkpoints.
    let checkpoint_dir = if is_calib {
        None
    } else {
        Some(args.output_dir.join("checkpoints"))
    };

    // If --resume is NOT set but a checkpoint dir exists from a prior run, clear
    // it so the run starts fresh (the checkpointed sweep would otherwise honour
    // stale checkpoints only with --resume, but a config-hash mismatch would
    // surface as a load error; clearing keeps the non-resume semantics clean).
    if !args.resume && !is_calib {
        if let Some(ref ckpt_dir) = checkpoint_dir {
            if ckpt_dir.exists() {
                std::fs::remove_dir_all(ckpt_dir)
                    .map_err(|e| format!("Cannot clear checkpoint dir: {e}"))?;
            }
        }
    }

    // JSON-lines tracing log: unconditional (matches legacy binary parity),
    // written by install_campaign_subscriber below.  Both production and
    // calibration runs produce the file; the cross-epic monitor watches it.
    let tracing_path = args.output_dir.join("tracing.jsonl");

    eprintln!(
        "Campaign (gf2-sim pipeline): {} {} | decoder={:?} demap={:?} | gpu={} strict_gpu={}",
        rate_display(args.rate),
        mod_str(args.modulation),
        args.decoder.algorithm(),
        args.demap,
        args.gpu,
        args.strict_gpu,
    );
    eprintln!(
        "SNR points (Es/N0): {:?}",
        esn0_points
            .iter()
            .map(|v| format!("{:.2}", v))
            .collect::<Vec<_>>()
    );

    // Build the pipeline via the typestate preset and configure the sweep.
    let pipeline = build_configured_pipeline(
        args,
        &esn0_points,
        target_errors,
        max_frames_per_snr,
        checkpoint_dir.clone(),
        if is_calib { 0 } else { args.heartbeat_frames },
        Some(tracing_path.clone()),
    )?;

    // Install the JSON-lines tracing subscriber as the PROCESS-GLOBAL default.
    // Must be called AFTER the pipeline is built (so `tracing_log_path` is set
    // on the config) and BEFORE the run starts. Global (not thread-local)
    // because the sweep's frame loops run on rayon-pool workers and helper
    // threads — a thread-local default would silently drop every event they
    // emit (the campaign_heartbeat events below).
    install_campaign_subscriber(pipeline.config())
        .map_err(|e| format!("Cannot install tracing subscriber: {e}"))?;

    // Emit a campaign_start event so tracing.jsonl has at least one record.
    // This matches the legacy binary's event name/shape used by external monitors
    // (cross-epic e4849f07).
    tracing::info!(
        name: "campaign_start",
        event_type = "campaign_start",
        rate = %rate_display(args.rate),
        modulation = %mod_str(args.modulation),
        seed = args.seed,
        gpu_enabled = args.gpu,
    );

    eprintln!(
        "Running via {} (parallelism={}, checkpoint={})",
        if checkpoint_dir.is_some() {
            "Scheduler::run_sweep_checkpointed"
        } else {
            "Pipeline::run"
        },
        pipeline.config().parallelism.get(),
        checkpoint_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "disabled".to_string()),
    );

    let campaign_start = Instant::now();

    // Route: a checkpointed sweep when a checkpoint dir is configured (the
    // production path — honours `target_errors` early-exit, heartbeat +
    // SNR-boundary + SIGINT checkpointing, and `--resume`); the plain run
    // otherwise (calibration — fixed frame budget, no checkpoints). The plain
    // `Pipeline::run` path has no `target_errors` early-exit, which matches
    // calibration's "run exactly N frames" semantics.
    //
    // The production arm calls `Scheduler::run_sweep_checkpointed` directly
    // (rather than the `Pipeline::run_checkpointed` thin wrapper, which passes
    // a no-op observer) so the per-frame `frame_observer` can emit live
    // `campaign_heartbeat` tracing events — the monitoring channel the
    // cross-epic e4849f07 multi-day sweep watches.
    let results: SimulationResults = if checkpoint_dir.is_some() {
        let scheduler = Scheduler::from_pipeline(&pipeline);
        let heartbeat_every = pipeline.config().heartbeat_every_frames;

        // Per-SNR-point counters of frames observed by this invocation.
        // `campaign_heartbeat` is emitted every `heartbeat_every` observed
        // frames. NOTE (frame_observer caveat, executor/drain.rs): on the
        // hybrid path, frames prepped into a batch that is discarded at an
        // interrupt are observed-but-unrecorded and re-observed on resume, and
        // after a resume the count restarts at 0 (it counts THIS invocation's
        // observations, not global progress) — heartbeat events are
        // approximate liveness/progress, not exact frame accounting. The
        // exact deterministic record is the checkpoint files + final CSV.
        let frames_seen: Vec<AtomicU64> = esn0_points.iter().map(|_| AtomicU64::new(0)).collect();
        let esn0_for_observer = esn0_points.clone();
        let observer = move |snr_idx: usize, _global_frame: usize| {
            let observed = frames_seen[snr_idx].fetch_add(1, Ordering::Relaxed) + 1;
            if heartbeat_every > 0 && observed.is_multiple_of(heartbeat_every) {
                tracing::info!(
                    name: "campaign_heartbeat",
                    event_type = "campaign_heartbeat",
                    snr_idx,
                    es_n0_db = esn0_for_observer[snr_idx],
                    frames_observed = observed,
                );
            }
        };

        let sweep = scheduler
            .run_sweep_checkpointed(&pipeline, args.resume, &observer)
            .map_err(|e| format!("Checkpointed sweep failed: {e}"))?;
        if sweep.interrupted {
            eprintln!(
                "Sweep interrupted (SIGINT/SIGTERM); a resumable checkpoint was flushed. \
                 Re-run with --resume to continue."
            );
        }
        sweep.results
    } else {
        pipeline
            .run_with_decoder()
            .map_err(|e| format!("Pipeline run failed: {e}"))?
    };

    let total_wall = campaign_start.elapsed().as_secs_f64();
    let n_points = results.per_point.len();
    let wall_per_point = if n_points > 0 {
        total_wall / n_points as f64
    } else {
        0.0
    };

    // Post-process SimulationResults into the campaign CSV format. The pipeline
    // already works in Es/N0, so per-point `es_n0_db` is the sweep value.
    let csv_rows: Vec<(f64, f64, f64, u64, u64, f64, f64)> = esn0_points
        .iter()
        .zip(results.per_point.iter())
        .map(|(&es_n0_db, p)| point_to_csv_row(es_n0_db, p, wall_per_point))
        .collect();

    // Emit one snr_point_completed event per completed point. NOTE: these are
    // emitted POST-SWEEP (after run_sweep_checkpointed / run_with_decoder
    // returns), not live at each SNR boundary — the executor's frame_observer
    // carries no per-point result data, so a live boundary event could not
    // include fer/errors/mean_iters. Live progress during the sweep is the
    // campaign_heartbeat channel above.
    for &(es_n0_db, fer, ber, frames, errors, mean_iters, wall_seconds) in &csv_rows {
        tracing::info!(
            name: "snr_point_completed",
            event_type = "snr_point_completed",
            es_n0_db,
            fer,
            ber,
            frames,
            errors,
            mean_iters,
            wall_seconds,
        );
    }

    write_campaign_csv(&csv_path, &csv_rows)
        .map_err(|e| format!("Cannot write campaign CSV: {e}"))?;

    // Write README (production runs only).
    if !is_calib {
        let readme_path = args.output_dir.join("README.md");
        write_readme(&readme_path, args, &esn0_points, total_wall);
    }

    eprintln!("Campaign complete. Output: {}", args.output_dir.display());
    eprintln!("  CSV: {}", csv_path.display());
    eprintln!("  Log: {}", tracing_path.display());
    if let Some(ref ckpt_dir) = checkpoint_dir {
        eprintln!("  Checkpoints: {}", ckpt_dir.display());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point.
// ---------------------------------------------------------------------------

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {e}");
            print_usage();
            std::process::exit(1);
        }
    };

    if let Err(e) = run_campaign(&args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_decoder_minsum() {
        let cfg = parse_decoder("minsum").expect("minsum parse");
        assert_eq!(cfg.algorithm(), DecoderAlgorithm::MinSum);
    }

    #[test]
    fn parse_decoder_sumproduct_and_spa_alias() {
        let cfg_spa = parse_decoder("spa").expect("spa parse");
        let cfg_sp = parse_decoder("sumproduct").expect("sumproduct parse");
        assert_eq!(cfg_spa.algorithm(), DecoderAlgorithm::SumProduct);
        assert_eq!(cfg_sp.algorithm(), DecoderAlgorithm::SumProduct);
    }

    #[test]
    fn parse_decoder_nms_with_alpha() {
        let cfg = parse_decoder("nms:0.75").expect("nms parse");
        assert_eq!(cfg.algorithm(), DecoderAlgorithm::NormalizedMinSum(0.75));
    }

    #[test]
    fn parse_decoder_oms_with_beta() {
        let cfg = parse_decoder("oms:0.5").expect("oms parse");
        assert_eq!(cfg.algorithm(), DecoderAlgorithm::OffsetMinSum(0.5));
    }

    #[test]
    fn parse_decoder_is_case_insensitive() {
        let cfg = parse_decoder("MinSum").expect("case-insensitive parse");
        assert_eq!(cfg.algorithm(), DecoderAlgorithm::MinSum);
    }

    #[test]
    fn parse_decoder_rejects_unknown_name() {
        assert!(parse_decoder("rubbish").is_err());
    }

    #[test]
    fn parse_decoder_rejects_unparseable_alpha() {
        assert!(parse_decoder("nms:notanumber").is_err());
    }

    #[test]
    fn parse_decoder_rejects_invalid_nms_alpha_range() {
        // DecoderConfig::new would panic on these; the parser must reject them.
        assert!(parse_decoder("nms:0.0").is_err()); // alpha = 0 is the boundary excluded by (0, 1]
        assert!(parse_decoder("nms:1.5").is_err()); // alpha > 1
        assert!(parse_decoder("nms:-0.25").is_err()); // negative
        assert!(parse_decoder("nms:NaN").is_err()); // non-finite
        assert!(parse_decoder("nms:inf").is_err()); // non-finite
    }

    #[test]
    fn parse_decoder_rejects_invalid_oms_beta_range() {
        // DecoderConfig::new would panic on these; the parser must reject them.
        assert!(parse_decoder("oms:-0.1").is_err()); // negative
        assert!(parse_decoder("oms:NaN").is_err()); // non-finite
        assert!(parse_decoder("oms:inf").is_err()); // non-finite
    }

    #[test]
    fn parse_decoder_accepts_nms_boundary_one() {
        // alpha = 1.0 is the inclusive upper bound — must be accepted.
        let cfg = parse_decoder("nms:1.0").expect("nms:1.0 parse");
        assert_eq!(cfg.algorithm(), DecoderAlgorithm::NormalizedMinSum(1.0));
    }

    #[test]
    fn parse_decoder_accepts_oms_zero_beta() {
        // beta = 0.0 is the inclusive lower bound — must be accepted.
        let cfg = parse_decoder("oms:0.0").expect("oms:0.0 parse");
        assert_eq!(cfg.algorithm(), DecoderAlgorithm::OffsetMinSum(0.0));
    }

    #[test]
    fn parse_demap_known_methods() {
        assert_eq!(parse_demap("maxlog").unwrap(), DemapMethod::MaxLog);
        assert_eq!(parse_demap("MaxLog").unwrap(), DemapMethod::MaxLog);
        assert_eq!(
            parse_demap("exactlogmap").unwrap(),
            DemapMethod::ExactLogMap
        );
        assert_eq!(parse_demap("exact").unwrap(), DemapMethod::ExactLogMap);
    }

    #[test]
    fn parse_demap_rejects_unknown() {
        assert!(parse_demap("softoutput").is_err());
    }

    #[test]
    fn build_snr_range_inclusive_endpoints() {
        let r = build_snr_range(4.0, 5.0, 0.5);
        assert_eq!(r, vec![4.0, 4.5, 5.0]);
    }

    /// `point_to_csv_row` derives `ber = total_bit_errors / total_bits` and
    /// forwards the four deterministic columns verbatim.
    #[test]
    fn point_to_csv_row_projects_columns() {
        use gf2_sim::parallel::WorkerCounters;
        let mut c = WorkerCounters::default();
        c.record_frame(true, 10, 100, 5);
        c.record_frame(false, 2, 100, 0);
        let p = SnrPointResult::from_counters(6.25, c);
        let (es, fer, ber, frames, errors, mean_iters, wall) = point_to_csv_row(6.25, &p, 1.5);
        assert_eq!(es, 6.25);
        assert_eq!(frames, 2);
        assert_eq!(errors, 1);
        assert!((fer - 0.5).abs() < 1e-12);
        assert!((ber - 5.0 / 200.0).abs() < 1e-12);
        assert!((mean_iters - 6.0).abs() < 1e-12);
        assert_eq!(wall, 1.5);
    }

    /// A minimal valid `Args` (rate 1/2, 16-QAM, one Es/N0 point) the config
    /// tests vary one field of at a time.
    fn base_args() -> Args {
        Args {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
            esn0_range: Some((6.0, 6.0, 0.5)),
            target_errors: 100,
            max_frames: 8,
            seed: 42,
            output_dir: PathBuf::from("/tmp/unused"),
            resume: false,
            gpu: false,
            strict_gpu: false,
            calibrate: false,
            calibrate_frames: 4,
            calibrate_bracket: None,
            decoder: DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
            demap: DemapMethod::ExactLogMap,
            heartbeat_frames: 1000,
        }
    }

    /// CLI→config: the `--strict-gpu` flag (here `args.strict_gpu`) wires through
    /// `build_configured_pipeline` onto `PipelineConfig::strict_gpu`. This is the
    /// "verified by a CLI→config parse test" success criterion.
    #[test]
    fn strict_gpu_flag_wires_to_config() {
        let mut args = base_args();

        // Default (flag absent): strict_gpu false on the config.
        let pipeline = build_configured_pipeline(&args, &[6.0], 100, 8, None, 0, None)
            .expect("pipeline builds");
        assert!(
            !pipeline.config().strict_gpu,
            "strict_gpu must default to false when --strict-gpu is absent"
        );

        // Flag present: strict_gpu true on the config.
        args.strict_gpu = true;
        let pipeline = build_configured_pipeline(&args, &[6.0], 100, 8, None, 0, None)
            .expect("pipeline builds");
        assert!(
            pipeline.config().strict_gpu,
            "--strict-gpu must set PipelineConfig::strict_gpu = true"
        );
    }

    /// CLI→config: the `--gpu` flag (here `args.gpu`) wires through
    /// `build_configured_pipeline` onto `PipelineConfig::gpu_enabled`.
    /// This is the hard-criterion wire verified by a CLI→config parse test.
    #[test]
    fn gpu_enabled_flag_wires_to_config() {
        let mut args = base_args();

        // Default (--gpu absent): gpu_enabled false on the config.
        let pipeline = build_configured_pipeline(&args, &[6.0], 100, 8, None, 0, None)
            .expect("pipeline builds without gpu");
        assert!(
            !pipeline.config().gpu_enabled,
            "gpu_enabled must default to false when --gpu is absent"
        );

        // --gpu present: gpu_enabled true on the config.  Note: `with_gpu(true)`
        // on a non-hip build degrades gracefully at build() time (no error) and
        // only errors at run time when the GPU executor is actually invoked.
        args.gpu = true;
        let pipeline = build_configured_pipeline(&args, &[6.0], 100, 8, None, 0, None)
            .expect("pipeline builds with gpu flag");
        assert!(
            pipeline.config().gpu_enabled,
            "--gpu must set PipelineConfig::gpu_enabled = true"
        );
    }

    /// CLI→config: the remaining run-control knobs (`seed`, `target_errors`,
    /// `max_frames`, `esn0_db_points`, `checkpoint_dir`, `heartbeat`,
    /// `tracing_log_path`) also wire through the single
    /// `build_configured_pipeline` site.
    #[test]
    fn run_control_knobs_wire_to_config() {
        let args = base_args();
        let ck = PathBuf::from("/tmp/ck_unused");
        let tl = PathBuf::from("/tmp/tl_unused.jsonl");
        let pipeline = build_configured_pipeline(
            &args,
            &[6.0, 6.5],
            100,
            8,
            Some(ck.clone()),
            1000,
            Some(tl.clone()),
        )
        .expect("pipeline builds");
        let cfg = pipeline.config();
        assert_eq!(cfg.seed, 42);
        assert_eq!(cfg.target_errors, 100);
        assert_eq!(cfg.max_frames, 8);
        assert_eq!(cfg.esn0_db_points, vec![6.0, 6.5]);
        assert_eq!(cfg.checkpoint_dir.as_deref(), Some(ck.as_path()));
        assert_eq!(cfg.heartbeat_every_frames, 1000);
        assert_eq!(cfg.tracing_log_path.as_deref(), Some(tl.as_path()));
    }

    #[test]
    fn point_to_csv_row_zero_bits_is_zero_ber() {
        use gf2_sim::parallel::WorkerCounters;
        let c = WorkerCounters::default();
        let p = SnrPointResult::from_counters(6.0, c);
        let (_, _, ber, _, _, _, _) = point_to_csv_row(6.0, &p, 0.0);
        assert_eq!(ber, 0.0);
    }
}
