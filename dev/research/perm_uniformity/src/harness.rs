// harness.rs — shared statistical primitives for perm-uniformity (JIT 8e4e19a0).
//
// This module is the SSOT for TVD computation and bootstrap CI estimation.
// It is used by both src/main.rs and tests/smoke.rs.

use gf2_core::rng::Lcg;

// ---------------------------------------------------------------------------
// TVD
// ---------------------------------------------------------------------------

/// Compute TVD(empirical, Uniform(q)) from a frequency histogram.
///
/// TVD = (1/2) * sum_{x in F_q} |count[x]/N - 1/q|
///
/// # Arguments
///
/// * `counts` — histogram of observed values, length q.
/// * `n_total` — total number of samples (= sum of counts).
/// * `q` — field order.
pub fn tvd_from_counts(counts: &[u64], n_total: u64, q: u64) -> f64 {
    let uniform_prob = 1.0 / q as f64;
    let mut sum = 0.0_f64;
    for &c in counts.iter() {
        let empirical = c as f64 / n_total as f64;
        sum += (empirical - uniform_prob).abs();
    }
    0.5 * sum
}

// ---------------------------------------------------------------------------
// Bootstrap CI for TVD (independent resampling)
// ---------------------------------------------------------------------------

/// Bootstrap CI for TVD: resample N samples with replacement 1000 times.
///
/// Returns `(ci_lo, ci_hi)` at 95% confidence.
///
/// # Arguments
///
/// * `samples` — field element value (0..q) for each matrix sample.
/// * `q` — field order.
/// * `n_bootstrap` — number of bootstrap resamples (1000 recommended).
/// * `seed` — deterministic seed for the bootstrap RNG.
pub fn bootstrap_tvd_ci(samples: &[u8], q: u64, n_bootstrap: usize, seed: u64) -> (f64, f64) {
    let n = samples.len();
    let mut rng = Lcg::new(seed);
    let mut bootstrap_tvds: Vec<f64> = Vec::with_capacity(n_bootstrap);

    for _ in 0..n_bootstrap {
        let mut counts = vec![0u64; q as usize];
        for _ in 0..n {
            let idx = rng.next_bounded_usize(n);
            counts[samples[idx] as usize] += 1;
        }
        bootstrap_tvds.push(tvd_from_counts(&counts, n as u64, q));
    }
    bootstrap_tvds.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lo_idx = (0.025 * n_bootstrap as f64) as usize;
    let hi_idx = (0.975 * n_bootstrap as f64) as usize;
    (
        bootstrap_tvds[lo_idx],
        bootstrap_tvds[hi_idx.min(n_bootstrap - 1)],
    )
}

// ---------------------------------------------------------------------------
// Bootstrap CI for the *difference* TVD_perm - TVD_det (paired resampling)
// ---------------------------------------------------------------------------

/// Bootstrap the one-sided 95% upper quantile of (TVD_perm - TVD_det).
///
/// This is the correct statistic for criterion 6: the comparison accounts for
/// the sampling uncertainty in *both* TVD estimates simultaneously by resampling
/// the same index set for perm and det on each bootstrap iteration.
///
/// Returns `(diff_mean, diff_q95)` where:
///   * `diff_mean` = point estimate TVD_perm - TVD_det
///   * `diff_q95` = 95th percentile of the bootstrap distribution of
///     (TVD_perm_b - TVD_det_b).
///
/// Criterion 6 PASSES when `diff_q95 < 0`, i.e., even the 95th-percentile
/// bootstrap outcome of (perm - det) is negative.
///
/// # Arguments
///
/// * `perm_samples` — field element (0..q) for each perm(A) evaluation.
/// * `det_samples` — field element (0..q) for each det(A) evaluation (same N,
///   but drawn from an independent RNG stream).
/// * `q` — field order.
/// * `n_bootstrap` — number of bootstrap resamples.
/// * `seed` — deterministic seed.
///
/// # Note on paired vs. independent resampling
///
/// The perm and det streams were sampled from *independent* matrices (different
/// RNG sub-seeds). They are therefore statistically independent samples, not
/// paired observations. Nevertheless, for the purpose of CI-on-the-difference
/// we resample each stream with the *same* bootstrap indices so that the
/// bootstrap variance of the difference reflects the within-resample covariance
/// structure. Because the streams are independent, the covariance is
/// approximately zero, and this reduces to independent bootstrap; using the
/// same index for both avoids a second independent RNG draw and keeps the
/// procedure exact across seeds.
pub fn bootstrap_diff_ci(
    perm_samples: &[u8],
    det_samples: &[u8],
    q: u64,
    n_bootstrap: usize,
    seed: u64,
) -> (f64, f64) {
    assert_eq!(
        perm_samples.len(),
        det_samples.len(),
        "perm and det sample vectors must have equal length"
    );
    let n = perm_samples.len();
    let mut rng = Lcg::new(seed);
    let mut diffs: Vec<f64> = Vec::with_capacity(n_bootstrap);

    for _ in 0..n_bootstrap {
        let mut perm_counts = vec![0u64; q as usize];
        let mut det_counts = vec![0u64; q as usize];
        for _ in 0..n {
            // Resample perm stream independently.
            let pi = rng.next_bounded_usize(n);
            perm_counts[perm_samples[pi] as usize] += 1;
            // Resample det stream independently (separate draw).
            let di = rng.next_bounded_usize(n);
            det_counts[det_samples[di] as usize] += 1;
        }
        let tvd_p = tvd_from_counts(&perm_counts, n as u64, q);
        let tvd_d = tvd_from_counts(&det_counts, n as u64, q);
        diffs.push(tvd_p - tvd_d);
    }
    diffs.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let diff_mean = diffs.iter().sum::<f64>() / n_bootstrap as f64;
    // One-sided 95% upper quantile.
    let q95_idx = ((0.95 * n_bootstrap as f64) as usize).min(n_bootstrap - 1);
    let diff_q95 = diffs[q95_idx];
    (diff_mean, diff_q95)
}

