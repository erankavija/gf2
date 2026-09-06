//! Benchmark receipt schema and the independent acceptance evaluation.
//!
//! A receipt is the durable evidence of one bounded campaign: pinned
//! protocol, contract and addendum identities, source and build identities,
//! runtime host observation, lock evidence, the append-only execution log,
//! checkpoints, every arm's raw timing windows, and the runner's own claims.
//! [`evaluate`] recomputes every digest, statistic and decision from those raw
//! parts and never trusts a claim; its summary is the acceptance record.

use crate::abtest::{
    bonferroni_confidence, bootstrap_seed, decide, flagged_windows, median, pair_orders,
    paired_bootstrap_speedup, wilson_interval_95, ArmOrder, BootstrapInterval, Decision, Margins,
    PairedObservation,
};
use crate::host::{CoreArm, HostObservation};
use crate::journal::{ExecutionLog, JournalEvent, JournalRecord};
use crate::protocol::{
    is_hex, sha256_hex, ArtifactPin, BuildIdentity, CacheState, CellObjective, CellRole,
    DecoderArmKind, FamilyAddendum, Normalization, Precision, ReceiptLabel, Schedule,
    SharedSettings, Stopping, ACCEPTANCE_SCHEMA_ID, ADDENDUM_SCHEMA_PATH, CONTRACT_PATH,
    PROTOCOL_PATH, RECEIPT_SCHEMA_ID, SHARED_SETTINGS,
};
use crate::provenance::{ProducingInputs, ProducingSnapshot};
use crate::schema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

/// File name of the receipt inside its directory.
pub const RECEIPT_FILE: &str = "receipt.json";
/// File name of the machine-readable acceptance summary.
pub const SUMMARY_JSON_FILE: &str = "acceptance-summary.json";
/// File name of the Markdown interpretation.
pub const SUMMARY_MARKDOWN_FILE: &str = "acceptance-summary.md";
/// Execution log copied beside the receipt.
pub const LOG_FILE: &str = "execution.log";
/// Checkpoint directory copied beside the receipt.
pub const CHECKPOINT_DIR: &str = "checkpoints";

/// Source identity at measurement time.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    /// Informational source-control locator; never an acceptance key.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub revision: String,
    /// Historical repository-tree digest; informational for old receipts.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tree_sha256: String,
    /// Historical whole-worktree observation; informational for old receipts.
    #[serde(default, skip_serializing_if = "is_false")]
    pub clean: bool,
    /// Canonical content closure of producing code and build inputs.
    #[serde(default)]
    pub producing: Option<ProducingSnapshot>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Lock evidence observed inside the wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockRecord {
    pub lock_path: String,
    pub holder_pid: u32,
    pub observation: String,
    /// Wrapper script the launcher used, repository-relative.
    pub wrapper: String,
}

/// Worker report of the runner process.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerReport {
    pub environment_rayon_threads: Option<String>,
    pub runner_threads: u32,
}

/// Execution log identity beside the receipt.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogRecord {
    pub path: String,
    pub sha256: String,
    pub sessions: u32,
    pub resumed: bool,
}

/// Checkpoint manifest identity beside the receipt.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointRecord {
    pub manifest_path: String,
    pub manifest_sha256: String,
}

/// Tuning profile an arm selected.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuningProfilePin {
    pub id: String,
    pub sha256: String,
}

/// One arm's build and launch identity.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArmRecord {
    pub build: BuildIdentity,
    pub description: String,
    pub executable_path: String,
    pub executable_sha256: String,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub rustflags: Option<String>,
    pub tuning_profile: Option<TuningProfilePin>,
}

/// One untrimmed timing window reported by an arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WindowRecord {
    pub calls: u64,
    pub elapsed_ns: u64,
}

impl WindowRecord {
    /// Nanoseconds per logical call.
    pub fn ns_per_call(self) -> f64 {
        self.elapsed_ns as f64 / self.calls as f64
    }
}

/// Setup and conversion costs an arm reports separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionCosts {
    pub setup_ns: u64,
    pub pack_ns: u64,
    pub unpack_ns: u64,
    pub batch_fill_ns: u64,
    pub dispatch_ns: u64,
}

/// Decoder settings an arm reports it actually used.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderArmSettings {
    pub precision: Precision,
    pub schedule: Schedule,
    pub normalization: Normalization,
    pub iteration_cap: u32,
    pub stopping: Stopping,
    pub batch_size: u32,
}

/// Iteration distribution over decoded frames.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IterationDistribution {
    pub mean: f64,
    pub p50: u32,
    pub p90: u32,
    pub max: u32,
}

/// Quality an arm measured on the declared input.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArmQuality {
    pub frames: u64,
    pub frame_errors: u64,
    pub bits: u64,
    pub bit_errors: u64,
    pub fer: f64,
    pub fer_interval: [f64; 2],
    pub ber: f64,
    pub ber_interval: [f64; 2],
    /// Interval method identifier; version 1 fixes `wilson-95`.
    pub interval_method: String,
    pub iterations: IterationDistribution,
    pub memory_bytes: u64,
    pub latency_ns_p50: u64,
    pub settings: DecoderArmSettings,
}

