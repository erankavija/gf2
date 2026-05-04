/*
 * benchmarks/reference/m4rie_one_cell.c
 *
 * Single-cell driver pinned to **matmul × GF(2^16) × n=1024 × uniform**,
 * the in-scope cell chosen for the M4RIE promotion perf-stat capture
 * (`dev/bench_results/2026-05-04-507b0036-m4rie-perf-stat.txt`,
 * jit:507b0036).
 *
 * The full sweep harness `m4rie_bench.c` runs ~30 seconds end-to-end
 * which is too coarse for a `perf stat -r N` capture; this driver
 * isolates a single cell so the perf-stat numbers reflect the actual
 * hot path of `mzed_mul` rather than the surrounding setup.
 *
 * Build (host or container):
 *   gcc -O3 -march=native -std=c11 m4rie_one_cell.c -lm4rie -lm4ri -lm \
 *       -o m4rie_one_cell
 *
 * Run:
 *   ./m4rie_one_cell [iters]   # default iters=10
 *
 * Inputs are seeded deterministically via SplitMix64. The driver pulls
 * the canonical implementation from `seed_helpers.h` (single source of
 * truth shared with `m4rie_bench.c`, `m4ri_bench.c`, `fflas_bench.cpp`,
 * etc.) — see jit:507b0036 R2 (rework against e3592fe code-review).
 */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <m4ri/m4ri.h>
#include <m4rie/m4rie.h>
#include "seed_helpers.h"

static void fill(mzed_t* M, int m, uint64_t s){
    word mask = ((word)1<<m)-1;
    uint64_t st=s;
    for (rci_t r=0;r<M->nrows;++r) for (rci_t c=0;c<M->ncols;++c) {
        uint64_t v=gf2_bench_splitmix64(&st);
        mzed_write_elem(M,r,c,(word)(v&mask));
    }
}
int main(int argc, char** argv){
    int iters = (argc>1)?atoi(argv[1]):10;
    gf2e* ff = gf2e_init(0x1002du);
    rci_t n = 1024;
    mzed_t* A = mzed_init(ff,n,n);
    mzed_t* B = mzed_init(ff,n,n);
    mzed_t* C = mzed_init(ff,n,n);
    fill(A,16,0xDEADBEEFCAFEBABEULL);
    fill(B,16,0x0123456789ABCDEFULL);
    for (int i=0;i<iters;++i) mzed_mul(C,A,B);
    mzed_free(A); mzed_free(B); mzed_free(C); gf2e_free(ff);
    return 0;
}
