//! Checkpointable pooling of independently produced campaign shards.
//!
//! The accumulator owns only plain count data. It does not open, write, or
//! rename checkpoint files; an orchestration layer can serialize the returned
//! [`AccumulatorSnapshot`] using the persistence policy appropriate to its
//! workload.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};

/// The schema version understood by this crate.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// A stable identity assigned to one shard.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShardId(String);

impl ShardId {
    /// Creates a shard identity from its external representation.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the external representation of this identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ShardId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ShardId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// The finite-field and matrix-size identity of one campaign cell.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellId {
    /// The field order, which is also the number of permanent residue bins.
    pub q: u64,
    /// The square matrix dimension.
    pub dimension: u64,
}

impl CellId {
    /// Creates a cell identity.
    ///
    /// # Errors
    ///
    /// Returns [`AccumulatorError::InvalidCell`] when `q` is less than two or
    /// `dimension` is zero.
    pub fn new(q: u64, dimension: u64) -> Result<Self, AccumulatorError> {
        if q < 2 || dimension == 0 {
            return Err(AccumulatorError::InvalidCell { q, dimension });
        }
        Ok(Self { q, dimension })
    }
}

/// Determinant-companion counts for one shard or pooled cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeterminantCounts {
    /// The determinant companion is not evaluated for this cell.
    NotEvaluated,
    /// The companion is evaluated on `sample_count` matrices.
    Evaluated {
        /// The number of matrices included in the determinant sample.
        sample_count: u64,
        /// The number of sampled matrices with zero determinant.
        zero_count: u64,
    },
}

impl DeterminantCounts {
    /// Creates evaluated determinant counts.
    #[must_use]
    pub const fn evaluated(sample_count: u64, zero_count: u64) -> Self {
        Self::Evaluated {
            sample_count,
            zero_count,
        }
    }

    /// Returns the evaluated sample and zero counts, if the companion runs.
    #[must_use]
    pub const fn counts(self) -> Option<(u64, u64)> {
        match self {
            Self::NotEvaluated => None,
            Self::Evaluated {
                sample_count,
                zero_count,
            } => Some((sample_count, zero_count)),
        }
    }
}

impl Serialize for DeterminantCounts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(tag = "state", rename_all = "snake_case")]
        enum Wire {
            NotEvaluated,
            Evaluated { sample_count: u64, zero_count: u64 },
        }

        match self {
            Self::NotEvaluated => Wire::NotEvaluated.serialize(serializer),
            Self::Evaluated {
                sample_count,
                zero_count,
            } => Wire::Evaluated {
                sample_count: *sample_count,
                zero_count: *zero_count,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for DeterminantCounts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            state: State,
            sample_count: Option<u64>,
            zero_count: Option<u64>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum State {
            NotEvaluated,
            Evaluated,
        }

        let wire = Wire::deserialize(deserializer)?;
        match (wire.state, wire.sample_count, wire.zero_count) {
            (State::NotEvaluated, None, None) => Ok(Self::NotEvaluated),
            (State::Evaluated, Some(sample_count), Some(zero_count)) => Ok(Self::Evaluated {
                sample_count,
                zero_count,
            }),
            (State::NotEvaluated, _, _) => Err(serde::de::Error::custom(
                "not_evaluated determinant forbids numeric counts",
            )),
            (State::Evaluated, _, _) => Err(serde::de::Error::custom(
                "evaluated determinant requires sample_count and zero_count",
            )),
        }
    }
}

/// Count data produced by one successful shard.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardCounts {
    /// Number of matrices evaluated by the shard.
    pub matrix_count: u64,
    /// Number of matrices whose permanent is zero.
    pub permanent_zero_count: u64,
    /// Permanent-value counts in residue order `0, ..., q - 1`.
    pub permanent_histogram: Vec<u64>,
    /// Determinant-companion counts or explicit non-evaluation.
    pub determinant: DeterminantCounts,
}

