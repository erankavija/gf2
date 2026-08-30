//! Persisted outside-coordinator state for the frozen permanent campaign.
//!
//! The coordinator owns schedule admission, manifest-derived attempt history,
//! terminal cell state, campaign halts, and field interpretation sidecars. The
//! exact-cell emitter remains a subordinate arm and cannot finalize a field.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::acceptance::{
    assess_completed_cell, AcceptanceError, AcceptanceFamily, AcceptancePlan,
    CompletedCellAssessment,
};
use super::launch_cost::resolve_accelerator_cost_table;
use super::provenance::{approve_emission, EmissionApproval};
use super::root_fs::{CampaignRoot, EntryKind};
use super::schedule::CAMPAIGN_CELL_PURPOSE_TAG;
use super::schedule::{
    emit_shard_with_durability_hook, enumerate_cell_work_items,
    evaluate_work_item_with_worker_count_and_accelerator, AcceleratorConfig, AcceleratorCostTable,
    EvaluatedShard, ScheduleError, WorkItem, DEFAULT_ACCELERATOR_LAUNCH_CAP,
};
use super::schema::{
    field_summary_file, shard_record_file, AcceptanceVerdict, ArtifactIdentity, ArtifactPath,
    Backend, CampaignId, CampaignManifest, CellSpec, CellTerminalState, DeterminantCount,
    DeterminantPlan, FieldSummary, HaltReason, Interval, QuarantinedShard, RngAlgorithm,
    Sha256Digest, ShardRecord, StreamAddress, SummaryRow, DATASET_HOME, MANIFEST_FILE,
    SCHEMA_VERSION,
};

const COORDINATOR_SCHEMA_VERSION: u32 = 1;
const FIRST_FIELD: u8 = 7;
const FIRST_ORDER: u16 = 20;
const COORDINATOR_DIRECTORY: &str = "campaign-coordinator";
const COORDINATOR_RECEIPT_FILE: &str = "coordinator-receipt.json";
const Q3_TARGET_SCHEMA: &str = "scheinerman2024-q3-reproduction-targets-v1";
const PROTOCOL_Z_95: f64 = 1.959_963_984_540_054;
const Q3_TARGET_COMMENTS: [&str; 9] = [
    "schema",
    "source",
    "transcription_cross_check",
    "scope",
    "p_hat_from_source_counts",
    "reference_precision",
    "reference_interval",
    "interval_attribution",
    "limitations",
];
const Q3_TARGET_FIELDS: [&str; 15] = [
    "q",
    "n",
    "source_table",
    "source_evidence",
    "source_zero_count",
    "source_n",
    "source_reported_p_hat",
    "p_hat_from_source_counts",
    "source_reported_precision",
    "reference_precision_kind",
    "reference_precision_value",
    "reference_interval_kind",
    "reference_interval_level",
    "reference_interval_lower",
    "reference_interval_upper",
];

/// Expected content-bound identities of the interpretation sources.
///
/// The caller selects repository policy by supplying exact path-and-digest
/// identities. The coordinator descriptor-reads those paths, requires the
/// actual bytes to match, and validates their schemas before sampler admission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinatorEvidenceSources {
    /// Versioned exact q=3 target table identity.
    pub q3_targets: ArtifactIdentity,
    /// Recorded bounded q=5/q=7 literature-search identity.
    pub q5_q7_literature_search: ArtifactIdentity,
}

/// Exact effective invocation of one subordinate emitter arm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmInvocation {
    /// Exact semantic scope selected by the arm.
    pub scope: ExactCellScope,
    /// Exact argument vector, including executable token.
    pub argv: Vec<String>,
    /// Explicit worker count effective for the arm.
    pub worker_count: usize,
    /// SHA-256 of the exact emitting executable.
    pub executable_sha256: Sha256Digest,
    /// Canonical content identity of the accelerator cost input, when used.
    pub accelerator_cost_table: Option<ArtifactIdentity>,
}

/// Exact manifested cell selected by one campaign-purpose arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactCellScope {
    /// Prime field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
}

/// Raw sufficient statistics admitted with one mechanically valid shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardObservation {
    /// Number of matrices in this complete shard.
    pub matrix_count: u64,
    /// Number of zero permanent values.
    pub permanent_zero_count: u64,
    /// Determinant companion counts or manifested absence.
    pub determinant: DeterminantCount,
}

/// Lifecycle state of one persisted shard execution attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ShardAttemptState {
    /// The exact attempt is persisted and may enter the sampler once.
    Authorized,
    /// Mechanically valid shard admitted to raw data.
    Accepted {
        /// Content-bound canonical raw shard identity.
        record: ArtifactIdentity,
        /// Raw sufficient statistics parsed from the accepted shard.
        observation: ShardObservation,
    },
    /// Mechanically invalid attempt preserved as quarantine evidence.
    Quarantined {
        /// Stable mechanical diagnostic; never an acceptance-test result.
        error: String,
    },
}

/// One immutable initial or same-address recovery attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardAttempt {
    /// Complete manifested matrix-stream address.
    pub stream_address: StreamAddress,
    /// Manifest shard id.
    pub shard_id: u64,
    /// One-based attempt number, limited to one initial and one recovery.
    pub attempt: u8,
    /// Manifest-selected backend.
    pub backend: Backend,
    /// Frozen receipt that selected the backend.
    pub backend_receipt: ArtifactIdentity,
    /// Frozen generator algorithm.
    pub rng_algorithm: RngAlgorithm,
    /// Frozen generator implementation version.
    pub rng_version: String,
    /// Authorized, accepted, or mechanically quarantined lifecycle state.
    pub state: ShardAttemptState,
}

/// Persisted state of one frozen manifest cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CellExecutionState {
    /// No arm is admitted for the cell.
    Pending,
    /// An exact emitter arm is admitted and may have durable shard attempts.
    Scheduled {
        /// Index into the receipt's arm list.
        arm_index: usize,
    },
    /// The cell reached its fixed sample count and exact decisions.
    Completed {
        /// Canonical estimate/test/verdict evidence.
        assessment: CompletedCellAssessment,
        /// Raw source records bound to the terminal assessment.
        source_records: Vec<ArtifactIdentity>,
    },
    /// The cell has a protocol terminal halt without a final estimate.
    Halted {
        /// Preserved partial mechanical raw row.
        summary: SummaryRow,
        /// Raw source records completed before the halt.
        source_records: Vec<ArtifactIdentity>,
    },
}

/// Closed reason that prevents all further campaign scheduling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CampaignHaltCause {
    /// An exact preregistered acceptance decision rejected.
    Acceptance {
        /// Rejecting field.
        q: u8,
        /// Rejecting matrix order.
        n: u16,
        /// Every rejecting family.
        rejected_families: Vec<AcceptanceFamily>,
    },
    /// A backend or mechanical execution path terminally failed.
    Mechanical {
        /// Failing field.
        q: u8,
        /// Failing matrix order.
        n: u16,
        /// Closed mechanical halt category.
        reason: HaltReason,
    },
}

/// Campaign-wide launch state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CampaignHaltState {
    /// Further cells may be scheduled subject to the first-cell gate.
    Running,
    /// A terminal cause prevents all further campaign-purpose work.
    Halted {
        /// Preserved cause of the campaign halt.
        cause: CampaignHaltCause,
    },
}

/// Closed retry rule fixed in every coordinator receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryRule {
    /// One recovery reuses the same stream address and spends no added alpha.
    OneSameAddressRecoveryNoAdditionalAlpha,
}

/// One field's derived terminal progress in the canonical receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FieldExecutionState {
    /// At least one field cell is pending or scheduled.
    InProgress,
    /// Every field cell completed without a halt.
    Completed,
    /// Every field cell is terminal and at least one is halted.
    Halted,
}

/// One cell entry in the persisted receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinatorCell {
    /// Prime field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
    /// Current lifecycle state.
    pub execution: CellExecutionState,
}

/// One field entry in the persisted receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinatorField {
    /// Prime field order.
    pub q: u8,
    /// Derived lifecycle state.
    pub execution: FieldExecutionState,
}

/// Canonical persisted campaign coordinator/execution receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignCoordinatorReceipt {
    /// Coordinator document schema.
    pub schema_version: u32,
    /// Frozen campaign id.
    pub campaign_id: CampaignId,
    /// Exact root-manifest content identity.
    pub manifest_identity: ArtifactIdentity,
    /// Exact frozen-protocol content identity.
    pub protocol_identity: ArtifactIdentity,
    /// Validated field-interpretation sources bound before sampling.
    pub evidence_sources: CoordinatorEvidenceSources,
    /// Shared pre-draw acceptance allocation.
    pub acceptance_plan: AcceptancePlan,
    /// Predeclared same-address retry rule.
    pub retry_rule: RetryRule,
    /// Every effective exact-cell arm invocation.
    pub arms: Vec<ArmInvocation>,
    /// Every accepted or quarantined shard attempt.
    pub attempts: Vec<ShardAttempt>,
    /// Every manifest cell lifecycle state.
    pub cells: Vec<CoordinatorCell>,
    /// Every populated manifest field lifecycle state.
    pub fields: Vec<CoordinatorField>,
    /// Campaign-wide halt state.
    pub halt: CampaignHaltState,
}

/// In-memory coordinator bound to its frozen manifest and persisted receipt.
#[derive(Clone, Debug)]
pub struct CampaignCoordinator {
    manifest: CampaignManifest,
    receipt: CampaignCoordinatorReceipt,
}

/// Terminal exact-cell execution projected from the persisted receipt.
#[derive(Clone, Debug, PartialEq)]
pub struct ExactCellExecution {
    /// Exact manifested cell executed by this arm.
    pub scope: ExactCellScope,
    /// Canonical raw records in manifest shard order.
    pub records: Vec<ShardRecord>,
    /// Receipt terminal state for the exact cell.
    pub terminal_state: CellExecutionState,
}

#[derive(Clone, Debug)]
struct PersistedExactArmAuthorization {
    scope: ExactCellScope,
    manifest_identity: ArtifactIdentity,
    arm: ArmInvocation,
}

#[derive(Clone, Debug)]
struct PersistedShardAttemptAuthorization {
    arm: PersistedExactArmAuthorization,
    attempt: ShardAttempt,
}

