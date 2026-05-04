/*
 * benchmarks/reference/m4ri_bench.c
 *
 * Reference reproducibility harness for M4RI on GF(2). Emits CSV rows on
 * stdout in the same schema as fflas_bench.cpp:
 *
 *   lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
 *
 * The seed scheme is identical to the C++ harness (SplitMix64 keyed by
 * the master seed) so that gf2's own M4RM benchmarks can fill the same
 * matrices when the master seed is shared.
 *
 * Operations:
 *   * matmul  (mzd_mul / Method-of-the-Four-Russians) at n=64..4096
 *   * echelon (mzd_echelonize_m4ri, full RREF) at n=64..1024
 *   * pluq    (mzd_pluq) at n=64..1024  -- issue 5dea7457
 *   * invert  (mzd_inv_m4ri) at n=64..1024 -- issue 5dea7457
 *   * solve   (mzd_solve_left, single RHS column) at n=64..1024
 *             -- issue 5dea7457
 *
 * charpoly and minpoly are not provided by M4RI and are excluded from
 * this lane (`not-supported-by-library` exclusion class per
 * `dev/plans/sota_reference_acceptance_protocol.md` § 8).
 *
 * Build:
 *   gcc -O3 -march=native -std=c11 m4ri_bench.c -lm4ri -o m4ri_bench
 *
 * The container Makefile compiles this with the same flags.
 *
 * CLI:
 *   m4ri_bench [--seed N] [--warmup K] [--iters K] [--smoke]
 *
 * `--smoke` runs each new operation at n=16 against a fixed seeded
 * input and asserts the per-operation correctness contract from
 * `dev/plans/sota_reference_acceptance_protocol.md` § 6:
 *   * pluq:   reconstruct P*L*U == A and verify rank.
 *   * invert: A * A^{-1} == I.
 *   * solve:  A * x == b for a square full-rank system.
 * Any failure exits with status 1 so the smoke gate fails fast.
 */

/* Need _POSIX_C_SOURCE >= 199309L for clock_gettime / CLOCK_MONOTONIC under
 * the strict c11 + glibc combination the container's gcc-12 ships with. */
#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>

#include <m4ri/m4ri.h>

#include "seed_helpers.h"

/* Local thin wrappers around the shared helpers (so call-sites read
 * naturally and the inlined definitions don't blow up the diff). */
static inline uint64_t splitmix64(uint64_t* state) {
    return gf2_bench_splitmix64(state);
}

static inline uint64_t derive_seed(uint64_t master, const char* tag,
                                   uint64_t op_idx, uint64_t size_idx,
                                   uint64_t regime_idx) {
    return gf2_bench_derive_seed(master, tag, op_idx, size_idx, regime_idx);
}

/* Fill an m×n GF(2) matrix with deterministic random bits. */
static void fill_uniform_gf2(mzd_t* A, uint64_t seed) {
    rci_t m = A->nrows;
    rci_t n = A->ncols;
    uint64_t st = seed;
    for (rci_t r = 0; r < m; ++r) {
        for (rci_t c = 0; c < n; ++c) {
            uint64_t v = splitmix64(&st);
            mzd_write_bit(A, r, c, (v & 1ULL));
        }
    }
}

/* Build a rank-`rank` m×n GF(2) matrix as L*R where L is m×rank and
 * R is rank×n, both uniform. mzd_mul defaults to "k=0" auto-tuning. */
static mzd_t* alloc_rank_deficient(rci_t m, rci_t n, rci_t rank,
                                   uint64_t seed) {
    mzd_t* L = mzd_init(m, rank);
    mzd_t* R = mzd_init(rank, n);
    fill_uniform_gf2(L, seed ^ 0xA5A5A5A5A5A5A5A5ULL);
    fill_uniform_gf2(R, seed ^ 0x5A5A5A5A5A5A5A5AULL);
    mzd_t* A = mzd_init(m, n);
    mzd_mul(A, L, R, 0);
    mzd_free(L);
    mzd_free(R);
    return A;
}

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static void emit_csv(const char* op,
                     size_t m, size_t k, size_t n,
                     const char* regime,
                     uint64_t seed,
                     uint64_t wall_ns,
                     double throughput_ops) {
    printf("m4ri,%s,GF(2),%zu,%zu,%zu,%s,%llu,%llu,%.6e\n",
           op, m, k, n, regime,
           (unsigned long long)seed,
           (unsigned long long)wall_ns,
           throughput_ops);
    fflush(stdout);
}

