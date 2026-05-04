// benchmarks/reference/ntl_flint_smoke.cpp
//
// Cross-equality oracle for NTL and FLINT on the per-operation
// contracts in dev/plans/sota_reference_acceptance_protocol.md § 6.
//
// At n=16, for each (operation, field) cell that BOTH libraries cover,
// this binary fills the same SplitMix64-derived input matrix in both
// libraries' native types, runs the operation, and asserts bitwise
// equality of the canonical-form output.
//
// Operations smoke-checked:
//   * fgemm (mul / nmod_mat_mul)
//   * inv (zz_p inv / nmod_mat_inv)
//   * solve (NTL solve / nmod_mat_solve)
//   * charpoly (NTL CharPoly / nmod_mat_charpoly)
//
// Operations only one library covers (FLINT pluq / echelon / minpoly)
// are smoke-checked against an internal idempotency invariant rather
// than cross-library equality:
//   * pluq idempotency: nmod_mat_lu reports rank == n on a uniform
//     square matrix over GF(p) (probabilistically full-rank).
//   * echelon idempotency: nmod_mat_rref(A) is fixed-point under a
//     second nmod_mat_rref call (RREF is unique).
//   * minpoly: minpoly | charpoly (the minimal polynomial divides the
//     characteristic polynomial — the canonical Cayley-Hamilton check).
//
// Exit non-zero on any mismatch. Prints a one-line "ok"/"fail" per
// cell to stderr; emits no stdout (so the smoke runner can ignore
// this binary's output entirely).
//
// Singular-resample policy (inv, solve): a uniform-random n=16 matrix
// over GF(p) is non-singular with probability ~1 - O(1/p), but the
// seeded sample CAN turn out singular for the smallest primes in
// scope. To eliminate silent skips we re-derive the seed via
// SplitMix64 and retry up to 3 times. If all 3 attempts are singular
// the cell counts as FAIL — at n=16 the probability of three
// independent failures is bounded by (1/p)^3 ≤ (1/7)^3 ≈ 3·10^-3
// for the worst case GF(7), and far smaller for the other fields,
// so a triple miss is treated as a real bug, not a non-event.
//
// Build: see benchmarks/reference/Makefile target ntl_flint_smoke.

#include <NTL/lzz_p.h>
#include <NTL/lzz_pX.h>
#include <NTL/mat_lzz_p.h>
#include <NTL/mat_poly_lzz_p.h>
#include <NTL/vec_lzz_p.h>

#include <flint/flint.h>
#include <flint/nmod.h>
#include <flint/nmod_mat.h>
#include <flint/nmod_poly.h>

#include <cstdint>
#include <cstdio>
#include <cstdlib>

#include "seed_helpers.h"

