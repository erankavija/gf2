//! Installed-profile witness for `soa_batch.parallel_min_len` and
//! `soa_batch.parallel_chunk_len` (jit:fa92608b, rework round 1).
//!
//! `tuning::install` resolves the process-wide profile once, so this binary
//! installs exactly one profile, per `dev/active/7d824b2f/design.md` §4's
//! "one installed profile per test binary" rule. Both fields are set in the
//! same installed profile so one process can assert the
//! `soa_parallel_route` boundary and the chunk-length determinism witness
//! together, through the public [`BatchExtField`] dispatch entry points.

use gf2_core::compute::field::{soa_parallel_route, SoaParallelRoute};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{BatchExtField, CubicExt, ExtConfig, QuadraticExt};
use gf2_core::tuning::{self, TuningProfile};

struct CfgBeta3;
impl ExtConfig for CfgBeta3 {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
type Fq2Big = QuadraticExt<CfgBeta3>;

struct CfgCubicBeta3;
impl ExtConfig for CfgCubicBeta3 {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
type Fq3Big = CubicExt<CfgCubicBeta3>;

const INSTALLED_MIN_LEN: usize = 40;
const INSTALLED_CHUNK_LEN: usize = 7;

#[test]
fn installed_soa_batch_profile_moves_route_boundary_and_chunk_length_is_deterministic() {
    let conservative_min_len = TuningProfile::CONSERVATIVE.soa_batch().parallel_min_len();
    let conservative_chunk_len = TuningProfile::CONSERVATIVE.soa_batch().parallel_chunk_len();
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "soa-batch-route-test",
          "provenance": {"kind": "inherited"},
          "selectors": {
            "soa_batch": {"parallel_min_len": 40, "parallel_chunk_len": 7}
          }
        }
        "#,
    )
    .expect("test profile is valid");

    assert_ne!(
        INSTALLED_MIN_LEN, conservative_min_len,
        "the installed boundary must differ from the conservative default to prove the route moved"
    );
    assert_ne!(
        INSTALLED_CHUNK_LEN, conservative_chunk_len,
        "the installed chunk length must differ from the conservative default to prove the read moved"
    );
    assert_eq!(tuning::install(profile), Ok(()));

    // The witness cannot silently run against the conservative table: the
    // resolved-read accessors must report the installed values.
    let soa_batch = tuning::active().soa_batch();
    assert_eq!(soa_batch.parallel_min_len(), INSTALLED_MIN_LEN);
    assert_eq!(soa_batch.parallel_chunk_len(), INSTALLED_CHUNK_LEN);

    // Route boundary: soa_parallel_route is production dispatch code
    // (should_parallelize_soa_batch calls it), not a test-local comparison.
    assert_eq!(
        soa_parallel_route(INSTALLED_MIN_LEN - 1),
        SoaParallelRoute::Sequential
    );
    assert_eq!(soa_parallel_route(0), SoaParallelRoute::Sequential);
    assert_eq!(
        soa_parallel_route(INSTALLED_MIN_LEN),
        SoaParallelRoute::Parallel
    );
    assert_eq!(
        soa_parallel_route(INSTALLED_MIN_LEN + 1),
        SoaParallelRoute::Parallel
    );

    // Chunk-length determinism witness: lengths at/above the installed
    // parallel_min_len (so the public dispatcher's route is Parallel),
    // including lengths that do not evenly divide the installed
    // parallel_chunk_len = 7, still match direct scalar arithmetic. The
    // chunk length only repartitions the rayon work; it carries no
    // correctness meaning, per `@/inv/deterministic-seeded-execution`.
    for &len in &[
        INSTALLED_MIN_LEN,
        INSTALLED_MIN_LEN + 3,
        6 * INSTALLED_CHUNK_LEN, // an exact multiple of the installed chunk length
        3 * INSTALLED_CHUNK_LEN + INSTALLED_MIN_LEN,
    ] {
        let a2: Vec<Fq2Big> = (0..len)
            .map(|i| {
                Fq2Big::new(
                    Fp::new((17 * i as u64 + 3) % 65537),
                    Fp::new((29 * i as u64 + 5) % 65537),
                )
            })
            .collect();
        let b2: Vec<Fq2Big> = (0..len)
            .map(|i| {
                Fq2Big::new(
                    Fp::new((31 * i as u64 + 7) % 65537),
                    Fp::new((43 * i as u64 + 11) % 65537),
                )
            })
            .collect();
        let ba2 = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&a2);
        let bb2 = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&b2);
        let expected_mul2: Vec<Fq2Big> = a2.iter().zip(b2.iter()).map(|(x, y)| *x * *y).collect();
        let expected_square2: Vec<Fq2Big> = a2.iter().map(|x| *x * *x).collect();
        assert_eq!(
            ba2.batch_mul_quadratic::<CfgBeta3>(&bb2)
                .to_quadratic::<CfgBeta3>(),
            expected_mul2,
            "installed chunk_len={INSTALLED_CHUNK_LEN} quadratic mul len {len}"
        );
        assert_eq!(
            ba2.batch_square_quadratic::<CfgBeta3>()
                .to_quadratic::<CfgBeta3>(),
            expected_square2,
            "installed chunk_len={INSTALLED_CHUNK_LEN} quadratic square len {len}"
        );

        let a3: Vec<Fq3Big> = (0..len)
            .map(|i| {
                Fq3Big::new(
                    Fp::new((17 * i as u64 + 3) % 65537),
                    Fp::new((29 * i as u64 + 5) % 65537),
                    Fp::new((37 * i as u64 + 13) % 65537),
                )
            })
            .collect();
        let b3: Vec<Fq3Big> = (0..len)
            .map(|i| {
                Fq3Big::new(
                    Fp::new((31 * i as u64 + 7) % 65537),
                    Fp::new((43 * i as u64 + 11) % 65537),
                    Fp::new((47 * i as u64 + 19) % 65537),
                )
            })
            .collect();
        let ba3 = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&a3);
        let bb3 = BatchExtField::<Fp<65537>, 3>::from_cubic::<CfgCubicBeta3>(&b3);
        let expected_mul3: Vec<Fq3Big> = a3.iter().zip(b3.iter()).map(|(x, y)| *x * *y).collect();
        let expected_square3: Vec<Fq3Big> = a3.iter().map(|x| *x * *x).collect();
        assert_eq!(
            ba3.batch_mul_cubic::<CfgCubicBeta3>(&bb3)
                .to_cubic::<CfgCubicBeta3>(),
            expected_mul3,
            "installed chunk_len={INSTALLED_CHUNK_LEN} cubic mul len {len}"
        );
        assert_eq!(
            ba3.batch_square_cubic::<CfgCubicBeta3>()
                .to_cubic::<CfgCubicBeta3>(),
            expected_square3,
            "installed chunk_len={INSTALLED_CHUNK_LEN} cubic square len {len}"
        );
    }
}
