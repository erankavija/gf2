//! Strassen–Winograd recursive matrix multiplication over a
//! [`FiniteField`] (`@/citation/DumasPernet2012` §1.4, algorithm 1.6):
//! 7 recursive half-size multiplies and 15 block additions per level.

use crate::field::matrix::{gemm, FieldMatrix};
use crate::field::{FieldVec, FiniteField};
use crate::tuning;

#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicU8, Ordering};

/// Conservative default for `gemm.winograd_min_dim`, consumed by
/// [`crate::tuning::CoreTuning::CONSERVATIVE`]: the dimension below which
/// [`gemm_winograd`] falls back to the classical blocked [`gemm`].
pub(crate) const WINOGRAD_MIN_DIM_DEFAULT: usize = 128;

/// The selected arm of the [`gemm_winograd`] dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinogradRoute {
    /// Use the classical blocked [`gemm`] base case.
    Classical,
    /// Peel a Winograd level and continue through the recursive kernel.
    Winograd,
}

/// Reports the [`gemm_winograd`] arm for an `m × k` by `k × n` product.
///
/// The comparison uses the active `gemm.winograd_min_dim` profile field,
/// floored at two.
#[must_use]
pub fn winograd_route(m: usize, k: usize, n: usize) -> WinogradRoute {
    winograd_route_resolved(tuning::active().gemm().winograd_min_dim(), m, k, n)
}

/// The base-case comparison shared by the route reporter and every
/// dispatch site: `true` when the classical base case fires for the given
/// shape at the given threshold. The floor at 2 keeps the half-dimension
/// of a peel non-zero.
#[inline]
pub(crate) fn winograd_takes_base_case(threshold: usize, m: usize, k: usize, n: usize) -> bool {
    m.min(k).min(n) < threshold.max(2)
}

fn winograd_route_resolved(winograd_min_dim: usize, m: usize, k: usize, n: usize) -> WinogradRoute {
    if winograd_takes_base_case(winograd_min_dim, m, k, n) {
        WinogradRoute::Classical
    } else {
        WinogradRoute::Winograd
    }
}

#[cfg(any(test, feature = "test-support"))]
static LAST_WINOGRAD_DISPATCH_ROUTE: AtomicU8 = AtomicU8::new(0);

#[cfg(any(test, feature = "test-support"))]
fn record_winograd_dispatch_route(route: WinogradRoute) {
    let value = match route {
        WinogradRoute::Classical => 1,
        WinogradRoute::Winograd => 2,
    };
    LAST_WINOGRAD_DISPATCH_ROUTE.store(value, Ordering::Relaxed);
}

/// Clears the test-support observation of the last production Winograd route.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_winograd_dispatch_route() {
    LAST_WINOGRAD_DISPATCH_ROUTE.store(0, Ordering::Relaxed);
}

/// Returns the route consumed by the last [`gemm_winograd`] dispatch.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_winograd_dispatch_route() -> Option<WinogradRoute> {
    match LAST_WINOGRAD_DISPATCH_ROUTE.load(Ordering::Relaxed) {
        1 => Some(WinogradRoute::Classical),
        2 => Some(WinogradRoute::Winograd),
        _ => None,
    }
}

/// Strassen–Winograd matrix multiplication over an arbitrary
/// [`FiniteField`]: the `m × n` product of `a` (`m × k`) and `b`
/// (`k × n`).
///
/// A product with any dimension below the active `gemm.winograd_min_dim`
/// profile field goes to the classical blocked [`gemm`]. Otherwise one
/// level of the 7-multiply split is peeled and the seven half-size
/// products recurse, unless [`theorem_4_bound`] at the next depth exceeds
/// `F::max_unreduced_additions() · (p − 1)²`, which also selects [`gemm`].
/// Odd dimensions are padded with zeros to the next even value and the
/// output is sliced back.
///
/// # Panics
///
/// Panics if `a.cols != b.rows`. Also panics (with the same contract as
/// [`gemm`]) for the `(m, 0) × (0, n)` shape on runtime-context fields
/// when both factors carry empty storage and `F::zero_hint()` returns
/// `None`.
///
/// # Complexity
///
/// `O(n^log₂ 7) ≈ O(n^2.807)` field multiplications above the threshold,
/// classical `O(n³)` at the base case. The scratch held along one
/// recursion path is `O(n²)`.
pub fn gemm_winograd<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    let winograd_min_dim = tuning::active().gemm().winograd_min_dim();
    let (m, k) = a.shape();
    let n = b.cols();
    let _route = winograd_route_resolved(winograd_min_dim, m, k, n);
    #[cfg(any(test, feature = "test-support"))]
    record_winograd_dispatch_route(_route);
    gemm_winograd_with_threshold(a, b, winograd_min_dim)
}

