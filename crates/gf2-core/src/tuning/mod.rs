//! Versioned execution-tuning profiles for the `gf2-core` crate.
//!
//! A profile is validated before it can be installed, and the installed value
//! is fixed once for the process. A caller that has profile text can parse and
//! install it before profile-governed selection boundaries are used. The
//! bit-backend boundary is compile-time selected and its runtime profile field
//! is retained for schema compatibility but does not govern routing; see
//! `dev/active/220cab0b/design.md` (DEC-G).
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

pub(crate) mod baked;

#[cfg(feature = "tuning-profile")]
use serde::{Deserialize, Serialize};

/// Conservative default for the `permanent.gray_chunk_subsets` field.
///
/// Every other conservative default is defined at its selector's own module,
/// which `dev/active/220cab0b/design.md` D3 fixes as the convention. This
/// field's selector lives in `gf2-algebra`, whose dependency on `gf2-core`
/// points inward, so the value's single definition site is here and the
/// selector's own `CHUNK_SUBSETS` names it; defining it at the selector would
/// need a reverse edge, which `@/inv/crate-dependency-direction` forbids.
pub const PERMANENT_GRAY_CHUNK_SUBSETS_DEFAULT: usize = 1 << 16;

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
    /// The bit-matrix selector family.
    BitMatrix,
    /// The structure-of-arrays batch selector family.
    SoaBatch,
    /// The M4RM selector family.
    M4rm,
    /// The dense-inverse selector family.
    DenseInverse,
    /// The triangular-solve selector family.
    Triangular,
    /// The PLE selector family.
    Ple,
    /// The GEMM selector family.
    Gemm,
    /// The field-vector selector family.
    FieldVec,
    /// The characteristic-polynomial selector family.
    Charpoly,
    /// The polynomial selector family.
    Polynomial,
    /// The prime-route selector family.
    PrimeRoute,
    /// The permanent selector family.
    Permanent,
    /// The calibrated provenance record.
    Provenance,
}

impl fmt::Display for ProfileFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BitBackend => "bit_backend",
            Self::BitMatrix => "bit_matrix",
            Self::SoaBatch => "soa_batch",
            Self::M4rm => "m4rm",
            Self::DenseInverse => "dense_inverse",
            Self::Triangular => "triangular",
            Self::Ple => "ple",
            Self::Gemm => "gemm",
            Self::FieldVec => "field_vec",
            Self::Charpoly => "charpoly",
            Self::Polynomial => "polynomial",
            Self::PrimeRoute => "prime_route",
            Self::Permanent => "permanent",
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
    /// The fast interpolation point-count threshold.
    InterpolateFastMinPoints,
    /// The matrix-vector SIMD word-count threshold.
    MatvecSimdMinWords,
    /// The simple-transpose block-count ceiling.
    TransposeSimpleMaxBlocks,
    /// The transpose macro-tile block extent.
    TransposeMacroTileBlocks,
    /// The structure-of-arrays parallelization length threshold.
    ParallelMinLen,
    /// The structure-of-arrays parallel chunk length.
    ParallelChunkLen,
    /// The M4RM wide-tier stride threshold.
    WideTierMinStrideWords,
    /// The M4RM tiled-schedule stride threshold.
    TiledMinStrideWords,
    /// The default M4RM table byte budget.
    DefaultTableBytes,
    /// The middle M4RM table byte budget.
    MidTableBytes,
    /// The wide M4RM table byte budget.
    WideTableBytes,
    /// The wide M4RM panel-width ceiling.
    WideMaxK,
    /// The small-N M4RM panel-width ceiling.
    SmallNMaxK,
    /// The M4RI inversion dimension threshold.
    M4riMinDim,
    /// The blocked inversion dimension threshold.
    BlockedMinDim,
    /// The blocked triangular-solve dimension threshold.
    TrsmBlockedMinDim,
    /// The blocked triangular-solve panel-row extent.
    TrsmPanelRows,
    /// The minimum dimension for the Winograd GEMM path.
    WinogradMinDim,
    /// The maximum dimension for triangular base cases.
    BaseCaseMaxDim,
    /// The recursive PLE panel-width ceiling.
    PanelBaseMaxCols,
    /// The maximum width for the scalar PLE base case.
    ScalarBaseMaxCols,
    /// The maximum byte-lane PLE panel width: the conservative default is
    /// the tuned L1d blocking factor, and the admissible maximum is the
    /// kernel's asserted structural scratch bound.
    PanelByteLaneMaxCols,
    /// The maximum u16-lane PLE panel width: the conservative default is the
    /// tuned L1d blocking factor, and the admissible maximum is the kernel's
    /// asserted structural scratch bound.
    PanelU16LaneMaxCols,
    /// The blocked PLE back-substitution dimension threshold.
    BlockedBackSubMinDim,
    /// The GEMM row-tile extent.
    RowTile,
    /// The GEMM column-tile extent.
    ColTile,
    /// The GEMM AXPY fast-path volume threshold.
    AxpyFastPathMinVolume,
    /// The field-vector dot-product chunk extent.
    DotChunkLen,
    /// The Keller-Gehrig dimension threshold.
    KellerGehrigMinDim,
    /// The F32 prime-route threshold.
    F32MinPrime,
    /// The F32 column-count threshold.
    F32MinCols,
    /// The F64 column-count threshold.
    F64MinCols,
    /// The permanent Gray-code chunk subset extent.
    GrayChunkSubsets,
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
            Self::InterpolateFastMinPoints => "interpolate_fast_min_points",
            Self::MatvecSimdMinWords => "matvec_simd_min_words",
            Self::TransposeSimpleMaxBlocks => "transpose_simple_max_blocks",
            Self::TransposeMacroTileBlocks => "transpose_macro_tile_blocks",
            Self::ParallelMinLen => "parallel_min_len",
            Self::ParallelChunkLen => "parallel_chunk_len",
            Self::WideTierMinStrideWords => "wide_tier_min_stride_words",
            Self::TiledMinStrideWords => "tiled_min_stride_words",
            Self::DefaultTableBytes => "default_table_bytes",
            Self::MidTableBytes => "mid_table_bytes",
            Self::WideTableBytes => "wide_table_bytes",
            Self::WideMaxK => "wide_max_k",
            Self::SmallNMaxK => "small_n_max_k",
            Self::M4riMinDim => "m4ri_min_dim",
            Self::BlockedMinDim => "blocked_min_dim",
            Self::TrsmBlockedMinDim => "trsm_blocked_min_dim",
            Self::TrsmPanelRows => "trsm_panel_rows",
            Self::WinogradMinDim => "winograd_min_dim",
            Self::BaseCaseMaxDim => "base_case_max_dim",
            Self::PanelBaseMaxCols => "panel_base_max_cols",
            Self::ScalarBaseMaxCols => "scalar_base_max_cols",
            Self::PanelByteLaneMaxCols => "panel_byte_lane_max_cols",
            Self::PanelU16LaneMaxCols => "panel_u16_lane_max_cols",
            Self::BlockedBackSubMinDim => "blocked_back_sub_min_dim",
            Self::RowTile => "row_tile",
            Self::ColTile => "col_tile",
            Self::AxpyFastPathMinVolume => "axpy_fast_path_min_volume",
            Self::DotChunkLen => "dot_chunk_len",
            Self::KellerGehrigMinDim => "keller_gehrig_min_dim",
            Self::F32MinPrime => "f32_min_prime",
            Self::F32MinCols => "f32_min_cols",
            Self::F64MinCols => "f64_min_cols",
            Self::GrayChunkSubsets => "gray_chunk_subsets",
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

    /// Returns the profile's minimum word count for the SIMD backend.
    ///
    /// The field remains part of the runtime profile schema and is observable
    /// through [`TuningProfile`] and [`active`]. The bit-backend routing
    /// boundary uses a compile-time constant instead, per DEC-G in
    /// `dev/active/220cab0b/design.md`.
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
    interpolate_fast_min_points: usize,
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
        interpolate_fast_min_points: usize,
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
        if interpolate_fast_min_points == 0 {
            return Err(out_of_range(
                ProfileFamily::Polynomial,
                ProfileField::InterpolateFastMinPoints,
                interpolate_fast_min_points as u64,
            ));
        }
        Ok(Self {
            karatsuba_min_degree,
            karatsuba_max_out_len,
            div_rem_fast_min_len,
            subproduct_min_len,
            interpolate_fast_min_points,
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

    /// Returns the minimum point count for fast interpolation.
    pub fn interpolate_fast_min_points(&self) -> usize {
        self.interpolate_fast_min_points
    }
}

/// Selector values for bit-matrix algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitMatrixSelectors {
    matvec_simd_min_words: usize,
    transpose_simple_max_blocks: usize,
    transpose_macro_tile_blocks: usize,
}

impl BitMatrixSelectors {
    /// Builds a validated bit-matrix selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when
    /// `transpose_macro_tile_blocks` is zero because the macro-tile walk must
    /// advance by at least one block.
    pub fn try_new(
        matvec_simd_min_words: usize,
        transpose_simple_max_blocks: usize,
        transpose_macro_tile_blocks: usize,
    ) -> Result<Self, ProfileError> {
        if transpose_macro_tile_blocks == 0 {
            return Err(out_of_range(
                ProfileFamily::BitMatrix,
                ProfileField::TransposeMacroTileBlocks,
                transpose_macro_tile_blocks as u64,
            ));
        }
        Ok(Self {
            matvec_simd_min_words,
            transpose_simple_max_blocks,
            transpose_macro_tile_blocks,
        })
    }

