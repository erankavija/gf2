/* Bitshuffle transpose comparison arm for jit:6fb89a3c.
 *
 * bshuf_bitshuffle(in, out, size, elem_size, block_size) transposes `size`
 * elements of `elem_size` bytes into `8 * elem_size` bit-planes of `size / 8`
 * bytes: bit `c` of element `r` becomes bit `r % 8` of byte `r / 8` in plane
 * `c`, LSB-first. For 64 elements of 8 bytes this is exactly gf2's canonical
 * 64x64 word transpose (row word `r`, bit `c` -> row word `c`, bit `r`) with
 * no adapter; `verify-bit-mapping.py` checks that mapping against naive bit
 * arithmetic before any timing.
 *
 * Fixed case `{n: 64, seed}`: the kernel-isolated 64x64 transform into a
 * preallocated output.
 *
 * Tiled case `{rows, cols, seed}`: a whole-consumer transpose of a gf2
 * `BitMatrix` (row stride `ceil(cols / 64)` words) into a fresh
 * `cols x rows` matrix. Bitshuffle requires `size % 8 == 0`, so a row count
 * that is not a multiple of eight is unavailable without padding and the
 * arm exits with an unavailability message. With `"adapter": "padded"` the
 * arm measures the geometry adapter instead: it packs the matrix into
 * `round_up(rows, 8)` elements of `ceil(cols / 8)` bytes, runs
 * `bshuf_bitshuffle` over that one block, and unpacks the first `cols`
 * planes into the fresh canonical output. Pack and unpack are byte copies
 * that are skipped when the canonical layout already coincides with
 * Bitshuffle's (pack: `rows % 8 == 0 && cols % 64 == 0`; unpack:
 * `rows % 64 == 0 && cols % 8 == 0`), so the 64x64 consumer pays no copy and
 * 63x63 / 65x65 pay the copies inside every timed call. `pack_ns` and
 * `unpack_ns` report one separately measured pass of each copy.
 *
 * GF2_BITSHUFFLE_ADAPTER selects byte/bit-order permutations for the
 * `--dump-check` mapping search only; the direct mapping is what transport
 * mode uses.
 */
#include "harness_common.h"
#include <bitshuffle_core.h>

typedef struct {
    /* Canonical gf2 layouts. */
    uint64_t *in_words[8];
    size_t rows, cols, in_stride, out_stride, banks;
    /* Bitshuffle geometry. */
    size_t rows_padded, elem, planes_bytes;
    unsigned char *pad, *planes;
    int pack_needed, unpack_needed, fixed;
    /* Fixed-case preallocated output. */
    uint64_t *fixed_out;
} bit_ctx;

static unsigned char reverse_byte(unsigned char x)
{
    x = (unsigned char)((x >> 4) | (x << 4)); x = (unsigned char)(((x & 0xcc) >> 2) | ((x & 0x33) << 2)); return (unsigned char)(((x & 0xaa) >> 1) | ((x & 0x55) << 1));
}

static int adapter_has(const char *adapter, const char *name)
{
    const char *at = adapter;
    size_t wanted = strlen(name);
    if (at == NULL) return 0;
    while (*at != '\0') {
        const char *end = strchr(at, '+'); size_t length = end == NULL ? strlen(at) : (size_t)(end - at);
        if ((length == wanted && !strncmp(at, name, wanted)) || (length == 4 && !strncmp(at, "both", 4) && (!strcmp(name, "input-byte-reverse") || !strcmp(name, "output-byte-reverse")))) return 1;
        if (end == NULL) break;
        at = end + 1;
    }
    return 0;
}

static void apply_input_adapter(unsigned char *buffer, size_t bytes, size_t elem_size, const char *adapter)
{
    if (adapter_has(adapter, "input-byte-reverse")) for (size_t i = 0; i < bytes; i++) buffer[i] = reverse_byte(buffer[i]);
    if (adapter_has(adapter, "element-byte-reverse")) for (size_t base = 0; base < bytes; base += elem_size) for (size_t i = 0; i < elem_size / 2; i++) { unsigned char t = buffer[base + i]; buffer[base + i] = buffer[base + elem_size - 1 - i]; buffer[base + elem_size - 1 - i] = t; }
}

static void apply_output_adapter(unsigned char *buffer, size_t bytes, const char *adapter)
{
    if (adapter_has(adapter, "output-byte-reverse")) for (size_t i = 0; i < bytes; i++) buffer[i] = reverse_byte(buffer[i]);
}

