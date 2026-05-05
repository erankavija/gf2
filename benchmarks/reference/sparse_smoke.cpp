// benchmarks/reference/sparse_smoke.cpp
//
// Cross-equality oracle for the sparse cells promoted by
// `dev/plans/sparse_benchmark_corpus.md` § 4. Implements protocol § 6
// *Comparable semantics* at `n = 16` for every claimed cross-library
// cell:
//
//   - `spmv × GF(p)` — fflas-ffpack `fspmv` against the gf2-core
//     ground-truth output (`SparseFieldMatrix::<Fp<P>>::matvec`); the
//     ground-truth bytes live in
//     `benchmarks/expected/sparse_smoke_n16.bin`, regenerated on every
//     `benchmarks/smoke.sh` run by the
//     `sparse_smoke_emit_expected` Cargo example. Exact equality
//     after canonical [0, p) reduction.
//   - `spmv × GF(2)` — same scheme using `Modular<int64_t>(2)` for the
//     fflas side; jit:0f708b36 added a LinBox oracle alongside the
//     fflas one so the smoke chain has two independent witnesses for
//     the GF(2) spmv cell. Both candidates are compared against the
//     gf2-core `SpBitMatrix::matvec` ground-truth output.
//   - `sparse×dense × GF(p)` — fflas-ffpack `fspmm` against the
//     gf2-core `SparseFieldMatrix::<Fp<P>>::matmat` ground-truth.
//     Same field set as `spmv × GF(p)`; blockSize = n so the
//     cross-check exercises the full `C = A·B` shape promoted in
//     scorecard § 3.
//   - `sparse×dense × GF(2)` — same scheme using gf2-core
//     `SpBitMatrix::matmat` (jit:521390db) as ground-truth.
//   - `linbox_spmv × GF(*)` — LinBox `SparseMatrix::apply(y, x)` against
//     the same gf2-core ground-truth, providing an independent witness
//     for both `spmv × GF(2)` (per `dev/plans/
//     sparse_benchmark_corpus.md:162`) and the GF(p) cells.
//   - `sparse_elim × {GF(2), GF(p)}` — LinBox `GaussDomain::NoReordering`
//     rank against the gf2-core `SpBitMatrix::rref` /
//     `SparseFieldMatrix::<Fp<P>>::rref` rank (and full RREF-content
//     bitwise equality against the in-harness scalar dense Gauss-Jordan
//     reference, since LinBox's NoReordering destroys the matrix
//     content during elimination). Promoted by jit:96fde7c7.
//
// The harness exits non-zero on any mismatch with a stderr trace
// identifying the (op, field, cell) pair.
//
// Determinism / ground-truth integrity:
//   * Every cell uses the same `gf2_bench_splitmix64` /
//     `gf2_bench_derive_seed` from `seed_helpers.h`, byte-equivalent
//     with the gf2-core Rust harness's seed walk.
//   * Per cell, before the candidate runs, we assert byte-equality
//     between (a) the input we just built and (b) the input bytes the
//     gf2-core emitter recorded for the same cell tag. A drift here
//     fails the smoke immediately with `seed-walk drift` and
//     identifies the cell.
//   * After the candidate runs, we assert byte-equality between the
//     candidate output and the gf2-core ground-truth output.
//   * Self-test mode (`--self-test`) loads the ground-truth file,
//     verifies the structure parses, and prints the cell list without
//     running candidates. Useful for debugging file-format drift in
//     isolation from the candidate libraries.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <string>
#include <vector>

#include <givaro/modular.h>
#include <fflas-ffpack/fflas/fflas.h>
#include <fflas-ffpack/fflas/fflas_sparse.h>
#include <linbox/matrix/sparse-matrix.h>
#include <linbox/algorithms/gauss.h>
#include <linbox/util/commentator.h>

#include <linbox/ring/modular.h>
#include <linbox/matrix/sparse-matrix.h>
#include <linbox/util/commentator.h>

#include "seed_helpers.h"

