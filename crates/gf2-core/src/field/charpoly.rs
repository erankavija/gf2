//! Characteristic polynomial, minimal polynomial, and Frobenius normal
//! form of a square [`FieldMatrix`] over an arbitrary [`FiniteField`].
//!
//! [`FieldMatrix::charpoly`] dispatches between a deterministic cubic
//! Krylov path and the Las-Vegas Keller–Gehrig path
//! (`@/citation/DumasPernet2012`, theorems 13.1 and 13.4);
//! [`FieldMatrix::minpoly`] dispatches between Wiedemann and
//! cyclic-decomposition paths. Singular inputs are accepted.

use crate::field::matrix::{BasisReducer, ChainPolyArith, FieldMatrix, PackedMatvec};
use crate::field::poly::FieldPoly;
use crate::field::vec::FieldVec;
use crate::field::FiniteField;
use crate::tuning;

/// Matrix–vector products against one matrix, through the field's
/// `try_prepack_matvec` form when it has one, so the matrix is packed
/// once per driver.
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
            // `x` is non-empty: `packed` exists only for `cols > 0`.
            let zero = x.as_slice()[0].zero_like();
            let mut y = FieldVec::<F>::zeros_from(self.rows, &zero);
            packed.matvec(x.as_slice(), y.as_mut_slice());
            return y;
        }
        self.a.matvec(x)
    }
}

/// Conservative default for `charpoly.keller_gehrig_min_dim()`, consumed
/// by [`crate::tuning::CoreTuning::CONSERVATIVE`]: the minimum matrix
/// dimension at which [`FieldMatrix::charpoly`] considers the
/// Keller–Gehrig path. [`charpoly_route`] reads the live value.
///
/// Both `usize` endpoints are admissible and no value is reserved as a
/// sentinel: `0` engages Keller–Gehrig at every dimension and
/// [`usize::MAX`], this constant's value, disables it.
pub const KG_DISPATCH_MIN_N: usize = usize::MAX;

/// The arm of the [`FieldMatrix::charpoly`] size gate at a given matrix
/// dimension.
///
/// [`CharpolyRoute::KellerGehrig`] still passes through the
/// field-cardinality gate and the Las-Vegas retry loop, both of which
/// can fall back to the cubic path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharpolyRoute {
    /// `n` is below the active threshold: the dispatcher returns
    /// [`FieldMatrix::charpoly_cubic`] unconditionally.
    Cubic,
    /// `n` clears the size gate: the dispatcher proceeds to the
    /// field-cardinality gate and, on success, the Las-Vegas
    /// Keller-Gehrig path.
    KellerGehrig,
}

/// Reports the [`FieldMatrix::charpoly`] size-gate arm for a matrix
/// dimension `n`.
///
/// The comparison uses the active `charpoly.keller_gehrig_min_dim()`
/// profile value (conservative default [`KG_DISPATCH_MIN_N`]).
#[must_use]
pub fn charpoly_route(n: usize) -> CharpolyRoute {
    charpoly_route_resolved(tuning::active().charpoly().keller_gehrig_min_dim(), n)
}

fn charpoly_route_resolved(keller_gehrig_min_dim: usize, n: usize) -> CharpolyRoute {
    if n < keller_gehrig_min_dim {
        CharpolyRoute::Cubic
    } else {
        CharpolyRoute::KellerGehrig
    }
}

/// Las-Vegas attempts [`FieldMatrix::charpoly_keller_gehrig`] makes, each
/// with a fresh deterministic seed, before returning `None`.
pub const KG_MAX_RETRIES: usize = 8;

/// One cyclic block of a Krylov decomposition.
struct CyclicBlock<F: FiniteField> {
    /// Annihilator polynomial of the block's generator in
    /// `V / span(previous blocks)`.
    poly: FieldPoly<F>,
}

/// Builds a cyclic decomposition of `V = F^n` under the action of `a`.
///
/// Each block's generator is the first standard basis vector outside the
/// span of the earlier blocks, and its polynomial is the minimal
/// polynomial of that generator in the quotient by those blocks. The
/// degrees sum to `n` and the product of the polynomials is
/// `charpoly(A)`.
///
/// `O(n³)` field operations: each of the `n` Krylov steps costs one
/// `O(n²)` matvec and one `O(n²)` reduction against the running basis.
fn cyclic_decomposition<F: FiniteField>(a: &FieldMatrix<F>) -> Vec<CyclicBlock<F>> {
    cyclic_decomposition_inner(a, true)
}

#[cfg(test)]
fn cyclic_decomposition_scalar_chain_polys<F: FiniteField>(
    a: &FieldMatrix<F>,
) -> Vec<CyclicBlock<F>> {
    cyclic_decomposition_inner(a, false)
}

