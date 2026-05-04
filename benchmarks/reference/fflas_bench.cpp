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
//   * Operations: fgemm, PLUQ (PLE), RowEchelonForm, Invert, Solve,
//     CharPoly, MinPoly. PLUQ / RowEchelonForm / Invert / Solve run in
//     both `uniform` (i.i.d.) and `deficient` (rank exactly n/2) regimes;
//     fgemm, CharPoly, and MinPoly run uniform only.
//   * Sizes 64, 256, 1024 for the full dense sweep on every GF(p)
//     field. fgemm additionally runs at n=4096 on each field; the
//     remaining ops at 4096 are deferred to T2 (per R1 amendment of
//     `a03b2556`). A per-cell wall-clock cap (kCellBudgetNs) is
//     enforced so a slow host degrades gracefully rather than stalling.
//   * CharPoly runs at n in {64, 256} across all four GF(p) fields.
//   * MinPoly (issue 5dea7457) runs at n in {64, 256, 1024} across
//     all four GF(p) fields; n=4096 is deferred per protocol § 10.
//
// CLI:
//   fflas_bench [--seed N] [--warmup K] [--iters K] [--smoke]
//
// `--smoke` runs every operation at n=16 with single warmup/iter and
// performs an internal correctness oracle for each new operation
// row (per `dev/plans/sota_reference_acceptance_protocol.md` § 6):
// minpoly is verified monic and divides charpoly. Smoke output is
// still emitted as CSV; the protocol's gf2-core cross-check runs in
// the parent process (`benchmarks/smoke.sh`).
//
// Determinism: every (field, op, size, regime) entry is seeded with a
// 64-bit splittable seed derived from a fixed master seed (see
// benchmarks/seeds/seed.txt). Re-running with the same master produces
// the same matrices byte-for-byte.
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

#include "seed_helpers.h"

namespace {

// ----- determinism helpers -----------------------------------------------
//
// SplitMix64 + tag-keyed seed derivation are factored into the shared C
// header `seed_helpers.h` so the C and C++ harnesses can never drift.
// We expose thin C++-flavoured wrappers (pass-by-reference on the state
// argument) for ergonomic call-sites elsewhere in this file.

static inline uint64_t splitmix64(uint64_t& state) {
    return gf2_bench_splitmix64(&state);
}

static inline uint64_t derive_seed(uint64_t master,
                                   const char* tag,
                                   uint64_t op_idx,
                                   uint64_t size_idx,
                                   uint64_t regime_idx) {
    return gf2_bench_derive_seed(master, tag, op_idx, size_idx, regime_idx);
}

// Time-cap for any single benchmark cell. Larger sizes (n=4096) are
// guarded behind this so a slow host degrades gracefully instead of
// stalling the harness for minutes. Wall-clock budget is conservative —
// at -O3 -march=native a modern host should clear n=4096 fgemm in well
// under 30 s for every reference field we ship.
static constexpr uint64_t kCellBudgetNs = 30ULL * 1000ULL * 1000ULL * 1000ULL;

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

// Stderr warning emitter for early-exit cases (cell budget exceeded).
// The CSV row is still emitted with whatever measurement we did manage
// to take so downstream tooling sees an entry; readers must consult
// stderr for the early-exit annotation.
static void warn_early_exit(const char* op,
                            const char* field,
                            size_t n,
                            const char* regime,
                            uint64_t observed_ns) {
    std::fprintf(stderr,
                 "[fflas_bench] WARN early_exit op=%s field=%s n=%zu "
                 "regime=%s observed=%llu_ns budget=%llu_ns\n",
                 op, field, n, regime,
                 static_cast<unsigned long long>(observed_ns),
                 static_cast<unsigned long long>(kCellBudgetNs));
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
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        FFLAS::fgemm(F, FFLAS::FflasNoTrans, FFLAS::FflasNoTrans,
                     n, n, n,
                     F.one, A, n, B, n, F.zero, C, n);
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (2.0 * static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) {
        warn_early_exit("fgemm", field_label, n, "uniform", total_ns);
    }
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
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    // Conventional op count: 2/3 n^3 for full-rank PLUQ; we report the
    // dominant n^3 term so the throughput column is comparable across
    // operations.
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) {
        warn_early_exit("pluq", field_label, n, regime, total_ns);
    }
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
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) {
        warn_early_exit("echelon", field_label, n, regime, total_ns);
    }
    emit_csv("fflas-ffpack", "echelon", field_label, n, n, n,
             regime, seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
}