namespace {

using NTL::zz_p;
using NTL::zz_pX;
using NTL::mat_zz_p;
using NTL::vec_zz_p;
using NTL::to_zz_p;
using NTL::rep;

// Fill both libraries' n×n matrices from the same SplitMix64 stream.
// `zz_p::init(p)` must already have been called.
void fill_both(mat_zz_p& A, nmod_mat_t F, long n, uint64_t seed) {
    A.SetDims(n, n);
    long p = zz_p::modulus();
    uint64_t st = seed;
    for (long i = 0; i < n; ++i)
        for (long j = 0; j < n; ++j) {
            uint64_t r = gf2_bench_splitmix64(&st);
            uint64_t v = r % static_cast<uint64_t>(p);
            A[i][j] = to_zz_p(static_cast<long>(v));
            nmod_mat_set_entry(F, i, j, static_cast<ulong>(v));
        }
}

void fill_both_vec(vec_zz_p& v, ulong* fv, long n, uint64_t seed) {
    v.SetLength(n);
    long p = zz_p::modulus();
    uint64_t st = seed;
    for (long i = 0; i < n; ++i) {
        uint64_t r = gf2_bench_splitmix64(&st);
        uint64_t x = r % static_cast<uint64_t>(p);
        v[i] = to_zz_p(static_cast<long>(x));
        fv[i] = static_cast<ulong>(x);
    }
}

bool mat_equal(const mat_zz_p& A, const nmod_mat_t F, long n) {
    for (long i = 0; i < n; ++i)
        for (long j = 0; j < n; ++j)
            if (static_cast<ulong>(rep(A[i][j])) != nmod_mat_get_entry(F, i, j))
                return false;
    return true;
}

int check_field(long p, const char* label) {
    const long n = 16;
    int fails = 0;

    zz_p::init(p);

    // ----- mul -----
    {
        mat_zz_p A_n, B_n, C_n;
        nmod_mat_t A_f, B_f, C_f;
        nmod_mat_init(A_f, n, n, static_cast<ulong>(p));
        nmod_mat_init(B_f, n, n, static_cast<ulong>(p));
        nmod_mat_init(C_f, n, n, static_cast<ulong>(p));
        fill_both(A_n, A_f, n, 0xA1A1A1A1ULL);
        fill_both(B_n, B_f, n, 0xB2B2B2B2ULL);
        mul(C_n, A_n, B_n);
        nmod_mat_mul(C_f, A_f, B_f);
        bool ok = mat_equal(C_n, C_f, n);
        std::fprintf(stderr, "[smoke] %s mul         : %s\n", label, ok ? "ok" : "FAIL");
        if (!ok) ++fails;
        nmod_mat_clear(A_f);
        nmod_mat_clear(B_f);
        nmod_mat_clear(C_f);
    }

    // ----- inv -----
    //
    // Singular-resample policy: at n=16 over GF(p) a uniform-random
    // matrix is non-singular with overwhelming probability (~1 - 1/p
    // for the smallest prime in scope), but it is not impossible. To
    // ensure every cell either passes or fails (no silent skips), we
    // re-derive the seed via SplitMix64 on each retry up to 3 attempts.
    // If all 3 are singular we count the cell as FAIL — at n=16 the
    // probability is astronomically small, so a triple miss indicates
    // a real bug (e.g., a bench library returning singular for a
    // valid input) and must surface, not skip.
    {
        mat_zz_p A_n, X_n;
        nmod_mat_t A_f, X_f;
        nmod_mat_init(A_f, n, n, static_cast<ulong>(p));
        nmod_mat_init(X_f, n, n, static_cast<ulong>(p));
        const int max_retries = 3;
        int attempt = 0;
        bool ntl_ok = false;
        int flint_ok = 0;
        uint64_t inv_seed = 0xC3C3C3C3ULL;
        for (; attempt < max_retries; ++attempt) {
            fill_both(A_n, A_f, n, inv_seed);
            ntl_ok = true;
            try { inv(X_n, A_n); } catch (...) { ntl_ok = false; }
            flint_ok = nmod_mat_inv(X_f, A_f);
            if (ntl_ok && flint_ok) break;
            // Re-derive the seed deterministically before retry.
            uint64_t st = inv_seed;
            inv_seed = gf2_bench_splitmix64(&st);
        }
        if (!ntl_ok || !flint_ok) {
            std::fprintf(stderr,
                "[smoke] %s inv         : FAIL (singular after %d retries: "
                "ntl=%d flint=%d)\n",
                label, max_retries, !ntl_ok, !flint_ok);
            ++fails;
        } else {
            bool ok = mat_equal(X_n, X_f, n);
            std::fprintf(stderr,
                "[smoke] %s inv         : %s (attempt=%d)\n",
                label, ok ? "ok" : "FAIL", attempt + 1);
            if (!ok) ++fails;
        }
        nmod_mat_clear(A_f);
        nmod_mat_clear(X_f);
    }

    // ----- solve (n×n A, n-vector b) -----
    //
    // For a non-singular A, the solution to A·x = b is unique. We
    // therefore verify A·x ≡ b for both libraries' solutions
    // independently rather than requiring x_ntl == x_flint
    // (which would force the implementations to agree on internal
    // pivot ordering and is not part of the protocol §6 contract).
    // The `solve` row in protocol §6 explicitly notes "equality is
    // on the **specific** particular solution by basis convention".
    // For a square non-singular system the basis convention is
    // immaterial: the unique solution agrees regardless. We verify
    // it via residual A·x - b == 0 in both libraries.
    {
        mat_zz_p A_n;
        vec_zz_p b_n, x_n, residual_n;
        nmod_mat_t A_f, B_f, X_f, R_f;
        nmod_mat_init(A_f, n, n, static_cast<ulong>(p));
        nmod_mat_init(B_f, n, 1, static_cast<ulong>(p));
        nmod_mat_init(X_f, n, 1, static_cast<ulong>(p));
        nmod_mat_init(R_f, n, 1, static_cast<ulong>(p));
        // Singular-resample policy mirrors the inv block above. Both A
        // and the RHS b are re-derived together on retry so the (A, b)
        // pair stays correlated to a single seed pair as the harness
        // documents in `seed_helpers.h`.
        const int max_retries = 3;
        int attempt = 0;
        bool ntl_ok = false;
        int flint_ok = 0;
        uint64_t a_seed = 0xD4D4D4D4ULL;
        uint64_t b_seed = 0xE5E5E5E5ULL;
        ulong b_arr[16];
        zz_p d_ntl;
        for (; attempt < max_retries; ++attempt) {
            fill_both(A_n, A_f, n, a_seed);
            fill_both_vec(b_n, b_arr, n, b_seed);
            for (long i = 0; i < n; ++i)
                nmod_mat_set_entry(B_f, i, 0, b_arr[i]);
            ntl_ok = false;
            try {
                // Column-vector convention: solve(d, A, x, b) computes
                // x with A*x = b. The other overload (d, x, A, b) does
                // the row-vector x*A = b which is NOT what the protocol
                // §6 contract describes. See NTL doc/mat_lzz_p.txt.
                solve(d_ntl, A_n, x_n, b_n);
                ntl_ok = (rep(d_ntl) != 0);
            } catch (...) { ntl_ok = false; }
            flint_ok = nmod_mat_solve(X_f, A_f, B_f);
            if (ntl_ok && flint_ok) break;
            uint64_t sa = a_seed, sb = b_seed;
            a_seed = gf2_bench_splitmix64(&sa);
            b_seed = gf2_bench_splitmix64(&sb);
        }
        if (!ntl_ok || !flint_ok) {
            std::fprintf(stderr,
                "[smoke] %s solve       : FAIL (singular after %d retries: "
                "ntl=%d flint=%d)\n",
                label, max_retries, !ntl_ok, !flint_ok);
            ++fails;
        } else {
            // NTL residual: A·x - b
            bool ntl_resid_ok = true;
            mul(residual_n, A_n, x_n);
            for (long i = 0; i < n; ++i) {
                if (rep(residual_n[i] - b_n[i]) != 0) { ntl_resid_ok = false; break; }
            }
            // FLINT residual: A·x - b
            nmod_mat_mul(R_f, A_f, X_f);
            bool flint_resid_ok = true;
            for (long i = 0; i < n; ++i) {
                ulong axi = nmod_mat_get_entry(R_f, i, 0);
                ulong bi  = nmod_mat_get_entry(B_f, i, 0);
                if (axi != bi) { flint_resid_ok = false; break; }
            }
            // Both libraries must satisfy A·x = b independently. For
            // non-singular A the solution is unique, so x_ntl and
            // x_flint must additionally agree.
            bool ntl_eq_flint = true;
            for (long i = 0; i < n; ++i) {
                if (static_cast<ulong>(rep(x_n[i])) != nmod_mat_get_entry(X_f, i, 0)) {
                    ntl_eq_flint = false;
                    break;
                }
            }
            bool ok = ntl_resid_ok && flint_resid_ok && ntl_eq_flint;
            std::fprintf(stderr,
                "[smoke] %s solve       : %s (attempt=%d ntl_residual=%d "
                "flint_residual=%d x_ntl==x_flint=%d)\n",
                label, ok ? "ok" : "FAIL", attempt + 1,
                (int)ntl_resid_ok, (int)flint_resid_ok, (int)ntl_eq_flint);
            if (!ok) ++fails;
        }
        nmod_mat_clear(A_f);
        nmod_mat_clear(B_f);
        nmod_mat_clear(X_f);
        nmod_mat_clear(R_f);
    }

    // ----- charpoly (NTL CharPoly vs FLINT nmod_mat_charpoly) -----
    {
        mat_zz_p A_n;
        nmod_mat_t A_f;
        nmod_mat_init(A_f, n, n, static_cast<ulong>(p));
        fill_both(A_n, A_f, n, 0xF6F6F6F6ULL);
        zz_pX f_n;
        CharPoly(f_n, A_n);
        nmod_poly_t f_f;
        nmod_poly_init(f_f, static_cast<ulong>(p));
        nmod_mat_charpoly(f_f, A_f);
        bool ok = (deg(f_n) == nmod_poly_degree(f_f));
        for (long i = 0; ok && i <= n; ++i) {
            ulong nv = static_cast<ulong>(rep(coeff(f_n, i)));
            ulong fv = nmod_poly_get_coeff_ui(f_f, i);
            if (nv != fv) ok = false;
        }
        std::fprintf(stderr, "[smoke] %s charpoly    : %s\n", label, ok ? "ok" : "FAIL");
        if (!ok) ++fails;
        nmod_poly_clear(f_f);
        nmod_mat_clear(A_f);
    }

    // ----- pluq full-rank check (FLINT-only) -----
    {
        nmod_mat_t A_f;
        nmod_mat_init(A_f, n, n, static_cast<ulong>(p));
        // Reuse the mul A seed, sourced through the same SplitMix64 stream.
        mat_zz_p dummy;
        fill_both(dummy, A_f, n, 0xA1A1A1A1ULL);
        slong P[16];
        slong rank = nmod_mat_lu(P, A_f, /*rank_check=*/0);
        bool ok = (rank == n);
        std::fprintf(stderr,
            "[smoke] %s pluq        : %s (rank=%ld expected=%ld)\n",
            label, ok ? "ok" : "FAIL", (long)rank, (long)n);
        if (!ok) ++fails;
        nmod_mat_clear(A_f);
    }

    // ----- echelon (RREF) idempotency (FLINT-only) -----
    {
        nmod_mat_t A_f, A_f2;
        nmod_mat_init(A_f, n, n, static_cast<ulong>(p));
        nmod_mat_init(A_f2, n, n, static_cast<ulong>(p));
        mat_zz_p dummy;
        fill_both(dummy, A_f, n, 0xA1A1A1A1ULL);
        nmod_mat_rref(A_f);
        nmod_mat_set(A_f2, A_f);
        nmod_mat_rref(A_f2);
        bool ok = nmod_mat_equal(A_f, A_f2);
        std::fprintf(stderr,
            "[smoke] %s echelon     : %s (idempotent=%d)\n",
            label, ok ? "ok" : "FAIL", (int)ok);
        if (!ok) ++fails;
        nmod_mat_clear(A_f);
        nmod_mat_clear(A_f2);
    }

    // ----- minpoly | charpoly (Cayley-Hamilton divisibility) -----
    {
        nmod_mat_t A_f;
        nmod_mat_init(A_f, n, n, static_cast<ulong>(p));
        mat_zz_p dummy;
        fill_both(dummy, A_f, n, 0xF6F6F6F6ULL);
        nmod_poly_t cf, mf, q, r;
        nmod_poly_init(cf, static_cast<ulong>(p));
        nmod_poly_init(mf, static_cast<ulong>(p));
        nmod_poly_init(q,  static_cast<ulong>(p));
        nmod_poly_init(r,  static_cast<ulong>(p));
        nmod_mat_charpoly(cf, A_f);
        nmod_mat_minpoly(mf, A_f);
        nmod_poly_divrem(q, r, cf, mf);
        bool ok = nmod_poly_is_zero(r);
        std::fprintf(stderr,
            "[smoke] %s minpoly     : %s (minpoly | charpoly = %d)\n",
            label, ok ? "ok" : "FAIL", (int)ok);
        if (!ok) ++fails;
        nmod_poly_clear(cf);
        nmod_poly_clear(mf);
        nmod_poly_clear(q);
        nmod_poly_clear(r);
        nmod_mat_clear(A_f);
    }

    return fails;
}

}  // namespace

int main() {
    flint_set_num_threads(1);
    int fails = 0;
    fails += check_field(7,             "GF(7)       ");
    fails += check_field(251,           "GF(251)     ");
    fails += check_field(65521,         "GF(65521)   ");
    fails += check_field((1L << 31) - 1, "GF(2^31-1)  ");
    flint_cleanup();
    if (fails != 0) {
        std::fprintf(stderr, "[smoke] FAILED with %d cell(s)\n", fails);
        return 1;
    }
    std::fprintf(stderr, "[smoke] OK\n");
    return 0;
}
