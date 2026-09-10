//! Crate-owned calibration of retained core thresholds and execution extents.
//!
//! The neutral campaign driver owns scheduling, durable logging, checkpointing,
//! and the outer host reservation. This producer owns typed cases, fixtures,
//! installed core-only envelopes, scalar witnesses, effective execution checks,
//! and `tuning-calibration-v4` artifact decisions. Only fresh child operations
//! install tuning. Owner reporting, validation, analysis, and emission never do.
//!
//! The exact experiment is declared in
//! `dev/active/a83583e0/premeasurement-protocol.md`, including its cumulative
//! reference to the immutable retained threshold protocol. `--owner-operation`
//! accepts one canonical neutral request on stdin and emits one framed response.
//! The reporting flags perform no measurement or artifact publication.
use std::env;
use std::fmt;
use std::fs;
use std::hint::black_box;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gf2_core::alg::gauss::{invert, invert_route, InvertRoute};
use gf2_core::alg::m4rm::{m4rm_schedule_route, multiply as m4rm_multiply, M4rmScheduleTier};
use gf2_core::alg::m4rm::{
    m4rm_tiled_effective_observation, reset_m4rm_tiled_effective_observation,
    M4rmTiledEffectiveObservation,
};
use gf2_core::compute::field::{
    last_effective_soa_chunk, reset_last_effective_soa_chunk, run_in_dedicated_parallel_pool,
};
use gf2_core::compute::field::{soa_parallel_route, SoaParallelRoute};
use gf2_core::field::inverse::{inv_route, InvRoute};
use gf2_core::field::matrix::{
    gemm_axpy_route, run_gemm_axpy_dispatch_for_test, FieldMatrix, GemmAxpyRoute,
};
use gf2_core::field::matrix::{last_gemm_axpy_dispatch_route, reset_last_gemm_axpy_dispatch_route};
use gf2_core::field::ple::{back_sub_route, ple_panel_route, BackSubRoute, PlePanelRoute};
use gf2_core::field::ple::{
    max_effective_panel_dispatch_cols, reset_max_effective_panel_dispatch_cols,
};
use gf2_core::field::poly::{
    batch_evaluate_auto_route, div_rem_auto_route, mul_fast, mul_fast_route, mul_route,
    BatchEvaluateRoute, DivRemAutoRoute, FieldPoly, MulFastRoute, MulRoute,
};
use gf2_core::field::poly_interpolate::{
    interpolate_auto, interpolate_auto_two_adic, interpolate_route, InterpolateRoute,
};
use gf2_core::field::triangular::{
    last_effective_trsm_panel_rows, reset_last_effective_trsm_panel_rows,
};
use gf2_core::field::triangular::{trsm_route, TrsmRoute};
use gf2_core::field::{FiniteField, PlePanelLane};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{BatchExtField, ExtConfig};
use gf2_core::kernels::{Backend, ScalarBackend};
use gf2_core::matrix::{transpose_route, TransposeRoute};
use gf2_core::rng::Lcg;
use gf2_core::tuning;
use gf2_core::tuning::HarnessSchema;
use gf2_core::tuning::{
    AssemblyProvenance, BitBackendSelectors, BitMatrixSelectors, CanonicalValue,
    CompiledProfileProvenance, CoreSelectors, CoreTuning, CoreTuningCodec, DenseInverseSelectors,
    GemmSelectors, GitRevision, M4rmSelectors, MeasurementProvenance, PleSelectors,
    PolynomialSelectors, PreparedEnvelope, ProfileId, ProfileRegistry, ProfileRegistryBuilder,
    RepoRelPath, Rfc3339Utc, SectionCodec, Sha256, SoaBatchSelectors, TriangularSelectors,
    TuningSection, PROFILE_FORMAT_VERSION,
};
use gf2_core::BitMatrix;
use sha2::{Digest, Sha256 as Sha256Hasher};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProducedCoreProfile {
    id: ProfileId,
    measurement: MeasurementProvenance,
    assembly: AssemblyProvenance,
    section: CoreTuning,
}

impl std::ops::Deref for ProducedCoreProfile {
    type Target = CoreTuning;

    fn deref(&self) -> &Self::Target {
        &self.section
    }
}

impl ProducedCoreProfile {
    fn to_json(&self) -> String {
        let prepared = PreparedEnvelope::compiled(
            self.id.clone(),
            CompiledProfileProvenance {
                artifact_id: self.id.clone(),
            },
        )
        .insert_measured::<CoreTuning, CoreTuningCodec>(
            self.section.clone(),
            self.measurement.clone(),
        )
        .expect("calibration evidence was validated when collected")
        .build()
        .expect("one typed section builds a prepared envelope");
        core_registry()
            .expect("the core owner registry has one valid codec")
            .to_json(&prepared, &self.assembly)
            .expect("validated calibration output encodes")
    }

    fn from_json(document: &str) -> Result<Self, String> {
        let prepared = core_registry()
            .map_err(|error| format!("core registry is invalid: {error}"))?
            .from_json(document)
            .map_err(|error| format!("core owner envelope is invalid: {error}"))?;
        let ids: Vec<&str> = prepared.section_ids().collect();
        if ids != ["gf2-core/selectors"] {
            return Err(format!("core owner envelope has section IDs {ids:?}"));
        }
        let projection = prepared
            .section::<CoreTuning>()
            .map_err(|error| format!("typed core projection failed: {error}"))?
            .ok_or("core owner envelope is missing its section")?;
        let assembly = prepared
            .verified_assembly()
            .ok_or("canonical core owner lacks verified assembly")?
            .provenance
            .clone();
        Ok(Self {
            id: prepared.profile_id().clone(),
            measurement: projection.measurement.clone(),
            assembly,
            section: projection.section.clone(),
        })
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn omitting(&self, omitted: &[SchemaField]) -> Result<Self, String> {
        let body = CoreTuningCodec::encode_body(&self.section)
            .map_err(|error| format!("complete core section does not encode: {error}"))?;
        let mut selectors = serde_json::to_value(body)
            .map_err(|error| format!("complete core section is not JSON: {error}"))?;
        for field in omitted {
            let family = selectors
                .get_mut(&field.family)
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| format!("complete core section has no `{}` family", field.family))?;
            if family.remove(&field.name).is_none() {
                return Err(format!("complete core section has no `{field}` field"));
            }
        }
        let canonical = CanonicalValue::serialize(&selectors)
            .map_err(|error| format!("omitted selector body is not canonical: {error}"))?;
        let section = CoreTuningCodec::decode_body(canonical)
            .map_err(|error| format!("omitted core section is invalid: {error}"))?;
        Ok(Self {
            section,
            ..self.clone()
        })
    }
}

fn core_registry() -> Result<ProfileRegistry, gf2_core::tuning::RegistryError> {
    ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()?
        .build()
}

#[allow(dead_code)]
fn complete_selector_value(section: &CoreTuning) -> Result<serde_json::Value, String> {
    let complete = CoreTuning::from_selectors(section.selectors().clone());
    let body = CoreTuningCodec::encode_body(&complete)
        .map_err(|error| format!("complete selector view does not encode: {error}"))?;
    serde_json::to_value(body)
        .map_err(|error| format!("complete selector view is not JSON: {error}"))
}

/// Prepared-host marker required before this action measures or emits.
const BENCH_MODE_VAR: &str = "GF2_BENCH";
const RAYON_THREADS_VAR: &str = "RAYON_NUM_THREADS";
const REQUIRED_RAYON_THREADS: &str = "4";
const REQUIRED_FEATURES: &str = tuning_campaign_support::campaign::FEATURE_CONTRACT;
const REQUIRED_SOA_PARALLEL_CHUNK_LEN: usize = 16_384;
const REQUIRED_PLE_BYTE_LANE_MAX_COLS: usize = 256;
const REQUIRED_TRSM_PANEL_ROWS: usize = 64;
/// Private guard for every forced tuning child.
const FRESH_CASE_VAR: &str = tuning_campaign_support::transport::FRESH_CASE_VAR;
const FRESH_CASE_VALUE: &str = tuning_campaign_support::transport::FRESH_CASE_VALUE;
const FRESH_RESULT_PREFIX: &str = tuning_campaign_support::transport::FRESH_RESULT_PREFIX;
const CHILD_OBSERVATION_PREFIX: &str = "GF2_TUNING_CHILD_OBSERVATION=";
#[cfg(test)]
#[allow(dead_code)]
const INTERPOLATION_RECONCILIATION_PREFIX: &str = "GF2_TUNING_INTERPOLATION=";
#[cfg(test)]
#[allow(dead_code)]
const SAMPLE_PREFIX: &str = "GF2_TUNING_SAMPLES=";
#[cfg(test)]
#[allow(dead_code)]
const SEED_PREFIX: &str = "GF2_TUNING_SEEDS=";
const RAW_SAMPLE_SCHEMA: &str = "raw-timing-samples-v3";
const SEED_SCHEMA: &str = "fixture-seeds-v2";
const SEED_DERIVATION: &str = "gf2-calibration-seed-v1";
/// Wrapper-overridable mutex path, read only to explain a failed lock probe.
#[cfg(test)]
#[allow(dead_code)]
const LOCK_PATH_VAR: &str = "GF2_CCX1_LOCK";
#[cfg(test)]
#[allow(dead_code)]
const DEFAULT_LOCK_PATH: &str = "/tmp/gf2-ccx1.lock";
const REQUIRED_RUSTUP_TOOLCHAIN: &str = "1.95.0";
const DEFAULT_EXECUTIONS: u64 = tuning_campaign_support::timing::EXECUTIONS;
const DEFAULT_REPETITIONS: u64 = tuning_campaign_support::timing::WINDOWS;
const DEFAULT_TARGET_MS: u64 = tuning_campaign_support::timing::TARGET.as_millis() as u64;
#[cfg(test)]
#[allow(dead_code)]
const EXPECTED_MEASURED_FIELDS: usize = 16;
const EXPECTED_CORE_SCHEMA_FIELDS: usize = 37;
#[cfg(test)]
#[allow(dead_code)]
const EXPECTED_OMITTED_FIELDS: usize = 21;
#[cfg(test)]
#[allow(dead_code)]
const EXPECTED_GRID_ARM_CELLS: usize = 306;
#[cfg(test)]
#[allow(dead_code)]
const EXPECTED_PROBE_CHILDREN: usize = 306;
#[cfg(test)]
#[allow(dead_code)]
const EXPECTED_TIMED_CHILDREN: usize = 1_530;
#[cfg(test)]
#[allow(dead_code)]
const EXPECTED_FRESH_CHILDREN: usize = 1_836;
#[cfg(test)]
#[allow(dead_code)]
const EXPECTED_RAW_WINDOWS: usize = 7_650;
/// Upper bound on the calibrated call count of one timed window.
const MAX_CALLS: u64 = tuning_campaign_support::timing::MAX_CALLS;
/// Fixture bank depth for the bit-backend arms, matching the sibling harness.
const BIT_FIXTURES: usize = tuning_campaign_support::timing::FIXTURE_BANKS;
/// `u64` words per 64-byte cache line on the supported targets.
const WORDS_PER_LINE: usize = 8;
const SEED_ROOT: u64 = tuning_campaign_support::seed::EXTENT_SEED_ROOT;
const SEED_ROLE_BIT_DST: u64 = 0xD000_0000;
const SEED_ROLE_BIT_SRC: u64 = 0xA000_0000;
const SEED_ROLE_POLY_LHS: u64 = 0xA;
const SEED_ROLE_POLY_RHS: u64 = 0xB;
const SEED_ROLE_DIVIDEND: u64 = 0xD;
const SEED_ROLE_DIVISOR: u64 = 0xE;
const SEED_ROLE_EVAL_POLY: u64 = 0xC;
const SEED_ROLE_EVAL_POINTS: u64 = 0xF;
const FOLLOW_ON_ROLES: &[(CalibratedField, &[(&str, u64)])] = &[
    (
        CalibratedField::TransposeSimpleMaxBlocks,
        &[("matrix", 0x100)],
    ),
    (
        CalibratedField::SoaParallelMinLen,
        &[
            ("quadratic_lhs", 0x200),
            ("quadratic_rhs", 0x201),
            ("cubic_lhs", 0x202),
            ("cubic_rhs", 0x203),
        ],
    ),
    (
        CalibratedField::M4rmWideTierMinStrideWords,
        &[("lhs", 0x300), ("rhs", 0x301)],
    ),
    (
        CalibratedField::M4rmTiledMinStrideWords,
        &[("lhs", 0x310), ("rhs", 0x311)],
    ),
    (
        CalibratedField::DenseInverseM4riMinDim,
        &[("unit_lower", 0x400), ("unit_upper", 0x401)],
    ),
    (
        CalibratedField::DenseInverseBlockedMinDim,
        &[("unit_lower", 0x500), ("unit_upper", 0x501)],
    ),
    (
        CalibratedField::TrsmBlockedMinDim,
        &[("unit_lower", 0x600), ("unit_upper", 0x601), ("rhs", 0x602)],
    ),
    (
        CalibratedField::PlePanelBaseMaxCols,
        &[("unit_lower", 0x700), ("unit_upper", 0x701)],
    ),
    (
        CalibratedField::PleBlockedBackSubMinDim,
        &[
            ("designated_nonzero", 0x800),
            ("non_designated", 0x801),
            ("row_mix", 0x802),
        ],
    ),
    (
        CalibratedField::GemmAxpyFastPathMinVolume,
        &[("lhs", 0x900), ("rhs", 0x901)],
    ),
    (
        CalibratedField::InterpolateFastMinPoints,
        &[("coefficients", 0xa00), ("point_offset", 0xa01)],
    ),
];
#[cfg(test)]
#[allow(dead_code)]
const GIT_STATUS_ARGS: &[&str] = &["status", "--porcelain", "--untracked-files=all"];
/// `polynomial.karatsuba_min_degree` a child installs to force the schoolbook
/// arm.
///
/// `usize::MAX` is the top of the field's admissible range and means "never
/// take the named path"; no operand degree reaches it, so every product on the
/// grid runs schoolbook. It is an ordinary admissible value rather than a
/// reserved sentinel.
const FORCED_SCHOOLBOOK_MIN_DEGREE: usize = usize::MAX;
/// Profile identifier a child installs, recorded nowhere but its own process.
const FORCED_ARM_PROFILE_ID: &str = "calibration-forced-arm";

/// The prime field every polynomial arm is measured over.
///
/// `Fp<65537>` is `TwoAdicField`, which the NTT, Newton-iteration division and
/// subproduct-tree arms all require, and it is the field the pinned selector
/// non-regression set already measures, so the two receipts describe the same
/// arithmetic.
type F = Fp<65537>;

// ---------------------------------------------------------------------
// Calibrated fields and their grids
// ---------------------------------------------------------------------

/// The sixteen selector fields this sweep measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum CalibratedField {
    SimdMinWords,
    KaratsubaMinDegree,
    KaratsubaMaxOutLen,
    DivRemFastMinLen,
    SubproductMinLen,
    TransposeSimpleMaxBlocks,
    SoaParallelMinLen,
    M4rmWideTierMinStrideWords,
    M4rmTiledMinStrideWords,
    DenseInverseM4riMinDim,
    DenseInverseBlockedMinDim,
    TrsmBlockedMinDim,
    PlePanelBaseMaxCols,
    PleBlockedBackSubMinDim,
    GemmAxpyFastPathMinVolume,
    InterpolateFastMinPoints,
}

impl CalibratedField {
    const ALL: [Self; 16] = [
        Self::SimdMinWords,
        Self::KaratsubaMinDegree,
        Self::KaratsubaMaxOutLen,
        Self::DivRemFastMinLen,
        Self::SubproductMinLen,
        Self::TransposeSimpleMaxBlocks,
        Self::SoaParallelMinLen,
        Self::M4rmWideTierMinStrideWords,
        Self::M4rmTiledMinStrideWords,
        Self::DenseInverseM4riMinDim,
        Self::DenseInverseBlockedMinDim,
        Self::TrsmBlockedMinDim,
        Self::PlePanelBaseMaxCols,
        Self::PleBlockedBackSubMinDim,
        Self::GemmAxpyFastPathMinVolume,
        Self::InterpolateFastMinPoints,
    ];

    /// Stable input to [`seed_for`], independent of enum declaration order.
    ///
    /// Stable tags make every field's stream independent of enum declaration
    /// order, so adding or reordering a variant cannot change a fixture.
    fn seed_tag(self) -> u64 {
        match self {
            Self::SimdMinWords => 0,
            Self::KaratsubaMinDegree => 1,
            Self::KaratsubaMaxOutLen => 2,
            Self::DivRemFastMinLen => 3,
            Self::SubproductMinLen => 4,
            Self::TransposeSimpleMaxBlocks => 5,
            Self::SoaParallelMinLen => 6,
            Self::M4rmWideTierMinStrideWords => 7,
            Self::M4rmTiledMinStrideWords => 8,
            Self::DenseInverseM4riMinDim => 9,
            Self::DenseInverseBlockedMinDim => 10,
            Self::TrsmBlockedMinDim => 11,
            Self::PlePanelBaseMaxCols => 12,
            Self::PleBlockedBackSubMinDim => 13,
            Self::GemmAxpyFastPathMinVolume => 14,
            Self::InterpolateFastMinPoints => 15,
        }
    }

    fn family(self) -> &'static str {
        match self {
            Self::SimdMinWords => "bit_backend",
            Self::KaratsubaMinDegree
            | Self::KaratsubaMaxOutLen
            | Self::DivRemFastMinLen
            | Self::SubproductMinLen
            | Self::InterpolateFastMinPoints => "polynomial",
            Self::TransposeSimpleMaxBlocks => "bit_matrix",
            Self::SoaParallelMinLen => "soa_batch",
            Self::M4rmWideTierMinStrideWords | Self::M4rmTiledMinStrideWords => "m4rm",
            Self::DenseInverseM4riMinDim | Self::DenseInverseBlockedMinDim => "dense_inverse",
            Self::TrsmBlockedMinDim => "triangular",
            Self::PlePanelBaseMaxCols | Self::PleBlockedBackSubMinDim => "ple",
            Self::GemmAxpyFastPathMinVolume => "gemm",
        }
    }

    /// The schema key this field's measured value is stated under.
    fn schema_field(self) -> SchemaField {
        SchemaField {
            family: self.family().to_owned(),
            name: self.to_string(),
        }
    }

    /// Whether the field bounds its conservative arm from above.
    ///
    /// `karatsuba_max_out_len` gates `out_len <= t` to the Karatsuba arm, so
    /// its value is the largest grid point below the crossover. Every other
    /// field gates `size >= t` to the asymptotic arm, so its value is the
    /// crossover itself.
    fn is_upper_bound(self) -> bool {
        matches!(
            self,
            Self::KaratsubaMaxOutLen | Self::TransposeSimpleMaxBlocks | Self::PlePanelBaseMaxCols
        )
    }

    fn upper_bound_floor(self) -> usize {
        if self == Self::PlePanelBaseMaxCols {
            1
        } else {
            0
        }
    }

    /// The conservative table's value for this field.
    fn conservative_default(self) -> usize {
        let profile = &CoreTuning::CONSERVATIVE;
        match self {
            Self::SimdMinWords => profile.bit_backend().simd_min_words(),
            Self::KaratsubaMinDegree => profile.polynomial().karatsuba_min_degree(),
            Self::KaratsubaMaxOutLen => profile.polynomial().karatsuba_max_out_len(),
            Self::DivRemFastMinLen => profile.polynomial().div_rem_fast_min_len(),
            Self::SubproductMinLen => profile.polynomial().subproduct_min_len(),
            Self::TransposeSimpleMaxBlocks => profile.bit_matrix().transpose_simple_max_blocks(),
            Self::SoaParallelMinLen => profile.soa_batch().parallel_min_len(),
            Self::M4rmWideTierMinStrideWords => profile.m4rm().wide_tier_min_stride_words(),
            Self::M4rmTiledMinStrideWords => profile.m4rm().tiled_min_stride_words(),
            Self::DenseInverseM4riMinDim => profile.dense_inverse().m4ri_min_dim(),
            Self::DenseInverseBlockedMinDim => profile.dense_inverse().blocked_min_dim(),
            Self::TrsmBlockedMinDim => profile.triangular().trsm_blocked_min_dim(),
            Self::PlePanelBaseMaxCols => profile.ple().panel_base_max_cols(),
            Self::PleBlockedBackSubMinDim => profile.ple().blocked_back_sub_min_dim(),
            Self::GemmAxpyFastPathMinVolume => profile.gemm().axpy_fast_path_min_volume(),
            Self::InterpolateFastMinPoints => profile.polynomial().interpolate_fast_min_points(),
        }
    }

    fn conservative_arm(self) -> &'static str {
        match self {
            Self::SimdMinWords => "scalar",
            Self::KaratsubaMinDegree => "schoolbook",
            Self::KaratsubaMaxOutLen => "karatsuba",
            Self::DivRemFastMinLen => "div_rem",
            Self::SubproductMinLen => "eval_batch",
            Self::TransposeSimpleMaxBlocks => "simple",
            Self::SoaParallelMinLen => "sequential",
            Self::M4rmWideTierMinStrideWords => "small_n",
            Self::M4rmTiledMinStrideWords => "row_wise",
            Self::DenseInverseM4riMinDim => "scalar",
            Self::DenseInverseBlockedMinDim => "scalar_ple",
            Self::TrsmBlockedMinDim => "recursive",
            Self::PlePanelBaseMaxCols => "panel_base",
            Self::PleBlockedBackSubMinDim => "scalar",
            Self::GemmAxpyFastPathMinVolume => "per_cell",
            Self::InterpolateFastMinPoints => "barycentric",
        }
    }

    fn asymptotic_arm(self) -> &'static str {
        match self {
            Self::SimdMinWords => "simd",
            Self::KaratsubaMinDegree => "karatsuba",
            Self::KaratsubaMaxOutLen => "mul_ntt",
            Self::DivRemFastMinLen => "div_rem_fast",
            Self::SubproductMinLen => "subproduct_auto",
            Self::TransposeSimpleMaxBlocks => "macro_tiled",
            Self::SoaParallelMinLen => "parallel",
            Self::M4rmWideTierMinStrideWords => "wide",
            Self::M4rmTiledMinStrideWords => "register_tiled",
            Self::DenseInverseM4riMinDim => "m4ri",
            Self::DenseInverseBlockedMinDim => "blocked_panelized",
            Self::TrsmBlockedMinDim => "blocked",
            Self::PlePanelBaseMaxCols => "sub_panel_recursion",
            Self::PleBlockedBackSubMinDim => "blocked",
            Self::GemmAxpyFastPathMinVolume => "whole_gemm",
            Self::InterpolateFastMinPoints => "subproduct_tree",
        }
    }

    fn arm_name(self, arm: Arm) -> &'static str {
        match arm {
            Arm::Conservative => self.conservative_arm(),
            Arm::Asymptotic => self.asymptotic_arm(),
        }
    }

    /// The process each of this field's arms is measured in.
    ///
    /// Every probe and timed execution runs in a fresh process. This keeps the
    /// process-global tuning cell out of the parent and gives every arm the
    /// same strict reopen-before-install evidence.
    #[cfg(test)]
    #[allow(dead_code)]
    fn arm_source(self) -> ArmSource {
        let _ = self;
        ArmSource::FreshChild
    }

    /// The unit the grid points are measured in.
    #[cfg(test)]
    #[allow(dead_code)]
    fn grid_unit(self) -> &'static str {
        match self {
            Self::SimdMinWords => "buffer words",
            Self::KaratsubaMinDegree => "operand degree",
            Self::KaratsubaMaxOutLen => "product length",
            Self::DivRemFastMinLen => "divisor length",
            Self::SubproductMinLen => "coefficients = points",
            Self::TransposeSimpleMaxBlocks => "64-row blocks",
            Self::SoaParallelMinLen => "batch elements",
            Self::M4rmWideTierMinStrideWords | Self::M4rmTiledMinStrideWords => "stride words",
            Self::DenseInverseM4riMinDim
            | Self::DenseInverseBlockedMinDim
            | Self::TrsmBlockedMinDim
            | Self::PlePanelBaseMaxCols
            | Self::PleBlockedBackSubMinDim => "matrix dimension",
            Self::GemmAxpyFastPathMinVolume => "matrix volume",
            Self::InterpolateFastMinPoints => "points",
        }
    }

    /// The size grid this field is swept over.
    ///
    /// Ordinary grids straddle `conservative_default`. The M4RM tiled grid is
    /// domain-clipped to begin at its boundary default of four words, where
    /// both arms remain forceable. The complete grids and fixtures are
    /// preregistered in `dev/active/eaae1b56/premeasurement-protocol.md` §3.
    ///
    /// `karatsuba_max_out_len` counts product lengths, and a product of two
    /// equal-length operands has odd length `2n - 1`; its grid therefore uses
    /// the odd lengths bracketing the default. It reaches 511 so the sweep
    /// spans the region where `mul_fast`'s two arms are recorded as converging,
    /// and it carries both 127 and 129 so the step across the default's own
    /// boundary is measured directly.
    ///
    /// The two large-operand fields reach two octaves further below their
    /// default than the small-operand ones, because their conservative arms are
    /// quadratic and a crossover several octaves below the default is the
    /// ordinary case for them rather than a surprise.
    fn grid(self) -> Vec<usize> {
        match self {
            Self::SimdMinWords | Self::DenseInverseM4riMinDim => {
                vec![1, 2, 4, 7, 8, 9, 16, 32, 64]
            }
            Self::KaratsubaMinDegree => vec![4, 8, 16, 31, 32, 33, 64, 128, 256],
            Self::KaratsubaMaxOutLen => vec![15, 31, 63, 127, 129, 191, 255, 383, 511],
            Self::DivRemFastMinLen => {
                vec![64, 128, 256, 512, 1024, 2047, 2048, 2049, 4096]
            }
            Self::SubproductMinLen => {
                vec![128, 256, 512, 1024, 2048, 4095, 4096, 4097, 8192]
            }
            Self::TransposeSimpleMaxBlocks
            | Self::M4rmWideTierMinStrideWords
            | Self::DenseInverseBlockedMinDim
            | Self::InterpolateFastMinPoints => vec![2, 4, 8, 15, 16, 17, 32, 64, 128],
            Self::SoaParallelMinLen => vec![
                4096, 8192, 16_384, 32_767, 32_768, 32_769, 65_536, 131_072, 262_144,
            ],
            Self::M4rmTiledMinStrideWords => vec![4, 5, 6, 8, 12, 16, 24, 32, 64],
            Self::TrsmBlockedMinDim => vec![8, 16, 32, 63, 64, 65, 96, 128, 256],
            Self::PlePanelBaseMaxCols => vec![16, 32, 64, 96, 127, 128, 129, 160, 256],
            Self::PleBlockedBackSubMinDim => {
                vec![16, 32, 64, 96, 127, 128, 129, 192, 256]
            }
            Self::GemmAxpyFastPathMinVolume => {
                vec![64, 512, 1728, 3375, 4096, 4913, 8000, 13_824, 32_768]
            }
        }
    }

    fn variants(self) -> &'static [SweepVariant] {
        if self == Self::InterpolateFastMinPoints {
            &[
                SweepVariant::GenericInterpolation,
                SweepVariant::TwoAdicInterpolation,
            ]
        } else {
            &[SweepVariant::Standard]
        }
    }
}

impl fmt::Display for CalibratedField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SimdMinWords => "simd_min_words",
            Self::KaratsubaMinDegree => "karatsuba_min_degree",
            Self::KaratsubaMaxOutLen => "karatsuba_max_out_len",
            Self::DivRemFastMinLen => "div_rem_fast_min_len",
            Self::SubproductMinLen => "subproduct_min_len",
            Self::TransposeSimpleMaxBlocks => "transpose_simple_max_blocks",
            Self::SoaParallelMinLen => "parallel_min_len",
            Self::M4rmWideTierMinStrideWords => "wide_tier_min_stride_words",
            Self::M4rmTiledMinStrideWords => "tiled_min_stride_words",
            Self::DenseInverseM4riMinDim => "m4ri_min_dim",
            Self::DenseInverseBlockedMinDim => "blocked_min_dim",
            Self::TrsmBlockedMinDim => "trsm_blocked_min_dim",
            Self::PlePanelBaseMaxCols => "panel_base_max_cols",
            Self::PleBlockedBackSubMinDim => "blocked_back_sub_min_dim",
            Self::GemmAxpyFastPathMinVolume => "axpy_fast_path_min_volume",
            Self::InterpolateFastMinPoints => "interpolate_fast_min_points",
        })
    }
}

/// A selector may have more than one production dispatcher implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SweepVariant {
    #[default]
    Standard,
    GenericInterpolation,
    TwoAdicInterpolation,
}

impl fmt::Display for SweepVariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Standard => "standard",
            Self::GenericInterpolation => "generic",
            Self::TwoAdicInterpolation => "two_adic",
        })
    }
}

/// One `selectors.<family>.<field>` key of the emitted schema.
///
/// The schema carries many more of these than this sweep measures, and the two
/// sets are compared by value rather than by a maintained list, so a field the
/// sweep does not name is omitted from the emitted document whatever it is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SchemaField {
    family: String,
    name: String,
}

impl fmt::Display for SchemaField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.family, self.name)
    }
}

/// Which side of a crossover an arm sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Arm {
    /// The arm the comparison selects below the threshold.
    Conservative,
    /// The arm the comparison selects above the threshold.
    Asymptotic,
}

impl Arm {
    const BOTH: [Self; 2] = [Self::Conservative, Self::Asymptotic];
}

impl fmt::Display for Arm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Conservative => "conservative",
            Self::Asymptotic => "asymptotic",
        })
    }
}

/// Where a field's two arms are timed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
#[allow(dead_code)]
enum ArmSource {
    /// Every probe and timed execution owns a new process and installed core
    /// section.
    FreshChild,
}

#[cfg(test)]
impl fmt::Display for ArmSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FreshChild => "fresh-child",
        })
    }
}

// ---------------------------------------------------------------------
// Command line
// ---------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Protocol {
    executions: u64,
    repetitions: u64,
    target_ms: u64,
}

impl Protocol {
    fn target(&self) -> Duration {
        Duration::from_millis(self.target_ms)
    }

    /// Timed windows recorded per arm at one grid point.
    #[cfg(test)]
    #[allow(dead_code)]
    fn windows(&self) -> usize {
        (self.executions * self.repetitions) as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    SelfCheck,
    ListGrid,
    CapabilityReport,
    FreshChild,
    OwnerOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Args {
    protocol: Protocol,
    mode: Mode,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut mode = None;
    for argument in args {
        let selected = match argument.as_str() {
            "--self-check" => Mode::SelfCheck,
            "--list-grid" => Mode::ListGrid,
            "--capability-report" => Mode::CapabilityReport,
            "--fresh-tuning-process-child" => Mode::FreshChild,
            "--owner-operation" => Mode::OwnerOperation,
            "--bench" => continue,
            _ => return Err(format!("unknown argument: {argument}")),
        };
        if mode.replace(selected).is_some() {
            return Err("select exactly one explicit owner mode".to_owned());
        }
    }
    Ok(Args {
        protocol: campaign_owner::fixed_protocol(),
        mode: mode.ok_or("an explicit owner mode is required")?,
    })
}

/// Resolves a path the caller gave on the command line.
///
/// An absolute path is used as given; a relative one is taken as
/// repository-relative, because `cargo bench` runs this binary with its working
/// directory at the package root while the receipts and profiles it names are
/// written relative to the repository root. This matches the resolution the
/// sibling receipt harness uses, so one command works from where the procedure
/// says to run it.
fn resolve_repository_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}

// ---------------------------------------------------------------------
// Child-process arms
// ---------------------------------------------------------------------

/// What a child process is asked to do with the arm it forces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ChildTask {
    /// Report the arm and the digests, and time nothing.
    Probe,
    /// Time one execution's windows as well.
    Measure { execution: u64 },
}

impl fmt::Display for ChildTask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe => formatter.write_str("probe"),
            Self::Measure { execution } => write!(formatter, "{execution}"),
        }
    }
}

/// One forced arm at one grid point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ChildSpec {
    field: CalibratedField,
    #[serde(default)]
    variant: SweepVariant,
    size: usize,
    arm: Arm,
    task: ChildTask,
}

impl fmt::Display for ChildSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}:{}:{}",
            self.field, self.variant, self.size, self.arm, self.task
        )
    }
}

/// Canonical case sent to one guarded forced-tuning child.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FreshProcessCase {
    spec: ChildSpec,
    protocol: Protocol,
}

// ---------------------------------------------------------------------
// Observed host facts
// ---------------------------------------------------------------------

/// Everything the emitted provenance records that this run observes rather than
/// takes from its own protocol constants.
#[derive(Clone, Debug)]
#[cfg(test)]
#[allow(dead_code)]
struct HostFacts {
    source_revision: GitRevision,
    source_dirty: bool,
    harness: RepoRelPath,
    binary_sha256: Sha256,
    toolchain: String,
    host: String,
    cpu_model: String,
    cpu_features: Vec<String>,
    os_kernel: String,
    governor: String,
    lock_file: String,
    cpu_affinity: String,
}

fn sha256_file(path: &Path) -> Result<Sha256, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {} for hashing: {error}", path.display()))?;
    Sha256::parse(&format!("{:x}", Sha256Hasher::digest(bytes)))
        .map_err(|error| format!("cannot represent {} digest: {error}", path.display()))
}

#[cfg(test)]
#[allow(dead_code)]
fn require_unchanged_binary(path: &Path, expected: &Sha256, role: &str) -> Result<(), String> {
    let observed = sha256_file(path)?;
    if &observed != expected {
        return Err(format!(
            "{role} binary changed during the campaign: expected {}, observed {}",
            expected.as_str(),
            observed.as_str()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
fn command_output(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{program} {} failed with {}",
            args.join(" "),
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Repository-relative path of this source file.
///
/// Derived from the compiler's own path for this file and the repository root
/// `git` reports, so the recorded harness identity follows the file if it moves
/// instead of restating a path written into the tool. The compiler's path is
/// relative to whichever directory `cargo` invoked it from, so both the
/// repository root and this package's directory are tried and the candidate
/// that names a real file wins; a path that names neither is an error rather
/// than a provenance claim nobody checked.
#[cfg(test)]
#[allow(dead_code)]
fn harness_path(repo_root: &Path) -> io::Result<String> {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .strip_prefix(repo_root)
        .unwrap_or(Path::new(""));
    let candidates = [
        normalize_relative(Path::new(file!())),
        normalize_relative(&package.join(file!())),
    ];
    candidates
        .iter()
        .find(|candidate| repo_root.join(candidate).is_file())
        .cloned()
        .ok_or_else(|| {
            io::Error::other(format!(
                "no candidate harness path among {candidates:?} names a file under {}",
                repo_root.display()
            ))
        })
}

/// Collapses `a/b/../c` to `a/c` so the result passes `RepoRelPath::parse`.
///
/// The compiler's path for this file gains a parent segment when the
/// `#[cfg(test)]` module below is compiled through the integration-test wrapper
/// that includes this file from `tests/`.
#[cfg(test)]
#[allow(dead_code)]
fn normalize_relative(path: &Path) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.iter().filter_map(|part| part.to_str()) {
        match part {
            "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Runtime-detected CPU feature tokens that gate the accelerated kernels.
///
/// The token list is the set `gf2-kernels-simd` probes; membership of the
/// returned vector is decided by this host's own detection at run time.
fn cpu_features() -> Vec<String> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let detected = [
            ("avx2", std::arch::is_x86_feature_detected!("avx2")),
            ("avx512vl", std::arch::is_x86_feature_detected!("avx512vl")),
            ("fma", std::arch::is_x86_feature_detected!("fma")),
            (
                "pclmulqdq",
                std::arch::is_x86_feature_detected!("pclmulqdq"),
            ),
            ("sse4.1", std::arch::is_x86_feature_detected!("sse4.1")),
            (
                "vpclmulqdq",
                std::arch::is_x86_feature_detected!("vpclmulqdq"),
            ),
        ];
        detected
            .into_iter()
            .filter(|(_, present)| *present)
            .map(|(token, _)| token.to_owned())
            .collect()
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        Vec::new()
    }
}

/// The CPU list this process is allowed to run on, as `taskset` left it.
#[cfg(test)]
#[allow(dead_code)]
fn cpu_affinity() -> io::Result<String> {
    let status = fs::read_to_string("/proc/self/status")?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .map(|value| value.trim().to_owned())
        .ok_or_else(|| io::Error::other("/proc/self/status has no Cpus_allowed_list"))
}

/// The scaling governor of the first CPU this process is pinned to.
#[cfg(test)]
#[allow(dead_code)]
fn governor(affinity: &str) -> io::Result<String> {
    let first = affinity
        .split(',')
        .next()
        .and_then(|range| range.split('-').next())
        .and_then(|cpu| cpu.trim().parse::<u32>().ok())
        .ok_or_else(|| io::Error::other(format!("cannot read a CPU number from `{affinity}`")))?;
    Ok(fs::read_to_string(format!(
        "/sys/devices/system/cpu/cpu{first}/cpufreq/scaling_governor"
    ))?
    .trim()
    .to_owned())
}

/// Whether this process inherited the exclusive `flock` on the expected mutex.
///
/// `dev/scripts/ccx1-bench-flock.sh` takes the lock with `flock -x` and does
/// not pass `-o`, so the locked descriptor stays open across the exec chain and
/// is inherited by every descendant, this binary included. Matching an open
/// descriptor of ours against the expected path's exact device and inode in
/// the kernel lock table therefore establishes that this run is serialised by
/// the repository mutex, rather than by some unrelated inherited lock. The
/// lock's owning PID is the wrapper's, not ours, so the match is on device and
/// inode rather than PID.
#[cfg(test)]
#[allow(dead_code)]
fn observed_lock_file(expected_path: &Path) -> io::Result<Option<String>> {
    let mut locked: Vec<(u64, u64)> = Vec::new();
    for line in fs::read_to_string("/proc/locks")?.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // `1: FLOCK  ADVISORY  WRITE 4711 00:2d:112936 0 EOF`
        if fields.len() < 6 || fields[1] != "FLOCK" || fields[3] != "WRITE" {
            continue;
        }
        let mut parts = fields[5].split(':');
        let major = parts.next().and_then(|v| u64::from_str_radix(v, 16).ok());
        let minor = parts.next().and_then(|v| u64::from_str_radix(v, 16).ok());
        let inode = parts.next().and_then(|v| v.parse::<u64>().ok());
        if let (Some(major), Some(minor), Some(inode)) = (major, minor, inode) {
            locked.push((makedev(major, minor), inode));
        }
    }
    let expected = fs::metadata(expected_path)?;
    let expected_identity = (expected.dev(), expected.ino());
    if !expected.is_file() || !locked.contains(&expected_identity) {
        return Ok(None);
    }
    for entry in fs::read_dir("/proc/self/fd")? {
        let entry = entry?;
        let Ok(metadata) = fs::metadata(entry.path()) else {
            continue;
        };
        if metadata.is_file() && (metadata.dev(), metadata.ino()) == expected_identity {
            return Ok(Some(
                fs::canonicalize(expected_path)?
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

/// Linux `makedev` encoding, matching the `MAJOR:MINOR` pair `/proc/locks`
/// prints against the device id `stat` reports.
#[cfg(test)]
#[allow(dead_code)]
fn makedev(major: u64, minor: u64) -> u64 {
    ((major & 0xfff) << 8) | (minor & 0xff) | ((major & !0xfff) << 32) | ((minor & !0xff) << 12)
}

#[cfg(test)]
#[allow(dead_code)]
fn collect_host_facts() -> io::Result<HostFacts> {
    let repo_root = PathBuf::from(command_output("git", &["rev-parse", "--show-toplevel"])?);
    let revision = command_output("git", &["rev-parse", "HEAD"])?;
    let binary = env::current_exe()?;
    let digest = command_output("sha256sum", &[binary.to_string_lossy().as_ref()])?;
    let digest = digest
        .split_whitespace()
        .next()
        .ok_or_else(|| io::Error::other("sha256sum printed no digest"))?;
    let affinity = cpu_affinity()?;
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
    let expected_lock =
        PathBuf::from(env::var(LOCK_PATH_VAR).unwrap_or_else(|_| DEFAULT_LOCK_PATH.to_owned()));
    let lock_file = observed_lock_file(&expected_lock)?.ok_or_else(|| {
        io::Error::other(format!(
            "no inherited exclusive flock: run this action through the repository lock wrapper, \
             which holds {} for the whole run",
            expected_lock.display()
        ))
    })?;
    let selected_toolchain = env::var("RUSTUP_TOOLCHAIN").map_err(|_| {
        io::Error::other(format!(
            "RUSTUP_TOOLCHAIN must be set to {REQUIRED_RUSTUP_TOOLCHAIN} for calibration"
        ))
    })?;
    if selected_toolchain != REQUIRED_RUSTUP_TOOLCHAIN {
        return Err(io::Error::other(format!(
            "RUSTUP_TOOLCHAIN is {selected_toolchain:?}, expected {REQUIRED_RUSTUP_TOOLCHAIN:?}"
        )));
    }
    let toolchain = command_output("rustc", &["--version", "--verbose"])?;
    if !toolchain.starts_with("rustc 1.95.0") {
        return Err(io::Error::other(format!(
            "runtime rustc is not the required 1.95.0 compiler: {toolchain}"
        )));
    }
    Ok(HostFacts {
        source_revision: GitRevision::parse(&revision).map_err(io::Error::other)?,
        source_dirty: !command_output("git", GIT_STATUS_ARGS)?.is_empty(),
        harness: RepoRelPath::parse(&harness_path(&repo_root)?).map_err(io::Error::other)?,
        binary_sha256: Sha256::parse(digest).map_err(io::Error::other)?,
        toolchain,
        host: command_output("hostname", &[])?,
        cpu_model: cpuinfo
            .lines()
            .find_map(|line| line.strip_prefix("model name\t: "))
            .unwrap_or("unknown")
            .to_owned(),
        cpu_features: cpu_features(),
        os_kernel: command_output("uname", &["-srvmo"])?,
        governor: governor(&affinity)?,
        lock_file,
        cpu_affinity: affinity,
    })
}

/// Refuses to start a calibration whose named revision cannot reproduce its binary.
#[cfg(test)]
#[allow(dead_code)]
fn require_clean_source(source_dirty: bool, source_revision: &GitRevision) -> Result<(), String> {
    if source_dirty {
        Err(format!(
            "the source tree is dirty at {}; calibration aborted before any probe or timed call",
            source_revision.as_str()
        ))
    } else {
        Ok(())
    }
}

/// Formats a UTC instant in the RFC 3339 form `Rfc3339Utc::parse` accepts.
#[cfg(test)]
#[allow(dead_code)]
fn rfc3339_utc(instant: SystemTime) -> io::Result<String> {
    let seconds = instant
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    let time = seconds % 86_400;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        (time / 60) % 60,
        time % 60
    ))
}

/// Civil date of the day `days` after 1970-01-01, by the shift-to-March
/// era arithmetic that avoids a month-length table.
#[cfg(test)]
#[allow(dead_code)]
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// `BIT_FIXTURES` buffers of `len` words laid out contiguously, each starting
/// on a 64-byte boundary.
///
/// The alignment is controlled rather than left to the allocator: a 64-byte
/// cache line holds eight `u64`, a wide load that straddles two lines costs
/// materially more than one that does not, and the allocator's address phase
/// differs between processes. An uncontrolled bank would make the two arms of
/// this family measure a property of the heap.
struct BitBank {
    storage: Vec<u64>,
    offset: usize,
    stride: usize,
    len: usize,
}

impl BitBank {
    fn new(len: usize, mut word: impl FnMut(usize, usize) -> u64) -> Self {
        let stride = len.next_multiple_of(WORDS_PER_LINE).max(WORDS_PER_LINE);
        let mut storage = vec![0_u64; BIT_FIXTURES * stride + WORDS_PER_LINE];
        let misalignment = (storage.as_ptr() as usize) % (WORDS_PER_LINE * 8);
        let offset = (WORDS_PER_LINE * 8 - misalignment) % (WORDS_PER_LINE * 8) / 8;
        for bank in 0..BIT_FIXTURES {
            for index in 0..len {
                storage[offset + bank * stride + index] = word(bank, index);
            }
        }
        let bank = Self {
            storage,
            offset,
            stride,
            len,
        };
        assert!(
            bank.is_line_aligned(),
            "bit fixture bank did not land on a cache-line boundary"
        );
        bank
    }

    fn is_line_aligned(&self) -> bool {
        let base = self.storage.as_ptr() as usize + self.offset * 8;
        base.is_multiple_of(WORDS_PER_LINE * 8) && self.stride.is_multiple_of(WORDS_PER_LINE)
    }

    fn start(&self, bank: usize) -> usize {
        self.offset + bank * self.stride
    }

    fn get(&self, bank: usize) -> &[u64] {
        let start = self.start(bank);
        &self.storage[start..start + self.len]
    }

    fn get_mut(&mut self, bank: usize) -> &mut [u64] {
        let start = self.start(bank);
        &mut self.storage[start..start + self.len]
    }
}

/// One grid point's operands, built once and shared by both arms.
enum Fixture {
    Bit {
        dst: BitBank,
        src: BitBank,
    },
    Mul {
        a: FieldPoly<F>,
        b: FieldPoly<F>,
    },
    DivRem {
        dividend: FieldPoly<F>,
        divisor: FieldPoly<F>,
    },
    BatchEval {
        poly: FieldPoly<F>,
        points: Vec<F>,
    },
}

fn seed_for(field: CalibratedField, size: usize, role: u64) -> u64 {
    tuning_campaign_support::seed::fixture_seed(SEED_ROOT, field.seed_tag(), size as u64, role)
}

/// One named deterministic fixture stream.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SeedStream {
    name: String,
    role: u64,
    seed: u64,
}

/// Complete seed inventory for one field/grid fixture.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SeedInventory {
    schema: String,
    derivation: String,
    seed_root: u64,
    field: CalibratedField,
    field_tag: u64,
    size: usize,
    streams: Vec<SeedStream>,
}

fn seed_inventory(field: CalibratedField, size: usize) -> SeedInventory {
    let stream = |name: String, role: u64| SeedStream {
        name,
        role,
        seed: seed_for(field, size, role),
    };
    let streams = match field {
        CalibratedField::SimdMinWords => {
            let mut streams = Vec::with_capacity(BIT_FIXTURES * 2);
            for bank in 0..BIT_FIXTURES {
                streams.push(stream(
                    format!("dst[{bank}]"),
                    SEED_ROLE_BIT_DST + bank as u64,
                ));
            }
            for bank in 0..BIT_FIXTURES {
                streams.push(stream(
                    format!("src[{bank}]"),
                    SEED_ROLE_BIT_SRC + bank as u64,
                ));
            }
            streams
        }
        CalibratedField::KaratsubaMinDegree | CalibratedField::KaratsubaMaxOutLen => vec![
            stream("lhs".to_owned(), SEED_ROLE_POLY_LHS),
            stream("rhs".to_owned(), SEED_ROLE_POLY_RHS),
        ],
        CalibratedField::DivRemFastMinLen => vec![
            stream("dividend".to_owned(), SEED_ROLE_DIVIDEND),
            stream("divisor".to_owned(), SEED_ROLE_DIVISOR),
        ],
        CalibratedField::SubproductMinLen => vec![
            stream("polynomial".to_owned(), SEED_ROLE_EVAL_POLY),
            stream("points".to_owned(), SEED_ROLE_EVAL_POINTS),
        ],
        _ => {
            let roles = FOLLOW_ON_ROLES
                .iter()
                .find_map(|(candidate, roles)| (*candidate == field).then_some(*roles))
                .expect("every follow-on has a predeclared role set");
            let mut streams = Vec::with_capacity(BIT_FIXTURES * roles.len());
            for bank in 0..BIT_FIXTURES {
                for &(name, role) in roles {
                    let banked_role = role.wrapping_add((bank as u64) << 16);
                    streams.push(stream(format!("{name}[{bank}]"), banked_role));
                }
            }
            streams
        }
    };
    SeedInventory {
        schema: SEED_SCHEMA.to_owned(),
        derivation: SEED_DERIVATION.to_owned(),
        seed_root: SEED_ROOT,
        field,
        field_tag: field.seed_tag(),
        size,
        streams,
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn seed_inventory_line(field: CalibratedField, size: usize) -> Result<String, String> {
    serde_json::to_string(&seed_inventory(field, size))
        .map(|json| format!("{SEED_PREFIX}{json}"))
        .map_err(|error| format!("cannot encode the seed inventory: {error}"))
}

/// Builds a deterministic polynomial holding exactly `len` coefficients.
///
/// Coefficients are drawn from `1..=65536`, so none of them is the zero element
/// of `Fp<65537>` and `FieldPoly::new`'s trailing-zero normalisation cannot
/// shorten the operand. The grid points select their arm from exact
/// coefficient counts, so the assertion is a precondition rather than a
/// diagnostic.
fn make_poly(len: usize, seed: u64) -> FieldPoly<F> {
    let mut rng = Lcg::new(seed);
    let coeffs: Vec<F> = (0..len)
        .map(|_| F::new((rng.next_u64() % 65_536) + 1))
        .collect();
    let poly = FieldPoly::new(coeffs);
    assert_eq!(poly.len(), len, "fixture polynomial lost coefficients");
    poly
}

/// Builds `len` distinct non-zero evaluation points.
fn make_points(len: usize, seed: u64) -> Vec<F> {
    let stride = 1_000_003_u64;
    let modulus_minus_one = 65_536_u64;
    let offset = Lcg::new(seed).next_u64() % modulus_minus_one;
    (0..len)
        .map(|index| F::new((offset + (index as u64).wrapping_mul(stride)) % modulus_minus_one + 1))
        .collect()
}

/// Operand length whose product has length `out_len`, for equal-length
/// operands.
fn operand_len_for_product(out_len: usize) -> usize {
    out_len.div_ceil(2)
}

fn build_fixture(field: CalibratedField, size: usize) -> Fixture {
    match field {
        CalibratedField::SimdMinWords => {
            let fill = |role: u64| {
                let mut rngs: Vec<Lcg> = (0..BIT_FIXTURES)
                    .map(|bank| Lcg::new(seed_for(field, size, role + bank as u64)))
                    .collect();
                BitBank::new(size, move |bank, _| rngs[bank].next_u64())
            };
            Fixture::Bit {
                dst: fill(SEED_ROLE_BIT_DST),
                src: fill(SEED_ROLE_BIT_SRC),
            }
        }
        CalibratedField::KaratsubaMinDegree => Fixture::Mul {
            a: make_poly(size + 1, seed_for(field, size, SEED_ROLE_POLY_LHS)),
            b: make_poly(size + 1, seed_for(field, size, SEED_ROLE_POLY_RHS)),
        },
        CalibratedField::KaratsubaMaxOutLen => {
            let len = operand_len_for_product(size);
            Fixture::Mul {
                a: make_poly(len, seed_for(field, size, SEED_ROLE_POLY_LHS)),
                b: make_poly(len, seed_for(field, size, SEED_ROLE_POLY_RHS)),
            }
        }
        CalibratedField::DivRemFastMinLen => Fixture::DivRem {
            dividend: make_poly(2 * size, seed_for(field, size, SEED_ROLE_DIVIDEND)),
            divisor: make_poly(size, seed_for(field, size, SEED_ROLE_DIVISOR)),
        },
        CalibratedField::SubproductMinLen => Fixture::BatchEval {
            poly: make_poly(size, seed_for(field, size, SEED_ROLE_EVAL_POLY)),
            points: make_points(size, seed_for(field, size, SEED_ROLE_EVAL_POINTS)),
        },
        _ => panic!("follow-on fixtures are built only inside their fresh child"),
    }
}

type Fp251 = Fp<251>;

struct QuadraticBeta3;
impl ExtConfig for QuadraticBeta3 {
    type BaseField = F;
    const NON_RESIDUE: F = F::new(3);
}

struct CubicBeta3;
impl ExtConfig for CubicBeta3 {
    type BaseField = F;
    const NON_RESIDUE: F = F::new(3);
}

/// Exact eight-bank fixture for one follow-on fresh child.
enum FollowOnFixture {
    Transpose(Vec<BitMatrix>),
    Soa {
        quadratic_lhs: Vec<BatchExtField<F, 2>>,
        quadratic_rhs: Vec<BatchExtField<F, 2>>,
        cubic_lhs: Vec<BatchExtField<F, 3>>,
        cubic_rhs: Vec<BatchExtField<F, 3>>,
    },
    M4rm(Vec<(BitMatrix, BitMatrix)>),
    BitInverse(Vec<BitMatrix>),
    FieldInverse(Vec<FieldMatrix<Fp251>>),
    Trsm(Vec<(FieldMatrix<Fp251>, FieldMatrix<Fp251>)>),
    PlePanel(Vec<FieldMatrix<Fp251>>),
    PleBackSub(Vec<FieldMatrix<Fp251>>),
    Gemm(Vec<(FieldMatrix<Fp251>, FieldMatrix<Fp251>)>),
    Interpolation(Vec<Vec<(F, F)>>),
}

fn bank_seed(field: CalibratedField, size: usize, role: u64, bank: usize) -> u64 {
    seed_for(field, size, role.wrapping_add((bank as u64) << 16))
}

/// Partner bank for every binary fixture, predeclared as `b + 3 (mod 8)`.
fn paired_bank(bank: usize) -> usize {
    (bank + 3) & (BIT_FIXTURES - 1)
}

fn bit_matrix_from_words(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let mut rng = Lcg::new(seed);
    let mut matrix = BitMatrix::zeros(rows, cols);
    for row in 0..rows {
        let words = matrix.row_words_mut(row);
        for word in &mut *words {
            *word = rng.next_u64();
        }
        if let (Some(last), tail @ 1..=63) = (words.last_mut(), cols & 63) {
            *last &= (1_u64 << tail) - 1;
        }
    }
    matrix
}

fn bit_lu_fixture(size: usize, field: CalibratedField, bank: usize) -> BitMatrix {
    let mut lower_rng = Lcg::new(bank_seed(field, size, 0x400, bank));
    let mut upper_rng = Lcg::new(bank_seed(field, size, 0x401, bank));
    let mut lower = BitMatrix::identity(size);
    let mut upper = BitMatrix::identity(size);
    for row in 0..size {
        for col in 0..row {
            lower.set(row, col, lower_rng.next_u64() & 1 != 0);
        }
    }
    for row in 0..size {
        for col in (row + 1)..size {
            upper.set(row, col, upper_rng.next_u64() & 1 != 0);
        }
    }
    scalar_bit_matmul(&lower, &upper)
}

fn scalar_bit_matmul(lhs: &BitMatrix, rhs: &BitMatrix) -> BitMatrix {
    let mut out = BitMatrix::zeros(lhs.rows(), rhs.cols());
    for row in 0..lhs.rows() {
        for col in 0..rhs.cols() {
            let mut value = false;
            for inner in 0..lhs.cols() {
                value ^= lhs.get(row, inner) & rhs.get(inner, col);
            }
            out.set(row, col, value);
        }
    }
    out
}

/// Independent word-parity oracle for the rectangular M4RM fixtures.
///
/// This packs each rhs column without calling a production transpose or
/// multiplication dispatcher, then evaluates each dot product as the parity
/// of canonical row words. The scalar bit-by-bit product above remains the
/// exact construction oracle for the much smaller inverse fixtures.
fn scalar_m4rm_oracle(lhs: &BitMatrix, rhs: &BitMatrix) -> BitMatrix {
    assert_eq!(lhs.cols(), rhs.rows());
    let mut out = BitMatrix::zeros(lhs.rows(), rhs.cols());
    let mut rhs_column = vec![0_u64; rhs.rows().div_ceil(64)];
    for col in 0..rhs.cols() {
        rhs_column.fill(0);
        for row in 0..rhs.rows() {
            if rhs.get(row, col) {
                rhs_column[row >> 6] |= 1_u64 << (row & 63);
            }
        }
        for row in 0..lhs.rows() {
            let parity = lhs
                .row_words(row)
                .iter()
                .zip(&rhs_column)
                .fold(0_u32, |acc, (left, right)| {
                    acc ^ (left & right).count_ones()
                })
                & 1
                != 0;
            out.set(row, col, parity);
        }
    }
    out
}

fn rref_nonzero_row_leads(matrix: &FieldMatrix<Fp251>) -> Vec<usize> {
    (0..matrix.rows())
        .filter_map(|row| (0..matrix.cols()).find(|&col| matrix.get(row, col) != Fp251::new(0)))
        .collect()
}

fn field_lu_parts(
    size: usize,
    lower_seed: u64,
    upper_seed: u64,
) -> (FieldMatrix<Fp251>, FieldMatrix<Fp251>) {
    let mut lower_rng = Lcg::new(lower_seed);
    let mut upper_rng = Lcg::new(upper_seed);
    let mut lower = FieldMatrix::<Fp251>::identity(size);
    let mut upper = FieldMatrix::<Fp251>::identity(size);
    for row in 0..size {
        for col in 0..row {
            lower.set(row, col, Fp251::new(lower_rng.next_u64() % 251));
        }
    }
    for row in 0..size {
        for col in (row + 1)..size {
            upper.set(row, col, Fp251::new(upper_rng.next_u64() % 251));
        }
    }
    (lower, upper)
}

fn scalar_field_matmul<const P: u64>(
    lhs: &FieldMatrix<Fp<P>>,
    rhs: &FieldMatrix<Fp<P>>,
) -> FieldMatrix<Fp<P>> {
    let mut out = FieldMatrix::<Fp<P>>::zeros(lhs.rows(), rhs.cols());
    for row in 0..lhs.rows() {
        for col in 0..rhs.cols() {
            let mut value = Fp::<P>::new(0);
            for inner in 0..lhs.cols() {
                value += lhs.get(row, inner) * rhs.get(inner, col);
            }
            out.set(row, col, value);
        }
    }
    out
}

fn filled_field_matrix(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp251> {
    let mut rng = Lcg::new(seed);
    let mut matrix = FieldMatrix::<Fp251>::zeros(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            matrix.set(row, col, Fp251::new(rng.next_u64() % 251));
        }
    }
    matrix
}

fn exact_back_sub_fixture(size: usize, field: CalibratedField, bank: usize) -> FieldMatrix<Fp251> {
    let rank = size / 2;
    let mut designated_rng = Lcg::new(bank_seed(field, size, 0x800, bank));
    let mut non_designated_rng = Lcg::new(bank_seed(field, size, 0x801, bank));
    let mut mix_rng = Lcg::new(bank_seed(field, size, 0x802, bank));
    let mut embedded = FieldMatrix::<Fp251>::zeros(size, size);
    for row in 0..rank {
        embedded.set(
            row,
            2 * row,
            Fp251::new(1 + designated_rng.next_u64() % 250),
        );
    }
    for row in 0..rank {
        for col in 0..size {
            if !col.is_multiple_of(2) || col >= 2 * rank {
                embedded.set(row, col, Fp251::new(non_designated_rng.next_u64() % 251));
            }
        }
    }
    let mut lower = FieldMatrix::<Fp251>::identity(size);
    for row in 0..size {
        for col in 0..row {
            lower.set(row, col, Fp251::new(mix_rng.next_u64() % 251));
        }
    }
    scalar_field_matmul(&lower, &embedded)
}

fn soa_batch<const N: usize>(size: usize, seed: u64) -> BatchExtField<F, N> {
    let mut rng = Lcg::new(seed);
    BatchExtField::new(std::array::from_fn(|_| {
        (0..size).map(|_| F::new(rng.next_u64() % 65_537)).collect()
    }))
}

fn cube_dim(volume: usize) -> Result<usize, String> {
    [4, 8, 12, 15, 16, 17, 20, 24, 32]
        .into_iter()
        .find(|dimension| dimension * dimension * dimension == volume)
        .ok_or_else(|| format!("{volume} is not a predeclared GEMM cube volume"))
}

fn interpolation_fixture(size: usize, field: CalibratedField, bank: usize) -> Vec<(F, F)> {
    let mut coeff_rng = Lcg::new(bank_seed(field, size, 0xa00, bank));
    let coefficients: Vec<F> = (0..size)
        .map(|_| F::new(1 + coeff_rng.next_u64() % 65_536))
        .collect();
    let offset = Lcg::new(bank_seed(field, size, 0xa01, bank)).next_u64() % 65_536;
    (0..size)
        .map(|index| {
            let x = F::new((offset + (index as u64).wrapping_mul(1_000_003)) % 65_536 + 1);
            let y = coefficients
                .iter()
                .rev()
                .fold(F::new(0), |value, coefficient| value * x + *coefficient);
            (x, y)
        })
        .collect()
}

fn build_follow_on_fixture(field: CalibratedField, size: usize) -> Result<FollowOnFixture, String> {
    let banks = 0..BIT_FIXTURES;
    Ok(match field {
        CalibratedField::TransposeSimpleMaxBlocks => FollowOnFixture::Transpose(
            banks
                .map(|bank| {
                    bit_matrix_from_words(64 * size, 64 * size, bank_seed(field, size, 0x100, bank))
                })
                .collect(),
        ),
        CalibratedField::SoaParallelMinLen => {
            let mut quadratic_lhs = Vec::with_capacity(BIT_FIXTURES);
            let mut quadratic_rhs = Vec::with_capacity(BIT_FIXTURES);
            let mut cubic_lhs = Vec::with_capacity(BIT_FIXTURES);
            let mut cubic_rhs = Vec::with_capacity(BIT_FIXTURES);
            for bank in banks {
                quadratic_lhs.push(soa_batch(size, bank_seed(field, size, 0x200, bank)));
                quadratic_rhs.push(soa_batch(size, bank_seed(field, size, 0x201, bank)));
                cubic_lhs.push(soa_batch(size, bank_seed(field, size, 0x202, bank)));
                cubic_rhs.push(soa_batch(size, bank_seed(field, size, 0x203, bank)));
            }
            FollowOnFixture::Soa {
                quadratic_lhs,
                quadratic_rhs,
                cubic_lhs,
                cubic_rhs,
            }
        }
        CalibratedField::M4rmWideTierMinStrideWords | CalibratedField::M4rmTiledMinStrideWords => {
            FollowOnFixture::M4rm(
                banks
                    .map(|bank| {
                        (
                            bit_matrix_from_words(
                                64,
                                512,
                                bank_seed(
                                    field,
                                    size,
                                    if field == CalibratedField::M4rmWideTierMinStrideWords {
                                        0x300
                                    } else {
                                        0x310
                                    },
                                    bank,
                                ),
                            ),
                            bit_matrix_from_words(
                                512,
                                64 * size,
                                bank_seed(
                                    field,
                                    size,
                                    if field == CalibratedField::M4rmWideTierMinStrideWords {
                                        0x301
                                    } else {
                                        0x311
                                    },
                                    bank,
                                ),
                            ),
                        )
                    })
                    .collect(),
            )
        }
        CalibratedField::DenseInverseM4riMinDim => FollowOnFixture::BitInverse(
            banks
                .map(|bank| bit_lu_fixture(size, field, bank))
                .collect(),
        ),
        CalibratedField::DenseInverseBlockedMinDim => FollowOnFixture::FieldInverse(
            banks
                .map(|bank| {
                    let (l, u) = field_lu_parts(
                        size,
                        bank_seed(field, size, 0x500, bank),
                        bank_seed(field, size, 0x501, bank),
                    );
                    scalar_field_matmul(&l, &u)
                })
                .collect(),
        ),
        CalibratedField::TrsmBlockedMinDim => FollowOnFixture::Trsm(
            banks
                .map(|bank| {
                    let (l, u) = field_lu_parts(
                        size,
                        bank_seed(field, size, 0x600, bank),
                        bank_seed(field, size, 0x601, bank),
                    );
                    (
                        scalar_field_matmul(&l, &u),
                        filled_field_matrix(size, size, bank_seed(field, size, 0x602, bank)),
                    )
                })
                .collect(),
        ),
        CalibratedField::PlePanelBaseMaxCols => FollowOnFixture::PlePanel(
            banks
                .map(|bank| {
                    let (l, u) = field_lu_parts(
                        size,
                        bank_seed(field, size, 0x700, bank),
                        bank_seed(field, size, 0x701, bank),
                    );
                    scalar_field_matmul(&l, &u)
                })
                .collect(),
        ),
        CalibratedField::PleBlockedBackSubMinDim => FollowOnFixture::PleBackSub(
            banks
                .map(|bank| exact_back_sub_fixture(size, field, bank))
                .collect(),
        ),
        CalibratedField::GemmAxpyFastPathMinVolume => {
            let dimension = cube_dim(size)?;
            FollowOnFixture::Gemm(
                banks
                    .map(|bank| {
                        (
                            filled_field_matrix(
                                dimension,
                                dimension,
                                bank_seed(field, size, 0x900, bank),
                            ),
                            filled_field_matrix(
                                dimension,
                                dimension,
                                bank_seed(field, size, 0x901, bank),
                            ),
                        )
                    })
                    .collect(),
            )
        }
        CalibratedField::InterpolateFastMinPoints => FollowOnFixture::Interpolation(
            banks
                .map(|bank| interpolation_fixture(size, field, bank))
                .collect(),
        ),
        _ => return Err(format!("{field} is a direct fixture, not a follow-on")),
    })
}

/// Derives the canonical operand digest from the requested field and grid
/// point without installing tuning state or calling a production dispatcher.
/// The parent computes this once before launching the point's children.
fn expected_operand_digest(field: CalibratedField, size: usize) -> Result<String, String> {
    if matches!(
        field,
        CalibratedField::SimdMinWords
            | CalibratedField::KaratsubaMinDegree
            | CalibratedField::KaratsubaMaxOutLen
            | CalibratedField::DivRemFastMinLen
            | CalibratedField::SubproductMinLen
    ) {
        Ok(direct_operand_digest(&build_fixture(field, size)))
    } else {
        Ok(follow_on_operand_digest(&build_follow_on_fixture(
            field, size,
        )?))
    }
}

fn digest_bit_matrix(matrix: &BitMatrix) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(b"gf2-calibration-bit-matrix-v1");
    digest.update(matrix.rows().to_le_bytes());
    digest.update(matrix.cols().to_le_bytes());
    for row in 0..matrix.rows() {
        for word in matrix.row_words(row) {
            digest.update(word.to_le_bytes());
        }
    }
    format!("{:x}", digest.finalize())
}

fn digest_field_matrix(matrix: &FieldMatrix<Fp251>) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(b"gf2-calibration-fp251-matrix-v1");
    digest.update(matrix.rows().to_le_bytes());
    digest.update(matrix.cols().to_le_bytes());
    for row in 0..matrix.rows() {
        for col in 0..matrix.cols() {
            digest.update(matrix.get(row, col).value().to_le_bytes());
        }
    }
    format!("{:x}", digest.finalize())
}

fn digest_poly(poly: &FieldPoly<F>) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(b"gf2-calibration-fp65537-polynomial-v1");
    digest.update(poly.len().to_le_bytes());
    for coefficient in poly.iter() {
        digest.update(coefficient.value().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn digest_tuple(domain: &[u8], parts: impl IntoIterator<Item = String>) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(domain);
    for part in parts {
        digest.update(part.len().to_le_bytes());
        digest.update(part.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn digest_batch<const N: usize>(batch: &BatchExtField<F, N>) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(b"gf2-calibration-fp65537-soa-v1");
    digest.update(N.to_le_bytes());
    digest.update(batch.len().to_le_bytes());
    for lane in 0..N {
        for value in batch.coeff(lane) {
            digest.update(value.value().to_le_bytes());
        }
    }
    format!("{:x}", digest.finalize())
}

fn digest_points(points: &[(F, F)]) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(b"gf2-calibration-interpolation-points-v1");
    digest.update(points.len().to_le_bytes());
    for (x, y) in points {
        digest.update(x.value().to_le_bytes());
        digest.update(y.value().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn follow_on_operand_digest(fixture: &FollowOnFixture) -> String {
    match fixture {
        FollowOnFixture::Transpose(banks) | FollowOnFixture::BitInverse(banks) => digest_tuple(
            b"gf2-calibration-unary-bit-banks-v1",
            banks.iter().map(digest_bit_matrix),
        ),
        FollowOnFixture::Soa {
            quadratic_lhs,
            quadratic_rhs,
            cubic_lhs,
            cubic_rhs,
        } => digest_tuple(
            b"gf2-calibration-soa-banks-v1",
            (0..quadratic_lhs.len()).flat_map(|bank| {
                [
                    digest_batch(&quadratic_lhs[bank]),
                    digest_batch(&quadratic_rhs[bank]),
                    digest_batch(&cubic_lhs[bank]),
                    digest_batch(&cubic_rhs[bank]),
                ]
            }),
        ),
        FollowOnFixture::M4rm(banks) => digest_tuple(
            b"gf2-calibration-binary-bit-banks-v1",
            banks
                .iter()
                .flat_map(|(lhs, rhs)| [digest_bit_matrix(lhs), digest_bit_matrix(rhs)]),
        ),
        FollowOnFixture::FieldInverse(banks)
        | FollowOnFixture::PlePanel(banks)
        | FollowOnFixture::PleBackSub(banks) => digest_tuple(
            b"gf2-calibration-unary-fp251-banks-v1",
            banks.iter().map(digest_field_matrix),
        ),
        FollowOnFixture::Trsm(banks) | FollowOnFixture::Gemm(banks) => digest_tuple(
            b"gf2-calibration-binary-fp251-banks-v1",
            banks
                .iter()
                .flat_map(|(lhs, rhs)| [digest_field_matrix(lhs), digest_field_matrix(rhs)]),
        ),
        FollowOnFixture::Interpolation(banks) => digest_tuple(
            b"gf2-calibration-interpolation-banks-v1",
            banks.iter().map(|points| digest_points(points)),
        ),
    }
}

fn follow_on_shape(field: CalibratedField, size: usize) -> Result<String, String> {
    Ok(match field {
        CalibratedField::TransposeSimpleMaxBlocks => format!("{}x{}", 64 * size, 64 * size),
        CalibratedField::SoaParallelMinLen => format!("quadratic+cubic length={size}"),
        CalibratedField::M4rmWideTierMinStrideWords | CalibratedField::M4rmTiledMinStrideWords => {
            format!("64x512 * 512x{}", 64 * size)
        }
        CalibratedField::DenseInverseM4riMinDim
        | CalibratedField::DenseInverseBlockedMinDim
        | CalibratedField::PlePanelBaseMaxCols
        | CalibratedField::PleBlockedBackSubMinDim => format!("{size}x{size}"),
        CalibratedField::TrsmBlockedMinDim => format!("{size}x{size} * {size}x{size}"),
        CalibratedField::GemmAxpyFastPathMinVolume => {
            let d = cube_dim(size)?;
            format!("{d}x{d} * {d}x{d}")
        }
        CalibratedField::InterpolateFastMinPoints => format!("{size} distinct points"),
        _ => return Err(format!("{field} is not a follow-on")),
    })
}

fn expected_fixture_shape(field: CalibratedField, size: usize) -> Result<String, String> {
    Ok(match field {
        CalibratedField::SimdMinWords => format!("8 aligned banks x {size} words"),
        CalibratedField::KaratsubaMinDegree => format!("degree {size} x degree {size}"),
        CalibratedField::KaratsubaMaxOutLen => format!("product_len={size}"),
        CalibratedField::DivRemFastMinLen => {
            format!("dividend_len={} divisor_len={size}", 2 * size)
        }
        CalibratedField::SubproductMinLen => format!("coefficients={size} points={size}"),
        _ => follow_on_shape(field, size)?,
    })
}

fn expected_observation_contract(spec: ChildSpec) -> Result<(String, String), String> {
    Ok(match spec.field {
        CalibratedField::SimdMinWords => (
            "baked_selector_direct_backend".to_owned(),
            match spec.arm {
                Arm::Conservative => "scalar_backend".to_owned(),
                Arm::Asymptotic => format!(
                    "simd_backend={}",
                    simd_backend()
                        .ok_or("no concrete SIMD backend is available")?
                        .name()
                ),
            },
        ),
        CalibratedField::KaratsubaMinDegree
        | CalibratedField::KaratsubaMaxOutLen
        | CalibratedField::DivRemFastMinLen
        | CalibratedField::SubproductMinLen => {
            ("production_dispatch".to_owned(), "not_required".to_owned())
        }
        CalibratedField::TransposeSimpleMaxBlocks
        | CalibratedField::DenseInverseM4riMinDim
        | CalibratedField::DenseInverseBlockedMinDim => {
            ("not_required".to_owned(), "not_required".to_owned())
        }
        CalibratedField::SoaParallelMinLen => (
            match spec.arm {
                Arm::Conservative => "sequential_no_chunk".to_owned(),
                Arm::Asymptotic => {
                    format!("parallel_chunk={REQUIRED_SOA_PARALLEL_CHUNK_LEN}")
                }
            },
            "dedicated_pool_width=4".to_owned(),
        ),
        CalibratedField::M4rmWideTierMinStrideWords => {
            ("panel_width".to_owned(), "not_required".to_owned())
        }
        CalibratedField::M4rmTiledMinStrideWords => (
            match spec.arm {
                Arm::Conservative => "RowWise".to_owned(),
                Arm::Asymptotic => "RegisterTiled".to_owned(),
            },
            "simd_tile8xn=resolved".to_owned(),
        ),
        CalibratedField::TrsmBlockedMinDim => (
            match spec.arm {
                Arm::Conservative => "panel_rows=None".to_owned(),
                Arm::Asymptotic => "panel_rows=Some(64)".to_owned(),
            },
            "fp251_whole_gemm_available=true".to_owned(),
        ),
        CalibratedField::PlePanelBaseMaxCols => (
            format!(
                "max_panel_cols={}",
                match spec.arm {
                    Arm::Conservative => spec.size,
                    Arm::Asymptotic => spec.size - 1,
                }
            ),
            format!("carrier_lane=byte panel_byte_lane_max_cols={REQUIRED_PLE_BYTE_LANE_MAX_COLS}"),
        ),
        CalibratedField::PleBlockedBackSubMinDim => (
            format!(
                "rank={} free_cols={}",
                spec.size / 2,
                spec.size - spec.size / 2
            ),
            "not_required".to_owned(),
        ),
        CalibratedField::GemmAxpyFastPathMinVolume => (
            match spec.arm {
                Arm::Conservative => "PerCell".to_owned(),
                Arm::Asymptotic => "WholeGemm".to_owned(),
            },
            "fp251_whole_gemm_available=true".to_owned(),
        ),
        CalibratedField::InterpolateFastMinPoints => {
            (spec.variant.to_string(), "not_required".to_owned())
        }
    })
}

fn digest_f_values(domain: &[u8], values: &[F]) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(domain);
    digest.update(values.len().to_le_bytes());
    for value in values {
        digest.update(value.value().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn direct_operand_digest(fixture: &Fixture) -> String {
    let mut digest = Sha256Hasher::new();
    digest.update(b"gf2-calibration-direct-operands-v3");
    match fixture {
        Fixture::Bit { dst, src } => {
            for bank in 0..BIT_FIXTURES {
                for word in dst.get(bank).iter().chain(src.get(bank)) {
                    digest.update(word.to_le_bytes());
                }
            }
        }
        Fixture::Mul { a, b } => {
            digest.update(digest_poly(a));
            digest.update(digest_poly(b));
        }
        Fixture::DivRem { dividend, divisor } => {
            digest.update(digest_poly(dividend));
            digest.update(digest_poly(divisor));
        }
        Fixture::BatchEval { poly, points } => {
            digest.update(digest_poly(poly));
            digest.update(digest_f_values(b"direct-points", points));
        }
    }
    format!("{:x}", digest.finalize())
}

fn execute_direct(
    spec: ChildSpec,
    fixture: &mut Fixture,
    logical_index: usize,
) -> Result<OperationObservation, String> {
    let bank = logical_index & (BIT_FIXTURES - 1);
    let mut observation = OperationObservation {
        observed_route: spec.field.arm_name(spec.arm).to_owned(),
        effective_observation: "production_dispatch".to_owned(),
        capability_observation: "not_required".to_owned(),
        fixture_shape: expected_fixture_shape(spec.field, spec.size)?,
        operand_digest: direct_operand_digest(fixture),
        ..OperationObservation::default()
    };
    let result_digest = match (spec.field, spec.arm, fixture) {
        (CalibratedField::SimdMinWords, arm, Fixture::Bit { dst, src }) => {
            if !dst.is_line_aligned() || !src.is_line_aligned() {
                return Err("the bit-backend fixture banks are not line aligned".to_owned());
            }
            let (lhs, rhs) = bank_indices(bank);
            let backend: &dyn Backend = match arm {
                Arm::Conservative => &ScalarBackend,
                Arm::Asymptotic => match simd_backend() {
                    Some(backend) => backend,
                    None => {
                        observation.observed_route = "unavailable_before_dispatch".to_owned();
                        observation.effective_observation = "unavailable".to_owned();
                        observation.capability_observation = "simd_backend=none".to_owned();
                        observation.availability = ChildAvailability::Unavailable {
                            omission: CapabilityOmission::SimdBackendUnavailable,
                        };
                        return Ok(observation);
                    }
                },
            };
            observation.capability_observation = match arm {
                Arm::Conservative => "scalar_backend".to_owned(),
                Arm::Asymptotic => format!("simd_backend={}", backend.name()),
            };
            observation.effective_observation = "baked_selector_direct_backend".to_owned();
            let source = src.get(rhs);
            backend.xor(dst.get_mut(lhs), source);
            let words = dst.get(lhs);
            let mut hash = Sha256Hasher::new();
            hash.update(b"gf2-calibration-bit-backend-result-v1");
            for word in words {
                hash.update(word.to_le_bytes());
            }
            format!("{:x}", hash.finalize())
        }
        (CalibratedField::KaratsubaMinDegree, _, Fixture::Mul { a, b }) => {
            observation.observed_route = match mul_route(spec.size, spec.size) {
                MulRoute::Schoolbook => "schoolbook",
                MulRoute::Karatsuba => "karatsuba",
            }
            .to_owned();
            let result = a.mul(b);
            digest_poly(&result)
        }
        (CalibratedField::KaratsubaMaxOutLen, _, Fixture::Mul { a, b }) => {
            observation.observed_route = match mul_fast_route(spec.size) {
                MulFastRoute::Karatsuba => "karatsuba",
                MulFastRoute::Ntt => "mul_ntt",
            }
            .to_owned();
            let result = mul_fast(a, b);
            digest_poly(&result)
        }
        (CalibratedField::DivRemFastMinLen, _, Fixture::DivRem { dividend, divisor }) => {
            observation.observed_route = match div_rem_auto_route(dividend.len(), divisor.len()) {
                DivRemAutoRoute::Schoolbook => "div_rem",
                DivRemAutoRoute::Fast => "div_rem_fast",
            }
            .to_owned();
            let (quotient, remainder) = dividend.div_rem_auto(divisor);
            digest_tuple(
                b"gf2-calibration-div-rem-result-v1",
                [digest_poly(&quotient), digest_poly(&remainder)],
            )
        }
        (CalibratedField::SubproductMinLen, _, Fixture::BatchEval { poly, points }) => {
            observation.observed_route =
                match batch_evaluate_auto_route(poly.len(), points.len()) {
                    BatchEvaluateRoute::Horner => "eval_batch",
                    BatchEvaluateRoute::SubproductTree => "subproduct_auto",
                }
                .to_owned();
            let result = poly.batch_evaluate_auto(points);
            digest_f_values(b"gf2-calibration-batch-eval-result-v1", &result)
        }
        _ => return Err(format!("direct fixture does not match {}", spec.field)),
    };
    observation.result_digest = result_digest.clone();
    observation.equivalence_digest = digest_tuple(
        b"gf2-calibration-equivalence-v1",
        [observation.operand_digest.clone(), result_digest],
    );
    Ok(observation)
}

fn execute_follow_on(
    spec: ChildSpec,
    fixture: &FollowOnFixture,
    logical_index: usize,
) -> Result<OperationObservation, String> {
    let bank = logical_index & (BIT_FIXTURES - 1);
    let mut observation = OperationObservation {
        observed_route: spec.field.arm_name(spec.arm).to_owned(),
        effective_observation: "not_required".to_owned(),
        capability_observation: "not_required".to_owned(),
        fixture_shape: follow_on_shape(spec.field, spec.size)?,
        operand_digest: follow_on_operand_digest(fixture),
        ..OperationObservation::default()
    };
    let result_digest = match (spec.field, fixture) {
        (CalibratedField::TransposeSimpleMaxBlocks, FollowOnFixture::Transpose(banks)) => {
            let route = transpose_route(spec.size, spec.size);
            observation.observed_route = match route {
                TransposeRoute::Simple => "simple",
                TransposeRoute::MacroTiled { .. } => "macro_tiled",
            }
            .to_owned();
            let result = banks[bank].transpose();
            if result.transpose() != banks[bank] {
                return Err("transpose did not round-trip".to_owned());
            }
            for row in 0..banks[bank].rows() {
                for col in 0..banks[bank].cols() {
                    if result.get(col, row) != banks[bank].get(row, col) {
                        return Err("transpose failed entrywise scalar semantics".to_owned());
                    }
                }
            }
            digest_bit_matrix(&result)
        }
        (
            CalibratedField::SoaParallelMinLen,
            FollowOnFixture::Soa {
                quadratic_lhs,
                quadratic_rhs,
                cubic_lhs,
                cubic_rhs,
            },
        ) => {
            observation.observed_route = match soa_parallel_route(spec.size) {
                SoaParallelRoute::Sequential => "sequential",
                SoaParallelRoute::Parallel => "parallel",
            }
            .to_owned();
            let pool_width = rayon::current_num_threads();
            if pool_width != 4 {
                observation.observed_route = "unavailable_before_dispatch".to_owned();
                observation.effective_observation = "unavailable".to_owned();
                observation.capability_observation = format!("dedicated_pool_width={pool_width}");
                observation.availability = ChildAvailability::Unavailable {
                    omission: CapabilityOmission::SoaPoolWidth {
                        observed: pool_width,
                    },
                };
                return Ok(observation);
            }
            let mut effective = Vec::new();
            reset_last_effective_soa_chunk();
            let q_mul = quadratic_lhs[bank]
                .batch_mul_quadratic::<QuadraticBeta3>(&quadratic_rhs[paired_bank(bank)]);
            effective.push(last_effective_soa_chunk());
            reset_last_effective_soa_chunk();
            let q_square = quadratic_lhs[bank].batch_square_quadratic::<QuadraticBeta3>();
            effective.push(last_effective_soa_chunk());
            reset_last_effective_soa_chunk();
            let c_mul =
                cubic_lhs[bank].batch_mul_cubic::<CubicBeta3>(&cubic_rhs[paired_bank(bank)]);
            effective.push(last_effective_soa_chunk());
            reset_last_effective_soa_chunk();
            let c_square = cubic_lhs[bank].batch_square_cubic::<CubicBeta3>();
            effective.push(last_effective_soa_chunk());
            let expected_q_mul = campaign_owner::scalar_soa_product(
                &quadratic_lhs[bank],
                &quadratic_rhs[paired_bank(bank)],
            );
            let expected_q_square =
                campaign_owner::scalar_soa_product(&quadratic_lhs[bank], &quadratic_lhs[bank]);
            let expected_c_mul =
                campaign_owner::scalar_soa_product(&cubic_lhs[bank], &cubic_rhs[paired_bank(bank)]);
            let expected_c_square =
                campaign_owner::scalar_soa_product(&cubic_lhs[bank], &cubic_lhs[bank]);
            if (0..2).any(|lane| {
                q_mul.coeff(lane) != expected_q_mul.coeff(lane)
                    || q_square.coeff(lane) != expected_q_square.coeff(lane)
            }) || (0..3).any(|lane| {
                c_mul.coeff(lane) != expected_c_mul.coeff(lane)
                    || c_square.coeff(lane) != expected_c_square.coeff(lane)
            }) {
                return Err(
                    "retained SoA composite differs from scalar field arithmetic".to_owned(),
                );
            }
            {
                let expected = if spec.arm == Arm::Asymptotic {
                    vec![Some(REQUIRED_SOA_PARALLEL_CHUNK_LEN); 4]
                } else {
                    vec![None; 4]
                };
                if effective != expected {
                    return Err(format!(
                        "SoA effective chunks {effective:?}, expected {expected:?}"
                    ));
                }
                observation.effective_observation = if spec.arm == Arm::Asymptotic {
                    format!("parallel_chunk={}", expected[0].unwrap())
                } else {
                    "sequential_no_chunk".to_owned()
                };
            }

            observation.capability_observation = format!("dedicated_pool_width={pool_width}");
            digest_tuple(
                b"gf2-calibration-soa-result-v1",
                [
                    digest_batch(&q_mul),
                    digest_batch(&q_square),
                    digest_batch(&c_mul),
                    digest_batch(&c_square),
                ],
            )
        }
        (
            field @ (CalibratedField::M4rmWideTierMinStrideWords
            | CalibratedField::M4rmTiledMinStrideWords),
            FollowOnFixture::M4rm(banks),
        ) => {
            let route = m4rm_schedule_route(512, 64 * spec.size);
            observation.observed_route = if field == CalibratedField::M4rmWideTierMinStrideWords {
                match route.tier() {
                    M4rmScheduleTier::SmallN => "small_n",
                    M4rmScheduleTier::Wide => "wide",
                }
            } else if route.tiled_stride_admitted() {
                "register_tiled"
            } else {
                "row_wise"
            }
            .to_owned();
            reset_m4rm_tiled_effective_observation();
            let result = m4rm_multiply(&banks[bank].0, &banks[paired_bank(bank)].1);
            let effective = m4rm_tiled_effective_observation();
            if result != scalar_m4rm_oracle(&banks[bank].0, &banks[paired_bank(bank)].1) {
                return Err("M4RM result differs from scalar multiplication".to_owned());
            }
            if field == CalibratedField::M4rmTiledMinStrideWords {
                {
                    let expected = match spec.arm {
                        Arm::Conservative => M4rmTiledEffectiveObservation::RowWise,
                        Arm::Asymptotic => M4rmTiledEffectiveObservation::RegisterTiled,
                    };
                    if effective != expected {
                        if spec.arm == Arm::Asymptotic
                            && effective == M4rmTiledEffectiveObservation::RowWise
                        {
                            observation.observed_route = "row_wise".to_owned();
                            observation.effective_observation = "RowWise".to_owned();
                            observation.capability_observation =
                                "simd_tile8xn=unavailable".to_owned();
                            observation.availability = ChildAvailability::Unavailable {
                                omission: CapabilityOmission::M4rmRegisterTiledUnavailable,
                            };
                            return Ok(observation);
                        }
                        return Err(format!(
                            "M4RM effective observation {effective:?}, expected {expected:?}"
                        ));
                    }
                    observation.effective_observation = format!("{effective:?}");
                }

                observation.capability_observation = "simd_tile8xn=resolved".to_owned();
            } else {
                observation.effective_observation = format!("panel_width={}", route.panel_width());
            }
            digest_bit_matrix(&result)
        }
        (CalibratedField::DenseInverseM4riMinDim, FollowOnFixture::BitInverse(banks)) => {
            observation.observed_route = match invert_route(spec.size) {
                InvertRoute::Scalar => "scalar",
                InvertRoute::M4ri => "m4ri",
            }
            .to_owned();
            let result = invert(&banks[bank]).ok_or("generated GF(2) matrix is singular")?;
            if scalar_bit_matmul(&banks[bank], &result) != BitMatrix::identity(spec.size) {
                return Err("GF(2) inverse failed A*A^-1=I".to_owned());
            }
            digest_bit_matrix(&result)
        }
        (CalibratedField::DenseInverseBlockedMinDim, FollowOnFixture::FieldInverse(banks)) => {
            observation.observed_route = match inv_route(spec.size) {
                InvRoute::ScalarPle => "scalar_ple",
                InvRoute::BlockedPanelized => "blocked_panelized",
            }
            .to_owned();
            let result = banks[bank]
                .inv()
                .ok_or("generated Fp251 matrix is singular")?;
            if scalar_field_matmul(&banks[bank], &result) != FieldMatrix::identity(spec.size) {
                return Err("Fp251 inverse failed A*A^-1=I".to_owned());
            }
            digest_field_matrix(&result)
        }
        (CalibratedField::TrsmBlockedMinDim, FollowOnFixture::Trsm(banks)) => {
            let whole_gemm_available = <Fp251 as FiniteField>::has_simd_gemm_classical();
            if spec.arm == Arm::Asymptotic && !whole_gemm_available {
                observation.observed_route = "unavailable_before_dispatch".to_owned();
                observation.effective_observation = "unavailable".to_owned();
                observation.capability_observation = "fp251_whole_gemm_available=false".to_owned();
                observation.availability = ChildAvailability::Unavailable {
                    omission: CapabilityOmission::Fp251WholeGemmUnavailable,
                };
                return Ok(observation);
            }
            observation.observed_route = match trsm_route(spec.size) {
                TrsmRoute::Recursive => "recursive",
                TrsmRoute::Blocked => "blocked",
            }
            .to_owned();
            reset_last_effective_trsm_panel_rows();
            let a = &banks[bank].0;
            let b = &banks[paired_bank(bank)].1;
            let result = a
                .solve_batch(b)
                .ok_or("generated TRSM matrix is singular")?;
            let effective = last_effective_trsm_panel_rows();
            {
                let expected = if spec.arm == Arm::Asymptotic {
                    Some(64)
                } else {
                    None
                };
                if effective != expected {
                    if spec.arm == Arm::Asymptotic && effective.is_none() {
                        observation.effective_observation = "panel_rows=None".to_owned();
                        observation.observed_route = "blocked_callee_declined".to_owned();
                        observation.capability_observation =
                            "fp251_whole_gemm_available=true".to_owned();
                        observation.availability = ChildAvailability::Unavailable {
                            omission: CapabilityOmission::TrsmBlockedCalleeDeclined,
                        };
                        return Ok(observation);
                    }
                    return Err(format!(
                        "TRSM panel observation {effective:?}, expected {expected:?}"
                    ));
                }
                if scalar_field_matmul(a, &result) != *b {
                    return Err("TRSM failed A*X=B".to_owned());
                }
                campaign_owner::reset_solve_observations();
                let quiet = a
                    .solve_batch_quiet_for_test(b)
                    .ok_or("quiet TRSM fixture became singular")?;
                if quiet != result || !campaign_owner::quiet_solve_observations_empty() {
                    return Err(
                        "quiet TRSM changed semantics or published subtree observations".to_owned(),
                    );
                }
                observation.effective_observation = format!("panel_rows={effective:?}");
            }

            observation.capability_observation =
                format!("fp251_whole_gemm_available={whole_gemm_available}");
            digest_field_matrix(&result)
        }
        (CalibratedField::PlePanelBaseMaxCols, FollowOnFixture::PlePanel(banks)) => {
            let lane = <Fp251 as FiniteField>::simd_ple_panel_lane();
            observation.observed_route = match ple_panel_route(lane, spec.size) {
                PlePanelRoute::PanelBase => "panel_base",
                PlePanelRoute::SubPanelRecursion => "sub_panel_recursion",
                PlePanelRoute::RecursiveSplit => "recursive_split",
            }
            .to_owned();
            if lane != Some(PlePanelLane::Byte) {
                observation.observed_route = "unavailable_before_dispatch".to_owned();
                observation.effective_observation = "unavailable".to_owned();
                observation.capability_observation = format!("carrier_lane={lane:?}");
                observation.availability = ChildAvailability::Unavailable {
                    omission: CapabilityOmission::PleByteLaneUnavailable,
                };
                return Ok(observation);
            }
            reset_max_effective_panel_dispatch_cols();
            let (p, l, e, rank) = banks[bank].ple();
            let effective = max_effective_panel_dispatch_cols();
            {
                let expected = Some(match spec.arm {
                    Arm::Conservative => spec.size,
                    Arm::Asymptotic => spec.size - 1,
                });
                if effective.is_none() {
                    observation.observed_route = "kernel_declined".to_owned();
                    observation.effective_observation = "max_panel_cols=None".to_owned();
                    observation.capability_observation = format!(
                    "carrier_lane=byte panel_byte_lane_max_cols={REQUIRED_PLE_BYTE_LANE_MAX_COLS}"
                );
                    observation.availability = ChildAvailability::Unavailable {
                        omission: CapabilityOmission::PlePanelKernelDeclined,
                    };
                    return Ok(observation);
                }
                if effective != expected {
                    return Err(format!("PLE lane/effective observation {lane:?}/{effective:?}, expected Byte/{expected:?}"));
                }
                let le = scalar_field_matmul(&l, &e);
                if p.apply(&le) != banks[bank] || rank != spec.size {
                    return Err("PLE failed P*(L*E)=A or full-rank contract".to_owned());
                }
                observation.effective_observation =
                    format!("max_panel_cols={}", effective.unwrap());
            }

            observation.capability_observation = format!(
                "carrier_lane=byte panel_byte_lane_max_cols={REQUIRED_PLE_BYTE_LANE_MAX_COLS}"
            );
            digest_tuple(
                b"gf2-calibration-ple-result-v1",
                p.indices().iter().map(|value| value.to_string()).chain([
                    digest_field_matrix(&l),
                    digest_field_matrix(&e),
                    rank.to_string(),
                ]),
            )
        }
        (CalibratedField::PleBlockedBackSubMinDim, FollowOnFixture::PleBackSub(banks)) => {
            observation.observed_route = match back_sub_route(spec.size, spec.size) {
                BackSubRoute::Scalar => "scalar",
                BackSubRoute::Blocked => "blocked",
            }
            .to_owned();
            let (x, r) = banks[bank].rref();
            if scalar_field_matmul(&x, &banks[bank]) != r {
                return Err("RREF failed X*A=R".to_owned());
            }
            let expected_rank = spec.size / 2;
            let nonzero_row_leads = rref_nonzero_row_leads(&r);
            if nonzero_row_leads.len() != expected_rank {
                return Err(format!(
                    "RREF observed {} nonzero rows, expected {expected_rank}: {nonzero_row_leads:?}",
                    nonzero_row_leads.len()
                ));
            }
            let free_cols = spec.size - nonzero_row_leads.len();
            observation.effective_observation =
                format!("rank={} free_cols={free_cols}", nonzero_row_leads.len());
            digest_tuple(
                b"gf2-calibration-rref-result-v1",
                [digest_field_matrix(&x), digest_field_matrix(&r)],
            )
        }
        (CalibratedField::GemmAxpyFastPathMinVolume, FollowOnFixture::Gemm(banks)) => {
            let whole_gemm_available = <Fp251 as FiniteField>::has_simd_gemm_classical();
            if spec.arm == Arm::Asymptotic && !whole_gemm_available {
                observation.observed_route = "unavailable_before_dispatch".to_owned();
                observation.effective_observation = "unavailable".to_owned();
                observation.capability_observation = "fp251_whole_gemm_available=false".to_owned();
                observation.availability = ChildAvailability::Unavailable {
                    omission: CapabilityOmission::Fp251WholeGemmUnavailable,
                };
                return Ok(observation);
            }
            let d = cube_dim(spec.size)?;
            observation.observed_route = match gemm_axpy_route(d, d, d) {
                GemmAxpyRoute::PerCell => "per_cell",
                GemmAxpyRoute::WholeGemm => "whole_gemm",
            }
            .to_owned();
            reset_last_gemm_axpy_dispatch_route();
            let mut result = FieldMatrix::<Fp251>::zeros(d, d);
            run_gemm_axpy_dispatch_for_test(
                &banks[bank].0,
                &banks[paired_bank(bank)].1,
                &mut result,
            );
            let effective = last_gemm_axpy_dispatch_route();
            let expected = match spec.arm {
                Arm::Conservative => GemmAxpyRoute::PerCell,
                Arm::Asymptotic => GemmAxpyRoute::WholeGemm,
            };
            if result != scalar_field_matmul(&banks[bank].0, &banks[paired_bank(bank)].1) {
                return Err("GEMM result differs from scalar product".to_owned());
            }
            observation.capability_observation =
                format!("fp251_whole_gemm_available={whole_gemm_available}");
            observation.availability = classify_gemm_effective(spec.arm, effective)?;
            if let ChildAvailability::Unavailable { .. } = observation.availability {
                observation.observed_route = "per_cell".to_owned();
                observation.effective_observation = "PerCell".to_owned();
                return Ok(observation);
            }
            observation.effective_observation = format!("{expected:?}");
            digest_field_matrix(&result)
        }
        (CalibratedField::InterpolateFastMinPoints, FollowOnFixture::Interpolation(banks)) => {
            observation.observed_route = match interpolate_route(spec.size) {
                InterpolateRoute::Barycentric => "barycentric",
                InterpolateRoute::SubproductTree => "subproduct_tree",
            }
            .to_owned();
            let result = match spec.variant {
                SweepVariant::GenericInterpolation => interpolate_auto(&banks[bank]),
                SweepVariant::TwoAdicInterpolation => interpolate_auto_two_adic(&banks[bank]),
                SweepVariant::Standard => {
                    return Err("interpolation child lacks a dispatcher variant".to_owned())
                }
            }
            .map_err(|error| format!("interpolation failed: {error}"))?;
            if banks[bank].iter().any(|(x, y)| result.eval(x) != *y) {
                return Err("interpolation failed reconstruction".to_owned());
            }
            observation.effective_observation = spec.variant.to_string();
            digest_poly(&result)
        }
        _ => return Err(format!("fixture does not match {}", spec.field)),
    };
    observation.result_digest = result_digest.clone();
    observation.equivalence_digest = digest_tuple(
        b"gf2-calibration-equivalence-v1",
        [observation.operand_digest.clone(), result_digest],
    );
    Ok(observation)
}

/// Executes only the declared production operation for one timed baseline
/// fixture call. Route reporters, digests, and capability probes belong to the
/// untimed child preflight and do not enter this path.
fn execute_direct_timed(spec: ChildSpec, fixture: &mut Fixture, logical_index: usize) {
    let _bank = logical_index & (BIT_FIXTURES - 1);
    match (spec.field, fixture) {
        (CalibratedField::KaratsubaMinDegree, Fixture::Mul { a, b }) => {
            black_box(a.mul(b));
        }
        (CalibratedField::KaratsubaMaxOutLen, Fixture::Mul { a, b }) => {
            black_box(mul_fast(a, b));
        }
        (CalibratedField::DivRemFastMinLen, Fixture::DivRem { dividend, divisor }) => {
            black_box(dividend.div_rem_auto(divisor));
        }
        (CalibratedField::SubproductMinLen, Fixture::BatchEval { poly, points }) => {
            black_box(poly.batch_evaluate_auto(points));
        }
        _ => panic!("timed baseline fixture does not match {}", spec.field),
    }
}

/// Executes only the declared production operation for one timed follow-on
/// fixture call. The SoA case assumes the child already entered its one
/// dedicated four-thread pool.
fn execute_follow_on_timed(spec: ChildSpec, fixture: &FollowOnFixture, logical_index: usize) {
    let bank = logical_index & (BIT_FIXTURES - 1);
    match (spec.field, fixture) {
        (CalibratedField::TransposeSimpleMaxBlocks, FollowOnFixture::Transpose(banks)) => {
            black_box(banks[bank].transpose());
        }
        (
            CalibratedField::SoaParallelMinLen,
            FollowOnFixture::Soa {
                quadratic_lhs,
                quadratic_rhs,
                cubic_lhs,
                cubic_rhs,
            },
        ) => {
            black_box(
                quadratic_lhs[bank]
                    .batch_mul_quadratic::<QuadraticBeta3>(&quadratic_rhs[paired_bank(bank)]),
            );
            black_box(quadratic_lhs[bank].batch_square_quadratic::<QuadraticBeta3>());
            black_box(cubic_lhs[bank].batch_mul_cubic::<CubicBeta3>(&cubic_rhs[paired_bank(bank)]));
            black_box(cubic_lhs[bank].batch_square_cubic::<CubicBeta3>());
        }
        (
            CalibratedField::M4rmWideTierMinStrideWords | CalibratedField::M4rmTiledMinStrideWords,
            FollowOnFixture::M4rm(banks),
        ) => {
            black_box(m4rm_multiply(&banks[bank].0, &banks[paired_bank(bank)].1));
        }
        (CalibratedField::DenseInverseM4riMinDim, FollowOnFixture::BitInverse(banks)) => {
            black_box(invert(&banks[bank]).expect("preflight proved GF(2) fixture invertible"));
        }
        (CalibratedField::DenseInverseBlockedMinDim, FollowOnFixture::FieldInverse(banks)) => {
            black_box(
                banks[bank]
                    .inv()
                    .expect("preflight proved Fp251 fixture invertible"),
            );
        }
        (CalibratedField::TrsmBlockedMinDim, FollowOnFixture::Trsm(banks)) => {
            black_box(
                banks[bank]
                    .0
                    .solve_batch_quiet_for_test(&banks[paired_bank(bank)].1)
                    .expect("preflight proved TRSM fixture invertible"),
            );
        }
        (CalibratedField::PlePanelBaseMaxCols, FollowOnFixture::PlePanel(banks)) => {
            black_box(banks[bank].ple());
        }
        (CalibratedField::PleBlockedBackSubMinDim, FollowOnFixture::PleBackSub(banks)) => {
            black_box(banks[bank].rref());
        }
        (CalibratedField::InterpolateFastMinPoints, FollowOnFixture::Interpolation(banks)) => {
            let result = match spec.variant {
                SweepVariant::GenericInterpolation => interpolate_auto(&banks[bank]),
                SweepVariant::TwoAdicInterpolation => interpolate_auto_two_adic(&banks[bank]),
                SweepVariant::Standard => panic!("timed interpolation lacks a dispatcher variant"),
            }
            .expect("preflight proved interpolation fixture valid");
            black_box(result);
        }
        _ => panic!("timed fixture does not match {}", spec.field),
    }
}

/// The detected SIMD logical backend, or `None` when the build or the host has
/// no second arm for the bit-backend family.
fn simd_backend() -> Option<&'static dyn Backend> {
    #[cfg(feature = "simd")]
    {
        gf2_core::kernels::simd::maybe_simd().map(|backend| backend as &'static dyn Backend)
    }
    #[cfg(not(feature = "simd"))]
    {
        None
    }
}

// ---------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------

use tuning_campaign_support::timing::TimingSample;

/// Destination and source bank indices for call number `index`, offset so a
/// call never reads and writes the same buffer.
fn bank_indices(index: usize) -> (usize, usize) {
    (
        index & (BIT_FIXTURES - 1),
        index.wrapping_add(3) & (BIT_FIXTURES - 1),
    )
}

/// Calibrates one call count against the target duration, then records
/// `repetitions` timed windows of exactly that many calls.
fn execution_windows(
    protocol: &Protocol,
    execution: u64,
    timing_context: Option<&(
        tuning_campaign_support::campaign::UnitIdentity,
        tuning_campaign_support::campaign::Sha256Digest,
    )>,
    mut body: impl FnMut(usize),
) -> Vec<TimingSample> {
    if let Some((identity, digest)) = timing_context {
        assert_eq!(protocol, &campaign_owner::fixed_protocol());
        return tuning_campaign_support::campaign::execution_with_progress(
            identity,
            digest,
            &mut body,
            &mut io::stderr().lock(),
        )
        .expect("campaign timing and progress writes must succeed");
    }
    tuning_campaign_support::timing::execution_windows_configured(
        execution,
        protocol.repetitions,
        protocol.target(),
        &mut body,
        |_| Ok(()),
    )
    .expect("validated test timing protocol")
}

// ---------------------------------------------------------------------
// Forcing an arm in a child process
// ---------------------------------------------------------------------

/// What a child process reports back on its standard output.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ChildReport {
    /// Protocol identities and sampling dimensions the child observed.
    protocol: ChildProtocolIdentity,
    /// Exact installed value and strict format-2 section identities.
    installed: InstalledEvidence,
    /// Exact fixture shape predeclared for the scalar grid value.
    fixture_shape: String,
    /// Complete deterministic seed inventory for this fixture.
    seed_inventory: SeedInventory,
    /// Domain-separated SHA-256 of all ordered operands.
    operand_digest: String,
    /// Either complete arm evidence or a closed capability omission.
    outcome: ChildOutcome,
}

/// Evidence emitted by a child after common install/fixture validation.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "status", deny_unknown_fields)]
enum ChildOutcome {
    Complete {
        requested_route: String,
        observed_route: String,
        /// Required preflight evidence for both Probe and Measure tasks.
        effective_observation: Option<String>,
        /// Required preflight evidence for both Probe and Measure tasks.
        capability_observation: Option<String>,
        result_digest: String,
        equivalence_digest: String,
        samples: Vec<TimingSample>,
    },
    Unavailable {
        requested_route: String,
        observed_route: String,
        omission: CapabilityOmission,
        effective_observation: String,
        capability_observation: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OperationObservation {
    observed_route: String,
    effective_observation: String,
    capability_observation: String,
    fixture_shape: String,
    operand_digest: String,
    result_digest: String,
    equivalence_digest: String,
    availability: ChildAvailability,
}

impl Default for OperationObservation {
    fn default() -> Self {
        Self {
            observed_route: String::new(),
            effective_observation: String::new(),
            capability_observation: String::new(),
            fixture_shape: String::new(),
            operand_digest: String::new(),
            result_digest: String::new(),
            equivalence_digest: String::new(),
            availability: ChildAvailability::Available,
        }
    }
}

/// Classifies the production observer after checking the GEMM result.
fn classify_gemm_effective(
    arm: Arm,
    effective: Option<GemmAxpyRoute>,
) -> Result<ChildAvailability, String> {
    match (arm, effective) {
        (Arm::Conservative, Some(GemmAxpyRoute::PerCell))
        | (Arm::Asymptotic, Some(GemmAxpyRoute::WholeGemm)) => Ok(ChildAvailability::Available),
        (Arm::Asymptotic, Some(GemmAxpyRoute::PerCell)) => Ok(ChildAvailability::Unavailable {
            omission: CapabilityOmission::GemmWholeKernelDeclined,
        }),
        _ => Err(format!(
            "GEMM effective route {effective:?} does not match {arm}"
        )),
    }
}

/// Closed reasons a predeclared comparison can be unavailable on a host.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "reason", deny_unknown_fields)]
enum CapabilityOmission {
    SimdBackendUnavailable,
    SoaPoolWidth { observed: usize },
    M4rmRegisterTiledUnavailable,
    PleByteLaneUnavailable,
    PlePanelKernelDeclined,
    Fp251WholeGemmUnavailable,
    TrsmBlockedCalleeDeclined,
    GemmWholeKernelDeclined,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ChildAvailability {
    Available,
    Unavailable { omission: CapabilityOmission },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ChildProtocolIdentity {
    fresh_case_schema: String,
    profile_format_version: u32,
    section_id: String,
    section_schema_version: u32,
    harness_schema: String,
    raw_sample_schema: String,
    timing: Protocol,
}

impl ChildProtocolIdentity {
    fn current(protocol: &Protocol) -> Self {
        Self {
            fresh_case_schema: FRESH_CASE_VALUE.to_owned(),
            profile_format_version: PROFILE_FORMAT_VERSION,
            section_id: CoreTuning::ID.as_str().to_owned(),
            section_schema_version: CoreTuningCodec::SCHEMA_VERSION,
            harness_schema: CoreTuningCodec::HARNESS_SCHEMA.to_owned(),
            raw_sample_schema: RAW_SAMPLE_SCHEMA.to_owned(),
            timing: protocol.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ObservedResolution {
    Installed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ObservedMeasurement {
    Inherited,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct InstalledEvidence {
    profile_id: String,
    section_id: String,
    resolution: ObservedResolution,
    measurement: ObservedMeasurement,
    active_values: Vec<ForcedValue>,
    section_sha256: String,
    envelope_content_sha256: String,
}

/// One selector or companion control that a fresh child must read back.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ForcedValue {
    family: String,
    field: String,
    value: usize,
}

impl ForcedValue {
    fn new(family: &str, field: &str, value: usize) -> Self {
        Self {
            family: family.to_owned(),
            field: field.to_owned(),
            value,
        }
    }
}

/// The `karatsuba_min_degree` a child installs to reach `arm` at `size`.
///
/// The Karatsuba arm forces the grid point itself, which is the smallest value
/// that still routes operands of that degree to Karatsuba. `mul_karatsuba_raw`
/// recurses on the same value, so the recursion splits once and its
/// sub-operands, at about half the degree, fall to the schoolbook base case —
/// the algorithm the dispatcher runs when `karatsuba_min_degree` is set to this
/// grid point. Forcing the bottom of the admissible range instead would time a
/// recursion carried to degree 0, which no threshold produces.
///
/// The schoolbook arm forces the top of the range, where no grid point routes
/// to Karatsuba at all.
fn forced_karatsuba_min_degree(arm: Arm, size: usize) -> usize {
    match arm {
        Arm::Conservative => FORCED_SCHOOLBOOK_MIN_DEGREE,
        Arm::Asymptotic => size,
    }
}

/// Builds the exact core section that forces one field's requested arm and
/// pins any protocol-declared companion controls.
fn forced_profile_for(spec: ChildSpec) -> Result<(PreparedEnvelope, Vec<ForcedValue>), String> {
    let mut selectors = CoreSelectors::CONSERVATIVE.clone();
    if selectors.soa_batch.parallel_chunk_len() != REQUIRED_SOA_PARALLEL_CHUNK_LEN {
        return Err(format!(
            "the conservative SoA chunk is {}, protocol requires {REQUIRED_SOA_PARALLEL_CHUNK_LEN}",
            selectors.soa_batch.parallel_chunk_len()
        ));
    }
    if selectors.ple.panel_byte_lane_max_cols() != REQUIRED_PLE_BYTE_LANE_MAX_COLS {
        return Err(format!(
            "the conservative PLE byte-lane ceiling is {}, protocol requires {REQUIRED_PLE_BYTE_LANE_MAX_COLS}",
            selectors.ple.panel_byte_lane_max_cols()
        ));
    }
    if selectors.triangular.trsm_panel_rows() != REQUIRED_TRSM_PANEL_ROWS {
        return Err(format!(
            "the conservative TRSM panel is {}, protocol requires {REQUIRED_TRSM_PANEL_ROWS}",
            selectors.triangular.trsm_panel_rows()
        ));
    }
    let forced = |below: usize, above: usize| match spec.arm {
        Arm::Conservative => below,
        Arm::Asymptotic => above,
    };
    let mut values = Vec::new();
    match spec.field {
        CalibratedField::SimdMinWords => {
            selectors.bit_backend =
                BitBackendSelectors::try_new(spec.size).map_err(|error| error.to_string())?;
            values.push(ForcedValue::new("bit_backend", "simd_min_words", spec.size));
        }
        CalibratedField::KaratsubaMinDegree => {
            let value = forced_karatsuba_min_degree(spec.arm, spec.size);
            let p = &selectors.polynomial;
            selectors.polynomial = PolynomialSelectors::try_new(
                value,
                p.karatsuba_max_out_len(),
                p.div_rem_fast_min_len(),
                p.subproduct_min_len(),
                p.interpolate_fast_min_points(),
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "polynomial",
                "karatsuba_min_degree",
                value,
            ));
        }
        CalibratedField::KaratsubaMaxOutLen => {
            let value = forced(spec.size, spec.size - 1);
            let p = &selectors.polynomial;
            selectors.polynomial = PolynomialSelectors::try_new(
                p.karatsuba_min_degree(),
                value,
                p.div_rem_fast_min_len(),
                p.subproduct_min_len(),
                p.interpolate_fast_min_points(),
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "polynomial",
                "karatsuba_max_out_len",
                value,
            ));
        }
        CalibratedField::DivRemFastMinLen => {
            let value = forced(spec.size + 1, spec.size);
            let p = &selectors.polynomial;
            selectors.polynomial = PolynomialSelectors::try_new(
                p.karatsuba_min_degree(),
                p.karatsuba_max_out_len(),
                value,
                p.subproduct_min_len(),
                p.interpolate_fast_min_points(),
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "polynomial",
                "div_rem_fast_min_len",
                value,
            ));
        }
        CalibratedField::SubproductMinLen | CalibratedField::InterpolateFastMinPoints => {
            let value = forced(spec.size + 1, spec.size);
            let p = &selectors.polynomial;
            let (subproduct, interpolation) = if spec.field == CalibratedField::SubproductMinLen {
                (value, p.interpolate_fast_min_points())
            } else {
                (p.subproduct_min_len(), value)
            };
            selectors.polynomial = PolynomialSelectors::try_new(
                p.karatsuba_min_degree(),
                p.karatsuba_max_out_len(),
                p.div_rem_fast_min_len(),
                subproduct,
                interpolation,
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "polynomial",
                &spec.field.to_string(),
                value,
            ));
        }
        CalibratedField::TransposeSimpleMaxBlocks => {
            let value = forced(spec.size, spec.size - 1);
            let p = &selectors.bit_matrix;
            selectors.bit_matrix = BitMatrixSelectors::try_new(
                p.matvec_simd_min_words(),
                value,
                p.transpose_macro_tile_blocks(),
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "bit_matrix",
                "transpose_simple_max_blocks",
                value,
            ));
        }
        CalibratedField::SoaParallelMinLen => {
            let value = forced(spec.size + 1, spec.size);
            let chunk = REQUIRED_SOA_PARALLEL_CHUNK_LEN;
            selectors.soa_batch =
                SoaBatchSelectors::try_new(value, chunk).map_err(|error| error.to_string())?;
            values.push(ForcedValue::new("soa_batch", "parallel_min_len", value));
            values.push(ForcedValue::new("soa_batch", "parallel_chunk_len", chunk));
        }
        CalibratedField::M4rmWideTierMinStrideWords => {
            let value = forced(spec.size + 1, spec.size);
            let p = &selectors.m4rm;
            selectors.m4rm = M4rmSelectors::try_new(
                value,
                usize::MAX,
                p.default_table_bytes(),
                p.mid_table_bytes(),
                p.wide_table_bytes(),
                p.wide_max_k(),
                p.small_n_max_k(),
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "m4rm",
                "wide_tier_min_stride_words",
                value,
            ));
            values.push(ForcedValue::new(
                "m4rm",
                "tiled_min_stride_words",
                usize::MAX,
            ));
        }
        CalibratedField::M4rmTiledMinStrideWords => {
            let value = forced(spec.size + 1, spec.size);
            let p = &selectors.m4rm;
            selectors.m4rm = M4rmSelectors::try_new(
                p.wide_tier_min_stride_words(),
                value,
                p.default_table_bytes(),
                p.mid_table_bytes(),
                p.wide_table_bytes(),
                p.wide_max_k(),
                p.small_n_max_k(),
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new("m4rm", "tiled_min_stride_words", value));
        }
        CalibratedField::DenseInverseM4riMinDim | CalibratedField::DenseInverseBlockedMinDim => {
            let value = forced(spec.size + 1, spec.size);
            let p = &selectors.dense_inverse;
            let (m4ri, blocked) = if spec.field == CalibratedField::DenseInverseM4riMinDim {
                (value, p.blocked_min_dim())
            } else {
                (p.m4ri_min_dim(), value)
            };
            selectors.dense_inverse =
                DenseInverseSelectors::try_new(m4ri, blocked).map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "dense_inverse",
                &spec.field.to_string(),
                value,
            ));
        }
        CalibratedField::TrsmBlockedMinDim => {
            let value = forced(spec.size + 1, spec.size);
            let p = &selectors.triangular;
            let panel_rows = REQUIRED_TRSM_PANEL_ROWS;
            selectors.triangular =
                TriangularSelectors::try_new(value, panel_rows, p.base_case_max_dim())
                    .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new(
                "triangular",
                "trsm_blocked_min_dim",
                value,
            ));
            values.push(ForcedValue::new(
                "triangular",
                "trsm_panel_rows",
                panel_rows,
            ));
        }
        CalibratedField::PlePanelBaseMaxCols | CalibratedField::PleBlockedBackSubMinDim => {
            let value = if spec.field == CalibratedField::PlePanelBaseMaxCols {
                forced(spec.size, spec.size - 1)
            } else {
                forced(spec.size + 1, spec.size)
            };
            let p = &selectors.ple;
            let (panel_base, back_sub) = if spec.field == CalibratedField::PlePanelBaseMaxCols {
                (value, p.blocked_back_sub_min_dim())
            } else {
                (p.panel_base_max_cols(), value)
            };
            let byte_lane = if spec.field == CalibratedField::PlePanelBaseMaxCols {
                REQUIRED_PLE_BYTE_LANE_MAX_COLS
            } else {
                p.panel_byte_lane_max_cols()
            };
            selectors.ple = PleSelectors::try_new(
                panel_base,
                back_sub,
                p.scalar_base_max_cols(),
                byte_lane,
                p.panel_u16_lane_max_cols(),
            )
            .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new("ple", &spec.field.to_string(), value));
            if spec.field == CalibratedField::PlePanelBaseMaxCols {
                values.push(ForcedValue::new(
                    "ple",
                    "panel_byte_lane_max_cols",
                    byte_lane,
                ));
            }
        }
        CalibratedField::GemmAxpyFastPathMinVolume => {
            let value = forced(spec.size + 1, spec.size);
            let p = &selectors.gemm;
            selectors.gemm =
                GemmSelectors::try_new(p.row_tile(), p.col_tile(), value, p.winograd_min_dim())
                    .map_err(|error| error.to_string())?;
            values.push(ForcedValue::new("gemm", "axpy_fast_path_min_volume", value));
        }
    }
    let id = ProfileId::parse(FORCED_ARM_PROFILE_ID)
        .map_err(|error| format!("`{FORCED_ARM_PROFILE_ID}` is not a profile id: {error}"))?;
    let section = CoreTuning::from_selectors(selectors);
    let profile =
        PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
            .insert(section)
            .map_err(|error| format!("the arm-forcing section does not prepare: {error}"))?
            .build()
            .map_err(|error| format!("the arm-forcing envelope does not build: {error}"))?;
    Ok((profile, values))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ForcedProfileDigests {
    section_sha256: String,
    envelope_content_sha256: String,
}

/// Computes strict format-2 identity for the exact typed value that is installed.
///
/// `ProfileRegistry::to_json` needs assembly provenance even though the
/// format-2 content digest excludes assembly. These constants are an
/// identity-only encoding scaffold and are never emitted as observed
/// provenance. The returned envelope digest is the registry's verified
/// format-2 digest, while the section digest is SHA-256 over the canonical
/// `sections["gf2-core/selectors"]` wire value.
fn forced_profile_digests(profile: &PreparedEnvelope) -> Result<ForcedProfileDigests, String> {
    let identity_assembly = AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("1970-01-01T00:00:00Z")
            .map_err(|error| format!("identity timestamp is invalid: {error}"))?,
        source_revision: GitRevision::parse(&"0".repeat(40))
            .map_err(|error| format!("identity revision is invalid: {error}"))?,
        source_dirty: false,
        tool: RepoRelPath::parse("crates/gf2-core/benches/tuning_calibration.rs")
            .map_err(|error| format!("identity tool path is invalid: {error}"))?,
        tool_sha256: Sha256::parse(&"0".repeat(64))
            .map_err(|error| format!("identity tool digest is invalid: {error}"))?,
    };
    let registry = core_registry().map_err(|error| format!("core registry is invalid: {error}"))?;
    let document = registry
        .to_json(profile, &identity_assembly)
        .map_err(|error| format!("the forced profile does not encode: {error}"))?;
    let reopened = registry
        .from_json(&document)
        .map_err(|error| format!("the forced profile does not strictly reopen: {error}"))?;
    let ids: Vec<&str> = reopened.section_ids().collect();
    if ids != [CoreTuning::ID.as_str()] {
        return Err(format!("the forced profile has section IDs {ids:?}"));
    }
    let envelope_content_sha256 = reopened
        .verified_assembly()
        .ok_or("the forced format-2 profile has no verified assembly")?
        .content_sha256
        .as_str()
        .to_owned();
    let wire: serde_json::Value = serde_json::from_str(&document)
        .map_err(|error| format!("the forced profile is not JSON: {error}"))?;
    let section = wire
        .pointer("/sections/gf2-core~1selectors")
        .ok_or("the forced profile has no core section wrapper")?;
    let section_bytes = serde_json::to_vec(section)
        .map_err(|error| format!("the forced section wrapper is not canonical JSON: {error}"))?;
    let section_sha256 = format!("{:x}", Sha256Hasher::digest(section_bytes));
    Ok(ForcedProfileDigests {
        section_sha256,
        envelope_content_sha256,
    })
}

/// Installs and reads back the exact profile that forces one multiplication arm.
///
/// The resolution and installed value are observed through `tuning::active`.
/// A lost install, missing section, frozen process, wrong measurement kind, or
/// identity mismatch fails before the child can report a successful result.
fn install_forced_profile(spec: ChildSpec) -> Result<InstalledEvidence, String> {
    let (profile, expected_values) = forced_profile_for(spec)?;
    install_prepared_profile(profile, expected_values)
}

fn install_prepared_profile(
    profile: PreparedEnvelope,
    expected_values: Vec<ForcedValue>,
) -> Result<InstalledEvidence, String> {
    let digests = forced_profile_digests(&profile)?;
    let identity_assembly = AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("1970-01-01T00:00:00Z").map_err(|e| e.to_string())?,
        source_revision: GitRevision::parse(&"0".repeat(40)).map_err(|e| e.to_string())?,
        source_dirty: false,
        tool: RepoRelPath::parse("crates/gf2-core/benches/tuning_calibration.rs")
            .map_err(|e| e.to_string())?,
        tool_sha256: Sha256::parse(&"0".repeat(64)).map_err(|e| e.to_string())?,
    };
    let registry = core_registry().map_err(|error| format!("core registry is invalid: {error}"))?;
    let document = registry
        .to_json(&profile, &identity_assembly)
        .map_err(|error| format!("the forced profile does not encode: {error}"))?;
    let reopened = registry
        .from_json(&document)
        .map_err(|error| format!("the forced profile does not strictly reopen: {error}"))?;
    let reencoded = registry
        .to_json(&reopened, &identity_assembly)
        .map_err(|error| format!("the reopened forced profile does not encode: {error}"))?;
    if reencoded != document {
        return Err("the reopened forced profile is not canonical".to_owned());
    }
    tuning::install(reopened)
        .map_err(|error| format!("the arm-forcing profile was not installed: {error}"))?;
    let active_tuning = tuning::active();
    let (profile_id, section_id, measurement) = match active_tuning.resolution {
        tuning::SectionResolution::Installed {
            profile_id,
            section_id,
            measurement,
        } => (profile_id, section_id, measurement),
        other => {
            return Err(format!(
                "the arm-forcing core section did not resolve as Installed: {other:?}"
            ))
        }
    };
    if profile_id.as_str() != FORCED_ARM_PROFILE_ID {
        return Err(format!(
            "the installed profile is {}, not {FORCED_ARM_PROFILE_ID}",
            profile_id.as_str()
        ));
    }
    if section_id != CoreTuning::ID {
        return Err(format!(
            "the installed section is {}, not {}",
            section_id.as_str(),
            CoreTuning::ID.as_str()
        ));
    }
    if measurement != &MeasurementProvenance::Inherited {
        return Err(format!(
            "the forced section carries unexpected measurement provenance: {measurement:?}"
        ));
    }
    let active_body = CoreTuningCodec::encode_body(active_tuning.section)
        .map_err(|error| format!("the active forced section does not encode: {error}"))?;
    let active_body = serde_json::to_value(active_body)
        .map_err(|error| format!("the active forced section is not JSON: {error}"))?;
    for expected in &expected_values {
        let pointer = format!("/{}/{}", expected.family, expected.field);
        let observed = active_body
            .pointer(&pointer)
            .and_then(serde_json::Value::as_u64);
        if observed != Some(expected.value as u64) {
            return Err(format!(
                "the active forced value {pointer} is {observed:?}, not {}",
                expected.value
            ));
        }
    }
    Ok(InstalledEvidence {
        profile_id: profile_id.as_str().to_owned(),
        section_id: section_id.as_str().to_owned(),
        resolution: ObservedResolution::Installed,
        measurement: ObservedMeasurement::Inherited,
        active_values: expected_values,
        section_sha256: digests.section_sha256,
        envelope_content_sha256: digests.envelope_content_sha256,
    })
}

/// Runs one child task and returns its structured report.
///
/// The profile is installed before anything else touches a selection boundary,
/// so `tuning::install` cannot fail on an already-resolved profile. The arm is
/// then read back from the production selector rather than assumed from the
/// value installed.
#[cfg(test)]
#[allow(dead_code)]
fn run_child(spec: ChildSpec, protocol: &Protocol) -> Result<ChildReport, String> {
    run_child_with_context(spec, protocol, None)
}

fn run_child_with_context(
    spec: ChildSpec,
    protocol: &Protocol,
    timing_context: Option<&(
        tuning_campaign_support::campaign::UnitIdentity,
        tuning_campaign_support::campaign::Sha256Digest,
    )>,
) -> Result<ChildReport, String> {
    if spec.field == CalibratedField::InterpolateFastMinPoints {
        if spec.variant == SweepVariant::Standard {
            return Err("interpolation requires generic or two_adic variant".to_owned());
        }
    } else if spec.variant != SweepVariant::Standard {
        return Err(format!(
            "{} does not admit variant {}",
            spec.field, spec.variant
        ));
    }
    let installed = install_forced_profile(spec)?;
    let expected_full = campaign_owner::section_values(&forced_profile_for(spec)?.0)?;
    let active_full =
        campaign_owner::flatten_selectors(&complete_selector_value(tuning::active().section)?)?;
    if active_full != expected_full {
        return Err(
            "retained child failed complete selector readback before fixture construction"
                .to_owned(),
        );
    }
    enum ChildFixture {
        Direct(Fixture),
        FollowOn(FollowOnFixture),
    }
    let mut fixture = if spec.field.seed_tag() <= 4 {
        ChildFixture::Direct(build_fixture(spec.field, spec.size))
    } else {
        ChildFixture::FollowOn(build_follow_on_fixture(spec.field, spec.size)?)
    };
    let finish = |fixture: &mut ChildFixture| -> Result<ChildReport, String> {
        let observation = match fixture {
            ChildFixture::Direct(fixture) => execute_direct(spec, fixture, 0)?,
            ChildFixture::FollowOn(fixture) => execute_follow_on(spec, fixture, 0)?,
        };
        let expected_route = spec.field.arm_name(spec.arm);
        Sha256::parse(&observation.operand_digest)
            .map_err(|_| format!("{spec} emitted malformed operand digest"))?;
        if let ChildAvailability::Unavailable { omission } = observation.availability {
            return Ok(ChildReport {
                protocol: ChildProtocolIdentity::current(protocol),
                installed,
                fixture_shape: observation.fixture_shape,
                seed_inventory: seed_inventory(spec.field, spec.size),
                operand_digest: observation.operand_digest,
                outcome: ChildOutcome::Unavailable {
                    requested_route: expected_route.to_owned(),
                    observed_route: observation.observed_route,
                    omission,
                    effective_observation: observation.effective_observation,
                    capability_observation: observation.capability_observation,
                },
            });
        }
        if observation.observed_route != expected_route {
            return Err(format!(
                "{} observed route {}, expected {expected_route}",
                spec, observation.observed_route
            ));
        }
        for digest in [&observation.result_digest, &observation.equivalence_digest] {
            Sha256::parse(digest)
                .map_err(|_| format!("{spec} emitted malformed digest {digest}"))?;
        }
        verify_probe_observations(
            spec,
            &observation.effective_observation,
            &observation.capability_observation,
        )?;
        // Check every retained bank without changing the original timed bit buffers.
        // Polynomial fixtures are single deterministic objects, independent of bank index.
        for bank in 1..BIT_FIXTURES {
            let secondary = match fixture {
                ChildFixture::Direct(Fixture::Bit { .. }) => {
                    let mut witness = build_fixture(spec.field, spec.size);
                    execute_direct(spec, &mut witness, bank)?
                }
                ChildFixture::Direct(_) => break,
                ChildFixture::FollowOn(witness) => execute_follow_on(spec, witness, bank)?,
            };
            if secondary.availability != ChildAvailability::Available
                || secondary.observed_route != expected_route
            {
                return Err(format!(
                    "retained fixture bank {bank} changed route or availability"
                ));
            }
            verify_probe_observations(
                spec,
                &secondary.effective_observation,
                &secondary.capability_observation,
            )?;
        }
        let samples = if let ChildTask::Measure { execution } = spec.task {
            let samples = match fixture {
                ChildFixture::Direct(Fixture::Bit { dst, src }) => match spec.arm {
                    Arm::Conservative => {
                        execution_windows(protocol, execution, timing_context, |index| {
                            let (left, right) = bank_indices(index);
                            let source: &[u64] = black_box(src.get(right));
                            let destination: &mut [u64] = black_box(dst.get_mut(left));
                            ScalarBackend.xor(destination, source);
                            black_box(destination);
                        })
                    }
                    Arm::Asymptotic => {
                        let backend = gf2_core::kernels::simd::maybe_simd()
                            .ok_or("the timed concrete SIMD backend is unavailable")?;
                        execution_windows(protocol, execution, timing_context, |index| {
                            let (left, right) = bank_indices(index);
                            let source: &[u64] = black_box(src.get(right));
                            let destination: &mut [u64] = black_box(dst.get_mut(left));
                            backend.xor(destination, source);
                            black_box(destination);
                        })
                    }
                },
                ChildFixture::FollowOn(FollowOnFixture::Gemm(banks)) => {
                    let d = cube_dim(spec.size)?;
                    let mut outputs: Vec<FieldMatrix<Fp251>> = (0..BIT_FIXTURES)
                        .map(|_| FieldMatrix::zeros(d, d))
                        .collect();
                    execution_windows(protocol, execution, timing_context, |index| {
                        let bank = index & (BIT_FIXTURES - 1);
                        run_gemm_axpy_dispatch_for_test(
                            &banks[bank].0,
                            &banks[paired_bank(bank)].1,
                            &mut outputs[bank],
                        );
                        black_box(&outputs[bank]);
                    })
                }
                _ => {
                    execution_windows(protocol, execution, timing_context, |index| match fixture {
                        ChildFixture::Direct(fixture) => {
                            execute_direct_timed(spec, fixture, index);
                        }
                        ChildFixture::FollowOn(fixture) => {
                            execute_follow_on_timed(spec, fixture, index);
                        }
                    })
                }
            };
            samples
        } else {
            Vec::new()
        };
        Ok(ChildReport {
            protocol: ChildProtocolIdentity::current(protocol),
            installed,
            fixture_shape: observation.fixture_shape,
            seed_inventory: seed_inventory(spec.field, spec.size),
            operand_digest: observation.operand_digest,
            outcome: ChildOutcome::Complete {
                requested_route: expected_route.to_owned(),
                observed_route: observation.observed_route,
                effective_observation: Some(observation.effective_observation),
                capability_observation: Some(observation.capability_observation),
                result_digest: observation.result_digest,
                equivalence_digest: observation.equivalence_digest,
                samples,
            },
        })
    };
    if spec.field == CalibratedField::SoaParallelMinLen {
        run_in_dedicated_parallel_pool(4, || finish(&mut fixture))
    } else {
        finish(&mut fixture)
    }
}

/// Chooses the entry protocol for a fresh tuning child.
///
/// The benchmark target uses its own guarded CLI. The integration-test target
/// is a libtest executable, so its test adapter must be selected explicitly;
/// `cargo bench` also enables `cfg(test)` for a `harness = false` target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FreshChildEntry {
    BenchmarkCli,
    LibtestAdapter,
}

impl FreshChildEntry {
    fn arguments(self) -> &'static [&'static str] {
        match self {
            Self::BenchmarkCli => &["--fresh-tuning-process-child"],
            Self::LibtestAdapter => &[
                "--exact",
                "tuning_calibration::tests::fresh_process_entry",
                "--nocapture",
            ],
        }
    }
}

/// Executes this harness for one guarded forced-tuning case.
fn fresh_tuning_process(
    executable: &Path,
    case: FreshProcessCase,
    entry: FreshChildEntry,
) -> Result<ChildReport, String> {
    if entry == FreshChildEntry::BenchmarkCli
        && matches!(case.spec.task, ChildTask::Measure { .. })
        && env::var(BENCH_MODE_VAR).as_deref() != Ok("1")
    {
        return Err(format!(
            "timed calibration children require {BENCH_MODE_VAR}=1 in the parent"
        ));
    }
    let input = serde_json::to_string(&case)
        .map_err(|error| format!("cannot encode the fresh tuning case: {error}"))?;
    let mut command = Command::new(executable);
    command.args(entry.arguments());
    let mut child = command
        .env(FRESH_CASE_VAR, FRESH_CASE_VALUE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "cannot run {} for {}: {error}",
                executable.display(),
                case.spec
            )
        })?;
    child
        .stdin
        .take()
        .ok_or("the fresh tuning child has no standard input")?
        .write_all(input.as_bytes())
        .map_err(|error| format!("cannot write the fresh tuning case: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("cannot wait for the fresh tuning child: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "the child for {} exited with {}\nstdout:\n{}\nstderr:\n{}",
            case.spec,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "the fresh tuning child emitted non-UTF-8 output".to_owned())?;
    // Strip libtest's own progress lines, retaining every protocol line so
    // duplicate/malformed reports still fail the production parser.
    let stdout = match entry {
        FreshChildEntry::BenchmarkCli => stdout,
        FreshChildEntry::LibtestAdapter => stdout
            .lines()
            .filter(|line| line.starts_with(FRESH_RESULT_PREFIX))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    parse_child_report(&stdout)
}

#[cfg(test)]
#[allow(dead_code)]
fn decode_fresh_case(input: &str) -> Result<FreshProcessCase, String> {
    tuning_campaign_support::transport::decode_case(input)
}

#[cfg(test)]
#[allow(dead_code)]
fn read_fresh_case() -> Result<FreshProcessCase, String> {
    tuning_campaign_support::transport::read_guarded_case(
        env::var(FRESH_CASE_VAR).ok().as_deref(),
        io::stdin().lock(),
    )
}

#[cfg(test)]
#[allow(dead_code)]
fn run_fresh_child() -> Result<(), String> {
    let case = read_fresh_case()?;
    let report = run_child(case.spec, &case.protocol)?;
    tuning_campaign_support::transport::write_result_line(io::stdout().lock(), &report)
        .map_err(|error| error.to_string())
}

/// Reads the one guarded structured result from a child's standard output.
fn parse_child_report(text: &str) -> Result<ChildReport, String> {
    tuning_campaign_support::transport::parse_result(text)
}

/// Checks a child's report against what the parent asked for and holds.
fn verify_child_report(
    spec: ChildSpec,
    expected_operand_digest: &str,
    protocol: &Protocol,
    report: &ChildReport,
) -> Result<(), String> {
    let expected_arm = spec.field.arm_name(spec.arm);
    let expected_protocol = ChildProtocolIdentity::current(protocol);
    if report.protocol != expected_protocol {
        return Err(format!(
            "the child for {spec} reported protocol {:?}, expected {expected_protocol:?}",
            report.protocol
        ));
    }
    let (_, forced_values) = forced_profile_for(spec)?;
    if report.installed.profile_id != FORCED_ARM_PROFILE_ID
        || report.installed.section_id != CoreTuning::ID.as_str()
        || report.installed.resolution != ObservedResolution::Installed
        || report.installed.measurement != ObservedMeasurement::Inherited
        || report.installed.active_values != forced_values
    {
        return Err(format!(
            "the child for {spec} did not report the exact Installed forced section: {:?}",
            report.installed
        ));
    }
    let expected_digests = forced_profile_digests(&forced_profile_for(spec)?.0)?;
    if report.installed.section_sha256 != expected_digests.section_sha256
        || report.installed.envelope_content_sha256 != expected_digests.envelope_content_sha256
    {
        return Err(format!(
            "the child for {spec} reported forced-profile digests {:?}, expected {expected_digests:?}",
            report.installed
        ));
    }
    Sha256::parse(&report.installed.section_sha256)
        .map_err(|_| format!("the child for {spec} reported a malformed section digest"))?;
    Sha256::parse(&report.installed.envelope_content_sha256)
        .map_err(|_| format!("the child for {spec} reported a malformed envelope digest"))?;
    let expected_seeds = seed_inventory(spec.field, spec.size);
    if report.seed_inventory != expected_seeds {
        return Err(format!(
            "the child for {spec} reported a stale seed inventory"
        ));
    }
    let expected_shape = expected_fixture_shape(spec.field, spec.size)?;
    if report.fixture_shape != expected_shape {
        return Err(format!(
            "the child for {spec} reported fixture shape {}, expected {expected_shape}",
            report.fixture_shape
        ));
    }
    Sha256::parse(&report.operand_digest)
        .map_err(|_| format!("the child for {spec} reported malformed operand digest"))?;
    if report.operand_digest != expected_operand_digest {
        return Err(format!(
            "the child for {spec} built operands outside the parent's deterministic fixture"
        ));
    }
    match &report.outcome {
        ChildOutcome::Unavailable {
            requested_route,
            observed_route,
            omission,
            effective_observation,
            capability_observation,
        } => {
            if !matches!(spec.task, ChildTask::Probe) {
                return Err(format!(
                    "timed child for {spec} reported capability omission {omission:?}"
                ));
            }
            verify_unavailable_routes(
                spec,
                expected_arm,
                requested_route,
                observed_route,
                omission,
            )?;
            verify_capability_omission(spec, omission, capability_observation)?;
            if effective_observation != expected_omission_effective(omission) {
                return Err(format!(
                    "{spec} has incorrect omission effective evidence: {effective_observation}"
                ));
            }
        }
        ChildOutcome::Complete {
            requested_route,
            observed_route,
            effective_observation,
            capability_observation,
            result_digest,
            equivalence_digest,
            samples,
        } => {
            verify_routes(spec, expected_arm, requested_route, observed_route)?;
            for (name, digest) in [
                ("result", result_digest.as_str()),
                ("equivalence", equivalence_digest.as_str()),
            ] {
                Sha256::parse(digest).map_err(|_| {
                    format!("the child for {spec} reported malformed {name} digest")
                })?;
            }
            if *equivalence_digest
                != digest_tuple(
                    b"gf2-calibration-equivalence-v1",
                    [report.operand_digest.clone(), result_digest.clone()],
                )
            {
                return Err(format!("the child for {spec} reported an equivalence digest outside its operand/result tuple"));
            }
            let effective = effective_observation
                .as_deref()
                .ok_or_else(|| format!("child for {spec} omitted effective evidence"))?;
            let capability = capability_observation
                .as_deref()
                .ok_or_else(|| format!("child for {spec} omitted capability evidence"))?;
            verify_probe_observations(spec, effective, capability)?;
            let (expected_execution, expected_windows) = match spec.task {
                ChildTask::Probe => (None, 0),
                ChildTask::Measure { execution } => {
                    if execution >= protocol.executions {
                        return Err(format!(
                            "the child for {spec} names execution {execution} outside {} executions",
                            protocol.executions
                        ));
                    }
                    (Some(execution), protocol.repetitions as usize)
                }
            };
            if samples.len() != expected_windows {
                return Err(format!(
                    "the child for {spec} timed {} windows rather than {expected_windows}",
                    samples.len()
                ));
            }
            for (index, sample) in samples.iter().enumerate() {
                if Some(sample.execution) != expected_execution
                    || sample.repetition != index as u64
                    || sample.calls == 0
                    || sample.calls > MAX_CALLS
                    || sample.elapsed_ns == 0
                {
                    return Err(format!(
                        "the child for {spec} reported invalid raw sample {index}: {sample:?}"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn verify_routes(
    spec: ChildSpec,
    expected: &str,
    requested: &str,
    observed: &str,
) -> Result<(), String> {
    if requested != expected || observed != expected {
        return Err(format!(
            "the child for {spec} reported requested/observed routes {requested:?}/{observed:?}, expected {expected:?}"
        ));
    }
    Ok(())
}

fn expected_omission_effective(omission: &CapabilityOmission) -> &'static str {
    match omission {
        CapabilityOmission::M4rmRegisterTiledUnavailable => "RowWise",
        CapabilityOmission::PlePanelKernelDeclined => "max_panel_cols=None",
        CapabilityOmission::TrsmBlockedCalleeDeclined => "panel_rows=None",
        CapabilityOmission::GemmWholeKernelDeclined => "PerCell",
        CapabilityOmission::SimdBackendUnavailable
        | CapabilityOmission::SoaPoolWidth { .. }
        | CapabilityOmission::PleByteLaneUnavailable
        | CapabilityOmission::Fp251WholeGemmUnavailable => "unavailable",
    }
}

fn verify_unavailable_routes(
    spec: ChildSpec,
    expected: &str,
    requested: &str,
    observed: &str,
    omission: &CapabilityOmission,
) -> Result<(), String> {
    let expected_observed = match omission {
        CapabilityOmission::M4rmRegisterTiledUnavailable => "row_wise",
        CapabilityOmission::PlePanelKernelDeclined => "kernel_declined",
        CapabilityOmission::TrsmBlockedCalleeDeclined => "blocked_callee_declined",
        CapabilityOmission::GemmWholeKernelDeclined => "per_cell",
        CapabilityOmission::SimdBackendUnavailable
        | CapabilityOmission::SoaPoolWidth { .. }
        | CapabilityOmission::PleByteLaneUnavailable
        | CapabilityOmission::Fp251WholeGemmUnavailable => "unavailable_before_dispatch",
    };
    if requested != expected || observed != expected_observed {
        return Err(format!(
            "the unavailable probe for {spec} reported requested/observed routes {requested:?}/{observed:?}, expected {expected:?}/{expected_observed:?} for {omission:?}"
        ));
    }
    Ok(())
}

fn verify_probe_observations(
    spec: ChildSpec,
    effective: &str,
    capability: &str,
) -> Result<(), String> {
    let (expected_effective, expected_capability) = expected_observation_contract(spec)?;
    let effective_matches = if expected_effective == "panel_width" {
        effective
            .strip_prefix("panel_width=")
            .and_then(|value| value.parse::<usize>().ok())
            .is_some_and(|value| value > 0)
    } else {
        effective == expected_effective
    };
    let capability_matches = if spec.arm == Arm::Conservative
        && matches!(
            spec.field,
            CalibratedField::TrsmBlockedMinDim | CalibratedField::GemmAxpyFastPathMinVolume
        ) {
        matches!(
            capability,
            "fp251_whole_gemm_available=true" | "fp251_whole_gemm_available=false"
        )
    } else {
        capability == expected_capability
    };
    if !effective_matches || !capability_matches {
        return Err(format!(
            "the probe for {spec} reported effective/capability {effective:?}/{capability:?}, expected {expected_effective:?}/{expected_capability:?}"
        ));
    }
    Ok(())
}

fn verify_capability_omission(
    spec: ChildSpec,
    omission: &CapabilityOmission,
    capability: &str,
) -> Result<(), String> {
    let valid = match omission {
        CapabilityOmission::SimdBackendUnavailable => {
            spec.field == CalibratedField::SimdMinWords
                && spec.arm == Arm::Asymptotic
                && capability == "simd_backend=none"
        }
        CapabilityOmission::SoaPoolWidth { observed } => {
            spec.field == CalibratedField::SoaParallelMinLen
                && *observed != 4
                && capability == format!("dedicated_pool_width={observed}")
        }
        CapabilityOmission::M4rmRegisterTiledUnavailable => {
            spec.field == CalibratedField::M4rmTiledMinStrideWords
                && spec.arm == Arm::Asymptotic
                && capability == "simd_tile8xn=unavailable"
        }
        CapabilityOmission::PleByteLaneUnavailable => {
            spec.field == CalibratedField::PlePanelBaseMaxCols
                && capability.starts_with("carrier_lane=")
        }
        CapabilityOmission::PlePanelKernelDeclined => {
            spec.field == CalibratedField::PlePanelBaseMaxCols
                && capability
                    == format!(
                    "carrier_lane=byte panel_byte_lane_max_cols={REQUIRED_PLE_BYTE_LANE_MAX_COLS}"
                )
        }
        CapabilityOmission::Fp251WholeGemmUnavailable => {
            matches!(
                spec.field,
                CalibratedField::TrsmBlockedMinDim | CalibratedField::GemmAxpyFastPathMinVolume
            ) && spec.arm == Arm::Asymptotic
                && capability == "fp251_whole_gemm_available=false"
        }
        CapabilityOmission::GemmWholeKernelDeclined => {
            spec.field == CalibratedField::GemmAxpyFastPathMinVolume
                && spec.arm == Arm::Asymptotic
                && capability == "fp251_whole_gemm_available=true"
        }
        CapabilityOmission::TrsmBlockedCalleeDeclined => {
            spec.field == CalibratedField::TrsmBlockedMinDim
                && spec.arm == Arm::Asymptotic
                && capability == "fp251_whole_gemm_available=true"
        }
    };
    if !valid {
        return Err(format!(
            "the probe for {spec} reported invalid capability omission {omission:?}/{capability:?}"
        ));
    }
    Ok(())
}

/// Verifies the semantic identity shared by a probe pair or by one arm's
/// probe and timed child. The parent derives case metadata from `ChildSpec`
/// in [`verify_child_report`]; only these result relationships come from a
/// second independently launched child.
fn verify_matching_evidence(
    context: &str,
    expected: &ChildReport,
    observed: &ChildReport,
) -> Result<(), String> {
    if observed.operand_digest != expected.operand_digest {
        return Err(format!("{context} built different operands"));
    }
    let ChildOutcome::Complete {
        result_digest: expected_result,
        equivalence_digest: expected_equivalence,
        ..
    } = &expected.outcome
    else {
        return Err(format!("{context} expected evidence is unavailable"));
    };
    let ChildOutcome::Complete {
        result_digest: observed_result,
        equivalence_digest: observed_equivalence,
        ..
    } = &observed.outcome
    else {
        return Err(format!("{context} observed evidence is unavailable"));
    };
    if observed_result != expected_result {
        return Err(format!("{context} produced a different result"));
    }
    if observed_equivalence != expected_equivalence {
        return Err(format!("{context} reported a different equivalence digest"));
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct VerifiedChildObservation<'a> {
    spec: ChildSpec,
    report: &'a ChildReport,
}

fn child_observation_line(spec: ChildSpec, report: &ChildReport) -> Result<String, String> {
    let observation = VerifiedChildObservation { spec, report };
    serde_json::to_string(&observation)
        .map(|json| format!("{CHILD_OBSERVATION_PREFIX}{json}"))
        .map_err(|error| format!("cannot encode the verified child observation: {error}"))
}

#[cfg(test)]
#[allow(dead_code)]
fn launch_verified_child(
    spec: ChildSpec,
    expected_operand_digest: &str,
    protocol: &Protocol,
    executable: &Path,
) -> Result<ChildReport, String> {
    launch_verified_child_with_entry(
        spec,
        expected_operand_digest,
        protocol,
        executable,
        FreshChildEntry::BenchmarkCli,
    )
}

fn launch_verified_child_with_entry(
    spec: ChildSpec,
    expected_operand_digest: &str,
    protocol: &Protocol,
    executable: &Path,
    entry: FreshChildEntry,
) -> Result<ChildReport, String> {
    let report = fresh_tuning_process(
        executable,
        FreshProcessCase {
            spec,
            protocol: protocol.clone(),
        },
        entry,
    )?;
    verify_child_report(spec, expected_operand_digest, protocol, &report)?;
    println!("{}", child_observation_line(spec, &report)?);
    Ok(report)
}

/// Runs the smallest preregistered probe for every host capability that can
/// omit a comparison. This mode performs no timed windows and writes no
/// profile; its structured child observations are suitable for preparing the
/// host before the authoritative campaign.
#[cfg(test)]
#[allow(dead_code)]
fn run_capability_report(protocol: &Protocol) -> Result<(), String> {
    validate_campaign_accounting(protocol)?;
    print_protocol(protocol);
    let controller = env::current_exe()
        .map_err(|error| format!("controller has no executable path: {error}"))?;
    let capability_fields = [
        CalibratedField::SimdMinWords,
        CalibratedField::SoaParallelMinLen,
        CalibratedField::M4rmTiledMinStrideWords,
        CalibratedField::TrsmBlockedMinDim,
        CalibratedField::PlePanelBaseMaxCols,
        CalibratedField::GemmAxpyFastPathMinVolume,
    ];
    let mut unavailable = 0_usize;
    for field in capability_fields {
        let size = field.grid()[0];
        let expected_operands = expected_operand_digest(field, size)?;
        for arm in Arm::BOTH {
            let spec = ChildSpec {
                field,
                variant: SweepVariant::Standard,
                size,
                arm,
                task: ChildTask::Probe,
            };
            let report = launch_verified_child(spec, &expected_operands, protocol, &controller)?;
            match report.outcome {
                ChildOutcome::Complete { .. } => println!(
                    "capability_report: field={field} size={size} arm={arm} outcome=complete"
                ),
                ChildOutcome::Unavailable { omission, .. } => {
                    unavailable += 1;
                    println!(
                        "capability_report: field={field} size={size} arm={arm} outcome=unavailable reason={omission:?}"
                    );
                }
            }
        }
    }
    println!(
        "capability_report_summary: probes={} unavailable={} timed=0 writes=0",
        capability_fields.len() * Arm::BOTH.len(),
        unavailable
    );
    Ok(())
}

// ---------------------------------------------------------------------
// Statistics and the selection rule
// ---------------------------------------------------------------------

/// One arm's timed windows at one grid point and the statistics derived from
/// them by the selection rule.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
struct ArmStat {
    median: f64,
    /// Interquartile range relative to the median.
    spread: f64,
    samples: Vec<TimingSample>,
}

impl ArmStat {
    fn from_samples(samples: Vec<TimingSample>) -> Self {
        let summary = tuning_campaign_support::statistics::WindowStatistics::from_samples(samples)
            .expect("validated positive timing windows");
        Self {
            median: summary.median,
            spread: summary.relative_iqr,
            samples: summary.samples,
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn windows(&self) -> usize {
        self.samples.len()
    }
}

/// One grid point of one field's sweep.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
struct GridPoint {
    size: usize,
    conservative: Option<ArmStat>,
    asymptotic: Option<ArmStat>,
}

#[cfg(test)]
#[allow(dead_code)]
impl GridPoint {
    /// The noise band and the asymptotic arm's relative margin over the other,
    /// or `None` when the point does not offer both arms.
    fn comparison(&self) -> Option<(f64, f64)> {
        let (conservative, asymptotic) = (self.conservative.as_ref()?, self.asymptotic.as_ref()?);
        let band = conservative.spread.max(asymptotic.spread);
        let margin = (conservative.median - asymptotic.median) / conservative.median;
        Some((band, margin))
    }

    /// Whether the asymptotic arm beats the other by more than the noise band.
    fn asymptotic_wins(&self) -> Option<bool> {
        self.comparison().map(|(band, margin)| margin > band)
    }
}

/// Why a field kept its conservative default.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum Fallback {
    /// At least one predeclared grid point lacked a two-arm comparison.
    NoComparableGridPoint,
    /// Both arms were measured, and the asymptotic one never won.
    NoGridPointWins,
    /// The asymptotic arm won at `first_win` and lost again at `later_loss`.
    NonMonotone { first_win: usize, later_loss: usize },
}

impl fmt::Display for Fallback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoComparableGridPoint => {
                formatter.write_str(
                    "at least one predeclared grid point lacks a two-arm comparison",
                )
            }
            Self::NoGridPointWins => {
                formatter.write_str("the asymptotic arm never beats the noise band")
            }
            Self::NonMonotone {
                first_win,
                later_loss,
            } => write!(
                formatter,
                "non-monotone: the asymptotic arm wins at {first_win} and loses again at {later_loss}"
            ),
        }
    }
}

/// What the sweep concluded for one field.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum Selection {
    /// A monotone crossover was measured.
    Crossover {
        value: usize,
        crossover: usize,
        band: f64,
        margin: f64,
    },
    /// The conservative default stands.
    KeptDefault { value: usize, reason: Fallback },
}

impl Selection {
    fn value(&self) -> usize {
        match self {
            Self::Crossover { value, .. } | Self::KeptDefault { value, .. } => *value,
        }
    }
}

/// One field's whole sweep.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
struct FieldSweep {
    field: CalibratedField,
    variant: SweepVariant,
    points: Vec<GridPoint>,
    selection: Selection,
}

/// Applies the selection rule to a measured grid.
///
/// The crossover is the smallest grid point at which the asymptotic arm wins,
/// and it counts only when the asymptotic arm also wins at every larger grid
/// point. A grid point that lacks a two-arm comparison invalidates the sweep
/// and yields the missing-comparison outcome before monotonicity is evaluated.
fn select(field: CalibratedField, points: &[GridPoint]) -> Selection {
    use tuning_campaign_support::statistics::{
        select_threshold, ThresholdDirection, ThresholdFallback, ThresholdPoint,
        ThresholdSelection, WindowStatistics,
    };
    let convert = |arm: &ArmStat| WindowStatistics {
        median: arm.median,
        relative_iqr: arm.spread,
        samples: arm.samples.clone(),
    };
    let points: Vec<_> = points
        .iter()
        .map(|point| ThresholdPoint {
            size: point.size,
            conservative: point.conservative.as_ref().map(convert),
            asymptotic: point.asymptotic.as_ref().map(convert),
        })
        .collect();
    let direction = if field.is_upper_bound() {
        ThresholdDirection::UpperBound {
            floor: field.upper_bound_floor(),
        }
    } else {
        ThresholdDirection::LowerBound
    };
    match select_threshold(&points, field.conservative_default(), direction) {
        ThresholdSelection::Crossover {
            value,
            crossover,
            band,
            margin,
        } => Selection::Crossover {
            value,
            crossover,
            band,
            margin,
        },
        ThresholdSelection::KeptDefault { value, reason } => Selection::KeptDefault {
            value,
            reason: match reason {
                ThresholdFallback::NoComparableGridPoint => Fallback::NoComparableGridPoint,
                ThresholdFallback::NoGridPointWins => Fallback::NoGridPointWins,
                ThresholdFallback::NonMonotone {
                    first_win,
                    later_loss,
                } => Fallback::NonMonotone {
                    first_win,
                    later_loss,
                },
            },
        },
    }
}

fn reconcile_interpolation(generic: &Selection, two_adic: &Selection) -> Selection {
    let default = CalibratedField::InterpolateFastMinPoints.conservative_default();
    match (generic, two_adic) {
        (
            Selection::KeptDefault {
                reason: Fallback::NoComparableGridPoint,
                ..
            },
            _,
        )
        | (
            _,
            Selection::KeptDefault {
                reason: Fallback::NoComparableGridPoint,
                ..
            },
        ) => Selection::KeptDefault {
            value: default,
            reason: Fallback::NoComparableGridPoint,
        },
        (Selection::Crossover { .. }, Selection::Crossover { .. }) => {
            if generic.value() >= two_adic.value() {
                generic.clone()
            } else {
                two_adic.clone()
            }
        }
        (Selection::KeptDefault { reason, .. }, _) | (_, Selection::KeptDefault { reason, .. }) => {
            Selection::KeptDefault {
                value: default,
                reason: reason.clone(),
            }
        }
    }
}

#[derive(serde::Serialize)]
#[cfg(test)]
#[allow(dead_code)]
struct InterpolationReconciliation<'a> {
    generic: &'a Selection,
    two_adic: &'a Selection,
    shared: &'a Selection,
}

#[cfg(test)]
#[allow(dead_code)]
fn interpolation_reconciliation_line(
    generic: &Selection,
    two_adic: &Selection,
    shared: &Selection,
) -> Result<String, String> {
    serde_json::to_string(&InterpolationReconciliation {
        generic,
        two_adic,
        shared,
    })
    .map(|json| format!("{INTERPOLATION_RECONCILIATION_PREFIX}{json}"))
    .map_err(|error| format!("cannot encode interpolation reconciliation: {error}"))
}

/// The sixteen swept values, named one per field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedValues {
    simd_min_words: usize,
    karatsuba_min_degree: usize,
    karatsuba_max_out_len: usize,
    div_rem_fast_min_len: usize,
    subproduct_min_len: usize,
    transpose_simple_max_blocks: usize,
    soa_parallel_min_len: usize,
    m4rm_wide_tier_min_stride_words: usize,
    m4rm_tiled_min_stride_words: usize,
    dense_inverse_m4ri_min_dim: usize,
    dense_inverse_blocked_min_dim: usize,
    trsm_blocked_min_dim: usize,
    ple_panel_base_max_cols: usize,
    ple_blocked_back_sub_min_dim: usize,
    gemm_axpy_fast_path_min_volume: usize,
    interpolate_fast_min_points: usize,
}

impl SelectedValues {
    #[cfg(test)]
    #[allow(dead_code)]
    fn from_sweeps(sweeps: &[FieldSweep]) -> Self {
        let value_of = |field: CalibratedField| {
            sweeps
                .iter()
                .find(|sweep| sweep.field == field)
                .map(|sweep| sweep.selection.value())
                .unwrap_or_else(|| field.conservative_default())
        };
        Self {
            simd_min_words: value_of(CalibratedField::SimdMinWords),
            karatsuba_min_degree: value_of(CalibratedField::KaratsubaMinDegree),
            karatsuba_max_out_len: value_of(CalibratedField::KaratsubaMaxOutLen),
            div_rem_fast_min_len: value_of(CalibratedField::DivRemFastMinLen),
            subproduct_min_len: value_of(CalibratedField::SubproductMinLen),
            transpose_simple_max_blocks: value_of(CalibratedField::TransposeSimpleMaxBlocks),
            soa_parallel_min_len: value_of(CalibratedField::SoaParallelMinLen),
            m4rm_wide_tier_min_stride_words: value_of(CalibratedField::M4rmWideTierMinStrideWords),
            m4rm_tiled_min_stride_words: value_of(CalibratedField::M4rmTiledMinStrideWords),
            dense_inverse_m4ri_min_dim: value_of(CalibratedField::DenseInverseM4riMinDim),
            dense_inverse_blocked_min_dim: value_of(CalibratedField::DenseInverseBlockedMinDim),
            trsm_blocked_min_dim: value_of(CalibratedField::TrsmBlockedMinDim),
            ple_panel_base_max_cols: value_of(CalibratedField::PlePanelBaseMaxCols),
            ple_blocked_back_sub_min_dim: value_of(CalibratedField::PleBlockedBackSubMinDim),
            gemm_axpy_fast_path_min_volume: value_of(CalibratedField::GemmAxpyFastPathMinVolume),
            interpolate_fast_min_points: value_of(CalibratedField::InterpolateFastMinPoints),
        }
    }
}

/// Builds the profile from the swept values.
///
/// `PolynomialSelectors::try_new` takes five consecutive `usize` parameters, so
/// a transposition among the polynomial thresholds would compile. Each value is
/// bound to a local named for its own field immediately before the call, which
/// puts the argument order and the field names on one screen.
fn build_profile(
    id: ProfileId,
    measurement: MeasurementProvenance,
    assembled_at: Rfc3339Utc,
    controller_binary_sha256: &Sha256,
    selected: &SelectedValues,
) -> Result<ProducedCoreProfile, String> {
    let simd_min_words: usize = selected.simd_min_words;
    let karatsuba_min_degree: usize = selected.karatsuba_min_degree;
    let karatsuba_max_out_len: usize = selected.karatsuba_max_out_len;
    let div_rem_fast_min_len: usize = selected.div_rem_fast_min_len;
    let subproduct_min_len: usize = selected.subproduct_min_len;
    let interpolate_fast_min_points: usize = selected.interpolate_fast_min_points;

    let bit_backend =
        BitBackendSelectors::try_new(simd_min_words).map_err(|error| error.to_string())?;
    let polynomial = PolynomialSelectors::try_new(
        karatsuba_min_degree,
        karatsuba_max_out_len,
        div_rem_fast_min_len,
        subproduct_min_len,
        interpolate_fast_min_points,
    )
    .map_err(|error| error.to_string())?;
    let conservative = &CoreTuning::CONSERVATIVE;
    let bit_matrix = BitMatrixSelectors::try_new(
        conservative.bit_matrix().matvec_simd_min_words(),
        selected.transpose_simple_max_blocks,
        conservative.bit_matrix().transpose_macro_tile_blocks(),
    )
    .map_err(|error| error.to_string())?;
    let soa_batch = SoaBatchSelectors::try_new(
        selected.soa_parallel_min_len,
        conservative.soa_batch().parallel_chunk_len(),
    )
    .map_err(|error| error.to_string())?;
    let m4rm = M4rmSelectors::try_new(
        selected.m4rm_wide_tier_min_stride_words,
        selected.m4rm_tiled_min_stride_words,
        conservative.m4rm().default_table_bytes(),
        conservative.m4rm().mid_table_bytes(),
        conservative.m4rm().wide_table_bytes(),
        conservative.m4rm().wide_max_k(),
        conservative.m4rm().small_n_max_k(),
    )
    .map_err(|error| error.to_string())?;
    let dense_inverse = DenseInverseSelectors::try_new(
        selected.dense_inverse_m4ri_min_dim,
        selected.dense_inverse_blocked_min_dim,
    )
    .map_err(|error| error.to_string())?;
    let triangular = TriangularSelectors::try_new(
        selected.trsm_blocked_min_dim,
        conservative.triangular().trsm_panel_rows(),
        conservative.triangular().base_case_max_dim(),
    )
    .map_err(|error| error.to_string())?;
    let ple = PleSelectors::try_new(
        selected.ple_panel_base_max_cols,
        selected.ple_blocked_back_sub_min_dim,
        conservative.ple().scalar_base_max_cols(),
        conservative.ple().panel_byte_lane_max_cols(),
        conservative.ple().panel_u16_lane_max_cols(),
    )
    .map_err(|error| error.to_string())?;
    let gemm = GemmSelectors::try_new(
        conservative.gemm().row_tile(),
        conservative.gemm().col_tile(),
        selected.gemm_axpy_fast_path_min_volume,
        conservative.gemm().winograd_min_dim(),
    )
    .map_err(|error| error.to_string())?;
    let assembly = match &measurement {
        MeasurementProvenance::Calibrated {
            source_revision,
            source_dirty,
            harness,
            ..
        } => AssemblyProvenance {
            assembled_at,
            source_revision: source_revision.clone(),
            source_dirty: *source_dirty,
            tool: harness.clone(),
            tool_sha256: controller_binary_sha256.clone(),
        },
        MeasurementProvenance::Inherited => {
            return Err("the measurement harness cannot emit inherited evidence".to_owned())
        }
    };
    Ok(ProducedCoreProfile {
        id,
        measurement,
        assembly,
        section: CoreTuning::from_selectors(CoreSelectors {
            bit_backend,
            bit_matrix,
            soa_batch,
            m4rm,
            dense_inverse,
            triangular,
            ple,
            gemm,
            polynomial,
            ..CoreSelectors::CONSERVATIVE
        }),
    })
}

// ---------------------------------------------------------------------
// Reporting and output
// ---------------------------------------------------------------------

#[cfg(test)]
#[allow(dead_code)]
fn print_grid() {
    println!(
        "field\tvariant\tfamily\tdefault\tunit\tconservative_arm\tasymptotic_arm\tarm_source\tgrid"
    );
    for field in CalibratedField::ALL {
        let grid: Vec<String> = field.grid().iter().map(usize::to_string).collect();
        for &variant in field.variants() {
            println!(
                "{field}\t{variant}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                field.family(),
                field.conservative_default(),
                field.grid_unit(),
                field.conservative_arm(),
                field.asymptotic_arm(),
                field.arm_source(),
                grid.join(",")
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
#[allow(dead_code)]
struct CampaignAccounting {
    cells: usize,
    probes: usize,
    timed: usize,
    launches: usize,
    windows: usize,
}

#[cfg(test)]
#[allow(dead_code)]
fn planned_campaign_accounting(protocol: &Protocol) -> CampaignAccounting {
    let cells = CalibratedField::ALL
        .into_iter()
        .map(|field| field.grid().len() * field.variants().len() * Arm::BOTH.len())
        .sum();
    let probes = cells;
    let timed = cells * protocol.executions as usize;
    CampaignAccounting {
        cells,
        probes,
        timed,
        launches: probes + timed,
        windows: timed * protocol.repetitions as usize,
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn validate_campaign_accounting(protocol: &Protocol) -> Result<CampaignAccounting, String> {
    let accounting = planned_campaign_accounting(protocol);
    if protocol.executions == DEFAULT_EXECUTIONS
        && protocol.repetitions == DEFAULT_REPETITIONS
        && accounting
            != (CampaignAccounting {
                cells: EXPECTED_GRID_ARM_CELLS,
                probes: EXPECTED_PROBE_CHILDREN,
                timed: EXPECTED_TIMED_CHILDREN,
                launches: EXPECTED_FRESH_CHILDREN,
                windows: EXPECTED_RAW_WINDOWS,
            })
    {
        return Err(format!("campaign plan drifted: {accounting:?}"));
    }
    Ok(accounting)
}

#[cfg(test)]
#[allow(dead_code)]
fn print_seed_inventory() -> Result<(), String> {
    for field in CalibratedField::ALL {
        for size in field.grid() {
            println!("{}", seed_inventory_line(field, size)?);
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
fn print_protocol(protocol: &Protocol) {
    let accounting = planned_campaign_accounting(protocol);
    println!(
        "protocol: profile_format_version={} section_id={} section_schema_version={} \
         harness_schema={} fresh_case_schema={} raw_sample_schema={} executions={} repetitions={} \
         target_ms={} windows_per_arm={} required_features={} required_rayon_num_threads={}",
        PROFILE_FORMAT_VERSION,
        CoreTuning::ID.as_str(),
        CoreTuningCodec::SCHEMA_VERSION,
        CoreTuningCodec::HARNESS_SCHEMA,
        FRESH_CASE_VALUE,
        RAW_SAMPLE_SCHEMA,
        protocol.executions,
        protocol.repetitions,
        protocol.target_ms,
        protocol.windows(),
        REQUIRED_FEATURES,
        REQUIRED_RAYON_THREADS,
    );
    println!(
        "campaign_plan: cells={} probes={} timed={} launches={} windows={}",
        accounting.cells,
        accounting.probes,
        accounting.timed,
        accounting.launches,
        accounting.windows
    );
    println!(
        "seed_protocol: schema={SEED_SCHEMA} derivation={SEED_DERIVATION} root={SEED_ROOT} \
         role_mix=0x9e3779b97f4a7c15 word_mix=0xbf58476d1ce4e5b9 rotate_left=27 \
         add=0x94d049bb133111eb final_xor_shift=31 words=field_tag,size"
    );
}

#[cfg(test)]
#[allow(dead_code)]
fn print_host_facts(facts: &HostFacts) {
    println!("host: {}", facts.host);
    println!("cpu_model: {}", facts.cpu_model);
    println!("cpu_features: {}", facts.cpu_features.join(","));
    println!("os_kernel: {}", facts.os_kernel);
    println!("governor: {}", facts.governor);
    println!("cpu_affinity: {}", facts.cpu_affinity);
    println!(
        "lock_file: {} (observed on an inherited descriptor)",
        facts.lock_file
    );
    println!(
        "source_revision: {} source_dirty={}",
        facts.source_revision.as_str(),
        facts.source_dirty
    );
    println!("harness: {}", facts.harness.as_str());
    println!("binary_sha256: {}", facts.binary_sha256.as_str());
    println!("toolchain: {}", facts.toolchain);
    println!(
        "rayon_num_threads_env: {}",
        env::var(RAYON_THREADS_VAR).unwrap_or_else(|_| "absent".to_owned())
    );
    println!(
        "simd_backend: {}",
        simd_backend().map_or("none", Backend::name)
    );
}

#[derive(serde::Serialize)]
#[cfg(test)]
#[allow(dead_code)]
struct SampleRecord<'a> {
    schema: &'static str,
    profile_format_version: u32,
    section_id: &'static str,
    section_schema_version: u32,
    harness_schema: &'static str,
    field: CalibratedField,
    variant: SweepVariant,
    size: usize,
    arm: Arm,
    samples: &'a [TimingSample],
}

#[cfg(test)]
#[allow(dead_code)]
fn sample_record_line(
    field: CalibratedField,
    variant: SweepVariant,
    size: usize,
    arm: Arm,
    samples: &[TimingSample],
) -> Result<String, String> {
    let record = SampleRecord {
        schema: RAW_SAMPLE_SCHEMA,
        profile_format_version: PROFILE_FORMAT_VERSION,
        section_id: CoreTuning::ID.as_str(),
        section_schema_version: CoreTuningCodec::SCHEMA_VERSION,
        harness_schema: CoreTuningCodec::HARNESS_SCHEMA,
        field,
        variant,
        size,
        arm,
        samples,
    };
    serde_json::to_string(&record)
        .map(|json| format!("{SAMPLE_PREFIX}{json}"))
        .map_err(|error| format!("cannot encode raw timing samples: {error}"))
}

#[cfg(test)]
#[allow(dead_code)]
fn print_sweep(sweep: &FieldSweep) -> Result<(), String> {
    let field = sweep.field;
    println!(
        "\n{field}/{} ({}): default {} in {}; arms {} vs {}",
        sweep.variant,
        field.family(),
        field.conservative_default(),
        field.grid_unit(),
        field.conservative_arm(),
        field.asymptotic_arm()
    );
    println!(
        "each probe and timed execution runs in a fresh child; every verified child record carries \
         the exact forced selector and companion-control values it installed"
    );
    println!(
        "size\t{}_ns\tspread\t{}_ns\tspread\twindows\tmargin\tband\tverdict",
        field.conservative_arm(),
        field.asymptotic_arm()
    );
    for point in &sweep.points {
        let render = |stat: &Option<ArmStat>| match stat {
            Some(stat) => (format!("{:.3}", stat.median), format!("{:.4}", stat.spread)),
            None => ("-".to_owned(), "-".to_owned()),
        };
        let (conservative_ns, conservative_spread) = render(&point.conservative);
        let (asymptotic_ns, asymptotic_spread) = render(&point.asymptotic);
        let windows = point
            .conservative
            .as_ref()
            .or(point.asymptotic.as_ref())
            .map_or(0, ArmStat::windows);
        let (margin, band, verdict) = match point.comparison() {
            Some((band, margin)) => (
                format!("{margin:.4}"),
                format!("{band:.4}"),
                if margin > band {
                    field.asymptotic_arm()
                } else {
                    field.conservative_arm()
                },
            ),
            None => ("-".to_owned(), "-".to_owned(), "no comparison"),
        };
        println!(
            "{}\t{conservative_ns}\t{conservative_spread}\t{asymptotic_ns}\t{asymptotic_spread}\t{windows}\t{margin}\t{band}\t{verdict}",
            point.size
        );
        for (arm, stat) in [
            (Arm::Conservative, point.conservative.as_ref()),
            (Arm::Asymptotic, point.asymptotic.as_ref()),
        ] {
            if let Some(stat) = stat {
                println!(
                    "{}",
                    sample_record_line(field, sweep.variant, point.size, arm, &stat.samples)?
                );
            }
        }
    }
    match &sweep.selection {
        Selection::Crossover {
            value,
            crossover,
            band,
            margin,
        } => println!(
            "selected: {field}={value} from crossover at {crossover} (margin {margin:.4} > band {band:.4})"
        ),
        Selection::KeptDefault { value, reason } => {
            println!("selected: {field}={value} (conservative default kept: {reason})");
        }
    }
    Ok(())
}

/// Prints the omission set with each field's inherited value and why it is
/// omitted, which is the inventory the receipt records.
///
/// The inherited values are read out of the conservative table's own
/// serialization, so the report states what the loader will resolve an absent
/// key to rather than a figure written into this tool.
#[cfg(test)]
#[allow(dead_code)]
fn print_omitted(omitted: &[SchemaField], sweeps: &[FieldSweep]) -> Result<(), String> {
    let conservative = CoreTuningCodec::encode_body(&CoreTuning::CONSERVATIVE)
        .map_err(|error| format!("the conservative section does not encode: {error}"))?;
    let conservative = serde_json::to_value(conservative)
        .map_err(|error| format!("the conservative section is not JSON: {error}"))?;
    let uncomparable: Vec<SchemaField> = uncalibrated_fields(sweeps)
        .into_iter()
        .map(CalibratedField::schema_field)
        .collect();
    println!(
        "\nomitted ({}): the emitted document states no value for these schema fields, and the \
         loader resolves each absent key to the inherited conservative default",
        omitted.len()
    );
    println!("family\tfield\tinherited\treason");
    for field in omitted {
        let inherited = conservative
            .pointer(&format!("/{}/{}", field.family, field.name))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("the conservative table states no `{field}`"))?;
        let reason = if uncomparable.contains(field) {
            "at least one predeclared grid point lacks a two-arm comparison"
        } else {
            "no sweep covers this field"
        };
        println!("{}\t{}\t{inherited}\t{reason}", field.family, field.name);
    }
    Ok(())
}

/// The swept fields with at least one missing predeclared comparison.
///
/// A field that keeps its default after a comparison — no grid point beat the
/// noise band, or the crossover was non-monotone — is a calibration outcome and
/// stays in the document. A field for which any predeclared grid point lacks a
/// two-arm comparison is uncalibrated, and design §5 condition 5 requires the
/// document to omit it rather than state a value: "a profile that carries an
/// uncalibrated value is a `@/inv/benchmark-backed-performance` defect".
///
/// These are not the whole omission set: [`omitted_fields`] adds every schema
/// field no sweep covers, which the same rule governs for the same reason.
#[cfg(test)]
#[allow(dead_code)]
fn uncalibrated_fields(sweeps: &[FieldSweep]) -> Vec<CalibratedField> {
    sweeps
        .iter()
        .filter(|sweep| {
            matches!(
                sweep.selection,
                Selection::KeptDefault {
                    reason: Fallback::NoComparableGridPoint,
                    ..
                }
            )
        })
        .map(|sweep| sweep.field)
        .collect()
}

/// The schema fields whose value this run measured.
///
/// A field the sweep covered and concluded on — including one that kept its
/// default on a tie or a non-monotone crossover — is measured. A field with any
/// missing predeclared comparison is not, and neither is any schema field no
/// sweep names.
#[cfg(test)]
#[allow(dead_code)]
fn measured_fields(sweeps: &[FieldSweep]) -> Vec<SchemaField> {
    let uncalibrated = uncalibrated_fields(sweeps);
    sweeps
        .iter()
        .map(|sweep| sweep.field)
        .filter(|field| !uncalibrated.contains(field))
        .map(CalibratedField::schema_field)
        .collect()
}

/// Every `selectors.<family>.<field>` key `document` states.
///
/// The inventory is read off the serialized profile at run time rather than
/// listed in this tool. A list here would be a hand-maintained copy of the
/// schema — the staleness defect `@/inv/runtime-observed-provenance` names —
/// and the moment it fell behind, an unswept field would be emitted with a
/// value nothing measured.
#[cfg(test)]
#[allow(dead_code)]
fn schema_fields(document: &str) -> Result<Vec<SchemaField>, String> {
    let parsed: serde_json::Value = serde_json::from_str(document)
        .map_err(|error| format!("the profile document is not JSON: {error}"))?;
    let families = parsed
        .pointer("/sections/gf2-core~1selectors/selectors")
        .and_then(serde_json::Value::as_object)
        .ok_or("the profile document has no `selectors` object")?;
    let mut fields = Vec::new();
    for (family, members) in families {
        let members = members
            .as_object()
            .ok_or_else(|| format!("the `{family}` selector family is not an object"))?;
        fields.extend(members.keys().map(|name| SchemaField {
            family: family.clone(),
            name: name.clone(),
        }));
    }
    Ok(fields)
}

/// Every schema field the emitted document omits.
///
/// The set is the complement of what this run measured, so it covers both a
/// swept field with any missing predeclared comparison and every schema field
/// outside the sweep, whatever the schema has grown since. Design §5 condition
/// 5 admits an omitted field and forbids an unmeasured stated one, so the
/// complement is the rule rather than a conservative approximation of it.
#[cfg(test)]
#[allow(dead_code)]
fn omitted_fields(document: &str, sweeps: &[FieldSweep]) -> Result<Vec<SchemaField>, String> {
    let measured = measured_fields(sweeps);
    Ok(schema_fields(document)?
        .into_iter()
        .filter(|field| !measured.contains(field))
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
#[allow(dead_code)]
struct CampaignCoverage {
    measured: usize,
    omitted: usize,
    total: usize,
}

/// Rejects publication unless the codec-derived inventory is exactly the
/// campaign's sixteen measured fields and its 21-of-37 omission complement.
#[cfg(test)]
#[allow(dead_code)]
fn validate_campaign_coverage(
    complete_document: &str,
    sweeps: &[FieldSweep],
    omitted: &[SchemaField],
) -> Result<CampaignCoverage, String> {
    let schema = schema_fields(complete_document)?;
    let measured = measured_fields(sweeps);
    let expected_measured: Vec<SchemaField> = CalibratedField::ALL
        .into_iter()
        .map(CalibratedField::schema_field)
        .collect();
    if measured != expected_measured {
        return Err(format!(
            "the run measured {measured:?}, not all sixteen preregistered campaign fields"
        ));
    }
    if schema.len() != EXPECTED_CORE_SCHEMA_FIELDS
        || measured.len() != EXPECTED_MEASURED_FIELDS
        || omitted.len() != EXPECTED_OMITTED_FIELDS
        || measured.len() + omitted.len() != schema.len()
    {
        return Err(format!(
            "the CoreTuningCodec inventory yielded measured={} omitted={} total={}, expected \
             {EXPECTED_MEASURED_FIELDS}/{EXPECTED_OMITTED_FIELDS}/{EXPECTED_CORE_SCHEMA_FIELDS}",
            measured.len(),
            omitted.len(),
            schema.len()
        ));
    }
    if measured
        .iter()
        .any(|field| omitted.contains(field) || !schema.contains(field))
        || omitted.iter().any(|field| !schema.contains(field))
    {
        return Err(
            "the measured/omitted complement does not partition the codec inventory".into(),
        );
    }
    Ok(CampaignCoverage {
        measured: measured.len(),
        omitted: omitted.len(),
        total: schema.len(),
    })
}

/// Re-encodes `profile` after omitting fields this run did not measure.
///
/// The owner codec decodes the reduced selector body so private presence state
/// records each omission, then the one registry encoder recomputes the format-2
/// content digest. Absent fields retain their conservative dispatch semantics
/// without making a measurement claim.
#[cfg(test)]
#[allow(dead_code)]
fn calibrated_document(
    profile: &ProducedCoreProfile,
    omitted: &[SchemaField],
) -> Result<String, String> {
    Ok(profile.omitting(omitted)?.to_json())
}

/// Writes the emitted document to a temporary file, reparses it, then publishes it.
///
/// Only a document the loader accepts is an artifact, so a document this
/// harness could not have loaded never reaches a receipt. The comparison is
/// against the measured profile: an omitted field resolves back to the
/// conservative default, which is the value the sweep left it at.
fn emit_profile(path: &Path, json: &str) -> io::Result<String> {
    let path = resolve_repository_path(path);
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "refusing to overwrite an existing tuning artifact",
        ));
    }
    let profile = ProducedCoreProfile::from_json(json).map_err(io::Error::other)?;
    if profile.to_json() != json {
        return Err(io::Error::other("owner document is not canonical"));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    tuning_campaign_support::journal::atomic_write_new(&path, json.as_bytes())?;
    let written = fs::read_to_string(&path)?;
    if ProducedCoreProfile::from_json(&written).map_err(io::Error::other)? != profile
        || written != json
    {
        return Err(io::Error::other("owner artifact changed on strict reopen"));
    }
    Ok(written)
}

/// The profile identifier, taken from `--profile-id` or the emitted file's own
/// basename, which is what a committed profile is named by.
#[cfg(test)]
#[allow(dead_code)]
fn profile_id_for(out: &Path, explicit: Option<&str>) -> Result<ProfileId, String> {
    let candidate = match explicit {
        Some(value) => value.to_owned(),
        None => out
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("--out has no file name to take a profile id from")?
            .to_owned(),
    };
    ProfileId::parse(&candidate).map_err(|_| {
        format!("`{candidate}` is not a kebab-case profile id; pass --profile-id with one")
    })
}

fn validate_rayon_threads(value: Option<&str>) -> Result<(), String> {
    match value {
        Some(REQUIRED_RAYON_THREADS) => Ok(()),
        Some(value) => Err(format!(
            "{RAYON_THREADS_VAR} must be {REQUIRED_RAYON_THREADS}, found {value:?}"
        )),
        None => Err(format!(
            "{RAYON_THREADS_VAR} is absent; the campaign requires {REQUIRED_RAYON_THREADS}"
        )),
    }
}

fn require_campaign_environment() -> Result<(), String> {
    if env::var("RUSTUP_TOOLCHAIN").as_deref() != Ok(REQUIRED_RUSTUP_TOOLCHAIN) {
        return Err("campaign requires RUSTUP_TOOLCHAIN=1.95.0".to_owned());
    }

    validate_rayon_threads(env::var(RAYON_THREADS_VAR).ok().as_deref())
}

// ---------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(env::args().skip(1))?;
    match args.mode {
        Mode::FreshChild => campaign_owner::fresh_child()?,
        Mode::OwnerOperation => campaign_owner::owner_operation()?,
        Mode::SelfCheck | Mode::ListGrid | Mode::CapabilityReport => {
            campaign_owner::report_mode(args.mode)?
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    struct TestOutput {
        directory: PathBuf,
        path: PathBuf,
    }

    #[allow(dead_code)]
    impl TestOutput {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = env::temp_dir().join(format!(
                "gf2-tuning-calibration-{}-{serial}-{label}",
                std::process::id()
            ));
            let path = directory.join("profile.json");
            Self { directory, path }
        }
    }

    impl Drop for TestOutput {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    /// Five distinct values, so a transposition among the four polynomial
    /// thresholds cannot satisfy the per-field assertions below.
    #[allow(dead_code)]
    const DISTINCT: SelectedValues = SelectedValues {
        simd_min_words: 11,
        karatsuba_min_degree: 22,
        karatsuba_max_out_len: 33,
        div_rem_fast_min_len: 44,
        subproduct_min_len: 55,
        transpose_simple_max_blocks: 16,
        soa_parallel_min_len: 32_768,
        m4rm_wide_tier_min_stride_words: 16,
        m4rm_tiled_min_stride_words: 8,
        dense_inverse_m4ri_min_dim: 8,
        dense_inverse_blocked_min_dim: 16,
        trsm_blocked_min_dim: 64,
        ple_panel_base_max_cols: 128,
        ple_blocked_back_sub_min_dim: 128,
        gemm_axpy_fast_path_min_volume: 4096,
        interpolate_fast_min_points: 16,
    };

    #[allow(dead_code)]
    fn calibrated_provenance() -> MeasurementProvenance {
        MeasurementProvenance::Calibrated {
            measured_at: Rfc3339Utc::parse("2026-08-20T00:00:00Z").unwrap(),
            source_revision: GitRevision::parse(&"a".repeat(40)).unwrap(),
            source_dirty: false,
            harness: RepoRelPath::parse("crates/gf2-core/benches/tuning_calibration.rs").unwrap(),
            harness_schema: HarnessSchema::parse(CoreTuningCodec::HARNESS_SCHEMA).unwrap(),
            binary_sha256: Sha256::parse(&"b".repeat(64)).unwrap(),
            toolchain: "rustc 1.95.0".to_owned(),
            host: "test-host".to_owned(),
            cpu_model: "test-cpu".to_owned(),
            cpu_features: vec!["avx2".to_owned()],
            os_kernel: "Linux".to_owned(),
            governor: "performance".to_owned(),
            receipt: RepoRelPath::parse("dev/benchmarks/tuning_profiles/receipt.md").unwrap(),
        }
    }

    fn assembly_instant() -> Rfc3339Utc {
        Rfc3339Utc::parse("2026-08-20T01:00:00Z").unwrap()
    }

    fn controller_binary_sha256() -> Sha256 {
        Sha256::parse(&"c".repeat(64)).unwrap()
    }

    #[allow(dead_code)]
    pub(super) fn profile_from(selected: &SelectedValues) -> ProducedCoreProfile {
        build_profile(
            ProfileId::parse("test-profile").unwrap(),
            calibrated_provenance(),
            assembly_instant(),
            &controller_binary_sha256(),
            selected,
        )
        .expect("the swept values are in range")
    }

    /// A grid point with both arms, stated as medians and spreads directly.
    #[allow(dead_code)]
    fn point(size: usize, conservative: f64, asymptotic: f64, spread: f64) -> GridPoint {
        GridPoint {
            size,
            conservative: Some(ArmStat {
                median: conservative,
                spread,
                samples: Vec::new(),
            }),
            asymptotic: Some(ArmStat {
                median: asymptotic,
                spread,
                samples: Vec::new(),
            }),
        }
    }

    #[test]
    fn assembly_uses_the_supplied_post_sweep_instant_not_measurement_start() {
        let profile = profile_from(&DISTINCT);
        let MeasurementProvenance::Calibrated { measured_at, .. } = &profile.measurement else {
            panic!("the test profile must carry calibrated measurement evidence");
        };

        assert_eq!(measured_at.as_str(), "2026-08-20T00:00:00Z");
        assert_eq!(
            profile.assembly.assembled_at.as_str(),
            "2026-08-20T01:00:00Z"
        );
        assert_ne!(profile.assembly.assembled_at, *measured_at);
    }

    #[test]
    fn emitted_profile_carries_the_swept_simd_min_words() {
        assert_eq!(
            profile_from(&DISTINCT).bit_backend().simd_min_words(),
            DISTINCT.simd_min_words
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_karatsuba_min_degree() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().karatsuba_min_degree(),
            DISTINCT.karatsuba_min_degree
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_karatsuba_max_out_len() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().karatsuba_max_out_len(),
            DISTINCT.karatsuba_max_out_len
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_div_rem_fast_min_len() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().div_rem_fast_min_len(),
            DISTINCT.div_rem_fast_min_len
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_subproduct_min_len() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().subproduct_min_len(),
            DISTINCT.subproduct_min_len
        );
    }

    /// A sweep of `field` that concluded with `selection`, carrying no grid.
    #[allow(dead_code)]
    fn concluded(field: CalibratedField, reason: Fallback) -> FieldSweep {
        FieldSweep {
            field,
            variant: SweepVariant::Standard,
            points: Vec::new(),
            selection: Selection::KeptDefault {
                value: field.conservative_default(),
                reason,
            },
        }
    }

    #[test]
    fn the_emitted_document_omits_the_field_the_sweep_could_not_compare() {
        let profile = profile_from(&DISTINCT);
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        assert!(
            !document.contains("karatsuba_min_degree"),
            "the uncalibrated field is still stated: {document}"
        );
        let loaded = ProducedCoreProfile::from_json(&document)
            .expect("an omitted field is a supported state of the schema");
        assert_eq!(
            loaded.polynomial().karatsuba_min_degree(),
            CoreTuning::CONSERVATIVE.polynomial().karatsuba_min_degree(),
            "the loader resolves the absent field to the conservative default"
        );
    }

    #[test]
    fn omitting_one_field_leaves_the_other_four_stated() {
        let profile = profile_from(&DISTINCT);
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        let loaded = ProducedCoreProfile::from_json(&document).unwrap();
        assert_eq!(
            loaded.bit_backend().simd_min_words(),
            DISTINCT.simd_min_words
        );
        assert_eq!(
            loaded.polynomial().karatsuba_max_out_len(),
            DISTINCT.karatsuba_max_out_len
        );
        assert_eq!(
            loaded.polynomial().div_rem_fast_min_len(),
            DISTINCT.div_rem_fast_min_len
        );
        assert_eq!(
            loaded.polynomial().subproduct_min_len(),
            DISTINCT.subproduct_min_len
        );
    }

    #[test]
    fn a_document_with_nothing_omitted_is_the_serialized_profile_verbatim() {
        let profile = profile_from(&DISTINCT);
        assert_eq!(
            calibrated_document(&profile, &[]).unwrap(),
            profile.to_json()
        );
    }

    #[test]
    fn omission_changes_only_the_selector_body_and_recomputes_the_digest() {
        let profile = profile_from(&DISTINCT);
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        let complete: serde_json::Value = serde_json::from_str(&profile.to_json()).unwrap();
        let omitted: serde_json::Value = serde_json::from_str(&document).unwrap();
        let mut expected = complete["sections"]["gf2-core/selectors"]["selectors"].clone();
        expected["polynomial"]
            .as_object_mut()
            .unwrap()
            .remove("karatsuba_min_degree");
        assert_eq!(
            omitted["sections"]["gf2-core/selectors"]["selectors"],
            expected
        );
        assert_ne!(
            omitted["assembly"]["content_sha256"],
            complete["assembly"]["content_sha256"]
        );
    }

    #[test]
    fn omitting_the_last_field_of_a_family_leaves_an_empty_object() {
        let profile = profile_from(&DISTINCT);
        let document =
            calibrated_document(&profile, &[CalibratedField::SimdMinWords.schema_field()]).unwrap();
        assert!(document.contains(r#""bit_backend":{}"#), "{document}");
        let loaded = ProducedCoreProfile::from_json(&document).unwrap();
        assert_eq!(
            loaded.bit_backend().simd_min_words(),
            CoreTuning::CONSERVATIVE.bit_backend().simd_min_words()
        );
    }

    #[test]
    fn a_provenance_string_is_never_mistaken_for_a_selector_key() {
        // The receipt path names the field, so a document-wide search would cut
        // the wrong bytes.
        let mut provenance = calibrated_provenance();
        if let MeasurementProvenance::Calibrated { receipt, .. } = &mut provenance {
            *receipt =
                RepoRelPath::parse("dev/benchmarks/tuning_profiles/karatsuba_min_degree-notes.md")
                    .unwrap();
        }
        let profile = build_profile(
            ProfileId::parse("test-profile").unwrap(),
            provenance,
            assembly_instant(),
            &controller_binary_sha256(),
            &DISTINCT,
        )
        .unwrap();
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        let loaded = ProducedCoreProfile::from_json(&document).unwrap();
        let MeasurementProvenance::Calibrated { receipt, .. } = &loaded.measurement else {
            panic!("the document stays calibrated");
        };
        assert_eq!(
            receipt.as_str(),
            "dev/benchmarks/tuning_profiles/karatsuba_min_degree-notes.md"
        );
        assert_eq!(
            loaded.polynomial().karatsuba_min_degree(),
            CoreTuning::CONSERVATIVE.polynomial().karatsuba_min_degree()
        );
    }

    #[test]
    fn only_an_uncomparable_field_is_omitted() {
        let sweeps = [
            concluded(
                CalibratedField::KaratsubaMinDegree,
                Fallback::NoComparableGridPoint,
            ),
            concluded(
                CalibratedField::KaratsubaMaxOutLen,
                Fallback::NonMonotone {
                    first_win: 255,
                    later_loss: 383,
                },
            ),
            concluded(CalibratedField::SimdMinWords, Fallback::NoGridPointWins),
        ];
        assert_eq!(
            uncalibrated_fields(&sweeps),
            vec![CalibratedField::KaratsubaMinDegree],
            "a tie and a non-monotone crossover are calibration outcomes, not absent measurements"
        );
    }

    /// A sweep for every field, each concluding on a comparison, so nothing is
    /// omitted for want of a grid point and the omission set is exactly the
    /// schema fields outside the sweep.
    #[allow(dead_code)]
    fn measured_sweeps() -> Vec<FieldSweep> {
        CalibratedField::ALL
            .into_iter()
            .map(|field| {
                let points = vec![point(field.conservative_default(), 100.0, 50.0, 0.01)];
                FieldSweep {
                    field,
                    variant: SweepVariant::Standard,
                    selection: select(field, &points),
                    points,
                }
            })
            .collect()
    }

    #[test]
    fn every_swept_field_names_a_key_the_schema_states() {
        let schema = schema_fields(&profile_from(&DISTINCT).to_json()).unwrap();
        for field in CalibratedField::ALL {
            let key = field.schema_field();
            assert!(
                schema.contains(&key),
                "the sweep states {key}, which the schema does not carry; a renamed schema field \
                 would leave the measured value silently omitted"
            );
        }
    }

    #[test]
    fn the_schema_carries_fields_no_sweep_covers() {
        let schema = schema_fields(&profile_from(&DISTINCT).to_json()).unwrap();
        assert!(
            schema.len() > CalibratedField::ALL.len(),
            "the schema states {} fields against {} swept ones; the omission-set tests below are \
             vacuous once the two coincide",
            schema.len(),
            CalibratedField::ALL.len()
        );
    }

    #[test]
    fn an_emitted_document_states_only_the_fields_the_run_measured() {
        let profile = profile_from(&DISTINCT);
        let sweeps = measured_sweeps();
        let omitted = omitted_fields(&profile.to_json(), &sweeps).unwrap();
        let document = calibrated_document(&profile, &omitted).unwrap();
        let mut stated = schema_fields(&document).unwrap();
        let mut swept: Vec<SchemaField> = CalibratedField::ALL
            .into_iter()
            .map(CalibratedField::schema_field)
            .collect();
        stated.sort();
        swept.sort();
        assert_eq!(
            stated, swept,
            "the emitted document states a field this run did not measure"
        );
    }

    #[test]
    fn an_unswept_schema_field_is_omitted_and_resolves_to_its_inherited_value() {
        let profile = profile_from(&DISTINCT);
        let sweeps = measured_sweeps();
        let omitted = omitted_fields(&profile.to_json(), &sweeps).unwrap();
        let document = calibrated_document(&profile, &omitted).unwrap();
        let stated = schema_fields(&document).unwrap();
        let loaded = ProducedCoreProfile::from_json(&document)
            .expect("an omitted field is a supported state of the schema");
        let loaded = complete_selector_value(&loaded.section).unwrap();
        let inherited = complete_selector_value(&CoreTuning::CONSERVATIVE).unwrap();
        assert!(!omitted.is_empty());
        for field in &omitted {
            let pointer = format!("/{}/{}", field.family, field.name);
            assert!(!stated.contains(field), "{field} is still stated");
            assert_eq!(
                loaded.pointer(&pointer),
                inherited.pointer(&pointer),
                "the loader does not resolve the absent {field} to its inherited value"
            );
        }
    }

    #[test]
    fn the_omission_set_is_the_complement_of_what_the_run_measured() {
        let document = profile_from(&DISTINCT).to_json();
        let sweeps = measured_sweeps();
        let measured = measured_fields(&sweeps);
        let omitted = omitted_fields(&document, &sweeps).unwrap();
        for field in &measured {
            assert!(
                !omitted.contains(field),
                "{field} is both measured and omitted"
            );
        }
        let mut union: Vec<SchemaField> = measured.into_iter().chain(omitted).collect();
        union.sort();
        let mut schema = schema_fields(&document).unwrap();
        schema.sort();
        assert_eq!(union, schema, "the two sets do not partition the schema");
    }

    #[test]
    fn campaign_publication_requires_16_measured_and_21_of_37_omitted() {
        let document = profile_from(&DISTINCT).to_json();
        let sweeps = measured_sweeps();
        let omitted = omitted_fields(&document, &sweeps).unwrap();
        assert_eq!(
            validate_campaign_coverage(&document, &sweeps, &omitted),
            Ok(CampaignCoverage {
                measured: 16,
                omitted: 21,
                total: 37,
            })
        );
    }

    #[test]
    fn a_fifteen_field_run_cannot_publish_a_22_field_omission_set() {
        let document = profile_from(&DISTINCT).to_json();
        let mut sweeps = measured_sweeps();
        sweeps.retain(|sweep| sweep.field != CalibratedField::SimdMinWords);
        sweeps.push(concluded(
            CalibratedField::SimdMinWords,
            Fallback::NoComparableGridPoint,
        ));
        let omitted = omitted_fields(&document, &sweeps).unwrap();
        assert_eq!(measured_fields(&sweeps).len(), 15);
        assert_eq!(omitted.len(), 22);
        assert!(validate_campaign_coverage(&document, &sweeps, &omitted).is_err());
    }

    #[test]
    fn a_swept_field_without_a_comparison_joins_the_unswept_fields_in_the_omission_set() {
        let profile = profile_from(&DISTINCT);
        let mut sweeps = measured_sweeps();
        sweeps.retain(|sweep| sweep.field != CalibratedField::SubproductMinLen);
        sweeps.push(concluded(
            CalibratedField::SubproductMinLen,
            Fallback::NoComparableGridPoint,
        ));
        let omitted = omitted_fields(&profile.to_json(), &sweeps).unwrap();
        assert!(omitted.contains(&CalibratedField::SubproductMinLen.schema_field()));
        let document = calibrated_document(&profile, &omitted).unwrap();
        assert_eq!(
            ProducedCoreProfile::from_json(&document)
                .unwrap()
                .polynomial()
                .subproduct_min_len(),
            CoreTuning::CONSERVATIVE.polynomial().subproduct_min_len()
        );
    }

    #[test]
    fn a_family_scoped_removal_leaves_a_similarly_named_field_of_another_family() {
        let profile = profile_from(&DISTINCT);
        let document =
            calibrated_document(&profile, &[CalibratedField::SimdMinWords.schema_field()]).unwrap();
        let stated = schema_fields(&document).unwrap();
        assert!(!stated.contains(&CalibratedField::SimdMinWords.schema_field()));
        assert!(stated.contains(&SchemaField {
            family: "bit_matrix".to_owned(),
            name: "matvec_simd_min_words".to_owned(),
        }));
    }

    #[test]
    fn a_measured_field_is_never_omitted() {
        let points = vec![point(16, 100.0, 50.0, 0.01)];
        let sweep = FieldSweep {
            field: CalibratedField::DivRemFastMinLen,
            variant: SweepVariant::Standard,
            points: points.clone(),
            selection: select(CalibratedField::DivRemFastMinLen, &points),
        };
        assert!(uncalibrated_fields(&[sweep]).is_empty());
    }

    #[test]
    fn emitted_profile_round_trips_through_the_loader() {
        let profile = profile_from(&DISTINCT);
        let reparsed = ProducedCoreProfile::from_json(&profile.to_json())
            .expect("the harness emits a document the loader accepts");
        assert_eq!(reparsed, profile);
    }

    #[test]
    fn valid_output_is_published_only_after_canonical_reopen() {
        let output = TestOutput::new("valid");
        let document = profile_from(&DISTINCT).to_json();

        assert_eq!(emit_profile(&output.path, &document).unwrap(), document);
        assert_eq!(fs::read_to_string(&output.path).unwrap(), document);
        assert_eq!(
            fs::read_dir(&output.directory).unwrap().count(),
            1,
            "the validated final file is the only surviving directory entry"
        );
    }

    #[test]
    fn invalid_output_leaves_no_final_or_temporary_artifact() {
        let output = TestOutput::new("invalid");

        let error = emit_profile(&output.path, "{}")
            .expect_err("a document outside the strict owner schema must not publish");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(!output.path.exists());
        if output.directory.exists() {
            assert_eq!(fs::read_dir(&output.directory).unwrap().count(), 0);
        }
    }

    #[test]
    fn output_publication_never_replaces_an_existing_path() {
        let output = TestOutput::new("occupied");
        fs::create_dir_all(&output.directory).unwrap();
        fs::write(&output.path, "sentinel").unwrap();

        let error = emit_profile(&output.path, &profile_from(&DISTINCT).to_json())
            .expect_err("an existing artifact must win the publication race");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&output.path).unwrap(), "sentinel");
        assert_eq!(fs::read_dir(&output.directory).unwrap().count(), 1);
    }

    #[test]
    fn emitted_provenance_is_calibrated_and_populated() {
        let profile = profile_from(&DISTINCT);
        let MeasurementProvenance::Calibrated {
            harness_schema,
            binary_sha256,
            toolchain,
            host,
            ..
        } = &profile.measurement
        else {
            panic!("the calibration action emits calibrated provenance");
        };
        assert_eq!(harness_schema.as_str(), CoreTuningCodec::HARNESS_SCHEMA);
        assert_eq!(toolchain, "rustc 1.95.0");
        assert_eq!(host, "test-host");
        assert_eq!(binary_sha256.as_str(), "b".repeat(64));
        assert_eq!(profile.assembly.tool_sha256.as_str(), "c".repeat(64));
        assert_ne!(binary_sha256, &profile.assembly.tool_sha256);
    }

    #[test]
    fn dirty_source_is_rejected_before_calibration_can_start() {
        let revision = GitRevision::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
        assert_eq!(require_clean_source(false, &revision), Ok(()));
        let error = require_clean_source(true, &revision).unwrap_err();
        assert!(error.contains(revision.as_str()));
        assert!(error.contains("before any probe or timed call"));
    }

    #[test]
    fn campaign_requires_exactly_four_rayon_threads() {
        assert_eq!(validate_rayon_threads(Some("4")), Ok(()));
        for value in [
            None,
            Some(""),
            Some("four"),
            Some("3"),
            Some("04"),
            Some("5"),
        ] {
            assert!(validate_rayon_threads(value).is_err(), "accepted {value:?}");
        }
    }

    #[allow(dead_code)]
    type SweepManifestRow = (CalibratedField, SweepVariant, &'static [usize]);

    #[allow(dead_code)]
    const SWEEP_MANIFEST: &[SweepManifestRow] = &[
        (
            CalibratedField::SimdMinWords,
            SweepVariant::Standard,
            &[1, 2, 4, 7, 8, 9, 16, 32, 64],
        ),
        (
            CalibratedField::KaratsubaMinDegree,
            SweepVariant::Standard,
            &[4, 8, 16, 31, 32, 33, 64, 128, 256],
        ),
        (
            CalibratedField::KaratsubaMaxOutLen,
            SweepVariant::Standard,
            &[15, 31, 63, 127, 129, 191, 255, 383, 511],
        ),
        (
            CalibratedField::DivRemFastMinLen,
            SweepVariant::Standard,
            &[64, 128, 256, 512, 1024, 2047, 2048, 2049, 4096],
        ),
        (
            CalibratedField::SubproductMinLen,
            SweepVariant::Standard,
            &[128, 256, 512, 1024, 2048, 4095, 4096, 4097, 8192],
        ),
        (
            CalibratedField::TransposeSimpleMaxBlocks,
            SweepVariant::Standard,
            &[2, 4, 8, 15, 16, 17, 32, 64, 128],
        ),
        (
            CalibratedField::SoaParallelMinLen,
            SweepVariant::Standard,
            &[
                4096, 8192, 16_384, 32_767, 32_768, 32_769, 65_536, 131_072, 262_144,
            ],
        ),
        (
            CalibratedField::M4rmWideTierMinStrideWords,
            SweepVariant::Standard,
            &[2, 4, 8, 15, 16, 17, 32, 64, 128],
        ),
        (
            CalibratedField::M4rmTiledMinStrideWords,
            SweepVariant::Standard,
            &[4, 5, 6, 8, 12, 16, 24, 32, 64],
        ),
        (
            CalibratedField::DenseInverseM4riMinDim,
            SweepVariant::Standard,
            &[1, 2, 4, 7, 8, 9, 16, 32, 64],
        ),
        (
            CalibratedField::DenseInverseBlockedMinDim,
            SweepVariant::Standard,
            &[2, 4, 8, 15, 16, 17, 32, 64, 128],
        ),
        (
            CalibratedField::TrsmBlockedMinDim,
            SweepVariant::Standard,
            &[8, 16, 32, 63, 64, 65, 96, 128, 256],
        ),
        (
            CalibratedField::PlePanelBaseMaxCols,
            SweepVariant::Standard,
            &[16, 32, 64, 96, 127, 128, 129, 160, 256],
        ),
        (
            CalibratedField::PleBlockedBackSubMinDim,
            SweepVariant::Standard,
            &[16, 32, 64, 96, 127, 128, 129, 192, 256],
        ),
        (
            CalibratedField::GemmAxpyFastPathMinVolume,
            SweepVariant::Standard,
            &[64, 512, 1728, 3375, 4096, 4913, 8000, 13_824, 32_768],
        ),
        (
            CalibratedField::InterpolateFastMinPoints,
            SweepVariant::GenericInterpolation,
            &[2, 4, 8, 15, 16, 17, 32, 64, 128],
        ),
        (
            CalibratedField::InterpolateFastMinPoints,
            SweepVariant::TwoAdicInterpolation,
            &[2, 4, 8, 15, 16, 17, 32, 64, 128],
        ),
    ];

    #[test]
    fn canonical_manifest_pins_all_16_fields_17_sweeps_and_306_cells() {
        assert_eq!(SWEEP_MANIFEST.len(), 17);
        for field in CalibratedField::ALL {
            let rows: Vec<_> = SWEEP_MANIFEST
                .iter()
                .filter(|(candidate, _, _)| *candidate == field)
                .collect();
            assert_eq!(rows.len(), field.variants().len(), "{field}");
            for (variant, grid) in field.variants().iter().zip(rows) {
                assert_eq!(&grid.1, variant, "{field}");
                assert_eq!(grid.2, field.grid(), "{field}/{variant}");
            }
        }
        let arm_cells: usize = SWEEP_MANIFEST
            .iter()
            .map(|(_, _, grid)| grid.len() * Arm::BOTH.len())
            .sum();
        let grid_points: usize = SWEEP_MANIFEST.iter().map(|(_, _, grid)| grid.len()).sum();
        assert_eq!(CalibratedField::ALL.len(), 16);
        assert_eq!(grid_points, 153);
        assert_eq!(arm_cells, 306);
        let protocol = Protocol {
            executions: 5,
            repetitions: 5,
            target_ms: 250,
        };
        assert_eq!(
            planned_campaign_accounting(&protocol),
            CampaignAccounting {
                cells: 306,
                probes: 306,
                timed: 1530,
                launches: 1836,
                windows: 7650,
            }
        );
        assert_eq!(7650_u64 * protocol.target_ms, 1_912_500);
    }

    #[allow(dead_code)]
    fn forced_values_oracle(spec: ChildSpec) -> Vec<ForcedValue> {
        let lower = match spec.arm {
            Arm::Conservative => spec.size + 1,
            Arm::Asymptotic => spec.size,
        };
        let upper = match spec.arm {
            Arm::Conservative => spec.size,
            Arm::Asymptotic => spec.size - 1,
        };
        match spec.field {
            CalibratedField::SimdMinWords => {
                vec![ForcedValue::new("bit_backend", "simd_min_words", spec.size)]
            }
            CalibratedField::KaratsubaMinDegree => vec![ForcedValue::new(
                "polynomial",
                "karatsuba_min_degree",
                match spec.arm {
                    Arm::Conservative => usize::MAX,
                    Arm::Asymptotic => spec.size,
                },
            )],
            CalibratedField::KaratsubaMaxOutLen => vec![ForcedValue::new(
                "polynomial",
                "karatsuba_max_out_len",
                upper,
            )],
            CalibratedField::DivRemFastMinLen => vec![ForcedValue::new(
                "polynomial",
                "div_rem_fast_min_len",
                lower,
            )],
            CalibratedField::SubproductMinLen => {
                vec![ForcedValue::new("polynomial", "subproduct_min_len", lower)]
            }
            CalibratedField::InterpolateFastMinPoints => vec![ForcedValue::new(
                "polynomial",
                "interpolate_fast_min_points",
                lower,
            )],
            CalibratedField::TransposeSimpleMaxBlocks => vec![ForcedValue::new(
                "bit_matrix",
                "transpose_simple_max_blocks",
                upper,
            )],
            CalibratedField::SoaParallelMinLen => vec![
                ForcedValue::new("soa_batch", "parallel_min_len", lower),
                ForcedValue::new("soa_batch", "parallel_chunk_len", 16_384),
            ],
            CalibratedField::M4rmWideTierMinStrideWords => vec![
                ForcedValue::new("m4rm", "wide_tier_min_stride_words", lower),
                ForcedValue::new("m4rm", "tiled_min_stride_words", usize::MAX),
            ],
            CalibratedField::M4rmTiledMinStrideWords => {
                vec![ForcedValue::new("m4rm", "tiled_min_stride_words", lower)]
            }
            CalibratedField::DenseInverseM4riMinDim => {
                vec![ForcedValue::new("dense_inverse", "m4ri_min_dim", lower)]
            }
            CalibratedField::DenseInverseBlockedMinDim => {
                vec![ForcedValue::new("dense_inverse", "blocked_min_dim", lower)]
            }
            CalibratedField::TrsmBlockedMinDim => vec![
                ForcedValue::new("triangular", "trsm_blocked_min_dim", lower),
                ForcedValue::new("triangular", "trsm_panel_rows", 64),
            ],
            CalibratedField::PlePanelBaseMaxCols => vec![
                ForcedValue::new("ple", "panel_base_max_cols", upper),
                ForcedValue::new("ple", "panel_byte_lane_max_cols", 256),
            ],
            CalibratedField::PleBlockedBackSubMinDim => {
                vec![ForcedValue::new("ple", "blocked_back_sub_min_dim", lower)]
            }
            CalibratedField::GemmAxpyFastPathMinVolume => {
                vec![ForcedValue::new("gemm", "axpy_fast_path_min_volume", lower)]
            }
        }
    }

    #[allow(dead_code)]
    fn assert_forcing_and_codec(fields: &[CalibratedField]) {
        let conservative = complete_selector_value(&CoreTuning::CONSERVATIVE).unwrap();
        for &(field, variant, grid) in SWEEP_MANIFEST {
            if !fields.contains(&field) {
                continue;
            }
            for &size in grid {
                for arm in Arm::BOTH {
                    let spec = ChildSpec {
                        field,
                        variant,
                        size,
                        arm,
                        task: ChildTask::Probe,
                    };
                    let (prepared, active_values) = forced_profile_for(spec).unwrap();
                    let expected_values = forced_values_oracle(spec);
                    assert_eq!(active_values, expected_values, "{spec}");
                    let projection = prepared.section::<CoreTuning>().unwrap().unwrap();
                    let forced = complete_selector_value(projection.section).unwrap();
                    for expected in &expected_values {
                        assert_eq!(
                            forced
                                .pointer(&format!("/{}/{}", expected.family, expected.field))
                                .and_then(serde_json::Value::as_u64),
                            Some(expected.value as u64),
                            "{spec}: {}/{}",
                            expected.family,
                            expected.field
                        );
                    }
                    for (family, conservative_fields) in conservative.as_object().unwrap() {
                        for (name, conservative_value) in conservative_fields.as_object().unwrap() {
                            let forced_value = &forced[family][name];
                            if forced_value != conservative_value {
                                assert!(
                                    expected_values.iter().any(|expected| {
                                        expected.family == *family && expected.field == *name
                                    }),
                                    "{spec} changed undeclared {family}/{name}"
                                );
                            }
                        }
                    }
                    forced_profile_digests(&prepared).unwrap();
                }
            }
        }
    }

    #[test]
    fn direct_selector_forcing_and_codec_are_exact_at_every_grid_arm() {
        assert_forcing_and_codec(&[
            CalibratedField::SimdMinWords,
            CalibratedField::KaratsubaMinDegree,
            CalibratedField::KaratsubaMaxOutLen,
            CalibratedField::DivRemFastMinLen,
            CalibratedField::SubproductMinLen,
        ]);
    }

    #[test]
    fn bit_and_parallel_forcing_and_codec_are_exact_at_every_grid_arm() {
        assert_forcing_and_codec(&[
            CalibratedField::TransposeSimpleMaxBlocks,
            CalibratedField::SoaParallelMinLen,
            CalibratedField::M4rmWideTierMinStrideWords,
            CalibratedField::M4rmTiledMinStrideWords,
            CalibratedField::DenseInverseM4riMinDim,
        ]);
    }

    #[test]
    fn field_matrix_forcing_and_codec_are_exact_at_every_grid_arm() {
        assert_forcing_and_codec(&[
            CalibratedField::DenseInverseBlockedMinDim,
            CalibratedField::TrsmBlockedMinDim,
            CalibratedField::PlePanelBaseMaxCols,
            CalibratedField::PleBlockedBackSubMinDim,
        ]);
    }

    #[test]
    fn gemm_and_interpolation_forcing_and_codec_are_exact_at_every_grid_arm() {
        assert_forcing_and_codec(&[
            CalibratedField::GemmAxpyFastPathMinVolume,
            CalibratedField::InterpolateFastMinPoints,
        ]);
    }

    #[test]
    fn every_grid_straddles_or_starts_at_its_boundary_default() {
        for field in CalibratedField::ALL {
            let grid = field.grid();
            let default = field.conservative_default();
            if field == CalibratedField::M4rmTiledMinStrideWords {
                assert_eq!(
                    grid.first(),
                    Some(&default),
                    "{field} must start at its codec-enforced boundary default"
                );
            } else {
                assert!(
                    grid.iter().any(|&size| size < default),
                    "{field} has no grid point below {default}"
                );
            }
            assert!(
                grid.iter().any(|&size| size > default),
                "{field} has no grid point above {default}"
            );
            assert!(
                grid.windows(2).all(|pair| pair[0] < pair[1]),
                "{field} grid is not strictly ascending: {grid:?}"
            );
            assert!(grid.iter().all(|&size| size > 0), "{field} grid has a zero");
        }
    }

    #[test]
    fn karatsuba_out_len_grid_spans_the_recorded_mul_fast_step() {
        let grid = CalibratedField::KaratsubaMaxOutLen.grid();
        assert!(grid.contains(&127) && grid.contains(&129));
        assert!(grid.iter().any(|&size| size >= 255));
        assert!(
            grid.iter().all(|&size| !size.is_multiple_of(2)),
            "equal-length operands give an odd product length: {grid:?}"
        );
    }

    #[test]
    fn bit_word_fixture_consumes_rows_in_word_order_and_masks_the_tail() {
        let seed = 0x1234_5678_9abc_def0;
        let matrix = bit_matrix_from_words(2, 65, seed);
        let mut rng = Lcg::new(seed);
        let expected = [
            [rng.next_u64(), rng.next_u64() & 1],
            [rng.next_u64(), rng.next_u64() & 1],
        ];
        assert_eq!(matrix.row_words(0), &expected[0]);
        assert_eq!(matrix.row_words(1), &expected[1]);
        assert_eq!(matrix.row_words(0)[1] & !1, 0);
        assert_eq!(matrix.row_words(1)[1] & !1, 0);
    }

    #[test]
    fn word_parity_m4rm_oracle_matches_the_independent_bit_oracle() {
        let lhs = bit_matrix_from_words(3, 65, 0x1111);
        let rhs = bit_matrix_from_words(65, 66, 0x2222);
        assert_eq!(
            scalar_m4rm_oracle(&lhs, &rhs),
            scalar_bit_matmul(&lhs, &rhs)
        );
    }

    #[test]
    fn soa_operand_digest_is_bank_major_then_role_major() {
        let quadratic = |base: u64| {
            (0..BIT_FIXTURES)
                .map(|bank| {
                    BatchExtField::new([
                        vec![F::new(base + bank as u64)],
                        vec![F::new(base + 100 + bank as u64)],
                    ])
                })
                .collect::<Vec<_>>()
        };
        let cubic = |base: u64| {
            (0..BIT_FIXTURES)
                .map(|bank| {
                    BatchExtField::new([
                        vec![F::new(base + bank as u64)],
                        vec![F::new(base + 100 + bank as u64)],
                        vec![F::new(base + 200 + bank as u64)],
                    ])
                })
                .collect::<Vec<_>>()
        };
        let fixture = FollowOnFixture::Soa {
            quadratic_lhs: quadratic(1),
            quadratic_rhs: quadratic(20),
            cubic_lhs: cubic(40),
            cubic_rhs: cubic(80),
        };
        let FollowOnFixture::Soa {
            quadratic_lhs,
            quadratic_rhs,
            cubic_lhs,
            cubic_rhs,
        } = &fixture
        else {
            unreachable!()
        };
        let bank_major = digest_tuple(
            b"gf2-calibration-soa-banks-v1",
            (0..BIT_FIXTURES).flat_map(|bank| {
                [
                    digest_batch(&quadratic_lhs[bank]),
                    digest_batch(&quadratic_rhs[bank]),
                    digest_batch(&cubic_lhs[bank]),
                    digest_batch(&cubic_rhs[bank]),
                ]
            }),
        );
        let role_major = digest_tuple(
            b"gf2-calibration-soa-banks-v1",
            quadratic_lhs
                .iter()
                .map(digest_batch)
                .chain(quadratic_rhs.iter().map(digest_batch))
                .chain(cubic_lhs.iter().map(digest_batch))
                .chain(cubic_rhs.iter().map(digest_batch)),
        );
        assert_eq!(follow_on_operand_digest(&fixture), bank_major);
        assert_ne!(bank_major, role_major);
    }

    #[test]
    fn a_monotone_crossover_selects_its_smallest_winning_grid_point() {
        let points = vec![
            point(16, 100.0, 120.0, 0.01),
            point(32, 100.0, 90.0, 0.01),
            point(64, 100.0, 50.0, 0.01),
        ];
        let selection = select(CalibratedField::DivRemFastMinLen, &points);
        assert!(matches!(
            selection,
            Selection::Crossover {
                value: 32,
                crossover: 32,
                ..
            }
        ));
    }

    #[test]
    fn an_upper_bound_field_selects_the_grid_point_below_the_crossover() {
        let points = vec![
            point(63, 100.0, 120.0, 0.01),
            point(127, 100.0, 110.0, 0.01),
            point(255, 100.0, 50.0, 0.01),
        ];
        let selection = select(CalibratedField::KaratsubaMaxOutLen, &points);
        assert!(matches!(
            selection,
            Selection::Crossover {
                value: 127,
                crossover: 255,
                ..
            }
        ));
    }

    #[test]
    fn transpose_uses_reversed_upper_bound_publication() {
        let points = vec![
            point(2, 100.0, 120.0, 0.01),
            point(4, 100.0, 50.0, 0.01),
            point(8, 100.0, 40.0, 0.01),
        ];
        assert!(matches!(
            select(CalibratedField::TransposeSimpleMaxBlocks, &points),
            Selection::Crossover {
                value: 2,
                crossover: 4,
                ..
            }
        ));
    }

    #[test]
    fn ple_first_grid_win_uses_its_codec_admissible_floor() {
        let points = vec![point(16, 100.0, 50.0, 0.01), point(32, 100.0, 40.0, 0.01)];
        assert!(matches!(
            select(CalibratedField::PlePanelBaseMaxCols, &points),
            Selection::Crossover {
                value: 1,
                crossover: 16,
                ..
            }
        ));
    }

    #[test]
    fn a_margin_inside_the_noise_band_does_not_win() {
        let points = vec![point(32, 100.0, 97.0, 0.05)];
        assert_eq!(
            select(CalibratedField::DivRemFastMinLen, &points),
            Selection::KeptDefault {
                value: CalibratedField::DivRemFastMinLen.conservative_default(),
                reason: Fallback::NoGridPointWins,
            }
        );
    }

    #[test]
    fn a_non_monotone_crossover_keeps_the_conservative_default() {
        let points = vec![
            point(16, 100.0, 120.0, 0.01),
            point(32, 100.0, 50.0, 0.01),
            point(64, 100.0, 130.0, 0.01),
        ];
        assert_eq!(
            select(CalibratedField::SubproductMinLen, &points),
            Selection::KeptDefault {
                value: CalibratedField::SubproductMinLen.conservative_default(),
                reason: Fallback::NonMonotone {
                    first_win: 32,
                    later_loss: 64,
                },
            }
        );
    }

    #[test]
    fn a_grid_with_one_arm_everywhere_keeps_the_conservative_default() {
        let points: Vec<GridPoint> = [16_usize, 32, 64]
            .into_iter()
            .map(|size| GridPoint {
                size,
                conservative: Some(ArmStat {
                    median: 100.0,
                    spread: 0.01,
                    samples: Vec::new(),
                }),
                asymptotic: None,
            })
            .collect();
        assert_eq!(
            select(CalibratedField::KaratsubaMinDegree, &points),
            Selection::KeptDefault {
                value: CalibratedField::KaratsubaMinDegree.conservative_default(),
                reason: Fallback::NoComparableGridPoint,
            }
        );
    }

    #[test]
    fn a_missing_comparison_before_or_after_a_win_is_uncalibrated() {
        let missing = |size| GridPoint {
            size,
            conservative: Some(ArmStat {
                median: 100.0,
                spread: 0.01,
                samples: Vec::new(),
            }),
            asymptotic: None,
        };
        for points in [
            vec![
                missing(16),
                point(32, 100.0, 50.0, 0.01),
                point(64, 100.0, 40.0, 0.01),
            ],
            vec![
                point(16, 100.0, 50.0, 0.01),
                missing(32),
                point(64, 100.0, 40.0, 0.01),
            ],
        ] {
            assert_eq!(
                select(CalibratedField::DivRemFastMinLen, &points),
                Selection::KeptDefault {
                    value: CalibratedField::DivRemFastMinLen.conservative_default(),
                    reason: Fallback::NoComparableGridPoint,
                }
            );
        }
    }

    #[test]
    fn interpolation_reconciliation_uses_max_default_and_uncalibrated_precedence() {
        let crossover = |value| Selection::Crossover {
            value,
            crossover: value,
            band: 0.01,
            margin: 0.10,
        };
        let default = CalibratedField::InterpolateFastMinPoints.conservative_default();
        let measured_default = Selection::KeptDefault {
            value: default,
            reason: Fallback::NoGridPointWins,
        };
        let uncalibrated = Selection::KeptDefault {
            value: default,
            reason: Fallback::NoComparableGridPoint,
        };

        assert_eq!(
            reconcile_interpolation(&crossover(16), &crossover(32)),
            crossover(32)
        );
        assert_eq!(
            reconcile_interpolation(&crossover(32), &measured_default),
            measured_default
        );
        assert_eq!(
            reconcile_interpolation(&measured_default, &uncalibrated),
            uncalibrated
        );
    }

    #[test]
    fn interpolation_reconciliation_line_records_both_variants_and_shared_result() {
        let generic = Selection::KeptDefault {
            value: 16,
            reason: Fallback::NoGridPointWins,
        };
        let two_adic = Selection::Crossover {
            value: 32,
            crossover: 32,
            band: 0.01,
            margin: 0.10,
        };
        let shared = reconcile_interpolation(&generic, &two_adic);
        let line = interpolation_reconciliation_line(&generic, &two_adic, &shared).unwrap();
        assert!(line.starts_with(INTERPOLATION_RECONCILIATION_PREFIX));
        assert!(line.contains("\"generic\""));
        assert!(line.contains("\"two_adic\""));
        assert!(line.contains("\"shared\""));
        assert!(line.contains("\"no_grid_point_wins\""));
    }

    #[test]
    fn selected_values_fall_back_to_the_defaults_of_unswept_fields() {
        let selected = SelectedValues::from_sweeps(&[]);
        assert_eq!(
            selected,
            SelectedValues {
                simd_min_words: CalibratedField::SimdMinWords.conservative_default(),
                karatsuba_min_degree: CalibratedField::KaratsubaMinDegree.conservative_default(),
                karatsuba_max_out_len: CalibratedField::KaratsubaMaxOutLen.conservative_default(),
                div_rem_fast_min_len: CalibratedField::DivRemFastMinLen.conservative_default(),
                subproduct_min_len: CalibratedField::SubproductMinLen.conservative_default(),
                transpose_simple_max_blocks: CalibratedField::TransposeSimpleMaxBlocks
                    .conservative_default(),
                soa_parallel_min_len: CalibratedField::SoaParallelMinLen.conservative_default(),
                m4rm_wide_tier_min_stride_words: CalibratedField::M4rmWideTierMinStrideWords
                    .conservative_default(),
                m4rm_tiled_min_stride_words: CalibratedField::M4rmTiledMinStrideWords
                    .conservative_default(),
                dense_inverse_m4ri_min_dim: CalibratedField::DenseInverseM4riMinDim
                    .conservative_default(),
                dense_inverse_blocked_min_dim: CalibratedField::DenseInverseBlockedMinDim
                    .conservative_default(),
                trsm_blocked_min_dim: CalibratedField::TrsmBlockedMinDim.conservative_default(),
                ple_panel_base_max_cols: CalibratedField::PlePanelBaseMaxCols
                    .conservative_default(),
                ple_blocked_back_sub_min_dim: CalibratedField::PleBlockedBackSubMinDim
                    .conservative_default(),
                gemm_axpy_fast_path_min_volume: CalibratedField::GemmAxpyFastPathMinVolume
                    .conservative_default(),
                interpolate_fast_min_points: CalibratedField::InterpolateFastMinPoints
                    .conservative_default(),
            }
        );
    }

    #[test]
    fn a_default_valued_sweep_reproduces_the_conservative_selectors() {
        let selected = SelectedValues::from_sweeps(&[]);
        let profile = profile_from(&selected);
        let conservative = &CoreTuning::CONSERVATIVE;
        assert_eq!(profile.bit_backend(), conservative.bit_backend());
        assert_eq!(profile.polynomial(), conservative.polynomial());
    }

    #[test]
    fn statistics_summarise_a_window_sample() {
        let samples = [10_u64, 12, 8, 11, 9]
            .into_iter()
            .enumerate()
            .map(|(repetition, elapsed_ns)| TimingSample {
                execution: 0,
                repetition: repetition as u64,
                calls: 1,
                elapsed_ns,
            })
            .collect();
        let stat = ArmStat::from_samples(samples);
        assert_eq!(stat.median, 10.0);
        assert_eq!(stat.windows(), 5);
        assert_eq!(
            stat.samples
                .iter()
                .map(|sample| sample.elapsed_ns)
                .collect::<Vec<_>>(),
            [10, 12, 8, 11, 9],
            "statistics must retain raw acquisition order"
        );
        // Nearest-rank quartiles of the five windows are 9 and 11.
        assert!((stat.spread - 0.2).abs() < 1e-12);
    }

    #[test]
    fn an_even_sample_medians_between_its_central_windows() {
        let samples = vec![
            TimingSample {
                execution: 0,
                repetition: 0,
                calls: 1,
                elapsed_ns: 10,
            },
            TimingSample {
                execution: 0,
                repetition: 1,
                calls: 1,
                elapsed_ns: 20,
            },
        ];
        assert_eq!(ArmStat::from_samples(samples).median, 15.0);
    }

    #[test]
    fn raw_sample_line_is_canonical_and_pins_format_two_identity() {
        let samples = [
            TimingSample {
                execution: 0,
                repetition: 0,
                calls: 4,
                elapsed_ns: 40,
            },
            TimingSample {
                execution: 0,
                repetition: 1,
                calls: 4,
                elapsed_ns: 44,
            },
        ];
        assert_eq!(
            sample_record_line(
                CalibratedField::KaratsubaMinDegree,
                SweepVariant::Standard,
                31,
                Arm::Asymptotic,
                &samples,
            )
            .unwrap(),
            "GF2_TUNING_SAMPLES={\"schema\":\"raw-timing-samples-v3\",\"profile_format_version\":2,\"section_id\":\"gf2-core/selectors\",\"section_schema_version\":1,\"harness_schema\":\"tuning-calibration-v4\",\"field\":\"karatsuba_min_degree\",\"variant\":\"standard\",\"size\":31,\"arm\":\"asymptotic\",\"samples\":[{\"execution\":0,\"repetition\":0,\"calls\":4,\"elapsed_ns\":40},{\"execution\":0,\"repetition\":1,\"calls\":4,\"elapsed_ns\":44}]}"
        );
    }

    #[test]
    fn seed_tags_and_streams_are_stable_and_explicit() {
        assert_eq!(
            CalibratedField::ALL.map(CalibratedField::seed_tag),
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
        for (field, size, role, expected) in [
            (
                CalibratedField::SimdMinWords,
                1,
                SEED_ROLE_BIT_DST,
                0x6bca_b429_7aa0_dfbc,
            ),
            (
                CalibratedField::KaratsubaMinDegree,
                4,
                SEED_ROLE_POLY_LHS,
                0x35af_77d8_8d1e_27e1,
            ),
            (
                CalibratedField::KaratsubaMaxOutLen,
                15,
                SEED_ROLE_POLY_RHS,
                0x3c26_d083_d284_6ea3,
            ),
            (
                CalibratedField::DivRemFastMinLen,
                64,
                SEED_ROLE_DIVIDEND,
                0xed46_bff6_1b47_ac5f,
            ),
            (
                CalibratedField::SubproductMinLen,
                128,
                SEED_ROLE_EVAL_POINTS,
                0xf137_5a83_95b8_3f18,
            ),
        ] {
            assert_eq!(seed_for(field, size, role), expected, "{field}:{size}");
        }
        let line = seed_inventory_line(CalibratedField::KaratsubaMinDegree, 31).unwrap();
        assert!(line.starts_with(SEED_PREFIX));
        assert!(line.contains("\"derivation\":\"gf2-calibration-seed-v1\""));
        assert!(line.contains("\"field_tag\":1"));
        assert!(line.contains("\"name\":\"lhs\",\"role\":10,\"seed\":"));
        assert!(line.contains("\"name\":\"rhs\",\"role\":11,\"seed\":"));
    }

    #[allow(dead_code)]
    fn seed_oracle(field_tag: u64, size: usize, role: u64) -> u64 {
        let mut value = 0x5ecc_9bf8_0000_0000 ^ role.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for word in [field_tag, size as u64] {
            value ^= word;
            value = value
                .wrapping_mul(0xbf58_476d_1ce4_e5b9)
                .rotate_left(27)
                .wrapping_add(0x94d0_49bb_1331_11eb);
        }
        value ^ (value >> 31)
    }

    #[allow(dead_code)]
    fn follow_on_roles_oracle(field: CalibratedField) -> &'static [(&'static str, u64)] {
        match field {
            CalibratedField::TransposeSimpleMaxBlocks => &[("matrix", 0x100)],
            CalibratedField::SoaParallelMinLen => &[
                ("quadratic_lhs", 0x200),
                ("quadratic_rhs", 0x201),
                ("cubic_lhs", 0x202),
                ("cubic_rhs", 0x203),
            ],
            CalibratedField::M4rmWideTierMinStrideWords => &[("lhs", 0x300), ("rhs", 0x301)],
            CalibratedField::M4rmTiledMinStrideWords => &[("lhs", 0x310), ("rhs", 0x311)],
            CalibratedField::DenseInverseM4riMinDim => {
                &[("unit_lower", 0x400), ("unit_upper", 0x401)]
            }
            CalibratedField::DenseInverseBlockedMinDim => {
                &[("unit_lower", 0x500), ("unit_upper", 0x501)]
            }
            CalibratedField::TrsmBlockedMinDim => {
                &[("unit_lower", 0x600), ("unit_upper", 0x601), ("rhs", 0x602)]
            }
            CalibratedField::PlePanelBaseMaxCols => &[("unit_lower", 0x700), ("unit_upper", 0x701)],
            CalibratedField::PleBlockedBackSubMinDim => &[
                ("designated_nonzero", 0x800),
                ("non_designated", 0x801),
                ("row_mix", 0x802),
            ],
            CalibratedField::GemmAxpyFastPathMinVolume => &[("lhs", 0x900), ("rhs", 0x901)],
            CalibratedField::InterpolateFastMinPoints => {
                &[("coefficients", 0xa00), ("point_offset", 0xa01)]
            }
            _ => unreachable!("direct fields have separate seed roles"),
        }
    }

    #[test]
    fn every_follow_on_seed_inventory_is_bank_major_then_role_major() {
        for &(field, _, grid) in SWEEP_MANIFEST {
            if field.seed_tag() <= 4 {
                continue;
            }
            let roles = follow_on_roles_oracle(field);
            for &size in grid {
                let inventory = seed_inventory(field, size);
                assert_eq!(inventory.streams.len(), 8 * roles.len(), "{field}/{size}");
                for bank in 0..8 {
                    for (role_index, &(name, role)) in roles.iter().enumerate() {
                        let stream = &inventory.streams[bank * roles.len() + role_index];
                        let banked_role = role + ((bank as u64) << 16);
                        assert_eq!(stream.name, format!("{name}[{bank}]"));
                        assert_eq!(stream.role, banked_role);
                        assert_eq!(
                            stream.seed,
                            seed_oracle(field.seed_tag(), size, banked_role),
                            "{field}/{size}/{name}/{bank}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_binary_fixture_uses_the_predeclared_plus_three_bank() {
        assert_eq!(
            (0..8).map(paired_bank).collect::<Vec<_>>(),
            vec![3, 4, 5, 6, 7, 0, 1, 2]
        );
    }

    #[test]
    fn calibration_requires_the_neutral_owner_operation() {
        assert!(parse_args(
            [
                "--out",
                "/tmp/x.json",
                "--lock-wrapper",
                "w",
                "--receipt",
                "r"
            ]
            .map(str::to_owned)
            .into_iter()
        )
        .is_err());
        assert_eq!(
            parse_args(["--owner-operation".to_owned()].into_iter())
                .unwrap()
                .mode,
            Mode::OwnerOperation
        );
    }

    #[test]
    fn a_zero_protocol_constant_is_rejected() {
        for flag in ["--executions", "--repetitions", "--target-ms"] {
            let args = [flag, "0", "--self-check"].map(str::to_owned);
            assert!(
                parse_args(args.into_iter()).is_err(),
                "{flag} accepted zero"
            );
        }
    }

    #[test]
    fn the_reporting_modes_need_no_output_path() {
        let self_check = parse_args(["--self-check".to_owned()].into_iter()).unwrap();
        assert_eq!(self_check.mode, Mode::SelfCheck);
        let list_grid = parse_args(["--list-grid".to_owned()].into_iter()).unwrap();
        assert_eq!(list_grid.mode, Mode::ListGrid);
        let capabilities = parse_args(["--capability-report".to_owned()].into_iter()).unwrap();
        assert_eq!(capabilities.mode, Mode::CapabilityReport);
    }

    /// A spec whose four components are pairwise distinguishable, so a
    /// transposition in the rendered form cannot round-trip.
    #[allow(dead_code)]
    const CHILD_SPEC: ChildSpec = ChildSpec {
        field: CalibratedField::KaratsubaMinDegree,
        variant: SweepVariant::Standard,
        size: 31,
        arm: Arm::Asymptotic,
        task: ChildTask::Measure { execution: 2 },
    };

    #[allow(dead_code)]
    fn child_report(spec: ChildSpec, protocol: &Protocol) -> ChildReport {
        let (profile, active_values) = forced_profile_for(spec).unwrap();
        let digests = forced_profile_digests(&profile).unwrap();
        let (effective_observation, capability_observation) =
            expected_observation_contract(spec).unwrap();
        let samples = match spec.task {
            ChildTask::Probe => Vec::new(),
            ChildTask::Measure { execution } => (0..protocol.repetitions)
                .map(|repetition| TimingSample {
                    execution,
                    repetition,
                    calls: 10,
                    elapsed_ns: 100 + repetition,
                })
                .collect(),
        };
        ChildReport {
            protocol: ChildProtocolIdentity::current(protocol),
            installed: InstalledEvidence {
                profile_id: FORCED_ARM_PROFILE_ID.to_owned(),
                section_id: CoreTuning::ID.as_str().to_owned(),
                resolution: ObservedResolution::Installed,
                measurement: ObservedMeasurement::Inherited,
                active_values,
                section_sha256: digests.section_sha256,
                envelope_content_sha256: digests.envelope_content_sha256,
            },
            fixture_shape: expected_fixture_shape(spec.field, spec.size).unwrap(),
            seed_inventory: seed_inventory(spec.field, spec.size),
            operand_digest: expected_operand_digest(spec.field, spec.size).unwrap(),
            outcome: ChildOutcome::Complete {
                requested_route: spec.field.arm_name(spec.arm).to_owned(),
                observed_route: spec.field.arm_name(spec.arm).to_owned(),
                effective_observation: Some(effective_observation),
                capability_observation: Some(capability_observation),
                result_digest: "2".repeat(64),
                equivalence_digest: digest_tuple(
                    b"gf2-calibration-equivalence-v1",
                    [
                        expected_operand_digest(spec.field, spec.size).unwrap(),
                        "2".repeat(64),
                    ],
                ),
                samples,
            },
        }
    }

    #[allow(dead_code)]
    fn unavailable_report(
        spec: ChildSpec,
        protocol: &Protocol,
        observed_route: &str,
        omission: CapabilityOmission,
        capability_observation: &str,
    ) -> ChildReport {
        assert!(matches!(spec.task, ChildTask::Probe));
        let mut report = child_report(spec, protocol);
        report.outcome = ChildOutcome::Unavailable {
            requested_route: spec.field.arm_name(spec.arm).to_owned(),
            observed_route: observed_route.to_owned(),
            effective_observation: expected_omission_effective(&omission).to_owned(),
            omission,
            capability_observation: capability_observation.to_owned(),
        };
        report
    }

    #[allow(dead_code)]
    fn child_protocol(repetitions: u64) -> Protocol {
        Protocol {
            executions: 3,
            repetitions,
            target_ms: 1,
        }
    }

    #[test]
    fn a_fresh_process_case_has_one_pinned_canonical_stdin_encoding() {
        let case = FreshProcessCase {
            spec: CHILD_SPEC,
            protocol: child_protocol(3),
        };
        let encoded = serde_json::to_string(&case).unwrap();
        assert_eq!(
            encoded,
            r#"{"spec":{"field":"karatsuba_min_degree","variant":"standard","size":31,"arm":"asymptotic","task":{"kind":"measure","execution":2}},"protocol":{"executions":3,"repetitions":3,"target_ms":1}}"#
        );
        assert_eq!(decode_fresh_case(&encoded), Ok(case));
        assert!(decode_fresh_case(&format!("{encoded}\n")).is_err());
        assert!(decode_fresh_case(&format!(" {encoded}")).is_err());
    }

    #[test]
    fn the_child_mode_accepts_no_case_data_outside_stdin() {
        let parsed = parse_args(["--fresh-tuning-process-child".to_owned()].into_iter()).unwrap();
        assert_eq!(parsed.mode, Mode::FreshChild);
        for args in [
            vec!["--fresh-tuning-process-child", "--list-grid"],
            vec!["--fresh-tuning-process-child", "--repetitions", "3"],
            vec!["--child-arm", "karatsuba_min_degree:31:asymptotic:2"],
        ] {
            assert!(
                parse_args(args.iter().copied().map(str::to_owned)).is_err(),
                "alternate child input {args:?} was accepted"
            );
        }
    }

    #[test]
    fn a_child_report_is_one_canonical_prefixed_json_line() {
        let protocol = child_protocol(2);
        let report = child_report(CHILD_SPEC, &protocol);
        let text = format!(
            "{FRESH_RESULT_PREFIX}{}\n",
            serde_json::to_string(&report).unwrap()
        );
        assert_eq!(parse_child_report(&text), Ok(report));
        assert!(text.contains("\"profile_format_version\":2"));
        assert!(text.contains("\"section_id\":\"gf2-core/selectors\""));
        assert!(text.contains("\"section_schema_version\":1"));
        assert!(text.contains("\"harness_schema\":\"tuning-calibration-v4\""));
        assert!(text.contains("\"resolution\":\"installed\""));
        assert!(text.contains(
            "\"samples\":[{\"execution\":2,\"repetition\":0,\"calls\":10,\"elapsed_ns\":100}"
        ));
    }

    #[test]
    fn an_incomplete_unprefixed_or_repeated_child_result_is_rejected() {
        for text in [
            "{}\n",
            "route\tkaratsuba\n",
            "GF2_TUNING_RESULT={\"operands\":1,\"product\":2,\"rates\":[]}\n",
            "GF2_TUNING_RESULT={\"route\":\"karatsuba\",\"operands\":1,\"product\":2,\"rates\":[]}\nextra\n",
        ] {
            assert!(
                parse_child_report(text).is_err(),
                "{text:?} was read as a report"
            );
        }
    }

    #[test]
    fn a_child_that_took_the_other_arm_is_rejected() {
        let spec = ChildSpec {
            task: ChildTask::Measure { execution: 0 },
            ..CHILD_SPEC
        };
        let protocol = child_protocol(1);
        let mut report = child_report(spec, &protocol);
        let ChildOutcome::Complete { observed_route, .. } = &mut report.outcome else {
            unreachable!()
        };
        *observed_route = "schoolbook".to_owned();
        let error =
            verify_child_report(spec, &report.operand_digest, &protocol, &report).unwrap_err();
        assert!(error.contains("schoolbook"), "{error}");
    }

    #[test]
    fn admitted_gemm_decline_is_a_typed_omission_and_never_a_conservative_failure() {
        assert_eq!(
            classify_gemm_effective(Arm::Asymptotic, Some(GemmAxpyRoute::PerCell)),
            Ok(ChildAvailability::Unavailable {
                omission: CapabilityOmission::GemmWholeKernelDeclined
            })
        );
        assert_eq!(
            classify_gemm_effective(Arm::Conservative, Some(GemmAxpyRoute::PerCell)),
            Ok(ChildAvailability::Available)
        );
        assert!(classify_gemm_effective(Arm::Asymptotic, None).is_err());
        assert!(
            classify_gemm_effective(Arm::Conservative, Some(GemmAxpyRoute::WholeGemm)).is_err()
        );
    }

    #[test]
    fn every_closed_capability_omission_is_accepted_only_on_its_probe_contract() {
        let protocol = child_protocol(1);
        let cases = [
            (
                CalibratedField::GemmAxpyFastPathMinVolume,
                64,
                Arm::Asymptotic,
                "per_cell",
                CapabilityOmission::GemmWholeKernelDeclined,
                "fp251_whole_gemm_available=true",
            ),
            (
                CalibratedField::SimdMinWords,
                1,
                Arm::Asymptotic,
                "unavailable_before_dispatch",
                CapabilityOmission::SimdBackendUnavailable,
                "simd_backend=none",
            ),
            (
                CalibratedField::SoaParallelMinLen,
                4096,
                Arm::Asymptotic,
                "unavailable_before_dispatch",
                CapabilityOmission::SoaPoolWidth { observed: 3 },
                "dedicated_pool_width=3",
            ),
            (
                CalibratedField::M4rmTiledMinStrideWords,
                4,
                Arm::Asymptotic,
                "row_wise",
                CapabilityOmission::M4rmRegisterTiledUnavailable,
                "simd_tile8xn=unavailable",
            ),
            (
                CalibratedField::PlePanelBaseMaxCols,
                16,
                Arm::Asymptotic,
                "unavailable_before_dispatch",
                CapabilityOmission::PleByteLaneUnavailable,
                "carrier_lane=None",
            ),
            (
                CalibratedField::PlePanelBaseMaxCols,
                16,
                Arm::Asymptotic,
                "kernel_declined",
                CapabilityOmission::PlePanelKernelDeclined,
                "carrier_lane=byte panel_byte_lane_max_cols=256",
            ),
            (
                CalibratedField::GemmAxpyFastPathMinVolume,
                64,
                Arm::Asymptotic,
                "unavailable_before_dispatch",
                CapabilityOmission::Fp251WholeGemmUnavailable,
                "fp251_whole_gemm_available=false",
            ),
            (
                CalibratedField::TrsmBlockedMinDim,
                8,
                Arm::Asymptotic,
                "blocked_callee_declined",
                CapabilityOmission::TrsmBlockedCalleeDeclined,
                "fp251_whole_gemm_available=true",
            ),
        ];
        for (field, size, arm, observed, omission, capability) in cases {
            let spec = ChildSpec {
                field,
                variant: SweepVariant::Standard,
                size,
                arm,
                task: ChildTask::Probe,
            };
            let report = unavailable_report(spec, &protocol, observed, omission, capability);
            verify_child_report(spec, &report.operand_digest, &protocol, &report)
                .unwrap_or_else(|error| panic!("{spec}: {error}"));
            let mut stale = report.clone();
            let ChildOutcome::Unavailable {
                effective_observation,
                ..
            } = &mut stale.outcome
            else {
                unreachable!();
            };
            *effective_observation = "stale".to_owned();
            assert!(verify_child_report(spec, &stale.operand_digest, &protocol, &stale).is_err());
            let timed = ChildSpec {
                task: ChildTask::Measure { execution: 0 },
                ..spec
            };
            assert!(
                verify_child_report(timed, &report.operand_digest, &protocol, &report).is_err()
            );
        }

        let conservative_spec = ChildSpec {
            field: CalibratedField::GemmAxpyFastPathMinVolume,
            variant: SweepVariant::Standard,
            size: 64,
            arm: Arm::Conservative,
            task: ChildTask::Probe,
        };
        let false_omission = unavailable_report(
            conservative_spec,
            &protocol,
            "unavailable_before_dispatch",
            CapabilityOmission::Fp251WholeGemmUnavailable,
            "fp251_whole_gemm_available=false",
        );
        assert!(verify_child_report(
            conservative_spec,
            &false_omission.operand_digest,
            &protocol,
            &false_omission
        )
        .is_err());
    }

    #[test]
    fn a_child_that_built_other_operands_is_rejected() {
        let spec = ChildSpec {
            task: ChildTask::Measure { execution: 0 },
            ..CHILD_SPEC
        };
        let protocol = child_protocol(1);
        let report = child_report(spec, &protocol);
        assert!(verify_child_report(spec, &"4".repeat(64), &protocol, &report).is_err());
    }

    #[test]
    fn paired_evidence_rejects_operand_result_and_equivalence_drift() {
        let protocol = child_protocol(1);
        let expected = child_report(CHILD_SPEC, &protocol);
        for field in ["operand", "result", "equivalence"] {
            let mut changed = expected.clone();
            match field {
                "operand" => changed.operand_digest = "4".repeat(64),
                "result" => {
                    let ChildOutcome::Complete { result_digest, .. } = &mut changed.outcome else {
                        unreachable!()
                    };
                    *result_digest = "4".repeat(64);
                }
                "equivalence" => {
                    let ChildOutcome::Complete {
                        equivalence_digest, ..
                    } = &mut changed.outcome
                    else {
                        unreachable!()
                    };
                    *equivalence_digest = "4".repeat(64);
                }
                _ => unreachable!(),
            }
            assert!(
                verify_matching_evidence(field, &expected, &changed).is_err(),
                "{field} drift was accepted"
            );
        }
    }

    #[test]
    fn a_child_that_timed_the_wrong_number_of_windows_is_rejected() {
        let spec = ChildSpec {
            task: ChildTask::Measure { execution: 0 },
            ..CHILD_SPEC
        };
        let protocol = child_protocol(2);
        let report = child_report(spec, &protocol);
        assert!(
            verify_child_report(spec, &report.operand_digest, &child_protocol(3), &report).is_err()
        );
        assert!(verify_child_report(spec, &report.operand_digest, &protocol, &report).is_ok());
    }

    #[test]
    fn a_child_with_wrong_protocol_or_installed_identity_is_rejected() {
        let spec = ChildSpec {
            task: ChildTask::Measure { execution: 0 },
            ..CHILD_SPEC
        };
        let protocol = child_protocol(2);
        let report = child_report(spec, &protocol);

        let mut wrong_protocol = report.clone();
        wrong_protocol.protocol.harness_schema = "tuning-calibration-v1".to_owned();
        assert!(
            verify_child_report(spec, &report.operand_digest, &protocol, &wrong_protocol).is_err()
        );

        let mut wrong_install = report.clone();
        wrong_install.installed.active_values[0].value += 1;
        assert!(
            verify_child_report(spec, &report.operand_digest, &protocol, &wrong_install).is_err()
        );

        let mut wrong_digest = report.clone();
        wrong_digest.installed.section_sha256 = "a".repeat(64);
        assert!(
            verify_child_report(spec, &report.operand_digest, &protocol, &wrong_digest).is_err()
        );

        let mut wrong_seeds = report.clone();
        wrong_seeds.seed_inventory.streams[0].seed ^= 1;
        assert!(
            verify_child_report(spec, &report.operand_digest, &protocol, &wrong_seeds).is_err()
        );

        let mut wrong_shape = report.clone();
        wrong_shape.fixture_shape.push_str(" stale");
        assert!(
            verify_child_report(spec, &report.operand_digest, &protocol, &wrong_shape).is_err()
        );

        let probe_spec = ChildSpec {
            task: ChildTask::Probe,
            ..spec
        };
        let probe = child_report(probe_spec, &protocol);
        let mut wrong_effective = probe.clone();
        let ChildOutcome::Complete {
            effective_observation,
            ..
        } = &mut wrong_effective.outcome
        else {
            unreachable!()
        };
        *effective_observation = Some("stale".to_owned());
        assert!(verify_child_report(
            probe_spec,
            &probe.operand_digest,
            &protocol,
            &wrong_effective
        )
        .is_err());

        let mut wrong_capability = probe.clone();
        let ChildOutcome::Complete {
            capability_observation,
            ..
        } = &mut wrong_capability.outcome
        else {
            unreachable!()
        };
        *capability_observation = Some("stale".to_owned());
        assert!(verify_child_report(
            probe_spec,
            &probe.operand_digest,
            &protocol,
            &wrong_capability
        )
        .is_err());
    }

    #[test]
    fn duplicate_or_invalid_raw_child_samples_are_rejected() {
        let spec = ChildSpec {
            task: ChildTask::Measure { execution: 0 },
            ..CHILD_SPEC
        };
        let protocol = child_protocol(2);
        let report = child_report(spec, &protocol);

        let mut duplicate = report.clone();
        let ChildOutcome::Complete { samples, .. } = &mut duplicate.outcome else {
            unreachable!()
        };
        samples[1].repetition = 0;
        assert!(verify_child_report(spec, &report.operand_digest, &protocol, &duplicate).is_err());

        let mut zero_calls = report.clone();
        let ChildOutcome::Complete { samples, .. } = &mut zero_calls.outcome else {
            unreachable!()
        };
        samples[0].calls = 0;
        assert!(verify_child_report(spec, &report.operand_digest, &protocol, &zero_calls).is_err());

        let mut wrong_execution = report.clone();
        let ChildOutcome::Complete { samples, .. } = &mut wrong_execution.outcome else {
            unreachable!()
        };
        samples[0].execution = 1;
        assert!(
            verify_child_report(spec, &report.operand_digest, &protocol, &wrong_execution).is_err()
        );
    }

    #[test]
    fn verified_child_observation_is_one_canonical_v4_line() {
        let protocol = child_protocol(2);
        let report = child_report(CHILD_SPEC, &protocol);
        verify_child_report(CHILD_SPEC, &report.operand_digest, &protocol, &report).unwrap();
        let line = child_observation_line(CHILD_SPEC, &report).unwrap();
        assert!(line.starts_with(CHILD_OBSERVATION_PREFIX));
        assert!(line.contains("\"requested_route\":\"karatsuba\""));
        assert!(line.contains("\"observed_route\":\"karatsuba\""));
        assert!(line.contains("\"resolution\":\"installed\""));
        assert!(line.contains("\"profile_format_version\":2"));
        assert!(line.contains("\"harness_schema\":\"tuning-calibration-v4\""));
    }

    #[test]
    fn retained_equivalence_digest_binds_the_operand_result_tuple() {
        let protocol = child_protocol(2);
        let mut report = child_report(CHILD_SPEC, &protocol);
        if let ChildOutcome::Complete {
            equivalence_digest, ..
        } = &mut report.outcome
        {
            *equivalence_digest = "f".repeat(64);
        }
        assert!(
            verify_child_report(CHILD_SPEC, &report.operand_digest, &protocol, &report).is_err()
        );
    }

    #[test]
    fn a_probe_child_times_nothing() {
        let spec = ChildSpec {
            task: ChildTask::Probe,
            ..CHILD_SPEC
        };
        let protocol = child_protocol(5);
        let report = child_report(spec, &protocol);
        assert!(verify_child_report(spec, &report.operand_digest, &protocol, &report).is_ok());
        let measured_spec = ChildSpec {
            task: ChildTask::Measure { execution: 0 },
            ..spec
        };
        let measured = child_report(measured_spec, &protocol);
        assert!(verify_child_report(spec, &measured.operand_digest, &protocol, &measured).is_err());
    }

    #[test]
    fn the_karatsuba_arm_forces_the_grid_point_and_the_schoolbook_arm_the_range_top() {
        let inherited = &CoreTuning::CONSERVATIVE;
        for size in CalibratedField::KaratsubaMinDegree.grid() {
            assert_eq!(
                forced_karatsuba_min_degree(Arm::Asymptotic, size),
                size,
                "the Karatsuba arm times the recursion a threshold of {size} produces"
            );
            assert_eq!(
                forced_karatsuba_min_degree(Arm::Conservative, size),
                FORCED_SCHOOLBOOK_MIN_DEGREE
            );
            // Both forced values are ordinary admissible values, so forcing an
            // arm reserves no sentinel and installs no profile the loader would
            // reject.
            for arm in Arm::BOTH {
                let conservative = inherited.polynomial();
                assert!(
                    PolynomialSelectors::try_new(
                        forced_karatsuba_min_degree(arm, size),
                        conservative.karatsuba_max_out_len(),
                        conservative.div_rem_fast_min_len(),
                        conservative.subproduct_min_len(),
                        conservative.interpolate_fast_min_points(),
                    )
                    .is_ok(),
                    "{arm} at {size} forces an inadmissible threshold"
                );
            }
        }
    }

    #[test]
    fn the_forced_thresholds_route_every_grid_point_to_their_own_arm() {
        // `mul_route_resolved` is private, so the rule is exercised through the
        // same comparison the dispatcher applies: schoolbook strictly below the
        // threshold, Karatsuba at or above it.
        for size in CalibratedField::KaratsubaMinDegree.grid() {
            assert!(
                size >= forced_karatsuba_min_degree(Arm::Asymptotic, size),
                "{size} does not reach the threshold forced for the Karatsuba arm"
            );
            assert!(
                size < forced_karatsuba_min_degree(Arm::Conservative, size),
                "{size} reaches the threshold forced for the schoolbook arm"
            );
        }
    }

    #[test]
    fn the_karatsuba_arm_hands_its_sub_operands_to_the_schoolbook_base_case() {
        // A split at degree `size` produces sub-operands of about half that
        // degree, which fall below the same threshold and take the schoolbook
        // arm. That is the recursion shape a chosen threshold produces, and it
        // is what distinguishes this forcing from one at the bottom of the
        // range, where the recursion would reach degree 0.
        for size in CalibratedField::KaratsubaMinDegree.grid() {
            let forced = forced_karatsuba_min_degree(Arm::Asymptotic, size);
            assert!(
                size.div_ceil(2) < forced,
                "a split at {size} recurses again"
            );
        }
    }

    // libtest is only an entry adapter: the child still decodes the canonical
    // stdin case and uses the producer's installation, fixtures and preflight.
    #[test]
    fn fresh_process_entry() {
        if env::var(FRESH_CASE_VAR).is_ok() {
            run_fresh_child().unwrap();
        }
    }

    #[test]
    fn fresh_child_entries_have_closed_command_protocols() {
        assert_eq!(
            FreshChildEntry::BenchmarkCli.arguments(),
            ["--fresh-tuning-process-child"]
        );
        assert_eq!(
            FreshChildEntry::LibtestAdapter.arguments(),
            [
                "--exact",
                "tuning_calibration::tests::fresh_process_entry",
                "--nocapture",
            ]
        );
    }

    #[allow(dead_code)] // The harness=false bench does not execute libtest helpers.
    fn real_probe_pair(field: CalibratedField) {
        let protocol = child_protocol(1);
        let executable = env::current_exe().unwrap();
        let size = field.grid()[0];
        let operands = expected_operand_digest(field, size).unwrap();
        for &variant in field.variants() {
            let mut reports = Vec::new();
            for arm in Arm::BOTH {
                let spec = ChildSpec {
                    field,
                    variant,
                    size,
                    arm,
                    task: ChildTask::Probe,
                };
                let report = launch_verified_child_with_entry(
                    spec,
                    &operands,
                    &protocol,
                    &executable,
                    FreshChildEntry::LibtestAdapter,
                )
                .unwrap_or_else(|error| panic!("{spec}: {error}"));
                if let ChildOutcome::Unavailable { omission, .. } = &report.outcome {
                    // The validator checks the closed omission against this
                    // field, arm and observed capability. Never call it a route hit.
                    eprintln!("UNAVAILABLE {spec}: {omission:?}; accelerated route untested");
                }
                if field == CalibratedField::M4rmWideTierMinStrideWords {
                    // Canonical minimum-grid schedule witness: 64x512 by
                    // 512x128 under the predeclared conservative table budgets.
                    let ChildOutcome::Complete {
                        effective_observation,
                        ..
                    } = &report.outcome
                    else {
                        panic!("M4RM wide has no optional capability");
                    };
                    assert_eq!(
                        effective_observation.as_deref(),
                        Some(match arm {
                            Arm::Conservative => "panel_width=6",
                            Arm::Asymptotic => "panel_width=9",
                        })
                    );
                }
                reports.push(report);
            }
            if reports
                .iter()
                .all(|report| matches!(report.outcome, ChildOutcome::Complete { .. }))
            {
                verify_matching_evidence(
                    &format!("real {field}/{variant}"),
                    &reports[0],
                    &reports[1],
                )
                .unwrap();
            }
        }
    }

    macro_rules! fresh_pairs {
        ($($name:ident: $field:ident),+ $(,)?) => { $(
            #[test]
            fn $name() { real_probe_pair(CalibratedField::$field); }
        )+ };
    }

    fresh_pairs! {
        fresh_pair_simd: SimdMinWords,
        fresh_pair_karatsuba_degree: KaratsubaMinDegree,
        fresh_pair_karatsuba_output: KaratsubaMaxOutLen,
        fresh_pair_division: DivRemFastMinLen,
        fresh_pair_evaluation: SubproductMinLen,
        fresh_pair_transpose: TransposeSimpleMaxBlocks,
        fresh_pair_soa: SoaParallelMinLen,
        fresh_pair_m4rm_wide: M4rmWideTierMinStrideWords,
        fresh_pair_m4rm_tiled: M4rmTiledMinStrideWords,
        fresh_pair_bit_inverse: DenseInverseM4riMinDim,
        fresh_pair_field_inverse: DenseInverseBlockedMinDim,
        fresh_pair_trsm: TrsmBlockedMinDim,
        fresh_pair_ple_panel: PlePanelBaseMaxCols,
        fresh_pair_back_sub: PleBlockedBackSubMinDim,
        fresh_pair_gemm: GemmAxpyFastPathMinVolume,
        fresh_pair_interpolation_variants: InterpolateFastMinPoints,
    }

    #[test]
    fn real_measure_child_includes_preflight_before_brief_windows() {
        let protocol = Protocol {
            executions: 1,
            repetitions: 1,
            target_ms: 1,
        };
        let field = CalibratedField::TransposeSimpleMaxBlocks;
        let spec = ChildSpec {
            field,
            variant: SweepVariant::Standard,
            size: field.grid()[0],
            arm: Arm::Asymptotic,
            task: ChildTask::Probe,
        };
        let operands = expected_operand_digest(field, spec.size).unwrap();
        let executable = env::current_exe().unwrap();
        let probe = launch_verified_child_with_entry(
            spec,
            &operands,
            &protocol,
            &executable,
            FreshChildEntry::LibtestAdapter,
        )
        .unwrap();
        let measure = launch_verified_child_with_entry(
            ChildSpec {
                task: ChildTask::Measure { execution: 0 },
                ..spec
            },
            &operands,
            &protocol,
            &executable,
            FreshChildEntry::LibtestAdapter,
        )
        .unwrap();
        verify_matching_evidence("real probe/measure", &probe, &measure).unwrap();
        let ChildOutcome::Complete {
            effective_observation,
            capability_observation,
            samples,
            ..
        } = measure.outcome
        else {
            panic!("transpose requires no optional capability");
        };
        assert!(effective_observation.is_some());
        assert!(capability_observation.is_some());
        assert_eq!(samples.len(), 1);
        assert!(samples[0].calls > 0 && samples[0].elapsed_ns > 0);
    }

    #[test]
    fn the_parent_derives_a_stable_grid_specific_operand_digest() {
        let field = CalibratedField::KaratsubaMinDegree;
        assert_eq!(
            expected_operand_digest(field, 16),
            expected_operand_digest(field, 16)
        );
        assert_ne!(
            expected_operand_digest(field, 16),
            expected_operand_digest(field, 8)
        );
    }

    #[test]
    fn cargo_bench_passes_a_bench_flag_the_parser_ignores() {
        let args = ["--bench", "--list-grid"].map(str::to_owned);
        assert_eq!(parse_args(args.into_iter()).unwrap().mode, Mode::ListGrid);
    }

    #[test]
    fn a_profile_id_comes_from_the_emitted_file_name() {
        let id = profile_id_for(Path::new("/tmp/gf2-calibration-fraktaali.json"), None).unwrap();
        assert_eq!(id.as_str(), "gf2-calibration-fraktaali");
    }

    #[test]
    fn a_non_kebab_case_file_name_is_rejected_rather_than_mangled() {
        assert!(profile_id_for(Path::new("/tmp/Gf2_Calibration.json"), None).is_err());
        let explicit = profile_id_for(Path::new("/tmp/Gf2_Calibration.json"), Some("host-avx2"));
        assert_eq!(explicit.unwrap().as_str(), "host-avx2");
    }

    #[test]
    fn a_parent_segment_is_collapsed_out_of_the_harness_path() {
        assert_eq!(
            normalize_relative(Path::new("crates/gf2-core/tests/../benches/x.rs")),
            "crates/gf2-core/benches/x.rs"
        );
        assert!(RepoRelPath::parse(&normalize_relative(Path::new(
            "crates/gf2-core/tests/../benches/x.rs"
        )))
        .is_ok());
    }

    #[test]
    fn timestamps_are_formatted_in_the_form_the_loader_accepts() {
        let epoch = rfc3339_utc(UNIX_EPOCH).unwrap();
        assert_eq!(epoch, "1970-01-01T00:00:00Z");
        assert!(Rfc3339Utc::parse(&epoch).is_ok());
        let leap_day = rfc3339_utc(UNIX_EPOCH + Duration::from_secs(1_772_323_200)).unwrap();
        assert_eq!(leap_day, "2026-03-01T00:00:00Z");
        assert!(Rfc3339Utc::parse(&rfc3339_utc(SystemTime::now()).unwrap()).is_ok());
    }

    #[test]
    fn the_device_encoding_matches_the_kernel_lock_table() {
        assert_eq!(makedev(0x00, 0x2d), 45);
        assert_eq!(makedev(0x08, 0x02), 2050);
    }

    #[test]
    fn a_product_length_maps_back_to_equal_operand_lengths() {
        for out_len in CalibratedField::KaratsubaMaxOutLen.grid() {
            let len = operand_len_for_product(out_len);
            assert_eq!(2 * len - 1, out_len);
        }
    }
}

/// Crate-owned extent cases and artifact decisions for the neutral campaign driver.
mod campaign_owner {
    use super::*;
    use gf2_core::field::matrix::{GemmTilePair, GemmTileSite};
    use gf2_core::field::vec::{DotChunkCandidate, FieldVec};
    use gf2_core::gf2m::{Gf2mElement, Gf2mField};
    use serde::{Deserialize, Serialize};
    use tuning_campaign_support::{seed, statistics, transport};

    const OWNER_PROTOCOL: &str = "core-tuning-campaign-v4";
    const EXTENT_SEEDS: &str = "fixture-seeds-v3";
    const RAW_WINDOWS: &str = "raw-timing-samples-v3";
    const CORE_CELLS: usize = 702;
    const CORE_FIELDS: usize = 27;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ExtentField {
        Transpose,
        Soa,
        M4rmDefaultBytes,
        M4rmMidBytes,
        M4rmWideBytes,
        M4rmWideCap,
        M4rmSmallCap,
        Trsm,
        GemmTiles,
        Dot,
        M4rmJoint,
    }

    impl ExtentField {
        const ALL: [Self; 10] = [
            Self::Transpose,
            Self::Soa,
            Self::M4rmDefaultBytes,
            Self::M4rmMidBytes,
            Self::M4rmWideBytes,
            Self::M4rmWideCap,
            Self::M4rmSmallCap,
            Self::Trsm,
            Self::GemmTiles,
            Self::Dot,
        ];

        fn tag(self) -> u64 {
            match self {
                Self::Transpose => 16,
                Self::Soa => 17,
                Self::M4rmDefaultBytes => 18,
                Self::M4rmMidBytes => 19,
                Self::M4rmWideBytes => 20,
                Self::M4rmWideCap => 21,
                Self::M4rmSmallCap => 22,
                Self::Trsm => 23,
                Self::GemmTiles => 24,
                Self::Dot => 26,
                Self::M4rmJoint => 18,
            }
        }

        fn paths(self) -> &'static [&'static str] {
            match self {
                Self::Transpose => &["bit_matrix.transpose_macro_tile_blocks"],
                Self::Soa => &["soa_batch.parallel_chunk_len"],
                Self::M4rmDefaultBytes => &["m4rm.default_table_bytes"],
                Self::M4rmMidBytes => &["m4rm.mid_table_bytes"],
                Self::M4rmWideBytes => &["m4rm.wide_table_bytes"],
                Self::M4rmWideCap => &["m4rm.wide_max_k"],
                Self::M4rmSmallCap => &["m4rm.small_n_max_k"],
                Self::Trsm => &["triangular.trsm_panel_rows"],
                Self::GemmTiles => &["gemm.row_tile", "gemm.col_tile"],
                Self::Dot => &["field_vec.dot_chunk_len"],
                Self::M4rmJoint => &[
                    "m4rm.default_table_bytes",
                    "m4rm.mid_table_bytes",
                    "m4rm.wide_table_bytes",
                    "m4rm.wide_max_k",
                    "m4rm.small_n_max_k",
                ],
            }
        }

        fn candidates(self) -> Vec<ExtentCandidate> {
            let scalars: &[usize] = match self {
                Self::Transpose => &[2, 4, 8, 16, 32],
                Self::Soa => &[4096, 8192, 16384, 32768, 65536],
                Self::M4rmDefaultBytes => &[16384, 32768, 65536, 131072, 262144],
                Self::M4rmMidBytes => &[32768, 65536, 131072, 262144, 524288],
                Self::M4rmWideBytes => &[65536, 131072, 262144, 524288, 1048576],
                Self::M4rmWideCap | Self::M4rmSmallCap => &[4, 5, 6, 7, 8, 9, 10],
                Self::Trsm => &[8, 16, 32, 64, 128],
                Self::Dot => &[128, 256, 512],
                Self::GemmTiles => {
                    return GemmTilePair::ALL
                        .into_iter()
                        .map(|pair| {
                            let (row, col) = pair.extents();
                            ExtentCandidate::Tiles { row, col }
                        })
                        .collect()
                }
                Self::M4rmJoint => return vec![self.default_candidate()],
            };
            scalars
                .iter()
                .map(|&value| ExtentCandidate::Scalar { value })
                .collect()
        }

        fn default_candidate(self) -> ExtentCandidate {
            if self == Self::GemmTiles {
                let p = CoreTuning::CONSERVATIVE.gemm();
                return ExtentCandidate::Tiles {
                    row: p.row_tile(),
                    col: p.col_tile(),
                };
            }
            if self == Self::M4rmJoint {
                return ExtentCandidate::Vector {
                    proposed: false,
                    values: M4rmVector::conservative(),
                };
            }
            let body =
                complete_selector_value(&CoreTuning::CONSERVATIVE).expect("conservative codec");
            let pointer = format!("/{}", self.paths()[0].replace('.', "/"));
            ExtentCandidate::Scalar {
                value: body
                    .pointer(&pointer)
                    .and_then(serde_json::Value::as_u64)
                    .expect("extent codec leaf") as usize,
            }
        }

        fn shape_count(self) -> usize {
            match self {
                Self::M4rmWideCap => 9,
                Self::M4rmJoint => 12,
                _ => 3,
            }
        }

        fn is_m4rm(self) -> bool {
            matches!(
                self,
                Self::M4rmDefaultBytes
                    | Self::M4rmMidBytes
                    | Self::M4rmWideBytes
                    | Self::M4rmWideCap
                    | Self::M4rmSmallCap
                    | Self::M4rmJoint
            )
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
    enum ExtentCandidate {
        Scalar { value: usize },
        Tiles { row: usize, col: usize },
        Vector { proposed: bool, values: M4rmVector },
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct M4rmVector {
        default_table_bytes: usize,
        mid_table_bytes: usize,
        wide_table_bytes: usize,
        wide_max_k: usize,
        small_n_max_k: usize,
    }
    impl M4rmVector {
        fn conservative() -> Self {
            let p = CoreTuning::CONSERVATIVE.m4rm();
            Self {
                default_table_bytes: p.default_table_bytes(),
                mid_table_bytes: p.mid_table_bytes(),
                wide_table_bytes: p.wide_table_bytes(),
                wide_max_k: p.wide_max_k(),
                small_n_max_k: p.small_n_max_k(),
            }
        }
        fn values(&self) -> [usize; 5] {
            [
                self.default_table_bytes,
                self.mid_table_bytes,
                self.wide_table_bytes,
                self.wide_max_k,
                self.small_n_max_k,
            ]
        }
        fn validate(&self) -> Result<(), String> {
            for (field, value) in [
                ExtentField::M4rmDefaultBytes,
                ExtentField::M4rmMidBytes,
                ExtentField::M4rmWideBytes,
                ExtentField::M4rmWideCap,
                ExtentField::M4rmSmallCap,
            ]
            .into_iter()
            .zip(self.values())
            {
                if !field
                    .candidates()
                    .contains(&ExtentCandidate::Scalar { value })
                {
                    return Err("joint vector leaves the declared one-factor grids".to_owned());
                }
            }
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ExtentSite {
        MatrixGemm,
        MatrixGemmIntoView,
        MatrixGemmAxpyIntoView,
        MatrixGemmAxpyIntoViewDiag,
        ExprGemmWithBeta,
        ExprGemmTransA,
        ExprGemmTransAWithBeta,
    }
    impl ExtentSite {
        fn production(self) -> GemmTileSite {
            match self {
                Self::MatrixGemm => GemmTileSite::MatrixGemm,
                Self::MatrixGemmIntoView => GemmTileSite::MatrixGemmIntoView,
                Self::MatrixGemmAxpyIntoView => GemmTileSite::MatrixGemmAxpyIntoView,
                Self::MatrixGemmAxpyIntoViewDiag => GemmTileSite::MatrixGemmAxpyIntoViewDiag,
                Self::ExprGemmWithBeta => GemmTileSite::ExprGemmWithBeta,
                Self::ExprGemmTransA => GemmTileSite::ExprGemmTransA,
                Self::ExprGemmTransAWithBeta => GemmTileSite::ExprGemmTransAWithBeta,
            }
        }
        fn all() -> Vec<Self> {
            GemmTileSite::ALL
                .into_iter()
                .map(|site| match site {
                    GemmTileSite::MatrixGemm => Self::MatrixGemm,
                    GemmTileSite::MatrixGemmIntoView => Self::MatrixGemmIntoView,
                    GemmTileSite::MatrixGemmAxpyIntoView => Self::MatrixGemmAxpyIntoView,
                    GemmTileSite::MatrixGemmAxpyIntoViewDiag => Self::MatrixGemmAxpyIntoViewDiag,
                    GemmTileSite::ExprGemmWithBeta => Self::ExprGemmWithBeta,
                    GemmTileSite::ExprGemmTransA => Self::ExprGemmTransA,
                    GemmTileSite::ExprGemmTransAWithBeta => Self::ExprGemmTransAWithBeta,
                })
                .collect()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExtentCell {
        field: ExtentField,
        shape_index: usize,
        site: Option<ExtentSite>,
        candidate: ExtentCandidate,
    }
    impl ExtentCell {
        fn validate(&self) -> Result<(), String> {
            if self.shape_index >= self.field.shape_count() {
                return Err("unknown extent shape".to_owned());
            }
            if (self.field == ExtentField::GemmTiles) != self.site.is_some() {
                return Err("extent site does not match field".to_owned());
            }
            if self.field == ExtentField::M4rmJoint {
                match &self.candidate {
                    ExtentCandidate::Vector { proposed, values } => {
                        values.validate()?;
                        if !proposed && *values != M4rmVector::conservative() {
                            return Err("joint control must be conservative".to_owned());
                        }
                    }
                    _ => return Err("joint field requires a vector".to_owned()),
                }
            } else if !self.field.candidates().contains(&self.candidate) {
                return Err("unknown extent candidate".to_owned());
            }
            Ok(())
        }

        fn dimensions(&self) -> (usize, usize, usize) {
            if self.field.is_m4rm() {
                let strides = [16, 24, 31, 32, 48, 63, 64, 96, 128];
                return match self.field {
                    ExtentField::M4rmDefaultBytes => (64, 512, 64 * strides[self.shape_index]),
                    ExtentField::M4rmMidBytes => (64, 512, 64 * strides[self.shape_index + 3]),
                    ExtentField::M4rmWideBytes => (64, 512, 64 * strides[self.shape_index + 6]),
                    ExtentField::M4rmSmallCap => (64, 2048, [512, 768, 960][self.shape_index]),
                    ExtentField::M4rmJoint if self.shape_index >= 9 => {
                        (64, 2048, [512, 768, 960][self.shape_index - 9])
                    }
                    _ => (64, 512, 64 * strides[self.shape_index]),
                };
            }
            let n = match self.field {
                ExtentField::Transpose => [32, 64, 128][self.shape_index] * 64,
                ExtentField::Soa => [65536, 131072, 262144][self.shape_index],
                ExtentField::Trsm => [129, 193, 257][self.shape_index],
                ExtentField::Dot => [4097, 16385, 65537][self.shape_index],
                ExtentField::GemmTiles => {
                    return [(65, 64, 129), (129, 128, 257), (193, 192, 385)][self.shape_index]
                }
                _ => unreachable!(),
            };
            (n, n, n)
        }
        fn seed_identity(&self) -> (u64, u64) {
            if self.field.is_m4rm() {
                let (_, k, n) = self.dimensions();
                (
                    18,
                    if k == 512 {
                        (n / 64) as u64
                    } else {
                        0x10000 + n as u64
                    },
                )
            } else {
                (self.field.tag(), self.shape_index as u64)
            }
        }
        fn roles(&self) -> &'static [(&'static str, u64)] {
            match self.field {
                ExtentField::Transpose => &[("matrix", 0x100)],
                ExtentField::Soa => &[
                    ("quadratic_lhs", 0x200),
                    ("quadratic_rhs", 0x201),
                    ("cubic_lhs", 0x202),
                    ("cubic_rhs", 0x203),
                ],
                ExtentField::Trsm => {
                    &[("unit_lower", 0x600), ("unit_upper", 0x601), ("rhs", 0x602)]
                }
                ExtentField::GemmTiles => &[("lhs", 0x900), ("rhs", 0x901), ("addend", 0x902)],
                ExtentField::Dot => &[("lhs", 0xb00), ("rhs", 0xb01)],
                _ => &[("lhs", 0x300), ("rhs", 0x301)],
            }
        }
        fn seed(&self, role: u64, bank: usize) -> u64 {
            let (tag, key) = self.seed_identity();
            seed::fixture_seed(SEED_ROOT, tag, key, seed::bank_role(role, bank))
        }
        fn seed_inventory(&self) -> ExtentSeeds {
            let (fixture_tag, shape_key) = self.seed_identity();
            ExtentSeeds {
                schema: EXTENT_SEEDS.to_owned(),
                derivation: seed::SEED_DERIVATION.to_owned(),
                seed_root: SEED_ROOT,
                fixture_tag,
                shape_key,
                streams: (0..BIT_FIXTURES)
                    .flat_map(|bank| {
                        self.roles().iter().map(move |(name, role)| SeedStream {
                            name: format!("{name}[{bank}]"),
                            role: seed::bank_role(*role, bank),
                            seed: self.seed(*role, bank),
                        })
                    })
                    .collect(),
            }
        }
        fn scalar(&self) -> usize {
            match self.candidate {
                ExtentCandidate::Scalar { value } => value,
                _ => unreachable!("validated scalar extent"),
            }
        }
        fn pair(&self) -> GemmTilePair {
            let ExtentCandidate::Tiles { row, col } = self.candidate else {
                unreachable!("validated GEMM pair")
            };
            GemmTilePair::ALL
                .into_iter()
                .find(|pair| pair.extents() == (row, col))
                .expect("validated pair")
        }
        fn section(&self) -> Result<CoreTuning, String> {
            self.validate()?;
            let mut body = complete_selector_value(&CoreTuning::CONSERVATIVE)?;
            let values = match &self.candidate {
                ExtentCandidate::Scalar { value } => vec![*value],
                ExtentCandidate::Tiles { row, col } => vec![*row, *col],
                ExtentCandidate::Vector { values, .. } => values.values().to_vec(),
            };
            for (path, value) in self.field.paths().iter().zip(values) {
                set_leaf(&mut body, path, value)?;
            }
            match self.field {
                ExtentField::Transpose => {
                    set_leaf(&mut body, "bit_matrix.transpose_simple_max_blocks", 0)?
                }
                ExtentField::Soa => set_leaf(&mut body, "soa_batch.parallel_min_len", 0)?,
                ExtentField::Trsm => set_leaf(&mut body, "triangular.trsm_blocked_min_dim", 0)?,
                ExtentField::GemmTiles => {
                    set_leaf(&mut body, "gemm.axpy_fast_path_min_volume", usize::MAX)?
                }
                _ => {}
            }
            if self.field.is_m4rm() {
                set_leaf(&mut body, "m4rm.wide_tier_min_stride_words", 16)?;
                set_leaf(&mut body, "m4rm.tiled_min_stride_words", usize::MAX)?;
            }
            CoreTuningCodec::decode_body(
                CanonicalValue::serialize(&body).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExtentSeeds {
        schema: String,
        derivation: String,
        seed_root: u64,
        fixture_tag: u64,
        shape_key: u64,
        streams: Vec<SeedStream>,
    }

    fn set_leaf(body: &mut serde_json::Value, path: &str, value: usize) -> Result<(), String> {
        let pointer = format!("/{}", path.replace('.', "/"));
        let leaf = body
            .pointer_mut(&pointer)
            .ok_or_else(|| format!("unknown codec leaf {path}"))?;
        *leaf = serde_json::json!(value);
        Ok(())
    }

    pub(super) fn flatten_selectors(body: &serde_json::Value) -> Result<Vec<ForcedValue>, String> {
        let families = body.as_object().ok_or("selectors must be an object")?;
        let mut values = Vec::new();
        for (family, fields) in families {
            for (field, value) in fields
                .as_object()
                .ok_or("selector family must be an object")?
            {
                values.push(ForcedValue::new(
                    family,
                    field,
                    usize::try_from(
                        value
                            .as_u64()
                            .ok_or("selector must be a nonnegative integer")?,
                    )
                    .map_err(|_| "selector exceeds usize")?,
                ));
            }
        }
        Ok(values)
    }

    fn measured_inventory() -> Result<Vec<String>, String> {
        let all = flatten_selectors(&complete_selector_value(&CoreTuning::CONSERVATIVE)?)?;
        let mut measured: Vec<_> = CalibratedField::ALL
            .into_iter()
            .map(|field| field.schema_field().to_string())
            .chain(
                ExtentField::ALL
                    .into_iter()
                    .flat_map(|field| field.paths().iter().map(|path| (*path).to_owned())),
            )
            .collect();
        measured.sort();
        measured.dedup();
        if measured.len() != CORE_FIELDS
            || all.len() != EXPECTED_CORE_SCHEMA_FIELDS
            || measured.iter().any(|path| {
                !all.iter()
                    .any(|leaf| format!("{}.{}", leaf.family, leaf.field) == *path)
            })
        {
            return Err("extent/retained coverage differs from owner codec".to_owned());
        }
        Ok(measured)
    }

    fn extent_cells() -> Vec<ExtentCell> {
        let mut cells = Vec::new();
        for field in ExtentField::ALL {
            for shape_index in 0..field.shape_count() {
                let sites = if field == ExtentField::GemmTiles {
                    ExtentSite::all().into_iter().map(Some).collect()
                } else {
                    vec![None]
                };
                for site in sites {
                    for candidate in field.candidates() {
                        cells.push(ExtentCell {
                            field,
                            shape_index,
                            site,
                            candidate,
                        });
                    }
                }
            }
        }
        cells
    }

    struct TrsmBank {
        a: FieldMatrix<Fp251>,
        lower: FieldMatrix<Fp251>,
        upper: FieldMatrix<Fp251>,
        rhs: FieldMatrix<Fp251>,
    }
    struct GemmBank {
        lhs: FieldMatrix<F>,
        rhs: FieldMatrix<F>,
        addend: FieldMatrix<F>,
        transposed_lhs: FieldMatrix<F>,
    }
    enum ExtentFixture {
        Transpose(Vec<BitMatrix>),
        Soa {
            quadratic_lhs: Vec<BatchExtField<F, 2>>,
            quadratic_rhs: Vec<BatchExtField<F, 2>>,
            cubic_lhs: Vec<BatchExtField<F, 3>>,
            cubic_rhs: Vec<BatchExtField<F, 3>>,
        },
        M4rm(Vec<(BitMatrix, BitMatrix)>),
        Trsm(Vec<TrsmBank>),
        Gemm(Vec<GemmBank>),
        Dot(Vec<(FieldVec<Gf2mElement>, FieldVec<Gf2mElement>)>),
    }

    fn fp_matrix<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut rng = Lcg::new(seed);
        let mut out = FieldMatrix::zeros(rows, cols);
        for row in 0..rows {
            for col in 0..cols {
                out.set(row, col, Fp::new(rng.next_u64() % P));
            }
        }
        out
    }
    fn scalar_fp_transpose<const P: u64>(a: &FieldMatrix<Fp<P>>) -> FieldMatrix<Fp<P>> {
        let mut out = FieldMatrix::zeros(a.cols(), a.rows());
        for row in 0..a.rows() {
            for col in 0..a.cols() {
                out.set(col, row, a.get(row, col));
            }
        }
        out
    }
    fn digest_fp<const P: u64>(matrix: &FieldMatrix<Fp<P>>) -> String {
        let mut h = Sha256Hasher::new();
        h.update(b"gf2-extent-prime-matrix-v1");
        h.update(P.to_le_bytes());
        h.update(matrix.rows().to_le_bytes());
        h.update(matrix.cols().to_le_bytes());
        for row in 0..matrix.rows() {
            for col in 0..matrix.cols() {
                h.update(matrix.get(row, col).value().to_le_bytes());
            }
        }
        format!("{:x}", h.finalize())
    }
    fn digest_dot(vector: &FieldVec<Gf2mElement>) -> String {
        let mut h = Sha256Hasher::new();
        h.update(b"gf2-extent-runtime-gf256-vector-v1");
        h.update(vector.len().to_le_bytes());
        for element in vector.iter() {
            h.update(element.value().to_le_bytes());
        }
        format!("{:x}", h.finalize())
    }
    fn digest_dot_result(value: Gf2mElement) -> String {
        let mut h = Sha256Hasher::new();
        h.update(b"gf2-extent-runtime-gf256-element-v1");
        h.update(value.value().to_le_bytes());
        format!("{:x}", h.finalize())
    }

    fn build_extent_fixture(cell: &ExtentCell) -> Result<ExtentFixture, String> {
        cell.validate()?;
        let (m, k, n) = cell.dimensions();
        let banks = 0..BIT_FIXTURES;
        Ok(match cell.field {
            ExtentField::Transpose => ExtentFixture::Transpose(
                banks
                    .map(|bank| bit_matrix_from_words(m, n, cell.seed(0x100, bank)))
                    .collect(),
            ),
            ExtentField::Soa => ExtentFixture::Soa {
                quadratic_lhs: banks
                    .clone()
                    .map(|bank| soa_batch(m, cell.seed(0x200, bank)))
                    .collect(),
                quadratic_rhs: banks
                    .clone()
                    .map(|bank| soa_batch(m, cell.seed(0x201, bank)))
                    .collect(),
                cubic_lhs: banks
                    .clone()
                    .map(|bank| soa_batch(m, cell.seed(0x202, bank)))
                    .collect(),
                cubic_rhs: banks
                    .map(|bank| soa_batch(m, cell.seed(0x203, bank)))
                    .collect(),
            },
            ExtentField::Trsm => ExtentFixture::Trsm(
                banks
                    .map(|bank| {
                        let (lower, upper) =
                            field_lu_parts(m, cell.seed(0x600, bank), cell.seed(0x601, bank));
                        TrsmBank {
                            a: scalar_field_matmul(&lower, &upper),
                            lower,
                            upper,
                            rhs: filled_field_matrix(m, n, cell.seed(0x602, bank)),
                        }
                    })
                    .collect(),
            ),
            ExtentField::GemmTiles => ExtentFixture::Gemm(
                banks
                    .map(|bank| {
                        let lhs = fp_matrix(m, k, cell.seed(0x900, bank));
                        GemmBank {
                            transposed_lhs: scalar_fp_transpose(&lhs),
                            lhs,
                            rhs: fp_matrix(k, n, cell.seed(0x901, bank)),
                            addend: fp_matrix(m, n, cell.seed(0x902, bank)),
                        }
                    })
                    .collect(),
            ),
            ExtentField::Dot => {
                let field = Gf2mField::gf256();
                ExtentFixture::Dot(
                    banks
                        .map(|bank| {
                            let mut lhs = Lcg::new(cell.seed(0xb00, bank));
                            let mut rhs = Lcg::new(cell.seed(0xb01, bank));
                            (
                                (0..m)
                                    .map(|_| field.element(lhs.next_u64() & 255))
                                    .collect(),
                                (0..m)
                                    .map(|_| field.element(rhs.next_u64() & 255))
                                    .collect(),
                            )
                        })
                        .collect(),
                )
            }
            _ => ExtentFixture::M4rm(
                banks
                    .map(|bank| {
                        (
                            bit_matrix_from_words(m, k, cell.seed(0x300, bank)),
                            bit_matrix_from_words(k, n, cell.seed(0x301, bank)),
                        )
                    })
                    .collect(),
            ),
        })
    }

    fn extent_operand_digest(fixture: &ExtentFixture) -> String {
        let mut parts = Vec::new();
        for bank in 0..BIT_FIXTURES {
            parts.extend(match fixture {
                ExtentFixture::Transpose(banks) => vec![digest_bit_matrix(&banks[bank])],
                ExtentFixture::Soa {
                    quadratic_lhs,
                    quadratic_rhs,
                    cubic_lhs,
                    cubic_rhs,
                } => vec![
                    digest_batch(&quadratic_lhs[bank]),
                    digest_batch(&quadratic_rhs[bank]),
                    digest_batch(&cubic_lhs[bank]),
                    digest_batch(&cubic_rhs[bank]),
                ],
                ExtentFixture::M4rm(banks) => vec![
                    digest_bit_matrix(&banks[bank].0),
                    digest_bit_matrix(&banks[bank].1),
                ],
                ExtentFixture::Trsm(banks) => {
                    vec![digest_fp(&banks[bank].a), digest_fp(&banks[bank].rhs)]
                }
                ExtentFixture::Gemm(banks) => vec![
                    digest_fp(&banks[bank].lhs),
                    digest_fp(&banks[bank].rhs),
                    digest_fp(&banks[bank].addend),
                ],
                ExtentFixture::Dot(banks) => {
                    vec![digest_dot(&banks[bank].0), digest_dot(&banks[bank].1)]
                }
            });
        }
        digest_tuple(b"gf2-extent-operands-bank-role-tuple-v1", parts)
    }

    pub(super) fn scalar_soa_product<const N: usize>(
        lhs: &BatchExtField<F, N>,
        rhs: &BatchExtField<F, N>,
    ) -> BatchExtField<F, N> {
        let mut coefficients: [Vec<F>; N] = std::array::from_fn(|_| Vec::with_capacity(lhs.len()));
        for element in 0..lhs.len() {
            let mut reduced = [F::new(0); N];
            for left in 0..N {
                for right in 0..N {
                    let product = lhs.coeff(left)[element] * rhs.coeff(right)[element];
                    let degree = left + right;
                    reduced[degree % N] += if degree >= N {
                        F::new(3) * product
                    } else {
                        product
                    };
                }
            }
            for lane in 0..N {
                coefficients[lane].push(reduced[lane]);
            }
        }
        BatchExtField::new(coefficients)
    }

    fn scalar_trsm(bank: &TrsmBank, rhs: &FieldMatrix<Fp251>) -> FieldMatrix<Fp251> {
        let n = bank.a.rows();
        let mut out = rhs.clone();
        for row in 0..n {
            for col in 0..rhs.cols() {
                let mut x = out.get(row, col);
                for inner in 0..row {
                    x = x - bank.lower.get(row, inner) * out.get(inner, col);
                }
                out.set(row, col, x);
            }
        }
        for row in (0..n).rev() {
            for col in 0..rhs.cols() {
                let mut x = out.get(row, col);
                for inner in row + 1..n {
                    x = x - bank.upper.get(row, inner) * out.get(inner, col);
                }
                out.set(row, col, x);
            }
        }
        out
    }

    fn gemm_oracle(site: ExtentSite, left: &GemmBank, right: &GemmBank) -> FieldMatrix<F> {
        let mut a = left.lhs.clone();
        let mut b = right.rhs.clone();
        if site == ExtentSite::MatrixGemmAxpyIntoViewDiag {
            for i in 0..a.rows().min(a.cols()) {
                a.set(i, i, F::new(1));
            }
            for i in 0..b.rows().min(b.cols()) {
                b.set(i, i, F::new(1));
            }
        }
        let product = scalar_field_matmul(&a, &b);
        let (alpha, beta) = match site {
            ExtentSite::MatrixGemmAxpyIntoView
            | ExtentSite::MatrixGemmAxpyIntoViewDiag
            | ExtentSite::ExprGemmTransAWithBeta => (3, 5),
            ExtentSite::ExprGemmWithBeta => (1, 5),
            _ => return product,
        };
        let mut output = product.clone();
        for row in 0..output.rows() {
            for col in 0..output.cols() {
                output.set(
                    row,
                    col,
                    F::new(alpha) * product.get(row, col)
                        + F::new(beta) * left.addend.get(row, col),
                );
            }
        }
        output
    }

    /// A const specialization is resolved once before any timing, including site selection.
    enum BoundGemm {
        Ordinary(gf2_core::field::matrix::GemmCandidateFn<F>),
        Into(gf2_core::field::matrix::GemmIntoViewCandidateFn<F>),
        Axpy(gf2_core::field::matrix::GemmAxpyCandidateFn<F>),
        Diag(gf2_core::field::matrix::GemmDiagCandidateFn<F>),
        Beta(gf2_core::field::expr::ExprGemmBetaCandidateFn<F>),
        Trans(gf2_core::field::expr::ExprGemmTransCandidateFn<F>),
        TransBeta(gf2_core::field::expr::ExprGemmTransBetaCandidateFn<F>),
    }
    impl BoundGemm {
        fn new(cell: &ExtentCell) -> Self {
            let pair = cell.pair();
            match cell.site.expect("validated GEMM site") {
                ExtentSite::MatrixGemm => Self::Ordinary(pair.gemm_fn()),
                ExtentSite::MatrixGemmIntoView => Self::Into(pair.gemm_into_view_fn()),
                ExtentSite::MatrixGemmAxpyIntoView => Self::Axpy(pair.gemm_axpy_fn()),
                ExtentSite::MatrixGemmAxpyIntoViewDiag => Self::Diag(pair.gemm_diag_fn()),
                ExtentSite::ExprGemmWithBeta => Self::Beta(pair.expr_gemm_beta_fn()),
                ExtentSite::ExprGemmTransA => Self::Trans(pair.expr_gemm_trans_fn()),
                ExtentSite::ExprGemmTransAWithBeta => {
                    Self::TransBeta(pair.expr_gemm_trans_beta_fn())
                }
            }
        }
        fn call(&self, left: &GemmBank, right: &GemmBank) -> FieldMatrix<F> {
            let (m, n) = (left.lhs.rows(), right.rhs.cols());
            match self {
                Self::Ordinary(f) => f(&left.lhs, &right.rhs),
                Self::Into(f) => {
                    let mut out = FieldMatrix::zeros(m, n);
                    f(&left.lhs, &right.rhs, &mut out);
                    out
                }
                Self::Axpy(f) => {
                    let mut out = left.addend.clone();
                    f(F::new(3), &left.lhs, &right.rhs, F::new(5), &mut out);
                    out
                }
                Self::Diag(f) => {
                    let mut out = left.addend.clone();
                    f(F::new(3), &left.lhs, &right.rhs, F::new(5), &mut out);
                    out
                }
                Self::Beta(f) => {
                    let mut out = FieldMatrix::zeros(m, n);
                    f(&left.lhs, &right.rhs, F::new(5), &left.addend, &mut out);
                    out
                }
                Self::Trans(f) => {
                    let mut out = FieldMatrix::zeros(m, n);
                    f(&left.transposed_lhs, &right.rhs, &mut out);
                    out
                }
                Self::TransBeta(f) => {
                    let mut out = FieldMatrix::zeros(m, n);
                    f(
                        F::new(3),
                        &left.transposed_lhs,
                        &right.rhs,
                        F::new(5),
                        &left.addend,
                        &mut out,
                    );
                    out
                }
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "route", rename_all = "snake_case", deny_unknown_fields)]
    enum EffectiveSchedule {
        MacroTiled {
            blocks: usize,
        },
        SoaParallel {
            chunk: usize,
        },
        M4rm {
            tier: M4rmTier,
            panel_width: usize,
            c_update: RowWise,
        },
        TrsmBlocked {
            panel_rows: usize,
        },
        GemmTiles {
            row: usize,
            col: usize,
            site: ExtentSite,
        },
        DotClmulBarrett {
            chunk: usize,
        },
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum M4rmTier {
        SmallN,
        Wide,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum RowWise {
        RowWise,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum BudgetBand {
        SmallN,
        Default,
        Mid,
        Wide,
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct M4rmConsumed {
        band: BudgetBand,
        table_bytes: Option<usize>,
        panel_width_cap: usize,
        tiled_stride_admitted: bool,
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExtentObservation {
        schedule: EffectiveSchedule,
        m4rm_consumed: Option<M4rmConsumed>,
        dedicated_pool_width: Option<usize>,
        fp251_whole_gemm: Option<bool>,
        quiet_timing: bool,
    }

    pub(super) fn reset_solve_observations() {
        gf2_core::field::matrix::reset_gemm_tile_observations();
        reset_last_gemm_axpy_dispatch_route();
        gf2_core::field::triangular::reset_last_effective_triangular_route();
        reset_last_effective_trsm_panel_rows();
        reset_max_effective_panel_dispatch_cols();
    }
    pub(super) fn quiet_solve_observations_empty() -> bool {
        gf2_core::field::matrix::gemm_tile_observations().is_empty()
            && last_gemm_axpy_dispatch_route().is_none()
            && gf2_core::field::triangular::last_effective_triangular_route().is_none()
            && last_effective_trsm_panel_rows().is_none()
            && max_effective_panel_dispatch_cols().is_none()
    }
    fn extent_oracle_digest(cell: &ExtentCell, fixture: &ExtentFixture) -> String {
        let mut outputs = Vec::new();
        for bank in 0..BIT_FIXTURES {
            let rhs = paired_bank(bank);
            let output = match fixture {
                ExtentFixture::Transpose(banks) => {
                    let a = &banks[bank];
                    let mut result = BitMatrix::zeros(a.cols(), a.rows());
                    for row in 0..a.rows() {
                        for col in 0..a.cols() {
                            result.set(col, row, a.get(row, col));
                        }
                    }
                    digest_bit_matrix(&result)
                }
                ExtentFixture::Soa {
                    quadratic_lhs,
                    quadratic_rhs,
                    cubic_lhs,
                    cubic_rhs,
                } => digest_tuple(
                    b"gf2-extent-soa-four-result-v1",
                    [
                        digest_batch(&scalar_soa_product(
                            &quadratic_lhs[bank],
                            &quadratic_rhs[rhs],
                        )),
                        digest_batch(&scalar_soa_product(
                            &quadratic_lhs[bank],
                            &quadratic_lhs[bank],
                        )),
                        digest_batch(&scalar_soa_product(&cubic_lhs[bank], &cubic_rhs[rhs])),
                        digest_batch(&scalar_soa_product(&cubic_lhs[bank], &cubic_lhs[bank])),
                    ],
                ),
                ExtentFixture::M4rm(banks) => {
                    digest_bit_matrix(&scalar_m4rm_oracle(&banks[bank].0, &banks[rhs].1))
                }
                ExtentFixture::Trsm(banks) => {
                    digest_fp(&scalar_trsm(&banks[bank], &banks[rhs].rhs))
                }
                ExtentFixture::Gemm(banks) => {
                    digest_fp(&gemm_oracle(cell.site.unwrap(), &banks[bank], &banks[rhs]))
                }
                ExtentFixture::Dot(banks) => {
                    digest_dot_result(banks[bank].0.dot_product(&banks[rhs].1))
                }
            };
            outputs.push(output);
        }
        digest_tuple(b"gf2-extent-eight-bank-results-v1", outputs)
    }

    fn probe_extent(
        cell: &ExtentCell,
        fixture: &ExtentFixture,
        bound_gemm: Option<&BoundGemm>,
        bound_dot: Option<gf2_core::field::vec::DotChunkCandidateFn>,
    ) -> Result<(String, Vec<ExtentObservation>), String> {
        use gf2_core::alg::m4rm::{M4rmEffectiveScheduleObservation, M4rmTableBand};
        use gf2_core::matrix::{
            reset_transpose_effective_observation, transpose_effective_observation,
            TransposeEffectiveObservation,
        };
        let mut outputs = Vec::new();
        let mut observations = Vec::new();
        for bank in 0..BIT_FIXTURES {
            let rhs = paired_bank(bank);
            let mut consumed = None;
            let mut pool = None;
            let mut fp251 = None;
            let (output, schedule) = match fixture {
                ExtentFixture::Transpose(banks) => {
                    reset_transpose_effective_observation();
                    let result = banks[bank].transpose();
                    if transpose_effective_observation()
                        != (TransposeEffectiveObservation::MacroTiled {
                            macro_tile_blocks: cell.scalar(),
                        })
                    {
                        return Err("transpose failed consumed macro-tile observation".to_owned());
                    }
                    if result.transpose() != banks[bank] {
                        return Err("transpose failed involution".to_owned());
                    }
                    (
                        digest_bit_matrix(&result),
                        EffectiveSchedule::MacroTiled {
                            blocks: cell.scalar(),
                        },
                    )
                }
                ExtentFixture::Soa {
                    quadratic_lhs,
                    quadratic_rhs,
                    cubic_lhs,
                    cubic_rhs,
                } => {
                    let width = rayon::current_num_threads();
                    if width != 4
                        || soa_parallel_route(cell.dimensions().0) != SoaParallelRoute::Parallel
                    {
                        return Err("SoA required dedicated parallel route unavailable".to_owned());
                    }
                    pool = Some(width);
                    reset_last_effective_soa_chunk();
                    let a = quadratic_lhs[bank]
                        .batch_mul_quadratic::<QuadraticBeta3>(&quadratic_rhs[rhs]);
                    let first = last_effective_soa_chunk();
                    reset_last_effective_soa_chunk();
                    let b = quadratic_lhs[bank].batch_square_quadratic::<QuadraticBeta3>();
                    let second = last_effective_soa_chunk();
                    reset_last_effective_soa_chunk();
                    let c = cubic_lhs[bank].batch_mul_cubic::<CubicBeta3>(&cubic_rhs[rhs]);
                    let third = last_effective_soa_chunk();
                    reset_last_effective_soa_chunk();
                    let d = cubic_lhs[bank].batch_square_cubic::<CubicBeta3>();
                    let fourth = last_effective_soa_chunk();
                    if [first, second, third, fourth] != [Some(cell.scalar()); 4] {
                        return Err(
                            "SoA did not consume the candidate at all four operations".to_owned()
                        );
                    }
                    (
                        digest_tuple(
                            b"gf2-extent-soa-four-result-v1",
                            [
                                digest_batch(&a),
                                digest_batch(&b),
                                digest_batch(&c),
                                digest_batch(&d),
                            ],
                        ),
                        EffectiveSchedule::SoaParallel {
                            chunk: cell.scalar(),
                        },
                    )
                }
                ExtentFixture::M4rm(banks) => {
                    let (_, k, n) = cell.dimensions();
                    let reported = m4rm_schedule_route(k, n);
                    gf2_core::alg::m4rm::reset_m4rm_effective_schedule_observation();
                    let result = m4rm_multiply(&banks[bank].0, &banks[rhs].1);
                    let M4rmEffectiveScheduleObservation::Completed(executed) =
                        gf2_core::alg::m4rm::m4rm_effective_schedule_observation()
                    else {
                        return Err("M4RM missing completed schedule".to_owned());
                    };
                    if executed.tier() != reported.tier()
                        || executed.panel_width() != reported.panel_width()
                        || executed.panel_width() < 2
                        || executed.c_update() != M4rmTiledEffectiveObservation::RowWise
                        || executed.tiled_stride_admitted()
                    {
                        return Err(
                            "M4RM executed a different schedule or bypassed RowWise panels"
                                .to_owned(),
                        );
                    }
                    let tier = match executed.tier() {
                        M4rmScheduleTier::SmallN => M4rmTier::SmallN,
                        M4rmScheduleTier::Wide => M4rmTier::Wide,
                    };
                    let band = match executed.table_band() {
                        M4rmTableBand::SmallN => BudgetBand::SmallN,
                        M4rmTableBand::Default => BudgetBand::Default,
                        M4rmTableBand::Mid => BudgetBand::Mid,
                        M4rmTableBand::Wide => BudgetBand::Wide,
                    };
                    consumed = Some(M4rmConsumed {
                        band,
                        table_bytes: executed.table_bytes(),
                        panel_width_cap: executed.panel_width_cap(),
                        tiled_stride_admitted: executed.tiled_stride_admitted(),
                    });
                    (
                        digest_bit_matrix(&result),
                        EffectiveSchedule::M4rm {
                            tier,
                            panel_width: executed.panel_width(),
                            c_update: RowWise::RowWise,
                        },
                    )
                }
                ExtentFixture::Trsm(banks) => {
                    let capable = <Fp251 as FiniteField>::has_simd_gemm_classical();
                    fp251 = Some(capable);
                    if !capable || trsm_route(cell.dimensions().0) != TrsmRoute::Blocked {
                        return Err(
                            "TRSM required blocked whole-GEMM capability unavailable".to_owned()
                        );
                    }
                    reset_solve_observations();
                    let result = banks[bank]
                        .a
                        .solve_batch(&banks[rhs].rhs)
                        .ok_or("TRSM fixture became singular")?;
                    if last_effective_trsm_panel_rows() != Some(cell.scalar()) {
                        return Err("TRSM did not consume the candidate panel".to_owned());
                    }
                    if scalar_field_matmul(&banks[bank].a, &result) != banks[rhs].rhs {
                        return Err("TRSM failed A*X=B".to_owned());
                    }
                    reset_solve_observations();
                    let quiet = banks[bank]
                        .a
                        .solve_batch_quiet_for_test(&banks[rhs].rhs)
                        .ok_or("quiet TRSM became singular")?;
                    if quiet != result || !quiet_solve_observations_empty() {
                        return Err(
                            "quiet TRSM differs in semantics or publishes observations".to_owned()
                        );
                    }
                    (
                        digest_fp(&result),
                        EffectiveSchedule::TrsmBlocked {
                            panel_rows: cell.scalar(),
                        },
                    )
                }
                ExtentFixture::Gemm(banks) => {
                    gf2_core::field::matrix::reset_gemm_tile_observations();
                    let result = bound_gemm
                        .expect("bound GEMM")
                        .call(&banks[bank], &banks[rhs]);
                    let observations = gf2_core::field::matrix::gemm_tile_observations();
                    let (row, col) = cell.pair().extents();
                    let site = cell.site.unwrap();
                    let expected = gf2_core::field::matrix::GemmTileObservation {
                        site: site.production(),
                        row_tile: row,
                        col_tile: col,
                    };
                    if observations != [expected] {
                        return Err(format!(
                            "GEMM missing exact candidate/site evidence: {observations:?}"
                        ));
                    }
                    (
                        digest_fp(&result),
                        EffectiveSchedule::GemmTiles { row, col, site },
                    )
                }
                ExtentFixture::Dot(banks) => {
                    gf2_core::field::vec::reset_max_effective_dot_chunk_len();
                    let result = bound_dot.expect("bound dot")(&banks[bank].0, &banks[rhs].1);
                    let effective = gf2_core::field::vec::max_effective_dot_chunk_len();
                    if effective != cell.scalar() {
                        return Err(
                            "dot did not execute required batched CLMUL/Barrett candidate"
                                .to_owned(),
                        );
                    }
                    (
                        digest_dot_result(result),
                        EffectiveSchedule::DotClmulBarrett { chunk: effective },
                    )
                }
            };
            let observation = ExtentObservation {
                schedule,
                m4rm_consumed: consumed,
                dedicated_pool_width: pool,
                fp251_whole_gemm: fp251,
                quiet_timing: cell.field == ExtentField::Trsm,
            };
            validate_extent_observation(cell, &observation)?;
            outputs.push(output);
            observations.push(observation);
        }
        Ok((
            digest_tuple(b"gf2-extent-eight-bank-results-v1", outputs),
            observations,
        ))
    }

    fn expected_m4rm_schedule(
        cell: &ExtentCell,
    ) -> Result<(EffectiveSchedule, M4rmConsumed), String> {
        use gf2_core::alg::m4rm::{m4rm_schedule_route_for_selectors, M4rmTableBand};
        let (_, k, n) = cell.dimensions();
        let section = cell.section()?;
        let route = m4rm_schedule_route_for_selectors(section.m4rm(), k, n);
        if route.panel_width() < 2 || route.tiled_stride_admitted() {
            return Err("M4RM declared controls do not select RowWise Gray-code panels".to_owned());
        }
        Ok((
            EffectiveSchedule::M4rm {
                tier: match route.tier() {
                    M4rmScheduleTier::SmallN => M4rmTier::SmallN,
                    M4rmScheduleTier::Wide => M4rmTier::Wide,
                },
                panel_width: route.panel_width(),
                c_update: RowWise::RowWise,
            },
            M4rmConsumed {
                band: match route.table_band() {
                    M4rmTableBand::SmallN => BudgetBand::SmallN,
                    M4rmTableBand::Default => BudgetBand::Default,
                    M4rmTableBand::Mid => BudgetBand::Mid,
                    M4rmTableBand::Wide => BudgetBand::Wide,
                },
                table_bytes: route.table_bytes(),
                panel_width_cap: route.panel_width_cap(),
                tiled_stride_admitted: route.tiled_stride_admitted(),
            },
        ))
    }

    fn validate_extent_observation(
        cell: &ExtentCell,
        observation: &ExtentObservation,
    ) -> Result<(), String> {
        let expected = match cell.field {
            ExtentField::Transpose => EffectiveSchedule::MacroTiled {
                blocks: cell.scalar(),
            },
            ExtentField::Soa => EffectiveSchedule::SoaParallel {
                chunk: cell.scalar(),
            },
            ExtentField::Trsm => EffectiveSchedule::TrsmBlocked {
                panel_rows: cell.scalar(),
            },
            ExtentField::GemmTiles => {
                let (row, col) = cell.pair().extents();
                EffectiveSchedule::GemmTiles {
                    row,
                    col,
                    site: cell.site.unwrap(),
                }
            }
            ExtentField::Dot => EffectiveSchedule::DotClmulBarrett {
                chunk: cell.scalar(),
            },
            _ => {
                let (schedule, consumed) = expected_m4rm_schedule(cell)?;
                if observation.m4rm_consumed.as_ref() != Some(&consumed) {
                    return Err("M4RM consumed selector context mismatch".to_owned());
                }
                schedule
            }
        };
        if observation.schedule != expected
            || observation.dedicated_pool_width
                != if cell.field == ExtentField::Soa {
                    Some(4)
                } else {
                    None
                }
            || observation.fp251_whole_gemm
                != if cell.field == ExtentField::Trsm {
                    Some(true)
                } else {
                    None
                }
            || observation.quiet_timing != (cell.field == ExtentField::Trsm)
            || observation.m4rm_consumed.is_some() != cell.field.is_m4rm()
        {
            return Err("extent capability/route context mismatch".to_owned());
        }
        Ok(())
    }

    fn time_extent(
        fixture: &ExtentFixture,
        bound_gemm: Option<&BoundGemm>,
        bound_dot: Option<gf2_core::field::vec::DotChunkCandidateFn>,
        bank: usize,
    ) {
        let bank = bank & (BIT_FIXTURES - 1);
        let rhs = paired_bank(bank);
        match fixture {
            ExtentFixture::Transpose(banks) => {
                black_box(banks[bank].transpose());
            }
            ExtentFixture::Soa {
                quadratic_lhs,
                quadratic_rhs,
                cubic_lhs,
                cubic_rhs,
            } => {
                black_box(
                    quadratic_lhs[bank].batch_mul_quadratic::<QuadraticBeta3>(&quadratic_rhs[rhs]),
                );
                black_box(quadratic_lhs[bank].batch_square_quadratic::<QuadraticBeta3>());
                black_box(cubic_lhs[bank].batch_mul_cubic::<CubicBeta3>(&cubic_rhs[rhs]));
                black_box(cubic_lhs[bank].batch_square_cubic::<CubicBeta3>());
            }
            ExtentFixture::M4rm(banks) => {
                black_box(m4rm_multiply(&banks[bank].0, &banks[rhs].1));
            }
            ExtentFixture::Trsm(banks) => {
                black_box(
                    banks[bank]
                        .a
                        .solve_batch_quiet_for_test(&banks[rhs].rhs)
                        .expect("verified invertible fixture"),
                );
            }
            ExtentFixture::Gemm(banks) => {
                black_box(
                    bound_gemm
                        .expect("bound GEMM")
                        .call(&banks[bank], &banks[rhs]),
                );
            }
            ExtentFixture::Dot(banks) => {
                black_box(bound_dot.expect("bound dot")(&banks[bank].0, &banks[rhs].1));
            }
        }
    }

    use neutral::{
        CanonicalJson, LaunchUnit, OwnerManifest, OwnerOperation, OwnerResponse, Sha256Digest,
        Task, Token, UnitIdentity,
    };
    use tuning_campaign_support::campaign as neutral;

    pub(super) fn fixed_protocol() -> Protocol {
        Protocol {
            executions: DEFAULT_EXECUTIONS,
            repetitions: DEFAULT_REPETITIONS,
            target_ms: DEFAULT_TARGET_MS,
        }
    }
    fn err(e: impl fmt::Display) -> String {
        e.to_string()
    }
    fn token(value: impl Into<String>) -> Result<Token, String> {
        Token::new(value).map_err(err)
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "experiment", rename_all = "snake_case", deny_unknown_fields)]
    enum OwnerCaseKind {
        Threshold { spec: ChildSpec },
        Extent { cell: ExtentCell },
        ReservedM4rm { cell: ExtentCell },
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "inventory", rename_all = "snake_case", deny_unknown_fields)]
    enum CaseSeeds {
        Retained { seeds: SeedInventory },
        Extent { seeds: ExtentSeeds },
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OwnerCase {
        schema: String,
        protocol: Protocol,
        identity: UnitIdentity,
        protocol_sha256: Sha256Digest,
        channels: neutral::SessionChannels,
        kind: OwnerCaseKind,
        forced_values: Vec<ForcedValue>,
        seeds: CaseSeeds,
    }

    fn section_prepared(section: CoreTuning) -> Result<PreparedEnvelope, String> {
        let id = ProfileId::parse(FORCED_ARM_PROFILE_ID).map_err(err)?;
        PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
            .insert(section)
            .map_err(err)?
            .build()
            .map_err(err)
    }
    pub(super) fn section_values(prepared: &PreparedEnvelope) -> Result<Vec<ForcedValue>, String> {
        let section = prepared
            .section::<CoreTuning>()
            .map_err(err)?
            .ok_or("missing core section")?;
        flatten_selectors(&complete_selector_value(section.section)?)
    }
    fn task_old(task: Task) -> ChildTask {
        match task {
            Task::Probe => ChildTask::Probe,
            Task::Measure { execution } => ChildTask::Measure { execution },
        }
    }
    fn task_neutral(task: ChildTask) -> Task {
        match task {
            ChildTask::Probe => Task::Probe,
            ChildTask::Measure { execution } => Task::Measure { execution },
        }
    }
    fn case_coordinates(
        kind: &OwnerCaseKind,
    ) -> Result<(String, String, String, String, Task), String> {
        Ok(match kind {
            OwnerCaseKind::Threshold { spec } => {
                if !spec.field.grid().contains(&spec.size)
                    || !spec.field.variants().contains(&spec.variant)
                {
                    return Err("unknown retained threshold shape/variant".to_owned());
                }
                (
                    "retained-thresholds".to_owned(),
                    spec.field.schema_field().to_string(),
                    format!("size-{}-{}", spec.size, spec.variant),
                    spec.arm.to_string(),
                    task_neutral(spec.task),
                )
            }
            OwnerCaseKind::Extent { cell } | OwnerCaseKind::ReservedM4rm { cell } => {
                cell.validate()?;
                let phase = if cell.field == ExtentField::M4rmJoint {
                    "m4rm-joint"
                } else {
                    "core-extents"
                };
                let field = if cell.field == ExtentField::GemmTiles {
                    "gemm.tiles"
                } else if cell.field == ExtentField::M4rmJoint {
                    "m4rm.joint"
                } else {
                    cell.field.paths()[0]
                };
                let stratum = match cell.site {
                    Some(site) => format!("shape-{}-{site:?}", cell.shape_index),
                    None => format!("shape-{}", cell.shape_index),
                };
                let candidate = match &cell.candidate {
                    ExtentCandidate::Scalar { value } => format!("candidate-{value}"),
                    ExtentCandidate::Tiles { row, col } => format!("row-{row}-col-{col}"),
                    ExtentCandidate::Vector { proposed, .. } => {
                        if *proposed {
                            "proposed".to_owned()
                        } else {
                            "conservative".to_owned()
                        }
                    }
                };
                (
                    phase.to_owned(),
                    field.to_owned(),
                    stratum,
                    candidate,
                    Task::Probe,
                )
            }
        })
    }
    impl OwnerCase {
        fn new(
            request: &neutral::ManifestRequest,
            kind: OwnerCaseKind,
            task: Task,
        ) -> Result<Self, String> {
            task.validate().map_err(err)?;
            let (phase, field, stratum, candidate, old_task) = case_coordinates(&kind)?;
            if matches!(kind, OwnerCaseKind::Threshold { .. }) && old_task != task {
                return Err("retained task mismatch".to_owned());
            }
            let (forced_values, seeds) = match &kind {
                OwnerCaseKind::Threshold { spec } => {
                    let (prepared, _) = forced_profile_for(*spec)?;
                    (
                        section_values(&prepared)?,
                        CaseSeeds::Retained {
                            seeds: seed_inventory(spec.field, spec.size),
                        },
                    )
                }
                OwnerCaseKind::Extent { cell } | OwnerCaseKind::ReservedM4rm { cell } => (
                    flatten_selectors(&complete_selector_value(&cell.section()?)?)?,
                    CaseSeeds::Extent {
                        seeds: cell.seed_inventory(),
                    },
                ),
            };
            Ok(Self {
                schema: OWNER_PROTOCOL.to_owned(),
                protocol: fixed_protocol(),
                identity: UnitIdentity {
                    protocol: token(OWNER_PROTOCOL)?,
                    owner: token("gf2-core")?,
                    campaign_id: request.campaign_id.clone(),
                    phase: token(phase)?,
                    field: token(field)?,
                    stratum: token(stratum)?,
                    candidate: token(candidate)?,
                    task,
                },
                protocol_sha256: request.protocol_sha256.clone(),
                channels: request.channels.clone(),
                kind,
                forced_values,
                seeds,
            })
        }
        fn validate(&self, allow_reserved: bool) -> Result<(), String> {
            self.channels.validate().map_err(err)?;
            if !allow_reserved && matches!(self.kind, OwnerCaseKind::ReservedM4rm { .. }) {
                return Err("reserved M4RM case has not been derived".to_owned());
            }
            let request = neutral::ManifestRequest {
                campaign_id: self.identity.campaign_id.clone(),
                protocol_sha256: self.protocol_sha256.clone(),
                channels: self.channels.clone(),
                processes: Vec::new(),
            };
            if Self::new(&request, self.kind.clone(), self.identity.task)? != *self {
                return Err("owner case identity/protocol/controls/seeds mismatch".to_owned());
            }
            Ok(())
        }
    }

    fn ordered_cases(request: &neutral::ManifestRequest) -> Result<Vec<OwnerCase>, String> {
        let mut cases = Vec::new();
        // Retained order is field, variant, size, two probes, then paired executions.
        for field in CalibratedField::ALL {
            for &variant in field.variants() {
                for size in field.grid() {
                    for task in std::iter::once(Task::Probe)
                        .chain((0..DEFAULT_EXECUTIONS).map(|execution| Task::Measure { execution }))
                    {
                        for arm in Arm::BOTH {
                            let spec = ChildSpec {
                                field,
                                variant,
                                size,
                                arm,
                                task: task_old(task),
                            };
                            cases.push(OwnerCase::new(
                                request,
                                OwnerCaseKind::Threshold { spec },
                                task,
                            )?);
                        }
                    }
                }
            }
        }
        for field in ExtentField::ALL {
            for shape_index in 0..field.shape_count() {
                let sites = if field == ExtentField::GemmTiles {
                    ExtentSite::all().into_iter().map(Some).collect()
                } else {
                    vec![None]
                };
                for site in sites {
                    for task in std::iter::once(Task::Probe)
                        .chain((0..DEFAULT_EXECUTIONS).map(|execution| Task::Measure { execution }))
                    {
                        let mut candidates = field.candidates();
                        if let Task::Measure { execution } = task {
                            let count = candidates.len();
                            candidates.rotate_left(execution as usize % count);
                            if execution % 2 == 1 {
                                candidates.reverse();
                            }
                        }
                        for candidate in candidates {
                            let cell = ExtentCell {
                                field,
                                shape_index,
                                site,
                                candidate,
                            };
                            cases.push(OwnerCase::new(
                                request,
                                OwnerCaseKind::Extent { cell },
                                task,
                            )?);
                        }
                    }
                }
            }
        }
        for shape_index in 0..12 {
            for task in std::iter::once(Task::Probe)
                .chain((0..DEFAULT_EXECUTIONS).map(|execution| Task::Measure { execution }))
            {
                let mut labels = vec![false, true];
                if let Task::Measure { execution } = task {
                    labels.rotate_left(execution as usize % 2);
                    if execution % 2 == 1 {
                        labels.reverse();
                    }
                }
                for proposed in labels {
                    let cell = ExtentCell {
                        field: ExtentField::M4rmJoint,
                        shape_index,
                        site: None,
                        candidate: ExtentCandidate::Vector {
                            proposed,
                            values: M4rmVector::conservative(),
                        },
                    };
                    cases.push(OwnerCase::new(
                        request,
                        OwnerCaseKind::ReservedM4rm { cell },
                        task,
                    )?);
                }
            }
        }
        if cases.len() != CORE_CELLS * 6 {
            return Err("core ordered work count mismatch".to_owned());
        }
        Ok(cases)
    }
    fn manifest(request: &neutral::ManifestRequest) -> Result<OwnerManifest, String> {
        request.channels.validate().map_err(err)?;
        preflight()?;
        let [process] = request.processes.as_slice() else {
            return Err(
                "core manifest requires exactly its own staged process descriptor".to_owned(),
            );
        };
        process.validate().map_err(err)?;
        if process.id.as_str() != "core-producer"
            || process.arguments != ["--fresh-tuning-process-child"]
        {
            return Err("core process descriptor must use the fixed fresh-child entry".to_owned());
        }
        let cases = ordered_cases(request)?;
        let units = cases
            .iter()
            .enumerate()
            .map(|(ordinal, case)| {
                LaunchUnit::new(
                    ordinal as u64,
                    case.identity.clone(),
                    process.id.clone(),
                    CanonicalJson::from_serializable(case).map_err(err)?,
                )
                .map_err(err)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut candidate_blocks: Vec<neutral::CandidateBlock> = Vec::new();
        for case in cases
            .iter()
            .filter(|case| case.identity.task == Task::Probe)
        {
            let id = &case.identity;
            if let Some(block) = candidate_blocks.last_mut().filter(|block| {
                block.phase == id.phase && block.field == id.field && block.stratum == id.stratum
            }) {
                block.base_candidates.push(id.candidate.clone());
            } else {
                candidate_blocks.push(neutral::CandidateBlock {
                    phase: id.phase.clone(),
                    field: id.field.clone(),
                    stratum: id.stratum.clone(),
                    base_candidates: vec![id.candidate.clone()],
                });
            }
        }
        let mut manifest = OwnerManifest {
            schema: neutral::MANIFEST_SCHEMA.to_owned(),
            owner: token("gf2-core")?,
            owner_protocol: token(OWNER_PROTOCOL)?,
            behavior_token: token(CoreTuningCodec::HARNESS_SCHEMA)?,
            campaign_id: request.campaign_id.clone(),
            phases: ["retained-thresholds", "core-extents", "m4rm-joint"]
                .into_iter()
                .map(token)
                .collect::<Result<_, _>>()?,
            counts: neutral::DeclaredCounts::for_cells(CORE_CELLS as u64).map_err(err)?,
            processes: request.processes.clone(),
            ordered_units: units,
            candidate_blocks,
            manifest_sha256: Sha256Digest::of(b""),
        };
        manifest.seal().map_err(err)?;
        Ok(manifest)
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OrdinaryCompanionConstants {
        gemm_row_tile: usize,
        gemm_col_tile: usize,
        dot_chunk_len: usize,
    }
    impl OrdinaryCompanionConstants {
        fn observe() -> Self {
            let (gemm_row_tile, gemm_col_tile) = gf2_core::field::matrix::selected_gemm_tiles();
            Self {
                gemm_row_tile,
                gemm_col_tile,
                dot_chunk_len: gf2_core::field::vec::selected_dot_chunk_len(),
            }
        }
        fn validate(&self) -> Result<(), String> {
            if *self == Self::observe() {
                Ok(())
            } else {
                Err("ordinary cfg-selected companion constants mismatch".to_owned())
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "experiment", rename_all = "snake_case", deny_unknown_fields)]
    enum OwnerEvidence {
        Threshold {
            schema: String,
            ordinary_companions: OrdinaryCompanionConstants,
            report: Box<ChildReport>,
            full_active_values: Vec<ForcedValue>,
        },
        Extent {
            schema: String,
            ordinary_companions: OrdinaryCompanionConstants,
            installed: Box<InstalledEvidence>,
            seeds: ExtentSeeds,
            dimensions: (usize, usize, usize),
            operands_sha256: Sha256Digest,
            oracle_sha256: Sha256Digest,
            result_sha256: Sha256Digest,
            observations: Vec<ExtentObservation>,
        },
    }

    fn run_owner_case(case: &OwnerCase) -> Result<neutral::ChildResult, String> {
        case.validate(false)?;
        let case_sha256 = CanonicalJson::from_serializable(case)
            .map_err(err)?
            .digest();
        if matches!(case.identity.task, Task::Measure { .. }) {
            if env::var(BENCH_MODE_VAR).as_deref() != Ok("1") {
                return Err("timed child requires prepared-host benchmark authorization".to_owned());
            }
            require_campaign_environment()?;
        }
        let (payload, samples, outcome) = match &case.kind {
            OwnerCaseKind::Threshold { spec } => {
                let context = (case.identity.clone(), case_sha256.clone());
                let report = run_child_with_context(*spec, &case.protocol, Some(&context))?;
                let full_active_values =
                    flatten_selectors(&complete_selector_value(tuning::active().section)?)?;
                if full_active_values != case.forced_values {
                    return Err("retained child failed full section readback".to_owned());
                }
                let (samples, outcome) = match &report.outcome {
                    ChildOutcome::Complete { samples, .. } => {
                        (samples.clone(), neutral::ChildOutcome::Complete)
                    }
                    ChildOutcome::Unavailable { .. } => {
                        (Vec::new(), neutral::ChildOutcome::Unavailable)
                    }
                };
                (
                    OwnerEvidence::Threshold {
                        schema: RAW_WINDOWS.to_owned(),
                        ordinary_companions: OrdinaryCompanionConstants::observe(),
                        report: Box::new(report),
                        full_active_values,
                    },
                    samples,
                    outcome,
                )
            }
            OwnerCaseKind::Extent { cell } => {
                let installed = install_prepared_profile(
                    section_prepared(cell.section()?)?,
                    case.forced_values.clone(),
                )?;
                let fixture = build_extent_fixture(cell)?;
                let operands_sha256 =
                    Sha256Digest::new(extent_operand_digest(&fixture)).map_err(err)?;
                let oracle_sha256 =
                    Sha256Digest::new(extent_oracle_digest(cell, &fixture)).map_err(err)?;
                let gemm = (cell.field == ExtentField::GemmTiles).then(|| BoundGemm::new(cell));
                let dot = (cell.field == ExtentField::Dot).then(|| {
                    DotChunkCandidate::ALL
                        .into_iter()
                        .find(|candidate| candidate.chunk_len() == cell.scalar())
                        .expect("validated dot candidate")
                        .function()
                });
                let finish = || -> Result<_, String> {
                    let (result, observations) = probe_extent(cell, &fixture, gemm.as_ref(), dot)?;
                    let result_sha256 = Sha256Digest::new(result).map_err(err)?;
                    if result_sha256 != oracle_sha256 {
                        return Err(
                            "extent outputs differ from the independent eight-bank scalar oracle"
                                .to_owned(),
                        );
                    }
                    let samples = if matches!(case.identity.task, Task::Measure { .. }) {
                        neutral::execution_with_progress(
                            &case.identity,
                            &case_sha256,
                            &mut |bank| time_extent(&fixture, gemm.as_ref(), dot, bank),
                            &mut io::stderr().lock(),
                        )
                        .map_err(err)?
                    } else {
                        Vec::new()
                    };
                    Ok((
                        OwnerEvidence::Extent {
                            schema: RAW_WINDOWS.to_owned(),
                            ordinary_companions: OrdinaryCompanionConstants::observe(),
                            installed: Box::new(installed),
                            seeds: cell.seed_inventory(),
                            dimensions: cell.dimensions(),
                            operands_sha256,
                            oracle_sha256,
                            result_sha256,
                            observations,
                        },
                        samples,
                        neutral::ChildOutcome::Complete,
                    ))
                };
                if cell.field == ExtentField::Soa {
                    run_in_dedicated_parallel_pool(4, finish)?
                } else {
                    finish()?
                }
            }
            OwnerCaseKind::ReservedM4rm { .. } => {
                return Err("reserved child is not executable".to_owned())
            }
        };
        Ok(neutral::ChildResult {
            schema: neutral::RESULT_SCHEMA.to_owned(),
            identity: case.identity.clone(),
            case_sha256,
            outcome,
            samples,
            payload: CanonicalJson::from_serializable(&payload).map_err(err)?,
        })
    }

    fn validate_result(
        unit: &LaunchUnit,
        result: &neutral::ChildResult,
        reconstruct: bool,
    ) -> Result<(), String> {
        unit.validate().map_err(err)?;
        let case: OwnerCase = unit.case.decode().map_err(err)?;
        case.validate(false)?;
        if unit.identity != case.identity
            || result.schema != neutral::RESULT_SCHEMA
            || result.identity != unit.identity
            || result.case_sha256 != unit.case.digest()
            || result.outcome != neutral::ChildOutcome::Complete
        {
            return Err("owner result identity/outcome mismatch".to_owned());
        }
        match case.identity.task {
            Task::Probe => {
                if !result.samples.is_empty() {
                    return Err("probe has timing samples".to_owned());
                }
            }
            Task::Measure { execution } => {
                if result.samples.len() != 5 {
                    return Err("timed result lacks five windows".to_owned());
                }
                for (repetition, sample) in result.samples.iter().enumerate() {
                    sample.validate().map_err(err)?;
                    if sample.execution != execution
                        || sample.repetition != repetition as u64
                        || sample.calls != result.samples[0].calls
                    {
                        return Err("timed result window ordering/call count mismatch".to_owned());
                    }
                }
            }
        }
        let evidence: OwnerEvidence = result.payload.decode().map_err(err)?;
        match (&case.kind, evidence) {
            (
                OwnerCaseKind::Threshold { spec },
                OwnerEvidence::Threshold {
                    schema,
                    ordinary_companions,
                    report,
                    full_active_values,
                },
            ) => {
                ordinary_companions.validate()?;
                if schema != RAW_WINDOWS || full_active_values != case.forced_values {
                    return Err("threshold result schema/full controls mismatch".to_owned());
                }
                let operands = if reconstruct {
                    expected_operand_digest(spec.field, spec.size)?
                } else {
                    report.operand_digest.clone()
                };
                verify_child_report(*spec, &operands, &case.protocol, &report)?;
                let ChildOutcome::Complete { samples, .. } = report.outcome else {
                    return Err("threshold result unavailable".to_owned());
                };
                if samples != result.samples {
                    return Err("threshold inner/outer windows mismatch".to_owned());
                }
            }
            (
                OwnerCaseKind::Extent { cell },
                OwnerEvidence::Extent {
                    schema,
                    ordinary_companions,
                    installed,
                    seeds,
                    dimensions,
                    operands_sha256,
                    oracle_sha256,
                    result_sha256,
                    observations,
                },
            ) => {
                ordinary_companions.validate()?;
                let prepared = section_prepared(cell.section()?)?;
                let digests = forced_profile_digests(&prepared)?;
                if schema != RAW_WINDOWS
                    || seeds != cell.seed_inventory()
                    || dimensions != cell.dimensions()
                    || installed.profile_id != FORCED_ARM_PROFILE_ID
                    || installed.section_id != CoreTuning::ID.as_str()
                    || installed.resolution != ObservedResolution::Installed
                    || installed.measurement != ObservedMeasurement::Inherited
                    || installed.active_values != case.forced_values
                    || installed.section_sha256 != digests.section_sha256
                    || installed.envelope_content_sha256 != digests.envelope_content_sha256
                    || result_sha256 != oracle_sha256
                    || observations.len() != 8
                {
                    return Err("extent installed/fixture/oracle evidence mismatch".to_owned());
                }
                for observation in &observations {
                    validate_extent_observation(cell, observation)?;
                }
                if observations.windows(2).any(|pair| pair[0] != pair[1]) {
                    return Err("extent schedule changed between fixture banks".to_owned());
                }
                if reconstruct {
                    let fixture = build_extent_fixture(cell)?;
                    if operands_sha256.as_str() != extent_operand_digest(&fixture)
                        || oracle_sha256.as_str() != extent_oracle_digest(cell, &fixture)
                    {
                        return Err("extent fixture or scalar oracle digest failed independent reconstruction".to_owned());
                    }
                }
            }
            _ => return Err("owner evidence does not match experiment".to_owned()),
        }
        Ok(())
    }

    pub(super) fn fresh_child() -> Result<(), String> {
        let case: OwnerCase = transport::read_guarded_case(
            env::var(FRESH_CASE_VAR).ok().as_deref(),
            io::stdin().lock(),
        )?;
        let result = run_owner_case(&case)?;
        transport::write_result_line(io::stdout().lock(), &result).map_err(err)
    }

    struct AnalysisInput<'a> {
        request: neutral::ManifestRequest,
        records: std::collections::BTreeMap<String, &'a neutral::AcceptedResult>,
    }
    impl<'a> AnalysisInput<'a> {
        fn new(
            bundle: &'a neutral::AcceptedResultsBundle,
            campaign_id: &Token,
            manifest_sha256: &Sha256Digest,
            complete: bool,
        ) -> Result<Self, String> {
            let expected_count = if complete {
                CORE_CELLS * 6
            } else {
                (CORE_CELLS - 24) * 6
            };
            if bundle.schema != neutral::ACCEPTED_RESULTS_SCHEMA
                || bundle.manifest_sha256 != *manifest_sha256
                || bundle.accepted.len() != expected_count
            {
                return Err("owner input manifest/accounting mismatch".to_owned());
            }
            let first = bundle.accepted.first().ok_or("empty owner input")?;
            let first_case: OwnerCase = first.unit.case.decode().map_err(err)?;
            if first_case.identity.campaign_id != *campaign_id {
                return Err("owner input campaign mismatch".to_owned());
            }
            let request = neutral::ManifestRequest {
                campaign_id: campaign_id.clone(),
                protocol_sha256: first_case.protocol_sha256.clone(),
                channels: first_case.channels.clone(),
                processes: Vec::new(),
            };
            let expected = ordered_cases(&request)?;
            let mut records = std::collections::BTreeMap::new();
            for (ordinal, (entry, expected)) in bundle.accepted.iter().zip(expected).enumerate() {
                let actual: OwnerCase = entry.unit.case.decode().map_err(err)?;
                if entry.unit.ordinal != ordinal as u64
                    || entry.unit.process != first.unit.process
                    || actual.identity != expected.identity
                    || actual.channels != expected.channels
                    || actual.protocol_sha256 != expected.protocol_sha256
                {
                    return Err("owner input order/channel/slot mismatch".to_owned());
                }
                if let OwnerCaseKind::ReservedM4rm { .. } = &expected.kind {
                    if !matches!(actual.kind,OwnerCaseKind::Extent{ref cell} if cell.field==ExtentField::M4rmJoint)
                    {
                        return Err("joint slot was not materialized".to_owned());
                    }
                } else if actual != expected {
                    return Err("owner input changed a fixed case".to_owned());
                }
                // Per-unit owner validation reconstructed operands/oracles before durable
                // acceptance. Analysis reopens every semantic envelope and recomputes
                // decisions from the accepted windows; it does not execute candidates.
                validate_result(&entry.unit, &entry.result, false)?;
                if records
                    .insert(entry.unit.key.as_str().to_owned(), entry)
                    .is_some()
                {
                    return Err("duplicate accepted owner unit".to_owned());
                }
            }
            Ok(Self { request, records })
        }
        fn get(
            &self,
            kind: OwnerCaseKind,
            task: Task,
        ) -> Result<&'a neutral::AcceptedResult, String> {
            let expected = OwnerCase::new(&self.request, kind, task)?;
            let key = expected.identity.key().map_err(err)?;
            let entry = *self
                .records
                .get(key.as_str())
                .ok_or_else(|| format!("missing accepted unit {}", key.as_str()))?;
            let case: OwnerCase = entry.unit.case.decode().map_err(err)?;
            if case != expected {
                return Err(
                    "accepted case differs from selected vector or fixed contract".to_owned(),
                );
            }
            Ok(entry)
        }
        fn extent_series(
            &self,
            field: ExtentField,
            candidates: &[ExtentCandidate],
        ) -> Result<Vec<statistics::CandidateSeries<ExtentCandidate, EffectiveSchedule>>, String>
        {
            let mut series = Vec::new();
            for candidate in candidates {
                let mut schedules = Vec::new();
                let mut strata = Vec::new();
                for shape_index in 0..field.shape_count() {
                    let sites = if field == ExtentField::GemmTiles {
                        ExtentSite::all().into_iter().map(Some).collect()
                    } else {
                        vec![None]
                    };
                    for site in sites {
                        let cell = ExtentCell {
                            field,
                            shape_index,
                            site,
                            candidate: candidate.clone(),
                        };
                        let kind = OwnerCaseKind::Extent { cell };
                        let probe = self.get(kind.clone(), Task::Probe)?;
                        let evidence: OwnerEvidence = probe.result.payload.decode().map_err(err)?;
                        let OwnerEvidence::Extent { observations, .. } = evidence else {
                            return Err("extent has threshold evidence".to_owned());
                        };
                        schedules.push(observations[0].schedule.clone());
                        let mut medians = Vec::new();
                        for execution in 0..5 {
                            let timed = self.get(kind.clone(), Task::Measure { execution })?;
                            if timed.result.payload != probe.result.payload {
                                return Err("extent timed/probe evidence diverges".to_owned());
                            }
                            medians.push(
                                statistics::execution_median(&timed.result.samples, execution)
                                    .map_err(err)?,
                            );
                        }
                        strata.push(medians);
                    }
                }
                series.push(statistics::CandidateSeries {
                    candidate: candidate.clone(),
                    schedules,
                    strata,
                });
            }
            // Pairing binds common deterministic operands and mathematical answers,
            // even where the candidate changes the effective schedule.
            for shape_index in 0..field.shape_count() {
                let sites = if field == ExtentField::GemmTiles {
                    ExtentSite::all().into_iter().map(Some).collect()
                } else {
                    vec![None]
                };
                for site in sites {
                    let mut paired = None;
                    for candidate in candidates {
                        let entry = self.get(
                            OwnerCaseKind::Extent {
                                cell: ExtentCell {
                                    field,
                                    shape_index,
                                    site,
                                    candidate: candidate.clone(),
                                },
                            },
                            Task::Probe,
                        )?;
                        let OwnerEvidence::Extent {
                            seeds,
                            dimensions,
                            operands_sha256,
                            oracle_sha256,
                            result_sha256,
                            ..
                        } = entry.result.payload.decode().map_err(err)?
                        else {
                            return Err("wrong pairing evidence".to_owned());
                        };
                        let this = (
                            seeds,
                            dimensions,
                            operands_sha256,
                            oracle_sha256,
                            result_sha256,
                        );
                        if let Some(previous) = &paired {
                            if previous != &this {
                                return Err(
                                    "candidate-paired operands/oracle/results differ".to_owned()
                                );
                            }
                        } else {
                            paired = Some(this);
                        }
                    }
                }
            }
            Ok(series)
        }
        fn thresholds(&self) -> Result<Vec<FieldSweep>, String> {
            let mut sweeps = Vec::new();
            for field in CalibratedField::ALL {
                let mut variants = Vec::new();
                for &variant in field.variants() {
                    let mut points = Vec::new();
                    for size in field.grid() {
                        let mut arms = Vec::new();
                        let mut probes = Vec::new();
                        for arm in Arm::BOTH {
                            let spec = ChildSpec {
                                field,
                                variant,
                                size,
                                arm,
                                task: ChildTask::Probe,
                            };
                            let entry = self.get(OwnerCaseKind::Threshold { spec }, Task::Probe)?;
                            let OwnerEvidence::Threshold { report: probe, .. } =
                                entry.result.payload.decode().map_err(err)?
                            else {
                                return Err("threshold has extent evidence".to_owned());
                            };
                            let mut samples = Vec::new();
                            for execution in 0..5 {
                                let spec = ChildSpec {
                                    task: ChildTask::Measure { execution },
                                    ..spec
                                };
                                let entry = self.get(
                                    OwnerCaseKind::Threshold { spec },
                                    Task::Measure { execution },
                                )?;
                                let OwnerEvidence::Threshold { report, .. } =
                                    entry.result.payload.decode().map_err(err)?
                                else {
                                    return Err("threshold has extent evidence".to_owned());
                                };
                                verify_matching_evidence(
                                    "accepted retained probe/timed evidence",
                                    &probe,
                                    &report,
                                )?;
                                samples.extend(entry.result.samples.iter().copied());
                            }
                            arms.push(ArmStat::from_samples(samples));
                            probes.push(probe);
                        }
                        verify_matching_evidence(
                            "accepted retained paired arms",
                            &probes[0],
                            &probes[1],
                        )?;
                        let mut arms = arms.into_iter();
                        points.push(GridPoint {
                            size,
                            conservative: arms.next(),
                            asymptotic: arms.next(),
                        });
                    }
                    variants.push(FieldSweep {
                        field,
                        variant,
                        selection: select(field, &points),
                        points,
                    });
                }
                if field == CalibratedField::InterpolateFastMinPoints {
                    let selection =
                        reconcile_interpolation(&variants[0].selection, &variants[1].selection);
                    // Retain both independently measured input curves beside reconciliation.
                    sweeps.extend(variants);
                    sweeps.push(FieldSweep {
                        field,
                        variant: SweepVariant::Standard,
                        points: Vec::new(),
                        selection,
                    });
                } else {
                    sweeps.extend(variants);
                }
            }
            Ok(sweeps)
        }
    }

    #[derive(Clone, Debug, PartialEq, Serialize)]
    struct NamedExtentDecision {
        field: ExtentField,
        decision: statistics::ExtentDecision<ExtentCandidate>,
    }
    fn one_factor_decisions(input: &AnalysisInput<'_>) -> Result<Vec<NamedExtentDecision>, String> {
        ExtentField::ALL
            .into_iter()
            .filter(|field| *field != ExtentField::GemmTiles)
            .map(|field| {
                let series = input.extent_series(field, &field.candidates())?;
                Ok(NamedExtentDecision {
                    field,
                    decision: statistics::analyze_extent(&series, &field.default_candidate())
                        .map_err(err)?,
                })
            })
            .collect()
    }
    fn suggested_m4rm(decisions: &[NamedExtentDecision]) -> Result<M4rmVector, String> {
        let get = |field| -> Result<usize, String> {
            let decision = &decisions
                .iter()
                .find(|d| d.field == field)
                .ok_or("missing one-factor M4RM decision")?
                .decision;
            match decision.selected {
                ExtentCandidate::Scalar { value } => Ok(value),
                _ => Err("M4RM one-factor suggestion is not scalar".to_owned()),
            }
        };
        Ok(M4rmVector {
            default_table_bytes: get(ExtentField::M4rmDefaultBytes)?,
            mid_table_bytes: get(ExtentField::M4rmMidBytes)?,
            wide_table_bytes: get(ExtentField::M4rmWideBytes)?,
            wide_max_k: get(ExtentField::M4rmWideCap)?,
            small_n_max_k: get(ExtentField::M4rmSmallCap)?,
        })
    }
    fn derive_manifest(
        request: &neutral::DeriveManifestRequest,
    ) -> Result<neutral::DerivedManifest, String> {
        let bundle: neutral::AcceptedResultsBundle = request.accepted_inputs.read().map_err(err)?;
        let input = AnalysisInput::new(
            &bundle,
            &request.campaign_id,
            &request.original_manifest_sha256,
            false,
        )?;
        let decisions = one_factor_decisions(&input)?;
        let proposed = suggested_m4rm(&decisions)?;
        let ordered = ordered_cases(&input.request)?;
        let expected = &ordered[(CORE_CELLS - 24) * 6..];
        if request.reserved_units.len() != 144 {
            return Err("M4RM derivation requires every fixed 24-cell slot".to_owned());
        }
        let mut units = Vec::new();
        for (reserved, expected) in request.reserved_units.iter().zip(expected) {
            let case: OwnerCase = reserved.case.decode().map_err(err)?;
            if &case != expected
                || reserved.identity != expected.identity
                || reserved.ordinal != (CORE_CELLS - 24) as u64 * 6 + units.len() as u64
            {
                return Err("M4RM reservation differs from declared order".to_owned());
            }
            let OwnerCaseKind::ReservedM4rm { mut cell } = case.kind else {
                return Err("nonreserved M4RM derivation input".to_owned());
            };
            if let ExtentCandidate::Vector {
                proposed: true,
                values,
            } = &mut cell.candidate
            {
                *values = proposed.clone();
            }
            let derived = OwnerCase::new(
                &input.request,
                OwnerCaseKind::Extent { cell },
                case.identity.task,
            )?;
            let mut unit = reserved.clone();
            unit.case = CanonicalJson::from_serializable(&derived).map_err(err)?;
            units.push(unit);
        }
        let derived=neutral::DerivedManifest{original_manifest_sha256:request.original_manifest_sha256.clone(),accepted_inputs_sha256:request.accepted_inputs.sha256.clone(),units,derivation:CanonicalJson::from_serializable(&serde_json::json!({"schema":"core-m4rm-conditional-vector-v1","decisions":decisions,"proposed":proposed})).map_err(err)?};
        derived.validate(request).map_err(err)?;
        Ok(derived)
    }

    #[derive(Clone, Debug, PartialEq, Serialize)]
    struct OwnerDecisions {
        schema: String,
        retained_thresholds: Vec<FieldSweep>,
        extents: Vec<NamedExtentDecision>,
        gemm: statistics::GemmDecision,
        proposed_m4rm: M4rmVector,
        joint_m4rm: statistics::JointVectorDecision<M4rmVector>,
        measured: Vec<String>,
        omitted: Vec<String>,
        counts: neutral::DeclaredCounts,
    }
    fn decide_owner(input: &AnalysisInput<'_>) -> Result<(CoreTuning, OwnerDecisions), String> {
        let thresholds = input.thresholds()?;
        let extents = one_factor_decisions(input)?;
        let proposed = suggested_m4rm(&extents)?;
        let gemm_series = input
            .extent_series(ExtentField::GemmTiles, &ExtentField::GemmTiles.candidates())?
            .into_iter()
            .map(|series| {
                let ExtentCandidate::Tiles { row, col } = series.candidate else {
                    unreachable!()
                };
                statistics::CandidateSeries {
                    candidate: (row, col),
                    schedules: series.schedules,
                    strata: series.strata,
                }
            })
            .collect::<Vec<_>>();
        let conservative_gemm = CoreTuning::CONSERVATIVE.gemm();
        let gemm = statistics::analyze_gemm(
            &gemm_series,
            (conservative_gemm.row_tile(), conservative_gemm.col_tile()),
        )
        .map_err(err)?;
        let joint_series = input
            .extent_series(
                ExtentField::M4rmJoint,
                &[
                    ExtentCandidate::Vector {
                        proposed: false,
                        values: M4rmVector::conservative(),
                    },
                    ExtentCandidate::Vector {
                        proposed: true,
                        values: proposed.clone(),
                    },
                ],
            )?
            .into_iter()
            .map(|series| {
                let ExtentCandidate::Vector { values, .. } = series.candidate else {
                    unreachable!()
                };
                statistics::CandidateSeries {
                    candidate: values,
                    schedules: series.schedules,
                    strata: series.strata,
                }
            })
            .collect::<Vec<_>>();
        let joint =
            statistics::analyze_joint_vector(&joint_series[0], &joint_series[1]).map_err(err)?;
        let mut body = complete_selector_value(&CoreTuning::CONSERVATIVE)?;
        for field in CalibratedField::ALL {
            let selection = &thresholds
                .iter()
                .find(|sweep| sweep.field == field && sweep.variant == SweepVariant::Standard)
                .ok_or("missing reconciled threshold")?
                .selection;
            set_leaf(
                &mut body,
                &field.schema_field().to_string(),
                selection.value(),
            )?;
        }
        for named in &extents {
            if !named.field.is_m4rm() {
                let ExtentCandidate::Scalar { value } = named.decision.selected else {
                    unreachable!()
                };
                set_leaf(&mut body, named.field.paths()[0], value)?;
            }
        }
        set_leaf(&mut body, "gemm.row_tile", gemm.selected.0)?;
        set_leaf(&mut body, "gemm.col_tile", gemm.selected.1)?;
        for (path, value) in ExtentField::M4rmJoint
            .paths()
            .iter()
            .zip(joint.selected.values())
        {
            set_leaf(&mut body, path, value)?;
        }
        let measured = measured_inventory()?;
        let mut omitted = Vec::new();
        for leaf in flatten_selectors(&body)? {
            let path = format!("{}.{}", leaf.family, leaf.field);
            if !measured.contains(&path) {
                body.get_mut(&leaf.family)
                    .and_then(serde_json::Value::as_object_mut)
                    .ok_or("missing family")?
                    .remove(&leaf.field);
                omitted.push(path);
            }
        }
        if omitted.len() != 10 {
            return Err("core omission complement mismatch".to_owned());
        }
        let section = CoreTuningCodec::decode_body(CanonicalValue::serialize(&body).map_err(err)?)
            .map_err(err)?;
        Ok((
            section,
            OwnerDecisions {
                schema: OWNER_PROTOCOL.to_owned(),
                retained_thresholds: thresholds,
                extents,
                gemm,
                proposed_m4rm: proposed,
                joint_m4rm: joint,
                measured,
                omitted,
                counts: neutral::DeclaredCounts::for_cells(CORE_CELLS as u64).map_err(err)?,
            },
        ))
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RuntimeFacts {
        source_dirty: bool,
        toolchain: String,
        cpu_model: String,
        cpu_features: Vec<String>,
        os_kernel: String,
        governor: String,
        receipt: String,
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AssemblyRuntime {
        source_dirty: bool,
        tool: String,
        tool_sha256: String,
    }
    fn runtime_facts(provenance: &neutral::ObservedProvenance) -> Result<RuntimeFacts, String> {
        let facts: RuntimeFacts = provenance.runtime.decode().map_err(err)?;
        if facts.source_dirty
            || [
                &facts.toolchain,
                &facts.cpu_model,
                &facts.os_kernel,
                &facts.governor,
                &provenance.identity.host_identity,
            ]
            .iter()
            .any(|value| value.trim().is_empty())
            || !facts.toolchain.contains("1.95.0")
            || facts.cpu_features.is_empty()
            || facts
                .cpu_features
                .iter()
                .any(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            || facts.cpu_features.windows(2).any(|p| p[0] >= p[1])
        {
            return Err("noncanonical or incomplete observed runtime facts".to_owned());
        }
        RepoRelPath::parse(&facts.receipt).map_err(err)?;
        Rfc3339Utc::parse(&provenance.observed_utc).map_err(err)?;
        if provenance.identity.feature_contract != neutral::FEATURE_CONTRACT
            || provenance.identity.thread_contract != neutral::THREAD_CONTRACT
            || provenance.process.as_str() != "core-producer"
            || !provenance
                .identity
                .behavior_sha256
                .contains_key("crates/gf2-core/benches/tuning_calibration.rs")
        {
            return Err("owner measurement feature/thread/behavior contract mismatch".to_owned());
        }
        Ok(facts)
    }
    fn emit_owner(request: &neutral::EmitOwnerRequest) -> Result<OwnerResponse, String> {
        if env::var(BENCH_MODE_VAR).as_deref() != Ok("1") {
            return Err("owner emission requires authorized campaign environment".to_owned());
        }
        require_campaign_environment()?;
        let bundle: neutral::AcceptedResultsBundle =
            request.accepted_results.read().map_err(err)?;
        let input = AnalysisInput::new(
            &bundle,
            &request.campaign_id,
            &request.manifest_sha256,
            true,
        )?;
        if request.measurement.identity != request.assembly.identity
            || request.measurement.process != request.assembly.process
            || request.measurement.identity.protocol_digest
                != input.request.protocol_sha256.as_str()
        {
            return Err("owner provenance differs from measured campaign identity".to_owned());
        }
        let parent = request
            .output
            .parent()
            .ok_or("owner output lacks a parent directory")?;
        let canonical_parent = fs::canonicalize(parent).map_err(err)?;
        if canonical_parent != parent
            || !canonical_parent.starts_with(&input.request.channels.stage)
            || request.output.file_name().is_none()
        {
            return Err(
                "owner publication destination must be inside the canonical campaign stage"
                    .to_owned(),
            );
        }
        let facts = runtime_facts(&request.measurement)?;
        let assembly_runtime: AssemblyRuntime = request.assembly.runtime.decode().map_err(err)?;
        if assembly_runtime.source_dirty
            || assembly_runtime.tool != "crates/gf2-core/benches/tuning_calibration.rs"
        {
            return Err("owner assembly source/tool identity mismatch".to_owned());
        }
        let binary = request
            .measurement
            .identity
            .executable_sha256
            .get(request.measurement.process.as_str())
            .ok_or("missing owner producing binary digest")?;
        let actual = sha256_file(&env::current_exe().map_err(err)?)?;
        if actual.as_str() != binary || assembly_runtime.tool_sha256 != *binary {
            return Err("owner emission executable differs from producing binary".to_owned());
        }
        let (section, decisions) = decide_owner(&input)?;
        let measurement = MeasurementProvenance::Calibrated {
            measured_at: Rfc3339Utc::parse(&request.measurement.observed_utc).map_err(err)?,
            source_revision: GitRevision::parse(&request.measurement.identity.source_revision)
                .map_err(err)?,
            source_dirty: false,
            harness: RepoRelPath::parse("crates/gf2-core/benches/tuning_calibration.rs")
                .map_err(err)?,
            harness_schema: HarnessSchema::parse(CoreTuningCodec::HARNESS_SCHEMA).map_err(err)?,
            binary_sha256: actual.clone(),
            toolchain: facts.toolchain,
            host: request.measurement.identity.host_identity.clone(),
            cpu_model: facts.cpu_model,
            cpu_features: facts.cpu_features,
            os_kernel: facts.os_kernel,
            governor: facts.governor,
            receipt: RepoRelPath::parse(&facts.receipt).map_err(err)?,
        };
        let assembly = AssemblyProvenance {
            assembled_at: Rfc3339Utc::parse(&request.assembly.observed_utc).map_err(err)?,
            source_revision: GitRevision::parse(&request.assembly.identity.source_revision)
                .map_err(err)?,
            source_dirty: false,
            tool: RepoRelPath::parse("crates/gf2-core/benches/tuning_calibration.rs")
                .map_err(err)?,
            tool_sha256: actual,
        };
        let profile = ProducedCoreProfile {
            id: ProfileId::parse(request.campaign_id.as_str()).map_err(err)?,
            measurement,
            assembly,
            section,
        };
        let json = profile.to_json();
        if ProducedCoreProfile::from_json(&json)? != profile {
            return Err("owner strict reopen changed the section or provenance".to_owned());
        }
        let written = emit_profile(&request.output, &json).map_err(err)?;
        if written != json {
            return Err("owner reopen differs from generated artifact".to_owned());
        }
        Ok(OwnerResponse::EmitOwner {
            artifact: neutral::ArtifactIdentity {
                path: fs::canonicalize(&request.output).map_err(err)?,
                sha256: Sha256Digest::of(json.as_bytes()),
            },
            decisions: CanonicalJson::from_serializable(&decisions).map_err(err)?,
        })
    }

    fn validate_extent_grid(
        field: ExtentField,
        candidates: &[ExtentCandidate],
    ) -> Result<(), String> {
        if !candidates.contains(&field.default_candidate()) {
            return Err(format!(
                "conservative default is absent from the declared {field:?} grid"
            ));
        }
        for candidate in candidates {
            let cell = ExtentCell {
                field,
                shape_index: 0,
                site: (field == ExtentField::GemmTiles).then_some(ExtentSite::MatrixGemm),
                candidate: candidate.clone(),
            };
            cell.validate()?;
            cell.section()?;
        }
        Ok(())
    }
    fn preflight() -> Result<(), String> {
        measured_inventory()?;
        for field in ExtentField::ALL {
            validate_extent_grid(field, &field.candidates())?;
        }
        for cell in extent_cells() {
            cell.validate()?;
            cell.section()?;
            if cell.field.is_m4rm() {
                expected_m4rm_schedule(&cell)?;
            }
        }
        Ok(())
    }
    fn report_inventory() -> Result<CanonicalJson, String> {
        preflight()?;
        let extents = extent_cells();
        let retained:Vec<_>=CalibratedField::ALL.into_iter().flat_map(|field|field.variants().iter().map(move|variant|serde_json::json!({"field":field,"variant":variant,"grid":field.grid(),"default":field.conservative_default()}))).collect();
        CanonicalJson::from_serializable(&serde_json::json!({"owner_protocol":OWNER_PROTOCOL,"behavior_token":CoreTuningCodec::HARNESS_SCHEMA,"raw_schema":RAW_WINDOWS,"measured":measured_inventory()?,"counts":neutral::DeclaredCounts::for_cells(CORE_CELLS as u64).map_err(err)?,"retained":retained,"extents":extents,"reserved_joint_cells":24})).map_err(err)
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum CapabilityScope {
        RepresentativePrerequisites,
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DotCapability {
        candidate_chunk: usize,
        effective_chunk: usize,
        batch_clmul_scalar_clmul_and_barrett: bool,
        scalar_equal: bool,
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CapabilityReport {
        scope: CapabilityScope,
        full_grid_probes: bool,
        timed_children: u64,
        fp251_whole_gemm: bool,
        simd_backend: Option<String>,
        dedicated_pool_width: usize,
        representative_dot_length: usize,
        dot: Vec<DotCapability>,
        ordinary_companions: OrdinaryCompanionConstants,
        cpu_features: Vec<String>,
        required_features: String,
        required_threads: String,
    }
    impl CapabilityReport {
        fn validate(&self) -> Result<(), String> {
            if !self.fp251_whole_gemm {
                return Err("required Fp251 whole-GEMM capability unavailable".to_owned());
            }
            if !matches!(
                self.simd_backend.as_deref(),
                Some("avx2" | "avx512" | "neon")
            ) {
                return Err("required concrete SIMD backend unavailable".to_owned());
            }
            if self.dedicated_pool_width != 4 {
                return Err("required dedicated pool width is four".to_owned());
            }
            if self.dot.len() != DotChunkCandidate::ALL.len()
                || self
                    .dot
                    .iter()
                    .zip(DotChunkCandidate::ALL)
                    .any(|(observed, candidate)| {
                        observed.candidate_chunk != candidate.chunk_len()
                            || observed.effective_chunk != candidate.chunk_len()
                            || !observed.batch_clmul_scalar_clmul_and_barrett
                            || !observed.scalar_equal
                            || self.representative_dot_length <= candidate.chunk_len()
                    })
            {
                return Err(
                    "required dot candidate CLMUL/Barrett/scalar-equivalence probe unavailable"
                        .to_owned(),
                );
            }
            self.ordinary_companions.validate()
        }
    }
    fn capability_report() -> Result<CanonicalJson, String> {
        preflight()?;
        let field = Gf2mField::new(8, 0x11b);
        let length = DotChunkCandidate::ALL
            .into_iter()
            .map(|candidate| candidate.chunk_len())
            .max()
            .ok_or("empty dot capability grid")?
            + 1;
        let lhs = FieldVec::from(vec![field.element(3); length]);
        let rhs = FieldVec::from(vec![field.element(5); length]);
        let oracle = lhs.dot_product(&rhs);
        let mut dot = Vec::new();
        for candidate in DotChunkCandidate::ALL {
            gf2_core::field::vec::reset_max_effective_dot_chunk_len();
            let result = candidate.function()(&lhs, &rhs);
            let effective_chunk = gf2_core::field::vec::max_effective_dot_chunk_len();
            // The production observer is written only after obtaining the batch
            // CLMUL, single CLMUL and Barrett reducer and completing the walk.
            dot.push(DotCapability {
                candidate_chunk: candidate.chunk_len(),
                effective_chunk,
                batch_clmul_scalar_clmul_and_barrett: effective_chunk == candidate.chunk_len(),
                scalar_equal: result == oracle,
            });
        }
        let dedicated_pool_width =
            run_in_dedicated_parallel_pool(
                4,
                || Ok::<usize, String>(rayon::current_num_threads()),
            )?;
        let report = CapabilityReport {
            scope: CapabilityScope::RepresentativePrerequisites,
            full_grid_probes: false,
            timed_children: 0,
            fp251_whole_gemm: <Fp251 as FiniteField>::has_simd_gemm_classical(),
            simd_backend: simd_backend().map(|backend| backend.name().to_owned()),
            dedicated_pool_width,
            representative_dot_length: length,
            dot,
            ordinary_companions: OrdinaryCompanionConstants::observe(),
            cpu_features: cpu_features(),
            required_features: REQUIRED_FEATURES.to_owned(),
            required_threads: REQUIRED_RAYON_THREADS.to_owned(),
        };
        report.validate()?;
        CanonicalJson::from_serializable(&report).map_err(err)
    }

    fn operation(request: OwnerOperation) -> Result<OwnerResponse, String> {
        match request {
            OwnerOperation::SelfCheck => Ok(OwnerResponse::SelfCheck {
                evidence: report_inventory()?,
            }),
            OwnerOperation::ListGrid => Ok(OwnerResponse::ListGrid {
                evidence: report_inventory()?,
            }),
            OwnerOperation::CapabilityReport => Ok(OwnerResponse::CapabilityReport {
                evidence: capability_report()?,
            }),
            OwnerOperation::CampaignManifest { request } => Ok(OwnerResponse::CampaignManifest {
                manifest: Box::new(manifest(&request)?),
            }),
            OwnerOperation::ValidateResult { unit, result } => {
                validate_result(&unit, &result, true)?;
                Ok(OwnerResponse::ValidateResult {
                    unit_key: unit.key,
                    result_sha256: result.digest().map_err(err)?,
                })
            }
            OwnerOperation::DeriveManifest { request } => Ok(OwnerResponse::DeriveManifest {
                manifest: derive_manifest(&request)?,
            }),
            OwnerOperation::EmitOwner { request } => emit_owner(&request),
        }
    }
    pub(super) fn report_mode(mode: Mode) -> Result<(), String> {
        let request = match mode {
            Mode::SelfCheck => OwnerOperation::SelfCheck,
            Mode::ListGrid => OwnerOperation::ListGrid,
            Mode::CapabilityReport => OwnerOperation::CapabilityReport,
            _ => return Err("not a report mode".to_owned()),
        };
        let response = operation(request)?;
        transport::write_result_line(io::stdout().lock(), &response).map_err(err)
    }

    pub(super) fn owner_operation() -> Result<(), String> {
        let mut input = String::new();
        io::Read::read_to_string(&mut io::stdin().lock(), &mut input).map_err(err)?;
        let request: OwnerOperation = transport::decode_case(&input)?;
        let response = operation(request)?;
        transport::write_result_line(io::stdout().lock(), &response).map_err(err)
    }

    #[cfg(test)]
    mod tests {
        #[allow(unused_imports)] // custom bench compilation omits test entry points
        use super::*;

        #[allow(dead_code)]
        fn request() -> neutral::ManifestRequest {
            let stage = PathBuf::from("/tmp/gf2-core-owner-contract");
            neutral::ManifestRequest {
                campaign_id: token("gf2-owner-contract").unwrap(),
                protocol_sha256: Sha256Digest::of(b"declared protocol"),
                channels: neutral::SessionChannels {
                    execution_log: stage.join("execution.log"),
                    checkpoints: stage.join("checkpoints"),
                    stage,
                },
                processes: vec![neutral::ProcessDescriptor {
                    id: token("core-producer").unwrap(),
                    executable: PathBuf::from("/tmp/core-producer"),
                    executable_sha256: Sha256Digest::of(b"staged executable"),
                    arguments: vec!["--fresh-tuning-process-child".to_owned()],
                    environment: neutral::measurement_environment(),
                    working_directory: PathBuf::from("/tmp"),
                }],
            }
        }

        #[allow(dead_code)]
        fn synthetic_observation(cell: &ExtentCell) -> ExtentObservation {
            let mut consumed = None;
            let schedule = match cell.field {
                ExtentField::Transpose => EffectiveSchedule::MacroTiled {
                    blocks: cell.scalar(),
                },
                ExtentField::Soa => EffectiveSchedule::SoaParallel {
                    chunk: cell.scalar(),
                },
                ExtentField::Trsm => EffectiveSchedule::TrsmBlocked {
                    panel_rows: cell.scalar(),
                },
                ExtentField::GemmTiles => {
                    let (row, col) = cell.pair().extents();
                    EffectiveSchedule::GemmTiles {
                        row,
                        col,
                        site: cell.site.unwrap(),
                    }
                }
                ExtentField::Dot => EffectiveSchedule::DotClmulBarrett {
                    chunk: cell.scalar(),
                },
                _ => {
                    let (schedule, context) = expected_m4rm_schedule(cell).unwrap();
                    consumed = Some(context);
                    schedule
                }
            };
            ExtentObservation {
                schedule,
                m4rm_consumed: consumed,
                dedicated_pool_width: if cell.field == ExtentField::Soa {
                    Some(4)
                } else {
                    None
                },
                fp251_whole_gemm: if cell.field == ExtentField::Trsm {
                    Some(true)
                } else {
                    None
                },
                quiet_timing: cell.field == ExtentField::Trsm,
            }
        }
        #[allow(dead_code)]
        fn synthetic_result(case: &OwnerCase) -> neutral::ChildResult {
            let samples = match case.identity.task {
                Task::Probe => Vec::new(),
                Task::Measure { execution } => (0..5)
                    .map(|repetition| TimingSample {
                        execution,
                        repetition,
                        calls: 10,
                        elapsed_ns: 1000,
                    })
                    .collect(),
            };
            let payload = match &case.kind {
                OwnerCaseKind::Threshold { spec } => {
                    let (prepared, active_values) = forced_profile_for(*spec).unwrap();
                    let digests = forced_profile_digests(&prepared).unwrap();
                    let (mut effective, capability) = expected_observation_contract(*spec).unwrap();
                    if effective == "panel_width" {
                        effective = "panel_width=4".to_owned();
                    }
                    OwnerEvidence::Threshold {
                        schema: RAW_WINDOWS.to_owned(),
                        ordinary_companions: OrdinaryCompanionConstants::observe(),
                        full_active_values: case.forced_values.clone(),
                        report: Box::new(ChildReport {
                            protocol: ChildProtocolIdentity::current(&fixed_protocol()),
                            installed: InstalledEvidence {
                                profile_id: FORCED_ARM_PROFILE_ID.to_owned(),
                                section_id: CoreTuning::ID.as_str().to_owned(),
                                resolution: ObservedResolution::Installed,
                                measurement: ObservedMeasurement::Inherited,
                                active_values,
                                section_sha256: digests.section_sha256,
                                envelope_content_sha256: digests.envelope_content_sha256,
                            },
                            fixture_shape: expected_fixture_shape(spec.field, spec.size).unwrap(),
                            seed_inventory: seed_inventory(spec.field, spec.size),
                            operand_digest: "a".repeat(64),
                            outcome: ChildOutcome::Complete {
                                requested_route: spec.field.arm_name(spec.arm).to_owned(),
                                observed_route: spec.field.arm_name(spec.arm).to_owned(),
                                effective_observation: Some(effective),
                                capability_observation: Some(capability),
                                result_digest: "b".repeat(64),
                                equivalence_digest: digest_tuple(
                                    b"gf2-calibration-equivalence-v1",
                                    ["a".repeat(64), "b".repeat(64)],
                                ),
                                samples: samples.clone(),
                            },
                        }),
                    }
                }
                OwnerCaseKind::Extent { cell } => {
                    let prepared = section_prepared(cell.section().unwrap()).unwrap();
                    let digests = forced_profile_digests(&prepared).unwrap();
                    OwnerEvidence::Extent {
                        schema: RAW_WINDOWS.to_owned(),
                        ordinary_companions: OrdinaryCompanionConstants::observe(),
                        installed: Box::new(InstalledEvidence {
                            profile_id: FORCED_ARM_PROFILE_ID.to_owned(),
                            section_id: CoreTuning::ID.as_str().to_owned(),
                            resolution: ObservedResolution::Installed,
                            measurement: ObservedMeasurement::Inherited,
                            active_values: case.forced_values.clone(),
                            section_sha256: digests.section_sha256,
                            envelope_content_sha256: digests.envelope_content_sha256,
                        }),
                        seeds: cell.seed_inventory(),
                        dimensions: cell.dimensions(),
                        operands_sha256: Sha256Digest::of(b"synthetic paired operands"),
                        oracle_sha256: Sha256Digest::of(b"synthetic paired oracle"),
                        result_sha256: Sha256Digest::of(b"synthetic paired oracle"),
                        observations: vec![synthetic_observation(cell); 8],
                    }
                }
                OwnerCaseKind::ReservedM4rm { .. } => panic!("synthetic case must be derived"),
            };
            neutral::ChildResult {
                schema: neutral::RESULT_SCHEMA.to_owned(),
                identity: case.identity.clone(),
                case_sha256: CanonicalJson::from_serializable(case).unwrap().digest(),
                outcome: neutral::ChildOutcome::Complete,
                samples,
                payload: CanonicalJson::from_serializable(&payload).unwrap(),
            }
        }

        #[allow(dead_code)]
        fn synthetic_bundle(
            proposed: &M4rmVector,
            elapsed: impl Fn(&ExtentCell) -> u64,
        ) -> (
            neutral::ManifestRequest,
            OwnerManifest,
            neutral::AcceptedResultsBundle,
        ) {
            let request = request();
            let manifest = manifest(&request).unwrap();
            let accepted = manifest
                .ordered_units
                .iter()
                .map(|original| {
                    let mut unit = original.clone();
                    let mut case: OwnerCase = unit.case.decode().unwrap();
                    if let OwnerCaseKind::ReservedM4rm { mut cell } = case.kind {
                        if let ExtentCandidate::Vector {
                            proposed: true,
                            values,
                        } = &mut cell.candidate
                        {
                            *values = proposed.clone();
                        }
                        case = OwnerCase::new(
                            &request,
                            OwnerCaseKind::Extent { cell },
                            case.identity.task,
                        )
                        .unwrap();
                        unit.case = CanonicalJson::from_serializable(&case).unwrap();
                    }
                    let mut result = synthetic_result(&case);
                    if let OwnerCaseKind::Extent { cell } = &case.kind {
                        for sample in &mut result.samples {
                            sample.elapsed_ns = elapsed(cell);
                        }
                    }
                    neutral::AcceptedResult {
                        unit,
                        result,
                        checkpoint_sha256: Sha256Digest::of(b"synthetic accepted checkpoint"),
                    }
                })
                .collect();
            let bundle = neutral::AcceptedResultsBundle {
                schema: neutral::ACCEPTED_RESULTS_SCHEMA.to_owned(),
                manifest_sha256: manifest.manifest_sha256.clone(),
                accepted,
            };
            (request, manifest, bundle)
        }
        #[allow(dead_code)]
        fn nondefault_m4rm() -> M4rmVector {
            M4rmVector {
                wide_max_k: 4,
                small_n_max_k: 4,
                ..M4rmVector::conservative()
            }
        }
        #[allow(dead_code)]
        fn m4rm_elapsed(cell: &ExtentCell, reject_joint: bool) -> u64 {
            match cell.field {
                ExtentField::M4rmWideCap | ExtentField::M4rmSmallCap => 100 * cell.scalar() as u64,
                ExtentField::M4rmJoint
                    if matches!(
                        cell.candidate,
                        ExtentCandidate::Vector { proposed: true, .. }
                    ) =>
                {
                    if reject_joint && cell.shape_index == 0 {
                        2000
                    } else {
                        500
                    }
                }
                _ => 1000,
            }
        }
        #[test]
        fn accepted_owner_selects_nondefault_gemm_coordinate_and_extent() {
            if simd_backend().is_none() {
                return;
            }
            let (request, manifest, bundle) =
                synthetic_bundle(&M4rmVector::conservative(), |cell| match cell.field {
                    ExtentField::GemmTiles => {
                        let (row, col) = cell.pair().extents();
                        (row + col) as u64 * 100
                    }
                    ExtentField::Transpose => 100 * (cell.scalar().ilog2().abs_diff(2) as u64 + 1),
                    _ => 1000,
                });
            let input = AnalysisInput::new(
                &bundle,
                &request.campaign_id,
                &manifest.manifest_sha256,
                true,
            )
            .unwrap();
            let (section, decisions) = decide_owner(&input).unwrap();
            assert_eq!(
                (section.gemm().row_tile(), section.gemm().col_tile()),
                (16, 32)
            );
            assert_eq!(
                decisions.gemm.reason,
                statistics::DecisionReason::SelectedNonDefault
            );
            assert_eq!(section.bit_matrix().transpose_macro_tile_blocks(), 4);
            assert_eq!(
                decisions
                    .extents
                    .iter()
                    .find(|d| d.field == ExtentField::Transpose)
                    .unwrap()
                    .decision
                    .reason,
                statistics::DecisionReason::SelectedNonDefault
            );
        }
        #[test]
        fn accepted_owner_keeps_the_proposed_m4rm_vector_after_joint_validation() {
            if simd_backend().is_none() {
                return;
            }
            let proposed = nondefault_m4rm();
            let (request, manifest, bundle) =
                synthetic_bundle(&proposed, |cell| m4rm_elapsed(cell, false));
            let input = AnalysisInput::new(
                &bundle,
                &request.campaign_id,
                &manifest.manifest_sha256,
                true,
            )
            .unwrap();
            let (section, decisions) = decide_owner(&input).unwrap();
            assert_eq!(decisions.proposed_m4rm, proposed);
            assert_eq!(decisions.joint_m4rm.selected, proposed);
            assert_eq!(section.m4rm().wide_max_k(), 4);
            assert_eq!(section.m4rm().small_n_max_k(), 4);
        }
        #[test]
        fn accepted_owner_falls_back_as_a_whole_after_one_joint_shape_fails() {
            if simd_backend().is_none() {
                return;
            }
            let proposed = nondefault_m4rm();
            let (request, manifest, bundle) =
                synthetic_bundle(&proposed, |cell| m4rm_elapsed(cell, true));
            let input = AnalysisInput::new(
                &bundle,
                &request.campaign_id,
                &manifest.manifest_sha256,
                true,
            )
            .unwrap();
            let (section, decisions) = decide_owner(&input).unwrap();
            assert_eq!(decisions.proposed_m4rm, proposed);
            assert_eq!(decisions.joint_m4rm.selected, M4rmVector::conservative());
            assert_eq!(section.m4rm(), CoreTuning::CONSERVATIVE.m4rm());
        }
        #[test]
        fn accepted_owner_rejects_a_joint_vector_inconsistent_with_one_factor_evidence() {
            if simd_backend().is_none() {
                return;
            }
            let wrong = M4rmVector {
                wide_max_k: 5,
                ..nondefault_m4rm()
            };
            let (request, manifest, bundle) =
                synthetic_bundle(&wrong, |cell| m4rm_elapsed(cell, false));
            let input = AnalysisInput::new(
                &bundle,
                &request.campaign_id,
                &manifest.manifest_sha256,
                true,
            )
            .unwrap();
            assert!(decide_owner(&input)
                .unwrap_err()
                .contains("accepted case differs from selected vector"));
        }
        #[test]
        fn reporting_preflight_rejects_a_default_missing_from_an_extent_grid() {
            preflight().unwrap();
            for field in ExtentField::ALL {
                let candidates = field
                    .candidates()
                    .into_iter()
                    .filter(|candidate| *candidate != field.default_candidate())
                    .collect::<Vec<_>>();
                assert!(validate_extent_grid(field, &candidates)
                    .unwrap_err()
                    .contains("conservative default"));
            }
            assert!(matches!(
                operation(OwnerOperation::SelfCheck).unwrap(),
                OwnerResponse::SelfCheck { .. }
            ));
            assert!(matches!(
                operation(OwnerOperation::ListGrid).unwrap(),
                OwnerResponse::ListGrid { .. }
            ));
        }
        #[test]
        fn capability_report_declares_representative_scope_and_tests_dot_prerequisites() {
            let OwnerResponse::CapabilityReport { evidence } =
                operation(OwnerOperation::CapabilityReport).unwrap()
            else {
                unreachable!()
            };
            let report: CapabilityReport = evidence.decode().unwrap();
            assert!(matches!(
                report.scope,
                CapabilityScope::RepresentativePrerequisites
            ));
            assert!(!report.full_grid_probes);
            assert_eq!(report.timed_children, 0);
            assert_eq!(report.dedicated_pool_width, 4);
            assert_eq!(report.dot.len(), DotChunkCandidate::ALL.len());
            assert!(report.dot.iter().all(|dot| dot.scalar_equal
                && dot.batch_clmul_scalar_clmul_and_barrett
                    == (dot.effective_chunk == dot.candidate_chunk)));
            report.ordinary_companions.validate().unwrap();
        }
        #[test]
        fn capability_preflight_rejects_every_missing_mandatory_prerequisite() {
            let report: CapabilityReport = capability_report().unwrap().decode().unwrap();
            let original = serde_json::to_value(&report).unwrap();
            for (pointer, value) in [
                ("/fp251_whole_gemm", serde_json::json!(false)),
                ("/simd_backend", serde_json::Value::Null),
                ("/dedicated_pool_width", serde_json::json!(1)),
                (
                    "/dot/0/batch_clmul_scalar_clmul_and_barrett",
                    serde_json::json!(false),
                ),
                ("/dot/1/effective_chunk", serde_json::json!(0)),
                ("/dot/2/scalar_equal", serde_json::json!(false)),
            ] {
                let mut changed = original.clone();
                *changed.pointer_mut(pointer).unwrap() = value;
                let changed: CapabilityReport = serde_json::from_value(changed).unwrap();
                assert!(
                    changed.validate().is_err(),
                    "accepted unavailable {pointer}"
                );
            }
            let mut report = report;
            report.dot.pop();
            assert!(
                report.validate().is_err(),
                "accepted a missing candidate probe"
            );
        }
        #[test]
        fn owner_rejects_companion_mutation_independently_of_installed_candidate() {
            let request = request();
            let ordinary = OrdinaryCompanionConstants::observe();
            let candidate = ExtentField::GemmTiles.candidates().into_iter().find(|candidate| !matches!(candidate, ExtentCandidate::Tiles { row, col } if *row == ordinary.gemm_row_tile && *col == ordinary.gemm_col_tile)).unwrap();
            let cell = ExtentCell {
                field: ExtentField::GemmTiles,
                shape_index: 0,
                site: Some(ExtentSite::MatrixGemm),
                candidate,
            };
            let case =
                OwnerCase::new(&request, OwnerCaseKind::Extent { cell }, Task::Probe).unwrap();
            let unit = LaunchUnit::new(
                0,
                case.identity.clone(),
                token("core-producer").unwrap(),
                CanonicalJson::from_serializable(&case).unwrap(),
            )
            .unwrap();
            let mut result = synthetic_result(&case);
            validate_result(&unit, &result, false).unwrap();
            let mut evidence: OwnerEvidence = result.payload.decode().unwrap();
            let OwnerEvidence::Extent {
                ordinary_companions,
                ..
            } = &mut evidence
            else {
                unreachable!()
            };
            ordinary_companions.gemm_row_tile += 1;
            result.payload = CanonicalJson::from_serializable(&evidence).unwrap();
            assert!(validate_result(&unit, &result, false)
                .unwrap_err()
                .contains("ordinary cfg-selected"));
        }

        #[test]
        fn accepted_complete_experiment_measures_defaults_and_reopens_only_its_27_leaves() {
            if simd_backend().is_none() {
                return;
            }
            let request = request();
            let manifest = manifest(&request).unwrap();
            let mut accepted = Vec::new();
            for original in &manifest.ordered_units {
                let mut unit = original.clone();
                let mut case: OwnerCase = unit.case.decode().unwrap();
                if let OwnerCaseKind::ReservedM4rm { cell } = case.kind {
                    case = OwnerCase::new(
                        &request,
                        OwnerCaseKind::Extent { cell },
                        case.identity.task,
                    )
                    .unwrap();
                    unit.case = CanonicalJson::from_serializable(&case).unwrap();
                }
                let result = synthetic_result(&case);
                accepted.push(neutral::AcceptedResult {
                    unit,
                    result,
                    checkpoint_sha256: Sha256Digest::of(b"synthetic accepted checkpoint"),
                });
            }
            let bundle = neutral::AcceptedResultsBundle {
                schema: neutral::ACCEPTED_RESULTS_SCHEMA.to_owned(),
                manifest_sha256: manifest.manifest_sha256.clone(),
                accepted,
            };
            let input = AnalysisInput::new(
                &bundle,
                &request.campaign_id,
                &manifest.manifest_sha256,
                true,
            )
            .unwrap();
            let (section, decisions) = decide_owner(&input).unwrap();
            assert_eq!(section.selectors(), CoreTuning::CONSERVATIVE.selectors());
            assert_eq!(decisions.measured.len(), 27);
            assert_eq!(decisions.omitted.len(), 10);
            assert_eq!(
                decisions.joint_m4rm.reason,
                statistics::JointVectorReason::ConservativeVector
            );
            assert!(decisions
                .extents
                .iter()
                .filter(|d| d.field.is_m4rm())
                .all(|d| d.decision.selected == d.field.default_candidate()));
            let mut profile = super::super::tests::profile_from(&SelectedValues::from_sweeps(&[]));
            profile.section = section;
            let document = profile.to_json();
            let reopened = ProducedCoreProfile::from_json(&document).unwrap();
            assert_eq!(profile, reopened);
            let encoded =
                serde_json::to_value(CoreTuningCodec::encode_body(&reopened.section).unwrap())
                    .unwrap();
            let present = flatten_selectors(&encoded).unwrap();
            assert_eq!(present.len(), 27);
            assert!(present.iter().all(|leaf| decisions
                .measured
                .contains(&format!("{}.{}", leaf.family, leaf.field))));
            let temporary = env::temp_dir().join(format!(
                "gf2-core-owner-analysis-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&temporary).unwrap();
            let input_path = temporary.join("accepted-input.json");
            let prefix = neutral::AcceptedResultsBundle {
                schema: neutral::ACCEPTED_RESULTS_SCHEMA.to_owned(),
                manifest_sha256: manifest.manifest_sha256.clone(),
                accepted: bundle.accepted[..(CORE_CELLS - 24) * 6].to_vec(),
            };
            let prefix_bytes = serde_json::to_vec(&prefix).unwrap();
            fs::write(&input_path, &prefix_bytes).unwrap();
            let derivation_request = neutral::DeriveManifestRequest {
                campaign_id: request.campaign_id.clone(),
                original_manifest_sha256: manifest.manifest_sha256.clone(),
                reserved_units: manifest.ordered_units[(CORE_CELLS - 24) * 6..].to_vec(),
                accepted_inputs: neutral::ArtifactIdentity {
                    path: input_path,
                    sha256: Sha256Digest::of(&prefix_bytes),
                },
            };
            let derived = derive_manifest(&derivation_request).unwrap();
            derived.validate(&derivation_request).unwrap();
            assert_eq!(
                derived.units,
                bundle.accepted[(CORE_CELLS - 24) * 6..]
                    .iter()
                    .map(|entry| entry.unit.clone())
                    .collect::<Vec<_>>()
            );
            let output_path = temporary.join("owner.json");
            assert_eq!(emit_profile(&output_path, &document).unwrap(), document);
            assert_eq!(
                ProducedCoreProfile::from_json(&fs::read_to_string(output_path).unwrap()).unwrap(),
                profile
            );
            fs::remove_dir_all(&temporary).unwrap();
            let mut missing = bundle.clone();
            missing.accepted.pop();
            assert!(AnalysisInput::new(
                &missing,
                &request.campaign_id,
                &manifest.manifest_sha256,
                true
            )
            .is_err());
            let mut changed = bundle;
            changed.accepted[0].unit.case = changed.accepted[1].unit.case.clone();
            assert!(AnalysisInput::new(
                &changed,
                &request.campaign_id,
                &manifest.manifest_sha256,
                true
            )
            .is_err());
        }

        #[test]
        fn complete_manifest_preserves_retained_order_and_rotates_extent_blocks() {
            let request = request();
            let manifest = manifest(&request).unwrap();
            let encoded = serde_json::to_vec(&manifest).unwrap();
            assert_eq!(OwnerManifest::decode(&encoded).unwrap(), manifest);
            assert_eq!(
                manifest.counts,
                neutral::DeclaredCounts::for_cells(702).unwrap()
            );
            let cases: Vec<OwnerCase> = manifest
                .ordered_units
                .iter()
                .map(|u| u.case.decode().unwrap())
                .collect();
            let mut expected = Vec::new();
            for field in CalibratedField::ALL {
                for &variant in field.variants() {
                    for size in field.grid() {
                        for task in std::iter::once(ChildTask::Probe)
                            .chain((0..5).map(|execution| ChildTask::Measure { execution }))
                        {
                            for arm in Arm::BOTH {
                                expected.push(ChildSpec {
                                    field,
                                    variant,
                                    size,
                                    arm,
                                    task,
                                });
                            }
                        }
                    }
                }
            }
            let retained: Vec<_> = cases
                .iter()
                .filter_map(|case| {
                    if let OwnerCaseKind::Threshold { spec } = case.kind {
                        Some(spec)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(retained, expected);
            assert_eq!(retained.len(), 306 * 6);
            let transpose:Vec<_>=cases.iter().filter(|case|matches!(&case.kind,OwnerCaseKind::Extent{cell} if cell.field==ExtentField::Transpose&&cell.shape_index==0)).collect();
            let values: Vec<_> = transpose
                .iter()
                .map(|case| {
                    let OwnerCaseKind::Extent { cell } = &case.kind else {
                        unreachable!()
                    };
                    cell.scalar()
                })
                .collect();
            assert_eq!(
                values,
                [
                    2, 4, 8, 16, 32, 2, 4, 8, 16, 32, 2, 32, 16, 8, 4, 8, 16, 32, 2, 4, 8, 4, 2,
                    32, 16, 32, 2, 4, 8, 16
                ]
            );
            assert!(cases[cases.len() - 144..]
                .iter()
                .all(|case| matches!(case.kind, OwnerCaseKind::ReservedM4rm { .. })));
            let mut malformed = manifest.clone();
            malformed.ordered_units.swap(0, 1);
            assert!(malformed.validate().is_err());
            let mut other = request.clone();
            other.channels.stage = PathBuf::from("/tmp/other");
            assert!(super::manifest(&other).is_err());
        }

        #[test]
        fn case_parser_rejects_extra_controls_wrong_identity_and_reserved_execution() {
            let case = OwnerCase::new(
                &request(),
                OwnerCaseKind::Extent {
                    cell: extent_cells().remove(0),
                },
                Task::Probe,
            )
            .unwrap();
            case.validate(false).unwrap();
            let bytes = transport::encode_case(&case).unwrap();
            assert_eq!(transport::decode_case::<OwnerCase>(&bytes).unwrap(), case);
            assert!(transport::decode_case::<OwnerCase>(&format!("{bytes}\n")).is_err());
            let mut value = serde_json::to_value(&case).unwrap();
            value["kind"]["cell"]["extra"] = serde_json::json!(true);
            assert!(serde_json::from_value::<OwnerCase>(value).is_err());
            let mut changed = case.clone();
            changed.forced_values[0].value += 1;
            assert!(changed.validate(false).is_err());
            let mut changed = case;
            changed.identity.owner = token("gf2-algebra").unwrap();
            assert!(changed.validate(false).is_err());
            let reserved = ordered_cases(&request()).unwrap().pop().unwrap();
            assert!(reserved.validate(true).is_ok());
            assert!(reserved.validate(false).is_err());
        }

        #[test]
        fn fresh_owner_case_adapter() {
            if env::var(FRESH_CASE_VAR).is_ok() {
                fresh_child().unwrap();
            }
        }
        #[allow(dead_code)]
        fn fresh_probe(cell: ExtentCell) -> neutral::ChildResult {
            let case =
                OwnerCase::new(&request(), OwnerCaseKind::Extent { cell }, Task::Probe).unwrap();
            let wire = CanonicalJson::from_serializable(&case).unwrap();
            let unit = LaunchUnit::new(
                0,
                case.identity.clone(),
                token("core-producer").unwrap(),
                wire.clone(),
            )
            .unwrap();
            let mut child = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tuning_calibration::campaign_owner::tests::fresh_owner_case_adapter",
                    "--nocapture",
                ])
                .env(FRESH_CASE_VAR, FRESH_CASE_VALUE)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(wire.as_str().as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!String::from_utf8_lossy(&output.stderr).contains(neutral::PROGRESS_PREFIX));
            let stdout =
                transport::retain_libtest_result_lines(&String::from_utf8(output.stdout).unwrap());
            let result: neutral::ChildResult = transport::parse_result(&stdout).unwrap();
            validate_result(&unit, &result, true).unwrap();
            let response = operation(OwnerOperation::ValidateResult {
                unit: Box::new(unit),
                result: Box::new(result.clone()),
            })
            .unwrap();
            assert!(
                matches!(response,OwnerResponse::ValidateResult{result_sha256,..} if result_sha256==result.digest().unwrap())
            );
            result
        }
        #[allow(dead_code)]
        fn scalar_cell(field: ExtentField, value: usize) -> ExtentCell {
            ExtentCell {
                field,
                shape_index: 0,
                site: None,
                candidate: ExtentCandidate::Scalar { value },
            }
        }

        #[test]
        fn extent_transpose_fresh_probe_proves_full_installed_and_scalar_semantics() {
            fresh_probe(scalar_cell(ExtentField::Transpose, 2));
        }
        #[test]
        fn extent_soa_fresh_probe_observes_all_four_operations() {
            fresh_probe(scalar_cell(ExtentField::Soa, 4096));
        }
        #[test]
        fn extent_m4rm_fresh_probe_proves_consumed_budget_and_rowwise_schedule() {
            fresh_probe(scalar_cell(ExtentField::M4rmDefaultBytes, 16384));
        }
        #[test]
        fn extent_m4rm_small_n_fresh_probe_uses_shared_fixture_identity() {
            fresh_probe(scalar_cell(ExtentField::M4rmSmallCap, 4));
        }
        #[test]
        fn extent_trsm_fresh_probe_proves_quiet_timing_subtree() {
            if <Fp251 as FiniteField>::has_simd_gemm_classical() {
                fresh_probe(scalar_cell(ExtentField::Trsm, 8));
            }
        }
        #[test]
        fn extent_dot_fresh_probe_reaches_const_clmul_chunk() {
            #[cfg(target_arch = "x86_64")]
            if gf2_core::kernels::x86::has_pclmulqdq() {
                fresh_probe(scalar_cell(ExtentField::Dot, 128));
            }
        }
        macro_rules! gemm_probe_test {
            ($name:ident, $site:ident) => {
                #[test]
                fn $name() {
                    fresh_probe(ExtentCell {
                        field: ExtentField::GemmTiles,
                        shape_index: 0,
                        site: Some(ExtentSite::$site),
                        candidate: ExtentCandidate::Tiles { row: 16, col: 32 },
                    });
                }
            };
        }
        gemm_probe_test!(extent_gemm_ordinary_formula, MatrixGemm);
        gemm_probe_test!(extent_gemm_into_formula, MatrixGemmIntoView);
        gemm_probe_test!(extent_gemm_axpy_formula, MatrixGemmAxpyIntoView);
        gemm_probe_test!(
            extent_gemm_implicit_diagonal_formula,
            MatrixGemmAxpyIntoViewDiag
        );
        gemm_probe_test!(extent_gemm_expression_beta_formula, ExprGemmWithBeta);
        gemm_probe_test!(extent_gemm_expression_transpose_formula, ExprGemmTransA);
        gemm_probe_test!(
            extent_gemm_expression_transpose_beta_formula,
            ExprGemmTransAWithBeta
        );

        #[test]
        fn extent_grid_adds_exactly_the_declared_cells_and_codec_leaves() {
            let cells = extent_cells();
            assert_eq!(cells.len(), 372);
            assert_eq!(
                cells
                    .iter()
                    .filter(|c| c.field == ExtentField::GemmTiles)
                    .count(),
                189
            );
            assert_eq!(cells.len() + EXPECTED_GRID_ARM_CELLS + 24, CORE_CELLS);
            let measured = measured_inventory().unwrap();
            assert_eq!(measured.len(), CORE_FIELDS);
            let all = complete_selector_value(&CoreTuning::CONSERVATIVE).unwrap();
            let leaves = flatten_selectors(&all).unwrap();
            assert_eq!(leaves.len(), 37);
            assert_eq!(
                leaves
                    .iter()
                    .filter(|v| !measured.contains(&format!("{}.{}", v.family, v.field)))
                    .count(),
                10
            );
            for field in ExtentField::ALL {
                assert!(field.candidates().contains(&field.default_candidate()));
            }
        }

        #[test]
        fn extent_seeds_pair_candidates_and_gemm_sites_and_m4rm_shapes() {
            let cells = extent_cells();
            for field in ExtentField::ALL {
                let same_shape: Vec<_> = cells
                    .iter()
                    .filter(|c| c.field == field && c.shape_index == 0)
                    .collect();
                let expected = same_shape[0].seed_inventory();
                assert!(same_shape.iter().all(|c| c.seed_inventory() == expected));
            }
            let mut wide = cells
                .iter()
                .find(|c| c.field == ExtentField::M4rmWideCap)
                .unwrap()
                .clone();
            let budget = cells
                .iter()
                .find(|c| c.field == ExtentField::M4rmDefaultBytes)
                .unwrap();
            assert_eq!(wide.seed_inventory(), budget.seed_inventory());
            wide.shape_index = 3;
            let middle = cells
                .iter()
                .find(|c| c.field == ExtentField::M4rmMidBytes)
                .unwrap();
            assert_eq!(wide.seed_inventory(), middle.seed_inventory());
        }

        #[test]
        fn owner_rejects_every_mutated_m4rm_schedule_component() {
            let request = request();
            let cell = scalar_cell(ExtentField::M4rmDefaultBytes, 16384);
            let case =
                OwnerCase::new(&request, OwnerCaseKind::Extent { cell }, Task::Probe).unwrap();
            let unit = LaunchUnit::new(
                0,
                case.identity.clone(),
                token("core-producer").unwrap(),
                CanonicalJson::from_serializable(&case).unwrap(),
            )
            .unwrap();
            let result = synthetic_result(&case);
            validate_result(&unit, &result, false).unwrap();
            let mutations = [
                ("/schedule/tier", serde_json::json!("small_n")),
                ("/schedule/panel_width", serde_json::json!(2)),
                ("/schedule/c_update", serde_json::json!("tiled")),
                ("/m4rm_consumed/band", serde_json::json!("wide")),
                ("/m4rm_consumed/table_bytes", serde_json::json!(32768)),
                ("/m4rm_consumed/panel_width_cap", serde_json::json!(4)),
                (
                    "/m4rm_consumed/tiled_stride_admitted",
                    serde_json::json!(true),
                ),
            ];
            for (path, value) in mutations {
                let mut changed = result.clone();
                let mut payload: serde_json::Value =
                    serde_json::from_str(changed.payload.as_str()).unwrap();
                for observation in payload["observations"].as_array_mut().unwrap() {
                    let target = observation.pointer_mut(path).unwrap();
                    assert_ne!(*target, value, "mutation must change {path}");
                    *target = value.clone();
                }
                match serde_json::from_value::<OwnerEvidence>(payload) {
                    Ok(evidence) => {
                        changed.payload = CanonicalJson::from_serializable(&evidence).unwrap();
                        assert!(
                            validate_result(&unit, &changed, false).is_err(),
                            "accepted changed {path}"
                        );
                    }
                    Err(_) => assert_eq!(
                        path, "/schedule/c_update",
                        "only the closed C-update vocabulary mutation fails decoding"
                    ),
                }
            }
        }

        #[test]
        fn m4rm_rejects_an_alternate_in_range_panel_width() {
            let cell = scalar_cell(ExtentField::M4rmDefaultBytes, 16384);
            let mut observation = synthetic_observation(&cell);
            let (_, k, n) = cell.dimensions();
            let section = cell.section().unwrap();
            let route =
                gf2_core::alg::m4rm::m4rm_schedule_route_for_selectors(section.m4rm(), k, n);
            let EffectiveSchedule::M4rm { panel_width, .. } = &mut observation.schedule else {
                unreachable!()
            };
            *panel_width = route.panel_width();
            validate_extent_observation(&cell, &observation).unwrap();
            let EffectiveSchedule::M4rm { panel_width, .. } = &mut observation.schedule else {
                unreachable!()
            };
            *panel_width = if *panel_width == 2 { 3 } else { 2 };
            assert!(*panel_width <= route.panel_width_cap());
            assert!(validate_extent_observation(&cell, &observation).is_err());
        }

        #[test]
        fn extent_candidate_validation_rejects_wrong_grid_site_and_controls() {
            let mut cell = extent_cells().remove(0);
            assert!(cell.validate().is_ok());
            cell.candidate = ExtentCandidate::Scalar { value: 3 };
            assert!(cell.validate().is_err());
            cell.candidate = ExtentCandidate::Scalar { value: 2 };
            cell.site = Some(ExtentSite::MatrixGemm);
            assert!(cell.validate().is_err());
        }
    }
}
