//! Deterministic, resumable ordered-statistics-decoding campaigns.
//!
//! This module owns the campaign mechanics shared by OSD executables: explicit
//! cell identities, order-independent seed derivation, block-index-keyed random
//! streams, multi-worker block evaluation, bit- and block-error accounting,
//! block-sampled intervals for BER and BLER, strict checkpoint and receipt
//! schemas, and cell-boundary resume. Domain executables retain only
//! code/channel construction and a per-block evaluator. Persistence delegates
//! to [`crate::checkpoint`], parallel dispatch to [`crate::parallel`], and the
//! exact binomial endpoints to [`gf2_stats::intervals`].
//!
//! # Evaluation and stopping
//!
//! A block's identity is its cell, that cell's deterministic seed, and its
//! zero-based cumulative [`block_index`](OsdBlockContext::block_index).
//! [`OsdBlockStream`] positions the block's random stream from that index
//! alone, so a block's outcome is a pure function of its identity — the
//! evaluator contract this protocol requires and the reason results do not
//! depend on which worker evaluated which block.
//!
//! The protocol dispatches blocks to `workers` per-worker evaluators through
//! [`crate::parallel::map_indices_in_order`] and commits the returned outcomes
//! in block-index order. It alone accumulates the sample, error, squared-error,
//! and work counters, and completes a cell at the smallest block index whose
//! in-order cumulative block-error count reaches the target `K`. Workers
//! evaluate blocks past that index speculatively; those outcomes are discarded
//! and contribute to no counter. The evaluator therefore answers for every
//! index it is handed, in any order, and cannot choose termination, report
//! aggregate counters, or skip, reorder, or double-count blocks.
//!
//! Results are byte-identical across worker counts for a fixed campaign and
//! seed, with the single-worker run as the reference. On resume the protocol
//! restores durable counters and continues at the next block index under the
//! same cell seed, reaching the evidence an uninterrupted run would have.
//! Earlier attempts remain in the receipt. Completed, censored, exhausted, and
//! contradictory cells are terminal evidence and are never evaluated again.
//!
//! The worker count is invocation-local: it never enters the campaign
//! configuration identity, and reaches a receipt only through the recorded
//! invocation argument vector.
//!
//! # Interval coverage and schema history
//!
//! Schema 2 treats one decoded block as the independent sampling unit for both
//! intervals, and both follow the stopping design rather than assuming a fixed
//! trial count. A completed cell is accepted only with exactly `K` block
//! errors, where `K` is the campaign's `target_block_errors`; the protocol
//! enforces that the last sampled block is the target's `K`-th error. The total
//! block count `N` is therefore a stopping time and the design is
//! inverse-binomial. `--max-samples` bounds one invocation without consulting
//! any outcome, so that censoring is independent of the sampled values and a
//! resumed completed cell still holds exactly `K` failing blocks drawn under
//! the same design.
//!
//! BER factors over that design as `BER = BLER * mu`, where `mu` is the mean
//! information-bit error fraction of a failing block. Both factors are
//! estimated at the component level `1 - alpha / 2` for a requested two-sided
//! level `1 - alpha`:
//!
//! - BLER uses the equal-tailed exact inversion of the inverse-binomial
//!   sampling distribution
//!   ([`gf2_stats::intervals::negative_binomial_interval`]).
//! - `mu` uses the Maurer-Pontil empirical-Bernstein bound
//!   ([`gf2_stats::intervals::empirical_bernstein_interval`]) over the `K`
//!   per-failing-block error fractions, intersected with the domain
//!   `[1 / k, 1]` for information-block length `k`. `K` is fixed by the
//!   stopping rule, so this fixed-sample bound applies; the failing blocks'
//!   error magnitudes are independent and identically distributed under the
//!   conditional law of a failing block, and are independent of where those
//!   failures fall in the block sequence. The bound is variance-adaptive, which
//!   matters because a failing block's error fraction concentrates far below
//!   the `[0, 1]` range a Hoeffding-type bound would have to assume.
//!
//! The recorded BER interval is the endpoint product
//! `[BLER_L * mu_L, BLER_U * mu_U]`. All four endpoints are non-negative, so
//! the product interval contains `BER` whenever both factor intervals contain
//! their factors; by the union bound its coverage is at least
//! `1 - alpha / 2 - alpha / 2 = 1 - alpha`. Nothing in the construction assumes
//! independence among the bit errors inside a block.
//!
//! That coverage statement applies only to a completed cell. An attempt that
//! stopped for any other reason retains its cumulative counters but records no
//! intervals: its block-error count is not the design's fixed `K`, so the
//! stopping design does not deliver the intervals' stated coverage there.
//! Published-value acceptance likewise applies only to a completed cell.
//!
//! Schema 1 receipts remain deserializable as historical evidence. Their BER
//! intervals used Clopper-Pearson over individual bits without naming a
//! sampling unit, so their stated coverage does not apply to clustered decoder
//! error bursts. Their BLER intervals are computed over blocks, but under a
//! bit-error stopping rule that leaves the block count a stopping time rather
//! than the fixed trial count that inversion assumes. Schema 1 also compared a
//! published value against `[L - delta, U + delta]`, applying a digitization
//! precision recorded in base-10 decades as a linear probability offset; schema
//! 2 supersedes that rule with [`accepts_published_value`]. Schema 1 stored one
//! usually executable-only invocation in the global
//! runtime provenance, and its `cpu_model` may contain an OS/architecture
//! platform token. Schema 2 records the full argument vector and runtime
//! provenance for every invocation that contributes an attempt, plus observed
//! processor topology.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::path::Path;
use std::str::FromStr;

