//! Deterministic scheduling and field-shard emission for permanent campaigns.
//!
//! A campaign invocation owns one field arm. Work is ordered by `(q, n,
//! shard_id)`, each shard opens the stream address recorded by its manifest,
//! and every matrix passes through draw, pack, evaluate, determinant, and count
//! phases when the cell requests the determinant companion. Generic Ryser
//! cells omit the pack phase because their row-major operands are evaluated
//! directly.
//! Timings remain in [`ShardRun`] for progress reporting; only schema records
//! and summaries are written to disk, so wall-clock variation cannot alter
//! emitted bytes.
//!
//! A `BatchParallel` cell draws each shard's matrices serially in bounded
//! chunks, then uses a locally configured Rayon pool for packing, permanent
//! evaluation, and optional determinant evaluation. Batch phase durations are
//! wall-clock durations for those per-chunk pool sections; the observer and
//! histogram updates retain input order on the caller thread.
//!
//! ```no_run
//! # use std::path::Path;
//! # use gf2_sim::permanent_campaign::schema::read_manifest;
//! # use gf2_sim::permanent_campaign::schedule::{emit_field, enumerate_work_items, run_field};
//! let root = Path::new("dev/simulation_results/permanent-zero-fraction/campaign");
//! let manifest = read_manifest(root).unwrap();
//! let work = enumerate_work_items(&manifest, Some(3)).unwrap();
//! let result = run_field(&manifest, 3).unwrap();
//! let written = emit_field(root, &manifest, &result).unwrap();
//! assert_eq!(written.len(), work.len() + 1);
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
use gf2_algebra::permanent::bipedal5::permanent_bipedal5_singleword;
use gf2_algebra::permanent::bipedal7::permanent_bipedal7_singleword;
use gf2_algebra::permanent::{
    permanent_bipedal3_parallel, permanent_bipedal3_singleword, permanent_ryser,
};
use gf2_core::field::{matrix::FieldMatrix, FieldVec};
use gf2_core::gfp::Fp;
use gf2_stats::binomial::{bonferroni_level, permanent_zero_floor_test, two_sided_test};
use gf2_stats::sampler::{
    FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose as SamplerPurpose,
};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

use super::schema::{
    field_summary_file, shard_record_file, AcceptanceVerdict, Backend, CampaignManifest, CellSpec,
    CellTerminalState, DeterminantCount, DeterminantEstimate, DeterminantPlan, FieldSummary,
    Interval, ProportionEstimate, QuarantinedShard, ShardRecord, ShardSpec, StreamAddress,
    SummaryRow, SCHEMA_VERSION,
};

/// The purpose tag reserved for published campaign-cell matrix streams.
pub const CAMPAIGN_CELL_PURPOSE_TAG: u8 = SamplerPurpose::CampaignCell as u8;

/// Family-wise error budget for the permanent-floor tests, as preregistered
/// in `dev/simulation_results/permanent-zero-fraction/protocol.md` under
/// "Error budgets" and "Permanent-floor decision".
const PERMANENT_FAMILYWISE_ERROR: f64 = 0.025;

/// Family-wise error budget for the determinant tests, as preregistered in
/// `dev/simulation_results/permanent-zero-fraction/protocol.md` under
/// "Error budgets" and "Determinant decision".
const DETERMINANT_FAMILYWISE_ERROR: f64 = 0.025;

/// Maximum number of field entries retained by one batch's raw matrices.
const BATCH_CHUNK_MAX_MATRIX_ENTRIES: usize = 16 * 1024;
/// Maximum number of matrices evaluated by one batch for small matrices.
const BATCH_CHUNK_MAX_MATRICES: usize = 64;

/// Default wall-clock cap for one accelerator launch.
pub const DEFAULT_ACCELERATOR_LAUNCH_CAP: Duration = Duration::from_millis(500);

/// Configuration used to size accelerator launches.
///
/// `per_matrix_cost` is measured configuration supplied by the caller; it is
/// not part of the frozen campaign manifest. The campaign binary's value must
/// be derived from the committed measurement receipt for the selected cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceleratorConfig {
    /// Measured wall-clock cost of one matrix on the selected accelerator.
    pub per_matrix_cost: Duration,
    /// Maximum target wall-clock duration for one accelerator launch.
    pub launch_cap: Duration,
}

/// Per-cell measured accelerator costs, keyed by the frozen `(q, n)` cell.
///
/// REQ-01 sizes launches from each cell's *measured* per-matrix cost. Permanent
/// evaluation costs about `M · n · 2^n / W`, so one cost applied across a field
/// reproduces, one level up, the fixed-batch-size error the criterion rules
/// out: the configured cap would hold at the size the value was measured at and
/// nowhere else. Every accelerator cell therefore carries its own entry, and a
/// cell with no entry is refused rather than defaulted — a default would be an
/// unmeasured cost wearing a measured cost's clothes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorCostTable {
    costs: BTreeMap<(u8, u16), Duration>,
    launch_cap: Duration,
}

impl Default for AcceleratorCostTable {
    /// An empty table at the default launch cap.
    ///
    /// Every accelerator cell resolves to
    /// [`ScheduleError::AcceleratorCostMissing`] against it, which is what a
    /// caller that supplied no measured costs should get.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(1)` time and space.
    fn default() -> Self {
        Self {
            costs: BTreeMap::new(),
            launch_cap: DEFAULT_ACCELERATOR_LAUNCH_CAP,
        }
    }
}

impl AcceleratorCostTable {
    /// Builds a table from measured `(q, n) -> per-matrix cost` entries.
    ///
    /// The map is adopted without iteration or validation. In particular, a
    /// zero cost remains valid and has the unbounded-rate meaning documented by
    /// [`launch_size`].
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(1)`; ownership of the existing map is moved into the table.
    #[must_use]
    pub fn new(costs: BTreeMap<(u8, u16), Duration>, launch_cap: Duration) -> Self {
        Self { costs, launch_cap }
    }

    /// Resolves the launch-sizing configuration for one cell.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::AcceleratorCostMissing`] when the cell has no
    /// measured entry.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(log C)` for `C` measured cells.
    pub fn config_for(&self, q: u8, n: u16) -> Result<AcceleratorConfig, ScheduleError> {
        let per_matrix_cost = self
            .costs
            .get(&(q, n))
            .copied()
            .ok_or(ScheduleError::AcceleratorCostMissing { q, n })?;
        Ok(AcceleratorConfig {
            per_matrix_cost,
            launch_cap: self.launch_cap,
        })
    }

    /// Whether the table has a measured entry for one cell.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(log C)` for `C` measured cells.
    #[must_use]
    pub fn contains(&self, q: u8, n: u16) -> bool {
        self.costs.contains_key(&(q, n))
    }
}

/// Chooses the number of matrices for the next accelerator launch.
///
/// The rule is `clamp(floor(cap / per_matrix), 1, remaining)`. Permanent
/// evaluation costs approximately `M · n · 2^n / W`, so holding `M` fixed
/// while `n` rises from 20 to 24 multiplies per-launch occupancy by about
/// 19.2; each further four-order increase costs about another factor of 19.
/// The measured per-matrix input therefore sizes each successive launch rather
/// than reusing one batch size across the grid. A zero measured cost is treated
/// as an unbounded rate and selects all remaining matrices.
///
/// # Panics
///
/// Does not panic, including for zero durations or zero remaining matrices.
///
/// # Complexity
///
/// `O(1)` time and space.
#[must_use]
pub fn launch_size(per_matrix: Duration, cap: Duration, remaining: u64) -> usize {
    if remaining == 0 {
        return 1;
    }
    let remaining_as_usize = remaining.min(usize::MAX as u64) as usize;
    if per_matrix.is_zero() {
        return remaining_as_usize.max(1);
    }
    let quotient = cap.as_nanos() / per_matrix.as_nanos();
    let requested = quotient.min(usize::MAX as u128) as usize;
    requested.clamp(1, remaining_as_usize.max(1))
}

/// One manifest shard expanded into executable scheduling data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkItem {
    /// Prime field order.
    pub q: u8,
    /// Square matrix dimension.
    pub n: u16,
    /// Stable cell-local shard identity.
    pub shard_id: u64,
    /// Stream index assigned by the manifest.
    pub stream_index: u64,
    /// Number of matrices this shard evaluates.
    pub matrix_count: u64,
    /// Frozen backend identity copied from the cell.
    pub backend: Backend,
    /// Whether the manifest requests a determinant companion.
    pub determinant_companion: DeterminantPlan,
}

/// Concrete processor kernel selected for one frozen campaign cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessorPath {
    /// F_3 single-word scalar kernel.
    Bipedal3SingleWord,
    /// F_5 single-word scalar kernel.
    Bipedal5SingleWord,
    /// F_7 single-word scalar kernel.
    Bipedal7SingleWord,
    /// F_3 intra-matrix Rayon kernel.
    Bipedal3IntraMatrixParallel,
    /// Generic finite-field Ryser kernel.
    GenericRyser,
    /// HIP permanent batch entry point selected by the frozen manifest.
    #[cfg(feature = "hip")]
    Accelerator,
}

impl WorkItem {
    /// Returns the deterministic ordering key `(q, n, shard_id)`.
    #[must_use]
    pub const fn key(&self) -> (u8, u16, u64) {
        (self.q, self.n, self.shard_id)
    }
}

/// Wall-clock durations for one shard's observable execution phases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseDurations {
    /// Time spent drawing row-major field entries.
    pub draw: Duration,
    /// Time spent constructing the packed representation: each batch's
    /// wall-clock packing section when `BatchParallel` is selected, and each
    /// launch's packing span when the accelerator is selected.
    /// This is zero for a `GenericRyser` cell, which does not construct a
    /// packed representation.
    pub pack: Duration,
    /// Time spent evaluating permanents, as serial per-matrix time or the
    /// wall-clock duration of each batch's parallel permanent section.
    pub evaluate: Duration,
    /// Time spent evaluating determinants when the companion is enabled; for
    /// batches this is the wall-clock duration of each parallel determinant
    /// section.
    pub determinant: Duration,
    /// Time spent updating the residue histogram and zero count.
    pub count: Duration,
}

/// One deterministic shard record together with non-persisted timing data.
#[derive(Clone, Debug, PartialEq)]
pub struct ShardRun {
    /// Schema record written for this shard.
    pub record: ShardRecord,
    /// Wall-clock observations for this shard.
    pub timing: PhaseDurations,
}

/// A deterministic shard result and the sampler's absolute generator position
/// after its final matrix draw.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedShard {
    /// Schema record produced by the evaluation.
    pub run: ShardRun,
    /// Absolute ChaCha20 generator word position observed at the checkpoint
    /// boundary. The driver records this caller-owned continuation state.
    pub generator_word_position: u128,
}

/// Completed execution for one field arm.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldRun {
    q: u8,
    shards: Vec<ShardRun>,
    summary: FieldSummary,
}

impl FieldRun {
    pub(crate) fn from_parts(q: u8, shards: Vec<ShardRun>, summary: FieldSummary) -> Self {
        Self { q, shards, summary }
    }

    /// Returns the field order covered by this invocation.
    #[must_use]
    pub const fn q(&self) -> u8 {
        self.q
    }

    /// Returns shard results in deterministic `(q, n, shard_id)` order.
    #[must_use]
    pub fn shards(&self) -> &[ShardRun] {
        &self.shards
    }

    /// Returns the schema summary whose rows pool this field's shard records.
    #[must_use]
    pub const fn summary(&self) -> &FieldSummary {
        &self.summary
    }
}

/// Failure raised while enumerating, executing, or emitting a campaign arm.
#[derive(Debug)]
pub enum ScheduleError {
    /// The requested field is not present in the manifest.
    FieldNotFound {
        /// Requested prime field order.
        q: u8,
    },
    /// The manifest has no campaign-cell stream purpose with the required tag.
    MissingCampaignPurpose,
    /// A manifest value cannot be represented by the execution API.
    InvalidWorkItem(String),
    /// The frozen cell backend has no implementation in this build.
    BackendUnavailable {
        /// Prime field order of the cell.
        q: u8,
        /// Square matrix dimension of the cell.
        n: u16,
        /// Backend named by the cell.
        backend: Backend,
    },
    /// An accelerator cell has no measured per-matrix cost entry.
    AcceleratorCostMissing {
        /// Prime field order of the cell.
        q: u8,
        /// Square matrix dimension of the cell.
        n: u16,
    },
    /// The manifest selected an accelerator, but this host has no usable one.
    AcceleratorDeviceUnavailable {
        /// Prime field order of the cell.
        q: u8,
        /// Square matrix dimension of the cell.
        n: u16,
        /// Device required by the selected backend.
        device: &'static str,
    },
    /// A filesystem or serialization operation failed.
    Io {
        /// Path involved in the filesystem operation.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// JSON serialization failed.
    Serialization(serde_json::Error),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldNotFound { q } => write!(formatter, "manifest has no field q={q}"),
            Self::MissingCampaignPurpose => write!(
                formatter,
                "manifest stream_purposes has no campaign-cell purpose tag {CAMPAIGN_CELL_PURPOSE_TAG}"
            ),
            Self::InvalidWorkItem(message) => formatter.write_str(message),
            Self::BackendUnavailable { q, n, backend } => write!(
                formatter,
                "cell q={q} n={n} names backend {}, which this build does not provide",
                backend.name()
            ),
            Self::AcceleratorCostMissing { q, n } => write!(
                formatter,
                "accelerator cell q={q} n={n} has no measured per-matrix cost entry; \
                 supply one from that cell's committed measurement receipt"
            ),
            Self::AcceleratorDeviceUnavailable { q, n, device } => write!(
                formatter,
                "cell q={q} n={n} requires {device}, but no usable device is present"
            ),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Serialization(source) => source.fmt(formatter),
        }
    }
}