impl ShardCounts {
    /// Creates count data, deriving the permanent-zero count from histogram bin
    /// zero.
    ///
    /// # Errors
    ///
    /// Returns [`AccumulatorError::InvalidShardCounts`] when the histogram is
    /// empty or does not sum to `matrix_count`, or when determinant counts are
    /// internally inconsistent.
    pub fn new(
        matrix_count: u64,
        permanent_histogram: Vec<u64>,
        determinant: DeterminantCounts,
    ) -> Result<Self, AccumulatorError> {
        let permanent_zero_count =
            *permanent_histogram
                .first()
                .ok_or_else(|| AccumulatorError::InvalidShardCounts {
                    reason: "permanent histogram must have at least one residue bin".to_owned(),
                })?;
        let counts = Self {
            matrix_count,
            permanent_zero_count,
            permanent_histogram,
            determinant,
        };
        counts.validate(None)?;
        Ok(counts)
    }

    fn validate(&self, q: Option<u64>) -> Result<(), AccumulatorError> {
        if self.permanent_histogram.is_empty()
            || q.is_some_and(|value| {
                usize::try_from(value).ok() != Some(self.permanent_histogram.len())
            })
        {
            return Err(AccumulatorError::InvalidShardCounts {
                reason: "permanent histogram length does not match the cell field order".to_owned(),
            });
        }
        let histogram_total = self
            .permanent_histogram
            .iter()
            .try_fold(0_u64, |total, count| total.checked_add(*count))
            .ok_or_else(|| AccumulatorError::InvalidShardCounts {
                reason: "permanent histogram total overflows u64".to_owned(),
            })?;
        if histogram_total != self.matrix_count {
            return Err(AccumulatorError::InvalidShardCounts {
                reason: "permanent histogram total must equal matrix_count".to_owned(),
            });
        }
        if self.permanent_zero_count != self.permanent_histogram[0]
            || self.permanent_zero_count > self.matrix_count
        {
            return Err(AccumulatorError::InvalidShardCounts {
                reason:
                    "permanent_zero_count must equal histogram bin zero and not exceed matrix_count"
                        .to_owned(),
            });
        }
        if let Some((sample_count, zero_count)) = self.determinant.counts() {
            if sample_count > self.matrix_count || zero_count > sample_count {
                return Err(AccumulatorError::InvalidShardCounts {
                    reason: "determinant counts must be bounded by matrix_count".to_owned(),
                });
            }
        }
        Ok(())
    }
}

/// A successful or failed shard offer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shard {
    /// Stable identity of this shard.
    pub identity: ShardId,
    /// Cell to which this shard belongs.
    pub cell: CellId,
    /// Successful counts, or a production/evaluation failure to quarantine.
    pub result: ShardResult,
}

impl Shard {
    /// Creates a successful shard offer.
    #[must_use]
    pub fn with_counts(identity: ShardId, cell: CellId, counts: ShardCounts) -> Self {
        Self {
            identity,
            cell,
            result: ShardResult::Counts(counts),
        }
    }

    /// Creates a failed shard that the accumulator retains in quarantine.
    #[must_use]
    pub fn failed(identity: ShardId, cell: CellId, reason: impl Into<String>) -> Self {
        Self {
            identity,
            cell,
            result: ShardResult::Failed {
                reason: reason.into(),
            },
        }
    }
}

/// The result retained by a [`Shard`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ShardResult {
    /// The shard completed and provides count data.
    Counts(ShardCounts),
    /// The shard failed and is excluded until readmission.
    Failed {
        /// Human-readable production or evaluation failure reason.
        reason: String,
    },
}

/// A failed shard retained outside pooled counts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuarantinedShard {
    /// Stable identity of the failed shard.
    pub identity: ShardId,
    /// Cell to which a future readmission applies.
    pub cell: CellId,
    /// Failure reason retained for the caller and a later operator decision.
    pub reason: String,
}

/// Counts pooled for one cell.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PooledCell {
    /// Cell identity.
    pub cell: CellId,
    /// Identities of the successful shards contributing to this cell.
    pub shard_ids: Vec<ShardId>,
    /// Total matrices represented by committed shards.
    pub matrix_count: u64,
    /// Total permanent-zero count.
    pub permanent_zero_count: u64,
    /// Pooled permanent histogram in residue order.
    pub permanent_histogram: Vec<u64>,
    /// Pooled determinant companion state.
    pub determinant: DeterminantCounts,
}