use rand08::SeedableRng;
use rand_chacha08::ChaCha20Rng;
use serde::{Deserialize, Deserializer, Serialize};

use crate::checkpoint::{
    CheckpointLoadError, CheckpointPayload, CheckpointReader, CheckpointWriter,
};
use crate::parallel::{
    map_indices_in_order, worker_offset, DEBUG_ASSERT_WORD_MARGIN, FRAME_STRIDE,
};
use crate::permanent_campaign::schema::{ArtifactIdentity, Provenance};

/// The schema version shared by OSD checkpoints, receipts, and cell records.
pub const OSD_CAMPAIGN_SCHEMA_VERSION: u32 = 2;

const CHECKPOINT_IDENTITY: &str = "osd-campaign/cell-progress";
const CELL_SEED_DOMAIN: &[u8] = b"gf2-sim/osd-campaign/cell-seed/v1\0";

/// Blocks dispatched to each worker per speculative wave.
///
/// A wave is the unit of speculation: the protocol dispatches
/// `workers * OSD_WAVE_BLOCKS_PER_WORKER` blocks (capped by the invocation's
/// remaining sample bound), then commits them in index order. A cell therefore
/// wastes at most one wave of block evaluations past its stopping block, once,
/// while the wave stays long enough to amortise the fan-out over the per-block
/// decode cost. The value changes how much speculative work happens, never the
/// committed result: index-ordered truncation makes the outcome a function of
/// the block sequence and the block-error target alone.
const OSD_WAVE_BLOCKS_PER_WORKER: u64 = 16;

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

/// Block-error interval implementation used by a campaign.
///
/// The closed vocabulary prevents a receipt from naming an estimator other
/// than the one actually dispatched through the shared `gf2-stats` API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinomialIntervalMethod {
    /// Equal-tailed exact Clopper-Pearson interval for a fixed trial count.
    ///
    /// Schema 1 campaigns stopped on a bit-error target and dispatched this for
    /// both rates. A live campaign under this protocol stops on block errors,
    /// where the block count is a stopping time, so [`OsdCampaign::new`]
    /// refuses this method and it survives only for reading schema 1 evidence.
    ClopperPearson,
    /// Equal-tailed exact inversion of the stop-at-Kth-error design.
    NegativeBinomialClopperPearson,
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
    /// Equal-tailed exact interval for a fixed-trial block-error count.
    ClopperPearson,
    /// Equal-tailed exact inversion of the stop-at-Kth-block-error design.
    NegativeBinomialClopperPearson,
    /// Product of the block-error rate and the mean failing-block error
    /// fraction, each bounded at half the requested error probability.
    BlockRatioProductInterval,
}

/// Unit used by every OSD published-curve digitization precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigitizationPrecisionUnit {
    /// Additive width in base-10 logarithmic space.
    Log10Decades,
}

/// Cumulative block-sampled counters that determine a cell's BER interval.
///
/// A block error is a sampled block holding at least one information-bit
/// error, so every counted bit error lies inside a failing block and a failing
/// block's error fraction lies in `[1 / k, 1]` for information-block length
/// `k = sampled_bits / samples`. The protocol refuses evaluator output that
/// contradicts those relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockSampleCounts {
    /// Sampled blocks, the stopping time of a completed cell.
    pub samples: u64,
    /// Sampled information bits across those blocks.
    pub sampled_bits: u64,
    /// Information-bit errors, all of them inside failing blocks.
    pub bit_errors: u64,
    /// Failing blocks; the design's fixed count once the cell completes.
    pub block_errors: u64,
    /// Sum of the squared per-block information-bit error counts.
    pub squared_block_bit_errors: u64,
}

impl BlockSampleCounts {
    /// Returns the mean and unbiased sample variance of the failing blocks'
    /// information-bit error fractions, both in error-fraction units.
    ///
    /// The counters are integers, so the sum of squared deviations is formed
    /// exactly in `u128` before it reaches floating point and can never turn
    /// negative through cancellation.
    fn failing_block_fraction_moments(self) -> (f64, f64) {
        let information_bits = (self.sampled_bits / self.samples) as f64;
        let failures = self.block_errors as f64;
        let mean = self.bit_errors as f64 / (failures * information_bits);
        if self.block_errors < 2 {
            return (mean, 0.0);
        }
        let scaled_deviation = u128::from(self.squared_block_bit_errors)
            * u128::from(self.block_errors)
            - u128::from(self.bit_errors) * u128::from(self.bit_errors);
        let variance = scaled_deviation as f64
            / (failures * (failures - 1.0) * information_bits * information_bits);
        (mean, variance)
    }

