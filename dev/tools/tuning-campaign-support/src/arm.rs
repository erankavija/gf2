//! The arm-request contract the benchmark runner speaks, the child dispatch it
//! speaks it over, and the non-timed smoke that drives a saved plan through it.
//!
//! One request type serves both positions of a timed pair and the non-timed
//! validation position, so a harness that smokes its arms and the campaign that
//! measures them cannot drift apart on the wire.

use crate::campaign::{ProcessOutcome, CHILD_KILL_GRACE_SECONDS};
use crate::journal::{ExecutionLog, JournalEvent};
use crate::process::run_process;
use crate::protocol::{
    sha256_hex, CacheState, CellDeclaration, DecoderCell, FamilyAddendum, PlanArm, PlanCell,
    RunnerPlan, SharedSettings,
};
use crate::receipt::{ArmQuality, ConversionCosts, WindowRecord};
use crate::transport::{self, FRESH_CASE_VALUE, FRESH_CASE_VAR};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Schema of the request each arm child reads on stdin.
pub const ARM_REQUEST_SCHEMA: &str = "zen3-benchmark-arm-request-v1";
/// Schema of the one result line each arm child writes.
pub const ARM_RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";
/// Schema of the non-timed smoke's record.
pub const SMOKE_RECORD_SCHEMA: &str = "zen3-arm-smoke-record-v1";

/// Variables an arm child inherits from the runner's own environment; the
/// child's environment is otherwise the plan arm's declaration and the
/// fresh-child sentinel.
pub const INHERITED_ENVIRONMENT: [&str; 4] =
    ["PATH", "HOME", "RAYON_NUM_THREADS", "RUSTUP_TOOLCHAIN"];

const CHILD_KILL_GRACE: Duration = Duration::from_secs(CHILD_KILL_GRACE_SECONDS);
static ALL_REAPED: AtomicBool = AtomicBool::new(true);

fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// Position an execution takes in its cell.
///
/// The value carries the position in the counterbalanced pair, not the cell's
/// sampling role: [`crate::protocol::CellRole`] never reaches an arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PairPosition {
    /// The pair's baseline arm.
    Baseline,
    /// The pair's candidate arm.
    Candidate,
    /// The non-timed position: the arm performs one untimed dispatch and
    /// reports no timing window. Only [`smoke`] builds a request in it; a
    /// campaign execution cannot take it, which [`ArmRequest::timed`] refuses.
    Validation,
}

impl PairPosition {
    /// Whether an execution in this position carries a timing-window budget.
    pub fn is_timed(self) -> bool {
        self != Self::Validation
    }
}

/// Request the runner writes on an arm child's stdin.
///
/// The field order is part of the contract: the canonical framing accepts a
/// request only when the child's own mirror re-encodes the received bytes
/// exactly, so an added, dropped, renamed or reordered field, and an absent
/// optional field spelled `null`, all fail the child's decode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmRequest {
    /// Request schema identity, always [`ARM_REQUEST_SCHEMA`].
    pub schema: String,
    /// Frozen cell this execution belongs to.
    pub cell_id: String,
    /// Plan name of the arm this child runs.
    pub arm: String,
    /// Position this execution takes in the cell.
    pub role: PairPosition,
    /// Zero-based pair index; zero in the validation position.
    pub pair: u32,
    /// The plan cell's case, forwarded verbatim.
    pub case: Value,
    /// Cache state the cell declares.
    pub cache_state: CacheState,
    /// Frozen call count of a `cold` cell; absent cells calibrate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cold_calls: Option<u64>,
    /// Decoder contract of a decoder cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<DecoderCell>,
    /// Timing windows this execution measures; zero in the validation position.
    pub windows: u32,
    /// Target length of one timing window in milliseconds; zero in the
    /// validation position.
    pub window_target_ms: u32,
    /// CPUs resolved for the cell's core arm.
    pub cpus: Vec<u32>,
    /// Workers the cell declares.
    pub workers_declared: u32,
}

