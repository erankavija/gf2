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
//     fflas side; jit:0f708b36 added a LinBox oracle alongside the
//     fflas one so the smoke chain has two independent witnesses for
//     the GF(2) spmv cell.
//   - `sparse×dense × GF(p)` — fflas-ffpack `fspmm` against an
//     in-harness scalar reference. Same field set as `spmv × GF(p)`;
//     blockSize = n so the cross-check exercises the full
//     `C = A·B` shape promoted in scorecard § 3.
//   - `linbox_spmv × GF(*)` — LinBox `SparseMatrix::apply(y, x)` against
//     the same in-harness scalar reference, providing an independent
//     witness for both `spmv × GF(2)` (per `dev/plans/
//     sparse_benchmark_corpus.md:162`) and the GF(p) cells.
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

#include <linbox/ring/modular.h>
#include <linbox/matrix/sparse-matrix.h>
#include <linbox/util/commentator.h>

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

// LinBox-side spmv oracle (jit:0f708b36) — independent witness alongside
// `oracle_spmv`. Builds a `LinBox::SparseMatrix<Field>` (default storage,
// `SparseSeq` of (col, val) pairs) from the same byte-equivalent CSR
// triples the fflas oracle uses, runs `apply(y, x)`, and bitwise-compares
// the output against the same in-harness `scalar_spmv` reference.
//
// Together with `oracle_spmv` this gives the smoke chain two independent
// library-side witnesses (fflas + LinBox) for every GF(*) `spmv` cell —
// closing the gap recorded in `dev/plans/sparse_benchmark_corpus.md:162`
// for GF(2) and providing redundant coverage for the GF(p) cells.
template <typename Field>
static int linbox_oracle_spmv(const Field& F,
                              const char* field_label,
                              uint64_t seed) {
    constexpr size_t n = 16;
    constexpr double density = 0.25;

    std::vector<uint64_t> row_idx_full;
    std::vector<uint64_t> col_idx;
    std::vector<typename Field::Element> values;
    build_csr(F, n, n, density, seed, row_idx_full, col_idx, values);
    if (values.empty()) {
        std::fprintf(stderr,
                     "[sparse_smoke] WARN nnz=0 op=linbox_spmv field=%s\n", field_label);
        return 0;
    }

    // Allocate the in-harness reference + input via fflas_new so the
    // canonical [0, card) representation matches the existing oracle.
    typename Field::Element_ptr x = FFLAS::fflas_new(F, n);
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

    // Build the LinBox sparse matrix from the same triples, then apply.
    using LBMatrix = LinBox::SparseMatrix<Field>;
    LBMatrix A(F, n, n);
    for (size_t k = 0; k < values.size(); ++k) {
        A.setEntry(row_idx_full[k], col_idx[k], values[k]);
    }

    using Vec = typename LinBox::DenseVector<Field>;
    Vec x_lb(F, n);
    Vec y_lb(F, n);
    for (size_t i = 0; i < n; ++i) {
        x_lb[i] = x[i];
    }
    A.apply(y_lb, x_lb);

    scalar_spmv(F, n, n, row_idx_full, col_idx, values, x, y_scalar);

    int rc = 0;
    for (size_t i = 0; i < n; ++i) {
        if (!F.areEqual(y_lb[i], y_scalar[i])) {
            std::fprintf(stderr,
                         "[sparse_smoke] FAIL linbox_spmv field=%s i=%zu linbox != scalar\n",
                         field_label, i);
            rc = 1;
            break;
        }
    }
    if (rc == 0) {
        std::fprintf(stderr, "[sparse_smoke] OK linbox_spmv field=%s nnz=%zu\n",
                     field_label, values.size());
    }

    FFLAS::fflas_delete(x);
    FFLAS::fflas_delete(y_scalar);
    return rc;
}

// Independent scalar sparse×dense: C = A·B with A sparse (m×n) and
// B dense (n×blockSize). Loops over non-zeros first, then over the
// columns of B/C — does not call fflas. All arithmetic in canonical
// [0, card) via Field::init/add/mul.
template <typename Field>
static void scalar_sparse_dense(const Field& F,
                                size_t m_rows,
                                size_t blockSize,
                                const std::vector<uint64_t>& row_idx_full,
                                const std::vector<uint64_t>& col_idx,
                                const std::vector<typename Field::Element>& values,
                                const typename Field::Element_ptr B,
                                size_t ldb,
                                typename Field::Element_ptr C,
                                size_t ldc) {
    for (size_t i = 0; i < m_rows; ++i) {
        for (size_t j = 0; j < blockSize; ++j) {
            F.init(C[i * ldc + j], 0);
        }
    }
    for (size_t k = 0; k < values.size(); ++k) {
        size_t i = row_idx_full[k];
        size_t kk = col_idx[k];
        for (size_t j = 0; j < blockSize; ++j) {
            typename Field::Element prod;
            F.mul(prod, values[k], B[kk * ldb + j]);
            F.addin(C[i * ldc + j], prod);
        }
    }
}

