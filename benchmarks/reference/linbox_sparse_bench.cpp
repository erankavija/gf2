// benchmarks/reference/linbox_sparse_bench.cpp
//
// Sparse reference harness for LinBox 1.7.1, scoped to the cells where
// LinBox is the canonical reference per `dev/plans/sparse_benchmark_corpus.md`
// § 4:
//
//   - `sparse-elim × GF(2)` and `sparse-elim × GF(p)` (canonical LinBox
//     `Method::SparseElimination`, the `linbox/algorithms/gauss-*.h` path).
//   - `spmv × GF(2)` cross-check (canonical = gf2-core self; LinBox is
//     the cross-library oracle at the n=16 smoke layer).
//
// The CSV schema and seed-helper contract match `fflas_sparse_bench.cpp`
// and `fflas_bench.cpp` so the rows merge without `analyze.py` source
// changes.
//
// Implementation note: LinBox's `SparseMatrix<Modular<int>, …>` API is
// non-trivial; we use the `SparseMatrix` template with the
// `Field, Storage = SparseSeq` defaults and call `setEntry` / `apply`
// directly. The Gauss elimination path is via the
// `linbox/algorithms/gauss.h` `GaussDomain<Field>` class, which is the
// underlying engine `Method::SparseElimination` dispatches to. We use
// `GaussDomain` directly for two reasons:
//
//   1. It's the same code path so the timing is representative.
//   2. The high-level `solutions/echelon.h` API in LinBox 1.7.1 has a
//      narrower coverage of sparse storage variants.
//
// CLI: --seed N --warmup K --iters K --smoke

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <vector>

#include <givaro/modular.h>
#include <linbox/ring/modular.h>
#include <linbox/matrix/sparse-matrix.h>
#include <linbox/matrix/dense-matrix.h>
#include <linbox/matrix/sparsematrix/sparse-tpl-matrix.h>
#include <linbox/blackbox/zero-one.h>
#include <linbox/algorithms/gauss.h>
#include <linbox/util/commentator.h>

#include "seed_helpers.h"

namespace {

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

static constexpr uint64_t kCellBudgetNs = 30ULL * 1000ULL * 1000ULL * 1000ULL;

static uint64_t monotonic_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(t).count());
}

static void emit_csv(const char* op,
                     const char* field,
                     size_t m, size_t k, size_t n,
                     const char* rank_regime,
                     uint64_t seed,
                     uint64_t wall_ns,
                     double throughput_ops) {
    std::printf("linbox,%s,%s,%zu,%zu,%zu,%s,%llu,%llu,%.6e\n",
                op, field, m, k, n, rank_regime,
                static_cast<unsigned long long>(seed),
                static_cast<unsigned long long>(wall_ns),
                throughput_ops);
    std::fflush(stdout);
}

// ----- sparse-elim over GF(p) ---------------------------------------------