static void bench_matmul(rci_t n, const char* regime, uint64_t seed,
                         int warmup, int iters) {
    rci_t rank = (strcmp(regime, "deficient") == 0) ? n / 2 : n;

    mzd_t* A = (rank == n) ? mzd_init(n, n)
                           : alloc_rank_deficient(n, n, rank, seed);
    mzd_t* B = mzd_init(n, n);
    mzd_t* C = mzd_init(n, n);
    if (rank == n) fill_uniform_gf2(A, seed);
    fill_uniform_gf2(B, seed ^ 0x1111111111111111ULL);

    for (int i = 0; i < warmup; ++i) {
        /* k=0 lets M4RI pick the optimal Method-of-Four-Russians block. */
        mzd_mul(C, A, B, 0);
    }

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        mzd_mul(C, A, B, 0);
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / (uint64_t)iters;
    /* GF(2) matmul: 2 n^3 bit-ops dominant term. */
    double tput = 2.0 * (double)n * (double)n * (double)n
                  / ((double)mean_ns * 1.0e-9);

    emit_csv("matmul", n, n, n, regime, seed, mean_ns, tput);

    mzd_free(A);
    mzd_free(B);
    mzd_free(C);
}

/* PLUQ benchmark (issue 5dea7457).
 *
 * `mzd_pluq(A, P, Q, 0)` factors A in place: L (unit lower) below the
 * diagonal, U (upper, including the diagonal) on/above. P and Q are
 * row/column permutations in LAPACK swap-list format. We restore A
 * from the unmodified A0 each iteration so warmup runs cannot pollute
 * subsequent timings.
 *
 * Conventional dominant-term op count: n^3.
 */
static void bench_pluq(rci_t n, const char* regime, uint64_t seed,
                       int warmup, int iters) {
    rci_t rank_target = (strcmp(regime, "deficient") == 0) ? n / 2 : n;

    mzd_t* A0 = (rank_target == n) ? mzd_init(n, n)
                                   : alloc_rank_deficient(n, n, rank_target, seed);
    if (rank_target == n) fill_uniform_gf2(A0, seed);
    mzd_t* A = mzd_init(n, n);
    mzp_t* P = mzp_init(n);
    mzp_t* Q = mzp_init(n);

    for (int i = 0; i < warmup; ++i) {
        mzd_copy(A, A0);
        mzp_set_ui(P, 1);
        mzp_set_ui(Q, 1);
        (void)mzd_pluq(A, P, Q, 0);
    }

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        mzd_copy(A, A0);
        mzp_set_ui(P, 1);
        mzp_set_ui(Q, 1);
        uint64_t t0 = monotonic_ns();
        (void)mzd_pluq(A, P, Q, 0);
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / (uint64_t)iters;
    double tput = (double)n * (double)n * (double)n
                  / ((double)mean_ns * 1.0e-9);

    emit_csv("pluq", n, n, n, regime, seed, mean_ns, tput);

    mzp_free(P);
    mzp_free(Q);
    mzd_free(A0);
    mzd_free(A);
}

/* Invert benchmark (issue 5dea7457).
 *
 * `mzd_inv_m4ri(NULL, A, 0)` returns a freshly-allocated inverse matrix.
 * In the deficient regime the input is singular; mzd_inv_m4ri may
 * return NULL or an undefined matrix. We treat both as the timed
 * outcome -- the timing reflects the cost of the (Konrod) inversion
 * pass plus the singularity-detection branch. When the result is
 * non-NULL we free it; when NULL we record that fact via the
 * SINGULAR stderr breadcrumb.
 *
 * Conventional dominant-term op count: n^3.
 */
