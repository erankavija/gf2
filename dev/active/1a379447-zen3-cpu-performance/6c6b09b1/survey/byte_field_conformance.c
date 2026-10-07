/*
 * byte_field_conformance.c — adapter validation for the byte-field survey
 * (jit:6c6b09b1).
 *
 * Every adapter is checked here before any timed run happens. The oracle is
 * `bfx_ref_mul`, a shift-and-reduce scalar multiply written inside the shim
 * that calls into no external library, so an adapter and its reference share
 * no code path.
 *
 * Checks, in order:
 *   1. Full multiplication table: all 65536 ordered pairs, per backend.
 *   2. Region multiply-XOR against a scalar accumulate loop, over the byte
 *      boundary lengths and several coefficients including 0 and 1.
 *   3. Region multiply-assign against a scalar loop.
 *   4. Arbitrary pairwise multiplication: each backend's single-element
 *      multiply applied per byte.
 *   5. M4RIE dense matmul at the square dimensions and the generator shape
 *      the cells measure, against a triple loop over the reference's product
 *      table.
 *   6. ISA-L generator encode at the cells' shape, against the same loop.
 *   7. Field distinctness: 0x11B and 0x11D disagree, so an adapter pinned to
 *      one polynomial is never compared against an adapter pinned to the
 *      other.
 *
 * Exit status is 0 when every check passes and 1 after any mismatch, with
 * each mismatch printed to stderr. Every check that a backend does not offer
 * is printed as an explicit `unsupported` line rather than skipped silently.
 *
 * Usage: ./byte_field_conformance
 */

#include "byte_field_ext.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define POLY_11D 0x11Du
#define POLY_11B 0x11Bu

static int failures = 0;
static int checks = 0;

static void fail(const char *what, const char *detail)
{
    fprintf(stderr, "FAIL %s: %s\n", what, detail);
    failures += 1;
}

static void pass(const char *what)
{
    checks += 1;
    printf("ok        %s\n", what);
}

static void unsupported(const char *what, const char *reason)
{
    checks += 1;
    printf("unsupported %s: %s\n", what, reason);
}

/* Deterministic byte stream so a failure is reproducible from the seed. */
static unsigned long long rng_state;
static unsigned char next_byte(void)
{
    rng_state += 0x9E3779B97F4A7C15ull;
    unsigned long long z = rng_state;
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
    return (unsigned char)((z ^ (z >> 31)) & 0xFFu);
}

static int check_table(bfx_ctx *ctx, const char *label, unsigned int poly)
{
    char detail[160];
    for (unsigned a = 0; a < 256; ++a) {
        for (unsigned b = 0; b < 256; ++b) {
            unsigned char got = bfx_mul_scalar(ctx, (unsigned char)a, (unsigned char)b);
            unsigned char want = bfx_ref_mul((unsigned char)a, (unsigned char)b, poly);
            if (got != want) {
                snprintf(detail, sizeof detail, "%u * %u = %u, reference %u", a, b, got, want);
                fail(label, detail);
                return 1;
            }
        }
    }
    pass(label);
    return 0;
}

