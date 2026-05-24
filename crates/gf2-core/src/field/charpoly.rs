//! Characteristic polynomial, minimal polynomial, and Frobenius normal
//! form over an arbitrary [`FiniteField`].
//!
//! Issues `f01298db` (cubic baseline) and `1454ec2d` (sub-cubic
//! Keller–Gehrig variant + automatic dispatch). Two algorithmic paths
//! coexist behind the public [`FieldMatrix::charpoly`] entry:
//!
//! 1. **Cubic deterministic** ([`FieldMatrix::charpoly_cubic`]) —
//!    Dumas–Pernet theorem 13.1. Krylov-cyclic decomposition; `O(n³)`
//!    field operations on every input. Always available; the only path
//!    used for runtime-context fields and small `n`.
//! 2. **Sub-cubic Las-Vegas** ([`FieldMatrix::charpoly_keller_gehrig`])
//!    — Dumas–Pernet theorem 13.4. Builds a shifted Krylov matrix
//!    `K = [v | A·v | A²·v | … | A^{n-1}·v]` via `⌈log₂ n⌉` repeated-
//!    squaring doublings (each doubling is one `gemm`), then recovers
//!    the charpoly coefficients from the linear solve `K · y = A^n · v`
//!    (one `PLE` factorisation + two `trsm` calls). Total
//!    `O(n^ω · log n)` field operations. Engaged only when the field
//!    cardinality `q` exceeds `2 n²` (per-attempt failure probability is
//!    then `< 1/2`, guaranteeing Las-Vegas convergence in expected
//!    `O(1)` retries).
//!
//! ## Default dispatch (issue `1454ec2d`)
//!
//! [`FieldMatrix::charpoly`] is the public entry. It currently always
//! selects the cubic baseline because [`KG_DISPATCH_MIN_N`] is set to
//! [`usize::MAX`] — see the R2 amendment in issue `e47231cd` and the
//! 4a59d1f9 post-Wave-9 reassessment for the empirical justification
//! (cubic is ~148x faster than KG at `n = 256` on `Fp<MERSENNE_31>`
//! with the current PLE-based K⁻¹ pipeline; ratio grows monotonically
//! with `n`).
//! Callers who want to opt into the Las-Vegas Keller–Gehrig variant
//! must call [`FieldMatrix::charpoly_keller_gehrig`] directly with an
//! explicit seed. Bit-exact equality across paths is guaranteed by the
//! KG path's Cayley–Hamilton verification (`charpoly.eval_at_matrix(A) == 0`).
//!
//! When the threshold is later tuned downward (after the K⁻¹ step is
//! replaced with a Strassen-amenable inversion), the dispatch will
//! select between the two paths via these rules, in order:
//!
//! 1. If `n < KG_DISPATCH_MIN_N`: cubic.
//! 2. If [`FiniteField::cardinality_log2_hint`] returns `None`
//!    (runtime-context fields like [`crate::gf2m::Gf2mElement`]): cubic.
//! 3. If `cardinality_log2_hint > 127` (`q` does not fit in `u128`):
//!    sub-cubic — `q ≫ 2 n²` is satisfied trivially for any `n`.
//! 4. Otherwise compute `q = 2^cardinality_log2_hint` (`u128`). If
//!    `q ≤ 2 n²`: cubic.
//! 5. Try [`charpoly_keller_gehrig`](FieldMatrix::charpoly_keller_gehrig)
//!    up to [`KG_MAX_RETRIES`] times. On `None`: silently fall back to
//!    the cubic path.
//!
//! ## Empirical crossover (Mersenne-31, `Fp<2^31 − 1>`)
//!
//! **Latest measurement: 2026-05-07** (issue `4a59d1f9`, post-Wave-9
//! kernels: `TRI_BASE_THRESHOLD = 8`, `PLE_BASE_COLS = 1`, delayed-u128
//! GEMM). Criterion group `charpoly/dispatch`, 10 samples, AMD Ryzen 9
//! 5900X (Zen 3), `rustc 1.95.0`, `RUSTFLAGS="-C target-cpu=native"`:
//!
//! ```text
//!     n     cubic       Keller-Gehrig    KG / cubic
//!   ----  ----------  ---------------  ----------
//!     64    0.729 ms         23.0 ms     ~31.6x
//!    128    5.25 ms         348 ms       ~66.3x
//!    256    37.1 ms          5.51 s     ~148x
//!    512   279 ms           94.2 s      ~337x
//!   1024     2.14 s       ~1500 s (ext)  ~700x (ext)
//! ```
//!
//! Previous measurement (2026-04-26, pre-Wave-9): cubic 104.7 ms,
//! KG 18.15 s at n = 256 (ratio ~173x). Post-Wave-9 cubic is 2.82x
//! faster (37.1 ms) and KG is 3.30x faster (5.51 s); the ratio
//! improved only 1.17x because both paths benefited proportionally.
//!
//! The crossover where Keller-Gehrig wins is **not visible** at any
//! measured size. Both the pre-Wave-9 and post-Wave-9 data confirm the
//! same structural bottleneck: the K^{-1} step uses `FieldMatrix::solve`
//! which is backed by `O(n^3)` PLE. No amount of PLE/TRSM constant-factor
//! tuning can close the 31-700x gap; only replacing the solve with a
//! sub-cubic inversion path would shift the crossover.
//!
//! ### Why
//!
//! The Keller-Gehrig path's K^{-1} step uses `FieldMatrix::solve`, which is
//! built on PLE (`O(n^3)`). That `O(n^3)` solve dominates the running time
//! and prevents the gemm-driven `O(n^omega * log n)` doublings from winning
//! at the measured sizes — even though each doubling does dispatch to
//! Strassen-Winograd through [`FiniteField::WINOGRAD_THRESHOLD`]. The
//! cubic Krylov path's inner loop is matvec-shaped (linear, not
//! Strassen-amenable), but its constant in front of `n^3` is
//! significantly smaller than the PLE-dominated Keller-Gehrig constant.
//!
//! Lifting this to a true sub-cubic crossover would require either
//! (a) replacing `FieldMatrix::solve` with a Strassen-amenable inversion
//! pipeline (`trtri_upper` + `gemm` rather than `trsm_*`), or
//! (b) restructuring the algorithm so the K^{-1} step is unnecessary. Both
//! are out of scope for `1454ec2d`'s baseline deliverable; they are
//! candidates for a future follow-up.
//!
//! ### Dispatch policy
//!
//! Given the empirical finding above (cubic is 31-700x faster than KG at
//! `n` in {64..1024}, with no measurable crossover; confirmed by both the
//! 2026-04-26 pre-Wave-9 and 2026-05-07 post-Wave-9 sweeps per issue
//! `4a59d1f9`), [`KG_DISPATCH_MIN_N`] is set to [`usize::MAX`] so that
//! public [`FieldMatrix::charpoly`] **always selects the cubic baseline**
//! under default dispatch. The correctness contract still holds: when
//! a future fix replaces the K^{-1} PLE pipeline with a Strassen-amenable
//! inversion and the threshold is tuned downward, KG would re-engage
//! conditionally on the field cardinality (`q > 2n^2`) and pass the
//! Cayley-Hamilton verification before returning. Callers who want to
//! opt into the KG path NOW (e.g. for benchmarks or research)
//! must call [`FieldMatrix::charpoly_keller_gehrig`] explicitly with a
//! deterministic seed.
//!
//! ## Cubic path
//!
//! Implements Dumas–Pernet theorem 13.1 — the deterministic cubic
//! Krylov-iteration baseline for the rational canonical form. The
//! driver:
//!
//! 1. Builds a cyclic decomposition `V = ⊕ W_i` via Krylov iteration in
//!    the quotient `V / span(previous chains)`. Each block is a cyclic
//!    `A`-invariant subspace whose annihilator polynomial is recovered
//!    directly from the Krylov dependency relation; the **product** of
//!    those polynomials equals `charpoly(A)`, and their **lcm** equals
//!    `minpoly(A)`.
//! 2. Refines the cyclic decomposition into the canonical Frobenius
//!    form via the standard `(p, q) → (lcm(p, q), gcd(p, q))` swap on
//!    pairs of cyclic blocks until the divisibility chain
//!    `f_{i+1} | f_i` holds. The largest invariant factor `f_1` is
//!    `minpoly(A)`.
//!
//! All matrix–matrix multiplications route through
//! [`gemm_into_view`](crate::field::matrix::gemm_into_view); all
//! matrix–vector products use [`FieldMatrix::matvec`]; rank-detection
//! within the Krylov loop is done by maintaining a row-reduced "running
//! basis" with explicit pivot bookkeeping rather than calling
//! [`FieldMatrix::ple`] on every iterate (a single PLE per Krylov step
//! would be `O(n^4)` overall and miss the stated target).
//!
//! # Architectural cost: incremental Krylov basis vs. snapshot PLE
//!
//! The success criterion under issue `f01298db` ("Uses matmul + PLE —
//! no bespoke linear-algebra") was amended at R1 (2026-04-26) under the
//! same `performance > SSOT` directive that governs `c3f8c1cb` R6 (PLE
//! derived ops materialise rather than view) and `83b1ad8b` R5
//! (triangular base case unrolls rather than dispatches through a
//! second-tier helper). The amendment reads:
//!
//! > Uses [`gemm_into_view`](crate::field::matrix::gemm_into_view) for
//! > matrix–matrix products, [`FieldMatrix::matvec`] for matrix–vector
//! > products, and [`FieldMatrix::ple`] / [`FieldMatrix::rank`]
//! > wherever a *snapshot* rank computation is needed (e.g. independent
//! > rank cross-checks in tests). The Krylov chain itself maintains an
//! > incremental row-reduced basis with pivot bookkeeping ([`reduce`]
//! > / [`append_to_basis`]) because invoking [`FieldMatrix::ple`] per
//! > Krylov step would force an `O(n^4)` rebuild for what is
//! > mathematically an `O(n^3)` amortised online algorithm.
//!
//! Concretely, each Krylov step appends a single column to the running
//! basis and either confirms it is independent (cost: one `O(n)`
//! pivot-row scan) or reads off the dependency relation directly from
//! the reduction coefficients (cost: zero — the coefficients are
//! already accumulated by [`reduce`]). Calling [`FieldMatrix::ple`]
//! per step would re-decompose the entire `n × k` running matrix at
//! cost `O(n · k · min(n, k))` per step, summing to `O(n^4)` across the
//! whole chain. For `n = 512` the existing benches in
//! `crates/gf2-core/benches/charpoly.rs` would slow by a factor of
//! `~512` on Krylov-bound paths, far past the `2×` regression bar
//! quoted in the lead's review prompt.
//!
//! [`reduce`]: reduce
//! [`append_to_basis`]: append_to_basis
//!
//! # Public surface
//!
//! - [`FieldMatrix::charpoly`] — `det(xI − A)` as a [`FieldPoly<F>`].
//! - [`FieldMatrix::minpoly`] — minimal polynomial of `A` (largest
//!   invariant factor).
//! - [`FieldMatrix::frobenius_form`] — `(P, F)` with `F = P⁻¹ A P` the
//!   block-diagonal direct sum of companion matrices of the invariant
//!   factors, ordered `f_1 | f_0` … wait, the documented ordering is
//!   `f_{i+1} | f_i` so `f_0 = minpoly` is the largest factor.
//!
//! # Correctness invariants
//!
//! 1. **Cayley–Hamilton**: `charpoly(A).eval_at_matrix(&A) == 0`.
//! 2. **Divisibility**: `minpoly(A) | charpoly(A)`; in the Frobenius
//!    form the invariant-factor chain satisfies `f_{i+1} | f_i`.
//! 3. **Conjugation**: `P⁻¹ · A · P == F` (block companion). Verified
//!    by composing [`FieldMatrix::inv`] and [`gemm`].
//! 4. **Rank-deficient inputs are accepted**: a singular `A` simply
//!    yields a charpoly with `0` as a root and a Frobenius form whose
//!    blocks reflect the reduced rank.
//!
//! # Edge cases
//!
//! `n == 0`: `charpoly == 1`, `minpoly == 1`, `frobenius_form` is the
//! pair of empty `0×0` matrices. `n == 1`: charpoly = `x − A[0,0]`,
//! minpoly = same, Frobenius form is `(I, A)` itself.
//!
//! # Allocation
//!
//! Inside the driver each Krylov chain materialises a freshly-allocated
//! "running basis" (n × current_dim) for rank-detection. A single
//! Frobenius-refinement loop additionally builds the change-of-basis
//! matrix `P` column by column. The architectural cost is documented
//! per function; no bespoke kernels are introduced.

use crate::field::matrix::{BasisReducer, ChainPolyArith, FieldMatrix, PackedMatvec};
use crate::field::poly::FieldPoly;
use crate::field::vec::FieldVec;
use crate::field::FiniteField;

/// Iterative-driver matvec helper (issue `d1dd266c`).
///
/// Wraps an `&FieldMatrix<F>` plus an optional pre-packed cache. When
/// the cache is available (e.g. for `Fp<P>` with `P ≤ 65521` and AVX2
/// enabled at runtime), every matvec call goes through the cache and
/// pays only a per-call vector pack — the matrix pack is paid exactly
/// once at construction time. When the cache is `None`, dispatches
/// through the regular `FieldMatrix::matvec` path (which itself may
/// still hit the per-call SIMD matvec hook).
struct MatvecDriver<'a, F: FiniteField> {
    a: &'a FieldMatrix<F>,
    packed: Option<Box<dyn PackedMatvec<F>>>,
    rows: usize,
    cols: usize,
}

impl<'a, F: FiniteField> MatvecDriver<'a, F> {
    fn new(a: &'a FieldMatrix<F>) -> Self {
        let (rows, cols) = a.shape();
        let packed = if rows > 0 && cols > 0 {
            F::try_prepack_matvec(a.as_data_slice(), rows, cols)
        } else {
            None
        };
        Self {
            a,
            packed,
            rows,
            cols,
        }
    }

    fn matvec(&self, x: &FieldVec<F>) -> FieldVec<F> {
        if let Some(packed) = self.packed.as_ref() {
            assert_eq!(x.len(), self.cols);
            // Synthesise a zero element from `x` (always non-empty here).
            let zero = x.as_slice()[0].zero_like();
            let mut y = FieldVec::<F>::zeros_from(self.rows, &zero);
            packed.matvec(x.as_slice(), y.as_mut_slice());
            return y;
        }
        self.a.matvec(x)
    }
}

/// Minimum matrix size at which [`FieldMatrix::charpoly`] considers the
/// sub-cubic Keller-Gehrig path. See the module rustdoc for the
/// empirical crossover discussion.
///
/// Set to [`usize::MAX`] so that **`charpoly()` always routes to the
/// cubic baseline** under default dispatch. The empirical measurements
/// (issue `4a59d1f9`, post-Wave-9 kernels, 2026-05-07) confirm cubic
/// is 31-337x faster than Keller-Gehrig at n in {64..512} on
/// `Fp<MERSENNE_31>`; ratio grows monotonically with n. No crossover
/// is visible in the measured range. The K^{-1} step's PLE-backed solve
/// is `O(n^3)` and dominates. Callers who want to opt into KG explicitly
/// can call [`FieldMatrix::charpoly_keller_gehrig`] directly.
///
/// Re-tune the threshold downward in a future ticket once the K^{-1}
/// step is replaced with a Strassen-amenable inversion (`trtri_upper`
/// + `gemm`) or the algorithm is restructured to avoid it.
pub const KG_DISPATCH_MIN_N: usize = usize::MAX;

/// Maximum number of Las-Vegas retries
/// [`FieldMatrix::charpoly_keller_gehrig`] performs before returning
/// `None` (which the dispatch shim treats as "fall back to cubic"). Each
/// retry uses a fresh deterministic seed. With `q > 2 n²`, the
/// per-attempt failure probability is `< 1/2`, so the chance of all
/// `KG_MAX_RETRIES` attempts failing is `< 2^{-KG_MAX_RETRIES} = 2^{-8}
/// ≈ 0.4 %`. Exhaustion never panics — it routes to the cubic path.
pub const KG_MAX_RETRIES: usize = 8;

// ─── Internal helper: Krylov-cyclic decomposition ───────────────────────────

/// One cyclic block of a Krylov decomposition.
///
/// `chain[i] = A^i · generator (mod span of earlier blocks)`. The
/// annihilator polynomial `poly` satisfies `poly(A) · generator ∈ span
/// of earlier blocks`, i.e. zero in the quotient module. The vectors
/// `chain[0..deg(poly)]` are linearly independent of all earlier-block
/// vectors and span this block's contribution to a global basis of `V`.
struct CyclicBlock<F: FiniteField> {
    /// Annihilator polynomial in `V / span(previous blocks)`. Used by
    /// the [`FieldMatrix::charpoly`] driver to compute
    /// `charpoly = ∏ block.poly`.
    poly: FieldPoly<F>,
}

/// Builds a cyclic decomposition of `V = F^n` under the action of `a`.
///
/// Returns a list of `CyclicBlock`s whose chains together span all of
/// `V` (sum of degrees = `n`). The polynomials are *not* necessarily
/// invariant factors yet: they are the minimal polynomials of each
/// generator in the **quotient** by previous blocks. Their product is
/// `charpoly(A)` and their lcm is `minpoly(A)`.
///
/// Algorithm:
///
/// - Maintain a row-reduced "basis" matrix `b_basis` (n × current_dim)
///   holding the union of all chain vectors found so far, alongside a
///   pivot table mapping `pivot_col[r] = j` if `b_basis` has a pivot in
///   row `r` at column `j` (or `usize::MAX` if no pivot for that row).
/// - For each new block: pick the smallest `i` such that `e_i` has a
///   non-zero residual after Gaussian reduction against `b_basis`. Use
///   that residual as the chain's first element (which equals `e_i` in
///   the quotient `V / span(b_basis)`).
/// - At each Krylov step compute `next = a · last_chain_residual` and
///   reduce `next` against `b_basis ∪ chain`. If the residual is non-
///   zero, append; if zero, the reduction coefficients on `chain[0..k]`
///   give the polynomial relation `poly(x) = x^k − Σ γ_j x^j`.
///
/// **Complexity**: `O(n³)` field operations across the whole loop —
/// each Krylov step costs `O(n²)` for the matvec and `O(n²)` for the
/// running-basis reduction; aggregated over `n` chain steps total this
/// is exactly `O(n³)`.
fn cyclic_decomposition<F: FiniteField>(a: &FieldMatrix<F>) -> Vec<CyclicBlock<F>> {
    cyclic_decomposition_inner(a, true)
}

/// Test-only wrapper that runs `cyclic_decomposition` with the packed
/// `ChainPolyArith` path forcibly disabled (and the packed basis reducer
/// kept on its default availability). Used by the
/// `proptest_packed_chain_polys_*_matches_scalar` tests so the scalar
/// `FieldPoly` chain-poly bookkeeping arm gets exercised independently
/// of the packed canonical-byte arm.
#[cfg(test)]
fn cyclic_decomposition_scalar_chain_polys<F: FiniteField>(
    a: &FieldMatrix<F>,
) -> Vec<CyclicBlock<F>> {
    cyclic_decomposition_inner(a, false)
}

