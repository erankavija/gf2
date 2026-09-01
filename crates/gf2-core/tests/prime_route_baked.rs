//! Routing witness for the baked `prime_route` selector family.
//!
//! These tests assert the committed conservative boundaries, so they are
//! compiled only under the declared cfg `gf2_tuning_baked`
//! (`RUSTFLAGS="--cfg gf2_tuning_baked"`); the default build's conservative
//! boundaries are asserted by `prime_route_reporter.rs`.
//!
//! The measured format-2 core owner at
//! `crates/gf2-core/data/tuning-profiles/gf2-389aa4de-20260901-040229-2742533.json`
//! cites `dev/benchmarks/tuning_profiles/2026-09-01-389aa4de.md` and omits
//! `prime_route`. These baked fields therefore retain their conservative
//! defaults, as `dev/active/3fa7c9d0/design.md` §7.1 requires.
//!
//! `prime_gemm_route` reports the dispatchers' whole gate chain, and whether a
//! cascade kernel is registered is a host property, so each boundary test
//! reads the arm at a reference cell above the bound and asserts against that
//! arm. No test here touches the GF(251) debug switches, which stay at their
//! `false` production default.
#![cfg(all(gf2_tuning_baked, feature = "simd"))]

use gf2_core::gfp::simd_ops::{prime_gemm_route, PrimeGemmRoute};
use gf2_core::tuning::{self, CoreTuning};

#[path = "support/measured_format2.rs"]
mod measured_format2;
#[path = "support/core_tuning.rs"]
mod support;

/// Operand shape held fixed across the boundary walks: the window predicates
/// take `m` and `k` but select on `P` and `n` alone.
const M: usize = 64;
const K: usize = 64;

/// Returns the defaults resolved from the measured owner's omission.
fn measured_default_section() -> CoreTuning {
    measured_format2::omitted_family_section("prime_route")
}

#[test]
fn baked_build_routes_gf251_by_the_measured_default_column_bound() {
    let profile = measured_default_section();
    let cols = profile.prime_route().f32_min_cols();
    let below = cols
        .checked_sub(1)
        .expect("the bound admits a cell below it");

    let at_bound = prime_gemm_route::<251>(M, K, cols);
    if at_bound.is_f32_cascade() {
        assert!(
            !prime_gemm_route::<251>(M, K, below).is_f32_cascade(),
            "the baked column bound must exclude the cell below it"
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
fn baked_build_brackets_the_measured_default_prime_bound() {
    let profile = measured_default_section();
    let prime_route = profile.prime_route();
    assert!(
        (242..=251).contains(&prime_route.f32_min_prime()),
        "the measured default prime_route.f32_min_prime is {}, outside the \
         (241, 251] window this witness brackets; re-pin it against the \
         in-scope primes adjacent to the candidate bound",
        prime_route.f32_min_prime()
    );
    let wide = prime_route.f32_min_cols() + 512;

    let below_bound = prime_gemm_route::<241>(M, K, wide);
    assert!(
        !below_bound.is_f32_cascade(),
        "the prime below the baked bound must stay off the cascade"
    );
    let at_bound = prime_gemm_route::<251>(M, K, wide);
    if !at_bound.is_f32_cascade() {
        assert_eq!(
            at_bound, below_bound,
            "this host registers no f32 cascade kernel, so the prime bound \
             cannot move the arm"
        );
    }
}

#[test]
fn baked_build_routes_medium_primes_by_the_measured_default_column_bound() {
    let profile = measured_default_section();
    let cols = profile.prime_route().f64_min_cols();
    let below = cols
        .checked_sub(1)
        .expect("the bound admits a cell below it");

    match prime_gemm_route::<65521>(M, K, cols) {
        PrimeGemmRoute::F64Cascade => assert_ne!(
            prime_gemm_route::<65521>(M, K, below),
            PrimeGemmRoute::F64Cascade,
            "the baked column bound must exclude the cell below it"
        ),
        arm => assert_eq!(
            prime_gemm_route::<65521>(M, K, below),
            arm,
            "this host registers no f64 cascade kernel, so the dispatcher \
             takes the same arm on both sides of the column bound"
        ),
    }
}

support::fresh_tuning_test!(install_does_not_govern_prime_route_selection, {
    let installed = support::prepared_core_json(
        r#"
        {
            "prime_route": {
              "f32_min_prime": 7,
              "f32_min_cols": 1,
              "f64_min_cols": 1
            }
        }"#,
    )
    .expect("test profile is valid");
    tuning::install(installed).expect("profile has not been resolved");

    let active = tuning::active();
    assert_eq!(active.prime_route().f32_min_prime(), 7);
    assert_eq!(active.prime_route().f32_min_cols(), 1);
    assert_eq!(active.prime_route().f64_min_cols(), 1);

    // The family is baked: the installed profile would put every cell below on
    // a cascade if the boundary were read at run time, and none of them move.
    let profile = measured_default_section();
    let prime_route = profile.prime_route();
    if let Some(below) = prime_route.f32_min_cols().checked_sub(1) {
        assert!(!prime_gemm_route::<251>(M, K, below).is_f32_cascade());
        assert!(!prime_gemm_route::<7>(M, K, below).is_f32_cascade());
    }
    if let Some(below) = prime_route.f64_min_cols().checked_sub(1) {
        assert_ne!(
            prime_gemm_route::<65521>(M, K, below),
            PrimeGemmRoute::F64Cascade
        );
    }
});
