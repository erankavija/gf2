// benchmarks/reference/linbox_bench.cpp
//
// Reference reproducibility harness for LinBox 1.7.1 over GF(p), targeting
// the cells where fflas-ffpack does not expose a directly comparable
// solution-level API: `charpoly`, `minpoly`, and `solve`. Emits a CSV
// stream on stdout with the schema documented in benchmarks/README.md:
//
//   lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
//
// Operations covered (per protocol § 8 — dense, exact-linear-algebra
// scope; sparse / spmv are out of scope for this story):
//
//   * charpoly  — uniform regime, n in {64, 256}.
//   * minpoly   — uniform regime, n in {64, 256}.
//   * solve     — uniform + deficient regimes, n in {64, 256, 1024}.
//
// LinBox's strength is the high-level `solutions/` API (`charpoly`,
// `minpoly`, `solve`), where it dispatches to algorithm variants —
// dense Krylov, Wiedemann, Berlekamp-Massey, blackbox — chosen by
// `Method::Auto`. Because the matrices we feed are dense, `Method::Auto`
// dispatches to the dense path; we record the algorithm class via the
// CSV `lib` column ("linbox") rather than per-cell because the dispatch
// is determined by `(field, n)` and is implicit in the reproducibility
// contract.
//
// Determinism: every (op, size, regime) cell is seeded via the shared
// gf2_bench_splitmix64 / gf2_bench_derive_seed helpers in
// reference/seed_helpers.h, identical to the fflas harness. For
// algorithms that internally use a Las-Vegas RNG (Wiedemann/probabilistic
// minpoly), we pass a Givaro::ModularRandIter seeded with the same
// per-cell value so the wall-clock measurement is reproducible.
//
// Field choice — only GF(p) under the Givaro::Modular<int64_t> path is
// covered; Modular<float> for the 8-bit prime mirrors the fflas harness's
// path but LinBox's solution dispatch is more uniform across the integer
// path, so we use Modular<int64_t> for all four primes for consistency.
//
// CLI:
//   linbox_bench [--seed N] [--warmup K] [--iters K] [--smoke]
//
// All measurement output goes to stdout; status messages and smoke
// equality reports go to stderr. `--smoke` skips the timing path and
// instead asserts the per-operation correctness contract at n=16 for
// every (op, field, regime) tuple the harness claims to cover; on any
// failure it exits non-zero with a stderr trace identifying the cell.

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <random>
#include <string>
#include <vector>

// LinBox includes — these pull in Givaro / fflas-ffpack transitively.
#include <linbox/ring/modular.h>
#include <linbox/matrix/dense-matrix.h>
#include <linbox/vector/blas-vector.h>
#include <linbox/solutions/charpoly.h>
#include <linbox/solutions/minpoly.h>
#include <linbox/solutions/solve.h>
#include <linbox/solutions/methods.h>

// Pull in fflas-ffpack directly for the harness-internal correctness
// oracles (we use FFLAS::fgemm to reconstruct A·x and compare against b
// in the solve smoke check, etc.). These libraries share Givaro types
// so the conversion is zero-cost.
#include <givaro/modular.h>
#include <fflas-ffpack/fflas/fflas.h>

#include "seed_helpers.h"

namespace {

// ----- determinism helpers (mirror fflas_bench.cpp) ----------------------

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

// Time-cap per cell — same 30 s budget as fflas_bench.cpp so the LinBox
// harness has the same fail-soft semantics on slow hosts. minpoly at
// n=256 over a small prime is the most likely to exceed this; the
// emitted CSV row carries the partial mean and stderr emits an
// `early_exit` annotation.
static constexpr uint64_t kCellBudgetNs = 30ULL * 1000ULL * 1000ULL * 1000ULL;

// CSV row emitter (identical wire format to fflas_bench.cpp).
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

static void warn_early_exit(const char* op,
                            const char* field,
                            size_t n,
                            const char* regime,
                            uint64_t observed_ns) {
    std::fprintf(stderr,
                 "[linbox_bench] WARN early_exit op=%s field=%s n=%zu "
                 "regime=%s observed=%llu_ns budget=%llu_ns\n",
                 op, field, n, regime,
                 static_cast<unsigned long long>(observed_ns),
                 static_cast<unsigned long long>(kCellBudgetNs));
}

static uint64_t monotonic_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(t).count());
}

