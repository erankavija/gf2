#ifndef GF2_SURVEY_WIRE_COMMON_H
#define GF2_SURVEY_WIRE_COMMON_H

#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include "splitmix64.h"
#include <errno.h>
#include <inttypes.h>
#include <sched.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define SURVEY_BANKS 8U
#define SURVEY_MAX_WINDOWS 5U
#define SURVEY_MAX_CPUS 4096U
#define SURVEY_MAX_STRING 256U

typedef struct {
    char schema[SURVEY_MAX_STRING];
    char cell_id[SURVEY_MAX_STRING];
    char arm[SURVEY_MAX_STRING];
    char role[SURVEY_MAX_STRING];
    uint64_t pair;
    char case_op[64];
    uint64_t words;
    uint64_t seed;
    uint64_t seed_lhs;
    uint64_t seed_rhs;
    char pattern[32];
    uint64_t word_offset;
    char cache_state[32];
    uint64_t windows;
    uint64_t window_target_ms;
    uint32_t cpus[SURVEY_MAX_CPUS];
    size_t cpu_count;
    uint64_t workers_declared;
} survey_request_t;

typedef struct {
    const char *text;
    size_t length;
    size_t position;
} survey_json_t;

static inline int survey_json_fail(survey_json_t *j) {
    (void)j;
    return 0;
}

static inline int survey_json_char(survey_json_t *j, char wanted) {
    if (j->position >= j->length || j->text[j->position] != wanted) return survey_json_fail(j);
    ++j->position;
    return 1;
}

static inline int survey_json_string(survey_json_t *j, char *out, size_t capacity) {
    size_t used = 0;
    if (!survey_json_char(j, '"')) return 0;
    while (j->position < j->length) {
        unsigned char c = (unsigned char)j->text[j->position++];
        if (c == '"') {
            if (used >= capacity) return 0;
            out[used] = '\0';
            return 1;
        }
        if (c < 0x20 || c == '\\') {
            if (c != '\\') return 0;
            if (j->position >= j->length) return 0;
            c = (unsigned char)j->text[j->position++];
            if (c == '"' || c == '\\' || c == '/' || c == 'b' || c == 'f' ||
                c == 'n' || c == 'r' || c == 't') {
                if (used + 1 >= capacity) return 0;
                out[used++] = (c == '"' || c == '\\' || c == '/') ? (char)c : ' ';
            } else {
                return 0;
            }
        } else {
            if (used + 1 >= capacity) return 0;
            out[used++] = (char)c;
        }
    }
    return 0;
}

static inline int survey_json_u64(survey_json_t *j, uint64_t *out) {
    uint64_t value = 0;
    size_t start = j->position;
    if (start >= j->length || j->text[start] < '0' || j->text[start] > '9') return 0;
    while (j->position < j->length && j->text[j->position] >= '0' && j->text[j->position] <= '9') {
        unsigned digit = (unsigned)(j->text[j->position++] - '0');
        if (value > (UINT64_MAX - digit) / 10U) return 0;
        value = value * 10U + digit;
    }
    *out = value;
    return j->position > start;
}

