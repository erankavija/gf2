//! Durable pre-draw validation for permanent campaigns.
//!
//! The reusable layer accepts a typed preregistration and journals each
//! address before opening a sampler. [`evaluate_validation_anchor`] exposes
//! the field-generic computation, while [`run_validation`] adds sealed runtime
//! provenance and no-redraw persistence. The repository-specific frozen plan
//! is enforced separately by [`run_frozen_campaign_validation`].
//!
//! The frozen runner refuses a wrong producing toolchain or an unusable
//! required backend before it opens any address, because an address opened
//! under a refused build could not be redrawn.
//!
//! Exhaustive matrices are emitted once in bounded batches. Each identical
//! batch is fanned to the independent fixed-expansion oracle result, the
//! production determinant evaluator, and every required production permanent
//! backend. Sampling opens a third fresh sampler at ordinal zero after the two
//! replay samplers.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf2_algebra::permanent::{
    determinant_singular_probability, try_visit_permanent_anchor_matrices,
};
use gf2_core::gfp::Fp;
use gf2_stats::binomial::two_sided_test;
use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::provenance::{
    observe_accelerator_identity, observe_cpu_identity, observe_provenance, repository_top_level,
};
use super::schedule::{
    backend_supports_cell, evaluate_production_determinants, evaluate_validation_sample,
    pool_production_outcome, ProductionBackendEvaluator, ScheduleError,
};
use super::schema::{
    read_manifest, ArtifactIdentity, ArtifactPath, Availability, Backend, Provenance, RngAlgorithm,
    Sha256Digest, StreamAddress,
};

/// Schema written by the committed preregistration.
pub const PREREGISTRATION_SCHEMA_VERSION: u32 = 1;
/// Schema written by the immutable validation receipt.
pub const RECEIPT_SCHEMA_VERSION: u32 = 1;
/// RNG implementation linked by the workspace sampler.
pub const RNG_VERSION: &str = "rand_chacha 0.9.0";
/// Established validation namespace from the exact-anchor evidence.
pub const FROZEN_VALIDATION_ROOT: u64 = 0x4453_4B2F_0000_0001;
/// Compiler-version prefix the frozen validation evidence must carry.
pub const FROZEN_TOOLCHAIN_PREFIX: &str = "rustc 1.95.0 ";
/// Frozen campaign directory whose payload and inventory must not change.
pub const FROZEN_CAMPAIGN_DIRECTORY: &str =
    "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829";

const FROZEN_PROTOCOL_PATH: &str = "dev/simulation_results/permanent-zero-fraction/protocol.md";
const FROZEN_MANIFEST_PATH: &str =
    "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json";
const EXACT_ANCHORS_PATH: &str = "dev/benchmarks/permanent_campaign/exact-anchors.csv";
const BACKEND_EQUIVALENCE_PATH: &str =
    "dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.csv";
/// The protocol's ten validation anchors, in address order.
///
/// These cells are a protocol constant. Their enumerated counts are not: those
/// come from the committed exact-anchor evidence the preregistration binds by
/// content, so this tool holds no second copy of them.
const FROZEN_ANCHOR_CELLS: &[(u8, u16)] = &[
    (3, 1),
    (3, 2),
    (3, 3),
    (3, 4),
    (5, 1),
    (5, 2),
    (5, 3),
    (7, 1),
    (7, 2),
    (7, 3),
];

/// The only stream purpose admitted by validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStreamPurpose {
    /// Domain-separated sampler validation.
    Validation,
}

impl ValidationStreamPurpose {
    const fn sampler(self) -> StreamPurpose {
        match self {
            Self::Validation => StreamPurpose::Validation,
        }
    }
}

/// Frozen exact-test convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionRule {
    /// Probability-ordering exact two-sided binomial test; pass only above the level.
    ProbabilityOrderingExactTwoSidedStrictGreater,
}

/// Frozen retry convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryRule {
    /// No address is redrawn after it has been opened.
    NoRedraw,
}

/// Runtime replay mode exposed by the sampler contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    /// The sampler has no worker mode, so both instances are fresh serial instances.
    TwoFreshSerial,
}

/// Origin of the fixed statistical sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleOrigin {
    /// A third fresh sampler starts at matrix ordinal zero.
    FreshAddressStart,
}

/// Terminal verdict of an anchor or receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationVerdict {
    /// Every required check passed.
    Passed,
    /// At least one required check failed or could not execute.
    Failed,
}

/// Explicit state of each anchor component.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    /// The component executed and passed.
    Passed,
    /// The component executed and disagreed with its authority.
    Failed,
    /// The component could not reach a mathematical verdict.
    MechanicalFailure,
    /// An earlier component prevented execution.
    Unexecuted,
}

/// Content-addressed inputs that define this validation campaign.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationAuthorities {
    /// Frozen protocol identity.
    pub protocol: ArtifactIdentity,
    /// Frozen campaign manifest identity.
    pub manifest: ArtifactIdentity,
    /// Exact-anchor evidence identity.
    pub exact_anchors: ArtifactIdentity,
    /// Cell-exhaustive backend-equivalence evidence identity.
    pub backend_equivalence: ArtifactIdentity,
}

/// Constants shared by every preregistered anchor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationProtocol {
    /// Validation namespace root.
    pub root_seed: u64,
    /// Closed validation-purpose token.
    pub stream_purpose: ValidationStreamPurpose,
    /// Fresh-instance matrices compared at each address.
    pub replay_matrix_count: u64,
    /// Fixed statistical draws at each address.
    pub sample_matrix_count: u64,
    /// Per-anchor exact-test level.
    pub exact_test_level: f64,
    /// Exact-test convention.
    pub decision_rule: DecisionRule,
    /// Redraw convention.
    pub retry_rule: RetryRule,
    /// Matrices retained in one exhaustive backend batch.
    pub backend_batch_matrix_count: usize,
    /// Production backend used for the statistical sample.
    pub sample_backend: Backend,
    /// Campaign-selected backends checked where mathematically supported.
    pub selectable_backends: Vec<Backend>,
}

/// One preregistered exhaustive and sampled anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorSpec {
    /// Prime field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
    /// Low-56-bit validation stream index.
    pub stream_index: u64,
    /// Complete exhaustive matrix count.
    pub expected_matrix_count: u64,
    /// Preregistered permanent-zero cross-check count.
    pub expected_permanent_zero_count: u64,
    /// Preregistered finite-size determinant singular count.
    pub expected_determinant_zero_count: u64,
}

/// Strict preregistration parsed before a validation address is opened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPreregistration {
    /// Preregistration schema.
    pub schema_version: u32,
    /// Shared constants and rules.
    pub protocol: ValidationProtocol,
    /// Anchors in canonical address order.
    pub anchors: Vec<AnchorSpec>,
    /// Content-bound protocol, manifest, and mechanical evidence.
    pub authorities: ValidationAuthorities,
}

impl ValidationPreregistration {
    /// Validates generic schema, count, address, and backend constraints.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != PREREGISTRATION_SCHEMA_VERSION {
            return invalid("unsupported preregistration schema");
        }
        if self.protocol.replay_matrix_count == 0
            || self.protocol.sample_matrix_count == 0
            || self.protocol.backend_batch_matrix_count == 0
        {
            return invalid("replay, sample, and backend batch counts must be nonzero");
        }
        if !(self.protocol.exact_test_level.is_finite()
            && 0.0 < self.protocol.exact_test_level
            && self.protocol.exact_test_level <= 1.0)
        {
            return invalid("exact test level must be finite and in (0, 1]");
        }
        if self.protocol.selectable_backends.is_empty() {
            return invalid("selectable backend inventory must be nonempty");
        }
        let mut backends = BTreeSet::new();
        for backend in &self.protocol.selectable_backends {
            if !Backend::campaign_inventory().contains(backend) || !backends.insert(backend.name())
            {
                return invalid("selectable backends must be unique schema backends");
            }
        }
        if !self
            .protocol
            .selectable_backends
            .contains(&self.protocol.sample_backend)
        {
            return invalid("sample backend must belong to the selectable inventory");
        }
        if self.anchors.is_empty() {
            return invalid("validation needs at least one anchor");
        }
        let mut addresses = BTreeSet::new();
        for anchor in &self.anchors {
            if !supported_anchor(anchor.q, anchor.n) {
                return invalid(format!(
                    "unsupported exact anchor q={} n={}",
                    anchor.q, anchor.n
                ));
            }
            if anchor.stream_index >= (1_u64 << 56)
                || !addresses.insert((anchor.q, anchor.n, anchor.stream_index))
            {
                return invalid("anchor addresses must be unique and fit the low 56 bits");
            }
            let expected_total = u64::from(anchor.q)
                .checked_pow(u32::from(anchor.n) * u32::from(anchor.n))
                .ok_or_else(|| {
                    ValidationError::InvalidPlan("anchor matrix count overflows".into())
                })?;
            if anchor.expected_matrix_count != expected_total
                || anchor.expected_permanent_zero_count > expected_total
            {
                return invalid(format!(
                    "anchor q={} n={} has inconsistent exhaustive counts",
                    anchor.q, anchor.n
                ));
            }
            if anchor.expected_determinant_zero_count
                != determinant_singular_count(anchor.q, anchor.n)
            {
                return invalid(format!(
                    "anchor q={} n={} determinant count disagrees with the finite-size formula",
                    anchor.q, anchor.n
                ));
            }
            if !backend_supports_cell(self.protocol.sample_backend, anchor.q, anchor.n) {
                return invalid(format!(
                    "sample backend {} does not support q={} n={}",
                    self.protocol.sample_backend.name(),
                    anchor.q,
                    anchor.n
                ));
            }
        }
        Ok(())
    }
}

