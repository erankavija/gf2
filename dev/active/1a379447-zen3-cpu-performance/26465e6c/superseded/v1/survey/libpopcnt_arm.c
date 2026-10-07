#define _GNU_SOURCE
#include "wire_common.h"
#include "libpopcnt.h"

typedef struct {
    uint64_t *banks[SURVEY_BANKS];
    uint64_t words;
    uint64_t offset;
} popcount_context_t;

static uint64_t libpopcnt_call(void *opaque, size_t bank) {
    popcount_context_t *context = (popcount_context_t *)opaque;
    return (uint64_t)popcnt(context->banks[bank] + context->offset, context->words * sizeof(uint64_t));
}

static int parse_u64_arg(const char *text, uint64_t *value) {
    char *end = NULL;
    errno = 0;
    *value = strtoull(text, &end, 10);
    return errno == 0 && end != text && *end == '\0';
}

static int make_popcount_context(popcount_context_t *context, uint64_t words, uint64_t seed, const char *pattern, uint64_t offset, size_t banks) {
    memset(context, 0, sizeof(*context));
    context->words = words; context->offset = offset;
    for (size_t bank = 0; bank < banks; ++bank) {
        context->banks[bank] = survey_make_words(words, seed, pattern, offset);
        if (!context->banks[bank]) return 0;
    }
    return 1;
}

static void free_popcount_context(popcount_context_t *context) {
    for (size_t bank = 0; bank < SURVEY_BANKS; ++bank) free(context->banks[bank]);
}

static int run_transport(void) {
    const char *sentinel = getenv("GF2_TUNING_FRESH_CASE");
    if (!sentinel || strcmp(sentinel, "child-v2") != 0) { fprintf(stderr, "libpopcnt-arm: invalid fresh-child sentinel\n"); return 2; }
    char *input = NULL; size_t input_length = 0;
    survey_request_t request;
    if (!survey_read_stdin(&input, &input_length) || !survey_parse_request(input, input_length, &request)) {
        free(input); fprintf(stderr, "libpopcnt-arm: invalid child-v2 request\n"); return 2;
    }
    free(input);
    if (strcmp(request.case_op, "popcount") != 0) { fprintf(stderr, "libpopcnt-arm: only popcount cases are supported\n"); return 2; }
    popcount_context_t context;
    int streaming = strcmp(request.cache_state, "streaming") == 0;
    if (!make_popcount_context(&context, request.words, request.seed, request.pattern, request.word_offset, survey_bank_count(request.cache_state))) {
        fprintf(stderr, "libpopcnt-arm: cannot allocate case buffers\n"); return 1;
    }
    if (strcmp(request.cache_state, "warm") == 0) survey_sink ^= libpopcnt_call(&context, 0);
    survey_sample_t samples[SURVEY_MAX_WINDOWS];
    int ok = survey_time(request.windows, request.window_target_ms, streaming, libpopcnt_call, &context, samples);
    survey_sink ^= 0;
    int result_ok = ok && survey_write_result(&request, "libpopcnt-runtime-dispatch", samples);
    free_popcount_context(&context);
    return result_ok ? 0 : 1;
}

static int run_check(int argc, char **argv) {
    uint64_t words, seed, offset;
    if (argc != 7 || strcmp(argv[2], "popcount") != 0 || !parse_u64_arg(argv[3], &words) ||
        !parse_u64_arg(argv[4], &seed) || !parse_u64_arg(argv[6], &offset) || offset > 3) return 2;
    popcount_context_t context;
    if (!make_popcount_context(&context, words, seed, argv[5], offset, 1)) return 1;
    printf("%" PRIu64 "\n", libpopcnt_call(&context, 0));
    free_popcount_context(&context);
    return 0;
}

static int run_selftest(void) {
    static const uint64_t sizes[] = {0, 1, 2, 7, 8, 9, 63, 64, 65, 256, 4096};
    static const uint64_t seeds[] = {0, 1, UINT64_C(0x0123456789abcdef)};
    static const char *patterns[] = {"random", "all_zero", "all_one"};
    size_t cases = 0;
    for (size_t si = 0; si < sizeof(sizes) / sizeof(sizes[0]); ++si)
        for (size_t pi = 0; pi < 3; ++pi)
            for (size_t seed_i = 0; seed_i < (pi == 0 ? 3 : 1); ++seed_i)
                for (uint64_t offset = 0; offset < 4; ++offset) {
                    uint64_t *storage = survey_make_words(sizes[si], seeds[seed_i], patterns[pi], offset);
                    if (!storage) { fprintf(stderr, "FAIL at allocation\n"); return 1; }
                    uint64_t expected = 0;
                    for (uint64_t i = 0; i < sizes[si]; ++i) expected += __builtin_popcountll(storage[offset + i]);
                    uint64_t actual = (uint64_t)popcnt(storage + offset, sizes[si] * sizeof(uint64_t));
                    free(storage);
                    ++cases;
                    if (actual != expected) { fprintf(stderr, "FAIL at case %zu\n", cases); return 1; }
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
