//! Coding-owned selectors and process-wide typed access.
//!
//! The generic process authority lives in [`gf2_core::tuning`]. This module
//! owns only the coding crate's selector vocabulary, conservative value,
//! optional format-2 codec, and typed accessor.
//!
//! The one selector family carried here is `encode`, the profile half of the
//! batch-encoding family dispatch described in
//! [`crate::bch::encode`](crate::bch::encode#algorithm-families). Its
//! thresholds decide which registered family the batch entry points select;
//! the other half is representation availability, which the kernels answer
//! and no profile can override.

use std::fmt;

use gf2_core::tuning::{active_section, ActiveSection, SectionError, SectionId, TuningSection};

pub use crate::bch::encode::{
    BITSLICE_INTERLEAVED_MIN_BATCH, CLMUL_FOLD_MIN_BATCH, TABLE_REMAINDER_MIN_BATCH,
    TABLE_REMAINDER_MIN_REDUNDANCY,
};

/// Validated selectors for batch-encoding family dispatch.
///
/// Every field is a lower bound a plan or batch must clear before the family
/// it names is selected: the first two before
/// [`EncodeFamily::TableRemainder`](crate::bch::encode::EncodeFamily::TableRemainder),
/// then one each for
/// [`EncodeFamily::BitsliceInterleaved`](crate::bch::encode::EncodeFamily::BitsliceInterleaved)
/// and
/// [`EncodeFamily::ClmulFold`](crate::bch::encode::EncodeFamily::ClmulFold).
/// They compose with, and never override, the representation availability the
/// kernels report, so no selector value can select a family that cannot run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodeSelectors {
    table_remainder_min_redundancy: usize,
    table_remainder_min_batch: usize,
    bitslice_interleaved_min_batch: usize,
    clmul_fold_min_batch: usize,
}

impl EncodeSelectors {
    /// Builds the encode selector family.
    ///
    /// Every `usize` combination is admissible, because a bound decides only
    /// which of several equivalent algorithms runs: zero opens an arm at
    /// every redundancy or batch length the representation implements it for,
    /// and [`usize::MAX`] closes it, which is the value the conservative
    /// section carries for the table family's redundancy bound and for the
    /// bit-sliced and fold families' batch bounds.
    ///
    /// # Errors
    ///
    /// The [`SectionError`] result is the shape every owner codec builds a
    /// selector family through; this vocabulary admits every value, so the
    /// error arm stays open for a bound a later selector constrains.
    pub fn try_new(
        table_remainder_min_redundancy: usize,
        table_remainder_min_batch: usize,
        bitslice_interleaved_min_batch: usize,
        clmul_fold_min_batch: usize,
    ) -> Result<Self, SectionError> {
        Ok(Self {
            table_remainder_min_redundancy,
            table_remainder_min_batch,
            bitslice_interleaved_min_batch,
            clmul_fold_min_batch,
        })
    }

    /// Returns the minimum redundancy $r$ at which the table family is selected.
    #[must_use]
    pub const fn table_remainder_min_redundancy(&self) -> usize {
        self.table_remainder_min_redundancy
    }

    /// Returns the minimum batch length at which the table family is selected.
    #[must_use]
    pub const fn table_remainder_min_batch(&self) -> usize {
        self.table_remainder_min_batch
    }

    /// Returns the minimum batch length at which the bit-sliced interleaved
    /// family is selected.
    #[must_use]
    pub const fn bitslice_interleaved_min_batch(&self) -> usize {
        self.bitslice_interleaved_min_batch
    }