static const char *case_adapter(const json_value *object)
{
    const char *adapter = NULL;
    const json_value *value = json_object_get(object, "adapter");
    if (value == NULL) return "";
    if (!json_get_string(value, &adapter)) return NULL;
    return adapter;
}

/* Reads the geometry: fixed `{n: 64}` or tiled `{rows, cols}`. */
static int read_geometry(const json_value *object, int *fixed, size_t *rows, size_t *cols, uint64_t *seed)
{
    uint64_t rows64, cols64, n;
    if (!json_get_u64(json_object_get(object, "seed"), seed)) return 0;
    if (json_get_u64(json_object_get(object, "rows"), &rows64) && json_get_u64(json_object_get(object, "cols"), &cols64)) {
        if (rows64 == 0 || cols64 == 0 || rows64 > SIZE_MAX / 2 || cols64 > SIZE_MAX / 2) return 0;
        *fixed = 0; *rows = (size_t)rows64; *cols = (size_t)cols64; return 1;
    }
    if (json_get_u64(json_object_get(object, "n"), &n) && n != 64) { fprintf(stderr, "fixed Bitshuffle transpose requires n=64\n"); return 0; }
    *fixed = 1; *rows = *cols = 64; return 1;
}

/* Canonical gf2 BitMatrix fill: one SplitMix64 draw per bit in row-major
 * order (tiled), or one draw per row word (fixed 64x64). */
static uint64_t *make_canonical(int fixed, size_t rows, size_t cols, size_t stride, uint64_t *state)
{
    uint64_t *words = (uint64_t *)calloc(rows * stride, sizeof(uint64_t));
    if (words == NULL) return NULL;
    if (fixed) { for (size_t r = 0; r < 64; r++) words[r] = splitmix64_next(state); return words; }
    for (size_t r = 0; r < rows; r++)
        for (size_t c = 0; c < cols; c++)
            if (splitmix64_next(state) & 1) words[r * stride + c / 64] |= UINT64_C(1) << (c % 64);
    return words;
}

static void pack_rows(bit_ctx *ctx, const uint64_t *in_words)
{
    for (size_t r = 0; r < ctx->rows; r++)
        memcpy(ctx->pad + r * ctx->elem, in_words + r * ctx->in_stride, ctx->elem);
}

static void unpack_planes(bit_ctx *ctx, uint64_t *out_words)
{
    size_t plane_bytes = ctx->rows_padded / 8, copy_bytes = (ctx->rows + 7) / 8;
    for (size_t c = 0; c < ctx->cols; c++)
        memcpy(out_words + c * ctx->out_stride, ctx->planes + c * plane_bytes, copy_bytes);
}

/* One whole-consumer call: fresh canonical output, pack when the layouts
 * differ, one Bitshuffle block, unpack when the layouts differ. */
static uint64_t *consumer_call(bit_ctx *ctx, const uint64_t *in_words)
{
    uint64_t *out = (uint64_t *)calloc(ctx->cols * ctx->out_stride, sizeof(uint64_t));
    const void *in = in_words; void *planes = out; int64_t result;
    if (out == NULL) { fprintf(stderr, "cannot allocate Bitshuffle consumer output\n"); abort(); }
    if (ctx->pack_needed) { pack_rows(ctx, in_words); in = ctx->pad; }
    if (ctx->unpack_needed) planes = ctx->planes;
    result = bshuf_bitshuffle(in, planes, ctx->rows_padded, ctx->elem, ctx->rows_padded);
    if (result < 0) { fprintf(stderr, "Bitshuffle call failed with error %lld\n", (long long)result); abort(); }
    if (ctx->unpack_needed) unpack_planes(ctx, out);
    return out;
}

static int geometry_available(size_t rows, const char *adapter)
{
    if (rows % 8 == 0 || !strcmp(adapter, "padded")) return 1;
    fprintf(stderr, "Bitshuffle transpose requires size (rows) multiple of 8; rows=%zu is unavailable without padding\n", rows);
    return 0;
}

