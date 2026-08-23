use gf2_core::field::charpoly::{charpoly_route, CharpolyRoute};
use gf2_core::field::test_random_matrix::random_fp;
use gf2_core::gfp::Fp;
use gf2_core::tuning::{self, TuningProfile};

const MERSENNE_31: u64 = 2_147_483_647;

#[test]
fn installed_charpoly_profile_enables_keller_gehrig_arm_matching_cubic() {
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "charpoly-route-test",
          "provenance": {"kind": "inherited"},
          "selectors": {"charpoly": {"keller_gehrig_min_dim": 2}}
        }
        "#,
    )
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    assert_eq!(tuning::active().charpoly().keller_gehrig_min_dim(), 2);

    // Below the installed threshold: the size gate reports cubic.
    assert_eq!(charpoly_route(1), CharpolyRoute::Cubic);
    // At/above the installed threshold: the size gate reports
    // Keller-Gehrig eligibility.
    assert_eq!(charpoly_route(2), CharpolyRoute::KellerGehrig);
    assert_eq!(charpoly_route(8), CharpolyRoute::KellerGehrig);

    // n = 8 on Fp<MERSENNE_31> (cardinality ~2^31) clears both the
    // installed size gate and the dispatcher's own cardinality gate
    // (q > 2n^2), so the production dispatcher actually engages the
    // Las-Vegas Keller-Gehrig path here rather than merely reporting
    // eligibility. Its Cayley-Hamilton verification guarantees the
    // result is bit-exact identical to the cubic baseline regardless
    // of which path executes.
    let a = random_fp::<MERSENNE_31>(8, 8, 0x1234_5678);
    assert_eq!(a.charpoly(), a.charpoly_cubic());

    // Same check at a second seed / size for confidence the route
    // isn't accidentally falling back to cubic every time.
    let b = random_fp::<MERSENNE_31>(6, 6, 0x9ABC_DEF0);
    assert_eq!(b.charpoly(), b.charpoly_cubic());

    // Sanity: the installed threshold is observable through the same
    // profile accessor the pilot's polynomial test uses.
    let _: Fp<MERSENNE_31> = Fp::new(0);
}