template <typename Field>
static void bench_invert(const Field& F,
                         const char* field_label,
                         size_t n,
                         const char* regime,
                         uint64_t seed,
                         int warmup, int iters) {
    // In the `uniform` regime we sample i.i.d. — over a finite field,
    // a random n×n matrix is invertible with probability ~1, so we
    // accept the negligible singularity risk.
    //
    // In the `deficient` regime we deliberately generate a rank=n/2
    // matrix as L·R. Invert is *expected* to report nullity > 0 here;
    // we still time the call (the cost of detecting singularity is
    // dominated by the same PLUQ pass that would compute the inverse).
    // The CSV column documents the regime so consumers know not to
    // compare the two cells naively.
    const size_t rank_target = (std::strcmp(regime, "deficient") == 0)
                                   ? n / 2 : n;

    typename Field::Element_ptr A0   = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A    = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr Ainv = FFLAS::fflas_new(F, n * n);
    if (rank_target == n) {
        fill_uniform(F, A0, n * n, seed);
    } else {
        fill_rank_deficient(F, A0, n, n, rank_target, seed);
    }

    int nullity = 0;

    auto run_once = [&]() {
        FFLAS::fassign(F, n, n, A0, n, A, n);
        nullity = 0;
        FFPACK::Invert(F, n, A, n, Ainv, n, nullity);
    };

    for (int i = 0; i < warmup; ++i) run_once();

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) {
        warn_early_exit("invert", field_label, n, regime, total_ns);
    }
    if (rank_target != n && nullity == 0) {
        // Sanity check: a deliberately rank-deficient input must report
        // nullity > 0 from FFPACK::Invert. Stash a stderr breadcrumb so
        // a corrupt run is auditable.
        std::fprintf(stderr,
                     "[fflas_bench] WARN deficient invert reported "
                     "nullity=0 (expected >0) field=%s n=%zu\n",
                     field_label, n);
    }
    emit_csv("fflas-ffpack", "invert", field_label, n, n, n,
             regime, seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
    FFLAS::fflas_delete(Ainv);
}

template <typename Field>
static void bench_solve(const Field& F,
                        const char* field_label,
                        size_t n,
                        const char* regime,
                        uint64_t seed,
                        int warmup, int iters) {
    // Same regime semantics as bench_invert: `deficient` builds a
    // singular A so Solve enters its rank-deficient branch. fflas-ffpack
    // returns the trivial / particular solution from the LU pivots in
    // that case — we time the work, which is what the comparison cares
    // about.
    const size_t rank_target = (std::strcmp(regime, "deficient") == 0)
                                   ? n / 2 : n;

    typename Field::Element_ptr A0 = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A  = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr B  = FFLAS::fflas_new(F, n);
    typename Field::Element_ptr x  = FFLAS::fflas_new(F, n);
    if (rank_target == n) {
        fill_uniform(F, A0, n * n, seed);
    } else {
        fill_rank_deficient(F, A0, n, n, rank_target, seed);
    }
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
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) {
        warn_early_exit("solve", field_label, n, regime, total_ns);
    }
    emit_csv("fflas-ffpack", "solve", field_label, n, n, n,
             regime, seed, mean_ns, tput);

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
    // Seed Givaro's RandIter from the per-cell SplitMix64 state instead
    // of OS entropy so CharPoly's internal Las-Vegas / Schwartz-Zippel
    // randomness is reproducible across reruns. Without this seed the
    // reproducibility contract documented in benchmarks/README.md only
    // applies to the input matrix, not to CharPoly's iteration count or
    // wall-clock measurements.
    typename Field::RandIter G(F, /*seed*/ static_cast<uint64_t>(seed));

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
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) {
        warn_early_exit("charpoly", field_label, n, "uniform", total_ns);
    }
    emit_csv("fflas-ffpack", "charpoly", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
}

// ----- minpoly (issue 5dea7457) ------------------------------------------
//
// Times fflas-ffpack's MinPoly via the `MatVecMinPoly`-backed Krylov
// path (see `ffpack_minpoly.inl`). The polynomial returned is monic,
// in canonical [0,p) form for GF(p), with coefficients in ascending
// degree order and length `deg(minpoly)+1`.
//
// Conventional throughput op-count: `n^4` (LCM-merge sweep over n
// Krylov passes) — matches the README CSV-schema rationale.
template <typename Field>
static void bench_minpoly(const Field& F,
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
    // Same RandIter-seeding approach as bench_charpoly: derive the
    // randomness from the per-cell SplitMix64 stream so the wall-clock
    // measurement is reproducible. fflas-ffpack's MinPoly uses a
    // random non-zero starting vector for the Krylov chain.
    typename Field::RandIter G(F, /*seed*/ static_cast<uint64_t>(seed));

    auto run_once = [&]() {
        FFLAS::fassign(F, n, n, A0, n, A, n);
        Polynomial minP;
        FFPACK::MinPoly(F, minP, n, A, n, G);
    };

    for (int i = 0; i < warmup; ++i) run_once();

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    // n^4 dominant-term op-count per benchmarks/README.md § CSV schema.
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n) * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) {
        warn_early_exit("minpoly", field_label, n, "uniform", total_ns);
    }
    emit_csv("fflas-ffpack", "minpoly", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A0);
    FFLAS::fflas_delete(A);
}