namespace {

static inline uint64_t splitmix64(uint64_t& state) {
    return gf2_bench_splitmix64(&state);
}

// ─── Ground-truth file loader (mechanism (b) per jit:96fde7c7) ──────────────

// One per-cell record from the binary `benchmarks/expected/sparse_smoke_n16.bin`
// emitted by the `sparse_smoke_emit_expected` Cargo example. The format is
// documented at the top of that file.
struct ExpectedCell {
    uint64_t seed;
    std::vector<uint8_t> in;
    std::vector<uint8_t> out;
};

using ExpectedTable = std::map<std::string, ExpectedCell>;

static const char* kGroundTruthPathEnv = "GF2_SPARSE_SMOKE_EXPECTED";
static const char* kGroundTruthDefaultPath =
    "benchmarks/expected/sparse_smoke_n16.bin";

// Magic bytes — must match the emitter's `MAGIC` constant.
static const char kMagic[8] = {'G', 'F', '2', 'S', 'M', 'K', '0', '1'};

// Read a little-endian primitive at `cursor` from `buf` and advance
// `cursor` by sizeof(T). On overflow, sets `ok=false` and returns 0.
template <typename T>
static T read_le(const std::vector<uint8_t>& buf, size_t& cursor, bool& ok) {
    if (!ok || cursor + sizeof(T) > buf.size()) {
        ok = false;
        return T{};
    }
    T v = 0;
    for (size_t i = 0; i < sizeof(T); ++i) {
        v |= static_cast<T>(buf[cursor + i]) << (8 * i);
    }
    cursor += sizeof(T);
    return v;
}

static bool load_expected(const std::string& path, ExpectedTable& out) {
    std::ifstream f(path, std::ios::binary);
    if (!f) {
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL ground-truth file missing at %s; "
                     "regenerate via `cargo run --release -p gf2-coding "
                     "--example sparse_smoke_emit_expected --features bench-csv "
                     "-- --output %s`\n",
                     path.c_str(), path.c_str());
        return false;
    }
    std::vector<uint8_t> buf((std::istreambuf_iterator<char>(f)),
                             std::istreambuf_iterator<char>());
    if (buf.size() < 12 || std::memcmp(buf.data(), kMagic, 8) != 0) {
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL ground-truth magic mismatch at %s "
                     "(expected GF2SMK01)\n",
                     path.c_str());
        return false;
    }
    size_t cursor = 8;
    bool ok = true;
    uint32_t n_cells = read_le<uint32_t>(buf, cursor, ok);
    for (uint32_t i = 0; i < n_cells && ok; ++i) {
        uint16_t tag_len = read_le<uint16_t>(buf, cursor, ok);
        if (!ok || cursor + tag_len > buf.size()) {
            ok = false;
            break;
        }
        std::string tag(reinterpret_cast<const char*>(buf.data() + cursor),
                        tag_len);
        cursor += tag_len;
        uint64_t seed = read_le<uint64_t>(buf, cursor, ok);
        uint32_t in_len = read_le<uint32_t>(buf, cursor, ok);
        if (!ok || cursor + in_len > buf.size()) {
            ok = false;
            break;
        }
        std::vector<uint8_t> input(buf.begin() + cursor,
                                   buf.begin() + cursor + in_len);
        cursor += in_len;
        uint32_t out_len = read_le<uint32_t>(buf, cursor, ok);
        if (!ok || cursor + out_len > buf.size()) {
            ok = false;
            break;
        }
        std::vector<uint8_t> output(buf.begin() + cursor,
                                    buf.begin() + cursor + out_len);
        cursor += out_len;
        ExpectedCell cell;
        cell.seed = seed;
        cell.in = std::move(input);
        cell.out = std::move(output);
        out.emplace(std::move(tag), std::move(cell));
    }
    if (!ok) {
        std::fprintf(stderr, "[sparse_smoke] FAIL ground-truth parse error at %s\n",
                     path.c_str());
        return false;
    }
    return true;
}

// Append a u64 LE to `buf`.
static inline void append_u64_le(std::vector<uint8_t>& buf, uint64_t v) {
    for (int i = 0; i < 8; ++i) {
        buf.push_back(static_cast<uint8_t>((v >> (8 * i)) & 0xFFu));
    }
}

// ─── CSR builder (matched against the Rust emitter's build_csr_cpp_walk) ───

// Build a CSR + values tuple deterministically from `seed` matching the
// gf2-core / fflas_sparse_bench convention. The exact splitmix walk
// rule must stay byte-for-byte identical to the Rust emitter; the L1
// assertion in each oracle catches drift between the two
// implementations.
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

