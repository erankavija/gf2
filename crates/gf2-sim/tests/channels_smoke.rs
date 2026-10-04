//! Statistical moment tests for the AWGN, Rayleigh and Rician channel stages:
//! each channel runs over [`FRAMES`] frames, reseeked per frame by its
//! `apply_for_frame`, and the sample mean and variance are compared with the
//! analytical moments.

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::WorkerCtx;

const FRAMES: usize = 10_000;

const SEED: u64 = 0xDEAD_BEEF;

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

fn one_symbol(i0: f32, q0: f32) -> SymbolBatch {
    SymbolBatch::new(vec![vec![i0]], vec![vec![q0]])
}

#[test]
fn test_awgn_noise_mean_and_variance() {
    let es_n0_db = 3.0_f32;
    let ch = Awgn::new(es_n0_db, 4);
    let sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(es_n0_db as f64 / 10.0));

    let mut ctx = WorkerCtx::new(SEED, 0, 0);
    let mut moments = Moments::new();
    for g in 0..FRAMES {
        let mut batch = one_symbol(1.0, 0.0);
        ch.apply_for_frame(&mut batch, &mut ctx, g);
        moments.push((batch.i[0][0] - 1.0) as f64);
        moments.push((batch.q[0][0] - 0.0) as f64);
    }
    let (mean, var) = moments.mean_var();

    assert!(
        mean.abs() < 0.05,
        "AWGN noise mean {mean:.6} too far from 0 (sigma^2 = {sigma_sq:.6})"
    );
    let rel = ((var - sigma_sq) / sigma_sq).abs();
    assert!(
        rel < 0.05,
        "AWGN noise variance {var:.6} vs sigma^2 {sigma_sq:.6} (rel err {:.2}% > 5%)",
        rel * 100.0
    );
}

/// At high SNR `r ≈ h` for `x = 1`: each axis of `h` is `N(0, 1/2)` and
/// `|h|^2` is `Exponential(1)`, with mean 1 and variance 1.
#[test]
fn test_rayleigh_fading_moments() {
    let ch = Rayleigh::new(40.0, 4);

    let mut ctx = WorkerCtx::new(SEED, 0, 0);
    let mut comp = Moments::new();
    let mut power = Moments::new();
    for g in 0..FRAMES {
        let mut batch = one_symbol(1.0, 0.0);
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
    // Wider tolerance: the fourth-moment estimator has higher Monte Carlo
    // variance.
    assert!(
        (pow_var - 1.0).abs() < 0.12,
        "Rayleigh Var[|h|^2] {pow_var:.6} too far from 1.0"
    );
}

/// At high SNR `r ≈ h` for `x = 1`: `E[Re(h)] = sqrt(K/(K+1))`,
/// `E[Im(h)] = 0`, `E[|h|^2] = 1` and `Var[|h|^2] = (2K+1)/(K+1)^2`, which is
/// 0.36 at K = 4 against Rayleigh's 1.
#[test]
fn test_rician_fading_moments() {
    let k = 4.0_f32;
    let kf = k as f64;
    let los_mag = (kf / (kf + 1.0)).sqrt();
    let expected_var = (2.0 * kf + 1.0) / (kf + 1.0).powi(2);

    let ch = Rician::new(40.0, 4, k);

    let mut ctx = WorkerCtx::new(SEED, 0, 0);
    let mut i_axis = Moments::new();
    let mut q_axis = Moments::new();
    let mut power = Moments::new();
    for g in 0..FRAMES {
        let mut batch = one_symbol(1.0, 0.0);
        ch.apply_for_frame(&mut batch, &mut ctx, g);
        let h_r = batch.i[0][0] as f64;
        let h_i = batch.q[0][0] as f64;
        i_axis.push(h_r);
        q_axis.push(h_i);
        power.push(h_r * h_r + h_i * h_i);
    }

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
    assert!(
        (pow_mean - 1.0).abs() < 0.05,
        "Rician E[|h|^2] {pow_mean:.6} too far from 1.0"
    );
    let rel = ((pow_var - expected_var) / expected_var).abs();
    assert!(
        rel < 0.12,
        "Rician Var[|h|^2] {pow_var:.6} vs analytical {expected_var:.6} (rel err {:.2}% > 12%)",
        rel * 100.0
    );
    assert!(
        pow_var < 0.6,
        "Rician Var[|h|^2] {pow_var:.6} should be well below Rayleigh's 1.0"
    );
}

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