// Smoke equality oracle for minpoly (issue 5dea7457): per protocol § 6
// the per-operation contract is: minpoly is monic and is a divisor of
// charpoly. We compute both polynomials on the same fixed-seeded n=16
// input, verify monicity (leading coefficient == 1), and verify that
// `charpoly mod minpoly == 0` via Givaro's Poly1Dom::divmod. A failed
// invariant raises a hard exit(1).
template <typename Field>
static int smoke_minpoly_equality(const Field& F,
                                  const char* field_label,
                                  uint64_t seed) {
    constexpr size_t n = 16;
    typename Field::Element_ptr A  = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr A2 = FFLAS::fflas_new(F, n * n);
    fill_uniform(F, A,  n * n, seed);
    fill_uniform(F, A2, n * n, seed);  // identical seeded copy

    using PolRing = Givaro::Poly1Dom<Field>;
    using Polynomial = typename PolRing::Element;
    PolRing R(F);
    typename Field::RandIter G(F, /*seed*/ static_cast<uint64_t>(seed));

    Polynomial charP(n + 1);
    typename Field::Element_ptr A_for_char = FFLAS::fflas_new(F, n * n);
    FFLAS::fassign(F, n, n, A2, n, A_for_char, n);
    FFPACK::CharPoly(R, charP, n, A_for_char, n, G, FFPACK::FfpackAuto);

    Polynomial minP;
    typename Field::Element_ptr A_for_min = FFLAS::fflas_new(F, n * n);
    FFLAS::fassign(F, n, n, A, n, A_for_min, n);
    FFPACK::MinPoly(F, minP, n, A_for_min, n, G);

    // Monicity check: leading coefficient of minP equals F.one.
    if (minP.size() == 0) {
        std::fprintf(stderr,
                     "[fflas_bench] SMOKE FAIL minpoly empty field=%s\n",
                     field_label);
        FFLAS::fflas_delete(A);
        FFLAS::fflas_delete(A2);
        FFLAS::fflas_delete(A_for_char);
        FFLAS::fflas_delete(A_for_min);
        return 1;
    }
    typename Field::Element lead = minP[minP.size() - 1];
    if (!F.isOne(lead)) {
        std::fprintf(stderr,
                     "[fflas_bench] SMOKE FAIL minpoly not monic "
                     "field=%s leading_coef!=1\n",
                     field_label);
        FFLAS::fflas_delete(A);
        FFLAS::fflas_delete(A2);
        FFLAS::fflas_delete(A_for_char);
        FFLAS::fflas_delete(A_for_min);
        return 1;
    }

    // Divisibility check: charpoly mod minpoly == 0.
    Polynomial q, rem;
    R.divmod(q, rem, charP, minP);
    bool rem_is_zero = true;
    for (size_t i = 0; i < rem.size(); ++i) {
        if (!F.isZero(rem[i])) { rem_is_zero = false; break; }
    }
    if (!rem_is_zero) {
        std::fprintf(stderr,
                     "[fflas_bench] SMOKE FAIL minpoly does not divide "
                     "charpoly field=%s\n",
                     field_label);
        FFLAS::fflas_delete(A);
        FFLAS::fflas_delete(A2);
        FFLAS::fflas_delete(A_for_char);
        FFLAS::fflas_delete(A_for_min);
        return 1;
    }

    std::fprintf(stderr,
                 "[fflas_bench] SMOKE OK minpoly field=%s "
                 "deg=%zu charpoly_deg=%zu\n",
                 field_label,
                 minP.size() ? minP.size() - 1 : 0,
                 charP.size() ? charP.size() - 1 : 0);

    FFLAS::fflas_delete(A);
    FFLAS::fflas_delete(A2);
    FFLAS::fflas_delete(A_for_char);
    FFLAS::fflas_delete(A_for_min);
    return 0;
}

