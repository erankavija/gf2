//! Typed execution tuning and the process-wide resolution authority.
//!
//! [`PreparedEnvelope`] is the one installable container for crate-owned
//! [`TuningSection`] values. Installation and first access race through one
//! process-wide authority: installation wins once, while access before install
//! permanently resolves missing sections to their owner defaults. With the
//! `tuning-profile` feature, an explicit [`ProfileRegistry`] strictly decodes
//! or encodes canonical format-2 envelopes.
//!
//! ```
//! use gf2_core::tuning::{
//!     self, BitBackendSelectors, CompiledProfileProvenance, CoreSelectors,
//!     CoreTuning, PreparedEnvelope, ProfileId,
//! };
//!
//! let mut selectors = CoreSelectors::CONSERVATIVE.clone();
//! selectors.bit_backend = BitBackendSelectors::try_new(16).unwrap();
//! let id = ProfileId::parse("example").unwrap();
//! let prepared = PreparedEnvelope::compiled(
//!     id.clone(),
//!     CompiledProfileProvenance { artifact_id: id },
//! )
//! .insert(CoreTuning::from_selectors(selectors))
//! .unwrap()
//! .build()
//! .unwrap();
//! tuning::install(prepared).expect("tuning has not been resolved");
//! assert_eq!(tuning::active().bit_backend().simd_min_words(), 16);
//! ```
//!
//! Provenance descriptions remain plain strings and feature names remain
//! `Vec<String>` because they are runtime-observed free text or opaque tokens.

use std::fmt;

#[cfg(any(test, gf2_tuning_baked))]
pub(crate) mod baked;
mod mechanism;
pub use mechanism::*;

#[cfg(feature = "tuning-profile")]
mod profile;
#[cfg(feature = "tuning-profile")]
pub use profile::*;

#[cfg(feature = "tuning-profile")]
use serde::Deserialize;

/// A logical tuning configuration or campaign label in lowercase kebab case.
///
/// The label is not a globally unique identity for artifact bytes. An exact
/// serialized artifact is identified by its explicit path, verified content
/// digest, and provenance; owner and complete envelopes may share one label.
#[derive(Clone, Debug)]
pub struct ProfileId(ProfileText);