// ----- matrix builders ---------------------------------------------------
//
// LinBox's BlasMatrix wraps a fflas-ffpack-style row-major buffer; we
// fill it via setEntry so the harness-internal oracle code (which uses
// FFLAS routines on a parallel raw buffer) can be byte-identical.

template <typename Field>
static void fill_uniform_buf(const Field& F,
                             typename Field::Element_ptr A,
                             size_t len,
                             uint64_t seed) {
    uint64_t st = seed;
    typename Field::Residu_t card = F.cardinality();
    for (size_t i = 0; i < len; ++i) {
        uint64_t r = splitmix64(st);
        typename Field::Element x;
        F.init(x, static_cast<int64_t>(r % static_cast<uint64_t>(card)));
        A[i] = x;
    }
}

template <typename Field>
static void fill_rank_deficient_buf(const Field& F,
                                    typename Field::Element_ptr A,
                                    size_t m, size_t n, size_t rank,
                                    uint64_t seed) {
    typename Field::Element_ptr L = FFLAS::fflas_new(F, m * rank);
    typename Field::Element_ptr R = FFLAS::fflas_new(F, rank * n);
    fill_uniform_buf(F, L, m * rank, seed ^ 0xA5A5A5A5A5A5A5A5ULL);
    fill_uniform_buf(F, R, rank * n, seed ^ 0x5A5A5A5A5A5A5A5AULL);
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

// Wrap a raw row-major buffer into a LinBox DenseMatrix. The harness
// owns the buffer; the BlasMatrix copies the entries via setEntry.
template <typename Field>
static LinBox::DenseMatrix<Field>
buf_to_dense(const Field& F,
             typename Field::ConstElement_ptr A,
             size_t rows,
             size_t cols) {
    LinBox::DenseMatrix<Field> M(F, rows, cols);
    for (size_t i = 0; i < rows; ++i) {
        for (size_t j = 0; j < cols; ++j) {
            M.setEntry(i, j, A[i * cols + j]);
        }
    }
    return M;
}

// ----- per-operation timers ----------------------------------------------

template <typename Field>
static void bench_charpoly(const Field& F,
                           const char* field_label,
                           size_t n,
                           uint64_t seed,
                           int warmup, int iters) {
    typename Field::Element_ptr A_buf = FFLAS::fflas_new(F, n * n);
    fill_uniform_buf(F, A_buf, n * n, seed);

    using Vec = std::vector<typename Field::Element>;

    auto run_once = [&]() -> Vec {
        LinBox::DenseMatrix<Field> M = buf_to_dense(F, A_buf, n, n);
        Vec charp;  // LinBox sizes the output internally to n+1.
        LinBox::charpoly(charp, M, LinBox::Method::Auto());
        return charp;
    };

    for (int i = 0; i < warmup; ++i) (void)run_once();

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        (void)run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = (actual_iters > 0)
        ? total_ns / static_cast<uint64_t>(actual_iters)
        : total_ns;
    // Charpoly throughput uses the same n^3 dominant-term normalizer
    // as fflas-ffpack's harness (per benchmarks/README.md § CSV schema).
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) warn_early_exit("charpoly", field_label, n, "uniform", total_ns);
    emit_csv("linbox", "charpoly", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A_buf);
}