/// Coordinator admission, lifecycle, or persistence failure.
#[derive(Debug)]
pub enum CoordinatorError {
    /// Semantic lifecycle or identity refusal.
    Refused(String),
    /// Acceptance assessment rejected malformed completed counts.
    Acceptance(AcceptanceError),
    /// Filesystem operation failed.
    Io {
        /// Path involved in the failed operation.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// JSON encoding or decoding failed.
    Json(serde_json::Error),
    /// Root manifest could not be read.
    Manifest(super::schema::SchemaError),
    /// Manifest work evaluation or raw emission failed.
    Schedule(ScheduleError),
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(message) => formatter.write_str(message),
            Self::Acceptance(error) => error.fmt(formatter),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json(error) => error.fmt(formatter),
            Self::Manifest(error) => error.fmt(formatter),
            Self::Schedule(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoordinatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Acceptance(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Json(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Schedule(error) => Some(error),
            Self::Refused(_) => None,
        }
    }
}

impl From<AcceptanceError> for CoordinatorError {
    fn from(error: AcceptanceError) -> Self {
        Self::Acceptance(error)
    }
}

impl From<ScheduleError> for CoordinatorError {
    fn from(error: ScheduleError) -> Self {
        Self::Schedule(error)
    }
}

impl CampaignCoordinator {
    /// Creates an empty coordinator from the on-disk frozen campaign.
    ///
    /// # Errors
    ///
    /// Refuses a manifest without the predeclared first cell, a campaign path
    /// mismatch, or unavailable manifest/protocol bytes.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(C log C)` time and `O(C)` space for `C` manifest cells.
    fn new_inner(
        campaign_root: &CampaignRoot,
        evidence_sources: CoordinatorEvidenceSources,
    ) -> Result<Self, CoordinatorError> {
        let manifest = read_manifest_anchored(campaign_root)?;
        validate_campaign_directory(campaign_root.path(), &manifest.campaign_id)?;
        let manifest_identity =
            identity_for_campaign_file(campaign_root, &manifest.campaign_id, MANIFEST_FILE)?;
        let protocol_identity = identity_for_protocol(campaign_root)?;
        let acceptance_plan = AcceptancePlan::for_manifest(&manifest)?;
        if !manifest
            .cells
            .iter()
            .any(|cell| (cell.q, cell.n) == (FIRST_FIELD, FIRST_ORDER))
        {
            return refused("manifest lacks required first cell q=7 n=20");
        }
        validate_frozen_paths(&manifest, &manifest_identity, &protocol_identity)?;
        let mut fields: Vec<_> = manifest.cells.iter().map(|cell| cell.q).collect();
        fields.sort_unstable();
        fields.dedup();
        let receipt = CampaignCoordinatorReceipt {
            schema_version: COORDINATOR_SCHEMA_VERSION,
            campaign_id: manifest.campaign_id.clone(),
            manifest_identity,
            protocol_identity,
            evidence_sources,
            acceptance_plan,
            retry_rule: RetryRule::OneSameAddressRecoveryNoAdditionalAlpha,
            arms: Vec::new(),
            attempts: Vec::new(),
            cells: manifest
                .cells
                .iter()
                .map(|cell| CoordinatorCell {
                    q: cell.q,
                    n: cell.n,
                    execution: CellExecutionState::Pending,
                })
                .collect(),
            fields: fields
                .into_iter()
                .map(|q| CoordinatorField {
                    q,
                    execution: FieldExecutionState::InProgress,
                })
                .collect(),
            halt: CampaignHaltState::Running,
        };
        Ok(Self { manifest, receipt })
    }

    /// Constructs an empty coordinator for receipt-state integration tests.
    #[cfg(test)]
    fn new(
        campaign_root: &Path,
        expected_sources: &CoordinatorEvidenceSources,
    ) -> Result<Self, CoordinatorError> {
        let root = open_campaign_root(campaign_root)?;
        let evidence_sources = resolve_evidence_sources(&root, expected_sources)?;
        Self::new_inner(&root, evidence_sources)
    }

    /// Returns the fixed shared acceptance plan.
    #[must_use]
    pub const fn acceptance_plan(&self) -> &AcceptancePlan {
        &self.receipt.acceptance_plan
    }

    /// Returns the canonical serializable receipt.
    #[must_use]
    pub const fn receipt(&self) -> &CampaignCoordinatorReceipt {
        &self.receipt
    }

    /// Returns one cell lifecycle state, if the manifest contains it.
    #[must_use]
    pub fn cell_state(&self, q: u8, n: u16) -> Option<&CellExecutionState> {
        self.receipt
            .cells
            .iter()
            .find(|cell| (cell.q, cell.n) == (q, n))
            .map(|cell| &cell.execution)
    }

    /// Returns the campaign-wide halt state.
    #[must_use]
    pub const fn halt_state(&self) -> &CampaignHaltState {
        &self.receipt.halt
    }

    /// Admits one exact-cell emitter arm under the frozen first-cell order.
    ///
    /// # Errors
    ///
    /// Refuses an unfrozen executable, ambiguous or conflicting selectors,
    /// duplicate scheduling, a halted campaign, or any arm before `q=7,n=20`
    /// is terminal.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(C + A)` for `C` cells and `A` argument tokens.
    fn authorize_arm_inner(&mut self, arm: ArmInvocation) -> Result<(), CoordinatorError> {
        let ExactCellScope { q, n } = arm.scope;
        validate_arm(&self.manifest, &arm, q, n)?;
        if !matches!(self.receipt.halt, CampaignHaltState::Running) {
            return refused("campaign is halted and admits no further arm");
        }
        if self
            .receipt
            .cells
            .iter()
            .any(|cell| matches!(cell.execution, CellExecutionState::Scheduled { .. }))
        {
            return refused("one exact cell must be terminal before another arm is admitted");
        }
        let first_terminal = self
            .cell_state(FIRST_FIELD, FIRST_ORDER)
            .is_some_and(is_terminal);
        if !first_terminal && (q, n) != (FIRST_FIELD, FIRST_ORDER) {
            return refused("q=7 n=20 must be terminal before another cell is scheduled");
        }
        let arm_index = self.receipt.arms.len();
        let cell = self.cell_mut(q, n)?;
        if !matches!(cell.execution, CellExecutionState::Pending) {
            return refused("cell is already scheduled or terminal");
        }
        cell.execution = CellExecutionState::Scheduled { arm_index };
        self.receipt.arms.push(arm);
        Ok(())
    }

    /// Admits a pending exact arm or verifies the identical persisted arm.
    ///
    /// # Errors
    ///
    /// Applies [`Self::authorize_arm`] to a pending cell. A scheduled cell is
    /// accepted only when its persisted arm is byte-for-byte identical. A
    /// terminal cell refuses another execution.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(C + A)` for receipt cells and arguments.
    fn authorize_or_resume_arm_inner(
        &mut self,
        arm: ArmInvocation,
    ) -> Result<(), CoordinatorError> {
        let scope = arm.scope;
        match self.cell_state(scope.q, scope.n) {
            Some(CellExecutionState::Pending) => self.authorize_arm_inner(arm),
            Some(CellExecutionState::Scheduled { arm_index }) => {
                if self.receipt.arms.get(*arm_index) == Some(&arm) {
                    Ok(())
                } else {
                    refused("scheduled arm differs from the persisted exact invocation")
                }
            }
            Some(CellExecutionState::Completed { .. } | CellExecutionState::Halted { .. }) => {
                if self.receipt.arms.iter().find(|prior| prior.scope == scope) == Some(&arm) {
                    Ok(())
                } else {
                    refused("terminal projection retry differs from its persisted exact arm")
                }
            }
            None => refused("arm scope does not name a manifest cell"),
        }
    }

    /// Test-support admission of one exact arm.
    #[cfg(test)]
    fn authorize_arm(&mut self, arm: ArmInvocation) -> Result<(), CoordinatorError> {
        self.authorize_arm_inner(arm)
    }

    /// Persists one exact shard-attempt authorization before sampler entry.
    ///
    /// The coordinator derives stream, backend, receipt, root seed, purpose,
    /// RNG, and attempt number. Attempt two is admitted only after attempt one
    /// is mechanically quarantined.
    ///
    /// # Errors
    ///
    /// Refuses unknown or unscheduled shards, an existing active attempt, a
    /// retry after acceptance, or a third attempt.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(C + S + T)` for manifest cells, shards, and prior attempts.
    fn authorize_attempt_inner(
        &mut self,
        q: u8,
        n: u16,
        shard_id: u64,
    ) -> Result<(), CoordinatorError> {
        if self
            .receipt
            .attempts
            .iter()
            .any(|attempt| matches!(attempt.state, ShardAttemptState::Authorized))
        {
            return refused("one persisted shard attempt is already authorized");
        }
        if !matches!(
            self.cell_state(q, n),
            Some(CellExecutionState::Scheduled { .. })
        ) {
            return refused("shard attempt requires a scheduled cell");
        }
        let cell = self.manifest_cell(q, n)?.clone();
        let shard = cell
            .shards
            .iter()
            .find(|shard| shard.shard_id == shard_id)
            .ok_or_else(|| CoordinatorError::Refused("manifest shard not found".to_owned()))?;
        let prior: Vec<_> = self
            .receipt
            .attempts
            .iter()
            .filter(|attempt| {
                (
                    attempt.stream_address.q,
                    attempt.stream_address.n,
                    attempt.shard_id,
                ) == (q, n, shard_id)
            })
            .collect();
        if prior.len() >= 2 {
            return refused("attempt exceeds the initial-plus-one-recovery rule");
        }
        if prior.len() == 1 && !matches!(prior[0].state, ShardAttemptState::Quarantined { .. }) {
            return refused("recovery requires one prior mechanical quarantine");
        }
        let attempt_number = u8::try_from(prior.len() + 1)
            .map_err(|_| CoordinatorError::Refused("attempt number overflow".to_owned()))?;
        let purpose_tag = campaign_purpose_tag(&self.manifest)?;
        self.receipt.attempts.push(ShardAttempt {
            stream_address: StreamAddress {
                root_seed: self.manifest.root_seed,
                q,
                n,
                purpose_tag,
                stream_index: shard.stream_index,
            },
            shard_id,
            attempt: attempt_number,
            backend: cell.backend,
            backend_receipt: cell.backend_receipt,
            rng_algorithm: self.manifest.provenance.rng_algorithm,
            rng_version: self.manifest.provenance.rng_version.clone(),
            state: ShardAttemptState::Authorized,
        });
        Ok(())
    }

    /// Terminalizes the active attempt as mechanically quarantined.
    ///
    /// A second quarantine terminalizes the cell and campaign without spending
    /// acceptance alpha.
    ///
    /// # Errors
    ///
    /// Refuses an empty diagnostic or a shard without one active authorization.
    fn record_quarantine_inner(
        &mut self,
        q: u8,
        n: u16,
        shard_id: u64,
        error: String,
    ) -> Result<(), CoordinatorError> {
        if error.trim().is_empty() {
            return refused("quarantine diagnostic must not be empty");
        }
        let attempt = self.active_attempt_mut(q, n, shard_id)?;
        let attempt_number = attempt.attempt;
        attempt.state = ShardAttemptState::Quarantined { error };
        if attempt_number == 2 {
            self.terminalize_mechanical(q, n, HaltReason::ExecutionFailure)?;
        }
        Ok(())
    }

    /// Reads, validates, hashes, and accepts the active emitted shard attempt.
    ///
    /// Accepted identity and counts derive only from exact canonical raw bytes.
    ///
    /// # Errors
    ///
    /// Refuses an absent active authorization or invalid raw evidence.
    fn record_accepted_inner(
        &mut self,
        campaign_root: &CampaignRoot,
        q: u8,
        n: u16,
        shard_id: u64,
    ) -> Result<(), CoordinatorError> {
        validate_campaign_directory(campaign_root.path(), &self.receipt.campaign_id)?;
        let cell = self.manifest_cell(q, n)?.clone();
        let shard = cell
            .shards
            .iter()
            .find(|shard| shard.shard_id == shard_id)
            .ok_or_else(|| CoordinatorError::Refused("manifest shard not found".to_owned()))?;
        let relative = shard_record_file(q, n, shard_id);
        let bytes = campaign_read(campaign_root, Path::new(&relative))?;
        let record: ShardRecord = serde_json::from_slice(&bytes).map_err(CoordinatorError::Json)?;
        validate_raw_record(&self.manifest, &cell, shard_id, shard.stream_index, &record)?;
        let identity = ArtifactIdentity {
            path: format!("{DATASET_HOME}/{}/{relative}", self.receipt.campaign_id)
                .parse()
                .map_err(|error| {
                    CoordinatorError::Refused(format!("invalid shard path: {error}"))
                })?,
            sha256: digest(&bytes),
        };
        let observation = ShardObservation {
            matrix_count: record.matrix_count,
            permanent_zero_count: record.permanent_zero_count,
            determinant: record.determinant,
        };
        self.active_attempt_mut(q, n, shard_id)?.state = ShardAttemptState::Accepted {
            record: identity,
            observation,
        };
        Ok(())
    }

    fn active_attempt_mut(
        &mut self,
        q: u8,
        n: u16,
        shard_id: u64,
    ) -> Result<&mut ShardAttempt, CoordinatorError> {
        self.receipt
            .attempts
            .iter_mut()
            .rev()
            .find(|attempt| {
                (
                    attempt.stream_address.q,
                    attempt.stream_address.n,
                    attempt.shard_id,
                ) == (q, n, shard_id)
                    && matches!(attempt.state, ShardAttemptState::Authorized)
            })
            .ok_or_else(|| {
                CoordinatorError::Refused("shard has no active authorized attempt".to_owned())
            })
    }

    fn persisted_arm_authorization(
        &self,
        scope: ExactCellScope,
    ) -> Result<PersistedExactArmAuthorization, CoordinatorError> {
        let Some(CellExecutionState::Scheduled { arm_index }) = self.cell_state(scope.q, scope.n)
        else {
            return refused("exact execution requires a persisted scheduled arm");
        };
        let arm = self
            .receipt
            .arms
            .get(*arm_index)
            .filter(|arm| arm.scope == scope)
            .cloned()
            .ok_or_else(|| {
                CoordinatorError::Refused(
                    "scheduled cell does not bind its persisted exact arm".to_owned(),
                )
            })?;
        Ok(PersistedExactArmAuthorization {
            scope,
            manifest_identity: self.receipt.manifest_identity.clone(),
            arm,
        })
    }

    fn persisted_attempt_authorization(
        &self,
        arm: &PersistedExactArmAuthorization,
        item: &WorkItem,
    ) -> Result<PersistedShardAttemptAuthorization, CoordinatorError> {
        if self.receipt.manifest_identity != arm.manifest_identity
            || arm.scope
                != (ExactCellScope {
                    q: item.q,
                    n: item.n,
                })
            || self
                .receipt
                .arms
                .iter()
                .find(|candidate| *candidate == &arm.arm)
                .is_none()
        {
            return refused("attempt authorization differs from its persisted exact arm");
        }
        let attempt = self
            .receipt
            .attempts
            .last()
            .filter(|attempt| {
                attempt.stream_address.q == item.q
                    && attempt.stream_address.n == item.n
                    && attempt.shard_id == item.shard_id
                    && attempt.stream_address.stream_index == item.stream_index
                    && matches!(attempt.state, ShardAttemptState::Authorized)
            })
            .cloned()
            .ok_or_else(|| {
                CoordinatorError::Refused(
                    "sampler entry lacks a persisted active shard authorization".to_owned(),
                )
            })?;
        Ok(PersistedShardAttemptAuthorization {
            arm: arm.clone(),
            attempt,
        })
    }

    /// Assesses raw completed counts and records immutable terminal evidence.
    ///
    /// # Errors
    ///
    /// Refuses counts not equal to the manifested fixed sample, counts that do
    /// not pool the accepted attempts, missing accepted shards, or determinant
    /// evidence contrary to the manifested plan.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(S + n + log N)` for `S` shards, matrix order `n`, and count `N`.
    fn record_completed_inner(
        &mut self,
        q: u8,
        n: u16,
        matrix_count: u64,
        permanent_zero_count: u64,
        determinant: DeterminantCount,
    ) -> Result<(), CoordinatorError> {
        if !matches!(
            self.cell_state(q, n),
            Some(CellExecutionState::Scheduled { .. })
        ) {
            return refused("completed counts require a scheduled cell");
        }
        let cell = self.manifest_cell(q, n)?.clone();
        let (pooled, sources) = self.pooled_accepted(&cell)?;
        if sources.len() != cell.shards.len() {
            return refused("completed cell lacks one accepted attempt for every shard");
        }
        if pooled
            != (ShardObservation {
                matrix_count,
                permanent_zero_count,
                determinant: determinant.clone(),
            })
        {
            return refused("completed counts differ from accepted shard evidence");
        }
        let assessment = assess_completed_cell(
            &self.receipt.acceptance_plan,
            &cell,
            matrix_count,
            permanent_zero_count,
            determinant,
        )?;
        self.cell_mut(q, n)?.execution = CellExecutionState::Completed {
            assessment: assessment.clone(),
            source_records: sources,
        };
        if assessment.rejected() {
            let mut rejected_families = Vec::new();
            if assessment.permanent.test.verdict == AcceptanceVerdict::Rejected {
                rejected_families.push(AcceptanceFamily::PermanentFloor);
            }
            if assessment
                .determinant
                .is_some_and(|value| value.test.verdict == AcceptanceVerdict::Rejected)
            {
                rejected_families.push(AcceptanceFamily::Determinant);
            }
            self.receipt.halt = CampaignHaltState::Halted {
                cause: CampaignHaltCause::Acceptance {
                    q,
                    n,
                    rejected_families,
                },
            };
            self.halt_other_cells(q, n, HaltReason::AcceptanceFailure)?;
        }
        self.refresh_fields();
        Ok(())
    }

    fn record_completed_from_attempts(&mut self, q: u8, n: u16) -> Result<(), CoordinatorError> {
        let cell = self.manifest_cell(q, n)?.clone();
        let (pooled, _) = self.pooled_accepted(&cell)?;
        self.record_completed_inner(
            q,
            n,
            pooled.matrix_count,
            pooled.permanent_zero_count,
            pooled.determinant,
        )
    }

    /// Test-support admission of one manifest-derived attempt authorization.
    ///
    /// Production campaign execution admits attempts only through
    /// [`execute_campaign_cell`].
    #[cfg(test)]
    fn authorize_attempt(&mut self, q: u8, n: u16, shard_id: u64) -> Result<(), CoordinatorError> {
        self.authorize_attempt_inner(q, n, shard_id)
    }

    /// Test-support terminalization of an active mechanical attempt.
    ///
    /// Production campaign execution records this state only through
    /// [`execute_campaign_cell`].
    #[cfg(test)]
    fn record_quarantine(
        &mut self,
        q: u8,
        n: u16,
        shard_id: u64,
        error: String,
    ) -> Result<(), CoordinatorError> {
        self.record_quarantine_inner(q, n, shard_id, error)
    }

    /// Test-support adoption of one active attempt's canonical raw bytes.
    ///
    /// Production campaign execution records this state only through
    /// [`execute_campaign_cell`].
    #[cfg(test)]
    fn record_accepted(
        &mut self,
        campaign_root: &Path,
        q: u8,
        n: u16,
        shard_id: u64,
    ) -> Result<(), CoordinatorError> {
        let root = open_campaign_root(campaign_root)?;
        self.record_accepted_inner(&root, q, n, shard_id)
    }

    /// Test-support assessment of counts pooled from accepted attempts.
    ///
    /// Production campaign execution assesses a complete cell only through
    /// [`execute_campaign_cell`].
    #[cfg(test)]
    fn record_completed(
        &mut self,
        q: u8,
        n: u16,
        matrix_count: u64,
        permanent_zero_count: u64,
        determinant: DeterminantCount,
    ) -> Result<(), CoordinatorError> {
        self.record_completed_inner(q, n, matrix_count, permanent_zero_count, determinant)
    }

    /// Builds a raw field summary only when every field cell is terminal.
    ///
    /// # Errors
    ///
    /// Refuses absent fields and fields with pending or scheduled cells.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(C + T)` for receipt cells and attempts.
    fn assemble_field_summary(&self, q: u8) -> Result<FieldSummary, CoordinatorError> {
        let mut rows = Vec::new();
        for cell in self.receipt.cells.iter().filter(|cell| cell.q == q) {
            let row = match &cell.execution {
                CellExecutionState::Completed { assessment, .. } => assessment.summary.clone(),
                CellExecutionState::Halted { summary, .. } => summary.clone(),
                _ => return refused("field summary requires every field cell to be terminal"),
            };
            rows.push(row);
        }
        if rows.is_empty() {
            return refused("manifest has no requested field");
        }
        rows.sort_by_key(|row| row.n);
        let mut quarantine_by_shard = BTreeMap::new();
        for attempt in &self.receipt.attempts {
            if let ShardAttemptState::Quarantined { error } = &attempt.state {
                if attempt.stream_address.q == q {
                    quarantine_by_shard.insert(
                        (attempt.stream_address.n, attempt.shard_id),
                        QuarantinedShard {
                            q,
                            n: attempt.stream_address.n,
                            shard_id: attempt.shard_id,
                            error: error.clone(),
                        },
                    );
                }
            }
        }
        let quarantined = quarantine_by_shard.into_values().collect();
        Ok(FieldSummary {
            schema_version: SCHEMA_VERSION,
            q,
            rows,
            quarantined,
        })
    }

    /// Persists a monotonic canonical receipt at its derived campaign path.
    ///
    /// An existing receipt may advance only by appending arms and attempts and
    /// by moving nonterminal states to terminal states. Frozen identities,
    /// plans, prior history, and prior terminal evidence are immutable.
    ///
    /// # Errors
    ///
    /// Refuses a mismatched campaign directory or non-monotonic overwrite and
    /// reports filesystem and JSON failures.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(C + A + T + B)` for cells, arms, attempts, and serialized bytes.
    #[cfg(test)]
    fn persist(&self, campaign_root: &Path) -> Result<(), CoordinatorError> {
        let root = open_campaign_root(campaign_root)?;
        let _lock = acquire_execution_lock(&root, &self.receipt.campaign_id)?;
        self.persist_locked(&root)
    }

    fn persist_locked(&self, campaign_root: &CampaignRoot) -> Result<(), CoordinatorError> {
        validate_campaign_directory(campaign_root.path(), &self.receipt.campaign_id)?;
        validate_receipt(&self.manifest, &self.receipt)?;
        validate_campaign_files(campaign_root, &self.manifest, &self.receipt)?;
        let relative = coordinator_receipt_relative(&self.receipt.campaign_id);
        if campaign_root_entry(campaign_root, &relative)? != EntryKind::Missing {
            let bytes = campaign_read(campaign_root, &relative)?;
            let prior: CampaignCoordinatorReceipt =
                serde_json::from_slice(&bytes).map_err(CoordinatorError::Json)?;
            validate_receipt(&self.manifest, &prior)?;
            validate_on_disk_attempts(campaign_root, &self.manifest, &prior)?;
            validate_monotonic_transition(&prior, &self.receipt)?;
        }
        atomic_json(campaign_root, &relative, &self.receipt)
    }

    /// Reads and validates a manifest-bound canonical receipt.
    ///
    /// # Errors
    ///
    /// Refuses a receipt whose identity, lifecycle evidence, or on-disk
    /// manifest digest differs from the frozen manifest.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(B + C + T)` for manifest bytes, cells, and attempts.
    pub fn read(campaign_root: &Path) -> Result<Self, CoordinatorError> {
        let root = open_campaign_root(campaign_root)?;
        Self::read_anchored(&root)
    }

    fn read_anchored(campaign_root: &CampaignRoot) -> Result<Self, CoordinatorError> {
        let manifest = read_manifest_anchored(campaign_root)?;
        validate_campaign_directory(campaign_root.path(), &manifest.campaign_id)?;
        let relative = coordinator_receipt_relative(&manifest.campaign_id);
        let bytes = campaign_read(campaign_root, &relative)?;
        let receipt: CampaignCoordinatorReceipt =
            serde_json::from_slice(&bytes).map_err(CoordinatorError::Json)?;
        validate_receipt(&manifest, &receipt)?;
        validate_campaign_files(campaign_root, &manifest, &receipt)?;
        Ok(Self { manifest, receipt })
    }

    fn manifest_cell(&self, q: u8, n: u16) -> Result<&CellSpec, CoordinatorError> {
        self.manifest
            .cells
            .iter()
            .find(|cell| (cell.q, cell.n) == (q, n))
            .ok_or_else(|| CoordinatorError::Refused("manifest cell not found".to_owned()))
    }

    fn cell_mut(&mut self, q: u8, n: u16) -> Result<&mut CoordinatorCell, CoordinatorError> {
        self.receipt
            .cells
            .iter_mut()
            .find(|cell| (cell.q, cell.n) == (q, n))
            .ok_or_else(|| CoordinatorError::Refused("manifest cell not found".to_owned()))
    }

    fn pooled_accepted(
        &self,
        cell: &CellSpec,
    ) -> Result<(ShardObservation, Vec<ArtifactIdentity>), CoordinatorError> {
        pooled_accepted_from_receipt(&self.receipt, cell)
    }

    fn terminalize_mechanical(
        &mut self,
        q: u8,
        n: u16,
        reason: HaltReason,
    ) -> Result<(), CoordinatorError> {
        let cell = self.manifest_cell(q, n)?.clone();
        let (partial, sources) = self.pooled_accepted(&cell)?;
        self.cell_mut(q, n)?.execution = CellExecutionState::Halted {
            summary: halted_row(&cell, partial, reason),
            source_records: sources,
        };
        self.receipt.halt = CampaignHaltState::Halted {
            cause: CampaignHaltCause::Mechanical { q, n, reason },
        };
        self.halt_other_cells(q, n, reason)?;
        self.refresh_fields();
        Ok(())
    }

    fn halt_other_cells(
        &mut self,
        terminal_q: u8,
        terminal_n: u16,
        reason: HaltReason,
    ) -> Result<(), CoordinatorError> {
        let targets: Vec<_> = self
            .receipt
            .cells
            .iter()
            .filter(|cell| {
                (cell.q, cell.n) != (terminal_q, terminal_n) && !is_terminal(&cell.execution)
            })
            .map(|cell| (cell.q, cell.n))
            .collect();
        for (q, n) in targets {
            let spec = self.manifest_cell(q, n)?.clone();
            let (partial, sources) = self.pooled_accepted(&spec)?;
            self.cell_mut(q, n)?.execution = CellExecutionState::Halted {
                summary: halted_row(&spec, partial, reason),
                source_records: sources,
            };
        }
        self.refresh_fields();
        Ok(())
    }

    fn refresh_fields(&mut self) {
        for field in &mut self.receipt.fields {
            let states: Vec<_> = self
                .receipt
                .cells
                .iter()
                .filter(|cell| cell.q == field.q)
                .map(|cell| &cell.execution)
                .collect();
            field.execution = if states.iter().all(|state| is_terminal(state)) {
                if states
                    .iter()
                    .any(|state| matches!(state, CellExecutionState::Halted { .. }))
                {
                    FieldExecutionState::Halted
                } else {
                    FieldExecutionState::Completed
                }
            } else {
                FieldExecutionState::InProgress
            };
        }
    }
}

/// Returns the canonical coordinator receipt path for one campaign id.
#[must_use]
pub fn coordinator_receipt_path(campaign_root: &Path, campaign_id: &CampaignId) -> PathBuf {
    campaign_root.join(coordinator_receipt_relative(campaign_id))
}

fn coordinator_receipt_relative(campaign_id: &CampaignId) -> PathBuf {
    PathBuf::from("derived")
        .join(campaign_id.to_string())
        .join(COORDINATOR_DIRECTORY)
        .join(COORDINATOR_RECEIPT_FILE)
}

/// Returns the synchronization lock path for one campaign executor.
///
/// The lock is synchronization only. All durable lifecycle state remains in
/// the coordinator receipt.
#[must_use]
pub fn coordinator_lock_path(campaign_root: &Path, campaign_id: &CampaignId) -> PathBuf {
    campaign_root.join(coordinator_lock_relative(campaign_id))
}

fn coordinator_lock_relative(campaign_id: &CampaignId) -> PathBuf {
    PathBuf::from("derived")
        .join(campaign_id.to_string())
        .join(COORDINATOR_DIRECTORY)
        .join("execution.lock")
}

fn acquire_execution_lock(
    campaign_root: &CampaignRoot,
    campaign_id: &CampaignId,
) -> Result<ExecutionLock, CoordinatorError> {
    let relative = coordinator_lock_relative(campaign_id);
    let lock_path = campaign_root.path().join(&relative);
    let lock_file =
        campaign_root
            .open_lock_file(&relative)
            .map_err(|source| CoordinatorError::Io {
                path: lock_path.clone(),
                source,
            })?;
    lock_file.try_lock().map_err(|source| {
        CoordinatorError::Refused(format!(
            "campaign execution lock is unavailable at {}: {source}",
            lock_path.display()
        ))
    })?;
    Ok(ExecutionLock(lock_file))
}

struct ExecutionLock(fs::File);

impl Drop for ExecutionLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Executes one exact campaign cell through the canonical receipt transaction.
///
/// The function holds the campaign execution lock across receipt revalidation,
/// attempt authorization, evaluation, durable raw emission, and attempt
/// terminalization. Every attempt authorization is persisted and re-read
/// before the sampler is entered. A durable raw shard left by an interrupted
/// process is adopted without another sampler entry.
///
/// # Errors
///
/// Refuses a non-live executable identity, lock contention, an invalid full
/// accelerator-cost snapshot, inconsistent durable raw evidence, or a receipt
/// transition failure. Evaluation and raw-emission failures consume the fixed
/// mechanical attempt and are preserved in the receipt.
///
/// # Panics
///
/// Does not intentionally panic.
///
/// # Complexity
///
/// Receipt work is `O(C + S + T)` for cells, selected shards, and attempts;
/// each admitted shard has the permanent and determinant cost documented by
/// [`evaluate_work_item_with_worker_count_and_accelerator`].
pub fn execute_campaign_cell(
    campaign_root: &Path,
    scope: ExactCellScope,
    worker_count: usize,
    expected_sources: &CoordinatorEvidenceSources,
) -> Result<ExactCellExecution, CoordinatorError> {
    let approval = approve_emission(campaign_root)
        .map_err(|error| CoordinatorError::Refused(format!("emission refused: {error}")))?;
    if !approval
        .campaign_root()
        .matches_path(campaign_root)
        .map_err(|source| CoordinatorError::Io {
            path: campaign_root.to_owned(),
            source,
        })?
    {
        return refused("live emission approval names a different campaign directory identity");
    }
    let root = approval.campaign_root().clone();
    let manifest = read_manifest_anchored(&root)?;
    let _lock = acquire_execution_lock(&root, &manifest.campaign_id)?;
    let effective_argv = std::env::args().collect();
    execute_campaign_cell_locked(
        LockedCellRequest {
            campaign_root: &root,
            scope,
            worker_count,
            effective_argv,
            approval,
            expected_sources,
        },
        evaluate_work_item_with_worker_count_and_accelerator,
        |_| {},
    )
}

/// Deterministic evaluator seam for receipt-state-machine integration tests.
///
/// This entry point has the same persisted admission, locking, retry, and raw
/// durability behavior as [`execute_campaign_cell`]. The test supplies a real
/// live approval, exact argv, evaluator, and post-durability observer.
///
/// # Errors
///
/// Returns the same failures as [`execute_campaign_cell`] plus failures from
/// `evaluator`.
///
/// # Panics
///
/// Does not intentionally panic.
///
/// # Complexity
///
/// Adds `O(1)` dispatch overhead per authorized attempt to the evaluator's
/// cost.
#[cfg(any(test, feature = "test-support"))]
#[allow(clippy::too_many_arguments)]
pub fn execute_campaign_cell_with_evaluator<E, H>(
    campaign_root: &Path,
    scope: ExactCellScope,
    worker_count: usize,
    effective_argv: Vec<String>,
    approval: EmissionApproval,
    expected_sources: &CoordinatorEvidenceSources,
    evaluator: E,
    on_raw_durable: H,
) -> Result<ExactCellExecution, CoordinatorError>
where
    E: FnMut(
        &CampaignManifest,
        &WorkItem,
        usize,
        Option<AcceleratorConfig>,
    ) -> Result<EvaluatedShard, ScheduleError>,
    H: FnMut(&Path),
{
    if !approval
        .campaign_root()
        .matches_path(campaign_root)
        .map_err(|source| CoordinatorError::Io {
            path: campaign_root.to_owned(),
            source,
        })?
    {
        return refused("live emission approval names a different campaign directory identity");
    }
    let root = approval.campaign_root().clone();
    let manifest = read_manifest_anchored(&root)?;
    let _lock = acquire_execution_lock(&root, &manifest.campaign_id)?;
    execute_campaign_cell_locked(
        LockedCellRequest {
            campaign_root: &root,
            scope,
            worker_count,
            effective_argv,
            approval,
            expected_sources,
        },
        evaluator,
        on_raw_durable,
    )
}

struct LockedCellRequest<'a> {
    campaign_root: &'a CampaignRoot,
    scope: ExactCellScope,
    worker_count: usize,
    effective_argv: Vec<String>,
    approval: EmissionApproval,
    expected_sources: &'a CoordinatorEvidenceSources,
}

fn execute_campaign_cell_locked<E, H>(
    request: LockedCellRequest<'_>,
    mut evaluator: E,
    mut on_raw_durable: H,
) -> Result<ExactCellExecution, CoordinatorError>
where
    E: FnMut(
        &CampaignManifest,
        &WorkItem,
        usize,
        Option<AcceleratorConfig>,
    ) -> Result<EvaluatedShard, ScheduleError>,
    H: FnMut(&Path),
{
    let LockedCellRequest {
        campaign_root,
        scope,
        worker_count,
        effective_argv,
        approval,
        expected_sources,
    } = request;
    let manifest_on_disk = read_manifest_anchored(campaign_root)?;
    let observed_sources = resolve_evidence_sources(campaign_root, expected_sources)?;
    let receipt_relative = coordinator_receipt_relative(&manifest_on_disk.campaign_id);
    let mut coordinator = match campaign_root_entry(campaign_root, &receipt_relative)? {
        EntryKind::Missing => {
            CampaignCoordinator::new_inner(campaign_root, observed_sources.clone())?
        }
        EntryKind::RegularFile => CampaignCoordinator::read_anchored(campaign_root)?,
        EntryKind::Other => return refused("coordinator receipt path is not a regular file"),
    };
    let manifest = coordinator.manifest.clone();
    if coordinator.receipt.manifest_identity.sha256 != *approval.manifest_sha256() {
        return refused("live emission approval names different committed manifest bytes");
    }
    if coordinator.receipt.evidence_sources != observed_sources {
        return refused("execution interpretation sources differ from the persisted receipt");
    }
    publish_terminal_fields_locked(campaign_root, &coordinator)?;
    let (accelerator_costs, accelerator_cost_table) =
        accelerator_costs_for_argv(&manifest, &effective_argv, approval.repository_root())?;
    accelerator_costs.validate_manifest(&manifest)?;
    let requested_arm = ArmInvocation {
        scope,
        argv: effective_argv,
        worker_count,
        executable_sha256: approval.binary_sha256().clone(),
        accelerator_cost_table,
    };
    coordinator.authorize_or_resume_arm_inner(requested_arm)?;
    if coordinator
        .cell_state(scope.q, scope.n)
        .is_some_and(is_terminal)
    {
        let items = enumerate_cell_work_items(&manifest, scope.q, scope.n)?;
        return exact_execution(campaign_root, &coordinator, scope, &items);
    }
    coordinator.persist_locked(campaign_root)?;
    coordinator = CampaignCoordinator::read_anchored(campaign_root)?;
    let arm_authorization = coordinator.persisted_arm_authorization(scope)?;
    if worker_count != arm_authorization.arm.worker_count {
        return refused("runtime worker count differs from the persisted exact arm");
    }
    let items = enumerate_cell_work_items(&manifest, scope.q, scope.n)?;
    for item in &items {
        loop {
            if accepted_attempt(&coordinator.receipt, item).is_some() {
                break;
            }
            if !matches!(
                coordinator.cell_state(scope.q, scope.n),
                Some(CellExecutionState::Scheduled { .. })
            ) {
                publish_terminal_fields_locked(campaign_root, &coordinator)?;
                return exact_execution(campaign_root, &coordinator, scope, &items);
            }

            if let Some(active) = active_attempt(&coordinator.receipt, item).cloned() {
                let raw_relative =
                    PathBuf::from(shard_record_file(scope.q, scope.n, item.shard_id));
                let raw_kind = campaign_root_entry(campaign_root, &raw_relative)?;
                if raw_kind == EntryKind::Other {
                    return refused("canonical raw shard path must be a regular file");
                }
                if raw_kind == EntryKind::RegularFile {
                    match coordinator.record_accepted_inner(
                        campaign_root,
                        scope.q,
                        scope.n,
                        item.shard_id,
                    ) {
                        Ok(()) => {
                            coordinator.persist_locked(campaign_root)?;
                            continue;
                        }
                        Err(error) => {
                            preserve_invalid_raw(
                                campaign_root,
                                &coordinator.receipt.campaign_id,
                                item,
                                active.attempt,
                                &raw_relative,
                            )?;
                            coordinator.record_quarantine_inner(
                                scope.q,
                                scope.n,
                                item.shard_id,
                                format!("durable raw shard failed canonical validation: {error}"),
                            )?;
                            coordinator.persist_locked(campaign_root)?;
                            continue;
                        }
                    }
                }
                coordinator.record_quarantine_inner(
                    scope.q,
                    scope.n,
                    item.shard_id,
                    "authorized attempt ended without a durable raw shard".to_owned(),
                )?;
                coordinator.persist_locked(campaign_root)?;
                continue;
            }

            let raw_relative = PathBuf::from(shard_record_file(scope.q, scope.n, item.shard_id));
            if campaign_root_entry(campaign_root, &raw_relative)? != EntryKind::Missing {
                return refused("raw shard path exists without an active or accepted attempt");
            }
            coordinator.authorize_attempt_inner(scope.q, scope.n, item.shard_id)?;
            coordinator.persist_locked(campaign_root)?;
            coordinator = CampaignCoordinator::read_anchored(campaign_root)?;
            let attempt_authorization =
                coordinator.persisted_attempt_authorization(&arm_authorization, item)?;
            let accelerator = if item.backend == Backend::Accelerator {
                Some(accelerator_costs.config_for(item.q, item.n)?)
            } else {
                None
            };
            match evaluate_authorized_attempt(
                &attempt_authorization,
                &manifest,
                item,
                worker_count,
                accelerator,
                &mut evaluator,
            ) {
                Ok(evaluated) => {
                    let execution_result = validate_evaluated_shard(&manifest, item, &evaluated)
                        .and_then(|()| {
                            emit_shard_with_durability_hook(
                                campaign_root,
                                &manifest,
                                &evaluated.run,
                                &mut on_raw_durable,
                            )
                            .map(|_| ())
                            .map_err(CoordinatorError::Schedule)
                        });
                    match execution_result {
                        Ok(()) => {
                            coordinator.record_accepted_inner(
                                campaign_root,
                                scope.q,
                                scope.n,
                                item.shard_id,
                            )?;
                            coordinator.persist_locked(campaign_root)?;
                        }
                        Err(error) => {
                            let raw_relative =
                                PathBuf::from(shard_record_file(scope.q, scope.n, item.shard_id));
                            if campaign_root_entry(campaign_root, &raw_relative)?
                                == EntryKind::RegularFile
                            {
                                preserve_invalid_raw(
                                    campaign_root,
                                    &coordinator.receipt.campaign_id,
                                    item,
                                    attempt_authorization.attempt.attempt,
                                    &raw_relative,
                                )?;
                            }
                            coordinator.record_quarantine_inner(
                                scope.q,
                                scope.n,
                                item.shard_id,
                                error.to_string(),
                            )?;
                            coordinator.persist_locked(campaign_root)?;
                        }
                    }
                }
                Err(error) => {
                    coordinator.record_quarantine_inner(
                        scope.q,
                        scope.n,
                        item.shard_id,
                        error.to_string(),
                    )?;
                    coordinator.persist_locked(campaign_root)?;
                }
            }
        }
    }
    coordinator.record_completed_from_attempts(scope.q, scope.n)?;
    coordinator.persist_locked(campaign_root)?;
    publish_terminal_fields_locked(campaign_root, &coordinator)?;
    exact_execution(campaign_root, &coordinator, scope, &items)
}

fn validate_evaluated_shard(
    manifest: &CampaignManifest,
    item: &WorkItem,
    evaluated: &EvaluatedShard,
) -> Result<(), CoordinatorError> {
    let cell = manifest
        .cells
        .iter()
        .find(|cell| (cell.q, cell.n) == (item.q, item.n))
        .ok_or_else(|| CoordinatorError::Refused("authorized cell is not manifested".to_owned()))?;
    validate_raw_record(
        manifest,
        cell,
        item.shard_id,
        item.stream_index,
        &evaluated.run.record,
    )
}

fn accelerator_costs_for_argv(
    manifest: &CampaignManifest,
    argv: &[String],
    repository_root: &Path,
) -> Result<(AcceleratorCostTable, Option<ArtifactIdentity>), CoordinatorError> {
    let launch_cap = match optional_unique_option(argv, "--accelerator-launch-cap-ms")? {
        Some(value) => {
            let milliseconds = value.parse::<u64>().map_err(|_| {
                CoordinatorError::Refused(
                    "persisted accelerator launch cap is not an integer".to_owned(),
                )
            })?;
            if milliseconds == 0 {
                return refused("persisted accelerator launch cap must be positive");
            }
            Duration::from_millis(milliseconds)
        }
        None => DEFAULT_ACCELERATOR_LAUNCH_CAP,
    };
    match optional_unique_option(argv, "--accelerator-cost-table")? {
        Some(path) => {
            let resolved = resolve_accelerator_cost_table(
                repository_root,
                Path::new(&path),
                manifest,
                launch_cap,
            )
            .map_err(|error| {
                CoordinatorError::Refused(format!(
                    "persisted accelerator cost table {path} is invalid: {error}"
                ))
            })?;
            Ok((resolved.table, Some(resolved.identity)))
        }
        None => Ok((AcceleratorCostTable::default(), None)),
    }
}

fn validate_accelerator_cost_identity(
    campaign_root: &CampaignRoot,
    manifest: &CampaignManifest,
    arm: &ArmInvocation,
) -> Result<(), CoordinatorError> {
    if optional_unique_option(&arm.argv, "--accelerator-cost-table")?.is_none()
        && arm.accelerator_cost_table.is_none()
    {
        return Ok(());
    }
    let repository_root = repository_root_for_campaign(campaign_root.path())?;
    let (_, observed) = accelerator_costs_for_argv(manifest, &arm.argv, &repository_root)?;
    if observed != arm.accelerator_cost_table {
        return refused("accelerator cost table identity differs from persisted arm evidence");
    }
    Ok(())
}

fn repository_root_for_campaign(campaign_root: &Path) -> Result<PathBuf, CoordinatorError> {
    let campaign = fs::canonicalize(campaign_root).map_err(|source| CoordinatorError::Io {
        path: campaign_root.to_owned(),
        source,
    })?;
    let mut repository = campaign.as_path();
    for _ in 0..=Path::new(DATASET_HOME).components().count() {
        repository = repository.parent().ok_or_else(|| {
            CoordinatorError::Refused(
                "campaign path cannot resolve a canonical repository root".to_owned(),
            )
        })?;
    }
    let repository = repository.to_owned();
    if repository
        .join(DATASET_HOME)
        .join(campaign.file_name().ok_or_else(|| {
            CoordinatorError::Refused("campaign path has no directory name".to_owned())
        })?)
        != campaign
    {
        return refused("campaign path is not at the canonical dataset location");
    }
    Ok(repository)
}

fn evaluate_authorized_attempt<E>(
    authorization: &PersistedShardAttemptAuthorization,
    manifest: &CampaignManifest,
    item: &WorkItem,
    worker_count: usize,
    accelerator: Option<AcceleratorConfig>,
    evaluator: &mut E,
) -> Result<EvaluatedShard, ScheduleError>
where
    E: FnMut(
        &CampaignManifest,
        &WorkItem,
        usize,
        Option<AcceleratorConfig>,
    ) -> Result<EvaluatedShard, ScheduleError>,
{
    debug_assert_eq!(authorization.arm.scope.q, item.q);
    debug_assert_eq!(authorization.arm.scope.n, item.n);
    debug_assert_eq!(authorization.attempt.shard_id, item.shard_id);
    debug_assert_eq!(
        authorization.attempt.stream_address.stream_index,
        item.stream_index
    );
    evaluator(manifest, item, worker_count, accelerator)
}

fn active_attempt<'a>(
    receipt: &'a CampaignCoordinatorReceipt,
    item: &WorkItem,
) -> Option<&'a ShardAttempt> {
    receipt.attempts.last().filter(|attempt| {
        attempt.stream_address.q == item.q
            && attempt.stream_address.n == item.n
            && attempt.shard_id == item.shard_id
            && matches!(attempt.state, ShardAttemptState::Authorized)
    })
}

