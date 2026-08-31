/* External-baseline harness: M4RI dense GF(2) generator-matrix materialization.
 *
 * Workload W2 of the ae03bcd0 workload-selection contract, by an algorithm
 * independent of the encode-each-basis-vector route:
 *
 *   genmatrix-rref : fill the k x n polynomial-form generator matrix (row i is
 *                    g(x) shifted by i) and reduce it to reduced row echelon
 *                    form with mzd_echelonize_m4ri, yielding the systematic
 *                    [I | P] generator matrix.
 *
 * Two dense GF(2) substrate cells are measured at the same shapes so the
 * survey records what the pinned reference costs for the primitives a
 * generator-matrix encoding family would consume:
 *
 *   echelonize     : mzd_echelonize_m4ri on a random k x n matrix.
 *   matmul-m4rm    : mzd_mul_m4rm of a batch x k by k x n matrix, the dense
 *                    form of "encode a batch by multiplying with G".
 *
 * Generator polynomials are read from the file named by argv[1], produced by
 * `aff3ct_bch_bench gdump`, so both harnesses reduce the same generator.
 *
 * Every printed fact is observed at run time or is a protocol constant
 * declared in this file.
 *
 * Output: CSV on stdout, provenance preamble on stderr.
 */

#define _POSIX_C_SOURCE 200809L

#include <m4ri/m4ri.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* ---------------------------------------------------------------- protocol */
enum
{
    TRIALS = 7,
    WARMUP_REPS = 1,
    MAX_CODES = 16
};
/* A cell stops taking further trials once its accumulated timed work passes
 * this budget, so one oversized shape cannot consume the measurement window.
 * Trials actually completed are reported per row; the survey states the count. */
static const double CELL_BUDGET_S = 45.0;
static const uint64_t SEED = 0xAE03BCD0ull;
/* Batch heights for the matmul-m4rm substrate cell. */
static const int MATMUL_BATCHES[] = { 256, 4096 };
enum
{
    N_MATMUL_BATCHES = 2
};

typedef struct
{
    char name[16];
    int n, k, deg;
    unsigned char g[1 << 12]; /* g[i] = coefficient of x^i, deg <= 192 in the contract */
} Code;

static double
now_s(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + 1e-9 * (double)ts.tv_nsec;
}

/* splitmix64: a self-contained deterministic source so the matrix contents of a
 * run are a function of SEED alone. */
static uint64_t
next_rand(uint64_t* s)
{
    uint64_t z = (*s += 0x9e3779b97f4a7c15ull);
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
    return z ^ (z >> 31);
}

/* Word-wise fill and digest. A per-bit loop over the largest contract shape
 * costs about a billion calls each way, all of it outside the timed region but
 * inside the measurement window. */
static void
fill_random(mzd_t* A, uint64_t* s)
{
    for (rci_t i = 0; i < A->nrows; i++)
    {
        word* row = mzd_row(A, i);
        for (wi_t w = 0; w < A->width; w++)
            row[w] = (word)next_rand(s);
    }
    /* Clear the excess bits past ncols so the matrix stays canonical. */
    for (rci_t i = 0; i < A->nrows; i++)
        mzd_row(A, i)[A->width - 1] &= A->high_bitmask;
}

static uint64_t
digest_matrix(const mzd_t* A)
{
    uint64_t h = 1469598103934665603ull;
    for (rci_t i = 0; i < A->nrows; i++)
    {
        word const* row = mzd_row_const(A, i);
        for (wi_t w = 0; w < A->width; w++)
        {
            h ^= (uint64_t)row[w];
            h *= 1099511628211ull;
        }
    }
    return h;
}

static void
emit(const char* version,
     const char* algorithm,
     const Code* c,
     int batch,
     int trial,
     double ns,
     double bits,
     uint64_t digest)
{
    printf("m4ri,%s,W2,%s,%s,%d,%d,%d,%d,%.3f,%.4f,%016llx\n",
           version,
           algorithm,
           c->name,
           c->n,
           c->k,
           batch,
           trial,
           ns,
           bits / ns * 1e3,
           (unsigned long long)digest);
}

/* Fill row i of G with g(x) shifted by i: the polynomial-form generator
 * matrix of the cyclic code. */
static void
fill_poly_form(mzd_t* G, const Code* c)
{
    mzd_set_ui(G, 0);
    for (rci_t i = 0; i < G->nrows; i++)
        for (int j = 0; j <= c->deg; j++)
            if (c->g[j])
                mzd_write_bit(G, i, (rci_t)(i + j), 1);
}