static void bench_invert(rci_t n, const char* regime, uint64_t seed,
                         int warmup, int iters) {
    rci_t rank_target = (strcmp(regime, "deficient") == 0) ? n / 2 : n;

    mzd_t* A0 = (rank_target == n) ? mzd_init(n, n)
                                   : alloc_rank_deficient(n, n, rank_target, seed);
    if (rank_target == n) fill_uniform_gf2(A0, seed);

    for (int i = 0; i < warmup; ++i) {
        mzd_t* inv = mzd_inv_m4ri(NULL, A0, 0);
        if (inv != NULL) mzd_free(inv);
    }

    uint64_t total_ns = 0;
    int singular_count = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        mzd_t* inv = mzd_inv_m4ri(NULL, A0, 0);
        total_ns += monotonic_ns() - t0;
        if (inv == NULL) {
            ++singular_count;
        } else {
            mzd_free(inv);
        }
    }
    uint64_t mean_ns = total_ns / (uint64_t)iters;
    double tput = (double)n * (double)n * (double)n
                  / ((double)mean_ns * 1.0e-9);

    if (rank_target != n && singular_count == 0) {
        fprintf(stderr,
                "[m4ri_bench] WARN deficient invert returned non-NULL "
                "all iterations field=GF(2) n=%d (expected SINGULAR)\n",
                (int)n);
    }
    if (singular_count > 0) {
        fprintf(stderr,
                "[m4ri_bench] SINGULAR invert n=%d regime=%s singular_iters=%d/%d\n",
                (int)n, regime, singular_count, iters);
    }

    emit_csv("invert", n, n, n, regime, seed, mean_ns, tput);

    mzd_free(A0);
}

/* Solve benchmark (issue 5dea7457).
 *
 * `mzd_solve_left(A, B, 0, 0)` solves A * X = B in place on B. We use
 * a single RHS column (B is n x 1) to match the fflas-ffpack solve
 * cell shape. Both A and B are overwritten by the call so we restore
 * them from frozen copies each iteration.
 *
 * Conventional dominant-term op count: n^3.
 */
static void bench_solve(rci_t n, const char* regime, uint64_t seed,
                        int warmup, int iters) {
    rci_t rank_target = (strcmp(regime, "deficient") == 0) ? n / 2 : n;

    mzd_t* A0 = (rank_target == n) ? mzd_init(n, n)
                                   : alloc_rank_deficient(n, n, rank_target, seed);
    if (rank_target == n) fill_uniform_gf2(A0, seed);
    mzd_t* B0 = mzd_init(n, 1);
    fill_uniform_gf2(B0, seed ^ 0xDEADBEEFCAFEBABEULL);

    mzd_t* A = mzd_init(n, n);
    mzd_t* B = mzd_init(n, 1);

    for (int i = 0; i < warmup; ++i) {
        mzd_copy(A, A0);
        mzd_copy(B, B0);
        (void)mzd_solve_left(A, B, 0, 1);
    }

    uint64_t total_ns = 0;
    int inconsistent_count = 0;
    for (int i = 0; i < iters; ++i) {
        mzd_copy(A, A0);
        mzd_copy(B, B0);
        uint64_t t0 = monotonic_ns();
        int rc = mzd_solve_left(A, B, 0, 1);
        total_ns += monotonic_ns() - t0;
        if (rc != 0) ++inconsistent_count;
    }
    uint64_t mean_ns = total_ns / (uint64_t)iters;
    double tput = (double)n * (double)n * (double)n
                  / ((double)mean_ns * 1.0e-9);

    if (inconsistent_count > 0) {
        fprintf(stderr,
                "[m4ri_bench] INCONSISTENT solve n=%d regime=%s iters=%d/%d\n",
                (int)n, regime, inconsistent_count, iters);
    }

    emit_csv("solve", n, n, n, regime, seed, mean_ns, tput);

    mzd_free(A0);
    mzd_free(B0);
    mzd_free(A);
    mzd_free(B);
}

/* ----- smoke equality oracles (issue 5dea7457) -----------------------
 *
 * Per `dev/plans/sota_reference_acceptance_protocol.md` § 6, every new
 * (operation, field, n=16) cell must satisfy a specific algebraic
 * equality contract. The smoke functions below enforce those contracts
 * inside the harness and exit non-zero on any violation so that
 * `benchmarks/smoke.sh` fails fast at the canonical n=16 cell.
 *
 * pluq:   reconstruct P*L*U == A_orig over GF(2); reported rank
 *         matches mzd_echelonize_m4ri on the same input.
 * invert: A * A^{-1} == I over GF(2).
 * solve:  A * x == b over GF(2) for a (presumed) full-rank A.
 */

