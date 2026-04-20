//! Lagrange polynomial interpolation over any [`FiniteField`].
//!
//! Given `n` distinct evaluation points `x_0, …, x_{n-1}` and corresponding
//! values `y_0, …, y_{n-1}`, these routines compute the unique polynomial of
//! degree at most `n − 1` that satisfies `L(x_i) = y_i` for all `i`.
//!
//! # Two implementations
//!
//! | Function | Algorithm | Complexity (generic) | Complexity (`TwoAdicField`) |
//! |----------|-----------|---------------------:|----------------------------:|
//! | [`interpolate`] | Lagrange barycentric | `O(n²)` field ops | `O(n²)` |
//! | [`interpolate_fast`] | Subproduct-tree | `O(n² log n)` field ops | `O(n log² n)` via [`FieldPoly::batch_evaluate_auto`]¹ |
//!
//! ¹ On `F: TwoAdicField` the middle step
//! (`m_deriv.batch_evaluate(&xs)`) is asymptotically `O(M(n) log k)`
//! when its inputs straddle
//! [`crate::field::poly::SUBPRODUCT_THRESHOLD`] and the per-node
//! reductions route through the Newton-iteration
//! [`FieldPoly::div_rem_auto`] primitive (issue `ae0c7e1f`,
//! `DIV_REM_THRESHOLD = 2048` on `Fp<65537>`). The subproduct-tree
//! integration landed in issue `046f95c1` — see
//! [`crate::field::poly::batch_evaluate_subproduct_auto`] for the
//! public free function that performs the [`TwoAdicField`]-specialised
//! reductions. With the schoolbook-substrate call through the generic
//! [`FieldPoly::batch_evaluate`] the fast Lagrange variant still
//! beats the quadratic one at every measured `n ≥ 4` on `Fp<65537>`
//! (see the benchmark table below).
//!
//! [`interpolate_auto`] is the recommended entry point: it dispatches to
//! [`interpolate_fast`] for `n ≥ INTERPOLATE_THRESHOLD` (currently 16) and to
//! [`interpolate`] below that. [`interpolate`] and [`interpolate_fast`] remain
//! available for callers who want a specific variant regardless of `n`.
//!
//! # Dependency on `batch_inverse`
//!
//! Both routines use [`crate::field::batch_ops::batch_inverse`] (Montgomery's
//! batch-inversion trick) to compute the `1 / Π_{j≠i}(x_i − x_j)` barycentric
//! weights in a single pass: one field inversion plus `3(n − 1)` multiplications
//! rather than `n` independent inversions. This is the canonical motivating
//! application of the Montgomery-trick module
//! ([`crate::field::batch_ops`]).
//!
//! # Interpolation benchmark results
//!
//! Measured on `Fp<65537>` with
//! `cargo bench -p gf2-core --bench field_poly -- --quick interpolate` on the
//! repo's reference Zen 3 host. Each cell is the median total wall-clock time
//! for one call on `n` random distinct points.
//!
//! | `n`  | naive O(n²) | fast O(n² log n) | fast / naive |
//! |-----:|------------:|-----------------:|-------------:|
//! |    4 |      1.60 µs |         1.00 µs |        0.63× |
//! |    8 |      5.57 µs |         2.50 µs |        0.45× |
//! |   16 |     20.17 µs |         6.88 µs |        0.34× |
//! |   32 |     77.20 µs |        20.05 µs |        0.26× |
//! |   64 |    300.0 µs  |        67.49 µs |        0.22× |
//! |  128 |      1.18 ms |       220.7 µs  |        0.19× |
//! |  256 |      4.68 ms |       738.9 µs  |        0.16× |
//! |  512 |     18.49 ms |         2.50 ms |        0.14× |
//! | 1024 |     73.57 ms |         8.61 ms |        0.12× |
//! | 2048 |    288.3 ms  |        30.08 ms |        0.10× |
//!
//! **`fast` wins at every measured `n ≥ 4` on `Fp<65537>`.** The `naive`
//! path issues `n` full-degree `div_rem`s on `M(x)`; the `fast` path does
//! one `from_roots` + one [`FieldPoly::batch_evaluate`] + one
//! `O(n log n)` upward merge. [`FieldPoly::batch_evaluate`] routes to
//! naive Horner below [`crate::field::poly::SUBPRODUCT_THRESHOLD`] =
//! 4096, so for the measured `n ≤ 2048` the middle step is still
//! `O(n²)` in field operations. Even at this schoolbook substrate the
//! merge savings alone push `fast` below `0.63×` of `naive` at
//! `n = 4`. Callers on [`TwoAdicField`] who want the `O(n log² n)`
//! asymptotic at `n ≥ SUBPRODUCT_THRESHOLD` can call
//! [`crate::field::poly::batch_evaluate_subproduct_auto`] directly
//! before the merge pass, or reach for
//! [`FieldPoly::batch_evaluate_auto`] in the
//! [`interpolate_fast`]-style recipe; the fast-division primitive
//! lands from [`DIV_REM_THRESHOLD`] = 2048 upwards.
//!
//! `INTERPOLATE_THRESHOLD = 16` is kept as a conservative safety margin
//! for callers on fields with very expensive Karatsuba (where the
//! merge-pass polynomial multiplications may flip the balance upward).
//! On cheap fields like `Fp<65537>` the threshold could safely drop to
//! 4; the tuning is deliberately conservative. Call [`interpolate`] or
//! [`interpolate_fast`] directly to override, or use
//! [`interpolate_auto`] for the tuned default.
//!
//! Regenerate this table with
//! `cargo bench -p gf2-core --bench field_poly -- --quick interpolate`.

