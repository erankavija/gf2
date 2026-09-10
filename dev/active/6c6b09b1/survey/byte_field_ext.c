/*
 * byte_field_ext.c — uniform C ABI over the pinned byte-field externals
 * (jit:6c6b09b1).
 *
 * See byte_field_ext.h for the contract. Each backend section states which
 * upstream entry point implements each operation and which operations that
 * library does not provide.
 */

#include "byte_field_ext.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <isa-l/erasure_code.h>
#include <isa-l/gf_vect_mul.h>

#include <gf_complete.h>

#include <m4rie/m4rie.h>

/* ISA-L reduces with the compiled-in polynomial x^8+x^4+x^3+x^2+1. */
#define BFX_ISAL_POLY 0x11Du

struct bfx_ctx {
    int backend;
    unsigned int poly;
    char name[96];
    char version[64];

    /* ISA-L: a 32-byte expanded table per coefficient, and the generator
       tables ec_init_tables produces for the encode shape. */
    unsigned char isal_tbl[32];
    unsigned char *isal_gftbls;
    int isal_k;
    int isal_rows;

    /* GF-Complete: one initialised field plus the coefficient the region
       calls carry as an argument (the library expands it internally). */
    gf_t gf;
    int gf_ready;
    unsigned char coeff;

    /* M4RIE: the field context; coefficient reuse is an argument, and a
       one-row scratch matrix pair carries the region operands. */
    gf2e *ff;
};

struct bfx_mat {
    bfx_ctx *ctx;
    size_t rows;
    size_t cols;
    mzed_t *mzed;
};

unsigned char bfx_ref_mul(unsigned char a, unsigned char b, unsigned int poly)
{
    unsigned int product = 0;
    unsigned int x = a;
    for (int bit = 0; bit < 8; ++bit) {
        if ((b >> bit) & 1u) {
            product ^= x << bit;
        }
    }
    for (int degree = 15; degree >= 8; --degree) {
        if ((product >> degree) & 1u) {
            product ^= poly << (degree - 8);
        }
    }
    return (unsigned char)(product & 0xFFu);
}

static void describe(bfx_ctx *ctx, const char *name, const char *version)
{
    snprintf(ctx->name, sizeof ctx->name, "%s", name);
    snprintf(ctx->version, sizeof ctx->version, "%s", version);
}

bfx_ctx *bfx_init(int backend, unsigned int poly, const char *variant, const char **why)
{
    const char *ignored = NULL;
    if (why == NULL) {
        why = &ignored;
    }
    bfx_ctx *ctx = calloc(1, sizeof *ctx);
    if (ctx == NULL) {
        *why = "cannot allocate a backend context";
        return NULL;
    }
    ctx->backend = backend;
    ctx->poly = poly;

    if (backend == BFX_ISAL) {
        if (poly != BFX_ISAL_POLY) {
            *why = "ISA-L compiles in GF(2^8) modulo 0x11D and accepts no other polynomial";
            free(ctx);
            return NULL;
        }
        if (variant != NULL && strcmp(variant, "default") != 0) {
            *why = "ISA-L selects its kernel at runtime and exposes no variant switch";
            free(ctx);
            return NULL;
        }
        describe(ctx, "isa-l/runtime-dispatched-simd", ISAL_VERSION_STR);
        return ctx;
    }

    if (backend == BFX_GFCOMPLETE) {
        int mult = GF_MULT_DEFAULT;
        int region = GF_REGION_DEFAULT;
        int arg1 = 0;
        int arg2 = 0;
        const char *label = "gf-complete/default";
        if (variant == NULL || strcmp(variant, "default") == 0) {
            /* GF_MULT_DEFAULT selects GF-Complete's SSSE3 nibble-shuffle
               region kernel where SSSE3 is present and its single full
               table otherwise (gf_w8.c:1144-1157). The host record's CPU
               flags say which branch this host takes. */
        } else if (strcmp(variant, "split-table-simd") == 0) {
            /* w=8 accepts only the 4/8 and 8/4 splits (gf_w8.c:2266). */
            mult = GF_MULT_SPLIT_TABLE;
            region = GF_REGION_SIMD;
            arg1 = 4;
            arg2 = 8;
            label = "gf-complete/split-table-4-8-simd";
        } else if (strcmp(variant, "table") == 0) {
            mult = GF_MULT_TABLE;
            label = "gf-complete/full-table";
        } else {
            *why = "unknown GF-Complete variant";
            free(ctx);
            return NULL;
        }
        if (gf_init_hard(&ctx->gf, 8, mult, region, GF_DIVIDE_DEFAULT, (uint64_t)poly, arg1, arg2,
                         NULL, NULL) == 0) {
            *why = "gf_init_hard rejected the requested GF(2^8) configuration";
            free(ctx);
            return NULL;
        }
        ctx->gf_ready = 1;
        describe(ctx, label, GFCOMPLETE_VERSION_STR);
        return ctx;
    }

    if (backend == BFX_M4RIE) {
        if (variant != NULL && strcmp(variant, "default") != 0) {
            *why = "M4RIE exposes no arithmetic-backend switch for GF(2^8)";
            free(ctx);
            return NULL;
        }
        ctx->ff = gf2e_init((word)poly);
        if (ctx->ff == NULL) {
            *why = "gf2e_init rejected the requested minimal polynomial";
            free(ctx);
            return NULL;
        }
        describe(ctx, "m4rie/newton-john", M4RIE_VERSION_STR);
        return ctx;
    }

    *why = "unknown backend selector";
    free(ctx);
    return NULL;
}