// Serialise the (triples + dense vec) input layout the Rust emitter
// uses for spmv cells, so the L1 assertion can compare bytes.
template <typename Field>
static std::vector<uint8_t> serialise_spmv_input(
    const Field& F,
    const std::vector<uint64_t>& row_idx_full,
    const std::vector<uint64_t>& col_idx,
    const std::vector<typename Field::Element>& values,
    const typename Field::Element_ptr x,
    size_t n) {
    std::vector<uint8_t> buf;
    buf.reserve(8 + 24 * values.size() + 8 + 8 * n);
    append_u64_le(buf, values.size());
    for (size_t k = 0; k < values.size(); ++k) {
        append_u64_le(buf, row_idx_full[k]);
        append_u64_le(buf, col_idx[k]);
        // Convert the field element back to its canonical [0, card) u64.
        typename Field::Element tmp = values[k];
        int64_t r;
        F.convert(r, tmp);
        append_u64_le(buf, static_cast<uint64_t>(r));
    }
    append_u64_le(buf, n);
    for (size_t i = 0; i < n; ++i) {
        int64_t r;
        F.convert(r, x[i]);
        append_u64_le(buf, static_cast<uint64_t>(r));
    }
    return buf;
}

template <typename Field>
static std::vector<uint8_t> serialise_spmv_output(
    const Field& F,
    const typename Field::Element_ptr y,
    size_t n) {
    std::vector<uint8_t> buf;
    buf.reserve(8 + 8 * n);
    append_u64_le(buf, n);
    for (size_t i = 0; i < n; ++i) {
        int64_t r;
        F.convert(r, y[i]);
        append_u64_le(buf, static_cast<uint64_t>(r));
    }
    return buf;
}

// `sparse_dense` input: triples + B (dense rows × cols, row-major).
template <typename Field>
static std::vector<uint8_t> serialise_sparse_dense_input(
    const Field& F,
    const std::vector<uint64_t>& row_idx_full,
    const std::vector<uint64_t>& col_idx,
    const std::vector<typename Field::Element>& values,
    const typename Field::Element_ptr B,
    size_t rows, size_t cols) {
    std::vector<uint8_t> buf;
    buf.reserve(8 + 24 * values.size() + 16 + 8 * rows * cols);
    append_u64_le(buf, values.size());
    for (size_t k = 0; k < values.size(); ++k) {
        append_u64_le(buf, row_idx_full[k]);
        append_u64_le(buf, col_idx[k]);
        typename Field::Element tmp = values[k];
        int64_t r;
        F.convert(r, tmp);
        append_u64_le(buf, static_cast<uint64_t>(r));
    }
    append_u64_le(buf, rows);
    append_u64_le(buf, cols);
    for (size_t i = 0; i < rows * cols; ++i) {
        int64_t r;
        F.convert(r, B[i]);
        append_u64_le(buf, static_cast<uint64_t>(r));
    }
    return buf;
}

template <typename Field>
static std::vector<uint8_t> serialise_sparse_dense_output(
    const Field& F,
    const typename Field::Element_ptr C,
    size_t rows, size_t cols) {
    std::vector<uint8_t> buf;
    buf.reserve(16 + 8 * rows * cols);
    append_u64_le(buf, rows);
    append_u64_le(buf, cols);
    for (size_t i = 0; i < rows * cols; ++i) {
        int64_t r;
        F.convert(r, C[i]);
        append_u64_le(buf, static_cast<uint64_t>(r));
    }
    return buf;
}

// `sparse_elim` input: just the triples (no dense vec).
template <typename Field>
static std::vector<uint8_t> serialise_sparse_elim_input(
    const Field& F,
    const std::vector<uint64_t>& row_idx_full,
    const std::vector<uint64_t>& col_idx,
    const std::vector<typename Field::Element>& values) {
    std::vector<uint8_t> buf;
    buf.reserve(8 + 24 * values.size());
    append_u64_le(buf, values.size());
    for (size_t k = 0; k < values.size(); ++k) {
        append_u64_le(buf, row_idx_full[k]);
        append_u64_le(buf, col_idx[k]);
        typename Field::Element tmp = values[k];
        int64_t r;
        F.convert(r, tmp);
        append_u64_le(buf, static_cast<uint64_t>(r));
    }
    return buf;
}

// Failure helper — writes the canonical `[sparse_smoke] FAIL …`
// message and returns 1 so the oracle pattern stays compact.
static int fail_l1_drift(const std::string& tag) {
    std::fprintf(stderr,
                 "[sparse_smoke] FAIL %s seed-walk drift detected; "
                 "gf2-core emitter must be regenerated for cell %s\n",
                 tag.c_str(), tag.c_str());
    return 1;
}

static int fail_output_mismatch(const std::string& op,
                                const std::string& field,
                                const std::string& tag) {
    std::fprintf(stderr,
                 "[sparse_smoke] FAIL %s field=%s candidate output != "
                 "gf2-core expected; cell %s\n",
                 op.c_str(), field.c_str(), tag.c_str());
    return 1;
}

