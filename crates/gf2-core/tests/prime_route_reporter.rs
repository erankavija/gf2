//! Route observation for the prime-field GEMM dispatchers in the default
//! build.
//!
//! `prime_gemm_route` walks the dispatchers' own gate chain — eligibility
//! window, shape guard, `select_f32_path` / `select_f64_path`, the GF(251)
//! toggles, and the kernel-registration lookups — so these assertions observe
//! the production selection rather than a copy of it. The `prime_route` family
//! is baked (`dev/active/7d824b2f/design.md` §3.11), so the default build's
//! boundaries are the conservative table's values and no installed profile
//! moves them; `prime_route_baked.rs` witnesses the committed calibrated
//! boundaries under `--cfg gf2_tuning_baked`.
//!
//! Whether a cascade kernel is registered is a host property, so each boundary
//! test reads the arm at a reference cell above the bound and asserts against
//! that arm: on a host carrying the kernel the arm changes exactly at the
//! bound, and on a host without it the arm cannot change at all. No test here
//! touches the GF(251) debug toggles, which stay at their `false` production
//! default; `prime_route_toggle.rs` covers the toggled arm.
#![cfg(all(feature = "simd", not(gf2_tuning_baked)))]

use gf2_core::gfp::simd_ops::{prime_gemm_route, PrimeGemmRoute};
use gf2_core::tuning::TuningProfile;

/// Operand shape held fixed across the boundary walks: the window predicates
/// take `m` and `k` but select on `P` and `n` alone.
const M: usize = 64;
const K: usize = 64;

#[test]
fn gf251_takes_the_f32_cascade_from_the_conservative_column_bound() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f32_min_cols();
    let below = cols
        .checked_sub(1)
        .expect("the bound admits a cell below it");

    let at_bound = prime_gemm_route::<251>(M, K, cols);
    if at_bound.is_f32_cascade() {
        assert!(prime_gemm_route::<251>(M, K, cols + 1).is_f32_cascade());
        assert!(
            !prime_gemm_route::<251>(M, K, below).is_f32_cascade(),
            "the column bound must exclude the cell below it"
        );
    } else {
        assert_eq!(
            prime_gemm_route::<251>(M, K, below),
            at_bound,
            "this host registers no f32 cascade kernel, so the dispatcher \
             takes the same arm on both sides of the column bound"
        );
    }
}

#[test]
fn small_primes_below_the_conservative_prime_bound_never_take_the_cascade() {
    let conservative = TuningProfile::CONSERVATIVE;
    let prime_route = conservative.prime_route();
    assert!(
        (242..=251).contains(&prime_route.f32_min_prime()),
        "the conservative prime_route.f32_min_prime is {}, outside the \
         (241, 251] window this test brackets; re-pin it against the \
         in-scope primes adjacent to the new bound",
        prime_route.f32_min_prime()
    );
    let wide = prime_route.f32_min_cols() + 512;

    for route in [
        prime_gemm_route::<7>(M, K, wide),
        prime_gemm_route::<31>(M, K, wide),
        prime_gemm_route::<127>(M, K, wide),
        prime_gemm_route::<241>(M, K, wide),
    ] {
        assert!(!route.is_f32_cascade());
    }
}

#[test]
fn medium_primes_take_the_f64_cascade_from_the_conservative_column_bound() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f64_min_cols();
    let below = cols
        .checked_sub(1)
        .expect("the bound admits a cell below it");

    match prime_gemm_route::<65521>(M, K, cols) {
        PrimeGemmRoute::F64Cascade => {
            assert_eq!(
                prime_gemm_route::<257>(M, K, cols),
                PrimeGemmRoute::F64Cascade
            );
            assert_ne!(
                prime_gemm_route::<65521>(M, K, below),
                PrimeGemmRoute::F64Cascade,
                "the column bound must exclude the cell below it"
            );
        }
        arm => assert_eq!(
            prime_gemm_route::<65521>(M, K, below),
            arm,
            "this host registers no f64 cascade kernel, so the dispatcher \
             takes the same arm on both sides of the column bound"
        ),
    }
}

#[test]
fn primes_outside_both_eligibility_windows_reach_no_dispatcher_arm() {
    let wide = TuningProfile::CONSERVATIVE.prime_route().f64_min_cols() + 512;

    // `fp_small_enabled` covers `3 ..= 251` and `fp_medium_eligible` covers
    // `252 .. 65536`; a prime outside both reaches neither dispatcher, whose
    // eligibility gate runs before any window predicate.
    assert_eq!(
        prime_gemm_route::<65537>(M, K, wide),
        PrimeGemmRoute::Deferred
    );
    assert_eq!(prime_gemm_route::<2>(M, K, wide), PrimeGemmRoute::Deferred);
}

#[test]
fn a_degenerate_shape_reaches_no_dispatcher_arm() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f32_min_cols();

    // Both dispatchers reject `m == 0 || k == 0 || n == 0` before selecting.
    assert_eq!(
        prime_gemm_route::<251>(0, K, cols),
        PrimeGemmRoute::Deferred
    );
    assert_eq!(
        prime_gemm_route::<251>(M, 0, cols),
        PrimeGemmRoute::Deferred
    );
    assert_eq!(prime_gemm_route::<251>(M, K, 0), PrimeGemmRoute::Deferred);
}
