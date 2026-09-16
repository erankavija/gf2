//! Deterministic non-timed smoke of the campaign wire, journal and checkpoints.
//!
//! The smoke drives the same arm executables a timed campaign drives, over the
//! canonical child-v2 framing, from the same projected runner plan and campaign
//! addendum. Its requests declare zero timing windows, so each arm builds its
//! fixture, resolves its route, performs the untimed arrangement its cache
//! policy declares and reports no timing window: the smoke collects no timing
//! sample and finalizes no receipt.
//!
//! Everything else is the shared campaign machinery: the append-only execution
//! log, the immutable checkpoint store keyed by cell, the resume identity that
//! pins the plan, the addendum, the producing-input closure and both arm
//! executables, and the plan's own per-session cell budget. A session that
//! exhausts the budget pauses; the next session resumes from the checkpoints
//! and repeats no completed cell.

use crate::wire::{ArmResult, Request, RESULT_SCHEMA};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tuning_campaign_support::campaign::ProcessOutcome;
use tuning_campaign_support::host::HostObservation;
use tuning_campaign_support::journal::{
    CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity, TerminalState,
};
use tuning_campaign_support::process::run_process;
use tuning_campaign_support::protocol::{
    sha256_hex, CellDeclaration, FamilyAddendum, PlanCell, RunnerPlan, PROTOCOL_PATH,
};
use tuning_campaign_support::provenance::ProducingInputs;
use tuning_campaign_support::transport::{self, FRESH_CASE_VALUE, FRESH_CASE_VAR};

/// Schema identity of the smoke's own output record.
pub const SMOKE_SCHEMA: &str = "logical-buffer-nontimed-smoke-v1";
/// Lifecycle schema the smoke's resume identity declares.
pub const LIFECYCLE_SCHEMA: &str = "logical-buffer-nontimed-smoke-lifecycle-v1";
/// File the completing session writes beside the execution log.
pub const HANDSHAKE_FILE: &str = "handshake.json";
/// Name of the staged copy of the plan the smoke drives.
pub const PLAN_FILE: &str = "plan.json";
/// Directory holding the smoke's immutable per-cell checkpoints.
pub const CHECKPOINT_DIR: &str = "checkpoints";

const CHILD_TIMEOUT: Duration = Duration::from_secs(300);
const CHILD_KILL_GRACE: Duration = Duration::from_secs(5);
static ALL_REAPED: AtomicBool = AtomicBool::new(true);

/// What one arm reported for one cell of the non-timed smoke.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmHandshake {
    /// Arm name the plan gave this child.
    pub arm: String,
    /// `baseline` or `candidate`.
    pub role: String,
    /// Content identity of the executable the plan named.
    pub executable_sha256: String,
    /// Cache state the arm reports applying.
    pub cache_state_applied: String,
    /// Route provenance the arm observed at run time.
    pub selected_path: String,
    /// Timing windows the arm reported; zero in every non-timed smoke.
    pub windows: usize,
}

/// What both arms reported for one cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellHandshake {
    /// Frozen cell identifier.
    pub cell_id: String,
    /// Cache state the campaign addendum declares for it.
    pub cache_state: String,
    /// Baseline then candidate, in the order the smoke drove them.
    pub arms: Vec<ArmHandshake>,
}

/// The completed smoke of one family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmokeRecord {
    /// Output schema identity.
    pub schema: String,
    /// Campaign the smoke drove.
    pub campaign_id: String,
    /// Family the campaign addendum declares.
    pub family: String,
    /// Every declared cell, in the plan's order.
    pub cells: Vec<CellHandshake>,
}

/// One session's outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionOutcome {
    /// Terminal state the session journaled.
    pub state: TerminalState,
    /// Cells this session completed.
    pub completed: usize,
}

impl SessionOutcome {
    /// Process exit code: zero when complete, three when paused, mirroring the
    /// campaign runner's resumable-pause convention.
    pub fn exit_code(self) -> i32 {
        if self.state == TerminalState::Complete {
            0
        } else {
            3
        }
    }
}

fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// Runs one non-timed smoke session over `plan` at `stage`.
///
/// `root` is the repository root every repository-relative path resolves
/// against. The first session creates the stage's execution log and checkpoint
/// store; a later session resumes them and skips every completed cell. The
/// session that completes the plan writes [`HANDSHAKE_FILE`] beside the log.
pub fn session(
    root: &Path,
    stage: &Path,
    plan: &RunnerPlan,
    plan_bytes: &[u8],
    addendum: &FamilyAddendum,
    addendum_bytes: &[u8],
) -> io::Result<SessionOutcome> {
    plan.validate(addendum)
        .map_err(|errors| invalid(format!("the plan is invalid: {}", errors.join("; "))))?;
    fs::create_dir_all(stage)?;
    let stage = fs::canonicalize(stage)?;
    let staged_plan = stage.join(PLAN_FILE);
    if staged_plan.exists() {
        if fs::read(&staged_plan)? != plan_bytes {
            return Err(invalid(
                "stage holds a different plan; a resume needs the identical plan",
            ));
        }
    } else {
        tuning_campaign_support::journal::atomic_write_new(&staged_plan, plan_bytes)?;
    }

    let host = HostObservation::observe()?;
    let identity = identity(root, plan, plan_bytes, addendum_bytes, &host)?;
    let log_path = stage.join("execution.log");
    let session_id = format!("smoke-{}", std::process::id());
    let mut log = if log_path.exists() {
        ExecutionLog::resume(&log_path, plan.campaign_id.clone(), session_id)?
    } else {
        ExecutionLog::create_new(&stage, plan.campaign_id.clone(), session_id)?
    };
    log.announce(io::stdout().lock())?;
    let facts = json!({
        "kind": "non-timed-smoke",
        "schema": SMOKE_SCHEMA,
        "family": addendum.family.id,
        "identity": identity,
        "host_observed_utc": host.observed_utc,
    });
    if log.next_sequence() == 0 {
        log.append(JournalEvent::CampaignStart, None, facts)?;
    } else {
        let records = ExecutionLog::validate_prefix(
            &log.validated_synced_prefix()?,
            &plan.campaign_id,
        )?;
        let recorded: ResumeIdentity = serde_json::from_value(records[0].details["identity"].clone())
            .map_err(|error| invalid(format!("campaign-start identity does not decode: {error}")))?;
        if !recorded.resume_equivalent(&identity) {
            return Err(invalid(
                "resume identity differs from the campaign-start facts",
            ));
        }
    }
    log.append(
        JournalEvent::OrchestrationStart,
        None,
        json!({"kind": "execution-log-announced", "path": log.path()}),
    )?;

    let checkpoint_root = stage.join(CHECKPOINT_DIR);
    let mut checkpoints = if checkpoint_root.join("manifest.json").exists() {
        CheckpointStore::resume(&checkpoint_root, plan.campaign_id.clone(), identity)?
    } else {
        CheckpointStore::create_new(&checkpoint_root, plan.campaign_id.clone(), identity)?
    };
    if let Some(recovery) = checkpoints.pending_recovery().cloned() {
        log.append(
            JournalEvent::PendingRecovery,
            None,
            serde_json::to_value(&recovery)?,
        )?;
        checkpoints.acknowledge_pending_recovery(&recovery.recovery_id)?;
    }

    let mut completed = 0usize;
    let mut state = TerminalState::Complete;
    for cell in &plan.cells {
        if checkpoints.completed_unit(&cell.cell_id).is_some() {
            log.append(
                JournalEvent::Omission,
                Some(json!({"key": cell.cell_id})),
                json!({"kind": "completed-in-prior-session"}),
            )?;
            continue;
        }
        if plan
            .max_cells_per_session
            .is_some_and(|limit| completed as u32 >= limit)
        {
            state = TerminalState::Paused;
            break;
        }
        let declared = addendum
            .cell(&cell.cell_id)
            .ok_or_else(|| invalid("undeclared cell"))?;
        handshake_cell(&mut log, &mut checkpoints, root, plan, cell, declared)?;
        completed += 1;
    }

    if state == TerminalState::Complete {
        let record = SmokeRecord {
            schema: SMOKE_SCHEMA.to_owned(),
            campaign_id: plan.campaign_id.clone(),
            family: addendum.family.id.clone(),
            cells: plan
                .cells
                .iter()
                .map(|cell| {
                    checkpoints
                        .load::<Value, CellHandshake>(&cell.cell_id)
                        .map(|(_, handshake)| handshake)
                })
                .collect::<io::Result<Vec<_>>>()?,
        };
        let mut bytes = serde_json::to_vec_pretty(&record)?;
        bytes.push(b'\n');
        let path = stage.join(HANDSHAKE_FILE);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        tuning_campaign_support::journal::atomic_write_new(&path, &bytes)?;
        log.append(
            JournalEvent::OrchestrationExit,
            None,
            json!({"kind": "handshake-record", "path": path, "sha256": sha256_hex(&bytes)}),
        )?;
    }
    log.terminal(state, json!({"handshake_cells": completed}))?;
    Ok(SessionOutcome { state, completed })
}

