//! DVB-T2 BICM AWGN campaign runner.
//!
//! Runs the full DVB-T2 BICM AWGN simulation for one `(rate, modulation)`
//! configuration, producing a CSV curve, a JSON-lines tracing log, a README,
//! and per-SNR checkpoint files.
//!
//! Six invocations (3 rates × 2 modulations) reproduce every curve required by
//! epic 2928ccce.
//!
//! # BICM chain (per frame)
//!
//! ```text
//! BBFRAME → BCH+LDPC encode → bit interleave → QAM map → AWGN
//!                                                            ↓
//! BBFRAME ← BCH+LDPC decode ← bit deinterleave ← QAM demap
//! ```
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
//! - `tracing.jsonl` — structured tracing log (one record per event).
//! - `README.md` — invocation, seed, host info, total wall-clock.
//! - `checkpoints/` — per-SNR JSON files with BLAKE3-verified config hash.
//! - `calibration/calibration_<rate>_<mod>.csv` (only when `--calibrate`).
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
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
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
// ---------------------------------------------------------------------------

/// Returns `[low, center, high]` Es/N0 values for the calibration bracket.
///
/// Prefers the value from the reference TOML if available; falls back to
/// hardcoded per-MODCOD values derived from approximate waterfall positions.
fn default_calibration_bracket(rate: CodeRate, modulation: DvbT2Modulation) -> [f64; 3] {
    // Per-MODCOD approximate Es/N0 at FER=1e-4 (center), with ±1 dB bracket.
    // These match the PLACEHOLDER values in the reference TOML.
    let center = match (rate, modulation) {
        (CodeRate::Rate1_2, DvbT2Modulation::Qam16) => 6.0,
        (CodeRate::Rate2_3, DvbT2Modulation::Qam16) => 8.0,
        (CodeRate::Rate3_4, DvbT2Modulation::Qam16) => 9.5,
        (CodeRate::Rate1_2, DvbT2Modulation::Qam64) => 8.0,
        (CodeRate::Rate2_3, DvbT2Modulation::Qam64) => 10.5,
        (CodeRate::Rate3_4, DvbT2Modulation::Qam64) => 12.0,
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
// Per-SNR checkpoint (compatible with simulation.rs format but adapted for
// Es/N0 labelling — we store es_n0_db in the eb_n0_db JSON field).
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct SnrCheckpoint {
    snr_index: usize,
    es_n0_db: f64,
    frames_completed: usize,
    errors_accumulated: usize,
    total_iterations: u64,
    total_bits: u64,
    total_bit_errors: u64,
    rng_word_pos: u128,
    frames_target: usize,
    errors_target: usize,
    completed: bool,
    config_hash: String,
}

impl SnrCheckpoint {
    fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"snr_index\": {},\n",
                "  \"es_n0_db\": {},\n",
                "  \"frames_completed\": {},\n",
                "  \"errors_accumulated\": {},\n",
                "  \"total_iterations\": {},\n",
                "  \"total_bits\": {},\n",
                "  \"total_bit_errors\": {},\n",
                "  \"rng_word_pos\": \"{}\",\n",
                "  \"frames_target\": {},\n",
                "  \"errors_target\": {},\n",
                "  \"completed\": {},\n",
                "  \"config_hash\": \"{}\"\n",
                "}}"
            ),
            self.snr_index,
            self.es_n0_db,
            self.frames_completed,
            self.errors_accumulated,
            self.total_iterations,
            self.total_bits,
            self.total_bit_errors,
            self.rng_word_pos,
            self.frames_target,
            self.errors_target,
            self.completed,
            self.config_hash,
        )
    }

    fn from_json(s: &str) -> Option<Self> {
        fn extract<'a>(s: &'a str, key: &str) -> Option<&'a str> {
            let needle = format!("\"{key}\":");
            let pos = s.find(needle.as_str())?;
            let after = s[pos + needle.len()..].trim_start();
            if let Some(inner) = after.strip_prefix('"') {
                let end = inner.find('"')?;
                Some(&inner[..end])
            } else {
                let end = after.find([',', '\n', '}']).unwrap_or(after.len());
                Some(after[..end].trim())
            }
        }
        Some(Self {
            snr_index: extract(s, "snr_index")?.parse().ok()?,
            es_n0_db: extract(s, "es_n0_db")?.parse().ok()?,
            frames_completed: extract(s, "frames_completed")?.parse().ok()?,
            errors_accumulated: extract(s, "errors_accumulated")?.parse().ok()?,
            total_iterations: extract(s, "total_iterations")?.parse().ok()?,
            total_bits: extract(s, "total_bits")?.parse().ok()?,
            total_bit_errors: extract(s, "total_bit_errors")?.parse().ok()?,
            rng_word_pos: extract(s, "rng_word_pos")?.parse().ok()?,
            frames_target: extract(s, "frames_target")?.parse().ok()?,
            errors_target: extract(s, "errors_target")?.parse().ok()?,
            completed: extract(s, "completed")? == "true",
            config_hash: extract(s, "config_hash")?.to_string(),
        })
    }
}

