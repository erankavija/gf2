//! Non-timed wire-contract smoke of this issue's arms (jit:f63a2464).
//!
//! Drives every arm of a saved runner plan the way the runner drives it — the
//! runner's own case encoder, its request sentinel and its child environment —
//! but in the `validation` role, so each arm performs one untimed dispatch,
//! applies every placement and decision check and returns no timing window.
//! Reading an arm's source does not establish the wire contract between the
//! runner and a child; running it does.
//!
//! The smoke records zero timing windows and refuses an arm that reports one.
//! It publishes no receipt: its output is a verdict on standard output.
//!
//! Usage (from the worktree root): qc-arm-smoke PLAN...

use ldpc_survey::arm::{ConversionCosts, Quality, Request, ARM_RESULT_SCHEMA};
use ldpc_throughput::driver::VALIDATION_ROLE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, ExitCode, Stdio};
use tuning_campaign_support::campaign::measurement_environment;
use tuning_campaign_support::protocol::{RunnerPlan, SHARED_SETTINGS};
use tuning_campaign_support::transport::{self, FRESH_CASE_VALUE, FRESH_CASE_VAR};

/// The request envelope the runner sends an arm child.
pub const ARM_REQUEST_SCHEMA: &str = "zen3-benchmark-arm-request-v1";

/// The arm result line, in the arm's own field order.
///
/// The arm-side type is serialize-only, and the canonical result parser
/// re-encodes what it decoded, so a smoke needs a mirror that reproduces the
/// arm's field order exactly. That makes this struct a pin on the result
/// contract: a field added, removed or reordered on either side fails here.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArmResultMirror {
    schema: String,
    windows: Vec<Value>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<ConversionCosts>,
    quality: Option<Quality>,
    calibrated: bool,
}

/// The request the runner would send for this cell, in the validation role.
fn mirror(cell_id: &str, arm: &str, case: Value, cpus: Vec<u32>, workers: u32) -> Request {
    Request {
        schema: ARM_REQUEST_SCHEMA.to_owned(),
        cell_id: cell_id.to_owned(),
        arm: arm.to_owned(),
        role: VALIDATION_ROLE.to_owned(),
        pair: 0,
        case,
        cache_state: "warm".to_owned(),
        cold_calls: None,
        decoder: None,
        windows: SHARED_SETTINGS.windows_per_execution,
        window_target_ms: SHARED_SETTINGS.window_target_ms,
        cpus,
        workers_declared: workers,
    }
}

fn drive(plan: &RunnerPlan, cell_index: usize, arm_name: &str) -> Result<(), String> {
    let cell = &plan.cells[cell_index];
    let arm = plan
        .arms
        .get(arm_name)
        .ok_or_else(|| format!("plan has no arm {arm_name}"))?;
    let workers = declared_workers(plan, &cell.cell_id)?;
    let cpus: Vec<u32> = (0..workers).collect();
    let request = mirror(&cell.cell_id, arm_name, cell.case.clone(), cpus, workers);
    let encoded = transport::encode_case(&request)?;

    let mut command = Command::new(&arm.executable);
    command.args(&arm.arguments);
    command.env_clear();
    for name in ["PATH", "HOME"] {
        if let Ok(value) = std::env::var(name) {
            command.env(name, value);
        }
    }
    for (name, value) in measurement_environment() {
        command.env(name, value);
    }
    for (name, value) in &arm.environment {
        command.env(name, value);
    }
    command.env(FRESH_CASE_VAR, FRESH_CASE_VALUE);

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{}: {error}", arm.executable))?;
    child
        .stdin
        .take()
        .ok_or("child has no stdin")?
        .write_all(encoded.as_bytes())
        .map_err(|error| error.to_string())?;
    let finished = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !finished.status.success() {
        return Err(format!(
            "{arm_name} on {} exited {}: {}",
            cell.cell_id,
            finished.status,
            String::from_utf8_lossy(&finished.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&finished.stdout).into_owned();
    let result: ArmResultMirror = transport::parse_result(&text)?;
    if result.schema != ARM_RESULT_SCHEMA {
        return Err(format!(
            "{arm_name} on {} wrote another schema",
            cell.cell_id
        ));
    }
    if !result.windows.is_empty() {
        return Err(format!(
            "{arm_name} on {} reported {} timing windows; a smoke records none",
            cell.cell_id,
            result.windows.len()
        ));
    }
    if result.workers_observed != workers {
        return Err(format!(
            "{arm_name} on {} observed {} workers, declared {workers}",
            cell.cell_id, result.workers_observed
        ));
    }
    println!(
        "{}\t{arm_name}\tvalidated\tworkers={workers}\twindows=0",
        cell.cell_id
    );
    Ok(())
}

/// Workers the plan's addendum declares for `cell_id`.
fn declared_workers(plan: &RunnerPlan, cell_id: &str) -> Result<u32, String> {
    let bytes =
        std::fs::read(&plan.addendum).map_err(|error| format!("{}: {error}", plan.addendum))?;
    let addendum = tuning_campaign_support::protocol::FamilyAddendum::decode(&bytes)?;
    addendum
        .cell(cell_id)
        .map(|cell| cell.workers.declared)
        .ok_or_else(|| format!("the addendum declares no cell {cell_id}"))
}

fn run() -> Result<(), String> {
    let plans: Vec<String> = std::env::args().skip(1).collect();
    if plans.is_empty() {
        return Err("usage: qc-arm-smoke PLAN...".to_owned());
    }
    for path in plans {
        let bytes = std::fs::read(&path).map_err(|error| format!("{path}: {error}"))?;
        let plan = RunnerPlan::decode(&bytes)?;
        for index in 0..plan.cells.len() {
            let cell = &plan.cells[index];
            let (baseline, candidate) = (cell.baseline_arm.clone(), cell.candidate_arm.clone());
            drive(&plan, index, &baseline)?;
            drive(&plan, index, &candidate)?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qc-arm-smoke: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The request mirror is the runner's request: the runner's own encoder
    /// accepts it, the arm-side contract decodes it with unknown fields
    /// rejected, and the re-encode is byte-identical, so no field is added,
    /// dropped or renamed on either side without this failing.
    #[test]
    fn the_request_mirror_matches_the_runner_request_on_the_wire() {
        let case = serde_json::json!({
            "bundle": "target/ldpc-inputs/nr-bg1-z384-mother",
            "code": "nr-bg1-r12",
            "iteration_cap": 50,
            "normalization_factor": 0.75,
            "syndrome_stopping": true,
            "batch_size": 8,
            "quality_frames": 128
        });
        let request = mirror(
            "nr-bg1-z384-qc-w1",
            "qc-intra-frame-nms-f32",
            case,
            vec![0],
            1,
        );
        let encoded = transport::encode_case(&request).expect("the runner's encoder");
        let decoded: Request =
            transport::read_guarded_case(Some(FRESH_CASE_VALUE), encoded.as_bytes())
                .expect("the arm's decoder");
        let reencoded = transport::encode_case(&decoded).expect("the runner's encoder");
        assert_eq!(encoded, reencoded);
        assert_eq!(decoded.schema, ARM_REQUEST_SCHEMA);
        assert_eq!(decoded.role, VALIDATION_ROLE);
        assert_eq!(decoded.windows, SHARED_SETTINGS.windows_per_execution);
        assert_eq!(decoded.window_target_ms, SHARED_SETTINGS.window_target_ms);
    }
}
