// benchmarks/reference/ntl_gf2pow32_smoke.cpp
//
// Direct gf2-core ↔ NTL bitwise-equality oracle for matmul over
// GF(2^32) at n=16 (jit:b13799ac criterion 3, R2 rewrite). Loads the
// gf2-core ground-truth file emitted by the Cargo example
// `gf2pow32_smoke_emit_expected` (`crates/gf2-coding/examples/gf2pow32_smoke_emit_expected.rs`)
// and asserts byte-equality between NTL `mat_GF2E::mul` output and the
// gf2-core `FieldMatrix<Gf2mWide<1, _>>::gemm` output.
//
// Replaces the prior R1 implementation that used a transitive smoke
// (NTL ↔ scalar reference + gf2-core ↔ scalar reference in
// `crates/gf2-core/tests/gf2pow32_matmul.rs`). The protocol § 6
// criterion-3 contract names the reference and gf2-core directly; the
// transitive form did not satisfy the literal text and burned a review
// cycle. The Rust-side gf2-core ↔ scalar test (`gf2pow32_matmul.rs`)
// is retained as an additional Rust-internal witness — it is not
// load-bearing for this oracle.
//
// File format (matches `gf2pow32_smoke_emit_expected.rs`, little-endian):
//
//   magic   : 8 bytes ASCII "GF2P32M0"
//   n       : u32                  (matrix dimension; expected 16)
//   a_seed  : u64                  (informational; not used to derive A here)
//   b_seed  : u64                  (informational)
//   conway  : u64                  (full Conway polynomial bits incl. bit 32)
//   a_bytes : 4 * n * n bytes      (row-major u32 LE for A)
//   b_bytes : 4 * n * n bytes      (row-major u32 LE for B)
//   c_bytes : 4 * n * n bytes      (row-major u32 LE for C = A * B from gf2-core)
//
// Element encoding: each GF(2^32) element is a polynomial of degree < 32
// over GF(2) with coefficient `c_i` at bit position `i` (little-endian
// within a `u32`). NTL's `GF2XFromBytes(buf, 4)` consumes a 4-byte
// little-endian payload and produces the matching `GF2X`; this is
// exactly the byte order the emitter uses, so no basis-change is required.
//
// Build:
//
//     make ntl_gf2pow32_smoke
//
// from `benchmarks/reference/`. Linked against NTL only (no FLINT, no
// gf2-core) so the harness builds inside the pinned container without
// pulling in extra layers. The gf2-core side runs as a Cargo example
// outside the container before this binary is invoked.
//
// Usage:
//
//     ntl_gf2pow32_smoke --expected <path-to-gf2pow32_smoke_n16.bin>
//
// Default path is `benchmarks/expected/gf2pow32_smoke_n16.bin` (relative
// to PWD); override via `--expected` or the `GF2_GF2POW32_SMOKE_EXPECTED`
// environment variable.
//
// Exit status:
//
//   0 — every n=16 cell matched bit-identically, polynomial bits agree
//       with the embedded `kGf2coreConwayM32` constant, and the file
//       parsed cleanly.
//   1 — any cell mismatched, polynomial drift, or the file failed to
//       parse / open.

#include <NTL/GF2E.h>
#include <NTL/GF2X.h>
#include <NTL/GF2XFactoring.h>
#include <NTL/mat_GF2E.h>
#include <NTL/vec_GF2.h>
#include <NTL/ZZ.h>

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <string>
#include <vector>

#include "gf2pow32_constants.h"