static int check_axpy(bfx_ctx *ctx, const char *label, unsigned int poly)
{
    /* Byte-region analogues of the word boundary cases, the smallest
       lengths ISA-L's region kernels accept, and the 4 KiB and 128 KiB
       region lengths the cells measure. */
    static const size_t lengths[] = { 0, 1, 31, 32, 63, 64, 65, 127, 128, 4096, 131072 };
    static const unsigned char coefficients[] = { 0, 1, 2, 3, 0x53, 0xFF };
    unsigned char *src = malloc(131072);
    unsigned char *dest = malloc(131072);
    unsigned char *want = malloc(131072);
    char detail[200];
    int reported_short = 0;
    if (src == NULL || dest == NULL || want == NULL) {
        fail(label, "allocation");
        free(src);
        free(dest);
        free(want);
        return 1;
    }
    for (size_t li = 0; li < sizeof lengths / sizeof lengths[0]; ++li) {
        size_t len = lengths[li];
        for (size_t ci = 0; ci < sizeof coefficients / sizeof coefficients[0]; ++ci) {
            unsigned char a = coefficients[ci];
            for (size_t i = 0; i < len; ++i) {
                src[i] = next_byte();
                dest[i] = next_byte();
                want[i] = (unsigned char)(dest[i] ^ bfx_ref_mul(a, src[i], poly));
            }
            if (bfx_prepare(ctx, a) != BFX_OK) {
                fail(label, "prepare");
                goto done;
            }
            int status = bfx_axpy_apply(ctx, src, dest, len);
            if (status == BFX_ERR_UNSUPPORTED) {
                unsupported(label,
                            "the backend has no free-standing region API; its row form is "
                            "checked separately");
                goto done;
            }
            if (status == BFX_ERR_INVALID) {
                /* A length the backend documents as out of range. Record it
                   once rather than treating it as a correctness failure. */
                if (!reported_short) {
                    snprintf(detail, sizeof detail,
                             "region lengths below the backend minimum are rejected (len=%zu)",
                             len);
                    printf("note      %s: %s\n", label, detail);
                    reported_short = 1;
                }
                continue;
            }
            if (status != BFX_OK) {
                snprintf(detail, sizeof detail, "axpy returned %d at len=%zu", status, len);
                fail(label, detail);
                goto done;
            }
            if (len > 0 && memcmp(dest, want, len) != 0) {
                size_t at = 0;
                while (at < len && dest[at] == want[at]) {
                    at += 1;
                }
                snprintf(detail, sizeof detail,
                         "coefficient %u len %zu byte %zu: got %u, reference %u", a, len, at,
                         dest[at], want[at]);
                fail(label, detail);
                goto done;
            }
        }
    }
    pass(label);
done:
    free(src);
    free(dest);
    free(want);
    return failures;
}

/*
 * M4RIE spells region multiply-XOR as a matrix row operation, so its axpy
 * equivalent is validated through the matrix type against the same scalar
 * oracle the free-standing adapters use.
 */
static int check_row_axpy(bfx_ctx *ctx, const char *label, unsigned int poly, size_t len)
{
    unsigned char *src = malloc(len);
    unsigned char *dest = malloc(len);
    unsigned char *want = malloc(len);
    unsigned char *got = malloc(len);
    char detail[200];
    bfx_mat *ms = bfx_mat_new(ctx, 1, len);
    bfx_mat *md = bfx_mat_new(ctx, 1, len);
    unsigned char a = 0x53u;
    if (ms == NULL || md == NULL) {
        unsupported(label, "the backend carries no dense GF(2^8) matrix type");
        goto cleanup;
    }
    if (src == NULL || dest == NULL || want == NULL || got == NULL) {
        fail(label, "allocation");
        goto cleanup;
    }
    for (size_t i = 0; i < len; ++i) {
        src[i] = next_byte();
        dest[i] = next_byte();
        want[i] = (unsigned char)(dest[i] ^ bfx_ref_mul(a, src[i], poly));
    }
    bfx_mat_pack(ms, src);
    bfx_mat_pack(md, dest);
    if (bfx_mat_row_axpy(md, 0, ms, 0, a) != BFX_OK) {
        fail(label, "row axpy failed");
        goto cleanup;
    }
    bfx_mat_unpack(md, got);
    for (size_t i = 0; i < len; ++i) {
        if (got[i] != want[i]) {
            snprintf(detail, sizeof detail, "byte %zu: got %u, reference %u", i, got[i], want[i]);
            fail(label, detail);
            goto cleanup;
        }
    }
    pass(label);
cleanup:
    bfx_mat_free(ms);
    bfx_mat_free(md);
    free(src);
    free(dest);
    free(want);
    free(got);
    return failures;
}

static int check_mul_region(bfx_ctx *ctx, const char *label, unsigned int poly)
{
    const size_t len = 4096;
    unsigned char *src = malloc(len);
    unsigned char *dest = malloc(len);
    char detail[200];
    if (src == NULL || dest == NULL) {
        fail(label, "allocation");
        free(src);
        free(dest);
        return 1;
    }
    unsigned char a = 0x8Du;
    for (size_t i = 0; i < len; ++i) {
        src[i] = next_byte();
    }
    if (bfx_prepare(ctx, a) != BFX_OK || bfx_mul_region_apply(ctx, src, dest, len) != BFX_OK) {
        unsupported(label, "the backend offers no region multiply-assign");
        free(src);
        free(dest);
        return 0;
    }
    for (size_t i = 0; i < len; ++i) {
        unsigned char want = bfx_ref_mul(a, src[i], poly);
        if (dest[i] != want) {
            snprintf(detail, sizeof detail, "byte %zu: got %u, reference %u", i, dest[i], want);
            fail(label, detail);
            free(src);
            free(dest);
            return 1;
        }
    }
    pass(label);
    free(src);
    free(dest);
    return 0;
}

