//! Coding-owned tuning selectors and their process-wide typed accessor; the
//! generic process authority is [`gf2_core::tuning`]. The section holds two
//! selector families: `encode`, the profile half of the batch-encoding family
//! dispatch of [`crate::bch::encode::EncodeFamily`], and `decode`, the
//! batch-decoding bounds the section stores, validates and serializes.

use std::fmt;
use std::num::NonZeroUsize;

use gf2_core::tuning::{active_section, ActiveSection, SectionError, SectionId, TuningSection};

pub use crate::bch::encode::{
    BITSLICE_INTERLEAVED_MIN_BATCH, CLMUL_FOLD_MIN_BATCH, TABLE_REMAINDER_MIN_BATCH,
    TABLE_REMAINDER_MIN_REDUNDANCY,
};

/// Conservative [`DecodeSelectors::lane_min_frames`]: the largest value of
/// the field's domain.
pub const LANE_MIN_FRAMES: usize = usize::MAX;

/// Conservative [`DecodeSelectors::lane_max_edges`]: the smallest value of
/// the field's domain.
pub const LANE_MAX_EDGES: usize = 0;

/// Conservative upper bound on configured decode workers.
pub const LANE_MAX_WORKERS: NonZeroUsize = NonZeroUsize::MIN;

/// Conservative [`DecodeSelectors::granules_per_task`]: the smallest value of
/// the field's domain.
pub const GRANULES_PER_TASK: NonZeroUsize = NonZeroUsize::MIN;

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
    /// Builds the encode selector family. Every `usize` combination is
    /// admissible: zero opens an arm at every redundancy or batch length the
    /// representation implements it for, and [`usize::MAX`] closes it.
    ///
    /// # Errors
    ///
    /// Never returns an error; the [`SectionError`] result is the shape every
    /// owner codec builds a selector family through.
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

/// Bounds of the QC route ([`decide_batch_route`](crate::ldpc::decide_batch_route)).
///
/// A wave of a code with retained QC structure takes the QC route when its
/// lifting size is at least `min_lifting_size`, the configured decode workers
/// are at most `max_workers`, and no eligible lane route claims it: the lane
/// route claims a wave from `lane_claim_min_frames` occupied lanes at lifting
/// sizes up to `lane_claim_max_lifting_size`. Each bound spans its whole
/// domain; [`Self::OPEN`] places every QC wave on the QC route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QcRouteBounds {
    min_lifting_size: usize,
    max_workers: NonZeroUsize,
    lane_claim_min_frames: usize,
    lane_claim_max_lifting_size: usize,
}

impl QcRouteBounds {
    /// The bounds that take the QC route for every wave of a code with
    /// retained QC structure under a min-sum rule.
    pub const OPEN: Self = Self {
        min_lifting_size: 0,
        max_workers: NonZeroUsize::MAX,
        lane_claim_min_frames: usize::MAX,
        lane_claim_max_lifting_size: usize::MAX,
    };

    /// The bounds that close the QC route.
    pub const CLOSED: Self = Self {
        min_lifting_size: usize::MAX,
        max_workers: NonZeroUsize::MIN,
        lane_claim_min_frames: 0,
        lane_claim_max_lifting_size: usize::MAX,
    };

    /// Builds the bounds.
    ///
    /// # Errors
    ///
    /// Returns [`SectionError::SelectorOutOfRange`] when `max_workers` is zero.
    pub fn try_new(
        min_lifting_size: usize,
        max_workers: usize,
        lane_claim_min_frames: usize,
        lane_claim_max_lifting_size: usize,
    ) -> Result<Self, SectionError> {
        let max_workers =
            NonZeroUsize::new(max_workers).ok_or(SectionError::SelectorOutOfRange {
                family: "decode",
                field: "qc_route.max_workers",
                value: 0,
            })?;
        Ok(Self {
            min_lifting_size,
            max_workers,
            lane_claim_min_frames,
            lane_claim_max_lifting_size,
        })
    }

    /// Returns the lower bound on the lifting size.
    #[must_use]
    pub const fn min_lifting_size(&self) -> usize {
        self.min_lifting_size
    }

    /// Returns the upper bound on configured decode workers.
    #[must_use]
    pub const fn max_workers(&self) -> NonZeroUsize {
        self.max_workers
    }