/// Runtime-observed producer configuration recorded in journal state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRuntime {
    /// Source closure, binary, toolchain, RNG, invocation, and hardware.
    pub provenance: Provenance,
    /// Maximum worker count configured for production evaluation.
    pub worker_count: usize,
}

/// UTC-independent lossless system timestamp.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnixTimestamp {
    /// Whole seconds since the Unix epoch.
    pub seconds: u64,
    /// Nanoseconds within `seconds`.
    pub nanoseconds: u32,
}

/// Backend comparison state over an exhaustive anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendAgreementStatus {
    /// Every production value equals the independent oracle.
    Identical,
    /// At least one production value differs from the oracle.
    Mismatch,
    /// The backend's mathematical support excludes this cell.
    Unsupported,
    /// The backend supports the cell but could not execute.
    Unavailable,
}

/// Complete exhaustive evidence for one production backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendAgreement {
    /// Campaign backend.
    pub backend: Backend,
    /// Terminal comparison state.
    pub status: BackendAgreementStatus,
    /// Matrices compared before the terminal state.
    pub matrices_compared: u64,
    /// Per-matrix permanent mismatches.
    pub mismatch_count: u64,
    /// Production-path permanent-zero count when execution completed.
    pub production_permanent_zero_count: Option<u64>,
    /// Runtime diagnostic for an unavailable supported backend.
    pub diagnostic: Option<String>,
}

/// Complete exhaustive evidence for one anchor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactAnchorEvidence {
    /// Matrices emitted once by the independent visitor.
    pub enumerated_matrix_count: u64,
    /// Zero permanents from the independent fixed expansion.
    pub oracle_permanent_zero_count: u64,
    /// Singular matrices from the production determinant evaluator.
    pub production_determinant_zero_count: u64,
    /// Every preregistered backend, including unsupported states.
    pub backend_agreements: Vec<BackendAgreement>,
}

/// Two-fresh-instance sampler replay evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayEvidence {
    /// Worker-mode interpretation.
    pub mode: ReplayMode,
    /// Matrices regenerated by each fresh instance.
    pub matrix_count: u64,
    /// Entry-byte positions that differ.
    pub mismatch_count: u64,
    /// Whether every canonical row-major byte agrees.
    pub identical: bool,
    /// SHA-256 of the first byte stream.
    pub first_sha256: Sha256Digest,
    /// SHA-256 of the second byte stream.
    pub second_sha256: Sha256Digest,
}

/// Fixed-draw exact-test evidence; this is an engineering gate, not an estimate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleEvidence {
    /// Closed validation-purpose token.
    pub stream_purpose: ValidationStreamPurpose,
    /// Proof that sampling opened a third fresh address at ordinal zero.
    pub origin: SampleOrigin,
    /// Exact fixed draw count.
    pub matrix_count: u64,
    /// Sampled zero-permanent count.
    pub permanent_zero_count: u64,
    /// Runtime exhaustive null numerator.
    pub null_numerator: String,
    /// Runtime exhaustive null denominator.
    pub null_denominator: String,
    /// Natural logarithm of the probability-ordering exact p-value.
    pub log_p_value: f64,
    /// Exact-test p-value in the representable range.
    pub p_value: f64,
    /// Strict protocol verdict.
    pub verdict: ValidationVerdict,
}

/// Phase named by a preserved mechanical failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationPhase {
    /// Exhaustive oracle phase.
    ExactOracle,
    /// Production backend comparison phase.
    Backends,
    /// Production determinant phase.
    Determinant,
    /// Fresh-instance regeneration phase.
    Replay,
    /// Fixed statistical sampler phase.
    Sample,
}

/// Why an anchor could not complete every required phase.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationFailure {
    /// A durable phase marker exists without a terminal record.
    InterruptedAfterStart {
        /// Latest phase whose durable start marker was published.
        phase: ValidationPhase,
    },
    /// A production component returned a runtime failure.
    Mechanical {
        /// Phase that failed.
        phase: ValidationPhase,
        /// Runtime-observed diagnostic.
        diagnostic: String,
    },
}

/// Field-generic result before journal timestamps are attached.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidationAnchorOutcome {
    /// Independent exhaustive evidence.
    pub exact: Option<ExactAnchorEvidence>,
    /// Replay evidence when reached.
    pub replay: Option<ReplayEvidence>,
    /// Statistical evidence when reached.
    pub sample: Option<SampleEvidence>,
    /// Independent-oracle authority status.
    pub exact_oracle_status: PhaseStatus,
    /// Production backend agreement status.
    pub backend_status: PhaseStatus,
    /// Production determinant status.
    pub determinant_status: PhaseStatus,
    /// Replay status.
    pub replay_status: PhaseStatus,
    /// Statistical exact-test status.
    pub statistical_status: PhaseStatus,
    /// Preserved mechanical failure.
    pub failure: Option<ValidationFailure>,
    /// Combined terminal verdict.
    pub verdict: ValidationVerdict,
}

/// Terminal evidence for one preregistered address.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorReceipt {
    /// Complete validation-purpose stream address.
    pub address: StreamAddress,
    /// Time the durable start marker was published.
    pub started_at: UnixTimestamp,
    /// Time the terminal anchor record was published.
    pub finished_at: UnixTimestamp,
    /// Exhaustive evidence when completed.
    pub exact: Option<ExactAnchorEvidence>,
    /// Replay evidence when reached.
    pub replay: Option<ReplayEvidence>,
    /// Fixed-draw evidence when reached.
    pub sample: Option<SampleEvidence>,
    /// Independent-oracle authority status.
    pub exact_oracle_status: PhaseStatus,
    /// Production backend agreement status.
    pub backend_status: PhaseStatus,
    /// Production determinant status.
    pub determinant_status: PhaseStatus,
    /// Replay status.
    pub replay_status: PhaseStatus,
    /// Statistical exact-test status.
    pub statistical_status: PhaseStatus,
    /// Mechanical or interrupted failure.
    pub failure: Option<ValidationFailure>,
    /// Terminal anchor verdict.
    pub verdict: ValidationVerdict,
}

/// Content inventory of the frozen campaign directory at one instant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenArtifactSnapshot {
    /// Frozen directory rooted at the repository.
    pub root: ArtifactPath,
    /// Sorted complete file inventory and content identities.
    pub artifacts: Vec<ArtifactIdentity>,
}

/// Before/after guard proving validation did not change frozen campaign files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenArtifactGuard {
    /// Snapshot durably captured before validation.
    pub before: FrozenArtifactSnapshot,
    /// Snapshot captured after every anchor reached a terminal record.
    pub after: FrozenArtifactSnapshot,
    /// Exact inventory-and-content equality verdict.
    pub status: PhaseStatus,
}

/// Immutable launch-validation receipt reconstructed from terminal records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationReceipt {
    /// Receipt schema.
    pub schema_version: u32,
    /// Content identity of the committed preregistration.
    pub preregistration_identity: ArtifactIdentity,
    /// Exact protocol constants consumed by the run.
    pub preregistration: ValidationPreregistration,
    /// Runtime-observed producer identity.
    pub runtime: ValidationRuntime,
    /// Earliest journaled start.
    pub started_at: UnixTimestamp,
    /// Latest terminal anchor timestamp.
    pub finished_at: UnixTimestamp,
    /// Every terminal anchor in preregistered order.
    pub anchors: Vec<AnchorReceipt>,
    /// Frozen payload guard for the strict repository run.
    pub frozen_artifacts: Option<FrozenArtifactGuard>,
    /// Combined runtime verdict.
    pub overall_verdict: ValidationVerdict,
}

