//! Persisted outside-coordinator state for the frozen permanent campaign.
//!
//! The coordinator owns schedule admission, manifest-derived attempt history,
//! terminal cell state, campaign halts, and field interpretation sidecars. The
//! exact-cell emitter remains a subordinate arm and cannot finalize a field.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::acceptance::{
    assess_completed_cell, AcceptanceError, AcceptanceFamily, AcceptancePlan,
    CompletedCellAssessment,
};
use super::launch_cost::read_accelerator_cost_table;
use super::schedule::CAMPAIGN_CELL_PURPOSE_TAG;
use super::schedule::{
    emit_shard_with_durability_hook, enumerate_cell_work_items,
    evaluate_work_item_with_worker_count_and_accelerator, AcceleratorConfig, AcceleratorCostTable,
    EvaluatedShard, ScheduleError, WorkItem, DEFAULT_ACCELERATOR_LAUNCH_CAP,
};
use super::schema::{
    shard_record_file, AcceptanceVerdict, ArtifactIdentity, Backend, CampaignId, CampaignManifest,
    CellSpec, CellTerminalState, DeterminantCount, DeterminantPlan, FieldSummary, HaltReason,
    QuarantinedShard, RngAlgorithm, Sha256Digest, ShardRecord, StreamAddress, SummaryRow,
    DATASET_HOME, MANIFEST_FILE, SCHEMA_VERSION,
};

const COORDINATOR_SCHEMA_VERSION: u32 = 1;
const FIRST_FIELD: u8 = 7;
const FIRST_ORDER: u16 = 20;
const COORDINATOR_DIRECTORY: &str = "campaign-coordinator";
const COORDINATOR_RECEIPT_FILE: &str = "coordinator-receipt.json";
const PROTOCOL_PATH: &str = "dev/simulation_results/permanent-zero-fraction/protocol.md";
const Q3_TARGET_PATH: &str =
    "dev/simulation_results/permanent-zero-fraction/scheinerman2024-q3-targets-v1.csv";