impl ProfileId {
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

/// A lexically validated calibration-harness behavior identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessSchema(String);

impl HarnessSchema {
    /// Parses a non-empty lowercase kebab-case harness schema token.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Malformed`] for any other lexical form. The
    /// owning section codec decides which valid tokens it supports.
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        if is_kebab_case(value) {
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

/// The core selector family named by a validation error.
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
}

impl fmt::Display for ProfileFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ProfileFamily {
    const fn as_str(self) -> &'static str {
        match self {
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
        }
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
}

impl fmt::Display for ProfileField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ProfileField {
    const fn as_str(self) -> &'static str {
        match self {
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
        }
    }
}

/// Errors returned while parsing or validating a tuning profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// The document uses an envelope format this loader does not implement.
    UnsupportedProfileFormatVersion {
        /// The version found in the document.
        found: u32,
        /// The only supported envelope format.
        supported: u32,
    },
    /// A present section uses a schema version its owner codec does not implement.
    UnsupportedSectionSchemaVersion {
        /// Stable wire identity of the section.
        id: String,
        /// Version found in the section wrapper.
        found: u32,
        /// Version supported by the registered owner codec.
        supported: u32,
    },
    /// A section's calibrated evidence names an unsupported harness behavior.
    UnsupportedHarnessSchema {
        /// Stable identity of the section interpreting the token.
        id: String,
        /// Token found in the section measurement record.
        found: String,
        /// Token supported by the registered owner codec.
        supported: &'static str,
    },
    /// A present section has no registered owner codec.
    UnexpectedSection {
        /// Stable wire identity found in the document.
        id: String,
    },
    /// A prepared entry has no registered codec for deterministic encoding.
    UnregisteredSectionForEncoding {
        /// Stable identity of the prepared section.
        id: String,
    },
    /// A profile or prepared-envelope builder received a duplicate section ID.
    DuplicateSectionId {
        /// Stable identity that appeared more than once.
        id: String,
    },
    /// A profile or prepared-envelope builder received one Rust type twice.
    DuplicateSectionType {
        /// Rust type name recorded for diagnostics.
        type_name: &'static str,
    },
    /// Canonical payload bytes do not match the assembly content digest.
    ContentDigestMismatch {
        /// Digest recorded in the envelope.
        found: String,
        /// Digest recomputed from the canonical content value.
        expected: String,
    },
    /// The registry's stable-ID and Rust-type invariants disagree.
    RegistryInvariant {
        /// Stable section identity involved in the mismatch.
        id: String,
        /// Expected Rust type name.
        expected_type: &'static str,
        /// Stored Rust type name.
        stored_type: &'static str,
    },
    /// An owner codec rejected its section body or measurement provenance.
    InvalidSection {
        /// Stable identity of the rejected section.
        id: String,
        /// Codec-owned validation failure.
        source: SectionError,
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
            Self::UnsupportedProfileFormatVersion { found, supported } => write!(
                formatter,
                "unsupported profile format version {found}; supported version is {supported}"
            ),
            Self::UnsupportedSectionSchemaVersion {
                id,
                found,
                supported,
            } => write!(
                formatter,
                "section {id} uses schema version {found}; supported version is {supported}"
            ),
            Self::UnsupportedHarnessSchema {
                id,
                found,
                supported,
            } => write!(
                formatter,
                "section {id} uses harness schema {found:?}; supported token is {supported}"
            ),
            Self::UnexpectedSection { id } => {
                write!(formatter, "unexpected or unregistered tuning section {id}")
            }
            Self::UnregisteredSectionForEncoding { id } => {
                write!(
                    formatter,
                    "no codec is registered to encode tuning section {id}"
                )
            }
            Self::DuplicateSectionId { id } => {
                write!(formatter, "duplicate tuning section ID {id}")
            }
            Self::DuplicateSectionType { type_name } => {
                write!(formatter, "duplicate tuning section Rust type {type_name}")
            }
            Self::ContentDigestMismatch { found, expected } => write!(
                formatter,
                "tuning envelope content digest {found} does not match {expected}"
            ),
            Self::RegistryInvariant {
                id,
                expected_type,
                stored_type,
            } => write!(
                formatter,
                "registry invariant failed for {id}: expected {expected_type}, stored {stored_type}"
            ),
            Self::InvalidSection { id, source } => {
                write!(formatter, "invalid tuning section {id}: {source}")
            }
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
    /// [`CoreTuningCodec`] encodes this field in the core section, and
    /// [`active`] exposes its installed or conservative value. The bit-backend
    /// routing boundary uses a compile-time constant instead, per DEC-G in
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

/// The complete set of selector families carried by a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreSelectors {
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
}

impl CoreSelectors {
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
            row_tile: crate::field::matrix::GEMM_ROW_TILE_DEFAULT,
            col_tile: crate::field::matrix::GEMM_COL_TILE_DEFAULT,
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
    };
}

/// Core-owned algorithm selectors carried by the `gf2-core/selectors` section.
#[derive(Clone, PartialEq, Eq)]
pub struct CoreTuning {
    selectors: CoreSelectors,
    #[cfg(feature = "tuning-profile")]
    presence: CorePresence,
}

impl fmt::Debug for CoreTuning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CoreTuning");
        debug.field("selectors", &self.selectors);
        #[cfg(feature = "tuning-profile")]
        {
            debug
                .field("declared_families", &self.presence.families.count_ones())
                .field("declared_fields", &self.presence.fields.count_ones());
        }
        debug.finish()
    }
}

impl CoreTuning {
    /// The parser-free conservative core section.
    pub const CONSERVATIVE: Self = Self {
        selectors: CoreSelectors::CONSERVATIVE,
        #[cfg(feature = "tuning-profile")]
        presence: CorePresence::COMPLETE,
    };

