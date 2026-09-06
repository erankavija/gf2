//! Zen 3 benchmark protocol identity, frozen shared settings, and the
//! family/cell addendum contract.
//!
//! The protocol document, the addendum schema and this module describe one
//! protocol version. Receipts pin the document and schema by path, Git commit
//! and content digest; the acceptance tool recomputes every digest and every
//! statistic instead of trusting a receipt's own claims.

use crate::abtest::Margins;
use crate::host::CoreArm;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

/// Stable protocol identifier; a new version keeps the identifier.
pub const PROTOCOL_ID: &str = "zen3-benchmark-protocol";
/// Protocol version described by this module and the committed document.
pub const PROTOCOL_VERSION: u32 = 1;
/// Repository-relative path of the executable protocol document.
pub const PROTOCOL_PATH: &str = "dev/active/f547c394/protocol.md";
/// Repository-relative path of the addendum JSON Schema.
pub const ADDENDUM_SCHEMA_PATH: &str = "dev/active/f547c394/addendum.schema.json";
/// Repository-relative path of the normative measurement contract.
pub const CONTRACT_PATH: &str = "dev/active/1a379447-zen3-cpu-performance/measurement-contract.md";
/// Schema identity carried by every addendum.
pub const ADDENDUM_SCHEMA_ID: &str = "zen3-benchmark-addendum-v1";
/// Schema identity carried by every runner plan.
pub const PLAN_SCHEMA_ID: &str = "zen3-benchmark-plan-v1";
/// Schema identity carried by every receipt.
pub const RECEIPT_SCHEMA_ID: &str = "zen3-benchmark-receipt-v1";
/// Schema identity carried by every acceptance summary.
pub const ACCEPTANCE_SCHEMA_ID: &str = "zen3-benchmark-acceptance-v1";
/// Lifecycle schema recorded in the runner's resume identity.
pub const RUNNER_LIFECYCLE_SCHEMA: &str = "zen3-benchmark-runner-session-v1";

/// Shared numeric settings frozen by protocol version 1.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SharedSettings {
    /// Family-wise two-sided error rate controlled by Bonferroni.
    pub family_alpha: f64,
    /// Bootstrap replicates per interval.
    pub bootstrap_resamples: u32,
    /// Pairs in every confirmatory or holdout trial; no data-dependent stop.
    pub confirmatory_pairs: u32,
    /// Fewest pairs in an exploratory pilot.
    pub pilot_min_pairs: u32,
    /// Most pairs in an exploratory pilot.
    pub pilot_max_pairs: u32,
    /// Timing windows in each fresh execution.
    pub windows_per_execution: u32,
    /// Target length of one timing window.
    pub window_target_ms: u32,
    /// A window slower than this multiple of its execution median is flagged.
    pub flagged_window_factor: f64,
    /// Largest flagged-window fraction a confirmatory cell may carry.
    pub max_flagged_fraction: f64,
    /// Most exploratory trials any cell may spend on candidates.
    pub max_pilot_trials_per_cell: u32,
    /// Confirmatory attempts one candidate identity gets per protocol version.
    pub max_confirmatory_attempts_per_candidate: u32,
    /// Wall-clock bound on one arm execution.
    pub child_timeout_seconds: u64,
    /// Two-sided confidence of every BER/FER interval.
    pub quality_confidence: f64,
}

/// The frozen version-1 settings.
pub const SHARED_SETTINGS: SharedSettings = SharedSettings {
    family_alpha: 0.05,
    bootstrap_resamples: 10_000,
    confirmatory_pairs: 24,
    pilot_min_pairs: 6,
    pilot_max_pairs: 24,
    windows_per_execution: 5,
    window_target_ms: 100,
    flagged_window_factor: 2.0,
    max_flagged_fraction: 0.10,
    max_pilot_trials_per_cell: 8,
    max_confirmatory_attempts_per_candidate: 1,
    child_timeout_seconds: 120,
    quality_confidence: 0.95,
};

