use gf2_core::field::poly::{
    batch_evaluate_auto_route, batch_evaluate_route, div_rem_auto_route, mul_fast_route, mul_route,
    BatchEvaluateRoute, DivRemAutoRoute, MulFastRoute, MulRoute, DIV_REM_THRESHOLD,
    KARATSUBA_THRESHOLD, NTT_THRESHOLD, SUBPRODUCT_THRESHOLD,
};
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(
    conservative_profile_resolves_once_and_rejects_late_install,
    {
        assert_eq!(tuning::active().section, &CoreTuning::CONSERVATIVE);
        let active_tuning = tuning::active();
        let polynomial = active_tuning.polynomial();
        assert_eq!(
            polynomial.karatsuba_min_degree(),
            KARATSUBA_THRESHOLD,
            "karatsuba_min_degree uses the conservative default"
        );
        assert_eq!(
            polynomial.karatsuba_max_out_len(),
            NTT_THRESHOLD,
            "karatsuba_max_out_len uses the conservative default"
        );
        assert_eq!(
            polynomial.div_rem_fast_min_len(),
            DIV_REM_THRESHOLD,
            "div_rem_fast_min_len uses the conservative default"
        );
        assert_eq!(
            polynomial.subproduct_min_len(),
            SUBPRODUCT_THRESHOLD,
            "subproduct_min_len uses the conservative default"
        );

        assert_eq!(
            mul_route(KARATSUBA_THRESHOLD - 1, KARATSUBA_THRESHOLD - 1),
            MulRoute::Schoolbook
        );
        assert_eq!(
            mul_route(KARATSUBA_THRESHOLD, KARATSUBA_THRESHOLD),
            MulRoute::Karatsuba
        );
        assert_eq!(mul_fast_route(NTT_THRESHOLD), MulFastRoute::Karatsuba);
        assert_eq!(mul_fast_route(NTT_THRESHOLD + 1), MulFastRoute::Ntt);
        assert_eq!(
            div_rem_auto_route(DIV_REM_THRESHOLD - 1, DIV_REM_THRESHOLD - 1),
            DivRemAutoRoute::Schoolbook
        );
        assert_eq!(
            div_rem_auto_route(DIV_REM_THRESHOLD, DIV_REM_THRESHOLD),
            DivRemAutoRoute::Fast
        );
        assert_eq!(
            batch_evaluate_route(SUBPRODUCT_THRESHOLD - 1, SUBPRODUCT_THRESHOLD - 1),
            BatchEvaluateRoute::Horner
        );
        assert_eq!(
            batch_evaluate_route(SUBPRODUCT_THRESHOLD, SUBPRODUCT_THRESHOLD),
            BatchEvaluateRoute::SubproductTree
        );
        assert_eq!(
            batch_evaluate_auto_route(SUBPRODUCT_THRESHOLD - 1, SUBPRODUCT_THRESHOLD - 1),
            BatchEvaluateRoute::Horner
        );
        assert_eq!(
            batch_evaluate_auto_route(SUBPRODUCT_THRESHOLD, SUBPRODUCT_THRESHOLD),
            BatchEvaluateRoute::SubproductTree
        );

        let first = tuning::active();
        let second = tuning::active();
        assert_eq!(first.section, second.section);
        assert_eq!(first.resolution, second.resolution);
        assert!(tuning::install(support::prepared_core(CoreTuning::CONSERVATIVE)).is_err());
    }
);