// ---------------------------------------------------------------------------
// Generic cell runner (shared sampling + TVD pattern)
// ---------------------------------------------------------------------------

/// Result of one (q, n) cell.
pub struct CellResult {
    pub q: u64,
    pub n: usize,
    pub n_samples: usize,
    pub tvd_perm: f64,
    pub tvd_perm_ci_lo: f64,
    pub tvd_perm_ci_hi: f64,
    pub tvd_det: f64,
    pub tvd_det_ci_lo: f64,
    pub tvd_det_ci_hi: f64,
    /// 95th-percentile bootstrap of (TVD_perm - TVD_det); negative = criterion 6 PASS.
    pub diff_q95: f64,
    pub mean_us_perm: f64,
    pub mean_us_det: f64,
}

/// Run a (q, n, n_samples) cell using caller-supplied perm and det sampling
/// closures.
///
/// Both closures accept `(rng: &mut Lcg, n: usize)` and return the field
/// element as `u8`.
///
/// Seeds: `perm_seed` drives the perm stream RNG, `det_seed` drives the det
/// stream RNG, `boot_perm_seed` and `boot_det_seed` drive the two independent
/// TVD bootstrap CIs, and `boot_diff_seed` drives the paired difference
/// bootstrap for criterion 6.
#[allow(clippy::too_many_arguments)]
pub fn run_cell<FP, FD>(
    q: u64,
    n: usize,
    n_samples: usize,
    perm_seed: u64,
    det_seed: u64,
    boot_perm_seed: u64,
    boot_det_seed: u64,
    boot_diff_seed: u64,
    mut sample_perm: FP,
    mut sample_det: FD,
) -> CellResult
where
    FP: FnMut(&mut Lcg, usize) -> u8,
    FD: FnMut(&mut Lcg, usize) -> u8,
{
    let mut perm_counts = vec![0u64; q as usize];
    let mut det_counts = vec![0u64; q as usize];
    let mut perm_samples = Vec::with_capacity(n_samples);
    let mut det_samples = Vec::with_capacity(n_samples);

    let mut rng_perm = Lcg::new(perm_seed);
    let t_perm_start = std::time::Instant::now();
    for _ in 0..n_samples {
        let v = sample_perm(&mut rng_perm, n);
        perm_counts[v as usize] += 1;
        perm_samples.push(v);
    }
    let perm_elapsed = t_perm_start.elapsed().as_secs_f64();

    let mut rng_det = Lcg::new(det_seed);
    let t_det_start = std::time::Instant::now();
    for _ in 0..n_samples {
        let v = sample_det(&mut rng_det, n);
        det_counts[v as usize] += 1;
        det_samples.push(v);
    }
    let det_elapsed = t_det_start.elapsed().as_secs_f64();

    let tvd_perm = tvd_from_counts(&perm_counts, n_samples as u64, q);
    let tvd_det = tvd_from_counts(&det_counts, n_samples as u64, q);

    let (pci_lo, pci_hi) = bootstrap_tvd_ci(&perm_samples, q, 1000, boot_perm_seed);
    let (dci_lo, dci_hi) = bootstrap_tvd_ci(&det_samples, q, 1000, boot_det_seed);
    let (_, diff_q95) = bootstrap_diff_ci(&perm_samples, &det_samples, q, 1000, boot_diff_seed);

    CellResult {
        q,
        n,
        n_samples,
        tvd_perm,
        tvd_perm_ci_lo: pci_lo,
        tvd_perm_ci_hi: pci_hi,
        tvd_det,
        tvd_det_ci_lo: dci_lo,
        tvd_det_ci_hi: dci_hi,
        diff_q95,
        mean_us_perm: perm_elapsed * 1e6 / n_samples as f64,
        mean_us_det: det_elapsed * 1e6 / n_samples as f64,
    }
}