fn accepted_attempt<'a>(
    receipt: &'a CampaignCoordinatorReceipt,
    item: &WorkItem,
) -> Option<&'a ShardAttempt> {
    receipt.attempts.iter().find(|attempt| {
        attempt.stream_address.q == item.q
            && attempt.stream_address.n == item.n
            && attempt.shard_id == item.shard_id
            && matches!(attempt.state, ShardAttemptState::Accepted { .. })
    })
}

fn exact_execution(
    campaign_root: &CampaignRoot,
    coordinator: &CampaignCoordinator,
    scope: ExactCellScope,
    items: &[WorkItem],
) -> Result<ExactCellExecution, CoordinatorError> {
    let terminal_state = coordinator
        .cell_state(scope.q, scope.n)
        .filter(|state| is_terminal(state))
        .cloned()
        .ok_or_else(|| CoordinatorError::Refused("exact cell is not terminal".to_owned()))?;
    let records = items
        .iter()
        .filter(|item| accepted_attempt(&coordinator.receipt, item).is_some())
        .map(|item| {
            let relative = PathBuf::from(shard_record_file(item.q, item.n, item.shard_id));
            let bytes = campaign_read(campaign_root, &relative)?;
            serde_json::from_slice(&bytes).map_err(CoordinatorError::Json)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ExactCellExecution {
        scope,
        records,
        terminal_state,
    })
}

fn preserve_invalid_raw(
    campaign_root: &CampaignRoot,
    campaign_id: &CampaignId,
    item: &WorkItem,
    attempt: u8,
    raw_relative: &Path,
) -> Result<(), CoordinatorError> {
    let relative = PathBuf::from("derived")
        .join(campaign_id.to_string())
        .join(COORDINATOR_DIRECTORY)
        .join("quarantine")
        .join(format!(
            "q{}-n{}-shard-{:06}-attempt-{attempt}.json",
            item.q, item.n, item.shard_id
        ));
    campaign_root
        .move_new(raw_relative, &relative)
        .map_err(|source| CoordinatorError::Io {
            path: campaign_root.path().join(relative),
            source,
        })
}

/// Closed conditional claim licensed by the canonical search record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiteratureSearchClaim {
    /// The recorded search locates no prior q=5/q=7 numerics, subject to every
    /// limitation recorded in that same source.
    NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
}