/// One fresh execution of an arm.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRecord {
    pub arm: String,
    pub pid: u32,
    pub windows: Vec<WindowRecord>,
    /// Median of the windows' nanoseconds per call.
    pub ns_per_call: f64,
    pub cache_state_applied: CacheState,
    pub workers_observed: u32,
    /// CPU affinity the arm observed for itself at runtime.
    pub cpus_observed: Vec<u32>,
    pub selected_path: Option<String>,
    pub conversion: Option<ConversionCosts>,
    pub elapsed_ns: u64,
}

/// One paired execution.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairRecord {
    pub index: u32,
    pub order: ArmOrder,
    pub baseline: ExecutionRecord,
    pub candidate: ExecutionRecord,
}

/// Whether a cell produced samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CellStatus {
    Measured,
    Unavailable,
}

/// The runner's own statistical claim for a cell.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellClaim {
    pub interval: BootstrapInterval,
    pub decision: Decision,
    pub margins: Margins,
}

/// Decoder quality of both arms.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderQualityRecord {
    pub baseline: ArmQuality,
    pub candidate: ArmQuality,
}

/// One cell of the receipt.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellRecord {
    pub cell_id: String,
    /// Checkpoint key; unique per campaign.
    pub key: String,
    pub role: CellRole,
    pub baseline_arm: String,
    pub candidate_arm: String,
    pub core_arm: CoreArm,
    pub resolved_cpus: Vec<u32>,
    pub status: CellStatus,
    pub unavailable_reason: Option<String>,
    pub pairs: Vec<PairRecord>,
    pub claimed: Option<CellClaim>,
    pub decoder_quality: Option<DecoderQualityRecord>,
    pub checkpoint_sha256: Option<String>,
}

/// The receipt.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkReceipt {
    pub schema: String,
    pub campaign_id: String,
    pub issue: String,
    /// Repository-relative publication path, used to reject self-references.
    #[serde(default)]
    pub receipt_path: String,
    pub label: ReceiptLabel,
    pub campaign_seed: u64,
    pub settings: SharedSettings,
    /// True when the runner ran with a timing override; such cells are never
    /// confirmatory.
    pub settings_deviation: bool,
    pub protocol: ArtifactPin,
    pub contract: ArtifactPin,
    pub addendum_schema: ArtifactPin,
    pub addendum: ArtifactPin,
    pub source: SourceIdentity,
    pub toolchain: String,
    pub host: HostObservation,
    pub lock: LockRecord,
    pub workers: WorkerReport,
    pub execution_log: LogRecord,
    pub checkpoints: CheckpointRecord,
    pub arms: BTreeMap<String, ArmRecord>,
    pub cells: Vec<CellRecord>,
}

impl BenchmarkReceipt {
    /// Decodes a receipt strictly.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|error| format!("receipt does not decode: {error}"))
    }
}

/// Severity of a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// The receipt is rejected.
    Error,
    /// Recorded; does not reject.
    Note,
}

/// One acceptance finding.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// Protocol rule identifier, `P-NN`.
    pub rule: String,
    pub severity: Severity,
    pub cell: Option<String>,
    pub message: String,
}

/// Outcome of one cell after independent evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CellOutcome {
    /// Confidence-bound improvement (or not-worse for a non-regression cell).
    Pass,
    /// Not worse, yet short of the worthwhile margin: a retained no-win.
    NotMaterial,
    /// Regressed beyond the equivalence margin.
    Fail,
    /// The interval spans a margin.
    Inconclusive,
    /// Flagged-window fraction above the frozen bound; re-run required.
    Unstable,
    /// Inapplicable arm or missing comparator with a recorded reason.
    Unavailable,
    /// Exploratory data; informative only.
    Pilot,
    /// A required setting is unresolved or shared settings deviate.
    NotConfirmatory,
    /// Candidate quality outside the predeclared tolerance.
    QualityIncompatible,
    /// A structural finding invalidates the cell.
    Invalid,
}

/// Per-cell acceptance record.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellVerdict {
    pub cell_id: String,
    pub role: CellRole,
    pub objective: Option<CellObjective>,
    pub core_arm: CoreArm,
    pub resolved_cpus: Vec<u32>,
    pub status: CellStatus,
    pub outcome: CellOutcome,
    pub pairs: usize,
    pub flagged_windows: usize,
    pub total_windows: usize,
    pub interval: Option<BootstrapInterval>,
    pub decision: Option<Decision>,
    pub margins: Option<Margins>,
    pub unresolved_settings: Vec<String>,
    pub reason: Option<String>,
}

/// Family-level statistics of the evaluation.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FamilySummary {
    pub family_id: String,
    pub comparisons: u32,
    pub family_alpha: f64,
    pub per_comparison_confidence: f64,
    pub bootstrap_resamples: u32,
}

/// Verdict of the whole receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Accepted,
    Rejected,
}

/// The machine-readable acceptance summary.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceSummary {
    pub schema: String,
    pub receipt_sha256: String,
    pub campaign_id: String,
    pub label: ReceiptLabel,
    pub verdict: Verdict,
    /// True only when every non-exploratory cell passed.
    pub qualifies: bool,
    pub family: Option<FamilySummary>,
    pub sessions: u32,
    pub resumed: bool,
    pub findings: Vec<Finding>,
    pub cells: Vec<CellVerdict>,
}

struct Evaluation {
    findings: Vec<Finding>,
}

impl Evaluation {
    fn error(&mut self, rule: &str, cell: Option<&str>, message: impl Into<String>) {
        self.findings.push(Finding {
            rule: rule.into(),
            severity: Severity::Error,
            cell: cell.map(str::to_owned),
            message: message.into(),
        });
    }