impl SharedSettings {
    /// Name/value rows in the order the protocol document tabulates them.
    pub fn table(&self) -> Vec<(&'static str, String)> {
        vec![
            ("family_alpha", format!("{}", self.family_alpha)),
            ("bootstrap_resamples", self.bootstrap_resamples.to_string()),
            ("confirmatory_pairs", self.confirmatory_pairs.to_string()),
            ("pilot_min_pairs", self.pilot_min_pairs.to_string()),
            ("pilot_max_pairs", self.pilot_max_pairs.to_string()),
            (
                "windows_per_execution",
                self.windows_per_execution.to_string(),
            ),
            ("window_target_ms", self.window_target_ms.to_string()),
            (
                "flagged_window_factor",
                format!("{}", self.flagged_window_factor),
            ),
            (
                "max_flagged_fraction",
                format!("{}", self.max_flagged_fraction),
            ),
            (
                "max_pilot_trials_per_cell",
                self.max_pilot_trials_per_cell.to_string(),
            ),
            (
                "max_confirmatory_attempts_per_candidate",
                self.max_confirmatory_attempts_per_candidate.to_string(),
            ),
            (
                "child_timeout_seconds",
                self.child_timeout_seconds.to_string(),
            ),
            ("quality_confidence", format!("{}", self.quality_confidence)),
        ]
    }
}

/// Git and content identity of one committed artifact.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPin {
    /// Repository-relative path.
    pub path: String,
    /// Full lowercase Git commit object ID of the tree the artifact was read from.
    pub git_commit: String,
    /// Lowercase hex SHA-256 of the exact file bytes.
    pub sha256: String,
}

impl ArtifactPin {
    /// Reads `relative` under `repo_root`, digests its bytes and records `HEAD`.
    pub fn pin(repo_root: &Path, relative: &str) -> io::Result<Self> {
        let bytes = fs::read(repo_root.join(relative))?;
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_root)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("git rev-parse HEAD failed"));
        }
        let git_commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let pin = Self {
            path: relative.to_owned(),
            git_commit,
            sha256: sha256_hex(&bytes),
        };
        pin.validate_shape()
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
        Ok(pin)
    }

    /// Checks the identity's syntax without touching the filesystem.
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.path.is_empty() || self.path.starts_with('/') {
            return Err(format!(
                "artifact path {:?} is not repository-relative",
                self.path
            ));
        }
        if !is_hex(&self.git_commit, 40) {
            return Err(format!(
                "artifact {} git commit is not a 40-hex object ID",
                self.path
            ));
        }
        if !is_hex(&self.sha256, 64) {
            return Err(format!(
                "artifact {} sha256 is not 64 hex digits",
                self.path
            ));
        }
        Ok(())
    }

    /// Reads the artifact's bytes at the pinned commit through `git show`,
    /// falling back to the working tree when that commit is not resolvable in
    /// `repo_root`. Evidence is judged against the document version that
    /// governed the run, so a later documentation edit does not invalidate it.
    pub fn read_pinned(&self, repo_root: &Path) -> Result<(Vec<u8>, PinSource), String> {
        self.validate_shape()?;
        let shown = Command::new("git")
            .arg("show")
            .arg(format!("{}:{}", self.git_commit, self.path))
            .current_dir(repo_root)
            .output();
        if let Ok(output) = shown {
            if output.status.success() {
                return Ok((output.stdout, PinSource::Commit));
            }
        }
        let bytes = fs::read(repo_root.join(&self.path))
            .map_err(|error| format!("cannot read pinned artifact {}: {error}", self.path))?;
        Ok((bytes, PinSource::WorkingTree))
    }

    /// Recomputes the content digest from the pinned bytes and reports where
    /// they came from.
    pub fn verify_content(&self, repo_root: &Path) -> Result<PinSource, String> {
        let (bytes, source) = self.read_pinned(repo_root)?;
        if sha256_hex(&bytes) != self.sha256 {
            return Err(format!(
                "pinned artifact {} content digest differs",
                self.path
            ));
        }
        Ok(source)
    }

    /// Whether the working-tree file still has the pinned digest; `None`
    /// when the file cannot be read.
    pub fn working_tree_matches(&self, repo_root: &Path) -> Option<bool> {
        fs::read(repo_root.join(&self.path))
            .ok()
            .map(|bytes| sha256_hex(&bytes) == self.sha256)
    }
}