static int check_pairwise(bfx_ctx *ctx, const char *label, unsigned int poly)
{
    const size_t len = 4096;
    unsigned char *x = malloc(len);
    unsigned char *y = malloc(len);
    unsigned char *dest = malloc(len);
    char detail[200];
    if (x == NULL || y == NULL || dest == NULL) {
        fail(label, "allocation");
        free(x);
        free(y);
        free(dest);
        return 1;
    }
    for (size_t i = 0; i < len; ++i) {
        x[i] = next_byte();
        y[i] = next_byte();
    }
    int status = bfx_mul_pairwise(ctx, x, y, dest, len);
    if (status == BFX_ERR_UNSUPPORTED) {
        unsupported(label, "the backend's region API assumes one reused coefficient");
        free(x);
        free(y);
        free(dest);
        return 0;
    }
    if (status != BFX_OK) {
        fail(label, "pairwise multiply failed");
        free(x);
        free(y);
        free(dest);
        return 1;
    }
    for (size_t i = 0; i < len; ++i) {
        unsigned char want = bfx_ref_mul(x[i], y[i], poly);
        if (dest[i] != want) {
            snprintf(detail, sizeof detail, "byte %zu: got %u, reference %u", i, dest[i], want);
            fail(label, detail);
            free(x);
            free(y);
            free(dest);
            return 1;
        }
    }
    pass(label);
    free(x);
    free(y);
    free(dest);
    return 0;
}

/* Product table of the independent reference, so the large matrix checks
   below cost one lookup per multiply-accumulate. */
static unsigned char ref_table[256][256];
static unsigned int ref_table_poly = 0;

static void build_ref_table(unsigned int poly)
{
    if (ref_table_poly == poly) {
        return;
    }
    for (unsigned a = 0; a < 256; ++a) {
        for (unsigned b = 0; b < 256; ++b) {
            ref_table[a][b] = bfx_ref_mul((unsigned char)a, (unsigned char)b, poly);
        }
    }
    ref_table_poly = poly;
}

/* Dense product c = a * b with a of shape rows x inner and b of shape
   inner x cols, at the square and generator-encode shapes the cells use. */
static int check_matmul(bfx_ctx *ctx, const char *label, unsigned int poly, size_t rows,
                        size_t inner, size_t cols)
{
    unsigned char *a = malloc(rows * inner);
    unsigned char *b = malloc(inner * cols);
    unsigned char *c = malloc(rows * cols);
    unsigned char *want = calloc(rows * cols, 1);
    char detail[200];
    bfx_mat *ma = bfx_mat_new(ctx, rows, inner);
    bfx_mat *mb = bfx_mat_new(ctx, inner, cols);
    bfx_mat *mc = bfx_mat_new(ctx, rows, cols);
    if (ma == NULL || mb == NULL || mc == NULL) {
        unsupported(label, "the backend carries no dense GF(2^8) matrix type");
        goto cleanup;
    }
    if (a == NULL || b == NULL || c == NULL || want == NULL) {
        fail(label, "allocation");
        goto cleanup;
    }
    for (size_t i = 0; i < rows * inner; ++i) {
        a[i] = next_byte();
    }
    for (size_t i = 0; i < inner * cols; ++i) {
        b[i] = next_byte();
    }
    build_ref_table(poly);
    for (size_t i = 0; i < rows; ++i) {
        for (size_t k = 0; k < inner; ++k) {
            const unsigned char *row = ref_table[a[i * inner + k]];
            for (size_t j = 0; j < cols; ++j) {
                want[i * cols + j] ^= row[b[k * cols + j]];
            }
        }
    }
    bfx_mat_pack(ma, a);
    bfx_mat_pack(mb, b);
    if (bfx_mat_mul(mc, ma, mb) != BFX_OK) {
        fail(label, "matmul failed");
        goto cleanup;
    }
    bfx_mat_unpack(mc, c);
    for (size_t i = 0; i < rows * cols; ++i) {
        if (c[i] != want[i]) {
            snprintf(detail, sizeof detail, "entry %zu: got %u, reference %u", i, c[i], want[i]);
            fail(label, detail);
            goto cleanup;
        }
    }
    /* Round-tripping the packed layout must preserve every element, so a
       conversion cost is a pure representation change and nothing else. */
    unsigned char *back = malloc(rows * inner);
    if (back == NULL) {
        fail(label, "allocation");
        goto cleanup;
    }
    bfx_mat_unpack(ma, back);
    int same = memcmp(a, back, rows * inner) == 0;
    free(back);
    if (!same) {
        fail(label, "pack/unpack round trip changed the matrix");
        goto cleanup;
    }
    pass(label);
cleanup:
    bfx_mat_free(ma);
    bfx_mat_free(mb);
    bfx_mat_free(mc);
    free(a);
    free(b);
    free(c);
    free(want);
    return failures;
}