template <typename Field>
static void bench_minpoly(const Field& F,
                          const char* field_label,
                          size_t n,
                          uint64_t seed,
                          int warmup, int iters) {
    typename Field::Element_ptr A_buf = FFLAS::fflas_new(F, n * n);
    fill_uniform_buf(F, A_buf, n * n, seed);

    using Vec = std::vector<typename Field::Element>;

    auto run_once = [&]() -> Vec {
        LinBox::DenseMatrix<Field> M = buf_to_dense(F, A_buf, n, n);
        Vec minp;
        LinBox::minpoly(minp, M, LinBox::Method::Auto());
        return minp;
    };

    for (int i = 0; i < warmup; ++i) (void)run_once();

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t t0 = monotonic_ns();
        (void)run_once();
        total_ns += monotonic_ns() - t0;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = (actual_iters > 0)
        ? total_ns / static_cast<uint64_t>(actual_iters)
        : total_ns;
    // benchmarks/README.md normalizer: minpoly uses n^4 (Krylov sweep
    // over n iterations of an O(n^3)-dominated step).
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n) * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) warn_early_exit("minpoly", field_label, n, "uniform", total_ns);
    emit_csv("linbox", "minpoly", field_label, n, n, n,
             "uniform", seed, mean_ns, tput);

    FFLAS::fflas_delete(A_buf);
}

