//! Compares the production `gemm()` dispatch with a naive scalar oracle over
//! GF(7), GF(31), GF(127), GF(241), GF(251), GF(257), GF(32749), GF(65521),
//! `Fp<65537>` and Mersenne31 at the square sizes {0, 1, 15, 16, 17, 63, 64,
//! 65}, and for the medium primes at rectangular shapes with `n` from 512.

#![cfg(feature = "simd")]

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::gfp::simd_ops::set_route_a_gf251_enabled;
use gf2_core::gfp::Fp;
use proptest::prelude::*;
use std::sync::Mutex;

// Serialise AtomicBool toggle mutations across concurrent test threads.
static DISPATCH_MUTEX: Mutex<()> = Mutex::new(());

const M31: u64 = (1u64 << 31) - 1;

/// Naive `C = A * B` for `A` (m × k) and `B` (k × n), as a flat row-major `Vec`.
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

/// Compares `gemm()` with the naive oracle at n × n, with route A disabled
/// under `DISPATCH_MUTEX`.
fn check_phase2_vs_scalar<const Q: u64>(n: usize, seed_a: u64, seed_b: u64) {
    let a_mat = fp_matrix_from_seed::<Q>(n, n, seed_a);
    let b_mat = fp_matrix_from_seed::<Q>(n, n, seed_b);

    let _guard = DISPATCH_MUTEX.lock().unwrap();
    set_route_a_gf251_enabled(false);
    let c_prod = gemm(&a_mat, &b_mat);
    set_route_a_gf251_enabled(false);

    let c_scalar = naive_gemm_gf::<Q>(&a_mat, &b_mat, n, n, n);

    // n = 0: both paths emit an empty result, so the shapes are compared.
    assert_eq!(
        c_prod.rows(),
        n,
        "production output row count mismatch at n={n} prime={Q}"
    );
    assert_eq!(
        c_prod.cols(),
        n,
        "production output col count mismatch at n={n} prime={Q}"
    );
    assert_eq!(
        c_scalar.len(),
        n * n,
        "scalar oracle length mismatch at n={n} prime={Q}"
    );

    for i in 0..n {
        for j in 0..n {
            assert_eq!(
                c_prod.get(i, j).value(),
                c_scalar[i * n + j].value(),
                "production-dispatch vs scalar mismatch at ({i},{j}) \
                 n={n} seed_a={seed_a} seed_b={seed_b} prime={Q}"
            );
        }
    }
}

proptest! {
    #[test]
    fn proptest_phase2_small_prime_sweep_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        check_phase2_vs_scalar::<7>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<31>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<127>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<241>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<251>(n, seed_a, seed_b);
    }
}

/// Rectangular variant of the comparison: (m × k) · (k × n).
fn check_dispatch_vs_scalar_rect<const Q: u64>(
    m: usize,
    k: usize,
    n: usize,
    seed_a: u64,
    seed_b: u64,
) {
    let a_mat = fp_matrix_from_seed::<Q>(m, k, seed_a);
    let b_mat = fp_matrix_from_seed::<Q>(k, n, seed_b);

    let _guard = DISPATCH_MUTEX.lock().unwrap();
    let c_prod = gemm(&a_mat, &b_mat);

    let c_scalar = naive_gemm_gf::<Q>(&a_mat, &b_mat, m, k, n);

    assert_eq!(c_prod.rows(), m);
    assert_eq!(c_prod.cols(), n);

    for i in 0..m {
        for j in 0..n {
            assert_eq!(
                c_prod.get(i, j).value(),
                c_scalar[i * n + j].value(),
                "dispatch vs scalar mismatch at ({i},{j}) m={m} k={k} n={n} \
                 seed_a={seed_a} seed_b={seed_b} prime={Q}"
            );
        }
    }
}

proptest! {
    // Kernel calls at n ≥ 512 are heavy, so the case count is small.
    #![proptest_config(ProptestConfig::with_cases(2))]

    #[test]
    fn proptest_0749dbad_medium_f64_cascade_boundary(
        m in prop_oneof![Just(1usize), Just(4), Just(17)],
        k in prop_oneof![Just(1usize), Just(17), Just(65)],
        n in prop_oneof![
            Just(512usize), Just(513), Just(527), Just(1024), Just(1025)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        check_dispatch_vs_scalar_rect::<257>(m, k, n, seed_a, seed_b);
        check_dispatch_vs_scalar_rect::<32749>(m, k, n, seed_a, seed_b);
        check_dispatch_vs_scalar_rect::<65521>(m, k, n, seed_a, seed_b);
    }
}

proptest! {
    #[test]
    fn proptest_phase2_medium_prime_sweep_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        check_phase2_vs_scalar::<257>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<32749>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<65521>(n, seed_a, seed_b);
    }
}

proptest! {
    #[test]
    fn proptest_phase2_fp65537_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        check_phase2_vs_scalar::<65537>(n, seed_a, seed_b);
    }
}

proptest! {
    #[test]
    fn proptest_phase2_mersenne31_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        check_phase2_vs_scalar::<{ M31 }>(n, seed_a, seed_b);
    }
}
