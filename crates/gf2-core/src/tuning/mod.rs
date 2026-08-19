//! Versioned execution-tuning profiles for the `gf2-core` crate.
//!
//! A profile is validated before it can be installed, and the installed value
//! is resolved once for the process. A caller that has profile text can parse
//! and install it before any selection boundary is used:
//!
//! ```
//! # #[cfg(feature = "tuning-profile")]
//! # {
//! use gf2_core::tuning::{self, TuningProfile};
//!
//! let profile = TuningProfile::from_json(r#"
//! {
//!   "schema_version": 1,
//!   "profile_id": "example",
//!   "provenance": {"kind": "inherited"},
//!   "selectors": {"bit_backend": {"simd_min_words": 16}}
//! }
//! "#)
//!     .expect("example profile is valid");
//! tuning::install(profile).expect("profile has not been resolved");
//! assert_eq!(tuning::active().bit_backend().simd_min_words(), 16);
//! # }
//! ```
//!
//! Provenance descriptions remain plain strings and feature names remain
//! `Vec<String>` because they are runtime-observed free text or opaque tokens.

use std::fmt;
use std::sync::OnceLock;

#[cfg(feature = "tuning-profile")]
use serde::{Deserialize, Serialize};

/// The only schema version understood by this loader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaVersion(u32);

impl SchemaVersion {
    /// The supported profile schema version.
    pub const SUPPORTED: u32 = 1;

    /// Parses the supported schema version.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::UnsupportedSchemaVersion`] for every value
    /// other than [`Self::SUPPORTED`].
    pub fn parse(value: u32) -> Result<Self, ProfileError> {
        if value == Self::SUPPORTED {
            Ok(Self(value))
        } else {
            Err(ProfileError::UnsupportedSchemaVersion {
                found: value,
                supported: Self::SUPPORTED,
            })
        }
    }

    #[cfg(feature = "tuning-profile")]
    fn value(self) -> u32 {
        self.0
    }

    /// Returns the numeric schema version.
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// A non-empty kebab-case profile basename: `[a-z0-9]+(-[a-z0-9]+)*`.
#[derive(Clone, Debug)]
pub struct ProfileId(ProfileText);

impl ProfileId {
    const fn from_static(value: &'static str) -> Self {
        Self(ProfileText::Static(value))
    }

    /// Parses a non-empty lowercase kebab-case token.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Malformed`] if `value` is not shaped like
    /// `[a-z0-9]+(-[a-z0-9]+)*`.
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        if is_kebab_case(value) {
            Ok(Self(ProfileText::Owned(value.to_owned())))
        } else {
            Err(ProfileError::Malformed)
        }
    }

    /// Returns the validated profile token.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl PartialEq for ProfileId {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ProfileId {}

impl fmt::Display for ProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A 40-character lowercase hexadecimal Git revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitRevision(String);

impl GitRevision {
    /// Parses exactly 40 lowercase hexadecimal characters.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Malformed`] for any other form.
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        if is_lower_hex(value, 40) {
            Ok(Self(value.to_owned()))
        } else {
            Err(ProfileError::Malformed)
        }
    }

    /// Returns the validated revision.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A 64-character lowercase hexadecimal SHA-256 digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha256(String);

impl Sha256 {
    /// Parses exactly 64 lowercase hexadecimal characters.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Malformed`] for any other form.
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        if is_lower_hex(value, 64) {
            Ok(Self(value.to_owned()))
        } else {
            Err(ProfileError::Malformed)
        }
    }

    /// Returns the validated digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An RFC 3339 UTC instant written with a `Z` suffix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rfc3339Utc(String);

impl Rfc3339Utc {
    /// Parses the structural form `YYYY-MM-DDTHH:MM:SS[.fraction]Z`.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Malformed`] if the UTC timestamp is not in that
    /// RFC 3339 form or contains an out-of-range clock or calendar component.
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        if is_rfc3339_utc(value) {
            Ok(Self(value.to_owned()))
        } else {
            Err(ProfileError::Malformed)
        }
    }

    /// Returns the validated timestamp.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The calibration harness schema token `tuning-calibration-v1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessSchema(String);

impl HarnessSchema {
    /// The schema token emitted by the tuning calibration harness.
    pub const SUPPORTED: &'static str = "tuning-calibration-v1";

    /// Parses the supported calibration harness schema token.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Malformed`] for any other token.
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        if value == Self::SUPPORTED {
            Ok(Self(value.to_owned()))
        } else {
            Err(ProfileError::Malformed)
        }
    }