/// Where a pinned artifact's bytes were read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinSource {
    /// The object at the pinned commit.
    Commit,
    /// The working tree; the pinned commit was not resolvable.
    WorkingTree,
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// True for exactly `len` lowercase hex digits.
pub fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// What a receipt was produced for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiptLabel {
    /// Pipeline proof; never a performance result.
    Smoke,
    Pilot,
    Confirmation,
    Holdout,
}

/// Which protocol an addendum freezes cells for.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolRef {
    pub id: String,
    pub version: u32,
}

/// What a family measures and adopts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FamilyPurpose {
    KernelFamily,
    ConsumerFamily,
    DecoderFamily,
    SelectorCalibration,
    FinalIntegration,
}

/// Family identity.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyIdentity {
    /// Lowercase kebab-case family identifier.
    pub id: String,
    /// Owning issue short ID.
    pub issue: String,
    pub purpose: FamilyPurpose,
    pub description: String,
}

/// Freezing record; `null` fields mean the addendum is not yet frozen.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Frozen {
    pub at_commit: Option<String>,
    pub frozen_utc: Option<String>,
}

/// Family-declared worthwhile effect, equivalence and material-gap rules.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRule {
    /// Speedup lower bound that counts as a worthwhile improvement.
    pub worthwhile_speedup: Option<f64>,
    pub rationale: String,
    /// Pilot-observed relative half-width of the speedup interval.
    pub measurement_resolution: Option<f64>,
    /// Receipt path that observed the resolution.
    pub resolution_evidence: Option<String>,
    /// Largest slowdown factor still declared not worse.
    pub equivalence_margin: Option<f64>,
    pub equivalence_rationale: String,
    /// Comparator speedup lower bound that makes a gap material.
    pub material_gap_threshold: Option<f64>,
    pub material_gap_rationale: String,
}

/// Maintenance budget a candidate must stay within.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplexityBudget {
    pub max_new_unsafe_kernels: u32,
    pub max_added_source_lines: Option<u32>,
    pub maintenance_rationale: String,
}

/// Prior confirmatory trial retained in the family ledger.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriorTrial {
    /// Repository-relative receipt path.
    pub receipt: String,
    pub sha256: String,
    /// Outcome the receipt recorded, retained verbatim.
    pub outcome: String,
}

/// Multiple-comparison family declaration.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyWise {
    pub alpha: f64,
    /// Confirmatory trials already spent under this protocol version.
    pub prior_confirmatory_trials: u32,
    pub prior_trials: Vec<PriorTrial>,
}

/// Bounded search declaration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchBudget {
    pub max_pilot_trials_per_cell: u32,
    pub max_confirmatory_attempts_per_candidate: u32,
}

/// Independent holdout declaration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Holdout {
    pub required: bool,
    pub cells: Vec<String>,
}

/// What a cell's comparison decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CellObjective {
    /// Candidate must beat baseline by the worthwhile margin.
    Improvement,
    /// Candidate must not be worse than baseline beyond the equivalence margin.
    NonRegression,
    /// External comparator versus gf2; decides material versus not material.
    ComparatorGap,
}

/// Sampling role of a cell's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CellRole {
    Exploratory,
    Confirmatory,
    Holdout,
}

/// Kernel-only or whole-consumer timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetricKind {
    KernelIsolated,
    WholeConsumer,
}

/// What the cell's time series characterizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scaling {
    SingleCoreLatency,
    SustainedThroughput,
    MulticoreThroughput,
}

/// Declared cache state the arm establishes before timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheState {
    Cold,
    Warm,
    Streaming,
}

/// Build identity of an arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuildIdentity {
    ConservativePortable,
    TunedPortable,
    Native,
    /// A surveyed comparator build; the receipt carries its own identity.
    External,
}

/// Worker declaration for multicore arms.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerDeclaration {
    pub declared: u32,
    pub nested_pools_allowed: bool,
}

/// Arm builds.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArmBuilds {
    pub baseline: BuildIdentity,
    pub candidate: BuildIdentity,
}

