/* C ABI over M4RI's dense GF(2) elimination, materializing the systematic
 * generator matrix of a cyclic code: row i of the k x n matrix is x^i g(x),
 * written with column c holding the coefficient of x^(n-1-c) (gf2's
 * descending layout), then reduced to reduced row echelon form, which is
 * [I_k | P] in that layout. M4RI packs column c at bit c % 64 of word c / 64,
 * gf2's canonical bit order. */

#include <m4ri/m4ri.h>
#include <stdint.h>

/* `g` holds deg + 1 coefficients, g[i] of x^i. `method` 0 runs
 * mzd_echelonize_m4ri, 1 mzd_echelonize_pluq. The caller frees the result
 * with gf2_m4ri_free. */
void* gf2_m4ri_generator(int32_t k, int32_t n, const uint8_t* g, int32_t deg, int32_t method)
{
    mzd_t* G = mzd_init(k, n);
    for (rci_t i = 0; i < k; i++)
        for (int32_t j = 0; j <= deg; j++)
            if (g[j])
                mzd_write_bit(G, i, (rci_t)(n - 1 - (i + j)), 1);
    if (method == 0)
        mzd_echelonize_m4ri(G, 1, 0);
    else
        mzd_echelonize_pluq(G, 1);
    return G;
}

const uint64_t* gf2_m4ri_row(void* matrix, int32_t row)
{
    return (const uint64_t*)mzd_row((mzd_t*)matrix, row);
}

void gf2_m4ri_free(void* matrix)
{
    mzd_free((mzd_t*)matrix);
}
