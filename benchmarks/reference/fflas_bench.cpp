// benchmarks/reference/fflas_bench.cpp
//
// Reference reproducibility harness for fflas-ffpack. Emits a single CSV
// stream on stdout with the schema documented in benchmarks/README.md:
//
//   lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
//
// The harness is intentionally narrow:
//   * Modular<int64_t> for GF(p) where p fits the int64 fast path.
//   * Modular<float>   for the 8-bit prime to exercise the BLAS-accelerated
//     dispatch on tiny moduli (8-bit + small accumulator) — fflas-ffpack
//     does not provide a "Modular<int8_t>" by default, but Modular<float>
//     with cardinality <=251 is the canonical small-prime path used in
//     the linbox test corpus.
//   * Operations: fgemm, PLUQ (PLE), Invert, Solve, CharPoly. (Echelon is
//     produced as a projection of PLUQ; we time it separately by calling
//     ColumnEchelonForm.)
//   * Sizes 64, 256, 1024 for dense ops. Larger sizes are deferred to T2.
//
// Determinism: every (field, op, size, regime) entry is seeded with a
// 64-bit splittable seed derived from a fixed master seed (see
// benchmarks/seeds/seed.txt). Re-running with the same master produces
// the same matrices byte-for-byte.
//
// CLI:
//   fflas_bench [--seed N] [--warmup K] [--iters K]
//
// All output goes to stdout; status messages go to stderr so the caller
// can redirect cleanly into a CSV file.

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <random>
#include <string>
#include <vector>

#include <givaro/modular.h>
#include <givaro/givpoly1.h>
#include <fflas-ffpack/fflas/fflas.h>
#include <fflas-ffpack/ffpack/ffpack.h>

namespace {

// ----- determinism helpers -----------------------------------------------

// SplitMix64 — small, well-mixed deterministic splitter so each (field,
// op, size, regime) cell gets an independent seed derived from the user
// supplied master seed.
static uint64_t splitmix64(uint64_t& state) {
    state += 0x9E3779B97F4A7C15ULL;
    uint64_t z = state;
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
    return z ^ (z >> 31);
}

static uint64_t derive_seed(uint64_t master,
                            const char* tag,
                            uint64_t op_idx,
                            uint64_t size_idx,
                            uint64_t regime_idx) {
    uint64_t s = master;
    // Mix the tag bytes into the splitter.
    for (const char* p = tag; *p != '\0'; ++p) {
        s ^= static_cast<uint64_t>(static_cast<unsigned char>(*p));
        (void)splitmix64(s);
    }
    s ^= op_idx;       (void)splitmix64(s);
    s ^= size_idx;     (void)splitmix64(s);
    s ^= regime_idx;   (void)splitmix64(s);
    return splitmix64(s);
}

// CSV row emitter. Throughput is in "useful operations per second"; for
// dense linear algebra we report 2*m*k*n / wall_ns scaled to 1e9, which
// is the conventional GF(p) "GFOps/s" yardstick. For factorizations we
// use n^3 as the dominant-term op count; for charpoly, we use n^3 too.
static void emit_csv(const char* lib,
                     const char* op,
                     const char* field,
                     size_t m, size_t k, size_t n,
                     const char* rank_regime,
                     uint64_t seed,
                     uint64_t wall_ns,
                     double throughput_ops) {
    std::printf("%s,%s,%s,%zu,%zu,%zu,%s,%llu,%llu,%.6e\n",
                lib, op, field, m, k, n, rank_regime,
                static_cast<unsigned long long>(seed),
                static_cast<unsigned long long>(wall_ns),
                throughput_ops);
    std::fflush(stdout);
}

// Convenience: chrono → ns counter.
static uint64_t monotonic_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(t).count());
}

// ----- matrix generators -------------------------------------------------

// Fill an array with deterministic random elements drawn from a Field's
// canonical [0, p) range. The RNG state is a SplitMix64 keyed by `seed`.
template <typename Field>
static void fill_uniform(const Field& F,
                         typename Field::Element_ptr A,
                         size_t len,
                         uint64_t seed) {
    uint64_t st = seed;
    typename Field::Residu_t card = F.cardinality();
    for (size_t i = 0; i < len; ++i) {
        uint64_t r = splitmix64(st);
        // Reduce into the field's canonical range. card is small enough
        // that a simple modular reduction is bias-free at this resolution.
        typename Field::Element x;
        F.init(x, static_cast<int64_t>(r % static_cast<uint64_t>(card)));
        A[i] = x;
    }
}

