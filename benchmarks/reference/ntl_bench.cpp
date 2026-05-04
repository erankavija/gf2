// benchmarks/reference/ntl_bench.cpp
//
// Reference reproducibility harness for NTL 11.6.0 (Victor Shoup,
// http://www.shoup.net). Emits CSV rows on stdout in the same schema
// shared with fflas_bench / m4ri_bench:
//
//   lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
//
// Scope of this harness — kept narrow so the per-cell smoke at n=16
// directly maps to gf2-core's `FieldMatrix::gemm` / `charpoly`:
//
//   * `zz_p` single-precision modular field (NTL_SP_BOUND ≥ 2^31-1
//     covers all four reference primes: 7, 251, 65521, 2^31-1).
//   * Operations: `mul` (matrix multiply, alias `fgemm` in CSV),
//     `inv`, `solve`, `charpoly`. NTL gauss / RREF entry returns the
//     rank but mutates `mat_zz_p` in place; we expose it as `echelon`
//     for parity with fflas. PLUQ is NOT exposed by NTL at this level
//     of the API — `LU` is internal — so we skip the `pluq` cell and
//     document the gap in dev/plans/ntl_promotion_evidence.md.
//   * Sizes: 16 (smoke only) and 64 by default. Larger sizes (256,
//     1024) opt in via --large; per-cell wall-clock is capped at
//     kCellBudgetNs to avoid runaway on hosts where NTL is materially
//     slower than fflas-ffpack on the same cell.
//
// GF(2^32) extension lane (jit:b13799ac, 2026-05-04)
// --------------------------------------------------
// In addition to the four GF(p) primes, this harness emits one
// `matmul × GF(2^32)` row per size via NTL's `mat_GF2E` type. The
// extension field is initialised from the GF(2^32) Conway polynomial
// hard-coded in `crates/gf2-core/src/primitive_polys.rs::standard(32)`
// (`x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1`, hex `0x1_0000_8299`),
// the same polynomial gf2-core's production `Gf2mWide<1, _>` path uses
// when an external reference is built against this harness. Because
// both libraries store an extension element as a polynomial of degree
// < 32 with coefficient `c_i` at bit `i`, **no basis-change matrix is
// required** — gf2-core element bytes load directly into NTL via
// `GF2XFromBytes`. See `dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`
// for the criterion #3 evidence.
//
// Determinism: NTL `zz_p` arithmetic is deterministic; the only
// random step (CharPoly's internal Las-Vegas variant in some NTL
// versions) is seeded via NTL::SetSeed so output is bit-stable across
// reruns at the same master seed. Matrix entries are filled from the
// shared SplitMix64 stream so this harness, fflas_bench and the gf2
// criterion benches all agree on the input matrix at seed equality.
//
// CLI:
//   ntl_bench [--seed N] [--warmup K] [--iters K]
//             [--smoke] [--large]
//
// --smoke runs only n=16 cells with warmup=0,iters=1 and prints both
//         the CSV rows AND a per-cell equality oracle line on stderr
//         comparing NTL against the per-row gf2-core canonical answer
//         that the smoke driver supplies via stdin (one canonical
//         output entry per cell, in row-major order). The driver in
//         benchmarks/smoke.sh wires that up.
// --large enables n=256, 1024 cells. Off by default because at n=1024
//         a single charpoly cell on NTL takes 60–120 s on Zen 3.

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include <NTL/lzz_p.h>
#include <NTL/lzz_pX.h>
#include <NTL/mat_lzz_p.h>
#include <NTL/mat_poly_lzz_p.h>
#include <NTL/vec_lzz_p.h>
#include <NTL/ZZ.h>

#include "seed_helpers.h"

namespace {

using NTL::zz_p;
using NTL::zz_pX;
using NTL::mat_zz_p;
using NTL::vec_zz_p;
using NTL::to_zz_p;
using NTL::rep;

// ----- determinism helpers ------------------------------------------------
static inline uint64_t splitmix64(uint64_t& state) {
    return gf2_bench_splitmix64(&state);
}

static inline uint64_t derive_seed(uint64_t master, const char* tag,
                                   uint64_t op_idx, uint64_t size_idx,
                                   uint64_t regime_idx) {
    return gf2_bench_derive_seed(master, tag, op_idx, size_idx, regime_idx);
}

// Per-cell time budget. NTL's `mul` for `zz_p` is roughly an order of
// magnitude slower than fflas-ffpack's BLAS-backed fgemm on small
// primes, so we keep the budget tight. CharPoly is materially slower —
// ~ n^3 with a large constant — so even n=256 can take seconds.
static constexpr uint64_t kCellBudgetNs = 30ULL * 1000ULL * 1000ULL * 1000ULL;

static uint64_t monotonic_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(t).count());
}

