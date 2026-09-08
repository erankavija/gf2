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
    paired_bootstrap_speedup, paired_bootstrap_speedup_legacy, wilson_interval_95, ArmOrder,
    BootstrapInterval, Decision, Margins, PairedObservation,
};
use crate::host::{CoreArm, HostObservation};
use crate::journal::{
    CheckpointManifest, CheckpointStore, ExecutionLog, JournalEvent, JournalRecord, ResumeIdentity,
};
use crate::protocol::{
    is_hex, sha256_hex, ArtifactPin, BuildIdentity, CacheState, CellObjective, CellRole,
    DecoderArmKind, FamilyAddendum, Normalization, Precision, ReceiptLabel, RunnerPlan, Schedule,
    SharedSettings, Stopping, ACCEPTANCE_SCHEMA_ID, ADDENDUM_SCHEMA_PATH, CONTRACT_PATH,
    PROTOCOL_PATH, RECEIPT_SCHEMA_ID, RUNNER_LIFECYCLE_SCHEMA, SHARED_SETTINGS,
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
/// Exact runner plan copied beside the receipt.
pub const PLAN_FILE: &str = "plan.json";

/// Source identity at measurement time.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    /// Informational source-control locator; never an acceptance key.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub revision: String,
    /// Canonical content closure of producing code and build inputs.
    pub producing: ProducingSnapshot,
}

/// Complete semantic inputs frozen in the opening campaign journal record.
///
/// The runner uses the same type for cross-session comparison and the
/// independent evaluator uses it to bind every receipt field back to the facts
/// recorded before measurement. Source-control revision strings remain
/// informational and are excluded from semantic comparison.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignFacts {
    /// Versioned complete premeasurement host observation; only material fields bind resume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<HostObservation>,
    /// Versioned immutable ledger prefix including this campaign's reservation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trial_ledger: Option<ArtifactPin>,
    pub plan_sha256: String,
    pub identity: ResumeIdentity,
    pub protocol: ArtifactPin,
    pub contract: ArtifactPin,
    pub addendum_schema: ArtifactPin,
    pub addendum: ArtifactPin,
    pub source: SourceIdentity,
    pub toolchain: String,
    pub settings: SharedSettings,
    pub settings_deviation: bool,
    pub arms: BTreeMap<String, ArmRecord>,
}

/// Lock evidence observed inside the wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockRecord {
    pub lock_path: String,
    pub holder_pid: u32,
    pub observation: String,
    /// V1/v2 launcher label retained only for historical receipt decoding.
    #[serde(default, skip_serializing_if = "String::is_empty")]
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
    /// Versioned information-bit errors in each independent frame, in frozen input order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frame_bit_errors: Vec<u64>,
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
    /// Versioned decoder evidence retained from the actual child response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<ArmQuality>,
    /// Versioned protocol records whether the workload ran before its first timing window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated: Option<bool>,
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
    /// Versioned ordered session observations, also present in the execution journal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_hosts: Vec<HostObservation>,
    /// Versioned frozen reservation chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trial_ledger: Option<ArtifactPin>,
    pub schema: String,
    pub campaign_id: String,
    /// Canonical family identity, recorded in v3 pilot bytes for derivation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family_id: String,
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

impl CampaignFacts {
    /// Whether two opening records describe the same measurement behavior and
    /// inputs. Git locators are informational at both levels.
    pub fn resume_equivalent(&self, other: &Self) -> bool {
        match (&self.host, &other.host) {
            (Some(a), Some(b)) if a.material_equivalent(b) => {}
            (None, None) => {}
            _ => return false,
        }
        self.trial_ledger == other.trial_ledger
            && self.plan_sha256 == other.plan_sha256
            && self.identity.resume_equivalent(&other.identity)
            && self.protocol == other.protocol
            && self.contract == other.contract
            && self.addendum_schema == other.addendum_schema
            && self.addendum == other.addendum
            && self.source.producing == other.source.producing
            && self.toolchain == other.toolchain
            && self.settings == other.settings
            && self.settings_deviation == other.settings_deviation
            && self.arms == other.arms
    }

