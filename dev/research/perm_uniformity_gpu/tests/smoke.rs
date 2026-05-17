// Smoke tests for perm-uniformity-gpu (JIT b293af5a).
//
// The substantive GPU-vs-CPU correctness assertion and the determinism check
// run inside the binary itself (`validate_gpu_matches_cpu` before any
// measurement; statistical-column bit-stability is verified by the repro
// script's sha256). These tests exercise the *reused* 8e4e19a0 harness
// surface through this crate's dependency edge so the path-dep wiring is
// covered even on non-hip hosts (where the GPU kernels are absent).

use perm_uniformity::harness::{bootstrap_diff_ci, bootstrap_tvd_ci, tvd_from_counts};

#[test]
fn reused_tvd_is_zero_for_uniform_histogram() {
    // Sanity: the SSOT TVD function is reachable through this crate and
    // behaves as documented (uniform -> 0).
    assert_eq!(tvd_from_counts(&[10, 10, 10], 30, 3), 0.0);
    assert!((tvd_from_counts(&[9, 0, 0], 9, 3) - (1.0 - 1.0 / 3.0)).abs() < 1e-12);
}

#[test]
fn reused_bootstrap_is_deterministic_in_seed() {
    let s = [0u8, 1, 2, 0, 1, 2, 0, 1, 2];
    let a = bootstrap_tvd_ci(&s, 3, 256, 0xC0FFEE);
    let b = bootstrap_tvd_ci(&s, 3, 256, 0xC0FFEE);
    assert_eq!(a, b, "bootstrap CI must be bit-identical for a fixed seed");
}

#[test]
fn reused_diff_statistic_negative_when_perm_more_uniform() {
    // perm near-uniform, det skewed -> (perm - det) bootstrap mean < 0,
    // i.e. the criterion-6 statistic this crate relies on works.
    let perm = [0u8, 1, 2, 0, 1, 2, 0, 1, 2];
    let det = [0u8, 0, 0, 0, 0, 0, 0, 0, 1];
    let (mean, q95) = bootstrap_diff_ci(&perm, &det, 3, 256, 7);
    assert!(mean.is_finite() && q95.is_finite());
    assert!(mean <= 0.0);
}