const SEARCH_RECEIPT_PATH: &str = "dev/studies/b488f02c/literature-search-2026-08-08.md";

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
    pub fn new(campaign_root: &Path) -> Result<Self, CoordinatorError> {
        let manifest =
            super::schema::read_manifest(campaign_root).map_err(CoordinatorError::Manifest)?;
        validate_campaign_directory(campaign_root, &manifest.campaign_id)?;
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
    pub fn authorize_arm(&mut self, arm: ArmInvocation) -> Result<(), CoordinatorError> {
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
    pub fn authorize_or_resume_arm(&mut self, arm: ArmInvocation) -> Result<(), CoordinatorError> {
        let scope = arm.scope;
        match self.cell_state(scope.q, scope.n) {
            Some(CellExecutionState::Pending) => self.authorize_arm(arm),
            Some(CellExecutionState::Scheduled { arm_index }) => {
                if self.receipt.arms.get(*arm_index) == Some(&arm) {
                    Ok(())
                } else {
                    refused("scheduled arm differs from the persisted exact invocation")
                }
            }
            Some(CellExecutionState::Completed { .. } | CellExecutionState::Halted { .. }) => {
                refused("terminal cell cannot be executed again")
            }
            None => refused("arm scope does not name a manifest cell"),
        }
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
        campaign_root: &Path,
        q: u8,
        n: u16,
        shard_id: u64,
    ) -> Result<(), CoordinatorError> {
        validate_campaign_directory(campaign_root, &self.receipt.campaign_id)?;
        let cell = self.manifest_cell(q, n)?.clone();
        let shard = cell
            .shards
            .iter()
            .find(|shard| shard.shard_id == shard_id)
            .ok_or_else(|| CoordinatorError::Refused("manifest shard not found".to_owned()))?;
        let relative = shard_record_file(q, n, shard_id);
        let path = campaign_root.join(&relative);
        let bytes = fs::read(&path).map_err(|source| CoordinatorError::Io {
            path: path.clone(),
            source,
        })?;
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
    /// [`execute_scheduled_cell`].
    #[cfg(any(test, feature = "test-support"))]
    pub fn authorize_attempt(
        &mut self,
        q: u8,
        n: u16,
        shard_id: u64,
    ) -> Result<(), CoordinatorError> {
        self.authorize_attempt_inner(q, n, shard_id)
    }

    /// Test-support terminalization of an active mechanical attempt.
    ///
    /// Production campaign execution records this state only through
    /// [`execute_scheduled_cell`].
    #[cfg(any(test, feature = "test-support"))]
    pub fn record_quarantine(
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
    /// [`execute_scheduled_cell`].
    #[cfg(any(test, feature = "test-support"))]
    pub fn record_accepted(
        &mut self,
        campaign_root: &Path,
        q: u8,
        n: u16,
        shard_id: u64,
    ) -> Result<(), CoordinatorError> {
        self.record_accepted_inner(campaign_root, q, n, shard_id)
    }

    /// Test-support assessment of counts pooled from accepted attempts.
    ///
    /// Production campaign execution assesses a complete cell only through
    /// [`execute_scheduled_cell`].
    #[cfg(any(test, feature = "test-support"))]
    pub fn record_completed(
        &mut self,
        q: u8,
        n: u16,
        matrix_count: u64,
        permanent_zero_count: u64,
        determinant: DeterminantCount,
    ) -> Result<(), CoordinatorError> {
        self.record_completed_inner(q, n, matrix_count, permanent_zero_count, determinant)
    }

    /// Records a backend-unavailable or fatal mechanical terminal path.
    ///
    /// Accepted shards remain pooled in the halted row and receipt. The halt
    /// never produces an acceptance verdict and spends no error budget.
    ///
    /// # Errors
    ///
    /// Refuses an acceptance-failure reason or a cell that is not scheduled.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(CS)` in the worst case while terminalizing remaining cells.
    #[cfg(any(test, feature = "test-support"))]
    pub fn record_terminal_failure(
        &mut self,
        q: u8,
        n: u16,
        reason: HaltReason,
    ) -> Result<(), CoordinatorError> {
        if reason == HaltReason::AcceptanceFailure {
            return refused("acceptance failure is produced only by an exact decision");
        }
        if !matches!(
            self.cell_state(q, n),
            Some(CellExecutionState::Scheduled { .. })
        ) {
            return refused("mechanical halt requires a scheduled cell");
        }
        self.terminalize_mechanical(q, n, reason)
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
    pub fn assemble_field_summary(&self, q: u8) -> Result<FieldSummary, CoordinatorError> {
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
        let quarantined = self
            .receipt
            .attempts
            .iter()
            .filter_map(|attempt| match &attempt.state {
                ShardAttemptState::Quarantined { error } if attempt.stream_address.q == q => {
                    Some(QuarantinedShard {
                        q,
                        n: attempt.stream_address.n,
                        shard_id: attempt.shard_id,
                        error: error.clone(),
                    })
                }
                _ => None,
            })
            .collect();
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
    pub fn persist(&self, campaign_root: &Path) -> Result<(), CoordinatorError> {
        validate_campaign_directory(campaign_root, &self.receipt.campaign_id)?;
        validate_receipt(&self.manifest, &self.receipt)?;
        validate_campaign_files(campaign_root, &self.manifest, &self.receipt)?;
        let path = coordinator_receipt_path(campaign_root, &self.receipt.campaign_id);
        if path.exists() {
            let bytes = fs::read(&path).map_err(|source| CoordinatorError::Io {
                path: path.clone(),
                source,
            })?;
            let prior: CampaignCoordinatorReceipt =
                serde_json::from_slice(&bytes).map_err(CoordinatorError::Json)?;
            validate_receipt(&self.manifest, &prior)?;
            validate_on_disk_attempts(campaign_root, &self.manifest, &prior)?;
            validate_monotonic_transition(&prior, &self.receipt)?;
        }
        atomic_json(&path, &self.receipt)
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
        let manifest =
            super::schema::read_manifest(campaign_root).map_err(CoordinatorError::Manifest)?;
        validate_campaign_directory(campaign_root, &manifest.campaign_id)?;
        let path = coordinator_receipt_path(campaign_root, &manifest.campaign_id);
        let bytes = fs::read(&path).map_err(|source| CoordinatorError::Io {
            path: path.clone(),
            source,
        })?;
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
    campaign_root
        .join("derived")
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
    campaign_root
        .join("derived")
        .join(campaign_id.to_string())
        .join(COORDINATOR_DIRECTORY)
        .join("execution.lock")
}

/// Executes one persisted exact-cell arm through the canonical receipt state machine.
///
/// The function holds the campaign execution lock across receipt revalidation,
/// attempt authorization, evaluation, durable raw emission, and attempt
/// terminalization. Every attempt authorization is persisted and re-read
/// before the sampler is entered. A durable raw shard left by an interrupted
/// process is adopted without another sampler entry.
///
/// # Errors
///
/// Refuses an unpersisted or mismatched arm, lock contention, an invalid full
/// accelerator-cost table, inconsistent durable raw evidence, or a receipt
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
pub fn execute_scheduled_cell(
    campaign_root: &Path,
    scope: ExactCellScope,
    worker_count: usize,
) -> Result<ExactCellExecution, CoordinatorError> {
    execute_scheduled_cell_inner(
        campaign_root,
        scope,
        worker_count,
        evaluate_work_item_with_worker_count_and_accelerator,
    )
}

/// Deterministic evaluator seam for receipt-state-machine integration tests.
///
/// This entry point has the same persisted admission, locking, retry, and raw
/// durability behavior as [`execute_scheduled_cell`]. Only shard evaluation is
/// supplied by the test.
///
/// # Errors
///
/// Returns the same failures as [`execute_scheduled_cell`] plus failures from
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
pub fn execute_scheduled_cell_with_evaluator<E>(
    campaign_root: &Path,
    scope: ExactCellScope,
    worker_count: usize,
    evaluator: E,
) -> Result<ExactCellExecution, CoordinatorError>
where
    E: FnMut(
        &CampaignManifest,
        &WorkItem,
        usize,
        Option<AcceleratorConfig>,
    ) -> Result<EvaluatedShard, ScheduleError>,
{
    execute_scheduled_cell_inner(campaign_root, scope, worker_count, evaluator)
}

fn execute_scheduled_cell_inner<E>(
    campaign_root: &Path,
    scope: ExactCellScope,
    worker_count: usize,
    mut evaluator: E,
) -> Result<ExactCellExecution, CoordinatorError>
where
    E: FnMut(
        &CampaignManifest,
        &WorkItem,
        usize,
        Option<AcceleratorConfig>,
    ) -> Result<EvaluatedShard, ScheduleError>,
{
    let preflight = CampaignCoordinator::read(campaign_root)?;
    let manifest = preflight.manifest.clone();
    let preflight_arm = preflight.persisted_arm_authorization(scope)?;
    if worker_count != preflight_arm.arm.worker_count {
        return refused("runtime worker count differs from the persisted exact arm");
    }
    let accelerator_costs = accelerator_costs_for_arm(&manifest, &preflight_arm.arm)?;
    accelerator_costs.validate_manifest(&manifest)?;
    let lock_path = coordinator_lock_path(campaign_root, &manifest.campaign_id);
    let lock_parent = lock_path
        .parent()
        .ok_or_else(|| CoordinatorError::Refused("execution lock has no parent".to_owned()))?;
    fs::create_dir_all(lock_parent).map_err(|source| CoordinatorError::Io {
        path: lock_parent.to_owned(),
        source,
    })?;
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| CoordinatorError::Io {
            path: lock_path.clone(),
            source,
        })?;
    if let Err(source) = lock_file.try_lock() {
        return Err(CoordinatorError::Refused(format!(
            "campaign execution lock is unavailable at {}: {source}",
            lock_path.display()
        )));
    }

    let mut coordinator = CampaignCoordinator::read(campaign_root)?;
    let arm_authorization = coordinator.persisted_arm_authorization(scope)?;
    if arm_authorization.scope != preflight_arm.scope
        || arm_authorization.manifest_identity != preflight_arm.manifest_identity
        || arm_authorization.arm != preflight_arm.arm
    {
        return refused("persisted exact arm changed while acquiring the execution lock");
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
                return exact_execution(campaign_root, &coordinator, scope, &items);
            }

            if let Some(active) = active_attempt(&coordinator.receipt, item).cloned() {
                let raw_path =
                    campaign_root.join(shard_record_file(scope.q, scope.n, item.shard_id));
                if raw_path.is_file() {
                    match coordinator.record_accepted_inner(
                        campaign_root,
                        scope.q,
                        scope.n,
                        item.shard_id,
                    ) {
                        Ok(()) => {
                            coordinator.persist(campaign_root)?;
                            continue;
                        }
                        Err(error) => {
                            preserve_invalid_raw(
                                campaign_root,
                                &coordinator.receipt.campaign_id,
                                item,
                                active.attempt,
                                &raw_path,
                            )?;
                            coordinator.record_quarantine_inner(
                                scope.q,
                                scope.n,
                                item.shard_id,
                                format!("durable raw shard failed canonical validation: {error}"),
                            )?;
                            coordinator.persist(campaign_root)?;
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
                coordinator.persist(campaign_root)?;
                continue;
            }

            let raw_path = campaign_root.join(shard_record_file(scope.q, scope.n, item.shard_id));
            if raw_path.exists() {
                return refused("raw shard exists without an active or accepted attempt");
            }
            coordinator.authorize_attempt_inner(scope.q, scope.n, item.shard_id)?;
            coordinator.persist(campaign_root)?;
            coordinator = CampaignCoordinator::read(campaign_root)?;
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
                                |_| {},
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
                            coordinator.persist(campaign_root)?;
                        }
                        Err(error) => {
                            let raw_path = campaign_root.join(shard_record_file(
                                scope.q,
                                scope.n,
                                item.shard_id,
                            ));
                            if raw_path.exists() {
                                preserve_invalid_raw(
                                    campaign_root,
                                    &coordinator.receipt.campaign_id,
                                    item,
                                    attempt_authorization.attempt.attempt,
                                    &raw_path,
                                )?;
                            }
                            coordinator.record_quarantine_inner(
                                scope.q,
                                scope.n,
                                item.shard_id,
                                error.to_string(),
                            )?;
                            coordinator.persist(campaign_root)?;
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
                    coordinator.persist(campaign_root)?;
                }
            }
        }
    }
    coordinator.record_completed_from_attempts(scope.q, scope.n)?;
    coordinator.persist(campaign_root)?;
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

fn accelerator_costs_for_arm(
    manifest: &CampaignManifest,
    arm: &ArmInvocation,
) -> Result<AcceleratorCostTable, CoordinatorError> {
    let launch_cap = match optional_unique_option(&arm.argv, "--accelerator-launch-cap-ms")? {
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
    match optional_unique_option(&arm.argv, "--accelerator-cost-table")? {
        Some(path) => {
            read_accelerator_cost_table(Path::new(&path), manifest, launch_cap).map_err(|error| {
                CoordinatorError::Refused(format!(
                    "persisted accelerator cost table {path} is invalid: {error}"
                ))
            })
        }
        None => Ok(AcceleratorCostTable::default()),
    }
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
    campaign_root: &Path,
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
            let path = campaign_root.join(shard_record_file(item.q, item.n, item.shard_id));
            let bytes = fs::read(&path).map_err(|source| CoordinatorError::Io {
                path: path.clone(),
                source,
            })?;
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
    campaign_root: &Path,
    campaign_id: &CampaignId,
    item: &WorkItem,
    attempt: u8,
    raw_path: &Path,
) -> Result<(), CoordinatorError> {
    let path = campaign_root
        .join("derived")
        .join(campaign_id.to_string())
        .join(COORDINATOR_DIRECTORY)
        .join("quarantine")
        .join(format!(
            "q{}-n{}-shard-{:06}-attempt-{attempt}.json",
            item.q, item.n, item.shard_id
        ));
    let parent = path
        .parent()
        .ok_or_else(|| CoordinatorError::Refused("quarantine path has no parent".to_owned()))?;
    fs::create_dir_all(parent).map_err(|source| CoordinatorError::Io {
        path: parent.to_owned(),
        source,
    })?;
    if path.exists() {
        return refused("quarantine evidence path already exists");
    }
    fs::rename(raw_path, &path).map_err(|source| CoordinatorError::Io {
        path: path.clone(),
        source,
    })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| CoordinatorError::Io {
            path: parent.to_owned(),
            source,
        })?;
    let raw_parent = raw_path
        .parent()
        .ok_or_else(|| CoordinatorError::Refused("raw shard path has no parent".to_owned()))?;
    fs::File::open(raw_parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| CoordinatorError::Io {
            path: raw_parent.to_owned(),
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

/// Field-qualified interpretation represented by one derived sidecar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldInterpretation {
    /// `q=3` comparison to the versioned Scheinerman target table.
    PublishedTargetComparison {
        /// Versioned target-table content identity.
        target_table: ArtifactIdentity,
    },
    /// Conditional `q=5` or `q=7` literature-search basis.
    LiteratureSearchBasis {
        /// Recorded bounded search and limitations identity.
        search_receipt: ArtifactIdentity,
        /// Closed claim projected from the recorded search.
        claim: LiteratureSearchClaim,
    },
}

/// Terminal availability state of one field-derived sidecar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSidecarStatus {
    /// Every field cell completed.
    Completed,
    /// Every field cell is terminal and at least one halted.
    Halted,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct FieldSidecar<'a> {
    schema_version: u32,
    campaign_id: &'a CampaignId,
    manifest_identity: &'a ArtifactIdentity,
    q: u8,
    source_records: Vec<ArtifactIdentity>,
    status: FieldSidecarStatus,
    interpretation: FieldInterpretation,
}

/// Emits one coordinator-owned field sidecar at its exact derived path.
///
/// # Errors
///
/// Refuses nonterminal fields, empty evidence, a field/interpretation mismatch,
/// a noncanonical source identity, or an existing sidecar. The function never
/// opens raw shard, summary, or checksum paths.
///
/// # Panics
///
/// Does not panic.
///
/// # Complexity
///
/// `O(F + B)` for receipt fields and serialized bytes.
pub fn emit_field_sidecar(
    campaign_root: &Path,
    receipt: &CampaignCoordinatorReceipt,
    q: u8,
    source_records: Vec<ArtifactIdentity>,
    interpretation: FieldInterpretation,
) -> Result<PathBuf, CoordinatorError> {
    validate_campaign_directory(campaign_root, &receipt.campaign_id)?;
    validate_interpretation(q, &interpretation)?;
    let field = receipt
        .fields
        .iter()
        .find(|field| field.q == q)
        .ok_or_else(|| CoordinatorError::Refused("sidecar field is not manifested".to_owned()))?;
    let status = match field.execution {
        FieldExecutionState::Completed => FieldSidecarStatus::Completed,
        FieldExecutionState::Halted => FieldSidecarStatus::Halted,
        FieldExecutionState::InProgress => {
            return refused("sidecar requires a completed-or-halted field")
        }
    };
    if source_records.is_empty() {
        return refused("sidecar requires source-record identities");
    }
    let path = campaign_root
        .join("derived")
        .join(receipt.campaign_id.to_string())
        .join(COORDINATOR_DIRECTORY)
        .join("field-sidecars")
        .join(format!("q{q}.json"));
    let sidecar = FieldSidecar {
        schema_version: COORDINATOR_SCHEMA_VERSION,
        campaign_id: &receipt.campaign_id,
        manifest_identity: &receipt.manifest_identity,
        q,
        source_records,
        status,
        interpretation,
    };
    write_new_json(&path, &sidecar)?;
    Ok(path)
}

fn validate_interpretation(
    q: u8,
    interpretation: &FieldInterpretation,
) -> Result<(), CoordinatorError> {
    match (q, interpretation) {
        (3, FieldInterpretation::PublishedTargetComparison { target_table })
            if target_table.path.as_str() == Q3_TARGET_PATH =>
        {
            Ok(())
        }
        (
            5 | 7,
            FieldInterpretation::LiteratureSearchBasis {
                search_receipt,
                claim: LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
            },
        ) if search_receipt.path.as_str() == SEARCH_RECEIPT_PATH => Ok(()),
        _ => refused("field interpretation does not bind its canonical source and semantics"),
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
    if protocol_identity.path.as_str() != PROTOCOL_PATH {
        return refused("protocol identity path differs from the frozen protocol path");
    }
    Ok(())
}

fn identity_for_campaign_file(
    campaign_root: &Path,
    campaign_id: &CampaignId,
    relative: &str,
) -> Result<ArtifactIdentity, CoordinatorError> {
    let path = campaign_root.join(relative);
    let bytes = fs::read(&path).map_err(|source| CoordinatorError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(ArtifactIdentity {
        path: format!("{DATASET_HOME}/{campaign_id}/{relative}")
            .parse()
            .map_err(|error| {
                CoordinatorError::Refused(format!("invalid artifact path: {error}"))
            })?,
        sha256: digest(&bytes),
    })
}

fn identity_for_protocol(campaign_root: &Path) -> Result<ArtifactIdentity, CoordinatorError> {
    let dataset_root = campaign_root.parent().ok_or_else(|| {
        CoordinatorError::Refused("campaign root has no dataset parent".to_owned())
    })?;
    let path = dataset_root.join("protocol.md");
    let bytes = fs::read(&path).map_err(|source| CoordinatorError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(ArtifactIdentity {
        path: PROTOCOL_PATH
            .parse()
            .expect("canonical protocol path is normalized"),
        sha256: digest(&bytes),
    })
}

fn validate_campaign_files(
    campaign_root: &Path,
    manifest: &CampaignManifest,
    receipt: &CampaignCoordinatorReceipt,
) -> Result<(), CoordinatorError> {
    let on_disk =
        super::schema::read_manifest(campaign_root).map_err(CoordinatorError::Manifest)?;
    if on_disk != *manifest
        || identity_for_campaign_file(campaign_root, &manifest.campaign_id, MANIFEST_FILE)?
            != receipt.manifest_identity
        || identity_for_protocol(campaign_root)? != receipt.protocol_identity
    {
        return refused("on-disk frozen manifest or protocol identity differs from the receipt");
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
        if matches!(attempt.state, ShardAttemptState::Authorized) {
            if authorized_attempt.replace((attempt_index, q, n)).is_some() {
                return refused("receipt contains more than one active authorized attempt");
            }
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
    campaign_root: &Path,
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
        let path = campaign_root.join(shard_record_file(q, n, attempt.shard_id));
        let bytes = fs::read(&path).map_err(|source| CoordinatorError::Io {
            path: path.clone(),
            source,
        })?;
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

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), CoordinatorError> {
    let parent = path
        .parent()
        .ok_or_else(|| CoordinatorError::Refused("coordinator path has no parent".to_owned()))?;
    fs::create_dir_all(parent).map_err(|source| CoordinatorError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let temporary = parent.join(format!(
        ".{COORDINATOR_RECEIPT_FILE}.{}.tmp",
        std::process::id()
    ));
    if temporary.exists() {
        return refused("coordinator temporary path already exists");
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(CoordinatorError::Json)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| CoordinatorError::Io {
            path: temporary.clone(),
            source,
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| CoordinatorError::Io {
            path: temporary.clone(),
            source,
        })?;
    fs::rename(&temporary, path).map_err(|source| CoordinatorError::Io {
        path: path.to_owned(),
        source,
    })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| CoordinatorError::Io {
            path: parent.to_owned(),
            source,
        })
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), CoordinatorError> {
    let parent = path
        .parent()
        .ok_or_else(|| CoordinatorError::Refused("sidecar path has no parent".to_owned()))?;
    fs::create_dir_all(parent).map_err(|source| CoordinatorError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let bytes = serde_json::to_vec_pretty(value).map_err(CoordinatorError::Json)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| CoordinatorError::Io {
            path: path.to_owned(),
            source,
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| CoordinatorError::Io {
            path: path.to_owned(),
            source,
        })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| CoordinatorError::Io {
            path: parent.to_owned(),
            source,
        })
}

fn refused<T>(message: &str) -> Result<T, CoordinatorError> {
    Err(CoordinatorError::Refused(message.to_owned()))
}