/// Evidence class of one published q=3 source row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q3SourceEvidence {
    /// Exhaustive source enumeration with no sampling uncertainty.
    ExactEnumeration,
    /// Published source Monte Carlo counts.
    MonteCarlo,
}

/// Source table containing a q=3 target row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q3SourceTable {
    /// Exact-enumeration table.
    Table3,
    /// Monte Carlo table.
    Table4,
}

/// Source-reported point estimate state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Q3ReportedProbability {
    /// A rounded point estimate printed by the source.
    Reported {
        /// Rounded value printed by the source.
        value: f64,
    },
    /// The source count is primary and no separate point was printed.
    NotSeparatelyReported,
}

/// Source-reported precision state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q3ReportedPrecision {
    /// Exact enumeration.
    ExactEnumeration,
    /// The source did not report a sampling precision.
    NotReported,
}

/// Typed source precision used by the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Q3SourcePrecision {
    /// Exact source row with zero sampling error.
    ExactSamplingError {
        /// Zero source sampling error.
        standard_error: f64,
    },
    /// Repository-derived plug-in binomial standard error.
    DerivedBinomialStandardError {
        /// Count-derived source standard error.
        standard_error: f64,
    },
}

/// Typed source interval used by the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Q3SourceInterval {
    /// Degenerate exact interval without a confidence level.
    ExactNoSamplingUncertainty {
        /// Degenerate lower bound.
        lower: f64,
        /// Degenerate upper bound.
        upper: f64,
    },
    /// Repository-derived Wilson score interval.
    DerivedWilsonScore {
        /// Nominal interval level.
        level: f64,
        /// Wilson lower bound.
        lower: f64,
        /// Wilson upper bound.
        upper: f64,
    },
}

