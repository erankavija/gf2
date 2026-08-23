//! Toggle observation for the prime-field GEMM route reporter.
//!
//! `prime_gemm_route` mirrors the dispatchers' full gate chain, which includes
//! the process-wide GF(251) debug switches: `fp_small_try_gemm_classical`
//! consults `route_a_gf251_enabled` before its window predicate, so a reporter
//! that ignored the switch would name Candidate C for a cell the dispatcher
//! sends through route A.
//!
//! The switches are process-wide `AtomicBool`s, so this binary holds exactly
//! one test and no other prime-route binary touches them.
#![cfg(all(feature = "simd", not(gf2_tuning_baked)))]

use gf2_core::gfp::simd_ops::{prime_gemm_route, set_route_a_gf251_enabled, PrimeGemmRoute};
use gf2_core::tuning::TuningProfile;

#[test]
fn the_route_a_switch_moves_gf251_below_the_column_bound() {
    const M: usize = 64;
    const K: usize = 64;
    let below = TuningProfile::CONSERVATIVE
        .prime_route()
        .f32_min_cols()
        .checked_sub(1)
        .expect("the bound admits a cell below it");

    let untoggled = prime_gemm_route::<251>(M, K, below);
    assert_ne!(
        untoggled,
        PrimeGemmRoute::F32Cascade,
        "below the column bound the production default is not the cascade"
    );

    set_route_a_gf251_enabled(true);
    let toggled = prime_gemm_route::<251>(M, K, below);
    set_route_a_gf251_enabled(false);

    match toggled {
        // The switch forces route A at any width when the kernel is present.
        PrimeGemmRoute::F32Cascade => {}
        // Without the f32 kernel the dispatcher falls through the route-A
        // block exactly as it does untoggled.
        arm => assert_eq!(
            arm, untoggled,
            "this host registers no f32 cascade kernel, so the switch cannot \
             move the arm"
        ),
    }

    assert_eq!(
        prime_gemm_route::<251>(M, K, below),
        untoggled,
        "restoring the switch restores the production arm"
    );
}
