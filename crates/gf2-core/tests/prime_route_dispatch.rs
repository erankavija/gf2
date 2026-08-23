//! Production-dispatch witness: the executed arm is the reported route.
//!
//! `prime_gemm_route` and both dispatchers resolve the same
//! `prime_gemm_select` call, so a report and an execution cannot disagree by
//! construction. This binary checks that construction end to end: it runs the
//! public `gemm` entry point, reads back the arm the dispatcher recorded at
//! its kernel-invocation site, and compares it against the reported route for
//! the same cell.
//!
//! The recorded arm is process-wide, so each test resets it immediately before
//! the GEMM it observes.
#![cfg(all(feature = "simd", not(gf2_tuning_baked)))]

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::gfp::simd_ops::{
    last_executed_prime_gemm_route, prime_gemm_route, reset_last_executed_prime_gemm_route,
    PrimeGemmRoute,
};
use gf2_core::tuning::TuningProfile;
use std::sync::Mutex;

/// Serialises the process-wide executed-arm observation across the test
/// threads this binary runs, so one test's GEMM cannot land between another's
/// reset and read.
static OBSERVATION_MUTEX: Mutex<()> = Mutex::new(());

/// Runs the public GEMM entry point for `Fp<Q>` at `m × k` times `k × n` and
/// returns the arm the dispatcher recorded while running it.
fn executed_route<const Q: u64>(m: usize, k: usize, n: usize) -> PrimeGemmRoute {
    let a = fp_matrix_from_seed::<Q>(m, k, 0x5eed_0001);
    let b = fp_matrix_from_seed::<Q>(k, n, 0x5eed_0002);

    let guard = OBSERVATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    reset_last_executed_prime_gemm_route();
    let _product = gemm(&a, &b);
    let executed = last_executed_prime_gemm_route();
    drop(guard);

    executed.expect(
        "the public gemm entry point did not reach a prime-field GEMM dispatcher for this shape",
    )
}

#[test]
fn gf251_above_the_column_bound_executes_the_reported_cascade() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f32_min_cols();
    let (m, k, n) = (4, 64, cols);

    let reported = prime_gemm_route::<251>(m, k, n);
    assert_eq!(executed_route::<251>(m, k, n), reported);
    assert!(
        reported.is_f32_cascade(),
        "this host registers no f32 cascade kernel, so the cell above the \
         column bound reported {reported:?} instead of a cascade arm"
    );
}

#[test]
fn gf251_below_the_column_bound_executes_the_reported_baseline() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f32_min_cols();
    let (m, k, n) = (4, 64, cols - 1);

    let reported = prime_gemm_route::<251>(m, k, n);
    assert_eq!(executed_route::<251>(m, k, n), reported);
    assert_eq!(reported, PrimeGemmRoute::ByteLaneBaseline);
}

#[test]
fn a_small_prime_below_the_prime_bound_executes_the_reported_baseline() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f32_min_cols();
    let (m, k, n) = (4, 64, cols);

    let reported = prime_gemm_route::<241>(m, k, n);
    assert_eq!(executed_route::<241>(m, k, n), reported);
    assert_eq!(reported, PrimeGemmRoute::ByteLaneBaseline);
}

#[test]
fn a_medium_prime_above_the_column_bound_executes_the_reported_cascade() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f64_min_cols();
    let (m, k, n) = (4, 64, cols);

    let reported = prime_gemm_route::<65521>(m, k, n);
    assert_eq!(executed_route::<65521>(m, k, n), reported);
    assert_eq!(reported, PrimeGemmRoute::F64Cascade);
}

#[test]
fn a_medium_prime_below_the_column_bound_executes_the_reported_baseline() {
    let cols = TuningProfile::CONSERVATIVE.prime_route().f64_min_cols();
    let (m, k, n) = (4, 64, cols - 1);

    let reported = prime_gemm_route::<65521>(m, k, n);
    assert_eq!(executed_route::<65521>(m, k, n), reported);
    assert_eq!(reported, PrimeGemmRoute::U16Panel);
}