    /// Builds a complete programmatic core section from validated selector families.
    #[must_use]
    pub const fn from_selectors(selectors: CoreSelectors) -> Self {
        Self {
            selectors,
            #[cfg(feature = "tuning-profile")]
            presence: CorePresence::COMPLETE,
        }
    }

    /// Returns the complete core selector view.
    #[must_use]
    pub fn selectors(&self) -> &CoreSelectors {
        &self.selectors
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
}

impl TuningSection for CoreTuning {
    const ID: SectionId = SectionId::from_static("gf2-core/selectors");
    type Selectors = CoreSelectors;

    fn conservative() -> &'static Self {
        &Self::CONSERVATIVE
    }

    fn selectors(&self) -> &Self::Selectors {
        &self.selectors
    }
}

/// Returns the process-wide core tuning section and its resolution provenance.
///
/// This wrapper performs the generic accessor's one `O(log s)` installed
/// section lookup (`s` is the number of installed sections) and does not
/// allocate or lock after process resolution. Call it once at the outer
/// operation boundary rather than from a hot inner loop.
///
/// # Panics
///
/// Panics with [`ActiveSectionInvariant`] only if installed erased storage
/// violates the typed section invariant. Malformed profile input is rejected
/// before installation.
#[must_use]
#[track_caller]
pub fn active() -> ActiveSection<'static, CoreTuning> {
    active_section::<CoreTuning>()
}

#[cfg(feature = "tuning-profile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CorePresence {
    families: u16,
    fields: u64,
}

#[cfg(feature = "tuning-profile")]
impl CorePresence {
    const COMPLETE: Self = Self {
        families: (1 << 12) - 1,
        fields: (1 << 37) - 1,
    };

    const fn family(self, bit: u16) -> bool {
        self.families & bit != 0
    }

    const fn field(self, bit: u64) -> bool {
        self.fields & bit != 0
    }
}

#[cfg(feature = "tuning-profile")]
const CORE_HARNESS_SCHEMA: &str = "tuning-calibration-v3";

/// Format-2 owner codec for [`CoreTuning`].
#[cfg(feature = "tuning-profile")]
pub struct CoreTuningCodec;

#[cfg(feature = "tuning-profile")]
impl CoreTuningCodec {
    /// Measurement-behavior token emitted by the current calibration harness.
    pub const HARNESS_SCHEMA: &'static str = CORE_HARNESS_SCHEMA;
}

#[cfg(feature = "tuning-profile")]
impl SectionCodec<CoreTuning> for CoreTuningCodec {
    const SCHEMA_VERSION: u32 = 1;

    fn validate_measurement(value: &MeasurementProvenance) -> Result<(), SectionError> {
        match value {
            MeasurementProvenance::Inherited => Ok(()),
            MeasurementProvenance::Calibrated { harness_schema, .. }
                if harness_schema.as_str() == CORE_HARNESS_SCHEMA =>
            {
                Ok(())
            }
            MeasurementProvenance::Calibrated { harness_schema, .. } => Err(
                SectionError::unsupported_harness_found(harness_schema, CORE_HARNESS_SCHEMA),
            ),
        }
    }