    /// Returns the occupied lanes from which an eligible lane route takes a
    /// wave instead of the QC route.
    #[must_use]
    pub const fn lane_claim_min_frames(&self) -> usize {
        self.lane_claim_min_frames
    }

    /// Returns the largest lifting size at which an eligible lane route
    /// claims a wave.
    #[must_use]
    pub const fn lane_claim_max_lifting_size(&self) -> usize {
        self.lane_claim_max_lifting_size
    }
}

/// Validated decode selector family of the coding section.
///
/// `lane_min_frames` is a lower bound on a count of occupied lanes and
/// `lane_max_edges` an upper bound on a Tanner-graph edge count, each over
/// all of `usize`; `lane_max_workers` bounds configured decode concurrency;
/// `granules_per_task` is a positive count of
/// preferred-granularity frame groups; `qc_route` bounds the QC route. The
/// section stores, validates and serializes the family, and
/// [`decide_batch_route`](crate::ldpc::decide_batch_route) reads it to decide
/// the LDPC batch route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeSelectors {
    lane_min_frames: usize,
    lane_max_edges: usize,
    lane_max_workers: NonZeroUsize,
    granules_per_task: NonZeroUsize,
    qc_route: QcRouteBounds,
}

impl DecodeSelectors {
    /// The conservative decode family: [`LANE_MIN_FRAMES`],
    /// [`LANE_MAX_EDGES`], [`LANE_MAX_WORKERS`] and [`GRANULES_PER_TASK`], with
    /// [`QcRouteBounds::OPEN`]: the QC kernels have a portable fallback on
    /// every host, so the QC route is the default wherever a code retains QC
    /// structure.
    pub const CONSERVATIVE: Self = Self {
        lane_min_frames: LANE_MIN_FRAMES,
        lane_max_edges: LANE_MAX_EDGES,
        lane_max_workers: LANE_MAX_WORKERS,
        granules_per_task: GRANULES_PER_TASK,
        qc_route: QcRouteBounds::OPEN,
    };

    /// Builds the decode selector family with [`QcRouteBounds::OPEN`]. Every
    /// `lane_min_frames` and `lane_max_edges` is admissible.
    ///
    /// # Errors
    ///
    /// Returns [`SectionError::SelectorOutOfRange`] when `lane_max_workers` or
    /// `granules_per_task` is zero, because both counts are positive.
    pub fn try_new(
        lane_min_frames: usize,
        lane_max_edges: usize,
        lane_max_workers: usize,
        granules_per_task: usize,
    ) -> Result<Self, SectionError> {
        let lane_max_workers =
            NonZeroUsize::new(lane_max_workers).ok_or(SectionError::SelectorOutOfRange {
                family: "decode",
                field: "lane_max_workers",
                value: 0,
            })?;
        let granules_per_task =
            NonZeroUsize::new(granules_per_task).ok_or(SectionError::SelectorOutOfRange {
                family: "decode",
                field: "granules_per_task",
                value: 0,
            })?;
        Ok(Self {
            lane_min_frames,
            lane_max_edges,
            lane_max_workers,
            granules_per_task,
            qc_route: QcRouteBounds::OPEN,
        })
    }

    /// Replaces the bounds of the QC route.
    #[must_use]
    pub const fn with_qc_route(mut self, bounds: QcRouteBounds) -> Self {
        self.qc_route = bounds;
        self
    }

    /// Returns the lower bound on a count of occupied lanes.
    #[must_use]
    pub const fn lane_min_frames(&self) -> usize {
        self.lane_min_frames
    }

    /// Returns the upper bound on a Tanner-graph edge count.
    #[must_use]
    pub const fn lane_max_edges(&self) -> usize {
        self.lane_max_edges
    }

    /// Returns the upper bound on configured decode workers for lane eligibility.
    #[must_use]
    pub const fn lane_max_workers(&self) -> NonZeroUsize {
        self.lane_max_workers
    }

    /// Returns the count of preferred-granularity frame groups.
    #[must_use]
    pub const fn granules_per_task(&self) -> NonZeroUsize {
        self.granules_per_task
    }

    /// Returns the bounds of the QC route.
    #[must_use]
    pub const fn qc_route(&self) -> QcRouteBounds {
        self.qc_route
    }
}

/// The complete set of selector families the coding section carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodingSelectors {
    /// Batch-encoding family selectors.
    pub encode: EncodeSelectors,
    /// Decode selector family.
    pub decode: DecodeSelectors,
}