// Independent scalar SpMV: y[i] = sum_j A[i,j]·x[j], no fflas call.
// All arithmetic in canonical [0, card) via Field::init/add/mul.
//
// Retained as a secondary witness alongside the gf2-core ground-truth
// (gf2-core remains the trusted oracle; this provides the legacy
// scalar-vs-fflas check the smoke shipped with — failing here means
// the in-harness reference disagrees with both fflas AND gf2-core,
// which is a useful triangulation signal during regressions).
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
static int oracle_spmv(const Field& F, const char* field_label, uint64_t seed,
                       const ExpectedTable& et) {
    constexpr size_t n = 16;
    constexpr double density = 0.25;

    std::string tag = std::string("spmv,") + field_label;
    auto it = et.find(tag);
    if (it == et.end()) {
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL %s missing from ground-truth file\n",
                     tag.c_str());
        return 1;
    }

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

    // L1: assert local input == ground-truth input (seed-walk equivalence).
    auto local_in = serialise_spmv_input(F, row_idx_full, col_idx, values, x, n);
    if (local_in != it->second.in) {
        FFLAS::fflas_delete(x);
        FFLAS::fflas_delete(y_fflas);
        return fail_l1_drift(tag);
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

    // L2: candidate output bytes == gf2-core ground-truth output bytes.
    auto local_out = serialise_spmv_output(F, y_fflas, n);
    int rc = 0;
    if (local_out != it->second.out) {
        rc = fail_output_mismatch("spmv", field_label, tag);
    } else {
        std::fprintf(stderr, "[sparse_smoke] OK spmv field=%s nnz=%zu\n",
                     field_label, values.size());
    }

    FFLAS::sparse_delete(A);
    FFLAS::fflas_delete(x);
    FFLAS::fflas_delete(y_fflas);
    return rc;
}

// LinBox-side spmv oracle (jit:0f708b36) — independent witness alongside
// `oracle_spmv`. Builds a `LinBox::SparseMatrix<Field>` (default storage,
// `SparseSeq` of (col, val) pairs) from the same byte-equivalent CSR
// triples the fflas oracle uses, runs `apply(y, x)`, and bitwise-compares
// the output against the same gf2-core ground-truth (jit:96fde7c7).
//
// Together with `oracle_spmv` this gives the smoke chain two independent
// library-side witnesses (fflas + LinBox) for every GF(*) `spmv` cell —
// closing the gap recorded in `dev/plans/sparse_benchmark_corpus.md:162`
// for GF(2) and providing redundant coverage for the GF(p) cells.
template <typename Field>
static int linbox_oracle_spmv(const Field& F,
                              const char* field_label,
                              uint64_t seed,
                              const ExpectedTable& et) {
    constexpr size_t n = 16;
    constexpr double density = 0.25;

    std::string tag = std::string("spmv,") + field_label;
    auto it = et.find(tag);
    if (it == et.end()) {
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL %s missing from ground-truth file "
                     "(linbox_spmv path)\n", tag.c_str());
        return 1;
    }

    std::vector<uint64_t> row_idx_full;
    std::vector<uint64_t> col_idx;
    std::vector<typename Field::Element> values;
    build_csr(F, n, n, density, seed, row_idx_full, col_idx, values);
    if (values.empty()) {
        std::fprintf(stderr,
                     "[sparse_smoke] WARN nnz=0 op=linbox_spmv field=%s\n", field_label);
        return 0;
    }

    // Allocate the input via fflas_new so the canonical [0, card) representation
    // matches the existing oracle.
    typename Field::Element_ptr x = FFLAS::fflas_new(F, n);
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

    // L1: assert local input == ground-truth input (seed-walk equivalence).
    auto local_in = serialise_spmv_input(F, row_idx_full, col_idx, values, x, n);
    if (local_in != it->second.in) {
        FFLAS::fflas_delete(x);
        return fail_l1_drift(tag);
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

    // L2: serialise LinBox output into the same byte layout as the
    // ground-truth and compare bytewise.
    std::vector<uint8_t> local_out;
    local_out.reserve(8 + 8 * n);
    append_u64_le(local_out, n);
    for (size_t i = 0; i < n; ++i) {
        int64_t r;
        F.convert(r, y_lb[i]);
        append_u64_le(local_out, static_cast<uint64_t>(r));
    }

    int rc = 0;
    if (local_out != it->second.out) {
        rc = fail_output_mismatch("linbox_spmv", field_label, tag);
    } else {
        std::fprintf(stderr, "[sparse_smoke] OK linbox_spmv field=%s nnz=%zu\n",
                     field_label, values.size());
    }

    FFLAS::fflas_delete(x);
    return rc;
}

