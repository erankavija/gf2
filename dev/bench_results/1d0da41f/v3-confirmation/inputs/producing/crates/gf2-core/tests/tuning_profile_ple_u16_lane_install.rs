//! u16-lane PLE panel-width route observation under an installed tuning
//! profile that narrows `ple.panel_u16_lane_max_cols`.
//!
//! The two lane widths are independent fields, so this binary installs the u16
//! lane's and observes that the byte lane keeps its conservative width. The
//! byte-lane half lives in `tuning_profile_ple_byte_lane_install.rs`, because
//! `tuning::install` resolves once per process.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::gemm;
use gf2_core::field::ple::{ple_panel_route, PlePanelRoute};
use gf2_core::field::test_random_matrix::random_fp;
use gf2_core::field::{FiniteField, PlePanelLane};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(installed_u16_lane_width_bounds_the_u16_lane_panel_window, {
    let text = r#"
    {
        "ple": {
          "panel_u16_lane_max_cols": 32
        }
      }"#;
    let profile = support::prepared_core_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let active_tuning = tuning::active();
    let ple = active_tuning.ple();
    assert_eq!(ple.panel_u16_lane_max_cols(), 32);
    // The absent keys resolve to their conservative defaults.
    assert_eq!(
        ple.panel_byte_lane_max_cols(),
        CoreTuning::CONSERVATIVE.ple().panel_byte_lane_max_cols()
    );
    assert_eq!(
        ple.panel_base_max_cols(),
        CoreTuning::CONSERVATIVE.ple().panel_base_max_cols()
    );

    // ── u16-lane width boundary ─────────────────────────────────────────────
    //
    // The installed width sits below the panel base width, so the lane's own
    // ceiling is the arm that binds at the boundary.
    let u16_width = ple.panel_u16_lane_max_cols();
    let panel_base = ple.panel_base_max_cols();
    assert!(u16_width < panel_base);
    assert_eq!(
        ple_panel_route(Some(PlePanelLane::U16), u16_width),
        PlePanelRoute::PanelBase,
        "the widest window the u16-lane kernel takes in one shot is the field value"
    );
    assert_eq!(
        ple_panel_route(Some(PlePanelLane::U16), u16_width + 1),
        PlePanelRoute::RecursiveSplit,
        "a window above the u16 lane's width takes the halving split, not the kernel"
    );
    assert_eq!(
        ple_panel_route(Some(PlePanelLane::U16), panel_base + 1),
        PlePanelRoute::SubPanelRecursion,
        "the panel base width still opens the sub-panel walk above it"
    );

    // ── The byte lane keeps its own width ───────────────────────────────────
    let byte = ple.panel_byte_lane_max_cols();
    assert!(byte > u16_width);
    assert_eq!(
        ple_panel_route(Some(PlePanelLane::Byte), u16_width + 1),
        PlePanelRoute::PanelBase,
        "the window the u16 lane declines still reaches the byte-lane kernel"
    );

    // A carrier that registers no panel kernel reaches no panel arm at any
    // width, whatever the lane fields hold.
    assert_eq!(
        ple_panel_route(None, u16_width),
        PlePanelRoute::RecursiveSplit
    );

    // ── The installed width reaches the dispatcher ──────────────────────────
    //
    // `Fp<65521>` registers the u16 lane on a host with the `simd` feature and
    // AVX2, and no lane otherwise; the expectation follows that gate.
    let lane = <Fp<65521> as FiniteField>::simd_ple_panel_lane();
    assert!(lane.is_none() || lane == Some(PlePanelLane::U16));
    assert_eq!(
        ple_panel_route(lane, u16_width / 2),
        if lane.is_some() {
            PlePanelRoute::PanelBase
        } else {
            PlePanelRoute::RecursiveSplit
        }
    );
    assert_eq!(
        ple_panel_route(lane, u16_width + 1),
        PlePanelRoute::RecursiveSplit,
        "above the installed width the halving split takes the window on every host"
    );

    let a = random_fp::<65521>(96, 96, 0xC701);
    let (p, l, e, rank) = a.ple();
    assert_eq!(l.cols(), rank);
    assert_eq!(e.rows(), rank);
    assert_eq!(p.apply(&gemm(&l, &e)), a, "P · L · E != A");
});
