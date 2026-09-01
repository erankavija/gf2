#![cfg(all(
    feature = "tuning-profile",
    feature = "test-support",
    not(feature = "simd")
))]

use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::ple::{
    max_effective_panel_dispatch_cols, ple_panel_route, reset_max_effective_panel_dispatch_cols,
    PlePanelRoute,
};
use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;

#[test]
fn unavailable_panel_kernel_does_not_count_as_effective_execution() {
    type Fp251 = Fp<251>;

    let lane = <Fp251 as FiniteField>::simd_ple_panel_lane();
    assert_eq!(lane, None, "the no-SIMD build must expose no panel lane");
    assert_eq!(
        ple_panel_route(lane, 16),
        PlePanelRoute::RecursiveSplit,
        "an unavailable carrier must not claim the panel route"
    );

    let matrix = FieldMatrix::<Fp251>::identity(16);
    reset_max_effective_panel_dispatch_cols();
    let (_, _, _, rank) = matrix.ple();
    assert_eq!(rank, 16);
    assert_eq!(
        max_effective_panel_dispatch_cols(),
        None,
        "only a successful SIMD panel-kernel return may count as execution"
    );
}
