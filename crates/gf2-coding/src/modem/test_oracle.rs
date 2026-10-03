//! Brute-force log-MAP oracle and seeded input generators for modem
//! demapper cross-checks, shared by unit and integration tests.

use super::bit_pack::bit_at_msb_first;

/// Deterministic LCG behind [`permutation`], [`bit_stream`] and [`label_stream`].
#[doc(hidden)]
pub use gf2_core::rng::Lcg;

/// Builds a deterministic Fisher-Yates permutation of `[0, n)` as a
/// `Vec<u16>`, seeded by `seed`.
///
/// # Panics
///
/// Panics if `n > u16::MAX as usize + 1`.
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
/// # Panics
///
/// Panics if `n == 0` or `n > u16::MAX as usize + 1`.
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
/// with a numerical-stability min-shift; a positive value means `bit == 0`
/// is more likely. `points` and `labels` are the post-normalization
/// constellation and its MSB-first labels, `(h_i, h_q)` is the complex
/// channel gain (`(1.0, 0.0)` for AWGN), and `b = 0` selects the MSB.
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
