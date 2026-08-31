/* External-baseline harness: bchlib (userspace Linux-kernel lib/bch.c).
 *
 * Workload W1 of the ae03bcd0 workload-selection contract, by the table-driven
 * remainder family: bch_encode consumes 32 message bits per step through four
 * 256-entry remainder tables, over packed bytes rather than one machine word
 * per bit.
 *
 * Only contract rows whose k is a whole number of bytes are measurable here;
 * bch_encode takes its message length in bytes. Rows that fail that test are
 * reported and skipped rather than measured at a different shape.
 *
 * Every printed fact is observed at run time or is a protocol constant
 * declared in this file.
 *
 * Output: CSV on stdout, provenance preamble on stderr.
 */

#define _POSIX_C_SOURCE 200809L

#include "bch.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* ---------------------------------------------------------------- protocol */
enum
{
    TRIALS = 7,
    WARMUP_REPS = 2
};
static const double MIN_TIMED_NS = 5e6;
static const double CELL_BUDGET_S = 90.0;
static const uint64_t SEED = 0xAE03BCD0ull;
static const int BATCHES[] = { 1, 16, 256, 4096 };
enum
{
    N_BATCHES = 4
};

typedef struct
{
    const char* name;
    int m, t, k, n;
    unsigned int prim;
} CodeSpec;

/* Same rows and same primitive polynomials as the sibling harnesses. */
static const CodeSpec CODES[] = {
    { "B1", 4, 3, 5, 15, 0x13u },
    { "B2", 7, 10, 64, 127, 0x83u },
    { "B3", 8, 4, 223, 255, 0x11du },
    { "T2S", 14, 12, 7032, 7200, 0x402bu },
    { "T2N", 16, 12, 32208, 32400, 0x1002du },
};
enum
{
    N_CODES = 5
};

static double
now_s(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + 1e-9 * (double)ts.tv_nsec;
}

static uint64_t
next_rand(uint64_t* s)
{
    uint64_t z = (*s += 0x9e3779b97f4a7c15ull);
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
    return z ^ (z >> 31);
}

int
main(int argc, char** argv)
{
    (void)argc;
    (void)argv;

    const char* only = getenv("GF2_SURVEY_CODES");

    fprintf(stderr, "# harness: bchlib_bch_bench\n");
    fprintf(stderr, "# bchlib_version: %s\n", BCHLIB_VERSION_STR);
    fprintf(stderr, "# trials_per_cell: %d\n", TRIALS);
    fprintf(stderr, "# cell_budget_s: %.1f\n", CELL_BUDGET_S);
    fprintf(stderr, "# seed: 0x%016llx\n", (unsigned long long)SEED);
    if (only && *only)
        fprintf(stderr, "# codes_selected: %s\n", only);

    printf("lib,version,workload,algorithm,code,n,k,t,batch,trial,ns_per_frame,info_mbit_per_s,digest\n");

    for (int ci = 0; ci < N_CODES; ci++)
    {
        const CodeSpec* cs = &CODES[ci];
        if (only && *only && !strstr(only, cs->name))
            continue;

        if (cs->k % 8 != 0)
        {
            fprintf(stderr, "# SKIP %s: k=%d is not a whole number of bytes; bch_encode takes a byte length\n",
                    cs->name, cs->k);
            continue;
        }

        struct bch_control* bch = bch_init(cs->m, cs->t, cs->prim, false);
        if (!bch)
        {
            fprintf(stderr, "# SKIP %s: bch_init(m=%d, t=%d, prim=0x%x) returned NULL\n", cs->name, cs->m, cs->t,
                    cs->prim);
            continue;
        }
        fprintf(stderr, "# code %s: ecc_bits=%u ecc_bytes=%u (contract n-k=%d)\n", cs->name, bch->ecc_bits,
                bch->ecc_bytes, cs->n - cs->k);

        const unsigned int kb = (unsigned int)cs->k / 8;
        const unsigned int eb = bch->ecc_bytes;

        for (int bi = 0; bi < N_BATCHES; bi++)
        {
            const int batch = BATCHES[bi];
            uint64_t s = SEED ^ ((uint64_t)batch << 32) ^ (uint64_t)cs->m;

            uint8_t* data = malloc((size_t)batch * kb);
            uint8_t* ecc = calloc((size_t)batch * eb, 1);
            if (!data || !ecc)
            {
                fprintf(stderr, "# SKIP %s batch=%d: allocation failed\n", cs->name, batch);
                free(data);
                free(ecc);
                continue;
            }
            for (size_t i = 0; i < (size_t)batch * kb; i++)
                data[i] = (uint8_t)(next_rand(&s) & 0xffu);

            /* bch_encode accumulates into ecc, so each frame's parity slot is
             * cleared before its own call. */
#define ENCODE_BATCH()                                                                                                 \
    do                                                                                                                 \
    {                                                                                                                  \
        memset(ecc, 0, (size_t)batch * eb);                                                                            \
        for (int f = 0; f < batch; f++)                                                                                \
            bch_encode(bch, data + (size_t)f * kb, kb, ecc + (size_t)f * eb);                                           \
    } while (0)

            for (int w = 0; w < WARMUP_REPS; w++)
                ENCODE_BATCH();

            uint64_t digest = 1469598103934665603ull;
            for (size_t i = 0; i < (size_t)batch * eb; i++)
            {
                digest ^= ecc[i];
                digest *= 1099511628211ull;
            }

            const double c0 = now_s();
            ENCODE_BATCH();
            const double one_ns = (now_s() - c0) * 1e9;
            long reps = (long)(MIN_TIMED_NS / (one_ns > 0.0 ? one_ns : 1.0)) + 1;
            if (reps < 1)
                reps = 1;

            double spent = 0.0;
            for (int trial = 0; trial < TRIALS && spent < CELL_BUDGET_S; trial++)
            {
                const double t0 = now_s();
                for (long rep = 0; rep < reps; rep++)
                    ENCODE_BATCH();
                const double total_ns = (now_s() - t0) * 1e9;
                spent += total_ns * 1e-9;
                const double ns = total_ns / (double)reps;

                printf("bchlib,%s,W1,table-remainder,%s,%d,%d,%d,%d,%d,%.3f,%.4f,%016llx\n", BCHLIB_VERSION_STR,
                       cs->name, cs->n, cs->k, cs->t, batch, trial, ns / (double)batch,
                       ((double)batch * (double)cs->k) / ns * 1e3, (unsigned long long)digest);
            }
            fprintf(stderr, "#   %s table-remainder batch=%-5d reps=%ld spent=%.2fs\n", cs->name, batch, reps, spent);
#undef ENCODE_BATCH

            free(data);
            free(ecc);
            fflush(stdout);
        }

        bch_free(bch);
    }

    return 0;
}
