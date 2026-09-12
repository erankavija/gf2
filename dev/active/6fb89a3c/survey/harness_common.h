#ifndef GF2_HARNESS_COMMON_H
#define GF2_HARNESS_COMMON_H

#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include "json_min.h"

#include <errno.h>
#include <limits.h>
#include <sched.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct { uint64_t calls, elapsed_ns; } harness_window;
typedef void (*harness_body)(void *ctx, uint64_t call_index);

static __attribute__((unused)) uint64_t splitmix64_next(uint64_t *state)
{
    uint64_t z = (*state += UINT64_C(0x9E3779B97F4A7C15));
    z = (z ^ (z >> 30)) * UINT64_C(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)) * UINT64_C(0x94D049BB133111EB);
    return z ^ (z >> 31);
}

static uint64_t monotonic_ns(void)
{
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) return 0;
    return (uint64_t)ts.tv_sec * UINT64_C(1000000000) + (uint64_t)ts.tv_nsec;
}

static int run_windows(harness_body body, void *ctx, uint64_t windows, uint64_t target_ms,
                       harness_window **out, uint64_t *out_calls)
{
    uint64_t start, elapsed, calls, total = 0, w;
    harness_window *samples;
    if (windows == 0 || target_ms == 0 || windows > SIZE_MAX / sizeof(*samples)) return 0;
    start = monotonic_ns(); body(ctx, 0); elapsed = monotonic_ns() - start;
    if (elapsed == 0) elapsed = 1;
    calls = (target_ms > UINT64_MAX / UINT64_C(1000000)) ? UINT64_MAX : target_ms * UINT64_C(1000000) / elapsed;
    if (calls == 0) calls = 1;
    if (calls > UINT64_C(0x100000000)) calls = UINT64_C(0x100000000);
    samples = (harness_window *)calloc((size_t)windows, sizeof(*samples));
    if (samples == NULL) return 0;
    for (w = 0; w < windows; w++) {
        start = monotonic_ns();
        for (uint64_t i = 0; i < calls; i++) body(ctx, total + i);
        elapsed = monotonic_ns() - start;
        samples[w].calls = calls;
        samples[w].elapsed_ns = elapsed;
        total += calls;
    }
    *out = samples;
    if (out_calls != NULL) *out_calls = calls;
    return 1;
}

static int read_all(FILE *file, char **out, size_t *length)
{
    size_t used = 0, capacity = 4096;
    char *buffer = (char *)malloc(capacity);
    if (buffer == NULL) return 0;
    for (;;) {
        size_t got;
        if (used == capacity) { capacity *= 2; char *grown = (char *)realloc(buffer, capacity); if (grown == NULL) { free(buffer); return 0; } buffer = grown; }
        got = fread(buffer + used, 1, capacity - used, file);
        used += got;
        if (got == 0) break;
    }
    if (ferror(file)) { free(buffer); return 0; }
    *out = buffer; *length = used; return 1;
}

static int request_fields(const json_value *request, const json_value **case_value,
                          const char **cache, uint64_t *windows, uint64_t *target_ms)
{
    const json_value *v;
    if (request == NULL || request->type != JSON_OBJECT) return 0;
    v = json_object_get(request, "case"); if (v == NULL || v->type != JSON_OBJECT) return 0; *case_value = v;
    v = json_object_get(request, "cache_state"); if (!json_get_string(v, cache)) return 0;
    v = json_object_get(request, "windows"); if (!json_get_u64(v, windows)) return 0;
    v = json_object_get(request, "window_target_ms"); if (!json_get_u64(v, target_ms)) return 0;
    if (strcmp(*cache, "cold") && strcmp(*cache, "warm") && strcmp(*cache, "streaming")) return 0;
    return 1;
}

static size_t observed_cpus(uint32_t **out)
{
    cpu_set_t set;
    size_t count = 0;
    uint32_t *cpus;
    if (sched_getaffinity(0, sizeof(set), &set) != 0) return 0;
    for (int i = 0; i < CPU_SETSIZE; i++) if (CPU_ISSET(i, &set)) count++;
    cpus = (uint32_t *)malloc(count * sizeof(*cpus));
    if (cpus == NULL && count != 0) return 0;
    count = 0;
    for (int i = 0; i < CPU_SETSIZE; i++) if (CPU_ISSET(i, &set)) cpus[count++] = (uint32_t)i;
    *out = cpus; return count;
}

static void print_windows_json(const harness_window *samples, uint64_t count)
{
    printf("["
    );
    for (uint64_t i = 0; i < count; i++) {
        if (i != 0) putchar(',');
        printf("{\"calls\":%llu,\"elapsed_ns\":%llu}", (unsigned long long)samples[i].calls, (unsigned long long)samples[i].elapsed_ns);
    }
    putchar(']');
}

static void print_cpus_json(const uint32_t *cpus, size_t count)
{
    putchar('[');
    for (size_t i = 0; i < count; i++) { if (i != 0) putchar(','); printf("%u", cpus[i]); }
    putchar(']');
}

static int emit_result(const harness_window *samples, uint64_t windows, const char *cache,
                       const char *path, int conversion, uint64_t setup_ns, uint64_t pack_ns,
                       uint64_t unpack_ns, uint64_t batch_fill_ns, uint64_t dispatch_ns)
{
    uint32_t *cpus = NULL; size_t cpu_count = observed_cpus(&cpus);
    if (cpu_count == 0 || cpus == NULL) { fprintf(stderr, "cannot observe process CPU affinity\n"); free(cpus); return 0; }
    fputs("GF2_TUNING_RESULT={\"schema\":\"zen3-benchmark-arm-result-v1\",\"windows\":", stdout);
    print_windows_json(samples, windows);
    printf(",\"cache_state_applied\":\"%s\",\"workers_observed\":1,\"cpus_observed\":", cache);
    print_cpus_json(cpus, cpu_count);
    printf(",\"selected_path\":\"%s\",\"conversion\":", path);
    if (!conversion) puts("null,\"quality\":null}");
    else printf("{\"setup_ns\":%llu,\"pack_ns\":%llu,\"unpack_ns\":%llu,\"batch_fill_ns\":%llu,\"dispatch_ns\":%llu},\"quality\":null}\n", (unsigned long long)setup_ns, (unsigned long long)pack_ns, (unsigned long long)unpack_ns, (unsigned long long)batch_fill_ns, (unsigned long long)dispatch_ns);
    free(cpus);
    return fflush(stdout) == 0;
}

static int require_child_mode(void)
{
    const char *value = getenv("GF2_TUNING_FRESH_CASE");
    if (value == NULL || strcmp(value, "child-v2") != 0) { fprintf(stderr, "GF2_TUNING_FRESH_CASE must equal child-v2\n"); return 0; }
    return 1;
}

static int parse_case_argument(const char *text, json_value **out)
{
    char *error = NULL;
    if (!json_parse(text, strlen(text), out, &error)) { fprintf(stderr, "case JSON parse failed: %s\n", error ? error : "unknown error"); free(error); return 0; }
    return 1;
}

#endif
