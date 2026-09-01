//! GEMM AXPY route observation under an installed profile whose volume
//! boundary is above the conservative default.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::{
    gemm_axpy_route, last_gemm_axpy_dispatch_route, reset_last_gemm_axpy_dispatch_route,
    run_gemm_axpy_dispatch, FieldMatrix, GemmAxpyRoute,
};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(installed_gemm_profile_raises_axpy_volume_boundary, {
    let profile = support::prepared_core_json(
        r#"
        {"gemm": {"axpy_fast_path_min_volume": 8192}}"#,
    )
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let active_tuning = tuning::active();
    let gemm = active_tuning.gemm();
    assert_eq!(gemm.axpy_fast_path_min_volume(), 8192);

    let conservative_boundary = CoreTuning::CONSERVATIVE.gemm().axpy_fast_path_min_volume();
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
    run_gemm_axpy_dispatch(&a, &b, &mut out);
    assert_eq!(out, FieldMatrix::<Fp<65537>>::identity(16));
    assert_eq!(
        last_gemm_axpy_dispatch_route(),
        Some(GemmAxpyRoute::PerCell),
        "the production dispatcher must consume the installed boundary"
    );
});
