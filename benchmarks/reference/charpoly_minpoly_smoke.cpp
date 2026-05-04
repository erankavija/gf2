// benchmarks/reference/charpoly_minpoly_smoke.cpp
//
// Cross-library bitwise-equality oracle for the charpoly + minpoly cells
// promoted in issue c3e79272 (Wave 3 of epic 97bf0879). Complements
// `ntl_flint_smoke.cpp` (which already covers `charpoly` between NTL
// and FLINT) by adding bitwise polynomial-coefficient equality for
// minpoly (which NTL does not expose at the user-facing API level —
// see dev/plans/ntl_promotion_evidence.md § Scope of promotion).
//
// Per `dev/plans/sota_reference_acceptance_protocol.md` § 6
// *Correctness-oracle harness*, every promoted (operation, field) cell
// must be smoke-asserted at n=16 against a fixed-seeded input. The
// existing per-library smokes cover internal contracts (LinBox:
// Cayley-Hamilton p(A)=0; FLINT: minpoly | charpoly idempotency); this
// binary adds the cross-library bitwise-equality contract that the
// protocol § 6 row for `charpoly` and `minpoly` requires:
//
//   "Equality of the characteristic [/ minimal] polynomial as a vector
//    of coefficients in canonical form, leading coefficient = 1, monic."
//
// Operations smoke-checked, at n=16, across LinBox + FLINT, for each
// of the four reference primes {GF(7), GF(251), GF(65521), GF(2^31-1)}:
//
//   * charpoly: bitwise equality of coefficient vectors, deg=n,
//     monic-leading-coefficient verified on both sides.
//   * minpoly: bitwise equality of coefficient vectors, deg≤n,
//     monic-leading-coefficient verified on both sides.
//
// The seed for each cell is derived via the shared
// `gf2_bench_derive_seed(master, tag, op_idx, size_idx, regime_idx)`
// helper from `seed_helpers.h` with the same `(op_idx, size_idx,
// regime_idx)` triples (and per-field master XOR pattern) that
// `linbox_bench` and `flint_bench` use for their n=64 timing rows. The
// smoke uses `size_idx = 0` (the slot that maps to `n=16` in the smoke
// configuration) so the cross-library seed and matrix bytes line up
// without re-implementing the index→size map.
//
// Exit non-zero on any mismatch with a per-cell stderr breadcrumb
// naming the (op, field, n) tuple that diverged.
//
// Build: see `benchmarks/reference/Makefile` target
// `charpoly_minpoly_smoke`. Linked against both LinBox and FLINT.

#include <linbox/ring/modular.h>
#include <linbox/matrix/dense-matrix.h>
#include <linbox/solutions/charpoly.h>
#include <linbox/solutions/minpoly.h>
#include <linbox/solutions/methods.h>

#include <givaro/modular.h>

#include <flint/flint.h>
#include <flint/nmod.h>
#include <flint/nmod_mat.h>
#include <flint/nmod_poly.h>

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include "seed_helpers.h"

