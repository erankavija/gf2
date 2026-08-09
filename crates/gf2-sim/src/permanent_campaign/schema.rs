//! Versioned permanent-zero-fraction dataset schema.
//!
//! JSON documents use strict serde schemas: required fields cannot be omitted,
//! unknown fields are rejected, and every document carries
//! [`SCHEMA_VERSION`]. The pooled summary is deliberately a flat CSV whose
//! exact header is [`SUMMARY_CSV_FIELDS`]. [`conform_dataset`] is the one
//! reader-side conformance entry point for the complete raw dataset; the
//! cryptographic half of reading a dataset lives in
//! [`provenance`](super::provenance).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

/// The only dataset schema version accepted by this module.
pub const SCHEMA_VERSION: u32 = 1;

/// Root manifest file name.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Campaign-scoped pooled summary file name.
pub const POOLED_SUMMARY_FILE: &str = "summary.csv";

/// Campaign-scoped raw-data integrity file name.
pub const INTEGRITY_FILE: &str = "checksums.sha256";

/// Returns the campaign-relative path of one shard record.
///
/// This is the one place the shard path grammar is written; the layout, the
/// conformance reader, and the integrity layer all derive their paths here so
/// a path built for one purpose cannot drift from a path built for another.
pub fn shard_record_file(q: u8, n: u16, shard_id: u64) -> String {
    format!("shards/q{q}/n{n:02}/shard-{shard_id:06}.json")
}

/// Returns the campaign-relative path of one field arm's summary.
pub fn field_summary_file(q: u8) -> String {
    format!("summaries/q{q}.json")
}

/// Canonical pooled-summary CSV header.
pub const SUMMARY_CSV_FIELDS: &[&str] = &[
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
];

const JSON_FIELDS: &[&str] = &[
    "schema_version",
    "campaign_id",
    "root_seed",
    "stream_purposes",
    "cells",
    "provenance",
    "name",
    "tag",
    "q",
    "n",
    "matrix_count",
    "shard_size",
    "shards",
    "backend",
    "backend_receipt",
    "determinant_companion",
    "path",
    "sha256",
    "shard_id",
    "stream_index",
    "git_revision",
    "compiler_version",
    "rng_algorithm",
    "rng_version",
    "invocation",
    "accelerator_runtime",
    "cpu_model",
    "gpu_model",
    "state",
    "value",
    "stream_address",
    "purpose_tag",
    "permanent_zero_count",
    "permanent_histogram",
    "determinant",
    "determinant_estimate",
    "sample_count",
    "zero_count",
    "rows",
    "permanent_estimate",
    "point",
    "interval",
    "lower",
    "upper",
    "permanent_verdict",
    "terminal_state",
    "estimate",
    "verdict",
    "reason",
];

/// Returns every admitted field name in the JSON and CSV schemas.
///
/// This list is the review surface used to ensure the durable format contains
/// data and mechanical provenance only. It intentionally contains no free-form
/// analysis or scientific-claim field.
pub fn schema_field_names() -> impl Iterator<Item = &'static str> {
    JSON_FIELDS.iter().chain(SUMMARY_CSV_FIELDS).copied()
}

/// A validated, serialization-safe campaign identifier.
///
/// Identifiers contain lowercase ASCII letters, digits, and interior hyphens.
/// A published identifier is immutable: the writer must create a new campaign
/// id instead of replacing the directory for an existing one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CampaignId(String);

impl FromStr for CampaignId {
    type Err = TokenError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_token(value, "campaign id")?;
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for CampaignId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for CampaignId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// A validated stream-purpose label at the serialization boundary.
///
/// The numeric [`StreamPurpose::tag`] is the domain-separation identity. The
/// label is retained for navigation and is mapped to the sampler's canonical
/// purpose type by the campaign driver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PurposeName(String);

impl FromStr for PurposeName {
    type Err = TokenError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_token(value, "stream purpose")?;
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for PurposeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for PurposeName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned when a constrained schema token is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenError {
    kind: &'static str,
    value: String,
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:?} must contain lowercase ASCII letters or digits separated by single hyphens",
            self.kind, self.value
        )
    }
}

impl std::error::Error for TokenError {}

fn validate_token(value: &str, kind: &'static str) -> Result<(), TokenError> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value.as_bytes()[0] != b'-'
        && value.as_bytes()[value.len() - 1] != b'-'
        && !value.as_bytes().windows(2).any(|pair| pair == b"--");
    if valid {
        Ok(())
    } else {
        Err(TokenError {
            kind,
            value: value.to_owned(),
        })
    }
}

/// A normalized repository-relative path to a committed artifact.
///
/// Paths use forward slashes, contain no empty, `.` or `..` component, and
/// cannot name an absolute or drive-qualified path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ArtifactPath(String);

impl ArtifactPath {
    /// Returns the validated repository-relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ArtifactPath {
    type Err = ArtifactPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let first_component = value.split('/').next().unwrap_or_default();
        let drive_qualified = first_component.as_bytes().get(1) == Some(&b':')
            && first_component
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic);
        let valid = !value.is_empty()
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains('\\')
            && !drive_qualified
            && !value.chars().any(char::is_control)
            && value
                .split('/')
                .all(|component| !component.is_empty() && component != "." && component != "..");
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(ArtifactPathError(value.to_owned()))
        }
    }
}

impl fmt::Display for ArtifactPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for ArtifactPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned for a path outside the canonical artifact-path grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPathError(String);

impl fmt::Display for ArtifactPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "artifact path {:?} must be a normalized repository-relative path",
            self.0
        )
    }
}

impl std::error::Error for ArtifactPathError {}

/// A canonical lowercase hexadecimal SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Returns the 64-character lowercase hexadecimal digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Sha256Digest {
    type Err = Sha256DigestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if is_lowercase_hex(value, 64) {
            Ok(Self(value.to_owned()))
        } else {
            Err(Sha256DigestError(value.to_owned()))
        }
    }
}

fn is_lowercase_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned for a non-canonical SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha256DigestError(String);

impl fmt::Display for Sha256DigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SHA-256 digest {:?} must contain exactly 64 lowercase hexadecimal characters",
            self.0
        )
    }
}

impl std::error::Error for Sha256DigestError {}

/// A canonical full-length git revision.
///
/// The recorded value is the complete 40-character lowercase hexadecimal
/// object name. An abbreviation is rejected because it resolves only against
/// the repository that produced it, while a published dataset is read
/// elsewhere and must still name exactly one commit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct GitRevision(String);

impl GitRevision {
    /// Returns the 40-character lowercase hexadecimal object name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for GitRevision {
    type Err = GitRevisionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if is_lowercase_hex(value, 40) {
            Ok(Self(value.to_owned()))
        } else {
            Err(GitRevisionError(value.to_owned()))
        }
    }
}

impl fmt::Display for GitRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for GitRevision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned for a revision that is not a full object name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRevisionError(String);

impl fmt::Display for GitRevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "git revision {:?} must contain exactly 40 lowercase hexadecimal characters",
            self.0
        )
    }
}

impl std::error::Error for GitRevisionError {}

/// Content identity for a committed mechanical artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    /// Normalized path from the repository root.
    pub path: ArtifactPath,
    /// SHA-256 of the committed artifact bytes.
    pub sha256: Sha256Digest,
}

/// Explicit availability state for optional mechanical provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability<T> {
    /// The value was available and recorded.
    Present {
        /// Recorded version or hardware model.
        value: T,
    },
    /// No matching runtime or hardware was present.
    NotPresent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AvailabilityWire<T> {
    state: AvailabilityState,
    value: Option<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AvailabilityState {
    Present,
    NotPresent,
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Availability<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = AvailabilityWire::deserialize(deserializer)?;
        match (wire.state, wire.value) {
            (AvailabilityState::Present, Some(value)) => Ok(Self::Present { value }),
            (AvailabilityState::NotPresent, None) => Ok(Self::NotPresent),
            (AvailabilityState::Present, None) => {
                Err(serde::de::Error::custom("present state requires value"))
            }
            (AvailabilityState::NotPresent, Some(_)) => {
                Err(serde::de::Error::custom("not_present state forbids value"))
            }
        }
    }
}

/// Root manifest for one immutable campaign id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignManifest {
    /// Dataset schema version.
    pub schema_version: u32,
    /// Immutable directory identity for this campaign.
    pub campaign_id: CampaignId,
    /// Campaign-wide root seed.
    pub root_seed: u64,
    /// Domain-separated stream-purpose namespace.
    pub stream_purposes: Vec<StreamPurpose>,
    /// Frozen campaign grid and shard identities.
    pub cells: Vec<CellSpec>,
    /// Source, toolchain, runtime, and hardware provenance.
    pub provenance: Provenance,
}

/// One named stream-purpose tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamPurpose {
    /// Serialization-boundary purpose label.
    pub name: PurposeName,
    /// Top-eight-bit tag used by seed derivation.
    pub tag: u8,
}

