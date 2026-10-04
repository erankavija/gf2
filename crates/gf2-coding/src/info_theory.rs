//! Information-theory utilities: SNR-unit conversion, binary-input AWGN
//! (BI-AWGN) capacity and dispersion, and the BPSK Shannon limit.
//!
//! # SNR convention
//!
//! The channel primitives take the symbol SNR `Es/N0` in dB. For BPSK
//! (antipodal amplitude `a`, real noise variance `σ² = N0/2`),
//! `Es/N0 = a²/(2σ²)`. A code of rate `R` mapped onto a constellation with
//! `m` bits per symbol carries `Es = m·R·Eb`; [`ebn0_to_esn0`] and
//! [`esn0_to_ebn0`] are the one place that factor is applied. For BPSK
//! `m = 1`, so `Es/N0 = R·Eb/N0`.
//!
//! Capacities and dispersions are in bits per channel use and bits² per
//! channel use.

/// Converts Es/N0 (dB) to Eb/N0 (dB): `Eb/N0 = Es/N0 − 10·log10(m·R)`.
///
/// # Arguments
///
/// - `es_n0_db`: Symbol energy per noise power spectral density, in dB.
/// - `bits_per_symbol`: Coded bits per constellation symbol `m` (1 for
///   BPSK, 4 for 16-QAM).
/// - `code_rate`: Code rate `R` as a fraction (0.5 for rate 1/2).
pub fn esn0_to_ebn0(es_n0_db: f64, bits_per_symbol: usize, code_rate: f64) -> f64 {
    es_n0_db - 10.0 * (bits_per_symbol as f64 * code_rate).log10()
}

/// Converts Eb/N0 (dB) to Es/N0 (dB): `Es/N0 = Eb/N0 + 10·log10(m·R)`.
///
/// Arguments as for [`esn0_to_ebn0`].
pub fn ebn0_to_esn0(eb_n0_db: f64, bits_per_symbol: usize, code_rate: f64) -> f64 {
    eb_n0_db + 10.0 * (bits_per_symbol as f64 * code_rate).log10()
}

/// Half-width of the standard-normal integration window, in standard
/// deviations. The Gaussian weight beyond it is below 1e-30.
const Z_MAX: f64 = 12.0;

/// Trapezoidal-rule node count over `[−Z_MAX, Z_MAX]`. The integrand is
/// smooth and Gaussian-weighted, so the rule converges spectrally.
const NUM_NODES: usize = 2400;

/// Evaluates `f(i(z))` for the BI-AWGN information density at each node and
/// returns the Gaussian-weighted mean, `E[f(i)]`.
///
/// With `x = +1` sent and `y = a + z`, `z ~ N(0, 1)`, `a = sqrt(2·Es/N0)`,
/// the information density is `i = 1 − log2(1 + exp(−2a·y))` bits.
fn bi_awgn_expectation(es_n0_db: f64, f: impl Fn(f64) -> f64) -> f64 {
    let es_n0 = 10.0_f64.powf(es_n0_db / 10.0);
    let a = (2.0 * es_n0).sqrt();
    let dz = 2.0 * Z_MAX / NUM_NODES as f64;
    let inv_sqrt_2pi = 1.0 / (2.0 * std::f64::consts::PI).sqrt();

    let mut acc = 0.0;
    for k in 0..=NUM_NODES {
        let z = -Z_MAX + k as f64 * dz;
        let llr = 2.0 * a * (a + z);
        // log(1 + e^{−llr}) evaluated without overflow.
        let softplus = (-llr).max(0.0) + (-llr.abs()).exp().ln_1p();
        let density = 1.0 - softplus / std::f64::consts::LN_2;
        let weight = if k == 0 || k == NUM_NODES { 0.5 } else { 1.0 };
        acc += weight * (-0.5 * z * z).exp() * inv_sqrt_2pi * f(density);
    }
    acc * dz
}

/// Capacity of the binary-input AWGN channel (BPSK with equiprobable inputs)
/// at symbol SNR `es_n0_db`, in bits per channel use.
///
/// `C = E[i]` with `i = 1 − log2(1 + exp(−2a·y))`, `y ~ N(a, 1)`,
/// `a = sqrt(2·Es/N0)`; this is the BI-AWGN capacity of `@/citation/PPV2010` with
/// channel SNR `P = 2·Es/N0`. To evaluate at a bit SNR, convert with
/// [`ebn0_to_esn0`] using `bits_per_symbol = 1` and the code rate.
///
/// # Complexity
///
/// O(N) in the fixed quadrature node count N.
pub fn bi_awgn_capacity(es_n0_db: f64) -> f64 {
    bi_awgn_expectation(es_n0_db, |i| i).clamp(0.0, 1.0)
}

/// Channel dispersion of the binary-input AWGN channel at symbol SNR
/// `es_n0_db`, in bits² per channel use.
///
/// `V = Var[i]` for the information density of [`bi_awgn_capacity`]
/// (the BI-AWGN dispersion of `@/citation/PPV2010` with `P = 2·Es/N0`). It is the
/// second-order term of the normal approximation
/// `R ≈ C − sqrt(V/n)·Q⁻¹(ε)`.
///
/// # Complexity
///
/// O(N) in the fixed quadrature node count N.
pub fn bi_awgn_dispersion(es_n0_db: f64) -> f64 {
    let mean = bi_awgn_expectation(es_n0_db, |i| i);
    bi_awgn_expectation(es_n0_db, |i| (i - mean) * (i - mean)).max(0.0)
}

/// Minimum Eb/N0 (dB) for reliable BPSK transmission at code rate `rate`
/// over AWGN: the Eb/N0 at which
/// `bi_awgn_capacity(ebn0_to_esn0(Eb/N0, 1, rate)) = rate`.
///
/// Returns `f64::INFINITY` for `rate = 1`, since the BI-AWGN capacity stays
/// below one bit at every finite SNR. As `rate → 0` the limit approaches
/// `10·log10(ln 2) ≈ −1.59 dB`.
///
/// # Panics
///
/// Panics if `rate` is not in `(0, 1]`.
///
/// # Complexity
///
/// A fixed number of bisection steps, each one capacity evaluation.
pub fn shannon_limit(rate: f64) -> f64 {
    assert!(
        rate > 0.0 && rate <= 1.0,
        "rate must be in (0, 1], got {rate}"
    );
    if rate == 1.0 {
        return f64::INFINITY;
    }

    // Capacity is strictly increasing in Eb/N0 at fixed rate. The lower end
    // sits below the ultimate limit 10·log10(ln 2), where capacity < rate for
    // every rate; the upper end exceeds the limit of every rate whose
    // capacity gap is resolvable in f64.
    let mut low = -1.6_f64;
    let mut high = 40.0_f64;
    for _ in 0..64 {
        let mid = 0.5 * (low + high);
        if bi_awgn_capacity(ebn0_to_esn0(mid, 1, rate)) < rate {
            low = mid;
        } else {
            high = mid;
        }
    }
    0.5 * (low + high)
}
