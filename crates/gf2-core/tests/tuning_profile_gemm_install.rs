//! GEMM AXPY route observation under an installed profile whose volume
//! boundary is below the conservative default.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::{
    gemm_axpy_route, last_gemm_axpy_dispatch_route, reset_last_gemm_axpy_dispatch_route,
    run_gemm_axpy_dispatch, FieldMatrix, GemmAxpyRoute,
};
use gf2_core::gfp::Fp;
use gf2_core::tuning;

support::fresh_tuning_test!(installed_gemm_profile_lowers_axpy_volume_boundary, {
    let profile = support::prepared_core_json(
        r#"
        {"gemm": {"axpy_fast_path_min_volume": 1}}"#,
    )
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let active_tuning = tuning::active();
    let gemm = active_tuning.gemm();
    assert_eq!(gemm.axpy_fast_path_min_volume(), 1);

    let threshold = gemm.axpy_fast_path_min_volume();
    assert_eq!(gemm_axpy_route(0, 1, 1), GemmAxpyRoute::PerCell);
    assert_eq!(gemm_axpy_route(threshold, 1, 1), GemmAxpyRoute::WholeGemm);

    reset_last_gemm_axpy_dispatch_route();
    let a = FieldMatrix::<Fp<65537>>::identity(1);
    let b = FieldMatrix::<Fp<65537>>::identity(1);
    let mut out = FieldMatrix::<Fp<65537>>::zeros(1, 1);
    run_gemm_axpy_dispatch(&a, &b, &mut out);
    assert_eq!(out, FieldMatrix::<Fp<65537>>::identity(1));
    assert_eq!(
        last_gemm_axpy_dispatch_route(),
        Some(GemmAxpyRoute::WholeGemm),
        "the production dispatcher must consume the installed boundary"
    );
});