/// Frozen execution specification for one $(q,n)$ cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellSpec {
    /// Prime field order.
    pub q: u8,
    /// Square matrix order.
    pub n: u16,
    /// Preregistered matrix count.
    pub matrix_count: u64,
    /// Maximum matrices in each shard.
    pub shard_size: u64,
    /// Ordered shard identities and stream indices.
    pub shards: Vec<ShardSpec>,
    /// Frozen permanent-evaluation backend.
    pub backend: Backend,
    /// Committed measurement receipt that determined the backend selection.
    pub backend_receipt: ArtifactIdentity,
    /// Whether determinant evaluation runs on the same matrices.
    pub determinant_companion: DeterminantPlan,
}

/// Frozen identity of one shard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardSpec {
    /// Cell-local stable shard identity.
    pub shard_id: u64,
    /// Low-56-bit stream index.
    pub stream_index: u64,
}

/// Backend selected for a campaign cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// Single-threaded field-specialized implementation.
    Scalar,
    /// Matrices distributed across a processor thread pool.
    BatchParallel,
    /// One matrix evaluated by an intra-matrix parallel implementation.
    IntraMatrixParallel,
    /// Generic finite-field Ryser reference implementation.
    GenericRyser,
    /// Accelerator batch implementation selected by the frozen manifest.
    Accelerator,
}

/// Frozen determinant-companion plan for a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminantPlan {
    /// Evaluate determinants on the permanent sample matrices.
    Evaluate,
    /// Do not evaluate determinants for this cell.
    NotEvaluated,
}

/// Deterministic random-number generator algorithm used by the campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RngAlgorithm {
    /// ChaCha20 with the implementation version recorded separately.
    ChaCha20,
}

/// Mechanical provenance required to regenerate and audit the dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Full source git revision embedded by the producing build.
    pub git_revision: GitRevision,
    /// Complete compiler version string.
    pub compiler_version: String,
    /// Closed RNG algorithm identity.
    pub rng_algorithm: RngAlgorithm,
    /// Exact RNG crate or implementation version.
    pub rng_version: String,
    /// Exact producer invocation as unquoted argument tokens.
    pub invocation: Vec<String>,
    /// Accelerator runtime version, or an explicit absent state.
    pub accelerator_runtime: Availability<String>,
    /// Processor model.
    pub cpu_model: String,
    /// Accelerator model, or an explicit absent state.
    pub gpu_model: Availability<String>,
}

/// Complete deterministic address of a shard's matrix stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamAddress {
    /// Campaign root seed.
    pub root_seed: u64,
    /// Prime field order.
    pub q: u8,
    /// Square matrix order.
    pub n: u16,
    /// Domain-separation purpose tag.
    pub purpose_tag: u8,
    /// Low-56-bit stream index.
    pub stream_index: u64,
}

/// Raw record for one regenerable shard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardRecord {
    /// Dataset schema version.
    pub schema_version: u32,
    /// Cell-local stable shard identity.
    pub shard_id: u64,
    /// Complete deterministic stream address.
    pub stream_address: StreamAddress,
    /// Matrices evaluated in this shard.
    pub matrix_count: u64,
    /// Matrices whose permanent was zero.
    pub permanent_zero_count: u64,
    /// Counts for residues $0,\ldots,q-1$ in order.
    pub permanent_histogram: Vec<u64>,
    /// Determinant companion counts or explicit absence.
    pub determinant: DeterminantCount,
}

/// Determinant companion state stored in a shard record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeterminantCount {
    /// The companion was not evaluated; no numeric count exists.
    NotEvaluated,
    /// The companion was evaluated on `sample_count` matrices.
    Evaluated {
        /// Matrices included in the determinant sample.
        sample_count: u64,
        /// Sample matrices whose determinant was zero.
        zero_count: u64,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeterminantCountWire {
    state: DeterminantState,
    sample_count: Option<u64>,
    zero_count: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum DeterminantState {
    NotEvaluated,
    Evaluated,
}

impl<'de> Deserialize<'de> for DeterminantCount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DeterminantCountWire::deserialize(deserializer)?;
        match (wire.state, wire.sample_count, wire.zero_count) {
            (DeterminantState::NotEvaluated, None, None) => Ok(Self::NotEvaluated),
            (DeterminantState::Evaluated, Some(sample_count), Some(zero_count)) => {
                Ok(Self::Evaluated {
                    sample_count,
                    zero_count,
                })
            }
            (DeterminantState::NotEvaluated, _, _) => Err(serde::de::Error::custom(
                "not_evaluated determinant forbids numeric counts",
            )),
            (DeterminantState::Evaluated, _, _) => Err(serde::de::Error::custom(
                "evaluated determinant requires sample_count and zero_count",
            )),
        }
    }
}

/// One field execution's raw summary document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSummary {
    /// Dataset schema version.
    pub schema_version: u32,
    /// Prime field order shared by every row.
    pub q: u8,
    /// Cell summaries ordered by matrix size.
    pub rows: Vec<SummaryRow>,
}

/// Pooled summary of one $(q,n)$ cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryRow {
    /// Dataset schema version.
    pub schema_version: u32,
    /// Prime field order.
    pub q: u8,
    /// Square matrix order.
    pub n: u16,
    /// Pooled matrix count.
    pub matrix_count: u64,
    /// Pooled permanent-zero count.
    pub permanent_zero_count: u64,
    /// Pooled determinant counts or explicit absence.
    pub determinant: DeterminantCount,
    /// Executed-to-completion statistics or a mechanical halt reason.
    pub terminal_state: CellTerminalState,
}

/// A point estimate and its uncertainty interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProportionEstimate {
    /// Estimated probability.
    pub point: f64,
    /// Confidence interval around the estimate.
    pub interval: Interval,
}

/// Closed interval for a probability estimate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interval {
    /// Inclusive lower endpoint.
    pub lower: f64,
    /// Inclusive upper endpoint.
    pub upper: f64,
}

/// Result of a preregistered acceptance check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceVerdict {
    /// The cell passed its preregistered check.
    Accepted,
    /// The cell failed its preregistered check.
    Rejected,
}

/// Determinant estimate and verdict for a completed summary row.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeterminantEstimate {
    /// The companion was not evaluated; no estimate exists.
    NotEvaluated,
    /// The evaluated companion reached a completed estimate and verdict.
    Evaluated {
        /// Determinant-zero point estimate and interval.
        estimate: ProportionEstimate,
        /// Preregistered determinant acceptance verdict.
        verdict: AcceptanceVerdict,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeterminantEstimateWire {
    state: DeterminantState,
    estimate: Option<ProportionEstimate>,
    verdict: Option<AcceptanceVerdict>,
}

impl<'de> Deserialize<'de> for DeterminantEstimate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DeterminantEstimateWire::deserialize(deserializer)?;
        match (wire.state, wire.estimate, wire.verdict) {
            (DeterminantState::NotEvaluated, None, None) => Ok(Self::NotEvaluated),
            (DeterminantState::Evaluated, Some(estimate), Some(verdict)) => {
                Ok(Self::Evaluated { estimate, verdict })
            }
            (DeterminantState::NotEvaluated, _, _) => Err(serde::de::Error::custom(
                "not_evaluated determinant estimate forbids estimate and verdict",
            )),
            (DeterminantState::Evaluated, _, _) => Err(serde::de::Error::custom(
                "evaluated determinant estimate requires estimate and verdict",
            )),
        }
    }
}

/// Typed completed-versus-halted outcome of a frozen campaign cell.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CellTerminalState {
    /// The cell reached its preregistered sample count and final statistics.
    Completed {
        /// Permanent-zero point estimate and interval.
        permanent_estimate: ProportionEstimate,
        /// Preregistered permanent acceptance verdict.
        permanent_verdict: AcceptanceVerdict,
        /// Determinant estimate and verdict or explicit non-evaluation.
        determinant_estimate: DeterminantEstimate,
    },
    /// Execution halted under the preregistered protocol.
    Halted {
        /// Mechanical halt category.
        reason: HaltReason,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CellTerminalStateWire {
    state: CellTerminalStateTag,
    reason: Option<HaltReason>,
    permanent_estimate: Option<ProportionEstimate>,
    permanent_verdict: Option<AcceptanceVerdict>,
    determinant_estimate: Option<DeterminantEstimate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum CellTerminalStateTag {
    Completed,
    Halted,
}

impl<'de> Deserialize<'de> for CellTerminalState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = CellTerminalStateWire::deserialize(deserializer)?;
        match (
            wire.state,
            wire.reason,
            wire.permanent_estimate,
            wire.permanent_verdict,
            wire.determinant_estimate,
        ) {
            (
                CellTerminalStateTag::Completed,
                None,
                Some(permanent_estimate),
                Some(permanent_verdict),
                Some(determinant_estimate),
            ) => Ok(Self::Completed {
                permanent_estimate,
                permanent_verdict,
                determinant_estimate,
            }),
            (CellTerminalStateTag::Halted, Some(reason), None, None, None) => {
                Ok(Self::Halted { reason })
            }
            (CellTerminalStateTag::Completed, _, _, _, _) => Err(serde::de::Error::custom(
                "completed state requires final statistics and forbids a halt reason",
            )),
            (CellTerminalStateTag::Halted, _, _, _, _) => Err(serde::de::Error::custom(
                "halted state requires a reason and forbids final statistics",
            )),
        }
    }
}

