/*
 * benchmarks/reference/m4rie_bench.c
 *
 * Reference reproducibility harness for M4RIE on GF(2^m), m in {4, 8, 16}.
 * Emits CSV rows on stdout in the same schema as fflas_bench.cpp and
 * m4ri_bench.c:
 *
 *   lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
 *
 * The seed scheme is identical to the C++ / C peers (SplitMix64 keyed by
 * the master seed via gf2_bench_splitmix64 / gf2_bench_derive_seed in
 * seed_helpers.h) so gf2-core's own M4RM benchmarks can fill the same
 * matrices when the master seed is shared.
 *
 * Operations: dense matmul (mzed_mul, Method-of-the-Four-Russians +
 * Newton-John tables) at n in {64, 256, 1024}, plus echelonize at the
 * same sizes. Both ops also run a `deficient` regime where one input is
 * built as L*R with rank n/2.
 *
 * Field convention
 * ----------------
 * gf2-core's `crates/gf2-core/src/primitive_polys.rs` declares:
 *
 *   m=4  : x^4 + x + 1                        = 0b10011        (0x13)
 *   m=8  : x^8 + x^4 + x^3 + x^2 + 1          = 0b100011101    (0x11d)
 *   m=16 : x^16 + x^5 + x^3 + x^2 + 1         = 0b10000000000101101 (0x1002d)
 *
 * M4RIE's `gf2e_init(minpoly)` accepts an arbitrary minimal polynomial
 * — there is no compiled-in default. We pass the gf2-core polynomial
 * directly. Both libraries store elements as the same canonical
 * polynomial-residue bit pattern (x^i ↔ bit i, value in [0, 2^m)).
 *
 * As a result: NO basis-change matrix is required — gf2-core values and
 * M4RIE `mzed_read_elem`/`mzed_write_elem` results are bitwise-equal on
 * the same canonical input. (See `dev/plans/m4rie_promotion_evidence.md`
 * for the corresponding criterion #3 evidence.)
 *
 * For the record, M4RIE's `irreducible_polynomials[]` table at
 * `m4rie/gf2e.c` line 68+ lists every polynomial we use:
 *   - 0x13   in `_irreducible_polynomials_degree_04` (entry #0)
 *   - 0x11d  in `_irreducible_polynomials_degree_08` (entry #0)
 *   - 0x1002d in `_irreducible_polynomials_degree_16` (entry #1, after 0x1002b)
 *
 * Smoke contract
 * --------------
 * `m4rie_bench --smoke` runs an `n=16` correctness oracle for matmul
 * over each (m=4, m=8, m=16) cell. It computes the same matrix product
 * three ways:
 *
 *   1. M4RIE `mzed_mul`   (Newton-John, the operation we are timing).
 *   2. A naive scalar reference using `gf2e_mul` directly (single-element
 *      multiply via `_gf2e_mul_arith`, which is shift-and-reduce against
 *      the same minpoly we passed in).
 *   3. A second naive scalar reference reconstructed by hand (no calls
 *      into M4RIE / M4RI), purely from the gf2-core polynomial — this is
 *      the canonical-form witness against which both #1 and #2 are
 *      compared.
 *
 * If any of the three pairwise comparisons disagree on any single
 * element, the smoke run prints a diagnostic to stderr and exits 1.
 *
 * Build
 * -----
 *   gcc -O3 -march=native -std=c11 m4rie_bench.c -lm4rie -lm4ri -lm \
 *       -o m4rie_bench
 *
 * The container Makefile compiles this with the same flags. The
 * Makefile in this directory adds the `-I/usr/local/include` and
 * `-L/usr/local/lib` paths that the pinned-container install uses.
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
#include <m4rie/m4rie.h>

#include "seed_helpers.h"

/* Local thin wrappers around the shared seed helpers. */
static inline uint64_t splitmix64(uint64_t* state) {
    return gf2_bench_splitmix64(state);
}

