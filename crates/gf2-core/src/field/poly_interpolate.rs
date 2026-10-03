//! Lagrange polynomial interpolation over any [`FiniteField`].
//!
//! Each entry point returns the unique polynomial of degree at most `n − 1`
//! through `n` points with distinct `x_i`, or
//! [`InterpolationError::DuplicatePoint`]. [`interpolate`] is the `O(n²)`
//! barycentric form and [`interpolate_fast`] the subproduct-tree form;
//! [`interpolate_auto`] selects between them at the active
//! `polynomial.interpolate_fast_min_points()` value. [`interpolate_fast_auto`]
//! and [`interpolate_auto_two_adic`] are the [`TwoAdicField`] forms, whose
//! `M'(x_i)` evaluation uses [`FieldPoly::batch_evaluate_auto`].

use crate::field::batch_ops::batch_inverse;
use crate::field::poly::build_subproduct_tree;
use crate::field::{FieldPoly, FiniteField, TwoAdicField};
use crate::tuning;
use std::fmt;

/// Conservative default for `polynomial.interpolate_fast_min_points()`
/// in the active [`crate::tuning::CoreTuning`].
///
/// Number-of-points threshold at which [`interpolate_auto`] prefers
/// [`interpolate_fast`] over [`interpolate`]; [`interpolate_route`]
/// reads the live value.
///
/// [`crate::tuning::CoreTuning::CONSERVATIVE`] consumes this constant.
pub const INTERPOLATE_THRESHOLD: usize = 16;

/// The selected arm of the [`interpolate_auto`] / [`interpolate_auto_two_adic`]
/// point-count dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterpolateRoute {
    /// Use the quadratic barycentric [`interpolate`].
    Barycentric,
    /// Use the subproduct-tree fast path: [`interpolate_fast`] for
    /// [`interpolate_auto`], [`interpolate_fast_auto`] for
    /// [`interpolate_auto_two_adic`].
    SubproductTree,
}

/// Reports the [`interpolate_auto`] / [`interpolate_auto_two_adic`] arm
/// for a point count.
///
/// The comparison uses the active `polynomial.interpolate_fast_min_points()`
/// profile value (conservative default [`INTERPOLATE_THRESHOLD`]). Both
/// dispatchers share this reporter and this boundary.
#[must_use]
pub fn interpolate_route(points_len: usize) -> InterpolateRoute {
    interpolate_route_resolved(
        tuning::active().polynomial().interpolate_fast_min_points(),
        points_len,
    )
}

/// Reports the [`interpolate_route`] arm for `points_len` against an
/// already-resolved `interpolate_fast_min_points`.
fn interpolate_route_resolved(
    interpolate_fast_min_points: usize,
    points_len: usize,
) -> InterpolateRoute {
    if points_len < interpolate_fast_min_points {
        InterpolateRoute::Barycentric
    } else {
        InterpolateRoute::SubproductTree
    }
}

/// Interpolates through `points` using the threshold-tuned dispatcher.
///
/// Routes to [`interpolate`] when `points.len()` is below the active
/// `polynomial.interpolate_fast_min_points()` value and to
/// [`interpolate_fast`] at or above it.
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] if any two `x_i` coincide.
///
/// # Complexity
///
/// `O(n²)` field operations on the [`interpolate`] route and `O(n² log n)` on
/// the [`interpolate_fast`] route.
pub fn interpolate_auto<F: FiniteField>(
    points: &[(F, F)],
) -> Result<FieldPoly<F>, InterpolationError> {
    match interpolate_route(points.len()) {
        InterpolateRoute::Barycentric => interpolate(points),
        InterpolateRoute::SubproductTree => interpolate_fast(points),
    }
}

/// [`TwoAdicField`]-specialised sibling of [`interpolate_auto`].
///
/// Routes as [`interpolate_auto`], with [`interpolate_fast_auto`] as the
/// subproduct-tree arm.
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] if any two `x_i` coincide.
///
/// # Complexity
///
/// `O(n²)` field operations on the [`interpolate`] route and `O(n² log n)` on
/// the [`interpolate_fast_auto`] route.
pub fn interpolate_auto_two_adic<F: TwoAdicField>(
    points: &[(F, F)],
) -> Result<FieldPoly<F>, InterpolationError> {
    match interpolate_route(points.len()) {
        InterpolateRoute::Barycentric => interpolate(points),
        InterpolateRoute::SubproductTree => interpolate_fast_auto(points),
    }
}