    /// Bounds the mean failing-block error fraction at `level`.
    ///
    /// With no observed failing block the fraction has no estimand and the
    /// whole domain is returned, which leaves the BER interval equal to the
    /// block-error interval. Otherwise the empirical-Bernstein bound is
    /// intersected with the domain floor `1 / k`; validated counters keep the
    /// sample mean at or above that floor, so the intersection is non-empty.
    fn failing_block_fraction_interval(self, level: f64) -> (f64, f64) {
        if self.block_errors == 0 {
            return (0.0, 1.0);
        }
        let (mean, variance) = self.failing_block_fraction_moments();
        let (lower, upper) = gf2_stats::intervals::empirical_bernstein_interval(
            mean,
            variance,
            self.block_errors,
            level,
        );
        (
            lower.max(self.samples as f64 / self.sampled_bits as f64),
            upper,
        )
    }
}

/// Exact block-error interval method and the campaign's confidence level.
///
/// `level` is the two-sided level of the composed BER interval, the campaign's
/// comparison metric. Each factor of that composition, the recorded BLER
/// interval included, is computed at [`Self::component_level`], and every
/// recorded interval carries the level it was computed at.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinomialIntervalSpec {
    /// Named exact estimator used for the block error rate.
    pub method: BinomialIntervalMethod,
    /// Requested two-sided confidence level of the composed BER interval.
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

    /// Returns the two-sided level spent on each factor of the BER interval.
    ///
    /// The BER interval is the product of a block-error rate interval and a
    /// failing-block error-fraction interval. Each is computed at
    /// `1 - alpha / 2` for the requested `1 - alpha`, so the union bound leaves
    /// the product at the requested level.
    #[must_use]
    pub fn component_level(self) -> f64 {
        1.0 - (1.0 - self.level) / 2.0
    }

    /// Computes the block error-rate interval at [`Self::component_level`].
    ///
    /// The estimator recorded with the endpoints is the one this method
    /// dispatched, so a receipt cannot name an interval it does not hold.
    #[must_use]
    pub fn bler_interval(self, block_errors: u64, samples: u64) -> BinomialConfidenceInterval {
        let level = self.component_level();
        let (estimator, (lower, upper)) = match self.method {
            BinomialIntervalMethod::ClopperPearson => (
                ConfidenceIntervalEstimator::ClopperPearson,
                gf2_stats::intervals::clopper_pearson_interval(block_errors, samples, level),
            ),
            BinomialIntervalMethod::NegativeBinomialClopperPearson => (
                ConfidenceIntervalEstimator::NegativeBinomialClopperPearson,
                gf2_stats::intervals::negative_binomial_interval(block_errors, samples, level),
            ),
        };
        BinomialConfidenceInterval {
            estimator,
            sampling_unit: Some(IntervalSamplingUnit::Block),
            level,
            lower,
            upper,
        }
    }

    /// Computes the composed bit error-rate interval at the requested level.
    ///
    /// The endpoints are the products of the [`Self::bler_interval`] endpoints
    /// with the endpoints of the mean failing-block error-fraction interval,
    /// both taken at [`Self::component_level`]. The composition, its coverage
    /// argument, and the conditions under which the stated coverage applies are
    /// documented at the [module level](self). This construction is recorded
    /// only for completed cells, where the stopping design supplies its stated
    /// coverage.
    ///
    /// # Panics
    ///
    /// Panics when `counts.samples` is zero, which has no information-block
    /// length and no estimand.
    #[must_use]
    pub fn ber_interval(self, counts: BlockSampleCounts) -> BinomialConfidenceInterval {
        assert!(
            counts.samples > 0,
            "BER interval requires at least one sampled block"
        );
        let block_rate = self.bler_interval(counts.block_errors, counts.samples);
        let (fraction_lower, fraction_upper) =
            counts.failing_block_fraction_interval(self.component_level());
        BinomialConfidenceInterval {
            estimator: ConfidenceIntervalEstimator::BlockRatioProductInterval,
            sampling_unit: Some(IntervalSamplingUnit::Block),
            level: self.level,
            lower: (block_rate.lower * fraction_lower).clamp(0.0, 1.0),
            upper: (block_rate.upper * fraction_upper).clamp(0.0, 1.0),
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

/// OSD work counters for either one block or cumulative cell evidence.
///
/// [`OsdBlockOutcome`] uses these as a per-block delta. Checkpoints and
/// receipts store their protocol-accumulated totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsdWorkCounters {
    /// Ordered-basis eliminations performed.
    pub eliminations: u64,
    /// Reprocessing patterns generated.
    pub generated_patterns: u64,
    /// Candidate words tested.
    pub tested_candidates: u64,
}

impl OsdWorkCounters {
    fn contains(self, prior: Self) -> bool {
        self.eliminations >= prior.eliminations
            && self.generated_patterns >= prior.generated_patterns
            && self.tested_candidates >= prior.tested_candidates
    }

    fn checked_add(self, increment: Self) -> Option<Self> {
        Some(Self {
            eliminations: self.eliminations.checked_add(increment.eliminations)?,
            generated_patterns: self
                .generated_patterns
                .checked_add(increment.generated_patterns)?,
            tested_candidates: self
                .tested_candidates
                .checked_add(increment.tested_candidates)?,
        })
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
        /// Published BER associated with the contradiction.
        published_value: f64,
    },
}

impl OsdCellTermination {
    fn is_terminal(&self) -> bool {
        !matches!(self, Self::Interrupted)
    }
}

/// Cumulative protocol-owned result for one cell attempt.
#[derive(Debug, Clone, PartialEq)]
struct CellRun {
    samples: u64,
    sampled_bits: u64,
    bit_errors: u64,
    block_errors: u64,
    squared_block_bit_errors: u64,
    work: OsdWorkCounters,
    termination: OsdCellTermination,
}

impl CellRun {
    fn counts(&self) -> BlockSampleCounts {
        BlockSampleCounts {
            samples: self.samples,
            sampled_bits: self.sampled_bits,
            bit_errors: self.bit_errors,
            block_errors: self.block_errors,
            squared_block_bit_errors: self.squared_block_bit_errors,
        }
    }
}

/// Cumulative progress restored from a preceding interrupted attempt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CellResume {
    samples: u64,
    sampled_bits: u64,
    bit_errors: u64,
    block_errors: u64,
    squared_block_bit_errors: u64,
    work: OsdWorkCounters,
}