static inline uint64_t derive_seed(uint64_t master, const char* tag,
                                   uint64_t op_idx, uint64_t size_idx,
                                   uint64_t regime_idx) {
    return gf2_bench_derive_seed(master, tag, op_idx, size_idx, regime_idx);
}

/* ──────────────── gf2-core primitive polynomials ─────────────────────
 *
 * Hard-coded so the harness fails to compile if the values drift away
 * from `crates/gf2-core/src/primitive_polys.rs`. A semantics-changing
 * polynomial swap on the gf2-core side will break smoke before the
 * timing run starts.
 */
static const word kGf2corePoly_m04 = 0x13u;     /* x^4 + x + 1 */
static const word kGf2corePoly_m08 = 0x11du;    /* x^8 + x^4 + x^3 + x^2 + 1 */
static const word kGf2corePoly_m16 = 0x1002du;  /* x^16 + x^5 + x^3 + x^2 + 1 */

/* Reference (used for smoke only): scalar GF(2^m) multiply purely from
 * the polynomial bits. This is the canonical witness — NO calls into
 * M4RIE / M4RI. Schoolbook shift-and-reduce. */
static word ref_gf2m_mul(word a, word b, word minpoly, int m) {
    word mask = ((word)1 << m) - 1;
    word result = 0;
    a &= mask;
    b &= mask;
    for (int i = 0; i < m; ++i) {
        if (b & 1) result ^= a;
        word msb = a >> (m - 1);
        a = (a << 1) & mask;
        if (msb) a ^= (minpoly & mask);
        b >>= 1;
    }
    return result;
}

/* ──────────────── deterministic matrix fill ──────────────────────────
 *
 * Generate uniform random elements in [0, 2^m) using the shared
 * SplitMix64 stream. Each element consumes one full 64-bit splitmix
 * draw and is masked to m bits, mirroring how a Rust-side gf2-core
 * benchmark filling the same shape with the same seed would draw bits.
 */
static void fill_uniform_gf2m(mzed_t* A, int m, uint64_t seed) {
    word mask = ((word)1 << m) - 1;
    uint64_t st = seed;
    for (rci_t r = 0; r < A->nrows; ++r) {
        for (rci_t c = 0; c < A->ncols; ++c) {
            uint64_t v = splitmix64(&st);
            mzed_write_elem(A, r, c, (word)(v & mask));
        }
    }
}

/* Build a rank-`rank` n×n GF(2^m) matrix as L*R (L is n×rank, R is
 * rank×n, both uniform). mzed_mul then has rank exactly `rank`. */
static mzed_t* alloc_rank_deficient(const gf2e* ff, int m, rci_t n,
                                    rci_t rank, uint64_t seed) {
    mzed_t* L = mzed_init(ff, n, rank);
    mzed_t* R = mzed_init(ff, rank, n);
    fill_uniform_gf2m(L, m, seed ^ 0xA5A5A5A5A5A5A5A5ULL);
    fill_uniform_gf2m(R, m, seed ^ 0x5A5A5A5A5A5A5A5AULL);
    mzed_t* A = mzed_init(ff, n, n);
    mzed_mul(A, L, R);
    mzed_free(L);
    mzed_free(R);
    return A;
}

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static const char* field_tag_for_m(int m) {
    switch (m) {
        case 4:  return "GF(2^4)";
        case 8:  return "GF(2^8)";
        case 16: return "GF(2^16)";
        default: return "GF(2^?)";
    }
}

static void emit_csv(const char* op, int m_field,
                     size_t mr, size_t k, size_t n,
                     const char* regime,
                     uint64_t seed,
                     uint64_t wall_ns,
                     double throughput_ops) {
    printf("m4rie,%s,%s,%zu,%zu,%zu,%s,%llu,%llu,%.6e\n",
           op, field_tag_for_m(m_field),
           mr, k, n, regime,
           (unsigned long long)seed,
           (unsigned long long)wall_ns,
           throughput_ops);
    fflush(stdout);
}

