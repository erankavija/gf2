//! Execute one field arm of a permanent-zero-fraction campaign.
//!
//! The manifest is read from `--manifest`; `--output` names the campaign
//! directory, `--q` selects exactly one field, and `--workers N` selects the
//! configured worker count (default: 1 when omitted). The binary reports the
//! effective campaign configuration and timings to standard output and writes
//! only the selected field's shard records and summary. Before execution, it
//! passes the output directory through
//! `approve_emission`; the guard contract binds writers, and this binary is the
//! writer, while `emit_field` remains the library emission primitive. Use
//! `permanent_dataset conform` after all field arms and campaign finalization
//! files are present.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use gf2_sim::permanent_campaign::driver::{
    field_checkpoint_path, run_field_checkpointed_with_accelerator_config,
};
use gf2_sim::permanent_campaign::provenance::approve_emission;
use gf2_sim::permanent_campaign::schedule::{AcceleratorConfig, DEFAULT_ACCELERATOR_LAUNCH_CAP};
use gf2_sim::permanent_campaign::schema::{read_manifest, Backend};

const USAGE: &str = "usage: permanent_campaign --manifest PATH --output CAMPAIGN-DIR --q FIELD [--workers N] [--accelerator-launch-cap-ms MS] [--accelerator-per-matrix-us US]

Accelerator options:
  --accelerator-launch-cap-ms MS     target cap per launch (default: 500 ms)
  --accelerator-per-matrix-us US     required for accelerator cells; measured per-matrix
                                     cost in microseconds from the committed measurement receipt
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut manifest_path = None;
    let mut output = None;
    let mut field = None;
    let mut workers = 1usize;
    let mut accelerator_launch_cap = DEFAULT_ACCELERATOR_LAUNCH_CAP;
    let mut accelerator_per_matrix = None;
    let mut index = 0;
    while index < arguments.len() {
        let name = arguments[index].as_str();
        let value = arguments.get(index + 1).map(String::as_str);
        match (name, value) {
            ("--manifest", Some(path)) => {
                manifest_path = Some(PathBuf::from(path));
                index += 2;
            }
            ("--output", Some(path)) => {
                output = Some(PathBuf::from(path));
                index += 2;
            }
            ("--q", Some(value)) => match value.parse::<u8>() {
                Ok(q) => {
                    field = Some(q);
                    index += 2;
                }
                Err(_) => return usage("--q must be an integer field order"),
            },
            ("--workers", Some(value)) => match value.parse::<usize>() {
                Ok(parsed_workers) if parsed_workers >= 1 => {
                    workers = parsed_workers;
                    index += 2;
                }
                _ => return usage("--workers must be an integer of at least 1"),
            },
            ("--accelerator-launch-cap-ms", Some(value)) => match value.parse::<u64>() {
                Ok(milliseconds) if milliseconds > 0 => {
                    accelerator_launch_cap = Duration::from_millis(milliseconds);
                    index += 2;
                }
                _ => return usage("--accelerator-launch-cap-ms must be a positive integer"),
            },
            ("--accelerator-per-matrix-us", Some(value)) => match value.parse::<u64>() {
                Ok(microseconds) if microseconds > 0 => {
                    accelerator_per_matrix = Some(Duration::from_micros(microseconds));
                    index += 2;
                }
                _ => return usage("--accelerator-per-matrix-us must be a positive integer"),
            },
            _ => return usage("unrecognized or incomplete argument"),
        }
    }

    let (Some(manifest_path), Some(output), Some(field)) = (manifest_path, output, field) else {
        return usage("missing required argument");
    };
    let manifest = match read_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => return failure(error),
    };
    if manifest
        .cells
        .iter()
        .any(|cell| cell.backend == Backend::Accelerator)
        && accelerator_per_matrix.is_none()
    {
        return usage(
            "--accelerator-per-matrix-us is required for accelerator cells; use the cell's committed measurement receipt",
        );
    }
    if let Err(refusal) = approve_emission(&output) {
        eprintln!("emission refused: {refusal}");
        return ExitCode::FAILURE;
    }
    let checkpoint = field_checkpoint_path(&output, field);
    let accelerator = AcceleratorConfig {
        per_matrix_cost: accelerator_per_matrix.unwrap_or_else(|| Duration::from_micros(1)),
        launch_cap: accelerator_launch_cap,
    };
    let run = match run_field_checkpointed_with_accelerator_config(
        &output,
        &manifest,
        field,
        &checkpoint,
        workers,
        accelerator,
    ) {
        Ok(run) => run,
        Err(error) => return failure(error),
    };
    println!("campaign q={} workers={workers}", run.q());
    for shard in run.shards() {
        println!(
            "q={} n={} shard={} matrices={} zeros={} draw_s={:.6} pack_s={:.6} evaluate_s={:.6} determinant_s={:.6} count_s={:.6}",
            run.q(),
            shard.record.stream_address.n,
            shard.record.shard_id,
            shard.record.matrix_count,
            shard.record.permanent_zero_count,
            shard.timing.draw.as_secs_f64(),
            shard.timing.pack.as_secs_f64(),
            shard.timing.evaluate.as_secs_f64(),
            shard.timing.determinant.as_secs_f64(),
            shard.timing.count.as_secs_f64(),
        );
    }
    println!("wrote deterministic field files for q={}", run.q());
    ExitCode::SUCCESS
}

fn usage(message: &str) -> ExitCode {
    eprintln!("{message}\n{USAGE}");
    ExitCode::from(64)
}

fn failure(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}
