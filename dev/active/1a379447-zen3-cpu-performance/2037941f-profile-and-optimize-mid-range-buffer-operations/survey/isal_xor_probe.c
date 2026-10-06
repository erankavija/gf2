/*
 * Untimed semantic probe for ISA-L xor_gen_base.
 *
 * The probe is deliberately linked directly with raid/raid_base.c.  It does
 * not claim to exercise ISA-L's NASM-dispatched public xor_gen route.
 */
#define _POSIX_C_SOURCE 200112L

#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "raid.h"

struct probe_case {
    size_t words;
    size_t sources;
};

static uint64_t next_word(uint64_t *state)
{
    *state += UINT64_C(0x9e3779b97f4a7c15);
    uint64_t value = *state;
    value = (value ^ (value >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)) * UINT64_C(0x94d049bb133111eb);
    return value ^ (value >> 31);
}

static uint64_t *aligned_words(size_t words)
{
    void *pointer = NULL;
    size_t bytes = words * sizeof(uint64_t);
    if (posix_memalign(&pointer, 32, bytes) != 0)
        return NULL;
    return pointer;
}

static int run_case(struct probe_case probe, uint64_t seed)
{
    uint64_t **sources = calloc(probe.sources, sizeof(*sources));
    void **vectors = calloc(probe.sources + 1, sizeof(*vectors));
    uint64_t *output = aligned_words(probe.words);
    uint64_t *expected = calloc(probe.words, sizeof(*expected));
    int result = 1;

    if (sources == NULL || vectors == NULL || output == NULL || expected == NULL)
        goto out;
    for (size_t source = 0; source < probe.sources; ++source) {
        sources[source] = aligned_words(probe.words);
        if (sources[source] == NULL)
            goto out;
        if (((uintptr_t)sources[source] & 31U) != 0) {
            fputs("source is not 32-byte aligned\n", stderr);
            goto out;
        }
        vectors[source] = sources[source];
    }
    if (((uintptr_t)output & 31U) != 0) {
        fputs("destination is not 32-byte aligned\n", stderr);
        goto out;
    }
    vectors[probe.sources] = output;

    for (size_t word = 0; word < probe.words; ++word) {
        uint64_t parity = 0;
        for (size_t source = 0; source < probe.sources; ++source) {
            uint64_t state = seed + source * UINT64_C(0x100000001b3) + word;
            sources[source][word] = next_word(&state);
            parity ^= sources[source][word];
        }
        expected[word] = parity;
        output[word] = UINT64_C(0xa5a5a5a5a5a5a5a5);
    }

    if (xor_gen_base((int)(probe.sources + 1),
                     (int)(probe.words * sizeof(uint64_t)), vectors) != 0) {
        fputs("xor_gen_base rejected a valid vector array\n", stderr);
        goto out;
    }
    for (size_t word = 0; word < probe.words; ++word) {
        if (output[word] != expected[word]) {
            fprintf(stderr, "word mismatch: words=%zu sources=%zu word=%zu\n",
                    probe.words, probe.sources, word);
            goto out;
        }
        for (unsigned bit = 0; bit < 64; ++bit) {
            unsigned got = (unsigned)((output[word] >> bit) & 1U);
            unsigned want = (unsigned)((expected[word] >> bit) & 1U);
            if (got != want) {
                fprintf(stderr,
                        "LSB-first bit mismatch: words=%zu sources=%zu word=%zu bit=%u\n",
                        probe.words, probe.sources, word, bit);
                goto out;
            }
        }
    }
    for (size_t source = 0; source < probe.sources; ++source) {
        for (size_t word = 0; word < probe.words; ++word) {
            uint64_t state = seed + source * UINT64_C(0x100000001b3) + word;
            if (sources[source][word] != next_word(&state)) {
                fprintf(stderr,
                        "source mutation: words=%zu sources=%zu source=%zu word=%zu\n",
                        probe.words, probe.sources, source, word);
                goto out;
            }
        }
    }
    result = 0;
out:
    if (sources != NULL) {
        for (size_t source = 0; source < probe.sources; ++source)
            free(sources[source]);
    }
    free(expected);
    free(output);
    free(vectors);
    free(sources);
    return result;
}

int main(void)
{
    static const size_t words[] = {1, 7, 8, 9, 63, 64, 65};
    static const size_t source_counts[] = {2, 3};
    unsigned cases = 0;

    if (sizeof(uint64_t) != 8) {
        fputs("the probe requires 64-bit words\n", stderr);
        return 1;
    }
    for (size_t word = 0; word < sizeof(words) / sizeof(words[0]); ++word) {
        for (size_t sources = 0;
             sources < sizeof(source_counts) / sizeof(source_counts[0]);
             ++sources) {
            struct probe_case probe = {words[word], source_counts[sources]};
            if (run_case(probe, UINT64_C(0x6a09e667f3bcc909) + cases) != 0)
                return 1;
            ++cases;
        }
    }
    printf("ISA-L xor_gen_base semantic probe: %u cases passed (LSB-first)\n", cases);
    return 0;
}