    fn decode_body(body: CanonicalValue) -> Result<CoreTuning, SectionError> {
        let selectors: JsonSelectors = body.deserialize()?;
        let presence = core_presence(&selectors);
        let bit_backend = match selectors.bit_backend {
            Present::Missing => JsonBitBackend::default(),
            Present::Value(Some(bit_backend)) => bit_backend,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null bit_backend family".into()))
            }
        };
        let bit_matrix = match selectors.bit_matrix {
            Present::Missing => JsonBitMatrix::default(),
            Present::Value(Some(bit_matrix)) => bit_matrix,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null bit_matrix family".into()))
            }
        };
        let soa_batch = match selectors.soa_batch {
            Present::Missing => JsonSoaBatch::default(),
            Present::Value(Some(soa_batch)) => soa_batch,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null soa_batch family".into()))
            }
        };
        let m4rm = match selectors.m4rm {
            Present::Missing => JsonM4rm::default(),
            Present::Value(Some(m4rm)) => m4rm,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null m4rm family".into()))
            }
        };
        let dense_inverse = match selectors.dense_inverse {
            Present::Missing => JsonDenseInverse::default(),
            Present::Value(Some(dense_inverse)) => dense_inverse,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody(
                    "null dense_inverse family".into(),
                ))
            }
        };
        let triangular = match selectors.triangular {
            Present::Missing => JsonTriangular::default(),
            Present::Value(Some(triangular)) => triangular,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null triangular family".into()))
            }
        };
        let ple = match selectors.ple {
            Present::Missing => JsonPle::default(),
            Present::Value(Some(ple)) => ple,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null ple family".into()))
            }
        };
        let gemm = match selectors.gemm {
            Present::Missing => JsonGemm::default(),
            Present::Value(Some(gemm)) => gemm,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null gemm family".into()))
            }
        };
        let field_vec = match selectors.field_vec {
            Present::Missing => JsonFieldVec::default(),
            Present::Value(Some(field_vec)) => field_vec,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null field_vec family".into()))
            }
        };
        let charpoly = match selectors.charpoly {
            Present::Missing => JsonCharpoly::default(),
            Present::Value(Some(charpoly)) => charpoly,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null charpoly family".into()))
            }
        };
        let polynomial = match selectors.polynomial {
            Present::Missing => JsonPolynomial::default(),
            Present::Value(Some(polynomial)) => polynomial,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null polynomial family".into()))
            }
        };
        let prime_route = match selectors.prime_route {
            Present::Missing => JsonPrimeRoute::default(),
            Present::Value(Some(prime_route)) => prime_route,
            Present::Value(None) => {
                return Err(SectionError::InvalidBody("null prime_route family".into()))
            }
        };
        let bit_backend = BitBackendSelectors::try_new(
            section_optional(bit_backend.simd_min_words)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .bit_backend
                    .simd_min_words,
            ),
        )
        .map_err(core_section_error)?;
        let bit_matrix = BitMatrixSelectors::try_new(
            section_optional(bit_matrix.matvec_simd_min_words)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .bit_matrix
                    .matvec_simd_min_words,
            ),
            section_optional(bit_matrix.transpose_simple_max_blocks)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .bit_matrix
                    .transpose_simple_max_blocks,
            ),
            section_optional(bit_matrix.transpose_macro_tile_blocks)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .bit_matrix
                    .transpose_macro_tile_blocks,
            ),
        )
        .map_err(core_section_error)?;
        let soa_batch = SoaBatchSelectors::try_new(
            section_optional(soa_batch.parallel_min_len)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .soa_batch
                    .parallel_min_len,
            ),
            section_optional(soa_batch.parallel_chunk_len)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .soa_batch
                    .parallel_chunk_len,
            ),
        )
        .map_err(core_section_error)?;
        let m4rm = M4rmSelectors::try_new(
            section_optional(m4rm.wide_tier_min_stride_words)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .m4rm
                    .wide_tier_min_stride_words,
            ),
            section_optional(m4rm.tiled_min_stride_words)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .m4rm
                    .tiled_min_stride_words,
            ),
            section_optional(m4rm.default_table_bytes)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.m4rm.default_table_bytes),
            section_optional(m4rm.mid_table_bytes)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.m4rm.mid_table_bytes),
            section_optional(m4rm.wide_table_bytes)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.m4rm.wide_table_bytes),
            section_optional(m4rm.wide_max_k)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.m4rm.wide_max_k),
            section_optional(m4rm.small_n_max_k)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.m4rm.small_n_max_k),
        )
        .map_err(core_section_error)?;
        let dense_inverse = DenseInverseSelectors::try_new(
            section_optional(dense_inverse.m4ri_min_dim)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .dense_inverse
                    .m4ri_min_dim,
            ),
            section_optional(dense_inverse.blocked_min_dim)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .dense_inverse
                    .blocked_min_dim,
            ),
        )
        .map_err(core_section_error)?;
        let triangular = TriangularSelectors::try_new(
            section_optional(triangular.trsm_blocked_min_dim)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .triangular
                    .trsm_blocked_min_dim,
            ),
            section_optional(triangular.trsm_panel_rows)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .triangular
                    .trsm_panel_rows,
            ),
            section_optional(triangular.base_case_max_dim)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .triangular
                    .base_case_max_dim,
            ),
        )
        .map_err(core_section_error)?;
        let ple = PleSelectors::try_new(
            section_optional(ple.panel_base_max_cols)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.ple.panel_base_max_cols),
            section_optional(ple.blocked_back_sub_min_dim)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .ple
                    .blocked_back_sub_min_dim,
            ),
            section_optional(ple.scalar_base_max_cols)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.ple.scalar_base_max_cols),
            section_optional(ple.panel_byte_lane_max_cols)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .ple
                    .panel_byte_lane_max_cols,
            ),
            section_optional(ple.panel_u16_lane_max_cols)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .ple
                    .panel_u16_lane_max_cols,
            ),
        )
        .map_err(core_section_error)?;
        let gemm = GemmSelectors::try_new(
            section_optional(gemm.row_tile)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.gemm.row_tile),
            section_optional(gemm.col_tile)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.gemm.col_tile),
            section_optional(gemm.axpy_fast_path_min_volume)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .gemm
                    .axpy_fast_path_min_volume,
            ),
            section_optional(gemm.winograd_min_dim)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.gemm.winograd_min_dim),
        )
        .map_err(core_section_error)?;
        let field_vec = FieldVecSelectors::try_new(
            section_optional(field_vec.dot_chunk_len)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.field_vec.dot_chunk_len),
        )
        .map_err(core_section_error)?;
        let charpoly = CharpolySelectors::try_new(
            section_optional(charpoly.keller_gehrig_min_dim)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .charpoly
                    .keller_gehrig_min_dim,
            ),
        )
        .map_err(core_section_error)?;
        let polynomial = PolynomialSelectors::try_new(
            section_optional(polynomial.karatsuba_min_degree)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .polynomial
                    .karatsuba_min_degree,
            ),
            section_optional(polynomial.karatsuba_max_out_len)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .polynomial
                    .karatsuba_max_out_len,
            ),
            section_optional(polynomial.div_rem_fast_min_len)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .polynomial
                    .div_rem_fast_min_len,
            ),
            section_optional(polynomial.subproduct_min_len)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .polynomial
                    .subproduct_min_len,
            ),
            section_optional(polynomial.interpolate_fast_min_points)?.unwrap_or(
                CoreTuning::CONSERVATIVE
                    .selectors
                    .polynomial
                    .interpolate_fast_min_points,
            ),
        )
        .map_err(core_section_error)?;
        let prime_route = PrimeRouteSelectors::try_new(
            section_optional(prime_route.f32_min_prime)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.prime_route.f32_min_prime),
            section_optional(prime_route.f32_min_cols)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.prime_route.f32_min_cols),
            section_optional(prime_route.f64_min_cols)?
                .unwrap_or(CoreTuning::CONSERVATIVE.selectors.prime_route.f64_min_cols),
        )
        .map_err(core_section_error)?;
        let selectors = CoreSelectors {
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
        };
        Ok(CoreTuning {
            selectors,
            presence,
        })
    }

    fn encode_body(section: &CoreTuning) -> Result<CanonicalValue, SectionError> {
        let mut families = std::collections::BTreeMap::<
            &'static str,
            std::collections::BTreeMap<&'static str, usize>,
        >::new();
        encode_core_body(section, &mut families);
        CanonicalValue::serialize(&families)
    }
}