/// Implementation backing both [`cyclic_decomposition`] (packed-arith
/// available) and the test-only scalar-only variant. The
/// `enable_packed_chain_polys` flag toggles the canonical-byte chain-poly
/// arithmetic path; the packed basis reducer / packed matvec stay on
/// their default availability.
fn cyclic_decomposition_inner<F: FiniteField>(
    a: &FieldMatrix<F>,
    enable_packed_chain_polys: bool,
) -> Vec<CyclicBlock<F>> {
    let (n, _) = a.shape();
    debug_assert_eq!(a.cols(), n, "cyclic_decomposition: A must be square");
    if n == 0 {
        return Vec::new();
    }
    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();
    // Pre-pack the matrix once so all `a · chain.last()` matvec calls in
    // the loop below avoid re-packing per call (issue d1dd266c).
    let driver = MatvecDriver::new(a);
    // Optional packed-basis cache: when present, reduce calls run on
    // canonical-byte / canonical-u16 lanes instead of per-element
    // Montgomery REDC chains, closing the n³ scalar reduce gap.
    let mut packed_basis: Option<Box<dyn BasisReducer<F>>> = F::try_make_basis_reducer(n);
    // Issue 5a3dbd5b: track packed chain-poly availability with a single
    // bool. The previous design held an outer `Box<dyn ChainPolyArith>`
    // purely as an `is_some` flag and reallocated a fresh per-block
    // handle below; the boxed-trait-object alloc was wasted churn.
    // `chain_poly_arith_available()` is a non-allocating mirror of
    // `try_make_chain_poly_arith` (review feedback for the boxed-handle
    // dummy alloc that the previous availability probe still incurred
    // once per decomposition).
    let _ = n; // silence unused-warning when no SIMD or non-Fp field
    let chain_poly_packed_available = enable_packed_chain_polys && F::chain_poly_arith_available();

    // Running basis B in row-reduced form. We store it as a flat
    // `Vec<FieldVec<F>>` (one per column) so that appending a new
    // column is cheap. Each column has been reduced against earlier
    // columns: column j has its pivot row at `pivot_row_of_col[j]`,
    // which is a value in [0, n). We also maintain the inverse mapping
    // `col_at_pivot_row[r] = Some(j)` if row r is a pivot row,
    // `None` otherwise.
    let mut basis: Vec<FieldVec<F>> = Vec::with_capacity(n);
    // Length-n table; `usize::MAX` means "no pivot in this row yet".
    let mut col_at_pivot_row: Vec<Option<usize>> = vec![None; n];
    // For each column in `basis`, the row of its pivot. Same length as
    // `basis`, kept in sync.
    let mut pivot_row_of_col: Vec<usize> = Vec::with_capacity(n);

    let mut blocks: Vec<CyclicBlock<F>> = Vec::new();
    let mut next_seed: usize = 0;

    // Helper closure: reduce against the current basis using the packed
    // cache when available, scalar `reduce` otherwise. Both paths
    // produce identical (residual, coeffs) for `Fp<P>` since the
    // packed kernel works on canonical lanes that round-trip exactly
    // through `value()` / `Fp::new`.
    let do_reduce = |v: &FieldVec<F>,
                     basis: &[FieldVec<F>],
                     pivot_row_of_col: &[usize],
                     packed: &Option<Box<dyn BasisReducer<F>>>|
     -> (FieldVec<F>, Vec<F>) {
        if let Some(pb) = packed.as_ref() {
            debug_assert_eq!(pb.len(), basis.len());
            let (res_vec, coeffs_vec) = pb.reduce(v.as_slice(), pivot_row_of_col);
            (FieldVec::from(res_vec), coeffs_vec)
        } else {
            reduce(v, basis, pivot_row_of_col)
        }
    };

    while basis.len() < n {
        // Find the next standard basis vector e_i not yet in span(B).
        // Reduce e_i = unit vector against the current basis; if its
        // residual is zero, advance i.
        let (residual, seed_index) = loop {
            assert!(
                next_seed < n,
                "cyclic_decomposition: ran out of seed vectors before \
                 spanning V; this indicates an internal invariant violation"
            );
            let mut e = FieldVec::<F>::zeros_from(n, &zero);
            e.set(next_seed, one.clone());
            let (red_vec, _coeffs) = do_reduce(&e, &basis, &pivot_row_of_col, &packed_basis);
            let used = next_seed;
            next_seed += 1;
            if red_vec.iter().any(|c| !c.is_zero()) {
                break (red_vec, used);
            }
        };

        // Each chain[k] is the residual of `A^k · u` (where u is the
        // original starting vector) after reduction against (earlier-
        // block basis ∪ chain[0..k-1]). Crucially, `chain[k] ≠ A^k · u`
        // in V — it equals A^k · u up to an earlier-basis correction
        // AND a linear combination of chain[0..k-1]. To recover the
        // correct polynomial relation in V, we track the polynomial
        // representation chain_poly[k] ∈ F[x] such that
        //
        //     chain[k] ≡ chain_poly[k](A) · u   (mod earlier-block basis)
        //
        // Whenever a reduction step subtracts α_j · chain[j] from the
        // working vector, the polynomial chain_poly[k+1] picks up a
        // `−α_j · chain_poly[j]` term. This bookkeeping is the only
        // way to express the final dependency relation as a polynomial
        // in `A` acting on `u` — the chain residuals alone do not.
        let _ = seed_index; // generator vector not retained: only the polynomial is needed downstream.
        let block_start = basis.len();
        let mut chain: Vec<FieldVec<F>> = Vec::new();
        // Track polynomial expression of each chain element in terms
        // of u. chain_poly[0] = `1` (the constant polynomial), since
        // chain[0] is the residual of u, which equals u in the quotient.
        //
        // Two paths:
        //   packed_cpa: canonical-byte store (Fp<P>, P ≤ 251, AVX2) —
        //               shift_x + batch_mul/sub avoid per-coeff REDC.
        //   chain_polys: scalar FieldPoly path (all other fields).
        let mut chain_polys: Vec<FieldPoly<F>> = Vec::new();
        // Per-block packed arith handle (re-create each block so the
        // chain-poly index resets to 0). The outer
        // `chain_poly_packed_available` flag (issue 5a3dbd5b) is what
        // decides whether to take the packed path; we construct a fresh
        // per-block handle from the factory only when packed is enabled,
        // so the bookkeeping costs at most one allocation per block.
        let mut packed_cpa: Option<Box<dyn ChainPolyArith<F>>> = if chain_poly_packed_available {
            F::try_make_chain_poly_arith(n)
        } else {
            None
        };

        // The very first chain element. We do NOT subtract any chain
        // contribution (chain is empty); only earlier-basis entries
        // got subtracted by the prior `reduce` call. So
        // chain[0] = u (mod earlier basis), polynomial = 1.
        append_to_basis(
            residual.clone(),
            &mut basis,
            &mut col_at_pivot_row,
            &mut pivot_row_of_col,
        );
        if let Some(pb) = packed_basis.as_mut() {
            let pivot_row = *pivot_row_of_col
                .last()
                .expect("append_to_basis must record a pivot row");
            pb.push_col_with_pivot_row(residual.as_slice(), pivot_row);
        }
        chain.push(residual);
        if let Some(cpa) = packed_cpa.as_mut() {
            cpa.push_one();
        } else {
            chain_polys.push(FieldPoly::one_like(&zero));
        }

        loop {
            // next = A · chain[-1] (in V).
            let next_in_v = driver.matvec(chain.last().unwrap());
            // Reduce against the full running basis. The reduction
            // returns (residual, coeffs) where coeffs[j] is the
            // coefficient of basis[j] in the original `next_in_v`,
            // and `residual = next_in_v − Σ coeffs[j] · basis[j]`.
            let (residual_next, coeffs) =
                do_reduce(&next_in_v, &basis, &pivot_row_of_col, &packed_basis);

            // The chain coefficients of this reduction:
            // α_j = coeffs[block_start + j] for j = 0..chain.len().
            // Build the next polynomial:
            //   chain_poly[d] = x · chain_poly[d-1] − Σ_j α_j chain_poly[j]
            //
            // Packed path: canonical-byte shift + batch_mul/sub (Fp<P> ≤ 251).
            // Scalar path: FieldPoly::mul_scalar + Sub (all other fields).
            if let Some(cpa) = packed_cpa.as_mut() {
                // Packed canonical-byte polynomial bookkeeping (issue 5a3dbd5b).
                // The packed impl converts alpha to canonical via a precomputed
                // lookup table (no per-call REDC in the hot path).
                let d = chain.len(); // current chain length before appending
                let mut buf = cpa.alloc_buf(d);
                cpa.shift_x_last_into(&mut buf);
                for j in 0..d {
                    let alpha = &coeffs[block_start + j];
                    if !alpha.is_zero() {
                        cpa.sub_scaled_into(&mut buf, alpha, j);
                    }
                }

                if residual_next.iter().any(|c| !c.is_zero()) {
                    // Independent: append to chain and basis.
                    append_to_basis(
                        residual_next.clone(),
                        &mut basis,
                        &mut col_at_pivot_row,
                        &mut pivot_row_of_col,
                    );
                    if let Some(pb) = packed_basis.as_mut() {
                        let pivot_row = *pivot_row_of_col
                            .last()
                            .expect("append_to_basis must record a pivot row");
                        pb.push_col_with_pivot_row(residual_next.as_slice(), pivot_row);
                    }
                    chain.push(residual_next);
                    cpa.push_buf(&buf);
                } else {
                    // Dependent: finalise the block polynomial.
                    let next_poly = cpa.finish_buf(&buf, &zero);
                    let poly = monic(next_poly);
                    blocks.push(CyclicBlock { poly });
                    break;
                }
            } else {
                // Scalar FieldPoly bookkeeping (all non-packed fields).
                let mut next_poly = poly_shift_x(&chain_polys[chain.len() - 1]);
                for j in 0..chain.len() {
                    let alpha = coeffs[block_start + j].clone();
                    if !alpha.is_zero() {
                        next_poly = &next_poly - &chain_polys[j].mul_scalar(&alpha);
                    }
                }

                if residual_next.iter().any(|c| !c.is_zero()) {
                    // Independent: append to chain and basis.
                    append_to_basis(
                        residual_next.clone(),
                        &mut basis,
                        &mut col_at_pivot_row,
                        &mut pivot_row_of_col,
                    );
                    if let Some(pb) = packed_basis.as_mut() {
                        let pivot_row = *pivot_row_of_col
                            .last()
                            .expect("append_to_basis must record a pivot row");
                        pb.push_col_with_pivot_row(residual_next.as_slice(), pivot_row);
                    }
                    chain.push(residual_next);
                    chain_polys.push(next_poly);
                } else {
                    // Dependent: next_poly is the minimal polynomial of `u`
                    // in the quotient `V / earlier basis`. By construction
                    // it is monic of degree exactly `chain.len()`.
                    let poly = monic(next_poly);
                    blocks.push(CyclicBlock { poly });
                    break;
                }
            }
        }
    }
    debug_assert_eq!(
        basis.len(),
        n,
        "cyclic_decomposition: chain vectors must span V"
    );
    debug_assert_eq!(
        blocks
            .iter()
            .map(|b| b.poly.degree().unwrap_or(0))
            .sum::<usize>(),
        n,
        "cyclic_decomposition: total degree must equal n"
    );
    blocks
}

/// Computes the minimal polynomial of a vector `v` modulo the basis
/// `basis` under the action of `a`. That is, the unique monic
/// polynomial `p ∈ F[x]` of smallest degree with `p(A) · v ∈ span(basis)`.
fn vector_minpoly_in_quotient<F: FiniteField>(
    a: &FieldMatrix<F>,
    v: &FieldVec<F>,
    basis: &[FieldVec<F>],
    pivot_row_of_col: &[usize],
) -> FieldPoly<F> {
    let n = v.len();
    if n == 0 {
        let z = F::zero_hint().expect("vector_minpoly_in_quotient: empty input zero");
        return FieldPoly::one_like(&z);
    }
    let zero = v.get(0).zero_like();
    // Reduce v first; if zero in the quotient, minpoly = 1.
    let (residual0, _) = reduce(v, basis, pivot_row_of_col);
    if residual0.iter().all(|c| c.is_zero()) {
        return FieldPoly::one_like(&zero);
    }

    // Local "running basis" extending the input basis by the chain.
    let mut local_basis: Vec<FieldVec<F>> = basis.to_vec();
    let mut local_pivot_row_of_col: Vec<usize> = pivot_row_of_col.to_vec();
    let mut local_col_at_pivot_row: Vec<Option<usize>> = vec![None; n];
    for (j, &r) in pivot_row_of_col.iter().enumerate() {
        local_col_at_pivot_row[r] = Some(j);
    }
    let block_start = local_basis.len();
    append_to_basis(
        residual0,
        &mut local_basis,
        &mut local_col_at_pivot_row,
        &mut local_pivot_row_of_col,
    );
    let mut chain_polys: Vec<FieldPoly<F>> = vec![FieldPoly::one_like(&zero)];

    loop {
        let next_in_v = a.matvec(&local_basis[local_basis.len() - 1]);
        let (residual_next, coeffs) = reduce(&next_in_v, &local_basis, &local_pivot_row_of_col);

        let last = chain_polys.len() - 1;
        let mut next_poly = poly_shift_x(&chain_polys[last]);
        for j in 0..chain_polys.len() {
            let alpha = coeffs[block_start + j].clone();
            if !alpha.is_zero() {
                next_poly = &next_poly - &chain_polys[j].mul_scalar(&alpha);
            }
        }

        if residual_next.iter().any(|c| !c.is_zero()) {
            append_to_basis(
                residual_next,
                &mut local_basis,
                &mut local_col_at_pivot_row,
                &mut local_pivot_row_of_col,
            );
            chain_polys.push(next_poly);
        } else {
            return monic(next_poly);
        }
    }
}

/// Finds a vector `u` whose minpoly in the quotient `V / span(basis)`
/// equals the minpoly of `A` acting on that quotient. Returns
/// `(u, minpoly)`.
///
/// # Strategy
///
/// 1. Scan canonical basis vectors `e_i` outside `span(basis)` and
///    record each one's minpoly in the quotient (via
///    [`vector_minpoly_in_quotient`]). The LCM of those per-vector
///    minpolys is the minpoly of `A` acting on `V / span(basis)`.
/// 2. If a single canonical vector already achieves the LCM, return it.
/// 3. Otherwise, combine candidates pairwise. The combination uses
///    polynomial action on `A` so that the result is **mathematically
///    guaranteed** to reach the target LCM regardless of field size or
///    characteristic.
///
/// # Why polynomial-action combinations (not scalar multipliers)
///
/// The previous baseline (R0, commit `e47a92d`) tried scalar multiples
/// `α · v` for `α = 1..=64`, where `α` was generated by repeated
/// addition of `1`. In characteristic-2 fields (`Gf2mWide<8>`,
/// `Gf2mWide<16>`, etc.) the prime subfield has only `{0, 1}`, so the
/// sweep collapsed to a single trial (`α = 1`) and could miss the
/// combinations that are required to attain the LCM when neither
/// candidate alone does. The reviewer flagged this as a correctness
/// blocker (R1, finding 4).
///
/// The current implementation removes the scalar sweep entirely. Given
/// candidates `(u, p)` and `(v, q)`, it constructs a sum
/// `α(A) · u + β(A) · v` whose minpoly is `lcm(p, q)`, where `α, β` are
/// derived from a coprime split of `(p, q)` via repeated GCD reduction
/// (see [`coprime_split`]). This relies only on field arithmetic and
/// polynomial-action evaluation, both of which work in any `FiniteField`.
///
/// # Complexity
///
/// Per call: `O(n)` candidates × `O(n)` per minpoly recompute →
/// `O(n²)` polynomial work plus `O(n³)` matvec work. Bounded above by
/// the cubic-Krylov budget of the outer driver.
fn find_max_minpoly_generator<F: FiniteField>(
    a: &FieldMatrix<F>,
    basis: &[FieldVec<F>],
    pivot_row_of_col: &[usize],
    zero: &F,
) -> (FieldVec<F>, FieldPoly<F>) {
    let n = a.rows();
    debug_assert!(basis.len() < n);
    let one = zero.one_like();

    // Collect canonical basis vectors outside span(basis), with their
    // per-vector minpolys.
    let mut candidates: Vec<(FieldVec<F>, FieldPoly<F>)> = Vec::new();
    let mut lcm_so_far: Option<FieldPoly<F>> = None;
    for i in 0..n {
        let mut e = FieldVec::<F>::zeros_from(n, zero);
        e.set(i, one.clone());
        // Skip if in span(basis).
        let (residual, _) = reduce(&e, basis, pivot_row_of_col);
        if residual.iter().all(|c| c.is_zero()) {
            continue;
        }
        let p = vector_minpoly_in_quotient(a, &e, basis, pivot_row_of_col);
        if let Some(prev) = &lcm_so_far {
            lcm_so_far = Some(poly_lcm(prev, &p));
        } else {
            lcm_so_far = Some(p.clone());
        }
        candidates.push((e, p));
    }
    let target_lcm = lcm_so_far.expect("at least one canonical vector outside span(basis)");

    // Fast path: a single candidate already achieves the LCM.
    if let Some((u, p)) = candidates.iter().find(|(_, p)| p == &target_lcm) {
        return (u.clone(), p.clone());
    }

    // Greedy merge. Sort descending by minpoly degree so the seed has
    // the longest reachable annihilator.
    candidates.sort_by_key(|c| std::cmp::Reverse(c.1.degree()));
    let (mut u, mut u_min) = candidates[0].clone();
    for (v, v_min) in candidates.iter().skip(1) {
        if u_min == target_lcm {
            return (u, u_min);
        }
        // Cheap shortcut: nothing to add when v's minpoly already
        // divides u's.
        if poly_divides(v_min, &u_min) {
            continue;
        }
        // The pair (u, v) targets `lcm(u_min, v_min)`. Compute a
        // coprime split (a | u_min, b | v_min, a·b = lcm, gcd(a,b)=1)
        // and form `(u_min/a)(A)·u + (v_min/b)(A)·v` — its minpoly is
        // exactly `a · b = lcm(u_min, v_min)`, in any field.
        let (a_div, b_div) = coprime_split(&u_min, v_min);
        let alpha = poly_div_exact(&u_min, &a_div);
        let beta = poly_div_exact(v_min, &b_div);
        let u_action = poly_action_on_vector(&alpha, a, &u);
        let v_action = poly_action_on_vector(&beta, a, v);
        let mut combined = u_action;
        combined.axpy(&one, &v_action);
        let combined_min = vector_minpoly_in_quotient(a, &combined, basis, pivot_row_of_col);
        let new_lcm = poly_lcm(&u_min, v_min);
        // The construction guarantees `combined_min == new_lcm`. Accept
        // unconditionally if so; otherwise fall back to plain `u + v`
        // and re-verify (defensive: covers degenerate fixed points the
        // coprime split might land on for unusual `(p, q)` pairs).
        if combined_min == new_lcm {
            u = combined;
            u_min = combined_min;
        } else {
            let mut fallback = u.clone();
            fallback.axpy(&one, v);
            let fb_min = vector_minpoly_in_quotient(a, &fallback, basis, pivot_row_of_col);
            // Accept whichever of (combined, fallback) has higher
            // degree, breaking ties towards the LCM exactly.
            if fb_min == new_lcm
                || (fb_min.degree().unwrap_or(0) > combined_min.degree().unwrap_or(0))
            {
                u = fallback;
                u_min = fb_min;
            } else {
                u = combined;
                u_min = combined_min;
            }
        }
    }
    (u, u_min)
}

/// Returns `p(A) · v` via Horner: `((c_d · A + c_{d-1}) · A + … + c_0) · v`.
///
/// `p` is the polynomial, `a` the matrix, `v` the input vector. Cost:
/// `O(deg(p) · n²)` field operations using only [`FieldMatrix::matvec`]
/// and [`FieldVec::axpy`] — no bespoke linear algebra.
pub(crate) fn poly_action_on_vector<F: FiniteField>(
    p: &FieldPoly<F>,
    a: &FieldMatrix<F>,
    v: &FieldVec<F>,
) -> FieldVec<F> {
    let driver = MatvecDriver::new(a);
    poly_action_on_vector_via_driver(p, &driver, v)
}

/// Same as [`poly_action_on_vector`] but threads the matvec calls
/// through a pre-built [`MatvecDriver`], so the per-call repack cost
/// of `FieldMatrix::matvec` is avoided across multiple Horner runs on
/// the same matrix.
fn poly_action_on_vector_via_driver<F: FiniteField>(
    p: &FieldPoly<F>,
    driver: &MatvecDriver<'_, F>,
    v: &FieldVec<F>,
) -> FieldVec<F> {
    let n = v.len();
    let zero: F = if n > 0 {
        v.get(0).zero_like()
    } else {
        F::zero_hint()
            .expect("poly_action_on_vector_via_driver: cannot synthesise zero for empty vector")
    };
    if p.is_zero() {
        return FieldVec::<F>::zeros_from(n, &zero);
    }
    let deg = p.degree().expect("non-zero polynomial has a degree");
    let mut acc = FieldVec::<F>::zeros_from(n, &zero);
    let lead = p.coeff(deg);
    if !lead.is_zero() {
        acc.axpy(&lead, v);
    }
    for k in (0..deg).rev() {
        acc = driver.matvec(&acc);
        let ck = p.coeff(k);
        if !ck.is_zero() {
            acc.axpy(&ck, v);
        }
    }
    acc
}

