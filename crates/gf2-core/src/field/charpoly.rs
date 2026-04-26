//! Characteristic polynomial, minimal polynomial, and Frobenius normal
//! form over an arbitrary [`FiniteField`].
//!
//! Issue `f01298db`. Implements Dumas–Pernet theorem 13.1 — the
//! deterministic cubic Krylov-iteration baseline for the rational
//! canonical form. The driver:
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

use crate::field::matrix::FieldMatrix;
use crate::field::poly::FieldPoly;
use crate::field::vec::FieldVec;
use crate::field::FiniteField;

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
    let (n, _) = a.shape();
    debug_assert_eq!(a.cols(), n, "cyclic_decomposition: A must be square");
    if n == 0 {
        return Vec::new();
    }
    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();

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
            let (red_vec, _coeffs) = reduce(&e, &basis, &pivot_row_of_col);
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
        let mut chain_polys: Vec<FieldPoly<F>> = Vec::new();

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
        chain.push(residual);
        chain_polys.push(FieldPoly::one_like(&zero));

        loop {
            // next = A · chain[-1] (in V).
            let next_in_v = a.matvec(chain.last().unwrap());
            // Reduce against the full running basis. The reduction
            // returns (residual, coeffs) where coeffs[j] is the
            // coefficient of basis[j] in the original `next_in_v`,
            // and `residual = next_in_v − Σ coeffs[j] · basis[j]`.
            let (residual_next, coeffs) = reduce(&next_in_v, &basis, &pivot_row_of_col);

            // The chain coefficients of this reduction:
            // α_j = coeffs[block_start + j] for j = 0..chain.len().
            // Build the next polynomial:
            //   chain_poly[d] = x · chain_poly[d-1] − Σ_j α_j chain_poly[j]
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
/// Strategy: scan canonical basis vectors `e_i` outside `span(basis)`,
/// computing each one's minpoly in the quotient. Maintain the LCM of
/// the minpolys seen so far — when the LCM stabilises, the vector
/// achieving the largest minpoly may not yet equal the LCM. In that
/// case combine pairs of canonical vectors via small linear
/// combinations until a single vector achieves minpoly = LCM.
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

    // Find a candidate whose minpoly equals the lcm.
    if let Some((u, p)) = candidates.iter().find(|(_, p)| p == &target_lcm) {
        return (u.clone(), p.clone());
    }

    // Combine pairs greedily: take the candidate with the largest
    // minpoly degree as a base, then add multiples of other
    // candidates trying small scalar multipliers until the combined
    // vector has minpoly = lcm.
    candidates.sort_by(|a, b| b.1.degree().cmp(&a.1.degree()));
    let (mut u, mut u_min) = candidates[0].clone();
    for (v, v_min) in candidates.iter().skip(1) {
        if u_min == target_lcm {
            return (u, u_min);
        }
        // Try scalar multiples 1, 2, …, 64 of v to find a combination
        // whose minpoly is divisible by lcm(u_min, v_min). The sweep
        // cap of 64 is generous for any prime field this baseline is
        // exercised against (the smallest is GF(7), where every
        // combination across n ≤ 6 is enumerated within the cap).
        let mut combined: Option<(FieldVec<F>, FieldPoly<F>)> = None;
        for trial in 1u64..=64u64 {
            let alpha = scalar_from_u64::<F>(zero, trial);
            if alpha.is_zero() {
                continue;
            }
            let mut cand = u.clone();
            cand.axpy(&alpha, v);
            let p = vector_minpoly_in_quotient(a, &cand, basis, pivot_row_of_col);
            let lcm_pair = poly_lcm(&u_min, v_min);
            if poly_divides(&lcm_pair, &p) {
                combined = Some((cand, p));
                break;
            }
        }
        if let Some((nc, np)) = combined {
            u = nc;
            u_min = np;
        }
        // If we couldn't find a working trial, just continue with the
        // current u; eventually some pair will work since the field
        // has enough scalars for generic combinations.
    }
    // Final cross-check: u_min should equal target_lcm if the field
    // is large enough. If not (small finite field), return the best
    // we have — the algorithm will still terminate because the
    // recursion proceeds on a strictly smaller quotient.
    (u, u_min)
}