void bfx_free(bfx_ctx *ctx)
{
    if (ctx == NULL) {
        return;
    }
    if (ctx->gf_ready) {
        gf_free(&ctx->gf, 1);
    }
    if (ctx->ff != NULL) {
        gf2e_free(ctx->ff);
    }
    free(ctx->isal_gftbls);
    free(ctx);
}

const char *bfx_backend_name(const bfx_ctx *ctx) { return ctx->name; }
const char *bfx_library_version(const bfx_ctx *ctx) { return ctx->version; }

int bfx_prepare(bfx_ctx *ctx, unsigned char a)
{
    ctx->coeff = a;
    if (ctx->backend == BFX_ISAL) {
        /* ec_init_tables expands one coefficient into the 32-byte
           lo/hi nibble table gf_vect_mad and gf_vect_mul consume. */
        unsigned char matrix[1] = { a };
        ec_init_tables(1, 1, matrix, ctx->isal_tbl);
        return BFX_OK;
    }
    /* GF-Complete expands the coefficient inside each region call and
       M4RIE takes it as an argument, so neither has a separable
       preparation step to charge here. */
    return BFX_OK;
}

int bfx_axpy_apply(bfx_ctx *ctx, const unsigned char *src, unsigned char *dest, size_t len)
{
    if (len == 0) {
        return BFX_OK;
    }
    switch (ctx->backend) {
    case BFX_ISAL:
        if (len < 64) {
            return BFX_ERR_INVALID; /* gf_vect_mad requires len >= 64. */
        }
        gf_vect_mad((int)len, 1, 0, ctx->isal_tbl, (unsigned char *)src, dest);
        return BFX_OK;
    case BFX_GFCOMPLETE:
        ctx->gf.multiply_region.w32(&ctx->gf, (void *)src, dest, (gf_val_32_t)ctx->coeff, (int)len,
                                    1);
        return BFX_OK;
    case BFX_M4RIE:
        return BFX_ERR_UNSUPPORTED; /* Row axpy goes through bfx_mat; see bfx_mat_row_axpy. */
    default:
        return BFX_ERR_INVALID;
    }
}

int bfx_mul_region_apply(bfx_ctx *ctx, const unsigned char *src, unsigned char *dest, size_t len)
{
    if (len == 0) {
        return BFX_OK;
    }
    switch (ctx->backend) {
    case BFX_ISAL:
        if (len < 32) {
            return BFX_ERR_INVALID; /* gf_vect_mul requires len >= 32. */
        }
        gf_vect_mul((int)len, ctx->isal_tbl, (void *)src, dest);
        return BFX_OK;
    case BFX_GFCOMPLETE:
        ctx->gf.multiply_region.w32(&ctx->gf, (void *)src, dest, (gf_val_32_t)ctx->coeff, (int)len,
                                    0);
        return BFX_OK;
    case BFX_M4RIE:
        return BFX_ERR_UNSUPPORTED;
    default:
        return BFX_ERR_INVALID;
    }
}

