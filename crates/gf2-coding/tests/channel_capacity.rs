//! Canonical suite for the binary-input AWGN capacity, dispersion, SNR-unit
//! conversion, and BPSK Shannon limit in `gf2_coding::info_theory`.

use gf2_coding::info_theory::{
    bi_awgn_capacity, bi_awgn_dispersion, ebn0_to_esn0, esn0_to_ebn0, shannon_limit,
};
use gf2_coding::modem::awgn_link::unit_energy_sigma_sq_from_eb_n0_db;

/// BI-AWGN capacity and dispersion at Es/N0 points: (Es/N0 dB, C bits,
/// V bits²). Computed independently of this crate by adaptive Gauss–Kronrod
/// quadrature (relative tolerance 1e-14) of the mean and variance
/// of `i = 1 − log2(1 + exp(−2a(a + z)))`, `z ~ N(0, 1)`, `a = sqrt(2·Es/N0)`.
const REFERENCE_CAPACITY_DISPERSION: &[(f64, f64, f64)] = &[
    (-10.0, 0.131416082353, 0.31641800871),
    (-3.0, 0.48671359211, 0.659712378047),
    (0.0, 0.72145159079, 0.533271940479),
    (3.0, 0.912352116906, 0.223239060342),
    (6.0, 0.990263800775, 0.029690421597),
    (10.0, 0.99998332824, 5.83925621148e-05),
];

/// Published BPSK Shannon limits (minimum Eb/N0 in dB) at rates spanning
/// (0, 1): (rate, limit, tolerance). Each tolerance is half a unit of the
/// printed precision plus 0.001 dB.
///
/// - Rates 1/3, 1/2, 2/3, 3/4: `@/citation/Lentmaier2010` Table II, column
///   `(Eb/N0)_sh`, printed to 0.001 dB.
/// - Rates 1/6, 1/4: `@/citation/Ccsds2020` §3.3 (vertical asymptotes of
///   Figure 3-3), printed to 0.1 dB.
const PUBLISHED_SHANNON_LIMITS: &[(f64, f64, f64)] = &[
    (1.0 / 6.0, -1.1, 0.051),
    (0.25, -0.8, 0.051),
    (1.0 / 3.0, -0.495, 0.0015),
    (0.5, 0.187, 0.0015),
    (2.0 / 3.0, 1.059, 0.0015),
    (0.75, 1.626, 0.0015),
];

#[test]
fn test_capacity_and_dispersion_match_reference_quadrature() {
    for &(es_n0_db, c_ref, v_ref) in REFERENCE_CAPACITY_DISPERSION {
        let c = bi_awgn_capacity(es_n0_db);
        let v = bi_awgn_dispersion(es_n0_db);
        assert!(
            (c - c_ref).abs() < 1e-9,
            "C at Es/N0 = {es_n0_db} dB: got {c}, reference {c_ref}"
        );
        assert!(
            (v - v_ref).abs() < 1e-9,
            "V at Es/N0 = {es_n0_db} dB: got {v}, reference {v_ref}"
        );
    }
}

#[test]
fn test_shannon_limit_matches_published_values() {
    for &(rate, published_db, tol_db) in PUBLISHED_SHANNON_LIMITS {
        let limit_db = shannon_limit(rate);
        assert!(
            (limit_db - published_db).abs() <= tol_db,
            "rate {rate}: shannon_limit = {limit_db} dB, published {published_db} ± {tol_db} dB"
        );
    }
}

#[test]
fn test_shannon_limit_approaches_ultimate_limit_at_low_rate() {
    let ultimate_db = 10.0 * std::f64::consts::LN_2.log10();
    let limit_db = shannon_limit(1e-3);
    assert!(
        limit_db > ultimate_db && limit_db - ultimate_db < 0.01,
        "rate 1e-3: {limit_db} dB, ultimate limit {ultimate_db} dB"
    );
}

#[test]
fn test_shannon_limit_is_infinite_at_rate_one() {
    assert_eq!(shannon_limit(1.0), f64::INFINITY);
}

#[test]
#[should_panic(expected = "rate must be in (0, 1]")]
fn test_shannon_limit_rejects_zero_rate() {
    shannon_limit(0.0);
}

#[test]
fn test_ebn0_esn0_offset_is_rate_aware() {
    for &(m, rate) in &[(1usize, 0.9), (1, 1.0 / 3.0), (4, 0.5), (6, 0.75)] {
        let es_n0 = ebn0_to_esn0(2.0, m, rate);
        let offset = 10.0 * (m as f64 * rate).log10();
        assert!((es_n0 - 2.0 - offset).abs() < 1e-12);
        assert!((esn0_to_ebn0(es_n0, m, rate) - 2.0).abs() < 1e-12);
    }
}

#[test]
fn test_esn0_agrees_with_modem_link_noise_variance() {
    // Unit-energy real BPSK: sigma² = N0/2 = 1/(2·Es/N0).
    for &(rate, eb_n0_db) in &[(0.9, 3.0), (1.0 / 3.0, -0.5), (0.5, 1.0)] {
        let es_n0_lin = 10.0_f64.powf(ebn0_to_esn0(eb_n0_db, 1, rate) / 10.0);
        let sigma_sq = unit_energy_sigma_sq_from_eb_n0_db(1, rate, eb_n0_db);
        assert!(
            (sigma_sq - 1.0 / (2.0 * es_n0_lin)).abs() < 1e-12 * sigma_sq,
            "rate {rate}, Eb/N0 {eb_n0_db} dB"
        );
    }
}

#[test]
fn test_dispersion_vanishes_at_high_snr() {
    assert!(bi_awgn_dispersion(20.0) < 1e-12);
    assert!(bi_awgn_capacity(20.0) > 1.0 - 1e-12);
}

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn capacity_in_unit_interval_and_increasing(
            es_n0_db in -20.0..20.0,
            delta in 0.1..5.0
        ) {
            let low = bi_awgn_capacity(es_n0_db);
            let high = bi_awgn_capacity(es_n0_db + delta);
            prop_assert!((0.0..=1.0).contains(&low));
            prop_assert!(high > low || high == 1.0);
        }

        #[test]
        fn dispersion_nonnegative(es_n0_db in -20.0..20.0) {
            prop_assert!(bi_awgn_dispersion(es_n0_db) >= 0.0);
        }

        #[test]
        fn capacity_at_shannon_limit_equals_rate(rate in 0.02..0.98) {
            let limit_db = shannon_limit(rate);
            let c = bi_awgn_capacity(ebn0_to_esn0(limit_db, 1, rate));
            prop_assert!((c - rate).abs() < 1e-9, "rate {}: C = {}", rate, c);
        }

        #[test]
        fn shannon_limit_increases_with_rate(
            rate_low in 0.02f64..0.9f64,
            delta in 0.01f64..0.08f64
        ) {
            prop_assert!(shannon_limit(rate_low + delta) > shannon_limit(rate_low));
        }
    }
}
