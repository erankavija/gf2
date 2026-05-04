// benchmarks/reference/fflas_sparse_bench.cpp
//
// Sparse `spmv` and `sparse×dense` reference harness for fflas-ffpack 2.5.0
// over GF(p), companion to the dense `fflas_bench.cpp`. Targets the cells
// promoted in `dev/plans/sparse_benchmark_corpus.md` § 4 — the canonical
// sparse-matrix-vector and sparse-matrix-block-vector products under
// `Givaro::Modular<int64_t>` / `Modular<float>`.
//
// Schema (matches benchmarks/README.md § CSV schema):
//
//   lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
//
// CLI: --seed N --warmup K --iters K --smoke
//
// Determinism: every (op, n, density, field) cell uses the shared
// `gf2_bench_splitmix64` / `gf2_bench_derive_seed` from `seed_helpers.h`.
// The Bernoulli support sample walks SplitMix64 row-major, included if
// `draw < density · 2^64`. This is byte-equivalent with the gf2-core
// emitter at `crates/gf2-coding/examples/bench_sparse_csv_emitter.rs` and
// the `gf2_core::bench_seed::fp_sparse_from_seed` helper.

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <vector>

#include <givaro/modular.h>
#include <fflas-ffpack/fflas/fflas.h>
#include <fflas-ffpack/fflas/fflas_sparse.h>

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
    std::printf("fflas-ffpack,%s,%s,%zu,%zu,%zu,%s,%llu,%llu,%.6e\n",
                op, field, m, k, n, rank_regime,
                static_cast<unsigned long long>(seed),
                static_cast<unsigned long long>(wall_ns),
                throughput_ops);
    std::fflush(stdout);
}

// Build a sparse matrix in CSR (row-pointer + col-index + values) using
// the same Bernoulli-then-non-zero rule as `bench_seed::fp_sparse_from_seed`.
// Walks SplitMix64 row-major across (i, j) cells; for each included cell,
// drives the value SplitMix64 stream forward by one and reduces mod (P-1)+1
// so values are non-zero. This is byte-equivalent with the Rust harness's
// support sample.
template <typename Field>
static void build_csr_uniform(const Field& F,
                              size_t m_rows,
                              size_t n_cols,
                              double density,
                              uint64_t seed,
                              std::vector<uint64_t>& row_ptr,
                              std::vector<uint64_t>& col_idx,
                              std::vector<typename Field::Element>& values) {
    uint64_t st = seed;
    typename Field::Residu_t card = F.cardinality();
    uint64_t threshold = static_cast<uint64_t>(density * 1.844674407370955e19);  // density * 2^64
    row_ptr.assign(m_rows + 1, 0);
    for (size_t i = 0; i < m_rows; ++i) {
        for (size_t j = 0; j < n_cols; ++j) {
            uint64_t draw = splitmix64(st);
            if (draw < threshold) {
                uint64_t v_raw = splitmix64(st);
                // Match the Rust path: values land in [1, P-1].
                uint64_t v = (v_raw % static_cast<uint64_t>(card - 1)) + 1;
                typename Field::Element x;
                F.init(x, static_cast<int64_t>(v));
                col_idx.push_back(j);
                values.push_back(x);
                ++row_ptr[i + 1];
            }
        }
    }
    // Prefix sum for row_ptr.
    for (size_t i = 1; i < row_ptr.size(); ++i) {
        row_ptr[i] += row_ptr[i - 1];
    }
}