/// Mechanical category for a halted cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HaltReason {
    /// A preregistered acceptance check failed.
    AcceptanceFailure,
    /// The frozen backend was unavailable.
    BackendUnavailable,
    /// The selected execution path reported a fatal failure.
    ExecutionFailure,
}

/// Role that has exclusive ownership of one required dataset path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterRole {
    /// Execution of the named field arm.
    FieldExecution {
        /// Prime field order whose arm owns the path.
        q: u8,
    },
    /// Campaign finalization after field executions finish.
    Finalization,
}

/// Classification of a required dataset file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetFileClass {
    /// Raw data covered by the integrity file.
    RawData,
    /// Integrity metadata that is not self-covered.
    IntegrityMetadata,
}

/// One required file in the dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetFile {
    /// Path relative to the campaign-id directory.
    pub relative_path: String,
    /// Exclusive writer for this path.
    pub writer: WriterRole,
    /// Raw-data or integrity-metadata classification.
    pub class: DatasetFileClass,
}

/// Canonical required-file layout for a planned or conformed campaign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetLayout {
    required_files: Vec<DatasetFile>,
}

impl DatasetLayout {
    /// Derives every planned path, its class, and its single writer role.
    ///
    /// [`conform_dataset`] removes unexecuted shard paths from this plan when
    /// it accepts a halted cell.
    pub fn from_manifest(manifest: &CampaignManifest) -> Self {
        let mut required_files = vec![DatasetFile {
            relative_path: MANIFEST_FILE.to_owned(),
            writer: WriterRole::Finalization,
            class: DatasetFileClass::RawData,
        }];
        let mut fields = BTreeSet::new();
        for cell in &manifest.cells {
            fields.insert(cell.q);
            for shard in &cell.shards {
                required_files.push(DatasetFile {
                    relative_path: shard_record_file(cell.q, cell.n, shard.shard_id),
                    writer: WriterRole::FieldExecution { q: cell.q },
                    class: DatasetFileClass::RawData,
                });
            }
        }
        for q in fields {
            required_files.push(DatasetFile {
                relative_path: field_summary_file(q),
                writer: WriterRole::FieldExecution { q },
                class: DatasetFileClass::RawData,
            });
        }
        required_files.extend([
            DatasetFile {
                relative_path: POOLED_SUMMARY_FILE.to_owned(),
                writer: WriterRole::Finalization,
                class: DatasetFileClass::RawData,
            },
            DatasetFile {
                relative_path: INTEGRITY_FILE.to_owned(),
                writer: WriterRole::Finalization,
                class: DatasetFileClass::IntegrityMetadata,
            },
        ]);
        Self { required_files }
    }

    /// Returns every required file, its class, and its exclusive writer.
    pub fn required_files(&self) -> &[DatasetFile] {
        &self.required_files
    }
}

/// A complete dataset accepted by the canonical conformance reader.
#[derive(Debug, Clone, PartialEq)]
pub struct ConformedDataset {
    /// Strictly parsed root manifest.
    pub manifest: CampaignManifest,
    /// Strictly parsed field summaries keyed by field order.
    pub field_summaries: BTreeMap<u8, FieldSummary>,
    /// Strictly parsed campaign-scoped pooled summary.
    pub pooled_summary: Vec<SummaryRow>,
    /// Present required paths, classes, and writer ownership.
    pub layout: DatasetLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CellAggregate {
    matrix_count: u64,
    permanent_zero_count: u64,
    determinant: DeterminantAggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeterminantAggregate {
    NotEvaluated,
    Evaluated { sample_count: u64, zero_count: u64 },
}

/// Dataset schema or conformance failure.
#[derive(Debug)]
pub enum SchemaError {
    /// A required raw path is absent.
    MissingFile {
        /// Missing path.
        path: PathBuf,
    },
    /// A filesystem operation failed.
    Io {
        /// Path being accessed.
        path: PathBuf,
        /// Underlying operating-system error.
        source: std::io::Error,
    },
    /// JSON or CSV did not match its strict document schema.
    InvalidDocument {
        /// Invalid document path.
        path: PathBuf,
        /// Parser diagnostic.
        message: String,
    },
    /// A document used a schema version this reader does not support.
    UnsupportedVersion {
        /// Document containing the version.
        path: PathBuf,
        /// Version found in the document.
        found: u32,
    },
    /// Parsed values violate a cross-field or cross-document contract.
    InvalidValue {
        /// Path containing the invalid value.
        path: PathBuf,
        /// Validation diagnostic.
        message: String,
    },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFile { path } => {
                write!(f, "required dataset file is missing: {}", path.display())
            }
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::InvalidDocument { path, message } => {
                write!(
                    f,
                    "{} does not match the dataset schema: {message}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, found } => write!(
                f,
                "{} uses dataset schema version {found}; expected {SCHEMA_VERSION}",
                path.display()
            ),
            Self::InvalidValue { path, message } => {
                write!(
                    f,
                    "{} contains invalid dataset values: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for SchemaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

trait SchemaDocument {
    fn version(&self) -> u32;
    fn validate(&self) -> Result<(), String>;
}

impl SchemaDocument for CampaignManifest {
    fn version(&self) -> u32 {
        self.schema_version
    }

    fn validate(&self) -> Result<(), String> {
        validate_nonempty("compiler_version", &self.provenance.compiler_version)?;
        validate_nonempty("rng_version", &self.provenance.rng_version)?;
        if self.provenance.rng_version.contains('\0') {
            return Err("rng_version must not contain NUL".to_owned());
        }
        if self.provenance.invocation.is_empty() || self.provenance.invocation[0].is_empty() {
            return Err("invocation must include a non-empty executable token".to_owned());
        }
        if self
            .provenance
            .invocation
            .iter()
            .any(|argument| argument.contains('\0'))
        {
            return Err("invocation tokens must not contain NUL".to_owned());
        }
        validate_nonempty("cpu_model", &self.provenance.cpu_model)?;
        validate_availability("accelerator_runtime", &self.provenance.accelerator_runtime)?;
        validate_availability("gpu_model", &self.provenance.gpu_model)?;
        if self.stream_purposes.is_empty() {
            return Err("stream_purposes must not be empty".to_owned());
        }
        let mut purpose_names = BTreeSet::new();
        let mut purpose_tags = BTreeSet::new();
        for purpose in &self.stream_purposes {
            if !purpose_names.insert(&purpose.name) {
                return Err(format!("duplicate stream purpose name {}", purpose.name));
            }
            if !purpose_tags.insert(purpose.tag) {
                return Err(format!("duplicate stream purpose tag {}", purpose.tag));
            }
        }
        if self.cells.is_empty() {
            return Err("cells must not be empty".to_owned());
        }
        let mut cell_keys = BTreeSet::new();
        let mut all_streams = BTreeSet::new();
        for cell in &self.cells {
            validate_field(cell.q)?;
            if cell.n == 0 || cell.matrix_count == 0 || cell.shard_size == 0 {
                return Err(format!(
                    "cell ({},{}) has a zero size or count",
                    cell.q, cell.n
                ));
            }
            if !cell_keys.insert((cell.q, cell.n)) {
                return Err(format!("duplicate cell ({},{})", cell.q, cell.n));
            }
            let expected_shards = cell.matrix_count.div_ceil(cell.shard_size);
            if cell.shards.len() as u64 != expected_shards {
                return Err(format!(
                    "cell ({},{}) has {} shards; expected {expected_shards}",
                    cell.q,
                    cell.n,
                    cell.shards.len()
                ));
            }
            let mut shard_ids = BTreeSet::new();
            for shard in &cell.shards {
                if !shard_ids.insert(shard.shard_id) {
                    return Err(format!(
                        "cell ({},{}) repeats shard {}",
                        cell.q, cell.n, shard.shard_id
                    ));
                }
                if shard.stream_index >= 1_u64 << 56 {
                    return Err(format!(
                        "shard {} stream index exceeds 56 bits",
                        shard.shard_id
                    ));
                }
                if !all_streams.insert((cell.q, cell.n, shard.stream_index)) {
                    return Err(format!(
                        "cell ({},{}) repeats stream {}",
                        cell.q, cell.n, shard.stream_index
                    ));
                }
            }
        }
        Ok(())
    }
}

impl SchemaDocument for ShardRecord {
    fn version(&self) -> u32 {
        self.schema_version
    }

    fn validate(&self) -> Result<(), String> {
        validate_field(self.stream_address.q)?;
        if self.stream_address.n == 0 {
            return Err("stream matrix order must be non-zero".to_owned());
        }
        if self.stream_address.stream_index >= 1_u64 << 56 {
            return Err("stream index exceeds 56 bits".to_owned());
        }
        if self.permanent_histogram.len() != usize::from(self.stream_address.q) {
            return Err("permanent histogram length must equal q".to_owned());
        }
        let histogram_total = self
            .permanent_histogram
            .iter()
            .try_fold(0_u64, |total, count| total.checked_add(*count))
            .ok_or_else(|| "permanent histogram total overflows u64".to_owned())?;
        if histogram_total != self.matrix_count {
            return Err("permanent histogram total must equal matrix_count".to_owned());
        }
        if self.permanent_histogram[0] != self.permanent_zero_count {
            return Err("histogram residue-zero bin must equal permanent_zero_count".to_owned());
        }
        match self.determinant {
            DeterminantCount::NotEvaluated => {}
            DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            } => validate_counts(sample_count, zero_count, self.matrix_count)?,
        }
        Ok(())
    }
}

impl SchemaDocument for FieldSummary {
    fn version(&self) -> u32 {
        self.schema_version
    }

    fn validate(&self) -> Result<(), String> {
        validate_field(self.q)?;
        if self.rows.is_empty() {
            return Err("field summary rows must not be empty".to_owned());
        }
        let mut orders = BTreeSet::new();
        for row in &self.rows {
            row.validate()?;
            if row.q != self.q {
                return Err(format!(
                    "row q={} differs from field summary q={}",
                    row.q, self.q
                ));
            }
            if !orders.insert(row.n) {
                return Err(format!("duplicate summary row ({},{})", row.q, row.n));
            }
        }
        Ok(())
    }
}

impl SummaryRow {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "summary row schema version is {}",
                self.schema_version
            ));
        }
        validate_field(self.q)?;
        if self.n == 0 {
            return Err("summary row n must be non-zero".to_owned());
        }
        if self.permanent_zero_count > self.matrix_count {
            return Err("permanent_zero_count exceeds matrix_count".to_owned());
        }
        match &self.determinant {
            DeterminantCount::NotEvaluated => {}
            DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            } => {
                validate_counts(*sample_count, *zero_count, self.matrix_count)?;
            }
        }
        match (&self.terminal_state, &self.determinant) {
            (
                CellTerminalState::Completed {
                    permanent_estimate,
                    determinant_estimate,
                    ..
                },
                determinant_counts,
            ) => {
                if self.matrix_count == 0 {
                    return Err("completed summary row matrix_count must be non-zero".to_owned());
                }
                validate_estimate(*permanent_estimate)?;
                match (determinant_counts, determinant_estimate) {
                    (DeterminantCount::NotEvaluated, DeterminantEstimate::NotEvaluated) => {}
                    (
                        DeterminantCount::Evaluated { .. },
                        DeterminantEstimate::Evaluated { estimate, .. },
                    ) => validate_estimate(*estimate)?,
                    _ => {
                        return Err(
                            "completed determinant counts and estimate states differ".to_owned()
                        );
                    }
                }
            }
            (CellTerminalState::Halted { .. }, _) => {}
        }
        Ok(())
    }
}

