//! `permanent_bipedal3` — single-`u64`-pair fast path for permanents over
//! `F_3`, restricted to matrices with `n ≤ 63`.
//!
//! For `n ≤ 63` the column-sum vector fits in a single Bipedal3 word (one
//! `u64` mag + one `u64` sgn pair).  Each Gray-code step is a single
//! Bipedal3 add or sub against the toggled column, followed by a horizontal
//! fold via the **bipedal multiplication tree** — a shift-halving reduce
//! over 64 lanes using the Scheinerman 2024 `mul` formula (~6 steps).
//!
//! This module is the **headline single-thread fast path** of the
//! permanent epic; the 50× speedup target is measured against
//! `permanent_mod3_reference` at `n = 36`.  Multi-word streaming for
//! `n > 63` lands in W3-T14 and lives in a separate module.
//!
//! # Algorithm reference
//!
//! `dev/plans/gf2_algebra_permanent.md` §7.3 (single-word path).
//!
//! # Implementation note — raw u64 tracking
//!
//! `Bipedal3`'s `mag`/`sgn` fields are private (encapsulated inside
//! `gf2-algebra::packed::bipedal3`).  The bipedal-mul-tree horizontal fold
//! requires shifting those raw words right by 32, 16, 8, 4, 2, 1 bits and
//! multiplying (AND for mag, XOR for sgn per the Scheinerman 2024 paper
//! formula).  Rather than adding new public API to `Bipedal3`, we maintain
//! the column-sum state as a pair of plain `u64` scalars (`cs_mag`,
//! `cs_sgn`) and inline the six-operation add/sub formulas verbatim from
//! the paper.  `Bipedal3::from_raw` is used only when computing the matrix
//! prep (column extraction), where the trait's `lane` / `with_lane` APIs
//! suffice without raw-field access.

use gf2_core::gfp::Fp;

use crate::gray::gray_code_iter;
use crate::packed::bipedal3::Bipedal3Matrix;
use crate::packed::PackedFieldVec;