/// A read-only, serializable view of all accumulator state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PooledState {
    /// Cells in deterministic `(q, dimension)` order.
    pub cells: Vec<PooledCell>,
    /// Identities of successful shards in deterministic order.
    pub committed_shards: Vec<ShardId>,
    /// Failed shards retained outside pooled counts in deterministic order.
    pub quarantined: Vec<QuarantinedShard>,
}

/// A self-describing checkpoint payload for an [`Accumulator`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccumulatorSnapshot {
    /// Snapshot schema version checked before any state is restored.
    pub schema_version: u32,
    /// Cells in deterministic `(q, dimension)` order.
    pub cells: Vec<PooledCell>,
    /// Identities of successful shards in deterministic order.
    pub committed_shards: Vec<ShardId>,
    /// Failed shards retained outside pooled counts in deterministic order.
    pub quarantined: Vec<QuarantinedShard>,
}

impl AccumulatorSnapshot {
    /// The snapshot schema version accepted by this reader.
    pub const SCHEMA_VERSION: u32 = SNAPSHOT_SCHEMA_VERSION;
}

/// The point at which an instrumented offer commit is interrupted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitPoint {
    /// The next accumulator has been staged and the live accumulator is intact.
    Staged,
    /// The staged accumulator has received the shard but is not yet installed.
    Applied,
}

/// The result of offering a shard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OfferOutcome {
    /// Count data enters the pooled state exactly once.
    Committed,
    /// A failed shard is retained outside pooled counts.
    Quarantined {
        /// Identity retained in quarantine.
        identity: ShardId,
    },
}

/// An error returned while validating, committing, or restoring accumulator state.
#[derive(Debug)]
pub enum AccumulatorError {
    /// The cell identity is not a usable finite-field matrix cell.
    InvalidCell {
        /// Field order supplied by the caller.
        q: u64,
        /// Matrix dimension supplied by the caller.
        dimension: u64,
    },
    /// A shard's count relationships are invalid.
    InvalidShardCounts {
        /// Validation diagnostic.
        reason: String,
    },
    /// A shard identity has already been committed or quarantined.
    DuplicateShard {
        /// Identity rejected as a duplicate.
        identity: ShardId,
    },
    /// A numeric pooled counter would overflow.
    CounterOverflow {
        /// Cell whose counter would overflow.
        cell: CellId,
        /// Counter name used in the diagnostic.
        counter: &'static str,
    },
    /// A cell mixes evaluated and not-evaluated determinant companions.
    DeterminantStateConflict {
        /// Cell whose determinant states disagree.
        cell: CellId,
    },
    /// An injected interruption aborts a staged commit.
    CommitInterrupted {
        /// Staging point at which the interruption occurs.
        point: CommitPoint,
    },
    /// A readmission names no retained failed shard.
    UnknownQuarantinedShard {
        /// Identity that was not retained in quarantine.
        identity: ShardId,
    },
    /// The serialized snapshot uses another schema version.
    UnsupportedSchemaVersion {
        /// Version carried by the snapshot.
        found: u32,
        /// Version implemented by this reader.
        expected: u32,
    },
    /// The serialized snapshot cannot be interpreted as an accumulator snapshot.
    InvalidSnapshot {
        /// Snapshot validation or decoding diagnostic.
        reason: String,
    },
    /// Snapshot serialization fails before a caller receives bytes.
    Serialization {
        /// Serializer diagnostic.
        reason: String,
    },
}

impl fmt::Display for AccumulatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCell { q, dimension } => {
                write!(formatter, "invalid cell F_{q} with dimension {dimension}")
            }
            Self::InvalidShardCounts { reason } => {
                write!(formatter, "invalid shard counts: {reason}")
            }
            Self::DuplicateShard { identity } => {
                write!(
                    formatter,
                    "shard identity {:?} was already offered",
                    identity.as_str()
                )
            }
            Self::CounterOverflow { cell, counter } => {
                write!(
                    formatter,
                    "pooled {counter} overflows for cell F_{} n={}",
                    cell.q, cell.dimension
                )
            }
            Self::DeterminantStateConflict { cell } => write!(
                formatter,
                "determinant evaluation state conflicts for cell F_{} n={}",
                cell.q, cell.dimension
            ),
            Self::CommitInterrupted { point } => {
                write!(formatter, "commit interrupted at {point:?}")
            }
            Self::UnknownQuarantinedShard { identity } => {
                write!(
                    formatter,
                    "shard identity {:?} is not quarantined",
                    identity.as_str()
                )
            }
            Self::UnsupportedSchemaVersion { found, expected } => {
                write!(
                    formatter,
                    "snapshot schema version {found} is unsupported; expected {expected}"
                )
            }
            Self::InvalidSnapshot { reason } => {
                write!(formatter, "invalid accumulator snapshot: {reason}")
            }
            Self::Serialization { reason } => {
                write!(formatter, "cannot serialize accumulator snapshot: {reason}")
            }
        }
    }
}