namespace {

using gf2_bench::kGf2coreConwayM32;
using gf2_bench::kGf2pow32Bytes;

// Initialise NTL `GF2E` to operate over GF(2^32) defined by the Conway
// polynomial. Aborts the process on any NTL setup failure (e.g. the
// polynomial is reducible — which would indicate Conway-polynomial-bits
// drift in the constant above).
void init_gf2pow32() {
    NTL::GF2X p;
    for (long i = 0; i <= 32; ++i) {
        if ((kGf2coreConwayM32 >> i) & 1ULL) {
            NTL::SetCoeff(p, i);
        }
    }
    if (NTL::deg(p) != 32) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] FATAL: built GF2X has degree %ld, "
                     "expected 32 (constant=0x%llx)\n",
                     (long)NTL::deg(p),
                     (unsigned long long)kGf2coreConwayM32);
        std::exit(1);
    }
    if (!NTL::IterIrredTest(p)) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] FATAL: GF2X polynomial 0x%llx is "
                     "not irreducible — Conway-polynomial constant has "
                     "drifted from primitive_polys.rs::standard(32)\n",
                     (unsigned long long)kGf2coreConwayM32);
        std::exit(1);
    }
    NTL::GF2E::init(p);
}

NTL::GF2E gf2e_from_u32(uint32_t v) {
    unsigned char buf[kGf2pow32Bytes];
    buf[0] = (unsigned char)(v & 0xFFu);
    buf[1] = (unsigned char)((v >> 8) & 0xFFu);
    buf[2] = (unsigned char)((v >> 16) & 0xFFu);
    buf[3] = (unsigned char)((v >> 24) & 0xFFu);
    NTL::GF2X x;
    NTL::GF2XFromBytes(x, buf, (long)kGf2pow32Bytes);
    return NTL::to_GF2E(x);
}

uint32_t u32_from_gf2e(const NTL::GF2E& e) {
    unsigned char buf[kGf2pow32Bytes] = {0, 0, 0, 0};
    const NTL::GF2X& x = NTL::rep(e);
    NTL::BytesFromGF2X(buf, x, (long)kGf2pow32Bytes);
    return (uint32_t)buf[0]
        | ((uint32_t)buf[1] << 8)
        | ((uint32_t)buf[2] << 16)
        | ((uint32_t)buf[3] << 24);
}

void load_mat(NTL::mat_GF2E& M, const std::vector<uint32_t>& src, long n) {
    M.SetDims(n, n);
    for (long i = 0; i < n; ++i) {
        for (long j = 0; j < n; ++j) {
            M[i][j] = gf2e_from_u32(src[(size_t)(i * n + j)]);
        }
    }
}

// Read exactly `n` bytes from the stream into `dst`. Aborts on short read.
bool read_exact(std::ifstream& f, void* dst, size_t n) {
    f.read(reinterpret_cast<char*>(dst), (std::streamsize)n);
    return (size_t)f.gcount() == n;
}

bool read_u32_le(std::ifstream& f, uint32_t& out) {
    unsigned char b[4];
    if (!read_exact(f, b, 4)) return false;
    out = (uint32_t)b[0] | ((uint32_t)b[1] << 8) | ((uint32_t)b[2] << 16) | ((uint32_t)b[3] << 24);
    return true;
}

bool read_u64_le(std::ifstream& f, uint64_t& out) {
    unsigned char b[8];
    if (!read_exact(f, b, 8)) return false;
    out = 0;
    for (int i = 0; i < 8; ++i) out |= (uint64_t)b[i] << (8 * i);
    return true;
}

bool read_u32_block(std::ifstream& f, std::vector<uint32_t>& out, size_t count) {
    out.resize(count);
    for (size_t i = 0; i < count; ++i) {
        if (!read_u32_le(f, out[i])) return false;
    }
    return true;
}

}  // namespace

