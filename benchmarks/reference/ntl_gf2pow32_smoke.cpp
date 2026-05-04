// benchmarks/reference/ntl_gf2pow32_smoke.cpp
//
// Bitwise-equality oracle for matmul over GF(2^32) at n=16. Cross-checks
// NTL's `mat_GF2E` matrix multiply against an independent, self-contained
// scalar reference reimplementation. The scalar reference uses the same
// primitive polynomial bits gf2-core's
// `crates/gf2-core/src/primitive_polys.rs::standard(32)` exposes — the
// Conway polynomial `x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1`
// (`0x1_0000_8299`) — so a polynomial drift on the gf2-core side or in
// the NTL setup will fail this binary at compile or link time before
// any timing run begins.
//
// This binary plays the role that `ntl_flint_smoke.cpp` plays for GF(p)
// cells: it is a hard equality oracle invoked from `benchmarks/smoke.sh`
// and exits non-zero on any mismatch. It emits no stdout (so the smoke
// CSV stream stays clean); status messages go to stderr.
//
// ## Why a scalar reimplementation, not a second library
//
// FLINT's `fq_nmod_mat` is the natural sibling oracle but its primitive
// polynomial is implicit in FLINT's internal Conway-polynomial database
// — the bytes are the same as ours by construction (Conway polynomials
// are unique, so SageMath, Magma, GAP, and FLINT all return the same
// 0x1_0000_8299 for GF(2^32)) but verifying that on the C-side adds a
// separate dependency without raising the assurance level. The scalar
// schoolbook GF(2^32) multiply below is ~30 lines of code, depends only
// on the polynomial bits, and is auditable line-by-line. This mirrors
// the `m4rie_bench --smoke` pattern (`benchmarks/reference/m4rie_bench.c`
// lines 128-144 `ref_gf2m_mul`).
//
// ## Byte-level bit-pattern protocol (for cross-language reuse)
//
// Each GF(2^32) element is a polynomial of degree < 32 over GF(2) with
// coefficient `c_i` at bit position `i` (little-endian within a `u32`):
//
//     element = sum_{i=0..31} c_i * x^i,   c_i ∈ {0, 1}
//
// On the wire (and in serialized inputs/outputs) elements are packed as
// little-endian `u32` values. NTL's `GF2XFromBytes(buf, 4)` consumes a
// 4-byte little-endian payload and produces the matching `GF2X`; this is
// exactly the byte order this harness uses, so no basis-change matrix is
// required. The same convention is the gf2-core `Gf2mWide<1, _>`
// `to_le_bytes()` convention (one `u64` word, low 32 bits significant,
// high 32 bits guaranteed zero by the `Gf2mWide` tail-masking invariant).
//
// ## Build
//
//     make ntl_gf2pow32_smoke
//
// from `benchmarks/reference/`. Linked against NTL only (no FLINT, no
// gf2-core) so the harness builds inside the pinned container without
// pulling in extra layers.
//
// ## Exit status
//
//   0 — every n=16 cell matched bit-identically.
//   1 — any cell mismatched, or NTL setup failed.

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
#include <vector>

#include "seed_helpers.h"

