//! Checkpointed permanent-campaign execution.
//!
//! The driver commits one shard record before it commits the corresponding
//! checkpoint state. On resume it also adopts an already-emitted shard, which
//! closes the crash window between those two durable writes without redoing
//! completed work. The configuration identity covers the complete manifest
//! and selected field, while output paths and worker count remain outside the
//! identity so a later processor can change its execution width safely.
//!
//! A normal caller passes the campaign output directory and a checkpoint path
//! to [`run_field_checkpointed`]. The driver writes each durable shard before
//! recording it in the checkpoint, observes interruption at the next work-item
//! boundary, and writes a field summary only after every item is completed or
//! quarantined. A restart uses the same configuration and checkpoint: completed
//! items are loaded, an emitted-but-uncheckpointed shard is adopted after a
//! deterministic byte comparison, and remaining work continues. A changed
//! manifest configuration is refused with named component differences. These
//! are the operational contracts of `@/inv/campaign-resumability` and
//! `@/inv/deterministic-seeded-execution`. Manifest-selected unavailable
//! backends are refused during pre-flight and never enter quarantine.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::checkpoint::{
    CheckpointLoadError, CheckpointPayload, CheckpointReader, CheckpointWriter,
};
use crate::permanent_campaign::schedule::{
    emit_shard_with_durability_hook, emit_summary_with_durability_hook, enumerate_cell_work_items,
    enumerate_work_items, evaluate_work_item_with_worker_count_and_accelerator,
    resolve_processor_path, shard_record_bytes, summarize_with_quarantine, AcceleratorCostTable,
    EvaluatedShard, PhaseDurations, ScheduleError, ShardRun, WorkItem,
};
use crate::permanent_campaign::schema::{
    field_summary_file, shard_record_file, Backend, CampaignManifest, QuarantinedShard, ShardRecord,
};
use crate::snr_checkpoint::is_interrupted;

const CHECKPOINT_SCHEMA_VERSION: u32 = 3;

/// Stable identity of one scheduler work item.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WorkItemId {
    /// Prime field order.
    pub q: u8,
    /// Matrix dimension.
    pub n: u16,
    /// Cell-local shard identity.
    pub shard_id: u64,
}

impl From<&WorkItem> for WorkItemId {
    fn from(item: &WorkItem) -> Self {
        Self {
            q: item.q,
            n: item.n,
            shard_id: item.shard_id,
        }
    }
}

/// Per-worker continuation evidence stored at a checkpoint boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkerCheckpoint {
    /// Stable logical worker index.
    pub worker_index: usize,
    /// Absolute generator word position after the worker's latest item.
    pub generator_word_position: u128,
}

/// Generic-checkpoint payload for one permanent-campaign field arm.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CampaignCheckpoint {
    /// The individually comparable configuration components for this resume;
    /// a mismatch is a hard refusal rather than fresh work.
    pub configuration: CampaignConfiguration,
    /// Work items whose records are durably emitted.
    pub completed: Vec<WorkItemId>,
    /// Work items retained as evaluation failures.
    pub quarantined: Vec<QuarantinedShard>,
    /// Absolute sampler positions observed at worker boundaries.
    pub worker_states: Vec<WorkerCheckpoint>,
    /// Whether every work item in the selected scope is terminal.
    pub scope_complete: bool,
}

impl CampaignCheckpoint {
    /// Creates an empty checkpoint for one field-arm configuration.
    #[must_use]
    pub fn new(configuration: CampaignConfiguration) -> Self {
        Self {
            configuration,
            completed: Vec::new(),
            quarantined: Vec::new(),
            worker_states: Vec::new(),
            scope_complete: false,
        }
    }
}

/// Manifest scope represented by one campaign checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CampaignExecutionScope {
    /// Every cell in one field arm.
    Field {
        /// Prime field order.
        q: u8,
    },
    /// One exact `(q, n)` manifest cell.
    ExactCell {
        /// Prime field order.
        q: u8,
        /// Matrix order.
        n: u16,
    },
}

/// Completed checkpointed execution for one semantic campaign scope.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckpointedCampaignRun {
    scope: CampaignExecutionScope,
    shards: Vec<ShardRun>,
    summary: crate::permanent_campaign::schema::FieldSummary,
}

impl CheckpointedCampaignRun {
    /// Returns the exact field or cell scope executed by this run.
    #[must_use]
    pub const fn scope(&self) -> CampaignExecutionScope {
        self.scope
    }

    /// Returns the field order containing the selected scope.
    #[must_use]
    pub const fn q(&self) -> u8 {
        match self.scope {
            CampaignExecutionScope::Field { q } | CampaignExecutionScope::ExactCell { q, .. } => q,
        }
    }

    /// Returns durable shard results in deterministic order.
    #[must_use]
    pub fn shards(&self) -> &[ShardRun] {
        &self.shards
    }

