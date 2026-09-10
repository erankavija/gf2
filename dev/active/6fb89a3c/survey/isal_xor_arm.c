/* ISA-L logical XOR comparison arm for jit:6fb89a3c.
 *
 * raid.h documents xor_gen(vects, len, array) as writing array[vects-1] with
 * the XOR of array[0] through array[vects-2]. With vects=3 this is a fresh
 * overwrite of dest with src0 ^ src1, not an accumulate operation.
 *
 * This harness calls xor_gen_base, not the public multi-binary xor_gen:
 * xor_gen's AVX/AVX512 dispatch (raid_multibinary.asm) needs NASM, which is
 * unavailable on this host (see fetch-build.sh). raid/raid_base.c's
 * xor_gen_base has the identical (vects, len, array) contract and is ISA-L's
 * own portable C reference for the same operation -- not a different one.
 */
#include "harness_common.h"
#include <raid.h>

/* Declared directly: raid.h only declares the multi-binary `xor_gen`
 * dispatcher, which this build does not link (see above). The symbol comes
 * from raid/raid_base.c, compiled and archived by fetch-build.sh. */
int xor_gen_base(int vects, int len, void **array);

typedef struct { uint64_t *src0[8], *src1[8], *dest[8]; size_t words, banks; } xor_ctx;

static int get_case(const json_value *object, size_t *words, uint64_t *seed)
{
    uint64_t count, alignment;
    if (!json_get_u64(json_object_get(object, "words"), &count) || !json_get_u64(json_object_get(object, "seed"), seed) || count == 0 || count > SIZE_MAX / sizeof(uint64_t)) return 0;
    if (!json_get_u64(json_object_get(object, "alignment_bytes"), &alignment) || alignment != 32) return 0;
    *words = (size_t)count; return 1;
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

static int do_xor(uint64_t *src0, uint64_t *src1, uint64_t *dest, size_t words)
{
    void *array[3] = {src0, src1, dest};
    return xor_gen_base(3, (int)(words * sizeof(uint64_t)), array) == 0;
}

static int dump_case(const json_value *object)
{
    size_t words; uint64_t seed; uint64_t *src0, *src1, *dest;
    if (!get_case(object, &words, &seed) || words > (size_t)INT_MAX / sizeof(uint64_t)) { fprintf(stderr, "invalid ISA-L XOR case\n"); return 0; }
    src0 = alloc_words(words); src1 = alloc_words(words); dest = alloc_words(words);
    if (src0 == NULL || src1 == NULL || dest == NULL) { free(src0); free(src1); free(dest); return 0; }
    fill_words(src0, words, seed); fill_words(src1, words, seed + 1);
    if (!do_xor(src0, src1, dest, words)) { fprintf(stderr, "ISA-L xor_gen failed\n"); free(src0); free(src1); free(dest); return 0; }
    for (size_t w = 0; w < words; w++) for (size_t b = 0; b < 64; b++) putchar((dest[w] >> b) & 1 ? '1' : '0');
    putchar('\n'); free(src0); free(src1); free(dest); return fflush(stdout) == 0;
}

static void xor_body(void *opaque, uint64_t call_index)
{
    xor_ctx *ctx = (xor_ctx *)opaque;
    if (!do_xor(ctx->src0[call_index % ctx->banks], ctx->src1[call_index % ctx->banks], ctx->dest[call_index % ctx->banks], ctx->words)) { fprintf(stderr, "ISA-L xor_gen failed in timed call\n"); abort(); }
}

static int transport_case(const json_value *object, const char *cache, uint64_t windows, uint64_t target_ms)
{
    xor_ctx ctx = {{0}, {0}, {0}, 0, strcmp(cache, "streaming") == 0 ? 8u : 1u}; size_t words; uint64_t seed, setup_start = monotonic_ns(), setup_ns; harness_window *samples = NULL;
    if (!get_case(object, &words, &seed) || words > (size_t)INT_MAX / sizeof(uint64_t)) { fprintf(stderr, "invalid ISA-L XOR case\n"); return 0; }
    ctx.words = words;
    for (size_t i = 0; i < ctx.banks; i++) {
        ctx.src0[i] = alloc_words(words); ctx.src1[i] = alloc_words(words); ctx.dest[i] = alloc_words(words);
        if (ctx.src0[i] == NULL || ctx.src1[i] == NULL || ctx.dest[i] == NULL) { fprintf(stderr, "cannot allocate ISA-L XOR buffers\n"); return 0; }
        fill_words(ctx.src0[i], words, seed + i); fill_words(ctx.src1[i], words, seed + i + 1);
    }
    setup_ns = monotonic_ns() - setup_start;
    if (!strcmp(cache, "warm")) for (size_t i = 0; i < ctx.banks; i++) xor_body(&ctx, i);
    if (!run_windows(xor_body, &ctx, windows, target_ms, &samples, NULL)) { fprintf(stderr, "ISA-L XOR timing failed\n"); return 0; }
    /* The three-pointer array is formed inside every do_xor call, so ISA-L's
     * two-source/one-output arrangement cost participates in every window. */
    if (!emit_result(samples, windows, cache, "isa-l-xor_gen_base-arity3-aligned32", 1, setup_ns, 0, 0, 0, 0)) return 0;
    free(samples); for (size_t i = 0; i < ctx.banks; i++) { free(ctx.src0[i]); free(ctx.src1[i]); free(ctx.dest[i]); } return 1;
}

int main(int argc, char **argv)
{
    char *input = NULL, *error = NULL; size_t length; json_value *root = NULL; const json_value *object; const char *cache; uint64_t windows, target_ms; int ok;
    if (argc == 2 && !strcmp(argv[1], "--backend")) {
        puts("{\"library\":\"isa-l\",\"entrypoint\":\"xor_gen_base\",\"selected_backend\":\"scalar-base\",\"runtime_dispatch\":false,\"public_xor_gen_available\":false,\"unavailable_reason\":\"NASM is absent; raid_multibinary.asm and SIMD xor_gen objects cannot be built\"}");
        return 0;
    }
    if (argc == 3 && !strcmp(argv[1], "--dump-check")) { if (!parse_case_argument(argv[2], &root)) return 2; ok = dump_case(root); json_free(root); return ok ? 0 : 1; }
    if (argc != 1 || !require_child_mode() || !read_all(stdin, &input, &length) || !json_parse(input, length, &root, &error)) { fprintf(stderr, "isal_xor_arm: invalid child-v2 request: %s\n", error ? error : "read or parse failed"); free(error); free(input); return 2; }
    free(input); if (!request_fields(root, &object, &cache, &windows, &target_ms)) { fprintf(stderr, "isal_xor_arm: invalid child-v2 fields\n"); json_free(root); return 2; }
    ok = transport_case(object, cache, windows, target_ms); json_free(root); return ok ? 0 : 1;
}
