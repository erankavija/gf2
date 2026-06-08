//! Statistical sanity tests for the AWGN, Rayleigh, and Rician channel stages
//! (issue `db9836e4`, deliverable 5).
//!
//! Each channel is exercised over **>= 10000 samples** and BOTH the mean and the
//! variance of the relevant quantity are asserted against analytical
//! expectations, with tolerances wide enough to be robust to Monte-Carlo noise
//! yet tight enough to catch a wrong moment.
//!
//! # Analytical moments asserted
//!
//! * **AWGN** (per-axis added noise `n ~ N(0, sigma^2)`):
//!   - `E[n] = 0`
//!   - `Var[n] = sigma^2`, where `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))`.
//!
//! * **Rayleigh** (fading coefficient `h ~ CN(0, 1)`, recovered at high SNR as
//!   `r ≈ h` for a unit transmitted symbol):
//!   - Per-axis fading component `h_r ~ N(0, 1/2)`: `E[h_r] = 0`, `Var[h_r] = 1/2`.
//!   - Envelope power `|h|^2 ~ Exponential(1)`: `E[|h|^2] = 1`, `Var[|h|^2] = 1`.
//!
//! * **Rician** (K-factor `K`, `h = sqrt(K/(K+1)) + sqrt(1/(K+1)) * CN(0,1)`):
//!   - `E[|h|^2] = 1` for all K (unit-power normalization).
//!   - `Var[|h|^2] = (2K + 1) / (K + 1)^2`. For K = 4 this is
//!     `9 / 25 = 0.36`, materially LOWER than Rayleigh's `1.0` — the
//!     line-of-sight component reduces fading variance. We assert both the
//!     analytical value (within tolerance) AND that it is well below 1.0.
//!
//! # Test tier
//!
//! 10000-sample passes through these lightweight per-symbol kernels run in a few
//! milliseconds, comfortably under the 5 s fast-tier limit, so all tests here
//! are fast-tier (un-ignored).

use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};

/// Sample count for the statistical tests (>= 10000 per deliverable 5).
const N: usize = 10_000;

/// Build a single-frame batch of `syms` symbols at transmitted value (i0, q0).
fn const_batch(syms: usize, i0: f32, q0: f32) -> SymbolBatch {
    SymbolBatch::new(vec![vec![i0; syms]], vec![vec![q0; syms]])
}

/// Sample mean and (population) variance of an iterator of f64 values.
fn mean_var(samples: impl Iterator<Item = f64>) -> (f64, f64) {
    let v: Vec<f64> = samples.collect();
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    (mean, var)
}

/// AWGN: per-axis noise mean ~ 0 and variance ~ sigma^2 over 10000 samples.
#[test]
fn test_awgn_noise_mean_and_variance() {
    let es_n0_db = 3.0_f32;
    let ch = Awgn::new(es_n0_db, 4);
    let sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(es_n0_db as f64 / 10.0));

    // Transmit (1, 0); noise = received - transmitted.
    let input = const_batch(N, 1.0, 0.0);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0xDEAD_BEEF);
    ch.apply(&mut batch, &mut rng);

    // Combine I and Q noise samples (both ~ N(0, sigma^2)).
    let noise = batch.i[0]
        .iter()
        .zip(input.i[0].iter())
        .map(|(&r, &t)| (r - t) as f64)
        .chain(
            batch.q[0]
                .iter()
                .zip(input.q[0].iter())
                .map(|(&r, &t)| (r - t) as f64),
        );
    let (mean, var) = mean_var(noise);

    // E[n] = 0: tolerance ~ several * sigma / sqrt(2N).
    assert!(
        mean.abs() < 0.05,
        "AWGN noise mean {mean:.6} too far from 0 (sigma^2 = {sigma_sq:.6})"
    );
    // Var[n] = sigma^2 within 5%.
    let rel = ((var - sigma_sq) / sigma_sq).abs();
    assert!(
        rel < 0.05,
        "AWGN noise variance {var:.6} vs sigma^2 {sigma_sq:.6} (rel err {:.2}% > 5%)",
        rel * 100.0
    );
}