impl ArmRequest {
    #[allow(clippy::too_many_arguments)]
    fn build(
        cell: &PlanCell,
        declared: &CellDeclaration,
        arm: &str,
        role: PairPosition,
        pair: u32,
        cpus: Vec<u32>,
        windows: u32,
        window_target_ms: u32,
    ) -> Self {
        Self {
            schema: ARM_REQUEST_SCHEMA.to_owned(),
            cell_id: cell.cell_id.clone(),
            arm: arm.to_owned(),
            role,
            pair,
            case: cell.case.clone(),
            cache_state: declared.cache_state,
            cold_calls: declared.cold_calls,
            decoder: declared.decoder.clone(),
            windows,
            window_target_ms,
            cpus,
            workers_declared: declared.workers.declared,
        }
    }

    /// Request of one timed execution: the window budget comes from the
    /// settings in force and `cpus` from the resolved core arm.
    ///
    /// Fails on [`PairPosition::Validation`], which carries no timing window
    /// and which therefore no campaign execution takes.
    pub fn timed(
        cell: &PlanCell,
        declared: &CellDeclaration,
        arm: &str,
        role: PairPosition,
        pair: u32,
        cpus: Vec<u32>,
        settings: &SharedSettings,
    ) -> Result<Self, String> {
        if !role.is_timed() {
            return Err(format!(
                "a timed execution cannot take the {} position",
                serde_json::to_value(role)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default()
            ));
        }
        Ok(Self::build(
            cell,
            declared,
            arm,
            role,
            pair,
            cpus,
            settings.windows_per_execution,
            settings.window_target_ms,
        ))
    }

    /// Request of the non-timed validation dispatch of `arm` on `cell`.
    ///
    /// The window budget is zero, so an arm that keys on the position and an
    /// arm that keys on the budget both stay out of the timing protocol. The
    /// smoke reserves no CPU, so the cell's declared worker width stands in for
    /// the resolved CPU set.
    pub fn validation(cell: &PlanCell, declared: &CellDeclaration, arm: &str) -> Self {
        Self::build(
            cell,
            declared,
            arm,
            PairPosition::Validation,
            0,
            (0..declared.workers.declared).collect(),
            0,
            0,
        )
    }
}

/// The one result line an arm child writes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmResult {
    /// Result schema identity, always [`ARM_RESULT_SCHEMA`].
    pub schema: String,
    /// Timing windows in acquisition order; empty in the validation position.
    pub windows: Vec<WindowRecord>,
    /// Cache state the arm reports applying.
    pub cache_state_applied: CacheState,
    /// Threads alive in the arm after its dispatch.
    pub workers_observed: u32,
    /// CPUs the arm observed for itself.
    pub cpus_observed: Vec<u32>,
    /// Route provenance the arm observed at run time.
    pub selected_path: Option<String>,
    /// Conversion costs around the kernel, when the arm accounts for them.
    pub conversion: Option<ConversionCosts>,
    /// Decoder quality, for a decoder cell.
    pub quality: Option<ArmQuality>,
    /// Whether the arm calibrated its call count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated: Option<bool>,
}

/// Observer of one arm child's lifecycle, so a caller with an execution log
/// journals what it did. `()` observes nothing.
pub trait ChildObserver {
    /// Announces the validation dispatch of `arm` on `cell_id`, so an observer
    /// that journals files the dispatch's records under that dispatch's case.
    fn validating(&mut self, cell_id: &str, arm: &str) -> io::Result<()> {
        let _ = (cell_id, arm);
        Ok(())
    }

    /// Receives the PID of the spawned child.
    fn spawned(&mut self, pid: u32) -> io::Result<()> {
        let _ = pid;
        Ok(())
    }

    /// Receives the observed terminal state and every standard-error byte,
    /// before the dispatch judges the child, so a failing child is recorded.
    fn exited(&mut self, outcome: &ProcessOutcome, stderr: &[u8]) -> io::Result<()> {
        let _ = (outcome, stderr);
        Ok(())
    }
}

impl ChildObserver for () {}

/// Journals one arm child's spawn, diagnostics and exit into an execution log.
pub struct JournalObserver<'a> {
    log: &'a mut ExecutionLog,
    case: Value,
    /// PID of the spawned child, zero until it is observed.
    pub pid: u32,
}

