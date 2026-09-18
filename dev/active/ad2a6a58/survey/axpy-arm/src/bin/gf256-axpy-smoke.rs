//! Non-timed harness smoke of this family's arms (jit:ad2a6a58).
//!
//! Drives every arm of every cell of a saved runner plan the way
//! `benchmark-ab-runner` drives it — the runner's own case encoder, its
//! fresh-child sentinel, its child environment and its one-result-line parser —
//! in the `validation` position, so each arm performs one untimed dispatch,
//! reports the lane that dispatch selected and returns no timing window.
//! Reading the runner and the arm side by side does not establish the wire
//! between two processes; running them does.
//!
//! The smoke takes no lock, opens no family ledger and finalizes no receipt. It
//! refuses an arm that reports a timing window, and its whole output is the
//! record on stdout.
//!
//! Usage (from the worktree root): gf256-axpy-smoke PLAN...

use byte_field_arm_common::{validation_request, ArmResult, Case, ARM_RESULT_SCHEMA};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use tuning_campaign_support::protocol::{
    sha256_hex, CellDeclaration, FamilyAddendum, PlanCell, RunnerPlan, SharedSettings,
};
use tuning_campaign_support::transport::{self, FRESH_CASE_VALUE, FRESH_CASE_VAR};

/// What one untimed dispatch reported, as the record states it.
#[derive(Debug)]
struct Observation {
    cell_id: String,
    arm: String,
    cache_state: String,
    operation: String,
    workers: u32,
    selected_path: String,
}

