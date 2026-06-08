//! Statistical sanity tests for the AWGN, Rayleigh, and Rician channel stages
//! (issue `db9836e4`, deliverable 5 + [hard] criterion-4).
//!
//! Each channel is exercised over **10000 FRAMES** with genuine per-frame §3
//! reseeking: for every global frame `g` a [`WorkerCtx`] is repositioned to
//! `worker_offset(seed, 0, 0, g)` by the channel's own
//! [`Awgn::apply_for_frame`](gf2_sim::channels::Awgn::apply_for_frame) (and the
//! Rayleigh/Rician equivalents), so each frame draws from its own reserved
//! ChaCha20 stream region — exactly the path the parallel executor exercises.
//! BOTH the mean and the variance of the relevant quantity are then asserted
//! against analytical expectations, with tolerances wide enough to be robust to
//! Monte-Carlo noise yet tight enough to catch a wrong moment.
//!
//! # Analytical moments asserted
//!
//! * **AWGN** (per-axis added noise `n ~ N(0, sigma^2)`):
//!   - `E[n] = 0`
//!   - `Var[n] = sigma^2`, where `sigma^2 = 1 / (2 * 10^(Es/N0_dB / 10))`.
//!
//! * **Rayleigh** (fading coefficient `h ~ CN(0, 1)`, recovered at high SNR as
//!   `r ≈ h` for a unit transmitted symbol `x = 1 + 0j`):
//!   - Per-axis fading component `h_r ~ N(0, 1/2)`: `E[h_r] = 0`, `Var[h_r] = 1/2`.
//!   - Envelope power `|h|^2 ~ Exponential(1)`: `E[|h|^2] = 1`, `Var[|h|^2] = 1`.
//!
//! * **Rician** (K-factor `K`, `h = sqrt(K/(K+1)) + sqrt(1/(K+1)) * CN(0,1)`):
//!   - **Received/fading mean** on the I axis (`x = 1 + 0j`, high SNR):
//!     `E[Re(r)] = E[Re(h)] = los_mag = sqrt(K/(K+1))`. The Q-axis mean is `0`.
//!   - `E[|h|^2] = 1` for all K (unit-power normalization).
//!   - `Var[|h|^2] = (2K + 1) / (K + 1)^2`. For K = 4 this is `9 / 25 = 0.36`,
//!     materially LOWER than Rayleigh's `1.0` — the line-of-sight component
//!     reduces fading variance. We assert the analytical value (within
//!     tolerance) AND that it is well below 1.0.
//!
//! # Test tier
//!
//! 10000-frame loops through these lightweight single-symbol kernels run in a
//! few milliseconds, comfortably under the 5 s fast-tier limit, so all tests
//! here are fast-tier (un-ignored).

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::WorkerCtx;

/// Number of frames per statistical test (deliverable 5: 10000-frame moments).
const FRAMES: usize = 10_000;

/// Fixed base seed for the statistical runs.
const SEED: u64 = 0xDEAD_BEEF;

/// Running mean/variance accumulator (Welford-free two-pass over a Vec).
struct Moments {
    samples: Vec<f64>,
}

impl Moments {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(2 * FRAMES),
        }
    }

    fn push(&mut self, x: f64) {
        self.samples.push(x);
    }

    /// Returns `(mean, population_variance)`.
    fn mean_var(&self) -> (f64, f64) {
        let n = self.samples.len() as f64;
        let mean = self.samples.iter().sum::<f64>() / n;
        let var = self.samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        (mean, var)
    }
}

/// Build a single-symbol transmitted batch at value `(i0, q0)`.
fn one_symbol(i0: f32, q0: f32) -> SymbolBatch {
    SymbolBatch::new(vec![vec![i0]], vec![vec![q0]])
}

/// AWGN: over 10000 frames (each per-frame seeked) the per-axis noise has
/// mean ~ 0 and variance ~ sigma^2.
#[test]
fn test_awgn_noise_mean_and_variance() {
    let es_n0_db = 3.0_f32;
    let ch = Awgn::new(es_n0_db, 4);
    let sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(es_n0_db as f64 / 10.0));

    let mut ctx = WorkerCtx::new(SEED, 0, 0);
    let mut moments = Moments::new();
    for g in 0..FRAMES {
        // Transmit (1, 0); the noise is received - transmitted on each axis.
        let mut batch = one_symbol(1.0, 0.0);
        ch.apply_for_frame(&mut batch, &mut ctx, g);
        moments.push((batch.i[0][0] - 1.0) as f64); // I-axis noise
        moments.push((batch.q[0][0] - 0.0) as f64); // Q-axis noise
    }
    let (mean, var) = moments.mean_var();

    // E[n] = 0 (tolerance ~ several * sigma / sqrt(2*FRAMES)).
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