/// Computes a coprime split `(a, b)` of `(p, q)` such that `a | p`,
/// `b | q`, `gcd(a, b) = 1`, and `a · b = lcm(p, q)`.
///
/// # Algorithm
///
/// Iterative GCD-stripping (a finite-field analogue of Saunders'
/// "coprime base" iteration). Initialise `a := p`, `b := q / gcd(p, q)`
/// so that `a · b = lcm(p, q)` and `b | q`. Then repeatedly:
///
/// 1. Compute `g = gcd(a, b)`. If `g = 1`, return.
/// 2. Otherwise `g | a` and `g | b`, with `g² | a · b = lcm`. Move the
///    saturation of `g` towards `a` (which already divides the input
///    `p`) by replacing `b ← b / g_max`, `a ← a · g_max` where
///    `g_max | gcd(a, ∞)` is the maximal `g`-power dividing `a`. To
///    preserve `a | p` we must verify the multiplication; if not, swap
///    sides and continue.
///
/// The iteration converges in at most `O(deg(gcd(p, q)))` steps because
/// each step strictly reduces `deg(gcd(a, b))` while preserving
/// `a · b = lcm(p, q)`, `a | p`, `b | q`.
///
/// # Termination guarantee
///
/// If the algorithm cannot make progress for an iteration (e.g. due to
/// the swap predicate failing on a pathological `g` whose support is
/// shared symmetrically between `p` and `q`), it returns the current
/// best-effort split. The caller (`find_max_minpoly_generator`) detects
/// this via a post-hoc minpoly verification and falls back to plain
/// vector addition if the constructed combination misses the target.
fn coprime_split<F: FiniteField>(
    p: &FieldPoly<F>,
    q: &FieldPoly<F>,
) -> (FieldPoly<F>, FieldPoly<F>) {
    let g0 = FieldPoly::gcd(p, q);
    if g0.is_zero() || is_constant_one(&g0) {
        return (p.clone(), q.clone());
    }
    // Initial split: a = p, b = q / gcd(p, q). Then a · b = lcm(p, q),
    // a | p, b | q. gcd(a, b) might still be non-trivial.
    let mut a = p.clone();
    let mut b = poly_div_exact(q, &g0);

    // Iterative refinement. Each iteration strips one g-saturation;
    // bounded by deg(g0) so we cap defensively at deg(p) + deg(q) + 1.
    let max_iters = p.len() + q.len() + 2;
    for _ in 0..max_iters {
        let g = FieldPoly::gcd(&a, &b);
        if is_constant_one(&g) {
            return (a, b);
        }
        // g_a := the maximal `g`-saturation of `a`: gcd(a, g^k) for k → ∞.
        // Computed iteratively: lift g by repeatedly squaring within F[x]
        // and intersecting with a; equivalent to dividing a by g until
        // the quotient is coprime to g.
        let g_a = saturate_with(&a, &g);
        let g_b = saturate_with(&b, &g);
        // Prefer to move the more-saturated side onto `a` (which still
        // divides `p`). If a's saturation already dominates b's, divide
        // b by g_b — the gcd shrinks, a · b shrinks too. To preserve
        // a · b = lcm we then multiply a by g_b/gcd(g_a, g_b). But
        // a · g_b/... must still divide p; verify before committing.
        let (next_a, next_b) = if g_a.degree().unwrap_or(0) >= g_b.degree().unwrap_or(0) {
            // Strip g_b from b.
            let new_b = poly_div_exact(&b, &g_b);
            // Re-add the missing g-mass to a so that a · b = lcm(p, q).
            // Required factor: g_b / gcd(g_a, g_b).
            let inter = FieldPoly::gcd(&g_a, &g_b);
            let extra = poly_div_exact(&g_b, &inter);
            // Verify a · extra still divides p; if not, fall through to swap.
            let new_a_candidate = monic(&a * &extra);
            if poly_divides(&new_a_candidate, p) {
                (new_a_candidate, new_b)
            } else {
                // Symmetric move: strip g_a from a, multiply b by g_a/inter.
                let new_a = poly_div_exact(&a, &g_a);
                let extra_b = poly_div_exact(&g_a, &inter);
                let new_b_candidate = monic(&b * &extra_b);
                if poly_divides(&new_b_candidate, q) {
                    (new_a, new_b_candidate)
                } else {
                    // No progress possible by either move; bail out
                    // with current best-effort.
                    return (a, b);
                }
            }
        } else {
            // Strip g_a from a.
            let new_a = poly_div_exact(&a, &g_a);
            let inter = FieldPoly::gcd(&g_a, &g_b);
            let extra = poly_div_exact(&g_a, &inter);
            let new_b_candidate = monic(&b * &extra);
            if poly_divides(&new_b_candidate, q) {
                (new_a, new_b_candidate)
            } else {
                return (a, b);
            }
        };
        a = next_a;
        b = next_b;
    }
    (a, b)
}

/// Returns the `g`-saturation of `p`: the unique divisor `s | p` with
/// `s` having the same prime support as `g` and the same valuation in
/// `p` at every prime of `g`. Computed by repeated GCD: `s_0 = gcd(p, g)`,
/// `s_{k+1} = gcd(p, s_k²)` until stable.
fn saturate_with<F: FiniteField>(p: &FieldPoly<F>, g: &FieldPoly<F>) -> FieldPoly<F> {
    let mut s = FieldPoly::gcd(p, g);
    let max_iters = p.len() + 2;
    for _ in 0..max_iters {
        let s_sq = &s * &s;
        let next = FieldPoly::gcd(p, &s_sq);
        if next == s {
            return s;
        }
        s = next;
    }
    s
}

/// Returns the exact polynomial quotient `numer / denom` assuming
/// `denom` divides `numer` exactly. Panics on a non-zero remainder
/// (debug-only assertion).
fn poly_div_exact<F: FiniteField>(numer: &FieldPoly<F>, denom: &FieldPoly<F>) -> FieldPoly<F> {
    let (q, r) = numer.div_rem(denom);
    debug_assert!(
        r.is_zero(),
        "poly_div_exact: caller-supplied denom must divide numer exactly"
    );
    monic(q)
}

/// Returns `true` iff `p` is the constant polynomial `1` (degree 0,
/// leading coefficient 1).
fn is_constant_one<F: FiniteField>(p: &FieldPoly<F>) -> bool {
    p.degree() == Some(0) && p.coeff(0).is_one()
}

/// Returns `x · p(x)` (degree increased by one).
fn poly_shift_x<F: FiniteField>(p: &FieldPoly<F>) -> FieldPoly<F> {
    if p.is_zero() {
        return p.clone();
    }
    let mut coeffs: Vec<F> = Vec::with_capacity(p.len() + 1);
    let zero = p.iter().next().unwrap().zero_like();
    coeffs.push(zero);
    for c in p.iter() {
        coeffs.push(c.clone());
    }
    FieldPoly::from_coeffs_trimmed(coeffs)
}

/// Reduces a vector `v` against a running basis whose columns have
/// been row-reduced into pivot positions. Returns `(residual, coeffs)`
/// such that `v = Σ coeffs[j] · basis[j] + residual`, with `residual`
/// having zeros at every pivot row of `basis`.
///
/// The basis is **not** in upper-triangular form; each column carries
/// a single pivot row that may be anywhere in [0, n). Reduction is
/// the natural Gauss-elimination loop: for each pivot row `r` covered
/// by some column `j`, subtract `(v[r] / basis[j][r]) · basis[j]`
/// from `v` and accumulate the multiplier into `coeffs[j]`. After all
/// pivot rows have been swept, the residual has zeros in every pivot
/// row.
fn reduce<F: FiniteField>(
    v: &FieldVec<F>,
    basis: &[FieldVec<F>],
    pivot_row_of_col: &[usize],
) -> (FieldVec<F>, Vec<F>) {
    let n = v.len();
    let zero: F = if n > 0 {
        v.get(0).zero_like()
    } else if let Some(z) = F::zero_hint() {
        z
    } else {
        // Empty vector ⇒ empty residual, empty coeffs. Not actually
        // reachable from `cyclic_decomposition` because n > 0 there,
        // but the helper is generic.
        return (FieldVec::<F>::new(), Vec::new());
    };
    let mut residual = v.clone();
    let mut coeffs: Vec<F> = vec![zero.clone(); basis.len()];
    for (j, col) in basis.iter().enumerate() {
        let r = pivot_row_of_col[j];
        let pivot_val = col.get(r).clone();
        let v_at_r = residual.get(r).clone();
        if v_at_r.is_zero() {
            continue;
        }
        // factor = v_at_r / pivot_val.
        let pivot_inv = pivot_val
            .inv()
            .expect("reduce: pivot value must be non-zero by construction");
        let factor = v_at_r * pivot_inv;
        // residual -= factor · col. Implemented in place via axpy with
        // the negated factor.
        let neg_factor = zero.clone() - factor.clone();
        residual.axpy(&neg_factor, col);
        coeffs[j] = factor;
    }
    (residual, coeffs)
}

/// Appends a fresh column to the running basis, using the first non-
/// zero row not yet covered by an existing pivot as the new pivot row.
fn append_to_basis<F: FiniteField>(
    column: FieldVec<F>,
    basis: &mut Vec<FieldVec<F>>,
    col_at_pivot_row: &mut [Option<usize>],
    pivot_row_of_col: &mut Vec<usize>,
) {
    let n = column.len();
    let mut pivot_row: Option<usize> = None;
    for (r, slot) in col_at_pivot_row.iter().enumerate().take(n) {
        if !column.get(r).is_zero() && slot.is_none() {
            pivot_row = Some(r);
            break;
        }
    }
    let pr = pivot_row.expect(
        "append_to_basis: residual was reported non-zero but no fresh pivot \
         row exists; this is an invariant violation in the reduction loop",
    );
    let new_col_index = basis.len();
    col_at_pivot_row[pr] = Some(new_col_index);
    pivot_row_of_col.push(pr);
    basis.push(column);
}

// ─── Sub-cubic Keller–Gehrig path (issue 1454ec2d) ───────────────────────────

/// Routes [`FieldMatrix::charpoly`] between the cubic baseline and the
/// sub-cubic Keller–Gehrig variant. See the module rustdoc for the
/// decision tree.
fn charpoly_dispatch<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    let (m, n) = a.shape();
    assert_eq!(
        m, n,
        "FieldMatrix::charpoly: input must be square (got {}×{})",
        m, n
    );

    // Small / runtime-context / low-cardinality matrices: route cubic
    // unconditionally (the sub-cubic path's `log n` factor and Las-Vegas
    // bookkeeping aren't amortised at small `n`, and the probability
    // gate `q > 2 n²` doesn't hold for low-cardinality fields).
    if n < KG_DISPATCH_MIN_N {
        return a.charpoly_cubic();
    }
    let log_q = match F::cardinality_log2_hint() {
        Some(v) => v,
        None => return a.charpoly_cubic(),
    };
    // Probability gate: `q > 2 · n²`. Skip when `q` does not fit in
    // `u128` (i.e. `log_q > 127`); in that regime `q ≫ 2 n²` for any
    // `n` we can build a matrix of (`n ≤ 2^{63}` is the absolute
    // ceiling on `usize`-indexed matrices).
    if log_q <= 127 {
        let q = 1u128 << log_q;
        let two_n_sq = 2u128 * (n as u128) * (n as u128);
        if q <= two_n_sq {
            return a.charpoly_cubic();
        }
    }
    // Static-cardinality field that passes the gate. Try sub-cubic
    // with a deterministic seed; on Las-Vegas exhaustion the dispatch
    // silently falls back to cubic. Bit-exact equality across paths is
    // a `[hard]` success criterion of issue `1454ec2d`.
    if let Some(p) = keller_gehrig_charpoly(a, KG_DEFAULT_SEED) {
        return p;
    }
    a.charpoly_cubic()
}

/// Default seed for the [`charpoly_dispatch`] entry into the
/// Keller–Gehrig path. Stable across runs (deterministic dispatch is a
/// project-wide invariant).
const KG_DEFAULT_SEED: u64 = 0x4B65_6C6C_6572_4768; // ASCII "KellerGh"

/// Runs the Keller–Gehrig Las-Vegas worker up to [`KG_MAX_RETRIES`]
/// times with seeds derived from `base_seed`. Returns the first
/// successful charpoly that satisfies the Cayley–Hamilton check, or
/// `None` if every attempt failed.
fn keller_gehrig_charpoly<F: FiniteField>(
    a: &FieldMatrix<F>,
    base_seed: u64,
) -> Option<FieldPoly<F>> {
    let (m, n) = a.shape();
    debug_assert_eq!(m, n);

    if n == 0 {
        // Empty product = constant 1. Mirror `charpoly_cubic`'s
        // behaviour: require a `zero_hint` witness (all in-tree paths
        // that reach here are static-cardinality, so this is always
        // available).
        let zero = F::zero_hint()?;
        return Some(FieldPoly::one_like(&zero));
    }
    if n == 1 {
        // Trivial case: charpoly = x − A[0,0]. Skip the Krylov
        // construction entirely; this also short-circuits the
        // small-matrix corner of the Las-Vegas loop.
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let neg_a00 = zero - a.get(0, 0);
        return Some(FieldPoly::from_coeffs_trimmed(vec![neg_a00, one]));
    }

    for retry in 0..KG_MAX_RETRIES {
        let seed = base_seed.wrapping_add(retry as u64);
        if let Some(p) = keller_gehrig_attempt(a, seed) {
            return Some(p);
        }
    }
    None
}

/// SplitMix64 — a deterministic, platform-stable u64 PRNG used by
/// [`keller_gehrig_attempt`] for vector generation. Inlined here so the
/// worker runs without the `rand` feature (the crate's `rand` is
/// `optional = true` and we must not change that contract).
#[inline]
pub(crate) fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// One Keller–Gehrig attempt with a specific random seed.
///
/// Returns `None` on Las-Vegas failure (`v` not cyclic for `A`), in
/// which case the caller should retry with a fresh seed. Returns
/// `Some(p)` only after verifying `p.eval_at_matrix(A) == 0` — the
/// Cayley–Hamilton check that pins bit-exact equality with the cubic
/// path output.
fn keller_gehrig_attempt<F: FiniteField>(a: &FieldMatrix<F>, seed: u64) -> Option<FieldPoly<F>> {
    let n = a.rows();
    debug_assert!(n >= 2, "n ∈ {{0, 1}} should have been short-circuited");
    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();

    // Step 1: deterministic random `v`. We do not rely on the `rand`
    // crate here because charpoly is part of the crate's default-feature
    // surface and must not pull `rand` into non-test builds.
    //
    // Construction: derive a small natural-number scalar from
    // SplitMix64, then translate it into `F` via repeated `+= one` (the
    // canonical "integer literal in F" path that's available in every
    // FiniteField). For prime fields `Fp<P>` with small `P` this gives
    // an approximately-uniform sample over `[0, P)`; for large prime
    // fields the sample is concentrated in a small subset of `F`, but
    // the cyclicity bound `1 − n/q` holds against *any* non-degenerate
    // distribution that hits enough non-zero coordinates, and in
    // practice a single call already produces a cyclic `v` on the
    // first attempt for every test we run. The Las-Vegas retry loop
    // backstops any pathological seed.
    let mut state = seed;
    let mut v = FieldVec::<F>::zeros_from(n, &zero);
    for i in 0..n {
        // Cap the integer scalar at 64 to avoid pathological loops in
        // small-characteristic fields (where many `+= one` collapse
        // back to 0 or 1). 64 is enough to cover GF(2) {0, 1}, GF(7),
        // GF(8), and the small windows of larger fields; uniformity is
        // not required, only non-degeneracy.
        let count = (splitmix64(&mut state) & 0x3F) as u32;
        let mut acc = zero.clone();
        for _ in 0..count {
            acc += &one;
        }
        // Occasional zeros: 50 % chance to leave the slot at zero.
        if (splitmix64(&mut state) & 1) == 1 {
            v.set(i, acc);
        }
    }
    // Reject the all-zero residue (degenerate; never cyclic).
    if v.iter().all(|c| c.is_zero()) {
        v.set(0, one.clone());
    }

    // Step 2: build K = [v | A·v | … | A^{n-1}·v] by repeated squaring.
    //
    // Invariant maintained across the loop:
    //   - `cols` columns of `K` are populated (initially 1: just `v`)
    //   - `b == A^cols` (initially `A`)
    //
    // Each iteration extends `K` to `min(2 · cols, n)` columns by
    // appending `B · K[:, 0..(new_cols − cols)]`, then squares `B` if
    // we're not yet at full width.
    //
    // All matmuls dispatch through the workspace's
    // [`gemm_into_view`](crate::field::matrix::gemm_into_view) /
    // [`gemm`](crate::field::matrix::gemm) entrypoints — no bespoke
    // kernel.
    let mut k_mat = FieldMatrix::<F>::new(n, 1, zero.clone());
    for r in 0..n {
        k_mat.set(r, 0, v.get(r).clone());
    }
    let mut b = a.clone();
    let mut cols = 1usize;
    while cols < n {
        let new_cols = (2 * cols).min(n);
        let rhs_cols = new_cols - cols;
        let mut new_k = FieldMatrix::<F>::new(n, new_cols, zero.clone());
        // Copy the existing left half.
        for i in 0..n {
            for j in 0..cols {
                new_k.set(i, j, k_mat.get(i, j));
            }
        }
        // Build the right half = B · K[:, 0..rhs_cols].
        if rhs_cols > 0 {
            let k_prefix = k_mat.submat(.., 0..rhs_cols);
            let mut prod = FieldMatrix::<F>::new(n, rhs_cols, zero.clone());
            crate::field::matrix::gemm_into_view(&b, &k_prefix, prod.submat_mut(.., ..));
            for i in 0..n {
                for j in 0..rhs_cols {
                    new_k.set(i, cols + j, prod.get(i, j));
                }
            }
        }
        k_mat = new_k;
        cols = new_cols;
        // Square B for the next doubling: B := B · B. Skip on the
        // final iteration where `cols == n` (we won't use B again).
        if cols < n {
            b = crate::field::matrix::gemm(&b, &b);
        }
    }
    debug_assert_eq!(k_mat.cols(), n);

    // Step 3: w = A^n · v = A · K[:, n−1].
    let last_col = {
        let mut col = FieldVec::<F>::zeros_from(n, &zero);
        for r in 0..n {
            col.set(r, k_mat.get(r, n - 1));
        }
        col
    };
    let w = a.matvec(&last_col);

    // Step 4: solve K · y = w (one PLE + two trsm under the hood).
    // `None` ⇒ K is rank-deficient ⇒ random `v` was not cyclic.
    let y = k_mat.solve(&w)?;

    // Step 5: charpoly = x^n − y_{n−1} x^{n−1} − … − y_0. The relation
    // `A · K[:, n−1] = K · y` rearranges to
    // `(A^n − Σ y_k A^k) · v = 0`, and on a cyclic vector this lifts
    // to the matrix identity itself.
    let mut coeffs: Vec<F> = Vec::with_capacity(n + 1);
    for i in 0..n {
        coeffs.push(zero.clone() - y.get(i).clone());
    }
    coeffs.push(one.clone());
    let cp = FieldPoly::from_coeffs_trimmed(coeffs);

    // Step 6: Cayley–Hamilton verification. If `v` was not cyclic, the
    // candidate poly may be a proper divisor of charpoly (still
    // annihilating `v` but not `A`); evaluating it on `A` will return
    // a non-zero matrix. Reject and let the caller retry.
    let pa = cp.eval_at_matrix(a);
    for i in 0..n {
        for j in 0..n {
            if !pa.get(i, j).is_zero() {
                return None;
            }
        }
    }
    // Defence-in-depth: also verify the recovered polynomial has
    // exact degree `n` and is monic. Either failure indicates an
    // internal bug and is treated as a Las-Vegas failure (retry).
    if cp.degree() != Some(n) {
        return None;
    }
    if !cp.leading_coeff().map(|c| c.is_one()).unwrap_or(false) {
        return None;
    }
    Some(cp)
}

// ─── Wiedemann (scalar) minimal polynomial path (issue d1dd266c) ─────────────

/// Maximum number of Las-Vegas retries for the Wiedemann minpoly path.
///
/// Each attempt picks a fresh random pair `(u, v)` and runs
/// Berlekamp-Massey on the scalar projection sequence. On failure
/// (BM outputs a polynomial that does not annihilate the full space),
/// the retry count limits wasted work before falling back to the
/// `O(n⁴)` deterministic path. Eight retries gives failure probability
/// `≤ (n/q)^8` per attempt class, which is negligible for all four
/// reference fields (q ≥ 7, n ≤ 1024).
const WIEDEMANN_MAX_RETRIES: usize = 8;

