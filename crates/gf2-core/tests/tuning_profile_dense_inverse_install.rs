use gf2_core::alg::gauss::{invert, invert_m4ri, invert_route, invert_scalar, InvertRoute};
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::inverse::{inv_route, InvRoute};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gfp::Fp;
use gf2_core::matrix::BitMatrix;
use gf2_core::tuning;

// `install` is one-shot per process, so both boundaries share one
// child-process install.
support::fresh_tuning_test!(installed_dense_inverse_profile_moves_both_routes, {
    let text = r#"
    {
        "dense_inverse": {
          "m4ri_min_dim": 9,
          "blocked_min_dim": 17
        }
      }"#;
    let profile = support::prepared_core_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let active_tuning = tuning::active();
    let dense_inverse = active_tuning.dense_inverse();
    assert_eq!(dense_inverse.m4ri_min_dim(), 9);
    assert_eq!(dense_inverse.blocked_min_dim(), 17);

    let m4ri_min_dim = dense_inverse.m4ri_min_dim();
    let below_m4ri = m4ri_min_dim - 1;
    assert_eq!(invert_route(below_m4ri), InvertRoute::Scalar);
    assert_eq!(invert_route(m4ri_min_dim), InvertRoute::M4ri);

    // The installed threshold is above the conservative one, so this size
    // takes the scalar arm only under the installed profile.
    let below_m4ri_id = BitMatrix::identity(below_m4ri);
    assert_eq!(
        invert(&below_m4ri_id).unwrap(),
        invert_scalar(&below_m4ri_id).unwrap(),
        "n={below_m4ri} should dispatch to the scalar arm under the installed profile"
    );

    let at_m4ri_id = BitMatrix::identity(m4ri_min_dim);
    assert_eq!(
        invert(&at_m4ri_id).unwrap(),
        invert_m4ri(&at_m4ri_id).unwrap(),
        "n={m4ri_min_dim} should dispatch to the M4RM arm under the installed profile"
    );

    let blocked_min_dim = dense_inverse.blocked_min_dim();
    let below_blocked = blocked_min_dim - 1;
    assert_eq!(inv_route(below_blocked), InvRoute::ScalarPle);
    assert_eq!(inv_route(blocked_min_dim), InvRoute::BlockedPanelized);

    // The installed threshold is above the conservative one. Both arms run
    // through the one public `inv` entry point, so correctness is checked by
    // the round trip A · A⁻¹ = I.
    let below_blocked_id = FieldMatrix::<Fp<65537>>::identity(below_blocked);
    let below_blocked_inv = below_blocked_id.inv().expect("identity is invertible");
    assert_eq!(
        gemm(&below_blocked_id, &below_blocked_inv),
        FieldMatrix::<Fp<65537>>::identity(below_blocked),
        "n={below_blocked} scalar-pivot arm should produce a correct inverse"
    );

    let at_blocked_id = FieldMatrix::<Fp<65537>>::identity(blocked_min_dim);
    let at_blocked_inv = at_blocked_id.inv().expect("identity is invertible");
    assert_eq!(
        gemm(&at_blocked_id, &at_blocked_inv),
        FieldMatrix::<Fp<65537>>::identity(blocked_min_dim),
        "n={blocked_min_dim} blocked-panelized arm should produce a correct inverse"
    );
});