impl CodingSelectors {
    /// The parser-free conservative selector families: every encode bound is
    /// the corresponding [`crate::bch::encode`] constant, which selects the
    /// scalar reference for every code, and the decode family is
    /// [`DecodeSelectors::CONSERVATIVE`].
    pub const CONSERVATIVE: Self = Self {
        encode: EncodeSelectors {
            table_remainder_min_redundancy: TABLE_REMAINDER_MIN_REDUNDANCY,
            table_remainder_min_batch: TABLE_REMAINDER_MIN_BATCH,
            bitslice_interleaved_min_batch: BITSLICE_INTERLEAVED_MIN_BATCH,
            clmul_fold_min_batch: CLMUL_FOLD_MIN_BATCH,
        },
        decode: DecodeSelectors::CONSERVATIVE,
    };
}

#[cfg(feature = "tuning-profile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CodingPresence {
    encode: bool,
    table_remainder_min_redundancy: bool,
    table_remainder_min_batch: bool,
    bitslice_interleaved_min_batch: bool,
    clmul_fold_min_batch: bool,
    decode: bool,
    lane_min_frames: bool,
    lane_max_edges: bool,
    lane_max_workers: bool,
    granules_per_task: bool,
    qc_route: bool,
}

#[cfg(feature = "tuning-profile")]
impl CodingPresence {
    const COMPLETE: Self = Self {
        encode: true,
        table_remainder_min_redundancy: true,
        table_remainder_min_batch: true,
        bitslice_interleaved_min_batch: true,
        clmul_fold_min_batch: true,
        decode: true,
        lane_min_frames: true,
        lane_max_edges: true,
        lane_max_workers: true,
        granules_per_task: true,
        qc_route: true,
    };
}

/// Coding-owned tuning section carried by `gf2-coding/encode`, the one wire
/// identity of every coding selector family.
#[derive(Clone, PartialEq, Eq)]
pub struct CodingTuning {
    selectors: CodingSelectors,
    #[cfg(feature = "tuning-profile")]
    presence: CodingPresence,
}

impl CodingTuning {
    /// The parser-free conservative coding section.
    pub const CONSERVATIVE: Self = Self {
        selectors: CodingSelectors::CONSERVATIVE,
        #[cfg(feature = "tuning-profile")]
        presence: CodingPresence::COMPLETE,
    };

    /// Builds a complete programmatic coding section.
    #[must_use]
    pub const fn from_selectors(selectors: CodingSelectors) -> Self {
        Self {
            selectors,
            #[cfg(feature = "tuning-profile")]
            presence: CodingPresence::COMPLETE,
        }
    }

    /// Replaces the bounds of the QC route and declares them, with their
    /// decode family, in the section's serialized body. Every other selector
    /// keeps its value and its declaration or omission.
    #[must_use]
    pub fn with_qc_route(mut self, bounds: QcRouteBounds) -> Self {
        self.selectors.decode.qc_route = bounds;
        #[cfg(feature = "tuning-profile")]
        {
            self.presence.decode = true;
            self.presence.qc_route = true;
        }
        self
    }

    /// Returns the complete coding selector view.
    #[must_use]
    pub const fn selectors(&self) -> &CodingSelectors {
        &self.selectors
    }

    /// Returns the encode selector family consumed by the batch entry points.
    #[must_use]
    pub const fn encode(&self) -> &EncodeSelectors {
        &self.selectors.encode
    }

    /// Returns the decode selector family read by
    /// [`decide_batch_route`](crate::ldpc::decide_batch_route).
    #[must_use]
    pub const fn decode(&self) -> &DecodeSelectors {
        &self.selectors.decode
    }
}

impl fmt::Debug for CodingTuning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CodingTuning");
        debug.field("selectors", &self.selectors);
        #[cfg(feature = "tuning-profile")]
        {
            let declared_families =
                usize::from(self.presence.encode) + usize::from(self.presence.decode);
            let declared_fields = usize::from(self.presence.table_remainder_min_redundancy)
                + usize::from(self.presence.table_remainder_min_batch)
                + usize::from(self.presence.bitslice_interleaved_min_batch)
                + usize::from(self.presence.clmul_fold_min_batch)
                + usize::from(self.presence.lane_min_frames)
                + usize::from(self.presence.lane_max_edges)
                + usize::from(self.presence.lane_max_workers)
                + usize::from(self.presence.granules_per_task)
                + usize::from(self.presence.qc_route);
            debug
                .field("declared_families", &declared_families)
                .field("declared_fields", &declared_fields);
        }
        debug.finish()
    }
}

