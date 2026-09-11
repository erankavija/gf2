/*
 * backend_provenance.c — reads the arithmetic backend each pinned external
 * library actually selects on this host, out of the loaded binaries
 * (jit:6c6b09b1).
 *
 * The survey's comparison is only meaningful if the external arm is the
 * kernel the library really runs here, not the one its documentation
 * advertises. Every line this program prints is observed at run time from
 * the process that just performed the operation:
 *
 *   - ISA-L resolves each region entry point through a multibinary
 *     dispatch slot. The slot is the `jmp *disp32(%rip)` target of the
 *     exported thunk; reading it after one call gives the address of the
 *     kernel this CPU selected.
 *   - GF-Complete stores the selected kernels in the `gf_t` function
 *     pointers `multiply.w32` and `multiply_region.w32`.
 *   - M4RIE reaches element arithmetic through `gf2e->mul` and reaches the
 *     product through a recursion whose base case this program counts by
 *     interposing the library's own PLT-bound entry points.
 *
 * A pointer is reported as `ptr <label> <library> <offset>`; the offset is
 * relative to the library's load base, so `arm-provenance.sh` resolves it
 * against that file's symbol table. Symbol resolution is left to the shell
 * because the kernels of interest are file-local symbols that `dladdr`
 * cannot name.
 */
#define _GNU_SOURCE

#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <isa-l/erasure_code.h>
#include <isa-l/gf_vect_mul.h>

#include <gf_complete.h>

#include <m4rie/m4rie.h>

#define POLY 0x11Du

/* ---------------------------------------------------------------- helpers */

static void report_pointer(const char *label, const void *fn)
{
    Dl_info info;
    if (fn == NULL) {
        printf("unresolved %s\n", label);
        return;
    }
    if (dladdr(fn, &info) == 0 || info.dli_fbase == NULL || info.dli_fname == NULL) {
        printf("unattributable %s %p\n", label, fn);
        return;
    }
    printf("ptr      %s %s 0x%tx\n", label, info.dli_fname,
           (const unsigned char *)fn - (const unsigned char *)info.dli_fbase);
}

/*
 * Follows one `[endbr64] [bnd] jmp *disp32(%rip)` hop and returns the stored
 * target, or NULL when the bytes are not that instruction. Both the ISA-L
 * multibinary thunk and the PLT stub its dispatch slot names have this shape.
 */
static void *follow_indirect_jump(const void *code)
{
    const unsigned char *at = (const unsigned char *)code;
    if (at == NULL) {
        return NULL;
    }
    if (at[0] == 0xF3 && at[1] == 0x0F && at[2] == 0x1E && at[3] == 0xFA) {
        at += 4; /* endbr64 */
    }
    if (at[0] == 0xF2) {
        at += 1; /* bnd prefix */
    }
    if (!(at[0] == 0xFF && at[1] == 0x25)) {
        return NULL;
    }
    int32_t displacement;
    memcpy(&displacement, at + 2, sizeof displacement);
    return *(void **)(at + 6 + displacement);
}

/*
 * Returns the implementation an ISA-L entry point currently reaches. The
 * exported name is a multibinary thunk jumping through the
 * `<entry>_dispatched` slot the dispatcher rewrote on the first call; that
 * slot holds the kernel's PLT stub, which jumps through the GOT to the
 * kernel itself. Following the chain to its last indirect hop therefore
 * names the code this CPU actually runs.
 */
static void *isal_dispatched(const char *name)
{
    void *target = dlsym(RTLD_DEFAULT, name);
    for (int hop = 0; hop < 4; ++hop) {
        void *next = follow_indirect_jump(target);
        if (next == NULL) {
            break;
        }
        target = next;
    }
    return target;
}

/* --------------------------------------------- M4RIE recursion observation */

/*
 * M4RIE is a shared library whose internal calls to these entry points go
 * through its own PLT, so a definition here is what it calls. Each wrapper
 * counts the call and forwards to the library's implementation, which makes
 * the recursion M4RIE actually takes for a given matrix size observable
 * instead of inferred from its source.
 */
static unsigned long strassen_calls;
static unsigned long newton_john_calls;
static unsigned long naive_calls;
/* At or above 512 in every dimension `_mzed_mul` takes the inline bitsliced
   Karatsuba path, which no PLT entry names; its conversions into and out of
   the bitsliced representation are exported, so they mark the path. */
static unsigned long slice_calls;
static unsigned long cling_calls;

mzd_slice_t *mzed_slice(mzd_slice_t *A, const mzed_t *Z)
{
    static mzd_slice_t *(*real)(mzd_slice_t *, const mzed_t *);
    if (real == NULL) {
        real = dlsym(RTLD_NEXT, "mzed_slice");
    }
    slice_calls++;
    return real(A, Z);
}

