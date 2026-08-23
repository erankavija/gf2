//! Triangular-solve route observation under an installed tuning profile.
//!
//! This binary installs values below the conservative threshold. The
//! above-default boundary lives in `tuning_profile_triangular_install_above`.

use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::triangular::{
    last_effective_trsm_panel_rows, reset_last_effective_trsm_panel_rows, trsm_route, TrsmRoute,
};
use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_triangular_profile_lowers_threshold_and_reaches_blocked_panel() {
    let text = r#"
    {
      "schema_version": 1,
      "profile_id": "triangular-route-test-below",
      "provenance": {"kind": "inherited"},
      "selectors": {
        "triangular": {
          "trsm_blocked_min_dim": 8,
          "trsm_panel_rows": 3
        }
      }
    }
    "#;
    let profile = TuningProfile::from_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));

    let triangular = tuning::active().triangular();
    assert_eq!(triangular.trsm_blocked_min_dim(), 8);
    assert_eq!(triangular.trsm_panel_rows(), 3);

    assert_eq!(trsm_route(7), TrsmRoute::Recursive);
    assert_eq!(trsm_route(8), TrsmRoute::Blocked);
    assert_eq!(
        trsm_route(
            TuningProfile::CONSERVATIVE
                .triangular()
                .trsm_blocked_min_dim()
                - 1
        ),
        TrsmRoute::Blocked,
        "the installed threshold must move the conservative boundary"
    );

    // `solve_batch` is the production dispatcher. The observation is taken
    // at the blocked callee entry, so equal results cannot mask an ignored
    // installed panel width. The blocked arm runs only when the field
    // exposes the SIMD whole-GEMM fast path; on fallback configurations
    // (no `simd` feature, or no AVX2 at runtime) the recursive arm runs
    // and no panel width is observed — the same gate the dispatcher uses
    // decides which assertion applies.
    reset_last_effective_trsm_panel_rows();
    let a = FieldMatrix::<Fp<251>>::identity(8);
    let b = FieldMatrix::<Fp<251>>::identity(8);
    assert!(a.solve_batch(&b).is_some());
    if <Fp<251> as FiniteField>::has_simd_gemm_classical() {
        assert_eq!(
            last_effective_trsm_panel_rows(),
            Some(3),
            "blocked callee must observe the installed panel width"
        );
    } else {
        assert_eq!(
            last_effective_trsm_panel_rows(),
            None,
            "without the SIMD gemm fast path the recursive arm runs"
        );
    }
}
