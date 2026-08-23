//! Installed-profile route observation for the M4RM schedule selectors, with
//! the wide-tier boundary moved *above* the conservative default.
//!
//! `tuning::install` is one-shot per process, so this file owns exactly one
//! installed profile and one test, following
//! `tests/tuning_profile_polynomial_install.rs`. The three table budgets are
//! omitted from the document so the loader resolves them to the conservative
//! defaults while the two ceilings and the boundary move.

use gf2_core::alg::m4rm::{m4rm_schedule_route, multiply, M4rmScheduleTier};
use gf2_core::matrix::BitMatrix;
use gf2_core::tuning::{self, TuningProfile};

const PROFILE: &str = r#"
{
  "schema_version": 1,
  "profile_id": "m4rm-route-boundary-up",
  "provenance": {"kind": "inherited"},
  "selectors": {
    "m4rm": {
      "wide_tier_min_stride_words": 64,
      "tiled_min_stride_words": 16,
      "wide_max_k": 3,
      "small_n_max_k": 5
    }
  }
}
"#;

#[test]
fn installed_m4rm_profile_raises_the_tier_boundary_and_keeps_omitted_budgets() {
    let profile = TuningProfile::from_json(PROFILE).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let m4rm = tuning::active().m4rm();
    let conservative_profile = TuningProfile::CONSERVATIVE;
    let conservative = conservative_profile.m4rm();
    assert_eq!(m4rm.wide_tier_min_stride_words(), 64);
    assert_eq!(m4rm.tiled_min_stride_words(), 16);
    assert_eq!(m4rm.wide_max_k(), 3);
    assert_eq!(m4rm.small_n_max_k(), 5);
    // Omitted keys inherit the conservative defaults.
    assert_eq!(
        m4rm.default_table_bytes(),
        conservative.default_table_bytes()
    );
    assert_eq!(m4rm.mid_table_bytes(), conservative.mid_table_bytes());
    assert_eq!(m4rm.wide_table_bytes(), conservative.wide_table_bytes());

    let boundary = m4rm.wide_tier_min_stride_words();
    let k = 4096;

    // A stride the conservative table routes to the wide tier now falls in the
    // small-`n` tier, which is the boundary moving with the profile.
    let n_conservative_wide = conservative.wide_tier_min_stride_words() * 64;
    assert!(n_conservative_wide < boundary * 64);
    assert_eq!(
        m4rm_schedule_route(k, n_conservative_wide).tier(),
        M4rmScheduleTier::SmallN
    );

    // The neighbours of the installed boundary.
    let n_below = (boundary - 1) * 64;
    let n_at = n_below + 1;
    assert_eq!(
        m4rm_schedule_route(k, n_below).tier(),
        M4rmScheduleTier::SmallN
    );
    assert_eq!(m4rm_schedule_route(k, n_at).tier(), M4rmScheduleTier::Wide);

    // Both panel-width ceilings bind: the small-`n` heuristic asks for a wider
    // panel than the installed ceiling at these widths, and the conservative
    // wide budget admits far more than the installed wide ceiling.
    assert_eq!(
        m4rm_schedule_route(k, n_conservative_wide).panel_width(),
        m4rm.small_n_max_k()
    );
    assert_eq!(
        m4rm_schedule_route(k, n_below).panel_width(),
        m4rm.small_n_max_k()
    );
    assert_eq!(
        m4rm_schedule_route(k, n_at).panel_width(),
        m4rm.wide_max_k()
    );

    // The product is still the product at the width the conservative table
    // would have routed to the wide tier.
    let a = BitMatrix::random_seeded(16, 33, 0x1f2e_3d4c);
    let b = BitMatrix::random_seeded(33, n_conservative_wide, 0x5b6a_7988);
    assert_eq!(multiply(&a, &b), naive_multiply(&a, &b));
}

/// Reference product used as the correctness witness: no schedule, no tables.
fn naive_multiply(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    let mut c = BitMatrix::zeros(a.rows(), b.cols());
    for row in 0..a.rows() {
        for inner in 0..a.cols() {
            if a.get(row, inner) {
                for col in 0..b.cols() {
                    if b.get(inner, col) {
                        c.set(row, col, !c.get(row, col));
                    }
                }
            }
        }
    }
    c
}