mzed_t *mzed_cling(mzed_t *A, const mzd_slice_t *Z)
{
    static mzed_t *(*real)(mzed_t *, const mzd_slice_t *);
    if (real == NULL) {
        real = dlsym(RTLD_NEXT, "mzed_cling");
    }
    cling_calls++;
    return real(A, Z);
}

mzed_t *_mzed_mul_strassen(mzed_t *C, const mzed_t *A, const mzed_t *B, int cutoff)
{
    static mzed_t *(*real)(mzed_t *, const mzed_t *, const mzed_t *, int);
    if (real == NULL) {
        real = dlsym(RTLD_NEXT, "_mzed_mul_strassen");
    }
    strassen_calls++;
    return real(C, A, B, cutoff);
}

mzed_t *_mzed_mul_newton_john(mzed_t *C, const mzed_t *A, const mzed_t *B)
{
    static mzed_t *(*real)(mzed_t *, const mzed_t *, const mzed_t *);
    if (real == NULL) {
        real = dlsym(RTLD_NEXT, "_mzed_mul_newton_john");
    }
    newton_john_calls++;
    return real(C, A, B);
}

mzed_t *_mzed_mul_naive(mzed_t *C, const mzed_t *A, const mzed_t *B)
{
    static mzed_t *(*real)(mzed_t *, const mzed_t *, const mzed_t *);
    if (real == NULL) {
        real = dlsym(RTLD_NEXT, "_mzed_mul_naive");
    }
    naive_calls++;
    return real(C, A, B);
}

/* ------------------------------------------------------------------ ISA-L */

static void isal(void)
{
    enum { LEN = 4096 };
    unsigned char coefficient[1] = { 0x53 };
    unsigned char table[32];
    unsigned char *source = calloc(LEN, 1);
    unsigned char *target = calloc(LEN, 1);
    if (source == NULL || target == NULL) {
        fprintf(stderr, "backend-provenance: cannot allocate ISA-L buffers\n");
        exit(1);
    }

    ec_init_tables(1, 1, coefficient, table);
    gf_vect_mad(LEN, 1, 0, table, source, target);
    gf_vect_mul(LEN, table, source, target);
    unsigned char *sources[1] = { source };
    gf_vect_dot_prod(LEN, 1, table, sources, target);

    const int k = 6;
    const int rows = 3;
    unsigned char generator[18];
    for (int index = 0; index < k * rows; ++index) {
        generator[index] = (unsigned char)(index + 1);
    }
    unsigned char *gftbls = calloc((size_t)k * rows * 32, 1);
    unsigned char *data[6];
    unsigned char *coding[3];
    unsigned char *pool = calloc((size_t)(k + rows) * LEN, 1);
    if (gftbls == NULL || pool == NULL) {
        fprintf(stderr, "backend-provenance: cannot allocate ISA-L encode buffers\n");
        exit(1);
    }
    for (int index = 0; index < k; ++index) {
        data[index] = pool + (size_t)index * LEN;
    }
    for (int index = 0; index < rows; ++index) {
        coding[index] = pool + (size_t)(k + index) * LEN;
    }
    ec_init_tables(k, rows, generator, gftbls);
    ec_encode_data(LEN, k, rows, gftbls, data, coding);

    printf("library  isa-l %s poly=0x11D compiled-in\n", ISAL_VERSION_STR);
    report_pointer("isa-l/ec_init_tables", isal_dispatched("ec_init_tables"));
    report_pointer("isa-l/gf_vect_mad", isal_dispatched("gf_vect_mad"));
    report_pointer("isa-l/gf_vect_mul", isal_dispatched("gf_vect_mul"));
    report_pointer("isa-l/gf_vect_dot_prod", isal_dispatched("gf_vect_dot_prod"));
    report_pointer("isa-l/ec_encode_data", isal_dispatched("ec_encode_data"));
    /* The pairwise control's per-byte multiply: a plain exported function
       with no dispatch slot. */
    report_pointer("isa-l/gf_mul", dlsym(RTLD_DEFAULT, "gf_mul"));

    free(pool);
    free(gftbls);
    free(target);
    free(source);
}

/* ------------------------------------------------------------ GF-Complete */

static void gfcomplete_variant(const char *label, int mult, int region, int arg1, int arg2)
{
    gf_t gf;
    if (gf_init_hard(&gf, 8, mult, region, GF_DIVIDE_DEFAULT, POLY, arg1, arg2, NULL, NULL) == 0) {
        printf("unresolved gf-complete/%s (gf_init_hard rejected the configuration)\n", label);
        return;
    }
    enum { LEN = 4096 };
    unsigned char *source = calloc(LEN, 1);
    unsigned char *target = calloc(LEN, 1);
    if (source == NULL || target == NULL) {
        fprintf(stderr, "backend-provenance: cannot allocate GF-Complete buffers\n");
        exit(1);
    }
    (void)gf.multiply.w32(&gf, 0x53, 0x11);
    gf.multiply_region.w32(&gf, source, target, 0x53, LEN, 1);
    char name[128];
    snprintf(name, sizeof name, "gf-complete/%s/multiply.w32", label);
    report_pointer(name, (const void *)gf.multiply.w32);
    snprintf(name, sizeof name, "gf-complete/%s/multiply_region.w32", label);
    report_pointer(name, (const void *)gf.multiply_region.w32);
    free(target);
    free(source);
    gf_free(&gf, 1);
}

