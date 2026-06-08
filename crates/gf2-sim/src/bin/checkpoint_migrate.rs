//! One-shot v1 → v2 checkpoint migration tool (design doc §4 "Migration tool").
//!
//! Converts legacy per-SNR checkpoints written by
//! `gf2_coding::simulation::SimulationRunner` (schema v1) into the
//! `gf2-sim` v2 schema ([`gf2_sim::checkpoint::CheckpointV2`]). The new pipeline
//! reader is **v2-only**; this binary is the offline path for legacy data.
//!
//! # Usage
//!
//! ```text
//! checkpoint_migrate --input <v1-dir> --output <v2-dir> [--parallelism N]
//! ```
//!
//! Reads each `<v1-dir>/snr_NNNN.json`, synthesises a **single-worker** v2
//! checkpoint (`worker_states[0].frames_in_worker = frames_completed`,
//! `worker_states[0].rng_word_pos = rng_word_pos`), and writes it to
//! `<v2-dir>/snr_NNNN.json`. The single-worker mapping is exact: a v1 run was
//! single-threaded, so worker 0 owns every frame and its recorded
//! `rng_word_pos` is the legacy stream position verbatim.
//!
//! `--parallelism N` is accepted for forward-compatibility with the CLI in the
//! design doc but does **not** re-partition a v1 stream: a single-threaded v1
//! RNG position cannot be split into N independent per-worker positions without
//! re-running. With `N > 1` the tool still emits a single populated worker
//! (worker 0) carrying the full count and position, plus `N-1` empty workers,
//! so a resume under N workers re-runs deterministically from the v1 position on
//! worker 0. This is documented and intentional.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;

use gf2_sim::checkpoint::{CheckpointV2, CheckpointWriter, WorkerState, SCHEMA_VERSION};

/// The legacy v1 per-SNR checkpoint schema (mirrors
/// `gf2_coding::simulation`'s `SnrCheckpoint`). All counters are plain JSON
/// numbers; `rng_word_pos` is a decimal string.
#[derive(Debug, Deserialize)]
struct V1Checkpoint {
    snr_index: usize,
    eb_n0_db: f64,
    frames_completed: u64,
    errors_accumulated: u64,
    total_iterations: u64,
    total_queries: u64,
    total_bits: u64,
    total_bit_errors: u64,
    /// Decimal string (`u128` does not fit a JSON number above `2^53`).
    rng_word_pos: String,
    frames_target: u64,
    errors_target: u64,
    completed: bool,
    config_hash: String,
}

/// Parsed command-line arguments.
struct Args {
    input: PathBuf,
    output: PathBuf,
    parallelism: usize,
}

fn parse_args() -> Result<Args, String> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut parallelism: usize = 1;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--input" => {
                input = Some(PathBuf::from(it.next().ok_or("--input requires a value")?));
            }
            "--output" => {
                output = Some(PathBuf::from(it.next().ok_or("--output requires a value")?));
            }
            "--parallelism" => {
                parallelism = it
                    .next()
                    .ok_or("--parallelism requires a value")?
                    .parse()
                    .map_err(|e| format!("--parallelism must be a positive integer: {e}"))?;
                if parallelism == 0 {
                    return Err("--parallelism must be >= 1".to_string());
                }
            }
            "-h" | "--help" => {
                return Err("help".to_string());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        input: input.ok_or("--input <v1-dir> is required")?,
        output: output.ok_or("--output <v2-dir> is required")?,
        parallelism,
    })
}

const USAGE: &str = "checkpoint_migrate --input <v1-dir> --output <v2-dir> [--parallelism N]";

/// Converts one v1 checkpoint into a single-(plus-empty)-worker v2 checkpoint.
///
/// Returns an error string if the recorded `rng_word_pos` is not a valid
/// `u128`.
fn migrate_one(v1: &V1Checkpoint, parallelism: usize) -> Result<CheckpointV2, String> {
    let rng_word_pos = v1.rng_word_pos.parse::<u128>().map_err(|e| {
        format!(
            "snr_{:04}: bad rng_word_pos {:?}: {e}",
            v1.snr_index, v1.rng_word_pos
        )
    })?;

    // Worker 0 carries the entire v1 stream; any extra workers are empty
    // (a single-threaded v1 position cannot be split — see the module docs).
    let mut worker_states = Vec::with_capacity(parallelism);
    worker_states.push(WorkerState {
        worker_idx: 0,
        frames_in_worker: v1.frames_completed,
        rng_word_pos,
    });
    for w in 1..parallelism {
        worker_states.push(WorkerState {
            worker_idx: w,
            frames_in_worker: 0,
            // Worker w's fresh start position (frame 0 of its partition).
            rng_word_pos: gf2_sim::worker_offset(0, v1.snr_index, w, 0),
        });
    }

    Ok(CheckpointV2 {
        schema_version: SCHEMA_VERSION,
        snr_index: v1.snr_index,
        // v1 stored Eb/N0; v2's diagnostic field carries it through verbatim
        // (the migration does not re-derive Es/N0 — the value is informational).
        esn0_db: v1.eb_n0_db,
        config_hash: v1.config_hash.clone(),
        frames_target: v1.frames_target,
        errors_target: v1.errors_target,
        max_frames: v1.frames_target,
        frames_completed: v1.frames_completed,
        errors_accumulated: v1.errors_accumulated,
        total_iterations: v1.total_iterations,
        total_queries: v1.total_queries,
        total_bits: v1.total_bits,
        total_bit_errors: v1.total_bit_errors,
        completed: v1.completed,
        worker_states,
        drain_committed_at_us_since_epoch: 0,
    })
}

/// Migrates every `snr_NNNN.json` in `input` into `output`. Returns the number
/// of files converted, or an error string on the first failure.
fn run(input: &Path, output: &Path, parallelism: usize) -> Result<usize, String> {
    if !input.is_dir() {
        return Err(format!("input is not a directory: {}", input.display()));
    }
    let writer = CheckpointWriter::new(output)
        .map_err(|e| format!("cannot create output dir {}: {e}", output.display()))?;

    let mut entries: Vec<PathBuf> = std::fs::read_dir(input)
        .map_err(|e| format!("cannot read input dir {}: {e}", input.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| is_snr_json(p))
        .collect();
    entries.sort();

    let mut count = 0usize;
    for path in entries {
        let bytes =
            std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let v1: V1Checkpoint = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{} is not a v1 checkpoint: {e}", path.display()))?;
        let v2 = migrate_one(&v1, parallelism)?;
        writer
            .write(&v2)
            .map_err(|e| format!("cannot write v2 checkpoint for snr {}: {e}", v1.snr_index))?;
        count += 1;
    }
    Ok(count)
}

/// True iff `path` is named `snr_<digits>.json`.
fn is_snr_json(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.starts_with("snr_")
        && name.ends_with(".json")
        && name["snr_".len()..name.len() - ".json".len()]
            .chars()
            .all(|c| c.is_ascii_digit())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            if msg == "help" {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            eprintln!("error: {msg}\nusage: {USAGE}");
            return ExitCode::from(2);
        }
    };

    match run(&args.input, &args.output, args.parallelism) {
        Ok(n) => {
            println!(
                "migrated {n} checkpoint(s) from {} to {} (v1 -> v2)",
                args.input.display(),
                args.output.display()
            );
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}
