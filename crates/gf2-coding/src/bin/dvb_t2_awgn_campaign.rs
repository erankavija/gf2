//! DVB-T2 BICM AWGN campaign runner.
//!
//! Runs the full DVB-T2 BICM AWGN simulation for one `(rate, modulation)`
//! configuration, producing a CSV curve, a JSON-lines tracing log, a README,
//! and per-SNR checkpoint files.
//!
//! Six invocations (3 rates × 2 modulations) reproduce every curve required
//! by epic 2928ccce.
//!
//! # BICM chain (per frame)
//!
//! ```text
//! BBFRAME → BCH+LDPC encode → bit interleave → QAM map → AWGN
//!                                                            ↓
//! BBFRAME ← BCH+LDPC decode ← bit deinterleave ← QAM demap
//! ```
//!
//! The simulation core is driven through [`SimulationRunner::run_with_decoder`]
//! from `gf2_coding::simulation`. Checkpointing, SIGINT flush, JSON-lines
//! tracing, and ChaCha20 RNG seek all come from the `sim-observability` layer
//! in that module. The binary itself is a thin CLI front-end: parse args →
//! build `SimulationConfig` + the BICM encoder/channel/decoder → call the
//! runner → post-process `SimulationResults` into the campaign CSV.
//!
//! # Usage
//!
//! ## Smoke run (3 SNR points, small frame budget)
//!
//! ```bash
//! cargo run --release --bin dvb_t2_awgn_campaign -- \
//!     --rate 1/2 --modulation 16qam \
//!     --esn0-range 4.0:5.0:0.5 \
//!     --max-frames 100 --target-errors 5 \
//!     --output-dir /tmp/dvb_smoke --seed 42
//! ```
//!
//! ## Production run
//!
//! ```bash
//! cargo run --release --bin dvb_t2_awgn_campaign -- \
//!     --rate 1/2 --modulation 16qam \
//!     --esn0-range 4.0:7.0:0.5 \
//!     --target-errors 100 --max-frames 10000000 \
//!     --output-dir /tmp/dvb_r12_16qam --seed 42
//! ```
//!
//! ## Resuming after interruption
//!
//! ```bash
//! cargo run --release --bin dvb_t2_awgn_campaign -- \
//!     --rate 1/2 --modulation 16qam \
//!     --esn0-range 4.0:7.0:0.5 \
//!     --target-errors 100 --max-frames 10000000 \
//!     --output-dir /tmp/dvb_r12_16qam --seed 42 --resume
//! ```
//!
//! ## Calibration sweep
//!
//! ```bash
//! cargo run --release --bin dvb_t2_awgn_campaign -- \
//!     --calibrate --rate 1/2 --modulation 16qam \
//!     --output-dir /tmp/dvb_calib --seed 42 --calibrate-frames 1000
//! ```
//!
//! # Output layout
//!
//! Under `<output-dir>/`:
//! - `curve_<rate>_<mod>.csv` — per-SNR results (columns: `es_n0_db, fer, ber,
//!   frames, errors, mean_iters, wall_seconds`).
//! - `tracing.jsonl` — structured tracing log (one record per event), written
//!   by the `sim-observability` layer of `SimulationRunner`.
//! - `README.md` — invocation, seed, host info, total wall-clock.
//! - `checkpoints/` — per-SNR JSON files with BLAKE3-verified config hash,
//!   written by `SimulationRunner`'s checkpoint subsystem.
//! - `calibration_<rate>_<mod>.csv` (only when `--calibrate`).
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
//!
//! # GPU
//!
//! `--gpu` is reserved for future HIP/ROCm integration (epic 806eb14e).
//! Passing `--gpu` currently returns a clear "GPU path not yet integrated"
//! error.

#![deny(unsafe_code)]

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::llr::Llr;
use gf2_coding::modem::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec};
use gf2_coding::simulation::{ChannelModel, SimulationConfig, SimulationRunner};
use gf2_coding::traits::{BlockEncoder, DecoderResult};
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::Rng;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

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
    calibrate: bool,
    calibrate_frames: usize,
    /// Optional explicit 3-point bracket [low, center, high] for calibration.
    calibrate_bracket: Option<[f64; 3]>,
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
           --gpu                            GPU dispatch (reserved; not yet integrated)\n\
           --calibrate                      Run calibration sweep instead of full campaign\n\
           --calibrate-frames <N>           Frames per calibration point [default: 1000]\n\
           --calibrate-bracket <a:b:c>      Custom 3-point Es/N0 bracket for calibration\n\
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
    let mut calibrate = false;
    let mut calibrate_frames: usize = 1000;
    let mut calibrate_bracket: Option<[f64; 3]> = None;

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
        calibrate,
        calibrate_frames,
        calibrate_bracket,
    })
}