static inline int survey_json_case(survey_json_t *j, survey_request_t *r) {
    unsigned seen = 0;
    if (!survey_json_char(j, '{')) return 0;
    if (j->position < j->length && j->text[j->position] == '}') { ++j->position; return 1; }
    for (;;) {
        char key[64];
        if (!survey_json_string(j, key, sizeof(key)) || !survey_json_char(j, ':')) return 0;
        unsigned bit = 0;
        if (strcmp(key, "op") == 0) { bit = 1U; if (seen & bit || !survey_json_string(j, r->case_op, sizeof(r->case_op))) return 0; }
        else if (strcmp(key, "words") == 0) { bit = 2U; if (seen & bit || !survey_json_u64(j, &r->words)) return 0; }
        else if (strcmp(key, "seed") == 0) { bit = 4U; if (seen & bit || !survey_json_u64(j, &r->seed)) return 0; }
        else if (strcmp(key, "seed_lhs") == 0) { bit = 8U; if (seen & bit || !survey_json_u64(j, &r->seed_lhs)) return 0; }
        else if (strcmp(key, "seed_rhs") == 0) { bit = 16U; if (seen & bit || !survey_json_u64(j, &r->seed_rhs)) return 0; }
        else if (strcmp(key, "pattern") == 0) { bit = 32U; if (seen & bit || !survey_json_string(j, r->pattern, sizeof(r->pattern))) return 0; }
        else if (strcmp(key, "word_offset") == 0) { bit = 64U; if (seen & bit || !survey_json_u64(j, &r->word_offset)) return 0; }
        else return 0;
        seen |= bit;
        if (j->position < j->length && j->text[j->position] == '}') { ++j->position; return 1; }
        if (!survey_json_char(j, ',')) return 0;
    }
}

static inline int survey_parse_request(const char *text, size_t length, survey_request_t *r) {
    unsigned seen = 0;
    memset(r, 0, sizeof(*r));
    if (length > 0 && text[length - 1] == '\n') --length;
    survey_json_t j = { text, length, 0 };
    if (!survey_json_char(&j, '{')) return 0;
    if (j.position < j.length && j.text[j.position] == '}') return 0;
    for (;;) {
        char key[64];
        if (!survey_json_string(&j, key, sizeof(key)) || !survey_json_char(&j, ':')) return 0;
        unsigned bit = 0;
        if (strcmp(key, "schema") == 0) { bit = 1U; if (seen & bit || !survey_json_string(&j, r->schema, sizeof(r->schema))) return 0; }
        else if (strcmp(key, "cell_id") == 0) { bit = 2U; if (seen & bit || !survey_json_string(&j, r->cell_id, sizeof(r->cell_id))) return 0; }
        else if (strcmp(key, "arm") == 0) { bit = 4U; if (seen & bit || !survey_json_string(&j, r->arm, sizeof(r->arm))) return 0; }
        else if (strcmp(key, "role") == 0) { bit = 8U; if (seen & bit || !survey_json_string(&j, r->role, sizeof(r->role))) return 0; }
        else if (strcmp(key, "pair") == 0) { bit = 16U; if (seen & bit || !survey_json_u64(&j, &r->pair)) return 0; }
        else if (strcmp(key, "case") == 0) { bit = 32U; if (seen & bit || !survey_json_case(&j, r)) return 0; }
        else if (strcmp(key, "cache_state") == 0) { bit = 64U; if (seen & bit || !survey_json_string(&j, r->cache_state, sizeof(r->cache_state))) return 0; }
        else if (strcmp(key, "windows") == 0) { bit = 128U; if (seen & bit || !survey_json_u64(&j, &r->windows)) return 0; }
        else if (strcmp(key, "window_target_ms") == 0) { bit = 256U; if (seen & bit || !survey_json_u64(&j, &r->window_target_ms)) return 0; }
        else if (strcmp(key, "cpus") == 0) {
            bit = 512U;
            if (seen & bit || !survey_json_char(&j, '[')) return 0;
            if (j.position < j.length && j.text[j.position] == ']') { ++j.position; }
            else {
                for (;;) {
                    uint64_t cpu;
                    if (r->cpu_count >= SURVEY_MAX_CPUS || !survey_json_u64(&j, &cpu) || cpu > UINT32_MAX) return 0;
                    r->cpus[r->cpu_count++] = (uint32_t)cpu;
                    if (j.position < j.length && j.text[j.position] == ']') { ++j.position; break; }
                    if (!survey_json_char(&j, ',')) return 0;
                }
            }
        }
        else if (strcmp(key, "workers_declared") == 0) { bit = 1024U; if (seen & bit || !survey_json_u64(&j, &r->workers_declared)) return 0; }
        else return 0;
        seen |= bit;
        if (j.position < j.length && j.text[j.position] == '}') { ++j.position; break; }
        if (!survey_json_char(&j, ',')) return 0;
    }
    return j.position == j.length && seen == 2047U &&
        (strcmp(r->cache_state, "cold") == 0 || strcmp(r->cache_state, "warm") == 0 || strcmp(r->cache_state, "streaming") == 0) &&
        r->windows >= 1 && r->windows <= SURVEY_MAX_WINDOWS && r->window_target_ms > 0 &&
        r->word_offset <= 3;
}

