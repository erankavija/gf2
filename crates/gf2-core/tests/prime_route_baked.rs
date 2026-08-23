//! Routing witness for the baked `prime_route` selector family.
//!
//! These tests assert the committed calibrated boundaries, so they are
//! compiled only under the declared cfg `gf2_tuning_baked`
//! (`RUSTFLAGS="--cfg gf2_tuning_baked"`); the default build's conservative
//! boundaries are asserted by `prime_route_reporter.rs`.
//!
//! The expected values come from the committed calibrated profile through the
//! profile loader, which resolves a field the document omits to its
//! conservative default — the same rule the baked constants carry
//! (`dev/active/7d824b2f/design.md` D5). The witness therefore names no
//! literal boundary and follows a future calibration on its own.
#![cfg(all(gf2_tuning_baked, feature = "simd"))]

use gf2_core::gfp::simd_ops::{prime_gemm_route, PrimeGemmRoute};
use gf2_core::tuning::{self, TuningProfile};
use std::fs;
use std::path::PathBuf;

/// Operand shape held fixed across the boundary walks: the two predicates
/// take `m` and `k` but select on `P` and `n` alone.
const M: usize = 64;
const K: usize = 64;

/// Loads the committed calibrated profile the baked constants mirror.
fn committed_profile() -> TuningProfile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("tuning-profiles")
        .join("gf2-5ecc9bf8-calibration-e202c080.json");
    let text = fs::read_to_string(path).expect("committed calibrated profile is readable");
    TuningProfile::from_json(&text).expect("committed calibrated profile is valid")
}

#[test]
fn baked_build_routes_gf251_by_the_committed_column_bound() {
    let profile = committed_profile();
    let f32_min_cols = profile.prime_route().f32_min_cols();

    assert_eq!(
        prime_gemm_route::<251>(M, K, f32_min_cols),
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
fn baked_build_brackets_the_committed_prime_bound() {
    let profile = committed_profile();
    let prime_route = profile.prime_route();
    assert!(
        (242..=251).contains(&prime_route.f32_min_prime()),
        "the committed prime_route.f32_min_prime is {}, outside the \
         (241, 251] window this witness brackets; re-pin it against the \
         in-scope primes adjacent to the new bound",
        prime_route.f32_min_prime()
    );
    let wide = prime_route.f32_min_cols() + 512;

    assert_eq!(
        prime_gemm_route::<241>(M, K, wide),
        PrimeGemmRoute::U16Baseline
    );
    assert_eq!(
        prime_gemm_route::<251>(M, K, wide),
        PrimeGemmRoute::F32Cascade
    );
}

#[test]
fn baked_build_routes_medium_primes_by_the_committed_column_bound() {
    let profile = committed_profile();
    let f64_min_cols = profile.prime_route().f64_min_cols();

    assert_eq!(
        prime_gemm_route::<65521>(M, K, f64_min_cols),
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
fn install_does_not_govern_prime_route_selection() {
    let installed = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "baked-prime-route-install-witness",
          "provenance": {"kind": "inherited"},
          "selectors": {
            "prime_route": {
              "f32_min_prime": 7,
              "f32_min_cols": 1,
              "f64_min_cols": 1
            }
          }
        }
        "#,
    )
    .expect("test profile is valid");
    tuning::install(installed).expect("profile has not been resolved");

    let active = tuning::active();
    assert_eq!(active.prime_route().f32_min_prime(), 7);
    assert_eq!(active.prime_route().f32_min_cols(), 1);
    assert_eq!(active.prime_route().f64_min_cols(), 1);

    // The family is baked, so the installed runtime profile moves neither
    // the prime window nor either column bound.
    let profile = committed_profile();
    let prime_route = profile.prime_route();
    if let Some(below) = prime_route.f32_min_cols().checked_sub(1) {
        assert_eq!(
            prime_gemm_route::<251>(M, K, below),
            PrimeGemmRoute::U16Baseline
        );
        assert_eq!(
            prime_gemm_route::<7>(M, K, below),
            PrimeGemmRoute::U16Baseline
        );
    }
    if let Some(below) = prime_route.f64_min_cols().checked_sub(1) {
        assert_eq!(
            prime_gemm_route::<65521>(M, K, below),
            PrimeGemmRoute::U16Baseline
        );
    }
}
