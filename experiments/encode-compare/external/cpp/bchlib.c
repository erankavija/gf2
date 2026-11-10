/* C ABI over bchlib's bch_encode, the table remainder of the Linux kernel's
 * lib/bch.c: four 256-entry tables consuming 32 message bits per step. */

#include <stdint.h>
#include <string.h>

#include "bch.h"

void* gf2_bchlib_new(int32_t m, int32_t t, uint32_t prim_poly)
{
    return bch_init(m, t, prim_poly, false);
}

void gf2_bchlib_free(void* handle)
{
    bch_free((struct bch_control*)handle);
}

uint32_t gf2_bchlib_ecc_bits(void* handle)
{
    return ((struct bch_control*)handle)->ecc_bits;
}

/* Parity of `len` data bytes into `ecc`, which bch_encode accumulates into
 * and is therefore cleared first. */
void gf2_bchlib_encode(void* handle, const uint8_t* data, uint32_t len, uint8_t* ecc)
{
    struct bch_control* bch = (struct bch_control*)handle;
    memset(ecc, 0, bch->ecc_bytes);
    bch_encode(bch, data, len, ecc);
}
