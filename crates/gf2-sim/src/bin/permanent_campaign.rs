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
//!
//! ```console
//! $ permanent_campaign --print-provenance --manifest <campaign-directory>
//! ```
//!
//! `--print-provenance` observes and prints the provenance for this emitting
//! executable without running a campaign or writing a dataset file.
//!
//! `--n ORDER` narrows `--q FIELD` to one exact manifest cell. Exact execution
//! uses a cell-scoped checkpoint and deliberately does not write a field
//! summary; the outside campaign coordinator assembles that summary only after
//! every cell in the field is terminal. `--dry-run-schedule` resolves the same
//! exact selector without opening the output directory, a checkpoint, or a
//! sampler.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use gf2_sim::permanent_campaign::driver::{
    cell_checkpoint_path, field_checkpoint_path, run_cell_checkpointed_with_accelerator_config,
    run_field_checkpointed_with_accelerator_config,
};
use gf2_sim::permanent_campaign::launch_cost::read_accelerator_cost_table;
use gf2_sim::permanent_campaign::provenance::{
    approve_emission, observe_provenance, repository_top_level,
};
use gf2_sim::permanent_campaign::schedule::{
    enumerate_cell_work_items, AcceleratorCostTable, DEFAULT_ACCELERATOR_LAUNCH_CAP,
};
use gf2_sim::permanent_campaign::schema::{read_manifest, Backend};

const USAGE: &str = "usage: permanent_campaign --manifest PATH --output CAMPAIGN-DIR --q FIELD [--n ORDER] [--workers N] [--accelerator-launch-cap-ms MS] [--accelerator-cost-table PATH]

       permanent_campaign --dry-run-schedule --manifest PATH --output CAMPAIGN-DIR --q FIELD --n ORDER

       permanent_campaign --print-provenance --manifest PATH

Accelerator options:
  --accelerator-launch-cap-ms MS     target cap per launch (default: 500 ms)
  --accelerator-cost-table PATH      required when the selected field has accelerator
                                     cells; CSV of measured per-matrix costs with header
                                     q,n,per_matrix_us, one row per accelerator cell,
                                     each value taken from that cell's committed receipt
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut manifest_path = None;
    let mut output = None;
    let mut field = None;
    let mut order = None;
    let mut workers = 1usize;
    let mut accelerator_launch_cap = DEFAULT_ACCELERATOR_LAUNCH_CAP;
    let mut accelerator_cost_table: Option<PathBuf> = None;
    let mut print_provenance = false;
    let mut dry_run_schedule = false;
    let mut index = 0;
    while index < arguments.len() {
        let name = arguments[index].as_str();
        let value = arguments.get(index + 1).map(String::as_str);
        match (name, value) {
            ("--print-provenance", _) => {
                print_provenance = true;
                index += 1;
            }
            ("--dry-run-schedule", _) => {
                dry_run_schedule = true;
                index += 1;
            }
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
            ("--n", Some(value)) => match value.parse::<u16>() {
                Ok(n) if n > 0 => {
                    order = Some(n);
                    index += 2;
                }
                _ => return usage("exact cell selector --n must be a positive integer order"),
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
            ("--accelerator-cost-table", Some(value)) => {
                accelerator_cost_table = Some(PathBuf::from(value));
                index += 2;
            }
            _ => return usage("unrecognized or incomplete argument"),
        }
    }

    if print_provenance {
        let Some(manifest_path) = manifest_path else {
            return usage("--print-provenance requires --manifest");
        };
        if output.is_some() || field.is_some() || order.is_some() || dry_run_schedule {
            return usage(
                "--print-provenance cannot be combined with execution or dry-schedule options",
            );
        }
        let manifest = match read_manifest(&manifest_path) {
            Ok(manifest) => manifest,
            Err(error) => return failure(error),
        };
        let repository = match repository_top_level(&manifest_path) {
            Ok(repository) => repository,
            Err(error) => return failure(error),
        };
        let observed = match observe_provenance(&repository, manifest.provenance) {
            Ok(observed) => observed,
            Err(error) => return failure(error),
        };
        return match serde_json::to_string_pretty(&observed) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => failure(error),
        };
    }

    let (Some(manifest_path), Some(output)) = (manifest_path, output) else {
        return usage("missing required argument");
    };
    let Some(field) = field else {
        return if order.is_some() {
            usage("exact cell selector --n requires --q")
        } else {
            usage("missing required argument")
        };
    };
    let manifest = match read_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => return failure(error),
    };
    if let Some(order) = order {
        let items = match enumerate_cell_work_items(&manifest, field, order) {
            Ok(items) => items,
            Err(error) => return failure(error),
        };
        if dry_run_schedule {
            println!("schedule q={field} n={order} shards={}", items.len());
            return ExitCode::SUCCESS;
        }
    } else if dry_run_schedule {
        return usage("exact cell selector --dry-run-schedule requires --q and --n");
    }
    if manifest
        .cells
        .iter()
        .any(|cell| cell.q == field && cell.backend == Backend::Accelerator)
        && accelerator_cost_table.is_none()
    {
        return usage(
            "--accelerator-cost-table is required for accelerator cells; supply each cell's measured per-matrix cost from its committed measurement receipt",
        );
    }
    let accelerator = match accelerator_cost_table {
        Some(path) => match read_accelerator_cost_table(&path, &manifest, accelerator_launch_cap) {
            Ok(table) => table,
            Err(error) => {
                eprintln!("accelerator cost table {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => AcceleratorCostTable::default(),
    };
    if let Err(refusal) = approve_emission(&output) {
        eprintln!("emission refused: {refusal}");
        return ExitCode::FAILURE;
    }
    let run = match order {
        Some(order) => run_cell_checkpointed_with_accelerator_config(
            &output,
            &manifest,
            field,
            order,
            &cell_checkpoint_path(&output, field, order),
            workers,
            &accelerator,
        ),
        None => run_field_checkpointed_with_accelerator_config(
            &output,
            &manifest,
            field,
            &field_checkpoint_path(&output, field),
            workers,
            &accelerator,
        ),
    };
    let run = match run {
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
