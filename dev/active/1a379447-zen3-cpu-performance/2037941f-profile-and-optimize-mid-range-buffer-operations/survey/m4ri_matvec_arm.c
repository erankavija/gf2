/*
 * External M4RI arm of the dense-parity comparator (jit:e1f9a78f).
 *
 * `mzd_read_bit` and `mzd_write_bit` are static inline in `m4ri/mzd.h`, so the
 * public coordinate interface the matched-operation specification names is
 * reachable only from a C translation unit. This unit exposes the two charged
 * boundaries the specification defines and nothing else: it holds no timing
 * loop and makes no production-selection claim.
 *
 * The caller supplies the canonical gf2 words of the matrix and the vector and
 * a zeroed output word buffer; every `mzd_t` this unit creates it also frees.
 */
#include <stdint.h>
#include <stdlib.h>

#include <m4ri/m4ri.h>

/// Retained converted inputs of a retained-state cell.
typedef struct {
    mzd_t *a;
    mzd_t *x;
    int rows;
} gf2_m4ri_state;

/* The exported boundary, declared before use so the arm and this unit agree. */
int gf2_m4ri_fresh_call(const uint64_t *rows_words, const uint64_t *x_words, int nrows, int ncols,
                        uint64_t *y_words);
gf2_m4ri_state *gf2_m4ri_retain(const uint64_t *rows_words, const uint64_t *x_words, int nrows,
                                int ncols);
int gf2_m4ri_retained_call(const gf2_m4ri_state *state, uint64_t *y_words);
void gf2_m4ri_release(gf2_m4ri_state *state);

static int bit_of(const uint64_t *words, int index) {
    return (int)((words[(size_t)index >> 6] >> ((unsigned)index & 63U)) & UINT64_C(1));
}

/* Packs the canonical gf2 words into public M4RI coordinates. */
static void pack(mzd_t *a, mzd_t *x, const uint64_t *rows_words, const uint64_t *x_words,
                 int nrows, int ncols) {
    const size_t words = ((size_t)ncols + 63U) / 64U;
    for (rci_t r = 0; r < nrows; ++r)
        for (rci_t c = 0; c < ncols; ++c)
            if (bit_of(rows_words + (size_t)r * words, c)) mzd_write_bit(a, r, c, 1);
    for (rci_t c = 0; c < ncols; ++c)
        if (bit_of(x_words, c)) mzd_write_bit(x, c, 0, 1);
}

/* Unpacks the product back into canonical gf2 words. */
static void unpack(const mzd_t *y, uint64_t *y_words, int nrows) {
    for (rci_t r = 0; r < nrows; ++r)
        if (mzd_read_bit(y, r, 0))
            y_words[(size_t)r >> 6] |= UINT64_C(1) << ((unsigned)r & 63U);
}

/*
 * One fresh whole-consumer call: initialization, packing, the matched product,
 * unpacking, and disposal of every owned `mzd_t`.
 */
int gf2_m4ri_fresh_call(const uint64_t *rows_words, const uint64_t *x_words, int nrows, int ncols,
                        uint64_t *y_words) {
    int ok = 0;
    mzd_t *a = mzd_init(nrows, ncols);
    mzd_t *x = mzd_init(ncols, 1);
    mzd_t *y = mzd_init(nrows, 1);
    if (a == NULL || x == NULL || y == NULL) goto out;
    mzd_set_ui(a, 0);
    mzd_set_ui(x, 0);
    mzd_set_ui(y, 0);
    pack(a, x, rows_words, x_words, nrows, ncols);
    if (mzd_mul(y, a, x, 0) != y) goto out;
    unpack(y, y_words, nrows);
    ok = 1;
out:
    if (y != NULL) mzd_free(y);
    if (x != NULL) mzd_free(x);
    if (a != NULL) mzd_free(a);
    return ok ? 0 : -1;
}

/* Converts and retains the inputs of a retained-state cell, outside timing. */
gf2_m4ri_state *gf2_m4ri_retain(const uint64_t *rows_words, const uint64_t *x_words, int nrows,
                                int ncols) {
    gf2_m4ri_state *state = calloc(1, sizeof(*state));
    if (state == NULL) return NULL;
    state->rows = nrows;
    state->a = mzd_init(nrows, ncols);
    state->x = mzd_init(ncols, 1);
    if (state->a == NULL || state->x == NULL) {
        gf2_m4ri_release(state);
        return NULL;
    }
    mzd_set_ui(state->a, 0);
    mzd_set_ui(state->x, 0);
    pack(state->a, state->x, rows_words, x_words, nrows, ncols);
    return state;
}

/* One retained-state call: `y` alone is created, filled, unpacked and freed. */
int gf2_m4ri_retained_call(const gf2_m4ri_state *state, uint64_t *y_words) {
    int ok = 0;
    mzd_t *y = mzd_init(state->rows, 1);
    if (y == NULL) return -1;
    mzd_set_ui(y, 0);
    if (mzd_mul(y, state->a, state->x, 0) != y) goto out;
    unpack(y, y_words, state->rows);
    ok = 1;
out:
    mzd_free(y);
    return ok ? 0 : -1;
}

/* Releases retained state. */
void gf2_m4ri_release(gf2_m4ri_state *state) {
    if (state == NULL) return;
    if (state->x != NULL) mzd_free(state->x);
    if (state->a != NULL) mzd_free(state->a);
    free(state);
}