static int init_geometry(bit_ctx *ctx, int fixed, size_t rows, size_t cols)
{
    ctx->fixed = fixed; ctx->rows = rows; ctx->cols = cols;
    ctx->in_stride = (cols + 63) / 64; ctx->out_stride = (rows + 63) / 64;
    ctx->rows_padded = (rows + 7) / 8 * 8; ctx->elem = (cols + 7) / 8;
    ctx->planes_bytes = ctx->rows_padded * ctx->elem;
    ctx->pack_needed = !(rows % 8 == 0 && cols % 64 == 0);
    ctx->unpack_needed = !(rows % 64 == 0 && cols % 8 == 0);
    ctx->pad = (unsigned char *)calloc(ctx->planes_bytes, 1);
    ctx->planes = (unsigned char *)calloc(ctx->planes_bytes, 1);
    return ctx->pad != NULL && ctx->planes != NULL;
}

static int dump_case(const json_value *object)
{
    bit_ctx ctx = {{0}, 0, 0, 0, 0, 1, 0, 0, 0, NULL, NULL, 0, 0, 0, NULL};
    int fixed; size_t rows, cols; uint64_t seed, state; uint64_t *in_words, *out;
    const char *adapter = case_adapter(object), *search = getenv("GF2_BITSHUFFLE_ADAPTER");
    if (adapter == NULL || !read_geometry(object, &fixed, &rows, &cols, &seed)) { fprintf(stderr, "invalid Bitshuffle transpose case\n"); return 0; }
    if (!geometry_available(rows, adapter)) return 0;
    if (!init_geometry(&ctx, fixed, rows, cols)) return 0;
    state = seed;
    in_words = make_canonical(fixed, rows, cols, ctx.in_stride, &state);
    if (in_words == NULL) return 0;
    if (search != NULL && *search != '\0') {
        /* Mapping search: apply the candidate byte/bit permutation to the
         * packed element bytes and to the produced planes. */
        unsigned char *bytes = (unsigned char *)calloc(ctx.planes_bytes, 1);
        if (bytes == NULL) return 0;
        ctx.pack_needed = 1; ctx.unpack_needed = 1;
        pack_rows(&ctx, in_words);
        memcpy(bytes, ctx.pad, ctx.planes_bytes);
        apply_input_adapter(bytes, ctx.planes_bytes, ctx.elem, search);
        if (bshuf_bitshuffle(bytes, ctx.planes, ctx.rows_padded, ctx.elem, ctx.rows_padded) < 0) { fprintf(stderr, "Bitshuffle cannot represent this geometry\n"); return 0; }
        apply_output_adapter(ctx.planes, ctx.planes_bytes, search);
        out = (uint64_t *)calloc(cols * ctx.out_stride, sizeof(uint64_t));
        if (out == NULL) return 0;
        unpack_planes(&ctx, out);
        free(bytes);
    } else {
        out = consumer_call(&ctx, in_words);
    }
    for (size_t c = 0; c < cols; c++) { for (size_t r = 0; r < rows; r++) putchar((out[c * ctx.out_stride + r / 64] >> (r % 64)) & 1 ? '1' : '0'); putchar('\n'); }
    free(out); free(in_words); free(ctx.pad); free(ctx.planes);
    return fflush(stdout) == 0;
}

static void fixed_body(void *opaque, uint64_t call_index)
{
    bit_ctx *ctx = (bit_ctx *)opaque;
    int64_t result = bshuf_bitshuffle(ctx->in_words[call_index % ctx->banks], ctx->fixed_out, 64, 8, 64);
    if (result < 0) { fprintf(stderr, "Bitshuffle timed call failed with error %lld\n", (long long)result); abort(); }
}

static void consumer_body(void *opaque, uint64_t call_index)
{
    bit_ctx *ctx = (bit_ctx *)opaque;
    free(consumer_call(ctx, ctx->in_words[call_index % ctx->banks]));
}

static const char *backend_name(void)
{
    return bshuf_using_AVX512() ? "avx512" : bshuf_using_AVX2() ? "avx2" : bshuf_using_SSE2() ? "sse2" : bshuf_using_NEON() ? "neon" : "scalar";
}