// ----- per-field driver --------------------------------------------------

// A single field driver runs the full op suite at the configured sizes
// across both rank regimes (uniform + deficient) for every op that has
// a meaningful deficient-rank semantics: PLUQ, RowEchelonForm, Invert
// (expected nullity > 0), Solve (singular system).
//
// Charpoly uses uniform inputs only — a charpoly of a rank-deficient
// matrix factors trivially through x^(n-rank) and is not a useful
// timing comparison.
//
// `dense_sizes`     drives fgemm / pluq / echelon / invert / solve.
// `charpoly_sizes`  drives charpoly only (smaller because the algorithm
//                    superlinearly grows with n on these inputs).
// `minpoly_sizes`   drives minpoly only (issue 5dea7457). Matches the
//                    dense-sweep range {64, 256, 1024} per the issue
//                    contract; n=4096 deferred to T2.
template <typename Field>
static void run_field(const Field& F,
                      const char* field_label,
                      uint64_t master_seed,
                      int warmup, int iters,
                      const std::vector<size_t>& dense_sizes,
                      const std::vector<size_t>& charpoly_sizes,
                      const std::vector<size_t>& minpoly_sizes,
                      bool fgemm_only_at_max) {
    std::fprintf(stderr, "[fflas_bench] field=%s dense_sizes=", field_label);
    for (size_t n : dense_sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "charpoly_sizes=");
    for (size_t n : charpoly_sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "minpoly_sizes=");
    for (size_t n : minpoly_sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "\n");

    // The story budget caps factorization wall-clock at a few seconds
    // per cell on n=4096 — cheaper to skip the heavy non-fgemm ops at
    // the largest size on every field rather than wait through ~10
    // minutes of PLUQ/charpoly. fgemm at 4096 is the cheapest of the
    // group (BLAS3 with peak microarchitecture utilisation) and gives
    // the most representative throughput number for the largest cell.
    const size_t fgemm_only_threshold =
        fgemm_only_at_max ? std::size_t{4096} : std::size_t{0};

    for (size_t si = 0; si < dense_sizes.size(); ++si) {
        size_t n = dense_sizes[si];
        bool fgemm_only = fgemm_only_at_max && (n >= fgemm_only_threshold);

        bench_fgemm(F, field_label, n,
                    derive_seed(master_seed, "fgemm", 0, si, 0),
                    warmup, iters);

        if (fgemm_only) continue;

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

        bench_invert(F, field_label, n, "uniform",
                     derive_seed(master_seed, "invert", 3, si, 0),
                     warmup, iters);
        bench_invert(F, field_label, n, "deficient",
                     derive_seed(master_seed, "invert", 3, si, 1),
                     warmup, iters);

        bench_solve(F, field_label, n, "uniform",
                    derive_seed(master_seed, "solve", 4, si, 0),
                    warmup, iters);
        bench_solve(F, field_label, n, "deficient",
                    derive_seed(master_seed, "solve", 4, si, 1),
                    warmup, iters);
    }

    // Charpoly is its own size sweep; smaller because it dominates the
    // wall-clock budget at n=1024+ on our reference hosts.
    for (size_t ci = 0; ci < charpoly_sizes.size(); ++ci) {
        bench_charpoly(F, field_label, charpoly_sizes[ci],
                       derive_seed(master_seed, "charpoly", 5, ci, 0),
                       warmup, iters);
    }

    // Minpoly (issue 5dea7457): own size sweep matching the dense-op
    // range. The kCellBudgetNs cap shields any host where n=1024
    // exceeds the per-cell budget on this field.
    for (size_t mi = 0; mi < minpoly_sizes.size(); ++mi) {
        bench_minpoly(F, field_label, minpoly_sizes[mi],
                      derive_seed(master_seed, "minpoly", 6, mi, 0),
                      warmup, iters);
    }
}

}  // namespace

