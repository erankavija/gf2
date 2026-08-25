//! Triangular base-case route observation below the conservative boundary.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::triangular::{
    last_effective_triangular_route, reset_last_effective_triangular_route, triangular_route,
    trmm_upper, trsm_upper, trtri_upper, TriangularRoute,
};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, CoreTuning};

type F = Fp<7>;

fn assert_trsm_observes(m: usize, threshold: usize, expected: TriangularRoute) {
    reset_last_effective_triangular_route();
    let a = FieldMatrix::<F>::identity(m);
    let mut b = FieldMatrix::<F>::identity(m);
    trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
    assert_eq!(
        last_effective_triangular_route(),
        Some((threshold, expected))
    );
}

fn assert_trmm_observes(m: usize, threshold: usize, expected: TriangularRoute) {
    reset_last_effective_triangular_route();
    let a = FieldMatrix::<F>::identity(m);
    let mut b = FieldMatrix::<F>::identity(m);
    trmm_upper(a.submat(.., ..), b.submat_mut(.., ..));
    assert_eq!(
        last_effective_triangular_route(),
        Some((threshold, expected))
    );
}

fn assert_trtri_observes(m: usize, threshold: usize, expected: TriangularRoute) {
    reset_last_effective_triangular_route();
    let mut a = FieldMatrix::<F>::identity(m);
    trtri_upper(a.submat_mut(.., ..));
    assert_eq!(
        last_effective_triangular_route(),
        Some((threshold, expected))
    );
}

support::fresh_tuning_test!(
    installed_triangular_profile_lowers_base_case_boundary_in_production_entries,
    {
        let profile = support::prepared_core_json(
            r#"
        {"triangular": {"base_case_max_dim": 2}}"#,
        )
        .expect("test profile is valid");
        assert_eq!(tuning::install(profile), Ok(()));

        let threshold = tuning::active().triangular().base_case_max_dim();
        assert_eq!(threshold, 2);
        assert_eq!(triangular_route(threshold), TriangularRoute::BaseCase);
        assert_eq!(triangular_route(threshold + 1), TriangularRoute::Recursive);
        assert_eq!(
            triangular_route(CoreTuning::CONSERVATIVE.triangular().base_case_max_dim()),
            TriangularRoute::Recursive,
            "the installed threshold must move the conservative boundary"
        );

        assert_trsm_observes(threshold, threshold, TriangularRoute::BaseCase);
        assert_trsm_observes(threshold + 1, threshold, TriangularRoute::Recursive);
        assert_trmm_observes(threshold, threshold, TriangularRoute::BaseCase);
        assert_trmm_observes(threshold + 1, threshold, TriangularRoute::Recursive);
        assert_trtri_observes(threshold, threshold, TriangularRoute::BaseCase);
        assert_trtri_observes(threshold + 1, threshold, TriangularRoute::Recursive);
    }
);