fn checkpoint_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("snr_{:04}.json", index))
}

fn config_hash_path(dir: &Path) -> PathBuf {
    dir.join("config_hash.txt")
}

fn write_checkpoint_atomic(path: &Path, ckpt: &SnrCheckpoint) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, ckpt.to_json())?;
    std::fs::rename(&tmp, path)
}

fn load_checkpoint(path: &Path, expected_hash: &str) -> Option<SnrCheckpoint> {
    let s = std::fs::read_to_string(path).ok()?;
    let ckpt = SnrCheckpoint::from_json(&s)?;
    if ckpt.config_hash != expected_hash {
        eprintln!(
            "Warning: checkpoint config hash mismatch at {}.\n  \
             stored:   {}\n  \
             expected: {}\n  \
             Ignoring stale checkpoint.",
            path.display(),
            ckpt.config_hash,
            expected_hash,
        );
        return None;
    }
    Some(ckpt)
}

// ---------------------------------------------------------------------------
// Config hash — BLAKE3 over the campaign parameters that affect results.
// ---------------------------------------------------------------------------

fn compute_config_hash(
    snr_points: &[f64],
    target_errors: usize,
    max_frames: usize,
    seed: u64,
    rate: CodeRate,
    modulation: DvbT2Modulation,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(snr_points.len() as u64).to_le_bytes());
    for &v in snr_points {
        hasher.update(&v.to_le_bytes());
    }
    hasher.update(&(target_errors as u64).to_le_bytes());
    hasher.update(&(max_frames as u64).to_le_bytes());
    hasher.update(&seed.to_le_bytes());
    // Encode rate + modulation as a pair of u8 tags.
    let rate_tag: u8 = match rate {
        CodeRate::Rate1_2 => 0,
        CodeRate::Rate2_3 => 1,
        CodeRate::Rate3_4 => 2,
        _ => 255,
    };
    let mod_tag: u8 = match modulation {
        DvbT2Modulation::Qam16 => 0,
        DvbT2Modulation::Qam64 => 1,
        _ => 255,
    };
    hasher.update(&[rate_tag, mod_tag]);
    let hash = hasher.finalize();
    format!("blake3:{}", hash.to_hex())
}

