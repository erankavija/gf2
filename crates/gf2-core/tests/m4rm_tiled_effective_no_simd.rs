#![cfg(all(feature = "tuning-profile", feature = "test-support"))]

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::alg::m4rm::{
    m4rm_schedule_route, m4rm_tiled_effective_observation, multiply,
    reset_m4rm_tiled_effective_observation, M4rmTiledEffectiveObservation,
};
use gf2_core::matrix::BitMatrix;
use gf2_core::tuning::{
    self, AssemblyProvenance, CoreTuning, CoreTuningCodec, GitRevision, ProfileRegistryBuilder,
    RepoRelPath, Rfc3339Utc, SectionResolution, Sha256,
};

fn assembly_provenance() -> AssemblyProvenance {
    AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("2026-09-01T00:00:00Z").unwrap(),
        source_revision: GitRevision::parse("0123456789abcdef0123456789abcdef01234567").unwrap(),
        source_dirty: false,
        tool: RepoRelPath::parse("crates/gf2-core/tests/m4rm_tiled_effective_no_simd.rs").unwrap(),
        tool_sha256: Sha256::parse(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap(),
    }
}

fn strict_tiled_profile() -> gf2_core::tuning::PreparedEnvelope {
    let prepared = support::prepared_core_json(r#"{"m4rm":{"tiled_min_stride_words":4}}"#)
        .expect("the M4RM test profile is valid");
    let registry = ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .unwrap()
        .build()
        .unwrap();
    let assembly = assembly_provenance();
    let canonical = registry.to_json(&prepared, &assembly).unwrap();
    let reopened = registry
        .from_json(&canonical)
        .expect("the canonical format-2 profile reopens strictly");
    assert_eq!(
        registry.to_json(&reopened, &assembly).unwrap(),
        canonical,
        "strict reopen preserves the canonical format-2 profile"
    );
    reopened
}

fn deterministic_fixture() -> (BitMatrix, BitMatrix) {
    let mut lhs = BitMatrix::zeros(64, 512);
    for row in 0..64 {
        lhs.set(row, row, true);
        lhs.set(row, row + 64, true);
    }

    let mut rhs = BitMatrix::zeros(512, 256);
    for row in 0..512 {
        for col in 0..256 {
            rhs.set(row, col, (row * 17 + col * 31 + 7) % 13 < 6);
        }
    }
    (lhs, rhs)
}

fn basis_pair_oracle(rhs: &BitMatrix) -> BitMatrix {
    let mut expected = BitMatrix::zeros(64, 256);
    for row in 0..64 {
        for col in 0..256 {
            expected.set(row, col, rhs.get(row, col) ^ rhs.get(row + 64, col));
        }
    }
    expected
}

support::fresh_tuning_test!(admitted_stride_falls_back_without_simd_capability, {
    let profile = strict_tiled_profile();
    assert_eq!(tuning::install(profile), Ok(()));
    let active = tuning::active();
    assert_eq!(active.m4rm().tiled_min_stride_words(), 4);
    assert!(matches!(
        active.resolution,
        SectionResolution::Installed { .. }
    ));

    let (lhs, rhs) = deterministic_fixture();
    assert_eq!((lhs.rows(), lhs.cols()), (64, 512));
    assert_eq!((rhs.rows(), rhs.cols()), (512, 256));
    assert!(m4rm_schedule_route(lhs.cols(), rhs.cols()).tiled_stride_admitted());

    let expected = basis_pair_oracle(&rhs);
    reset_m4rm_tiled_effective_observation();
    assert_eq!(
        m4rm_tiled_effective_observation(),
        M4rmTiledEffectiveObservation::None
    );
    let actual = multiply(&lhs, &rhs);
    assert_eq!(
        m4rm_tiled_effective_observation(),
        M4rmTiledEffectiveObservation::RowWise,
        "the no-SIMD build must expose capability decline after stride admission"
    );
    assert_eq!(actual, expected);
});