/// Rayleigh: at high SNR `r ≈ h`. Assert per-axis fading-component mean ~ 0 and
/// variance ~ 0.5, AND envelope-power `|h|^2` mean ~ 1 and variance ~ 1.
#[test]
fn test_rayleigh_fading_moments() {
    // Very high SNR so noise is negligible (< 0.1% of fading power): r ≈ h.
    let ch = Rayleigh::new(40.0, 4);
    let input = const_batch(N, 1.0, 0.0); // x = 1+0j → r ≈ h
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0xCAFE_BABE);
    ch.apply(&mut batch, &mut rng);

    // Per-axis fading components (h_r from I, h_i from Q), each ~ N(0, 1/2).
    let comp = batch.i[0]
        .iter()
        .map(|&v| v as f64)
        .chain(batch.q[0].iter().map(|&v| v as f64));
    let (comp_mean, comp_var) = mean_var(comp);
    assert!(
        comp_mean.abs() < 0.05,
        "Rayleigh fading-component mean {comp_mean:.6} too far from 0"
    );
    // Var[h_r] = 1/2 within 6%.
    let rel = ((comp_var - 0.5) / 0.5).abs();
    assert!(
        rel < 0.06,
        "Rayleigh fading-component variance {comp_var:.6} vs 0.5 (rel err {:.2}% > 6%)",
        rel * 100.0
    );

    // Envelope power |h|^2 ~ Exponential(1): mean 1, variance 1.
    let power = batch.i[0]
        .iter()
        .zip(batch.q[0].iter())
        .map(|(&ri, &rq)| (ri as f64).powi(2) + (rq as f64).powi(2));
    let (pow_mean, pow_var) = mean_var(power);
    assert!(
        (pow_mean - 1.0).abs() < 0.05,
        "Rayleigh E[|h|^2] {pow_mean:.6} too far from 1.0"
    );
    // Var[|h|^2] = 1 (exponential): wider tolerance — 4th-moment estimator has
    // higher Monte-Carlo variance.
    assert!(
        (pow_var - 1.0).abs() < 0.12,
        "Rayleigh Var[|h|^2] {pow_var:.6} too far from 1.0"
    );
}

/// Rician (K=4): `E[|h|^2] = 1` AND `Var[|h|^2] = (2K+1)/(K+1)^2 = 9/25 = 0.36`,
/// which is materially LOWER than Rayleigh's 1.0 (LOS reduces fading variance).
#[test]
fn test_rician_fading_moments() {
    let k = 4.0_f32;
    // Analytical Var[|h|^2] for Rician: (2K+1)/(K+1)^2.
    let expected_var = (2.0 * k as f64 + 1.0) / (k as f64 + 1.0).powi(2); // 0.36

    let ch = Rician::new(40.0, 4, k); // high SNR so r ≈ h
    let input = const_batch(N, 1.0, 0.0);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0x0FF5_1CE5);
    ch.apply(&mut batch, &mut rng);

    let power = batch.i[0]
        .iter()
        .zip(batch.q[0].iter())
        .map(|(&ri, &rq)| (ri as f64).powi(2) + (rq as f64).powi(2));
    let (pow_mean, pow_var) = mean_var(power);

    // E[|h|^2] = 1 within 5%.
    assert!(
        (pow_mean - 1.0).abs() < 0.05,
        "Rician E[|h|^2] {pow_mean:.6} too far from 1.0"
    );
    // Var[|h|^2] = (2K+1)/(K+1)^2 within 12% (4th-moment estimator).
    let rel = ((pow_var - expected_var) / expected_var).abs();
    assert!(
        rel < 0.12,
        "Rician Var[|h|^2] {pow_var:.6} vs analytical {expected_var:.6} (rel err {:.2}% > 12%)",
        rel * 100.0
    );
    // And materially lower than Rayleigh's variance of 1.0 (LOS effect).
    assert!(
        pow_var < 0.6,
        "Rician Var[|h|^2] {pow_var:.6} should be well below Rayleigh's 1.0"
    );
}

/// Fast smoke: Stage::process returns a batch with the same dimensions as input.
#[test]
fn test_stage_process_dimensions() {
    use gf2_sim::channels::awgn::ChannelScratch;
    use gf2_sim::stage::Stage;

    let awgn = Awgn::new(6.25, 4);
    let rayleigh = Rayleigh::new(6.25, 4);
    let rician = Rician::new(6.25, 4, 2.0);

    let input = const_batch(
        64,
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    let mut scratch = ChannelScratch::default();

    let out_awgn = awgn.process(&input, &mut scratch).unwrap();
    assert_eq!(out_awgn.i.len(), 1);
    assert_eq!(out_awgn.i[0].len(), 64);

    let out_rayleigh = rayleigh.process(&input, &mut scratch).unwrap();
    assert_eq!(out_rayleigh.i[0].len(), 64);

    let out_rician = rician.process(&input, &mut scratch).unwrap();
    assert_eq!(out_rician.i[0].len(), 64);
}

/// Fast smoke: all three channels expose ExecutionClass::CpuOnly.
#[test]
fn test_all_channels_are_cpu_only() {
    use gf2_sim::stage::{ExecutionClass, Stage};

    let awgn = Awgn::new(6.25, 4);
    let rayleigh = Rayleigh::new(6.25, 4);
    let rician = Rician::new(6.25, 4, 2.0);

    assert_eq!(awgn.execution_class(), ExecutionClass::CpuOnly);
    assert_eq!(rayleigh.execution_class(), ExecutionClass::CpuOnly);
    assert_eq!(rician.execution_class(), ExecutionClass::CpuOnly);
}
