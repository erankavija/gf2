//! PLE route observation under an installed tuning profile whose selector
//! values sit below the conservative defaults.
//!
//! `tuning::install` resolves once per process, so the above-default half of
//! both boundaries lives in `tuning_profile_ple_install_above.rs`.

use gf2_core::field::matrix::gemm;
use gf2_core::field::ple::{back_sub_route, ple_panel_route, BackSubRoute, PlePanelRoute};
use gf2_core::field::test_random_matrix::{direct_rref_oracle_fp, random_fp};
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_ple_profile_lowers_both_route_boundaries() {
    let text = r#"
    {
      "schema_version": 1,
      "profile_id": "ple-route-test-below",
      "provenance": {"kind": "inherited"},
      "selectors": {
        "ple": {
          "panel_base_max_cols": 32,
          "blocked_back_sub_min_dim": 8
        }
      }
    }
    "#;
    let profile = TuningProfile::from_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let ple = tuning::active().ple();
    assert_eq!(ple.panel_base_max_cols(), 32);
    assert_eq!(ple.blocked_back_sub_min_dim(), 8);

    // ── Panel-width boundary ────────────────────────────────────────────────
    let panel_base_max_cols = ple.panel_base_max_cols();
    assert_eq!(
        ple_panel_route(panel_base_max_cols),
        PlePanelRoute::PanelBase,
        "the widest window the panel base handles directly is the field value"
    );
    assert_eq!(
        ple_panel_route(panel_base_max_cols + 1),
        PlePanelRoute::SubPanelRecursion
    );
    let conservative_panel = TuningProfile::CONSERVATIVE.ple().panel_base_max_cols();
    assert_eq!(
        ple_panel_route(conservative_panel),
        PlePanelRoute::SubPanelRecursion,
        "the conservative panel width now splits into sub-panels"
    );

    // ── Back-substitution boundary ──────────────────────────────────────────
    let blocked_back_sub_min_dim = ple.blocked_back_sub_min_dim();
    assert_eq!(
        back_sub_route(blocked_back_sub_min_dim - 1, blocked_back_sub_min_dim - 1),
        BackSubRoute::Scalar
    );
    assert_eq!(
        back_sub_route(blocked_back_sub_min_dim, 1),
        BackSubRoute::Blocked
    );
    let conservative_dim = TuningProfile::CONSERVATIVE.ple().blocked_back_sub_min_dim();
    assert_eq!(
        back_sub_route(conservative_dim - 1, conservative_dim - 1),
        BackSubRoute::Blocked,
        "a dimension below the conservative threshold now takes the blocked arm"
    );

    // ── The lowered values reach the dispatchers ────────────────────────────
    //
    // A window wider than the installed panel width drives the sub-panel
    // recursion on hosts whose `Fp<251>` exposes a SIMD panel lane, and the
    // recursive trsm + gemm split elsewhere; both reconstruct their input.
    let a = random_fp::<251>(96, 96, 0x9E01);
    let (p, l, e, rank) = a.ple();
    assert_eq!(l.cols(), rank);
    assert_eq!(e.rows(), rank);
    assert_eq!(p.apply(&gemm(&l, &e)), a, "P · L · E != A");

    // A matrix far below the conservative dimension takes the blocked
    // back-substitution arm and agrees with the canonical RREF oracle.
    let small = random_fp::<7>(40, 40, 0x9E02);
    assert_eq!(back_sub_route(40, 40), BackSubRoute::Blocked);
    let (x, r) = small.rref();
    assert_eq!(r, direct_rref_oracle_fp(&small));
    assert_eq!(gemm(&x, &small), r, "X · A != R");
}