template <typename Field>
static int oracle_sparse_dense(const Field& F, const char* field_label, uint64_t seed,
                               const ExpectedTable& et) {
    constexpr size_t n = 16;
    constexpr double density = 0.25;

    std::string tag = std::string("sparse_dense,") + field_label;
    auto it = et.find(tag);
    if (it == et.end()) {
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL %s missing from ground-truth file\n",
                     tag.c_str());
        return 1;
    }

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

    // L1: assert local input == ground-truth.
    auto local_in = serialise_sparse_dense_input(F, row_idx_full, col_idx, values, B, n, n);
    if (local_in != it->second.in) {
        FFLAS::fflas_delete(B);
        FFLAS::fflas_delete(C_fflas);
        return fail_l1_drift(tag);
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

    // L3: candidate (fflas) output bytes == gf2-core ground-truth.
    auto local_out = serialise_sparse_dense_output(F, C_fflas, n, n);
    int rc = 0;
    if (local_out != it->second.out) {
        rc = fail_output_mismatch("sparse_dense", field_label, tag);
    } else {
        std::fprintf(stderr, "[sparse_smoke] OK sparse_dense field=%s nnz=%zu\n",
                     field_label, values.size());
    }

    FFLAS::sparse_delete(A);
    FFLAS::fflas_delete(B);
    FFLAS::fflas_delete(C_fflas);
    return rc;
}

// Independent scalar Gauss-Jordan over `Field` to obtain the reduced row
// echelon form (RREF). Mutates `A_dense` in place and returns the rank.
// Layout: row-major, ld = n_cols. All arithmetic in canonical [0, card)
// via Field::init/add/mul/inv.
//
// Retained as a secondary witness against the gf2-core ground-truth so
// the smoke chain has *two* independent RREF implementations agreeing on
// the dense output (gf2-core's `SparseFieldMatrix::rref` and this
// in-harness scalar dense Gauss-Jordan). LinBox's `NoReordering`
// destroys the matrix content (zeros each pivot row in-place) and only
// exports rank — see oracle_sparse_elim for the multi-witness layout.
template <typename Field>
static size_t scalar_rref(const Field& F,
                          size_t m_rows, size_t n_cols,
                          typename Field::Element_ptr A) {
    size_t pivot_row = 0;
    for (size_t col = 0; col < n_cols && pivot_row < m_rows; ++col) {
        // Find a pivot row at or below `pivot_row` with non-zero in `col`.
        size_t found = m_rows;
        for (size_t r = pivot_row; r < m_rows; ++r) {
            if (!F.isZero(A[r * n_cols + col])) {
                found = r;
                break;
            }
        }
        if (found == m_rows) continue;

        // Swap into position.
        if (found != pivot_row) {
            for (size_t j = 0; j < n_cols; ++j) {
                typename Field::Element t = A[pivot_row * n_cols + j];
                A[pivot_row * n_cols + j] = A[found * n_cols + j];
                A[found * n_cols + j] = t;
            }
        }

        // Scale pivot row so leading entry is 1.
        typename Field::Element inv;
        F.inv(inv, A[pivot_row * n_cols + col]);
        for (size_t j = 0; j < n_cols; ++j) {
            F.mulin(A[pivot_row * n_cols + j], inv);
        }

        // Eliminate `col` from every other row.
        for (size_t r = 0; r < m_rows; ++r) {
            if (r == pivot_row) continue;
            typename Field::Element factor = A[r * n_cols + col];
            if (F.isZero(factor)) continue;
            for (size_t j = 0; j < n_cols; ++j) {
                typename Field::Element prod;
                F.mul(prod, factor, A[pivot_row * n_cols + j]);
                F.subin(A[r * n_cols + j], prod);
            }
        }
        ++pivot_row;
    }
    return pivot_row;
}

