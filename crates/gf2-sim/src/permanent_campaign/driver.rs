//! Checkpointed permanent-campaign execution.
//!
//! The driver commits one shard record before it commits the corresponding
//! checkpoint state. On resume it also adopts an already-emitted shard, which
//! closes the crash window between those two durable writes without redoing
//! completed work. The configuration identity covers the complete manifest
//! and selected field, while output paths and worker count remain outside the
//! identity so a later processor can change its execution width safely.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::checkpoint::{
    CheckpointLoadError, CheckpointPayload, CheckpointReader, CheckpointWriter,
};
use crate::permanent_campaign::schedule::{
    emit_shard, emit_summary, enumerate_work_items, evaluate_work_item, summarize_with_quarantine,
    EvaluatedShard, FieldRun, PhaseDurations, ShardRun, WorkItem,
};
use crate::permanent_campaign::schema::{
    field_summary_file, shard_record_file, CampaignManifest, QuarantinedShard, ShardRecord,
};
use crate::snr_checkpoint::is_interrupted;

const CHECKPOINT_SCHEMA_VERSION: u32 = 1;

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
    /// Field arm represented by this payload.
    pub field: u8,
    /// Work items whose records are durably emitted.
    pub completed: Vec<WorkItemId>,
    /// Work items retained as evaluation failures.
    pub quarantined: Vec<QuarantinedShard>,
    /// Absolute sampler positions observed at worker boundaries.
    pub worker_states: Vec<WorkerCheckpoint>,
    /// Whether the field summary is durably emitted.
    pub field_complete: bool,
}

impl CampaignCheckpoint {
    /// Creates an empty checkpoint for one field arm.
    #[must_use]
    pub fn new(field: u8) -> Self {
        Self {
            field,
            completed: Vec::new(),
            quarantined: Vec::new(),
            worker_states: Vec::new(),
            field_complete: false,
        }
    }
}

