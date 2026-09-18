//! Non-timed wire-contract smoke of a protocol-v4 plan's arms (jit:9fb40c83,
//! jit:85fc5ff4).
//!
//! Drives every arm of every cell of a saved runner plan the way
//! `benchmark-ab-runner` drives it — the runner's own request encoder, result
//! parser and child environment — in the [`VALIDATION_ROLE`], so each arm
//! performs one untimed dispatch and returns no timing window. Reading an arm's
//! source does not establish the wire contract between the runner and a child;
//! running it does.
//!
//! The smoke opens no campaign: it takes no lock, reserves nothing in a family
//! ledger, writes no stage and finalizes no receipt. It refuses an arm that
//! reports a timing window, so no invocation of it can produce a timing sample.
//! Its output is the observation document a story's record generator renders.
//!
//! Usage: arm-smoke --plan <plan.json> --output <observations.json>

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use tuning_campaign_support::protocol::{
    sha256_hex, CacheState, CellDeclaration, DecoderCell, FamilyAddendum, RunnerPlan,
    SharedSettings,
};
use tuning_campaign_support::receipt::{ArmQuality, ConversionCosts, WindowRecord};
use tuning_campaign_support::transport::{self, FRESH_CASE_VALUE, FRESH_CASE_VAR};

/// Request role that asks an arm for one untimed dispatch and no window.
const VALIDATION_ROLE: &str = "validation";
const ARM_REQUEST_SCHEMA: &str = "zen3-benchmark-arm-request-v1";
const ARM_RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";
const OBSERVATION_SCHEMA: &str = "zen3-arm-smoke-observations-v1";
/// Variables the runner installs from its own environment after `env_clear`.
const INHERITED: [&str; 4] = ["PATH", "HOME", "RAYON_NUM_THREADS", "RUSTUP_TOOLCHAIN"];

/// The request `benchmark-ab-runner` writes to an arm child, field for field.
///
/// `case` stays a [`Value`] because the runner forwards the plan cell's case
/// verbatim, and the canonical framing accepts a request only when it
/// re-encodes byte for byte.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArmRequest {
    schema: String,
    cell_id: String,
    arm: String,
    role: String,
    pair: u32,
    case: Value,
    cache_state: CacheState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cold_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decoder: Option<DecoderCell>,
    windows: u32,
    window_target_ms: u32,
    cpus: Vec<u32>,
    workers_declared: u32,
}

/// The one result line an arm child writes, field for field as the runner
/// parses it. `calibrated` is omitted by an arm that does not calibrate.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArmResult {
    schema: String,
    windows: Vec<WindowRecord>,
    cache_state_applied: CacheState,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<ConversionCosts>,
    quality: Option<ArmQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calibrated: Option<bool>,
}

/// Structural facts this run observed, and nothing else: no clock reading, so a
/// rerun on the same executables reproduces the document byte for byte.
#[derive(Serialize)]
struct Observations {
    schema: &'static str,
    plan: PlanIdentity,
    arms: Vec<ArmIdentity>,
    dispatches: Vec<Dispatch>,
}

#[derive(Serialize)]
struct PlanIdentity {
    path: String,
    sha256: String,
    campaign_id: String,
    issue: String,
    addendum: String,
    addendum_sha256: String,
    cells: usize,
}

#[derive(Serialize)]
struct ArmIdentity {
    arm: String,
    executable: String,
    sha256: String,
    environment: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct Dispatch {
    cell_id: String,
    arm: String,
    role: &'static str,
    cache_state_declared: CacheState,
    cache_state_applied: CacheState,
    workers_declared: u32,
    workers_observed: u32,
    result_lines: u32,
    timing_windows: usize,
    selected_path: String,
    conversion_reported: bool,
    quality_reported: bool,
}

/// The request the runner would send for this cell and arm, in the validation
/// role at pair zero.
fn validation_request(
    cell_id: &str,
    arm: &str,
    case: Value,
    declared: &CellDeclaration,
    settings: &SharedSettings,
) -> ArmRequest {
    let workers = declared.workers.declared;
    ArmRequest {
        schema: ARM_REQUEST_SCHEMA.to_owned(),
        cell_id: cell_id.to_owned(),
        arm: arm.to_owned(),
        role: VALIDATION_ROLE.to_owned(),
        pair: 0,
        case,
        cache_state: declared.cache_state,
        cold_calls: declared.cold_calls,
        decoder: declared.decoder.clone(),
        windows: settings.windows_per_execution,
        window_target_ms: settings.window_target_ms,
        cpus: (0..workers).collect(),
        workers_declared: workers,
    }
}

/// Path as the repository sees it, so a record carries no checkout location.
fn repository_relative(path: &str) -> Result<String, String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    let resolved = std::fs::canonicalize(path).map_err(|error| format!("{path}: {error}"))?;
    Ok(resolved
        .strip_prefix(&root)
        .unwrap_or(&resolved)
        .to_string_lossy()
        .into_owned())
}

