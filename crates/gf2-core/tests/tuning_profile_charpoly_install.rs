use gf2_core::field::charpoly::{charpoly_route, CharpolyRoute};
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::test_random_matrix::random_fp;
use gf2_core::gfp::Fp;
use gf2_core::tuning;

const MERSENNE_31: u64 = 2_147_483_647;

support::fresh_tuning_test!(
    installed_charpoly_profile_enables_keller_gehrig_arm_matching_cubic,
    {
        let profile = support::prepared_core_json(
            r#"
        {"charpoly": {"keller_gehrig_min_dim": 2}}"#,
        )
        .expect("test profile is valid");
        assert_eq!(tuning::install(profile), Ok(()));
        assert_eq!(tuning::active().charpoly().keller_gehrig_min_dim(), 2);

        assert_eq!(charpoly_route(1), CharpolyRoute::Cubic);
        assert_eq!(charpoly_route(2), CharpolyRoute::KellerGehrig);
        assert_eq!(charpoly_route(8), CharpolyRoute::KellerGehrig);

        // n = 8 on Fp<MERSENNE_31> clears both the installed size gate and
        // the dispatcher's cardinality gate (q > 2n²), so the Keller-Gehrig
        // path runs.
        let a = random_fp::<MERSENNE_31>(8, 8, 0x1234_5678);
        assert_eq!(a.charpoly(), a.charpoly_cubic());

        let b = random_fp::<MERSENNE_31>(6, 6, 0x9ABC_DEF0);
        assert_eq!(b.charpoly(), b.charpoly_cubic());

        let _: Fp<MERSENNE_31> = Fp::new(0);
    }
);