// ---------------------------------------------------------------------------
// Naming helpers.
// ---------------------------------------------------------------------------

fn rate_str(r: CodeRate) -> &'static str {
    match r {
        CodeRate::Rate1_2 => "1_2",
        CodeRate::Rate2_3 => "2_3",
        CodeRate::Rate3_4 => "3_4",
        _ => "unknown",
    }
}

fn rate_f64(r: CodeRate) -> f64 {
    match r {
        CodeRate::Rate1_2 => 0.5,
        CodeRate::Rate2_3 => 2.0 / 3.0,
        CodeRate::Rate3_4 => 0.75,
        _ => 1.0,
    }
}

fn rate_display(r: CodeRate) -> &'static str {
    match r {
        CodeRate::Rate1_2 => "1/2",
        CodeRate::Rate2_3 => "2/3",
        CodeRate::Rate3_4 => "3/4",
        _ => "?",
    }
}

fn mod_str(m: DvbT2Modulation) -> &'static str {
    match m {
        DvbT2Modulation::Qam16 => "16qam",
        DvbT2Modulation::Qam64 => "64qam",
        _ => "unknown",
    }
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
// SNR conversion: Es/N0 ↔ Eb/N0.
//
// Es/N0 = Eb/N0 + 10*log10(m * r)
// => Eb/N0 = Es/N0 - 10*log10(m * r)
// ---------------------------------------------------------------------------

fn esn0_to_ebn0(es_n0_db: f64, bits_per_symbol: usize, code_rate: f64) -> f64 {
    es_n0_db - 10.0 * (bits_per_symbol as f64 * code_rate).log10()
}

fn ebn0_to_esn0(eb_n0_db: f64, bits_per_symbol: usize, code_rate: f64) -> f64 {
    eb_n0_db + 10.0 * (bits_per_symbol as f64 * code_rate).log10()
}

// ---------------------------------------------------------------------------
// BICM encoder wrapper: implements BlockEncoder for DvbT2Concat.
//
// k = k_bch (BBFRAME bits), n = n_ldpc (FECFRAME bits).
// encode: BBFRAME → BCH+LDPC → FECFRAME.
// ---------------------------------------------------------------------------

struct BicmFecEncoder {
    concat: DvbT2Concat,
}

impl BicmFecEncoder {
    fn new(concat: DvbT2Concat) -> Self {
        Self { concat }
    }
}

impl BlockEncoder for BicmFecEncoder {
    fn k(&self) -> usize {
        self.concat.k_bch()
    }

    fn n(&self) -> usize {
        self.concat.n_ldpc()
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        self.concat.encode(message)
    }
}

// ---------------------------------------------------------------------------
// BICM channel: implements ChannelModel for the DVB-T2 BICM chain.
//
// transmit_and_demodulate receives n_ldpc FECFRAME bits (encoder output),
// applies bit interleaving, QAM mapping, AWGN noise, QAM soft demapping,
// and bit deinterleaving, returning n_ldpc LLRs in FECFRAME order.
//
// The caller (SimulationRunner) passes eb_n0_db and rate. We convert to
// Es/N0 internally for the noise computation.
// ---------------------------------------------------------------------------

struct BicmAwgnChannel {
    interleaver: DvbT2BitInterleaver,
    bits_per_symbol: usize,
    spec: ModemSpec<f32>,
}

impl BicmAwgnChannel {
    fn new(interleaver: DvbT2BitInterleaver, bits_per_symbol: usize) -> Self {
        let spec = ModemSpec::<f32>::gray_square_qam(if bits_per_symbol == 4 { 16 } else { 64 });
        Self {
            interleaver,
            bits_per_symbol,
            spec,
        }
    }
}

impl ChannelModel for BicmAwgnChannel {
    fn batch_alignment(&self) -> usize {
        // FECFRAME length is always divisible by bits_per_symbol for DVB-T2.
        // Return 1 since the runner passes the full n_ldpc-bit codeword.
        1
    }

