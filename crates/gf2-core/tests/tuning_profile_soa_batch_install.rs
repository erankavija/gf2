//! Installed-profile witness for `soa_batch.parallel_min_len` and
//! `soa_batch.parallel_chunk_len` through the public [`BatchExtField`]
//! dispatch entry points. `tuning::install` resolves once per process, so
//! this binary installs one profile carrying both fields. The parallel and
//! scalar arms return equal output for every valid chunk length, so the
//! binary reads [`last_effective_soa_chunk`], which the parallel arm of each
//! entry point records, and confirms that a scalar-arm call records nothing.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::compute::field::{
    last_effective_soa_chunk, reset_last_effective_soa_chunk, run_in_dedicated_parallel_pool,
    soa_parallel_route, SoaParallelRoute,
};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{BatchExtField, CubicExt, ExtConfig, QuadraticExt};
use gf2_core::tuning::{self, CoreTuning};

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

support::fresh_tuning_test!(
    installed_soa_batch_profile_moves_route_boundary_and_chunk_length_is_deterministic,
    {
        let conservative_min_len = CoreTuning::CONSERVATIVE.soa_batch().parallel_min_len();
        let conservative_chunk_len = CoreTuning::CONSERVATIVE.soa_batch().parallel_chunk_len();
        let profile = support::prepared_core_json(
            r#"
        {
            "soa_batch": {"parallel_min_len": 40, "parallel_chunk_len": 7}
          }"#,
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
        let active_tuning = tuning::active();
        let soa_batch = active_tuning.soa_batch();
        assert_eq!(soa_batch.parallel_min_len(), INSTALLED_MIN_LEN);
        assert_eq!(soa_batch.parallel_chunk_len(), INSTALLED_CHUNK_LEN);

        // Route boundary: soa_parallel_route delegates to the same private
        // resolved reporter the dispatcher
        // (should_parallelize_soa_batch_resolved) consumes, so this asserts
        // production dispatch code, not a test-local comparison.
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

        // The scalar arm (below parallel_min_len) records nothing: proves the
        // hook only fires on the parallel path, so a `Some` result below can't
        // be a stale value from an earlier call.
        reset_last_effective_soa_chunk();
        assert_eq!(last_effective_soa_chunk(), None);
        let scalar_len = INSTALLED_MIN_LEN - 1;
        let scalar_a: Vec<Fq2Big> = (0..scalar_len)
            .map(|i| {
                Fq2Big::new(
                    Fp::new((17 * i as u64 + 3) % 65537),
                    Fp::new((29 * i as u64 + 5) % 65537),
                )
            })
            .collect();
        let scalar_ba = BatchExtField::<Fp<65537>, 2>::from_quadratic::<CfgBeta3>(&scalar_a);
        let _ = scalar_ba.batch_square_quadratic::<CfgBeta3>();
        assert_eq!(
        last_effective_soa_chunk(),
        None,
        "a scalar-arm call (len {scalar_len} < parallel_min_len {INSTALLED_MIN_LEN}) must not record a chunk"
    );

        // Lengths at or above the installed `parallel_min_len`, including
        // ones the installed chunk length does not divide, match direct
        // scalar arithmetic. Each call is bracketed by a reset and a
        // `last_effective_soa_chunk` check, so the observation is that
        // call's own. The loop runs inside a dedicated two-thread pool, which
        // satisfies the dispatcher's thread gate
        // (`rayon::current_num_threads() > 1`).
        run_in_dedicated_parallel_pool(2, || {
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
                let expected_mul2: Vec<Fq2Big> =
                    a2.iter().zip(b2.iter()).map(|(x, y)| *x * *y).collect();
                let expected_square2: Vec<Fq2Big> = a2.iter().map(|x| *x * *x).collect();

                reset_last_effective_soa_chunk();
                let got_mul2 = ba2
                    .batch_mul_quadratic::<CfgBeta3>(&bb2)
                    .to_quadratic::<CfgBeta3>();
                assert_eq!(
                last_effective_soa_chunk(),
                Some(INSTALLED_CHUNK_LEN),
                "quadratic mul len {len} must run its parallel arm and consume the installed chunk length"
            );
                assert_eq!(
                    got_mul2, expected_mul2,
                    "installed chunk_len={INSTALLED_CHUNK_LEN} quadratic mul len {len}"
                );

                reset_last_effective_soa_chunk();
                let got_square2 = ba2
                    .batch_square_quadratic::<CfgBeta3>()
                    .to_quadratic::<CfgBeta3>();
                assert_eq!(
                last_effective_soa_chunk(),
                Some(INSTALLED_CHUNK_LEN),
                "quadratic square len {len} must run its parallel arm and consume the installed chunk length"
            );
                assert_eq!(
                    got_square2, expected_square2,
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
                let expected_mul3: Vec<Fq3Big> =
                    a3.iter().zip(b3.iter()).map(|(x, y)| *x * *y).collect();
                let expected_square3: Vec<Fq3Big> = a3.iter().map(|x| *x * *x).collect();

                reset_last_effective_soa_chunk();
                let got_mul3 = ba3
                    .batch_mul_cubic::<CfgCubicBeta3>(&bb3)
                    .to_cubic::<CfgCubicBeta3>();
                assert_eq!(
                last_effective_soa_chunk(),
                Some(INSTALLED_CHUNK_LEN),
                "cubic mul len {len} must run its parallel arm and consume the installed chunk length"
            );
                assert_eq!(
                    got_mul3, expected_mul3,
                    "installed chunk_len={INSTALLED_CHUNK_LEN} cubic mul len {len}"
                );

                reset_last_effective_soa_chunk();
                let got_square3 = ba3
                    .batch_square_cubic::<CfgCubicBeta3>()
                    .to_cubic::<CfgCubicBeta3>();
                assert_eq!(
                last_effective_soa_chunk(),
                Some(INSTALLED_CHUNK_LEN),
                "cubic square len {len} must run its parallel arm and consume the installed chunk length"
            );
                assert_eq!(
                    got_square3, expected_square3,
                    "installed chunk_len={INSTALLED_CHUNK_LEN} cubic square len {len}"
                );
            }
        });
    }
);