static int smoke_pluq(uint64_t seed) {
    const rci_t n = 16;
    mzd_t* A0 = mzd_init(n, n);
    fill_uniform_gf2(A0, seed);

    /* Decompose. */
    mzd_t* A = mzd_init(n, n);
    mzd_copy(A, A0);
    mzp_t* P = mzp_init(n);
    mzp_t* Q = mzp_init(n);
    mzp_set_ui(P, 1);
    mzp_set_ui(Q, 1);
    rci_t rank_pluq = mzd_pluq(A, P, Q, 0);

    /* Independent rank check via reduced row-echelon form. */
    mzd_t* Ae = mzd_init(n, n);
    mzd_copy(Ae, A0);
    rci_t rank_echelon = mzd_echelonize_m4ri(Ae, 1, 0);
    mzd_free(Ae);

    if (rank_pluq != rank_echelon) {
        fprintf(stderr,
                "[m4ri_bench] SMOKE FAIL pluq rank mismatch "
                "n=%d pluq=%d echelon=%d\n",
                (int)n, (int)rank_pluq, (int)rank_echelon);
        mzp_free(P); mzp_free(Q); mzd_free(A); mzd_free(A0);
        return 1;
    }

    /* Reconstruct P*L*U*Q and compare with A0. M4RI's mzp_t is a
     * LAPACK-style swap-list, and the documented identity is
     * "PLUQ = A". Empirically (and consistent with linbox's pluq
     * test fixture), the reconstruction sequence is:
     *   apply_p_left_trans(LU, P) ; apply_p_right(LU, Q)
     * which yields P_mat * (L*U) * Q_mat == A0 where P_mat / Q_mat
     * are the permutation matrices that the swap-lists represent. */
    mzd_t* L  = mzd_extract_l(NULL, A);
    mzd_t* U  = mzd_extract_u(NULL, A);
    mzd_t* LU = mzd_mul(NULL, L, U, 0);
    mzd_apply_p_left_trans(LU, P);
    mzd_apply_p_right(LU, Q);

    int eq = mzd_equal(LU, A0);
    mzd_free(L); mzd_free(U); mzd_free(LU);
    mzp_free(P); mzp_free(Q); mzd_free(A); mzd_free(A0);

    if (!eq) {
        fprintf(stderr,
                "[m4ri_bench] SMOKE FAIL pluq P*L*U != A n=%d\n",
                (int)n);
        return 1;
    }
    fprintf(stderr,
            "[m4ri_bench] SMOKE OK pluq n=%d rank=%d\n",
            (int)n, (int)rank_pluq);
    return 0;
}

/* Build an invertible n x n GF(2) matrix from a seed. We construct
 * A = L * U where L is unit-lower-triangular with random strict-lower
 * entries and U is unit-upper-triangular with random strict-upper
 * entries. Both L and U are invertible by construction (det = 1 over
 * GF(2)) so A = L*U is invertible. This avoids the ~71% singular rate
 * of i.i.d. random GF(2) matrices that would otherwise flake the smoke
 * gate. */
static mzd_t* alloc_invertible_gf2(rci_t n, uint64_t seed) {
    mzd_t* L = mzd_init(n, n);
    mzd_t* U = mzd_init(n, n);
    /* Identity diagonals. */
    for (rci_t i = 0; i < n; ++i) {
        mzd_write_bit(L, i, i, 1);
        mzd_write_bit(U, i, i, 1);
    }
    /* Random strict-lower / strict-upper bits. */
    uint64_t st = seed;
    for (rci_t r = 0; r < n; ++r) {
        for (rci_t c = 0; c < r; ++c) {
            uint64_t v = splitmix64(&st);
            mzd_write_bit(L, r, c, (v & 1ULL));
        }
    }
    for (rci_t r = 0; r < n; ++r) {
        for (rci_t c = r + 1; c < n; ++c) {
            uint64_t v = splitmix64(&st);
            mzd_write_bit(U, r, c, (v & 1ULL));
        }
    }
    mzd_t* A = mzd_mul(NULL, L, U, 0);
    mzd_free(L);
    mzd_free(U);
    return A;
}

