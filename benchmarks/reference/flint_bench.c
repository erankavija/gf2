/*
 * benchmarks/reference/flint_bench.c
 *
 * Reference reproducibility harness for FLINT 3.5.0
 * (https://flintlib.org). Emits CSV rows on stdout in the schema
 * shared with fflas_bench / m4ri_bench / ntl_bench:
 *
 *   lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
 *
 * Scope of this harness — keeps `nmod_mat` (single-word modular
 * matrices) ops aligned with gf2-core's in-scope cells:
 *
 *   * Fields: GF(7), GF(251), GF(65521), GF(2^31-1) — same set as
 *     fflas_bench. nmod_mat supports moduli up to 2^63-1 so all four
 *     are well within range.
 *   * Operations:
 *       - nmod_mat_mul         → CSV `fgemm`
 *       - nmod_mat_lu          → CSV `pluq`  (LU with row pivoting)
 *       - nmod_mat_rref        → CSV `echelon`
 *       - nmod_mat_inv         → CSV `invert`
 *       - nmod_mat_solve       → CSV `solve`  (full-rank A only)
 *       - nmod_mat_charpoly    → CSV `charpoly`
 *       - nmod_mat_minpoly     → CSV `minpoly`
 *   * Sizes: n=16 (smoke), n=64 (default sanity), and on --large
 *     n in {64, 256, 1024} (charpoly/minpoly capped at 256).
 *
 * Determinism: nmod_mat ops are deterministic. Matrices are filled
 * from the shared SplitMix64 stream (seed_helpers.h) so this harness
 * agrees byte-for-byte with fflas_bench / ntl_bench / gf2-side
 * criterion benches at the same master seed.
 *
 * Build:
 *   cc -O3 -march=native -std=c11 flint_bench.c -lflint -lgmp -lmpfr -lm
 *
 * The container Makefile compiles this with the same flags.
 *
 * CLI:
 *   flint_bench [--seed N] [--warmup K] [--iters K] [--smoke] [--large]
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>

#include <flint/flint.h>
#include <flint/nmod.h>
#include <flint/nmod_mat.h>
#include <flint/nmod_poly.h>
#include <flint/nmod_vec.h>

#include "seed_helpers.h"

static inline uint64_t splitmix64(uint64_t* state) {
    return gf2_bench_splitmix64(state);
}

static inline uint64_t derive_seed(uint64_t master, const char* tag,
                                   uint64_t op_idx, uint64_t size_idx,
                                   uint64_t regime_idx) {
    return gf2_bench_derive_seed(master, tag, op_idx, size_idx, regime_idx);
}

/* Per-cell wall-clock budget. FLINT's nmod_mat_mul is very fast
 * (BLAS-backed via Strassen on larger n, classical otherwise) so 30s
 * is generous. */
static const uint64_t kCellBudgetNs = (uint64_t)30 * 1000ULL * 1000ULL * 1000ULL;

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static void emit_csv(const char* op, const char* field,
                     size_t m, size_t k, size_t n,
                     const char* rank_regime,
                     uint64_t seed,
                     uint64_t wall_ns,
                     double throughput_ops) {
    printf("flint,%s,%s,%zu,%zu,%zu,%s,%llu,%llu,%.6e\n",
           op, field, m, k, n, rank_regime,
           (unsigned long long)seed,
           (unsigned long long)wall_ns,
           throughput_ops);
    fflush(stdout);
}

static void warn_early_exit(const char* op, const char* field, size_t n,
                            const char* regime, uint64_t observed_ns) {
    fprintf(stderr,
            "[flint_bench] WARN early_exit op=%s field=%s n=%zu "
            "regime=%s observed=%llu_ns budget=%llu_ns\n",
            op, field, n, regime,
            (unsigned long long)observed_ns,
            (unsigned long long)kCellBudgetNs);
}

/* Fill an n×n nmod_mat with deterministic uniform entries reduced
 * to canonical [0, p). The modulus is read from `A->mod.n` (FLINT 3.x
 * exposes `nmod_t mod` directly on the matrix struct). */
static void fill_uniform(nmod_mat_t A, slong n, uint64_t seed) {
    uint64_t st = seed;
    ulong p = A->mod.n;
    for (slong i = 0; i < n; ++i)
        for (slong j = 0; j < n; ++j) {
            uint64_t r = splitmix64(&st);
            nmod_mat_set_entry(A, i, j, (ulong)(r % (uint64_t)p));
        }
}