impl ValidationReceipt {
    /// Whether every anchor and the optional frozen-artifact guard passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.overall_verdict == ValidationVerdict::Passed
    }

    /// Semantically validates counts, decisions, identities, order, and state.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != RECEIPT_SCHEMA_VERSION {
            return invalid("unsupported validation receipt schema");
        }
        self.preregistration.validate()?;
        validate_runtime(&self.runtime)?;
        if self.anchors.len() != self.preregistration.anchors.len() {
            return invalid("receipt does not contain every preregistered anchor");
        }
        if self.started_at > self.finished_at {
            return invalid("receipt timestamps are reversed");
        }
        for (spec, anchor) in self.preregistration.anchors.iter().zip(&self.anchors) {
            if anchor.address != address(&self.preregistration.protocol, spec)
                || anchor.started_at < self.started_at
                || anchor.started_at > anchor.finished_at
                || anchor.finished_at > self.finished_at
            {
                return invalid("anchor address or timestamps disagree with the plan");
            }
            validate_anchor_receipt(&self.preregistration.protocol, spec, anchor)?;
        }
        if self.anchors.iter().map(|anchor| anchor.finished_at).max() != Some(self.finished_at) {
            return invalid("receipt finish time is not the latest terminal anchor");
        }
        let guard_passed = match &self.frozen_artifacts {
            Some(guard) => {
                let expected = if guard.before == guard.after {
                    PhaseStatus::Passed
                } else {
                    PhaseStatus::Failed
                };
                if guard.status != expected {
                    return invalid("frozen artifact guard verdict is inconsistent");
                }
                guard.status == PhaseStatus::Passed
            }
            None => true,
        };
        let expected = if guard_passed
            && self
                .anchors
                .iter()
                .all(|anchor| anchor.verdict == ValidationVerdict::Passed)
        {
            ValidationVerdict::Passed
        } else {
            ValidationVerdict::Failed
        };
        if self.overall_verdict != expected {
            return invalid("overall receipt verdict is inconsistent");
        }
        Ok(())
    }
}

/// Validation planning, execution, journal, or receipt error.
#[derive(Debug)]
pub enum ValidationError {
    /// The preregistration or receipt violates its semantic contract.
    InvalidPlan(String),
    /// A filesystem operation failed.
    Io {
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// JSON encoding or decoding failed.
    Json(serde_json::Error),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(message) => formatter.write_str(message),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(source) => Some(source),
            Self::InvalidPlan(_) => None,
        }
    }
}

/// Returns whether a compiler version may produce frozen validation evidence.
///
/// The frozen campaign pins its producing toolchain, so a receipt built with
/// any other compiler is refused. The runner checks this before it opens the
/// first address: discovering the mismatch afterwards would leave opened
/// addresses that the protocol forbids redrawing.
#[must_use]
pub fn is_frozen_validation_toolchain(compiler_version: &str) -> bool {
    compiler_version.starts_with(FROZEN_TOOLCHAIN_PREFIX)
}

/// Loads and content-validates a committed preregistration and its authorities.
pub fn load_validation_preregistration(
    repository: &Path,
    path: &Path,
) -> Result<(ValidationPreregistration, ArtifactIdentity), ValidationError> {
    if path.is_absolute() {
        return invalid("preregistration path must be repository-relative");
    }
    let bytes = read_bytes(&repository.join(path))?;
    let plan: ValidationPreregistration =
        serde_json::from_slice(&bytes).map_err(ValidationError::Json)?;
    plan.validate()?;
    for authority in [
        &plan.authorities.protocol,
        &plan.authorities.manifest,
        &plan.authorities.exact_anchors,
        &plan.authorities.backend_equivalence,
    ] {
        verify_identity(repository, authority)?;
    }
    let identity = ArtifactIdentity {
        path: artifact_path(path)?,
        sha256: digest(&bytes),
    };
    Ok((plan, identity))
}

/// Evaluates one anchor over the natural const-generic prime-field domain.
///
/// This reusable computation does not persist state. Launch tooling should use
/// [`run_validation`], which durably marks an address before calling it.
/// Statistical sampling always opens a fresh validation-purpose sampler at
/// ordinal zero and derives its null probability from this invocation's
/// exhaustive numerator and denominator.
pub fn evaluate_validation_anchor<const Q: u64>(
    protocol: &ValidationProtocol,
    spec: &AnchorSpec,
    worker_count: usize,
) -> Result<ValidationAnchorOutcome, ValidationError> {
    evaluate_validation_anchor_with_hook::<Q, _>(protocol, spec, worker_count, |_| Ok(()))
}

/// Runs or adopts every anchor through a durable no-redraw journal.
///
/// Runtime provenance is observed internally from the running executable,
/// source closure, build compiler, hardware, invocation, and linked RNG. A
/// caller can choose worker count but cannot inject provenance. If a prior
/// invocation left a phase marker without a terminal record, this invocation
/// preserves an interruption failure and does not reopen the address.
pub fn run_validation(
    preregistration: &ValidationPreregistration,
    preregistration_identity: ArtifactIdentity,
    worker_count: usize,
    state_directory: &Path,
) -> Result<ValidationReceipt, ValidationError> {
    preregistration.validate()?;
    let runtime = observe_validation_runtime(worker_count)?;
    create_directory_durable(state_directory)?;
    let proposed = RunState {
        schema_version: RECEIPT_SCHEMA_VERSION,
        preregistration_identity: preregistration_identity.clone(),
        preregistration: preregistration.clone(),
        runtime: runtime.clone(),
        started_at: now()?,
    };
    let run_state = publish_or_adopt(
        &state_directory.join("run-state.json"),
        &proposed,
        |existing| {
            existing.schema_version == proposed.schema_version
                && existing.preregistration_identity == proposed.preregistration_identity
                && existing.preregistration == proposed.preregistration
                && existing.runtime == proposed.runtime
        },
    )?;

    let mut anchors = Vec::with_capacity(preregistration.anchors.len());
    for spec in &preregistration.anchors {
        anchors.push(run_or_adopt_anchor(&run_state, spec, state_directory)?);
    }
    let finished_at = anchors
        .iter()
        .map(|anchor| anchor.finished_at)
        .max()
        .unwrap_or(run_state.started_at);
    let overall_verdict = combined_verdict(&anchors, true);
    let receipt = ValidationReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        preregistration_identity,
        preregistration: preregistration.clone(),
        runtime,
        started_at: run_state.started_at,
        finished_at,
        anchors,
        frozen_artifacts: None,
        overall_verdict,
    };
    receipt.validate()?;
    Ok(receipt)
}

/// Confirms every required backend can execute before an address is opened.
///
/// A backend the frozen manifest selects is required at each anchor its kernel
/// domain covers, and one that cannot build or run fails that anchor. Because
/// the protocol forbids redrawing a failed anchor, discovering an unusable
/// build after the first start marker is published would block the campaign
/// with no remedy inside this protocol. The frozen runner therefore proves each
/// required backend on one fixed all-zero matrix first and refuses to start
/// instead. The probe matrix is constructed, never sampled, so it consumes no
/// stream and reads no oracle.
///
/// # Errors
///
/// Returns [`ValidationError::InvalidPlan`] naming the first backend, cell, and
/// runtime diagnostic that prevents execution.
pub fn preflight_required_backends(
    preregistration: &ValidationPreregistration,
    worker_count: usize,
) -> Result<(), ValidationError> {
    preregistration.validate()?;
    if worker_count == 0 {
        return invalid("validation worker count must be positive");
    }
    for spec in &preregistration.anchors {
        for &backend in &preregistration.protocol.selectable_backends {
            if !backend_supports_cell(backend, spec.q, spec.n) {
                continue;
            }
            let probed = match spec.q {
                3 => probe_backend::<3>(spec, backend, worker_count),
                5 => probe_backend::<5>(spec, backend, worker_count),
                7 => probe_backend::<7>(spec, backend, worker_count),
                _ => return invalid("preregistration bounds the field"),
            };
            probed.map_err(|error| {
                ValidationError::InvalidPlan(format!(
                    "required backend {} cannot execute q={} n={}: {error}",
                    backend.name(),
                    spec.q,
                    spec.n
                ))
            })?;
        }
    }
    Ok(())
}

fn probe_backend<const Q: u64>(
    spec: &AnchorSpec,
    backend: Backend,
    worker_count: usize,
) -> Result<(), ScheduleError> {
    let evaluator = ProductionBackendEvaluator::new(spec.q, spec.n, backend, worker_count)?;
    let probe = vec![vec![Fp::<Q>::new(0); usize::from(spec.n).pow(2)]];
    if evaluator.evaluate(&probe)?.len() != probe.len() {
        return Err(ScheduleError::InvalidWorkItem(
            "a backend probe returned the wrong number of values".to_owned(),
        ));
    }
    Ok(())
}