/// [`gemm_winograd`] with an explicit base-case threshold in place of the
/// active `gemm.winograd_min_dim` profile field: the base case fires at
/// `min(m, k, n) < max(threshold, 2)`, and `usize::MAX` forces the
/// classical path.
///
/// # Panics
///
/// Same as [`gemm_winograd`].
///
/// # Complexity
///
/// Same as [`gemm_winograd`].
#[doc(hidden)]
pub fn gemm_winograd_with_threshold<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    threshold: usize,
) -> FieldMatrix<F> {
    gemm_winograd_inner(a, b, threshold, 0)
}

/// Recursive Winograd kernel. `level` is the recursion depth, 0 at the
/// public entry points, and feeds the theorem-4 gate.
fn gemm_winograd_inner<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    threshold: usize,
    level: u32,
) -> FieldMatrix<F> {
    assert_eq!(
        a.cols(),
        b.rows(),
        "gemm_winograd: inner dimensions must match ({} vs {})",
        a.cols(),
        b.rows()
    );

    let (m, k) = a.shape();
    let n = b.cols();

    // `gemm` handles the zero witness of empty shapes.
    if m == 0 || k == 0 || n == 0 {
        return gemm(a, b);
    }

    if winograd_takes_base_case(threshold, m, k, n) {
        return gemm(a, b);
    }

    // Refuse to recurse when the `@/citation/DumasPernet2012` §1.4
    // theorem 4 bound on an intermediate cell at depth `level + 1` exceeds
    // the base-case headroom `F::max_unreduced_additions() · (p − 1)²`.
    // Binary fields report `usize::MAX` / `u128::MAX` and skip the gate.
    let kmax = F::max_unreduced_additions();
    let p_minus_1 = F::theorem_4_operand_bound();
    if kmax != usize::MAX && p_minus_1 != u128::MAX {
        let bound = theorem_4_bound(level + 1, k, p_minus_1);
        let headroom = (kmax as u128)
            .saturating_mul(p_minus_1)
            .saturating_mul(p_minus_1);
        if bound > headroom {
            return gemm(a, b);
        }
    }

    debug_assert!(!a.is_empty() && !b.is_empty());
    let zero: F = a.get(0, 0).zero_like();

    // Pad odd dimensions up to the next even value.
    let m_even = m + (m & 1);
    let k_even = k + (k & 1);
    let n_even = n + (n & 1);

    let a_padded = pad_to(a, m_even, k_even, &zero);
    let b_padded = pad_to(b, k_even, n_even, &zero);

    let c_padded = winograd_step(&a_padded, &b_padded, &zero, threshold, level);

    if (m_even, n_even) == (m, n) {
        c_padded
    } else {
        slice_to(&c_padded, m, n)
    }
}

/// One Winograd level: the S/T sums, seven recursive multiplies and the
/// U-assembly. Both inputs must have all-even dimensions.
fn winograd_step<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    zero: &F,
    threshold: usize,
    level: u32,
) -> FieldMatrix<F> {
    let (m, k) = a.shape();
    let n = b.cols();
    debug_assert_eq!(a.cols(), b.rows());
    debug_assert!(m.is_multiple_of(2) && k.is_multiple_of(2) && n.is_multiple_of(2));

    let mh = m / 2;
    let kh = k / 2;
    let nh = n / 2;

    let a11 = submatrix(a, 0, 0, mh, kh, zero);
    let a12 = submatrix(a, 0, kh, mh, kh, zero);
    let a21 = submatrix(a, mh, 0, mh, kh, zero);
    let a22 = submatrix(a, mh, kh, mh, kh, zero);

    let b11 = submatrix(b, 0, 0, kh, nh, zero);
    let b12 = submatrix(b, 0, nh, kh, nh, zero);
    let b21 = submatrix(b, kh, 0, kh, nh, zero);
    let b22 = submatrix(b, kh, nh, kh, nh, zero);

    // S, T, M and U follow `@/citation/DumasPernet2012` §1.4,
    // algorithm 1.6.
    let s1 = add_mats(&a21, &a22);
    let s2 = sub_mats(&s1, &a11);
    let s3 = sub_mats(&a11, &a21);
    let s4 = sub_mats(&a12, &s2);

    let t1 = sub_mats(&b12, &b11);
    let t2 = sub_mats(&b22, &t1);
    let t3 = sub_mats(&b22, &b12);
    let t4 = sub_mats(&t2, &b21);

    let m1 = gemm_winograd_inner(&a11, &b11, threshold, level + 1);
    let m2 = gemm_winograd_inner(&a12, &b21, threshold, level + 1);
    let m3 = gemm_winograd_inner(&s4, &b22, threshold, level + 1);
    let m4 = gemm_winograd_inner(&a22, &t4, threshold, level + 1);
    let m5 = gemm_winograd_inner(&s1, &t1, threshold, level + 1);
    let m6 = gemm_winograd_inner(&s2, &t2, threshold, level + 1);
    let m7 = gemm_winograd_inner(&s3, &t3, threshold, level + 1);

    let c11 = add_mats(&m1, &m2);
    let u2 = add_mats(&m1, &m6);
    let u3 = add_mats(&u2, &m7);
    let u4 = add_mats(&u2, &m5);
    let c12 = add_mats(&u4, &m3);
    let c21 = sub_mats(&u3, &m4);
    let c22 = add_mats(&u3, &m5);

    assemble_quarters(&c11, &c12, &c21, &c22, zero)
}