static void fill_uniform_vec(ulong* v, slong n, ulong p, uint64_t seed) {
    uint64_t st = seed;
    for (slong i = 0; i < n; ++i) {
        uint64_t r = splitmix64(&st);
        v[i] = (ulong)(r % (uint64_t)p);
    }
}

/* ---------- per-operation timers ---------- */

static void bench_mul(const char* field_label, slong n, ulong p,
                      uint64_t seed, int warmup, int iters) {
    nmod_mat_t A, B, C;
    nmod_mat_init(A, n, n, p);
    nmod_mat_init(B, n, n, p);
    nmod_mat_init(C, n, n, p);
    fill_uniform(A, n, seed);
    fill_uniform(B, n, seed ^ 0x1111111111111111ULL);

    for (int i = 0; i < warmup; ++i) nmod_mat_mul(C, A, B);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    int early_exit = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        nmod_mat_mul(C, A, B);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = 1; break; }
    }
    uint64_t mean_ns = total_ns / (uint64_t)actual_iters;
    double tput = (2.0 * (double)n * (double)n * (double)n)
                  / ((double)mean_ns * 1.0e-9);
    if (early_exit) warn_early_exit("fgemm", field_label, n, "uniform", total_ns);
    emit_csv("fgemm", field_label, n, n, n, "uniform", seed, mean_ns, tput);

    nmod_mat_clear(A);
    nmod_mat_clear(B);
    nmod_mat_clear(C);
}

static void bench_lu(const char* field_label, slong n, ulong p,
                     uint64_t seed, int warmup, int iters) {
    /* nmod_mat_lu mutates in place (writes the LU factors back), so
     * we copy from A0 each iteration. */
    nmod_mat_t A0, A;
    nmod_mat_init(A0, n, n, p);
    nmod_mat_init(A, n, n, p);
    fill_uniform(A0, n, seed);

    slong* P = (slong*)flint_malloc(sizeof(slong) * n);

    /* warmup */
    for (int i = 0; i < warmup; ++i) {
        nmod_mat_set(A, A0);
        (void)nmod_mat_lu(P, A, /*rank_check=*/0);
    }

    uint64_t total_ns = 0;
    int actual_iters = 0;
    int early_exit = 0;
    for (int i = 0; i < iters; ++i) {
        nmod_mat_set(A, A0);
        uint64_t t0 = monotonic_ns();
        (void)nmod_mat_lu(P, A, 0);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = 1; break; }
    }
    uint64_t mean_ns = total_ns / (uint64_t)actual_iters;
    double tput = ((double)n * (double)n * (double)n)
                  / ((double)mean_ns * 1.0e-9);
    if (early_exit) warn_early_exit("pluq", field_label, n, "uniform", total_ns);
    emit_csv("pluq", field_label, n, n, n, "uniform", seed, mean_ns, tput);

    flint_free(P);
    nmod_mat_clear(A0);
    nmod_mat_clear(A);
}

static void bench_rref(const char* field_label, slong n, ulong p,
                       uint64_t seed, int warmup, int iters) {
    nmod_mat_t A0, A;
    nmod_mat_init(A0, n, n, p);
    nmod_mat_init(A, n, n, p);
    fill_uniform(A0, n, seed);

    for (int i = 0; i < warmup; ++i) {
        nmod_mat_set(A, A0);
        (void)nmod_mat_rref(A);
    }

    uint64_t total_ns = 0;
    int actual_iters = 0;
    int early_exit = 0;
    for (int i = 0; i < iters; ++i) {
        nmod_mat_set(A, A0);
        uint64_t t0 = monotonic_ns();
        (void)nmod_mat_rref(A);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = 1; break; }
    }
    uint64_t mean_ns = total_ns / (uint64_t)actual_iters;
    double tput = ((double)n * (double)n * (double)n)
                  / ((double)mean_ns * 1.0e-9);
    if (early_exit) warn_early_exit("echelon", field_label, n, "uniform", total_ns);
    emit_csv("echelon", field_label, n, n, n, "uniform", seed, mean_ns, tput);

    nmod_mat_clear(A0);
    nmod_mat_clear(A);
}

