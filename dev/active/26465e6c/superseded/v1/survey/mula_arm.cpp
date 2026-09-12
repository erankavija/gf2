// The vendored `popcnt_AVX2_harley_seal` (vendor/mula/popcnt-avx2-harley-seal.cpp,
// upstream WojciechMula/sse-popcount at 138c91e21c3e6dab7875521b5d33b995e0e4c85e)
// takes its input through `AVX2_harley_seal::popcnt(const __m256i *data, ...)`,
// which the compiler lowers to ALIGNED 256-bit loads (`data[i]` on a `__m256i*`).
// Unlike gf2's own kernels and libpopcnt (both of which load through `loadu`/
// `_mm256_loadu_si256` and tolerate any word alignment), this reference
// implementation requires its input pointer to be 32-byte aligned; reading
// through it on an unaligned pointer is undefined behavior and was observed to
// SIGSEGV on this host/compiler. This file is a survey finding, not a defect in
// the vendored upstream file (which this survey does not modify): only
// `word_offset == 0` cases route to it, and other offsets fail closed here
// rather than crash, so the addendum's "external" arm never receives a case
// its own precondition does not support. See findings.md.
#include "wire_common.h"

uint64_t popcnt_AVX2_harley_seal(const uint8_t *data, size_t length);

// True when a buffer starting at `word_offset` words into a fresh allocation
// is 32-byte aligned. `survey_make_words` allocates the base array 32-byte
// aligned (`posix_memalign`), so word_offset 0 is always AVX2-vector aligned
// and a 1-, 2-, or 3-word (8/16/24-byte) shift deterministically breaks that
// alignment by construction.
static int mula_offset_supported(uint64_t word_offset) { return word_offset == 0; }

typedef struct {
    uint64_t *banks[SURVEY_BANKS];
    uint64_t words;
    uint64_t offset;
} popcount_context_t;

static uint64_t mula_call(void *opaque, size_t bank) {
    popcount_context_t *context = static_cast<popcount_context_t *>(opaque);
    return popcnt_AVX2_harley_seal(reinterpret_cast<const uint8_t *>(context->banks[bank] + context->offset), context->words * sizeof(uint64_t));
}

static int parse_u64_arg(const char *text, uint64_t *value) {
    char *end = NULL;
    errno = 0;
    *value = strtoull(text, &end, 10);
    return errno == 0 && end != text && *end == '\0';
}

static int make_context(popcount_context_t *context, uint64_t words, uint64_t seed, const char *pattern, uint64_t offset, size_t banks) {
    memset(context, 0, sizeof(*context)); context->words = words; context->offset = offset;
    for (size_t bank = 0; bank < banks; ++bank) {
        context->banks[bank] = survey_make_words(words, seed, pattern, offset);
        if (!context->banks[bank]) return 0;
    }
    return 1;
}

static void free_context(popcount_context_t *context) {
    for (size_t bank = 0; bank < SURVEY_BANKS; ++bank) free(context->banks[bank]);
}

static int run_transport(void) {
    const char *sentinel = getenv("GF2_TUNING_FRESH_CASE");
    if (!sentinel || strcmp(sentinel, "child-v2") != 0) { fprintf(stderr, "mula-arm: invalid fresh-child sentinel\n"); return 2; }
    char *input = NULL; size_t input_length = 0; survey_request_t request;
    if (!survey_read_stdin(&input, &input_length) || !survey_parse_request(input, input_length, &request)) {
        free(input); fprintf(stderr, "mula-arm: invalid child-v2 request\n"); return 2;
    }
    free(input);
    if (strcmp(request.case_op, "popcount") != 0) { fprintf(stderr, "mula-arm: only popcount cases are supported\n"); return 2; }
    if (!mula_offset_supported(request.word_offset)) {
        fprintf(stderr, "mula-arm: word_offset %" PRIu64 " is unaligned for this AVX2 Harley-Seal reference (requires 0)\n", request.word_offset);
        return 2;
    }
    popcount_context_t context;
    int streaming = strcmp(request.cache_state, "streaming") == 0;
    if (!make_context(&context, request.words, request.seed, request.pattern, request.word_offset, survey_bank_count(request.cache_state))) {
        fprintf(stderr, "mula-arm: cannot allocate case buffers\n"); return 1;
    }
    if (strcmp(request.cache_state, "warm") == 0) survey_sink ^= mula_call(&context, 0);
    survey_sample_t samples[SURVEY_MAX_WINDOWS];
    int ok = survey_time(request.windows, request.window_target_ms, streaming, mula_call, &context, samples);
    int result_ok = ok && survey_write_result(&request, "mula-avx2-harley-seal-csa", samples);
    free_context(&context);
    return result_ok ? 0 : 1;
}

