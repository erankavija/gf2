//! Deterministic, resumable ordered-statistics-decoding campaigns.
//!
//! This module owns the campaign mechanics shared by OSD executables: explicit
//! cell identities, order-independent seed derivation, bit- and block-error
//! accounting, block-sampled intervals for BER and BLER, strict checkpoint and
//! receipt schemas, and cell-boundary resume. Domain executables retain only
//! code/channel construction and one evaluator closure. Persistence delegates
//! to [`crate::checkpoint`], while the exact binomial endpoints delegate to
//! [`gf2_stats::intervals`].
//!
//! An interrupted evaluation reports cumulative counters. On the next call,
//! the evaluator receives those counters in [`OsdCellExecution::resume`] and
//! the same cell seed, so it can continue at the next sample. Earlier attempts
//! remain in the receipt. Completed, censored, exhausted, and contradictory
//! cells are terminal evidence and are never evaluated again.
//!
//! # Interval coverage and schema history
//!
//! Schema 2 treats one decoded block as the independent sampling unit for both
//! intervals. BLER uses an equal-tailed Clopper-Pearson interval. BER is the
//! mean of the per-block information-bit error fractions and uses a two-sided,
//! time-uniform Hoeffding confidence sequence for independent observations in
//! `[0, 1]`. At block count `n`, it spends error probability
//! `alpha / (n * (n + 1))`; the sum over all possible stopping times is
//! `alpha`. Its coverage therefore survives the block-error stopping rule and
//! permits arbitrary dependence among bit errors inside a block. It requires
//! independent, identically distributed blocks with a fixed information-block
//! length and is conservative because it assumes no within-block error model.
//!
//! Schema 1 receipts remain deserializable as historical evidence. Their BER
//! intervals used Clopper-Pearson over individual bits without naming a
//! sampling unit, so their stated coverage does not apply to clustered decoder
//! error bursts. Their BLER intervals remain valid block-level intervals.
//! Schema 1 also stored one usually executable-only invocation in the global
//! runtime provenance, and its `cpu_model` may contain an OS/architecture
//! platform token. Schema 2 records the full argument vector and runtime
//! provenance for every invocation that contributes an attempt, plus observed
//! processor topology.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

use crate::checkpoint::{
    CheckpointLoadError, CheckpointPayload, CheckpointReader, CheckpointWriter,
};
use crate::permanent_campaign::schema::{ArtifactIdentity, Provenance};

/// The schema version shared by OSD checkpoints, receipts, and cell records.
pub const OSD_CAMPAIGN_SCHEMA_VERSION: u32 = 2;

const CHECKPOINT_IDENTITY: &str = "osd-campaign/cell-progress";
const CELL_SEED_DOMAIN: &[u8] = b"gf2-sim/osd-campaign/cell-seed/v1\0";

/// Stable identity of one OSD campaign cell.
///
/// The identity is part of deterministic seed derivation and must therefore
/// remain unchanged when cells are reordered. It contains lowercase ASCII
/// letters or digits separated by single hyphens.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct OsdCellId(String);

impl OsdCellId {
    /// Returns the canonical serialization token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for OsdCellId {
    type Err = OsdCellIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let valid = !value.is_empty()
            && !value.starts_with('-')
            && !value.ends_with('-')
            && !value.contains("--")
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(OsdCellIdError(value.to_owned()))
        }
    }
}

impl fmt::Display for OsdCellId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for OsdCellId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned for a non-canonical OSD cell identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsdCellIdError(String);

impl fmt::Display for OsdCellIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "OSD cell id {:?} must contain lowercase ASCII letters or digits separated by single hyphens",
            self.0
        )
    }
}

impl std::error::Error for OsdCellIdError {}

/// One explicit point in an OSD campaign grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdCell {
    /// Stable cell identity used for checkpoint matching and seed derivation.
    pub id: OsdCellId,
    /// Channel point in decibels of energy per information bit to noise.
    pub eb_n0_db: f64,
    /// OSD reprocessing order exercised by this cell.
    pub osd_order: u8,
    /// Digitization precision in base-10 logarithmic decades.
    pub digitization_precision: f64,
}

impl OsdCell {
    /// Constructs a validated explicit campaign cell.
    ///
    /// # Errors
    ///
    /// Returns [`OsdCampaignError::InvalidConfiguration`] when `eb_n0_db` is
    /// not finite or `digitization_precision` is negative or non-finite.
    pub fn new(
        id: OsdCellId,
        eb_n0_db: f64,
        osd_order: u8,
        digitization_precision: f64,
    ) -> Result<Self, OsdCampaignError> {
        let cell = Self {
            id,
            eb_n0_db,
            osd_order,
            digitization_precision,
        };
        validate_cell(&cell)?;
        Ok(cell)
    }
}

/// Binomial confidence-interval implementation used by a campaign.
///
/// The closed vocabulary prevents a receipt from naming an estimator other
/// than the one actually dispatched through the shared `gf2-stats` API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinomialIntervalMethod {
    /// Equal-tailed exact Clopper-Pearson interval.
    ClopperPearson,
}

/// Independent sampling unit supporting a receipted interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntervalSamplingUnit {
    /// One decoded code block.
    Block,
}

/// Named confidence-interval estimator recorded with interval endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceIntervalEstimator {
    /// Equal-tailed exact interval for a binomial block-error count.
    ClopperPearson,
    /// Time-uniform two-sided Hoeffding sequence for bounded block means.
    HoeffdingBlockMeanSequence,
}

/// Unit used by every OSD published-curve digitization precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigitizationPrecisionUnit {
    /// Additive width in base-10 logarithmic space.
    Log10Decades,
}

/// Exact BLER interval method and confidence level shared by both cell rates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinomialIntervalSpec {
    /// Named exact estimator used for BLER.
    pub method: BinomialIntervalMethod,
    /// Requested two-sided confidence level.
    pub level: f64,
}

