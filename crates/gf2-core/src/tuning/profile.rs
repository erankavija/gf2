//! Strict profile-format-2 canonical JSON and typed owner-codec registry.

use std::any::{type_name, Any, TypeId};
use std::collections::{btree_map::Entry, BTreeMap};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256 as Sha256Hasher};

use super::mechanism::{
    valid_owned_section_id, ErasedSection, MeasurementProvenance, PreparedEnvelope, PreparedOrigin,
    SectionError, SectionId, TuningSection, VerifiedAssembly, PROFILE_FORMAT_VERSION,
};
use super::{
    AssemblyProvenance, GitRevision, HarnessSchema, ProfileError, ProfileId, RepoRelPath,
    Rfc3339Utc, Sha256,
};

/// An opaque deterministic JSON value used at owner-codec boundaries.
///
/// Object members are ordered lexicographically, duplicate keys and floating-
/// point numbers are rejected, and serialization contains no insignificant
/// whitespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalValue(CanonicalNode);

#[derive(Clone, Debug, PartialEq, Eq)]
enum CanonicalNode {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    String(String),
    Array(Vec<CanonicalValue>),
    Object(BTreeMap<String, CanonicalValue>),
}

impl CanonicalValue {
    /// Converts a serializable owner value to canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SectionError::InvalidBody`] if serialization fails or emits a
    /// floating-point number.
    pub fn serialize<T: Serialize>(value: &T) -> Result<Self, SectionError> {
        let json = serde_json::to_value(value)
            .map_err(|error| SectionError::InvalidBody(error.to_string()))?;
        Self::from_json_value(json)
    }

    /// Deserializes this canonical value into an owner codec's private wire type.
    ///
    /// # Errors
    ///
    /// Returns [`SectionError::InvalidBody`] when the requested type rejects the
    /// value.
    pub fn deserialize<T: DeserializeOwned>(self) -> Result<T, SectionError> {
        serde_json::from_value(self.into_json_value())
            .map_err(|error| SectionError::InvalidBody(error.to_string()))
    }

    fn from_json_value(value: serde_json::Value) -> Result<Self, SectionError> {
        let node = match value {
            serde_json::Value::Null => CanonicalNode::Null,
            serde_json::Value::Bool(value) => CanonicalNode::Bool(value),
            serde_json::Value::Number(value) => {
                if let Some(value) = value.as_u64() {
                    CanonicalNode::U64(value)
                } else if let Some(value) = value.as_i64() {
                    CanonicalNode::I64(value)
                } else {
                    return Err(SectionError::InvalidBody(
                        "floating-point JSON numbers are forbidden".to_owned(),
                    ));
                }
            }
            serde_json::Value::String(value) => CanonicalNode::String(value),
            serde_json::Value::Array(values) => CanonicalNode::Array(
                values
                    .into_iter()
                    .map(Self::from_json_value)
                    .collect::<Result<_, _>>()?,
            ),
            serde_json::Value::Object(values) => CanonicalNode::Object(
                values
                    .into_iter()
                    .map(|(key, value)| Ok((key, Self::from_json_value(value)?)))
                    .collect::<Result<_, SectionError>>()?,
            ),
        };
        Ok(Self(node))
    }

    fn into_json_value(self) -> serde_json::Value {
        match self.0 {
            CanonicalNode::Null => serde_json::Value::Null,
            CanonicalNode::Bool(value) => serde_json::Value::Bool(value),
            CanonicalNode::I64(value) => serde_json::Value::Number(value.into()),
            CanonicalNode::U64(value) => serde_json::Value::Number(value.into()),
            CanonicalNode::String(value) => serde_json::Value::String(value),
            CanonicalNode::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(Self::into_json_value).collect())
            }
            CanonicalNode::Object(values) => serde_json::Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, value.into_json_value()))
                    .collect(),
            ),
        }
    }
}

