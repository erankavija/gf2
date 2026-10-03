//! Multi-word streaming column-sum path for `permanent_bipedal3` at `n ≥ 64`.
//!
//! Evaluates Ryser's inclusion-exclusion formula in binary-reflected
//! Gray-code order: each step adds or subtracts one column to a packed
//! column-sum of `W = ceil(n / 64)` words per leg (`mag` + `sgn`) with the
//! `@/citation/Scheinerman2024` Theorem 2.1 formulas. The step counter is a
//! little-endian `[u64; 4]`, which bounds the dimension at
//! [`N_MAX_MULTIWORD`]. Within that bound the matrix occupies at most
//! [`MAX_MATRIX_BYTES_FOR_L1`] bytes (checked at compile time) and the loop
//! is not cache-blocked.

use gf2_core::gfp::Fp;

use crate::packed::bipedal3::{Bipedal3, Bipedal3Matrix};
use crate::packed::PackedField;

/// Maximum supported `n` for the multi-word streaming path.
///
/// The step counter `k` is a 256-bit little-endian `[u64; 4]` that runs up
/// to `2^n`, so `n = 255` keeps it below `2^256` and
/// `gray_counter_is_zero_above` reads only the 4 in-bounds counter words.
pub const N_MAX_MULTIWORD: usize = 255;

/// L1 data-cache size, in bytes, assumed by [`MAX_MATRIX_BYTES_FOR_L1`].
pub const L1D_BYTES: usize = 32 * 1024;

/// Matrix footprint budget in bytes: half of [`L1D_BYTES`].
///
/// The matrix occupies `2 * W * n * 8` bytes (mag + sgn legs of `W` words
/// per column); at `n = N_MAX_MULTIWORD` that is `16320` bytes.
pub const MAX_MATRIX_BYTES_FOR_L1: usize = L1D_BYTES / 2;

/// Footprint in bytes of an `n × n` Bipedal3 matrix: each column carries
/// `ceil(n / 64) * 8` bytes per leg (mag + sgn).
#[inline]
pub const fn matrix_bytes_for_n(n: usize) -> usize {
    let w = n.div_ceil(64);
    2 * w * 8 * n
}

const _: () = {
    assert!(matrix_bytes_for_n(N_MAX_MULTIWORD) <= MAX_MATRIX_BYTES_FOR_L1);
};