/// Loads and validates the exact committed ten-anchor frozen plan.
pub fn load_frozen_campaign_validation_preregistration(
    repository: &Path,
    path: &Path,
) -> Result<(ValidationPreregistration, ArtifactIdentity), ValidationError> {
    let (plan, identity) = load_validation_preregistration(repository, path)?;
    validate_frozen_plan(repository, &plan)?;
    Ok((plan, identity))
}

/// Executes the exact frozen ten-anchor plan with a before/after payload guard.
///
/// The generic journal executes every anchor even when an earlier anchor has a
/// statistical failure. The frozen directory is only read. Its initial
/// content-addressed inventory is durably adopted before any anchor, and the
/// final receipt fails if any path or byte changes.
pub fn run_frozen_campaign_validation(
    repository: &Path,
    preregistration: &ValidationPreregistration,
    preregistration_identity: ArtifactIdentity,
    worker_count: usize,
    state_directory: &Path,
) -> Result<ValidationReceipt, ValidationError> {
    validate_frozen_plan(repository, preregistration)?;
    validate_frozen_toolchain(env!("GF2_BUILD_RUSTC_VERSION"))?;
    preflight_required_backends(preregistration, worker_count)?;
    create_directory_durable(state_directory)?;
    let observed_before = snapshot_frozen_campaign(repository)?;
    let before = publish_or_adopt(
        &state_directory.join("frozen-artifacts-start.json"),
        &observed_before,
        |existing| existing == &observed_before,
    )?;
    let mut receipt = run_validation(
        preregistration,
        preregistration_identity,
        worker_count,
        state_directory,
    )?;
    let after = snapshot_frozen_campaign(repository)?;
    let guard_status = if before == after {
        PhaseStatus::Passed
    } else {
        PhaseStatus::Failed
    };
    receipt.frozen_artifacts = Some(FrozenArtifactGuard {
        before,
        after,
        status: guard_status,
    });
    receipt.overall_verdict =
        combined_verdict(&receipt.anchors, guard_status == PhaseStatus::Passed);
    validate_frozen_receipt(repository, &receipt)?;
    Ok(receipt)
}

/// Atomically publishes or adopts an identical immutable receipt.
pub fn publish_validation_receipt_atomic(
    path: &Path,
    receipt: &ValidationReceipt,
) -> Result<(), ValidationError> {
    receipt.validate()?;
    let adopted = publish_or_adopt(path, receipt, |existing| existing == receipt)?;
    if adopted != *receipt {
        return invalid("existing validation receipt differs from reconstructed evidence");
    }
    Ok(())
}

/// Reads and semantically validates an immutable validation receipt.
pub fn read_validation_receipt(path: &Path) -> Result<ValidationReceipt, ValidationError> {
    let receipt: ValidationReceipt = read_json(path)?;
    receipt.validate()?;
    Ok(receipt)
}