static int smoke_invert(uint64_t seed) {
    const rci_t n = 16;
    /* Constructed-invertible input avoids the ~71% singular rate of
     * a random GF(2) matrix at this size. */
    mzd_t* A = alloc_invertible_gf2(n, seed);

    mzd_t* inv = mzd_inv_m4ri(NULL, A, 0);
    if (inv == NULL) {
        fprintf(stderr,
                "[m4ri_bench] SMOKE FAIL invert returned NULL on uniform "
                "seed=0x%llx (singular outcome at n=16)\n",
                (unsigned long long)seed);
        mzd_free(A);
        return 1;
    }

    /* A * A^{-1} == I check. */
    mzd_t* prod = mzd_mul(NULL, A, inv, 0);
    mzd_t* I = mzd_init(n, n);
    mzd_set_ui(I, 1);  /* identity */
    int eq = mzd_equal(prod, I);
    mzd_free(prod); mzd_free(I); mzd_free(inv); mzd_free(A);

    if (!eq) {
        fprintf(stderr, "[m4ri_bench] SMOKE FAIL invert A*Ainv != I n=%d\n",
                (int)n);
        return 1;
    }
    fprintf(stderr, "[m4ri_bench] SMOKE OK invert n=%d\n", (int)n);
    return 0;
}

static int smoke_solve(uint64_t seed) {
    const rci_t n = 16;
    /* Constructed-invertible A so the smoke check measures the
     * solver's correctness on a guaranteed-consistent square system. */
    mzd_t* A0 = alloc_invertible_gf2(n, seed);
    mzd_t* b0 = mzd_init(n, 1);
    fill_uniform_gf2(b0, seed ^ 0xDEADBEEFCAFEBABEULL);

    mzd_t* A = mzd_init(n, n);
    mzd_t* x = mzd_init(n, 1);
    mzd_copy(A, A0);
    mzd_copy(x, b0);

    int rc = mzd_solve_left(A, x, 0, 1);
    if (rc != 0) {
        fprintf(stderr,
                "[m4ri_bench] SMOKE FAIL solve reported inconsistent "
                "system seed=0x%llx (singular at n=16)\n",
                (unsigned long long)seed);
        mzd_free(A); mzd_free(x); mzd_free(A0); mzd_free(b0);
        return 1;
    }

    /* A0 * x == b0 check. */
    mzd_t* prod = mzd_mul(NULL, A0, x, 0);
    int eq = mzd_equal(prod, b0);
    mzd_free(prod); mzd_free(A); mzd_free(x); mzd_free(A0); mzd_free(b0);

    if (!eq) {
        fprintf(stderr, "[m4ri_bench] SMOKE FAIL solve A*x != b n=%d\n",
                (int)n);
        return 1;
    }
    fprintf(stderr, "[m4ri_bench] SMOKE OK solve n=%d\n", (int)n);
    return 0;
}

static void bench_echelonize(rci_t n, const char* regime, uint64_t seed,
                             int warmup, int iters) {
    rci_t rank = (strcmp(regime, "deficient") == 0) ? n / 2 : n;

    mzd_t* A0 = (rank == n) ? mzd_init(n, n)
                            : alloc_rank_deficient(n, n, rank, seed);
    if (rank == n) fill_uniform_gf2(A0, seed);
    mzd_t* A = mzd_init(n, n);

    for (int i = 0; i < warmup; ++i) {
        mzd_copy(A, A0);
        /* full=1 -> reduced row-echelon form. */
        (void)mzd_echelonize_m4ri(A, 1, 0);
    }

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        mzd_copy(A, A0);
        uint64_t t0 = monotonic_ns();
        (void)mzd_echelonize_m4ri(A, 1, 0);
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / (uint64_t)iters;
    double tput = (double)n * (double)n * (double)n
                  / ((double)mean_ns * 1.0e-9);

    emit_csv("echelon", n, n, n, regime, seed, mean_ns, tput);

    mzd_free(A0);
    mzd_free(A);
}

