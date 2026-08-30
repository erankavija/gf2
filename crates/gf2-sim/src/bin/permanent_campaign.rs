//! Execute one exact cell of a permanent-zero-fraction campaign.
//!
//! The manifest is read from `--manifest`; `--output` names the campaign
//! directory, `--q FIELD --n ORDER` selects exactly one manifest cell, and
//! `--workers N` selects the configured worker count (default: 1 when omitted).
//! The binary persists coordinator admission before executing manifested shard
//! attempts, then writes the selected cell's raw records through that receipt
//! state machine. Before execution, it passes the output directory through
//! `approve_emission`; the guard contract binds this writer to the frozen
//! emitter identity.
//!
//! ```console
//! $ permanent_campaign --print-provenance --manifest <campaign-directory>
//! ```
//!
//! `--print-provenance` observes and prints the provenance for this emitting
//! executable without running a campaign or writing a dataset file.
//!
//! Exact execution does not write a field summary. The outside campaign
//! coordinator assembles that summary only after every cell in the field is
//! terminal. `--dry-run-schedule` resolves the same selector and validates the
//! complete manifest accelerator-cost input without opening the output
//! directory, coordinator receipt, execution lock, or sampler.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use gf2_sim::permanent_campaign::coordinator::{
    coordinator_receipt_path, execute_scheduled_cell, ArmInvocation, CampaignCoordinator,
    CellExecutionState, ExactCellScope, ShardAttemptState,
};
use gf2_sim::permanent_campaign::launch_cost::read_accelerator_cost_table;
use gf2_sim::permanent_campaign::provenance::{
    approve_emission, observe_provenance, repository_top_level,
};
use gf2_sim::permanent_campaign::schedule::{
    enumerate_cell_work_items, AcceleratorCostTable, DEFAULT_ACCELERATOR_LAUNCH_CAP,
};
use gf2_sim::permanent_campaign::schema::{read_manifest, Backend};

const USAGE: &str = "usage: permanent_campaign --manifest PATH --output CAMPAIGN-DIR --q FIELD --n ORDER [--workers N] [--accelerator-launch-cap-ms MS] [--accelerator-cost-table PATH]

       permanent_campaign --dry-run-schedule --manifest PATH --output CAMPAIGN-DIR --q FIELD --n ORDER

       permanent_campaign --print-provenance --manifest PATH

Accelerator options:
  --accelerator-launch-cap-ms MS     target cap per launch (default: 500 ms)
  --accelerator-cost-table PATH      required when the manifest has accelerator cells;
                                     CSV of measured per-matrix costs with header
                                     q,n,per_matrix_us, one row per accelerator cell,
                                     each value taken from that cell's committed receipt
";

fn main() -> ExitCode {
    let effective_argv: Vec<String> = std::env::args().collect();
    let arguments = effective_argv[1..].to_vec();
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

    let (Some(manifest_path), Some(output), Some(field), Some(order)) =
        (manifest_path, output, field, order)
    else {
        return if field.is_some() || order.is_some() {
            usage("exact cell selector requires both --q and --n")
        } else {
            usage("missing required argument")
        };
    };
    let scope = ExactCellScope { q: field, n: order };
    let manifest = match read_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => return failure(error),
    };
    let items = match enumerate_cell_work_items(&manifest, field, order) {
        Ok(items) => items,
        Err(error) => return failure(error),
    };
    if dry_run_schedule {
        if manifest
            .cells
            .iter()
            .any(|cell| cell.backend == Backend::Accelerator)
            && accelerator_cost_table.is_none()
        {
            return usage(
                "--accelerator-cost-table is required for a manifest with accelerator cells; supply every cell's measured per-matrix cost from its committed measurement receipt",
            );
        }
        let accelerator = match accelerator_cost_table {
            Some(path) => {
                match read_accelerator_cost_table(&path, &manifest, accelerator_launch_cap) {
                    Ok(table) => table,
                    Err(error) => {
                        eprintln!("accelerator cost table {}: {error}", path.display());
                        return ExitCode::FAILURE;
                    }
                }
            }
            None => AcceleratorCostTable::default(),
        };
        if let Err(error) = accelerator.validate_manifest(&manifest) {
            return failure(error);
        }
        println!("schedule q={field} n={order} shards={}", items.len());
        return ExitCode::SUCCESS;
    }
    if let Err(refusal) = approve_emission(&output) {
        eprintln!("emission refused: {refusal}");
        return ExitCode::FAILURE;
    }
    let output_manifest = match read_manifest(&output) {
        Ok(output_manifest) if output_manifest == manifest => output_manifest,
        Ok(_) => return failure("output manifest differs from --manifest"),
        Err(error) => return failure(error),
    };
    let receipt_path = coordinator_receipt_path(&output, &output_manifest.campaign_id);
    let mut coordinator = if receipt_path.exists() {
        match CampaignCoordinator::read(&output) {
            Ok(coordinator) => coordinator,
            Err(error) => return failure(error),
        }
    } else {
        match CampaignCoordinator::new(&output) {
            Ok(coordinator) => coordinator,
            Err(error) => return failure(error),
        }
    };
    let Some(executable_sha256) = output_manifest.provenance.binary_sha256.clone() else {
        return failure("frozen manifest has no emitter executable digest");
    };
    if let Err(error) = coordinator.authorize_or_resume_arm(ArmInvocation {
        scope,
        argv: effective_argv,
        worker_count: workers,
        executable_sha256,
    }) {
        return failure(error);
    }
    if let Err(error) = coordinator.persist(&output) {
        return failure(error);
    }
    let execution = match execute_scheduled_cell(&output, scope, workers) {
        Ok(execution) => execution,
        Err(error) => return failure(error),
    };
    println!("campaign q={field} n={order} workers={workers}");
    for shard in &execution.records {
        println!(
            "q={} n={} shard={} matrices={} zeros={}",
            shard.stream_address.q,
            shard.stream_address.n,
            shard.shard_id,
            shard.matrix_count,
            shard.permanent_zero_count,
        );
    }
    match execution.terminal_state {
        CellExecutionState::Completed { assessment, .. } if !assessment.rejected() => {
            println!("wrote deterministic exact-cell files for q={field} n={order}");
            ExitCode::SUCCESS
        }
        CellExecutionState::Completed { .. } => {
            failure("exact acceptance decision rejected; campaign is halted")
        }
        CellExecutionState::Halted { .. } => {
            match CampaignCoordinator::read(&output)
                .ok()
                .and_then(|coordinator| {
                    coordinator
                        .receipt()
                        .attempts
                        .last()
                        .and_then(|attempt| match &attempt.state {
                            ShardAttemptState::Quarantined { error } => Some(error.clone()),
                            ShardAttemptState::Authorized | ShardAttemptState::Accepted { .. } => {
                                None
                            }
                        })
                }) {
                Some(error) => failure(format!(
                    "exact execution halted mechanically; campaign is halted: {error}"
                )),
                None => failure("exact execution halted mechanically; campaign is halted"),
            }
        }
        CellExecutionState::Pending | CellExecutionState::Scheduled { .. } => {
            failure("exact execution returned a nonterminal receipt state")
        }
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!("{message}\n{USAGE}");
    ExitCode::from(64)
}

fn failure(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}