impl Error for AccumulatorError {}

/// A streaming, duplicate-rejecting accumulator for shard count data.
#[derive(Clone, Debug, Default)]
pub struct Accumulator {
    cells: BTreeMap<CellId, PooledCell>,
    committed_shards: BTreeSet<ShardId>,
    quarantined: BTreeMap<ShardId, QuarantinedShard>,
}

impl Accumulator {
    /// Creates an empty accumulator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Offers a shard and commits its counts as one unit.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid counts, a duplicate identity, incompatible
    /// determinant state, or counter overflow. The accumulator is unchanged for
    /// every error.
    pub fn offer(&mut self, shard: Shard) -> Result<OfferOutcome, AccumulatorError> {
        self.offer_with_interrupt(shard, |_| Ok(()))
    }

    /// Offers a shard through an interruption hook used by crash/restart tests.
    ///
    /// The hook runs on a private staged copy. An error from the hook discards
    /// that copy, so a simulated interruption cannot expose partial counts.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::offer`], or the hook's
    /// interruption error. The accumulator is unchanged for every error.
    pub fn offer_with_interrupt<F>(
        &mut self,
        shard: Shard,
        mut interruption: F,
    ) -> Result<OfferOutcome, AccumulatorError>
    where
        F: FnMut(CommitPoint) -> Result<(), AccumulatorError>,
    {
        self.reject_duplicate(&shard.identity)?;
        validate_cell(shard.cell)?;
        if let ShardResult::Counts(counts) = &shard.result {
            counts.validate(Some(shard.cell.q))?;
        }

        let mut staged = self.clone();
        interruption(CommitPoint::Staged)?;
        let outcome = match shard.result {
            ShardResult::Counts(counts) => {
                staged.apply_counts(shard.identity, shard.cell, counts)?;
                OfferOutcome::Committed
            }
            ShardResult::Failed { reason } => {
                let identity = shard.identity.clone();
                staged.quarantined.insert(
                    identity.clone(),
                    QuarantinedShard {
                        identity: identity.clone(),
                        cell: shard.cell,
                        reason,
                    },
                );
                OfferOutcome::Quarantined { identity }
            }
        };
        interruption(CommitPoint::Applied)?;
        *self = staged;
        Ok(outcome)
    }

    /// Explicitly readmits a retained failed shard with verified count data.
    ///
    /// # Errors
    ///
    /// Returns [`AccumulatorError::UnknownQuarantinedShard`] when `identity`
    /// is not quarantined, or a count/overflow/conflict error. Failed
    /// readmission leaves the quarantine entry intact.
    pub fn readmit(
        &mut self,
        identity: &ShardId,
        counts: ShardCounts,
    ) -> Result<OfferOutcome, AccumulatorError> {
        let retained = self.quarantined.get(identity).cloned().ok_or_else(|| {
            AccumulatorError::UnknownQuarantinedShard {
                identity: identity.clone(),
            }
        })?;
        counts.validate(Some(retained.cell.q))?;
        let mut staged = self.clone();
        staged.apply_counts(identity.clone(), retained.cell, counts)?;
        staged.quarantined.remove(identity);
        *self = staged;
        Ok(OfferOutcome::Committed)
    }

    /// Returns a deterministic view of the currently pooled and quarantined state.
    #[must_use]
    pub fn pooled_state(&self) -> PooledState {
        PooledState {
            cells: self.cells.values().cloned().collect(),
            committed_shards: self.committed_shards.iter().cloned().collect(),
            quarantined: self.quarantined.values().cloned().collect(),
        }
    }

