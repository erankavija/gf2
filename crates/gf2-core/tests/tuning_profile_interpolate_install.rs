use gf2_core::field::poly_interpolate::{
    interpolate, interpolate_auto, interpolate_auto_two_adic, interpolate_fast,
    interpolate_fast_auto, interpolate_route, InterpolateRoute,
};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, TuningProfile};

type FP = Fp<65537>;

fn points(n: usize) -> Vec<(FP, FP)> {
    (0..n as u64)
        .map(|i| (FP::new(i + 1), FP::new(((i * 7) % 65537) + 1)))
        .collect()
}

#[test]
fn installed_interpolate_profile_moves_both_entry_points() {
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "interpolate-route-test",
          "provenance": {"kind": "inherited"},
          "selectors": {"polynomial": {"interpolate_fast_min_points": 3}}
        }
        "#,
    )
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let threshold = tuning::active().polynomial().interpolate_fast_min_points();
    assert_eq!(threshold, 3);

    assert_eq!(
        interpolate_route(threshold - 1),
        InterpolateRoute::Barycentric
    );
    assert_eq!(
        interpolate_route(threshold),
        InterpolateRoute::SubproductTree
    );

    let below = points(threshold - 1);
    let at = points(threshold);

    // `interpolate_auto` follows the moved boundary.
    assert_eq!(
        interpolate_auto(&below).unwrap(),
        interpolate(&below).unwrap()
    );
    assert_eq!(
        interpolate_auto(&at).unwrap(),
        interpolate_fast(&at).unwrap()
    );

    // `interpolate_auto_two_adic` follows the same moved boundary.
    assert_eq!(
        interpolate_auto_two_adic(&below).unwrap(),
        interpolate(&below).unwrap()
    );
    assert_eq!(
        interpolate_auto_two_adic(&at).unwrap(),
        interpolate_fast_auto(&at).unwrap()
    );
}