/// Workload identity and size.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Workload {
    pub identity: String,
    pub size: BTreeMap<String, u64>,
    pub seed: u64,
}

/// Decoder arm kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecoderArmKind {
    MatchedAlgorithm,
    FastestQualityCompatible,
}

/// Message precision of a decoder arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Precision {
    F64,
    F32,
    I16,
    I8,
}

/// Update schedule of a decoder arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Schedule {
    Flooding,
    Layered,
    Other,
}

/// Check-node rule and its constant.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Normalization {
    pub kind: NormalizationKind,
    pub factor: Option<f64>,
}

/// Check-node update rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NormalizationKind {
    MinSum,
    NormalizedMinSum,
    OffsetMinSum,
    SumProduct,
}

/// Stopping contract.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stopping {
    pub kind: StoppingKind,
    pub crc: Option<String>,
}

/// Stopping rule kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StoppingKind {
    Syndrome,
    Crc,
    Fixed,
    None,
}

/// Codeword source for quality measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodewordSource {
    AllZero,
    Random,
    Both,
}

/// Decoder input identity.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderInput {
    pub llr_source: String,
    pub llr_sha256: Option<String>,
    pub frames: u64,
    pub seed: u64,
    pub codeword_source: CodewordSource,
    pub snr_db: f64,
}

/// Code identity.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeIdentity {
    pub identity: String,
    pub n: u64,
    pub k: u64,
    pub h_sha256: String,
}

/// Batching declaration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Batching {
    pub batch_size: u32,
    pub batch_fill_included: bool,
}

/// Predeclared quality tolerance for fastest-quality-compatible arms.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QualityTolerance {
    /// Candidate FER upper bound may be at most this multiple of the baseline FER upper bound.
    pub fer_ratio_max: f64,
    pub confidence: f64,
}

/// Rate matching declaration for other arms.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RateMatching {
    pub puncturing: String,
    pub fillers: String,
}

/// Decoder cell fields of the measurement contract.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderCell {
    pub arm_kind: DecoderArmKind,
    pub code: CodeIdentity,
    pub input: DecoderInput,
    pub precision: Precision,
    pub schedule: Schedule,
    pub normalization: Normalization,
    pub iteration_cap: u32,
    pub stopping: Stopping,
    pub batching: Batching,
    pub quality_tolerance: QualityTolerance,
    pub rate_matching: Option<RateMatching>,
}

/// One frozen cell.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellDeclaration {
    pub cell_id: String,
    pub objective: CellObjective,
    pub role: CellRole,
    pub workload: Workload,
    pub metric_kind: MetricKind,
    pub scaling: Scaling,
    pub core_arm: CoreArm,
    pub workers: WorkerDeclaration,
    pub cache_state: CacheState,
    pub builds: ArmBuilds,
    pub conversion_costs_included: bool,
    pub decoder: Option<DecoderCell>,
}

/// One family's independently frozen addendum.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyAddendum {
    pub schema: String,
    pub protocol: ProtocolRef,
    pub family: FamilyIdentity,
    pub frozen: Frozen,
    pub effect: EffectRule,
    pub complexity_budget: ComplexityBudget,
    pub family_wise: FamilyWise,
    pub search_budget: SearchBudget,
    pub holdout: Holdout,
    pub cells: Vec<CellDeclaration>,
}

