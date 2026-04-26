/*
 * benchmarks/reference/seed_helpers.h
 *
 * Single source of truth for the deterministic seed-derivation scheme
 * shared between fflas_bench.cpp and m4ri_bench.c. Both harnesses must
 * draw row seeds from this header so that:
 *
 *   1. The CSV `seed` column has the same meaning across harnesses.
 *   2. The gf2-side criterion benches (which re-implement SplitMix64 in
 *      Rust against the same master seed) line up byte-for-byte.
 *
 * The header is C / C++ dual-mode: the C harness includes it directly,
 * the C++ harness includes it from inside an `extern "C"`-clean scope.
 * No platform headers are pulled in; only <stdint.h>.
 *
 * Algorithm: the standard SplitMix64 with constants from Sebastiano
 * Vigna's xoroshiro reference. Derive() mixes a tag string plus three
 * integer indices (op_idx, size_idx, regime_idx) into the master seed.
 */
#ifndef GF2_BENCH_SEED_HELPERS_H
#define GF2_BENCH_SEED_HELPERS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* SplitMix64 — small, well-mixed deterministic splitter. The state is
 * advanced by reference; the return value is the next 64 bits of stream. */
static inline uint64_t gf2_bench_splitmix64(uint64_t* state) {
    *state += (uint64_t)0x9E3779B97F4A7C15ULL;
    uint64_t z = *state;
    z = (z ^ (z >> 30)) * (uint64_t)0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * (uint64_t)0x94D049BB133111EBULL;
    return z ^ (z >> 31);
}

/* Derive a 64-bit row seed for a (tag, op_idx, size_idx, regime_idx) cell
 * from the run's master seed. The tag is mixed byte-by-byte so two
 * benchmarks that use different tag strings (e.g. "fgemm" vs "pluq")
 * derive disjoint seed streams even at identical (op_idx, size_idx,
 * regime_idx) tuples. */
static inline uint64_t gf2_bench_derive_seed(uint64_t master,
                                             const char* tag,
                                             uint64_t op_idx,
                                             uint64_t size_idx,
                                             uint64_t regime_idx) {
    uint64_t s = master;
    for (const char* p = tag; *p != '\0'; ++p) {
        s ^= (uint64_t)(unsigned char)*p;
        (void)gf2_bench_splitmix64(&s);
    }
    s ^= op_idx;       (void)gf2_bench_splitmix64(&s);
    s ^= size_idx;     (void)gf2_bench_splitmix64(&s);
    s ^= regime_idx;   (void)gf2_bench_splitmix64(&s);
    return gf2_bench_splitmix64(&s);
}

#ifdef __cplusplus
}  /* extern "C" */
#endif

#endif  /* GF2_BENCH_SEED_HELPERS_H */