namespace {

// Fill both libraries' n×n matrices from the same SplitMix64 stream so
// the input is byte-identical (mod p) on both sides. The Givaro field
// is the LinBox/Modular<int64_t> path; FLINT uses nmod_mat with the
// same prime modulus.
template <typename Field>
void fill_both(const Field& F,
               typename Field::Element_ptr A_lb,   // LinBox BlasMatrix backing buffer
               nmod_mat_t A_fl,
               long n,
               uint64_t seed) {
    typename Field::Residu_t card = F.cardinality();
    ulong p = (ulong)card;
    uint64_t st = seed;
    for (long i = 0; i < n; ++i)
        for (long j = 0; j < n; ++j) {
            uint64_t r = gf2_bench_splitmix64(&st);
            uint64_t v = r % (uint64_t)p;
            typename Field::Element x;
            F.init(x, (int64_t)v);
            A_lb[i * n + j] = x;
            nmod_mat_set_entry(A_fl, i, j, (ulong)v);
        }
}

// Compare a LinBox-style coefficient vector (low→high) against a FLINT
// nmod_poly (low→high). LinBox's charpoly/minpoly returns coefficients
// in `std::vector<Field::Element>` ordered from constant term up to
// leading; FLINT's `nmod_poly_get_coeff_ui(p, i)` is also indexed
// constant→leading. Both libraries return monic polynomials by
// construction.
template <typename Field>
bool poly_equal(const Field& F,
                const std::vector<typename Field::Element>& lb_coeffs,
                const nmod_poly_t fl,
                const char* op_label,
                const char* field_label,
                long n) {
    long lb_deg = (long)lb_coeffs.size() - 1;
    long fl_deg = (long)nmod_poly_degree(fl);
    if (lb_deg != fl_deg) {
        std::fprintf(stderr,
            "[smoke] FAIL %s %s n=%ld degree mismatch: lb=%ld fl=%ld\n",
            op_label, field_label, n, lb_deg, fl_deg);
        return false;
    }
    // Verify monic-leading: both libraries should report 1 at index `lb_deg`.
    typename Field::Element one;
    F.init(one, 1);
    if (!F.areEqual(lb_coeffs.back(), one)) {
        std::fprintf(stderr,
            "[smoke] FAIL %s %s n=%ld linbox not monic\n",
            op_label, field_label, n);
        return false;
    }
    if (nmod_poly_get_coeff_ui(fl, fl_deg) != 1) {
        std::fprintf(stderr,
            "[smoke] FAIL %s %s n=%ld flint not monic\n",
            op_label, field_label, n);
        return false;
    }
    // Bitwise per-coefficient equality after canonicalisation. Both
    // sides reduce to [0, p) by construction (LinBox uses
    // Givaro::Modular<int64_t>'s canonical form; FLINT's nmod_poly
    // stores coefficients reduced into [0, p)).
    for (long i = 0; i <= lb_deg; ++i) {
        uint64_t lb_v = (uint64_t)(int64_t)lb_coeffs[i];
        ulong fl_v = nmod_poly_get_coeff_ui(fl, i);
        if (lb_v != (uint64_t)fl_v) {
            std::fprintf(stderr,
                "[smoke] FAIL %s %s n=%ld coeff[%ld]: lb=%llu fl=%lu\n",
                op_label, field_label, n, i,
                (unsigned long long)lb_v, fl_v);
            return false;
        }
    }
    return true;
}

template <typename Field>
int check_charpoly(const Field& F,
                   const char* field_label,
                   long n,
                   uint64_t seed) {
    nmod_mat_t A_fl;
    nmod_mat_init(A_fl, n, n, (ulong)F.cardinality());
    std::vector<typename Field::Element> A_lb_buf((size_t)n * (size_t)n);
    fill_both(F, A_lb_buf.data(), A_fl, n, seed);

    LinBox::DenseMatrix<Field> M(F, n, n);
    for (long i = 0; i < n; ++i)
        for (long j = 0; j < n; ++j)
            M.setEntry(i, j, A_lb_buf[i * n + j]);

    std::vector<typename Field::Element> lb_charp;
    LinBox::charpoly(lb_charp, M, LinBox::Method::Auto());

    nmod_poly_t fl_charp;
    nmod_poly_init(fl_charp, (ulong)F.cardinality());
    nmod_mat_charpoly(fl_charp, A_fl);

    bool ok = poly_equal(F, lb_charp, fl_charp, "charpoly", field_label, n);
    std::fprintf(stderr, "[smoke] %s charpoly    : %s (lb_deg=%zu fl_deg=%ld)\n",
                 field_label, ok ? "ok" : "FAIL",
                 lb_charp.size() ? lb_charp.size() - 1 : (size_t)0,
                 (long)nmod_poly_degree(fl_charp));

    nmod_poly_clear(fl_charp);
    nmod_mat_clear(A_fl);
    return ok ? 0 : 1;
}

template <typename Field>
int check_minpoly(const Field& F,
                  const char* field_label,
                  long n,
                  uint64_t seed) {
    nmod_mat_t A_fl;
    nmod_mat_init(A_fl, n, n, (ulong)F.cardinality());
    std::vector<typename Field::Element> A_lb_buf((size_t)n * (size_t)n);
    fill_both(F, A_lb_buf.data(), A_fl, n, seed);

    LinBox::DenseMatrix<Field> M(F, n, n);
    for (long i = 0; i < n; ++i)
        for (long j = 0; j < n; ++j)
            M.setEntry(i, j, A_lb_buf[i * n + j]);

    std::vector<typename Field::Element> lb_minp;
    LinBox::minpoly(lb_minp, M, LinBox::Method::Auto());

    nmod_poly_t fl_minp;
    nmod_poly_init(fl_minp, (ulong)F.cardinality());
    nmod_mat_minpoly(fl_minp, A_fl);

    bool ok = poly_equal(F, lb_minp, fl_minp, "minpoly", field_label, n);
    std::fprintf(stderr, "[smoke] %s minpoly     : %s (lb_deg=%zu fl_deg=%ld)\n",
                 field_label, ok ? "ok" : "FAIL",
                 lb_minp.size() ? lb_minp.size() - 1 : (size_t)0,
                 (long)nmod_poly_degree(fl_minp));

    nmod_poly_clear(fl_minp);
    nmod_mat_clear(A_fl);
    return ok ? 0 : 1;
}

// Per-field driver.
//
// The XOR pattern (master_seed ^ {0x33,0x22,0x11,0}) and the tag/op_idx
// pairs ("charpoly", 5) and ("minpoly", 6) match the convention used by
// `linbox_bench` / `flint_bench` so the smoke seeds for size_idx=0 are
// the same `derive_seed(...)` outputs those harnesses produce. We use
// size_idx=0 (the slot that maps to n=16 in the smoke configuration of
// each harness — both `flint_bench --smoke` and `linbox_bench --smoke`
// use `size_idx=0` for their n=16 inputs).
template <typename Field>
int check_field(long p, const char* field_label, uint64_t master) {
    Field F(p);
    constexpr long n = 16;
    int fails = 0;
    fails += check_charpoly(F, field_label, n,
                            gf2_bench_derive_seed(master, "charpoly", 5, 0, 0));
    fails += check_minpoly(F, field_label, n,
                           gf2_bench_derive_seed(master, "minpoly", 6, 0, 0));
    return fails;
}

}  // namespace

int main() {
    flint_set_num_threads(1);
    using Field = Givaro::Modular<int64_t>;
    const uint64_t master = 0x6F73AC91D31E4A7CULL;

    int fails = 0;
    fails += check_field<Field>(7,             "GF(7)       ", master ^ 0x33ULL);
    fails += check_field<Field>(251,           "GF(251)     ", master ^ 0x22ULL);
    fails += check_field<Field>(65521,         "GF(65521)   ", master ^ 0x11ULL);
    fails += check_field<Field>((1L << 31) - 1, "GF(2^31-1)  ", master);
    flint_cleanup();

    if (fails != 0) {
        std::fprintf(stderr,
            "[smoke] charpoly_minpoly_smoke FAILED with %d cell(s)\n",
            fails);
        return 1;
    }
    std::fprintf(stderr, "[smoke] charpoly_minpoly_smoke OK\n");
    return 0;
}