template <typename Field>
static void bench_sparse_elim(const Field& F,
                              const char* field_label,
                              size_t n,
                              double density,
                              uint64_t seed,
                              int warmup, int iters) {
    using Matrix = LinBox::SparseMatrix<Field>;

    // Build a fresh Bernoulli-supported matrix from `seed`. We re-build
    // for every iteration because GaussDomain mutates in-place; the
    // build cost is included in the first warmup pass and excluded from
    // the timer for the measurement passes.
    auto build = [&](Matrix& A) {
        uint64_t st = seed;
        typename Field::Residu_t card = F.cardinality();
        uint64_t threshold = static_cast<uint64_t>(density * 1.844674407370955e19);
        for (size_t i = 0; i < n; ++i) {
            for (size_t j = 0; j < n; ++j) {
                uint64_t draw = splitmix64(st);
                if (draw < threshold) {
                    uint64_t v_raw = splitmix64(st);
                    uint64_t v = (v_raw % static_cast<uint64_t>(card - 1)) + 1;
                    typename Field::Element x;
                    F.init(x, static_cast<int64_t>(v));
                    A.setEntry(i, j, x);
                }
            }
        }
    };

    auto run_once = [&](uint64_t& wall_ns, unsigned long& reported_rank) {
        Matrix A(F, n, n);
        build(A);
        uint64_t t0 = monotonic_ns();
        LinBox::GaussDomain<Field> G(F);
        size_t rank;
        typename Field::Element det;
        F.init(det, 1);
        // In-place sparse Gauss-Jordan from `linbox/algorithms/gauss.h`.
        // The five-arg overload computes both rank and determinant; the
        // determinant is unused but the harness needs a real value to
        // bind into the call-site.
        G.NoReordering(rank, det, A, A.rowdim(), A.coldim());
        wall_ns = monotonic_ns() - t0;
        reported_rank = static_cast<unsigned long>(rank);
    };

    for (int i = 0; i < warmup; ++i) {
        uint64_t wall_ns;
        unsigned long rank;
        run_once(wall_ns, rank);
    }

    uint64_t total_ns = 0;
    int actual_iters = 0;
    bool early_exit = false;
    for (int i = 0; i < iters; ++i) {
        uint64_t wall_ns;
        unsigned long rank;
        run_once(wall_ns, rank);
        total_ns += wall_ns;
        ++actual_iters;
        if (total_ns >= kCellBudgetNs) {
            early_exit = true;
            break;
        }
    }
    uint64_t mean_ns = total_ns / static_cast<uint64_t>(actual_iters);
    // Throughput: dominant-term n³ for sparse Gauss-Jordan.
    double tput = static_cast<double>(n) * static_cast<double>(n)
                  * static_cast<double>(n)
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    char regime_buf[64];
    std::snprintf(regime_buf, sizeof(regime_buf), "density_%.6e_csr", density);
    if (early_exit) {
        std::fprintf(stderr,
                     "[linbox_sparse_bench] WARN early_exit op=sparse-elim field=%s n=%zu\n",
                     field_label, n);
    }
    emit_csv("sparse-elim", field_label, n, n, n, regime_buf, seed, mean_ns, tput);
}

// ----- spmv over GF(p) (cross-check; canonical is fflas-ffpack) ----------