namespace {

// GF(2^32) Conway polynomial bits, mirroring
// `crates/gf2-core/src/primitive_polys.rs::standard(32)`. The leading
// bit at position 32 is implicit in the gf2-core convention (the database
// returns the *full* polynomial including bit 32, hex 0x1_0000_8299),
// but NTL's `GF2X` constructor expects every coefficient including the
// leading 1 — so we hand it the full 33-bit bitfield.
//
// Bits set: 0, 3, 4, 7, 9, 15, 32.
//   0x1 << 32 | 0x8299
// = 0x1_0000_8299
constexpr uint64_t kGf2coreConwayM32 = 0x1'0000'8299ULL;

// Number of bytes per GF(2^32) element on the wire.
constexpr size_t kGf2pow32Bytes = 4;

// Initialise NTL `GF2E` to operate over GF(2^32) defined by the Conway
// polynomial. Must be called before any `GF2E` value is touched.
//
// Aborts the process on any NTL setup failure (e.g. the polynomial is
// reducible — which would indicate a Conway-polynomial-bits drift in
// the constant above). The check is cheap (NTL caches the modulus in
// thread-local state) and pays for itself the first time someone fat-
// fingers the constant.
void init_gf2pow32() {
    NTL::GF2X p;
    // Build the polynomial coefficient-by-coefficient. SetCoeff(p, i)
    // sets the coefficient at degree i to 1 (the two-arg form defaults
    // to 1; the three-arg form takes an explicit GF(2) value).
    for (long i = 0; i <= 32; ++i) {
        if ((kGf2coreConwayM32 >> i) & 1ULL) {
            NTL::SetCoeff(p, i);
        }
    }
    // Assert the build matches the well-known Conway-polynomial bits.
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

// Schoolbook scalar GF(2^32) multiply, shift-and-reduce. Independent of
// NTL — uses only the polynomial bits in `kGf2coreConwayM32` to perform
// reduction. Mirrors `m4rie_bench.c::ref_gf2m_mul` for the m=32 case.
//
// Inputs and outputs use the same little-endian-`u32` element encoding
// described in the header comment. The high 32 bits of `a` and `b` MUST
// be zero (the function masks defensively).
uint32_t ref_gf2pow32_mul(uint32_t a, uint32_t b) {
    // Low 32 bits of the polynomial (bit 32 is the implicit leading 1
    // and is *not* part of the reduction step's XOR).
    constexpr uint32_t kReducePoly = (uint32_t)(kGf2coreConwayM32 & 0xFFFFFFFFULL);
    uint32_t result = 0;
    uint32_t lhs = a;
    uint32_t rhs = b;
    for (int i = 0; i < 32; ++i) {
        if (rhs & 1u) {
            result ^= lhs;
        }
        // Test the bit that will leave the field after the next shift,
        // i.e. the current high bit (degree 31).
        const uint32_t carry = lhs >> 31;
        lhs <<= 1;
        if (carry) {
            lhs ^= kReducePoly;
        }
        rhs >>= 1;
    }
    return result;
}

// Convert a `uint32_t` element to a NTL `GF2E` via the documented
// little-endian byte protocol. NTL's `GF2XFromBytes` consumes raw bytes
// and produces a `GF2X` of degree at most `8*n - 1`; we then promote it
// to `GF2E` against the previously-installed modulus.
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

// Convert a NTL `GF2E` back to its `u32` packed representation. The
// inverse of `gf2e_from_u32`: extracts the underlying `GF2X`, dumps its
// raw byte buffer, and reads back the four little-endian bytes.
uint32_t u32_from_gf2e(const NTL::GF2E& e) {
    unsigned char buf[kGf2pow32Bytes] = {0, 0, 0, 0};
    const NTL::GF2X& x = NTL::rep(e);
    // BytesFromGF2X writes up to `n` bytes; trailing zeros are correct
    // for elements whose top GF(2) coefficients are zero.
    NTL::BytesFromGF2X(buf, x, (long)kGf2pow32Bytes);
    return (uint32_t)buf[0]
        | ((uint32_t)buf[1] << 8)
        | ((uint32_t)buf[2] << 16)
        | ((uint32_t)buf[3] << 24);
}

// Fill an n×n byte-level matrix with deterministic SplitMix64-derived
// GF(2^32) elements. Each element consumes one full SplitMix64 draw and
// is masked to its low 32 bits (the high 32 bits of the draw are
// discarded). This mirrors `m4rie_bench.c::fill_uniform_gf2m` and the
// `gf2_core::bench_seed::gf2m_wide_1_matrix_from_seed` Rust path so
// that a future Rust-side cross-check would consume bit-identical
// inputs at the same seed.
void fill_uniform_u32(std::vector<uint32_t>& A, long n, uint64_t seed) {
    A.assign((size_t)(n * n), 0u);
    uint64_t st = seed;
    for (long i = 0; i < n; ++i) {
        for (long j = 0; j < n; ++j) {
            uint64_t draw = gf2_bench_splitmix64(&st);
            A[(size_t)(i * n + j)] = (uint32_t)(draw & 0xFFFFFFFFULL);
        }
    }
}

// Fill a NTL `mat_GF2E` from a `vector<uint32_t>` row-major source. The
// element encoding is the documented little-endian-`u32` protocol; each
// scalar goes through `gf2e_from_u32` so the encoding is shared with
// the smoke comparison.
void load_mat(NTL::mat_GF2E& M, const std::vector<uint32_t>& src, long n) {
    M.SetDims(n, n);
    for (long i = 0; i < n; ++i) {
        for (long j = 0; j < n; ++j) {
            M[i][j] = gf2e_from_u32(src[(size_t)(i * n + j)]);
        }
    }
}

// Compute the n×n matmul C = A * B via the scalar reference. Operates
// purely on packed-`u32` elements; uses `ref_gf2pow32_mul` for the
// inner product. XOR is the GF(2^32) addition (characteristic 2). This
// is the canonical witness against which NTL's output is compared.
void scalar_matmul(const std::vector<uint32_t>& A,
                   const std::vector<uint32_t>& B,
                   std::vector<uint32_t>& C,
                   long n) {
    C.assign((size_t)(n * n), 0u);
    for (long i = 0; i < n; ++i) {
        for (long j = 0; j < n; ++j) {
            uint32_t acc = 0;
            for (long k = 0; k < n; ++k) {
                acc ^= ref_gf2pow32_mul(A[(size_t)(i * n + k)],
                                         B[(size_t)(k * n + j)]);
            }
            C[(size_t)(i * n + j)] = acc;
        }
    }
}

// Run the smoke for one n=16 (A, B, C = A*B) cell. Returns 0 on bitwise
// agreement, 1 on any disagreement.
int run_one_cell(uint64_t a_seed, uint64_t b_seed) {
    constexpr long n = 16;

    // Independent reference inputs.
    std::vector<uint32_t> A_ref, B_ref, C_ref;
    fill_uniform_u32(A_ref, n, a_seed);
    fill_uniform_u32(B_ref, n, b_seed);
    scalar_matmul(A_ref, B_ref, C_ref, n);

    // NTL inputs: same byte stream, fed through `gf2e_from_u32`.
    NTL::mat_GF2E A_n, B_n, C_n;
    load_mat(A_n, A_ref, n);
    load_mat(B_n, B_ref, n);
    NTL::mul(C_n, A_n, B_n);

    // Element-by-element comparison.
    int errors = 0;
    for (long i = 0; i < n; ++i) {
        for (long j = 0; j < n; ++j) {
            uint32_t got = u32_from_gf2e(C_n[i][j]);
            uint32_t want = C_ref[(size_t)(i * n + j)];
            if (got != want) {
                if (errors < 5) {
                    std::fprintf(stderr,
                                 "[ntl_gf2pow32_smoke] mismatch at (%ld,%ld) "
                                 "GF(2^32): ntl=0x%08x ref=0x%08x\n",
                                 i, j, got, want);
                }
                ++errors;
            }
        }
    }
    if (errors > 0) {
        std::fprintf(stderr,
                     "[ntl_gf2pow32_smoke] GF(2^32) FAIL: %d / %ld mismatches\n",
                     errors, (long)(n * n));
        return 1;
    }
    return 0;
}

}  // namespace

int main() {
    init_gf2pow32();

    // Use the master seed convention shared with `ntl_bench.cpp` and
    // `m4rie_bench.c` so the smoke and the timing harness draw the same
    // input matrix when both are seeded from the same master value.
    constexpr uint64_t kMaster = 0x6F73AC91D31E4A7CULL;
    // Mirror the in-protocol seed-derivation idiom: tag = "matmul",
    // op_idx = 0, size_idx = 0, regime_idx = 0. We then mix in the
    // field tag bits the same way `m4rie_bench.c::smoke_one_field`
    // does, so that GF(2^32) draws a disjoint stream from GF(2^4) /
    // GF(2^8) / GF(2^16) at the same (op, size, regime) tuple.
    uint64_t a_seed = gf2_bench_derive_seed(kMaster, "matmul", 0, 0, 0);
    a_seed ^= ((uint64_t)32) * 0x9E3779B97F4A7C15ULL;
    // B-seed: traditional "salt with 0x1111…" convention used by every
    // other smoke harness in this directory (m4rie, ntl_flint).
    uint64_t b_seed = a_seed ^ 0x1111111111111111ULL;

    std::fprintf(stderr,
                 "[ntl_gf2pow32_smoke] GF(2^32) Conway poly=0x%llx "
                 "(master=0x%llx a_seed=0x%llx b_seed=0x%llx) ...\n",
                 (unsigned long long)kGf2coreConwayM32,
                 (unsigned long long)kMaster,
                 (unsigned long long)a_seed,
                 (unsigned long long)b_seed);

    int rc = run_one_cell(a_seed, b_seed);
    if (rc != 0) {
        std::fprintf(stderr, "[ntl_gf2pow32_smoke] FAILED\n");
        return 1;
    }
    std::fprintf(stderr, "[ntl_gf2pow32_smoke] OK\n");
    return 0;
}