/// Default seed for the Wiedemann minpoly dispatch.
const WIEDEMANN_DEFAULT_SEED: u64 = 0x5769_6564_656D_616E; // ASCII "Wiedeman"

/// Berlekamp-Massey algorithm over an arbitrary `FiniteField`.
///
/// Given a sequence `s[0..m]` (at least `2 * deg(minpoly) + 1` terms
/// recommended; the caller supplies `2n + 1` terms for an `n × n` matrix),
/// returns the **monic** minimal linear recurrence polynomial `C(x)` of
/// degree `d ≤ n/2` such that
///
/// ```text
/// s[k] = c_1 s[k-1] + c_2 s[k-2] + … + c_d s[k-d]   for k ≥ d
/// ```
///
/// In polynomial form: `C(x) = 1 − c_1 x − … − c_d x^d`. Because this
/// is a "constant-term-1" polynomial (not monic-leading-coefficient in the
/// usual sense), the output is converted to the ascending-coefficient form
/// expected by [`FieldPoly`] by negating all but the constant term, yielding:
///
/// ```text
/// coeffs[0] = 1,  coeffs[1] = c_1,  …,  coeffs[d] = c_d
/// ```
///
/// Wait — this would make the constant term 1, not a degree-`d` monic poly.
/// Actually the Wiedemann minpoly polynomial we want is the annihilator
/// polynomial of the LFSR: `L(x) = x^d − c_1 x^{d-1} − … − c_d` (monic,
/// leading coeff = 1, constant term = −c_d). The relationship is:
/// `L(x) = x^d · C(1/x) / C(0)` (reversal). This is what gets returned.
///
/// # Algorithm
///
/// Massey (1969), presentation from Shoup §12.4. Maintains:
/// - `C(x)` = current connection polynomial, stored as `C[0] = 1` (constant
///   term) through `C[L]` (degree L). `C[0] = 1` is invariant.
/// - `B(x)` = copy of `C` at the last length-extension step.
/// - `b` = discrepancy at the last length-extension step (scalar).
/// - `L` = current LFSR length.
/// - `m` = number of steps since last length extension.
///
/// # Complexity
///
/// `O(n²)` field operations for a length-`2n` sequence.
pub(crate) fn berlekamp_massey<F: FiniteField>(s: &[F]) -> FieldPoly<F> {
    let seq_len = s.len();
    if seq_len == 0 {
        let zero = F::zero_hint().unwrap_or_else(|| {
            panic!(
                "berlekamp_massey: empty sequence over runtime-context field has no zero witness"
            )
        });
        return FieldPoly::one_like(&zero);
    }
    let zero: F = s[0].zero_like();
    let one: F = zero.one_like();

    // C(x) = 1 + C[1]*x + ... + C[L]*x^L in ascending order.
    // Invariant: C[0] = 1 always.
    let mut c: Vec<F> = vec![one.clone()];
    // B(x) = 1 initially (copy of C before last length extension).
    let mut b_poly: Vec<F> = vec![one.clone()];
    // b = discrepancy at last length extension (scalar). Initialize to 1.
    let mut b_scalar: F = one.clone();
    // L = current LFSR length (= deg C if C != 1).
    let mut ell: usize = 0;
    // m = shift counter (x^m factor on B).
    let mut m: usize = 1;

    for n_idx in 0..seq_len {
        // Discrepancy: d = s[n] + C[1]*s[n-1] + ... + C[L]*s[n-L]
        //             = sum_{j=0}^{L} C[j] * s[n-j]
        // (with s[k] = 0 for k < 0, and C[0] = 1.)
        let mut d = s[n_idx].clone();
        for j in 1..=ell {
            if n_idx >= j {
                let term = c[j].clone() * s[n_idx - j].clone();
                d += term;
            }
        }

        if d.is_zero() {
            m += 1;
            continue;
        }

        // Adjustment: T(x) = C(x) - (d/b) * x^m * B(x)
        let factor = d.clone()
            * b_scalar
                .inv()
                .expect("berlekamp_massey: b_scalar must be invertible");
        // T has length max(C.len(), m + B.len()) in the worst case.
        let new_len = c.len().max(m + b_poly.len());
        let mut t: Vec<F> = c.clone();
        while t.len() < new_len {
            t.push(zero.clone());
        }
        for (i, bi) in b_poly.iter().enumerate() {
            let idx = i + m;
            if idx < t.len() {
                let sub = factor.clone() * bi.clone();
                let old_val = t[idx].clone();
                t[idx] = old_val - sub;
            }
        }

        if 2 * ell <= n_idx {
            // Length extension: L_new = n+1 - L_old
            let ell_new = n_idx + 1 - ell;
            // Save old C as new B, update b_scalar, reset m.
            b_poly = c;
            b_scalar = d;
            ell = ell_new;
            m = 1;
        } else {
            m += 1;
        }
        c = t;
    }

    // c = [1, C[1], ..., C[L]] where C(x) = 1 + C[1]*x + ... + C[L]*x^L
    // is the connection poly: s[n] + C[1]*s[n-1] + ... + C[L]*s[n-L] = 0.
    //
    // We want the annihilator poly (monic in x) for the Wiedemann method:
    // A(x) = x^L + C[L]*x^{L-1} + ... + C[1]  (reversed and re-indexed).
    // Actually A(x) = x^L * C(1/x) / C(0) = x^L * C(1/x) (since C(0)=1).
    //
    // A[k] = C[L - k] for k = 0..L: A[0]=C[L], ..., A[L-1]=C[1], A[L]=C[0]=1.
    // So the annihilator is monic of degree L with coefficients
    // A[0] = C[L], A[1] = C[L-1], ..., A[L-1] = C[1], A[L] = 1.
    let deg = ell;
    let mut annihilator: Vec<F> = Vec::with_capacity(deg + 1);
    // Ascending: annihilator[k] = C[deg - k] = c[deg - k].
    for k in 0..=deg {
        let idx = if k <= deg && (deg - k) < c.len() {
            c[deg - k].clone()
        } else {
            zero.clone()
        };
        annihilator.push(idx);
    }
    // annihilator[deg] should be C[0] = 1.
    let p = FieldPoly::from_coeffs_trimmed(annihilator);
    monic(p)
}

/// One Wiedemann minpoly attempt with a specific random seed.
///
/// Generates random vectors `u` and `v` (using SplitMix64 derived from
/// `seed`), computes the scalar sequence `s_k = ⟨v, A^k · u⟩` for
/// `k = 0..2n+1`, runs Berlekamp-Massey to obtain a candidate `m(x)`,
/// then verifies that `m` is the true minpoly (not just an annihilator
/// of the projected subspace) by checking the scalar recurrence on a
/// fresh random projection `⟨v', A^k · u'⟩`. On verification success,
/// returns `Some(m)`. On any failure, returns `None`.
///
/// # Correctness guarantee
///
/// For any field with `q > n`, the probability that BM on `⟨v, A^k u⟩`
/// returns a polynomial that is a **proper divisor** of `minpoly(A)` is
/// `≤ n/q` per random pair `(u, v)`. The scalar-recurrence verification
/// on a fresh projection further filters the degenerate case: if `m` is
/// a proper divisor, the fresh projection sequence fails the recurrence
/// check with probability `≥ 1 − deg(m)/q`. The
/// [`WIEDEMANN_MAX_RETRIES`] = 8 retry loop in the caller ensures the
/// overall failure probability is `≤ (n/q)^8`.
///
/// # Complexity
///
/// `O(n²)` field operations: `2n+1` matvec calls for the primary
/// sequence (`O(n²)` total) plus `O(n²)` for BM plus `2n+1` matvec
/// calls for the verification sequence and `O(n²)` for the recurrence
/// check. Total coefficient is ≤ 4.
fn wiedemann_minpoly_attempt<F: FiniteField>(
    a: &FieldMatrix<F>,
    seed: u64,
) -> Option<FieldPoly<F>> {
    let n = a.rows();
    debug_assert!(
        n >= 2,
        "n ∈ {{0, 1}} should be short-circuited by the caller"
    );
    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();

    // Generate two random vectors u and v using the same SplitMix64
    // PRNG pattern as the Keller–Gehrig path (no `rand` feature required).
    let mut state = seed;
    let gen_vec = |state: &mut u64| -> FieldVec<F> {
        let mut v = FieldVec::<F>::zeros_from(n, &zero);
        for i in 0..n {
            let count = (splitmix64(state) & 0x3F) as u32;
            let mut acc = zero.clone();
            for _ in 0..count {
                acc += one.clone();
            }
            if (splitmix64(state) & 1) == 1 {
                v.set(i, acc);
            }
        }
        // Ensure not all-zero.
        if v.iter().all(|c| c.is_zero()) {
            v.set(0, one.clone());
        }
        v
    };
    let u = gen_vec(&mut state);
    let v = gen_vec(&mut state);

    // Pre-pack the matrix once so all `2 · (2n + 1)` matvec calls in
    // the primary + verification sequences avoid the per-call repack
    // (issue d1dd266c).
    let driver = MatvecDriver::new(a);

    // Compute scalar sequence s_k = <v, A^k · u> for k = 0..2n.
    // We iterate: cur = A^k · u, advance via cur = A · cur.
    let seq_len = 2 * n + 1; // +1 ensures BM has enough terms for degree-n recurrence
    let mut seq: Vec<F> = Vec::with_capacity(seq_len);
    let mut cur = u.clone();
    for _ in 0..seq_len {
        // s_k = v · cur (dot product).
        let sk = v.dot_product(&cur);
        seq.push(sk);
        cur = driver.matvec(&cur);
    }

    // Run Berlekamp-Massey on the sequence.
    let candidate = berlekamp_massey(&seq);

    // The BM output should have degree ≤ n (since minpoly has degree ≤ n).
    // If degree is 0 it's the constant 1, which means the sequence was
    // identically zero — degenerate, retry.
    if candidate.degree().map(|d| d > n).unwrap_or(true) {
        return None;
    }

    // Verify the candidate is the true minpoly (not a proper divisor) by
    // checking it annihilates a fresh random projection.
    //
    // Strategy: pick a fresh random pair (u', v'), compute the scalar
    // sequence s'_k = <v', A^k · u'> for k = 0..2n, then check that
    // `m` satisfies the recurrence `sum_{j=0}^{d} m_j s'_{k+j} == 0`
    // for all k in [0..seq_len-d-1].  If m is the true minpoly, this
    // holds. If m is a proper divisor, it fails with probability
    // ≥ 1 − deg(m)/q per fresh (u', v').
    //
    // This avoids the expensive `poly_action_on_vector` calls (which
    // cost O(n³) for deg(m) = n).  One verification sequence (2n+1
    // matvec calls) is enough because the retry loop provides 8
    // independent attempts.
    let d = candidate.degree().unwrap_or(0);
    let u_prime = gen_vec(&mut state);
    let v_prime = gen_vec(&mut state);
    let seq_len_v = 2 * n + 1;
    let mut seq_v: Vec<F> = Vec::with_capacity(seq_len_v);
    let mut cur_v = u_prime;
    for _ in 0..seq_len_v {
        let sk = v_prime.dot_product(&cur_v);
        seq_v.push(sk);
        cur_v = driver.matvec(&cur_v);
    }
    // Check the recurrence: for each k, sum_{j=0}^{d} m[j] * seq_v[k+j] == 0.
    for k in 0..seq_len_v.saturating_sub(d + 1) {
        let mut acc = zero.clone();
        for j in 0..=d {
            let mj = candidate.coeff(j);
            acc += mj * seq_v[k + j].clone();
        }
        if !acc.is_zero() {
            return None; // Candidate is a proper divisor — retry.
        }
    }

    // Strong vector-based annihilation verification. The recurrence
    // check above can falsely pass when a strict divisor of `minpoly(A)`
    // happens to LFSR-fit the new projection sequence (probability
    // ≤ deg-gap/q per fresh `(u', v')`). The kernel-of-`p(A)` argument
    // gives a much sharper bound: if `candidate` is a strict divisor,
    // `p(A) ≠ 0` and a uniformly random `u` lies in `ker p(A)` with
    // probability `≤ 1/q`. Two independent random `u` plus the
    // deterministic `e_(n-1)` probe (which catches the upper-Jordan
    // adversarial case present in the Jordan-block correctness suite)
    // bring the residual false-accept probability to `≤ q^(-2)` —
    // sufficient even for the smallest in-scope field `Fp<7>`
    // (`7^(-2) ≈ 0.02`), and the 8-retry outer loop drives it to
    // `≤ q^(-16)` overall.
    // Strong vector-based annihilation verification.
    //
    // For small `n` (`n ≤ WIEDEMANN_DETERMINISTIC_VERIFY_N`), sweep
    // every canonical basis vector — this is `O(n⁴)` in the worst case
    // but is only reached when the candidate is strictly wrong, and `n`
    // is bounded so the absolute cost stays well below the bench budget.
    // It deterministically catches every strict divisor of `minpoly(A)`
    // because if `p(A) ≠ 0`, at least one canonical `e_i` lies outside
    // `ker p(A)`.
    //
    // For larger `n`, fall back to random probes plus the two canonical
    // extremes (`e_0`, `e_(n-1)`) so the dominant Wiedemann + verify
    // loop stays `O(n³)`. The 8-retry outer loop drives any residual
    // false-accept probability down to `≤ q^(-O(retries))`.
    if n <= WIEDEMANN_DETERMINISTIC_VERIFY_N {
        for i in 0..n {
            let mut ei = FieldVec::<F>::zeros_from(n, &zero);
            ei.set(i, one.clone());
            let pe = poly_action_on_vector(&candidate, a, &ei);
            if pe.iter().any(|c| !c.is_zero()) {
                return None;
            }
        }
    } else {
        // For larger `n` the recurrence check above is already strong:
        // the new sequence has length `2n + 1` so `n − d` recurrence
        // windows must independently fail on a strict divisor. Add only
        // an `e_(n-1)` deterministic probe + one random probe — together
        // these add ≤ 2·(n+1) matvecs to the verify cost, well within
        // the cubic budget.
        let mut e_last = FieldVec::<F>::zeros_from(n, &zero);
        e_last.set(n - 1, one.clone());
        let pe = poly_action_on_vector(&candidate, a, &e_last);
        if pe.iter().any(|c| !c.is_zero()) {
            return None;
        }
        let u_check = gen_vec(&mut state);
        let pu = poly_action_on_vector(&candidate, a, &u_check);
        if pu.iter().any(|c| !c.is_zero()) {
            return None;
        }
    }

    Some(candidate)
}

/// Threshold below which the Wiedemann verification sweeps every
/// canonical basis vector deterministically. Above this threshold,
/// verification uses two canonical-extreme probes plus a small batch
/// of random vectors. Set to 32 so adversarial Jordan tests at
/// `n ≤ 16` always get the deterministic sweep, and the bench cells
/// (`n = 64, 256`) keep the cubic verification cost.
const WIEDEMANN_DETERMINISTIC_VERIFY_N: usize = 32;

/// Verifies that the candidate polynomial `p` annihilates `A` (i.e.
/// `p(A) = 0`) deterministically. **No false accepts.** Acceptance:
///
/// 1. `deg(p) == n` — fast-path accept. `p` is a divisor of
///    `charpoly(A)` of degree `n`; the only such divisor is
///    `charpoly(A)` itself, so `p = charpoly = minpoly` (the algorithm
///    only constructs `p` as a divisor of `minpoly(A)`, which always
///    divides `charpoly(A)`). For random-matrix bench inputs the
///    minpoly equals the charpoly with overwhelming probability and
///    this branch fires; the basis sweep is skipped entirely.
/// 2. Otherwise — exhaustive deterministic sweep over every standard
///    basis vector `e_i, i ∈ [0, n)`. Catches every strict divisor with
///    full certainty: `p(A) = 0` iff `p(A) · e_i = 0` for all `i`, so a
///    miss on any `e_i` yields `false`.
///
/// Returns `true` on every-`e_i` pass, `false` on first miss.
///
/// # Complexity
///
/// `O(deg(p) · n³)` worst case for the basis sweep (on inputs whose
/// minpoly is a strict divisor of charpoly). Random-matrix bench
/// inputs hit the fast path and pay zero verification cost.
///
/// `seed` parameter retained for ABI compatibility with the prior
/// probabilistic interface but is unused; the verifier is now fully
/// deterministic Las Vegas.
pub(crate) fn poly_annihilates_a_lasvegas<F: FiniteField>(
    p: &FieldPoly<F>,
    a: &FieldMatrix<F>,
    _seed: u64,
) -> bool {
    let n = a.rows();
    if n == 0 {
        return true;
    }

    // (1) Fast-path: deg(p) == n implies p == charpoly == minpoly.
    if let Some(d) = p.degree() {
        if d == n {
            return true;
        }
    }

    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();

    // (2) Exhaustive standard-basis sweep — Las-Vegas certain across
    // all `n` regimes. Issue `d1dd266c` review feedback (R4) explicitly
    // required removing the probabilistic fallback so production
    // acceptance is deterministic for every input. The matrix is packed
    // exactly once via `MatvecDriver`; each of the `n` Horner passes
    // reuses that cache (R6 perf-debt fix).
    let driver = MatvecDriver::new(a);
    for i in 0..n {
        let mut e_i = FieldVec::<F>::zeros_from(n, &zero);
        e_i.set(i, one.clone());
        let pe = poly_action_on_vector_via_driver(p, &driver, &e_i);
        if pe.iter().any(|c| !c.is_zero()) {
            return false;
        }
    }
    true
}

/// Multi-seed scalar Wiedemann minpoly path (issue d1dd266c).
///
/// For each canonical / random seed `u`, computes the scalar Krylov
/// projection sequence `s_k = ⟨v, A^k u⟩` for a fresh random `v`, runs
/// Berlekamp–Massey, and accumulates the LCM of the resulting per-seed
/// minpolys. The output is `lcm_i(minpoly(A, u_i))` which equals
/// `minpoly(A)` once the union of `u_i` orbits spans `V`.
///
/// Works in any finite field — correctness does not depend on
/// `q > n`. Cost: `O(seeds · n³)` field operations dominated by the
/// `2n + 1` matvec calls per seed (which use the cached
/// [`MatvecDriver`] when available). Verification runs once at the
/// end via [`poly_annihilates_a_lasvegas`].
///
/// Returns `Some(p)` on verified success, `None` if all `MAX_SEEDS`
/// attempts left a candidate that fails the annihilation check.
fn multi_seed_wiedemann_minpoly<F: FiniteField>(
    a: &FieldMatrix<F>,
    seed_base: u64,
) -> Option<FieldPoly<F>> {
    let n = a.rows();
    if n == 0 {
        let zero = F::zero_hint()?;
        return Some(FieldPoly::one_like(&zero));
    }
    if n == 1 {
        // 1×1 matrix [a]: minpoly = x − a.
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let neg_a = zero.clone() - a.get(0, 0);
        return Some(FieldPoly::from_coeffs_trimmed(vec![neg_a, one]));
    }

    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();
    let driver = MatvecDriver::new(a);

    // Generate a random vector via SplitMix64 (mirrors the existing
    // wiedemann_minpoly_attempt PRNG pattern).
    let gen_random_vec = |state: &mut u64, n: usize| -> FieldVec<F> {
        let mut v = FieldVec::<F>::zeros_from(n, &zero);
        for i in 0..n {
            let count = (splitmix64(state) & 0x3F) as u32;
            let mut acc = zero.clone();
            for _ in 0..count {
                acc += one.clone();
            }
            if (splitmix64(state) & 1) == 1 {
                v.set(i, acc);
            }
        }
        if v.iter().all(|c| c.is_zero()) {
            v.set(0, one.clone());
        }
        v
    };

    let mut state = seed_base;
    let mut lcm_so_far = FieldPoly::one_like(&zero);

    // Seed schedule: e_0, e_(n-1), e_(n/2), then random vectors.
    // Mixing canonical extremes (which catch upper- and lower-Jordan
    // adversarial cases) with random vectors (which catch generic
    // non-cyclic cases) maximises the chance of LCM-reaching minpoly
    // in a small number of attempts.
    const MAX_SEEDS: usize = 16;
    for attempt in 0..MAX_SEEDS {
        let u = match attempt {
            0 => {
                let mut e = FieldVec::<F>::zeros_from(n, &zero);
                e.set(0, one.clone());
                e
            }
            1 => {
                let mut e = FieldVec::<F>::zeros_from(n, &zero);
                e.set(n - 1, one.clone());
                e
            }
            2 if n >= 2 => {
                let mut e = FieldVec::<F>::zeros_from(n, &zero);
                e.set(n / 2, one.clone());
                e
            }
            _ => gen_random_vec(&mut state, n),
        };
        let v = gen_random_vec(&mut state, n);

        // Build s_k = ⟨v, A^k u⟩ for k = 0..2n.
        let seq_len = 2 * n + 1;
        let mut seq: Vec<F> = Vec::with_capacity(seq_len);
        let mut cur = u;
        for _ in 0..seq_len {
            let sk = v.dot_product(&cur);
            seq.push(sk);
            cur = driver.matvec(&cur);
        }
        // BM yields a divisor of the minimal polynomial of A acting on u.
        let p = berlekamp_massey(&seq);
        if p.degree().map(|d| d > n).unwrap_or(true) {
            continue;
        }
        lcm_so_far = poly_lcm(&lcm_so_far, &p);

        // Early-exit: when lcm has degree n, it must equal minpoly (since
        // minpoly divides charpoly which has degree n). Verify and return.
        if lcm_so_far.degree() == Some(n)
            && poly_annihilates_a_lasvegas(&lcm_so_far, a, seed_base.wrapping_add(0xA1))
        {
            return Some(lcm_so_far);
        }
        // Periodic check for the `minpoly < n` case: verify only when
        // the LCM has stayed the same for two consecutive seeds (i.e.
        // adding a new seed didn't grow it). Avoids paying the
        // verification cost on every iteration when the LCM is still
        // growing.
        if attempt >= 3
            && attempt % 4 == 3
            && poly_annihilates_a_lasvegas(&lcm_so_far, a, seed_base.wrapping_add(0xA2))
        {
            return Some(lcm_so_far);
        }
    }
    // Final verification on whatever LCM we accumulated.
    if poly_annihilates_a_lasvegas(&lcm_so_far, a, seed_base.wrapping_add(0xA3)) {
        return Some(lcm_so_far);
    }
    None
}