impl FamilyAddendum {
    /// Decodes an addendum; unknown fields and shape errors fail closed.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|error| format!("addendum does not decode: {error}"))
    }

    /// Structural and semantic validation independent of freezing state.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.schema != ADDENDUM_SCHEMA_ID {
            errors.push(format!(
                "schema {:?} is not {ADDENDUM_SCHEMA_ID}",
                self.schema
            ));
        }
        if self.protocol.id != PROTOCOL_ID || self.protocol.version != PROTOCOL_VERSION {
            errors.push(format!(
                "addendum targets protocol {}/{} rather than {PROTOCOL_ID}/{PROTOCOL_VERSION}",
                self.protocol.id, self.protocol.version
            ));
        }
        if !is_kebab(&self.family.id) {
            errors.push(format!("family id {:?} is not kebab-case", self.family.id));
        }
        if self.family.issue.len() != 8 || !is_hex(&self.family.issue, 8) {
            errors.push(format!(
                "family issue {:?} is not an 8-hex short ID",
                self.family.issue
            ));
        }
        if self.family_wise.alpha != SHARED_SETTINGS.family_alpha {
            errors.push(format!(
                "family alpha {} differs from the frozen shared alpha {}",
                self.family_wise.alpha, SHARED_SETTINGS.family_alpha
            ));
        }
        if self.family_wise.prior_trials.len() as u32 != self.family_wise.prior_confirmatory_trials
        {
            errors.push("prior_confirmatory_trials disagrees with the prior_trials ledger".into());
        }
        for trial in &self.family_wise.prior_trials {
            if !is_hex(&trial.sha256, 64) {
                errors.push(format!(
                    "prior trial {} sha256 is not 64 hex digits",
                    trial.receipt
                ));
            }
        }
        if let Some(commit) = &self.frozen.at_commit {
            if !is_hex(commit, 40) {
                errors.push("frozen.at_commit is not a 40-hex object ID".into());
            }
            if self.frozen.frozen_utc.is_none() {
                errors.push("frozen.at_commit without frozen_utc".into());
            }
        } else if self.frozen.frozen_utc.is_some() {
            errors.push("frozen_utc without frozen.at_commit".into());
        }
        let effect = &self.effect;
        if let Some(value) = effect.worthwhile_speedup {
            if !(value.is_finite() && value > 1.0) {
                errors.push("worthwhile_speedup must exceed 1".into());
            }
            if effect.rationale.trim().is_empty() {
                errors.push("worthwhile_speedup needs a rationale".into());
            }
            if let Some(resolution) = effect.measurement_resolution {
                if value - 1.0 < resolution {
                    errors.push(format!(
                        "worthwhile_speedup {value} lies inside the measurement resolution {resolution}"
                    ));
                }
            }
        }
        if let Some(value) = effect.measurement_resolution {
            if !(value.is_finite() && value > 0.0 && value < 1.0) {
                errors.push("measurement_resolution must lie in (0, 1)".into());
            }
            if effect.resolution_evidence.is_none() {
                errors.push("measurement_resolution needs resolution_evidence".into());
            }
        }
        if let Some(value) = effect.equivalence_margin {
            if !(value.is_finite() && value >= 1.0) {
                errors.push("equivalence_margin must be at least 1".into());
            }
            if effect.equivalence_rationale.trim().is_empty() {
                errors.push("equivalence_margin needs a rationale".into());
            }
        }
        if let Some(value) = effect.material_gap_threshold {
            if !(value.is_finite() && value >= 1.0) {
                errors.push("material_gap_threshold must be at least 1".into());
            }
            if effect.material_gap_rationale.trim().is_empty() {
                errors.push("material_gap_threshold needs a rationale".into());
            }
        }
        if self
            .complexity_budget
            .maintenance_rationale
            .trim()
            .is_empty()
        {
            errors.push("complexity_budget needs a maintenance rationale".into());
        }
        let budget = &self.search_budget;
        if budget.max_pilot_trials_per_cell == 0
            || budget.max_pilot_trials_per_cell > SHARED_SETTINGS.max_pilot_trials_per_cell
        {
            errors.push(format!(
                "max_pilot_trials_per_cell must lie in 1..={}",
                SHARED_SETTINGS.max_pilot_trials_per_cell
            ));
        }
        if budget.max_confirmatory_attempts_per_candidate
            != SHARED_SETTINGS.max_confirmatory_attempts_per_candidate
        {
            errors.push(format!(
                "max_confirmatory_attempts_per_candidate must equal the frozen {}",
                SHARED_SETTINGS.max_confirmatory_attempts_per_candidate
            ));
        }
        let mut ids = BTreeSet::new();
        for cell in &self.cells {
            if !ids.insert(cell.cell_id.as_str()) {
                errors.push(format!("duplicate cell id {:?}", cell.cell_id));
            }
            if !is_kebab(&cell.cell_id) {
                errors.push(format!("cell id {:?} is not kebab-case", cell.cell_id));
            }
            if cell.workload.identity.trim().is_empty() {
                errors.push(format!("cell {} lacks a workload identity", cell.cell_id));
            }
            if cell.workers.declared == 0 {
                errors.push(format!("cell {} declares zero workers", cell.cell_id));
            }
            if matches!(cell.core_arm, CoreArm::SingleCore) && cell.workers.declared != 1 {
                errors.push(format!(
                    "cell {} single-core arm declares several workers",
                    cell.cell_id
                ));
            }
            if matches!(cell.metric_kind, MetricKind::WholeConsumer)
                && !cell.conversion_costs_included
            {
                errors.push(format!(
                    "cell {} is whole-consumer but excludes conversion costs",
                    cell.cell_id
                ));
            }
            if matches!(self.family.purpose, FamilyPurpose::DecoderFamily) && cell.decoder.is_none()
            {
                errors.push(format!(
                    "decoder-family cell {} lacks decoder fields",
                    cell.cell_id
                ));
            }
            if matches!(cell.role, CellRole::Holdout) && !self.holdout.cells.contains(&cell.cell_id)
            {
                errors.push(format!(
                    "holdout cell {} is not listed under holdout.cells",
                    cell.cell_id
                ));
            }
            if let Some(decoder) = &cell.decoder {
                errors.extend(validate_decoder(&cell.cell_id, decoder));
            }
        }
        for holdout in &self.holdout.cells {
            match self.cells.iter().find(|cell| &cell.cell_id == holdout) {
                None => errors.push(format!("holdout.cells names unknown cell {holdout:?}")),
                Some(cell) if cell.role != CellRole::Holdout => errors.push(format!(
                    "holdout.cells names {holdout:?} whose role is not holdout"
                )),
                Some(_) => {}
            }
        }
        if matches!(
            self.family.purpose,
            FamilyPurpose::SelectorCalibration | FamilyPurpose::FinalIntegration
        ) && (!self.holdout.required || self.holdout.cells.is_empty())
        {
            errors.push(
                "selector-calibration and final-integration families require holdout cells".into(),
            );
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Settings a cell still needs before it may be confirmatory. An empty
    /// list means the cell is ready; any entry makes the cell
    /// non-confirmatory regardless of its declared role.
    pub fn unresolved_settings(&self, cell: &CellDeclaration) -> Vec<String> {
        let mut unresolved = Vec::new();
        if self.frozen.at_commit.is_none() {
            unresolved.push("frozen.at_commit".into());
        }
        if self.effect.measurement_resolution.is_none() {
            unresolved.push("effect.measurement_resolution".into());
        }
        match cell.objective {
            CellObjective::Improvement => {
                if self.effect.worthwhile_speedup.is_none() {
                    unresolved.push("effect.worthwhile_speedup".into());
                }
                if self.effect.equivalence_margin.is_none() {
                    unresolved.push("effect.equivalence_margin".into());
                }
            }
            CellObjective::NonRegression => {
                if self.effect.equivalence_margin.is_none() {
                    unresolved.push("effect.equivalence_margin".into());
                }
                if self.effect.worthwhile_speedup.is_none() {
                    unresolved.push("effect.worthwhile_speedup".into());
                }
            }
            CellObjective::ComparatorGap => {
                if self.effect.material_gap_threshold.is_none() {
                    unresolved.push("effect.material_gap_threshold".into());
                }
                if self.effect.equivalence_margin.is_none() {
                    unresolved.push("effect.equivalence_margin".into());
                }
            }
        }
        if self.complexity_budget.max_added_source_lines.is_none()
            && !matches!(cell.objective, CellObjective::ComparatorGap)
        {
            unresolved.push("complexity_budget.max_added_source_lines".into());
        }
        unresolved
    }

    /// Decision margins for a cell, available once its settings resolve.
    pub fn margins(&self, cell: &CellDeclaration) -> Option<Margins> {
        let equivalence = self.effect.equivalence_margin?;
        let improvement = match cell.objective {
            CellObjective::ComparatorGap => self.effect.material_gap_threshold?,
            _ => self.effect.worthwhile_speedup?,
        };
        Some(Margins {
            improvement,
            equivalence,
        })
    }

    /// Comparisons counted in the Bonferroni family: confirmatory and holdout
    /// cells in this addendum plus prior confirmatory trials.
    pub fn family_comparisons(&self) -> u32 {
        let current = self
            .cells
            .iter()
            .filter(|cell| !matches!(cell.role, CellRole::Exploratory))
            .count() as u32;
        current.max(1) + self.family_wise.prior_confirmatory_trials
    }

    /// The declared cell with this identifier.
    pub fn cell(&self, cell_id: &str) -> Option<&CellDeclaration> {
        self.cells.iter().find(|cell| cell.cell_id == cell_id)
    }
}

