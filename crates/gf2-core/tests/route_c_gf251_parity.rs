//! Route-C GF(251) integer-panel kernel parity: the public `gemm` entry point
//! returns the same matrix with the debug switch
//! [`gf2_core::gfp::simd_ops::set_route_c_gf251_enabled`] on and off.

#![cfg(feature = "simd")]

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::gfp::simd_ops::set_route_c_gf251_enabled;
use std::sync::Mutex;

// The route-C switch is process-wide; tests serialise their set/restore pairs.
static ROUTE_C_MUTEX: Mutex<()> = Mutex::new(());

const P: u64 = 251;

fn run_one(m: usize, k: usize, n: usize, seed_a: u64, seed_b: u64) {
    let a = fp_matrix_from_seed::<P>(m, k, seed_a);
    let b = fp_matrix_from_seed::<P>(k, n, seed_b);

    let _guard = ROUTE_C_MUTEX.lock().unwrap();

    set_route_c_gf251_enabled(false);
    let c_default = gemm(&a, &b);

    set_route_c_gf251_enabled(true);
    let c_route_c = gemm(&a, &b);

    set_route_c_gf251_enabled(false);

    // Compare element by element so a failure pinpoints the cell.
    for i in 0..m {
        for j in 0..n {
            assert_eq!(
                c_default.get(i, j).value(),
                c_route_c.get(i, j).value(),
                "route-C vs default mismatch at ({i}, {j}) for (m={m}, k={k}, n={n})",
            );
        }
    }
}

#[test]
fn route_c_matches_default_at_criterion_n_values() {
    let ns = [
        1usize, 15, 16, 17, 23, 24, 25, 47, 48, 49, 63, 64, 65, 95, 96, 97, 121, 255, 256, 257,
        1023, 1024,
    ];
    for &n in &ns {
        run_one(n, n, n, 1, 2);
    }
}

#[test]
fn route_c_matches_default_at_k_chunk_boundary() {
    // k brackets KC = 256 and includes odd values for the pair tail.
    let m = 4;
    let n = 256;
    let ks = [
        1usize, 2, 64, 127, 128, 255, 256, 257, 511, 512, 1023, 1024, 1025,
    ];
    for &k in &ks {
        run_one(m, k, n, 3, 4);
    }
}

#[test]
fn route_c_matches_default_at_m_partial() {
    // m not a multiple of MR = 4 → trailing partial row tile.
    let k = 256;
    let n = 256;
    for &m in &[1usize, 2, 3, 5, 6, 7, 9, 33] {
        run_one(m, k, n, 5, 6);
    }
}

#[test]
fn route_c_matches_default_at_n_partial() {
    // n not a multiple of NR = 24 → trailing partial column panel.
    let m = 4;
    let k = 64;
    for &n in &[1usize, 8, 23, 24, 25, 47, 48, 49, 95, 96, 97, 121] {
        run_one(m, k, n, 7, 8);
    }
}

#[test]
fn route_c_off_leaves_dispatch_unchanged() {
    // With the switch off, two consecutive calls agree.
    let _guard = ROUTE_C_MUTEX.lock().unwrap();
    set_route_c_gf251_enabled(false);
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