fn digest(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    Ok(sha256_hex(&bytes))
}

/// Runs one arm of one cell and returns the dispatch it observed.
fn drive(
    plan: &RunnerPlan,
    declared: &CellDeclaration,
    cell_case: &Value,
    arm_name: &str,
    settings: &SharedSettings,
) -> Result<Dispatch, String> {
    let arm = plan
        .arms
        .get(arm_name)
        .ok_or_else(|| format!("the plan declares no arm {arm_name}"))?;
    let request = validation_request(
        &declared.cell_id,
        arm_name,
        cell_case.clone(),
        declared,
        settings,
    );
    let encoded = transport::encode_case(&request)?;

    let mut command = Command::new(&arm.executable);
    command.args(&arm.arguments);
    command.env_clear();
    for name in INHERITED {
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
    let context = format!("{arm_name} on {}", declared.cell_id);
    if !finished.status.success() {
        return Err(format!(
            "{context} exited {}: {}",
            finished.status,
            String::from_utf8_lossy(&finished.stderr).trim()
        ));
    }
    if !finished.stderr.is_empty() {
        return Err(format!(
            "{context} wrote a diagnostic: {}",
            String::from_utf8_lossy(&finished.stderr).trim()
        ));
    }
    let result: ArmResult = transport::parse_result(&String::from_utf8_lossy(&finished.stdout))?;
    if result.schema != ARM_RESULT_SCHEMA {
        return Err(format!("{context} wrote result schema {:?}", result.schema));
    }
    if !result.windows.is_empty() {
        return Err(format!(
            "{context} reported {} timing windows; a smoke records none",
            result.windows.len()
        ));
    }
    if result.workers_observed != request.workers_declared {
        return Err(format!(
            "{context} observed {} workers against {} declared",
            result.workers_observed, request.workers_declared
        ));
    }
    if result.cache_state_applied != declared.cache_state {
        return Err(format!(
            "{context} applied cache state {:?} against {:?} declared",
            result.cache_state_applied, declared.cache_state
        ));
    }
    let selected_path = result
        .selected_path
        .ok_or_else(|| format!("{context} reported no selected path"))?;
    Ok(Dispatch {
        cell_id: declared.cell_id.clone(),
        arm: arm_name.to_owned(),
        role: VALIDATION_ROLE,
        cache_state_declared: declared.cache_state,
        cache_state_applied: result.cache_state_applied,
        workers_declared: request.workers_declared,
        workers_observed: result.workers_observed,
        result_lines: 1,
        timing_windows: result.windows.len(),
        selected_path,
        conversion_reported: result.conversion.is_some(),
        quality_reported: result.quality.is_some(),
    })
}

fn smoke(plan_path: &str) -> Result<Observations, String> {
    let plan_bytes = std::fs::read(plan_path).map_err(|error| format!("{plan_path}: {error}"))?;
    let plan = RunnerPlan::decode(&plan_bytes)?;
    let addendum_bytes =
        std::fs::read(&plan.addendum).map_err(|error| format!("{}: {error}", plan.addendum))?;
    let addendum = FamilyAddendum::decode(&addendum_bytes)?;
    addendum
        .validate()
        .map_err(|errors| format!("the addendum is invalid: {}", errors.join("; ")))?;
    plan.validate(&addendum)
        .map_err(|errors| format!("the plan is invalid: {}", errors.join("; ")))?;
    let (settings, overridden) = plan.settings();
    if overridden {
        return Err("the plan overrides the frozen timing settings".to_owned());
    }

    let mut arms = Vec::new();
    for (name, arm) in &plan.arms {
        arms.push(ArmIdentity {
            arm: name.clone(),
            executable: repository_relative(&arm.executable)?,
            sha256: digest(&arm.executable)?,
            environment: arm.environment.clone(),
        });
    }
    let mut dispatches = Vec::new();
    for cell in &plan.cells {
        let declared = addendum
            .cell(&cell.cell_id)
            .ok_or_else(|| format!("the addendum declares no cell {}", cell.cell_id))?;
        for arm_name in [&cell.baseline_arm, &cell.candidate_arm] {
            dispatches.push(drive(&plan, declared, &cell.case, arm_name, &settings)?);
        }
    }
    Ok(Observations {
        schema: OBSERVATION_SCHEMA,
        plan: PlanIdentity {
            path: repository_relative(plan_path)?,
            sha256: sha256_hex(&plan_bytes),
            campaign_id: plan.campaign_id.clone(),
            issue: plan.issue.clone(),
            addendum: plan.addendum.clone(),
            addendum_sha256: sha256_hex(&addendum_bytes),
            cells: plan.cells.len(),
        },
        arms,
        dispatches,
    })
}

fn run() -> Result<(), String> {
    let mut plan = None;
    let mut output = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--plan" => plan = arguments.next(),
            "--output" => output = arguments.next(),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let (plan, output) = match (plan, output) {
        (Some(plan), Some(output)) => (plan, output),
        _ => return Err("usage: arm-smoke --plan <plan.json> --output <path>".to_owned()),
    };
    let observations = smoke(&plan)?;
    let encoded = serde_json::to_string_pretty(&observations).map_err(|error| error.to_string())?;
    std::fs::write(Path::new(&output), format!("{encoded}\n"))
        .map_err(|error| format!("{output}: {error}"))?;
    println!(
        "{} dispatches over {} cells and {} arms, {} timing windows -> {output}",
        observations.dispatches.len(),
        observations.plan.cells,
        observations.arms.len(),
        observations
            .dispatches
            .iter()
            .map(|dispatch| dispatch.timing_windows)
            .sum::<usize>(),
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("arm-smoke: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuning_campaign_support::host::CoreArm;
    use tuning_campaign_support::protocol::{
        ArmBuilds, BuildIdentity, CellObjective, CellRole, MetricKind, Scaling, WorkerDeclaration,
        Workload, SHARED_SETTINGS,
    };

    fn declared() -> CellDeclaration {
        CellDeclaration {
            cell_id: "left-small-r1-w64".to_owned(),
            objective: CellObjective::ComparatorGap,
            role: CellRole::Exploratory,
            workload: Workload {
                identity: "bitvec-left-small".to_owned(),
                size: BTreeMap::from([("length_bits".to_owned(), 65)]),
                seed: 8501,
            },
            metric_kind: MetricKind::KernelIsolated,
            scaling: Scaling::SingleCoreLatency,
            core_arm: CoreArm::SingleCore,
            workers: WorkerDeclaration {
                declared: 1,
                nested_pools_allowed: false,
            },
            cache_state: CacheState::Warm,
            cold_calls: None,
            builds: ArmBuilds {
                baseline: BuildIdentity::ConservativePortable,
                candidate: BuildIdentity::ConservativePortable,
            },
            conversion_costs_included: false,
            decoder: None,
        }
    }

    /// The request mirror is the runner's request on the wire: the runner's own
    /// encoder accepts it, the canonical decoder round-trips it byte for byte
    /// with unknown fields rejected, and a cell that declares neither cold calls
    /// nor a decoder omits both fields rather than spelling them `null`, which
    /// the canonical decoder rejects. A field added, dropped, renamed or
    /// reordered on either side fails here.
    #[test]
    fn the_request_mirror_is_the_runner_request_on_the_wire() {
        let request = validation_request(
            "left-small-r1-w64",
            "residual-production",
            serde_json::json!({"direction": "left", "length_bits": 65, "seed": 8501}),
            &declared(),
            &SHARED_SETTINGS,
        );
        let encoded = transport::encode_case(&request).expect("the runner's encoder");
        let decoded: ArmRequest = transport::decode_case(&encoded).expect("the canonical decoder");
        assert_eq!(
            transport::encode_case(&decoded).expect("the runner's encoder"),
            encoded
        );
        assert_eq!(decoded.schema, ARM_REQUEST_SCHEMA);
        assert_eq!(decoded.role, VALIDATION_ROLE);
        assert_eq!(decoded.windows, SHARED_SETTINGS.windows_per_execution);
        assert_eq!(decoded.window_target_ms, SHARED_SETTINGS.window_target_ms);
        assert!(!encoded.contains("cold_calls"));
        assert!(!encoded.contains("decoder"));
    }

    /// The result mirror is the line the runner parses: a zero-window result
    /// decodes canonically whether the arm reports `calibrated` or omits it, so
    /// both of this story's arms answer the same parser the runner uses.
    #[test]
    fn the_result_mirror_parses_a_zero_window_result_line() {
        for calibrated in ["", ",\"calibrated\":true"] {
            let line = format!(
                "{}{{\"schema\":\"{ARM_RESULT_SCHEMA}\",\"windows\":[],\
                 \"cache_state_applied\":\"warm\",\"workers_observed\":1,\
                 \"cpus_observed\":[0],\"selected_path\":\"route\",\"conversion\":null,\
                 \"quality\":null{calibrated}}}",
                transport::FRESH_RESULT_PREFIX
            );
            let result: ArmResult = transport::parse_result(&line).expect("the runner's parser");
            assert!(result.windows.is_empty());
            assert_eq!(result.cache_state_applied, CacheState::Warm);
        }
    }
}
