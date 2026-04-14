//! Information-theory utilities: Shannon capacity and minimum Eb/N0 limits.
//!
//! These helpers are modem-agnostic — they describe the AWGN channel itself,
//! not any particular mapper/demapper. They moved here from the old
//! `channel` module when the BPSK-specific surface was deleted as part of
//! the modem-framework cleanup.
//!
//! All capacities are expressed as bits per channel use for the BPSK
//! constellation, which is the standard reference for binary coding
//! research.

/// Computes the Shannon capacity for BPSK over AWGN at the given Eb/N0.
///
/// For BPSK modulation, the channel capacity in bits per channel use is:
/// $$
/// C = 1 - \int_{-\infty}^{\infty} p(y) \log_2\left(\frac{1}{p(y|+1) + p(y|-1)}\right) dy
/// $$
///
/// where $p(y|x)$ is the Gaussian conditional density.
///
/// # Arguments
///
/// * `eb_n0_db` - Energy per bit to noise ratio in dB (= Es/N0 for BPSK)
///
/// # Returns
///
/// Channel capacity in bits per channel use (0 to 1.0).
///
/// # Examples
///
/// ```
/// use gf2_coding::info_theory::shannon_capacity;
///
/// let capacity = shannon_capacity(3.0);
/// assert!(capacity > 0.7 && capacity < 0.8);
/// ```
pub fn shannon_capacity(eb_n0_db: f64) -> f64 {
    let snr = 10.0_f64.powf(eb_n0_db / 10.0);
    shannon_capacity_numerical(snr)
}

/// Returns the minimum Eb/N0 (in dB) required to achieve a given rate.
///
/// This is the Shannon limit: the theoretical minimum SNR needed for
/// reliable communication at the specified rate over a BPSK AWGN channel.
///
/// For rate R, finds Eb/N0 such that `shannon_capacity(Eb/N0) = R`.
///
/// # Panics
///
/// Panics if `rate` is not in `(0, 1]`.
///
/// # Examples
///
/// ```
/// use gf2_coding::info_theory::shannon_limit;
///
/// // Rate 1/2 code requires approximately 0.2 dB at Shannon limit
/// let eb_n0_min = shannon_limit(0.5);
/// assert!(eb_n0_min < 1.0 && eb_n0_min > -1.0);
/// ```
pub fn shannon_limit(rate: f64) -> f64 {
    assert!(rate > 0.0 && rate <= 1.0, "Rate must be in (0, 1]");

    // Binary search for Eb/N0 where capacity equals rate
    let mut low = -10.0_f64;
    let mut high = 25.0_f64;

    for _ in 0..60 {
        let mid = (low + high) / 2.0;
        let capacity = shannon_capacity(mid);

        if (capacity - rate).abs() < 1e-6 {
            return mid;
        }

        if capacity > rate {
            high = mid;
        } else {
            low = mid;
        }
    }

    (low + high) / 2.0
}

/// Numerically computes Shannon capacity for BPSK at given SNR (Eb/N0).
///
/// For BPSK modulation over AWGN, the capacity is:
/// `C = 1 - integral_{-inf}^{inf} f(y) log2(1 + exp(-2*sqrt(SNR)*y)) dy`
/// where f(y) is N(sqrt(SNR), 1) distribution and SNR = Eb/N0.
fn shannon_capacity_numerical(eb_n0_linear: f64) -> f64 {
    let sqrt_snr = eb_n0_linear.sqrt();

    let num_points = 1000;
    let y_max = sqrt_snr + 6.0;
    let dy = 2.0 * y_max / num_points as f64;

    let sqrt_2pi = (2.0 * std::f64::consts::PI).sqrt();
    let mut integral = 0.0;

    for i in 0..=num_points {
        let y = -y_max + i as f64 * dy;

        let f_y = (-(y - sqrt_snr).powi(2) / 2.0).exp() / sqrt_2pi;

        let arg = -2.0 * sqrt_snr * y;
        let log_term = if arg > 20.0 {
            arg / std::f64::consts::LN_2
        } else if arg < -20.0 {
            0.0
        } else {
            (1.0 + arg.exp()).log2()
        };

        let weight = if i == 0 || i == num_points { 0.5 } else { 1.0 };
        integral += weight * f_y * log_term;
    }

    let capacity = 1.0 - integral * dy;
    capacity.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_capacity_high_snr() {
        let capacity = shannon_capacity(20.0);
        assert!(capacity > 0.95);
    }

    #[test]
    fn test_shannon_capacity_low_snr() {
        let capacity = shannon_capacity(-10.0);
        assert!(capacity < 0.2);
    }

    #[test]
    fn test_shannon_limit_rate_half() {
        let eb_n0_min = shannon_limit(0.5);
        assert!(eb_n0_min > -1.0 && eb_n0_min < 1.0);
    }

    #[test]
    fn test_shannon_limit_rate_high() {
        let eb_n0_min = shannon_limit(0.9);
        assert!(eb_n0_min > 2.0);
    }
}
