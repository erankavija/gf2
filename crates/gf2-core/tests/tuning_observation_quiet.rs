//! Observation-free specialization of the complete solve subtree.

#![cfg(feature = "test-support")]

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::{
    gemm_tile_observations, last_gemm_axpy_dispatch_route, reset_gemm_tile_observations,
    reset_last_gemm_axpy_dispatch_route, FieldMatrix, GemmAxpyRoute, GemmTileSite,
};
use gf2_core::field::ple::{
    max_effective_panel_dispatch_cols, reset_max_effective_panel_dispatch_cols,
};
use gf2_core::field::triangular::{
    last_effective_triangular_route, last_effective_trsm_panel_rows,
    reset_last_effective_triangular_route, reset_last_effective_trsm_panel_rows, TriangularRoute,
};
use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;
use gf2_core::tuning;

fn fixture(n: usize) -> FieldMatrix<Fp<251>> {
    let mut matrix = FieldMatrix::zeros(n, n);
    for row in 0..n {
        for col in 0..n {
            matrix.set(row, col, Fp::<251>::new((7 + 13 * row + 19 * col) as u64));
        }
    }
    matrix
}

support::fresh_tuning_test!(quiet_solve_matches_recorded_blocked_solve, {
    let profile = support::prepared_core_json(
        r#"{"triangular":{"trsm_blocked_min_dim":0,"trsm_panel_rows":8,"base_case_max_dim":2}}"#,
    )
    .expect("blocked solve profile is valid");
    tuning::install(profile).expect("fresh process has no installed profile");

    let a = FieldMatrix::<Fp<251>>::identity(17);
    let b = fixture(17);

    reset_gemm_tile_observations();
    reset_last_gemm_axpy_dispatch_route();
    reset_last_effective_triangular_route();
    reset_last_effective_trsm_panel_rows();
    reset_max_effective_panel_dispatch_cols();
    let recorded = a
        .solve_batch(&b)
        .expect("identity coefficient matrix is invertible");

    assert_eq!(
        last_effective_triangular_route(),
        Some((2, TriangularRoute::Recursive)),
        "the recorded solve must publish its completed top-level triangular route"
    );
    if <Fp<251> as FiniteField>::has_simd_gemm_classical() {
        assert_eq!(
            last_effective_trsm_panel_rows(),
            Some(8),
            "the capable recorded solve must publish the installed blocked panel width"
        );
    } else {
        assert_eq!(
            last_effective_trsm_panel_rows(),
            None,
            "the fallback recorded solve must stay on recursive TRSM"
        );
    }
    if <Fp<251> as FiniteField>::simd_ple_panel_lane().is_some() {
        assert_eq!(
            max_effective_panel_dispatch_cols(),
            Some(17),
            "the capable recorded solve must publish its full-width PLE panel"
        );
    } else {
        assert_eq!(
            max_effective_panel_dispatch_cols(),
            None,
            "the fallback recorded solve has no PLE panel dispatch to publish"
        );
    }
    assert_eq!(
        last_gemm_axpy_dispatch_route(),
        Some(GemmAxpyRoute::PerCell),
        "the recorded solve's sub-threshold updates must publish their effective fallback route"
    );
    assert!(
        gemm_tile_observations()
            .iter()
            .any(|observation| observation.site == GemmTileSite::MatrixGemmAxpyIntoView),
        "the recorded solve must publish the tile consumed by its GEMM updates"
    );

    reset_gemm_tile_observations();
    reset_last_gemm_axpy_dispatch_route();
    reset_last_effective_triangular_route();
    reset_last_effective_trsm_panel_rows();
    reset_max_effective_panel_dispatch_cols();
    let quiet = a
        .solve_batch_quiet_for_test(&b)
        .expect("quiet specialization preserves invertibility");

    assert_eq!(quiet, recorded);
    assert!(gemm_tile_observations().is_empty());
    assert_eq!(last_gemm_axpy_dispatch_route(), None);
    assert_eq!(last_effective_triangular_route(), None);
    assert_eq!(last_effective_trsm_panel_rows(), None);
    assert_eq!(max_effective_panel_dispatch_cols(), None);
});