    fn note(&mut self, rule: &str, cell: Option<&str>, message: impl Into<String>) {
        self.findings.push(Finding {
            rule: rule.into(),
            severity: Severity::Note,
            cell: cell.map(str::to_owned),
            message: message.into(),
        });
    }
}

/// Evaluates a self-contained receipt directory from its durable snapshots.
pub fn evaluate(receipt_dir: &Path) -> io::Result<AcceptanceSummary> {
    let receipt_bytes = fs::read(receipt_dir.join(RECEIPT_FILE))?;
    let receipt_sha256 = sha256_hex(&receipt_bytes);
    let receipt = BenchmarkReceipt::decode(&receipt_bytes)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
    let mut evaluation = Evaluation {
        findings: Vec::new(),
    };
    let e = &mut evaluation;

    if receipt.schema != RECEIPT_SCHEMA_ID {
        e.error(
            "P-01",
            None,
            format!(
                "receipt schema {:?} is not {RECEIPT_SCHEMA_ID}",
                receipt.schema
            ),
        );
    }
    let mut verified_pins: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    for (rule, name, pin, expected_path) in [
        ("P-02", "protocol", &receipt.protocol, Some(PROTOCOL_PATH)),
        ("P-02", "contract", &receipt.contract, Some(CONTRACT_PATH)),
        (
            "P-02",
            "addendum schema",
            &receipt.addendum_schema,
            Some(ADDENDUM_SCHEMA_PATH),
        ),
        ("P-03", "addendum", &receipt.addendum, None),
    ] {
        if let Err(message) = pin.validate_shape() {
            e.error(rule, None, format!("{name} identity: {message}"));
        }
        if let Some(expected) = expected_path {
            if pin.path != expected {
                e.error(
                    rule,
                    None,
                    format!("{name} pin names {:?}, expected {expected:?}", pin.path),
                );
            }
        }
        match pin.verify_content(receipt_dir) {
            Err(message) => e.error(rule, None, format!("{name} identity: {message}")),
            Ok(bytes) => {
                verified_pins.insert(name, bytes);
            }
        }
    }
    if receipt.settings != SHARED_SETTINGS && !receipt.settings_deviation {
        e.error(
            "P-04",
            None,
            "receipt settings differ from the frozen shared settings without declaring a deviation",
        );
    }
    if receipt.settings_deviation {
        e.note(
            "P-04",
            None,
            "settings deviation declared: no cell is confirmatory",
        );
    }

    // Decode the schema and addendum only from the exact pinned bytes already
    // authenticated above.
    let addendum = match verified_pins.remove("addendum") {
        Some(bytes) => match FamilyAddendum::decode(&bytes) {
            Ok(addendum) => {
                if let Err(errors) = addendum.validate() {
                    for message in errors {
                        e.error("P-03", None, format!("addendum: {message}"));
                    }
                }
                if let Some(schema_bytes) = verified_pins.get("addendum schema") {
                    match (
                        serde_json::from_slice::<Value>(schema_bytes),
                        serde_json::from_slice::<Value>(&bytes),
                    ) {
                        (Ok(schema_value), Ok(instance)) => {
                            for violation in schema::validate(&schema_value, &instance) {
                                e.error(
                                    "P-03",
                                    None,
                                    format!(
                                        "addendum schema violation at {}: {}",
                                        violation.instance_path, violation.message
                                    ),
                                );
                            }
                        }
                        _ => e.error("P-03", None, "addendum schema or addendum is not JSON"),
                    }
                } else {
                    e.error(
                        "P-03",
                        None,
                        "addendum schema snapshot is missing from the receipt",
                    );
                }
                Some(addendum)
            }
            Err(message) => {
                e.error("P-03", None, message);
                None
            }
        },
        None => {
            e.error(
                "P-03",
                None,
                "addendum not decoded because its pinned bytes were not verified",
            );
            None
        }
    };

    if receipt.toolchain.trim().is_empty() {
        e.error("P-05", None, "toolchain identity is empty");
    }
    match &receipt.source.producing {
        Some(snapshot) => {
            if let Err(error) =
                ProducingInputs::verify_snapshot(&receipt_dir.join("inputs/producing"), snapshot)
            {
                e.error("P-05", None, format!("producing inputs: {error}"));
            }
        }
        None => e.error("P-05", None, "receipt lacks a producing-input snapshot"),
    }
    if let Some(addendum) = &addendum {
        verify_resolution_evidence(
            e,
            receipt_dir,
            &receipt.receipt_path,
            &receipt_sha256,
            addendum,
        );
    }
    let host = &receipt.host;
    if host.cpu_model.trim().is_empty()
        || host.cpu_flags.is_empty()
        || host.governors.is_empty()
        || host.affinity.cpus().is_empty()
        || host.topology.cpus.is_empty()
        || host.os_kernel.trim().is_empty()
    {
        e.error("P-06", None, "host observation is incomplete");
    }
    if !receipt.lock.lock_path.starts_with('/')
        || receipt.lock.holder_pid == 0
        || receipt.lock.observation.trim().is_empty()
        || receipt.lock.wrapper.trim().is_empty()
    {
        e.error("P-07", None, "lock evidence is incomplete");
    }
    if receipt.workers.runner_threads == 0 {
        e.error("P-08", None, "worker report lacks the runner thread count");
    }
    for (name, arm) in &receipt.arms {
        if !is_hex(&arm.executable_sha256, 64) || arm.executable_path.is_empty() {
            e.error(
                "P-09",
                None,
                format!("arm {name} lacks an executable identity"),
            );
        }
    }

    // Execution log and sessions.
    let mut announced_before_first_cell = false;
    let mut sessions = 0u32;
    let mut terminal = None;
    let mut cell_starts: BTreeMap<String, usize> = BTreeMap::new();
    let mut cell_completes: BTreeMap<String, usize> = BTreeMap::new();
    let log_path = receipt_dir.join(&receipt.execution_log.path);
    match fs::read(&log_path) {
        Ok(bytes) => {
            if sha256_hex(&bytes) != receipt.execution_log.sha256 {
                e.error(
                    "P-10",
                    None,
                    "execution log digest differs from the receipt",
                );
            }
            match ExecutionLog::validate_prefix(&bytes, &receipt.campaign_id) {
                Ok(records) => {
                    let frozen_addendum = records.first().and_then(|record| {
                        record
                            .details
                            .get("addendum")
                            .cloned()
                            .and_then(|value| serde_json::from_value::<ArtifactPin>(value).ok())
                    });
                    if frozen_addendum.as_ref() != Some(&receipt.addendum) {
                        e.error(
                            "P-03",
                            None,
                            "campaign-start does not freeze the receipt addendum content pin",
                        );
                    }
                    let mut seen_cell = false;
                    for record in &records {
                        match record.event {
                            JournalEvent::CampaignStart | JournalEvent::SessionStart => {
                                sessions += 1
                            }
                            JournalEvent::OrchestrationStart if is_announcement(record) => {
                                if !seen_cell {
                                    announced_before_first_cell = true;
                                }
                            }
                            JournalEvent::CellStart => {
                                seen_cell = true;
                                *cell_starts.entry(record_key(record)).or_default() += 1;
                            }
                            JournalEvent::CellComplete => {
                                *cell_completes.entry(record_key(record)).or_default() += 1;
                            }
                            JournalEvent::Complete
                            | JournalEvent::Failed
                            | JournalEvent::Paused
                            | JournalEvent::BudgetExhausted => terminal = Some(record.event),
                            _ => {}
                        }
                    }
                    if records.first().map(|record| record.event)
                        != Some(JournalEvent::CampaignStart)
                    {
                        e.error(
                            "P-10",
                            None,
                            "execution log does not open with campaign-start",
                        );
                    }
                }
                Err(error) => e.error("P-10", None, format!("execution log invalid: {error}")),
            }
        }
        Err(error) => e.error("P-10", None, format!("execution log unreadable: {error}")),
    }
    if !announced_before_first_cell {
        e.error(
            "P-10",
            None,
            "execution log path was not announced before the first bounded run",
        );
    }
    if sessions != receipt.execution_log.sessions {
        e.error(
            "P-11",
            None,
            format!(
                "receipt claims {} sessions, log holds {sessions}",
                receipt.execution_log.sessions
            ),
        );
    }
    if receipt.execution_log.resumed != (sessions > 1) {
        e.error(
            "P-11",
            None,
            "resumed flag disagrees with the session count",
        );
    }
    if terminal != Some(JournalEvent::Complete) {
        e.error(
            "P-11",
            None,
            format!("execution log terminal state is {terminal:?}, not complete"),
        );
    }
    for (key, count) in &cell_starts {
        if *count != 1 {
            e.error(
                "P-11",
                Some(key),
                format!("cell started {count} times; completed cells must not repeat"),
            );
        }
    }
    for (key, count) in &cell_completes {
        if *count != 1 {
            e.error("P-11", Some(key), format!("cell completed {count} times"));
        }
    }

    // Checkpoints.
    let manifest_path = receipt_dir.join(&receipt.checkpoints.manifest_path);
    match fs::read(&manifest_path) {
        Ok(bytes) => {
            if sha256_hex(&bytes) != receipt.checkpoints.manifest_sha256 {
                e.error(
                    "P-12",
                    None,
                    "checkpoint manifest digest differs from the receipt",
                );
            }
        }
        Err(error) => e.error(
            "P-12",
            None,
            format!("checkpoint manifest unreadable: {error}"),
        ),
    }
    let units_dir = manifest_path
        .parent()
        .map(|parent| parent.join("units"))
        .unwrap_or_else(|| receipt_dir.join(CHECKPOINT_DIR).join("units"));

    // Cells.
    let family = addendum.as_ref().map(|addendum| {
        let comparisons = addendum.family_comparisons();
        let confidence = bonferroni_confidence(receipt.settings.family_alpha, comparisons)
            .unwrap_or(1.0 - receipt.settings.family_alpha);
        FamilySummary {
            family_id: addendum.family.id.clone(),
            comparisons,
            family_alpha: receipt.settings.family_alpha,
            per_comparison_confidence: confidence,
            bootstrap_resamples: receipt.settings.bootstrap_resamples,
        }
    });
    let mut verdicts = Vec::new();
    let mut seen_cells = BTreeSet::new();
    for cell in &receipt.cells {
        let id = cell.cell_id.as_str();
        if !seen_cells.insert(id.to_owned()) {
            e.error("P-13", Some(id), "cell recorded more than once");
        }
        let declaration = addendum.as_ref().and_then(|addendum| addendum.cell(id));
        if addendum.is_some() && declaration.is_none() {
            e.error("P-13", Some(id), "cell is not declared in the addendum");
        }
        let mut verdict = CellVerdict {
            cell_id: id.to_owned(),
            role: cell.role,
            objective: declaration.map(|declared| declared.objective),
            core_arm: cell.core_arm,
            resolved_cpus: cell.resolved_cpus.clone(),
            status: cell.status,
            outcome: CellOutcome::Invalid,
            pairs: cell.pairs.len(),
            flagged_windows: 0,
            total_windows: 0,
            interval: None,
            decision: None,
            margins: None,
            unresolved_settings: Vec::new(),
            reason: cell.unavailable_reason.clone(),
        };
        let mut invalid = false;
        if let Some(declared) = declaration {
            if declared.role != cell.role {
                e.error(
                    "P-13",
                    Some(id),
                    format!(
                        "receipt role {:?} differs from the declared {:?}",
                        cell.role, declared.role
                    ),
                );
                invalid = true;
            }
            if declared.core_arm != cell.core_arm {
                e.error("P-13", Some(id), "core arm differs from the declaration");
                invalid = true;
            }
        }
        if !receipt.arms.contains_key(&cell.baseline_arm)
            || !receipt.arms.contains_key(&cell.candidate_arm)
        {
            e.error(
                "P-09",
                Some(id),
                "cell names an arm the receipt does not describe",
            );
            invalid = true;
        }
        if cell_starts.get(&cell.key).copied().unwrap_or(0) != 1
            && cell.status == CellStatus::Measured
        {
            e.error(
                "P-11",
                Some(id),
                "measured cell has no single cell-start record",
            );
            invalid = true;
        }
        match cell.status {
            CellStatus::Unavailable => {
                if cell
                    .unavailable_reason
                    .as_deref()
                    .is_none_or(|reason| reason.trim().is_empty())
                {
                    e.error("P-14", Some(id), "unavailable cell carries no reason");
                    invalid = true;
                }
                if !cell.pairs.is_empty() || cell.claimed.is_some() {
                    e.error(
                        "P-14",
                        Some(id),
                        "unavailable cell carries samples or a claim",
                    );
                    invalid = true;
                }
                if !cell.resolved_cpus.is_empty() {
                    e.error("P-14", Some(id), "unavailable cell resolved CPUs");
                    invalid = true;
                }
                verdict.outcome = if invalid {
                    CellOutcome::Invalid
                } else {
                    CellOutcome::Unavailable
                };
                verdicts.push(verdict);
                continue;
            }
            CellStatus::Measured => {}
        }
        if cell.resolved_cpus.is_empty() {
            e.error(
                "P-14",
                Some(id),
                "measured cell lacks runtime-resolved CPU IDs",
            );
            invalid = true;
        }
        // Checkpoint unit.
        match &cell.checkpoint_sha256 {
            Some(expected) => {
                let unit_path = units_dir.join(format!("{}.json", sha256_hex(cell.key.as_bytes())));
                match fs::read(&unit_path) {
                    Ok(bytes) if sha256_hex(&bytes) == *expected => {
                        if let Ok(unit) = serde_json::from_slice::<Value>(&bytes) {
                            if unit.get("key").and_then(Value::as_str) != Some(cell.key.as_str()) {
                                e.error(
                                    "P-12",
                                    Some(id),
                                    "checkpoint unit key differs from the cell key",
                                );
                                invalid = true;
                            }
                        }
                    }
                    Ok(_) => {
                        e.error(
                            "P-12",
                            Some(id),
                            "checkpoint unit digest differs from the receipt",
                        );
                        invalid = true;
                    }
                    Err(error) => {
                        e.error(
                            "P-12",
                            Some(id),
                            format!("checkpoint unit unreadable: {error}"),
                        );
                        invalid = true;
                    }
                }
            }
            None => {
                e.error("P-12", Some(id), "measured cell has no checkpoint digest");
                invalid = true;
            }
        }
        // Sample budget and structure.
        let settings = &receipt.settings;
        let required_pairs = match cell.role {
            CellRole::Confirmatory | CellRole::Holdout => {
                Some(settings.confirmatory_pairs as usize)
            }
            CellRole::Exploratory => None,
        };
        match required_pairs {
            Some(required) if cell.pairs.len() != required => {
                e.error(
                    "P-15",
                    Some(id),
                    format!(
                        "{} pairs recorded, {required} required for a {:?} cell",
                        cell.pairs.len(),
                        cell.role
                    ),
                );
                invalid = true;
            }
            None if cell.pairs.len() < settings.pilot_min_pairs as usize
                || cell.pairs.len() > settings.pilot_max_pairs as usize =>
            {
                e.error(
                    "P-15",
                    Some(id),
                    format!(
                        "{} pairs recorded, pilots use {}..={}",
                        cell.pairs.len(),
                        settings.pilot_min_pairs,
                        settings.pilot_max_pairs
                    ),
                );
                invalid = true;
            }
            _ => {}
        }
        let cell_seed = bootstrap_seed(receipt.campaign_seed, &cell.key);
        let expected_orders = pair_orders(cell_seed, cell.pairs.len());
        let mut observations = Vec::with_capacity(cell.pairs.len());
        let mut all_windows = Vec::new();
        for (index, pair) in cell.pairs.iter().enumerate() {
            if pair.index as usize != index {
                e.error(
                    "P-15",
                    Some(id),
                    format!("pair {index} carries index {}", pair.index),
                );
                invalid = true;
            }
            if expected_orders.get(index) != Some(&pair.order) {
                e.error(
                    "P-16",
                    Some(id),
                    format!("pair {index} order is not the seed-determined counterbalanced order"),
                );
                invalid = true;
            }
            for (name, execution, arm) in [
                ("baseline", &pair.baseline, &cell.baseline_arm),
                ("candidate", &pair.candidate, &cell.candidate_arm),
            ] {
                if &execution.arm != arm {
                    e.error(
                        "P-15",
                        Some(id),
                        format!(
                            "pair {index} {name} execution names arm {:?}",
                            execution.arm
                        ),
                    );
                    invalid = true;
                }
                if execution.windows.len() != settings.windows_per_execution as usize {
                    e.error(
                        "P-15",
                        Some(id),
                        format!(
                            "pair {index} {name} has {} windows, {} required",
                            execution.windows.len(),
                            settings.windows_per_execution
                        ),
                    );
                    invalid = true;
                }
                if execution
                    .windows
                    .iter()
                    .any(|window| window.calls == 0 || window.elapsed_ns == 0)
                {
                    e.error(
                        "P-15",
                        Some(id),
                        format!("pair {index} {name} holds a zero window"),
                    );
                    invalid = true;
                    continue;
                }
                let mut values: Vec<f64> = execution
                    .windows
                    .iter()
                    .map(|window| window.ns_per_call())
                    .collect();
                all_windows.extend(values.iter().copied());
                match median(&mut values) {
                    Ok(center)
                        if (center - execution.ns_per_call).abs() <= 1e-9 * center.max(1.0) => {}
                    _ => {
                        e.error(
                            "P-15",
                            Some(id),
                            format!(
                                "pair {index} {name} ns_per_call is not the median of its windows"
                            ),
                        );
                        invalid = true;
                    }
                }
                if execution.cpus_observed != cell.resolved_cpus {
                    e.error(
                        "P-17",
                        Some(id),
                        format!(
                            "pair {index} {name} observed CPUs {:?}, resolved {:?}",
                            execution.cpus_observed, cell.resolved_cpus
                        ),
                    );
                    invalid = true;
                }
                if let Some(declared) = declaration {
                    if execution.cache_state_applied != declared.cache_state {
                        e.error(
                            "P-17",
                            Some(id),
                            format!(
                                "pair {index} {name} applied cache state {:?}, declared {:?}",
                                execution.cache_state_applied, declared.cache_state
                            ),
                        );
                        invalid = true;
                    }
                    if execution.workers_observed != declared.workers.declared {
                        e.error(
                            "P-17",
                            Some(id),
                            format!(
                                "pair {index} {name} observed {} workers, declared {}",
                                execution.workers_observed, declared.workers.declared
                            ),
                        );
                        invalid = true;
                    }
                    if declared.conversion_costs_included && execution.conversion.is_none() {
                        e.error(
                            "P-17",
                            Some(id),
                            format!("pair {index} {name} reports no conversion costs"),
                        );
                        invalid = true;
                    }
                }
            }
            observations.push(PairedObservation {
                baseline_ns_per_call: pair.baseline.ns_per_call,
                candidate_ns_per_call: pair.candidate.ns_per_call,
            });
        }
        verdict.total_windows = all_windows.len();
        if !all_windows.is_empty() {
            verdict.flagged_windows =
                flagged_windows(&all_windows, settings.flagged_window_factor).unwrap_or(0);
        }
        // Decoder quality.
        let mut quality_incompatible = false;
        if let Some(declared) = declaration {
            if let Some(decoder) = &declared.decoder {
                match &cell.decoder_quality {
                    None => {
                        e.error("P-18", Some(id), "decoder cell reports no BER/FER evidence");
                        invalid = true;
                    }
                    Some(quality) => {
                        for (name, arm) in [
                            ("baseline", &quality.baseline),
                            ("candidate", &quality.candidate),
                        ] {
                            if arm.interval_method != "wilson-95" {
                                e.error(
                                    "P-18",
                                    Some(id),
                                    format!(
                                        "{name} interval method {:?} is not wilson-95",
                                        arm.interval_method
                                    ),
                                );
                                invalid = true;
                                continue;
                            }
                            match (
                                wilson_interval_95(arm.frame_errors, arm.frames),
                                wilson_interval_95(arm.bit_errors, arm.bits),
                            ) {
                                (Ok(fer), Ok(ber)) => {
                                    if !close(fer.0, arm.fer_interval[0])
                                        || !close(fer.1, arm.fer_interval[1])
                                        || !close(ber.0, arm.ber_interval[0])
                                        || !close(ber.1, arm.ber_interval[1])
                                        || !close(
                                            arm.fer,
                                            arm.frame_errors as f64 / arm.frames as f64,
                                        )
                                    {
                                        e.error(
                                            "P-18",
                                            Some(id),
                                            format!("{name} BER/FER intervals do not recompute"),
                                        );
                                        invalid = true;
                                    }
                                }
                                _ => {
                                    e.error(
                                        "P-18",
                                        Some(id),
                                        format!("{name} BER/FER counts are invalid"),
                                    );
                                    invalid = true;
                                }
                            }
                            let expected = DecoderArmSettings {
                                precision: decoder.precision,
                                schedule: decoder.schedule,
                                normalization: decoder.normalization.clone(),
                                iteration_cap: decoder.iteration_cap,
                                stopping: decoder.stopping.clone(),
                                batch_size: decoder.batching.batch_size,
                            };
                            if matches!(decoder.arm_kind, DecoderArmKind::MatchedAlgorithm)
                                && arm.settings != expected
                            {
                                e.error("P-19", Some(id), format!("matched-algorithm {name} arm settings differ from the declaration"));
                                invalid = true;
                            }
                        }
                        if matches!(decoder.arm_kind, DecoderArmKind::FastestQualityCompatible)
                            && quality.candidate.fer_interval[1]
                                > decoder.quality_tolerance.fer_ratio_max
                                    * quality.baseline.fer_interval[1]
                        {
                            quality_incompatible = true;
                            e.note(
                                "P-19",
                                Some(id),
                                "candidate FER upper bound exceeds the predeclared tolerance",
                            );
                        }
                    }
                }
            }
        }
        if invalid {
            verdict.outcome = CellOutcome::Invalid;
            verdicts.push(verdict);
            continue;
        }
        // Statistics.
        let confidence = family
            .as_ref()
            .map(|family| family.per_comparison_confidence)
            .unwrap_or(1.0 - settings.family_alpha);
        let interval = match paired_bootstrap_speedup(
            &observations,
            settings.bootstrap_resamples,
            confidence,
            cell_seed,
        ) {
            Ok(interval) => interval,
            Err(error) => {
                e.error("P-20", Some(id), format!("bootstrap failed: {error}"));
                verdict.outcome = CellOutcome::Invalid;
                verdicts.push(verdict);
                continue;
            }
        };
        verdict.interval = Some(interval);
        let margins = addendum
            .as_ref()
            .zip(declaration)
            .and_then(|(addendum, declared)| addendum.margins(declared));
        verdict.margins = margins;
        let unresolved = addendum
            .as_ref()
            .zip(declaration)
            .map(|(addendum, declared)| addendum.unresolved_settings(declared))
            .unwrap_or_default();
        verdict.unresolved_settings = unresolved.clone();
        let decision = margins.and_then(|margins| decide(&interval, &margins).ok());
        verdict.decision = decision;
        if let Some(claim) = &cell.claimed {
            let same = close(claim.interval.lower, interval.lower)
                && close(claim.interval.upper, interval.upper)
                && close(claim.interval.estimate, interval.estimate)
                && claim.interval.resamples == interval.resamples
                && claim.interval.seed == interval.seed
                && close(claim.interval.confidence, interval.confidence);
            if !same {
                e.error(
                    "P-20",
                    Some(id),
                    "runner's claimed interval differs from the independent recomputation",
                );
                verdict.outcome = CellOutcome::Invalid;
                verdicts.push(verdict);
                continue;
            }
            if Some(claim.decision) != decision && decision.is_some() {
                e.error(
                    "P-20",
                    Some(id),
                    "runner's claimed decision differs from the independent decision",
                );
                verdict.outcome = CellOutcome::Invalid;
                verdicts.push(verdict);
                continue;
            }
        }
        let flagged_fraction = verdict.flagged_windows as f64 / verdict.total_windows.max(1) as f64;
        verdict.outcome = if matches!(cell.role, CellRole::Exploratory) {
            CellOutcome::Pilot
        } else if receipt.settings_deviation || !unresolved.is_empty() {
            CellOutcome::NotConfirmatory
        } else if flagged_fraction > settings.max_flagged_fraction {
            CellOutcome::Unstable
        } else if quality_incompatible {
            CellOutcome::QualityIncompatible
        } else {
            match (decision, declaration.map(|declared| declared.objective)) {
                (Some(Decision::Improved), _) => CellOutcome::Pass,
                (Some(Decision::NotWorse), Some(CellObjective::NonRegression)) => CellOutcome::Pass,
                (Some(Decision::NotWorse), _) => CellOutcome::NotMaterial,
                (Some(Decision::Regressed), _) => CellOutcome::Fail,
                (Some(Decision::Inconclusive), _) => CellOutcome::Inconclusive,
                (None, _) => CellOutcome::NotConfirmatory,
            }
        };
        verdicts.push(verdict);
    }
    if let Some(addendum) = &addendum {
        for declared in &addendum.cells {
            if !seen_cells.contains(&declared.cell_id) {
                e.error("P-21", Some(&declared.cell_id), "declared cell is absent from the receipt; negative and unavailable results must be recorded");
            }
        }
        for trial in &addendum.family_wise.prior_trials {
            let pin = ArtifactPin {
                path: trial.receipt.clone(),
                snapshot: format!("inputs/prior-trials/{}.json", trial.sha256),
                sha256: trial.sha256.clone(),
            };
            if let Err(message) = pin.verify_content(receipt_dir) {
                e.error(
                    "P-22",
                    None,
                    format!("prior trial {} snapshot invalid: {message}", trial.receipt),
                );
            }
        }
    }
    let verdict = if evaluation
        .findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Verdict::Rejected
    } else {
        Verdict::Accepted
    };
    let qualifies = verdict == Verdict::Accepted
        && !verdicts.is_empty()
        && verdicts
            .iter()
            .filter(|cell| !matches!(cell.role, CellRole::Exploratory))
            .all(|cell| cell.outcome == CellOutcome::Pass);
    Ok(AcceptanceSummary {
        schema: ACCEPTANCE_SCHEMA_ID.into(),
        receipt_sha256,
        campaign_id: receipt.campaign_id.clone(),
        label: receipt.label,
        verdict,
        qualifies,
        family,
        sessions,
        resumed: sessions > 1,
        findings: evaluation.findings,
        cells: verdicts,
    })
}