#[cfg(feature = "tuning-profile")]
fn section_optional<T>(value: Present<T>) -> Result<Option<T>, SectionError> {
    match value {
        Present::Missing => Ok(None),
        Present::Value(Some(value)) => Ok(Some(value)),
        Present::Value(None) => Err(SectionError::InvalidBody(
            "selector fields cannot be null".to_owned(),
        )),
    }
}

#[cfg(feature = "tuning-profile")]
fn core_section_error(error: ProfileError) -> SectionError {
    match error {
        ProfileError::SelectorOutOfRange {
            family,
            field,
            value,
        } => SectionError::SelectorOutOfRange {
            family: family.as_str(),
            field: field.as_str(),
            value,
        },
        error => SectionError::InvalidBody(error.to_string()),
    }
}

#[cfg(feature = "tuning-profile")]
fn core_presence(selectors: &JsonSelectors) -> CorePresence {
    let mut presence = CorePresence {
        families: 0,
        fields: 0,
    };
    macro_rules! family {
        ($member:ident, $family_bit:expr, [$($field:ident => $field_bit:expr),* $(,)?]) => {
            if let Present::Value(value) = &selectors.$member {
                presence.families |= $family_bit;
                if let Some(value) = value {
                    $(if matches!(value.$field, Present::Value(Some(_))) {
                        presence.fields |= $field_bit;
                    })*
                }
            }
        };
    }
    family!(bit_backend, 1 << 0, [simd_min_words => 1 << 0]);
    family!(
        bit_matrix,
        1 << 1,
        [
            matvec_simd_min_words => 1 << 1,
            transpose_simple_max_blocks => 1 << 2,
            transpose_macro_tile_blocks => 1 << 3,
        ]
    );
    family!(
        soa_batch,
        1 << 2,
        [parallel_min_len => 1 << 4, parallel_chunk_len => 1 << 5]
    );
    family!(
        m4rm,
        1 << 3,
        [
            wide_tier_min_stride_words => 1 << 6,
            tiled_min_stride_words => 1 << 7,
            default_table_bytes => 1 << 8,
            mid_table_bytes => 1 << 9,
            wide_table_bytes => 1 << 10,
            wide_max_k => 1 << 11,
            small_n_max_k => 1 << 12,
        ]
    );
    family!(
        dense_inverse,
        1 << 4,
        [m4ri_min_dim => 1 << 13, blocked_min_dim => 1 << 14]
    );
    family!(
        triangular,
        1 << 5,
        [
            trsm_blocked_min_dim => 1 << 15,
            trsm_panel_rows => 1 << 16,
            base_case_max_dim => 1 << 17,
        ]
    );
    family!(
        ple,
        1 << 6,
        [
            panel_base_max_cols => 1 << 18,
            blocked_back_sub_min_dim => 1 << 19,
            scalar_base_max_cols => 1 << 20,
            panel_byte_lane_max_cols => 1 << 21,
            panel_u16_lane_max_cols => 1 << 22,
        ]
    );
    family!(
        gemm,
        1 << 7,
        [
            row_tile => 1 << 23,
            col_tile => 1 << 24,
            axpy_fast_path_min_volume => 1 << 25,
            winograd_min_dim => 1 << 26,
        ]
    );
    family!(field_vec, 1 << 8, [dot_chunk_len => 1 << 27]);
    family!(charpoly, 1 << 9, [keller_gehrig_min_dim => 1 << 28]);
    family!(
        polynomial,
        1 << 10,
        [
            karatsuba_min_degree => 1 << 29,
            karatsuba_max_out_len => 1 << 30,
            div_rem_fast_min_len => 1 << 31,
            subproduct_min_len => 1 << 32,
            interpolate_fast_min_points => 1 << 33,
        ]
    );
    family!(
        prime_route,
        1 << 11,
        [
            f32_min_prime => 1 << 34,
            f32_min_cols => 1 << 35,
            f64_min_cols => 1 << 36,
        ]
    );
    presence
}