static int transport_case(const json_value *object, const char *cache, uint64_t windows, uint64_t target_ms)
{
    bit_ctx ctx = {{0}, 0, 0, 0, 0, 1, 0, 0, 0, NULL, NULL, 0, 0, 0, NULL};
    int fixed; size_t rows, cols; uint64_t seed, state, setup_start = monotonic_ns(), setup_ns, pack_ns = 0, unpack_ns = 0, start;
    harness_window *samples = NULL; char selected[160];
    const char *adapter = case_adapter(object);
    if (adapter == NULL || !read_geometry(object, &fixed, &rows, &cols, &seed)) { fprintf(stderr, "invalid Bitshuffle transpose case\n"); return 0; }
    if (!geometry_available(rows, adapter)) return 0;
    if (!init_geometry(&ctx, fixed, rows, cols)) { fprintf(stderr, "cannot allocate Bitshuffle adapter buffers\n"); return 0; }
    ctx.banks = strcmp(cache, "streaming") == 0 ? 8u : 1u;
    state = seed;
    for (size_t i = 0; i < ctx.banks; i++) {
        ctx.in_words[i] = make_canonical(fixed, rows, cols, ctx.in_stride, &state);
        if (fixed) state = seed + i + 1;
        if (ctx.in_words[i] == NULL) { fprintf(stderr, "cannot allocate Bitshuffle input bank\n"); return 0; }
    }
    if (fixed) { ctx.fixed_out = (uint64_t *)calloc(64, sizeof(uint64_t)); if (ctx.fixed_out == NULL) return 0; }
    setup_ns = monotonic_ns() - setup_start;
    if (!fixed) {
        /* One measured pass of each adapter copy, outside the timed loop. */
        if (ctx.pack_needed) { start = monotonic_ns(); pack_rows(&ctx, ctx.in_words[0]); pack_ns = monotonic_ns() - start; }
        if (ctx.unpack_needed) {
            uint64_t *probe = (uint64_t *)calloc(cols * ctx.out_stride, sizeof(uint64_t));
            if (probe == NULL) return 0;
            start = monotonic_ns(); unpack_planes(&ctx, probe); unpack_ns = monotonic_ns() - start; free(probe);
        }
    }
    if (!strcmp(cache, "warm")) for (size_t i = 0; i < ctx.banks; i++) { if (fixed) fixed_body(&ctx, i); else consumer_body(&ctx, i); }
    if (!run_windows(fixed ? fixed_body : consumer_body, &ctx, windows, target_ms, &samples, NULL)) { fprintf(stderr, "Bitshuffle timing failed\n"); return 0; }
    (void)snprintf(selected, sizeof(selected), "bitshuffle-bshuf_bitshuffle-%s%s%s%s", backend_name(),
                   fixed ? "" : "-consumer", (!fixed && ctx.pack_needed) ? "-pack" : "", (!fixed && ctx.unpack_needed) ? "-unpack" : "");
    if (!emit_result(samples, windows, cache, selected, !fixed, setup_ns, pack_ns, unpack_ns, 0, 0)) return 0;
    free(samples); free(ctx.fixed_out); free(ctx.pad); free(ctx.planes);
    for (size_t i = 0; i < ctx.banks; i++) free(ctx.in_words[i]);
    return 1;
}

int main(int argc, char **argv)
{
    char *input = NULL, *error = NULL; size_t length; json_value *root = NULL; const json_value *object; const char *cache; uint64_t windows, target_ms; int ok;
    if (argc == 2 && !strcmp(argv[1], "--backend")) {
        /* Compile-time backend selection as reported by the linked library's
         * own predicates; the disassembly probe in record-build-evidence.py
         * supplies the instruction-level observation. */
        printf("{\"library\":\"bitshuffle\",\"entrypoint\":\"bshuf_bitshuffle\",\"library_reported_backend\":\"%s\",\"compiled\":{\"avx512\":%s,\"avx2\":%s,\"sse2\":%s,\"neon\":%s},\"observation\":\"bshuf_using_* predicates of the linked archive\"}\n",
               backend_name(),
               bshuf_using_AVX512() ? "true" : "false", bshuf_using_AVX2() ? "true" : "false",
               bshuf_using_SSE2() ? "true" : "false", bshuf_using_NEON() ? "true" : "false");
        return 0;
    }
    if (argc == 3 && !strcmp(argv[1], "--dump-check")) { if (!parse_case_argument(argv[2], &root)) return 2; ok = dump_case(root); json_free(root); return ok ? 0 : 1; }
    if (argc != 1 || !require_child_mode() || !read_all(stdin, &input, &length) || !json_parse(input, length, &root, &error)) { fprintf(stderr, "bitshuffle_transpose_arm: invalid child-v2 request: %s\n", error ? error : "read or parse failed"); free(error); free(input); return 2; }
    free(input); if (!request_fields(root, &object, &cache, &windows, &target_ms)) { fprintf(stderr, "bitshuffle_transpose_arm: invalid child-v2 fields\n"); json_free(root); return 2; }
    ok = transport_case(object, cache, windows, target_ms); json_free(root); return ok ? 0 : 1;
}