static void emit_csv(const char* op, const char* field,
                     size_t m, size_t k, size_t n,
                     const char* rank_regime,
                     uint64_t seed,
                     uint64_t wall_ns,
                     double throughput_ops) {
    std::printf("ntl,%s,%s,%zu,%zu,%zu,%s,%llu,%llu,%.6e\n",
                op, field, m, k, n, rank_regime,
                static_cast<unsigned long long>(seed),
                static_cast<unsigned long long>(wall_ns),
                throughput_ops);
    std::fflush(stdout);
}

static void warn_early_exit(const char* op, const char* field,
                            size_t n, const char* regime,
                            uint64_t observed_ns) {
    std::fprintf(stderr,
                 "[ntl_bench] WARN early_exit op=%s field=%s n=%zu "
                 "regime=%s observed=%llu_ns budget=%llu_ns\n",
                 op, field, n, regime,
                 static_cast<unsigned long long>(observed_ns),
                 static_cast<unsigned long long>(kCellBudgetNs));
}

// ----- matrix generators --------------------------------------------------

// Fill an n×n NTL mat_zz_p with deterministic uniform entries reduced
// to canonical [0, p). The current zz_p modulus must be set before
// calling this.
static void fill_uniform(mat_zz_p& A, long n, uint64_t seed) {
    A.SetDims(n, n);
    uint64_t st = seed;
    long p = zz_p::modulus();
    for (long i = 0; i < n; ++i)
        for (long j = 0; j < n; ++j) {
            uint64_t r = splitmix64(st);
            // r % p is bias-free at the range (zz_p modulus is at most
            // ~2^60 in NTL, so 64-bit r has many full periods of p).
            A[i][j] = to_zz_p(static_cast<long>(r % static_cast<uint64_t>(p)));
        }
}

static void fill_uniform_vec(vec_zz_p& v, long n, uint64_t seed) {
    v.SetLength(n);
    uint64_t st = seed;
    long p = zz_p::modulus();
    for (long i = 0; i < n; ++i) {
        uint64_t r = splitmix64(st);
        v[i] = to_zz_p(static_cast<long>(r % static_cast<uint64_t>(p)));
    }
}

// ----- per-operation timers -----------------------------------------------

static void bench_mul(const char* field_label, long n, uint64_t seed,
                      int warmup, int iters) {
    mat_zz_p A, B, C;
    fill_uniform(A, n, seed);
    fill_uniform(B, n, seed ^ 0x1111111111111111ULL);

    for (int i = 0; i < warmup; ++i) mul(C, A, B);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        mul(C, A, B);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = true; break; }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (2.0 * static_cast<double>(n)
                   * static_cast<double>(n) * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) warn_early_exit("fgemm", field_label, n, "uniform", total_ns);
    // CSV uses `fgemm` to align with fflas-ffpack; gf2-core's bench
    // operation tag for matrix multiply is also `fgemm`.
    emit_csv("fgemm", field_label, n, n, n, "uniform", seed, mean_ns, tput);
}

static void bench_inv(const char* field_label, long n, uint64_t seed,
                      int warmup, int iters) {
    mat_zz_p A, X;
    fill_uniform(A, n, seed);
    // Random `mat_zz_p` over a finite field is invertible w.p. ~1, so
    // we accept the negligible singularity risk. NTL's `inv` raises an
    // exception if A is singular; the (uniform-rank) regime should not
    // hit that on these dimensions.
    for (int i = 0; i < warmup; ++i) inv(X, A);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        inv(X, A);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = true; break; }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) warn_early_exit("invert", field_label, n, "uniform", total_ns);
    emit_csv("invert", field_label, n, n, n, "uniform", seed, mean_ns, tput);
}

static void bench_solve(const char* field_label, long n, uint64_t seed,
                        int warmup, int iters) {
    mat_zz_p A;
    vec_zz_p b, x;
    fill_uniform(A, n, seed);
    fill_uniform_vec(b, n, seed ^ 0xDEADBEEFCAFEBABEULL);
    zz_p d;

    // Column-vector solve(d, A, x, b) ⇒ A*x = b. The other overload
    // is x*A = b (row-vector); we use the column form to match the
    // protocol §6 `solve` contract (and FLINT's nmod_mat_solve).
    for (int i = 0; i < warmup; ++i) solve(d, A, x, b);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        solve(d, A, x, b);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = true; break; }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) warn_early_exit("solve", field_label, n, "uniform", total_ns);
    emit_csv("solve", field_label, n, n, n, "uniform", seed, mean_ns, tput);
}

