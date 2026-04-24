//! Strassen–Winograd recursive matrix multiplication over a
//! [`FiniteField`](crate::field::FiniteField).
//!
//! This module implements the sub-cubic Strassen–Winograd variant described
//! in Dumas–Pernet §1.4 (algorithm 1.6): 7 recursive half-size multiplies
//! and 15 block additions per recursion level, giving an asymptotic
//! `O(n^log₂ 7) ≈ O(n^2.807)` complexity versus the classical `O(n³)` gemm
//! shipped in [`crate::field::matrix::gemm`] (T1, issue `91c06222`).
//!
//! The public entry point is [`gemm_winograd`]. Below a configurable
//! threshold [`WINO_THRESHOLD`] the recursion peels down to T1's classical
//! blocked gemm, which inherits the crate's SIMD path via
//! `FieldVec::dot_product_slices`. Odd dimensions are handled by padding a
//! single row/column of zero field elements, recursing, then slicing the
//! result back to the original output shape — zero-padding is admissible
//! over any field because `0 · anything = 0`.
//!
//! # Bound propagation
//!
//! Dumas–Pernet theorem 4 states that after `l` levels of recursion every
//! intermediate cell value `z` satisfies
//!
//! ```text
//! |z| ≤ ((1 + 3^l) / 2)² · ceil(k / 2^l) · (p − 1)²
//! ```
//!
//! Before a sub-problem at depth `l` is handed to the base-case gemm, the
//! implementation compares this bound with `F::max_unreduced_additions()`
//! and refuses to recurse deeper if the classical gemm's own delayed-
//! reduction budget would be exceeded. For Mersenne-31
//! (`F::max_unreduced_additions()` ≈ 4·10⁹) the bound is generous enough
//! that the threshold is the binding constraint at practical matrix
//! sizes; for small prime fields near `2^63` it can fire, at which point
//! we fall back to the classical base case.
//!
//! # Bit-exact correctness
//!
//! Because every intermediate is reduced lazily through
//! `F::reduce_wide` at the base case and Winograd's U-assembly uses only
//! field addition / subtraction (exact over any field), the output is
//! **bit-exact** equal to the classical `gemm` for all shapes, all fields,
//! and every intermediate padding configuration. The module-level tests
//! exercise this for odd-`m`, odd-`k`, odd-`n`, all-three-odd, and
//! threshold-straddling cases.
//!
//! # Odd dimensions
//!
//! The 7-multiply split requires `m`, `k`, and `n` all even. For a peeled
//! subproblem with any odd dimension we pad out the short axis to the
//! next even value with zero-valued field elements, recurse, then slice
//! the padded output back to the original shape. The padding adds at
//! most one extra row/column per level, so the asymptotic cost is still
//! `O(n^log₂ 7)`.
//!
//! # Non-`ConstField` fields
//!
//! The zero-padding step needs a concrete zero-valued `F` to clone. For
//! [`ConstField`](crate::field::ConstField) implementations we use
//! `F::zero_hint()`. For runtime-context fields (`Gf2mElement`) the
//! caller must pass matrices with at least one non-empty factor; if both
//! factors are empty we fall back to `F::zero_hint()`, which returns
//! `None`, and panic with the same contract as
//! [`crate::field::matrix::gemm`].

use crate::field::matrix::{gemm, FieldMatrix};
use crate::field::{FieldVec, FiniteField};

/// Square-matrix size threshold below which [`gemm_winograd`] dispatches
/// directly to the classical blocked [`gemm`] rather than peeling a
/// Winograd layer. Above the threshold the recursion fires; below it the
/// per-layer overhead (seven half-size multiplies + fifteen block adds
/// + padding slices) is larger than the classical `O(n³)` work.
///
/// The chosen value (128) is empirically tuned in
/// `benches/strassen_threshold.rs` against
/// `Fp<2^31 - 1>` (Mersenne-31) at `n ∈ {2048, 4096}`. See
/// `benches/strassen_threshold_results.md` for the sweep data. For
/// `Gf2mWide<1, Gf2m8>` the scalar MAC is much cheaper and the classical
/// gemm wins at all practical sizes; the same single threshold still
/// routes through the classical path in that regime because 128 × 128 is
/// below the measured cross-over on GF(2⁸).
///
/// This knob is soft — correctness is independent of it. Retuning it is
/// a wording change, not an API change.
pub const WINO_THRESHOLD: usize = 128;

