/* M4RI transpose comparison arm for jit:6fb89a3c. */
#include "harness_common.h"
#include <m4ri/m4ri.h>

typedef struct {
    mzd_t *a[8];
    mzd_t *out[8];
    uint64_t banks, rows, cols;
    int include_allocation;
} transpose_ctx;

static int case_u64(const json_value *object, const char *key, uint64_t *out)
{
    return json_get_u64(json_object_get(object, key), out);
}

static mzd_t *make_matrix(uint64_t rows, uint64_t cols, uint64_t *seed)
{
    mzd_t *matrix;
    if (rows == 0 || cols == 0 || rows > (uint64_t)SIZE_MAX || cols > (uint64_t)SIZE_MAX) return NULL;
    matrix = mzd_init((rci_t)rows, (rci_t)cols);
    if (matrix == NULL) return NULL;
    mzd_set_ui(matrix, 0);
    for (uint64_t r = 0; r < rows; r++)
        for (uint64_t c = 0; c < cols; c++)
            if (splitmix64_next(seed) & 1) mzd_write_bit(matrix, (rci_t)r, (rci_t)c, 1);
    return matrix;
}

static mzd_t *make_fixed(uint64_t seed)
{
    mzd_t *matrix = mzd_init(64, 64);
    if (matrix == NULL) return NULL;
    for (rci_t r = 0; r < 64; r++) mzd_row(matrix, r)[0] = (word)splitmix64_next(&seed);
    return matrix;
}

static void transpose_body(void *opaque, uint64_t call_index)
{
    transpose_ctx *ctx = (transpose_ctx *)opaque;
    uint64_t bank = call_index % ctx->banks;
    mzd_t *out = mzd_transpose(ctx->include_allocation ? NULL : ctx->out[bank], ctx->a[bank]);
    if (out == NULL) { fprintf(stderr, "mzd_transpose returned NULL\n"); abort(); }
    if (ctx->include_allocation) mzd_free(out);
}

static int print_matrix(const mzd_t *matrix)
{
    for (rci_t r = 0; r < matrix->nrows; r++) {
        for (rci_t c = 0; c < matrix->ncols; c++) putchar(mzd_read_bit(matrix, r, c) ? '1' : '0');
        putchar('\n');
    }
    return fflush(stdout) == 0;
}

static int dump_case(const json_value *object)
{
    uint64_t seed, rows, cols;
    mzd_t *input, *output;
    if (!case_u64(object, "seed", &seed)) { fprintf(stderr, "transpose case requires uint64 seed\n"); return 0; }
    if (case_u64(object, "rows", &rows) && case_u64(object, "cols", &cols)) input = make_matrix(rows, cols, &seed);
    else input = make_fixed(seed);
    if (input == NULL) { fprintf(stderr, "cannot allocate transpose input\n"); return 0; }
    output = mzd_transpose(NULL, input);
    if (output == NULL) { fprintf(stderr, "mzd_transpose returned NULL\n"); mzd_free(input); return 0; }
    print_matrix(output);
    mzd_free(output); mzd_free(input);
    return 1;
}

static int transport_case(const json_value *object, const char *cache, uint64_t windows, uint64_t target_ms)
{
    uint64_t seed, rows, cols, setup_start, setup_ns;
    int tiled = case_u64(object, "rows", &rows) && case_u64(object, "cols", &cols);
    transpose_ctx ctx = {{0}, {0}, strcmp(cache, "streaming") == 0 ? 8u : 1u, 0, 0, tiled};
    harness_window *samples = NULL;
    if (!case_u64(object, "seed", &seed)) { fprintf(stderr, "transpose case requires uint64 seed\n"); return 0; }
    if (tiled && (rows == 0 || cols == 0)) { fprintf(stderr, "transpose dimensions must be nonzero\n"); return 0; }
    if (!tiled) rows = cols = 64;
    ctx.rows = rows; ctx.cols = cols;
    setup_start = monotonic_ns();
    uint64_t state = seed;
    for (uint64_t i = 0; i < ctx.banks; i++) {
        ctx.a[i] = tiled ? make_matrix(rows, cols, &state) : make_fixed(state);
        if (!tiled) state = seed + i + 1;
        if (ctx.a[i] == NULL) { fprintf(stderr, "cannot allocate transpose working matrix\n"); for (uint64_t j = 0; j < i; j++) mzd_free(ctx.a[j]); return 0; }
        if (!tiled) {
            ctx.out[i] = mzd_init((rci_t)cols, (rci_t)rows);
            if (ctx.out[i] == NULL) { fprintf(stderr, "cannot allocate transpose output matrix\n"); return 0; }
        }
    }
    setup_ns = monotonic_ns() - setup_start;
    if (!strcmp(cache, "warm")) for (uint64_t i = 0; i < ctx.banks; i++) { mzd_t *out = mzd_transpose(NULL, ctx.a[i]); if (out == NULL) { fprintf(stderr, "warm-up transpose failed\n"); return 0; } mzd_free(out); }
    if (!run_windows(transpose_body, &ctx, windows, target_ms, &samples, NULL)) { fprintf(stderr, "transpose timing failed\n"); return 0; }
    if (!emit_result(samples, windows, cache, tiled ? "m4ri-mzd_transpose-tiled" : "m4ri-mzd_transpose", tiled, setup_ns, 0, 0, 0, 0)) return 0;
    free(samples);
    for (uint64_t i = 0; i < ctx.banks; i++) { mzd_free(ctx.a[i]); if (ctx.out[i] != NULL) mzd_free(ctx.out[i]); }
    return 1;
}

int main(int argc, char **argv)
{
    char *input = NULL, *error = NULL; size_t length; json_value *root = NULL; const json_value *object; const char *cache; uint64_t windows, target_ms;
    int ok;
    if (argc == 2 && !strcmp(argv[1], "--backend")) {
        /* Compile-time facts of the linked library; the instruction-level
         * observation of mzd_transpose and its absence of runtime dispatch
         * come from the disassembly probe in record-build-evidence.py. */
        printf("{\"library\":\"m4ri\",\"entrypoint\":\"mzd_transpose\",\"version\":\"%s\",\"compiled_sse2\":%s,\"observation\":\"m4ri_config.h macros of the linked build\"}\n",
               M4RI_VERSION_STR,
#if __M4RI_HAVE_SSE2
               "true"
#else
               "false"
#endif
        );
        return 0;
    }
    if (argc == 3 && !strcmp(argv[1], "--dump-check")) { if (!parse_case_argument(argv[2], &root)) return 2; ok = dump_case(root); json_free(root); return ok ? 0 : 1; }
    if (argc != 1 || !require_child_mode() || !read_all(stdin, &input, &length) || !json_parse(input, length, &root, &error)) { fprintf(stderr, "m4ri_transpose_arm: invalid child-v2 request: %s\n", error ? error : "read or parse failed"); free(error); free(input); return 2; }
    free(input);
    if (!request_fields(root, &object, &cache, &windows, &target_ms)) { fprintf(stderr, "m4ri_transpose_arm: request lacks valid case/cache/timing fields\n"); json_free(root); return 2; }
    ok = transport_case(object, cache, windows, target_ms); json_free(root); return ok ? 0 : 1;
}