impl<'a> JournalObserver<'a> {
    /// Journals into `log` under `case`, the journal case of this execution.
    pub fn new(log: &'a mut ExecutionLog, case: Value) -> Self {
        Self { log, case, pid: 0 }
    }
}

impl ChildObserver for JournalObserver<'_> {
    fn validating(&mut self, cell_id: &str, arm: &str) -> io::Result<()> {
        self.case = json!({
            "key": cell_id,
            "cell_id": cell_id,
            "arm": arm,
            "role": PairPosition::Validation,
        });
        Ok(())
    }

    fn spawned(&mut self, pid: u32) -> io::Result<()> {
        self.pid = pid;
        self.log
            .append(
                JournalEvent::ChildSpawn,
                Some(self.case.clone()),
                json!({"pid": pid}),
            )
            .map(|_| ())
    }

    fn exited(&mut self, outcome: &ProcessOutcome, stderr: &[u8]) -> io::Result<()> {
        if !stderr.is_empty() {
            self.log.append(
                JournalEvent::ChildDiagnostic,
                Some(self.case.clone()),
                json!({"stderr": String::from_utf8_lossy(stderr)}),
            )?;
        }
        self.log
            .append(
                JournalEvent::ChildExit,
                Some(self.case.clone()),
                json!({"outcome": outcome}),
            )
            .map(|_| ())
    }
}

/// Runs one arm child on `request` and returns the result line it wrote.
///
/// The child is a fresh process with the runner's own environment contract:
/// [`INHERITED_ENVIRONMENT`] from this process, the plan arm's declared
/// variables and the fresh-child sentinel, and nothing else.
///
/// Fails when the child exits other than cleanly, writes something other than
/// exactly one canonical result line, or names another result schema.
pub fn dispatch_arm(
    executable: &Path,
    arm: &PlanArm,
    request: &ArmRequest,
    timeout: Duration,
    observer: &mut dyn ChildObserver,
) -> io::Result<ArmResult> {
    let mut command = Command::new(executable);
    command.args(&arm.arguments);
    command.env_clear();
    for (name, value) in INHERITED_ENVIRONMENT
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|value| (*name, value)))
    {
        command.env(name, value);
    }
    for (name, value) in &arm.environment {
        command.env(name, value);
    }
    command.env(FRESH_CASE_VAR, FRESH_CASE_VALUE);
    let input = transport::encode_case(request).map_err(invalid)?;
    let mut stderr = Vec::new();
    let result = run_process(
        command,
        input.as_bytes(),
        timeout,
        CHILD_KILL_GRACE,
        || Ok(()),
        &ALL_REAPED,
        |pid| observer.spawned(pid),
        |chunk| {
            stderr.extend_from_slice(chunk);
            Ok(())
        },
    )?;
    observer.exited(&result.outcome, &stderr)?;
    if let Some(error) = result.callback_error {
        return Err(error);
    }
    match result.outcome {
        ProcessOutcome::Exited { exit_code: 0, .. } => {}
        other => {
            return Err(invalid(format!(
                "arm {} did not exit cleanly: {other:?}",
                request.arm
            )))
        }
    }
    let text = String::from_utf8(result.stdout).map_err(|_| invalid("arm stdout is not UTF-8"))?;
    let parsed: ArmResult = transport::parse_result(&text).map_err(invalid)?;
    if parsed.schema != ARM_RESULT_SCHEMA {
        return Err(invalid(format!("arm result schema {:?}", parsed.schema)));
    }
    Ok(parsed)
}

/// What one arm reported in the validation position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmValidation {
    /// Plan name of the arm.
    pub arm: String,
    /// Position of the dispatch, always [`PairPosition::Validation`].
    pub role: PairPosition,
    /// Executable the plan named, as the repository sees it.
    pub executable: String,
    /// Content identity of that executable.
    pub executable_sha256: String,
    /// Cache state the cell declares.
    pub cache_state_declared: CacheState,
    /// Cache state the arm reports applying.
    pub cache_state_applied: CacheState,
    /// Route provenance the arm reports.
    pub selected_path: String,
    /// Timing windows the arm reported, always zero.
    pub windows: usize,
}

