//! Generic typed tuning-section storage and the process-wide resolution authority.

use std::any::{type_name, Any, TypeId};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;
use std::panic::Location;
use std::sync::{Arc, OnceLock};

use super::{GitRevision, HarnessSchema, ProfileError, ProfileId, RepoRelPath, Rfc3339Utc, Sha256};

/// The only serialized tuning-envelope format understood by this repository.
pub const PROFILE_FORMAT_VERSION: u32 = 2;

/// A stable, crate-neutral wire identity for one typed tuning section.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SectionId(&'static str);

impl SectionId {
    /// Creates a section ID from a compile-time string.
    ///
    /// The ID must contain exactly two non-empty lowercase kebab-case path
    /// components separated by `/`. Invalid constants panic during constant
    /// evaluation.
    #[must_use]
    pub const fn from_static(id: &'static str) -> Self {
        if !valid_section_id(id) {
            panic!("tuning section ID must contain two lowercase kebab-case components");
        }
        Self(id)
    }

    /// Returns the stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

const fn valid_section_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut index = 0;
    let mut slashes = 0;
    let mut component_len = 0;
    let mut previous_hyphen = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'/' {
            if component_len == 0 || previous_hyphen || slashes != 0 {
                return false;
            }
            slashes += 1;
            component_len = 0;
            previous_hyphen = false;
        } else if byte == b'-' {
            if component_len == 0 || previous_hyphen {
                return false;
            }
            component_len += 1;
            previous_hyphen = true;
        } else if (byte >= b'a' && byte <= b'z') || (byte >= b'0' && byte <= b'9') {
            component_len += 1;
            previous_hyphen = false;
        } else {
            return false;
        }
        index += 1;
    }
    slashes == 1 && component_len != 0 && !previous_hyphen
}

#[cfg(feature = "tuning-profile")]
pub(super) fn valid_owned_section_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut slashes = 0;
    for component in value.split('/') {
        slashes += 1;
        if component.is_empty()
            || component.starts_with('-')
            || component.ends_with('-')
            || component.split('-').any(str::is_empty)
            || !component
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return false;
        }
    }
    slashes == 2
}

/// A crate-owned typed tuning section.
pub trait TuningSection: Clone + Send + Sync + 'static {
    /// Stable wire identity owned by the crate defining this type.
    const ID: SectionId;
    /// Selector view consumed by the owning crate's algorithms.
    type Selectors: Send + Sync + 'static;

    /// Returns the owning crate's conservative static value.
    fn conservative() -> &'static Self;

    /// Returns the section's selector view.
    fn selectors(&self) -> &Self::Selectors;
}

/// Section-local evidence describing whether selector values were measured.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasurementProvenance {
    /// Values are inherited from crate-owned conservative constants.
    Inherited,
    /// Values were produced by a named calibration harness.
    Calibrated {
        /// UTC instant at which measurement began.
        measured_at: Rfc3339Utc,
        /// Git revision from which the harness was built.
        source_revision: GitRevision,
        /// Whether the producing source tree was dirty.
        source_dirty: bool,
        /// Repository-relative harness path.
        harness: RepoRelPath,
        /// Measurement-behavior identity interpreted by the owner codec.
        harness_schema: HarnessSchema,
        /// Digest of the producing executable.
        binary_sha256: Sha256,
        /// Full Rust toolchain description.
        toolchain: String,
        /// Runtime-observed host name.
        host: String,
        /// Runtime-observed CPU model.
        cpu_model: String,
        /// Runtime-observed CPU feature tokens.
        cpu_features: Vec<String>,
        /// Runtime-observed operating-system and kernel description.
        os_kernel: String,
        /// Runtime-observed CPU governor description.
        governor: String,
        /// Repository-relative committed receipt path.
        receipt: RepoRelPath,
    },
}

/// Provenance supplied when typed sections are assembled into an envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssemblyProvenance {
    /// UTC assembly instant.
    pub assembled_at: Rfc3339Utc,
    /// Source revision of the assembler.
    pub source_revision: GitRevision,
    /// Whether the assembler source tree was dirty.
    pub source_dirty: bool,
    /// Repository-relative assembler path.
    pub tool: RepoRelPath,
    /// SHA-256 digest of the assembler source or executable.
    pub tool_sha256: Sha256,
}

/// Assembly metadata retained after a canonical envelope digest is verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedAssembly {
    /// Assembly action metadata.
    pub provenance: AssemblyProvenance,
    /// Verified digest of format, profile identity, and sections.
    pub content_sha256: Sha256,
}

/// Identifies an in-process typed construction without making a measurement claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledProfileProvenance {
    /// Logical configuration or campaign label supplying the compiled values.
    ///
    /// This is not byte identity: compiled values have no serialized envelope
    /// path or content digest.
    pub artifact_id: ProfileId,
}