static inline int survey_read_stdin(char **out, size_t *length) {
    size_t capacity = 4096, used = 0;
    char *buffer = (char *)malloc(capacity);
    if (!buffer) return 0;
    for (;;) {
        if (used == capacity) {
            size_t next = capacity > SIZE_MAX / 2U ? 0 : capacity * 2U;
            char *grown;
            if (!next || !(grown = (char *)realloc(buffer, next))) { free(buffer); return 0; }
            buffer = grown; capacity = next;
        }
        size_t n = fread(buffer + used, 1, capacity - used, stdin);
        used += n;
        if (n == 0) {
            if (ferror(stdin)) { free(buffer); return 0; }
            break;
        }
    }
    *out = buffer; *length = used; return 1;
}

/// Allocates the base array 32-byte aligned so `word_offset == 0` is a
/// well-defined AVX2-vector-aligned buffer and `word_offset` in `1..=3`
/// deterministically breaks that alignment by 8/16/24 bytes (glibc `malloc`
/// itself only guarantees 16-byte alignment, which is not sufficient here).
static inline uint64_t *survey_make_words(uint64_t words, uint64_t seed, const char *pattern, uint64_t offset) {
    if (words > (SIZE_MAX / sizeof(uint64_t)) - 4U || offset > 3U) return NULL;
    size_t total = (size_t)words + 4U;
    void *raw = NULL;
    if (posix_memalign(&raw, 32, total * sizeof(uint64_t)) != 0) return NULL;
    uint64_t *all = (uint64_t *)raw;
    if (!all) return NULL;
    splitmix64_t generator;
    splitmix64_init(&generator, seed);
    for (size_t i = 0; i < total; ++i) {
        all[i] = strcmp(pattern, "random") == 0 ? splitmix64_next(&generator) :
            strcmp(pattern, "all_zero") == 0 ? 0U : UINT64_MAX;
    }
    if (strcmp(pattern, "random") != 0 && strcmp(pattern, "all_zero") != 0 && strcmp(pattern, "all_one") != 0) { free(all); return NULL; }
    return all;
}

/// Banks a cell allocates and rotates through. A streaming cell uses all
/// eight so successive calls do not reuse cache-resident data; a cold or warm
/// cell keeps one working set. Mirrors `bank_count` in the gf2-side harness and
/// the `banks` selection in `ab-smoke-workload.rs`, so both sides of the survey
/// present the same working-set size to the same declared cache state.
static inline size_t survey_bank_count(const char *cache_state) {
    return strcmp(cache_state, "streaming") == 0 ? (size_t)SURVEY_BANKS : (size_t)1;
}

/// Prints the byte alignment of the fixture window each `word_offset` selects.
/// The gf2-side harness prints the identical lines under `--alignment`, so the
/// alignment cell's premise -- both sides time the same bytes at the same
/// alignment -- is checkable by comparing two command outputs.
static inline int survey_report_alignment(void) {
    static const uint64_t sizes[] = {4, 8, 64, 256};
    for (size_t si = 0; si < sizeof(sizes) / sizeof(sizes[0]); ++si) {
        for (uint64_t offset = 0; offset < 4; ++offset) {
            uint64_t *storage = survey_make_words(sizes[si], 1, "random", offset);
            if (!storage) return 1;
            printf("words=%" PRIu64 " word_offset=%" PRIu64 " address_mod_32=%" PRIuPTR "\n",
                   sizes[si], offset, (uintptr_t)(storage + offset) % 32U);
            free(storage);
        }
    }
    return 0;
}