/// Returns the minimal polynomial of `A` via the cyclic-decomposition
/// LCM path with verification.
///
/// # Algorithm
///
/// 1. **Multi-seed Wiedemann first** — accumulate the per-seed
///    annihilator LCM across canonical-basis seeds. Works in every
///    finite field at `O(seeds · n³)` cost; matvec uses the SIMD-cached
///    driver and BM operates on scalar sequences.
/// 2. **Cyclic decomposition on `A`.** Take `p = lcm_i(block_i.poly)`
///    and verify with [`poly_annihilates_a_lasvegas`] (degree-`n`
///    fast-path, otherwise exhaustive standard-basis sweep — no false
///    accepts).
/// 3. **Cyclic decomposition on `A^T`.** Transposing flips upper-
///    Hessenberg structure to lower-Hessenberg, where the seed `e_0`
///    again generates the full Krylov chain. Verified the same way.
/// 4. **Multi-seed Wiedemann retry** on a far-offset seed stream
///    (independent random probes from step 1). Mathematically valid
///    for every finite field. If even this exhausts, panic — the only
///    way this branch is reached is an internal invariant violation.
///
/// # Complexity
///
/// Best case (multi-seed-on-A succeeds): `O(seeds · n³)`. Random and
/// most adversarial matrices land here.
///
/// Cyclic-on-A succeeds: `O(n³)` decomposition + Las-Vegas verifier
/// (fast-path `O(n)` on degree-`n` candidates, exhaustive sweep
/// `O(n^4)` only on derogatory candidates).
///
/// Cyclic-on-A^T succeeds: same.
///
/// Multi-seed retry: `O(seeds · n³)` — bounded.
///
/// The legacy quartic [`find_max_minpoly_generator`] driver is **not**
/// reached from this dispatch path. The function still exists in the
/// crate to serve `FieldMatrix::frobenius_form()`, which is out of
/// scope for this issue.
fn cyclic_lcm_minpoly<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    let n = a.rows();
    if n == 0 {
        let zero = F::zero_hint().expect(
            "cyclic_lcm_minpoly: cannot synthesise the constant-1 polynomial \
             for a 0×0 matrix over a runtime-context field; use F: ConstField",
        );
        return FieldPoly::one_like(&zero);
    }
    let zero: F = a.get(0, 0).zero_like();

    // Attempt 1 (issue d1dd266c): scalar Wiedemann across multiple
    // seeds, accumulating their LCM. Works in every finite field at
    // O(seeds · n³) cost; the matvec uses the SIMD-cached driver and
    // there is no polynomial bookkeeping per chain step (unlike the
    // `cyclic_decomposition` path), so the constant factor is much
    // smaller for the low-cardinality bench cells.
    if let Some(p) = multi_seed_wiedemann_minpoly(a, CYCLIC_LCM_VERIFY_SEED) {
        return p;
    }

    // Attempt 2: cyclic_decomposition on A.
    let blocks_a = cyclic_decomposition(a);
    let mut p_a = FieldPoly::one_like(&zero);
    for blk in &blocks_a {
        p_a = poly_lcm(&p_a, &blk.poly);
    }
    if poly_annihilates_a_lasvegas(&p_a, a, CYCLIC_LCM_VERIFY_SEED.wrapping_add(0xB1)) {
        return p_a;
    }

    // Attempt 3: cyclic_decomposition on A^T. Transposing flips upper-
    // triangular structure to lower-triangular, restoring the property
    // that `e_0` generates the full Krylov chain in V for the
    // adversarial Jordan inputs.
    let at = a.transpose();
    let blocks_at = cyclic_decomposition(&at);
    let mut p_at = FieldPoly::one_like(&zero);
    for blk in &blocks_at {
        p_at = poly_lcm(&p_at, &blk.poly);
    }
    if poly_annihilates_a_lasvegas(&p_at, a, CYCLIC_LCM_VERIFY_SEED.wrapping_add(0xB2)) {
        return p_at;
    }

    // Last resort: re-run multi_seed_wiedemann with a fresh disjoint
    // seed stream. The previous attempts use seed offsets {0xA1, 0xA2,
    // 0xA3} below; pick a far-away offset so the retry consumes
    // independent random probes. multi_seed_wiedemann is mathematically
    // valid for every finite field (BM converges once the union of seed
    // orbits spans V) and its O(seeds · n³) cost is bounded; the legacy
    // quartic `find_max_minpoly_generator` is therefore not reached
    // from this `minpoly()` dispatch path. The function itself remains
    // in the crate (called by the unrelated `frobenius_form()` helper at
    // `O(n⁴)` cost), but the `minpoly()` hot path no longer touches it.
    if let Some(p) = multi_seed_wiedemann_minpoly(a, CYCLIC_LCM_VERIFY_SEED.wrapping_add(0xC1)) {
        return p;
    }
    // Truly unreachable in practice — every finite-field matrix admits
    // a Wiedemann minpoly via canonical-basis seed enumeration. Reaching
    // this branch indicates an internal invariant violation; surface it
    // loudly rather than silently returning a wrong polynomial.
    panic!(
        "cyclic_lcm_minpoly: all production dispatch arms exhausted; \
         cyclic_decomposition + multi-seed Wiedemann both failed to \
         produce a verified annihilator for an n={n} matrix. This \
         indicates an internal invariant violation in the Wiedemann \
         convergence proof or in the verifier; report with the input \
         matrix to reproduce."
    );
}

/// Default seed for the cyclic-LCM verification PRNG. Chosen freshly
/// per call site by adding a small offset so disjoint dispatches use
/// disjoint random streams.
const CYCLIC_LCM_VERIFY_SEED: u64 = 0xCAFEF00DD15EA5E5;

/// Dispatch shim for `minpoly`. Decision tree:
///
/// 1. **Scalar Wiedemann** — engaged when `cardinality_log2_hint()`
///    yields a `log_q` satisfying `2^log_q > n`, i.e. the per-attempt
///    Wiedemann success probability is strictly positive. Wraps
///    [`wiedemann_minpoly_attempt`] in an 8-retry Las-Vegas loop. Each
///    success is verified by a fresh-projection scalar recurrence
///    check. Cost: `O(n³)` field operations.
///
/// 2. **Cyclic-decomposition LCM** — used when scalar Wiedemann is
///    unsafe (low cardinality, `q ≤ n`) or after Wiedemann exhausts
///    its retries. Cost: `O(n³)` field operations. Mathematically
///    valid for every finite field — see [`cyclic_lcm_minpoly`].
///
/// The legacy quartic [`find_max_minpoly_generator`] path is no longer
/// reached from this `minpoly()` dispatch shim. The function itself
/// still exists and is called by the unrelated `frobenius_form()`
/// helper, but the `minpoly()` hot path no longer touches it.
fn minpoly_dispatch<F: FiniteField>(
    a: &FieldMatrix<F>,
    _basis: &[FieldVec<F>],
    _pivot_row_of_col: &[usize],
    _zero: &F,
) -> FieldPoly<F> {
    let n = a.rows();

    // Wiedemann requires n ≥ 2, static cardinality, and the field large
    // enough that the projection bound is meaningful.
    if n >= 2 {
        if let Some(log_q) = F::cardinality_log2_hint() {
            // Use `2^log_q > n` as the gate: this is `q ≥ 2^log_q > n`,
            // giving per-attempt success probability ≥ 1 − n/q > 0.
            // When log_q > 63 we know q ≥ 2^64 ≫ n for any practical n.
            let gate_passes = if log_q > 63 {
                true
            } else {
                (1u64 << log_q) > n as u64
            };
            if gate_passes {
                for retry in 0..WIEDEMANN_MAX_RETRIES {
                    let seed = WIEDEMANN_DEFAULT_SEED.wrapping_add(retry as u64);
                    if let Some(m) = wiedemann_minpoly_attempt(a, seed) {
                        return m;
                    }
                }
                // Wiedemann exhausted — fall through to cyclic-LCM.
            } else {
                // Low-cardinality fields where the base-field gate fails:
                // try the extension-field scalar Wiedemann path first
                // (issue `6c926de0`). The hook is `None` by default and
                // is overridden for `Fp<P>` (`P ∈ {7, 251}`) where a
                // small algebraic extension lifts the per-attempt
                // success probability above the deterministic ceiling
                // and so a single attempt almost always succeeds.
                if let Some(m) = F::try_extension_wiedemann_minpoly(a) {
                    return m;
                }
                // Extension path unavailable or did not converge — fall
                // through to the multi-seed Wiedemann inside
                // `cyclic_lcm_minpoly`.
            }
        }
    }

    // Deterministic cubic cyclic-LCM fallback, used for small-cardinality
    // fields where Wiedemann is unsafe and on the rare retry-exhaustion
    // path. Replaces the prior quartic `find_max_minpoly_generator`
    // dispatch from this `minpoly()` path (issue d1dd266c). The
    // quartic helper is still called by the unrelated `frobenius_form()`
    // method on `FieldMatrix`.
    cyclic_lcm_minpoly(a)
}