fn backend_unavailable(q: u8, n: u16, backend: Backend) -> Result<ProcessorPath, ScheduleError> {
    Err(ScheduleError::BackendUnavailable { q, n, backend })
}

/// Resolves a frozen cell triple to the concrete processor kernel it names.
///
/// This is a pure function of the manifest's `(q, n, backend)` values. It
/// does not inspect host capabilities, timing, sampled matrices, or any
/// measurement result. Matrix distribution for `BatchParallel` remains a
/// separate scheduling concern.
pub(crate) fn resolve_processor_path(
    q: u8,
    n: u16,
    backend: Backend,
) -> Result<ProcessorPath, ScheduleError> {
    match backend {
        Backend::Scalar | Backend::BatchParallel => match q {
            3 if n <= 63 => Ok(ProcessorPath::Bipedal3SingleWord),
            5 if n <= 63 => Ok(ProcessorPath::Bipedal5SingleWord),
            7 if n <= 16 => Ok(ProcessorPath::Bipedal7SingleWord),
            _ => backend_unavailable(q, n, backend),
        },
        Backend::IntraMatrixParallel => match q {
            3 if n <= 63 => Ok(ProcessorPath::Bipedal3IntraMatrixParallel),
            _ => backend_unavailable(q, n, backend),
        },
        Backend::GenericRyser if n <= 63 => Ok(ProcessorPath::GenericRyser),
        Backend::Accelerator => {
            #[cfg(feature = "hip")]
            {
                match q {
                    3 | 5 | 7 if n <= 63 => Ok(ProcessorPath::Accelerator),
                    _ => backend_unavailable(q, n, backend),
                }
            }
            #[cfg(not(feature = "hip"))]
            {
                backend_unavailable(q, n, backend)
            }
        }
        Backend::GenericRyser => backend_unavailable(q, n, backend),
    }
}

impl std::error::Error for ScheduleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Serialization(source) => Some(source),
            _ => None,
        }
    }
}

/// Enumerates manifest shards in deterministic `(q, n, shard_id)` order.
///
/// When `field` is `Some(q)`, only that field arm is returned. The function
/// performs no I/O and does not change manifest order in place. Complexity is
/// `O(S log S)` for `S` selected shards.
pub fn enumerate_work_items(
    manifest: &CampaignManifest,
    field: Option<u8>,
) -> Result<Vec<WorkItem>, ScheduleError> {
    let mut items = Vec::new();
    for cell in &manifest.cells {
        if field.is_some_and(|wanted| wanted != cell.q) {
            continue;
        }
        for shard in &cell.shards {
            items.push(work_item(cell, shard)?);
        }
    }
    if let Some(q) = field {
        if items.is_empty() {
            return Err(ScheduleError::FieldNotFound { q });
        }
    }
    items.sort_by_key(WorkItem::key);
    Ok(items)
}

/// Executes every shard for one field and builds its schema summary.
///
/// The selected field is the only execution scope. Matrix generation uses the
/// production ChaCha20 rejection sampler, and permanent evaluation dispatches
/// to the backend named by each cell. The result is in memory; use
/// [`emit_field`] to write only its shard paths and field summary. For `M`
/// matrices of dimension `n`, the evaluation cost is the selected algebra
/// kernel plus an optional `O(n³)` determinant per matrix; sampler and packing
/// storage remain `O(n²)`.
///
/// Accelerator cells are refused with
/// [`ScheduleError::AcceleratorCostMissing`] because this convenience entry
/// point supplies no measured accelerator costs. Processor-only fields execute
/// normally.
///
/// # Errors
///
/// Returns [`ScheduleError::FieldNotFound`] if `field` is absent,
/// [`ScheduleError::MissingCampaignPurpose`] if the manifest has no campaign
/// stream purpose, [`ScheduleError::InvalidWorkItem`] for invalid work-item or
/// sampler configuration, [`ScheduleError::BackendUnavailable`] when a frozen
/// backend is unsupported, [`ScheduleError::AcceleratorCostMissing`] when an
/// accelerator cell has no measured cost.
///
/// # Panics
///
/// Does not intentionally panic. Invalid execution configuration is returned
/// as an error.
///
/// # Complexity
///
/// Linear in selected shards, with each shard dominated by its configured
/// permanent kernel and optional `O(n³)` determinant per matrix.
pub fn run_field(manifest: &CampaignManifest, field: u8) -> Result<FieldRun, ScheduleError> {
    run_field_with_worker_count(manifest, field, 1)
}

/// Executes every shard for one field using the caller's configured worker
/// count for `BatchParallel` cells.
///
/// Matrix draws remain serial and deterministic. A worker count of zero is
/// rejected before any sampler is opened; positive counts build a local Rayon
/// pool for each batch-parallel shard. Scalar, generic-Ryser, and
/// intra-matrix-parallel cells retain their existing per-matrix dispatch.
/// `BatchParallel` controls how matrices are distributed, while the resolved
/// `ProcessorPath` controls which kernel evaluates each matrix.
///
/// # Errors
///
/// Returns [`ScheduleError::FieldNotFound`] if `field` is absent,
/// [`ScheduleError::MissingCampaignPurpose`] if the manifest has no campaign
/// stream purpose, [`ScheduleError::InvalidWorkItem`] when `worker_count` is
/// zero or a work item, sampler, pool, or dispatch is invalid,
/// [`ScheduleError::BackendUnavailable`] when a frozen backend is unsupported,
/// [`ScheduleError::AcceleratorCostMissing`] when an accelerator cell has no
/// measured cost. The batch pool is configured exactly with `worker_count`.
///
/// # Panics
///
/// Does not intentionally panic. Invalid execution configuration and
/// pool-construction failures are returned as schedule errors.
///
/// # Complexity
///
/// For a batch of `B` matrices of dimension `n`, raw matrix storage is bounded
/// by the configured chunk limit and the per-matrix kernel remains the
/// selected single-matrix cost; the sampler itself advances in input order.
pub fn run_field_with_worker_count(
    manifest: &CampaignManifest,
    field: u8,
    worker_count: usize,
) -> Result<FieldRun, ScheduleError> {
    run_field_with_worker_count_and_accelerator(
        manifest,
        field,
        worker_count,
        &AcceleratorCostTable::default(),
    )
}

/// Executes one field with measured accelerator costs resolved per `(q, n)`.
///
/// The table is runtime input and is intentionally not read from or written to
/// the frozen manifest. Every accelerator work item resolves its own entry,
/// and all entries are preflighted before any matrix is drawn or evaluated.
/// An empty/default table therefore retains processor-only behavior while
/// refusing an accelerator field before partial execution.
///
/// # Errors
///
/// Returns [`ScheduleError::FieldNotFound`] if `field` is absent,
/// [`ScheduleError::MissingCampaignPurpose`] if the manifest has no campaign
/// stream purpose, [`ScheduleError::InvalidWorkItem`] when `worker_count` is
/// zero or a work item, sampler, pool, or dispatch is invalid,
/// [`ScheduleError::BackendUnavailable`] when a frozen backend is unsupported,
/// [`ScheduleError::AcceleratorCostMissing`] when any accelerator `(q, n)` has
/// no measured entry, and [`ScheduleError::AcceleratorDeviceUnavailable`] when
/// an accelerator cell's required device is absent.
///
/// # Panics
///
/// Does not intentionally panic. Invalid execution configuration and
/// pool-construction failures are returned as schedule errors.
///
/// # Complexity
///
/// Preflight is `O(S log C)` for `S` selected shards and `C` measured cells.
/// Execution is linear in selected shards, each dominated by its configured
/// permanent kernel and optional `O(n³)` determinant per matrix.
pub fn run_field_with_worker_count_and_accelerator(
    manifest: &CampaignManifest,
    field: u8,
    worker_count: usize,
    accelerator: &AcceleratorCostTable,
) -> Result<FieldRun, ScheduleError> {
    run_field_with_accelerator_evaluator(manifest, field, worker_count, accelerator, run_shard)
}

