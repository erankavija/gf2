//! Shared brute-force log-MAP oracle for modem demapper cross-checks.
//!
//! Kept as a `#[doc(hidden)] pub` module (re-exported from
//! [`super::mod`](super)) so both internal unit tests and out-of-crate
//! integration tests can compute LLRs from a post-normalization
//! `(points, labels)` snapshot through a single implementation.
//!
//! This is test-support infrastructure; it is not part of the public
//! modem API and carries no stability guarantees.

use super::bit_pack::bit_at_msb_first;

/// Re-export of the workspace SSOT deterministic LCG. The primitive
/// lives in `gf2-core` so both `gf2-coding` (this crate) and
/// `gf2-kernels-simd` (a dependency) can share one implementation
/// without introducing a dependency cycle. All modem test RNG usage
/// must go through this re-export; modem-specific helpers
/// ([`bit_stream`], [`permutation`], [`label_stream`]) live as free
/// functions in this module and take a seed.
#[doc(hidden)]
pub use gf2_core::test_rng::Lcg;

/// Builds a deterministic Fisher-Yates permutation of `[0, n)` as a
/// `Vec<u16>`, seeded by `seed`.
///
/// # Arguments
///
/// * `seed` — 64-bit seed for the internal [`Lcg`].
/// * `n` — Size of the permutation; must fit in `u16`.
///
/// # Panics
///
/// Panics if `n > u16::MAX as usize + 1`.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::test_oracle::permutation;
///
/// let perm = permutation(0xA11CE, 8);
/// assert_eq!(perm.len(), 8);
/// let mut sorted = perm.clone();
/// sorted.sort_unstable();
/// assert_eq!(sorted, (0..8u16).collect::<Vec<_>>());
/// ```
///
/// # Complexity
///
/// O(`n`).
pub fn permutation(seed: u64, n: usize) -> Vec<u16> {
    assert!(
        n <= u16::MAX as usize + 1,
        "permutation size {n} exceeds u16 range"
    );
    let mut perm: Vec<u16> = (0..n as u16).collect();
    let mut rng = Lcg::new(seed);
    for i in (1..n).rev() {
        let j = rng.next_bounded_usize(i + 1);
        perm.swap(i, j);
    }
    perm
}

/// Builds a deterministic pseudo-random bit stream of length `n_bits`,
/// seeded by `seed`.
///
/// # Arguments
///
/// * `seed` — 64-bit seed for the internal [`Lcg`].
/// * `n_bits` — Number of bits to generate.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::test_oracle::bit_stream;
///
/// let bits = bit_stream(0xA11CE, 64);
/// assert_eq!(bits.len(), 64);
/// ```
///
/// # Complexity
///
/// O(`n_bits`).
pub fn bit_stream(seed: u64, n_bits: usize) -> Vec<bool> {
    let mut rng = Lcg::new(seed);
    let mut out = Vec::with_capacity(n_bits);
    for _ in 0..n_bits {
        out.push((rng.next_u64() & 1) == 1);
    }
    out
}

/// Builds a deterministic pseudo-random stream of `batch` label
/// integers drawn uniformly from `[0, n)`, seeded by `seed`.
///
/// # Arguments
///
/// * `seed` — 64-bit seed for the internal [`Lcg`].
/// * `batch` — Number of labels to generate.
/// * `n` — Exclusive label upper bound; must fit in `u16` and be `> 0`.
///
/// # Panics
///
/// Panics if `n == 0` or `n > u16::MAX as usize + 1`.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::test_oracle::label_stream;
///
/// let stream = label_stream(0xBEEF, 16, 4);
/// assert_eq!(stream.len(), 16);
/// assert!(stream.iter().all(|&v| v < 4));
/// ```
///
/// # Complexity
///
/// O(`batch`).
pub fn label_stream(seed: u64, batch: usize, n: usize) -> Vec<u16> {
    assert!(n > 0, "label_stream requires n > 0");
    assert!(
        n <= u16::MAX as usize + 1,
        "label_stream alphabet {n} exceeds u16 range"
    );
    let mut rng = Lcg::new(seed);
    let mut labels = Vec::with_capacity(batch);
    for _ in 0..batch {
        labels.push(rng.next_bounded_usize(n) as u16);
    }
    labels
}

/// Brute-force exact log-MAP LLR for a single received sample, bit
/// position, and total complex noise variance `N0 = 2 sigma^2`.
///
/// Computes
/// `log(sum_{j ∈ S0} exp(-d_j/N0)) - log(sum_{j ∈ S1} exp(-d_j/N0))`
/// with a numerical-stability min-shift. Operates directly on flat
/// `Vec<(f64, f64)>` / `Vec<u16>` snapshots of a post-normalization
/// `ModemSpec` so callers do not depend on `ModemSpec<S>` generics for
/// their oracle math.
///
/// Positive return value means `bit == 0` is more likely.
///
/// # Arguments
///
/// * `points` - Constellation points as `(I, Q)` pairs, post-normalization.
/// * `labels` - Per-point MSB-first labels (length = `points.len()`).
/// * `bits_per_symbol` - Label width in bits.
/// * `y_i`, `y_q` - Received sample.
/// * `h_i`, `h_q` - Complex channel gain (`(1.0, 0.0)` for pure AWGN).
/// * `n0` - Total complex noise variance (`2 sigma^2` convention).
/// * `b` - Bit position (MSB-first, `b = 0` is the MSB).
///
/// # Complexity
///
/// O(`points.len()`).
#[allow(clippy::too_many_arguments)]
pub fn brute_force_log_map_llr(
    points: &[(f64, f64)],
    labels: &[u16],
    bits_per_symbol: u8,
    y_i: f64,
    y_q: f64,
    h_i: f64,
    h_q: f64,
    n0: f64,
    b: u8,
) -> f64 {
    let dists: Vec<f64> = points
        .iter()
        .map(|&(pi, pq)| {
            let ei = y_i - (h_i * pi - h_q * pq);
            let eq = y_q - (h_i * pq + h_q * pi);
            (ei * ei + eq * eq) / n0
        })
        .collect();
    let d_min = dists.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut sum0 = 0.0_f64;
    let mut sum1 = 0.0_f64;
    for (j, &d) in dists.iter().enumerate() {
        let bit = bit_at_msb_first(labels[j], b, bits_per_symbol);
        let e = (d_min - d).exp();
        if bit == 0 {
            sum0 += e;
        } else {
            sum1 += e;
        }
    }
    sum0.ln() - sum1.ln()
}
