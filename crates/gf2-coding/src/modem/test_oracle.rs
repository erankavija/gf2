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
