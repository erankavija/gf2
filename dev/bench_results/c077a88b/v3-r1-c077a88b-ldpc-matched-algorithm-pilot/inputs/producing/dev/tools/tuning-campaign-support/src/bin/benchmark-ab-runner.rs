//! Zen 3 benchmark protocol runner: paired, interleaved A/B cells under the
//! canonical quiet-host lock, with the shared append-only execution log and
//! checkpoint/resume primitives.
//!
//! `run <stage> <plan.json>` executes unfinished cells inside the lock wrapper
//! and pauses or completes; `finalize <stage> <out-dir>` assembles the receipt
//! directory from the stage after the wrapper returns. Every arm is a fresh
//! child process speaking the canonical child-v2 framing.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tuning_campaign_support::abtest::{
    bootstrap_seed, decide, median, pair_orders, paired_bootstrap_speedup, ArmOrder,
    PairedObservation,
};
use tuning_campaign_support::campaign::{LockEvidence, ProcessOutcome, Token};
use tuning_campaign_support::host::{
    inherited_lock, resolve_core_arm, CpuAffinity, HostObservation,
};
use tuning_campaign_support::journal::{
    atomic_write_new, CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity, TerminalState,
};
use tuning_campaign_support::process::run_process;
use tuning_campaign_support::protocol::{
    sha256_hex, ArtifactPin, CacheState, FamilyAddendum, RunnerPlan, ADDENDUM_SCHEMA_PATH,
    CONTRACT_PATH, PROTOCOL_PATH, RUNNER_LIFECYCLE_SCHEMA,
};
use tuning_campaign_support::provenance::{ProducingInputs, ProducingSnapshot};
use tuning_campaign_support::receipt::{
    ArmQuality, ArmRecord, BenchmarkReceipt, CampaignFacts, CellClaim, CellRecord, CellStatus,
    CheckpointRecord, ConversionCosts, ExecutionRecord, LockRecord, LogRecord, PairRecord,
    SourceIdentity, WindowRecord, WorkerReport, CHECKPOINT_DIR, LOG_FILE, PLAN_FILE, RECEIPT_FILE,
};
use tuning_campaign_support::transport::{self, FRESH_CASE_VALUE, FRESH_CASE_VAR};

/// Schema of the request each arm child reads on stdin.
const ARM_REQUEST_SCHEMA: &str = "zen3-benchmark-arm-request-v1";
/// Schema of the one result line each arm child writes.
const ARM_RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";
const CHILD_KILL_GRACE: Duration = Duration::from_secs(5);
static ALL_REAPED: AtomicBool = AtomicBool::new(true);

fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// Request forwarded to an arm child.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    decoder: Option<tuning_campaign_support::protocol::DecoderCell>,
    windows: u32,
    window_target_ms: u32,
    cpus: Vec<u32>,
    workers_declared: u32,
}

/// Result line an arm child writes.
#[derive(Clone, Debug, Serialize, Deserialize)]
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

