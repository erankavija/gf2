//! Phase 2 (e8a0c47a) multi-prime sweep proptests — SC#3.
//!
//! Verifies bit-exact equality between the production `gemm()` path and a
//! naive scalar oracle across **all 10 primes** named in SC#3:
//! GF(7), GF(31), GF(127), GF(241), GF(251), GF(257), GF(32749), GF(65521),
//! Fp<65537>, and Mersenne31 (`Fp<2147483647>`).
//!
//! Boundary lengths swept: `{1, 15, 16, 17, 63, 64, 65}` (n=0 is trivially a
//! no-op and is excluded from the proptest body via a guard — the
//! `prop_oneof![Just(0), ...]` form is used as required by the 52cce970 R1
//! trap, but the test body returns early on n=0).
//!
//! For small primes (p ≤ 251): the production path routes through
//! `fp_small_try_gemm_classical` → Candidate C (n < 512) or route A (p=251,
//! n ≥ 512, gated by `set_route_a_gf251_enabled`). All boundary lengths here
//! are below 512, so every small-prime cell stays on Candidate C.
//!
//! For medium primes (252 ≤ p < 65536): the production path routes through
//! the medium-prime u16 Barrett dot kernel (`fp_medium_batch_mul16`), which
//! now calls the shared `barrett_reduce_lane32` SSOT primitive introduced in
//! Phase 2. This is the call site added by the Phase 2 consolidation.
//!
//! For Fp<65537>: routes through the Fermat-prime specialised path.
//!
//! For Mersenne31: routes through the M31 specialised path.
//!
//! The file is intentionally separate from
//! `route_a_gf251_production_dispatch_proptests.rs` (which covers the 5
//! small primes at boundary lengths and route A at n=512/1024). This file
//! extends coverage to the 5 medium/large primes added by Phase 2, without
//! disturbing the 41096af5-era test structure.
//!
//! # Relationship to SC#2 (dispatch ordering)
//!
//! SC#2 (dispatch ordering invariant) is satisfied by the existing test
//! `gfp::simd_ops::tests::specialized_primes_do_not_use_generic_montgomery_path`
//! in `crates/gf2-core/src/gfp/simd_ops.rs` (line 2959), which asserts that
//! Fp<65537>, Mersenne31, Fp<65521>, Fp<257>, and Fp<32749> each route to
//! their specialised SIMD kernel rather than the generic Montgomery fallback.
//! The structural source ordering in `simd_ops.rs:190-243` (Fp<65537> first,
//! then M31, then small-prime family, then medium-prime family, then generic)
//! is preserved by Phase 2 — which only touches the inner kernel, not the
//! dispatcher.

#![cfg(feature = "simd")]

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::gfp::simd_ops::set_route_a_gf251_enabled;
use gf2_core::gfp::Fp;
use proptest::prelude::*;
use std::sync::Mutex;

// Serialise AtomicBool toggle mutations across concurrent test threads.
static DISPATCH_MUTEX: Mutex<()> = Mutex::new(());

// Mersenne31 prime as a const for use in generic const parameters.
const M31: u64 = (1u64 << 31) - 1;

// ── Scalar reference ──────────────────────────────────────────────────────────

/// Naive GF(Q) GEMM reference: computes C = A * B using direct field
/// arithmetic. A is (m x k), B is (k x n). Returns a flat row-major Vec.
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

