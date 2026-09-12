//! Triangular-solve route observation under an installed above-default profile.
//!
//! The below-default boundary and the panel-width production-path witness
//! live in `tuning_profile_triangular_install`.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::triangular::{trsm_route, TrsmRoute};
use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(
    installed_triangular_profile_raises_threshold_above_conservative_boundary,
    {
        let text = r#"
    {
        "triangular": {
          "trsm_blocked_min_dim": 128,
          "trsm_panel_rows": 5
        }
      }"#;
        let profile = support::prepared_core_json(text).expect("test profile is valid");
        assert_eq!(tuning::install(profile), Ok(()));

        let active_tuning = tuning::active();

        let triangular = active_tuning.triangular();
        assert_eq!(triangular.trsm_blocked_min_dim(), 128);
        assert_eq!(triangular.trsm_panel_rows(), 5);

        assert_eq!(trsm_route(127), TrsmRoute::Recursive);
        assert_eq!(trsm_route(128), TrsmRoute::Blocked);
        assert_eq!(
            trsm_route(CoreTuning::CONSERVATIVE.triangular().trsm_blocked_min_dim()),
            TrsmRoute::Recursive,
            "the installed threshold must move the conservative boundary"
        );
    }
);