fn command_text(program: &str, args: &[&str], cwd: &Path) -> io::Result<String> {
    let output = Command::new(program).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        return Err(invalid(format!("{program} {args:?} failed")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn repository_root() -> io::Result<PathBuf> {
    let cwd = fs::canonicalize(".")?;
    let top = fs::canonicalize(command_text(
        "git",
        &["rev-parse", "--show-toplevel"],
        &cwd,
    )?)?;
    if top != cwd {
        return Err(invalid(
            "the runner must be invoked from the repository root",
        ));
    }
    Ok(cwd)
}

fn source_identity(producing: ProducingSnapshot) -> SourceIdentity {
    SourceIdentity {
        revision: String::new(),
        producing,
    }
}

fn utc_compact() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{now}")
}

fn canonical_json<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(io::Error::other)
}

fn read_plan(path: &Path) -> io::Result<(RunnerPlan, Vec<u8>)> {
    let bytes = fs::read(path)?;
    let plan = RunnerPlan::decode(&bytes).map_err(invalid)?;
    Ok((plan, bytes))
}

fn load_addendum(bytes: &[u8], plan: &RunnerPlan) -> io::Result<FamilyAddendum> {
    let addendum = FamilyAddendum::decode(bytes).map_err(invalid)?;
    addendum
        .validate()
        .map_err(|errors| invalid(format!("addendum invalid: {}", errors.join("; "))))?;
    plan.validate(&addendum)
        .map_err(|errors| invalid(format!("plan invalid: {}", errors.join("; "))))?;
    Ok(addendum)
}

fn arm_records(plan: &RunnerPlan) -> io::Result<BTreeMap<String, ArmRecord>> {
    let mut arms = BTreeMap::new();
    for (name, arm) in &plan.arms {
        let path = fs::canonicalize(&arm.executable)?;
        let bytes = fs::read(&path)?;
        arms.insert(
            name.clone(),
            ArmRecord {
                build: arm.build,
                description: arm.description.clone(),
                executable_path: arm.executable.clone(),
                executable_sha256: sha256_hex(&bytes),
                arguments: arm.arguments.clone(),
                environment: arm.environment.clone(),
                rustflags: arm.rustflags.clone(),
                tuning_profile: arm.tuning_profile.as_ref().map(|profile| {
                    tuning_campaign_support::receipt::TuningProfilePin {
                        id: profile.id.clone(),
                        sha256: profile.sha256.clone(),
                    }
                }),
            },
        );
    }
    Ok(arms)
}

fn facts(
    root: &Path,
    stage: &Path,
    plan: &RunnerPlan,
    plan_bytes: &[u8],
) -> io::Result<CampaignFacts> {
    let protocol = ArtifactPin::capture(root, stage, PROTOCOL_PATH, "inputs/protocol.md")?;
    let contract =
        ArtifactPin::capture(root, stage, CONTRACT_PATH, "inputs/measurement-contract.md")?;
    let addendum_schema = ArtifactPin::capture(
        root,
        stage,
        ADDENDUM_SCHEMA_PATH,
        "inputs/addendum.schema.json",
    )?;
    let addendum =
        ArtifactPin::capture(root, stage, &plan.addendum, "inputs/family-addendum.json")?;
    let producing = ProducingInputs::capture_to(
        root,
        plan.producing_manifest_path(),
        &stage.join("inputs/producing"),
    )?;
    let source = source_identity(producing.clone());
    let family = FamilyAddendum::decode(&addendum.verify_content(stage).map_err(invalid)?)
        .map_err(invalid)?;
    let host = (family.protocol.version >= 2)
        .then(HostObservation::observe)
        .transpose()?;
    let arms = arm_records(plan)?;
    let (settings, settings_deviation) = plan.settings();
    let identity = ResumeIdentity {
        protocol_digest: protocol.sha256.clone(),
        source_revision: source.revision.clone(),
        source_sha256: producing.identity_sha256()?,
        ordered_work_manifest_sha256: sha256_hex(&canonical_json(&plan.cells)?),
        process_descriptors_sha256: sha256_hex(&canonical_json(&arms)?),
        executable_sha256: arms
            .iter()
            .map(|(name, arm)| (name.clone(), arm.executable_sha256.clone()))
            .collect(),
        behavior_sha256: producing.behavior_sha256.clone(),
        lifecycle_schema: RUNNER_LIFECYCLE_SCHEMA.to_owned(),
        lifecycle_behavior_sha256: producing.lifecycle_sha256()?,
        feature_contract: "release".to_owned(),
        thread_contract: format!(
            "RAYON_NUM_THREADS={}",
            env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "unset".into())
        ),
        host_identity: CpuAffinity::observe()?.host_identity()?,
    };
    Ok(CampaignFacts {
        host,
        trial_ledger: None,
        plan_sha256: sha256_hex(plan_bytes),
        identity,
        protocol,
        contract,
        addendum_schema,
        addendum,
        source,
        toolchain: command_text("rustc", &["--version"], root)?,
        settings,
        settings_deviation,
        arms,
    })
}