    /// Returns the minimum word count for matrix-vector SIMD.
    pub fn matvec_simd_min_words(&self) -> usize {
        self.matvec_simd_min_words
    }

    /// Returns the maximum block count for simple transpose.
    pub fn transpose_simple_max_blocks(&self) -> usize {
        self.transpose_simple_max_blocks
    }

    /// Returns the macro-tile block extent for transpose.
    pub fn transpose_macro_tile_blocks(&self) -> usize {
        self.transpose_macro_tile_blocks
    }
}

/// Selector values for structure-of-arrays batch algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoaBatchSelectors {
    parallel_min_len: usize,
    parallel_chunk_len: usize,
}

impl SoaBatchSelectors {
    /// Builds a validated structure-of-arrays batch selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when
    /// `parallel_chunk_len` is zero because parallel chunks must advance.
    pub fn try_new(
        parallel_min_len: usize,
        parallel_chunk_len: usize,
    ) -> Result<Self, ProfileError> {
        if parallel_chunk_len == 0 {
            return Err(out_of_range(
                ProfileFamily::SoaBatch,
                ProfileField::ParallelChunkLen,
                parallel_chunk_len as u64,
            ));
        }
        Ok(Self {
            parallel_min_len,
            parallel_chunk_len,
        })
    }

    /// Returns the minimum batch length for parallel execution.
    pub fn parallel_min_len(&self) -> usize {
        self.parallel_min_len
    }

    /// Returns the parallel batch chunk length.
    pub fn parallel_chunk_len(&self) -> usize {
        self.parallel_chunk_len
    }
}

/// Selector values for M4RM algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct M4rmSelectors {
    wide_tier_min_stride_words: usize,
    tiled_min_stride_words: usize,
    default_table_bytes: usize,
    mid_table_bytes: usize,
    wide_table_bytes: usize,
    wide_max_k: usize,
    small_n_max_k: usize,
}

impl M4rmSelectors {
    /// Builds a validated M4RM selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when
    /// `tiled_min_stride_words` is below `M4RM_TILE_WORDS`, when `wide_max_k`
    /// is zero, or when `small_n_max_k` is below two. The bounds preserve a
    /// complete tile and valid panel widths.
    pub fn try_new(
        wide_tier_min_stride_words: usize,
        tiled_min_stride_words: usize,
        default_table_bytes: usize,
        mid_table_bytes: usize,
        wide_table_bytes: usize,
        wide_max_k: usize,
        small_n_max_k: usize,
    ) -> Result<Self, ProfileError> {
        if tiled_min_stride_words < crate::alg::m4rm::M4RM_TILE_WORDS {
            return Err(out_of_range(
                ProfileFamily::M4rm,
                ProfileField::TiledMinStrideWords,
                tiled_min_stride_words as u64,
            ));
        }
        if wide_max_k == 0 {
            return Err(out_of_range(
                ProfileFamily::M4rm,
                ProfileField::WideMaxK,
                wide_max_k as u64,
            ));
        }
        if small_n_max_k < 2 {
            return Err(out_of_range(
                ProfileFamily::M4rm,
                ProfileField::SmallNMaxK,
                small_n_max_k as u64,
            ));
        }
        Ok(Self {
            wide_tier_min_stride_words,
            tiled_min_stride_words,
            default_table_bytes,
            mid_table_bytes,
            wide_table_bytes,
            wide_max_k,
            small_n_max_k,
        })
    }

    /// Returns the minimum stride for the wide tier.
    pub fn wide_tier_min_stride_words(&self) -> usize {
        self.wide_tier_min_stride_words
    }

    /// Returns the minimum stride for tiled M4RM updates.
    pub fn tiled_min_stride_words(&self) -> usize {
        self.tiled_min_stride_words
    }

    /// Returns the default M4RM table byte budget.
    pub fn default_table_bytes(&self) -> usize {
        self.default_table_bytes
    }

    /// Returns the middle-tier M4RM table byte budget.
    pub fn mid_table_bytes(&self) -> usize {
        self.mid_table_bytes
    }

    /// Returns the wide-tier M4RM table byte budget.
    pub fn wide_table_bytes(&self) -> usize {
        self.wide_table_bytes
    }

    /// Returns the maximum panel width for the wide tier.
    pub fn wide_max_k(&self) -> usize {
        self.wide_max_k
    }

    /// Returns the maximum panel width for the small-N tier.
    pub fn small_n_max_k(&self) -> usize {
        self.small_n_max_k
    }
}

/// Selector values for dense matrix inversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenseInverseSelectors {
    m4ri_min_dim: usize,
    blocked_min_dim: usize,
}

impl DenseInverseSelectors {
    /// Builds a validated dense-inverse selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when `blocked_min_dim` is
    /// zero because blocked inversion requires a positive dimension.
    pub fn try_new(m4ri_min_dim: usize, blocked_min_dim: usize) -> Result<Self, ProfileError> {
        if blocked_min_dim == 0 {
            return Err(out_of_range(
                ProfileFamily::DenseInverse,
                ProfileField::BlockedMinDim,
                blocked_min_dim as u64,
            ));
        }
        Ok(Self {
            m4ri_min_dim,
            blocked_min_dim,
        })
    }

    /// Returns the minimum dimension for M4RI inversion.
    pub fn m4ri_min_dim(&self) -> usize {
        self.m4ri_min_dim
    }

    /// Returns the minimum dimension for blocked inversion.
    pub fn blocked_min_dim(&self) -> usize {
        self.blocked_min_dim
    }
}

/// Selector values for triangular solves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriangularSelectors {
    trsm_blocked_min_dim: usize,
    trsm_panel_rows: usize,
    base_case_max_dim: usize,
}

impl TriangularSelectors {
    /// Builds a validated triangular selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when `trsm_panel_rows` or
    /// `base_case_max_dim` is zero because both dimensions must be positive.
    pub fn try_new(
        trsm_blocked_min_dim: usize,
        trsm_panel_rows: usize,
        base_case_max_dim: usize,
    ) -> Result<Self, ProfileError> {
        if trsm_panel_rows == 0 {
            return Err(out_of_range(
                ProfileFamily::Triangular,
                ProfileField::TrsmPanelRows,
                trsm_panel_rows as u64,
            ));
        }
        if base_case_max_dim == 0 {
            return Err(out_of_range(
                ProfileFamily::Triangular,
                ProfileField::BaseCaseMaxDim,
                base_case_max_dim as u64,
            ));
        }
        Ok(Self {
            trsm_blocked_min_dim,
            trsm_panel_rows,
            base_case_max_dim,
        })
    }

    /// Returns the minimum dimension for blocked triangular solves.
    pub fn trsm_blocked_min_dim(&self) -> usize {
        self.trsm_blocked_min_dim
    }

    /// Returns the row extent of a blocked triangular panel.
    pub fn trsm_panel_rows(&self) -> usize {
        self.trsm_panel_rows
    }

    /// Returns the maximum dimension for triangular base cases.
    pub fn base_case_max_dim(&self) -> usize {
        self.base_case_max_dim
    }
}

/// Selector values for PLE decomposition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PleSelectors {
    panel_base_max_cols: usize,
    blocked_back_sub_min_dim: usize,
    scalar_base_max_cols: usize,
    panel_byte_lane_max_cols: usize,
    panel_u16_lane_max_cols: usize,
}

impl PleSelectors {
    /// Builds a validated PLE selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when a panel or scalar
    /// base width is zero, or when a SIMD panel width exceeds its kernel's
    /// asserted bound.
    pub fn try_new(
        panel_base_max_cols: usize,
        blocked_back_sub_min_dim: usize,
        scalar_base_max_cols: usize,
        panel_byte_lane_max_cols: usize,
        panel_u16_lane_max_cols: usize,
    ) -> Result<Self, ProfileError> {
        if panel_base_max_cols == 0 {
            return Err(out_of_range(
                ProfileFamily::Ple,
                ProfileField::PanelBaseMaxCols,
                panel_base_max_cols as u64,
            ));
        }
        if scalar_base_max_cols == 0 {
            return Err(out_of_range(
                ProfileFamily::Ple,
                ProfileField::ScalarBaseMaxCols,
                scalar_base_max_cols as u64,
            ));
        }
        if panel_byte_lane_max_cols == 0
            || panel_byte_lane_max_cols > gf2_kernels_simd::fp_small_ple::PANEL_SCRATCH_COLS
        {
            return Err(out_of_range(
                ProfileFamily::Ple,
                ProfileField::PanelByteLaneMaxCols,
                panel_byte_lane_max_cols as u64,
            ));
        }
        if panel_u16_lane_max_cols == 0
            || panel_u16_lane_max_cols > gf2_kernels_simd::fp_medium_ple::PANEL_SCRATCH_COLS
        {
            return Err(out_of_range(
                ProfileFamily::Ple,
                ProfileField::PanelU16LaneMaxCols,
                panel_u16_lane_max_cols as u64,
            ));
        }
        Ok(Self {
            panel_base_max_cols,
            blocked_back_sub_min_dim,
            scalar_base_max_cols,
            panel_byte_lane_max_cols,
            panel_u16_lane_max_cols,
        })
    }

    /// Returns the maximum recursive PLE panel width.
    pub fn panel_base_max_cols(&self) -> usize {
        self.panel_base_max_cols
    }

    /// Returns the minimum dimension for blocked back substitution.
    pub fn blocked_back_sub_min_dim(&self) -> usize {
        self.blocked_back_sub_min_dim
    }

    /// Returns the maximum width for the scalar PLE base case.
    pub fn scalar_base_max_cols(&self) -> usize {
        self.scalar_base_max_cols
    }

