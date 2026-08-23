//! Winograd route observation under an installed profile above the
//! conservative `gemm.winograd_min_dim` value.
//!
//! `tuning::install` is one-shot per process, so the below-default boundary
//! lives in `tuning_profile_winograd_install`.

use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::winograd::{
    gemm_winograd, last_winograd_dispatch_route, reset_last_winograd_dispatch_route,
    winograd_route, WinogradRoute,
};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_winograd_profile_raises_boundary_above_conservative() {
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "winograd-route-test-above",
          "provenance": {"kind": "inherited"},
          "selectors": {"gemm": {"winograd_min_dim": 129}}
        }
        "#,
    )
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));

    let gemm_selectors = tuning::active().gemm();
    assert_eq!(gemm_selectors.winograd_min_dim(), 129);
    let conservative = TuningProfile::CONSERVATIVE.gemm().winograd_min_dim();
    assert!(129 > conservative);

    assert_eq!(winograd_route(128, 128, 128), WinogradRoute::Classical);
    assert_eq!(winograd_route(129, 129, 129), WinogradRoute::Winograd);

    let a = FieldMatrix::<Fp<7>>::identity(128);
    let b = FieldMatrix::<Fp<7>>::identity(128);
    let expected = gemm(&a, &b);
    reset_last_winograd_dispatch_route();
    assert_eq!(gemm_winograd(&a, &b), expected);
    assert_eq!(
        last_winograd_dispatch_route(),
        Some(WinogradRoute::Classical),
        "the production dispatcher must consume the installed threshold"
    );
}