static void bench_inv(const char* field_label, slong n, ulong p,
                      uint64_t seed, int warmup, int iters) {
    nmod_mat_t A, X;
    nmod_mat_init(A, n, n, p);
    nmod_mat_init(X, n, n, p);
    fill_uniform(A, n, seed);

    for (int i = 0; i < warmup; ++i) (void)nmod_mat_inv(X, A);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    int early_exit = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        (void)nmod_mat_inv(X, A);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = 1; break; }
    }
    uint64_t mean_ns = total_ns / (uint64_t)actual_iters;
    double tput = ((double)n * (double)n * (double)n)
                  / ((double)mean_ns * 1.0e-9);
    if (early_exit) warn_early_exit("invert", field_label, n, "uniform", total_ns);
    emit_csv("invert", field_label, n, n, n, "uniform", seed, mean_ns, tput);

    nmod_mat_clear(A);
    nmod_mat_clear(X);
}

static void bench_solve(const char* field_label, slong n, ulong p,
                        uint64_t seed, int warmup, int iters) {
    /* nmod_mat_solve takes A x = B as matrices; we run with B as a
     * single column so it matches the fflas-ffpack/NTL `solve(Ax=b)`
     * shape. */
    nmod_mat_t A, B, X;
    nmod_mat_init(A, n, n, p);
    nmod_mat_init(B, n, 1, p);
    nmod_mat_init(X, n, 1, p);
    fill_uniform(A, n, seed);

    /* fill B (n×1) deterministically */
    {
        uint64_t st = seed ^ 0xDEADBEEFCAFEBABEULL;
        for (slong i = 0; i < n; ++i)
            nmod_mat_set_entry(B, i, 0,
                (ulong)(splitmix64(&st) % (uint64_t)p));
    }

    for (int i = 0; i < warmup; ++i) (void)nmod_mat_solve(X, A, B);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    int early_exit = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        (void)nmod_mat_solve(X, A, B);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = 1; break; }
    }
    uint64_t mean_ns = total_ns / (uint64_t)actual_iters;
    double tput = ((double)n * (double)n * (double)n)
                  / ((double)mean_ns * 1.0e-9);
    if (early_exit) warn_early_exit("solve", field_label, n, "uniform", total_ns);
    emit_csv("solve", field_label, n, n, n, "uniform", seed, mean_ns, tput);

    nmod_mat_clear(A);
    nmod_mat_clear(B);
    nmod_mat_clear(X);
}

static void bench_charpoly(const char* field_label, slong n, ulong p,
                           uint64_t seed, int warmup, int iters) {
    nmod_mat_t A;
    nmod_mat_init(A, n, n, p);
    fill_uniform(A, n, seed);
    nmod_poly_t f;
    nmod_poly_init(f, p);

    for (int i = 0; i < warmup; ++i) nmod_mat_charpoly(f, A);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    int early_exit = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        nmod_mat_charpoly(f, A);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = 1; break; }
    }
    uint64_t mean_ns = total_ns / (uint64_t)actual_iters;
    double tput = ((double)n * (double)n * (double)n)
                  / ((double)mean_ns * 1.0e-9);
    if (early_exit) warn_early_exit("charpoly", field_label, n, "uniform", total_ns);
    emit_csv("charpoly", field_label, n, n, n, "uniform", seed, mean_ns, tput);

    nmod_poly_clear(f);
    nmod_mat_clear(A);
}

static void bench_minpoly(const char* field_label, slong n, ulong p,
                          uint64_t seed, int warmup, int iters) {
    nmod_mat_t A;
    nmod_mat_init(A, n, n, p);
    fill_uniform(A, n, seed);
    nmod_poly_t f;
    nmod_poly_init(f, p);

    for (int i = 0; i < warmup; ++i) nmod_mat_minpoly(f, A);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    int early_exit = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        nmod_mat_minpoly(f, A);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = 1; break; }
    }
    uint64_t mean_ns = total_ns / (uint64_t)actual_iters;
    /* minpoly normalizer is n^4 per benchmarks/README.md schema. */
    double tput = ((double)n * (double)n * (double)n * (double)n)
                  / ((double)mean_ns * 1.0e-9);
    if (early_exit) warn_early_exit("minpoly", field_label, n, "uniform", total_ns);
    emit_csv("minpoly", field_label, n, n, n, "uniform", seed, mean_ns, tput);

    nmod_poly_clear(f);
    nmod_mat_clear(A);
}

/* ---------- per-field driver ---------- */