int
main(int argc, char** argv)
{
    if (argc < 2)
    {
        fprintf(stderr, "usage: %s <generators.txt> [selector]\n", argv[0]);
        return 2;
    }
    const char* selector = (argc > 2) ? argv[2] : "all";
    const int do_gen = (!strcmp(selector, "all") || !strcmp(selector, "genmatrix"));
    const int do_sub = (!strcmp(selector, "all") || !strcmp(selector, "substrate"));

    FILE* f = fopen(argv[1], "r");
    if (!f)
    {
        fprintf(stderr, "cannot open %s\n", argv[1]);
        return 2;
    }

    Code codes[MAX_CODES];
    int n_codes = 0;
    char bits[1 << 12];
    while (n_codes < MAX_CODES
           && fscanf(f, "%15s %d %d %d %4095s", codes[n_codes].name, &codes[n_codes].n, &codes[n_codes].k,
                     &codes[n_codes].deg, bits)
                == 5)
    {
        const size_t len = strlen(bits);
        if ((int)len != codes[n_codes].deg + 1)
        {
            fprintf(stderr, "# SKIP %s: %zu generator coefficients for degree %d\n", codes[n_codes].name, len,
                    codes[n_codes].deg);
            continue;
        }
        for (size_t i = 0; i < len; i++)
            codes[n_codes].g[i] = (unsigned char)(bits[i] == '1');
        n_codes++;
    }
    fclose(f);

    fprintf(stderr, "# harness: m4ri_genmatrix_bench\n");
    fprintf(stderr, "# m4ri_version: %s\n", M4RI_VERSION_STR);
    fprintf(stderr, "# trials_per_cell: %d\n", TRIALS);
    fprintf(stderr, "# cell_budget_s: %.1f\n", CELL_BUDGET_S);
    fprintf(stderr, "# seed: 0x%016llx\n", (unsigned long long)SEED);
    fprintf(stderr, "# codes_loaded: %d\n", n_codes);

    printf("lib,version,workload,algorithm,code,n,k,batch,trial,ns_total,bits_per_s_scaled,digest\n");

    /* Optional comma-separated allowlist of contract rows, so a run can be
     * bounded to the shapes that fit the available measurement window. */
    const char* only = getenv("GF2_SURVEY_CODES");
    if (only && *only)
        fprintf(stderr, "# codes_selected: %s\n", only);

    for (int ci = 0; ci < n_codes; ci++)
    {
        const Code* c = &codes[ci];
        if (only && *only && !strstr(only, c->name))
            continue;

        if (do_gen)
        {
            mzd_t* G = mzd_init(c->k, c->n);
            double spent = 0.0;
            uint64_t digest = 0;

            for (int w = 0; w < WARMUP_REPS; w++)
            {
                fill_poly_form(G, c);
                mzd_echelonize_m4ri(G, 1, 0);
                digest = digest_matrix(G);
            }

            for (int trial = 0; trial < TRIALS && spent < CELL_BUDGET_S; trial++)
            {
                const double t0 = now_s();
                fill_poly_form(G, c);
                mzd_echelonize_m4ri(G, 1, 0);
                const double t1 = now_s();
                spent += t1 - t0;
                emit(M4RI_VERSION_STR, "genmatrix-rref", c, c->k, trial, (t1 - t0) * 1e9,
                     (double)c->k * (double)c->n, digest);
            }
            fprintf(stderr, "#   %s genmatrix-rref: %.3f s of timed work\n", c->name, spent);
            mzd_free(G);
            fflush(stdout);
        }

        if (do_sub)
        {
            uint64_t s = SEED ^ (uint64_t)c->n;

            mzd_t* R = mzd_init(c->k, c->n);
            double spent = 0.0;
            fill_random(R, &s);
            mzd_t* work = mzd_copy(NULL, R);
            mzd_echelonize_m4ri(work, 1, 0);
            const uint64_t ech_digest = digest_matrix(work);
            mzd_free(work);

            for (int trial = 0; trial < TRIALS && spent < CELL_BUDGET_S; trial++)
            {
                mzd_t* A = mzd_copy(NULL, R);
                const double t0 = now_s();
                mzd_echelonize_m4ri(A, 1, 0);
                const double t1 = now_s();
                spent += t1 - t0;
                emit(M4RI_VERSION_STR, "echelonize", c, c->k, trial, (t1 - t0) * 1e9,
                     (double)c->k * (double)c->n, ech_digest);
                mzd_free(A);
            }
            fprintf(stderr, "#   %s echelonize: %.3f s of timed work\n", c->name, spent);
            mzd_free(R);

            for (int bi = 0; bi < N_MATMUL_BATCHES; bi++)
            {
                const int batch = MATMUL_BATCHES[bi];
                mzd_t* M = mzd_init(batch, c->k);
                mzd_t* G = mzd_init(c->k, c->n);
                fill_random(M, &s);
                fill_poly_form(G, c);

                mzd_t* C = mzd_mul_m4rm(NULL, M, G, 0);
                const uint64_t mm_digest = digest_matrix(C);
                mzd_free(C);

                double mspent = 0.0;
                for (int trial = 0; trial < TRIALS && mspent < CELL_BUDGET_S; trial++)
                {
                    const double t0 = now_s();
                    mzd_t* out = mzd_mul_m4rm(NULL, M, G, 0);
                    const double t1 = now_s();
                    mspent += t1 - t0;
                    /* Scaled per information bit of the batch, matching the W1
                     * throughput unit, so the genmatrix-multiply family is
                     * directly comparable with the LFSR families. */
                    emit(M4RI_VERSION_STR, "matmul-m4rm", c, batch, trial, (t1 - t0) * 1e9,
                         (double)batch * (double)c->k, mm_digest);
                    mzd_free(out);
                }
                fprintf(stderr, "#   %s matmul-m4rm batch=%d: %.3f s of timed work\n", c->name, batch, mspent);
                mzd_free(M);
                mzd_free(G);
            }
            fflush(stdout);
        }
    }

    return 0;
}
