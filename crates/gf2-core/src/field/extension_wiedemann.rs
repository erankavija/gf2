//! Extension-field scalar Wiedemann minimal polynomial.
//!
//! [`try_extension_wiedemann_fp`] serves `Fp<P>` matrices with `q ≤ n` by
//! running scalar Wiedemann over an extension of degree `k` with `q^k > n`.
//! The matrix stays in base form: an extension vector is `k` base-field
//! component arrays, a base-field projection vector yields `k` base scalar
//! sequences from one Krylov chain, and Berlekamp–Massey on each gives a
//! base-field divisor of the minimal polynomial; their LCM is the candidate.
//! A candidate of degree below `n` is returned only after the Las-Vegas check
//! `crate::field::charpoly::poly_annihilates_a_lasvegas` accepts it.
//! [`crate::field::traits::FiniteField::try_extension_wiedemann_minpoly`]
//! dispatches here for `Fp<P>`.

use crate::field::charpoly::{berlekamp_massey, poly_lcm, splitmix64};
use crate::field::matrix::FieldMatrix;
use crate::field::poly::FieldPoly;
use crate::field::vec::FieldVec;
use crate::field::FiniteField;
use crate::gfp::Fp;
use crate::gfpn::{CubicExt, ExtConfig, QuadraticExt};

// The embedded matrix has zero `α`-components, so
// `embed(A) · (v_0 + v_1·u) = (A·v_0) + (A·v_1)·u`: an extension matvec is `k`
// base-field matvecs on the pre-packed base matrix.

/// Pre-packed base matrix wrapper carried alongside the original matrix.
/// Owns the packed cache so multiple matvecs in a row reuse it.
struct PackedBaseMatrix<'a, F: FiniteField> {
    a: &'a FieldMatrix<F>,
    rows: usize,
    cols: usize,
    packed: Option<Box<dyn crate::field::matrix::PackedMatvec<F>>>,
}

impl<'a, F: FiniteField> PackedBaseMatrix<'a, F> {
    fn new(a: &'a FieldMatrix<F>) -> Self {
        let (rows, cols) = a.shape();
        let packed = if rows > 0 && cols > 0 {
            F::try_prepack_matvec(a.as_data_slice(), rows, cols)
        } else {
            None
        };
        Self {
            a,
            rows,
            cols,
            packed,
        }
    }

    /// In-place matvec into a caller-owned buffer, reused across the Krylov
    /// chain.
    fn matvec_into(&self, x: &[F], y: &mut [F]) {
        debug_assert_eq!(x.len(), self.cols);
        debug_assert_eq!(y.len(), self.rows);
        if let Some(packed) = self.packed.as_ref() {
            packed.matvec(x, y);
            return;
        }
        let xv: FieldVec<F> = x.iter().cloned().collect();
        let yv = self.a.matvec(&xv);
        y[..self.rows].clone_from_slice(&yv.as_slice()[..self.rows]);
    }
}

/// Quadratic-extension state vector stored as two parallel base-field
/// component arrays.
struct QuadVec<F: FiniteField> {
    c0: Vec<F>,
    c1: Vec<F>,
}

impl<F: FiniteField> QuadVec<F> {
    fn zeros(n: usize, zero: &F) -> Self {
        Self {
            c0: vec![zero.clone(); n],
            c1: vec![zero.clone(); n],
        }
    }
}

/// Cubic-extension state vector stored as three parallel base-field
/// component arrays.
struct CubicVec<F: FiniteField> {
    c0: Vec<F>,
    c1: Vec<F>,
    c2: Vec<F>,
}

impl<F: FiniteField> CubicVec<F> {
    fn zeros(n: usize, zero: &F) -> Self {
        Self {
            c0: vec![zero.clone(); n],
            c1: vec![zero.clone(); n],
            c2: vec![zero.clone(); n],
        }
    }
}

