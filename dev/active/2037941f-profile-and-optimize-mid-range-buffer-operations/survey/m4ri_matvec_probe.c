/*
 * Semantic qualification probe for jit:92385645.
 *
 * The probe deliberately contains no timing loop.  It checks that the public
 * M4RI dense product used by the proposed comparator has the same logical
 * result as BitMatrix::matvec: y[r] = parity(A[r, :] & x), with bit c in
 * canonical word c / 64 at mask 1 << (c % 64).
 */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include <m4ri/m4ri.h>

typedef struct {
    rci_t rows;
    rci_t cols;
} probe_case;

static uint64_t splitmix64_next(uint64_t *state) {
    uint64_t z;
    *state += UINT64_C(0x9e3779b97f4a7c15);
    z = *state;
    z = (z ^ (z >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)) * UINT64_C(0x94d049bb133111eb);
    return z ^ (z >> 31);
}

static int bit_parity(uint64_t value) {
    return __builtin_parityll(value);
}

static int run_case(probe_case shape, uint64_t seed) {
    const size_t words = ((size_t)shape.cols + 63U) / 64U;
    uint64_t *canonical_rows = calloc((size_t)shape.rows * words, sizeof(*canonical_rows));
    uint64_t *canonical_x = calloc(words, sizeof(*canonical_x));
    mzd_t *a = NULL;
    mzd_t *x = NULL;
    mzd_t *y = NULL;
    int ok = 0;

    if (canonical_rows == NULL || canonical_x == NULL) goto out;
    a = mzd_init(shape.rows, shape.cols);
    x = mzd_init(shape.cols, 1);
    y = mzd_init(shape.rows, 1);
    if (a == NULL || x == NULL || y == NULL) goto out;
    mzd_set_ui(a, 0);
    mzd_set_ui(x, 0);
    mzd_set_ui(y, 0);

    for (rci_t r = 0; r < shape.rows; ++r) {
        for (rci_t c = 0; c < shape.cols; ++c) {
            const int set = (int)(splitmix64_next(&seed) & 1U);
            if (set) {
                canonical_rows[(size_t)r * words + ((size_t)c >> 6)] |= UINT64_C(1) << (c & 63U);
                mzd_write_bit(a, r, c, 1);
            }
        }
    }
    for (rci_t c = 0; c < shape.cols; ++c) {
        if (splitmix64_next(&seed) & 1U) {
            canonical_x[(size_t)c >> 6] |= UINT64_C(1) << (c & 63U);
            mzd_write_bit(x, c, 0, 1);
        }
    }

    if (mzd_mul(y, a, x, 0) != y) goto out;
    for (rci_t r = 0; r < shape.rows; ++r) {
        uint64_t parity_words = 0;
        for (size_t word = 0; word < words; ++word)
            parity_words ^= canonical_rows[(size_t)r * words + word] & canonical_x[word];
        if ((int)mzd_read_bit(y, r, 0) != bit_parity(parity_words)) {
            fprintf(stderr, "mismatch: rows=%" PRIu32 " cols=%" PRIu32 " row=%" PRIu32 "\n",
                    (uint32_t)shape.rows, (uint32_t)shape.cols, (uint32_t)r);
            goto out;
        }
    }
    printf("ok rows=%" PRIu32 " cols=%" PRIu32 " words=%zu\n",
           (uint32_t)shape.rows, (uint32_t)shape.cols, words);
    ok = 1;

out:
    if (y != NULL) mzd_free(y);
    if (x != NULL) mzd_free(x);
    if (a != NULL) mzd_free(a);
    free(canonical_x);
    free(canonical_rows);
    return ok;
}

int main(void) {
    /* 63/64/65 preserve the public tail boundary; 512 and 4096 are 8 and
     * 64 canonical u64 words, the story's dense mid-range input band. */
    static const probe_case cases[] = {
        {1, 1}, {63, 63}, {64, 64}, {65, 65}, {65, 512}, {65, 4096},
    };
    uint64_t seed = UINT64_C(0x92385645c0decafe);
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i)
        if (!run_case(cases[i], seed + i)) return EXIT_FAILURE;
    return EXIT_SUCCESS;
}