/// Whether every cell of `mat`, read through `value_of`, is at most
/// `theorem_4_bound(level, k, p_minus_1)`.
#[cfg(test)]
fn canonical_values_respect_bound<F: FiniteField>(
    mat: &FieldMatrix<F>,
    level: u32,
    k: usize,
    p_minus_1: u128,
    value_of: impl Fn(&F) -> u128,
) -> bool {
    let bound = theorem_4_bound(level, k, p_minus_1);
    let (rows, cols) = mat.shape();
    for r in 0..rows {
        for c in 0..cols {
            let v = value_of(&mat.get_unchecked(r, c));
            if v > bound {
                return false;
            }
        }
    }
    true
}

/// Returns a freshly allocated `rows × cols` matrix that contains `src`
/// in its top-left corner, with the remaining cells set to `zero`.
fn pad_to<F: FiniteField>(
    src: &FieldMatrix<F>,
    rows: usize,
    cols: usize,
    zero: &F,
) -> FieldMatrix<F> {
    let (sr, sc) = src.shape();
    debug_assert!(sr <= rows && sc <= cols);
    if (sr, sc) == (rows, cols) {
        return src.clone();
    }
    let data = FieldVec::zeros_from(rows * cols, zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..sr {
        for c in 0..sc {
            out.set(r, c, src.get_unchecked(r, c));
        }
    }
    out
}

/// Returns a freshly allocated `rows × cols` view of the top-left corner
/// of `src`.
fn slice_to<F: FiniteField>(src: &FieldMatrix<F>, rows: usize, cols: usize) -> FieldMatrix<F> {
    let (sr, sc) = src.shape();
    debug_assert!(rows <= sr && cols <= sc);
    if (rows, cols) == (sr, sc) {
        return src.clone();
    }
    let zero = src.get(0, 0).zero_like();
    let data = FieldVec::zeros_from(rows * cols, &zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, src.get_unchecked(r, c));
        }
    }
    out
}

/// Extracts a freshly allocated `rows × cols` sub-matrix at the given
/// offset. The recursion works on owned row-major storage, which keeps
/// the base-case gemm on contiguous operands.
fn submatrix<F: FiniteField>(
    src: &FieldMatrix<F>,
    row_off: usize,
    col_off: usize,
    rows: usize,
    cols: usize,
    zero: &F,
) -> FieldMatrix<F> {
    debug_assert!(row_off + rows <= src.rows());
    debug_assert!(col_off + cols <= src.cols());
    let data = FieldVec::zeros_from(rows * cols, zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, src.get_unchecked(row_off + r, col_off + c));
        }
    }
    out
}

/// Stitches four equally-sized quarter matrices into a single `(2·mh) ×
/// (2·nh)` matrix.
fn assemble_quarters<F: FiniteField>(
    c11: &FieldMatrix<F>,
    c12: &FieldMatrix<F>,
    c21: &FieldMatrix<F>,
    c22: &FieldMatrix<F>,
    zero: &F,
) -> FieldMatrix<F> {
    let (mh, nh) = c11.shape();
    debug_assert_eq!(c12.shape(), (mh, nh));
    debug_assert_eq!(c21.shape(), (mh, nh));
    debug_assert_eq!(c22.shape(), (mh, nh));
    let m = 2 * mh;
    let n = 2 * nh;
    let data = FieldVec::zeros_from(m * n, zero);
    let mut out = FieldMatrix::from_raw_parts(m, n, data);
    for r in 0..mh {
        for c in 0..nh {
            out.set(r, c, c11.get_unchecked(r, c));
            out.set(r, nh + c, c12.get_unchecked(r, c));
            out.set(mh + r, c, c21.get_unchecked(r, c));
            out.set(mh + r, nh + c, c22.get_unchecked(r, c));
        }
    }
    out
}

/// Elementwise `A + B` producing a fresh matrix.
fn add_mats<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    debug_assert_eq!(a.shape(), b.shape());
    let (rows, cols) = a.shape();
    let zero = a.get(0, 0).zero_like();
    let data = FieldVec::zeros_from(rows * cols, &zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, a.get_unchecked(r, c) + b.get_unchecked(r, c));
        }
    }
    out
}

/// Elementwise `A − B` producing a fresh matrix.
fn sub_mats<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    debug_assert_eq!(a.shape(), b.shape());
    let (rows, cols) = a.shape();
    let zero = a.get(0, 0).zero_like();
    let data = FieldVec::zeros_from(rows * cols, &zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, a.get_unchecked(r, c) - b.get_unchecked(r, c));
        }
    }
    out
}

