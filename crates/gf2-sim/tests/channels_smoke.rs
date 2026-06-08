//! Statistical sanity tests for the AWGN, Rayleigh, and Rician channel stages
//! (issue `db9836e4`).
//!
//! Each test passes a synthetic IQ batch through the channel and verifies that
//! the statistical moments of the received signal match analytical expectations
//! within a tolerance wide enough to be robust yet tight enough to catch a
//! wrong variance formula.
//!
//! # Test tier
//!
//! The fast-tier smoke tests run at most ~1000 symbols per channel and are
//! kept well under the 5 s nextest limit. Any test that would exceed 5 s is
//! marked `#[ignore = "sim: <description>"]`.
//!
//! # What is verified
//!
//! * **AWGN**: the per-axis added noise has zero mean and variance `sigma^2`.
//! * **Rayleigh**: `E[|h|^2] = 1` — the fading envelope has unit average power.
//!   Verified indirectly: at high SNR (`sigma ~ 0`) the output power should be
//!   close to the input power (unit input symbols), so the mean received power
//!   `E[|r|^2] ≈ 1`.
//! * **Rician**: same unit-power check; additionally, K→∞ approaches AWGN, so
//!   at a very large K-factor the output power must also be close to input power.

use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};

/// Build a batch of unit-power QPSK-like symbols: all I = 1/√2, Q = 1/√2.
fn unit_batch(frames: usize, syms: usize) -> SymbolBatch {
    let v = std::f32::consts::FRAC_1_SQRT_2;
    let i = vec![vec![v; syms]; frames];
    let q = vec![vec![v; syms]; frames];
    SymbolBatch::new(i, q)
}

/// Build a batch of symbols transmitted at (1, 0) (unit I, zero Q).
fn real_batch(frames: usize, syms: usize) -> SymbolBatch {
    let i = vec![vec![1.0_f32; syms]; frames];
    let q = vec![vec![0.0_f32; syms]; frames];
    SymbolBatch::new(i, q)
}

/// Compute the per-axis noise variance from a sample of (received - transmitted)
/// differences over many symbols.
fn noise_variance_i(received: &SymbolBatch, transmitted: &SymbolBatch) -> f32 {
    let mut sum_sq = 0.0_f64;
    let mut count = 0usize;
    for (r_frame, t_frame) in received.i.iter().zip(transmitted.i.iter()) {
        for (&r, &t) in r_frame.iter().zip(t_frame.iter()) {
            let d = (r - t) as f64;
            sum_sq += d * d;
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    (sum_sq / count as f64) as f32
}

/// Compute mean received power `E[|r|^2]` over all symbols.
fn mean_received_power(batch: &SymbolBatch) -> f32 {
    let mut sum = 0.0_f64;
    let mut count = 0usize;
    for (i_frame, q_frame) in batch.i.iter().zip(batch.q.iter()) {
        for (&ri, &rq) in i_frame.iter().zip(q_frame.iter()) {
            sum += (ri as f64).powi(2) + (rq as f64).powi(2);
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    (sum / count as f64) as f32
}

/// Fast smoke: AWGN noise variance matches sigma^2 within 10% (1000 samples).
///
/// Formula: sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10)).
/// At Es/N0 = 0 dB, sigma^2 = 0.5, sigma = 1/sqrt(2).
#[test]
fn test_awgn_noise_variance_matches_formula() {
    let es_n0_db = 0.0_f32;
    let ch = Awgn::new(es_n0_db, 4);
    let expected_sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(es_n0_db as f64 / 10.0));

    let input = real_batch(1, 2000);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0xDEAD_BEEF);
    ch.apply(&mut batch, &mut rng);

    let measured = noise_variance_i(&batch, &input) as f64;
    let rel_err = ((measured - expected_sigma_sq) / expected_sigma_sq).abs();
    assert!(
        rel_err < 0.10,
        "AWGN I-axis noise variance {measured:.6} vs expected {expected_sigma_sq:.6} \
         (relative error {:.2}% > 10%)",
        rel_err * 100.0
    );
}

/// Fast smoke: AWGN noise has zero mean over 2000 samples.
#[test]
fn test_awgn_noise_mean_zero() {
    let ch = Awgn::new(0.0, 4);
    let input = real_batch(1, 2000);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0x1234_5678);
    ch.apply(&mut batch, &mut rng);

    let noise_mean: f32 = {
        let sum: f32 = batch.i[0]
            .iter()
            .zip(input.i[0].iter())
            .map(|(&r, &t)| r - t)
            .sum();
        sum / 2000.0
    };
    // Mean should be < ~3 * sigma / sqrt(N) ≈ 3 * 0.707 / sqrt(2000) ≈ 0.047.
    assert!(
        noise_mean.abs() < 0.1,
        "AWGN I-axis noise mean {noise_mean:.6} too far from zero"
    );
}

/// Fast smoke: Rayleigh fading preserves unit average power (E[|h|^2] = 1).
///
/// At very high SNR the noise is negligible, so E[|r|^2] ≈ E[|h*x|^2] =
/// E[|h|^2] * E[|x|^2] = 1 * 1 = 1.
#[test]
fn test_rayleigh_unit_average_power() {
    // Very high SNR so noise contribution is < 0.1% of signal power.
    let ch = Rayleigh::new(30.0, 4);
    let input = unit_batch(1, 2000);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0xCAFE_BABE);
    ch.apply(&mut batch, &mut rng);

    let power = mean_received_power(&batch);
    // E[|h|^2] = 1, E[|x|^2] = 1 → expected power ≈ 1.
    assert!(
        (power - 1.0).abs() < 0.1,
        "Rayleigh mean received power {power:.6} too far from 1.0 (expected E[|h|^2]=1)"
    );
}

