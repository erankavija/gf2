//! Execute one field arm of a permanent-zero-fraction campaign.
//!
//! The manifest is read from `--manifest`; `--output` names the campaign
//! directory and `--q` selects exactly one field. The binary reports timings
//! to standard output and writes only the selected field's shard records and
//! summary. Before execution, it passes the output directory through
//! `approve_emission`; the guard contract binds writers, and this binary is the
//! writer, while `emit_field` remains the library emission primitive. Use
//! `permanent_dataset conform` after all field arms and campaign finalization
//! files are present.

use std::path::PathBuf;
use std::process::ExitCode;

use gf2_sim::permanent_campaign::driver::run_field_checkpointed;
use gf2_sim::permanent_campaign::provenance::approve_emission;
use gf2_sim::permanent_campaign::schema::read_manifest;

const USAGE: &str = "usage: permanent_campaign --manifest PATH --output CAMPAIGN-DIR --q FIELD";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut manifest_path = None;
    let mut output = None;
    let mut field = None;
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
    if let Err(refusal) = approve_emission(&output) {
        eprintln!("emission refused: {refusal}");
        return ExitCode::FAILURE;
    }
    let checkpoint = output.join("campaign.checkpoint.json");
    let run = match run_field_checkpointed(&output, &manifest, field, &checkpoint, 1) {
        Ok(run) => run,
        Err(error) => return failure(error),
    };
    for shard in run.shards() {
        println!(
            "q={} n={} shard={} matrices={} zeros={} draw_s={:.6} pack_s={:.6} evaluate_s={:.6} count_s={:.6}",
            run.q(),
            shard.record.stream_address.n,
            shard.record.shard_id,
            shard.record.matrix_count,
            shard.record.permanent_zero_count,
            shard.timing.draw.as_secs_f64(),
            shard.timing.pack.as_secs_f64(),
            shard.timing.evaluate.as_secs_f64(),
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