#[derive(Clone, Debug)]
pub(super) enum PreparedOrigin {
    #[cfg(feature = "tuning-profile")]
    CanonicalEnvelope(VerifiedAssembly),
    Compiled(CompiledProfileProvenance),
}

#[derive(Clone)]
pub(super) struct ErasedSection {
    pub(super) value: Arc<dyn Any + Send + Sync>,
    pub(super) type_id: TypeId,
    pub(super) type_name: &'static str,
    pub(super) measurement: MeasurementProvenance,
}

impl fmt::Debug for ErasedSection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ErasedSection")
            .field("type_name", &self.type_name)
            .field("measurement", &self.measurement)
            .finish_non_exhaustive()
    }
}

/// A completely validated, typed envelope ready for installation or encoding.
#[derive(Clone, Debug)]
pub struct PreparedEnvelope {
    pub(super) id: ProfileId,
    pub(super) entries: BTreeMap<String, ErasedSection>,
    pub(super) origin: PreparedOrigin,
}

impl PreparedEnvelope {
    /// Starts an in-memory envelope that requires no serialization feature.
    #[must_use]
    pub fn compiled(
        id: ProfileId,
        provenance: CompiledProfileProvenance,
    ) -> PreparedEnvelopeBuilder {
        PreparedEnvelopeBuilder {
            id,
            entries: BTreeMap::new(),
            origin: PreparedOrigin::Compiled(provenance),
        }
    }

    /// Returns this envelope's validated profile identity.
    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.id
    }

    /// Iterates the exact set of present stable section IDs in canonical order.
    pub fn section_ids(&self) -> impl ExactSizeIterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Projects one present decoded section and its immutable measurement evidence.
    ///
    /// This is a read-only, pre-install composition operation. It does not
    /// inspect or resolve the process-wide tuning cell. Owner codecs still
    /// revalidate the returned measurement when a composer inserts and encodes
    /// the projected section.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::RegistryInvariant`] if a stored ID and Rust type
    /// disagree. Absence is returned as `Ok(None)`.
    pub fn section<T: TuningSection>(
        &self,
    ) -> Result<Option<PreparedSection<'_, T>>, ProfileError> {
        let Some(entry) = self.entries.get(T::ID.as_str()) else {
            return Ok(None);
        };
        if entry.type_id != TypeId::of::<T>() {
            return Err(ProfileError::RegistryInvariant {
                id: T::ID.as_str().to_owned(),
                expected_type: type_name::<T>(),
                stored_type: entry.type_name,
            });
        }
        let section =
            entry
                .value
                .downcast_ref::<T>()
                .ok_or_else(|| ProfileError::RegistryInvariant {
                    id: T::ID.as_str().to_owned(),
                    expected_type: type_name::<T>(),
                    stored_type: entry.type_name,
                })?;
        Ok(Some(PreparedSection {
            section,
            measurement: &entry.measurement,
        }))
    }

    /// Returns verified canonical assembly metadata, if this value was parsed.
    #[must_use]
    pub fn verified_assembly(&self) -> Option<&VerifiedAssembly> {
        match &self.origin {
            #[cfg(feature = "tuning-profile")]
            PreparedOrigin::CanonicalEnvelope(assembly) => Some(assembly),
            PreparedOrigin::Compiled(_) => None,
        }
    }

    /// Returns compiled construction metadata, if this value was built in memory.
    #[must_use]
    pub fn compiled_provenance(&self) -> Option<&CompiledProfileProvenance> {
        match &self.origin {
            PreparedOrigin::Compiled(provenance) => Some(provenance),
            #[cfg(feature = "tuning-profile")]
            PreparedOrigin::CanonicalEnvelope(_) => None,
        }
    }
}

/// A read-only typed projection from a prepared envelope.
#[derive(Clone, Copy, Debug)]
pub struct PreparedSection<'a, T> {
    /// Decoded owner type.
    pub section: &'a T,
    /// Section-local measurement evidence retained unchanged.
    pub measurement: &'a MeasurementProvenance,
}

/// Builder for an in-memory typed envelope.
pub struct PreparedEnvelopeBuilder {
    pub(super) id: ProfileId,
    pub(super) entries: BTreeMap<String, ErasedSection>,
    pub(super) origin: PreparedOrigin,
}

impl PreparedEnvelopeBuilder {
    /// Inserts one already-typed section with inherited measurement provenance.
    ///
    /// # Errors
    ///
    /// Duplicate stable IDs and duplicate Rust section types are rejected.
    pub fn insert<T: TuningSection>(self, section: T) -> Result<Self, ProfileError> {
        self.insert_erased(section, MeasurementProvenance::Inherited)
    }