/// Error returned by [`interpolate`] and [`interpolate_fast`] when the
/// input is invalid.
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

/// Computes the formal derivative `f'(x) = Σ i · a_i · x^{i-1}` of `f`.
///
/// The index `i` is formed by repeated field addition of one, so terms with
/// `i` a multiple of the characteristic vanish.
pub fn formal_derivative<F: FiniteField>(f: &FieldPoly<F>) -> FieldPoly<F> {
    let n = f.len();
    if n <= 1 {
        return FieldPoly::new(vec![]);
    }

    let sample = f.try_coeff(1).unwrap();
    let one = sample.one_like();

    let mut deriv_coeffs: Vec<F> = Vec::with_capacity(n - 1);
    let mut i_field = one.clone(); // i = 1 for the first derivative term
    for i in 1..n {
        let ai = f.try_coeff(i).unwrap();
        deriv_coeffs.push(i_field.clone() * ai.clone());
        i_field += one.clone();
    }

    FieldPoly::new(deriv_coeffs)
}

/// Scans for the first pair of duplicate x-coordinates in O(n²).
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

/// Lagrange interpolation via the **barycentric form** in O(n²).
///
/// Given `n` distinct (x, y) pairs, returns the unique polynomial of degree
/// at most `n − 1` satisfying `L(x_i) = y_i`.
///
/// **Empty input** returns the zero polynomial. **Single point** `(x, y)`
/// returns the constant polynomial `y`.
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] with the indices of the
/// first pair sharing an `x`-coordinate.
///
/// # Complexity
///
/// `O(n²)` field operations and one field inversion.
pub fn interpolate<F: FiniteField>(points: &[(F, F)]) -> Result<FieldPoly<F>, InterpolationError> {
    let n = points.len();

    if n == 0 {
        return Ok(FieldPoly::new(vec![]));
    }

    if n == 1 {
        return Ok(FieldPoly::constant(points[0].1.clone()));
    }

    check_no_duplicate_x(points)?;

    let xs: Vec<F> = points.iter().map(|(x, _)| x.clone()).collect();
    let ys: Vec<F> = points.iter().map(|(_, y)| y.clone()).collect();

    // Denominators d[i] = Π_{j≠i}(x_i − x_j).
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

    let weights =
        batch_inverse(&denoms).expect("denominators are non-zero for distinct x-coordinates");

    let wy: Vec<F> = weights
        .into_iter()
        .zip(ys.iter())
        .map(|(w, y)| w * y.clone())
        .collect();

    // L(x) = Σ_i wy[i] · M(x) / (x − x_i) with M(x) = Π_i (x − x_i).

    let one = xs[0].one_like();
    let m = FieldPoly::from_roots(&xs);

    let zero_poly = FieldPoly::new(vec![]);
    let mut result: FieldPoly<F> = zero_poly;

    for i in 0..n {
        let linear = FieldPoly::new(vec![-xs[i].clone(), one.clone()]);
        // M(x) / (x - x_i) — exact division because x_i is a root of M.
        let (quotient, _rem) = m.div_rem(&linear);
        let mut term = quotient;
        term.scale(&wy[i]);
        result += term;
    }

    Ok(result)
}

/// Lagrange interpolation via the **subproduct-tree** algorithm.
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
/// 2. Compute `M'(x)` via [`formal_derivative`].
/// 3. Evaluate `M'` at all `x_i` via [`FieldPoly::batch_evaluate`]; by the
///    product rule, `M'(x_i) = Π_{j ≠ i} (x_i − x_j)`.
/// 4. Compute barycentric weights `w_i = y_i / M'(x_i)` using
///    [`crate::field::batch_ops::batch_inverse`].
/// 5. Upward merge over the [`build_subproduct_tree`] nodes from the leaves
///    `[w_0, …, w_{n-1}]` with
///    `L_{left+right}(x) = L_left(x) · M_right(x) + L_right(x) · M_left(x)`.
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] with the indices of the
/// first pair sharing an `x`-coordinate.
///
/// # Complexity
///
/// `O(n² log n)` field operations.
pub fn interpolate_fast<F: FiniteField>(
    points: &[(F, F)],
) -> Result<FieldPoly<F>, InterpolationError> {
    interpolate_fast_with_batch_eval(points, |poly, xs| poly.batch_evaluate(xs))
}

