//! End-to-end batch GPU dispatcher tests for `gf2_algebra::gpu` (issue 2fbbdfa5).
//!
//! Verifies that the three host-side dispatcher functions —
//! [`permanent_batch_bipedal3`], [`permanent_batch_bipedal5`], and
//! [`permanent_batch_bipedal7`] — produce results bit-identical to the CPU
//! reference permanents on 1 000 random matrices at n = 24.
//!
//! # Gating
//!
//! The entire file is `#![cfg(feature = "hip")]` and every test carries
//! `#[ignore = "external: gfx1030 device required"]` per CLAUDE.md §Test
//! tiers. To run on a gfx1030 host with ROCm installed:
//!
//! ```text
//! cargo nextest run -p gf2-algebra \
//!     --release --features hip \
//!     --run-ignored ignored-only \
//!     -E 'test(test_permanent_batch_bipedal)'
//! ```
//!
//! # Timing note (n=24, 1000 matrices per prime)
//!
//! The GPU kernel runs one HIP block per matrix; all M blocks execute in
//! parallel on the device. With gfx1030's 36 compute units the effective
//! wall-clock is roughly `ceil(1000/36) * T_single` where `T_single` at
//! n=24 is approximately 14 s on a single GPU block (estimated from the
//! ad55b777 verification doc). That gives ~390 s for 1 000 matrices — well
//! above the 120 s slow-tier per-test budget.
//!
//! Per the issue implementation plan §7 (option 2), the criterion tests at
//! n=24 are left as-is (manual run, always `#[ignore = "external: ...]`)
//! and three smaller-n smoke tests at n=16 / 100 matrices are added to
//! demonstrate end-to-end dispatcher correctness in a feasible wall-clock.
//!
//! # CPU reference for F_7 at n=24
//!
//! The CPU single-word path `permanent_bipedal7_singleword` is limited to
//! `n <= 16` (Packed7::LANES). For the n=24 criterion test we use
//! `permanent_ryser::<Fp<7>>` as the reference oracle — it is the
//! correctness SSOT for the entire permanent family and works for any n.
//!
//! # RNG
//!
//! All random matrices are generated via [`gf2_algebra::testutil::random_matrix`]
//! (the workspace SSOT for deterministic mod-P matrix generation via
//! `gf2_core::rng::Lcg`). The pinned seed is `0xDEAD_BEEF_u64`.

#![cfg(feature = "hip")]

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_algebra::testutil::random_matrix;
use gf2_core::gfp::Fp;

#[cfg(feature = "f5")]
use gf2_algebra::packed::Packed5Matrix;
#[cfg(feature = "f5")]
use gf2_algebra::permanent::permanent_bipedal5;

#[cfg(feature = "f7")]
use gf2_algebra::packed::Packed7Matrix;
#[cfg(feature = "f7")]
use gf2_algebra::permanent::ryser::permanent_ryser;

/// Matrix dimension for the criterion test (1 000 matrices at n=24).
///
/// timing: ~390 s per prime on gfx1030 (all 1 000 blocks in parallel;
/// `ceil(1000/36) ≈ 28` waves × ~14 s/wave). Run manually only.
const N: usize = 24;
const M: usize = 1_000;
const SEED: u64 = 0xDEAD_BEEF_u64;

/// Matrix dimension for the end-to-end smoke tests (n=16, 100 matrices).
///
/// timing: 2^16 = 65 536 Gray steps × 100 blocks — completes in < 1 s on
/// gfx1030. Demonstrates dispatcher correctness at a feasible size (plan §7
/// option 2).
const N_SMOKE: usize = 16;
const M_SMOKE: usize = 100;
const SEED_SMOKE: u64 = 0xC0DE_CAFE_BEEF_5555_u64;

// ---------------------------------------------------------------------------
// Criterion tests: 1 000 matrices at n=24 (manual run, slow)
// ---------------------------------------------------------------------------

