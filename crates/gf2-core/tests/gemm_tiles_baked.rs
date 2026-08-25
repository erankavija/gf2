//! Production-site witnesses for the baked GEMM row and column tiles.
//!
//! The format-1 calibration archived at
//! `dev/archive/3fa7c9d0/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080-v1.json`
//! (SHA-256 `674eea65379d1c814cd54584ad1ea4517fc3f2adbef3d5229d58593e9aad63bb`)
//! cites `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md` but
//! omits `gemm`. These extent fields therefore retain their conservative
//! defaults until measured format-2 evidence supersedes that anchor, as
//! `dev/active/3fa7c9d0/design.md` §7.1 requires.
#![cfg(gf2_tuning_baked)]

#[path = "support/historical_v1.rs"]
mod historical_v1;
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::{
    gemm, gemm_tile_observations, reset_gemm_tile_observations, FieldMatrix, GemmTileSite,
};
use gf2_core::field::triangular::trtrm;
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(baked_tiles_reach_all_seven_blocked_loops, {
    let profile = support::prepared_core_json(
        r#"
        {"gemm": {"row_tile": 1, "col_tile": 1}}"#,
    )
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    assert_eq!(tuning::active().gemm().row_tile(), 1);
    assert_eq!(tuning::active().gemm().col_tile(), 1);

    reset_gemm_tile_observations();

    // The ordinary GEMM loop.
    let a = FieldMatrix::<Fp<65537>>::identity(2);
    let b = FieldMatrix::<Fp<65537>>::identity(2);
    assert_eq!(gemm(&a, &b), a);

    // One recursive trtrm level reaches gemm_into_view, gemm_axpy_into_view,
    // and the implicit-diagonal gemm_axpy_into_view_diag loop.
    let mut l = FieldMatrix::<Fp<65537>>::identity(9);
    let u = FieldMatrix::<Fp<65537>>::identity(9);
    trtrm(l.submat_mut(.., ..), u.submat(.., ..));
    assert_eq!(l, FieldMatrix::<Fp<65537>>::identity(9));

    // The three concrete expression-template GEMM loops.
    let c = FieldMatrix::<Fp<65537>>::identity(2);
    let _: FieldMatrix<Fp<65537>> = (&a * &b + &c).into();
    let _: FieldMatrix<Fp<65537>> = (a.t() * &b).into();
    let one = Fp::<65537>::new(1);
    let _: FieldMatrix<Fp<65537>> = ((one * a.t()) * &b + one * &c).into();

    historical_v1::assert_family_was_omitted("gemm");
    let expected_row_tile = CoreTuning::CONSERVATIVE.gemm().row_tile();
    let expected_col_tile = CoreTuning::CONSERVATIVE.gemm().col_tile();

    let observations = gemm_tile_observations();
    assert_eq!(observations.len(), GemmTileSite::ALL.len());
    for site in GemmTileSite::ALL {
        let observation = observations
            .iter()
            .find(|observation| observation.site == site)
            .unwrap_or_else(|| panic!("missing production observation for {site:?}"));
        assert_eq!(observation.row_tile, expected_row_tile, "site {site:?}");
        assert_eq!(observation.col_tile, expected_col_tile, "site {site:?}");
    }
});