    /// Returns the validated harness schema token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A non-empty repository-relative path with no `..` component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRelPath(String);

impl RepoRelPath {
    /// Parses a non-empty path that has no leading `/` and no `..` component.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Malformed`] for an absolute, empty, or parent-
    /// traversing path.
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        if !value.is_empty()
            && !value.starts_with('/')
            && !value.split('/').any(|part| part == "..")
        {
            Ok(Self(value.to_owned()))
        } else {
            Err(ProfileError::Malformed)
        }
    }

    /// Returns the validated repository-relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The selector or provenance family named by a validation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileFamily {
    /// The bit-backend selector family.
    BitBackend,
    /// The polynomial selector family.
    Polynomial,
    /// The calibrated provenance record.
    Provenance,
}

impl fmt::Display for ProfileFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BitBackend => "bit_backend",
            Self::Polynomial => "polynomial",
            Self::Provenance => "provenance",
        })
    }
}

/// The closed vocabulary of fields that can be named by validation errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileField {
    /// The SIMD word-count threshold.
    SimdMinWords,
    /// The Karatsuba degree threshold.
    KaratsubaMinDegree,
    /// The Karatsuba output-length ceiling.
    KaratsubaMaxOutLen,
    /// The fast division/remainder length threshold.
    DivRemFastMinLen,
    /// The subproduct-tree length threshold.
    SubproductMinLen,
    /// The number of benchmark executions.
    Executions,
    /// The repetitions per benchmark execution.
    Repetitions,
    /// The benchmark target duration in milliseconds.
    TargetMs,
}

impl fmt::Display for ProfileField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SimdMinWords => "simd_min_words",
            Self::KaratsubaMinDegree => "karatsuba_min_degree",
            Self::KaratsubaMaxOutLen => "karatsuba_max_out_len",
            Self::DivRemFastMinLen => "div_rem_fast_min_len",
            Self::SubproductMinLen => "subproduct_min_len",
            Self::Executions => "executions",
            Self::Repetitions => "repetitions",
            Self::TargetMs => "target_ms",
        })
    }
}

/// Errors returned while parsing or validating a tuning profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// The document uses a schema version this loader does not implement.
    UnsupportedSchemaVersion {
        /// The version found in the document.
        found: u32,
        /// The only version supported by this loader.
        supported: u32,
    },
    /// A selector or calibrated protocol value is outside its admissible range.
    SelectorOutOfRange {
        /// The closed family vocabulary entry containing the invalid field.
        family: ProfileFamily,
        /// The closed field vocabulary entry naming the invalid value.
        field: ProfileField,
        /// The invalid unsigned value.
        value: u64,
    },
    /// The document or one of its semantic values is malformed.
    Malformed,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                formatter,
                "unsupported schema version {found}; supported version is {supported}"
            ),
            Self::SelectorOutOfRange {
                family,
                field,
                value,
            } => write!(formatter, "{family}.{field} has out-of-range value {value}"),
            Self::Malformed => formatter.write_str("malformed tuning profile"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// Returned when a profile is installed after [`active`] has resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlreadyResolved;

impl fmt::Display for AlreadyResolved {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("tuning profile is already resolved")
    }
}

impl std::error::Error for AlreadyResolved {}

/// Provenance attached to a tuning table.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// Values inherited from the source conservative constants.
    Inherited,
    /// Values measured by the calibration harness.
    Calibrated {
        /// UTC instant at which the measurements were taken.
        measured_at: Rfc3339Utc,
        /// Git revision from which the harness binary was built.
        source_revision: GitRevision,
        /// Whether the source tree contained uncommitted changes.
        source_dirty: bool,
        /// Repository-relative calibration harness path.
        harness: RepoRelPath,
        /// Calibration harness schema token.
        harness_schema: HarnessSchema,
        /// SHA-256 digest of the producing benchmark binary.
        binary_sha256: Sha256,
        /// Full Rust toolchain description.
        toolchain: String,
        /// Host name recorded by the harness.
        host: String,
        /// CPU model recorded by the harness.
        cpu_model: String,
        /// Runtime-observed opaque CPU feature tokens.
        cpu_features: Vec<String>,
        /// OS and kernel description.
        os_kernel: String,
        /// CPU frequency governor description.
        governor: String,
        /// Repository-relative lock-wrapper path.
        lock_wrapper: RepoRelPath,
        /// Lock-file path used by the benchmark wrapper.
        lock_file: String,
        /// CPU affinity used by the benchmark wrapper.
        cpu_affinity: String,
        /// Number of benchmark executions; must be at least one.
        executions: u64,
        /// Number of repetitions per execution; must be at least one.
        repetitions: u64,
        /// Target duration in milliseconds; must be at least one.
        target_ms: u64,
        /// Repository-relative committed receipt path.
        receipt: RepoRelPath,
    },
}