    /// Checks the complete receipt and saved-plan projection against the facts
    /// recorded before measurement.
    pub fn validate_receipt(
        &self,
        receipt: &BenchmarkReceipt,
        plan: &RunnerPlan,
        plan_bytes: &[u8],
        addendum: &FamilyAddendum,
        checkpoint: &CheckpointManifest,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        macro_rules! require {
            ($condition:expr, $message:literal) => {
                if !$condition {
                    errors.push($message.to_owned());
                }
            };
        }

        let projection = Self {
            host: (addendum.protocol.version >= 2).then(|| receipt.host.clone()),
            trial_ledger: receipt.trial_ledger.clone(),
            plan_sha256: sha256_hex(plan_bytes),
            identity: checkpoint.identity.clone(),
            protocol: receipt.protocol.clone(),
            contract: receipt.contract.clone(),
            addendum_schema: receipt.addendum_schema.clone(),
            addendum: receipt.addendum.clone(),
            source: receipt.source.clone(),
            toolchain: receipt.toolchain.clone(),
            settings: receipt.settings,
            settings_deviation: receipt.settings_deviation,
            arms: receipt.arms.clone(),
        };
        require!(
            self.resume_equivalent(&projection),
            "receipt and saved-plan facts differ from campaign-start"
        );

        require!(
            plan.campaign_id == receipt.campaign_id,
            "saved plan campaign differs from receipt"
        );
        require!(
            plan.issue == receipt.issue,
            "saved plan issue differs from receipt"
        );
        require!(
            plan.label == receipt.label,
            "saved plan label differs from receipt"
        );
        require!(
            plan.campaign_seed == receipt.campaign_seed,
            "saved plan seed differs from receipt"
        );
        require!(
            plan.addendum == self.addendum.path,
            "saved plan addendum path differs from campaign-start"
        );
        require!(
            plan.lock_path == receipt.lock.lock_path,
            "saved plan lock path differs from receipt lock evidence"
        );
        if addendum.protocol.version < 3 {
            require!(
                plan.wrapper == receipt.lock.wrapper,
                "saved plan wrapper differs from receipt lock evidence"
            );
        }
        let (plan_settings, plan_deviation) = plan.settings();
        require!(
            plan_settings == self.settings && plan_deviation == self.settings_deviation,
            "saved plan timing settings differ from campaign-start"
        );

        if let Err(plan_errors) = plan.validate(addendum) {
            errors.extend(
                plan_errors
                    .into_iter()
                    .map(|message| format!("saved plan is invalid: {message}")),
            );
        }
        if plan.arms.len() != self.arms.len() {
            errors.push("saved plan arm set differs from campaign-start".into());
        }
        for (name, planned) in &plan.arms {
            let Some(frozen) = self.arms.get(name) else {
                errors.push(format!(
                    "saved plan arm {name:?} is absent from campaign-start"
                ));
                continue;
            };
            let profile_matches = match (&planned.tuning_profile, &frozen.tuning_profile) {
                (None, None) => true,
                (Some(planned), Some(frozen)) => {
                    planned.id == frozen.id && planned.sha256 == frozen.sha256
                }
                _ => false,
            };
            if planned.build != frozen.build
                || planned.description != frozen.description
                || planned.executable != frozen.executable_path
                || planned.arguments != frozen.arguments
                || planned.environment != frozen.environment
                || planned.rustflags != frozen.rustflags
                || !profile_matches
            {
                errors.push(format!(
                    "saved plan arm {name:?} differs from its campaign-start descriptor"
                ));
            }
        }

        require!(
            checkpoint.campaign_id == receipt.campaign_id,
            "checkpoint campaign differs from receipt"
        );
        require!(
            checkpoint.identity.resume_equivalent(&self.identity),
            "checkpoint identity differs from campaign-start"
        );
        require!(
            self.identity.protocol_digest == self.protocol.sha256,
            "campaign-start protocol digest is inconsistent"
        );
        match self.source.producing.identity_sha256() {
            Ok(digest) => require!(
                self.identity.source_sha256 == digest,
                "campaign-start source digest is inconsistent"
            ),
            Err(error) => errors.push(format!(
                "campaign-start source identity cannot encode: {error}"
            )),
        }
        match serde_json::to_vec(&plan.cells) {
            Ok(bytes) => require!(
                self.identity.ordered_work_manifest_sha256 == sha256_hex(&bytes),
                "campaign-start work-manifest digest is inconsistent"
            ),
            Err(error) => errors.push(format!("saved plan cells cannot encode: {error}")),
        }
        match serde_json::to_vec(&self.arms) {
            Ok(bytes) => require!(
                self.identity.process_descriptors_sha256 == sha256_hex(&bytes),
                "campaign-start process-descriptor digest is inconsistent"
            ),
            Err(error) => errors.push(format!(
                "campaign-start arm descriptors cannot encode: {error}"
            )),
        }
        let executable_sha256 = self
            .arms
            .iter()
            .map(|(name, arm)| (name.clone(), arm.executable_sha256.clone()))
            .collect();
        require!(
            self.identity.executable_sha256 == executable_sha256,
            "campaign-start executable map is inconsistent"
        );
        require!(
            self.identity.behavior_sha256 == self.source.producing.behavior_sha256,
            "campaign-start behavior-source map is inconsistent"
        );
        match self.source.producing.lifecycle_sha256() {
            Ok(digest) => require!(
                self.identity.lifecycle_behavior_sha256 == digest,
                "campaign-start lifecycle digest is inconsistent"
            ),
            Err(error) => errors.push(format!(
                "campaign-start lifecycle identity cannot encode: {error}"
            )),
        }
        require!(
            self.identity.lifecycle_schema == RUNNER_LIFECYCLE_SCHEMA,
            "campaign-start lifecycle schema is inconsistent"
        );
        require!(
            self.identity.feature_contract == "release",
            "campaign-start feature contract is inconsistent"
        );
        let thread_contract = format!(
            "RAYON_NUM_THREADS={}",
            receipt
                .workers
                .environment_rayon_threads
                .as_deref()
                .unwrap_or("unset")
        );
        require!(
            self.identity.thread_contract == thread_contract,
            "campaign-start thread contract differs from runtime worker evidence"
        );
        match receipt.host.affinity.host_identity() {
            Ok(identity) => require!(
                self.identity.host_identity == identity,
                "campaign-start host identity differs from runtime host evidence"
            ),
            Err(error) => errors.push(format!("runtime host identity cannot encode: {error}")),
        }
        errors
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
    /// True only when at least one non-exploratory cell exists and all such cells passed.
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
    evaluate_version(receipt_dir, None)
}

/// Evaluates only under the receipt-pinned rules. An explicit expected version
/// rejects a differently pinned receipt; it cannot select different semantics.
pub fn evaluate_version(
    receipt_dir: &Path,
    expected_version: Option<u32>,
) -> io::Result<AcceptanceSummary> {
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

    let version = addendum.as_ref().map(|a| a.protocol.version).unwrap_or(0);
    if expected_version.is_some_and(|expected| expected != version) {
        e.error(
            "P-01",
            None,
            "requested protocol version differs from receipt pin",
        );
    }
    let marker = format!("Protocol `zen3-benchmark-protocol` version {version}.");
    if !verified_pins
        .get("protocol")
        .is_some_and(|bytes| String::from_utf8_lossy(bytes).contains(&marker))
    {
        e.error(
            "P-01",
            None,
            "pinned protocol document version differs from addendum",
        );
    }
    if receipt.toolchain.trim().is_empty() {
        e.error("P-05", None, "toolchain identity is empty");
    }
    if let Err(error) = ProducingInputs::verify_snapshot(
        &receipt_dir.join("inputs/producing"),
        &receipt.source.producing,
    ) {
        e.error("P-05", None, format!("producing inputs: {error}"));
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
    let saved_plan = match fs::read(receipt_dir.join(PLAN_FILE)) {
        Ok(bytes) => match RunnerPlan::decode(&bytes) {
            Ok(plan) => Some((plan, bytes)),
            Err(message) => {
                e.error("P-03", None, format!("saved plan: {message}"));
                None
            }
        },
        Err(error) => {
            e.error("P-03", None, format!("saved plan is unreadable: {error}"));
            None
        }
    };
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
    let lock_observation_is_complete = receipt.lock.lock_path.starts_with('/')
        && receipt.lock.holder_pid != 0
        && !receipt.lock.observation.trim().is_empty();
    let v3_lock_observation_is_complete = receipt.lock.observation
        == "inherited-fd-and-independent-flock-conflict";
    if !lock_observation_is_complete
        || (version >= 3 && !v3_lock_observation_is_complete)
        || (version < 3 && receipt.lock.wrapper.trim().is_empty())
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
    let mut cell_start_cases: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut cell_completes: BTreeMap<String, usize> = BTreeMap::new();
    let mut campaign_facts = None;
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
                    campaign_facts =
                        records.first().and_then(|record| {
                            match serde_json::from_value::<CampaignFacts>(record.details.clone()) {
                                Ok(facts) => Some(facts),
                                Err(error) => {
                                    e.error(
                                        "P-23",
                                        None,
                                        format!("campaign-start facts do not decode: {error}"),
                                    );
                                    None
                                }
                            }
                        });
                    if version >= 2 {
                        if !addendum.as_ref().is_some_and(|a| {
                            records.first().is_some_and(|r| {
                                crate::protocol::freeze_precedes(&a.frozen, &r.timestamp_utc)
                            })
                        }) {
                            e.error(
                                "P-23",
                                None,
                                "addendum freeze timestamp is after campaign start",
                            );
                        }
                        let hosts: Vec<HostObservation> = records
                            .iter()
                            .filter(|r| {
                                r.event == JournalEvent::DriverDiagnostic
                                    && r.details["kind"] == "host-observation"
                            })
                            .filter_map(|r| {
                                serde_json::from_value(r.details["observation"].clone()).ok()
                            })
                            .collect();
                        let session_count = records
                            .iter()
                            .filter(|r| {
                                matches!(
                                    r.event,
                                    JournalEvent::CampaignStart | JournalEvent::SessionStart
                                )
                            })
                            .count();
                        let frozen = campaign_facts.as_ref().and_then(|f| f.host.as_ref());
                        if hosts != receipt.session_hosts
                            || hosts.len() != session_count
                            || frozen.is_none()
                            || hosts
                                .iter()
                                .any(|h| !frozen.unwrap().material_equivalent(h))
                            || hosts.last() != Some(&receipt.host)
                        {
                            e.error("P-23", None, "session host observations differ from frozen material conditions or journal");
                        }
                        let mut observed_sessions = BTreeSet::new();
                        for record in &records {
                            if record.event == JournalEvent::DriverDiagnostic
                                && record.details["kind"] == "host-observation"
                                && !observed_sessions.insert(record.session_id.clone())
                            {
                                e.error("P-23", None, "session repeats host observation");
                            }
                            if record.event == JournalEvent::CellStart
                                && !observed_sessions.contains(&record.session_id)
                            {
                                e.error(
                                    "P-23",
                                    None,
                                    "measurement precedes session host observation",
                                );
                            }
                        }
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
                                let key = record_key(record);
                                *cell_starts.entry(key.clone()).or_default() += 1;
                                if let Some(case) = &record.case {
                                    cell_start_cases.entry(key).or_default().push(case.clone());
                                }
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
    let checkpoint_store = (|| -> io::Result<CheckpointStore> {
        if manifest_path.file_name().and_then(|name| name.to_str()) != Some("manifest.json") {
            return Err(io::Error::other(
                "checkpoint manifest must be named manifest.json",
            ));
        }
        if sha256_hex(&fs::read(&manifest_path)?) != receipt.checkpoints.manifest_sha256 {
            return Err(io::Error::other(
                "checkpoint manifest digest differs from the receipt",
            ));
        }
        CheckpointStore::inspect(manifest_path.parent().unwrap_or(receipt_dir))
    })();
    let checkpoint_store = match checkpoint_store {
        Ok(store) => Some(store),
        Err(error) => {
            e.error(
                "P-12",
                None,
                format!("checkpoint evidence invalid: {error}"),
            );
            None
        }
    };
    let checkpoint_manifest = checkpoint_store.as_ref().map(CheckpointStore::manifest);
    match (
        campaign_facts.as_ref(),
        saved_plan.as_ref(),
        addendum.as_ref(),
        checkpoint_manifest,
    ) {
        (Some(facts), Some((plan, bytes)), Some(addendum), Some(checkpoint)) => {
            for message in facts.validate_receipt(&receipt, plan, bytes, addendum, checkpoint) {
                e.error("P-23", None, format!("campaign-start freeze: {message}"));
            }
        }
        (None, _, _, _) => e.error(
            "P-23",
            None,
            "complete campaign-start facts are unavailable",
        ),
        _ => e.error(
            "P-23",
            None,
            "saved plan, addendum or checkpoint facts are unavailable",
        ),
    }
    // Cells.
    let family = addendum.as_ref().map(|addendum| {
        let comparisons = if version >= 2 {
            match receipt
                .trial_ledger
                .as_ref()
                .ok_or_else(|| io::Error::other("versioned receipt lacks trial ledger"))
                .and_then(|pin| {
                    crate::trial_ledger::verify(
                        pin,
                        receipt_dir,
                        addendum,
                        &receipt.campaign_id,
                        &receipt.addendum.sha256,
                        &crate::trial_ledger::candidate_ids(
                            addendum,
                            &saved_plan
                                .as_ref()
                                .ok_or_else(|| io::Error::other("missing saved plan"))?
                                .0,
                            &receipt.arms,
                        )?,
                    )
                }) {
                Ok(count) => count,
                Err(error) => {
                    e.error("P-22", None, error.to_string());
                    1
                }
            }
        } else {
            addendum.family_comparisons()
        };
        let alpha = if version >= 2 {
            receipt
                .trial_ledger
                .as_ref()
                .and_then(|pin| crate::trial_ledger::attempt_alpha(pin, receipt_dir, addendum).ok())
                .unwrap_or(receipt.settings.family_alpha)
        } else {
            receipt.settings.family_alpha
        };
        let confidence = bonferroni_confidence(alpha, comparisons)
            .unwrap_or(1.0 - receipt.settings.family_alpha);
        FamilySummary {
            family_id: addendum.family.id.clone(),
            comparisons,
            family_alpha: alpha,
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
        let planned = saved_plan
            .as_ref()
            .and_then(|(plan, _)| plan.cells.iter().find(|planned| planned.cell_id == id));
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
        match planned {
            Some(planned)
                if planned.baseline_arm == cell.baseline_arm
                    && planned.candidate_arm == cell.candidate_arm => {}
            Some(_) => {
                e.error(
                    "P-13",
                    Some(id),
                    "cell arm pairing differs from the saved plan",
                );
                invalid = true;
            }
            None => {
                e.error("P-13", Some(id), "cell is absent from the saved plan");
                invalid = true;
            }
        }
        if let Some(planned) = planned {
            let expected_case = serde_json::json!({
                "key": cell.key,
                "cell_id": cell.cell_id,
                "case": planned.case,
            });
            if cell_start_cases.get(&cell.key) != Some(&vec![expected_case]) {
                e.error(
                    "P-11",
                    Some(id),
                    "cell-start input differs from the saved plan",
                );
                invalid = true;
            }
        }
        if cell_starts.get(&cell.key).copied().unwrap_or(0) != 1 {
            e.error("P-11", Some(id), "cell has no single cell-start record");
            invalid = true;
        }
        // Every planned cell, including an unavailable one, is accepted into
        // the checkpoint store before the runner records cell completion.
        let checkpoint_result = (|| -> io::Result<()> {
            let store = checkpoint_store
                .as_ref()
                .ok_or_else(|| io::Error::other("validated checkpoint store unavailable"))?;
            let completed = store
                .completed_unit(&cell.key)
                .ok_or_else(|| io::Error::other("cell has no checkpoint unit"))?;
            if cell.checkpoint_sha256.as_ref() != Some(&completed.sha256) {
                return Err(io::Error::other(
                    "cell checkpoint digest differs from stored unit",
                ));
            }
            let (case, result): (Value, CellRecord) = store.load(&cell.key)?;
            if let Some(planned) = planned {
                let expected = serde_json::json!({
                    "key": cell.key, "cell_id": cell.cell_id, "case": planned.case,
                });
                if case != expected {
                    return Err(io::Error::other(
                        "checkpoint case differs from the saved plan",
                    ));
                }
            }
            let mut expected = cell.clone();
            expected.checkpoint_sha256 = None;
            if result != expected {
                return Err(io::Error::other(
                    "receipt cell differs from its checkpoint result",
                ));
            }
            Ok(())
        })();
        if let Err(error) = checkpoint_result {
            e.error("P-12", Some(id), error.to_string());
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
        if let Some(planned) = planned {
            let planned_pairs = planned.pair_count(cell.role, settings) as usize;
            if cell.pairs.len() != planned_pairs {
                e.error("P-15", Some(id), "pair count differs from the saved plan");
                invalid = true;
            }
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
                if version >= 2 {
                    verdict.flagged_windows += if version >= 3 {
                        flagged_windows(&values, settings.flagged_window_factor).unwrap_or(0)
                    } else {
                        flagged_windows_legacy(&values, settings.flagged_window_factor)
                            .unwrap_or(0)
                    };
                }
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
                    if version >= 2
                        && declared.cache_state == CacheState::Cold
                        && (execution.calibrated != Some(false)
                            || declared.cold_calls.is_none()
                            || execution
                                .windows
                                .iter()
                                .any(|w| Some(w.calls) != declared.cold_calls))
                    {
                        e.error(
                            "P-17",
                            Some(id),
                            "cold execution used calibration or differs from frozen call count",
                        );
                        invalid = true;
                    }
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
        if version == 1 && !all_windows.is_empty() {
            verdict.flagged_windows =
                flagged_windows_legacy(&all_windows, settings.flagged_window_factor).unwrap_or(0);
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
                            if version >= 2 {
                                if let Err(message) = validate_frame_quality(arm, decoder) {
                                    e.error("P-18", Some(id), format!("{name}: {message}"));
                                    invalid = true;
                                }
                                if cell.pairs.iter().any(|p| {
                                    let execution = if name == "baseline" {
                                        &p.baseline
                                    } else {
                                        &p.candidate
                                    };
                                    execution.quality.as_ref() != Some(arm)
                                }) {
                                    e.error(
                                        "P-18",
                                        Some(id),
                                        "decoder quality differs from runner execution evidence",
                                    );
                                    invalid = true;
                                }
                            }
                            if version == 1 && arm.interval_method != "wilson-95" {
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
                                if version >= 2 {
                                    crate::abtest::frame_ber_interval(
                                        &arm.frame_bit_errors,
                                        decoder.code.k,
                                        settings.quality_confidence,
                                    )
                                } else {
                                    wilson_interval_95(arm.bit_errors, arm.bits)
                                },
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
                        let exceeds_tolerance = if version >= 2 {
                            crate::abtest::paired_fer_upper(
                                &quality.baseline.frame_bit_errors,
                                &quality.candidate.frame_bit_errors,
                                decoder.quality_tolerance.fer_ratio_max,
                                family
                                    .as_ref()
                                    .map(|f| f.per_comparison_confidence)
                                    .unwrap_or(0.95),
                            )
                            .map_or(true, |upper| upper > 0.0)
                        } else {
                            quality.candidate.fer_interval[1]
                                > decoder.quality_tolerance.fer_ratio_max
                                    * quality.baseline.fer_interval[1]
                        };
                        if matches!(decoder.arm_kind, DecoderArmKind::FastestQualityCompatible)
                            && exceeds_tolerance
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
        let corrected_alpha = family
            .as_ref()
            .map(|family| family.family_alpha / f64::from(family.comparisons))
            .unwrap_or(settings.family_alpha);
        let interval = match if version >= 3 {
            paired_bootstrap_speedup(
                &observations,
                settings.bootstrap_resamples,
                corrected_alpha,
                cell_seed,
            )
        } else {
            paired_bootstrap_speedup_legacy(
                &observations,
                settings.bootstrap_resamples,
                confidence,
                cell_seed,
            )
        } {
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
        let mut endpoint_unresolved = false;
        if version >= 2 && cell.role != CellRole::Exploratory {
            let check = if version >= 3 {
                paired_bootstrap_speedup(
                    &observations,
                    settings.bootstrap_resamples,
                    corrected_alpha,
                    cell_seed ^ 0xd1b54a32d192ed03,
                )
            } else {
                paired_bootstrap_speedup_legacy(
                    &observations,
                    settings.bootstrap_resamples,
                    confidence,
                    cell_seed ^ 0xd1b54a32d192ed03,
                )
            }
            .map_err(io::Error::other)?;
            let endpoint_shift = (check.lower - interval.lower)
                .abs()
                .max((check.upper - interval.upper).abs())
                / interval.estimate;
            let resolution = addendum
                .as_ref()
                .and_then(|a| a.effect.measurement_resolution);
            endpoint_unresolved = resolution.is_none_or(|r| endpoint_shift > r)
                || f64::from(settings.bootstrap_resamples) * corrected_alpha / 2.0 < 20.0;
            if endpoint_unresolved {
                e.note("P-20", Some(id), "bootstrap endpoints lack declared numerical resolution or twenty tail replicates");
            }
        }
        let flagged_fraction = verdict.flagged_windows as f64 / verdict.total_windows.max(1) as f64;
        verdict.outcome = if matches!(cell.role, CellRole::Exploratory) {
            CellOutcome::Pilot
        } else if receipt.settings_deviation || !unresolved.is_empty() || endpoint_unresolved {
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
    if let Some((plan, _)) = &saved_plan {
        for planned in &plan.cells {
            if !seen_cells.contains(&planned.cell_id) {
                e.error(
                    "P-13",
                    Some(&planned.cell_id),
                    "saved-plan cell is absent from the receipt",
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
        && verdicts
            .iter()
            .any(|cell| cell.role != CellRole::Exploratory)
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
        Ok(pilot) if pilot.label == ReceiptLabel::Pilot => {
            if addendum.protocol.version >= 3 {
                if pilot.family_id != addendum.family.id || pilot.issue != addendum.family.issue {
                    evaluation.error(
                        "P-03",
                        None,
                        "resolution evidence pilot belongs to a different family",
                    );
                    return;
                }
                match pilot_resolution(&pilot) {
                    Ok(derived) if addendum.effect.measurement_resolution.unwrap() < derived => {
                        evaluation.error(
                            "P-03",
                            None,
                            format!(
                                "declared measurement resolution is below the verified pilot half-width {derived}"
                            ),
                        );
                    }
                    Ok(_) => {}
                    Err(message) => evaluation.error("P-03", None, message),
                }
            }
        }
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

/// Recomputes the conservative widest relative interval half-width from a
/// verified v3 pilot receipt's raw paired observations.
fn pilot_resolution(pilot: &BenchmarkReceipt) -> Result<f64, String> {
    let mut widest: Option<f64> = None;
    for cell in &pilot.cells {
        if cell.pairs.is_empty() {
            continue;
        }
        let claimed = cell
            .claimed
            .as_ref()
            .ok_or_else(|| format!("pilot cell {} lacks a declared bootstrap alpha", cell.cell_id))?;
        let observations: Vec<_> = cell
            .pairs
            .iter()
            .map(|pair| PairedObservation {
                baseline_ns_per_call: pair.baseline.ns_per_call,
                candidate_ns_per_call: pair.candidate.ns_per_call,
            })
            .collect();
        let interval = paired_bootstrap_speedup(
            &observations,
            pilot.settings.bootstrap_resamples,
            claimed.interval.alpha,
            claimed.interval.seed,
        )
        .map_err(|error| format!("pilot cell {} bootstrap: {error}", cell.cell_id))?;
        if !(interval.estimate.is_finite() && interval.estimate > 0.0) {
            return Err(format!(
                "pilot cell {} has no positive finite speedup estimate",
                cell.cell_id
            ));
        }
        let half_width = (interval.estimate - interval.lower)
            .abs()
            .max((interval.upper - interval.estimate).abs())
            / interval.estimate;
        widest = Some(widest.map_or(half_width, |current| current.max(half_width)));
    }
    widest.ok_or_else(|| "pilot receipt contains no measured paired cells".into())
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

/// Preserves the strict v1/v2 boundary while their frozen receipts remain
/// reproducibly evaluable. Version 3 uses `abtest::flagged_windows`.
fn flagged_windows_legacy(ns_per_call: &[f64], factor: f64) -> Result<usize, crate::abtest::AbError> {
    let mut sorted = ns_per_call.to_vec();
    let center = median(&mut sorted)?;
    Ok(ns_per_call
        .iter()
        .filter(|value| **value > factor * center)
        .count())
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

/// Checks versioned quality denominators, frame counts, points and interval method
/// against the frozen code/input contract. BER counts information bits (`k`).
/// Each frame vector slot corresponds to the same recorded input in both arms.
pub fn validate_frame_quality(
    arm: &ArmQuality,
    decoder: &crate::protocol::DecoderCell,
) -> Result<(), String> {
    if arm.frames != decoder.input.frames || arm.frame_bit_errors.len() as u64 != arm.frames {
        return Err("frame count differs from frozen input".into());
    }
    if decoder.code.k.checked_mul(arm.frames) != Some(arm.bits) || arm.bits == 0 {
        return Err("bit denominator differs from frozen information-bit identity".into());
    }
    let errors = arm
        .frame_bit_errors
        .iter()
        .try_fold(0u64, |n, e| n.checked_add(*e));
    if arm.frame_bit_errors.iter().any(|e| *e > decoder.code.k)
        || errors != Some(arm.bit_errors)
        || arm.frame_bit_errors.iter().filter(|e| **e != 0).count() as u64 != arm.frame_errors
    {
        return Err("aggregate error counts differ from frame evidence".into());
    }
    if !close(arm.ber, arm.bit_errors as f64 / arm.bits as f64) {
        return Err("BER point differs from error counts".into());
    }
    if arm.interval_method != "frame-hoeffding-95+fer-wilson-95" {
        return Err("versioned protocol requires frame-independent BER intervals".into());
    }
    Ok(())
}