#[cfg(feature = "tuning-profile")]
fn encode_core_body(
    section: &CoreTuning,
    families: &mut std::collections::BTreeMap<
        &'static str,
        std::collections::BTreeMap<&'static str, usize>,
    >,
) {
    let presence = section.presence;
    let selectors = &section.selectors;
    macro_rules! family {
        ($name:literal, $family_bit:expr, [$($field_name:literal => $field_bit:expr => $value:expr),* $(,)?]) => {
            if presence.family($family_bit) {
                let mut values = std::collections::BTreeMap::new();
                $(if presence.field($field_bit) {
                    values.insert($field_name, $value);
                })*
                families.insert($name, values);
            }
        };
    }
    family!(
        "bit_backend",
        1 << 0,
        ["simd_min_words" => 1 << 0 => selectors.bit_backend.simd_min_words]
    );
    family!(
        "bit_matrix",
        1 << 1,
        [
            "matvec_simd_min_words" => 1 << 1 => selectors.bit_matrix.matvec_simd_min_words,
            "transpose_simple_max_blocks" => 1 << 2 => selectors.bit_matrix.transpose_simple_max_blocks,
            "transpose_macro_tile_blocks" => 1 << 3 => selectors.bit_matrix.transpose_macro_tile_blocks,
        ]
    );
    family!(
        "soa_batch",
        1 << 2,
        [
            "parallel_min_len" => 1 << 4 => selectors.soa_batch.parallel_min_len,
            "parallel_chunk_len" => 1 << 5 => selectors.soa_batch.parallel_chunk_len,
        ]
    );
    family!(
        "m4rm",
        1 << 3,
        [
            "wide_tier_min_stride_words" => 1 << 6 => selectors.m4rm.wide_tier_min_stride_words,
            "tiled_min_stride_words" => 1 << 7 => selectors.m4rm.tiled_min_stride_words,
            "default_table_bytes" => 1 << 8 => selectors.m4rm.default_table_bytes,
            "mid_table_bytes" => 1 << 9 => selectors.m4rm.mid_table_bytes,
            "wide_table_bytes" => 1 << 10 => selectors.m4rm.wide_table_bytes,
            "wide_max_k" => 1 << 11 => selectors.m4rm.wide_max_k,
            "small_n_max_k" => 1 << 12 => selectors.m4rm.small_n_max_k,
        ]
    );
    family!(
        "dense_inverse",
        1 << 4,
        [
            "m4ri_min_dim" => 1 << 13 => selectors.dense_inverse.m4ri_min_dim,
            "blocked_min_dim" => 1 << 14 => selectors.dense_inverse.blocked_min_dim,
        ]
    );
    family!(
        "triangular",
        1 << 5,
        [
            "trsm_blocked_min_dim" => 1 << 15 => selectors.triangular.trsm_blocked_min_dim,
            "trsm_panel_rows" => 1 << 16 => selectors.triangular.trsm_panel_rows,
            "base_case_max_dim" => 1 << 17 => selectors.triangular.base_case_max_dim,
        ]
    );
    family!(
        "ple",
        1 << 6,
        [
            "panel_base_max_cols" => 1 << 18 => selectors.ple.panel_base_max_cols,
            "blocked_back_sub_min_dim" => 1 << 19 => selectors.ple.blocked_back_sub_min_dim,
            "scalar_base_max_cols" => 1 << 20 => selectors.ple.scalar_base_max_cols,
            "panel_byte_lane_max_cols" => 1 << 21 => selectors.ple.panel_byte_lane_max_cols,
            "panel_u16_lane_max_cols" => 1 << 22 => selectors.ple.panel_u16_lane_max_cols,
        ]
    );
    family!(
        "gemm",
        1 << 7,
        [
            "row_tile" => 1 << 23 => selectors.gemm.row_tile,
            "col_tile" => 1 << 24 => selectors.gemm.col_tile,
            "axpy_fast_path_min_volume" => 1 << 25 => selectors.gemm.axpy_fast_path_min_volume,
            "winograd_min_dim" => 1 << 26 => selectors.gemm.winograd_min_dim,
        ]
    );
    family!(
        "field_vec",
        1 << 8,
        ["dot_chunk_len" => 1 << 27 => selectors.field_vec.dot_chunk_len]
    );
    family!(
        "charpoly",
        1 << 9,
        ["keller_gehrig_min_dim" => 1 << 28 => selectors.charpoly.keller_gehrig_min_dim]
    );
    family!(
        "polynomial",
        1 << 10,
        [
            "karatsuba_min_degree" => 1 << 29 => selectors.polynomial.karatsuba_min_degree,
            "karatsuba_max_out_len" => 1 << 30 => selectors.polynomial.karatsuba_max_out_len,
            "div_rem_fast_min_len" => 1 << 31 => selectors.polynomial.div_rem_fast_min_len,
            "subproduct_min_len" => 1 << 32 => selectors.polynomial.subproduct_min_len,
            "interpolate_fast_min_points" => 1 << 33 => selectors.polynomial.interpolate_fast_min_points,
        ]
    );
    family!(
        "prime_route",
        1 << 11,
        [
            "f32_min_prime" => 1 << 34 => selectors.prime_route.f32_min_prime,
            "f32_min_cols" => 1 << 35 => selectors.prime_route.f32_min_cols,
            "f64_min_cols" => 1 << 36 => selectors.prime_route.f64_min_cols,
        ]
    );
}