template <typename Field>
static void bench_spmv(const Field& F,
                       const char* field_label,
                       size_t n,
                       double density,
                       uint64_t seed,
                       int warmup, int iters) {
    using Matrix = LinBox::SparseMatrix<Field>;

    Matrix A(F, n, n);
    uint64_t st = seed;
    typename Field::Residu_t card = F.cardinality();
    uint64_t threshold = static_cast<uint64_t>(density * 1.844674407370955e19);
    uint64_t nnz = 0;
    for (size_t i = 0; i < n; ++i) {
        for (size_t j = 0; j < n; ++j) {
            uint64_t draw = splitmix64(st);
            if (draw < threshold) {
                uint64_t v_raw = splitmix64(st);
                uint64_t v = (v_raw % static_cast<uint64_t>(card - 1)) + 1;
                typename Field::Element x;
                F.init(x, static_cast<int64_t>(v));
                A.setEntry(i, j, x);
                ++nnz;
            }
        }
    }
    if (nnz == 0) {
        return;
    }

    typedef typename LinBox::DenseVector<Field> Vec;
    Vec x(F, n);
    Vec y(F, n);
    {
        uint64_t st2 = seed ^ 0xCAFEBABEULL;
        for (size_t i = 0; i < n; ++i) {
            uint64_t r = splitmix64(st2);
            typename Field::Element xi;
            F.init(xi, static_cast<int64_t>(r % static_cast<uint64_t>(card)));
            x[i] = xi;
        }
    }

    auto run_once = [&]() {
        A.apply(y, x);
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
    double tput = static_cast<double>(nnz) / (static_cast<double>(mean_ns) * 1.0e-9);

    char regime_buf[64];
    std::snprintf(regime_buf, sizeof(regime_buf), "density_%.6e_csr", density);
    if (early_exit) {
        std::fprintf(stderr,
                     "[linbox_sparse_bench] WARN early_exit op=spmv field=%s n=%zu\n",
                     field_label, n);
    }
    emit_csv("spmv", field_label, n, n, 1, regime_buf, seed, mean_ns, tput);
}

// ----- sparse×dense over GF(p) (LinBox cross-check; canonical fflas-ffpack)
//
// Uses the `SparseMatrix<Field, SparseMatrixFormat::TPL>` triples-format
// matrix because `applyLeft(Y, X)` (sparse × dense → dense) is only
// declared on the TPL specialisation in LinBox 1.7.1
// (`linbox/matrix/sparsematrix/sparse-tpl-matrix.h:112`). The CSR
// specialisation has a `SparseMatrixDomain` wrapper that exposes
// `applyLeft(Y, X, alpha)` but its OOO/AVX dispatch path is gated on
// `Givaro::Modular<double>` only — for the int64-modular fields used by
// this harness it falls back to the same underlying saxpy loop, so
// timing parity with the TPL path is acceptable.
//
// The triples are walked once per `applyLeft` and dispatched via
// `MatrixDomain::saxpyin(Y_row, t.elt, X_row)`, matching the row-block
// strategy in `dev/plans/sparse_benchmark_corpus.md:169`.
template <typename Field>
static void bench_sparse_dense(const Field& F,
                               const char* field_label,
                               size_t n,
                               double density,
                               uint64_t seed,
                               int warmup, int iters) {
    using TplMatrix = LinBox::SparseMatrix<Field, LinBox::SparseMatrixFormat::TPL>;
    using DenseMat = LinBox::DenseMatrix<Field>;

    TplMatrix A(F, n, n);
    uint64_t st = seed;
    typename Field::Residu_t card = F.cardinality();
    uint64_t threshold = static_cast<uint64_t>(density * 1.844674407370955e19);
    uint64_t nnz = 0;
    for (size_t i = 0; i < n; ++i) {
        for (size_t j = 0; j < n; ++j) {
            uint64_t draw = splitmix64(st);
            if (draw < threshold) {
                uint64_t v_raw = splitmix64(st);
                uint64_t v = (v_raw % static_cast<uint64_t>(card - 1)) + 1;
                typename Field::Element x;
                F.init(x, static_cast<int64_t>(v));
                A.setEntry(i, j, x);
                ++nnz;
            }
        }
    }
    if (nnz == 0) {
        return;
    }
    A.finalize(TplMatrix::cacheOpt);

    // Block size matches `fflas_sparse_bench`'s sparse_dense column
    // count (n) so the cross-library numbers can be compared at parity.
    size_t blockSize = n;
    DenseMat B(F, n, blockSize);
    DenseMat C(F, n, blockSize);
    {
        uint64_t st2 = seed ^ 0xDEADBEEFULL;
        for (size_t i = 0; i < n; ++i) {
            for (size_t j = 0; j < blockSize; ++j) {
                uint64_t r = splitmix64(st2);
                typename Field::Element xij;
                F.init(xij, static_cast<int64_t>(r % static_cast<uint64_t>(card)));
                B.setEntry(i, j, xij);
            }
        }
    }

    auto run_once = [&]() {
        A.applyLeft(C, B);
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
    // Throughput: each non-zero contributes one saxpy across `blockSize`
    // dense columns, so the dominant op count is nnz * blockSize.
    double tput = static_cast<double>(nnz) * static_cast<double>(blockSize)
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    char regime_buf[64];
    std::snprintf(regime_buf, sizeof(regime_buf), "density_%.6e_csr", density);
    if (early_exit) {
        std::fprintf(stderr,
                     "[linbox_sparse_bench] WARN early_exit op=sparse×dense field=%s n=%zu\n",
                     field_label, n);
    }
    emit_csv("sparse×dense", field_label, n, n, blockSize, regime_buf, seed, mean_ns, tput);
}

template <typename Field>
static void run_field(const Field& F,
                      const char* field_label,
                      uint64_t master_seed,
                      int warmup, int iters,
                      const std::vector<size_t>& sizes,
                      const std::vector<size_t>& elim_sizes,
                      const std::vector<size_t>& sparse_dense_sizes) {
    std::fprintf(stderr, "[linbox_sparse_bench] field=%s\n", field_label);

    for (size_t si = 0; si < sizes.size(); ++si) {
        size_t n = sizes[si];
        double density = 10.0 / static_cast<double>(n);

        bench_spmv(F, field_label, n, density,
                   derive_seed(master_seed, "spmv-er", 0, si, 1),
                   warmup, iters);
    }

    for (size_t si = 0; si < sparse_dense_sizes.size(); ++si) {
        size_t n = sparse_dense_sizes[si];
        double density = 10.0 / static_cast<double>(n);
        bench_sparse_dense(F, field_label, n, density,
                           derive_seed(master_seed, "spdense-er", 1, si, 1),
                           warmup, iters);
    }

    for (size_t si = 0; si < elim_sizes.size(); ++si) {
        size_t n = elim_sizes[si];
        double density = 10.0 / static_cast<double>(n);
        bench_sparse_elim(F, field_label, n, density,
                          derive_seed(master_seed, "spelim-er", 3, si, 1),
                          warmup, iters);
    }
}

}  // namespace

int main(int argc, char** argv) {
    // Silence LinBox commentator chatter (it goes to stderr by default).
    LinBox::commentator().setMaxDetailLevel(-1);
    LinBox::commentator().setMaxDepth(0);
    LinBox::commentator().setReportStream(std::cerr);

    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    int warmup = 1;
    int iters  = 3;
    bool smoke = false;
    bool full = false;

    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = static_cast<uint64_t>(std::strtoull(argv[++i], nullptr, 0));
        } else if (std::strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            warmup = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else if (std::strcmp(argv[i], "--iters") == 0 && i + 1 < argc) {
            iters = static_cast<int>(std::strtol(argv[++i], nullptr, 10));
        } else if (std::strcmp(argv[i], "--smoke") == 0) {
            smoke = true;
        } else if (std::strcmp(argv[i], "--full") == 0) {
            full = true;
        } else if (std::strcmp(argv[i], "--quick") == 0) {
            full = false;
        } else {
            std::fprintf(stderr,
                         "usage: linbox_sparse_bench [--seed N] [--warmup K] [--iters K] "
                         "[--smoke] [--quick|--full]\n");
            return 2;
        }
    }

    std::fprintf(stderr,
                 "[linbox_sparse_bench] master_seed=0x%llx warmup=%d iters=%d smoke=%d full=%d\n",
                 static_cast<unsigned long long>(master_seed), warmup, iters,
                 smoke ? 1 : 0, full ? 1 : 0);

    if (smoke) {
        std::fprintf(stderr, "[linbox_sparse_bench] smoke OK (no-op; see sparse_smoke)\n");
        return 0;
    }

    // sparse-elim wall is dominated by the build (n×n cell scan) at
    // n=4096, so the elim sweep stops at n=1024 in --quick. The
    // sweep at --full extends to n=4096; n=16384 is deferred.
    //
    // sparse_dense uses {1024, 4096} unconditionally to satisfy
    // jit:0f708b36 success criterion 1; 4096 fits within the 30s
    // per-cell budget enforced by `kCellBudgetNs`.
    const std::vector<size_t> quick_sizes = {1024};
    const std::vector<size_t> full_sizes  = {1024, 4096};
    const std::vector<size_t> elim_quick  = {256, 1024};
    const std::vector<size_t> elim_full   = {256, 1024, 4096};
    const std::vector<size_t> sparse_dense_sizes = {1024, 4096};
    const auto& sizes = full ? full_sizes : quick_sizes;
    const auto& elim_sizes = full ? elim_full : elim_quick;

    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        run_field(F, "GF(2^31-1)", master_seed, warmup, iters,
                  sizes, elim_sizes, sparse_dense_sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        run_field(F, "GF(65521)", master_seed ^ 0x11ULL, warmup, iters,
                  sizes, elim_sizes, sparse_dense_sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(251);
        run_field(F, "GF(251)", master_seed ^ 0x22ULL, warmup, iters,
                  sizes, elim_sizes, sparse_dense_sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        run_field(F, "GF(7)", master_seed ^ 0x33ULL, warmup, iters,
                  sizes, elim_sizes, sparse_dense_sizes);
    }

    // GF(2) sparse-elim via Givaro::Modular<int64_t>(2) — uses a one-bit
    // value space which both LinBox and gf2-core handle as a regular
    // GF(p) cell. The dedicated `linbox/algorithms/gauss-gf2.h` would be
    // faster but its public header surface is narrower; the int64-mod-2
    // path runs through the same GaussDomain code generator and produces
    // numbers comparable to the GF(p) cells.
    //
    // GF(2) is intentionally excluded from `sparse_dense_sizes` here:
    // the design doc (`dev/plans/sparse_benchmark_corpus.md`) promotes
    // fflas-ffpack as canonical for `sparse×dense × GF(p)`, and the
    // gf2-core side has no `SpBitMatrix::matmat` entry-point for a
    // GF(2) cross-check (scorecard § 5 #6).
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        const std::vector<size_t> empty_sd;
        run_field(F, "GF(2)", master_seed ^ 0x55ULL, warmup, iters,
                  sizes, elim_sizes, empty_sd);
    }

    return 0;
}
