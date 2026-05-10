//! Integration test: test-vector suite for the `permanent_*` family (T11).
//!
//! Covers:
//!
//! 1. **Hand-checked vectors** — at least 12 small-n cases (n ∈ {1, 2, 3, 4})
//!    where all three implementations (`permanent_ryser<Fp<3>>`,
//!    `permanent_mod3_reference`, `permanent_bipedal3`) are compared against
//!    literal expected values derived by pen-and-paper or exhaustive enumeration.
//!
//! 2. **Random cross-check, default tier** — 1000 matrices for each n ∈ {4, 8,
//!    12}; all three implementations must agree on every case. Per-test wall-clock
//!    fits under 5 s (CI hard limit).
//!
//! 3. **Random cross-check n=16, slow tier** — 1000 matrices, three-way agreement;
//!    `#[ignore = "sim: ..."]`, not exercised by `--profile ci`.
//!
//! 4. **Large-n cross-check, slow tier** — n ∈ {20, 24}, T9 vs T8 oracle (100
//!    matrices each, split into sub-tests to fit the 120 s/test slow-tier budget).
//!
//! # RNG
//!
//! All random matrices use [`gf2_core::rng::Lcg`] — the workspace SSOT RNG.
//! Seed values are derived deterministically from the issue short ID
//! `0x1cd3_eb09` concatenated with n so that each per-n test stream is
//! independent and reproducible.
//!
//! # Integration-test boundary
//!
//! This file compiles as a **separate crate** (Cargo integration test), so it
//! cannot import `#[cfg(test)] mod testutil` from `gf2_algebra::lib`. The
//! `random_matrix_fp3` helper below is a local copy of the same logic — see
//! `testutil.rs` in the library for the canonical version.

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::{permanent_bipedal3, permanent_mod3_reference, permanent_ryser};
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

// ---------------------------------------------------------------------------
// Local matrix generator — mirrors testutil::random_matrix (SSOT in lib.rs)
// but is inlined here because integration tests are a separate compilation
// unit and cannot reach `#[cfg(test)] mod testutil`.
// ---------------------------------------------------------------------------

/// Generate a deterministic pseudo-random `n × n` matrix of [`Fp<3>`] elements,
/// row-major, using the workspace SSOT [`Lcg`] RNG seeded with `seed`.
fn random_matrix_fp3(n: usize, seed: u64) -> Vec<Fp<3>> {
    let mut rng = Lcg::new(seed);
    (0..n * n)
        .map(|_| Fp::<3>::new(rng.next_u64() % 3))
        .collect()
}

/// Convert a row-major `Fp<3>` slice to a [`Bipedal3Matrix`].
fn to_bipedal(row_major: &[Fp<3>], n: usize) -> Bipedal3Matrix {
    Bipedal3Matrix::from_row_major(row_major, n, n)
}

/// Assert all three implementations return `expected` for the given flat
/// row-major matrix of dimension `n`.
///
/// Prints a diagnostic label on mismatch for easy bisection.
fn assert_all_three(label: &str, row_major: &[Fp<3>], n: usize, expected: u64) {
    let exp = Fp::<3>::new(expected);
    let mat = to_bipedal(row_major, n);

    let r = permanent_ryser::<Fp<3>>(row_major, n);
    let m = permanent_mod3_reference(row_major, n);
    let b = permanent_bipedal3(&mat);

    assert_eq!(
        r, exp,
        "{label}: permanent_ryser expected {expected}, got {r:?}"
    );
    assert_eq!(
        m, exp,
        "{label}: permanent_mod3_reference expected {expected}, got {m:?}"
    );
    assert_eq!(
        b, exp,
        "{label}: permanent_bipedal3 expected {expected}, got {b:?}"
    );
}

// ---------------------------------------------------------------------------
// Section 1 — Hand-checked vectors (14 cases, ≥ 12 required)
// ---------------------------------------------------------------------------

/// Case 1: n=1, matrix [1]. permanent = 1 (trivial single-entry permanent).
#[test]
fn test_hand_checked_1x1_one() {
    let m = vec![Fp::<3>::new(1)];
    assert_all_three("1×1 [1]", &m, 1, 1);
}