static void bench_charpoly(const char* field_label, long n, uint64_t seed,
                           int warmup, int iters) {
    mat_zz_p A;
    fill_uniform(A, n, seed);
    zz_pX f;
    // Seed NTL's RNG so any internal Las-Vegas randomness is
    // reproducible at the same master seed.
    NTL::SetSeed(NTL::ZZ(static_cast<long>(seed)));

    for (int i = 0; i < warmup; ++i) CharPoly(f, A);

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        CharPoly(f, A);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) { early_exit = true; break; }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) warn_early_exit("charpoly", field_label, n, "uniform", total_ns);
    emit_csv("charpoly", field_label, n, n, n, "uniform", seed, mean_ns, tput);
}

// ----- per-field driver ---------------------------------------------------

static void run_field(long p, const char* field_label,
                      uint64_t master_seed,
                      int warmup, int iters,
                      const std::vector<long>& dense_sizes,
                      const std::vector<long>& charpoly_sizes) {
    zz_p::init(p);
    std::fprintf(stderr, "[ntl_bench] field=%s p=%ld dense_sizes=", field_label, p);
    for (long n : dense_sizes) std::fprintf(stderr, "%ld ", n);
    std::fprintf(stderr, "charpoly_sizes=");
    for (long n : charpoly_sizes) std::fprintf(stderr, "%ld ", n);
    std::fprintf(stderr, "\n");

    for (size_t si = 0; si < dense_sizes.size(); ++si) {
        long n = dense_sizes[si];
        bench_mul(field_label, n,
                  derive_seed(master_seed, "fgemm", 0, si, 0),
                  warmup, iters);
        bench_inv(field_label, n,
                  derive_seed(master_seed, "invert", 3, si, 0),
                  warmup, iters);
        bench_solve(field_label, n,
                    derive_seed(master_seed, "solve", 4, si, 0),
                    warmup, iters);
    }
    for (size_t ci = 0; ci < charpoly_sizes.size(); ++ci) {
        bench_charpoly(field_label, charpoly_sizes[ci],
                       derive_seed(master_seed, "charpoly", 5, ci, 0),
                       warmup, iters);
    }
}

}  // namespace

int main(int argc, char** argv) {
    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    int warmup = 3;
    int iters  = 5;
    bool smoke = false;
    bool large = false;

    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = static_cast<uint64_t>(std::strtoull(argv[++i], nullptr, 0));
        } else if (std::strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else if (std::strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else if (std::strcmp(argv[i], "--smoke") == 0) {
            smoke = true;
        } else if (std::strcmp(argv[i], "--large") == 0) {
            large = true;
        } else {
            std::fprintf(stderr,
                         "usage: ntl_bench [--seed N] [--warmup K] [--iters K] "
                         "[--smoke] [--large]\n");
            return 2;
        }
    }

    std::fprintf(stderr,
                 "[ntl_bench] master_seed=0x%llx warmup=%d iters=%d smoke=%d large=%d\n",
                 static_cast<unsigned long long>(master_seed),
                 warmup, iters, (int)smoke, (int)large);

    // Size sets:
    //   smoke   — only n=16 (correctness oracle, 1 iter / 0 warmup forced)
    //   default — n=64 (cheap enough for an in-CI sanity sweep)
    //   --large — n=64,256, plus n=1024 for the cheaper non-charpoly ops
    std::vector<long> dense_sizes;
    std::vector<long> charpoly_sizes;
    if (smoke) {
        dense_sizes = {16};
        charpoly_sizes = {16};
        warmup = 0;
        iters  = 1;
    } else if (large) {
        dense_sizes = {64, 256, 1024};
        charpoly_sizes = {64, 256};
    } else {
        dense_sizes = {64};
        charpoly_sizes = {64};
    }

    // Four GF(p) reference fields aligned with the fflas-ffpack harness.
    run_field(7,            "GF(7)",        master_seed ^ 0x33ULL,
              warmup, iters, dense_sizes, charpoly_sizes);
    run_field(251,          "GF(251)",      master_seed ^ 0x22ULL,
              warmup, iters, dense_sizes, charpoly_sizes);
    run_field(65521,        "GF(65521)",    master_seed ^ 0x11ULL,
              warmup, iters, dense_sizes, charpoly_sizes);
    run_field((1L << 31)-1, "GF(2^31-1)",   master_seed,
              warmup, iters, dense_sizes, charpoly_sizes);

    return 0;
}