/// The `@/citation/DumasPernet2012` §1.4 theorem 4 bound: after `levels`
/// recursion levels every intermediate cell value satisfies
/// `|z| ≤ ((1 + 3^levels) / 2)² · ceil(k / 2^levels) · (p − 1)²`.
///
/// `k` is the inner dimension at the top of the recursion and `p_minus_1`
/// the per-cell operand bound. The product saturates at `u128::MAX`.
pub fn theorem_4_bound(levels: u32, k: usize, p_minus_1: u128) -> u128 {
    if p_minus_1 == 0 {
        return 0;
    }
    let three_pow_l: u128 = 3u128.pow(levels);
    let one_plus = 1u128 + three_pow_l;
    // ((1 + 3^l) / 2)² — divisible since 1 + 3^l is always even.
    debug_assert!(one_plus.is_multiple_of(2));
    let half = one_plus / 2;
    let factor = half.saturating_mul(half);

    let divisor = 1usize << (levels as usize).min(usize::BITS as usize - 1);
    let ceil_k = if divisor == 0 {
        k as u128
    } else {
        k.div_ceil(divisor) as u128
    };

    factor
        .saturating_mul(ceil_k)
        .saturating_mul(p_minus_1)
        .saturating_mul(p_minus_1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::gemm;
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use rand::{Rng, SeedableRng};

    // GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
    struct WinoGf2m8Cfg;
    impl Gf2mWideConfig<1> for WinoGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "WinoGf2m8Cfg";
    }
    type WinoGf2m8 = Gf2mWide<1, WinoGf2m8Cfg>;

    const MERSENNE_31: u64 = 2_147_483_647;

    const TEST_THRESHOLD: usize = WINOGRAD_MIN_DIM_DEFAULT;

    fn random_fp<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
            }
        }
        m
    }

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<WinoGf2m8> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<WinoGf2m8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, WinoGf2m8::new([rng.gen::<u64>() & 0xFF]));
            }
        }
        m
    }

    #[test]
    fn test_winograd_below_threshold_fp_small() {
        let a = random_fp::<7>(10, 12, 0x01);
        let b = random_fp::<7>(12, 8, 0x02);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_at_threshold_fp() {
        let n = TEST_THRESHOLD;
        let a = random_fp::<65_521>(n, n, 0xA1);
        let b = random_fp::<65_521>(n, n, 0xA2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_above_threshold_fp() {
        let n = TEST_THRESHOLD + 2;
        let a = random_fp::<65_521>(n, n, 0xB1);
        let b = random_fp::<65_521>(n, n, 0xB2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_one_below_threshold_fp() {
        let n = TEST_THRESHOLD - 1;
        let a = random_fp::<65_521>(n, n, 0xC1);
        let b = random_fp::<65_521>(n, n, 0xC2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_odd_m_fp() {
        let n = TEST_THRESHOLD + 1; // odd m
        let k = TEST_THRESHOLD + 4;
        let nn = TEST_THRESHOLD + 4;
        let a = random_fp::<65_521>(n, k, 0xD1);
        let b = random_fp::<65_521>(k, nn, 0xD2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd m");
    }

    #[test]
    fn test_winograd_odd_k_fp() {
        let m = TEST_THRESHOLD + 4;
        let k = TEST_THRESHOLD + 1; // odd k
        let n = TEST_THRESHOLD + 4;
        let a = random_fp::<65_521>(m, k, 0xE1);
        let b = random_fp::<65_521>(k, n, 0xE2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd k");
    }

    #[test]
    fn test_winograd_odd_n_fp() {
        let m = TEST_THRESHOLD + 4;
        let k = TEST_THRESHOLD + 4;
        let n = TEST_THRESHOLD + 1; // odd n
        let a = random_fp::<65_521>(m, k, 0xF1);
        let b = random_fp::<65_521>(k, n, 0xF2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd n");
    }

    #[test]
    fn test_winograd_all_odd_fp() {
        let m = TEST_THRESHOLD + 1;
        let k = TEST_THRESHOLD + 3;
        let n = TEST_THRESHOLD + 5;
        let a = random_fp::<65_521>(m, k, 0x11);
        let b = random_fp::<65_521>(k, n, 0x12);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "all three odd");
    }

    #[test]
    fn test_winograd_all_odd_gf2m8() {
        let m = TEST_THRESHOLD + 1;
        let k = TEST_THRESHOLD + 3;
        let n = TEST_THRESHOLD + 5;
        let a = random_gf2m8(m, k, 0x21);
        let b = random_gf2m8(k, n, 0x22);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "gf2m8 all odd");
    }

    #[test]
    fn test_winograd_empty_outer() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 5);
        let b = FieldMatrix::<Fp<7>>::zeros(5, 3);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got.shape(), (0, 3));
    }

    #[test]
    fn test_winograd_empty_inner_const_field() {
        let a = FieldMatrix::<Fp<7>>::zeros(3, 0);
        let b = FieldMatrix::<Fp<7>>::zeros(0, 4);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got.shape(), (3, 4));
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(got.get(r, c), Fp::<7>::new(0));
            }
        }
    }

    #[test]
    fn test_pad_slice_roundtrip_preserves_values() {
        let n = TEST_THRESHOLD + 3;
        let a = random_fp::<65_521>(n, n, 0x31);
        let padded = pad_to(&a, n + 1, n + 1, &Fp::<65_521>::new(0));
        assert_eq!(padded.shape(), (n + 1, n + 1));
        for r in 0..n {
            for c in 0..n {
                assert_eq!(padded.get(r, c), a.get(r, c), "({}, {})", r, c);
            }
        }
        for r in 0..n {
            assert_eq!(padded.get(r, n), Fp::<65_521>::new(0));
        }
        for c in 0..=n {
            assert_eq!(padded.get(n, c), Fp::<65_521>::new(0));
        }
        let sliced = slice_to(&padded, n, n);
        assert_eq!(sliced, a);
    }

    #[test]
    fn test_theorem_4_bound_level_0_matches_classical() {
        // At level 0 the bound is `1 · k · (p-1)²`, exactly the classical
        // gemm inner-sum bound.
        let p_m1 = 6u128;
        let k = 17usize;
        assert_eq!(theorem_4_bound(0, k, p_m1), (k as u128) * p_m1 * p_m1);
    }

    #[test]
    fn test_theorem_4_bound_level_1_formula() {
        // Level 1: ((1+3)/2)² = 4, ceil(k/2) = ceil(17/2) = 9.
        let p_m1 = 6u128;
        let k = 17usize;
        assert_eq!(theorem_4_bound(1, k, p_m1), 4 * 9 * p_m1 * p_m1);
    }

    #[test]
    fn test_theorem_4_bound_level_2_formula() {
        // Level 2: ((1+9)/2)² = 25, ceil(k/4) = ceil(17/4) = 5.
        let p_m1 = 6u128;
        let k = 17usize;
        assert_eq!(theorem_4_bound(2, k, p_m1), 25 * 5 * p_m1 * p_m1);
    }

    #[test]
    fn test_theorem_4_bound_zero_field() {
        // Degenerate p = 1 → bound is 0.
        assert_eq!(theorem_4_bound(3, 16, 0), 0);
    }

    /// Mirrors [`gemm_winograd_inner`] and asserts, at each level, that
    /// every S/T block entering a recursive multiply has canonical values
    /// within the theorem-4 bound of the level being entered.
    fn verify_bound_recursive(
        a: &FieldMatrix<Fp<MERSENNE_31>>,
        b: &FieldMatrix<Fp<MERSENNE_31>>,
        threshold: usize,
        level: u32,
        k_top: usize,
        p_minus_1: u128,
    ) -> FieldMatrix<Fp<MERSENNE_31>> {
        let (m, k) = a.shape();
        let n = b.cols();
        if m == 0 || k == 0 || n == 0 {
            return gemm(a, b);
        }
        if winograd_takes_base_case(threshold, m, k, n) {
            assert!(
                canonical_values_respect_bound(a, level, k_top, p_minus_1, |f| f.value() as u128),
                "level {} operand A fails theorem-4 bound",
                level
            );
            assert!(
                canonical_values_respect_bound(b, level, k_top, p_minus_1, |f| f.value() as u128),
                "level {} operand B fails theorem-4 bound",
                level
            );
            return gemm(a, b);
        }
        let zero = a.get(0, 0).zero_like();
        let m_even = m + (m & 1);
        let k_even = k + (k & 1);
        let n_even = n + (n & 1);
        let a_p = pad_to(a, m_even, k_even, &zero);
        let b_p = pad_to(b, k_even, n_even, &zero);
        let mh = m_even / 2;
        let kh = k_even / 2;
        let nh = n_even / 2;
        let a11 = submatrix(&a_p, 0, 0, mh, kh, &zero);
        let a12 = submatrix(&a_p, 0, kh, mh, kh, &zero);
        let a21 = submatrix(&a_p, mh, 0, mh, kh, &zero);
        let a22 = submatrix(&a_p, mh, kh, mh, kh, &zero);
        let b11 = submatrix(&b_p, 0, 0, kh, nh, &zero);
        let b12 = submatrix(&b_p, 0, nh, kh, nh, &zero);
        let b21 = submatrix(&b_p, kh, 0, kh, nh, &zero);
        let b22 = submatrix(&b_p, kh, nh, kh, nh, &zero);
        let s1 = add_mats(&a21, &a22);
        let s2 = sub_mats(&s1, &a11);
        let s3 = sub_mats(&a11, &a21);
        let s4 = sub_mats(&a12, &s2);
        let t1 = sub_mats(&b12, &b11);
        let t2 = sub_mats(&b22, &t1);
        let t3 = sub_mats(&b22, &b12);
        let t4 = sub_mats(&t2, &b21);

        let next_level = level + 1;
        for (name, block) in [
            ("S1", &s1),
            ("S2", &s2),
            ("S3", &s3),
            ("S4", &s4),
            ("T1", &t1),
            ("T2", &t2),
            ("T3", &t3),
            ("T4", &t4),
        ] {
            assert!(
                canonical_values_respect_bound(block, next_level, k_top, p_minus_1, |f| {
                    f.value() as u128
                }),
                "block {} at level {} fails theorem-4 bound",
                name,
                next_level
            );
        }
        let m1 = verify_bound_recursive(&a11, &b11, threshold, next_level, k_top, p_minus_1);
        let m2 = verify_bound_recursive(&a12, &b21, threshold, next_level, k_top, p_minus_1);
        let m3 = verify_bound_recursive(&s4, &b22, threshold, next_level, k_top, p_minus_1);
        let m4 = verify_bound_recursive(&a22, &t4, threshold, next_level, k_top, p_minus_1);
        let m5 = verify_bound_recursive(&s1, &t1, threshold, next_level, k_top, p_minus_1);
        let m6 = verify_bound_recursive(&s2, &t2, threshold, next_level, k_top, p_minus_1);
        let m7 = verify_bound_recursive(&s3, &t3, threshold, next_level, k_top, p_minus_1);
        let c11 = add_mats(&m1, &m2);
        let u2 = add_mats(&m1, &m6);
        let u3 = add_mats(&u2, &m7);
        let u4 = add_mats(&u2, &m5);
        let c12 = add_mats(&u4, &m3);
        let c21 = sub_mats(&u3, &m4);
        let c22 = add_mats(&u3, &m5);
        let c_padded = assemble_quarters(&c11, &c12, &c21, &c22, &zero);
        if (m_even, n_even) == (m, n) {
            c_padded
        } else {
            slice_to(&c_padded, m, n)
        }
    }

    /// Unreduced `i128` shadow of a matrix: the S/T/U assembly propagates
    /// the integer values without `mod p`, so magnitudes grow with each
    /// peel where canonical values stay at most `p − 1`.
    #[derive(Clone)]
    struct I128Mat {
        rows: usize,
        cols: usize,
        data: Vec<i128>,
    }
    impl I128Mat {
        fn zeros(rows: usize, cols: usize) -> Self {
            Self {
                rows,
                cols,
                data: vec![0i128; rows * cols],
            }
        }
        fn from_fp(src: &FieldMatrix<Fp<MERSENNE_31>>) -> Self {
            let (rows, cols) = src.shape();
            let mut out = Self::zeros(rows, cols);
            for r in 0..rows {
                for c in 0..cols {
                    out.data[r * cols + c] = src.get_unchecked(r, c).value() as i128;
                }
            }
            out
        }
        fn pad_to(&self, rows: usize, cols: usize) -> Self {
            debug_assert!(self.rows <= rows && self.cols <= cols);
            let mut out = Self::zeros(rows, cols);
            for r in 0..self.rows {
                for c in 0..self.cols {
                    out.data[r * cols + c] = self.data[r * self.cols + c];
                }
            }
            out
        }
        fn submatrix(&self, row_off: usize, col_off: usize, rows: usize, cols: usize) -> Self {
            let mut out = Self::zeros(rows, cols);
            for r in 0..rows {
                for c in 0..cols {
                    out.data[r * cols + c] = self.data[(row_off + r) * self.cols + (col_off + c)];
                }
            }
            out
        }
        fn add(&self, other: &Self) -> Self {
            debug_assert_eq!((self.rows, self.cols), (other.rows, other.cols));
            let mut out = Self::zeros(self.rows, self.cols);
            for i in 0..self.data.len() {
                out.data[i] = self.data[i].saturating_add(other.data[i]);
            }
            out
        }
        fn sub(&self, other: &Self) -> Self {
            debug_assert_eq!((self.rows, self.cols), (other.rows, other.cols));
            let mut out = Self::zeros(self.rows, self.cols);
            for i in 0..self.data.len() {
                out.data[i] = self.data[i].saturating_sub(other.data[i]);
            }
            out
        }
        fn max_abs(&self) -> u128 {
            let mut m: u128 = 0;
            for &v in &self.data {
                let a = v.unsigned_abs();
                if a > m {
                    m = a;
                }
            }
            m
        }
    }

    /// Asserts the theorem-4 bound on every unreduced intermediate of the
    /// Winograd recursion and returns the padded output shadow.
    fn verify_wide_shadow_recursive(
        a_shadow: &I128Mat,
        b_shadow: &I128Mat,
        threshold: usize,
        level: u32,
        k_top: usize,
        p_minus_1: u128,
    ) -> I128Mat {
        let (m, k) = (a_shadow.rows, a_shadow.cols);
        let n = b_shadow.cols;
        debug_assert_eq!(b_shadow.rows, k);
        if m == 0 || k == 0 || n == 0 {
            return I128Mat::zeros(m, n);
        }
        if winograd_takes_base_case(threshold, m, k, n) {
            let bound = theorem_4_bound(level, k_top, p_minus_1);
            assert!(
                a_shadow.max_abs() <= bound,
                "wide-shadow level {} A.max_abs = {} > bound {}",
                level,
                a_shadow.max_abs(),
                bound,
            );
            assert!(
                b_shadow.max_abs() <= bound,
                "wide-shadow level {} B.max_abs = {} > bound {}",
                level,
                b_shadow.max_abs(),
                bound,
            );
            let mut out = I128Mat::zeros(m, n);
            for i in 0..m {
                for j in 0..n {
                    let mut acc: i128 = 0;
                    for t in 0..k {
                        let av = a_shadow.data[i * k + t];
                        let bv = b_shadow.data[t * n + j];
                        acc = acc.saturating_add(av.saturating_mul(bv));
                    }
                    out.data[i * n + j] = acc;
                }
            }
            // The product is an `M_i` of the caller, bounded at this
            // call's level.
            let bound = theorem_4_bound(level, k_top, p_minus_1);
            assert!(
                out.max_abs() <= bound,
                "wide-shadow level {} base-case product max_abs = {} > theorem_4_bound = {}",
                level,
                out.max_abs(),
                bound,
            );
            return out;
        }
        let m_even = m + (m & 1);
        let k_even = k + (k & 1);
        let n_even = n + (n & 1);
        let a_p = a_shadow.pad_to(m_even, k_even);
        let b_p = b_shadow.pad_to(k_even, n_even);
        let mh = m_even / 2;
        let kh = k_even / 2;
        let nh = n_even / 2;
        let a11 = a_p.submatrix(0, 0, mh, kh);
        let a12 = a_p.submatrix(0, kh, mh, kh);
        let a21 = a_p.submatrix(mh, 0, mh, kh);
        let a22 = a_p.submatrix(mh, kh, mh, kh);
        let b11 = b_p.submatrix(0, 0, kh, nh);
        let b12 = b_p.submatrix(0, nh, kh, nh);
        let b21 = b_p.submatrix(kh, 0, kh, nh);
        let b22 = b_p.submatrix(kh, nh, kh, nh);
        let s1 = a21.add(&a22);
        let s2 = s1.sub(&a11);
        let s3 = a11.sub(&a21);
        let s4 = a12.sub(&s2);
        let t1 = b12.sub(&b11);
        let t2 = b22.sub(&t1);
        let t3 = b22.sub(&b12);
        let t4 = t2.sub(&b21);
        let next_level = level + 1;
        let bound_next = theorem_4_bound(next_level, k_top, p_minus_1);
        for (name, blk) in [
            ("S1", &s1),
            ("S2", &s2),
            ("S3", &s3),
            ("S4", &s4),
            ("T1", &t1),
            ("T2", &t2),
            ("T3", &t3),
            ("T4", &t4),
        ] {
            let observed = blk.max_abs();
            assert!(
                observed <= bound_next,
                "wide-shadow level {} block {} observed {} > theorem_4_bound = {}",
                next_level,
                name,
                observed,
                bound_next
            );
        }
        let m1 = verify_wide_shadow_recursive(&a11, &b11, threshold, next_level, k_top, p_minus_1);
        let m2 = verify_wide_shadow_recursive(&a12, &b21, threshold, next_level, k_top, p_minus_1);
        let m3 = verify_wide_shadow_recursive(&s4, &b22, threshold, next_level, k_top, p_minus_1);
        let m4 = verify_wide_shadow_recursive(&a22, &t4, threshold, next_level, k_top, p_minus_1);
        let m5 = verify_wide_shadow_recursive(&s1, &t1, threshold, next_level, k_top, p_minus_1);
        let m6 = verify_wide_shadow_recursive(&s2, &t2, threshold, next_level, k_top, p_minus_1);
        let m7 = verify_wide_shadow_recursive(&s3, &t3, threshold, next_level, k_top, p_minus_1);
        // At the outermost level the base case may not be reached, so the
        // `M_i` are checked here as well.
        for (name, blk) in [
            ("M1", &m1),
            ("M2", &m2),
            ("M3", &m3),
            ("M4", &m4),
            ("M5", &m5),
            ("M6", &m6),
            ("M7", &m7),
        ] {
            let observed = blk.max_abs();
            assert!(
                observed <= bound_next,
                "wide-shadow level {} product {} observed {} > theorem_4_bound = {}",
                next_level,
                name,
                observed,
                bound_next
            );
        }
        let c11 = m1.add(&m2);
        let u2 = m1.add(&m6);
        let u3 = u2.add(&m7);
        let u4 = u2.add(&m5);
        let c12 = u4.add(&m3);
        let c21 = u3.sub(&m4);
        let c22 = u3.add(&m5);
        // U_i and the quarters C_ij are bounded at this level.
        let bound_here = theorem_4_bound(level, k_top, p_minus_1);
        for (name, blk) in [
            ("U2", &u2),
            ("U3", &u3),
            ("U4", &u4),
            ("C11", &c11),
            ("C12", &c12),
            ("C21", &c21),
            ("C22", &c22),
        ] {
            let observed = blk.max_abs();
            assert!(
                observed <= bound_here,
                "wide-shadow level {} assembly {} observed {} > theorem_4_bound = {}",
                level,
                name,
                observed,
                bound_here
            );
        }
        let mut c = I128Mat::zeros(2 * mh, 2 * nh);
        for r in 0..mh {
            for ccol in 0..nh {
                c.data[r * (2 * nh) + ccol] = c11.data[r * nh + ccol];
                c.data[r * (2 * nh) + nh + ccol] = c12.data[r * nh + ccol];
                c.data[(mh + r) * (2 * nh) + ccol] = c21.data[r * nh + ccol];
                c.data[(mh + r) * (2 * nh) + nh + ccol] = c22.data[r * nh + ccol];
            }
        }
        c
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(4))]

        /// `n = 4 · threshold` gives at least two recursion levels.
        #[test]
        #[ignore = "slow: proptest Winograd bound propagation; n=4*TEST_THRESHOLD recursion exceeds 5s"]
        fn prop_winograd_bound_propagates_across_levels_fp31(
            seed in 0u64..256,
        ) {
            let threshold_small = 4usize; // force ≥ 2 levels at small n
            let n = 4 * threshold_small; // = 16; guarantees 2+ levels
            proptest::prop_assume!(n >= 4 * threshold_small);
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let b = random_fp::<MERSENNE_31>(n, n, seed.wrapping_add(1));
            let p_minus_1 = (MERSENNE_31 - 1) as u128;
            let expected = gemm(&a, &b);
            let got = verify_bound_recursive(&a, &b, threshold_small, 0, n, p_minus_1);
            proptest::prop_assert_eq!(got.clone(), expected);
            let n_prod = 4 * TEST_THRESHOLD;
            let a_prod = random_fp::<MERSENNE_31>(n_prod, n_prod, seed ^ 0xABCD);
            let b_prod = random_fp::<MERSENNE_31>(n_prod, n_prod, seed ^ 0x1234);
            let expected_prod = gemm(&a_prod, &b_prod);
            let got_prod = gemm_winograd(&a_prod, &b_prod);
            proptest::prop_assert_eq!(got_prod, expected_prod);
        }

        #[test]
        fn prop_winograd_matches_classical_fp7(
            m in 1usize..6,
            k in 1usize..6,
            n in 1usize..6,
            seed_a in 0u64..1024,
            seed_b in 0u64..1024,
        ) {
            let a = random_fp::<7>(m, k, seed_a);
            let b = random_fp::<7>(k, n, seed_b);
            let got = gemm_winograd(&a, &b);
            let expected = gemm(&a, &b);
            proptest::prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_winograd_wide_shadow_respects_theorem_4_bound_fp31(
            seed in 0u64..256,
        ) {
            let threshold_small = 4usize; // force ≥ 2 peels at n = 16
            let n = 4 * threshold_small; // = 16
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let b = random_fp::<MERSENNE_31>(n, n, seed.wrapping_add(7));
            let p_minus_1 = (MERSENNE_31 - 1) as u128;
            let a_sh = I128Mat::from_fp(&a);
            let b_sh = I128Mat::from_fp(&b);
            let _out_shadow = verify_wide_shadow_recursive(
                &a_sh,
                &b_sh,
                threshold_small,
                0,
                n,
                p_minus_1,
            );
            let expected = gemm(&a, &b);
            let got = gemm_winograd_with_threshold(&a, &b, threshold_small);
            proptest::prop_assert_eq!(got, expected);
        }
    }

    #[test]
    fn test_explicit_threshold_bit_exact_fp7() {
        // Thresholds below 2 are floored at 2; `usize::MAX` forces the
        // classical path.
        let a = random_fp::<7>(6, 6, 0x41);
        let b = random_fp::<7>(6, 6, 0x42);
        let expected = gemm(&a, &b);
        for threshold in [1usize, 2, 3, 4, 5, 6, 7, 8, usize::MAX] {
            let got = gemm_winograd_with_threshold(&a, &b, threshold);
            assert_eq!(got, expected, "threshold = {}", threshold);
        }
    }
}