fn handshake_cell(
    log: &mut ExecutionLog,
    checkpoints: &mut CheckpointStore,
    root: &Path,
    plan: &RunnerPlan,
    cell: &PlanCell,
    declared: &CellDeclaration,
) -> io::Result<()> {
    let case = json!({"key": cell.cell_id, "cell_id": cell.cell_id, "case": cell.case});
    log.append(
        JournalEvent::CellStart,
        Some(case.clone()),
        json!({"role": declared.role, "core_arm": declared.core_arm}),
    )?;
    let cache_state = serde_json::to_value(declared.cache_state)?
        .as_str()
        .ok_or_else(|| invalid("cache state is not a string"))?
        .to_owned();
    let mut arms = Vec::with_capacity(2);
    for (role, name) in [
        ("baseline", &cell.baseline_arm),
        ("candidate", &cell.candidate_arm),
    ] {
        arms.push(handshake_arm(
            log,
            root,
            plan,
            cell,
            declared,
            &cache_state,
            role,
            name,
        )?);
    }
    let handshake = CellHandshake {
        cell_id: cell.cell_id.clone(),
        cache_state,
        arms,
    };
    let accepted = checkpoints.accept(&cell.cell_id, &case, &handshake)?;
    log.append(
        JournalEvent::CheckpointAccepted,
        Some(case.clone()),
        json!({"sha256": accepted.sha256, "path": accepted.path}),
    )?;
    log.append(
        JournalEvent::CellComplete,
        Some(case),
        json!({"status": "handshake", "windows": 0, "pairs": 0}),
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handshake_arm(
    log: &mut ExecutionLog,
    root: &Path,
    plan: &RunnerPlan,
    cell: &PlanCell,
    declared: &CellDeclaration,
    cache_state: &str,
    role: &str,
    name: &str,
) -> io::Result<ArmHandshake> {
    let arm = plan
        .arms
        .get(name)
        .ok_or_else(|| invalid(format!("the plan declares no arm {name:?}")))?;
    let executable = if Path::new(&arm.executable).is_absolute() {
        Path::new(&arm.executable).to_path_buf()
    } else {
        root.join(&arm.executable)
    };
    let executable_sha256 = sha256_hex(&fs::read(&executable)?);
    // Zero windows and a zero window target: the arm arranges its fixture and
    // its route and returns without entering the timing protocol.
    let request = Request {
        schema: "zen3-benchmark-arm-request-v1".to_owned(),
        cell_id: cell.cell_id.clone(),
        arm: name.to_owned(),
        role: role.to_owned(),
        pair: 0,
        case: cell.case.clone(),
        cache_state: cache_state.to_owned(),
        cold_calls: declared.cold_calls,
        decoder: None,
        windows: 0,
        window_target_ms: 0,
        cpus: Vec::new(),
        workers_declared: declared.workers.declared,
    };
    let input = transport::encode_case(&request).map_err(invalid)?;
    let mut command = Command::new(&executable);
    command.args(&arm.arguments);
    command.env_clear();
    for (variable, value) in ["PATH", "HOME", "RAYON_NUM_THREADS", "RUSTUP_TOOLCHAIN"]
        .iter()
        .filter_map(|variable| std::env::var(variable).ok().map(|value| (*variable, value)))
    {
        command.env(variable, value);
    }
    for (variable, value) in &arm.environment {
        command.env(variable, value);
    }
    command.env(FRESH_CASE_VAR, FRESH_CASE_VALUE);

    let journal_case =
        json!({"key": cell.cell_id, "cell_id": cell.cell_id, "arm": name, "role": role});
    let mut stderr = Vec::new();
    let mut spawned = 0u32;
    let result = {
        let log = &mut *log;
        run_process(
            command,
            input.as_bytes(),
            CHILD_TIMEOUT,
            CHILD_KILL_GRACE,
            || Ok(()),
            &ALL_REAPED,
            |pid| {
                spawned = pid;
                log.append(
                    JournalEvent::ChildSpawn,
                    Some(journal_case.clone()),
                    json!({"pid": pid}),
                )
                .map(|_| ())
            },
            |chunk| {
                stderr.extend_from_slice(chunk);
                Ok(())
            },
        )?
    };
    if !stderr.is_empty() {
        log.append(
            JournalEvent::ChildDiagnostic,
            Some(journal_case.clone()),
            json!({"stderr": String::from_utf8_lossy(&stderr)}),
        )?;
    }
    log.append(
        JournalEvent::ChildExit,
        Some(journal_case.clone()),
        json!({"outcome": result.outcome}),
    )?;
    if let Some(error) = result.callback_error {
        return Err(error);
    }
    match result.outcome {
        ProcessOutcome::Exited { exit_code: 0, .. } => {}
        other => return Err(invalid(format!("arm {name} did not exit cleanly: {other:?}"))),
    }
    let text = String::from_utf8(result.stdout).map_err(|_| invalid("arm stdout is not UTF-8"))?;
    let parsed: ArmResult = transport::parse_result(&text).map_err(invalid)?;
    if parsed.schema != RESULT_SCHEMA {
        return Err(invalid(format!("arm result schema {:?}", parsed.schema)));
    }
    if !parsed.windows.is_empty() {
        return Err(invalid(format!(
            "arm {name} returned {} timing windows for a zero-window request",
            parsed.windows.len()
        )));
    }
    if parsed.cache_state_applied != cache_state {
        return Err(invalid(format!(
            "arm {name} applied cache state {:?} rather than the declared {cache_state:?}",
            parsed.cache_state_applied
        )));
    }
    let selected_path = parsed
        .selected_path
        .ok_or_else(|| invalid(format!("arm {name} reported no route provenance")))?;
    log.append(
        JournalEvent::DriverDiagnostic,
        Some(journal_case),
        json!({"kind": "arm-handshake", "windows": 0, "selected_path": selected_path}),
    )?;
    Ok(ArmHandshake {
        arm: name.to_owned(),
        role: role.to_owned(),
        executable_sha256,
        cache_state_applied: parsed.cache_state_applied,
        selected_path,
        windows: 0,
    })
}

/// Resume identity pinning everything whose bytes can change what the smoke
/// observes: the protocol document, the producing-input closure, the ordered
/// cell list, the arm descriptors and both arm executables.
fn identity(
    root: &Path,
    plan: &RunnerPlan,
    plan_bytes: &[u8],
    addendum_bytes: &[u8],
    host: &HostObservation,
) -> io::Result<ResumeIdentity> {
    let snapshot = ProducingInputs::capture(root, plan.producing_manifest_path())?;
    let ordered: Vec<&str> = plan.cells.iter().map(|cell| cell.cell_id.as_str()).collect();
    let mut executables = BTreeMap::new();
    for (name, arm) in &plan.arms {
        let path = if Path::new(&arm.executable).is_absolute() {
            Path::new(&arm.executable).to_path_buf()
        } else {
            root.join(&arm.executable)
        };
        executables.insert(name.clone(), sha256_hex(&fs::read(path)?));
    }
    Ok(ResumeIdentity {
        protocol_digest: sha256_hex(&fs::read(root.join(PROTOCOL_PATH))?),
        source_revision: String::new(),
        source_sha256: snapshot.identity_sha256()?,
        ordered_work_manifest_sha256: sha256_hex(&serde_json::to_vec(&ordered)?),
        process_descriptors_sha256: sha256_hex(&serde_json::to_vec(&plan.arms)?),
        executable_sha256: executables,
        behavior_sha256: BTreeMap::from([
            ("campaign-addendum".to_owned(), sha256_hex(addendum_bytes)),
            ("runner-plan".to_owned(), sha256_hex(plan_bytes)),
        ]),
        lifecycle_schema: LIFECYCLE_SCHEMA.to_owned(),
        lifecycle_behavior_sha256: snapshot.lifecycle_sha256()?,
        feature_contract: "logical-buffer-harness with gf2-core/simd".to_owned(),
        thread_contract: "one declared worker, no nested pools".to_owned(),
        host_identity: host.affinity.host_identity()?,
    })
}