/// Selector values for the bit-buffer backend family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitBackendSelectors {
    simd_min_words: usize,
}

impl BitBackendSelectors {
    /// Builds a validated bit-backend selector family.
    ///
    /// Every `usize` word count is admissible, including zero, because zero
    /// means that SIMD is eligible for every buffer when the feature is
    /// available.
    pub fn try_new(simd_min_words: usize) -> Result<Self, ProfileError> {
        Ok(Self { simd_min_words })
    }

    /// Returns the minimum word count for the SIMD backend.
    pub fn simd_min_words(&self) -> usize {
        self.simd_min_words
    }
}

/// Selector values for polynomial algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolynomialSelectors {
    karatsuba_min_degree: usize,
    karatsuba_max_out_len: usize,
    div_rem_fast_min_len: usize,
    subproduct_min_len: usize,
}

impl PolynomialSelectors {
    /// Builds a validated polynomial selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when a bounded threshold
    /// that must be positive is zero. The Karatsuba output-length ceiling may
    /// be zero.
    pub fn try_new(
        karatsuba_min_degree: usize,
        karatsuba_max_out_len: usize,
        div_rem_fast_min_len: usize,
        subproduct_min_len: usize,
    ) -> Result<Self, ProfileError> {
        if karatsuba_min_degree == 0 {
            return Err(out_of_range(
                ProfileFamily::Polynomial,
                ProfileField::KaratsubaMinDegree,
                karatsuba_min_degree as u64,
            ));
        }
        if div_rem_fast_min_len == 0 {
            return Err(out_of_range(
                ProfileFamily::Polynomial,
                ProfileField::DivRemFastMinLen,
                div_rem_fast_min_len as u64,
            ));
        }
        if subproduct_min_len == 0 {
            return Err(out_of_range(
                ProfileFamily::Polynomial,
                ProfileField::SubproductMinLen,
                subproduct_min_len as u64,
            ));
        }
        Ok(Self {
            karatsuba_min_degree,
            karatsuba_max_out_len,
            div_rem_fast_min_len,
            subproduct_min_len,
        })
    }

    /// Returns the minimum operand degree for Karatsuba multiplication.
    pub fn karatsuba_min_degree(&self) -> usize {
        self.karatsuba_min_degree
    }

    /// Returns the maximum product length for the Karatsuba multiplication arm.
    pub fn karatsuba_max_out_len(&self) -> usize {
        self.karatsuba_max_out_len
    }

    /// Returns the minimum length for fast division and remainder.
    pub fn div_rem_fast_min_len(&self) -> usize {
        self.div_rem_fast_min_len
    }

    /// Returns the minimum length for subproduct-tree evaluation.
    pub fn subproduct_min_len(&self) -> usize {
        self.subproduct_min_len
    }
}

/// A validated, versioned table of algorithm-selection thresholds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuningProfile {
    schema_version: SchemaVersion,
    id: ProfileId,
    provenance: Provenance,
    bit_backend: BitBackendSelectors,
    polynomial: PolynomialSelectors,
}

impl TuningProfile {
    /// The parser-free conservative table derived from the selector constants.
    pub const CONSERVATIVE: Self = Self {
        schema_version: SchemaVersion(SchemaVersion::SUPPORTED),
        id: ProfileId::from_static("conservative"),
        provenance: Provenance::Inherited,
        bit_backend: BitBackendSelectors {
            simd_min_words: crate::kernels::backend::SIMD_MIN_WORDS_DEFAULT,
        },
        polynomial: PolynomialSelectors {
            karatsuba_min_degree: crate::field::poly::KARATSUBA_THRESHOLD,
            karatsuba_max_out_len: crate::field::poly::NTT_THRESHOLD,
            div_rem_fast_min_len: crate::field::poly::DIV_REM_THRESHOLD,
            subproduct_min_len: crate::field::poly::SUBPRODUCT_THRESHOLD,
        },
    };