/// Strict typed target transcribed from one canonical q=3 CSV row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q3SourceTarget {
    /// Matrix order.
    pub n: u16,
    /// Source table.
    pub table: Q3SourceTable,
    /// Exact or Monte Carlo source evidence.
    pub evidence: Q3SourceEvidence,
    /// Published zero count.
    pub zero_count: u64,
    /// Published sample size.
    pub sample_count: u64,
    /// Separately printed point, when present.
    pub reported_probability: Q3ReportedProbability,
    /// Point recomputed from the published counts.
    pub count_derived_probability: f64,
    /// Source-reported precision state.
    pub reported_precision: Q3ReportedPrecision,
    /// Typed comparison precision.
    pub precision: Q3SourcePrecision,
    /// Typed comparison interval.
    pub interval: Q3SourceInterval,
}

/// Count-derived campaign measurement for a completed q=3 row.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q3CampaignMeasurement {
    /// Observed zero count.
    pub zero_count: u64,
    /// Fixed campaign sample count.
    pub sample_count: u64,
    /// Count-derived point estimate.
    pub estimate: f64,
    /// Campaign 95% Wilson interval.
    pub wilson_interval: Interval,
    /// Count-derived plug-in binomial standard error.
    pub plugin_standard_error: f64,
}

/// Closed relation between the source and campaign intervals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q3IntervalRelation {
    /// Closed intervals intersect.
    Overlap,
    /// Closed intervals do not intersect.
    Disjoint,
}

/// Protocol precision classification for a q=3 comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q3PrecisionClassification {
    /// Source evidence is exact enumeration.
    PriorExact,
    /// Campaign standard error is below 0.9 times the source error.
    ExceedsPriorPrecision,
    /// Campaign standard error is within inclusive [0.9, 1.1] source error.
    MatchesPriorPrecision,
    /// Campaign standard error is above 1.1 times the source error.
    BelowPriorPrecision,
}

/// One typed q=3 completed or halted comparison row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoordinatorQ3ComparisonRow {
    /// A completed campaign measurement and source comparison.
    Completed {
        /// Matrix order.
        n: u16,
        /// Typed source target.
        source_target: Q3SourceTarget,
        /// Count-derived campaign measurement.
        campaign_measurement: Q3CampaignMeasurement,
        /// Closed interval relation.
        interval_relation: Q3IntervalRelation,
        /// Whether the campaign interval excludes the source point.
        interval_excludes_published: bool,
        /// Protocol precision classification.
        precision_classification: Q3PrecisionClassification,
    },
    /// Raw-only halted evidence; no estimate or comparison is constructed.
    Halted {
        /// Matrix order.
        n: u16,
        /// Raw matrices completed before the halt.
        matrix_count: u64,
        /// Raw zero permanents completed before the halt.
        permanent_zero_count: u64,
        /// Raw determinant companion counts.
        determinant: DeterminantCount,
        /// Closed mechanical halt reason.
        reason: HaltReason,
    },
}

/// Field-qualified interpretation represented by a coordinator projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoordinatorFieldInterpretation {
    /// `q=3` comparison to the validated versioned target table.
    PublishedTargetComparison {
        /// Validated target-table identity.
        target_table: ArtifactIdentity,
        /// One typed row per terminal q=3 cell.
        rows: Vec<CoordinatorQ3ComparisonRow>,
    },
    /// Conditional `q=5` or `q=7` literature-search basis.
    ConditionalLiteratureSearch {
        /// Validated bounded-search identity.
        search_receipt: ArtifactIdentity,
        /// Closed claim licensed by that search.
        claim: LiteratureSearchClaim,
    },
}

/// Terminal availability state of one field projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorFieldSidecarStatus {
    /// Every field cell completed.
    Completed,
    /// Every field cell is terminal and at least one halted.
    Halted,
}

/// Canonical versioned field projection published by the coordinator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinatorFieldSidecar {
    /// Sidecar schema version.
    pub schema_version: u32,
    /// Frozen campaign id.
    pub campaign_id: CampaignId,
    /// Exact root-manifest identity.
    pub manifest_identity: ArtifactIdentity,
    /// Actual atomically published field-summary identity.
    pub field_summary: ArtifactIdentity,
    /// Prime field order.
    pub q: u8,
    /// Canonical accepted raw identities in manifest order; may be empty.
    pub source_records: Vec<ArtifactIdentity>,
    /// Completed or halted field state.
    pub status: CoordinatorFieldSidecarStatus,
    /// Coordinator-derived typed interpretation.
    pub interpretation: CoordinatorFieldInterpretation,
}

/// Returns the canonical read-only path of a coordinator field sidecar.
#[must_use]
pub fn coordinator_field_sidecar_path(
    campaign_root: &Path,
    campaign_id: &CampaignId,
    q: u8,
) -> PathBuf {
    campaign_root.join(coordinator_field_sidecar_relative(campaign_id, q))
}

fn coordinator_field_sidecar_relative(campaign_id: &CampaignId, q: u8) -> PathBuf {
    PathBuf::from("derived")
        .join(campaign_id.to_string())
        .join(COORDINATOR_DIRECTORY)
        .join("field-sidecars")
        .join(format!("q{q}.json"))
}

pub(crate) fn classify_q3_precision(
    evidence: Q3SourceEvidence,
    campaign_standard_error: f64,
    source_standard_error: f64,
) -> Q3PrecisionClassification {
    if evidence == Q3SourceEvidence::ExactEnumeration {
        Q3PrecisionClassification::PriorExact
    } else if campaign_standard_error < 0.9 * source_standard_error {
        Q3PrecisionClassification::ExceedsPriorPrecision
    } else if campaign_standard_error <= 1.1 * source_standard_error {
        Q3PrecisionClassification::MatchesPriorPrecision
    } else {
        Q3PrecisionClassification::BelowPriorPrecision
    }
}

pub(crate) fn parse_q3_target_table(bytes: &[u8]) -> Result<Vec<Q3SourceTarget>, CoordinatorError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| CoordinatorError::Refused("q=3 target table is not UTF-8".to_owned()))?;
    let mut lines = text.lines();
    for expected_key in Q3_TARGET_COMMENTS {
        let line = lines.next().ok_or_else(|| {
            CoordinatorError::Refused("q=3 target table preamble is incomplete".to_owned())
        })?;
        let (key, value) = line
            .strip_prefix("# ")
            .and_then(|comment| comment.split_once(": "))
            .ok_or_else(|| {
                CoordinatorError::Refused("q=3 target table comment is malformed".to_owned())
            })?;
        if key != expected_key || value.trim().is_empty() {
            return refused("q=3 target table comment inventory is not canonical");
        }
        if key == "schema" && value != Q3_TARGET_SCHEMA {
            return refused("q=3 target table schema token is not supported");
        }
    }
    let header = lines
        .next()
        .ok_or_else(|| CoordinatorError::Refused("q=3 target table has no header".to_owned()))?;
    if header.split(',').collect::<Vec<_>>() != Q3_TARGET_FIELDS {
        return refused("q=3 target table does not have the exact 15-column header");
    }
    let mut targets = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.is_empty() || line.starts_with('#') {
            return refused("q=3 target table contains a non-row after its header");
        }
        let fields: Vec<_> = line.split(',').collect();
        if fields.len() != Q3_TARGET_FIELDS.len() {
            return refused("q=3 target table row does not have exactly 15 columns");
        }
        let expected_n = u16::try_from(offset)
            .ok()
            .and_then(|offset| 4_u16.checked_add(offset))
            .ok_or_else(|| CoordinatorError::Refused("q=3 target order overflow".to_owned()))?;
        targets.push(parse_q3_target_row(&fields, expected_n)?);
    }
    if targets.len() != 25 || targets.last().map(|target| target.n) != Some(28) {
        return refused("q=3 target table must cover every order n=4..28 exactly once");
    }
    Ok(targets)
}

fn parse_q3_target_row(
    fields: &[&str],
    expected_n: u16,
) -> Result<Q3SourceTarget, CoordinatorError> {
    let q = parse_q3_integer::<u8>(fields[0], "q")?;
    let n = parse_q3_integer::<u16>(fields[1], "n")?;
    if q != 3 || n != expected_n {
        return refused("q=3 target rows must be ordered exactly over n=4..28");
    }
    let table = match fields[2] {
        "Table 3" => Q3SourceTable::Table3,
        "Table 4" => Q3SourceTable::Table4,
        _ => return refused("q=3 target source_table token is invalid"),
    };
    let evidence = match fields[3] {
        "exact_enumeration" => Q3SourceEvidence::ExactEnumeration,
        "monte_carlo" => Q3SourceEvidence::MonteCarlo,
        _ => return refused("q=3 target source_evidence token is invalid"),
    };
    let zero_count = parse_q3_integer::<u64>(fields[4], "source_zero_count")?;
    let sample_count = parse_q3_integer::<u64>(fields[5], "source_n")?;
    if sample_count == 0 || zero_count > sample_count {
        return refused("q=3 target source counts are invalid");
    }
    let reported_probability = if fields[6] == "not_separately_reported" {
        Q3ReportedProbability::NotSeparatelyReported
    } else {
        Q3ReportedProbability::Reported {
            value: parse_q3_probability(fields[6], "source_reported_p_hat")?,
        }
    };
    let count_derived_probability = parse_q3_probability(fields[7], "p_hat_from_source_counts")?;
    let reported_precision = match fields[8] {
        "exact_enumeration" => Q3ReportedPrecision::ExactEnumeration,
        "not_reported" => Q3ReportedPrecision::NotReported,
        _ => return refused("q=3 target source_reported_precision token is invalid"),
    };
    let precision_value = parse_q3_nonnegative(fields[10], "reference_precision_value")?;
    let precision = match fields[9] {
        "exact_sampling_error" => Q3SourcePrecision::ExactSamplingError {
            standard_error: precision_value,
        },
        "derived_binomial_standard_error" => Q3SourcePrecision::DerivedBinomialStandardError {
            standard_error: precision_value,
        },
        _ => return refused("q=3 target reference_precision_kind token is invalid"),
    };
    let lower = parse_q3_probability(fields[13], "reference_interval_lower")?;
    let upper = parse_q3_probability(fields[14], "reference_interval_upper")?;
    if lower > upper {
        return refused("q=3 target reference interval is reversed");
    }
    let interval = match (fields[11], fields[12]) {
        ("exact_no_sampling_uncertainty", "not_applicable") => {
            Q3SourceInterval::ExactNoSamplingUncertainty { lower, upper }
        }
        ("derived_wilson_score", level) => Q3SourceInterval::DerivedWilsonScore {
            level: parse_q3_probability(level, "reference_interval_level")?,
            lower,
            upper,
        },
        _ => return refused("q=3 target reference interval tokens are invalid"),
    };
    let expected_probability = zero_count as f64 / sample_count as f64;
    if !approximately_equal(count_derived_probability, expected_probability) {
        return refused("q=3 target count-derived probability differs from its counts");
    }
    if evidence == Q3SourceEvidence::ExactEnumeration
        && fields[6] != format!("{expected_probability:.4}")
    {
        return refused(
            "q=3 exact target source_reported_p_hat is not the canonical four-decimal count rounding",
        );
    }
    let expected_se =
        (expected_probability * (1.0 - expected_probability) / sample_count as f64).sqrt();
    let (expected_lower, expected_upper) =
        gf2_stats::intervals::wilson_interval(zero_count, sample_count, PROTOCOL_Z_95);
    let exact_semantics = evidence == Q3SourceEvidence::ExactEnumeration
        && table == Q3SourceTable::Table3
        && reported_precision == Q3ReportedPrecision::ExactEnumeration
        && matches!(reported_probability, Q3ReportedProbability::Reported { .. })
        && matches!(precision, Q3SourcePrecision::ExactSamplingError { standard_error } if standard_error == 0.0)
        && matches!(interval, Q3SourceInterval::ExactNoSamplingUncertainty { lower, upper }
            if approximately_equal(lower, expected_probability)
                && approximately_equal(upper, expected_probability));
    let monte_carlo_semantics = evidence == Q3SourceEvidence::MonteCarlo
        && table == Q3SourceTable::Table4
        && reported_precision == Q3ReportedPrecision::NotReported
        && reported_probability == Q3ReportedProbability::NotSeparatelyReported
        && matches!(precision, Q3SourcePrecision::DerivedBinomialStandardError { standard_error }
            if approximately_equal(standard_error, expected_se))
        && matches!(interval, Q3SourceInterval::DerivedWilsonScore { level, lower, upper }
            if approximately_equal(level, 0.95)
                && approximately_equal(lower, expected_lower)
                && approximately_equal(upper, expected_upper));
    if (n <= 5 && !exact_semantics) || (n >= 6 && !monte_carlo_semantics) {
        return Err(CoordinatorError::Refused(format!(
            "q=3 target row n={n} contradicts the n=4..5 exact and n=6..28 Monte Carlo semantics"
        )));
    }
    Ok(Q3SourceTarget {
        n,
        table,
        evidence,
        zero_count,
        sample_count,
        reported_probability,
        count_derived_probability,
        reported_precision,
        precision,
        interval,
    })
}

fn parse_q3_integer<T>(text: &str, field: &str) -> Result<T, CoordinatorError>
where
    T: std::str::FromStr + ToString,
{
    let value = text
        .parse::<T>()
        .map_err(|_| CoordinatorError::Refused(format!("q=3 target {field} is not an integer")))?;
    if value.to_string() != text {
        return refused("q=3 target integer is not canonically encoded");
    }
    Ok(value)
}