impl BinomialIntervalSpec {
    /// Constructs a validated interval specification.
    ///
    /// # Errors
    ///
    /// Returns [`OsdCampaignError::InvalidConfiguration`] unless `level` is
    /// finite and strictly between zero and one.
    pub fn new(method: BinomialIntervalMethod, level: f64) -> Result<Self, OsdCampaignError> {
        if !level.is_finite() || !(0.0..1.0).contains(&level) {
            return Err(OsdCampaignError::InvalidConfiguration(
                "confidence level must be finite and strictly between zero and one".to_owned(),
            ));
        }
        Ok(Self { method, level })
    }

    fn compute(self, errors: u64, samples: u64) -> BinomialConfidenceInterval {
        let (lower, upper) = match self.method {
            BinomialIntervalMethod::ClopperPearson => {
                gf2_stats::intervals::clopper_pearson_interval(errors, samples, self.level)
            }
        };
        BinomialConfidenceInterval {
            estimator: ConfidenceIntervalEstimator::ClopperPearson,
            sampling_unit: Some(IntervalSamplingUnit::Block),
            level: self.level,
            lower,
            upper,
        }
    }
}

/// A receipted confidence interval with its estimator and sampling unit.
///
/// Schema 1 serialized the estimator under the field name `method` and omitted
/// `sampling_unit`. Deserialization retains that estimator and represents its
/// historically unstated unit as `None`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinomialConfidenceInterval {
    /// Named estimator used to compute the endpoints.
    #[serde(alias = "method")]
    pub estimator: ConfidenceIntervalEstimator,
    /// Independent unit supporting the stated coverage, absent in schema 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_unit: Option<IntervalSamplingUnit>,
    /// Two-sided confidence level used to compute the endpoints.
    pub level: f64,
    /// Inclusive lower endpoint.
    pub lower: f64,
    /// Inclusive upper endpoint.
    pub upper: f64,
}

/// Mechanical provenance attached to an OSD campaign receipt.
///
/// This deliberately composes the permanent-campaign schema's canonical
/// [`crate::permanent_campaign::schema::Provenance`] and
/// [`crate::permanent_campaign::schema::ArtifactIdentity`] types. `runtime`
/// records the git revision, compiler, invocation, and hardware observed by
/// the producer; the two artifact identities bind the exact campaign
/// configuration and the measurement behavior that gives prior receipts their
/// validity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdCampaignProvenance {
    /// Runtime-observed source, toolchain, invocation, RNG, and hardware.
    pub runtime: Provenance,
    /// Committed configuration artifact used by this campaign.
    pub configuration: ArtifactIdentity,
    /// Committed identity of the producer's measurement behavior.
    pub measurement_behavior: ArtifactIdentity,
}

/// Runtime provenance for one invocation that contributed cell attempts.
///
/// `provenance.invocation` is the complete unquoted argument vector, including
/// the executable token. The half-open attempt range begins at
/// `first_attempt_index` and contains `attempt_count` consecutive entries in
/// [`OsdCampaignReceipt::cell_results`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdCampaignInvocation {
    /// Full runtime provenance observed for this invocation.
    pub provenance: Provenance,
    /// Zero-based index of the first attempt contributed by this invocation.
    pub first_attempt_index: u64,
    /// Number of consecutive attempts contributed by this invocation.
    pub attempt_count: u64,
}

/// Aggregate OSD work performed while producing a cell result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdWorkCounters {
    /// Ordered-basis eliminations performed across sampled blocks.
    pub eliminations: u64,
    /// Reprocessing patterns generated across sampled blocks.
    pub generated_patterns: u64,
    /// Candidate words tested across sampled blocks.
    pub tested_candidates: u64,
}

impl OsdWorkCounters {
    fn contains(self, prior: Self) -> bool {
        self.eliminations >= prior.eliminations
            && self.generated_patterns >= prior.generated_patterns
            && self.tested_candidates >= prior.tested_candidates
    }
}

/// Result state of one OSD campaign cell attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum OsdCellTermination {
    /// The cell reached its preregistered statistical stopping rule.
    Completed,
    /// The cell stopped gracefully and can continue from its cumulative state.
    Interrupted,
    /// The result is retained but its sampling process was censored.
    Censored {
        /// Mechanical reason the result was censored.
        reason: String,
    },
    /// The result is retained after a configured work budget was exhausted.
    Exhausted {
        /// Mechanical budget diagnostic.
        reason: String,
    },
    /// The result contradicts the recorded published comparison value.
    Contradictory {
        /// Published BER rejected by the widened BER receipt interval.
        published_value: f64,
    },
}

impl OsdCellTermination {
    fn is_terminal(&self) -> bool {
        !matches!(self, Self::Interrupted)
    }
}

/// Cumulative evaluator output for one cell attempt.
///
/// On resume these fields must be at least the corresponding values in
/// [`OsdCellExecution::resume`]. Both sample denominators must advance, bit
/// errors cannot exceed sampled bits, and block errors cannot exceed sampled
/// blocks; violations are refused before checkpoint persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct OsdCellRun {
    /// Cumulative sampled blocks for this cell.
    pub samples: u64,
    /// Cumulative sampled information bits across the sampled blocks.
    pub sampled_bits: u64,
    /// Cumulative information-bit errors for this cell.
    pub bit_errors: u64,
    /// Cumulative block errors for this cell.
    pub block_errors: u64,
    /// Cumulative OSD work counters for this cell.
    pub work: OsdWorkCounters,
    /// State reached by this attempt.
    pub termination: OsdCellTermination,
}