fn capture_referenced_receipts(
    root: &Path,
    stage: &Path,
    addendum: &FamilyAddendum,
) -> io::Result<()> {
    if let Some(evidence) = &addendum.effect.resolution_evidence {
        let pin = ArtifactPin::capture(
            root,
            stage,
            &evidence.receipt,
            "inputs/resolution-evidence/receipt.json",
        )?;
        if pin.sha256 != evidence.sha256 {
            return Err(invalid("resolution-evidence snapshot digest differs"));
        }
        if addendum.protocol.version >= 3 {
            let pilot = BenchmarkReceipt::decode(&pin.verify_content(stage).map_err(invalid)?)
                .map_err(invalid)?;
            let pilot_dir = root
                .join(&evidence.receipt)
                .parent()
                .ok_or_else(|| invalid("resolution evidence receipt has no parent"))?
                .to_path_buf();
            capture_pilot_input(
                &pilot_dir,
                stage,
                &pilot.addendum,
                "inputs/resolution-evidence/family-addendum.json",
            )?;
            let ledger = pilot
                .trial_ledger
                .as_ref()
                .ok_or_else(|| invalid("v3 resolution evidence pilot lacks trial ledger"))?;
            capture_pilot_input(
                &pilot_dir,
                stage,
                ledger,
                "inputs/resolution-evidence/trial-ledger.jsonl",
            )?;
        }
    }
    for trial in &addendum.family_wise.prior_trials {
        let snapshot = format!("inputs/prior-trials/{}.json", trial.sha256);
        let pin = ArtifactPin::capture(root, stage, &trial.receipt, &snapshot)?;
        if pin.sha256 != trial.sha256 {
            return Err(invalid(format!(
                "prior-trial snapshot {} digest differs",
                trial.receipt
            )));
        }
    }
    Ok(())
}

/// Copies an input already authenticated by a referenced pilot receipt. A
/// resumed campaign keeps the original bytes and rejects a changed source.
fn capture_pilot_input(
    pilot_dir: &Path,
    stage: &Path,
    pin: &ArtifactPin,
    snapshot: &str,
) -> io::Result<()> {
    let bytes = pin.verify_content(pilot_dir).map_err(invalid)?;
    let destination = stage.join(snapshot);
    if destination.exists() {
        if fs::read(&destination)? != bytes {
            return Err(invalid(
                "resolution evidence pilot input changed after freeze",
            ));
        }
        return Ok(());
    }
    fs::create_dir_all(destination.parent().unwrap())?;
    atomic_write_new(&destination, &bytes)
}

struct Session {
    log: ExecutionLog,
    checkpoints: CheckpointStore,
    plan: RunnerPlan,
    addendum: FamilyAddendum,
    facts: CampaignFacts,
    affinity: CpuAffinity,
    host: HostObservation,
}

fn open_session(root: &Path, stage: &Path, plan_path: &Path) -> io::Result<Session> {
    let (plan, plan_bytes) = read_plan(plan_path)?;
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
        tuning_campaign_support::journal::atomic_write_new(&staged_plan, &plan_bytes)?;
    }
    let mut facts = facts(root, &stage, &plan, &plan_bytes)?;
    let addendum_bytes = facts.addendum.verify_content(&stage).map_err(invalid)?;
    let addendum = load_addendum(&addendum_bytes, &plan)?;
    let protocol_text = facts.protocol.verify_content(&stage).map_err(invalid)?;
    if !String::from_utf8_lossy(&protocol_text).contains(&format!(
        "Protocol `zen3-benchmark-protocol` version {}.",
        addendum.protocol.version
    )) {
        return Err(invalid(
            "protocol document version differs from frozen addendum",
        ));
    }
    capture_referenced_receipts(root, &stage, &addendum)?;
    let holder = inherited_lock(Path::new(&plan.lock_path))?;
    if addendum.protocol.version >= 2 {
        if !tuning_campaign_support::protocol::freeze_precedes(
            &addendum.frozen,
            &facts
                .host
                .as_ref()
                .ok_or_else(|| invalid("versioned protocol lacks host observation"))?
                .observed_utc,
        ) {
            return Err(invalid("addendum freeze timestamp is after campaign start"));
        }
        facts.trial_ledger = Some(tuning_campaign_support::trial_ledger::reserve(
            root,
            &stage,
            &addendum,
            &plan.campaign_id,
            &facts.addendum.sha256,
            &tuning_campaign_support::trial_ledger::candidate_ids(&addendum, &plan, &facts.arms)?,
        )?);
    }
    let session_id = format!("session-{}-{}", utc_compact(), std::process::id());
    let log_path = stage.join(LOG_FILE);
    let resumed = log_path.exists();
    let mut log = if resumed {
        ExecutionLog::resume(&log_path, plan.campaign_id.clone(), session_id)?
    } else {
        ExecutionLog::create_new(&stage, plan.campaign_id.clone(), session_id)?
    };
    log.announce(io::stdout().lock())?;
    if !resumed {
        log.append(
            JournalEvent::CampaignStart,
            None,
            serde_json::to_value(&facts)?,
        )?;
    } else {
        let bytes = fs::read(&log_path)?;
        let records = ExecutionLog::validate_prefix(&bytes, &plan.campaign_id)?;
        let first: CampaignFacts = serde_json::from_value(records[0].details.clone())
            .map_err(|error| invalid(format!("campaign-start facts do not decode: {error}")))?;
        if !first.resume_equivalent(&facts) {
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
        CheckpointStore::resume(
            &checkpoint_root,
            plan.campaign_id.clone(),
            facts.identity.clone(),
        )?
    } else {
        CheckpointStore::create_new(
            &checkpoint_root,
            plan.campaign_id.clone(),
            facts.identity.clone(),
        )?
    };
    if let Some(recovery) = checkpoints.pending_recovery().cloned() {
        log.append(
            JournalEvent::PendingRecovery,
            None,
            serde_json::to_value(&recovery)?,
        )?;
        checkpoints.acknowledge_pending_recovery(&recovery.recovery_id)?;
    }
    let host = facts
        .host
        .clone()
        .map(Ok)
        .unwrap_or_else(HostObservation::observe)?;
    log.append(
        JournalEvent::DriverDiagnostic,
        None,
        json!({"kind": "host-observation", "observation": host}),
    )?;
    let evidence = LockEvidence {
        lock_path: fs::canonicalize(&plan.lock_path)?,
        holder_pid: holder,
        observation: Token::new("inherited-fd-and-independent-flock-conflict")?,
    };
    log.append(JournalEvent::LockHold, None, json!({"evidence": evidence}))?;
    log.append(
        JournalEvent::DriverDiagnostic,
        None,
        json!({
            "kind": "workers",
            "environment_rayon_threads": env::var("RAYON_NUM_THREADS").ok(),
            "runner_threads": 1,
        }),
    )?;
    let affinity = host.affinity.clone();
    Ok(Session {
        log,
        checkpoints,
        plan,
        addendum,
        facts,
        affinity,
        host,
    })
}

