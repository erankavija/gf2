//! Integration test: shared behavioural suite for the permanental-rank
//! decision (jit issue `175972df`).
//!
//! Two implementations answer the same question — does an `n × k` matrix over
//! `F_q` with `k ≤ n` satisfy `per-rank(A) < k`? — and every case below runs
//! through both:
//!
//! * [`gf2_algebra::permanent::permanental_rank_status`], the production
//!   predicate: lexicographic row-subset enumeration, `permanent_ryser` per
//!   submatrix, early exit at the first nonzero permanent;
//! * [`gf2_algebra::testutil::permanental_rank_bruteforce`], the oracle: a
//!   `2^n` bitmask scan with a direct `S_k` permutation-sum permanent and no
//!   early exit, sharing no code path with the predicate.
//!
//! Covers:
//!
//! 1. **Exhaustive agreement** — every one of the `q^(n·k)` matrices for
//!    `(q, n, k) ∈ {(3,3,1), (3,3,2), (3,4,2), (5,3,2), (7,3,2)}` is decided by
//!    both routines and the decisions must match. Split one test per triple so
//!    each stays inside the fast tier's five-second per-test kill.
//!
//! 2. **Hand-constructed boundary vectors** — a zero row, a zero column,
//!    `k = 1`, `k = n`, and the matrix that separates a vanishing scalar
//!    rectangular permanent from permanental rank deficiency.
//!
//! # The two quantities the boundary section separates
//!
//! For an `n × k` matrix with `k ≤ n`, the scalar rectangular permanent is the
//! sum over injections from the `k` columns into the `n` rows, which regroups
//! as `sum over k-subsets S of rows of perm(A_S)`. Permanental rank
//! deficiency asks instead whether *every* `perm(A_S)` vanishes. A sum of
//! nonzero terms can vanish, so the two are different questions;
//! [`rectangular_permanent`] below computes the first one so the distinction is
//! asserted rather than described.
//!
//! # Integration-test boundary
//!
//! This file compiles as a separate Cargo integration-test crate, so it reaches
//! the oracle through the `test-support` feature gate that exposes
//! [`gf2_algebra::testutil`] publicly. See `crates/gf2-algebra/Cargo.toml` for
//! the self-dev-dependency that auto-enables that feature under `cargo test`.

use gf2_algebra::permanent::{permanental_rank_status, PermanentalRank};
use gf2_algebra::testutil::permanental_rank_bruteforce;
use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a flat row-major `n × k` matrix over `F_P` from residue literals.
fn matrix<const P: u64>(values: &[u64]) -> Vec<Fp<P>> {
    values.iter().map(|&v| Fp::<P>::new(v)).collect()
}

/// Assert the predicate and the oracle agree, and that both return `expected`.
fn assert_both<const P: u64>(
    label: &str,
    values: &[u64],
    n: usize,
    k: usize,
    expected: PermanentalRank,
) {
    let a = matrix::<P>(values);
    let predicate = permanental_rank_status::<Fp<P>>(&a, n, k);
    let oracle = permanental_rank_bruteforce::<Fp<P>>(&a, n, k);
    assert_eq!(
        predicate, expected,
        "{label}: predicate returned {predicate:?}, expected {expected:?}"
    );
    assert_eq!(
        oracle, expected,
        "{label}: oracle returned {oracle:?}, expected {expected:?}"
    );
}

/// Decide every `n × k` matrix over `F_Q` with both routines and require the
/// decisions to match.
///
/// Iterates the `Q^(n·k)` matrices as a mixed-radix counter over the `n · k`
/// entries, least significant entry first.
fn assert_exhaustive_agreement<const Q: u64>(n: usize, k: usize) {
    let cells = n * k;
    let mut digits = vec![0u64; cells];
    let mut examined: u64 = 0;
    let mut deficient: u64 = 0;

    loop {
        let a: Vec<Fp<Q>> = digits.iter().map(|&d| Fp::<Q>::new(d)).collect();
        let predicate = permanental_rank_status::<Fp<Q>>(&a, n, k);
        let oracle = permanental_rank_bruteforce::<Fp<Q>>(&a, n, k);
        assert_eq!(
            predicate, oracle,
            "q={Q} n={n} k={k}: predicate {predicate:?} != oracle {oracle:?} on entries {digits:?}"
        );
        examined += 1;
        if predicate.is_deficient() {
            deficient += 1;
        }

        // Increment the mixed-radix counter; a full wrap ends the enumeration.
        let mut position = 0;
        loop {
            if position == cells {
                // Every matrix was visited exactly once.
                assert_eq!(
                    examined,
                    Q.pow(cells as u32),
                    "q={Q} n={n} k={k}: enumerated {examined} matrices, expected q^(n*k)"
                );
                // A deficient matrix exists at every shape tested here (the
                // all-zero matrix is one), and full-rank matrices dominate, so
                // neither routine can be trivially constant.
                assert!(
                    deficient > 0 && deficient < examined,
                    "q={Q} n={n} k={k}: {deficient} deficient of {examined} — the shape must \
                     exercise both decisions"
                );
                return;
            }
            digits[position] += 1;
            if digits[position] < Q {
                break;
            }
            digits[position] = 0;
            position += 1;
        }
    }
}