/// Cumulative progress supplied to a resumed cell evaluator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OsdCellResume {
    /// Sampled blocks already represented by the preceding attempt.
    pub samples: u64,
    /// Sampled information bits already represented by the preceding attempt.
    pub sampled_bits: u64,
    /// Information-bit errors already represented by the preceding attempt.
    pub bit_errors: u64,
    /// Block errors already represented by the preceding attempt.
    pub block_errors: u64,
    /// OSD work already represented by the preceding attempt.
    pub work: OsdWorkCounters,
}

/// Evaluator context for one pending OSD campaign cell.
#[derive(Debug, Clone, Copy)]
pub struct OsdCellExecution<'a> {
    /// Explicit campaign cell being evaluated.
    pub cell: &'a OsdCell,
    /// Deterministic seed derived from campaign seed and cell identity.
    pub seed: u64,
    /// Cumulative counters from an earlier interrupted attempt.
    pub resume: OsdCellResume,
    /// Cumulative independent block-error target that completes the cell.
    pub target_block_errors: u64,
}

/// Durable statistical evidence for one cell attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdCellReceipt {
    /// Receipt schema version.
    pub schema_version: u32,
    /// Invocation-history entry that produced this attempt, absent in schema 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_index: Option<u64>,
    /// Complete explicit cell configuration and stable identity.
    pub cell: OsdCell,
    /// Deterministic cell seed.
    pub seed: u64,
    /// Cumulative sampled blocks.
    pub samples: u64,
    /// Cumulative sampled information bits.
    pub sampled_bits: u64,
    /// Cumulative information-bit errors.
    pub bit_errors: u64,
    /// Cumulative block errors.
    pub block_errors: u64,
    /// Bit error-rate point estimate `bit_errors / sampled_bits`.
    pub ber: f64,
    /// Block error-rate point estimate `block_errors / samples`.
    pub bler: f64,
    /// Block-sampled BER interval for the mean per-block bit-error fraction.
    pub ber_confidence_interval: BinomialConfidenceInterval,
    /// Named BLER interval computed from `block_errors` and `samples`.
    pub bler_confidence_interval: BinomialConfidenceInterval,
    /// Cumulative OSD work.
    pub work: OsdWorkCounters,
    /// State reached by this cell attempt.
    pub termination: OsdCellTermination,
}

impl OsdCellReceipt {
    /// Tests a published BER against this receipt's widened BER interval.
    ///
    /// This interprets the cell's recorded digitization precision in
    /// [`DigitizationPrecisionUnit::Log10Decades`] and accepts both
    /// multiplicatively widened endpoints exactly.
    #[must_use]
    pub fn accepts_published_value(&self, published_value: f64) -> bool {
        accepts_published_value(
            published_value,
            &self.ber_confidence_interval,
            self.cell.digitization_precision,
        )
    }
}

/// Campaign-wide execution state stored in checkpoints and receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum OsdCampaignTermination {
    /// More explicit cells remain after the latest durable boundary.
    InProgress,
    /// Every explicit cell has a retained terminal result.
    Completed,
    /// One cell has retained cumulative progress for a later invocation.
    Interrupted {
        /// Stable identity of the interrupted cell.
        cell_id: OsdCellId,
    },
}

/// Versioned generic-checkpoint payload for an OSD campaign.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdCampaignCheckpoint {
    /// Checkpoint payload schema version.
    pub schema_version: u32,
    /// Campaign root seed used for every cell seed.
    pub campaign_seed: u64,
    /// Ordered explicit campaign grid.
    pub cells: Vec<OsdCell>,
    /// Exact BLER method and confidence level shared by both rates.
    pub interval: BinomialIntervalSpec,
    /// Cumulative block-error target that completes each cell.
    pub target_block_errors: u64,
    /// Runtime, configuration, and measurement-behavior provenance.
    pub provenance: OsdCampaignProvenance,
    /// Runtime provenance and attempt ranges for contributing invocations.
    pub invocation_history: Vec<OsdCampaignInvocation>,
    /// Ordered history of cell attempts, including interrupted evidence.
    pub cell_results: Vec<OsdCellReceipt>,
    /// Latest campaign-wide execution state.
    pub termination: OsdCampaignTermination,
}

impl CheckpointPayload for OsdCampaignCheckpoint {
    const IDENTITY: &'static str = CHECKPOINT_IDENTITY;
    const SCHEMA_VERSION: u32 = OSD_CAMPAIGN_SCHEMA_VERSION;
}

/// Versioned, standalone statistical receipt for an OSD campaign.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdCampaignReceipt {
    /// Receipt schema version.
    pub schema_version: u32,
    /// Campaign root seed used for every cell seed.
    pub campaign_seed: u64,
    /// Ordered explicit campaign grid.
    pub cells: Vec<OsdCell>,
    /// Exact BLER method and confidence level shared by both rates.
    pub interval: BinomialIntervalSpec,
    /// Cumulative block-error target, absent from historical schema 1 receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_block_errors: Option<u64>,
    /// Unit of every cell's digitization precision, absent in schema 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digitization_precision_unit: Option<DigitizationPrecisionUnit>,
    /// Runtime, configuration, and measurement-behavior provenance.
    pub provenance: OsdCampaignProvenance,
    /// Runtime provenance and attempt ranges for every contributing invocation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invocation_history: Vec<OsdCampaignInvocation>,
    /// Deterministic configuration hash bound to the checkpoint envelope.
    pub configuration_hash: String,
    /// Ordered history of all cell attempts.
    pub cell_results: Vec<OsdCellReceipt>,
    /// Final state of this invocation.
    pub termination: OsdCampaignTermination,
}

/// Validated configuration for one reusable OSD campaign.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdCampaign {
    schema_version: u32,
    campaign_seed: u64,
    cells: Vec<OsdCell>,
    interval: BinomialIntervalSpec,
    target_block_errors: u64,
    provenance: OsdCampaignProvenance,
}