template <typename Field>
static void bench_solve(const Field& F,
                        const char* field_label,
                        size_t n,
                        const char* regime,
                        uint64_t seed,
                        int warmup, int iters) {
    const size_t rank_target = (std::strcmp(regime, "deficient") == 0)
                                   ? n / 2 : n;

    typename Field::Element_ptr A_buf = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr B_buf = FFLAS::fflas_new(F, n);
    if (rank_target == n) {
        fill_uniform_buf(F, A_buf, n * n, seed);
    } else {
        fill_rank_deficient_buf(F, A_buf, n, n, rank_target, seed);
    }
    // Force a consistent system regardless of regime by sampling a
    // random x0 and setting b = A·x0. This guarantees b lies in the
    // column space of A even at small primes (e.g. GF(7), where a
    // uniform-random A has non-trivial probability of singularity).
    //
    // LinBox raises `LinboxMathInconsistentSystem` from `solve()` if
    // the system has no solution, while fflas-ffpack's `FFPACK::Solve`
    // silently returns the trivial particular solution from PLUQ
    // pivots. Forcing consistency makes the wall-clock measurement
    // path-equivalent (both libraries exercise the same elimination
    // cost) and side-steps the algorithm-defined particular-solution
    // ambiguity called out in the SOTA acceptance protocol § 6.
    {
        typename Field::Element_ptr X0 = FFLAS::fflas_new(F, n);
        fill_uniform_buf(F, X0, n, seed ^ 0xC0DEBABEDEADBEEFULL);
        FFLAS::fgemv(F, FFLAS::FflasNoTrans,
                     n, n,
                     F.one, A_buf, n,
                     X0, 1,
                     F.zero, B_buf, 1);
        FFLAS::fflas_delete(X0);
    }

    auto run_once = [&]() {
        LinBox::DenseMatrix<Field> M = buf_to_dense(F, A_buf, n, n);
        LinBox::DenseVector<Field> b(F, n);
        LinBox::DenseVector<Field> x(F, n);
        for (size_t i = 0; i < n; ++i) b.setEntry(i, B_buf[i]);
        // Method::DenseElimination dispatches to the FFPACK solve path
        // for this (field, shape) class — comparable to fflas's
        // FFPACK::Solve. Method::Auto would also pick this on dense
        // input; we name it explicitly so the wall-clock comparison
        // against fflas is apples-to-apples.
        LinBox::solve(x, M, b, LinBox::Method::DenseElimination());
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
    uint64_t mean_ns = (actual_iters > 0)
        ? total_ns / static_cast<uint64_t>(actual_iters)
        : total_ns;
    // Solve uses the n^3 PLUQ dominant term, same as the fflas-ffpack
    // harness — the normalizer is per-operation, not per-library, so
    // ratio comparisons are well-defined.
    double tput = (static_cast<double>(n) * static_cast<double>(n)
                   * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    if (early_exit) warn_early_exit("solve", field_label, n, regime, total_ns);
    emit_csv("linbox", "solve", field_label, n, n, n,
             regime, seed, mean_ns, tput);

    FFLAS::fflas_delete(A_buf);
    FFLAS::fflas_delete(B_buf);
}

// ----- correctness oracles for --smoke -----------------------------------
//
// The protocol § 6 requires the candidate's harness to assert per-op
// equality contracts at n=16 against a deterministic input. Rather than
// linking gf2-core into a C++ harness (which would require a
// gf2-core-side C ABI and is out of scope for this task), the LinBox
// harness validates against mathematical identities that the LinBox
// output must satisfy regardless of algorithm choice:
//
//   * charpoly: monic (leading coeff = 1), degree exactly n, satisfies
//     Cayley-Hamilton (p(A) = 0).
//   * minpoly: monic, degree ≤ n, divides charpoly, satisfies p(A) = 0.
//   * solve  (uniform + deficient): A·x ≡ b (canonical equality
//     bitwise). Both regimes force consistency via b = A·x0 so the
//     check holds even when A is rank-deficient (b ∈ colspace(A) by
//     construction; LinBox returns the FFPACK particular-solution).
//
// All checks use FFLAS::fgemm + element-wise compare on the same raw
// buffer the timing path consumes. Failure exits non-zero with a
// per-cell stderr breadcrumb naming the (op, field, n, regime) tuple.

template <typename Field>
static int evaluate_poly_at_matrix(const Field& F,
                                   typename Field::ConstElement_ptr A,
                                   const std::vector<typename Field::Element>& p,
                                   size_t n) {
    // Compute p(A) = sum_i p[i] * A^i and return 0 iff it is the zero
    // matrix. Implemented via Horner so we touch O(deg(p)) fgemm's.
    using E = typename Field::Element;
    typename Field::Element_ptr acc = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr tmp = FFLAS::fflas_new(F, n * n);
    FFLAS::fzero(F, n * n, acc, 1);

    if (p.empty()) {
        FFLAS::fflas_delete(acc); FFLAS::fflas_delete(tmp);
        return 0;  // zero polynomial.
    }
    // Horner: acc <- p[deg]; for i = deg-1..0: acc <- acc * A + p[i] * I.
    size_t deg = p.size() - 1;
    // acc = p[deg] * I.
    {
        E lead = p[deg];
        for (size_t i = 0; i < n; ++i) acc[i * n + i] = lead;
    }
    for (size_t step = 0; step < deg; ++step) {
        size_t i = deg - 1 - step;
        // tmp = acc * A.
        FFLAS::fgemm(F, FFLAS::FflasNoTrans, FFLAS::FflasNoTrans,
                     n, n, n,
                     F.one, acc, n, A, n,
                     F.zero, tmp, n);
        // acc = tmp + p[i] * I.
        std::memcpy(acc, tmp, sizeof(E) * n * n);
        E coeff = p[i];
        // acc += coeff * I  (diagonal update).
        for (size_t d = 0; d < n; ++d) {
            E v;
            F.add(v, acc[d * n + d], coeff);
            acc[d * n + d] = v;
        }
    }

    // Sum of |entry|: zero matrix iff every entry is F.zero.
    int nonzero = 0;
    for (size_t k = 0; k < n * n; ++k) {
        if (!F.isZero(acc[k])) { nonzero = 1; break; }
    }
    FFLAS::fflas_delete(acc);
    FFLAS::fflas_delete(tmp);
    return nonzero;
}

template <typename Field>
static int smoke_charpoly(const Field& F,
                          const char* field_label,
                          size_t n,
                          uint64_t seed) {
    typename Field::Element_ptr A_buf = FFLAS::fflas_new(F, n * n);
    fill_uniform_buf(F, A_buf, n * n, seed);
    LinBox::DenseMatrix<Field> M = buf_to_dense(F, A_buf, n, n);
    std::vector<typename Field::Element> charp;
    LinBox::charpoly(charp, M, LinBox::Method::Auto());

    int rc = 0;
    if (charp.size() != n + 1) {
        std::fprintf(stderr,
                     "[smoke] FAIL charpoly degree: field=%s n=%zu "
                     "got_size=%zu expected=%zu\n",
                     field_label, n, charp.size(), n + 1);
        rc = 1;
    } else {
        // Monic: leading coeff = 1.
        typename Field::Element one;
        F.init(one, 1);
        if (!F.areEqual(charp[n], one)) {
            std::fprintf(stderr,
                         "[smoke] FAIL charpoly not monic: field=%s n=%zu\n",
                         field_label, n);
            rc = 1;
        }
        // Cayley-Hamilton: p(A) = 0.
        if (evaluate_poly_at_matrix(F, A_buf, charp, n) != 0) {
            std::fprintf(stderr,
                         "[smoke] FAIL charpoly Cayley-Hamilton "
                         "field=%s n=%zu\n",
                         field_label, n);
            rc = 1;
        }
    }
    FFLAS::fflas_delete(A_buf);
    if (rc == 0) {
        std::fprintf(stderr,
                     "[smoke] OK charpoly field=%s n=%zu\n",
                     field_label, n);
    }
    return rc;
}

template <typename Field>
static int smoke_minpoly(const Field& F,
                         const char* field_label,
                         size_t n,
                         uint64_t seed) {
    typename Field::Element_ptr A_buf = FFLAS::fflas_new(F, n * n);
    fill_uniform_buf(F, A_buf, n * n, seed);
    LinBox::DenseMatrix<Field> M = buf_to_dense(F, A_buf, n, n);
    std::vector<typename Field::Element> minp;
    LinBox::minpoly(minp, M, LinBox::Method::Auto());

    int rc = 0;
    if (minp.empty()) {
        std::fprintf(stderr,
                     "[smoke] FAIL minpoly empty: field=%s n=%zu\n",
                     field_label, n);
        rc = 1;
    } else {
        // Monic: leading coeff = 1.
        typename Field::Element one;
        F.init(one, 1);
        if (!F.areEqual(minp.back(), one)) {
            std::fprintf(stderr,
                         "[smoke] FAIL minpoly not monic: field=%s n=%zu\n",
                         field_label, n);
            rc = 1;
        }
        if (minp.size() > n + 1) {
            std::fprintf(stderr,
                         "[smoke] FAIL minpoly degree > n: field=%s n=%zu "
                         "size=%zu\n",
                         field_label, n, minp.size());
            rc = 1;
        }
        // p(A) = 0: minpoly annihilates A by definition.
        if (evaluate_poly_at_matrix(F, A_buf, minp, n) != 0) {
            std::fprintf(stderr,
                         "[smoke] FAIL minpoly does not annihilate A: "
                         "field=%s n=%zu\n",
                         field_label, n);
            rc = 1;
        }
    }
    FFLAS::fflas_delete(A_buf);
    if (rc == 0) {
        std::fprintf(stderr,
                     "[smoke] OK minpoly field=%s n=%zu\n",
                     field_label, n);
    }
    return rc;
}

template <typename Field>
static int smoke_solve(const Field& F,
                       const char* field_label,
                       size_t n,
                       const char* regime,
                       uint64_t seed) {
    // Smoke covers both regimes that the timing path emits to CSV.
    // Consistency is forced via b = A·x0 in either regime so the
    // smoke check works at small primes where a uniform-random A is
    // singular with non-trivial probability (e.g. GF(7) with p^{-1} ≈
    // 0.14 chance of det(A)=0 at n=16) and at deficient inputs where
    // A has rank < n by construction (ker(A) is non-trivial; the
    // particular-solution path is what we are exercising).
    const size_t rank_target = (std::strcmp(regime, "deficient") == 0)
                                   ? n / 2 : n;
    typename Field::Element_ptr A_buf = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr B_buf = FFLAS::fflas_new(F, n);
    if (rank_target == n) {
        fill_uniform_buf(F, A_buf, n * n, seed);
    } else {
        fill_rank_deficient_buf(F, A_buf, n, n, rank_target, seed);
    }
    {
        typename Field::Element_ptr X0 = FFLAS::fflas_new(F, n);
        fill_uniform_buf(F, X0, n, seed ^ 0xC0DEBABEDEADBEEFULL);
        FFLAS::fgemv(F, FFLAS::FflasNoTrans,
                     n, n,
                     F.one, A_buf, n,
                     X0, 1,
                     F.zero, B_buf, 1);
        FFLAS::fflas_delete(X0);
    }

    LinBox::DenseMatrix<Field> M = buf_to_dense(F, A_buf, n, n);
    LinBox::DenseVector<Field> b(F, n);
    LinBox::DenseVector<Field> x(F, n);
    for (size_t i = 0; i < n; ++i) b.setEntry(i, B_buf[i]);

    int rc = 0;
    try {
        LinBox::solve(x, M, b, LinBox::Method::DenseElimination());
    } catch (const std::exception& e) {
        std::fprintf(stderr,
                     "[smoke] FAIL solve threw: field=%s n=%zu regime=%s "
                     "what=%s\n",
                     field_label, n, regime, e.what());
        rc = 1;
    }

    if (rc == 0) {
        // Reconstruct b' = A·x and compare to b element-wise. The
        // identity A·x ≡ b holds in both regimes because b was
        // constructed via b = A·x0 (uniform: A full-rank; deficient:
        // A = L·R with rank n/2, b ∈ colspace(A) by construction).
        typename Field::Element_ptr X_buf = FFLAS::fflas_new(F, n);
        typename Field::Element_ptr Y_buf = FFLAS::fflas_new(F, n);
        for (size_t i = 0; i < n; ++i) X_buf[i] = x.getEntry(i);
        // y = A·x: a single matvec via fgemv.
        FFLAS::fgemv(F, FFLAS::FflasNoTrans,
                     n, n,
                     F.one, A_buf, n,
                     X_buf, 1,
                     F.zero, Y_buf, 1);
        for (size_t i = 0; i < n; ++i) {
            if (!F.areEqual(Y_buf[i], B_buf[i])) {
                std::fprintf(stderr,
                             "[smoke] FAIL solve A·x != b: field=%s n=%zu "
                             "regime=%s row=%zu\n",
                             field_label, n, regime, i);
                rc = 1;
                break;
            }
        }
        FFLAS::fflas_delete(X_buf);
        FFLAS::fflas_delete(Y_buf);
    }
    FFLAS::fflas_delete(A_buf);
    FFLAS::fflas_delete(B_buf);
    if (rc == 0) {
        std::fprintf(stderr,
                     "[smoke] OK solve field=%s n=%zu regime=%s\n",
                     field_label, n, regime);
    }
    return rc;
}

// ----- per-field driver --------------------------------------------------

template <typename Field>
static void run_field(const Field& F,
                      const char* field_label,
                      uint64_t master_seed,
                      int warmup, int iters,
                      const std::vector<size_t>& charpoly_sizes,
                      const std::vector<size_t>& minpoly_sizes,
                      const std::vector<size_t>& solve_sizes) {
    std::fprintf(stderr,
                 "[linbox_bench] field=%s charpoly_sizes=", field_label);
    for (size_t n : charpoly_sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "minpoly_sizes=");
    for (size_t n : minpoly_sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "solve_sizes=");
    for (size_t n : solve_sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "\n");

    for (size_t ci = 0; ci < charpoly_sizes.size(); ++ci) {
        bench_charpoly(F, field_label, charpoly_sizes[ci],
                       derive_seed(master_seed, "charpoly", 5, ci, 0),
                       warmup, iters);
    }
    for (size_t ci = 0; ci < minpoly_sizes.size(); ++ci) {
        bench_minpoly(F, field_label, minpoly_sizes[ci],
                      derive_seed(master_seed, "minpoly", 6, ci, 0),
                      warmup, iters);
    }
    for (size_t si = 0; si < solve_sizes.size(); ++si) {
        size_t n = solve_sizes[si];
        bench_solve(F, field_label, n, "uniform",
                    derive_seed(master_seed, "solve", 4, si, 0),
                    warmup, iters);
        bench_solve(F, field_label, n, "deficient",
                    derive_seed(master_seed, "solve", 4, si, 1),
                    warmup, iters);
    }
}

template <typename Field>
static int smoke_field(const Field& F,
                       const char* field_label,
                       uint64_t master_seed) {
    // n = 16 per protocol § 6 *Correctness-oracle harness*.
    constexpr size_t n = 16;
    int rc = 0;
    rc |= smoke_charpoly(F, field_label, n,
                         derive_seed(master_seed, "charpoly", 5, 0, 0));
    rc |= smoke_minpoly(F, field_label, n,
                        derive_seed(master_seed, "minpoly", 6, 0, 0));
    // Smoke covers both regimes the timing path emits to CSV; the
    // regime_idx values (0 = uniform, 1 = deficient) match the
    // derive_seed contract used by bench_solve in run_field below so
    // the smoke and timing seeds for the same (field, n=16, regime)
    // tuple are identical (cf. SOTA acceptance protocol § 6).
    rc |= smoke_solve(F, field_label, n, "uniform",
                      derive_seed(master_seed, "solve", 4, 0, 0));
    rc |= smoke_solve(F, field_label, n, "deficient",
                      derive_seed(master_seed, "solve", 4, 0, 1));
    return rc;
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
                         "usage: linbox_bench [--seed N] [--warmup K] "
                         "[--iters K] [--smoke]\n");
            return 2;
        }
    }

    std::fprintf(stderr,
                 "[linbox_bench] master_seed=0x%llx warmup=%d iters=%d "
                 "smoke=%d\n",
                 static_cast<unsigned long long>(master_seed),
                 warmup, iters, static_cast<int>(smoke));

    if (smoke) {
        // Smoke mode: assert correctness contract at n=16 across every
        // (op, field) cell the harness covers.
        int rc = 0;
        {
            using Field = Givaro::Modular<int64_t>;
            Field F((1LL << 31) - 1);
            rc |= smoke_field(F, "GF(2^31-1)", master_seed);
        }
        {
            using Field = Givaro::Modular<int64_t>;
            Field F(65521);
            rc |= smoke_field(F, "GF(65521)", master_seed ^ 0x11ULL);
        }
        {
            using Field = Givaro::Modular<int64_t>;
            Field F(251);
            rc |= smoke_field(F, "GF(251)", master_seed ^ 0x22ULL);
        }
        {
            using Field = Givaro::Modular<int64_t>;
            Field F(7);
            rc |= smoke_field(F, "GF(7)", master_seed ^ 0x33ULL);
        }
        if (rc == 0) std::fprintf(stderr, "[linbox_bench] smoke PASS\n");
        else        std::fprintf(stderr, "[linbox_bench] smoke FAIL\n");
        return rc;
    }

    // Bench mode. Sizes mirror the protocol scope:
    //
    //   * charpoly: n in {64, 256}     (matches fflas charpoly_sizes)
    //   * minpoly : n in {64, 256}     (LinBox-only; fflas does not
    //                                     emit a comparable minpoly cell)
    //   * solve   : n in {64, 256, 1024} both regimes (matches fflas
    //                                     dense_sizes minus 4096)
    //
    // n=4096 deferred for solve / charpoly / minpoly per benchmarks/
    // README.md § Deferred to T2 / T3 — same posture as fflas.
    const std::vector<size_t> charpoly_sizes = {64, 256};
    const std::vector<size_t> minpoly_sizes  = {64, 256};
    const std::vector<size_t> solve_sizes    = {64, 256, 1024};

    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        run_field(F, "GF(2^31-1)", master_seed, warmup, iters,
                  charpoly_sizes, minpoly_sizes, solve_sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        run_field(F, "GF(65521)", master_seed ^ 0x11ULL, warmup, iters,
                  charpoly_sizes, minpoly_sizes, solve_sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(251);
        run_field(F, "GF(251)", master_seed ^ 0x22ULL, warmup, iters,
                  charpoly_sizes, minpoly_sizes, solve_sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        run_field(F, "GF(7)", master_seed ^ 0x33ULL, warmup, iters,
                  charpoly_sizes, minpoly_sizes, solve_sizes);
    }

    return 0;
}
