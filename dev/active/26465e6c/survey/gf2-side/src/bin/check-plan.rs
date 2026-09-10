//! Validates a runner plan against its frozen addendum before any lock is
//! taken (jit:26465e6c), with the same decoders and checks the runner applies
//! when it opens a session.
//!
//! Usage: `check-plan <plan.json>` from the repository root.

use std::process::ExitCode;
use tuning_campaign_support::protocol::{FamilyAddendum, RunnerPlan};

fn check(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    let plan = RunnerPlan::decode(&bytes)?;
    let addendum_bytes = std::fs::read(&plan.addendum)
        .map_err(|error| format!("cannot read {}: {error}", plan.addendum))?;
    let addendum = FamilyAddendum::decode(&addendum_bytes)?;
    addendum.validate().map_err(|errors| errors.join("; "))?;
    plan.validate(&addendum)
        .map_err(|errors| errors.join("; "))?;
    let unresolved: Vec<String> = addendum
        .cells
        .iter()
        .flat_map(|cell| {
            addendum
                .unresolved_settings(cell)
                .into_iter()
                .map(move |name| format!("{}: {name}", cell.cell_id))
        })
        .collect();
    Ok(format!(
        "plan {path}: {} cells over {} arms validate against {} (family {}); unresolved settings: {}",
        plan.cells.len(),
        plan.arms.len(),
        plan.addendum,
        addendum.family.id,
        if unresolved.is_empty() {
            "none".to_owned()
        } else {
            unresolved.len().to_string()
        }
    ))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path] = args.as_slice() else {
        eprintln!("usage: check-plan <plan.json>");
        return ExitCode::from(2);
    };
    match check(path) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("check-plan: {error}");
            ExitCode::FAILURE
        }
    }
}