/// `enable_packed_chain_polys` gates only the chain-polynomial
/// arithmetic; the packed basis reducer and packed matvec keep their
/// default availability.
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
    let driver = MatvecDriver::new(a);
    let mut packed_basis: Option<Box<dyn BasisReducer<F>>> = F::try_make_basis_reducer(n);
    let _ = n;
    let chain_poly_packed_available = enable_packed_chain_polys && F::chain_poly_arith_available();

    // Running basis, one `FieldVec` per column. Column `j` has its pivot
    // at row `pivot_row_of_col[j]`; `col_at_pivot_row` is the inverse map.
    let mut basis: Vec<FieldVec<F>> = Vec::with_capacity(n);
    let mut col_at_pivot_row: Vec<Option<usize>> = vec![None; n];
    let mut pivot_row_of_col: Vec<usize> = Vec::with_capacity(n);

    let mut blocks: Vec<CyclicBlock<F>> = Vec::new();
    let mut next_seed: usize = 0;

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

        // `chain[k]` is the residual of `A^k · u` after reduction against
        // the earlier blocks and `chain[0..k]`, so it differs from
        // `A^k · u`. `chain_poly[k]` tracks the polynomial with
        //
        //     chain[k] ≡ chain_poly[k](A) · u   (mod earlier-block basis);
        //
        // a reduction step that subtracts `α_j · chain[j]` contributes
        // `−α_j · chain_poly[j]`.
        let _ = seed_index; // generator vector not retained: only the polynomial is needed downstream.
        let block_start = basis.len();
        let mut chain: Vec<FieldVec<F>> = Vec::new();
        let mut chain_polys: Vec<FieldPoly<F>> = Vec::new();
        let mut packed_cpa: Option<Box<dyn ChainPolyArith<F>>> = if chain_poly_packed_available {
            F::try_make_chain_poly_arith(n)
        } else {
            None
        };

        // chain[0] ≡ u modulo the earlier basis, so chain_poly[0] = 1.
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
            let next_in_v = driver.matvec(chain.last().unwrap());
            let (residual_next, coeffs) =
                do_reduce(&next_in_v, &basis, &pivot_row_of_col, &packed_basis);

            // With α_j = coeffs[block_start + j]:
            //   chain_poly[d] = x · chain_poly[d-1] − Σ_j α_j chain_poly[j]
            if let Some(cpa) = packed_cpa.as_mut() {
                let d = chain.len();
                let mut buf = cpa.alloc_buf(d);
                cpa.shift_x_last_into(&mut buf);
                for j in 0..d {
                    let alpha = &coeffs[block_start + j];
                    if !alpha.is_zero() {
                        cpa.sub_scaled_into(&mut buf, alpha, j);
                    }
                }

                if residual_next.iter().any(|c| !c.is_zero()) {
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
                let mut next_poly = poly_shift_x(&chain_polys[chain.len() - 1]);
                for j in 0..chain.len() {
                    let alpha = coeffs[block_start + j].clone();
                    if !alpha.is_zero() {
                        next_poly = &next_poly - &chain_polys[j].mul_scalar(&alpha);
                    }
                }

                if residual_next.iter().any(|c| !c.is_zero()) {
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
/// Scans the standard basis vectors outside `span(basis)`; the lcm of
/// their quotient minpolys is the target. When no single vector attains
/// it, candidates `(u, p)` and `(v, q)` are merged into
/// `α(A) · u + β(A) · v` with `α`, `β` derived from a [`coprime_split`]
/// of `(p, q)`, which works in any characteristic.
fn find_max_minpoly_generator<F: FiniteField>(
    a: &FieldMatrix<F>,
    basis: &[FieldVec<F>],
    pivot_row_of_col: &[usize],
    zero: &F,
) -> (FieldVec<F>, FieldPoly<F>) {
    let n = a.rows();
    debug_assert!(basis.len() < n);
    let one = zero.one_like();

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

/// Returns `p(A) · v` via Horner: `((c_d · A + c_{d-1}) · A + … + c_0) · v`,
/// in `O(deg(p) · n²)` field operations.
pub(crate) fn poly_action_on_vector<F: FiniteField>(
    p: &FieldPoly<F>,
    a: &FieldMatrix<F>,
    v: &FieldVec<F>,
) -> FieldVec<F> {
    let driver = MatvecDriver::new(a);
    poly_action_on_vector_via_driver(p, &driver, v)
}

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

/// Computes a split `(a, b)` of `(p, q)` with `a | p`, `b | q` and
/// `a · b = lcm(p, q)`, coprime when the iteration converges.
///
/// Starts from `a = p`, `b = q / gcd(p, q)` and, while
/// `g = gcd(a, b) ≠ 1`, moves the `g`-saturation of one side onto the
/// other, keeping `a | p` and `b | q`. When neither move preserves
/// divisibility it returns the current split;
/// `find_max_minpoly_generator` detects that by recomputing the minpoly.
fn coprime_split<F: FiniteField>(
    p: &FieldPoly<F>,
    q: &FieldPoly<F>,
) -> (FieldPoly<F>, FieldPoly<F>) {
    let g0 = FieldPoly::gcd(p, q);
    if g0.is_zero() || is_constant_one(&g0) {
        return (p.clone(), q.clone());
    }
    let mut a = p.clone();
    let mut b = poly_div_exact(q, &g0);

    // Defensive cap; each iteration strips one g-saturation.
    let max_iters = p.len() + q.len() + 2;
    for _ in 0..max_iters {
        let g = FieldPoly::gcd(&a, &b);
        if is_constant_one(&g) {
            return (a, b);
        }
        let g_a = saturate_with(&a, &g);
        let g_b = saturate_with(&b, &g);
        // Prefer to move the more-saturated side onto `a` (which still
        // divides `p`). If a's saturation already dominates b's, divide
        // b by g_b — the gcd shrinks, a · b shrinks too. To preserve
        // a · b = lcm we then multiply a by g_b/gcd(g_a, g_b). But
        // a · g_b/... must still divide p; verify before committing.
        let (next_a, next_b) = if g_a.degree().unwrap_or(0) >= g_b.degree().unwrap_or(0) {
            let new_b = poly_div_exact(&b, &g_b);
            // Re-add the missing g-mass to a so that a · b = lcm(p, q).
            // Required factor: g_b / gcd(g_a, g_b).
            let inter = FieldPoly::gcd(&g_a, &g_b);
            let extra = poly_div_exact(&g_b, &inter);
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

/// Returns the monic associate of `numer / denom`; `denom` must divide
/// `numer` (debug-asserted).
fn poly_div_exact<F: FiniteField>(numer: &FieldPoly<F>, denom: &FieldPoly<F>) -> FieldPoly<F> {
    let (q, r) = numer.div_rem(denom);
    debug_assert!(
        r.is_zero(),
        "poly_div_exact: caller-supplied denom must divide numer exactly"
    );
    monic(q)
}

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
/// The basis is not triangular: column `j` carries a single pivot row
/// `pivot_row_of_col[j]` that may be anywhere in `[0, n)`.
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
        let pivot_inv = pivot_val
            .inv()
            .expect("reduce: pivot value must be non-zero by construction");
        let factor = v_at_r * pivot_inv;
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

fn charpoly_dispatch<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    let (m, n) = a.shape();
    assert_eq!(
        m, n,
        "FieldMatrix::charpoly: input must be square (got {}×{})",
        m, n
    );

    let keller_gehrig_min_dim = tuning::active().charpoly().keller_gehrig_min_dim();
    if charpoly_route_resolved(keller_gehrig_min_dim, n) == CharpolyRoute::Cubic {
        return a.charpoly_cubic();
    }
    let log_q = match F::cardinality_log2_hint() {
        Some(v) => v,
        None => return a.charpoly_cubic(),
    };
    // Gate `q > 2n²`. A hint above 127 bits passes: `2n² ≤ 2^127` for
    // any `usize`-indexed matrix.
    if log_q <= 127 {
        let q = 1u128 << log_q;
        let two_n_sq = 2u128 * (n as u128) * (n as u128);
        if q <= two_n_sq {
            return a.charpoly_cubic();
        }
    }
    if let Some(p) = keller_gehrig_charpoly(a, KG_DEFAULT_SEED) {
        return p;
    }
    a.charpoly_cubic()
}

/// Seed [`charpoly_dispatch`] passes to the Keller–Gehrig path.
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
        // Empty product; `None` when the field has no static zero.
        let zero = F::zero_hint()?;
        return Some(FieldPoly::one_like(&zero));
    }
    if n == 1 {
        // charpoly = x − A[0,0].
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

/// SplitMix64 step. Keeps the seeded vector generators independent of
/// the optional `rand` feature.
#[inline]
pub(crate) fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// One Keller–Gehrig attempt with the given seed.
///
/// Returns `None` when the seeded vector `v` is not cyclic for `A`, and
/// `Some(p)` only after verifying `p.eval_at_matrix(A) == 0`.
fn keller_gehrig_attempt<F: FiniteField>(a: &FieldMatrix<F>, seed: u64) -> Option<FieldPoly<F>> {
    let n = a.rows();
    debug_assert!(n >= 2, "n ∈ {{0, 1}} should have been short-circuited");
    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();

    // Step 1: seeded `v`. Each entry is zero with probability 1/2 and
    // otherwise a sum of at most 63 ones, which every `FiniteField`
    // supports; the distribution is not uniform over `F`.
    let mut state = seed;
    let mut v = FieldVec::<F>::zeros_from(n, &zero);
    for i in 0..n {
        let count = (splitmix64(&mut state) & 0x3F) as u32;
        let mut acc = zero.clone();
        for _ in 0..count {
            acc += &one;
        }
        if (splitmix64(&mut state) & 1) == 1 {
            v.set(i, acc);
        }
    }
    // Reject the all-zero residue (degenerate; never cyclic).
    if v.iter().all(|c| c.is_zero()) {
        v.set(0, one.clone());
    }

    // Step 2: build K = [v | A·v | … | A^{n-1}·v] by repeated squaring.
    // Loop invariant: `cols` columns of `K` are populated and
    // `b == A^cols`.
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

    // Step 4: solve K · y = w. `None` ⇒ K is rank-deficient ⇒ `v` is
    // not cyclic.
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

    let pa = cp.eval_at_matrix(a);
    for i in 0..n {
        for j in 0..n {
            if !pa.get(i, j).is_zero() {
                return None;
            }
        }
    }
    // A candidate that is not monic of degree `n` is a failed attempt.
    if cp.degree() != Some(n) {
        return None;
    }
    if !cp.leading_coeff().map(|c| c.is_one()).unwrap_or(false) {
        return None;
    }
    Some(cp)
}

/// Las-Vegas attempts of the scalar Wiedemann path before
/// [`minpoly_dispatch`] falls back to [`cyclic_lcm_minpoly`].
const WIEDEMANN_MAX_RETRIES: usize = 8;

/// Default seed for the Wiedemann minpoly dispatch.
const WIEDEMANN_DEFAULT_SEED: u64 = 0x5769_6564_656D_616E; // ASCII "Wiedeman"

/// Berlekamp–Massey (`@/citation/Massey1969`) over an arbitrary
/// `FiniteField`.
///
/// Returns the monic annihilator `L(x) = x^d − c_1 x^{d-1} − … − c_d` of
/// the shortest linear recurrence satisfied by `s`:
///
/// ```text
/// s[k] = c_1 s[k-1] + c_2 s[k-2] + … + c_d s[k-d]   for k ≥ d
/// ```
///
/// `L` is the reversal of the connection polynomial
/// `C(x) = 1 − c_1 x − … − c_d x^d`.
///
/// # Panics
///
/// Panics on an empty sequence over a field without a static zero.
///
/// # Complexity
///
/// `O(len²)` field operations for a sequence of length `len`.
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

    // C(x) = 1 + C[1]*x + ... + C[L]*x^L in ascending order; C[0] = 1.
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
        // Discrepancy d = sum_{j=0}^{L} C[j] * s[n-j], with s[k] = 0 for
        // k < 0.
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
            b_poly = c;
            b_scalar = d;
            ell = ell_new;
            m = 1;
        } else {
            m += 1;
        }
        c = t;
    }

    // Reverse the connection polynomial into the monic annihilator
    // A(x) = x^L * C(1/x): A[k] = C[L - k].
    let deg = ell;
    let mut annihilator: Vec<F> = Vec::with_capacity(deg + 1);
    for k in 0..=deg {
        let idx = if k <= deg && (deg - k) < c.len() {
            c[deg - k].clone()
        } else {
            zero.clone()
        };
        annihilator.push(idx);
    }
    let p = FieldPoly::from_coeffs_trimmed(annihilator);
    monic(p)
}

/// One scalar Wiedemann minpoly attempt with the given seed.
///
/// Runs Berlekamp–Massey on `s_k = ⟨v, A^k · u⟩`, `k = 0..=2n`, for
/// seeded `u`, `v`. The candidate is accepted only if it satisfies the
/// recurrence on a second projection `⟨v', A^k · u'⟩` and annihilates
/// the probe vectors: every `e_i` for
/// `n ≤ WIEDEMANN_DETERMINISTIC_VERIFY_N`, otherwise `e_{n-1}` and one
/// seeded vector. Returns `None` on any failure.
///
/// Above that threshold the `4n + 2` matvecs and the two probes cost
/// `O(n³)` field operations.
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
        if v.iter().all(|c| c.is_zero()) {
            v.set(0, one.clone());
        }
        v
    };
    let u = gen_vec(&mut state);
    let v = gen_vec(&mut state);

    let driver = MatvecDriver::new(a);

    let seq_len = 2 * n + 1; // +1 ensures BM has enough terms for degree-n recurrence
    let mut seq: Vec<F> = Vec::with_capacity(seq_len);
    let mut cur = u.clone();
    for _ in 0..seq_len {
        let sk = v.dot_product(&cur);
        seq.push(sk);
        cur = driver.matvec(&cur);
    }

    let candidate = berlekamp_massey(&seq);

    // The minpoly has degree ≤ n.
    if candidate.degree().map(|d| d > n).unwrap_or(true) {
        return None;
    }

    // Recurrence check on a second projection: the true minpoly satisfies
    // `sum_{j=0}^{d} m_j s'_{k+j} == 0` for every window `k`.
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

    // Annihilation probes. Below the threshold every `e_i` is checked,
    // which rejects every candidate with `p(A) ≠ 0`.
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
        // Probabilistic: `e_(n-1)` and one seeded vector only.
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

/// Largest `n` at which the Wiedemann verification checks every standard
/// basis vector; above it, `e_{n-1}` and one seeded vector.
const WIEDEMANN_DETERMINISTIC_VERIFY_N: usize = 32;

/// Returns whether `p(A) = 0`, deterministically.
///
/// `deg(p) == n` is accepted without evaluation, which is sound only for
/// `p` constructed as a divisor of `minpoly(A)`: then `p = charpoly(A)`.
/// Otherwise `p(A) · e_i` is evaluated for every standard basis vector,
/// `O(deg(p) · n³)` field operations in the worst case.
///
/// `_seed` is unused.
pub(crate) fn poly_annihilates_a_lasvegas<F: FiniteField>(
    p: &FieldPoly<F>,
    a: &FieldMatrix<F>,
    _seed: u64,
) -> bool {
    let n = a.rows();
    if n == 0 {
        return true;
    }

    if let Some(d) = p.degree() {
        if d == n {
            return true;
        }
    }

    let zero: F = a.get(0, 0).zero_like();
    let one: F = zero.one_like();

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

/// Multi-seed scalar Wiedemann minpoly path, valid in any finite field.
///
/// Accumulates the lcm of the Berlekamp–Massey outputs for
/// `s_k = ⟨v, A^k u_i⟩` over the seeds `u_i` (`e_0`, `e_{n-1}`,
/// `e_{n/2}`, then seeded vectors), with a fresh seeded `v` per seed.
/// `O(seeds · n³)` field operations plus the
/// [`poly_annihilates_a_lasvegas`] checks.
///
/// Returns `Some(p)` once the lcm passes that check, `None` if it never
/// does within `MAX_SEEDS` seeds.
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

    // The canonical extremes cover upper- and lower-Jordan inputs.
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
        // Below degree `n` the check is a full basis sweep, so it runs
        // on every fourth seed only.
        if attempt >= 3
            && attempt % 4 == 3
            && poly_annihilates_a_lasvegas(&lcm_so_far, a, seed_base.wrapping_add(0xA2))
        {
            return Some(lcm_so_far);
        }
    }
    if poly_annihilates_a_lasvegas(&lcm_so_far, a, seed_base.wrapping_add(0xA3)) {
        return Some(lcm_so_far);
    }
    None
}

/// Returns the minimal polynomial of `A`, trying in order: multi-seed
/// Wiedemann; the lcm of the [`cyclic_decomposition`] block polynomials
/// of `A`; the same for `Aᵀ`; multi-seed Wiedemann on a second seed
/// stream. Each candidate is accepted only by
/// [`poly_annihilates_a_lasvegas`].
///
/// # Panics
///
/// Panics if every arm fails, and on a `0×0` matrix over a field without
/// a static zero.
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

    if let Some(p) = multi_seed_wiedemann_minpoly(a, CYCLIC_LCM_VERIFY_SEED) {
        return p;
    }

    let blocks_a = cyclic_decomposition(a);
    let mut p_a = FieldPoly::one_like(&zero);
    for blk in &blocks_a {
        p_a = poly_lcm(&p_a, &blk.poly);
    }
    if poly_annihilates_a_lasvegas(&p_a, a, CYCLIC_LCM_VERIFY_SEED.wrapping_add(0xB1)) {
        return p_a;
    }

    // On Aᵀ an upper-triangular Jordan input becomes lower-triangular,
    // where `e_0` generates the full Krylov chain.
    let at = a.transpose();
    let blocks_at = cyclic_decomposition(&at);
    let mut p_at = FieldPoly::one_like(&zero);
    for blk in &blocks_at {
        p_at = poly_lcm(&p_at, &blk.poly);
    }
    if poly_annihilates_a_lasvegas(&p_at, a, CYCLIC_LCM_VERIFY_SEED.wrapping_add(0xB2)) {
        return p_at;
    }

    if let Some(p) = multi_seed_wiedemann_minpoly(a, CYCLIC_LCM_VERIFY_SEED.wrapping_add(0xC1)) {
        return p;
    }
    panic!(
        "cyclic_lcm_minpoly: all production dispatch arms exhausted; \
         cyclic_decomposition + multi-seed Wiedemann both failed to \
         produce a verified annihilator for an n={n} matrix. This \
         indicates an internal invariant violation in the Wiedemann \
         convergence proof or in the verifier; report with the input \
         matrix to reproduce."
    );
}

