// Survey entry points into the vendored Mula AVX2 Harley-Seal reference
// (jit:26465e6c).
//
// Upstream compiles its implementations as one translation unit: speed.cpp
// includes config.h and then each implementation file. This unit keeps that
// model for the two files `popcnt_AVX2_harley_seal` needs and changes none of
// their bytes.
//
// `AVX2_harley_seal::popcnt` dereferences `const __m256i*`, so the input must
// be 32-byte aligned; the Rust arm resolves this kernel only for aligned
// fixtures.
#include <cstddef>
#include <cstdint>

#include "config.h"
#include "popcnt-lookup.cpp"
#include "popcnt-avx2-harley-seal.cpp"

extern "C" std::uint64_t survey_mula_avx2_harley_seal_words(const std::uint64_t *words,
                                                            std::size_t count)
{
    return popcnt_AVX2_harley_seal(reinterpret_cast<const std::uint8_t *>(words),
                                   count * sizeof(std::uint64_t));
}

extern "C" std::uint64_t survey_mula_avx2_harley_seal_bytes(const std::uint8_t *data,
                                                            std::size_t bytes)
{
    return popcnt_AVX2_harley_seal(data, bytes);
}