use crate::field::batch_ops::batch_inverse;
use crate::field::poly::build_subproduct_tree;
use crate::field::{FieldPoly, FiniteField};
use std::fmt;

// ---------------------------------------------------------------------
// Threshold + dispatcher
// ---------------------------------------------------------------------

/// Number-of-points threshold at which [`interpolate_auto`] prefers
/// [`interpolate_fast`] over [`interpolate`].
///
/// Tuned from the benchmark table in the module docstring: `fast`
/// already wins the `n = 4` cell on `Fp<65537>` at `0.63×` of the
/// naive wall-clock, but the threshold is deliberately set at `16` as
/// a conservative safety margin for callers on fields with very
/// expensive polynomial multiplication (where the subproduct-tree
/// build and upward merge can flip the balance). Re-verified under
/// issue `046f95c1` after the subproduct-tree
/// [`crate::field::poly::div_rem_auto`] integration landed: the
/// crossover remains at `n = 4` so no retuning was needed. Callers
/// who want a specific variant regardless of `n` should call
/// [`interpolate`] or [`interpolate_fast`] directly.
pub const INTERPOLATE_THRESHOLD: usize = 16;

/// Interpolates through `points` using the threshold-tuned dispatcher.
///
/// Routes to [`interpolate`] for `points.len() < INTERPOLATE_THRESHOLD`
/// (where the quadratic path is at worst on par with the fast path) and to
/// [`interpolate_fast`] otherwise. Retains the same error contract as the
/// two entry points.
///
/// # Arguments
///
/// * `points` — slice of `(x_i, y_i)` pairs. All `x_i` must be distinct.
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly_interpolate::interpolate_auto;
/// use gf2_core::gfp::Fp;
///
/// let p = interpolate_auto(&[
///     (Fp::<7>::new(0), Fp::<7>::new(1)),
///     (Fp::<7>::new(1), Fp::<7>::new(3)),
/// ])
/// .unwrap();
/// assert_eq!(p.eval(&Fp::<7>::new(0)), Fp::<7>::new(1));
/// assert_eq!(p.eval(&Fp::<7>::new(1)), Fp::<7>::new(3));
/// ```
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] if any two `x_i` coincide.
///
/// # Complexity
///
/// Below the threshold: `O(n²)` field operations (via [`interpolate`]).
/// Above the threshold: `O(M(n) log n)` field operations, where `M(n)` is
/// the cost of multiplying two degree-`n` polynomials (via
/// [`interpolate_fast`]).
pub fn interpolate_auto<F: FiniteField>(
    points: &[(F, F)],
) -> Result<FieldPoly<F>, InterpolationError> {
    if points.len() < INTERPOLATE_THRESHOLD {
        interpolate(points)
    } else {
        interpolate_fast(points)
    }
}

// ---------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------

/// Error returned by [`interpolate`] and [`interpolate_fast`] when the
/// input is invalid.
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly_interpolate::{interpolate, InterpolationError};
/// use gf2_core::gfp::Fp;
///
/// let points = vec![
///     (Fp::<7>::new(1), Fp::<7>::new(3)),
///     (Fp::<7>::new(1), Fp::<7>::new(5)), // duplicate x
/// ];
/// match interpolate(&points) {
///     Err(InterpolationError::DuplicatePoint { index_a, index_b }) => {
///         assert_eq!(index_a, 0);
///         assert_eq!(index_b, 1);
///     }
///     Ok(_) => panic!("expected error"),
/// }
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InterpolationError {
    /// Two input points share the same `x`-coordinate, making the interpolation
    /// problem under-determined.
    ///
    /// `index_a < index_b` is guaranteed; `index_a` is the first occurrence and
    /// `index_b` is the second.
    DuplicatePoint {
        /// Index of the first point with this `x`-coordinate.
        index_a: usize,
        /// Index of the second (or later) point sharing `x` with `index_a`.
        index_b: usize,
    },
}