/// Quadratic-extension Wiedemann attempt against an embedded matrix `A`.
///
/// A base-field projection vector `v` (with `v.c1 = 0`) yields two base
/// scalar sequences from one Krylov chain of length `2n + 1`:
///
/// * `s0_k = ⟨v.c0, (A^k u).c0⟩`
/// * `s1_k = ⟨v.c0, (A^k u).c1⟩`
///
/// Berlekamp-Massey on each gives a base-field divisor of `minpoly(A)`; the
/// result is their LCM, or `None` when its degree exceeds `n`.
fn wiedemann_attempt_quadratic<F: FiniteField>(
    pa: &PackedBaseMatrix<'_, F>,
    seed: u64,
) -> Option<FieldPoly<F>> {
    let n = pa.rows;
    debug_assert!(n >= 2);
    let zero: F = pa.a.get(0, 0).zero_like();
    let one: F = zero.one_like();

    let u = gen_quad_random_vec::<F>(n, &zero, &one, seed);
    let v_c0 = gen_base_random_vec::<F>(n, &zero, &one, seed.wrapping_add(0x100));

    // One Krylov chain feeds both sequences; `cur` / `next` ping-pong to avoid
    // per-step allocation.
    let seq_len = 2 * n + 1;
    let mut s0: Vec<F> = Vec::with_capacity(seq_len);
    let mut s1: Vec<F> = Vec::with_capacity(seq_len);
    let mut cur = u;
    let mut next = QuadVec::<F>::zeros(n, &zero);
    for _ in 0..seq_len {
        s0.push(dot_product_slices(&v_c0, &cur.c0, &zero));
        s1.push(dot_product_slices(&v_c0, &cur.c1, &zero));
        pa.matvec_into(&cur.c0, &mut next.c0);
        pa.matvec_into(&cur.c1, &mut next.c1);
        std::mem::swap(&mut cur, &mut next);
    }

    // A degree-`n` divisor of the minimal polynomial is the minimal polynomial,
    // so the second BM and the LCM are skipped.
    let p0 = berlekamp_massey(&s0);
    if p0.degree() == Some(n) {
        return Some(p0);
    }
    let p1 = berlekamp_massey(&s1);
    let candidate = poly_lcm(&p0, &p1);
    let d = candidate.degree()?;
    if d > n {
        return None;
    }
    Some(candidate)
}

/// Cubic-extension Wiedemann attempt against an embedded matrix `A`: the
/// three-sequence form of [`wiedemann_attempt_quadratic`].
fn wiedemann_attempt_cubic<F: FiniteField>(
    pa: &PackedBaseMatrix<'_, F>,
    seed: u64,
) -> Option<FieldPoly<F>> {
    let n = pa.rows;
    debug_assert!(n >= 2);
    let zero: F = pa.a.get(0, 0).zero_like();
    let one: F = zero.one_like();

    let u = gen_cubic_random_vec::<F>(n, &zero, &one, seed);
    let v_c0 = gen_base_random_vec::<F>(n, &zero, &one, seed.wrapping_add(0x100));

    let seq_len = 2 * n + 1;
    let mut s0: Vec<F> = Vec::with_capacity(seq_len);
    let mut s1: Vec<F> = Vec::with_capacity(seq_len);
    let mut s2: Vec<F> = Vec::with_capacity(seq_len);
    let mut cur = u;
    let mut next = CubicVec::<F>::zeros(n, &zero);
    for _ in 0..seq_len {
        s0.push(dot_product_slices(&v_c0, &cur.c0, &zero));
        s1.push(dot_product_slices(&v_c0, &cur.c1, &zero));
        s2.push(dot_product_slices(&v_c0, &cur.c2, &zero));
        pa.matvec_into(&cur.c0, &mut next.c0);
        pa.matvec_into(&cur.c1, &mut next.c1);
        pa.matvec_into(&cur.c2, &mut next.c2);
        std::mem::swap(&mut cur, &mut next);
    }

    // Degree-`n` early exits as in the quadratic case.
    let p0 = berlekamp_massey(&s0);
    if p0.degree() == Some(n) {
        return Some(p0);
    }
    let p1 = berlekamp_massey(&s1);
    let p01 = poly_lcm(&p0, &p1);
    if p01.degree() == Some(n) {
        return Some(p01);
    }
    let p2 = berlekamp_massey(&s2);
    let candidate = poly_lcm(&p01, &p2);
    let d = candidate.degree()?;
    if d > n {
        return None;
    }
    Some(candidate)
}