    /// Returns the rows and quarantine index for exactly this run's scope.
    #[must_use]
    pub const fn summary(&self) -> &crate::permanent_campaign::schema::FieldSummary {
        &self.summary
    }
}

impl CampaignExecutionScope {
    fn identity(self) -> String {
        match self {
            Self::Field { q } => format!("field:q={q}"),
            Self::ExactCell { q, n } => format!("exact_cell:q={q},n={n}"),
        }
    }
}

/// Individually comparable configuration components recorded in a checkpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CampaignConfiguration {
    /// BLAKE3 digest of the complete serialized campaign manifest.
    pub manifest_content_hash: String,
    /// Immutable campaign directory identity.
    pub campaign_id: String,
    /// Exact execution scope selected by the invocation.
    pub scope: CampaignExecutionScope,
    /// Campaign-wide sampler root seed.
    pub root_seed: u64,
}

impl CheckpointPayload for CampaignCheckpoint {
    const IDENTITY: &'static str = "permanent-campaign/scoped-progress";
    const SCHEMA_VERSION: u32 = CHECKPOINT_SCHEMA_VERSION;
}

/// Error returned by the checkpointed campaign driver.
#[derive(Debug)]
pub enum CampaignDriverError {
    /// A present checkpoint could not be safely loaded.
    Checkpoint(CheckpointLoadError),
    /// A dataset or checkpoint filesystem operation failed.
    Io {
        /// Path involved in the filesystem operation.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The scheduler rejected a work item or emission.
    Schedule(crate::permanent_campaign::schedule::ScheduleError),
    /// The caller requested a graceful interruption after the preceding shard
    /// or quarantine checkpoint was durably flushed; remaining work is left
    /// for the next invocation.
    Interrupted,
    /// A present checkpoint disagrees with the live campaign configuration.
    /// The message names each differing manifest hash, campaign id, field, or
    /// root seed component and includes checkpointed/current values.
    ResumeRefused(String),
    /// A previously emitted record is not a valid shard document.
    InvalidExistingShard {
        /// Existing shard path.
        path: PathBuf,
        /// Validation or serialized-byte-mismatch diagnostic.
        message: String,
    },
}

impl fmt::Display for CampaignDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Checkpoint(error) => write!(formatter, "campaign checkpoint failed: {error}"),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Schedule(error) => error.fmt(formatter),
            Self::Interrupted => formatter.write_str("campaign interrupted after checkpoint flush"),
            Self::ResumeRefused(message) => write!(formatter, "campaign resume refused: {message}"),
            Self::InvalidExistingShard { path, message } => {
                write!(
                    formatter,
                    "existing shard {} is invalid: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for CampaignDriverError {}

impl From<crate::permanent_campaign::schedule::ScheduleError> for CampaignDriverError {
    fn from(error: crate::permanent_campaign::schedule::ScheduleError) -> Self {
        Self::Schedule(error)
    }
}

/// Returns the stable configuration identity for one execution scope.
///
/// The digest covers the complete serialized manifest. The named components
/// are retained in the identity so a refusal can identify common disagreements
/// instead of exposing only an opaque digest.
#[must_use]
pub fn campaign_config_hash(manifest: &CampaignManifest, scope: CampaignExecutionScope) -> String {
    let configuration = campaign_configuration(manifest, scope);
    format!(
        "blake3:{digest};campaign_id={campaign_id};scope={scope};root_seed={root_seed};manifest={digest}",
        campaign_id = configuration.campaign_id,
        scope = configuration.scope.identity(),
        root_seed = configuration.root_seed,
        digest = configuration.manifest_content_hash,
    )
}

/// Returns the individually comparable scoped checkpoint configuration.
#[must_use]
pub fn campaign_configuration(
    manifest: &CampaignManifest,
    scope: CampaignExecutionScope,
) -> CampaignConfiguration {
    let bytes = serde_json::to_vec(manifest).expect("campaign manifest is serializable");
    CampaignConfiguration {
        manifest_content_hash: blake3::hash(&bytes).to_hex().to_string(),
        campaign_id: manifest.campaign_id.to_string(),
        scope,
        root_seed: manifest.root_seed,
    }
}

/// Returns the checkpoint path for one field arm in a campaign directory.
#[must_use]
pub fn field_checkpoint_path(root: &Path, field: u8) -> PathBuf {
    root.join(format!("campaign.q{field}.checkpoint.json"))
}

/// Returns the checkpoint path for one exact campaign cell.
#[must_use]
pub fn cell_checkpoint_path(root: &Path, field: u8, order: u16) -> PathBuf {
    root.join(format!("campaign.q{field}.n{order}.checkpoint.json"))
}

/// Runs one exact manifest cell with checkpointed shard execution.
///
/// The cell uses a cell-scoped configuration identity and checkpoint. It does
/// not emit the field summary: a field summary is canonical only after an
/// outside coordinator has terminal evidence for every cell in that field.
/// Accelerator costs remain a full-manifest input validated by the caller;
/// this function resolves the selected cell's frozen cost without deriving a
/// replacement or fallback.
///
/// # Errors
///
/// Returns the same checkpoint, selection, scheduler, and I/O failures as
/// [`run_field_checkpointed_with_accelerator_config`]. An invalid `(q, n)` is
/// rejected before the checkpoint reader or sampler is opened.
///
/// # Complexity
///
/// Linear in the selected cell's shards, with one checkpoint write per shard.
pub fn run_cell_checkpointed_with_accelerator_config(
    root: &Path,
    manifest: &CampaignManifest,
    field: u8,
    order: u16,
    checkpoint_path: &Path,
    worker_count: usize,
    accelerator: &AcceleratorCostTable,
) -> Result<CheckpointedCampaignRun, CampaignDriverError> {
    accelerator
        .validate_manifest(manifest)
        .map_err(CampaignDriverError::Schedule)?;
    let items = enumerate_cell_work_items(manifest, field, order)?;
    for item in &items {
        if item.backend == Backend::Accelerator {
            accelerator
                .config_for(item.q, item.n)
                .map_err(CampaignDriverError::Schedule)?;
        }
    }
    run_scope_checkpointed_inner(
        root,
        manifest,
        CampaignExecutionScope::ExactCell { q: field, n: order },
        checkpoint_path,
        worker_count,
        |item| {
            let cell = if item.backend == Backend::Accelerator {
                Some(accelerator.config_for(item.q, item.n)?)
            } else {
                None
            };
            evaluate_work_item_with_worker_count_and_accelerator(manifest, item, worker_count, cell)
        },
        DurabilityHooks {
            on_shard_durable: ignore_durable_path,
            on_summary_durable: ignore_durable_path,
            on_checkpoint_fsync: ignore_checkpoint_fsync,
        },
    )
}

/// Runs one exact manifest cell through a caller-supplied deterministic
/// evaluator seam.
///
/// This has the same cell-scoped checkpoint and no-field-summary contract as
/// [`run_cell_checkpointed_with_accelerator_config`]. It exists for
/// conformance fixtures that must observe selection, checkpoint, and emission
/// behavior without opening the production matrix sampler.
///
/// # Errors
///
/// Returns selector, checkpoint, resume, I/O, and evaluator errors under the
/// same rules as [`run_field_checkpointed_with_evaluator`].
///
/// # Complexity
///
/// Linear in the selected cell's manifested shards.
pub fn run_cell_checkpointed_with_evaluator<E>(
    root: &Path,
    manifest: &CampaignManifest,
    field: u8,
    order: u16,
    checkpoint_path: &Path,
    worker_count: usize,
    mut evaluator: E,
) -> Result<CheckpointedCampaignRun, CampaignDriverError>
where
    E: FnMut(&WorkItem) -> Result<EvaluatedShard, String>,
{
    enumerate_cell_work_items(manifest, field, order)?;
    run_scope_checkpointed_inner(
        root,
        manifest,
        CampaignExecutionScope::ExactCell { q: field, n: order },
        checkpoint_path,
        worker_count,
        |item| evaluator(item),
        DurabilityHooks {
            on_shard_durable: ignore_durable_path,
            on_summary_durable: ignore_durable_path,
            on_checkpoint_fsync: ignore_checkpoint_fsync,
        },
    )
}

/// Runs one field arm with checkpointed shard execution.
///
/// Resume uses the checkpoint's configuration identity and completed work set.
/// Completed items are loaded without re-evaluation. If a shard was durably
/// emitted before its completion checkpoint, the driver deterministically
/// re-evaluates it and adopts it only when its serialized bytes match; a
/// mismatch names the existing path and refuses to overwrite it. Checkpointed
/// shard and summary bytes are durable before their completion state is
/// persisted. An interrupt is observed at a work-item boundary after the
/// preceding checkpoint write, and a caller callback error quarantines that
/// item in the field summary while remaining work continues. Production
/// schedule errors, including manifest-selected unavailable backends, are
/// returned before they can enter quarantine. These contracts enforce
/// `@/inv/campaign-resumability` and `@/inv/deterministic-seeded-execution`.
/// `worker_count` must be non-zero; zero is refused with
/// [`CampaignDriverError::ResumeRefused`].
pub fn run_field_checkpointed(
    root: &Path,
    manifest: &CampaignManifest,
    field: u8,
    checkpoint_path: &Path,
    worker_count: usize,
) -> Result<CheckpointedCampaignRun, CampaignDriverError> {
    run_field_checkpointed_with_accelerator_config(
        root,
        manifest,
        field,
        checkpoint_path,
        worker_count,
        &AcceleratorCostTable::default(),
    )
}

/// Runs one field arm, sizing each accelerator cell's launches from its own
/// measured per-matrix cost.
///
/// The costs and launch cap are runtime inputs. They are not manifest fields
/// and must be supplied from each selected cell's committed measurement
/// receipt by a campaign caller. Every accelerator cell in the field is
/// required to have an entry before any work runs: the pre-flight refuses a
/// missing one rather than substituting a default, because a default cost
/// would size that cell's launches from a number nobody measured.
///
/// # Errors
///
/// Returns [`CampaignDriverError::Schedule`] when an accelerator cell has no
/// measured cost entry, when the manifest does not resolve for this field, or
/// when a work item cannot be executed; [`CampaignDriverError::Checkpoint`] and
/// [`CampaignDriverError::ResumeRefused`] when a present checkpoint cannot be
/// loaded or disagrees with the live campaign configuration;
/// [`CampaignDriverError::InvalidExistingShard`] when an already-emitted record
/// is not a valid shard or does not match its deterministic re-evaluation;
/// [`CampaignDriverError::Io`] for a dataset or checkpoint filesystem failure;
/// and [`CampaignDriverError::Interrupted`] when a graceful interruption was
/// requested at a work-item boundary.
///
/// # Panics
///
/// Does not intentionally panic.
///
/// # Complexity
///
/// Linear in the field's work items, each costing its cell's evaluation; one
/// checkpoint write per completed item.
pub fn run_field_checkpointed_with_accelerator_config(
    root: &Path,
    manifest: &CampaignManifest,
    field: u8,
    checkpoint_path: &Path,
    worker_count: usize,
    accelerator: &AcceleratorCostTable,
) -> Result<CheckpointedCampaignRun, CampaignDriverError> {
    accelerator
        .validate_manifest(manifest)
        .map_err(CampaignDriverError::Schedule)?;
    for item in enumerate_work_items(manifest, Some(field))? {
        if item.backend == Backend::Accelerator {
            accelerator
                .config_for(item.q, item.n)
                .map_err(CampaignDriverError::Schedule)?;
        }
    }
    run_scope_checkpointed_inner(
        root,
        manifest,
        CampaignExecutionScope::Field { q: field },
        checkpoint_path,
        worker_count,
        |item| {
            let cell = if item.backend == Backend::Accelerator {
                Some(accelerator.config_for(item.q, item.n)?)
            } else {
                None
            };
            evaluate_work_item_with_worker_count_and_accelerator(manifest, item, worker_count, cell)
        },
        DurabilityHooks {
            on_shard_durable: ignore_durable_path,
            on_summary_durable: ignore_durable_path,
            on_checkpoint_fsync: ignore_checkpoint_fsync,
        },
    )
}

/// Runs one field arm with a caller-supplied evaluator seam.
///
/// A callback error quarantines that work item and does not stop remaining
/// work. A manifest-selected unavailable backend is refused during pre-flight
/// instead of reaching the callback quarantine branch. Successful callbacks
/// must return the production [`EvaluatedShard`] result; this seam lets
/// conformance tests exercise quarantine without replacing the scheduler or
/// its sampler. The resume, interruption, adoption, durability, and quarantine
/// contracts are the same as [`run_field_checkpointed`]. `worker_count` must
/// be non-zero; zero is refused with [`CampaignDriverError::ResumeRefused`].
pub fn run_field_checkpointed_with_evaluator<E>(
    root: &Path,
    manifest: &CampaignManifest,
    field: u8,
    checkpoint_path: &Path,
    worker_count: usize,
    mut evaluator: E,
) -> Result<CheckpointedCampaignRun, CampaignDriverError>
where
    E: FnMut(&WorkItem) -> Result<EvaluatedShard, String>,
{
    run_scope_checkpointed_inner(
        root,
        manifest,
        CampaignExecutionScope::Field { q: field },
        checkpoint_path,
        worker_count,
        |item| evaluator(item),
        DurabilityHooks {
            on_shard_durable: ignore_durable_path,
            on_summary_durable: ignore_durable_path,
            on_checkpoint_fsync: ignore_checkpoint_fsync,
        },
    )
}

trait EvaluationErrorDisposition {
    fn into_driver_result(self) -> Result<String, CampaignDriverError>;
}

impl EvaluationErrorDisposition for String {
    fn into_driver_result(self) -> Result<String, CampaignDriverError> {
        Ok(self)
    }
}

impl EvaluationErrorDisposition for ScheduleError {
    fn into_driver_result(self) -> Result<String, CampaignDriverError> {
        Err(CampaignDriverError::Schedule(self))
    }
}

struct DurabilityHooks<S, M, C> {
    on_shard_durable: S,
    on_summary_durable: M,
    on_checkpoint_fsync: C,
}

fn ignore_durable_path(_: &Path) {}

fn ignore_checkpoint_fsync() {}

fn run_scope_checkpointed_inner<E, EError, S, M, C>(
    root: &Path,
    manifest: &CampaignManifest,
    scope: CampaignExecutionScope,
    checkpoint_path: &Path,
    worker_count: usize,
    mut evaluator: E,
    hooks: DurabilityHooks<S, M, C>,
) -> Result<CheckpointedCampaignRun, CampaignDriverError>
where
    E: FnMut(&WorkItem) -> Result<EvaluatedShard, EError>,
    EError: EvaluationErrorDisposition,
    S: FnMut(&Path),
    M: FnMut(&Path),
    C: FnMut(),
{
    let DurabilityHooks {
        mut on_shard_durable,
        mut on_summary_durable,
        mut on_checkpoint_fsync,
    } = hooks;
    if worker_count == 0 {
        return Err(CampaignDriverError::ResumeRefused(
            "worker_count must be non-zero".to_owned(),
        ));
    }
    let (field, items, emit_field_summary) = match scope {
        CampaignExecutionScope::Field { q } => (q, enumerate_work_items(manifest, Some(q))?, true),
        CampaignExecutionScope::ExactCell { q, n } => {
            (q, enumerate_cell_work_items(manifest, q, n)?, false)
        }
    };
    for item in &items {
        resolve_processor_path(item.q, item.n, item.backend)
            .map(|_| ())
            .map_err(CampaignDriverError::Schedule)?;
    }
    let configuration = campaign_configuration(manifest, scope);
    let hash = campaign_config_hash(manifest, scope);
    let reader =
        CheckpointReader::<CampaignCheckpoint, _>::for_payload(checkpoint_path, hash.clone());
    let writer = CheckpointWriter::<CampaignCheckpoint, _>::for_payload(checkpoint_path, hash)
        .map_err(|source| CampaignDriverError::Io {
            path: checkpoint_path.to_owned(),
            source,
        })?;
    let mut checkpoint = match reader.load_payload() {
        Ok(Some(payload)) => payload,
        Ok(None) => CampaignCheckpoint::new(configuration.clone()),
        Err(error @ CheckpointLoadError::ConfigHashMismatch { .. }) => {
            return Err(CampaignDriverError::ResumeRefused(
                configuration_disagreement(checkpoint_path, &configuration, &error.to_string()),
            ));
        }
        Err(error) => return Err(CampaignDriverError::Checkpoint(error)),
    };
    if checkpoint.configuration != configuration {
        return Err(CampaignDriverError::ResumeRefused(
            configuration_differences(&checkpoint.configuration, &configuration),
        ));
    }

    let expected: BTreeSet<_> = items.iter().map(WorkItemId::from).collect();
    let mut completed: BTreeSet<_> = checkpoint.completed.into_iter().collect();
    let mut quarantined = checkpoint.quarantined;
    let quarantined_ids: BTreeSet<_> = quarantined
        .iter()
        .map(|item| WorkItemId {
            q: item.q,
            n: item.n,
            shard_id: item.shard_id,
        })
        .collect();
    if completed
        .union(&quarantined_ids)
        .any(|id| !expected.contains(id))
    {
        return Err(CampaignDriverError::ResumeRefused(
            "checkpoint contains a work item absent from the live manifest".to_owned(),
        ));
    }
    let mut shards = Vec::new();
    for item in &items {
        let id = WorkItemId::from(item);
        if quarantined_ids.contains(&id) {
            continue;
        }
        if completed.contains(&id) {
            shards.push(load_existing_shard(root, manifest, item)?);
            continue;
        }
        if is_interrupted() {
            checkpoint.completed = completed.into_iter().collect();
            checkpoint.quarantined = quarantined.clone();
            checkpoint
                .worker_states
                .sort_by_key(|state| state.worker_index);
            persist_checkpoint(
                &writer,
                &checkpoint,
                checkpoint_path,
                &mut on_checkpoint_fsync,
            )?;
            return Err(CampaignDriverError::Interrupted);
        }

        let evaluated = match evaluator(item) {
            Ok(evaluated) => evaluated,
            Err(error) => {
                let error = error.into_driver_result()?;
                quarantined.push(QuarantinedShard {
                    q: item.q,
                    n: item.n,
                    shard_id: item.shard_id,
                    error,
                });
                checkpoint.completed = completed.iter().cloned().collect();
                checkpoint.quarantined = quarantined.clone();
                checkpoint
                    .worker_states
                    .sort_by_key(|state| state.worker_index);
                persist_checkpoint(
                    &writer,
                    &checkpoint,
                    checkpoint_path,
                    &mut on_checkpoint_fsync,
                )?;
                continue;
            }
        };
        let shard = emit_or_adopt_shard(root, manifest, item, &evaluated, &mut on_shard_durable)?;
        completed.insert(id);
        checkpoint
            .worker_states
            .retain(|state| state.worker_index != item.shard_id as usize % worker_count);
        checkpoint.worker_states.push(WorkerCheckpoint {
            worker_index: item.shard_id as usize % worker_count,
            generator_word_position: evaluated.generator_word_position,
        });
        checkpoint.completed = completed.iter().cloned().collect();
        checkpoint.quarantined = quarantined.clone();
        checkpoint
            .worker_states
            .sort_by_key(|state| state.worker_index);
        persist_checkpoint(
            &writer,
            &checkpoint,
            checkpoint_path,
            &mut on_checkpoint_fsync,
        )?;
        shards.push(shard);
    }

    // Records emitted before the checkpoint boundary are authoritative. This
    // reload also handles a crash after the shard rename but before the flush.
    for item in &items {
        let id = WorkItemId::from(item);
        if completed.contains(&id)
            && !shards.iter().any(|shard| {
                shard.record.shard_id == item.shard_id
                    && shard.record.stream_address.n == item.n
                    && shard.record.stream_address.q == item.q
            })
        {
            shards.push(load_existing_shard(root, manifest, item)?);
        }
    }
    shards.sort_by_key(|shard| {
        (
            shard.record.stream_address.q,
            shard.record.stream_address.n,
            shard.record.shard_id,
        )
    });
    let mut summary = summarize_with_quarantine(manifest, field, &shards, quarantined.clone())?;
    if let CampaignExecutionScope::ExactCell { n: order, .. } = scope {
        summary.rows.retain(|row| row.n == order);
        summary.quarantined.retain(|item| item.n == order);
    }
    if emit_field_summary {
        let summary_path = root.join(field_summary_file(field));
        if summary_path.is_file() {
            let _ = crate::permanent_campaign::schema::read_field_summary(root, field)
                .map_err(|error| CampaignDriverError::ResumeRefused(error.to_string()))?;
        } else {
            emit_summary_with_durability_hook(root, manifest, &summary, |path| {
                on_summary_durable(path)
            })?;
        }
    }
    checkpoint.completed = completed.into_iter().collect();
    checkpoint.quarantined = quarantined;
    checkpoint.scope_complete = true;
    persist_checkpoint(
        &writer,
        &checkpoint,
        checkpoint_path,
        &mut on_checkpoint_fsync,
    )?;
    Ok(CheckpointedCampaignRun {
        scope,
        shards,
        summary,
    })
}

fn persist_checkpoint<C>(
    writer: &CheckpointWriter<CampaignCheckpoint, String>,
    checkpoint: &CampaignCheckpoint,
    checkpoint_path: &Path,
    on_checkpoint_fsync: &mut C,
) -> Result<(), CampaignDriverError>
where
    C: FnMut(),
{
    writer
        .write_payload_with_fsync_hook(checkpoint, on_checkpoint_fsync)
        .map_err(|source| CampaignDriverError::Io {
            path: checkpoint_path.to_owned(),
            source,
        })
}

fn emit_or_adopt_shard<S>(
    root: &Path,
    manifest: &CampaignManifest,
    item: &WorkItem,
    evaluated: &EvaluatedShard,
    on_shard_durable: &mut S,
) -> Result<ShardRun, CampaignDriverError>
where
    S: FnMut(&Path),
{
    let path = root.join(shard_record_file(item.q, item.n, item.shard_id));
    let expected = shard_record_bytes(&evaluated.run.record)?;
    match fs::read(&path) {
        Ok(existing) => {
            if existing != expected {
                return Err(CampaignDriverError::InvalidExistingShard {
                    path,
                    message: "serialized shard bytes mismatch (corruption or foreign file)"
                        .to_owned(),
                });
            }
            load_existing_shard(root, manifest, item)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            emit_shard_with_durability_hook(root, manifest, &evaluated.run, |durable_path| {
                on_shard_durable(durable_path)
            })?;
            Ok(evaluated.run.clone())
        }
        Err(source) => Err(CampaignDriverError::Io { path, source }),
    }
}

fn load_existing_shard(
    root: &Path,
    manifest: &CampaignManifest,
    item: &WorkItem,
) -> Result<ShardRun, CampaignDriverError> {
    let path = root.join(shard_record_file(item.q, item.n, item.shard_id));
    let bytes = fs::read(&path).map_err(|source| CampaignDriverError::Io {
        path: path.clone(),
        source,
    })?;
    let record: ShardRecord = serde_json::from_slice(&bytes).map_err(|error| {
        CampaignDriverError::InvalidExistingShard {
            path: path.clone(),
            message: error.to_string(),
        }
    })?;
    let expected = enumerate_work_items(manifest, Some(item.q))?
        .into_iter()
        .find(|candidate| candidate.shard_id == item.shard_id && candidate.n == item.n)
        .expect("item came from the same manifest enumeration");
    if record.shard_id != expected.shard_id
        || record.stream_address.q != expected.q
        || record.stream_address.n != expected.n
        || record.matrix_count != expected.matrix_count
    {
        return Err(CampaignDriverError::InvalidExistingShard {
            path,
            message: "record identity or matrix count differs from manifest".to_owned(),
        });
    }
    Ok(ShardRun {
        record,
        timing: PhaseDurations {
            draw: std::time::Duration::ZERO,
            pack: std::time::Duration::ZERO,
            evaluate: std::time::Duration::ZERO,
            determinant: std::time::Duration::ZERO,
            count: std::time::Duration::ZERO,
        },
    })
}

fn configuration_disagreement(
    checkpoint_path: &Path,
    current: &CampaignConfiguration,
    fallback: &str,
) -> String {
    let loaded = fs::read(checkpoint_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|envelope| envelope.get("payload").cloned())
        .and_then(|payload| serde_json::from_value::<CampaignCheckpoint>(payload).ok())
        .map(|checkpoint| checkpoint.configuration);
    loaded
        .map(|loaded| configuration_differences(&loaded, current))
        .unwrap_or_else(|| {
            format!("manifest configuration differs from the checkpoint: {fallback}")
        })
}

fn configuration_differences(
    checkpoint: &CampaignConfiguration,
    current: &CampaignConfiguration,
) -> String {
    let mut differences = Vec::new();
    if checkpoint.manifest_content_hash != current.manifest_content_hash {
        differences.push(format!(
            "manifest_content_hash differs (checkpointed={}, current={})",
            checkpoint.manifest_content_hash, current.manifest_content_hash
        ));
    }
    if checkpoint.campaign_id != current.campaign_id {
        differences.push(format!(
            "campaign_id differs (checkpointed={}, current={})",
            checkpoint.campaign_id, current.campaign_id
        ));
    }
    if checkpoint.scope != current.scope {
        differences.push(format!(
            "scope differs (checkpointed={}, current={})",
            checkpoint.scope.identity(),
            current.scope.identity()
        ));
    }
    if checkpoint.root_seed != current.root_seed {
        differences.push(format!(
            "root_seed differs (checkpointed={}, current={})",
            checkpoint.root_seed, current.root_seed
        ));
    }
    if differences.is_empty() {
        "manifest configuration differs from the checkpoint".to_owned()
    } else {
        differences.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permanent_campaign::fixture::{manifest, TestDir};
    use crate::permanent_campaign::provenance::recorded_manifest_hash;
    use crate::permanent_campaign::schedule::evaluate_work_item;
    use crate::permanent_campaign::schema::{
        field_summary_file, read_manifest, DeterminantCount, ShardRecord, StreamAddress,
        MANIFEST_FILE, SCHEMA_VERSION,
    };
    use sha2::{Digest, Sha256};
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;

    fn fixture_evaluation(manifest: &CampaignManifest, item: &WorkItem) -> EvaluatedShard {
        let mut histogram = vec![0; usize::from(item.q)];
        histogram[0] = item.matrix_count;
        EvaluatedShard {
            run: ShardRun {
                record: ShardRecord {
                    schema_version: SCHEMA_VERSION,
                    shard_id: item.shard_id,
                    stream_address: StreamAddress {
                        root_seed: manifest.root_seed,
                        q: item.q,
                        n: item.n,
                        purpose_tag: 3,
                        stream_index: item.stream_index,
                    },
                    matrix_count: item.matrix_count,
                    permanent_zero_count: item.matrix_count,
                    permanent_histogram: histogram,
                    determinant: DeterminantCount::NotEvaluated,
                },
                timing: PhaseDurations {
                    draw: std::time::Duration::ZERO,
                    pack: std::time::Duration::ZERO,
                    evaluate: std::time::Duration::ZERO,
                    determinant: std::time::Duration::ZERO,
                    count: std::time::Duration::ZERO,
                },
            },
            generator_word_position: u128::from(item.stream_index),
        }
    }

    #[test]
    fn frozen_manifest_checkpoint_identity_matches_integrity_sidecar() {
        let campaign = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829",
        );
        let manifest_bytes =
            fs::read(campaign.join(MANIFEST_FILE)).expect("frozen manifest must be readable");
        let manifest = read_manifest(&campaign).expect("frozen manifest must pass its schema");
        let reserialized =
            serde_json::to_vec(&manifest).expect("campaign manifest is serializable");
        let blake3_digest = blake3::hash(&reserialized).to_hex().to_string();

        let scope = CampaignExecutionScope::Field { q: 3 };
        let checkpoint = CampaignCheckpoint::new(campaign_configuration(&manifest, scope));
        assert_eq!(
            checkpoint.configuration.manifest_content_hash, blake3_digest,
            "checkpoint identity must use BLAKE3 of the driver's re-serialization"
        );
        assert_eq!(
            campaign_config_hash(&manifest, scope),
            format!(
                "blake3:{blake3_digest};campaign_id={};scope=field:q=3;root_seed={};manifest={blake3_digest}",
                manifest.campaign_id, manifest.root_seed
            )
        );

        let on_disk_sha256 = Sha256::digest(&manifest_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let sidecar_sha256 = recorded_manifest_hash(&campaign)
            .expect("checksums.sha256 must cover the frozen manifest");
        assert_eq!(
            on_disk_sha256,
            sidecar_sha256.as_str(),
            "sidecar identity must be SHA-256 of the exact manifest bytes"
        );
        assert_ne!(
            blake3_digest,
            sidecar_sha256.as_str(),
            "the execution and on-disk identities use distinct algorithms"
        );
    }

    #[test]
    fn exact_cell_checkpoint_is_scoped_and_does_not_finalize_the_field() {
        let directory = TestDir::new();
        let mut campaign = manifest();
        let mut second = campaign.cells[0].clone();
        second.n = 5;
        for shard in &mut second.shards {
            shard.stream_index += 100;
        }
        campaign.cells.push(second);
        let checkpoint = cell_checkpoint_path(directory.root(), 3, 5);

        let run = run_cell_checkpointed_with_evaluator(
            directory.root(),
            &campaign,
            3,
            5,
            &checkpoint,
            1,
            |item| Ok(fixture_evaluation(&campaign, item)),
        )
        .unwrap();

        assert!(checkpoint.is_file());
        assert_eq!(run.summary().rows.len(), 1);
        assert_eq!(run.summary().rows[0].n, 5);
        assert!(
            !directory.root().join(field_summary_file(3)).exists(),
            "one exact terminal cell cannot finalize a partial field summary"
        );

        let error = run_cell_checkpointed_with_evaluator(
            directory.root(),
            &campaign,
            3,
            4,
            &checkpoint,
            1,
            |item| Ok(fixture_evaluation(&campaign, item)),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains(
                "scope differs (checkpointed=exact_cell:q=3,n=5, current=exact_cell:q=3,n=4)"
            ),
            "cross-cell resume must name the semantic mismatch: {error}"
        );
    }

    #[test]
    fn exact_cell_driver_requires_costs_for_every_manifest_accelerator_cell() {
        let directory = TestDir::new();
        let mut campaign = manifest();
        campaign.cells[0].backend = Backend::Accelerator;
        let mut second = campaign.cells[0].clone();
        second.n = 5;
        for shard in &mut second.shards {
            shard.stream_index += 100;
        }
        campaign.cells.push(second);
        let mut costs = std::collections::BTreeMap::new();
        costs.insert((3, 4), std::time::Duration::from_micros(10));
        let costs = AcceleratorCostTable::new(costs, std::time::Duration::from_millis(500));
        let checkpoint = cell_checkpoint_path(directory.root(), 3, 4);

        let error = run_cell_checkpointed_with_accelerator_config(
            directory.root(),
            &campaign,
            3,
            4,
            &checkpoint,
            1,
            &costs,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CampaignDriverError::Schedule(ScheduleError::AcceleratorCostMissing { q: 3, n: 5 })
        ));
        assert!(
            !checkpoint.exists(),
            "manifest-wide cost validation must precede checkpoint creation"
        );
    }

    #[test]
    fn shard_and_summary_durability_precede_checkpoint_recording() {
        let directory = TestDir::new();
        let campaign = manifest();
        let checkpoint = field_checkpoint_path(directory.root(), 3);
        let events = Rc::new(RefCell::new(Vec::new()));
        let shard_events = Rc::clone(&events);
        let summary_events = Rc::clone(&events);
        let checkpoint_events = Rc::clone(&events);

        run_scope_checkpointed_inner(
            directory.root(),
            &campaign,
            CampaignExecutionScope::Field { q: 3 },
            &checkpoint,
            1,
            |item| evaluate_work_item(&campaign, item),
            DurabilityHooks {
                on_shard_durable: move |path: &Path| {
                    shard_events
                        .borrow_mut()
                        .push(format!("shard:{}", path.display()))
                },
                on_summary_durable: move |path: &Path| {
                    summary_events
                        .borrow_mut()
                        .push(format!("summary:{}", path.display()))
                },
                on_checkpoint_fsync: move || {
                    checkpoint_events.borrow_mut().push("checkpoint".to_owned())
                },
            },
        )
        .unwrap();

        let events = events.borrow();
        let first_shard = events
            .iter()
            .position(|event| event.starts_with("shard:"))
            .unwrap();
        let first_checkpoint = events
            .iter()
            .position(|event| event == "checkpoint")
            .unwrap();
        let summary = events
            .iter()
            .position(|event| event.starts_with("summary:"))
            .unwrap();
        let final_checkpoint = events
            .iter()
            .rposition(|event| event == "checkpoint")
            .unwrap();
        assert!(first_shard < first_checkpoint, "events: {events:?}");
        assert!(summary < final_checkpoint, "events: {events:?}");
    }
}
