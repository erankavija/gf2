// benchmarks/reference/sparse_smoke.cpp
//
// Cross-equality oracle for the sparse cells promoted by
// `dev/plans/sparse_benchmark_corpus.md` § 4. Implements protocol § 6
// *Comparable semantics* at `n = 16` for every claimed cross-library
// cell:
//
//   - `spmv × GF(p)` — fflas-ffpack `fspmv` against an in-harness
//     scalar reference (independent O(n²) kernel that does not call
//     fflas-ffpack), exact equality after canonical [0, p) reduction.
//   - `spmv × GF(2)` — same scheme using `Modular<int64_t>(2)` for the
//     fflas side; LinBox cross-check via `SparseMatrix::apply` is
//     implicit (we exercise it in `linbox_sparse_bench --smoke` future
//     work). Today we run the fflas-side oracle.
//
// The harness exits non-zero on any mismatch with a stderr trace
// identifying the (op, field, cell) pair.
//
// Determinism: every cell uses the same `gf2_bench_splitmix64` /
// `gf2_bench_derive_seed` from `seed_helpers.h`, byte-equivalent with
// the gf2-core Rust harness's seed walk.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include <givaro/modular.h>
#include <fflas-ffpack/fflas/fflas.h>
#include <fflas-ffpack/fflas/fflas_sparse.h>

#include "seed_helpers.h"

namespace {

static inline uint64_t splitmix64(uint64_t& state) {
    return gf2_bench_splitmix64(&state);
}

// Build a CSR + values tuple deterministically from `seed` matching the
// gf2-core / fflas_sparse_bench convention.
template <typename Field>
static void build_csr(const Field& F,
                      size_t m_rows, size_t n_cols, double density,
                      uint64_t seed,
                      std::vector<uint64_t>& row_idx_full,
                      std::vector<uint64_t>& col_idx,
                      std::vector<typename Field::Element>& values) {
    uint64_t st = seed;
    typename Field::Residu_t card = F.cardinality();
    uint64_t threshold = static_cast<uint64_t>(density * 1.844674407370955e19);
    for (size_t i = 0; i < m_rows; ++i) {
        for (size_t j = 0; j < n_cols; ++j) {
            uint64_t draw = splitmix64(st);
            if (draw < threshold) {
                uint64_t v_raw = splitmix64(st);
                uint64_t v = (v_raw % static_cast<uint64_t>(card - 1)) + 1;
                typename Field::Element x;
                F.init(x, static_cast<int64_t>(v));
                row_idx_full.push_back(i);
                col_idx.push_back(j);
                values.push_back(x);
            }
        }
    }
}

// Independent scalar SpMV: y[i] = sum_j A[i,j]·x[j], no fflas call.
// All arithmetic in canonical [0, card) via Field::init/add/mul.
template <typename Field>
static void scalar_spmv(const Field& F,
                        size_t m_rows, size_t /*n_cols*/,
                        const std::vector<uint64_t>& row_idx_full,
                        const std::vector<uint64_t>& col_idx,
                        const std::vector<typename Field::Element>& values,
                        const typename Field::Element_ptr x,
                        typename Field::Element_ptr y) {
    for (size_t i = 0; i < m_rows; ++i) F.init(y[i], 0);
    for (size_t k = 0; k < values.size(); ++k) {
        size_t i = row_idx_full[k];
        size_t j = col_idx[k];
        typename Field::Element prod;
        F.mul(prod, values[k], x[j]);
        F.addin(y[i], prod);
    }
}

template <typename Field>
static int oracle_spmv(const Field& F, const char* field_label, uint64_t seed) {
    constexpr size_t n = 16;
    constexpr double density = 0.25;  // n=16 needs higher density for non-trivial coverage

    std::vector<uint64_t> row_idx_full;
    std::vector<uint64_t> col_idx;
    std::vector<typename Field::Element> values;
    build_csr(F, n, n, density, seed, row_idx_full, col_idx, values);
    if (values.empty()) {
        std::fprintf(stderr,
                     "[sparse_smoke] WARN nnz=0 op=spmv field=%s\n", field_label);
        return 0;
    }

    typename Field::Element_ptr x = FFLAS::fflas_new(F, n);
    typename Field::Element_ptr y_fflas = FFLAS::fflas_new(F, n);
    typename Field::Element_ptr y_scalar = FFLAS::fflas_new(F, n);
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

    // Build the fflas CSR matrix.
    using FFLAS::Sparse;
    using FFLAS::SparseMatrix_t;
    Sparse<Field, SparseMatrix_t::CSR> A;
    FFLAS::sparse_init(F, A,
                       row_idx_full.data(),
                       col_idx.data(),
                       values.data(),
                       static_cast<uint64_t>(n),
                       static_cast<uint64_t>(n),
                       values.size());

    FFLAS::fzero(F, n, y_fflas, 1);
    FFLAS::fspmv(F, A, x, F.zero, y_fflas);
    scalar_spmv(F, n, n, row_idx_full, col_idx, values, x, y_scalar);

    int rc = 0;
    for (size_t i = 0; i < n; ++i) {
        if (!F.areEqual(y_fflas[i], y_scalar[i])) {
            std::fprintf(stderr,
                         "[sparse_smoke] FAIL spmv field=%s i=%zu fflas != scalar\n",
                         field_label, i);
            rc = 1;
            break;
        }
    }
    if (rc == 0) {
        std::fprintf(stderr, "[sparse_smoke] OK spmv field=%s nnz=%zu\n",
                     field_label, values.size());
    }

    FFLAS::sparse_delete(A);
    FFLAS::fflas_delete(x);
    FFLAS::fflas_delete(y_fflas);
    FFLAS::fflas_delete(y_scalar);
    return rc;
}

}  // namespace

int main(int argc, char** argv) {
    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = static_cast<uint64_t>(std::strtoull(argv[++i], nullptr, 0));
        }
    }

    int rc = 0;
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        rc |= oracle_spmv(F, "GF(2^31-1)",
                          gf2_bench_derive_seed(master_seed, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        rc |= oracle_spmv(F, "GF(65521)",
                          gf2_bench_derive_seed(master_seed ^ 0x11ULL, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<float>;
        Field F(251.0f);
        rc |= oracle_spmv(F, "GF(251)",
                          gf2_bench_derive_seed(master_seed ^ 0x22ULL, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        rc |= oracle_spmv(F, "GF(7)",
                          gf2_bench_derive_seed(master_seed ^ 0x33ULL, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        rc |= oracle_spmv(F, "GF(2)",
                          gf2_bench_derive_seed(master_seed ^ 0x55ULL, "smoke-spmv", 0, 0, 0));
    }

    if (rc != 0) {
        std::fprintf(stderr, "[sparse_smoke] FAIL — %d cell(s) mismatched\n", rc);
        return 1;
    }
    std::fprintf(stderr, "[sparse_smoke] all OK\n");
    return 0;
}