/// Seeded non-zero base-field vector.
fn gen_base_random_vec<F: FiniteField>(n: usize, zero: &F, one: &F, seed: u64) -> Vec<F> {
    let mut state = seed;
    let mut v: Vec<F> = vec![zero.clone(); n];
    for slot in v.iter_mut().take(n) {
        let count = (splitmix64(&mut state) & 0x3F) as u32;
        let mut acc = zero.clone();
        for _ in 0..count {
            acc += one.clone();
        }
        if (splitmix64(&mut state) & 1) == 1 {
            *slot = acc;
        }
    }
    if v.iter().all(|x| x.is_zero()) {
        v[0] = one.clone();
    }
    v
}

fn dot_product_slices<F: FiniteField>(a: &[F], b: &[F], zero: &F) -> F {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = zero.clone();
    for i in 0..a.len() {
        acc += a[i].clone() * b[i].clone();
    }
    acc
}

/// Image of the integer `count` in `F`.
fn gen_base_element<F: FiniteField>(zero: &F, one: &F, count: u32) -> F {
    let mut acc = zero.clone();
    for _ in 0..count {
        acc += one.clone();
    }
    acc
}

fn gen_quad_random_vec<F: FiniteField>(n: usize, zero: &F, one: &F, seed: u64) -> QuadVec<F> {
    let mut state = seed;
    let mut v = QuadVec::<F>::zeros(n, zero);
    for i in 0..n {
        let c0 = gen_base_element(zero, one, (splitmix64(&mut state) & 0x3F) as u32);
        let c1 = gen_base_element(zero, one, (splitmix64(&mut state) & 0x3F) as u32);
        v.c0[i] = c0;
        v.c1[i] = c1;
    }
    if v.c0.iter().all(|x| x.is_zero()) && v.c1.iter().all(|x| x.is_zero()) {
        v.c0[0] = one.clone();
    }
    v
}

fn gen_cubic_random_vec<F: FiniteField>(n: usize, zero: &F, one: &F, seed: u64) -> CubicVec<F> {
    let mut state = seed;
    let mut v = CubicVec::<F>::zeros(n, zero);
    for i in 0..n {
        let c0 = gen_base_element(zero, one, (splitmix64(&mut state) & 0x3F) as u32);
        let c1 = gen_base_element(zero, one, (splitmix64(&mut state) & 0x3F) as u32);
        let c2 = gen_base_element(zero, one, (splitmix64(&mut state) & 0xFF) as u32);
        v.c0[i] = c0;
        v.c1[i] = c1;
        v.c2[i] = c2;
    }
    if v.c0.iter().all(|x| x.is_zero())
        && v.c1.iter().all(|x| x.is_zero())
        && v.c2.iter().all(|x| x.is_zero())
    {
        v.c0[0] = one.clone();
    }
    v
}