/// Reads a final receipt and rechecks its binding to the frozen committed plan.
pub fn read_frozen_validation_receipt(
    repository: &Path,
    path: &Path,
) -> Result<ValidationReceipt, ValidationError> {
    let receipt = read_validation_receipt(path)?;
    validate_frozen_receipt(repository, &receipt)?;
    Ok(receipt)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunState {
    schema_version: u32,
    preregistration_identity: ArtifactIdentity,
    preregistration: ValidationPreregistration,
    runtime: ValidationRuntime,
    started_at: UnixTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhaseStart {
    schema_version: u32,
    address: StreamAddress,
    started_at: UnixTimestamp,
    phase: ValidationPhase,
}

fn run_or_adopt_anchor(
    run: &RunState,
    spec: &AnchorSpec,
    state_directory: &Path,
) -> Result<AnchorReceipt, ValidationError> {
    let stem = format!("q{}-n{:02}-s{}", spec.q, spec.n, spec.stream_index);
    let terminal_path = state_directory.join(format!("{stem}.terminal.json"));
    if terminal_path.exists() {
        let receipt: AnchorReceipt = read_json(&terminal_path)?;
        validate_anchor_receipt(&run.preregistration.protocol, spec, &receipt)?;
        return Ok(receipt);
    }
    let start_path = state_directory.join(format!("{stem}.exact.started.json"));
    if start_path.exists() {
        let start: PhaseStart = read_json(&start_path)?;
        if start.address != address(&run.preregistration.protocol, spec) {
            return invalid("existing anchor start marker names a different address");
        }
        let phase = latest_started_phase(state_directory, &stem, &start.address)?;
        let interrupted = interrupted_receipt(start, phase)?;
        return publish_or_adopt(&terminal_path, &interrupted, |existing| {
            existing == &interrupted
        });
    }

    let start = PhaseStart {
        schema_version: RECEIPT_SCHEMA_VERSION,
        address: address(&run.preregistration.protocol, spec),
        started_at: now()?,
        phase: ValidationPhase::ExactOracle,
    };
    let start = publish_or_adopt(&start_path, &start, |existing| existing == &start)?;
    let marker = |phase| {
        if phase == ValidationPhase::ExactOracle {
            return Ok(());
        }
        let phase_start = PhaseStart {
            schema_version: RECEIPT_SCHEMA_VERSION,
            address: start.address.clone(),
            started_at: now()?,
            phase,
        };
        let path = state_directory.join(format!("{stem}.{}.started.json", phase_file_token(phase)));
        publish_or_adopt(&path, &phase_start, |existing| existing == &phase_start).map(|_| ())
    };
    let outcome = match spec.q {
        3 => evaluate_validation_anchor_with_hook::<3, _>(
            &run.preregistration.protocol,
            spec,
            run.runtime.worker_count,
            marker,
        ),
        5 => evaluate_validation_anchor_with_hook::<5, _>(
            &run.preregistration.protocol,
            spec,
            run.runtime.worker_count,
            marker,
        ),
        7 => evaluate_validation_anchor_with_hook::<7, _>(
            &run.preregistration.protocol,
            spec,
            run.runtime.worker_count,
            marker,
        ),
        _ => unreachable!("preregistration bounds the field"),
    }?;
    let receipt = receipt_from_outcome(start, outcome)?;
    publish_or_adopt(&terminal_path, &receipt, |existing| existing == &receipt)
}

fn evaluate_validation_anchor_with_hook<const Q: u64, F>(
    protocol: &ValidationProtocol,
    spec: &AnchorSpec,
    worker_count: usize,
    mut before_phase: F,
) -> Result<ValidationAnchorOutcome, ValidationError>
where
    F: FnMut(ValidationPhase) -> Result<(), ValidationError>,
{
    if Q != u64::from(spec.q) || worker_count == 0 {
        return invalid("field parameter and positive worker count must match the anchor");
    }
    before_phase(ValidationPhase::ExactOracle)?;
    let exact = exact_anchor::<Q>(protocol, spec, worker_count);
    let exact_oracle_status = if exact.enumerated_matrix_count == spec.expected_matrix_count
        && exact.oracle_permanent_zero_count == spec.expected_permanent_zero_count
    {
        PhaseStatus::Passed
    } else {
        PhaseStatus::Failed
    };
    let determinant_status =
        if exact.production_determinant_zero_count == spec.expected_determinant_zero_count {
            PhaseStatus::Passed
        } else {
            PhaseStatus::Failed
        };
    let backend_status = if backend_agreements_pass(protocol, spec, &exact) {
        PhaseStatus::Passed
    } else {
        PhaseStatus::Failed
    };
    if [exact_oracle_status, determinant_status, backend_status].contains(&PhaseStatus::Failed) {
        return Ok(ValidationAnchorOutcome {
            exact: Some(exact),
            replay: None,
            sample: None,
            exact_oracle_status,
            backend_status,
            determinant_status,
            replay_status: PhaseStatus::Unexecuted,
            statistical_status: PhaseStatus::Unexecuted,
            failure: None,
            verdict: ValidationVerdict::Failed,
        });
    }

    before_phase(ValidationPhase::Replay)?;
    let replay = match Q {
        3 => replay::<3>(protocol, spec, FieldOrder::F3),
        5 => replay::<5>(protocol, spec, FieldOrder::F5),
        7 => replay::<7>(protocol, spec, FieldOrder::F7),
        _ => unreachable!("validated field"),
    };
    if !replay.identical {
        return Ok(ValidationAnchorOutcome {
            exact: Some(exact),
            replay: Some(replay),
            sample: None,
            exact_oracle_status,
            backend_status,
            determinant_status,
            replay_status: PhaseStatus::Failed,
            statistical_status: PhaseStatus::Unexecuted,
            failure: None,
            verdict: ValidationVerdict::Failed,
        });
    }

    before_phase(ValidationPhase::Sample)?;
    match sample(protocol, spec, &exact, worker_count) {
        Ok(sample) => {
            let statistical_status = if sample.verdict == ValidationVerdict::Passed {
                PhaseStatus::Passed
            } else {
                PhaseStatus::Failed
            };
            Ok(ValidationAnchorOutcome {
                exact: Some(exact),
                replay: Some(replay),
                verdict: sample.verdict,
                sample: Some(sample),
                exact_oracle_status,
                backend_status,
                determinant_status,
                replay_status: PhaseStatus::Passed,
                statistical_status,
                failure: None,
            })
        }
        Err(error) => Ok(ValidationAnchorOutcome {
            exact: Some(exact),
            replay: Some(replay),
            sample: None,
            exact_oracle_status,
            backend_status,
            determinant_status,
            replay_status: PhaseStatus::Passed,
            statistical_status: PhaseStatus::MechanicalFailure,
            failure: Some(ValidationFailure::Mechanical {
                phase: ValidationPhase::Sample,
                diagnostic: error.to_string(),
            }),
            verdict: ValidationVerdict::Failed,
        }),
    }
}

struct BackendState {
    backend: Backend,
    evaluator: Option<ProductionBackendEvaluator>,
    status: BackendAgreementStatus,
    matrices_compared: u64,
    mismatch_count: u64,
    histogram: Vec<u64>,
    zero_count: u64,
    ignored_determinant_zero_count: u64,
    diagnostic: Option<String>,
}

fn exact_anchor<const Q: u64>(
    protocol: &ValidationProtocol,
    spec: &AnchorSpec,
    worker_count: usize,
) -> ExactAnchorEvidence {
    let mut states = protocol
        .selectable_backends
        .iter()
        .copied()
        .map(|backend| {
            if !backend_supports_cell(backend, spec.q, spec.n) {
                BackendState {
                    backend,
                    evaluator: None,
                    status: BackendAgreementStatus::Unsupported,
                    matrices_compared: 0,
                    mismatch_count: 0,
                    histogram: vec![0; Q as usize],
                    zero_count: 0,
                    ignored_determinant_zero_count: 0,
                    diagnostic: None,
                }
            } else {
                match ProductionBackendEvaluator::new(spec.q, spec.n, backend, worker_count) {
                    Ok(evaluator) => BackendState {
                        backend,
                        evaluator: Some(evaluator),
                        status: BackendAgreementStatus::Identical,
                        matrices_compared: 0,
                        mismatch_count: 0,
                        histogram: vec![0; Q as usize],
                        zero_count: 0,
                        ignored_determinant_zero_count: 0,
                        diagnostic: None,
                    },
                    Err(error) => BackendState {
                        backend,
                        evaluator: None,
                        status: BackendAgreementStatus::Unavailable,
                        matrices_compared: 0,
                        mismatch_count: 0,
                        histogram: vec![0; Q as usize],
                        zero_count: 0,
                        ignored_determinant_zero_count: 0,
                        diagnostic: Some(error.to_string()),
                    },
                }
            }
        })
        .collect::<Vec<_>>();
    let mut matrices = Vec::<Vec<Fp<Q>>>::with_capacity(protocol.backend_batch_matrix_count);
    let mut oracle_values = Vec::with_capacity(protocol.backend_batch_matrix_count);
    let mut determinant_zero_count = 0_u64;
    let exact = try_visit_permanent_anchor_matrices::<Q, std::convert::Infallible, _>(
        usize::from(spec.n),
        |entries, oracle| {
            matrices.push(entries.to_vec());
            oracle_values.push(oracle.value());
            if matrices.len() == protocol.backend_batch_matrix_count {
                evaluate_exact_batch(
                    &matrices,
                    &oracle_values,
                    usize::from(spec.n),
                    &mut states,
                    &mut determinant_zero_count,
                );
                matrices.clear();
                oracle_values.clear();
            }
            Ok(())
        },
    )
    .expect("an infallible exhaustive visitor cannot fail");
    if !matrices.is_empty() {
        evaluate_exact_batch(
            &matrices,
            &oracle_values,
            usize::from(spec.n),
            &mut states,
            &mut determinant_zero_count,
        );
    }
    ExactAnchorEvidence {
        enumerated_matrix_count: exact
            .matrix_count()
            .to_string()
            .parse()
            .expect("supported anchor count fits u64"),
        oracle_permanent_zero_count: exact
            .zero_count()
            .to_string()
            .parse()
            .expect("supported anchor count fits u64"),
        production_determinant_zero_count: determinant_zero_count,
        backend_agreements: states
            .into_iter()
            .map(|state| BackendAgreement {
                backend: state.backend,
                status: state.status,
                matrices_compared: state.matrices_compared,
                mismatch_count: state.mismatch_count,
                production_permanent_zero_count: matches!(
                    state.status,
                    BackendAgreementStatus::Identical | BackendAgreementStatus::Mismatch
                )
                .then_some(state.zero_count),
                diagnostic: state.diagnostic,
            })
            .collect(),
    }
}

fn evaluate_exact_batch<const Q: u64>(
    matrices: &[Vec<Fp<Q>>],
    oracle_values: &[u64],
    n: usize,
    states: &mut [BackendState],
    determinant_zero_count: &mut u64,
) {
    for determinant in evaluate_production_determinants(matrices, n) {
        *determinant_zero_count += u64::from(determinant == 0);
    }
    for state in states {
        let Some(evaluator) = state.evaluator.as_ref() else {
            continue;
        };
        match evaluator.evaluate(matrices) {
            Ok(values) if values.len() == oracle_values.len() => {
                state.matrices_compared += values.len() as u64;
                for (&value, &oracle) in values.iter().zip(oracle_values) {
                    state.mismatch_count += u64::from(value != oracle);
                    pool_production_outcome(
                        &mut state.histogram,
                        &mut state.zero_count,
                        &mut state.ignored_determinant_zero_count,
                        value,
                        None,
                    );
                }
                if state.mismatch_count != 0 {
                    state.status = BackendAgreementStatus::Mismatch;
                }
            }
            Ok(values) => {
                state.status = BackendAgreementStatus::Unavailable;
                state.diagnostic = Some(format!(
                    "backend returned {} values for {} matrices",
                    values.len(),
                    oracle_values.len()
                ));
                state.evaluator = None;
            }
            Err(error) => {
                state.status = BackendAgreementStatus::Unavailable;
                state.diagnostic = Some(error.to_string());
                state.evaluator = None;
            }
        }
    }
}

fn backend_agreements_pass(
    protocol: &ValidationProtocol,
    spec: &AnchorSpec,
    exact: &ExactAnchorEvidence,
) -> bool {
    exact.backend_agreements.len() == protocol.selectable_backends.len()
        && exact
            .backend_agreements
            .iter()
            .zip(&protocol.selectable_backends)
            .all(|(agreement, expected_backend)| {
                agreement.backend == *expected_backend
                    && match agreement.status {
                        BackendAgreementStatus::Identical => {
                            agreement.matrices_compared == exact.enumerated_matrix_count
                                && agreement.mismatch_count == 0
                                && agreement.production_permanent_zero_count
                                    == Some(exact.oracle_permanent_zero_count)
                                && agreement.diagnostic.is_none()
                        }
                        BackendAgreementStatus::Unsupported => {
                            !backend_supports_cell(agreement.backend, spec.q, spec.n)
                                && agreement.matrices_compared == 0
                                && agreement.mismatch_count == 0
                                && agreement.production_permanent_zero_count.is_none()
                                && agreement.diagnostic.is_none()
                        }
                        BackendAgreementStatus::Mismatch | BackendAgreementStatus::Unavailable => {
                            false
                        }
                    }
            })
}

fn replay<const Q: u64>(
    protocol: &ValidationProtocol,
    spec: &AnchorSpec,
    field_order: FieldOrder,
) -> ReplayEvidence {
    let address = MatrixAddress::new(
        protocol.root_seed,
        field_order,
        usize::from(spec.n),
        protocol.stream_purpose.sampler(),
        StreamIndex::new(spec.stream_index).expect("validated stream index"),
    );
    let mut first = MatrixSampler::<Q>::new(address).expect("validated field address");
    let mut second = MatrixSampler::<Q>::new(address).expect("validated field address");
    let mut first_matrix = vec![Fp::<Q>::new(0); usize::from(spec.n).pow(2)];
    let mut second_matrix = first_matrix.clone();
    let mut first_hash = Sha256::new();
    let mut second_hash = Sha256::new();
    let mut mismatch_count = 0_u64;
    for _ in 0..protocol.replay_matrix_count {
        first.fill_next_matrix(&mut first_matrix);
        second.fill_next_matrix(&mut second_matrix);
        for (left, right) in first_matrix.iter().zip(&second_matrix) {
            let left = left.value() as u8;
            let right = right.value() as u8;
            first_hash.update([left]);
            second_hash.update([right]);
            mismatch_count += u64::from(left != right);
        }
    }
    let first_sha256 = format!("{:x}", first_hash.finalize())
        .parse()
        .expect("SHA-256 is canonical");
    let second_sha256 = format!("{:x}", second_hash.finalize())
        .parse()
        .expect("SHA-256 is canonical");
    ReplayEvidence {
        mode: ReplayMode::TwoFreshSerial,
        matrix_count: protocol.replay_matrix_count,
        mismatch_count,
        identical: mismatch_count == 0 && first_sha256 == second_sha256,
        first_sha256,
        second_sha256,
    }
}

fn sample(
    protocol: &ValidationProtocol,
    spec: &AnchorSpec,
    exact: &ExactAnchorEvidence,
    worker_count: usize,
) -> Result<SampleEvidence, ScheduleError> {
    let evaluated = evaluate_validation_sample(
        protocol.root_seed,
        spec.q,
        spec.n,
        spec.stream_index,
        protocol.sample_matrix_count,
        protocol.sample_backend,
        worker_count,
    )?;
    let record = evaluated.run.record;
    if record.stream_address.purpose_tag != StreamPurpose::Validation.tag()
        || record.matrix_count != protocol.sample_matrix_count
    {
        return Err(ScheduleError::InvalidWorkItem(
            "validation sampler returned a mismatched purpose or count".to_owned(),
        ));
    }
    let null_probability =
        exact.oracle_permanent_zero_count as f64 / exact.enumerated_matrix_count as f64;
    let test = two_sided_test(
        record.permanent_zero_count,
        record.matrix_count,
        null_probability,
    );
    let verdict = if test.rejects_at(protocol.exact_test_level) {
        ValidationVerdict::Failed
    } else {
        ValidationVerdict::Passed
    };
    Ok(SampleEvidence {
        stream_purpose: ValidationStreamPurpose::Validation,
        origin: SampleOrigin::FreshAddressStart,
        matrix_count: record.matrix_count,
        permanent_zero_count: record.permanent_zero_count,
        null_numerator: exact.oracle_permanent_zero_count.to_string(),
        null_denominator: exact.enumerated_matrix_count.to_string(),
        log_p_value: test.log_p_value(),
        p_value: test.log_p_value().exp(),
        verdict,
    })
}

fn validate_anchor_receipt(
    protocol: &ValidationProtocol,
    spec: &AnchorSpec,
    receipt: &AnchorReceipt,
) -> Result<(), ValidationError> {
    if receipt.address != address(protocol, spec) {
        return invalid("terminal anchor address differs from the preregistration");
    }
    if let Some(failure) = &receipt.failure {
        if receipt.verdict != ValidationVerdict::Failed || receipt.sample.is_some() {
            return invalid("mechanical failure evidence is inconsistent");
        }
        let expected_mechanical = match failure {
            ValidationFailure::InterruptedAfterStart { phase }
            | ValidationFailure::Mechanical { phase, .. } => *phase,
        };
        let statuses = phase_statuses(receipt);
        if status_for_phase(expected_mechanical, statuses) != PhaseStatus::MechanicalFailure {
            return invalid("failure phase does not match phase status");
        }
        return Ok(());
    }
    let Some(exact) = receipt.exact.as_ref() else {
        return invalid("a non-mechanical terminal anchor needs exact evidence");
    };
    if exact.backend_agreements.len() != protocol.selectable_backends.len() {
        return invalid("backend evidence does not cover the preregistered inventory");
    }
    let exact_status = if exact.enumerated_matrix_count == spec.expected_matrix_count
        && exact.oracle_permanent_zero_count == spec.expected_permanent_zero_count
    {
        PhaseStatus::Passed
    } else {
        PhaseStatus::Failed
    };
    let determinant_status =
        if exact.production_determinant_zero_count == spec.expected_determinant_zero_count {
            PhaseStatus::Passed
        } else {
            PhaseStatus::Failed
        };
    let backend_status = if backend_agreements_pass(protocol, spec, exact) {
        PhaseStatus::Passed
    } else {
        PhaseStatus::Failed
    };
    if receipt.exact_oracle_status != exact_status
        || receipt.determinant_status != determinant_status
        || receipt.backend_status != backend_status
    {
        return invalid("exact component verdicts are inconsistent");
    }
    if [exact_status, determinant_status, backend_status].contains(&PhaseStatus::Failed) {
        if receipt.replay.is_some()
            || receipt.sample.is_some()
            || receipt.replay_status != PhaseStatus::Unexecuted
            || receipt.statistical_status != PhaseStatus::Unexecuted
            || receipt.verdict != ValidationVerdict::Failed
        {
            return invalid("an exact component failure must stop this anchor before replay");
        }
        return Ok(());
    }
    let Some(replay) = receipt.replay.as_ref() else {
        return invalid("an exact-passing anchor needs replay evidence");
    };
    if replay.matrix_count != protocol.replay_matrix_count
        || replay.mode != ReplayMode::TwoFreshSerial
        || replay.identical
            != (replay.mismatch_count == 0 && replay.first_sha256 == replay.second_sha256)
    {
        return invalid("replay evidence is inconsistent");
    }
    let replay_status = if replay.identical {
        PhaseStatus::Passed
    } else {
        PhaseStatus::Failed
    };
    if receipt.replay_status != replay_status {
        return invalid("replay verdict is inconsistent");
    }
    if !replay.identical {
        if receipt.sample.is_some()
            || receipt.statistical_status != PhaseStatus::Unexecuted
            || receipt.verdict != ValidationVerdict::Failed
        {
            return invalid("a replay failure must stop before sampling");
        }
        return Ok(());
    }
    let Some(sample) = receipt.sample.as_ref() else {
        return invalid("a replay-passing anchor needs sample evidence");
    };
    let numerator = sample
        .null_numerator
        .parse::<u64>()
        .map_err(|_| ValidationError::InvalidPlan("invalid null numerator".into()))?;
    let denominator = sample
        .null_denominator
        .parse::<u64>()
        .map_err(|_| ValidationError::InvalidPlan("invalid null denominator".into()))?;
    if numerator != exact.oracle_permanent_zero_count
        || denominator != exact.enumerated_matrix_count
        || sample.stream_purpose != ValidationStreamPurpose::Validation
        || sample.origin != SampleOrigin::FreshAddressStart
        || sample.matrix_count != protocol.sample_matrix_count
        || sample.permanent_zero_count > sample.matrix_count
    {
        return invalid(
            "sample counts, purpose, origin, or null disagree with runtime exact evidence",
        );
    }
    let test = two_sided_test(
        sample.permanent_zero_count,
        sample.matrix_count,
        numerator as f64 / denominator as f64,
    );
    let expected_verdict = if test.rejects_at(protocol.exact_test_level) {
        ValidationVerdict::Failed
    } else {
        ValidationVerdict::Passed
    };
    let expected_status = if expected_verdict == ValidationVerdict::Passed {
        PhaseStatus::Passed
    } else {
        PhaseStatus::Failed
    };
    if sample.log_p_value.to_bits() != test.log_p_value().to_bits()
        || sample.p_value.to_bits() != test.log_p_value().exp().to_bits()
        || sample.verdict != expected_verdict
        || receipt.statistical_status != expected_status
        || receipt.verdict != expected_verdict
    {
        return invalid("sample p-value or strict decision is inconsistent");
    }
    Ok(())
}

fn validate_frozen_plan(
    repository: &Path,
    plan: &ValidationPreregistration,
) -> Result<(), ValidationError> {
    plan.validate()?;
    if plan.protocol.root_seed != FROZEN_VALIDATION_ROOT
        || plan.protocol.stream_purpose != ValidationStreamPurpose::Validation
        || plan.protocol.replay_matrix_count != 1_024
        || plan.protocol.sample_matrix_count != 400_000
        || plan.protocol.exact_test_level.to_bits() != 0.001_f64.to_bits()
        || plan.protocol.decision_rule
            != DecisionRule::ProbabilityOrderingExactTwoSidedStrictGreater
        || plan.protocol.retry_rule != RetryRule::NoRedraw
    {
        return invalid("frozen protocol constants do not match the ten-anchor plan");
    }
    for (identity, expected) in [
        (&plan.authorities.protocol, FROZEN_PROTOCOL_PATH),
        (&plan.authorities.manifest, FROZEN_MANIFEST_PATH),
        (&plan.authorities.exact_anchors, EXACT_ANCHORS_PATH),
        (
            &plan.authorities.backend_equivalence,
            BACKEND_EQUIVALENCE_PATH,
        ),
    ] {
        if identity.path.to_string() != expected {
            return invalid(format!("frozen authority must be {expected}"));
        }
        verify_identity(repository, identity)?;
    }
    if plan.anchors.len() != FROZEN_ANCHOR_CELLS.len() {
        return invalid("frozen plan must contain exactly ten anchors");
    }
    let enumerated = read_exact_anchor_authority(repository)?;
    for (spec, &(q, n)) in plan.anchors.iter().zip(FROZEN_ANCHOR_CELLS) {
        let Some(&(zero_count, matrix_count)) = enumerated.get(&(q, n)) else {
            return invalid(format!(
                "the exact-anchor evidence has no row for q={q} n={n}"
            ));
        };
        let expected = AnchorSpec {
            q,
            n,
            stream_index: 0,
            expected_matrix_count: matrix_count,
            expected_permanent_zero_count: zero_count,
            expected_determinant_zero_count: determinant_singular_count(q, n),
        };
        if *spec != expected {
            return invalid("frozen anchor order, address, or authority count differs");
        }
    }
    let manifest_path = repository.join(FROZEN_MANIFEST_PATH);
    let manifest_root = manifest_path
        .parent()
        .ok_or_else(|| ValidationError::InvalidPlan("manifest has no parent".into()))?;
    let manifest = read_manifest(manifest_root)
        .map_err(|error| ValidationError::InvalidPlan(error.to_string()))?;
    let selected = Backend::campaign_inventory()
        .iter()
        .copied()
        .filter(|backend| manifest.cells.iter().any(|cell| cell.backend == *backend))
        .collect::<Vec<_>>();
    if selected != plan.protocol.selectable_backends {
        return invalid("selectable backend set is not the frozen manifest union in schema order");
    }
    if plan.protocol.sample_backend != Backend::BatchParallel {
        return invalid("frozen statistical samples use the batch-parallel production path");
    }
    if selected.iter().any(|backend| {
        !plan
            .anchors
            .iter()
            .any(|anchor| backend_supports_cell(*backend, anchor.q, anchor.n))
    }) {
        return invalid("a manifest-selected backend has no supported validation anchor");
    }
    Ok(())
}

/// Enumerated zero count and universe size for one exact-anchor cell.
type EnumeratedCounts = (u64, u64);

/// Reads the committed exact-anchor evidence as `(q, n) -> (zeros, universe)`.
///
/// The caller verifies this artifact's content identity before this function
/// consumes it. Comment lines and the column header are skipped; every
/// remaining row supplies the field order, dimension, zero count, and matrix
/// count in its first four fields.
fn read_exact_anchor_authority(
    repository: &Path,
) -> Result<BTreeMap<(u8, u16), EnumeratedCounts>, ValidationError> {
    let bytes = read_bytes(&repository.join(EXACT_ANCHORS_PATH))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| ValidationError::InvalidPlan("exact-anchor evidence is not UTF-8".into()))?;
    let mut rows = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("field_order") {
            continue;
        }
        let mut fields = line.split(',');
        let mut column = |label: &str| -> Result<u64, ValidationError> {
            fields
                .next()
                .ok_or_else(|| {
                    ValidationError::InvalidPlan(format!("an exact-anchor row lacks its {label}"))
                })?
                .trim()
                .parse()
                .map_err(|_| {
                    ValidationError::InvalidPlan(format!(
                        "an exact-anchor {label} is not an integer"
                    ))
                })
        };
        let q = column("field order")?;
        let n = column("dimension")?;
        let zero_count = column("zero count")?;
        let matrix_count = column("matrix count")?;
        let cell = (
            u8::try_from(q).map_err(|_| {
                ValidationError::InvalidPlan("an exact-anchor field order exceeds u8".into())
            })?,
            u16::try_from(n).map_err(|_| {
                ValidationError::InvalidPlan("an exact-anchor dimension exceeds u16".into())
            })?,
        );
        if rows.insert(cell, (zero_count, matrix_count)).is_some() {
            return invalid("the exact-anchor evidence repeats a cell");
        }
    }
    Ok(rows)
}

