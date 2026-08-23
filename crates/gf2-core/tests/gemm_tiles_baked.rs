//! Production-site witnesses for the baked GEMM row and column tiles.
#![cfg(gf2_tuning_baked)]

use gf2_core::field::matrix::{
    gemm, gemm_tile_observations, reset_gemm_tile_observations, FieldMatrix, GemmTileSite,
};
use gf2_core::field::triangular::trtrm;
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn baked_tiles_reach_all_seven_blocked_loops() {
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "gemm-baked-tile-install-witness",
          "provenance": {"kind": "inherited"},
          "selectors": {"gemm": {"row_tile": 1, "col_tile": 1}}
        }
        "#,
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

    let committed: serde_json::Value = serde_json::from_str(include_str!(
        "../data/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080.json"
    ))
    .expect("committed calibrated profile is valid JSON");
    let conservative_profile = TuningProfile::CONSERVATIVE;
    let conservative = conservative_profile.gemm();
    let expected_row_tile = committed["selectors"]["gemm"]["row_tile"]
        .as_u64()
        .map_or(conservative.row_tile(), |value| value as usize);
    let expected_col_tile = committed["selectors"]["gemm"]["col_tile"]
        .as_u64()
        .map_or(conservative.col_tile(), |value| value as usize);

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
}