/// Compute the permanent of `mat` over `F_3` using the multi-word streaming
/// column-sum.
///
/// The dispatcher [`permanent_bipedal3`](crate::permanent::bipedal3::permanent_bipedal3)
/// routes `64 ≤ n ≤ N_MAX_MULTIWORD` here and `n ≤ 63` to
/// [`permanent_bipedal3_singleword`](crate::permanent::bipedal3::permanent_bipedal3_singleword).
/// Direct calls accept any `n` in `1..=N_MAX_MULTIWORD`.
///
/// Its unit tests compare it against `permanent_ryser::<Fp<3>>` at
/// `n ≤ 24`.
///
/// # Examples
///
/// ```no_run
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::bipedal3_multiword::permanent_bipedal3_multiword;
/// use gf2_core::gfp::Fp;
///
/// // 65×65 identity over F_3: permanent = 1
/// let mut id = vec![Fp::<3>::new(0); 65 * 65];
/// for i in 0..65 { id[i * 65 + i] = Fp::<3>::new(1); }
/// let m = Bipedal3Matrix::from_row_major(&id, 65, 65);
/// assert_eq!(permanent_bipedal3_multiword(&m), Fp::<3>::new(1));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > N_MAX_MULTIWORD` (`n` must be `≤ 255`).
///
/// Panics if `mat.cols() == 0` (the zero-dimension edge case belongs to
/// `permanent_bipedal3_singleword` / the dispatcher).
///
/// # Complexity
///
/// `O(n · 2^n)` bitwise word operations:
/// - Gray walk: `2^n - 1` steps, each with `W = ceil(n / 64)` bipedal
///   add/sub ops (6 word-level ops each) plus `W` bipedal mul ops for the
///   sequential word fold.
/// - Space: `O(W)` extra (`col_sum_mag` + `col_sum_sgn` buffers of
///   length `W` each, plus a 256-bit Gray counter).
pub fn permanent_bipedal3_multiword(mat: &Bipedal3Matrix) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3_multiword: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= N_MAX_MULTIWORD,
        "permanent_bipedal3_multiword: n = {n} exceeds N_MAX_MULTIWORD = {N_MAX_MULTIWORD}"
    );
    // No lower-bound assert: the counter and word loops handle
    // `1..=N_MAX_MULTIWORD` uniformly, and the tests call this path directly
    // at small `n`.

    let w = n.div_ceil(64);

    // Tail lanes (bits n%64..63 of the last word) hold packed-one (mag = 1,
    // sgn = 0) so the lane product in `fold_mul_words` ignores them.
    let mut col_sum_mag = vec![0u64; w];
    let mut col_sum_sgn = vec![0u64; w];
    let tail_mask_hi: u64 = if n.is_multiple_of(64) {
        0u64
    } else {
        !0u64 << (n % 64)
    };
    col_sum_mag[w - 1] |= tail_mask_hi;

    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // Step counter k, little-endian (word 0 least significant).
    let mut gray_counter = [0u64; 4];

    loop {
        inc_counter(&mut gray_counter);

        // Stop at k = 2^n.
        if gray_counter_is_zero_above(&gray_counter, n) {
            break;
        }

        let flip = trailing_zeros_256(&gray_counter);

        // Bit `flip` of g_k = k ^ (k >> 1) is set iff column `flip` just
        // entered the subset.
        let added = gray_bit_at(&gray_counter, flip);

        let col = mat.column(flip);
        let col_mag = col.raw_mag();
        let col_sgn = col.raw_sgn();

        if added {
            subset_size += 1;
            for i in 0..w {
                let result = Bipedal3::from_raw(col_sum_mag[i], col_sum_sgn[i])
                    .add(Bipedal3::from_raw(col_mag[i], col_sgn[i]));
                col_sum_mag[i] = result.mag();
                col_sum_sgn[i] = result.sgn();
            }
        } else {
            subset_size -= 1;
            for i in 0..w {
                let result = Bipedal3::from_raw(col_sum_mag[i], col_sum_sgn[i])
                    .sub(Bipedal3::from_raw(col_mag[i], col_sgn[i]));
                col_sum_mag[i] = result.mag();
                col_sum_sgn[i] = result.sgn();
            }
        }

        // Reset the tail lanes to packed-one after add/sub.
        col_sum_mag[w - 1] |= tail_mask_hi;
        col_sum_sgn[w - 1] &= !tail_mask_hi;

        let term = fold_mul_words(&col_sum_mag, &col_sum_sgn, n);

        // Ryser sign: (-1)^|S|.
        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total += term;
        }
    }

    // Apply outer (-1)^n factor from Ryser's formula.
    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

// ---------------------------------------------------------------------------
// 256-bit Gray-code counter helpers
// ---------------------------------------------------------------------------

/// Increment a 256-bit little-endian counter stored as `[u64; 4]`.
#[inline]
fn inc_counter(c: &mut [u64; 4]) {
    for word in c.iter_mut() {
        *word = word.wrapping_add(1);
        if *word != 0 {
            break; // No carry out of this word.
        }
    }
}

/// Return `true` iff the 256-bit counter equals `2^n`.
#[inline]
fn gray_counter_is_zero_above(c: &[u64; 4], n: usize) -> bool {
    let word = n >> 6;
    let bit = n & 63;
    if (c[word] >> bit) & 1 != 1 {
        return false;
    }
    if c[word] & ((1u64 << bit) - 1) != 0 {
        return false;
    }
    if c.iter().take(word).any(|&w| w != 0) {
        return false;
    }
    if c.iter().skip(word + 1).any(|&w| w != 0) {
        return false;
    }
    true
}

/// Number of trailing zero bits of a 256-bit little-endian counter `k`
/// (256 for `k = 0`): the Gray-code flip index of step `k`.
#[inline]
fn trailing_zeros_256(c: &[u64; 4]) -> usize {
    for (i, &word) in c.iter().enumerate() {
        if word != 0 {
            return i * 64 + word.trailing_zeros() as usize;
        }
    }
    256
}

/// Return the value of bit `pos` in the Gray-code register `g(k) = k ^ (k >> 1)`
/// where `k` is represented as a 256-bit little-endian counter.
///
/// At `pos = trailing_zeros(k)`, `true` means column `pos` just entered the
/// subset (add) and `false` that it just left (subtract).
#[inline]
fn gray_bit_at(c: &[u64; 4], pos: usize) -> bool {
    let k_bit_pos = {
        let w = pos >> 6;
        let b = pos & 63;
        (c[w] >> b) & 1
    };
    let k_bit_pos_plus_1 = if pos + 1 < 256 {
        let w = (pos + 1) >> 6;
        let b = (pos + 1) & 63;
        (c[w] >> b) & 1
    } else {
        0
    };
    (k_bit_pos ^ k_bit_pos_plus_1) == 1
}