    /// Inserts a typed section after its owner codec validates measurement evidence.
    ///
    /// # Errors
    ///
    /// Returns an owner validation error or a duplicate identity error.
    #[cfg(feature = "tuning-profile")]
    pub fn insert_measured<T, C>(
        self,
        section: T,
        measurement: MeasurementProvenance,
    ) -> Result<Self, ProfileError>
    where
        T: TuningSection,
        C: super::SectionCodec<T>,
    {
        C::validate_measurement(&measurement).map_err(|source| ProfileError::InvalidSection {
            id: T::ID.as_str().to_owned(),
            source,
        })?;
        self.insert_erased(section, measurement)
    }

    fn insert_erased<T: TuningSection>(
        mut self,
        section: T,
        measurement: MeasurementProvenance,
    ) -> Result<Self, ProfileError> {
        let id = T::ID.as_str();
        if self
            .entries
            .values()
            .any(|entry| entry.type_id == TypeId::of::<T>())
        {
            return Err(ProfileError::DuplicateSectionType {
                type_name: type_name::<T>(),
            });
        }
        if self.entries.contains_key(id) {
            return Err(ProfileError::DuplicateSectionId { id: id.to_owned() });
        }
        self.entries.insert(
            id.to_owned(),
            ErasedSection {
                value: Arc::new(section),
                type_id: TypeId::of::<T>(),
                type_name: type_name::<T>(),
                measurement,
            },
        );
        Ok(self)
    }

    /// Finishes the prepared value.
    ///
    /// Empty envelopes are valid and make every typed access default missing.
    pub fn build(self) -> Result<PreparedEnvelope, ProfileError> {
        Ok(PreparedEnvelope {
            id: self.id,
            entries: self.entries,
            origin: self.origin,
        })
    }
}

/// Why and where the process-wide tuning authority first resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolutionSite {
    /// Source file captured by `#[track_caller]`.
    pub file: &'static str,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
    /// Operation that resolved the process authority.
    pub cause: ResolutionCause,
}

/// Operation that first resolved the process tuning authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionCause {
    /// A prepared envelope was installed.
    Installed,
    /// A section was accessed before installation.
    FirstAccess {
        /// Section whose access froze the process.
        section_id: SectionId,
    },
}

impl ResolutionSite {
    fn at(location: &'static Location<'static>, cause: ResolutionCause) -> Self {
        Self {
            file: location.file(),
            line: location.line(),
            column: location.column(),
            cause,
        }
    }
}

/// Returned when installation loses the one-shot process resolution race.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlreadyResolved {
    /// Site and cause of the first successful resolution.
    pub first_resolution: ResolutionSite,
}

impl fmt::Display for AlreadyResolved {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let site = self.first_resolution;
        write!(
            formatter,
            "tuning profile already resolved at {}:{}:{}",
            site.file, site.line, site.column
        )
    }
}

impl std::error::Error for AlreadyResolved {}

/// Per-section explanation of the value returned by [`active_section`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionResolution<'a> {
    /// The requested section was present in the installed envelope.
    Installed {
        /// Installed profile identity.
        profile_id: &'a ProfileId,
        /// Requested stable section identity.
        section_id: SectionId,
        /// Section-local measurement evidence.
        measurement: &'a MeasurementProvenance,
    },
    /// An envelope was installed but did not contain the requested section.
    DefaultedMissing {
        /// Installed profile identity.
        profile_id: &'a ProfileId,
        /// Requested stable section identity.
        section_id: SectionId,
    },
    /// Access occurred before any installation and froze the process.
    FrozenBeforeInstall {
        /// Requested stable section identity.
        section_id: SectionId,
        /// Site of the first access that froze the authority.
        first_resolution: ResolutionSite,
    },
}

/// A typed section together with its process resolution provenance.
#[derive(Clone, Copy, Debug)]
pub struct ActiveSection<'a, T> {
    /// Installed or conservative typed value.
    pub section: &'a T,
    /// Explanation of how that value resolved.
    pub resolution: SectionResolution<'a>,
}

impl<T> Deref for ActiveSection<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.section
    }
}

/// Typed invariant panic raised only if installed erased storage is corrupt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveSectionInvariant {
    /// Stable identity involved in the mismatch.
    pub section_id: SectionId,
    /// Rust type requested by the caller.
    pub expected_type: &'static str,
    /// Rust type recorded at preparation time.
    pub stored_type: &'static str,
}

impl fmt::Display for ActiveSectionInvariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "installed section {} expected {}, stored {}",
            self.section_id.as_str(),
            self.expected_type,
            self.stored_type
        )
    }
}

enum ProcessTuning {
    Installed {
        at: ResolutionSite,
        profile_id: ProfileId,
        entries: BTreeMap<String, ErasedSection>,
    },
    FrozenBeforeInstall {
        at: ResolutionSite,
    },
}