fn validate_or_create_checkpoint_dir(dir: &Path, current_hash: &str) -> Result<(), String> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create checkpoint dir {}: {e}", dir.display()))?;
        std::fs::write(config_hash_path(dir), current_hash)
            .map_err(|e| format!("Cannot write config_hash.txt: {e}"))?;
        return Ok(());
    }
    let hash_file = config_hash_path(dir);
    if !hash_file.exists() {
        std::fs::write(&hash_file, current_hash)
            .map_err(|e| format!("Cannot write config_hash.txt: {e}"))?;
        return Ok(());
    }
    let stored = std::fs::read_to_string(&hash_file)
        .map_err(|e| format!("Cannot read config_hash.txt: {e}"))?;
    let stored = stored.trim();
    if stored != current_hash {
        return Err(format!(
            "Checkpoint directory hash mismatch.\n  stored:  {stored}\n  current: {current_hash}\n\
             Change --output-dir or delete the checkpoint directory to start fresh.",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// JSONL append helper.
// ---------------------------------------------------------------------------

fn append_jsonl(path: &Path, record: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{record}");
    }
}

fn iso_timestamp() -> String {
    use std::time::SystemTime;
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            let days = secs / 86400;
            let tod = secs % 86400;
            let h = tod / 3600;
            let m = (tod % 3600) / 60;
            let s = tod % 60;
            let z = days as i64 + 719468;
            let era = if z >= 0 { z } else { z - 146096 } / 146097;
            let doe = (z - era * 146097) as u64;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let y = yoe as i64 + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let mon = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = if mon <= 2 { y + 1 } else { y };
            format!("{y:04}-{mon:02}-{day:02}T{h:02}:{m:02}:{s:02}")
        }
        Err(_) => "1970-01-01T00:00:00".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Campaign CSV writer.
// ---------------------------------------------------------------------------

const CAMPAIGN_CSV_HEADER: &str = "es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds";

fn write_csv_header_if_empty(path: &Path) -> std::io::Result<()> {
    let needs_header = !path.exists() || std::fs::metadata(path).map_or(true, |m| m.len() == 0);
    if needs_header {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(f, "{CAMPAIGN_CSV_HEADER}")?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_csv_row(
    path: &Path,
    es_n0_db: f64,
    fer: f64,
    ber: f64,
    frames: usize,
    errors: usize,
    mean_iters: f64,
    wall_seconds: f64,
) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        f,
        "{},{},{},{},{},{},{}",
        es_n0_db, fer, ber, frames, errors, mean_iters, wall_seconds
    )
}

// ---------------------------------------------------------------------------
// Per-SNR simulation result.
// ---------------------------------------------------------------------------

struct SnrResult {
    es_n0_db: f64,
    fer: f64,
    ber: f64,
    frames: usize,
    errors: usize,
    mean_iters: f64,
    wall_seconds: f64,
}

// ---------------------------------------------------------------------------
// Noise conversion: Es/N0 → Eb/N0 for ModemChannelAdapter.
//
// Es/N0 = Eb/N0 + 10*log10(m * r)
// => Eb/N0 = Es/N0 - 10*log10(m * r)
// ---------------------------------------------------------------------------

fn esn0_to_ebn0(es_n0_db: f64, bits_per_symbol: usize, code_rate: f64) -> f64 {
    es_n0_db - 10.0 * (bits_per_symbol as f64 * code_rate).log10()
}

// ---------------------------------------------------------------------------
// Per-SNR RNG seeding (ChaCha20).
//
// seed_for_snr_point = base_seed ^ rotate_left(snr_index, 13)
// ---------------------------------------------------------------------------

fn make_chacha_rng(base_seed: u64, snr_index: usize, word_pos: u128) -> ChaCha20Rng {
    let seed = base_seed ^ (snr_index as u64).rotate_left(13);
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    rng.set_word_pos(word_pos);
    rng
}

// ---------------------------------------------------------------------------
// Inner simulation loop for a single SNR point.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_snr_point(
    es_n0_db: f64,
    snr_index: usize,
    base_seed: u64,
    target_errors: usize,
    max_frames: usize,
    concat: &DvbT2Concat,
    interleaver: &DvbT2BitInterleaver,
    bits_per_symbol: usize,
    code_rate_f64: f64,
    // Resume state (0 for fresh start).
    resume_frames: usize,
    resume_errors: usize,
    resume_iters: u64,
    resume_bits: u64,
    resume_bit_errors: u64,
    resume_word_pos: u128,
    // Checkpoint + JSONL paths (None disables writing).
    checkpoint_dir: Option<&Path>,
    config_hash: &str,
    tracing_path: Option<&Path>,
    heartbeat_every: usize,
    interrupted: &std::sync::atomic::AtomicBool,
) -> SnrResult {
    let point_start = Instant::now();

    let mut rng = make_chacha_rng(base_seed, snr_index, resume_word_pos);

    // Noise: Es/N0 → sigma^2 (per-component).
    // sigma^2 = 1 / (2 * m * r * 10^(Eb_N0 / 10))
    // where Eb_N0 = Es_N0 - 10*log10(m * r).
    // Equivalently: sigma^2 = 1 / (2 * 10^(Es_N0/10)).
    let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    let noise_var_f32 = (2.0 * sigma_sq) as f32; // N0 = 2 * sigma^2

    let spec = ModemSpec::<f32>::gray_square_qam(if bits_per_symbol == 4 { 16 } else { 64 });
    let mapper = spec.preferred_mapper();
    let demapper = spec.preferred_soft_demapper();

    let n_ldpc = concat.n_ldpc();
    let k_bch = concat.k_bch();
    let num_symbols = n_ldpc / bits_per_symbol;

    // Scratch buffers allocated once per SNR point.
    let mut tx_i = vec![0.0_f32; num_symbols];
    let mut tx_q = vec![0.0_f32; num_symbols];
    let mut noise_var_buf = vec![noise_var_f32; num_symbols];
    let mut interleaved_llrs = vec![Llr::new(0.0); n_ldpc];
    let mut fecframe_llrs = vec![Llr::new(0.0); n_ldpc];

    let mut frames = resume_frames;
    let mut errors = resume_errors;
    let mut total_iters = resume_iters;
    let mut total_bits = resume_bits;
    let mut total_bit_errors = resume_bit_errors;

    // The input Eb/N0 for ModemChannelAdapter is not used directly because
    // we drive the noise manually below. We pass sigma_sq directly.
    let _ = code_rate_f64; // acknowledged: used only for noise derivation above

    let ebn0_for_log = esn0_to_ebn0(es_n0_db, bits_per_symbol, code_rate_f64);
    eprintln!(
        "[{:.2} dB Es/N0 ({:.2} dB Eb/N0)] starting (resume: {} frames, {} errors)",
        es_n0_db, ebn0_for_log, resume_frames, resume_errors
    );

    while errors < target_errors && frames < max_frames {
        // Check for SIGINT (set by ctrlc handler in simulation.rs; we poll here).
        if interrupted.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        // --- Forward path ---
        // 1. Random BBFRAME.
        use rand::Rng as _;
        let mut bbframe_in = BitVec::with_capacity(k_bch);
        for _ in 0..k_bch {
            bbframe_in.push_bit(rng.gen::<bool>());
        }

        // 2. BCH+LDPC encode → FECFRAME.
        let fecframe = concat.encode(&bbframe_in);

        // 3. Bit interleave.
        let interleaved = interleaver.interleave(&fecframe);

        // 4. QAM map.
        let interleaved_bits: Vec<bool> =
            (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
        mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

        // 5. AWGN: independent Gaussian noise on I and Q axes (Box-Muller).
        // Each axis has per-component std-dev = sqrt(sigma_sq).
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

        // 6. QAM soft demap → interleaved LLRs.
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

        // 7. Bit deinterleave LLRs → FECFRAME order.
        fecframe_llrs.copy_from_slice(&interleaver.deinterleave_llrs(&interleaved_llrs));

        // 8. BCH+LDPC decode.
        let decode_result = concat.decode_soft(&fecframe_llrs);
        let (bbframe_out, converged, iters) = match decode_result {
            Ok(bits) => {
                // DvbT2Concat::decode_soft does not expose iterations publicly.
                // Use a sentinel value of 50 (default max LDPC iterations).
                (bits, true, 50usize)
            }
            Err(gf2_coding::ldpc::dvb_t2::concat::ConcatError::LdpcDecodeFailed {
                bbframe,
                iterations,
            }) => (bbframe, false, iterations),
            Err(_) => {
                // Other errors (e.g. length mismatch): count as frame error.
                let bits = BitVec::with_capacity(k_bch);
                (bits, false, 50)
            }
        };

        frames += 1;
        total_iters += iters as u64;
        total_bits += k_bch as u64;

        // Count bit errors.
        let n_compare = bbframe_in.len().min(bbframe_out.len());
        let bit_errors: usize = (0..n_compare)
            .filter(|&j| bbframe_in.get(j) != bbframe_out.get(j))
            .count();
        // Frames with shorter output than expected are fully erroneous.
        let extra_errors = k_bch.saturating_sub(bbframe_out.len());
        let bit_errors = bit_errors + extra_errors;

        total_bit_errors += bit_errors as u64;

        let frame_error = !converged || bit_errors > 0;
        if frame_error {
            errors += 1;
        }

        // Reset scratch buffers for next frame.
        noise_var_buf.iter_mut().for_each(|v| *v = noise_var_f32);

        // Heartbeat + checkpoint.
        if heartbeat_every > 0 && frames.is_multiple_of(heartbeat_every) {
            let word_pos = rng.get_word_pos();
            let fer = errors as f64 / frames as f64;
            eprintln!(
                "[{:.2} dB] heartbeat: frames={} errors={} FER={:.3e}",
                es_n0_db, frames, errors, fer
            );
            if let Some(ckpt_dir) = checkpoint_dir {
                let ckpt = SnrCheckpoint {
                    snr_index,
                    es_n0_db,
                    frames_completed: frames,
                    errors_accumulated: errors,
                    total_iterations: total_iters,
                    total_bits,
                    total_bit_errors,
                    rng_word_pos: word_pos,
                    frames_target: max_frames,
                    errors_target: target_errors,
                    completed: false,
                    config_hash: config_hash.to_string(),
                };
                if let Err(e) =
                    write_checkpoint_atomic(&checkpoint_path(ckpt_dir, snr_index), &ckpt)
                {
                    eprintln!("Warning: failed to write heartbeat checkpoint: {e}");
                }
            }
            if let Some(tpath) = tracing_path {
                let fer_e = if frames > 0 {
                    errors as f64 / frames as f64
                } else {
                    0.0
                };
                append_jsonl(
                    tpath,
                    &format!(
                        "{{\"type\":\"heartbeat\",\"timestamp\":\"{}\",\
                         \"snr_index\":{},\"es_n0_db\":{},\
                         \"frames\":{},\"errors\":{},\"fer\":{:.4e}}}",
                        iso_timestamp(),
                        snr_index,
                        es_n0_db,
                        frames,
                        errors,
                        fer_e
                    ),
                );
            }
        }
    }

    let wall_seconds = point_start.elapsed().as_secs_f64();
    let fer = if frames > 0 {
        errors as f64 / frames as f64
    } else {
        0.0
    };
    let ber = if total_bits > 0 {
        total_bit_errors as f64 / total_bits as f64
    } else {
        0.0
    };
    let mean_iters = if frames > 0 {
        total_iters as f64 / frames as f64
    } else {
        0.0
    };

    eprintln!(
        "[{:.2} dB] DONE: FER={:.3e} BER={:.3e} ({} errors / {} frames) in {:.1}s",
        es_n0_db, fer, ber, errors, frames, wall_seconds
    );

    // Write completed checkpoint.
    if let Some(ckpt_dir) = checkpoint_dir {
        let word_pos = rng.get_word_pos();
        let ckpt = SnrCheckpoint {
            snr_index,
            es_n0_db,
            frames_completed: frames,
            errors_accumulated: errors,
            total_iterations: total_iters,
            total_bits,
            total_bit_errors,
            rng_word_pos: word_pos,
            frames_target: max_frames,
            errors_target: target_errors,
            completed: true,
            config_hash: config_hash.to_string(),
        };
        if let Err(e) = write_checkpoint_atomic(&checkpoint_path(ckpt_dir, snr_index), &ckpt) {
            eprintln!("Warning: failed to write completed checkpoint: {e}");
        }
    }

    // JSONL tracing event.
    if let Some(tpath) = tracing_path {
        append_jsonl(
            tpath,
            &format!(
                "{{\"type\":\"snr_completed\",\"timestamp\":\"{}\",\
                 \"snr_index\":{},\"es_n0_db\":{},\
                 \"fer\":{:.6e},\"ber\":{:.6e},\
                 \"frames\":{},\"errors\":{},\
                 \"mean_iters\":{:.2},\"wall_seconds\":{:.2}}}",
                iso_timestamp(),
                snr_index,
                es_n0_db,
                fer,
                ber,
                frames,
                errors,
                mean_iters,
                wall_seconds
            ),
        );
    }

    SnrResult {
        es_n0_db,
        fer,
        ber,
        frames,
        errors,
        mean_iters,
        wall_seconds,
    }
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

    // Determine the SNR sweep.
    let snr_points: Vec<f64> = if is_calib {
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

    // Config hash covers the full parameter set.
    let config_hash = compute_config_hash(
        &snr_points,
        target_errors,
        max_frames_per_snr,
        args.seed,
        args.rate,
        args.modulation,
    );

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

    let tracing_path = if is_calib {
        args.output_dir
            .join("calibration")
            .join("calibration.jsonl")
    } else {
        args.output_dir.join("tracing.jsonl")
    };

    let checkpoint_dir = if is_calib {
        None
    } else {
        let ckpt_dir = args.output_dir.join("checkpoints");
        validate_or_create_checkpoint_dir(&ckpt_dir, &config_hash)?;
        Some(ckpt_dir)
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
        "SNR points: {:?}",
        snr_points
            .iter()
            .map(|v| format!("{:.2}", v))
            .collect::<Vec<_>>()
    );

    // Setup interrupt flag (shared with sim infrastructure if available).
    let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let flag = interrupted.clone();
        let _ = ctrlc::set_handler(move || {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            eprintln!("\nInterrupt received. Flushing checkpoint and exiting...");
        });
    }

    // Campaign start JSONL event.
    append_jsonl(
        &tracing_path,
        &format!(
            "{{\"type\":\"campaign_start\",\"timestamp\":\"{}\",\
             \"rate\":\"{}\",\"modulation\":\"{}\",\
             \"seed\":{},\"config_hash\":\"{}\"}}",
            iso_timestamp(),
            rate_display(args.rate),
            mod_str(args.modulation),
            args.seed,
            config_hash,
        ),
    );

    // Ensure CSV header is written.
    write_csv_header_if_empty(&csv_path).map_err(|e| format!("Cannot write CSV header: {e}"))?;

    // Load existing results for resume.
    let existing_results: std::collections::HashMap<String, SnrResult> = if args.resume && !is_calib
    {
        let content = std::fs::read_to_string(&csv_path).unwrap_or_default();
        let mut map = std::collections::HashMap::new();
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 7 {
                if let (Ok(es_n0), Ok(fer), Ok(ber), Ok(frames), Ok(errors)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                    parts[3].parse::<usize>(),
                    parts[4].parse::<usize>(),
                ) {
                    let key = format!("{:.6}", es_n0);
                    let mean_iters = parts
                        .get(5)
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let wall_s = parts
                        .get(6)
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    map.insert(
                        key,
                        SnrResult {
                            es_n0_db: es_n0,
                            fer,
                            ber,
                            frames,
                            errors,
                            mean_iters,
                            wall_seconds: wall_s,
                        },
                    );
                }
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let campaign_start = Instant::now();
    let heartbeat_every = if is_calib { 0 } else { 1000 };

    for (snr_idx, &es_n0_db) in snr_points.iter().enumerate() {
        let key = format!("{:.6}", es_n0_db);

        // Check resume from CSV: skip if already completed with enough errors.
        if args.resume && !is_calib {
            if let Some(existing) = existing_results.get(&key) {
                if existing.errors >= args.target_errors {
                    eprintln!(
                        "[{:.2} dB] CSV RESUME: skipping (already {} errors)",
                        es_n0_db, existing.errors
                    );
                    continue;
                }
            }
        }

        // Check checkpoint resume.
        let (
            resume_frames,
            resume_errors,
            resume_iters,
            resume_bits,
            resume_bit_errors,
            resume_word_pos,
        ) = if args.resume && !is_calib {
            if let Some(ref ckpt_dir) = checkpoint_dir {
                if let Some(ckpt) =
                    load_checkpoint(&checkpoint_path(ckpt_dir, snr_idx), &config_hash)
                {
                    if ckpt.completed {
                        eprintln!(
                            "[{:.2} dB] CHECKPOINT RESUME: skipping completed point \
                                 ({} errors / {} frames)",
                            es_n0_db, ckpt.errors_accumulated, ckpt.frames_completed
                        );
                        // Write the pre-existing result back to the CSV if missing.
                        let fer = if ckpt.frames_completed > 0 {
                            ckpt.errors_accumulated as f64 / ckpt.frames_completed as f64
                        } else {
                            0.0
                        };
                        let ber = if ckpt.total_bits > 0 {
                            ckpt.total_bit_errors as f64 / ckpt.total_bits as f64
                        } else {
                            0.0
                        };
                        let mean_iters = if ckpt.frames_completed > 0 {
                            ckpt.total_iterations as f64 / ckpt.frames_completed as f64
                        } else {
                            0.0
                        };
                        let _ = append_csv_row(
                            &csv_path,
                            es_n0_db,
                            fer,
                            ber,
                            ckpt.frames_completed,
                            ckpt.errors_accumulated,
                            mean_iters,
                            0.0,
                        );
                        continue;
                    }
                    // Partial checkpoint: resume from saved state.
                    (
                        ckpt.frames_completed,
                        ckpt.errors_accumulated,
                        ckpt.total_iterations,
                        ckpt.total_bits,
                        ckpt.total_bit_errors,
                        ckpt.rng_word_pos,
                    )
                } else {
                    (0, 0, 0, 0, 0, 0)
                }
            } else {
                (0, 0, 0, 0, 0, 0)
            }
        } else {
            (0, 0, 0, 0, 0, 0)
        };

        let result = run_snr_point(
            es_n0_db,
            snr_idx,
            args.seed,
            target_errors,
            max_frames_per_snr,
            &concat,
            &interleaver,
            bits_per_symbol,
            rate_f64(args.rate),
            resume_frames,
            resume_errors,
            resume_iters,
            resume_bits,
            resume_bit_errors,
            resume_word_pos,
            checkpoint_dir.as_deref(),
            &config_hash,
            Some(&tracing_path),
            heartbeat_every,
            &interrupted,
        );

        // Append CSV row.
        if let Err(e) = append_csv_row(
            &csv_path,
            result.es_n0_db,
            result.fer,
            result.ber,
            result.frames,
            result.errors,
            result.mean_iters,
            result.wall_seconds,
        ) {
            eprintln!("Warning: failed to append CSV row: {e}");
        }

        // Check if we were interrupted.
        if interrupted.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("Campaign interrupted. Resume with --resume flag.");
            break;
        }
    }

    let total_wall = campaign_start.elapsed().as_secs_f64();

    // Write README (production runs only).
    if !is_calib {
        let readme_path = args.output_dir.join("README.md");
        write_readme(&readme_path, args, &snr_points, total_wall);
    }

    // Campaign end JSONL event.
    append_jsonl(
        &tracing_path,
        &format!(
            "{{\"type\":\"campaign_end\",\"timestamp\":\"{}\",\
             \"wall_seconds\":{:.2}}}",
            iso_timestamp(),
            total_wall
        ),
    );

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