fn set_affinity(cpus: &[u32]) -> io::Result<()> {
    let mut set = rustix::thread::CpuSet::new();
    for cpu in cpus {
        set.set(*cpu as usize);
    }
    rustix::thread::sched_setaffinity(None, &set)?;
    Ok(())
}

fn run_arm(
    session: &mut Session,
    cell_id: &str,
    key: &str,
    arm_name: &str,
    role: &str,
    pair: u32,
    request: &ArmRequest,
) -> io::Result<ExecutionRecord> {
    let arm = session
        .plan
        .arms
        .get(arm_name)
        .ok_or_else(|| invalid("unknown arm"))?;
    let record = session
        .facts
        .arms
        .get(arm_name)
        .ok_or_else(|| invalid("unknown arm"))?;
    let mut command = Command::new(&record.executable_path);
    command.args(&arm.arguments);
    command.env_clear();
    for (name, value) in ["PATH", "HOME", "RAYON_NUM_THREADS", "RUSTUP_TOOLCHAIN"]
        .iter()
        .filter_map(|name| env::var(name).ok().map(|value| (*name, value)))
    {
        command.env(name, value);
    }
    for (name, value) in &arm.environment {
        command.env(name, value);
    }
    command.env(FRESH_CASE_VAR, FRESH_CASE_VALUE);
    let input = transport::encode_case(request).map_err(invalid)?;
    let case = json!({"key": key, "cell_id": cell_id, "arm": arm_name, "role": role, "pair": pair});
    let start = Instant::now();
    let timeout = Duration::from_secs(session.facts.settings.child_timeout_seconds);
    let log = &mut session.log;
    let mut spawned_pid = 0;
    let mut stderr = Vec::new();
    let result = run_process(
        command,
        input.as_bytes(),
        timeout,
        CHILD_KILL_GRACE,
        || Ok(()),
        &ALL_REAPED,
        |pid| {
            spawned_pid = pid;
            log.append(
                JournalEvent::ChildSpawn,
                Some(case.clone()),
                json!({"pid": pid}),
            )
            .map(|_| ())
        },
        |chunk| {
            stderr.extend_from_slice(chunk);
            Ok(())
        },
    )?;
    if !stderr.is_empty() {
        session.log.append(
            JournalEvent::ChildDiagnostic,
            Some(case.clone()),
            json!({"stderr": String::from_utf8_lossy(&stderr)}),
        )?;
    }
    session.log.append(
        JournalEvent::ChildExit,
        Some(case.clone()),
        json!({"outcome": result.outcome}),
    )?;
    if let Some(error) = result.callback_error {
        return Err(error);
    }
    match result.outcome {
        ProcessOutcome::Exited { exit_code: 0, .. } => {}
        other => {
            return Err(invalid(format!(
                "arm {arm_name} did not exit cleanly: {other:?}"
            )))
        }
    }
    let text = String::from_utf8(result.stdout).map_err(|_| invalid("arm stdout is not UTF-8"))?;
    let parsed: ArmResult = transport::parse_result(&text).map_err(invalid)?;
    if parsed.schema != ARM_RESULT_SCHEMA {
        return Err(invalid(format!("arm result schema {:?}", parsed.schema)));
    }
    if parsed.windows.len() != request.windows as usize
        || parsed
            .windows
            .iter()
            .any(|window| window.calls == 0 || window.elapsed_ns == 0)
    {
        return Err(invalid(format!(
            "arm {arm_name} returned an invalid window set"
        )));
    }
    let mut values: Vec<f64> = parsed
        .windows
        .iter()
        .map(|window| window.ns_per_call())
        .collect();
    let ns_per_call = median(&mut values).map_err(invalid)?;
    let execution = ExecutionRecord {
        quality: parsed.quality.clone(),
        calibrated: parsed.calibrated,
        arm: arm_name.to_owned(),
        pid: spawned_pid,
        windows: parsed.windows,
        ns_per_call,
        cache_state_applied: parsed.cache_state_applied,
        workers_observed: parsed.workers_observed,
        cpus_observed: parsed.cpus_observed,
        selected_path: parsed.selected_path,
        conversion: parsed.conversion,
        elapsed_ns: start.elapsed().as_nanos() as u64,
    };
    session.log.append(
        JournalEvent::ExecutionProgress,
        Some(case),
        json!({"ns_per_call": ns_per_call, "windows": execution.windows, "quality": parsed.quality}),
    )?;
    Ok(execution)
}