fn parse_q3_probability(text: &str, field: &str) -> Result<f64, CoordinatorError> {
    let value = parse_q3_nonnegative(text, field)?;
    if value > 1.0 {
        return refused("q=3 target probability is outside [0,1]");
    }
    Ok(value)
}

fn parse_q3_nonnegative(text: &str, field: &str) -> Result<f64, CoordinatorError> {
    let value = text
        .parse::<f64>()
        .map_err(|_| CoordinatorError::Refused(format!("q=3 target {field} is not numeric")))?;
    if !value.is_finite() || value < 0.0 {
        return refused("q=3 target numeric value is negative or non-finite");
    }
    Ok(value)
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 5e-12 * left.abs().max(right.abs()).max(1.0)
}

fn validate_literature_search(bytes: &[u8]) -> Result<(), CoordinatorError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        CoordinatorError::Refused("literature-search receipt is not UTF-8".to_owned())
    })?;
    let sections: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .collect();
    if !sections.contains(&"Limitations") || !sections.contains(&"Conclusion") {
        return refused("literature-search receipt lacks limitations or conclusion sections");
    }
    Ok(())
}

fn resolve_evidence_sources(
    campaign_root: &CampaignRoot,
    expected: &CoordinatorEvidenceSources,
) -> Result<CoordinatorEvidenceSources, CoordinatorError> {
    let repository = repository_root_for_campaign(campaign_root.path())?;
    let (q3_targets, q3_bytes) = repository_artifact(&repository, &expected.q3_targets.path)?;
    if q3_targets != expected.q3_targets {
        return refused("q=3 target bytes differ from the expected artifact identity");
    }
    parse_q3_target_table(&q3_bytes)?;
    let (q5_q7_literature_search, search_bytes) =
        repository_artifact(&repository, &expected.q5_q7_literature_search.path)?;
    if q5_q7_literature_search != expected.q5_q7_literature_search {
        return refused("literature-search bytes differ from the expected artifact identity");
    }
    validate_literature_search(&search_bytes)?;
    Ok(CoordinatorEvidenceSources {
        q3_targets,
        q5_q7_literature_search,
    })
}

fn repository_artifact(
    repository: &Path,
    relative: &ArtifactPath,
) -> Result<(ArtifactIdentity, Vec<u8>), CoordinatorError> {
    let path = repository.join(relative.as_str());
    let bytes = super::root_fs::read_absolute(&path)
        .map_err(|source| CoordinatorError::Io { path, source })?;
    Ok((
        ArtifactIdentity {
            path: relative.clone(),
            sha256: digest(&bytes),
        },
        bytes,
    ))
}

fn publish_terminal_fields_locked(
    campaign_root: &CampaignRoot,
    coordinator: &CampaignCoordinator,
) -> Result<(), CoordinatorError> {
    validate_receipt(&coordinator.manifest, &coordinator.receipt)?;
    validate_campaign_files(campaign_root, &coordinator.manifest, &coordinator.receipt)?;
    let repository = repository_root_for_campaign(campaign_root.path())?;
    for field in &coordinator.receipt.fields {
        let status = match field.execution {
            FieldExecutionState::InProgress => continue,
            FieldExecutionState::Completed => CoordinatorFieldSidecarStatus::Completed,
            FieldExecutionState::Halted => CoordinatorFieldSidecarStatus::Halted,
        };
        let summary = coordinator.assemble_field_summary(field.q)?;
        let summary_relative = PathBuf::from(field_summary_file(field.q));
        write_new_json(campaign_root, &summary_relative, &summary)?;
        let summary_bytes = campaign_read(campaign_root, &summary_relative)?;
        let actual_summary: FieldSummary =
            serde_json::from_slice(&summary_bytes).map_err(CoordinatorError::Json)?;
        if actual_summary != summary {
            return refused("published field summary differs from terminal receipt evidence");
        }
        let field_summary = ArtifactIdentity {
            path: format!(
                "{DATASET_HOME}/{}/{}",
                coordinator.receipt.campaign_id,
                field_summary_file(field.q)
            )
            .parse()
            .map_err(|error| {
                CoordinatorError::Refused(format!("invalid field-summary path: {error}"))
            })?,
            sha256: digest(&summary_bytes),
        };
        let source_records = field_source_records(&coordinator.receipt, field.q);
        let interpretation = field_interpretation(
            &repository,
            &coordinator.receipt.evidence_sources,
            field.q,
            &summary,
        )?;
        let sidecar = CoordinatorFieldSidecar {
            schema_version: COORDINATOR_SCHEMA_VERSION,
            campaign_id: coordinator.receipt.campaign_id.clone(),
            manifest_identity: coordinator.receipt.manifest_identity.clone(),
            field_summary,
            q: field.q,
            source_records,
            status,
            interpretation,
        };
        let relative =
            coordinator_field_sidecar_relative(&coordinator.receipt.campaign_id, field.q);
        write_new_json(campaign_root, &relative, &sidecar)?;
    }
    Ok(())
}

fn field_source_records(receipt: &CampaignCoordinatorReceipt, q: u8) -> Vec<ArtifactIdentity> {
    receipt
        .cells
        .iter()
        .filter(|cell| cell.q == q)
        .flat_map(|cell| match &cell.execution {
            CellExecutionState::Completed { source_records, .. }
            | CellExecutionState::Halted { source_records, .. } => source_records.clone(),
            CellExecutionState::Pending | CellExecutionState::Scheduled { .. } => Vec::new(),
        })
        .collect()
}

fn field_interpretation(
    repository: &Path,
    sources: &CoordinatorEvidenceSources,
    q: u8,
    summary: &FieldSummary,
) -> Result<CoordinatorFieldInterpretation, CoordinatorError> {
    match q {
        3 => {
            let (identity, bytes) = repository_artifact(repository, &sources.q3_targets.path)?;
            if identity != sources.q3_targets {
                return refused("q=3 target bytes differ from the receipt-bound identity");
            }
            let targets = parse_q3_target_table(&bytes)?;
            Ok(CoordinatorFieldInterpretation::PublishedTargetComparison {
                target_table: identity,
                rows: build_q3_comparison_rows(summary, &targets)?,
            })
        }
        5 | 7 => {
            let (identity, bytes) =
                repository_artifact(repository, &sources.q5_q7_literature_search.path)?;
            if identity != sources.q5_q7_literature_search {
                return refused("literature-search bytes differ from the receipt-bound identity");
            }
            validate_literature_search(&bytes)?;
            Ok(
                CoordinatorFieldInterpretation::ConditionalLiteratureSearch {
                    search_receipt: identity,
                    claim: LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
                },
            )
        }
        _ => refused("terminal field has no canonical interpretation source"),
    }
}

fn build_q3_comparison_rows(
    summary: &FieldSummary,
    targets: &[Q3SourceTarget],
) -> Result<Vec<CoordinatorQ3ComparisonRow>, CoordinatorError> {
    summary
        .rows
        .iter()
        .map(|row| match &row.terminal_state {
            CellTerminalState::Completed {
                permanent_estimate, ..
            } => {
                let target = targets
                    .iter()
                    .find(|target| target.n == row.n)
                    .cloned()
                    .ok_or_else(|| {
                        CoordinatorError::Refused(
                            "completed q=3 row lacks its canonical target".to_owned(),
                        )
                    })?;
                if row.matrix_count == 0 {
                    return refused("completed q=3 row has no samples");
                }
                let plugin_standard_error = (permanent_estimate.point
                    * (1.0 - permanent_estimate.point)
                    / row.matrix_count as f64)
                    .sqrt();
                let (campaign_lower, campaign_upper) = gf2_stats::intervals::wilson_interval(
                    row.permanent_zero_count,
                    row.matrix_count,
                    PROTOCOL_Z_95,
                );
                let campaign_interval = Interval {
                    lower: campaign_lower,
                    upper: campaign_upper,
                };
                let (source_lower, source_upper) = source_interval_bounds(target.interval);
                let relation = if campaign_interval.upper >= source_lower
                    && source_upper >= campaign_interval.lower
                {
                    Q3IntervalRelation::Overlap
                } else {
                    Q3IntervalRelation::Disjoint
                };
                let source_standard_error = match target.precision {
                    Q3SourcePrecision::ExactSamplingError { standard_error }
                    | Q3SourcePrecision::DerivedBinomialStandardError { standard_error } => {
                        standard_error
                    }
                };
                Ok(CoordinatorQ3ComparisonRow::Completed {
                    n: row.n,
                    source_target: target.clone(),
                    campaign_measurement: Q3CampaignMeasurement {
                        zero_count: row.permanent_zero_count,
                        sample_count: row.matrix_count,
                        estimate: permanent_estimate.point,
                        wilson_interval: campaign_interval,
                        plugin_standard_error,
                    },
                    interval_relation: relation,
                    interval_excludes_published: target.count_derived_probability
                        < campaign_interval.lower
                        || target.count_derived_probability > campaign_interval.upper,
                    precision_classification: classify_q3_precision(
                        target.evidence,
                        plugin_standard_error,
                        source_standard_error,
                    ),
                })
            }
            CellTerminalState::Halted { reason } => Ok(CoordinatorQ3ComparisonRow::Halted {
                n: row.n,
                matrix_count: row.matrix_count,
                permanent_zero_count: row.permanent_zero_count,
                determinant: row.determinant.clone(),
                reason: *reason,
            }),
        })
        .collect()
}

fn source_interval_bounds(interval: Q3SourceInterval) -> (f64, f64) {
    match interval {
        Q3SourceInterval::ExactNoSamplingUncertainty { lower, upper }
        | Q3SourceInterval::DerivedWilsonScore { lower, upper, .. } => (lower, upper),
    }
}

fn validate_frozen_paths(
    manifest: &CampaignManifest,
    manifest_identity: &ArtifactIdentity,
    protocol_identity: &ArtifactIdentity,
) -> Result<(), CoordinatorError> {
    let expected_manifest = format!("{DATASET_HOME}/{}/{MANIFEST_FILE}", manifest.campaign_id);
    if manifest_identity.path.as_str() != expected_manifest {
        return refused("manifest identity path differs from the frozen campaign path");
    }
    if protocol_identity.path != protocol_artifact_path() {
        return refused("protocol identity path differs from the frozen protocol path");
    }
    Ok(())
}

fn identity_for_campaign_file(
    campaign_root: &CampaignRoot,
    campaign_id: &CampaignId,
    relative: &str,
) -> Result<ArtifactIdentity, CoordinatorError> {
    let bytes = campaign_read(campaign_root, Path::new(relative))?;
    Ok(ArtifactIdentity {
        path: format!("{DATASET_HOME}/{campaign_id}/{relative}")
            .parse()
            .map_err(|error| {
                CoordinatorError::Refused(format!("invalid artifact path: {error}"))
            })?,
        sha256: digest(&bytes),
    })
}

