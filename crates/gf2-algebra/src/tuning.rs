//! Algebra-owned permanent selectors and process-wide typed access.
//!
//! The generic process authority lives in [`gf2_core::tuning`]. This module
//! owns only the permanent algorithm's selector vocabulary, conservative
//! value, optional format-2 codec, and typed accessor.

use std::fmt;

use gf2_core::tuning::{active_section, ActiveSection, SectionError, SectionId, TuningSection};

/// Conservative number of Gray-code subsets assigned to one parallel chunk.
pub use crate::permanent::CHUNK_SUBSETS;

/// Validated selectors for parallel permanent algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermanentSelectors {
    gray_chunk_subsets: usize,
}

impl PermanentSelectors {
    /// Builds permanent selectors with a nonzero parallel chunk length.
    ///
    /// # Errors
    ///
    /// Returns [`SectionError::SelectorOutOfRange`] when
    /// `gray_chunk_subsets` is zero.
    pub fn try_new(gray_chunk_subsets: usize) -> Result<Self, SectionError> {
        if gray_chunk_subsets == 0 {
            return Err(SectionError::SelectorOutOfRange {
                family: "permanent",
                field: "gray_chunk_subsets",
                value: 0,
            });
        }
        Ok(Self { gray_chunk_subsets })
    }

    /// Returns the number of Gray-code subsets assigned to one worker chunk.
    #[must_use]
    pub const fn gray_chunk_subsets(&self) -> usize {
        self.gray_chunk_subsets
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AlgebraPresence {
    permanent: bool,
    gray_chunk_subsets: bool,
}

#[cfg(feature = "tuning-profile")]
impl AlgebraPresence {
    const COMPLETE: Self = Self {
        permanent: true,
        gray_chunk_subsets: true,
    };
}

/// Algebra-owned tuning section carried by `gf2-algebra/permanent`.
#[derive(Clone, PartialEq, Eq)]
pub struct AlgebraTuning {
    permanent: PermanentSelectors,
    #[cfg(feature = "tuning-profile")]
    presence: AlgebraPresence,
}

impl AlgebraTuning {
    /// Complete parser-free conservative algebra section.
    pub const CONSERVATIVE: Self = Self {
        permanent: PermanentSelectors {
            gray_chunk_subsets: CHUNK_SUBSETS,
        },
        #[cfg(feature = "tuning-profile")]
        presence: AlgebraPresence::COMPLETE,
    };

    /// Builds a complete programmatic algebra section.
    #[must_use]
    pub const fn from_selectors(permanent: PermanentSelectors) -> Self {
        Self {
            permanent,
            #[cfg(feature = "tuning-profile")]
            presence: AlgebraPresence::COMPLETE,
        }
    }

    /// Returns the permanent selector view consumed by algebra algorithms.
    #[must_use]
    pub const fn permanent(&self) -> &PermanentSelectors {
        &self.permanent
    }
}

impl fmt::Debug for AlgebraTuning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("AlgebraTuning");
        debug.field("permanent", &self.permanent);
        #[cfg(feature = "tuning-profile")]
        {
            let declared_families = usize::from(self.presence.permanent);
            let declared_fields = usize::from(self.presence.gray_chunk_subsets);
            debug
                .field("declared_families", &declared_families)
                .field("declared_fields", &declared_fields);
        }
        debug.finish()
    }
}

impl TuningSection for AlgebraTuning {
    const ID: SectionId = SectionId::from_static("gf2-algebra/permanent");
    type Selectors = PermanentSelectors;

    fn conservative() -> &'static Self {
        &Self::CONSERVATIVE
    }

    fn selectors(&self) -> &Self::Selectors {
        &self.permanent
    }
}

/// Returns the process-wide algebra tuning section and resolution provenance.
#[must_use]
#[track_caller]
pub fn active() -> ActiveSection<'static, AlgebraTuning> {
    active_section::<AlgebraTuning>()
}

#[cfg(feature = "tuning-profile")]
const ALGEBRA_HARNESS_SCHEMA: &str = "algebra-tuning-calibration-v1";

/// Format-2 owner codec for [`AlgebraTuning`].
#[cfg(feature = "tuning-profile")]
pub struct AlgebraTuningCodec;

#[cfg(feature = "tuning-profile")]
impl AlgebraTuningCodec {
    /// Measurement-behavior token accepted for calibrated algebra sections.
    pub const HARNESS_SCHEMA: &'static str = ALGEBRA_HARNESS_SCHEMA;
}

#[cfg(feature = "tuning-profile")]
impl gf2_core::tuning::SectionCodec<AlgebraTuning> for AlgebraTuningCodec {
    const SCHEMA_VERSION: u32 = 1;

    fn validate_measurement(
        value: &gf2_core::tuning::MeasurementProvenance,
    ) -> Result<(), SectionError> {
        match value {
            gf2_core::tuning::MeasurementProvenance::Inherited => Ok(()),
            gf2_core::tuning::MeasurementProvenance::Calibrated { harness_schema, .. }
                if harness_schema.as_str() == ALGEBRA_HARNESS_SCHEMA =>
            {
                Ok(())
            }
            gf2_core::tuning::MeasurementProvenance::Calibrated { harness_schema, .. } => Err(
                SectionError::unsupported_harness_found(harness_schema, ALGEBRA_HARNESS_SCHEMA),
            ),
        }
    }

    fn decode_body(body: gf2_core::tuning::CanonicalValue) -> Result<AlgebraTuning, SectionError> {
        let selectors: JsonAlgebraSelectors = body.deserialize()?;
        let (permanent_present, permanent) = match selectors.permanent {
            Present::Missing => (false, JsonPermanent::default()),
            Present::Value(Some(permanent)) => (true, permanent),
            Present::Value(None) => {
                return Err(SectionError::InvalidBody(
                    "null permanent family".to_owned(),
                ))
            }
        };
        let (field_present, gray_chunk_subsets) = match permanent.gray_chunk_subsets {
            Present::Missing => (false, CHUNK_SUBSETS),
            Present::Value(Some(value)) => (true, value),
            Present::Value(None) => {
                return Err(SectionError::InvalidBody(
                    "null gray_chunk_subsets selector".to_owned(),
                ))
            }
        };
        Ok(AlgebraTuning {
            permanent: PermanentSelectors::try_new(gray_chunk_subsets)?,
            presence: AlgebraPresence {
                permanent: permanent_present,
                gray_chunk_subsets: field_present,
            },
        })
    }

    fn encode_body(
        section: &AlgebraTuning,
    ) -> Result<gf2_core::tuning::CanonicalValue, SectionError> {
        use std::collections::BTreeMap;

        let mut selectors = BTreeMap::<&str, BTreeMap<&str, usize>>::new();
        if section.presence.permanent {
            let mut permanent = BTreeMap::new();
            if section.presence.gray_chunk_subsets {
                permanent.insert("gray_chunk_subsets", section.permanent.gray_chunk_subsets());
            }
            selectors.insert("permanent", permanent);
        }
        gf2_core::tuning::CanonicalValue::serialize(&selectors)
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonAlgebraSelectors {
    #[serde(default)]
    permanent: Present<JsonPermanent>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPermanent {
    #[serde(default)]
    gray_chunk_subsets: Present<usize>,
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