static void run_field(ulong p, const char* field_label,
                      uint64_t master_seed,
                      int warmup, int iters,
                      const slong* dense_sizes, size_t n_dense,
                      const slong* poly_sizes, size_t n_poly) {
    fprintf(stderr, "[flint_bench] field=%s p=%lu dense_sizes=", field_label, p);
    for (size_t i = 0; i < n_dense; ++i) fprintf(stderr, "%ld ", (long)dense_sizes[i]);
    fprintf(stderr, "poly_sizes=");
    for (size_t i = 0; i < n_poly; ++i) fprintf(stderr, "%ld ", (long)poly_sizes[i]);
    fprintf(stderr, "\n");

    for (size_t si = 0; si < n_dense; ++si) {
        slong n = dense_sizes[si];
        bench_mul(field_label, n, p,
                  derive_seed(master_seed, "fgemm", 0, si, 0),
                  warmup, iters);
        bench_lu(field_label, n, p,
                 derive_seed(master_seed, "pluq", 1, si, 0),
                 warmup, iters);
        bench_rref(field_label, n, p,
                   derive_seed(master_seed, "echelon", 2, si, 0),
                   warmup, iters);
        bench_inv(field_label, n, p,
                  derive_seed(master_seed, "invert", 3, si, 0),
                  warmup, iters);
        bench_solve(field_label, n, p,
                    derive_seed(master_seed, "solve", 4, si, 0),
                    warmup, iters);
    }
    for (size_t ci = 0; ci < n_poly; ++ci) {
        bench_charpoly(field_label, poly_sizes[ci], p,
                       derive_seed(master_seed, "charpoly", 5, ci, 0),
                       warmup, iters);
        bench_minpoly(field_label, poly_sizes[ci], p,
                      derive_seed(master_seed, "minpoly", 6, ci, 0),
                      warmup, iters);
    }
}

int main(int argc, char** argv) {
    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    int warmup = 3;
    int iters  = 5;
    int smoke  = 0;
    int large  = 0;

    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = (uint64_t)strtoull(argv[++i], NULL, 0);
        } else if (strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = (int)strtol(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters  = (int)strtol(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--smoke") == 0) {
            smoke = 1;
        } else if (strcmp(argv[i], "--large") == 0) {
            large = 1;
        } else {
            fprintf(stderr,
                    "usage: flint_bench [--seed N] [--warmup K] [--iters K] "
                    "[--smoke] [--large]\n");
            return 2;
        }
    }

    /* Force single-thread to satisfy protocol §5 single-thread requirement. */
    flint_set_num_threads(1);

    fprintf(stderr,
            "[flint_bench] master_seed=0x%llx warmup=%d iters=%d "
            "smoke=%d large=%d threads=%d\n",
            (unsigned long long)master_seed, warmup, iters, smoke, large,
            flint_get_num_threads());

    static const slong dense_smoke[] = { 16 };
    static const slong dense_default[] = { 64 };
    static const slong dense_large[] = { 64, 256, 1024 };
    static const slong poly_smoke[] = { 16 };
    static const slong poly_default[] = { 64 };
    static const slong poly_large[] = { 64, 256 };

    const slong* dense_sizes;
    size_t n_dense;
    const slong* poly_sizes;
    size_t n_poly;

    if (smoke) {
        dense_sizes = dense_smoke; n_dense = 1;
        poly_sizes  = poly_smoke;  n_poly  = 1;
        warmup = 0;
        iters  = 1;
    } else if (large) {
        dense_sizes = dense_large; n_dense = 3;
        poly_sizes  = poly_large;  n_poly  = 2;
    } else {
        dense_sizes = dense_default; n_dense = 1;
        poly_sizes  = poly_default;  n_poly  = 1;
    }

    /* Same four GF(p) reference fields as the fflas/NTL harnesses. */
    run_field(7,            "GF(7)",      master_seed ^ 0x33ULL,
              warmup, iters, dense_sizes, n_dense, poly_sizes, n_poly);
    run_field(251,          "GF(251)",    master_seed ^ 0x22ULL,
              warmup, iters, dense_sizes, n_dense, poly_sizes, n_poly);
    run_field(65521,        "GF(65521)",  master_seed ^ 0x11ULL,
              warmup, iters, dense_sizes, n_dense, poly_sizes, n_poly);
    run_field((1UL << 31) - 1, "GF(2^31-1)", master_seed,
              warmup, iters, dense_sizes, n_dense, poly_sizes, n_poly);

    flint_cleanup();
    return 0;
}
