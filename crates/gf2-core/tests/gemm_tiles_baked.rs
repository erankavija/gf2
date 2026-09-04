//! Production-site witnesses for the baked GEMM row and column tiles.
//!
//! The measured format-2 core owner at
//! `crates/gf2-core/data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json`
//! cites `dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md` and omits
//! `gemm.row_tile` and `gemm.col_tile`. These extents retain their conservative defaults,
//! as `dev/active/3fa7c9d0/design.md` §7.1 requires.
#![cfg(gf2_tuning_baked)]

#[path = "support/measured_format2.rs"]
mod measured_format2;
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::{
    gemm, gemm_tile_observations, reset_gemm_tile_observations, FieldMatrix, GemmTileSite,
};
use gf2_core::field::triangular::trtrm;
use gf2_core::gfp::Fp;
use gf2_core::tuning;

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

    let measured = measured_format2::omitted_fields_section("gemm", &["row_tile", "col_tile"]);
    let expected_row_tile = measured.gemm().row_tile();
    let expected_col_tile = measured.gemm().col_tile();

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