fn validate_frozen_receipt(
    repository: &Path,
    receipt: &ValidationReceipt,
) -> Result<(), ValidationError> {
    receipt.validate()?;
    validate_frozen_plan(repository, &receipt.preregistration)?;
    let preregistration_path = repository.join(receipt.preregistration_identity.path.to_string());
    let (plan, identity) = load_frozen_campaign_validation_preregistration(
        repository,
        Path::new(&receipt.preregistration_identity.path.to_string()),
    )?;
    if !preregistration_path.is_file()
        || plan != receipt.preregistration
        || identity != receipt.preregistration_identity
    {
        return invalid("receipt is not bound to the committed frozen preregistration");
    }
    let guard = receipt.frozen_artifacts.as_ref().ok_or_else(|| {
        ValidationError::InvalidPlan("frozen receipt lacks artifact guard".into())
    })?;
    if guard.before.root.to_string() != FROZEN_CAMPAIGN_DIRECTORY
        || guard.after.root.to_string() != FROZEN_CAMPAIGN_DIRECTORY
    {
        return invalid("frozen artifact guard names the wrong directory");
    }
    validate_frozen_toolchain(&receipt.runtime.provenance.compiler_version)?;
    Ok(())
}

fn validate_frozen_toolchain(compiler_version: &str) -> Result<(), ValidationError> {
    if is_frozen_validation_toolchain(compiler_version) {
        return Ok(());
    }
    invalid(format!(
        "frozen validation needs a `{FROZEN_TOOLCHAIN_PREFIX}` build, not `{compiler_version}`"
    ))
}