typedef uint64_t (*survey_call_fn)(void *context, size_t bank);
typedef struct { uint64_t calls; uint64_t elapsed_ns; } survey_sample_t;
static volatile uint64_t survey_sink;

static inline uint64_t survey_clock_ns(void) {
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return 0;
    return (uint64_t)now.tv_sec * UINT64_C(1000000000) + (uint64_t)now.tv_nsec;
}

static inline uint64_t survey_run_calls(uint64_t calls, size_t start, int streaming, survey_call_fn call, void *context) {
    uint64_t before = survey_clock_ns();
    for (uint64_t i = 0; i < calls; ++i) {
        size_t bank = streaming ? (start + (size_t)i) & (SURVEY_BANKS - 1U) : 0U;
        survey_sink ^= call(context, bank);
    }
    uint64_t after = survey_clock_ns();
    return after >= before ? after - before : 0;
}

static inline int survey_time(uint64_t windows, uint64_t target_ms, int streaming, survey_call_fn call, void *context, survey_sample_t *samples) {
    uint64_t target_ns = target_ms * UINT64_C(1000000);
    uint64_t probe_target = target_ns < UINT64_C(20000000) ? target_ns : UINT64_C(20000000);
    uint64_t calls = 1, elapsed;
    do {
        elapsed = survey_run_calls(calls, 0, streaming, call, context);
        if (elapsed >= probe_target || calls >= UINT64_C(4294967296)) break;
        calls = calls > UINT64_C(2147483648) ? UINT64_C(4294967296) : calls * 2U;
    } while (1);
    __uint128_t wanted128 = (__uint128_t)target_ns * calls / (elapsed ? elapsed : 1U);
    uint64_t wanted = wanted128 > UINT64_C(4294967296) ? UINT64_C(4294967296) : (uint64_t)wanted128;
    if (wanted == 0) wanted = 1;
    for (uint64_t repetition = 0; repetition < windows; ++repetition) {
        samples[repetition].calls = wanted;
        samples[repetition].elapsed_ns = survey_run_calls(wanted, (size_t)repetition & 7U, streaming, call, context);
        if (samples[repetition].elapsed_ns == 0) return 0;
    }
    return 1;
}

static inline size_t survey_observed_cpus(uint32_t *out) {
    cpu_set_t set;
    if (sched_getaffinity(0, sizeof(set), &set) != 0) return 0;
    size_t count = 0;
    for (size_t cpu = 0; cpu < CPU_SETSIZE && count < SURVEY_MAX_CPUS; ++cpu)
        if (CPU_ISSET((int)cpu, &set)) out[count++] = (uint32_t)cpu;
    return count;
}

static inline int survey_write_result(const survey_request_t *r, const char *path, const survey_sample_t *samples) {
    uint32_t cpus[SURVEY_MAX_CPUS];
    size_t count = survey_observed_cpus(cpus);
    fputs("GF2_TUNING_RESULT={\"schema\":\"zen3-benchmark-arm-result-v1\",\"windows\":[", stdout);
    for (uint64_t i = 0; i < r->windows; ++i) {
        if (i) putchar(',');
        printf("{\"calls\":%" PRIu64 ",\"elapsed_ns\":%" PRIu64 "}", samples[i].calls, samples[i].elapsed_ns);
    }
    fputs("],\"cache_state_applied\":\"", stdout); fputs(r->cache_state, stdout);
    fputs("\",\"workers_observed\":1,\"cpus_observed\":[", stdout);
    for (size_t i = 0; i < count; ++i) { if (i) putchar(','); printf("%u", cpus[i]); }
    printf("],\"selected_path\":\"%s\",\"conversion\":null,\"quality\":null}\n", path);
    return ferror(stdout) ? 0 : 1;
}

#endif