template <typename Field>
static int oracle_sparse_elim(const Field& F, const char* field_label, uint64_t seed,
                              const ExpectedTable& et) {
    constexpr size_t n = 16;
    constexpr double density = 0.25;

    std::string tag = std::string("sparse_elim,") + field_label;
    auto it = et.find(tag);
    if (it == et.end()) {
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL %s missing from ground-truth file\n",
                     tag.c_str());
        return 1;
    }

    std::vector<uint64_t> row_idx_full;
    std::vector<uint64_t> col_idx;
    std::vector<typename Field::Element> values;
    build_csr(F, n, n, density, seed, row_idx_full, col_idx, values);
    if (values.empty()) {
        std::fprintf(stderr,
                     "[sparse_smoke] WARN nnz=0 op=sparse_elim field=%s\n", field_label);
        return 0;
    }

    // L1: assert local triples == ground-truth triples.
    auto local_in = serialise_sparse_elim_input(F, row_idx_full, col_idx, values);
    if (local_in != it->second.in) {
        return fail_l1_drift(tag);
    }

    // Independent scalar reference: dense row-major Gauss-Jordan.
    typename Field::Element_ptr A_scalar = FFLAS::fflas_new(F, n * n);
    for (size_t i = 0; i < n * n; ++i) F.init(A_scalar[i], 0);
    for (size_t k = 0; k < values.size(); ++k) {
        size_t i = row_idx_full[k];
        size_t j = col_idx[k];
        A_scalar[i * n + j] = values[k];
    }

    // ── LinBox sparse Gauss-Jordan via GaussDomain::NoReordering ──────
    //
    // `NoReordering` is in-place and destructive: it zeros each pivot
    // row after elimination (see linbox/algorithms/gauss/gauss.inl
    // line 813, `LigneA[k] = Vzer;`). Only `rank` and `det` are
    // exported as cross-checkable invariants; the matrix-content output
    // is not useful as an RREF witness. Rank-equality across the two
    // independent libraries is the protocol § 6 invariant we enforce
    // here.
    using LBMatrix = LinBox::SparseMatrix<Field>;
    LBMatrix A_linbox(F, n, n);
    for (size_t k = 0; k < values.size(); ++k) {
        A_linbox.setEntry(row_idx_full[k], col_idx[k], values[k]);
    }
    LinBox::GaussDomain<Field> G(F);
    LinBox::size_t rank_linbox = 0;
    typename Field::Element det;
    F.init(det, 1);
    G.NoReordering(rank_linbox, det, A_linbox, A_linbox.rowdim(), A_linbox.coldim());

    // Independent scalar Gauss-Jordan reference (full RREF).
    size_t rank_scalar = scalar_rref(F, n, n, A_scalar);

    // Parse ground-truth output: `rank: u64`, `m: u64`, `n: u64`,
    // `m*n × val: u64`. (See sparse_smoke_emit_expected.rs file format.)
    const auto& out_bytes = it->second.out;
    if (out_bytes.size() < 24) {
        FFLAS::fflas_delete(A_scalar);
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL sparse_elim field=%s ground-truth "
                     "output too short (%zu bytes)\n",
                     field_label, out_bytes.size());
        return 1;
    }
    uint64_t rank_gt = 0;
    for (int b = 0; b < 8; ++b) {
        rank_gt |= static_cast<uint64_t>(out_bytes[b]) << (8 * b);
    }
    uint64_t rref_rows = 0;
    for (int b = 0; b < 8; ++b) {
        rref_rows |= static_cast<uint64_t>(out_bytes[8 + b]) << (8 * b);
    }
    uint64_t rref_cols = 0;
    for (int b = 0; b < 8; ++b) {
        rref_cols |= static_cast<uint64_t>(out_bytes[16 + b]) << (8 * b);
    }
    if (rref_rows != n || rref_cols != n) {
        FFLAS::fflas_delete(A_scalar);
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL sparse_elim field=%s ground-truth "
                     "RREF dims %llu×%llu != n×n\n",
                     field_label,
                     static_cast<unsigned long long>(rref_rows),
                     static_cast<unsigned long long>(rref_cols));
        return 1;
    }
    size_t expected_size = 24 + 8 * static_cast<size_t>(rref_rows) *
                                static_cast<size_t>(rref_cols);
    if (out_bytes.size() != expected_size) {
        FFLAS::fflas_delete(A_scalar);
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL sparse_elim field=%s ground-truth "
                     "output size %zu != %zu\n",
                     field_label, out_bytes.size(), expected_size);
        return 1;
    }

    int rc = 0;

    // L4 (rank witness): LinBox rank == gf2-core rank.
    if (static_cast<uint64_t>(rank_linbox) != rank_gt) {
        std::fprintf(stderr,
                     "[sparse_smoke] FAIL sparse_elim field=%s rank linbox=%zu "
                     "gf2-core=%llu\n",
                     field_label, static_cast<size_t>(rank_linbox),
                     static_cast<unsigned long long>(rank_gt));
        rc = 1;
    }

    // L5 (RREF content witness): scalar Gauss-Jordan RREF == gf2-core RREF
    // dense bytes. This is a strictly stronger check than the LinBox-rank
    // assertion above — full byte equality of the reduced matrix between
    // two independent implementations (in-harness scalar + gf2-core).
    if (rc == 0) {
        if (static_cast<uint64_t>(rank_scalar) != rank_gt) {
            std::fprintf(stderr,
                         "[sparse_smoke] FAIL sparse_elim field=%s rank scalar=%zu "
                         "gf2-core=%llu (rank witness divergence)\n",
                         field_label, rank_scalar,
                         static_cast<unsigned long long>(rank_gt));
            rc = 1;
        }
    }
    if (rc == 0) {
        for (size_t i = 0; i < n * n && rc == 0; ++i) {
            int64_t scalar_val;
            F.convert(scalar_val, A_scalar[i]);
            uint64_t gt_val = 0;
            for (int b = 0; b < 8; ++b) {
                gt_val |= static_cast<uint64_t>(out_bytes[24 + 8 * i + b])
                          << (8 * b);
            }
            if (static_cast<uint64_t>(scalar_val) != gt_val) {
                std::fprintf(stderr,
                             "[sparse_smoke] FAIL sparse_elim field=%s i=%zu "
                             "scalar_rref=%lld gf2-core=%llu (RREF content drift)\n",
                             field_label, i,
                             static_cast<long long>(scalar_val),
                             static_cast<unsigned long long>(gt_val));
                rc = 1;
            }
        }
    }

    if (rc == 0) {
        std::fprintf(stderr,
                     "[sparse_smoke] OK sparse_elim field=%s nnz=%zu rank=%llu\n",
                     field_label, values.size(),
                     static_cast<unsigned long long>(rank_gt));
    }

    FFLAS::fflas_delete(A_scalar);
    return rc;
}

}  // namespace