fn validate_field(q: u8) -> Result<(), String> {
    if matches!(q, 3 | 5 | 7) {
        Ok(())
    } else {
        Err(format!("unsupported field order {q}; expected 3, 5, or 7"))
    }
}

fn validate_nonempty(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_availability(name: &str, value: &Availability<String>) -> Result<(), String> {
    if let Availability::Present { value } = value {
        validate_nonempty(name, value)?;
    }
    Ok(())
}

fn validate_counts(sample_count: u64, zero_count: u64, maximum: u64) -> Result<(), String> {
    if sample_count > maximum {
        return Err("determinant sample_count exceeds matrix_count".to_owned());
    }
    if zero_count > sample_count {
        return Err("determinant zero_count exceeds sample_count".to_owned());
    }
    Ok(())
}

fn validate_estimate(estimate: ProportionEstimate) -> Result<(), String> {
    let values = [
        estimate.point,
        estimate.interval.lower,
        estimate.interval.upper,
    ];
    if values
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err("estimate and interval endpoints must be finite probabilities".to_owned());
    }
    if estimate.interval.lower > estimate.point || estimate.point > estimate.interval.upper {
        return Err("estimate point must lie inside its interval".to_owned());
    }
    Ok(())
}

/// Strictly reads and validates the root manifest of a dataset directory.
///
/// The manifest declares the dataset's file layout and its recorded source
/// revision, both of which the integrity layer needs before the remaining
/// files can be checked. It is therefore readable on its own, without the
/// whole-dataset cross-document conformance [`conform_dataset`] performs.
pub fn read_manifest(root: &Path) -> Result<CampaignManifest, SchemaError> {
    read_json(&root.join(MANIFEST_FILE))
}

/// Strictly reads and validates one field arm's summary.
///
/// The integrity layer needs each cell's recorded terminal state to tell a
/// shard a halted cell never executed from one that has been lost, and that
/// state lives here. Like [`read_manifest`], this reads one document without
/// the whole-dataset conformance [`conform_dataset`] performs.
pub fn read_field_summary(root: &Path, q: u8) -> Result<FieldSummary, SchemaError> {
    read_json(&root.join(field_summary_file(q)))
}