/// Compute the permanent of an `n × n` matrix over `F_3` using the
/// single-`u64` Bipedal3 fast path.
///
/// For `n ≤ 63` the column-sum vector fits in a single Bipedal3 word
/// (one `u64` mag + one `u64` sgn pair), so each Gray-code step performs
/// exactly one Bipedal3 add or sub followed by a horizontal
/// bipedal-multiplication-tree fold of the `n` active lanes.
///
/// The permanent of an `n × n` matrix `A` over `F_3` is:
///
/// ```text
/// perm(A) = sum over all permutations sigma of prod_{i=0}^{n-1} A[i, sigma(i)]
/// ```
///
/// Evaluated via Ryser's inclusion-exclusion formula in Gray-code order
/// (see `permanent_ryser` for the generic version):
///
/// ```text
/// perm(A) = (-1)^n * sum_{S ⊆ [n], S ≠ ∅} (-1)^|S| * prod_{i=0}^{n-1} sum_{j ∈ S} A[i,j]
/// ```
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Bipedal3Matrix`] (column-major, `rows == cols`).
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::permanent_bipedal3;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_3: permanent = 1
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// let m = Bipedal3Matrix::from_row_major(&id, 2, 2);
/// assert_eq!(permanent_bipedal3(&m), Fp::<3>::new(1));
///
/// // 2×2 all-ones over F_3: permanent = 2! mod 3 = 2
/// let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 4];
/// let m2 = Bipedal3Matrix::from_row_major(&ones, 2, 2);
/// assert_eq!(permanent_bipedal3(&m2), Fp::<3>::new(2));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > 63` (single-`u64` fast path requires `n ≤ 63`
/// because [`gray_code_iter`] uses `1u64 << n` as the iteration bound,
/// which is undefined behaviour for `n ≥ 64`).
///
/// # Complexity
///
/// `O(n · 2^n)` field operations over `Fp<3>`:
/// - Matrix prep: `O(n^2)` one-time lane-by-lane column extraction.
/// - Gray walk: `2^n - 1` steps, each with 1 Bipedal3 add/sub (6 word-level
///   bitwise ops) plus 1 bipedal-multiplication-tree fold (~6 Bipedal3::mul
///   word-pairs, 2 ops each).
/// - Space: `O(n)` extra (the `columns` Vec plus raw col-sum pair).
pub fn permanent_bipedal3(mat: &Bipedal3Matrix) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 63,
        "permanent_bipedal3: single-u64 fast path requires n <= 63; got n = {}",
        n
    );

    // Edge case: the 0×0 matrix has exactly one permutation (the empty
    // one), whose product over an empty index set is the vacuous product 1.
    if n == 0 {
        return Fp::<3>::new(1);
    }

    // One-time matrix-prep: extract each column into a raw (mag, sgn) pair.
    //
    // The Bipedal3 encoding of an Fp<3> element v is:
    //   v = 0 → (mag_bit = 0, sgn_bit = 0)
    //   v = 1 → (mag_bit = 1, sgn_bit = 0)
    //   v = 2 → (mag_bit = 1, sgn_bit = 1)
    //
    // We pack the n rows of each column j into two u64 words.  Bit i of
    // `col_mag[j]` holds the mag_bit for row i; bit i of `col_sgn[j]`
    // holds sgn_bit for row i.  Bits n..63 remain 0.
    //
    // Cost: O(n^2) — dominated by the O(n · 2^n) Gray walk for n ≥ 4.
    let mut col_mag = vec![0u64; n];
    let mut col_sgn = vec![0u64; n];
    for j in 0..n {
        let col_vec = mat.column(j);
        for i in 0..n {
            let v = col_vec.get(i).value(); // 0, 1, or 2
            let mag_bit = if v != 0 { 1u64 } else { 0u64 };
            let sgn_bit = if v == 2 { 1u64 } else { 0u64 };
            col_mag[j] |= mag_bit << i;
            col_sgn[j] |= sgn_bit << i;
        }
    }

    // Column-sum accumulator as raw u64 pair.
    // cs_mag / cs_sgn encode sum_{j ∈ S} A[i,j] for each row i (bit i).
    // Lanes n..63 remain 0 throughout.
    let mut cs_mag: u64 = 0;
    let mut cs_sgn: u64 = 0;

    // Mask covering the n active lanes (bits 0..n-1).
    // Bits n..63 will be forced to the multiplicative identity (mag=1, sgn=0)
    // before each halving fold, so they contribute 1 and do not zero the product.
    let used_mask: u64 = (1u64 << n) - 1; // n <= 63, so no UB here
    let id_mag_for_unused = !used_mask; // mag=1 for bits n..63

    // Running Ryser accumulator and subset-size counter.
    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // Gray walk: enumerate all 2^n - 1 non-empty subsets of [n].
    // At each step (flip, parity):
    //   flip   — which column just entered or left S
    //   parity — +1 (entered, ADD) or -1 (left, SUB)
    for (flip, parity) in gray_code_iter(n) {
        let bm = col_mag[flip];
        let bsg = col_sgn[flip];

        if parity == 1 {
            // cs += col[flip]: Scheinerman 2024 Algorithm 2 (6 bitwise ops).
            subset_size += 1;
            let am = cs_mag;
            let asg = cs_sgn;
            let t = am ^ asg ^ bsg;
            let u = bm & t;
            cs_mag = u | (am ^ bm);
            cs_sgn = u ^ asg;
        } else {
            // cs -= col[flip]: Scheinerman 2024 sub formula (6 bitwise ops).
            subset_size -= 1;
            let am = cs_mag;
            let asg = cs_sgn;
            let t = asg ^ bsg;
            let u = am & t;
            cs_mag = u | (am ^ bm);
            cs_sgn = u ^ (bm ^ bsg);
        }

        // Horizontal fold via bipedal multiplication tree.
        //
        // Goal: compute the product of the n active lanes (bits 0..n-1) in
        // the (cs_mag, cs_sgn) word pair.
        //
        // Strategy: identity-pad the inactive lanes (n..63) to mul-identity
        // (mag=1, sgn=0), then halve-and-mul six times.  Each halving step
        // uses the Scheinerman 2024 paper-mul formula:
        //   mul(a, b).mag = a.mag AND b.mag
        //   mul(a, b).sgn = a.sgn XOR b.sgn
        // which holds lane-wise in packed form.
        //
        // After 6 halvings (32→16→8→4→2→1), bit 0 holds the product of
        // all 64 logical lanes.  Because inactive lanes were padded to the
        // identity, bit 0 equals the product of the n active lanes.
        let mut acc_m = cs_mag | id_mag_for_unused; // identity for bits n..63
        let mut acc_s = cs_sgn & used_mask; // zero sgn for bits n..63

        // log2(64) = 6 halving steps.
        let mut step: u32 = 32;
        while step > 0 {
            // Scheinerman 2024 paper-mul: mag = AND, sgn = XOR.
            acc_m &= acc_m >> step;
            acc_s ^= acc_s >> step;
            step >>= 1;
        }
        // Bit 0 of (acc_m, acc_s) encodes the product of the n active lanes.
        // Decode via the canonical Bipedal3 mapping:
        //   acc_m=0 → 0 (regardless of acc_s)
        //   acc_m=1, acc_s=0 → 1
        //   acc_m=1, acc_s=1 → 2
        let term = if acc_m & 1 == 0 {
            Fp::<3>::new(0)
        } else if acc_s & 1 == 0 {
            Fp::<3>::new(1)
        } else {
            Fp::<3>::new(2)
        };

        // Ryser sign: (-1)^|S|.
        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total += term;
        }
    }

    // Apply the outer (-1)^n factor from Ryser's formula.
    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packed::Bipedal3Matrix;
    use crate::permanent::reference::permanent_mod3_reference;
    use crate::permanent::ryser::permanent_ryser;
    use gf2_core::gfp::Fp;
    use gf2_core::rng::Lcg;

    // -----------------------------------------------------------------------
    // Deterministic pseudo-random matrix generator using gf2_core::rng::Lcg
    // -----------------------------------------------------------------------

    /// Generate a deterministic pseudo-random `n×n` matrix of `Fp<3>` elements
    /// as a flat row-major Vec, using `gf2_core::rng::Lcg` (Knuth MMIX constants).
    fn random_matrix_fp3(n: usize, seed: u64) -> Vec<Fp<3>> {
        let mut rng = Lcg::new(seed);
        (0..n * n)
            .map(|_| Fp::<3>::new(rng.next_u64() % 3))
            .collect()
    }

    /// Wrap a row-major `Vec<Fp<3>>` into a `Bipedal3Matrix`.
    fn to_bipedal3_matrix(row_major: &[Fp<3>], n: usize) -> Bipedal3Matrix {
        Bipedal3Matrix::from_row_major(row_major, n, n)
    }

    // -----------------------------------------------------------------------
    // Hand-checked vectors
    // -----------------------------------------------------------------------

    /// `permanent_bipedal3` of the 0×0 matrix is `Fp::<3>::new(1)` (vacuous product).
    #[test]
    fn test_permanent_empty_matrix() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 0);
        assert_eq!(
            permanent_bipedal3(&m),
            Fp::<3>::new(1),
            "0×0 permanent must be 1"
        );
    }

    /// A 1×1 matrix `[v]` has permanent = `v`.
    #[test]
    fn test_permanent_1x1() {
        for v in 0u64..3 {
            let row = vec![Fp::<3>::new(v)];
            let m = Bipedal3Matrix::from_row_major(&row, 1, 1);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(v),
                "1×1 permanent of [{v}] must be {v}"
            );
        }
    }

    /// `I_n` has permanent = 1 for `n ∈ {1, 2, 3, 4}`.
    #[test]
    fn test_permanent_identity_n() {
        for n in 1..=4usize {
            let mut id = vec![Fp::<3>::new(0); n * n];
            for i in 0..n {
                id[i * n + i] = Fp::<3>::new(1);
            }
            let m = Bipedal3Matrix::from_row_major(&id, n, n);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(1),
                "identity permanent must be 1 for n={n}"
            );
        }
    }

    /// All-ones `n×n` matrix: permanent = `n! mod 3` for `n ∈ {1, 2, 3, 4}`.
    ///
    /// n! mod 3: n=1 → 1, n=2 → 2, n=3 → 6 ≡ 0, n=4 → 24 ≡ 0.
    #[test]
    fn test_permanent_all_ones_n() {
        // n! mod 3: {1, 2, 0, 0}
        let expected = [1u64, 2, 0, 0];
        for n in 1..=4usize {
            let ones = vec![Fp::<3>::new(1); n * n];
            let m = Bipedal3Matrix::from_row_major(&ones, n, n);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(expected[n - 1]),
                "all-ones permanent for n={n} must be {} (= n! mod 3)",
                expected[n - 1]
            );
        }
    }

    // -----------------------------------------------------------------------
    // Bipedal mul tree spot-check: verify tree fold == scalar lane decode
    //
    // This test directly exercises the halving-fold kernel used inside
    // `permanent_bipedal3` on a range of known (mag, sgn) patterns,
    // confirming it equals an explicit scalar per-lane decode.
    // -----------------------------------------------------------------------

    /// Verify the bipedal multiplication tree fold (used in `permanent_bipedal3`)
    /// produces the same result as explicit scalar per-lane decode.
    #[test]
    fn test_bipedal_mul_tree_matches_scalar_fold() {
        // Patterns: (mag, sgn, n, expected_product)
        // where expected_product = product of lanes 0..n over F_3.
        //
        // Case 1: all lanes = 2 (mag=all1s, sgn=all1s) for n=1..8.
        // Product = 2^n mod 3; period-2: n=1→2, n=2→1, n=3→2, ...
        for n in 1usize..=8 {
            let mag = (1u64 << n) - 1; // bits 0..n all set
            let sgn = mag; // all lanes = 2 (mag=1, sgn=1)
            let expected = if n % 2 == 1 {
                Fp::<3>::new(2)
            } else {
                Fp::<3>::new(1)
            };

            let used_mask: u64 = (1u64 << n) - 1;
            let id_mag_for_unused = !used_mask;
            let mut acc_m = mag | id_mag_for_unused;
            let mut acc_s = sgn & used_mask;
            let mut step: u32 = 32;
            while step > 0 {
                acc_m &= acc_m >> step;
                acc_s ^= acc_s >> step;
                step >>= 1;
            }
            let tree_prod = if acc_m & 1 == 0 {
                Fp::<3>::new(0)
            } else if acc_s & 1 == 0 {
                Fp::<3>::new(1)
            } else {
                Fp::<3>::new(2)
            };
            assert_eq!(
                tree_prod, expected,
                "all-2s product mismatch at n={n}: got {tree_prod:?} want {expected:?}"
            );
        }

        // Case 2: mixed {0,1,2} pattern — any zero lane makes product zero.
        // n=4, pattern: lane0=1, lane1=2, lane2=0, lane3=2 → product=0.
        {
            let n = 4usize;
            // lane0=1: mag0=1,sgn0=0 → mag bit 0 set; lane1=2: mag1=1,sgn1=1;
            // lane2=0: mag2=0,sgn2=0; lane3=2: mag3=1,sgn3=1.
            let mag: u64 = 0b1011; // bits 0,1,3 set
            let sgn: u64 = 0b1010; // bits 1,3 set
            let expected = Fp::<3>::new(0); // zero lane kills product

            let used_mask: u64 = (1u64 << n) - 1;
            let id_mag_for_unused = !used_mask;
            let mut acc_m = mag | id_mag_for_unused;
            let mut acc_s = sgn & used_mask;
            let mut step: u32 = 32;
            while step > 0 {
                acc_m &= acc_m >> step;
                acc_s ^= acc_s >> step;
                step >>= 1;
            }
            let tree_prod = if acc_m & 1 == 0 {
                Fp::<3>::new(0)
            } else if acc_s & 1 == 0 {
                Fp::<3>::new(1)
            } else {
                Fp::<3>::new(2)
            };
            assert_eq!(
                tree_prod, expected,
                "mixed pattern: zero-lane product should be 0"
            );
        }

        // Case 3: all lanes = 1 (mag=all1s, sgn=0) for n=4 → product=1.
        {
            let n = 4usize;
            let mag: u64 = (1u64 << n) - 1;
            let sgn: u64 = 0;
            let expected = Fp::<3>::new(1);

            let used_mask: u64 = (1u64 << n) - 1;
            let id_mag_for_unused = !used_mask;
            let mut acc_m = mag | id_mag_for_unused;
            let mut acc_s = sgn & used_mask;
            let mut step: u32 = 32;
            while step > 0 {
                acc_m &= acc_m >> step;
                acc_s ^= acc_s >> step;
                step >>= 1;
            }
            let tree_prod = if acc_m & 1 == 0 {
                Fp::<3>::new(0)
            } else if acc_s & 1 == 0 {
                Fp::<3>::new(1)
            } else {
                Fp::<3>::new(2)
            };
            assert_eq!(tree_prod, expected, "all-1s product should be 1 for n={n}");
        }
    }

    // -----------------------------------------------------------------------
    // Panic tests
    // -----------------------------------------------------------------------

    /// Non-square matrix panics.
    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_permanent_bipedal3_panics_on_non_square() {
        let data = vec![Fp::<3>::new(0); 3 * 5];
        let m = Bipedal3Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal3(&m);
    }

    /// `n = 65` exceeds the single-u64 fast path limit and panics.
    #[test]
    #[should_panic(expected = "single-u64 fast path requires n <= 63")]
    fn test_permanent_bipedal3_panics_on_n_exceeding_63() {
        let data = vec![Fp::<3>::new(0); 65 * 65];
        let m = Bipedal3Matrix::from_row_major(&data, 65, 65);
        let _ = permanent_bipedal3(&m);
    }

    /// `n = 64` exceeds the single-u64 fast path limit and panics.
    ///
    /// `gray_code_iter` requires `n <= 63` because `1u64 << 64` is undefined
    /// behaviour per the Rust reference. This test guards the boundary.
    #[test]
    #[should_panic(expected = "single-u64 fast path requires n <= 63")]
    fn test_permanent_bipedal3_panics_on_n_64() {
        let data = vec![Fp::<3>::new(0); 64 * 64];
        let m = Bipedal3Matrix::from_row_major(&data, 64, 64);
        let _ = permanent_bipedal3(&m);
    }

    // -----------------------------------------------------------------------
    // Cross-checks: permanent_bipedal3 vs permanent_ryser (default tier)
    // Per-n tests with 1000 random matrices each.
    // n=1..12 fit well within the 5 s budget; n=13..16 are slow-tier.
    // -----------------------------------------------------------------------

    macro_rules! cross_check_n {
        ($name:ident, $n:expr) => {
            #[test]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xb085_7ae9_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix_fp3(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
        ($name:ident, $n:expr, slow) => {
            #[test]
            #[ignore = "sim: per-n cross-check (n>12, 1000 matrices) — slow oracle, multi-second runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xb085_7ae9_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix_fp3(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    cross_check_n!(test_cross_check_n1, 1);
    cross_check_n!(test_cross_check_n2, 2);
    cross_check_n!(test_cross_check_n3, 3);
    cross_check_n!(test_cross_check_n4, 4);
    cross_check_n!(test_cross_check_n5, 5);
    cross_check_n!(test_cross_check_n6, 6);
    cross_check_n!(test_cross_check_n7, 7);
    cross_check_n!(test_cross_check_n8, 8);
    cross_check_n!(test_cross_check_n9, 9);
    cross_check_n!(test_cross_check_n10, 10);
    cross_check_n!(test_cross_check_n11, 11);
    cross_check_n!(test_cross_check_n12, 12);
    // n=13..16: 1000 matrices × Ryser O(n·2^n) exceeds 5 s for n≥13 in
    // release mode; these run only under the nightly slow tier.
    cross_check_n!(test_cross_check_n13, 13, slow);
    cross_check_n!(test_cross_check_n14, 14, slow);
    cross_check_n!(test_cross_check_n15, 15, slow);
    cross_check_n!(test_cross_check_n16, 16, slow);

    // -----------------------------------------------------------------------
    // Cross-checks: large n (slow tier — must not run in default CI)
    //
    // Oracle: `permanent_mod3_reference` (scalar i32, ~10× faster than generic
    // Fp<3> Ryser at large n). Correctness of the reference vs
    // `permanent_ryser` is established by T8's own cross-checks, so
    // "bit-identical to permanent_ryser" is preserved here by transitivity.
    //
    // Per the 2026-05-10 user-approved amendment to T9 criterion 3:
    //   - n=28/32 are NOT required.
    //   - n=20: 100 matrices × ~5 s/matrix → 5 sub-tests × 20 matrices each
    //     (each ≈ 100 s, fits 120 s slow-tier budget).
    //   - n=24: 100 matrices × ~8 s/matrix → 10 sub-tests × 10 matrices each
    //     (each ≈ 80 s, fits 120 s slow-tier budget).
    // -----------------------------------------------------------------------

    macro_rules! large_n_cross_check {
        ($name:ident, $n:expr, $trials:expr, $seed_salt:expr) => {
            #[test]
            #[ignore = "sim: large-n cross-check (n in {20, 24}) — slow oracle, multi-minute runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 = 0xb085_7ae9_2000_0000_u64
                    .wrapping_add(n as u64)
                    .wrapping_add($seed_salt);
                for trial in 0u64..$trials {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix_fp3(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    // Use permanent_mod3_reference as oracle: ~10× faster than
                    // generic Ryser at large n. Correctness of the reference vs
                    // permanent_ryser is established by T8 cross-checks.
                    let expected = permanent_mod3_reference(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    // n=20: 5 sub-tests × 20 matrices each = 100 total.
    // ~5 s/matrix × 20 = 100 s/sub-test — fits 120 s slow-tier budget.
    large_n_cross_check!(test_cross_check_n20_a, 20, 20, 0);
    large_n_cross_check!(test_cross_check_n20_b, 20, 20, 1_000);
    large_n_cross_check!(test_cross_check_n20_c, 20, 20, 2_000);
    large_n_cross_check!(test_cross_check_n20_d, 20, 20, 3_000);
    large_n_cross_check!(test_cross_check_n20_e, 20, 20, 4_000);

    // n=24: 10 sub-tests × 10 matrices each = 100 total.
    // ~8 s/matrix × 10 = 80 s/sub-test — fits 120 s slow-tier budget.
    large_n_cross_check!(test_cross_check_n24_a, 24, 10, 0);
    large_n_cross_check!(test_cross_check_n24_b, 24, 10, 1_000);
    large_n_cross_check!(test_cross_check_n24_c, 24, 10, 2_000);
    large_n_cross_check!(test_cross_check_n24_d, 24, 10, 3_000);
    large_n_cross_check!(test_cross_check_n24_e, 24, 10, 4_000);
    large_n_cross_check!(test_cross_check_n24_f, 24, 10, 5_000);
    large_n_cross_check!(test_cross_check_n24_g, 24, 10, 6_000);
    large_n_cross_check!(test_cross_check_n24_h, 24, 10, 7_000);
    large_n_cross_check!(test_cross_check_n24_i, 24, 10, 8_000);
    large_n_cross_check!(test_cross_check_n24_j, 24, 10, 9_000);
}
