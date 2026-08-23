//! Route observation for the prime-field GEMM dispatchers in the default
//! build.
//!
//! `prime_gemm_route` calls the same `select_f32_path` / `select_f64_path`
//! predicates that `fp_small_try_gemm_classical` and
//! `fp_medium_try_gemm_panel` call, so these assertions observe the
//! production selection rather than a copy of it. The `prime_route` family is
//! baked (`dev/active/7d824b2f/design.md` §3.11), so the default build's
//! boundaries are the conservative table's values and no installed profile
//! moves them; `prime_route_baked.rs` witnesses the committed calibrated
//! boundaries under `--cfg gf2_tuning_baked`.
#![cfg(all(feature = "simd", not(gf2_tuning_baked)))]

use gf2_core::gfp::simd_ops::{prime_gemm_route, PrimeGemmRoute};
use gf2_core::tuning::TuningProfile;

/// Operand shape held fixed across the boundary walks: the two predicates
/// take `m` and `k` but select on `P` and `n` alone.
const M: usize = 64;
const K: usize = 64;

#[test]
fn gf251_takes_the_f32_cascade_from_the_conservative_column_bound() {
    let f32_min_cols = TuningProfile::CONSERVATIVE.prime_route().f32_min_cols();

    assert_eq!(
        prime_gemm_route::<251>(M, K, f32_min_cols),
        PrimeGemmRoute::F32Cascade
    );
    assert_eq!(
        prime_gemm_route::<251>(M, K, f32_min_cols + 1),
        PrimeGemmRoute::F32Cascade
    );
    if let Some(below) = f32_min_cols.checked_sub(1) {
        assert_eq!(
            prime_gemm_route::<251>(M, K, below),
            PrimeGemmRoute::U16Baseline
        );
    }
}

#[test]
fn small_primes_below_the_conservative_prime_bound_stay_on_the_baseline() {
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
        assert_eq!(route, PrimeGemmRoute::U16Baseline);
    }
    assert_eq!(
        prime_gemm_route::<251>(M, K, wide),
        PrimeGemmRoute::F32Cascade
    );
}

#[test]
fn medium_primes_take_the_f64_cascade_from_the_conservative_column_bound() {
    let f64_min_cols = TuningProfile::CONSERVATIVE.prime_route().f64_min_cols();

    assert_eq!(
        prime_gemm_route::<65521>(M, K, f64_min_cols),
        PrimeGemmRoute::F64Cascade
    );
    assert_eq!(
        prime_gemm_route::<257>(M, K, f64_min_cols),
        PrimeGemmRoute::F64Cascade
    );
    if let Some(below) = f64_min_cols.checked_sub(1) {
        assert_eq!(
            prime_gemm_route::<65521>(M, K, below),
            PrimeGemmRoute::U16Baseline
        );
    }
}

#[test]
fn primes_outside_both_windows_stay_on_the_baseline() {
    let wide = TuningProfile::CONSERVATIVE.prime_route().f64_min_cols() + 512;

    // 65537 is above the medium-prime window's upper bound, which is kernel
    // shape rather than a profile field.
    assert_eq!(
        prime_gemm_route::<65537>(M, K, wide),
        PrimeGemmRoute::U16Baseline
    );
}