    /// Returns the minimum batch length at which the carry-less-multiply fold
    /// family is selected.
    #[must_use]
    pub const fn clmul_fold_min_batch(&self) -> usize {
        self.clmul_fold_min_batch
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CodingPresence {
    encode: bool,
    table_remainder_min_redundancy: bool,
    table_remainder_min_batch: bool,
    bitslice_interleaved_min_batch: bool,
    clmul_fold_min_batch: bool,
}

#[cfg(feature = "tuning-profile")]
impl CodingPresence {
    const COMPLETE: Self = Self {
        encode: true,
        table_remainder_min_redundancy: true,
        table_remainder_min_batch: true,
        bitslice_interleaved_min_batch: true,
        clmul_fold_min_batch: true,
    };
}

/// Coding-owned tuning section carried by `gf2-coding/encode`.
#[derive(Clone, PartialEq, Eq)]
pub struct CodingTuning {
    encode: EncodeSelectors,
    #[cfg(feature = "tuning-profile")]
    presence: CodingPresence,
}

impl CodingTuning {
    /// Complete parser-free conservative coding section.
    ///
    /// The table family's redundancy bound is
    /// [`TABLE_REMAINDER_MIN_REDUNDANCY`], the bit-sliced family's batch
    /// bound is [`BITSLICE_INTERLEAVED_MIN_BATCH`], and the fold family's is
    /// [`CLMUL_FOLD_MIN_BATCH`], so the conservative section selects the
    /// scalar reference for every code: the crossovers among the families are
    /// measurements no committed receipt has made yet.
    pub const CONSERVATIVE: Self = Self {
        encode: EncodeSelectors {
            table_remainder_min_redundancy: TABLE_REMAINDER_MIN_REDUNDANCY,
            table_remainder_min_batch: TABLE_REMAINDER_MIN_BATCH,
            bitslice_interleaved_min_batch: BITSLICE_INTERLEAVED_MIN_BATCH,
            clmul_fold_min_batch: CLMUL_FOLD_MIN_BATCH,
        },
        #[cfg(feature = "tuning-profile")]
        presence: CodingPresence::COMPLETE,
    };

    /// Builds a complete programmatic coding section.
    #[must_use]
    pub const fn from_selectors(encode: EncodeSelectors) -> Self {
        Self {
            encode,
            #[cfg(feature = "tuning-profile")]
            presence: CodingPresence::COMPLETE,
        }
    }

    /// Returns the encode selector view consumed by the batch entry points.
    #[must_use]
    pub const fn encode(&self) -> &EncodeSelectors {
        &self.encode
    }
}

impl fmt::Debug for CodingTuning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CodingTuning");
        debug.field("encode", &self.encode);
        #[cfg(feature = "tuning-profile")]
        {
            let declared_families = usize::from(self.presence.encode);
            let declared_fields = usize::from(self.presence.table_remainder_min_redundancy)
                + usize::from(self.presence.table_remainder_min_batch)
                + usize::from(self.presence.bitslice_interleaved_min_batch)
                + usize::from(self.presence.clmul_fold_min_batch);
            debug
                .field("declared_families", &declared_families)
                .field("declared_fields", &declared_fields);
        }
        debug.finish()
    }
}

impl TuningSection for CodingTuning {
    const ID: SectionId = SectionId::from_static("gf2-coding/encode");
    type Selectors = EncodeSelectors;

    fn conservative() -> &'static Self {
        &Self::CONSERVATIVE
    }

    fn selectors(&self) -> &Self::Selectors {
        &self.encode
    }
}

/// Returns the process-wide coding tuning section and resolution provenance.
///
/// This wrapper performs the generic accessor's one `O(log s)` installed
/// section lookup (`s` is the number of installed sections) and does not
/// allocate or lock after process resolution. Resolve it once at the outer
/// batch boundary and pass the value into the partition loop.
///
/// # Panics
///
/// Panics with [`gf2_core::tuning::ActiveSectionInvariant`] only if installed
/// erased storage violates the typed section invariant. Malformed profile
/// input is rejected before installation.
#[must_use]
#[track_caller]
pub fn active() -> ActiveSection<'static, CodingTuning> {
    active_section::<CodingTuning>()
}

#[cfg(feature = "tuning-profile")]
const CODING_HARNESS_SCHEMA: &str = "coding-tuning-calibration-v1";

/// Format-2 owner codec for [`CodingTuning`].
#[cfg(feature = "tuning-profile")]
pub struct CodingTuningCodec;

#[cfg(feature = "tuning-profile")]
impl CodingTuningCodec {
    /// Measurement-behavior token accepted for calibrated coding sections.
    pub const HARNESS_SCHEMA: &'static str = CODING_HARNESS_SCHEMA;
}

#[cfg(feature = "tuning-profile")]
impl gf2_core::tuning::SectionCodec<CodingTuning> for CodingTuningCodec {
    const SCHEMA_VERSION: u32 = 1;

    fn validate_measurement(
        value: &gf2_core::tuning::MeasurementProvenance,
    ) -> Result<(), SectionError> {
        match value {
            gf2_core::tuning::MeasurementProvenance::Inherited => Ok(()),
            gf2_core::tuning::MeasurementProvenance::Calibrated { harness_schema, .. }
                if harness_schema.as_str() == CODING_HARNESS_SCHEMA =>
            {
                Ok(())
            }
            gf2_core::tuning::MeasurementProvenance::Calibrated { harness_schema, .. } => Err(
                SectionError::unsupported_harness_found(harness_schema, CODING_HARNESS_SCHEMA),
            ),
        }
    }