impl TuningSection for CodingTuning {
    const ID: SectionId = SectionId::from_static("gf2-coding/encode");
    type Selectors = CodingSelectors;

    fn conservative() -> &'static Self {
        static CONSERVATIVE: CodingTuning = CodingTuning::CONSERVATIVE;
        &CONSERVATIVE
    }

    fn selectors(&self) -> &Self::Selectors {
        &self.selectors
    }
}

/// Returns the process-wide coding tuning section and resolution provenance;
/// the returned section holds every selector family.
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
    /// Measurement-behavior token accepted for calibrated coding sections,
    /// the token the coding producer in `experiments/tuning-calibration` uses.
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
        let (encode_present, encode) = family_or_default(selectors.encode, "encode")?;
        let (decode_present, decode) = family_or_default(selectors.decode, "decode")?;
        let (redundancy_present, table_remainder_min_redundancy) = selector_or(
            encode.table_remainder_min_redundancy,
            TABLE_REMAINDER_MIN_REDUNDANCY,
            "table_remainder_min_redundancy",
        )?;
        let (batch_present, table_remainder_min_batch) = selector_or(
            encode.table_remainder_min_batch,
            TABLE_REMAINDER_MIN_BATCH,
            "table_remainder_min_batch",
        )?;
        let (bitslice_present, bitslice_interleaved_min_batch) = selector_or(
            encode.bitslice_interleaved_min_batch,
            BITSLICE_INTERLEAVED_MIN_BATCH,
            "bitslice_interleaved_min_batch",
        )?;
        let (fold_present, clmul_fold_min_batch) = selector_or(
            encode.clmul_fold_min_batch,
            CLMUL_FOLD_MIN_BATCH,
            "clmul_fold_min_batch",
        )?;
        let (lane_frames_present, lane_min_frames) =
            selector_or(decode.lane_min_frames, LANE_MIN_FRAMES, "lane_min_frames")?;
        let (lane_edges_present, lane_max_edges) =
            selector_or(decode.lane_max_edges, LANE_MAX_EDGES, "lane_max_edges")?;
        let (workers_present, lane_max_workers) = selector_or(
            decode.lane_max_workers,
            LANE_MAX_WORKERS.get(),
            "lane_max_workers",
        )?;
        let (granules_present, granules_per_task) = selector_or(
            decode.granules_per_task,
            GRANULES_PER_TASK.get(),
            "granules_per_task",
        )?;
        let (qc_route_present, qc_route) = selector_or(
            decode.qc_route,
            JsonQcRoute::from(QcRouteBounds::OPEN),
            "qc_route",
        )?;
        let qc_route = QcRouteBounds::try_new(
            qc_route.min_lifting_size,
            qc_route.max_workers,
            qc_route.lane_claim_min_frames,
            qc_route.lane_claim_max_lifting_size,
        )?;
        Ok(CodingTuning {
            selectors: CodingSelectors {
                encode: EncodeSelectors::try_new(
                    table_remainder_min_redundancy,
                    table_remainder_min_batch,
                    bitslice_interleaved_min_batch,
                    clmul_fold_min_batch,
                )?,
                decode: DecodeSelectors::try_new(
                    lane_min_frames,
                    lane_max_edges,
                    lane_max_workers,
                    granules_per_task,
                )?
                .with_qc_route(qc_route),
            },
            presence: CodingPresence {
                encode: encode_present,
                table_remainder_min_redundancy: redundancy_present,
                table_remainder_min_batch: batch_present,
                bitslice_interleaved_min_batch: bitslice_present,
                clmul_fold_min_batch: fold_present,
                decode: decode_present,
                lane_min_frames: lane_frames_present,
                lane_max_edges: lane_edges_present,
                lane_max_workers: workers_present,
                granules_per_task: granules_present,
                qc_route: qc_route_present,
            },
        })
    }

    fn encode_body(
        section: &CodingTuning,
    ) -> Result<gf2_core::tuning::CanonicalValue, SectionError> {
        use std::collections::BTreeMap;

        let mut selectors = BTreeMap::<&str, gf2_core::tuning::CanonicalValue>::new();
        if section.presence.encode {
            let family = section.encode();
            let mut encode = BTreeMap::new();
            if section.presence.table_remainder_min_redundancy {
                encode.insert(
                    "table_remainder_min_redundancy",
                    family.table_remainder_min_redundancy(),
                );
            }
            if section.presence.table_remainder_min_batch {
                encode.insert(
                    "table_remainder_min_batch",
                    family.table_remainder_min_batch(),
                );
            }
            if section.presence.bitslice_interleaved_min_batch {
                encode.insert(
                    "bitslice_interleaved_min_batch",
                    family.bitslice_interleaved_min_batch(),
                );
            }
            if section.presence.clmul_fold_min_batch {
                encode.insert("clmul_fold_min_batch", family.clmul_fold_min_batch());
            }
            selectors.insert(
                "encode",
                gf2_core::tuning::CanonicalValue::serialize(&encode)?,
            );
        }
        if section.presence.decode {
            let family = section.decode();
            let mut decode = BTreeMap::new();
            if section.presence.lane_min_frames {
                decode.insert(
                    "lane_min_frames",
                    JsonDecodeField::Bound(family.lane_min_frames()),
                );
            }
            if section.presence.lane_max_edges {
                decode.insert(
                    "lane_max_edges",
                    JsonDecodeField::Bound(family.lane_max_edges()),
                );
            }
            if section.presence.lane_max_workers {
                decode.insert(
                    "lane_max_workers",
                    JsonDecodeField::Bound(family.lane_max_workers().get()),
                );
            }
            if section.presence.granules_per_task {
                decode.insert(
                    "granules_per_task",
                    JsonDecodeField::Bound(family.granules_per_task().get()),
                );
            }
            if section.presence.qc_route {
                decode.insert("qc_route", JsonDecodeField::Qc(family.qc_route().into()));
            }
            selectors.insert(
                "decode",
                gf2_core::tuning::CanonicalValue::serialize(&decode)?,
            );
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
    #[serde(default)]
    decode: Present<JsonDecode>,
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
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonDecode {
    #[serde(default)]
    lane_min_frames: Present<usize>,
    #[serde(default)]
    lane_max_edges: Present<usize>,
    #[serde(default)]
    lane_max_workers: Present<usize>,
    #[serde(default)]
    granules_per_task: Present<usize>,
    #[serde(default)]
    qc_route: Present<JsonQcRoute>,
}

#[cfg(feature = "tuning-profile")]
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct JsonQcRoute {
    min_lifting_size: usize,
    max_workers: usize,
    lane_claim_min_frames: usize,
    lane_claim_max_lifting_size: usize,
}

#[cfg(feature = "tuning-profile")]
impl From<QcRouteBounds> for JsonQcRoute {
    fn from(bounds: QcRouteBounds) -> Self {
        Self {
            min_lifting_size: bounds.min_lifting_size(),
            max_workers: bounds.max_workers().get(),
            lane_claim_min_frames: bounds.lane_claim_min_frames(),
            lane_claim_max_lifting_size: bounds.lane_claim_max_lifting_size(),
        }
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(serde::Serialize)]
#[serde(untagged)]
enum JsonDecodeField {
    Bound(usize),
    Qc(JsonQcRoute),
}

/// Returns whether `family` is present and its fields, all missing for an
/// absent family.
#[cfg(feature = "tuning-profile")]
fn family_or_default<T: Default>(
    family: Present<T>,
    name: &str,
) -> Result<(bool, T), SectionError> {
    match family {
        Present::Missing => Ok((false, T::default())),
        Present::Value(Some(family)) => Ok((true, family)),
        Present::Value(None) => Err(SectionError::InvalidBody(format!("null {name} family"))),
    }
}

/// Returns whether `selector` is present and its value, `conservative` for an
/// absent selector.
#[cfg(feature = "tuning-profile")]
fn selector_or<T>(
    selector: Present<T>,
    conservative: T,
    name: &str,
) -> Result<(bool, T), SectionError> {
    match selector {
        Present::Missing => Ok((false, conservative)),
        Present::Value(Some(value)) => Ok((true, value)),
        Present::Value(None) => Err(SectionError::InvalidBody(format!("null {name} selector"))),
    }
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