/// Case 2: n=1, matrix [2]. permanent = 2.
#[test]
fn test_hand_checked_1x1_two() {
    let m = vec![Fp::<3>::new(2)];
    assert_all_three("1×1 [2]", &m, 1, 2);
}

/// Case 3: n=1, matrix [0]. permanent = 0.
#[test]
fn test_hand_checked_1x1_zero() {
    let m = vec![Fp::<3>::new(0)];
    assert_all_three("1×1 [0]", &m, 1, 0);
}

/// Case 4: n=2, I_2 identity. permanent = 1.
///
/// Only the identity permutation contributes: 1 * 1 = 1.
#[test]
fn test_hand_checked_2x2_identity() {
    // [[1,0],[0,1]]
    let m = vec![
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
    ];
    assert_all_three("2×2 I_2", &m, 2, 1);
}

/// Case 5: n=2, all-ones. permanent = 2! mod 3 = 2.
///
/// Both permutations contribute 1*1=1 each; total = 2.
#[test]
fn test_hand_checked_2x2_all_ones() {
    let m = vec![Fp::<3>::new(1); 4];
    assert_all_three("2×2 all-ones", &m, 2, 2);
}

/// Case 6: n=2, 2·I_2 = [[2,0],[0,2]]. permanent = 2*2 mod 3 = 4 mod 3 = 1.
///
/// Only the identity permutation contributes: 2 * 2 = 4 ≡ 1 (mod 3).
#[test]
fn test_hand_checked_2x2_scaled_identity() {
    // [[2,0],[0,2]]
    let m = vec![
        Fp::<3>::new(2),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(2),
    ];
    assert_all_three("2×2 2·I_2", &m, 2, 1);
}

/// Case 7: n=3, I_3 identity. permanent = 1.
///
/// Only σ=(0,1,2) contributes: 1*1*1=1.
#[test]
fn test_hand_checked_3x3_identity() {
    let mut m = vec![Fp::<3>::new(0); 9];
    for i in 0..3 {
        m[i * 3 + i] = Fp::<3>::new(1);
    }
    assert_all_three("3×3 I_3", &m, 3, 1);
}

/// Case 8: n=3, all-ones. permanent = 3! mod 3 = 6 mod 3 = 0.
///
/// All 6 permutations contribute 1 each; 6 ≡ 0 (mod 3).
#[test]
fn test_hand_checked_3x3_all_ones() {
    let m = vec![Fp::<3>::new(1); 9];
    assert_all_three("3×3 all-ones", &m, 3, 0);
}

/// Case 9: n=3, cyclic permutation matrix [[0,1,0],[0,0,1],[1,0,0]].
/// permanent = 1.
///
/// The matrix represents the cycle (0→1→2→0). Only one permutation has all
/// non-zero products: σ=(1,2,0) contributing M[0,1]*M[1,2]*M[2,0]=1*1*1=1.
#[test]
fn test_hand_checked_3x3_cyclic_permutation() {
    // Row 0: [0,1,0], Row 1: [0,0,1], Row 2: [1,0,0]
    let m = vec![
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
    ];
    assert_all_three("3×3 cyclic permutation", &m, 3, 1);
}

/// Case 10: n=3, paper case [[1,2,0],[2,1,2],[0,2,1]]. permanent = 0 (mod 3).
///
/// Pen-and-paper enumeration of all 6 permutations:
/// - σ=(0,1,2): A[0,0]*A[1,1]*A[2,2] = 1*1*1 = 1
/// - σ=(0,2,1): A[0,0]*A[1,2]*A[2,1] = 1*2*2 = 4 ≡ 1 (mod 3)
/// - σ=(1,0,2): A[0,1]*A[1,0]*A[2,2] = 2*2*1 = 4 ≡ 1 (mod 3)
/// - σ=(1,2,0): A[0,1]*A[1,2]*A[2,0] = 2*2*0 = 0
/// - σ=(2,0,1): A[0,2]*A[1,0]*A[2,1] = 0*2*2 = 0
/// - σ=(2,1,0): A[0,2]*A[1,1]*A[2,0] = 0*1*0 = 0
///
/// Sum = 1+1+1+0+0+0 = 3 ≡ 0 (mod 3).
#[test]
fn test_hand_checked_3x3_paper_case() {
    // [[1,2,0],[2,1,2],[0,2,1]]
    let m = vec![
        Fp::<3>::new(1),
        Fp::<3>::new(2),
        Fp::<3>::new(0),
        Fp::<3>::new(2),
        Fp::<3>::new(1),
        Fp::<3>::new(2),
        Fp::<3>::new(0),
        Fp::<3>::new(2),
        Fp::<3>::new(1),
    ];
    assert_all_three("3×3 paper case", &m, 3, 0);
}