    /// Returns the maximum width for the byte-lane PLE panel kernel.
    ///
    /// The conservative default is the tuned L1d blocking factor
    /// [`gf2_kernels_simd::fp_small_panel::KC`]. The admissible maximum is
    /// the kernel's structural scratch bound
    /// [`gf2_kernels_simd::fp_small_ple::PANEL_SCRATCH_COLS`].
    pub fn panel_byte_lane_max_cols(&self) -> usize {
        self.panel_byte_lane_max_cols
    }

    /// Returns the maximum width for the u16-lane PLE panel kernel.
    ///
    /// The conservative default is the tuned L1d blocking factor
    /// [`gf2_kernels_simd::fp_medium_ple::KC_U16`]. The admissible maximum is
    /// the kernel's structural scratch bound
    /// [`gf2_kernels_simd::fp_medium_ple::PANEL_SCRATCH_COLS`].
    pub fn panel_u16_lane_max_cols(&self) -> usize {
        self.panel_u16_lane_max_cols
    }
}

/// Selector values for finite-field GEMM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GemmSelectors {
    row_tile: usize,
    col_tile: usize,
    axpy_fast_path_min_volume: usize,
    winograd_min_dim: usize,
}

impl GemmSelectors {
    /// Builds a validated GEMM selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when `row_tile` or
    /// `col_tile` is zero because blocked loop steps must be positive.
    pub fn try_new(
        row_tile: usize,
        col_tile: usize,
        axpy_fast_path_min_volume: usize,
        winograd_min_dim: usize,
    ) -> Result<Self, ProfileError> {
        if row_tile == 0 {
            return Err(out_of_range(
                ProfileFamily::Gemm,
                ProfileField::RowTile,
                row_tile as u64,
            ));
        }
        if col_tile == 0 {
            return Err(out_of_range(
                ProfileFamily::Gemm,
                ProfileField::ColTile,
                col_tile as u64,
            ));
        }
        Ok(Self {
            row_tile,
            col_tile,
            axpy_fast_path_min_volume,
            winograd_min_dim,
        })
    }

    /// Returns the GEMM row-tile extent.
    pub fn row_tile(&self) -> usize {
        self.row_tile
    }

    /// Returns the GEMM column-tile extent.
    pub fn col_tile(&self) -> usize {
        self.col_tile
    }

    /// Returns the minimum volume for the GEMM AXPY fast path.
    pub fn axpy_fast_path_min_volume(&self) -> usize {
        self.axpy_fast_path_min_volume
    }

    /// Returns the minimum dimension for the Winograd GEMM path.
    pub fn winograd_min_dim(&self) -> usize {
        self.winograd_min_dim
    }
}

/// Selector values for field-vector operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldVecSelectors {
    dot_chunk_len: usize,
}

impl FieldVecSelectors {
    /// Builds a validated field-vector selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when `dot_chunk_len` is
    /// zero because the vector walk must advance.
    pub fn try_new(dot_chunk_len: usize) -> Result<Self, ProfileError> {
        if dot_chunk_len == 0 {
            return Err(out_of_range(
                ProfileFamily::FieldVec,
                ProfileField::DotChunkLen,
                dot_chunk_len as u64,
            ));
        }
        Ok(Self { dot_chunk_len })
    }

    /// Returns the field-vector dot-product chunk length.
    pub fn dot_chunk_len(&self) -> usize {
        self.dot_chunk_len
    }
}

/// Selector values for characteristic-polynomial algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharpolySelectors {
    keller_gehrig_min_dim: usize,
}

impl CharpolySelectors {
    /// Builds a validated characteristic-polynomial selector family.
    ///
    /// Every `usize` dimension is admissible: zero takes the Keller-Gehrig arm
    /// at every dimension and `usize::MAX` disables it, which is the value the
    /// conservative table carries.
    pub fn try_new(keller_gehrig_min_dim: usize) -> Result<Self, ProfileError> {
        Ok(Self {
            keller_gehrig_min_dim,
        })
    }

    /// Returns the minimum dimension for Keller-Gehrig dispatch.
    pub fn keller_gehrig_min_dim(&self) -> usize {
        self.keller_gehrig_min_dim
    }
}

/// Selector values for prime-field routes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimeRouteSelectors {
    f32_min_prime: usize,
    f32_min_cols: usize,
    f64_min_cols: usize,
}

impl PrimeRouteSelectors {
    /// Builds a validated prime-route selector family.
    ///
    /// Every `usize` bound is admissible: zero opens the floating-point route
    /// to every prime or output width, and a bound above the largest supported
    /// prime or width closes it.
    pub fn try_new(
        f32_min_prime: usize,
        f32_min_cols: usize,
        f64_min_cols: usize,
    ) -> Result<Self, ProfileError> {
        Ok(Self {
            f32_min_prime,
            f32_min_cols,
            f64_min_cols,
        })
    }

    /// Returns the minimum prime for the F32 route.
    pub fn f32_min_prime(&self) -> usize {
        self.f32_min_prime
    }

    /// Returns the minimum F32 route column count.
    pub fn f32_min_cols(&self) -> usize {
        self.f32_min_cols
    }

    /// Returns the minimum F64 route column count.
    pub fn f64_min_cols(&self) -> usize {
        self.f64_min_cols
    }
}

/// Selector values for permanent algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermanentSelectors {
    gray_chunk_subsets: usize,
}

impl PermanentSelectors {
    /// Builds a validated permanent selector family.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::SelectorOutOfRange`] when `gray_chunk_subsets`
    /// is zero because a Gray-code chunk must contain a subset.
    pub fn try_new(gray_chunk_subsets: usize) -> Result<Self, ProfileError> {
        if gray_chunk_subsets == 0 {
            return Err(out_of_range(
                ProfileFamily::Permanent,
                ProfileField::GrayChunkSubsets,
                gray_chunk_subsets as u64,
            ));
        }
        Ok(Self { gray_chunk_subsets })
    }

    /// Returns the Gray-code chunk subset extent.
    pub fn gray_chunk_subsets(&self) -> usize {
        self.gray_chunk_subsets
    }
}

/// The complete set of selector families carried by a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorFamilies {
    /// Bit-buffer backend selectors.
    pub bit_backend: BitBackendSelectors,
    /// Bit-matrix algorithm selectors.
    pub bit_matrix: BitMatrixSelectors,
    /// Structure-of-arrays batch selectors.
    pub soa_batch: SoaBatchSelectors,
    /// M4RM algorithm selectors.
    pub m4rm: M4rmSelectors,
    /// Dense-inverse algorithm selectors.
    pub dense_inverse: DenseInverseSelectors,
    /// Triangular-solve selectors.
    pub triangular: TriangularSelectors,
    /// PLE algorithm selectors.
    pub ple: PleSelectors,
    /// GEMM algorithm selectors.
    pub gemm: GemmSelectors,
    /// Field-vector selectors.
    pub field_vec: FieldVecSelectors,
    /// Characteristic-polynomial selectors.
    pub charpoly: CharpolySelectors,
    /// Polynomial algorithm selectors.
    pub polynomial: PolynomialSelectors,
    /// Prime-field route selectors.
    pub prime_route: PrimeRouteSelectors,
    /// Permanent algorithm selectors.
    pub permanent: PermanentSelectors,
}

