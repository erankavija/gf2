/* M4RI BCH generator-matrix arm for jit:6fb89a3c.
 *
 * Conversion timing is outside the windowed loop: setup_ns measures mzd_init
 * plus the shifted-polynomial fill; pack_ns and dispatch_ns are measured once
 * on representative fresh copies. The timed operation intentionally includes
 * mzd_copy followed by mzd_echelonize_m4ri, because each call must reduce a
 * fresh copy and the copy allocation cannot be moved out of that operation.
 */
#include "harness_common.h"
#include <m4ri/m4ri.h>

/* Reuses the already-committed generator-polynomial dump from issue
 * 4e732b56's survey (jit:4e732b56, dev/bench_results/4e732b56/generators.txt)
 * rather than recomputing or duplicating it (@/inv/single-source-prose): its
 * B1/B2/B3 rows (n/k/deg-g = 15/5/10, 127/64/63, 255/223/32) are the exact
 * same codes crates/gf2-coding/benches/bch_genmatrix.rs's ROWS table builds.
 * That file is read-only input here; this issue's directory owns none of it. */
#define GENERATORS_PATH "dev/bench_results/4e732b56/generators.txt"
#define MAX_DEGREE 4095

typedef struct { char name[32]; int n, k, deg; unsigned char g[MAX_DEGREE + 1]; } code_info;
typedef struct { mzd_t *base[8]; uint64_t banks; const code_info *code; } gen_ctx;

static int load_code(const char *wanted, code_info *out)
{
    FILE *file = fopen(GENERATORS_PATH, "r");
    char name[32], bits[MAX_DEGREE + 2]; int n, k, deg;
    if (file == NULL) { fprintf(stderr, "m4ri_genmatrix_arm: cannot open %s; generators.txt is required\n", GENERATORS_PATH); return 0; }
    while (fscanf(file, "%31s %d %d %d %4096s", name, &n, &k, &deg, bits) == 5) {
        size_t length = strlen(bits);
        if (length != (size_t)deg + 1 || deg < 0 || deg > MAX_DEGREE || n <= 0 || k <= 0 || k > n) continue;
        if (strcmp(name, wanted) != 0) continue;
        memset(out, 0, sizeof(*out)); (void)snprintf(out->name, sizeof(out->name), "%s", name); out->n = n; out->k = k; out->deg = deg;
        for (size_t i = 0; i < length; i++) { if (bits[i] != '0' && bits[i] != '1') { fclose(file); return 0; } out->g[i] = (unsigned char)(bits[i] == '1'); }
        fclose(file); return 1;
    }
    fclose(file); fprintf(stderr, "m4ri_genmatrix_arm: code %s is absent from %s\n", wanted, GENERATORS_PATH); return 0;
}

/* Copied/adapted exactly from baseline-survey/m4ri_genmatrix_bench.c:
 * fill_repo_form followed by mzd_echelonize_m4ri(G, 1, 0). */
static void fill_repo_form(mzd_t *matrix, const code_info *code)
{
    mzd_set_ui(matrix, 0);
    for (rci_t i = 0; i < matrix->nrows; i++)
        for (int j = 0; j <= code->deg; j++)
            if (code->g[j]) mzd_write_bit(matrix, i, (rci_t)(code->n - 1 - (i + j)), 1);
}

/* Exact established algorithm from m4ri_genmatrix_bench.c. */
static void genmatrix_rref(mzd_t *matrix, const code_info *code)
{
    fill_repo_form(matrix, code);
    mzd_echelonize_m4ri(matrix, 1, 0);
}

static void gen_body(void *opaque, uint64_t call_index)
{
    gen_ctx *ctx = (gen_ctx *)opaque;
    mzd_t *work = mzd_copy(NULL, ctx->base[call_index % ctx->banks]);
    if (work == NULL) { fprintf(stderr, "mzd_copy failed in genmatrix timing\n"); abort(); }
    mzd_echelonize_m4ri(work, 1, 0);
    mzd_free(work);
}