/// Batch GPU dispatcher for F_3: 1 000 random 24×24 matrices (criterion test).
///
/// Generates M = 1 000 random matrices at n = 24 using the workspace-SSOT
/// `random_matrix::<3>` with seed 0xDEAD_BEEF, computes permanents both via
/// the GPU batch dispatcher and the CPU `permanent_bipedal3`, and asserts
/// element-wise bit identity.
///
/// # Timing
///
/// Estimated ~390 s on gfx1030 for 1 000 matrices at n=24 (all blocks run
/// in parallel; `ceil(1000/36) ≈ 28` waves × ~14 s/wave). Run manually.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal3_matches_cpu_n24() {
    let mut gpu_inputs: Vec<Bipedal3Matrix> = Vec::with_capacity(M);
    let mut cpu_results: Vec<Fp<3>> = Vec::with_capacity(M);

    for trial in 0..M {
        // Deterministic seed derived from the base seed and trial index.
        let seed = SEED.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<3>(N, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, N, N);
        cpu_results.push(permanent_bipedal3(&mat));
        gpu_inputs.push(Bipedal3Matrix::from_row_major(&row_major, N, N));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal3(&gpu_inputs);
    assert_eq!(
        gpu_results.len(),
        M,
        "GPU returned {} results, expected {M}",
        gpu_results.len()
    );

    for i in 0..M {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={N})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

/// Batch GPU dispatcher for F_5: 1 000 random 24×24 matrices (criterion test).
///
/// Generates M = 1 000 random matrices at n = 24 using
/// `random_matrix::<5>` with seed 0xDEAD_BEEF, computes permanents via
/// the GPU batch dispatcher and `permanent_bipedal5`, and asserts element-wise
/// bit identity.
///
/// # Timing
///
/// Similar to F_3: estimated ~390 s on gfx1030. Run manually.
#[cfg(feature = "f5")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal5_matches_cpu_n24() {
    let mut gpu_inputs: Vec<Packed5Matrix> = Vec::with_capacity(M);
    let mut cpu_results: Vec<Fp<5>> = Vec::with_capacity(M);

    for trial in 0..M {
        let seed = SEED.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<5>(N, seed);
        let mat = Packed5Matrix::from_row_major(&row_major, N, N);
        cpu_results.push(permanent_bipedal5(&mat));
        gpu_inputs.push(Packed5Matrix::from_row_major(&row_major, N, N));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal5(&gpu_inputs);
    assert_eq!(
        gpu_results.len(),
        M,
        "GPU returned {} results, expected {M}",
        gpu_results.len()
    );

    for i in 0..M {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={N})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

/// Batch GPU dispatcher for F_7: 1 000 random 24×24 matrices (criterion test).
///
/// Generates M = 1 000 random matrices at n = 24 using
/// `random_matrix::<7>` with seed 0xDEAD_BEEF. Uses `permanent_ryser::<Fp<7>>`
/// as the CPU reference because `permanent_bipedal7` (the single-word fast
/// path) is limited to n <= 16 = Packed7::LANES. `permanent_ryser` is the
/// correctness SSOT for the entire permanent family and handles any n.
///
/// # Timing
///
/// F_7 GPU kernel is similar in complexity to F_3/F_5 (same Gray-code walk,
/// LUT-based arithmetic); estimated ~390 s on gfx1030. Run manually.
#[cfg(feature = "f7")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal7_matches_cpu_n24() {
    let mut gpu_inputs: Vec<Packed7Matrix> = Vec::with_capacity(M);
    let mut cpu_results: Vec<Fp<7>> = Vec::with_capacity(M);

    for trial in 0..M {
        let seed = SEED.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<7>(N, seed);
        // CPU reference: permanent_ryser works for any n; permanent_bipedal7
        // is limited to n <= 16. Correctness of permanent_ryser is the SSOT
        // established by T7 and used by every other cross-check in this crate.
        cpu_results.push(permanent_ryser::<Fp<7>>(&row_major, N));
        let mat = Packed7Matrix::from_row_major(&row_major, N, N);
        gpu_inputs.push(mat);
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal7(&gpu_inputs);
    assert_eq!(
        gpu_results.len(),
        M,
        "GPU returned {} results, expected {M}",
        gpu_results.len()
    );

    for i in 0..M {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={N})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

// ---------------------------------------------------------------------------
// End-to-end smoke tests at n=16 (100 matrices)
//
// These smaller-n tests demonstrate end-to-end dispatcher correctness in a
// wall-clock that fits the manual-run ignoring budget (~seconds on gfx1030).
// They use the same dispatcher code as the n=24 criterion tests but at a
// dimension where the Gray walk (2^16 = 65 536 steps per block) completes
// quickly. Added per the implementation plan §7 option 2.
// ---------------------------------------------------------------------------

/// F_3 batch dispatcher smoke test: 100 random 16×16 matrices.
///
/// timing: ~65 K Gray steps per block × 100 blocks — all complete in < 1 s on
/// gfx1030. Demonstrates end-to-end dispatcher correctness at a feasible size.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal3_smoke_n16() {
    let mut gpu_inputs: Vec<Bipedal3Matrix> = Vec::with_capacity(M_SMOKE);
    let mut cpu_results: Vec<Fp<3>> = Vec::with_capacity(M_SMOKE);

    for trial in 0..M_SMOKE {
        let seed = SEED_SMOKE.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<3>(N_SMOKE, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE);
        cpu_results.push(permanent_bipedal3(&mat));
        gpu_inputs.push(Bipedal3Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal3(&gpu_inputs);
    assert_eq!(gpu_results.len(), M_SMOKE);
    for i in 0..M_SMOKE {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "smoke mismatch at matrix {i}: GPU={} CPU={} (n={N_SMOKE})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

/// F_5 batch dispatcher smoke test: 100 random 16×16 matrices.
///
/// timing: < 1 s on gfx1030 (same 2^16 Gray-walk cost as F_3 at n=16).
#[cfg(feature = "f5")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal5_smoke_n16() {
    let mut gpu_inputs: Vec<Packed5Matrix> = Vec::with_capacity(M_SMOKE);
    let mut cpu_results: Vec<Fp<5>> = Vec::with_capacity(M_SMOKE);

    for trial in 0..M_SMOKE {
        let seed = SEED_SMOKE.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<5>(N_SMOKE, seed);
        let mat = Packed5Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE);
        cpu_results.push(permanent_bipedal5(&mat));
        gpu_inputs.push(Packed5Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal5(&gpu_inputs);
    assert_eq!(gpu_results.len(), M_SMOKE);
    for i in 0..M_SMOKE {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "smoke mismatch at matrix {i}: GPU={} CPU={} (n={N_SMOKE})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

/// F_7 batch dispatcher smoke test: 100 random 16×16 matrices.
///
/// Uses `permanent_bipedal7` (CPU single-word, limited to n ≤ 16) as the
/// reference because n=16 = Packed7::LANES is exactly the CPU fast-path limit.
///
/// timing: < 1 s on gfx1030.
#[cfg(feature = "f7")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal7_smoke_n16() {
    use gf2_algebra::permanent::permanent_bipedal7;

    let mut gpu_inputs: Vec<Packed7Matrix> = Vec::with_capacity(M_SMOKE);
    let mut cpu_results: Vec<Fp<7>> = Vec::with_capacity(M_SMOKE);

    for trial in 0..M_SMOKE {
        let seed = SEED_SMOKE.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<7>(N_SMOKE, seed);
        let mat = Packed7Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE);
        cpu_results.push(permanent_bipedal7(&mat));
        gpu_inputs.push(Packed7Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal7(&gpu_inputs);
    assert_eq!(gpu_results.len(), M_SMOKE);
    for i in 0..M_SMOKE {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "smoke mismatch at matrix {i}: GPU={} CPU={} (n={N_SMOKE})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}
