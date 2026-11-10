/* C ABI over bchlib's bch_encode, the table remainder of the Linux kernel's
 * lib/bch.c: four 256-entry tables consuming 32 message bits per step.
 *
 * The build compiles this unit once per bchlib configuration and names each
 * one through GF2_BCH_PREFIX: the runtime-parameter build, and builds with
 * CONFIG_BCH_CONST_PARAMS for one (m, t) each, the option bch.c documents as
 * its encoding optimization. bch.c is included here so that its external
 * symbols take the prefix too and every build links into one binary. */

#define GF2_CAT2(a, b) a##_##b
#define GF2_CAT(a, b) GF2_CAT2(a, b)
#define bch_init GF2_CAT(GF2_BCH_PREFIX, bch_init)
#define bch_free GF2_CAT(GF2_BCH_PREFIX, bch_free)
#define bch_encode GF2_CAT(GF2_BCH_PREFIX, bch_encode)
#define bch_decode GF2_CAT(GF2_BCH_PREFIX, bch_decode)
#define bch_compute_even_syndromes GF2_CAT(GF2_BCH_PREFIX, bch_compute_even_syndromes)

/* bch.c's user-space preamble defines no printk, which its const-parameter
 * rejection of a mismatched (m, t) calls. */
#include <stdio.h>
#define KERN_ERR ""
#define printk(...) fprintf(stderr, __VA_ARGS__)

#include "bch.c"

void* GF2_CAT(GF2_BCH_PREFIX, new)(int32_t m, int32_t t, uint32_t prim_poly)
{
    return bch_init(m, t, prim_poly, false);
}

void GF2_CAT(GF2_BCH_PREFIX, free)(void* handle)
{
    bch_free((struct bch_control*)handle);
}

uint32_t GF2_CAT(GF2_BCH_PREFIX, ecc_bits)(void* handle)
{
    return ((struct bch_control*)handle)->ecc_bits;
}

/* Parity of `len` data bytes into `ecc`, which bch_encode accumulates into
 * and is therefore cleared first. */
void GF2_CAT(GF2_BCH_PREFIX, encode)(void* handle, const uint8_t* data, uint32_t len,
    uint8_t* ecc)
{
    struct bch_control* bch = (struct bch_control*)handle;
    memset(ecc, 0, bch->ecc_bytes);
    bch_encode(bch, data, len, ecc);
}