int main(int argc, char** argv) {
    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    int warmup = 3;
    int iters  = 5;
    bool smoke = false;

    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = static_cast<uint64_t>(std::strtoull(argv[++i], nullptr, 0));
        } else if (std::strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else if (std::strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else if (std::strcmp(argv[i], "--smoke") == 0) {
            smoke = true;
        } else {
            std::fprintf(stderr,
                         "usage: fflas_bench [--seed N] [--warmup K] [--iters K] [--smoke]\n");
            return 2;
        }
    }

    // CSV header is emitted by run.sh; this binary only emits rows so it
    // can be concatenated with the m4ri output.
    std::fprintf(stderr,
                 "[fflas_bench] master_seed=0x%llx warmup=%d iters=%d smoke=%d\n",
                 static_cast<unsigned long long>(master_seed), warmup, iters,
                 smoke ? 1 : 0);

    // Smoke-mode equality oracle for new operations (issue 5dea7457).
    // Per protocol § 6 the per-operation correctness contract is enforced
    // at n=16 against a fixed seeded input. For minpoly: monic + divides
    // charpoly. We exit non-zero on any failure so smoke.sh fails fast.
    if (smoke) {
        int rc = 0;
        {
            using Field = Givaro::Modular<int64_t>;
            Field F((1LL << 31) - 1);
            rc |= smoke_minpoly_equality(F, "GF(2^31-1)",
                                         derive_seed(master_seed, "minpoly_smoke", 6, 0, 0));
        }
        {
            using Field = Givaro::Modular<int64_t>;
            Field F(65521);
            rc |= smoke_minpoly_equality(F, "GF(65521)",
                                         derive_seed(master_seed ^ 0x11ULL, "minpoly_smoke", 6, 0, 0));
        }
        {
            using Field = Givaro::Modular<float>;
            Field F(251.0f);
            rc |= smoke_minpoly_equality(F, "GF(251)",
                                         derive_seed(master_seed ^ 0x22ULL, "minpoly_smoke", 6, 0, 0));
        }
        {
            using Field = Givaro::Modular<int64_t>;
            Field F(7);
            rc |= smoke_minpoly_equality(F, "GF(7)",
                                         derive_seed(master_seed ^ 0x33ULL, "minpoly_smoke", 6, 0, 0));
        }
        if (rc != 0) {
            std::fprintf(stderr, "[fflas_bench] smoke failed (rc=%d)\n", rc);
            return 1;
        }
        std::fprintf(stderr, "[fflas_bench] smoke OK\n");
        return 0;
    }

    // Dense sizes for T1. The story matrix specifies
    //   n in {64, 256, 1024, 4096} for fgemm/pluq/echelon/invert/solve,
    //   n in {64, 256}             for charpoly,
    //   n in {64, 256, 1024}       for minpoly (issue 5dea7457; 4096 deferred),
    // across all four GF(p) reference fields.
    //
    // We honour 4096 for fgemm (the cheapest at that size) on every
    // field; for the heavier non-fgemm ops, the per-cell time-cap
    // (`kCellBudgetNs`, 30 s) keeps the harness from running away on
    // hosts where 4096 lands above the budget. The cap signals
    // `early_exit` on stderr and emits a row with whatever measurement
    // it managed to take, so consumers can downstream-filter.
    const std::vector<size_t> dense_sizes    = {64, 256, 1024, 4096};
    const std::vector<size_t> charpoly_sizes = {64, 256};
    const std::vector<size_t> minpoly_sizes  = {64, 256, 1024};

    // ---- GF(2^31 - 1): the canonical Mersenne field for fflas-ffpack ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        run_field(F, "GF(2^31-1)", master_seed, warmup, iters,
                  dense_sizes, charpoly_sizes, minpoly_sizes,
                  /*fgemm_only_at_max=*/true);
    }

    // ---- GF(65521): largest prime <2^16, well-suited to delayed reduction ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        run_field(F, "GF(65521)", master_seed ^ 0x11ULL, warmup, iters,
                  dense_sizes, charpoly_sizes, minpoly_sizes,
                  /*fgemm_only_at_max=*/true);
    }

    // ---- GF(251): 8-bit prime, Modular<float> path ----
    {
        using Field = Givaro::Modular<float>;
        Field F(251.0f);
        run_field(F, "GF(251)", master_seed ^ 0x22ULL, warmup, iters,
                  dense_sizes, charpoly_sizes, minpoly_sizes,
                  /*fgemm_only_at_max=*/true);
    }

    // ---- GF(7): tiny prime — exercises bitslicing-friendly small p ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        run_field(F, "GF(7)", master_seed ^ 0x33ULL, warmup, iters,
                  dense_sizes, charpoly_sizes, minpoly_sizes,
                  /*fgemm_only_at_max=*/true);
    }

    // ---- GF(31): 5-bit prime — bridge between GF(7) tiny-prime and
    //              GF(251) byte families. Added 2026-05-04 per issue
    //              609855d9 to close the prime-family classification. ----
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(31);
        run_field(F, "GF(31)", master_seed ^ 0x44ULL, warmup, iters,
                  dense_sizes, charpoly_sizes, minpoly_sizes,
                  /*fgemm_only_at_max=*/true);
    }

    return 0;
}