// Bench spmv: y = A·x with A sparse CSR, x dense vector.
template <typename Field>
static void bench_spmv(const Field& F,
                       const char* field_label,
                       size_t n,
                       double density,
                       uint64_t seed,
                       int warmup, int iters) {
    using Element = typename Field::Element;

    std::vector<uint64_t> row_ptr_u64;
    std::vector<uint64_t> col_idx_u64;
    std::vector<Element> values;
    build_csr_uniform(F, n, n, density, seed, row_ptr_u64, col_idx_u64, values);
    uint64_t nnz = values.size();
    if (nnz == 0) {
        std::fprintf(stderr,
                     "[fflas_sparse_bench] WARN nnz=0 op=spmv field=%s n=%zu d=%g\n",
                     field_label, n, density);
        return;
    }

    // FFLAS::Sparse<Field, CSR> uses index_t = uint32_t typically; we
    // marshal into the matching width.
    using FFLAS::Sparse;
    using FFLAS::SparseMatrix_t;
    using IndexT = uint64_t;  // must match the index_t of fflas-ffpack's CSR

    Sparse<Field, SparseMatrix_t::CSR> A;

    std::vector<IndexT> row_idx_for_init(nnz);  // sparse_init takes (row, col, dat, m, n, nnz)
    {
        size_t k = 0;
        for (size_t i = 0; i < n; ++i) {
            for (size_t e = row_ptr_u64[i]; e < row_ptr_u64[i + 1]; ++e) {
                row_idx_for_init[k++] = i;
            }
        }
    }
    FFLAS::sparse_init(F, A,
                       row_idx_for_init.data(),
                       reinterpret_cast<const IndexT*>(col_idx_u64.data()),
                       values.data(),
                       static_cast<uint64_t>(n),
                       static_cast<uint64_t>(n),
                       nnz);

    typename Field::Element_ptr x = FFLAS::fflas_new(F, n);
    typename Field::Element_ptr y = FFLAS::fflas_new(F, n);
    {
        uint64_t st = seed ^ 0xCAFEBABEULL;
        typename Field::Residu_t card = F.cardinality();
        for (size_t i = 0; i < n; ++i) {
            uint64_t r = splitmix64(st);
            typename Field::Element xi;
            F.init(xi, static_cast<int64_t>(r % static_cast<uint64_t>(card)));
            x[i] = xi;
        }
    }

    auto run_once = [&]() {
        FFLAS::fzero(F, n, y, 1);
        FFLAS::fspmv(F, A, x, F.zero, y);
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
                     "[fflas_sparse_bench] WARN early_exit op=spmv field=%s n=%zu\n",
                     field_label, n);
    }
    emit_csv("spmv", field_label, n, n, 1, regime_buf, seed, mean_ns, tput);

    FFLAS::sparse_delete(A);
    FFLAS::fflas_delete(x);
    FFLAS::fflas_delete(y);
}