fn run_field_with_accelerator_evaluator<E>(
    manifest: &CampaignManifest,
    field: u8,
    worker_count: usize,
    accelerator: &AcceleratorCostTable,
    mut evaluator: E,
) -> Result<FieldRun, ScheduleError>
where
    E: FnMut(
        u64,
        u8,
        &WorkItem,
        usize,
        Option<AcceleratorConfig>,
    ) -> Result<ShardRun, ScheduleError>,
{
    validate_worker_count(worker_count)?;
    let purpose = manifest
        .stream_purposes
        .iter()
        .find(|purpose| purpose.tag == CAMPAIGN_CELL_PURPOSE_TAG)
        .ok_or(ScheduleError::MissingCampaignPurpose)?;
    let items = enumerate_work_items(manifest, Some(field))?;
    let accelerator_configs = items
        .iter()
        .map(|item| {
            if item.backend == Backend::Accelerator {
                accelerator.config_for(item.q, item.n).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, ScheduleError>>()?;
    let mut shards = Vec::with_capacity(items.len());
    for (item, accelerator_config) in items.iter().zip(accelerator_configs) {
        shards.push(evaluator(
            manifest.root_seed,
            purpose.tag,
            item,
            worker_count,
            accelerator_config,
        )?);
    }
    let summary = summarize(field, &shards, manifest.cells.len() as u64);
    Ok(FieldRun {
        q: field,
        shards,
        summary,
    })
}

/// Writes one field's shard JSON files and field summary, returning exactly the
/// paths opened for writing in write order.
///
/// The campaign manifest is read-only input and is intentionally not written.
/// Consequently two invocations selecting different fields open disjoint
/// shard and summary paths. The writer refuses every dataset file that already
/// exists, so re-emission of the same work items targets a fresh campaign tree.
pub fn emit_field(
    root: &Path,
    manifest: &CampaignManifest,
    run: &FieldRun,
) -> Result<Vec<PathBuf>, ScheduleError> {
    let campaign_name = manifest.campaign_id.to_string();
    if root.file_name() != Some(std::ffi::OsStr::new(&campaign_name)) {
        return Err(ScheduleError::InvalidWorkItem(format!(
            "output directory must be named by campaign id {campaign_name}"
        )));
    }
    if run.q != run.summary.q || run.q == 0 {
        return Err(ScheduleError::InvalidWorkItem(
            "field result has an invalid or mismatched field order".to_owned(),
        ));
    }
    let expected = enumerate_work_items(manifest, Some(run.q))?;
    if expected.len() != run.shards.len()
        || run.shards.iter().any(|shard| {
            !expected.iter().any(|item| {
                item.shard_id == shard.record.shard_id
                    && item.q == shard.record.stream_address.q
                    && item.n == shard.record.stream_address.n
            })
        })
    {
        return Err(ScheduleError::InvalidWorkItem(
            "field result does not match manifest work items".to_owned(),
        ));
    }

    let mut opened = Vec::with_capacity(run.shards.len() + 1);
    for shard in &run.shards {
        let address = &shard.record.stream_address;
        let relative = shard_record_file(address.q, address.n, shard.record.shard_id);
        let path = root.join(relative);
        create_parent(&path)?;
        let bytes =
            serde_json::to_vec_pretty(&shard.record).map_err(ScheduleError::Serialization)?;
        write_file(&path, &bytes)?;
        opened.push(path);
    }
    let path = root.join(field_summary_file(run.q));
    create_parent(&path)?;
    let bytes = serde_json::to_vec_pretty(&run.summary).map_err(ScheduleError::Serialization)?;
    write_file(&path, &bytes)?;
    opened.push(path);
    Ok(opened)
}

fn work_item(cell: &CellSpec, shard: &ShardSpec) -> Result<WorkItem, ScheduleError> {
    if cell.n == 0 || shard.stream_index >= (1_u64 << 56) || shard.shard_id >= 1_000_000 {
        return Err(ScheduleError::InvalidWorkItem(format!(
            "invalid work item q={}, n={}, shard={}, stream={}",
            cell.q, cell.n, shard.shard_id, shard.stream_index
        )));
    }
    Ok(WorkItem {
        q: cell.q,
        n: cell.n,
        shard_id: shard.shard_id,
        stream_index: shard.stream_index,
        matrix_count: shard_matrix_count(cell, shard),
        backend: cell.backend,
        determinant_companion: cell.determinant_companion,
    })
}

fn shard_matrix_count(cell: &CellSpec, shard: &ShardSpec) -> u64 {
    let shard_position = cell
        .shards
        .iter()
        .position(|candidate| candidate.shard_id == shard.shard_id)
        .unwrap_or(0) as u64;
    let start = shard_position.saturating_mul(cell.shard_size);
    cell.matrix_count.saturating_sub(start).min(cell.shard_size)
}

fn run_shard(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    worker_count: usize,
    accelerator: Option<AcceleratorConfig>,
) -> Result<ShardRun, ScheduleError> {
    Ok(run_shard_with_position(root_seed, purpose_tag, item, worker_count, accelerator)?.run)
}

/// Evaluates one manifest work item and returns its continuation position.
///
/// This is the library seam used by the checkpointed driver and by tests that
/// inject an evaluation failure. It performs no dataset I/O.
/// In a build that provides the accelerator backend, accelerator work is
/// refused with [`ScheduleError::AcceleratorCostMissing`] because this
/// convenience entry point supplies no measured cost; other builds return
/// [`ScheduleError::BackendUnavailable`] first.
///
/// # Errors
///
/// Returns [`ScheduleError::MissingCampaignPurpose`] when the manifest has no
/// campaign stream purpose, [`ScheduleError::InvalidWorkItem`] for invalid
/// work-item or sampler configuration, [`ScheduleError::BackendUnavailable`]
/// when the frozen backend is unsupported,
/// [`ScheduleError::AcceleratorCostMissing`] when an accelerator item has no
/// measured cost.
///
/// # Panics
///
/// Does not intentionally panic. Invalid execution configuration is returned
/// as an error.
///
/// # Complexity
///
/// Dominated by the selected permanent kernel, plus `O(n³)` when the
/// determinant companion is enabled.
pub fn evaluate_work_item(
    manifest: &CampaignManifest,
    item: &WorkItem,
) -> Result<EvaluatedShard, ScheduleError> {
    evaluate_work_item_with_worker_count(manifest, item, 1)
}

/// Evaluates one manifest work item using the caller's configured worker count
/// for a `BatchParallel` backend.
///
/// # Errors
///
/// Returns [`ScheduleError::MissingCampaignPurpose`] when the manifest has no
/// campaign stream purpose, [`ScheduleError::InvalidWorkItem`] when
/// `worker_count` is zero or the work item, sampler, pool, or dispatch is
/// invalid, [`ScheduleError::BackendUnavailable`] when the frozen backend is
/// unsupported, [`ScheduleError::AcceleratorCostMissing`] when an accelerator
/// item has no measured cost.
///
/// # Panics
///
/// Does not intentionally panic. Invalid execution configuration and
/// pool-construction failures are returned as schedule errors.
///
/// # Complexity
///
/// Dominated by the selected permanent kernel, plus `O(n³)` when the
/// determinant companion is enabled.
pub fn evaluate_work_item_with_worker_count(
    manifest: &CampaignManifest,
    item: &WorkItem,
    worker_count: usize,
) -> Result<EvaluatedShard, ScheduleError> {
    evaluate_work_item_with_worker_count_and_accelerator(manifest, item, worker_count, None)
}

/// Evaluates one work item with explicit accelerator launch-sizing input.
///
/// The cell's frozen backend decides which kernel runs; `accelerator` supplies
/// only the launch sizing an accelerator cell needs, and is ignored by every
/// processor path. Callers resolve it per cell from
/// [`AcceleratorCostTable::config_for`] so each size is sized by its own
/// measured cost.
///
/// # Errors
///
/// Returns [`ScheduleError::MissingCampaignPurpose`] when the manifest declares
/// no campaign-cell stream purpose, [`ScheduleError::InvalidWorkItem`] for a
/// zero or unrepresentable worker count, an unusable stream index, a sampler
/// that cannot be opened, or a dispatch that returns the wrong number of
/// values, [`ScheduleError::BackendUnavailable`] when the cell names a backend
/// this build does not provide, and — for an accelerator cell —
/// [`ScheduleError::AcceleratorCostMissing`] when `accelerator` is `None`, and
/// [`ScheduleError::AcceleratorDeviceUnavailable`] when no usable device is
/// present.
///
/// # Panics
///
/// Does not intentionally panic. Invalid execution configuration and
/// pool-construction failures are returned as schedule errors.
///
/// # Complexity
///
/// `O(matrix_count · n · 2^n)` field operations for the Gray-code kernels, plus
/// `O(n^3)` per matrix when the determinant companion is enabled. An
/// accelerator cell issues `ceil(matrix_count / launch)` device launches, where
/// `launch` comes from the measured per-matrix cost and the configured cap.
pub fn evaluate_work_item_with_worker_count_and_accelerator(
    manifest: &CampaignManifest,
    item: &WorkItem,
    worker_count: usize,
    accelerator: Option<AcceleratorConfig>,
) -> Result<EvaluatedShard, ScheduleError> {
    validate_worker_count(worker_count)?;
    let purpose = manifest
        .stream_purposes
        .iter()
        .find(|purpose| purpose.tag == CAMPAIGN_CELL_PURPOSE_TAG)
        .ok_or(ScheduleError::MissingCampaignPurpose)?;
    run_shard_with_position(
        manifest.root_seed,
        purpose.tag,
        item,
        worker_count,
        accelerator,
    )
}

fn run_shard_with_position(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    worker_count: usize,
    accelerator: Option<AcceleratorConfig>,
) -> Result<EvaluatedShard, ScheduleError> {
    match item.q {
        3 => run_shard_for::<3>(
            root_seed,
            purpose_tag,
            item,
            FieldOrder::F3,
            worker_count,
            accelerator,
        ),
        5 => run_shard_for::<5>(
            root_seed,
            purpose_tag,
            item,
            FieldOrder::F5,
            worker_count,
            accelerator,
        ),
        7 => run_shard_for::<7>(
            root_seed,
            purpose_tag,
            item,
            FieldOrder::F7,
            worker_count,
            accelerator,
        ),
        q => Err(ScheduleError::InvalidWorkItem(format!(
            "unsupported campaign field q={q}"
        ))),
    }
}

fn run_shard_for<const Q: u64>(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    field_order: FieldOrder,
    worker_count: usize,
    accelerator: Option<AcceleratorConfig>,
) -> Result<EvaluatedShard, ScheduleError> {
    let mut observer = |_: &[Fp<Q>], _: u64, _: Option<u64>| {};
    run_shard_for_with_observer_with_worker_count(
        root_seed,
        purpose_tag,
        item,
        field_order,
        worker_count,
        accelerator,
        &mut observer,
    )
}

/// Evaluates a shard while observing the exact matrix operands and both values.
///
/// The observer runs once per sampled matrix after both evaluations, with the
/// row-major operand passed to the evaluators, the permanent value, and the
/// determinant value when the companion is enabled. Production execution uses
/// [`run_shard_for`] with a no-op observer; this seam is crate-visible so the
/// one-draw contract can be tested without changing the public API.
#[cfg(test)]
pub(crate) fn run_shard_for_with_observer<const Q: u64, O>(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    field_order: FieldOrder,
    observer: &mut O,
) -> Result<EvaluatedShard, ScheduleError>
where
    O: FnMut(&[Fp<Q>], u64, Option<u64>),
{
    run_shard_for_with_observer_with_worker_count(
        root_seed,
        purpose_tag,
        item,
        field_order,
        1,
        None,
        observer,
    )
}

#[cfg(feature = "hip")]
fn run_shard_for_accelerator<const Q: u64, O>(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    field_order: FieldOrder,
    accelerator: Option<AcceleratorConfig>,
    observer: &mut O,
) -> Result<EvaluatedShard, ScheduleError>
where
    O: FnMut(&[Fp<Q>], u64, Option<u64>),
{
    run_shard_for_accelerator_with_dispatch(
        root_seed,
        purpose_tag,
        item,
        field_order,
        accelerator,
        gf2_algebra::gpu::has_usable_device,
        dispatch_accelerator::<Q>,
        observer,
    )
}

#[cfg(feature = "hip")]
#[allow(clippy::too_many_arguments)]
fn run_shard_for_accelerator_with_dispatch<const Q: u64, P, D, O>(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    field_order: FieldOrder,
    accelerator: Option<AcceleratorConfig>,
    probe: P,
    mut dispatch: D,
    observer: &mut O,
) -> Result<EvaluatedShard, ScheduleError>
where
    P: Fn() -> bool,
    D: FnMut(&[Vec<Fp<Q>>], usize) -> Result<(Vec<u64>, Duration), ScheduleError>,
    O: FnMut(&[Fp<Q>], u64, Option<u64>),
{
    // No measured cost, no accelerator run. A placeholder here would size this
    // cell's launches from a number nobody measured, which is the same defect
    // as a fixed batch size and harder to see.
    let accelerator = accelerator.ok_or(ScheduleError::AcceleratorCostMissing {
        q: item.q,
        n: item.n,
    })?;
    if !probe() {
        return Err(ScheduleError::AcceleratorDeviceUnavailable {
            q: item.q,
            n: item.n,
            device: "a usable HIP accelerator device",
        });
    }
    let stream = StreamIndex::new(item.stream_index).map_err(|error| {
        ScheduleError::InvalidWorkItem(format!("invalid stream index: {error}"))
    })?;
    let address = MatrixAddress::new(
        root_seed,
        field_order,
        usize::from(item.n),
        SamplerPurpose::CampaignCell,
        stream,
    );
    let mut sampler = MatrixSampler::<Q>::new(address).map_err(|error| {
        ScheduleError::InvalidWorkItem(format!("cannot open matrix sampler: {error}"))
    })?;
    let n = usize::from(item.n);
    let matrix_entries = n.checked_mul(n).ok_or_else(|| {
        ScheduleError::InvalidWorkItem("matrix dimension overflows entry count".to_owned())
    })?;
    let mut histogram = vec![0_u64; Q as usize];
    let mut permanent_zero_count = 0_u64;
    let mut draw = Duration::ZERO;
    let mut pack = Duration::ZERO;
    let mut evaluate = Duration::ZERO;
    let mut determinant = Duration::ZERO;
    let mut count = Duration::ZERO;
    let mut determinant_zero_count = 0_u64;
    let mut remaining = item.matrix_count;

    while remaining != 0 {
        let batch_len = launch_size(
            accelerator.per_matrix_cost,
            accelerator.launch_cap,
            remaining,
        );
        let started = Instant::now();
        let mut matrices = Vec::with_capacity(batch_len);
        for _ in 0..batch_len {
            let mut entries = vec![Fp::<Q>::new(0); matrix_entries];
            sampler.fill_next_matrix(&mut entries);
            matrices.push(entries);
        }
        draw += started.elapsed();

        let started = Instant::now();
        let (permanent_values, pack_span) = dispatch(&matrices, n)?;
        // The dispatch reports the packing it performed so `pack` measures
        // packing and `evaluate` measures the device launch alone, matching
        // the processor paths' phase attribution.
        pack += pack_span;
        evaluate += started.elapsed().saturating_sub(pack_span);
        if permanent_values.len() != matrices.len() {
            return Err(ScheduleError::InvalidWorkItem(format!(
                "accelerator dispatch returned {} values for {} matrices",
                permanent_values.len(),
                matrices.len()
            )));
        }

        let determinant_values = if item.determinant_companion == DeterminantPlan::Evaluate {
            let started = Instant::now();
            let values: Vec<u64> = matrices
                .iter()
                .map(|entries| evaluate_determinant(entries, n))
                .collect();
            determinant += started.elapsed();
            Some(values)
        } else {
            None
        };

        for index in 0..batch_len {
            let value = permanent_values[index];
            let determinant_value = determinant_values.as_ref().map(|values| values[index]);
            if determinant_value == Some(0) {
                determinant_zero_count += 1;
            }
            observer(&matrices[index], value, determinant_value);

            let started = Instant::now();
            histogram[value as usize] += 1;
            if value == 0 {
                permanent_zero_count += 1;
            }
            count += started.elapsed();
        }
        remaining -= batch_len as u64;
    }

    Ok(EvaluatedShard {
        run: ShardRun {
            record: ShardRecord {
                schema_version: SCHEMA_VERSION,
                shard_id: item.shard_id,
                stream_address: StreamAddress {
                    root_seed,
                    q: item.q,
                    n: item.n,
                    purpose_tag,
                    stream_index: item.stream_index,
                },
                matrix_count: item.matrix_count,
                permanent_zero_count,
                permanent_histogram: histogram,
                determinant: match item.determinant_companion {
                    DeterminantPlan::Evaluate => DeterminantCount::Evaluated {
                        sample_count: item.matrix_count,
                        zero_count: determinant_zero_count,
                    },
                    DeterminantPlan::NotEvaluated => DeterminantCount::NotEvaluated,
                },
            },
            timing: PhaseDurations {
                draw,
                pack,
                evaluate,
                determinant,
                count,
            },
        },
        generator_word_position: sampler.generator_word_position(),
    })
}

#[cfg(feature = "hip")]
/// Packs one launch and evaluates it on the accelerator.
///
/// `n` is the cell's frozen dimension, passed in rather than recovered from the
/// operand length: the manifest already fixes it, and a second derivation would
/// be a competing source of truth for it.
///
/// Returns the permanent values in input order together with the wall-clock
/// span spent packing, so the caller can charge packing to the `pack` phase and
/// leave `evaluate` measuring the device launch alone.
fn dispatch_accelerator<const Q: u64>(
    matrices: &[Vec<Fp<Q>>],
    n: usize,
) -> Result<(Vec<u64>, Duration), ScheduleError> {
    if matrices.is_empty() {
        return Err(ScheduleError::InvalidWorkItem(
            "accelerator launch is empty".to_owned(),
        ));
    }
    let entry_count = n.checked_mul(n).ok_or_else(|| {
        ScheduleError::InvalidWorkItem("matrix dimension overflows entry count".to_owned())
    })?;
    if let Some(position) = matrices
        .iter()
        .position(|entries| entries.len() != entry_count)
    {
        return Err(ScheduleError::InvalidWorkItem(format!(
            "accelerator launch matrix {position} has {} entries, expected {entry_count} for n={n}",
            matrices[position].len()
        )));
    }
    let pack_started = Instant::now();
    let packed: Vec<PackedMatrix> = matrices
        .iter()
        .map(|entries| PackedMatrix::new(entries, n))
        .collect();
    let pack_span = pack_started.elapsed();
    match Q {
        3 => {
            let matrices: Vec<_> = packed
                .into_iter()
                .map(|matrix| match matrix {
                    PackedMatrix::F3(matrix) => Ok(matrix),
                    _ => Err(ScheduleError::InvalidWorkItem(
                        "packed matrix and F_3 accelerator path do not agree".to_owned(),
                    )),
                })
                .collect::<Result<_, _>>()?;
            Ok((
                gf2_algebra::gpu::permanent_batch_bipedal3(&matrices)
                    .into_iter()
                    .map(|value| value.value())
                    .collect(),
                pack_span,
            ))
        }
        5 => {
            let matrices: Vec<_> = packed
                .into_iter()
                .map(|matrix| match matrix {
                    PackedMatrix::F5(matrix) => Ok(matrix),
                    _ => Err(ScheduleError::InvalidWorkItem(
                        "packed matrix and F_5 accelerator path do not agree".to_owned(),
                    )),
                })
                .collect::<Result<_, _>>()?;
            Ok((
                gf2_algebra::gpu::permanent_batch_bipedal5(&matrices)
                    .into_iter()
                    .map(|value| value.value())
                    .collect(),
                pack_span,
            ))
        }
        7 => {
            let matrices: Vec<_> = packed
                .into_iter()
                .map(|matrix| match matrix {
                    PackedMatrix::F7(matrix) => Ok(matrix),
                    _ => Err(ScheduleError::InvalidWorkItem(
                        "packed matrix and F_7 accelerator path do not agree".to_owned(),
                    )),
                })
                .collect::<Result<_, _>>()?;
            Ok((
                gf2_algebra::gpu::permanent_batch_bipedal7(&matrices)
                    .into_iter()
                    .map(|value| value.value())
                    .collect(),
                pack_span,
            ))
        }
        _ => Err(ScheduleError::InvalidWorkItem(format!(
            "unsupported accelerator field q={Q}"
        ))),
    }
}

fn run_shard_for_with_observer_with_worker_count<const Q: u64, O>(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    field_order: FieldOrder,
    worker_count: usize,
    accelerator: Option<AcceleratorConfig>,
    observer: &mut O,
) -> Result<EvaluatedShard, ScheduleError>
where
    O: FnMut(&[Fp<Q>], u64, Option<u64>),
{
    validate_worker_count(worker_count)?;
    #[cfg(not(feature = "hip"))]
    let _ = accelerator;
    let processor_path = resolve_processor_path(item.q, item.n, item.backend)?;
    #[cfg(feature = "hip")]
    if processor_path == ProcessorPath::Accelerator {
        return run_shard_for_accelerator(
            root_seed,
            purpose_tag,
            item,
            field_order,
            accelerator,
            observer,
        );
    }
    if item.backend == Backend::BatchParallel {
        return run_shard_for_batch(
            root_seed,
            purpose_tag,
            item,
            field_order,
            worker_count,
            processor_path,
            observer,
        );
    }
    run_shard_for_serial(
        root_seed,
        purpose_tag,
        item,
        field_order,
        processor_path,
        observer,
    )
}

fn run_shard_for_serial<const Q: u64, O>(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    field_order: FieldOrder,
    processor_path: ProcessorPath,
    observer: &mut O,
) -> Result<EvaluatedShard, ScheduleError>
where
    O: FnMut(&[Fp<Q>], u64, Option<u64>),
{
    let stream = StreamIndex::new(item.stream_index).map_err(|error| {
        ScheduleError::InvalidWorkItem(format!("invalid stream index: {error}"))
    })?;
    let address = MatrixAddress::new(
        root_seed,
        field_order,
        usize::from(item.n),
        SamplerPurpose::CampaignCell,
        stream,
    );
    let mut sampler = MatrixSampler::<Q>::new(address).map_err(|error| {
        ScheduleError::InvalidWorkItem(format!("cannot open matrix sampler: {error}"))
    })?;
    let n = usize::from(item.n);
    let mut row_major = vec![Fp::<Q>::new(0); n * n];
    let mut histogram = vec![0_u64; Q as usize];
    let mut permanent_zero_count = 0_u64;
    let mut draw = Duration::ZERO;
    let mut pack = Duration::ZERO;
    let mut evaluate = Duration::ZERO;
    let mut determinant = Duration::ZERO;
    let mut count = Duration::ZERO;
    let mut determinant_zero_count = 0_u64;

    for _ in 0..item.matrix_count {
        let started = Instant::now();
        sampler.fill_next_matrix(&mut row_major);
        draw += started.elapsed();

        let packed = if processor_path == ProcessorPath::GenericRyser {
            None
        } else {
            let started = Instant::now();
            let packed = PackedMatrix::new(&row_major, n);
            pack += started.elapsed();
            Some(packed)
        };

        let started = Instant::now();
        let value = evaluate_permanent(processor_path, &row_major, packed.as_ref(), n)?;
        evaluate += started.elapsed();

        let determinant_value = if item.determinant_companion == DeterminantPlan::Evaluate {
            let started = Instant::now();
            let value = evaluate_determinant(&row_major, n);
            if value == 0 {
                determinant_zero_count += 1;
            }
            determinant += started.elapsed();
            Some(value)
        } else {
            None
        };

        observer(&row_major, value, determinant_value);

        let started = Instant::now();
        histogram[value as usize] += 1;
        if value == 0 {
            permanent_zero_count += 1;
        }
        count += started.elapsed();
    }

    Ok(EvaluatedShard {
        run: ShardRun {
            record: ShardRecord {
                schema_version: SCHEMA_VERSION,
                shard_id: item.shard_id,
                stream_address: StreamAddress {
                    root_seed,
                    q: item.q,
                    n: item.n,
                    purpose_tag,
                    stream_index: item.stream_index,
                },
                matrix_count: item.matrix_count,
                permanent_zero_count,
                permanent_histogram: histogram,
                determinant: match item.determinant_companion {
                    DeterminantPlan::Evaluate => DeterminantCount::Evaluated {
                        sample_count: item.matrix_count,
                        zero_count: determinant_zero_count,
                    },
                    DeterminantPlan::NotEvaluated => DeterminantCount::NotEvaluated,
                },
            },
            timing: PhaseDurations {
                draw,
                pack,
                evaluate,
                determinant,
                count,
            },
        },
        generator_word_position: sampler.generator_word_position(),
    })
}

fn run_shard_for_batch<const Q: u64, O>(
    root_seed: u64,
    purpose_tag: u8,
    item: &WorkItem,
    field_order: FieldOrder,
    worker_count: usize,
    processor_path: ProcessorPath,
    observer: &mut O,
) -> Result<EvaluatedShard, ScheduleError>
where
    O: FnMut(&[Fp<Q>], u64, Option<u64>),
{
    let stream = StreamIndex::new(item.stream_index).map_err(|error| {
        ScheduleError::InvalidWorkItem(format!("invalid stream index: {error}"))
    })?;
    let address = MatrixAddress::new(
        root_seed,
        field_order,
        usize::from(item.n),
        SamplerPurpose::CampaignCell,
        stream,
    );
    let mut sampler = MatrixSampler::<Q>::new(address).map_err(|error| {
        ScheduleError::InvalidWorkItem(format!("cannot open matrix sampler: {error}"))
    })?;
    let n = usize::from(item.n);
    let matrix_entries = n.checked_mul(n).ok_or_else(|| {
        ScheduleError::InvalidWorkItem("matrix dimension overflows entry count".to_owned())
    })?;
    let chunk_size =
        (BATCH_CHUNK_MAX_MATRIX_ENTRIES / matrix_entries.max(1)).clamp(1, BATCH_CHUNK_MAX_MATRICES);
    let pool = ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build()
        .map_err(|error| {
            ScheduleError::InvalidWorkItem(format!("cannot build batch thread pool: {error}"))
        })?;

    let mut histogram = vec![0_u64; Q as usize];
    let mut permanent_zero_count = 0_u64;
    let mut draw = Duration::ZERO;
    let mut pack = Duration::ZERO;
    let mut evaluate = Duration::ZERO;
    let mut determinant = Duration::ZERO;
    let mut count = Duration::ZERO;
    let mut determinant_zero_count = 0_u64;
    let mut remaining = item.matrix_count;

    while remaining != 0 {
        let batch_len = remaining.min(chunk_size as u64) as usize;
        let started = Instant::now();
        let mut matrices = Vec::with_capacity(batch_len);
        for _ in 0..batch_len {
            let mut entries = vec![Fp::<Q>::new(0); matrix_entries];
            sampler.fill_next_matrix(&mut entries);
            matrices.push(entries);
        }
        draw += started.elapsed();

        let packed = if processor_path == ProcessorPath::GenericRyser {
            None
        } else {
            let started = Instant::now();
            let packed: Vec<_> = pool.install(|| {
                matrices
                    .par_iter()
                    .map(|entries| PackedMatrix::new(entries, n))
                    .collect()
            });
            pack += started.elapsed();
            Some(packed)
        };

        let started = Instant::now();
        let permanent_values: Vec<Result<u64, ScheduleError>> = pool.install(|| {
            if let Some(packed) = packed.as_ref() {
                packed
                    .par_iter()
                    .zip(matrices.par_iter())
                    .map(|(packed, entries)| {
                        evaluate_permanent(processor_path, entries, Some(packed), n)
                    })
                    .collect()
            } else {
                matrices
                    .par_iter()
                    .map(|entries| evaluate_permanent(processor_path, entries, None, n))
                    .collect()
            }
        });
        evaluate += started.elapsed();
        let permanent_values: Vec<u64> = permanent_values.into_iter().collect::<Result<_, _>>()?;

        let determinant_values = if item.determinant_companion == DeterminantPlan::Evaluate {
            let started = Instant::now();
            let values: Vec<u64> = pool.install(|| {
                matrices
                    .par_iter()
                    .map(|entries| evaluate_determinant(entries, n))
                    .collect()
            });
            determinant += started.elapsed();
            Some(values)
        } else {
            None
        };

        for index in 0..batch_len {
            let value = permanent_values[index];
            let determinant_value = determinant_values.as_ref().map(|values| values[index]);
            if determinant_value == Some(0) {
                determinant_zero_count += 1;
            }
            observer(&matrices[index], value, determinant_value);

            let started = Instant::now();
            histogram[value as usize] += 1;
            if value == 0 {
                permanent_zero_count += 1;
            }
            count += started.elapsed();
        }
        remaining -= batch_len as u64;
    }

    Ok(EvaluatedShard {
        run: ShardRun {
            record: ShardRecord {
                schema_version: SCHEMA_VERSION,
                shard_id: item.shard_id,
                stream_address: StreamAddress {
                    root_seed,
                    q: item.q,
                    n: item.n,
                    purpose_tag,
                    stream_index: item.stream_index,
                },
                matrix_count: item.matrix_count,
                permanent_zero_count,
                permanent_histogram: histogram,
                determinant: match item.determinant_companion {
                    DeterminantPlan::Evaluate => DeterminantCount::Evaluated {
                        sample_count: item.matrix_count,
                        zero_count: determinant_zero_count,
                    },
                    DeterminantPlan::NotEvaluated => DeterminantCount::NotEvaluated,
                },
            },
            timing: PhaseDurations {
                draw,
                pack,
                evaluate,
                determinant,
                count,
            },
        },
        generator_word_position: sampler.generator_word_position(),
    })
}

fn validate_worker_count(worker_count: usize) -> Result<(), ScheduleError> {
    if worker_count == 0 {
        return Err(ScheduleError::InvalidWorkItem(
            "worker_count must be non-zero".to_owned(),
        ));
    }
    Ok(())
}

enum PackedMatrix {
    F3(Bipedal3Matrix),
    F5(Packed5Matrix),
    F7(Packed7Matrix),
}

impl PackedMatrix {
    fn new<const Q: u64>(entries: &[Fp<Q>], n: usize) -> Self {
        match Q {
            3 => {
                let values: Vec<Fp<3>> =
                    entries.iter().map(|value| Fp::new(value.value())).collect();
                Self::F3(Bipedal3Matrix::from_row_major(&values, n, n))
            }
            5 => {
                let values: Vec<Fp<5>> =
                    entries.iter().map(|value| Fp::new(value.value())).collect();
                Self::F5(Packed5Matrix::from_row_major(&values, n, n))
            }
            7 => {
                let values: Vec<Fp<7>> =
                    entries.iter().map(|value| Fp::new(value.value())).collect();
                Self::F7(Packed7Matrix::from_row_major(&values, n, n))
            }
            _ => unreachable!("campaign q is validated before packing"),
        }
    }
}

fn evaluate_permanent<const Q: u64>(
    processor_path: ProcessorPath,
    row_major: &[Fp<Q>],
    packed: Option<&PackedMatrix>,
    n: usize,
) -> Result<u64, ScheduleError> {
    let value = match processor_path {
        ProcessorPath::Bipedal3SingleWord => match packed {
            Some(PackedMatrix::F3(matrix)) => permanent_bipedal3_singleword(matrix).value(),
            _ => {
                return Err(ScheduleError::InvalidWorkItem(
                    "packed matrix and F_3 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::Bipedal5SingleWord => match packed {
            Some(PackedMatrix::F5(matrix)) => permanent_bipedal5_singleword(matrix).value(),
            _ => {
                return Err(ScheduleError::InvalidWorkItem(
                    "packed matrix and F_5 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::Bipedal7SingleWord => match packed {
            Some(PackedMatrix::F7(matrix)) => permanent_bipedal7_singleword(matrix).value(),
            _ => {
                return Err(ScheduleError::InvalidWorkItem(
                    "packed matrix and F_7 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::Bipedal3IntraMatrixParallel => match packed {
            Some(PackedMatrix::F3(matrix)) => permanent_bipedal3_parallel(matrix).value(),
            _ => {
                return Err(ScheduleError::InvalidWorkItem(
                    "packed matrix and F_3 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::GenericRyser => permanent_ryser(row_major, n).value(),
        #[cfg(feature = "hip")]
        ProcessorPath::Accelerator => {
            return Err(ScheduleError::InvalidWorkItem(
                "accelerator path must use batched dispatch".to_owned(),
            ));
        }
    };
    Ok(value)
}

fn evaluate_determinant<const Q: u64>(row_major: &[Fp<Q>], n: usize) -> u64 {
    let rows: Vec<FieldVec<Fp<Q>>> = row_major.chunks(n).map(|row| row.to_vec().into()).collect();
    FieldMatrix::from_rows(rows).det().value()
}

fn summarize(q: u8, shards: &[ShardRun], family_test_count: u64) -> FieldSummary {
    let mut rows = Vec::new();
    let mut index = 0;
    while index < shards.len() {
        let n = shards[index].record.stream_address.n;
        let mut matrix_count = 0_u64;
        let mut zero_count = 0_u64;
        let mut determinant_evaluated = false;
        let mut determinant_sample_count = 0_u64;
        let mut determinant_zero_count = 0_u64;
        while index < shards.len() && shards[index].record.stream_address.n == n {
            matrix_count += shards[index].record.matrix_count;
            zero_count += shards[index].record.permanent_zero_count;
            if let DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            } = &shards[index].record.determinant
            {
                determinant_evaluated = true;
                determinant_sample_count += *sample_count;
                determinant_zero_count += *zero_count;
            }
            index += 1;
        }
        let (lower, upper) = wilson_interval(zero_count, matrix_count);
        let determinant = if determinant_evaluated {
            DeterminantCount::Evaluated {
                sample_count: determinant_sample_count,
                zero_count: determinant_zero_count,
            }
        } else {
            DeterminantCount::NotEvaluated
        };
        let determinant_estimate = match determinant {
            DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            } => {
                let (lower, upper) = wilson_interval(zero_count, sample_count);
                DeterminantEstimate::Evaluated {
                    estimate: ProportionEstimate {
                        point: zero_count as f64 / sample_count as f64,
                        interval: Interval { lower, upper },
                    },
                    verdict: determinant_acceptance(
                        q,
                        n,
                        zero_count,
                        sample_count,
                        family_test_count,
                    ),
                }
            }
            DeterminantCount::NotEvaluated => DeterminantEstimate::NotEvaluated,
        };
        rows.push(SummaryRow {
            schema_version: SCHEMA_VERSION,
            q,
            n,
            matrix_count,
            permanent_zero_count: zero_count,
            determinant,
            terminal_state: CellTerminalState::Completed {
                permanent_estimate: ProportionEstimate {
                    point: zero_count as f64 / matrix_count as f64,
                    interval: Interval { lower, upper },
                },
                permanent_verdict: permanent_acceptance(
                    q,
                    zero_count,
                    matrix_count,
                    family_test_count,
                ),
                determinant_estimate,
            },
        });
    }
    FieldSummary {
        schema_version: SCHEMA_VERSION,
        q,
        rows,
        quarantined: Vec::new(),
    }
}

/// Builds a field summary while retaining failed work-item identities.
pub(crate) fn summarize_with_quarantine(
    manifest: &CampaignManifest,
    q: u8,
    shards: &[ShardRun],
    quarantined: Vec<QuarantinedShard>,
) -> FieldSummary {
    let mut summary = summarize(q, shards, manifest.cells.len() as u64);
    for row in &mut summary.rows {
        if quarantined
            .iter()
            .any(|item| item.q == row.q && item.n == row.n)
        {
            row.terminal_state = CellTerminalState::Halted {
                reason: super::schema::HaltReason::ExecutionFailure,
            };
        }
    }
    for cell in manifest.cells.iter().filter(|cell| cell.q == q) {
        if !summary.rows.iter().any(|row| row.n == cell.n) {
            summary.rows.push(SummaryRow {
                schema_version: SCHEMA_VERSION,
                q,
                n: cell.n,
                matrix_count: 0,
                permanent_zero_count: 0,
                determinant: match cell.determinant_companion {
                    DeterminantPlan::Evaluate => DeterminantCount::Evaluated {
                        sample_count: 0,
                        zero_count: 0,
                    },
                    DeterminantPlan::NotEvaluated => DeterminantCount::NotEvaluated,
                },
                terminal_state: CellTerminalState::Halted {
                    reason: super::schema::HaltReason::ExecutionFailure,
                },
            });
        }
    }
    summary.rows.sort_by_key(|row| row.n);
    summary.quarantined = quarantined;
    summary
}

fn permanent_acceptance(
    q: u8,
    permanent_zero_count: u64,
    matrix_count: u64,
    family_test_count: u64,
) -> AcceptanceVerdict {
    let level = bonferroni_level(PERMANENT_FAMILYWISE_ERROR, family_test_count);
    if permanent_zero_floor_test(permanent_zero_count, matrix_count, u64::from(q)).rejects_at(level)
    {
        AcceptanceVerdict::Rejected
    } else {
        AcceptanceVerdict::Accepted
    }
}

fn determinant_acceptance(
    q: u8,
    n: u16,
    determinant_zero_count: u64,
    determinant_sample_count: u64,
    family_test_count: u64,
) -> AcceptanceVerdict {
    let level = bonferroni_level(DETERMINANT_FAMILYWISE_ERROR, family_test_count);
    let null_probability = determinant_null_probability(q, n);
    if two_sided_test(
        determinant_zero_count,
        determinant_sample_count,
        null_probability,
    )
    .rejects_at(level)
    {
        AcceptanceVerdict::Rejected
    } else {
        AcceptanceVerdict::Accepted
    }
}

/// Returns the exact finite-size singular probability
/// \(p_{\det}(q,n)=1-\prod_{i=1}^{n}(1-q^{-i})\) in `f64`.
fn determinant_null_probability(q: u8, n: u16) -> f64 {
    let q = f64::from(q);
    1.0 - (1..=n).fold(1.0, |nonsingular_probability, i| {
        nonsingular_probability * (1.0 - q.powi(-i32::from(i)))
    })
}

fn wilson_interval(successes: u64, trials: u64) -> (f64, f64) {
    gf2_stats::intervals::wilson_interval(successes, trials, gf2_stats::intervals::Z_95)
}

fn create_parent(path: &Path) -> Result<(), ScheduleError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ScheduleError::Io {
            path: parent.to_owned(),
            source,
        })?;
    }
    Ok(())
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ScheduleError> {
    write_file_with_durability_hook(path, bytes, |_| {})
}

fn write_file_with_durability_hook(
    path: &Path,
    bytes: &[u8],
    mut on_durable: impl FnMut(&Path),
) -> Result<(), ScheduleError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ScheduleError::Io {
            path: path.to_owned(),
            source,
        })?;
    file.write_all(bytes).map_err(|source| ScheduleError::Io {
        path: path.to_owned(),
        source,
    })?;
    file.sync_all().map_err(|source| ScheduleError::Io {
        path: path.to_owned(),
        source,
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| ScheduleError::Io {
            path: parent.to_owned(),
            source,
        })?;
    on_durable(path);
    Ok(())
}

/// Emits one shard record with the same create-new refusal as [`emit_field`],
/// fsyncing the file and its directory and reporting the durable path.
pub(crate) fn emit_shard_with_durability_hook(
    root: &Path,
    manifest: &CampaignManifest,
    shard: &ShardRun,
    mut on_durable: impl FnMut(&Path),
) -> Result<PathBuf, ScheduleError> {
    let campaign_name = manifest.campaign_id.to_string();
    if root.file_name() != Some(std::ffi::OsStr::new(&campaign_name)) {
        return Err(ScheduleError::InvalidWorkItem(format!(
            "output directory must be named by campaign id {campaign_name}"
        )));
    }
    let address = &shard.record.stream_address;
    let expected = enumerate_work_items(manifest, Some(address.q))?;
    if !expected.iter().any(|item| {
        item.shard_id == shard.record.shard_id && item.q == address.q && item.n == address.n
    }) {
        return Err(ScheduleError::InvalidWorkItem(
            "shard result does not match manifest work items".to_owned(),
        ));
    }
    let path = root.join(shard_record_file(
        address.q,
        address.n,
        shard.record.shard_id,
    ));
    create_parent(&path)?;
    let bytes = shard_record_bytes(&shard.record)?;
    write_file_with_durability_hook(&path, &bytes, &mut on_durable)?;
    Ok(path)
}

/// Canonical serialized form of a shard record, shared by emission and the
/// resume-time adoption comparison.
pub(crate) fn shard_record_bytes(record: &ShardRecord) -> Result<Vec<u8>, ScheduleError> {
    serde_json::to_vec_pretty(record).map_err(ScheduleError::Serialization)
}

/// Emits a field summary after all selected work items reach a terminal
/// state, with the existing create-new refusal and the same durability
/// contract as shard emission.
pub(crate) fn emit_summary_with_durability_hook(
    root: &Path,
    manifest: &CampaignManifest,
    summary: &FieldSummary,
    mut on_durable: impl FnMut(&Path),
) -> Result<PathBuf, ScheduleError> {
    let campaign_name = manifest.campaign_id.to_string();
    if root.file_name() != Some(std::ffi::OsStr::new(&campaign_name)) {
        return Err(ScheduleError::InvalidWorkItem(format!(
            "output directory must be named by campaign id {campaign_name}"
        )));
    }
    let path = root.join(field_summary_file(summary.q));
    create_parent(&path)?;
    let bytes = serde_json::to_vec_pretty(summary).map_err(ScheduleError::Serialization)?;
    write_file_with_durability_hook(&path, &bytes, &mut on_durable)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permanent_campaign::schema::{
        ArtifactIdentity, Availability, CellSpec, DeterminantCount, DeterminantPlan, GitRevision,
        Provenance, RngAlgorithm, ShardSpec, StreamPurpose,
    };
    use gf2_core::field::{matrix::FieldMatrix, FieldVec};
    use gf2_stats::binomial::{bonferroni_level, permanent_zero_floor_test, two_sided_test};
    use std::collections::BTreeSet;

    #[test]
    fn test_resolve_processor_path_reads_scalar_for_each_field() {
        assert_eq!(
            resolve_processor_path(3, 20, Backend::Scalar).unwrap(),
            ProcessorPath::Bipedal3SingleWord
        );
        assert_eq!(
            resolve_processor_path(5, 20, Backend::Scalar).unwrap(),
            ProcessorPath::Bipedal5SingleWord
        );
        assert_eq!(
            resolve_processor_path(7, 12, Backend::Scalar).unwrap(),
            ProcessorPath::Bipedal7SingleWord
        );
    }

    #[test]
    fn test_resolve_processor_path_reads_batch_parallel_for_each_field() {
        assert_eq!(
            resolve_processor_path(3, 20, Backend::BatchParallel).unwrap(),
            ProcessorPath::Bipedal3SingleWord
        );
        assert_eq!(
            resolve_processor_path(5, 20, Backend::BatchParallel).unwrap(),
            ProcessorPath::Bipedal5SingleWord
        );
        assert_eq!(
            resolve_processor_path(7, 12, Backend::BatchParallel).unwrap(),
            ProcessorPath::Bipedal7SingleWord
        );
    }

    #[test]
    fn test_resolve_processor_path_reads_intra_matrix_parallel_for_f3() {
        let parallel = resolve_processor_path(3, 20, Backend::IntraMatrixParallel).unwrap();
        let scalar = resolve_processor_path(3, 20, Backend::Scalar).unwrap();
        assert_eq!(parallel, ProcessorPath::Bipedal3IntraMatrixParallel);
        assert_ne!(parallel, scalar);
    }

    #[test]
    fn test_resolve_processor_path_reads_generic_ryser_above_the_f7_ceiling() {
        assert_eq!(
            resolve_processor_path(7, 24, Backend::GenericRyser).unwrap(),
            ProcessorPath::GenericRyser
        );
    }

    #[test]
    fn test_resolve_processor_path_halts_for_intra_matrix_parallel_on_f5_and_f7() {
        for (q, n) in [(5, 20), (7, 12)] {
            let error = resolve_processor_path(q, n, Backend::IntraMatrixParallel).unwrap_err();
            assert!(matches!(
                error,
                ScheduleError::BackendUnavailable {
                    q: error_q,
                    n: error_n,
                    backend: Backend::IntraMatrixParallel,
                } if error_q == q && error_n == n
            ));
            let rendered = error.to_string();
            assert!(rendered.contains(&format!("q={q}")));
            assert!(rendered.contains(&format!("n={n}")));
            assert!(rendered.contains("intra_matrix_parallel"));
        }
    }

    #[test]
    fn test_resolve_processor_path_halts_above_the_packed_f7_ceiling() {
        for backend in [Backend::Scalar, Backend::BatchParallel] {
            let error = resolve_processor_path(7, 24, backend).unwrap_err();
            assert!(matches!(
                error,
                ScheduleError::BackendUnavailable {
                    q: 7,
                    n: 24,
                    backend: error_backend,
                } if error_backend == backend
            ));
        }
    }

    #[cfg(not(feature = "hip"))]
    #[test]
    fn test_resolve_processor_path_refuses_accelerator_without_the_hip_feature() {
        let error = resolve_processor_path(3, 20, Backend::Accelerator).unwrap_err();
        assert!(matches!(
            error,
            ScheduleError::BackendUnavailable {
                q: 3,
                n: 20,
                backend: Backend::Accelerator,
            }
        ));
        let rendered = error.to_string();
        assert!(rendered.contains("q=3"));
        assert!(rendered.contains("n=20"));
        assert!(rendered.contains("accelerator"));
    }

    #[cfg(feature = "hip")]
    #[test]
    fn test_resolve_processor_path_reads_accelerator_with_the_hip_feature() {
        assert_eq!(
            resolve_processor_path(3, 20, Backend::Accelerator).unwrap(),
            ProcessorPath::Accelerator
        );
    }

    #[test]
    fn test_launch_size_scales_inversely_with_per_matrix_cost() {
        let cap = Duration::from_millis(100);
        let cheap = launch_size(Duration::from_micros(50), cap, 10_000);
        let costly = launch_size(Duration::from_millis(1), cap, 10_000);
        assert_eq!(cheap, 2_000);
        assert_eq!(costly, 100);
        assert!(costly < cheap);
    }

    #[test]
    fn test_accelerator_cost_table_resolves_each_cell_from_its_own_measurement() {
        let mut costs = BTreeMap::new();
        costs.insert((3, 20), Duration::from_micros(40));
        costs.insert((3, 24), Duration::from_micros(770));
        let table = AcceleratorCostTable::new(costs, Duration::from_millis(500));

        let small = table.config_for(3, 20).unwrap();
        let large = table.config_for(3, 24).unwrap();
        assert_eq!(small.per_matrix_cost, Duration::from_micros(40));
        assert_eq!(large.per_matrix_cost, Duration::from_micros(770));

        // The point of a per-cell table: one cap yields different launch sizes
        // at different sizes. A single field-wide cost cannot do this.
        let small_launch = launch_size(small.per_matrix_cost, small.launch_cap, u64::MAX);
        let large_launch = launch_size(large.per_matrix_cost, large.launch_cap, u64::MAX);
        assert!(
            small_launch > large_launch,
            "cheaper cell must take the larger launch: {small_launch} vs {large_launch}"
        );
    }

    #[test]
    fn test_accelerator_cost_table_refuses_a_cell_with_no_measurement() {
        let mut costs = BTreeMap::new();
        costs.insert((3, 20), Duration::from_micros(40));
        let table = AcceleratorCostTable::new(costs, Duration::from_millis(500));

        assert!(table.contains(3, 20));
        assert!(!table.contains(3, 24));
        let error = table.config_for(3, 24).unwrap_err();
        assert!(matches!(
            error,
            ScheduleError::AcceleratorCostMissing { q: 3, n: 24 }
        ));
        let rendered = error.to_string();
        assert!(rendered.contains("q=3"), "{rendered}");
        assert!(rendered.contains("n=24"), "{rendered}");
    }

    #[test]
    fn test_accelerator_cost_table_default_measures_nothing() {
        let table = AcceleratorCostTable::default();
        assert!(matches!(
            table.config_for(3, 20).unwrap_err(),
            ScheduleError::AcceleratorCostMissing { q: 3, n: 20 }
        ));
    }

    #[test]
    fn test_run_field_resolves_each_accelerator_cell_and_preflights_missing_costs() {
        let mut small = cell(3, 2, 1, &[(0, 41)]);
        small.backend = Backend::Accelerator;
        let mut large = cell(3, 3, 1, &[(0, 43)]);
        large.backend = Backend::Accelerator;
        let campaign = manifest(vec![small, large]);
        let launch_cap = Duration::from_millis(500);
        let mut costs = BTreeMap::new();
        costs.insert((3, 2), Duration::from_micros(40));
        costs.insert((3, 3), Duration::from_micros(770));
        let table = AcceleratorCostTable::new(costs, launch_cap);

        let mut resolved = Vec::new();
        let run = run_field_with_accelerator_evaluator(
            &campaign,
            3,
            1,
            &table,
            |root_seed, purpose_tag, item, _worker_count, accelerator| {
                resolved.push(((item.q, item.n), accelerator));
                Ok(synthetic_shard_run(root_seed, purpose_tag, item))
            },
        )
        .unwrap();

        assert_eq!(run.shards().len(), 2);
        assert_eq!(
            resolved,
            vec![
                (
                    (3, 2),
                    Some(AcceleratorConfig {
                        per_matrix_cost: Duration::from_micros(40),
                        launch_cap,
                    })
                ),
                (
                    (3, 3),
                    Some(AcceleratorConfig {
                        per_matrix_cost: Duration::from_micros(770),
                        launch_cap,
                    })
                ),
            ]
        );

        let mut incomplete_costs = BTreeMap::new();
        incomplete_costs.insert((3, 2), Duration::from_micros(40));
        let incomplete = AcceleratorCostTable::new(incomplete_costs, launch_cap);
        let mut evaluation_count = 0;
        let error = run_field_with_accelerator_evaluator(
            &campaign,
            3,
            1,
            &incomplete,
            |root_seed, purpose_tag, item, _worker_count, _accelerator| {
                evaluation_count += 1;
                Ok(synthetic_shard_run(root_seed, purpose_tag, item))
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ScheduleError::AcceleratorCostMissing { q: 3, n: 3 }
        ));
        let rendered = error.to_string();
        assert!(rendered.contains("q=3"), "{rendered}");
        assert!(rendered.contains("n=3"), "{rendered}");
        assert_eq!(evaluation_count, 0, "preflight must precede evaluation");
    }

    #[test]
    fn test_launch_size_never_returns_zero() {
        assert_eq!(
            launch_size(Duration::from_secs(2), Duration::from_millis(1), 10),
            1
        );
    }

    #[test]
    fn test_launch_size_is_capped_by_remaining_matrices() {
        assert_eq!(
            launch_size(Duration::from_micros(1), Duration::from_secs(1), 7),
            7
        );
    }

    #[cfg(feature = "hip")]
    #[test]
    fn test_accelerator_device_absence_halts_named_cell_without_dispatch() {
        let campaign = manifest(vec![cell(3, 2, 4, &[(0, 17)])]);
        let item = enumerate_work_items(&campaign, Some(3)).unwrap().remove(0);
        let mut observed = 0;
        let error = run_shard_for_accelerator_with_dispatch(
            campaign.root_seed,
            CAMPAIGN_CELL_PURPOSE_TAG,
            &item,
            FieldOrder::F3,
            // A measured cost is supplied, so this cell is refused for the
            // reason REQ-04 names — the absent device — and not for a missing
            // cost it does have.
            Some(AcceleratorConfig {
                per_matrix_cost: Duration::from_micros(40),
                launch_cap: Duration::from_millis(500),
            }),
            || false,
            |_matrices, _n| -> Result<(Vec<u64>, Duration), ScheduleError> {
                panic!("accelerator dispatch must not run without a device")
            },
            &mut |_: &[Fp<3>], _: u64, _: Option<u64>| observed += 1,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ScheduleError::AcceleratorDeviceUnavailable {
                q: 3,
                n: 2,
                device: "a usable HIP accelerator device",
            }
        ));
        let rendered = error.to_string();
        assert!(rendered.contains("q=3"));
        assert!(rendered.contains("n=2"));
        assert!(rendered.contains("usable HIP accelerator device"));
        assert_eq!(observed, 0);
    }

    #[cfg(feature = "hip")]
    #[test]
    fn test_accelerator_refuses_a_cell_with_no_measured_cost_supplied() {
        // The default scheduler entry points supply no cost. An accelerator
        // cell reached through them must refuse rather than run against a
        // placeholder: a fabricated per-matrix cost sizes launches from a
        // number nobody measured, which is the defect REQ-01 rules out wearing
        // a measured cost's clothes.
        let campaign = manifest(vec![cell(3, 2, 4, &[(0, 29)])]);
        let item = enumerate_work_items(&campaign, Some(3)).unwrap().remove(0);
        let mut observed = 0;
        let error = run_shard_for_accelerator_with_dispatch(
            campaign.root_seed,
            CAMPAIGN_CELL_PURPOSE_TAG,
            &item,
            FieldOrder::F3,
            None,
            || true,
            |_matrices, _n| -> Result<(Vec<u64>, Duration), ScheduleError> {
                panic!("dispatch must not run without a measured per-matrix cost")
            },
            &mut |_: &[Fp<3>], _: u64, _: Option<u64>| observed += 1,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ScheduleError::AcceleratorCostMissing { q: 3, n: 2 }
        ));
        assert_eq!(observed, 0, "no matrix may be evaluated");
    }

    #[cfg(feature = "hip")]
    #[test]
    fn test_accelerator_launch_split_preserves_input_order_and_values() {
        let campaign = manifest(vec![cell(3, 2, 9, &[(0, 23)])]);
        let item = enumerate_work_items(&campaign, Some(3)).unwrap().remove(0);
        let mut first_values = Vec::new();
        let mut first_launches = 0;
        let first = run_shard_for_accelerator_with_dispatch(
            campaign.root_seed,
            CAMPAIGN_CELL_PURPOSE_TAG,
            &item,
            FieldOrder::F3,
            Some(AcceleratorConfig {
                per_matrix_cost: Duration::from_millis(1),
                launch_cap: Duration::from_millis(2),
            }),
            || true,
            |matrices, n| {
                first_launches += 1;
                Ok((
                    matrices
                        .iter()
                        .map(|entries| permanent_ryser(entries, n).value())
                        .collect(),
                    Duration::ZERO,
                ))
            },
            &mut |_: &[Fp<3>], value, _: Option<u64>| first_values.push(value),
        )
        .unwrap();

        let mut second_values = Vec::new();
        let mut second_launches = 0;
        let second = run_shard_for_accelerator_with_dispatch(
            campaign.root_seed,
            CAMPAIGN_CELL_PURPOSE_TAG,
            &item,
            FieldOrder::F3,
            Some(AcceleratorConfig {
                per_matrix_cost: Duration::from_millis(1),
                launch_cap: Duration::from_millis(3),
            }),
            || true,
            |matrices, n| {
                second_launches += 1;
                Ok((
                    matrices
                        .iter()
                        .map(|entries| permanent_ryser(entries, n).value())
                        .collect(),
                    Duration::ZERO,
                ))
            },
            &mut |_: &[Fp<3>], value, _: Option<u64>| second_values.push(value),
        )
        .unwrap();

        assert_eq!(first_launches, 5);
        assert_eq!(second_launches, 3);
        assert_eq!(first_values, second_values);
        assert_eq!(first.run.record, second.run.record);
    }

    #[test]
    fn test_backend_name_agrees_with_serialized_token() {
        for backend in [
            Backend::Scalar,
            Backend::BatchParallel,
            Backend::IntraMatrixParallel,
            Backend::GenericRyser,
            Backend::Accelerator,
        ] {
            let serialized = serde_json::to_value(backend).unwrap();
            assert_eq!(serialized.as_str(), Some(backend.name()));
        }
    }

    fn manifest(cells: Vec<CellSpec>) -> CampaignManifest {
        CampaignManifest {
            schema_version: SCHEMA_VERSION,
            campaign_id: "campaign-test".parse().unwrap(),
            root_seed: 0x1234,
            stream_purposes: vec![StreamPurpose {
                name: "campaign-cells".parse().unwrap(),
                tag: CAMPAIGN_CELL_PURPOSE_TAG,
            }],
            cells,
            provenance: Provenance {
                git_revision: "95ccd9776376b2b060e0dd40785e2effae29e766"
                    .parse::<GitRevision>()
                    .unwrap(),
                compiler_version: "rustc test".to_owned(),
                rng_algorithm: RngAlgorithm::ChaCha20,
                rng_version: "rand_chacha 0.9".to_owned(),
                invocation: vec!["permanent_campaign".to_owned()],
                accelerator_runtime: Availability::NotPresent,
                cpu_model: "test".to_owned(),
                gpu_model: Availability::NotPresent,
            },
        }
    }

    #[test]
    fn determinant_fixture_manifest_selects_the_companion() {
        let campaign = crate::permanent_campaign::fixture::manifest_with_determinant_companion();
        let work = enumerate_work_items(&campaign, Some(3)).unwrap();
        assert!(work
            .iter()
            .all(|item| item.determinant_companion == DeterminantPlan::Evaluate));
    }

    fn cell(q: u8, n: u16, matrix_count: u64, shards: &[(u64, u64)]) -> CellSpec {
        CellSpec {
            q,
            n,
            matrix_count,
            shard_size: shards.iter().map(|(_, _)| matrix_count).max().unwrap_or(1),
            shards: shards
                .iter()
                .map(|&(shard_id, stream_index)| ShardSpec {
                    shard_id,
                    stream_index,
                })
                .collect(),
            backend: Backend::GenericRyser,
            backend_receipt: ArtifactIdentity {
                path: "dev/benchmarks/permanent/test-receipt.json"
                    .parse()
                    .unwrap(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .parse()
                    .unwrap(),
            },
            determinant_companion: DeterminantPlan::NotEvaluated,
        }
    }

    fn synthetic_shard_run(root_seed: u64, purpose_tag: u8, item: &WorkItem) -> ShardRun {
        let mut permanent_histogram = vec![0; usize::from(item.q)];
        permanent_histogram[1] = item.matrix_count;
        ShardRun {
            record: ShardRecord {
                schema_version: SCHEMA_VERSION,
                shard_id: item.shard_id,
                stream_address: StreamAddress {
                    root_seed,
                    q: item.q,
                    n: item.n,
                    purpose_tag,
                    stream_index: item.stream_index,
                },
                matrix_count: item.matrix_count,
                permanent_zero_count: 0,
                permanent_histogram,
                determinant: DeterminantCount::NotEvaluated,
            },
            timing: PhaseDurations {
                draw: Duration::ZERO,
                pack: Duration::ZERO,
                evaluate: Duration::ZERO,
                determinant: Duration::ZERO,
                count: Duration::ZERO,
            },
        }
    }

    #[test]
    fn test_work_item_backend_is_copied_from_the_manifest_cell() {
        let mut scalar_cell = cell(3, 2, 1, &[(0, 20)]);
        scalar_cell.backend = Backend::Scalar;
        let mut batch_cell = cell(5, 2, 1, &[(1, 21)]);
        batch_cell.backend = Backend::BatchParallel;
        let campaign = manifest(vec![scalar_cell, batch_cell]);
        let items = enumerate_work_items(&campaign, None).unwrap();
        let manifest_backends: Vec<_> = campaign.cells.iter().map(|cell| cell.backend).collect();

        assert_eq!(items.len(), 2);
        for item in &items {
            let cell = campaign
                .cells
                .iter()
                .find(|cell| cell.q == item.q && cell.n == item.n)
                .unwrap();
            assert_eq!(item.backend, cell.backend);
            assert!(manifest_backends.contains(&item.backend));
        }
    }

    #[test]
    fn test_run_shard_evaluates_generic_ryser_above_the_f7_ceiling() {
        for n in 17..=18 {
            let mut campaign = manifest(vec![cell(7, n, 4, &[(0, 23)])]);
            campaign.cells[0].backend = Backend::GenericRyser;
            let item = enumerate_work_items(&campaign, Some(7)).unwrap().remove(0);
            let mut observed = Vec::new();
            let evaluated = run_shard_for_with_observer(
                campaign.root_seed,
                CAMPAIGN_CELL_PURPOSE_TAG,
                &item,
                FieldOrder::F7,
                &mut |entries: &[Fp<7>], permanent, _| {
                    observed.push((entries.to_vec(), permanent));
                },
            )
            .unwrap();

            assert_eq!(observed.len(), 4);
            assert_eq!(evaluated.run.timing.pack, Duration::ZERO);
            for (entries, permanent) in observed {
                assert_eq!(permanent, permanent_ryser(&entries, usize::from(n)).value());
            }
        }
    }

    #[test]
    fn work_item_enumeration_is_ordered_and_field_filtered() {
        let manifest = manifest(vec![
            cell(5, 1, 1, &[(0, 20)]),
            cell(3, 2, 2, &[(1, 11), (0, 10)]),
        ]);
        let all = enumerate_work_items(&manifest, None).unwrap();
        assert_eq!(
            all.iter().map(WorkItem::key).collect::<Vec<_>>(),
            vec![(3, 2, 0), (3, 2, 1), (5, 1, 0)]
        );
        let only_q3 = enumerate_work_items(&manifest, Some(3)).unwrap();
        assert_eq!(only_q3.len(), 2);
        assert!(only_q3.iter().all(|item| item.q == 3));
    }

    #[test]
    fn composite_loop_matches_direct_oracle_and_records_timings() {
        let campaign = manifest(vec![cell(3, 2, 4, &[(0, 7)])]);
        let mut campaign = campaign;
        campaign.cells[0].determinant_companion = DeterminantPlan::Evaluate;
        let run = run_field(&campaign, 3).unwrap();
        let shard = &run.shards()[0];
        let address = MatrixAddress::new(
            campaign.root_seed,
            FieldOrder::F3,
            2,
            SamplerPurpose::CampaignCell,
            StreamIndex::new(7).unwrap(),
        );
        let mut sampler = MatrixSampler::<3>::new(address).unwrap();
        let mut expected = vec![0_u64; 3];
        let mut expected_determinant_zero_count = 0_u64;
        let mut entries = vec![Fp::<3>::new(0); 4];
        for _ in 0..4 {
            sampler.fill_next_matrix(&mut entries);
            expected[permanent_ryser(&entries, 2).value() as usize] += 1;
            let rows = entries
                .chunks(2)
                .map(|row| FieldVec::from(row.to_vec()))
                .collect();
            let matrix = FieldMatrix::from_rows(rows);
            if matrix.det() == Fp::<3>::new(0) {
                expected_determinant_zero_count += 1;
            }
        }
        assert_eq!(shard.record.permanent_histogram, expected);
        assert_eq!(shard.record.permanent_zero_count, expected[0]);
        assert_eq!(
            shard.record.determinant,
            DeterminantCount::Evaluated {
                sample_count: 4,
                zero_count: expected_determinant_zero_count,
            }
        );
        assert!(shard.timing.draw >= Duration::ZERO);
        assert!(shard.timing.pack >= Duration::ZERO);
        assert!(shard.timing.evaluate >= Duration::ZERO);
        assert!(shard.timing.count >= Duration::ZERO);
    }

    #[test]
    fn companion_observer_sees_same_one_pass_operands_for_both_values() {
        let mut campaign = manifest(vec![cell(3, 2, 8, &[(0, 17)])]);
        campaign.cells[0].backend = Backend::BatchParallel;
        campaign.cells[0].determinant_companion = DeterminantPlan::Evaluate;
        let run = run_field_with_worker_count(&campaign, 3, 4).unwrap();

        let item = enumerate_work_items(&campaign, Some(3)).unwrap().remove(0);
        let mut observed = Vec::new();
        let observed_run = run_shard_for_with_observer(
            campaign.root_seed,
            CAMPAIGN_CELL_PURPOSE_TAG,
            &item,
            FieldOrder::F3,
            &mut |entries: &[Fp<3>], permanent, determinant| {
                observed.push((
                    entries
                        .iter()
                        .map(|entry| entry.value())
                        .collect::<Vec<_>>(),
                    permanent,
                    determinant,
                ));
            },
        )
        .unwrap();

        let address = MatrixAddress::new(
            campaign.root_seed,
            FieldOrder::F3,
            2,
            SamplerPurpose::CampaignCell,
            StreamIndex::new(17).unwrap(),
        );
        let mut sampler = MatrixSampler::<3>::new(address).unwrap();
        let mut entries = vec![Fp::<3>::new(0); 4];
        let mut permanent_values = Vec::new();
        let mut determinant_values = Vec::new();
        for (observed_entries, observed_permanent, observed_determinant) in &observed {
            sampler.fill_next_matrix(&mut entries);
            permanent_values.push(permanent_ryser(&entries, 2).value());
            let observed_matrix: Vec<Fp<3>> =
                observed_entries.iter().copied().map(Fp::<3>::new).collect();
            let rows = observed_matrix
                .chunks(2)
                .map(|row| FieldVec::from(row.to_vec()))
                .collect();
            let determinant = FieldMatrix::from_rows(rows).det().value();
            determinant_values.push(determinant);
            assert_eq!(
                observed_entries,
                &entries
                    .iter()
                    .map(|entry| entry.value())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                *observed_permanent,
                permanent_ryser(&observed_matrix, 2).value()
            );
            assert_eq!(*observed_determinant, Some(determinant));
        }

        assert_eq!(observed.len(), 8);
        assert_eq!(observed_run.run.record, run.shards()[0].record);
        let shard = &run.shards()[0];
        assert_eq!(
            shard.record.permanent_zero_count,
            permanent_values.iter().filter(|&&value| value == 0).count() as u64
        );
        assert_eq!(
            shard.record.determinant,
            DeterminantCount::Evaluated {
                sample_count: 8,
                zero_count: determinant_values
                    .iter()
                    .filter(|&&value| value == 0)
                    .count() as u64,
            }
        );
    }

    fn assert_batch_results_match_ryser<const Q: u64>(q: u8) {
        let mut campaign = manifest(vec![cell(q, 2, 9, &[(0, 23)])]);
        campaign.cells[0].backend = Backend::BatchParallel;
        let item = enumerate_work_items(&campaign, Some(q)).unwrap().remove(0);
        let field_order = match q {
            3 => FieldOrder::F3,
            5 => FieldOrder::F5,
            7 => FieldOrder::F7,
            _ => unreachable!(),
        };
        let mut observed = Vec::new();
        run_shard_for_with_observer_with_worker_count(
            campaign.root_seed,
            CAMPAIGN_CELL_PURPOSE_TAG,
            &item,
            field_order,
            4,
            None,
            &mut |entries: &[Fp<Q>], permanent, _| {
                observed.push((entries.to_vec(), permanent));
            },
        )
        .unwrap();

        let address = MatrixAddress::new(
            campaign.root_seed,
            field_order,
            2,
            SamplerPurpose::CampaignCell,
            StreamIndex::new(23).unwrap(),
        );
        let mut sampler = MatrixSampler::<Q>::new(address).unwrap();
        let mut entries = vec![Fp::<Q>::new(0); 4];
        for (observed_entries, observed_permanent) in observed {
            sampler.fill_next_matrix(&mut entries);
            assert_eq!(observed_entries, entries);
            assert_eq!(observed_permanent, permanent_ryser(&entries, 2).value());
        }
    }

    #[test]
    fn batch_results_match_ryser_in_input_order_for_each_supported_field() {
        assert_batch_results_match_ryser::<3>(3);
        assert_batch_results_match_ryser::<5>(5);
        assert_batch_results_match_ryser::<7>(7);
    }

    // This seed and stream identify the common matrix set used to compare each
    // selectable backend within a field. Keep them fixed so a discrepancy is
    // reproducible from this test alone.
    const BACKEND_CONFORMANCE_ROOT_SEED: u64 = 0xC0DE_1947_125E_5EED;
    const BACKEND_CONFORMANCE_STREAM_INDEX: u64 = 47;
    const BACKEND_CONFORMANCE_MATRIX_COUNT: u64 = 12;
    const BACKEND_CONFORMANCE_DIMENSION: u16 = 4;

    fn assert_backend_conformance<const Q: u64>(
        q: u8,
        field_order: FieldOrder,
        backends: &[Backend],
    ) {
        let mut reference_sampler = MatrixSampler::<Q>::new(MatrixAddress::new(
            BACKEND_CONFORMANCE_ROOT_SEED,
            field_order,
            usize::from(BACKEND_CONFORMANCE_DIMENSION),
            SamplerPurpose::CampaignCell,
            StreamIndex::new(BACKEND_CONFORMANCE_STREAM_INDEX).unwrap(),
        ))
        .unwrap();
        let mut entries = vec![Fp::<Q>::new(0); usize::from(BACKEND_CONFORMANCE_DIMENSION).pow(2)];
        let reference: Vec<_> = (0..BACKEND_CONFORMANCE_MATRIX_COUNT)
            .map(|_| {
                reference_sampler.fill_next_matrix(&mut entries);
                (
                    entries.clone(),
                    permanent_ryser(&entries, usize::from(BACKEND_CONFORMANCE_DIMENSION)).value(),
                )
            })
            .collect();

        for &backend in backends {
            let mut campaign = manifest(vec![cell(
                q,
                BACKEND_CONFORMANCE_DIMENSION,
                BACKEND_CONFORMANCE_MATRIX_COUNT,
                &[(0, BACKEND_CONFORMANCE_STREAM_INDEX)],
            )]);
            campaign.root_seed = BACKEND_CONFORMANCE_ROOT_SEED;
            campaign.cells[0].backend = backend;
            let item = enumerate_work_items(&campaign, Some(q)).unwrap().remove(0);
            let mut matrix_index = 0;

            run_shard_for_with_observer_with_worker_count(
                campaign.root_seed,
                CAMPAIGN_CELL_PURPOSE_TAG,
                &item,
                field_order,
                4,
                #[cfg(feature = "hip")]
                (backend == Backend::Accelerator).then_some(AcceleratorConfig {
                    per_matrix_cost: Duration::ZERO,
                    launch_cap: DEFAULT_ACCELERATOR_LAUNCH_CAP,
                }),
                #[cfg(not(feature = "hip"))]
                None,
                &mut |observed_entries: &[Fp<Q>], observed_permanent, _| {
                    let (reference_entries, reference_permanent) = &reference[matrix_index];
                    assert_eq!(
                        observed_entries,
                        reference_entries,
                        "backend {} changed the common q={q} matrix set at index {matrix_index}",
                        backend.name()
                    );
                    assert_eq!(
                        observed_permanent,
                        *reference_permanent,
                        "backend {} disagreed with generic Ryser at q={q}, matrix {matrix_index}",
                        backend.name()
                    );
                    matrix_index += 1;
                },
            )
            .unwrap();
            assert_eq!(
                matrix_index,
                BACKEND_CONFORMANCE_MATRIX_COUNT as usize,
                "backend {} did not evaluate every common q={q} matrix",
                backend.name()
            );
        }
    }

    #[test]
    fn campaign_selectable_backends_match_generic_ryser_per_matrix() {
        assert_backend_conformance::<3>(
            3,
            FieldOrder::F3,
            &[
                Backend::Scalar,
                Backend::BatchParallel,
                Backend::IntraMatrixParallel,
                Backend::GenericRyser,
            ],
        );
        assert_backend_conformance::<5>(
            5,
            FieldOrder::F5,
            &[
                Backend::Scalar,
                Backend::BatchParallel,
                Backend::GenericRyser,
            ],
        );
        assert_backend_conformance::<7>(
            7,
            FieldOrder::F7,
            &[
                Backend::Scalar,
                Backend::BatchParallel,
                Backend::GenericRyser,
            ],
        );

        #[cfg(feature = "hip")]
        if gf2_algebra::gpu::has_usable_device() {
            assert_backend_conformance::<3>(3, FieldOrder::F3, &[Backend::Accelerator]);
            assert_backend_conformance::<5>(5, FieldOrder::F5, &[Backend::Accelerator]);
            assert_backend_conformance::<7>(7, FieldOrder::F7, &[Backend::Accelerator]);
        } else {
            eprintln!("skipping accelerator backend conformance: no usable HIP accelerator device");
        }
    }

    #[test]
    fn batch_records_are_identical_across_configured_thread_counts() {
        for (q, stream_index) in [(3, 29), (5, 31), (7, 37)] {
            let mut campaign = manifest(vec![cell(q, 2, 12, &[(0, stream_index)])]);
            campaign.cells[0].backend = Backend::BatchParallel;
            let one = run_field_with_worker_count(&campaign, q, 1).unwrap();
            let four = run_field_with_worker_count(&campaign, q, 4).unwrap();
            assert_eq!(one.shards()[0].record, four.shards()[0].record);
            assert_eq!(
                shard_record_bytes(&one.shards()[0].record).unwrap(),
                shard_record_bytes(&four.shards()[0].record).unwrap()
            );

            let mut scalar = campaign.clone();
            scalar.cells[0].backend = Backend::Scalar;
            let scalar_run = run_field(&scalar, q).unwrap();
            assert_eq!(
                scalar_run.shards()[0].record.permanent_histogram,
                one.shards()[0].record.permanent_histogram
            );
            assert_eq!(
                scalar_run.shards()[0].record.permanent_zero_count,
                one.shards()[0].record.permanent_zero_count
            );

            if q == 3 {
                let scalar_item = enumerate_work_items(&scalar, Some(q)).unwrap().remove(0);
                let batch_item = enumerate_work_items(&campaign, Some(q)).unwrap().remove(0);
                let mut scalar_values = Vec::new();
                let mut batch_values = Vec::new();
                run_shard_for_with_observer_with_worker_count(
                    scalar.root_seed,
                    CAMPAIGN_CELL_PURPOSE_TAG,
                    &scalar_item,
                    FieldOrder::F3,
                    1,
                    None,
                    &mut |_: &[Fp<3>], value, _| scalar_values.push(value),
                )
                .unwrap();
                run_shard_for_with_observer_with_worker_count(
                    campaign.root_seed,
                    CAMPAIGN_CELL_PURPOSE_TAG,
                    &batch_item,
                    FieldOrder::F3,
                    4,
                    None,
                    &mut |_: &[Fp<3>], value, _| batch_values.push(value),
                )
                .unwrap();
                assert_eq!(scalar_values, batch_values);
            }
        }
    }

    #[test]
    fn batch_determinant_counts_match_scalar_on_the_same_address() {
        let mut scalar = manifest(vec![cell(5, 2, 10, &[(0, 41)])]);
        scalar.cells[0].determinant_companion = DeterminantPlan::Evaluate;
        let mut batch = scalar.clone();
        scalar.cells[0].backend = Backend::Scalar;
        batch.cells[0].backend = Backend::BatchParallel;

        let scalar_run = run_field_with_worker_count(&scalar, 5, 1).unwrap();
        let batch_run = run_field_with_worker_count(&batch, 5, 4).unwrap();
        assert_eq!(
            scalar_run.shards()[0].record.determinant,
            batch_run.shards()[0].record.determinant
        );
        assert_eq!(
            scalar_run.shards()[0].record.permanent_histogram,
            batch_run.shards()[0].record.permanent_histogram
        );
    }

    #[test]
    fn exhaustive_small_field_determinant_count_matches_external_recomputation() {
        let mut campaign = manifest(vec![cell(3, 2, 81, &[(0, 31)])]);
        campaign.cells[0].determinant_companion = DeterminantPlan::Evaluate;
        let run = run_field(&campaign, 3).unwrap();

        let address = MatrixAddress::new(
            campaign.root_seed,
            FieldOrder::F3,
            2,
            SamplerPurpose::CampaignCell,
            StreamIndex::new(31).unwrap(),
        );
        let mut sampler = MatrixSampler::<3>::new(address).unwrap();
        let mut entries = vec![Fp::<3>::new(0); 4];
        let mut expected_zero_count = 0_u64;
        for _ in 0..81 {
            sampler.fill_next_matrix(&mut entries);
            let rows = entries
                .chunks(2)
                .map(|row| FieldVec::from(row.to_vec()))
                .collect();
            if FieldMatrix::from_rows(rows).det() == Fp::<3>::new(0) {
                expected_zero_count += 1;
            }
        }

        assert_eq!(
            run.shards()[0].record.determinant,
            DeterminantCount::Evaluated {
                sample_count: 81,
                zero_count: expected_zero_count,
            }
        );
    }

    #[test]
    fn companion_does_not_change_permanent_counts() {
        let disabled = manifest(vec![cell(3, 2, 8, &[(0, 43)])]);
        let mut enabled = disabled.clone();
        enabled.cells[0].determinant_companion = DeterminantPlan::Evaluate;
        let disabled_run = run_field(&disabled, 3).unwrap();
        let enabled_run = run_field(&enabled, 3).unwrap();

        assert_eq!(
            disabled_run.shards()[0].record.permanent_zero_count,
            enabled_run.shards()[0].record.permanent_zero_count
        );
        assert_eq!(
            disabled_run.shards()[0].record.permanent_histogram,
            enabled_run.shards()[0].record.permanent_histogram
        );
    }

    #[test]
    fn not_evaluated_plan_is_explicit_in_shard_and_summary() {
        let campaign = manifest(vec![cell(3, 2, 2, &[(0, 47)])]);
        let run = run_field(&campaign, 3).unwrap();

        assert_eq!(
            run.shards()[0].record.determinant,
            DeterminantCount::NotEvaluated
        );
        assert_eq!(
            run.summary().rows[0].determinant,
            DeterminantCount::NotEvaluated
        );
    }

    #[test]
    fn determinant_counts_pool_across_shards_in_one_cell() {
        let mut campaign = manifest(vec![cell(3, 2, 6, &[(0, 53), (1, 54)])]);
        campaign.cells[0].shard_size = 3;
        campaign.cells[0].determinant_companion = DeterminantPlan::Evaluate;
        let run = run_field(&campaign, 3).unwrap();

        let shard_counts: Vec<_> = run
            .shards()
            .iter()
            .map(|shard| shard.record.determinant.clone())
            .collect();
        let (sample_count, zero_count) =
            shard_counts
                .into_iter()
                .fold((0, 0), |acc, count| match count {
                    DeterminantCount::Evaluated {
                        sample_count,
                        zero_count,
                    } => (acc.0 + sample_count, acc.1 + zero_count),
                    DeterminantCount::NotEvaluated => {
                        panic!("evaluated plan produced no determinant")
                    }
                });
        assert_eq!(
            run.summary().rows[0].determinant,
            DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            }
        );
    }

    #[test]
    fn enabled_completed_summary_round_trips_through_canonical_reader() {
        let mut campaign = manifest(vec![cell(3, 2, 8, &[(0, 61)])]);
        campaign.cells[0].determinant_companion = DeterminantPlan::Evaluate;
        let run = run_field(&campaign, 3).unwrap();
        let parent = std::env::temp_dir().join(format!(
            "campaign-summary-round-trip-{}",
            std::process::id()
        ));
        let root = parent.join("campaign-test");

        emit_field(&root, &campaign, &run).unwrap();
        let read_back = crate::permanent_campaign::schema::read_field_summary(&root, 3).unwrap();

        assert_eq!(read_back, *run.summary());
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn determinant_verdict_uses_the_protocol_probability_ordered_test() {
        let q = 3;
        let n = 2;
        let sample_count = 100;
        let family_test_count = 63;
        let level = bonferroni_level(DETERMINANT_FAMILYWISE_ERROR, family_test_count);
        let null_probability = determinant_null_probability(q, n);

        for (zero_count, expected_verdict) in [
            (41, AcceptanceVerdict::Accepted),
            (0, AcceptanceVerdict::Rejected),
            (sample_count, AcceptanceVerdict::Rejected),
        ] {
            let expected_rejection =
                two_sided_test(zero_count, sample_count, null_probability).rejects_at(level);
            assert_eq!(
                expected_rejection,
                matches!(expected_verdict, AcceptanceVerdict::Rejected)
            );
            assert_eq!(
                determinant_acceptance(q, n, zero_count, sample_count, family_test_count),
                expected_verdict
            );
        }
    }

    #[test]
    fn permanent_floor_rejection_is_the_preregistered_exact_test() {
        let level = bonferroni_level(0.025, 1);
        let rejected = permanent_zero_floor_test(0, 11, 3);
        let accepted = permanent_zero_floor_test(4, 11, 3);

        assert!(rejected.rejects_at(level));
        assert!(!accepted.rejects_at(level));
        assert_eq!(
            permanent_acceptance(3, 0, 11, 1),
            AcceptanceVerdict::Rejected
        );
        assert_eq!(
            permanent_acceptance(3, 4, 11, 1),
            AcceptanceVerdict::Accepted
        );
    }

    #[test]
    fn summary_verdict_matches_the_pooled_permanent_floor_decision() {
        let campaign = manifest(vec![cell(3, 2, 4, &[(0, 7)])]);
        let family_test_count = campaign.cells.len() as u64;
        let run = run_field(&campaign, 3).unwrap();
        let row = &run.summary().rows[0];
        let CellTerminalState::Completed {
            permanent_verdict, ..
        } = row.terminal_state
        else {
            panic!("small completed run must produce a completed summary row");
        };

        assert_eq!(
            permanent_verdict,
            permanent_acceptance(
                row.q,
                row.permanent_zero_count,
                row.matrix_count,
                family_test_count,
            )
        );
    }

    #[test]
    fn emission_paths_are_disjoint_for_single_field_invocations() {
        let campaign = manifest(vec![cell(3, 2, 1, &[(0, 10)]), cell(5, 2, 1, &[(0, 20)])]);
        let q3 = run_field(&campaign, 3).unwrap();
        let q5 = run_field(&campaign, 5).unwrap();
        let parent = std::env::temp_dir().join(format!("gf2-campaign-test-{}", std::process::id()));
        let root = parent.join("campaign-test");
        let q3_paths = emit_field(&root, &campaign, &q3).unwrap();
        let q5_paths = emit_field(&root, &campaign, &q5).unwrap();
        let q3_set: BTreeSet<_> = q3_paths.iter().collect();
        let q5_set: BTreeSet<_> = q5_paths.iter().collect();
        assert!(q3_set.is_disjoint(&q5_set));
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn emitted_records_parse_and_are_under_the_campaign_id() {
        let campaign = manifest(vec![cell(3, 2, 1, &[(0, 10)])]);
        let run = run_field(&campaign, 3).unwrap();
        assert_eq!(run.q(), 3);
        assert_eq!(run.summary().q, 3);
        let parent = std::env::temp_dir().join(format!("campaign-parse-{}", std::process::id()));
        let root = parent.join("campaign-test");
        let paths = emit_field(&root, &campaign, &run).unwrap();
        let campaign_name = campaign.campaign_id.to_string();
        assert_eq!(
            root.file_name().unwrap(),
            std::ffi::OsStr::new(&campaign_name)
        );
        let record: ShardRecord = serde_json::from_slice(&fs::read(&paths[0]).unwrap()).unwrap();
        let summary: FieldSummary =
            serde_json::from_slice(&fs::read(paths.last().unwrap()).unwrap()).unwrap();
        assert_eq!(record.stream_address.q, 3);
        assert_eq!(summary.q, 3);
        assert_eq!(
            crate::permanent_campaign::schema::read_field_summary(&root, 3)
                .unwrap()
                .q,
            3
        );
        assert!(paths.iter().all(|path| path.starts_with(&root)));
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn identical_runs_emit_byte_identical_files() {
        let campaign = manifest(vec![cell(3, 2, 2, &[(0, 10)])]);
        let first = run_field(&campaign, 3).unwrap();
        let second = run_field(&campaign, 3).unwrap();
        let left_parent =
            std::env::temp_dir().join(format!("campaign-left-{}", std::process::id()));
        let right_parent =
            std::env::temp_dir().join(format!("campaign-right-{}", std::process::id()));
        let left = left_parent.join("campaign-test");
        let right = right_parent.join("campaign-test");
        let left_paths = emit_field(&left, &campaign, &first).unwrap();
        let right_paths = emit_field(&right, &campaign, &second).unwrap();
        assert_eq!(left_paths.len(), right_paths.len());
        for (left_path, right_path) in left_paths.iter().zip(right_paths.iter()) {
            assert_eq!(fs::read(left_path).unwrap(), fs::read(right_path).unwrap());
        }
        let _ = fs::remove_dir_all(left_parent);
        let _ = fs::remove_dir_all(right_parent);
    }

    #[test]
    fn re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission() {
        let campaign = manifest(vec![cell(3, 2, 1, &[(0, 10)])]);
        let run = run_field(&campaign, 3).unwrap();
        let parent = std::env::temp_dir().join(format!("campaign-reemit-{}", std::process::id()));
        let root = parent.join("campaign-test");
        let paths = emit_field(&root, &campaign, &run).unwrap();
        let first_bytes: Vec<_> = paths
            .iter()
            .map(fs::read)
            .collect::<Result<_, _>>()
            .unwrap();

        let refusal = emit_field(&root, &campaign, &run)
            .expect_err("re-emitting into an existing dataset must refuse");
        assert!(refusal
            .to_string()
            .contains(paths[0].to_string_lossy().as_ref()));
        for (path, bytes) in paths.iter().zip(first_bytes) {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        let _ = fs::remove_dir_all(parent);
    }
}