/// Strictly reads and validates every required raw document in a dataset.
///
/// The function performs schema and cross-document conformance. Cryptographic
/// checksum verification is intentionally separate from this shape check and
/// belongs to [`verify_dataset`](super::provenance::verify_dataset).
pub fn conform_dataset(root: &Path) -> Result<ConformedDataset, SchemaError> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest = read_manifest(root)?;
    if root.file_name().and_then(|name| name.to_str()) != Some(manifest.campaign_id.0.as_str()) {
        return invalid_value(
            &manifest_path,
            "dataset directory name differs from campaign_id",
        );
    }
    let mut layout = DatasetLayout::from_manifest(&manifest);
    for required_file in layout.required_files() {
        if required_file.relative_path.starts_with("shards/") {
            continue;
        }
        let path = root.join(&required_file.relative_path);
        if !path.is_file() {
            return Err(SchemaError::MissingFile { path });
        }
    }

    let fields: BTreeSet<_> = manifest.cells.iter().map(|cell| cell.q).collect();
    let mut field_summaries = BTreeMap::new();
    let mut field_rows = BTreeMap::new();
    for q in fields {
        let path = root.join(field_summary_file(q));
        let summary: FieldSummary = read_json(&path)?;
        if summary.q != q {
            return invalid_value(&path, "field summary identity differs from its path");
        }
        for row in &summary.rows {
            let key = (row.q, row.n);
            if !manifest.cells.iter().any(|cell| (cell.q, cell.n) == key) {
                return invalid_value(&path, "summary row does not name a manifest cell");
            }
            if field_rows.insert(key, row.clone()).is_some() {
                return invalid_value(&path, "duplicate cell across field summaries");
            }
        }
        field_summaries.insert(q, summary);
    }
    if field_rows.len() != manifest.cells.len() {
        return invalid_value(
            &manifest_path,
            "not every manifest cell has one field-summary row",
        );
    }

    let purpose_tags: BTreeSet<_> = manifest
        .stream_purposes
        .iter()
        .map(|purpose| purpose.tag)
        .collect();
    let mut observed_shard_paths = BTreeSet::new();
    for cell in &manifest.cells {
        let row = field_rows
            .get(&(cell.q, cell.n))
            .expect("field-summary coverage checked above");
        let completed = matches!(row.terminal_state, CellTerminalState::Completed { .. });
        let mut cell_count = 0_u64;
        let mut permanent_zero_count = 0_u64;
        let mut determinant_sample_count = 0_u64;
        let mut determinant_zero_count = 0_u64;
        for (ordinal, shard_spec) in cell.shards.iter().enumerate() {
            let relative_path = shard_record_file(cell.q, cell.n, shard_spec.shard_id);
            let path = root.join(&relative_path);
            if !path.is_file() {
                if completed {
                    return Err(SchemaError::MissingFile { path });
                }
                continue;
            }
            observed_shard_paths.insert(PathBuf::from(&relative_path));
            let record: ShardRecord = read_json(&path)?;
            if record.shard_id != shard_spec.shard_id
                || record.stream_address.root_seed != manifest.root_seed
                || record.stream_address.q != cell.q
                || record.stream_address.n != cell.n
                || record.stream_address.stream_index != shard_spec.stream_index
                || !purpose_tags.contains(&record.stream_address.purpose_tag)
            {
                return invalid_value(
                    &path,
                    "shard identity or stream address differs from manifest",
                );
            }
            let consumed = u64::try_from(ordinal)
                .ok()
                .and_then(|index| index.checked_mul(cell.shard_size))
                .ok_or_else(|| SchemaError::InvalidValue {
                    path: path.clone(),
                    message: "shard ordinal overflows u64".to_owned(),
                })?;
            let expected_count = cell
                .matrix_count
                .saturating_sub(consumed)
                .min(cell.shard_size);
            if record.matrix_count != expected_count {
                return invalid_value(&path, "shard matrix_count differs from manifest partition");
            }
            cell_count = cell_count.checked_add(record.matrix_count).ok_or_else(|| {
                SchemaError::InvalidValue {
                    path: path.clone(),
                    message: "pooled shard matrix count overflows u64".to_owned(),
                }
            })?;
            permanent_zero_count = permanent_zero_count
                .checked_add(record.permanent_zero_count)
                .ok_or_else(|| SchemaError::InvalidValue {
                    path: path.clone(),
                    message: "pooled shard permanent-zero count overflows u64".to_owned(),
                })?;
            match (cell.determinant_companion, &record.determinant) {
                (DeterminantPlan::NotEvaluated, DeterminantCount::NotEvaluated) => {}
                (DeterminantPlan::NotEvaluated, DeterminantCount::Evaluated { .. }) => {
                    return invalid_value(
                        &path,
                        "shard determinant state contradicts not_evaluated determinant plan",
                    );
                }
                (
                    DeterminantPlan::Evaluate,
                    DeterminantCount::Evaluated {
                        sample_count,
                        zero_count,
                    },
                ) => {
                    determinant_sample_count = determinant_sample_count
                        .checked_add(*sample_count)
                        .ok_or_else(|| SchemaError::InvalidValue {
                            path: path.clone(),
                            message: "pooled determinant sample count overflows u64".to_owned(),
                        })?;
                    determinant_zero_count = determinant_zero_count
                        .checked_add(*zero_count)
                        .ok_or_else(|| SchemaError::InvalidValue {
                            path: path.clone(),
                            message: "pooled determinant zero count overflows u64".to_owned(),
                        })?;
                }
                (DeterminantPlan::Evaluate, DeterminantCount::NotEvaluated) => {
                    return invalid_value(
                        &path,
                        "shard determinant state contradicts evaluate determinant plan",
                    );
                }
            }
        }
        if completed && cell_count != cell.matrix_count {
            return invalid_value(
                &manifest_path,
                "completed pooled shard count differs from manifest cell count",
            );
        }
        let determinant = match cell.determinant_companion {
            DeterminantPlan::NotEvaluated => DeterminantAggregate::NotEvaluated,
            DeterminantPlan::Evaluate => DeterminantAggregate::Evaluated {
                sample_count: determinant_sample_count,
                zero_count: determinant_zero_count,
            },
        };
        let path = root.join(field_summary_file(cell.q));
        validate_summary_aggregate(
            &path,
            row,
            CellAggregate {
                matrix_count: cell_count,
                permanent_zero_count,
                determinant,
            },
        )?;
    }

    let actual_shard_paths = collect_shard_files(root)?;
    if let Some(path) = actual_shard_paths.difference(&observed_shard_paths).next() {
        return invalid_value(&root.join(path), "unmanifested shard path");
    }
    layout.required_files.retain(|required_file| {
        !required_file.relative_path.starts_with("shards/")
            || observed_shard_paths.contains(Path::new(&required_file.relative_path))
    });

    let pooled_path = root.join(POOLED_SUMMARY_FILE);
    let pooled_summary = decode_summary_csv(&pooled_path)?;
    let pooled_rows: BTreeMap<_, _> = pooled_summary
        .iter()
        .cloned()
        .map(|row| ((row.q, row.n), row))
        .collect();
    if pooled_rows.len() != pooled_summary.len() || pooled_rows != field_rows {
        return invalid_value(
            &pooled_path,
            "pooled summary must equal the field-summary rows",
        );
    }

    Ok(ConformedDataset {
        manifest,
        field_summaries,
        pooled_summary,
        layout,
    })
}

fn collect_shard_files(root: &Path) -> Result<BTreeSet<PathBuf>, SchemaError> {
    let shard_root = root.join("shards");
    if !shard_root.exists() {
        return Ok(BTreeSet::new());
    }
    if !shard_root.is_dir() {
        return invalid_value(&shard_root, "shards path must be a directory");
    }
    let mut files = BTreeSet::new();
    collect_files_below(root, &shard_root, &mut files)?;
    Ok(files)
}

fn collect_files_below(
    root: &Path,
    directory: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), SchemaError> {
    let entries = fs::read_dir(directory).map_err(|source| SchemaError::Io {
        path: directory.to_owned(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| SchemaError::Io {
            path: directory.to_owned(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| SchemaError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            collect_files_below(root, &path, files)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| SchemaError::InvalidValue {
                    path: path.clone(),
                    message: "shard path is outside the dataset root".to_owned(),
                })?;
            files.insert(relative.to_owned());
        }
    }
    Ok(())
}

fn validate_summary_aggregate(
    path: &Path,
    row: &SummaryRow,
    aggregate: CellAggregate,
) -> Result<(), SchemaError> {
    if row.matrix_count != aggregate.matrix_count {
        return invalid_value(path, "summary matrix_count differs from pooled shards");
    }
    if row.permanent_zero_count != aggregate.permanent_zero_count {
        return invalid_value(
            path,
            "summary permanent_zero_count differs from pooled shards",
        );
    }
    match (aggregate.determinant, &row.determinant) {
        (DeterminantAggregate::NotEvaluated, DeterminantCount::NotEvaluated) => Ok(()),
        (DeterminantAggregate::NotEvaluated, DeterminantCount::Evaluated { .. }) => invalid_value(
            path,
            "summary determinant state contradicts not_evaluated determinant plan",
        ),
        (
            DeterminantAggregate::Evaluated {
                sample_count: pooled_samples,
                zero_count: pooled_zeros,
            },
            DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            },
        ) if *sample_count == pooled_samples && *zero_count == pooled_zeros => Ok(()),
        (DeterminantAggregate::Evaluated { .. }, DeterminantCount::Evaluated { .. }) => {
            invalid_value(path, "summary determinant counts differ from pooled shards")
        }
        (DeterminantAggregate::Evaluated { .. }, DeterminantCount::NotEvaluated) => invalid_value(
            path,
            "summary determinant state contradicts evaluate determinant plan",
        ),
    }
}

fn read_json<T>(path: &Path) -> Result<T, SchemaError>
where
    T: DeserializeOwned + SchemaDocument,
{
    let bytes = fs::read(path).map_err(|source| SchemaError::Io {
        path: path.to_owned(),
        source,
    })?;
    let document: T =
        serde_json::from_slice(&bytes).map_err(|source| SchemaError::InvalidDocument {
            path: path.to_owned(),
            message: source.to_string(),
        })?;
    if document.version() != SCHEMA_VERSION {
        return Err(SchemaError::UnsupportedVersion {
            path: path.to_owned(),
            found: document.version(),
        });
    }
    document
        .validate()
        .map_err(|message| SchemaError::InvalidValue {
            path: path.to_owned(),
            message,
        })?;
    Ok(document)
}

fn invalid_value<T>(path: &Path, message: &str) -> Result<T, SchemaError> {
    Err(SchemaError::InvalidValue {
        path: path.to_owned(),
        message: message.to_owned(),
    })
}

/// Encodes summary rows with the exact [`SUMMARY_CSV_FIELDS`] header.
///
/// All fields are numeric or closed-vocabulary tokens, so CSV quoting is not
/// needed. A determinant not-evaluated state emits empty count, estimate,
/// interval, and verdict cells rather than numeric zeroes.
pub fn encode_summary_csv(rows: &[SummaryRow]) -> String {
    let mut output = String::new();
    output.push_str(&SUMMARY_CSV_FIELDS.join(","));
    output.push('\n');
    for row in rows {
        let (det_state, det_sample, det_zero) = match &row.determinant {
            DeterminantCount::NotEvaluated => {
                ("not_evaluated".to_owned(), String::new(), String::new())
            }
            DeterminantCount::Evaluated {
                sample_count,
                zero_count,
            } => (
                "evaluated".to_owned(),
                sample_count.to_string(),
                zero_count.to_string(),
            ),
        };
        let (
            permanent_point,
            permanent_lower,
            permanent_upper,
            permanent_verdict,
            det_point,
            det_lower,
            det_upper,
            det_verdict,
            terminal_state,
            halt_reason,
        ) = match row.terminal_state {
            CellTerminalState::Completed {
                permanent_estimate,
                permanent_verdict,
                determinant_estimate,
            } => {
                let (det_point, det_lower, det_upper, det_verdict) = match determinant_estimate {
                    DeterminantEstimate::NotEvaluated => {
                        (String::new(), String::new(), String::new(), String::new())
                    }
                    DeterminantEstimate::Evaluated { estimate, verdict } => (
                        estimate.point.to_string(),
                        estimate.interval.lower.to_string(),
                        estimate.interval.upper.to_string(),
                        verdict.as_str().to_owned(),
                    ),
                };
                (
                    permanent_estimate.point.to_string(),
                    permanent_estimate.interval.lower.to_string(),
                    permanent_estimate.interval.upper.to_string(),
                    permanent_verdict.as_str().to_owned(),
                    det_point,
                    det_lower,
                    det_upper,
                    det_verdict,
                    "completed",
                    "",
                )
            }
            CellTerminalState::Halted { reason } => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "halted",
                reason.as_str(),
            ),
        };
        let fields = [
            row.schema_version.to_string(),
            row.q.to_string(),
            row.n.to_string(),
            row.matrix_count.to_string(),
            row.permanent_zero_count.to_string(),
            permanent_point,
            permanent_lower,
            permanent_upper,
            permanent_verdict,
            det_state,
            det_sample,
            det_zero,
            det_point,
            det_lower,
            det_upper,
            det_verdict,
            terminal_state.to_owned(),
            halt_reason.to_owned(),
        ];
        output.push_str(&fields.join(","));
        output.push('\n');
    }
    output
}

