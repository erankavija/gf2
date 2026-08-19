use gf2_core::field::poly::{
    batch_evaluate_subproduct_auto, mul_fast, FieldPoly, DIV_REM_THRESHOLD, KARATSUBA_THRESHOLD,
    NTT_THRESHOLD, SUBPRODUCT_THRESHOLD,
};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_polynomial_profile_moves_routes_and_keeps_schoolbook_fallback_reachable() {
    let text = r#"
    {
      "schema_version": 1,
      "profile_id": "polynomial-route-test",
      "provenance": {"kind": "inherited"},
      "selectors": {
        "polynomial": {
          "karatsuba_min_degree": 33,
          "karatsuba_max_out_len": 0,
          "div_rem_fast_min_len": 1,
          "subproduct_min_len": 1
        }
      }
    }
    "#;
    let profile = TuningProfile::from_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let polynomial = tuning::active().polynomial();
    assert_eq!(polynomial.karatsuba_min_degree(), 33);
    assert_eq!(polynomial.karatsuba_max_out_len(), 0);
    assert_eq!(polynomial.div_rem_fast_min_len(), 1);
    assert_eq!(polynomial.subproduct_min_len(), 1);

    // With the conservative default, degree 32 takes Karatsuba. The
    // installed minimum degree of 33 moves this same input to schoolbook.
    let lhs = FieldPoly::new(vec![Fp::<65537>::new(1); KARATSUBA_THRESHOLD + 1]);
    let rhs = FieldPoly::new(vec![Fp::<65537>::new(2); KARATSUBA_THRESHOLD + 1]);
    assert!(lhs.len() > KARATSUBA_THRESHOLD, "default route: Karatsuba");
    assert!(
        lhs.len() - 1 < polynomial.karatsuba_min_degree(),
        "installed route: schoolbook fallback"
    );
    assert_eq!(lhs.mul(&rhs).len(), lhs.len() + rhs.len() - 1);

    // With the conservative default, output length 33 is at or below 128 and
    // takes the Karatsuba/schoolbook dispatcher. A zero ceiling moves it to
    // the NTT arm.
    let ntt_lhs = FieldPoly::new(vec![Fp::<65537>::new(3); 17]);
    let ntt_rhs = FieldPoly::new(vec![Fp::<65537>::new(4); 17]);
    let out_len = ntt_lhs.len() + ntt_rhs.len() - 1;
    assert!(out_len <= NTT_THRESHOLD, "default route: Karatsuba");
    assert!(
        out_len > polynomial.karatsuba_max_out_len(),
        "installed route: NTT"
    );
    assert_eq!(mul_fast(&ntt_lhs, &ntt_rhs), ntt_lhs.mul_ntt(&ntt_rhs));

    // With the conservative default, two short operands take schoolbook
    // division. A minimum length of one moves the same input to fast division.
    let dividend = FieldPoly::new(vec![Fp::<65537>::new(1); 2]);
    let divisor = FieldPoly::new(vec![Fp::<65537>::new(1); 2]);
    assert!(
        dividend.len() < DIV_REM_THRESHOLD,
        "default route: schoolbook"
    );
    assert!(
        dividend.len() >= polynomial.div_rem_fast_min_len(),
        "installed route: Newton fast division"
    );
    assert_eq!(
        dividend.div_rem_auto(&divisor),
        dividend.div_rem_fast(&divisor)
    );

    // With the conservative default, a one-point evaluation takes Horner.
    // A minimum length of one moves the same input to the subproduct tree.
    let evaluation_poly = FieldPoly::new(vec![Fp::<65537>::new(7)]);
    let points = [Fp::<65537>::new(5)];
    assert!(points.len() < SUBPRODUCT_THRESHOLD, "default route: Horner");
    assert!(
        points.len() >= polynomial.subproduct_min_len(),
        "installed route: subproduct tree"
    );
    assert_eq!(
        evaluation_poly.batch_evaluate_auto(&points),
        batch_evaluate_subproduct_auto(&evaluation_poly, &points)
    );
}