/// Strassen–Winograd matrix multiplication over an arbitrary
/// [`FiniteField`](crate::field::FiniteField).
///
/// Below [`WINO_THRESHOLD`] the implementation dispatches directly to the
/// classical blocked [`gemm`] — that path already carries T1's cache
/// tiling and SIMD-accelerated dot products. Above the threshold one
/// level of Winograd's 7-multiply split is peeled and the seven half-size
/// products are computed by recursive calls into this same function.
///
/// Odd dimensions are handled by padding the short axis up to the next
/// even value with zero-valued field elements, recursing, then slicing
/// the padded output back to the original shape. Padding is admissible
/// over any field because `0 · anything = 0`.
///
/// The result is **bit-exact equal** to [`gemm`] for all shapes and all
/// fields (see the module-level proptests in this file).
///
/// # Arguments
///
/// * `a` — Left operand of shape `m × k`. Its column count must equal
///   `b.rows`.
/// * `b` — Right operand of shape `k × n`. Its row count must equal
///   `a.cols`.
///
/// The output has shape `m × n` with cell `(i, j) = ∑_{t=0}^{k-1}
/// a[i, t] · b[t, j]`.
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
/// Asymptotic `O(n^log₂ 7) ≈ O(n^2.807)` field multiplications above the
/// threshold, dropping to the classical `O(n³)` at the base case. Each
/// recursion level allocates `O(n² / 4)` scratch `FieldMatrix` values
/// for the seven half-size sums; the total allocation footprint across
/// the recursion tree is `O(n²)` when summed as a geometric series.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::field::winograd::gemm_winograd;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(1), Fp::<7>::new(2)].into_iter().collect(),
///     vec![Fp::<7>::new(3), Fp::<7>::new(4)].into_iter().collect(),
/// ]);
/// let b = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(5), Fp::<7>::new(6)].into_iter().collect(),
///     vec![Fp::<7>::new(7), Fp::<7>::new(8)].into_iter().collect(),
/// ]);
///
/// let expected = gemm(&a, &b);
/// let got = gemm_winograd(&a, &b);
/// assert_eq!(got, expected);
/// ```
pub fn gemm_winograd<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    assert_eq!(
        a.cols(),
        b.rows(),
        "gemm_winograd: inner dimensions must match ({} vs {})",
        a.cols(),
        b.rows()
    );

    let (m, k) = a.shape();
    let n = b.cols();

    // Delegate degenerate cases to `gemm`: it already handles the
    // zero-witness dance and emits a `FieldMatrix` with the correct
    // storage length for `m × n` shapes where the inner dim is zero.
    if m == 0 || k == 0 || n == 0 {
        return gemm(a, b);
    }

    // Below the threshold the classical path is strictly faster. Dispatch
    // immediately — the `gemm` call inherits SIMD and the T1 tiling.
    if m.min(k).min(n) < WINO_THRESHOLD {
        return gemm(a, b);
    }

    // Theorem-4 sanity check: refuse to recurse if the bound would exceed
    // the field's delayed-reduction budget. At depth `l = 1` (a single
    // Winograd peel) the bound is
    //   |z| ≤ 4 · ceil(k / 2) · (p - 1)²
    // and the base-case gemm needs the inner dim to fit under
    //   F::max_unreduced_additions() * (p - 1)² ≥ |z|.
    // Concretely: we require the ceil(k/2) half-dimension of the
    // sub-problems to fit under `F::max_unreduced_additions() / 4`. If
    // that is false the classical `gemm` is safer because its own
    // chunked accumulator already chops the inner dim at `kmax`.
    let kmax = F::max_unreduced_additions();
    if kmax != usize::MAX {
        let half_k = k.div_ceil(2);
        // Each half-size product's bound factor at one recursion level is
        // 4 · ceil(k/2) · (p-1)². The wide accumulator budgets
        // `kmax · (p-1)²` per single reduction. So we need `4 · half_k ≤
        // kmax` to be safe for one Winograd level at the base.
        if half_k > kmax / 4 {
            return gemm(a, b);
        }
    }

    // Sourcing a zero element. Every non-degenerate path lands here;
    // degenerate (zero-dim) paths are already delegated above.
    debug_assert!(!a.is_empty() && !b.is_empty());
    let zero: F = a.get(0, 0).zero_like();

    // Pad odd dimensions up to the next even value. We pad to the nearest
    // even number above — at most one extra row and/or column per axis,
    // per level.
    let m_even = m + (m & 1);
    let k_even = k + (k & 1);
    let n_even = n + (n & 1);

    let a_padded = pad_to(a, m_even, k_even, &zero);
    let b_padded = pad_to(b, k_even, n_even, &zero);

    // Recurse on the padded even-dim problem.
    let c_padded = winograd_step(&a_padded, &b_padded, &zero);

    // Slice the padded `c_padded` back to the original `m × n` shape.
    if (m_even, n_even) == (m, n) {
        c_padded
    } else {
        slice_to(&c_padded, m, n)
    }
}

