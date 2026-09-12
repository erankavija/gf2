//! Pre-flight check of saved runner plans (jit:3be770d5).
//!
//! For each plan path, decodes the plan strictly, decodes the addendum it
//! names, validates that addendum against `addendum.schema.json` and the
//! protocol's semantic rules, and validates the plan against the addendum:
//! the same checks the runner applies before opening a campaign. Times
//! nothing and writes nothing. Run from the repository root.
//!
//! Usage: ldpc-plan-check PLAN...

use serde_json::Value;
use std::process::ExitCode;
use tuning_campaign_support::protocol::{FamilyAddendum, RunnerPlan, ADDENDUM_SCHEMA_PATH};
use tuning_campaign_support::schema;

fn check(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let plan = RunnerPlan::decode(&bytes)?;
    let addendum_bytes =
        std::fs::read(&plan.addendum).map_err(|e| format!("{}: {e}", plan.addendum))?;
    let schema_text = std::fs::read(ADDENDUM_SCHEMA_PATH).map_err(|e| e.to_string())?;
    let schema: Value = serde_json::from_slice(&schema_text).map_err(|e| e.to_string())?;
    let instance: Value = serde_json::from_slice(&addendum_bytes).map_err(|e| e.to_string())?;
    let violations = schema::validate(&schema, &instance);
    if !violations.is_empty() {
        return Err(format!(
            "{}: schema violations {violations:?}",
            plan.addendum
        ));
    }
    let addendum = FamilyAddendum::decode(&addendum_bytes)?;
    addendum
        .validate()
        .map_err(|errors| format!("{}: {}", plan.addendum, errors.join("; ")))?;
    plan.validate(&addendum)
        .map_err(|errors| format!("{path}: {}", errors.join("; ")))?;
    Ok(format!(
        "{path}: {} cells of {} valid",
        plan.cells.len(),
        addendum.family.id
    ))
}

fn main() -> ExitCode {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        match check(&path) {
            Ok(line) => println!("{line}"),
            Err(error) => {
                eprintln!("ldpc-plan-check: {error}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