impl OsdCampaign {
    /// Constructs a campaign over an ordered collection of explicit cells.
    ///
    /// Cell identities must be unique. Cell order controls execution and
    /// receipt order but does not affect the identity-derived cell seeds.
    ///
    /// # Errors
    ///
    /// Returns [`OsdCampaignError::InvalidConfiguration`] for an invalid cell,
    /// interval, zero block-error target, or duplicate cell identity.
    pub fn new(
        campaign_seed: u64,
        cells: Vec<OsdCell>,
        interval: BinomialIntervalSpec,
        target_block_errors: u64,
        provenance: OsdCampaignProvenance,
    ) -> Result<Self, OsdCampaignError> {
        let campaign = Self {
            schema_version: OSD_CAMPAIGN_SCHEMA_VERSION,
            campaign_seed,
            cells,
            interval,
            target_block_errors,
            provenance,
        };
        validate_campaign(&campaign)?;
        Ok(campaign)
    }

    /// Returns the ordered explicit cells.
    #[must_use]
    pub fn cells(&self) -> &[OsdCell] {
        &self.cells
    }

    /// Returns the campaign root seed.
    #[must_use]
    pub fn campaign_seed(&self) -> u64 {
        self.campaign_seed
    }

    /// Returns the cumulative independent block-error stopping target.
    #[must_use]
    pub fn target_block_errors(&self) -> u64 {
        self.target_block_errors
    }

    /// Derives the deterministic seed for a stable cell identity.
    #[must_use]
    pub fn cell_seed(&self, cell_id: &OsdCellId) -> u64 {
        derive_cell_seed(self.campaign_seed, cell_id)
    }

    /// Computes the checkpoint configuration identity.
    ///
    /// The hash covers the complete validated campaign, including the stopping
    /// rule, interval, explicit cell grid, source/toolchain/hardware
    /// observations, committed configuration, and measurement behavioral
    /// identity. The invocation argument vector is excluded because
    /// `--max-samples` is deliberately an invocation-local bound and may differ
    /// across resumptions; every full vector is retained in invocation history.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the in-memory campaign cannot be
    /// represented by the canonical JSON serializer.
    pub fn config_hash(&self) -> Result<String, serde_json::Error> {
        let mut configuration = self.clone();
        configuration.provenance.runtime.invocation.clear();
        serde_json::to_vec(&configuration)
            .map(|bytes| format!("blake3:{}", blake3::hash(&bytes).to_hex()))
    }
}

/// Error returned by OSD campaign validation, persistence, or recovery.
#[derive(Debug)]
pub enum OsdCampaignError {
    /// Live campaign configuration is invalid.
    InvalidConfiguration(String),
    /// An evaluator returned invalid or non-monotonic cumulative evidence.
    InvalidCellResult {
        /// Stable identity of the rejected cell result.
        cell_id: OsdCellId,
        /// Mechanical validation diagnostic.
        message: String,
    },
    /// A present checkpoint failed its generic persistence envelope checks.
    Checkpoint(CheckpointLoadError),
    /// A matching checkpoint contained inconsistent OSD payload state.
    InvalidCheckpoint(String),
    /// Checkpoint creation or persistence failed.
    Io(std::io::Error),
    /// Canonical campaign hashing failed.
    Serialization(serde_json::Error),
}

impl fmt::Display for OsdCampaignError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid OSD campaign configuration: {message}")
            }
            Self::InvalidCellResult { cell_id, message } => {
                write!(
                    formatter,
                    "invalid result for OSD cell {cell_id}: {message}"
                )
            }
            Self::Checkpoint(error) => write!(formatter, "OSD campaign checkpoint failed: {error}"),
            Self::InvalidCheckpoint(message) => {
                write!(formatter, "invalid OSD campaign checkpoint: {message}")
            }
            Self::Io(error) => write!(formatter, "OSD campaign checkpoint I/O failed: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "OSD campaign serialization failed: {error}")
            }
        }
    }
}

impl std::error::Error for OsdCampaignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Serialization(error) => Some(error),
            Self::InvalidConfiguration(_)
            | Self::InvalidCellResult { .. }
            | Self::InvalidCheckpoint(_) => None,
        }
    }
}

/// Derives a cell seed from a campaign seed and stable cell identity.
///
/// The BLAKE3 input is domain separated and uses the root seed's little-endian
/// bytes followed by the canonical identity bytes. The first eight digest
/// bytes are interpreted as a little-endian `u64`. Reordering the campaign
/// grid therefore cannot change a cell's random stream.
#[must_use]
pub fn derive_cell_seed(campaign_seed: u64, cell_id: &OsdCellId) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CELL_SEED_DOMAIN);
    hasher.update(&campaign_seed.to_le_bytes());
    hasher.update(cell_id.as_str().as_bytes());
    let digest = hasher.finalize();
    let mut seed_bytes = [0_u8; 8];
    seed_bytes.copy_from_slice(&digest.as_bytes()[..8]);
    u64::from_le_bytes(seed_bytes)
}

/// Applies the log-space digitization-precision reproduction predicate.
///
/// `digitization_precision` is measured in base-10 logarithmic decades. For
/// finite probability endpoints and a finite non-negative precision `delta`,
/// this accepts a finite `published_value` exactly when it lies in the inclusive
/// interval `[lower * 10^(-delta), upper * 10^(delta)]`. Invalid numeric inputs
/// return `false`.
#[must_use]
pub fn accepts_published_value(
    published_value: f64,
    interval: &BinomialConfidenceInterval,
    digitization_precision: f64,
) -> bool {
    let scale = 10.0_f64.powf(digitization_precision);
    published_value.is_finite()
        && interval.lower.is_finite()
        && interval.upper.is_finite()
        && interval.lower >= 0.0
        && interval.lower <= interval.upper
        && digitization_precision.is_finite()
        && digitization_precision >= 0.0
        && published_value >= interval.lower / scale
        && published_value <= interval.upper * scale
}