    fn demap_method(&self) -> DemapMethod {
        DemapMethod::MaxLog
    }

    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr> {
        // bits = FECFRAME (n_ldpc bits) from the encoder.
        let n_ldpc = bits.len();
        let num_symbols = n_ldpc / self.bits_per_symbol;

        // Convert Eb/N0 to Es/N0, then to per-component noise variance.
        // Es/N0 = Eb/N0 + 10*log10(m * r)
        // sigma^2 = 1 / (2 * 10^(Es_N0/10))
        let es_n0_db = ebn0_to_esn0(eb_n0_db, self.bits_per_symbol, rate);
        let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        let noise_var_f32 = (2.0 * sigma_sq) as f32; // N0 = 2 * sigma^2

        let mapper = self.spec.preferred_mapper();
        let demapper = self.spec.preferred_soft_demapper();

        // 1. Bit interleave: FECFRAME order → interleaved order.
        let interleaved = self.interleaver.interleave(bits);

        // 2. QAM map: interleaved bits → I/Q symbols.
        let interleaved_bits: Vec<bool> =
            (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
        let mut tx_i = vec![0.0_f32; num_symbols];
        let mut tx_q = vec![0.0_f32; num_symbols];
        mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

        // 3. AWGN: independent Gaussian noise on I and Q axes (Box-Muller).
        let sigma_f32 = (sigma_sq as f32).sqrt();
        for s in tx_i.iter_mut() {
            let u1: f64 = rng.gen::<f64>().max(1e-15);
            let u2: f64 = rng.gen::<f64>();
            let n = ((-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()) as f32;
            *s += sigma_f32 * n;
        }
        for s in tx_q.iter_mut() {
            let u1: f64 = rng.gen::<f64>().max(1e-15);
            let u2: f64 = rng.gen::<f64>();
            let n = ((-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()) as f32;
            *s += sigma_f32 * n;
        }

        // 4. QAM soft demap → interleaved LLRs.
        let noise_var_buf = vec![noise_var_f32; num_symbols];
        let mut interleaved_llrs = vec![Llr::new(0.0); n_ldpc];
        demapper.demap_llrs(
            DemapInput {
                rx_i: &tx_i,
                rx_q: &tx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &noise_var_buf,
                method: DemapMethod::MaxLog,
            },
            &mut interleaved_llrs,
        );

        // 5. Bit deinterleave LLRs → FECFRAME order.
        self.interleaver.deinterleave_llrs(&interleaved_llrs)
    }
}

// ---------------------------------------------------------------------------
// Campaign CSV writer.
// ---------------------------------------------------------------------------

const CAMPAIGN_CSV_HEADER: &str = "es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds";

fn write_campaign_csv(
    path: &Path,
    points: &[(f64, f64, f64, usize, usize, f64, f64)],
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
// Full campaign (production or calibration).
// ---------------------------------------------------------------------------

fn run_campaign(args: &Args) -> Result<(), String> {
    if args.gpu {
        return Err(
            "GPU path not yet integrated: --gpu requires epic 806eb14e (HIP LDPC BP prototype) \
             to land first. Compile without --gpu or add --features hip and wait for the \
             integration epic."
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
        // Calibration: stop only at max_frames; usize::MAX ensures the error
        // count never triggers early exit.
        usize::MAX
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
        args.output_dir
            .join(calib_csv_name(args.rate, args.modulation))
    } else {
        args.output_dir
            .join(curve_csv_name(args.rate, args.modulation))
    };

    let tracing_path = args.output_dir.join("tracing.jsonl");

    let checkpoint_dir = if is_calib {
        None
    } else {
        Some(args.output_dir.join("checkpoints"))
    };

    // Build the BICM components.
    let concat = DvbT2Concat::new(FrameSize::Normal, args.rate).map_err(|e| format!("{e:?}"))?;
    let modcod = DvbT2Modcod::new(FrameSize::Normal, args.rate, args.modulation);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    let bits_per_symbol = match args.modulation {
        DvbT2Modulation::Qam16 => 4,
        DvbT2Modulation::Qam64 => 6,
        _ => return Err("unsupported modulation".to_string()),
    };

    // Validate that n_ldpc is divisible by bits_per_symbol.
    if concat.n_ldpc() % bits_per_symbol != 0 {
        return Err(format!(
            "n_ldpc={} is not divisible by bits_per_symbol={}",
            concat.n_ldpc(),
            bits_per_symbol
        ));
    }

    eprintln!(
        "Campaign: {} {} | n_ldpc={} k_bch={} bits/sym={}",
        rate_display(args.rate),
        mod_str(args.modulation),
        concat.n_ldpc(),
        concat.k_bch(),
        bits_per_symbol,
    );
    eprintln!(
        "SNR points (Es/N0): {:?}",
        esn0_points
            .iter()
            .map(|v| format!("{:.2}", v))
            .collect::<Vec<_>>()
    );

    // Convert Es/N0 to Eb/N0 for SimulationConfig (the runner uses Eb/N0).
    let code_rate = rate_f64(args.rate);
    let ebn0_points: Vec<f64> = esn0_points
        .iter()
        .map(|&es| esn0_to_ebn0(es, bits_per_symbol, code_rate))
        .collect();

    // Build SimulationConfig.
    let mut sim_config = SimulationConfig {
        eb_n0_range_db: ebn0_points.clone(),
        min_errors: target_errors,
        max_frames: max_frames_per_snr,
        max_decoder_iterations: 50, // DVB-T2 default max BP iterations
        rng_seed: Some(args.seed),
        output_path: None, // We post-process into campaign CSV format ourselves.
        checkpoint_dir: checkpoint_dir.clone(),
        tracing_log_path: Some(tracing_path.clone()),
        heartbeat_every_frames: if is_calib { None } else { Some(1000) },
    };

    // For resume: if --resume is set and a checkpoint dir exists, the runner's
    // own checkpoint mechanism handles it via checkpoint_dir above. We don't
    // need separate CSV-based resume since the runner reads checkpoints.
    // However, the runner's legacy CSV-based resume path requires output_path
    // to be set. Since we don't set output_path (to avoid format mismatch),
    // we rely purely on checkpoint-based resume.
    //
    // If --resume is NOT set but a checkpoint dir exists from a prior run,
    // the runner will still honor those checkpoints. Clear them by removing
    // the checkpoint dir if not resuming.
    if !args.resume && !is_calib {
        if let Some(ref ckpt_dir) = checkpoint_dir {
            if ckpt_dir.exists() {
                std::fs::remove_dir_all(ckpt_dir)
                    .map_err(|e| format!("Cannot clear checkpoint dir: {e}"))?;
            }
        }
    }

    // Disable checkpoint dir for calibration (already None above, but ensure).
    if is_calib {
        sim_config.checkpoint_dir = None;
        sim_config.heartbeat_every_frames = None;
    }

    // Wrap DvbT2Concat in a BlockEncoder shim.
    let encoder = BicmFecEncoder::new(concat);

    // Build the BICM AWGN channel model.
    let channel = BicmAwgnChannel::new(interleaver, bits_per_symbol);

    eprintln!(
        "Running via SimulationRunner::run_with_decoder (checkpoint={}, tracing={})",
        checkpoint_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "disabled".to_string()),
        tracing_path.display(),
    );

    let campaign_start = Instant::now();

    // Drive the simulation through the runner. The runner handles:
    // - Checkpoint write/resume (via checkpoint_dir)
    // - SIGINT/SIGTERM flush (via ctrlc handler in simulation.rs)
    // - JSON-lines tracing (via tracing_log_path)
    // - Within-SNR heartbeat checkpoints (via heartbeat_every_frames)
    // - ChaCha20 RNG seek for byte-identical resume
    let sim_results = SimulationRunner::run_with_decoder(
        &encoder,
        |llrs| {
            // The runner passes FECFRAME-order LLRs (after BicmAwgnChannel
            // deinterleaving). Call DvbT2Concat::decode_soft directly.
            let decode_result = encoder.concat.decode_soft(llrs);
            match decode_result {
                Ok(bbframe) => {
                    // LDPC converged; return with iterations = max (sentinel).
                    DecoderResult::success(bbframe)
                }
                Err(gf2_coding::ldpc::dvb_t2::concat::ConcatError::LdpcDecodeFailed {
                    bbframe,
                    iterations,
                }) => {
                    // LDPC did not converge; bbframe is best-effort.
                    DecoderResult::new(bbframe, iterations, false, false)
                }
                Err(_) => {
                    // Other errors: return empty bitvec as total failure.
                    DecoderResult::new(BitVec::with_capacity(encoder.k()), 50, false, false)
                }
            }
        },
        &channel,
        &sim_config,
    );

    let total_wall = campaign_start.elapsed().as_secs_f64();
    let n_points = sim_results.points.len();
    let wall_per_point = if n_points > 0 {
        total_wall / n_points as f64
    } else {
        0.0
    };

    // Post-process SimulationResults into the campaign CSV format.
    // The runner uses eb_n0_db; we map back to es_n0_db for the CSV.
    let csv_rows: Vec<(f64, f64, f64, usize, usize, f64, f64)> = sim_results
        .points
        .iter()
        .zip(esn0_points.iter())
        .map(|(p, &es_n0_db)| {
            let fer = p.bler;
            let ber = p.ber;
            let frames = p.num_frames;
            let errors = p.num_frame_errors;
            let mean_iters = p.avg_iterations.unwrap_or(0.0);
            // wall_seconds: approximate from total wall / n_points (per-point
            // timing is not exposed by SimulationResult).
            let wall_seconds = wall_per_point;
            (es_n0_db, fer, ber, frames, errors, mean_iters, wall_seconds)
        })
        .collect();

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