/// Fast smoke: Rician (K=0 ≡ Rayleigh) preserves unit average power.
#[test]
fn test_rician_k0_unit_average_power() {
    let ch = Rician::new(30.0, 4, 0.0);
    let input = unit_batch(1, 2000);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0x0FF5_1CE5);
    ch.apply(&mut batch, &mut rng);

    let power = mean_received_power(&batch);
    assert!(
        (power - 1.0).abs() < 0.1,
        "Rician K=0 mean received power {power:.6} too far from 1.0"
    );
}

/// Fast smoke: Rician (K=10) preserves unit average power (E[|h|^2] = 1 for all K).
#[test]
fn test_rician_k10_unit_average_power() {
    let ch = Rician::new(30.0, 4, 10.0);
    let input = unit_batch(1, 2000);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0xF00D_1234);
    ch.apply(&mut batch, &mut rng);

    let power = mean_received_power(&batch);
    assert!(
        (power - 1.0).abs() < 0.1,
        "Rician K=10 mean received power {power:.6} too far from 1.0"
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

    let input = unit_batch(3, 64);
    let mut scratch = ChannelScratch::default();

    let out_awgn = awgn.process(&input, &mut scratch).unwrap();
    assert_eq!(out_awgn.i.len(), 3);
    assert_eq!(out_awgn.i[0].len(), 64);

    let out_rayleigh = rayleigh.process(&input, &mut scratch).unwrap();
    assert_eq!(out_rayleigh.i.len(), 3);

    let out_rician = rician.process(&input, &mut scratch).unwrap();
    assert_eq!(out_rician.i.len(), 3);
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

/// Statistical regression: AWGN variance at 10 dB Es/N0.
///
/// sigma^2 = 1 / (2 * 10^(10/10)) = 1/20 = 0.05. Over 5000 samples the
/// sample variance should be within 5% of 0.05.
#[test]
fn test_awgn_variance_10db() {
    let es_n0_db = 10.0_f32;
    let ch = Awgn::new(es_n0_db, 4);
    let expected = 1.0_f64 / (2.0 * 10.0_f64.powf(es_n0_db as f64 / 10.0)); // 0.05

    let input = real_batch(1, 5000);
    let mut batch = input.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(0xABCD_EF01);
    ch.apply(&mut batch, &mut rng);

    let measured = noise_variance_i(&batch, &input) as f64;
    let rel_err = ((measured - expected) / expected).abs();
    assert!(
        rel_err < 0.05,
        "AWGN 10dB variance {measured:.6} vs {expected:.6} (relative error {:.2}% > 5%)",
        rel_err * 100.0
    );
}