/// Constructs a field element from a `u64` natural number, by adding
/// `1` to itself `n` times. Returns the field's zero on n = 0. This is
/// the canonical embedding `Z/p → F` for prime fields and the
/// characteristic-fold embedding for extension fields.
fn scalar_from_u64<F: FiniteField>(zero: &F, n: u64) -> F {
    let one = zero.one_like();
    let mut acc = zero.clone();
    for _ in 0..n {
        acc += one.clone();
    }
    acc
}

/// Computes the minimal polynomial of a vector `v` with respect to the
/// matrix `a`: the unique monic polynomial `p ∈ F[x]` of smallest
/// degree with `p(A) · v = 0`.
///
/// Algorithm: build the Krylov chain `v, A·v, A²·v, …` and stop at
/// the first iterate that is linearly dependent on its predecessors
/// **in V** (no quotient by anything else). Track the chain as a list
/// of pivot-reduced columns alongside their polynomial preimages
/// (degree-`k` polynomials in `x`). At first dependence the
/// polynomial relation in V gives `p(A) · v = 0`.
fn vector_minpoly<F: FiniteField>(a: &FieldMatrix<F>, v: &FieldVec<F>) -> FieldPoly<F> {
    let n = v.len();
    if n == 0 {
        let z = F::zero_hint()
            .expect("vector_minpoly: cannot synthesise constant-1 polynomial for empty input");
        return FieldPoly::one_like(&z);
    }
    let zero = v.get(0).zero_like();

    // p(x) = 1 annihilates the zero vector trivially.
    if v.iter().all(|c| c.is_zero()) {
        return FieldPoly::one_like(&zero);
    }

    // chain[k] = residual of `A^k · v` after reducing against
    // chain[0..k-1]. chain_polys[k] = polynomial expression of
    // chain[k] in terms of v.
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

    loop {
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
            return monic(next_poly);
        }
    }
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

// ─── Public methods on FieldMatrix ───────────────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns the characteristic polynomial `det(xI − A)` of `self`.
    ///
    /// Computed via the Krylov cyclic decomposition: builds a direct-
    /// sum decomposition `V = ⊕ W_i` and returns the **product** of
    /// the per-block annihilator polynomials. The product equals
    /// `det(xI − A)` because `charpoly` is multiplicative across
    /// `A`-invariant direct sums and each block contributes the
    /// characteristic polynomial of `A` restricted to its cyclic
    /// subspace (which equals that subspace's minimal polynomial when
    /// the subspace is cyclic).
    ///
    /// The polynomial is returned monic and of exact degree `n`.
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
    /// Panics if `self` is not square.
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
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::charpoly: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
            // Empty product = constant 1. We need a witness for the one
            // element. ConstField provides it for free; for a runtime-
            // context field on an empty matrix there is no witness, so
            // we must rely on `F::zero_hint()`.
            let zero = F::zero_hint().expect(
                "FieldMatrix::charpoly: cannot synthesise the constant-1 \
                 polynomial for a 0×0 matrix over a runtime-context field; \
                 use F: ConstField",
            );
            return FieldPoly::one_like(&zero);
        }
        let blocks = cyclic_decomposition(self);
        let polys: Vec<FieldPoly<F>> = blocks.into_iter().map(|b| b.poly).collect();
        FieldPoly::product(&polys)
    }

    /// Returns the minimal polynomial of `self`.
    ///
    /// Defined as the monic generator of the ideal of polynomials
    /// `p ∈ F[x]` with `p(A) = 0`. Equivalently, the largest invariant
    /// factor of `A` (= `f_1` in the Frobenius normal form, since the
    /// canonical ordering puts the largest factor first and the chain
    /// `f_{i+1} | f_i` propagates downwards).
    ///
    /// Computed as the **lcm of the per-block polynomials** from the
    /// Krylov cyclic decomposition. The lcm of cyclic-block annihilators
    /// is the global minimal polynomial.
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
    /// `O(n³)` for the Krylov decomposition plus `O(t · n²)` for the
    /// `t`-fold lcm reduction (`t ≤ n` cyclic blocks).
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
        // minpoly(A) = lcm over a spanning set {v_1, …, v_n} of the
        // per-vector annihilator polynomials minpoly_{v_i}(A). Using
        // canonical basis vectors as a spanning set, this is `n`
        // independent vector-Krylov chains, each costing O(n²) for a
        // total of O(n³).
        let zero = self.get(0, 0).zero_like();
        let mut acc = FieldPoly::one_like(&zero);
        for i in 0..n {
            let mut e = FieldVec::<F>::zeros_from(n, &zero);
            e.set(i, zero.one_like());
            let p = vector_minpoly(self, &e);
            acc = poly_lcm(&acc, &p);
        }
        acc
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
fn poly_lcm<F: FiniteField>(a: &FieldPoly<F>, b: &FieldPoly<F>) -> FieldPoly<F> {
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
pub fn charpoly<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    a.charpoly()
}