impl ProcessTuning {
    fn resolution_site(&self) -> ResolutionSite {
        match self {
            Self::Installed { at, .. } | Self::FrozenBeforeInstall { at } => *at,
        }
    }
}

static PROCESS_TUNING: OnceLock<ProcessTuning> = OnceLock::new();

/// Installs one prepared envelope before any typed access resolves the process.
///
/// # Errors
///
/// Returns [`AlreadyResolved`] carrying the original installation or access
/// site if the process authority has already resolved.
#[track_caller]
pub fn install(prepared: PreparedEnvelope) -> Result<(), AlreadyResolved> {
    let at = ResolutionSite::at(Location::caller(), ResolutionCause::Installed);
    let state = ProcessTuning::Installed {
        at,
        profile_id: prepared.id,
        entries: prepared.entries,
    };
    match PROCESS_TUNING.set(state) {
        Ok(()) => Ok(()),
        Err(_) => Err(AlreadyResolved {
            first_resolution: PROCESS_TUNING
                .get()
                .expect("failed OnceLock set leaves the winning value installed")
                .resolution_site(),
        }),
    }
}

/// Resolves one typed section through the single process-wide authority.
///
/// The first call before installation permanently freezes conservative
/// resolution. A section absent from an installed envelope defaults without
/// consulting which codecs parsed that envelope.
///
/// The installed path performs one immutable `BTreeMap` lookup, so resolution
/// is `O(log s)` for `s` present sections; the frozen path is `O(1)`. After the
/// process resolves, neither path allocates or takes a lock. Resolve once at a
/// public-operation boundary and thread the selector through hot inner loops.
///
/// # Panics
///
/// Panics with [`ActiveSectionInvariant`] only if installed erased storage
/// violates its internal stable-ID/Rust-type invariant. Ordinary malformed
/// serialized input is rejected before installation and cannot cause this
/// panic.
#[must_use]
#[track_caller]
pub fn active_section<T: TuningSection>() -> ActiveSection<'static, T> {
    let caller = Location::caller();
    let process = PROCESS_TUNING.get_or_init(|| ProcessTuning::FrozenBeforeInstall {
        at: ResolutionSite::at(caller, ResolutionCause::FirstAccess { section_id: T::ID }),
    });
    match process {
        ProcessTuning::FrozenBeforeInstall { at } => ActiveSection {
            section: T::conservative(),
            resolution: SectionResolution::FrozenBeforeInstall {
                section_id: T::ID,
                first_resolution: *at,
            },
        },
        ProcessTuning::Installed {
            profile_id,
            entries,
            ..
        } => match entries.get(T::ID.as_str()) {
            None => ActiveSection {
                section: T::conservative(),
                resolution: SectionResolution::DefaultedMissing {
                    profile_id,
                    section_id: T::ID,
                },
            },
            Some(entry) => {
                let section = if entry.type_id == TypeId::of::<T>() {
                    entry.value.downcast_ref::<T>()
                } else {
                    None
                };
                let Some(section) = section else {
                    std::panic::panic_any(ActiveSectionInvariant {
                        section_id: T::ID,
                        expected_type: type_name::<T>(),
                        stored_type: entry.type_name,
                    });
                };
                ActiveSection {
                    section,
                    resolution: SectionResolution::Installed {
                        profile_id,
                        section_id: T::ID,
                        measurement: &entry.measurement,
                    },
                }
            }
        },
    }
}

/// Owner-codec validation error independent of any particular outer envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectionError {
    /// The selector body is malformed or violates an owner invariant.
    InvalidBody(String),
    /// A selector is outside the range accepted by its owner.
    SelectorOutOfRange {
        /// Owner-local selector family.
        family: &'static str,
        /// Owner-local selector field.
        field: &'static str,
        /// Rejected value.
        value: u64,
    },
    /// Calibrated evidence names a harness behavior the owner does not support.
    UnsupportedHarnessSchema {
        /// Harness token found in the evidence.
        found: String,
        /// Harness token supported by this codec.
        supported: &'static str,
    },
}

impl SectionError {
    /// Builds a harness-schema error retaining the unsupported token.
    #[must_use]
    pub fn unsupported_harness_found(found: &HarnessSchema, supported: &'static str) -> Self {
        Self::UnsupportedHarnessSchema {
            found: found.as_str().to_owned(),
            supported,
        }
    }
}

impl fmt::Display for SectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBody(message) => formatter.write_str(message),
            Self::SelectorOutOfRange {
                family,
                field,
                value,
            } => write!(formatter, "{family}.{field} has out-of-range value {value}"),
            Self::UnsupportedHarnessSchema { found, supported } => write!(
                formatter,
                "unsupported harness schema {found:?}; supported token is {supported}"
            ),
        }
    }
}

impl std::error::Error for SectionError {}
