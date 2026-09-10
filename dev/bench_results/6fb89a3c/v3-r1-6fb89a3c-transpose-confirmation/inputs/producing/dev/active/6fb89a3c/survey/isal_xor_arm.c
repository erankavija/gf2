/* ISA-L logical XOR / parity comparison arm for jit:6fb89a3c.
 *
 * raid.h documents xor_gen(vects, len, array) as writing array[vects-1] with
 * the XOR of array[0] through array[vects-2] ("Must be > 2" vectors, "Src and
 * dest pointers must be aligned to 32B"). With vects=3 this is a fresh
 * overwrite of dest with src0 ^ src1; with vects=4 a three-source parity.
 * It is not an accumulate operation and the contract names distinct source
 * and destination pointers, so a gf2 `dst ^= src` aliasing arrangement has
 * no ISA-L equivalent and is recorded unavailable.
 *
 * This harness calls xor_gen_base, not the public multi-binary xor_gen:
 * xor_gen's SSE/AVX/AVX-512 dispatch (raid_multibinary.asm) needs NASM,
 * which is unavailable on this host (see fetch-build.sh). raid/raid_base.c's
 * xor_gen_base has the identical (vects, len, array) contract and is ISA-L's
 * own portable C reference for the same operation. It is compiled here with
 * -O3 -march=native, so what the compiler made of its byte loop is an
 * observed fact (record-build-evidence.py disassembles it), not a label.
 *
 * Case `{words, seed, alignment_bytes: 32, sources?}`; `sources` defaults to
 * 2 and sets vects = sources + 1. The pointer array is formed inside every
 * timed call; `dispatch_ns` reports one separately measured formation.
 */
#include "harness_common.h"
#include <raid.h>

/* Declared directly: raid.h declares the multi-binary `xor_gen` dispatcher
 * and `xor_gen_base`; only the latter is linked here (raid/raid_base.c,
 * compiled and archived by fetch-build.sh). */
int xor_gen_base(int vects, int len, void **array);

#define MAX_SOURCES 8

typedef struct { uint64_t *src[8][MAX_SOURCES], *dest[8]; size_t words, sources, banks; } xor_ctx;

static int get_case(const json_value *object, size_t *words, size_t *sources, uint64_t *seed)
{
    uint64_t count, alignment, arity = 2;
    if (!json_get_u64(json_object_get(object, "words"), &count) || !json_get_u64(json_object_get(object, "seed"), seed) || count == 0 || count > (uint64_t)INT_MAX / sizeof(uint64_t)) return 0;
    if (!json_get_u64(json_object_get(object, "alignment_bytes"), &alignment) || alignment != 32) { fprintf(stderr, "ISA-L xor_gen requires 32-byte aligned source and destination pointers\n"); return 0; }
    if (json_object_get(object, "sources") != NULL && (!json_get_u64(json_object_get(object, "sources"), &arity) || arity < 2 || arity > MAX_SOURCES)) return 0;
    *words = (size_t)count; *sources = (size_t)arity; return 1;
}

static void fill_words(uint64_t *buffer, size_t words, uint64_t seed)
{
    for (size_t i = 0; i < words; i++) buffer[i] = splitmix64_next(&seed);
}

static uint64_t *alloc_words(size_t words)
{
    void *pointer = NULL;
    if (posix_memalign(&pointer, 32, words * sizeof(uint64_t)) != 0) return NULL;
    return (uint64_t *)pointer;
}

/* The two-source/one-output (or n-source) arrangement: the pointer array is
 * formed here, inside every call. */
static int do_xor(uint64_t **src, size_t sources, uint64_t *dest, size_t words)
{
    void *array[MAX_SOURCES + 1];
    for (size_t s = 0; s < sources; s++) array[s] = src[s];
    array[sources] = dest;
    return xor_gen_base((int)sources + 1, (int)(words * sizeof(uint64_t)), array) == 0;
}

static int dump_case(const json_value *object)
{
    size_t words, sources; uint64_t seed; uint64_t *src[MAX_SOURCES] = {0}, *dest;
    if (!get_case(object, &words, &sources, &seed)) { fprintf(stderr, "invalid ISA-L XOR case\n"); return 0; }
    dest = alloc_words(words);
    for (size_t s = 0; s < sources; s++) { src[s] = alloc_words(words); if (src[s] == NULL) return 0; fill_words(src[s], words, seed + s); }
    if (dest == NULL) return 0;
    if (!do_xor(src, sources, dest, words)) { fprintf(stderr, "ISA-L xor_gen_base failed\n"); return 0; }
    for (size_t w = 0; w < words; w++) for (size_t b = 0; b < 64; b++) putchar((dest[w] >> b) & 1 ? '1' : '0');
    putchar('\n');
    for (size_t s = 0; s < sources; s++) free(src[s]);
    free(dest); return fflush(stdout) == 0;
}