fn verify_resolution_evidence(
    evaluation: &mut Evaluation,
    receipt_dir: &Path,
    evaluated_receipt_path: &str,
    evaluated_receipt_sha256: &str,
    addendum: &FamilyAddendum,
) {
    if addendum.effect.measurement_resolution.is_none() {
        return;
    }
    let Some(evidence) = &addendum.effect.resolution_evidence else {
        // Structural validation emits the precise missing-field finding.
        return;
    };
    let evidence_path = Path::new(&evidence.receipt);
    if evidence_path.is_absolute()
        || evidence_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        evaluation.error(
            "P-03",
            None,
            "resolution evidence receipt is not repository-relative",
        );
        return;
    }
    if evidence.receipt == evaluated_receipt_path || evidence.sha256 == evaluated_receipt_sha256 {
        evaluation.error(
            "P-03",
            None,
            "resolution evidence names the receipt under evaluation",
        );
        return;
    }
    let pin = ArtifactPin {
        path: evidence.receipt.clone(),
        snapshot: "inputs/resolution-evidence/receipt.json".into(),
        sha256: evidence.sha256.clone(),
    };
    let bytes = match pin.verify_content(receipt_dir) {
        Ok(bytes) => bytes,
        Err(message) => {
            evaluation.error(
                "P-03",
                None,
                format!("resolution evidence is not a verified pilot snapshot: {message}"),
            );
            return;
        }
    };
    match BenchmarkReceipt::decode(&bytes) {
        Ok(pilot) if pilot.label == ReceiptLabel::Pilot => {}
        Ok(_) => evaluation.error(
            "P-03",
            None,
            "resolution evidence receipt is not labelled pilot",
        ),
        Err(message) => evaluation.error(
            "P-03",
            None,
            format!("resolution evidence receipt does not decode: {message}"),
        ),
    }
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