/// One level of Winograd peel followed by seven recursive multiplies and
/// the U-assembly. Expects both inputs to have all-even dimensions. The
/// `zero` argument seeds output-allocation calls that cannot source a
/// witness from the operands (the sub-matrix shapes are by construction
/// non-empty once the outer dimensions were even).
fn winograd_step<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    zero: &F,
) -> FieldMatrix<F> {
    let (m, k) = a.shape();
    let n = b.cols();
    debug_assert_eq!(a.cols(), b.rows());
    debug_assert!(m % 2 == 0 && k % 2 == 0 && n % 2 == 0);

    let mh = m / 2;
    let kh = k / 2;
    let nh = n / 2;

    // Extract the four quarter blocks of A and B. Each is a freshly
    // allocated row-major `FieldMatrix` so the recursive calls stay on the
    // crate's main contiguous-storage path (no view recursion).
    let a11 = submatrix(a, 0, 0, mh, kh, zero);
    let a12 = submatrix(a, 0, kh, mh, kh, zero);
    let a21 = submatrix(a, mh, 0, mh, kh, zero);
    let a22 = submatrix(a, mh, kh, mh, kh, zero);

    let b11 = submatrix(b, 0, 0, kh, nh, zero);
    let b12 = submatrix(b, 0, nh, kh, nh, zero);
    let b21 = submatrix(b, kh, 0, kh, nh, zero);
    let b22 = submatrix(b, kh, nh, kh, nh, zero);

    // Dumas–Pernet §1.4 algorithm 1.6 — S-T-M-U assembly.
    //
    // S1 = A21 + A22
    // S2 = S1 − A11
    // S3 = A11 − A21
    // S4 = A12 − S2
    let s1 = add_mats(&a21, &a22);
    let s2 = sub_mats(&s1, &a11);
    let s3 = sub_mats(&a11, &a21);
    let s4 = sub_mats(&a12, &s2);

    // T1 = B12 − B11
    // T2 = B22 − T1
    // T3 = B22 − B12
    // T4 = T2 − B21
    let t1 = sub_mats(&b12, &b11);
    let t2 = sub_mats(&b22, &t1);
    let t3 = sub_mats(&b22, &b12);
    let t4 = sub_mats(&t2, &b21);

    // Seven recursive multiplies. Each call re-enters `gemm_winograd` so
    // the bound / threshold gates apply at every level.
    let m1 = gemm_winograd(&a11, &b11);
    let m2 = gemm_winograd(&a12, &b21);
    let m3 = gemm_winograd(&s4, &b22);
    let m4 = gemm_winograd(&a22, &t4);
    let m5 = gemm_winograd(&s1, &t1);
    let m6 = gemm_winograd(&s2, &t2);
    let m7 = gemm_winograd(&s3, &t3);

    // U-assembly (DP §1.4):
    //   C11 = M1 + M2
    //   U2  = M1 + M6
    //   U3  = U2 + M7        →  C21 = U3 − M4   and   C22 = U3 + M5
    //   U4  = U2 + M5        →  C12 = U4 + M3
    let c11 = add_mats(&m1, &m2);
    let u2 = add_mats(&m1, &m6);
    let u3 = add_mats(&u2, &m7);
    let u4 = add_mats(&u2, &m5);
    let c12 = add_mats(&u4, &m3);
    let c21 = sub_mats(&u3, &m4);
    let c22 = add_mats(&u3, &m5);

    // Stitch the four output quarters back into an `m × n` matrix.
    assemble_quarters(&c11, &c12, &c21, &c22, zero)
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
/// offset. Used to materialise the four quarters of A and B before the
/// recursive multiplies. (Submatrix views would save the allocation but
/// the recursive `gemm_winograd` works on owned row-major storage, so
/// materialising once up front keeps the base-case gemm on its hot path.)
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
/// (2·nh)` matrix. Called at each Winograd level to re-assemble the
/// output.
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

/// Elementwise `A + B` producing a fresh matrix. Used for the S/T/U
/// block adds in Winograd's peel.
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

/// Elementwise `A − B` producing a fresh matrix. Used for the S/T/U
/// block subs in Winograd's peel.
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

/// Dumas–Pernet theorem 4 bound. After `levels` recursion levels every
/// intermediate cell value satisfies
/// `|z| ≤ ((1 + 3^levels) / 2)² · ceil(k / 2^levels) · (p − 1)²`.
///
/// Returned as a `u128` so callers over prime fields with `p < 2^32` can
/// cross-check the observed cell value against the bound without
/// overflow. For characteristic-2 fields the theorem is vacuous (the
/// XOR accumulator never wraps); this helper then returns `u128::MAX`.
///
/// # Arguments
///
/// * `levels` — Depth of the Winograd recursion so far (0 = pure
///   classical gemm at the base).
/// * `k` — Inner matrix dimension at the top of the recursion.
/// * `p_minus_1` — Field characteristic bound per operand cell. Pass
///   `p - 1` for `Fp<P>`; pass `u128::MAX` / call sites should skip the
///   bound entirely for binary fields.
///
/// # Examples
///
/// ```
/// use gf2_core::field::winograd::theorem_4_bound;
///
/// // Classical gemm over `Fp<7>`, k = 4.
/// let b0 = theorem_4_bound(0, 4, 6);
/// assert_eq!(b0, 4 * 6 * 6); // (1·6)² implied here: (1+3^0)/2 = 1
///
/// // One Winograd level over `Fp<7>`, k = 4.
/// let b1 = theorem_4_bound(1, 4, 6);
/// assert_eq!(b1, 4 * 2 * 6 * 6); // 2² · ceil(4/2) · 36
/// ```
///
/// # Complexity
///
/// O(levels) integer multiplies.
pub fn theorem_4_bound(levels: u32, k: usize, p_minus_1: u128) -> u128 {
    if p_minus_1 == 0 {
        return 0;
    }
    let three_pow_l: u128 = 3u128.pow(levels);
    let one_plus = 1u128 + three_pow_l;
    // ((1 + 3^l) / 2)² — divisible since 1 + 3^l is always even.
    debug_assert!(one_plus % 2 == 0);
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

    // Test config: GF(2^8) with AES irreducible via `Gf2mWide<1>`.
    struct WinoGf2m8Cfg;
    impl Gf2mWideConfig<1> for WinoGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "WinoGf2m8Cfg";
    }
    type WinoGf2m8 = Gf2mWide<1, WinoGf2m8Cfg>;

    const MERSENNE_31: u64 = 2_147_483_647;

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

    // ─── Bit-exactness below / at / above the threshold ──────────────────

    #[test]
    fn test_winograd_below_threshold_fp_small() {
        // Dimensions well below `WINO_THRESHOLD` → should reduce to `gemm`.
        let a = random_fp::<7>(10, 12, 0x01);
        let b = random_fp::<7>(12, 8, 0x02);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_at_threshold_fp() {
        let n = WINO_THRESHOLD;
        let a = random_fp::<65_521>(n, n, 0xA1);
        let b = random_fp::<65_521>(n, n, 0xA2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_above_threshold_fp() {
        // Just above → one Winograd peel.
        let n = WINO_THRESHOLD + 2;
        let a = random_fp::<65_521>(n, n, 0xB1);
        let b = random_fp::<65_521>(n, n, 0xB2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_one_below_threshold_fp() {
        let n = WINO_THRESHOLD - 1;
        let a = random_fp::<65_521>(n, n, 0xC1);
        let b = random_fp::<65_521>(n, n, 0xC2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    // ─── Odd-dimension combinations ───────────────────────────────────────

    #[test]
    fn test_winograd_odd_m_fp() {
        let n = WINO_THRESHOLD + 1; // odd m
        let k = WINO_THRESHOLD + 4;
        let nn = WINO_THRESHOLD + 4;
        let a = random_fp::<65_521>(n, k, 0xD1);
        let b = random_fp::<65_521>(k, nn, 0xD2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd m");
    }

    #[test]
    fn test_winograd_odd_k_fp() {
        let m = WINO_THRESHOLD + 4;
        let k = WINO_THRESHOLD + 1; // odd k
        let n = WINO_THRESHOLD + 4;
        let a = random_fp::<65_521>(m, k, 0xE1);
        let b = random_fp::<65_521>(k, n, 0xE2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd k");
    }

    #[test]
    fn test_winograd_odd_n_fp() {
        let m = WINO_THRESHOLD + 4;
        let k = WINO_THRESHOLD + 4;
        let n = WINO_THRESHOLD + 1; // odd n
        let a = random_fp::<65_521>(m, k, 0xF1);
        let b = random_fp::<65_521>(k, n, 0xF2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd n");
    }

    #[test]
    fn test_winograd_all_odd_fp() {
        let m = WINO_THRESHOLD + 1;
        let k = WINO_THRESHOLD + 3;
        let n = WINO_THRESHOLD + 5;
        let a = random_fp::<65_521>(m, k, 0x11);
        let b = random_fp::<65_521>(k, n, 0x12);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "all three odd");
    }

    #[test]
    fn test_winograd_all_odd_gf2m8() {
        // Binary field with the same odd-dim combinations.
        let m = WINO_THRESHOLD + 1;
        let k = WINO_THRESHOLD + 3;
        let n = WINO_THRESHOLD + 5;
        let a = random_gf2m8(m, k, 0x21);
        let b = random_gf2m8(k, n, 0x22);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "gf2m8 all odd");
    }

    // ─── Degenerate shapes: empty rows / cols / inner dim ────────────────

    #[test]
    fn test_winograd_empty_outer() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 5);
        let b = FieldMatrix::<Fp<7>>::zeros(5, 3);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got.shape(), (0, 3));
    }

    #[test]
    fn test_winograd_empty_inner_const_field() {
        // (m, 0) × (0, n) on a const field — zero-output, non-zero shape.
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

    // ─── Padding round-trip: slice-back sanity ───────────────────────────

    #[test]
    fn test_pad_slice_roundtrip_preserves_values() {
        let n = WINO_THRESHOLD + 3;
        let a = random_fp::<65_521>(n, n, 0x31);
        let padded = pad_to(&a, n + 1, n + 1, &Fp::<65_521>::new(0));
        assert_eq!(padded.shape(), (n + 1, n + 1));
        // Original region is preserved bit-exactly.
        for r in 0..n {
            for c in 0..n {
                assert_eq!(padded.get(r, c), a.get(r, c), "({}, {})", r, c);
            }
        }
        // Padded region is zero.
        for r in 0..n {
            assert_eq!(padded.get(r, n), Fp::<65_521>::new(0));
        }
        for c in 0..=n {
            assert_eq!(padded.get(n, c), Fp::<65_521>::new(0));
        }
        // Slicing back gives exactly `a`.
        let sliced = slice_to(&padded, n, n);
        assert_eq!(sliced, a);
    }

    // ─── theorem_4_bound helper ──────────────────────────────────────────

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
    fn test_theorem_4_bound_zero_field() {
        // Degenerate p = 1 → bound is 0.
        assert_eq!(theorem_4_bound(3, 16, 0), 0);
    }

    // ─── Bound-propagation proptest ──────────────────────────────────────

    // For a random Winograd call, every cell of the result must satisfy
    // the theorem-4 bound at the level reached. Over Mersenne-31 with
    // `k = n = WINO_THRESHOLD + 4` we reach exactly 1 recursion level,
    // so the per-cell canonical value (already in `[0, p)`) is trivially
    // bounded by `p - 1`, which is far less than `theorem_4_bound(1,
    // k, p-1)`. The invariant is vacuously held by every
    // canonical-valued output cell; the proptest asserts it explicitly
    // to anchor the regression gate.
    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(4))]

        #[test]
        fn prop_winograd_output_respects_theorem_4_bound_fp31(
            seed in 0u64..256,
        ) {
            let n = WINO_THRESHOLD + 4;
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let b = random_fp::<MERSENNE_31>(n, n, seed.wrapping_add(1));
            let got = gemm_winograd(&a, &b);
            let bound = theorem_4_bound(1, n, (MERSENNE_31 - 1) as u128);
            for r in 0..n {
                for c in 0..n {
                    let v = got.get(r, c).value() as u128;
                    // Canonical value is in [0, p), always strictly less
                    // than the level-1 bound for any k ≥ 1.
                    proptest::prop_assert!(v <= bound, "cell ({}, {}) = {} exceeds theorem-4 bound {}", r, c, v, bound);
                }
            }
            // Sanity: Winograd matches classical bit-exactly.
            let expected = gemm(&a, &b);
            proptest::prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_winograd_matches_classical_fp7(
            m in 1usize..6,
            k in 1usize..6,
            n in 1usize..6,
            seed_a in 0u64..1024,
            seed_b in 0u64..1024,
        ) {
            // Below threshold: Winograd must dispatch to `gemm` and
            // therefore match it bit-exactly regardless of input shape.
            let a = random_fp::<7>(m, k, seed_a);
            let b = random_fp::<7>(k, n, seed_b);
            let got = gemm_winograd(&a, &b);
            let expected = gemm(&a, &b);
            proptest::prop_assert_eq!(got, expected);
        }
    }
}