fn validate_decoder(cell_id: &str, decoder: &DecoderCell) -> Vec<String> {
    let mut errors = Vec::new();
    if decoder.code.n == 0 || decoder.code.k == 0 || decoder.code.k > decoder.code.n {
        errors.push(format!("cell {cell_id} code dimensions are invalid"));
    }
    if !is_hex(&decoder.code.h_sha256, 64) {
        errors.push(format!(
            "cell {cell_id} parity-check digest is not 64 hex digits"
        ));
    }
    if decoder.input.frames == 0 {
        errors.push(format!("cell {cell_id} decoder input has zero frames"));
    }
    if decoder.iteration_cap == 0 {
        errors.push(format!("cell {cell_id} iteration cap is zero"));
    }
    match (decoder.normalization.kind, decoder.normalization.factor) {
        (NormalizationKind::NormalizedMinSum, Some(factor)) if factor > 0.0 && factor <= 1.0 => {}
        (NormalizationKind::OffsetMinSum, Some(factor)) if factor >= 0.0 => {}
        (NormalizationKind::MinSum | NormalizationKind::SumProduct, None) => {}
        _ => errors.push(format!(
            "cell {cell_id} normalization kind and factor disagree"
        )),
    }
    if matches!(decoder.stopping.kind, StoppingKind::Crc) != decoder.stopping.crc.is_some() {
        errors.push(format!(
            "cell {cell_id} CRC stopping needs exactly one polynomial"
        ));
    }
    if decoder.batching.batch_size == 0 {
        errors.push(format!("cell {cell_id} batch size is zero"));
    }
    if decoder.quality_tolerance.fer_ratio_max.partial_cmp(&1.0)
        != Some(std::cmp::Ordering::Greater)
        && decoder.quality_tolerance.fer_ratio_max != 1.0
    {
        errors.push(format!("cell {cell_id} fer_ratio_max must be at least 1"));
    }
    if decoder.quality_tolerance.confidence != SHARED_SETTINGS.quality_confidence {
        errors.push(format!(
            "cell {cell_id} quality confidence differs from the frozen {}",
            SHARED_SETTINGS.quality_confidence
        ));
    }
    errors
}

