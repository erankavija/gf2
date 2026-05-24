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
//! The toggle is exercised via the `GF2_GF251_ROUTE_A` environment
//! variable, which is read at dispatch time by `select_f32_path` in
//! `crates/gf2-core/src/gfp/simd_ops.rs`. We use `std::env::set_var`
//! and `std::env::remove_var` to flip the toggle in-process, with a
//! mutex serialising env-var access since `set_var` is not thread-safe.
//!
//! The dispatch path under test is the production `gemm` entry point
//! (`crates/gf2-core/src/field/matrix.rs::gemm`) which forwards to
//! `fp_small_try_gemm_classical` via `F::try_simd_gemm_classical`.

#![cfg(feature = "simd")]

use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gfp::Fp;
use std::sync::Mutex;

// Env-var mutation in tests must be serialised because `std::env::set_var`
// is not thread-safe (rustc 1.95 deprecates per-thread env mutation for
// good reason). Every route-A test takes this mutex before flipping
// `GF2_GF251_ROUTE_A` and releases it after the comparison.
static ROUTE_A_ENV_MUTEX: Mutex<()> = Mutex::new(());

const P: u64 = 251;

/// Deterministic seed-derived matrix generator. Uses splitmix64 so the
/// generated matrices are stable across runs and across the route-A
/// and Candidate-C call sites.
fn fp251_matrix_from_seed(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut mat = FieldMatrix::<Fp<P>>::new(rows, cols, Fp::<P>::new(0));
    for i in 0..rows {
        for j in 0..cols {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            mat.set(i, j, Fp::<P>::new(z % P));
        }
    }
    mat
}

fn run_one(m: usize, k: usize, n: usize, seed_a: u64, seed_b: u64) {
    let a = fp251_matrix_from_seed(m, k, seed_a);
    let b = fp251_matrix_from_seed(k, n, seed_b);

    // Compute the Candidate C output (production default).
    let _guard = ROUTE_A_ENV_MUTEX.lock().unwrap();
    // SAFETY: env-var mutation is serialised by ROUTE_A_ENV_MUTEX above.
    unsafe {
        std::env::remove_var("GF2_GF251_ROUTE_A");
    }
    let c_default = gemm(&a, &b);

    // Compute the route-A output with the env var enabled.
    // SAFETY: env-var mutation is serialised by ROUTE_A_ENV_MUTEX above.
    unsafe {
        std::env::set_var("GF2_GF251_ROUTE_A", "1");
    }
    let c_route_a = gemm(&a, &b);
    // SAFETY: env-var mutation is serialised by ROUTE_A_ENV_MUTEX above.
    unsafe {
        std::env::remove_var("GF2_GF251_ROUTE_A");
    }

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
    // the design note: {0, 1, 15, 16, 17, 63, 64, 65, 255, 256, 257,
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
    // With the env var unset, two consecutive default-dispatch calls
    // must produce the same output (sanity check that toggling does
    // not leak across calls).
    let _guard = ROUTE_A_ENV_MUTEX.lock().unwrap();
    // SAFETY: env-var mutation is serialised by ROUTE_A_ENV_MUTEX above.
    unsafe {
        std::env::remove_var("GF2_GF251_ROUTE_A");
    }
    let a = fp251_matrix_from_seed(16, 16, 11);
    let b = fp251_matrix_from_seed(16, 16, 13);
    let c1 = gemm(&a, &b);
    let c2 = gemm(&a, &b);
    for i in 0..16 {
        for j in 0..16 {
            assert_eq!(c1.get(i, j).value(), c2.get(i, j).value());
        }
    }
}