static void xor_body(void *opaque, uint64_t call_index)
{
    xor_ctx *ctx = (xor_ctx *)opaque; size_t bank = call_index % ctx->banks;
    if (!do_xor(ctx->src[bank], ctx->sources, ctx->dest[bank], ctx->words)) { fprintf(stderr, "ISA-L xor_gen_base failed in timed call\n"); abort(); }
}

/* Measures forming the pointer array alone, averaged over many repetitions
 * so a sub-nanosecond cost still rounds to an observed value. The empty asm
 * takes the array's address and clobbers memory, so every repetition must
 * store all vects pointers; without it GCC deletes the loop and the probe
 * times two adjacent clock reads (the protocol-v3 pilots reported 0 ns). */
static uint64_t arrangement_probe_ns(xor_ctx *ctx)
{
    enum { REPS = 1000000 };
    uint64_t start = monotonic_ns();
    for (int i = 0; i < REPS; i++) {
        void *array[MAX_SOURCES + 1];
        for (size_t s = 0; s < ctx->sources; s++) array[s] = ctx->src[0][s];
        array[ctx->sources] = ctx->dest[0];
        __asm__ volatile("" : : "r"(array) : "memory");
    }
    return (monotonic_ns() - start + REPS / 2) / REPS;
}

static int transport_case(const json_value *object, const char *cache, uint64_t windows, uint64_t target_ms)
{
    xor_ctx ctx; size_t words, sources; uint64_t seed, setup_start = monotonic_ns(), setup_ns, dispatch_ns; harness_window *samples = NULL; char selected[96];
    memset(&ctx, 0, sizeof(ctx));
    if (!get_case(object, &words, &sources, &seed)) { fprintf(stderr, "invalid ISA-L XOR case\n"); return 0; }
    ctx.words = words; ctx.sources = sources; ctx.banks = strcmp(cache, "streaming") == 0 ? 8u : 1u;
    for (size_t i = 0; i < ctx.banks; i++) {
        for (size_t s = 0; s < sources; s++) { ctx.src[i][s] = alloc_words(words); if (ctx.src[i][s] == NULL) { fprintf(stderr, "cannot allocate ISA-L XOR buffers\n"); return 0; } fill_words(ctx.src[i][s], words, seed + i * MAX_SOURCES + s); }
        ctx.dest[i] = alloc_words(words);
        if (ctx.dest[i] == NULL) { fprintf(stderr, "cannot allocate ISA-L XOR buffers\n"); return 0; }
        memset(ctx.dest[i], 0, words * sizeof(uint64_t));
    }
    setup_ns = monotonic_ns() - setup_start;
    dispatch_ns = arrangement_probe_ns(&ctx);
    if (!strcmp(cache, "warm")) for (size_t i = 0; i < ctx.banks; i++) xor_body(&ctx, i);
    if (!run_windows(xor_body, &ctx, windows, target_ms, &samples, NULL)) { fprintf(stderr, "ISA-L XOR timing failed\n"); return 0; }
    (void)snprintf(selected, sizeof(selected), "isa-l-xor_gen_base-vects%zu-aligned32", sources + 1);
    if (!emit_result(samples, windows, cache, selected, 1, setup_ns, 0, 0, 0, dispatch_ns)) return 0;
    free(samples);
    for (size_t i = 0; i < ctx.banks; i++) { for (size_t s = 0; s < sources; s++) free(ctx.src[i][s]); free(ctx.dest[i]); }
    return 1;
}

int main(int argc, char **argv)
{
    char *input = NULL, *error = NULL; size_t length; json_value *root = NULL; const json_value *object; const char *cache; uint64_t windows, target_ms; int ok;
    if (argc == 2 && !strcmp(argv[1], "--backend")) {
        /* Which routine is linked is a build fact; the instruction-level
         * observation of xor_gen_base comes from the disassembly probe in
         * record-build-evidence.py. */
        puts("{\"library\":\"isa-l\",\"entrypoint\":\"xor_gen_base\",\"runtime_dispatch\":false,\"public_xor_gen_available\":false,\"unavailable_reason\":\"NASM is absent; raid_multibinary.asm and the SSE/AVX/AVX-512 xor_gen objects cannot be built\",\"observation\":\"linked symbol; instruction mix recorded by the disassembly probe\"}");
        return 0;
    }
    if (argc == 3 && !strcmp(argv[1], "--dump-check")) { if (!parse_case_argument(argv[2], &root)) return 2; ok = dump_case(root); json_free(root); return ok ? 0 : 1; }
    if (argc != 1 || !require_child_mode() || !read_all(stdin, &input, &length) || !json_parse(input, length, &root, &error)) { fprintf(stderr, "isal_xor_arm: invalid child-v2 request: %s\n", error ? error : "read or parse failed"); free(error); free(input); return 2; }
    free(input); if (!request_fields(root, &object, &cache, &windows, &target_ms)) { fprintf(stderr, "isal_xor_arm: invalid child-v2 fields\n"); json_free(root); return 2; }
    ok = transport_case(object, cache, windows, target_ms); json_free(root); return ok ? 0 : 1;
}