impl fmt::Display for InterpolationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpolationError::DuplicatePoint { index_a, index_b } => write!(
                f,
                "duplicate x-coordinate: points[{index_a}] and points[{index_b}] share the same x"
            ),
        }
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// Computes the formal derivative `f'(x) = Σ i · a_i · x^{i-1}` of `f`.
///
/// In characteristic `p`, all terms where `i` is a multiple of `p` vanish
/// (the characteristic is summed by field addition, not an integer cast).
/// This is the standard definition used in interpolation and error-locator
/// derivative computation.
///
/// # Arguments
///
/// * `f` — the polynomial to differentiate.
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly_interpolate::formal_derivative;
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// // d/dx (x^3 + 2x^2 + 3x + 4) = 3x^2 + 4x + 3  over Fp<7>
/// let f = FieldPoly::new(vec![
///     Fp::<7>::new(4),
///     Fp::<7>::new(3),
///     Fp::<7>::new(2),
///     Fp::<7>::new(1),
/// ]);
/// let df = formal_derivative(&f);
/// assert_eq!(df.degree(), Some(2));
/// // coefficient of x^2 is 3*1 = 3
/// assert_eq!(df.try_coeff(2), Some(&Fp::<7>::new(3)));
/// // coefficient of x^1 is 2*2 = 4
/// assert_eq!(df.try_coeff(1), Some(&Fp::<7>::new(4)));
/// // coefficient of x^0 is 1*3 = 3
/// assert_eq!(df.try_coeff(0), Some(&Fp::<7>::new(3)));
/// ```
///
/// # Complexity
///
/// `O(n)` field additions, where `n = f.degree()`.
pub fn formal_derivative<F: FiniteField>(f: &FieldPoly<F>) -> FieldPoly<F> {
    let n = f.len();
    if n <= 1 {
        // Constant or zero polynomial → derivative is zero.
        return FieldPoly::new(vec![]);
    }

    // The coefficient of x^{i-1} in f' is i · f[i], where i is computed by
    // adding the field's one element i times. This correctly handles any
    // characteristic: in GF(2^m), even i gives zero (as expected).
    let sample = f.try_coeff(1).unwrap();
    let one = sample.one_like();

    let mut deriv_coeffs: Vec<F> = Vec::with_capacity(n - 1);
    // `i_field` tracks the field element corresponding to index i.
    let mut i_field = one.clone(); // i = 1 for the first derivative term
    for i in 1..n {
        let ai = f.try_coeff(i).unwrap();
        deriv_coeffs.push(i_field.clone() * ai.clone());
        i_field += one.clone();
    }

    FieldPoly::new(deriv_coeffs)
}

// ---------------------------------------------------------------------
// Duplicate check
// ---------------------------------------------------------------------