impl Serialize for CanonicalValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.0 {
            CanonicalNode::Null => serializer.serialize_unit(),
            CanonicalNode::Bool(value) => serializer.serialize_bool(*value),
            CanonicalNode::I64(value) => serializer.serialize_i64(*value),
            CanonicalNode::U64(value) => serializer.serialize_u64(*value),
            CanonicalNode::String(value) => serializer.serialize_str(value),
            CanonicalNode::Array(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(value)?;
                }
                sequence.end()
            }
            CanonicalNode::Object(values) => {
                let mut map = serializer.serialize_map(Some(values.len()))?;
                for (key, value) in values {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for CanonicalValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CanonicalVisitor;

        impl<'de> Visitor<'de> for CanonicalVisitor {
            type Value = CanonicalValue;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("canonical JSON without duplicate keys or floating point")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(CanonicalValue(CanonicalNode::Null))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_unit()
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(CanonicalValue(CanonicalNode::Bool(value)))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(CanonicalValue(CanonicalNode::I64(value)))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(CanonicalValue(CanonicalNode::U64(value)))
            }

            fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom("floating-point JSON numbers are forbidden"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(CanonicalValue(CanonicalNode::String(value.to_owned())))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(CanonicalValue(CanonicalNode::String(value)))
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(CanonicalValue(CanonicalNode::Array(values)))
            }

            fn visit_map<A>(self, mut input: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = input.next_entry::<String, CanonicalValue>()? {
                    match values.entry(key) {
                        Entry::Vacant(entry) => {
                            entry.insert(value);
                        }
                        Entry::Occupied(entry) => {
                            return Err(serde::de::Error::custom(format!(
                                "duplicate JSON key {:?}",
                                entry.key()
                            )));
                        }
                    }
                }
                Ok(CanonicalValue(CanonicalNode::Object(values)))
            }
        }

        deserializer.deserialize_any(CanonicalVisitor)
    }
}

/// The sole codec for one crate-owned tuning-section schema version.
pub trait SectionCodec<T: TuningSection>: Send + Sync + 'static {
    /// The one section schema version this codec accepts and emits.
    const SCHEMA_VERSION: u32;

    /// Validates section-local measurement evidence.
    fn validate_measurement(value: &MeasurementProvenance) -> Result<(), SectionError>;

    /// Decodes and validates an owner body, resolving omitted fields as defined by the owner.
    fn decode_body(body: CanonicalValue) -> Result<T, SectionError>;

    /// Re-encodes a typed owner value into canonical JSON.
    fn encode_body(section: &T) -> Result<CanonicalValue, SectionError>;
}

trait ErasedCodec: Send + Sync {
    fn section_id(&self) -> SectionId;
    fn section_type_id(&self) -> TypeId;
    fn section_type_name(&self) -> &'static str;
    fn schema_version(&self) -> u32;
    fn decode(
        &self,
        measurement: &MeasurementProvenance,
        body: CanonicalValue,
    ) -> Result<Arc<dyn Any + Send + Sync>, SectionError>;
    fn encode(
        &self,
        measurement: &MeasurementProvenance,
        section: &(dyn Any + Send + Sync),
    ) -> Result<CanonicalValue, SectionError>;
}

struct TypedErasedCodec<T, C>(PhantomData<(T, C)>);

impl<T, C> ErasedCodec for TypedErasedCodec<T, C>
where
    T: TuningSection,
    C: SectionCodec<T>,
{
    fn section_id(&self) -> SectionId {
        T::ID
    }

    fn section_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn section_type_name(&self) -> &'static str {
        type_name::<T>()
    }

    fn schema_version(&self) -> u32 {
        C::SCHEMA_VERSION
    }

    fn decode(
        &self,
        measurement: &MeasurementProvenance,
        body: CanonicalValue,
    ) -> Result<Arc<dyn Any + Send + Sync>, SectionError> {
        C::validate_measurement(measurement)?;
        Ok(Arc::new(C::decode_body(body)?))
    }

    fn encode(
        &self,
        measurement: &MeasurementProvenance,
        section: &(dyn Any + Send + Sync),
    ) -> Result<CanonicalValue, SectionError> {
        C::validate_measurement(measurement)?;
        let section = section.downcast_ref::<T>().ok_or_else(|| {
            SectionError::InvalidBody("erased section has the wrong Rust type".to_owned())
        })?;
        C::encode_body(section)
    }
}