static int check_encode(bfx_ctx *ctx, const char *label, unsigned int poly, int k, int rows,
                        int len)
{
    unsigned char *g = malloc((size_t)(k * rows));
    unsigned char *flat = malloc((size_t)(k * len));
    unsigned char *out = malloc((size_t)(rows * len));
    unsigned char *want = malloc((size_t)(rows * len));
    unsigned char **data = malloc(sizeof(unsigned char *) * (size_t)k);
    unsigned char **coding = malloc(sizeof(unsigned char *) * (size_t)rows);
    char detail[200];
    if (g == NULL || flat == NULL || out == NULL || want == NULL || data == NULL ||
        coding == NULL) {
        fail(label, "allocation");
        goto cleanup;
    }
    for (int i = 0; i < k * rows; ++i) {
        g[i] = next_byte();
    }
    for (int i = 0; i < k * len; ++i) {
        flat[i] = next_byte();
    }
    for (int j = 0; j < k; ++j) {
        data[j] = flat + (size_t)j * (size_t)len;
    }
    for (int r = 0; r < rows; ++r) {
        coding[r] = out + (size_t)r * (size_t)len;
    }
    build_ref_table(poly);
    for (int r = 0; r < rows; ++r) {
        for (int i = 0; i < len; ++i) {
            unsigned char acc = 0;
            for (int j = 0; j < k; ++j) {
                acc ^= ref_table[g[r * k + j]][data[j][i]];
            }
            want[(size_t)r * (size_t)len + (size_t)i] = acc;
        }
    }
    if (bfx_encode_prepare(ctx, g, k, rows) == BFX_ERR_UNSUPPORTED) {
        unsupported(label, "the backend offers no generator-matrix region encode");
        goto cleanup;
    }
    if (bfx_encode_apply(ctx, len, k, rows, data, coding) != BFX_OK) {
        fail(label, "encode failed");
        goto cleanup;
    }
    for (int i = 0; i < rows * len; ++i) {
        if (out[i] != want[i]) {
            snprintf(detail, sizeof detail, "entry %d: got %u, reference %u", i, out[i], want[i]);
            fail(label, detail);
            goto cleanup;
        }
    }
    pass(label);
cleanup:
    free(g);
    free(flat);
    free(out);
    free(want);
    free(data);
    free(coding);
    return failures;
}

/*
 * 0x11B and 0x11D define non-equal fields on the same byte carrier. This
 * check records a witness pair so no later cell silently compares an
 * adapter fixed at one polynomial against an adapter fixed at the other.
 */
static void check_field_distinctness(void)
{
    for (unsigned a = 2; a < 256; ++a) {
        for (unsigned b = 2; b < 256; ++b) {
            unsigned char at_11d = bfx_ref_mul((unsigned char)a, (unsigned char)b, POLY_11D);
            unsigned char at_11b = bfx_ref_mul((unsigned char)a, (unsigned char)b, POLY_11B);
            if (at_11d != at_11b) {
                printf("ok        0x11B and 0x11D are distinct fields: %u * %u is %u under 0x11D "
                       "and %u under 0x11B\n",
                       a, b, at_11d, at_11b);
                checks += 1;
                return;
            }
        }
    }
    fail("field-distinctness", "0x11B and 0x11D agreed on every product, which cannot happen");
}

static bfx_ctx *open_backend(int backend, unsigned int poly, const char *variant,
                             const char *label)
{
    const char *why = NULL;
    bfx_ctx *ctx = bfx_init(backend, poly, variant, &why);
    if (ctx == NULL) {
        printf("unsupported %s: %s\n", label, why == NULL ? "unknown reason" : why);
        checks += 1;
        return NULL;
    }
    printf("backend   %s -> %s (%s)\n", label, bfx_backend_name(ctx), bfx_library_version(ctx));
    return ctx;
}

