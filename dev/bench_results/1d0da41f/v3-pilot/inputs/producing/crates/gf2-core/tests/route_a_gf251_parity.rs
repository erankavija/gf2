//! Route-A GF(251) f32/FMA cascade parity tests (issue 68cdf4c8).
//!
//! Verifies bit-exact equality between the reworked Candidate F path
//! (route A) and the default Candidate C dispatch across the boundary
//! `n` values listed in the issue's success criterion 2:
//!
//! > Bit-exact equality vs the existing Candidate C output across n in
//! > {64, 256, 1024} on canonical seeds (proptest or fixed-seed parity
//! > test).
//!
//! The toggle is exercised via [`gf2_core::gfp::simd_ops::set_route_a_gf251_enabled`],
//! a safe `AtomicBool`-backed setter. No `unsafe` env-var mutation is
//! needed. Concurrent tests use the `ROUTE_A_MUTEX` to serialise toggle
//! access since the flag is process-wide.
//!
//! The dispatch path under test is the production `gemm` entry point
//! (`crates/gf2-core/src/field/matrix.rs::gemm`) which forwards to
//! `fp_small_try_gemm_classical` via `F::try_simd_gemm_classical`.

#![cfg(feature = "simd")]

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::gfp::simd_ops::set_route_a_gf251_enabled;
use std::sync::Mutex;

// The route-A toggle is a process-wide AtomicBool; tests must serialise
// their set/restore pairs to avoid races when nextest runs them in
// parallel threads.
static ROUTE_A_MUTEX: Mutex<()> = Mutex::new(());

const P: u64 = 251;

fn run_one(m: usize, k: usize, n: usize, seed_a: u64, seed_b: u64) {
    let a = fp_matrix_from_seed::<P>(m, k, seed_a);
    let b = fp_matrix_from_seed::<P>(k, n, seed_b);

    // Serialise toggle mutation across concurrent test threads.
    let _guard = ROUTE_A_MUTEX.lock().unwrap();

    // Compute the Candidate C output (production default, toggle off).
    set_route_a_gf251_enabled(false);
    let c_default = gemm(&a, &b);

    // Compute the route-A output with the toggle enabled.
    set_route_a_gf251_enabled(true);
    let c_route_a = gemm(&a, &b);

    // Restore the toggle to off before releasing the mutex.
    set_route_a_gf251_enabled(false);

    // Compare element by element so a failure pinpoints the cell.
    for i in 0..m {
        for j in 0..n {
            assert_eq!(
                c_default.get(i, j).value(),
                c_route_a.get(i, j).value(),
                "route-A vs default mismatch at ({i}, {j}) for (m={m}, k={k}, n={n})",
            );
        }
    }
}

#[test]
fn route_a_matches_default_at_criterion_n_values() {
    // The issue's success criterion 2 calls out n ∈ {64, 256, 1024}.
    // We extend the sweep to include the boundary n values cited in
    // the design note: {1, 15, 16, 17, 63, 64, 65, 255, 256, 257,
    // 1023, 1024}. m = k = n in each cell (square gemm), which is the
    // shape the headline benchmark cells use.
    let ns = [1usize, 15, 16, 17, 63, 64, 65, 255, 256, 257, 1023, 1024];
    for &n in &ns {
        run_one(n, n, n, 1, 2);
    }
}

#[test]
fn route_a_matches_default_at_k_chunk_boundary() {
    // k around k_max(p=251) = 268 and the K_CHUNK_CAP = 1024 cell;
    // covers the multi-chunk vs single-chunk transition that the
    // route-A reduction depends on.
    let m = 4;
    let n = 256;
    let ks = [1usize, 64, 256, 267, 268, 269, 512, 1023, 1024, 1025];
    for &k in &ks {
        run_one(m, k, n, 3, 4);
    }
}

#[test]
fn route_a_matches_default_at_m_partial() {
    // m not a multiple of M_R = 4 → trailing partial row tile.
    let k = 256;
    let n = 256;
    for &m in &[1usize, 2, 3, 5, 6, 7, 9, 33] {
        run_one(m, k, n, 5, 6);
    }
}

#[test]
fn route_a_matches_default_at_n_partial() {
    // n not a multiple of N_R = 24 → trailing partial column panel.
    let m = 4;
    let k = 64;
    for &n in &[1usize, 8, 23, 24, 25, 47, 48, 49, 95, 96, 97, 121] {
        run_one(m, k, n, 7, 8);
    }
}

#[test]
fn route_a_off_leaves_dispatch_unchanged() {
    // With the toggle off, two consecutive default-dispatch calls
    // must produce the same output (sanity check that toggling does
    // not leak across calls).
    let _guard = ROUTE_A_MUTEX.lock().unwrap();
    set_route_a_gf251_enabled(false);
    let a = fp_matrix_from_seed::<P>(16, 16, 11);
    let b = fp_matrix_from_seed::<P>(16, 16, 13);
    let c1 = gemm(&a, &b);
    let c2 = gemm(&a, &b);
    for i in 0..16 {
        for j in 0..16 {
            assert_eq!(c1.get(i, j).value(), c2.get(i, j).value());
        }
    }
}