impl CheckpointPayload for CampaignCheckpoint {
    const IDENTITY: &'static str = "permanent-campaign/field-progress";
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
    /// The caller requested a graceful interruption after the durable flush.
    Interrupted,
    /// A present checkpoint disagrees with the live campaign configuration.
    ResumeRefused(String),
    /// A previously emitted record is not a valid shard document.
    InvalidExistingShard {
        /// Existing shard path.
        path: PathBuf,
        /// Validation diagnostic.
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

/// Returns the stable configuration identity for a field checkpoint.
///
/// The digest covers the complete serialized manifest. The named components
/// are retained in the identity so a refusal can identify common disagreements
/// instead of exposing only an opaque digest.
#[must_use]
pub fn campaign_config_hash(manifest: &CampaignManifest, field: u8) -> String {
    let bytes = serde_json::to_vec(manifest).expect("campaign manifest is serializable");
    let digest = blake3::hash(&bytes).to_hex();
    format!(
        "blake3:{digest};campaign_id={};field={field};root_seed={};manifest={digest}",
        manifest.campaign_id, manifest.root_seed
    )
}

/// Runs one field arm with checkpointed shard execution.
pub fn run_field_checkpointed(
    root: &Path,
    manifest: &CampaignManifest,
    field: u8,
    checkpoint_path: &Path,
    worker_count: usize,
) -> Result<FieldRun, CampaignDriverError> {
    run_field_checkpointed_with_evaluator(
        root,
        manifest,
        field,
        checkpoint_path,
        worker_count,
        |item| evaluate_work_item(manifest, item).map_err(|error| error.to_string()),
    )
}

/// Runs one field arm with a caller-supplied evaluator seam.
///
/// A callback error quarantines that work item and does not stop remaining
/// work. Successful callbacks must return the production [`EvaluatedShard`]
/// result; this seam lets conformance tests exercise quarantine without
/// replacing the scheduler or its sampler.
pub fn run_field_checkpointed_with_evaluator<E>(
    root: &Path,
    manifest: &CampaignManifest,
    field: u8,
    checkpoint_path: &Path,
    worker_count: usize,
    mut evaluator: E,
) -> Result<FieldRun, CampaignDriverError>
where
    E: FnMut(&WorkItem) -> Result<EvaluatedShard, String>,
{
    if worker_count == 0 {
        return Err(CampaignDriverError::ResumeRefused(
            "worker_count must be non-zero".to_owned(),
        ));
    }
    let items = enumerate_work_items(manifest, Some(field))?;
    let hash = campaign_config_hash(manifest, field);
    let reader =
        CheckpointReader::<CampaignCheckpoint, _>::for_payload(checkpoint_path, hash.clone());
    let writer = CheckpointWriter::<CampaignCheckpoint, _>::for_payload(checkpoint_path, hash)
        .map_err(|source| CampaignDriverError::Io {
            path: checkpoint_path.to_owned(),
            source,
        })?;
    let mut checkpoint = match reader.load_payload() {
        Ok(Some(payload)) => payload,
        Ok(None) => CampaignCheckpoint::new(field),
        Err(error @ CheckpointLoadError::ConfigHashMismatch { .. }) => {
            return Err(CampaignDriverError::ResumeRefused(
                configuration_disagreement(&error.to_string()),
            ));
        }
        Err(error) => return Err(CampaignDriverError::Checkpoint(error)),
    };
    if checkpoint.field != field {
        return Err(CampaignDriverError::ResumeRefused(format!(
            "field differs: checkpoint has {}, live configuration has {field}",
            checkpoint.field
        )));
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
            writer
                .write_payload(&checkpoint)
                .map_err(|source| CampaignDriverError::Io {
                    path: checkpoint_path.to_owned(),
                    source,
                })?;
            return Err(CampaignDriverError::Interrupted);
        }

        let evaluated = match evaluator(item) {
            Ok(evaluated) => evaluated,
            Err(error) => {
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
                writer
                    .write_payload(&checkpoint)
                    .map_err(|source| CampaignDriverError::Io {
                        path: checkpoint_path.to_owned(),
                        source,
                    })?;
                continue;
            }
        };
        emit_shard(root, manifest, &evaluated.run)?;
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
        writer
            .write_payload(&checkpoint)
            .map_err(|source| CampaignDriverError::Io {
                path: checkpoint_path.to_owned(),
                source,
            })?;
        shards.push(evaluated.run);
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
    let summary = summarize_with_quarantine(manifest, field, &shards, quarantined.clone());
    let summary_path = root.join(field_summary_file(field));
    if summary_path.is_file() {
        let _ = crate::permanent_campaign::schema::read_field_summary(root, field)
            .map_err(|error| CampaignDriverError::ResumeRefused(error.to_string()))?;
    } else {
        emit_summary(root, manifest, &summary)?;
    }
    checkpoint.completed = completed.into_iter().collect();
    checkpoint.quarantined = quarantined;
    checkpoint.field_complete = true;
    writer
        .write_payload(&checkpoint)
        .map_err(|source| CampaignDriverError::Io {
            path: checkpoint_path.to_owned(),
            source,
        })?;
    Ok(FieldRun::from_parts(field, shards, summary))
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
            count: std::time::Duration::ZERO,
        },
    })
}

fn configuration_disagreement(error: &str) -> String {
    let detail = error
        .split("loaded ")
        .nth(1)
        .and_then(|value| value.split(", expected").next())
        .unwrap_or(error);
    if detail.contains("root_seed=") {
        "root_seed differs between checkpoint and live campaign".to_owned()
    } else if detail.contains("field=") {
        "field differs between checkpoint and live campaign".to_owned()
    } else if detail.contains("campaign_id=") {
        "campaign_id differs between checkpoint and live campaign".to_owned()
    } else {
        "campaign manifest configuration differs from the checkpoint".to_owned()
    }
}
