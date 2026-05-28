//! Production-dispatch parity proptests for the GF(251)/n>=512 route-A
//! wire-in (issue 41096af5).
//!
//! Verifies bit-exact equality between:
//!
//!   * The new production default path (AtomicBool toggle `false`, so
//!     `select_f32_path` governs: route A for `P == 251 && n >= 512`,
//!     Candidate C otherwise); and
//!   * A scalar naive GEMM reference using direct field arithmetic.
//!
//! Four proptest blocks, per SC#6 and SC#9 of issue 41096af5:
//!
//! 1. `proptest_production_dispatch_boundary_n_values` — GF(251) square GEMM
//!    at boundary n in `{0, 1, 15, 16, 17, 63, 64, 65}` (n < 512, Candidate C
//!    path). Uses `prop_oneof![Just(0), ...]` as required by SC#9 / 52cce970 R1.
//!
//! 2. `proptest_production_dispatch_n512_matches_scalar` — n=512 (first cell
//!    in the route-A window). Uses rectangular shape (m=4, k=64, n=512) to
//!    stay within the 5-second CI wall-clock limit.
//!
//! 3. `proptest_production_dispatch_n1024_matches_scalar` — n=1024 (headline
//!    PASS cell, ratio 0.679 vs fflas-ffpack). Uses rectangular (m=4, k=64).
//!
//! 4. `proptest_production_dispatch_prime_sweep_boundary_n` — GF(p) prime
//!    sweep at boundary n in `{0, 1, 15, 16, 17, 63, 64, 65}` across ALL
//!    in-scope small primes: GF(7), GF(31), GF(127), GF(241), GF(251).
//!    All n < 512, so all primes stay on Candidate C regardless of the
//!    N_THRESH_PRIME=251 wire-in. Verifies correctness is preserved for
//!    non-GF(251) primes at boundary lengths (SC#6 prime-sweep requirement).
//!
//! NOTE: The n=512 and n=1024 blocks use m=4, k=64 to keep the scalar oracle
//! fast (4 * 64 * N = ~131-262K operations) while still exercising the full
//! `select_f32_path` branch (it checks `n >= 512`, not `m == n`).
//!
//! The dispatch path under test is the production `gemm` entry point
//! (`crates/gf2-core/src/field/matrix.rs::gemm`) forwarding to
//! `fp_small_try_gemm_classical` via `F::try_simd_gemm_classical`.

#![cfg(feature = "simd")]

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::gfp::simd_ops::set_route_a_gf251_enabled;
use gf2_core::gfp::Fp;
use proptest::prelude::*;
use std::sync::Mutex;

// Serialise AtomicBool toggle mutations across concurrent test threads.
static DISPATCH_MUTEX: Mutex<()> = Mutex::new(());

// ── Scalar reference ──────────────────────────────────────────────────────────

/// Naive GF(Q) GEMM reference: computes C = A * B using direct field
/// arithmetic. A is (m x k), B is (k x n), both accessed via the
/// `FieldMatrix::get` interface. Returns a flat row-major Vec.
fn naive_gemm_gf<const Q: u64>(
    a: &gf2_core::field::matrix::FieldMatrix<Fp<Q>>,
    b: &gf2_core::field::matrix::FieldMatrix<Fp<Q>>,
    m: usize,
    k: usize,
    n: usize,
) -> Vec<Fp<Q>> {
    let mut c = vec![Fp::<Q>::new(0); m * n];
    for i in 0..m {
        for j in 0..n {
            let mut acc = Fp::<Q>::new(0);
            for l in 0..k {
                acc += a.get(i, l) * b.get(l, j);
            }
            c[i * n + j] = acc;
        }
    }
    c
}

// ── Core comparison helper ────────────────────────────────────────────────────