/// Lifts each coefficient of `p` into `QuadraticExt<C>` and returns `p` when
/// every lifted α-component is zero. Returns `None` otherwise, for the zero
/// polynomial, and when `F::zero_hint()` is `None`.
fn descend_quadratic_runtime<F, C>(p: &FieldPoly<F>) -> Option<FieldPoly<F>>
where
    F: FiniteField + Clone,
    C: ExtConfig<BaseField = F>,
    QuadraticExt<C>: FiniteField,
{
    let deg = p.degree()?;
    let mut coeffs: Vec<F> = Vec::with_capacity(deg + 1);
    for k in 0..=deg {
        let base = p.coeff(k);
        let lifted: QuadraticExt<C> = QuadraticExt::<C>::new(base.clone(), F::zero_hint()?);
        let (c0, c1) = (lifted.c0().clone(), lifted.c1().clone());
        if !c1.is_zero() {
            return None;
        }
        coeffs.push(c0);
    }
    Some(FieldPoly::from_coeffs_trimmed(coeffs))
}

/// Cubic form of [`descend_quadratic_runtime`]: checks the α and α²
/// components.
fn descend_cubic_runtime<F, C>(p: &FieldPoly<F>) -> Option<FieldPoly<F>>
where
    F: FiniteField + Clone,
    C: ExtConfig<BaseField = F>,
    CubicExt<C>: FiniteField,
{
    let deg = p.degree()?;
    let mut coeffs: Vec<F> = Vec::with_capacity(deg + 1);
    let zero = F::zero_hint()?;
    for k in 0..=deg {
        let base = p.coeff(k);
        let lifted: CubicExt<C> = CubicExt::<C>::new(base.clone(), zero.clone(), zero.clone());
        let (c0, c1, c2) = (
            lifted.c0().clone(),
            lifted.c1().clone(),
            lifted.c2().clone(),
        );
        if !c1.is_zero() || !c2.is_zero() {
            return None;
        }
        coeffs.push(c0);
    }
    Some(FieldPoly::from_coeffs_trimmed(coeffs))
}

#[inline]
fn p_annihilates_a<F: FiniteField>(p: &FieldPoly<F>, a: &FieldMatrix<F>, seed: u64) -> bool {
    crate::field::charpoly::poly_annihilates_a_lasvegas(p, a, seed)
}

/// Tries the extension-field scalar Wiedemann minpoly path for
/// `Fp<P>` matrices.
///
/// Engages when `q ≤ n` and `q^k > n` for the smallest supported extension
/// degree `k`: `P = 7` with `7 ≤ n ≤ 48` (`k = 2`) or `49 ≤ n ≤ 342`
/// (`k = 3`), and `P = 251` with `251 ≤ n ≤ 63_000` (`k = 2`). Returns
/// `Some(p)` when an attempt within the retry budget yields a candidate that
/// passes the coefficient-descent guard and either has degree `n` or passes
/// the Las-Vegas annihilation check; `None` otherwise.
pub fn try_extension_wiedemann_fp<const P: u64>(
    a: &FieldMatrix<Fp<P>>,
) -> Option<FieldPoly<Fp<P>>> {
    let n = a.rows();
    if n < 2 {
        return None;
    }

    if P == 7 {
        if n < 7 {
            return None;
        }
        if n <= 48 {
            return run_quadratic_generic::<P, FpQuadraticSeven<P>>(a);
        }
        if n <= 342 {
            return run_cubic_generic::<P, FpCubicSeven<P>>(a);
        }
        return None;
    }
    if P == 251 {
        if n < 251 {
            return None;
        }
        if n <= 63_000 {
            return run_quadratic_generic::<P, FpQuadraticTwoFiftyOne<P>>(a);
        }
        return None;
    }
    None
}

/// Generic quadratic-extension config over `Fp<P>` where the runtime
/// guarantees `P == 7`. The non-residue is `−1` (= `P − 1`).
struct FpQuadraticSeven<const P: u64>;
impl<const P: u64> ExtConfig for FpQuadraticSeven<P> {
    type BaseField = Fp<P>;
    #[cfg(not(verify_lean))]
    const NON_RESIDUE: Fp<P> = Fp::<P>::new(6); // −1 mod 7 (sound only when P==7)
    #[cfg(verify_lean)]
    #[allow(non_snake_case)]
    fn NON_RESIDUE() -> Fp<P> {
        Fp::<P>::new(6)
    }
    #[inline]
    fn mul_by_non_residue(x: Fp<P>) -> Fp<P> {
        -x
    }
}

