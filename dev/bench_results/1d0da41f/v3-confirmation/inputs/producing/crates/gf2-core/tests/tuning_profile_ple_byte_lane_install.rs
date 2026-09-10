//! Byte-lane PLE panel-width route observation under an installed tuning
//! profile that narrows `ple.panel_byte_lane_max_cols`.
//!
//! The two lane widths are independent fields, so this binary installs the byte
//! lane's and observes that the u16 lane keeps its conservative width. The
//! u16-lane half lives in `tuning_profile_ple_u16_lane_install.rs`, because
//! `tuning::install` resolves once per process.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::gemm;
use gf2_core::field::ple::{
    max_effective_panel_dispatch_cols, ple_panel_route, reset_max_effective_panel_dispatch_cols,
    PlePanelRoute,
};
use gf2_core::field::test_random_matrix::random_fp;
use gf2_core::field::{FiniteField, PlePanelLane};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(
    installed_byte_lane_width_bounds_the_byte_lane_panel_window,
    {
        let text = r#"
    {
        "ple": {
          "panel_byte_lane_max_cols": 64
        }
      }"#;
        let profile = support::prepared_core_json(text).expect("test profile is valid");
        assert_eq!(tuning::install(profile), Ok(()));
        let active_tuning = tuning::active();
        let ple = active_tuning.ple();
        assert_eq!(ple.panel_byte_lane_max_cols(), 64);
        // The absent keys resolve to their conservative defaults.
        assert_eq!(
            ple.panel_u16_lane_max_cols(),
            CoreTuning::CONSERVATIVE.ple().panel_u16_lane_max_cols()
        );
        assert_eq!(
            ple.panel_base_max_cols(),
            CoreTuning::CONSERVATIVE.ple().panel_base_max_cols()
        );

        // ── Byte-lane width boundary ────────────────────────────────────────────
        //
        // The installed width sits below the panel base width, so the lane's own
        // ceiling is the arm that binds at the boundary.
        let byte = ple.panel_byte_lane_max_cols();
        let panel_base = ple.panel_base_max_cols();
        assert!(byte < panel_base);
        assert_eq!(
            ple_panel_route(Some(PlePanelLane::Byte), byte),
            PlePanelRoute::PanelBase,
            "the widest window the byte-lane kernel takes in one shot is the field value"
        );
        assert_eq!(
            ple_panel_route(Some(PlePanelLane::Byte), byte + 1),
            PlePanelRoute::RecursiveSplit,
            "a window above the byte lane's width takes the halving split, not the kernel"
        );
        assert_eq!(
            ple_panel_route(Some(PlePanelLane::Byte), panel_base + 1),
            PlePanelRoute::SubPanelRecursion,
            "the panel base width still opens the sub-panel walk above it"
        );

        // ── The u16 lane keeps its own width ────────────────────────────────────
        let u16_width = ple.panel_u16_lane_max_cols();
        assert!(u16_width > byte);
        assert_eq!(
            ple_panel_route(Some(PlePanelLane::U16), byte + 1),
            PlePanelRoute::PanelBase,
            "the window the byte lane declines still reaches the u16-lane kernel"
        );
        assert_eq!(
            ple_panel_route(Some(PlePanelLane::U16), u16_width),
            PlePanelRoute::PanelBase
        );

        // A carrier that registers no panel kernel reaches no panel arm at any
        // width, whatever the lane fields hold.
        assert_eq!(ple_panel_route(None, byte), PlePanelRoute::RecursiveSplit);
        assert_eq!(ple_panel_route(None, 1), PlePanelRoute::RecursiveSplit);

        // ── The installed width reaches the dispatcher ──────────────────────────
        //
        // `Fp<251>` registers the byte lane on a host with the `simd` feature and
        // AVX2, and no lane otherwise; the expectation follows that gate.
        let lane = <Fp<251> as FiniteField>::simd_ple_panel_lane();
        assert!(lane.is_none() || lane == Some(PlePanelLane::Byte));
        assert_eq!(
            ple_panel_route(lane, byte / 2),
            if lane.is_some() {
                PlePanelRoute::PanelBase
            } else {
                PlePanelRoute::RecursiveSplit
            }
        );
        assert_eq!(
            ple_panel_route(lane, byte + 1),
            PlePanelRoute::RecursiveSplit,
            "above the installed width the halving split takes the window on every host"
        );

        // ── The installed lane ceiling bounds EVERY panel dispatch ──────────────
        //
        // 256 columns exceeds the conservative sub-panel split width (128), so the
        // recursive splitter runs; the recorder observes the widest window the
        // panel kernel actually received. Output equality cannot show this — a
        // splitter slicing at the split width alone would still produce correct
        // results while bypassing the installed 64-column lane ceiling.
        reset_max_effective_panel_dispatch_cols();
        let a = random_fp::<251>(96, 256, 0xB702);
        let (p, l, e, rank) = a.ple();
        assert_eq!(l.cols(), rank);
        assert_eq!(e.rows(), rank);
        assert_eq!(p.apply(&gemm(&l, &e)), a, "P · L · E != A (wide)");
        match max_effective_panel_dispatch_cols() {
            Some(widest) => {
                assert!(lane.is_some(), "panel dispatch observed without a lane");
                assert!(
                    widest <= byte,
                    "panel kernel received a {widest}-column window above the installed \
                 byte-lane ceiling {byte}"
                );
            }
            None => assert!(
                lane.is_none(),
                "a byte-lane carrier must reach the panel kernel on this shape"
            ),
        }

        let a = random_fp::<251>(96, 96, 0xB701);
        let (p, l, e, rank) = a.ple();
        assert_eq!(l.cols(), rank);
        assert_eq!(e.rows(), rank);
        assert_eq!(p.apply(&gemm(&l, &e)), a, "P · L · E != A");
    }
);