fn observe_validation_runtime(worker_count: usize) -> Result<ValidationRuntime, ValidationError> {
    if worker_count == 0 {
        return invalid("validation worker count must be positive");
    }
    let repository = repository_top_level(Path::new(env!("CARGO_MANIFEST_DIR")))
        .map_err(|error| ValidationError::InvalidPlan(error.to_string()))?;
    let cpu = observe_cpu_identity().map_err(|source| ValidationError::Io {
        path: PathBuf::from("/proc/cpuinfo"),
        source,
    })?;
    let accelerator = observe_accelerator_identity();
    let invocation = env::args_os()
        .map(|argument| {
            argument.into_string().map_err(|_| {
                ValidationError::InvalidPlan("validation argv contains non-UTF-8 bytes".into())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let provenance = Provenance {
        git_revision: "0000000000000000000000000000000000000000"
            .parse()
            .expect("zero revision has the canonical shape"),
        binary_sha256: None,
        deps_source_revision: None,
        deps_source_dirty: None,
        compiler_version: env!("GF2_BUILD_RUSTC_VERSION").to_owned(),
        rng_algorithm: RngAlgorithm::ChaCha20,
        rng_version: RNG_VERSION.to_owned(),
        invocation,
        accelerator_runtime: accelerator.runtime,
        cpu_model: cpu.model,
        cpu_physical_cores: Some(cpu.physical_cores),
        cpu_logical_threads: Some(cpu.logical_threads),
        gpu_model: accelerator.model,
    };
    let provenance = observe_provenance(&repository, provenance)
        .map_err(|error| ValidationError::InvalidPlan(error.to_string()))?;
    let runtime = ValidationRuntime {
        provenance,
        worker_count,
    };
    validate_runtime(&runtime)?;
    Ok(runtime)
}

fn validate_runtime(runtime: &ValidationRuntime) -> Result<(), ValidationError> {
    let provenance = &runtime.provenance;
    if runtime.worker_count == 0
        || provenance.compiler_version.is_empty()
        || provenance.rng_algorithm != RngAlgorithm::ChaCha20
        || provenance.rng_version != RNG_VERSION
        || provenance.binary_sha256.is_none()
        || provenance.deps_source_revision.is_none()
        || provenance.deps_source_dirty.is_none()
        || provenance.invocation.is_empty()
        || provenance.cpu_model.is_empty()
        || provenance.cpu_physical_cores == Some(0)
        || provenance.cpu_logical_threads == Some(0)
    {
        return invalid("runtime identity is incomplete or uses the wrong RNG");
    }
    for available in [&provenance.accelerator_runtime, &provenance.gpu_model] {
        if matches!(available, Availability::Present { value } if value.is_empty()) {
            return invalid("present accelerator provenance must be nonempty");
        }
    }
    Ok(())
}

fn snapshot_frozen_campaign(repository: &Path) -> Result<FrozenArtifactSnapshot, ValidationError> {
    let root: ArtifactPath =
        FROZEN_CAMPAIGN_DIRECTORY
            .parse()
            .map_err(|error: super::schema::ArtifactPathError| {
                ValidationError::InvalidPlan(error.to_string())
            })?;
    let absolute = repository.join(FROZEN_CAMPAIGN_DIRECTORY);
    let mut artifacts = Vec::new();
    visit_snapshot(repository, &absolute, &mut artifacts)?;
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(FrozenArtifactSnapshot { root, artifacts })
}

fn visit_snapshot(
    repository: &Path,
    directory: &Path,
    artifacts: &mut Vec<ArtifactIdentity>,
) -> Result<(), ValidationError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|source| ValidationError::Io {
            path: directory.to_owned(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ValidationError::Io {
            path: directory.to_owned(),
            source,
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| ValidationError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            visit_snapshot(repository, &path, artifacts)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(repository).map_err(|_| {
                ValidationError::InvalidPlan("frozen artifact escaped repository".into())
            })?;
            artifacts.push(ArtifactIdentity {
                path: artifact_path(relative)?,
                sha256: digest(&read_bytes(&path)?),
            });
        } else {
            return invalid(format!(
                "frozen artifact inventory contains non-regular path {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn receipt_from_outcome(
    start: PhaseStart,
    outcome: ValidationAnchorOutcome,
) -> Result<AnchorReceipt, ValidationError> {
    Ok(AnchorReceipt {
        address: start.address,
        started_at: start.started_at,
        finished_at: now()?,
        exact: outcome.exact,
        replay: outcome.replay,
        sample: outcome.sample,
        exact_oracle_status: outcome.exact_oracle_status,
        backend_status: outcome.backend_status,
        determinant_status: outcome.determinant_status,
        replay_status: outcome.replay_status,
        statistical_status: outcome.statistical_status,
        failure: outcome.failure,
        verdict: outcome.verdict,
    })
}

fn interrupted_receipt(
    start: PhaseStart,
    phase: ValidationPhase,
) -> Result<AnchorReceipt, ValidationError> {
    let mut statuses = [PhaseStatus::Unexecuted; 5];
    statuses[phase_index(phase)] = PhaseStatus::MechanicalFailure;
    Ok(AnchorReceipt {
        address: start.address,
        started_at: start.started_at,
        finished_at: now()?,
        exact: None,
        replay: None,
        sample: None,
        exact_oracle_status: statuses[0],
        backend_status: statuses[1],
        determinant_status: statuses[2],
        replay_status: statuses[3],
        statistical_status: statuses[4],
        failure: Some(ValidationFailure::InterruptedAfterStart { phase }),
        verdict: ValidationVerdict::Failed,
    })
}

fn latest_started_phase(
    state_directory: &Path,
    stem: &str,
    expected_address: &StreamAddress,
) -> Result<ValidationPhase, ValidationError> {
    for phase in [ValidationPhase::Sample, ValidationPhase::Replay] {
        let path = state_directory.join(format!("{stem}.{}.started.json", phase_file_token(phase)));
        if path.exists() {
            let marker: PhaseStart = read_json(&path)?;
            if marker.phase != phase || marker.address != *expected_address {
                return invalid("phase marker disagrees with its anchor");
            }
            return Ok(phase);
        }
    }
    Ok(ValidationPhase::ExactOracle)
}

fn phase_file_token(phase: ValidationPhase) -> &'static str {
    match phase {
        ValidationPhase::ExactOracle => "exact",
        ValidationPhase::Backends => "backends",
        ValidationPhase::Determinant => "determinant",
        ValidationPhase::Replay => "replay",
        ValidationPhase::Sample => "sample",
    }
}

fn phase_index(phase: ValidationPhase) -> usize {
    match phase {
        ValidationPhase::ExactOracle => 0,
        ValidationPhase::Backends => 1,
        ValidationPhase::Determinant => 2,
        ValidationPhase::Replay => 3,
        ValidationPhase::Sample => 4,
    }
}

fn phase_statuses(receipt: &AnchorReceipt) -> [PhaseStatus; 5] {
    [
        receipt.exact_oracle_status,
        receipt.backend_status,
        receipt.determinant_status,
        receipt.replay_status,
        receipt.statistical_status,
    ]
}

fn status_for_phase(phase: ValidationPhase, statuses: [PhaseStatus; 5]) -> PhaseStatus {
    statuses[phase_index(phase)]
}

fn combined_verdict(anchors: &[AnchorReceipt], guard_passed: bool) -> ValidationVerdict {
    if guard_passed
        && anchors
            .iter()
            .all(|anchor| anchor.verdict == ValidationVerdict::Passed)
    {
        ValidationVerdict::Passed
    } else {
        ValidationVerdict::Failed
    }
}

fn supported_anchor(q: u8, n: u16) -> bool {
    (q == 3 && (1..=4).contains(&n)) || (matches!(q, 5 | 7) && (1..=3).contains(&n))
}

/// Projects the canonical finite-`n` singular probability to a machine count.
///
/// The formula itself lives in `gf2-algebra`; this adapter only narrows the
/// arbitrary-precision count for a preregistered anchor, whose universe is
/// bounded by construction.
fn determinant_singular_count(q: u8, n: u16) -> u64 {
    determinant_singular_probability(u64::from(q), usize::from(n))
        .zero_count()
        .to_string()
        .parse()
        .expect("a supported anchor's singular count fits u64")
}

fn address(protocol: &ValidationProtocol, spec: &AnchorSpec) -> StreamAddress {
    StreamAddress {
        root_seed: protocol.root_seed,
        q: spec.q,
        n: spec.n,
        purpose_tag: protocol.stream_purpose.sampler().tag(),
        stream_index: spec.stream_index,
    }
}

fn verify_identity(repository: &Path, identity: &ArtifactIdentity) -> Result<(), ValidationError> {
    let path = repository.join(identity.path.to_string());
    let bytes = read_bytes(&path)?;
    if digest(&bytes) != identity.sha256 {
        return invalid(format!("artifact identity mismatch for {}", identity.path));
    }
    Ok(())
}

fn artifact_path(path: &Path) -> Result<ArtifactPath, ValidationError> {
    path.to_str()
        .ok_or_else(|| ValidationError::InvalidPlan("artifact path is not UTF-8".into()))?
        .parse()
        .map_err(|error: super::schema::ArtifactPathError| {
            ValidationError::InvalidPlan(error.to_string())
        })
}

fn digest(bytes: &[u8]) -> Sha256Digest {
    format!("{:x}", Sha256::digest(bytes))
        .parse()
        .expect("SHA-256 is canonical")
}

fn now() -> Result<UnixTimestamp, ValidationError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            ValidationError::InvalidPlan(format!("system clock precedes Unix epoch: {error}"))
        })?;
    Ok(UnixTimestamp {
        seconds: duration.as_secs(),
        nanoseconds: duration.subsec_nanos(),
    })
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, ValidationError> {
    let mut file = File::open(path).map_err(|source| ValidationError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| ValidationError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(bytes)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, ValidationError> {
    serde_json::from_slice(&read_bytes(path)?).map_err(ValidationError::Json)
}

fn create_directory_durable(path: &Path) -> Result<(), ValidationError> {
    fs::create_dir_all(path).map_err(|source| ValidationError::Io {
        path: path.to_owned(),
        source,
    })?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        sync_directory(parent)?;
    }
    sync_directory(path)
}

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn publish_or_adopt<T, F>(path: &Path, value: &T, compatible: F) -> Result<T, ValidationError>
where
    T: Clone + DeserializeOwned + Serialize,
    F: Fn(&T) -> bool,
{
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_directory_durable(parent)?;
    if path.exists() {
        let existing: T = read_json(path)?;
        return if compatible(&existing) {
            Ok(existing)
        } else {
            invalid(format!("existing {} is incompatible", path.display()))
        };
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(ValidationError::Json)?;
    let timestamp = now()?;
    let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_extension(format!(
        "tmp-{}-{}-{}-{}",
        std::process::id(),
        timestamp.seconds,
        timestamp.nanoseconds,
        sequence
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|source| ValidationError::Io {
            path: temporary.clone(),
            source,
        })?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|source| ValidationError::Io {
            path: temporary.clone(),
            source,
        })?;
    match fs::hard_link(&temporary, path) {
        Ok(()) => sync_directory(parent)?,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => {
            return Err(ValidationError::Io {
                path: path.to_owned(),
                source,
            });
        }
    }
    fs::remove_file(&temporary).map_err(|source| ValidationError::Io {
        path: temporary,
        source,
    })?;
    sync_directory(parent)?;
    let existing: T = read_json(path)?;
    if compatible(&existing) {
        Ok(existing)
    } else {
        invalid(format!(
            "published {} differs from expected content",
            path.display()
        ))
    }
}

fn sync_directory(path: &Path) -> Result<(), ValidationError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| ValidationError::Io {
            path: path.to_owned(),
            source,
        })
}

fn invalid<T>(message: impl Into<String>) -> Result<T, ValidationError> {
    Err(ValidationError::InvalidPlan(message.into()))
}
