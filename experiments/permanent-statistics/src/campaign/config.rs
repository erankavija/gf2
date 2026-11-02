use super::{backend_supports_cell, invalid, Backend, CampaignError};
use experiments_common::{ArtifactPath, Sha256Digest};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Campaign parameters frozen before drawing matrices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignConfig {
    /// Dataset name.
    pub campaign_id: String,
    /// Campaign sampler seed.
    pub root_seed: u64,
    /// Fixed statistical budgets.
    pub acceptance: AcceptanceConfig,
    /// Admission and bounded launch rules.
    pub execution: ExecutionConfig,
    /// Ordered, nonoverlapping cell ranges.
    pub cell_groups: Vec<CellGroup>,
    /// Committed comparison data, consumed by the analysis script.
    pub reference_data: ReferenceData,
    /// Independent validation protocol.
    pub validation: ValidationConfig,
}
/// Family budgets and the single report interval critical value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceConfig {
    /// One-sided permanent-floor family budget.
    pub permanent_family_error: f64,
    /// Two-sided determinant family budget.
    pub determinant_family_error: f64,
    /// Wilson interval critical value.
    pub wilson_z: f64,
}
/// Rules shared by every invocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    /// Cell that must terminate before another starts.
    pub first_cell: (u8, u16),
    /// Reported failures allowed for one shard.
    pub max_shard_attempts: u32,
    /// Per-launch accelerator budget based on configured cost.
    pub accelerator_launch_cap_ms: u64,
}
/// Cells with one execution policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellGroup {
    /// Field order.
    pub q: u8,
    /// First matrix order, inclusive.
    pub n_first: u16,
    /// Last matrix order, inclusive.
    pub n_last: u16,
    /// Samples per cell.
    pub matrix_count: u64,
    /// Samples per stream.
    pub shard_size: u64,
    /// Configured implementation.
    pub backend: Backend,
    /// Evaluate determinants on the same matrices.
    pub determinant_companion: bool,
    /// Cited backend evidence; never used for acceptance.
    pub backend_evidence: ArtifactPath,
    /// Measured launch sizing input, exactly for accelerator groups.
    pub accelerator_per_matrix_us: Option<f64>,
}
/// Paths resolved against the operator's repository root.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceData {
    /// Published target CSV.
    pub reproduction_targets: ArtifactPath,
    /// Literature claim JSON.
    pub literature_claim: ArtifactPath,
}
/// Preregistered validation stream and checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationConfig {
    /// Domain-separated validation seed.
    pub root_seed: u64,
    /// Fresh draws in each regeneration comparison.
    pub regeneration_matrix_count: u64,
    /// Draws for each exact-null check.
    pub sample_matrix_count: u64,
    /// Inclusive rejection threshold.
    pub level: f64,
    /// Maximum exact comparison batch.
    pub exact_batch_matrix_count: usize,
    /// Implementation used for random draws.
    pub sample_backend: Backend,
    /// Every applicable entry must be available and agree with the oracle.
    pub selectable_backends: Vec<Backend>,
    /// Exact enumeration domains.
    pub anchors: Vec<Anchor>,
}
/// Expected exhaustive count and independent stream address.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    /// Field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
    /// Validation stream index.
    pub stream_index: u64,
    /// Preregistered oracle zero count.
    pub permanent_zero_count: u64,
}
/// One expanded cell, with its ordinal fixing the stream namespace.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// Field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
    /// Samples per cell.
    pub matrix_count: u64,
    /// Samples per stream.
    pub shard_size: u64,
    /// Configured implementation.
    pub backend: Backend,
    /// Paired determinants.
    pub determinant_companion: bool,
    /// Cited evidence.
    pub backend_evidence: ArtifactPath,
    /// Launch sizing input.
    pub accelerator_per_matrix_us: Option<f64>,
    /// Ordinal in configuration order.
    pub ordinal: u64,
}
/// One bounded stream of a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShardSpec {
    /// Ordinal within the cell.
    pub shard_id: u64,
    /// Domain-separated stream index.
    pub stream_index: u64,
    /// Number of matrices in this stream.
    pub matrix_count: u64,
}
impl Cell {
    /// Number of bounded streams.
    ///
    /// # Panics
    /// Panics if `shard_size` is zero; validated configurations exclude it.
    pub fn shard_count(&self) -> u64 {
        self.matrix_count.div_ceil(self.shard_size)
    }
    /// Expands one stream; refuses an out-of-range ordinal.
    ///
    /// # Panics
    /// Panics if `shard_size` is zero; validated configurations exclude it.
    pub fn shard(&self, shard_id: u64) -> Result<ShardSpec, CampaignError> {
        if shard_id >= self.shard_count() {
            return invalid("shard outside cell");
        }
        Ok(ShardSpec {
            shard_id,
            stream_index: (self.ordinal << 32) | shard_id,
            matrix_count: self
                .shard_size
                .min(self.matrix_count - shard_id * self.shard_size),
        })
    }
}
impl CampaignConfig {
    /// Loads and validates a JSON configuration, rejecting unknown fields.
    pub fn load(path: &Path) -> Result<Self, CampaignError> {
        let config: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        config.validate()?;
        Ok(config)
    }
    /// Digests serialized parameters; input whitespace has no effect.
    pub fn digest(&self) -> Result<Sha256Digest, CampaignError> {
        Ok(Sha256Digest::of(&serde_json::to_vec(self)?))
    }
    /// Expands groups in configuration order, without a shard inventory.
    pub fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::new();
        for g in &self.cell_groups {
            for n in g.n_first..=g.n_last {
                cells.push(Cell {
                    q: g.q,
                    n,
                    matrix_count: g.matrix_count,
                    shard_size: g.shard_size,
                    backend: g.backend,
                    determinant_companion: g.determinant_companion,
                    backend_evidence: g.backend_evidence.clone(),
                    accelerator_per_matrix_us: g.accelerator_per_matrix_us,
                    ordinal: cells.len() as u64,
                });
            }
        }
        cells
    }
    /// Checks domains, stream bounds, budgets, and admission rules.
    pub fn validate(&self) -> Result<(), CampaignError> {
        let a = &self.acceptance;
        if self.campaign_id.is_empty()
            || !self.campaign_id.split('-').all(|s| {
                !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
            || !(0.0..1.0).contains(&a.permanent_family_error)
            || a.permanent_family_error == 0.0
            || !(0.0..1.0).contains(&a.determinant_family_error)
            || a.determinant_family_error == 0.0
            || a.permanent_family_error + a.determinant_family_error > 1.0
            || !a.wilson_z.is_finite()
            || a.wilson_z <= 0.0
            || self.execution.max_shard_attempts == 0
            || self.execution.accelerator_launch_cap_ms == 0
        {
            return invalid("invalid campaign budgets or execution rules");
        }
        let mut previous = None;
        for g in &self.cell_groups {
            if g.n_first == 0
                || g.n_first > g.n_last
                || g.matrix_count == 0
                || g.shard_size == 0
                || g.matrix_count.div_ceil(g.shard_size) > (1u64 << 32)
                || previous.is_some_and(|key| key >= (g.q, g.n_first))
                || !backend_supports_cell(g.backend, g.q, g.n_last)
                || (g.backend == Backend::Accelerator) != g.accelerator_per_matrix_us.is_some()
                || g.accelerator_per_matrix_us
                    .is_some_and(|x| !x.is_finite() || x <= 0.0)
            {
                return invalid("invalid, unsorted, or overlapping cell group");
            }
            previous = Some((g.q, g.n_last));
        }
        let cells = self.cells();
        if cells.is_empty()
            || cells.len() >= 1 << 24
            || !cells
                .iter()
                .any(|c| (c.q, c.n) == self.execution.first_cell)
        {
            return invalid("first_cell must name an expanded cell");
        }
        let v = &self.validation;
        if v.anchors.is_empty()
            || v.exact_batch_matrix_count == 0
            || v.regeneration_matrix_count == 0
            || v.sample_matrix_count == 0
            || !(0.0..1.0).contains(&v.level)
            || v.level == 0.0
            || v.selectable_backends.is_empty()
        {
            return invalid("invalid validation allocation");
        }
        let mut addresses = std::collections::BTreeSet::new();
        for anchor in &v.anchors {
            if !matches!(anchor.q, 3 | 5 | 7)
                || anchor.n == 0
                || anchor.n > if anchor.q == 3 { 4 } else { 3 }
                || anchor.stream_index >= gf2_stats::sampler::STREAM_INDEX_LIMIT
                || !addresses.insert((anchor.q, anchor.n, anchor.stream_index))
                || !backend_supports_cell(v.sample_backend, anchor.q, anchor.n)
            {
                return invalid("invalid validation anchor");
            }
        }
        Ok(())
    }
}
