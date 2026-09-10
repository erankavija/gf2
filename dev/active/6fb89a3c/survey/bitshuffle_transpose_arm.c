/* Bitshuffle transpose comparison arm for jit:6fb89a3c.
 *
 * bshuf_bitshuffle(in, out, size, elem_size, block_size) with size=rows,
 * elem_size=ceil(cols/8), block_size=64 for the fixed 64x64 case (0,
 * library-default, otherwise) computes exactly gf2's 64x64 bit-block
 * transpose operation: bit `c` of input element `r` becomes bit `r` of
 * output "row" `c`, LSB-first, no adapter needed. This was confirmed by
 * probing the built library directly with single-bit inputs at element/bit
 * corners (0,0), (0,1), (1,0), (63,0), (0,63) and reading back which output
 * bit lit up, because raid.h's own doc comment does not spell out the
 * byte/bit packing order. The optional GF2_BITSHUFFLE_ADAPTER is dead
 * infrastructure kept for `--dump-check`'s adapter search in case a future
 * geometry ever needs one; the direct mapping is what both --dump-check and
 * the transport path use today.
 */
#include "harness_common.h"
#include <bitshuffle_core.h>

typedef struct { unsigned char *in[8], *out; size_t bytes, rows, cols, elem_size, block_size, banks; } bit_ctx;

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

static int build_input(const json_value *object, unsigned char **out, size_t *rows_out, size_t *cols_out, size_t *elem_out)
{
    uint64_t seed, rows64, cols64; size_t rows, cols, elem; unsigned char *buffer; uint64_t state;
    if (!json_get_u64(json_object_get(object, "seed"), &seed)) return 0;
    if (json_get_u64(json_object_get(object, "rows"), &rows64) && json_get_u64(json_object_get(object, "cols"), &cols64)) { rows = (size_t)rows64; cols = (size_t)cols64; }
    else { rows = cols = 64; }
    if (rows == 0 || cols == 0 || rows64 > SIZE_MAX || cols64 > SIZE_MAX) return 0;
    elem = (cols + 7) / 8;
    if (elem == 0 || rows > SIZE_MAX / elem) return 0;
    buffer = (unsigned char *)calloc(rows * elem, 1); if (buffer == NULL) return 0;
    state = seed;
    for (size_t r = 0; r < rows; r++) for (size_t c = 0; c < cols; c++) if (splitmix64_next(&state) & 1) buffer[r * elem + c / 8] |= (unsigned char)(1u << (c & 7));
    *out = buffer; *rows_out = rows; *cols_out = cols; *elem_out = elem; return 1;
}

static int make_input_case(const json_value *object, unsigned char **out, size_t *rows, size_t *cols, size_t *elem)
{
    uint64_t seed; if (!json_get_u64(json_object_get(object, "seed"), &seed)) return 0;
    if (json_object_get(object, "rows") == NULL) {
        unsigned char *buffer = (unsigned char *)malloc(64 * 8); if (buffer == NULL) return 0;
        for (size_t r = 0; r < 64; r++) { uint64_t word = splitmix64_next(&seed); for (size_t b = 0; b < 8; b++) buffer[r * 8 + b] = (unsigned char)(word >> (8 * b)); }
        *out = buffer; *rows = 64; *cols = 64; *elem = 8; return 1;
    }
    return build_input(object, out, rows, cols, elem);
}

static int bitshuffle_run(const unsigned char *input, unsigned char *output, size_t rows, size_t elem, size_t block_size, const char *adapter)
{
    size_t bytes = rows * elem; unsigned char *working = (unsigned char *)malloc(bytes); int result;
    if (working == NULL) return 0;
    memcpy(working, input, bytes);
    apply_input_adapter(working, bytes, elem, adapter);
    result = bshuf_bitshuffle(working, output, rows, elem, block_size); free(working);
    if (result < 0) { fprintf(stderr, "Bitshuffle cannot represent size=%zu elem_size=%zu block_size=%zu (error %d)\n", rows, elem, block_size, result); return 0; }
    apply_output_adapter(output, bytes, adapter); return 1;
}

static int dump_case(const json_value *object)
{
    unsigned char *input, *output; size_t rows, cols, elem; const char *adapter = getenv("GF2_BITSHUFFLE_ADAPTER");
    if (!make_input_case(object, &input, &rows, &cols, &elem)) { fprintf(stderr, "invalid Bitshuffle transpose case\n"); return 0; }
    /* Same restriction as transport_case: the plane byte-packing this
     * decoder (and the library's own block tiling) relies on needs `rows`
     * (the element count) a multiple of 8. A non-multiple size is unavailable
     * for Bitshuffle without padding -- recorded as such, not compared. */
    if (rows % 8 != 0) { fprintf(stderr, "Bitshuffle transpose requires rows/size multiple of 8; rows=%zu is unavailable without padding\n", rows); free(input); return 0; }
    output = (unsigned char *)malloc(rows * elem); if (output == NULL) { free(input); return 0; }
    if (!bitshuffle_run(input, output, rows, elem, rows == 64 && elem == 8 ? 64 : 0, adapter)) { free(input); free(output); return 0; }
    /* bshuf_bitshuffle's output is `size` (== rows) bit-planes of `rows/8`
     * bytes each, plane `c` holding bit `c` of every element, packed
     * LSB-first with element r at byte r/8 bit r%8 -- confirmed empirically
     * against the library (single-bit probes at element/bit corners) rather
     * than assumed from documentation, since the doc comment does not spell
     * out byte/bit packing order. This is exactly gf2's canonical
     * little-endian convention with no adapter needed. */
    for (size_t c = 0; c < cols; c++) { for (size_t r = 0; r < rows; r++) { unsigned char value = output[c * (rows / 8) + r / 8]; putchar((value >> (r & 7)) & 1 ? '1' : '0'); } putchar('\n'); }
    free(input); free(output); return fflush(stdout) == 0;
}