/// Case 11: n=4, I_4 identity. permanent = 1.
///
/// Only the identity permutation contributes: all diagonal entries are 1.
#[test]
fn test_hand_checked_4x4_identity() {
    let mut m = vec![Fp::<3>::new(0); 16];
    for i in 0..4 {
        m[i * 4 + i] = Fp::<3>::new(1);
    }
    assert_all_three("4×4 I_4", &m, 4, 1);
}

/// Case 12: n=4, all-ones. permanent = 4! mod 3 = 24 mod 3 = 0.
///
/// All 24 permutations contribute 1 each; 24 ≡ 0 (mod 3).
#[test]
fn test_hand_checked_4x4_all_ones() {
    let m = vec![Fp::<3>::new(1); 16];
    assert_all_three("4×4 all-ones", &m, 4, 0);
}

/// Case 13: n=4, pair-swapping permutation matrix.
/// [[0,1,0,0],[1,0,0,0],[0,0,0,1],[0,0,1,0]]. permanent = 1.
///
/// This represents σ=(1,0,3,2). The only non-zero permutation product corresponds
/// to σ itself: M[0,1]*M[1,0]*M[2,3]*M[3,2]=1*1*1*1=1.
#[test]
fn test_hand_checked_4x4_pair_swap_permutation() {
    // Rows: [0,1,0,0], [1,0,0,0], [0,0,0,1], [0,0,1,0]
    let m = vec![
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
    ];
    assert_all_three("4×4 pair-swap permutation", &m, 4, 1);
}

/// Case 14: n=4, band-diagonal [[1,1,0,0],[0,1,1,0],[0,0,1,1],[1,0,0,1]].
/// permanent = 2 (mod 3).
///
/// Pen-and-paper: M[0,j] nonzero iff j∈{0,1}; M[1,j] iff j∈{1,2};
/// M[2,j] iff j∈{2,3}; M[3,j] iff j∈{0,3}. Enumerating valid permutations
/// (each row's chosen column must be nonzero):
/// - σ=(1,2,3,0): σ(3)=0 ⟹ σ(0)=1 (only remaining from {0,1}), σ(1)=2, σ(2)=3
///   → product = M[0,1]*M[1,2]*M[2,3]*M[3,0] = 1*1*1*1 = 1
/// - σ=(0,1,2,3): σ(3)=3, σ(2)=2 (only remaining from {2,3}), σ(1)=1, σ(0)=0
///   → product = M[0,0]*M[1,1]*M[2,2]*M[3,3] = 1*1*1*1 = 1
///
/// All other permutation assignments yield at least one zero factor.
/// Sum = 1+1 = 2. (Verified by permanent_ryser as oracle.)
#[test]
fn test_hand_checked_4x4_band_diagonal() {
    // Rows: [1,1,0,0], [0,1,1,0], [0,0,1,1], [1,0,0,1]
    let m = vec![
        Fp::<3>::new(1),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
        Fp::<3>::new(1),
        Fp::<3>::new(1),
        Fp::<3>::new(0),
        Fp::<3>::new(0),
        Fp::<3>::new(1),
    ];
    assert_all_three("4×4 band-diagonal", &m, 4, 2);
}

// ---------------------------------------------------------------------------
// Section 2 — Random cross-check, default tier (n ∈ {4, 8, 12})
//
// 1000 matrices each; all three implementations must agree.
// Seed base derived from the issue short ID 0x1cd3_eb09.
// Per-test wall-clock must fit under the 5 s CI hard limit.
// ---------------------------------------------------------------------------