fn identity_for_protocol(
    campaign_root: &CampaignRoot,
) -> Result<ArtifactIdentity, CoordinatorError> {
    let dataset_root = campaign_root.path().parent().ok_or_else(|| {
        CoordinatorError::Refused("campaign root has no dataset parent".to_owned())
    })?;
    let path = dataset_root.join("protocol.md");
    let bytes = super::root_fs::read_absolute(&path).map_err(|source| CoordinatorError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(ArtifactIdentity {
        path: protocol_artifact_path(),
        sha256: digest(&bytes),
    })
}

fn protocol_artifact_path() -> ArtifactPath {
    format!("{DATASET_HOME}/protocol.md")
        .parse()
        .expect("dataset home plus protocol file is a normalized artifact path")
}

fn validate_campaign_files(
    campaign_root: &CampaignRoot,
    manifest: &CampaignManifest,
    receipt: &CampaignCoordinatorReceipt,
) -> Result<(), CoordinatorError> {
    let on_disk = read_manifest_anchored(campaign_root)?;
    if on_disk != *manifest
        || identity_for_campaign_file(campaign_root, &manifest.campaign_id, MANIFEST_FILE)?
            != receipt.manifest_identity
        || identity_for_protocol(campaign_root)? != receipt.protocol_identity
    {
        return refused("on-disk frozen manifest or protocol identity differs from the receipt");
    }
    let observed_sources = resolve_evidence_sources(campaign_root, &receipt.evidence_sources)?;
    if observed_sources != receipt.evidence_sources {
        return refused("field-interpretation source bytes differ from the coordinator receipt");
    }
    for arm in &receipt.arms {
        validate_accelerator_cost_identity(campaign_root, manifest, arm)?;
    }
    validate_on_disk_attempts(campaign_root, manifest, receipt)
}

fn validate_arm(
    manifest: &CampaignManifest,
    arm: &ArmInvocation,
    q: u8,
    n: u16,
) -> Result<(), CoordinatorError> {
    if arm.worker_count == 0 || arm.argv.is_empty() {
        return refused("arm invocation requires nonzero workers and argv");
    }
    if manifest.provenance.binary_sha256.as_ref() != Some(&arm.executable_sha256) {
        return refused("arm executable digest differs from the frozen manifest");
    }
    if manifest.provenance.invocation.first() != arm.argv.first() {
        return refused("arm executable token differs from the frozen invocation");
    }
    if arm.argv.iter().any(|arg| arg == "--dry-run-schedule") {
        return refused("dry preflight is not a campaign-purpose execution arm");
    }
    let selected_q = unique_option(&arm.argv, "--q")?;
    let selected_n = unique_option(&arm.argv, "--n")?;
    let workers = effective_worker_count(&arm.argv)?;
    if selected_q != q.to_string() || selected_n != n.to_string() || workers != arm.worker_count {
        return refused("arm selectors or effective worker count conflict with its receipt");
    }
    if normalized_invocation(&manifest.provenance.invocation)? != normalized_invocation(&arm.argv)?
    {
        return refused("arm arguments differ from the frozen invocation shape");
    }
    Ok(())
}

fn normalized_invocation(argv: &[String]) -> Result<Vec<String>, CoordinatorError> {
    unique_option(argv, "--q")?;
    unique_option(argv, "--n")?;
    let workers = effective_worker_count(argv)?;
    let mut normalized = argv.to_vec();
    for option in ["--q", "--n"] {
        let index = normalized
            .iter()
            .position(|value| value == option)
            .ok_or_else(|| CoordinatorError::Refused("frozen selector is absent".to_owned()))?;
        normalized[index + 1] = format!("<{option}>");
    }
    if let Some(index) = normalized.iter().position(|value| value == "--workers") {
        normalized.drain(index..=index + 1);
    }
    normalized.extend(["--workers".to_owned(), workers.to_string()]);
    Ok(normalized)
}

fn effective_worker_count(argv: &[String]) -> Result<usize, CoordinatorError> {
    match optional_unique_option(argv, "--workers")? {
        Some(value) => {
            let workers = value.parse::<usize>().map_err(|_| {
                CoordinatorError::Refused(
                    "arm worker count must be a positive canonical integer".to_owned(),
                )
            })?;
            if workers == 0 || value != workers.to_string() {
                return refused("arm worker count must be a positive canonical integer");
            }
            Ok(workers)
        }
        None => Ok(1),
    }
}

fn unique_option(argv: &[String], option: &str) -> Result<String, CoordinatorError> {
    let values: Vec<_> = argv
        .iter()
        .enumerate()
        .filter(|(_, value)| value.as_str() == option)
        .map(|(index, _)| argv.get(index + 1))
        .collect();
    match values.as_slice() {
        [Some(value)] if !value.starts_with("--") => Ok((*value).clone()),
        _ => refused("arm requires exactly one value for every effective selector"),
    }
}

fn optional_unique_option(
    argv: &[String],
    option: &str,
) -> Result<Option<String>, CoordinatorError> {
    let values: Vec<_> = argv
        .iter()
        .enumerate()
        .filter(|(_, value)| value.as_str() == option)
        .map(|(index, _)| argv.get(index + 1))
        .collect();
    match values.as_slice() {
        [] => Ok(None),
        [Some(value)] if !value.starts_with("--") => Ok(Some((*value).clone())),
        _ => refused("arm contains an ambiguous optional execution input"),
    }
}

fn campaign_purpose_tag(manifest: &CampaignManifest) -> Result<u8, CoordinatorError> {
    manifest
        .stream_purposes
        .iter()
        .find(|purpose| purpose.tag == CAMPAIGN_CELL_PURPOSE_TAG)
        .map(|purpose| purpose.tag)
        .ok_or_else(|| CoordinatorError::Refused("campaign purpose tag is absent".to_owned()))
}

fn validate_attempt_state(
    campaign_id: &CampaignId,
    cell: &CellSpec,
    shard_id: u64,
    state: &ShardAttemptState,
) -> Result<(), CoordinatorError> {
    match state {
        ShardAttemptState::Authorized => Ok(()),
        ShardAttemptState::Quarantined { error } if error.trim().is_empty() => {
            refused("quarantine diagnostic must not be empty")
        }
        ShardAttemptState::Quarantined { .. } => Ok(()),
        ShardAttemptState::Accepted {
            record,
            observation,
        } => {
            let expected_path = format!(
                "{DATASET_HOME}/{campaign_id}/{}",
                shard_record_file(cell.q, cell.n, shard_id)
            );
            if record.path.as_str() != expected_path {
                return refused("accepted record path is not the canonical manifested shard path");
            }
            let expected = expected_shard_count(cell, shard_id)?;
            if observation.matrix_count != expected
                || observation.permanent_zero_count > observation.matrix_count
            {
                return refused("accepted observation differs from the manifested shard count");
            }
            match (&cell.determinant_companion, &observation.determinant) {
                (DeterminantPlan::NotEvaluated, DeterminantCount::NotEvaluated) => Ok(()),
                (
                    DeterminantPlan::Evaluate,
                    DeterminantCount::Evaluated {
                        sample_count,
                        zero_count,
                    },
                ) if *sample_count == observation.matrix_count && zero_count <= sample_count => {
                    Ok(())
                }
                _ => refused("accepted determinant counts differ from the manifested plan"),
            }
        }
    }
}

fn validate_raw_record(
    manifest: &CampaignManifest,
    cell: &CellSpec,
    shard_id: u64,
    stream_index: u64,
    record: &ShardRecord,
) -> Result<(), CoordinatorError> {
    if record.schema_version != manifest.schema_version
        || record.shard_id != shard_id
        || record.stream_address
            != (StreamAddress {
                root_seed: manifest.root_seed,
                q: cell.q,
                n: cell.n,
                purpose_tag: campaign_purpose_tag(manifest)?,
                stream_index,
            })
    {
        return refused("raw shard schema or stream identity differs from the manifest");
    }
    let expected = expected_shard_count(cell, shard_id)?;
    let histogram_total = record
        .permanent_histogram
        .iter()
        .try_fold(0_u64, |sum, count| sum.checked_add(*count))
        .ok_or_else(|| CoordinatorError::Refused("raw histogram count overflow".to_owned()))?;
    if record.permanent_histogram.len() != usize::from(cell.q)
        || histogram_total != expected
        || record.matrix_count != expected
        || record.permanent_histogram.first().copied() != Some(record.permanent_zero_count)
    {
        return refused("raw shard histogram or sample count is invalid");
    }
    validate_attempt_state(
        &manifest.campaign_id,
        cell,
        shard_id,
        &ShardAttemptState::Accepted {
            record: ArtifactIdentity {
                path: format!(
                    "{DATASET_HOME}/{}/{}",
                    manifest.campaign_id,
                    shard_record_file(cell.q, cell.n, shard_id)
                )
                .parse()
                .map_err(|error| {
                    CoordinatorError::Refused(format!("invalid shard path: {error}"))
                })?,
                sha256: "0".repeat(64).parse().expect("canonical digest fixture"),
            },
            observation: ShardObservation {
                matrix_count: record.matrix_count,
                permanent_zero_count: record.permanent_zero_count,
                determinant: record.determinant.clone(),
            },
        },
    )
}

fn expected_shard_count(cell: &CellSpec, shard_id: u64) -> Result<u64, CoordinatorError> {
    let index = cell
        .shards
        .iter()
        .position(|shard| shard.shard_id == shard_id)
        .ok_or_else(|| CoordinatorError::Refused("manifest shard not found".to_owned()))?;
    let offset = u64::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(cell.shard_size))
        .ok_or_else(|| CoordinatorError::Refused("manifest shard offset overflow".to_owned()))?;
    Ok(cell
        .matrix_count
        .saturating_sub(offset)
        .min(cell.shard_size))
}

fn halted_row(cell: &CellSpec, partial: ShardObservation, reason: HaltReason) -> SummaryRow {
    SummaryRow {
        schema_version: SCHEMA_VERSION,
        q: cell.q,
        n: cell.n,
        matrix_count: partial.matrix_count,
        permanent_zero_count: partial.permanent_zero_count,
        determinant: partial.determinant,
        terminal_state: CellTerminalState::Halted { reason },
    }
}

fn pooled_accepted_from_receipt(
    receipt: &CampaignCoordinatorReceipt,
    cell: &CellSpec,
) -> Result<(ShardObservation, Vec<ArtifactIdentity>), CoordinatorError> {
    let mut matrix_count = 0_u64;
    let mut permanent_zero_count = 0_u64;
    let mut determinant_sample_count = 0_u64;
    let mut determinant_zero_count = 0_u64;
    let mut sources = Vec::new();
    for shard in &cell.shards {
        if let Some((record, observation)) = receipt.attempts.iter().find_map(|attempt| {
            if (
                attempt.stream_address.q,
                attempt.stream_address.n,
                attempt.shard_id,
            ) != (cell.q, cell.n, shard.shard_id)
            {
                return None;
            }
            match &attempt.state {
                ShardAttemptState::Accepted {
                    record,
                    observation,
                } => Some((record, observation)),
                ShardAttemptState::Authorized | ShardAttemptState::Quarantined { .. } => None,
            }
        }) {
            matrix_count = matrix_count
                .checked_add(observation.matrix_count)
                .ok_or_else(|| {
                    CoordinatorError::Refused("pooled matrix count overflow".to_owned())
                })?;
            permanent_zero_count = permanent_zero_count
                .checked_add(observation.permanent_zero_count)
                .ok_or_else(|| {
                    CoordinatorError::Refused("pooled zero count overflow".to_owned())
                })?;
            if let DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            } = observation.determinant
            {
                determinant_sample_count = determinant_sample_count
                    .checked_add(sample_count)
                    .ok_or_else(|| {
                        CoordinatorError::Refused("pooled determinant count overflow".to_owned())
                    })?;
                determinant_zero_count = determinant_zero_count
                    .checked_add(zero_count)
                    .ok_or_else(|| {
                        CoordinatorError::Refused(
                            "pooled determinant zero count overflow".to_owned(),
                        )
                    })?;
            }
            sources.push(record.clone());
        }
    }
    let determinant = match cell.determinant_companion {
        DeterminantPlan::Evaluate => DeterminantCount::Evaluated {
            sample_count: determinant_sample_count,
            zero_count: determinant_zero_count,
        },
        DeterminantPlan::NotEvaluated => DeterminantCount::NotEvaluated,
    };
    Ok((
        ShardObservation {
            matrix_count,
            permanent_zero_count,
            determinant,
        },
        sources,
    ))
}

fn validate_receipt(
    manifest: &CampaignManifest,
    receipt: &CampaignCoordinatorReceipt,
) -> Result<(), CoordinatorError> {
    if receipt.schema_version != COORDINATOR_SCHEMA_VERSION
        || receipt.campaign_id != manifest.campaign_id
        || receipt.acceptance_plan != AcceptancePlan::for_manifest(manifest)?
        || receipt.retry_rule != RetryRule::OneSameAddressRecoveryNoAdditionalAlpha
    {
        return refused("coordinator receipt fixed identity or plan is invalid");
    }
    if receipt.evidence_sources.q3_targets.path
        == receipt.evidence_sources.q5_q7_literature_search.path
    {
        return refused("coordinator interpretation sources must be distinct artifacts");
    }
    validate_frozen_paths(
        manifest,
        &receipt.manifest_identity,
        &receipt.protocol_identity,
    )?;
    let cells: Vec<_> = receipt.cells.iter().map(|cell| (cell.q, cell.n)).collect();
    let manifested: Vec<_> = manifest.cells.iter().map(|cell| (cell.q, cell.n)).collect();
    if cells != manifested {
        return refused("coordinator cell inventory differs from the frozen manifest");
    }
    let mut expected_fields: Vec<_> = manifest.cells.iter().map(|cell| cell.q).collect();
    expected_fields.sort_unstable();
    expected_fields.dedup();
    if receipt
        .fields
        .iter()
        .map(|field| field.q)
        .collect::<Vec<_>>()
        != expected_fields
    {
        return refused("coordinator field inventory differs from the frozen manifest");
    }

    let mut arm_indices = BTreeMap::new();
    for (index, arm) in receipt.arms.iter().enumerate() {
        let ExactCellScope { q, n } = arm.scope;
        validate_arm(manifest, arm, q, n)?;
        if !manifest.cells.iter().any(|cell| (cell.q, cell.n) == (q, n))
            || arm_indices.insert((q, n), index).is_some()
        {
            return refused("persisted arm is duplicate or not manifested");
        }
    }
    if receipt.arms.first().is_some_and(|arm| {
        arm.scope
            != ExactCellScope {
                q: FIRST_FIELD,
                n: FIRST_ORDER,
            }
    }) {
        return refused("first persisted arm is not q=7 n=20");
    }

    let mut attempt_history: BTreeMap<(u8, u16, u64), Vec<&ShardAttemptState>> = BTreeMap::new();
    let mut preceding_arm_index = 0_usize;
    let mut authorized_attempt = None;
    for (attempt_index, attempt) in receipt.attempts.iter().enumerate() {
        let q = attempt.stream_address.q;
        let n = attempt.stream_address.n;
        let cell = manifest
            .cells
            .iter()
            .find(|cell| (cell.q, cell.n) == (q, n))
            .ok_or_else(|| {
                CoordinatorError::Refused("attempt cell is not manifested".to_owned())
            })?;
        let shard = cell
            .shards
            .iter()
            .find(|shard| shard.shard_id == attempt.shard_id)
            .ok_or_else(|| {
                CoordinatorError::Refused("attempt shard is not manifested".to_owned())
            })?;
        if attempt.stream_address
            != (StreamAddress {
                root_seed: manifest.root_seed,
                q,
                n,
                purpose_tag: campaign_purpose_tag(manifest)?,
                stream_index: shard.stream_index,
            })
            || attempt.backend != cell.backend
            || attempt.backend_receipt != cell.backend_receipt
            || attempt.rng_algorithm != manifest.provenance.rng_algorithm
            || attempt.rng_version != manifest.provenance.rng_version
        {
            return refused("persisted attempt identity differs from the manifest");
        }
        let key = (q, n, attempt.shard_id);
        let history = attempt_history.entry(key).or_default();
        let expected_number = u8::try_from(history.len() + 1)
            .map_err(|_| CoordinatorError::Refused("attempt number overflow".to_owned()))?;
        if attempt.attempt != expected_number
            || attempt.attempt > 2
            || (attempt.attempt == 2
                && !matches!(history.first(), Some(ShardAttemptState::Quarantined { .. })))
        {
            return refused("persisted retry history violates the quarantine rule");
        }
        validate_attempt_state(&receipt.campaign_id, cell, attempt.shard_id, &attempt.state)?;
        let arm_index = *arm_indices
            .get(&(q, n))
            .ok_or_else(|| CoordinatorError::Refused("attempt has no admitted arm".to_owned()))?;
        if !receipt.attempts.is_empty() && arm_index < preceding_arm_index {
            return refused("attempt history interleaves serial exact-cell arms");
        }
        preceding_arm_index = arm_index;
        if matches!(attempt.state, ShardAttemptState::Authorized)
            && authorized_attempt.replace((attempt_index, q, n)).is_some()
        {
            return refused("receipt contains more than one active authorized attempt");
        }
        history.push(&attempt.state);
    }
    if let Some((attempt_index, q, n)) = authorized_attempt {
        if attempt_index + 1 != receipt.attempts.len()
            || !matches!(receipt.halt, CampaignHaltState::Running)
            || !matches!(
                receipt
                    .cells
                    .iter()
                    .find(|cell| (cell.q, cell.n) == (q, n))
                    .map(|cell| &cell.execution),
                Some(CellExecutionState::Scheduled { .. })
            )
        {
            return refused(
                "active attempt is not the final evidence of the scheduled running cell",
            );
        }
    }
    for (&(q, n, _), history) in &attempt_history {
        if history.len() == 2
            && matches!(history[1], ShardAttemptState::Quarantined { .. })
            && (!matches!(
                receipt
                    .cells
                    .iter()
                    .find(|cell| (cell.q, cell.n) == (q, n))
                    .map(|cell| &cell.execution),
                Some(CellExecutionState::Halted { .. })
            ) || !matches!(
                receipt.halt,
                CampaignHaltState::Halted {
                    cause: CampaignHaltCause::Mechanical {
                        q: cause_q,
                        n: cause_n,
                        reason: HaltReason::ExecutionFailure,
                    }
                } if (cause_q, cause_n) == (q, n)
            ))
        {
            return refused("exhausted recovery is not a terminal mechanical campaign halt");
        }
    }

    let scheduled = receipt
        .cells
        .iter()
        .filter(|cell| matches!(cell.execution, CellExecutionState::Scheduled { .. }))
        .count();
    if scheduled > 1 {
        return refused("receipt contains more than one scheduled cell");
    }
    for cell in &receipt.cells {
        let spec = manifest
            .cells
            .iter()
            .find(|spec| (spec.q, spec.n) == (cell.q, cell.n))
            .ok_or_else(|| CoordinatorError::Refused("manifest cell not found".to_owned()))?;
        let arm_index = arm_indices.get(&(cell.q, cell.n)).copied();
        let (pooled, expected_sources) = pooled_accepted_from_receipt(receipt, spec)?;
        match &cell.execution {
            CellExecutionState::Pending => {
                if arm_index.is_some() || pooled.matrix_count != 0 {
                    return refused("pending cell carries arm or attempt evidence");
                }
            }
            CellExecutionState::Scheduled {
                arm_index: stored_index,
            } => {
                if arm_index != Some(*stored_index)
                    || Some(*stored_index) != receipt.arms.len().checked_sub(1)
                {
                    return refused("scheduled cell does not bind the final exact arm");
                }
            }
            CellExecutionState::Completed {
                assessment,
                source_records,
            } => {
                if arm_index.is_none() || expected_sources.len() != spec.shards.len() {
                    return refused("completed cell lacks its admitted arm or accepted shards");
                }
                let expected_assessment = assess_completed_cell(
                    &receipt.acceptance_plan,
                    spec,
                    pooled.matrix_count,
                    pooled.permanent_zero_count,
                    pooled.determinant,
                )?;
                if *assessment != expected_assessment || *source_records != expected_sources {
                    return refused("completed evidence differs from accepted shard attempts");
                }
            }
            CellExecutionState::Halted {
                summary,
                source_records,
            } => {
                let CellTerminalState::Halted { reason } = summary.terminal_state else {
                    return refused("halted receipt cell contains a completed summary");
                };
                if *summary != halted_row(spec, pooled, reason)
                    || *source_records != expected_sources
                {
                    return refused("halted evidence differs from accepted shard attempts");
                }
            }
        }
    }

    for (index, arm) in receipt.arms.iter().enumerate() {
        let ExactCellScope { q, n } = arm.scope;
        let state = receipt
            .cells
            .iter()
            .find(|cell| (cell.q, cell.n) == (q, n))
            .map(|cell| &cell.execution)
            .ok_or_else(|| CoordinatorError::Refused("arm cell is absent".to_owned()))?;
        if index + 1 < receipt.arms.len() && !matches!(state, CellExecutionState::Completed { .. })
        {
            return refused("a later arm follows a non-completed serial arm");
        }
    }

    for field in &receipt.fields {
        let states: Vec<_> = receipt
            .cells
            .iter()
            .filter(|cell| cell.q == field.q)
            .map(|cell| &cell.execution)
            .collect();
        let expected = if states.iter().all(|state| is_terminal(state)) {
            if states
                .iter()
                .any(|state| matches!(state, CellExecutionState::Halted { .. }))
            {
                FieldExecutionState::Halted
            } else {
                FieldExecutionState::Completed
            }
        } else {
            FieldExecutionState::InProgress
        };
        if field.execution != expected {
            return refused("persisted field state is not derived from its cells");
        }
    }

    validate_halt_state(receipt)?;
    Ok(())
}