/// Scans for the first pair of duplicate x-coordinates in O(n²).
///
/// Returns `Err(InterpolationError::DuplicatePoint { index_a, index_b })`
/// where `index_a < index_b` if any duplicate is found, `Ok(())` otherwise.
fn check_no_duplicate_x<F: FiniteField>(points: &[(F, F)]) -> Result<(), InterpolationError> {
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            if points[i].0 == points[j].0 {
                return Err(InterpolationError::DuplicatePoint {
                    index_a: i,
                    index_b: j,
                });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------

/// Lagrange interpolation via the **barycentric form** in O(n²).
///
/// Given `n` distinct (x, y) pairs, returns the unique polynomial of degree
/// at most `n − 1` satisfying `L(x_i) = y_i`.
///
/// **Empty input** returns the zero polynomial. **Single point** `(x, y)`
/// returns the constant polynomial `y`.
///
/// The barycentric weights
///
/// ```text
///     w_i = 1 / Π_{j ≠ i} (x_i − x_j)
/// ```
///
/// are computed in bulk using [`crate::field::batch_ops::batch_inverse`]
/// (Montgomery's trick: one field inversion, `3(n − 1)` multiplications).
///
/// # Arguments
///
/// * `points` — slice of `(x_i, y_i)` pairs. All `x_i` must be distinct.
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] with the indices of the
/// first pair sharing an `x`-coordinate.
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly_interpolate::interpolate;
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// // Interpolate through (0, 1), (1, 4), (2, 9) — that's x^2 + 1.
/// // (But over Fp<7>: 1+0=1, 1+1=2... let's use a simpler example.)
/// // p(x) = 3 (constant) through single point (5, 3).
/// let points = vec![(Fp::<7>::new(5), Fp::<7>::new(3))];
/// let p = interpolate(&points).unwrap();
/// assert_eq!(p.eval(&Fp::<7>::new(5)), Fp::<7>::new(3));
/// ```
///
/// ```
/// use gf2_core::field::poly_interpolate::interpolate;
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// // Empty input returns zero polynomial.
/// let points: Vec<(Fp<7>, Fp<7>)> = vec![];
/// let p = interpolate(&points).unwrap();
/// assert!(p.is_zero());
/// ```
///
/// # Panics
///
/// Does not panic on valid input. The one inversion inside `batch_inverse`
/// can panic internally only if all denominator products are zero, which
/// cannot happen when all `x_i` are distinct (guaranteed by the duplicate
/// check before the inverse call).
///
/// # Complexity
///
/// `O(n²)` field multiplications and additions. One call to
/// [`batch_inverse`] (one inversion, `3(n−1)` multiplications). The
/// final summation over `n` degree-`(n−1)` polynomials dominates at `O(n²)`.
pub fn interpolate<F: FiniteField>(points: &[(F, F)]) -> Result<FieldPoly<F>, InterpolationError> {
    let n = points.len();

    if n == 0 {
        return Ok(FieldPoly::new(vec![]));
    }

    // Single point: constant polynomial y.
    if n == 1 {
        return Ok(FieldPoly::constant(points[0].1.clone()));
    }

    // Reject duplicate x-coordinates.
    check_no_duplicate_x(points)?;

    let xs: Vec<F> = points.iter().map(|(x, _)| x.clone()).collect();
    let ys: Vec<F> = points.iter().map(|(_, y)| y.clone()).collect();

    // Compute barycentric weights w[i] = 1 / Π_{j≠i}(x_i − x_j).
    // Step 1: build the n denominators d[i] = Π_{j≠i}(x_i − x_j).
    let mut denoms: Vec<F> = Vec::with_capacity(n);
    for i in 0..n {
        let mut d = xs[0].one_like();
        for j in 0..n {
            if j != i {
                d = d * (xs[i].clone() - xs[j].clone());
            }
        }
        denoms.push(d);
    }

    // Step 2: batch-invert all denominators at once.
    // The denominators are all non-zero (distinct x_i guaranteed above),
    // so `batch_inverse` cannot return None here.
    let weights =
        batch_inverse(&denoms).expect("denominators are non-zero for distinct x-coordinates");

    // Scale weights by y_i: w[i] *= y[i].
    let wy: Vec<F> = weights
        .into_iter()
        .zip(ys.iter())
        .map(|(w, y)| w * y.clone())
        .collect();

    // Build the interpolating polynomial:
    //   L(x) = Σ_i  wy[i] · Π_{j≠i} (x − x_j)
    // Computed as: for each i, build (x - x_0)···(x - x_{i-1})(x - x_{i+1})···(x - x_{n-1})
    // scaled by wy[i], then accumulate.
    //
    // For efficiency we precompute the full product M(x) = Π_i (x - x_i) using
    // from_roots, then divide out (x - x_i) one at a time via div_rem. This is
    // still O(n²) but with a smaller constant because each div_rem is O(n).

    let one = xs[0].one_like();
    let m = FieldPoly::from_roots(&xs);

    let zero_poly = FieldPoly::new(vec![]);
    let mut result: FieldPoly<F> = zero_poly;

    for i in 0..n {
        // (x - x_i) = [-x_i, 1]
        let linear = FieldPoly::new(vec![-xs[i].clone(), one.clone()]);
        // M(x) / (x - x_i) — exact division because x_i is a root of M.
        let (quotient, _rem) = m.div_rem(&linear);
        // Accumulate wy[i] · quotient.
        let mut term = quotient;
        term.scale(&wy[i]);
        result += term;
    }

    Ok(result)
}

/// Lagrange interpolation via the **subproduct-tree** algorithm in O(n log² n).
///
/// Given `n` distinct (x, y) pairs, returns the unique polynomial of degree
/// at most `n − 1` satisfying `L(x_i) = y_i`.
///
/// **Empty input** returns the zero polynomial. **Single point** `(x, y)`
/// returns the constant polynomial `y`.
///
/// # Algorithm
///
/// 1. Build `M(x) = Π_i (x − x_i)` via [`FieldPoly::from_roots`].
/// 2. Compute `M'(x)` (formal derivative) via [`formal_derivative`].
/// 3. Evaluate `M'` at all `x_i` via [`FieldPoly::batch_evaluate`] — the
///    public batch-evaluation API named by the issue contract. Below
///    [`crate::field::poly::SUBPRODUCT_THRESHOLD`] the dispatcher
///    routes through the naive per-point Horner fallback; above it,
///    the schoolbook-[`FieldPoly::div_rem`] subproduct tree takes
///    over. On [`TwoAdicField`] callers who want the Newton-iteration
///    fast-division primitive
///    [`crate::field::poly::batch_evaluate_subproduct_auto`] or
///    [`FieldPoly::batch_evaluate_auto`] reach the `O(M(n) log k)`
///    path above [`crate::field::poly::DIV_REM_THRESHOLD`]; both
///    primitives landed under issues `ae0c7e1f` + `046f95c1`. By the
///    product rule, `M'(x_i) = Π_{j ≠ i} (x_i − x_j)`.
/// 4. Compute barycentric weights `w_i = y_i / M'(x_i)` using
///    [`crate::field::batch_ops::batch_inverse`].
/// 5. Upward merge: starting from leaf vector `[w_0, …, w_{n-1}]`,
///    merge pairs bottom-up on the subproduct tree using the recurrence
///    `L_{left+right}(x) = L_left(x) · M_right(x) + L_right(x) · M_left(x)`
///    where `M_left`, `M_right` are the subproduct-tree nodes. The tree
///    itself is built via [`build_subproduct_tree`][crate::field::poly::build_subproduct_tree]
///    so both `batch_evaluate` and `interpolate_fast` share one
///    construction.
///
/// Steps 1–4 are `O(M(n) log n)` with fast polynomial multiplication;
/// with schoolbook multiplication they remain `O(n² log n)`.
/// Step 5 (the upward merge) is `O(M(n) log n)`.
///
/// # Arguments
///
/// * `points` — slice of `(x_i, y_i)` pairs. All `x_i` must be distinct.
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] with the indices of the
/// first pair sharing an `x`-coordinate.
///
/// # Examples
///
/// ```
/// use gf2_core::field::poly_interpolate::interpolate_fast;
/// use gf2_core::field::FieldPoly;
/// use gf2_core::gfp::Fp;
///
/// let points = vec![
///     (Fp::<7>::new(0), Fp::<7>::new(2)),
///     (Fp::<7>::new(1), Fp::<7>::new(4)),
///     (Fp::<7>::new(2), Fp::<7>::new(0)),
/// ];
/// let p = interpolate_fast(&points).unwrap();
/// for (x, y) in &points {
///     assert_eq!(p.eval(x), *y);
/// }
/// ```
///
/// # Panics
///
/// Does not panic on valid input with distinct x-coordinates.
///
/// # Complexity
///
/// With the schoolbook-backed call through the generic
/// [`FieldPoly::batch_evaluate`] this path costs `O(n² log n)` field
/// operations; the `O(n log² n)` optimum is reached on
/// [`TwoAdicField`] callers at `n ≥ SUBPRODUCT_THRESHOLD` when the
/// middle step routes through
/// [`crate::field::poly::batch_evaluate_subproduct_auto`], which wires
/// the Newton-iteration [`FieldPoly::div_rem_auto`] primitive
/// (`ae0c7e1f`, `DIV_REM_THRESHOLD = 2048` on `Fp<65537>`) into the
/// subproduct-tree reductions (integration landed under issue
/// `046f95c1`). Empirically this path already beats the `O(n²)`
/// [`interpolate`] at every measured `n ≥ 4` on `Fp<65537>` — see the
/// benchmark table in the module docstring.
pub fn interpolate_fast<F: FiniteField>(
    points: &[(F, F)],
) -> Result<FieldPoly<F>, InterpolationError> {
    let n = points.len();

    if n == 0 {
        return Ok(FieldPoly::new(vec![]));
    }

    // Single point: constant polynomial y.
    if n == 1 {
        return Ok(FieldPoly::constant(points[0].1.clone()));
    }

    // Reject duplicate x-coordinates.
    check_no_duplicate_x(points)?;

    let xs: Vec<F> = points.iter().map(|(x, _)| x.clone()).collect();
    let ys: Vec<F> = points.iter().map(|(_, y)| y.clone()).collect();

    // Step 1 & 2: Build M(x) = Π(x − x_i) and its formal derivative M'(x).
    let m_poly = FieldPoly::from_roots(&xs);
    let m_deriv = formal_derivative(&m_poly);

    // Step 3: Evaluate M'(x) at all x_i via the public FieldPoly::batch_evaluate
    // API — this is the evaluation surface named by the bdf95060 story for
    // this role. The public generic dispatcher uses schoolbook
    // FieldPoly::div_rem below SUBPRODUCT_THRESHOLD = 4096 and above it; the
    // TwoAdicField-specialised dispatcher FieldPoly::batch_evaluate_auto
    // (landed under issue 046f95c1) instead routes the tree's reductions
    // through FieldPoly::div_rem_auto (landed under ae0c7e1f), picking up the
    // O(M(n) log k) fast-division asymptotic above DIV_REM_THRESHOLD. This
    // generic function keeps calling the generic batch_evaluate so it stays
    // usable on non-TwoAdicField element types; TwoAdicField callers can
    // reach for batch_evaluate_subproduct_auto directly when they want the
    // O(n log² n) middle step at n ≥ SUBPRODUCT_THRESHOLD.
    // By the product rule: M'(x_i) = Π_{j≠i}(x_i − x_j).
    let m_prime_vals: Vec<F> = m_deriv.batch_evaluate(&xs);

    // Step 4: Compute weights w_i = y_i / M'(x_i).
    // M'(x_i) is non-zero for distinct x_i (it equals the product of all
    // pairwise differences), so batch_inverse cannot return None.
    let m_prime_invs =
        batch_inverse(&m_prime_vals).expect("M'(x_i) is non-zero for distinct evaluation points");

    let weights: Vec<F> = m_prime_invs
        .into_iter()
        .zip(ys.iter())
        .map(|(inv, y)| inv * y.clone())
        .collect();

    // Step 5: Upward merge pass using the same subproduct tree that
    // batch_evaluate uses for reduction, built through the shared
    // [`build_subproduct_tree`] helper (SSOT). Leaf level:
    // `L_i(x) = w_i` (constant polynomial). Merge rule at each level:
    //   `L_{left ∪ right}(x) = L_left(x) · M_right(x) + L_right(x) · M_left(x)`
    // where `M_left` / `M_right` are the subproduct-tree nodes. After
    // the root merge, `L_root = L(x)`, the unique interpolant.
    let tree = build_subproduct_tree(&xs);

    // Initialise the "partial interpolant" at each leaf as the constant w_i.
    let mut cur_interp: Vec<FieldPoly<F>> = weights.into_iter().map(FieldPoly::constant).collect();

    // Bottom-up merge: iterate over every level except the root (the last).
    let tree_len = tree.len();
    for prods in tree.iter().take(tree_len - 1) {
        let next_len = prods.len().div_ceil(2);
        let mut next_interp: Vec<FieldPoly<F>> = Vec::with_capacity(next_len);

        let mut i = 0;
        while i + 1 < cur_interp.len() {
            // Merge pair: L_left · M_right + L_right · M_left
            let m_left = &prods[i];
            let m_right = &prods[i + 1];
            let l_left = &cur_interp[i];
            let l_right = &cur_interp[i + 1];
            let merged = l_left * m_right + l_right * m_left;
            next_interp.push(merged);
            i += 2;
        }
        if i < cur_interp.len() {
            // Odd carry-up: the lonely node has no sibling; carry through.
            next_interp.push(cur_interp[i].clone());
        }

        cur_interp = next_interp;
    }

    debug_assert_eq!(cur_interp.len(), 1);
    Ok(cur_interp.into_iter().next().unwrap())
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::{Gf2mElement, Gf2mField};
    use crate::gfp::Fp;
    use proptest::prelude::*;

    type FP7 = Fp<7>;

    fn fp7(v: u64) -> FP7 {
        FP7::new(v)
    }

    fn gf16_field() -> Gf2mField {
        Gf2mField::new(4, 0b10011)
    }

    // -----------------------------------------------------------------
    // Unit tests: edge cases
    // -----------------------------------------------------------------

    #[test]
    fn test_interpolate_empty_returns_zero() {
        let pts: Vec<(FP7, FP7)> = vec![];
        let p = interpolate(&pts).unwrap();
        assert!(p.is_zero());
    }

    #[test]
    fn test_interpolate_fast_empty_returns_zero() {
        let pts: Vec<(FP7, FP7)> = vec![];
        let p = interpolate_fast(&pts).unwrap();
        assert!(p.is_zero());
    }

    #[test]
    fn test_interpolate_single_point_constant() {
        let pts = vec![(fp7(3), fp7(5))];
        let p = interpolate(&pts).unwrap();
        assert_eq!(p.degree(), Some(0));
        assert_eq!(p.eval(&fp7(3)), fp7(5));
        assert_eq!(p.eval(&fp7(0)), fp7(5)); // constant polynomial
    }

    #[test]
    fn test_interpolate_fast_single_point_constant() {
        let pts = vec![(fp7(3), fp7(5))];
        let p = interpolate_fast(&pts).unwrap();
        assert_eq!(p.degree(), Some(0));
        assert_eq!(p.eval(&fp7(3)), fp7(5));
    }

    #[test]
    fn test_interpolate_single_point_zero_y() {
        // y = 0 → constant zero polynomial
        let pts = vec![(fp7(4), fp7(0))];
        let p = interpolate(&pts).unwrap();
        assert!(p.is_zero());
    }

    #[test]
    fn test_interpolate_duplicate_x_returns_error() {
        let pts = vec![
            (fp7(1), fp7(2)),
            (fp7(3), fp7(4)),
            (fp7(1), fp7(6)), // duplicate x=1
        ];
        match interpolate(&pts) {
            Err(InterpolationError::DuplicatePoint { index_a, index_b }) => {
                assert_eq!(index_a, 0);
                assert_eq!(index_b, 2);
            }
            Ok(_) => panic!("expected DuplicatePoint error"),
        }
    }

    #[test]
    fn test_interpolate_fast_duplicate_x_returns_error() {
        let pts = vec![
            (fp7(2), fp7(1)),
            (fp7(2), fp7(5)), // duplicate x=2
        ];
        match interpolate_fast(&pts) {
            Err(InterpolationError::DuplicatePoint { index_a, index_b }) => {
                assert_eq!(index_a, 0);
                assert_eq!(index_b, 1);
            }
            Ok(_) => panic!("expected DuplicatePoint error"),
        }
    }

    #[test]
    fn test_interpolation_error_display() {
        let err = InterpolationError::DuplicatePoint {
            index_a: 2,
            index_b: 5,
        };
        let s = format!("{err}");
        assert!(s.contains("2"));
        assert!(s.contains("5"));
        assert!(s.contains("duplicate"));
    }

    #[test]
    fn test_interpolate_two_points_linear() {
        // Points: (0, 1), (1, 3). Linear through them: y = 2x + 1.
        let pts = vec![(fp7(0), fp7(1)), (fp7(1), fp7(3))];
        let p = interpolate(&pts).unwrap();
        assert_eq!(p.degree(), Some(1));
        assert_eq!(p.eval(&fp7(0)), fp7(1));
        assert_eq!(p.eval(&fp7(1)), fp7(3));
    }

    #[test]
    fn test_interpolate_fast_two_points_linear() {
        let pts = vec![(fp7(0), fp7(1)), (fp7(1), fp7(3))];
        let p = interpolate_fast(&pts).unwrap();
        assert_eq!(p.degree(), Some(1));
        assert_eq!(p.eval(&fp7(0)), fp7(1));
        assert_eq!(p.eval(&fp7(1)), fp7(3));
    }

    #[test]
    fn test_interpolate_round_trip_fp7_three_points() {
        // Three points in Fp<7>: should give a unique degree-2 polynomial.
        let pts = vec![(fp7(0), fp7(4)), (fp7(1), fp7(2)), (fp7(3), fp7(5))];
        let p = interpolate(&pts).unwrap();
        for (x, y) in &pts {
            assert_eq!(p.eval(x), *y, "eval at {x:?} should be {y:?}");
        }
    }

    #[test]
    fn test_interpolate_fast_round_trip_fp7_three_points() {
        let pts = vec![(fp7(0), fp7(4)), (fp7(1), fp7(2)), (fp7(3), fp7(5))];
        let p = interpolate_fast(&pts).unwrap();
        for (x, y) in &pts {
            assert_eq!(p.eval(x), *y, "eval at {x:?} should be {y:?}");
        }
    }

    #[test]
    fn test_interpolate_agreement_fp7_three_points() {
        let pts = vec![(fp7(0), fp7(4)), (fp7(1), fp7(2)), (fp7(3), fp7(5))];
        let naive = interpolate(&pts).unwrap();
        let fast = interpolate_fast(&pts).unwrap();
        assert_eq!(naive, fast);
    }

    #[test]
    fn test_formal_derivative_constant_is_zero() {
        let f = FieldPoly::constant(fp7(5));
        let df = formal_derivative(&f);
        assert!(df.is_zero());
    }

    #[test]
    fn test_formal_derivative_linear() {
        // d/dx (3x + 2) = 3
        let f = FieldPoly::new(vec![fp7(2), fp7(3)]);
        let df = formal_derivative(&f);
        assert_eq!(df.degree(), Some(0));
        assert_eq!(df.try_coeff(0), Some(&fp7(3)));
    }

    #[test]
    fn test_formal_derivative_quadratic() {
        // d/dx (x^2 + 3x + 2) = 2x + 3 over Fp<7>
        let f = FieldPoly::new(vec![fp7(2), fp7(3), fp7(1)]);
        let df = formal_derivative(&f);
        assert_eq!(df.degree(), Some(1));
        assert_eq!(df.try_coeff(0), Some(&fp7(3))); // 1 * 3 = 3
        assert_eq!(df.try_coeff(1), Some(&fp7(2))); // 2 * 1 = 2
    }

    #[test]
    fn test_formal_derivative_characteristic_two() {
        // Over GF(2^4): d/dx(x^2 + x + 1) = 2x + 1 = 0x + 1 = 1
        // (since char 2: coefficient 2 becomes 0)
        let field = gf16_field();
        let f = FieldPoly::new(vec![field.element(1), field.element(1), field.element(1)]);
        let df = formal_derivative(&f);
        // x^2 term: coeff = 2*1 = 0 in char 2 → vanishes
        // x^1 term: coeff = 1*1 = 1
        assert_eq!(df.degree(), Some(0));
        assert_eq!(df.try_coeff(0), Some(&field.element(1)));
    }

    // -----------------------------------------------------------------
    // Proptests: round-trip on Fp<7>
    // -----------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        /// Round-trip: interpolate recovers the values at all input points (Fp<7>).
        #[test]
        fn prop_interpolate_round_trip_fp7(
            // Generate up to 6 distinct x-values from Fp<7> = {0..6}
            x_vals in prop::collection::hash_set(0u64..7, 1..7usize),
            y_vals in prop::collection::vec(0u64..7, 6..=6usize),
        ) {
            let xs: Vec<u64> = x_vals.into_iter().collect();
            let n = xs.len();
            let points: Vec<(FP7, FP7)> = xs.iter().zip(y_vals.iter().take(n))
                .map(|(&x, &y)| (fp7(x), fp7(y)))
                .collect();
            let p = interpolate(&points).unwrap();
            for (x, y) in &points {
                prop_assert_eq!(p.eval(x), *y);
            }
        }

        /// Round-trip: interpolate_fast recovers the values at all input points (Fp<7>).
        #[test]
        fn prop_interpolate_fast_round_trip_fp7(
            x_vals in prop::collection::hash_set(0u64..7, 1..7usize),
            y_vals in prop::collection::vec(0u64..7, 6..=6usize),
        ) {
            let xs: Vec<u64> = x_vals.into_iter().collect();
            let n = xs.len();
            let points: Vec<(FP7, FP7)> = xs.iter().zip(y_vals.iter().take(n))
                .map(|(&x, &y)| (fp7(x), fp7(y)))
                .collect();
            let p = interpolate_fast(&points).unwrap();
            for (x, y) in &points {
                prop_assert_eq!(p.eval(x), *y);
            }
        }

        /// Agreement: interpolate_fast == interpolate for n up to 6 on Fp<7>.
        #[test]
        fn prop_interpolate_agreement_fp7(
            x_vals in prop::collection::hash_set(0u64..7, 1..7usize),
            y_vals in prop::collection::vec(0u64..7, 6..=6usize),
        ) {
            let xs: Vec<u64> = x_vals.into_iter().collect();
            let n = xs.len();
            let points: Vec<(FP7, FP7)> = xs.iter().zip(y_vals.iter().take(n))
                .map(|(&x, &y)| (fp7(x), fp7(y)))
                .collect();
            let naive = interpolate(&points).unwrap();
            let fast = interpolate_fast(&points).unwrap();
            prop_assert_eq!(naive, fast);
        }
    }

    // -----------------------------------------------------------------
    // Proptests: round-trip on GF(2^4) (Gf2mElement, char 2)
    // -----------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// Round-trip: interpolate on GF(16).
        #[test]
        fn prop_interpolate_round_trip_gf16(
            x_vals in prop::collection::hash_set(0u64..16, 1..9usize),
            y_vals in prop::collection::vec(0u64..16, 8..=8usize),
        ) {
            let field = gf16_field();
            let xs: Vec<u64> = x_vals.into_iter().collect();
            let n = xs.len();
            let points: Vec<(Gf2mElement, Gf2mElement)> = xs.iter()
                .zip(y_vals.iter().take(n))
                .map(|(&x, &y)| (field.element(x), field.element(y)))
                .collect();
            let p = interpolate(&points).unwrap();
            for (x, y) in &points {
                prop_assert_eq!(p.eval(x), y.clone());
            }
        }

        /// Round-trip: interpolate_fast on GF(16).
        #[test]
        fn prop_interpolate_fast_round_trip_gf16(
            x_vals in prop::collection::hash_set(0u64..16, 1..9usize),
            y_vals in prop::collection::vec(0u64..16, 8..=8usize),
        ) {
            let field = gf16_field();
            let xs: Vec<u64> = x_vals.into_iter().collect();
            let n = xs.len();
            let points: Vec<(Gf2mElement, Gf2mElement)> = xs.iter()
                .zip(y_vals.iter().take(n))
                .map(|(&x, &y)| (field.element(x), field.element(y)))
                .collect();
            let p = interpolate_fast(&points).unwrap();
            for (x, y) in &points {
                prop_assert_eq!(p.eval(x), y.clone());
            }
        }

        /// Agreement: interpolate_fast == interpolate on GF(16) for n up to 8.
        #[test]
        fn prop_interpolate_agreement_gf16(
            x_vals in prop::collection::hash_set(0u64..16, 1..9usize),
            y_vals in prop::collection::vec(0u64..16, 8..=8usize),
        ) {
            let field = gf16_field();
            let xs: Vec<u64> = x_vals.into_iter().collect();
            let n = xs.len();
            let points: Vec<(Gf2mElement, Gf2mElement)> = xs.iter()
                .zip(y_vals.iter().take(n))
                .map(|(&x, &y)| (field.element(x), field.element(y)))
                .collect();
            let naive = interpolate(&points).unwrap();
            let fast = interpolate_fast(&points).unwrap();
            prop_assert_eq!(naive, fast);
        }
    }

    // -----------------------------------------------------------------
    // Proptests: agreement on Fp<65537> up to n=32
    // -----------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Agreement: interpolate_fast == interpolate on Fp<65537> for n up to 32.
        #[test]
        fn prop_interpolate_agreement_fp65537_n32(
            x_vals in prop::collection::hash_set(1u64..65537, 1..33usize),
            y_vals in prop::collection::vec(0u64..65537, 32..=32usize),
        ) {
            type FP = Fp<65537>;
            let xs: Vec<u64> = x_vals.into_iter().collect();
            let n = xs.len();
            let points: Vec<(FP, FP)> = xs.iter()
                .zip(y_vals.iter().take(n))
                .map(|(&x, &y)| (FP::new(x), FP::new(y)))
                .collect();
            let naive = interpolate(&points).unwrap();
            let fast = interpolate_fast(&points).unwrap();
            prop_assert_eq!(naive, fast);
        }
    }
}
