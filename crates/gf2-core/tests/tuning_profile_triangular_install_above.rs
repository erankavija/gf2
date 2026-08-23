//! Triangular-solve route observation under an installed above-default profile.
//!
//! The below-default boundary and the panel-width production-path witness
//! live in `tuning_profile_triangular_install`.

use gf2_core::field::triangular::{trsm_route, TrsmRoute};
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_triangular_profile_raises_threshold_above_conservative_boundary() {
    let text = r#"
    {
      "schema_version": 1,
      "profile_id": "triangular-route-test-above",
      "provenance": {"kind": "inherited"},
      "selectors": {
        "triangular": {
          "trsm_blocked_min_dim": 128,
          "trsm_panel_rows": 5
        }
      }
    }
    "#;
    let profile = TuningProfile::from_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));

    let triangular = tuning::active().triangular();
    assert_eq!(triangular.trsm_blocked_min_dim(), 128);
    assert_eq!(triangular.trsm_panel_rows(), 5);

    assert_eq!(trsm_route(127), TrsmRoute::Recursive);
    assert_eq!(trsm_route(128), TrsmRoute::Blocked);
    assert_eq!(
        trsm_route(
            TuningProfile::CONSERVATIVE
                .triangular()
                .trsm_blocked_min_dim()
        ),
        TrsmRoute::Recursive,
        "the installed threshold must move the conservative boundary"
    );
}