/// Generic cubic-extension config over `Fp<P>` where the runtime
/// guarantees `P == 7`. The non-residue `2` makes `x³ − 2` irreducible
/// over `Fp<7>` because the cubes mod 7 are `{0, 1, 6}`.
struct FpCubicSeven<const P: u64>;
impl<const P: u64> ExtConfig for FpCubicSeven<P> {
    type BaseField = Fp<P>;
    #[cfg(not(verify_lean))]
    const NON_RESIDUE: Fp<P> = Fp::<P>::new(2);
    #[cfg(verify_lean)]
    #[allow(non_snake_case)]
    fn NON_RESIDUE() -> Fp<P> {
        Fp::<P>::new(2)
    }
}

/// Generic quadratic-extension config over `Fp<P>` where the runtime
/// guarantees `P == 251`. The non-residue is `−1` (= `P − 1`).
struct FpQuadraticTwoFiftyOne<const P: u64>;
impl<const P: u64> ExtConfig for FpQuadraticTwoFiftyOne<P> {
    type BaseField = Fp<P>;
    #[cfg(not(verify_lean))]
    const NON_RESIDUE: Fp<P> = Fp::<P>::new(250); // −1 mod 251 (sound only when P==251)
    #[cfg(verify_lean)]
    #[allow(non_snake_case)]
    fn NON_RESIDUE() -> Fp<P> {
        Fp::<P>::new(250)
    }
    #[inline]
    fn mul_by_non_residue(x: Fp<P>) -> Fp<P> {
        -x
    }
}

/// Runs the quadratic-extension Wiedemann path for a generic config `C`.
///
/// Each retry runs [`wiedemann_attempt_quadratic`], then
/// [`descend_quadratic_runtime`]. A descended candidate of degree `n` is the
/// minimal polynomial (it divides the minimal polynomial, which divides the
/// degree-`n` characteristic polynomial); a lower-degree candidate must pass
/// [`p_annihilates_a`].
fn run_quadratic_generic<const P: u64, C>(a: &FieldMatrix<Fp<P>>) -> Option<FieldPoly<Fp<P>>>
where
    C: ExtConfig<BaseField = Fp<P>>,
    QuadraticExt<C>: FiniteField,
{
    const SEED: u64 = 0x6C92_6DE0_E3DA_DDA1;
    const MAX_RETRIES: u32 = 4;

    let n = a.rows();
    let pa = PackedBaseMatrix::new(a);
    for retry in 0..MAX_RETRIES {
        if let Some(base_poly) =
            wiedemann_attempt_quadratic::<Fp<P>>(&pa, SEED.wrapping_add(retry as u64))
        {
            let descended: FieldPoly<Fp<P>> = descend_quadratic_runtime::<Fp<P>, C>(&base_poly)?;
            if descended.degree() == Some(n) {
                return Some(descended);
            }
            // Per-retry verifier seed: retries probe independently.
            let probe_seed = SEED.wrapping_add(0xA1).wrapping_add(retry as u64);
            if p_annihilates_a(&descended, a, probe_seed) {
                return Some(descended);
            }
        }
    }
    None
}