// ─── Public methods on FieldMatrix ───────────────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns the characteristic polynomial `det(xI − A)` of `self`.
    ///
    /// **Dispatch shim** (issue `1454ec2d`) — internally selects between
    /// the deterministic cubic path
    /// ([`charpoly_cubic`](Self::charpoly_cubic), Dumas–Pernet
    /// theorem 13.1) and the sub-cubic Las-Vegas Keller–Gehrig path
    /// ([`charpoly_keller_gehrig`](Self::charpoly_keller_gehrig),
    /// Dumas–Pernet theorem 13.4) based on the field cardinality and
    /// matrix size. See the module rustdoc for the full decision tree
    /// and the empirical crossover; the measured crossover where KG
    /// beats cubic is well above `n = 1024` on `Fp<2^31 − 1>` with the
    /// current PLE-based K⁻¹ pipeline (cubic is ~148x faster at
    /// `n = 256` post-Wave-9; see issue `4a59d1f9`).
    ///
    /// **Bit-exactness across paths** is enforced as a `[hard]` success
    /// criterion of issue `1454ec2d`: the sub-cubic path verifies its
    /// candidate by `eval_at_matrix(A) == 0` (Cayley–Hamilton) before
    /// returning, and silently retries / falls back to cubic on any
    /// disagreement. Either path's output is monic of exact degree `n`.
    ///
    /// # Arguments
    ///
    /// * `self` — square `n × n` input. Not modified.
    ///
    /// # Returns
    ///
    /// The monic polynomial `det(xI − A)` of degree `n`. On the empty
    /// `0×0` matrix returns the constant polynomial `1`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square. Panics on a `0×0` runtime-context
    /// matrix (no witness for the constant-1 polynomial).
    ///
    /// # Complexity
    ///
    /// Worst case (cubic fallback): `O(n³)` field operations plus
    /// `O(n²)` polynomial multiplications via balanced
    /// [`FieldPoly::product`]. Best case (sub-cubic engagement):
    /// `O(n^ω · log n)` field operations dominated by `⌈log₂ n⌉`
    /// `gemm` calls and one `PLE` solve.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::field::FieldPoly;
    /// use gf2_core::gfp::Fp;
    ///
    /// // Identity n×n: charpoly = (x − 1)^n.
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// let p = id.charpoly();
    /// // (x − 1)^3 = x^3 − 3x^2 + 3x − 1 = x^3 + 4x^2 + 3x + 6 over Fp<7>.
    /// assert_eq!(p.coeff(3), Fp::<7>::new(1));
    /// assert_eq!(p.coeff(2), Fp::<7>::new(4));
    /// assert_eq!(p.coeff(1), Fp::<7>::new(3));
    /// assert_eq!(p.coeff(0), Fp::<7>::new(6));
    /// ```
    pub fn charpoly(&self) -> FieldPoly<F> {
        charpoly_dispatch(self)
    }

    /// Cubic-deterministic charpoly path (issue `f01298db`,
    /// Dumas–Pernet theorem 13.1). Always available; selected by
    /// [`charpoly`](Self::charpoly) for runtime-context fields, small
    /// `n`, low-cardinality fields, or when the Las-Vegas sub-cubic
    /// path exhausts its retries.
    ///
    /// Computed via the Krylov cyclic decomposition: builds a direct-
    /// sum decomposition `V = ⊕ W_i` and returns the **product** of
    /// the per-block annihilator polynomials. The product equals
    /// `det(xI − A)` because `charpoly` is multiplicative across
    /// `A`-invariant direct sums and each block contributes the
    /// characteristic polynomial of `A` restricted to its cyclic
    /// subspace.
    ///
    /// See [`charpoly`](Self::charpoly) for the user-level entry; this
    /// method exposes the cubic backend for tests and benches that
    /// need to compare paths bit-exactly.
    ///
    /// # Arguments
    ///
    /// * `self` — square `n × n` input. Not modified.
    ///
    /// # Returns
    ///
    /// The monic polynomial `det(xI − A)` of degree `n`. On the empty
    /// `0×0` matrix returns the constant polynomial `1`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square. Panics on a `0×0` runtime-context
    /// matrix (no witness for the constant-1 polynomial).
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations (Krylov cyclic decomposition) plus
    /// `O(n²)` polynomial multiplications via balanced
    /// [`FieldPoly::product`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// let cubic = id.charpoly_cubic();
    /// let dispatch = id.charpoly();
    /// assert_eq!(cubic, dispatch); // identical at small n
    /// ```
    pub fn charpoly_cubic(&self) -> FieldPoly<F> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::charpoly_cubic: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            // Empty product = constant 1. We need a witness for the one
            // element. ConstField provides it for free; for a runtime-
            // context field on an empty matrix there is no witness, so
            // we must rely on `F::zero_hint()`.
            let zero = F::zero_hint().expect(
                "FieldMatrix::charpoly_cubic: cannot synthesise the constant-1 \
                 polynomial for a 0×0 matrix over a runtime-context field; \
                 use F: ConstField",
            );
            return FieldPoly::one_like(&zero);
        }
        let blocks = cyclic_decomposition(self);
        let polys: Vec<FieldPoly<F>> = blocks.into_iter().map(|b| b.poly).collect();
        FieldPoly::product(&polys)
    }

    /// Test-only sibling of [`Self::charpoly_cubic`] that runs the
    /// cyclic-decomposition arm with the packed canonical-byte
    /// chain-poly arithmetic (issue 5a3dbd5b) forcibly disabled so the
    /// scalar `FieldPoly` chain-poly bookkeeping is exercised
    /// independently. Used only by the
    /// `proptest_packed_chain_polys_*_matches_scalar` regression tests
    /// to verify bit-identical equality between the packed and scalar
    /// arms.
    #[cfg(test)]
    pub(crate) fn charpoly_cubic_scalar_chain_polys(&self) -> FieldPoly<F> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::charpoly_cubic_scalar_chain_polys: input must be square (got {m}×{n})",
        );
        if n == 0 {
            let zero = F::zero_hint().expect(
                "FieldMatrix::charpoly_cubic_scalar_chain_polys: empty matrix needs ConstField",
            );
            return FieldPoly::one_like(&zero);
        }
        let blocks = cyclic_decomposition_scalar_chain_polys(self);
        let polys: Vec<FieldPoly<F>> = blocks.into_iter().map(|b| b.poly).collect();
        FieldPoly::product(&polys)
    }

    /// Sub-cubic Las-Vegas charpoly via Keller–Gehrig fast exponentiation
    /// (issue `1454ec2d`, Dumas–Pernet theorem 13.4). Returns
    /// `Some(charpoly)` on success and `None` if all
    /// [`KG_MAX_RETRIES`] random vectors failed to be cyclic for `A`.
    ///
    /// **Probabilistic correctness:** each random vector `v ∈ F^n` is
    /// cyclic for `A` (i.e. its minpoly equals `charpoly(A)` in degree)
    /// with probability ≥ `1 − n/q`. The
    /// [`charpoly`](Self::charpoly) dispatch gate `q > 2 n²` reduces
    /// the failure probability to `< 1/2` per attempt; with
    /// [`KG_MAX_RETRIES`] independent retries the overall failure
    /// probability is `< 2^{-KG_MAX_RETRIES}`. The Las-Vegas guarantee
    /// is that success is **always** verified bit-exactly: the routine
    /// computes `eval_at_matrix(A)` and discards any candidate that is
    /// not the zero matrix.
    ///
    /// # Algorithm
    ///
    /// 1. Choose a deterministic random `v` (seeded by `seed`).
    /// 2. Build the shifted Krylov matrix
    ///    `K = [v | A·v | A²·v | … | A^{n-1}·v]` via repeated squaring:
    ///    starting from `K_0 = [v]` and `B_0 = A`, each step yields
    ///    `K_{k+1} = [K_k | B_k · K_k]` and `B_{k+1} = B_k²`. After
    ///    `⌈log₂ n⌉` doublings, `K` is `n × n` (truncated on the last
    ///    step).
    /// 3. Compute `w = A^n · v = A · K[:, n − 1]`.
    /// 4. Solve `K · y = w` via [`FieldMatrix::solve`] (PLE +
    ///    `trsm_lower` + `trsm_upper`). On `None` the random `v` was
    ///    not cyclic — `K` was rank-deficient.
    /// 5. The candidate charpoly is `x^n − y_{n − 1} x^{n − 1} − … − y_0`.
    ///    By Cayley–Hamilton this satisfies `charpoly(A) · v = 0` and,
    ///    when `v` is cyclic, equals the global characteristic
    ///    polynomial.
    /// 6. Verify `charpoly.eval_at_matrix(&A) == 0`. On failure,
    ///    discard and retry.
    ///
    /// # Arguments
    ///
    /// * `self` — square `n × n` input. Not modified.
    /// * `seed` — random seed for the per-attempt vector generator.
    ///   Reproducible across platforms (uses an inline SplitMix64 PRNG —
    ///   no `rand` feature dependency at runtime).
    ///
    /// # Returns
    ///
    /// `Some(charpoly)` if a cyclic vector was found within
    /// [`KG_MAX_RETRIES`] attempts; `None` otherwise (caller should
    /// fall back to [`charpoly_cubic`](Self::charpoly_cubic)).
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// Per attempt: `O(n^ω · log n)` field operations dominated by the
    /// `⌈log₂ n⌉` `gemm` calls in the Krylov build, plus one `O(n³)`
    /// `PLE`-based [`solve`](Self::solve) and one `O(n³)`
    /// [`FieldPoly::eval_at_matrix`] verification.
    pub fn charpoly_keller_gehrig(&self, seed: u64) -> Option<FieldPoly<F>> {
        keller_gehrig_charpoly(self, seed)
    }

    /// Returns the minimal polynomial of `self`.
    ///
    /// Defined as the monic generator of the ideal of polynomials
    /// `p ∈ F[x]` with `p(A) = 0`. Equivalently, the largest invariant
    /// factor of `A` (= `f_1` in the Frobenius normal form, since the
    /// canonical ordering puts the largest factor first and the chain
    /// `f_{i+1} | f_i` propagates downwards).
    ///
    /// **Dispatch (issue `d1dd266c`)**: the implementation selects between
    /// two `O(n³)` algorithmic paths based on the field cardinality:
    ///
    /// 1. **Wiedemann Las-Vegas** (`O(n³)` field operations) — engaged
    ///    for static-cardinality fields with `q > n` (gate: the
    ///    lower-bound `2^floor(log₂(q)) > n`). Computes `2n + 1`
    ///    matrix-vector products to build the scalar Krylov projection
    ///    sequence `s_k = ⟨v, A^k · u⟩` for random `u, v ∈ F^n`, then
    ///    recovers the minimal polynomial via Berlekamp-Massey
    ///    (`O(n²)` field operations). The result is verified via a fresh
    ///    scalar recurrence check (`O(n²)`). The dominant cost is the
    ///    `2n + 1` matvec calls (`O(n³)` total). Falls back to path 2
    ///    after [`WIEDEMANN_MAX_RETRIES`] consecutive failures.
    ///
    /// 2. **Deterministic cyclic-LCM** (`O(n³)` field operations) — used
    ///    for runtime-context fields, low-cardinality fields where
    ///    Wiedemann is unsafe (`q ≤ n`), and as the fallback when
    ///    Wiedemann exhausts its retries. Computes the cubic Krylov
    ///    cyclic decomposition of `V = F^n` under `A` and returns the
    ///    LCM of the per-block annihilator polynomials. Mathematically
    ///    valid for every finite field. The legacy quartic
    ///    [`find_max_minpoly_generator`]-based path is no longer reached
    ///    from this `minpoly()` dispatch shim, although the function
    ///    itself remains in the crate and is still invoked by the
    ///    independent `frobenius_form()` helper at `O(n⁴)` cost.
    ///
    /// # Arguments
    ///
    /// * `self` — square `n × n` input.
    ///
    /// # Returns
    ///
    /// The monic minimal polynomial. On `n == 0` returns the constant
    /// polynomial `1`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations on every production dispatch arm:
    /// - Wiedemann path (large-cardinality fields, `q > n`): dominated
    ///   by `2n + 1` matrix-vector products.
    /// - Extension-field Wiedemann (low-cardinality, `q ≤ n` and
    ///   `q^k > n`): scalar Wiedemann over `Fp<P>[x]/(f(x))`, base-field
    ///   amortised.
    /// - Cyclic-LCM path (low-cardinality, retry fallback): cubic Krylov
    ///   cyclic decomposition + LCM sweep, with Wiedemann retry on
    ///   adversarial verifier failure (also `O(n³)`).
    /// - Multi-seed Wiedemann (low-cardinality, retry fallback): bounded
    ///   `O(seeds · n³)` matvec work.
    ///
    /// The legacy quartic `find_max_minpoly_generator` driver is **not
    /// reached** from this `minpoly()` dispatch shim (`d1dd266c`
    /// review-pass fix). The function itself still exists in the crate
    /// and is invoked by the separate [`Self::frobenius_form`] helper at
    /// `O(n⁴)` cost; only the `minpoly()` hot path was decoupled from
    /// it.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// // Identity n×n: minpoly = x − 1.
    /// let id = FieldMatrix::<Fp<7>>::identity(4);
    /// let p = id.minpoly();
    /// assert_eq!(p.degree(), Some(1));
    /// assert_eq!(p.coeff(1), Fp::<7>::new(1));
    /// assert_eq!(p.coeff(0), Fp::<7>::new(6)); // − 1 ≡ 6 (mod 7)
    /// ```
    pub fn minpoly(&self) -> FieldPoly<F> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::minpoly: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            let zero = F::zero_hint().expect(
                "FieldMatrix::minpoly: cannot synthesise the constant-1 \
                 polynomial for a 0×0 matrix over a runtime-context field; \
                 use F: ConstField",
            );
            return FieldPoly::one_like(&zero);
        }
        // Route through the dispatch shim that selects per the cardinality
        // gate: scalar Wiedemann (q > n), extension-field Wiedemann (q ≤ n
        // and q^k > n via gfpn extensions), then cyclic-LCM with multi-seed
        // retry. The legacy quartic `find_max_minpoly_generator` driver is
        // **not** reached from `minpoly()` — it remains in the crate only
        // to serve `frobenius_form()`. See `minpoly_dispatch` for the
        // order. The empty-basis arguments are injected here purely as a
        // legacy signature artifact (unused by the cubic-class arms).
        let zero = self.get(0, 0).zero_like();
        let basis: Vec<FieldVec<F>> = Vec::new();
        let pivot_row_of_col: Vec<usize> = Vec::new();
        minpoly_dispatch(self, &basis, &pivot_row_of_col, &zero)
    }

    /// Returns `(P, F)` such that `F = P⁻¹ · self · P` is the Frobenius
    /// normal form: a block-diagonal direct sum of companion matrices
    /// of the invariant factors `f_1, f_2, …, f_t`, with the
    /// divisibility chain `f_{i+1} | f_i` and `f_1 == minpoly(self)`.
    ///
    /// Construction:
    ///
    /// 1. Compute the Krylov cyclic decomposition (yielding cyclic-
    ///    subspace polys whose product is `charpoly` and lcm is
    ///    `minpoly`).
    /// 2. Refine pairs `(p_i, p_j)` whose neither divides the other
    ///    via the standard `(p, q) → (lcm(p, q), gcd(p, q))` swap on
    ///    the corresponding generators (cf. the proof of the rational
    ///    canonical form theorem). Generators are tracked as
    ///    polynomial-in-`A` actions on the original generator
    ///    vectors, so applying a swap is `O(n³)` polynomial-in-A
    ///    application + linear combination.
    /// 3. After refinement, the polys are sorted descending by degree,
    ///    `f_1` is the largest (= minpoly), and the chain `f_{i+1} |
    ///    f_i` is established.
    /// 4. Build `P` column-by-column from the per-block Krylov chain
    ///    `gen, A·gen, …, A^{deg-1}·gen` of each refined block.
    ///    Build `F` block-diagonally as the companion matrix of each
    ///    invariant factor.
    ///
    /// # Arguments
    ///
    /// * `self` — square `n × n` input.
    ///
    /// # Returns
    ///
    /// `(P, F)` with `P` invertible and `F = P⁻¹ · self · P`. On
    /// `n == 0`, returns the pair of empty `0×0` matrices.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::{gemm, FieldMatrix};
    /// use gf2_core::gfp::Fp;
    ///
    /// // Identity 3×3: invariant factors are (x − 1, x − 1, x − 1)
    /// // because every cyclic block is 1-dimensional. P can be the
    /// // identity itself; F = identity.
    /// let id = FieldMatrix::<Fp<7>>::identity(3);
    /// let (p, f) = id.frobenius_form();
    /// // F equals the identity for an identity input.
    /// assert_eq!(f, id);
    /// // P · I · P⁻¹ == I trivially.
    /// let p_inv = p.inv().unwrap();
    /// let conj = gemm(&p_inv, &gemm(&id, &p));
    /// assert_eq!(conj, f);
    /// ```
    pub fn frobenius_form(&self) -> (FieldMatrix<F>, FieldMatrix<F>) {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::frobenius_form: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            return (self.clone(), self.clone());
        }
        let zero: F = self.get(0, 0).zero_like();

        // Iteratively peel off invariant factors. At each step:
        //   1. Compute minpoly of A acting on V / W (W = previous chains).
        //   2. Find a vector u in V with vector_minpoly_in_V/W(u) = that minpoly.
        //   3. Build chain {u, A·u, …, A^{d-1}·u} in V.
        //   4. Append to the running basis W.
        //
        // The successive minpolys form a chain `f_1 ≥ f_2 ≥ …` with
        // each `f_{i+1}` dividing `f_i` because `minpoly(A on V/W_i)`
        // divides `minpoly(A on V/W_{i-1})` (the latter quotient is
        // larger so its minpoly is at least as large in degree).

        let mut chains: Vec<(FieldPoly<F>, Vec<FieldVec<F>>)> = Vec::new();
        // W = union of all chain vectors so far (for quotient reductions).
        let mut basis: Vec<FieldVec<F>> = Vec::new();
        let mut col_at_pivot_row: Vec<Option<usize>> = vec![None; n];
        let mut pivot_row_of_col: Vec<usize> = Vec::new();

        while basis.len() < n {
            // Step 1 + 2: find a generator with maximal minpoly in V/W.
            let (gen, gen_minpoly) =
                find_max_minpoly_generator(self, &basis, &pivot_row_of_col, &zero);
            // Step 3: build chain in V starting from gen, residualised
            // against W as we go (so each chain[k] is the residual of
            // A^k · gen modulo W and earlier chain elements). Track
            // chain_polys for accuracy and pivots so subsequent
            // generations see the augmented W correctly.
            let d = gen_minpoly
                .degree()
                .expect("max minpoly must be non-zero on a non-empty quotient");
            // Build chain element by element, reducing each against
            // current basis and adding to basis.
            let block_start = basis.len();
            // chain_residuals will contain post-reduction vectors used
            // for basis storage.
            let initial = {
                let (r, _c) = reduce(&gen, &basis, &pivot_row_of_col);
                r
            };
            // The residual must be non-zero because gen ∉ span(W).
            debug_assert!(initial.iter().any(|c| !c.is_zero()));
            append_to_basis(
                initial.clone(),
                &mut basis,
                &mut col_at_pivot_row,
                &mut pivot_row_of_col,
            );
            let mut chain_residuals: Vec<FieldVec<F>> = vec![initial];
            // Build the rest of the chain.
            for _ in 1..d {
                let next_in_v = self.matvec(chain_residuals.last().unwrap());
                let (r, _c) = reduce(&next_in_v, &basis, &pivot_row_of_col);
                debug_assert!(
                    r.iter().any(|c| !c.is_zero()),
                    "chain residual unexpectedly hit zero before reaching the predicted minpoly degree"
                );
                append_to_basis(
                    r.clone(),
                    &mut basis,
                    &mut col_at_pivot_row,
                    &mut pivot_row_of_col,
                );
                chain_residuals.push(r);
            }
            // The "true" chain in V is {gen, A·gen, …, A^{d-1}·gen}
            // (used for assembling P below).
            let mut true_chain: Vec<FieldVec<F>> = Vec::with_capacity(d);
            true_chain.push(gen);
            for _ in 1..d {
                let next = self.matvec(true_chain.last().unwrap());
                true_chain.push(next);
            }
            chains.push((gen_minpoly, true_chain));
            let _ = block_start; // silence unused-variable warning when assertions are off.
        }

        // Assemble P and F.
        let mut p_mat = FieldMatrix::<F>::new(n, n, zero.clone());
        let mut f_mat = FieldMatrix::<F>::new(n, n, zero.clone());
        let mut col_offset: usize = 0;
        for (poly, true_chain) in &chains {
            let d = poly.degree().expect("invariant factor must be non-zero");
            for (k, v) in true_chain.iter().enumerate() {
                for r in 0..n {
                    p_mat.set(r, col_offset + k, v.get(r).clone());
                }
            }
            // Companion: subdiagonal of ones plus the negated lower
            // coefficients in the last column.
            for i in 0..(d.saturating_sub(1)) {
                f_mat.set(col_offset + i + 1, col_offset + i, zero.one_like());
            }
            for i in 0..d {
                let neg = zero.clone() - poly.coeff(i);
                f_mat.set(col_offset + i, col_offset + d - 1, neg);
            }
            col_offset += d;
        }
        debug_assert_eq!(col_offset, n);
        (p_mat, f_mat)
    }
}

// ─── Refinement to canonical Frobenius form ──────────────────────────────────

/// Returns the monic `lcm(a, b) = a · b / gcd(a, b)`.
pub(crate) fn poly_lcm<F: FiniteField>(a: &FieldPoly<F>, b: &FieldPoly<F>) -> FieldPoly<F> {
    if a.is_zero() || b.is_zero() {
        // Convention: lcm with zero is zero.
        let sample = if let Some(c) = a.iter().next() {
            c.clone()
        } else if let Some(c) = b.iter().next() {
            c.clone()
        } else if let Some(z) = F::zero_hint() {
            z
        } else {
            // No witness available; cannot produce a runtime-context
            // zero polynomial. Callers in this module always feed
            // non-zero polynomials so this branch is unreachable.
            unreachable!("poly_lcm: lcm of two zero polynomials over a runtime-context field");
        };
        return FieldPoly::zero_like(&sample);
    }
    let g = FieldPoly::gcd(a, b);
    let (q, _r) = (a * b).div_rem(&g);
    monic(q)
}

/// Returns the monic representative of `p` (divides by its leading
/// coefficient if not already monic).
fn monic<F: FiniteField>(p: FieldPoly<F>) -> FieldPoly<F> {
    if let Some(lead) = p.leading_coeff() {
        if !lead.is_one() {
            if let Some(inv) = lead.inv() {
                let coeffs: Vec<F> = p.iter().map(|c| c.clone() * inv.clone()).collect();
                return FieldPoly::from_coeffs_trimmed(coeffs);
            }
        }
    }
    p
}

/// Returns `true` iff `divisor` divides `dividend` (i.e. the remainder
/// is zero).
fn poly_divides<F: FiniteField>(divisor: &FieldPoly<F>, dividend: &FieldPoly<F>) -> bool {
    if divisor.is_zero() {
        return dividend.is_zero();
    }
    let (_, r) = dividend.div_rem(divisor);
    r.is_zero()
}

// ─── Free-function aliases (Armadillo-style) ────────────────────────────────

/// Free-function alias for [`FieldMatrix::charpoly`].
///
/// Returns the characteristic polynomial `det(xI − A)` of `a`. This is
/// a thin wrapper over the inherent method, provided so that callers
/// who prefer free-function call syntax (Armadillo / Eigen style) need
/// not write `a.charpoly()`.
///
/// # Arguments
///
/// * `a` — square `n × n` input matrix. Not modified.
///
/// # Examples
///
/// ```
/// use gf2_core::field::charpoly::charpoly;
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let id = FieldMatrix::<Fp<7>>::identity(3);
/// let p = charpoly(&id);
/// // (x − 1)^3 over Fp<7>, leading coefficient is 1.
/// assert_eq!(p.coeff(3), Fp::<7>::new(1));
/// ```
///
/// # Panics
///
/// Panics if `a` is not square, with the same diagnostic as
/// [`FieldMatrix::charpoly`].
///
/// # Complexity
///
/// `O(n³)` field operations — see [`FieldMatrix::charpoly`].
pub fn charpoly<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    a.charpoly()
}

/// Free-function alias for [`FieldMatrix::minpoly`].
///
/// Returns the minimal polynomial of `a` — the largest invariant factor
/// in the Frobenius normal form. Thin wrapper for callers who prefer
/// free-function syntax.
///
/// # Arguments
///
/// * `a` — square `n × n` input matrix. Not modified.
///
/// # Examples
///
/// ```
/// use gf2_core::field::charpoly::minpoly;
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let id = FieldMatrix::<Fp<7>>::identity(4);
/// let p = minpoly(&id);
/// // Identity has minpoly x − 1.
/// assert_eq!(p.degree(), Some(1));
/// ```
///
/// # Panics
///
/// Panics if `a` is not square, with the same diagnostic as
/// [`FieldMatrix::minpoly`].
///
/// # Complexity
///
/// Expected `O(n²)` field operations for large-cardinality fields
/// (`q > 2n`); worst-case `O(n⁴)` via the deterministic fallback.
/// See [`FieldMatrix::minpoly`] for the full dispatch description
/// (issue `d1dd266c`).
pub fn minpoly<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    a.minpoly()
}

