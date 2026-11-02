//! Schema-3 records, pooled summaries, and dataset integrity.
use super::{
    invalid, AcceptanceConfig, Backend, CampaignConfig, CampaignError, Cell, ReferenceData,
};
use experiments_common::{publish_atomic, ArtifactPath, RunRecord, Sha256Digest};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, ffi::OsStr, fs, path::Path};

/// The only supported dataset representation.
pub const SCHEMA_VERSION: u32 = 3;
/// Stable summary CSV contract, including log-scale evidence.
pub const SUMMARY_FIELDS: &[&str] = &[
    "schema_version",
    "q",
    "n",
    "matrix_count",
    "permanent_zero_count",
    "permanent_point_estimate",
    "permanent_interval_lower",
    "permanent_interval_upper",
    "permanent_verdict",
    "determinant_state",
    "determinant_sample_count",
    "determinant_zero_count",
    "determinant_point_estimate",
    "determinant_interval_lower",
    "determinant_interval_upper",
    "determinant_verdict",
    "terminal_state",
    "halt_reason",
    "permanent_log_p_value",
    "determinant_log_p_value",
];
/// Compact pre-draw contract; shards are derived from each cell's size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Exact schema edition.
    pub schema_version: u32,
    /// Dataset name.
    pub campaign_id: String,
    /// Stream seed.
    pub root_seed: u64,
    /// Digest of the complete frozen configuration.
    pub configuration_sha256: Sha256Digest,
    /// Campaign-cell sampler purpose.
    pub purpose_tag: u8,
    /// Statistical budgets and interval convention.
    pub acceptance: AcceptanceConfig,
    /// Committed comparison sources.
    pub reference_data: ReferenceData,
    /// Cells in stream order.
    pub cells: Vec<ManifestCell>,
}
/// One pre-draw cell contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestCell {
    /// Field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
    /// Fixed sample count.
    pub matrix_count: u64,
    /// Samples per stream.
    pub shard_size: u64,
    /// Configured backend.
    pub backend: Backend,
    /// Cited evidence, not an acceptance input.
    pub backend_evidence: ArtifactPath,
    /// Determinants on the same draws.
    pub determinant_companion: bool,
}
impl Manifest {
    /// Pure projection of validated configuration.
    pub fn from_config(config: &CampaignConfig) -> Result<Self, CampaignError> {
        config.validate()?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            campaign_id: config.campaign_id.clone(),
            root_seed: config.root_seed,
            configuration_sha256: config.digest()?,
            purpose_tag: 3,
            acceptance: config.acceptance.clone(),
            reference_data: config.reference_data.clone(),
            cells: config
                .cells()
                .into_iter()
                .map(|c| ManifestCell {
                    q: c.q,
                    n: c.n,
                    matrix_count: c.matrix_count,
                    shard_size: c.shard_size,
                    backend: c.backend,
                    backend_evidence: c.backend_evidence,
                    determinant_companion: c.determinant_companion,
                })
                .collect(),
        })
    }
    pub(crate) fn expanded_cells(&self) -> Vec<Cell> {
        self.cells
            .iter()
            .enumerate()
            .map(|(i, c)| Cell {
                q: c.q,
                n: c.n,
                matrix_count: c.matrix_count,
                shard_size: c.shard_size,
                backend: c.backend,
                backend_evidence: c.backend_evidence.clone(),
                determinant_companion: c.determinant_companion,
                ordinal: i as u64,
                accelerator_per_matrix_us: None,
            })
            .collect()
    }
    /// Reads exactly schema 3 and checks the mathematical cell domains.
    pub fn load(directory: &Path) -> Result<Self, CampaignError> {
        let manifest: Self = serde_json::from_slice(&fs::read(directory.join("manifest.json"))?)?;
        if manifest.schema_version != SCHEMA_VERSION
            || manifest.purpose_tag != 3
            || manifest.cells.is_empty()
        {
            return invalid("unsupported or empty manifest");
        }
        let mut previous = None;
        for c in manifest.expanded_cells() {
            if c.matrix_count == 0
                || c.shard_size == 0
                || c.n == 0
                || c.shard_count() > 1 << 32
                || c.ordinal >= 1 << 24
                || !super::backend_supports_cell(c.backend, c.q, c.n)
                || previous.is_some_and(|key| key >= (c.q, c.n))
            {
                return invalid("invalid manifest cell");
            }
            previous = Some((c.q, c.n));
        }
        Ok(manifest)
    }
    pub(crate) fn publish(&self, directory: &Path) -> Result<(), CampaignError> {
        let path = directory.join("manifest.json");
        if path.exists() {
            let existing: Self = serde_json::from_slice(&fs::read(path)?)?;
            if existing != *self {
                return invalid("existing manifest differs");
            }
        } else {
            publish_atomic(&path, &serde_json::to_vec_pretty(self)?)?;
        }
        Ok(())
    }
}
/// One accepted stream, also embedded in the write-ahead log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardRecord {
    /// Stream ordinal within a cell.
    pub shard_id: u64,
    /// Full sampler index.
    pub stream_index: u64,
    /// Draw count.
    pub matrix_count: u64,
    /// Permanent-zero count.
    pub permanent_zero_count: u64,
    /// Residue counts in field order.
    pub permanent_histogram: Vec<u64>,
    /// Paired determinant-zero count, absent if not evaluated.
    pub determinant_zero_count: Option<u64>,
    /// Implementation that actually ran.
    pub executed_backend: Backend,
}
/// Inclusive exact-test decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceVerdict {
    /// Does not reject the registered null.
    Accepted,
    /// Rejects at the allocated level.
    Rejected,
}
/// Why campaign admission stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HaltReason {
    /// An exact test rejects.
    AcceptanceFailure,
    /// The selected device is unavailable.
    BackendUnavailable,
    /// Reported failures exhaust the shard's retry allocation.
    ExecutionFailure,
}
/// Cell terminal category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellTerminalState {
    /// All registered samples are retained.
    Completed,
    /// Partial or zero evidence is retained with a reason.
    Halted,
}
/// Point estimate and Wilson report interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProportionEstimate {
    /// Observed proportion.
    pub point: f64,
    /// Lower endpoint.
    pub lower: f64,
    /// Upper endpoint.
    pub upper: f64,
}
/// Terminal pooled counts and their canonical assessment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryRow {
    /// Exact schema edition.
    pub schema_version: u32,
    /// Field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
    /// Retained samples.
    pub matrix_count: u64,
    /// Retained permanent zeros.
    pub permanent_zero_count: u64,
    /// Retained paired determinant zeros.
    pub determinant_zero_count: Option<u64>,
    /// Completed permanent estimate.
    pub permanent_estimate: Option<ProportionEstimate>,
    /// Completed determinant estimate.
    pub determinant_estimate: Option<ProportionEstimate>,
    /// Permanent-floor decision.
    pub permanent_verdict: Option<AcceptanceVerdict>,
    /// Determinant decision.
    pub determinant_verdict: Option<AcceptanceVerdict>,
    /// Log permanent p-value.
    pub permanent_log_p_value: Option<f64>,
    /// Log determinant p-value.
    pub determinant_log_p_value: Option<f64>,
    /// Completion category.
    pub terminal_state: CellTerminalState,
    /// Reason for a partial cell.
    pub halt_reason: Option<HaltReason>,
}
impl SummaryRow {
    pub(crate) fn halted(cell: &Cell, shards: &[ShardRecord], reason: HaltReason) -> Self {
        let (matrix_count, permanent_zero_count, determinant_zero_count) = pool(cell, shards);
        Self {
            schema_version: SCHEMA_VERSION,
            q: cell.q,
            n: cell.n,
            matrix_count,
            permanent_zero_count,
            determinant_zero_count,
            permanent_estimate: None,
            determinant_estimate: None,
            permanent_verdict: None,
            determinant_verdict: None,
            permanent_log_p_value: None,
            determinant_log_p_value: None,
            terminal_state: CellTerminalState::Halted,
            halt_reason: Some(reason),
        }
    }
}
pub(crate) fn pool(cell: &Cell, shards: &[ShardRecord]) -> (u64, u64, Option<u64>) {
    (
        shards.iter().map(|r| r.matrix_count).sum(),
        shards.iter().map(|r| r.permanent_zero_count).sum(),
        cell.determinant_companion
            .then(|| shards.iter().filter_map(|r| r.determinant_zero_count).sum()),
    )
}
/// Checks stream membership, counts, histogram, and determinant presence.
pub fn validate_shard(cell: &Cell, shard: &ShardRecord) -> Result<(), CampaignError> {
    let spec = cell.shard(shard.shard_id)?;
    if shard.stream_index != spec.stream_index
        || shard.matrix_count != spec.matrix_count
        || shard.permanent_histogram.len() != cell.q as usize
        || shard
            .permanent_histogram
            .iter()
            .try_fold(0u64, |s, x| s.checked_add(*x))
            != Some(shard.matrix_count)
        || shard.permanent_histogram.first() != Some(&shard.permanent_zero_count)
        || shard.determinant_zero_count.is_some() != cell.determinant_companion
        || shard
            .determinant_zero_count
            .is_some_and(|z| z > shard.matrix_count)
        || !super::backend_supports_cell(shard.executed_backend, cell.q, cell.n)
    {
        return invalid("inconsistent accepted shard");
    }
    Ok(())
}
/// Canonical per-cell shard path.
pub fn shard_path(q: u8, n: u16) -> String {
    format!("shards/q{q}-n{n:02}.jsonl")
}
pub(crate) fn publish_shards(
    directory: &Path,
    cell: &Cell,
    shards: &[ShardRecord],
) -> Result<(), CampaignError> {
    let mut bytes = Vec::new();
    for shard in shards {
        serde_json::to_writer(&mut bytes, shard)?;
        bytes.push(b'\n');
    }
    publish_atomic(&directory.join(shard_path(cell.q, cell.n)), &bytes)?;
    Ok(())
}
fn token<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .unwrap()
        .into()
}
fn number<T: ToString>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}
/// Encodes the stable CSV boundary without rounding the stored floating values.
pub fn encode_summary_csv(rows: &[SummaryRow]) -> String {
    let mut csv = SUMMARY_FIELDS.join(",") + "\n";
    for r in rows {
        let p = r.permanent_estimate;
        let d = r.determinant_estimate;
        let fields = vec![
            r.schema_version.to_string(),
            r.q.to_string(),
            r.n.to_string(),
            r.matrix_count.to_string(),
            r.permanent_zero_count.to_string(),
            number(p.map(|e| e.point)),
            number(p.map(|e| e.lower)),
            number(p.map(|e| e.upper)),
            number(r.permanent_verdict.map(token)),
            if r.determinant_zero_count.is_some() {
                "evaluated"
            } else {
                "not_evaluated"
            }
            .into(),
            number(r.determinant_zero_count.map(|_| r.matrix_count)),
            number(r.determinant_zero_count),
            number(d.map(|e| e.point)),
            number(d.map(|e| e.lower)),
            number(d.map(|e| e.upper)),
            number(r.determinant_verdict.map(token)),
            token(r.terminal_state),
            number(r.halt_reason.map(token)),
            number(r.permanent_log_p_value),
            number(r.determinant_log_p_value),
        ];
        csv += &(fields.join(",") + "\n");
    }
    csv
}
fn parse<T: std::str::FromStr>(value: &str) -> Result<T, CampaignError> {
    value
        .parse()
        .map_err(|_| CampaignError::Invalid("invalid summary value".into()))
}
fn optional<T: std::str::FromStr>(value: &str) -> Result<Option<T>, CampaignError> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse(value).map(Some)
    }
}
fn enum_value<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, CampaignError> {
    Ok(serde_json::from_value(serde_json::Value::String(
        value.into(),
    ))?)
}
fn optional_enum<T: serde::de::DeserializeOwned>(value: &str) -> Result<Option<T>, CampaignError> {
    if value.is_empty() {
        Ok(None)
    } else {
        enum_value(value).map(Some)
    }
}
/// Parses one row in the schema-3 column order, rejecting incomplete estimates.
pub fn parse_summary_row(fields: &[&str]) -> Result<SummaryRow, CampaignError> {
    if fields.len() != SUMMARY_FIELDS.len() || fields[0] != "3" {
        return invalid("summary schema differs");
    }
    let estimate = |start: usize| -> Result<Option<ProportionEstimate>, CampaignError> {
        if fields[start..start + 3].iter().all(|v| v.is_empty()) {
            return Ok(None);
        }
        Ok(Some(ProportionEstimate {
            point: parse(fields[start])?,
            lower: parse(fields[start + 1])?,
            upper: parse(fields[start + 2])?,
        }))
    };
    let matrix_count = parse(fields[3])?;
    let determinant_zero_count = match fields[9] {
        "evaluated" if parse::<u64>(fields[10])? == matrix_count => Some(parse(fields[11])?),
        "not_evaluated" if fields[10..16].iter().all(|f| f.is_empty()) => None,
        _ => return invalid("invalid determinant summary state"),
    };
    Ok(SummaryRow {
        schema_version: 3,
        q: parse(fields[1])?,
        n: parse(fields[2])?,
        matrix_count,
        permanent_zero_count: parse(fields[4])?,
        determinant_zero_count,
        permanent_estimate: estimate(5)?,
        determinant_estimate: estimate(12)?,
        permanent_verdict: optional_enum(fields[8])?,
        determinant_verdict: optional_enum(fields[15])?,
        terminal_state: enum_value(fields[16])?,
        halt_reason: optional_enum(fields[17])?,
        permanent_log_p_value: optional(fields[18])?,
        determinant_log_p_value: optional(fields[19])?,
    })
}
/// Run observations and the cells whose sampling they describe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCells {
    /// Closed identity plus rebuild pointer.
    pub record: RunRecord,
    /// Cells started under this record.
    pub cells: Vec<(u8, u16)>,
}
fn membership(manifest: &Manifest) -> BTreeSet<String> {
    [
        "manifest.json",
        "execution.jsonl",
        "summary.csv",
        "runs.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(manifest.cells.iter().map(|c| shard_path(c.q, c.n)))
    .collect()
}
/// Removes what interrupted writes left beside the dataset's files, which
/// [`verify`]'s exact membership would otherwise reject.
fn remove_interrupted_writes(directory: &Path) -> Result<(), CampaignError> {
    let checkpoint = directory.join("checkpoint.json");
    for parent in [directory.to_owned(), directory.join("shards")] {
        if !parent.is_dir() {
            continue;
        }
        for entry in fs::read_dir(parent)? {
            let path = entry?.path();
            if is_interrupted_publish(&path)
                || gf2_sim::checkpoint::is_interrupted_write(&checkpoint, &path)
            {
                fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

fn is_interrupted_publish(candidate: &Path) -> bool {
    let name = candidate.file_name().map(OsStr::to_string_lossy);
    name.as_deref()
        .and_then(|name| name.strip_prefix('.')?.strip_suffix(".tmp"))
        .and_then(|rest| {
            let (rest, sequence) = rest.rsplit_once('.')?;
            let (published, process) = rest.rsplit_once('.')?;
            let numeric = process.parse::<u32>().is_ok() && sequence.parse::<u64>().is_ok();
            Some(numeric && !published.is_empty())
        })
        .unwrap_or(false)
}

/// Publishes pooled summaries, run observations, and integrity digests, and
/// removes temporaries of interrupted writes. No run may be active.
/// Refuses until every cell has a logged terminal row.
pub fn finalize(directory: &Path) -> Result<(), CampaignError> {
    let manifest = Manifest::load(directory)?;
    let state = super::runner::replay(directory, &manifest)?;
    let rows: Vec<_> = state
        .cells
        .iter()
        .map(|c| {
            c.terminal
                .clone()
                .ok_or_else(|| CampaignError::Invalid("nonterminal cell".into()))
        })
        .collect::<Result<_, _>>()?;
    remove_interrupted_writes(directory)?;
    for (cell, progress) in manifest.expanded_cells().iter().zip(&state.cells) {
        publish_shards(directory, cell, &progress.shards)?;
    }
    publish_atomic(
        &directory.join("summary.csv"),
        encode_summary_csv(&rows).as_bytes(),
    )?;
    publish_atomic(
        &directory.join("runs.json"),
        &serde_json::to_vec_pretty(&state.runs)?,
    )?;
    let mut sums = String::new();
    for path in membership(&manifest) {
        sums += &format!(
            "{}  {path}\n",
            Sha256Digest::of(&fs::read(directory.join(&path))?)
        );
    }
    publish_atomic(&directory.join("checksums.sha256"), sums.as_bytes())?;
    Ok(())
}
/// Verifies exact membership, digests, log projections, shard semantics, and pooled decisions.
pub fn verify(directory: &Path) -> Result<(), CampaignError> {
    let manifest = Manifest::load(directory)?;
    let expected = membership(&manifest);
    let mut seen = BTreeSet::new();
    for line in fs::read_to_string(directory.join("checksums.sha256"))?.lines() {
        let (digest, path) = line
            .split_once("  ")
            .ok_or_else(|| CampaignError::Invalid("invalid checksum line".into()))?;
        if !expected.contains(path)
            || !seen.insert(path.to_owned())
            || Sha256Digest::of(&fs::read(directory.join(path))?).as_str() != digest
        {
            return invalid("checksum membership or digest mismatch");
        }
    }
    if seen != expected {
        return invalid("checksum membership incomplete");
    }
    let mut actual = BTreeSet::new();
    fn files(root: &Path, dir: &Path, result: &mut BTreeSet<String>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|s| s == "derived") {
                    continue;
                }
                files(root, &path, result)?;
            } else {
                result.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        Ok(())
    }
    files(directory, directory, &mut actual)?;
    actual.remove("checkpoint.json");
    actual.remove("checksums.sha256");
    if actual != expected {
        return invalid("dataset file membership differs");
    }
    let state = super::runner::replay(directory, &manifest)?;
    let mut rows = Vec::new();
    for (cell, progress) in manifest.expanded_cells().iter().zip(&state.cells) {
        let bytes = fs::read_to_string(directory.join(shard_path(cell.q, cell.n)))?;
        let shards: Vec<ShardRecord> = bytes
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        if shards != progress.shards {
            return invalid("shards differ from log");
        }
        let row = progress
            .terminal
            .as_ref()
            .ok_or_else(|| CampaignError::Invalid("nonterminal dataset".into()))?;
        validate_summary_aggregate(&manifest, cell, &shards, row)?;
        rows.push(row.clone());
    }
    if fs::read_to_string(directory.join("summary.csv"))? != encode_summary_csv(&rows) {
        return invalid("summary differs from pooled shards");
    }
    let runs: Vec<RunCells> = serde_json::from_slice(&fs::read(directory.join("runs.json"))?)?;
    if runs != state.runs {
        return invalid("run observations differ from log");
    }
    Ok(())
}
/// Recomputes a terminal row from accepted streams and the manifest budgets.
pub fn validate_summary_aggregate(
    manifest: &Manifest,
    cell: &Cell,
    shards: &[ShardRecord],
    row: &SummaryRow,
) -> Result<(), CampaignError> {
    for (index, shard) in shards.iter().enumerate() {
        validate_shard(cell, shard)?;
        if shard.shard_id != index as u64 {
            return invalid("shard sequence differs");
        }
    }
    let expected = match row.terminal_state {
        CellTerminalState::Completed => {
            let (count, zeros, det) = pool(cell, shards);
            super::assess_completed_cell(
                &super::AcceptancePlan::for_manifest(manifest)?,
                cell,
                count,
                zeros,
                det,
            )?
        }
        CellTerminalState::Halted => SummaryRow::halted(
            cell,
            shards,
            row.halt_reason
                .ok_or_else(|| CampaignError::Invalid("halt without reason".into()))?,
        ),
    };
    if &expected != row {
        return invalid("terminal assessment differs from pooled evidence");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_interrupted_publish;
    use std::path::Path;

    #[test]
    fn only_a_reserved_sibling_is_an_interrupted_publish() {
        assert!(is_interrupted_publish(Path::new(".a.b.123.4.tmp")));
        for name in ["a.b", ".a.b", "a.1.2.tmp", ".a.x.2.tmp", ".1.2.tmp"] {
            assert!(!is_interrupted_publish(Path::new(name)), "{name}");
        }
    }
}
