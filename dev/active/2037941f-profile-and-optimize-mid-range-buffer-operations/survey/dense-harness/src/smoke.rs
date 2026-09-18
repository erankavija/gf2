//! Deterministic non-timed smoke of the dense-parity arms, journal and
//! checkpoints.
//!
//! Every arm dispatch is the shared `tuning_campaign_support::arm::smoke`
//! contract, driven through [`validate_arm`].
//!
//! Around those dispatches this smoke adds the campaign machinery a timed
//! session uses: the append-only execution log, the immutable checkpoint store
//! keyed by cell, the resume identity that pins the plan, the addendum, the
//! producing-input closure and every arm executable, and the plan's own
//! per-session cell budget. A session that exhausts the budget pauses; the next
//! session resumes from the checkpoints and repeats no completed cell.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;
use tuning_campaign_support::arm::{validate_arm, CellValidation, JournalObserver};
use tuning_campaign_support::host::HostObservation;
use tuning_campaign_support::journal::{
    CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity, TerminalState,
};
use tuning_campaign_support::protocol::{
    sha256_hex, CellDeclaration, FamilyAddendum, PlanCell, RunnerPlan, PROTOCOL_PATH,
    SHARED_SETTINGS,
};
use tuning_campaign_support::provenance::ProducingInputs;

/// Schema identity of the smoke's own output record.
pub const SMOKE_SCHEMA: &str = "dense-parity-nontimed-smoke-v1";
/// Lifecycle schema the smoke's resume identity declares.
pub const LIFECYCLE_SCHEMA: &str = "dense-parity-nontimed-smoke-lifecycle-v1";
/// File the completing session writes beside the execution log.
pub const HANDSHAKE_FILE: &str = "handshake.json";
/// Name of the staged copy of the plan the smoke drives.
pub const PLAN_FILE: &str = "plan.json";
/// Directory holding the smoke's immutable per-cell checkpoints.
pub const CHECKPOINT_DIR: &str = "checkpoints";

/// The addendum's child timeout, which is the protocol's own shared setting.
pub const CHILD_TIMEOUT: Duration = Duration::from_secs(SHARED_SETTINGS.child_timeout_seconds);

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
    pub cells: Vec<CellValidation>,
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
        let records =
            ExecutionLog::validate_prefix(&log.validated_synced_prefix()?, &plan.campaign_id)?;
        let recorded: ResumeIdentity =
            serde_json::from_value(records[0].details["identity"].clone()).map_err(|error| {
                invalid(format!("campaign-start identity does not decode: {error}"))
            })?;
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
                        .load::<Value, CellValidation>(&cell.cell_id)
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
    let mut arms = Vec::with_capacity(2);
    for name in [&cell.baseline_arm, &cell.candidate_arm] {
        let journal_case = json!({"key": cell.cell_id, "cell_id": cell.cell_id, "arm": name});
        let mut observer = JournalObserver::new(log, journal_case.clone());
        let validated = validate_arm(
            root,
            plan,
            cell,
            declared,
            name,
            CHILD_TIMEOUT,
            &mut observer,
        )
        .map_err(|error| invalid(format!("cell {} arm {name}: {error}", cell.cell_id)))?;
        log.append(
            JournalEvent::DriverDiagnostic,
            Some(journal_case),
            json!({
                "kind": "arm-handshake",
                "windows": validated.windows,
                "selected_path": validated.selected_path,
            }),
        )?;
        arms.push(validated);
    }
    let handshake = CellValidation {
        cell_id: cell.cell_id.clone(),
        cache_state: declared.cache_state,
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

/// Resume identity pinning everything whose bytes can change what the smoke
/// observes: the protocol document, the producing-input closure, the ordered
/// cell list, the arm descriptors and every arm executable.
fn identity(
    root: &Path,
    plan: &RunnerPlan,
    plan_bytes: &[u8],
    addendum_bytes: &[u8],
    host: &HostObservation,
) -> io::Result<ResumeIdentity> {
    let snapshot = ProducingInputs::capture(root, plan.producing_manifest_path())?;
    let ordered: Vec<&str> = plan
        .cells
        .iter()
        .map(|cell| cell.cell_id.as_str())
        .collect();
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
        feature_contract: "dense-parity-harness; each arm's feature set is pinned by its \
             executable digest"
            .to_owned(),
        thread_contract: "one declared worker, no nested pools".to_owned(),
        host_identity: host.affinity.host_identity()?,
    })
}