int main(int argc, char** argv) {
    init_gf2pow32();

    // Default file location (relative to PWD); overridable via
    // `--expected` or the `GF2_GF2POW32_SMOKE_EXPECTED` env var so
    // run.sh / smoke.sh can invoke the binary inside the container with
    // a `/work/expected/...` path.
    std::string expected_path = "benchmarks/expected/gf2pow32_smoke_n16.bin";
    if (const char* env = std::getenv("GF2_GF2POW32_SMOKE_EXPECTED")) {
        expected_path = env;
    }
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--expected") == 0 && i + 1 < argc) {
            expected_path = argv[++i];
        } else if (std::strcmp(argv[i], "--help") == 0 || std::strcmp(argv[i], "-h") == 0) {
            std::fprintf(stderr,
                         "usage: ntl_gf2pow32_smoke [--expected <path>]\n"
                         "  default path: benchmarks/expected/gf2pow32_smoke_n16.bin\n"
                         "  env var GF2_GF2POW32_SMOKE_EXPECTED is also honoured.\n");
            return 0;
        }
    }

    std::ifstream f(expected_path, std::ios::binary);
    if (!f) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] FATAL: cannot open expected file %s\n",
                     expected_path.c_str());
        return 1;
    }

    char magic[8];
    if (!read_exact(f, magic, 8) || std::memcmp(magic, "GF2P32M0", 8) != 0) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] FATAL: bad magic in %s (expected GF2P32M0)\n",
                     expected_path.c_str());
        return 1;
    }

    uint32_t n = 0;
    if (!read_u32_le(f, n) || n != 16) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] FATAL: ground-truth file declares "
                     "n=%u, expected 16\n", n);
        return 1;
    }

    uint64_t a_seed = 0, b_seed = 0, conway_in_file = 0;
    if (!read_u64_le(f, a_seed) || !read_u64_le(f, b_seed)
            || !read_u64_le(f, conway_in_file)) {
        std::fprintf(stderr, "[ntl_gf2pow32_smoke] FATAL: short header\n");
        return 1;
    }
    if (conway_in_file != kGf2coreConwayM32) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] FATAL: Conway polynomial drift "
                     "(file=0x%llx, header=0x%llx) — primitive_polys.rs "
                     "and gf2pow32_constants.h disagree on m=32 SSOT\n",
                     (unsigned long long)conway_in_file,
                     (unsigned long long)kGf2coreConwayM32);
        return 1;
    }

    std::vector<uint32_t> A_bytes, B_bytes, C_expected;
    if (!read_u32_block(f, A_bytes, (size_t)n * n)
            || !read_u32_block(f, B_bytes, (size_t)n * n)
            || !read_u32_block(f, C_expected, (size_t)n * n)) {
        std::fprintf(stderr, "[ntl_gf2pow32_smoke] FATAL: short matrix block\n");
        return 1;
    }

    std::fprintf(stderr,
                 "[ntl_gf2pow32_smoke] GF(2^32) Conway poly=0x%llx "
                 "(n=%u, a_seed=0x%llx, b_seed=0x%llx) ...\n",
                 (unsigned long long)kGf2coreConwayM32,
                 n,
                 (unsigned long long)a_seed,
                 (unsigned long long)b_seed);

    // NTL inputs: same byte stream the gf2-core emitter wrote.
    NTL::mat_GF2E A_n, B_n, C_n;
    load_mat(A_n, A_bytes, (long)n);
    load_mat(B_n, B_bytes, (long)n);
    NTL::mul(C_n, A_n, B_n);

    // Direct comparison: NTL output bytes vs gf2-core ground-truth bytes.
    int errors = 0;
    for (long i = 0; i < (long)n; ++i) {
        for (long j = 0; j < (long)n; ++j) {
            uint32_t got = u32_from_gf2e(C_n[i][j]);
            uint32_t want = C_expected[(size_t)(i * n + j)];
            if (got != want) {
                if (errors < 5) {
                    std::fprintf(stderr,
                                 "[ntl_gf2pow32_smoke] mismatch at (%ld,%ld) "
                                 "GF(2^32): ntl=0x%08x gf2-core=0x%08x\n",
                                 i, j, got, want);
                }
                ++errors;
            }
        }
    }
    if (errors > 0) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] GF(2^32) FAIL: %d / %ld mismatches\n",
                     errors, (long)((long)n * n));
        return 1;
    }
    std::fprintf(stderr, "[ntl_gf2pow32_smoke] OK gf2-core ↔ NTL\n");
    return 0;
}