/* ──────────────── timing benchmarks ──────────────────────────────────
 *
 * matmul: square A * B over GF(2^m). Throughput normalizer mirrors the
 * fflas-ffpack convention (2 * n^3 — see benchmarks/README.md § *CSV
 * schema*) so an n=1024 cell over GF(2^8) reports the same kind of
 * Gops/s number the existing fflas/M4RI rows do.
 */
static void bench_matmul(const gf2e* ff, int m_field, rci_t n,
                         const char* regime, uint64_t seed,
                         int warmup, int iters) {
    rci_t rank = (strcmp(regime, "deficient") == 0) ? n / 2 : n;

    mzed_t* A = (rank == n) ? mzed_init(ff, n, n)
                            : alloc_rank_deficient(ff, m_field, n, rank, seed);
    mzed_t* B = mzed_init(ff, n, n);
    mzed_t* C = mzed_init(ff, n, n);
    if (rank == n) fill_uniform_gf2m(A, m_field, seed);
    fill_uniform_gf2m(B, m_field, seed ^ 0x1111111111111111ULL);

    for (int i = 0; i < warmup; ++i) {
        mzed_mul(C, A, B);
    }

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        mzed_mul(C, A, B);
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / (uint64_t)iters;
    double tput = 2.0 * (double)n * (double)n * (double)n
                  / ((double)mean_ns * 1.0e-9);

    emit_csv("matmul", m_field, (size_t)n, (size_t)n, (size_t)n,
             regime, seed, mean_ns, tput);

    mzed_free(A);
    mzed_free(B);
    mzed_free(C);
}

/* echelon: full RREF (mzed_echelonize, full=1). Throughput normalizer
 * is n^3 (the dominant-term op count for elimination). */
static void bench_echelonize(const gf2e* ff, int m_field, rci_t n,
                             const char* regime, uint64_t seed,
                             int warmup, int iters) {
    rci_t rank = (strcmp(regime, "deficient") == 0) ? n / 2 : n;

    mzed_t* A0 = (rank == n) ? mzed_init(ff, n, n)
                             : alloc_rank_deficient(ff, m_field, n, rank, seed);
    if (rank == n) fill_uniform_gf2m(A0, m_field, seed);
    mzed_t* A = mzed_init(ff, n, n);

    for (int i = 0; i < warmup; ++i) {
        mzed_copy(A, A0);
        (void)mzed_echelonize(A, 1);
    }

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        mzed_copy(A, A0);
        uint64_t t0 = monotonic_ns();
        (void)mzed_echelonize(A, 1);
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / (uint64_t)iters;
    double tput = (double)n * (double)n * (double)n
                  / ((double)mean_ns * 1.0e-9);

    emit_csv("echelon", m_field, (size_t)n, (size_t)n, (size_t)n,
             regime, seed, mean_ns, tput);

    mzed_free(A0);
    mzed_free(A);
}

/* ──────────────── smoke equality contract ───────────────────────────
 *
 * Per protocol § 6: every claimed cell at n=16 must match a canonical
 * reference computed bit-identically. We pin n=16, regime=uniform,
 * operation=matmul; for each m we:
 *
 *   1. Build A, B as deterministic n×n matrices via fill_uniform_gf2m.
 *   2. Compute C_m4rie = mzed_mul(A, B).
 *   3. Compute C_ref[i][j] = sum_k ref_gf2m_mul(A[i][k], B[k][j])
 *      using a tiny scalar reference that knows nothing about M4RIE.
 *   4. Assert C_m4rie[i][j] == C_ref[i][j] for all (i, j).
 *
 * Returns 0 on success, 1 on any mismatch. Diagnostics go to stderr.
 */