    /// Returns this profile's schema version.
    pub fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns this profile's validated identifier.
    pub fn id(&self) -> &ProfileId {
        &self.id
    }

    /// Returns this profile's provenance record.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Returns the bit-backend selector family.
    pub fn bit_backend(&self) -> &BitBackendSelectors {
        &self.bit_backend
    }

    /// Returns the polynomial selector family.
    pub fn polynomial(&self) -> &PolynomialSelectors {
        &self.polynomial
    }

    /// Builds a profile after validating all selector and calibrated-count ranges.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] for a zero-valued
    /// calibrated protocol count. Selector-family ranges are validated by
    /// their respective constructors before this method receives them.
    pub fn try_new(
        id: ProfileId,
        provenance: Provenance,
        bit_backend: BitBackendSelectors,
        polynomial: PolynomialSelectors,
    ) -> Result<Self, ProfileError> {
        validate_provenance(&provenance)?;
        Ok(Self {
            schema_version: SchemaVersion(SchemaVersion::SUPPORTED),
            id,
            provenance,
            bit_backend,
            polynomial,
        })
    }

    #[cfg(feature = "tuning-profile")]
    /// Parses and validates a caller-supplied JSON profile document.
    ///
    /// Unknown fields, malformed semantic values, unsupported schema versions,
    /// and out-of-range values are rejected as a whole. This method performs no
    /// file or environment access.
    pub fn from_json(text: &str) -> Result<Self, ProfileError> {
        let raw: JsonProfile = serde_json::from_str(text).map_err(|_| ProfileError::Malformed)?;
        let schema_version = SchemaVersion::parse(raw.schema_version)?;
        let id = ProfileId::parse(&raw.profile_id)?;
        let provenance = raw.provenance.into_provenance()?;
        let selectors = match raw.selectors {
            None => JsonSelectors::default(),
            Some(Some(selectors)) => selectors,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let bit_backend = match selectors.bit_backend {
            None => JsonBitBackend::default(),
            Some(Some(bit_backend)) => bit_backend,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let polynomial = match selectors.polynomial {
            None => JsonPolynomial::default(),
            Some(Some(polynomial)) => polynomial,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let bit_backend = BitBackendSelectors::try_new(
            optional(bit_backend.simd_min_words)?
                .unwrap_or(Self::CONSERVATIVE.bit_backend.simd_min_words),
        )?;
        let polynomial = PolynomialSelectors::try_new(
            optional(polynomial.karatsuba_min_degree)?
                .unwrap_or(Self::CONSERVATIVE.polynomial.karatsuba_min_degree),
            optional(polynomial.karatsuba_max_out_len)?
                .unwrap_or(Self::CONSERVATIVE.polynomial.karatsuba_max_out_len),
            optional(polynomial.div_rem_fast_min_len)?
                .unwrap_or(Self::CONSERVATIVE.polynomial.div_rem_fast_min_len),
            optional(polynomial.subproduct_min_len)?
                .unwrap_or(Self::CONSERVATIVE.polynomial.subproduct_min_len),
        )?;
        Self::try_new(id, provenance, bit_backend, polynomial).map(|mut profile| {
            profile.schema_version = schema_version;
            profile
        })
    }

    #[cfg(feature = "tuning-profile")]
    /// Serializes this validated profile as a JSON document.
    ///
    /// The operation does not access the filesystem or environment and cannot
    /// fail for a value constructed by this module.
    pub fn to_json(&self) -> String {
        let output = JsonProfileOut {
            schema_version: self.schema_version.value(),
            profile_id: self.id.as_str().to_owned(),
            provenance: JsonProvenance::from(&self.provenance),
            selectors: JsonSelectorsOut {
                bit_backend: JsonBitBackendOut {
                    simd_min_words: self.bit_backend.simd_min_words,
                },
                polynomial: JsonPolynomialOut {
                    karatsuba_min_degree: self.polynomial.karatsuba_min_degree,
                    karatsuba_max_out_len: self.polynomial.karatsuba_max_out_len,
                    div_rem_fast_min_len: self.polynomial.div_rem_fast_min_len,
                    subproduct_min_len: self.polynomial.subproduct_min_len,
                },
            },
        };
        serde_json::to_string(&output).expect("validated tuning profile is serializable")
    }
}

static ACTIVE: OnceLock<TuningProfile> = OnceLock::new();

/// Returns the process-wide profile, resolving to [`TuningProfile::CONSERVATIVE`]
/// when no caller installs one first.
pub fn active() -> &'static TuningProfile {
    ACTIVE.get_or_init(|| TuningProfile::CONSERVATIVE.clone())
}

/// Installs the profile before [`active`] resolves.
///
/// # Errors
///
/// Returns [`AlreadyResolved`] if this process has already called [`active`] or
/// successfully installed another profile.
pub fn install(profile: TuningProfile) -> Result<(), AlreadyResolved> {
    ACTIVE.set(profile).map_err(|_| AlreadyResolved)
}

#[derive(Clone, Debug)]
enum ProfileText {
    Static(&'static str),
    Owned(String),
}

impl ProfileText {
    fn as_str(&self) -> &str {
        match self {
            Self::Static(value) => value,
            Self::Owned(value) => value,
        }
    }
}

fn is_kebab_case(value: &str) -> bool {
    if value.is_empty() || value.starts_with('-') || value.ends_with('-') {
        return false;
    }
    let mut previous_hyphen = false;
    for byte in value.bytes() {
        if byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_hyphen = false;
        } else {
            return false;
        }
    }
    true
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_rfc3339_utc(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || *bytes.last().unwrap_or(&0) != b'Z'
    {
        return false;
    }
    if !bytes[..4]
        .iter()
        .chain(bytes[5..7].iter())
        .chain(bytes[8..10].iter())
        .chain(bytes[11..13].iter())
        .chain(bytes[14..16].iter())
        .chain(bytes[17..19].iter())
        .all(u8::is_ascii_digit)
    {
        return false;
    }
    if bytes.len() > 20
        && (bytes[19] != b'.'
            || bytes[20..bytes.len() - 1].is_empty()
            || !bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let number = |start: usize, end: usize| {
        value.as_bytes()[start..end]
            .iter()
            .fold(0_u32, |total, byte| total * 10 + u32::from(byte - b'0'))
    };
    let month = number(5, 7);
    let day = number(8, 10);
    let year = number(0, 4);
    let hour = number(11, 13);
    let minute = number(14, 16);
    let second = number(17, 19);
    let days_in_month = match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=12).contains(&month)
        && (1..=days_in_month).contains(&day)
        && hour <= 23
        && minute <= 59
        && second <= 60
}

fn validate_provenance(provenance: &Provenance) -> Result<(), ProfileError> {
    if let Provenance::Calibrated {
        executions,
        repetitions,
        target_ms,
        ..
    } = provenance
    {
        for (field, value) in [
            (ProfileField::Executions, *executions),
            (ProfileField::Repetitions, *repetitions),
            (ProfileField::TargetMs, *target_ms),
        ] {
            if value == 0 {
                return Err(out_of_range(ProfileFamily::Provenance, field, value));
            }
        }
    }
    Ok(())
}

fn out_of_range(family: ProfileFamily, field: ProfileField, value: impl Into<u64>) -> ProfileError {
    ProfileError::SelectorOutOfRange {
        family,
        field,
        value: value.into(),
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonProfile {
    schema_version: u32,
    profile_id: String,
    provenance: JsonProvenance,
    selectors: Option<Option<JsonSelectors>>,
}

#[cfg(feature = "tuning-profile")]
#[allow(clippy::large_enum_variant)]
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum JsonProvenance {
    Inherited {},
    Calibrated {
        measured_at: Option<String>,
        source_revision: Option<String>,
        source_dirty: Option<bool>,
        harness: Option<String>,
        harness_schema: Option<String>,
        binary_sha256: Option<String>,
        toolchain: Option<String>,
        host: Option<String>,
        cpu_model: Option<String>,
        cpu_features: Option<Vec<String>>,
        os_kernel: Option<String>,
        governor: Option<String>,
        lock_wrapper: Option<String>,
        lock_file: Option<String>,
        cpu_affinity: Option<String>,
        executions: Option<u64>,
        repetitions: Option<u64>,
        target_ms: Option<u64>,
        receipt: Option<String>,
    },
}

#[cfg(feature = "tuning-profile")]
impl JsonProvenance {
    fn into_provenance(self) -> Result<Provenance, ProfileError> {
        match self {
            Self::Inherited {} => Ok(Provenance::Inherited),
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
                lock_wrapper,
                lock_file,
                cpu_affinity,
                executions,
                repetitions,
                target_ms,
                receipt,
            } => {
                let provenance = Provenance::Calibrated {
                    measured_at: Rfc3339Utc::parse(&required(measured_at)?)?,
                    source_revision: GitRevision::parse(&required(source_revision)?)?,
                    source_dirty: required(source_dirty)?,
                    harness: RepoRelPath::parse(&required(harness)?)?,
                    harness_schema: HarnessSchema::parse(&required(harness_schema)?)?,
                    binary_sha256: Sha256::parse(&required(binary_sha256)?)?,
                    toolchain: required(toolchain)?,
                    host: required(host)?,
                    cpu_model: required(cpu_model)?,
                    cpu_features: required(cpu_features)?,
                    os_kernel: required(os_kernel)?,
                    governor: required(governor)?,
                    lock_wrapper: RepoRelPath::parse(&required(lock_wrapper)?)?,
                    lock_file: required(lock_file)?,
                    cpu_affinity: required(cpu_affinity)?,
                    executions: required(executions)?,
                    repetitions: required(repetitions)?,
                    target_ms: required(target_ms)?,
                    receipt: RepoRelPath::parse(&required(receipt)?)?,
                };
                validate_provenance(&provenance)?;
                Ok(provenance)
            }
        }
    }
}

#[cfg(feature = "tuning-profile")]
fn required<T>(value: Option<T>) -> Result<T, ProfileError> {
    value.ok_or(ProfileError::Malformed)
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonSelectors {
    bit_backend: Option<Option<JsonBitBackend>>,
    polynomial: Option<Option<JsonPolynomial>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonBitBackend {
    simd_min_words: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPolynomial {
    karatsuba_min_degree: Option<Option<usize>>,
    karatsuba_max_out_len: Option<Option<usize>>,
    div_rem_fast_min_len: Option<Option<usize>>,
    subproduct_min_len: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
fn optional<T>(value: Option<Option<T>>) -> Result<Option<T>, ProfileError> {
    match value {
        None => Ok(None),
        Some(Some(value)) => Ok(Some(value)),
        Some(None) => Err(ProfileError::Malformed),
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonProfileOut {
    schema_version: u32,
    profile_id: String,
    provenance: JsonProvenance,
    selectors: JsonSelectorsOut,
}

#[cfg(feature = "tuning-profile")]
impl From<&Provenance> for JsonProvenance {
    fn from(provenance: &Provenance) -> Self {
        match provenance {
            Provenance::Inherited => Self::Inherited {},
            Provenance::Calibrated {
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
                lock_wrapper,
                lock_file,
                cpu_affinity,
                executions,
                repetitions,
                target_ms,
                receipt,
            } => Self::Calibrated {
                measured_at: Some(measured_at.as_str().to_owned()),
                source_revision: Some(source_revision.as_str().to_owned()),
                source_dirty: Some(*source_dirty),
                harness: Some(harness.as_str().to_owned()),
                harness_schema: Some(harness_schema.as_str().to_owned()),
                binary_sha256: Some(binary_sha256.as_str().to_owned()),
                toolchain: Some(toolchain.clone()),
                host: Some(host.clone()),
                cpu_model: Some(cpu_model.clone()),
                cpu_features: Some(cpu_features.clone()),
                os_kernel: Some(os_kernel.clone()),
                governor: Some(governor.clone()),
                lock_wrapper: Some(lock_wrapper.as_str().to_owned()),
                lock_file: Some(lock_file.clone()),
                cpu_affinity: Some(cpu_affinity.clone()),
                executions: Some(*executions),
                repetitions: Some(*repetitions),
                target_ms: Some(*target_ms),
                receipt: Some(receipt.as_str().to_owned()),
            },
        }
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonSelectorsOut {
    bit_backend: JsonBitBackendOut,
    polynomial: JsonPolynomialOut,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonBitBackendOut {
    simd_min_words: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonPolynomialOut {
    karatsuba_min_degree: usize,
    karatsuba_max_out_len: usize,
    div_rem_fast_min_len: usize,
    subproduct_min_len: usize,
}

#[cfg(all(test, feature = "tuning-profile"))]
mod tests {
    use super::*;

    const COMMITTED: &str = include_str!("../../data/tuning-profiles/conservative.json");

    fn inherited_document(selectors: &str) -> String {
        format!(
            r#"{{"schema_version":1,"profile_id":"example","provenance":{{"kind":"inherited"}},"selectors":{selectors}}}"#
        )
    }

    fn calibrated_document() -> &'static str {
        r#"{
          "schema_version": 1,
          "profile_id": "example-host-avx2",
          "provenance": {
            "kind": "calibrated",
            "measured_at": "2026-08-19T12:34:56.123Z",
            "source_revision": "0123456789abcdef0123456789abcdef01234567",
            "source_dirty": false,
            "harness": "crates/gf2-core/benches/tuning_calibration.rs",
            "harness_schema": "tuning-calibration-v1",
            "binary_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "toolchain": "rustc 1.95.0",
            "host": "host",
            "cpu_model": "cpu",
            "cpu_features": ["avx2", "pclmulqdq"],
            "os_kernel": "Linux 6",
            "governor": "performance",
            "lock_wrapper": "dev/scripts/ccx1-bench-flock.sh",
            "lock_file": "/tmp/gf2-ccx1.lock",
            "cpu_affinity": "6-11",
            "executions": 5,
            "repetitions": 5,
            "target_ms": 250,
            "receipt": "dev/benchmarks/tuning_profiles/example.md"
          },
          "selectors": {
            "bit_backend": {"simd_min_words": 16},
            "polynomial": {
              "karatsuba_min_degree": 33,
              "karatsuba_max_out_len": 129,
              "div_rem_fast_min_len": 2049,
              "subproduct_min_len": 4097
            }
          }
        }"#
    }

    #[test]
    fn committed_conservative_profile_matches_constant() {
        assert_eq!(
            TuningProfile::from_json(COMMITTED).unwrap(),
            TuningProfile::CONSERVATIVE
        );
    }

    #[test]
    fn calibrated_profile_round_trips() {
        let profile = TuningProfile::from_json(calibrated_document()).unwrap();
        assert_eq!(
            TuningProfile::from_json(&profile.to_json()).unwrap(),
            profile
        );
    }

    #[test]
    fn rfc3339_utc_validates_gregorian_calendar_dates() {
        for (value, valid) in [
            ("2026-02-31T00:00:00Z", false),
            ("2026-02-29T00:00:00Z", false),
            ("2024-02-29T00:00:00Z", true),
            ("2000-02-29T00:00:00Z", true),
            ("1900-02-29T00:00:00Z", false),
            ("2026-04-31T00:00:00Z", false),
            ("2026-04-30T00:00:00Z", true),
            ("2026-13-01T00:00:00Z", false),
            ("2026-01-00T00:00:00Z", false),
        ] {
            assert_eq!(Rfc3339Utc::parse(value).is_ok(), valid, "{value}");
        }

        let malformed =
            calibrated_document().replace("2026-08-19T12:34:56.123Z", "2026-02-31T00:00:00Z");
        assert_eq!(
            TuningProfile::from_json(&malformed).unwrap_err(),
            ProfileError::Malformed
        );
    }

    #[test]
    fn malformed_json_is_rejected() {
        assert_eq!(
            TuningProfile::from_json("{").unwrap_err(),
            ProfileError::Malformed
        );
    }

    #[test]
    fn unsupported_schema_versions_are_explicit() {
        for found in [0, 2] {
            let text = inherited_document("{}").replace(
                "\"schema_version\":1",
                &format!("\"schema_version\":{found}"),
            );
            assert_eq!(
                TuningProfile::from_json(&text).unwrap_err(),
                ProfileError::UnsupportedSchemaVersion {
                    found,
                    supported: 1
                }
            );
        }
    }

    #[test]
    fn unknown_keys_are_rejected_at_each_level() {
        let cases = [
            r#"{"schema_version":1,"profile_id":"example","provenance":{"kind":"inherited"},"selectors":{},"unknown":0}"#.to_owned(),
            inherited_document("{}").replace(
                "\"provenance\":{\"kind\":\"inherited\"}",
                "\"provenance\":{\"kind\":\"inherited\",\"unknown\":0}",
            ),
            inherited_document(r#"{"unknown":{}}"#),
            inherited_document(r#"{"bit_backend":{"unknown":0}}"#),
        ];
        for text in cases {
            assert!(TuningProfile::from_json(&text).is_err(), "{text}");
        }
    }

    #[test]
    fn inherited_provenance_rejects_calibrated_fields() {
        let text = inherited_document("{}").replace(
            "\"provenance\":{\"kind\":\"inherited\"}",
            "\"provenance\":{\"kind\":\"inherited\",\"executions\":1}",
        );
        assert_eq!(
            TuningProfile::from_json(&text).unwrap_err(),
            ProfileError::Malformed
        );
    }

    #[test]
    fn bounded_fields_report_their_family_and_field() {
        for (field, value) in [
            (ProfileField::KaratsubaMinDegree, "karatsuba_min_degree"),
            (ProfileField::DivRemFastMinLen, "div_rem_fast_min_len"),
            (ProfileField::SubproductMinLen, "subproduct_min_len"),
        ] {
            let text = inherited_document(&format!(r#"{{"polynomial":{{"{value}":0}}}}"#));
            assert_eq!(
                TuningProfile::from_json(&text).unwrap_err(),
                ProfileError::SelectorOutOfRange {
                    family: ProfileFamily::Polynomial,
                    field,
                    value: 0
                }
            );
        }
        let text = calibrated_document().replace("\"executions\": 5", "\"executions\": 0");
        assert_eq!(
            TuningProfile::from_json(&text).unwrap_err(),
            ProfileError::SelectorOutOfRange {
                family: ProfileFamily::Provenance,
                field: ProfileField::Executions,
                value: 0
            }
        );
    }

    #[test]
    fn endpoint_values_are_admissible() {
        let max = usize::MAX.to_string();
        let text = inherited_document(&format!(
            r#"{{"bit_backend":{{"simd_min_words":0}},"polynomial":{{"karatsuba_min_degree":{max},"karatsuba_max_out_len":0,"div_rem_fast_min_len":{max},"subproduct_min_len":{max}}}}}"#
        ));
        let profile = TuningProfile::from_json(&text).unwrap();
        assert_eq!(profile.bit_backend().simd_min_words(), 0);
        assert_eq!(profile.polynomial().karatsuba_max_out_len(), 0);
        assert_eq!(profile.polynomial().karatsuba_min_degree(), usize::MAX);
    }

    #[test]
    fn absent_selectors_families_and_fields_inherit_defaults() {
        let absent = TuningProfile::from_json(
            r#"{"schema_version":1,"profile_id":"example","provenance":{"kind":"inherited"}}"#,
        )
        .unwrap();
        assert_eq!(
            absent.bit_backend(),
            TuningProfile::CONSERVATIVE.bit_backend()
        );
        assert_eq!(
            absent.polynomial(),
            TuningProfile::CONSERVATIVE.polynomial()
        );
        let partial = TuningProfile::from_json(&inherited_document(
            r#"{"bit_backend":{"simd_min_words":16}}"#,
        ))
        .unwrap();
        assert_eq!(partial.bit_backend().simd_min_words(), 16);
        assert_eq!(
            partial.polynomial(),
            TuningProfile::CONSERVATIVE.polynomial()
        );
        let field = TuningProfile::from_json(&inherited_document(
            r#"{"polynomial":{"karatsuba_max_out_len":0}}"#,
        ))
        .unwrap();
        assert_eq!(field.polynomial().karatsuba_max_out_len(), 0);
        assert_eq!(
            field.polynomial().karatsuba_min_degree(),
            TuningProfile::CONSERVATIVE
                .polynomial()
                .karatsuba_min_degree()
        );
    }

    #[test]
    fn semantic_types_reject_ill_formed_inputs() {
        assert!(ProfileId::parse("Bad").is_err());
        assert!(ProfileId::parse("-leading").is_err());
        assert!(GitRevision::parse(&"a".repeat(39)).is_err());
        assert!(Sha256::parse(&format!("{}g", "a".repeat(63))).is_err());
        assert!(RepoRelPath::parse("/absolute").is_err());
        assert!(RepoRelPath::parse("dev/../receipt").is_err());
    }

    #[test]
    fn conservative_values_name_the_source_constants() {
        assert_eq!(
            TuningProfile::CONSERVATIVE.bit_backend().simd_min_words(),
            crate::kernels::backend::SIMD_MIN_WORDS_DEFAULT
        );
        assert_eq!(
            TuningProfile::CONSERVATIVE
                .polynomial()
                .karatsuba_min_degree(),
            crate::field::poly::KARATSUBA_THRESHOLD
        );
        assert_eq!(
            TuningProfile::CONSERVATIVE
                .polynomial()
                .karatsuba_max_out_len(),
            crate::field::poly::NTT_THRESHOLD
        );
        assert_eq!(
            TuningProfile::CONSERVATIVE
                .polynomial()
                .div_rem_fast_min_len(),
            crate::field::poly::DIV_REM_THRESHOLD
        );
        assert_eq!(
            TuningProfile::CONSERVATIVE
                .polynomial()
                .subproduct_min_len(),
            crate::field::poly::SUBPRODUCT_THRESHOLD
        );
    }
}