    /// Creates a schema-versioned snapshot of this accumulator.
    #[must_use]
    pub fn snapshot(&self) -> AccumulatorSnapshot {
        let state = self.pooled_state();
        AccumulatorSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            cells: state.cells,
            committed_shards: state.committed_shards,
            quarantined: state.quarantined,
        }
    }

    /// Serializes a compact, self-describing JSON snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`AccumulatorError::Serialization`] if the serializer rejects
    /// the plain snapshot payload.
    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, AccumulatorError> {
        serde_json::to_vec(&self.snapshot()).map_err(|error| AccumulatorError::Serialization {
            reason: error.to_string(),
        })
    }

    /// Restores an accumulator from a self-describing JSON snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`AccumulatorError::UnsupportedSchemaVersion`] for a version
    /// this reader does not understand, or [`AccumulatorError::InvalidSnapshot`]
    /// for malformed or internally inconsistent state.
    pub fn from_snapshot_bytes(bytes: &[u8]) -> Result<Self, AccumulatorError> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| AccumulatorError::InvalidSnapshot {
                reason: error.to_string(),
            })?;
        let found = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u32::try_from(version).ok())
            .ok_or_else(|| AccumulatorError::InvalidSnapshot {
                reason: "schema_version is missing or is not a u32".to_owned(),
            })?;
        if found != SNAPSHOT_SCHEMA_VERSION {
            return Err(AccumulatorError::UnsupportedSchemaVersion {
                found,
                expected: SNAPSHOT_SCHEMA_VERSION,
            });
        }
        let snapshot: AccumulatorSnapshot =
            serde_json::from_value(value).map_err(|error| AccumulatorError::InvalidSnapshot {
                reason: error.to_string(),
            })?;
        Self::from_snapshot(snapshot)
    }

    /// Restores an accumulator from an already decoded snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`AccumulatorError::UnsupportedSchemaVersion`] or
    /// [`AccumulatorError::InvalidSnapshot`] when the snapshot is not valid.
    pub fn from_snapshot(snapshot: AccumulatorSnapshot) -> Result<Self, AccumulatorError> {
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(AccumulatorError::UnsupportedSchemaVersion {
                found: snapshot.schema_version,
                expected: SNAPSHOT_SCHEMA_VERSION,
            });
        }
        let mut cells = BTreeMap::new();
        for mut cell in snapshot.cells {
            validate_cell(cell.cell)?;
            cell.shard_ids.sort();
            validate_pooled_cell(&cell)?;
            if cells.insert(cell.cell, cell).is_some() {
                return Err(AccumulatorError::InvalidSnapshot {
                    reason: "snapshot contains duplicate cells".to_owned(),
                });
            }
        }
        let committed_count = snapshot.committed_shards.len();
        let committed_shards: BTreeSet<_> = snapshot.committed_shards.into_iter().collect();
        if committed_shards.len() != committed_count {
            return Err(AccumulatorError::InvalidSnapshot {
                reason: "snapshot contains duplicate committed shard identities".to_owned(),
            });
        }
        let mut quarantined = BTreeMap::new();
        for failed in snapshot.quarantined {
            validate_cell(failed.cell)?;
            if committed_shards.contains(&failed.identity)
                || quarantined
                    .insert(failed.identity.clone(), failed)
                    .is_some()
            {
                return Err(AccumulatorError::InvalidSnapshot {
                    reason: "snapshot contains duplicate or committed quarantined identity"
                        .to_owned(),
                });
            }
        }
        let mut cell_shards = BTreeSet::new();
        for cell in cells.values() {
            for identity in &cell.shard_ids {
                if !committed_shards.contains(identity) || !cell_shards.insert(identity.clone()) {
                    return Err(AccumulatorError::InvalidSnapshot {
                        reason: "pooled cell shard identities do not match committed shards"
                            .to_owned(),
                    });
                }
            }
        }
        if cell_shards != committed_shards {
            return Err(AccumulatorError::InvalidSnapshot {
                reason: "a committed shard identity is missing from pooled cells".to_owned(),
            });
        }
        Ok(Self {
            cells,
            committed_shards,
            quarantined,
        })
    }

    fn reject_duplicate(&self, identity: &ShardId) -> Result<(), AccumulatorError> {
        if self.committed_shards.contains(identity) || self.quarantined.contains_key(identity) {
            return Err(AccumulatorError::DuplicateShard {
                identity: identity.clone(),
            });
        }
        Ok(())
    }

    fn apply_counts(
        &mut self,
        identity: ShardId,
        cell: CellId,
        counts: ShardCounts,
    ) -> Result<(), AccumulatorError> {
        let is_new_cell = !self.cells.contains_key(&cell);
        let pooled = self.cells.entry(cell).or_insert_with(|| PooledCell {
            cell,
            shard_ids: Vec::new(),
            matrix_count: 0,
            permanent_zero_count: 0,
            permanent_histogram: vec![0; usize::try_from(cell.q).expect("validated q fits usize")],
            determinant: counts.determinant,
        });
        if pooled.determinant.counts().is_some() != counts.determinant.counts().is_some() {
            return Err(AccumulatorError::DeterminantStateConflict { cell });
        }
        pooled.matrix_count = pooled.matrix_count.checked_add(counts.matrix_count).ok_or(
            AccumulatorError::CounterOverflow {
                cell,
                counter: "matrix_count",
            },
        )?;
        pooled.permanent_zero_count = pooled
            .permanent_zero_count
            .checked_add(counts.permanent_zero_count)
            .ok_or(AccumulatorError::CounterOverflow {
                cell,
                counter: "permanent_zero_count",
            })?;
        for (pooled_count, shard_count) in pooled
            .permanent_histogram
            .iter_mut()
            .zip(counts.permanent_histogram)
        {
            *pooled_count =
                pooled_count
                    .checked_add(shard_count)
                    .ok_or(AccumulatorError::CounterOverflow {
                        cell,
                        counter: "permanent_histogram",
                    })?;
        }
        if !is_new_cell {
            if let (Some((pooled_samples, pooled_zeros)), Some((samples, zeros))) =
                (pooled.determinant.counts(), counts.determinant.counts())
            {
                pooled.determinant = DeterminantCounts::Evaluated {
                    sample_count: pooled_samples.checked_add(samples).ok_or(
                        AccumulatorError::CounterOverflow {
                            cell,
                            counter: "determinant sample_count",
                        },
                    )?,
                    zero_count: pooled_zeros.checked_add(zeros).ok_or(
                        AccumulatorError::CounterOverflow {
                            cell,
                            counter: "determinant zero_count",
                        },
                    )?,
                };
            }
        }
        pooled.shard_ids.push(identity.clone());
        pooled.shard_ids.sort();
        self.committed_shards.insert(identity);
        Ok(())
    }
}