/// [`TwoAdicField`]-specialised sibling of [`interpolate_fast`].
///
/// Same contract as [`interpolate_fast`], with the `M'(x_i)` evaluation through
/// [`FieldPoly::batch_evaluate_auto`].
///
/// # Errors
///
/// Returns [`InterpolationError::DuplicatePoint`] with the indices of the
/// first pair sharing an `x`-coordinate.
///
/// # Complexity
///
/// `O(n² log n)` field operations.
pub fn interpolate_fast_auto<F: TwoAdicField>(
    points: &[(F, F)],
) -> Result<FieldPoly<F>, InterpolationError> {
    interpolate_fast_with_batch_eval(points, |poly, xs| poly.batch_evaluate_auto(xs))
}

/// Shared body of [`interpolate_fast`] and [`interpolate_fast_auto`];
/// `batch_eval` evaluates `M'` at the `x_i`.
fn interpolate_fast_with_batch_eval<F, E>(
    points: &[(F, F)],
    batch_eval: E,
) -> Result<FieldPoly<F>, InterpolationError>
where
    F: FiniteField,
    E: Fn(&FieldPoly<F>, &[F]) -> Vec<F>,
{
    let n = points.len();

    if n == 0 {
        return Ok(FieldPoly::new(vec![]));
    }

    if n == 1 {
        return Ok(FieldPoly::constant(points[0].1.clone()));
    }

    check_no_duplicate_x(points)?;

    let xs: Vec<F> = points.iter().map(|(x, _)| x.clone()).collect();
    let ys: Vec<F> = points.iter().map(|(_, y)| y.clone()).collect();

    let m_poly = FieldPoly::from_roots(&xs);
    let m_deriv = formal_derivative(&m_poly);

    // By the product rule, M'(x_i) = Π_{j≠i}(x_i − x_j).
    let m_prime_vals: Vec<F> = batch_eval(&m_deriv, &xs);

    // w_i = y_i / M'(x_i); M'(x_i) is non-zero for distinct x_i.
    let m_prime_invs =
        batch_inverse(&m_prime_vals).expect("M'(x_i) is non-zero for distinct evaluation points");

    let weights: Vec<F> = m_prime_invs
        .into_iter()
        .zip(ys.iter())
        .map(|(inv, y)| inv * y.clone())
        .collect();

    // Upward merge: leaves `L_i(x) = w_i`, then
    //   `L_{left ∪ right}(x) = L_left(x) · M_right(x) + L_right(x) · M_left(x)`.
    let tree = build_subproduct_tree(&xs);

    let mut cur_interp: Vec<FieldPoly<F>> = weights.into_iter().map(FieldPoly::constant).collect();

    // Bottom-up merge: iterate over every level except the root (the last).
    let tree_len = tree.len();
    for prods in tree.iter().take(tree_len - 1) {
        let next_len = prods.len().div_ceil(2);
        let mut next_interp: Vec<FieldPoly<F>> = Vec::with_capacity(next_len);

        let mut i = 0;
        while i + 1 < cur_interp.len() {
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
        assert_eq!(df.degree(), Some(0));
        assert_eq!(df.try_coeff(0), Some(&field.element(1)));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        #[test]
        fn prop_interpolate_round_trip_fp7(
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

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

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

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

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

        #[test]
        fn prop_interpolate_fast_auto_matches_fast_fp65537_n32(
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
            let fast = interpolate_fast(&points).unwrap();
            let fast_auto = interpolate_fast_auto(&points).unwrap();
            prop_assert_eq!(fast, fast_auto);
        }

        #[test]
        fn prop_interpolate_auto_two_adic_matches_auto_fp65537_n32(
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
            let auto = interpolate_auto(&points).unwrap();
            let auto_two_adic = interpolate_auto_two_adic(&points).unwrap();
            prop_assert_eq!(auto, auto_two_adic);
        }
    }

    #[test]
    fn test_interpolate_auto_two_adic_routes_through_fast_auto_above_threshold() {
        type FP = Fp<65537>;
        // Above INTERPOLATE_THRESHOLD and well below SUBPRODUCT_THRESHOLD.
        let n = INTERPOLATE_THRESHOLD + 4;
        let points: Vec<(FP, FP)> = (0..n as u64)
            .map(|i| (FP::new(i + 1), FP::new(((i * 7) % 65537) + 1)))
            .collect();

        let via_auto = interpolate_auto_two_adic(&points).unwrap();
        let via_fast_auto = interpolate_fast_auto(&points).unwrap();
        assert_eq!(via_auto, via_fast_auto);

        for (x, y) in &points {
            assert_eq!(via_auto.eval(x), *y);
        }
    }
}