/// True for `[a-z0-9]+(-[a-z0-9]+)*`.
pub fn is_kebab(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Timing override recorded as a settings deviation; test use only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimingOverride {
    pub windows_per_execution: u32,
    pub window_target_ms: u32,
}

/// Tuning profile identity an arm was built or launched with.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuningProfileRef {
    pub id: String,
    pub sha256: String,
}

/// One arm the runner launches as a fresh child per execution.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanArm {
    pub build: BuildIdentity,
    pub description: String,
    /// Absolute executable path; its bytes are digested at run time.
    pub executable: String,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub rustflags: Option<String>,
    pub tuning_profile: Option<TuningProfileRef>,
}

/// One cell the runner measures; `case` is opaque to the runner and is
/// forwarded verbatim to both arms.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCell {
    pub cell_id: String,
    pub baseline_arm: String,
    pub candidate_arm: String,
    pub case: serde_json::Value,
    /// Pairs for an exploratory cell; confirmatory cells use the frozen count.
    pub pilot_pairs: Option<u32>,
}

/// The runner's input: a bounded campaign over frozen addendum cells.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerPlan {
    pub schema: String,
    pub campaign_id: String,
    pub issue: String,
    pub label: ReceiptLabel,
    pub campaign_seed: u64,
    /// Repository-relative addendum path.
    pub addendum: String,
    /// Absolute canonical lock path the wrapper holds.
    pub lock_path: String,
    /// Repository-relative wrapper script.
    pub wrapper: String,
    pub timing_override: Option<TimingOverride>,
    pub arms: BTreeMap<String, PlanArm>,
    pub cells: Vec<PlanCell>,
    /// Cells to measure in one session before pausing; `null` means all.
    pub max_cells_per_session: Option<u32>,
}