int main(void)
{
    rng_state = 0x6C6B09B1ull;

    struct {
        int backend;
        const char *variant;
        const char *label;
    } cases[] = {
        { BFX_ISAL, "default", "isa-l" },
        { BFX_GFCOMPLETE, "default", "gf-complete-default" },
        { BFX_GFCOMPLETE, "split-table-simd", "gf-complete-split-table-simd" },
        { BFX_GFCOMPLETE, "table", "gf-complete-table" },
        { BFX_M4RIE, "default", "m4rie" },
    };

    for (size_t i = 0; i < sizeof cases / sizeof cases[0]; ++i) {
        char label[160];
        bfx_ctx *ctx = open_backend(cases[i].backend, POLY_11D, cases[i].variant, cases[i].label);
        if (ctx == NULL) {
            continue;
        }
        snprintf(label, sizeof label, "%s multiplication table under 0x11D", cases[i].label);
        check_table(ctx, label, POLY_11D);
        snprintf(label, sizeof label, "%s region multiply-XOR under 0x11D", cases[i].label);
        check_axpy(ctx, label, POLY_11D);
        snprintf(label, sizeof label, "%s row region multiply-XOR len=4096 under 0x11D",
                 cases[i].label);
        check_row_axpy(ctx, label, POLY_11D, 4096);
        snprintf(label, sizeof label, "%s row region multiply-XOR len=131072 under 0x11D",
                 cases[i].label);
        check_row_axpy(ctx, label, POLY_11D, 131072);
        snprintf(label, sizeof label, "%s region multiply-assign under 0x11D", cases[i].label);
        check_mul_region(ctx, label, POLY_11D);
        snprintf(label, sizeof label, "%s arbitrary pairwise multiply under 0x11D",
                 cases[i].label);
        check_pairwise(ctx, label, POLY_11D);
        /* The square dimensions and the generator shape the cells measure,
           plus a small and an odd dimension. */
        static const size_t squares[] = { 16, 63, 64, 256, 512 };
        for (size_t s = 0; s < sizeof squares / sizeof squares[0]; ++s) {
            snprintf(label, sizeof label, "%s dense matmul n=%zu under 0x11D", cases[i].label,
                     squares[s]);
            check_matmul(ctx, label, POLY_11D, squares[s], squares[s], squares[s]);
        }
        snprintf(label, sizeof label, "%s dense matmul 4x10 by 10x65536 under 0x11D",
                 cases[i].label);
        check_matmul(ctx, label, POLY_11D, 4, 10, 65536);
        snprintf(label, sizeof label, "%s generator encode k=6 rows=3 len=512 under 0x11D",
                 cases[i].label);
        check_encode(ctx, label, POLY_11D, 6, 3, 512);
        snprintf(label, sizeof label, "%s generator encode k=10 rows=4 len=65536 under 0x11D",
                 cases[i].label);
        check_encode(ctx, label, POLY_11D, 10, 4, 65536);
        bfx_free(ctx);
    }

    /* ISA-L must refuse 0x11B rather than quietly using its own
       polynomial; the other two must accept it and then disagree with
       0x11D. */
    bfx_ctx *isal_11b = open_backend(BFX_ISAL, POLY_11B, "default", "isa-l under 0x11B");
    if (isal_11b != NULL) {
        fail("isa-l under 0x11B", "ISA-L accepted a polynomial it does not implement");
        bfx_free(isal_11b);
    }
    bfx_ctx *gfc_11b = open_backend(BFX_GFCOMPLETE, POLY_11B, "default", "gf-complete under 0x11B");
    if (gfc_11b != NULL) {
        check_table(gfc_11b, "gf-complete multiplication table under 0x11B", POLY_11B);
        bfx_free(gfc_11b);
    }
    bfx_ctx *m4rie_11b = open_backend(BFX_M4RIE, POLY_11B, "default", "m4rie under 0x11B");
    if (m4rie_11b != NULL) {
        check_table(m4rie_11b, "m4rie multiplication table under 0x11B", POLY_11B);
        bfx_free(m4rie_11b);
    }
    check_field_distinctness();

    printf("\n%d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
