//! Test-only helpers shared across the crate's test modules.
//!
//! Centralises the deterministic pseudo-random matrix generators used by the
//! `permanent_*` algorithm family's cross-check tests, plus the brute-force
//! oracles those tests compare the production kernels against. All random
//! helpers route through the workspace SSOT RNG [`gf2_core::rng::Lcg`] so that
//! seed values reproduce bit-identical streams across modules.

use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

use crate::permanent::PermanentalRank;

/// Generate a deterministic pseudo-random `n × n` matrix of [`Fp<P>`] elements,
/// row-major.
///
/// Internally constructs a fresh [`Lcg`] from `seed` and draws `n * n` words,
/// reducing each modulo `P` to obtain a canonical `Fp<P>` value. The output
/// layout matches the row-major convention used by every `permanent_*` kernel
/// in this crate.
///
/// # Arguments
///
/// * `n`    — matrix dimension; result has length `n * n`.
/// * `seed` — seed for the workspace SSOT [`Lcg`] RNG.
///
/// # Examples
///
/// ```
/// use gf2_algebra::testutil::random_matrix;
///
/// let mat = random_matrix::<3>(4, 0xdead_beef);
/// assert_eq!(mat.len(), 16);
/// // Same seed reproduces bit-identical output.
/// assert_eq!(mat, random_matrix::<3>(4, 0xdead_beef));
/// ```
///
/// # Complexity
///
/// `O(n^2)` — one [`Lcg::next_u64`] call per entry.
pub fn random_matrix<const P: u64>(n: usize, seed: u64) -> Vec<Fp<P>> {
    let mut rng = Lcg::new(seed);
    (0..n * n)
        .map(|_| Fp::<P>::new(rng.next_u64() % P))
        .collect()
}

/// Same as [`random_matrix`] but draws from an existing [`Lcg`] stream rather
/// than reseeding, so callers can produce multiple independent matrices from a
/// single deterministic stream.
///
/// # Arguments
///
/// * `rng` — mutable [`Lcg`] state to draw from; advanced by `n * n` words.
/// * `n`   — matrix dimension; result has length `n * n`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::testutil::random_matrix_with_rng;
/// use gf2_core::rng::Lcg;
///
/// let mut rng = Lcg::new(0xfeed_face);
/// let m1 = random_matrix_with_rng::<3>(&mut rng, 4);
/// let m2 = random_matrix_with_rng::<3>(&mut rng, 4);
/// assert_ne!(m1, m2); // independent draws from the same stream
/// ```
///
/// # Complexity
///
/// `O(n^2)` — one [`Lcg::next_u64`] call per entry.
pub fn random_matrix_with_rng<const P: u64>(rng: &mut Lcg, n: usize) -> Vec<Fp<P>> {
    (0..n * n)
        .map(|_| Fp::<P>::new(rng.next_u64() % P))
        .collect()
}

/// Brute-force permanental-rank oracle for an `n × k` matrix with `k ≤ n`.
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
/// # Arguments
///
/// * `matrix` — flat row-major slice of `n * k` field elements.
/// * `n` — number of rows.
/// * `k` — number of columns; must satisfy `k <= n`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::PermanentalRank;
/// use gf2_algebra::testutil::permanental_rank_bruteforce;
/// use gf2_core::gfp::Fp;
///
/// // A zero column makes every 2x2 row-submatrix permanent vanish.
/// let a: Vec<Fp<5>> = [1, 0, 2, 0, 3, 0].iter().map(|&v| Fp::<5>::new(v)).collect();
/// assert_eq!(
///     permanental_rank_bruteforce::<Fp<5>>(&a, 3, 2),
///     PermanentalRank::Deficient
/// );
/// ```
///
/// # Panics
///
/// Panics if `k > n`, if `matrix.len() != n * k`, or if `n > 63` (the subset
/// scan holds its mask in a single `u64`).
///
/// # Complexity
///
/// `O(2^n · k · k!)` field operations — exponential in both dimensions by
/// construction, since an oracle that shortcut anything would stop being one.
/// Intended for exhaustive validation at the tiny shapes where `q^(n·k)` is
/// itself enumerable.
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
/// bijection onto `S_k`, and accumulates `prod_i A[i, sigma(i)]`. No
/// inclusion-exclusion, no Gray code, no shared helper with the crate's
/// production permanent kernels.
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
/// Howard Hinnant civil-from-days algorithm.
///
/// Inlined to avoid pulling `chrono`/`time` into `gf2-algebra` for the
/// benchmark/repro examples (`paper_repro_slope`, `parallel_chunk_sweep`,
/// `parallel_scaling_sweep`) that need a date string for the CSV filename.
///
/// # Arguments
///
/// * `secs` — Unix epoch seconds (signed; pre-1970 dates produce
///   correct historical UTC dates).
///
/// # Examples
///
/// ```
/// use gf2_algebra::testutil::unix_secs_to_ymd;
///
/// assert_eq!(unix_secs_to_ymd(0), (1970, 1, 1));
/// assert_eq!(unix_secs_to_ymd(946_684_800), (2000, 1, 1));
/// // 2026-05-11 UTC midnight = 1_778_457_600 seconds.
/// assert_eq!(unix_secs_to_ymd(1_778_457_600), (2026, 5, 11));
/// // Leap-day handling
/// assert_eq!(unix_secs_to_ymd(951_782_400), (2000, 2, 29));
/// ```
///
/// # Complexity
///
/// `O(1)` — fixed-shape integer arithmetic per call.
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
///
/// # Examples
///
/// ```
/// use gf2_algebra::testutil::today_yyyy_mm_dd;
///
/// // Override via env var:
/// std::env::set_var("SA_DATE", "2026-05-11");
/// assert_eq!(today_yyyy_mm_dd(), "2026-05-11");
/// std::env::remove_var("SA_DATE");
/// // Without override returns today's UTC date; format invariant verified.
/// let today = today_yyyy_mm_dd();
/// assert_eq!(today.len(), 10);
/// assert_eq!(&today[4..5], "-");
/// assert_eq!(&today[7..8], "-");
/// ```
///
/// # Complexity
///
/// `O(1)` plus one `SystemTime::now()` syscall.
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
