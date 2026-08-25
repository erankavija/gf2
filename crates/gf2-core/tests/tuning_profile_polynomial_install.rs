use gf2_core::field::poly::{
    batch_evaluate_auto_route, batch_evaluate_route, batch_evaluate_subproduct,
    batch_evaluate_subproduct_auto, div_rem_auto_route, mul_fast, mul_fast_route, mul_route,
    BatchEvaluateRoute, DivRemAutoRoute, FieldPoly, MulFastRoute, MulRoute,
};
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::gfp::Fp;
use gf2_core::tuning;

support::fresh_tuning_test!(
    installed_polynomial_profile_moves_routes_and_keeps_schoolbook_fallback_reachable,
    {
        let text = r#"
    {
        "polynomial": {
          "karatsuba_min_degree": 33,
          "karatsuba_max_out_len": 127,
          "div_rem_fast_min_len": 1,
          "subproduct_min_len": 1
        }
      }"#;
        let profile = support::prepared_core_json(text).expect("test profile is valid");
        assert_eq!(tuning::install(profile), Ok(()));
        let active_tuning = tuning::active();
        let polynomial = active_tuning.polynomial();
        assert_eq!(polynomial.karatsuba_min_degree(), 33);
        assert_eq!(polynomial.karatsuba_max_out_len(), 127);
        assert_eq!(polynomial.div_rem_fast_min_len(), 1);
        assert_eq!(polynomial.subproduct_min_len(), 1);

        let karatsuba_min_degree = polynomial.karatsuba_min_degree();
        let below_karatsuba_degree = karatsuba_min_degree - 1;
        assert_eq!(
            mul_route(below_karatsuba_degree, below_karatsuba_degree),
            MulRoute::Schoolbook
        );
        assert_eq!(
            mul_route(karatsuba_min_degree, karatsuba_min_degree),
            MulRoute::Karatsuba
        );
        let lhs = FieldPoly::new(vec![Fp::<65537>::new(1); below_karatsuba_degree + 1]);
        let rhs = FieldPoly::new(vec![Fp::<65537>::new(2); below_karatsuba_degree + 1]);
        assert_eq!(lhs.mul(&rhs).len(), lhs.len() + rhs.len() - 1);

        let karatsuba_max_out_len = polynomial.karatsuba_max_out_len();
        assert_eq!(
            mul_fast_route(karatsuba_max_out_len),
            MulFastRoute::Karatsuba
        );
        assert_eq!(mul_fast_route(karatsuba_max_out_len + 1), MulFastRoute::Ntt);
        let ntt_lhs = FieldPoly::new(vec![Fp::<65537>::new(3); 1]);
        let ntt_rhs = FieldPoly::new(vec![Fp::<65537>::new(4); karatsuba_max_out_len]);
        let ntt_lhs_above = FieldPoly::new(vec![Fp::<65537>::new(5); 1]);
        let ntt_rhs_above = FieldPoly::new(vec![Fp::<65537>::new(6); karatsuba_max_out_len + 1]);
        assert_eq!(mul_fast(&ntt_lhs, &ntt_rhs), ntt_lhs.mul(&ntt_rhs));
        assert_eq!(
            mul_fast(&ntt_lhs_above, &ntt_rhs_above),
            ntt_lhs_above.mul_ntt(&ntt_rhs_above)
        );

        let div_rem_fast_min_len = polynomial.div_rem_fast_min_len();
        assert_eq!(
            div_rem_auto_route(div_rem_fast_min_len - 1, div_rem_fast_min_len - 1),
            DivRemAutoRoute::Schoolbook
        );
        assert_eq!(
            div_rem_auto_route(div_rem_fast_min_len, div_rem_fast_min_len),
            DivRemAutoRoute::Fast
        );
        let dividend = FieldPoly::new(vec![Fp::<65537>::new(1); div_rem_fast_min_len + 1]);
        let divisor = FieldPoly::new(vec![Fp::<65537>::new(1); div_rem_fast_min_len + 1]);
        assert_eq!(
            dividend.div_rem_auto(&divisor),
            dividend.div_rem_fast(&divisor)
        );

        let subproduct_min_len = polynomial.subproduct_min_len();
        let below_subproduct_len = subproduct_min_len - 1;
        assert_eq!(
            batch_evaluate_route(below_subproduct_len, below_subproduct_len),
            BatchEvaluateRoute::Horner
        );
        assert_eq!(
            batch_evaluate_route(subproduct_min_len, subproduct_min_len),
            BatchEvaluateRoute::SubproductTree
        );
        assert_eq!(
            batch_evaluate_auto_route(below_subproduct_len, below_subproduct_len),
            BatchEvaluateRoute::Horner
        );
        assert_eq!(
            batch_evaluate_auto_route(subproduct_min_len, subproduct_min_len),
            BatchEvaluateRoute::SubproductTree
        );
        let evaluation_poly = FieldPoly::new(vec![Fp::<65537>::new(7); subproduct_min_len]);
        let points = vec![Fp::<65537>::new(5); subproduct_min_len];
        assert_eq!(
            evaluation_poly.batch_evaluate(&points),
            batch_evaluate_subproduct(&evaluation_poly, &points)
        );
        assert_eq!(
            evaluation_poly.batch_evaluate_auto(&points),
            batch_evaluate_subproduct_auto(&evaluation_poly, &points)
        );
    }
);