template <typename Field>
static int oracle_sparse_dense(const Field& F, const char* field_label, uint64_t seed) {
    constexpr size_t n = 16;
    constexpr double density = 0.25;

    std::vector<uint64_t> row_idx_full;
    std::vector<uint64_t> col_idx;
    std::vector<typename Field::Element> values;
    build_csr(F, n, n, density, seed, row_idx_full, col_idx, values);
    if (values.empty()) {
        std::fprintf(stderr,
                     "[sparse_smoke] WARN nnz=0 op=sparse_dense field=%s\n", field_label);
        return 0;
    }

    typename Field::Element_ptr B = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr C_fflas = FFLAS::fflas_new(F, n * n);
    typename Field::Element_ptr C_scalar = FFLAS::fflas_new(F, n * n);
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

    FFLAS::fzero(F, n, n, C_fflas, n);
    FFLAS::fspmm(F, A, n, B, static_cast<int>(n), F.zero, C_fflas, static_cast<int>(n));
    scalar_sparse_dense(F, n, n, row_idx_full, col_idx, values, B, n, C_scalar, n);

    int rc = 0;
    for (size_t i = 0; i < n; ++i) {
        for (size_t j = 0; j < n; ++j) {
            if (!F.areEqual(C_fflas[i * n + j], C_scalar[i * n + j])) {
                std::fprintf(stderr,
                             "[sparse_smoke] FAIL sparse_dense field=%s "
                             "i=%zu j=%zu fflas != scalar\n",
                             field_label, i, j);
                rc = 1;
                break;
            }
        }
        if (rc) break;
    }
    if (rc == 0) {
        std::fprintf(stderr, "[sparse_smoke] OK sparse_dense field=%s nnz=%zu\n",
                     field_label, values.size());
    }

    FFLAS::sparse_delete(A);
    FFLAS::fflas_delete(B);
    FFLAS::fflas_delete(C_fflas);
    FFLAS::fflas_delete(C_scalar);
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

    // Silence LinBox commentator chatter (it goes to stderr by default and
    // pollutes the smoke trace).
    LinBox::commentator().setMaxDetailLevel(-1);
    LinBox::commentator().setMaxDepth(0);
    LinBox::commentator().setReportStream(std::cerr);

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

    // LinBox-side spmv oracle (jit:0f708b36). Independent witness alongside
    // the fflas oracles above. GF(2) is the explicit gap-closing target
    // from `dev/plans/sparse_benchmark_corpus.md:162`; the GF(p) cells get
    // redundant coverage for free since LinBox's `SparseMatrix::apply` is
    // a separate code path from fflas-ffpack's `fspmv`. Seeds match the
    // fflas oracle calls so both witnesses see the same triples + RHS.
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        rc |= linbox_oracle_spmv(F, "GF(2^31-1)",
                                 gf2_bench_derive_seed(master_seed, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        rc |= linbox_oracle_spmv(F, "GF(65521)",
                                 gf2_bench_derive_seed(master_seed ^ 0x11ULL, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(251);
        rc |= linbox_oracle_spmv(F, "GF(251)",
                                 gf2_bench_derive_seed(master_seed ^ 0x22ULL, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        rc |= linbox_oracle_spmv(F, "GF(7)",
                                 gf2_bench_derive_seed(master_seed ^ 0x33ULL, "smoke-spmv", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        rc |= linbox_oracle_spmv(F, "GF(2)",
                                 gf2_bench_derive_seed(master_seed ^ 0x55ULL, "smoke-spmv", 0, 0, 0));
    }

    // sparse×dense × GF(p) — fspmm cross-equality oracle for the four
    // GF(p) primes the design doc promotes fflas-ffpack as canonical
    // for, plus GF(2) (jit:521390db). The oracle_sparse_dense template
    // calls fflas-ffpack's `fspmm` against an in-harness scalar
    // reference (`scalar_sparse_dense`), satisfying protocol § 6
    // *Comparable semantics* without depending on the gf2-core
    // integration follow-up (96fde7c7, upstream design work).
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        rc |= oracle_sparse_dense(F, "GF(2^31-1)",
                                  gf2_bench_derive_seed(master_seed, "smoke-spmm", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        rc |= oracle_sparse_dense(F, "GF(65521)",
                                  gf2_bench_derive_seed(master_seed ^ 0x11ULL, "smoke-spmm", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<float>;
        Field F(251.0f);
        rc |= oracle_sparse_dense(F, "GF(251)",
                                  gf2_bench_derive_seed(master_seed ^ 0x22ULL, "smoke-spmm", 0, 0, 0));
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        rc |= oracle_sparse_dense(F, "GF(7)",
                                  gf2_bench_derive_seed(master_seed ^ 0x33ULL, "smoke-spmm", 0, 0, 0));
    }
    // GF(2) sparse×dense — added jit:521390db. Uses `Modular<int64_t>(2)`
    // to match the GF(2) spmv smoke above; the in-harness scalar
    // reference (`scalar_sparse_dense`) provides the canonical semantic
    // anchor against fflas's `fspmm`. The gf2-core side now exposes
    // `SpBitMatrix::matmat`; the LinBox-side `applyLeft` reference is
    // still tracked as `not-yet-harnessed` (sibling 0f708b36).
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        rc |= oracle_sparse_dense(F, "GF(2)",
                                  gf2_bench_derive_seed(master_seed ^ 0x55ULL, "smoke-spmm", 0, 0, 0));
    }

    if (rc != 0) {
        std::fprintf(stderr, "[sparse_smoke] FAIL — %d cell(s) mismatched\n", rc);
        return 1;
    }
    std::fprintf(stderr, "[sparse_smoke] all OK\n");
    return 0;
}