/// Both arms' validation dispatches of one declared cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellValidation {
    /// Frozen cell identifier.
    pub cell_id: String,
    /// Cache state the cell declares.
    pub cache_state: CacheState,
    /// Baseline then candidate, in the order the smoke drove them.
    pub arms: Vec<ArmValidation>,
}

/// One non-timed smoke of one saved plan.
///
/// Every value is observed at run time or derived from the plan and the
/// addendum it names, and none is a clock reading, so two runs over one build
/// reproduce the record byte for byte.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmokeRecord {
    /// Record schema identity, always [`SMOKE_RECORD_SCHEMA`].
    pub schema: String,
    /// Campaign the plan projects.
    pub campaign_id: String,
    /// Issue the plan names.
    pub issue: String,
    /// Family the addendum declares.
    pub family: String,
    /// Content identity of the plan bytes.
    pub plan_sha256: String,
    /// Repository-relative addendum path the plan names.
    pub addendum: String,
    /// Content identity of the addendum bytes.
    pub addendum_sha256: String,
    /// Every cell of the plan, in the plan's order.
    pub cells: Vec<CellValidation>,
}

impl SmokeRecord {
    /// The record of `cells` validated against the plan and the family
    /// addendum whose bytes these are, in the plan's cell order.
    pub fn of(
        plan: &RunnerPlan,
        plan_bytes: &[u8],
        addendum: &FamilyAddendum,
        addendum_bytes: &[u8],
        cells: Vec<CellValidation>,
    ) -> Self {
        Self {
            schema: SMOKE_RECORD_SCHEMA.to_owned(),
            campaign_id: plan.campaign_id.clone(),
            issue: plan.issue.clone(),
            family: addendum.family.id.clone(),
            plan_sha256: sha256_hex(plan_bytes),
            addendum: plan.addendum.clone(),
            addendum_sha256: sha256_hex(addendum_bytes),
            cells,
        }
    }
}

/// Drives one arm of one declared cell once in the validation position.
///
/// `root` is the repository root a repository-relative executable resolves
/// against, `observer` the caller's journal. Fails when the arm reports a
/// timing window, applies a cache state other than the declared one, or reports
/// no route provenance.
pub fn validate_arm(
    root: &Path,
    plan: &RunnerPlan,
    cell: &PlanCell,
    declared: &CellDeclaration,
    arm_name: &str,
    timeout: Duration,
    observer: &mut dyn ChildObserver,
) -> io::Result<ArmValidation> {
    let arm = plan
        .arms
        .get(arm_name)
        .ok_or_else(|| invalid(format!("the plan declares no arm {arm_name:?}")))?;
    let executable = resolve(root, &arm.executable)?;
    let executable_sha256 = sha256_hex(&std::fs::read(&executable)?);
    let request = ArmRequest::validation(cell, declared, arm_name);
    let result = dispatch_arm(&executable, arm, &request, timeout, observer)?;
    if !result.windows.is_empty() {
        return Err(invalid(format!(
            "the arm reported {} timing windows; the validation position measures none",
            result.windows.len()
        )));
    }
    if result.cache_state_applied != declared.cache_state {
        return Err(invalid(format!(
            "the arm applied cache state {:?} against the declared {:?}",
            result.cache_state_applied, declared.cache_state
        )));
    }
    let selected_path = result
        .selected_path
        .ok_or_else(|| invalid("the arm reported no route provenance"))?;
    Ok(ArmValidation {
        arm: arm_name.to_owned(),
        role: request.role,
        executable: repository_relative(root, &executable),
        executable_sha256,
        cache_state_declared: declared.cache_state,
        cache_state_applied: result.cache_state_applied,
        selected_path,
        windows: result.windows.len(),
    })
}