/// Errors produced while constructing an explicit owner-codec registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Two codecs claim the same stable section ID.
    DuplicateSectionId(String),
    /// Two stable IDs claim the same Rust section type.
    DuplicateSectionType(&'static str),
    /// A registry with no codecs cannot validate any serialized owner section.
    EmptyRegistry,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSectionId(id) => write!(formatter, "duplicate section codec ID {id}"),
            Self::DuplicateSectionType(type_name) => {
                write!(formatter, "duplicate section codec type {type_name}")
            }
            Self::EmptyRegistry => formatter.write_str("a profile registry cannot be empty"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Builder for an explicit, safely erased codec registry.
#[derive(Default)]
pub struct ProfileRegistryBuilder {
    codecs: BTreeMap<String, Box<dyn ErasedCodec>>,
}

impl ProfileRegistryBuilder {
    /// Starts an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the one owner codec for `T::ID`.
    ///
    /// # Errors
    ///
    /// Duplicate stable IDs and duplicate Rust section types are rejected even
    /// if the second codec advertises another schema version.
    pub fn register<T, C>(mut self) -> Result<Self, RegistryError>
    where
        T: TuningSection,
        C: SectionCodec<T>,
    {
        let id = T::ID.as_str();
        if self.codecs.contains_key(id) {
            return Err(RegistryError::DuplicateSectionId(id.to_owned()));
        }
        if self
            .codecs
            .values()
            .any(|codec| codec.section_type_id() == TypeId::of::<T>())
        {
            return Err(RegistryError::DuplicateSectionType(type_name::<T>()));
        }
        self.codecs.insert(
            id.to_owned(),
            Box::new(TypedErasedCodec::<T, C>(PhantomData)),
        );
        Ok(self)
    }

    /// Finishes the immutable registry.
    ///
    /// # Errors
    ///
    /// An empty registry is rejected.
    pub fn build(self) -> Result<ProfileRegistry, RegistryError> {
        if self.codecs.is_empty() {
            return Err(RegistryError::EmptyRegistry);
        }
        Ok(ProfileRegistry {
            codecs: self.codecs,
        })
    }
}

/// An explicit immutable set of owner codecs accepted by a caller.
pub struct ProfileRegistry {
    codecs: BTreeMap<String, Box<dyn ErasedCodec>>,
}

impl ProfileRegistry {
    /// Strictly validates and decodes every present format-2 section.
    ///
    /// Registered codecs may be absent. Every present section must have a
    /// registered codec and pass its one supported section version, measurement,
    /// and body validation. No subset or skipped-section mode exists.
    ///
    /// # Errors
    ///
    /// Returns a phase-specific [`ProfileError`] and never produces a partial
    /// prepared value.
    pub fn from_json(&self, text: &str) -> Result<PreparedEnvelope, ProfileError> {
        let canonical: CanonicalValue =
            serde_json::from_str(text).map_err(|_| ProfileError::Malformed)?;
        let envelope: WireEnvelope = serde_json::from_value(canonical.into_json_value())
            .map_err(|_| ProfileError::Malformed)?;

        if envelope.profile_format_version != PROFILE_FORMAT_VERSION {
            return Err(ProfileError::UnsupportedProfileFormatVersion {
                found: envelope.profile_format_version,
                supported: PROFILE_FORMAT_VERSION,
            });
        }
        let profile_id = ProfileId::parse(&envelope.profile_id)?;
        if envelope.assembly.kind != "assembled" {
            return Err(ProfileError::Malformed);
        }

        let expected_digest = content_digest(
            envelope.profile_format_version,
            &envelope.profile_id,
            &envelope.sections,
        )?;
        let found_digest = Sha256::parse(&envelope.assembly.content_sha256)?;
        if found_digest != expected_digest {
            return Err(ProfileError::ContentDigestMismatch {
                found: found_digest.as_str().to_owned(),
                expected: expected_digest.as_str().to_owned(),
            });
        }

        let assembly = AssemblyProvenance {
            assembled_at: Rfc3339Utc::parse(&envelope.assembly.assembled_at)?,
            source_revision: GitRevision::parse(&envelope.assembly.source_revision)?,
            source_dirty: envelope.assembly.source_dirty,
            tool: RepoRelPath::parse(&envelope.assembly.tool)?,
            tool_sha256: Sha256::parse(&envelope.assembly.tool_sha256)?,
        };

        let mut entries = BTreeMap::new();
        for (id, section) in envelope.sections {
            if !valid_owned_section_id(&id) {
                return Err(ProfileError::Malformed);
            }
            let Some(codec) = self.codecs.get(&id) else {
                return Err(ProfileError::UnexpectedSection { id });
            };
            if section.schema_version != codec.schema_version() {
                return Err(ProfileError::UnsupportedSectionSchemaVersion {
                    id,
                    found: section.schema_version,
                    supported: codec.schema_version(),
                });
            }
            let measurement = section.measurement.into_semantic()?;
            let value = codec
                .decode(&measurement, section.selectors)
                .map_err(|source| section_error(id.clone(), source))?;
            entries.insert(
                id,
                ErasedSection {
                    value,
                    type_id: codec.section_type_id(),
                    type_name: codec.section_type_name(),
                    measurement,
                },
            );
        }

        Ok(PreparedEnvelope {
            id: profile_id,
            entries,
            origin: PreparedOrigin::CanonicalEnvelope(VerifiedAssembly {
                provenance: assembly,
                content_sha256: expected_digest,
            }),
        })
    }

    /// Deterministically re-encodes every present typed section.
    ///
    /// The caller supplies assembly action metadata but cannot supply the
    /// content digest. Every entry is type-checked, its owner codec revalidates
    /// measurement evidence, and its body is re-canonicalized.
    ///
    /// # Errors
    ///
    /// Returns an unregistered-section, typed registry-invariant, owner-codec,
    /// or serialization error.
    pub fn to_json(
        &self,
        prepared: &PreparedEnvelope,
        assembly: &AssemblyProvenance,
    ) -> Result<String, ProfileError> {
        let mut sections = BTreeMap::new();
        for (id, entry) in &prepared.entries {
            let Some(codec) = self.codecs.get(id) else {
                return Err(ProfileError::UnregisteredSectionForEncoding { id: id.clone() });
            };
            if codec.section_id().as_str() != id
                || codec.section_type_id() != entry.type_id
                || codec.section_type_name() != entry.type_name
            {
                return Err(ProfileError::RegistryInvariant {
                    id: id.clone(),
                    expected_type: codec.section_type_name(),
                    stored_type: entry.type_name,
                });
            }
            let selectors = codec
                .encode(&entry.measurement, entry.value.as_ref())
                .map_err(|source| section_error(id.clone(), source))?;
            sections.insert(
                id.clone(),
                WireSection {
                    schema_version: codec.schema_version(),
                    measurement: WireMeasurement::from_semantic(&entry.measurement),
                    selectors,
                },
            );
        }

        let digest = content_digest(PROFILE_FORMAT_VERSION, prepared.id.as_str(), &sections)?;
        let envelope = WireEnvelope {
            profile_format_version: PROFILE_FORMAT_VERSION,
            profile_id: prepared.id.as_str().to_owned(),
            assembly: WireAssembly {
                kind: "assembled".to_owned(),
                assembled_at: assembly.assembled_at.as_str().to_owned(),
                source_revision: assembly.source_revision.as_str().to_owned(),
                source_dirty: assembly.source_dirty,
                tool: assembly.tool.as_str().to_owned(),
                tool_sha256: assembly.tool_sha256.as_str().to_owned(),
                content_sha256: digest.as_str().to_owned(),
            },
            sections,
        };
        let mut json = serde_json::to_string(&envelope).map_err(|_| ProfileError::Malformed)?;
        json.push('\n');
        Ok(json)
    }
}

fn section_error(id: String, source: SectionError) -> ProfileError {
    match source {
        SectionError::UnsupportedHarnessSchema { found, supported } => {
            ProfileError::UnsupportedHarnessSchema {
                id,
                found,
                supported,
            }
        }
        source => ProfileError::InvalidSection { id, source },
    }
}

fn content_digest(
    profile_format_version: u32,
    profile_id: &str,
    sections: &BTreeMap<String, WireSection>,
) -> Result<Sha256, ProfileError> {
    #[derive(Serialize)]
    struct Content<'a> {
        profile_format_version: u32,
        profile_id: &'a str,
        sections: &'a BTreeMap<String, WireSection>,
    }

    let bytes = serde_json::to_vec(&Content {
        profile_format_version,
        profile_id,
        sections,
    })
    .map_err(|_| ProfileError::Malformed)?;
    let digest = Sha256Hasher::digest(bytes);
    Ok(Sha256(format!("{digest:x}")))
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireEnvelope {
    profile_format_version: u32,
    profile_id: String,
    assembly: WireAssembly,
    sections: BTreeMap<String, WireSection>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireAssembly {
    kind: String,
    assembled_at: String,
    source_revision: String,
    source_dirty: bool,
    tool: String,
    tool_sha256: String,
    content_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireSection {
    schema_version: u32,
    measurement: WireMeasurement,
    selectors: CanonicalValue,
}

#[allow(clippy::large_enum_variant)]
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum WireMeasurement {
    Inherited {},
    Calibrated {
        measured_at: String,
        source_revision: String,
        source_dirty: bool,
        harness: String,
        harness_schema: String,
        binary_sha256: String,
        toolchain: String,
        host: String,
        cpu_model: String,
        cpu_features: Vec<String>,
        os_kernel: String,
        governor: String,
        receipt: String,
    },
}

impl WireMeasurement {
    fn from_semantic(value: &MeasurementProvenance) -> Self {
        match value {
            MeasurementProvenance::Inherited => Self::Inherited {},
            MeasurementProvenance::Calibrated {
                measured_at,
                source_revision,
                source_dirty,
                harness,
                harness_schema,
                binary_sha256,
                toolchain,
                host,
                cpu_model,
                cpu_features,
                os_kernel,
                governor,
                receipt,
            } => Self::Calibrated {
                measured_at: measured_at.as_str().to_owned(),
                source_revision: source_revision.as_str().to_owned(),
                source_dirty: *source_dirty,
                harness: harness.as_str().to_owned(),
                harness_schema: harness_schema.as_str().to_owned(),
                binary_sha256: binary_sha256.as_str().to_owned(),
                toolchain: toolchain.clone(),
                host: host.clone(),
                cpu_model: cpu_model.clone(),
                cpu_features: cpu_features.clone(),
                os_kernel: os_kernel.clone(),
                governor: governor.clone(),
                receipt: receipt.as_str().to_owned(),
            },
        }
    }

    fn into_semantic(self) -> Result<MeasurementProvenance, ProfileError> {
        match self {
            Self::Inherited {} => Ok(MeasurementProvenance::Inherited),
            Self::Calibrated {
                measured_at,
                source_revision,
                source_dirty,
                harness,
                harness_schema,
                binary_sha256,
                toolchain,
                host,
                cpu_model,
                cpu_features,
                os_kernel,
                governor,
                receipt,
            } => {
                if [
                    toolchain.as_str(),
                    host.as_str(),
                    cpu_model.as_str(),
                    os_kernel.as_str(),
                    governor.as_str(),
                ]
                .into_iter()
                .any(str::is_empty)
                    || cpu_features.iter().any(String::is_empty)
                {
                    return Err(ProfileError::Malformed);
                }
                Ok(MeasurementProvenance::Calibrated {
                    measured_at: Rfc3339Utc::parse(&measured_at)?,
                    source_revision: GitRevision::parse(&source_revision)?,
                    source_dirty,
                    harness: RepoRelPath::parse(&harness)?,
                    harness_schema: HarnessSchema::parse(&harness_schema)?,
                    binary_sha256: Sha256::parse(&binary_sha256)?,
                    toolchain,
                    host,
                    cpu_model,
                    cpu_features,
                    os_kernel,
                    governor,
                    receipt: RepoRelPath::parse(&receipt)?,
                })
            }
        }
    }
}
