//! Production-dispatch parity for prime-field GEMM: the public `gemm` entry
//! point, with the route-A debug switch at its `false` default, agrees with a
//! scalar reference at boundary widths and at n = 512, 1024 and 4096.

#![cfg(feature = "simd")]

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::gfp::simd_ops::set_route_a_gf251_enabled;
use gf2_core::gfp::Fp;
use proptest::prelude::*;
use std::sync::Mutex;

// The switch is process-wide; tests serialise their set/restore pairs.
static DISPATCH_MUTEX: Mutex<()> = Mutex::new(());

/// Scalar reference `C = A · B`, returned row-major.
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

    let _guard = DISPATCH_MUTEX.lock().unwrap();
    set_route_a_gf251_enabled(false);
    let c_prod = gemm(&a_mat, &b_mat);
    set_route_a_gf251_enabled(false);

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

proptest! {
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
        check_production_vs_scalar::<251>(n, n, n, seed_a, seed_b);
    }
}

proptest! {
    /// Rectangular (m = 4, k = 64) to bound the scalar reference's cost.
    #[test]
    fn proptest_production_dispatch_n512_matches_scalar(
        seed_a in 1u64..=100,
        seed_b in 101u64..=200,
    ) {
        check_production_vs_scalar::<251>(4, 64, 512, seed_a, seed_b);
    }
}

proptest! {
    #[test]
    fn proptest_production_dispatch_n1024_matches_scalar(
        seed_a in 1u64..=100,
        seed_b in 101u64..=200,
    ) {
        check_production_vs_scalar::<251>(4, 64, 1024, seed_a, seed_b);
    }
}

proptest! {
    #[test]
    fn proptest_production_dispatch_n4096_matches_scalar(
        seed_a in 1u64..=50,
        seed_b in 51u64..=100,
    ) {
        check_production_vs_scalar::<251>(4, 16, 4096, seed_a, seed_b);
    }
}

proptest! {
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
        check_production_vs_scalar::<7>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<31>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<127>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<241>(n, n, n, seed_a, seed_b);
        check_production_vs_scalar::<251>(n, n, n, seed_a, seed_b);
    }
}