// ---------------------------------------------------------------------------
// fold_mul: sequential word reduction then per-word lane fold
// ---------------------------------------------------------------------------

/// Product of all active lanes of a `W`-word bipedal value as one `Fp<3>`.
///
/// Words are first combined lane-wise with the bipedal product
/// (`m' = m1 & m2; s' = s1 ^ s2`), then the lanes of the combined word are
/// folded with `Bipedal3::fold_mul_first_n`. F_3 multiplication is
/// commutative and associative, so this equals folding each word and
/// multiplying the per-word scalars.
///
/// For `W > 1` the caller must have set the tail lanes of the last word to
/// the identity (`mag = 1, sgn = 0`); for `W = 1` the fold covers the first
/// `n` lanes only.
#[inline]
fn fold_mul_words(mag: &[u64], sgn: &[u64], n: usize) -> Fp<3> {
    debug_assert_eq!(mag.len(), sgn.len());
    let w = mag.len();
    debug_assert!(w >= 1);

    let mut acc_mag = u64::MAX;
    let mut acc_sgn = 0u64;
    for i in 0..w {
        acc_mag &= mag[i];
        acc_sgn ^= sgn[i];
    }

    let b3 = Bipedal3::from_raw(acc_mag, acc_sgn);
    if w == 1 {
        b3.fold_mul_first_n(n.clamp(1, 64))
    } else {
        b3.fold_mul_first_n(64)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permanent::ryser::permanent_ryser;
    use crate::testutil::random_matrix;

    // -----------------------------------------------------------------------
    // gray_bit_at unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_gray_bit_at_matches_scalar_for_small_k() {
        for k in 1u64..128u64 {
            let g_k = k ^ (k >> 1);
            for pos in 0..7usize {
                let scalar_bit = ((g_k >> pos) & 1) == 1;
                let c = [k, 0, 0, 0];
                let computed = gray_bit_at(&c, pos);
                assert_eq!(
                    computed, scalar_bit,
                    "gray_bit_at mismatch: k={k}, pos={pos}"
                );
            }
        }
    }

    #[test]
    fn test_inc_counter_carry_propagation() {
        let mut c = [u64::MAX, 0, 0, 0];
        inc_counter(&mut c);
        assert_eq!(c, [0, 1, 0, 0], "carry from word 0 to word 1");

        let mut c2 = [u64::MAX, u64::MAX, 0, 0];
        inc_counter(&mut c2);
        assert_eq!(c2, [0, 0, 1, 0], "carry from word 1 to word 2");
    }

    #[test]
    fn test_trailing_zeros_256_basic() {
        assert_eq!(trailing_zeros_256(&[1, 0, 0, 0]), 0);
        assert_eq!(trailing_zeros_256(&[2, 0, 0, 0]), 1);
        assert_eq!(trailing_zeros_256(&[4, 0, 0, 0]), 2);
        assert_eq!(trailing_zeros_256(&[0, 1, 0, 0]), 64);
        assert_eq!(trailing_zeros_256(&[0, 2, 0, 0]), 65);
    }

    #[test]
    fn test_gray_counter_is_zero_above_basic() {
        assert!(gray_counter_is_zero_above(&[16, 0, 0, 0], 4));
        assert!(gray_counter_is_zero_above(&[1u64 << 63, 0, 0, 0], 63));
        assert!(gray_counter_is_zero_above(&[0, 1, 0, 0], 64));
        assert!(gray_counter_is_zero_above(&[0, 2, 0, 0], 65));
        assert!(!gray_counter_is_zero_above(&[15, 0, 0, 0], 4));
    }

    // -----------------------------------------------------------------------
    // Cross-check vs `permanent_ryser<Fp<3>>`, calling the multi-word path
    // directly at small n.
    // -----------------------------------------------------------------------

    fn run_multiword_vs_ryser_at_n(n: usize, n_trials: u64, seed_tag: u64) {
        let seed_base: u64 = 0xa788_6bd8_0000_0000_u64
            .wrapping_add((n as u64) << 16)
            .wrapping_add(seed_tag);
        for trial in 0u64..n_trials {
            let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
            let row_major = random_matrix::<3>(n, seed);
            let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
            let actual = permanent_bipedal3_multiword(&mat);
            let expected = permanent_ryser::<Fp<3>>(&row_major, n);
            assert_eq!(
                actual, expected,
                "multi-word vs ryser mismatch: n={n}, trial={trial}, seed={seed:#018x}"
            );
        }
    }

    #[test]
    fn test_multiword_vs_ryser_n2() {
        run_multiword_vs_ryser_at_n(2, 200, 0x0002);
    }

    /// A non-square input must panic in release builds too, rather than
    /// silently discard the rows beyond `cols()`.
    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_multiword_panics_on_non_square() {
        let n_rows = 66usize;
        let n_cols = 65usize;
        let data = vec![Fp::<3>::new(0); n_rows * n_cols];
        let m = Bipedal3Matrix::from_row_major(&data, n_rows, n_cols);
        let _ = permanent_bipedal3_multiword(&m);
    }

    /// `n > N_MAX_MULTIWORD` must panic in release builds too, before the
    /// walk indexes past the `[u64; 4]` Gray counter.
    #[test]
    #[should_panic(expected = "exceeds N_MAX_MULTIWORD")]
    fn test_multiword_panics_above_n_max() {
        let n = N_MAX_MULTIWORD + 1;
        let data = vec![Fp::<3>::new(0); n * n];
        let m = Bipedal3Matrix::from_row_major(&data, n, n);
        let _ = permanent_bipedal3_multiword(&m);
    }

    #[test]
    fn test_multiword_vs_ryser_n5() {
        run_multiword_vs_ryser_at_n(5, 200, 0x0005);
    }

    #[test]
    fn test_multiword_vs_ryser_n8() {
        run_multiword_vs_ryser_at_n(8, 200, 0x0008);
    }

    #[test]
    fn test_multiword_vs_ryser_n16() {
        run_multiword_vs_ryser_at_n(16, 200, 0x0010);
    }

    #[test]
    #[ignore = "slow: multi-word vs ryser at n=20 (50 trials; ~4 s solo, \
                exceeds the 5 s fast-tier hard kill under parallel --workspace load)"]
    fn test_multiword_vs_ryser_n20_slow() {
        run_multiword_vs_ryser_at_n(20, 50, 0x0014);
    }

    #[test]
    #[ignore = "slow: multi-word vs ryser at n=24 (per-trial ~1 s; 5 trials ~5 s)"]
    fn test_multiword_vs_ryser_n24_slow() {
        run_multiword_vs_ryser_at_n(24, 5, 0x0018);
    }

    // -----------------------------------------------------------------------
    // Block-diagonal cross-check: perm(A_{n0} ⊕ I_{n - n0}) = perm(A_{n0}).
    // -----------------------------------------------------------------------

    /// Row-major `A_{n0} ⊕ I_{n - n0}` with a random F_3 block `A_{n0}`;
    /// returns `(full, block)`.
    fn build_block_diagonal(n: usize, n0: usize, seed: u64) -> (Vec<Fp<3>>, Vec<Fp<3>>) {
        assert!(n0 < n, "n0 must be strictly less than n for padding");
        let block = random_matrix::<3>(n0, seed);
        let mut full = vec![Fp::<3>::new(0); n * n];
        for i in 0..n0 {
            for j in 0..n0 {
                full[i * n + j] = block[i * n0 + j];
            }
        }
        for k in 0..(n - n0) {
            let idx = n0 + k;
            full[idx * n + idx] = Fp::<3>::new(1);
        }
        (full, block)
    }

    fn run_block_diagonal_at_n(n: usize, n0_base: usize, seed_tag: u64) {
        let seed_base: u64 = 0xa788_6bd8_0000_0000_u64
            .wrapping_add((n as u64) << 16)
            .wrapping_add(seed_tag);
        // n0 cycles through {n0_base, n0_base + 1, n0_base + 2}.
        for trial in 0u64..5 {
            let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
            let n0 = n0_base + ((trial as usize) % 3);
            let (full, block) = build_block_diagonal(n, n0, seed);
            let expected = permanent_ryser::<Fp<3>>(&block, n0);
            let mat = Bipedal3Matrix::from_row_major(&full, n, n);
            let actual = permanent_bipedal3_multiword(&mat);
            assert_eq!(
                actual, expected,
                "block-decomposable cross-check failed: n={n}, n0={n0}, trial={trial}, seed={seed:#018x}"
            );
        }
    }

    #[test]
    fn test_block_diagonal_at_n10() {
        run_block_diagonal_at_n(10, 5, 0x000a);
    }

    #[test]
    fn test_block_diagonal_at_n16() {
        run_block_diagonal_at_n(16, 10, 0x0010);
    }

    #[test]
    fn test_block_diagonal_at_n20() {
        run_block_diagonal_at_n(20, 10, 0x0014);
    }
}