static int run_check(int argc, char **argv) {
    uint64_t words, seed, offset;
    if (argc != 7 || strcmp(argv[2], "popcount") != 0 || !parse_u64_arg(argv[3], &words) ||
        !parse_u64_arg(argv[4], &seed) || !parse_u64_arg(argv[6], &offset) || offset > 3) return 2;
    if (!mula_offset_supported(offset)) {
        fprintf(stderr, "mula-arm: word_offset %" PRIu64 " is unaligned for this AVX2 Harley-Seal reference (requires 0)\n", offset);
        return 2;
    }
    popcount_context_t context;
    if (!make_context(&context, words, seed, argv[5], offset, 1)) return 1;
    printf("%" PRIu64 "\n", mula_call(&context, 0));
    free_context(&context);
    return 0;
}

static int run_selftest(void) {
    static const uint64_t sizes[] = {0, 1, 2, 7, 8, 9, 63, 64, 65, 256, 4096};
    static const uint64_t seeds[] = {0, 1, UINT64_C(0x0123456789abcdef)};
    static const char *patterns[] = {"random", "all_zero", "all_one"};
    size_t cases = 0;
    // Only word_offset 0 is exercised against the real kernel: see the file
    // header comment on the vendored implementation's alignment precondition.
    for (size_t si = 0; si < sizeof(sizes) / sizeof(sizes[0]); ++si)
        for (size_t pi = 0; pi < 3; ++pi)
            for (size_t seed_i = 0; seed_i < (pi == 0 ? 3 : 1); ++seed_i) {
                uint64_t offset = 0;
                uint64_t *storage = survey_make_words(sizes[si], seeds[seed_i], patterns[pi], offset);
                if (!storage) { fprintf(stderr, "FAIL at allocation\n"); return 1; }
                uint64_t expected = 0;
                for (uint64_t i = 0; i < sizes[si]; ++i) expected += __builtin_popcountll(storage[offset + i]);
                uint64_t actual = popcnt_AVX2_harley_seal(reinterpret_cast<const uint8_t *>(storage + offset), sizes[si] * sizeof(uint64_t));
                free(storage);
                ++cases;
                if (actual != expected) { fprintf(stderr, "FAIL at case %zu\n", cases); return 1; }
            }
    // Negative cases: the transport and --check entry points must fail closed
    // (not crash) on the unaligned offsets this reference cannot accept.
    for (uint64_t offset = 1; offset <= 3; ++offset) {
        char words[] = "64", seed[] = "1", pattern[] = "random";
        char offset_text[4];
        snprintf(offset_text, sizeof(offset_text), "%" PRIu64, offset);
        char *argv[] = {(char *)"mula-avx2-harleyseal-arm", (char *)"--check", (char *)"popcount", words, seed, pattern, offset_text};
        int code = run_check(7, argv);
        ++cases;
        if (code != 2) { fprintf(stderr, "FAIL at case %zu: unaligned offset %" PRIu64 " did not fail closed (exit %d)\n", cases, offset, code); return 1; }
    }
    printf("PASS %zu cases\n", cases);
    return 0;
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--alignment") == 0) return survey_report_alignment();
    if (argc > 1 && strcmp(argv[1], "--selftest") == 0) return run_selftest();
    if (argc > 1 && strcmp(argv[1], "--check") == 0) return run_check(argc, argv);
    return run_transport();
}