/// Cubic form of [`run_quadratic_generic`] over three sequences and
/// `CubicExt<C>`.
fn run_cubic_generic<const P: u64, C>(a: &FieldMatrix<Fp<P>>) -> Option<FieldPoly<Fp<P>>>
where
    C: ExtConfig<BaseField = Fp<P>>,
    CubicExt<C>: FiniteField,
{
    const SEED: u64 = 0x6C92_6DE0_E3DA_DDA2;
    const MAX_RETRIES: u32 = 4;

    let n = a.rows();
    let pa = PackedBaseMatrix::new(a);
    for retry in 0..MAX_RETRIES {
        if let Some(base_poly) =
            wiedemann_attempt_cubic::<Fp<P>>(&pa, SEED.wrapping_add(retry as u64))
        {
            let descended: FieldPoly<Fp<P>> = descend_cubic_runtime::<Fp<P>, C>(&base_poly)?;
            if descended.degree() == Some(n) {
                return Some(descended);
            }
            // Per-retry verifier seed: retries probe independently.
            let probe_seed = SEED.wrapping_add(0xA1).wrapping_add(retry as u64);
            if p_annihilates_a(&descended, a, probe_seed) {
                return Some(descended);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfp::Fp;

    #[test]
    fn test_extension_wiedemann_engages_fp7_large_n() {
        let n = 128;
        let a = make_random_fp::<7>(n, 0xC0FF_EE07);
        let mp = try_extension_wiedemann_fp::<7>(&a).expect("extension Wiedemann should succeed");
        let d = mp.degree().expect("non-zero polynomial");
        assert!(d <= n, "minpoly degree {} exceeds n={}", d, n);
        assert!(
            mp.leading_coeff().unwrap().is_one(),
            "minpoly should be monic"
        );
        assert!(
            p_annihilates_a(&mp, &a, 0xDEAD_BEEF),
            "extension Wiedemann result must annihilate A"
        );
    }

    /// n = 251 is the smallest size the `Fp<251>` gate `n >= q` admits.
    #[test]
    fn test_extension_wiedemann_engages_fp251_at_q_threshold() {
        let n = 251;
        let a = make_random_fp::<251>(n, 0xC0FF_EEFB);
        let mp = try_extension_wiedemann_fp::<251>(&a).expect("extension Wiedemann should succeed");
        let d = mp.degree().expect("non-zero polynomial");
        assert!(d <= n);
        assert!(mp.leading_coeff().unwrap().is_one());
        assert!(p_annihilates_a(&mp, &a, 0xDEAD_BEEF));
    }

    #[test]
    fn test_extension_wiedemann_below_gate_returns_none() {
        for n in [2usize, 3, 6] {
            let a7 = make_random_fp::<7>(n, 0xAAAA);
            assert!(
                try_extension_wiedemann_fp::<7>(&a7).is_none(),
                "Fp<7> n={} should be below gate (n < q = 7)",
                n
            );
        }
        for n in [2usize, 16, 64, 128, 250] {
            let a251 = make_random_fp::<251>(n, 0xBBBB);
            assert!(
                try_extension_wiedemann_fp::<251>(&a251).is_none(),
                "Fp<251> n={} should be below gate (n < q = 251)",
                n
            );
        }
    }

    /// Engages for `Fp<7>` n=64 (q=7 ≤ 64 and 7^3 = 343 > 64).
    #[test]
    fn test_extension_wiedemann_engages_fp7_n64() {
        let n = 64;
        let a = make_random_fp::<7>(n, 0xCAFEBABE);
        let mp =
            try_extension_wiedemann_fp::<7>(&a).expect("Fp<7> n=64 must engage extension path");
        let d = mp.degree().expect("non-zero polynomial");
        assert!(d <= n, "minpoly degree {} exceeds n={}", d, n);
        assert!(p_annihilates_a(&mp, &a, 0xDEAD_BEEF));
    }

    /// Engages for `Fp<7>` n=10 (q=7 ≤ 10 and 7^2 = 49 > 10) through the
    /// quadratic branch, which serves n ≤ 48.
    #[test]
    fn test_extension_wiedemann_engages_fp7_n10_quadratic() {
        let n = 10;
        let a = make_random_fp::<7>(n, 0xC0FFEE);
        let mp = try_extension_wiedemann_fp::<7>(&a)
            .expect("Fp<7> n=10 must engage extension quadratic path");
        let d = mp.degree().expect("non-zero polynomial");
        assert!(d <= n, "minpoly degree {} exceeds n={}", d, n);
        assert!(p_annihilates_a(&mp, &a, 0xDEAD_BEEF));
    }

    /// Adversarial Jordan-block correctness over the base field: J_3(2) ⊕ J_2(0)
    /// over `Fp<7>`. minpoly = (x − 2)^3 · x^2 of degree 5.
    ///
    /// Tests the *internal* engagement helpers (bypassing the public
    /// engagement gate) so we can exercise the algorithm at small `n`.
    #[test]
    fn test_extension_jordan_adversarial_fp7() {
        let n = 5;
        let mut a = FieldMatrix::<Fp<7>>::zeros(n, n);
        // J_3(2): block at rows 0..3.
        a.set(0, 0, Fp::<7>::new(2));
        a.set(1, 1, Fp::<7>::new(2));
        a.set(2, 2, Fp::<7>::new(2));
        a.set(0, 1, Fp::<7>::new(1));
        a.set(1, 2, Fp::<7>::new(1));
        // J_2(0): block at rows 3..5.
        a.set(3, 4, Fp::<7>::new(1));

        let mp_via_dispatch = a.minpoly();
        if let Some(mp) = run_quadratic_generic::<7, FpQuadraticSeven<7>>(&a) {
            assert!(
                p_annihilates_a(&mp, &a, 0xCAFE),
                "adversarial Jordan extension minpoly must annihilate A"
            );
            assert_eq!(
                mp, mp_via_dispatch,
                "extension Wiedemann must match public minpoly when engaged",
            );
        }
        assert_eq!(
            mp_via_dispatch.degree(),
            Some(5),
            "(x-2)^3 · x^2 has degree 5"
        );
    }

    /// Bypasses the public gate so the algorithm runs at small `n`.
    #[test]
    fn test_extension_random_cross_check_fp7() {
        for n in [2usize, 3, 5, 8, 16] {
            for seed in [1u64, 17, 42, 1000] {
                let a = make_random_fp::<7>(n, seed);
                let dispatch_mp = a.minpoly();
                if let Some(ext_mp) = run_quadratic_generic::<7, FpQuadraticSeven<7>>(&a) {
                    assert_returned_poly_is_consistent_divisor::<7>(
                        &ext_mp,
                        &dispatch_mp,
                        &a,
                        "quadratic Fp<7>",
                        n,
                        seed,
                    );
                }
                if let Some(ext_mp) = run_cubic_generic::<7, FpCubicSeven<7>>(&a) {
                    assert_returned_poly_is_consistent_divisor::<7>(
                        &ext_mp,
                        &dispatch_mp,
                        &a,
                        "cubic Fp<7>",
                        n,
                        seed,
                    );
                }
            }
        }
    }

    #[test]
    fn test_extension_random_cross_check_fp251() {
        for n in [2usize, 3, 5, 8, 16] {
            for seed in [1u64, 17, 42, 1000] {
                let a = make_random_fp::<251>(n, seed);
                let dispatch_mp = a.minpoly();
                if let Some(ext_mp) = run_quadratic_generic::<251, FpQuadraticTwoFiftyOne<251>>(&a)
                {
                    assert_returned_poly_is_consistent_divisor::<251>(
                        &ext_mp,
                        &dispatch_mp,
                        &a,
                        "quadratic Fp<251>",
                        n,
                        seed,
                    );
                }
            }
        }
    }

    fn assert_returned_poly_is_consistent_divisor<const P: u64>(
        ext_mp: &FieldPoly<Fp<P>>,
        dispatch_mp: &FieldPoly<Fp<P>>,
        a: &FieldMatrix<Fp<P>>,
        label: &str,
        n: usize,
        seed: u64,
    ) {
        let pa = ext_mp.eval_at_matrix(a);
        let zero = Fp::<P>::new(0);
        for i in 0..n {
            for j in 0..n {
                assert_eq!(
                    pa.get(i, j),
                    zero,
                    "{}: p(A) non-zero at ({},{}) for n={} seed={} (poly {:?})",
                    label,
                    i,
                    j,
                    n,
                    seed,
                    ext_mp,
                );
            }
        }
        let (_q, r) = dispatch_mp.div_rem(ext_mp);
        assert!(
            r.is_zero(),
            "{}: extension result does not divide dispatcher minpoly for n={} seed={} \
             (extension {:?}, dispatch {:?})",
            label,
            n,
            seed,
            ext_mp,
            dispatch_mp,
        );
    }

    #[test]
    fn test_extension_descent_fp7_random() {
        let n = 16;
        let a = make_random_fp::<7>(n, 0x1234_5678);
        let mp = run_quadratic_generic::<7, FpQuadraticSeven<7>>(&a).expect("must engage at n=16");
        let dispatch_mp = a.minpoly();
        assert_eq!(mp, dispatch_mp);
        let zero = Fp::<7>::new(0);
        let pa = mp.eval_at_matrix(&a);
        for i in 0..n {
            for j in 0..n {
                assert_eq!(pa.get(i, j), zero, "p(A) must vanish at ({},{})", i, j);
            }
        }
    }

    #[test]
    fn test_extension_descent_fp251_random() {
        let n = 16;
        let a = make_random_fp::<251>(n, 0xDEAD_BEEF);
        let mp = run_quadratic_generic::<251, FpQuadraticTwoFiftyOne<251>>(&a)
            .expect("must engage at n=16");
        let dispatch_mp = a.minpoly();
        assert_eq!(mp, dispatch_mp);
        let zero = Fp::<251>::new(0);
        let pa = mp.eval_at_matrix(&a);
        for i in 0..n {
            for j in 0..n {
                assert_eq!(pa.get(i, j), zero);
            }
        }
    }

    /// A pure-base polynomial passes [`descend_quadratic_runtime`] and
    /// [`descend_cubic_runtime`] unchanged. `FieldPoly<Fp<P>>` cannot carry a
    /// non-zero α component, so the rejecting branch is exercised only as the
    /// per-coefficient predicate in [`runtime_descent_synthetic_alpha_test`].
    #[test]
    fn test_extension_descent_helpers_runtime_guard() {
        let pure: FieldPoly<Fp<7>> =
            FieldPoly::from_coeffs_trimmed(vec![Fp::<7>::new(3), Fp::<7>::new(0), Fp::<7>::new(1)]);
        let q = descend_quadratic_runtime::<Fp<7>, FpQuadraticSeven<7>>(&pure)
            .expect("pure-base poly must descend");
        assert_eq!(q, pure);
        let c = descend_cubic_runtime::<Fp<7>, FpCubicSeven<7>>(&pure)
            .expect("pure-base poly must descend (cubic)");
        assert_eq!(c, pure);

        runtime_descent_synthetic_alpha_test();
    }

    fn runtime_descent_synthetic_alpha_test() {
        use crate::gfpn::{CubicExt, QuadraticExt};

        let bad_quad: QuadraticExt<FpQuadraticSeven<7>> =
            QuadraticExt::new(Fp::<7>::new(3), Fp::<7>::new(2));
        assert!(
            !bad_quad.c1().is_zero(),
            "non-zero α component must trip rejection",
        );

        let bad_cubic: CubicExt<FpCubicSeven<7>> =
            CubicExt::new(Fp::<7>::new(3), Fp::<7>::new(0), Fp::<7>::new(1));
        assert!(
            !bad_cubic.c1().is_zero() || !bad_cubic.c2().is_zero(),
            "non-zero α² component must trip rejection",
        );
    }

    fn make_random_fp<const P: u64>(n: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut state = seed;
        let mut next = || {
            let mut z = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            state = z;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        let mut a = FieldMatrix::<Fp<P>>::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                a.set(i, j, Fp::<P>::new(next() % P));
            }
        }
        a
    }
}