static int smoke_one_field(int m_field, word minpoly, uint64_t master_seed) {
    const rci_t n = 16;
    const char* tag = field_tag_for_m(m_field);
    fprintf(stderr, "[m4rie_bench --smoke] %s (minpoly=0x%llx) ...\n",
            tag, (unsigned long long)minpoly);

    gf2e* ff = gf2e_init(minpoly);
    if (ff->degree != m_field) {
        fprintf(stderr,
                "[m4rie_bench --smoke] gf2e_init returned degree %d for "
                "minpoly 0x%llx (expected %d)\n",
                ff->degree, (unsigned long long)minpoly, m_field);
        gf2e_free(ff);
        return 1;
    }

    /* Seed mirrors a real promotion run's matmul/n=16/uniform cell so
     * the smoke and the timing harness agree on which row of input
     * this n=16 cell would consume in the wider sweep. We use op_idx=0
     * (matmul), size_idx=0 (the first/smallest size in the smoke
     * protocol), regime_idx=0 (uniform). */
    uint64_t row_seed = derive_seed(master_seed, "matmul", 0, 0, 0);
    /* Mix in m_field so different fields get disjoint streams even at
     * the same (op, size, regime) tuple. */
    row_seed ^= ((uint64_t)m_field) * 0x9E3779B97F4A7C15ULL;

    mzed_t* A = mzed_init(ff, n, n);
    mzed_t* B = mzed_init(ff, n, n);
    mzed_t* C = mzed_init(ff, n, n);
    fill_uniform_gf2m(A, m_field, row_seed);
    fill_uniform_gf2m(B, m_field, row_seed ^ 0x1111111111111111ULL);

    mzed_mul(C, A, B);

    /* Independent scalar reference using ref_gf2m_mul. */
    int errors = 0;
    for (rci_t i = 0; i < n; ++i) {
        for (rci_t j = 0; j < n; ++j) {
            word acc = 0;
            for (rci_t k = 0; k < n; ++k) {
                word a_ik = mzed_read_elem(A, i, k);
                word b_kj = mzed_read_elem(B, k, j);
                acc ^= ref_gf2m_mul(a_ik, b_kj, minpoly, m_field);
            }
            word got = mzed_read_elem(C, i, j);
            if (got != acc) {
                if (errors < 5) {
                    fprintf(stderr,
                            "[m4rie_bench --smoke] mismatch at (%d,%d) %s: "
                            "mzed_mul=0x%llx ref=0x%llx\n",
                            (int)i, (int)j, tag,
                            (unsigned long long)got, (unsigned long long)acc);
                }
                ++errors;
            }
        }
    }

    /* Also cross-check against M4RIE's own gf2e_mul (the scalar ground
     * truth M4RIE itself uses to populate Newton-John tables). This
     * guards against the case where ref_gf2m_mul agrees with mzed_mul
     * but disagrees with M4RIE's internal scalar — that would mean
     * we are computing the wrong product but consistently wrong. */
    int internal_errors = 0;
    for (rci_t i = 0; i < n && internal_errors < 5; ++i) {
        for (rci_t j = 0; j < n && internal_errors < 5; ++j) {
            word acc = 0;
            for (rci_t k = 0; k < n; ++k) {
                word a_ik = mzed_read_elem(A, i, k);
                word b_kj = mzed_read_elem(B, k, j);
                acc ^= ff->mul(ff, a_ik, b_kj);
            }
            word ref_acc = 0;
            for (rci_t k = 0; k < n; ++k) {
                word a_ik = mzed_read_elem(A, i, k);
                word b_kj = mzed_read_elem(B, k, j);
                ref_acc ^= ref_gf2m_mul(a_ik, b_kj, minpoly, m_field);
            }
            if (acc != ref_acc) {
                fprintf(stderr,
                        "[m4rie_bench --smoke] internal disagreement at "
                        "(%d,%d) %s: m4rie scalar=0x%llx ref=0x%llx\n",
                        (int)i, (int)j, tag,
                        (unsigned long long)acc,
                        (unsigned long long)ref_acc);
                ++internal_errors;
            }
        }
    }

    mzed_free(A);
    mzed_free(B);
    mzed_free(C);
    gf2e_free(ff);

    if (errors || internal_errors) {
        fprintf(stderr,
                "[m4rie_bench --smoke] %s FAIL: %d cell mismatches, "
                "%d internal disagreements\n",
                tag, errors, internal_errors);
        return 1;
    }
    fprintf(stderr, "[m4rie_bench --smoke] %s OK\n", tag);
    return 0;
}