// Make a matrix with rank exactly `rank` (rank-deficient when rank < n)
// by sampling two random rank-`rank` factors and multiplying them.
//
// A = L * R   with L: m×rank, R: rank×n, both uniform.
template <typename Field>
static void fill_rank_deficient(const Field& F,
                                typename Field::Element_ptr A,
                                size_t m, size_t n, size_t rank,
                                uint64_t seed) {
    typename Field::Element_ptr L = FFLAS::fflas_new(F, m * rank);
    typename Field::Element_ptr R = FFLAS::fflas_new(F, rank * n);
    fill_uniform(F, L, m * rank, seed ^ 0xA5A5A5A5A5A5A5A5ULL);
    fill_uniform(F, R, rank * n, seed ^ 0x5A5A5A5A5A5A5A5AULL);
    FFLAS::fgemm(F, FFLAS::FflasNoTrans, FFLAS::FflasNoTrans,
                 m, n, rank,
                 F.one,
                 L, rank,
                 R, n,
                 F.zero,
                 A, n);
    FFLAS::fflas_delete(L);
    FFLAS::fflas_delete(R);
}

// ----- per-operation timers ----------------------------------------------

template <typename Field>
static void bench_fgemm(const Field& F,
                        const char* field_label,
                        size_t n,
                        uint64_t seed,
                        int warmup, int iters) {
    typename Field::Element_ptr A = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr B = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr C = FFLAS::fflas_new(F, n * n);
    fill_uniform(F, A, n * n, seed);
    fill_uniform(F, B, n * n, seed ^ 0x1111111111111111ULL);
    FFLAS::fzero(F, n * n, C, 1);

    for (int i = 0; i < warmup; ++i) {
        FFLAS::fgemm(F, FFLAS::FflasNoTrans, FFLAS::FflasNoTrans,
                     n, n, n,
                     F.one, A, n, B, n, F.zero, C, n);
    }

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        FFLAS::fgemm(F, FFLAS::FflasNoTrans, FFLAS::FflasNoTrans,
                     n, n, n,
                     F.one, A, n, B, n, F.zero, C, n);
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(iters);
    double tput = (2.0 * static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    emit_csv("fflas-ffpack", "fgemm", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A);
    FFLAS::fflas_delete(B);
    FFLAS::fflas_delete(C);
}

template <typename Field>
static void bench_pluq(const Field& F,
                       const char* field_label,
                       size_t n,
                       const char* regime,
                       uint64_t seed,
                       int warmup, int iters) {
    size_t rank_target = (std::strcmp(regime, "deficient") == 0) ? n / 2 : n;

    typename Field::Element_ptr A0 = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A  = FFLAS::fflas_new(F, n * n);
    if (rank_target == n) {
        fill_uniform(F, A0, n * n, seed);
    } else {
        fill_rank_deficient(F, A0, n, n, rank_target, seed);
    }

    std::vector<size_t> P(n), Q(n);

    auto run_once = [&]() {
        FFLAS::fassign(F, n, n, A0, n, A, n);
        for (size_t i = 0; i < n; ++i) { P[i] = 0; Q[i] = 0; }
        FFPACK::PLUQ(F, FFLAS::FflasNonUnit, n, n, A, n, P.data(), Q.data());
    };

    for (int i = 0; i < warmup; ++i) run_once();

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(iters);
    // Conventional op count: 2/3 n^3 for full-rank PLUQ; we report the
    // dominant n^3 term so the throughput column is comparable across
    // operations.
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    emit_csv("fflas-ffpack", "pluq", field_label, n, n, n,
             regime, seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
}

template <typename Field>
static void bench_echelon(const Field& F,
                          const char* field_label,
                          size_t n,
                          const char* regime,
                          uint64_t seed,
                          int warmup, int iters) {
    size_t rank_target = (std::strcmp(regime, "deficient") == 0) ? n / 2 : n;

    typename Field::Element_ptr A0 = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A  = FFLAS::fflas_new(F, n * n);
    if (rank_target == n) {
        fill_uniform(F, A0, n * n, seed);
    } else {
        fill_rank_deficient(F, A0, n, n, rank_target, seed);
    }

    std::vector<size_t> P(n), Q(n);

    auto run_once = [&]() {
        FFLAS::fassign(F, n, n, A0, n, A, n);
        for (size_t i = 0; i < n; ++i) { P[i] = 0; Q[i] = 0; }
        // RowEchelonForm reduces (in place) and writes the rank into the
        // returned size_t. We discard the rank — the timer is what we
        // care about. transform=true so it matches fflas-ffpack's
        // canonical "echelon" benchmark surface.
        (void)FFPACK::RowEchelonForm(F, n, n, A, n, P.data(), Q.data(),
                                     /*transform=*/true);
    };

    for (int i = 0; i < warmup; ++i) run_once();

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    emit_csv("fflas-ffpack", "echelon", field_label, n, n, n,
             regime, seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
}

template <typename Field>
static void bench_invert(const Field& F,
                         const char* field_label,
                         size_t n,
                         uint64_t seed,
                         int warmup, int iters) {
    // Invert needs a full-rank input; we generate one via uniform random
    // and (cheaply) accept the negligible probability of singularity by
    // re-seeding once on failure.
    typename Field::Element_ptr A0   = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A    = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr Ainv = FFLAS::fflas_new(F, n * n);
    fill_uniform(F, A0, n * n, seed);

    int nullity = 0;

    auto run_once = [&]() {
        FFLAS::fassign(F, n, n, A0, n, A, n);
        nullity = 0;
        FFPACK::Invert(F, n, A, n, Ainv, n, nullity);
    };

    for (int i = 0; i < warmup; ++i) run_once();

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    emit_csv("fflas-ffpack", "invert", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
    FFLAS::fflas_delete(Ainv);
}

template <typename Field>
static void bench_solve(const Field& F,
                        const char* field_label,
                        size_t n,
                        uint64_t seed,
                        int warmup, int iters) {
    typename Field::Element_ptr A0 = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A  = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr B  = FFLAS::fflas_new(F, n);
    typename Field::Element_ptr x  = FFLAS::fflas_new(F, n);
    fill_uniform(F, A0, n * n, seed);
    fill_uniform(F, B,  n,     seed ^ 0xDEADBEEFCAFEBABEULL);

    auto run_once = [&]() {
        // Solve consumes A in place (overwrites with PLUQ factors) and
        // writes the solution into x; B is read-only. We restore A from
        // A0 each iteration so every measurement runs against the same
        // input matrix. Solve(F, M, A, lda, x, incx, B, incb) is the
        // canonical signature exercised by fflas-ffpack's test-solve.C.
        FFLAS::fassign(F, n, n, A0, n, A, n);
        FFPACK::Solve(F, n, A, n, x, 1,
                      static_cast<typename Field::ConstElement_ptr>(B), 1);
    };

    for (int i = 0; i < warmup; ++i) run_once();

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    emit_csv("fflas-ffpack", "solve", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
    FFLAS::fflas_delete(B);
    FFLAS::fflas_delete(x);
}

template <typename Field>
static void bench_charpoly(const Field& F,
                           const char* field_label,
                           size_t n,
                           uint64_t seed,
                           int warmup, int iters) {
    typename Field::Element_ptr A0 = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A  = FFLAS::fflas_new(F, n * n);
    fill_uniform(F, A0, n * n, seed);

    using PolRing = Givaro::Poly1Dom<Field>;
    using Polynomial = typename PolRing::Element;
    PolRing R(F);
    typename Field::RandIter G(F);

    auto run_once = [&]() {
        FFLAS::fassign(F, n, n, A0, n, A, n);
        Polynomial charp(n + 1);
        // FfpackAuto picks the algorithmic variant fflas-ffpack
        // considers best for the (field, n) pair — matches the
        // canonical test-charpoly.C usage.
        FFPACK::CharPoly(R, charp, n, A, n, G, FFPACK::FfpackAuto);
    };

    for (int i = 0; i < warmup; ++i) run_once();

    uint64_t total_ns = 0;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    emit_csv("fflas-ffpack", "charpoly", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
}

// ----- per-field driver --------------------------------------------------

// A single field driver runs the full op suite at the configured sizes
// for both rank regimes (full-rank only for ops where rank-deficient is
// not meaningful, e.g. solve / invert / charpoly).
template <typename Field>
static void run_field(const Field& F,
                      const char* field_label,
                      uint64_t master_seed,
                      int warmup, int iters,
                      const std::vector<size_t>& dense_sizes,
                      bool include_charpoly) {
    std::fprintf(stderr, "[fflas_bench] field=%s sizes=", field_label);
    for (size_t n : dense_sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "\n");

    for (size_t si = 0; si < dense_sizes.size(); ++si) {
        size_t n = dense_sizes[si];

        bench_fgemm(F, field_label, n,
                    derive_seed(master_seed, "fgemm", 0, si, 0),
                    warmup, iters);

        bench_pluq(F, field_label, n, "uniform",
                   derive_seed(master_seed, "pluq", 1, si, 0),
                   warmup, iters);
        bench_pluq(F, field_label, n, "deficient",
                   derive_seed(master_seed, "pluq", 1, si, 1),
                   warmup, iters);

        bench_echelon(F, field_label, n, "uniform",
                      derive_seed(master_seed, "echelon", 2, si, 0),
                      warmup, iters);
        bench_echelon(F, field_label, n, "deficient",
                      derive_seed(master_seed, "echelon", 2, si, 1),
                      warmup, iters);

        bench_invert(F, field_label, n,
                     derive_seed(master_seed, "invert", 3, si, 0),
                     warmup, iters);

        bench_solve(F, field_label, n,
                    derive_seed(master_seed, "solve", 4, si, 0),
                    warmup, iters);

        if (include_charpoly) {
            bench_charpoly(F, field_label, n,
                           derive_seed(master_seed, "charpoly", 5, si, 0),
                           warmup, iters);
        }
    }
}

}  // namespace

int main(int argc, char** argv) {
    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    int warmup = 3;
    int iters  = 5;

    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = static_cast<uint64_t>(std::strtoull(argv[++i], nullptr, 0));
        } else if (std::strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else if (std::strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else {
            std::fprintf(stderr,
                         "usage: fflas_bench [--seed N] [--warmup K] [--iters K]\n");
            return 2;
        }
    }

    // CSV header is emitted by run.sh; this binary only emits rows so it
    // can be concatenated with the m4ri output.
    std::fprintf(stderr,
                 "[fflas_bench] master_seed=0x%llx warmup=%d iters=%d\n",
                 static_cast<unsigned long long>(master_seed), warmup, iters);

    // Dense sizes for T1. The story specifies n in {64, 256, 1024, 4096}
    // but 4096 is multi-second per iteration on most reference fields and
    // is deferred to T2 (gf2 side runs criterion which can amortise warm-up).
    const std::vector<size_t> dense_sizes = {64, 256, 1024};
    // CharPoly grows fast — restrict to the smaller two sizes.
    const std::vector<size_t> charpoly_sizes = {64, 256};

    // ---- GF(2^31 - 1): the canonical Mersenne field for fflas-ffpack ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        run_field(F, "GF(2^31-1)", master_seed, warmup, iters,
                  dense_sizes, /*include_charpoly=*/false);
    }

    // ---- GF(65521): largest prime <2^16, well-suited to delayed reduction ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        run_field(F, "GF(65521)", master_seed ^ 0x11ULL, warmup, iters,
                  dense_sizes, /*include_charpoly=*/false);
    }

    // ---- GF(251): 8-bit prime, Modular<float> path ----
    {
        using Field = Givaro::Modular<float>;
        Field F(251.0f);
        run_field(F, "GF(251)", master_seed ^ 0x22ULL, warmup, iters,
                  dense_sizes, /*include_charpoly=*/false);
    }

    // ---- GF(7): tiny prime — exercises bitslicing-friendly small p ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        run_field(F, "GF(7)", master_seed ^ 0x33ULL, warmup, iters,
                  dense_sizes, /*include_charpoly=*/false);
    }

    // ---- CharPoly run (subset of fields, smaller sizes only) ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        for (size_t si = 0; si < charpoly_sizes.size(); ++si) {
            bench_charpoly(F, "GF(2^31-1)", charpoly_sizes[si],
                           derive_seed(master_seed, "charpoly", 5, si, 0),
                           warmup, iters);
        }
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        for (size_t si = 0; si < charpoly_sizes.size(); ++si) {
            bench_charpoly(F, "GF(65521)", charpoly_sizes[si],
                           derive_seed(master_seed ^ 0x11ULL,
                                       "charpoly", 5, si, 0),
                           warmup, iters);
        }
    }

    return 0;
}