/// The scalar rectangular permanent of an `n × k` matrix with `k ≤ n`: the sum
/// over injections `sigma` from the `k` columns into the `n` rows of
/// `prod_j A[sigma(j), j]`.
///
/// This is **not** the permanental-rank event. It exists here so the boundary
/// test can pin both numbers on one matrix and show they disagree.
fn rectangular_permanent<const P: u64>(values: &[Fp<P>], n: usize, k: usize) -> Fp<P> {
    fn recurse<const P: u64>(
        values: &[Fp<P>],
        n: usize,
        k: usize,
        column: usize,
        used: &mut Vec<usize>,
        product: Fp<P>,
    ) -> Fp<P> {
        if column == k {
            return product;
        }
        let mut total = Fp::<P>::new(0);
        for row in 0..n {
            if used.contains(&row) {
                continue;
            }
            used.push(row);
            total += recurse::<P>(
                values,
                n,
                k,
                column + 1,
                used,
                product * values[row * k + column],
            );
            used.pop();
        }
        total
    }

    let mut used = Vec::with_capacity(k);
    recurse::<P>(values, n, k, 0, &mut used, Fp::<P>::new(1))
}

// ---------------------------------------------------------------------------
// Section 1 — Exhaustive agreement over every matrix (REQ-03)
// ---------------------------------------------------------------------------

/// `(q, n, k) = (3, 3, 1)`: all `3^3 = 27` matrices.
#[test]
fn test_exhaustive_agreement_q3_n3_k1() {
    assert_exhaustive_agreement::<3>(3, 1);
}

/// `(q, n, k) = (3, 3, 2)`: all `3^6 = 729` matrices.
#[test]
fn test_exhaustive_agreement_q3_n3_k2() {
    assert_exhaustive_agreement::<3>(3, 2);
}

/// `(q, n, k) = (3, 4, 2)`: all `3^8 = 6 561` matrices.
#[test]
fn test_exhaustive_agreement_q3_n4_k2() {
    assert_exhaustive_agreement::<3>(4, 2);
}

/// `(q, n, k) = (5, 3, 2)`: all `5^6 = 15 625` matrices.
#[test]
fn test_exhaustive_agreement_q5_n3_k2() {
    assert_exhaustive_agreement::<5>(3, 2);
}

/// `(q, n, k) = (7, 3, 2)`: all `7^6 = 117 649` matrices.
#[test]
fn test_exhaustive_agreement_q7_n3_k2() {
    assert_exhaustive_agreement::<7>(3, 2);
}

// ---------------------------------------------------------------------------
// Section 2 — Hand-constructed boundary cases (REQ-04)
// ---------------------------------------------------------------------------

/// Zero row, `n > k`: the two submatrices that contain row 1 have permanent 0,
/// but rows `{0, 2}` give `1 · 1 + 0 · 0 = 1`, so the rank is still full.
///
/// A zero row removes rows from consideration; it forces deficiency only when
/// too few rows survive, which the companion test below pins.
#[test]
fn test_boundary_zero_row_leaves_rank_full() {
    assert_both::<3>(
        "3x2 with zero middle row",
        &[
            1, 0, // row 0
            0, 0, // row 1 — zero row
            0, 1, // row 2
        ],
        3,
        2,
        PermanentalRank::Full,
    );
}

/// Zero row at `n = k`: the one available submatrix contains it, so its
/// permanent vanishes and the rank is deficient.
#[test]
fn test_boundary_zero_row_forces_deficiency_at_n_equals_k() {
    assert_both::<3>(
        "2x2 with zero second row",
        &[
            1, 2, // row 0
            0, 0, // row 1 — zero row
        ],
        2,
        2,
        PermanentalRank::Deficient,
    );
}