fn validate_cell(cell: CellId) -> Result<(), AccumulatorError> {
    if cell.q < 2 || cell.dimension == 0 || usize::try_from(cell.q).is_err() {
        return Err(AccumulatorError::InvalidCell {
            q: cell.q,
            dimension: cell.dimension,
        });
    }
    Ok(())
}

fn validate_pooled_cell(cell: &PooledCell) -> Result<(), AccumulatorError> {
    if cell.shard_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(AccumulatorError::InvalidSnapshot {
            reason: "pooled cell contains duplicate shard identities".to_owned(),
        });
    }
    if usize::try_from(cell.cell.q).ok() != Some(cell.permanent_histogram.len()) {
        return Err(AccumulatorError::InvalidSnapshot {
            reason: "pooled histogram length does not equal cell q".to_owned(),
        });
    }
    let histogram_total = cell
        .permanent_histogram
        .iter()
        .try_fold(0_u64, |total, count| total.checked_add(*count))
        .ok_or_else(|| AccumulatorError::InvalidSnapshot {
            reason: "pooled histogram total overflows u64".to_owned(),
        })?;
    if histogram_total != cell.matrix_count
        || cell.permanent_zero_count != cell.permanent_histogram[0]
    {
        return Err(AccumulatorError::InvalidSnapshot {
            reason: "pooled histogram and count totals disagree".to_owned(),
        });
    }
    if let Some((sample_count, zero_count)) = cell.determinant.counts() {
        if sample_count > cell.matrix_count || zero_count > sample_count {
            return Err(AccumulatorError::InvalidSnapshot {
                reason: "pooled determinant counts are inconsistent".to_owned(),
            });
        }
    }
    Ok(())
}