fn decode_summary_csv(path: &Path) -> Result<Vec<SummaryRow>, SchemaError> {
    let text = fs::read_to_string(path).map_err(|source| SchemaError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default().trim_end_matches('\r');
    if header != SUMMARY_CSV_FIELDS.join(",") {
        return Err(SchemaError::InvalidDocument {
            path: path.to_owned(),
            message: "CSV header has a missing, unknown, or reordered field".to_owned(),
        });
    }
    let mut rows = Vec::new();
    for (index, line) in lines.enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split(',').collect();
        if fields.len() != SUMMARY_CSV_FIELDS.len() {
            return Err(SchemaError::InvalidDocument {
                path: path.to_owned(),
                message: format!("CSV row {} has {} fields", index + 2, fields.len()),
            });
        }
        let row = parse_summary_row(&fields).map_err(|message| SchemaError::InvalidDocument {
            path: path.to_owned(),
            message: format!("CSV row {}: {message}", index + 2),
        })?;
        if row.schema_version != SCHEMA_VERSION {
            return Err(SchemaError::UnsupportedVersion {
                path: path.to_owned(),
                found: row.schema_version,
            });
        }
        row.validate()
            .map_err(|message| SchemaError::InvalidValue {
                path: path.to_owned(),
                message: format!("CSV row {}: {message}", index + 2),
            })?;
        rows.push(row);
    }
    if rows.is_empty() {
        return invalid_value(path, "pooled summary must contain at least one row");
    }
    Ok(rows)
}

fn parse_summary_row(fields: &[&str]) -> Result<SummaryRow, String> {
    let determinant = match fields[9] {
        "not_evaluated" => {
            if fields[10..12].iter().any(|field| !field.is_empty()) {
                return Err("not_evaluated determinant has numeric count fields".to_owned());
            }
            DeterminantCount::NotEvaluated
        }
        "evaluated" => DeterminantCount::Evaluated {
            sample_count: parse_field(fields, 10)?,
            zero_count: parse_field(fields, 11)?,
        },
        other => return Err(format!("unknown determinant_state {other:?}")),
    };
    let terminal_state = match fields[16] {
        "completed" if fields[17].is_empty() => {
            let determinant_estimate = match &determinant {
                DeterminantCount::NotEvaluated => {
                    if fields[12..16].iter().any(|field| !field.is_empty()) {
                        return Err(
                            "completed not_evaluated determinant has estimate fields".to_owned()
                        );
                    }
                    DeterminantEstimate::NotEvaluated
                }
                DeterminantCount::Evaluated { .. } => DeterminantEstimate::Evaluated {
                    estimate: ProportionEstimate {
                        point: parse_field(fields, 12)?,
                        interval: Interval {
                            lower: parse_field(fields, 13)?,
                            upper: parse_field(fields, 14)?,
                        },
                    },
                    verdict: fields[15].parse()?,
                },
            };
            CellTerminalState::Completed {
                permanent_estimate: ProportionEstimate {
                    point: parse_field(fields, 5)?,
                    interval: Interval {
                        lower: parse_field(fields, 6)?,
                        upper: parse_field(fields, 7)?,
                    },
                },
                permanent_verdict: fields[8].parse()?,
                determinant_estimate,
            }
        }
        "halted" => {
            if fields[5..9].iter().any(|field| !field.is_empty())
                || fields[12..16].iter().any(|field| !field.is_empty())
            {
                return Err("halted row has completed estimate or verdict fields".to_owned());
            }
            CellTerminalState::Halted {
                reason: fields[17].parse()?,
            }
        }
        "completed" => return Err("completed row has a halt_reason".to_owned()),
        other => return Err(format!("unknown terminal_state {other:?}")),
    };
    Ok(SummaryRow {
        schema_version: parse_field(fields, 0)?,
        q: parse_field(fields, 1)?,
        n: parse_field(fields, 2)?,
        matrix_count: parse_field(fields, 3)?,
        permanent_zero_count: parse_field(fields, 4)?,
        determinant,
        terminal_state,
    })
}

fn parse_field<T: FromStr>(fields: &[&str], index: usize) -> Result<T, String>
where
    T::Err: fmt::Display,
{
    fields[index]
        .parse()
        .map_err(|error| format!("invalid {}: {error}", SUMMARY_CSV_FIELDS[index]))
}

impl AcceptanceVerdict {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

impl FromStr for AcceptanceVerdict {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("unknown acceptance verdict {value:?}")),
        }
    }
}

impl HaltReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::AcceptanceFailure => "acceptance_failure",
            Self::BackendUnavailable => "backend_unavailable",
            Self::ExecutionFailure => "execution_failure",
        }
    }
}