impl SelectorFamilies {
    /// The parser-free conservative selector families.
    pub const CONSERVATIVE: Self = Self {
        bit_backend: BitBackendSelectors {
            simd_min_words: crate::kernels::backend::SIMD_MIN_WORDS_DEFAULT,
        },
        bit_matrix: BitMatrixSelectors {
            matvec_simd_min_words: crate::matrix::MATVEC_SIMD_MIN_WORDS,
            transpose_simple_max_blocks:
                crate::matrix::BitMatrix::TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS,
            transpose_macro_tile_blocks: crate::matrix::MACRO_TILE_BLOCKS,
        },
        soa_batch: SoaBatchSelectors {
            parallel_min_len: crate::compute::SOA_PARALLEL_MIN_LEN,
            parallel_chunk_len: crate::compute::SOA_PARALLEL_CHUNK_LEN,
        },
        m4rm: M4rmSelectors {
            wide_tier_min_stride_words: crate::alg::m4rm::M4RM_WIDE_TIER_MIN_STRIDE_WORDS,
            tiled_min_stride_words: crate::alg::m4rm::M4RM_TILED_MIN_STRIDE_WORDS,
            default_table_bytes: crate::alg::m4rm::M4RM_DEFAULT_TABLE_BYTES,
            mid_table_bytes: crate::alg::m4rm::M4RM_MID_TABLE_BYTES,
            wide_table_bytes: crate::alg::m4rm::M4RM_WIDE_TABLE_BYTES,
            wide_max_k: crate::alg::m4rm::M4RM_WIDE_MAX_K,
            small_n_max_k: crate::alg::m4rm::M4RM_SMALL_N_MAX_K,
        },
        dense_inverse: DenseInverseSelectors {
            m4ri_min_dim: crate::alg::gauss::INVERT_M4RI_THRESHOLD,
            blocked_min_dim: crate::field::inverse::BLOCKED_INVERT_THRESHOLD,
        },
        triangular: TriangularSelectors {
            trsm_blocked_min_dim: crate::field::triangular::TRSM_BLOCKED_PANEL_SIZE,
            trsm_panel_rows: crate::field::triangular::TRSM_BLOCKED_PANEL_SIZE,
            base_case_max_dim: crate::field::triangular::TRI_BASE_MAX_DIM_DEFAULT,
        },
        ple: PleSelectors {
            panel_base_max_cols: crate::field::ple::PLE_PANEL_RECURSIVE_BASE,
            blocked_back_sub_min_dim: crate::field::ple::BLOCKED_BACK_SUB_MIN_DIM,
            scalar_base_max_cols: crate::field::ple::PLE_SCALAR_BASE_MAX_COLS_DEFAULT,
            panel_byte_lane_max_cols: gf2_kernels_simd::fp_small_panel::KC,
            panel_u16_lane_max_cols: gf2_kernels_simd::fp_medium_ple::KC_U16,
        },
        gemm: GemmSelectors {
            row_tile: crate::field::matrix::GEMM_ROW_TILE,
            col_tile: crate::field::matrix::GEMM_COL_TILE,
            axpy_fast_path_min_volume: crate::field::matrix::GEMM_AXPY_FAST_PATH_THRESHOLD,
            winograd_min_dim: crate::field::winograd::WINOGRAD_MIN_DIM_DEFAULT,
        },
        field_vec: FieldVecSelectors {
            dot_chunk_len: crate::field::vec::DOT_CHUNK_LEN,
        },
        charpoly: CharpolySelectors {
            keller_gehrig_min_dim: crate::field::charpoly::KG_DISPATCH_MIN_N,
        },
        polynomial: PolynomialSelectors {
            karatsuba_min_degree: crate::field::poly::KARATSUBA_THRESHOLD,
            karatsuba_max_out_len: crate::field::poly::NTT_THRESHOLD,
            div_rem_fast_min_len: crate::field::poly::DIV_REM_THRESHOLD,
            subproduct_min_len: crate::field::poly::SUBPRODUCT_THRESHOLD,
            interpolate_fast_min_points: crate::field::poly_interpolate::INTERPOLATE_THRESHOLD,
        },
        prime_route: PrimeRouteSelectors {
            f32_min_prime: crate::gfp::simd_ops::N_THRESH_PRIME as usize,
            f32_min_cols: crate::gfp::simd_ops::F32_MIN_COLS,
            f64_min_cols: crate::gfp::simd_ops::F64_MIN_COLS,
        },
        permanent: PermanentSelectors {
            gray_chunk_subsets: PERMANENT_GRAY_CHUNK_SUBSETS_DEFAULT,
        },
    };
}

/// A validated, versioned table of algorithm-selection thresholds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuningProfile {
    schema_version: SchemaVersion,
    id: ProfileId,
    provenance: Provenance,
    selectors: SelectorFamilies,
}

impl TuningProfile {
    /// The parser-free conservative table derived from the selector constants.
    pub const CONSERVATIVE: Self = Self {
        schema_version: SchemaVersion(SchemaVersion::SUPPORTED),
        id: ProfileId::from_static("conservative"),
        provenance: Provenance::Inherited,
        selectors: SelectorFamilies::CONSERVATIVE,
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
        &self.selectors.bit_backend
    }

    /// Returns the bit-matrix selector family.
    pub fn bit_matrix(&self) -> &BitMatrixSelectors {
        &self.selectors.bit_matrix
    }

    /// Returns the structure-of-arrays batch selector family.
    pub fn soa_batch(&self) -> &SoaBatchSelectors {
        &self.selectors.soa_batch
    }

    /// Returns the M4RM selector family.
    pub fn m4rm(&self) -> &M4rmSelectors {
        &self.selectors.m4rm
    }

    /// Returns the dense-inverse selector family.
    pub fn dense_inverse(&self) -> &DenseInverseSelectors {
        &self.selectors.dense_inverse
    }

    /// Returns the triangular-solve selector family.
    pub fn triangular(&self) -> &TriangularSelectors {
        &self.selectors.triangular
    }

    /// Returns the PLE selector family.
    pub fn ple(&self) -> &PleSelectors {
        &self.selectors.ple
    }

    /// Returns the GEMM selector family.
    pub fn gemm(&self) -> &GemmSelectors {
        &self.selectors.gemm
    }

    /// Returns the field-vector selector family.
    pub fn field_vec(&self) -> &FieldVecSelectors {
        &self.selectors.field_vec
    }

    /// Returns the characteristic-polynomial selector family.
    pub fn charpoly(&self) -> &CharpolySelectors {
        &self.selectors.charpoly
    }

    /// Returns the polynomial selector family.
    pub fn polynomial(&self) -> &PolynomialSelectors {
        &self.selectors.polynomial
    }

    /// Returns the prime-field route selector family.
    pub fn prime_route(&self) -> &PrimeRouteSelectors {
        &self.selectors.prime_route
    }