/// Rayleigh: over 10000 per-frame-seeked frames at high SNR (`r ≈ h`), the
/// per-axis fading component has mean ~ 0 and variance ~ 1/2, and the envelope
/// power `|h|^2` has mean ~ 1 and variance ~ 1 (Exponential(1)).
#[test]
fn test_rayleigh_fading_moments() {
    // Very high SNR so noise is negligible (< 0.1% of fading power): r ≈ h.
    let ch = Rayleigh::new(40.0, 4);

    let mut ctx = WorkerCtx::new(SEED, 0, 0);
    let mut comp = Moments::new(); // per-axis fading component ~ N(0, 1/2)
    let mut power = Moments::new(); // |h|^2 ~ Exponential(1)
    for g in 0..FRAMES {
        let mut batch = one_symbol(1.0, 0.0); // x = 1+0j → r ≈ h
        ch.apply_for_frame(&mut batch, &mut ctx, g);
        let h_r = batch.i[0][0] as f64;
        let h_i = batch.q[0][0] as f64;
        comp.push(h_r);
        comp.push(h_i);
        power.push(h_r * h_r + h_i * h_i);
    }

    let (comp_mean, comp_var) = comp.mean_var();
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

    let (pow_mean, pow_var) = power.mean_var();
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

/// Rician (K=4): over 10000 per-frame-seeked frames at high SNR (`r ≈ h`),
/// assert the **received/fading I-axis mean** `E[Re(h)] = los_mag = sqrt(K/(K+1))`,
/// the Q-axis mean ~ 0, `E[|h|^2] = 1`, and
/// `Var[|h|^2] = (2K+1)/(K+1)^2 = 0.36` (well below Rayleigh's 1.0).
#[test]
fn test_rician_fading_moments() {
    let k = 4.0_f32;
    let kf = k as f64;
    let los_mag = (kf / (kf + 1.0)).sqrt(); // E[Re(h)] = sqrt(K/(K+1)) ≈ 0.8944
    let expected_var = (2.0 * kf + 1.0) / (kf + 1.0).powi(2); // 9/25 = 0.36

    let ch = Rician::new(40.0, 4, k); // high SNR so r ≈ h

    let mut ctx = WorkerCtx::new(SEED, 0, 0);
    let mut i_axis = Moments::new(); // Re(h): mean los_mag
    let mut q_axis = Moments::new(); // Im(h): mean 0
    let mut power = Moments::new(); // |h|^2: mean 1, var 0.36
    for g in 0..FRAMES {
        let mut batch = one_symbol(1.0, 0.0); // x = 1+0j → r ≈ h
        ch.apply_for_frame(&mut batch, &mut ctx, g);
        let h_r = batch.i[0][0] as f64;
        let h_i = batch.q[0][0] as f64;
        i_axis.push(h_r);
        q_axis.push(h_i);
        power.push(h_r * h_r + h_i * h_i);
    }

    // Received/fading mean: E[Re(h)] = los_mag (the LOS component), E[Im(h)] = 0.
    let (i_mean, _) = i_axis.mean_var();
    let rel_mean = ((i_mean - los_mag) / los_mag).abs();
    assert!(
        rel_mean < 0.05,
        "Rician E[Re(h)] {i_mean:.6} vs los_mag {los_mag:.6} (rel err {:.2}% > 5%)",
        rel_mean * 100.0
    );
    let (q_mean, _) = q_axis.mean_var();
    assert!(
        q_mean.abs() < 0.05,
        "Rician E[Im(h)] {q_mean:.6} too far from 0"
    );

    let (pow_mean, pow_var) = power.mean_var();
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

    let v = std::f32::consts::FRAC_1_SQRT_2;
    let input = SymbolBatch::new(vec![vec![v; 64]], vec![vec![v; 64]]);
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