// Bench sparse×dense: C = A·B where A sparse CSR (n×n), B dense (n×n).
// Uses fflas-ffpack's `fspmm` with `blockSize = n` and `ldb = ldc = n`.
template <typename Field>
static void bench_spmm(const Field& F,
                       const char* field_label,
                       size_t n,
                       double density,
                       uint64_t seed,
                       int warmup, int iters) {
    using Element = typename Field::Element;

    std::vector<uint64_t> row_ptr_u64;
    std::vector<uint64_t> col_idx_u64;
    std::vector<Element> values;
    build_csr_uniform(F, n, n, density, seed, row_ptr_u64, col_idx_u64, values);
    uint64_t nnz = values.size();
    if (nnz == 0) {
        return;
    }

    using FFLAS::Sparse;
    using FFLAS::SparseMatrix_t;
    using IndexT = uint64_t;

    Sparse<Field, SparseMatrix_t::CSR> A;
    std::vector<IndexT> row_idx_for_init(nnz);
    {
        size_t k = 0;
        for (size_t i = 0; i < n; ++i) {
            for (size_t e = row_ptr_u64[i]; e < row_ptr_u64[i + 1]; ++e) {
                row_idx_for_init[k++] = i;
            }
        }
    }
    FFLAS::sparse_init(F, A,
                       row_idx_for_init.data(),
                       reinterpret_cast<const IndexT*>(col_idx_u64.data()),
                       values.data(),
                       static_cast<uint64_t>(n),
                       static_cast<uint64_t>(n),
                       nnz);

    typename Field::Element_ptr B = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr C = FFLAS::fflas_new(F, n * n);
    {
        uint64_t st = seed ^ 0xDEADBEEFULL;
        typename Field::Residu_t card = F.cardinality();
        for (size_t i = 0; i < n * n; ++i) {
            uint64_t r = splitmix64(st);
            typename Field::Element bi;
            F.init(bi, static_cast<int64_t>(r % static_cast<uint64_t>(card)));
            B[i] = bi;
        }
    }

    auto run_once = [&]() {
        FFLAS::fzero(F, n * n, C, 1);
        FFLAS::fspmm(F, A, n, B, static_cast<int>(n), F.zero, C, static_cast<int>(n));
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
    // Throughput = nnz · n (each non-zero contributes n MAC pairs).
    double tput = (static_cast<double>(nnz) * static_cast<double>(n))
                  / (static_cast<double>(mean_ns) * 1.0e-9);

    char regime_buf[64];
    std::snprintf(regime_buf, sizeof(regime_buf), "density_%.6e_csr", density);
    if (early_exit) {
        std::fprintf(stderr,
                     "[fflas_sparse_bench] WARN early_exit op=sparse×dense field=%s n=%zu\n",
                     field_label, n);
    }
    emit_csv("sparse×dense", field_label, n, n, n, regime_buf, seed, mean_ns, tput);

    FFLAS::sparse_delete(A);
    FFLAS::fflas_delete(B);
    FFLAS::fflas_delete(C);
}

template <typename Field>
static void run_field(const Field& F,
                      const char* field_label,
                      uint64_t master_seed,
                      int warmup, int iters,
                      const std::vector<size_t>& sizes) {
    std::fprintf(stderr, "[fflas_sparse_bench] field=%s sizes=", field_label);
    for (size_t n : sizes) std::fprintf(stderr, "%zu ", n);
    std::fprintf(stderr, "\n");

    for (size_t si = 0; si < sizes.size(); ++si) {
        size_t n = sizes[si];
        double density = 10.0 / static_cast<double>(n);

        bench_spmv(F, field_label, n, density,
                   derive_seed(master_seed, "spmv-er", 0, si, 1),
                   warmup, iters);

        bench_spmm(F, field_label, n, density,
                   derive_seed(master_seed, "spmm-er", 1, si, 1),
                   warmup, iters);
    }
}

}  // namespace

int main(int argc, char** argv) {
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
                         "usage: fflas_sparse_bench [--seed N] [--warmup K] [--iters K] "
                         "[--smoke] [--quick|--full]\n");
            return 2;
        }
    }

    std::fprintf(stderr,
                 "[fflas_sparse_bench] master_seed=0x%llx warmup=%d iters=%d smoke=%d full=%d\n",
                 static_cast<unsigned long long>(master_seed), warmup, iters,
                 smoke ? 1 : 0, full ? 1 : 0);

    if (smoke) {
        // Smoke is a no-op for now — the cross-equality oracle lives in
        // sparse_smoke.cpp (per § 6 of the protocol). We still return 0
        // so smoke.sh doesn't fail.
        std::fprintf(stderr, "[fflas_sparse_bench] smoke OK (no-op; see sparse_smoke)\n");
        return 0;
    }

    const std::vector<size_t> quick_sizes = {1024};
    const std::vector<size_t> full_sizes  = {1024, 4096, 16384};
    const std::vector<size_t>& sizes = full ? full_sizes : quick_sizes;

    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        run_field(F, "GF(2^31-1)", master_seed, warmup, iters, sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        run_field(F, "GF(65521)", master_seed ^ 0x11ULL, warmup, iters, sizes);
    }
    {
        using Field = Givaro::Modular<float>;
        Field F(251.0f);
        run_field(F, "GF(251)", master_seed ^ 0x22ULL, warmup, iters, sizes);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        run_field(F, "GF(7)", master_seed ^ 0x33ULL, warmup, iters, sizes);
    }

    return 0;
}
