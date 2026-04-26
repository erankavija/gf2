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
 * Operations: dense matmul (mzd_mul / Method-of-the-Four-Russians) at
 * sizes 64, 256, 1024, 4096, and rank-deficient matmul where one input
 * has rank n/2.
 *
 * Build:
 *   gcc -O3 -march=native -std=c11 m4ri_bench.c -lm4ri -o m4ri_bench
 *
 * The container Makefile compiles this with the same flags.
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

    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = strtoull(argv[++i], NULL, 0);
        } else if (strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = (int)strtol(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters = (int)strtol(argv[++i], NULL, 10);
        } else {
            fprintf(stderr,
                    "usage: m4ri_bench [--seed N] [--warmup K] [--iters K]\n");
            return 2;
        }
    }

    fprintf(stderr,
            "[m4ri_bench] master_seed=0x%llx warmup=%d iters=%d\n",
            (unsigned long long)master_seed, warmup, iters);

    /* Sizes 64..1024 for the dense sweep + 4096 for matmul only.
     * Echelonize at 4096 is deferred to T2 to keep the wall-clock budget
     * of T1 reasonable. */
    static const rci_t MATMUL_SIZES[]   = {64, 256, 1024, 4096};
    static const rci_t ECHELON_SIZES[]  = {64, 256, 1024};

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

    return 0;
}