fn validate_on_disk_attempts(
    campaign_root: &CampaignRoot,
    manifest: &CampaignManifest,
    receipt: &CampaignCoordinatorReceipt,
) -> Result<(), CoordinatorError> {
    for attempt in &receipt.attempts {
        let ShardAttemptState::Accepted {
            record,
            observation,
        } = &attempt.state
        else {
            continue;
        };
        let q = attempt.stream_address.q;
        let n = attempt.stream_address.n;
        let cell = manifest
            .cells
            .iter()
            .find(|cell| (cell.q, cell.n) == (q, n))
            .ok_or_else(|| {
                CoordinatorError::Refused("attempt cell is not manifested".to_owned())
            })?;
        let shard = cell
            .shards
            .iter()
            .find(|shard| shard.shard_id == attempt.shard_id)
            .ok_or_else(|| {
                CoordinatorError::Refused("attempt shard is not manifested".to_owned())
            })?;
        let relative = PathBuf::from(shard_record_file(q, n, attempt.shard_id));
        let bytes = campaign_read(campaign_root, &relative)?;
        if digest(&bytes) != record.sha256 {
            return refused("accepted attempt digest differs from raw shard bytes");
        }
        let raw: ShardRecord = serde_json::from_slice(&bytes).map_err(CoordinatorError::Json)?;
        validate_raw_record(manifest, cell, attempt.shard_id, shard.stream_index, &raw)?;
        if observation
            != &(ShardObservation {
                matrix_count: raw.matrix_count,
                permanent_zero_count: raw.permanent_zero_count,
                determinant: raw.determinant,
            })
        {
            return refused("accepted attempt observation differs from raw shard bytes");
        }
    }
    Ok(())
}

fn rejected_families(assessment: &CompletedCellAssessment) -> Vec<AcceptanceFamily> {
    let mut families = Vec::new();
    if assessment.permanent.test.verdict == AcceptanceVerdict::Rejected {
        families.push(AcceptanceFamily::PermanentFloor);
    }
    if assessment
        .determinant
        .is_some_and(|value| value.test.verdict == AcceptanceVerdict::Rejected)
    {
        families.push(AcceptanceFamily::Determinant);
    }
    families
}

fn validate_halt_state(receipt: &CampaignCoordinatorReceipt) -> Result<(), CoordinatorError> {
    match &receipt.halt {
        CampaignHaltState::Running => {
            if receipt.cells.iter().any(|cell| match &cell.execution {
                CellExecutionState::Completed { assessment, .. } => assessment.rejected(),
                CellExecutionState::Halted { .. } => true,
                CellExecutionState::Pending | CellExecutionState::Scheduled { .. } => false,
            }) {
                return refused("running campaign contains rejecting or halted terminal evidence");
            }
        }
        CampaignHaltState::Halted { cause } => {
            if !receipt
                .cells
                .iter()
                .all(|cell| is_terminal(&cell.execution))
            {
                return refused("halted campaign contains a nonterminal cell");
            }
            let (cause_q, cause_n) =
                match cause {
                    CampaignHaltCause::Acceptance {
                        q,
                        n,
                        rejected_families: stored,
                    } => {
                        let Some(CellExecutionState::Completed { assessment, .. }) = receipt
                            .cells
                            .iter()
                            .find(|cell| (cell.q, cell.n) == (*q, *n))
                            .map(|cell| &cell.execution)
                        else {
                            return refused("acceptance halt does not name a completed cell");
                        };
                        if rejected_families(assessment) != *stored || stored.is_empty() {
                            return refused("acceptance halt families differ from exact decisions");
                        }
                        for cell in &receipt.cells {
                            if (cell.q, cell.n) == (*q, *n) {
                                continue;
                            }
                            match &cell.execution {
                                CellExecutionState::Completed { assessment, .. }
                                    if !assessment.rejected() => {}
                                CellExecutionState::Halted { summary, .. }
                                    if matches!(
                                        summary.terminal_state,
                                        CellTerminalState::Halted {
                                            reason: HaltReason::AcceptanceFailure
                                        }
                                    ) => {}
                                _ => return refused(
                                    "acceptance halt is inconsistent with another terminal cell",
                                ),
                            }
                        }
                        (*q, *n)
                    }
                    CampaignHaltCause::Mechanical { q, n, reason } => {
                        if *reason == HaltReason::AcceptanceFailure {
                            return refused("mechanical halt uses an acceptance reason");
                        }
                        let Some(CellExecutionState::Halted { summary, .. }) = receipt
                            .cells
                            .iter()
                            .find(|cell| (cell.q, cell.n) == (*q, *n))
                            .map(|cell| &cell.execution)
                        else {
                            return refused("mechanical halt does not name a halted cell");
                        };
                        if !matches!(
                            summary.terminal_state,
                            CellTerminalState::Halted { reason: stored } if stored == *reason
                        ) {
                            return refused("mechanical halt reason differs from its cause cell");
                        }
                        for cell in &receipt.cells {
                            match &cell.execution {
                                CellExecutionState::Completed { assessment, .. }
                                    if !assessment.rejected() => {}
                                CellExecutionState::Halted { summary, .. }
                                    if matches!(
                                        summary.terminal_state,
                                        CellTerminalState::Halted { reason: stored }
                                            if stored == *reason
                                    ) => {}
                                _ => return refused(
                                    "mechanical halt is inconsistent with another terminal cell",
                                ),
                            }
                        }
                        (*q, *n)
                    }
                };
            if receipt.arms.last().map(|arm| arm.scope)
                != Some(ExactCellScope {
                    q: cause_q,
                    n: cause_n,
                })
            {
                return refused("campaign halt cause is not the final admitted arm");
            }
        }
    }
    Ok(())
}

fn validate_monotonic_transition(
    prior: &CampaignCoordinatorReceipt,
    next: &CampaignCoordinatorReceipt,
) -> Result<(), CoordinatorError> {
    if prior.schema_version != next.schema_version
        || prior.campaign_id != next.campaign_id
        || prior.manifest_identity != next.manifest_identity
        || prior.protocol_identity != next.protocol_identity
        || prior.evidence_sources != next.evidence_sources
        || prior.acceptance_plan != next.acceptance_plan
        || prior.retry_rule != next.retry_rule
        || !next.arms.starts_with(&prior.arms)
        || next.attempts.len() < prior.attempts.len()
        || prior.cells.len() != next.cells.len()
    {
        return refused("receipt update changes frozen identity or prior append-only evidence");
    }
    for (index, old) in prior.attempts.iter().enumerate() {
        let new = &next.attempts[index];
        let terminalizes_active = index + 1 == prior.attempts.len()
            && matches!(old.state, ShardAttemptState::Authorized)
            && !matches!(new.state, ShardAttemptState::Authorized)
            && same_attempt_identity(old, new);
        if old != new && !terminalizes_active {
            return refused("receipt update changes prior shard-attempt evidence");
        }
    }
    for (old, new) in prior.cells.iter().zip(&next.cells) {
        if (old.q, old.n) != (new.q, new.n) {
            return refused("receipt update changes prior terminal cell evidence");
        }
        let allowed = match (&old.execution, &new.execution) {
            (CellExecutionState::Pending, CellExecutionState::Pending) => true,
            (CellExecutionState::Pending, CellExecutionState::Scheduled { .. }) => true,
            (CellExecutionState::Pending, CellExecutionState::Halted { .. }) => {
                matches!(next.halt, CampaignHaltState::Halted { .. })
            }
            (
                CellExecutionState::Scheduled {
                    arm_index: old_index,
                },
                CellExecutionState::Scheduled {
                    arm_index: new_index,
                },
            ) => old_index == new_index,
            (CellExecutionState::Scheduled { .. }, state) => is_terminal(state),
            (old_state, new_state) if is_terminal(old_state) => old_state == new_state,
            _ => false,
        };
        if !allowed {
            return refused("receipt update violates the cell lifecycle transition graph");
        }
    }
    match (&prior.halt, &next.halt) {
        (CampaignHaltState::Running, _) => {}
        (CampaignHaltState::Halted { .. }, CampaignHaltState::Halted { .. })
            if prior.halt == next.halt => {}
        _ => return refused("receipt update changes the terminal campaign halt"),
    }
    Ok(())
}

fn same_attempt_identity(left: &ShardAttempt, right: &ShardAttempt) -> bool {
    left.stream_address == right.stream_address
        && left.shard_id == right.shard_id
        && left.attempt == right.attempt
        && left.backend == right.backend
        && left.backend_receipt == right.backend_receipt
        && left.rng_algorithm == right.rng_algorithm
        && left.rng_version == right.rng_version
}

fn validate_campaign_directory(
    campaign_root: &Path,
    campaign_id: &CampaignId,
) -> Result<(), CoordinatorError> {
    if campaign_root.file_name().and_then(|name| name.to_str())
        != Some(campaign_id.to_string().as_str())
    {
        return refused("campaign directory name differs from coordinator campaign id");
    }
    Ok(())
}

fn is_terminal(state: &CellExecutionState) -> bool {
    matches!(
        state,
        CellExecutionState::Completed { .. } | CellExecutionState::Halted { .. }
    )
}

fn digest(bytes: &[u8]) -> Sha256Digest {
    let text = format!("{:x}", Sha256::digest(bytes));
    text.parse()
        .expect("SHA-256 formatting produces a canonical lowercase digest")
}

fn atomic_json(
    root: &CampaignRoot,
    relative: &Path,
    value: &impl Serialize,
) -> Result<(), CoordinatorError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(CoordinatorError::Json)?;
    root.write_atomic_replace(relative, &bytes)
        .map_err(|source| CoordinatorError::Io {
            path: root.path().join(relative),
            source,
        })
}

fn write_new_json(
    root: &CampaignRoot,
    relative: &Path,
    value: &impl Serialize,
) -> Result<(), CoordinatorError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(CoordinatorError::Json)?;
    root.write_atomic_new_or_adopt(relative, &bytes)
        .map_err(|source| CoordinatorError::Io {
            path: root.path().join(relative),
            source,
        })
}

fn open_campaign_root(path: &Path) -> Result<CampaignRoot, CoordinatorError> {
    CampaignRoot::open(path).map_err(|source| CoordinatorError::Io {
        path: path.to_owned(),
        source,
    })
}

fn read_manifest_anchored(root: &CampaignRoot) -> Result<CampaignManifest, CoordinatorError> {
    let relative = Path::new(MANIFEST_FILE);
    let bytes = campaign_read(root, relative)?;
    super::schema::read_manifest_bytes(&bytes, &root.path().join(relative))
        .map_err(CoordinatorError::Manifest)
}

fn campaign_read(root: &CampaignRoot, relative: &Path) -> Result<Vec<u8>, CoordinatorError> {
    root.read(relative).map_err(|source| CoordinatorError::Io {
        path: root.path().join(relative),
        source,
    })
}

fn campaign_root_entry(
    root: &CampaignRoot,
    relative: &Path,
) -> Result<EntryKind, CoordinatorError> {
    root.entry_kind(relative)
        .map_err(|source| CoordinatorError::Io {
            path: root.path().join(relative),
            source,
        })
}

fn refused<T>(message: &str) -> Result<T, CoordinatorError> {
    Err(CoordinatorError::Refused(message.to_owned()))
}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod tests;