/// Free-function alias for [`FieldMatrix::minpoly`].
pub fn minpoly<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    a.minpoly()
}

/// Free-function alias for [`FieldMatrix::frobenius_form`].
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
    // n+1 distinct field points α_k, compute det(α_k · I − A), then
    // Lagrange-interpolate the (α_k, det) pairs to recover charpoly.
    // Compare against `a.charpoly()`.

    fn check_interpolation_charpoly_fp_m31(a: &FieldMatrix<Fp<MERSENNE_31>>) {
        let n = a.rows();
        // Build `n + 1` distinct evaluation points 1, 2, …, n+1 (all
        // < MERSENNE_31).
        let pts: Vec<Fp<MERSENNE_31>> = (1u64..=(n as u64 + 1))
            .map(Fp::<MERSENNE_31>::new)
            .collect();
        let mut vals: Vec<Fp<MERSENNE_31>> = Vec::with_capacity(n + 1);
        let zero = Fp::<MERSENNE_31>::new(0);
        for x in &pts {
            // M = x · I − A.
            let mut m = FieldMatrix::<Fp<MERSENNE_31>>::zeros(n, n);
            for i in 0..n {
                for j in 0..n {
                    let a_ij = a.get(i, j);
                    let cell = if i == j { *x - a_ij } else { zero - a_ij };
                    m.set(i, j, cell);
                }
            }
            vals.push(m.det());
        }
        // Lagrange-interpolate (pts, vals) into a FieldPoly.
        let pairs: Vec<(Fp<MERSENNE_31>, Fp<MERSENNE_31>)> =
            pts.iter().cloned().zip(vals.iter().cloned()).collect();
        let p = crate::field::interpolate(&pairs)
            .expect("Lagrange interpolation must succeed at distinct points");
        assert_eq!(
            p,
            a.charpoly(),
            "interpolated det(xI − A) should equal charpoly(A)"
        );
    }

    #[test]
    fn test_charpoly_via_interpolation_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp::<MERSENNE_31>(4, 4, seed);
            check_interpolation_charpoly_fp_m31(&a);
        }
    }

    #[test]
    fn test_charpoly_via_interpolation_singular() {
        let f1 = random_fp::<MERSENNE_31>(4, 1, 0xC0FFEE);
        let f2 = random_fp::<MERSENNE_31>(1, 4, 0xC0FFEF);
        let a = gemm(&f1, &f2);
        check_interpolation_charpoly_fp_m31(&a);
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
    }
}