impl RunnerPlan {
    /// Decodes a plan strictly.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|error| format!("plan does not decode: {error}"))
    }

    /// Validates the plan against itself and the addendum it names.
    pub fn validate(&self, addendum: &FamilyAddendum) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.schema != PLAN_SCHEMA_ID {
            errors.push(format!(
                "plan schema {:?} is not {PLAN_SCHEMA_ID}",
                self.schema
            ));
        }
        if !is_kebab(&self.campaign_id) {
            errors.push(format!(
                "campaign id {:?} is not kebab-case",
                self.campaign_id
            ));
        }
        if self.issue != addendum.family.issue {
            errors.push(format!(
                "plan issue {} differs from the addendum family issue {}",
                self.issue, addendum.family.issue
            ));
        }
        if !self.lock_path.starts_with('/') {
            errors.push("lock path must be absolute".into());
        }
        if self.arms.is_empty() {
            errors.push("plan declares no arms".into());
        }
        if self.cells.is_empty() {
            errors.push("plan declares no cells".into());
        }
        if self.max_cells_per_session == Some(0) {
            errors.push("max_cells_per_session must be positive".into());
        }
        let mut seen = BTreeSet::new();
        for cell in &self.cells {
            if !seen.insert(cell.cell_id.as_str()) {
                errors.push(format!("plan repeats cell {:?}", cell.cell_id));
            }
            let Some(declared) = addendum.cell(&cell.cell_id) else {
                errors.push(format!(
                    "plan cell {:?} is not declared in the addendum",
                    cell.cell_id
                ));
                continue;
            };
            for (name, arm, build) in [
                ("baseline", &cell.baseline_arm, declared.builds.baseline),
                ("candidate", &cell.candidate_arm, declared.builds.candidate),
            ] {
                match self.arms.get(arm) {
                    None => errors.push(format!(
                        "cell {} {name} arm {arm:?} is not declared",
                        cell.cell_id
                    )),
                    Some(plan_arm) if plan_arm.build != build => errors.push(format!(
                        "cell {} {name} arm build {:?} differs from the declared {:?}",
                        cell.cell_id, plan_arm.build, build
                    )),
                    Some(_) => {}
                }
            }
            match (declared.role, cell.pilot_pairs) {
                (CellRole::Exploratory, Some(pairs))
                    if !(SHARED_SETTINGS.pilot_min_pairs..=SHARED_SETTINGS.pilot_max_pairs)
                        .contains(&pairs) =>
                {
                    errors.push(format!(
                        "cell {} pilot pairs {pairs} lie outside {}..={}",
                        cell.cell_id,
                        SHARED_SETTINGS.pilot_min_pairs,
                        SHARED_SETTINGS.pilot_max_pairs
                    ))
                }
                (CellRole::Confirmatory | CellRole::Holdout, Some(_)) => errors.push(format!(
                    "cell {} is not exploratory yet declares pilot pairs",
                    cell.cell_id
                )),
                _ => {}
            }
        }
        for name in self.arms.keys() {
            if !self
                .cells
                .iter()
                .any(|cell| &cell.baseline_arm == name || &cell.candidate_arm == name)
            {
                errors.push(format!("arm {name:?} is declared but unused"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Settings in force for this plan and whether they deviate from the
    /// frozen shared settings.
    pub fn settings(&self) -> (SharedSettings, bool) {
        match self.timing_override {
            Some(override_) => (
                SharedSettings {
                    windows_per_execution: override_.windows_per_execution,
                    window_target_ms: override_.window_target_ms,
                    ..SHARED_SETTINGS
                },
                true,
            ),
            None => (SHARED_SETTINGS, false),
        }
    }
}
