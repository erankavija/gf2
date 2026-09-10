/*
 * Survey entry points into the vendored libpopcnt v4.2 (jit:26465e6c).
 *
 * `popcnt()` is a static function of the header, so this translation unit is
 * the library build: compiled with the README's plain `-O3`, it keeps
 * libpopcnt's CPUID runtime dispatch rather than a compile-time ISA choice.
 */
#include <stddef.h>
#include <stdint.h>

#include "libpopcnt.h"

#if !defined(LIBPOPCNT_HAVE_CPUID)
#error "the survey measures libpopcnt's runtime-dispatch build; compile without ISA flags"
#endif

uint64_t survey_libpopcnt_words(const uint64_t *words, size_t count)
{
  return popcnt(words, (uint64_t)count * sizeof(uint64_t));
}

uint64_t survey_libpopcnt_bytes(const void *data, size_t bytes)
{
  return popcnt(data, (uint64_t)bytes);
}

/*
 * The capability word libpopcnt's own dispatch computes, followed by the three
 * bit masks `popcnt()` tests it against.
 */
void survey_libpopcnt_capabilities(int32_t out[4])
{
  out[0] = get_cpuid();
  out[1] = LIBPOPCNT_BIT_POPCNT;
  out[2] = LIBPOPCNT_BIT_AVX2;
  out[3] = LIBPOPCNT_BIT_AVX512_VPOPCNTDQ;
}
