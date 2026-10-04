//! Test-only helpers shared across the crate's test modules: deterministic
//! pseudo-random matrix generators and the brute-force oracles the production
//! kernels are checked against. Random helpers route through
//! [`gf2_core::rng::Lcg`] so that seed values reproduce identical streams
//! across modules.

use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

use crate::permanent::PermanentalRank;

/// Generate a deterministic pseudo-random `n × n` matrix of [`Fp<P>`] elements,
/// row-major.
///
/// Each entry is one [`Lcg::next_u64`] draw from a fresh [`Lcg`] seeded with
/// `seed`, reduced modulo `P`.
pub fn random_matrix<const P: u64>(n: usize, seed: u64) -> Vec<Fp<P>> {
    let mut rng = Lcg::new(seed);
    (0..n * n)
        .map(|_| Fp::<P>::new(rng.next_u64() % P))
        .collect()
}

/// Same as [`random_matrix`] but draws its `n * n` words from an existing
/// [`Lcg`] stream rather than reseeding, so callers can produce multiple
/// independent matrices from a single deterministic stream.
pub fn random_matrix_with_rng<const P: u64>(rng: &mut Lcg, n: usize) -> Vec<Fp<P>> {
    (0..n * n)
        .map(|_| Fp::<P>::new(rng.next_u64() % P))
        .collect()
}

/// Brute-force permanental-rank oracle for a row-major `n × k` matrix with
/// `k ≤ n`.
///
/// Returns [`PermanentalRank::Deficient`] exactly when every `k × k` row
/// submatrix has zero permanent. This is the independent cross-check for
/// [`crate::permanent::permanental_rank_status`] and deliberately **shares no
/// code path with it**:
///
/// * row subsets come from a scan over all `2^n` bitmasks, keeping those of
///   popcount `k`, rather than from a lexicographic combination vector;
/// * each `k × k` permanent is the direct `S_k` definition
///   `sum over sigma of prod_i A[i, sigma(i)]`, with the `k!` permutations
///   decoded from the factorial number system, rather than Ryser's
///   inclusion-exclusion formula over a Gray-code subset walk;
/// * no subset is skipped — the scan never exits early.
///
/// The only thing it shares with the predicate is the [`PermanentalRank`]
/// return vocabulary, so that the two decisions compare directly.
///
/// # Panics
///
/// Panics if `k > n`, if `matrix.len() != n * k`, or if `n > 63` (the subset
/// scan holds its mask in a single `u64`).
///
/// # Complexity
///
/// `O(2^n · k · k!)` field operations.
pub fn permanental_rank_bruteforce<F: FiniteField>(
    matrix: &[F],
    n: usize,
    k: usize,
) -> PermanentalRank {
    assert!(
        k <= n,
        "permanental_rank_bruteforce: k ({k}) must not exceed n ({n})",
    );
    assert_eq!(
        matrix.len(),
        n * k,
        "permanental_rank_bruteforce: matrix.len() ({}) must equal n * k ({})",
        matrix.len(),
        n * k,
    );
    assert!(
        n <= 63,
        "permanental_rank_bruteforce: n ({n}) exceeds the single-u64 subset mask's n <= 63 bound",
    );

    // The empty submatrix has permanent 1, so per-rank(A) = 0 is not < 0.
    if k == 0 {
        return PermanentalRank::Full;
    }

    let mut deficient = true;
    let mut rows: Vec<usize> = Vec::with_capacity(k);
    let mut submatrix: Vec<F> = Vec::with_capacity(k * k);

    for mask in 0u64..(1u64 << n) {
        if mask.count_ones() as usize != k {
            continue;
        }
        rows.clear();
        for row in 0..n {
            if (mask >> row) & 1 == 1 {
                rows.push(row);
            }
        }
        submatrix.clear();
        for &row in &rows {
            submatrix.extend_from_slice(&matrix[row * k..(row + 1) * k]);
        }
        if !permanent_permutation_sum::<F>(&submatrix, k).is_zero() {
            // No early exit: the whole scan runs so that the oracle's answer
            // never depends on subset order.
            deficient = false;
        }
    }

    if deficient {
        PermanentalRank::Deficient
    } else {
        PermanentalRank::Full
    }
}

/// Permanent of a `k × k` matrix from the `S_k` definition, `k ≥ 1`.
///
/// Enumerates all `k!` permutations by decoding each index in `0..k!` through
/// the factorial number system (radices `k, k-1, ..., 1`), which is a
/// bijection onto `S_k`, and accumulates `prod_i A[i, sigma(i)]`.
fn permanent_permutation_sum<F: FiniteField>(matrix: &[F], k: usize) -> F {
    debug_assert!(k >= 1 && matrix.len() == k * k);

    let mut total = matrix[0].zero_like();
    let one = matrix[0].one_like();
    let factorial: usize = (1..=k).product();

    let mut available: Vec<usize> = Vec::with_capacity(k);
    for code in 0..factorial {
        available.clear();
        available.extend(0..k);

        let mut rest = code;
        let mut term = one.clone();
        for row in 0..k {
            let radix = available.len();
            let pick = rest % radix;
            rest /= radix;
            term = term * &matrix[row * k + available.remove(pick)];
        }
        total += term;
    }

    total
}

/// Convert Unix epoch seconds to a `(year, month, day)` UTC tuple via the
/// Howard Hinnant civil-from-days algorithm; negative `secs` give pre-1970
/// dates.
///
/// Inlined to keep `chrono`/`time` out of the crate's dependencies.
pub fn unix_secs_to_ymd(secs: i64) -> (i32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y_final = if m <= 2 { y + 1 } else { y };
    (y_final, m, d)
}

/// Format today's UTC date as `YYYY-MM-DD`. Respects the `SA_DATE` env
/// variable for reproducible benchmark output paths.
pub fn today_yyyy_mm_dd() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    if let Ok(s) = std::env::var("SA_DATE") {
        return s;
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = unix_secs_to_ymd(secs);
    format!("{y:04}-{m:02}-{d:02}")
}