#[derive(Clone, Debug)]
enum ProfileText {
    Owned(String),
}

impl ProfileText {
    fn as_str(&self) -> &str {
        match self {
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

fn out_of_range(family: ProfileFamily, field: ProfileField, value: impl Into<u64>) -> ProfileError {
    ProfileError::SelectorOutOfRange {
        family,
        field,
        value: value.into(),
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
impl<'de, T> Deserialize<'de> for Present<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Self::Value)
    }
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonSelectors {
    bit_backend: Present<JsonBitBackend>,
    bit_matrix: Present<JsonBitMatrix>,
    soa_batch: Present<JsonSoaBatch>,
    m4rm: Present<JsonM4rm>,
    dense_inverse: Present<JsonDenseInverse>,
    triangular: Present<JsonTriangular>,
    ple: Present<JsonPle>,
    gemm: Present<JsonGemm>,
    field_vec: Present<JsonFieldVec>,
    charpoly: Present<JsonCharpoly>,
    polynomial: Present<JsonPolynomial>,
    prime_route: Present<JsonPrimeRoute>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonBitBackend {
    simd_min_words: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonBitMatrix {
    matvec_simd_min_words: Present<usize>,
    transpose_simple_max_blocks: Present<usize>,
    transpose_macro_tile_blocks: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonSoaBatch {
    parallel_min_len: Present<usize>,
    parallel_chunk_len: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonM4rm {
    wide_tier_min_stride_words: Present<usize>,
    tiled_min_stride_words: Present<usize>,
    default_table_bytes: Present<usize>,
    mid_table_bytes: Present<usize>,
    wide_table_bytes: Present<usize>,
    wide_max_k: Present<usize>,
    small_n_max_k: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonDenseInverse {
    m4ri_min_dim: Present<usize>,
    blocked_min_dim: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonTriangular {
    trsm_blocked_min_dim: Present<usize>,
    trsm_panel_rows: Present<usize>,
    base_case_max_dim: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonPle {
    panel_base_max_cols: Present<usize>,
    blocked_back_sub_min_dim: Present<usize>,
    scalar_base_max_cols: Present<usize>,
    panel_byte_lane_max_cols: Present<usize>,
    panel_u16_lane_max_cols: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonGemm {
    row_tile: Present<usize>,
    col_tile: Present<usize>,
    axpy_fast_path_min_volume: Present<usize>,
    winograd_min_dim: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonFieldVec {
    dot_chunk_len: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonCharpoly {
    keller_gehrig_min_dim: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonPolynomial {
    karatsuba_min_degree: Present<usize>,
    karatsuba_max_out_len: Present<usize>,
    div_rem_fast_min_len: Present<usize>,
    subproduct_min_len: Present<usize>,
    interpolate_fast_min_points: Present<usize>,
}

#[cfg(feature = "tuning-profile")]
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct JsonPrimeRoute {
    f32_min_prime: Present<usize>,
    f32_min_cols: Present<usize>,
    f64_min_cols: Present<usize>,
}