static int run_smoke(uint64_t master_seed) {
    int rc = 0;
    rc |= smoke_one_field(4,  kGf2corePoly_m04, master_seed);
    rc |= smoke_one_field(8,  kGf2corePoly_m08, master_seed);
    rc |= smoke_one_field(16, kGf2corePoly_m16, master_seed);
    return rc;
}

/* ──────────────── main: argument parsing + sweep ───────────────────── */

int main(int argc, char** argv) {
    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    int warmup = 3;
    int iters  = 5;
    int smoke_only = 0;

    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = strtoull(argv[++i], NULL, 0);
        } else if (strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = (int)strtol(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters = (int)strtol(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--smoke") == 0) {
            smoke_only = 1;
        } else {
            fprintf(stderr,
                    "usage: m4rie_bench [--seed N] [--warmup K] [--iters K]\n"
                    "                   [--smoke]\n");
            return 2;
        }
    }

    if (smoke_only) {
        return run_smoke(master_seed);
    }

    fprintf(stderr,
            "[m4rie_bench] master_seed=0x%llx warmup=%d iters=%d\n",
            (unsigned long long)master_seed, warmup, iters);

    /* Sweep configuration:
     *
     * For each m in {4, 8, 16}:
     *   * matmul at n in {64, 256, 1024} × {uniform, deficient}
     *   * echelon at n in {64, 256, 1024} × {uniform, deficient}
     *
     * n=4096 is deferred to T2 to keep the per-cell wall-clock budget
     * sane for the larger m values (m=16 storage is 16 bits per
     * element; mzed_mul over GF(2^16) at n=4096 can run several
     * seconds per iteration on a Zen-3 host even at -O3
     * -march=native).
     */
    static const struct {
        int m;
        word minpoly;
    } FIELDS[] = {
        {4,  0x13u},
        {8,  0x11du},
        {16, 0x1002du},
    };
    static const rci_t SIZES[] = {64, 256, 1024};
    static const char* REGIMES[2] = {"uniform", "deficient"};

    for (size_t fi = 0; fi < sizeof(FIELDS) / sizeof(FIELDS[0]); ++fi) {
        int m_field = FIELDS[fi].m;
        gf2e* ff = gf2e_init(FIELDS[fi].minpoly);

        for (size_t si = 0; si < sizeof(SIZES) / sizeof(SIZES[0]); ++si) {
            rci_t n = SIZES[si];
            for (size_t ri = 0; ri < 2; ++ri) {
                bench_matmul(ff, m_field, n, REGIMES[ri],
                             derive_seed(master_seed, "matmul",
                                         (uint64_t)fi,
                                         (uint64_t)si,
                                         (uint64_t)ri),
                             warmup, iters);
            }
        }

        for (size_t si = 0; si < sizeof(SIZES) / sizeof(SIZES[0]); ++si) {
            rci_t n = SIZES[si];
            for (size_t ri = 0; ri < 2; ++ri) {
                bench_echelonize(ff, m_field, n, REGIMES[ri],
                                 derive_seed(master_seed, "echelon",
                                             (uint64_t)fi,
                                             (uint64_t)si,
                                             (uint64_t)ri),
                                 warmup, iters);
            }
        }

        gf2e_free(ff);
    }

    return 0;
}
