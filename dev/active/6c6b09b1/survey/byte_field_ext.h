/*
 * byte_field_ext.h — uniform C ABI over the pinned byte-field externals
 * (jit:6c6b09b1).
 *
 * The three externals disagree about how a GF(2^8) region operation is
 * spelled, so this shim gives each one the same entry points and reports
 * honestly which ones a backend does not provide. It adds no arithmetic of
 * its own beyond the independent scalar reference used for validation.
 *
 * Element convention, shared by every backend and by gf2-core: a GF(2^8)
 * element is one byte whose bit i is the coefficient of x^i, reduced modulo
 * the full degree-8 polynomial passed to bfx_init. ISA-L compiles in 0x11D
 * and rejects any other value; M4RIE and GF-Complete accept the polynomial
 * as a parameter.
 */
#ifndef BYTE_FIELD_EXT_H
#define BYTE_FIELD_EXT_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Backend selectors accepted by bfx_init. */
#define BFX_ISAL 1
#define BFX_GFCOMPLETE 2
#define BFX_M4RIE 3

/* Reasons an operation is not offered by the selected backend. */
#define BFX_OK 0
#define BFX_ERR_UNSUPPORTED 1
#define BFX_ERR_INVALID 2
#define BFX_ERR_ALLOC 3

typedef struct bfx_ctx bfx_ctx;
typedef struct bfx_mat bfx_mat;

/*
 * Creates a backend context for the full reduction polynomial `poly`
 * (every cell uses 0x11D; the conformance run also opens 0x11B to record
 * which backends accept it). `variant` names the
 * arithmetic backend within the library, or is NULL for that library's
 * default; unknown variants fail rather than falling back silently.
 * Returns NULL and sets *why to a static reason on failure.
 */
bfx_ctx *bfx_init(int backend, unsigned int poly, const char *variant, const char **why);
void bfx_free(bfx_ctx *ctx);

/* Runtime-observed identity of the arithmetic backend actually selected. */
const char *bfx_backend_name(const bfx_ctx *ctx);
/* Version string of the library actually linked. */
const char *bfx_library_version(const bfx_ctx *ctx);

/*
 * Fixed-coefficient reuse. bfx_prepare performs the table preparation a
 * backend needs for one coefficient; bfx_axpy_apply then accumulates
 * dest[i] ^= a * src[i] over `len` bytes with that prepared coefficient.
 * Splitting them lets the caller attribute table-preparation cost
 * separately from the region pass. src and dest do not overlap.
 */
int bfx_prepare(bfx_ctx *ctx, unsigned char a);
int bfx_axpy_apply(bfx_ctx *ctx, const unsigned char *src, unsigned char *dest, size_t len);

/*
 * Fixed-coefficient region assignment dest[i] = a * src[i], without the
 * accumulation step, for backends that separate the two.
 */
int bfx_mul_region_apply(bfx_ctx *ctx, const unsigned char *src, unsigned char *dest, size_t len);

/*
 * Arbitrary pairwise multiplication dest[i] = x[i] * y[i]: every element
 * pair is distinct, so no coefficient table can be reused across the
 * region. No backend has a region kernel for it; each applies its public
 * single-element multiply per byte.
 */
int bfx_mul_pairwise(bfx_ctx *ctx, const unsigned char *x, const unsigned char *y,
                     unsigned char *dest, size_t len);

/*
 * Nonzero when bfx_prepare performs a table preparation separate from the
 * region call (ISA-L's ec_init_tables). GF-Complete expands the coefficient
 * inside each region call and M4RIE takes it as an argument, so for them
 * bfx_prepare only records the coefficient.
 */
int bfx_has_separate_prepare(const bfx_ctx *ctx);

/* Single-element multiply, used to validate the region paths. */
unsigned char bfx_mul_scalar(bfx_ctx *ctx, unsigned char a, unsigned char b);

/*
 * Independent scalar reference: shift-and-reduce against `poly`, written
 * here and calling into no external library, so a region result can be
 * checked against arithmetic that shares no code with the backend.
 */
unsigned char bfx_ref_mul(unsigned char a, unsigned char b, unsigned int poly);

/*
 * Dense matrices in the backend's own layout. bfx_mat_pack and
 * bfx_mat_unpack convert to and from row-major bytes so representation
 * conversion is measurable on its own. bfx_mat_mul computes c = a * b.
 * Backends with no dense matrix layer return NULL from bfx_mat_new.
 */
bfx_mat *bfx_mat_new(bfx_ctx *ctx, size_t rows, size_t cols);
void bfx_mat_free(bfx_mat *mat);
int bfx_mat_pack(bfx_mat *mat, const unsigned char *bytes);
int bfx_mat_unpack(const bfx_mat *mat, unsigned char *bytes);
int bfx_mat_mul(bfx_mat *c, const bfx_mat *a, const bfx_mat *b);

/*
 * Fixed-coefficient row accumulate dest[dest_row] ^= a * src[src_row],
 * M4RIE's region multiply-XOR: the same operation as bfx_axpy_apply, but
 * addressed through the matrix type because M4RIE has no free-standing
 * region API.
 */
int bfx_mat_row_axpy(bfx_mat *dest, size_t dest_row, const bfx_mat *src, size_t src_row,
                     unsigned char a);

/*
 * Generator-matrix region encode: coding[r][i] = sum_j g[r*k + j] *
 * data[j][i] over `len` bytes, the shape ISA-L's ec_encode_data has.
 * bfx_encode_prepare performs the generator table preparation once.
 */
int bfx_encode_prepare(bfx_ctx *ctx, const unsigned char *g, int k, int rows);
int bfx_encode_apply(bfx_ctx *ctx, int len, int k, int rows,
                     unsigned char **data, unsigned char **coding);

#ifdef __cplusplus
}
#endif

#endif /* BYTE_FIELD_EXT_H */