/// Zero column: every `k × k` submatrix inherits it, and a permanent with a
/// zero column has every permutation product zero, so the rank is deficient
/// however large `n` is.
#[test]
fn test_boundary_zero_column_is_deficient() {
    assert_both::<5>(
        "4x2 with zero second column",
        &[
            1, 0, // row 0
            2, 0, // row 1
            3, 0, // row 2
            4, 0, // row 3
        ],
        4,
        2,
        PermanentalRank::Deficient,
    );
}

/// `k = 1`, all-zero column: the `1 × 1` submatrices are the entries
/// themselves, so deficiency is exactly "the single column is zero".
#[test]
fn test_boundary_k_equals_one_all_zero_is_deficient() {
    assert_both::<7>(
        "4x1 all zero",
        &[0, 0, 0, 0],
        4,
        1,
        PermanentalRank::Deficient,
    );
}

/// `k = 1`, one nonzero entry anywhere in the column: that entry is a `1 × 1`
/// submatrix with nonzero permanent, so the rank is full.
#[test]
fn test_boundary_k_equals_one_single_nonzero_is_full() {
    assert_both::<7>(
        "4x1 nonzero last",
        &[0, 0, 0, 5],
        4,
        1,
        PermanentalRank::Full,
    );
}

/// `k = n`: exactly one submatrix exists, so the predicate reduces to
/// `perm(A) = 0`. Here `perm = 1·1·1 = 1` for the identity — full rank.
#[test]
fn test_boundary_k_equals_n_identity_is_full() {
    assert_both::<3>(
        "3x3 identity",
        &[
            1, 0, 0, //
            0, 1, 0, //
            0, 0, 1, //
        ],
        3,
        3,
        PermanentalRank::Full,
    );
}

/// `k = n`: the all-ones `3 × 3` matrix has permanent `3! = 6 = 0 mod 3`, and
/// it is the only submatrix, so the rank is deficient. Together with the
/// identity case above this pins that at `k = n` the predicate degenerates to
/// the square permanent test.
#[test]
fn test_boundary_k_equals_n_all_ones_is_deficient() {
    assert_both::<3>("3x3 all ones", &[1; 9], 3, 3, PermanentalRank::Deficient);
}

/// The scalar rectangular permanent vanishes while a `k × k` submatrix
/// permanent does not (REQ-04, and the assertion behind REQ-05).
///
/// Over `F_3` with
///
/// ```text
/// A = [[1, 0],
///      [0, 1],
///      [1, 1]]
/// ```
///
/// the three `2 × 2` row-submatrix permanents are
/// `perm(rows 0,1) = 1·1 + 0·0 = 1`, `perm(rows 0,2) = 1·1 + 0·1 = 1`, and
/// `perm(rows 1,2) = 0·1 + 1·1 = 1`. Each is nonzero, so `per-rank(A) = 2` is
/// full. The scalar rectangular permanent is their sum,
/// `1 + 1 + 1 = 3 = 0 mod 3`, so it vanishes. A test that checked the
/// rectangular permanent would report deficiency here and be wrong.
#[test]
fn test_rectangular_permanent_vanishes_but_submatrix_does_not() {
    let values = [
        1, 0, // row 0
        0, 1, // row 1
        1, 1, // row 2
    ];
    let a = matrix::<3>(&values);

    // Quantity 1 — the scalar rectangular permanent: zero.
    let rect = rectangular_permanent::<3>(&a, 3, 2);
    assert_eq!(
        rect,
        Fp::<3>::new(0),
        "the scalar rectangular permanent of A must vanish for this test to say anything"
    );
    assert!(rect.is_zero());

    // Quantity 2 — the individual 2x2 row-submatrix permanents: all nonzero.
    for (rows, sub) in [
        ([0usize, 1usize], [1u64, 0, 0, 1]),
        ([0, 2], [1, 0, 1, 1]),
        ([1, 2], [0, 1, 1, 1]),
    ] {
        let permanent = rectangular_permanent::<3>(&matrix::<3>(&sub), 2, 2);
        assert_eq!(
            permanent,
            Fp::<3>::new(1),
            "perm of rows {rows:?} must be 1, so no submatrix witnesses deficiency"
        );
    }

    // The predicate answers the rank question, not the rectangular-permanent
    // question, so it reports full rank.
    assert_both::<3>(
        "3x2 with vanishing rectangular permanent",
        &values,
        3,
        2,
        PermanentalRank::Full,
    );
}