/// Base seed of the [`cyclic_lcm_minpoly`] arms; call sites add distinct
/// offsets.
const CYCLIC_LCM_VERIFY_SEED: u64 = 0xCAFEF00DD15EA5E5;

/// Dispatch behind [`FieldMatrix::minpoly`]: scalar Wiedemann with
/// [`WIEDEMANN_MAX_RETRIES`] attempts when `2^h > n` for the cardinality
/// hint `h`; the field's extension-field Wiedemann hook when
/// `2^h ≤ n`; then [`cyclic_lcm_minpoly`]. The basis arguments are
/// unused.
fn minpoly_dispatch<F: FiniteField>(
    a: &FieldMatrix<F>,
    _basis: &[FieldVec<F>],
    _pivot_row_of_col: &[usize],
    _zero: &F,
) -> FieldPoly<F> {
    let n = a.rows();

    if n >= 2 {
        if let Some(log_q) = F::cardinality_log2_hint() {
            // `q ≥ 2^log_q > n` gives per-attempt success probability
            // ≥ 1 − n/q > 0.
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
                if let Some(m) = F::try_extension_wiedemann_minpoly(a) {
                    return m;
                }
            }
        }
    }

    cyclic_lcm_minpoly(a)
}

impl<F: FiniteField> FieldMatrix<F> {
    /// Returns the characteristic polynomial `det(xI − A)` of `self`:
    /// monic of degree `n`, the constant `1` for the `0×0` matrix.
    ///
    /// Runs [`charpoly_keller_gehrig`](Self::charpoly_keller_gehrig) when
    /// [`charpoly_route`] reports [`CharpolyRoute::KellerGehrig`] and the
    /// field's [`FiniteField::cardinality_log2_hint`] `h` satisfies
    /// `2^h > 2n²`. Otherwise, and when that path returns `None`, runs
    /// [`charpoly_cubic`](Self::charpoly_cubic).
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square. Panics on a `0×0` runtime-context
    /// matrix (no witness for the constant-1 polynomial).
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations on the cubic path; see
    /// [`charpoly_keller_gehrig`](Self::charpoly_keller_gehrig) for the
    /// other.
    pub fn charpoly(&self) -> FieldPoly<F> {
        charpoly_dispatch(self)
    }