int main(int argc, char** argv) {
    // Silence LinBox's commentator (chatty by default; goes to stderr
    // and would corrupt the smoke harness's `[sparse_smoke] OK …` lines
    // if left enabled).
    LinBox::commentator().setMaxDetailLevel(-1);
    LinBox::commentator().setMaxDepth(0);
    LinBox::commentator().setReportStream(std::cerr);

    uint64_t master_seed = 0x6F73AC91D31E4A7CULL;
    bool self_test = false;
    std::string expected_path;
    {
        const char* env = std::getenv(kGroundTruthPathEnv);
        expected_path = env ? std::string(env) : std::string(kGroundTruthDefaultPath);
    }
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            master_seed = static_cast<uint64_t>(std::strtoull(argv[++i], nullptr, 0));
        } else if (std::strcmp(argv[i], "--self-test") == 0) {
            self_test = true;
        } else if (std::strcmp(argv[i], "--expected") == 0 && i + 1 < argc) {
            expected_path = argv[++i];
        }
    }

    // Silence LinBox commentator chatter (it goes to stderr by default and
    // pollutes the smoke trace).
    LinBox::commentator().setMaxDetailLevel(-1);
    LinBox::commentator().setMaxDepth(0);
    LinBox::commentator().setReportStream(std::cerr);

    ExpectedTable et;
    if (!load_expected(expected_path, et)) {
        return 1;
    }

    if (self_test) {
        std::fprintf(stderr, "[sparse_smoke] --self-test loaded %zu cells from %s\n",
                     et.size(), expected_path.c_str());
        for (const auto& kv : et) {
            std::fprintf(stderr,
                         "[sparse_smoke] cell tag=%s seed=0x%016llx in=%zuB out=%zuB\n",
                         kv.first.c_str(),
                         static_cast<unsigned long long>(kv.second.seed),
                         kv.second.in.size(), kv.second.out.size());
        }
        return 0;
    }

    int rc = 0;
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        rc |= oracle_spmv(F, "GF(2^31-1)",
                          gf2_bench_derive_seed(master_seed, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        rc |= oracle_spmv(F, "GF(65521)",
                          gf2_bench_derive_seed(master_seed ^ 0x11ULL, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<float>;
        Field F(251.0f);
        rc |= oracle_spmv(F, "GF(251)",
                          gf2_bench_derive_seed(master_seed ^ 0x22ULL, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        rc |= oracle_spmv(F, "GF(7)",
                          gf2_bench_derive_seed(master_seed ^ 0x33ULL, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        rc |= oracle_spmv(F, "GF(2)",
                          gf2_bench_derive_seed(master_seed ^ 0x55ULL, "smoke-spmv", 0, 0, 0), et);
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
                                 gf2_bench_derive_seed(master_seed, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        rc |= linbox_oracle_spmv(F, "GF(65521)",
                                 gf2_bench_derive_seed(master_seed ^ 0x11ULL, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(251);
        rc |= linbox_oracle_spmv(F, "GF(251)",
                                 gf2_bench_derive_seed(master_seed ^ 0x22ULL, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        rc |= linbox_oracle_spmv(F, "GF(7)",
                                 gf2_bench_derive_seed(master_seed ^ 0x33ULL, "smoke-spmv", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        rc |= linbox_oracle_spmv(F, "GF(2)",
                                 gf2_bench_derive_seed(master_seed ^ 0x55ULL, "smoke-spmv", 0, 0, 0), et);
    }

    // sparse×dense × GF(p) — fspmm cross-equality oracle for the four
    // GF(p) primes the design doc promotes fflas-ffpack as canonical
    // for, plus GF(2) (jit:521390db). The oracle_sparse_dense template
    // calls fflas-ffpack's `fspmm` against the gf2-core
    // `SparseFieldMatrix::matmat` ground-truth, satisfying protocol § 6
    // *Comparable semantics* with the gf2-core integration that 96fde7c7
    // landed.
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        rc |= oracle_sparse_dense(F, "GF(2^31-1)",
                                  gf2_bench_derive_seed(master_seed, "smoke-spmm", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        rc |= oracle_sparse_dense(F, "GF(65521)",
                                  gf2_bench_derive_seed(master_seed ^ 0x11ULL, "smoke-spmm", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<float>;
        Field F(251.0f);
        rc |= oracle_sparse_dense(F, "GF(251)",
                                  gf2_bench_derive_seed(master_seed ^ 0x22ULL, "smoke-spmm", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        rc |= oracle_sparse_dense(F, "GF(7)",
                                  gf2_bench_derive_seed(master_seed ^ 0x33ULL, "smoke-spmm", 0, 0, 0), et);
    }
    // GF(2) sparse×dense — added jit:521390db. Uses `Modular<int64_t>(2)`
    // to match the GF(2) spmv smoke above; the gf2-core
    // `SpBitMatrix::matmat` ground-truth provides the canonical semantic
    // anchor against fflas's `fspmm`.
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        rc |= oracle_sparse_dense(F, "GF(2)",
                                  gf2_bench_derive_seed(master_seed ^ 0x55ULL, "smoke-spmm", 0, 0, 0), et);
    }

    // sparse-elim × {GF(2), GF(p)} — LinBox `GaussDomain::NoReordering`
    // (sparse Gauss-Jordan) cross-checked against the gf2-core
    // `rref()` ground-truth and the in-harness scalar Gauss-Jordan.
    // gf2-core ground-truth provides both rank (matched against
    // LinBox) and dense RREF content (matched against the scalar
    // reference).
    {
        using Field = Givaro::Modular<int64_t>;
        Field F((1LL << 31) - 1);
        rc |= oracle_sparse_elim(F, "GF(2^31-1)",
                                 gf2_bench_derive_seed(master_seed, "smoke-spelim", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(65521);
        rc |= oracle_sparse_elim(F, "GF(65521)",
                                 gf2_bench_derive_seed(master_seed ^ 0x11ULL, "smoke-spelim", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(251);
        rc |= oracle_sparse_elim(F, "GF(251)",
                                 gf2_bench_derive_seed(master_seed ^ 0x22ULL, "smoke-spelim", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(7);
        rc |= oracle_sparse_elim(F, "GF(7)",
                                 gf2_bench_derive_seed(master_seed ^ 0x33ULL, "smoke-spelim", 0, 0, 0), et);
    }
    {
        using Field = Givaro::Modular<int64_t>;
        Field F(2);
        rc |= oracle_sparse_elim(F, "GF(2)",
                                 gf2_bench_derive_seed(master_seed ^ 0x55ULL, "smoke-spelim", 0, 0, 0), et);
    }

    if (rc != 0) {
        std::fprintf(stderr, "[sparse_smoke] FAIL — %d cell(s) mismatched\n", rc);
        return 1;
    }
    std::fprintf(stderr, "[sparse_smoke] all OK\n");
    return 0;
}