    /// Returns the permanent selector family.
    pub fn permanent(&self) -> &PermanentSelectors {
        &self.selectors.permanent
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
        selectors: SelectorFamilies,
    ) -> Result<Self, ProfileError> {
        validate_provenance(&provenance)?;
        Ok(Self {
            schema_version: SchemaVersion(SchemaVersion::SUPPORTED),
            id,
            provenance,
            selectors,
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
        let bit_matrix = match selectors.bit_matrix {
            None => JsonBitMatrix::default(),
            Some(Some(bit_matrix)) => bit_matrix,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let soa_batch = match selectors.soa_batch {
            None => JsonSoaBatch::default(),
            Some(Some(soa_batch)) => soa_batch,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let m4rm = match selectors.m4rm {
            None => JsonM4rm::default(),
            Some(Some(m4rm)) => m4rm,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let dense_inverse = match selectors.dense_inverse {
            None => JsonDenseInverse::default(),
            Some(Some(dense_inverse)) => dense_inverse,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let triangular = match selectors.triangular {
            None => JsonTriangular::default(),
            Some(Some(triangular)) => triangular,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let ple = match selectors.ple {
            None => JsonPle::default(),
            Some(Some(ple)) => ple,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let gemm = match selectors.gemm {
            None => JsonGemm::default(),
            Some(Some(gemm)) => gemm,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let field_vec = match selectors.field_vec {
            None => JsonFieldVec::default(),
            Some(Some(field_vec)) => field_vec,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let charpoly = match selectors.charpoly {
            None => JsonCharpoly::default(),
            Some(Some(charpoly)) => charpoly,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let polynomial = match selectors.polynomial {
            None => JsonPolynomial::default(),
            Some(Some(polynomial)) => polynomial,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let prime_route = match selectors.prime_route {
            None => JsonPrimeRoute::default(),
            Some(Some(prime_route)) => prime_route,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let permanent = match selectors.permanent {
            None => JsonPermanent::default(),
            Some(Some(permanent)) => permanent,
            Some(None) => return Err(ProfileError::Malformed),
        };
        let bit_backend = BitBackendSelectors::try_new(
            optional(bit_backend.simd_min_words)?
                .unwrap_or(Self::CONSERVATIVE.selectors.bit_backend.simd_min_words),
        )?;
        let bit_matrix = BitMatrixSelectors::try_new(
            optional(bit_matrix.matvec_simd_min_words)?.unwrap_or(
                Self::CONSERVATIVE
                    .selectors
                    .bit_matrix
                    .matvec_simd_min_words,
            ),
            optional(bit_matrix.transpose_simple_max_blocks)?.unwrap_or(
                Self::CONSERVATIVE
                    .selectors
                    .bit_matrix
                    .transpose_simple_max_blocks,
            ),
            optional(bit_matrix.transpose_macro_tile_blocks)?.unwrap_or(
                Self::CONSERVATIVE
                    .selectors
                    .bit_matrix
                    .transpose_macro_tile_blocks,
            ),
        )?;
        let soa_batch = SoaBatchSelectors::try_new(
            optional(soa_batch.parallel_min_len)?
                .unwrap_or(Self::CONSERVATIVE.selectors.soa_batch.parallel_min_len),
            optional(soa_batch.parallel_chunk_len)?
                .unwrap_or(Self::CONSERVATIVE.selectors.soa_batch.parallel_chunk_len),
        )?;
        let m4rm = M4rmSelectors::try_new(
            optional(m4rm.wide_tier_min_stride_words)?
                .unwrap_or(Self::CONSERVATIVE.selectors.m4rm.wide_tier_min_stride_words),
            optional(m4rm.tiled_min_stride_words)?
                .unwrap_or(Self::CONSERVATIVE.selectors.m4rm.tiled_min_stride_words),
            optional(m4rm.default_table_bytes)?
                .unwrap_or(Self::CONSERVATIVE.selectors.m4rm.default_table_bytes),
            optional(m4rm.mid_table_bytes)?
                .unwrap_or(Self::CONSERVATIVE.selectors.m4rm.mid_table_bytes),
            optional(m4rm.wide_table_bytes)?
                .unwrap_or(Self::CONSERVATIVE.selectors.m4rm.wide_table_bytes),
            optional(m4rm.wide_max_k)?.unwrap_or(Self::CONSERVATIVE.selectors.m4rm.wide_max_k),
            optional(m4rm.small_n_max_k)?
                .unwrap_or(Self::CONSERVATIVE.selectors.m4rm.small_n_max_k),
        )?;
        let dense_inverse = DenseInverseSelectors::try_new(
            optional(dense_inverse.m4ri_min_dim)?
                .unwrap_or(Self::CONSERVATIVE.selectors.dense_inverse.m4ri_min_dim),
            optional(dense_inverse.blocked_min_dim)?
                .unwrap_or(Self::CONSERVATIVE.selectors.dense_inverse.blocked_min_dim),
        )?;
        let triangular = TriangularSelectors::try_new(
            optional(triangular.trsm_blocked_min_dim)?
                .unwrap_or(Self::CONSERVATIVE.selectors.triangular.trsm_blocked_min_dim),
            optional(triangular.trsm_panel_rows)?
                .unwrap_or(Self::CONSERVATIVE.selectors.triangular.trsm_panel_rows),
            optional(triangular.base_case_max_dim)?
                .unwrap_or(Self::CONSERVATIVE.selectors.triangular.base_case_max_dim),
        )?;
        let ple = PleSelectors::try_new(
            optional(ple.panel_base_max_cols)?
                .unwrap_or(Self::CONSERVATIVE.selectors.ple.panel_base_max_cols),
            optional(ple.blocked_back_sub_min_dim)?
                .unwrap_or(Self::CONSERVATIVE.selectors.ple.blocked_back_sub_min_dim),
            optional(ple.scalar_base_max_cols)?
                .unwrap_or(Self::CONSERVATIVE.selectors.ple.scalar_base_max_cols),
            optional(ple.panel_byte_lane_max_cols)?
                .unwrap_or(Self::CONSERVATIVE.selectors.ple.panel_byte_lane_max_cols),
            optional(ple.panel_u16_lane_max_cols)?
                .unwrap_or(Self::CONSERVATIVE.selectors.ple.panel_u16_lane_max_cols),
        )?;
        let gemm = GemmSelectors::try_new(
            optional(gemm.row_tile)?.unwrap_or(Self::CONSERVATIVE.selectors.gemm.row_tile),
            optional(gemm.col_tile)?.unwrap_or(Self::CONSERVATIVE.selectors.gemm.col_tile),
            optional(gemm.axpy_fast_path_min_volume)?
                .unwrap_or(Self::CONSERVATIVE.selectors.gemm.axpy_fast_path_min_volume),
            optional(gemm.winograd_min_dim)?
                .unwrap_or(Self::CONSERVATIVE.selectors.gemm.winograd_min_dim),
        )?;
        let field_vec = FieldVecSelectors::try_new(
            optional(field_vec.dot_chunk_len)?
                .unwrap_or(Self::CONSERVATIVE.selectors.field_vec.dot_chunk_len),
        )?;
        let charpoly = CharpolySelectors::try_new(
            optional(charpoly.keller_gehrig_min_dim)?
                .unwrap_or(Self::CONSERVATIVE.selectors.charpoly.keller_gehrig_min_dim),
        )?;
        let polynomial = PolynomialSelectors::try_new(
            optional(polynomial.karatsuba_min_degree)?
                .unwrap_or(Self::CONSERVATIVE.selectors.polynomial.karatsuba_min_degree),
            optional(polynomial.karatsuba_max_out_len)?.unwrap_or(
                Self::CONSERVATIVE
                    .selectors
                    .polynomial
                    .karatsuba_max_out_len,
            ),
            optional(polynomial.div_rem_fast_min_len)?
                .unwrap_or(Self::CONSERVATIVE.selectors.polynomial.div_rem_fast_min_len),
            optional(polynomial.subproduct_min_len)?
                .unwrap_or(Self::CONSERVATIVE.selectors.polynomial.subproduct_min_len),
            optional(polynomial.interpolate_fast_min_points)?.unwrap_or(
                Self::CONSERVATIVE
                    .selectors
                    .polynomial
                    .interpolate_fast_min_points,
            ),
        )?;
        let prime_route = PrimeRouteSelectors::try_new(
            optional(prime_route.f32_min_prime)?
                .unwrap_or(Self::CONSERVATIVE.selectors.prime_route.f32_min_prime),
            optional(prime_route.f32_min_cols)?
                .unwrap_or(Self::CONSERVATIVE.selectors.prime_route.f32_min_cols),
            optional(prime_route.f64_min_cols)?
                .unwrap_or(Self::CONSERVATIVE.selectors.prime_route.f64_min_cols),
        )?;
        let permanent = PermanentSelectors::try_new(
            optional(permanent.gray_chunk_subsets)?
                .unwrap_or(Self::CONSERVATIVE.selectors.permanent.gray_chunk_subsets),
        )?;
        let selectors = SelectorFamilies {
            bit_backend,
            bit_matrix,
            soa_batch,
            m4rm,
            dense_inverse,
            triangular,
            ple,
            gemm,
            field_vec,
            charpoly,
            polynomial,
            prime_route,
            permanent,
        };
        Self::try_new(id, provenance, selectors).map(|mut profile| {
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
                    simd_min_words: self.selectors.bit_backend.simd_min_words,
                },
                bit_matrix: JsonBitMatrixOut {
                    matvec_simd_min_words: self.selectors.bit_matrix.matvec_simd_min_words,
                    transpose_simple_max_blocks: self
                        .selectors
                        .bit_matrix
                        .transpose_simple_max_blocks,
                    transpose_macro_tile_blocks: self
                        .selectors
                        .bit_matrix
                        .transpose_macro_tile_blocks,
                },
                soa_batch: JsonSoaBatchOut {
                    parallel_min_len: self.selectors.soa_batch.parallel_min_len,
                    parallel_chunk_len: self.selectors.soa_batch.parallel_chunk_len,
                },
                m4rm: JsonM4rmOut {
                    wide_tier_min_stride_words: self.selectors.m4rm.wide_tier_min_stride_words,
                    tiled_min_stride_words: self.selectors.m4rm.tiled_min_stride_words,
                    default_table_bytes: self.selectors.m4rm.default_table_bytes,
                    mid_table_bytes: self.selectors.m4rm.mid_table_bytes,
                    wide_table_bytes: self.selectors.m4rm.wide_table_bytes,
                    wide_max_k: self.selectors.m4rm.wide_max_k,
                    small_n_max_k: self.selectors.m4rm.small_n_max_k,
                },
                dense_inverse: JsonDenseInverseOut {
                    m4ri_min_dim: self.selectors.dense_inverse.m4ri_min_dim,
                    blocked_min_dim: self.selectors.dense_inverse.blocked_min_dim,
                },
                triangular: JsonTriangularOut {
                    trsm_blocked_min_dim: self.selectors.triangular.trsm_blocked_min_dim,
                    trsm_panel_rows: self.selectors.triangular.trsm_panel_rows,
                    base_case_max_dim: self.selectors.triangular.base_case_max_dim,
                },
                ple: JsonPleOut {
                    panel_base_max_cols: self.selectors.ple.panel_base_max_cols,
                    blocked_back_sub_min_dim: self.selectors.ple.blocked_back_sub_min_dim,
                    scalar_base_max_cols: self.selectors.ple.scalar_base_max_cols,
                    panel_byte_lane_max_cols: self.selectors.ple.panel_byte_lane_max_cols,
                    panel_u16_lane_max_cols: self.selectors.ple.panel_u16_lane_max_cols,
                },
                gemm: JsonGemmOut {
                    row_tile: self.selectors.gemm.row_tile,
                    col_tile: self.selectors.gemm.col_tile,
                    axpy_fast_path_min_volume: self.selectors.gemm.axpy_fast_path_min_volume,
                    winograd_min_dim: self.selectors.gemm.winograd_min_dim,
                },
                field_vec: JsonFieldVecOut {
                    dot_chunk_len: self.selectors.field_vec.dot_chunk_len,
                },
                charpoly: JsonCharpolyOut {
                    keller_gehrig_min_dim: self.selectors.charpoly.keller_gehrig_min_dim,
                },
                polynomial: JsonPolynomialOut {
                    karatsuba_min_degree: self.selectors.polynomial.karatsuba_min_degree,
                    karatsuba_max_out_len: self.selectors.polynomial.karatsuba_max_out_len,
                    div_rem_fast_min_len: self.selectors.polynomial.div_rem_fast_min_len,
                    subproduct_min_len: self.selectors.polynomial.subproduct_min_len,
                    interpolate_fast_min_points: self
                        .selectors
                        .polynomial
                        .interpolate_fast_min_points,
                },
                prime_route: JsonPrimeRouteOut {
                    f32_min_prime: self.selectors.prime_route.f32_min_prime,
                    f32_min_cols: self.selectors.prime_route.f32_min_cols,
                    f64_min_cols: self.selectors.prime_route.f64_min_cols,
                },
                permanent: JsonPermanentOut {
                    gray_chunk_subsets: self.selectors.permanent.gray_chunk_subsets,
                },
            },
        };
        serde_json::to_string(&output).expect("validated tuning profile is serializable")
    }
}

static ACTIVE: OnceLock<TuningProfile> = OnceLock::new();

/// Returns the process-wide profile, resolving to [`TuningProfile::CONSERVATIVE`]
/// when no caller installs one first. Calling this function marks the profile
/// resolved for [`install`]; a bit-backend selection alone does not. The
/// installed bit-backend field remains observable here but does not alter the
/// compile-time bit-backend selection boundary; see DEC-G in
/// `dev/active/220cab0b/design.md`.
pub fn active() -> &'static TuningProfile {
    ACTIVE.get_or_init(|| TuningProfile::CONSERVATIVE.clone())
}

/// Installs the profile before [`active`] resolves.
///
/// A successful installation resolves the same [`OnceLock`] used by [`active`].
/// It does not publish `bit_backend.simd_min_words` to the hot selection
/// boundary: that boundary is a compile-time constant under DEC-G in
/// `dev/active/220cab0b/design.md`. The field remains observable through
/// [`active`] and continues to participate in profile validation and JSON
/// round trips.
///
/// # Errors
///
/// Returns [`AlreadyResolved`] if this process has already called [`active`] or
/// successfully installed another profile. A prior bit-backend selection does
/// not resolve the profile and therefore does not cause this error.
pub fn install(profile: TuningProfile) -> Result<(), AlreadyResolved> {
    let mut installed = false;
    ACTIVE.get_or_init(|| {
        installed = true;
        profile
    });
    if installed {
        Ok(())
    } else {
        Err(AlreadyResolved)
    }
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
    bit_matrix: Option<Option<JsonBitMatrix>>,
    soa_batch: Option<Option<JsonSoaBatch>>,
    m4rm: Option<Option<JsonM4rm>>,
    dense_inverse: Option<Option<JsonDenseInverse>>,
    triangular: Option<Option<JsonTriangular>>,
    ple: Option<Option<JsonPle>>,
    gemm: Option<Option<JsonGemm>>,
    field_vec: Option<Option<JsonFieldVec>>,
    charpoly: Option<Option<JsonCharpoly>>,
    polynomial: Option<Option<JsonPolynomial>>,
    prime_route: Option<Option<JsonPrimeRoute>>,
    permanent: Option<Option<JsonPermanent>>,
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
struct JsonBitMatrix {
    matvec_simd_min_words: Option<Option<usize>>,
    transpose_simple_max_blocks: Option<Option<usize>>,
    transpose_macro_tile_blocks: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonSoaBatch {
    parallel_min_len: Option<Option<usize>>,
    parallel_chunk_len: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonM4rm {
    wide_tier_min_stride_words: Option<Option<usize>>,
    tiled_min_stride_words: Option<Option<usize>>,
    default_table_bytes: Option<Option<usize>>,
    mid_table_bytes: Option<Option<usize>>,
    wide_table_bytes: Option<Option<usize>>,
    wide_max_k: Option<Option<usize>>,
    small_n_max_k: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonDenseInverse {
    m4ri_min_dim: Option<Option<usize>>,
    blocked_min_dim: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonTriangular {
    trsm_blocked_min_dim: Option<Option<usize>>,
    trsm_panel_rows: Option<Option<usize>>,
    base_case_max_dim: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPle {
    panel_base_max_cols: Option<Option<usize>>,
    blocked_back_sub_min_dim: Option<Option<usize>>,
    scalar_base_max_cols: Option<Option<usize>>,
    panel_byte_lane_max_cols: Option<Option<usize>>,
    panel_u16_lane_max_cols: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonGemm {
    row_tile: Option<Option<usize>>,
    col_tile: Option<Option<usize>>,
    axpy_fast_path_min_volume: Option<Option<usize>>,
    winograd_min_dim: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonFieldVec {
    dot_chunk_len: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCharpoly {
    keller_gehrig_min_dim: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPolynomial {
    karatsuba_min_degree: Option<Option<usize>>,
    karatsuba_max_out_len: Option<Option<usize>>,
    div_rem_fast_min_len: Option<Option<usize>>,
    subproduct_min_len: Option<Option<usize>>,
    interpolate_fast_min_points: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPrimeRoute {
    f32_min_prime: Option<Option<usize>>,
    f32_min_cols: Option<Option<usize>>,
    f64_min_cols: Option<Option<usize>>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPermanent {
    gray_chunk_subsets: Option<Option<usize>>,
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
    bit_matrix: JsonBitMatrixOut,
    soa_batch: JsonSoaBatchOut,
    m4rm: JsonM4rmOut,
    dense_inverse: JsonDenseInverseOut,
    triangular: JsonTriangularOut,
    ple: JsonPleOut,
    gemm: JsonGemmOut,
    field_vec: JsonFieldVecOut,
    charpoly: JsonCharpolyOut,
    polynomial: JsonPolynomialOut,
    prime_route: JsonPrimeRouteOut,
    permanent: JsonPermanentOut,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonBitBackendOut {
    simd_min_words: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonBitMatrixOut {
    matvec_simd_min_words: usize,
    transpose_simple_max_blocks: usize,
    transpose_macro_tile_blocks: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonSoaBatchOut {
    parallel_min_len: usize,
    parallel_chunk_len: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonM4rmOut {
    wide_tier_min_stride_words: usize,
    tiled_min_stride_words: usize,
    default_table_bytes: usize,
    mid_table_bytes: usize,
    wide_table_bytes: usize,
    wide_max_k: usize,
    small_n_max_k: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonDenseInverseOut {
    m4ri_min_dim: usize,
    blocked_min_dim: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonTriangularOut {
    trsm_blocked_min_dim: usize,
    trsm_panel_rows: usize,
    base_case_max_dim: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonPleOut {
    panel_base_max_cols: usize,
    blocked_back_sub_min_dim: usize,
    scalar_base_max_cols: usize,
    panel_byte_lane_max_cols: usize,
    panel_u16_lane_max_cols: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonGemmOut {
    row_tile: usize,
    col_tile: usize,
    axpy_fast_path_min_volume: usize,
    winograd_min_dim: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonFieldVecOut {
    dot_chunk_len: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonCharpolyOut {
    keller_gehrig_min_dim: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonPolynomialOut {
    karatsuba_min_degree: usize,
    karatsuba_max_out_len: usize,
    div_rem_fast_min_len: usize,
    subproduct_min_len: usize,
    interpolate_fast_min_points: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonPrimeRouteOut {
    f32_min_prime: usize,
    f32_min_cols: usize,
    f64_min_cols: usize,
}

#[cfg(feature = "tuning-profile")]
#[derive(Serialize)]
struct JsonPermanentOut {
    gray_chunk_subsets: usize,
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
            r#"{{"bit_backend":{{"simd_min_words":{max}}},"polynomial":{{"karatsuba_min_degree":{max},"karatsuba_max_out_len":0,"div_rem_fast_min_len":{max},"subproduct_min_len":{max}}}}}"#
        ));
        let profile = TuningProfile::from_json(&text).unwrap();
        assert_eq!(profile.bit_backend().simd_min_words(), usize::MAX);
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

    #[test]
    fn conservative_values_name_the_follow_on_source_constants() {
        let profile = &TuningProfile::CONSERVATIVE;
        assert_eq!(
            profile.bit_matrix().matvec_simd_min_words(),
            crate::matrix::MATVEC_SIMD_MIN_WORDS
        );
        assert_eq!(
            profile.bit_matrix().transpose_simple_max_blocks(),
            crate::matrix::BitMatrix::TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS
        );
        assert_eq!(
            profile.bit_matrix().transpose_macro_tile_blocks(),
            crate::matrix::MACRO_TILE_BLOCKS
        );
        assert_eq!(
            profile.soa_batch().parallel_min_len(),
            crate::compute::SOA_PARALLEL_MIN_LEN
        );
        assert_eq!(
            profile.soa_batch().parallel_chunk_len(),
            crate::compute::SOA_PARALLEL_CHUNK_LEN
        );
        assert_eq!(
            profile.m4rm().wide_tier_min_stride_words(),
            crate::alg::m4rm::M4RM_WIDE_TIER_MIN_STRIDE_WORDS
        );
        assert_eq!(
            profile.m4rm().tiled_min_stride_words(),
            crate::alg::m4rm::M4RM_TILED_MIN_STRIDE_WORDS
        );
        assert_eq!(
            profile.m4rm().default_table_bytes(),
            crate::alg::m4rm::M4RM_DEFAULT_TABLE_BYTES
        );
        assert_eq!(
            profile.m4rm().mid_table_bytes(),
            crate::alg::m4rm::M4RM_MID_TABLE_BYTES
        );
        assert_eq!(
            profile.m4rm().wide_table_bytes(),
            crate::alg::m4rm::M4RM_WIDE_TABLE_BYTES
        );
        assert_eq!(
            profile.m4rm().wide_max_k(),
            crate::alg::m4rm::M4RM_WIDE_MAX_K
        );
        assert_eq!(
            profile.m4rm().small_n_max_k(),
            crate::alg::m4rm::M4RM_SMALL_N_MAX_K
        );
        assert_eq!(
            profile.dense_inverse().m4ri_min_dim(),
            crate::alg::gauss::INVERT_M4RI_THRESHOLD
        );
        assert_eq!(
            profile.dense_inverse().blocked_min_dim(),
            crate::field::inverse::BLOCKED_INVERT_THRESHOLD
        );
        assert_eq!(
            profile.triangular().trsm_blocked_min_dim(),
            crate::field::triangular::TRSM_BLOCKED_PANEL_SIZE
        );
        assert_eq!(
            profile.triangular().trsm_panel_rows(),
            crate::field::triangular::TRSM_BLOCKED_PANEL_SIZE
        );
        assert_eq!(
            profile.ple().panel_base_max_cols(),
            crate::field::ple::PLE_PANEL_RECURSIVE_BASE
        );
        assert_eq!(
            profile.ple().blocked_back_sub_min_dim(),
            crate::field::ple::BLOCKED_BACK_SUB_MIN_DIM
        );
        assert_eq!(
            profile.gemm().row_tile(),
            crate::field::matrix::GEMM_ROW_TILE
        );
        assert_eq!(
            profile.gemm().col_tile(),
            crate::field::matrix::GEMM_COL_TILE
        );
        assert_eq!(
            profile.gemm().axpy_fast_path_min_volume(),
            crate::field::matrix::GEMM_AXPY_FAST_PATH_THRESHOLD
        );
        assert_eq!(
            profile.field_vec().dot_chunk_len(),
            crate::field::vec::DOT_CHUNK_LEN
        );
        assert_eq!(
            profile.charpoly().keller_gehrig_min_dim(),
            crate::field::charpoly::KG_DISPATCH_MIN_N
        );
        assert_eq!(
            profile.polynomial().interpolate_fast_min_points(),
            crate::field::poly_interpolate::INTERPOLATE_THRESHOLD
        );
        assert_eq!(
            profile.prime_route().f32_min_prime(),
            crate::gfp::simd_ops::N_THRESH_PRIME as usize
        );
        assert_eq!(
            profile.prime_route().f32_min_cols(),
            crate::gfp::simd_ops::F32_MIN_COLS
        );
        assert_eq!(
            profile.prime_route().f64_min_cols(),
            crate::gfp::simd_ops::F64_MIN_COLS
        );
        assert_eq!(
            profile.permanent().gray_chunk_subsets(),
            PERMANENT_GRAY_CHUNK_SUBSETS_DEFAULT
        );
    }

    #[test]
    fn follow_on_bounded_fields_report_their_family_and_field() {
        let cases = [
            (
                ProfileFamily::BitMatrix,
                ProfileField::TransposeMacroTileBlocks,
                "bit_matrix",
                "transpose_macro_tile_blocks",
                0,
            ),
            (
                ProfileFamily::SoaBatch,
                ProfileField::ParallelChunkLen,
                "soa_batch",
                "parallel_chunk_len",
                0,
            ),
            (
                ProfileFamily::M4rm,
                ProfileField::TiledMinStrideWords,
                "m4rm",
                "tiled_min_stride_words",
                crate::alg::m4rm::M4RM_TILE_WORDS - 1,
            ),
            (
                ProfileFamily::M4rm,
                ProfileField::WideMaxK,
                "m4rm",
                "wide_max_k",
                0,
            ),
            (
                ProfileFamily::M4rm,
                ProfileField::SmallNMaxK,
                "m4rm",
                "small_n_max_k",
                1,
            ),
            (
                ProfileFamily::DenseInverse,
                ProfileField::BlockedMinDim,
                "dense_inverse",
                "blocked_min_dim",
                0,
            ),
            (
                ProfileFamily::Triangular,
                ProfileField::TrsmPanelRows,
                "triangular",
                "trsm_panel_rows",
                0,
            ),
            (
                ProfileFamily::Ple,
                ProfileField::PanelBaseMaxCols,
                "ple",
                "panel_base_max_cols",
                0,
            ),
            (
                ProfileFamily::Gemm,
                ProfileField::RowTile,
                "gemm",
                "row_tile",
                0,
            ),
            (
                ProfileFamily::Gemm,
                ProfileField::ColTile,
                "gemm",
                "col_tile",
                0,
            ),
            (
                ProfileFamily::FieldVec,
                ProfileField::DotChunkLen,
                "field_vec",
                "dot_chunk_len",
                0,
            ),
            (
                ProfileFamily::Polynomial,
                ProfileField::InterpolateFastMinPoints,
                "polynomial",
                "interpolate_fast_min_points",
                0,
            ),
            (
                ProfileFamily::Permanent,
                ProfileField::GrayChunkSubsets,
                "permanent",
                "gray_chunk_subsets",
                0,
            ),
        ];
        for (family, field, family_name, field_name, value) in cases {
            let text = inherited_document(&format!(
                r#"{{"{family_name}":{{"{field_name}":{value}}}}}"#
            ));
            assert_eq!(
                TuningProfile::from_json(&text).unwrap_err(),
                ProfileError::SelectorOutOfRange {
                    family,
                    field,
                    value: value as u64,
                }
            );
        }
    }

    #[test]
    fn follow_on_range_floors_are_admissible() {
        let cases = [
            ("bit_matrix", "transpose_macro_tile_blocks", 1),
            ("soa_batch", "parallel_chunk_len", 1),
            (
                "m4rm",
                "tiled_min_stride_words",
                crate::alg::m4rm::M4RM_TILE_WORDS,
            ),
            ("m4rm", "wide_max_k", 1),
            ("m4rm", "small_n_max_k", 2),
            ("dense_inverse", "blocked_min_dim", 1),
            ("triangular", "trsm_panel_rows", 1),
            ("ple", "panel_base_max_cols", 1),
            ("gemm", "row_tile", 1),
            ("gemm", "col_tile", 1),
            ("field_vec", "dot_chunk_len", 1),
            ("polynomial", "interpolate_fast_min_points", 1),
            ("permanent", "gray_chunk_subsets", 1),
        ];
        for (family, field, floor) in cases {
            let text = inherited_document(&format!(r#"{{"{family}":{{"{field}":{floor}}}}}"#));
            let profile = TuningProfile::from_json(&text)
                .unwrap_or_else(|error| panic!("{family}.{field} rejects its floor: {error}"));
            let read_back = match (family, field) {
                ("bit_matrix", _) => profile.bit_matrix().transpose_macro_tile_blocks(),
                ("soa_batch", _) => profile.soa_batch().parallel_chunk_len(),
                ("m4rm", "tiled_min_stride_words") => profile.m4rm().tiled_min_stride_words(),
                ("m4rm", "wide_max_k") => profile.m4rm().wide_max_k(),
                ("m4rm", _) => profile.m4rm().small_n_max_k(),
                ("dense_inverse", _) => profile.dense_inverse().blocked_min_dim(),
                ("triangular", _) => profile.triangular().trsm_panel_rows(),
                ("ple", _) => profile.ple().panel_base_max_cols(),
                ("gemm", "row_tile") => profile.gemm().row_tile(),
                ("gemm", _) => profile.gemm().col_tile(),
                ("field_vec", _) => profile.field_vec().dot_chunk_len(),
                ("polynomial", _) => profile.polynomial().interpolate_fast_min_points(),
                _ => profile.permanent().gray_chunk_subsets(),
            };
            assert_eq!(read_back, floor, "{family}.{field}");
        }
    }

    #[test]
    fn unbounded_follow_on_fields_admit_both_endpoints() {
        let zero = TuningProfile::from_json(&inherited_document(
            r#"{"bit_matrix":{"matvec_simd_min_words":0,"transpose_simple_max_blocks":0},"soa_batch":{"parallel_min_len":0},"m4rm":{"wide_tier_min_stride_words":0,"default_table_bytes":0,"mid_table_bytes":0,"wide_table_bytes":0},"dense_inverse":{"m4ri_min_dim":0},"triangular":{"trsm_blocked_min_dim":0},"ple":{"blocked_back_sub_min_dim":0},"gemm":{"axpy_fast_path_min_volume":0},"field_vec":{},"prime_route":{"f32_min_prime":0,"f32_min_cols":0,"f64_min_cols":0}}"#,
        )).unwrap();
        assert_eq!(zero.bit_matrix().matvec_simd_min_words(), 0);
        assert_eq!(zero.bit_matrix().transpose_simple_max_blocks(), 0);
        assert_eq!(zero.soa_batch().parallel_min_len(), 0);
        assert_eq!(zero.m4rm().wide_tier_min_stride_words(), 0);
        assert_eq!(zero.m4rm().default_table_bytes(), 0);
        assert_eq!(zero.m4rm().mid_table_bytes(), 0);
        assert_eq!(zero.m4rm().wide_table_bytes(), 0);
        assert_eq!(zero.dense_inverse().m4ri_min_dim(), 0);
        assert_eq!(zero.triangular().trsm_blocked_min_dim(), 0);
        assert_eq!(zero.ple().blocked_back_sub_min_dim(), 0);
        assert_eq!(zero.gemm().axpy_fast_path_min_volume(), 0);
        assert_eq!(zero.prime_route().f32_min_prime(), 0);
        assert_eq!(zero.prime_route().f32_min_cols(), 0);
        assert_eq!(zero.prime_route().f64_min_cols(), 0);

        let max = inherited_document(&format!(
            r#"{{"charpoly":{{"keller_gehrig_min_dim":{}}}}}"#,
            usize::MAX
        ));
        let profile = TuningProfile::from_json(&max).unwrap();
        assert_eq!(profile.charpoly().keller_gehrig_min_dim(), usize::MAX);
    }

    #[test]
    fn absent_follow_on_families_inherit_defaults() {
        let profile = TuningProfile::from_json(
            r#"{"schema_version":1,"profile_id":"example","provenance":{"kind":"inherited"}}"#,
        )
        .unwrap();
        let conservative = &TuningProfile::CONSERVATIVE;
        assert_eq!(profile.bit_matrix(), conservative.bit_matrix());
        assert_eq!(profile.soa_batch(), conservative.soa_batch());
        assert_eq!(profile.m4rm(), conservative.m4rm());
        assert_eq!(profile.dense_inverse(), conservative.dense_inverse());
        assert_eq!(profile.triangular(), conservative.triangular());
        assert_eq!(profile.ple(), conservative.ple());
        assert_eq!(profile.gemm(), conservative.gemm());
        assert_eq!(profile.field_vec(), conservative.field_vec());
        assert_eq!(profile.charpoly(), conservative.charpoly());
        assert_eq!(profile.prime_route(), conservative.prime_route());
        assert_eq!(profile.permanent(), conservative.permanent());
        assert_eq!(
            profile.polynomial().interpolate_fast_min_points(),
            conservative.polynomial().interpolate_fast_min_points()
        );
    }

    #[test]
    fn partially_specified_follow_on_family_inherits_sibling_defaults() {
        let profile =
            TuningProfile::from_json(&inherited_document(r#"{"m4rm":{"wide_max_k":10}}"#)).unwrap();
        let conservative = &TuningProfile::CONSERVATIVE;
        assert_eq!(profile.m4rm().wide_max_k(), 10);
        assert_eq!(
            profile.m4rm().wide_tier_min_stride_words(),
            conservative.m4rm().wide_tier_min_stride_words()
        );
        assert_eq!(
            profile.m4rm().tiled_min_stride_words(),
            conservative.m4rm().tiled_min_stride_words()
        );
        assert_eq!(
            profile.m4rm().default_table_bytes(),
            conservative.m4rm().default_table_bytes()
        );
        assert_eq!(
            profile.m4rm().mid_table_bytes(),
            conservative.m4rm().mid_table_bytes()
        );
        assert_eq!(
            profile.m4rm().wide_table_bytes(),
            conservative.m4rm().wide_table_bytes()
        );
        assert_eq!(
            profile.m4rm().small_n_max_k(),
            conservative.m4rm().small_n_max_k()
        );
        assert_eq!(profile.bit_matrix(), conservative.bit_matrix());
        assert_eq!(profile.soa_batch(), conservative.soa_batch());
        assert_eq!(profile.dense_inverse(), conservative.dense_inverse());
        assert_eq!(profile.triangular(), conservative.triangular());
        assert_eq!(profile.ple(), conservative.ple());
        assert_eq!(profile.gemm(), conservative.gemm());
        assert_eq!(profile.field_vec(), conservative.field_vec());
        assert_eq!(profile.charpoly(), conservative.charpoly());
        assert_eq!(profile.polynomial(), conservative.polynomial());
        assert_eq!(profile.prime_route(), conservative.prime_route());
        assert_eq!(profile.permanent(), conservative.permanent());
    }

    #[test]
    fn unknown_keys_in_follow_on_families_are_rejected() {
        for family in [
            "bit_matrix",
            "soa_batch",
            "m4rm",
            "dense_inverse",
            "triangular",
            "ple",
            "gemm",
            "field_vec",
            "charpoly",
            "prime_route",
            "permanent",
        ] {
            let text = inherited_document(&format!(r#"{{"{family}":{{"unknown":0}}}}"#));
            assert!(TuningProfile::from_json(&text).is_err(), "{family}");
        }
        let text = inherited_document(r#"{"unknown_family":{}}"#);
        assert!(TuningProfile::from_json(&text).is_err());
    }

    #[test]
    fn follow_on_profile_round_trips() {
        let text = inherited_document(
            r#"{
                "bit_backend":{"simd_min_words":9},
                "bit_matrix":{"matvec_simd_min_words":9,"transpose_simple_max_blocks":17,"transpose_macro_tile_blocks":9},
                "soa_batch":{"parallel_min_len":32769,"parallel_chunk_len":16385},
                "m4rm":{"wide_tier_min_stride_words":17,"tiled_min_stride_words":5,"default_table_bytes":65537,"mid_table_bytes":131073,"wide_table_bytes":262145,"wide_max_k":10,"small_n_max_k":9},
                "dense_inverse":{"m4ri_min_dim":9,"blocked_min_dim":17},
                "triangular":{"trsm_blocked_min_dim":65,"trsm_panel_rows":65},
                "ple":{"panel_base_max_cols":129,"blocked_back_sub_min_dim":129},
                "gemm":{"row_tile":33,"col_tile":65,"axpy_fast_path_min_volume":4097},
                "field_vec":{"dot_chunk_len":257},
                "charpoly":{"keller_gehrig_min_dim":18446744073709551614},
                "polynomial":{"karatsuba_min_degree":33,"karatsuba_max_out_len":129,"div_rem_fast_min_len":2049,"subproduct_min_len":4097,"interpolate_fast_min_points":17},
                "prime_route":{"f32_min_prime":252,"f32_min_cols":513,"f64_min_cols":513},
                "permanent":{"gray_chunk_subsets":65537}
            }"#,
        );
        let profile = TuningProfile::from_json(&text).unwrap();
        assert_eq!(
            TuningProfile::from_json(&profile.to_json()).unwrap(),
            profile
        );
        assert_eq!(profile.bit_matrix().matvec_simd_min_words(), 9);
        assert_eq!(profile.soa_batch().parallel_min_len(), 32769);
        assert_eq!(profile.m4rm().wide_tier_min_stride_words(), 17);
        assert_eq!(profile.dense_inverse().m4ri_min_dim(), 9);
        assert_eq!(profile.triangular().trsm_blocked_min_dim(), 65);
        assert_eq!(profile.ple().panel_base_max_cols(), 129);
        assert_eq!(profile.gemm().row_tile(), 33);
        assert_eq!(profile.field_vec().dot_chunk_len(), 257);
        assert_eq!(profile.charpoly().keller_gehrig_min_dim(), usize::MAX - 1);
        assert_eq!(profile.polynomial().interpolate_fast_min_points(), 17);
        assert_eq!(profile.prime_route().f32_min_prime(), 252);
        assert_eq!(profile.permanent().gray_chunk_subsets(), 65_537);
    }

    #[test]
    fn seam_fields_validate_ranges_and_round_trip() {
        let conservative = &TuningProfile::CONSERVATIVE;
        assert_eq!(
            conservative.gemm().winograd_min_dim(),
            crate::field::winograd::WINOGRAD_MIN_DIM_DEFAULT
        );
        assert_eq!(
            conservative.triangular().base_case_max_dim(),
            crate::field::triangular::TRI_BASE_MAX_DIM_DEFAULT
        );
        assert_eq!(
            conservative.ple().scalar_base_max_cols(),
            crate::field::ple::PLE_SCALAR_BASE_MAX_COLS_DEFAULT
        );
        assert_eq!(
            conservative.ple().panel_byte_lane_max_cols(),
            gf2_kernels_simd::fp_small_panel::KC
        );
        assert_eq!(
            conservative.ple().panel_u16_lane_max_cols(),
            gf2_kernels_simd::fp_medium_ple::KC_U16
        );

        let profile = TuningProfile::from_json(&inherited_document(&format!(
            r#"{{
                "gemm":{{"winograd_min_dim":0}},
                "triangular":{{"base_case_max_dim":1}},
                "ple":{{"scalar_base_max_cols":1,"panel_byte_lane_max_cols":{},"panel_u16_lane_max_cols":{}}}
            }}"#,
            gf2_kernels_simd::fp_small_ple::PANEL_SCRATCH_COLS,
            gf2_kernels_simd::fp_medium_ple::PANEL_SCRATCH_COLS,
        )))
        .unwrap();
        assert_eq!(
            TuningProfile::from_json(&profile.to_json()).unwrap(),
            profile
        );
        assert_eq!(profile.gemm().winograd_min_dim(), 0);
        assert_eq!(profile.triangular().base_case_max_dim(), 1);
        assert_eq!(profile.ple().scalar_base_max_cols(), 1);
        assert_eq!(
            profile.ple().panel_byte_lane_max_cols(),
            gf2_kernels_simd::fp_small_ple::PANEL_SCRATCH_COLS
        );
        assert_eq!(
            profile.ple().panel_u16_lane_max_cols(),
            gf2_kernels_simd::fp_medium_ple::PANEL_SCRATCH_COLS
        );
    }

    #[test]
    fn seam_fields_reject_zero_or_kernel_bound_excess() {
        for (family, field, expected) in [
            (
                "triangular",
                "base_case_max_dim",
                ProfileField::BaseCaseMaxDim,
            ),
            (
                "ple",
                "scalar_base_max_cols",
                ProfileField::ScalarBaseMaxCols,
            ),
            (
                "ple",
                "panel_byte_lane_max_cols",
                ProfileField::PanelByteLaneMaxCols,
            ),
            (
                "ple",
                "panel_u16_lane_max_cols",
                ProfileField::PanelU16LaneMaxCols,
            ),
        ] {
            let text = inherited_document(&format!(r#"{{"{family}":{{"{field}":0}}}}"#));
            assert_eq!(
                TuningProfile::from_json(&text).unwrap_err(),
                ProfileError::SelectorOutOfRange {
                    family: if family == "triangular" {
                        ProfileFamily::Triangular
                    } else {
                        ProfileFamily::Ple
                    },
                    field: expected,
                    value: 0,
                }
            );
        }

        for (field, value) in [
            (
                ProfileField::PanelByteLaneMaxCols,
                gf2_kernels_simd::fp_small_ple::PANEL_SCRATCH_COLS + 1,
            ),
            (
                ProfileField::PanelU16LaneMaxCols,
                gf2_kernels_simd::fp_medium_ple::PANEL_SCRATCH_COLS + 1,
            ),
        ] {
            let name = field.to_string();
            let text = inherited_document(&format!(r#"{{"ple":{{"{name}":{value}}}}}"#));
            assert_eq!(
                TuningProfile::from_json(&text).unwrap_err(),
                ProfileError::SelectorOutOfRange {
                    family: ProfileFamily::Ple,
                    field,
                    value: value as u64,
                }
            );
        }

        for (field, value) in [
            (
                ProfileField::PanelByteLaneMaxCols,
                gf2_kernels_simd::fp_small_ple::PANEL_SCRATCH_COLS,
            ),
            (
                ProfileField::PanelU16LaneMaxCols,
                gf2_kernels_simd::fp_medium_ple::PANEL_SCRATCH_COLS,
            ),
        ] {
            let name = field.to_string();
            let text = inherited_document(&format!(r#"{{"ple":{{"{name}":{value}}}}}"#));
            assert!(TuningProfile::from_json(&text).is_ok());
        }

        let text = inherited_document(&format!(
            r#"{{"ple":{{"panel_u16_lane_max_cols":{}}}}}"#,
            gf2_kernels_simd::fp_medium_ple::KC_U16 + 1,
        ));
        assert!(TuningProfile::from_json(&text).is_ok());
    }
}