    fn decode_body(body: gf2_core::tuning::CanonicalValue) -> Result<CodingTuning, SectionError> {
        let selectors: JsonCodingSelectors = body.deserialize()?;
        let (encode_present, encode) = match selectors.encode {
            Present::Missing => (false, JsonEncode::default()),
            Present::Value(Some(encode)) => (true, encode),
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null encode family".to_owned()))
            }
        };
        let (redundancy_present, table_remainder_min_redundancy) =
            match encode.table_remainder_min_redundancy {
                Present::Missing => (false, TABLE_REMAINDER_MIN_REDUNDANCY),
                Present::Value(Some(value)) => (true, value),
                Present::Value(None) => {
                    return Err(SectionError::InvalidBody(
                        "null table_remainder_min_redundancy selector".to_owned(),
                    ))
                }
            };
        let (batch_present, table_remainder_min_batch) = match encode.table_remainder_min_batch {
            Present::Missing => (false, TABLE_REMAINDER_MIN_BATCH),
            Present::Value(Some(value)) => (true, value),
            Present::Value(None) => {
                return Err(SectionError::InvalidBody(
                    "null table_remainder_min_batch selector".to_owned(),
                ))
            }
        };
        let (bitslice_present, bitslice_interleaved_min_batch) =
            match encode.bitslice_interleaved_min_batch {
                Present::Missing => (false, BITSLICE_INTERLEAVED_MIN_BATCH),
                Present::Value(Some(value)) => (true, value),
                Present::Value(None) => {
                    return Err(SectionError::InvalidBody(
                        "null bitslice_interleaved_min_batch selector".to_owned(),
                    ))
                }
            };
        let (fold_present, clmul_fold_min_batch) = match encode.clmul_fold_min_batch {
            Present::Missing => (false, CLMUL_FOLD_MIN_BATCH),
            Present::Value(Some(value)) => (true, value),
            Present::Value(None) => {
                return Err(SectionError::InvalidBody(
                    "null clmul_fold_min_batch selector".to_owned(),
                ))
            }
        };
        Ok(CodingTuning {
            encode: EncodeSelectors::try_new(
                table_remainder_min_redundancy,
                table_remainder_min_batch,
                bitslice_interleaved_min_batch,
                clmul_fold_min_batch,
            )?,
            presence: CodingPresence {
                encode: encode_present,
                table_remainder_min_redundancy: redundancy_present,
                table_remainder_min_batch: batch_present,
                bitslice_interleaved_min_batch: bitslice_present,
                clmul_fold_min_batch: fold_present,
            },
        })
    }

    fn encode_body(
        section: &CodingTuning,
    ) -> Result<gf2_core::tuning::CanonicalValue, SectionError> {
        use std::collections::BTreeMap;

        let mut selectors = BTreeMap::<&str, BTreeMap<&str, usize>>::new();
        if section.presence.encode {
            let mut encode = BTreeMap::new();
            if section.presence.table_remainder_min_redundancy {
                encode.insert(
                    "table_remainder_min_redundancy",
                    section.encode.table_remainder_min_redundancy(),
                );
            }
            if section.presence.table_remainder_min_batch {
                encode.insert(
                    "table_remainder_min_batch",
                    section.encode.table_remainder_min_batch(),
                );
            }
            if section.presence.bitslice_interleaved_min_batch {
                encode.insert(
                    "bitslice_interleaved_min_batch",
                    section.encode.bitslice_interleaved_min_batch(),
                );
            }
            if section.presence.clmul_fold_min_batch {
                encode.insert(
                    "clmul_fold_min_batch",
                    section.encode.clmul_fold_min_batch(),
                );
            }
            selectors.insert("encode", encode);
        }
        gf2_core::tuning::CanonicalValue::serialize(&selectors)
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCodingSelectors {
    #[serde(default)]
    encode: Present<JsonEncode>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonEncode {
    #[serde(default)]
    table_remainder_min_redundancy: Present<usize>,
    #[serde(default)]
    table_remainder_min_batch: Present<usize>,
    #[serde(default)]
    bitslice_interleaved_min_batch: Present<usize>,
    #[serde(default)]
    clmul_fold_min_batch: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default)]
enum Present<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

#[cfg(feature = "tuning-profile")]
impl<'de, T> serde::Deserialize<'de> for Present<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Self::Value)
    }
}
