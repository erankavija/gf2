//! Deterministic scheduling and field-shard emission for permanent campaigns.
//!
//! A campaign invocation owns one field arm. Work is ordered by `(q, n,
//! shard_id)`, each shard opens the stream address recorded by its manifest,
//! and every matrix passes through draw, pack, evaluate, and count phases.
//! Timings remain in [`ShardRun`] for progress reporting; only schema records
//! and summaries are written to disk, so wall-clock variation cannot alter
//! emitted bytes.
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

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
use gf2_algebra::permanent::{
    permanent_bipedal3, permanent_bipedal5, permanent_bipedal7, permanent_ryser,
};
use gf2_core::gfp::Fp;
use gf2_stats::binomial::{bonferroni_level, permanent_zero_floor_test};
use gf2_stats::sampler::{
    FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose as SamplerPurpose,
};

use super::schema::{
    field_summary_file, shard_record_file, AcceptanceVerdict, Backend, CampaignManifest, CellSpec,
    CellTerminalState, DeterminantCount, DeterminantEstimate, DeterminantPlan, FieldSummary,
    Interval, ProportionEstimate, ShardRecord, ShardSpec, StreamAddress, SummaryRow,
    SCHEMA_VERSION,
};

/// The purpose tag reserved for published campaign-cell matrix streams.
pub const CAMPAIGN_CELL_PURPOSE_TAG: u8 = SamplerPurpose::CampaignCell as u8;

/// Family-wise error budget for the permanent-floor tests, as preregistered
/// in `dev/simulation_results/permanent-zero-fraction/protocol.md` under
/// "Error budgets" and "Permanent-floor decision".
const PERMANENT_FAMILYWISE_ERROR: f64 = 0.025;

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
    /// Time spent constructing the packed representation.
    pub pack: Duration,
    /// Time spent evaluating permanents.
    pub evaluate: Duration,
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

/// Completed execution for one field arm.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldRun {
    q: u8,
    shards: Vec<ShardRun>,
    summary: FieldSummary,
}

