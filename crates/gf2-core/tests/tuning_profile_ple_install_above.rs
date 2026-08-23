//! PLE route observation under an installed tuning profile that pushes both
//! selector values to `usize::MAX`.
//!
//! Neither field reserves a sentinel: `usize::MAX` is an ordinary admissible
//! value that disables the sub-panel recursion and the blocked
//! back-substitution for every finite operand. The below-default half of both
//! boundaries lives in `tuning_profile_ple_install.rs`.

use gf2_core::field::matrix::gemm;
use gf2_core::field::ple::{back_sub_route, ple_panel_route, BackSubRoute, PlePanelRoute};
use gf2_core::field::test_random_matrix::{direct_rref_oracle_fp, random_fp};
use gf2_core::field::PlePanelLane;
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_ple_profile_raises_both_route_boundaries_to_the_usize_ceiling() {
    let text = r#"
    {
      "schema_version": 1,
      "profile_id": "ple-route-test-above",
      "provenance": {"kind": "inherited"},
      "selectors": {
        "ple": {
          "panel_base_max_cols": 18446744073709551615,
          "blocked_back_sub_min_dim": 18446744073709551615
        }
      }
    }
    "#;
    let profile = TuningProfile::from_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let ple = tuning::active().ple();
    assert_eq!(ple.panel_base_max_cols(), usize::MAX);
    assert_eq!(ple.blocked_back_sub_min_dim(), usize::MAX);

    // ── Panel-width boundary ────────────────────────────────────────────────
    let conservative_panel = TuningProfile::CONSERVATIVE.ple().panel_base_max_cols();
    assert_eq!(
        ple_panel_route(Some(PlePanelLane::Byte), conservative_panel + 1),
        PlePanelRoute::PanelBase,
        "a window above the conservative width no longer splits into sub-panels"
    );
    // With panel_base_max_cols = usize::MAX no window takes the sub-panel
    // walk; a window this wide also exceeds the byte lane's own width, so it
    // takes the recursive split.
    assert_eq!(
        ple_panel_route(Some(PlePanelLane::Byte), usize::MAX),
        PlePanelRoute::RecursiveSplit
    );

    // ── Back-substitution boundary ──────────────────────────────────────────
    let conservative_dim = TuningProfile::CONSERVATIVE.ple().blocked_back_sub_min_dim();
    assert_eq!(
        back_sub_route(conservative_dim, conservative_dim),
        BackSubRoute::Scalar,
        "the conservative dimension no longer reaches the blocked arm"
    );
    assert_eq!(back_sub_route(usize::MAX - 1, 1), BackSubRoute::Scalar);
    assert_eq!(
        back_sub_route(usize::MAX, usize::MAX),
        BackSubRoute::Blocked,
        "the ceiling is an admissible value, not a reserved sentinel"
    );

    // ── The raised values reach the dispatchers ─────────────────────────────
    //
    // A window wider than the conservative panel width takes the panel-base
    // and recursive-split arms instead of the sub-panel recursion.
    let a = random_fp::<251>(300, 300, 0x9F01);
    let (p, l, e, rank) = a.ple();
    assert_eq!(l.cols(), rank);
    assert_eq!(e.rows(), rank);
    assert_eq!(p.apply(&gemm(&l, &e)), a, "P · L · E != A");

    // A matrix at the conservative dimension takes the scalar
    // back-substitution loop and agrees with the canonical RREF oracle.
    let large = random_fp::<7>(conservative_dim, conservative_dim, 0x9F02);
    assert_eq!(
        back_sub_route(conservative_dim, conservative_dim),
        BackSubRoute::Scalar
    );
    let (x, r) = large.rref();
    assert_eq!(r, direct_rref_oracle_fp(&large));
    assert_eq!(gemm(&x, &large), r, "X · A != R");
}