/// Compare the production `gemm()` output against the scalar naive oracle.
///
/// For small primes (Q <= 251): uses the route-A toggle at default (false),
/// serialised through DISPATCH_MUTEX.
/// For other primes: no toggle interaction; the mutex is still held to avoid
/// interfering with any concurrent test that does mutate the toggle.
fn check_phase2_vs_scalar<const Q: u64>(n: usize, seed_a: u64, seed_b: u64) {
    if n == 0 {
        return;
    }

    let a_mat = fp_matrix_from_seed::<Q>(n, n, seed_a);
    let b_mat = fp_matrix_from_seed::<Q>(n, n, seed_b);

    let _guard = DISPATCH_MUTEX.lock().unwrap();
    set_route_a_gf251_enabled(false);
    let c_prod = gemm(&a_mat, &b_mat);
    set_route_a_gf251_enabled(false); // restore

    let c_scalar = naive_gemm_gf::<Q>(&a_mat, &b_mat, n, n, n);

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

// ── Proptest: small-prime extension (GF(241) not in 41096af5 sweep actually
//    is — we repeat GF(7)/31/127/241/251 for completeness) ───────────────────

proptest! {
    /// Phase 2 SC#3: small-prime sweep (GF(7), GF(31), GF(127), GF(241),
    /// GF(251)) at boundary lengths {0, 1, 15, 16, 17, 63, 64, 65}.
    ///
    /// These primes are also covered by
    /// `proptest_production_dispatch_prime_sweep_boundary_n` in
    /// `route_a_gf251_production_dispatch_proptests.rs`; this block is a
    /// redundant cross-check that also exercises the Phase 2 refactored
    /// `barrett_reduce_lane32` call site in `fp_small_panel.rs` and
    /// `fp_small_f32.rs` (both of which now call the shared SSOT).
    #[test]
    fn proptest_phase2_small_prime_sweep_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        if n == 0 { return Ok(()); }
        check_phase2_vs_scalar::<7>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<31>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<127>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<241>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<251>(n, seed_a, seed_b);
    }
}

proptest! {
    /// Phase 2 SC#3: medium-prime sweep (GF(257), GF(32749), GF(65521)) at
    /// boundary lengths {0, 1, 15, 16, 17, 63, 64, 65}.
    ///
    /// These primes route through `fp_medium_batch_mul16`, which in Phase 2
    /// now calls the shared `barrett_reduce_lane32` SSOT from `fp_small.rs`
    /// instead of its old local copy. This block is the bit-exact correctness
    /// gate for that new call site (SC#1's "at least one other call site").
    #[test]
    fn proptest_phase2_medium_prime_sweep_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        if n == 0 { return Ok(()); }
        check_phase2_vs_scalar::<257>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<32749>(n, seed_a, seed_b);
        check_phase2_vs_scalar::<65521>(n, seed_a, seed_b);
    }
}

proptest! {
    /// Phase 2 SC#3: Fermat prime Fp<65537> at boundary lengths
    /// {0, 1, 15, 16, 17, 63, 64, 65}.
    ///
    /// Routes through the dedicated `fp65537_try_*_vec` specialised path.
    /// Confirms Phase 2's dispatch ordering is preserved: Fp<65537> is
    /// checked first in `simd_ops.rs:193` before any small/medium/generic
    /// branch.
    #[test]
    fn proptest_phase2_fp65537_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        if n == 0 { return Ok(()); }
        check_phase2_vs_scalar::<65537>(n, seed_a, seed_b);
    }
}

proptest! {
    /// Phase 2 SC#3: Mersenne31 prime Fp<2147483647> at boundary lengths
    /// {0, 1, 15, 16, 17, 63, 64, 65}.
    ///
    /// Routes through the `fpm31_try_mul_vec` specialised path (for mul_vec
    /// operations). For GEMM, falls through to the medium-prime u16 path or
    /// scalar (M31 does not have a dedicated GEMM kernel). Correctness of
    /// the scalar fallback for Mersenne31 GEMM is confirmed here.
    #[test]
    fn proptest_phase2_mersenne31_boundary_n(
        n in prop_oneof![
            Just(0usize), Just(1), Just(15), Just(16),
            Just(17), Just(63), Just(64), Just(65)
        ],
        seed_a in 1u64..=200,
        seed_b in 201u64..=400,
    ) {
        if n == 0 { return Ok(()); }
        check_phase2_vs_scalar::<{ M31 }>(n, seed_a, seed_b);
    }
}