/// Drives both arms of one cell of a saved plan once in the validation
/// position, in the plan's baseline-then-candidate order.
///
/// `observer` hears each dispatch before it starts, so a caller with an
/// execution log journals the cell's children under their own cases. Fails,
/// naming the cell and the arm, on every defect [`validate_arm`] names, and
/// before any arm runs when the addendum declares no such cell.
pub fn validate_cell(
    root: &Path,
    plan: &RunnerPlan,
    addendum: &FamilyAddendum,
    cell: &PlanCell,
    timeout: Duration,
    observer: &mut dyn ChildObserver,
) -> io::Result<CellValidation> {
    let declared = addendum
        .cell(&cell.cell_id)
        .ok_or_else(|| invalid(format!("the addendum declares no cell {}", cell.cell_id)))?;
    let mut arms = Vec::with_capacity(2);
    for arm_name in [&cell.baseline_arm, &cell.candidate_arm] {
        observer.validating(&cell.cell_id, arm_name)?;
        arms.push(
            validate_arm(root, plan, cell, declared, arm_name, timeout, observer).map_err(
                |error| invalid(format!("cell {} arm {arm_name}: {error}", cell.cell_id)),
            )?,
        );
    }
    Ok(CellValidation {
        cell_id: cell.cell_id.clone(),
        cache_state: declared.cache_state,
        arms,
    })
}

/// Drives every arm of every declared cell of a saved runner plan once, untimed.
///
/// The smoke establishes the runner-to-arm wire in a working session: it reads
/// the plan and the family addendum the plan names, validates both exactly as
/// the timed `run` path validates them before its first measurement, and drives
/// each cell's baseline and candidate arm once in
/// [`PairPosition::Validation`] — the runner's own request type and encoder,
/// fresh-child sentinel, child environment, child dispatch and result parser.
///
/// It is non-timed by construction: the request carries no timing window, the
/// smoke takes no host lock, opens no family ledger, writes no receipt, and
/// emits no timing sample. It fails, naming the cell and the arm, when an arm
/// reports a timing window, exits other than cleanly, writes something other
/// than one canonical result line, applies another cache state or reports no
/// route provenance; a plan cell the addendum does not declare fails before any
/// arm runs.
///
/// This entry point writes nothing.
/// `benchmark-ab-runner smoke <plan> --stage <dir>` drives the same validation
/// dispatches through the runner's session
/// loop, append-only execution log and checkpoints, pausing at the plan's
/// cells-per-session budget and resuming without repeating a completed cell;
/// such a stage carries zero timing samples and `finalize` refuses it.
///
/// `root` is the repository root the plan's relative paths resolve against.
///
/// Each dispatch makes this process a child subreaper that reaps adopted
/// children ([`crate::process::run_process`]), so a test binary calling the
/// smoke must not also spawn and wait on unrelated children: those belong in
/// their own test binary.
pub fn smoke(root: &Path, plan_path: &Path) -> io::Result<SmokeRecord> {
    let plan_bytes = std::fs::read(plan_path)?;
    let plan = RunnerPlan::decode(&plan_bytes).map_err(invalid)?;
    let addendum_bytes = std::fs::read(root.join(&plan.addendum))?;
    let addendum = FamilyAddendum::decode(&addendum_bytes).map_err(invalid)?;
    addendum
        .validate()
        .map_err(|errors| invalid(format!("addendum invalid: {}", errors.join("; "))))?;
    plan.validate(&addendum)
        .map_err(|errors| invalid(format!("plan invalid: {}", errors.join("; "))))?;
    let timeout = Duration::from_secs(plan.settings().0.child_timeout_seconds);
    let mut cells = Vec::with_capacity(plan.cells.len());
    for cell in &plan.cells {
        cells.push(validate_cell(
            root,
            &plan,
            &addendum,
            cell,
            timeout,
            &mut (),
        )?);
    }
    Ok(SmokeRecord::of(
        &plan,
        &plan_bytes,
        &addendum,
        &addendum_bytes,
        cells,
    ))
}

fn resolve(root: &Path, executable: &str) -> io::Result<PathBuf> {
    let path = Path::new(executable);
    std::fs::canonicalize(if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    })
}

/// The path as the repository sees it, so a record carries no checkout
/// location. An executable outside the repository keeps its canonical path.
fn repository_relative(root: &Path, executable: &Path) -> String {
    executable
        .strip_prefix(root)
        .unwrap_or(executable)
        .to_string_lossy()
        .into_owned()
}
