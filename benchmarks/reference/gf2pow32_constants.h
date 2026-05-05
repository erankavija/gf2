/*
 * benchmarks/reference/gf2pow32_constants.h
 *
 * Single C++ source of truth for the GF(2^32) Conway polynomial used by
 * every C++ harness that touches GF(2^32) (currently `ntl_bench.cpp`
 * and `ntl_gf2pow32_smoke.cpp`; future m4rie/flint/linbox m=32 lanes
 * when those land).
 *
 * The Rust source of truth is `crates/gf2-core/src/primitive_polys.rs`
 * `PrimitivePolynomialDatabase::standard(32)`. The Rust integration test
 * `crates/gf2-core/tests/gf2pow32_constant_drift.rs` parses this header
 * at test time and asserts the constant below matches the Rust SSOT — so a
 * drift in either direction fails CI.
 *
 * Element encoding: each GF(2^32) element is a polynomial of degree < 32
 * over GF(2) with coefficient `c_i` at bit position `i` (little-endian
 * within a `uint32_t`). On the wire (and in this harness's serialized
 * inputs/outputs) elements are packed as little-endian `uint32_t` values.
 * NTL's `GF2XFromBytes(buf, 4)` consumes a 4-byte little-endian payload
 * and produces the matching `GF2X`; this is the byte order this and every
 * peer harness uses, so no basis-change matrix is required against gf2-core's
 * `Gf2mWide<1, _>` `to_le_bytes()` convention.
 *
 * Note: the prior R1 revision of this header also defined
 * `ref_gf2pow32_mul(a, b)` — a self-contained scalar GF(2^32) multiply
 * used by `ntl_gf2pow32_smoke.cpp`'s transitive (NTL ↔ scalar) oracle.
 * The R2 rewrite of the smoke replaced the transitive form with a direct
 * (NTL ↔ gf2-core ground-truth) oracle (jit:b13799ac criterion 3), so the
 * scalar multiply is no longer needed in C++ and was removed to keep the
 * header to a single SSOT (the Conway polynomial bits) — there is now
 * exactly one reason for this header to exist. The Rust scalar reference
 * in `crates/gf2-core/tests/gf2pow32_matmul.rs::ref_gf2pow32_mul` is
 * retained as a Rust-internal gf2-core ↔ scalar witness independent of
 * NTL.
 */
#ifndef GF2_BENCH_GF2POW32_CONSTANTS_H
#define GF2_BENCH_GF2POW32_CONSTANTS_H

#include <cstdint>

namespace gf2_bench {

// GF(2^32) Conway polynomial bits, mirroring
// `crates/gf2-core/src/primitive_polys.rs::standard(32)`. The leading bit
// at position 32 is the implicit irreducible leading term; gf2-core's
// `PrimitivePolynomialDatabase::standard(32)` returns the *full*
// polynomial including bit 32, hex 0x1_0000_8299. NTL's `GF2X`
// constructor expects every coefficient including the leading 1, so we
// keep the full 33-bit bitfield here.
//
// Bits set: 0, 3, 4, 7, 9, 15, 32  →  x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1.
//   (1ULL << 32) | 0x8299  =  0x1'0000'8299
//
// Citation: Frank Lübeck's Conway polynomial database, table row
// `f_{2,32}`. https://www.math.rwth-aachen.de/~Frank.Luebeck/data/ConwayPol/CP2.html
constexpr uint64_t kGf2coreConwayM32 = 0x1'0000'8299ULL;

// Bytes per packed GF(2^32) element on the wire / in serialised matrices.
constexpr std::size_t kGf2pow32Bytes = 4;

}  // namespace gf2_bench

#endif  // GF2_BENCH_GF2POW32_CONSTANTS_H