/// Read-only context for exactly one protocol-selected sampled block.
///
/// `block_index` is the zero-based cumulative index within `cell`. `seed` is
/// the campaign's stable per-cell seed and is identical on every call for that
/// cell, so an evaluator positions its deterministic random stream from
/// `block_index` alone — [`OsdBlockStream`] is that seek.
///
/// Workers evaluate a cell's blocks concurrently, so calls arrive in no
/// particular order and include blocks past the cell's stopping index. An
/// evaluator answers for every index it is handed and lets no block's outcome
/// depend on another's.
#[derive(Debug, Clone, Copy)]
pub struct OsdBlockContext<'a> {
    /// Explicit campaign cell owning this block.
    pub cell: &'a OsdCell,
    /// Deterministic seed derived from campaign seed and cell identity.
    pub seed: u64,
    /// Zero-based cumulative block index selected by the protocol.
    pub block_index: u64,
}

/// A cell's ChaCha20 stream, positioned at one block's reserved region.
///
/// This is the protocol's per-block seek: the stream is selected by the cell
/// seed and positioned by [`worker_offset`]`(seed, 0, 0, block_index)`, the
/// design-doc §3 word-position scheme. A block's draws therefore depend on its
/// global index alone, which is what makes a cell's counters, stopping index,
/// and receipt byte-identical across worker counts and across a checkpoint
/// boundary.
///
/// Each block owns a reserved [`FRAME_STRIDE`] region (`2^20` ChaCha20 32-bit
/// words, 4 MiB), so a variable-consumption sampler — the campaign's
/// rejection-sampled Gaussian noise — cannot draw into the next block's region.
/// [`debug_assert_block_budget`] checks that in debug builds, as
/// [`WorkerCtx`](crate::parallel::WorkerCtx) does per frame.
///
/// # Named exception to the shared worker context
///
/// This type mirrors [`WorkerCtx`](crate::parallel::WorkerCtx)'s seek surface
/// instead of reusing it, and delegates the offset arithmetic to the shared
/// [`worker_offset`]. `WorkerCtx` owns a `rand_chacha` 0.9 stream, while the
/// campaign's channel (`gf2_coding::simulation::BpskAwgnChannel`) takes a
/// `rand` 0.8 RNG, and the two ecosystems' `ChaCha20Rng` types do not
/// interoperate. Their word-position
/// contracts do agree — both count ChaCha20 32-bit words in a 68-bit space with
/// `BLOCK_WORDS = 16` — so the same offsets are valid in both. The exception is
/// recorded under `@/inv/convention-convergence` against `@/issue/c1b253cb`
/// (the `rand` 0.8 holdout), whose convergence condition is `gf2-coding` moving
/// to `rand` 0.9; `@/issue/90a88fa9` tracks converging the campaign execution
/// stacks.
///
/// [`debug_assert_block_budget`]: Self::debug_assert_block_budget
///
/// # Examples
///
/// ```
/// use gf2_sim::osd_campaign::OsdBlockStream;
/// use gf2_sim::parallel::worker_offset;
///
/// let mut stream = OsdBlockStream::new(0x5eed);
/// stream.seek_to_block(3);
/// assert_eq!(stream.current_word_pos(), worker_offset(0x5eed, 0, 0, 3));
///
/// // Two streams on the same cell seed agree at the same block, whichever
/// // worker holds them and whatever they drew before.
/// let mut other = OsdBlockStream::new(0x5eed);
/// other.seek_to_block(9);
/// other.seek_to_block(3);
/// assert_eq!(other.current_word_pos(), stream.current_word_pos());
/// ```
pub struct OsdBlockStream {
    seed: u64,
    rng: ChaCha20Rng,
}