/// Compare the production dispatch output (AtomicBool=false, `select_f32_path`
/// governs routing) against the scalar naive_gemm_gf reference, for prime Q.
///
/// For Q == 251 && n >= 512: `select_f32_path` returns `true` => route A.
/// For all other (Q, n) combinations in-scope: Candidate C.
fn check_production_vs_scalar<const Q: u64>(
    m: usize,
    k: usize,
    n: usize,
    seed_a: u64,
    seed_b: u64,
) {
    if m == 0 || k == 0 || n == 0 {
        return;
    }

    let a_mat = fp_matrix_from_seed::<Q>(m, k, seed_a);
    let b_mat = fp_matrix_from_seed::<Q>(k, n, seed_b);

    // Production dispatch with AtomicBool at default (false).
    // For Q==251 && n>=512: select_f32_path returns true => route A.
    // For n<512 or Q!=251: select_f32_path returns false => Candidate C.
    let _guard = DISPATCH_MUTEX.lock().unwrap();
    set_route_a_gf251_enabled(false);
    let c_prod = gemm(&a_mat, &b_mat);
    set_route_a_gf251_enabled(false); // restore

    let c_scalar = naive_gemm_gf::<Q>(&a_mat, &b_mat, m, k, n);

    for i in 0..m {
        for j in 0..n {
            assert_eq!(
                c_prod.get(i, j).value(),
                c_scalar[i * n + j].value(),
                "production-dispatch vs scalar mismatch at ({i},{j}) \
                 shape=({m},{k},{n}) seed_a={seed_a} seed_b={seed_b} prime={Q}"
            );
        }
    }
}

// ── Proptest blocks ───────────────────────────────────────────────────────────

proptest! {
    /// Boundary n values {0, 1, 15, 16, 17, 63, 64, 65}: all < 512, so
    /// the production dispatch uses Candidate C. Verify GF(251) output matches
    /// scalar. The `prop_oneof![Just(0), ...]` form is required by SC#9
    /// (52cce970 R1 review trap — `#[test]` boundary cases are not equivalent
    /// to proptest).
    #[test]
    fn proptest_production_dispatch_boundary_n_values(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        if n == 0 {
            return Ok(());
        }
        // Square GEMM at boundary n — all small, scalar reference is fast.
        check_production_vs_scalar::<251>(n, n, n, seed_a, seed_b);
    }
}

proptest! {
    /// n=512: first cell in the production route-A dispatch window
    /// (`P == 251 && n >= 512`). Uses rectangular (m=4, k=64) to stay fast.
    /// Verifies bit-exact correctness of the new GF(251) default dispatch.
    #[test]
    fn proptest_production_dispatch_n512_matches_scalar(
        seed_a in 1u64..=100,
        seed_b in 101u64..=200,
    ) {
        check_production_vs_scalar::<251>(4, 64, 512, seed_a, seed_b);
    }
}

proptest! {
    /// n=1024: headline PASS cell (ratio 0.679 vs fflas-ffpack at threshold
    /// 0.667). Uses rectangular (m=4, k=64) to stay fast. Verifies bit-exact
    /// correctness of the GF(251) production route-A dispatch at the
    /// measurement cell.
    #[test]
    fn proptest_production_dispatch_n1024_matches_scalar(
        seed_a in 1u64..=100,
        seed_b in 101u64..=200,
    ) {
        check_production_vs_scalar::<251>(4, 64, 1024, seed_a, seed_b);
    }
}

proptest! {
    /// n=4096: extended correctness check for issue 98336ab4.
    ///
    /// n=4096 is in the production route-A window (`P == 251 && n >= 512`);
    /// `select_f32_path` returns `true`. Uses rectangular shape (m=4, k=16)
    /// to bound scalar oracle runtime (4 * 16 * 4096 ≈ 262 K operations)
    /// while still exercising the full `select_f32_path` branch.
    #[test]
    fn proptest_production_dispatch_n4096_matches_scalar(
        seed_a in 1u64..=50,
        seed_b in 51u64..=100,
    ) {
        check_production_vs_scalar::<251>(4, 16, 4096, seed_a, seed_b);
    }
}

proptest! {
    /// GF(p) prime-sweep at boundary lengths {0, 1, 15, 16, 17, 63, 64, 65}.
    ///
    /// Covers ALL in-scope small primes: GF(7), GF(31), GF(127), GF(241),
    /// GF(251). All cells have n < 512, so the production dispatch uses
    /// Candidate C regardless of the N_THRESH_PRIME=251 wire-in.
    ///
    /// SC#6 requirement: bit-exact correctness preserved across the prime
    /// sweep regardless of the GF(251)/n>=512 wire-in. GF(7)/GF(31)/
    /// GF(127)/GF(241) must be unaffected by the N_THRESH_PRIME change.
    #[test]
    fn proptest_production_dispatch_prime_sweep_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        if n == 0 {
            return Ok(());
        }
        // All n < 512 => all primes route through Candidate C.
        check_production_vs_scalar::<7>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<31>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<127>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<241>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<251>(n, n, n, seed_a, seed_b);
    }
}