fn is_announcement(record: &JournalRecord) -> bool {
    record.details.get("kind").and_then(Value::as_str) == Some("execution-log-announced")
}

fn record_key(record: &JournalRecord) -> String {
    record
        .case
        .as_ref()
        .and_then(|case| case.get("key"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

/// Renders the Markdown interpretation from the summary alone.
pub fn render_markdown(summary: &AcceptanceSummary) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Acceptance summary for `{}`", summary.campaign_id);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Label `{:?}`; verdict **{:?}**; qualifies for production selection: **{}**; sessions {}; resumed {}.",
        summary.label, summary.verdict, summary.qualifies, summary.sessions, summary.resumed
    );
    let _ = writeln!(out, "Receipt digest `{}`.", summary.receipt_sha256);
    if summary.label == ReceiptLabel::Smoke {
        let _ = writeln!(out);
        let _ = writeln!(out, "This is a smoke receipt: it proves the receipt pipeline and claims no performance result.");
    }
    if let Some(family) = &summary.family {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "Family `{}`: {} comparisons at family-wise alpha {}, per-comparison confidence {:.6}, {} bootstrap resamples.",
            family.family_id, family.comparisons, family.family_alpha, family.per_comparison_confidence, family.bootstrap_resamples
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Cells");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |");
    let _ = writeln!(out, "|---|---|---|---|---:|---:|---:|---|---|---|---|");
    for cell in &summary.cells {
        let cpus = if cell.resolved_cpus.is_empty() {
            "none".to_owned()
        } else {
            cell.resolved_cpus
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        let (speedup, interval) = match &cell.interval {
            Some(interval) => (
                format!("{:.4}", interval.estimate),
                format!(
                    "[{:.4}, {:.4}] at {:.4}",
                    interval.lower, interval.upper, interval.confidence
                ),
            ),
            None => ("n/a".into(), "n/a".into()),
        };
        let decision = cell
            .decision
            .map_or("n/a".to_owned(), |decision| format!("{decision:?}"));
        let mut note = cell.reason.clone().unwrap_or_default();
        if !cell.unresolved_settings.is_empty() {
            note = format!("unresolved: {}", cell.unresolved_settings.join(", "));
        }
        let _ = writeln!(
            out,
            "| `{}` | {:?} | {:?} | {} | {} | {}/{} | {} | {} | {} | **{:?}** | {} |",
            cell.cell_id,
            cell.role,
            cell.core_arm,
            cpus,
            cell.pairs,
            cell.flagged_windows,
            cell.total_windows,
            speedup,
            interval,
            decision,
            cell.outcome,
            note
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Findings");
    let _ = writeln!(out);
    if summary.findings.is_empty() {
        let _ = writeln!(out, "No findings.");
    }
    for finding in &summary.findings {
        let cell = finding
            .cell
            .as_deref()
            .map_or(String::new(), |cell| format!(" `{cell}`"));
        let _ = writeln!(
            out,
            "- {} {:?}{}: {}",
            finding.rule, finding.severity, cell, finding.message
        );
    }
    out
}