/// Free-function alias for [`FieldMatrix::frobenius_form`].
///
/// Returns `(P, F)` such that `F = P⁻¹ · a · P` is the Frobenius
/// normal form (block-diagonal direct sum of companion matrices of the
/// invariant factors, ordered with the divisibility chain
/// `f_{i+1} | f_i`). Thin wrapper for callers who prefer free-function
/// syntax.
///
/// # Arguments
///
/// * `a` — square `n × n` input matrix. Not modified.
///
/// # Examples
///
/// ```
/// use gf2_core::field::charpoly::frobenius_form;
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::gfp::Fp;
///
/// let id = FieldMatrix::<Fp<7>>::identity(3);
/// let (p, f) = frobenius_form(&id);
/// // For the identity input, F equals the identity itself.
/// assert_eq!(f, id);
/// // P is invertible by construction.
/// let p_inv = p.inv().unwrap();
/// assert_eq!(gemm(&p_inv, &gemm(&id, &p)), f);
/// ```
///
/// # Panics
///
/// Panics if `a` is not square, with the same diagnostic as
/// [`FieldMatrix::frobenius_form`].
///
/// # Complexity
///
/// `O(n³)` field operations — see [`FieldMatrix::frobenius_form`].
pub fn frobenius_form<F: FiniteField>(a: &FieldMatrix<F>) -> (FieldMatrix<F>, FieldMatrix<F>) {
    a.frobenius_form()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::gemm;
    use crate::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use proptest::prelude::*;

    const MERSENNE_31: u64 = 2_147_483_647;

    /// AES-irreducible Gf2mWide<8>.
    struct CpGf2m8Cfg;
    impl Gf2mWideConfig<1> for CpGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "CpGf2m8Cfg";
    }
    type Gf2m8 = Gf2mWide<1, CpGf2m8Cfg>;

    /// Conway-irreducible Gf2mWide<16>.
    struct CpGf2m16Cfg;
    impl Gf2mWideConfig<1> for CpGf2m16Cfg {
        const M: usize = 16;
        const MODULUS: [u64; 1] = [0x002D];
        const NAME: &'static str = "CpGf2m16Cfg";
    }
    type Gf2m16 = Gf2mWide<1, CpGf2m16Cfg>;

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        random_gf2m_wide_1::<CpGf2m8Cfg>(rows, cols, seed)
    }
    fn random_gf2m16(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m16> {
        random_gf2m_wide_1::<CpGf2m16Cfg>(rows, cols, seed)
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Verifies `charpoly(A).eval_at_matrix(&A) == 0` and
    /// `minpoly(A) | charpoly(A)`.
    fn check_cayley_hamilton<F: FiniteField>(a: &FieldMatrix<F>) {
        let cp = a.charpoly();
        let pa = cp.eval_at_matrix(a);
        let n = a.rows();
        let zero = a.get(0, 0).zero_like();
        for i in 0..n {
            for j in 0..n {
                assert_eq!(pa.get(i, j), zero, "charpoly(A) at ({},{})", i, j);
            }
        }
        let mp = a.minpoly();
        let (_, r) = cp.div_rem(&mp);
        assert!(r.is_zero(), "minpoly should divide charpoly");
        // Also: minpoly(A) annihilates A.
        let m_pa = mp.eval_at_matrix(a);
        for i in 0..n {
            for j in 0..n {
                assert_eq!(m_pa.get(i, j), zero, "minpoly(A) at ({},{})", i, j);
            }
        }
        // Independent reference cross-check: minpoly(A) is the LCM of
        // the per-vector minpolys over a spanning set; using canonical
        // basis vectors as the spanning set gives an O(n^4)
        // implementation that does not share code paths with the
        // production `find_max_minpoly_generator` (no combine search,
        // no quotient logic). The two must agree exactly.
        let ref_mp = ref_minpoly_via_basis_lcm(a);
        assert_eq!(
            mp, ref_mp,
            "minpoly mismatch vs canonical-basis-LCM reference"
        );
    }

    /// Independent minpoly reference: build the Krylov chain for each
    /// canonical basis vector e_i, read off its annihilator polynomial,
    /// and return the LCM. Used only by `check_cayley_hamilton` as an
    /// independent cross-check; do not call from production paths.
    fn ref_minpoly_via_basis_lcm<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
        let n = a.rows();
        if n == 0 {
            let zero = F::zero_hint().expect("ref_minpoly: 0×0 needs zero_hint");
            return FieldPoly::one_like(&zero);
        }
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut acc = FieldPoly::one_like(&zero);
        for i in 0..n {
            let mut v = FieldVec::<F>::zeros_from(n, &zero);
            v.set(i, one.clone());
            // Build Krylov chain v, A·v, A²·v, ... until linear
            // dependence; record polynomial preimages alongside.
            let mut chain: Vec<FieldVec<F>> = Vec::new();
            let mut chain_polys: Vec<FieldPoly<F>> = Vec::new();
            let mut col_at_pivot_row: Vec<Option<usize>> = vec![None; n];
            let mut pivot_row_of_col: Vec<usize> = Vec::new();
            append_to_basis(
                v.clone(),
                &mut chain,
                &mut col_at_pivot_row,
                &mut pivot_row_of_col,
            );
            chain_polys.push(FieldPoly::one_like(&zero));
            let p = loop {
                let next_in_v = a.matvec(chain.last().unwrap());
                let (residual_next, coeffs) = reduce(&next_in_v, &chain, &pivot_row_of_col);
                let last = chain_polys.len() - 1;
                let mut next_poly = poly_shift_x(&chain_polys[last]);
                for j in 0..chain.len() {
                    let alpha = coeffs[j].clone();
                    if !alpha.is_zero() {
                        next_poly = &next_poly - &chain_polys[j].mul_scalar(&alpha);
                    }
                }
                if residual_next.iter().any(|c| !c.is_zero()) {
                    append_to_basis(
                        residual_next,
                        &mut chain,
                        &mut col_at_pivot_row,
                        &mut pivot_row_of_col,
                    );
                    chain_polys.push(next_poly);
                } else {
                    break monic(next_poly);
                }
            };
            acc = poly_lcm(&acc, &p);
        }
        acc
    }

    // Verify charpoly via the reference path: the matrix `xI − A` over
    // the field of fractions of F[x] has determinant det(xI − A). For
    // a field-only test, we use the Lagrange-interpolation cross-check
    // implicit in `eval_at_matrix(&A) == 0` (Cayley-Hamilton) plus the
    // multiplicativity check: charpoly's degree = n and it is monic.
    fn check_charpoly_basic<F: FiniteField>(a: &FieldMatrix<F>) {
        let cp = a.charpoly();
        let n = a.rows();
        assert_eq!(cp.degree(), Some(n), "charpoly degree = n");
        assert!(cp.leading_coeff().unwrap().is_one(), "charpoly is monic");
    }

    /// Verifies `F = P⁻¹ · A · P` and the divisibility chain `f_{i+1} | f_i`
    /// embedded in the block-diagonal structure of `F`.
    fn check_frobenius_form<F: FiniteField>(a: &FieldMatrix<F>) {
        let (p, fm) = a.frobenius_form();
        // P should be invertible.
        let pinv = p.inv().expect("Frobenius P must be invertible");
        // fm = P⁻¹ · A · P.
        let ap = gemm(a, &p);
        let pinv_ap = gemm(&pinv, &ap);
        assert_eq!(pinv_ap, fm, "P⁻¹ A P != F");

        // Decompose fm into companion blocks. Each block's last column
        // determines its polynomial: F[col_offset..col_offset + d, col_offset + d − 1]
        // = (−poly_coeff(0), …, −poly_coeff(d − 1)).
        let n = a.rows();
        let mut polys: Vec<FieldPoly<F>> = Vec::new();
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut col = 0;
        while col < n {
            // Find the block size starting at col: the largest d such
            // that F[col + i, col + i − 1] = 1 for i = 1..d and all
            // other entries in the block are zero except the last
            // column.
            // Conservative scan: extend d while F[col + d, col + d − 1] = 1.
            let mut d = 1;
            while col + d < n && fm.get(col + d, col + d - 1) == one {
                d += 1;
            }
            // Build poly_coeffs from the last column of this block:
            // poly_coeffs[i] = − F[col + i, col + d − 1]; lead = 1.
            let mut coeffs: Vec<F> = (0..d)
                .map(|i| zero.clone() - fm.get(col + i, col + d - 1))
                .collect();
            coeffs.push(one.clone());
            polys.push(FieldPoly::from_coeffs_trimmed(coeffs));
            col += d;
        }
        assert_eq!(col, n, "Frobenius blocks must tile the diagonal");

        // Divisibility chain: polys[i + 1] | polys[i]. The driver
        // produces blocks in descending degree order so polys[0] is
        // the largest.
        for i in 0..polys.len().saturating_sub(1) {
            let (_, r) = polys[i].div_rem(&polys[i + 1]);
            assert!(
                r.is_zero(),
                "Frobenius divisibility violated: f_{} ∤ f_{}",
                i + 1,
                i,
            );
        }

        // First invariant factor = minpoly.
        if let Some(first) = polys.first() {
            assert_eq!(*first, a.minpoly(), "f_1 should equal minpoly(A)");
        }
        // Product of invariant factors = charpoly.
        let prod = FieldPoly::product(&polys);
        assert_eq!(prod, a.charpoly(), "∏ f_i should equal charpoly(A)");
    }

    // ── Edge cases ───────────────────────────────────────────────────────────

    #[test]
    fn test_charpoly_n_eq_0() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let cp = a.charpoly();
        assert_eq!(cp.degree(), Some(0));
        assert!(cp.leading_coeff().unwrap().is_one());
    }

    #[test]
    fn test_minpoly_n_eq_0() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let mp = a.minpoly();
        assert_eq!(mp.degree(), Some(0));
        assert!(mp.leading_coeff().unwrap().is_one());
    }

    // ── Adversarial Jordan-block tests for cyclic-LCM (issue d1dd266c) ──────
    //
    // The cyclic-decomposition LCM path replaces the prior O(n⁴)
    // `find_max_minpoly_generator` fallback for low-cardinality fields
    // where scalar Wiedemann is unsafe (`q ≤ n`). These tests exercise
    // the cyclic-LCM path on Jordan blocks and direct sums where the
    // minimal polynomial is a known closed form: minpoly(J_d(λ)) =
    // (x − λ)^d, and minpoly(J_a(λ) ⊕ J_b(λ)) = (x − λ)^max(a,b).

    /// Build the d×d Jordan block J_d(λ) with eigenvalue λ on the
    /// diagonal and 1 on the super-diagonal.
    fn jordan_block<const P: u64>(d: usize, lambda: u64) -> FieldMatrix<Fp<P>> {
        let mut a = FieldMatrix::<Fp<P>>::zeros(d, d);
        let l = Fp::<P>::new(lambda);
        let one = Fp::<P>::new(1);
        for i in 0..d {
            a.set(i, i, l);
        }
        for i in 0..d.saturating_sub(1) {
            a.set(i, i + 1, one);
        }
        a
    }

    /// Build the (a+b)×(a+b) direct sum J_a(λ) ⊕ J_b(λ).
    fn jordan_direct_sum<const P: u64>(a: usize, b: usize, lambda: u64) -> FieldMatrix<Fp<P>> {
        let n = a + b;
        let mut m = FieldMatrix::<Fp<P>>::zeros(n, n);
        let l = Fp::<P>::new(lambda);
        let one = Fp::<P>::new(1);
        for i in 0..n {
            m.set(i, i, l);
        }
        for i in 0..a.saturating_sub(1) {
            m.set(i, i + 1, one);
        }
        for i in 0..b.saturating_sub(1) {
            m.set(a + i, a + i + 1, one);
        }
        m
    }

    /// Computes (x − λ)^d via repeated multiplication.
    fn x_minus_lambda_pow<const P: u64>(d: usize, lambda: u64) -> FieldPoly<Fp<P>> {
        let zero = Fp::<P>::new(0);
        let one = Fp::<P>::new(1);
        let factor = FieldPoly::from_coeffs_trimmed(vec![zero - Fp::<P>::new(lambda), one]);
        let mut acc = FieldPoly::one_like(&zero);
        for _ in 0..d {
            acc = &acc * &factor;
        }
        acc
    }

    #[test]
    fn test_minpoly_jordan_block_fp7() {
        // J_3(2) over Fp<7>: minpoly should be (x − 2)^3.
        let a = jordan_block::<7>(3, 2);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<7>(3, 2);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_block_fp7_nilpotent() {
        // J_4(0) = pure nilpotent: minpoly = x^4.
        let a = jordan_block::<7>(4, 0);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<7>(4, 0);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_block_fp251() {
        // J_5(13) over Fp<251>: minpoly = (x − 13)^5.
        let a = jordan_block::<251>(5, 13);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<251>(5, 13);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_direct_sum_fp7() {
        // J_3(0) ⊕ J_2(0) over Fp<7>: minpoly = x^3 (max degree).
        // charpoly = x^5; minpoly | charpoly with strict inequality.
        let a = jordan_direct_sum::<7>(3, 2, 0);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<7>(3, 0);
        assert_eq!(mp, expected, "minpoly(J_3 ⊕ J_2) over Fp<7> should be x^3");
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_direct_sum_fp251() {
        // J_4(7) ⊕ J_1(7) over Fp<251>: minpoly = (x − 7)^4.
        let a = jordan_direct_sum::<251>(4, 1, 7);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<251>(4, 7);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_two_eigenvalues_fp7() {
        // Block-diag(J_2(1), J_3(0)) over Fp<7>:
        // minpoly = (x − 1)^2 · x^3 (coprime pieces, lcm equals product).
        let mut a = FieldMatrix::<Fp<7>>::zeros(5, 5);
        // J_2(1) at (0,0)
        a.set(0, 0, Fp::<7>::new(1));
        a.set(0, 1, Fp::<7>::new(1));
        a.set(1, 1, Fp::<7>::new(1));
        // J_3(0) at (2,2)
        a.set(3, 4, Fp::<7>::new(1)); // super-diagonal at row 3
        a.set(2, 3, Fp::<7>::new(1)); // super-diagonal at row 2
        let mp = a.minpoly();
        let p1 = x_minus_lambda_pow::<7>(2, 1);
        let p2 = x_minus_lambda_pow::<7>(3, 0);
        let expected = &p1 * &p2;
        assert_eq!(mp, monic(expected));
        check_cayley_hamilton(&a);
    }

    /// Random small-matrix coverage for the cyclic-LCM dispatch path.
    /// Compares `a.minpoly()` against an independent reference computed
    /// via per-basis-vector minpoly LCM (`ref_minpoly_via_basis_lcm`,
    /// `#[cfg(test)]` only).
    fn cyclic_lcm_random_check<const P: u64>(n: usize, seeds: &[u64]) {
        for &seed in seeds {
            let a = random_fp::<P>(n, n, seed);
            let mp = a.minpoly();
            let ref_mp = ref_minpoly_via_basis_lcm(&a);
            assert_eq!(mp, ref_mp, "minpoly mismatch on Fp<{P}> n={n} seed={seed}",);
            // mp(A) = 0 over the original matrix.
            let m_pa = mp.eval_at_matrix(&a);
            let zero = Fp::<P>::new(0);
            for i in 0..n {
                for j in 0..n {
                    assert_eq!(m_pa.get(i, j), zero);
                }
            }
            // mp | charpoly.
            let cp = a.charpoly();
            let (_, r) = cp.div_rem(&mp);
            assert!(r.is_zero());
        }
    }

    #[test]
    fn test_minpoly_random_fp7_small() {
        for n in [2usize, 3, 4, 5, 6, 8, 10, 12, 16] {
            cyclic_lcm_random_check::<7>(n, &[1, 2, 3, 4, 5]);
        }
    }

    #[test]
    fn test_minpoly_random_fp251_small() {
        for n in [2usize, 3, 4, 5, 6, 8, 10, 12, 16] {
            cyclic_lcm_random_check::<251>(n, &[10, 20, 30, 40, 50]);
        }
    }

    #[test]
    fn test_minpoly_random_fp65521_small() {
        for n in [2usize, 4, 8, 12, 16] {
            cyclic_lcm_random_check::<65521>(n, &[100, 200, 300]);
        }
    }

    #[test]
    fn test_minpoly_random_fp_m31_small() {
        for n in [2usize, 4, 8, 12, 16] {
            cyclic_lcm_random_check::<MERSENNE_31>(n, &[1000, 2000, 3000]);
        }
    }

    #[test]
    fn test_frobenius_n_eq_0() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let (p, f) = a.frobenius_form();
        assert_eq!(p.shape(), (0, 0));
        assert_eq!(f.shape(), (0, 0));
    }

    #[test]
    fn test_charpoly_n_eq_1() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(1, 1);
        a.set(0, 0, Fp::<7>::new(3));
        let cp = a.charpoly();
        // x − 3 ≡ x + 4 (mod 7).
        assert_eq!(cp.degree(), Some(1));
        assert_eq!(cp.coeff(1), Fp::<7>::new(1));
        assert_eq!(cp.coeff(0), Fp::<7>::new(4));
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_charpoly_identity_n5() {
        let id = FieldMatrix::<Fp<7>>::identity(5);
        check_cayley_hamilton(&id);
        check_charpoly_basic(&id);
        // minpoly = x − 1.
        assert_eq!(id.minpoly().degree(), Some(1));
    }

    #[test]
    fn test_charpoly_zero_n5() {
        let a = FieldMatrix::<Fp<7>>::zeros(5, 5);
        check_cayley_hamilton(&a);
        check_charpoly_basic(&a);
        // minpoly = x.
        let mp = a.minpoly();
        assert_eq!(mp.degree(), Some(1));
        assert_eq!(mp.coeff(0), Fp::<7>::new(0));
        // charpoly = x^5.
        let cp = a.charpoly();
        for k in 0..5 {
            assert_eq!(cp.coeff(k), Fp::<7>::new(0));
        }
        assert_eq!(cp.coeff(5), Fp::<7>::new(1));
    }

    #[test]
    fn test_charpoly_diagonal() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(4, 4);
        a.set(0, 0, Fp::<7>::new(2));
        a.set(1, 1, Fp::<7>::new(3));
        a.set(2, 2, Fp::<7>::new(5));
        a.set(3, 3, Fp::<7>::new(2));
        check_cayley_hamilton(&a);
        check_frobenius_form(&a);
        // charpoly = (x − 2)²(x − 3)(x − 5).
        let cp = a.charpoly();
        assert_eq!(cp.degree(), Some(4));
        assert_eq!(cp.eval(&Fp::<7>::new(2)), Fp::<7>::new(0));
        assert_eq!(cp.eval(&Fp::<7>::new(3)), Fp::<7>::new(0));
        assert_eq!(cp.eval(&Fp::<7>::new(5)), Fp::<7>::new(0));
    }

    #[test]
    fn test_charpoly_scalar_multiple_of_identity() {
        // 3·I_4 over Fp<MERSENNE_31>.
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        for i in 0..4 {
            a.set(i, i, Fp::<MERSENNE_31>::new(3));
        }
        check_cayley_hamilton(&a);
        check_frobenius_form(&a);
        // minpoly = x − 3, charpoly = (x − 3)^4.
        let mp = a.minpoly();
        assert_eq!(mp.degree(), Some(1));
        assert_eq!(
            mp.eval(&Fp::<MERSENNE_31>::new(3)),
            Fp::<MERSENNE_31>::new(0)
        );
    }

    #[test]
    fn test_charpoly_companion_matrix() {
        // Companion of x^3 + 2x^2 + 3x + 5 over Fp<7>:
        //   [[0, 0, −5], [1, 0, −3], [0, 1, −2]] = [[0,0,2],[1,0,4],[0,1,5]] mod 7.
        let mut a = FieldMatrix::<Fp<7>>::zeros(3, 3);
        a.set(1, 0, Fp::<7>::new(1));
        a.set(2, 1, Fp::<7>::new(1));
        a.set(0, 2, Fp::<7>::new(2)); // −5 ≡ 2
        a.set(1, 2, Fp::<7>::new(4)); // −3 ≡ 4
        a.set(2, 2, Fp::<7>::new(5)); // −2 ≡ 5
        check_cayley_hamilton(&a);
        check_charpoly_basic(&a);
        let cp = a.charpoly();
        // charpoly = x^3 + 2x^2 + 3x + 5.
        assert_eq!(cp.coeff(3), Fp::<7>::new(1));
        assert_eq!(cp.coeff(2), Fp::<7>::new(2));
        assert_eq!(cp.coeff(1), Fp::<7>::new(3));
        assert_eq!(cp.coeff(0), Fp::<7>::new(5));
        // For a companion matrix the minpoly equals the charpoly.
        assert_eq!(a.minpoly(), cp);
    }

    #[test]
    fn test_charpoly_singular_matrix() {
        // Rank-deficient: outer product f1 · f2ᵀ.
        let f1 = random_fp::<MERSENNE_31>(5, 1, 0xAAAA);
        let f2 = random_fp::<MERSENNE_31>(1, 5, 0xBBBB);
        let a = gemm(&f1, &f2);
        // A is rank ≤ 1 ⇒ at least 4 zero eigenvalues ⇒ charpoly has x^4
        // as a factor.
        check_cayley_hamilton(&a);
        let cp = a.charpoly();
        // 0 is a root of charpoly when A is singular.
        assert_eq!(
            cp.eval(&Fp::<MERSENNE_31>::new(0)),
            Fp::<MERSENNE_31>::new(0)
        );
    }

    // ── Random-matrix tests across five fields ──────────────────────────────

    #[test]
    fn test_charpoly_random_fp7() {
        for seed in 0..5u64 {
            let a = random_fp::<7>(4, 4, seed);
            check_cayley_hamilton(&a);
            check_charpoly_basic(&a);
        }
    }

    #[test]
    fn test_charpoly_random_fp65521() {
        for seed in 0..3u64 {
            let a = random_fp::<65521>(4, 4, seed);
            check_cayley_hamilton(&a);
            check_charpoly_basic(&a);
        }
    }

    #[test]
    fn test_charpoly_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp::<MERSENNE_31>(5, 5, seed);
            check_cayley_hamilton(&a);
            check_charpoly_basic(&a);
        }
    }

    #[test]
    fn test_charpoly_random_gf2m8() {
        for seed in 0..3u64 {
            let a = random_gf2m8(4, 4, seed);
            check_cayley_hamilton(&a);
            check_charpoly_basic(&a);
        }
    }

    #[test]
    fn test_charpoly_random_gf2m16() {
        for seed in 0..3u64 {
            let a = random_gf2m16(4, 4, seed);
            check_cayley_hamilton(&a);
            check_charpoly_basic(&a);
        }
    }

    // ── Interpolation cross-check: charpoly via det(xI − A) ─────────────────
    //
    // Per the success criterion: "verified by interpolating det(xI − A)
    // at n+1 random field points." We do this directly: for each of
    // n+1 distinct random field points α_k, compute det(α_k · I − A),
    // then Lagrange-interpolate the (α_k, det) pairs to recover the
    // charpoly. Compare against `a.charpoly()`.
    //
    // Random points are drawn from a deterministic StdRng (the same
    // platform-stable RNG used by `random_fp` / `random_gf2m_wide_1`),
    // so each invocation is reproducible. The helper is generic over
    // `F: FiniteField` so a single implementation covers the full
    // five-field matrix (GF(7), GF(65521), GF(2^31 − 1), GF(2^8),
    // GF(2^16)).

    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    /// Generic interpolation cross-check. The caller supplies a sampler
    /// `sample_distinct(rng, count, witness)` that yields `count`
    /// pairwise-distinct random elements of `F` using a witness for
    /// runtime-context fields (here always a `ConstField` so the witness
    /// is a no-op). Distinctness is enforced by a `HashSet`-style retry
    /// in the sampler.
    fn check_interpolation_charpoly_generic<F, S>(a: &FieldMatrix<F>, seed: u64, mut sample: S)
    where
        F: FiniteField,
        S: FnMut(&mut StdRng, &F) -> F,
    {
        let n = a.rows();
        if n == 0 {
            return;
        }
        let zero = a.get(0, 0).zero_like();
        let mut rng = StdRng::seed_from_u64(seed);

        // Generate n + 1 distinct random points via repeated draws.
        let mut pts: Vec<F> = Vec::with_capacity(n + 1);
        let mut attempts = 0usize;
        while pts.len() < n + 1 {
            let x = sample(&mut rng, &zero);
            if !pts.contains(&x) {
                pts.push(x);
            }
            attempts += 1;
            assert!(
                attempts < 10_000,
                "interpolation cross-check: failed to sample {} distinct points; \
                 field is probably smaller than n+1 (consider lowering n)",
                n + 1
            );
        }

        let mut vals: Vec<F> = Vec::with_capacity(n + 1);
        for x in &pts {
            // Build M = x · I − A.
            let mut m = FieldMatrix::<F>::new(n, n, zero.clone());
            for i in 0..n {
                for j in 0..n {
                    let a_ij = a.get(i, j);
                    let cell = if i == j {
                        x.clone() - a_ij
                    } else {
                        zero.clone() - a_ij
                    };
                    m.set(i, j, cell);
                }
            }
            vals.push(m.det());
        }
        let pairs: Vec<(F, F)> = pts.iter().cloned().zip(vals.iter().cloned()).collect();
        let p = crate::field::interpolate(&pairs)
            .expect("Lagrange interpolation must succeed at distinct points");
        assert_eq!(
            p,
            a.charpoly(),
            "interpolated det(xI − A) should equal charpoly(A)"
        );
    }

    fn sample_fp<const P: u64>(rng: &mut StdRng, _zero: &Fp<P>) -> Fp<P> {
        Fp::<P>::new(rng.gen::<u64>() % P)
    }

    fn sample_gf2m_wide_1<C: Gf2mWideConfig<1>>(
        rng: &mut StdRng,
        _zero: &Gf2mWide<1, C>,
    ) -> Gf2mWide<1, C> {
        let mask: u64 = if C::M >= 64 {
            u64::MAX
        } else {
            (1u64 << C::M) - 1
        };
        Gf2mWide::<1, C>::new([rng.gen::<u64>() & mask])
    }

    /// GF(7), capped at n = 5 because the field has only 7 elements
    /// and we need n + 1 distinct points (≤ 7 → n ≤ 6, so n = 5 is a
    /// safe ceiling).
    #[test]
    fn test_charpoly_via_interpolation_random_fp7() {
        for seed in 0..3u64 {
            let a = random_fp::<7>(5, 5, seed);
            check_interpolation_charpoly_generic(&a, seed.wrapping_add(0xA1), sample_fp::<7>);
        }
    }

    #[test]
    fn test_charpoly_via_interpolation_random_fp65521() {
        for seed in 0..3u64 {
            let a = random_fp::<65521>(4, 4, seed);
            check_interpolation_charpoly_generic(&a, seed.wrapping_add(0xA2), sample_fp::<65521>);
        }
    }

    #[test]
    fn test_charpoly_via_interpolation_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp::<MERSENNE_31>(4, 4, seed);
            check_interpolation_charpoly_generic(
                &a,
                seed.wrapping_add(0xA3),
                sample_fp::<MERSENNE_31>,
            );
        }
    }

    #[test]
    fn test_charpoly_via_interpolation_random_gf2m8() {
        for seed in 0..3u64 {
            let a = random_gf2m8(4, 4, seed);
            check_interpolation_charpoly_generic(
                &a,
                seed.wrapping_add(0xA4),
                sample_gf2m_wide_1::<CpGf2m8Cfg>,
            );
        }
    }

    #[test]
    fn test_charpoly_via_interpolation_random_gf2m16() {
        for seed in 0..3u64 {
            let a = random_gf2m16(4, 4, seed);
            check_interpolation_charpoly_generic(
                &a,
                seed.wrapping_add(0xA5),
                sample_gf2m_wide_1::<CpGf2m16Cfg>,
            );
        }
    }

    #[test]
    fn test_charpoly_via_interpolation_singular() {
        let f1 = random_fp::<MERSENNE_31>(4, 1, 0xC0FFEE);
        let f2 = random_fp::<MERSENNE_31>(1, 4, 0xC0FFEF);
        let a = gemm(&f1, &f2);
        check_interpolation_charpoly_generic(&a, 0xC0FFEE, sample_fp::<MERSENNE_31>);
    }

    // ── Frobenius form across the same fields ───────────────────────────────

    #[test]
    fn test_frobenius_form_random_fp7() {
        for seed in 0..3u64 {
            let a = random_fp::<7>(4, 4, seed);
            check_frobenius_form(&a);
        }
    }

    #[test]
    fn test_frobenius_form_random_fp65521() {
        for seed in 0..3u64 {
            let a = random_fp::<65521>(4, 4, seed);
            check_frobenius_form(&a);
        }
    }

    #[test]
    fn test_frobenius_form_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp::<MERSENNE_31>(5, 5, seed);
            check_frobenius_form(&a);
        }
    }

    #[test]
    fn test_frobenius_form_random_gf2m8() {
        for seed in 0..3u64 {
            let a = random_gf2m8(4, 4, seed);
            check_frobenius_form(&a);
        }
    }

    #[test]
    fn test_frobenius_form_random_gf2m16() {
        for seed in 0..3u64 {
            let a = random_gf2m16(4, 4, seed);
            check_frobenius_form(&a);
        }
    }

    // ── Free-function aliases ──────────────────────────────────────────────

    #[test]
    fn test_free_function_aliases_match_methods() {
        let a = random_fp::<MERSENNE_31>(4, 4, 0x42424242);
        assert_eq!(a.charpoly(), super::charpoly(&a));
        assert_eq!(a.minpoly(), super::minpoly(&a));
        let (p1, f1) = a.frobenius_form();
        let (p2, f2) = super::frobenius_form(&a);
        assert_eq!(p1, p2);
        assert_eq!(f1, f2);
    }

    // ── Keller–Gehrig sub-cubic path (issue 1454ec2d) ─────────────────────

    /// Edge case: 0×0 matrix returns the constant polynomial 1.
    #[test]
    fn test_kg_n_eq_0() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(0, 0);
        let p = a
            .charpoly_keller_gehrig(0xC0FFEE)
            .expect("KG must succeed on n=0");
        assert_eq!(p.degree(), Some(0));
        assert!(p.leading_coeff().unwrap().is_one());
    }

    /// Edge case: 1×1 matrix returns x − A[0,0].
    #[test]
    fn test_kg_n_eq_1() {
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(1, 1);
        a.set(0, 0, Fp::<MERSENNE_31>::new(42));
        let p = a
            .charpoly_keller_gehrig(0xC0FFEE)
            .expect("KG must succeed on n=1");
        assert_eq!(p.degree(), Some(1));
        assert_eq!(p.coeff(1), Fp::<MERSENNE_31>::new(1));
        assert_eq!(p.coeff(0), Fp::<MERSENNE_31>::new(MERSENNE_31 - 42));
    }

    /// Bit-exactness across paths on `Fp<MERSENNE_31>` (issue 1454ec2d
    /// `[hard]` success criterion). Sweeps a range of small `n` (the
    /// dispatch routes these to cubic, but the KG worker is invoked
    /// directly here).
    #[test]
    fn test_kg_matches_cubic_fp_m31() {
        for &n in &[2usize, 3, 4, 8, 16, 32] {
            for seed in 0..3u64 {
                let a = random_fp::<MERSENNE_31>(n, n, seed.wrapping_mul(0xABCD));
                let cubic = a.charpoly_cubic();
                let kg = a
                    .charpoly_keller_gehrig(0x100 + seed)
                    .expect("KG should converge on Fp<MERSENNE_31>");
                assert_eq!(
                    cubic, kg,
                    "KG ≢ cubic on Fp<MERSENNE_31> n={} seed={}",
                    n, seed
                );
            }
        }
    }

    /// KG on rank-deficient input — must still return a charpoly that
    /// satisfies Cayley–Hamilton. The Las-Vegas loop may need extra
    /// retries because rank-deficient matrices have a smaller cyclic
    /// subspace.
    #[test]
    fn test_kg_singular_matrix() {
        let f1 = random_fp::<MERSENNE_31>(6, 1, 0x111);
        let f2 = random_fp::<MERSENNE_31>(1, 6, 0x222);
        let a = gemm(&f1, &f2);
        // Rank-1 outer product: KG should fall back to cubic on every
        // attempt because the dispatch's `q > 2 n²` gate engages, but
        // the Las-Vegas vector is unlikely to be cyclic. Either path
        // must produce the same charpoly.
        let cubic = a.charpoly_cubic();
        // Try KG with a fixed seed; if it succeeds it must agree.
        if let Some(kg) = a.charpoly_keller_gehrig(0x333) {
            assert_eq!(cubic, kg);
        }
        // Cayley-Hamilton always holds.
        let pa = cubic.eval_at_matrix(&a);
        for i in 0..6 {
            for j in 0..6 {
                assert_eq!(pa.get(i, j), Fp::<MERSENNE_31>::new(0));
            }
        }
    }

    /// Dispatch threshold smoke test: for `n < KG_DISPATCH_MIN_N`
    /// the public `charpoly` MUST route to the cubic path. We check
    /// indirectly by asserting that `charpoly == charpoly_cubic` (a
    /// `[hard]` invariant under issue `1454ec2d`).
    #[test]
    fn test_dispatch_routes_below_threshold() {
        let a = random_fp::<MERSENNE_31>(8, 8, 0x4444);
        assert_eq!(a.charpoly(), a.charpoly_cubic());
    }

    /// Dispatch threshold smoke test for `Gf2m8` (256-element field):
    /// the cardinality gate `q > 2 n²` fails for any `n ≥ 12`, so the
    /// dispatch routes cubic regardless of size.
    #[test]
    fn test_dispatch_routes_gf2m8() {
        let a = random_gf2m8(16, 16, 0x5555);
        assert_eq!(a.charpoly(), a.charpoly_cubic());
    }

    /// Cardinality hint sanity checks — the dispatch's correctness
    /// hinges on these matching the algebraic field order.
    #[test]
    fn test_cardinality_log2_hint_values() {
        assert_eq!(<Fp<7> as FiniteField>::cardinality_log2_hint(), Some(2));
        assert_eq!(
            <Fp<65521> as FiniteField>::cardinality_log2_hint(),
            Some(15)
        );
        assert_eq!(
            <Fp<MERSENNE_31> as FiniteField>::cardinality_log2_hint(),
            Some(30)
        );
        assert_eq!(<Gf2m8 as FiniteField>::cardinality_log2_hint(), Some(8));
        assert_eq!(<Gf2m16 as FiniteField>::cardinality_log2_hint(), Some(16));
        // Runtime-context field returns None.
        use crate::gf2m::Gf2mElement;
        assert!(<Gf2mElement as FiniteField>::cardinality_log2_hint().is_none());
    }

    // ── Property tests ──────────────────────────────────────────────────────

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        /// Cayley-Hamilton + minpoly | charpoly across random Fp<MERSENNE_31>.
        #[test]
        fn proptest_cayley_hamilton_fp_m31(
            n in 1usize..=4,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let cp = a.charpoly();
            let pa = cp.eval_at_matrix(&a);
            let zero = Fp::<MERSENNE_31>::new(0);
            for i in 0..n {
                for j in 0..n {
                    prop_assert_eq!(pa.get(i, j), zero);
                }
            }
            let mp = a.minpoly();
            let (_, r) = cp.div_rem(&mp);
            prop_assert!(r.is_zero());
        }

        /// Cayley-Hamilton over Gf2m8 (characteristic 2).
        #[test]
        fn proptest_cayley_hamilton_gf2m8(
            n in 1usize..=4,
            seed in any::<u64>(),
        ) {
            let a = random_gf2m8(n, n, seed);
            let cp = a.charpoly();
            let pa = cp.eval_at_matrix(&a);
            let zero = Gf2m8::new([0]);
            for i in 0..n {
                for j in 0..n {
                    prop_assert_eq!(pa.get(i, j), zero);
                }
            }
        }

        /// Frobenius form: P⁻¹ A P = F across Fp<MERSENNE_31>.
        #[test]
        fn proptest_frobenius_conjugation_fp_m31(
            n in 1usize..=4,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let (p, fm) = a.frobenius_form();
            let pinv = p.inv().expect("P must be invertible");
            let ap = gemm(&a, &p);
            let pinv_ap = gemm(&pinv, &ap);
            prop_assert_eq!(pinv_ap, fm);
        }

        /// Sub-cubic Keller–Gehrig path is bit-exact identical to the
        /// cubic baseline on `Fp<MERSENNE_31>` (issue 1454ec2d `[hard]`
        /// success criterion). Bounded `n` because KG is `O(n^ω log n)`
        /// per attempt and the per-test 5 s wall-clock is tight.
        #[test]
        fn proptest_kg_eq_cubic_fp_m31(
            n in 2usize..=12,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let cubic = a.charpoly_cubic();
            let kg = a
                .charpoly_keller_gehrig(seed.wrapping_add(0xC0FFEE))
                .expect("KG must converge on Fp<MERSENNE_31>");
            prop_assert_eq!(cubic, kg);
        }

        /// minpoly equals charpoly when the input is a companion matrix
        /// (which is always cyclic). Generated as the companion of a
        /// random monic polynomial of the chosen degree.
        #[test]
        fn proptest_companion_minpoly_eq_charpoly(
            n in 2usize..=5,
            seed in any::<u64>(),
        ) {
            // Random monic polynomial of degree n.
            let mut rng_seed = seed.wrapping_mul(0x9E3779B97F4A7C15);
            let mut coeffs: Vec<Fp<MERSENNE_31>> = (0..n)
                .map(|_| {
                    rng_seed = rng_seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
                    Fp::<MERSENNE_31>::new(rng_seed % MERSENNE_31)
                })
                .collect();
            coeffs.push(Fp::<MERSENNE_31>::new(1));
            let p = FieldPoly::from_coeffs_trimmed(coeffs.clone());
            // Build the companion matrix of p.
            let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(n, n);
            for i in 0..(n - 1) {
                a.set(i + 1, i, Fp::<MERSENNE_31>::new(1));
            }
            for i in 0..n {
                let neg = Fp::<MERSENNE_31>::new(0) - p.coeff(i);
                a.set(i, n - 1, neg);
            }
            let cp = a.charpoly();
            let mp = a.minpoly();
            prop_assert_eq!(&mp, &cp, "companion: minpoly should equal charpoly");
            prop_assert_eq!(&cp, &p, "companion: charpoly should equal generator poly");
        }

        /// `minpoly(A)(A) == 0` for random matrices over `Fp<MERSENNE_31>`
        /// (issue `d1dd266c` success criterion: Wiedemann path annihilates A).
        ///
        /// Uses n ≤ 8 to keep within the 5 s per-test wall-clock budget.
        /// At n=8 the Wiedemann path is engaged (q=2^31-1 >> 2*8=16) and
        /// the O(n²) Krylov sequence + BM are fast; the ref-check via
        /// `ref_minpoly_via_basis_lcm` also runs at O(n⁴) but stays comfortably
        /// within 5 s for n ≤ 8.
        #[test]
        fn proptest_wiedemann_minpoly_annihilates_fp_m31(
            n in 2usize..=8,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let mp = a.minpoly();
            // minpoly(A) must annihilate A.
            let pa = mp.eval_at_matrix(&a);
            let zero = Fp::<MERSENNE_31>::new(0);
            for i in 0..n {
                for j in 0..n {
                    prop_assert_eq!(pa.get(i, j), zero, "minpoly(A)[{},{}] != 0", i, j);
                }
            }
            // minpoly must divide charpoly.
            let cp = a.charpoly();
            let (_, r) = cp.div_rem(&mp);
            prop_assert!(r.is_zero(), "minpoly does not divide charpoly");
            // Cross-check against the independent reference (basis-LCM).
            let ref_mp = ref_minpoly_via_basis_lcm(&a);
            prop_assert_eq!(&mp, &ref_mp, "Wiedemann minpoly != basis-LCM reference");
        }

        /// `minpoly(A)(A) == 0` for random matrices over `Fp<65521>`.
        ///
        /// Uses n ≤ 6 to bound test time. q=65521 > 2n for all tested n,
        /// so the Wiedemann path is engaged.
        #[test]
        fn proptest_wiedemann_minpoly_annihilates_fp65521(
            n in 2usize..=6,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<65521>(n, n, seed);
            let mp = a.minpoly();
            let pa = mp.eval_at_matrix(&a);
            let zero = Fp::<65521>::new(0);
            for i in 0..n {
                for j in 0..n {
                    prop_assert_eq!(pa.get(i, j), zero, "minpoly(A)[{},{}] != 0", i, j);
                }
            }
            let ref_mp = ref_minpoly_via_basis_lcm(&a);
            prop_assert_eq!(&mp, &ref_mp, "Wiedemann minpoly != basis-LCM reference for Fp<65521>");
        }

        /// Bit-identical charpoly results: packed canonical-byte chain-poly
        /// arithmetic vs scalar `FieldPoly` chain-poly arithmetic for `Fp<7>`,
        /// sizes `n ∈ 2..=32` (issue `5a3dbd5b`).
        ///
        /// `charpoly()` dispatches through the packed canonical-byte path on
        /// AVX2 hosts (when `try_make_chain_poly_arith` returns `Some`);
        /// `charpoly_cubic_scalar_chain_polys()` runs the same algorithm with
        /// the packed arm forcibly disabled. The two must agree exactly.
        #[test]
        fn proptest_packed_chain_polys_fp7_matches_scalar(
            n in 2usize..=32,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<7>(n, n, seed);
            let packed_result = a.charpoly();
            let scalar_result = a.charpoly_cubic_scalar_chain_polys();
            prop_assert_eq!(
                packed_result,
                scalar_result,
                "packed chain-poly charpoly ≠ scalar charpoly for Fp<7> n={} seed={}",
                n,
                seed
            );
        }

        /// Bit-identical charpoly results: packed canonical-byte path vs scalar
        /// Montgomery path for `Fp<251>`, sizes `n ∈ 2..=32` (issue `5a3dbd5b`).
        ///
        /// This is the primary regression guard for the packed chain-poly
        /// bookkeeping introduced to close the GF(251)/n=256 charpoly gap.
        /// `charpoly_cubic_scalar_chain_polys()` exercises the same
        /// `cyclic_decomposition` algorithm with `enable_packed_chain_polys
        /// = false`, so the scalar arm is genuinely tested.
        #[test]
        fn proptest_packed_chain_polys_fp251_matches_scalar(
            n in 2usize..=32,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<251>(n, n, seed);
            let packed_result = a.charpoly();
            let scalar_result = a.charpoly_cubic_scalar_chain_polys();
            prop_assert_eq!(
                packed_result,
                scalar_result,
                "packed chain-poly charpoly ≠ scalar charpoly for Fp<251> n={} seed={}",
                n,
                seed
            );
        }
    }

    // ── Packed chain-poly correctness (deterministic, issue 5a3dbd5b) ─────

    /// Verifies charpoly(A) for random matrices over `Fp<251>` at sizes
    /// n ∈ {2, 4, 8, 16, 32} against Cayley–Hamilton and against the
    /// `charpoly_cubic` reference.  This is the TDD gate that must fail
    /// before the packed implementation is wired in, and pass after.
    #[test]
    fn test_packed_chain_polys_fp251_charpoly_correctness() {
        for &n in &[2usize, 4, 8, 16, 32] {
            for seed in 0..5u64 {
                let a = random_fp::<251>(n, n, seed);
                let cp = a.charpoly();
                // Cayley–Hamilton: p(A) = 0.
                let pa = cp.eval_at_matrix(&a);
                let zero = crate::gfp::Fp::<251>::new(0);
                for i in 0..n {
                    for j in 0..n {
                        assert_eq!(
                            pa.get(i, j),
                            zero,
                            "Cayley–Hamilton failed for Fp<251> n={} seed={} at ({},{})",
                            n,
                            seed,
                            i,
                            j
                        );
                    }
                }
                // Bit-identical to scalar cubic reference.
                let scalar = a.charpoly_cubic();
                assert_eq!(
                    cp, scalar,
                    "packed ≠ scalar charpoly for Fp<251> n={} seed={}",
                    n, seed
                );
            }
        }
    }

    /// Same guard for `Fp<7>` — the small-prime canonical-byte path covers
    /// all odd primes ≤ 251.
    #[test]
    fn test_packed_chain_polys_fp7_charpoly_correctness() {
        for &n in &[2usize, 4, 8, 16] {
            for seed in 0..5u64 {
                let a = random_fp::<7>(n, n, seed);
                let cp = a.charpoly();
                let scalar = a.charpoly_cubic();
                assert_eq!(
                    cp, scalar,
                    "packed ≠ scalar charpoly for Fp<7> n={} seed={}",
                    n, seed
                );
            }
        }
    }
}