impl FieldRun {
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
    /// This scheduler does not execute determinant companions.
    DeterminantCompanionRequested {
        /// Prime field order of the cell.
        q: u8,
        /// Matrix dimension of the cell.
        n: u16,
    },
    /// A manifest value cannot be represented by the execution API.
    InvalidWorkItem(String),
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
            Self::DeterminantCompanionRequested { q, n } => write!(
                formatter,
                "determinant companion is not part of the scheduler for q={q}, n={n}"
            ),
            Self::InvalidWorkItem(message) => formatter.write_str(message),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Serialization(source) => source.fmt(formatter),
        }
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
/// kernel's cost per matrix and the sampler/packing storage is `O(n²)`.
pub fn run_field(manifest: &CampaignManifest, field: u8) -> Result<FieldRun, ScheduleError> {
    let purpose = manifest
        .stream_purposes
        .iter()
        .find(|purpose| purpose.tag == CAMPAIGN_CELL_PURPOSE_TAG)
        .ok_or(ScheduleError::MissingCampaignPurpose)?;
    let items = enumerate_work_items(manifest, Some(field))?;
    let mut shards = Vec::with_capacity(items.len());
    for item in &items {
        if item.determinant_companion != DeterminantPlan::NotEvaluated {
            return Err(ScheduleError::DeterminantCompanionRequested {
                q: item.q,
                n: item.n,
            });
        }
        shards.push(run_shard(manifest.root_seed, purpose.tag, item)?);
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

fn run_shard(root_seed: u64, purpose_tag: u8, item: &WorkItem) -> Result<ShardRun, ScheduleError> {
    match item.q {
        3 => run_shard_for::<3>(root_seed, purpose_tag, item, FieldOrder::F3),
        5 => run_shard_for::<5>(root_seed, purpose_tag, item, FieldOrder::F5),
        7 => run_shard_for::<7>(root_seed, purpose_tag, item, FieldOrder::F7),
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
) -> Result<ShardRun, ScheduleError> {
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
    let mut count = Duration::ZERO;

    for _ in 0..item.matrix_count {
        let started = Instant::now();
        sampler.fill_next_matrix(&mut row_major);
        draw += started.elapsed();

        let started = Instant::now();
        let packed = PackedMatrix::new(&row_major, n);
        pack += started.elapsed();

        let started = Instant::now();
        let value = evaluate_permanent(item.backend, &row_major, &packed, n)?;
        evaluate += started.elapsed();

        let started = Instant::now();
        histogram[value as usize] += 1;
        if value == 0 {
            permanent_zero_count += 1;
        }
        count += started.elapsed();
    }

    Ok(ShardRun {
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
            determinant: DeterminantCount::NotEvaluated,
        },
        timing: PhaseDurations {
            draw,
            pack,
            evaluate,
            count,
        },
    })
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
    backend: Backend,
    row_major: &[Fp<Q>],
    packed: &PackedMatrix,
    n: usize,
) -> Result<u64, ScheduleError> {
    let value = match (Q, backend, packed) {
        (3, Backend::Scalar, PackedMatrix::F3(matrix))
        | (3, Backend::BatchParallel, PackedMatrix::F3(matrix))
        | (3, Backend::IntraMatrixParallel, PackedMatrix::F3(matrix)) => {
            permanent_bipedal3(matrix).value()
        }
        (5, Backend::Scalar, PackedMatrix::F5(matrix))
        | (5, Backend::BatchParallel, PackedMatrix::F5(matrix))
        | (5, Backend::IntraMatrixParallel, PackedMatrix::F5(matrix)) => {
            permanent_bipedal5(matrix).value()
        }
        (7, Backend::Scalar, PackedMatrix::F7(matrix))
        | (7, Backend::BatchParallel, PackedMatrix::F7(matrix))
        | (7, Backend::IntraMatrixParallel, PackedMatrix::F7(matrix)) => {
            permanent_bipedal7(matrix).value()
        }
        (_, Backend::GenericRyser, _) => permanent_ryser(row_major, n).value(),
        (_, Backend::Accelerator, _) => {
            return Err(ScheduleError::InvalidWorkItem(
                "accelerator backend is layered above the scheduler".to_owned(),
            ));
        }
        _ => {
            return Err(ScheduleError::InvalidWorkItem(
                "packed matrix and field order do not agree".to_owned(),
            ));
        }
    };
    Ok(value)
}

fn summarize(q: u8, shards: &[ShardRun], family_test_count: u64) -> FieldSummary {
    let mut rows = Vec::new();
    let mut index = 0;
    while index < shards.len() {
        let n = shards[index].record.stream_address.n;
        let mut matrix_count = 0_u64;
        let mut zero_count = 0_u64;
        while index < shards.len() && shards[index].record.stream_address.n == n {
            matrix_count += shards[index].record.matrix_count;
            zero_count += shards[index].record.permanent_zero_count;
            index += 1;
        }
        let (lower, upper) = wilson_interval(zero_count, matrix_count);
        rows.push(SummaryRow {
            schema_version: SCHEMA_VERSION,
            q,
            n,
            matrix_count,
            permanent_zero_count: zero_count,
            determinant: DeterminantCount::NotEvaluated,
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
                determinant_estimate: DeterminantEstimate::NotEvaluated,
            },
        });
    }
    FieldSummary {
        schema_version: SCHEMA_VERSION,
        q,
        rows,
    }
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permanent_campaign::schema::{
        ArtifactIdentity, Availability, CellSpec, GitRevision, Provenance, RngAlgorithm, ShardSpec,
        StreamPurpose,
    };
    use gf2_stats::binomial::{bonferroni_level, permanent_zero_floor_test};
    use std::collections::BTreeSet;

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
        let mut entries = vec![Fp::<3>::new(0); 4];
        for _ in 0..4 {
            sampler.fill_next_matrix(&mut entries);
            expected[permanent_ryser(&entries, 2).value() as usize] += 1;
        }
        assert_eq!(shard.record.permanent_histogram, expected);
        assert_eq!(shard.record.permanent_zero_count, expected[0]);
        assert!(shard.timing.draw >= Duration::ZERO);
        assert!(shard.timing.pack >= Duration::ZERO);
        assert!(shard.timing.evaluate >= Duration::ZERO);
        assert!(shard.timing.count >= Duration::ZERO);
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