fn measure_cell(session: &mut Session, index: usize) -> io::Result<()> {
    let plan_cell = session.plan.cells[index].clone();
    let key = plan_cell.cell_id.clone();
    let declared = session
        .addendum
        .cell(&plan_cell.cell_id)
        .cloned()
        .ok_or_else(|| invalid("undeclared cell"))?;
    let case = json!({"key": key, "cell_id": plan_cell.cell_id, "case": plan_cell.case});
    session.log.append(
        JournalEvent::CellStart,
        Some(case.clone()),
        json!({"role": declared.role, "core_arm": declared.core_arm}),
    )?;
    let settings = session.facts.settings;
    let mut record = CellRecord {
        cell_id: plan_cell.cell_id.clone(),
        key: key.clone(),
        role: declared.role,
        baseline_arm: plan_cell.baseline_arm.clone(),
        candidate_arm: plan_cell.candidate_arm.clone(),
        core_arm: declared.core_arm,
        resolved_cpus: Vec::new(),
        status: CellStatus::Unavailable,
        unavailable_reason: None,
        pairs: Vec::new(),
        claimed: None,
        decoder_quality: None,
        checkpoint_sha256: None,
    };
    match resolve_core_arm(declared.core_arm, &session.host.topology, &session.affinity) {
        Err(reason) => {
            record.unavailable_reason = Some(reason.clone());
            session.log.append(
                JournalEvent::Omission,
                Some(case.clone()),
                json!({"kind": "inapplicable-arm", "reason": reason}),
            )?;
        }
        Ok(cpus) => {
            record.resolved_cpus = cpus.clone();
            record.status = CellStatus::Measured;
            let pairs = plan_cell.pair_count(declared.role, &settings);
            let cell_seed = bootstrap_seed(session.plan.campaign_seed, &key);
            let orders = pair_orders(cell_seed, pairs as usize);
            set_affinity(&cpus)?;
            let outcome = (|| -> io::Result<()> {
                for (pair, order) in orders.iter().enumerate() {
                    let request = |arm: &str, role: &str| ArmRequest {
                        schema: ARM_REQUEST_SCHEMA.into(),
                        cell_id: plan_cell.cell_id.clone(),
                        arm: arm.to_owned(),
                        role: role.to_owned(),
                        pair: pair as u32,
                        case: plan_cell.case.clone(),
                        cache_state: declared.cache_state,
                        cold_calls: declared.cold_calls,
                        decoder: declared.decoder.clone(),
                        windows: settings.windows_per_execution,
                        window_target_ms: settings.window_target_ms,
                        cpus: cpus.clone(),
                        workers_declared: declared.workers.declared,
                    };
                    let baseline = plan_cell.baseline_arm.clone();
                    let candidate = plan_cell.candidate_arm.clone();
                    let (first, second) = match order {
                        ArmOrder::BaselineFirst => (
                            ("baseline", baseline.clone()),
                            ("candidate", candidate.clone()),
                        ),
                        ArmOrder::CandidateFirst => (
                            ("candidate", candidate.clone()),
                            ("baseline", baseline.clone()),
                        ),
                    };
                    let first_record = run_arm(
                        session,
                        &plan_cell.cell_id,
                        &key,
                        &first.1,
                        first.0,
                        pair as u32,
                        &request(&first.1, first.0),
                    )?;
                    let second_record = run_arm(
                        session,
                        &plan_cell.cell_id,
                        &key,
                        &second.1,
                        second.0,
                        pair as u32,
                        &request(&second.1, second.0),
                    )?;
                    let (baseline_record, candidate_record) = match order {
                        ArmOrder::BaselineFirst => (first_record, second_record),
                        ArmOrder::CandidateFirst => (second_record, first_record),
                    };
                    if declared.decoder.is_some() {
                        let quality = tuning_campaign_support::receipt::DecoderQualityRecord {
                            baseline: baseline_record
                                .quality
                                .clone()
                                .ok_or_else(|| invalid("baseline decoder omitted quality"))?,
                            candidate: candidate_record
                                .quality
                                .clone()
                                .ok_or_else(|| invalid("candidate decoder omitted quality"))?,
                        };
                        if record.decoder_quality.as_ref().is_some_and(|prior| {
                            if session.addendum.protocol.version >= 3 {
                                !prior.baseline.same_decoder_evidence(&quality.baseline)
                                    || !prior.candidate.same_decoder_evidence(&quality.candidate)
                            } else {
                                prior != &quality
                            }
                        }) {
                            return Err(invalid("decoder quality changed across repeated timing executions on the same frozen frames"));
                        }
                        record.decoder_quality = Some(quality);
                    }
                    record.pairs.push(PairRecord {
                        index: pair as u32,
                        order: *order,
                        baseline: baseline_record,
                        candidate: candidate_record,
                    });
                }
                Ok(())
            })();
            set_affinity(session.affinity.cpus())?;
            outcome?;
            let observations: Vec<_> = record
                .pairs
                .iter()
                .map(|pair| PairedObservation {
                    baseline_ns_per_call: pair.baseline.ns_per_call,
                    candidate_ns_per_call: pair.candidate.ns_per_call,
                })
                .collect();
            let comparisons = if let Some(pin) = &session.facts.trial_ledger {
                tuning_campaign_support::trial_ledger::verify(
                    pin,
                    session.log.path().parent().unwrap(),
                    &session.addendum,
                    &session.plan.campaign_id,
                    &session.facts.addendum.sha256,
                    &tuning_campaign_support::trial_ledger::candidate_ids(
                        &session.addendum,
                        &session.plan,
                        &session.facts.arms,
                    )?,
                )?
            } else {
                session.addendum.family_comparisons().max(1)
            };
            let alpha = if let Some(pin) = &session.facts.trial_ledger {
                tuning_campaign_support::trial_ledger::attempt_alpha(
                    pin,
                    session.log.path().parent().unwrap(),
                    &session.addendum,
                )?
            } else {
                settings.family_alpha
            };
            let corrected_alpha = alpha / f64::from(comparisons);
            let interval = paired_bootstrap_speedup(
                &observations,
                settings.bootstrap_resamples,
                corrected_alpha,
                cell_seed,
            )
            .map_err(invalid)?;
            if let Some(margins) = session.addendum.margins(&declared) {
                let decision = decide(&interval, &margins).map_err(invalid)?;
                record.claimed = Some(CellClaim {
                    interval,
                    decision,
                    margins,
                });
            }
        }
    }
    let accepted = session.checkpoints.accept(&key, &case, &record)?;
    session.log.append(
        JournalEvent::CheckpointAccepted,
        Some(case.clone()),
        json!({"sha256": accepted.sha256, "path": accepted.path}),
    )?;
    session.log.append(
        JournalEvent::CellComplete,
        Some(case),
        json!({"status": record.status, "pairs": record.pairs.len(), "claimed": record.claimed}),
    )?;
    Ok(())
}