int bfx_mul_pairwise(bfx_ctx *ctx, const unsigned char *x, const unsigned char *y,
                     unsigned char *dest, size_t len)
{
    switch (ctx->backend) {
    case BFX_ISAL:
        /* ISA-L's region API is built entirely around one expanded
           coefficient; it offers no vectorised elementwise product of two
           regions. gf_mul is a scalar helper, not a region kernel, so
           reporting it as a region arm would compare a different
           operation. */
        return BFX_ERR_UNSUPPORTED;
    case BFX_GFCOMPLETE:
        for (size_t i = 0; i < len; ++i) {
            dest[i] = (unsigned char)ctx->gf.multiply.w32(&ctx->gf, x[i], y[i]);
        }
        return BFX_OK;
    case BFX_M4RIE:
        /* M4RIE's element accessors are scalar and its region kernels are
           all fixed-coefficient or matrix shaped. */
        return BFX_ERR_UNSUPPORTED;
    default:
        return BFX_ERR_INVALID;
    }
}

unsigned char bfx_mul_scalar(bfx_ctx *ctx, unsigned char a, unsigned char b)
{
    switch (ctx->backend) {
    case BFX_ISAL:
        return gf_mul(a, b);
    case BFX_GFCOMPLETE:
        return (unsigned char)ctx->gf.multiply.w32(&ctx->gf, a, b);
    case BFX_M4RIE:
        return (unsigned char)gf2e_mul(ctx->ff, (word)a, (word)b);
    default:
        return 0;
    }
}

bfx_mat *bfx_mat_new(bfx_ctx *ctx, size_t rows, size_t cols)
{
    if (ctx->backend != BFX_M4RIE) {
        /* Neither ISA-L nor GF-Complete carries a dense GF(2^8) matrix
           type; their nearest shape is the generator encode below. */
        return NULL;
    }
    bfx_mat *mat = calloc(1, sizeof *mat);
    if (mat == NULL) {
        return NULL;
    }
    mat->ctx = ctx;
    mat->rows = rows;
    mat->cols = cols;
    mat->mzed = mzed_init(ctx->ff, (rci_t)rows, (rci_t)cols);
    if (mat->mzed == NULL) {
        free(mat);
        return NULL;
    }
    return mat;
}

void bfx_mat_free(bfx_mat *mat)
{
    if (mat == NULL) {
        return;
    }
    mzed_free(mat->mzed);
    free(mat);
}

int bfx_mat_pack(bfx_mat *mat, const unsigned char *bytes)
{
    for (size_t r = 0; r < mat->rows; ++r) {
        for (size_t c = 0; c < mat->cols; ++c) {
            mzed_write_elem(mat->mzed, (rci_t)r, (rci_t)c, (word)bytes[r * mat->cols + c]);
        }
    }
    return BFX_OK;
}

int bfx_mat_unpack(const bfx_mat *mat, unsigned char *bytes)
{
    for (size_t r = 0; r < mat->rows; ++r) {
        for (size_t c = 0; c < mat->cols; ++c) {
            bytes[r * mat->cols + c] =
                (unsigned char)mzed_read_elem(mat->mzed, (rci_t)r, (rci_t)c);
        }
    }
    return BFX_OK;
}

int bfx_mat_mul(bfx_mat *c, const bfx_mat *a, const bfx_mat *b)
{
    if (mzed_mul(c->mzed, a->mzed, b->mzed) == NULL) {
        return BFX_ERR_INVALID;
    }
    return BFX_OK;
}

int bfx_mat_row_axpy(bfx_mat *dest, size_t dest_row, const bfx_mat *src, size_t src_row,
                     unsigned char a)
{
    mzed_add_multiple_of_row(dest->mzed, (rci_t)dest_row, src->mzed, (rci_t)src_row, (word)a, 0);
    return BFX_OK;
}

int bfx_encode_prepare(bfx_ctx *ctx, const unsigned char *g, int k, int rows)
{
    if (ctx->backend != BFX_ISAL) {
        return BFX_ERR_UNSUPPORTED;
    }
    unsigned char *tables = realloc(ctx->isal_gftbls, (size_t)(32 * k * rows));
    if (tables == NULL) {
        return BFX_ERR_ALLOC;
    }
    ctx->isal_gftbls = tables;
    ctx->isal_k = k;
    ctx->isal_rows = rows;
    ec_init_tables(k, rows, (unsigned char *)g, ctx->isal_gftbls);
    return BFX_OK;
}

int bfx_encode_apply(bfx_ctx *ctx, int len, int k, int rows, unsigned char **data,
                     unsigned char **coding)
{
    if (ctx->backend != BFX_ISAL) {
        return BFX_ERR_UNSUPPORTED;
    }
    if (k != ctx->isal_k || rows != ctx->isal_rows) {
        return BFX_ERR_INVALID;
    }
    ec_encode_data(len, k, rows, ctx->isal_gftbls, data, coding);
    return BFX_OK;
}