impl OsdBlockStream {
    /// Opens the ChaCha20 stream for a cell seed, at word position zero.
    ///
    /// Call [`seek_to_block`](Self::seek_to_block) before each block's draws.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rng: ChaCha20Rng::seed_from_u64(seed),
        }
    }

    /// Positions the stream at the start of `block_index`'s reserved region.
    ///
    /// # Panics
    ///
    /// Panics when `block_index` exceeds `usize::MAX`, the index width the
    /// shared [`worker_offset`] takes; no campaign reaches `2^64` blocks on a
    /// 64-bit host.
    pub fn seek_to_block(&mut self, block_index: u64) {
        let block_index =
            usize::try_from(block_index).expect("block index fits the host word size");
        self.rng
            .set_word_pos(worker_offset(self.seed, 0, 0, block_index));
    }

    /// Mutable access to the positioned stream for the block's draws.
    #[inline]
    pub fn rng_mut(&mut self) -> &mut ChaCha20Rng {
        &mut self.rng
    }

    /// The stream's current absolute word position (ChaCha20 32-bit words).
    #[inline]
    #[must_use]
    pub fn current_word_pos(&self) -> u128 {
        self.rng.get_word_pos()
    }

    /// Debug-asserts the block's draws stayed inside its reserved region.
    ///
    /// Call after a block's draws with the index it was seeked to. No-op in
    /// release builds.
    pub fn debug_assert_block_budget(&self, block_index: u64) {
        debug_assert!({
            let block_index =
                usize::try_from(block_index).expect("block index fits the host word size");
            let start = worker_offset(self.seed, 0, 0, block_index);
            let drawn = self.rng.get_word_pos().saturating_sub(start);
            drawn <= FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN
        });
    }
}

impl fmt::Debug for OsdBlockStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OsdBlockStream")
            .field("seed", &self.seed)
            .field("word_pos", &self.current_word_pos())
            .finish()
    }
}

/// Domain result for one sampled information block.
///
/// The protocol derives whether this is a block error from
/// `information_bit_errors != 0`, accumulates the squared error count, and adds
/// `work` as a per-block delta. The evaluator cannot report aggregate counters
/// or a termination state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsdBlockOutcome {
    /// Number of information bits in this block; it must be positive and fixed
    /// for every block of a cell, including across resume.
    pub information_bits: u64,
    /// Information-bit errors observed in this block.
    pub information_bit_errors: u64,
    /// OSD work performed for this block, added to the cell's durable totals.
    pub work: OsdWorkCounters,
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
    /// Cumulative squared per-block bit-error sum, absent in schema 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squared_block_bit_errors: Option<u64>,
    /// Bit error-rate point estimate `bit_errors / sampled_bits`.
    pub ber: f64,
    /// Block error-rate point estimate `block_errors / samples`.
    pub bler: f64,
    /// Composed BER interval over the block-error stopping design, present for
    /// completed cells only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ber_confidence_interval: Option<BinomialConfidenceInterval>,
    /// Named BLER interval computed from `block_errors` and `samples`, present
    /// for completed cells only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bler_confidence_interval: Option<BinomialConfidenceInterval>,
    /// Cumulative OSD work.
    pub work: OsdWorkCounters,
    /// State reached by this cell attempt.
    pub termination: OsdCellTermination,
}

impl OsdCellReceipt {
    /// Tests a published BER against a completed receipt's widened BER
    /// interval.
    ///
    /// This interprets the cell's recorded digitization precision in
    /// [`DigitizationPrecisionUnit::Log10Decades`] and accepts both
    /// multiplicatively widened endpoints exactly. It returns `false` for a
    /// non-completed cell, which has no valid interval for this predicate.
    #[must_use]
    pub fn accepts_published_value(&self, published_value: f64) -> bool {
        matches!(self.termination, OsdCellTermination::Completed)
            && self
                .ber_confidence_interval
                .as_ref()
                .is_some_and(|interval| {
                    accepts_published_value(
                        published_value,
                        interval,
                        self.cell.digitization_precision,
                    )
                })
    }