fn run(stage: &Path, plan_path: &Path) -> io::Result<i32> {
    let root = repository_root()?;
    let mut session = open_session(&root, stage, plan_path)?;
    let budget = session.plan.max_cells_per_session;
    let mut measured = 0u32;
    let outcome = (|| -> io::Result<TerminalState> {
        for index in 0..session.plan.cells.len() {
            let key = session.plan.cells[index].cell_id.clone();
            if session.checkpoints.completed_unit(&key).is_some() {
                session.log.append(
                    JournalEvent::Omission,
                    Some(json!({"key": key})),
                    json!({"kind": "completed-in-prior-session"}),
                )?;
                continue;
            }
            if budget.is_some_and(|limit| measured >= limit) {
                return Ok(TerminalState::Paused);
            }
            measure_cell(&mut session, index)?;
            measured += 1;
        }
        Ok(TerminalState::Complete)
    })();
    match outcome {
        Ok(state) => {
            session
                .log
                .terminal(state, json!({"measured_cells": measured}))?;
            Ok(if state == TerminalState::Complete {
                0
            } else {
                3
            })
        }
        Err(error) => {
            session
                .log
                .terminal(TerminalState::Failed, json!({"error": error.to_string()}))?;
            Err(error)
        }
    }
}

fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

fn finalize(stage: &Path, out_dir: &Path) -> io::Result<()> {
    let root = repository_root()?;
    let stage = fs::canonicalize(stage)?;
    let (plan, plan_bytes) = read_plan(&stage.join(PLAN_FILE))?;
    let log_bytes = fs::read(stage.join(LOG_FILE))?;
    let records = ExecutionLog::validate_prefix(&log_bytes, &plan.campaign_id)?;
    let facts: CampaignFacts = serde_json::from_value(
        records
            .first()
            .filter(|record| record.event == JournalEvent::CampaignStart)
            .ok_or_else(|| invalid("log lacks campaign-start"))?
            .details
            .clone(),
    )
    .map_err(|error| invalid(format!("campaign facts do not decode: {error}")))?;
    if facts.plan_sha256 != sha256_hex(&plan_bytes) {
        return Err(invalid(
            "staged plan differs from the campaign-start plan digest",
        ));
    }
    let addendum = FamilyAddendum::decode(&facts.addendum.verify_content(&stage).map_err(invalid)?)
        .map_err(invalid)?;
    let terminal = records.iter().rev().find(|record| {
        matches!(
            record.event,
            JournalEvent::Complete
                | JournalEvent::Failed
                | JournalEvent::Paused
                | JournalEvent::BudgetExhausted
        )
    });
    if terminal.map(|record| record.event) != Some(JournalEvent::Complete) {
        return Err(invalid(
            "campaign is not complete; resume it before finalizing",
        ));
    }
    let sessions = records
        .iter()
        .filter(|record| {
            matches!(
                record.event,
                JournalEvent::CampaignStart | JournalEvent::SessionStart
            )
        })
        .count() as u32;
    let host: HostObservation = records
        .iter()
        .rev()
        .find(|record| {
            record.event == JournalEvent::DriverDiagnostic
                && record.details.get("kind").and_then(Value::as_str) == Some("host-observation")
        })
        .and_then(|record| serde_json::from_value(record.details["observation"].clone()).ok())
        .ok_or_else(|| invalid("log lacks a host observation"))?;
    let lock = records
        .iter()
        .rev()
        .find(|record| record.event == JournalEvent::LockHold)
        .ok_or_else(|| invalid("log lacks lock-hold evidence"))?;
    let evidence: LockEvidence = serde_json::from_value(lock.details["evidence"].clone())
        .map_err(|error| invalid(format!("lock evidence does not decode: {error}")))?;
    let workers = records
        .iter()
        .rev()
        .find(|record| {
            record.event == JournalEvent::DriverDiagnostic
                && record.details.get("kind").and_then(Value::as_str) == Some("workers")
        })
        .ok_or_else(|| invalid("log lacks a worker report"))?;
    let checkpoints = CheckpointStore::resume(
        stage.join(CHECKPOINT_DIR),
        plan.campaign_id.clone(),
        facts.identity.clone(),
    )?;
    let mut cells = Vec::new();
    for plan_cell in &plan.cells {
        let unit = checkpoints
            .completed_unit(&plan_cell.cell_id)
            .ok_or_else(|| invalid(format!("cell {} has no checkpoint", plan_cell.cell_id)))?
            .clone();
        let (_case, mut record): (Value, CellRecord) = checkpoints.load(&plan_cell.cell_id)?;
        record.checkpoint_sha256 = Some(unit.sha256);
        cells.push(record);
    }
    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join(PLAN_FILE), &plan_bytes)?;
    fs::write(out_dir.join(LOG_FILE), &log_bytes)?;
    copy_tree(&stage.join("inputs"), &out_dir.join("inputs"))?;
    let checkpoint_out = out_dir.join(CHECKPOINT_DIR);
    fs::create_dir_all(&checkpoint_out)?;
    fs::copy(
        stage.join(CHECKPOINT_DIR).join("manifest.json"),
        checkpoint_out.join("manifest.json"),
    )?;
    copy_tree(
        &stage.join(CHECKPOINT_DIR).join("units"),
        &checkpoint_out.join("units"),
    )?;
    let manifest_bytes = fs::read(checkpoint_out.join("manifest.json"))?;
    let receipt_path = fs::canonicalize(out_dir)?
        .strip_prefix(fs::canonicalize(&root)?)
        .map_err(|_| invalid("receipt output must be inside the repository"))?
        .join(RECEIPT_FILE)
        .to_string_lossy()
        .into_owned();
    let session_hosts = if facts.host.is_some() {
        records
            .iter()
            .filter(|record| {
                record.event == JournalEvent::DriverDiagnostic
                    && record.details["kind"] == "host-observation"
            })
            .map(|record| {
                serde_json::from_value(record.details["observation"].clone()).map_err(invalid)
            })
            .collect::<io::Result<Vec<HostObservation>>>()?
    } else {
        Vec::new()
    };
    let receipt = BenchmarkReceipt {
        session_hosts,
        trial_ledger: facts.trial_ledger,
        schema: tuning_campaign_support::protocol::RECEIPT_SCHEMA_ID.into(),
        campaign_id: plan.campaign_id.clone(),
        family_id: addendum.family.id.clone(),
        issue: plan.issue.clone(),
        receipt_path,
        label: plan.label,
        campaign_seed: plan.campaign_seed,
        settings: facts.settings,
        settings_deviation: facts.settings_deviation,
        protocol: facts.protocol,
        contract: facts.contract,
        addendum_schema: facts.addendum_schema,
        addendum: facts.addendum,
        source: facts.source,
        toolchain: facts.toolchain,
        host,
        lock: LockRecord {
            lock_path: evidence.lock_path.to_string_lossy().into_owned(),
            holder_pid: evidence.holder_pid,
            observation: evidence.observation.as_str().to_owned(),
            wrapper: String::new(),
        },
        workers: WorkerReport {
            environment_rayon_threads: workers.details["environment_rayon_threads"]
                .as_str()
                .map(str::to_owned),
            runner_threads: workers.details["runner_threads"].as_u64().unwrap_or(0) as u32,
        },
        execution_log: LogRecord {
            path: LOG_FILE.into(),
            sha256: sha256_hex(&log_bytes),
            sessions,
            resumed: sessions > 1,
        },
        checkpoints: CheckpointRecord {
            manifest_path: format!("{CHECKPOINT_DIR}/manifest.json"),
            manifest_sha256: sha256_hex(&manifest_bytes),
        },
        arms: facts.arms,
        cells,
    };
    let mut bytes = serde_json::to_vec_pretty(&receipt)?;
    bytes.push(b'\n');
    fs::write(out_dir.join(RECEIPT_FILE), bytes)?;
    let _ = root;
    println!(
        "GF2_BENCHMARK_RECEIPT={}",
        out_dir.join(RECEIPT_FILE).display()
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [_, "run", stage, plan] => run(Path::new(stage), Path::new(plan)),
        [_, "finalize", stage, out_dir] => {
            finalize(Path::new(stage), Path::new(out_dir)).map(|()| 0)
        }
        _ => {
            eprintln!(
                "usage: benchmark-ab-runner run <stage> <plan.json> | finalize <stage> <out-dir>"
            );
            std::process::exit(2);
        }
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("benchmark-ab-runner: {error}");
            let _ = io::stderr().flush();
            std::process::exit(1);
        }
    }
}