int main(int argc, char** argv) {
    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    int warmup = 3;
    int iters  = 5;
    int smoke  = 0;

    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = strtoull(argv[++i], NULL, 0);
        } else if (strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = (int)strtol(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters = (int)strtol(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--smoke") == 0) {
            smoke = 1;
        } else {
            fprintf(stderr,
                    "usage: m4ri_bench [--seed N] [--warmup K] [--iters K] [--smoke]\n");
            return 2;
        }
    }

    fprintf(stderr,
            "[m4ri_bench] master_seed=0x%llx warmup=%d iters=%d smoke=%d\n",
            (unsigned long long)master_seed, warmup, iters, smoke);

    /* Smoke equality oracle for the new operations (issue 5dea7457).
     * Per `dev/plans/sota_reference_acceptance_protocol.md` § 6 we run
     * each new operation at n=16 against fixed-seeded inputs and assert
     * the operation-specific algebraic invariants. */
    if (smoke) {
        int rc = 0;
        rc |= smoke_pluq  (derive_seed(master_seed, "pluq_smoke",   2, 0, 0));
        rc |= smoke_invert(derive_seed(master_seed, "invert_smoke", 3, 0, 0));
        rc |= smoke_solve (derive_seed(master_seed, "solve_smoke",  4, 0, 0));
        if (rc != 0) {
            fprintf(stderr, "[m4ri_bench] smoke failed (rc=%d)\n", rc);
            return 1;
        }
        fprintf(stderr, "[m4ri_bench] smoke OK\n");
        return 0;
    }

    /* Sizes 64..1024 for the dense sweep + 4096 for matmul only.
     * Echelonize / pluq / invert / solve at 4096 are deferred to T2 to
     * keep the wall-clock budget of T1 reasonable. */
    static const rci_t MATMUL_SIZES[]   = {64, 256, 1024, 4096};
    static const rci_t ECHELON_SIZES[]  = {64, 256, 1024};
    /* PLUQ / Invert / Solve sizes (issue 5dea7457). */
    static const rci_t PLUQ_SIZES[]     = {64, 256, 1024};
    static const rci_t INVERT_SIZES[]   = {64, 256, 1024};
    static const rci_t SOLVE_SIZES[]    = {64, 256, 1024};

    for (size_t si = 0; si < sizeof(MATMUL_SIZES) / sizeof(MATMUL_SIZES[0]); ++si) {
        rci_t n = MATMUL_SIZES[si];
        bench_matmul(n, "uniform",
                     derive_seed(master_seed, "matmul", 0, (uint64_t)si, 0),
                     warmup, iters);
        bench_matmul(n, "deficient",
                     derive_seed(master_seed, "matmul", 0, (uint64_t)si, 1),
                     warmup, iters);
    }

    for (size_t si = 0; si < sizeof(ECHELON_SIZES) / sizeof(ECHELON_SIZES[0]); ++si) {
        rci_t n = ECHELON_SIZES[si];
        bench_echelonize(n, "uniform",
                         derive_seed(master_seed, "echelon", 1, (uint64_t)si, 0),
                         warmup, iters);
        bench_echelonize(n, "deficient",
                         derive_seed(master_seed, "echelon", 1, (uint64_t)si, 1),
                         warmup, iters);
    }

    /* New operations (issue 5dea7457). */
    for (size_t si = 0; si < sizeof(PLUQ_SIZES) / sizeof(PLUQ_SIZES[0]); ++si) {
        rci_t n = PLUQ_SIZES[si];
        bench_pluq(n, "uniform",
                   derive_seed(master_seed, "pluq", 2, (uint64_t)si, 0),
                   warmup, iters);
        bench_pluq(n, "deficient",
                   derive_seed(master_seed, "pluq", 2, (uint64_t)si, 1),
                   warmup, iters);
    }

    for (size_t si = 0; si < sizeof(INVERT_SIZES) / sizeof(INVERT_SIZES[0]); ++si) {
        rci_t n = INVERT_SIZES[si];
        bench_invert(n, "uniform",
                     derive_seed(master_seed, "invert", 3, (uint64_t)si, 0),
                     warmup, iters);
        bench_invert(n, "deficient",
                     derive_seed(master_seed, "invert", 3, (uint64_t)si, 1),
                     warmup, iters);
    }

    for (size_t si = 0; si < sizeof(SOLVE_SIZES) / sizeof(SOLVE_SIZES[0]); ++si) {
        rci_t n = SOLVE_SIZES[si];
        bench_solve(n, "uniform",
                    derive_seed(master_seed, "solve", 4, (uint64_t)si, 0),
                    warmup, iters);
        bench_solve(n, "deficient",
                    derive_seed(master_seed, "solve", 4, (uint64_t)si, 1),
                    warmup, iters);
    }

    return 0;
}