/// Runs one arm of one cell once, untimed, and returns what it reported.
fn drive(
    plan: &RunnerPlan,
    cell: &PlanCell,
    declaration: &CellDeclaration,
    case: &Case,
    arm_name: &str,
    settings: &SharedSettings,
) -> Result<Observation, String> {
    let arm = plan
        .arms
        .get(arm_name)
        .ok_or_else(|| format!("the plan declares no arm {arm_name}"))?;
    let workers = declaration.workers.declared;
    // The runner resolves the child's CPUs from the host topology and pins
    // them; an untimed dispatch reserves no core, so the request carries the
    // declared width instead.
    let cpus = (0..workers).collect();
    let request = validation_request(declaration, arm_name, cell.case.clone(), cpus, settings);
    let encoded = transport::encode_case(&request)?;

    // The child environment of `benchmark-ab-runner::run_arm`: four forwarded
    // names, the arm's declared environment, then the fresh-child sentinel.
    let mut command = Command::new(&arm.executable);
    command.args(&arm.arguments);
    command.env_clear();
    for name in ["PATH", "HOME", "RAYON_NUM_THREADS", "RUSTUP_TOOLCHAIN"] {
        if let Ok(value) = std::env::var(name) {
            command.env(name, value);
        }
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
        .ok_or("the child has no stdin")?
        .write_all(encoded.as_bytes())
        .map_err(|error| error.to_string())?;
    let finished = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    let context = format!("{arm_name} on {}", cell.cell_id);
    if !finished.status.success() {
        return Err(format!(
            "{context} exited {}: {}",
            finished.status,
            String::from_utf8_lossy(&finished.stderr).trim()
        ));
    }
    let result: ArmResult = transport::parse_result(&String::from_utf8_lossy(&finished.stdout))?;
    accept(result, &context, declaration, case, arm_name)
}

/// Checks one result line against the declaration it answers.
///
/// A window here means the arm timed the dispatch, which belongs to a benchmark
/// window and not to a smoke, so the smoke refuses it rather than recording it.
fn accept(
    result: ArmResult,
    context: &str,
    declaration: &CellDeclaration,
    case: &Case,
    arm_name: &str,
) -> Result<Observation, String> {
    if result.schema != ARM_RESULT_SCHEMA {
        return Err(format!("{context} wrote schema {:?}", result.schema));
    }
    if !result.windows.is_empty() {
        return Err(format!(
            "{context} reported {} timing windows; this smoke records none",
            result.windows.len()
        ));
    }
    if result.calibrated {
        return Err(format!("{context} calibrated a call count without timing"));
    }
    if result.cache_state_applied != declaration.cache_state {
        return Err(format!(
            "{context} applied cache state {:?}, declared {:?}",
            result.cache_state_applied, declaration.cache_state
        ));
    }
    let workers = declaration.workers.declared;
    if result.workers_observed != workers {
        return Err(format!(
            "{context} observed {} workers, declared {workers}",
            result.workers_observed
        ));
    }
    let selected_path = result
        .selected_path
        .ok_or_else(|| format!("{context} named no selected path"))?;
    Ok(Observation {
        cell_id: declaration.cell_id.clone(),
        arm: arm_name.to_owned(),
        cache_state: format!("{:?}", declaration.cache_state).to_lowercase(),
        operation: format!("{:?}", case.operation).to_lowercase(),
        workers,
        selected_path,
    })
}

/// Drives every arm of every cell one plan declares.
fn smoke(path: &str, record: &mut Vec<String>) -> Result<Vec<Observation>, String> {
    let plan =
        RunnerPlan::decode(&std::fs::read(path).map_err(|error| format!("{path}: {error}"))?)?;
    let addendum_bytes =
        std::fs::read(&plan.addendum).map_err(|error| format!("{}: {error}", plan.addendum))?;
    let addendum = FamilyAddendum::decode(&addendum_bytes)?;
    addendum
        .validate()
        .map_err(|errors| format!("addendum invalid: {}", errors.join("; ")))?;
    plan.validate(&addendum)
        .map_err(|errors| format!("plan invalid: {}", errors.join("; ")))?;
    let (settings, _) = plan.settings();

    record.push(format!(
        "# plan: campaign {} over {} ({} cells, {} arms)",
        plan.campaign_id,
        plan.addendum,
        plan.cells.len(),
        plan.arms.len()
    ));
    // Named by file name, not by path: the plan carries a checkout-specific
    // absolute path and the digest is what identifies the measured build.
    let mut digests = BTreeMap::new();
    for arm in plan.arms.values() {
        let bytes = std::fs::read(&arm.executable)
            .map_err(|error| format!("{}: {error}", arm.executable))?;
        let name = Path::new(&arm.executable)
            .file_name()
            .ok_or_else(|| format!("{} names no executable", arm.executable))?
            .to_string_lossy()
            .into_owned();
        digests.insert(name, sha256_hex(&bytes));
    }
    for (executable, digest) in &digests {
        record.push(format!("# arm executable {executable} sha256: {digest}"));
    }

    let mut observations = Vec::new();
    for cell in &plan.cells {
        let declaration = addendum
            .cell(&cell.cell_id)
            .ok_or_else(|| format!("the addendum declares no cell {}", cell.cell_id))?;
        let case: Case = serde_json::from_value(cell.case.clone())
            .map_err(|error| format!("cell {} case does not decode: {error}", cell.cell_id))?;
        for arm_name in [&cell.baseline_arm, &cell.candidate_arm] {
            observations.push(drive(&plan, cell, declaration, &case, arm_name, &settings)?);
        }
    }
    Ok(observations)
}

fn run() -> Result<Vec<String>, String> {
    let plans: Vec<String> = std::env::args().skip(1).collect();
    if plans.is_empty() {
        return Err("usage: gf256-axpy-smoke PLAN...".to_owned());
    }
    let mut record = [
        "# GF(2^8) axpy arm smoke (jit:ad2a6a58)",
        "# command: dev/active/ad2a6a58/survey/smoke-arms.sh",
        "# every line below is observed at run time from the plan this smoke drove and the result",
        "# lines the arms wrote; none carries a clock reading or a timing window, so a rerun on the",
        "# same executables reproduces this record byte for byte",
    ]
    .map(str::to_owned)
    .to_vec();
    let mut observations = Vec::new();
    for path in &plans {
        observations.extend(smoke(path, &mut record)?);
    }
    for seen in &observations {
        record.push(format!(
            "PASS {}/{}: cache-state={} operation={} workers={} windows=0 {}",
            seen.cell_id,
            seen.arm,
            seen.cache_state,
            seen.operation,
            seen.workers,
            seen.selected_path
        ));
    }
    let set = |values: BTreeSet<&str>| values.into_iter().collect::<Vec<_>>().join(",");
    record.push(format!(
        "PASS coverage: arms {}",
        set(observations.iter().map(|seen| seen.arm.as_str()).collect())
    ));
    record.push(format!(
        "PASS coverage: operations {}",
        set(observations
            .iter()
            .map(|seen| seen.operation.as_str())
            .collect())
    ));
    record.push(format!(
        "PASS coverage: cache states {}",
        set(observations
            .iter()
            .map(|seen| seen.cache_state.as_str())
            .collect())
    ));
    record.push(format!(
        "PASS coverage: {} untimed dispatches, 0 timing windows",
        observations.len()
    ));
    Ok(record)
}

fn main() -> ExitCode {
    match run() {
        Ok(record) => {
            println!("{}", record.join("\n"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gf256-axpy-smoke: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byte_field_arm_common::{
        runner_request_fields, PairPosition, Request, Window, ARM_REQUEST_SCHEMA,
    };
    use serde_json::Value;
    use tuning_campaign_support::protocol::SHARED_SETTINGS;

    /// The frozen pilot family, so the mirror is pinned against a declaration
    /// the campaigns measure rather than a fixture written here.
    const PILOT_ADDENDUM: &str = include_str!("../../../../addendum-v4-axpy-pilot.json");

    /// The family's cold declaration, which is the one that carries frozen
    /// calls, so a request built from it exercises the runner's optional field.
    fn cold_declaration() -> CellDeclaration {
        FamilyAddendum::decode(PILOT_ADDENDUM.as_bytes())
            .expect("the frozen addendum decodes")
            .cells
            .into_iter()
            .find(|cell| cell.cold_calls.is_some())
            .expect("the family declares a cold cell")
    }

    /// The case `make-plan.py` projects for one declaration.
    fn case_for(declaration: &CellDeclaration) -> Value {
        serde_json::json!({
            "operation": "axpy",
            "bytes": declaration.workload.size["bytes"],
            "n": 0,
            "k": 0,
            "rows": 0,
            "poly": 285,
            "metric": "kernel-isolated",
            "seed": declaration.workload.seed,
            "workers": declaration.workers.declared,
        })
    }

    /// Runner request fields no cell of this family carries, and why.
    const ABSENT_FIELDS: [(&str, &str); 1] = [(
        "decoder",
        "no cell declares a decoder, and the arm rejects a request that carries one",
    )];

    /// Keys of one compact canonical JSON object, in the order it encodes them.
    ///
    /// Serialization order is part of the wire contract, and
    /// `serde_json::Value` would sort the keys away, so the scan reads them off
    /// the encoded bytes.
    fn top_level_keys(encoded: &str) -> Vec<String> {
        let bytes = encoded.as_bytes();
        let mut keys = Vec::new();
        let mut depth = 0usize;
        let mut index = 0usize;
        while index < bytes.len() {
            match bytes[index] {
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                b'"' => {
                    let start = index + 1;
                    let mut end = start;
                    while bytes[end] != b'"' {
                        end += if bytes[end] == b'\\' { 2 } else { 1 };
                    }
                    if depth == 1 && bytes.get(end + 1) == Some(&b':') {
                        keys.push(encoded[start..end].to_owned());
                    }
                    index = end;
                }
                _ => {}
            }
            index += 1;
        }
        keys
    }

    /// The request this smoke sends is the runner's request: it carries the
    /// runner's declared fields in the runner's declaration order, the arm's
    /// own guarded decoder accepts it, and its re-encoding is byte-identical,
    /// so a field added, dropped or reordered on either side fails here rather
    /// than inside a benchmark window.
    #[test]
    fn the_request_mirror_is_the_runners_request() {
        let declaration = cold_declaration();
        let request = validation_request(
            &declaration,
            "scalar-element",
            case_for(&declaration),
            vec![0],
            &SHARED_SETTINGS,
        );
        let encoded = transport::encode_case(&request).expect("the runner's encoder");

        let expected: Vec<&str> = runner_request_fields()
            .into_iter()
            .filter(|field| !ABSENT_FIELDS.iter().any(|(absent, _)| absent == field))
            .collect();
        assert_eq!(top_level_keys(&encoded), expected);
        for (field, reason) in ABSENT_FIELDS {
            assert!(
                runner_request_fields().contains(&field),
                "{field} is no longer a runner request field ({reason})"
            );
        }

        let decoded: Request =
            transport::read_guarded_case(Some(FRESH_CASE_VALUE), encoded.as_bytes())
                .expect("the arm's decoder");
        assert_eq!(
            transport::encode_case(&decoded).expect("re-encodes"),
            encoded
        );
        assert_eq!(decoded.schema, ARM_REQUEST_SCHEMA);
        assert_eq!(decoded.role, PairPosition::Validation);
        assert_eq!(decoded.cache_state, declaration.cache_state);
        assert_eq!(decoded.cold_calls, declaration.cold_calls);
        assert_eq!(decoded.windows, SHARED_SETTINGS.windows_per_execution);
        assert_eq!(decoded.window_target_ms, SHARED_SETTINGS.window_target_ms);
        assert_eq!(decoded.workers_declared, declaration.workers.declared);
    }

    /// A result carrying a timing window is refused: an arm that timed its
    /// dispatch was driven in a measuring position, and a smoke records none.
    #[test]
    fn a_reported_timing_window_fails_the_smoke() {
        let declaration = cold_declaration();
        let case: Case = serde_json::from_value(case_for(&declaration)).expect("the case decodes");
        let untimed = ArmResult {
            schema: ARM_RESULT_SCHEMA.to_owned(),
            windows: Vec::new(),
            cache_state_applied: declaration.cache_state,
            workers_observed: declaration.workers.declared,
            cpus_observed: vec![0],
            selected_path: Some("lane=gf256-product-table/table-builds=1".to_owned()),
            conversion: None,
            quality: None,
            calibrated: false,
        };
        let seen = accept(
            untimed.clone(),
            "fixture",
            &declaration,
            &case,
            "table-element",
        )
        .expect("an untimed result is accepted");
        assert_eq!(seen.cell_id, declaration.cell_id);

        let timed = ArmResult {
            windows: vec![Window {
                calls: 1,
                elapsed_ns: 1,
            }],
            ..untimed
        };
        let error = accept(timed, "fixture", &declaration, &case, "table-element")
            .expect_err("a timed result is refused");
        assert!(error.contains("timing windows"), "{error}");
    }
}