static void bitshuffle_body(void *opaque, uint64_t call_index)
{
    bit_ctx *ctx = (bit_ctx *)opaque; int result = bshuf_bitshuffle(ctx->in[call_index % ctx->banks], ctx->out, ctx->rows, ctx->elem_size, ctx->block_size);
    if (result < 0) { fprintf(stderr, "Bitshuffle timed call failed with error %d\n", result); abort(); }
}

static int transport_case(const json_value *object, const char *cache, uint64_t windows, uint64_t target_ms)
{
    bit_ctx ctx = {{0}, NULL, 0, 0, 0, 0, 0, strcmp(cache, "streaming") == 0 ? 8u : 1u};
    int fixed = json_object_get(object, "rows") == NULL;
    uint64_t setup_start = monotonic_ns(), setup_ns; uint64_t seed; size_t rows, cols, elem; unsigned char *first = NULL; harness_window *samples = NULL;
    if (!make_input_case(object, &first, &rows, &cols, &elem) || !json_get_u64(json_object_get(object, "seed"), &seed)) { fprintf(stderr, "invalid Bitshuffle transpose case\n"); return 0; }
    if (rows % 8 != 0) { fprintf(stderr, "Bitshuffle transpose requires rows/size multiple of 8; rows=%zu is unavailable without padding\n", rows); free(first); return 0; }
    ctx.rows = rows; ctx.cols = cols; ctx.elem_size = elem; ctx.bytes = rows * elem; ctx.block_size = fixed ? 64 : 0;
    for (size_t i = 0; i < ctx.banks; i++) { ctx.in[i] = (unsigned char *)malloc(ctx.bytes); if (ctx.in[i] == NULL) { fprintf(stderr, "cannot allocate Bitshuffle input bank\n"); return 0; } if (i == 0) memcpy(ctx.in[i], first, ctx.bytes); else { uint64_t s = seed + i; for (size_t j = 0; j < ctx.bytes; j++) ctx.in[i][j] = (unsigned char)splitmix64_next(&s); } }
    ctx.out = (unsigned char *)malloc(ctx.bytes); setup_ns = monotonic_ns() - setup_start; free(first);
    if (ctx.out == NULL) { fprintf(stderr, "cannot allocate Bitshuffle output\n"); return 0; }
    if (!strcmp(cache, "warm")) bitshuffle_body(&ctx, 0);
    if (!run_windows(bitshuffle_body, &ctx, windows, target_ms, &samples, NULL)) { fprintf(stderr, "Bitshuffle timing failed\n"); return 0; }
    const char *backend = bshuf_using_AVX512() ? "avx512" : bshuf_using_AVX2() ? "avx2" : bshuf_using_SSE2() ? "sse2" : bshuf_using_NEON() ? "neon" : "scalar";
    char selected[128];
    (void)snprintf(selected, sizeof(selected), "bitshuffle-bshuf_bitshuffle-%s%s", backend, fixed ? "" : "-tiled");
    if (!emit_result(samples, windows, cache, selected, !fixed, setup_ns, 0, 0, 0, 0)) return 0;
    free(samples); free(ctx.out); for (size_t i = 0; i < ctx.banks; i++) free(ctx.in[i]); return 1;
}

int main(int argc, char **argv)
{
    char *input = NULL, *error = NULL; size_t length; json_value *root = NULL; const json_value *object; const char *cache; uint64_t windows, target_ms; int ok;
    if (argc == 2 && !strcmp(argv[1], "--backend")) {
        printf("{\"library\":\"bitshuffle\",\"entrypoint\":\"bshuf_bitshuffle\",\"selected_backend\":\"%s\",\"compiled\":{\"avx512\":%s,\"avx2\":%s,\"sse2\":%s,\"neon\":%s}}\n",
               bshuf_using_AVX512() ? "avx512" : bshuf_using_AVX2() ? "avx2" : bshuf_using_SSE2() ? "sse2" : bshuf_using_NEON() ? "neon" : "scalar",
               bshuf_using_AVX512() ? "true" : "false", bshuf_using_AVX2() ? "true" : "false",
               bshuf_using_SSE2() ? "true" : "false", bshuf_using_NEON() ? "true" : "false");
        return 0;
    }
    if (argc == 3 && !strcmp(argv[1], "--dump-check")) { if (!parse_case_argument(argv[2], &root)) return 2; ok = dump_case(root); json_free(root); return ok ? 0 : 1; }
    if (argc != 1 || !require_child_mode() || !read_all(stdin, &input, &length) || !json_parse(input, length, &root, &error)) { fprintf(stderr, "bitshuffle_transpose_arm: invalid child-v2 request: %s\n", error ? error : "read or parse failed"); free(error); free(input); return 2; }
    free(input); if (!request_fields(root, &object, &cache, &windows, &target_ms)) { fprintf(stderr, "bitshuffle_transpose_arm: invalid child-v2 fields\n"); json_free(root); return 2; }
    ok = transport_case(object, cache, windows, target_ms); json_free(root); return ok ? 0 : 1;
}