/// Random cross-check n=4, default tier: 1000 matrices, three-way agreement.
///
/// Seed base: 0x1cd3_eb09_0000_0004 (issue ID salt + n).
#[test]
fn test_cross_check_random_n4_three_way() {
    let n = 4usize;
    let seed_base: u64 = 0x1cd3_eb09_0000_0000_u64.wrapping_add(n as u64);
    for trial in 0u64..1000 {
        let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
        let row_major = random_matrix_fp3(n, seed);
        let mat = to_bipedal(&row_major, n);
        let r = permanent_ryser::<Fp<3>>(&row_major, n);
        let m = permanent_mod3_reference(&row_major, n);
        let b = permanent_bipedal3(&mat);
        assert_eq!(
            r, m,
            "T7 vs T8 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
        assert_eq!(
            m, b,
            "T8 vs T9 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
    }
}

/// Random cross-check n=8, default tier: 1000 matrices, three-way agreement.
///
/// Seed base: 0x1cd3_eb09_0000_0008 (issue ID salt + n).
#[test]
fn test_cross_check_random_n8_three_way() {
    let n = 8usize;
    let seed_base: u64 = 0x1cd3_eb09_0000_0000_u64.wrapping_add(n as u64);
    for trial in 0u64..1000 {
        let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
        let row_major = random_matrix_fp3(n, seed);
        let mat = to_bipedal(&row_major, n);
        let r = permanent_ryser::<Fp<3>>(&row_major, n);
        let m = permanent_mod3_reference(&row_major, n);
        let b = permanent_bipedal3(&mat);
        assert_eq!(
            r, m,
            "T7 vs T8 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
        assert_eq!(
            m, b,
            "T8 vs T9 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
    }
}

/// Random cross-check n=12, default tier: 1000 matrices, three-way agreement.
///
/// Seed base: 0x1cd3_eb09_0000_000c (issue ID salt + n).
/// Wall-clock budget: n=12 Ryser uses 2^12-1 = 4095 Gray steps per matrix;
/// 1000 matrices × ~0.4 ms/matrix ≈ 0.4 s total — fits the 5 s CI limit.
#[test]
fn test_cross_check_random_n12_three_way() {
    let n = 12usize;
    let seed_base: u64 = 0x1cd3_eb09_0000_0000_u64.wrapping_add(n as u64);
    for trial in 0u64..1000 {
        let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
        let row_major = random_matrix_fp3(n, seed);
        let mat = to_bipedal(&row_major, n);
        let r = permanent_ryser::<Fp<3>>(&row_major, n);
        let m = permanent_mod3_reference(&row_major, n);
        let b = permanent_bipedal3(&mat);
        assert_eq!(
            r, m,
            "T7 vs T8 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
        assert_eq!(
            m, b,
            "T8 vs T9 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
    }
}

// ---------------------------------------------------------------------------
// Section 3 — Random cross-check n=16, slow tier
// ---------------------------------------------------------------------------

/// Random cross-check n=16, slow tier: 1000 matrices, three-way agreement.
///
/// Seed base: 0x1cd3_eb09_0000_0010 (issue ID salt + n=16).
/// Wall-clock: n=16 Ryser uses 2^16-1=65535 Gray steps; ~5 s/matrix × 1000
/// exceeds the CI 5 s/test limit → must be slow tier.
#[test]
#[ignore = "sim: 1000-matrix three-way cross-check at n=16 (slow tier)"]
fn test_cross_check_random_n16_three_way_slow() {
    let n = 16usize;
    let seed_base: u64 = 0x1cd3_eb09_0000_0000_u64.wrapping_add(n as u64);
    for trial in 0u64..1000 {
        let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
        let row_major = random_matrix_fp3(n, seed);
        let mat = to_bipedal(&row_major, n);
        let r = permanent_ryser::<Fp<3>>(&row_major, n);
        let m = permanent_mod3_reference(&row_major, n);
        let b = permanent_bipedal3(&mat);
        assert_eq!(
            r, m,
            "T7 vs T8 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
        assert_eq!(
            m, b,
            "T8 vs T9 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
    }
}

// ---------------------------------------------------------------------------
// Section 4 — Large-n cross-check, slow tier (n ∈ {20, 24})
//
// Oracle: `permanent_mod3_reference` (T8). Correctness of T8 vs T7
// (`permanent_ryser`) is established by T8's own 12k-matrix cross-check;
// transitivity closes the loop for T11 here.
//
// n=20: ~5 s/matrix × 20 matrices/chunk = ~100 s/chunk < 120 s slow-tier limit.
//   5 chunks × 20 matrices = 100 total.
// n=24: ~8 s/matrix × 10 matrices/chunk = ~80 s/chunk < 120 s slow-tier limit.
//   10 chunks × 10 matrices = 100 total.
//
// Seed derivation mirrors T9's `large_n_cross_check!` macro
// (crates/gf2-algebra/src/permanent/bipedal3.rs:382-407):
//   seed_base = 0x1cd3_eb09_<n>000_0000 + seed_salt
//   seed_i = seed_base + trial * 1_000_003
// ---------------------------------------------------------------------------

/// Run `trials` T9-vs-T8 cross-checks for dimension `n`, starting from
/// `seed_salt`. Private helper used by the large-n slow-tier tests below.
fn cross_check_n_chunk(n: usize, seed_salt: u64, trials: u64) {
    // Seed base: issue ID prefix + n in low byte + salt offset.
    // Mirrors T9's large_n_cross_check! shape for consistency.
    let seed_base: u64 = 0x1cd3_eb09_0000_0000_u64
        .wrapping_add((n as u64) << 24)
        .wrapping_add(seed_salt);
    for trial in 0..trials {
        let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
        let row_major = random_matrix_fp3(n, seed);
        let mat = to_bipedal(&row_major, n);
        let expected = permanent_mod3_reference(&row_major, n);
        let actual = permanent_bipedal3(&mat);
        assert_eq!(
            actual, expected,
            "T9 vs T8 mismatch: n={n} trial={trial} seed={seed:#018x}"
        );
    }
}

// n=20 — 5 sub-tests × 20 matrices = 100 total.

/// Large-n cross-check n=20 chunk A (matrices 0–19), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=20 chunk A (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n20_chunk_a() {
    cross_check_n_chunk(20, 0, 20);
}

/// Large-n cross-check n=20 chunk B (matrices 20–39), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=20 chunk B (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n20_chunk_b() {
    cross_check_n_chunk(20, 1_000, 20);
}

/// Large-n cross-check n=20 chunk C (matrices 40–59), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=20 chunk C (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n20_chunk_c() {
    cross_check_n_chunk(20, 2_000, 20);
}

/// Large-n cross-check n=20 chunk D (matrices 60–79), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=20 chunk D (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n20_chunk_d() {
    cross_check_n_chunk(20, 3_000, 20);
}

/// Large-n cross-check n=20 chunk E (matrices 80–99), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=20 chunk E (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n20_chunk_e() {
    cross_check_n_chunk(20, 4_000, 20);
}

// n=24 — 10 sub-tests × 10 matrices = 100 total.

/// Large-n cross-check n=24 chunk A (matrices 0–9), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk A (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_a() {
    cross_check_n_chunk(24, 0, 10);
}

/// Large-n cross-check n=24 chunk B (matrices 10–19), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk B (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_b() {
    cross_check_n_chunk(24, 1_000, 10);
}

/// Large-n cross-check n=24 chunk C (matrices 20–29), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk C (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_c() {
    cross_check_n_chunk(24, 2_000, 10);
}

/// Large-n cross-check n=24 chunk D (matrices 30–39), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk D (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_d() {
    cross_check_n_chunk(24, 3_000, 10);
}

/// Large-n cross-check n=24 chunk E (matrices 40–49), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk E (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_e() {
    cross_check_n_chunk(24, 4_000, 10);
}

/// Large-n cross-check n=24 chunk F (matrices 50–59), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk F (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_f() {
    cross_check_n_chunk(24, 5_000, 10);
}

/// Large-n cross-check n=24 chunk G (matrices 60–69), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk G (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_g() {
    cross_check_n_chunk(24, 6_000, 10);
}

/// Large-n cross-check n=24 chunk H (matrices 70–79), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk H (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_h() {
    cross_check_n_chunk(24, 7_000, 10);
}

/// Large-n cross-check n=24 chunk I (matrices 80–89), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk I (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_i() {
    cross_check_n_chunk(24, 8_000, 10);
}

/// Large-n cross-check n=24 chunk J (matrices 90–99), slow tier.
#[test]
#[ignore = "sim: large-n cross-check n=24 chunk J (slow tier, T9 vs T8 oracle)"]
fn test_cross_check_random_n24_chunk_j() {
    cross_check_n_chunk(24, 9_000, 10);
}