    /// Deterministic cubic characteristic polynomial
    /// (`@/citation/DumasPernet2012`, theorem 13.1): the product of the
    /// per-block annihilator polynomials of a Krylov cyclic
    /// decomposition. Monic of degree `n`, the constant `1` for the `0×0`
    /// matrix.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square. Panics on a `0×0` runtime-context
    /// matrix (no witness for the constant-1 polynomial).
    ///
    /// # Complexity
    ///
    /// `O(n³)` field operations.
    pub fn charpoly_cubic(&self) -> FieldPoly<F> {
        let (m, n) = self.shape();
        assert_eq!(
            m, n,
            "FieldMatrix::charpoly_cubic: input must be square (got {}×{})",
            m, n
        );
        if n == 0 {
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

    /// [`Self::charpoly_cubic`] with the packed chain-polynomial
    /// arithmetic disabled.
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

    /// Las-Vegas characteristic polynomial by Keller–Gehrig repeated
    /// squaring (`@/citation/DumasPernet2012`, theorem 13.4).
    ///
    /// Each attempt draws a vector `v` from `seed`, builds the Krylov
    /// matrix `K = [v | A·v | … | A^{n-1}·v]` with `⌈log₂ n⌉` doublings
    /// `K ← [K | B · K]`, `B ← B²`, solves `K · y = A^n · v`, and accepts
    /// `x^n − y_{n − 1} x^{n − 1} − … − y_0` only if it evaluates to the
    /// zero matrix at `A`. Returns `None` when [`KG_MAX_RETRIES`]
    /// attempts all fail: `K` is singular whenever `v` is not cyclic for
    /// `A`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// Per attempt: `⌈log₂ n⌉` `gemm` calls of dimension `n`, one `O(n³)`
    /// [`solve`](Self::solve) and one [`FieldPoly::eval_at_matrix`].
    pub fn charpoly_keller_gehrig(&self, seed: u64) -> Option<FieldPoly<F>> {
        keller_gehrig_charpoly(self, seed)
    }

    /// Returns the minimal polynomial of `self`: the monic generator of
    /// the ideal of polynomials `p ∈ F[x]` with `p(A) = 0`, the constant
    /// `1` for `n == 0`.
    ///
    /// With `h` the field's [`FiniteField::cardinality_log2_hint`], the
    /// arms are tried in order:
    ///
    /// 1. `2^h > n`: scalar Wiedemann. Berlekamp–Massey on
    ///    `s_k = ⟨v, A^k · u⟩` for seeded `u, v ∈ F^n`; a candidate is
    ///    accepted after a recurrence check on a second projection and
    ///    annihilation probes, which cover every standard basis vector
    ///    only below a fixed small dimension.
    /// 2. `2^h ≤ n`: the field's extension-field Wiedemann hook, when it
    ///    has one.
    /// 3. Multi-seed Wiedemann and the Krylov cyclic decomposition; a
    ///    candidate of degree below `n` is accepted only if it annihilates
    ///    every standard basis vector.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square. Panics on a `0×0` runtime-context
    /// matrix (no witness for the constant-1 polynomial).
    ///
    /// # Complexity
    ///
    /// Arm 1: `O(n³)` field operations per attempt. Arm 3: `O(n³)`, plus
    /// `O(d · n³)` for each verification of a candidate of degree
    /// `d < n`.
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
        // `minpoly_dispatch` ignores the basis arguments.
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
    /// On `n == 0`, returns the pair of empty `0×0` matrices.
    ///
    /// Each `f_i` is the minimal polynomial of `A` on the quotient by the
    /// earlier Krylov chains, attained by a generator `g_i`; the columns
    /// of `P` are the chains `g_i, A·g_i, …, A^{deg f_i − 1}·g_i`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not square.
    ///
    /// # Complexity
    ///
    /// `O(n⁴)` field operations in matrix–vector products and basis
    /// reductions (up to `n` quotient minimal polynomials per invariant
    /// factor), plus polynomial gcd and lcm arithmetic.
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

        // Peel off invariant factors: find `u` attaining the minpoly of
        // `A` on V / W (W = previous chains), then append its chain to W.
        // The successive minpolys satisfy `f_{i+1} | f_i` because
        // V / W_i is a quotient of V / W_{i-1}.

        let mut chains: Vec<(FieldPoly<F>, Vec<FieldVec<F>>)> = Vec::new();
        // W = union of all chain vectors so far (for quotient reductions).
        let mut basis: Vec<FieldVec<F>> = Vec::new();
        let mut col_at_pivot_row: Vec<Option<usize>> = vec![None; n];
        let mut pivot_row_of_col: Vec<usize> = Vec::new();

        while basis.len() < n {
            let (gen, gen_minpoly) =
                find_max_minpoly_generator(self, &basis, &pivot_row_of_col, &zero);
            let d = gen_minpoly
                .degree()
                .expect("max minpoly must be non-zero on a non-empty quotient");
            let block_start = basis.len();
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
            // `P` takes the unreduced chain {gen, A·gen, …, A^{d-1}·gen}.
            let mut true_chain: Vec<FieldVec<F>> = Vec::with_capacity(d);
            true_chain.push(gen);
            for _ in 1..d {
                let next = self.matvec(true_chain.last().unwrap());
                true_chain.push(next);
            }
            chains.push((gen_minpoly, true_chain));
            let _ = block_start; // silence unused-variable warning when assertions are off.
        }

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

/// Returns the monic `lcm(a, b) = a · b / gcd(a, b)`.
pub(crate) fn poly_lcm<F: FiniteField>(a: &FieldPoly<F>, b: &FieldPoly<F>) -> FieldPoly<F> {
    FieldPoly::lcm(a, b)
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

fn poly_divides<F: FiniteField>(divisor: &FieldPoly<F>, dividend: &FieldPoly<F>) -> bool {
    if divisor.is_zero() {
        return dividend.is_zero();
    }
    let (_, r) = dividend.div_rem(divisor);
    r.is_zero()
}

/// Returns the characteristic polynomial `det(xI − A)` of `a`; see
/// [`FieldMatrix::charpoly`] for panics and complexity.
pub fn charpoly<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    a.charpoly()
}

/// Returns the minimal polynomial of `a`; see [`FieldMatrix::minpoly`]
/// for panics and complexity.
pub fn minpoly<F: FiniteField>(a: &FieldMatrix<F>) -> FieldPoly<F> {
    a.minpoly()
}

/// Returns the Frobenius normal form `(P, F)` of `a`; see
/// [`FieldMatrix::frobenius_form`] for panics and complexity.
pub fn frobenius_form<F: FiniteField>(a: &FieldMatrix<F>) -> (FieldMatrix<F>, FieldMatrix<F>) {
    a.frobenius_form()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::gemm;
    use crate::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use proptest::prelude::*;

    const MERSENNE_31: u64 = 2_147_483_647;

    /// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
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
        let m_pa = mp.eval_at_matrix(a);
        for i in 0..n {
            for j in 0..n {
                assert_eq!(m_pa.get(i, j), zero, "minpoly(A) at ({},{})", i, j);
            }
        }
        let ref_mp = ref_minpoly_via_basis_lcm(a);
        assert_eq!(
            mp, ref_mp,
            "minpoly mismatch vs canonical-basis-LCM reference"
        );
    }

    /// Independent minpoly reference: the lcm of the annihilators of the
    /// Krylov chains of every standard basis vector.
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
        let pinv = p.inv().expect("Frobenius P must be invertible");
        let ap = gemm(a, &p);
        let pinv_ap = gemm(&pinv, &ap);
        assert_eq!(pinv_ap, fm, "P⁻¹ A P != F");

        // Each companion block's last column is (−f(0), …, −f(d − 1))
        // for its factor f.
        let n = a.rows();
        let mut polys: Vec<FieldPoly<F>> = Vec::new();
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut col = 0;
        while col < n {
            // Block size: extend d while F[col + d, col + d − 1] = 1.
            let mut d = 1;
            while col + d < n && fm.get(col + d, col + d - 1) == one {
                d += 1;
            }
            let mut coeffs: Vec<F> = (0..d)
                .map(|i| zero.clone() - fm.get(col + i, col + d - 1))
                .collect();
            coeffs.push(one.clone());
            polys.push(FieldPoly::from_coeffs_trimmed(coeffs));
            col += d;
        }
        assert_eq!(col, n, "Frobenius blocks must tile the diagonal");

        for i in 0..polys.len().saturating_sub(1) {
            let (_, r) = polys[i].div_rem(&polys[i + 1]);
            assert!(
                r.is_zero(),
                "Frobenius divisibility violated: f_{} ∤ f_{}",
                i + 1,
                i,
            );
        }

        if let Some(first) = polys.first() {
            assert_eq!(*first, a.minpoly(), "f_1 should equal minpoly(A)");
        }
        let prod = FieldPoly::product(&polys);
        assert_eq!(prod, a.charpoly(), "∏ f_i should equal charpoly(A)");
    }

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

    // minpoly(J_d(λ)) = (x − λ)^d and
    // minpoly(J_a(λ) ⊕ J_b(λ)) = (x − λ)^max(a,b).

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
        let a = jordan_block::<7>(3, 2);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<7>(3, 2);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_block_fp7_nilpotent() {
        let a = jordan_block::<7>(4, 0);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<7>(4, 0);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_block_fp251() {
        let a = jordan_block::<251>(5, 13);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<251>(5, 13);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_direct_sum_fp7() {
        let a = jordan_direct_sum::<7>(3, 2, 0);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<7>(3, 0);
        assert_eq!(mp, expected, "minpoly(J_3 ⊕ J_2) over Fp<7> should be x^3");
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_direct_sum_fp251() {
        let a = jordan_direct_sum::<251>(4, 1, 7);
        let mp = a.minpoly();
        let expected = x_minus_lambda_pow::<251>(4, 7);
        assert_eq!(mp, expected);
        check_cayley_hamilton(&a);
    }

    #[test]
    fn test_minpoly_jordan_two_eigenvalues_fp7() {
        // Coprime blocks J_2(1) and J_3(0): the lcm equals the product.
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

    fn cyclic_lcm_random_check<const P: u64>(n: usize, seeds: &[u64]) {
        for &seed in seeds {
            let a = random_fp::<P>(n, n, seed);
            let mp = a.minpoly();
            let ref_mp = ref_minpoly_via_basis_lcm(&a);
            assert_eq!(mp, ref_mp, "minpoly mismatch on Fp<{P}> n={n} seed={seed}",);
            let m_pa = mp.eval_at_matrix(&a);
            let zero = Fp::<P>::new(0);
            for i in 0..n {
                for j in 0..n {
                    assert_eq!(m_pa.get(i, j), zero);
                }
            }
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
        assert_eq!(cp.coeff(3), Fp::<7>::new(1));
        assert_eq!(cp.coeff(2), Fp::<7>::new(2));
        assert_eq!(cp.coeff(1), Fp::<7>::new(3));
        assert_eq!(cp.coeff(0), Fp::<7>::new(5));
        // For a companion matrix the minpoly equals the charpoly.
        assert_eq!(a.minpoly(), cp);
    }

    #[test]
    fn test_charpoly_singular_matrix() {
        let f1 = random_fp::<MERSENNE_31>(5, 1, 0xAAAA);
        let f2 = random_fp::<MERSENNE_31>(1, 5, 0xBBBB);
        let a = gemm(&f1, &f2);
        // A is rank ≤ 1 ⇒ at least 4 zero eigenvalues ⇒ charpoly has x^4
        // as a factor.
        check_cayley_hamilton(&a);
        let cp = a.charpoly();
        assert_eq!(
            cp.eval(&Fp::<MERSENNE_31>::new(0)),
            Fp::<MERSENNE_31>::new(0)
        );
    }

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

    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    /// Compares `charpoly` against the Lagrange interpolation of
    /// `det(xI − A)` at `n + 1` distinct points drawn by `sample`.
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

    /// `n = 5`: the interpolation needs `n + 1` distinct points of GF(7).
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

    #[test]
    fn test_kg_n_eq_0() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(0, 0);
        let p = a
            .charpoly_keller_gehrig(0xC0FFEE)
            .expect("KG must succeed on n=0");
        assert_eq!(p.degree(), Some(0));
        assert!(p.leading_coeff().unwrap().is_one());
    }

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

    #[test]
    fn test_kg_singular_matrix() {
        let f1 = random_fp::<MERSENNE_31>(6, 1, 0x111);
        let f2 = random_fp::<MERSENNE_31>(1, 6, 0x222);
        let a = gemm(&f1, &f2);
        let cubic = a.charpoly_cubic();
        if let Some(kg) = a.charpoly_keller_gehrig(0x333) {
            assert_eq!(cubic, kg);
        }
        let pa = cubic.eval_at_matrix(&a);
        for i in 0..6 {
            for j in 0..6 {
                assert_eq!(pa.get(i, j), Fp::<MERSENNE_31>::new(0));
            }
        }
    }

    #[test]
    fn test_dispatch_routes_below_threshold() {
        let a = random_fp::<MERSENNE_31>(8, 8, 0x4444);
        assert_eq!(a.charpoly(), a.charpoly_cubic());
    }

    /// GF(2^8): the cardinality gate `q > 2 n²` fails for any `n ≥ 12`.
    #[test]
    fn test_dispatch_routes_gf2m8() {
        let a = random_gf2m8(16, 16, 0x5555);
        assert_eq!(a.charpoly(), a.charpoly_cubic());
    }

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
        use crate::gf2m::Gf2mElement;
        assert!(<Gf2mElement as FiniteField>::cardinality_log2_hint().is_none());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

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

        /// A companion matrix is cyclic, so its minpoly equals its
        /// charpoly.
        #[test]
        fn proptest_companion_minpoly_eq_charpoly(
            n in 2usize..=5,
            seed in any::<u64>(),
        ) {
            let mut rng_seed = seed.wrapping_mul(0x9E3779B97F4A7C15);
            let mut coeffs: Vec<Fp<MERSENNE_31>> = (0..n)
                .map(|_| {
                    rng_seed = rng_seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
                    Fp::<MERSENNE_31>::new(rng_seed % MERSENNE_31)
                })
                .collect();
            coeffs.push(Fp::<MERSENNE_31>::new(1));
            let p = FieldPoly::from_coeffs_trimmed(coeffs.clone());
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

        #[test]
        fn proptest_wiedemann_minpoly_annihilates_fp_m31(
            n in 2usize..=8,
            seed in any::<u64>(),
        ) {
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let mp = a.minpoly();
            let pa = mp.eval_at_matrix(&a);
            let zero = Fp::<MERSENNE_31>::new(0);
            for i in 0..n {
                for j in 0..n {
                    prop_assert_eq!(pa.get(i, j), zero, "minpoly(A)[{},{}] != 0", i, j);
                }
            }
            let cp = a.charpoly();
            let (_, r) = cp.div_rem(&mp);
            prop_assert!(r.is_zero(), "minpoly does not divide charpoly");
            let ref_mp = ref_minpoly_via_basis_lcm(&a);
            prop_assert_eq!(&mp, &ref_mp, "Wiedemann minpoly != basis-LCM reference");
        }

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

    #[test]
    fn test_packed_chain_polys_fp251_charpoly_correctness() {
        for &n in &[2usize, 4, 8, 16, 32] {
            for seed in 0..5u64 {
                let a = random_fp::<251>(n, n, seed);
                let cp = a.charpoly();
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
                let scalar = a.charpoly_cubic();
                assert_eq!(
                    cp, scalar,
                    "packed ≠ scalar charpoly for Fp<251> n={} seed={}",
                    n, seed
                );
            }
        }
    }

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