static void gfcomplete(void)
{
    printf("library  gf-complete %s poly=0x%X requested\n", GFCOMPLETE_VERSION_STR, POLY);
    /* The three configurations bfx_init offers, in the same order. */
    gfcomplete_variant("default", GF_MULT_DEFAULT, GF_REGION_DEFAULT, 0, 0);
    gfcomplete_variant("split-table-4-8-simd", GF_MULT_SPLIT_TABLE, GF_REGION_SIMD, 4, 8);
    gfcomplete_variant("full-table", GF_MULT_TABLE, GF_REGION_DEFAULT, 0, 0);
}

/* ------------------------------------------------------------------ M4RIE */

/* Counts the recursion one product of shape rows x inner by inner x cols
   takes, at the shapes the survey's cells measure. */
static void m4rie_sizes(gf2e *ff, int rows, int inner, int cols)
{
    mzed_t *a = mzed_init(ff, rows, inner);
    mzed_t *b = mzed_init(ff, inner, cols);
    mzed_randomize(a);
    mzed_randomize(b);
    strassen_calls = 0;
    newton_john_calls = 0;
    naive_calls = 0;
    slice_calls = 0;
    cling_calls = 0;
    mzed_t *c = mzed_mul(NULL, a, b);
    printf("m4rie    mzed_mul %dx%d by %dx%d strassen=%lu newton-john=%lu naive=%lu "
           "bitslice-karatsuba(slice=%lu cling=%lu)\n",
           rows, inner, inner, cols, strassen_calls, newton_john_calls, naive_calls, slice_calls,
           cling_calls);
    mzed_free(c);
    mzed_free(b);
    mzed_free(a);
}

static void m4rie(void)
{
    gf2e *ff = gf2e_init(POLY);
    if (ff == NULL) {
        printf("unresolved m4rie (gf2e_init rejected 0x%X)\n", POLY);
        return;
    }
    printf("library  m4rie %s (over m4ri %s) poly=0x%X requested\n", M4RIE_VERSION_STR,
           M4RI_VERSION_STR, POLY);
    printf("m4rie    degree=%u minpoly=0x%llX element-multiply-table=%s\n", (unsigned)ff->degree,
           (unsigned long long)ff->minpoly, ff->_mul != NULL ? "present" : "absent");
    report_pointer("m4rie/gf2e->mul", (const void *)ff->mul);
    report_pointer("m4rie/gf2e->inv", (const void *)ff->inv);

    /* The row form the survey's axpy cell measures. */
    mzed_t *source = mzed_init(ff, 1, 4096);
    mzed_t *target = mzed_init(ff, 1, 4096);
    mzed_randomize(source);
    mzed_randomize(target);
    mzed_add_multiple_of_row(target, 0, source, 0, 0x53, 0);
    report_pointer("m4rie/mzed_add_multiple_of_row", (const void *)dlsym(RTLD_DEFAULT,
                                                                        "mzed_add_multiple_of_row"));
    mzed_free(target);
    mzed_free(source);

    mzed_t *probe_a = mzed_init(ff, 128, 128);
    mzed_t *probe_b = mzed_init(ff, 128, 128);
    mzed_t *probe_c = mzed_init(ff, 128, 128);
    printf("m4rie    strassen-cutoff-n=%d\n", _mzed_strassen_cutoff(probe_c, probe_a, probe_b));
    mzed_free(probe_c);
    mzed_free(probe_b);
    mzed_free(probe_a);

    for (int n = 64; n <= 1024; n *= 2) {
        m4rie_sizes(ff, n, n, n);
    }
    m4rie_sizes(ff, 4, 10, 65536);
    gf2e_free(ff);
}

/* ------------------------------------------------------------------- main */

int main(void)
{
    printf("cpu      avx2=%d avx512f=%d ssse3=%d sse4.2=%d pclmul=%d\n",
           __builtin_cpu_supports("avx2") != 0, __builtin_cpu_supports("avx512f") != 0,
           __builtin_cpu_supports("ssse3") != 0, __builtin_cpu_supports("sse4.2") != 0,
           __builtin_cpu_supports("pclmul") != 0);
    isal();
    gfcomplete();
    m4rie();
    return 0;
}