static int dump_code(const code_info *code)
{
    mzd_t *matrix = mzd_init((rci_t)code->k, (rci_t)code->n);
    if (matrix == NULL) { fprintf(stderr, "m4ri_genmatrix_arm: cannot allocate generator matrix\n"); return 0; }
    genmatrix_rref(matrix, code);
    for (rci_t r = 0; r < matrix->nrows; r++) { for (rci_t c = 0; c < matrix->ncols; c++) putchar(mzd_read_bit(matrix, r, c) ? '1' : '0'); putchar('\n'); }
    mzd_free(matrix); return fflush(stdout) == 0;
}

static int transport_code(const code_info *code, const char *cache, uint64_t windows, uint64_t target_ms)
{
    gen_ctx ctx = {{0}, strcmp(cache, "streaming") == 0 ? 8u : 1u, code};
    uint64_t setup_start = monotonic_ns(), setup_ns, pack_ns, dispatch_ns, start;
    harness_window *samples = NULL; mzd_t *probe;
    for (uint64_t i = 0; i < ctx.banks; i++) {
        ctx.base[i] = mzd_init((rci_t)code->k, (rci_t)code->n);
        if (ctx.base[i] == NULL) { fprintf(stderr, "m4ri_genmatrix_arm: mzd_init failed\n"); return 0; }
        fill_repo_form(ctx.base[i], code);
    }
    setup_ns = monotonic_ns() - setup_start;
    if (!strcmp(cache, "warm")) for (uint64_t i = 0; i < ctx.banks; i++) gen_body(&ctx, i);
    start = monotonic_ns(); probe = mzd_copy(NULL, ctx.base[0]); pack_ns = monotonic_ns() - start;
    if (probe == NULL) { fprintf(stderr, "m4ri_genmatrix_arm: pack probe failed\n"); return 0; }
    start = monotonic_ns(); mzd_echelonize_m4ri(probe, 1, 0); dispatch_ns = monotonic_ns() - start; mzd_free(probe);
    if (!run_windows(gen_body, &ctx, windows, target_ms, &samples, NULL)) { fprintf(stderr, "m4ri_genmatrix_arm: timing failed\n"); return 0; }
    if (!emit_result(samples, windows, cache, "m4ri-genmatrix-rref", 1, setup_ns, pack_ns, 0, 0, dispatch_ns)) return 0;
    free(samples); for (uint64_t i = 0; i < ctx.banks; i++) mzd_free(ctx.base[i]); return 1;
}

int main(int argc, char **argv)
{
    char *input = NULL, *error = NULL; size_t length; json_value *root = NULL; const json_value *object; const char *cache, *name; uint64_t windows, target_ms; code_info code; int ok;
    if (argc == 2 && !strcmp(argv[1], "--backend")) {
        puts("{\"library\":\"m4ri\",\"entrypoint\":\"mzd_echelonize_m4ri\",\"selected_backend\":\"m4ri-rref\",\"runtime_dispatch\":false}");
        return 0;
    }
    if (argc == 3 && !strcmp(argv[1], "--dump-check")) {
        if (!parse_case_argument(argv[2], &root)) return 2;
        if (!json_get_string(json_object_get(root, "code"), &name) || !load_code(name, &code)) { json_free(root); return 2; }
        ok = dump_code(&code); json_free(root); return ok ? 0 : 1;
    }
    if (argc != 1 || !require_child_mode() || !read_all(stdin, &input, &length) || !json_parse(input, length, &root, &error)) { fprintf(stderr, "m4ri_genmatrix_arm: invalid child-v2 request: %s\n", error ? error : "read or parse failed"); free(error); free(input); return 2; }
    free(input);
    if (!request_fields(root, &object, &cache, &windows, &target_ms) || !json_get_string(json_object_get(object, "code"), &name) || !load_code(name, &code)) { json_free(root); return 2; }
    ok = transport_code(&code, cache, windows, target_ms); json_free(root); return ok ? 0 : 1;
}
