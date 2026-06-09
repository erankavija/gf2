//! [`SimulationResults`] — the aggregate outcome of a [`Pipeline`](crate::Pipeline)
//! run, the v2 successor to the legacy `gf2_coding::simulation` result rows.
//!
//! The per-SNR-point columns are a thin projection of the [`WorkerCounters`]
//! SSOT (`frames` / `errors` / `total_iterations` / `total_bits` /
//! `total_bit_errors`), so the determinism contract that pins those counters
//! (design doc §11) carries through verbatim: `fer = errors / frames`,
//! `errors` is the **frame**-error count (not bit errors), and `mean_iters =
//! total_iterations / frames`. No divergent column definition is introduced —
//! a [`SnrPointResult`] is built directly from a [`WorkerCounters`] via
//! [`SnrPointResult::from_counters`].

use crate::parallel::WorkerCounters;

/// The aggregate result of one SNR point.
///
/// Every numeric column is derived from a [`WorkerCounters`] (the SSOT), so the
/// byte-identity guarantees of design doc §11 hold: `fer` / `frames` / `errors`
/// / `mean_iters` are byte-identical across worker counts at a fixed seed (and,
/// for the hybrid path, run-to-run since it is the same device path twice).
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::SnrPointResult;
/// use gf2_sim::parallel::WorkerCounters;
///
/// let mut c = WorkerCounters::default();
/// c.record_frame(true, 12, 100, 3);
/// c.record_frame(false, 1, 100, 0);
/// let r = SnrPointResult::from_counters(6.5, c);
/// assert_eq!(r.frames, 2);
/// assert_eq!(r.errors, 1);
/// assert!((r.fer - 0.5).abs() < 1e-12);
/// assert!((r.mean_iters - 6.5).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnrPointResult {
    /// The Es/N0 (dB) this point ran at.
    pub es_n0_db: f64,
    /// Frame error rate (`errors / frames`).
    pub fer: f64,
    /// Frames simulated at this point.
    pub frames: u64,
    /// **Frame**-error count (frames whose decoded BBFRAME differs from the
    /// transmitted one) — the `WorkerCounters::errors` semantics, not a bit
    /// count.
    pub errors: u64,
    /// Mean decoder iterations per frame (`total_iterations / frames`).
    pub mean_iters: f64,
    /// Total information bits compared across all frames.
    pub total_bits: u64,
    /// Total information-bit errors across all frames.
    pub total_bit_errors: u64,
    /// Total decoder iterations across all frames.
    pub total_iterations: u64,
}

impl SnrPointResult {
    /// Builds a point result from its Es/N0 and the SSOT [`WorkerCounters`].
    ///
    /// `fer` and `mean_iters` are the counters' derived ratios; `errors` is the
    /// frame-error count. This is the single conversion point so no column is
    /// ever computed two different ways.
    ///
    /// # Arguments
    ///
    /// * `es_n0_db` — the SNR point's Es/N0 in dB.
    /// * `counters` — the aggregated per-SNR-point counters.
    #[must_use]
    pub fn from_counters(es_n0_db: f64, counters: WorkerCounters) -> Self {
        Self {
            es_n0_db,
            fer: counters.fer(),
            frames: counters.frames,
            errors: counters.errors,
            mean_iters: counters.mean_iters(),
            total_bits: counters.total_bits,
            total_bit_errors: counters.total_bit_errors,
            total_iterations: counters.total_iterations,
        }
    }
}

/// The aggregate result of a full [`Pipeline`](crate::Pipeline) run: one
/// [`SnrPointResult`] per simulated Es/N0 point, in sweep order.
///
/// This is the type the §12 migration table's `Pipeline::run` /
/// `Pipeline::run_with_decoder` / `Pipeline::run_parallel` return. Downstream
/// consumers (the D.3 calibration receipt, the campaign-binary migration) read
/// the four contractual columns `fer` / `frames` / `errors` / `mean_iters` off
/// each point.
///
/// # Examples
///
/// ```
/// use gf2_sim::executor::{SimulationResults, SnrPointResult};
/// use gf2_sim::parallel::WorkerCounters;
///
/// let mut c = WorkerCounters::default();
/// c.record_frame(false, 3, 8, 0);
/// let results = SimulationResults {
///     per_point: vec![SnrPointResult::from_counters(6.0, c)],
/// };
/// assert_eq!(results.per_point.len(), 1);
/// assert_eq!(results.per_point[0].frames, 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationResults {
    /// One result per SNR point, in `esn0_db_points` order.
    pub per_point: Vec<SnrPointResult>,
}

impl SimulationResults {
    /// An empty result set (no SNR points run).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            per_point: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_counters_projects_ssot_columns() {
        let mut c = WorkerCounters::default();
        c.record_frame(true, 10, 100, 5);
        c.record_frame(false, 2, 100, 0);
        c.record_frame(false, 4, 100, 0);
        let r = SnrPointResult::from_counters(7.0, c);
        assert_eq!(r.frames, 3);
        assert_eq!(r.errors, 1); // one frame in error
        assert_eq!(r.total_bit_errors, 5);
        assert_eq!(r.total_iterations, 16);
        assert!((r.fer - 1.0 / 3.0).abs() < 1e-12);
        assert!((r.mean_iters - 16.0 / 3.0).abs() < 1e-12);
        assert!((r.es_n0_db - 7.0).abs() < 1e-12);
    }

    #[test]
    fn test_empty_results_have_no_points() {
        assert!(SimulationResults::empty().per_point.is_empty());
    }
}