    /// Returns the block-sampled counters supporting this attempt's intervals.
    ///
    /// A schema 1 attempt carries no squared per-block bit-error sum, so its
    /// counters report zero there and do not reproduce its recorded intervals.
    #[must_use]
    pub fn counts(&self) -> BlockSampleCounts {
        BlockSampleCounts {
            samples: self.samples,
            sampled_bits: self.sampled_bits,
            bit_errors: self.bit_errors,
            block_errors: self.block_errors,
            squared_block_bit_errors: self.squared_block_bit_errors.unwrap_or_default(),
        }
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
    /// Exact block-error interval method and campaign confidence level.
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
    /// Exact block-error interval method and campaign confidence level.
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
///
/// The predicate is total over every finite non-negative `delta`. A `delta`
/// large enough to overflow `10^delta` widens the upper endpoint to infinity,
/// and a zero upper endpoint stays zero rather than becoming an indeterminate
/// product, so a zero-width interval at zero still accepts a published zero.
#[must_use]
pub fn accepts_published_value(
    published_value: f64,
    interval: &BinomialConfidenceInterval,
    digitization_precision: f64,
) -> bool {
    if !published_value.is_finite()
        || !interval.lower.is_finite()
        || !interval.upper.is_finite()
        || interval.lower < 0.0
        || interval.lower > interval.upper
        || !digitization_precision.is_finite()
        || digitization_precision < 0.0
    {
        return false;
    }
    // `delta >= 0` keeps `scale >= 1`, so dividing cannot overflow and the
    // quotient stays finite even when the scale itself does not.
    let scale = 10.0_f64.powf(digitization_precision);
    let widened_lower = interval.lower / scale;
    let widened_upper = if interval.upper > 0.0 {
        interval.upper * scale
    } else {
        0.0
    };
    published_value >= widened_lower && published_value <= widened_upper
}

/// Runs or resumes an explicit OSD campaign through the canonical checkpoint.
///
/// The generic reader first validates the payload identity, schema version,
/// and complete campaign hash. For each pending cell the protocol dispatches
/// blocks to `workers` per-worker evaluators, commits the outcomes in
/// block-index order, and completes the cell at the block carrying the target's
/// `K`-th block error. Blocks the workers evaluated past that one are
/// discarded. An evaluator receives only the selected cell, stable cell seed,
/// and block index: it cannot choose termination, alter cumulative counters, or
/// skip, reorder, or double-count blocks. If `max_samples` blocks are sampled
/// for a pending cell in this invocation before it completes, the protocol
/// records an interrupted attempt; a later call restores the counters and
/// continues at the next block index. Each attempt is atomically checkpointed
/// before the next cell, and terminal cells are skipped on recovery.
///
/// `workers` is invocation-local. It changes how fast a cell is sampled, never
/// what is sampled: for a fixed campaign and seed every counter, the stopping
/// index, and the receipt payload are byte-identical across worker counts, with
/// the single-worker run as the reference. It is excluded from the campaign
/// configuration identity and reaches a receipt only through the caller's
/// recorded invocation argument vector.
///
/// # Arguments
///
/// * `checkpoint_path` — the protocol's durable progress file.
/// * `campaign` — the validated campaign configuration.
/// * `max_samples` — this invocation's per-cell sample bound. It is consulted
///   without reference to any outcome, so censoring stays independent of the
///   sampled values.
/// * `workers` — number of concurrent block evaluators.
/// * `make_evaluator` — per-worker evaluator factory, called once per worker
///   per dispatched wave. Every evaluator it produces must return the same
///   outcome for the same block, so a block's result stays a pure function of
///   its cell, seed, and index; [`OsdBlockStream`] is the seek that delivers
///   that for a random block process.
///
/// # Errors
///
/// Returns [`OsdCampaignError`] for invalid live configuration, a present
/// invalid or mismatched checkpoint, a zero invocation sample bound, an
/// invalid per-block outcome, counter overflow, serialization failure, or
/// checkpoint I/O failure.
pub fn run_osd_campaign<E, M>(
    checkpoint_path: impl AsRef<Path>,
    campaign: &OsdCampaign,
    max_samples: u64,
    workers: NonZeroUsize,
    make_evaluator: M,
) -> Result<OsdCampaignReceipt, OsdCampaignError>
where
    M: Fn() -> E + Sync,
    E: FnMut(OsdBlockContext<'_>) -> OsdBlockOutcome,
{
    validate_campaign(campaign)?;
    if max_samples == 0 {
        return Err(OsdCampaignError::InvalidConfiguration(
            "max samples per cell invocation must be positive".to_owned(),
        ));
    }
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
        let run = sample_cell(
            cell,
            campaign.cell_seed(&cell.id),
            campaign.target_block_errors,
            max_samples,
            workers,
            resume,
            &make_evaluator,
        )?;
        validate_cell_run(cell, campaign.target_block_errors, resume, &run)?;
        let receipt = make_cell_receipt(campaign, cell, run, invocation_index);
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
    if campaign.interval.method != BinomialIntervalMethod::NegativeBinomialClopperPearson {
        return Err(OsdCampaignError::InvalidConfiguration(
            "a block-error stopping rule leaves the block count a stopping time and requires the inverse-binomial exact interval".to_owned(),
        ));
    }
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

fn latest_resume(results: &[OsdCellReceipt], cell_id: &OsdCellId) -> CellResume {
    results
        .iter()
        .rev()
        .find(|receipt| &receipt.cell.id == cell_id)
        .map_or_else(CellResume::default, |receipt| CellResume {
            samples: receipt.samples,
            sampled_bits: receipt.sampled_bits,
            bit_errors: receipt.bit_errors,
            block_errors: receipt.block_errors,
            squared_block_bit_errors: receipt.squared_block_bit_errors.unwrap_or_default(),
            work: receipt.work,
        })
}

/// Samples one pending cell, dispatching blocks in waves and committing their
/// outcomes in block-index order.
///
/// Each wave covers `workers * OSD_WAVE_BLOCKS_PER_WORKER` consecutive block
/// indices, capped by the invocation's remaining sample bound so that bound
/// never depends on an outcome. The wave's outcomes are committed in index
/// order until the cell reaches the block-error target; the rest of the wave is
/// discarded unread.
fn sample_cell<E, M>(
    cell: &OsdCell,
    seed: u64,
    target_block_errors: u64,
    max_samples: u64,
    workers: NonZeroUsize,
    resume: CellResume,
    make_evaluator: &M,
) -> Result<CellRun, OsdCampaignError>
where
    M: Fn() -> E + Sync,
    E: FnMut(OsdBlockContext<'_>) -> OsdBlockOutcome,
{
    let invalid = |message: String| OsdCampaignError::InvalidCellResult {
        cell_id: cell.id.clone(),
        message,
    };
    let mut run = CellRun {
        samples: resume.samples,
        sampled_bits: resume.sampled_bits,
        bit_errors: resume.bit_errors,
        block_errors: resume.block_errors,
        squared_block_bit_errors: resume.squared_block_bit_errors,
        work: resume.work,
        termination: OsdCellTermination::Interrupted,
    };
    let mut information_bits = (resume.samples > 0).then(|| resume.sampled_bits / resume.samples);
    let wave_blocks = OSD_WAVE_BLOCKS_PER_WORKER.saturating_mul(workers.get() as u64);
    let mut remaining = max_samples;

    while remaining > 0 {
        let wave = remaining.min(wave_blocks);
        let first_index = run.samples;
        let end_index = first_index
            .checked_add(wave)
            .ok_or_else(|| invalid("block index overflow".to_owned()))?;
        let outcomes = map_indices_in_order(
            first_index..end_index,
            workers,
            make_evaluator,
            |block_index, evaluator| {
                evaluator(OsdBlockContext {
                    cell,
                    seed,
                    block_index,
                })
            },
        );

        for outcome in outcomes {
            commit_block(&mut run, &mut information_bits, outcome, &invalid)?;
            if run.block_errors == target_block_errors {
                run.termination = OsdCellTermination::Completed;
                return Ok(run);
            }
        }
        remaining -= wave;
    }

    Ok(run)
}

/// Accumulates one validated block outcome into a cell's cumulative counters.
fn commit_block(
    run: &mut CellRun,
    information_bits: &mut Option<u64>,
    outcome: OsdBlockOutcome,
    invalid: &impl Fn(String) -> OsdCampaignError,
) -> Result<(), OsdCampaignError> {
    if outcome.information_bits == 0 {
        return Err(invalid(
            "sampled information blocks must contain at least one bit".to_owned(),
        ));
    }
    if outcome.information_bit_errors > outcome.information_bits {
        return Err(invalid(
            "information-bit errors cannot exceed the sampled block length".to_owned(),
        ));
    }
    match information_bits {
        Some(expected) if outcome.information_bits != *expected => {
            return Err(invalid(
                "information-block length cannot change within a cell".to_owned(),
            ));
        }
        None => *information_bits = Some(outcome.information_bits),
        _ => {}
    }

    run.samples = run
        .samples
        .checked_add(1)
        .ok_or_else(|| invalid("sample counter overflow".to_owned()))?;
    run.sampled_bits = run
        .sampled_bits
        .checked_add(outcome.information_bits)
        .ok_or_else(|| invalid("sampled-bit counter overflow".to_owned()))?;
    run.bit_errors = run
        .bit_errors
        .checked_add(outcome.information_bit_errors)
        .ok_or_else(|| invalid("bit-error counter overflow".to_owned()))?;
    run.block_errors = run
        .block_errors
        .checked_add(u64::from(outcome.information_bit_errors != 0))
        .ok_or_else(|| invalid("block-error counter overflow".to_owned()))?;
    let squared_errors = outcome
        .information_bit_errors
        .checked_mul(outcome.information_bit_errors)
        .ok_or_else(|| invalid("squared block bit-error counter overflow".to_owned()))?;
    run.squared_block_bit_errors = run
        .squared_block_bit_errors
        .checked_add(squared_errors)
        .ok_or_else(|| invalid("squared block bit-error counter overflow".to_owned()))?;
    run.work = run
        .work
        .checked_add(outcome.work)
        .ok_or_else(|| invalid("OSD work counter overflow".to_owned()))?;
    Ok(())
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
    resume: CellResume,
    run: &CellRun,
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
    if run.squared_block_bit_errors < resume.squared_block_bit_errors {
        return Err(invalid(
            "cumulative squared block bit errors cannot decrease",
        ));
    }
    if run.squared_block_bit_errors < run.bit_errors {
        return Err(invalid(
            "squared block bit errors cannot fall below the bit-error count",
        ));
    }
    let information_bits = run.sampled_bits / run.samples;
    if u128::from(run.bit_errors) > u128::from(run.block_errors) * u128::from(information_bits) {
        return Err(invalid("bit errors must fit inside the failing blocks"));
    }
    if u128::from(run.squared_block_bit_errors)
        > u128::from(information_bits) * u128::from(run.bit_errors)
    {
        return Err(invalid(
            "a block cannot contribute more bit errors than its information-block length",
        ));
    }
    if u128::from(run.squared_block_bit_errors) * u128::from(run.block_errors)
        < u128::from(run.bit_errors) * u128::from(run.bit_errors)
    {
        return Err(invalid(
            "squared block bit errors contradict the bit-error total",
        ));
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
        OsdCellTermination::Completed if run.block_errors > target_block_errors => {
            return Err(invalid(&format!(
                "completed cell must stop exactly at the target block-error count; the inverse-binomial interval assumes the last sampled block is the target's K-th error (observed {}, target {})",
                run.block_errors, target_block_errors
            )));
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
    run: CellRun,
    invocation_index: u64,
) -> OsdCellReceipt {
    let intervals = if matches!(&run.termination, OsdCellTermination::Completed) {
        let counts = run.counts();
        (
            Some(campaign.interval.ber_interval(counts)),
            Some(
                campaign
                    .interval
                    .bler_interval(run.block_errors, run.samples),
            ),
        )
    } else {
        (None, None)
    };
    OsdCellReceipt {
        schema_version: OSD_CAMPAIGN_SCHEMA_VERSION,
        invocation_index: Some(invocation_index),
        cell: cell.clone(),
        seed: campaign.cell_seed(&cell.id),
        samples: run.samples,
        sampled_bits: run.sampled_bits,
        bit_errors: run.bit_errors,
        block_errors: run.block_errors,
        squared_block_bit_errors: Some(run.squared_block_bit_errors),
        ber: run.bit_errors as f64 / run.sampled_bits as f64,
        bler: run.block_errors as f64 / run.samples as f64,
        ber_confidence_interval: intervals.0,
        bler_confidence_interval: intervals.1,
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

fn same_derived_optional_interval(
    left: Option<BinomialConfidenceInterval>,
    right: Option<BinomialConfidenceInterval>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same_derived_interval(left, right),
        (None, None) => true,
        _ => false,
    }
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
    let mut latest = BTreeMap::<OsdCellId, CellResume>::new();
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
            || receipt.squared_block_bit_errors.is_none()
            || receipt.cell != **cell
            || receipt.seed != campaign.cell_seed(&receipt.cell.id)
        {
            return Err(invalid(format!(
                "result identity or schema differs for cell {}",
                receipt.cell.id
            )));
        }
        let resume = latest.get(&receipt.cell.id).copied().unwrap_or_default();
        let run = CellRun {
            samples: receipt.samples,
            sampled_bits: receipt.sampled_bits,
            bit_errors: receipt.bit_errors,
            block_errors: receipt.block_errors,
            squared_block_bit_errors: receipt.squared_block_bit_errors.unwrap_or_default(),
            work: receipt.work,
            termination: receipt.termination.clone(),
        };
        validate_cell_run(cell, campaign.target_block_errors, resume, &run)
            .map_err(|error| invalid(error.to_string()))?;
        let expected_ber = receipt.bit_errors as f64 / receipt.sampled_bits as f64;
        let (expected_ber_interval, expected_bler_interval) =
            if matches!(&receipt.termination, OsdCellTermination::Completed) {
                (
                    Some(campaign.interval.ber_interval(receipt.counts())),
                    Some(
                        campaign
                            .interval
                            .bler_interval(receipt.block_errors, receipt.samples),
                    ),
                )
            } else {
                (None, None)
            };
        let expected_bler = receipt.block_errors as f64 / receipt.samples as f64;
        if !same_derived_probability(receipt.ber, expected_ber)
            || !same_derived_probability(receipt.bler, expected_bler)
            || !same_derived_optional_interval(
                receipt.ber_confidence_interval,
                expected_ber_interval,
            )
            || !same_derived_optional_interval(
                receipt.bler_confidence_interval,
                expected_bler_interval,
            )
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
        if receipt.termination.is_terminal() {
            settled.insert(receipt.cell.id.clone());
        }
        latest.insert(
            receipt.cell.id.clone(),
            CellResume {
                samples: receipt.samples,
                sampled_bits: receipt.sampled_bits,
                bit_errors: receipt.bit_errors,
                block_errors: receipt.block_errors,
                squared_block_bit_errors: run.squared_block_bit_errors,
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