fn block_mean_hoeffding_sequence(
    point_estimate: f64,
    samples: u64,
    level: f64,
) -> BinomialConfidenceInterval {
    let alpha = 1.0 - level;
    let sample_count = samples as f64;
    let radius =
        ((2.0 * sample_count * (sample_count + 1.0) / alpha).ln() / (2.0 * sample_count)).sqrt();
    BinomialConfidenceInterval {
        estimator: ConfidenceIntervalEstimator::HoeffdingBlockMeanSequence,
        sampling_unit: Some(IntervalSamplingUnit::Block),
        level,
        lower: (point_estimate - radius).max(0.0),
        upper: (point_estimate + radius).min(1.0),
    }
}

/// Runs or resumes an explicit OSD campaign through the canonical checkpoint.
///
/// The generic reader first validates the payload identity, schema version,
/// and complete campaign hash. Each evaluator result is validated, converted
/// to BER and BLER plus their configured block-sampled intervals, and atomically
/// checkpointed before the next cell. The receipt comparison method applies
/// the metric-agnostic predicate to the BER interval because the campaign's
/// published reproduction target is BER. An interrupted result ends this
/// invocation; a later call receives the same seed and cumulative resume
/// counters. All terminal cell identities are derived from their retained
/// results and skipped on recovery.
///
/// # Errors
///
/// Returns [`OsdCampaignError`] for invalid live configuration, a present
/// invalid or mismatched checkpoint, invalid/non-monotonic evaluator output,
/// serialization failure, or checkpoint I/O failure.
pub fn run_osd_campaign(
    checkpoint_path: impl AsRef<Path>,
    campaign: &OsdCampaign,
    mut evaluate: impl FnMut(OsdCellExecution<'_>) -> OsdCellRun,
) -> Result<OsdCampaignReceipt, OsdCampaignError> {
    validate_campaign(campaign)?;
    let configuration_hash = campaign
        .config_hash()
        .map_err(OsdCampaignError::Serialization)?;
    let checkpoint_path = checkpoint_path.as_ref();
    let reader = CheckpointReader::<OsdCampaignCheckpoint, _>::for_payload(
        checkpoint_path,
        configuration_hash.clone(),
    );
    let mut checkpoint = match reader.load_payload() {
        Ok(Some(checkpoint)) => checkpoint,
        Ok(None) => new_checkpoint(campaign),
        Err(error) => return Err(OsdCampaignError::Checkpoint(error)),
    };
    validate_checkpoint(&checkpoint, campaign)?;

    let writer = CheckpointWriter::<OsdCampaignCheckpoint, _>::for_payload(
        checkpoint_path,
        configuration_hash.clone(),
    )
    .map_err(OsdCampaignError::Io)?;
    let mut completed = settled_cell_ids(&checkpoint.cell_results);
    let invocation_index = u64::try_from(checkpoint.invocation_history.len()).map_err(|_| {
        OsdCampaignError::InvalidCheckpoint("invocation history length exceeds u64".to_owned())
    })?;
    let first_attempt_index = u64::try_from(checkpoint.cell_results.len()).map_err(|_| {
        OsdCampaignError::InvalidCheckpoint("cell attempt history length exceeds u64".to_owned())
    })?;
    let mut invocation_recorded = false;

    for cell in &campaign.cells {
        if completed.contains(&cell.id) {
            continue;
        }
        let resume = latest_resume(&checkpoint.cell_results, &cell.id);
        let run = evaluate(OsdCellExecution {
            cell,
            seed: campaign.cell_seed(&cell.id),
            resume,
            target_block_errors: campaign.target_block_errors,
        });
        validate_cell_run(cell, campaign.target_block_errors, resume, &run)?;
        let receipt = make_cell_receipt(campaign, cell, run, invocation_index);
        if let OsdCellTermination::Contradictory { published_value } = &receipt.termination {
            if receipt.accepts_published_value(*published_value) {
                return Err(OsdCampaignError::InvalidCellResult {
                    cell_id: cell.id.clone(),
                    message: "contradictory published value is accepted by the widened interval"
                        .to_owned(),
                });
            }
        }
        let interrupted = matches!(&receipt.termination, OsdCellTermination::Interrupted);
        let terminal = receipt.termination.is_terminal();
        if !invocation_recorded {
            checkpoint.provenance.runtime = campaign.provenance.runtime.clone();
            checkpoint.invocation_history.push(OsdCampaignInvocation {
                provenance: campaign.provenance.runtime.clone(),
                first_attempt_index,
                attempt_count: 0,
            });
            invocation_recorded = true;
        }
        let invocation = checkpoint
            .invocation_history
            .last_mut()
            .expect("the current invocation was just recorded");
        invocation.attempt_count = invocation
            .attempt_count
            .checked_add(1)
            .expect("cell attempt history length cannot exceed u64");
        checkpoint.cell_results.push(receipt);

        if terminal {
            completed.insert(cell.id.clone());
            checkpoint.termination = if completed.len() == campaign.cells.len() {
                OsdCampaignTermination::Completed
            } else {
                OsdCampaignTermination::InProgress
            };
        } else {
            checkpoint.termination = OsdCampaignTermination::Interrupted {
                cell_id: cell.id.clone(),
            };
        }
        writer
            .write_payload(&checkpoint)
            .map_err(OsdCampaignError::Io)?;

        if interrupted {
            return receipt_from_persisted_checkpoint(&reader, campaign, configuration_hash);
        }
    }

    checkpoint.termination = OsdCampaignTermination::Completed;
    writer
        .write_payload(&checkpoint)
        .map_err(OsdCampaignError::Io)?;
    receipt_from_persisted_checkpoint(&reader, campaign, configuration_hash)
}

fn validate_cell(cell: &OsdCell) -> Result<(), OsdCampaignError> {
    if !cell.eb_n0_db.is_finite() {
        return Err(OsdCampaignError::InvalidConfiguration(format!(
            "cell {} has a non-finite Eb/N0 value",
            cell.id
        )));
    }
    if !cell.digitization_precision.is_finite() || cell.digitization_precision < 0.0 {
        return Err(OsdCampaignError::InvalidConfiguration(format!(
            "cell {} digitization precision must be finite and non-negative",
            cell.id
        )));
    }
    Ok(())
}

fn validate_runtime_provenance(runtime: &Provenance) -> Result<(), OsdCampaignError> {
    if runtime.invocation.is_empty()
        || runtime.invocation[0].is_empty()
        || runtime.invocation.iter().any(|token| token.contains('\0'))
    {
        return Err(OsdCampaignError::InvalidConfiguration(
            "runtime invocation must contain a complete non-NUL argument vector".to_owned(),
        ));
    }
    let platform_token = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    if runtime.cpu_model.trim().is_empty() || runtime.cpu_model == platform_token {
        return Err(OsdCampaignError::InvalidConfiguration(
            "cpu_model must be an observed processor identity, not a platform token".to_owned(),
        ));
    }
    let physical_cores = runtime.cpu_physical_cores.ok_or_else(|| {
        OsdCampaignError::InvalidConfiguration(
            "runtime provenance must record the physical CPU core count".to_owned(),
        )
    })?;
    let logical_threads = runtime.cpu_logical_threads.ok_or_else(|| {
        OsdCampaignError::InvalidConfiguration(
            "runtime provenance must record the logical CPU thread count".to_owned(),
        )
    })?;
    if physical_cores == 0 || logical_threads < physical_cores {
        return Err(OsdCampaignError::InvalidConfiguration(
            "CPU topology requires positive cores and at least one thread per core".to_owned(),
        ));
    }
    Ok(())
}

fn validate_campaign(campaign: &OsdCampaign) -> Result<(), OsdCampaignError> {
    if campaign.schema_version != OSD_CAMPAIGN_SCHEMA_VERSION {
        return Err(OsdCampaignError::InvalidConfiguration(format!(
            "schema version {} differs from supported version {}",
            campaign.schema_version, OSD_CAMPAIGN_SCHEMA_VERSION
        )));
    }
    BinomialIntervalSpec::new(campaign.interval.method, campaign.interval.level)?;
    if campaign.target_block_errors == 0 {
        return Err(OsdCampaignError::InvalidConfiguration(
            "target block errors must be positive".to_owned(),
        ));
    }
    validate_runtime_provenance(&campaign.provenance.runtime)?;
    let mut ids = BTreeSet::new();
    for cell in &campaign.cells {
        validate_cell(cell)?;
        if !ids.insert(cell.id.clone()) {
            return Err(OsdCampaignError::InvalidConfiguration(format!(
                "duplicate cell identity {}",
                cell.id
            )));
        }
    }
    Ok(())
}

fn new_checkpoint(campaign: &OsdCampaign) -> OsdCampaignCheckpoint {
    OsdCampaignCheckpoint {
        schema_version: OSD_CAMPAIGN_SCHEMA_VERSION,
        campaign_seed: campaign.campaign_seed,
        cells: campaign.cells.clone(),
        interval: campaign.interval,
        target_block_errors: campaign.target_block_errors,
        provenance: campaign.provenance.clone(),
        invocation_history: Vec::new(),
        cell_results: Vec::new(),
        termination: if campaign.cells.is_empty() {
            OsdCampaignTermination::Completed
        } else {
            OsdCampaignTermination::InProgress
        },
    }
}

fn latest_resume(results: &[OsdCellReceipt], cell_id: &OsdCellId) -> OsdCellResume {
    results
        .iter()
        .rev()
        .find(|receipt| &receipt.cell.id == cell_id)
        .map_or_else(OsdCellResume::default, |receipt| OsdCellResume {
            samples: receipt.samples,
            sampled_bits: receipt.sampled_bits,
            bit_errors: receipt.bit_errors,
            block_errors: receipt.block_errors,
            work: receipt.work,
        })
}

fn settled_cell_ids(results: &[OsdCellReceipt]) -> BTreeSet<OsdCellId> {
    results
        .iter()
        .filter(|receipt| receipt.termination.is_terminal())
        .map(|receipt| receipt.cell.id.clone())
        .collect()
}

fn validate_cell_run(
    cell: &OsdCell,
    target_block_errors: u64,
    resume: OsdCellResume,
    run: &OsdCellRun,
) -> Result<(), OsdCampaignError> {
    let invalid = |message: &str| OsdCampaignError::InvalidCellResult {
        cell_id: cell.id.clone(),
        message: message.to_owned(),
    };
    if run.samples <= resume.samples {
        return Err(invalid("cumulative samples must advance"));
    }
    if run.sampled_bits <= resume.sampled_bits {
        return Err(invalid("cumulative sampled bits must advance"));
    }
    if run.sampled_bits < run.samples {
        return Err(invalid("sampled bits cannot be less than samples"));
    }
    if !run.sampled_bits.is_multiple_of(run.samples) {
        return Err(invalid(
            "sampled bits must represent a fixed information-block length",
        ));
    }
    if resume.samples > 0 && run.sampled_bits / run.samples != resume.sampled_bits / resume.samples
    {
        return Err(invalid(
            "information-block length cannot change across resume",
        ));
    }
    if run.bit_errors < resume.bit_errors {
        return Err(invalid("cumulative bit errors cannot decrease"));
    }
    if run.block_errors < resume.block_errors {
        return Err(invalid("cumulative block errors cannot decrease"));
    }
    if run.bit_errors > run.sampled_bits {
        return Err(invalid("bit errors cannot exceed sampled bits"));
    }
    if run.block_errors > run.samples {
        return Err(invalid("block errors cannot exceed samples"));
    }
    if run.block_errors > run.bit_errors {
        return Err(invalid("block errors cannot exceed bit errors"));
    }
    if !run.work.contains(resume.work) {
        return Err(invalid("cumulative OSD work counters cannot decrease"));
    }
    match &run.termination {
        OsdCellTermination::Completed if run.block_errors < target_block_errors => {
            return Err(invalid(
                "completed cell has not reached the target block-error count",
            ));
        }
        OsdCellTermination::Interrupted if run.block_errors >= target_block_errors => {
            return Err(invalid(
                "interrupted cell has already reached the target block-error count",
            ));
        }
        _ => {}
    }
    match &run.termination {
        OsdCellTermination::Censored { reason } | OsdCellTermination::Exhausted { reason }
            if reason.trim().is_empty() =>
        {
            return Err(invalid("terminal diagnostic must not be empty"));
        }
        OsdCellTermination::Contradictory { published_value } if !published_value.is_finite() => {
            return Err(invalid("contradictory published value must be finite"));
        }
        _ => {}
    }
    Ok(())
}

fn make_cell_receipt(
    campaign: &OsdCampaign,
    cell: &OsdCell,
    run: OsdCellRun,
    invocation_index: u64,
) -> OsdCellReceipt {
    let ber = run.bit_errors as f64 / run.sampled_bits as f64;
    OsdCellReceipt {
        schema_version: OSD_CAMPAIGN_SCHEMA_VERSION,
        invocation_index: Some(invocation_index),
        cell: cell.clone(),
        seed: campaign.cell_seed(&cell.id),
        samples: run.samples,
        sampled_bits: run.sampled_bits,
        bit_errors: run.bit_errors,
        block_errors: run.block_errors,
        ber,
        bler: run.block_errors as f64 / run.samples as f64,
        ber_confidence_interval: block_mean_hoeffding_sequence(
            ber,
            run.samples,
            campaign.interval.level,
        ),
        bler_confidence_interval: campaign.interval.compute(run.block_errors, run.samples),
        work: run.work,
        termination: run.termination,
    }
}

fn same_derived_probability(left: f64, right: f64) -> bool {
    const MAX_ULPS: u64 = 4;

    left == right
        || (left.is_finite()
            && right.is_finite()
            && left.to_bits().abs_diff(right.to_bits()) <= MAX_ULPS)
}

fn same_derived_interval(
    left: BinomialConfidenceInterval,
    right: BinomialConfidenceInterval,
) -> bool {
    left.estimator == right.estimator
        && left.sampling_unit == right.sampling_unit
        && left.level == right.level
        && same_derived_probability(left.lower, right.lower)
        && same_derived_probability(left.upper, right.upper)
}

fn same_resume_provenance(
    checkpoint: &OsdCampaignProvenance,
    live: &OsdCampaignProvenance,
) -> bool {
    let mut checkpoint_runtime = checkpoint.runtime.clone();
    let mut live_runtime = live.runtime.clone();
    checkpoint_runtime.invocation.clear();
    live_runtime.invocation.clear();
    checkpoint_runtime == live_runtime
        && checkpoint.configuration == live.configuration
        && checkpoint.measurement_behavior == live.measurement_behavior
}

fn validate_invocation_history(checkpoint: &OsdCampaignCheckpoint) -> Result<(), OsdCampaignError> {
    let invalid = |message: String| OsdCampaignError::InvalidCheckpoint(message);
    let mut next_attempt = 0_u64;
    for (index, invocation) in checkpoint.invocation_history.iter().enumerate() {
        validate_runtime_provenance(&invocation.provenance)
            .map_err(|error| invalid(error.to_string()))?;
        if invocation.attempt_count == 0 || invocation.first_attempt_index != next_attempt {
            return Err(invalid(format!(
                "invocation {index} does not name a non-empty contiguous attempt range"
            )));
        }
        let end = next_attempt
            .checked_add(invocation.attempt_count)
            .ok_or_else(|| invalid("invocation attempt range overflows u64".to_owned()))?;
        let start_index = usize::try_from(next_attempt)
            .map_err(|_| invalid("invocation attempt index exceeds usize".to_owned()))?;
        let end_index = usize::try_from(end)
            .map_err(|_| invalid("invocation attempt index exceeds usize".to_owned()))?;
        let expected_invocation_index = u64::try_from(index)
            .map_err(|_| invalid("invocation history index exceeds u64".to_owned()))?;
        let attempts = checkpoint
            .cell_results
            .get(start_index..end_index)
            .ok_or_else(|| invalid(format!("invocation {index} attempt range is out of bounds")))?;
        if attempts
            .iter()
            .any(|attempt| attempt.invocation_index != Some(expected_invocation_index))
        {
            return Err(invalid(format!(
                "invocation {index} attempt range disagrees with cell receipts"
            )));
        }
        next_attempt = end;
    }
    let attempt_count = u64::try_from(checkpoint.cell_results.len())
        .map_err(|_| invalid("cell attempt history length exceeds u64".to_owned()))?;
    if next_attempt != attempt_count {
        return Err(invalid(
            "invocation history does not cover every cell attempt".to_owned(),
        ));
    }
    Ok(())
}

fn validate_checkpoint(
    checkpoint: &OsdCampaignCheckpoint,
    campaign: &OsdCampaign,
) -> Result<(), OsdCampaignError> {
    let invalid = |message: String| OsdCampaignError::InvalidCheckpoint(message);
    if checkpoint.schema_version != OSD_CAMPAIGN_SCHEMA_VERSION {
        return Err(invalid(format!(
            "payload schema version {} differs from supported version {}",
            checkpoint.schema_version, OSD_CAMPAIGN_SCHEMA_VERSION
        )));
    }
    if checkpoint.campaign_seed != campaign.campaign_seed
        || checkpoint.cells != campaign.cells
        || checkpoint.interval != campaign.interval
        || checkpoint.target_block_errors != campaign.target_block_errors
        || !same_resume_provenance(&checkpoint.provenance, &campaign.provenance)
    {
        return Err(invalid(
            "payload campaign configuration differs from the live campaign".to_owned(),
        ));
    }
    validate_invocation_history(checkpoint)?;

    let expected: BTreeMap<_, _> = campaign
        .cells
        .iter()
        .map(|cell| (cell.id.clone(), cell))
        .collect();
    let mut settled = BTreeSet::new();
    let mut latest = BTreeMap::<OsdCellId, OsdCellResume>::new();
    for receipt in &checkpoint.cell_results {
        let cell = expected.get(&receipt.cell.id).ok_or_else(|| {
            invalid(format!(
                "result names unknown cell identity {}",
                receipt.cell.id
            ))
        })?;
        if settled.contains(&receipt.cell.id) {
            return Err(invalid(format!(
                "terminal cell {} has a later attempt",
                receipt.cell.id
            )));
        }
        if receipt.schema_version != OSD_CAMPAIGN_SCHEMA_VERSION
            || receipt.invocation_index.is_none()
            || receipt.cell != **cell
            || receipt.seed != campaign.cell_seed(&receipt.cell.id)
        {
            return Err(invalid(format!(
                "result identity or schema differs for cell {}",
                receipt.cell.id
            )));
        }
        let resume = latest.get(&receipt.cell.id).copied().unwrap_or_default();
        let run = OsdCellRun {
            samples: receipt.samples,
            sampled_bits: receipt.sampled_bits,
            bit_errors: receipt.bit_errors,
            block_errors: receipt.block_errors,
            work: receipt.work,
            termination: receipt.termination.clone(),
        };
        validate_cell_run(cell, campaign.target_block_errors, resume, &run)
            .map_err(|error| invalid(error.to_string()))?;
        let expected_ber = receipt.bit_errors as f64 / receipt.sampled_bits as f64;
        let expected_ber_interval =
            block_mean_hoeffding_sequence(expected_ber, receipt.samples, campaign.interval.level);
        let expected_bler_interval = campaign
            .interval
            .compute(receipt.block_errors, receipt.samples);
        let expected_bler = receipt.block_errors as f64 / receipt.samples as f64;
        if !same_derived_probability(receipt.ber, expected_ber)
            || !same_derived_probability(receipt.bler, expected_bler)
            || !same_derived_interval(receipt.ber_confidence_interval, expected_ber_interval)
            || !same_derived_interval(receipt.bler_confidence_interval, expected_bler_interval)
        {
            return Err(invalid(format!(
                "statistical fields disagree with counts for cell {}: BER {} (expected {}), BLER {} (expected {}), BER interval {:?} (expected {:?}), BLER interval {:?} (expected {:?})",
                receipt.cell.id,
                receipt.ber,
                expected_ber,
                receipt.bler,
                expected_bler,
                receipt.ber_confidence_interval,
                expected_ber_interval,
                receipt.bler_confidence_interval,
                expected_bler_interval,
            )));
        }
        if let OsdCellTermination::Contradictory { published_value } = &receipt.termination {
            if receipt.accepts_published_value(*published_value) {
                return Err(invalid(format!(
                    "contradictory value is accepted for cell {}",
                    receipt.cell.id
                )));
            }
        }
        if receipt.termination.is_terminal() {
            settled.insert(receipt.cell.id.clone());
        }
        latest.insert(
            receipt.cell.id.clone(),
            OsdCellResume {
                samples: receipt.samples,
                sampled_bits: receipt.sampled_bits,
                bit_errors: receipt.bit_errors,
                block_errors: receipt.block_errors,
                work: receipt.work,
            },
        );
    }

    let expected_termination = if settled.len() == campaign.cells.len() {
        OsdCampaignTermination::Completed
    } else if let Some(last) = checkpoint.cell_results.last() {
        if matches!(&last.termination, OsdCellTermination::Interrupted) {
            OsdCampaignTermination::Interrupted {
                cell_id: last.cell.id.clone(),
            }
        } else {
            OsdCampaignTermination::InProgress
        }
    } else {
        OsdCampaignTermination::InProgress
    };
    if checkpoint.termination != expected_termination {
        return Err(invalid(
            "campaign termination does not match retained cell results".to_owned(),
        ));
    }
    Ok(())
}

fn receipt_from_checkpoint(
    checkpoint: OsdCampaignCheckpoint,
    configuration_hash: String,
) -> OsdCampaignReceipt {
    OsdCampaignReceipt {
        schema_version: checkpoint.schema_version,
        campaign_seed: checkpoint.campaign_seed,
        cells: checkpoint.cells,
        interval: checkpoint.interval,
        target_block_errors: Some(checkpoint.target_block_errors),
        digitization_precision_unit: Some(DigitizationPrecisionUnit::Log10Decades),
        provenance: checkpoint.provenance,
        invocation_history: checkpoint.invocation_history,
        configuration_hash,
        cell_results: checkpoint.cell_results,
        termination: checkpoint.termination,
    }
}

fn receipt_from_persisted_checkpoint(
    reader: &CheckpointReader<OsdCampaignCheckpoint, String>,
    campaign: &OsdCampaign,
    configuration_hash: String,
) -> Result<OsdCampaignReceipt, OsdCampaignError> {
    let checkpoint = reader
        .load_payload()
        .map_err(OsdCampaignError::Checkpoint)?
        .ok_or_else(|| {
            OsdCampaignError::InvalidCheckpoint(
                "checkpoint disappeared after a successful durable write".to_owned(),
            )
        })?;
    validate_checkpoint(&checkpoint, campaign)?;
    Ok(receipt_from_checkpoint(checkpoint, configuration_hash))
}