impl FromStr for HaltReason {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "acceptance_failure" => Ok(Self::AcceptanceFailure),
            "backend_unavailable" => Ok(Self::BackendUnavailable),
            "execution_failure" => Ok(Self::ExecutionFailure),
            _ => Err(format!("unknown halt reason {value:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use serde_json::{json, Value};

    use super::super::fixture::{
        manifest, shard, summary_row, write_fixture, write_halted_summary,
        write_multifield_fixture, TestDir,
    };
    use super::*;

    fn read_field_summary(root: &Path) -> FieldSummary {
        serde_json::from_slice(&fs::read(root.join("summaries/q3.json")).unwrap()).unwrap()
    }

    fn write_field_and_pooled_summaries(root: &Path, summary: &FieldSummary) {
        fs::write(
            root.join("summaries/q3.json"),
            serde_json::to_vec_pretty(summary).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(POOLED_SUMMARY_FILE),
            encode_summary_csv(&summary.rows),
        )
        .unwrap();
    }

    fn set_manifest_determinant_plan(root: &Path, plan: DeterminantPlan) {
        let path = root.join(MANIFEST_FILE);
        let mut manifest: CampaignManifest =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        manifest.cells[0].determinant_companion = plan;
        fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    }

    fn set_shard_determinant(root: &Path, shard_id: u64, determinant: DeterminantCount) {
        let path = root.join(format!("shards/q3/n04/shard-{shard_id:06}.json"));
        let mut shard: ShardRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        shard.determinant = determinant;
        fs::write(path, serde_json::to_vec_pretty(&shard).unwrap()).unwrap();
    }

    fn set_coherent_evaluated_determinants(root: &Path) {
        set_manifest_determinant_plan(root, DeterminantPlan::Evaluate);
        for shard_id in 0..2 {
            set_shard_determinant(
                root,
                shard_id,
                DeterminantCount::Evaluated {
                    sample_count: 10,
                    zero_count: 4,
                },
            );
        }
        let mut summary = read_field_summary(root);
        summary.rows[0].determinant = DeterminantCount::Evaluated {
            sample_count: 20,
            zero_count: 8,
        };
        summary.rows[0].terminal_state = CellTerminalState::Completed {
            permanent_estimate: ProportionEstimate {
                point: 0.3,
                interval: Interval {
                    lower: 0.145,
                    upper: 0.519,
                },
            },
            permanent_verdict: AcceptanceVerdict::Accepted,
            determinant_estimate: DeterminantEstimate::Evaluated {
                estimate: ProportionEstimate {
                    point: 0.4,
                    interval: Interval {
                        lower: 0.2,
                        upper: 0.6,
                    },
                },
                verdict: AcceptanceVerdict::Accepted,
            },
        };
        write_field_and_pooled_summaries(root, &summary);
    }

    #[test]
    fn well_formed_dataset_conforms_and_has_one_writer_per_raw_path() {
        let fixture = TestDir::new();
        write_fixture(fixture.root());

        let dataset = conform_dataset(fixture.root()).expect("well-formed fixture must conform");
        assert_eq!(dataset.manifest, manifest());

        let required_files = dataset.layout.required_files();
        assert_eq!(required_files.len(), 6);
        assert_eq!(
            required_files
                .iter()
                .filter(|entry| entry.class == DatasetFileClass::RawData)
                .count(),
            5
        );
        let unique_paths: BTreeSet<_> = required_files
            .iter()
            .map(|entry| &entry.relative_path)
            .collect();
        assert_eq!(unique_paths.len(), required_files.len());
        assert!(required_files.iter().all(|entry| match entry.writer {
            WriterRole::FieldExecution { q } => {
                entry.relative_path.starts_with(&format!("shards/q{q}/"))
                    || entry.relative_path == format!("summaries/q{q}.json")
            }
            WriterRole::Finalization => {
                matches!(
                    entry.relative_path.as_str(),
                    MANIFEST_FILE | POOLED_SUMMARY_FILE | INTEGRITY_FILE
                )
            }
        }));
    }

    #[test]
    fn conformance_binds_campaign_id_to_directory_basename() {
        let fixture = TestDir::new();
        write_fixture(fixture.root());
        let manifest_path = fixture.root().join(MANIFEST_FILE);
        let mut value: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        value["campaign_id"] = json!("different-campaign");
        fs::write(&manifest_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let error = conform_dataset(fixture.root()).unwrap_err().to_string();
        assert!(
            error.contains("dataset directory name differs from campaign_id"),
            "unexpected campaign directory identity error: {error}"
        );
    }

    #[test]
    fn conformance_rejects_field_summary_files_swapped_between_fields() {
        let fixture = TestDir::new();
        write_multifield_fixture(fixture.root());
        conform_dataset(fixture.root()).expect("well-formed multi-field fixture must conform");

        let q3_path = fixture.root().join("summaries/q3.json");
        let q5_path = fixture.root().join("summaries/q5.json");
        let q3_bytes = fs::read(&q3_path).unwrap();
        let q5_bytes = fs::read(&q5_path).unwrap();
        fs::write(&q3_path, q5_bytes).unwrap();
        fs::write(&q5_path, q3_bytes).unwrap();

        let error = conform_dataset(fixture.root()).unwrap_err().to_string();
        assert!(
            error.contains("field summary identity differs from its path"),
            "unexpected swapped field-summary error: {error}"
        );
    }

    #[test]
    fn conformance_rejects_field_summary_rows_swapped_between_fields() {
        let fixture = TestDir::new();
        write_multifield_fixture(fixture.root());
        let q3_path = fixture.root().join("summaries/q3.json");
        let q5_path = fixture.root().join("summaries/q5.json");
        let mut q3: FieldSummary = serde_json::from_slice(&fs::read(&q3_path).unwrap()).unwrap();
        let mut q5: FieldSummary = serde_json::from_slice(&fs::read(&q5_path).unwrap()).unwrap();
        std::mem::swap(&mut q3.rows, &mut q5.rows);
        fs::write(&q3_path, serde_json::to_vec_pretty(&q3).unwrap()).unwrap();
        fs::write(&q5_path, serde_json::to_vec_pretty(&q5).unwrap()).unwrap();

        let error = conform_dataset(fixture.root()).unwrap_err().to_string();
        assert!(
            error.contains("differs from field summary"),
            "unexpected swapped field-summary row error: {error}"
        );
    }

    #[test]
    fn halted_partial_cell_preserves_counts_without_completed_metrics() {
        let fixture = TestDir::new();
        write_fixture(fixture.root());
        fs::remove_file(fixture.root().join("shards/q3/n04/shard-000001.json")).unwrap();
        write_halted_summary(
            fixture.root(),
            10,
            3,
            DeterminantCount::NotEvaluated,
            HaltReason::ExecutionFailure,
        );

        let dataset = conform_dataset(fixture.root()).expect("halted partial cell must conform");
        assert_eq!(dataset.pooled_summary[0].matrix_count, 10);
        assert_eq!(dataset.pooled_summary[0].permanent_zero_count, 3);
        assert!(dataset
            .layout
            .required_files()
            .iter()
            .all(|file| { file.relative_path != "shards/q3/n04/shard-000001.json" }));
    }

    #[test]
    fn propagated_acceptance_failure_can_halt_before_any_shard_executes() {
        let fixture = TestDir::new();
        write_fixture(fixture.root());
        set_manifest_determinant_plan(fixture.root(), DeterminantPlan::Evaluate);
        for shard_id in 0..2 {
            fs::remove_file(
                fixture
                    .root()
                    .join(format!("shards/q3/n04/shard-{shard_id:06}.json")),
            )
            .unwrap();
        }
        write_halted_summary(
            fixture.root(),
            0,
            0,
            DeterminantCount::Evaluated {
                sample_count: 0,
                zero_count: 0,
            },
            HaltReason::AcceptanceFailure,
        );

        let dataset =
            conform_dataset(fixture.root()).expect("zero-shard propagated halt must conform");
        assert!(dataset
            .layout
            .required_files()
            .iter()
            .all(|file| !file.relative_path.starts_with("shards/")));
    }

    #[test]
    fn halted_cell_accepts_any_manifested_shard_subset() {
        let fixture = TestDir::new();
        write_fixture(fixture.root());
        fs::remove_file(fixture.root().join("shards/q3/n04/shard-000000.json")).unwrap();
        write_halted_summary(
            fixture.root(),
            10,
            3,
            DeterminantCount::NotEvaluated,
            HaltReason::ExecutionFailure,
        );

        let dataset =
            conform_dataset(fixture.root()).expect("halted shards need not form a prefix");
        assert!(dataset
            .layout
            .required_files()
            .iter()
            .any(|file| { file.relative_path == "shards/q3/n04/shard-000001.json" }));
    }

    #[test]
    fn conformance_rejects_silent_or_completed_partial_cells() {
        let silent = TestDir::new();
        write_fixture(silent.root());
        let path = silent.root().join("summaries/q3.json");
        let mut summary: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        summary["rows"] = json!([]);
        fs::write(&path, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
        assert!(conform_dataset(silent.root()).is_err());

        let partial = TestDir::new();
        write_fixture(partial.root());
        fs::remove_file(partial.root().join("shards/q3/n04/shard-000001.json")).unwrap();
        assert!(conform_dataset(partial.root()).is_err());
    }

    #[test]
    fn conformance_rejects_unmanifested_shard_paths() {
        let fixture = TestDir::new();
        write_fixture(fixture.root());
        fs::copy(
            fixture.root().join("shards/q3/n04/shard-000000.json"),
            fixture.root().join("shards/q3/n04/shard-999999.json"),
        )
        .unwrap();

        let error = conform_dataset(fixture.root()).unwrap_err().to_string();
        assert!(
            error.contains("unmanifested shard path"),
            "unexpected unmanifested shard error: {error}"
        );
    }

    #[test]
    fn conformance_rejects_missing_unknown_and_wrong_version_fields() {
        for mutation in ["missing", "unknown", "version"] {
            let fixture = TestDir::new();
            write_fixture(fixture.root());
            let manifest_path = fixture.root().join(MANIFEST_FILE);
            let mut value: Value =
                serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
            match mutation {
                "missing" => {
                    value.as_object_mut().unwrap().remove("root_seed");
                }
                "unknown" => {
                    value["interpretation"] = json!("the curve confirms the conjecture");
                }
                "version" => value["schema_version"] = json!(SCHEMA_VERSION + 1),
                _ => unreachable!(),
            }
            fs::write(&manifest_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

            assert!(
                conform_dataset(fixture.root()).is_err(),
                "{mutation} manifest mutation must be rejected"
            );
        }
    }

    #[test]
    fn conformance_requires_backend_receipt_and_rng_reproduction_fields() {
        for missing_field in [
            "backend_receipt",
            "rng_algorithm",
            "rng_version",
            "invocation",
        ] {
            let fixture = TestDir::new();
            write_fixture(fixture.root());
            let manifest_path = fixture.root().join(MANIFEST_FILE);
            let mut value: Value =
                serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
            let object = if missing_field == "backend_receipt" {
                value["cells"][0].as_object_mut().unwrap()
            } else {
                value["provenance"].as_object_mut().unwrap()
            };
            object.remove(missing_field);
            fs::write(&manifest_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

            assert!(
                conform_dataset(fixture.root()).is_err(),
                "manifest without {missing_field} must be rejected"
            );
        }
    }

    #[test]
    fn conformance_rejects_noncanonical_backend_receipt_identity() {
        for mutation in [
            "missing_path",
            "missing_digest",
            "absolute",
            "traversal",
            "uppercase_digest",
            "unknown",
        ] {
            let fixture = TestDir::new();
            write_fixture(fixture.root());
            let manifest_path = fixture.root().join(MANIFEST_FILE);
            let mut value: Value =
                serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
            let receipt = &mut value["cells"][0]["backend_receipt"];
            match mutation {
                "missing_path" => {
                    receipt.as_object_mut().unwrap().remove("path");
                }
                "missing_digest" => {
                    receipt.as_object_mut().unwrap().remove("sha256");
                }
                "absolute" => receipt["path"] = json!("/tmp/selection.json"),
                "traversal" => receipt["path"] = json!("dev/../selection.json"),
                "uppercase_digest" => {
                    receipt["sha256"] =
                        json!("0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF");
                }
                "unknown" => receipt["media_type"] = json!("application/json"),
                _ => unreachable!(),
            }
            fs::write(&manifest_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

            assert!(
                conform_dataset(fixture.root()).is_err(),
                "{mutation} backend receipt identity must be rejected"
            );
        }
    }

    #[test]
    fn conformance_rejects_incomplete_rng_reproduction_provenance() {
        for mutation in [
            "empty_version",
            "empty_invocation",
            "empty_executable",
            "unknown_rng",
        ] {
            let fixture = TestDir::new();
            write_fixture(fixture.root());
            let manifest_path = fixture.root().join(MANIFEST_FILE);
            let mut value: Value =
                serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
            match mutation {
                "empty_version" => value["provenance"]["rng_version"] = json!(""),
                "empty_invocation" => value["provenance"]["invocation"] = json!([]),
                "empty_executable" => {
                    value["provenance"]["invocation"] = json!(["", "--manifest", "manifest.json"]);
                }
                "unknown_rng" => value["provenance"]["rng_algorithm"] = json!("system_random"),
                _ => unreachable!(),
            }
            fs::write(&manifest_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

            assert!(
                conform_dataset(fixture.root()).is_err(),
                "{mutation} RNG provenance must be rejected"
            );
        }
    }

    #[test]
    fn conformance_rejects_summary_permanent_zeros_not_pooled_from_shards() {
        let fixture = TestDir::new();
        write_fixture(fixture.root());
        let mut summary = read_field_summary(fixture.root());
        summary.rows[0].permanent_zero_count += 1;
        write_field_and_pooled_summaries(fixture.root(), &summary);

        let error = conform_dataset(fixture.root()).unwrap_err().to_string();
        assert!(
            error.contains("permanent_zero_count differs from pooled shards"),
            "unexpected conformance error: {error}"
        );
    }

    #[test]
    fn conformance_rejects_numeric_determinants_for_not_evaluated_plan() {
        for surface in ["shard", "summary"] {
            let fixture = TestDir::new();
            write_fixture(fixture.root());
            if surface == "shard" {
                set_shard_determinant(
                    fixture.root(),
                    0,
                    DeterminantCount::Evaluated {
                        sample_count: 10,
                        zero_count: 0,
                    },
                );
            } else {
                let mut summary = read_field_summary(fixture.root());
                summary.rows[0].determinant = DeterminantCount::Evaluated {
                    sample_count: 20,
                    zero_count: 0,
                };
                summary.rows[0].terminal_state = CellTerminalState::Completed {
                    permanent_estimate: ProportionEstimate {
                        point: 0.3,
                        interval: Interval {
                            lower: 0.145,
                            upper: 0.519,
                        },
                    },
                    permanent_verdict: AcceptanceVerdict::Accepted,
                    determinant_estimate: DeterminantEstimate::Evaluated {
                        estimate: ProportionEstimate {
                            point: 0.0,
                            interval: Interval {
                                lower: 0.0,
                                upper: 0.2,
                            },
                        },
                        verdict: AcceptanceVerdict::Accepted,
                    },
                };
                write_field_and_pooled_summaries(fixture.root(), &summary);
            }

            let error = conform_dataset(fixture.root()).unwrap_err().to_string();
            assert!(
                error.contains("not_evaluated determinant plan"),
                "{surface} mismatch returned unexpected error: {error}"
            );
        }
    }

    #[test]
    fn conformance_rejects_evaluated_plan_state_and_pooled_count_mismatches() {
        let missing_state = TestDir::new();
        write_fixture(missing_state.root());
        set_manifest_determinant_plan(missing_state.root(), DeterminantPlan::Evaluate);
        let error = conform_dataset(missing_state.root())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("evaluate determinant plan"),
            "unexpected evaluated-plan state error: {error}"
        );

        let coherent = TestDir::new();
        write_fixture(coherent.root());
        set_coherent_evaluated_determinants(coherent.root());
        conform_dataset(coherent.root()).expect("coherently pooled evaluated plan must conform");

        let wrong_pool = TestDir::new();
        write_fixture(wrong_pool.root());
        set_coherent_evaluated_determinants(wrong_pool.root());
        let mut summary = read_field_summary(wrong_pool.root());
        summary.rows[0].determinant = DeterminantCount::Evaluated {
            sample_count: 20,
            zero_count: 7,
        };
        write_field_and_pooled_summaries(wrong_pool.root(), &summary);

        let error = conform_dataset(wrong_pool.root()).unwrap_err().to_string();
        assert!(
            error.contains("determinant counts differ from pooled shards"),
            "unexpected determinant pooling error: {error}"
        );
    }

    #[test]
    fn not_evaluated_determinant_never_serializes_numeric_counts() {
        let shard_value = serde_json::to_value(shard(0, 100)).unwrap();
        let determinant = shard_value["determinant"].as_object().unwrap();
        assert_eq!(determinant.get("state"), Some(&json!("not_evaluated")));
        assert!(!determinant.contains_key("sample_count"));
        assert!(!determinant.contains_key("zero_count"));

        let mut invalid = shard_value;
        invalid["determinant"] = json!({"state": "not_evaluated", "zero_count": 0});
        assert!(serde_json::from_value::<ShardRecord>(invalid).is_err());

        let mut evaluated = shard(0, 100);
        evaluated.determinant = DeterminantCount::Evaluated {
            sample_count: 10,
            zero_count: 0,
        };
        let round_trip: ShardRecord =
            serde_json::from_value(serde_json::to_value(&evaluated).unwrap()).unwrap();
        assert_eq!(round_trip, evaluated);

        let csv = encode_summary_csv(&[summary_row()]);
        let row = csv.lines().nth(1).unwrap().split(',').collect::<Vec<_>>();
        let determinant_columns = [10, 11, 12, 13, 14, 15];
        assert!(determinant_columns
            .iter()
            .all(|column| row[*column].is_empty()));
        assert_eq!(row[9], "not_evaluated");
    }

    #[test]
    fn halted_terminal_outcome_forbids_completed_metrics() {
        let halted = CellTerminalState::Halted {
            reason: HaltReason::ExecutionFailure,
        };
        let mut value = serde_json::to_value(halted).unwrap();
        let object = value.as_object().unwrap();
        assert!(!object.contains_key("permanent_estimate"));
        assert!(!object.contains_key("permanent_verdict"));
        assert!(!object.contains_key("determinant_estimate"));

        value["permanent_verdict"] = json!("rejected");
        assert!(serde_json::from_value::<CellTerminalState>(value).is_err());
    }

    #[test]
    fn schema_field_names_admit_only_data_and_mechanical_provenance() {
        let banned = [
            "claim",
            "comment",
            "conclusion",
            "description",
            "interpretation",
            "meaning",
            "narrative",
            "note",
            "rationale",
        ];
        let declared: BTreeSet<_> = schema_field_names().collect();
        let documents = [
            serde_json::to_value(manifest()).unwrap(),
            serde_json::to_value(shard(0, 100)).unwrap(),
            serde_json::to_value(FieldSummary {
                schema_version: SCHEMA_VERSION,
                q: 3,
                rows: vec![summary_row()],
            })
            .unwrap(),
            serde_json::to_value(DeterminantEstimate::Evaluated {
                estimate: ProportionEstimate {
                    point: 0.0,
                    interval: Interval {
                        lower: 0.0,
                        upper: 0.2,
                    },
                },
                verdict: AcceptanceVerdict::Accepted,
            })
            .unwrap(),
            serde_json::to_value(CellTerminalState::Halted {
                reason: HaltReason::BackendUnavailable,
            })
            .unwrap(),
            serde_json::to_value(Availability::Present {
                value: "runtime 1".to_owned(),
            })
            .unwrap(),
        ];
        let mut serialized = BTreeSet::new();
        for document in &documents {
            collect_json_keys(document, &mut serialized);
        }
        assert!(serialized.is_subset(&declared));

        for field in declared {
            assert!(
                banned.iter().all(|word| !field.contains(word)),
                "schema field {field:?} admits interpretive prose"
            );
        }
    }

    fn collect_json_keys<'a>(value: &'a Value, keys: &mut BTreeSet<&'a str>) {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    keys.insert(key);
                    collect_json_keys(child, keys);
                }
            }
            Value::Array(array) => {
                for child in array {
                    collect_json_keys(child, keys);
                }
            }
            _ => {}
        }
    }
}
