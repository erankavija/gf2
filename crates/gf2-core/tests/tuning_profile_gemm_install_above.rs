//! GEMM AXPY route observation under an installed profile whose volume
//! boundary is above the conservative default.

use gf2_core::field::matrix::{
    gemm_axpy_route, last_gemm_axpy_dispatch_route, reset_last_gemm_axpy_dispatch_route,
    run_gemm_axpy_dispatch_for_test, FieldMatrix, GemmAxpyRoute,
};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_gemm_profile_raises_axpy_volume_boundary() {
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "gemm-axpy-route-test-above",
          "provenance": {"kind": "inherited"},
          "selectors": {"gemm": {"axpy_fast_path_min_volume": 8192}}
        }
        "#,
    )
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let gemm = tuning::active().gemm();
    assert_eq!(gemm.axpy_fast_path_min_volume(), 8192);

    let conservative_boundary = TuningProfile::CONSERVATIVE
        .gemm()
        .axpy_fast_path_min_volume();
    assert_eq!(conservative_boundary, 16 * 16 * 16);
    assert_eq!(
        gemm_axpy_route(16, 16, 16),
        GemmAxpyRoute::PerCell,
        "the conservative boundary must no longer select the whole-GEMM route"
    );
    assert_eq!(gemm_axpy_route(32, 16, 16), GemmAxpyRoute::WholeGemm);

    reset_last_gemm_axpy_dispatch_route();
    let a = FieldMatrix::<Fp<65537>>::identity(16);
    let b = FieldMatrix::<Fp<65537>>::identity(16);
    let mut out = FieldMatrix::<Fp<65537>>::zeros(16, 16);
    run_gemm_axpy_dispatch_for_test(&a, &b, &mut out);
    assert_eq!(out, FieldMatrix::<Fp<65537>>::identity(16));
    assert_eq!(
        last_gemm_axpy_dispatch_route(),
        Some(GemmAxpyRoute::PerCell),
        "the production dispatcher must consume the installed boundary"
    );
}
